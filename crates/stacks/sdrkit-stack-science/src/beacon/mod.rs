//! Beacon openings (`beacon_events`): follows a carrier in a ±1 kHz window around the tuned beacon and logs every opening with
//! its start, duration, peak S/N, width, shift and flutter, and a class by the character of the signal.
//!
//! Front end: the input is averaged in groups of max(1, round(rate / 2000)) samples to about 2 ksps; transforms of 1024 points
//! (Hann) without overlap give rows every 0.512 s of bins of about 1.95 Hz, centred on the dial. The window correlates
//! neighbouring bins, so noise alone reaches a z of 12 in about 1 % of the rows: such a single hit keeps an open opening
//! open, but four in a row are needed to open one. Per row: the noise `nm` (the
//! median bin over 0.693); a scan of windows of 5, 13, 31, 77 and 155 bins, sliding by a quarter of their width, with
//! `z = (sum - w nm) / (nm sqrt(w))`; the best z of all; the narrowest window whose own best z is 0.85 of that or more gives the
//! centre, the excess and the S/N; the width is the larger of that window and 2.355 × the rms spread of the bins over 4 nm within
//! ±100 bins of the centre (a Gaussian full width at half height).
//!
//! Openings: a row is a hit when its best z is the threshold (12 by default) or more. An opening opens after 4 hits in a row
//! (its start is the first of them) and closes 10 s after its last hit. Any miss resets the count of hits; while no opening is
//! open, a miss also forgets the rows collected. The width and the shift of an opening are the medians of its hit rows, the
//! flutter the standard deviation of their S/N without the first and the last, the peak their largest S/N. Classes, by the
//! signal only (no public numeric split; the path is not proven by any): width 40 Hz or more "Doppler-spread (aurora-like)",
//! else flutter 3 dB or more "fluttering carrier (TEP / scatter-like)", else "steady carrier (sporadic-E-like)".

pub mod gen;

use crate::events::{EventLog, EventRow, TrendLog};
use crate::typed::{BeaconEventsFields, BeaconEventsTrendRow, BEACON_EVENTS_TREND};
use sdrkit_core::interface::Hertz;
use crate::family::REPORT_S;
use crate::mscatter::START_UTC;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::floor::BinMedian;
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
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "beacon_events";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 8e3;
/// The rate the front end aims at, Hz.
pub const WORK_RATE_HZ: f64 = 2000.0;
/// The transform size and hop.
pub const FFT_N: usize = 1024;
/// The scan windows, bins.
pub const WINDOWS: [usize; 5] = [5, 13, 31, 77, 155];
/// The share of the best z that the width window must reach.
const WIDTH_SHARE: f64 = 0.85;
/// The bins either side of the centre for the spread.
const SPREAD_BINS: usize = 100;
/// The bins over this many noise levels weigh in the spread.
const SPREAD_NOISE: f64 = 4.0;
/// A Gaussian's full width at half height over its standard deviation.
const FWHM: f64 = 2.355;
/// Hits in a row that open an opening.
const OPEN_HITS: usize = 4;
/// Time without a hit that closes an opening, s.
const CLOSE_S: f64 = 10.0;
/// The threshold when the setting is 0.
pub const DEFAULT_THRESHOLD: f64 = 12.0;
/// The width of a Doppler-spread opening, Hz.
const SPREAD_HZ: f64 = 40.0;
/// The flutter of a fluttering opening, dB.
const FLUTTER_DB: f64 = 3.0;
/// The S/N of a row without excess, dB.
const NO_SNR_DB: f64 = -10.0;
/// Display rows kept.
pub const DISPLAY_KEPT: usize = 120;
/// Bins of a display cell.
const DISPLAY_GROUP: usize = 4;
/// The limits of a display value, dB over the noise.
const DISPLAY_DB: (f32, f32) = (-5.0, 40.0);
/// A trend minute is written this long after its end, s.
const TREND_DELAY_S: f64 = 15.0;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: every rule of the detector (the windows of 5 to 155 bins, \
the threshold z of 12, 4 hits to open and 10 s to close, the width at 0.85 of the best z and from the bins over 4 noise \
levels) and the classes (40 Hz and 3 dB), which describe the signal, not the path.";

/// The opening threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_z", "Opening threshold", "z", DEFAULT_THRESHOLD)
        .range(0.0, 30.0, 0.0)
        .zero_means("12")
        .help("The best scan z of a hit row: 4 to 30, 0 for 12.");

sdrkit_core::settings! {
    /// The settings of [`BeaconDecoder`].
    pub enum DecoderSetting {
        /// The threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `beacon_events` stack.
    pub enum BeaconSetting {
        /// The threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The level of the test signal.
        GenLevel = gen::GEN_LEVEL_DB,
        /// The spread of the test signal.
        GenSpread = gen::GEN_SPREAD_HZ,
        /// The flutter of the test signal.
        GenFlutter = gen::GEN_FLUTTER_DB,
    }
}

fn threshold_of(v: f64) -> f64 {
    if v > 0.0 {
        v
    } else {
        DEFAULT_THRESHOLD
    }
}

/// The measures of one row.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowMeasure {
    /// The noise per bin.
    pub noise: f64,
    /// The best z.
    pub z: f64,
    /// The S/N of the width window, dB.
    pub snr_db: f64,
    /// The width, Hz.
    pub width_hz: f64,
    /// The shift of the centre from the dial, Hz.
    pub shift_hz: f64,
}

/// The scan of one row of `n` power bins (DC at `n / 2`) with noise `nm` and bins of `bin_hz` (module docs).
pub fn measure(p: &[f32], nm: f64, bin_hz: f64) -> RowMeasure {
    let n = p.len();
    let mut prefix = vec![0.0f64; n + 1];
    for (i, &v) in p.iter().enumerate() {
        prefix[i + 1] = prefix[i] + f64::from(v);
    }
    let nm = nm.max(1e-30);
    // The best window of each size: (z, start, excess).
    let best: Vec<(f64, usize, f64)> = WINDOWS
        .iter()
        .map(|&w| {
            let w = w.min(n);
            let step = (w / 4).max(1);
            let mut b = (f64::NEG_INFINITY, 0, 0.0);
            let mut s = 0;
            while s + w <= n {
                let sum = prefix[s + w] - prefix[s];
                let z = (sum - w as f64 * nm) / (nm * (w as f64).sqrt());
                if z > b.0 {
                    b = (z, s, sum - w as f64 * nm);
                }
                s += step;
            }
            b
        })
        .collect();
    let best_z = best.iter().map(|b| b.0).fold(f64::NEG_INFINITY, f64::max);
    let (i, &(_, start, excess)) = best
        .iter()
        .enumerate()
        .find(|(_, b)| b.0 >= WIDTH_SHARE * best_z)
        .unwrap_or((0, &best[0]));
    let w = WINDOWS[i].min(n);
    let centre = start as f64 + (w as f64 - 1.0) / 2.0;
    let snr_db = if excess > 0.0 {
        10.0 * (excess / nm).log10()
    } else {
        NO_SNR_DB
    };
    let c = centre.round() as usize;
    let lo = c.saturating_sub(SPREAD_BINS);
    let hi = (c + SPREAD_BINS).min(n - 1);
    let (mut sw, mut swk) = (0.0, 0.0);
    for (k, &v) in p.iter().enumerate().take(hi + 1).skip(lo) {
        let wt = (f64::from(v) - SPREAD_NOISE * nm).max(0.0);
        sw += wt;
        swk += wt * k as f64;
    }
    let rms = if sw > 0.0 {
        let mu = swk / sw;
        let var = p[lo..=hi]
            .iter()
            .enumerate()
            .map(|(j, &v)| {
                let wt = (f64::from(v) - SPREAD_NOISE * nm).max(0.0);
                let d = (lo + j) as f64 - mu;
                wt * d * d
            })
            .sum::<f64>()
            / sw;
        var.sqrt()
    } else {
        0.0
    };
    RowMeasure {
        noise: nm,
        z: best_z,
        snr_db,
        width_hz: (w as f64 * bin_hz).max(FWHM * rms * bin_hz),
        shift_hz: (centre - (n / 2) as f64) * bin_hz,
    }
}

/// The class of an opening of median width `width_hz` and flutter `flutter_db`.
pub fn class_of(width_hz: f64, flutter_db: f64) -> &'static str {
    if width_hz >= SPREAD_HZ {
        "Doppler-spread (aurora-like)"
    } else if flutter_db >= FLUTTER_DB {
        "fluttering carrier (TEP / scatter-like)"
    } else {
        "steady carrier (sporadic-E-like)"
    }
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s.get(s.len() / 2).copied().unwrap_or(0.0)
}

#[derive(Clone, Debug, Default)]
struct Open {
    start_s: f64,
    last_s: f64,
    rows: Vec<RowMeasure>,
}

/// The detector at one input rate (module docs). Every decision follows the row index.
pub struct OpeningDetector {
    dec: usize,
    fs: f64,
    row_s: f64,
    bin_hz: f64,
    threshold: f64,
    origin_s: f64,
    front: FirDecimator<Complex32>,
    out: Vec<Complex32>,
    frame: Vec<Complex32>,
    fft: WindowedFft,
    median: BinMedian,
    row: Vec<f32>,
    rows: u64,
    hits: usize,
    collected: Vec<RowMeasure>,
    open: Option<Open>,
    last: RowMeasure,
    found: Vec<EventRow>,
    minute: Option<(u64, f64, f64, u32)>,
    trend: Vec<(u64, [f64; 2])>,
    display: VecDeque<Vec<f32>>,
    display_changed: bool,
}

impl OpeningDetector {
    /// A detector for input at `rate` whose first sample is at `origin_s`, with the threshold `threshold` (0 for 12).
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`MIN_RATE_HZ`].
    pub fn new(rate: f64, threshold: f64, origin_s: f64) -> Result<Self, ConfigError> {
        if !rate.is_finite() || rate < MIN_RATE_HZ * 0.999_999 {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: rate,
            });
        }
        let dec = ((rate / WORK_RATE_HZ + 0.5).floor() as usize).max(1);
        let fs = rate / dec as f64;
        Ok(OpeningDetector {
            dec,
            fs,
            row_s: FFT_N as f64 / fs,
            bin_hz: fs / FFT_N as f64,
            threshold: threshold_of(threshold),
            origin_s,
            front: FirDecimator::new(&vec![1.0 / dec as f32; dec], dec),
            out: Vec::new(),
            frame: Vec::with_capacity(FFT_N),
            fft: WindowedFft::new(FFT_N, FFT_N, Window::Hann).expect("1024 is a valid size"),
            median: BinMedian::new(1).expect("one frame"),
            row: vec![0.0; FFT_N],
            rows: 0,
            hits: 0,
            collected: Vec::new(),
            open: None,
            last: RowMeasure::default(),
            found: Vec::new(),
            minute: None,
            trend: Vec::new(),
            display: VecDeque::with_capacity(DISPLAY_KEPT),
            display_changed: false,
        })
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }

    /// The time of a row, s.
    pub fn row_s(&self) -> f64 {
        self.row_s
    }

    /// Sets the threshold (0 for 12).
    pub fn set_threshold(&mut self, v: f64) {
        self.threshold = threshold_of(v);
    }

    /// The measures of the last row.
    pub fn last(&self) -> RowMeasure {
        self.last
    }

    /// Whether an opening is open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Rows made.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    fn row_time(&self, r: u64) -> f64 {
        self.origin_s + r as f64 * self.row_s + (self.dec as f64 - 1.0) / (2.0 * self.fs * self.dec as f64)
    }

    /// Feeds samples at the input rate.
    pub fn process(&mut self, x: &[Complex32]) {
        self.out.clear();
        self.front.process(x, &mut self.out);
        let half = FFT_N / 2;
        for i in 0..self.out.len() {
            let z = self.out[i];
            self.frame.push(if z.re.is_finite() && z.im.is_finite() {
                z
            } else {
                Complex32::default()
            });
            if self.frame.len() == FFT_N {
                let p = self.fft.run_power(&self.frame);
                for (k, r) in self.row.iter_mut().enumerate() {
                    *r = p[(k + half) % FFT_N];
                }
                self.frame.clear();
                self.make_row();
            }
        }
    }

    fn make_row(&mut self) {
        let r = self.rows;
        self.rows += 1;
        let nm = f64::from(self.median.estimate(self.row.iter().copied()).unwrap_or(0.0));
        let m = measure(&self.row, nm, self.bin_hz);
        self.last = m;
        let t = self.row_time(r);
        self.display_row(nm);
        let hit = m.z >= self.threshold;
        if hit {
            self.hits += 1;
            let minute = (t / 60.0).floor().max(0.0) as u64;
            match self.minute.as_mut() {
                Some(e) if e.0 == minute => {
                    e.1 = e.1.max(m.snr_db);
                    e.2 += m.width_hz;
                    e.3 += 1;
                }
                _ => {
                    self.flush_minute();
                    self.minute = Some((minute, m.snr_db, m.width_hz, 1));
                }
            }
            match self.open.as_mut() {
                Some(o) => {
                    o.last_s = t;
                    o.rows.push(m);
                }
                None => {
                    self.collected.push(m);
                    if self.hits >= OPEN_HITS {
                        let rows = std::mem::take(&mut self.collected);
                        let first = rows.len() - OPEN_HITS;
                        self.open = Some(Open {
                            start_s: t - (OPEN_HITS - 1) as f64 * self.row_s,
                            last_s: t,
                            rows: rows[first..].to_vec(),
                        });
                    }
                }
            }
        } else {
            self.hits = 0;
            if self.open.is_none() {
                self.collected.clear();
            }
        }
        if let Some(o) = &self.open {
            if t - o.last_s >= CLOSE_S {
                let o = self.open.take().expect("open");
                self.close(o);
            }
        }
        if self.minute.is_some_and(|e| (e.0 + 1) as f64 * 60.0 + TREND_DELAY_S <= t) {
            self.flush_minute();
        }
    }

    fn flush_minute(&mut self) {
        if let Some((m, peak, w, n)) = self.minute.take() {
            self.trend.push((m, [peak, w / f64::from(n)]));
        }
    }

    fn close(&mut self, o: Open) {
        if o.rows.len() < OPEN_HITS {
            return;
        }
        let widths: Vec<f64> = o.rows.iter().map(|m| m.width_hz).collect();
        let shifts: Vec<f64> = o.rows.iter().map(|m| m.shift_hz).collect();
        let inner: Vec<f64> = o.rows[1..o.rows.len() - 1].iter().map(|m| m.snr_db).collect();
        let flutter = if inner.len() >= 2 {
            let mean = inner.iter().sum::<f64>() / inner.len() as f64;
            (inner.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / inner.len() as f64).sqrt()
        } else {
            0.0
        };
        let width = median(&widths);
        let peak = o.rows.iter().map(|m| m.snr_db).fold(f64::NEG_INFINITY, f64::max);
        let class = class_of(width, flutter);
        self.found.push(EventRow {
            signal_time_s: o.start_s,
            kind: "opening".into(),
            frequency_hz: median(&shifts),
            peak_db: peak,
            duration_s: o.last_s + self.row_s - o.start_s,
            aux1: width,
            aux2: flutter,
            note: class.into(),
        });
    }

    fn display_row(&mut self, nm: f64) {
        if self.display.len() >= DISPLAY_KEPT {
            self.display.pop_front();
        }
        let nm = nm.max(1e-30) as f32;
        self.display.push_back(
            self.row
                .chunks(DISPLAY_GROUP)
                .map(|c| {
                    let v = c.iter().copied().fold(0f32, f32::max);
                    (10.0 * (v / nm).max(1e-30).log10()).clamp(DISPLAY_DB.0, DISPLAY_DB.1)
                })
                .collect(),
        );
        self.display_changed = true;
    }

    /// The openings finished since the last call.
    pub fn take_found(&mut self) -> Vec<EventRow> {
        std::mem::take(&mut self.found)
    }

    /// The trend rows written since the last call: the minute, the peak S/N and the mean width of its hit rows.
    pub fn take_trend(&mut self) -> Vec<(u64, [f64; 2])> {
        std::mem::take(&mut self.trend)
    }

    /// Writes the display rows into `r` when they changed. Returns whether they changed.
    pub fn display(&mut self, r: &mut Raster) -> bool {
        if !std::mem::take(&mut self.display_changed) {
            return false;
        }
        r.width = FFT_N / DISPLAY_GROUP;
        r.height = self.display.len();
        let cell = DISPLAY_GROUP as f64 * self.bin_hz;
        r.x = Axis::new(-((FFT_N / 2) as f64) * self.bin_hz + cell / 2.0, cell, "Hz");
        let last = self.row_time(self.rows.saturating_sub(1));
        r.y = Axis::new(last - self.display.len().saturating_sub(1) as f64 * self.row_s, self.row_s, "s");
        r.unit.clear();
        r.unit.push_str("dB");
        r.values.clear();
        for row in &self.display {
            r.values.extend_from_slice(row);
        }
        true
    }
}

/// The display: ±1 kHz, 4 bins a cell, dB over the noise, a row every 0.512 s.
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

/// The `reset` command.
pub const RESET: Command = Command::new("reset", "Reset")
    .help("Empties the tables and starts the detector again.");
static COMMANDS: [&Command; 1] = [&RESET];

fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The `beacon_events` decoder.
pub struct BeaconDecoder {
    det: Option<OpeningDetector>,
    rate: f64,
    threshold: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    input: u64,
    log: EventLog,
    trend: TrendLog,
    openings: u64,
    events: Vec<Event>,
    raster: Raster,
    raster_id: Option<RasterId>,
    started: bool,
    status: Status,
}

impl Default for BeaconDecoder {
    fn default() -> Self {
        BeaconDecoder {
            det: None,
            rate: 0.0,
            threshold: THRESHOLD.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            input: 0,
            log: EventLog::default(),
            trend: TrendLog::default(),
            openings: 0,
            events: Vec::new(),
            raster: Raster::default(),
            raster_id: None,
            started: false,
            status: Status::default(),
        }
    }
}

impl BeaconDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn forget(&mut self) {
        self.log.clear();
        self.trend.clear();
        self.openings = 0;
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(det) = self.det.as_mut() else {
            return;
        };
        for row in det.take_found() {
            self.openings += 1;
            self.log.push(row, self.start_utc, &mut self.events);
        }
        for (m, v) in det.take_trend() {
            self.trend.push(m, v);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
}

impl Decoder for BeaconDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.beacon_events"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DecoderSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecoderSetting::Threshold => {
                let v = THRESHOLD.read(id, value)?;
                if v > 0.0 && v < 4.0 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 4.0,
                        max: 30.0,
                        got: v,
                    });
                }
                self.threshold = v;
                if let Some(d) = self.det.as_mut() {
                    d.set_threshold(v);
                }
            }
            DecoderSetting::StartUtc => {
                self.start_setting = START_UTC.read(id, value)?;
                self.start_utc = start_utc_of(self.start_setting, &self.clock);
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (DecoderSetting::Threshold, Value::F64(self.threshold)),
            (DecoderSetting::StartUtc, Value::F64(self.start_setting)),
        ])
    }
    fn commands(&self) -> &'static [&'static Command] {
        &COMMANDS
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        if id.index() == 0 {
            self.forget();
            if self.rate > 0.0 {
                self.det = OpeningDetector::new(self.rate, self.threshold, self.now()).ok();
            }
            self.raster = Raster::default();
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
        self.det = Some(OpeningDetector::new(input.rate, self.threshold, 0.0)?);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, x: &[Complex32], first: u64, out: &mut Outbox) {
        self.started = true;
        self.input = first + x.len() as u64;
        if let Some(d) = self.det.as_mut() {
            d.process(x);
        }
        self.take(out);
    }
    fn report(&mut self, p: &mut Published) {
        let (m, open, rows) = self
            .det
            .as_ref()
            .map_or((RowMeasure::default(), false, 0), |d| (d.last(), d.is_open(), d.rows()));
        let have = rows > 0;
        self.log.publish(p, self.start_utc);
        self.trend.publish(
            p,
            &BEACON_EVENTS_TREND,
            self.start_utc,
            |utc, [peak_snr_db, mean_width_hz]| BeaconEventsTrendRow {
                utc,
                peak_snr_db,
                mean_width_hz,
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
        st.lock_state = match (self.started, open, self.openings > 0) {
            (false, ..) => LockState::Off,
            (_, true, _) => LockState::Locked,
            (_, false, true) => LockState::Tracking,
            (_, false, false) => LockState::Searching,
        };
        st.snr_db = have.then_some(m.snr_db as f32);
        st.offset_hz = (have && open).then_some(m.shift_hz);
        st.summary.clear();
        st.summary.push_str(&if !self.started {
            "Beacon: waiting for samples".to_string()
        } else if open {
            format!(
                "Beacon: open, {:.1} dB, {:.0} Hz wide, {:+.0} Hz; {} openings",
                m.snr_db, m.width_hz, m.shift_hz, self.openings
            )
        } else {
            format!("Beacon: closed; {} openings", self.openings)
        });
        p.put(&BeaconEventsFields {
            text: &st.summary,
            openings: i64::try_from(self.openings).unwrap_or(i64::MAX),
            noise_db: have.then(|| 10.0 * m.noise.max(1e-30).log10()),
            open,
            width_hz: have.then_some(Hertz(m.width_hz)),
            z: have.then_some(m.z),
            snr_db: have.then_some(m.snr_db),
            shift_hz: have.then_some(Hertz(m.shift_hz)),
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
        .add("dec", DecoderComponent::new(BeaconDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "Beacon openings need a sample rate of at least 8 ksps")
        .route(THRESHOLD.key(), &["dec.threshold_z"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("dec", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.beacon_events", || {
        Box::new(DecoderComponent::new(BeaconDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = gen::GenParams {
        level_db: cfg.settings.get(&gen::GEN_LEVEL_DB),
        spread_hz: cfg.settings.get(&gen::GEN_SPREAD_HZ),
        flutter_db: cfg.settings.get(&gen::GEN_FLUTTER_DB),
        ..gen::GenParams::default()
    };
    Ok(Box::new(
        gen::BeaconGenerator::new(cfg.rate_hz, p, cfg.seed).with_own_noise(crate::family::own_noise(cfg)),
    ))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, openings at 5 s and 40 s for 18 s", o, 0.0);
    c.seconds = o.seconds.max(75.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::family::field(r, BeaconEventsFields::OPENINGS.key);
        vec![CheckLine::new(BeaconEventsFields::OPENINGS.key, n >= 1.0, format!("{n}, at least 1 expected"))]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// Beacon openings.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Beacon openings",
    group: "Science, time and utility",
    blurb: "Follows a beacon carrier within ±1 kHz and logs every opening: start, duration, peak S/N, width, shift and \
            flutter, with a class by the character of the signal (steady, fluttering or Doppler-spread).",
    tuning: Tuning {
        min_hz: 28e6,
        max_hz: 1300e6,
        default_hz: 50e6,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 200e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: BeaconSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};
