//! Whistler and tweek detector (`whistler`). It takes VLF audio as the real part of the input, makes a spectrogram of rows of
//! dB over a per-bin floor at about 24 ksps, finds tweeks (a short ridge near n × 1.8 kHz just after a sferic) and whistlers
//! (tones that fall along Eckersley's law t = t0 + D / sqrt(f), found by a path search over D), and counts sferics.
//!
//! Documents: T. L. Eckersley (1935), the law t = t0 + D / sqrt(f) with D in s sqrt(Hz); R. A. Helliwell, "Whistlers and Related
//! Ionospheric Phenomena" (1965). The tweek cut-off of the first waveguide mode is 1.6 to 1.9 kHz in published studies.

pub mod gen;

use crate::events::EventRow;
use crate::typed::{WhistlerFields, WhistlerTrendRow, WHISTLER_TREND};
use crate::family::REPORT_S;
use crate::events::{EventLog, TrendLog};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::dispersion::{eckersley, geometric_grid, RidgeSearch, Scoring};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::peak::parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::{field, Published, RasterDef};
use sdrkit_core::results::{copy_if_changed, Axis, Raster};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState,
    RasterId, Receiver, Record, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "whistler";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 16e3;
/// The rate the front end aims at, Hz.
pub const WORK_RATE_HZ: f64 = 24e3;
/// The transform size.
pub const FFT_N: usize = 512;
/// The hop between rows, samples of the working rate.
pub const HOP: usize = 128;
/// The hop between rows at 24 ksps, s. Row counts below are at this hop; other rates scale them by time.
const HOP_24K_S: f64 = HOP as f64 / WORK_RATE_HZ;
/// Rows whose running mean is the first floor.
const FLOOR_ROWS: usize = 200;
/// The time constant of the floor after the first rows, s.
const FLOOR_TAU_S: f64 = 20.0;
/// The floor follows at most this many times itself.
const FLOOR_CAP: f32 = 3.0;
/// The limits of a row value, dB.
const D_DB: (f32, f32) = (-10.0, 40.0);
/// The band of the broadband value, Hz.
const BROADBAND_HZ: (f64, f64) = (1000.0, 10_000.0);
/// The rows kept, s.
const MEMORY_S: f64 = 4.0;
/// Rows of one display row.
const DISPLAY_ROWS: usize = 19;
/// The band of the display, Hz.
const DISPLAY_HZ: (f64, f64) = (500.0, 10_000.0);
/// Display rows kept.
pub const DISPLAY_KEPT: usize = 120;
/// No detection before this row.
const START_ROW: usize = 400;
/// The first row examined for a tweek.
const TWEEK_FROM_ROW: usize = 250;
/// Rows after a row that must exist before it is examined.
const TWEEK_AFTER: usize = 18;
/// A sferic row is the largest broadband value of this many rows either side.
const PEAK_HALF: usize = 8;
/// The least broadband value of a sferic, dB.
const SFERIC_DB: f32 = 5.0;
/// No tweek this soon after the last, s.
const TWEEK_GAP_S: f64 = 0.12;
/// The rows after the sferic averaged for a tweek: all, early, late.
const TWEEK_ROWS: [(usize, usize); 3] = [(3, 9), (3, 5), (7, 9)];
/// Where the tweek ridge is looked for, Hz.
const RIDGE_HZ: (f64, f64) = (1200.0, 5000.0);
/// The least ridge, dB.
const RIDGE_DB: f64 = 6.0;
/// The windows of the first and the second mode, Hz.
const MODE_HZ: [(f64, f64); 2] = [(1400.0, 2300.0), (3100.0, 4600.0)];
/// The distance of the sides of a ridge, Hz.
const SIDE_HZ: f64 = 600.0;
/// The ridge over its sides, dB.
const NARROW_DB: f64 = 4.0;
/// The band that must stay quiet after a tweek, Hz.
const QUIET_HZ: (f64, f64) = (6000.0, 9000.0);
/// The most mean value of the quiet band, dB.
const QUIET_DB: f64 = 4.0;
/// The duration given to a tweek, s.
const TWEEK_DURATION_S: f64 = 0.04;
/// Rows between two whistler searches.
const SEARCH_EVERY: usize = 94;
/// The least memory of a search, s.
const SEARCH_MEMORY_S: f64 = 3.0;
/// The path, Hz.
const PATH_HZ: (f64, f64) = (1500.0, 9000.0);
/// Bins between the points of the path.
const PATH_STEP: usize = 3;
/// The pivot of the path, Hz.
pub const PIVOT_HZ: f64 = 4000.0;
/// The grid of D: start, factor, end (s sqrt(Hz)).
const D_GRID: (f64, f64, f64) = (10.0, 1.07, 300.0);
/// The last point of a path lies at least this many rows before the newest.
const TAIL_ROWS: usize = 4;
/// How a path is scored.
const SCORING: Scoring = Scoring {
    low: 0.0,
    high: 15.0,
    hit_above: 4.0,
    min_share: 0.75,
};
/// The least time between two whistlers, s.
const WHISTLER_GAP_S: f64 = 1.5;
/// The whistler threshold when the setting is 0, dB.
pub const DEFAULT_THRESHOLD_DB: f64 = 6.0;
/// A trend minute is written this long after its end, when no detection can still fall into it, s.
const TREND_DELAY_S: f64 = 5.0;
/// The part of the rate the bands may reach.
const BAND_TOP: f64 = 0.45;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: every rule of the detector (the floor, the tweek windows \
1400 to 2300 Hz and 3100 to 4600 Hz with their 6 dB and 4 dB margins, the path from 1.5 to 9 kHz scored above 4 dB on 75 % \
of its points, the search of D from 10 to 300 s√Hz in steps of 7 %, the 1.5 s between whistlers). Eckersley's law holds only \
well below the nose frequency; tweek modes above the second are not reported.";

/// The whistler path threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Whistler path threshold", "dB", DEFAULT_THRESHOLD_DB)
        .range(0.0, 15.0, 0.0)
        .zero_means("6 dB")
        .help("The least mean level of a whistler path over the floor: 1 to 15 dB, 0 for the default of 6 dB.");
use crate::mscatter::START_UTC;

sdrkit_core::settings! {
    /// The settings of [`WhistlerDecoder`].
    pub enum DetectorSetting {
        /// The whistler threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `whistler` stack.
    pub enum WhistlerSetting {
        /// The whistler threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The dispersion of the test signal.
        GenDispersion = gen::GEN_DISPERSION,
        /// The tweek cut-off of the test signal.
        GenCutoff = gen::GEN_CUTOFF_HZ,
        /// The whistler level of the test signal.
        GenToneSnr = gen::GEN_TONE_SNR_DB,
    }
}

/// The threshold of a setting value: 0 is the default.
fn threshold_of(v: f64) -> f64 {
    if v > 0.0 {
        v
    } else {
        DEFAULT_THRESHOLD_DB
    }
}

/// What the detector found: an event row and whether it is a whistler.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The event.
    pub row: EventRow,
    /// A whistler (`true`) or a tweek.
    pub whistler: bool,
}

/// The bins of `lo..=hi` Hz at `bin` Hz a bin, cut to the bins below `top` Hz.
fn bins(lo: f64, hi: f64, bin: f64, top: f64) -> std::ops::RangeInclusive<usize> {
    let a = (lo / bin).ceil().max(1.0) as usize;
    let b = (hi.min(top) / bin).floor().max(0.0) as usize;
    a..=b
}

/// The detector at one input rate: the front end, the spectrogram, the floor, the tweek and whistler rules and the per-minute
/// counts. Fed real samples; every decision follows the absolute row index, so pieces of any size give the same output.
pub struct Detector {
    rate: f64,
    dec: usize,
    fs: f64,
    hop_s: f64,
    bin_hz: f64,
    width: usize,
    origin_s: f64,
    threshold_db: f64,
    front: FirDecimator<f32>,
    out: Vec<f32>,
    samples: Vec<f32>,
    samples_first: u64,
    fft: WindowedFft,
    frame: Vec<Complex32>,
    norm: f32,
    floor: Vec<f32>,
    row: Vec<f32>,
    rows: u64,
    memory: VecDeque<f32>,
    memory_first: u64,
    memory_rows: usize,
    broadband: VecDeque<f32>,
    // Row counts at this rate.
    floor_rows: u64,
    start_row: u64,
    tweek_after: u64,
    peak_half: u64,
    tweek_rows: [(usize, usize); 3],
    search_every: u64,
    search_memory: usize,
    display_rows: u64,
    // Bands.
    broadband_bins: std::ops::RangeInclusive<usize>,
    ridge_bins: std::ops::RangeInclusive<usize>,
    quiet_bins: std::ops::RangeInclusive<usize>,
    display_bins: std::ops::RangeInclusive<usize>,
    side_bins: usize,
    path_top_hz: f64,
    search: RidgeSearch,
    display_acc: Vec<f32>,
    display: VecDeque<Vec<f32>>,
    display_changed: bool,
    next_tweek: u64,
    last_tweek_t: Option<f64>,
    last_whistler_t: Option<f64>,
    minutes: BTreeMap<u64, [u32; 2]>,
    next_minute: u64,
    sferics: u64,
    found: Vec<Found>,
    trend: Vec<(u64, [f64; 2])>,
    last_broadband: Option<f32>,
}

impl Detector {
    /// A detector for input at `rate` whose first sample is at `origin_s` seconds of signal, with the whistler threshold
    /// `threshold_db` (0 for the default).
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`].
    pub fn new(rate: f64, threshold_db: f64, origin_s: f64) -> Result<Self, ConfigError> {
        let low = ConfigError::RateTooLow {
            need: MIN_RATE_HZ,
            got: rate,
        };
        if !rate.is_finite() || rate < MIN_RATE_HZ * 0.999_999 {
            return Err(low);
        }
        let dec = ((rate / WORK_RATE_HZ + 0.5).floor() as usize).max(1);
        let fs = rate / dec as f64;
        let hop_s = HOP as f64 / fs;
        let bin_hz = fs / FFT_N as f64;
        let width = FFT_N / 2 + 1;
        let top = BAND_TOP * fs;
        let rows = |n: usize| ((n as f64 * HOP_24K_S / hop_s).round() as u64).max(1);
        let path_top_hz = PATH_HZ.1.min(top);
        let first = (PATH_HZ.0 / bin_hz).round() as usize;
        let points: Vec<(f64, usize)> = (first..)
            .step_by(PATH_STEP)
            .take_while(|&k| k as f64 * bin_hz <= path_top_hz)
            .map(|k| (k as f64 * bin_hz, k))
            .collect();
        let grid = geometric_grid(D_GRID.0, D_GRID.1, D_GRID.2).map_err(|_| low.clone())?;
        let search = RidgeSearch::new(eckersley, &points, PIVOT_HZ, &grid, hop_s)
            .map_err(|_| low.clone())?;
        let fft = WindowedFft::new(FFT_N, FFT_N, Window::Hann).map_err(|_| low.clone())?;
        let norm = 1.0 / fft.window().iter().map(|w| w * w).sum::<f32>();
        let scale = |r: (usize, usize)| {
            (
                rows(r.0) as usize,
                (rows(r.1) as usize).max(rows(r.0) as usize),
            )
        };
        let display_bins = bins(DISPLAY_HZ.0, DISPLAY_HZ.1, bin_hz, fs / 2.0);
        let display_width = (display_bins.end() + 1)
            .saturating_sub(*display_bins.start())
            .div_ceil(2);
        let memory_rows = (MEMORY_S / hop_s).ceil() as usize;
        Ok(Detector {
            rate,
            dec,
            fs,
            hop_s,
            bin_hz,
            width,
            origin_s,
            threshold_db: threshold_of(threshold_db),
            front: FirDecimator::new(&vec![1.0 / dec as f32; dec], dec),
            out: Vec::new(),
            samples: Vec::new(),
            samples_first: 0,
            fft,
            frame: vec![Complex32::default(); FFT_N],
            norm,
            floor: vec![0.0; width],
            row: vec![0.0; width],
            rows: 0,
            memory: VecDeque::with_capacity(memory_rows * width),
            memory_first: 0,
            memory_rows,
            broadband: VecDeque::with_capacity(memory_rows),
            floor_rows: rows(FLOOR_ROWS),
            start_row: rows(START_ROW),
            tweek_after: rows(TWEEK_AFTER),
            peak_half: rows(PEAK_HALF),
            tweek_rows: TWEEK_ROWS.map(scale),
            search_every: rows(SEARCH_EVERY),
            search_memory: (SEARCH_MEMORY_S / hop_s).ceil() as usize,
            display_rows: rows(DISPLAY_ROWS),
            broadband_bins: bins(BROADBAND_HZ.0, BROADBAND_HZ.1, bin_hz, top),
            ridge_bins: bins(RIDGE_HZ.0, RIDGE_HZ.1, bin_hz, top),
            quiet_bins: bins(QUIET_HZ.0, QUIET_HZ.1, bin_hz, top),
            display_bins,
            side_bins: (SIDE_HZ / bin_hz).round() as usize,
            path_top_hz,
            search,
            display_acc: vec![D_DB.0; display_width],
            display: VecDeque::with_capacity(DISPLAY_KEPT),
            display_changed: false,
            next_tweek: rows(TWEEK_FROM_ROW),
            last_tweek_t: None,
            last_whistler_t: None,
            minutes: BTreeMap::new(),
            next_minute: (origin_s / 60.0).floor().max(0.0) as u64,
            sferics: 0,
            found: Vec::new(),
            trend: Vec::new(),
            last_broadband: None,
        })
    }

    /// The input rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The working rate after the front end.
    pub fn work_rate(&self) -> f64 {
        self.fs
    }

    /// The seconds between rows.
    pub fn hop_s(&self) -> f64 {
        self.hop_s
    }

    /// The highest frequency of the path, Hz.
    pub fn path_top_hz(&self) -> f64 {
        self.path_top_hz
    }

    /// Sets the whistler threshold (0 for the default).
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = threshold_of(db);
    }

    /// The signal time of the centre of row `r`, s.
    fn row_time(&self, r: u64) -> f64 {
        let centre = r as f64 * HOP as f64 + FFT_N as f64 / 2.0 - 0.5;
        self.origin_s + centre / self.fs + (self.dec as f64 - 1.0) / (2.0 * self.rate)
    }

    /// Feeds real samples at the input rate.
    pub fn process(&mut self, x: &[f32]) {
        self.out.clear();
        self.front.process(x, &mut self.out);
        self.samples.extend(
            self.out
                .iter()
                .map(|v| if v.is_finite() { *v } else { 0.0 }),
        );
        loop {
            let start = self.rows * HOP as u64;
            if self.samples_first + (self.samples.len() as u64) < start + FFT_N as u64 {
                break;
            }
            let at = (start - self.samples_first) as usize;
            for (f, &v) in self.frame.iter_mut().zip(&self.samples[at..at + FFT_N]) {
                *f = Complex32::new(v, 0.0);
            }
            self.make_row();
            self.rows += 1;
            let keep_from = self.rows * HOP as u64;
            let drop = (keep_from - self.samples_first) as usize;
            if drop >= 8 * FFT_N {
                self.samples.drain(..drop);
                self.samples_first = keep_from;
            }
        }
    }

    /// The row from the frame: power, floor, values, broadband value, memory and display; then the rules.
    fn make_row(&mut self) {
        let n = self.rows;
        let power = self.fft.run_power(&self.frame);
        let learning = n < self.floor_rows;
        let k_new = 1.0 / (n + 1) as f32;
        let follow = (self.hop_s / FLOOR_TAU_S) as f32;
        for ((&pw, fl), d) in power.iter().zip(&mut self.floor).zip(&mut self.row) {
            let p = pw * self.norm;
            if learning {
                *fl += (p - *fl) * k_new;
            }
            let base = fl.max(1e-30);
            *d = (10.0 * (p / base).max(1e-30).log10()).clamp(D_DB.0, D_DB.1);
            if !learning {
                *fl += (p.min(FLOOR_CAP * base) - base) * follow;
            }
        }
        let bb = {
            let r = &self.row[self.broadband_bins.clone()];
            r.iter().map(|v| v.max(0.0)).sum::<f32>() / r.len().max(1) as f32
        };
        self.last_broadband = Some(bb);
        if self.memory.len() >= self.memory_rows * self.width {
            self.memory.drain(..self.width);
            self.broadband.pop_front();
            self.memory_first += 1;
        }
        self.memory.extend(self.row.iter().copied());
        self.broadband.push_back(bb);
        self.display_row(n);
        if n >= self.start_row {
            while self.next_tweek + self.tweek_after <= n {
                let r = self.next_tweek;
                self.next_tweek += 1;
                self.examine(r);
            }
            if n.is_multiple_of(self.search_every) && self.memory_len() >= self.search_memory {
                self.search_whistler();
            }
        }
        let now = self.row_time(n);
        while (self.next_minute + 1) as f64 * 60.0 + TREND_DELAY_S <= now {
            let m = self.next_minute;
            let c = self.minutes.remove(&m).unwrap_or([0, 0]);
            self.trend.push((m, [f64::from(c[0]), f64::from(c[1])]));
            self.next_minute += 1;
        }
    }

    /// Rows held.
    fn memory_len(&self) -> usize {
        self.broadband.len()
    }

    /// Row `r` of the memory. `None` when it is not held.
    fn mem_row(&self, r: u64) -> Option<impl Iterator<Item = f32> + '_> {
        if r < self.memory_first || r >= self.memory_first + self.memory_len() as u64 {
            return None;
        }
        let i = (r - self.memory_first) as usize * self.width;
        Some(self.memory.range(i..i + self.width).copied())
    }

    /// The broadband value of row `r`.
    fn bb(&self, r: u64) -> Option<f32> {
        r.checked_sub(self.memory_first)
            .and_then(|i| self.broadband.get(i as usize).copied())
    }

    /// Adds row `n` to the display: the largest value of each pair of bins over [`DISPLAY_ROWS`] rows.
    fn display_row(&mut self, n: u64) {
        let lo = *self.display_bins.start();
        let hi = (*self.display_bins.end()).min(self.width - 1);
        for (j, acc) in self.display_acc.iter_mut().enumerate() {
            let a = lo + 2 * j;
            let b = (a + 1).min(hi);
            if a <= hi {
                *acc = acc.max(self.row[a]).max(self.row[b]);
            }
        }
        if (n + 1).is_multiple_of(self.display_rows) {
            if self.display.len() >= DISPLAY_KEPT {
                self.display.pop_front();
            }
            let fresh = vec![D_DB.0; self.display_acc.len()];
            self.display
                .push_back(std::mem::replace(&mut self.display_acc, fresh));
            self.display_changed = true;
        }
    }

    /// Seconds between display rows.
    pub fn display_step_s(&self) -> f64 {
        self.display_rows as f64 * self.hop_s
    }

    /// The tweek rules on row `r` (module docs); a sferic row is counted.
    fn examine(&mut self, r: u64) {
        let h = self.peak_half;
        if r < self.memory_first + h {
            return;
        }
        let Some(v) = self.bb(r) else {
            return;
        };
        if v < SFERIC_DB {
            return;
        }
        for j in r - h..=r + h {
            let Some(o) = self.bb(j) else {
                return;
            };
            if (j < r && o >= v) || (j > r && o > v) {
                return;
            }
        }
        let t = self.row_time(r);
        self.sferics += 1;
        self.count(t, 1);
        if self.last_tweek_t.is_some_and(|l| t - l < TWEEK_GAP_S) {
            return;
        }
        let means = self
            .tweek_rows
            .map(|(a, b)| self.mean_rows(r + a as u64, r + b as u64));
        let [Some(m), Some(_early), Some(late)] = means else {
            return;
        };
        let ms = smooth3(&m);
        let Some(kp) = self
            .ridge_bins
            .clone()
            .filter(|&k| k < self.width)
            .max_by(|&a, &b| ms[a].total_cmp(&ms[b]).then(b.cmp(&a)))
        else {
            return;
        };
        let mp = f64::from(ms[kp]);
        let fp = kp as f64 * self.bin_hz;
        if mp < RIDGE_DB {
            return;
        }
        let Some(mode) = MODE_HZ.iter().position(|&(lo, hi)| (lo..=hi).contains(&fp)) else {
            return;
        };
        let side = [kp.checked_sub(self.side_bins), Some(kp + self.side_bins)]
            .into_iter()
            .flatten()
            .filter(|&k| k < self.width)
            .map(|k| f64::from(ms[k]))
            .fold(f64::NEG_INFINITY, f64::max);
        if mp - side < NARROW_DB {
            return;
        }
        let quiet: Vec<f32> = self
            .quiet_bins
            .clone()
            .filter(|&k| k < self.width)
            .map(|k| m[k])
            .collect();
        if !quiet.is_empty() && f64::from(quiet.iter().sum::<f32>()) / quiet.len() as f64 > QUIET_DB
        {
            return;
        }
        // Where the ridge settles: the peak of the late mean near the ridge, refined on the dB values.
        let lo = kp.saturating_sub(self.side_bins).max(1);
        let hi = (kp + self.side_bins).min(self.width - 2);
        let kl = (lo..=hi)
            .max_by(|&a, &b| late[a].total_cmp(&late[b]).then(b.cmp(&a)))
            .unwrap_or(kp);
        let p = parabolic(
            f64::from(late[kl - 1]),
            f64::from(late[kl]),
            f64::from(late[kl + 1]),
        );
        let f = (kl as f64 + p.offset) * self.bin_hz;
        self.last_tweek_t = Some(t);
        self.count(t, 0);
        self.found.push(Found {
            row: EventRow {
                signal_time_s: t,
                kind: "tweek".into(),
                frequency_hz: f,
                peak_db: mp,
                duration_s: TWEEK_DURATION_S,
                aux1: (mode + 1) as f64,
                aux2: 0.0,
                note: format!("mode {}, ridge {fp:.0} Hz", mode + 1),
            },
            whistler: false,
        });
    }

    /// The mean of the rows `a..=b`, bin by bin. `None` when a row is not held.
    fn mean_rows(&self, a: u64, b: u64) -> Option<Vec<f32>> {
        let mut m = vec![0f32; self.width];
        for r in a..=b {
            for (acc, v) in m.iter_mut().zip(self.mem_row(r)?) {
                *acc += v;
            }
        }
        let n = (b - a + 1) as f32;
        m.iter_mut().for_each(|v| *v /= n);
        Some(m)
    }

    /// Adds one to count `i` (0 events, 1 sferics) of the minute of `t`, unless that minute is written already.
    fn count(&mut self, t: f64, i: usize) {
        let m = (t / 60.0).floor().max(0.0) as u64;
        if m >= self.next_minute {
            self.minutes.entry(m).or_insert([0, 0])[i] += 1;
        }
    }

    /// The path search over D in the memory; a path at the threshold or more, far enough from the last whistler, is one.
    fn search_whistler(&mut self) {
        let rows = self.memory_len();
        let cells = self.memory.make_contiguous();
        let Some(r) = self
            .search
            .best(cells, self.width, rows.saturating_sub(TAIL_ROWS), &SCORING)
        else {
            return;
        };
        if r.score < self.threshold_db || r.pivot_row < 0 {
            return;
        }
        let t = self.row_time(self.memory_first + r.pivot_row as u64);
        if self
            .last_whistler_t
            .is_some_and(|l| (t - l).abs() < WHISTLER_GAP_S)
        {
            return;
        }
        self.last_whistler_t = Some(t);
        self.count(t, 0);
        let d = r.dispersion;
        self.found.push(Found {
            row: EventRow {
                signal_time_s: t,
                kind: "whistler".into(),
                frequency_hz: PIVOT_HZ,
                peak_db: r.score,
                duration_s: d * (eckersley(PATH_HZ.0) - eckersley(self.path_top_hz)),
                aux1: d,
                aux2: r.share(),
                note: format!("share of the path above 4 dB: {:.0} %", 100.0 * r.share()),
            },
            whistler: true,
        });
    }

    /// The events found since the last call, in the order found.
    pub fn take_found(&mut self) -> Vec<Found> {
        std::mem::take(&mut self.found)
    }

    /// The trend rows written since the last call: the minute and its events and sferics.
    pub fn take_trend(&mut self) -> Vec<(u64, [f64; 2])> {
        std::mem::take(&mut self.trend)
    }

    /// Sferics counted.
    pub fn sferics(&self) -> u64 {
        self.sferics
    }

    /// The broadband value of the newest row, dB.
    pub fn broadband_db(&self) -> Option<f32> {
        self.last_broadband
    }

    /// Rows made.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Whether the rules run (the start row is reached).
    pub fn started(&self) -> bool {
        self.rows > self.start_row
    }

    /// Writes the display rows into `r` when they changed since the last call. Returns whether they changed.
    pub fn display(&mut self, r: &mut Raster) -> bool {
        if !std::mem::take(&mut self.display_changed) {
            return false;
        }
        let w = self.display_acc.len();
        r.width = w;
        r.height = self.display.len();
        r.x = Axis::new(
            (*self.display_bins.start() as f64 + 0.5) * self.bin_hz,
            2.0 * self.bin_hz,
            "Hz",
        );
        // The newest display row starts at the last whole multiple of its row count.
        let last_t =
            self.row_time((self.rows / self.display_rows).saturating_sub(1) * self.display_rows);
        r.y = Axis::new(
            last_t - (self.display.len().saturating_sub(1)) as f64 * self.display_step_s(),
            self.display_step_s(),
            "s",
        );
        r.unit.clear();
        r.unit.push_str("dB");
        r.values.clear();
        for row in &self.display {
            r.values.extend_from_slice(row);
        }
        true
    }
}

/// The mean of each value and its two neighbours (one at the ends).
fn smooth3(v: &[f32]) -> Vec<f32> {
    (0..v.len())
        .map(|i| {
            let a = i.saturating_sub(1);
            let b = (i + 1).min(v.len() - 1);
            v[a..=b].iter().sum::<f32>() / (b - a + 1) as f32
        })
        .collect()
}

/// The spectrogram: 0.5 to 10 kHz, a row every 19 rows of the detector.
pub const SPECTROGRAM: RasterDef = RasterDef {
    key: "spectrogram",
    label: "Spectrogram",
    x_unit: "Hz",
    y_unit: "s",
    unit: "dB",
};

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The `reset` command: the detector starts again (its floor is learned again); the tables are emptied.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Empties the tables and starts the detector again; the floor is learned again.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The `whistler` decoder: the [`Detector`] with its tables, trend, counts and spectrogram.
pub struct WhistlerDecoder {
    det: Option<Detector>,
    rate: f64,
    threshold_set: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    input: u64,
    reals: Vec<f32>,
    log: EventLog,
    trend: TrendLog,
    whistlers: u64,
    tweeks: u64,
    sferics_cleared: u64,
    last_d: Option<f64>,
    events: Vec<Event>,
    raster: Raster,
    raster_id: Option<RasterId>,
    started: bool,
    status: Status,
}

impl Default for WhistlerDecoder {
    fn default() -> Self {
        WhistlerDecoder {
            det: None,
            rate: 0.0,
            threshold_set: THRESHOLD.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            input: 0,
            reals: Vec::new(),
            log: EventLog::default(),
            trend: TrendLog::default(),
            whistlers: 0,
            tweeks: 0,
            sferics_cleared: 0,
            last_d: None,
            events: Vec::new(),
            raster: Raster::default(),
            raster_id: None,
            started: false,
            status: Status::default(),
        }
    }
}

impl WhistlerDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    /// Empties the tables and the counters.
    fn forget(&mut self) {
        self.log.clear();
        self.trend.clear();
        self.whistlers = 0;
        self.tweeks = 0;
        self.sferics_cleared = self.det.as_ref().map_or(0, Detector::sferics);
        self.last_d = None;
    }

    /// The tables emptied and the detector made again from the next sample.
    fn reset(&mut self) {
        self.forget();
        if self.rate > 0.0 {
            self.det = Detector::new(self.rate, self.threshold_set, self.now()).ok();
        }
        self.sferics_cleared = 0;
        self.raster = Raster::default();
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(det) = self.det.as_mut() else {
            return;
        };
        for f in det.take_found() {
            if f.whistler {
                self.whistlers += 1;
                self.last_d = Some(f.row.aux1);
            } else {
                self.tweeks += 1;
            }
            self.log.push(f.row, self.start_utc, &mut self.events);
        }
        for (m, v) in det.take_trend() {
            self.trend.push(m, v);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
}

impl Decoder for WhistlerDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.whistler"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DetectorSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DetectorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DetectorSetting::Threshold => {
                let v = THRESHOLD.read(id, value)?;
                if v > 0.0 && v < 1.0 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 1.0,
                        max: 15.0,
                        got: v,
                    });
                }
                self.threshold_set = v;
                if let Some(d) = self.det.as_mut() {
                    d.set_threshold(v);
                }
            }
            DetectorSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DetectorSetting::Threshold, Value::F64(self.threshold_set)),
            (DetectorSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.reset();
            Ok(None)
        } else {
            Err(CommandError::NoSuchId { id })
        }
    }
    fn results(&self) -> Published {
        Published::from_decl(Arc::new(declaration()))
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.clock = ctx.clock.clone();
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.input = 0;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.det = Some(Detector::new(input.rate, self.threshold_set, 0.0)?);
        self.sferics_cleared = 0;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        self.reals.clear();
        self.reals.extend(x.iter().map(|z| z.re));
        if let Some(d) = self.det.as_mut() {
            d.process(&self.reals);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let (sferics, bb, rows_started) = self.det.as_ref().map_or((0, None, false), |d| {
            (
                d.sferics().saturating_sub(self.sferics_cleared),
                d.broadband_db(),
                d.started(),
            )
        });
        self.log.publish(p, self.start_utc);
        self.trend.publish(
            p,
            &WHISTLER_TREND,
            self.start_utc,
            |utc, [events_per_min, sferics_per_min]| WhistlerTrendRow {
                utc,
                events_per_min,
                sferics_per_min,
            },
        );
        if let Some(d) = self.det.as_mut() {
            if d.display(&mut self.raster) {
                if self.raster_id.is_none() {
                    self.raster_id = p.results().decl().raster_id(SPECTROGRAM.key);
                }
                if let Some(id) = self.raster_id {
                    p.results_mut()
                        .update_raster(id, |r| copy_if_changed(r, &self.raster));
                }
            }
        }
        let st = &mut self.status;
        st.lock_state = match (self.started, rows_started, self.log.total() > 0) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.summary.clear();
        st.summary.push_str(&if !self.started {
            "Whistlers: waiting for samples".to_string()
        } else if !rows_started {
            "Whistlers: learning the floor".to_string()
        } else {
            format!(
                "Whistlers: {} whistlers, {} tweeks, {sferics} sferics",
                self.whistlers, self.tweeks
            )
        });
        p.put(&WhistlerFields {
            text: &st.summary,
            whistlers: i64::try_from(self.whistlers).unwrap_or(i64::MAX),
            tweeks: i64::try_from(self.tweeks).unwrap_or(i64::MAX),
            events_total: i64::try_from(self.log.total()).unwrap_or(i64::MAX),
            sferics: i64::try_from(sferics).unwrap_or(i64::MAX),
            dispersion: self.last_d,
            broadband_db: bb.map(f64::from),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.forget();
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(WhistlerDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The whistler detector needs a sample rate of at least 16 ksps",
        )
        .route(THRESHOLD.key(), &["dec.threshold_db"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("dec", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.whistler", || {
        Box::new(DecoderComponent::new(WhistlerDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::WhistlerGenerator::from_config(cfg)?))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a whistler at 3 s and a tweek at 6 s", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let w = crate::family::field(r, WhistlerFields::WHISTLERS.key);
        let t = crate::family::field(r, WhistlerFields::TWEEKS.key);
        vec![
            CheckLine::new(WhistlerFields::WHISTLERS.key, w >= 1.0, format!("{w}, at least 1 expected")),
            CheckLine::new(WhistlerFields::TWEEKS.key, t >= 1.0, format!("{t}, at least 1 expected")),
        ]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The whistler and tweek detector.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Whistlers and tweeks",
    group: "Science, time and utility",
    blurb: "In VLF audio from a sound card: whistlers that fall along Eckersley's law, found by a search over the dispersion, \
            tweeks just after a sferic, and the count of sferics.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.0,
        max_hz: 0.1e6,
        default_hz: 0.0,
        rate_hz: 48e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 10e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(48e3),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: WhistlerSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
