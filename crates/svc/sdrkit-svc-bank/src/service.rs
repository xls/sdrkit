//! The bank service as an owner: channels and spans of channels on one wide input, each channel decoded by a receiver in a session
//! of its own. The service holds no session: it requests that the engine start and stop the receivers, passes the engine the
//! components that cut the channels from the wide input, and reads the carrier levels that the engine passes to it.

use crate::component::{parse_raster, CarrierScan, ChannelOut, Channelize};
use crate::detect::{Gate, Reading};
use crate::error::BankError;
use crate::plan::{self, Path};
use crate::spec::{channel_rate, ChannelSpec, Raster, SpanOpen, SpanSpec};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, keys, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::{
    Owner, OwnerKey, Record, RecordDecl, Results, ResultsDecl, Schema, Setting, SettingId,
    StackLookup, StateBlob, TableId, Value, Values,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The owner key of the bank service.
pub const KEY: OwnerKey = OwnerKey::of("service.bank");
const OWNER: &str = "service.bank";
const STATE_VERSION: u32 = 1;

const MAX_CHANNELS: usize = 1024;
const MAX_SPANS: usize = 64;
const MAX_CARRIERS: usize = 100_000;

sdrkit_core::choice_enum! {
    /// Where a channel stands.
    pub enum ChannelState {
        /// The engine has not started its receiver yet.
        Starting = ("starting", "Starting"),
        /// Its receiver runs.
        Running = ("running", "Running"),
        /// It lies outside the wide input. It is kept and runs again when a retune brings it back.
        Outside = ("outside", "Outside"),
    }
}

sdrkit_core::choice_enum! {
    /// How the samples of a channel are cut from the wide input.
    pub enum PathKind {
        /// A bin of the shared filterbank.
        Filterbank = ("filterbank", "Shared filterbank"),
        /// Its own filter: the channel is too wide for a bin.
        Own = ("own", "Own filter"),
        /// Nothing: the channel is outside the wide input.
        None = ("none", "None"),
    }
}

sdrkit_core::choice_enum! {
    /// What the command `set` does to the squelch of a channel.
    pub enum SquelchChange {
        /// Keeps it as it is.
        Keep = ("keep", "Keep"),
        /// Turns it off: the channel is always open.
        Off = ("off", "Off"),
        /// Turns it on at `squelch_db`.
        On = ("on", "On"),
    }
}

/// What the engine must do for the bank, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelChange {
    /// Start a session for channel `channel` that decodes the output of the channelizer for it, then call
    /// [`BankService::attached`].
    Start {
        /// The row key of the channel.
        channel: String,
        /// The row key of the radio whose stream the channel is cut from, in the engine's radios table.
        radio: String,
        /// Its frequency, Hz.
        freq_hz: f64,
        /// Its width, Hz.
        bandwidth_hz: f64,
        /// The rate of its samples, Hz.
        rate_hz: f64,
        /// The stack id.
        stack: String,
        /// The submode. Empty for the default.
        submode: String,
    },
    /// Stop the session of channel `channel`.
    Stop {
        /// The row key of the channel.
        channel: String,
        /// The row key of its session in the engine's sessions table, if it started.
        session: Option<String>,
    },
    /// The channels, the spans or the wide input changed: make the channelizer and the carrier scan again
    /// ([`BankService::channelizer`], [`BankService::carrier_scan`]).
    Layout {
        /// The row key of the radio of a span that was added, so that the engine can start on that radio before any channel
        /// names it. `None` for any other change.
        radio: Option<String>,
    },
}

// Settings.
const MAX_BINS: Setting<field::I64> = Setting::new("max_bins", "Most filterbank bins", "", 4096)
    .range(8.0, 65_536.0, 1.0)
    .help("The most bins of the shared filterbank. More bins mean less work after it, and a longer filter.");
const HYSTERESIS: Setting<field::F64> =
    Setting::new("hysteresis_db", "Squelch hysteresis", "dB", 3.0)
        .range(0.0, 30.0, 0.0)
        .help("How far below the squelch level a channel must fall before it starts to close.");
const HANG: Setting<field::Duration> = Setting::new("hang", "Squelch hang", "s", 1.0)
    .range(0.0, 60.0, 0.0)
    .help("How long a squelch stays open after the level falls.");

sdrkit_core::settings! {
    /// The settings of the bank service.
    pub enum BankSetting {
        /// The most bins of the filterbank.
        MaxBins = MAX_BINS,
        /// The squelch hysteresis.
        Hysteresis = HYSTERESIS,
        /// The squelch hang time.
        Hang = HANG,
    }
}

// Results.
const LABEL: Field<field::Text> = Field::new("label", "Name", "");
const BANDWIDTH: Field<field::Hz> = Field::new("bandwidth_hz", "Width", "Hz");
const RATE: Field<field::Hz> = Field::new("rate_hz", "Rate", "Hz");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const SUBMODE: Field<field::Text> = Field::new("submode", "Submode", "");
const OF_SPAN: Field<field::Text> = Field::new("span", "Span", "").refers_to_table("spans");
const STATE: Field<field::Choice<ChannelState>> = Field::new("state", "State", "");
const PATH: Field<field::Choice<PathKind>> = Field::new("path", "Cut", "");
const OPEN: Field<field::Bool> = Field::new("open", "Squelch open", "");
const SQUELCH: Field<field::F64> = Field::new("squelch_db", "Squelch", "dB");
const SESSION: Field<field::Text> =
    Field::new("session", "Session", "").refers_to("engine", "sessions");
const RADIO: Field<field::Text> = Field::new("radio", "Radio", "").refers_to("engine", "radios");
const OF_CHANNEL: Field<field::Text> =
    Field::new("channel", "Channel", "").refers_to_table("channels");
const ACTIVE: Field<field::Bool> = Field::new("active", "Active", "");
const SPAN_OPEN: Field<field::Choice<SpanOpen>> = Field::new("open", "Stacks run", "");
const THRESHOLD: Field<field::F64> = Field::new("threshold_db", "Threshold", "dB");
const IDLE: Field<field::Duration> = Field::new("idle_s", "Idle time", "s");
const MAX_RX: Field<field::I64> = Field::new("max_receivers", "Most stacks", "");
const COUNT: Field<field::I64> = Field::new("count", "Raster channels", "");
const CHANNEL_COUNT: Field<field::I64> = Field::new("channels", "Channels", "");
const BINS: Field<field::I64> = Field::new("bins", "Filterbank bins", "");
const WAITING: Field<field::I64> = Field::new("waiting", "Carriers waiting", "");

/// The channels, each decoded in a session of its own. The row key is `c<n>`. The results of a channel are the results of its
/// session, which the column `session` names.
pub const CHANNELS: TableDef = TableDef {
    key: "channels",
    label: "Channels",
    columns: &[
        &LABEL,
        &RADIO,
        &keys::FREQ_HZ,
        &BANDWIDTH,
        &RATE,
        &STACK,
        &SUBMODE,
        &OF_SPAN,
        &STATE,
        &PATH,
        &keys::LEVEL_DBFS,
        &keys::SNR_DB,
        &OPEN,
        &SQUELCH,
        &SESSION,
    ],
    cap: MAX_CHANNELS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// Every raster channel of every span, with its level and the channel that decodes it. The row key is `<span>/<n>`.
pub const CARRIERS: TableDef = TableDef {
    key: "carriers",
    label: "Carriers",
    columns: &[
        &OF_SPAN,
        &keys::FREQ_HZ,
        &keys::LEVEL_DBFS,
        &keys::SNR_DB,
        &ACTIVE,
        &OF_CHANNEL,
    ],
    cap: MAX_CARRIERS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The spans. The row key is `s<n>`.
pub const SPANS: TableDef = TableDef {
    key: "spans",
    label: "Spans",
    columns: &[
        &LABEL, &RADIO, &STACK, &SUBMODE, &BANDWIDTH, &RATE, &SPAN_OPEN, &THRESHOLD, &IDLE,
        &MAX_RX, &COUNT,
    ],
    cap: MAX_SPANS,
    marks: sdrkit_core::RowMarks::NONE,
};

// Commands.
const A_ROW: Argument<field::Text> = Argument::new(Field::new("row", "Row", ""));
const A_FREQ: Argument<field::Hz> = Argument::new(keys::FREQ_HZ);
const A_BANDWIDTH: Setting<field::Hz> =
    Setting::new("bandwidth_hz", "Width", "Hz", 12.5e3).range(1.0, 1e9, 0.0);
const A_RATE: Setting<field::Hz> = Setting::new("rate_hz", "Rate", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .help("The rate that the stack gets. 0 takes the least that the stack and the width allow.");
const A_STACK: Argument<field::Text> = Argument::new(STACK);
const A_RADIO: Argument<field::Text> =
    Argument::new(RADIO).help("The row key of the radio in the engine's radios table.");
const A_SUBMODE: Setting<field::Text> = Setting::new("submode", "Submode", "", "");
const A_LABEL: Setting<field::Text> = Setting::new("label", "Name", "", "");
const A_HAS_SQUELCH: Setting<field::Bool> = Setting::new("squelch", "Squelch", "", false);
const A_SQUELCH: Setting<field::F64> =
    Setting::new("squelch_db", "Squelch", "dB", 10.0).range(-20.0, 60.0, 0.0);
const A_SET_FREQ: Setting<field::Hz> = Setting::new("freq_hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .zero_means("No change");
const A_SET_BANDWIDTH: Setting<field::Hz> = Setting::new("bandwidth_hz", "Width", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .zero_means("No change");
const A_SET_LABEL: Setting<field::Text> =
    Setting::new("label", "Name", "", "").help("Empty keeps the name.");
const A_SET_SQUELCH: Setting<field::Choice<SquelchChange>> =
    Setting::choice("squelch", "Squelch", SquelchChange::Keep);
const A_RASTER: Argument<field::Text> = Argument::new(Field::new("raster", "Raster", ""))
    .help("first:step:count in Hz, or a list of frequencies in Hz.");
const A_OPEN: Setting<field::Choice<SpanOpen>> =
    Setting::choice("open", "Stacks run", SpanOpen::Always);
const A_THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Threshold", "dB", 8.0).range(-20.0, 60.0, 0.0);
const A_IDLE: Setting<field::Duration> =
    Setting::new("idle_s", "Idle time", "s", 5.0).range(0.0, 3600.0, 0.0);
const A_MAX_RX: Setting<field::I64> =
    Setting::new("max_receivers", "Most stacks", "", 64).range(1.0, MAX_CHANNELS as f64, 1.0);
const R_ROW: Field<field::Text> = Field::new("row", "Row", "");

const C_ADD: Command = Command::new("add", "Add channel")
    .help("Adds a channel. The engine starts its stack in a session of its own.")
    .args(&[
        &A_RADIO,
        &A_FREQ,
        &A_BANDWIDTH,
        &A_RATE,
        &A_STACK,
        &A_SUBMODE,
        &A_LABEL,
        &A_HAS_SQUELCH,
        &A_SQUELCH,
    ])
    .reply(&[&R_ROW]);
const C_REMOVE: Command = Command::new("remove", "Remove channel")
    .help("Removes a channel and stops its stack.")
    .args(&[&A_ROW]);
const C_SET: Command = Command::new("set", "Change channel")
    .help("Changes the frequency, the width, the name or the squelch of a channel. Volume, pan, mute and solo are settings of its session.")
    .args(&[
        &A_ROW,
        &A_SET_FREQ,
        &A_SET_BANDWIDTH,
        &A_SET_LABEL,
        &A_SET_SQUELCH,
        &A_SQUELCH,
    ]);
const C_SPAN_ADD: Command = Command::new("span.add", "Add span")
    .help("Decodes every carrier of one stack on a raster, each in its own stack.")
    .args(&[
        &A_RADIO,
        &A_LABEL,
        &A_STACK,
        &A_SUBMODE,
        &A_RASTER,
        &A_BANDWIDTH,
        &A_RATE,
        &A_OPEN,
        &A_THRESHOLD,
        &A_IDLE,
        &A_MAX_RX,
    ])
    .reply(&[&R_ROW]);
const C_SPAN_REMOVE: Command = Command::new("span.remove", "Remove span")
    .help("Removes a span and stops the stacks of its channels.")
    .args(&[&A_ROW]);

sdrkit_core::commands! {
    /// The commands of the bank service.
    pub enum BankCommand {
        /// Adds a channel.
        Add = C_ADD,
        /// Removes a channel.
        Remove = C_REMOVE,
        /// Changes a channel.
        Set = C_SET,
        /// Adds a span.
        SpanAdd = C_SPAN_ADD,
        /// Removes a span.
        SpanRemove = C_SPAN_REMOVE,
    }
}

/// One channel and its state in the service.
#[derive(Clone, Debug)]
struct Chan {
    key: String,
    spec: ChannelSpec,
    span: Option<String>,
    rate_hz: f64,
    path: PathKind,
    session: Option<String>,
    gate: Gate,
}

/// One raster channel of a span.
#[derive(Clone, Debug)]
struct Carrier {
    key: String,
    freq_hz: f64,
    gate: Gate,
    reading: Reading,
    channel: Option<String>,
    quiet_since: Option<f64>,
}

#[derive(Clone, Debug)]
struct Span {
    key: String,
    spec: SpanSpec,
    carriers: Vec<Carrier>,
}

/// The saved form of the bank: the channels added by hand and the spans.
#[derive(Serialize, Deserialize)]
struct Saved {
    next: u64,
    channels: Vec<ChannelSpec>,
    spans: Vec<SpanSpec>,
}

/// The bank service.
pub struct BankService {
    stacks: StackLookup,
    values: Values,
    channels: Vec<Chan>,
    spans: Vec<Span>,
    next: u64,
    input: Option<(f64, f64)>,
    bins: Option<usize>,
    changes: Vec<ChannelChange>,
    p: Published,
    ids: Ids,
}

#[derive(Clone, Copy, Debug)]
struct Ids {
    channels: TableId,
    spans: TableId,
}

impl std::fmt::Debug for BankService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BankService")
            .field("channels", &self.channels.len())
            .field("spans", &self.spans.len())
            .finish()
    }
}

fn decl() -> Arc<ResultsDecl> {
    Arc::new(
        ResultsDecl::new()
            .with_fields(&[&CHANNEL_COUNT, &BINS, &WAITING])
            .with_table(&CHANNELS)
            .with_table(&CARRIERS)
            .with_table(&SPANS),
    )
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// The error of an argument that the service refuses: [`CommandError::BadArgument`] with `error` made from its id.
fn refused(
    args: &Values,
    key: &'static str,
    error: impl Fn(SettingId) -> SettingError,
) -> CommandError {
    match args.schema().id(key) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: error(arg),
        },
        Err(_) => CommandError::NotAnArgument { key },
    }
}

fn reply(key: &str) -> Record {
    let d = Arc::new(RecordDecl::of(&[&R_ROW]));
    let mut r = Record::new(d);
    r.set(&R_ROW, key);
    r
}

impl BankService {
    /// Makes an empty bank. `stacks` finds the descriptor of a stack, for its least rate.
    pub fn new(stacks: StackLookup) -> Self {
        let p = Published::from_decl(decl());
        let d = p.results().decl();
        let ids = Ids {
            channels: d
                .table_id(CHANNELS.key)
                .expect("the service declares its tables"),
            spans: d
                .table_id(SPANS.key)
                .expect("the service declares its tables"),
        };
        let mut s = BankService {
            stacks,
            values: Values::defaults(&BankSetting::schema()),
            channels: Vec::new(),
            spans: Vec::new(),
            next: 0,
            input: None,
            bins: None,
            changes: Vec::new(),
            p,
            ids,
        };
        s.p.set(&CHANNEL_COUNT, 0);
        s.p.set(&WAITING, 0);
        s
    }

    /// Tells the bank the rate and the centre of the wide input, after a start or a retune. Channels that leave it are kept and
    /// listed as outside.
    pub fn input(&mut self, rate_hz: f64, centre_hz: f64) {
        self.input = (rate_hz > 0.0).then_some((rate_hz, centre_hz));
        self.replan();
    }

    /// Takes what the engine must do since the last call, in order.
    pub fn take_changes(&mut self) -> Vec<ChannelChange> {
        std::mem::take(&mut self.changes)
    }

    /// The engine started the session `session` (its row key in the engine's sessions table) for channel `channel`.
    ///
    /// # Errors
    ///
    /// [`BankError::NoChannel`] if the bank has no such channel (it was removed meanwhile).
    pub fn attached(&mut self, channel: &str, session: &str) -> Result<(), BankError> {
        let c = self
            .channels
            .iter_mut()
            .find(|c| c.key == channel)
            .ok_or_else(|| BankError::NoChannel {
                key: channel.to_owned(),
            })?;
        c.session = Some(session.to_owned());
        let key = c.key.clone();
        self.publish_channel(&key);
        Ok(())
    }

    /// The channelizer for the channels now, and the row key of the channel of each output (`ch0` is the first).
    pub fn channelizer(&self) -> (Channelize, Vec<String>) {
        let outs: Vec<ChannelOut> = self
            .channels
            .iter()
            .map(|c| ChannelOut {
                freq_hz: c.spec.freq_hz,
                bandwidth_hz: c.spec.bandwidth_hz,
                rate_hz: c.rate_hz,
            })
            .collect();
        let keys = self.channels.iter().map(|c| c.key.clone()).collect();
        (Channelize::new(&outs, self.max_bins()), keys)
    }

    /// The channels whose levels the bank reads, each a frequency and a width in Hz: the channels, then the raster channels of
    /// each span. [`BankService::levels`] takes the readings in this order.
    pub fn watched(&self) -> Vec<(f64, f64)> {
        self.channels
            .iter()
            .map(|c| (c.spec.freq_hz, c.spec.bandwidth_hz))
            .chain(self.spans.iter().flat_map(|s| {
                s.carriers
                    .iter()
                    .map(move |c| (c.freq_hz, s.spec.bandwidth_hz))
            }))
            .collect()
    }

    /// The carrier scan for [`BankService::watched`], to run on the shared spectrum of the wide input.
    pub fn carrier_scan(&self) -> CarrierScan {
        CarrierScan::watching(&self.watched())
    }

    /// Takes the readings of [`BankService::watched`], in that order, at `now_s` seconds on the signal clock. It moves the
    /// squelch of each channel, and starts and stops the receivers of the spans that run on activity.
    ///
    /// # Errors
    ///
    /// [`BankError::Readings`] if `readings` does not hold one reading for each watched channel. Nothing changes then.
    pub fn levels(&mut self, now_s: f64, readings: &[Reading]) -> Result<(), BankError> {
        let n = self.channels.len();
        let expected = n + self.spans.iter().map(|s| s.carriers.len()).sum::<usize>();
        if readings.len() != expected {
            return Err(BankError::Readings {
                expected,
                got: readings.len(),
            });
        }
        let (own, rest) = readings.split_at(n);
        for (i, r) in own.iter().enumerate() {
            let c = &mut self.channels[i];
            let open = match c.spec.squelch_db {
                Some(_) => c.gate.update(r.snr_db, now_s),
                None => true,
            };
            let key = c.key.clone();
            if let Some(mut row) = self.p.row_mut(&CHANNELS, &key) {
                row.set(&keys::LEVEL_DBFS, f64::from(r.level_dbfs).round());
                row.set(&keys::SNR_DB, f64::from(r.snr_db).round());
                row.set(&OPEN, open);
            }
        }
        let mut at = 0;
        let mut waiting = 0;
        for si in 0..self.spans.len() {
            let len = self.spans[si].carriers.len();
            let part = &rest[at..at + len];
            at += len;
            for (c, r) in self.spans[si].carriers.iter_mut().zip(part) {
                c.reading = *r;
                let active = c.gate.update(r.snr_db, now_s);
                c.quiet_since = if active {
                    None
                } else {
                    c.quiet_since.or(Some(now_s))
                };
            }
            if self.spans[si].spec.open == SpanOpen::OnActivity {
                waiting += self.step_span(si, now_s);
            }
            self.publish_carriers(si);
        }
        self.p.set(&WAITING, count(waiting));
        Ok(())
    }

    /// Starts receivers on the strongest active carriers of span `si` up to its cap, and stops those quiet for the idle time.
    /// Returns the active carriers that wait for room.
    fn step_span(&mut self, si: usize, now_s: f64) -> usize {
        let idle = self.spans[si].spec.idle_s;
        let quiet: Vec<String> = self.spans[si]
            .carriers
            .iter()
            .filter(|c| c.quiet_since.is_some_and(|t| now_s - t >= idle))
            .filter_map(|c| c.channel.clone())
            .collect();
        for key in quiet {
            self.drop_channel(&key);
        }
        let open = self.spans[si]
            .carriers
            .iter()
            .filter(|c| c.channel.is_some())
            .count();
        let mut want: Vec<usize> = (0..self.spans[si].carriers.len())
            .filter(|&i| {
                let c = &self.spans[si].carriers[i];
                c.channel.is_none() && c.gate.is_open()
            })
            .collect();
        want.sort_by(|&a, &b| {
            let c = &self.spans[si].carriers;
            c[b].reading.snr_db.total_cmp(&c[a].reading.snr_db)
        });
        let room = self.spans[si].spec.max_receivers.saturating_sub(open);
        let waiting = want.len().saturating_sub(room);
        for i in want.into_iter().take(room) {
            let key = self.span_channel(si, i);
            self.spans[si].carriers[i].channel = Some(key);
        }
        waiting
    }

    /// Adds the channel of carrier `i` of span `si` and returns its row key.
    fn span_channel(&mut self, si: usize, i: usize) -> String {
        let s = &self.spans[si];
        let spec = ChannelSpec {
            radio: s.spec.radio.clone(),
            freq_hz: s.carriers[i].freq_hz,
            bandwidth_hz: s.spec.bandwidth_hz,
            rate_hz: s.spec.rate_hz,
            stack: s.spec.stack.clone(),
            submode: s.spec.submode.clone(),
            label: format!("{} {:.4} MHz", s.spec.label, s.carriers[i].freq_hz / 1e6),
            squelch_db: None,
        };
        let span = Some(s.key.clone());
        self.add_channel(spec, span)
    }

    fn max_bins(&self) -> usize {
        usize::try_from(self.values.get(&MAX_BINS)).unwrap_or(8)
    }

    fn min_rate(&self, stack: &str) -> f64 {
        (self.stacks)(stack).map_or(0.0, |d| d.tuning.min_rate_hz)
    }

    fn gate(&self, squelch_db: Option<f64>) -> Gate {
        let mut g = Gate::new(
            squelch_db.unwrap_or(0.0) as f32,
            self.values.get(&HYSTERESIS) as f32,
            self.values.get(&HANG),
        );
        if squelch_db.is_none() {
            g.force(true);
        }
        g
    }

    fn add_channel(&mut self, spec: ChannelSpec, span: Option<String>) -> String {
        self.next += 1;
        let key = format!("c{}", self.next);
        let gate = self.gate(spec.squelch_db);
        self.channels.push(Chan {
            key: key.clone(),
            spec,
            span,
            rate_hz: 0.0,
            path: PathKind::None,
            session: None,
            gate,
        });
        self.replan();
        let c = self.channels.last().expect("a channel was just added");
        self.changes.push(ChannelChange::Start {
            channel: key.clone(),
            radio: c.spec.radio.clone(),
            freq_hz: c.spec.freq_hz,
            bandwidth_hz: c.spec.bandwidth_hz,
            rate_hz: c.rate_hz,
            stack: c.spec.stack.clone(),
            submode: c.spec.submode.clone(),
        });
        key
    }

    fn drop_channel(&mut self, key: &str) -> bool {
        let Some(i) = self.channels.iter().position(|c| c.key == key) else {
            return false;
        };
        let c = self.channels.remove(i);
        for s in &mut self.spans {
            for car in &mut s.carriers {
                if car.channel.as_deref() == Some(key) {
                    car.channel = None;
                }
            }
        }
        self.p.remove_row(&CHANNELS, key);
        self.changes.push(ChannelChange::Stop {
            channel: c.key,
            session: c.session,
        });
        self.replan();
        true
    }

    /// Plans the cut of every channel on the wide input, and publishes the channels.
    fn replan(&mut self) {
        let max_bins = self.max_bins();
        match self.input {
            Some((rate, centre)) => {
                let list: Vec<(f64, f64)> = self
                    .channels
                    .iter()
                    .map(|c| (c.spec.freq_hz - centre, c.spec.bandwidth_hz))
                    .collect();
                let plan = plan::plan(rate, &list, max_bins);
                self.bins = plan.bins;
                for (i, path) in plan.paths.iter().enumerate() {
                    let min = self.min_rate(&self.channels[i].spec.stack);
                    let c = &mut self.channels[i];
                    c.rate_hz = channel_rate(c.spec.rate_hz, min, c.spec.bandwidth_hz, rate);
                    c.path = match path {
                        Path::Bin { .. } => PathKind::Filterbank,
                        Path::Direct => PathKind::Own,
                        Path::Outside => PathKind::None,
                    };
                }
            }
            None => {
                self.bins = None;
                for i in 0..self.channels.len() {
                    let min = self.min_rate(&self.channels[i].spec.stack);
                    let c = &mut self.channels[i];
                    c.rate_hz = channel_rate(c.spec.rate_hz, min, c.spec.bandwidth_hz, f64::MAX);
                    c.path = PathKind::None;
                }
            }
        }
        self.p.opt(&BINS, self.bins.map(count));
        self.p.set(&CHANNEL_COUNT, count(self.channels.len()));
        let keys: Vec<String> = self.channels.iter().map(|c| c.key.clone()).collect();
        for k in keys {
            self.publish_channel(&k);
        }
        self.layout_changed(None);
    }

    fn layout_changed(&mut self, radio: Option<String>) {
        let change = ChannelChange::Layout { radio };
        if self.changes.last() != Some(&change) {
            self.changes.push(change);
        }
    }

    fn publish_channel(&mut self, key: &str) {
        let Some(c) = self.channels.iter().find(|c| c.key == key) else {
            return;
        };
        let state = if self.input.is_some() && c.path == PathKind::None {
            ChannelState::Outside
        } else if c.session.is_none() {
            ChannelState::Starting
        } else {
            ChannelState::Running
        };
        let label = if c.spec.label.is_empty() {
            format!("{:.4} MHz", c.spec.freq_hz / 1e6)
        } else {
            c.spec.label.clone()
        };
        if let Some(mut r) = self.p.row_mut(&CHANNELS, key) {
            r.set(&LABEL, &label);
            r.set(&RADIO, &c.spec.radio);
            r.set(&keys::FREQ_HZ, c.spec.freq_hz);
            r.set(&BANDWIDTH, c.spec.bandwidth_hz);
            r.set(&RATE, c.rate_hz);
            r.set(&STACK, &c.spec.stack);
            r.set(&SUBMODE, &c.spec.submode);
            r.opt(&OF_SPAN, c.span.as_deref());
            r.set(&STATE, state);
            r.set(&PATH, c.path);
            r.opt(&SQUELCH, c.spec.squelch_db);
            r.opt(&SESSION, c.session.as_deref());
            if c.spec.squelch_db.is_none() {
                r.set(&OPEN, true);
            }
        }
    }

    fn publish_carriers(&mut self, si: usize) {
        let s = &self.spans[si];
        for c in &s.carriers {
            if let Some(mut r) = self.p.row_mut(&CARRIERS, &c.key) {
                r.set(&OF_SPAN, &s.key);
                r.set(&keys::FREQ_HZ, c.freq_hz);
                r.set(&keys::LEVEL_DBFS, f64::from(c.reading.level_dbfs).round());
                r.set(&keys::SNR_DB, f64::from(c.reading.snr_db).round());
                r.set(&ACTIVE, c.gate.is_open());
                r.opt(&OF_CHANNEL, c.channel.as_deref());
            }
        }
    }

    fn publish_span(&mut self, si: usize) {
        let s = &self.spans[si];
        if let Some(mut r) = self.p.row_mut(&SPANS, &s.key) {
            r.set(&LABEL, &s.spec.label);
            r.set(&RADIO, &s.spec.radio);
            r.set(&STACK, &s.spec.stack);
            r.set(&SUBMODE, &s.spec.submode);
            r.set(&BANDWIDTH, s.spec.bandwidth_hz);
            r.set(&RATE, s.spec.rate_hz);
            r.set(&SPAN_OPEN, s.spec.open);
            r.set(&THRESHOLD, s.spec.threshold_db);
            r.set(&IDLE, s.spec.idle_s);
            r.set(&MAX_RX, count(s.spec.max_receivers));
            r.set(&COUNT, count(s.carriers.len()));
        }
        self.publish_carriers(si);
    }

    fn add_span(&mut self, spec: SpanSpec) -> String {
        self.next += 1;
        let key = format!("s{}", self.next);
        let gate = Gate::new(
            spec.threshold_db as f32,
            self.values.get(&HYSTERESIS) as f32,
            0.0,
        );
        let carriers = spec
            .raster
            .freqs()
            .into_iter()
            .enumerate()
            .map(|(i, f)| Carrier {
                key: format!("{key}/{i}"),
                freq_hz: f,
                gate,
                reading: Reading::default(),
                channel: None,
                quiet_since: None,
            })
            .collect();
        let always = spec.open == SpanOpen::Always;
        self.spans.push(Span {
            key: key.clone(),
            spec,
            carriers,
        });
        let si = self.spans.len() - 1;
        if always {
            for i in 0..self.spans[si].carriers.len() {
                let c = self.span_channel(si, i);
                self.spans[si].carriers[i].channel = Some(c);
            }
        }
        self.publish_span(si);
        self.layout_changed(Some(self.spans[si].spec.radio.clone()));
        key
    }

    fn remove_span(&mut self, key: &str) -> bool {
        let Some(si) = self.spans.iter().position(|s| s.key == key) else {
            return false;
        };
        let chans: Vec<String> = self
            .channels
            .iter()
            .filter(|c| c.span.as_deref() == Some(key))
            .map(|c| c.key.clone())
            .collect();
        for c in chans {
            self.drop_channel(&c);
        }
        let s = self.spans.remove(si);
        for c in &s.carriers {
            self.p.remove_row(&CARRIERS, &c.key);
        }
        self.p.remove_row(&SPANS, key);
        self.layout_changed(None);
        true
    }

    /// The channels added by hand and the spans, as saved state.
    ///
    /// # Errors
    ///
    /// [`BankError::Encode`] if the state cannot be written.
    pub fn save_state(&self) -> Result<StateBlob, BankError> {
        let saved = Saved {
            next: self.next,
            channels: self
                .channels
                .iter()
                .filter(|c| c.span.is_none())
                .map(|c| c.spec.clone())
                .collect(),
            spans: self.spans.iter().map(|s| s.spec.clone()).collect(),
        };
        let bytes = serde_json::to_vec(&saved).map_err(|_| BankError::Encode)?;
        Ok(StateBlob::new(OWNER, STATE_VERSION, bytes))
    }

    /// Replaces the channels and spans with saved state. The engine gets a stop for each channel now and a start for each channel
    /// of the state.
    ///
    /// # Errors
    ///
    /// [`BankError::State`] if the state is of another owner or version, or damaged. The bank does not change then.
    pub fn restore_state(&mut self, state: &StateBlob) -> Result<(), BankError> {
        let bytes = state.open(OWNER, STATE_VERSION)?;
        let saved: Saved = serde_json::from_slice(bytes)
            .map_err(|_| BankError::State(sdrkit_core::StateError::Damaged))?;
        let spans: Vec<String> = self.spans.iter().map(|s| s.key.clone()).collect();
        for s in spans {
            self.remove_span(&s);
        }
        let chans: Vec<String> = self.channels.iter().map(|c| c.key.clone()).collect();
        for c in chans {
            self.drop_channel(&c);
        }
        for spec in saved.channels {
            self.add_channel(spec, None);
        }
        for spec in saved.spans {
            self.add_span(spec);
        }
        self.next = self.next.max(saved.next);
        Ok(())
    }

    fn no_row(&self, table: TableId, key: &str) -> CommandError {
        CommandError::NoSuchRow {
            table,
            key: key.to_owned(),
        }
    }

    fn stack_arg<'a>(&self, args: &'a Values) -> Result<&'a str, CommandError> {
        let stack = args.arg(&A_STACK)?;
        if (self.stacks)(stack).is_none() {
            return Err(refused(args, A_STACK.field.key, |id| {
                SettingError::NotPermitted { id }
            }));
        }
        Ok(stack)
    }

    fn run_command(
        &mut self,
        cmd: BankCommand,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        match cmd {
            BankCommand::Add => {
                let stack = self.stack_arg(args)?.to_owned();
                let spec = ChannelSpec {
                    radio: args.arg(&A_RADIO)?.to_owned(),
                    freq_hz: args.arg(&A_FREQ)?,
                    bandwidth_hz: args.get(&A_BANDWIDTH),
                    rate_hz: args.get(&A_RATE),
                    stack,
                    submode: args.get(&A_SUBMODE).to_owned(),
                    label: args.get(&A_LABEL).to_owned(),
                    squelch_db: args.get(&A_HAS_SQUELCH).then(|| args.get(&A_SQUELCH)),
                };
                let key = self.add_channel(spec, None);
                Ok(Some(reply(&key)))
            }
            BankCommand::Remove => {
                let row = args.arg(&A_ROW)?;
                if self.drop_channel(row) {
                    Ok(None)
                } else {
                    Err(self.no_row(self.ids.channels, row))
                }
            }
            BankCommand::Set => {
                let row = args.arg(&A_ROW)?;
                let freq = args.get(&A_SET_FREQ);
                let width = args.get(&A_SET_BANDWIDTH);
                let label = args.get(&A_SET_LABEL);
                let squelch = args.get(&A_SET_SQUELCH);
                let level = args.get(&A_SQUELCH);
                let hysteresis = self.values.get(&HYSTERESIS) as f32;
                let hang = self.values.get(&HANG);
                let Some(c) = self.channels.iter_mut().find(|c| c.key == row) else {
                    return Err(self.no_row(self.ids.channels, row));
                };
                let moved = freq > 0.0 || width > 0.0;
                if freq > 0.0 {
                    c.spec.freq_hz = freq;
                }
                if width > 0.0 {
                    c.spec.bandwidth_hz = width;
                }
                if !label.is_empty() {
                    c.spec.label = label.to_owned();
                }
                match squelch {
                    SquelchChange::Keep => {}
                    SquelchChange::Off => {
                        c.spec.squelch_db = None;
                        c.gate.force(true);
                    }
                    SquelchChange::On => {
                        c.spec.squelch_db = Some(level);
                        c.gate = Gate::new(level as f32, hysteresis, hang);
                    }
                }
                let key = c.key.clone();
                if moved {
                    self.replan();
                } else {
                    self.publish_channel(&key);
                }
                Ok(None)
            }
            BankCommand::SpanAdd => {
                let stack = self.stack_arg(args)?.to_owned();
                let text = args.arg(&A_RASTER)?;
                let freqs = parse_raster(text).map_err(|at| {
                    refused(args, A_RASTER.field.key, |id| SettingError::Malformed {
                        id,
                        at: Some(at),
                    })
                })?;
                let label = args.get(&A_LABEL);
                let spec = SpanSpec {
                    radio: args.arg(&A_RADIO)?.to_owned(),
                    label: if label.is_empty() {
                        "Span".into()
                    } else {
                        label.to_owned()
                    },
                    stack,
                    submode: args.get(&A_SUBMODE).to_owned(),
                    raster: Raster::List(freqs),
                    bandwidth_hz: args.get(&A_BANDWIDTH),
                    rate_hz: args.get(&A_RATE),
                    open: args.get(&A_OPEN),
                    threshold_db: args.get(&A_THRESHOLD),
                    idle_s: args.get(&A_IDLE),
                    max_receivers: usize::try_from(args.get(&A_MAX_RX)).unwrap_or(1),
                };
                let key = self.add_span(spec);
                Ok(Some(reply(&key)))
            }
            BankCommand::SpanRemove => {
                let row = args.arg(&A_ROW)?;
                if self.remove_span(row) {
                    Ok(None)
                } else {
                    Err(self.no_row(self.ids.spans, row))
                }
            }
        }
    }
}

impl Owner for BankService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match BankSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BankSetting::MaxBins => {
                self.values.set(id, value)?;
                self.replan();
                Ok(())
            }
            BankSetting::Hysteresis | BankSetting::Hang => self.values.set(id, value),
        }
    }
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
        BankCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = BankCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run_command(cmd, args)
    }
}
