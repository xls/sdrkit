//! AIS burst detection and burst decoding at 48 kHz (5 samples per bit): the noise floor and the cut-out of each burst, then the
//! decoder that tries carrier candidates, timing phases and three bit detectors, and cancels each decoded burst to find a weaker one
//! under it.
//!
//! Standard: ITU-R M.1371-5 Annex 2

use super::gmsk::{self, Params};
use super::line::{self, LineCoder};
use sdrkit_algo_dsp::fir::{self, Convolver};
use sdrkit_algo_dsp::linalg::LeastSquares;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::stats::quantile_f64;
use sdrkit_algo_modem::freq::{kay, PowerLine};
use sdrkit_algo_modem::gmsk::{ls_channel, Mlse};
use sdrkit_core::Complex32;
use sdrkit_proto_link::hdlc::Receiver;
use sdrkit_util_bits::pack::unpack;
use sdrkit_util_bits::BitOrder;
use std::f64::consts::{FRAC_PI_2, TAU};

/// The channel rate.
pub const RATE: f64 = 48_000.0;
/// Samples per bit.
pub const SPB: f64 = 5.0;

const SMOOTH: usize = 24;
const BLOCK: usize = 960;
const RING: usize = 16_384;
const PRE: usize = 480;
const POST: usize = 120;
/// A burst that starts this close after the end of the one before continues it (a power step inside one transmission).
const MERGE_GAP: usize = 24;
const MAX_LEN: usize = 7200;
/// The longest cut the detector outputs, samples.
pub const MAX_CUT: usize = MAX_LEN + PRE + POST;
/// The most bit integrals of one cut.
const MAX_BITS: usize = MAX_CUT / SPB as usize + 2;
/// The fewest bit integrals the sequence search works on.
const MIN_SEARCH: usize = 8;
/// The least noise variance given to the sequence search, rad^2, so its integer metrics keep their range.
const NOISE_VAR_FLOOR: f32 = 1e-2;
/// The FFT of the line search of a remainder's carrier.
const LINE_FFT: usize = 4096;
/// The least SNR of the weaker line of the squared signal, dB.
const LINE_SNR_DB: f32 = 6.0;
/// The taps of the least-squares channel between the remodulated burst and the samples (lags -4 to +4).
const LS_TAPS: usize = 9;

/// A burst cut out of a channel.
#[derive(Clone, Debug, Default)]
pub struct BurstCut {
    /// The samples.
    pub samples: Vec<Complex32>,
    /// The signal core inside `samples`.
    pub core: (usize, usize),
    /// The absolute index (48 kHz samples since the start) of `samples[0]`.
    pub first: u64,
    /// The noise floor, mean power per sample.
    pub floor: f64,
    /// The channel index (0 to 3).
    pub channel: u8,
}

/// The burst detector of one channel.
#[derive(Clone, Debug)]
pub struct Detector {
    ring: Vec<Complex32>,
    count: u64,
    sm: [f64; SMOOTH],
    sm_i: usize,
    sm_sum: f64,
    block_sum: f64,
    block_n: usize,
    block_dirty: bool,
    clean: Vec<f64>,
    clean_i: usize,
    scratch: Vec<f64>,
    floor: Option<f64>,
    /// The smoothed mean of all blocks (the channel level).
    pub level: f64,
    in_burst: bool,
    start: u64,
    peak: f64,
    below: usize,
    pending: Vec<(u64, u64, f64)>,
    channel: u8,
    /// Bursts detected.
    pub detected: u64,
}

impl Detector {
    /// For channel index `channel`.
    pub fn new(channel: u8) -> Self {
        Detector {
            ring: vec![Complex32::default(); RING],
            count: 0,
            sm: [0.0; SMOOTH],
            sm_i: 0,
            sm_sum: 0.0,
            block_sum: 0.0,
            block_n: 0,
            block_dirty: false,
            clean: Vec::with_capacity(64),
            clean_i: 0,
            scratch: Vec::with_capacity(64),
            floor: None,
            level: 0.0,
            in_burst: false,
            start: 0,
            peak: 0.0,
            below: 0,
            pending: Vec::with_capacity(8),
            channel,
            detected: 0,
        }
    }

    /// The noise floor (mean power per sample), once known.
    pub fn floor(&self) -> Option<f64> {
        self.floor
    }

    /// Whether a burst is being received.
    pub fn busy(&self) -> bool {
        self.in_burst
    }

    /// The floor: the 20th percentile of the last 64 blocks without a burst, raised by 3 %.
    fn update_floor(&mut self) {
        if self.clean.len() >= 3 {
            if let Some(p) = quantile_f64(&self.clean, 0.2, &mut self.scratch) {
                self.floor = Some((p * 1.03).max(1e-10));
            }
        }
    }

    /// Feeds samples. Calls `out` with every complete burst.
    pub fn process(&mut self, x: &[Complex32], mut out: impl FnMut(BurstCut)) {
        for &s in x {
            let p = f64::from(s.norm_sqr());
            self.ring[(self.count % RING as u64) as usize] = s;
            self.sm_sum += p - self.sm[self.sm_i];
            self.sm[self.sm_i] = p;
            self.sm_i = (self.sm_i + 1) % SMOOTH;
            let smooth = (self.sm_sum / SMOOTH as f64).max(0.0);
            // Blocks for the floor.
            self.block_sum += p;
            self.block_n += 1;
            if self.in_burst {
                self.block_dirty = true;
            }
            if self.block_n == BLOCK {
                let m = self.block_sum / BLOCK as f64;
                self.level = if self.level == 0.0 {
                    m
                } else {
                    self.level + 0.1 * (m - self.level)
                };
                if !self.block_dirty {
                    if self.clean.len() < 64 {
                        self.clean.push(m);
                    } else {
                        self.clean[self.clean_i] = m;
                        self.clean_i = (self.clean_i + 1) % 64;
                    }
                    self.update_floor();
                }
                self.block_sum = 0.0;
                self.block_n = 0;
                self.block_dirty = self.in_burst;
            }
            let idx = self.count;
            self.count += 1;
            if let Some(floor) = self.floor {
                if !self.in_burst {
                    if smooth > 2.0 * floor {
                        self.in_burst = true;
                        self.block_dirty = true;
                        self.start = idx.saturating_sub((SMOOTH / 2) as u64);
                        self.peak = smooth;
                        self.below = 0;
                        // A power step down inside one transmission (a weaker burst that started under a stronger one and
                        // outlasts it) ends the burst and starts another at once: the two stay one cut, so the weaker one's
                        // tail is there for the cancellation.
                        if let Some(&(s, e, _)) = self.pending.last() {
                            if self.start <= e + MERGE_GAP as u64 && idx - s < MAX_LEN as u64 {
                                self.pending.pop();
                                self.detected -= 1;
                                self.start = s;
                            }
                        }
                    }
                } else {
                    self.peak = self.peak.max(smooth);
                    let thr = (1.5 * floor).max(0.08 * self.peak);
                    if smooth < thr {
                        self.below += 1;
                    } else {
                        self.below = 0;
                    }
                    let len = idx - self.start;
                    if self.below >= 12 || len >= MAX_LEN as u64 {
                        self.in_burst = false;
                        let end = if self.below >= 12 {
                            idx + 1 - self.below as u64
                        } else {
                            idx
                        };
                        if end - self.start >= 100 {
                            self.detected += 1;
                            self.pending.push((self.start, end, floor));
                        }
                    }
                }
            }
            // Cut out the bursts whose trailing samples have arrived.
            let mut k = 0;
            while k < self.pending.len() {
                let (s, e, floor) = self.pending[k];
                if self.count >= e + POST as u64 {
                    let first = s
                        .saturating_sub(PRE as u64)
                        .max(self.count.saturating_sub(RING as u64));
                    let last = e + POST as u64;
                    let samples: Vec<Complex32> = (first..last)
                        .map(|i| self.ring[(i % RING as u64) as usize])
                        .collect();
                    out(BurstCut {
                        samples,
                        core: ((s - first) as usize, (e - first) as usize),
                        first,
                        floor,
                        channel: self.channel,
                    });
                    self.pending.remove(k);
                } else {
                    k += 1;
                }
            }
        }
    }

    /// Starts again (a retune).
    pub fn reset(&mut self) {
        *self = Detector::new(self.channel);
    }
}

sdrkit_core::choice_enum! {
    /// Which decoding path found a frame.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Layer {
        /// On the first carrier estimate of the burst.
        #[default]
        Direct = ("direct", "Direct"),
        /// On another frequency candidate or part of the burst.
        Recovered = ("recovered", "Recovered"),
        /// After cancelling a stronger burst.
        Cancelled = ("cancelled", "Cancelled"),
    }
}

/// A decoded frame.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Payload bits, message-table order, without the FCS.
    pub bits: Vec<u8>,
    /// The absolute 48 kHz sample index of the start of the burst's training sequence (estimate).
    pub start_index: f64,
    /// The signal to noise ratio in 48 kHz, dB.
    pub snr_db: f64,
    /// The carrier, Hz from the channel centre.
    pub offset_hz: f64,
    /// How it was found.
    pub layer: Layer,
    /// The channel index (0 to 3).
    pub channel: u8,
}

/// Options of the burst decoder.
#[derive(Clone, Debug, Default)]
pub struct DecodeOptions {
    /// Collision handling (coast mode): extra frequency tries and cancellation.
    pub collision: bool,
    /// Satellite mode: the wide grid, the hints, up to 3 layers.
    pub satellite: bool,
    /// Frequency hints (Hz, satellite mode).
    pub hints: Vec<f64>,
    /// The noise-bandwidth factor: noise in 48 kHz = floor x this.
    pub noise_scale: f64,
}

/// The result of decoding a burst.
#[derive(Clone, Debug, Default)]
pub struct BurstResult {
    /// The frames found, strongest first.
    pub frames: Vec<Frame>,
    /// A plausible stretch with a failed FCS and nothing decoded.
    pub bad: bool,
}

/// A remodulated burst: the samples in `a..e` of `buf`, zero elsewhere.
#[derive(Clone, Debug, Default)]
struct Wave {
    buf: Vec<Complex32>,
    a: usize,
    e: usize,
}

impl Wave {
    fn sized(n: usize) -> Self {
        Wave {
            buf: vec![Complex32::default(); n],
            a: 0,
            e: 0,
        }
    }

    /// Clears the span held, and makes room for `n` samples.
    fn reset(&mut self, n: usize) {
        self.buf[self.a..self.e].fill(Complex32::default());
        if self.buf.len() < n {
            self.buf.resize(n, Complex32::default());
        }
        self.a = 0;
        self.e = 0;
    }

    #[inline]
    fn at(&self, i: usize) -> Complex32 {
        if i >= self.a && i < self.e {
            self.buf[i]
        } else {
            Complex32::default()
        }
    }
}

/// The measurements of one fit (its waveform is in the decoder's buffers).
#[derive(Clone, Copy, Debug, Default)]
struct Fit {
    start: f64,
    offset_hz: f64,
    power: f64,
    score: f64,
}

/// The buffers of the coherent detector.
#[derive(Clone, Debug, Default)]
struct Coherent {
    z: Vec<Complex32>,
    z4: Vec<Complex32>,
    zr: Vec<Complex32>,
    refs: Vec<f64>,
    back: Vec<[u8; 4]>,
    path: Vec<usize>,
}

/// The burst decoder. It holds every buffer it works in, sized for the longest cut, so decoding a burst allocates only the frames
/// it returns.
#[derive(Debug)]
pub struct Decoder {
    narrow: Convolver<Complex32>,
    chan: Convolver<Complex32>,
    x: Vec<Complex32>,
    mixed: Vec<Complex32>,
    shifted: Vec<Complex32>,
    phase: Vec<f64>,
    d: Vec<f64>,
    levels: Vec<u8>,
    ok_levels: Vec<u8>,
    ranked: Vec<(f64, f64)>,
    tried: Vec<f64>,
    cands: Vec<f64>,
    rx: Receiver,
    frame: Vec<u8>,
    coder: LineCoder,
    remod: Vec<u8>,
    trial: Vec<u8>,
    mlse: Mlse,
    mlse_y: Vec<Complex32>,
    mlse_s: Vec<Complex32>,
    mlse_bits: Vec<u8>,
    coh: Coherent,
    raw: Vec<Complex32>,
    modulator: gmsk::Modulator,
    ls: LeastSquares,
    lines: PowerLine,
    cand: Wave,
    best: Wave,
    keep: Wave,
}

/// The channel filter of the remodulated model: coast (passband 7.5 kHz, stopband 15 kHz) or satellite (12 kHz, 18 kHz).
fn channel_taps(satellite: bool) -> Vec<f32> {
    if satellite {
        fir::lowpass(RATE, 12e3, 18e3, 60.0).expect("the filter settings are valid")
    } else {
        fir::lowpass(RATE, 7.5e3, 15e3, 60.0).expect("the filter settings are valid")
    }
}

/// The default channel of the sequence search, when the fit is singular: 0.74 and 0.13 of a quarter turn.
fn default_taps() -> [Complex32; 3] {
    let (alpha, beta) = ((0.74 * FRAC_PI_2) as f32, (0.13 * FRAC_PI_2) as f32);
    [
        Complex32::new(beta, 0.0),
        Complex32::new(alpha, 0.0),
        Complex32::new(beta, 0.0),
    ]
}

impl Default for Decoder {
    fn default() -> Self {
        let conv = |t: &[f32]| Convolver::new(t).expect("the AIS filters have taps");
        let bits = || Vec::with_capacity(MAX_BITS);
        let samples = || Vec::with_capacity(MAX_CUT);
        Decoder {
            narrow: conv(
                &fir::lowpass(RATE, 3500.0, 8500.0, 50.0).expect("the filter settings are valid"),
            ),
            chan: conv(&channel_taps(false)),
            x: samples(),
            mixed: samples(),
            shifted: samples(),
            phase: Vec::with_capacity(MAX_CUT),
            d: Vec::with_capacity(MAX_BITS),
            levels: bits(),
            ok_levels: bits(),
            ranked: Vec::with_capacity(50),
            tried: Vec::with_capacity(16),
            cands: Vec::with_capacity(32),
            rx: line::receiver(),
            frame: Vec::with_capacity(8 * line::MAX_FRAME_BYTES),
            coder: LineCoder::default(),
            remod: bits(),
            trial: bits(),
            mlse: Mlse::new(&default_taps(), MAX_BITS)
                .expect("the sequence search sizes are valid"),
            mlse_y: samples(),
            mlse_s: samples(),
            mlse_bits: bits(),
            coh: Coherent {
                z: bits_c(),
                z4: bits_c(),
                zr: bits_c(),
                refs: Vec::with_capacity(MAX_BITS),
                back: Vec::with_capacity(MAX_BITS),
                path: Vec::with_capacity(MAX_BITS),
            },
            raw: samples(),
            modulator: gmsk::Modulator::default(),
            ls: LeastSquares::new(2 * LS_TAPS).expect("the fit size is valid"),
            lines: PowerLine::new(2, LINE_FFT).expect("the line search sizes are valid"),
            cand: Wave::sized(MAX_CUT),
            best: Wave::sized(MAX_CUT),
            keep: Wave::sized(MAX_CUT),
        }
    }
}

fn bits_c() -> Vec<Complex32> {
    Vec::with_capacity(MAX_BITS)
}

impl Decoder {
    /// Uses the satellite channel filter (passband 12 kHz, stopband 18 kHz) for the remodulated model.
    pub fn set_satellite(&mut self, on: bool) {
        self.chan = Convolver::new(&channel_taps(on)).expect("the AIS filters have taps");
    }

    /// The carrier inside the core by Kay's weighted phase-step estimator (`sdrkit_algo_modem::freq::kay`), Hz. The core is trimmed by
    /// 24 samples each end when it is long enough. 0 when the core is too short.
    pub fn carrier(x: &[Complex32], core: (usize, usize)) -> f64 {
        let (a, b) = (core.0 + 24, core.1.saturating_sub(24));
        let (a, b) = if b > a + 20 { (a, b) } else { (core.0, core.1) };
        let (a, b) = (a.min(x.len()), b.min(x.len()));
        if b <= a {
            return 0.0;
        }
        kay(&x[a..b]).map_or(0.0, |f| f * RATE)
    }

    /// The carrier from the two spectral lines of the squared signal (`sdrkit_algo_modem::freq::PowerLine::estimate_pair`), searched
    /// within `span_hz` of 0, Hz. With h = 0.5 the square turns by half a turn per bit, so it holds lines at twice the carrier
    /// +- half the bit rate whatever the data. `None` when the core is short or the weaker line is less than 6 dB over the floor.
    pub fn carrier_lines(
        &mut self,
        x: &[Complex32],
        core: (usize, usize),
        span_hz: f64,
    ) -> Option<f64> {
        let (a, b) = (core.0.min(x.len()), core.1.min(x.len()));
        if b < a + 200 {
            return None;
        }
        self.lines.set_span(span_hz / RATE).ok()?;
        let p = self
            .lines
            .estimate_pair(&x[a..b], gmsk::BIT_RATE / RATE)
            .ok()?;
        (p.snr_db >= LINE_SNR_DB).then_some(p.freq * RATE)
    }

    /// Shifts `x` by `-offset_hz`, filters it narrowly with no delay and unwraps the phase into `self.phase` (sample n of the
    /// filtered signal aligned with sample n of the input).
    fn prepare(&mut self, x: &[Complex32], offset_hz: f64) {
        let n = x.len();
        self.mixed.clear();
        self.mixed.resize(n, Complex32::default());
        Nco::new(RATE, -offset_hz).mix(x, &mut self.mixed);
        self.shifted.clear();
        self.shifted.resize(n, Complex32::default());
        // The lengths are equal, so the convolution cannot fail.
        let _ = self.narrow.centred(&self.mixed, &mut self.shifted);
        self.phase.clear();
        self.phase.push(0.0);
        let mut ph = 0.0;
        for i in 1..n {
            ph += f64::from((self.shifted[i] * self.shifted[i - 1].conj()).arg());
            self.phase.push(ph);
        }
    }

    fn phase_at(&self, t: f64) -> f64 {
        let n = self.phase.len();
        if t <= 0.0 {
            return self.phase[0];
        }
        let i = t.floor() as usize;
        if i + 1 >= n {
            return self.phase[n - 1];
        }
        let f = t - i as f64;
        self.phase[i] * (1.0 - f) + self.phase[i + 1] * f
    }

    /// The phase turned in each bit from timing phase `tau`, into `self.d`.
    fn integrals(&mut self, tau: f64) {
        self.d.clear();
        let n = self.phase.len() as f64;
        let mut t = tau;
        while t + SPB < n {
            let v = self.phase_at(t + SPB) - self.phase_at(t);
            self.d.push(v);
            t += SPB;
        }
    }

    /// Runs the levels through the HDLC receiver. Returns whether a frame was found (its bits in `self.frame`) and whether a frame
    /// of an allowed length failed its FCS.
    fn search_levels(&mut self) -> (bool, bool) {
        self.rx.reset();
        let bad0 = self.rx.stats().bad_fcs;
        let mut found = false;
        for &l in &self.levels {
            if self.rx.push(l).is_some() {
                self.frame.clear();
                unpack(self.rx.frame(), BitOrder::Msb, &mut self.frame);
                found = true;
                break;
            }
        }
        (found, self.rx.stats().bad_fcs > bad0)
    }

    /// A frame found on the current levels: keeps the levels for the fit.
    fn found(&mut self) {
        self.ok_levels.clone_from(&self.levels);
    }

    /// One frequency candidate: timing phases, slicing, the sequence search, coherent detection. Returns the timing phase of the
    /// frame found (its bits in `self.frame`), and whether a frame failed its FCS.
    fn attempt(
        &mut self,
        x: &[Complex32],
        core: (usize, usize),
        offset_hz: f64,
    ) -> (Option<f64>, bool) {
        self.prepare(x, offset_hz);
        let (a, b) = (core.0 as f64, core.1 as f64);
        // Rank the timing phases by eye opening in the core.
        self.ranked.clear();
        for k in 0..50 {
            let tau = k as f64 * 0.1;
            let mut s = 0.0;
            let mut cnt = 0;
            let mut t = a + tau;
            while t + SPB < b {
                s += (self.phase_at(t + SPB) - self.phase_at(t)).abs();
                cnt += 1;
                t += SPB;
            }
            self.ranked
                .push((tau, if cnt > 0 { s / cnt as f64 } else { 0.0 }));
        }
        self.ranked.sort_by(|p, q| q.1.total_cmp(&p.1));
        let mut bad = false;
        for r in 0..4 {
            let tau = self.ranked[r].0;
            self.integrals(tau);
            // Decision directed threshold, re-centred twice.
            let mut thr = 0.0;
            for _ in 0..2 {
                let (mut hi, mut nh, mut lo, mut nl) = (0.0, 0, 0.0, 0);
                let k0 = (a / SPB) as usize;
                let k1 = ((b / SPB) as usize).min(self.d.len());
                for &v in &self.d[k0.min(k1)..k1] {
                    if v > thr {
                        hi += v;
                        nh += 1;
                    } else {
                        lo += v;
                        nl += 1;
                    }
                }
                if nh > 0 && nl > 0 {
                    thr = (hi / nh as f64 + lo / nl as f64) / 2.0;
                }
            }
            self.levels.clear();
            self.levels
                .extend(self.d.iter().map(|&v| u8::from(v > thr)));
            let (ok, b1) = self.search_levels();
            if ok {
                self.found();
                return (Some(tau), false);
            }
            bad |= b1;
            if self.sequence_search(thr) {
                let (ok, b2) = self.search_levels();
                if ok {
                    self.found();
                    return (Some(tau), false);
                }
                bad |= b2;
            }
        }
        // Coherent detection on the best phase.
        let tau = self.ranked[0].0;
        if self.coherent(tau, core) {
            let (ok, b3) = self.search_levels();
            if ok {
                self.found();
                return (Some(tau), false);
            }
            bad |= b3;
        }
        (None, bad)
    }

    /// Coherent detection: with h = 0.5 the carrier turns by +-90 degrees per bit, so at each bit boundary the phase is
    /// theta + 90 deg x S. A fine frequency search, a phase estimate from neighbouring boundaries, and a 4-state best-path search
    /// over S (mod 4), where each step moves S by +-1.
    fn coherent(&mut self, tau: f64, core: (usize, usize)) -> bool {
        let n = self.shifted.len();
        let c = &mut self.coh;
        c.z.clear();
        let mut t = tau;
        while t < n as f64 - 1.0 {
            let i = t.floor() as usize;
            let f = (t - i as f64) as f32;
            c.z.push(self.shifted[i] * (1.0 - f) + self.shifted[(i + 1).min(n - 1)] * f);
            t += SPB;
        }
        if c.z.len() < 40 {
            return false;
        }
        let quad = |v: &Complex32| {
            let m = v.norm().max(1e-12);
            let u = v / m;
            u * u * u * u
        };
        // z^4 removes the data (S x 90 deg). Its rotation per bit is 4 x the residual frequency.
        let (k0, k1) = (
            (core.0 as f64 / SPB) as usize,
            ((core.1 as f64 / SPB) as usize).min(c.z.len()),
        );
        c.z4.clear();
        c.z4.extend(c.z.iter().map(quad));
        // Frequency search over +-0.4 rad per bit.
        let mut best = (0.0f64, 0.0f32);
        let steps = 81;
        for s in 0..steps {
            let w = -0.4 + 0.8 * s as f64 / (steps - 1) as f64;
            let mut acc = Complex32::default();
            for (k, v) in c.z4.iter().enumerate().take(k1).skip(k0) {
                let p = -4.0 * w * k as f64;
                acc += v * Complex32::new(p.cos() as f32, p.sin() as f32);
            }
            if acc.norm() > best.1 {
                best = (w, acc.norm());
            }
        }
        let w = best.0;
        // Remove the frequency, then a phase estimate per boundary from 32 neighbours (fourth power, averaged).
        c.zr.clear();
        c.zr.extend(c.z.iter().enumerate().map(|(k, v)| {
            let p = -w * k as f64;
            v * Complex32::new(p.cos() as f32, p.sin() as f32)
        }));
        c.z4.clear();
        c.z4.extend(c.zr.iter().map(quad));
        let len = c.zr.len();
        c.refs.clear();
        for k in 0..len {
            let lo = k.saturating_sub(16);
            let hi = (k + 16).min(len);
            let mut acc = Complex32::default();
            for v in &c.z4[lo..hi] {
                acc += v;
            }
            c.refs.push(f64::from(acc.arg()) / 4.0);
        }
        // Unwrap the estimate by quarter turns so it is continuous.
        for k in 1..len {
            let mut dlt = c.refs[k] - c.refs[k - 1];
            while dlt > FRAC_PI_2 / 2.0 {
                c.refs[k] -= FRAC_PI_2;
                dlt -= FRAC_PI_2;
            }
            while dlt < -FRAC_PI_2 / 2.0 {
                c.refs[k] += FRAC_PI_2;
                dlt += FRAC_PI_2;
            }
        }
        // The best path over S (mod 4), from S[k] to S[k+1] = S[k] +- 1. The metric is the projection of z on its expected phase.
        let mut metric = [0.0f64; 4];
        c.back.clear();
        for (k, v) in c.zr.iter().enumerate() {
            let mut next = [f64::NEG_INFINITY; 4];
            let mut from = [0u8; 4];
            for s in 0..4usize {
                let e = c.refs[k] + FRAC_PI_2 * s as f64;
                let gain = f64::from(v.re) * e.cos() + f64::from(v.im) * e.sin();
                if k == 0 {
                    next[s] = gain;
                    continue;
                }
                for prev in [(s + 3) % 4, (s + 1) % 4] {
                    let m = metric[prev] + gain;
                    if m > next[s] {
                        next[s] = m;
                        from[s] = prev as u8;
                    }
                }
            }
            metric = next;
            c.back.push(from);
        }
        let mut s = (0..4)
            .max_by(|&a, &b| metric[a].total_cmp(&metric[b]))
            .unwrap_or(0);
        c.path.clear();
        c.path.resize(len, 0);
        for k in (0..len).rev() {
            c.path[k] = s;
            s = c.back[k][s] as usize;
        }
        self.levels.clear();
        for k in 0..len.saturating_sub(1) {
            let up = (c.path[k + 1] + 4 - c.path[k]) % 4 == 1;
            self.levels.push(u8::from(up));
        }
        true
    }

    /// The sequence search over the bit integrals: each integral is `g1 a[k] + g0 a[k+1] + g2 a[k-1] + c` (the intersymbol
    /// interference of the Gaussian pulse), with the channel fitted by least squares to the hard decisions at `thr`, then the most
    /// likely levels by `sdrkit_algo_modem::gmsk::Mlse` into `self.levels`. Returns false when there are too few integrals.
    fn sequence_search(&mut self, thr: f64) -> bool {
        let n = self.d.len();
        if !(MIN_SEARCH..=MAX_BITS + 1).contains(&n) {
            return false;
        }
        let hard = |v: f64| if v > thr { 1.0f32 } else { -1.0 };
        // Delayed by one bit so the model is causal: y_j = d[j], newest symbol a[j + 1].
        self.mlse_y.clear();
        self.mlse_s.clear();
        for j in 0..n - 1 {
            self.mlse_y.push(Complex32::new(self.d[j] as f32, 0.0));
            self.mlse_s.push(Complex32::new(hard(self.d[j + 1]), 0.0));
        }
        let mut taps = [Complex32::default(); 3];
        let (taps, nv) = match ls_channel(&self.mlse_y, &self.mlse_s, &mut taps, true) {
            Ok(fit) => {
                for y in &mut self.mlse_y {
                    *y -= fit.dc;
                }
                (taps, fit.noise_var)
            }
            Err(_) => (default_taps(), 0.1),
        };
        if self.mlse.set_channel(&taps).is_err() {
            return false;
        }
        self.mlse_bits.clear();
        self.mlse_bits.resize(n - 1, 0);
        if self
            .mlse
            .decode(
                &self.mlse_y,
                nv.max(NOISE_VAR_FLOOR),
                None,
                &mut self.mlse_bits,
            )
            .is_err()
        {
            return false;
        }
        // Bit 0 is the symbol +1, which is line level 1. The first level keeps its hard decision.
        self.levels.clear();
        self.levels.push(u8::from(self.d[0] > thr));
        self.levels
            .extend(self.mlse_bits.iter().map(|&b| 1 - (b & 1)));
        true
    }

    /// Decodes one burst.
    pub fn decode(&mut self, cut: &BurstCut, opts: &DecodeOptions) -> BurstResult {
        let mut x = std::mem::take(&mut self.x);
        x.clear();
        x.extend_from_slice(&cut.samples);
        let res = self.decode_in(&mut x, cut, opts);
        self.x = x;
        res
    }

    fn decode_in(
        &mut self,
        x: &mut [Complex32],
        cut: &BurstCut,
        opts: &DecodeOptions,
    ) -> BurstResult {
        let mut res = BurstResult::default();
        let mut core = cut.core;
        let n = x.len();
        let layers = if opts.satellite {
            3
        } else if opts.collision {
            2
        } else {
            1
        };
        for layer in 0..layers {
            // A remainder's carrier comes from the squared signal's lines: the mean frequency of what is left leans on the data and
            // on the edges of the cancelled burst.
            let f0 = if layer > 0 {
                self.carrier_lines(x, core, 2000.0)
                    .unwrap_or_else(|| Self::carrier(x, core))
            } else {
                Self::carrier(x, core)
            };
            self.cands.clear();
            self.cands.push(f0);
            if layer > 0 || opts.collision || opts.satellite {
                if opts.satellite {
                    self.cands.extend_from_slice(&opts.hints);
                    let mut g = -4500.0;
                    while g <= 4500.0 {
                        self.cands.push(g);
                        g += 1500.0;
                    }
                } else {
                    self.cands
                        .extend([-1000.0, -500.0, 500.0, 1000.0].map(|d| f0 + d));
                }
            }
            let max_c = if opts.satellite {
                8
            } else if opts.collision {
                3
            } else {
                1
            };
            let mut used = 0;
            let mut found: Option<(f64, f64)> = None;
            let mut first = true;
            self.tried.clear();
            for ci in 0..self.cands.len() {
                let fc = self.cands[ci];
                if used >= max_c {
                    break;
                }
                let skip = if opts.satellite { 500.0 } else { 250.0 };
                if self.tried.iter().any(|t| (t - fc).abs() < skip) {
                    continue;
                }
                self.tried.push(fc);
                used += 1;
                let (tau, bad) = self.attempt(x, core, fc);
                if let Some(tau) = tau {
                    found = Some((tau, fc));
                    break;
                }
                if first {
                    res.bad |= bad;
                }
                first = false;
                if !opts.collision && !opts.satellite {
                    break;
                }
            }
            let Some((tau, fc)) = found else {
                break;
            };
            let bits = self.frame.clone();
            // The fit is needed for cancellation and for the fitted power of extra layers.
            let fit = if opts.collision || opts.satellite {
                self.fit(x, &bits, fc, tau, false)
            } else {
                None
            };
            let floor = cut.floor.max(1e-20);
            let (a, b) = (core.0 + 24, core.1.saturating_sub(24).max(core.0 + 25));
            let (ra, rb) = (a.min(n), b.min(n));
            let p_core: f64 = x[ra..rb]
                .iter()
                .map(|s| f64::from(s.norm_sqr()))
                .sum::<f64>()
                / rb.saturating_sub(ra).max(1) as f64;
            let sig = match &fit {
                Some(f) if layer > 0 || opts.satellite => f.power,
                _ => (p_core - floor).max(1e-20),
            };
            let snr = 10.0 * (sig / (floor * opts.noise_scale.max(1e-9))).log10();
            let layer_kind = if layer > 0 {
                Layer::Cancelled
            } else if (fc - f0).abs() > 1.0 {
                Layer::Recovered
            } else {
                Layer::Direct
            };
            let offset = fit.as_ref().map_or(fc, |f| f.offset_hz);
            let start = fit.as_ref().map_or(core.0 as f64, |f| f.start);
            // Extra tries are kept only 6 dB above the noise.
            if (layer_kind != Layer::Direct && snr < 6.0)
                || res.frames.iter().any(|f| f.bits == bits)
            {
                break;
            }
            res.frames.push(Frame {
                bits,
                start_index: cut.first as f64 + start,
                snr_db: snr,
                offset_hz: offset,
                layer: layer_kind,
                channel: cut.channel,
            });
            res.bad = false;
            if layer + 1 >= layers {
                break;
            }
            if fit.is_none() {
                break;
            }
            // Cancel and see whether a strong remainder is left. The residual of a fit: the mean power left in the core.
            let residual = |x: &[Complex32], w: &Wave| -> f64 {
                (ra..rb)
                    .map(|i| f64::from((x[i] - w.at(i)).norm_sqr()))
                    .sum::<f64>()
                    / rb.saturating_sub(ra).max(1) as f64
            };
            let mut rem = residual(x, &self.best);
            // Nothing strong is left: a clean burst (the fit's residual is a small part of the burst's own power).
            let clean = |rem: f64| rem < 3.0 * floor || rem < 0.01 * p_core;
            if !clean(rem) {
                // Something is left: fit the transmitter's edges too (its power ramp and the ramp-down bits), which a weaker
                // burst underneath needs to come out clean.
                std::mem::swap(&mut self.keep, &mut self.best);
                let last = res.frames.len() - 1;
                let g = self.fit(x, &res.frames[last].bits, fc, tau, true);
                let better = g.is_some() && {
                    let r2 = residual(x, &self.best);
                    if r2 < rem {
                        rem = r2;
                        true
                    } else {
                        false
                    }
                };
                if !better {
                    std::mem::swap(&mut self.keep, &mut self.best);
                }
            }
            if clean(rem) {
                break;
            }
            let w = &self.best;
            for i in w.a..w.e.min(n) {
                x[i] -= w.buf[i];
            }
            // The remainder's core: where its power (a 48-sample mean) stands clearly above the floor and within 8 dB of its peak.
            const W: usize = 48;
            let boxed = |x: &[Complex32], f: &mut dyn FnMut(usize, f64)| {
                let mut acc = 0.0f64;
                for i in 0..x.len() {
                    acc += f64::from(x[i].norm_sqr());
                    if i >= W {
                        acc -= f64::from(x[i - W].norm_sqr());
                    }
                    if i + 1 >= W {
                        f(i + 1 - W / 2, acc / W as f64);
                    }
                }
            };
            let mut peak = 0.0f64;
            boxed(x, &mut |_, p| peak = peak.max(p));
            let thr = (3.0 * floor).max(0.16 * peak);
            let (mut c0, mut c1) = (None, 0usize);
            boxed(x, &mut |i, p| {
                if p > thr {
                    if c0.is_none() {
                        c0 = Some(i);
                    }
                    c1 = i;
                }
            });
            match c0 {
                Some(c) if c1 > c + 100 => core = (c, c1),
                _ => break,
            }
        }
        res
    }

    /// Remodulates a decoded frame and fits it (timing, frequency, amplitude, phase) through the channel filter to the samples. The
    /// waveform of the fit is left in `self.best`.
    fn fit(
        &mut self,
        x: &[Complex32],
        bits: &[u8],
        f_hint: f64,
        tau: f64,
        edges: bool,
    ) -> Option<Fit> {
        let mut levels = std::mem::take(&mut self.remod);
        let mut trial = std::mem::take(&mut self.trial);
        let r = self.fit_with(x, bits, f_hint, tau, edges, &mut levels, &mut trial);
        self.remod = levels;
        self.trial = trial;
        r
    }

    #[allow(clippy::too_many_arguments)]
    fn fit_with(
        &mut self,
        x: &[Complex32],
        bits: &[u8],
        f_hint: f64,
        tau: f64,
        edges: bool,
        levels: &mut Vec<u8>,
        trial: &mut Vec<u8>,
    ) -> Option<Fit> {
        self.coder.levels(bits, 0, levels);
        let n_frame = levels.len();
        // The frame's place in the decoded levels determines its start: level j holds the window [tau + 5 j, tau + 5 j + 5).
        let dec = &self.ok_levels;
        if dec.len() < n_frame {
            return None;
        }
        let mut best = (0usize, 0usize, false);
        for k in 0..=dec.len() - n_frame {
            let same = levels.iter().zip(&dec[k..]).filter(|(a, b)| a == b).count();
            let (m, inv) = if same * 2 >= n_frame {
                (same, false)
            } else {
                (n_frame - same, true)
            };
            if m > best.1 {
                best = (k, m, inv);
            }
        }
        if best.2 {
            levels.iter_mut().for_each(|l| *l = 1 - *l);
        }
        let t0 = tau + SPB * best.0 as f64;
        // The ramp-down bits after the end flag hold the level. How many were sent is read from the decoded levels when they show
        // it (another burst can go on after this one, so the counts 0, 4 and 8 are tried as well).
        let last = levels[n_frame - 1];
        let after = &dec[(best.0 + n_frame).min(dec.len())..(best.0 + n_frame + 8).min(dec.len())];
        let seen = after
            .iter()
            .take_while(|&&l| (if best.2 { 1 - l } else { l }) == last)
            .count();
        levels.extend(std::iter::repeat_n(last, seen));
        // Timing: three trials and a parabola through their scores.
        let sc = [-0.4, 0.0, 0.4].map(|d| self.fit_at(x, levels, t0 + d, f_hint, 0.0).score);
        let (a, b, c) = (sc[0], sc[1], sc[2]);
        let den = a - 2.0 * b + c;
        let d = if den < 0.0 {
            (0.5 * (a - c) / den * 0.4).clamp(-0.6, 0.6)
        } else if a > c {
            -0.4
        } else {
            0.4
        };
        let mut f = self.fit_at(x, levels, t0 + d, f_hint, 0.0);
        self.take_candidate();
        // Refine the frequency (and the timing once more) from a weighted straight-line fit of the phase of x conj(s).
        for _ in 0..2 {
            let mut ph = 0.0f64;
            let mut prev: Option<Complex32> = None;
            let (mut sw, mut st, mut sp, mut stt, mut stp) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
            let w = &self.best;
            for i in w.a..w.e.min(x.len()) {
                let s = w.buf[i];
                let wt = f64::from(s.norm_sqr());
                if wt < 1e-12 {
                    continue;
                }
                let p = x[i] * s.conj();
                if let Some(q) = prev {
                    ph += f64::from((p * q.conj()).arg());
                }
                prev = Some(p);
                let t = i as f64;
                sw += wt;
                st += wt * t;
                sp += wt * ph;
                stt += wt * t * t;
                stp += wt * t * ph;
            }
            let den = sw * stt - st * st;
            if den <= 0.0 {
                break;
            }
            let slope = (sw * stp - st * sp) / den;
            let df = slope * RATE / TAU;
            if df.abs() < 0.5 {
                break;
            }
            let f2 = self.fit_at(x, levels, f.start, f.offset_hz + df, 0.0);
            if f2.score > f.score {
                f = f2;
                self.take_candidate();
            } else {
                break;
            }
        }
        for d in [-0.1, 0.1] {
            let f2 = self.fit_at(x, levels, f.start + d, f.offset_hz, 0.0);
            if f2.score > f.score {
                f = f2;
                self.take_candidate();
            }
        }
        // The edges: the transmitter's power ramp (none, or a raised cosine over one or two bits) and the ramp-down bits sent.
        let mut ramp = 0.0;
        let tries: &[usize] = if edges { &[seen, 0, 4, 8] } else { &[] };
        for &extra in tries {
            for r in [0.0, 1.0, 2.0] {
                if extra == seen && r == 0.0 {
                    continue;
                }
                trial.clear();
                trial.extend_from_slice(&levels[..n_frame]);
                trial.extend(std::iter::repeat_n(last, extra));
                let f2 = self.fit_at(x, trial, f.start, f.offset_hz, r);
                if f2.score > f.score {
                    f = f2;
                    self.take_candidate();
                    ramp = r;
                    std::mem::swap(levels, trial);
                }
            }
        }
        // Last, the receiver's filters as they are: a least-squares channel between the remodulated burst and the samples.
        if let Some(g) = self.fit_ls(x, levels, f.start, f.offset_hz, ramp) {
            if g.score >= f.score {
                f = g;
                self.take_candidate();
            }
        }
        Some(f)
    }

    /// The candidate waveform becomes the best one.
    fn take_candidate(&mut self) {
        std::mem::swap(&mut self.cand, &mut self.best);
    }

    /// The burst of `levels` remodulated from `start` (samples) at `offset_hz` with a `ramp` of bits, into `self.raw` over the
    /// samples `a..e` of a signal of `n` samples (`before` and `after` samples beyond the burst). Returns `(a, e)`.
    #[allow(clippy::too_many_arguments)]
    fn remodulate(
        &mut self,
        n: usize,
        levels: &[u8],
        start: f64,
        offset_hz: f64,
        ramp: f64,
        before: usize,
        after: usize,
    ) -> (usize, usize) {
        let a = ((start.floor() as isize) - before as isize).max(0) as usize;
        let e = (((start + SPB * levels.len() as f64).ceil() as usize) + after).min(n);
        self.raw.clear();
        self.raw.resize(e.saturating_sub(a), Complex32::default());
        if a < e {
            let p = Params {
                start_s: start / RATE,
                amplitude: 1.0,
                offset_hz,
                phase: 0.0,
                clock_ppm: 0.0,
                ramp_bits: ramp,
            };
            self.modulator
                .add(levels, &p, &mut self.raw, a as f64 / RATE, RATE);
        }
        (a, e)
    }

    /// A fit through the channel filter (its delay taken out, as the samples' delay is already in the timing) with a complex gain
    /// by projection. The waveform goes to `self.cand`.
    fn fit_at(
        &mut self,
        x: &[Complex32],
        levels: &[u8],
        start: f64,
        offset_hz: f64,
        ramp: f64,
    ) -> Fit {
        let n = x.len();
        let reach = self.chan.len();
        let (a, e) = self.remodulate(n, levels, start, offset_hz, ramp, reach, reach + 1);
        self.cand.reset(n);
        let e = e.max(a);
        if a < e {
            // The lengths are equal, so the convolution cannot fail.
            let _ = self.chan.centred(&self.raw, &mut self.cand.buf[a..e]);
        }
        let mut xs = Complex32::default();
        let mut ss = 0.0f64;
        for (p, s) in x[a..e].iter().zip(&self.cand.buf[a..e]) {
            xs += p * s.conj();
            ss += f64::from(s.norm_sqr());
        }
        let g = if ss > 0.0 {
            xs / ss as f32
        } else {
            Complex32::default()
        };
        for s in &mut self.cand.buf[a..e] {
            *s *= g;
        }
        (self.cand.a, self.cand.e) = (a, e);
        Fit {
            start,
            offset_hz,
            power: f64::from(g.norm_sqr()),
            score: f64::from(xs.norm()) / ss.max(1e-12).sqrt(),
        }
    }

    /// A least-squares channel of [`LS_TAPS`] complex taps (lags -4 to +4) between the remodulated burst and the samples, fitted
    /// as a real system of twice as many unknowns by `sdrkit_algo_dsp::linalg::LeastSquares`. The waveform goes to `self.cand`. `None`
    /// when the span is too short or the system is singular.
    fn fit_ls(
        &mut self,
        x: &[Complex32],
        levels: &[u8],
        start: f64,
        offset_hz: f64,
        ramp: f64,
    ) -> Option<Fit> {
        const HALF: usize = LS_TAPS / 2;
        let n = x.len();
        let (a, e) = self.remodulate(n, levels, start, offset_hz, ramp, 16, 16);
        if e <= a + 2 * LS_TAPS {
            return None;
        }
        let len = e - a;
        let raw = &self.raw;
        // The sample of the remodulated burst that tap j weighs for output i: raw[i + 4 - j], zero outside.
        let r = |i: usize, j: usize| -> Complex32 {
            (i + HALF)
                .checked_sub(j)
                .and_then(|k| raw.get(k))
                .copied()
                .unwrap_or_default()
        };
        // x = sum_j h_j r_j in real terms: Re x = sum (hr_j Re r_j - hi_j Im r_j), Im x = sum (hr_j Im r_j + hi_j Re r_j).
        self.ls.reset();
        let mut row = [0.0f64; 2 * LS_TAPS];
        for i in 0..len {
            let xi = x[a + i];
            for j in 0..LS_TAPS {
                let v = r(i, j);
                row[j] = f64::from(v.re);
                row[LS_TAPS + j] = -f64::from(v.im);
            }
            self.ls.add_row(&row, f64::from(xi.re), 1.0).ok()?;
            for j in 0..LS_TAPS {
                let v = r(i, j);
                row[j] = f64::from(v.im);
                row[LS_TAPS + j] = f64::from(v.re);
            }
            self.ls.add_row(&row, f64::from(xi.im), 1.0).ok()?;
        }
        let mut h = [0.0f64; 2 * LS_TAPS];
        self.ls.solve(0.0, &mut h).ok()?;
        let taps: [Complex32; LS_TAPS] =
            std::array::from_fn(|j| Complex32::new(h[j] as f32, h[LS_TAPS + j] as f32));
        self.cand.reset(n);
        let (mut xs, mut ss) = (Complex32::default(), 0.0f64);
        for i in 0..len {
            let mut acc = Complex32::default();
            for (j, g) in taps.iter().enumerate() {
                acc += g * r(i, j);
            }
            xs += x[a + i] * acc.conj();
            ss += f64::from(acc.norm_sqr());
            self.cand.buf[a + i] = acc;
        }
        (self.cand.a, self.cand.e) = (a, e);
        Some(Fit {
            start,
            offset_hz,
            power: ss / len as f64 * (len as f64 / (SPB * levels.len() as f64)).max(1.0),
            score: f64::from(xs.norm()) / ss.max(1e-12).sqrt(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ais::encode;

    fn frame_bits() -> Vec<u8> {
        encode::position_a(&encode::PositionA {
            msg_type: 1,
            mmsi: 244_123_456,
            status: 0,
            rot_raw: 0,
            sog_kn: Some(12.5),
            lat: Some(52.5),
            lon: Some(5.25),
            cog_deg: Some(80.0),
            heading: Some(80),
            second: 10,
        })
    }

    #[test]
    fn a_clean_burst_decodes() {
        let bits = frame_bits();
        let b = gmsk::Burst {
            levels: line::burst_levels(&bits, 8),
            start_s: 0.1,
            amplitude: 0.3,
            offset_hz: 700.0,
            phase: 0.3,
            clock_ppm: 0.0,
            ramp_bits: 1.0,
        };
        let n = (0.25 * RATE) as usize;
        let mut x = vec![Complex32::default(); n];
        b.add_to(&mut x, 0.0, RATE);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
        for s in &mut x {
            *s += Complex32::new((rng.normal() * 0.003) as f32, (rng.normal() * 0.003) as f32);
        }
        let mut det = Detector::new(0);
        let mut cuts = Vec::new();
        det.process(&x, |c| cuts.push(c));
        assert_eq!(cuts.len(), 1);
        let mut dec = Decoder::default();
        let r = dec.decode(
            &cuts[0],
            &DecodeOptions {
                noise_scale: 1.0,
                ..Default::default()
            },
        );
        assert_eq!(r.frames.len(), 1);
        assert_eq!(r.frames[0].bits, bits);
        assert!(
            (r.frames[0].offset_hz - 700.0).abs() < 150.0,
            "{}",
            r.frames[0].offset_hz
        );
    }
}
