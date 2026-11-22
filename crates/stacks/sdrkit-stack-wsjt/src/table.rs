//! The decode table, the clock estimator, JS8 message assembly and the stations, and what the receiver publishes. The lists
//! are the shared object tables, and every result is a declared typed field, table or event.

use crate::components::Js8Speed;
use crate::mode::Mode;
use crate::proto::js8msg;
use crate::rx::js8::{Js8Frame, Js8FrameKind};
use crate::rx::slot::{locator_centre, Decode};
use crate::slots::TimeSource;
use crate::typed::{
    self, DecodeEvent, DecodeRow, Js8FrameEvent, Js8FrameRow, Js8MessageEvent, Js8MessageRow,
    Js8StationRow, ModeRow, SpotEvent, StationRow, WsjtFields, WsprSpotRow, DECODE, DECODES,
    JS8_FRAME, JS8_FRAMES, JS8_MESSAGE, JS8_MESSAGES, JS8_STATIONS, MODES, SPOT, STATIONS,
    STATION_ROW, WSPR_SPOTS,
};
use sdrkit_algo_dsp::spectrum::percentile as quantile;
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, CommandError, Event, LockState, Results, ResultsDecl, StateBlob, StateError, Status,
    Values,
};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format, LogRing, ObjectTable, Seen, Shift, Stamp, Tables};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, OnceLock};

/// The decode table's cap.
pub const DECODES_CAP: usize = 1000;
/// JS8 frames kept.
pub const JS8_FRAMES_CAP: usize = 600;
/// JS8 messages kept.
pub const JS8_MESSAGES_CAP: usize = 200;
/// JS8 stations kept.
pub const JS8_STATIONS_CAP: usize = 600;

/// `hh:mm:ss` of a UTC, from the shared civil calendar (empty for a time it cannot show).
pub fn hms(utc: f64) -> String {
    sdrkit_util_geo::time::Civil::from_unix_f64(utc)
        .map(|c| format!("{:02}:{:02}:{:02}", c.hour, c.minute, c.second))
        .unwrap_or_default()
}

/// The clock estimate.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ClockReport {
    /// The median DT, s.
    pub error_s: f64,
    /// Half the distance between the 16th and 84th percentile DT, s.
    pub spread_s: f64,
    /// Decodes used.
    pub count: usize,
    /// |error| > 1 s with at least 2 decodes.
    pub warning: bool,
    /// The median DT of the newest 40 JS8 frames.
    pub js8_error_s: f64,
    /// Slot search: the shift to apply to the clock, s (once, when found).
    pub correction_s: Option<f64>,
}

/// The clock estimator.
#[derive(Clone, Debug, Default)]
pub struct ClockEstimator {
    recent: VecDeque<(f64, f64)>,
    js8: VecDeque<f64>,
    pending: Option<f64>,
    /// Reused buffers of the quantiles.
    values: Vec<f32>,
    scratch: Vec<f32>,
}

/// The `p` quantile of `v` by the shared quantile (0 for an empty list), with the buffers `values` and `scratch`. The values are
/// taken relative to the smallest, so times of day keep their precision in `f32`.
fn percentile(
    v: impl Iterator<Item = f64> + Clone,
    p: f64,
    values: &mut Vec<f32>,
    scratch: &mut Vec<f32>,
) -> f64 {
    let base = v.clone().fold(f64::INFINITY, f64::min);
    if !base.is_finite() {
        return 0.0;
    }
    values.clear();
    values.extend(v.map(|x| (x - base) as f32));
    base + f64::from(quantile(values, p, scratch))
}

impl ClockEstimator {
    /// Takes the decodes of one slot (`search`: a slot-search window of `period`).
    pub fn slot(&mut self, decodes: &[Decode], search: bool, period: f64) {
        if search && !decodes.is_empty() && self.pending.is_none() {
            // The true slot start of each decode: its window's nominal start plus its DT.
            let starts = decodes.iter().map(|d| d.slot + d.dt);
            let m = percentile(starts, 0.5, &mut self.values, &mut self.scratch);
            let shift = (m / period).round() * period - m;
            self.pending = Some(shift);
            return;
        }
        for d in decodes {
            self.recent.push_back((d.slot, d.dt));
        }
        let newest = self.recent.iter().map(|r| r.0).fold(f64::MIN, f64::max);
        while self.recent.front().is_some_and(|r| r.0 < newest - 60.0) {
            self.recent.pop_front();
        }
        while self.recent.len() > 2000 {
            self.recent.pop_front();
        }
    }
    /// Takes JS8 frames.
    pub fn js8(&mut self, frames: &[Js8Frame]) {
        for f in frames {
            self.js8.push_back(f.dt);
        }
        while self.js8.len() > 40 {
            self.js8.pop_front();
        }
    }
    /// The estimate now. The slot-search correction is returned once.
    pub fn report(&mut self) -> ClockReport {
        let (v, s) = (&mut self.values, &mut self.scratch);
        let dts = self.recent.iter().map(|r| r.1);
        let n = self.recent.len();
        let error_s = percentile(dts.clone(), 0.5, v, s);
        let spread_s = (percentile(dts.clone(), 0.84, v, s) - percentile(dts, 0.16, v, s)) / 2.0;
        ClockReport {
            error_s,
            spread_s,
            count: n,
            warning: n >= 2 && error_s.abs() > 1.0,
            js8_error_s: percentile(self.js8.iter().copied(), 0.5, v, s),
            correction_s: self.pending,
        }
    }
    /// Takes the slot-search correction (once).
    pub fn take_correction(&mut self) -> Option<f64> {
        self.pending.take()
    }
    /// Forgets the decodes (Clear).
    pub fn clear(&mut self) {
        self.recent.clear();
        self.js8.clear();
    }
}

/// An assembled JS8 message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Js8Message {
    /// Time of its first frame (slot UTC).
    pub time: f64,
    /// Sub-mode.
    pub submode: u8,
    /// Frequency (followed), Hz.
    pub hz: f64,
    /// SNR of the first frame.
    pub snr: i32,
    /// From.
    pub from: String,
    /// To.
    pub to: String,
    /// The command text.
    pub command: String,
    /// The text.
    pub text: String,
    /// Frames in it.
    pub frames: usize,
    /// Its last frame came.
    pub complete: bool,
    /// A relay.
    pub relay: bool,
    /// The relay's destination.
    pub relay_dest: String,
    #[serde(skip)]
    last_slot: f64,
    #[serde(skip)]
    open: bool,
}

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for Js8Message {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// A JS8 station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Js8Station {
    /// Call.
    pub call: String,
    /// Locator.
    pub grid: String,
    /// SNR of the newest frame.
    pub snr: i32,
    /// Frequency.
    pub hz: f64,
    /// Last heard (slot UTC).
    pub last: f64,
    /// Sub-mode of the newest frame.
    pub submode: u8,
    /// Frames heard.
    pub frames: u64,
    /// Text of the last frame.
    pub last_text: String,
    /// Sent a heartbeat.
    pub heartbeat: bool,
    /// Sent a CQ.
    pub cq: bool,
}

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for Js8Station {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// JS8 message assembly and the stations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Js8Assembler {
    /// The newest frames.
    pub frames: LogRing<Js8Frame>,
    /// The newest messages (open ones included).
    pub messages: LogRing<Js8Message>,
    /// Stations by call, the least recently heard leaving first.
    pub stations: ObjectTable<String, Js8Station>,
}

impl Default for Js8Assembler {
    fn default() -> Self {
        Js8Assembler {
            frames: LogRing::new(JS8_FRAMES_CAP),
            messages: LogRing::new(JS8_MESSAGES_CAP),
            stations: ObjectTable::new(JS8_STATIONS_CAP),
        }
    }
}

impl Js8Assembler {
    /// Takes the frames of one slot of one sub-mode; returns the messages completed or closed.
    pub fn slot(
        &mut self,
        submode: u8,
        slot_utc: f64,
        period: f64,
        frames: &[Js8Frame],
    ) -> Vec<Js8Message> {
        let mut done = Vec::new();
        let mut ordered: Vec<&Js8Frame> = frames.iter().collect();
        ordered.sort_by(|a, b| a.hz.total_cmp(&b.hz));
        let spacing = crate::mode::Sub::new(Mode::Js8)
            .with_sub(submode)
            .spacing_hz();
        for f in ordered {
            self.frames.push(f.clone());
            self.station(f);
            let beat = matches!(f.kind, Js8FrameKind::Heartbeat | Js8FrameKind::Cq);
            let first = f.flags & js8msg::FLAG_FIRST != 0 || beat;
            let last = f.flags & js8msg::FLAG_LAST != 0 || beat;
            // Continue an open message of the previous slot near this frequency.
            let cont = if first {
                None
            } else {
                self.messages.find_recent_mut(JS8_MESSAGES_CAP, |m| {
                    m.open
                        && m.submode == submode
                        && (m.last_slot - (slot_utc - period)).abs() < 0.01
                        && (m.hz - f.hz).abs() <= 2.0 * spacing
                })
            };
            match cont {
                Some(m) => {
                    m.text.push_str(&f.text);
                    m.hz = f.hz;
                    m.frames += 1;
                    m.last_slot = slot_utc;
                    if last {
                        m.open = false;
                        m.complete = true;
                        if m.relay {
                            m.relay_dest = relay_dest(&m.text);
                        }
                        done.push(m.clone());
                    }
                }
                None => {
                    let relay = f.kind == Js8FrameKind::Directed && f.cmd == ">";
                    let text = if first {
                        f.text.clone()
                    } else {
                        format!("... {}", f.text)
                    };
                    let mut m = Js8Message {
                        time: slot_utc,
                        submode,
                        hz: f.hz,
                        snr: f.snr,
                        from: f.from.clone(),
                        to: f.to.clone(),
                        command: f.cmd.clone(),
                        text,
                        frames: 1,
                        complete: false,
                        relay,
                        relay_dest: String::new(),
                        last_slot: slot_utc,
                        open: true,
                    };
                    if last && first {
                        m.open = false;
                        m.complete = true;
                        done.push(m.clone());
                    }
                    self.messages.push(m);
                }
            }
        }
        // Messages of this sub-mode not continued in this slot are closed.
        while let Some(m) = self.messages.find_recent_mut(JS8_MESSAGES_CAP, |m| {
            m.open && m.submode == submode && m.last_slot < slot_utc - 0.01
        }) {
            m.open = false;
            done.push(m.clone());
        }
        // A completed message with the GRID command and a locator sets the sender's grid.
        for m in &done {
            if m.complete && m.command == " GRID" {
                if let Some(g) = m.text.split_whitespace().rev().find(|w| {
                    crate::proto::msg77::is_grid4(&w[..w.len().min(4)])
                        && (w.len() == 4 || w.len() == 6)
                }) {
                    if let Some(s) = self.stations.get_mut(m.from.as_str()) {
                        s.grid = g.to_string();
                    }
                }
            }
        }
        done
    }

    fn station(&mut self, f: &Js8Frame) {
        if f.from.is_empty() || f.from.starts_with('@') || f.from.starts_with('<') {
            return;
        }
        let Ok(up) =
            self.stations
                .upsert(f.from.as_str(), Stamp::at(f.slot).with_wall(f.slot), || {
                    Js8Station {
                        call: f.from.clone(),
                        grid: String::new(),
                        snr: f.snr,
                        hz: f.hz,
                        last: f.slot,
                        submode: f.submode,
                        frames: 0,
                        last_text: String::new(),
                        heartbeat: false,
                        cq: false,
                    }
                })
        else {
            return;
        };
        let s = up.row;
        s.snr = f.snr;
        s.hz = f.hz;
        s.last = f.slot;
        s.submode = f.submode;
        s.frames += 1;
        s.last_text = f.text.clone();
        s.heartbeat |= f.kind == Js8FrameKind::Heartbeat;
        s.cq |= f.kind == Js8FrameKind::Cq;
        if !f.grid.is_empty() {
            s.grid = f.grid.clone();
        }
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.messages.clear();
        self.stations.clear();
    }
}

fn relay_dest(text: &str) -> String {
    text.split_once('>')
        .map(|(_, r)| r.split_whitespace().next().unwrap_or("").to_string())
        .unwrap_or_default()
}

// What the receiver publishes.

sdrkit_core::choice_enum! {
    /// What JS8 has heard.
    pub enum Js8State {
        /// Nothing yet.
        Listening = ("listening", "Listening"),
        /// Slots decoded, no frame within the last minute.
        Slots = ("slots", "Slots decoded"),
        /// Frames within the last minute.
        Frames = ("frames", "Frames"),
    }
}

sdrkit_core::choice_enum! {
    /// Whether a mode decodes.
    pub enum ModeState {
        /// It decodes.
        Decoding = ("decoding", "Decoding"),
        /// sdrkit has no decoder for its code (no mode of the family is in this state).
        NoDecoder = ("no_decoder", "No decoder"),
    }
}

/// The key of the JS8 waterfall series: (Hz, dB) of the newest line.
pub const WATERFALL_SERIES: &str = "js8_waterfall";

/// The declaration of the display, made once.
fn shared_declaration() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    Arc::clone(DECL.get_or_init(|| Arc::new(typed::display_declaration())))
}

/// The stations kept.
pub const STATIONS_CAP: usize = 1000;

/// A station heard in any mode, by callsign.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Callsign.
    pub call: String,
    /// Locator, empty when none was heard.
    pub grid: String,
    /// The mode of the newest decode.
    pub mode: Mode,
    /// SNR of the newest decode.
    pub snr: i32,
    /// Radio frequency of the newest decode, Hz.
    pub freq_hz: f64,
}

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for Station {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// A decode of WSPR or FST4W as a row of the spots table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WsprSpot(pub Decode);

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for WsprSpot {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the display keeps and saves (format `wsjt.heard`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// Decodes, oldest first.
    pub decodes: LogRing<Decode>,
    /// The WSPR and FST4W spots, oldest first.
    pub wspr: LogRing<WsprSpot>,
    /// The stations heard, by callsign.
    pub stations: ObjectTable<String, Station>,
    /// JS8 frames, messages and stations.
    pub js8: Js8Assembler,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            decodes: LogRing::new(DECODES_CAP),
            wspr: LogRing::new(DECODES_CAP),
            stations: ObjectTable::new(STATIONS_CAP),
            js8: Js8Assembler::default(),
        }
    }
}

impl Heard {
    /// Takes the station of a decode or a JS8 frame.
    fn station(&mut self, call: &str, grid: &str, mode: Mode, snr: i32, freq_hz: f64, utc: f64) {
        if call.is_empty() || call.starts_with('@') || call.starts_with('<') {
            return;
        }
        let Ok(up) = self
            .stations
            .upsert(call, Stamp::at(utc).with_wall(utc), || Station {
                call: call.to_owned(),
                grid: String::new(),
                mode,
                snr,
                freq_hz,
            })
        else {
            return;
        };
        let s = up.row;
        s.mode = mode;
        s.snr = snr;
        s.freq_hz = freq_hz;
        if !grid.is_empty() && s.grid != grid {
            grid.clone_into(&mut s.grid);
        }
    }
}

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for Decode {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// Its times are UTC (wall clock), so a restore moves none of them.
impl Shift for Js8Frame {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.decodes.shift_times(by_s);
        self.wspr.shift_times(by_s);
        self.stations.shift_times(by_s);
        self.js8.frames.shift_times(by_s);
        self.js8.messages.shift_times(by_s);
        self.js8.stations.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: Format = Format::new("wsjt.heard", 2);
    fn clear(&mut self) {
        self.decodes.clear();
        self.wspr.clear();
        self.stations.clear();
        self.js8.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.decodes.adopt(saved.decodes);
        self.wspr.adopt(saved.wspr);
        self.stations.adopt(saved.stations);
        self.js8.frames.adopt(saved.js8.frames);
        self.js8.messages.adopt(saved.js8.messages);
        self.js8.stations.adopt(saved.js8.stations);
    }
}

impl Shaped for Decode {
    type Row = DecodeRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&DecodeRow {
            utc: Time(self.slot),
            snr_db: f64::from(self.snr),
            dt_s: self.dt,
            audio_hz: Hertz(self.hz),
            band: self.band.as_deref(),
            dial_hz: (self.dial > 0.0).then_some(Hertz(self.dial)),
            mode: &self.label,
            message: &self.text,
            mirrored: self.mirrored,
            how: self.how,
            early: self.early,
        });
    }
}

impl Shaped for WsprSpot {
    type Row = WsprSpotRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let d = &self.0;
        out.put(&WsprSpotRow {
            utc: Time(d.slot),
            snr_db: f64::from(d.snr),
            dt_s: d.dt,
            audio_hz: Hertz(d.hz),
            callsign: &d.call,
            locator: &d.grid,
            power_dbm: i64::from(d.dbm),
            message: &d.text,
        });
    }
}

impl Shaped for Station {
    type Row = StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&StationRow {
            callsign: &self.call,
            locator: (!self.grid.is_empty()).then_some(self.grid.as_str()),
            mode: self.mode,
            snr_db: f64::from(self.snr),
            freq_hz: Hertz(self.freq_hz),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
    fn point(&self) -> Option<Point<'_>> {
        let (lat, lon) = locator_centre(&self.grid)?;
        Some(Point::new(
            sdrkit_util_geo::Position::new(lat, lon),
            self.call.as_str(),
        ))
    }
}

impl Shaped for Js8Frame {
    type Row = Js8FrameRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Js8FrameRow {
            utc: Time(self.slot),
            submode: Js8Speed::from_index(u16::from(self.submode)),
            snr_db: f64::from(self.snr),
            dt_s: self.dt,
            audio_hz: Hertz(self.hz),
            frame: &self.text,
        });
    }
}

impl Shaped for Js8Message {
    type Row = Js8MessageRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Js8MessageRow {
            utc: Time(self.time),
            snr_db: f64::from(self.snr),
            audio_hz: Hertz(self.hz),
            message: &self.text,
        });
    }
}

impl Shaped for Js8Station {
    type Row = Js8StationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&Js8StationRow {
            callsign: &self.call,
            locator: (!self.grid.is_empty()).then_some(self.grid.as_str()),
            snr_db: f64::from(self.snr),
            audio_hz: Hertz(self.hz),
            utc: Time(self.last),
            last_text: &self.last_text,
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// Per-mode counters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModeStats {
    /// Decodes since Clear.
    pub decodes: u64,
    /// Slots decoded.
    pub slots: u64,
    /// Decodes in the last slot.
    pub last_count: usize,
    /// The last slot's UTC.
    pub last_slot: f64,
    /// Decode time, ms.
    pub last_ms: f64,
    /// Sub-mode and period in use.
    pub label: String,
    /// Period, s.
    pub period: f64,
}

/// The display hub: the decode table, counters, clock, JS8 and the published results.
pub struct Display {
    /// What it keeps and saves.
    pub heard: Heard,
    /// The clock estimator.
    pub clock: ClockEstimator,
    /// Per mode (and per JS8 speed: `js8n`, `js8f`, `js8t`).
    pub modes: BTreeMap<String, ModeStats>,
    published: Published,
    total: u64,
    js8_frames: u64,
    js8_failed: u64,
    utc: f64,
    source: Option<TimeSource>,
    last_decode_utc: Option<f64>,
    last_js8_utc: Option<f64>,
    slots_seen: u64,
    /// Spot events are made (the user opted in).
    pub spots: bool,
    /// The bands that name the band of a decode's dial.
    pub bands: crate::bands::BandPlan,
    /// The waterfall line: (Hz, dB).
    pub waterfall: Vec<[f32; 2]>,
    /// Its sequence number.
    pub waterfall_seq: u64,
    waterfall_shown: u64,
    warned: bool,
    summary: String,
}

impl Default for Display {
    fn default() -> Self {
        Self::new()
    }
}

impl Display {
    /// `d` with the band of its dial.
    fn with_band(&self, d: &Decode) -> Decode {
        let mut d = d.clone();
        d.band = self.bands.band_name(d.dial).map(str::to_owned);
        d
    }

    /// An empty display.
    pub fn new() -> Display {
        let mut published = Published::from_decl(shared_declaration());
        published
            .results_mut()
            .set_status(&Status::new(LockState::Searching).with_summary("listening"));
        Display {
            heard: Heard::default(),
            clock: ClockEstimator::default(),
            modes: BTreeMap::new(),
            published,
            total: 0,
            js8_frames: 0,
            js8_failed: 0,
            utc: 0.0,
            source: None,
            last_decode_utc: None,
            last_js8_utc: None,
            slots_seen: 0,
            spots: false,
            bands: crate::bands::BandPlan::default(),
            waterfall: Vec::new(),
            waterfall_seq: 0,
            waterfall_shown: 0,
            warned: false,
            summary: String::new(),
        }
    }

    /// The published results.
    pub fn results(&self) -> &Results {
        self.published.results()
    }

    fn longest_period(&self) -> f64 {
        self.modes
            .values()
            .map(|m| m.period)
            .fold(0.0, f64::max)
            .max(6.0)
    }

    /// Takes the newest line of the waterfall spectrum: `db` in display order (0 Hz in the middle), `df` Hz apart. It keeps
    /// 200 to 3200 Hz.
    pub fn waterfall_line(&mut self, db: &[f32], df: f64) {
        let mid = db.len() / 2;
        let bin = |hz: f64| mid + (hz / df).round() as usize;
        let (b0, b1) = (bin(200.0), bin(3200.0).min(db.len().saturating_sub(1)));
        if b0 > b1 {
            return;
        }
        self.waterfall.clear();
        self.waterfall.extend(
            db[b0..=b1]
                .iter()
                .enumerate()
                .map(|(i, &v)| [((b0 + i - mid) as f64 * df) as f32, v]),
        );
        self.waterfall_seq += 1;
    }

    /// Takes a decoded slot and appends the events it makes to `ev`. The decodes
    /// of an early slot go to the decode table only, marked early, once each: no event, no count, no station, no spot. A decode
    /// of the whole slot replaces the early row of the same stack, slot and message.
    pub fn slot(&mut self, r: &crate::runner::SlotResult, ev: &mut Vec<Event>) {
        let Some(mode) = r.mode else { return };
        if r.early {
            for d in &r.decodes {
                let shown = self.heard.decodes.iter().rev().any(|x| {
                    x.early && x.mode == d.mode && x.slot == d.slot && x.text == d.text
                });
                if !shown {
                    let d = self.with_band(d);
                    self.heard.decodes.push(d);
                }
            }
            let line = format!(
                "{} {} early: {} decodes ({:.0} ms)",
                r.label,
                hms(r.utc),
                r.decodes.len(),
                r.ms
            );
            self.published.results_mut().log(line);
            self.publish();
            return;
        }
        self.slots_seen += 1;
        {
            let st = self.modes.entry(mode_key(mode, r.submode)).or_default();
            st.slots += 1;
            st.last_slot = r.utc;
            st.last_ms = r.ms;
            st.label.clone_from(&r.label);
            st.period = r.period;
        }
        if mode == Mode::Js8 {
            self.js8_slot(r, ev);
            return;
        }
        self.clock.slot(&r.decodes, r.search, r.period);
        if let Some(st) = self.modes.get_mut(mode.id()) {
            st.last_count = r.decodes.len();
            if !r.search {
                st.decodes += r.decodes.len() as u64;
            }
        }
        if !r.search {
            for d in &r.decodes {
                self.total += 1;
                self.last_decode_utc = Some(d.slot);
                ev.push(decode_event(d));
                if self.spots && !d.call.is_empty() {
                    ev.push(spot_event(
                        mode.name(),
                        d.slot,
                        d.dial + d.hz,
                        d.snr,
                        &d.call,
                        &d.grid,
                    ));
                }
                self.heard
                    .station(&d.call, &d.grid, mode, d.snr, d.dial + d.hz, d.slot);
                if matches!(d.mode, Mode::Wspr | Mode::Fst4w) {
                    self.heard.wspr.push(WsprSpot(d.clone()));
                }
                let d = self.with_band(d);
                let early = self.heard.decodes.find_recent_mut(DECODES_CAP, |x| {
                    x.early && x.mode == d.mode && x.slot == d.slot && x.text == d.text
                });
                match early {
                    Some(x) => *x = d,
                    None => {
                        self.heard.decodes.push(d);
                    }
                }
            }
        }
        let line = format!(
            "{} {} {} decodes ({:.0} ms)",
            r.label,
            hms(r.utc),
            r.decodes.len(),
            r.ms
        );
        self.published.results_mut().log(line);
        self.publish();
    }

    fn js8_slot(&mut self, r: &crate::runner::SlotResult, ev: &mut Vec<Event>) {
        let p = [15.0, 10.0, 6.0][r.submode.min(2) as usize];
        self.clock.js8(&r.frames);
        let msgs = self.heard.js8.slot(r.submode, r.utc, p, &r.frames);
        self.js8_frames += r.frames.len() as u64;
        self.js8_failed += r.failed;
        if !r.frames.is_empty() {
            self.last_js8_utc = Some(r.utc);
        }
        if let Some(st) = self.modes.get_mut(&mode_key(Mode::Js8, r.submode)) {
            st.last_count = r.frames.len();
            st.decodes += r.frames.len() as u64;
        }
        for f in &r.frames {
            let from = (!f.from.is_empty() && !f.from.starts_with('@')).then_some(f.from.as_str());
            ev.push(JS8_FRAME.event(
                r.utc,
                f.dial + f.hz,
                &Js8FrameEvent {
                    text: &f.text,
                    chars: &f.chars,
                    snr_db: f64::from(f.snr),
                    submode: Js8Speed::from_index(u16::from(f.submode)),
                    source: from,
                    target: (!f.to.is_empty()).then_some(f.to.as_str()),
                    family: Family::Message,
                },
            ));
            if let Some(from) = from {
                if self.spots {
                    ev.push(spot_event(
                        "JS8",
                        r.utc,
                        f.dial + f.hz,
                        f.snr,
                        from,
                        &f.grid,
                    ));
                }
                // The JS8 station's locator includes one received in a GRID message.
                let grid = match self.heard.js8.stations.get(from) {
                    Some(s) if f.grid.is_empty() => s.grid.clone(),
                    _ => f.grid.clone(),
                };
                self.heard
                    .station(from, &grid, Mode::Js8, f.snr, f.dial + f.hz, r.utc);
            }
        }
        let dial = r.frames.first().map_or(0.0, |f| f.dial);
        for m in &msgs {
            ev.push(JS8_MESSAGE.event(
                m.time,
                dial + m.hz,
                &Js8MessageEvent {
                    text: &m.text,
                    complete: m.complete,
                    source: (!m.from.is_empty()).then_some(m.from.as_str()),
                    target: (!m.to.is_empty()).then_some(m.to.as_str()),
                    family: Family::Message,
                },
            ));
        }
        if !r.frames.is_empty() {
            let line = format!(
                "{} {} {} frames ({:.0} ms)",
                r.label,
                hms(r.utc),
                r.frames.len(),
                r.ms
            );
            self.published.results_mut().log(line);
        }
        self.publish();
    }

    /// Refreshes the published results at `utc` (the newest audio), with the time source and the audio level.
    pub fn refresh(&mut self, utc: f64, source: TimeSource, level: Option<f32>) {
        self.utc = utc;
        self.source = Some(source);
        self.published.results_mut().update_status(|s| {
            let changed = s.level_dbfs != level;
            s.level_dbfs = level;
            changed
        });
        self.publish();
    }

    fn js8_state(&self) -> Js8State {
        let valid = self.last_js8_utc.is_some_and(|t| self.utc - t <= 60.0);
        let slots: u64 = ["js8n", "js8f", "js8t"]
            .iter()
            .filter_map(|k| self.modes.get(*k))
            .map(|s| s.slots)
            .sum();
        if valid {
            Js8State::Frames
        } else if slots > 0 {
            Js8State::Slots
        } else {
            Js8State::Listening
        }
    }

    /// The lock state and the summary of the status.
    fn status(&mut self, js8: Js8State) -> LockState {
        let lock_state = if self.slots_seen == 0 {
            LockState::Searching
        } else if self
            .last_decode_utc
            .is_some_and(|t| self.utc - t <= 3.0 * self.longest_period())
            || js8 == Js8State::Frames
        {
            LockState::Locked
        } else {
            LockState::Tracking
        };
        // Writing to a String cannot fail, so the results of `write!` below are dropped.
        use std::fmt::Write;
        let s = &mut self.summary;
        s.clear();
        let sep = |s: &mut String| {
            if !s.is_empty() {
                s.push_str("; ");
            }
        };
        for m in Mode::ALL {
            if let Some(st) = self.modes.get(m.id()).filter(|st| st.decodes > 0) {
                sep(s);
                let _ = write!(s, "{} {}", m.name(), st.last_count);
            }
        }
        if js8 == Js8State::Frames || self.js8_frames > 0 {
            sep(s);
            let _ = write!(
                s,
                "JS8 {} frames, {} stations",
                self.js8_frames,
                self.heard.js8.stations.len()
            );
        }
        if s.is_empty() {
            s.push_str("listening");
        }
        // The Q65 stack alone (not the combined stack, whose summary lists every mode).
        if self.modes.len() == 1 && self.modes.contains_key(Mode::Q65.id()) {
            sep(s);
            s.push_str(crate::rx::q65::UNCONFIRMED);
        }
        lock_state
    }

    fn publish(&mut self) {
        let rep = self.clock.report();
        if rep.warning && !self.warned {
            let line = format!(
                "clock off by {:+.1} s: set the computer clock (decodes still work to 2.5 s)",
                rep.error_s
            );
            self.published.results_mut().log(line);
        }
        self.warned = rep.warning;
        let utc = self.utc;
        let js8 = self.js8_state();
        let lock_state = self.status(js8);
        let snr = self
            .heard
            .decodes
            .newest_first()
            .next()
            .map(|d| d.snr as f32);
        let summary = &self.summary;
        let p = &mut self.published;
        let measured = rep.count > 0;
        p.put(&WsjtFields {
            time_source: self.source,
            utc: Time(utc),
            clock_error_s: measured.then_some(rep.error_s),
            clock_spread_s: measured.then_some(rep.spread_s),
            clock_decodes: rep.count as i64,
            clock_warning: rep.warning,
            decodes_total: self.total as i64,
            js8_frames: self.js8_frames as i64,
            js8_failed: self.js8_failed as i64,
            js8_state: js8,
            js8_waterfall_seq: (self.waterfall_seq > 0).then_some(self.waterfall_seq as i64),
        });
        p.results_mut().update_status(|s| {
            let changed = s.lock_state != lock_state || s.snr_db != snr || s.summary != *summary;
            if changed {
                s.lock_state = lock_state;
                s.snr_db = snr;
                s.summary.clone_from(summary);
            }
            changed
        });
        // The tables.
        shaped::log_rows(&self.heard.decodes, &DECODES, utc, p);
        shaped::log_rows(&self.heard.wspr, &WSPR_SPOTS, utc, p);
        for (k, m) in &self.modes {
            MODES.put(
                p,
                k,
                &ModeRow {
                    mode: &m.label,
                    period: Seconds(m.period),
                    slots: m.slots as i64,
                    decodes: m.decodes as i64,
                    last_count: m.last_count as i64,
                    utc: (m.slots > 0).then_some(Time(m.last_slot)),
                    decode_ms: m.last_ms,
                    state: ModeState::Decoding,
                },
            );
        }
        let heard = &self.heard;
        shaped::rows(&heard.stations, &STATIONS, utc, p);
        shaped::log_rows(&heard.js8.frames, &JS8_FRAMES, utc, p);
        shaped::log_rows(&heard.js8.messages, &JS8_MESSAGES, utc, p);
        shaped::rows(&heard.js8.stations, &JS8_STATIONS, utc, p);
        // The map: the stations with a locator, by call.
        shaped::points(&heard.stations, "", p.results_mut());
        if self.waterfall_seq != self.waterfall_shown {
            self.waterfall_shown = self.waterfall_seq;
            let line = &self.waterfall;
            let r = p.results_mut();
            if let Some(id) = r.decl().series_id(WATERFALL_SERIES) {
                r.update_series(id, |s| {
                    s.points.clear();
                    s.points.extend_from_slice(line);
                    s.line = true;
                    true
                });
            }
        }
    }

    /// Clear: the tables, counters, JS8 and the map. The time source and the slot clock stay.
    pub fn clear(&mut self) {
        Tables::clear(&mut self.heard);
        self.clock.clear();
        self.total = 0;
        self.js8_frames = 0;
        self.js8_failed = 0;
        for s in self.modes.values_mut() {
            s.decodes = 0;
            s.slots = 0;
            s.last_count = 0;
        }
        self.last_decode_utc = None;
        self.last_js8_utc = None;
        self.publish();
    }

    /// Forgets what belongs to the frequency that the host left: the clock estimate, the counters of the modes, the
    /// waterfall and the times of the last decodes, and each log, station list and total that `kept` does not name.
    pub fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        let h = &mut self.heard;
        if !kept.table(DECODES.key) {
            h.decodes.clear();
        }
        if !kept.table(WSPR_SPOTS.key) {
            h.wspr.clear();
        }
        if !kept.table(STATIONS.key) {
            h.stations.clear();
        }
        if !kept.table(JS8_FRAMES.key) {
            h.js8.frames.clear();
        }
        if !kept.table(JS8_MESSAGES.key) {
            h.js8.messages.clear();
        }
        if !kept.table(JS8_STATIONS.key) {
            h.js8.stations.clear();
        }
        if !kept.field("decodes_total") {
            self.total = 0;
        }
        if !kept.field("js8_frames") {
            self.js8_frames = 0;
        }
        if !kept.field("js8_failed") {
            self.js8_failed = 0;
        }
        self.clock.clear();
        for m in self.modes.values_mut() {
            m.decodes = 0;
            m.slots = 0;
            m.last_count = 0;
        }
        self.last_decode_utc = None;
        self.last_js8_utc = None;
        self.slots_seen = 0;
        if !self.waterfall.is_empty() {
            self.waterfall.clear();
            self.waterfall_seq += 1;
        }
        self.publish();
    }

    /// Forgets the station whose callsign is the argument `row`.
    ///
    /// # Errors
    ///
    /// [`CommandError::NoSuchRow`] if no station has that callsign.
    pub fn remove_station(&mut self, args: &Values) -> Result<(), CommandError> {
        let table = self
            .published
            .results()
            .decl()
            .table_id(STATIONS.key)
            .ok_or(CommandError::NotAnArgument { key: STATIONS.key })?;
        sdrkit_util_objects::commands::remove(
            &mut self.heard.stations,
            table,
            &mut self.published,
            args,
            &STATION_ROW,
        )?;
        self.publish();
        Ok(())
    }

    /// Replaces what is held with a saved state (see [`Tables::restore_state`]). The counters stay.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the state is of another owner or version, or damaged. Nothing changes then.
    pub fn restore(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.heard.restore_state(state, self.utc)?;
        self.publish();
        Ok(())
    }
}

/// The key of a row of the modes table: the mode id, or `js8n`, `js8f`, `js8t` for the JS8 speeds.
fn mode_key(mode: Mode, submode: u8) -> String {
    if mode == Mode::Js8 {
        format!("js8{}", ["n", "f", "t"][submode.min(2) as usize])
    } else {
        mode.id().to_string()
    }
}

/// The event of a decode.
pub fn decode_event(d: &Decode) -> Event {
    let call = (!d.call.is_empty()).then_some(d.call.as_str());
    let grid = (!d.grid.is_empty()).then_some(d.grid.as_str());
    DECODE.event(
        d.slot,
        d.dial + d.hz,
        &DecodeEvent {
            mode: d.mode,
            text: &d.text,
            snr_db: f64::from(d.snr),
            dt_s: d.dt,
            audio_hz: Hertz(d.hz),
            callsign: call,
            locator: grid,
            cq: d.cq,
            mirrored: d.mirrored,
            how: d.how,
            power_dbm: (d.dbm >= 0).then_some(i64::from(d.dbm)),
            source: call,
            position: locator_centre(&d.grid),
            family: Family::Message,
        },
    )
}

/// The contact lines of the map: from the decodes of the last `minutes` before `now`, each message that names two calls, as
/// (first call, second call), once per pair.
pub fn contact_lines<'a>(
    decodes: impl IntoIterator<Item = &'a Decode>,
    now: f64,
    minutes: f64,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for d in decodes {
        if d.slot < now - minutes * 60.0
            || d.text.starts_with("CQ ")
            || matches!(d.mode, Mode::Wspr | Mode::Fst4w)
        {
            continue;
        }
        let w: Vec<&str> = d.text.split_whitespace().collect();
        if w.len() < 2 {
            continue;
        }
        let a = w[0]
            .trim_matches(|c| c == '<' || c == '>')
            .trim_end_matches(';');
        let b = if w.len() >= 4 && w[1] == "RR73;" {
            w[3]
        } else {
            w[1]
        }
        .trim_matches(|c| c == '<' || c == '>');
        let call = |c: &str| {
            c.len() >= 3
                && c != "..."
                && c.chars().any(|x| x.is_ascii_digit())
                && crate::proto::msg77::is_plausible(c)
        };
        if call(a)
            && call(b)
            && !out
                .iter()
                .any(|(x, y)| (x == a && y == b) || (x == b && y == a))
        {
            out.push((a.to_string(), b.to_string()));
        }
    }
    out
}

/// A spot event: a station `call` at `grid` heard in `mode` (the emission type as spot networks name it) at `rf_hz`.
pub fn spot_event(mode: &str, utc: f64, rf_hz: f64, snr: i32, call: &str, grid: &str) -> Event {
    SPOT.event(
        utc,
        rf_hz,
        &SpotEvent {
            callsign: call,
            locator: (!grid.is_empty()).then_some(grid),
            snr_db: f64::from(snr),
            freq_hz: Hertz(rf_hz),
            mode,
            source: call,
            family: Family::Message,
        },
    )
}
