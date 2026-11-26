//! Moon echo test (`eme_echo`): finds the station's own keyed carrier and its echo from the Moon 2.3 to 2.85 s later within
//! ±500 Hz, and measures the echo's delay, Doppler shift, level and spread. The stack listens while the station transmits;
//! it transmits nothing.
//!
//! Front end: the input is averaged in groups of max(1, round(rate / 2000)) samples to about 2 ksps; transforms of 512 points
//! (Hann) every 128 samples give rows of bins of about 3.9 Hz. Per row: the strongest bin and the noise (the median bin over
//! 0.693). The noise of the stack is the mean of the first 60 rows, then follows the rows with a 30 s time constant while the
//! station does not transmit.
//!
//! Own transmission: on when the strongest bin is 25 dB over the noise, off after 3 rows below; a pulse of 0.5 s or more is
//! kept with its carrier bin (the median of the strongest bins) and its power (their mean).
//!
//! Echo search, once per pulse, when 3.9 s + twice its length have passed: a window of `len = max(2, rows - 2)` rows and three
//! bins gives a score (the mean power). Control scores at delays from 3.15 s + len rows to 0.6 s more, every third bin within
//! ±500 Hz, give a mean and a standard deviation; the best score at delays 2.3 to 2.85 s and every bin within ±500 Hz is an echo
//! when it is the threshold (6 by default) or more standard deviations over the mean. Windows that overlap a transmission are
//! skipped. The delay is refined by a parabola through the scores one row either side; the spread is the run of bins whose score
//! is over half the best's excess.
//!
//! Documents: J. Taylor, "Lunar echoes" (QEX); B. Atkins, "EME echo Doppler spreading and lunar libration". The round trip at the
//! mean Earth-Moon distance (384 400 km) is about 2.56 s; public guides give about 300 to 350 Hz of Doppler at 144 MHz.

pub mod gen;

use crate::events::{EventLog, EventRow, TrendLog};
use crate::typed::{EmeEchoFields, EmeEchoTrendRow, EME_ECHO_TREND};
use sdrkit_core::interface::{Hertz, Seconds};
use crate::family::REPORT_S;
use crate::mscatter::START_UTC;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::floor::BinMedian;
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
use std::collections::VecDeque;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "eme_echo";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 8e3;
/// The rate the front end aims at, Hz.
pub const WORK_RATE_HZ: f64 = 2000.0;
/// The transform size.
pub const FFT_N: usize = 512;
/// The hop between rows.
pub const HOP: usize = 128;
/// Rows whose mean is the first noise.
const NOISE_ROWS: u64 = 60;
/// The time constant of the noise after the first rows, s.
const NOISE_TAU_S: f64 = 30.0;
/// The rows kept, s.
const MEMORY_S: f64 = 24.0;
/// The own carrier over the noise, dB.
const TX_DB: f64 = 25.0;
/// Rows below that end a transmission.
const TX_OFF_ROWS: u32 = 3;
/// The least pulse, s.
const MIN_PULSE_S: f64 = 0.5;
/// A pulse is searched this long after its start, plus twice its length, s.
const SEARCH_AFTER_S: f64 = 3.9;
/// The delays searched, s.
const DELAY_S: (f64, f64) = (2.3, 2.85);
/// The first control delay, s, before `len` rows are added.
const CONTROL_S: f64 = 3.15;
/// The span of the control delays, s.
const CONTROL_SPAN_S: f64 = 0.6;
/// Every this many bins in the control.
const CONTROL_BIN_STEP: usize = 3;
/// The least control scores.
const CONTROL_MIN: usize = 20;
/// The Doppler window, Hz.
const DOPPLER_HZ: f64 = 500.0;
/// The echo threshold when the setting is 0, standard deviations.
pub const DEFAULT_THRESHOLD: f64 = 6.0;
/// Display rows kept.
pub const DISPLAY_KEPT: usize = 120;
/// Rows of one display row.
const DISPLAY_ROWS: u64 = 4;
/// The limits of a display value, dB over the noise.
const DISPLAY_DB: (f32, f32) = (-5.0, 40.0);
/// A trend minute is written this long after its end, s.
const TREND_DELAY_S: f64 = 15.0;

/// What no public document confirms.
pub const NOTE_TEXT: &str = "Not confirmed by a public document: every rule of the detector (the 3.9 Hz bins, the noise of \
the first 60 rows then a 30 s time constant, the own carrier at 25 dB over the noise for 0.5 s or more, the search 3.9 s plus \
twice the pulse length after its start, the control delays from 3.15 s plus the window, at least 20 control scores, the \
threshold of 6 standard deviations, the spread at half the best excess). The delay window of 2.3 to 2.85 s covers the \
Earth-Moon distance with margin; ±500 Hz covers the Doppler shift that public guides give at 144 MHz (about 300 to 350 Hz). \
The level of an echo is its three-bin score over the control mean, not a level in 12 Hz. Pulses about 10 s apart: a window \
over another transmission is skipped.";

/// The echo threshold.
pub const THRESHOLD: Setting<field::F64> =
    Setting::new("threshold_sigma", "Echo threshold", "sigma", DEFAULT_THRESHOLD)
        .range(0.0, 20.0, 0.0)
        .zero_means("6 sigma")
        .help("The best echo score over the control mean in standard deviations of the control: 2 to 20, 0 for 6.");

sdrkit_core::settings! {
    /// The settings of [`EmeDecoder`].
    pub enum DecoderSetting {
        /// The echo threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
    }
}

sdrkit_core::settings! {
    /// The settings of the `eme_echo` stack.
    pub enum EmeSetting {
        /// The echo threshold.
        Threshold = THRESHOLD,
        /// The UTC of sample 0.
        StartUtc = START_UTC,
        /// The echo delay of the test signal.
        GenDelay = gen::GEN_DELAY_S,
        /// The echo Doppler of the test signal.
        GenDoppler = gen::GEN_DOPPLER_HZ,
        /// The echo spread of the test signal.
        GenSpread = gen::GEN_SPREAD_HZ,
        /// The echo level of the test signal.
        GenEcho = gen::GEN_ECHO_DB,
    }
}

fn threshold_of(v: f64) -> f64 {
    if v > 0.0 {
        v
    } else {
        DEFAULT_THRESHOLD
    }
}

/// An own pulse waiting for its echo search.
#[derive(Clone, Debug, PartialEq)]
pub struct Pulse {
    /// The first row.
    pub start_row: u64,
    /// Rows.
    pub rows: u64,
    /// The carrier bin.
    pub bin: usize,
    /// The mean power of the strongest bin.
    pub power: f64,
}

#[derive(Clone, Debug, Default)]
struct Tx {
    start: u64,
    last: u64,
    below: u32,
    bins: Vec<usize>,
    powers: Vec<f64>,
}

/// An echo.
#[derive(Clone, Debug, PartialEq)]
pub struct Echo {
    /// The event row.
    pub row: EventRow,
    /// Its z score.
    pub z: f64,
}

/// The detector at one input rate (module docs). Every decision follows the row index, so pieces of any size give the same
/// output.
pub struct EchoDetector {
    dec: usize,
    fs: f64,
    hop_s: f64,
    bin_hz: f64,
    half: usize,
    threshold: f64,
    origin_s: f64,
    front: FirDecimator<Complex32>,
    out: Vec<Complex32>,
    samples: Vec<Complex32>,
    samples_first: u64,
    fft: WindowedFft,
    median: BinMedian,
    rows: u64,
    memory: VecDeque<f32>,
    mem_first: u64,
    mem_rows: usize,
    noise: f64,
    noise_n: u64,
    tx: Option<Tx>,
    intervals: VecDeque<(u64, u64)>,
    pending: VecDeque<Pulse>,
    pulses: u64,
    carrier_bin: Option<usize>,
    found: Vec<Echo>,
    display_acc: Vec<f32>,
    display: VecDeque<Vec<f32>>,
    display_changed: bool,
}

impl EchoDetector {
    /// A detector for input at `rate` whose first sample is at `origin_s`, with the echo threshold `threshold` (0 for 6).
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
        let hop_s = HOP as f64 / fs;
        let bin_hz = fs / FFT_N as f64;
        let half = ((DOPPLER_HZ / bin_hz).floor() as usize).min(FFT_N / 2 - 2);
        let mem_rows = (MEMORY_S / hop_s).ceil() as usize;
        Ok(EchoDetector {
            dec,
            fs,
            hop_s,
            bin_hz,
            half,
            threshold: threshold_of(threshold),
            origin_s,
            front: FirDecimator::new(&vec![1.0 / dec as f32; dec], dec),
            out: Vec::new(),
            samples: Vec::new(),
            samples_first: 0,
            fft: WindowedFft::new(FFT_N, FFT_N, Window::Hann).expect("512 is a valid size"),
            median: BinMedian::new(1).expect("one frame"),
            rows: 0,
            memory: VecDeque::with_capacity(mem_rows * FFT_N),
            mem_first: 0,
            mem_rows,
            noise: 0.0,
            noise_n: 0,
            tx: None,
            intervals: VecDeque::new(),
            pending: VecDeque::new(),
            pulses: 0,
            carrier_bin: None,
            found: Vec::new(),
            display_acc: vec![DISPLAY_DB.0; half + 1],
            display: VecDeque::with_capacity(DISPLAY_KEPT),
            display_changed: false,
        })
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }

    /// The time between rows, s.
    pub fn hop_s(&self) -> f64 {
        self.hop_s
    }

    /// Sets the echo threshold (0 for 6).
    pub fn set_threshold(&mut self, v: f64) {
        self.threshold = threshold_of(v);
    }

    /// Own pulses kept.
    pub fn pulses(&self) -> u64 {
        self.pulses
    }

    /// Whether the station transmits now.
    pub fn transmitting(&self) -> bool {
        self.tx.is_some()
    }

    /// The noise per bin, dB of full scale; `None` before the first row.
    pub fn noise_db(&self) -> Option<f64> {
        (self.noise_n > 0).then(|| 10.0 * self.noise.max(1e-30).log10())
    }

    /// The frequency of the own carrier of the last pulse, Hz from the centre.
    pub fn carrier_hz(&self) -> Option<f64> {
        self.carrier_bin.map(|k| self.freq(k))
    }

    /// The frequency of bin `k`, Hz from the centre.
    fn freq(&self, k: usize) -> f64 {
        let k = k as f64;
        if k < FFT_N as f64 / 2.0 {
            k * self.bin_hz
        } else {
            (k - FFT_N as f64) * self.bin_hz
        }
    }

    /// The signal time of the start of row `r`, s.
    fn row_time(&self, r: u64) -> f64 {
        self.origin_s + (r * HOP as u64) as f64 / self.fs + (self.dec as f64 - 1.0) / (2.0 * self.fs * self.dec as f64)
    }

    /// Feeds samples at the input rate.
    pub fn process(&mut self, x: &[Complex32]) {
        self.out.clear();
        self.front.process(x, &mut self.out);
        self.samples.extend(self.out.iter().map(|z| {
            if z.re.is_finite() && z.im.is_finite() {
                *z
            } else {
                Complex32::default()
            }
        }));
        loop {
            let start = self.rows * HOP as u64;
            if self.samples_first + (self.samples.len() as u64) < start + FFT_N as u64 {
                break;
            }
            let at = (start - self.samples_first) as usize;
            let power: Vec<f32> = self.fft.run_power(&self.samples[at..at + FFT_N]).to_vec();
            self.row(&power);
            self.rows += 1;
            let keep = self.rows * HOP as u64;
            let drop = (keep - self.samples_first) as usize;
            if drop >= 8 * FFT_N {
                self.samples.drain(..drop);
                self.samples_first = keep;
            }
        }
    }

    fn row(&mut self, p: &[f32]) {
        let r = self.rows;
        let (kmax, pmax) = p
            .iter()
            .enumerate()
            .fold((0, f32::MIN), |b, (k, &v)| if v > b.1 { (k, v) } else { b });
        let pmax = f64::from(pmax);
        let nr = f64::from(self.median.estimate(p.iter().copied()).unwrap_or(0.0));
        let ready = self.noise_n >= NOISE_ROWS;
        let above = ready && pmax > self.noise * 10f64.powf(TX_DB / 10.0);
        if !ready {
            self.noise += (nr - self.noise) / (self.noise_n + 1) as f64;
            self.noise_n += 1;
        } else if self.tx.is_none() && !above {
            self.noise += (nr - self.noise) * self.hop_s / NOISE_TAU_S;
        }
        match (self.tx.as_mut(), above) {
            (None, true) => {
                self.tx = Some(Tx {
                    start: r,
                    last: r,
                    below: 0,
                    bins: vec![kmax],
                    powers: vec![pmax],
                });
            }
            (Some(t), true) => {
                t.last = r;
                t.below = 0;
                t.bins.push(kmax);
                t.powers.push(pmax);
            }
            (Some(t), false) => {
                t.below += 1;
                if t.below >= TX_OFF_ROWS {
                    let t = self.tx.take().expect("transmitting");
                    self.end_tx(t);
                }
            }
            (None, false) => {}
        }
        if self.memory.len() >= self.mem_rows * FFT_N {
            self.memory.drain(..FFT_N);
            self.mem_first += 1;
        }
        self.memory.extend(p.iter().copied());
        self.display_row(r, p);
        while let Some(pulse) = self.pending.front() {
            let len = Self::len_of(pulse);
            let due = pulse.start_row
                + ((SEARCH_AFTER_S + 2.0 * len as f64 * self.hop_s) / self.hop_s).ceil() as u64;
            if r < due {
                break;
            }
            let pulse = self.pending.pop_front().expect("pending");
            self.search(&pulse);
        }
        let oldest = self.rows.saturating_sub(self.mem_rows as u64 * 2);
        while self.intervals.front().is_some_and(|i| i.1 < oldest) {
            self.intervals.pop_front();
        }
    }

    fn end_tx(&mut self, t: Tx) {
        self.intervals.push_back((t.start, t.last));
        let rows = t.last - t.start + 1;
        if (rows as f64) * self.hop_s < MIN_PULSE_S {
            return;
        }
        let mut bins = t.bins.clone();
        bins.sort_unstable();
        let bin = bins[bins.len() / 2];
        let power = t.powers.iter().sum::<f64>() / t.powers.len() as f64;
        self.pulses += 1;
        self.carrier_bin = Some(bin);
        self.pending.push_back(Pulse {
            start_row: t.start,
            rows,
            bin,
            power,
        });
    }

    fn len_of(p: &Pulse) -> u64 {
        p.rows.saturating_sub(2).max(2)
    }

    /// The power of bin `k` (any integer, taken modulo the size) of row `r`; `None` when the row is not held.
    fn cell(&self, r: u64, k: i64) -> Option<f32> {
        if r < self.mem_first || r >= self.mem_first + (self.memory.len() / FFT_N) as u64 {
            return None;
        }
        let k = k.rem_euclid(FFT_N as i64) as usize;
        self.memory.get((r - self.mem_first) as usize * FFT_N + k).copied()
    }

    /// The score of the window of `len` rows from `a` in bins `k - 1` to `k + 1`.
    fn score(&self, a: u64, len: u64, k: i64) -> Option<f64> {
        let mut s = 0.0;
        for r in a..a + len {
            for j in k - 1..=k + 1 {
                s += f64::from(self.cell(r, j)?);
            }
        }
        Some(s / (3 * len) as f64)
    }

    fn over_tx(&self, a: u64, len: u64) -> bool {
        let b = a + len - 1;
        self.intervals.iter().any(|&(s, e)| a <= e && b >= s)
            || self.tx.as_ref().is_some_and(|t| b >= t.start)
    }

    fn search(&mut self, p: &Pulse) {
        let len = Self::len_of(p);
        let hop = self.hop_s;
        let h = self.half as i64;
        let k0 = p.bin as i64;
        // Control scores.
        let c0 = (CONTROL_S / hop).round() as u64 + len;
        let c1 = c0 + (CONTROL_SPAN_S / hop).round() as u64;
        let mut control = Vec::new();
        for d in c0..=c1 {
            let a = p.start_row + d;
            if self.over_tx(a, len) {
                continue;
            }
            for dk in (-h..=h).step_by(CONTROL_BIN_STEP) {
                if let Some(s) = self.score(a, len, k0 + dk) {
                    control.push(s);
                }
            }
        }
        if control.len() < CONTROL_MIN {
            return;
        }
        let n = control.len() as f64;
        let mean = control.iter().sum::<f64>() / n;
        let sd = (control.iter().map(|s| (s - mean) * (s - mean)).sum::<f64>() / n).sqrt();
        if sd <= 0.0 {
            return;
        }
        // The search.
        let d0 = (DELAY_S.0 / hop).ceil() as u64;
        let d1 = (DELAY_S.1 / hop).floor() as u64;
        let mut best: Option<(f64, u64, i64)> = None;
        for d in d0..=d1 {
            let a = p.start_row + d;
            if self.over_tx(a, len) {
                continue;
            }
            for dk in -h..=h {
                if let Some(s) = self.score(a, len, k0 + dk) {
                    if best.is_none_or(|b| s > b.0) {
                        best = Some((s, d, dk));
                    }
                }
            }
        }
        let Some((bs, bd, bk)) = best else {
            return;
        };
        let z = (bs - mean) / sd;
        if z < self.threshold {
            return;
        }
        let at = |d: u64| self.score(p.start_row + d, len, k0 + bk);
        let off = match (at(bd.wrapping_sub(1)), at(bd + 1)) {
            (Some(m), Some(q)) if bd > 0 => parabolic(m, bs, q).offset.clamp(-0.5, 0.5),
            _ => 0.0,
        };
        let delay = (bd as f64 + off) * hop;
        // The spread: bins either side while the score keeps over half the best excess.
        let half = 0.5 * (bs - mean);
        let mut width = 1;
        for dir in [-1i64, 1] {
            let mut j = bk + dir;
            while j.abs() <= h {
                match self.score(p.start_row + bd, len, k0 + j) {
                    Some(s) if s - mean > half => {
                        width += 1;
                        j += dir;
                    }
                    _ => break,
                }
            }
        }
        let peak = 10.0 * (bs / mean).log10();
        let below = 10.0 * (p.power / bs).log10();
        self.found.push(Echo {
            row: EventRow {
                signal_time_s: self.row_time(p.start_row),
                kind: "echo".into(),
                frequency_hz: bk as f64 * self.bin_hz,
                peak_db: peak,
                duration_s: len as f64 * hop,
                aux1: delay,
                aux2: width as f64 * self.bin_hz,
                note: format!("{below:.1} dB below the own carrier, z {z:.1}"),
            },
            z,
        });
    }

    fn display_row(&mut self, r: u64, p: &[f32]) {
        let k0 = self.carrier_bin.unwrap_or(0) as i64;
        let h = self.half as i64;
        let noise = self.noise.max(1e-30) as f32;
        for (j, acc) in self.display_acc.iter_mut().enumerate() {
            let a = k0 - h + 2 * j as i64;
            let v = [a, a + 1]
                .iter()
                .filter(|&&k| k <= k0 + h)
                .map(|&k| p[k.rem_euclid(FFT_N as i64) as usize])
                .fold(0f32, f32::max);
            *acc = acc.max(10.0 * (v / noise).max(1e-30).log10());
        }
        if (r + 1).is_multiple_of(DISPLAY_ROWS) {
            if self.display.len() >= DISPLAY_KEPT {
                self.display.pop_front();
            }
            let row = self
                .display_acc
                .iter()
                .map(|v| v.clamp(DISPLAY_DB.0, DISPLAY_DB.1))
                .collect();
            self.display.push_back(row);
            self.display_acc.iter_mut().for_each(|v| *v = DISPLAY_DB.0);
            self.display_changed = true;
        }
    }

    /// The echoes found since the last call.
    pub fn take_found(&mut self) -> Vec<Echo> {
        std::mem::take(&mut self.found)
    }

    /// Writes the display rows into `r` when they changed. Returns whether they changed.
    pub fn display(&mut self, r: &mut Raster) -> bool {
        if !std::mem::take(&mut self.display_changed) {
            return false;
        }
        r.width = self.display_acc.len();
        r.height = self.display.len();
        r.x = Axis::new(-(self.half as f64) * self.bin_hz, 2.0 * self.bin_hz, "Hz");
        let step = DISPLAY_ROWS as f64 * self.hop_s;
        let last = self.row_time((self.rows / DISPLAY_ROWS).saturating_sub(1) * DISPLAY_ROWS);
        r.y = Axis::new(last - self.display.len().saturating_sub(1) as f64 * step, step, "s");
        r.unit.clear();
        r.unit.push_str("dB");
        r.values.clear();
        for row in &self.display {
            r.values.extend_from_slice(row);
        }
        true
    }
}

/// The display: ±500 Hz around the own carrier, bins in pairs, dB over the noise, a row every 4 rows.
pub const SPECTROGRAM: RasterDef = RasterDef {
    key: "spectrogram",
    label: "Spectrogram around the own carrier",
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
    .help("Empties the tables and starts the detector again; the noise is learned again.");
static COMMANDS: [&Command; 1] = [&RESET];

fn start_utc_of(setting: f64, clock: &Clock) -> f64 {
    if setting > 0.0 {
        setting
    } else {
        clock.now().floor()
    }
}

/// The `eme_echo` decoder.
pub struct EmeDecoder {
    det: Option<EchoDetector>,
    rate: f64,
    threshold: f64,
    start_setting: f64,
    start_utc: f64,
    clock: Clock,
    input: u64,
    log: EventLog,
    trend: TrendLog,
    minutes: Vec<(u64, f64, f64, u32)>,
    next_minute: u64,
    echoes: u64,
    last: Option<EventRow>,
    events: Vec<Event>,
    raster: Raster,
    raster_id: Option<RasterId>,
    started: bool,
    status: Status,
}

impl Default for EmeDecoder {
    fn default() -> Self {
        EmeDecoder {
            det: None,
            rate: 0.0,
            threshold: THRESHOLD.default,
            start_setting: START_UTC.default,
            start_utc: 0.0,
            clock: Clock::fixed(0.0),
            input: 0,
            log: EventLog::default(),
            trend: TrendLog::default(),
            minutes: Vec::new(),
            next_minute: 0,
            echoes: 0,
            last: None,
            events: Vec::new(),
            raster: Raster::default(),
            raster_id: None,
            started: false,
            status: Status::default(),
        }
    }
}

impl EmeDecoder {
    fn now(&self) -> f64 {
        self.input as f64 / self.rate.max(1.0)
    }

    fn forget(&mut self) {
        self.log.clear();
        self.trend.clear();
        self.minutes.clear();
        self.echoes = 0;
        self.last = None;
        self.next_minute = (self.now() / 60.0).floor() as u64;
    }

    fn take(&mut self, out: &mut Outbox) {
        let Some(det) = self.det.as_mut() else {
            return;
        };
        for e in det.take_found() {
            self.echoes += 1;
            let m = (e.row.signal_time_s / 60.0).floor().max(0.0) as u64;
            if m >= self.next_minute {
                match self.minutes.iter_mut().find(|x| x.0 == m) {
                    Some(x) => {
                        x.1 = x.1.max(e.row.peak_db);
                        x.2 += e.row.aux1;
                        x.3 += 1;
                    }
                    None => self.minutes.push((m, e.row.peak_db, e.row.aux1, 1)),
                }
            }
            self.last = Some(e.row.clone());
            self.log.push(e.row, self.start_utc, &mut self.events);
        }
        let now = self.now();
        while (self.next_minute + 1) as f64 * 60.0 + TREND_DELAY_S <= now {
            let m = self.next_minute;
            self.next_minute += 1;
            let row = match self.minutes.iter().position(|x| x.0 == m) {
                Some(i) => {
                    let x = self.minutes.remove(i);
                    [x.1, x.2 / f64::from(x.3)]
                }
                None => [0.0, 0.0],
            };
            self.trend.push(m, row);
        }
        if !self.events.is_empty() {
            out.messages(0).append(&mut self.events);
        }
    }
}

impl Decoder for EmeDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "science.eme_echo"
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
                if v > 0.0 && v < 2.0 {
                    return Err(SettingError::OutOfRange {
                        id,
                        min: 2.0,
                        max: 20.0,
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
                self.det = EchoDetector::new(self.rate, self.threshold, self.now()).ok();
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
        self.next_minute = 0;
        self.start_utc = start_utc_of(self.start_setting, &self.clock);
        self.det = Some(EchoDetector::new(input.rate, self.threshold, 0.0)?);
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
        let (pulses, carrier, tx, noise) = self.det.as_ref().map_or((0, None, false, None), |d| {
            (d.pulses(), d.carrier_hz(), d.transmitting(), d.noise_db())
        });
        self.log.publish(p, self.start_utc);
        self.trend.publish(
            p,
            &EME_ECHO_TREND,
            self.start_utc,
            |utc, [echo_level_db, echo_delay_s]| EmeEchoTrendRow {
                utc,
                echo_level_db,
                echo_delay_s,
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
        st.lock_state = match (self.started, pulses > 0, self.echoes > 0) {
            (false, ..) => LockState::Off,
            (_, false, _) => LockState::Searching,
            (_, true, false) => LockState::Tracking,
            (_, true, true) => LockState::Locked,
        };
        st.offset_hz = carrier;
        st.summary.clear();
        st.summary.push_str(&if !self.started {
            "Moon echo: waiting for samples".to_string()
        } else if tx {
            "Moon echo: transmitting".to_string()
        } else if let Some(e) = &self.last {
            format!(
                "Moon echo: {} echoes of {pulses} pulses; last {:.3} s, {:+.0} Hz, {:.1} dB",
                self.echoes, e.aux1, e.frequency_hz, e.peak_db
            )
        } else {
            format!("Moon echo: {pulses} pulses, no echo yet")
        });
        p.put(&EmeEchoFields {
            text: &st.summary,
            pulses: i64::try_from(pulses).unwrap_or(i64::MAX),
            echoes: i64::try_from(self.echoes).unwrap_or(i64::MAX),
            carrier_hz: carrier.map(Hertz),
            transmitting: tx,
            noise_db: noise,
            last_delay_s: self.last.as_ref().map(|r| Seconds(r.aux1)),
            last_doppler_hz: self.last.as_ref().map(|r| Hertz(r.frequency_hz)),
            last_level_db: self.last.as_ref().map(|r| r.peak_db),
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
        .add("dec", DecoderComponent::new(EmeDecoder::default()))
        .input("dec")
        .output("dec.events")
        .status_from("dec")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "The moon echo test needs a sample rate of at least 8 ksps")
        .route(THRESHOLD.key(), &["dec.threshold_sigma"])?
        .route(START_UTC.key(), &["dec.start_utc"])?
        .command("clear", CommandTarget::every("clear"))
        .command("reset", CommandTarget::of("dec", "reset"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("science.eme_echo", || {
        Box::new(DecoderComponent::new(EmeDecoder::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = gen::GenParams {
        delay_s: cfg.settings.get(&gen::GEN_DELAY_S),
        doppler_hz: cfg.settings.get(&gen::GEN_DOPPLER_HZ),
        spread_hz: cfg.settings.get(&gen::GEN_SPREAD_HZ),
        echo_db: cfg.settings.get(&gen::GEN_ECHO_DB),
        ..gen::GenParams::default()
    };
    Ok(Box::new(
        gen::EmeGenerator::new(cfg.rate_hz, p, cfg.seed).with_own_noise(crate::family::own_noise(cfg)),
    ))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, pulses at 5 s and 12 s with echoes 2.55 s later", o, 0.0);
    c.seconds = o.seconds.max(20.0);
    c.snr_db = None;
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let e = crate::family::field(r, EmeEchoFields::ECHOES.key);
        let d = crate::family::field(r, EmeEchoFields::LAST_DELAY_S.key);
        vec![
            CheckLine::new(EmeEchoFields::ECHOES.key, e >= 1.0, format!("{e}, at least 1 expected")),
            CheckLine::new(
                EmeEchoFields::LAST_DELAY_S.key,
                (d - 2.55).abs() <= 0.1,
                format!("{d:.3} s, 2.55 ± 0.1 expected"),
            ),
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

/// The moon echo test.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Moon echo test",
    group: "Science, time and utility",
    blurb: "Finds your own keyed carrier and its echo from the Moon 2.3 to 2.85 s later within ±500 Hz: delay, Doppler \
            shift, level and spread. The stack listens while you transmit; it transmits nothing.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 50e6,
        max_hz: 10_400e6,
        default_hz: 144e6,
        rate_hz: 250e3,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 200e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: EmeSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&VERIFY),
    benchmarks: None,
};
