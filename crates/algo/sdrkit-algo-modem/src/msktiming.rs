//! MSK symbol-clock timer: the time of the symbol boundaries of an MSK station whose symbol clock comes from a frequency
//! standard (a VLF or LF timing transmitter), read once per group of about one second, with its error.
//!
//! [`MskTimer`] mixes the carrier (its offset from the tuned centre is a setting) to 0 Hz and averages runs of
//! `D = max(1, floor(rate / (8 baud)))` input samples, about 8 averages a symbol, each stamped at the centre of its input samples.
//! Per group of about 1 s, cut into parts of about 50 ms:
//!
//! 1. The squared signal has lines at `+-baud/2`. Their sums over each part give a coarse boundary (modulo the symbol period `T`),
//!    the coherence `R` (0 to 1) of the parts and a coarse carrier offset (the product of the two lines turns at four times the
//!    offset).
//! 2. The bits are read at the coarse boundary from the phase change between `0.15 T` and `0.85 T` of each symbol. Each average
//!    gets its exact model, the average of the MSK phasor over the same input samples, split where a boundary falls inside. The
//!    boundary maximises `J = sum over parts |sum y conj(m)|^2 / sum |m|^2` (a parabolic line search within `T/4`).
//! 3. The phase slope over the parts (the carrier left over) is removed twice and the search is repeated with a smaller step.
//! 4. The 1-sigma error is `1 / sqrt(2 SNR N (pi / 2T)^2 max(1e-3, 4 n+ n- / N^2))`: `N` averages, `n+` and `n-` of them in
//!    symbols of each sign, SNR per average.
//!
//! The mixer follows the carrier, the sample-clock rate comes from the last locked boundaries, and readings go out once the
//! first three locked groups have settled both. [`MskSignal`] makes a matching test signal.

use crate::{Complex32, ModemError, Param};
use num_complex::Complex64;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::peak::{line_fit, parabolic};
use sdrkit_util_bits::lfsr::Lfsr;
use std::collections::VecDeque;
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::ops::Range;

/// Averages a symbol that the decimation aims at.
const AVERAGES_PER_SYMBOL: f64 = 8.0;
/// Length of a part, seconds, and its least number of averages.
const PART_SECONDS: f64 = 0.05;
const PART_MIN: usize = 8;
/// Length of a group, seconds.
const GROUP_SECONDS: f64 = 1.0;
/// The bit is the sign of the phase change from this fraction of the symbol to one minus it.
const BIT_FROM: f64 = 0.15;
/// Most iterations of one line search.
const SEARCH_STEPS: usize = 40;
/// Mixer updates that take the full measured step; later ones take half.
const FULL_STEPS: u32 = 2;
/// Locked groups that only settle the carrier and the rate before readings go out.
const SETTLE_GROUPS: u32 = 3;
/// Locked boundaries in the rate fit.
const RATE_HISTORY: usize = 30;
/// Limit of the measured sample-clock rate error, ppm.
const MAX_PPM: f64 = 200.0;
/// Least coherence of a locked group, and of a group whose coarse carrier offset moves the mixer.
const MIN_COHERENCE: f64 = 0.3;
/// The carrier offset from the phase slope steers the mixer when it is under this, Hz.
const PRECISE_MAX_HZ: f64 = 2.0;
/// Input samples mixed at a time.
const MIX_CHUNK: usize = 1024;
/// Register of the test signal's bits: `x^23 + x^18 + 1`.
const PRBS23: u64 = (1 << 23) | (1 << 18) | 1;

/// One reading of [`MskTimer`]: the symbol boundary nearest the middle of a group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MskReading {
    /// The boundary as a fractional sample index of the input, in the index frame of `first` of [`MskTimer::process`].
    pub boundary: f64,
    /// The symbol period in input samples at the measured sample-clock rate: boundaries lie at `boundary + k period`.
    pub period: f64,
    /// The 1-sigma error of `boundary`, seconds.
    pub sigma_s: f64,
    /// The SNR of one average (residual of the fit per average), dB.
    pub snr_db: f64,
    /// Whether the group is locked: a good fit, coherence above 0.3 and `sigma_s` under `T/200`.
    pub locked: bool,
    /// The carrier minus the set offset, Hz, as the sample clock sees it.
    pub carrier_error_hz: f64,
    /// The sample-clock rate error, ppm; positive when the sample clock runs fast.
    pub rate_ppm: f64,
    /// The coherence `R` of the square-law lines over the parts, 0 to 1.
    pub coherence: f64,
    /// Restarts (gaps or jumps of the input index) before this reading. Readings of different counts are not on one time line.
    pub restarts: u64,
}

/// The MSK symbol-clock timer (module documentation).
#[derive(Clone, Debug)]
pub struct MskTimer {
    rate: f64,
    offset_hz: f64,
    baud: f64,
    dec: usize,
    part: usize,
    group_len: usize,
    max_steer_hz: f64,
    nco: Nco,
    steer_hz: f64,
    steps: u32,
    /// Input index of the first input sample of average 0.
    origin: Option<u64>,
    /// The input index expected next.
    next: u64,
    acc: Complex64,
    acc_n: usize,
    group: Vec<Complex64>,
    groups: u64,
    ppm: f64,
    /// Locked boundaries: whole periods from the first, and the boundary (input index).
    history: VecDeque<(i64, f64)>,
    locked_groups: u32,
    restarts: u64,
    last: Option<MskReading>,
    mix: Vec<Complex32>,
    scratch: Scratch,
}

impl MskTimer {
    /// A timer for input at `rate` samples a second with the carrier `offset_hz` from the tuned centre and `baud` symbols a
    /// second. Errors when `rate` is not above 0 (up to 1e9), `baud` is not above 0 (up to 1e5 and `rate / 4`), or the offset is
    /// outside `+-rate/2`.
    pub fn new(rate: f64, offset_hz: f64, baud: f64) -> Result<Self, ModemError> {
        Param::Rate.check_above(rate, 0.0, 1e9)?;
        Param::Baud.check_above(baud, 0.0, 1e5_f64.min(rate / 4.0))?;
        Param::Frequency.check_in(offset_hz, -rate / 2.0, rate / 2.0)?;
        let dec = ((rate / (AVERAGES_PER_SYMBOL * baud)).floor() as usize).max(1);
        let fs2 = rate / dec as f64;
        let part = ((PART_SECONDS * fs2).round() as usize).max(PART_MIN);
        let parts = ((GROUP_SECONDS * fs2 / part as f64).round() as usize).max(1);
        // The coarse offset is unambiguous while it turns the line product by less than half a turn per part.
        let max_steer_hz = fs2 / (8.0 * part as f64);
        Ok(MskTimer {
            rate,
            offset_hz,
            baud,
            dec,
            part,
            group_len: parts * part,
            max_steer_hz,
            nco: Nco::new(rate, -offset_hz),
            steer_hz: 0.0,
            steps: 0,
            origin: None,
            next: 0,
            acc: Complex64::default(),
            acc_n: 0,
            group: Vec::with_capacity(parts * part),
            groups: 0,
            ppm: 0.0,
            history: VecDeque::with_capacity(RATE_HISTORY + 1),
            locked_groups: 0,
            restarts: 0,
            last: None,
            mix: vec![Complex32::default(); MIX_CHUNK],
            scratch: Scratch::default(),
        })
    }

    /// The input rate, samples a second.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The set carrier offset from the tuned centre, Hz.
    pub fn offset_hz(&self) -> f64 {
        self.offset_hz
    }

    /// The set symbol rate, baud.
    pub fn baud(&self) -> f64 {
        self.baud
    }

    /// The input samples in one average.
    pub fn decimation(&self) -> usize {
        self.dec
    }

    /// The input samples in one group (the spacing of the readings).
    pub fn group_samples(&self) -> u64 {
        (self.group_len * self.dec) as u64
    }

    /// The symbol period in input samples at the measured sample-clock rate.
    pub fn period_samples(&self) -> f64 {
        self.period_s() * self.rate
    }

    /// The measured sample-clock rate error, ppm (0 until two groups have locked).
    pub fn rate_ppm(&self) -> f64 {
        self.ppm
    }

    /// The mixer's offset from the set carrier offset, Hz.
    pub fn steer_hz(&self) -> f64 {
        self.steer_hz
    }

    /// The count of restarts: gaps or jumps of the input index.
    pub fn restarts(&self) -> u64 {
        self.restarts
    }

    /// The result of the last group, also while the first groups settle and no reading goes out.
    pub fn last(&self) -> Option<&MskReading> {
        self.last.as_ref()
    }

    /// Whether the last group was locked.
    pub fn locked(&self) -> bool {
        self.last.is_some_and(|r| r.locked)
    }

    /// Forgets the signal: the mixer, the measured rate, the boundaries and the restart count start again.
    pub fn reset(&mut self) {
        self.nco = Nco::new(self.rate, -self.offset_hz);
        self.steer_hz = 0.0;
        self.steps = 0;
        self.ppm = 0.0;
        self.restarts = 0;
        self.last = None;
        self.origin = None;
        self.clear_run(0);
    }

    /// Starts the grid of averages and groups again at input index `first`. The mixer and the measured rate are kept.
    fn clear_run(&mut self, first: u64) {
        self.next = first;
        self.acc = Complex64::default();
        self.acc_n = 0;
        self.group.clear();
        self.groups = 0;
        self.history.clear();
        self.locked_groups = 0;
    }

    /// The symbol period in seconds of the sample clock.
    fn period_s(&self) -> f64 {
        (1.0 + self.ppm * 1e-6) / self.baud
    }

    /// Takes the input samples `x`, the first at absolute index `first`, and appends a reading to `out` for each group that
    /// ends inside them once the first three locked groups have passed. The averages and groups lie on a grid of absolute
    /// indices from the first sample seen, so the readings do not depend on how the input is split. When `first` is not the
    /// index that follows the last call, the timer restarts its grid at `first` (the boundaries, the rate history and the
    /// settling start again; the mixer and the measured rate are kept). A sample that is not finite counts as 0.
    pub fn process(&mut self, first: u64, x: &[Complex32], out: &mut Vec<MskReading>) {
        match self.origin {
            None => {
                self.origin = Some(first);
                self.clear_run(first);
            }
            Some(_) if first != self.next => {
                self.origin = Some(first);
                self.restarts += 1;
                self.clear_run(first);
            }
            Some(_) => {}
        }
        let origin = self.origin.unwrap_or(first);
        let group_in = self.group_samples();
        let mut i = 0;
        while i < x.len() {
            let left = (group_in - (self.next - origin) % group_in) as usize;
            let n = left.min(x.len() - i).min(MIX_CHUNK);
            for (m, &v) in self.mix.iter_mut().zip(&x[i..i + n]) {
                *m = if v.re.is_finite() && v.im.is_finite() {
                    v
                } else {
                    Complex32::default()
                };
            }
            self.nco.mix_in_place(&mut self.mix[..n]);
            for v in &self.mix[..n] {
                self.acc += Complex64::new(v.re as f64, v.im as f64);
                self.acc_n += 1;
                if self.acc_n == self.dec {
                    self.group.push(self.acc / self.dec as f64);
                    self.acc = Complex64::default();
                    self.acc_n = 0;
                }
            }
            i += n;
            self.next += n as u64;
            if self.group.len() == self.group_len {
                self.finish_group(origin, out);
            }
        }
    }

    /// Fits the group just completed, moves the mixer from the next input sample on, updates the rate and the lock, and
    /// appends the reading once settled.
    fn finish_group(&mut self, origin: u64, out: &mut Vec<MskReading>) {
        let dec = self.dec as f64;
        let stamp0 = origin as f64 + (self.groups * self.group_len as u64) as f64 * dec + (dec - 1.0) / 2.0;
        let tp = self.period_s();
        let geometry = Geometry {
            fs: self.rate,
            dec: self.dec,
            part: self.part,
            tp,
        };
        let fit = fit_group(&mut self.group, &geometry, &mut self.scratch);
        self.group.clear();
        self.groups += 1;

        let boundary = stamp0 + fit.tau * self.rate;
        let precise = fit.good && fit.precise_hz.abs() < PRECISE_MAX_HZ;
        let residual = if precise {
            fit.precise_hz
        } else {
            fit.coarse_hz
        };
        let carrier_error_hz = self.steer_hz + residual;
        let locked = fit.good && fit.coherence > MIN_COHERENCE && fit.sigma < tp / 200.0;

        if precise || fit.coherence > MIN_COHERENCE {
            let gain = if self.steps < FULL_STEPS { 1.0 } else { 0.5 };
            self.steer_hz =
                (self.steer_hz + gain * residual).clamp(-self.max_steer_hz, self.max_steer_hz);
            self.steps = self.steps.saturating_add(1);
            self.nco.set_freq(-(self.offset_hz + self.steer_hz));
        }
        if locked {
            self.add_boundary(boundary);
        }
        let settled = self.locked_groups >= SETTLE_GROUPS;
        if locked {
            self.locked_groups = self.locked_groups.saturating_add(1);
        }
        let reading = MskReading {
            boundary,
            period: self.period_samples(),
            sigma_s: fit.sigma,
            snr_db: 10.0 * fit.snr.max(1e-10).log10(),
            locked,
            carrier_error_hz,
            rate_ppm: self.ppm,
            coherence: fit.coherence,
            restarts: self.restarts,
        };
        self.last = Some(reading);
        if settled {
            out.push(reading);
        }
    }

    /// Adds a locked boundary to the rate history and refits the rate. Whole periods are counted from the previous locked
    /// boundary with the current rate, so a rate error far from 0 does not break the count.
    fn add_boundary(&mut self, b: f64) {
        let period = self.period_samples();
        let periods = match self.history.back() {
            Some(&(n, last)) => n + ((b - last) / period).round() as i64,
            None => 0,
        };
        self.history.push_back((periods, b));
        while self.history.len() > RATE_HISTORY {
            self.history.pop_front();
        }
        if self.history.len() < 2 {
            return;
        }
        let &(n0, b0) = self.history.back().unwrap_or(&(periods, b));
        let s = &mut self.scratch;
        s.xs.clear();
        s.ys.clear();
        for &(n, v) in &self.history {
            s.xs.push((n - n0) as f64);
            s.ys.push(v - b0);
        }
        if let Ok(line) = line_fit(&s.xs, &s.ys, None) {
            let nominal = self.rate / self.baud;
            self.ppm = ((line.slope / nominal - 1.0) * 1e6).clamp(-MAX_PPM, MAX_PPM);
        }
    }
}

/// The shape of a group.
struct Geometry {
    /// Input rate.
    fs: f64,
    /// Input samples an average.
    dec: usize,
    /// Averages a part.
    part: usize,
    /// Symbol period, seconds of the sample clock.
    tp: f64,
}

/// The fit of one group.
#[derive(Clone, Copy, Debug)]
struct Fit {
    /// The boundary nearest the middle, seconds after the stamp of the first average.
    tau: f64,
    coherence: f64,
    coarse_hz: f64,
    precise_hz: f64,
    snr: f64,
    sigma: f64,
    good: bool,
}

/// The sums of one part for a trial boundary.
#[derive(Clone, Copy, Debug, Default)]
struct PartSum {
    /// `sum y conj(m)`.
    c: Complex64,
    /// `sum |m|^2`.
    m: f64,
    /// `sum |y|^2`.
    e: f64,
    /// Sum of the stamps (seconds) and the count of averages.
    t: f64,
    n: usize,
}

/// Buffers reused from group to group.
#[derive(Clone, Debug, Default)]
struct Scratch {
    bits: Vec<f64>,
    phases: Vec<f64>,
    parts: Vec<PartSum>,
    xs: Vec<f64>,
    ys: Vec<f64>,
    ws: Vec<f64>,
}

/// The decided symbols of a group and the exact model of its averages.
struct Model<'a> {
    /// Spacing of the averages, seconds.
    ts: f64,
    fs: f64,
    dec: usize,
    /// From a stamp to its first input sample, seconds.
    half: f64,
    tp: f64,
    /// Boundary `m = 0` at the coarse fit, seconds after the first stamp.
    start: f64,
    m_lo: i64,
    /// `+1` or `-1` for symbols `m_lo ..`.
    bits: &'a [f64],
    /// MSK phase at the start of each symbol.
    phases: &'a [f64],
    /// Phase step per input sample of a `+1` symbol.
    beta: f64,
    /// `sum e^{j beta i}` over one whole average.
    g_full: Complex64,
}

/// `sum e^{j beta i}` for `i < n`.
fn geometric(beta: f64, n: usize) -> Complex64 {
    let s = (beta / 2.0).sin();
    if s.abs() < 1e-300 {
        return Complex64::new(n as f64, 0.0);
    }
    let n = n as f64;
    Complex64::from_polar((n * beta / 2.0).sin() / s, beta * (n - 1.0) / 2.0)
}

impl Model<'_> {
    /// The phasor sum over `n` input samples of symbol `m` from input time `u0` (seconds after the first stamp), boundaries at
    /// `base + m tp`.
    fn segment(&self, m: i64, base: f64, u0: f64, n: usize) -> Complex64 {
        if n == 0 {
            return Complex64::default();
        }
        let idx = (m - self.m_lo).clamp(0, self.bits.len() as i64 - 1) as usize;
        let a = self.bits[idx];
        let b = base + m as f64 * self.tp;
        let phase = self.phases[idx] + a * FRAC_PI_2 * (u0 - b) / self.tp;
        let g = if n == self.dec {
            self.g_full
        } else {
            geometric(self.beta, n)
        };
        let g = if a > 0.0 { g } else { g.conj() };
        Complex64::from_polar(1.0, phase) * g
    }

    /// The model of average `k` with the boundaries at `start + delta + m tp`.
    fn at(&self, k: usize, delta: f64) -> Complex64 {
        let base = self.start + delta;
        let c = k as f64 * self.ts;
        let (u0, u1) = (c - self.half, c + self.half);
        let ma = ((u0 - base) / self.tp).floor() as i64;
        let mb = ((u1 - base) / self.tp).floor() as i64;
        let sum = if ma == mb {
            self.segment(ma, base, u0, self.dec)
        } else {
            let b = base + mb as f64 * self.tp;
            let split = (((b - u0) * self.fs).ceil().max(0.0) as usize).min(self.dec);
            self.segment(ma, base, u0, split)
                + self.segment(mb, base, u0 + split as f64 / self.fs, self.dec - split)
        };
        sum / self.dec as f64
    }

    /// The part sums over the averages `used` for the boundaries moved by `delta`.
    fn sums(
        &self,
        y: &[Complex64],
        used: &Range<usize>,
        part: usize,
        delta: f64,
        out: &mut Vec<PartSum>,
    ) {
        out.clear();
        let mut cur = usize::MAX;
        for k in used.clone() {
            if k / part != cur {
                cur = k / part;
                out.push(PartSum::default());
            }
            let m = self.at(k, delta);
            let last = out.len() - 1;
            let p = &mut out[last];
            p.c += y[k] * m.conj();
            p.m += m.norm_sqr();
            p.e += y[k].norm_sqr();
            p.t += k as f64 * self.ts;
            p.n += 1;
        }
    }
}

/// `J = sum over parts |c|^2 / m`.
fn score(parts: &[PartSum]) -> f64 {
    parts
        .iter()
        .filter(|p| p.m > 0.0)
        .map(|p| p.c.norm_sqr() / p.m)
        .sum()
}

/// Parabolic line search of `J` over `delta` within `+-T/4`, from `from` with first step `step`; stops when a move is under
/// `T/20000` or after 40 iterations.
fn search(
    model: &Model,
    y: &[Complex64],
    used: &Range<usize>,
    part: usize,
    from: f64,
    step: f64,
    parts: &mut Vec<PartSum>,
) -> f64 {
    let lim = model.tp / 4.0;
    let tol = model.tp / 20000.0;
    let mut j = |d: f64| {
        if d.abs() > lim {
            return f64::NEG_INFINITY;
        }
        model.sums(y, used, part, d, parts);
        score(parts)
    };
    let mut x = from;
    let mut h = step;
    let mut jx = j(x);
    for _ in 0..SEARCH_STEPS {
        let jm = j(x - h);
        let jp = j(x + h);
        if jp > jx || jm > jx {
            if jp >= jm {
                x += h;
                jx = jp;
            } else {
                x -= h;
                jx = jm;
            }
            continue;
        }
        if !(jm.is_finite() && jp.is_finite()) {
            h *= 0.5;
            if h < tol {
                break;
            }
            continue;
        }
        let d = parabolic(jm, jx, jp).offset * h;
        x += d;
        jx = j(x);
        if d.abs() < tol || h < tol {
            break;
        }
        h *= 0.5;
    }
    x
}

/// The phase slope over the parts, rad/s: a weighted line through the unwrapped phases of `c` against the mean stamp of each
/// part. 0 when fewer than two parts carry weight.
fn phase_slope(
    parts: &[PartSum],
    xs: &mut Vec<f64>,
    ys: &mut Vec<f64>,
    ws: &mut Vec<f64>,
) -> f64 {
    xs.clear();
    ys.clear();
    ws.clear();
    let mut prev: Option<(Complex64, f64)> = None;
    for p in parts.iter().filter(|p| p.n > 0 && p.c.norm_sqr() > 0.0) {
        let phase = match prev {
            Some((c, ph)) => ph + (p.c * c.conj()).arg(),
            None => p.c.arg(),
        };
        prev = Some((p.c, phase));
        xs.push(p.t / p.n as f64);
        ys.push(phase);
        ws.push(p.c.norm());
    }
    line_fit(xs, ys, Some(ws)).map_or(0.0, |l| l.slope)
}

/// Steps 2 to 6 on the averages `y` of one group (`y` loses its residual carrier).
fn fit_group(y: &mut [Complex64], g: &Geometry, s: &mut Scratch) -> Fit {
    let ts = g.dec as f64 / g.fs;
    let tp = g.tp;
    let n = y.len();
    let (tau0, coherence, coarse_hz) = lines(y, g);
    let mid = (n.saturating_sub(1)) as f64 * ts / 2.0;
    let start = tau0 + ((mid - tau0) / tp).round() * tp;
    let mut fit = Fit {
        tau: start,
        coherence,
        coarse_hz,
        precise_hz: 0.0,
        snr: 0.0,
        sigma: tp / 50.0,
        good: false,
    };

    // The symbols whose reading instants lie inside the group, and their bits.
    let last = n.saturating_sub(1) as f64 * ts;
    let m_lo = (-start / tp).ceil() as i64;
    let m_hi = ((last - start) / tp - 1.0).floor() as i64;
    if m_hi - m_lo < 4 {
        return fit;
    }
    let Scratch {
        bits,
        phases,
        parts,
        xs,
        ys,
        ws,
    } = s;
    bits.clear();
    phases.clear();
    let mut ph = 0.0;
    for m in m_lo..=m_hi {
        let b = start + m as f64 * tp;
        let k1 = (((b + BIT_FROM * tp) / ts).round().max(0.0) as usize).min(n - 1);
        let k2 = (((b + (1.0 - BIT_FROM) * tp) / ts).round().max(0.0) as usize).min(n - 1);
        let a = if (y[k2] * y[k1].conj()).arg() >= 0.0 {
            1.0
        } else {
            -1.0
        };
        bits.push(a);
        phases.push(ph);
        ph = (ph + a * FRAC_PI_2).rem_euclid(TAU);
    }

    // The averages whose model is defined T/4 before and after the coarse boundary.
    let half = (g.dec - 1) as f64 / 2.0 / g.fs;
    let lo_t = start + m_lo as f64 * tp + tp / 4.0 + half;
    let hi_t = start + (m_hi + 1) as f64 * tp - tp / 4.0 - half;
    let k_lo = (lo_t / ts).ceil().max(0.0) as usize;
    let k_hi = ((hi_t / ts).floor().max(0.0) as usize).min(n - 1);
    if k_hi < k_lo + 2 * PART_MIN {
        return fit;
    }
    let used = k_lo..k_hi + 1;
    let beta = FRAC_PI_2 / (tp * g.fs);
    let model = Model {
        ts,
        fs: g.fs,
        dec: g.dec,
        half,
        tp,
        start,
        m_lo,
        bits,
        phases,
        beta,
        g_full: geometric(beta, g.dec),
    };

    let d1 = search(&model, y, &used, g.part, 0.0, tp / 16.0, parts);
    // The carrier left over: the phase slope over the parts, removed in two passes.
    let mut omega = 0.0;
    for _ in 0..2 {
        model.sums(y, &used, g.part, d1, parts);
        let w = phase_slope(parts, xs, ys, ws);
        for (k, v) in y.iter_mut().enumerate() {
            *v *= Complex64::from_polar(1.0, -w * (k as f64 * ts - mid));
        }
        omega += w;
    }
    let d2 = search(&model, y, &used, g.part, d1, tp / 64.0, parts);

    // The error of the boundary.
    model.sums(y, &used, g.part, d2, parts);
    let (mut es, mut et, mut mm) = (0.0, 0.0, 0.0);
    for p in parts.iter() {
        if p.m > 0.0 {
            es += p.c.norm_sqr() / p.m;
        }
        et += p.e;
        mm += p.m;
    }
    let count = used.len() as f64;
    let noise = ((et - es) / (count - parts.len() as f64).max(1.0)).max(1e-300);
    let snr = if mm > 0.0 { es / mm / noise } else { 0.0 };
    let (mut plus, mut minus) = (0.0, 0.0);
    for k in used.clone() {
        let m = ((k as f64 * ts - start - d2) / tp).floor() as i64;
        let idx = (m - m_lo).clamp(0, model.bits.len() as i64 - 1) as usize;
        if model.bits[idx] > 0.0 {
            plus += 1.0;
        } else {
            minus += 1.0;
        }
    }
    let balance = (4.0 * plus * minus / (count * count)).max(1e-3);
    let slope = PI / (2.0 * tp);
    let information = 2.0 * snr * count * slope * slope * balance;
    let good = d2.abs() < tp / 8.0 && snr > 1.0;
    fit.tau = start + d2;
    fit.precise_hz = omega / TAU;
    fit.snr = snr;
    fit.good = good;
    if good && information > 0.0 {
        fit.sigma = 1.0 / information.sqrt();
    }
    fit
}

/// The square-law lines at `+-baud/2` over the parts of `y`: the coarse boundary (seconds after the first stamp, in `[0, T)`),
/// the coherence `R` and the coarse carrier offset (Hz).
fn lines(y: &[Complex64], g: &Geometry) -> (f64, f64, f64) {
    let ts = g.dec as f64 / g.fs;
    let w = PI / g.tp;
    let mut q = Complex64::default();
    let mut qa = 0.0;
    let mut turn = Complex64::default();
    let mut prev: Option<Complex64> = None;
    for (j, chunk) in y.chunks(g.part).enumerate() {
        let (mut ap, mut am) = (Complex64::default(), Complex64::default());
        for (i, v) in chunk.iter().enumerate() {
            let t = (j * g.part + i) as f64 * ts;
            let z = v * v;
            let e = Complex64::from_polar(1.0, w * t);
            ap += z * e.conj();
            am += z * e;
        }
        let qj = am * ap.conj();
        q += qj;
        qa += qj.norm();
        let pj = ap * am;
        if let Some(pp) = prev {
            turn += pj * pp.conj();
        }
        prev = Some(pj);
    }
    let tau = (q.arg() / TAU * g.tp).rem_euclid(g.tp);
    let coherence = if qa > 0.0 { q.norm() / qa } else { 0.0 };
    let part_s = g.part as f64 * ts;
    let coarse = turn.arg() / (TAU * 4.0 * part_s);
    (tau, coherence, coarse)
}

/// Continuous-phase MSK at a carrier offset: tones at `+-baud/4`, pseudo-random bits from a 23-stage register, symbol
/// boundaries at true times `first_boundary + k / baud`, sampled by a clock with a rate error. For tests and test signals.
#[derive(Clone, Debug)]
pub struct MskSignal {
    rate: f64,
    true_rate: f64,
    offset_hz: f64,
    baud: f64,
    amplitude: f64,
    first_boundary: f64,
    seed: Lfsr,
    reg: Lfsr,
    /// The symbol of input sample 0.
    k0: i64,
    /// The current symbol, its start phase and its sign.
    k: i64,
    phase: f64,
    bit: f64,
}

impl MskSignal {
    /// A signal for input at `rate` with the carrier at `offset_hz` from the tuned centre (true hertz), `baud` symbols a
    /// second and peak amplitude `amplitude`; `seed` picks the bits. Boundary 0 lies at true time 0 and the sample clock is
    /// exact until [`with_first_boundary`](Self::with_first_boundary) and [`with_clock_ppm`](Self::with_clock_ppm) change them.
    /// Errors when a value is not finite, `rate` or `baud` is not above 0, or the amplitude is negative.
    pub fn new(
        rate: f64,
        offset_hz: f64,
        baud: f64,
        amplitude: f64,
        seed: u64,
    ) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate)?;
        Param::Baud.check_positive(baud)?;
        Param::Frequency.check_finite(offset_hz)?;
        Param::Amplitude.check_in(amplitude, 0.0, f64::MAX)?;
        let reg = Lfsr::fibonacci(PRBS23, seed % 0x7F_FFFF + 1);
        let mut s = MskSignal {
            rate,
            true_rate: rate,
            offset_hz,
            baud,
            amplitude,
            first_boundary: 0.0,
            seed: reg,
            reg,
            k0: 0,
            k: 0,
            phase: 0.0,
            bit: 1.0,
        };
        s.restart();
        Ok(s)
    }

    /// The sample clock runs `ppm` parts per million fast (negative: slow): input sample `n` lies at true time
    /// `n / (rate (1 + ppm 1e-6))`. Errors outside `+-10000` ppm.
    pub fn with_clock_ppm(mut self, ppm: f64) -> Result<Self, ModemError> {
        Param::MaxPpm.check_in(ppm, -1e4, 1e4)?;
        self.true_rate = self.rate * (1.0 + ppm * 1e-6);
        self.restart();
        Ok(self)
    }

    /// Symbol boundary 0 lies at true time `seconds` (input sample 0 lies at true time 0). Errors when it is not finite.
    pub fn with_first_boundary(mut self, seconds: f64) -> Result<Self, ModemError> {
        Param::Start.check_finite(seconds)?;
        self.first_boundary = seconds;
        self.restart();
        Ok(self)
    }

    /// The true time of input sample `sample` (fractional), seconds.
    pub fn sample_time(&self, sample: f64) -> f64 {
        sample / self.true_rate
    }

    /// The true time of input sample `sample` minus the time of the nearest symbol boundary, seconds.
    pub fn boundary_error(&self, sample: f64) -> f64 {
        let u = (self.sample_time(sample) - self.first_boundary) * self.baud;
        (u - u.round()) / self.baud
    }

    /// Adds the signal at input samples `first ..` to `out`. Calls in increasing order of `first` are cheapest; an earlier
    /// `first` makes the bits start again from sample 0, so the same index always gets the same value.
    pub fn add(&mut self, first: u64, out: &mut [Complex32]) {
        for (i, v) in out.iter_mut().enumerate() {
            let t = (first + i as u64) as f64 / self.true_rate;
            let u = (t - self.first_boundary) * self.baud;
            let k = u.floor() as i64;
            if k < self.k {
                self.restart();
            }
            while self.k < k {
                self.phase = (self.phase + self.bit * FRAC_PI_2).rem_euclid(TAU);
                self.k += 1;
                self.bit = self.next_bit();
            }
            let carrier = TAU * (self.offset_hz * t).rem_euclid(1.0);
            let phase = self.phase + self.bit * FRAC_PI_2 * (u - k as f64) + carrier;
            let s = Complex64::from_polar(self.amplitude, phase);
            *v += Complex32::new(s.re as f32, s.im as f32);
        }
    }

    /// Back to the symbol of input sample 0 and the first bit.
    fn restart(&mut self) {
        self.k0 = (-self.first_boundary * self.baud).floor() as i64;
        self.k = self.k0;
        self.reg = self.seed;
        self.phase = 0.0;
        self.bit = self.next_bit();
    }

    fn next_bit(&mut self) -> f64 {
        if self.reg.bit() == 1 {
            1.0
        } else {
            -1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::noise::Awgn;

    const RATE: f64 = 250_000.0;
    const OFFSET: f64 = 40_000.0;
    const BAUD: f64 = 200.0;
    const PPM: f64 = -17.0;

    /// 16 s of the timing transmitter at amplitude `amp` with a carrier error `carrier_hz`, in noise of rms 0.02, through the
    /// timer; the readings and the signal.
    fn run(amp: f64, carrier_hz: f64, seed: u64) -> (Vec<MskReading>, MskSignal) {
        let mut sig = MskSignal::new(RATE, OFFSET + carrier_hz, BAUD, amp, seed)
            .and_then(|s| s.with_clock_ppm(PPM))
            .and_then(|s| s.with_first_boundary(0.001234))
            .unwrap();
        let mut noise = Awgn::with_power(0.02 * 0.02, seed + 100).unwrap();
        let mut timer = MskTimer::new(RATE, OFFSET, BAUD).unwrap();
        let total = 16 * RATE as u64;
        let mut buf = vec![Complex32::default(); 65_536];
        let mut out = Vec::new();
        let mut first = 0;
        while first < total {
            let n = ((total - first) as usize).min(buf.len());
            let b = &mut buf[..n];
            b.fill(Complex32::default());
            noise.process(b);
            sig.add(first, b);
            timer.process(first, b, &mut out);
            first += n as u64;
        }
        (out, sig)
    }

    /// One level: at least 6 locked readings for both carrier errors, with a stated 1-sigma that agrees with the errors; with
    /// `tight`, the mean timing error under 3 us, and the rate and the carrier measured; with `rms_us`, the rms of the timing
    /// errors under that limit.
    fn level(amp: f64, tight: bool, rms_us: Option<f64>) {
        for (carrier, seed) in [(0.0, 11), (1.2, 12)] {
            let (out, sig) = run(amp, carrier, seed);
            let locked: Vec<&MskReading> = out.iter().filter(|r| r.locked).collect();
            assert!(locked.len() >= 6, "amp {amp} carrier {carrier}: {} locked", locked.len());
            let errors: Vec<f64> = locked
                .iter()
                .map(|r| sig.boundary_error(r.boundary) * 1e6)
                .collect();
            let mean = errors.iter().sum::<f64>() / errors.len() as f64;
            let rms = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
            // The stated 1-sigma agrees with the errors.
            let sigma = locked.iter().map(|r| r.sigma_s).sum::<f64>() / locked.len() as f64 * 1e6;
            assert!(
                rms > 0.5 * sigma && rms < 1.6 * sigma,
                "amp {amp} carrier {carrier}: rms {rms} us, sigma {sigma} us"
            );
            if let Some(limit) = rms_us {
                assert!(rms < limit, "amp {amp} carrier {carrier}: rms {rms} us");
            }
            if tight {
                assert!(mean.abs() < 3.0, "amp {amp} carrier {carrier}: mean {mean} us");
                let last = out.last().unwrap();
                assert!((last.rate_ppm - PPM).abs() < 2.0, "rate {}", last.rate_ppm);
                let seen = (OFFSET + carrier) * RATE / (RATE * (1.0 + PPM * 1e-6)) - OFFSET;
                assert!(
                    (last.carrier_error_hz - seen).abs() < 0.2,
                    "carrier {} against {seen}",
                    last.carrier_error_hz
                );
            }
        }
    }

    #[test]
    fn timer_at_19_db() {
        level(0.005, false, None);
    }

    #[test]
    fn timer_at_31_db() {
        // The stated sigma here is about 4.5 us, so an rms limit of 4 us holds for some noise draws only; the sigma check
        // bounds the rms instead.
        level(0.02, true, None);
    }

    #[test]
    fn timer_at_45_db() {
        level(0.1, true, Some(4.0));
    }

    #[test]
    fn pieces_of_any_size_give_the_same_readings() {
        let rate = 48_000.0;
        let n = 6 * 48_000;
        let mut x = vec![Complex32::default(); n];
        Awgn::with_power(4e-4, 9).unwrap().process(&mut x);
        let mut sig = MskSignal::new(rate, 10_000.0, BAUD, 0.05, 5).unwrap();
        sig.add(0, &mut x);
        let start = 1_000_000u64;
        let mut whole = MskTimer::new(rate, 10_000.0, BAUD).unwrap();
        let mut a = Vec::new();
        whole.process(start, &x, &mut a);
        assert!(!a.is_empty());
        let mut pieces = MskTimer::new(rate, 10_000.0, BAUD).unwrap();
        let mut b = Vec::new();
        let mut i = 0;
        for size in [1usize, 7, 999, 4096, 33, 20_000].iter().cycle() {
            if i >= n {
                break;
            }
            let k = (*size).min(n - i);
            pieces.process(start + i as u64, &x[i..i + k], &mut b);
            i += k;
        }
        assert_eq!(a, b);
        // The boundary is in the index frame of `first`.
        let r = a[0];
        assert!(sig.boundary_error(r.boundary - start as f64).abs() < 20e-6);
        // A jump of the index restarts the grid.
        pieces.process(start + n as u64 + 5, &x[..1000], &mut b);
        assert_eq!(pieces.restarts(), 1);
        pieces.reset();
        assert_eq!(pieces.restarts(), 0);
        assert!(pieces.last().is_none());
    }

    #[test]
    fn settings_are_checked() {
        assert!(MskTimer::new(0.0, 0.0, 200.0).is_err());
        assert!(MskTimer::new(1000.0, 0.0, 400.0).is_err());
        assert!(MskTimer::new(48_000.0, 30_000.0, 200.0).is_err());
        assert!(MskTimer::new(48_000.0, f64::NAN, 200.0).is_err());
        let t = MskTimer::new(250_000.0, 40_000.0, 200.0).unwrap();
        assert_eq!(t.decimation(), 156);
        assert!((t.period_samples() - 1250.0).abs() < 1e-9);
        assert!(MskSignal::new(48_000.0, 0.0, 200.0, -1.0, 1).is_err());
        assert!(MskSignal::new(48_000.0, 0.0, 200.0, 1.0, 1)
            .unwrap()
            .with_clock_ppm(2e4)
            .is_err());
    }
}
