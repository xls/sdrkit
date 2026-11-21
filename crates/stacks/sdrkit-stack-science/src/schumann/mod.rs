//! Schumann resonance monitor (`schumann`). It takes ELF audio as the real part of the input, averages its spectrum at about
//! 200 sps over 5 minutes, finds the five lowest resonances over a running median baseline (frequency, height, Q), keeps a
//! spectrogram of 3 to 45 Hz and the fundamental minute by minute, and reports ELF transients of the 5 to 40 Hz band.
//!
//! Document: the published Schumann resonance frequencies 7.83, 14.1, 20.3, 26.3 and 32.5 Hz, with diurnal maxima near 9, 14
//! and 20 UT (as collected in the Wikipedia article "Schumann resonances").

pub mod gen;

use crate::events::EventRow;
use crate::typed::{
    SchumannFields, SchumannPeakRow, SchumannTrendRow, SCHUMANN_PEAKS, SCHUMANN_TREND,
};
use sdrkit_core::interface::{Hertz, Seconds};
use crate::family::{write_series, REPORT_S};
use crate::mscatter::START_UTC;
use crate::events::{EventLog, TrendLog};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::meter::power_to_db;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandError, CommandId};
use sdrkit_core::publish::{field, Published, RasterDef, SeriesDef};
use sdrkit_core::results::{copy_if_changed, Axis, Raster};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, Clock, Complex32, ConfigError, Event, GenConfig, Generator, LockProfile, LockState,
    RasterId, Receiver, Record, ResultsDecl, Schema, SeriesId, Setting, SettingError, SettingId,
    SettingSet, StackConfig, StackDescriptor, Status, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, CommandTarget, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Kind, Outbox, Port, RateFrom, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "schumann";
/// The least input rate of the stack, Hz.
pub const MIN_RATE_HZ: f64 = 400.0;
/// The least working rate of [`Monitor`], Hz: the baseline reaches 65 Hz.
pub const MIN_WORK_RATE_HZ: f64 = 140.0;
/// The rate the front end aims at, Hz.
pub const WORK_RATE_HZ: f64 = 200.0;
/// The transform size.
pub const FFT_N: usize = 2048;
/// The hop between spectra.
pub const HOP: usize = 1024;
/// The memory of the average, s.
const AVERAGE_S: f64 = 300.0;
/// The bins with a baseline, Hz.
const BASELINE_HZ: (f64, f64) = (3.0, 60.0);
/// The baseline is the median of the average over this much either side, Hz.
const BASELINE_HALF_HZ: f64 = 5.0;
/// The band of the spectrogram and the average spectrum, Hz.
const DISPLAY_HZ: (f64, f64) = (3.0, 45.0);
/// Bins of one spectrogram cell.
const DISPLAY_GROUP: usize = 4;
/// Spectrogram rows kept.
pub const DISPLAY_KEPT: usize = 120;
/// Peaks are searched after this much averaging, s.
const PEAKS_AFTER_S: f64 = 60.0;
/// A peak is searched this far either side of a published frequency, Hz.
const PEAK_WINDOW_HZ: f64 = 1.6;
/// The least peak over the baseline, dB.
const PEAK_DB: f64 = 2.0;
/// The band filter of the transients: high-pass and low-pass corners, Hz.
const BAND_HZ: (f64, f64) = (5.0, 40.0);
/// The energy of the transients is measured over groups of this length, s.
const GROUP_S: f64 = 0.5;
/// Quiet groups whose median is the comparison level.
const QUIET_KEPT: usize = 600;
/// Transients are judged once this many quiet groups are held.
const JUDGE_AFTER: usize = 60;
/// The transient threshold when the setting is 0, dB.
pub const DEFAULT_THRESHOLD_DB: f64 = 8.0;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: the windows of ±1.6 Hz around the published frequencies, \
the 2 dB least peak, the 5-minute average, the baseline (the median over ±5 Hz), and the transient rules (the 5 to 40 Hz \
band, the energy of 0.5 s groups against the median of the last 600 quiet groups). Transients are not claimed to be Q-bursts.";

/// The transient threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_db", "Transient threshold", "dB", DEFAULT_THRESHOLD_DB)
        .range(0.0, 20.0, 0.0)
        .zero_means("8 dB")
        .help("A 0.5 s group this much over the median of the quiet groups is part of a transient: 1 to 20 dB, 0 for the \
               default of 8 dB.");

sdrkit_core::settings! {
    /// The settings of [`SchumannDecoder`].
    pub enum MonitorSetting {
        /// The transient threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `schumann` stack.
    pub enum SchumannSetting {
        /// The transient threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The fundamental of the test signal.
        GenFundamental = gen::GEN_FUNDAMENTAL_HZ,
        /// The transients of the test signal.
        GenTransients = gen::GEN_TRANSIENTS,
    }
}

/// The decimation of the front end at `rate`: `max(1, floor(rate / 200 + 0.5))`.
pub fn decimation(rate: f64) -> usize {
    ((rate / WORK_RATE_HZ + 0.5).floor() as usize).max(1)
}

/// The threshold of a setting value: 0 is the default.
fn threshold_of(v: f64) -> f64 {
    if v > 0.0 {
        v
    } else {
        DEFAULT_THRESHOLD_DB
    }
}

/// A resonance found in the average.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Resonance {
    /// Frequency, Hz.
    pub frequency_hz: f64,
    /// Height over the baseline, dB.
    pub height_db: f64,
    /// Frequency over the width.
    pub q: f64,
}

/// A transient that has not ended.
#[derive(Clone, Copy, Debug)]
struct Open {
    start_s: f64,
    groups: usize,
    peak_db: f64,
    crossings: u64,
}

/// The monitor at one input rate: the front end, the average spectrum, its baseline, the resonances, the spectrogram, the
/// transients and the trend. Fed real samples; every decision follows absolute sample indices, so pieces of any size give the
/// same output.
pub struct Monitor {
    rate: f64,
    dec: usize,
    fs: f64,
    bin_hz: f64,
    row_s: f64,
    origin_s: f64,
    threshold_db: f64,
    front: FirDecimator<f32>,
    out: Vec<f32>,
    k: u64,
    samples: Vec<f32>,
    samples_first: u64,
    rows: u64,
    fft: WindowedFft,
    frame: Vec<Complex32>,
    norm: f32,
    power: Vec<f64>,
    average: Vec<f64>,
    baseline: Vec<f64>,
    scratch: Vec<f64>,
    baseline_bins: std::ops::RangeInclusive<usize>,
    display_bins: std::ops::RangeInclusive<usize>,
    half: usize,
    display: VecDeque<Vec<f32>>,
    display_changed: bool,
    resonances: [Option<Resonance>; 5],
    // The band filter and the groups.
    hp: (f64, f64, f64),
    lp: (f64, f64, f64),
    group_len: usize,
    group_sum: f64,
    group_n: usize,
    group_crossings: u64,
    last_sign: bool,
    quiet: VecDeque<f64>,
    open: Option<Open>,
    level_db: Option<f64>,
    found: Vec<EventRow>,
    trend: Vec<(u64, [f64; 2])>,
    next_minute: u64,
}

impl Monitor {
    /// A monitor for input at `rate` whose first sample is at `origin_s` seconds of signal, with the transient threshold
    /// `threshold_db` (0 for the default).
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] when the working rate would be below [`MIN_WORK_RATE_HZ`].
    pub fn new(rate: f64, threshold_db: f64, origin_s: f64) -> Result<Self, ConfigError> {
        let low = ConfigError::RateTooLow {
            need: MIN_WORK_RATE_HZ,
            got: rate,
        };
        if !(rate.is_finite() && rate >= MIN_WORK_RATE_HZ) {
            return Err(low);
        }
        let dec = decimation(rate);
        let fs = rate / dec as f64;
        if fs < MIN_WORK_RATE_HZ {
            return Err(low);
        }
        let bin_hz = fs / FFT_N as f64;
        let fft = WindowedFft::new(FFT_N, FFT_N, Window::Hann).map_err(|_| low.clone())?;
        let norm = 1.0 / fft.window().iter().map(|w| w * w).sum::<f32>();
        let width = FFT_N / 2 + 1;
        let span = |lo: f64, hi: f64| {
            ((lo / bin_hz).ceil() as usize).max(1)..=((hi / bin_hz).floor() as usize).min(width - 2)
        };
        let group_len = ((GROUP_S * fs).round() as usize).max(1);
        Ok(Monitor {
            rate,
            dec,
            fs,
            bin_hz,
            row_s: HOP as f64 / fs,
            origin_s,
            threshold_db: threshold_of(threshold_db),
            front: FirDecimator::new(&vec![1.0 / dec as f32; dec], dec),
            out: Vec::new(),
            k: 0,
            samples: Vec::new(),
            samples_first: 0,
            rows: 0,
            fft,
            frame: vec![Complex32::default(); FFT_N],
            norm,
            power: vec![0.0; width],
            average: vec![0.0; width],
            baseline: vec![0.0; width],
            scratch: Vec::new(),
            baseline_bins: span(BASELINE_HZ.0, BASELINE_HZ.1),
            display_bins: span(DISPLAY_HZ.0, DISPLAY_HZ.1),
            half: (BASELINE_HALF_HZ / bin_hz).floor() as usize,
            display: VecDeque::with_capacity(DISPLAY_KEPT),
            display_changed: false,
            resonances: [None; 5],
            hp: ((-std::f64::consts::TAU * BAND_HZ.0 / fs).exp(), 0.0, 0.0),
            lp: (
                1.0 - (-std::f64::consts::TAU * BAND_HZ.1 / fs).exp(),
                0.0,
                0.0,
            ),
            group_len,
            group_sum: 0.0,
            group_n: 0,
            group_crossings: 0,
            last_sign: false,
            quiet: VecDeque::with_capacity(QUIET_KEPT),
            open: None,
            level_db: None,
            found: Vec::new(),
            trend: Vec::new(),
            next_minute: (origin_s / 60.0).floor().max(0.0) as u64,
        })
    }

    /// The working rate after the front end.
    pub fn work_rate(&self) -> f64 {
        self.fs
    }

    /// Seconds between spectra.
    pub fn row_s(&self) -> f64 {
        self.row_s
    }

    /// Sets the transient threshold (0 for the default).
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = threshold_of(db);
    }

    /// The signal time of working sample `k`, s.
    fn time_of(&self, k: f64) -> f64 {
        self.origin_s + k / self.fs + (self.dec as f64 - 1.0) / (2.0 * self.rate)
    }

    /// Feeds real samples at the input rate. Each working sample goes through the transient band, then completes a spectrum
    /// when it is the last of one, then closes a trend minute when it is the last of one.
    pub fn process(&mut self, x: &[f32]) {
        self.out.clear();
        self.front.process(x, &mut self.out);
        let out = std::mem::take(&mut self.out);
        for &v in &out {
            let v = if v.is_finite() { v } else { 0.0 };
            self.transient_sample(f64::from(v));
            self.samples.push(v);
            self.k += 1;
            let start = self.rows * HOP as u64;
            if self.k == start + FFT_N as u64 {
                let at = (start - self.samples_first) as usize;
                for (f, &v) in self.frame.iter_mut().zip(&self.samples[at..at + FFT_N]) {
                    *f = Complex32::new(v, 0.0);
                }
                self.make_row();
                self.rows += 1;
                let keep_from = self.rows * HOP as u64;
                let drop = (keep_from - self.samples_first) as usize;
                self.samples.drain(..drop);
                self.samples_first = keep_from;
            }
            let now = self.time_of(self.k as f64);
            while (self.next_minute + 1) as f64 * 60.0 <= now {
                let m = self.next_minute;
                let r = self.resonances[0].unwrap_or_default();
                self.trend.push((m, [r.frequency_hz, r.height_db]));
                self.next_minute += 1;
            }
        }
        self.out = out;
    }

    /// One working sample through the band filter into the energy of its group; a full group is judged.
    fn transient_sample(&mut self, x: f64) {
        let (a, y, xp) = self.hp;
        let h = a * (y + x - xp);
        self.hp = (a, h, x);
        let (b, l1, l2) = self.lp;
        let l1 = l1 + b * (h - l1);
        let l2 = l2 + b * (l1 - l2);
        self.lp = (b, l1, l2);
        let sign = l2 >= 0.0;
        if self.group_n > 0 && sign != self.last_sign {
            self.group_crossings += 1;
        }
        self.last_sign = sign;
        self.group_sum += l2 * l2;
        self.group_n += 1;
        if self.group_n == self.group_len {
            let e = self.group_sum / self.group_len as f64;
            let end_k = self.k + 1;
            let crossings = self.group_crossings;
            self.group_sum = 0.0;
            self.group_n = 0;
            self.group_crossings = 0;
            self.judge(e, end_k, crossings);
        }
    }

    /// Judges the energy `e` of the group that ends before working sample `end_k`.
    fn judge(&mut self, e: f64, end_k: u64, crossings: u64) {
        let db_of = |v: f64| 10.0 * v.max(1e-30).log10();
        self.level_db = Some(db_of(e));
        let comparison = (self.quiet.len() >= JUDGE_AFTER).then(|| {
            self.scratch.clear();
            self.scratch.extend(self.quiet.iter().copied());
            let m = self.scratch.len() / 2;
            *self.scratch.select_nth_unstable_by(m, f64::total_cmp).1
        });
        let over = comparison.map(|c| db_of(e) - db_of(c));
        if let (Some(over), Some(c)) = (over, comparison) {
            if over >= self.threshold_db {
                let start_s = self.time_of((end_k - self.group_len as u64) as f64);
                let o = self.open.get_or_insert(Open {
                    start_s,
                    groups: 0,
                    peak_db: over,
                    crossings: 0,
                });
                o.groups += 1;
                o.peak_db = o.peak_db.max(over);
                o.crossings += crossings;
                return;
            }
            if let Some(o) = self.open.take() {
                let duration = o.groups as f64 * self.group_len as f64 / self.fs;
                let f = o.crossings as f64 / (2.0 * duration);
                self.found.push(EventRow {
                    signal_time_s: o.start_s,
                    kind: "transient".into(),
                    frequency_hz: f,
                    peak_db: o.peak_db,
                    duration_s: duration,
                    aux1: db_of(c),
                    aux2: 0.0,
                    note: format!("ELF transient, {f:.1} Hz; not classified as a Q-burst"),
                });
            }
        }
        if self.quiet.len() >= QUIET_KEPT {
            self.quiet.pop_front();
        }
        self.quiet.push_back(e);
    }

    /// The spectrum of the frame: the average, the baseline, the spectrogram row and the resonances.
    fn make_row(&mut self) {
        let p = self.fft.run_power(&self.frame);
        for (dst, &v) in self.power.iter_mut().zip(p) {
            *dst = f64::from(v * self.norm);
        }
        let count = self.rows + 1;
        let alpha = (1.0 / count as f64).max(self.row_s / AVERAGE_S);
        for (a, &v) in self.average.iter_mut().zip(&self.power) {
            *a += alpha * (v - *a);
        }
        // The baseline: the median of the average over ±5 Hz (the upper middle of an even count).
        let last = self.average.len() - 1;
        for k in self.baseline_bins.clone() {
            let lo = k.saturating_sub(self.half);
            let hi = (k + self.half).min(last);
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.average[lo..=hi]);
            let m = self.scratch.len() / 2;
            self.baseline[k] = self
                .scratch
                .select_nth_unstable_by(m, f64::total_cmp)
                .1
                .max(1e-30);
        }
        let db = |v: f64| power_to_db(v) as f32;
        let row: Vec<f32> = self
            .display_bins
            .clone()
            .collect::<Vec<_>>()
            .chunks(DISPLAY_GROUP)
            .map(|g| {
                g.iter()
                    .map(|&k| db(self.power[k] / self.baseline[k]))
                    .fold(f32::NEG_INFINITY, f32::max)
            })
            .collect();
        if self.display.len() >= DISPLAY_KEPT {
            self.display.pop_front();
        }
        self.display.push_back(row);
        self.display_changed = true;
        if count as f64 * self.row_s >= PEAKS_AFTER_S {
            for (i, &f) in gen::RESONANCES_HZ.iter().enumerate() {
                self.resonances[i] = self.peak(f);
            }
        }
    }

    /// The ratio of the average to the baseline at bin `k`.
    fn ratio(&self, k: usize) -> f64 {
        self.average[k] / self.baseline[k]
    }

    /// The resonance within ±1.6 Hz of `centre`: the largest ratio of the average to the baseline, 2 dB or more, its frequency
    /// from a parabola through the logarithms of three ratios, its width from the run of bins above half its excess.
    fn peak(&self, centre: f64) -> Option<Resonance> {
        let lo = ((centre - PEAK_WINDOW_HZ) / self.bin_hz).ceil().max(1.0) as usize;
        let hi = (((centre + PEAK_WINDOW_HZ) / self.bin_hz).floor() as usize)
            .min(*self.baseline_bins.end());
        let kp =
            (lo..=hi).max_by(|&a, &b| self.ratio(a).total_cmp(&self.ratio(b)).then(b.cmp(&a)))?;
        let rp = self.ratio(kp);
        if 10.0 * rp.log10() < PEAK_DB {
            return None;
        }
        let fit = log_parabolic(self.ratio(kp - 1), rp, self.ratio(kp + 1));
        let f = (kp as f64 + fit.offset) * self.bin_hz;
        let half = 1.0 + (rp - 1.0) / 2.0;
        let (mut a, mut b) = (kp, kp);
        while a > lo && self.ratio(a - 1) > half {
            a -= 1;
        }
        while b < hi && self.ratio(b + 1) > half {
            b += 1;
        }
        let width = (b - a).max(1) as f64 * self.bin_hz;
        Some(Resonance {
            frequency_hz: f,
            height_db: 10.0 * fit.value.max(rp).log10(),
            q: f / width,
        })
    }

    /// The resonances 1 to 5, `None` where none was found.
    pub fn resonances(&self) -> &[Option<Resonance>; 5] {
        &self.resonances
    }

    /// Seconds of averaging.
    pub fn averaged_s(&self) -> f64 {
        self.rows as f64 * self.row_s
    }

    /// The level of the last group of the transient band, dB.
    pub fn level_db(&self) -> Option<f64> {
        self.level_db
    }

    /// The transients found since the last call.
    pub fn take_found(&mut self) -> Vec<EventRow> {
        std::mem::take(&mut self.found)
    }

    /// The trend rows written since the last call: the minute, the fundamental (Hz, 0 when none) and its height (dB).
    pub fn take_trend(&mut self) -> Vec<(u64, [f64; 2])> {
        std::mem::take(&mut self.trend)
    }

    /// The average spectrum of 3 to 45 Hz: frequency (Hz) and dB over the baseline.
    pub fn average_spectrum(&self) -> impl Iterator<Item = [f32; 2]> + '_ {
        self.display_bins.clone().map(|k| {
            [
                (k as f64 * self.bin_hz) as f32,
                (10.0 * self.ratio(k).max(1e-30).log10()) as f32,
            ]
        })
    }

    /// Spectra made.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Writes the spectrogram rows into `r` when they changed since the last call. Returns whether they changed.
    pub fn display(&mut self, r: &mut Raster) -> bool {
        if !std::mem::take(&mut self.display_changed) {
            return false;
        }
        r.width = self.display.front().map_or(0, Vec::len);
        r.height = self.display.len();
        r.x = Axis::new(
            (*self.display_bins.start() as f64 + (DISPLAY_GROUP as f64 - 1.0) / 2.0) * self.bin_hz,
            DISPLAY_GROUP as f64 * self.bin_hz,
            "Hz",
        );
        let newest = self.rows.saturating_sub(1) as f64 * HOP as f64 + FFT_N as f64 / 2.0;
        let newest_t = self.time_of(newest);
        r.y = Axis::new(
            newest_t - self.display.len().saturating_sub(1) as f64 * self.row_s,
            self.row_s,
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

/// The spectrogram: 3 to 45 Hz, the ratio of each spectrum to the baseline.
pub const SPECTROGRAM: RasterDef = RasterDef {
    key: "spectrogram",
    label: "Spectrogram",
    x_unit: "Hz",
    y_unit: "s",
    unit: "dB",
};

/// The average spectrum over the baseline, 3 to 45 Hz.
pub const AVERAGE: SeriesDef = SeriesDef {
    key: "average_spectrum",
    label: "Average spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The `reset` command: the monitor starts again (average, baseline and comparison level are learned again).
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Empties the tables and starts the monitor again; the average and the comparison level are learned again.");
static COMMANDS: [&Command; 1] = [&RESET];

/// The UTC of sample 0: the setting, else the host clock now.
fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The `schumann` decoder: the [`Monitor`] with its tables, trend, counts, spectrogram and average spectrum.
pub struct SchumannDecoder {
    mon: Option<Monitor>,
    rate: f64,
    threshold_set: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    input: u64,
    reals: Vec<f32>,
    log: EventLog,
    trend: TrendLog,
    events: Vec<Event>,
    raster: Raster,
    raster_id: Option<RasterId>,
    average_id: Option<SeriesId>,
    rows_shown: u64,
    started: bool,
    status: Status,
}

impl Default for SchumannDecoder {
    fn default() -> Self {
        SchumannDecoder {
            mon: None,
            rate: 0.0,
            threshold_set: THRESHOLD.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            input: 0,
            reals: Vec::new(),
            log: EventLog::default(),
            trend: TrendLog::default(),
            events: Vec::new(),
            raster: Raster::default(),
            raster_id: None,
            average_id: None,
            rows_shown: u64::MAX,
            started: false,
            status: Status::default(),
        }
    }
}

impl SchumannDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    /// The tables emptied and the monitor made again from the next sample.
    fn reset(&mut self) {
        self.log.clear();
        self.trend.clear();
        if self.rate > 0.0 {
            self.mon = Monitor::new(self.rate, self.threshold_set, self.now()).ok();
        }
        self.raster = Raster::default();
        self.rows_shown = u64::MAX;
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(m) = self.mon.as_mut() else {
            return;
        };
        for row in m.take_found() {
            self.log.push(row, self.start_utc, &mut self.events);
        }
        for (minute, v) in m.take_trend() {
            self.trend.push(minute, v);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
}

impl Decoder for SchumannDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.schumann"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        MonitorSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MonitorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MonitorSetting::Threshold => {
                let v = THRESHOLD.read(id, value)?;
                if v > 0.0 && v < 1.0 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 1.0,
                        max: 20.0,
                        got: v,
                    });
                }
                self.threshold_set = v;
                if let Some(m) = self.mon.as_mut() {
                    m.set_threshold(v);
                }
            }
            MonitorSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (MonitorSetting::Threshold, Value::F64(self.threshold_set)),
            (MonitorSetting::StartUtc, Value::F64(self.start_setting)),
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
        if input.rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: input.rate,
            });
        }
        self.rate = input.rate;
        self.input = 0;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.mon = Some(Monitor::new(input.rate, self.threshold_set, 0.0)?);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        self.reals.clear();
        self.reals.extend(x.iter().map(|z| z.re));
        if let Some(m) = self.mon.as_mut() {
            m.process(&self.reals);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let Some(m) = self.mon.as_mut() else {
            self.status.lock_state = LockState::Off;
            p.results_mut().set_status(&self.status);
            return;
        };
        let res = *m.resonances();
        let first = res[0];
        let found = res.iter().flatten().count();
        if m.rows() != self.rows_shown {
            self.rows_shown = m.rows();
            if m.averaged_s() >= PEAKS_AFTER_S {
                for (i, r) in res.iter().enumerate() {
                    let r = r.unwrap_or_default();
                    SCHUMANN_PEAKS.put(
                        p,
                        &format!("{}", i + 1),
                        &SchumannPeakRow {
                            resonance: i as i64 + 1,
                            frequency_hz: Hertz(r.frequency_hz),
                            height_db: r.height_db,
                            q: r.q,
                        },
                    );
                }
            } else {
                p.clear_rows(&SCHUMANN_PEAKS);
            }
            if self.average_id.is_none() {
                self.average_id = p.results().decl().series_id(AVERAGE.key);
            }
            if let Some(id) = self.average_id {
                if m.rows() > 0 {
                    write_series(p.results_mut(), id, true, m.average_spectrum());
                }
            }
        }
        if m.display(&mut self.raster) {
            if self.raster_id.is_none() {
                self.raster_id = p.results().decl().raster_id(SPECTROGRAM.key);
            }
            if let Some(id) = self.raster_id {
                p.results_mut()
                    .update_raster(id, |r| copy_if_changed(r, &self.raster));
            }
        }
        let rows = m.rows();
        let averaged = m.averaged_s();
        let level_db = m.level_db();
        self.log.publish(p, self.start_utc);
        self.trend.publish(
            p,
            &SCHUMANN_TREND,
            self.start_utc,
            |utc, [fundamental_hz, height_db]| SchumannTrendRow {
                utc,
                fundamental_hz,
                height_db,
            },
        );
        let st = &mut self.status;
        st.lock_state = match (self.started, rows > 0, first.is_some()) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.summary.clear();
        st.summary.push_str(&match first {
            _ if !self.started => "Schumann: waiting for samples".to_string(),
            _ if rows == 0 => "Schumann: filling the first spectrum (10.2 s)".to_string(),
            Some(r) => format!(
                "Schumann: fundamental {:.2} Hz, {:.1} dB, Q {:.1}; {found} of 5 resonances",
                r.frequency_hz, r.height_db, r.q
            ),
            None if averaged < PEAKS_AFTER_S => {
                format!("Schumann: averaging, {averaged:.0} of 60 s")
            }
            None => "Schumann: no resonance found".to_string(),
        });
        p.put(&SchumannFields {
            text: &st.summary,
            fundamental_hz: first.map(|r| Hertz(r.frequency_hz)),
            fundamental_db: first.map(|r| r.height_db),
            fundamental_q: first.map(|r| r.q),
            peaks_found: found as i64,
            averaged_s: Seconds(averaged),
            level_db,
            events_total: i64::try_from(self.log.total()).unwrap_or(i64::MAX),
            note: NOTE_TEXT,
        });
        p.results_mut().set_status(st);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.log.clear();
        self.trend.clear();
    }
}

/// The receiver: one decoder on the input.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("dec", DecoderComponent::new(SchumannDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE_HZ,
            "The Schumann monitor needs a sample rate of at least 400 sps",
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
    r.add("science.schumann", || {
        Box::new(DecoderComponent::new(SchumannDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::SchumannGenerator::from_config(cfg)?))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 90 s", o, 0.0);
    // The first spectrum needs 10.24 s and the resonances 60 s of averaging.
    c.seconds = o.seconds.max(90.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::family::field(r, SchumannFields::FUNDAMENTAL_HZ.key);
        vec![CheckLine::new(
            SchumannFields::FUNDAMENTAL_HZ.key,
            (f - 7.83).abs() <= 0.8,
            format!("{f:.2} Hz, 7.83 ± 0.8 expected"),
        )]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The Schumann resonance monitor.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Schumann resonances",
    group: "Science, time and utility",
    blurb: "From an ELF sensor on a sound card: the five lowest Schumann resonances (frequency, height, Q) over a 5-minute \
            average, a spectrogram of 3 to 45 Hz, the fundamental minute by minute, and ELF transients.",
    tuning: Tuning {
        min_hz: 0.0,
        max_hz: 0.1e6,
        default_hz: 0.0,
        rate_hz: 48e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 100.0,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: true,
        radio_filter_hz: Some(10e3),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: SchumannSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: None,
};
