//! `ism.decode` (the protocols, and the line-code guess of trains no protocol decoded) and `ism.devices` (the device table, the Z-Wave view,
//! the log, the unknown signals, the Sigfox messages heard inside `ism` and the frames of the pulse analyser, with the status and
//! the events). In the submode `pulse_analyser`, `ism.decode` reads nothing and the status is that of the pulse analyser.

use super::analyser::{pulse_code, PulseFrame, Submode, SubmodeSetting, SUBMODE};
use crate::family::{set_status, wall};
use crate::sigfox::{
    message_event, Message as SfxMessage, Messages as SfxMessages, SigfoxState, Uplink,
};
use crate::typed::{
    IsmDeviceEvent, IsmDeviceRow, IsmFields, IsmLogRow, IsmPulseFields, IsmPulseFrameEvent,
    IsmSigfoxRow, IsmUnknownRow, IsmZwaveRow, ISM_DEVICE, ISM_DEVICES, ISM_LOG, ISM_PULSE_FRAME,
    ISM_PULSE_FRAMES, ISM_SIGFOX, ISM_UNKNOWN, ISM_ZWAVE, PAN_DEVICE_ROW, PAN_REMOVE,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, Record, Results, SettingError, SettingId, SettingSet,
    StateBlob, StateError, TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    RunContext, Timed,
};
use sdrkit_proto_ookdev::components::DecodeFields;
use sdrkit_proto_ookdev::guess::{Guess, Guesser, LineCode};
use sdrkit_proto_ookdev::protocol::Decoding;
use sdrkit_proto_ookdev::reading::measure;
use sdrkit_proto_ookdev::{Channel, Check, Measurement, Modulation, ProtocolList, Reading, Train};
use sdrkit_util_objects::commands;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// Devices kept.
pub const DEVICE_CAP: usize = 300;
/// Log lines kept.
pub const LOG_CAP: usize = 400;
/// Unknown signals kept.
pub const UNKNOWN_CAP: usize = 60;
/// Sigfox messages kept.
pub const SIGFOX_CAP: usize = 300;
/// Pulses an unknown signal keeps.
pub const UNKNOWN_PULSES: usize = 600;
/// Frames of the pulse analyser kept.
pub const PULSE_FRAME_CAP: usize = 500;
/// Values of the history of a device.
pub const HISTORY: usize = 64;

/// A train no protocol decoded, with its line-code guess (`ism.unknown`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UnknownSignal {
    /// Signal time, seconds.
    pub time_s: f64,
    /// Hz from the input centre.
    pub offset_hz: f64,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// SNR, dB.
    pub snr_db: f32,
    /// OOK or FSK.
    pub modulation: Modulation,
    /// FSK deviation, Hz.
    pub deviation_hz: f64,
    /// The first pulses as (mark, space) in µs.
    pub pulses: Vec<(f32, f32)>,
    /// The number of pulses.
    pub count: usize,
    /// The line-code guess.
    pub guess: Guess,
    /// The frequency, Hz (the input centre plus the offset).
    #[serde(default)]
    pub freq_hz: f64,
}

impl Item for UnknownSignal {
    const KIND: &'static str = "ism.unknown";
}

impl UnknownSignal {
    /// The signal of `train` with the guess of `guesser`.
    pub fn of(train: &Train, guesser: &mut Guesser) -> UnknownSignal {
        let n = train.pulses.len();
        UnknownSignal {
            time_s: train.time_s(),
            offset_hz: train.offset_hz,
            level_dbfs: train.level_dbfs,
            snr_db: train.snr_db,
            modulation: train.modulation,
            deviation_hz: train.deviation_hz,
            pulses: train
                .pulses
                .iter()
                .take(UNKNOWN_PULSES)
                .enumerate()
                .map(|(i, p)| {
                    let space = if i + 1 == n && train.modulation == Modulation::Ook {
                        0.0
                    } else {
                        train.us(p.space)
                    };
                    (train.us(p.mark), space)
                })
                .collect(),
            count: n,
            guess: guesser.guess(train),
            freq_hz: 0.0,
        }
    }

    /// "OOK PWM, 25 pulses, short 464 us, long 1404 us".
    pub fn line(&self) -> String {
        let m = self.modulation.name();
        if self.guess.code == LineCode::Unknown {
            format!("{m} ?, {} pulses", self.count)
        } else {
            format!(
                "{m} {}, {} pulses, short {:.0} us, long {:.0} us",
                self.guess.code.name(),
                self.count,
                self.guess.short_us,
                self.guess.long_us
            )
        }
    }

    /// The copy text: "OOK PWM 464 us/1404 us; 464/1404 1404/464 ...".
    pub fn copy_text(&self) -> String {
        let mut s = format!(
            "{} {} {:.0} us/{:.0} us;",
            self.modulation.name(),
            self.guess.code.name(),
            self.guess.short_us,
            self.guess.long_us
        );
        for (a, b) in &self.pulses {
            s.push_str(&format!(" {a:.0}/{b:.0}"));
        }
        s
    }
}

impl Shift for UnknownSignal {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

impl Shaped for UnknownSignal {
    type Row = IsmUnknownRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&IsmUnknownRow {
            time_s: self.time_s,
            frequency: Hertz(self.freq_hz),
            modulation: self.modulation.name(),
            line_code: self.guess.code.name(),
            pulses: count(self.count as u64),
            short_us: f64::from(self.guess.short_us),
            long_us: f64::from(self.guess.long_us),
            level_dbfs: f64::from(self.level_dbfs),
            snr_db: f64::from(self.snr_db),
            copy: &self.copy_text(),
        });
    }
}

/// A count as a whole number of a result field.
fn count(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// Trains with fewer pulses that no protocol decodes are not unknown signals (noise).
pub const UNKNOWN_MIN_PULSES: usize = 8;

/// `ism.decode`: offers each train to every protocol of the list (`sdrkit_proto_ookdev::Decoding`), and turns the trains no protocol
/// decoded into unknown signals with their line-code guess. The outputs keep the input's centre and dial.
pub struct Decode {
    list: Arc<ProtocolList>,
    work: Decoding,
    guesser: Guesser,
    found: Vec<Reading>,
    readings: u64,
    unknown: u64,
    published: Published,
    submode: Submode,
}

impl Decode {
    /// A decoder over `list`.
    pub fn new(list: Arc<ProtocolList>) -> Self {
        Decode {
            list,
            work: Decoding::new(),
            guesser: Guesser::default(),
            found: Vec::new(),
            readings: 0,
            unknown: 0,
            published: Published::new(DecodeFields::SHAPE.fields),
            submode: SUBMODE.default,
        }
    }

    fn put(&mut self) {
        self.published.put(&DecodeFields {
            readings: count(self.readings),
            unknown_trains: count(self.unknown),
        });
    }
}

impl Component for Decode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.decode",
            vec![Port::typed::<Train>("trains")],
            vec![
                Port::typed::<Reading>("readings").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<UnknownSignal>("unknown").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(SubmodeSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(SubmodeSetting::Submode, Value::Choice(self.submode.index()))])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match SubmodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SubmodeSetting::Submode => self.submode = SUBMODE.read(id, value)?,
        }
        Ok(Applied::Live)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.put();
        Ok(vec![
            inputs[0].with_kind(Kind::typed::<Reading>()),
            inputs[0].with_kind(Kind::typed::<UnknownSignal>()),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let trains = io.items::<Train>(0);
        if trains.is_empty() || self.submode == Submode::PulseAnalyser {
            return;
        }
        let mut good = Vec::new();
        let mut unknown = Vec::new();
        for t in trains {
            self.found.clear();
            let n = self
                .work
                .run(&self.list, &t.value, &mut self.found)
                .unwrap_or(0);
            if n > 0 {
                good.extend(self.found.drain(..).map(|r| Timed {
                    time: t.time,
                    value: r,
                }));
            } else if t.value.pulses.len() >= UNKNOWN_MIN_PULSES {
                unknown.push(Timed {
                    time: t.time,
                    value: UnknownSignal::of(&t.value, &mut self.guesser),
                });
            }
        }
        self.readings += good.len() as u64;
        self.unknown += unknown.len() as u64;
        self.put();
        io.items_out::<Reading>(0).extend(good);
        io.items_out::<UnknownSignal>(1).extend(unknown);
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// One device of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// Model.
    pub model: String,
    /// Kind.
    pub kind: String,
    /// Id.
    pub id: String,
    /// Channel.
    pub channel: Channel,
    /// Battery state.
    pub battery_ok: Option<bool>,
    /// The frame test of the last reading.
    pub check: Check,
    /// The measurements of the last reading.
    pub fields: Vec<Measurement>,
    /// Signal time of the last transmission, seconds.
    pub last_time_s: f64,
    /// Distinct transmissions.
    pub count: u64,
    /// Hz from the input centre.
    pub offset_hz: f64,
    /// The frequency, Hz.
    pub freq_hz: f64,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// SNR, dB.
    pub snr_db: f32,
    /// The first number with a unit of each reading, oldest first.
    pub history: VecDeque<f64>,
    /// That number's label and unit.
    pub history_field: String,
    /// The fields as one line.
    pub line: String,
}

impl Shift for Device {
    fn shift_times(&mut self, by_s: f64) {
        self.last_time_s += by_s;
    }
}

impl Shaped for Device {
    type Row = IsmDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let channel = (self.channel != Channel::None).then(|| self.channel.to_string());
        let history = (!self.history.is_empty()).then(|| {
            let mut s = format!("{}:", self.history_field);
            for v in &self.history {
                let _ = write!(s, " {v}");
            }
            s
        });
        out.put(&IsmDeviceRow {
            model: &self.model,
            kind: &self.kind,
            id: &self.id,
            channel: channel.as_deref(),
            battery_ok: self.battery_ok,
            readings: &self.line,
            history: history.as_deref(),
            level_dbfs: f64::from(self.level_dbfs),
            snr_db: f64::from(self.snr_db),
            last_heard: columns::last_heard(seen).map(Time),
            count: count(self.count),
            frequency: Hertz(self.freq_hz),
            check: self.check,
        });
    }
}

/// Writes the row of the Z-Wave view of device `d` (of kind `zwave`).
fn zwave_row(d: &Device, seen: &Seen, out: RowOut<'_, '_, IsmZwaveRow<'static>>) {
    let home = d.fields.iter().find_map(|m| m.get(&measure::HOME_ID));
    let node = d.fields.iter().find_map(|m| m.get(&measure::SOURCE_NODE));
    let command = d.fields.iter().find_map(|m| m.get(&measure::COMMAND));
    let value = d
        .fields
        .iter()
        .skip_while(|m| m.key() != measure::COMMAND.column().key)
        .nth(1)
        .map(Measurement::line);
    out.put(&IsmZwaveRow {
        home_id: home,
        node,
        rate: d.model.trim_start_matches("Z-Wave-"),
        last_heard: columns::last_heard(Some(seen)).map(Time),
        level_dbfs: f64::from(d.level_dbfs),
        count: count(d.count),
        command,
        value: value.as_deref(),
    });
}

/// One log line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// Signal time, seconds.
    pub time_s: f64,
    /// Model.
    pub model: String,
    /// Id.
    pub id: String,
    /// Frequency, Hz.
    pub freq_hz: f64,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// The reading as one line.
    pub text: String,
}

impl Shift for LogEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

impl Shaped for LogEntry {
    type Row = IsmLogRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&IsmLogRow {
            time_s: self.time_s,
            model: &self.model,
            id: &self.id,
            frequency: Hertz(self.freq_hz),
            level_dbfs: f64::from(self.level_dbfs),
            text: &self.text,
        });
    }
}

/// Makes the rows of [`ISM_SIGFOX`] in `p` hold `messages` (newest first), with the input centre `centre_hz` for their
/// frequency: writes each, and removes the rows of messages that left. `keys` holds the row keys written last time.
fn message_rows(
    messages: &VecDeque<SfxMessage>,
    centre_hz: f64,
    p: &mut Published,
    keys: &mut Vec<String>,
) {
    for k in keys.iter() {
        if !messages.iter().any(|m| &m.key == k) {
            p.remove_row(&ISM_SIGFOX, k);
        }
    }
    keys.clear();
    for m in messages.iter().rev() {
        let mac_crc = format!("{} (not checked) / {}", m.auth, m.crc);
        ISM_SIGFOX.put(
            p,
            &m.key,
            &IsmSigfoxRow {
                time_s: m.first,
                device_id: &m.device_id,
                seq: i64::from(m.counter),
                payload: &m.payload,
                frequency: Hertz(centre_hz + m.freqs.first().copied().unwrap_or(0.0)),
                level_dbfs: f64::from(m.level_db),
                reps: i64::from(m.reps),
                mac_crc: &mac_crc,
            },
        );
        keys.push(m.key.clone());
    }
}

/// Everything `ism` keeps through a restart and in the saved state.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The devices, by "model|id|channel".
    pub devices: ObjectTable<String, Device>,
    /// The reading log.
    pub log: LogRing<LogEntry>,
    /// The unknown signals.
    pub unknown: LogRing<UnknownSignal>,
    /// The Sigfox messages, newest first.
    pub sigfox: VecDeque<SfxMessage>,
    /// Distinct transmissions decoded.
    pub packets: u64,
    /// Trains no protocol decoded.
    pub unknown_bursts: u64,
    /// The frames of the pulse analyser with a line code.
    #[serde(default = "pulse_log")]
    pub pulse_frames: LogRing<PulseFrame>,
    /// Frames the pulse analyser read.
    #[serde(default)]
    pub frames_seen: u64,
    /// Frames the pulse analyser named a line code for.
    #[serde(default)]
    pub frames_good: u64,
}

/// An empty log of the frames of the pulse analyser.
fn pulse_log() -> LogRing<PulseFrame> {
    LogRing::new(PULSE_FRAME_CAP)
}

impl Default for Store {
    fn default() -> Self {
        Store {
            devices: ObjectTable::new(DEVICE_CAP),
            log: LogRing::new(LOG_CAP),
            unknown: LogRing::new(UNKNOWN_CAP),
            sigfox: VecDeque::new(),
            packets: 0,
            unknown_bursts: 0,
            pulse_frames: pulse_log(),
            frames_seen: 0,
            frames_good: 0,
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.devices.shift_times(by_s);
        self.log.shift_times(by_s);
        self.unknown.shift_times(by_s);
        self.pulse_frames.shift_times(by_s);
        for m in &mut self.sigfox {
            m.shift_times(by_s);
        }
    }
}

impl Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("ism.devices", 2);
    fn clear(&mut self) {
        self.devices.clear();
        self.log.clear();
        self.unknown.clear();
        self.sigfox.clear();
        self.packets = 0;
        self.unknown_bursts = 0;
        self.pulse_frames.clear();
        self.frames_seen = 0;
        self.frames_good = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.devices.adopt(saved.devices);
        self.log.adopt(saved.log);
        self.unknown.adopt(saved.unknown);
        self.sigfox = saved.sigfox;
        self.sigfox.truncate(SIGFOX_CAP);
        self.packets = saved.packets;
        self.unknown_bursts = saved.unknown_bursts;
        self.pulse_frames.adopt(saved.pulse_frames);
        self.frames_seen = saved.frames_seen;
        self.frames_good = saved.frames_good;
    }
}

/// The device table and everything `ism` publishes.
pub struct Devices {
    published: Published,
    devices_table: TableId,
    sigfox_keys: Vec<String>,
    summary: String,
    store: Store,
    centre_hz: f64,
    dial_hz: f64,
    sigfox_active: bool,
    live: bool,
    now_s: f64,
    last_heard_s: f64,
    last_snr_db: Option<f32>,
    events: Vec<Event>,
    submode: Submode,
    /// The last frame of the pulse analyser with a line code.
    last_frame: Option<PulseFrame>,
    /// The signal times of the last frame of the pulse analyser and of the last one with a line code.
    frame_s: f64,
    good_frame_s: f64,
}

impl Default for Devices {
    fn default() -> Self {
        let fields: Vec<_> = IsmFields::SHAPE
            .fields
            .iter()
            .chain(IsmPulseFields::SHAPE.fields)
            .copied()
            .collect();
        let published = Published::new(&fields).with_tables(&[
            &ISM_DEVICES,
            &ISM_ZWAVE,
            &ISM_LOG,
            &ISM_UNKNOWN,
            &ISM_SIGFOX,
            &ISM_PULSE_FRAMES,
        ]);
        let devices_table = published
            .results()
            .decl()
            .table_id(ISM_DEVICES.key)
            .expect("the tables of ism.devices are declared");
        Devices {
            published,
            devices_table,
            sigfox_keys: Vec::new(),
            summary: String::new(),
            store: Store::default(),
            centre_hz: 0.0,
            dial_hz: 0.0,
            sigfox_active: false,
            live: false,
            now_s: 0.0,
            last_heard_s: f64::NEG_INFINITY,
            last_snr_db: None,
            events: vec![],
            submode: SUBMODE.default,
            last_frame: None,
            frame_s: f64::NEG_INFINITY,
            good_frame_s: f64::NEG_INFINITY,
        }
    }
}

/// The table key of a reading: model, id and channel.
pub fn device_key(r: &Reading) -> String {
    format!("{}|{}|{}", r.model, r.id, r.channel)
}

impl Devices {
    /// The tables.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn freq(&self, offset_hz: f64) -> f64 {
        self.centre_hz + offset_hz
    }

    /// Adds a reading.
    pub fn add(&mut self, r: &Reading) {
        let t = r.time_s;
        self.now_s = self.now_s.max(t);
        self.last_heard_s = t;
        self.last_snr_db = Some(r.snr_db);
        let line = r.line();
        let freq = self.freq(r.offset_hz);
        let mut stamp = Stamp::at(t);
        if let Some(w) = wall(self.live) {
            stamp = stamp.with_wall(w);
        }
        let key = device_key(r);
        let Ok(up) = self.store.devices.upsert(key.as_str(), stamp, || Device {
            model: r.model.to_string(),
            kind: r.kind.to_string(),
            id: r.id.to_string(),
            channel: r.channel.clone(),
            battery_ok: r.battery_ok,
            check: r.check,
            fields: Vec::new(),
            last_time_s: f64::NEG_INFINITY,
            count: 0,
            offset_hz: r.offset_hz,
            freq_hz: freq,
            level_dbfs: r.level_dbfs,
            snr_db: r.snr_db,
            history: VecDeque::with_capacity(HISTORY),
            history_field: String::new(),
            line: String::new(),
        }) else {
            return;
        };
        let new = up.new;
        let d = up.row;
        // A repeat of the same reading within 2 s counts once.
        let repeat = !new && d.fields == r.fields && (t - d.last_time_s).abs() < 2.0;
        d.last_time_s = t;
        d.level_dbfs = r.level_dbfs;
        d.snr_db = r.snr_db;
        d.offset_hz = r.offset_hz;
        d.freq_hz = freq;
        if repeat {
            return;
        }
        d.count += 1;
        d.fields = r.fields.clone();
        d.battery_ok = r.battery_ok;
        d.check = r.check;
        d.line.clone_from(&line);
        let first_number = r.fields.iter().find_map(|m| match m.value() {
            Value::F64(v) => {
                let decl = m.field().decl();
                (!decl.unit.is_empty()).then_some((*v, decl))
            }
            _ => None,
        });
        if let Some((v, decl)) = first_number {
            if d.history.len() == HISTORY {
                d.history.pop_front();
            }
            d.history.push_back(v);
            if d.history_field.is_empty() {
                d.history_field = format!("{} ({})", decl.label, decl.unit);
            }
        }
        if new {
            let source = format!("{} {}", r.model, r.id);
            let text = format!("ISM: {}, ID {}, at {:.3} MHz", r.model, r.id, freq / 1e6);
            self.events.push(ISM_DEVICE.event(
                t,
                freq,
                &IsmDeviceEvent {
                    source: &source,
                    text: &text,
                    family: Family::Sensor,
                    model: &r.model,
                    id: &r.id,
                },
            ));
        }
        self.store.packets += 1;
        self.published.results_mut().log(format!(
            "{t:.3} {} {} {:.3} MHz {:.1} dB {line}",
            r.model,
            r.id,
            freq / 1e6,
            r.level_dbfs
        ));
        self.store.log.push(LogEntry {
            time_s: t,
            model: r.model.to_string(),
            id: r.id.to_string(),
            freq_hz: freq,
            level_dbfs: r.level_dbfs,
            text: line,
        });
    }

    /// Adds an unknown signal.
    pub fn add_unknown(&mut self, u: &UnknownSignal) {
        self.store.unknown_bursts += 1;
        let mut u = u.clone();
        u.freq_hz = self.freq(u.offset_hz);
        self.published
            .results_mut()
            .log(format!("{:.3} unknown: {}", u.time_s, u.line()));
        self.store.unknown.push(u);
    }

    /// Adds a frame of the pulse analyser: every frame counts as seen; a frame with a line code goes to the log, becomes the
    /// last frame and makes an event.
    pub fn add_frame(&mut self, f: &PulseFrame) {
        let t = f.time_s;
        self.now_s = self.now_s.max(t);
        self.frame_s = t;
        self.store.frames_seen += 1;
        let Some(line_code) = pulse_code(f.code) else {
            return;
        };
        self.good_frame_s = t;
        self.store.frames_good += 1;
        let mut f = f.clone();
        f.freq_hz = self.freq(f.offset_hz);
        let headline = f.headline();
        let text = format!(
            "Pulse analyser: {headline}, at {:.3} MHz (line code guessed from the timings)",
            f.freq_hz / 1e6
        );
        self.published
            .results_mut()
            .log(format!("{t:.3} {headline}"));
        self.events.push(ISM_PULSE_FRAME.event(
            t,
            f.freq_hz,
            &IsmPulseFrameEvent {
                text: &text,
                family: Family::Sensor,
                line_code,
                bits: count(f.bits.len() as u64),
                bits_hex: &f.hex(),
                repeats: i64::from(f.repeats),
                level_dbfs: f64::from(f.level_dbfs),
            },
        ));
        self.store.pulse_frames.push(f.clone());
        self.last_frame = Some(f);
    }

    /// The summary and the lock level of the pulse analyser: 2 with a frame with a line code in the last 8 s, 1 with any frame
    /// in the last 2 s, else 0.
    fn pulse_status(&mut self) -> u8 {
        let s = &self.store;
        let now = self.now_s;
        let headline = match &self.last_frame {
            Some(f) => f.headline(),
            None if s.frames_seen > 0 => format!(
                "{} frames seen, none with a recognised pattern yet",
                s.frames_seen
            ),
            None if now > 0.0 => "no OOK frame yet".to_string(),
            None => "waiting for data".to_string(),
        };
        self.summary.clear();
        let _ = write!(
            self.summary,
            "Pulse analyser (measures pulse timings and guesses the line code; decodes no device): {headline}"
        );
        if now - self.good_frame_s < 8.0 {
            2
        } else if now - self.frame_s < 2.0 {
            1
        } else {
            0
        }
    }

    /// The result fields of the pulse analyser.
    fn pulse_fields(&self) -> (Option<String>, Option<String>) {
        let f = self.last_frame.as_ref();
        (f.map(PulseFrame::bin), f.map(PulseFrame::hex))
    }

    /// Adds a Sigfox uplink.
    pub fn add_sigfox(&mut self, u: &Uplink, time: f64) {
        let mut m = SfxMessages {
            list: std::mem::take(&mut self.store.sigfox),
            frames: 0,
        };
        if let Some(line) = m.add(u, self.dial_hz) {
            self.events
                .push(message_event(u, time, self.freq(u.offset_hz), &line));
        }
        m.list.truncate(SIGFOX_CAP);
        self.store.sigfox = m.list;
    }

    fn clear_rows(&mut self) {
        for t in [
            ISM_DEVICES.def(),
            ISM_ZWAVE.def(),
            ISM_LOG.def(),
            ISM_UNKNOWN.def(),
            ISM_SIGFOX.def(),
            ISM_PULSE_FRAMES.def(),
        ] {
            self.published.clear_rows(t);
        }
        self.sigfox_keys.clear();
    }

    fn publish(&mut self) {
        let now = self.now_s;
        let heard = now - self.last_heard_s < 10.0;
        let pulse_level = (self.submode == Submode::PulseAnalyser).then(|| self.pulse_status());
        let (bits_bin, bits_hex) = self.pulse_fields();
        let p = &mut self.published;
        let s = &self.store;
        let n = s.devices.len();
        let _ = if pulse_level.is_some() {
            Ok(())
        } else if self.sigfox_active && !s.sigfox.is_empty() {
            self.summary.clear();
            write!(
                self.summary,
                "{n} devices, {} packets, {} Sigfox messages",
                s.packets,
                s.sigfox.len()
            )
        } else {
            self.summary.clear();
            write!(
                self.summary,
                "{n} devices, {} packets, {} unknown",
                s.packets, s.unknown_bursts
            )
        };
        let level = pulse_level.unwrap_or(if heard { 2 } else { 0 });
        set_status(p, LockState::from_level(level), &self.summary);
        let snr_db = self.last_snr_db;
        p.results_mut().update_status(|st| {
            let changed = st.snr_db != snr_db || st.offset_hz != Some(0.0);
            st.snr_db = snr_db;
            st.offset_hz = Some(0.0);
            changed
        });
        p.put(&IsmFields {
            devices: count(n as u64),
            packets_decoded: count(s.packets),
            unknown_bursts: count(s.unknown_bursts),
            sigfox_messages: count(s.sigfox.len() as u64),
        });
        let f = self.last_frame.as_ref();
        p.put(&IsmPulseFields {
            line_code: f.and_then(|f| pulse_code(f.code)),
            bits: f.map(|f| count(f.bits.len() as u64)),
            bits_bin: bits_bin.as_deref(),
            bits_hex: bits_hex.as_deref(),
            short_us: f.and_then(|f| f.short_us).map(f64::from),
            long_us: f.and_then(|f| f.long_us).map(f64::from),
            pulse_us: f.and_then(|f| f.pulse_us).map(f64::from),
            period_us: f.and_then(|f| f.period_us).map(f64::from),
            half_bit_us: f.and_then(PulseFrame::half_bit_us),
            unit_us: f.and_then(PulseFrame::pcm_unit_us),
            bit_rate: f.and_then(|f| f.bit_rate),
            pulses: f.map(|f| count(f.pulses as u64)),
            frame_ms: f.map(|f| f.frame_ms),
            repeats: f.map(|f| i64::from(f.repeats)),
            frames_seen: count(s.frames_seen),
            frames_good: count(s.frames_good),
        });
        shaped::rows(&s.devices, &ISM_DEVICES, now, p);
        shaped::rows_by(
            &s.devices,
            &ISM_ZWAVE,
            p,
            |r| r.record.kind == "zwave",
            |r, out| zwave_row(r.record, r.seen, out),
        );
        shaped::log_rows(&s.log, &ISM_LOG, now, p);
        shaped::log_rows(&s.unknown, &ISM_UNKNOWN, now, p);
        shaped::log_rows(&s.pulse_frames, &ISM_PULSE_FRAMES, now, p);
        message_rows(&s.sigfox, self.centre_hz, p, &mut self.sigfox_keys);
    }
}

impl Component for Devices {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ism.devices",
            vec![
                Port::typed::<Reading>("readings"),
                Port::typed::<UnknownSignal>("unknown"),
                Port::typed::<Uplink>("sigfox"),
                Port::typed::<SigfoxState>("sigfox_state"),
                Port::typed::<PulseFrame>("frames"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SubmodeSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.centre_hz = inputs[0].centre_hz;
        self.dial_hz = inputs[0].tuned_hz - inputs[0].centre_hz;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
    }
    fn process(&mut self, io: &mut Io) {
        let mut new = false;
        for t in io.items::<Reading>(0) {
            self.add(&t.value);
            new = true;
        }
        for t in io.items::<UnknownSignal>(1) {
            self.add_unknown(&t.value);
            new = true;
        }
        for t in io.items::<Uplink>(2) {
            self.add_sigfox(&t.value, t.time);
            new = true;
        }
        for s in io.items::<SigfoxState>(3) {
            self.sigfox_active = s.value.active;
            new = true;
        }
        for f in io.items::<PulseFrame>(4) {
            self.add_frame(&f.value);
            new = true;
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        if io.tick() {
            self.now_s += crate::family::REPORT_S;
        }
        if io.tick() || new {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        values_of([(SubmodeSetting::Submode, Value::Choice(self.submode.index()))])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match SubmodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SubmodeSetting::Submode => self.submode = SUBMODE.read(id, value)?,
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn commands(&self) -> &Commands {
        DevicesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match DevicesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DevicesCommand::Clear => {
                self.store.clear();
                self.clear_rows();
                self.last_frame = None;
                self.frame_s = f64::NEG_INFINITY;
                self.good_frame_s = f64::NEG_INFINITY;
            }
            DevicesCommand::Remove => commands::remove(
                &mut self.store.devices,
                self.devices_table,
                &mut self.published,
                args,
                &PAN_DEVICE_ROW,
            )?,
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn forget(&mut self, kept: &Kept) {
        let s = &mut self.store;
        if !kept.table(ISM_DEVICES.key) {
            s.devices.clear();
            self.published.clear_rows(&ISM_DEVICES);
            self.published.clear_rows(&ISM_ZWAVE);
        }
        if !kept.table(ISM_LOG.key) {
            s.log.clear();
            self.published.clear_rows(&ISM_LOG);
        }
        if !kept.table(ISM_UNKNOWN.key) {
            s.unknown.clear();
            self.published.clear_rows(&ISM_UNKNOWN);
        }
        if !kept.table(ISM_SIGFOX.key) {
            s.sigfox.clear();
            self.published.clear_rows(&ISM_SIGFOX);
            self.sigfox_keys.clear();
        }
        if !kept.field("packets_decoded") {
            s.packets = 0;
        }
        if !kept.field("unknown_bursts") {
            s.unknown_bursts = 0;
        }
        if !kept.table(ISM_PULSE_FRAMES.key) {
            s.pulse_frames.clear();
            self.published.clear_rows(&ISM_PULSE_FRAMES);
        }
        if !kept.field("frames_seen") {
            s.frames_seen = 0;
        }
        if !kept.field("frames_good") {
            s.frames_good = 0;
        }
        self.last_frame = None;
        self.frame_s = f64::NEG_INFINITY;
        self.good_frame_s = f64::NEG_INFINITY;
        self.last_heard_s = f64::NEG_INFINITY;
        self.last_snr_db = None;
        self.publish();
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish();
        Ok(())
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}

sdrkit_core::commands! {
    /// The commands of [`Devices`].
    pub enum DevicesCommand {
        /// Forgets everything heard.
        Clear = commands::CLEAR,
        /// Forgets one device.
        Remove = PAN_REMOVE.command,
    }
}
