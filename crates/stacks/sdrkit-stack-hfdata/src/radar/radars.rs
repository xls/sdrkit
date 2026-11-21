//! Radars, the heard log, the chirp waterfall, the Morse ID and the scan requests of the ocean radar stack, and its results.

use super::bands::{band_of, hint, scan_plan, BANDS};
use super::detect::{Detection, Sweep};
use crate::typed::{
    HfradarBandArgs, HfradarBandRow, HfradarFields, HfradarHeardRow, HfradarRadarHeardEvent,
    HfradarRadarRow, HFRADAR_BANDS, HFRADAR_BAND_TUNE, HFRADAR_HEARD, HFRADAR_RADARS,
    HFRADAR_RADAR_HEARD,
};
use crate::util::{update_points, Origin};
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::media::{Codec, Picture, PixelFormat, PixelLayout};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    ConfigError, HostRequest, LockState, Record, Results, ResultsDecl, Setting, SettingError,
    SettingId, SettingSet, StageError, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RunContext, Timed,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use sdrkit_util_text::morse::{fit_dot, Decoder, DecoderConfig, Event as MorseEvent};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

const RADARS_CAP: usize = 64;
const EVENTS_CAP: usize = 300;
/// The scan help.
const SCAN_HELP: &str = "Step through the radar bands in parts that the sample rate takes in, and listen to each for the dwell time. A radar heard on one part is not heard on the others meanwhile. Off: one position.";

/// Waterfall rows kept (25 ms each, so 4 s).
pub const ROWS: usize = 160;
/// Waterfall columns over the used bins.
pub const COLS: usize = 256;
/// Seconds per waterfall row.
pub const ROW_S: f64 = 0.025;

sdrkit_core::choice_enum! {
    /// The direction of a sweep.
    #[derive(Serialize, Deserialize)]
    pub enum SweepDirection {
        /// The frequency rises.
        Rising = ("rising", "rising"),
        /// The frequency falls.
        Falling = ("falling", "falling"),
    }
}

/// The sweep marks: (signal time, offset Hz) of the counted sweeps' centres, newest last.
pub const MARKS_SERIES: &str = "sweep_marks";
/// The SNR history of the radar heard last: one point per second heard, oldest first.
pub const SNR_SERIES: &str = "snr_history";

/// What the ocean radar stack publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(HfradarFields::SHAPE.fields)
        .with_table(&HFRADAR_RADARS)
        .with_table(&HFRADAR_HEARD)
        .with_table(&HFRADAR_BANDS)
        .series(MARKS_SERIES, "Sweep marks", "s", "Hz")
        .series(SNR_SERIES, "SNR history", "s", "dB")
        .event(&HFRADAR_RADAR_HEARD)
}

/// One radar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Radar {
    /// Numbered 1, 2 ... in the order heard.
    pub number: u32,
    /// Centre: absolute Hz when the radio's frequency is known, else the offset.
    pub centre_hz: f64,
    /// Centre offset from the radio's centre, Hz.
    pub offset_hz: f64,
    /// Sweep width, Hz.
    pub width_hz: f64,
    /// Hz per second (positive).
    pub rate_hz_s: f64,
    /// +1 rising, -1 falling.
    pub dir: i8,
    /// Time between sweep starts, when known.
    pub repeat_s: Option<f64>,
    /// SNR of the last sweep, dB.
    pub snr_db: f64,
    /// Peak level of the last sweep, dBFS.
    pub level_dbfs: f64,
    /// Sweeps counted.
    pub sweeps: u32,
    /// Signal time of the first sweep.
    pub first_t: f64,
    /// Signal time of the last sweep.
    pub last_t: f64,
    /// The band name, if it is in a radar band.
    pub band: Option<String>,
    /// The hint.
    pub hint: String,
    /// The keyed callsign.
    pub id: Option<String>,
    /// Its speed, wpm.
    pub id_wpm: f64,
    /// Times it was read.
    pub id_seen: u32,
    /// Signal time of the last reading.
    pub id_time: f64,
    /// SNR per second heard, dB (120 points).
    pub snr_history: Vec<f32>,
    #[serde(skip)]
    starts: VecDeque<f64>,
    #[serde(skip)]
    sec: Option<(i64, f64, u32)>,
    #[serde(skip)]
    power: VecDeque<f32>,
    #[serde(skip)]
    last_candidate: Option<String>,
    #[serde(skip)]
    seen_until: f64,
    #[serde(skip, default = "never")]
    last_event_t: f64,
}

fn never() -> f64 {
    f64::NEG_INFINITY
}

impl Radar {
    fn lo(&self) -> f64 {
        self.offset_hz - self.width_hz / 2.0
    }
    fn hi(&self) -> f64 {
        self.offset_hz + self.width_hz / 2.0
    }
    fn direction(&self) -> SweepDirection {
        if self.dir > 0 {
            SweepDirection::Rising
        } else {
            SweepDirection::Falling
        }
    }
    fn row(&self, origin: &Origin) -> HfradarRadarRow<'_> {
        HfradarRadarRow {
            number: i64::from(self.number),
            centre: Hertz(self.centre_hz),
            offset_hz: Hertz(self.offset_hz),
            band: self.band.as_deref(),
            width: Hertz(self.width_hz),
            rate_hz_s: self.rate_hz_s,
            direction: self.direction(),
            repeat: self.repeat_s.map(Seconds),
            snr_db: self.snr_db,
            level_dbfs: self.level_dbfs,
            sweeps: i64::from(self.sweeps),
            first: Seconds(self.first_t),
            last: Seconds(self.last_t),
            first_heard: origin.wall(self.first_t).map(Time),
            last_heard: origin.wall(self.last_t).map(f64::floor).map(Time),
            id: self.id.as_deref(),
            id_wpm: self.id.as_ref().map(|_| self.id_wpm),
            id_seen: i64::from(self.id_seen),
            hint: &self.hint,
        }
    }
}

impl Shift for Radar {
    fn shift_times(&mut self, by_s: f64) {
        self.first_t += by_s;
        self.last_t += by_s;
        self.id_time += by_s;
    }
}

/// One heard-log entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HeardEvent {
    /// Signal time, seconds.
    pub t: f64,
    /// UTC, when the clock is known.
    pub utc: Option<f64>,
    /// The radar number.
    pub radar: u32,
    /// Its frequency: absolute when the radio's frequency is known, else the offset.
    pub freq_hz: f64,
    /// The band name, if it is in a radar band.
    pub band: Option<String>,
    /// SNR, dB.
    pub snr_db: f64,
}

impl Shift for HeardEvent {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for HeardEvent {
    type Row = HfradarHeardRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&HfradarHeardRow {
            start: Seconds(self.t),
            time: self.utc.map(Time),
            number: i64::from(self.radar),
            frequency: Hertz(self.freq_hz),
            band: self.band.as_deref(),
            snr_db: self.snr_db,
        });
    }
}

/// What the stack keeps between runs: the radars and the heard log. Restored radars keep their numbers; new ones continue after the
/// highest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RadarLists {
    /// The radars, in the order heard.
    pub radars: Vec<Radar>,
    /// The heard log, oldest first.
    pub heard: LogRing<HeardEvent>,
    /// The number the next new radar takes.
    pub next_number: u32,
}

impl Default for RadarLists {
    fn default() -> Self {
        RadarLists {
            radars: Vec::new(),
            heard: LogRing::new(EVENTS_CAP),
            next_number: 1,
        }
    }
}

impl Shift for RadarLists {
    fn shift_times(&mut self, by_s: f64) {
        for r in &mut self.radars {
            r.shift_times(by_s);
        }
        self.heard.shift_times(by_s);
    }
}

impl Tables for RadarLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.hfradar_lists", 1);
    fn clear(&mut self) {
        self.radars.clear();
        self.heard.clear();
        self.next_number = 1;
    }
    fn adopt(&mut self, saved: Self) {
        let highest = saved.radars.iter().map(|r| r.number).max().unwrap_or(0);
        self.next_number = saved.next_number.max(highest + 1);
        self.radars = saved.radars.into_iter().take(RADARS_CAP).collect();
        self.heard.adopt(saved.heard);
    }
}

/// A keying segment: start, end, runs.
type Segment = (f64, f64, Vec<(bool, f64)>);

/// The scan of the radar bands.
pub const SCAN: Setting<field::Bool> =
    Setting::new("scan", "Scan the radar bands", "", false).help(SCAN_HELP);
/// The time on each stop of the scan.
pub const DWELL: Setting<field::Duration> =
    Setting::new("dwell_s", "Dwell", "s", 20.0).range(5.0, 300.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`RadarTable`].
    pub enum RadarSetting {
        /// Scan the radar bands.
        Scan = SCAN,
        /// Dwell.
        Dwell = DWELL,
    }
}

sdrkit_core::commands! {
    /// The commands of [`RadarTable`].
    pub enum RadarCommand {
        /// Forget the radars, the heard log and the sweep marks.
        Clear = objcmd::CLEAR,
        /// Tune to the middle of a radar band.
        BandTune = HFRADAR_BAND_TUNE.command,
    }
}

/// `hfdata.hfradar_radars`: detections and the 5 ms excess frames in. `hfradar.radar_heard` events, the waterfall picture (grey
/// pixels, 256 columns) and the scan requests out.
pub struct RadarTable {
    p: Published,
    series: [SeriesId; 2],
    rows_shown: Vec<u32>,
    origin: Origin,
    centre_hz: f64,
    tuned_hz: f64,
    known: bool,
    lists: RadarLists,
    sweeps_ok: u64,
    tracks_bad: u64,
    bursts: u64,
    last_sweep_end: f64,
    recent_snr: VecDeque<(f64, f64)>,
    // Excess frames: 200 per second.
    frame_t: u64,
    bins: usize,
    first_bin_hz: f64,
    bin_hz: f64,
    rate: f64,
    row_acc: Vec<f32>,
    row_n: u32,
    rows: VecDeque<Vec<u8>>,
    rows_changed: bool,
    marks: VecDeque<(f64, f64)>,
    next_id: f64,
    now: f64,
    log_heard: Vec<u32>,
    scan: bool,
    dwell_s: f64,
    live: bool,
    configured: bool,
    hopping: bool,
    first_request: bool,
    requests: Vec<HostRequest>,
    /// Input power since the last snapshot: the sum of |x|^2 and the samples.
    power: (f64, u64),
    level_dbfs: Option<f64>,
    // Buffers of the snapshot.
    order: Vec<usize>,
    numbers: Vec<u32>,
    summary: String,
    key: String,
    pixels: Option<Arc<[u8]>>,
}

impl Default for RadarTable {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let id = |k| decl.series_id(k).expect("the declaration holds the series");
        let series = [id(MARKS_SERIES), id(SNR_SERIES)];
        let mut t = RadarTable {
            p: Published::from_decl(decl),
            series,
            rows_shown: Vec::new(),
            origin: Origin::default(),
            centre_hz: 0.0,
            tuned_hz: 0.0,
            known: false,
            lists: RadarLists::default(),
            sweeps_ok: 0,
            tracks_bad: 0,
            bursts: 0,
            last_sweep_end: f64::NEG_INFINITY,
            recent_snr: VecDeque::new(),
            frame_t: 0,
            bins: 0,
            first_bin_hz: 0.0,
            bin_hz: 1.0,
            rate: 1.0,
            row_acc: vec![0.0; COLS],
            row_n: 0,
            rows: VecDeque::new(),
            rows_changed: false,
            marks: VecDeque::new(),
            next_id: 12.0,
            now: 0.0,
            log_heard: Vec::new(),
            scan: false,
            dwell_s: 20.0,
            live: false,
            configured: false,
            hopping: false,
            first_request: false,
            requests: Vec::new(),
            power: (0.0, 0),
            level_dbfs: None,
            order: Vec::new(),
            numbers: Vec::new(),
            summary: String::new(),
            key: String::new(),
            pixels: None,
        };
        for b in BANDS {
            HFRADAR_BANDS.put(&mut t.p, b.0, &band_row(b));
        }
        t
    }
}

/// `band.tune` while the scan is on: the scan owns the frequency.
#[derive(Debug)]
struct ScanOn;

impl std::fmt::Display for ScanOn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the scan is on: turn it off to choose a band")
    }
}

impl std::error::Error for ScanOn {}

/// The `bands` row of band `b`.
fn band_row(b: &(&'static str, f64, f64, &'static str)) -> HfradarBandRow<'static> {
    HfradarBandRow {
        name: b.0,
        low: Hertz(b.1 * 1e6),
        high: Hertz(b.2 * 1e6),
        middle: Hertz(band_middle(b)),
        range_class: b.3,
    }
}

/// The middle of band `b`, Hz, rounded to kHz.
pub fn band_middle(b: &(&'static str, f64, f64, &'static str)) -> f64 {
    ((b.1 + b.2) / 2.0 * 1e3).round() * 1e3
}

impl RadarTable {
    /// What the stack keeps: the radars and the heard log.
    pub fn lists(&self) -> &RadarLists {
        &self.lists
    }

    /// Takes one sweep.
    pub fn sweep(&mut self, s: &Sweep) {
        self.sweeps_ok += 1;
        let end = s.start + s.duration;
        self.last_sweep_end = self.last_sweep_end.max(end);
        self.recent_snr.push_back((end, s.snr_db));
        let dir: i8 = if s.rate >= 0.0 { 1 } else { -1 };
        let centre = s.centre;
        let width = s.width;
        let rate = s.rate.abs();
        if self.marks.len() >= 200 {
            self.marks.pop_front();
        }
        self.marks.push_back((end, centre));
        let radars = &mut self.lists.radars;
        let joined = radars.iter().position(|r| {
            r.dir == dir
                && (r.offset_hz - centre).abs() <= (4e3f64).max(0.15 * r.width_hz)
                && (r.width_hz - width).abs() <= (3e3f64).max(0.25 * r.width_hz)
        });
        let i = match joined {
            Some(i) => i,
            None => {
                // A fragment of a known radar (its ends faded).
                let frag = radars.iter().position(|r| {
                    r.dir == dir
                        && centre - width / 2.0 >= r.lo() - 3e3
                        && centre + width / 2.0 <= r.hi() + 3e3
                        && (rate - r.rate_hz_s).abs() <= 0.2 * r.rate_hz_s
                });
                if let Some(f) = frag {
                    let r = &mut radars[f];
                    r.last_t = end;
                    r.level_dbfs = s.level_dbfs;
                    return;
                }
                if radars.len() >= RADARS_CAP {
                    // The radar heard longest ago makes room.
                    if let Some((k, _)) = radars
                        .iter()
                        .enumerate()
                        .min_by(|a, b| a.1.last_t.total_cmp(&b.1.last_t))
                    {
                        radars.remove(k);
                    }
                }
                let n = self.lists.next_number;
                self.lists.next_number += 1;
                let centre_hz = if self.known {
                    self.centre_hz + centre
                } else {
                    centre
                };
                self.lists.radars.push(Radar {
                    number: n,
                    centre_hz,
                    offset_hz: centre,
                    width_hz: width,
                    rate_hz_s: rate,
                    dir,
                    repeat_s: None,
                    snr_db: s.snr_db,
                    level_dbfs: s.level_dbfs,
                    sweeps: 0,
                    first_t: s.start,
                    last_t: end,
                    band: None,
                    hint: String::new(),
                    id: None,
                    id_wpm: 0.0,
                    id_seen: 0,
                    id_time: 0.0,
                    snr_history: Vec::new(),
                    starts: VecDeque::new(),
                    sec: None,
                    power: VecDeque::new(),
                    last_candidate: None,
                    seen_until: 0.0,
                    last_event_t: f64::NEG_INFINITY,
                });
                self.lists.radars.len() - 1
            }
        };
        let known = self.known;
        let cfreq = self.centre_hz;
        let r = &mut self.lists.radars[i];
        let w = if r.sweeps < 3 { 0.5 } else { 0.15 };
        if r.sweeps > 0 {
            r.offset_hz += w * (centre - r.offset_hz);
            r.width_hz += w * (width - r.width_hz);
            r.rate_hz_s += w * (rate - r.rate_hz_s);
        }
        r.centre_hz = if known {
            cfreq + r.offset_hz
        } else {
            r.offset_hz
        };
        r.sweeps += 1;
        r.snr_db = s.snr_db;
        r.level_dbfs = s.level_dbfs;
        let was = r.last_t;
        r.last_t = end;
        if r.starts.len() >= 10 {
            r.starts.pop_front();
        }
        r.starts.push_back(s.start);
        // Repetition.
        let iv: Vec<f64> = r
            .starts
            .iter()
            .zip(r.starts.iter().skip(1))
            .map(|(a, b)| b - a)
            .filter(|d| (0.05..=10.0).contains(d))
            .collect();
        if iv.len() >= 3 {
            let med = quantile_f64(&iv, 0.5, &mut Vec::new()).unwrap_or(0.0);
            let near: Vec<f64> = iv
                .iter()
                .copied()
                .filter(|d| (d - med).abs() <= 0.1 * med)
                .collect();
            if near.len() >= 3 {
                r.repeat_s = Some(near.iter().sum::<f64>() / near.len() as f64);
            }
        }
        let rf = cfreq + r.offset_hz;
        r.band = band_of(rf).map(str::to_owned);
        r.hint = hint(rf, r.width_hz, r.repeat_s);
        // SNR history: one point per second heard.
        let sec = end.floor() as i64;
        match r.sec {
            Some((k, sum, n)) if k == sec => r.sec = Some((k, sum + s.snr_db, n + 1)),
            Some((_, sum, n)) => {
                if r.snr_history.len() >= 120 {
                    r.snr_history.remove(0);
                }
                r.snr_history.push((sum / n as f64) as f32);
                r.sec = Some((sec, s.snr_db, 1));
            }
            None => r.sec = Some((sec, s.snr_db, 1)),
        }
        // Heard first, or again after more than 60 s.
        if r.sweeps >= 2 && (r.last_event_t == f64::NEG_INFINITY || end - was > 60.0) {
            r.last_event_t = end;
            let ev = HeardEvent {
                t: end,
                utc: self.origin.wall(end),
                radar: r.number,
                freq_hz: r.centre_hz,
                band: r.band.clone(),
                snr_db: r.snr_db,
            };
            let line = format!(
                "CODAR: radar {} heard, {:.4} MHz, {:.0} kHz sweeps, {}",
                r.number,
                rf / 1e6,
                r.width_hz / 1e3,
                if r.dir > 0 { "rising" } else { "falling" }
            );
            self.log_heard.push(r.number);
            self.lists.heard.push(ev);
            self.p.results_mut().log(line);
        }
        self.absorb();
    }

    /// Radars of at most 3 sweeps inside a stronger radar's span at its rate (+-30 %) and direction are absorbed.
    fn absorb(&mut self) {
        let radars = &mut self.lists.radars;
        let mut gone = Vec::new();
        for (i, a) in radars.iter().enumerate() {
            if a.sweeps > 3 {
                continue;
            }
            if radars.iter().enumerate().any(|(j, b)| {
                j != i
                    && b.sweeps > a.sweeps
                    && b.dir == a.dir
                    && a.lo() >= b.lo() - 3e3
                    && a.hi() <= b.hi() + 3e3
                    && (a.rate_hz_s - b.rate_hz_s).abs() <= 0.3 * b.rate_hz_s
            }) {
                gone.push(i);
            }
        }
        for i in gone.into_iter().rev() {
            radars.remove(i);
        }
    }

    /// Takes the excess frames (5 ms each).
    fn frames(&mut self, values: &[f32]) {
        if self.bins == 0 {
            return;
        }
        let nf = values.len() / self.bins;
        for k in 0..nf {
            let f = &values[k * self.bins..(k + 1) * self.bins];
            self.frame_t += 1;
            let t = self.frame_t as f64 * 0.005;
            // Waterfall columns: 256 over the used bins, the maximum in a column.
            for c in 0..COLS {
                let a = c * self.bins / COLS;
                let b = ((c + 1) * self.bins / COLS).max(a + 1);
                let m = f[a..b].iter().copied().fold(0f32, f32::max);
                self.row_acc[c] = self.row_acc[c].max(m);
            }
            self.row_n += 1;
            if self.row_n == 5 {
                let row: Vec<u8> = self
                    .row_acc
                    .iter()
                    .map(|&v| ((10.0 * v.max(1e-3).log10()) / 30.0 * 255.0).clamp(0.0, 255.0) as u8)
                    .collect();
                if self.rows.len() >= ROWS {
                    self.rows.pop_front();
                }
                self.rows.push_back(row);
                self.rows_changed = true;
                self.row_acc.iter_mut().for_each(|v| *v = 0.0);
                self.row_n = 0;
            }
            // Band power of each radar with 2 sweeps or more (at most 16, skipping those silent for 10 minutes).
            let mut active = 0;
            for r in self.lists.radars.iter_mut() {
                if r.sweeps < 2 || t - r.last_t > 600.0 || active >= 16 {
                    continue;
                }
                active += 1;
                let lo = ((r.lo() - 2e3 - self.first_bin_hz) / self.bin_hz)
                    .floor()
                    .max(0.0) as usize;
                let hi = (((r.hi() + 2e3 - self.first_bin_hz) / self.bin_hz).ceil() as usize)
                    .min(self.bins);
                if hi <= lo {
                    continue;
                }
                let m: f32 = f[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
                if r.power.len() >= 8000 {
                    r.power.pop_front();
                }
                r.power.push_back(10.0 * m.max(1e-3).log10());
            }
        }
        self.now = self.frame_t as f64 * 0.005;
        if self.now + 0.011 >= self.next_id {
            // Every second of signal, so a keyed segment is read soon after the silence that ends it.
            self.next_id = self.now.floor() + 1.0;
            self.id_pass();
        }
    }

    /// The Morse ID: every second, the newest 30 s of each radar's band power (once 12 s exist). Each segment is read once.
    fn id_pass(&mut self) {
        let now = self.now;
        let mut lines = Vec::new();
        for r in self.lists.radars.iter_mut() {
            if r.power.len() < 2400 {
                continue;
            }
            let n = r.power.len().min(6000);
            let p: Vec<f32> = r.power.iter().skip(r.power.len() - n).copied().collect();
            let s: Vec<f64> = p.iter().map(|&v| v as f64).collect();
            let mut sc = Vec::new();
            let p10 = quantile_f64(&s, 0.1, &mut sc).unwrap_or(0.0);
            let p90 = quantile_f64(&s, 0.9, &mut sc).unwrap_or(0.0);
            let range = p90 - p10;
            if range < 2.5 {
                continue;
            }
            let (lo, hi) = (p10 + 0.4 * range, p10 + 0.6 * range);
            let mut runs: Vec<(bool, f64)> = Vec::new();
            let mut on = false;
            for &v in &p {
                let v = v as f64;
                if on && v < lo {
                    on = false;
                } else if !on && v > hi {
                    on = true;
                }
                match runs.last_mut() {
                    Some(l) if l.0 == on => l.1 += 0.005,
                    _ => runs.push((on, 0.005)),
                }
            }
            // Runs shorter than 15 ms join their neighbours.
            let mut merged: Vec<(bool, f64)> = Vec::new();
            for &(o, d) in &runs {
                if d < 0.015 {
                    if let Some(l) = merged.last_mut() {
                        l.1 += d;
                        continue;
                    }
                }
                match merged.last_mut() {
                    Some(l) if l.0 == o => l.1 += d,
                    _ => merged.push((o, d)),
                }
            }
            // Segments between silences of 0.9 s or more. A segment still running at the end is not complete.
            let t0 = now - n as f64 * 0.005;
            let mut t = t0;
            let mut seg: Vec<(bool, f64)> = Vec::new();
            let mut seg_start = t0;
            let mut segments: Vec<Segment> = Vec::new();
            for &(o, d) in &merged {
                if !o && d >= 0.9 {
                    if !seg.is_empty() {
                        segments.push((seg_start, t, std::mem::take(&mut seg)));
                    }
                    seg_start = t + d;
                } else {
                    seg.push((o, d));
                }
                t += d;
            }
            for (a, b, sg) in segments {
                if sg.len() < 12 || a < r.seen_until - 1e-6 {
                    continue;
                }
                let Some(dot) = fit_dot(&sg, 0.1) else {
                    continue;
                };
                if !(0.03..=0.3).contains(&dot) {
                    continue;
                }
                r.seen_until = b;
                let text = read_morse(&sg, dot);
                let word = text.split_whitespace().find(|w| {
                    (4..=10).contains(&w.len()) && w.chars().all(|c| c.is_ascii_alphanumeric()) && {
                        let mut cs: Vec<char> = w.chars().collect();
                        cs.sort();
                        cs.dedup();
                        cs.len() >= 3
                    }
                });
                let Some(word) = word else {
                    continue;
                };
                if r.last_candidate.as_deref() == Some(word) || r.id.as_deref() == Some(word) {
                    if r.id.as_deref() != Some(word) {
                        lines.push(format!("CODAR: radar {} ID {word} (unverified)", r.number));
                        // The two sightings that made it.
                        r.id_seen = 1;
                    }
                    r.id = Some(word.to_string());
                    r.id_wpm = 1.2 / dot;
                    r.id_seen += 1;
                    r.id_time = b;
                }
                r.last_candidate = Some(word.to_string());
            }
        }
        for l in lines {
            self.p.results_mut().log(l);
        }
    }

    /// The radio moved: the radars and the log stay. Their offsets move to the new centre, so a radar keeps its frequency; their
    /// band power belongs to the old part of the band.
    fn move_centre(&mut self, centre_hz: f64, tuned_hz: f64) {
        let shift = if self.known && centre_hz > 0.0 {
            self.centre_hz - centre_hz
        } else {
            0.0
        };
        self.centre_hz = centre_hz;
        self.tuned_hz = tuned_hz;
        self.known = centre_hz > 0.0;
        for r in self.lists.radars.iter_mut() {
            r.offset_hz += shift;
            r.power.clear();
        }
    }

    /// The scan request that a setting requires, once the receiver runs from a radio.
    fn scan_request(&mut self) {
        if !(self.configured && self.live) {
            return;
        }
        if self.scan {
            let plan = scan_plan(self.rate);
            if !plan.is_empty() {
                self.requests.push(HostRequest::Hop {
                    channels: plan,
                    dwell_s: self.dwell_s,
                    stop_on_lock: false,
                });
                self.hopping = true;
            }
        } else if self.hopping {
            // Stay where the scan is now.
            self.requests
                .push(HostRequest::retune(self.tuned_hz, "ocean radar scan off"));
            self.hopping = false;
        }
    }

    fn publish(&mut self) {
        let locked = self.now - self.last_sweep_end <= 5.0;
        while self
            .recent_snr
            .front()
            .is_some_and(|x| self.now - x.0 > 3.0)
        {
            self.recent_snr.pop_front();
        }
        let best = self
            .recent_snr
            .iter()
            .map(|x| x.1)
            .fold(f64::NEG_INFINITY, f64::max);
        if self.power.1 > 0 {
            let mean = self.power.0 / self.power.1 as f64;
            self.level_dbfs = Some(10.0 * mean.max(1e-30).log10());
            self.power = (0.0, 0);
        }
        let band_now = band_of(self.centre_hz);
        let stops = self.hopping.then(|| scan_plan(self.rate).len());
        // The radars with 2 sweeps or more, the one heard last first. Buffers are kept: a snapshot allocates nothing.
        let radars = &self.lists.radars;
        self.order.clear();
        self.order
            .extend((0..radars.len()).filter(|&i| radars[i].sweeps >= 2));
        self.order.sort_unstable_by(|&a, &b| {
            radars[b]
                .last_t
                .total_cmp(&radars[a].last_t)
                .then(radars[a].number.cmp(&radars[b].number))
        });
        let mut summary = std::mem::take(&mut self.summary);
        summary.clear();
        let _ = match self.order.first().map(|&i| &radars[i]) {
            None if self.sweeps_ok == 0 && self.known => match band_now {
                Some(b) => write!(
                    summary,
                    "no radar sweep heard yet (the radio is in the {b} band)"
                ),
                None => write!(
                    summary,
                    "no radar sweep heard yet (the radio is outside the radar bands)"
                ),
            },
            None if self.known => write!(
                summary,
                "listening on {:.3} MHz, {}",
                self.centre_hz / 1e6,
                band_now.unwrap_or("outside the radar bands")
            ),
            None => write!(summary, "listening for ocean radar sweeps"),
            Some(r) => {
                let _ = write!(
                    summary,
                    "{} radars, {:.0} kHz sweeps",
                    self.order.len(),
                    r.width_hz / 1e3
                );
                match &r.id {
                    Some(id) => write!(summary, ", ID {id}"),
                    None => Ok(()),
                }
            }
        };
        if let Some(n) = stops {
            let _ = write!(summary, ", stepping {n} stops, {:.0} s each", self.dwell_s);
        }
        let count = self.order.len();
        self.numbers.clear();
        self.numbers
            .extend(self.order.iter().map(|&i| radars[i].number));
        let p = &mut self.p;
        p.put(&HfradarFields {
            band: band_now,
            radar_count: count as i64,
            sweeps_counted: self.sweeps_ok as i64,
            tracks_rejected: self.tracks_bad as i64,
            broadband_bursts: self.bursts as i64,
            snr_db: best.is_finite().then_some(best),
            scan_stops: stops.map(|n| n as i64),
            waterfall_row: Seconds(ROW_S),
            waterfall_newest: Seconds(self.now),
            clock: self.origin.source(),
        });
        // The table lists the newest row first: write the radar heard last at the end.
        let key = &mut self.key;
        for &i in self.order.iter().rev() {
            let r = &radars[i];
            key.clear();
            let _ = write!(key, "{}", r.number);
            HFRADAR_RADARS.put(p, key, &r.row(&self.origin));
        }
        for n in &self.rows_shown {
            if !self.numbers.contains(n) {
                key.clear();
                let _ = write!(key, "{n}");
                p.remove_row(&HFRADAR_RADARS, key);
            }
        }
        shaped::log_rows(&self.lists.heard, &HFRADAR_HEARD, self.now, p);
        let r = p.results_mut();
        let status = Status {
            lock_state: if locked {
                LockState::Locked
            } else {
                LockState::Searching
            },
            snr_db: best.is_finite().then_some(best as f32),
            level_dbfs: self.level_dbfs.map(|v| v as f32),
            offset_hz: None,
            summary,
        };
        r.set_status(&status);
        self.summary = status.summary;
        let [marks, snr] = self.series;
        update_points(
            r,
            marks,
            false,
            self.marks.iter().map(|&(t, f)| [t as f32, f as f32]),
        );
        let history = self
            .order
            .first()
            .map_or(&[][..], |&i| &radars[i].snr_history[..]);
        let n = history.len();
        update_points(
            r,
            snr,
            true,
            history
                .iter()
                .enumerate()
                .map(|(i, &v)| [i as f32 - n as f32, v]),
        );
        std::mem::swap(&mut self.rows_shown, &mut self.numbers);
    }

    /// The waterfall picture, when a row was added since the last one. The pixel buffer is reused when no reader holds
    /// the last picture.
    fn picture(&mut self) -> Option<Picture> {
        if !self.rows_changed || self.rows.is_empty() {
            return None;
        }
        self.rows_changed = false;
        let len = self.rows.len() * COLS;
        let reuse = self
            .pixels
            .as_mut()
            .filter(|a| a.len() == len)
            .and_then(Arc::get_mut);
        match reuse {
            Some(buf) => {
                for (dst, row) in buf.as_chunks_mut::<COLS>().0.iter_mut().zip(&self.rows) {
                    dst.copy_from_slice(row);
                }
            }
            None => {
                self.pixels = Some(self.rows.iter().flatten().copied().collect());
            }
        }
        let data = Arc::clone(self.pixels.as_ref()?);
        Some(Picture {
            codec: Codec::Pixels(PixelFormat::Grey8),
            layout: Some(PixelLayout::packed(
                PixelFormat::Grey8,
                COLS as u32,
                self.rows.len() as u32,
            )),
            name: None,
            data,
        })
    }
}

/// Reads keying runs as Morse text with the shared decoder, starting at the dot length `dot` (s).
fn read_morse(runs: &[(bool, f64)], dot: f64) -> String {
    let Ok(mut d) = Decoder::new(DecoderConfig::fixed((1.2 / dot).clamp(1.0, 200.0))) else {
        return String::new();
    };
    let mut text = String::new();
    let mut put = |e: MorseEvent| match e {
        MorseEvent::Char(c) => text.push_str(c.text_or("?")),
        MorseEvent::Space => text.push(' '),
    };
    for &(on, s) in runs {
        d.push(on, s, &mut put);
    }
    d.flush(&mut put);
    text
}

impl Component for RadarTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.hfradar_radars",
            vec![
                Port::typed::<Detection>("sweeps"),
                Port::new("excess", Kind::Spectrum),
            ],
            vec![
                Port::new("events", Kind::Messages),
                Port::typed::<Picture>("waterfall"),
                Port::typed::<HostRequest>("requests"),
            ],
        )
        .with_settings(RadarSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let e = &inputs[1];
        self.origin = Origin::new(self.live);
        self.move_centre(f.centre_hz, f.tuned_hz);
        self.rate = f.bandwidth_hz;
        self.bins = e.frame_len;
        self.bin_hz = if e.frame_len > 0 {
            e.bandwidth_hz / e.frame_len as f64
        } else {
            1.0
        };
        self.first_bin_hz = -((self.bins / 2) as f64) * self.bin_hz;
        self.rows.clear();
        self.row_n = 0;
        self.frame_t = 0;
        self.next_id = 12.0;
        if !self.configured {
            // The first request waits for the first samples: the run context (live or not) is known by then.
            self.configured = true;
            self.first_request = true;
        }
        self.publish();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<Picture>(), 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.first_request) {
            self.scan_request();
        }
        self.log_heard.clear();
        for d in io.items::<Detection>(0) {
            match d.value {
                Detection::Sweep(s) => self.sweep(&s),
                Detection::Rejected => self.tracks_bad += 1,
                Detection::Burst => self.bursts += 1,
                Detection::Power { sum, n } => {
                    self.power.0 += sum;
                    self.power.1 += n;
                }
                Detection::Reset {
                    centre_hz,
                    tuned_hz,
                } => {
                    self.move_centre(centre_hz, tuned_hz);
                    self.publish();
                }
            }
        }
        let vals = &io.spectrum(1).values;
        self.frames(vals);
        let heard = std::mem::take(&mut self.log_heard);
        for n in &heard {
            if let Some(r) = self.lists.radars.iter().find(|r| r.number == *n) {
                let e = HFRADAR_RADAR_HEARD.event(
                    r.last_t,
                    r.centre_hz,
                    &HfradarRadarHeardEvent {
                        radar: i64::from(r.number),
                        band: r.band.as_deref(),
                        width: Hertz(r.width_hz),
                        direction: r.direction(),
                        source: r.id.as_deref(),
                        family: Family::Other,
                    },
                );
                io.messages_out(0).push(e);
            }
        }
        self.log_heard = heard;
        if io.tick() {
            self.publish();
            if let Some(pic) = self.picture() {
                io.items_out::<Picture>(1).push(Timed {
                    time: self.now,
                    value: pic,
                });
            }
        }
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(2).push(Timed {
                time: self.now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RadarSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RadarSetting::Scan => self.scan = SCAN.read(id, v)?,
            RadarSetting::Dwell => self.dwell_s = DWELL.read(id, v)?,
        }
        self.scan_request();
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (RadarSetting::Scan, Value::Bool(self.scan)),
            (RadarSetting::Dwell, Value::Duration(self.dwell_s)),
        ])
    }
    fn commands(&self) -> &Commands {
        RadarCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match RadarCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RadarCommand::Clear => {
                self.lists.clear();
                self.marks.clear();
            }
            RadarCommand::BandTune => {
                let HfradarBandArgs { row } = HFRADAR_BAND_TUNE.args(args)?;
                if self.scan {
                    return Err(CommandError::Failed {
                        source: StageError::new(ScanOn),
                    });
                }
                let Some(b) = BANDS.iter().find(|b| b.0 == row) else {
                    return Err(CommandError::NoSuchRow {
                        table: self
                            .p
                            .results()
                            .decl()
                            .table_id(HFRADAR_BANDS.def().key)
                            .expect("the declaration holds the bands"),
                        key: row.to_owned(),
                    });
                };
                self.requests.push(HostRequest::retune(
                    band_middle(b),
                    &format!("ocean radar band {}", b.0),
                ));
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_text::morse::Keyer;

    #[test]
    fn the_shared_decoder_reads_a_keyed_callsign() {
        // 15 words a minute: a dot of 80 ms.
        let runs: Vec<(bool, f64)> = Keyer::new(15.0)
            .unwrap()
            .key("AB1CD")
            .map(|k| (k.on, k.seconds))
            .collect();
        let dot = fit_dot(&runs, 0.1).unwrap();
        assert!((dot - 0.08).abs() < 1e-9);
        assert_eq!(read_morse(&runs, dot), "AB1CD");
    }
}
