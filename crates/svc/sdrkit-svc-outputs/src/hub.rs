//! The outputs: the owner `service.outputs`. Each output that the user set up has a filter, a rate limit, a line format
//! and its own thread behind a bounded queue. An output that is off has no thread and sends nothing.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::owner::{Owner, OwnerKey};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, Setting};
use sdrkit_core::storage::Storage;
use sdrkit_core::{Clock, Record, Results, ResultsDecl, StorageError, StorePath, Values};
use sdrkit_svc_events::service::{failed, no_such_row, table_id};
use sdrkit_svc_events::{choice_key, choice_of_key, Kinds, Observation, Template};
use serde::{Deserialize, Serialize};

use crate::aprsis::{AprsIsSettings, AprsIsSink};
use crate::format::{topic, Filter, Format, Gate, LineFormat};
use crate::http::{WebhookSettings, WebhookSink};
use crate::mqtt::{MqttSettings, MqttSink};
use crate::net::{host_port, Backoff, Url};
use crate::runner::{Item, Sink, SinkRunner, SinkStats};
use crate::spots::{spot_of, SpotFields, SpotSettings, SpotSink};
use crate::stream::{FileSink, TcpServerSink, UdpSink};
use crate::OutputError;
use sdrkit_core::net::Transport;

/// The owner key of the outputs.
pub const KEY: OwnerKey = OwnerKey::of("service.outputs");

/// The file in the host's storage that keeps the outputs.
pub const FILE: &str = "outputs/outputs.json";

sdrkit_core::choice_enum! {
    /// What an output is.
    pub enum OutputKind {
        /// MQTT, with discovery.
        Mqtt = ("mqtt", "MQTT"),
        /// An HTTP POST of each event.
        Webhook = ("webhook", "Webhook"),
        /// A file in the host's storage, rotated by size.
        File = ("file", "File"),
        /// UDP datagrams.
        Udp = ("udp", "UDP"),
        /// A TCP server for other programs.
        TcpServer = ("tcp_server", "TCP server"),
        /// Receive-only APRS-IS gating.
        AprsIs = ("aprs_is", "APRS-IS"),
        /// Spot reports.
        Spots = ("spots", "Spot reports"),
    }
}

/// The row key of an output: its name.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Output", ""));
/// What the output is.
pub const KIND: Setting<field::Choice<OutputKind>> =
    Setting::choice("kind", "Kind", OutputKind::File);
/// The output sends.
pub const ENABLED: Setting<field::Bool> = Setting::new("enabled", "On", "", false);
/// The families that the output takes, separated by commas. Empty takes every family.
pub const FAMILIES: Setting<field::Text> = Setting::new("families", "Families", "", "");
/// The event kinds that the output takes, separated by commas (`"adsb.*"`). Empty takes every kind.
pub const KINDS: Setting<field::Text> = Setting::new("kinds", "Kinds", "", "");
/// Only urgent events.
pub const URGENT_ONLY: Setting<field::Bool> = Setting::new("urgent_only", "Urgent only", "", false);
/// The most events per second. 0 for no limit.
pub const MAX_PER_S: Setting<field::F64> =
    Setting::new("max_per_s", "Most per second", "", 0.0).range(0.0, 1e6, 0.1);
/// The size of the queue, events.
pub const QUEUE: Setting<field::I64> =
    Setting::new("queue", "Queue", "", 1000).range(1.0, 1e6, 1.0);
/// What a stream output writes.
pub const FORMAT: Setting<field::Choice<LineFormat>> =
    Setting::choice("format", "Format", LineFormat::Json);
/// The template of the text, NMEA and KISS formats.
pub const TEMPLATE: Setting<field::Text> = Setting::new("template", "Template", "", "");
/// The fields of the SBS format: `altitude=altitude_ft, squawk=squawk`.
pub const FIELDS: Setting<field::Text> = Setting::new("fields", "Fields", "", "");
/// The address: a URL for MQTT, webhooks and spots, `host:port` for UDP, a TCP server and APRS-IS, a file in the host's
/// storage for a file.
pub const ADDRESS: Setting<field::Text> = Setting::new("address", "Address", "", "");
/// The user name of an MQTT broker.
pub const USER: Setting<field::Text> = Setting::new("user", "User", "", "");
/// The password of an MQTT broker. The tables never show it.
pub const PASSWORD: Setting<field::Text> = Setting::new("password", "Password", "", "");
/// The bearer token of a webhook. The tables never show it.
pub const TOKEN: Setting<field::Text> = Setting::new("token", "Token", "", "");
/// The MQTT client id. Empty makes one.
pub const CLIENT_ID: Setting<field::Text> = Setting::new("client_id", "Client id", "", "");
/// The MQTT keep-alive.
pub const KEEPALIVE: Setting<field::Duration> =
    Setting::new("keepalive", "Keep-alive", "s", 60.0).range(2.0, 65_535.0, 1.0);
/// The MQTT QoS, 0 or 1.
pub const QOS: Setting<field::I64> = Setting::new("qos", "QoS", "", 0).range(0.0, 1.0, 1.0);
/// The MQTT topic, a template.
pub const TOPIC: Setting<field::Text> =
    Setting::new("topic", "Topic", "", "sdrkit/{family}/{kind}/{source}");
/// MQTT messages are retained.
pub const RETAIN: Setting<field::Bool> = Setting::new("retain", "Retain", "", false);
/// The MQTT base topic of the status and discovery state.
pub const BASE: Setting<field::Text> = Setting::new("base", "Base topic", "", "sdrkit");
/// MQTT discovery for home automation.
pub const DISCOVERY: Setting<field::Bool> = Setting::new("discovery", "Discovery", "", false);
/// The MQTT discovery prefix.
pub const DISCOVERY_PREFIX: Setting<field::Text> =
    Setting::new("discovery_prefix", "Discovery prefix", "", "homeassistant");
/// The MQTT node id of discovery.
pub const NODE_ID: Setting<field::Text> = Setting::new("node_id", "Node id", "", "sdrkit");
/// A file rotates before it passes this size, bytes. 0 never rotates.
pub const MAX_BYTES: Setting<field::I64> =
    Setting::new("max_bytes", "Most bytes", "", 16 << 20).range(0.0, 1e13, 1.0);
/// Rotated files kept.
pub const KEEP: Setting<field::I64> =
    Setting::new("keep", "Files kept", "", 5).range(0.0, 100.0, 1.0);
/// A TCP server can listen beyond this computer.
pub const ALLOW_REMOTE: Setting<field::Bool> =
    Setting::new("allow_remote", "Allow remote", "", false);
/// The most clients of a TCP server.
pub const MAX_CLIENTS: Setting<field::I64> =
    Setting::new("max_clients", "Most clients", "", 16).range(1.0, 1000.0, 1.0);
/// The user's call sign, for APRS-IS and spots.
pub const CALLSIGN: Setting<field::Text> = Setting::new("callsign", "Call sign", "", "");
/// The APRS-IS passcode. -1 computes it from the call sign.
pub const PASSCODE: Setting<field::I64> =
    Setting::new("passcode", "Passcode", "", -1).range(-1.0, 65_535.0, 1.0);
/// A repeat of the same packet within this time is not gated again.
pub const DUPE: Setting<field::Duration> =
    Setting::new("dupe", "Repeat window", "s", 30.0).range(0.0, 86_400.0, 1.0);
/// The connection and read timeout.
pub const TIMEOUT: Setting<field::Duration> =
    Setting::new("timeout", "Timeout", "s", 10.0).range(0.1, 120.0, 0.1);
/// The user's locator, for spots.
pub const LOCATOR: Setting<field::Text> = Setting::new("locator", "Locator", "", "");
/// Spots go out in batches this far apart.
pub const BATCH: Setting<field::Duration> =
    Setting::new("batch", "Batch every", "s", 300.0).range(1.0, 86_400.0, 1.0);
/// The same call on the same band is spotted once within this time.
pub const DEDUPE: Setting<field::Duration> =
    Setting::new("dedupe", "Spot once within", "s", 900.0).range(0.0, 86_400.0, 1.0);
/// The most spots in one batch.
pub const MAX_BATCH: Setting<field::I64> =
    Setting::new("max_batch", "Most spots per batch", "", 500).range(1.0, 10_000.0, 1.0);
/// The output's state now.
pub const ON: Argument<field::Bool> = Argument::new(Field::new("on", "On", ""));

/// Adds an output, or replaces the output with this name.
pub const ADD: Command = Command::new("output.add", "Add an output")
    .help("Adds an output or replaces it. Names in the format and the filter must resolve.")
    .args(&[
        &ROW,
        &KIND,
        &ENABLED,
        &FAMILIES,
        &KINDS,
        &URGENT_ONLY,
        &MAX_PER_S,
        &QUEUE,
        &FORMAT,
        &TEMPLATE,
        &FIELDS,
        &ADDRESS,
        &USER,
        &PASSWORD,
        &TOKEN,
        &CLIENT_ID,
        &KEEPALIVE,
        &QOS,
        &TOPIC,
        &RETAIN,
        &BASE,
        &DISCOVERY,
        &DISCOVERY_PREFIX,
        &NODE_ID,
        &MAX_BYTES,
        &KEEP,
        &ALLOW_REMOTE,
        &MAX_CLIENTS,
        &CALLSIGN,
        &PASSCODE,
        &DUPE,
        &TIMEOUT,
        &LOCATOR,
        &BATCH,
        &DEDUPE,
        &MAX_BATCH,
    ]);
/// Turns an output on or off.
pub const ENABLE: Command =
    Command::new("output.enable", "Turn an output on or off").args(&[&ROW, &ON]);
/// Removes an output.
pub const REMOVE: Command = Command::new("output.remove", "Remove an output").args(&[&ROW]);
/// Starts every output that is on.
pub const START: Command = Command::new("start", "Start");
/// Stops every output. Their settings stay.
pub const STOP: Command = Command::new("stop", "Stop");

sdrkit_core::commands! {
    /// The commands of the outputs.
    pub enum OutputCommand {
        /// [`ADD`].
        Add = ADD,
        /// [`ENABLE`].
        Enable = ENABLE,
        /// [`REMOVE`].
        Remove = REMOVE,
        /// [`START`].
        Start = START,
        /// [`STOP`].
        Stop = STOP,
    }
}

/// Items sent.
pub const SENT: Field<field::I64> = Field::new("sent", "Sent", "");
/// Items dropped: the queue was full, or the output returned up after a failure.
pub const DROPPED: Field<field::I64> = Field::new("dropped", "Dropped", "");
/// Failures.
pub const ERRORS: Field<field::I64> = Field::new("errors", "Errors", "");
/// The output holds a live connection.
pub const CONNECTED: Field<field::Bool> = Field::new("connected", "Connected", "");
/// Events that the filter or the rate limit held back.
pub const FILTERED: Field<field::I64> = Field::new("filtered", "Held back", "");
/// The address a TCP server listens on.
pub const LISTEN: Field<field::Text> = Field::new("listen", "Listening on", "");
/// The last failure, for people. It never holds a password or a token.
pub const LAST_ERROR: Field<field::Text> = Field::new("last_error", "Last error", "");
/// The output has a password.
pub const HAS_PASSWORD: Field<field::Bool> = Field::new("has_password", "Password set", "");
/// The output has a token.
pub const HAS_TOKEN: Field<field::Bool> = Field::new("has_token", "Token set", "");
/// The outputs are running.
pub const RUNNING: Field<field::Bool> = Field::new("running", "Running", "");

/// The outputs. The row key is the name.
pub const OUTPUTS: TableDef = TableDef {
    key: "outputs",
    label: "Outputs",
    columns: &[
        &KIND.field,
        &ENABLED.field,
        &FORMAT.field,
        &ADDRESS.field,
        &FAMILIES.field,
        &KINDS.field,
        &SENT,
        &DROPPED,
        &ERRORS,
        &CONNECTED,
        &FILTERED,
        &LISTEN,
        &LAST_ERROR,
        &HAS_PASSWORD,
        &HAS_TOKEN,
    ],
    cap: 1000,
    marks: sdrkit_core::RowMarks::NONE,
};

/// The output that could not start.
pub const OF: Field<field::Text> =
    Field::new("of", "Output", "").refers_to("service.outputs", "outputs");
/// Why, for people.
pub const PROBLEM: Field<field::Text> = Field::new("problem", "Problem", "");

/// The outputs that could not start. The row key is the name.
pub const PROBLEMS: TableDef = TableDef {
    key: "problems",
    label: "Problems",
    columns: &[&OF, &PROBLEM],
    cap: 1000,
    marks: sdrkit_core::RowMarks::NONE,
};

/// An output as the user set it, as kept in [`FILE`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct OutputText {
    kind: String,
    enabled: bool,
    families: String,
    kinds: String,
    urgent_only: bool,
    max_per_s: f64,
    queue: i64,
    format: String,
    template: String,
    fields: String,
    address: String,
    user: String,
    password: String,
    token: String,
    client_id: String,
    keepalive: f64,
    qos: i64,
    topic: String,
    retain: bool,
    base: String,
    discovery: bool,
    discovery_prefix: String,
    node_id: String,
    max_bytes: i64,
    keep: i64,
    allow_remote: bool,
    max_clients: i64,
    callsign: String,
    passcode: i64,
    dupe: f64,
    timeout: f64,
    locator: String,
    batch: f64,
    dedupe: f64,
    max_batch: i64,
}

impl OutputText {
    fn of(args: &Values) -> OutputText {
        let t = |s: &Setting<field::Text>| args.get(s).trim().to_owned();
        OutputText {
            kind: choice_key(args.get(&KIND)).to_owned(),
            enabled: args.get(&ENABLED),
            families: t(&FAMILIES),
            kinds: t(&KINDS),
            urgent_only: args.get(&URGENT_ONLY),
            max_per_s: args.get(&MAX_PER_S),
            queue: args.get(&QUEUE),
            format: choice_key(args.get(&FORMAT)).to_owned(),
            template: args.get(&TEMPLATE).to_owned(),
            fields: t(&FIELDS),
            address: t(&ADDRESS),
            user: t(&USER),
            password: args.get(&PASSWORD).to_owned(),
            token: args.get(&TOKEN).to_owned(),
            client_id: t(&CLIENT_ID),
            keepalive: args.get(&KEEPALIVE),
            qos: args.get(&QOS),
            topic: t(&TOPIC),
            retain: args.get(&RETAIN),
            base: t(&BASE),
            discovery: args.get(&DISCOVERY),
            discovery_prefix: t(&DISCOVERY_PREFIX),
            node_id: t(&NODE_ID),
            max_bytes: args.get(&MAX_BYTES),
            keep: args.get(&KEEP),
            allow_remote: args.get(&ALLOW_REMOTE),
            max_clients: args.get(&MAX_CLIENTS),
            callsign: t(&CALLSIGN),
            passcode: args.get(&PASSCODE),
            dupe: args.get(&DUPE),
            timeout: args.get(&TIMEOUT),
            locator: t(&LOCATOR),
            batch: args.get(&BATCH),
            dedupe: args.get(&DEDUPE),
            max_batch: args.get(&MAX_BATCH),
        }
    }

    fn kind(&self) -> OutputKind {
        choice_of_key(&self.kind).unwrap_or(OutputKind::File)
    }
}

/// A sink ready to run, and the address it listens on (TCP servers).
type Built = (Box<dyn Sink<Item>>, Option<SocketAddr>);

/// One output, resolved.
struct Entry {
    key: String,
    text: OutputText,
    kind: OutputKind,
    gate: Gate,
    format: Format,
    topic: Option<Template>,
    spot: Option<SpotFields>,
    runner: Option<SinkRunner<Item>>,
    listen: Option<SocketAddr>,
    filtered: u64,
    last_stats: SinkStats,
}

fn to_usize(v: i64) -> usize {
    usize::try_from(v).unwrap_or(0)
}

/// The outputs: the owner `service.outputs`.
pub struct Outputs {
    kinds: Arc<Kinds>,
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
    clock: Clock,
    path: StorePath,
    saved: BTreeMap<String, OutputText>,
    entries: Vec<Entry>,
    running: bool,
    text: String,
    buf: Vec<u8>,
    topic: String,
    p: Published,
}

impl Outputs {
    /// Opens the outputs kept at [`FILE`] in `storage`. Network outputs connect through `transport`, and `clock` names an
    /// MQTT client that has no id. No output runs until the command `start`.
    ///
    /// # Errors
    ///
    /// [`OutputError::Storage`] if the file cannot be read. [`OutputError::Json`] if it does not parse.
    pub fn open(
        kinds: Arc<Kinds>,
        storage: Arc<dyn Storage>,
        transport: Arc<dyn Transport>,
        clock: Clock,
    ) -> Result<Self, OutputError> {
        let path = StorePath::new(FILE).map_err(|e| OutputError::Storage {
            path: StorePath::root(),
            source: StorageError::InvalidPath(e),
        })?;
        let saved = match storage.read(&path) {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(StorageError::NotFound) => BTreeMap::new(),
            Err(source) => return Err(OutputError::Storage { path, source }),
        };
        let mut o = Outputs {
            kinds,
            storage,
            transport,
            clock,
            path,
            saved,
            entries: Vec::new(),
            running: false,
            text: String::new(),
            buf: Vec::new(),
            topic: String::new(),
            p: Published::new(&[&RUNNING]).with_tables(&[&OUTPUTS, &PROBLEMS]),
        };
        let keys: Vec<String> = o.saved.keys().cloned().collect();
        for k in keys {
            if let Some(t) = o.saved.get(&k).cloned() {
                match o.resolve(&k, t) {
                    Ok(e) => o.entries.push(e),
                    Err(e) => o.problem(&k, &e),
                }
            }
        }
        o.show_all();
        Ok(o)
    }

    fn save(&self) -> Result<(), OutputError> {
        let data = serde_json::to_vec_pretty(&self.saved)?;
        self.storage
            .write(&self.path, &data)
            .map_err(|source| OutputError::Storage {
                path: self.path.clone(),
                source,
            })
    }

    fn resolve(&self, key: &str, text: OutputText) -> Result<Entry, OutputError> {
        let name = |arg| move |error| OutputError::Name { arg, error };
        let kinds = self
            .kinds
            .kind_list(&text.kinds, 0)
            .map_err(name("kinds"))?;
        let families = Kinds::family_list(&text.families, 0).map_err(name("families"))?;
        let line = choice_of_key(&text.format).unwrap_or(LineFormat::Json);
        let format = Format::resolve(
            line,
            &text.template,
            &text.fields,
            &self.kinds,
            kinds.as_ref(),
        )?;
        let kind = text.kind();
        let topic = match kind {
            OutputKind::Mqtt => Some(
                Template::parse(&text.topic, 0, &self.kinds, kinds.as_ref())
                    .map_err(name("topic"))?,
            ),
            _ => None,
        };
        let spot =
            (kind == OutputKind::Spots).then(|| SpotFields::resolve(&self.kinds, kinds.as_ref()));
        self.check_address(kind, &text)?;
        Ok(Entry {
            key: key.to_owned(),
            gate: Gate::new(Filter {
                families,
                kinds,
                urgent_only: text.urgent_only,
                max_per_s: text.max_per_s,
            }),
            kind,
            format,
            topic,
            spot,
            text,
            runner: None,
            listen: None,
            filtered: 0,
            last_stats: SinkStats::default(),
        })
    }

    fn check_address(&self, kind: OutputKind, t: &OutputText) -> Result<(), OutputError> {
        match kind {
            OutputKind::Mqtt | OutputKind::Webhook | OutputKind::Spots => {
                Url::parse(&t.address).map(|_| ())
            }
            OutputKind::Udp | OutputKind::TcpServer | OutputKind::AprsIs => {
                host_port(&t.address).map(|_| ())
            }
            OutputKind::File => StorePath::new(&t.address)
                .map(|_| ())
                .map_err(|_| OutputError::Setting(crate::BadSetting::Address)),
        }
    }

    fn build(&self, e: &Entry) -> Result<Built, OutputError> {
        let t = &e.text;
        let tr = &self.transport;
        Ok(match e.kind {
            OutputKind::Mqtt => {
                let s = MqttSettings {
                    url: t.address.clone(),
                    client_id: t.client_id.clone(),
                    username: t.user.clone(),
                    password: t.password.clone(),
                    keepalive_s: t.keepalive as u16,
                    qos: u8::try_from(t.qos).unwrap_or(0),
                    topic: t.topic.clone(),
                    retain: t.retain,
                    base: t.base.clone(),
                    discovery: t.discovery,
                    discovery_prefix: t.discovery_prefix.clone(),
                    node_id: t.node_id.clone(),
                    timeout_s: t.timeout,
                };
                (Box::new(MqttSink::new(s, tr.clone(), &self.clock)?), None)
            }
            OutputKind::Webhook => {
                let s = WebhookSettings {
                    url: t.address.clone(),
                    token: t.token.clone(),
                    timeout_s: t.timeout,
                };
                (Box::new(WebhookSink::new(s, tr.clone())?), None)
            }
            OutputKind::File => {
                let path = StorePath::new(&t.address)
                    .map_err(|_| OutputError::Setting(crate::BadSetting::Address))?;
                let keep = u32::try_from(t.keep).unwrap_or(0);
                let sink = FileSink::new(self.storage.clone(), path, t.max_bytes as u64, keep);
                (Box::new(sink), None)
            }
            OutputKind::Udp => {
                let per_line = matches!(
                    choice_of_key(&t.format),
                    Some(LineFormat::Nmea | LineFormat::Sbs)
                );
                (
                    Box::new(UdpSink::new(&t.address, per_line, tr.clone())?),
                    None,
                )
            }
            OutputKind::TcpServer => {
                let s = TcpServerSink::bind(
                    &t.address,
                    t.allow_remote,
                    to_usize(t.max_clients),
                    &**tr,
                )?;
                let a = s.local_addr();
                (Box::new(s), Some(a))
            }
            OutputKind::AprsIs => {
                let (host, port) = host_port(&t.address)?;
                let s = AprsIsSettings {
                    host,
                    port,
                    callsign: t.callsign.clone(),
                    passcode: u16::try_from(t.passcode).ok(),
                    dupe_s: t.dupe,
                    timeout_s: t.timeout,
                };
                (Box::new(AprsIsSink::new(s, tr.clone())?), None)
            }
            OutputKind::Spots => {
                let s = SpotSettings {
                    callsign: t.callsign.clone(),
                    locator: t.locator.clone(),
                    endpoint: t.address.clone(),
                    batch_s: t.batch,
                    dedupe_s: t.dedupe,
                    max_batch: to_usize(t.max_batch),
                };
                (Box::new(SpotSink::new(s, tr.clone())?), None)
            }
        })
    }

    fn start_entry(&mut self, i: usize) {
        let built = self.build(&self.entries[i]);
        let key = self.entries[i].key.clone();
        let started = built.and_then(|(sink, listen)| {
            let e = &self.entries[i];
            let runner =
                SinkRunner::spawn(&e.key, sink, to_usize(e.text.queue), Backoff::default())?;
            Ok((runner, listen))
        });
        match started {
            Ok((runner, listen)) => {
                let e = &mut self.entries[i];
                e.listen = listen;
                e.runner = Some(runner);
                self.p.remove_row(&PROBLEMS, &key);
            }
            Err(err) => self.problem(&key, &err),
        }
    }

    fn stop_entry(e: &mut Entry) {
        if let Some(r) = e.runner.take() {
            e.last_stats = r.stats();
            r.stop();
        }
    }

    fn problem(&mut self, key: &str, err: &OutputError) {
        if let Some(mut r) = self.p.row_mut(&PROBLEMS, key) {
            r.set(&OF, key);
            r.set(&PROBLEM, &err.to_string());
        }
    }

    fn show(&mut self, i: usize) {
        let Some(e) = self.entries.get(i) else { return };
        let stats = e
            .runner
            .as_ref()
            .map_or_else(|| e.last_stats.clone(), SinkRunner::stats);
        let listen = e.listen.map(|a| a.to_string());
        let (key, t, filtered) = (e.key.clone(), e.text.clone(), e.filtered);
        let n = |x: u64| i64::try_from(x).unwrap_or(i64::MAX);
        let Some(mut r) = self.p.row_mut(&OUTPUTS, &key) else {
            return;
        };
        r.set(&KIND.field, t.kind());
        r.set(&ENABLED.field, t.enabled);
        r.set(
            &FORMAT.field,
            choice_of_key(&t.format).unwrap_or(LineFormat::Json),
        );
        r.set(&ADDRESS.field, &t.address);
        r.set(&FAMILIES.field, &t.families);
        r.set(&KINDS.field, &t.kinds);
        r.set(&SENT, n(stats.sent));
        r.set(&DROPPED, n(stats.dropped));
        r.set(&ERRORS, n(stats.errors));
        r.set(&CONNECTED, stats.connected);
        r.set(&FILTERED, n(filtered));
        r.opt(&LISTEN, listen.as_deref());
        r.opt(&LAST_ERROR, stats.last_error.as_deref());
        r.set(&HAS_PASSWORD, !t.password.is_empty());
        r.set(&HAS_TOKEN, !t.token.is_empty());
    }

    fn show_all(&mut self) {
        let id = table_id(&self.p, &OUTPUTS);
        let gone: Vec<String> = self
            .p
            .results()
            .table(id)
            .map(|t| {
                t.rows_newest_first()
                    .map(|(k, _)| k.to_owned())
                    .filter(|k| !self.saved.contains_key(k))
                    .collect()
            })
            .unwrap_or_default();
        for k in gone {
            self.p.remove_row(&OUTPUTS, &k);
        }
        for i in 0..self.entries.len() {
            self.show(i);
        }
        self.p.set(&RUNNING, self.running);
    }

    /// Brings the counters of the `outputs` table up to date. The engine calls it about once a second.
    pub fn tick(&mut self) {
        for i in 0..self.entries.len() {
            self.show(i);
        }
    }

    fn render(&mut self, i: usize, o: &Observation<'_>) -> Option<Item> {
        let e = &self.entries[i];
        if let Some(f) = &e.spot {
            return spot_of(o, f).map(Item::Spot);
        }
        let format = match e.kind {
            OutputKind::AprsIs => {
                e.format.text(o, &mut self.text);
                self.buf.clear();
                self.buf.extend_from_slice(self.text.trim().as_bytes());
                !self.buf.is_empty()
            }
            _ => e.format.render(o, &mut self.text, &mut self.buf),
        };
        if !format {
            return None;
        }
        self.topic.clear();
        if let Some(t) = &e.topic {
            topic(t, o, &mut self.topic);
        }
        Some(Item::Packet {
            topic: self.topic.as_str().into(),
            family: o.family,
            body: self.buf.as_slice().into(),
        })
    }

    /// Offers an observation to every output that is on and takes it. Never blocks. An observation that no output takes
    /// allocates nothing. For each output that takes it, the line is written into a reused buffer and copied once into
    /// the queue.
    pub fn submit(&mut self, o: &Observation<'_>) {
        if !self.running {
            return;
        }
        let mut now: Option<Instant> = None;
        for i in 0..self.entries.len() {
            let e = &mut self.entries[i];
            if e.runner.is_none() {
                continue;
            }
            if !e.gate.wants(o) {
                continue;
            }
            let t = *now.get_or_insert_with(Instant::now);
            if !e.gate.admit(o, t) {
                e.filtered += 1;
                continue;
            }
            if let Some(item) = self.render(i, o) {
                if let Some(r) = &self.entries[i].runner {
                    r.submit(item);
                }
            }
        }
    }

    /// Sends an observation to one output past its filter (the automation's `output` request).
    ///
    /// # Errors
    ///
    /// [`OutputError::NoSuchOutput`], [`OutputError::Off`] or [`OutputError::QueueFull`].
    pub fn send_to(&mut self, row: &str, o: &Observation<'_>) -> Result<(), OutputError> {
        let i = self
            .entries
            .iter()
            .position(|e| e.key == row)
            .ok_or_else(|| OutputError::NoSuchOutput(row.to_owned()))?;
        if self.entries[i].runner.is_none() {
            return Err(OutputError::Off);
        }
        let Some(item) = self.render(i, o) else {
            return Ok(());
        };
        match &self.entries[i].runner {
            Some(r) if r.submit(item) => Ok(()),
            Some(_) => Err(OutputError::QueueFull),
            None => Err(OutputError::Off),
        }
    }

    fn index(&self, row: &str) -> Result<usize, CommandError> {
        self.entries
            .iter()
            .position(|e| e.key == row)
            .ok_or_else(|| no_such_row(&self.p, &OUTPUTS, row))
    }
}

impl Owner for Outputs {
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.p.results().decl()
    }
    fn results(&self) -> &Results {
        self.p.results()
    }
    fn problem(&mut self, text: &str) {
        self.p.results_mut().problem(text);
    }
    fn commands(&self) -> &Commands {
        OutputCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match OutputCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            OutputCommand::Add => {
                let key = args.arg(&ROW)?.trim().to_owned();
                if key.is_empty() {
                    return Err(failed(OutputError::Setting(crate::BadSetting::NoName)));
                }
                let text = OutputText::of(args);
                let e = self.resolve(&key, text.clone()).map_err(failed)?;
                if let Some(i) = self.entries.iter().position(|x| x.key == key) {
                    let mut old = self.entries.remove(i);
                    Self::stop_entry(&mut old);
                }
                self.saved.insert(key.clone(), text);
                self.entries.push(e);
                self.entries.sort_by(|a, b| a.key.cmp(&b.key));
                let i = self.index(&key)?;
                if self.running && self.entries[i].text.enabled {
                    self.start_entry(i);
                }
            }
            OutputCommand::Enable => {
                let i = self.index(args.arg(&ROW)?.trim())?;
                let on = args.arg(&ON)?;
                let key = self.entries[i].key.clone();
                self.entries[i].text.enabled = on;
                if let Some(t) = self.saved.get_mut(&key) {
                    t.enabled = on;
                }
                match (on, self.entries[i].runner.is_some()) {
                    (true, false) if self.running => self.start_entry(i),
                    (false, true) => Self::stop_entry(&mut self.entries[i]),
                    _ => {}
                }
            }
            OutputCommand::Remove => {
                let i = self.index(args.arg(&ROW)?.trim())?;
                let mut e = self.entries.remove(i);
                Self::stop_entry(&mut e);
                self.saved.remove(&e.key);
                self.p.remove_row(&PROBLEMS, &e.key);
            }
            OutputCommand::Start => {
                self.running = true;
                for i in 0..self.entries.len() {
                    if self.entries[i].text.enabled && self.entries[i].runner.is_none() {
                        self.start_entry(i);
                    }
                }
                self.show_all();
                return Ok(None);
            }
            OutputCommand::Stop => {
                self.running = false;
                for e in &mut self.entries {
                    Self::stop_entry(e);
                }
                self.show_all();
                return Ok(None);
            }
        }
        self.show_all();
        self.save().map_err(failed)?;
        Ok(None)
    }
}
