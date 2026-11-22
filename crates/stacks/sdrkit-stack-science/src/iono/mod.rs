//! Ionospheric sounders (`iono`): finds the chirp sounders that sweep the HF band, learns their period and offset, dechirps their
//! sweeps into ionograms, and scales them (foE, foF2, MUF(3000)F2, or the modes and path MUF of a distant sounder). The graph is one
//! component, `iono.sounders`, with the detector, the sounder table, the dechirp and the scaling as plain types.

pub mod detect;
pub mod ionogram;
pub mod scale;
pub mod synth;

use crate::family::{lock_state_of, status, write_series, REPORT_S};
use detect::{Active, Detector, Found, RATES_HZ_S};
use ionogram::{Dechirper, Ionogram};
use scale::{Geometry, KnownSounder, Match, Scaling, TraceOptions, COLS, COL_HZ};
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::command::Command;
use crate::typed::{
    IonoBandRow, IonoFields, IonoSelectArgs, IonoSounderRow, IonoTimelineRow, IONO_BANDS,
    IONO_COVERAGE, IONO_HISTORY, IONO_SELECT, IONO_SOUNDERS, IONO_TIMELINE,
};
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    Choice, Clock, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError,
    GenConfig, Generator, Hz, Receiver, Record, Results, ResultsDecl, SeriesId, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StackConfig, StateBlob, StateError, Status,
    TableId, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io,
    Port, RateFrom, RunContext, StackGraph,
};
use sdrkit_util_geo::Position;
use sdrkit_util_objects::commands::{self as objcmd};
use sdrkit_util_objects::shaped::{log_rows, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

sdrkit_core::choice_enum! {
    /// The state of a band.
    pub enum BandStateChoice {
        /// It starts below the MUF and not below the lowest echo.
        Open = ("open", "Open"),
        /// It starts above the MUF.
        Closed = ("closed", "Closed"),
        /// It starts below the lowest echo.
        Absorbed = ("absorbed", "Absorbed"),
        /// No MUF is known.
        Unknown = ("unknown", "Unknown"),
    }
}

impl BandStateChoice {
    fn of(s: scale::BandState) -> BandStateChoice {
        match s {
            scale::BandState::Open => BandStateChoice::Open,
            scale::BandState::Closed => BandStateChoice::Closed,
            scale::BandState::Absorbed => BandStateChoice::Absorbed,
            scale::BandState::Unknown => BandStateChoice::Unknown,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The geometry of an ionogram.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum GeometryChoice {
        /// From the known sounder's distance, else from the height of the lowest trace.
        Auto = ("auto", "Automatic"),
        /// A local ionosonde.
        Vertical = ("vertical", "Vertical"),
        /// A distant sounder.
        Oblique = ("oblique", "Oblique"),
    }
}

/// Sweeps kept in the timeline.
pub const MAX_TIMELINE: usize = 200;

/// What `iono` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape("iono").results_decl()
}

/// One point of a sounder's scaling history. Frequencies in MHz, heights and distances in km.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoryPoint {
    /// The start of the sweep scaled, s of signal time.
    pub time_s: f64,
    /// Vertical or oblique.
    pub geometry: GeometryChoice,
    /// The values in the order of [`IONO_HISTORY`](crate::typed::IONO_HISTORY).
    pub values: [Option<f64>; 9],
}

impl HistoryPoint {
    fn of(time_s: f64, s: &Scaling) -> HistoryPoint {
        let mhz = |f: Option<scale::Freq>| f.map(|f| f.hz / 1e6);
        match s {
            Scaling::Vertical(v) => HistoryPoint {
                time_s,
                geometry: GeometryChoice::Vertical,
                values: [
                    mhz(v.fo_e),
                    mhz(v.fo_f1),
                    mhz(v.fo_f2),
                    v.hp_e_km,
                    v.hp_f_km,
                    v.m3000,
                    v.muf3000_hz.map(|m| m / 1e6),
                    None,
                    None,
                ],
            },
            Scaling::Oblique(o) => HistoryPoint {
                time_s,
                geometry: GeometryChoice::Oblique,
                values: [
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    mhz(o.path_muf),
                    Some(o.distance_km),
                ],
            },
        }
    }
}

/// The CSV of a scaling history: a header, then one line per point.
pub fn history_csv(points: &[HistoryPoint]) -> String {
    let mut s = String::from(
        "time_s,geometry,foe_mhz,fof1_mhz,fof2_mhz,hpe_km,hpf_km,m3000,muf3000_mhz,path_muf_mhz,distance_km\n",
    );
    for p in points {
        let geometry = &GeometryChoice::CHOICES[usize::from(p.geometry.index())].key;
        let _ = write!(s, "{:.3},{geometry}", p.time_s);
        for v in p.values {
            match v {
                Some(v) => {
                    let _ = write!(s, ",{v:.3}");
                }
                None => s.push(','),
            }
        }
        s.push('\n');
    }
    s
}

/// A sounder heard.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sounder {
    /// Its id, from 1.
    pub id: u32,
    /// The sweep rate, Hz per second.
    pub rate_hz_s: f64,
    /// The repetition period, once two sweeps were heard, s.
    pub period_s: Option<f64>,
    /// The start modulo the period, s.
    pub offset_s: Option<f64>,
    /// The start of the last sweep heard, s.
    pub last_start_s: f64,
    /// When it was last heard, s.
    pub last_seen_s: f64,
    /// The start of the next sweep, once the period is known, s.
    pub next_s: Option<f64>,
    /// The strongest chirp peak over the noise, dB.
    pub snr_db: f64,
    /// The radio frequencies heard, Hz.
    pub heard_hz: (f64, f64),
    /// Sweeps received.
    pub sweeps: u32,
    /// Expected sweeps not heard.
    pub missed: u32,
    /// The known sounder it matches.
    pub matched: Match,
    /// The scaling history.
    pub history: Vec<HistoryPoint>,
    /// The latest scaling.
    pub scaling: Option<Scaling>,
    /// The ionogram sums (not saved: a restored sounder starts a fresh ionogram).
    #[serde(skip)]
    pub ionogram: Ionogram,
}

/// A sweep in the timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimelineRow {
    /// The start, s.
    pub start_s: f64,
    /// The sounder.
    pub sounder: u32,
    /// The rate, Hz per second.
    pub rate_hz_s: f64,
    /// The radio frequencies that crossed the span, Hz.
    pub part_hz: (f64, f64),
    /// The SNR, dB.
    pub snr_db: f64,
    /// The sweep was expected (its whole crossing went into the ionogram).
    pub predicted: bool,
}

impl Shift for TimelineRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for TimelineRow {
    type Row = IonoTimelineRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&IonoTimelineRow {
            start_s: self.start_s,
            id: i64::from(self.sounder),
            rate_khz_s: self.rate_hz_s / 1e3,
            part_low: Hertz(self.part_hz.0),
            part_high: Hertz(self.part_hz.1),
            snr_db: self.snr_db,
            predicted: self.predicted,
        });
    }
}

/// What the component saves: the sounders with their history, and the timeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Store {
    /// The sounders.
    pub sounders: Vec<Sounder>,
    /// The timeline, oldest first.
    pub timeline: LogRing<TimelineRow>,
    /// The next sounder id.
    pub next_id: u32,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            sounders: Vec::new(),
            timeline: LogRing::new(MAX_TIMELINE),
            next_id: 1,
        }
    }
}

/// The owner and version of the saved state.
const STATE: (&str, u32) = ("iono.sounders", 2);

struct Sweep {
    sounder: u32,
    rate: usize,
    tie_s: f64,
    predicted: bool,
    hits: u32,
    snr_db: f64,
    dechirp: Dechirper,
}

/// A predicted sweep needs this many tied hits to count as heard.
const PREDICTED_HITS: u32 = 10;
/// Two starts belong to the same schedule when they agree within this, s.
const SCHEDULE_S: f64 = 0.1;
/// The highest frequency of a sweep, Hz.
const SWEEP_TOP_HZ: f64 = 30e6;

/// The sounders component (`iono.sounders`: iq in, results out).
pub struct Sounders {
    picture_due: bool,
    rate: f64,
    centre: f64,
    t0_s: f64,
    live: bool,
    clock: Clock,
    detector: Option<Detector>,
    store: Store,
    active: Vec<Sweep>,
    tie: Vec<Active>,
    tied: Vec<u32>,
    found: Vec<Found>,
    coverage: Vec<bool>,
    now_s: f64,
    power: (f64, u64),
    level_dbfs: Option<f64>,
    locked: bool,
    selected: Option<u32>,
    geometry: GeometryChoice,
    db_top: f64,
    list: Vec<KnownSounder>,
    site: Option<Position>,
    published: Published,
    sounders_table: TableId,
    series: [SeriesId; 10],
    shown: Status,
    key: String,
}

impl Default for Sounders {
    fn default() -> Self {
        let decl = std::sync::Arc::new(declaration());
        let sounders_table = decl
            .table_id(IONO_SOUNDERS.key)
            .expect("the declaration holds the sounders");
        let mut series = [SeriesId(0); 10];
        series[0] = decl
            .series_id(IONO_COVERAGE.key)
            .expect("the declaration holds the coverage");
        for (s, d) in series[1..].iter_mut().zip(IONO_HISTORY) {
            *s = decl
                .series_id(d.key)
                .expect("the declaration holds the history");
        }
        Sounders {
            picture_due: false,
            rate: 0.0,
            centre: 0.0,
            t0_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            detector: None,
            store: Store::default(),
            active: Vec::new(),
            tie: Vec::new(),
            tied: Vec::new(),
            found: Vec::new(),
            coverage: vec![false; COLS],
            now_s: 0.0,
            power: (0.0, 0),
            level_dbfs: None,
            locked: LOCKED.default,
            selected: None,
            geometry: GEOMETRY.default,
            db_top: DB_TOP.default,
            list: scale::known_sounders(),
            site: None,
            published: Published::from_decl(decl),
            sounders_table,
            series,
            shown: Status::default(),
            key: String::new(),
        }
    }
}

impl Sounders {
    /// What is kept: the sounders and the timeline.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The sounder whose ionogram is shown: the selected one, else the latest heard.
    pub fn selected(&self) -> Option<&Sounder> {
        if let Some(id) = self.selected {
            if let Some(s) = self.store.sounders.iter().find(|s| s.id == id) {
                return Some(s);
            }
        }
        self.store
            .sounders
            .iter()
            .max_by(|a, b| a.last_seen_s.total_cmp(&b.last_seen_s))
    }

    /// The columns of 0 to 30 MHz captured since the reset.
    pub fn coverage(&self) -> &[bool] {
        &self.coverage
    }

    /// The selected sounder's ionogram as a grey picture (`db_top` dB white).
    fn picture(&self) -> Option<sdrkit_codec_image::Raster> {
        self.selected().map(|s| s.ionogram.picture(self.db_top))
    }

    fn clear(&mut self) {
        self.store = Store::default();
        self.selected = None;
        self.published.clear_rows(&IONO_SOUNDERS);
        self.active.clear();
        self.coverage.iter_mut().for_each(|c| *c = false);
        if let Some(d) = &mut self.detector {
            d.clear();
        }
    }

    fn span(&self) -> (f64, f64) {
        (
            self.centre - 0.45 * self.rate,
            self.centre + 0.45 * self.rate,
        )
    }

    fn mark_coverage(&mut self) {
        let (lo, hi) = self.span();
        for (c, v) in self.coverage.iter_mut().enumerate() {
            let a = c as f64 * COL_HZ;
            if a >= lo && a + COL_HZ <= hi {
                *v = true;
            }
        }
    }

    fn start_of(&self, f: &Found) -> f64 {
        if self.locked {
            f.start_s.round()
        } else {
            // Delays are relative: the model starts a little before the first echo, so every echo has a positive delay.
            f.start_s - 1e-3
        }
    }

    /// Ties a found sweep to a sounder (a new one when none fits) and starts its dechirp.
    fn add_found(&mut self, f: &Found) {
        let r = RATES_HZ_S[f.rate];
        let start = self.start_of(f);
        let sounders = &mut self.store.sounders;
        let idx = sounders
            .iter()
            .position(|s| {
                (s.rate_hz_s - r).abs() < 1.0
                    && match (s.period_s, s.next_s) {
                        (Some(p), Some(next)) => {
                            let k = ((start - next) / p).round();
                            k >= 0.0 && (start - next - k * p).abs() < SCHEDULE_S
                        }
                        _ => start - s.last_start_s > 2.0,
                    }
            })
            .unwrap_or_else(|| {
                let id = self.store.next_id;
                self.store.next_id += 1;
                sounders.push(Sounder {
                    id,
                    rate_hz_s: r,
                    period_s: None,
                    offset_s: None,
                    last_start_s: f64::NEG_INFINITY,
                    last_seen_s: self.now_s,
                    next_s: None,
                    snr_db: f.snr_db,
                    heard_hz: f.range_hz,
                    sweeps: 0,
                    missed: 0,
                    matched: Match::default(),
                    history: Vec::new(),
                    scaling: None,
                    ionogram: Ionogram::default(),
                });
                sounders.len() - 1
            });
        let s = &mut sounders[idx];
        match (s.period_s, s.next_s) {
            (Some(p), Some(next)) => s.missed += ((start - next) / p).round().max(0.0) as u32,
            _ if s.last_start_s.is_finite() => {
                let p = start - s.last_start_s;
                s.period_s = Some(p);
                s.offset_s = Some(start.rem_euclid(p));
            }
            _ => {}
        }
        s.next_s = s.period_s.map(|p| start + p);
        s.last_start_s = start;
        s.last_seen_s = self.now_s;
        s.sweeps += 1;
        s.snr_db = s.snr_db.max(f.snr_db);
        s.heard_hz = (
            s.heard_hz.0.min(f.range_hz.0),
            s.heard_hz.1.max(f.range_hz.1),
        );
        let id = s.id;
        self.active.push(Sweep {
            sounder: id,
            rate: f.rate,
            tie_s: f.start_s,
            predicted: false,
            hits: f.hits,
            snr_db: f.snr_db,
            dechirp: Dechirper::new(self.rate, self.centre, r, start),
        });
    }

    /// Starts the dechirp of every expected sweep that enters the span now.
    fn predict(&mut self) {
        let (lo, _) = self.span();
        for s in &mut self.store.sounders {
            let (Some(p), Some(next)) = (s.period_s, s.next_s) else {
                continue;
            };
            if self.active.iter().any(|a| a.sounder == s.id) {
                continue;
            }
            let enter = next + lo.max(0.0) / s.rate_hz_s;
            if self.now_s >= enter - 0.05 {
                let rate = RATES_HZ_S
                    .iter()
                    .position(|&r| (r - s.rate_hz_s).abs() < 1.0)
                    .unwrap_or(0);
                self.active.push(Sweep {
                    sounder: s.id,
                    rate,
                    tie_s: next + if self.locked { 0.0 } else { 1e-3 },
                    predicted: true,
                    hits: 0,
                    snr_db: f64::NEG_INFINITY,
                    dechirp: Dechirper::new(self.rate, self.centre, s.rate_hz_s, next),
                });
                s.next_s = Some(next + p);
            }
        }
    }

    /// Ends the sweeps that left the span: the timeline, the counts and the scaling.
    fn end_sweeps(&mut self) {
        let (lo, hi) = self.span();
        let now = self.now_s;
        let mut i = 0;
        while i < self.active.len() {
            let a = &self.active[i];
            let r = RATES_HZ_S[a.rate];
            let f_now = r * (now - a.dechirp.start_s());
            if f_now <= hi.min(SWEEP_TOP_HZ) {
                i += 1;
                continue;
            }
            let a = self.active.swap_remove(i);
            let Some(s) = self.store.sounders.iter_mut().find(|s| s.id == a.sounder) else {
                continue;
            };
            if a.predicted {
                if a.hits >= PREDICTED_HITS {
                    s.sweeps += 1;
                    s.last_start_s = a.dechirp.start_s();
                    s.last_seen_s = now;
                    s.snr_db = s.snr_db.max(a.snr_db);
                } else {
                    s.missed += 1;
                    continue;
                }
            }
            let part = (lo.max(0.0), hi.min(SWEEP_TOP_HZ));
            self.store.timeline.push(TimelineRow {
                start_s: a.dechirp.start_s(),
                sounder: a.sounder,
                rate_hz_s: r,
                part_hz: part,
                snr_db: a.snr_db,
                predicted: a.predicted,
            });
            let geometry = match self.geometry {
                GeometryChoice::Auto => Geometry::Auto,
                GeometryChoice::Vertical => Geometry::Vertical,
                GeometryChoice::Oblique => Geometry::Oblique,
            };
            Self::scale_sounder(s, geometry, &self.list, self.site, a.dechirp.start_s());
            self.picture_due = true;
        }
    }

    fn scale_sounder(
        s: &mut Sounder,
        geometry: Geometry,
        list: &[KnownSounder],
        site: Option<Position>,
        time_s: f64,
    ) {
        s.matched = scale::match_sounder(list, s.rate_hz_s, s.period_s, s.offset_s, site, None);
        let known_km = s.matched.distance_km;
        let ion = s.ionogram.db();
        if let Some(sc) = scale::scale(&ion, &TraceOptions::default(), geometry, known_km) {
            s.history.push(HistoryPoint::of(time_s, &sc));
            if s.history.len() > 1000 {
                s.history.remove(0);
            }
            s.scaling = Some(sc);
        }
    }

    fn publish(&mut self) {
        let (lo, hi) = self.span();
        let sel = self.selected().map(|s| s.id);
        let state = if self.power.1 > 0 || self.detector.as_ref().is_some_and(|d| d.ffts() > 0) {
            1
        } else {
            0
        };
        let selected = self.store.sounders.iter().find(|s| Some(s.id) == sel);
        status(
            &mut self.shown,
            lock_state_of(state),
            None,
            None,
            None,
            |line| {
                let _ = match selected {
                Some(s) => write!(
                    line,
                    "Ionospheric sounders: sounder {}, {:.0} kHz/s sweep, {:.3} – {:.3} MHz, {:.0} dB",
                    s.id,
                    s.rate_hz_s / 1e3,
                    s.heard_hz.0 / 1e6,
                    s.heard_hz.1 / 1e6,
                    s.snr_db
                ),
                None => write!(
                    line,
                    "Ionospheric sounders: looking for chirps of 100, 125, 150, 200 and 500 kHz/s"
                ),
            };
            },
        );
        let p = &mut self.published;
        p.results_mut().set_status(&self.shown);
        if self.power.1 > 0 {
            self.level_dbfs = Some(power_to_db(self.power.0 / self.power.1 as f64));
        }
        self.power = (0.0, 0);
        let d = self.detector.as_ref();
        p.put(&IonoFields {
            rate: Hertz(self.rate),
            centre: Hertz(self.centre),
            signal_time_s: self.now_s,
            timing_lock: self.locked,
            span: Hertz(hi - lo),
            sounders_heard: self.store.sounders.len() as i64,
            active_sweeps: self.active.len() as i64,
            detector_fft: d.map(|d| d.fft_len() as i64),
            detector_ms: d.map(|d| 1e3 * d.fft_len() as f64 / self.rate),
            detector_bin: d.map(|d| Hertz(d.bin_hz())),
            ffts: d.map(|d| d.ffts() as i64),
            loose_peaks: d.map(|d| d.loose_peaks() as i64),
            level_dbfs: self.level_dbfs,
            selected: sel.map(|id| {
                self.key.clear();
                let _ = write!(self.key, "{id}");
                self.key.as_str()
            }),
        });
        for s in &self.store.sounders {
            self.key.clear();
            let _ = write!(self.key, "{}", s.id);
            let active = self.active.iter().any(|a| a.sounder == s.id);
            let last = s.history.last();
            let v = |k: usize| last.and_then(|h| h.values[k]);
            IONO_SOUNDERS.put(
                p,
                &self.key,
                &IonoSounderRow {
                    id: i64::from(s.id),
                    rate_khz_s: s.rate_hz_s / 1e3,
                    period: s.period_s.map(Seconds),
                    offset_s: s.offset_s,
                    last_start_s: s.last_start_s,
                    last_seen_s: s.last_seen_s,
                    next_s: s.next_s,
                    snr_db: s.snr_db,
                    heard_low: Hertz(s.heard_hz.0),
                    heard_high: Hertz(s.heard_hz.1),
                    sweeps: i64::from(s.sweeps),
                    missed: i64::from(s.missed),
                    active,
                    known: s.matched.index.map(|i| self.list[i].name.as_str()),
                    match_level: i64::from(s.matched.level),
                    candidates: s.matched.candidates.len() as i64,
                    geometry: last.map(|h| h.geometry),
                    foe_mhz: v(0),
                    fof1_mhz: v(1),
                    fof2_mhz: v(2),
                    hpe_km: v(3),
                    hpf_km: v(4),
                    m3000: v(5),
                    muf3000_mhz: v(6),
                    path_muf_mhz: v(7),
                    distance_km: v(8),
                },
            );
        }
        log_rows(&self.store.timeline, &IONO_TIMELINE, self.now_s, p);
        let scaling = selected.and_then(|s| s.scaling.as_ref());
        for (name, st) in scale::bands_open(
            scaling.and_then(|s| s.muf_hz()),
            scaling.and_then(|s| s.fmin_hz()),
        ) {
            IONO_BANDS.put(
                p,
                name,
                &IonoBandRow {
                    band: name,
                    state: BandStateChoice::of(st),
                },
            );
        }
        let r = p.results_mut();
        let coverage = &self.coverage;
        write_series(
            r,
            self.series[0],
            false,
            (0..COLS).map(|c| {
                let a = c as f64 * COL_HZ;
                let mut v = if coverage[c] { 1.0 } else { 0.0 };
                if a >= lo && a + COL_HZ <= hi {
                    v = 2.0;
                }
                if selected.is_some_and(|s| s.ionogram.count[c] > 0) {
                    v = 3.0;
                }
                [((a + 0.5 * COL_HZ) / 1e6) as f32, v]
            }),
        );
        for (k, id) in self.series[1..].iter().enumerate() {
            let history = selected.map_or(&[][..], |x| x.history.as_slice());
            write_series(
                r,
                *id,
                true,
                history
                    .iter()
                    .filter_map(|h| h.values[k].map(|v| [h.time_s as f32, v as f32])),
            );
        }
    }
}

/// Sweeps start on whole seconds.
pub const LOCKED: Setting<field::Bool> =
    Setting::new("locked", "Sweeps start on whole seconds", "", true);
/// The geometry of the ionograms.
pub const GEOMETRY: Setting<field::Choice<GeometryChoice>> =
    Setting::choice("geometry", "Geometry", GeometryChoice::Auto);
/// The top of the colour scale of the ionogram picture.
pub const DB_TOP: Setting<field::F64> =
    Setting::new("db_top", "Colour scale top", "dB", 20.0).range(5.0, 40.0, 1.0);
/// The radio's centre of the test signal.
pub const GEN_CENTRE: Setting<field::Hz> = Setting::new(
    "gen_centre_hz",
    "Test signal: the radio's centre",
    "Hz",
    10e6,
)
.range(0.1e6, 40e6, 1.0)
.section("Test signal")
.advanced();
/// The test signal has no sounders.
pub const GEN_NOISE_ONLY: Setting<field::Bool> =
    Setting::new("gen_noise_only", "Test signal: no sounders", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`Sounders`].
    pub enum SoundersSetting {
        /// Sweeps start on whole seconds.
        Locked = LOCKED,
        /// The geometry of the ionograms.
        Geometry = GEOMETRY,
        /// The top of the colour scale.
        DbTop = DB_TOP,
    }
}

sdrkit_core::settings! {
    /// The settings of the ionospheric sounder stack.
    pub enum IonoSetting {
        /// Sweeps start on whole seconds.
        Locked = LOCKED,
        /// The geometry of the ionograms.
        Geometry = GEOMETRY,
        /// The top of the colour scale.
        DbTop = DB_TOP,
        /// The radio's centre of the test signal.
        GenCentre = GEN_CENTRE,
        /// The test signal has no sounders.
        GenNoiseOnly = GEN_NOISE_ONLY,
    }
}

/// Shows the ionogram and history of the sounder heard last.
pub const SELECT_LATEST: Command = Command::new("select_latest", "Show the latest sounder");

sdrkit_core::commands! {
    /// The commands of [`Sounders`].
    pub enum SoundersCommand {
        /// Forgets the sounders, ionograms and coverage.
        Clear = objcmd::CLEAR,
        /// Shows one sounder.
        Select = IONO_SELECT.command,
        /// Shows the sounder heard last.
        SelectLatest = SELECT_LATEST,
    }
}

impl Component for Sounders {
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "iono.sounders",
            vec![Port::iq("in")],
            vec![Port::typed::<sdrkit_core::media::Picture>("ionogram")
                .rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(SoundersSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
        self.site = ctx.station.position;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.centre = inputs[0].centre_hz;
        self.t0_s = crate::family::wall(self.live, &self.clock).unwrap_or(0.0);
        self.detector = Some(Detector::new(self.rate, self.centre, self.t0_s));
        self.active.clear();
        self.publish();
        Ok(vec![Format::new(
            sdrkit_graph::Kind::typed::<sdrkit_core::media::Picture>(),
            0.0,
        )])
    }
    fn retuned(&mut self, centre_hz: f64, _tuned_hz: f64) {
        self.centre = centre_hz;
        if let Some(d) = &mut self.detector {
            d.retune(centre_hz);
        }
        for a in &mut self.active {
            a.dechirp.retune(centre_hz);
        }
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let first = io.input(0).first();
        let Some(mut det) = self.detector.take() else {
            return;
        };
        if !x.is_empty() {
            self.now_s = self.t0_s + first as f64 / self.rate;
            self.mark_coverage();
            self.predict();
            self.tie.clear();
            self.tie.extend(self.active.iter().map(|a| Active {
                rate: a.rate,
                start_s: a.tie_s,
            }));
            self.tied.clear();
            self.tied.resize(self.active.len(), 0);
            self.found.clear();
            det.push(x, first, &self.tie, &mut self.tied, &mut self.found);
            for (a, &n) in self.active.iter_mut().zip(&self.tied) {
                a.hits += n;
            }
            let found = std::mem::take(&mut self.found);
            for f in &found {
                self.add_found(f);
            }
            self.found = found;
            let span = self.span();
            let t = self.t0_s + first as f64 / self.rate;
            for a in &mut self.active {
                if let Some(s) = self.store.sounders.iter_mut().find(|s| s.id == a.sounder) {
                    a.dechirp.push(x, t, span, &mut s.ionogram);
                }
            }
            self.power.0 += sdrkit_algo_dsp::meter::mean_power(x) * x.len() as f64;
            self.power.1 += x.len() as u64;
            self.now_s = self.t0_s + (first + x.len() as u64) as f64 / self.rate;
            self.end_sweeps();
        }
        self.detector = Some(det);
        if std::mem::take(&mut self.picture_due) {
            if let Some(r) = self.picture() {
                io.items_out::<sdrkit_core::media::Picture>(0)
                    .push(sdrkit_graph::Timed {
                        time: self.now_s,
                        // The ionogram after a sweep, or of the sounder chosen: each is a whole picture.
                        value: r.to_picture(
                            "ionogram",
                            sdrkit_core::Picture::WHOLE,
                            sdrkit_core::PictureState::Finished,
                        ),
                    });
            }
        }
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SoundersSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SoundersSetting::Locked => self.locked = LOCKED.read(id, v)?,
            SoundersSetting::Geometry => self.geometry = GEOMETRY.read(id, v)?,
            SoundersSetting::DbTop => self.db_top = DB_TOP.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (SoundersSetting::Locked, Value::Bool(self.locked)),
            (
                SoundersSetting::Geometry,
                Value::Choice(self.geometry.index()),
            ),
            (SoundersSetting::DbTop, Value::F64(self.db_top)),
        ])
    }
    fn commands(&self) -> &Commands {
        SoundersCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SoundersCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SoundersCommand::Clear => self.clear(),
            SoundersCommand::Select => {
                let IonoSelectArgs { row } = IONO_SELECT.args(args)?;
                let found = row
                    .parse::<u32>()
                    .ok()
                    .filter(|id| self.store.sounders.iter().any(|s| s.id == *id));
                let Some(id) = found else {
                    return Err(CommandError::NoSuchRow {
                        table: self.sounders_table,
                        key: row.to_owned(),
                    });
                };
                self.selected = Some(id);
                self.picture_due = true;
            }
            SoundersCommand::SelectLatest => {
                self.selected = None;
                self.picture_due = true;
            }
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        let bytes = serde_json::to_vec(&self.store).ok()?;
        Some(StateBlob::new(STATE.0, STATE.1, bytes))
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let bytes = state.open(STATE.0, STATE.1)?;
        let store: Store = serde_json::from_slice(bytes).map_err(|_| StateError::Damaged)?;
        self.store = store;
        self.active.clear();
        self.published.clear_rows(&IONO_SOUNDERS);
        self.publish();
        Ok(())
    }
}

/// The lowest input rate.
pub const MIN_RATE: f64 = 250e3;

/// The receiver: the sounders component.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("sounders", Sounders::default())
        .input("sounders")
        .output("sounders")
        .clock(REPORT_S)
        .status_from("sounders")
        .declare(&declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape("iono"), graph)
        .min_rate(
            MIN_RATE,
            "Ionospheric sounders need a sample rate of at least 250 ksps",
        )
        .route(LOCKED.key(), &["sounders.locked"])?
        .route(GEOMETRY.key(), &["sounders.geometry"])?
        .route(DB_TOP.key(), &["sounders.db_top"])?
        .command(
            objcmd::CLEAR.key,
            sdrkit_graph::CommandTarget::of("sounders", "clear"),
        )
        .command(
            IONO_SELECT.command.key,
            sdrkit_graph::CommandTarget::of("sounders", "select"),
        )
        .command(
            SELECT_LATEST.key,
            sdrkit_graph::CommandTarget::of("sounders", "select_latest"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// One sounder of the test signal: rate, period, first start and its paths (delay s, amplitude).
#[derive(Clone, Debug, PartialEq)]
pub struct GenSounder {
    /// The sweep rate, Hz per second.
    pub rate_hz_s: f64,
    /// The period, s.
    pub period_s: f64,
    /// The first start, s from the first sample.
    pub start_s: f64,
    /// The paths: delay in s and amplitude.
    pub paths: Vec<(f64, f32)>,
}

/// The two sounders of the test signal: A 100 kHz/s every 30 s from +2 s (paths 1.6 ms and 2.4 ms, amplitudes 1.0 and 0.6), B
/// 500 kHz/s every 20 s from +7 s (1.8 ms and 2.9 ms, 1.0 and 0.5).
pub fn test_sounders() -> Vec<GenSounder> {
    vec![
        GenSounder {
            rate_hz_s: 100e3,
            period_s: 30.0,
            start_s: 2.0,
            paths: vec![(1.6e-3, 1.0), (2.4e-3, 0.6)],
        },
        GenSounder {
            rate_hz_s: 500e3,
            period_s: 20.0,
            start_s: 7.0,
            paths: vec![(1.8e-3, 1.0), (2.9e-3, 0.5)],
        },
    ]
}

/// The generator: chirp sounders that sweep up from 0 Hz for 95 % of their period, seen by a radio at `centre_hz` (only while the
/// chirp is inside ±rate/2), with the strongest path at unit amplitude. Noise comes from the standard `gen_*` settings; the signal
/// is declared as wide as the rate, so `gen_snr_db` is per sample (−10 dB puts the strongest path 10 dB under the noise).
pub struct IonoGenerator {
    rate: f64,
    centre: f64,
    sounders: Vec<GenSounder>,
    n: u64,
    osc: Nco,
    tmp: Vec<Complex32>,
}

impl IonoGenerator {
    /// The generator at `rate_hz` for a radio at `centre_hz`.
    pub fn new(rate_hz: f64, centre_hz: f64, sounders: Vec<GenSounder>) -> IonoGenerator {
        IonoGenerator {
            rate: rate_hz,
            centre: centre_hz,
            sounders,
            n: 0,
            osc: Nco::new(rate_hz, 0.0),
            tmp: vec![Complex32::new(0.0, 0.0); 64],
        }
    }
    /// From the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> IonoGenerator {
        let s = &cfg.settings;
        let centre = s.get(&GEN_CENTRE);
        let sounders = if s.get(&GEN_NOISE_ONLY) {
            Vec::new()
        } else {
            test_sounders()
        };
        IonoGenerator::new(cfg.rate_hz, centre, sounders)
    }
}

impl Generator for IonoGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let dt = 1.0 / self.rate;
        for chunk in out.chunks_mut(64) {
            chunk.fill(Complex32::new(0.0, 0.0));
            let t = self.n as f64 * dt;
            let len = chunk.len() as f64 * dt;
            for s in &self.sounders {
                for &(delay, amp) in &s.paths {
                    let since = t - delay - s.start_s;
                    if since < 0.0 {
                        continue;
                    }
                    let u = since.rem_euclid(s.period_s);
                    if u + len > 0.95 * s.period_s {
                        continue;
                    }
                    let fb0 = s.rate_hz_s * u - self.centre;
                    let fb1 = fb0 + s.rate_hz_s * len;
                    if fb0 < -0.5 * self.rate || fb1 > 0.5 * self.rate {
                        continue;
                    }
                    let cycles = 0.5 * s.rate_hz_s * u * u - self.centre * u;
                    self.osc.set_freq(0.5 * (fb0 + fb1));
                    self.osc.set_phase_cycles(cycles.rem_euclid(1.0));
                    let tmp = &mut self.tmp[..chunk.len()];
                    self.osc.fill(tmp);
                    for (o, v) in chunk.iter_mut().zip(tmp.iter()) {
                        *o += v * amp;
                    }
                }
            }
            self.n += chunk.len() as u64;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.rate))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(IonoGenerator::from_config(cfg)))
}

/// The verify case: the test signal, searching for sounders.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, two chirp sounders", o, 0.0);
    c.expect_audio = Some(false);
    if !o.clean {
        c.snr_db = Some(-10.0);
    }
    c.check = Some(|r| {
        let n = crate::family::field(r, IonoFields::FFTS.key);
        vec![CheckLine::new(
            IonoFields::FFTS.key,
            n > 0.0,
            format!("{n} detector FFTs"),
        )]
    });
    vec![c]
}

/// Adds the components of the tool to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("iono.sounders", || Box::new(Sounders::default()));
}
