//! The QO-100 display: the beacon lock and LNB error, the spectra, the signal tracker with its classes, the conversations and the
//! DATV carriers.

use super::rx::{BeaconLock, Datv, LockPhase, QoItem, Signal, SignalClass, BEACONS};
use super::{BEACON_PSK, NB_LO};
use crate::display::{set_series, Clock};
use crate::typed::{
    Qo100BeaconRow, Qo100CallsignRow, Qo100ConversationEvent, Qo100ConversationRow,
    Qo100DatvCarrierRow, Qo100Fields, Qo100ListenArgs, Qo100LockEvent, Qo100SignalRow,
    Qo100SpeechArgs, Qo100TrackRow, QO100_BEACONS, QO100_CALLSIGNS, QO100_CONVERSATION,
    QO100_CONVERSATIONS, QO100_CONVERSATION_LISTEN, QO100_CONVERSATION_ROW, QO100_DATV,
    QO100_LNB_ERROR, QO100_LOCK, QO100_NARROWBAND, QO100_REMOVE, QO100_SIGNALS, QO100_SPEECH_TEXT,
    QO100_TRACKS, QO100_WIDEBAND,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Command, ConfigError, Event, LockState, Record, ResultsDecl, Schema, Setting,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Kind, Outbox, Port, RateFrom, RunContext, Timed};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The most conversations kept.
pub const CONVERSATIONS_CAP: usize = 60;
/// The most tracks followed.
pub const TRACKS_CAP: usize = 256;
/// A track not seen for this long is dropped, seconds.
const TRACK_HOLD_S: f64 = 120.0;
/// The share of time a track was on is measured over this window, seconds.
const DUTY_WINDOW_S: f64 = 30.0;
/// A conversation is active while somebody spoke this recently, seconds.
const ACTIVE_S: f64 = 5.0;
/// The most points of the LNB error history.
const HISTORY_CAP: usize = 600;

/// What the display publishes.
pub fn results_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        let mut d = ResultsDecl::new()
            .with_fields(Qo100Fields::SHAPE.fields)
            .with_table(&QO100_BEACONS)
            .with_table(&QO100_SIGNALS)
            .with_table(&QO100_TRACKS)
            .with_table(&QO100_CONVERSATIONS)
            .with_table(&QO100_CALLSIGNS)
            .with_table(&QO100_DATV);
        for s in [QO100_NARROWBAND, QO100_WIDEBAND, QO100_LNB_ERROR] {
            d = d.series(s.key, s.label, s.x_unit, s.y_unit);
        }
        Arc::new(d)
    })
    .clone()
}

/// A followed signal.
#[derive(Clone, Debug, Default)]
pub struct Track {
    /// Its RF, Hz.
    pub rf_hz: f64,
    /// Its lower edge, Hz.
    pub lo_edge_hz: f64,
    /// Its upper edge, Hz.
    pub hi_edge_hz: f64,
    /// Its width, Hz.
    pub width_hz: f64,
    /// Its level, dBFS.
    pub level_dbfs: f64,
    /// Its SNR, dB.
    pub snr_db: f64,
    /// It is on in the newest spectrum.
    pub active: bool,
    /// The share of the last 30 s it was on, 0 to 1.
    pub duty: f64,
    /// Its class.
    pub class: SignalClass,
    /// Its conversation (0: none).
    pub conversation: u64,
    /// (signal time, on, level) of each spectrum of the last 30 s.
    hist: VecDeque<(f64, bool, f64)>,
}

impl Shaped for Track {
    type Row = Qo100TrackRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Qo100TrackRow {
            last_heard: columns::last_heard(seen).map(Time),
            rf_hz: Hertz(self.rf_hz),
            width_hz: Hertz(self.width_hz),
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            active: self.active,
            duty_pct: self.duty * 100.0,
            class: self.class,
            conversation: (self.conversation > 0).then_some(self.conversation as i64),
        });
    }
}

/// A conversation: SSB tracks grouped by frequency and time.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    /// Somebody spoke recently.
    pub active: bool,
    /// The USB dial of the conversation: its lower edge less 300 Hz (corrected RF).
    pub dial_hz: f64,
    /// Its lower edge, Hz.
    pub lower_edge_hz: f64,
    /// Its level, dBFS.
    pub level_dbfs: f64,
    /// Its peak level, dBFS.
    pub peak_dbfs: f64,
    /// Seconds somebody spoke.
    pub talk_s: f64,
    /// The tracks heard in it.
    pub tracks: Vec<u64>,
    /// The callsigns read from the host's speech-to-text.
    pub callsigns: Vec<String>,
}

impl Shift for Conversation {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Conversation {
    type Row = Qo100ConversationRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Qo100ConversationRow {
            last_heard: columns::last_heard(seen).map(Time),
            active: self.active,
            dial_hz: Hertz(self.dial_hz),
            lower_edge_hz: Hertz(self.lower_edge_hz),
            level_dbfs: self.level_dbfs,
            peak_dbfs: self.peak_dbfs,
            stations: self.tracks.len() as i64,
            talk: Seconds(self.talk_s),
            length: Seconds(seen.map_or(0.0, |s| s.span())),
        });
    }
}

/// What the display keeps across sessions: the conversations.
#[derive(Debug, Serialize, Deserialize)]
pub struct QoTables {
    /// The conversations.
    pub conversations: ObjectTable<u64, Conversation>,
    /// The number of the next conversation.
    pub next_conversation: u64,
}

impl Default for QoTables {
    fn default() -> Self {
        QoTables {
            conversations: ObjectTable::new(CONVERSATIONS_CAP),
            next_conversation: 1,
        }
    }
}

impl Shift for QoTables {
    fn shift_times(&mut self, by_s: f64) {
        self.conversations.shift_times(by_s);
    }
}

impl Tables for QoTables {
    const FORMAT: StateFormat = StateFormat::new("satcom.qo100", 1);
    fn clear(&mut self) {
        self.conversations.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.conversations.adopt(saved.conversations);
        self.next_conversation = saved.next_conversation.max(self.next_conversation);
    }
}

/// The class of a tracked signal from its width and how steady it is.
pub fn classify(width: f64, duty: f64, level_sd: f64, beacon: bool) -> SignalClass {
    if beacon {
        SignalClass::Beacon
    } else if width < 120.0 && level_sd < 1.0 {
        SignalClass::Carrier
    } else if width < 120.0 {
        SignalClass::Cw
    } else if width < 400.0 {
        SignalClass::Psk
    } else if (1500.0..3500.0).contains(&width) && level_sd >= 1.0 {
        SignalClass::Ssb
    } else if width < 3500.0 && duty > 0.9 {
        SignalClass::Sstv
    } else if width >= 3500.0 {
        SignalClass::WideDigital
    } else {
        SignalClass::Unknown
    }
}

/// The NATO phonetic words and the spoken digits, each with the character it stands for.
const SPOKEN: &[(&str, char)] = &[
    ("alpha", 'A'),
    ("alfa", 'A'),
    ("bravo", 'B'),
    ("charlie", 'C'),
    ("delta", 'D'),
    ("echo", 'E'),
    ("foxtrot", 'F'),
    ("golf", 'G'),
    ("hotel", 'H'),
    ("india", 'I'),
    ("juliet", 'J'),
    ("juliett", 'J'),
    ("kilo", 'K'),
    ("lima", 'L'),
    ("mike", 'M'),
    ("november", 'N'),
    ("oscar", 'O'),
    ("papa", 'P'),
    ("quebec", 'Q'),
    ("romeo", 'R'),
    ("sierra", 'S'),
    ("tango", 'T'),
    ("uniform", 'U'),
    ("victor", 'V'),
    ("whiskey", 'W'),
    ("x-ray", 'X'),
    ("xray", 'X'),
    ("yankee", 'Y'),
    ("zulu", 'Z'),
    ("zero", '0'),
    ("one", '1'),
    ("two", '2'),
    ("three", '3'),
    ("four", '4'),
    ("five", '5'),
    ("six", '6'),
    ("seven", '7'),
    ("eight", '8'),
    ("nine", '9'),
    ("niner", '9'),
];

/// The callsigns in a spoken text: runs of NATO phonetic words and digit words become one word first, then
/// `sdrkit_util_text::callsign` reads each word.
///
/// ```
/// use sdrkit_stack_satcom::modes::qo100::display::callsigns;
/// assert_eq!(callsigns("lima yankee two alpha bravo charlie here"), vec!["LY2ABC"]);
/// ```
pub fn callsigns(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut run = String::new();
    for w in text.split_whitespace() {
        let lw = w.to_lowercase();
        let lw = lw.trim_matches(|c: char| !c.is_alphanumeric() && c != '-');
        if let Some((_, c)) = SPOKEN.iter().find(|(n, _)| *n == lw) {
            run.push(*c);
        } else {
            if !run.is_empty() {
                words.push(std::mem::take(&mut run));
            }
            words.push(w.to_uppercase());
        }
    }
    if !run.is_empty() {
        words.push(run);
    }
    let mut out: Vec<String> = Vec::new();
    for w in words {
        let c: String = w
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '/')
            .collect();
        if sdrkit_util_text::callsign::is_callsign(&c) && !out.contains(&c) {
            out.push(c);
        }
    }
    out
}

/// The QO-100 display.
pub struct QoDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tables: QoTables,
    /// The conversation tables changed since the last report.
    dirty: bool,
    tracks: ObjectTable<u64, Track>,
    next_track: u64,
    lock: BeaconLock,
    was_locked: bool,
    narrow: Option<(Arc<[f32]>, f64, f64, f64)>,
    signals: Vec<Signal>,
    wide: Option<(Arc<[f32]>, f64, f64)>,
    wide_outside: bool,
    datv: Vec<Datv>,
    history: VecDeque<(f64, f64)>,
    last_measurements: u64,
    gap_s: f64,
    dial_hz: f64,
    /// The dial as the setting holds it: the sender transmits there, whatever conversation the dial follows.
    set_dial_hz: f64,
    auto_listen: AutoListen,
    listening: Option<u64>,
    snippet: String,
}

sdrkit_core::choice_enum! {
    /// Which conversation the dial follows by itself.
    pub enum AutoListen {
        /// None: the dial or the conversation chosen.
        Off = ("off", "Off"),
        /// The loudest active conversation.
        Loudest = ("loudest", "The loudest"),
        /// The newest active conversation.
        Newest = ("newest", "The newest"),
    }
}

/// The USB dial for listening (RF).
pub const DIAL: Setting<field::Hz> = Setting::new("dial_hz", "Dial (RF)", "Hz", 10489.6e6)
    .range(10489.4e6, 10500.0e6, 1.0)
    .help("The USB dial for listening.");
/// Which conversation the dial follows by itself.
pub const AUTO_LISTEN: Setting<field::Choice<AutoListen>> =
    Setting::choice("auto_listen", "Auto-listen", AutoListen::Off);
/// The silence that ends a conversation.
pub const CONVERSATION_GAP: Setting<field::Duration> =
    Setting::new("conversation_gap_s", "Conversation gap", "s", 90.0).range(5.0, 3600.0, 1.0);
sdrkit_core::commands! {
    /// The commands of [`QoDisplay`].
    pub enum QoDisplayCommand {
        /// Speech-to-text.
        SpeechText = QO100_SPEECH_TEXT.command,
        /// Listen to a conversation.
        ConversationListen = QO100_CONVERSATION_LISTEN.command,
        /// Forgets one conversation.
        Remove = QO100_REMOVE.command,
    }
}

sdrkit_core::settings! {
    /// The settings of [`QoDisplay`].
    pub enum QoDisplaySetting {
        /// Dial.
        Dial = DIAL,
        /// Auto-listen.
        AutoListen = AUTO_LISTEN,
        /// Conversation gap.
        ConversationGap = CONVERSATION_GAP,
    }
}

/// The width above the dial that holds the signal of a station on the dial, Hz.
pub const OWN_WIDTH_HZ: f64 = 2700.0;

/// The level of the strongest signal with its top in the [`OWN_WIDTH_HZ`] above `dial_hz` less the level of the PSK beacon,
/// dB, both as the peak of a spectrum bin. `None` without such a signal or without the beacon.
pub fn own_to_beacon_db(signals: &[Signal], dial_hz: f64) -> Option<f64> {
    let beacon = signals.iter().find(|s| s.beacon == "PSK")?;
    let own = signals
        .iter()
        .filter(|s| {
            s.beacon.is_empty() && s.rf_hz >= dial_hz && s.rf_hz <= dial_hz + OWN_WIDTH_HZ
        })
        .map(|s| s.level_dbfs)
        .fold(None, |a: Option<f64>, l| Some(a.map_or(l, |a| a.max(l))))?;
    Some(own - beacon.level_dbfs)
}

impl Default for QoDisplay {
    fn default() -> Self {
        QoDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tables: QoTables::default(),
            dirty: true,
            tracks: ObjectTable::new(TRACKS_CAP),
            next_track: 1,
            lock: BeaconLock::default(),
            was_locked: false,
            narrow: None,
            signals: Vec::new(),
            wide: None,
            wide_outside: true,
            datv: Vec::new(),
            history: VecDeque::new(),
            last_measurements: 0,
            gap_s: 90.0,
            dial_hz: 10489.6e6,
            set_dial_hz: 10489.6e6,
            auto_listen: AutoListen::Off,
            listening: None,
            snippet: String::new(),
        }
    }
}

impl QoDisplay {
    /// The tracker: every signal of the averaged spectrum followed across spectra, and the SSB conversations.
    fn follow(&mut self, t: f64, signals: &[Signal], events: &mut Vec<Event>) {
        for (_, tr) in self.tracks.records_mut() {
            tr.active = false;
        }
        self.signals.clear();
        self.signals.extend_from_slice(signals);
        for s in self.signals.iter_mut() {
            let near = self
                .tracks
                .iter()
                .find(|r| (r.record.rf_hz - s.rf_hz).abs() < (s.width_hz / 2.0).max(300.0))
                .map(|r| *r.key);
            let id = near.unwrap_or_else(|| {
                self.next_track += 1;
                self.next_track - 1
            });
            let Ok(up) = self.tracks.upsert(&id, self.clock.stamp(t), Track::default) else {
                continue;
            };
            let tr = up.row;
            tr.width_hz = tr.width_hz.max(s.width_hz) * 0.9 + s.width_hz * 0.1;
            tr.rf_hz = s.rf_hz;
            tr.lo_edge_hz = s.rf_hz - s.width_hz / 2.0;
            tr.hi_edge_hz = s.rf_hz + s.width_hz / 2.0;
            tr.level_dbfs = s.level_dbfs;
            tr.snr_db = s.snr_db;
            tr.active = true;
            tr.hist.push_back((t, true, s.level_dbfs));
            s.track = id;
        }
        let signals = &self.signals;
        for (id, tr) in self.tracks.records_mut() {
            if !tr.active {
                tr.hist.push_back((t, false, 0.0));
            }
            while tr.hist.front().is_some_and(|h| t - h.0 > DUTY_WINDOW_S) {
                tr.hist.pop_front();
            }
            let on = tr.hist.iter().filter(|h| h.1).count().max(1) as f64;
            tr.duty = tr.hist.iter().filter(|h| h.1).count() as f64 / tr.hist.len().max(1) as f64;
            let mean = tr.hist.iter().filter(|h| h.1).map(|h| h.2).sum::<f64>() / on;
            let var = tr
                .hist
                .iter()
                .filter(|h| h.1)
                .map(|h| (h.2 - mean).powi(2))
                .sum::<f64>()
                / on;
            let beacon = signals
                .iter()
                .any(|s| s.track == *id && !s.beacon.is_empty())
                || BEACONS.iter().any(|b| (tr.rf_hz - b.1).abs() < 1.5e3);
            tr.class = classify(tr.width_hz, tr.duty, var.sqrt(), beacon);
        }
        self.tracks.expire(t, TRACK_HOLD_S);
        for s in self.signals.iter_mut() {
            if let Some(tr) = self.tracks.get(&s.track) {
                s.class = tr.class;
            }
        }
        self.converse(t, events);
    }

    /// Groups the active SSB tracks into conversations (within 3 kHz, until a silence of the gap).
    fn converse(&mut self, t: f64, events: &mut Vec<Event>) {
        let ssb: Vec<(u64, f64, f64)> = self
            .tracks
            .iter()
            .filter(|r| r.record.class == SignalClass::Ssb && r.record.active)
            .map(|r| (*r.key, r.record.lo_edge_hz, r.record.level_dbfs))
            .collect();
        for (tid, edge, level) in ssb {
            let gap = self.gap_s;
            let found = self
                .tables
                .conversations
                .iter()
                .find(|r| (r.record.lower_edge_hz - edge).abs() < 3e3 && r.seen.age(t) < gap)
                .map(|r| *r.key);
            let cid = match found {
                Some(c) => c,
                None => {
                    let c = self.tables.next_conversation;
                    self.tables.next_conversation += 1;
                    let mut channel = String::new();
                    let _ = write!(channel, "{c}");
                    events.push(QO100_CONVERSATION.event(
                        t,
                        edge - 300.0,
                        &Qo100ConversationEvent {
                            text: "conversation started",
                            family: Family::Voice,
                            channel: &channel,
                            conversation: c as i64,
                            dial_hz: Hertz(edge - 300.0),
                        },
                    ));
                    c
                }
            };
            let Ok(up) = self
                .tables
                .conversations
                .upsert(&cid, self.clock.stamp(t), || Conversation {
                    peak_dbfs: f64::NEG_INFINITY,
                    ..Default::default()
                })
            else {
                continue;
            };
            let cv = up.row;
            self.dirty = true;
            cv.active = true;
            cv.lower_edge_hz = edge;
            cv.dial_hz = edge - 300.0;
            cv.level_dbfs = level;
            cv.peak_dbfs = cv.peak_dbfs.max(level);
            cv.talk_s += 0.1;
            if !cv.tracks.contains(&tid) {
                cv.tracks.push(tid);
            }
            if let Some(tr) = self.tracks.get_mut(&tid) {
                tr.conversation = cid;
            }
        }
        let snippet = std::mem::take(&mut self.snippet);
        let mut first_active = true;
        let ages: Vec<(u64, bool)> = self
            .tables
            .conversations
            .iter()
            .map(|r| (*r.key, r.seen.age(t) < ACTIVE_S))
            .collect();
        for (id, active) in ages {
            if let Some(cv) = self.tables.conversations.get_mut(&id) {
                cv.active = active;
                if active && first_active && !snippet.is_empty() {
                    first_active = false;
                    for c in callsigns(&snippet) {
                        if !cv.callsigns.contains(&c) {
                            cv.callsigns.push(c);
                        }
                    }
                }
            }
        }
    }

    /// The conversation the dial follows now, from auto-listen or the user's choice.
    fn follow_dial(&mut self) {
        let active = || self.tables.conversations.iter().filter(|r| r.record.active);
        let pick = match self.auto_listen {
            AutoListen::Loudest => active()
                .max_by(|a, b| a.record.level_dbfs.total_cmp(&b.record.level_dbfs))
                .map(|r| r.record.dial_hz),
            AutoListen::Newest => active()
                .max_by(|a, b| a.seen.first_s.total_cmp(&b.seen.first_s))
                .map(|r| r.record.dial_hz),
            AutoListen::Off => self
                .listening
                .and_then(|id| self.tables.conversations.get(&id))
                .map(|c| c.dial_hz),
        };
        if let Some(d) = pick {
            self.dial_hz = d;
        }
    }

    fn lock_item(&mut self, t: f64, l: &BeaconLock, events: &mut Vec<Event>) {
        self.lock = *l;
        let locked = l.phase == LockPhase::Locked;
        if locked && !self.was_locked {
            events.push(QO100_LOCK.event(
                t,
                BEACON_PSK,
                &Qo100LockEvent {
                    text: &format!("beacon lock, LNB error {:+.1} kHz", l.error_hz / 1e3),
                    family: Family::Satellite,
                    lnb_error_hz: Hertz(l.error_hz),
                    lnb_error_ppm: l.error_ppm,
                },
            ));
        }
        self.was_locked = locked;
        if l.measurements > self.last_measurements {
            self.last_measurements = l.measurements;
            self.history.push_back((t, l.error_hz));
            while self.history.len() > HISTORY_CAP {
                self.history.pop_front();
            }
        }
    }
}

/// A spectrum as series points: x Hz above 10489.5 MHz, y dBFS.
fn spectrum_points(bins: &[f32], f0: f64, bin: f64) -> impl Iterator<Item = [f32; 2]> + '_ {
    bins.iter()
        .enumerate()
        .map(move |(i, v)| [(f0 + i as f64 * bin - NB_LO) as f32, *v])
}

impl Decoder for QoDisplay {
    type Input = Timed<QoItem>;
    fn name(&self) -> &'static str {
        "satcom.qo100_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        QoDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (QoDisplaySetting::Dial, Value::Hz(self.dial_hz)),
            (
                QoDisplaySetting::AutoListen,
                Value::Choice(self.auto_listen.index()),
            ),
            (
                QoDisplaySetting::ConversationGap,
                Value::Duration(self.gap_s),
            ),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        QoDisplayCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match QoDisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            QoDisplayCommand::SpeechText => {
                self.snippet.clear();
                let Qo100SpeechArgs { text } = QO100_SPEECH_TEXT.args(args)?;
                self.snippet.push_str(text);
            }
            QoDisplayCommand::ConversationListen => {
                let Qo100ListenArgs { row } = QO100_CONVERSATION_LISTEN.args(args)?;
                if row.is_empty() {
                    self.listening = None;
                } else {
                    let id = row
                        .parse::<u64>()
                        .ok()
                        .filter(|id| self.tables.conversations.get(id).is_some())
                        .ok_or_else(|| self.results().no_such_row(&QO100_CONVERSATIONS, row))?;
                    self.listening = Some(id);
                }
            }
            QoDisplayCommand::Remove => {
                let p = self.results();
                crate::display::remove(
                    &mut self.tables.conversations,
                    &QO100_CONVERSATIONS,
                    &p,
                    args,
                    &QO100_CONVERSATION_ROW,
                )?;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match QoDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            QoDisplaySetting::Dial => {
                self.dial_hz = DIAL.read(id, v)?;
                self.set_dial_hz = self.dial_hz;
            }
            QoDisplaySetting::AutoListen => self.auto_listen = AUTO_LISTEN.read(id, v)?,
            QoDisplaySetting::ConversationGap => self.gap_s = CONVERSATION_GAP.read(id, v)?,
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(results_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, _input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, _input: &Format) {
        self.tracks.clear();
        self.signals.clear();
    }
    fn process(&mut self, x: &[Timed<QoItem>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            let t = self.clock.at(it.time);
            match &it.value {
                QoItem::Lock(l) => self.lock_item(t, l, events),
                QoItem::Narrow {
                    bins_dbfs,
                    f0_hz,
                    bin_hz,
                    floor_dbfs,
                    signals,
                } => {
                    self.narrow = Some((bins_dbfs.clone(), *f0_hz, *bin_hz, *floor_dbfs));
                    self.follow(t, signals, events);
                }
                QoItem::Wide {
                    bins_dbfs,
                    f0_hz,
                    bin_hz,
                    datv,
                    outside,
                } => {
                    self.wide = Some((bins_dbfs.clone(), *f0_hz, *bin_hz));
                    self.wide_outside = *outside;
                    self.datv.clear();
                    self.datv.extend_from_slice(datv);
                }
            }
        }
    }
    fn report(&mut self, p: &mut Published) {
        self.follow_dial();
        let now = self.clock.now();
        let l = self.lock;
        p.put(&Qo100Fields {
            lnb_error_hz: Hertz(l.error_hz),
            lnb_error_ppm: l.error_ppm,
            drift_hz_s: l.drift_hz_s,
            beacon_lock: l.phase,
            correction_applied: l.applied && l.phase == LockPhase::Locked,
            measurements: l.measurements as i64,
            locked_for: Seconds(l.since_lock_s),
            dial_hz: Hertz(self.dial_hz),
            listening: self.listening.map(|v| v as i64),
            wideband_outside: self.wide_outside,
            narrow_floor_dbfs: self.narrow.as_ref().map(|n| n.3),
            own_to_beacon_db: own_to_beacon_db(&self.signals, self.set_dial_hz),
        });
        let named: Vec<_> = l
            .beacons
            .iter()
            .filter(|b| !b.name.is_empty())
            .copied()
            .collect();
        shaped::indexed_rows(p, &QO100_BEACONS, &named, |b, out| {
            out.put(&Qo100BeaconRow {
                    name: b.name,
                    nominal_hz: Hertz(b.nominal_hz),
                    found: b.found,
                    apparent_hz: Hertz(b.apparent_hz),
                    snr_db: b.snr_db,
                },
            );
        });
        shaped::indexed_rows(p, &QO100_SIGNALS, &self.signals, |s, out| {
            out.put(&Qo100SignalRow {
                    rf_hz: Hertz(s.rf_hz),
                    width_hz: Hertz(s.width_hz),
                    level_dbfs: s.level_dbfs,
                    snr_db: s.snr_db,
                    class: s.class,
                    beacon: (!s.beacon.is_empty()).then_some(s.beacon),
                    track: (s.track > 0).then_some(s.track as i64),
                },
            );
        });
        shaped::indexed_rows(p, &QO100_DATV, &self.datv, |d, out| {
            out.put(&Qo100DatvCarrierRow {
                    rf_hz: Hertz(d.rf_hz),
                    if_hz: Hertz(d.if_hz),
                    width_hz: Hertz(d.width_hz),
                    symbol_rate_hz: Hertz(d.symbol_rate_hz),
                    snr_db: d.snr_db,
                    flat_topped: d.flat_topped,
                    is_beacon: d.beacon,
                },
            );
        });
        shaped::rows(&self.tracks, &QO100_TRACKS, now, p);
        if std::mem::take(&mut self.dirty) {
            shaped::rows(&self.tables.conversations, &QO100_CONVERSATIONS, now, p);
            p.clear_rows(&QO100_CALLSIGNS);
            let mut key = String::new();
            for r in self.tables.conversations.iter() {
                for c in &r.record.callsigns {
                    key.clear();
                    let _ = write!(key, "{}/{c}", r.key);
                    QO100_CALLSIGNS.put(
                        p,
                        &key,
                        &Qo100CallsignRow {
                            conversation: *r.key as i64,
                            callsign: c,
                        },
                    );
                }
            }
        }
        let r = p.results_mut();
        if let Some((bins, f0, bin, _)) = &self.narrow {
            set_series(r, QO100_NARROWBAND.key, true, || {
                spectrum_points(bins, *f0, *bin)
            });
        }
        if let Some((bins, f0, bin)) = &self.wide {
            set_series(r, QO100_WIDEBAND.key, true, || {
                spectrum_points(bins, *f0, *bin)
            });
        }
        let history = || self.history.iter().map(|(t, e)| [*t as f32, *e as f32]);
        set_series(r, QO100_LNB_ERROR.key, true, history);
        let active = self
            .tables
            .conversations
            .iter()
            .filter(|c| c.record.active)
            .count();
        let signals = self.signals.len();
        let lock_state = match l.phase {
            LockPhase::Outside | LockPhase::Searching => LockState::Searching,
            LockPhase::Held => LockState::Tracking,
            LockPhase::Locked => LockState::Locked,
        };
        let offset = (l.measurements > 0).then_some(l.error_hz);
        let snr = l
            .beacons
            .iter()
            .filter(|b| b.found)
            .map(|b| b.snr_db as f32)
            .reduce(f32::max);
        r.update_status(|st| {
            let mut summary = String::new();
            let _ = match l.phase {
                LockPhase::Outside => write!(
                    summary,
                    "QO-100: the narrowband transponder is outside the radio's band"
                ),
                LockPhase::Searching => write!(summary, "QO-100: searching for the beacons"),
                LockPhase::Locked => write!(
                    summary,
                    "QO-100: locked, LNB {:+.1} kHz ({:+.2} ppm), {signals} signals, {active} conversations",
                    l.error_hz / 1e3,
                    l.error_ppm,
                ),
                LockPhase::Held => write!(
                    summary,
                    "QO-100: beacons lost, holding LNB {:+.1} kHz",
                    l.error_hz / 1e3
                ),
            };
            let same =
                st.lock_state == lock_state && st.offset_hz == offset && st.snr_db == snr && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.summary = summary;
            }
            !same
        });
    }
    fn clear(&mut self) {
        self.tables.clear();
        self.tracks.clear();
        self.history.clear();
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_callsigns() {
        assert_eq!(
            callsigns("lima yankee two alpha bravo charlie here"),
            vec!["LY2ABC"]
        );
        assert!(callsigns("hello world kilo oscar").is_empty());
        assert!(callsigns("cq de x-ray ray").is_empty());
        assert_eq!(callsigns("this is DL1ABC calling"), vec!["DL1ABC"]);
    }

    #[test]
    fn classes_by_width_and_level() {
        assert_eq!(classify(50.0, 1.0, 0.1, false), SignalClass::Carrier);
        assert_eq!(classify(50.0, 0.5, 3.0, false), SignalClass::Cw);
        assert_eq!(classify(2400.0, 0.6, 4.0, false), SignalClass::Ssb);
        assert_eq!(classify(2400.0, 1.0, 4.0, true), SignalClass::Beacon);
    }

    #[test]
    fn the_own_signal_is_measured_against_the_psk_beacon() {
        let sig = |rf_hz: f64, level_dbfs: f64, beacon: &'static str| Signal {
            rf_hz,
            level_dbfs,
            beacon,
            ..Signal::default()
        };
        let dial = 10489.65e6;
        let signals = [
            sig(BEACON_PSK, -30.0, "PSK"),
            sig(dial + 1200.0, -36.5, ""),
            sig(dial + 2000.0, -41.0, ""),
            sig(dial + 5000.0, -20.0, ""),
            sig(dial - 100.0, -25.0, ""),
        ];
        assert_eq!(own_to_beacon_db(&signals, dial), Some(-6.5));
        assert_eq!(own_to_beacon_db(&signals, dial + 20e3), None);
        assert_eq!(own_to_beacon_db(&signals[1..], dial), None);
        // The beacon is not the user's own signal.
        assert_eq!(own_to_beacon_db(&signals, BEACON_PSK - 100.0), None);
        // A followed conversation moves the dial heard, not the place measured.
        let mut d = QoDisplay::default();
        Decoder::apply(&mut d, QoDisplaySetting::Dial.id(), &Value::Hz(dial)).unwrap();
        d.signals.extend_from_slice(&signals);
        d.dial_hz = dial + 20e3;
        let mut p = Decoder::results(&d);
        d.report(&mut p);
        let r = p.results();
        let id = r.decl().field_id("own_to_beacon_db").unwrap();
        assert_eq!(r.fields().value(id), Some(&Value::F64(-6.5)));
    }
}
