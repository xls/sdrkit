//! The watch: the owner `service.watch`. Watches are a table that commands set. On each event the engine calls
//! [`Watch::check`], which returns a `watch.alert` event for each match and for each urgent event, within the limits.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::owner::{Owner, OwnerKey};
use sdrkit_core::publish::keys::{self, Urgency};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::storage::Storage;
use sdrkit_core::{
    Clock, Event, EventDef, EventKindId, Record, Results, ResultsDecl, Schema, StorageError,
    StorePath, Value, Values,
};
use sdrkit_svc_events::service::{failed, no_such_row, reply, table_id};
use sdrkit_svc_events::{choice_key, choice_of_key, Kinds, Observation};
use sdrkit_svc_outputs::{Backoff, SinkRunner, Transport};
use serde::{Deserialize, Serialize};

use crate::entry::{Entry, EntryText, WatchWhat};
use crate::push::{ChannelSink, PushChannel};
use crate::WatchError;

/// The owner key of the watch.
pub const KEY: OwnerKey = OwnerKey::of("service.watch");
const OWNER: &str = "service.watch";

/// The file in the host's storage that keeps the watches and the push channels.
pub const FILE: &str = "watch/watches.json";

/// The most alerts in any hour. Urgent alerts do not count. 0 for no limit.
pub const MAX_PER_HOUR: Setting<field::I64> =
    Setting::new("max_per_hour", "Most alerts per hour", "", 30).range(0.0, 1e6, 1.0);
/// No second alert of the same watch and the same thing within this time.
pub const COOLDOWN: Setting<field::Duration> =
    Setting::new("cooldown", "Cooldown", "s", 600.0).range(0.0, 1e7, 1.0);
/// Start of the quiet hours, `HH:MM` local time. Empty for none.
pub const QUIET_FROM: Setting<field::Text> = Setting::new("quiet_from", "Quiet from", "", "");
/// End of the quiet hours, `HH:MM` local time.
pub const QUIET_TO: Setting<field::Text> = Setting::new("quiet_to", "Quiet to", "", "");
/// Every urgent event produces an alert.
pub const BUILT_IN: Setting<field::Bool> = Setting::new("built_in", "Emergency alerts", "", true);

sdrkit_core::settings! {
    /// The settings of the watch.
    pub enum WatchSetting {
        /// [`MAX_PER_HOUR`].
        MaxPerHour = MAX_PER_HOUR,
        /// [`COOLDOWN`].
        Cooldown = COOLDOWN,
        /// [`QUIET_FROM`].
        QuietFrom = QUIET_FROM,
        /// [`QUIET_TO`].
        QuietTo = QUIET_TO,
        /// [`BUILT_IN`].
        BuiltIn = BUILT_IN,
    }
}

sdrkit_core::choice_enum! {
    /// What happened to a match.
    pub enum Outcome {
        /// The host shows it.
        Alerted = ("alerted", "Alerted"),
        /// In the quiet hours: kept, without a sound.
        Quiet = ("quiet", "Quiet"),
        /// The same watch and thing alerted not long ago.
        Cooldown = ("cooldown", "Cooldown"),
        /// The limit per hour was reached.
        HourlyLimit = ("hourly_limit", "Hourly limit"),
    }
}

sdrkit_core::choice_enum! {
    /// A push service.
    pub enum Channel {
        /// ntfy: `address` is the server, `topic` the topic.
        Ntfy = ("ntfy", "ntfy"),
        /// A Telegram bot: `address` is the API base, `topic` the chat id, `token` the bot token.
        Telegram = ("telegram", "Telegram"),
        /// A webhook: `address` is the URL.
        Webhook = ("webhook", "Webhook"),
    }
}

/// The row key that a command acts on.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Row", ""));
/// A label for people (`"Dad's ship"`).
pub const LABEL: Setting<field::Text> = Setting::new("label", "Label", "", "");
/// What the watch looks for.
pub const WHAT: Setting<field::Choice<WatchWhat>> =
    Setting::choice("what", "Looks for", WatchWhat::Source);
/// The families the watch reads, separated by commas. Empty reads every family.
pub const FAMILIES: Setting<field::Text> = Setting::new("families", "Families", "", "");
/// The event kinds the watch reads, separated by commas. Empty reads every kind.
pub const KINDS: Setting<field::Text> = Setting::new("kinds", "Kinds", "", "");
/// The field of a field watch.
pub const FIELD: Setting<field::Text> = Setting::new("field", "Field", "", "");
/// The value, word or identity to look for. A trailing `*` matches a prefix.
pub const VALUE: Setting<field::Text> = Setting::new("value", "Value", "", "");
/// The frequency of a frequency watch, or the lower edge of a range.
pub const HZ: Setting<field::Hz> = Setting::new("hz", "Frequency", "Hz", 0.0).range(0.0, 1e12, 1.0);
/// Plus or minus, around the frequency.
pub const TOLERANCE_HZ: Setting<field::Hz> =
    Setting::new("tolerance_hz", "Tolerance", "Hz", 0.0).range(0.0, 1e9, 1.0);
/// The upper edge of a range.
pub const TO_HZ: Setting<field::Hz> = Setting::new("to_hz", "To", "Hz", 0.0).range(0.0, 1e12, 1.0);
/// Matches are urgent: a sound even in the quiet hours, and no hourly limit.
pub const URGENT: Setting<field::Bool> = Setting::new("urgent", "Urgent", "", false);
/// Matches go to the push channels as well.
pub const PUSH: Setting<field::Bool> = Setting::new("push", "Push", "", true);
/// Off watches never match.
pub const ENABLED: Setting<field::Bool> = Setting::new("enabled", "On", "", true);
/// The push service.
pub const CHANNEL: Setting<field::Choice<Channel>> =
    Setting::choice("channel", "Service", Channel::Ntfy);
/// The address of the push service.
pub const ADDRESS: Setting<field::Text> = Setting::new("address", "Address", "", "");
/// The ntfy topic or the Telegram chat id.
pub const TOPIC: Setting<field::Text> = Setting::new("topic", "Topic", "", "");
/// The token. The tables never show it.
pub const TOKEN: Setting<field::Text> = Setting::new("token", "Token", "", "");

/// Adds a watch, or replaces the watch with this row key.
pub const ADD: Command = Command::new("add", "Add a watch")
    .help("Adds a watch or replaces it. Names in the watch must resolve.")
    .args(&[
        &ROW,
        &LABEL,
        &WHAT,
        &FAMILIES,
        &KINDS,
        &FIELD,
        &VALUE,
        &HZ,
        &TOLERANCE_HZ,
        &TO_HZ,
        &URGENT,
        &PUSH,
        &ENABLED,
    ]);
/// Removes a watch.
pub const REMOVE: Command = Command::new("remove", "Remove a watch").args(&[&ROW]);
/// Removes every watch.
pub const CLEAR: Command = Command::new("clear", "Remove every watch");
/// Adds a push channel, or replaces the channel with this row key.
pub const PUSH_ADD: Command = Command::new("push.add", "Add a push channel")
    .args(&[&ROW, &CHANNEL, &ADDRESS, &TOPIC, &TOKEN]);
/// Removes a push channel.
pub const PUSH_REMOVE: Command = Command::new("push.remove", "Remove a push channel").args(&[&ROW]);
/// The id of the alert that a command made.
pub const ALERT: Field<field::Text> = Field::new("alert", "Alert", "");
/// Sends a test alert to the host and every push channel, past every limit.
pub const TEST: Command = Command::new("alert.test", "Test alert")
    .help("Sends a test alert to the host and every push channel.")
    .reply(&[&ALERT]);

sdrkit_core::commands! {
    /// The commands of the watch.
    pub enum WatchCommand {
        /// [`ADD`].
        Add = ADD,
        /// [`REMOVE`].
        Remove = REMOVE,
        /// [`CLEAR`].
        Clear = CLEAR,
        /// [`PUSH_ADD`].
        PushAdd = PUSH_ADD,
        /// [`PUSH_REMOVE`].
        PushRemove = PUSH_REMOVE,
        /// [`TEST`].
        Test = TEST,
    }
}

/// The watches.
pub const WATCHES: TableDef = TableDef {
    key: "watches",
    label: "Watches",
    columns: &[
        &LABEL.field,
        &WHAT.field,
        &FAMILIES.field,
        &KINDS.field,
        &FIELD.field,
        &VALUE.field,
        &HZ.field,
        &TOLERANCE_HZ.field,
        &TO_HZ.field,
        &URGENT.field,
        &PUSH.field,
        &ENABLED.field,
    ],
    cap: 10_000,
    marks: sdrkit_core::RowMarks::NONE,
};

/// When.
pub const TIME: Field<field::Time> = Field::new("time", "Time", "");
/// The watch that matched. Empty for an emergency alert.
pub const ENTRY: Field<field::Text> = Field::new("entry", "Watch", "").refers_to(OWNER, "watches");
/// The title.
pub const TITLE: Field<field::Text> = Field::new("title", "Title", "");
/// The kind key of the event that matched.
pub const TRIGGER: Field<field::Text> = Field::new("trigger", "Trigger", "");
/// What happened to the match.
pub const OUTCOME: Field<field::Choice<Outcome>> = Field::new("outcome", "Outcome", "");
/// The alert went to the push channels.
pub const PUSHED: Field<field::Bool> = Field::new("pushed", "Pushed", "");

/// The last alerts and what happened to each. The row key is the alert id.
pub const ALERTS: TableDef = TableDef {
    key: "alerts",
    label: "Alerts",
    columns: &[
        &TIME,
        &ENTRY,
        &TITLE,
        &keys::TEXT,
        &keys::URGENCY,
        &TRIGGER,
        &keys::SOURCE,
        &keys::FREQ_HZ,
        &OUTCOME,
        &PUSHED,
    ],
    cap: 500,
    marks: sdrkit_core::RowMarks::NONE,
};

/// Alerts sent.
pub const SENT: Field<field::I64> = Field::new("sent", "Sent", "");
/// Alerts dropped.
pub const DROPPED: Field<field::I64> = Field::new("dropped", "Dropped", "");
/// The last failure, for people. It never holds a token.
pub const LAST_ERROR: Field<field::Text> = Field::new("last_error", "Last error", "");
/// The channel has a token.
pub const HAS_TOKEN: Field<field::Bool> = Field::new("has_token", "Token set", "");

/// The push channels.
pub const PUSHES: TableDef = TableDef {
    key: "pushes",
    label: "Push channels",
    columns: &[
        &CHANNEL.field,
        &ADDRESS.field,
        &TOPIC.field,
        &HAS_TOKEN,
        &SENT,
        &DROPPED,
        &LAST_ERROR,
    ],
    cap: 100,
    marks: sdrkit_core::RowMarks::NONE,
};

/// An alert: the event `watch.alert`.
pub static WATCH_ALERT: EventDef = EventDef::new(
    "watch.alert",
    "Alert",
    &[
        &ENTRY,
        &TITLE,
        &keys::TEXT,
        &keys::URGENCY,
        &TRIGGER,
        &keys::SOURCE,
        &OUTCOME,
        &PUSHED,
    ],
);

/// The event kinds of the watch, for the engine's catalog.
pub static WATCH_EVENTS: [&EventDef; 1] = [&WATCH_ALERT];

/// An alert as a push channel sends it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct Alert {
    pub id: u64,
    pub time: f64,
    pub entry: String,
    pub title: String,
    pub body: String,
    pub urgent: bool,
    pub kind: String,
    pub identity: Option<String>,
    pub freq_hz: f64,
}

impl Alert {
    /// The test alert at `now`.
    pub fn test(now: f64) -> Alert {
        Alert {
            id: 0,
            time: now,
            entry: "test".into(),
            title: "sdrkit test alert".into(),
            body: "If you can read this, alerts reach you.".into(),
            urgent: false,
            kind: WATCH_ALERT.kind.to_owned(),
            identity: None,
            freq_hz: 0.0,
        }
    }
}

/// A push channel as the user set it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct PushText {
    channel: String,
    address: String,
    topic: String,
    token: String,
}

impl PushText {
    fn channel(&self) -> PushChannel {
        match choice_of_key(&self.channel).unwrap_or(Channel::Ntfy) {
            Channel::Ntfy => PushChannel::Ntfy {
                server: self.address.clone(),
                topic: self.topic.clone(),
                token: self.token.clone(),
            },
            Channel::Telegram => PushChannel::Telegram {
                api: self.address.clone(),
                bot_token: self.token.clone(),
                chat_id: self.topic.clone(),
            },
            Channel::Webhook => PushChannel::Webhook {
                url: self.address.clone(),
                token: self.token.clone(),
            },
        }
    }
}

/// The rows as the user set them, as kept in [`FILE`].
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Saved {
    watches: BTreeMap<String, EntryText>,
    pushes: BTreeMap<String, PushText>,
}

/// One match before the limits.
struct Hit {
    entry: usize,
    urgent: bool,
    push: bool,
}

/// The watch: the owner `service.watch`.
pub struct Watch {
    kinds: Arc<Kinds>,
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
    clock: Clock,
    path: StorePath,
    values: Values,
    quiet: Option<(u16, u16)>,
    saved: Saved,
    entries: Vec<Entry>,
    pushes: Vec<(String, SinkRunner<Arc<Alert>>)>,
    last: HashMap<(String, String), f64>,
    hour: VecDeque<f64>,
    next_id: u64,
    own: Option<EventKindId>,
    what: String,
    hits: Vec<Hit>,
    pending: Vec<Event>,
    p: Published,
}

/// Minutes since midnight of `"HH:MM"`.
fn hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl Watch {
    /// Opens the watch with the watches and push channels kept at [`FILE`] in `storage`. Push channels connect through
    /// `transport`. `clock` returns the time of a test alert.
    ///
    /// # Errors
    ///
    /// [`WatchError::Storage`] if the file cannot be read. [`WatchError::Json`] if it does not parse.
    pub fn open(
        kinds: Arc<Kinds>,
        storage: Arc<dyn Storage>,
        transport: Arc<dyn Transport>,
        clock: Clock,
    ) -> Result<Self, WatchError> {
        let path = StorePath::new(FILE).map_err(|e| WatchError::Storage {
            path: StorePath::root(),
            source: StorageError::InvalidPath(e),
        })?;
        let saved: Saved = match storage.read(&path) {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(StorageError::NotFound) => Saved::default(),
            Err(source) => return Err(WatchError::Storage { path, source }),
        };
        let own = kinds.catalog().id_of(&WATCH_ALERT);
        let mut w = Watch {
            kinds,
            storage,
            transport,
            clock,
            path,
            values: Values::defaults(&WatchSetting::schema()),
            quiet: None,
            saved,
            entries: Vec::new(),
            pushes: Vec::new(),
            last: HashMap::new(),
            hour: VecDeque::new(),
            next_id: 1,
            own,
            what: String::new(),
            hits: Vec::new(),
            pending: Vec::new(),
            p: Published::from_decl(Arc::new(ResultsDecl::new().event(&WATCH_ALERT)))
                .with_tables(&[&WATCHES, &ALERTS, &PUSHES]),
        };
        w.rebuild()?;
        Ok(w)
    }

    fn save(&self) -> Result<(), WatchError> {
        let data = serde_json::to_vec_pretty(&self.saved)?;
        self.storage
            .write(&self.path, &data)
            .map_err(|source| WatchError::Storage {
                path: self.path.clone(),
                source,
            })
    }

    fn rebuild(&mut self) -> Result<(), WatchError> {
        self.entries = self
            .saved
            .watches
            .iter()
            .filter_map(|(k, t)| {
                let what = choice_of_key(&t.what).unwrap_or(WatchWhat::Source);
                Entry::resolve(k.clone(), t.clone(), what, &self.kinds).ok()
            })
            .collect();
        let keep: Vec<String> = self.saved.pushes.keys().cloned().collect();
        self.pushes.retain(|(k, _)| keep.contains(k));
        for (k, t) in &self.saved.pushes {
            if self.pushes.iter().any(|(x, _)| x == k) {
                continue;
            }
            let sink = ChannelSink {
                channel: t.channel(),
                transport: self.transport.clone(),
            };
            let r = SinkRunner::spawn(
                &format!("push-{k}"),
                Box::new(sink),
                100,
                Backoff::new(Duration::from_secs(2), Duration::from_secs(120)),
            )?;
            self.pushes.push((k.clone(), r));
        }
        self.show();
        Ok(())
    }

    fn show(&mut self) {
        for t in [&WATCHES, &PUSHES] {
            let id = table_id(&self.p, t);
            let gone: Vec<String> = self
                .p
                .results()
                .table(id)
                .map(|tb| {
                    tb.rows_newest_first()
                        .map(|(k, _)| k.to_owned())
                        .filter(|k| match t.key {
                            "watches" => !self.saved.watches.contains_key(k),
                            _ => !self.saved.pushes.contains_key(k),
                        })
                        .collect()
                })
                .unwrap_or_default();
            for k in gone {
                self.p.remove_row(t, &k);
            }
        }
        for (k, t) in &self.saved.watches {
            let Some(mut r) = self.p.row_mut(&WATCHES, k) else {
                continue;
            };
            r.set(&LABEL.field, &t.label);
            r.set(
                &WHAT.field,
                choice_of_key(&t.what).unwrap_or(WatchWhat::Source),
            );
            r.set(&FAMILIES.field, &t.families);
            r.set(&KINDS.field, &t.kinds);
            r.set(&FIELD.field, &t.field);
            r.set(&VALUE.field, &t.value);
            r.set(&HZ.field, t.hz);
            r.set(&TOLERANCE_HZ.field, t.tolerance_hz);
            r.set(&TO_HZ.field, t.to_hz);
            r.set(&URGENT.field, t.urgent);
            r.set(&PUSH.field, t.push);
            r.set(&ENABLED.field, t.enabled);
        }
        self.tick();
    }

    /// Brings the counters of the `pushes` table up to date. The engine calls it about once a second.
    pub fn tick(&mut self) {
        let n = |x: u64| i64::try_from(x).unwrap_or(i64::MAX);
        for (k, runner) in &self.pushes {
            let Some(t) = self.saved.pushes.get(k) else {
                continue;
            };
            let st = runner.stats();
            let Some(mut r) = self.p.row_mut(&PUSHES, k) else {
                continue;
            };
            r.set(
                &CHANNEL.field,
                choice_of_key(&t.channel).unwrap_or(Channel::Ntfy),
            );
            r.set(&ADDRESS.field, &t.address);
            r.set(&TOPIC.field, &t.topic);
            r.set(&HAS_TOKEN, !t.token.is_empty());
            r.set(&SENT, n(st.sent));
            r.set(&DROPPED, n(st.dropped));
            r.opt(&LAST_ERROR, st.last_error.as_deref());
        }
    }

    fn quiet_now(&self, unix_s: f64, offset_min: i32) -> bool {
        let Some((from, to)) = self.quiet else {
            return false;
        };
        let Some(c) = sdrkit_util_geo::time::split_f64(unix_s)
            .ok()
            .and_then(|(s, _)| {
                sdrkit_util_geo::time::Civil::from_unix_local(s, 0, offset_min).ok()
            })
        else {
            return false;
        };
        let now = u16::from(c.hour) * 60 + u16::from(c.minute);
        match from.cmp(&to) {
            std::cmp::Ordering::Equal => false,
            std::cmp::Ordering::Less => now >= from && now < to,
            std::cmp::Ordering::Greater => now >= from || now < to,
        }
    }

    /// Takes an observation: each watch that matches returns a `watch.alert` event, and so does an urgent observation if
    /// emergency alerts are on. `offset_min` is the host's offset from UTC, minutes, for the quiet hours. The events go to the
    /// end of `out`, after the alert of a test command that came before. An alert that the limits let through also goes to the
    /// push channels.
    ///
    /// If no watch matches and the observation is not urgent, the call allocates nothing.
    pub fn check(&mut self, o: &Observation<'_>, offset_min: i32, out: &mut Vec<Event>) {
        out.append(&mut self.pending);
        if Some(o.kind) == self.own {
            return;
        }
        self.hits.clear();
        for (i, e) in self.entries.iter().enumerate() {
            if e.matches(o, &mut self.what) {
                self.hits.push(Hit {
                    entry: i,
                    urgent: e.text.urgent,
                    push: e.text.push,
                });
            }
        }
        if self.values.get(&BUILT_IN) && o.is_urgent() {
            self.hits.push(Hit {
                entry: usize::MAX,
                urgent: true,
                push: true,
            });
        }
        if self.hits.is_empty() {
            return;
        }
        let hits = std::mem::take(&mut self.hits);
        for h in &hits {
            self.alert(o, h, offset_min, out);
        }
        self.hits = hits;
        if self.last.len() > 50_000 {
            let (now, cd) = (o.unix_time, self.values.get(&COOLDOWN));
            self.last.retain(|_, t| now - *t < cd);
        }
    }

    fn alert(&mut self, o: &Observation<'_>, h: &Hit, offset_min: i32, out: &mut Vec<Event>) {
        let now = o.unix_time;
        let (entry, title, what) = match self.entries.get(h.entry) {
            Some(e) => {
                let mut what = String::new();
                e.matches(o, &mut what);
                let label = if e.text.label.is_empty() {
                    what.clone()
                } else {
                    e.text.label.clone()
                };
                (e.key.clone(), format!("Watch: {label}"), what)
            }
            None => {
                let reason = o.text.unwrap_or(choice_key(o.urgency)).to_owned();
                (String::new(), format!("Emergency: {reason}"), reason)
            }
        };
        while self.hour.front().is_some_and(|t| now - t >= 3600.0) {
            self.hour.pop_front();
        }
        let station = o
            .identity
            .map_or_else(|| format!("{:.0}", o.event.freq_hz), str::to_owned);
        let key = (entry.clone(), station);
        let max = self.values.get(&MAX_PER_HOUR);
        let outcome = if self
            .last
            .get(&key)
            .is_some_and(|t| now - t < self.values.get(&COOLDOWN))
        {
            Outcome::Cooldown
        } else if !h.urgent && max > 0 && self.hour.len() as i64 >= max {
            Outcome::HourlyLimit
        } else if !h.urgent && self.quiet_now(now, offset_min) {
            Outcome::Quiet
        } else {
            Outcome::Alerted
        };
        if matches!(outcome, Outcome::Alerted | Outcome::Quiet) {
            self.last.insert(key, now);
            self.hour.push_back(now);
        }
        let mut body = o.identity.map(|i| format!("{i}: ")).unwrap_or_default();
        body.push_str(&what);
        if o.event.freq_hz > 0.0 {
            let _ = write!(body, " on {:.4} MHz", o.event.freq_hz / 1e6);
        }
        if let Some(d) = o.distance_km {
            let _ = write!(body, ", {d:.1} km away");
        }
        let pushed = h.push && outcome == Outcome::Alerted;
        let a = Alert {
            id: self.next_id,
            time: now,
            entry,
            title,
            body,
            urgent: h.urgent,
            kind: o.event.kind().to_owned(),
            identity: o.identity.map(str::to_owned),
            freq_hz: o.event.freq_hz,
        };
        self.next_id += 1;
        let urgency = if h.urgent && o.urgency == Urgency::None {
            Urgency::Urgent
        } else {
            o.urgency
        };
        out.push(self.record(&a, urgency, outcome, pushed));
        if pushed {
            self.push(a);
        }
    }

    fn record(&mut self, a: &Alert, urgency: Urgency, outcome: Outcome, pushed: bool) -> Event {
        if let Some(mut r) = self.p.row_mut(&ALERTS, &a.id.to_string()) {
            r.set(&TIME, a.time);
            r.set(&ENTRY, &a.entry);
            r.set(&TITLE, &a.title);
            r.set(&keys::TEXT, &a.body);
            r.set(&keys::URGENCY, urgency);
            r.set(&TRIGGER, &a.kind);
            r.opt(&keys::SOURCE, a.identity.as_deref());
            r.set(&keys::FREQ_HZ, a.freq_hz);
            r.set(&OUTCOME, outcome);
            r.set(&PUSHED, pushed);
        }
        self.own
            .and_then(|id| self.kinds.catalog().event(id, a.time, a.freq_hz))
            .unwrap_or_else(|| WATCH_ALERT.event(a.time, a.freq_hz))
            .set(&ENTRY, &a.entry)
            .set(&TITLE, &a.title)
            .set(&keys::TEXT, &a.body)
            .set(&keys::URGENCY, urgency)
            .set(&TRIGGER, &a.kind)
            .opt(&keys::SOURCE, a.identity.as_deref())
            .set(&OUTCOME, outcome)
            .set(&PUSHED, pushed)
    }

    fn push(&self, a: Alert) {
        if self.pushes.is_empty() {
            return;
        }
        let shared = Arc::new(a);
        for (_, r) in &self.pushes {
            r.submit(shared.clone());
        }
    }

    fn entry_text(args: &Values) -> EntryText {
        let t = |s: &Setting<field::Text>| args.get(s).trim().to_owned();
        EntryText {
            label: t(&LABEL),
            what: choice_key(args.get(&WHAT)).to_owned(),
            families: t(&FAMILIES),
            kinds: t(&KINDS),
            field: t(&FIELD),
            value: t(&VALUE),
            hz: args.get(&HZ),
            tolerance_hz: args.get(&TOLERANCE_HZ),
            to_hz: args.get(&TO_HZ),
            urgent: args.get(&URGENT),
            push: args.get(&PUSH),
            enabled: args.get(&ENABLED),
        }
    }
}

impl Owner for Watch {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = WatchSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        if let (WatchSetting::QuietFrom | WatchSetting::QuietTo, Value::Text(t)) = (s, value) {
            if !t.trim().is_empty() && hhmm(t).is_none() {
                return Err(SettingError::Malformed { id, at: Some(0) });
            }
        }
        self.values.set(s.id(), value)?;
        self.quiet = hhmm(self.values.get(&QUIET_FROM)).zip(hhmm(self.values.get(&QUIET_TO)));
        Ok(())
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
        WatchCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match WatchCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            WatchCommand::Add => {
                let key = args.arg(&ROW)?.trim().to_owned();
                let text = Self::entry_text(args);
                let what = args.get(&WHAT);
                Entry::resolve(key.clone(), text.clone(), what, &self.kinds).map_err(failed)?;
                self.saved.watches.insert(key, text);
            }
            WatchCommand::Remove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.watches.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &WATCHES, key));
                }
            }
            WatchCommand::Clear => self.saved.watches.clear(),
            WatchCommand::PushAdd => {
                let key = args.arg(&ROW)?.trim().to_owned();
                let text = PushText {
                    channel: choice_key(args.get(&CHANNEL)).to_owned(),
                    address: args.get(&ADDRESS).trim().to_owned(),
                    topic: args.get(&TOPIC).trim().to_owned(),
                    token: args.get(&TOKEN).to_owned(),
                };
                sdrkit_svc_outputs::Url::parse(&text.address)
                    .map_err(|e| failed(WatchError::Output(e)))?;
                self.pushes.retain(|(k, _)| *k != key);
                self.saved.pushes.insert(key, text);
            }
            WatchCommand::PushRemove => {
                let key = args.arg(&ROW)?.trim();
                if self.saved.pushes.remove(key).is_none() {
                    return Err(no_such_row(&self.p, &PUSHES, key));
                }
            }
            WatchCommand::Test => {
                let mut a = Alert::test(self.clock.now());
                a.id = self.next_id;
                self.next_id += 1;
                let ev = self.record(&a, Urgency::None, Outcome::Alerted, true);
                self.pending.push(ev);
                let row = a.id.to_string();
                self.push(a);
                let mut r = reply(WatchCommand::commands(), id)?;
                r.set(&ALERT, &row);
                return Ok(Some(r));
            }
        }
        self.rebuild().map_err(failed)?;
        self.save().map_err(failed)?;
        Ok(None)
    }
}
