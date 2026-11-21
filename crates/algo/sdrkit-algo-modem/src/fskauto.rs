//! A 2-FSK front that needs nothing but a list of candidate bauds: the tone pair from the spectrum, the baud from the eye
//! opening (else from the line of the transition times), the clock phase, a refinement of the two tones, and an
//! integrate-and-dump slicer whose clock follows the signal slowly.
//!
//! The input is complex with the signal at positive frequencies (an audio tone of f Hz at +f Hz). Each search runs at a fixed
//! count of input samples, so the output does not depend on how the input is cut. The decisions are soft values in `[-1, 1]`:
//! `(|e1|^2 - |e0|^2) / (|e1|^2 + |e0|^2)` of the higher tone `e1` and the lower tone `e0` over one symbol, so a positive value
//! is a bit 1 when the higher tone means 1.
//!
//! | Step | What it does |
//! |---|---|
//! | Tone pair | Hann spectrum of the newest 1.024 s; the 12 strongest local maxima are paired; the first pair (in order of strength) with an accepted shift, the weaker peak at least 1.5 % of the stronger and 12 times the mean bin power, wins |
//! | Baud | Eye measure over the newest 1 to 4 s: for each candidate, sliding one-symbol sums of the two mixed signals, `|d| = ||S1|^2 - |S0|^2|` sampled at 12 clock phases; the eye is the largest phase mean over the smallest; at least 1.6, and the lowest candidate within 0.85 of the best eye wins (a multiple of the rate also opens an eye). Fallback (and the only test for a known baud): the line of the transition times at each candidate |
//! | Clock | 12 phases a symbol over the newest 160 symbols, the sum of `||e1|^2 - |e0|^2|` |
//! | Tones | once a lock: both tones moved by -36 to +36 Hz in 6 Hz steps for the largest clock metric; the clock phase is found again |
//! | Slicer | from the oldest kept sample; at each transition, the window centred on the symbol edge moves the clock by 2 % of its imbalance |
//!
//! ```
//! use sdrkit_algo_modem::fskauto::{BinaryFskConfig, BinaryFskFront};
//! use sdrkit_algo_modem::Complex32;
//! let rate = 8000.0;
//! let mut front = BinaryFskFront::new(BinaryFskConfig::new(rate, &[50.0, 100.0])).unwrap();
//! let (mut ph, mut x, mut s) = (0.0f64, Vec::new(), 1u32);
//! for _ in 0..800 {
//!     s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
//!     let f = if s >> 16 & 1 == 1 { 1700.0 } else { 1300.0 };
//!     for _ in 0..80 {
//!         ph += std::f64::consts::TAU * f / rate;
//!         x.push(Complex32::from_polar(1.0, ph as f32));
//!     }
//! }
//! let mut soft = Vec::new();
//! front.process(&x, &mut soft);
//! let lock = front.lock().unwrap();
//! assert_eq!(lock.baud, 100.0);
//! assert!((lock.f1 - lock.f0 - 400.0).abs() < 8.0);
//! assert!(soft.len() > 500);
//! ```

use crate::error::{ModemError, Param};
use crate::Complex32;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use std::f64::consts::TAU;

/// The settings of a [`BinaryFskFront`].
#[derive(Clone, Debug)]
pub struct BinaryFskConfig {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The candidate bauds.
    pub bauds: Vec<f64>,
    /// A known baud: the eye measure is skipped and the transition test runs with this rate only.
    pub baud: Option<f64>,
    /// A known shift, Hz: a pair is taken only within [`BinaryFskConfig::shift_tol_hz`] of it.
    pub shift_hz: Option<f64>,
    /// The tolerance of a known shift, Hz.
    pub shift_tol_hz: f64,
    /// The band both tones lie in, Hz.
    pub band_hz: (f64, f64),
    /// The shifts accepted, Hz.
    pub shifts_hz: (f64, f64),
}

impl BinaryFskConfig {
    /// A front at `rate` for the candidates `bauds`: tones in 100 to 3900 Hz, shifts of 60 to 1100 Hz.
    pub fn new(rate: f64, bauds: &[f64]) -> Self {
        BinaryFskConfig {
            rate,
            bauds: bauds.to_vec(),
            baud: None,
            shift_hz: None,
            shift_tol_hz: 40.0,
            band_hz: (100.0, 3900.0),
            shifts_hz: (60.0, 1100.0),
        }
    }
}

/// What a front has locked to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FskLock {
    /// The lower tone, Hz.
    pub f0: f64,
    /// The higher tone, Hz.
    pub f1: f64,
    /// The baud.
    pub baud: f64,
}

impl FskLock {
    /// The shift, Hz.
    pub fn shift(&self) -> f64 {
        self.f1 - self.f0
    }
    /// The centre, Hz.
    pub fn centre(&self) -> f64 {
        0.5 * (self.f0 + self.f1)
    }
}

#[derive(Clone, Copy, Debug)]
enum State {
    Search,
    Tones { f0: f64, f1: f64, since: u64 },
    Locked { lock: FskLock, t: f64, prev: f32 },
}

/// The part of the search that a front is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FskStage {
    /// Looking for a tone pair.
    Searching,
    /// A tone pair, looking for the baud.
    Tones,
    /// Deciding bits.
    Locked,
}

/// The 2-FSK front. See the module documentation.
pub struct BinaryFskFront {
    cfg: BinaryFskConfig,
    nfft: usize,
    hop: u64,
    keep: usize,
    max_eye: usize,
    give_up: u64,
    fft: WindowedFft,
    hist: Vec<Complex32>,
    base: u64,
    n: u64,
    state: State,
    fresh: bool,
    m0: Vec<Complex32>,
    m1: Vec<Complex32>,
    d: Vec<f32>,
}

impl BinaryFskFront {
    /// A front with the settings `cfg`. Errors for a rate below 2 kHz or above 192 kHz, no candidate baud, or a baud that is not
    /// positive or above a quarter of the rate.
    pub fn new(cfg: BinaryFskConfig) -> Result<Self, ModemError> {
        if !(2000.0..=192_000.0).contains(&cfg.rate) {
            return Err(ModemError::OutOfRange {
                param: Param::Rate,
                min: 2000.0,
                max: 192_000.0,
                got: cfg.rate,
            });
        }
        let bauds: Vec<f64> = cfg
            .baud
            .into_iter()
            .chain(cfg.bauds.iter().copied())
            .collect();
        if bauds.is_empty() {
            return Err(ModemError::Length {
                param: Param::Baud,
                expected: 1,
                got: 0,
            });
        }
        for &b in &bauds {
            if !(b > 0.0 && b <= cfg.rate / 4.0) {
                return Err(ModemError::OutOfRange {
                    param: Param::Baud,
                    min: 0.0,
                    max: cfg.rate / 4.0,
                    got: b,
                });
            }
        }
        let nfft = (cfg.rate * 1.024).round().max(64.0) as usize;
        let nfft = nfft.next_power_of_two();
        let fft =
            WindowedFft::new(nfft, nfft, Window::Hann).map_err(|_| ModemError::OutOfRange {
                param: Param::FftSize,
                min: 64.0,
                max: 1e6,
                got: nfft as f64,
            })?;
        let max_eye = (cfg.rate * 4.096) as usize;
        Ok(BinaryFskFront {
            hop: (cfg.rate * 0.25) as u64,
            keep: max_eye,
            max_eye,
            give_up: (cfg.rate * 12.0) as u64,
            nfft,
            fft,
            cfg,
            hist: Vec::new(),
            base: 0,
            n: 0,
            state: State::Search,
            fresh: false,
            m0: Vec::new(),
            m1: Vec::new(),
            d: Vec::new(),
        })
    }

    /// Forgets the signal and the kept samples.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.base = self.n;
        self.state = State::Search;
        self.fresh = false;
    }

    /// Searches again from the tone pair, with the kept samples.
    pub fn restart(&mut self) {
        self.state = State::Search;
        self.fresh = false;
    }

    /// The lock, if any.
    pub fn lock(&self) -> Option<FskLock> {
        match self.state {
            State::Locked { lock, .. } => Some(lock),
            _ => None,
        }
    }

    /// The tone pair found while the baud is searched, or the locked one.
    pub fn tones(&self) -> Option<(f64, f64)> {
        match self.state {
            State::Search => None,
            State::Tones { f0, f1, .. } => Some((f0, f1)),
            State::Locked { lock, .. } => Some((lock.f0, lock.f1)),
        }
    }

    /// The stage.
    pub fn stage(&self) -> FskStage {
        match self.state {
            State::Search => FskStage::Searching,
            State::Tones { .. } => FskStage::Tones,
            State::Locked { .. } => FskStage::Locked,
        }
    }

    /// Whether a new lock was taken since the last call. The decisions of the kept samples follow a new lock at once.
    pub fn take_new_lock(&mut self) -> bool {
        std::mem::take(&mut self.fresh)
    }

    /// Takes `x` and appends a soft decision per symbol to `soft`.
    pub fn process(&mut self, x: &[Complex32], soft: &mut Vec<f32>) {
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.hist.push(v);
            self.n += 1;
            if self.hist.len() >= 2 * self.keep {
                let cut = self.hist.len() - self.keep;
                self.hist.drain(..cut);
                self.base += cut as u64;
            }
            if self.n.is_multiple_of(self.hop) {
                self.step();
            }
            self.decide(soft);
        }
    }

    fn step(&mut self) {
        match self.state {
            State::Search => {
                if let Some((f0, f1)) = self.find_pair() {
                    self.state = State::Tones {
                        f0,
                        f1,
                        since: self.n,
                    };
                }
            }
            State::Tones { f0, f1, since } => {
                if self.hist.len() < self.nfft {
                    return;
                }
                let baud = match self.cfg.baud {
                    Some(b) => self.transition_baud(f0, f1, &[b]),
                    None => self
                        .eye_baud(f0, f1)
                        .or_else(|| self.transition_baud(f0, f1, &self.cfg.bauds.clone())),
                };
                match baud {
                    Some(baud) => self.take_lock(f0, f1, baud),
                    None if self.n - since > self.give_up => self.state = State::Search,
                    None => {}
                }
            }
            State::Locked { .. } => {}
        }
    }

    /// The tone pair of the newest spectrum.
    fn find_pair(&mut self) -> Option<(f64, f64)> {
        if self.hist.len() < self.nfft {
            return None;
        }
        let n = self.nfft;
        let p = self.fft.run_power(&self.hist[self.hist.len() - n..]);
        let bin = self.cfg.rate / n as f64;
        let lo = ((self.cfg.band_hz.0 / bin).ceil() as usize).max(1);
        let hi = ((self.cfg.band_hz.1 / bin).floor() as usize).min(n / 2 - 2);
        if hi <= lo + 2 {
            return None;
        }
        let mean = p[lo..=hi].iter().map(|&v| f64::from(v)).sum::<f64>() / (hi - lo + 1) as f64;
        let mut peaks: Vec<(usize, f64)> = (lo..=hi)
            .filter(|&k| p[k] > p[k - 1] && p[k] >= p[k + 1])
            .map(|k| (k, f64::from(p[k])))
            .collect();
        peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
        peaks.truncate(12);
        for i in 0..peaks.len() {
            for j in (i + 1)..peaks.len() {
                let (a, b) = (peaks[i], peaks[j]);
                let shift = (a.0 as f64 - b.0 as f64).abs() * bin;
                if shift < self.cfg.shifts_hz.0 || shift > self.cfg.shifts_hz.1 {
                    continue;
                }
                if self
                    .cfg
                    .shift_hz
                    .is_some_and(|s| (shift - s).abs() > self.cfg.shift_tol_hz)
                {
                    continue;
                }
                if b.1 < 0.015 * a.1 || b.1 < 12.0 * mean {
                    continue;
                }
                let (fa, fb) = (a.0 as f64 * bin, b.0 as f64 * bin);
                return Some((fa.min(fb), fa.max(fb)));
            }
        }
        None
    }

    /// Mixes the newest `len` kept samples down by `f0` and `f1` into `m0` and `m1`.
    fn mix(&mut self, f0: f64, f1: f64, len: usize) {
        let start = self.hist.len() - len;
        let a = self.base + start as u64;
        for (m, f) in [(&mut self.m0, f0), (&mut self.m1, f1)] {
            m.clear();
            let w = -TAU * f / self.cfg.rate;
            let ph0 = (w * a as f64).rem_euclid(TAU);
            let mut rot = Complex32::from_polar(1.0, ph0 as f32);
            let step = Complex32::from_polar(1.0, w as f32);
            for (i, &x) in self.hist[start..].iter().enumerate() {
                m.push(x * rot);
                rot *= step;
                if i % 1024 == 1023 {
                    let ph = (w * (a + i as u64 + 1) as f64).rem_euclid(TAU);
                    rot = Complex32::from_polar(1.0, ph as f32);
                }
            }
        }
    }

    /// Fills `d` with `||S1|^2 - |S0|^2|` (absolute when `abs`) of sliding sums over `l` samples of the mixed signals; `d[i]` is
    /// the sum that ends at sample `i` (0 before the first full window).
    fn slide(&mut self, l: usize, abs: bool) {
        let n = self.m0.len();
        self.d.clear();
        self.d.resize(n, 0.0);
        let (mut s0, mut s1) = (Complex32::default(), Complex32::default());
        for i in 0..n {
            s0 += self.m0[i];
            s1 += self.m1[i];
            if i >= l {
                s0 -= self.m0[i - l];
                s1 -= self.m1[i - l];
            }
            if i + 1 >= l {
                let v = s1.norm_sqr() - s0.norm_sqr();
                self.d[i] = if abs { v.abs() } else { v };
            }
        }
    }

    /// The baud from the eye opening, if one opens.
    fn eye_baud(&mut self, f0: f64, f1: f64) -> Option<f64> {
        let len = self.hist.len().min(self.max_eye);
        self.mix(f0, f1, len);
        let rate = self.cfg.rate;
        let mut eyes: Vec<(f64, f64)> = Vec::new();
        for &b in &self.cfg.bauds.clone() {
            let sps = rate / b;
            if (len as f64) / sps < 40.0 {
                continue;
            }
            let l = (sps.round() as usize).max(2);
            self.slide(l, true);
            let mut means = [0.0f64; 12];
            for (p, m) in means.iter_mut().enumerate() {
                let mut pos = (l - 1) as f64 + p as f64 * sps / 12.0;
                let (mut s, mut k) = (0.0f64, 0u32);
                while (pos.round() as usize) < len {
                    s += f64::from(self.d[pos.round() as usize]);
                    k += 1;
                    pos += sps;
                }
                *m = s / f64::from(k.max(1));
            }
            let hi = means.iter().copied().fold(0.0, f64::max);
            let lo = means.iter().copied().fold(f64::MAX, f64::min);
            if hi > 0.0 {
                eyes.push((b, hi / lo.max(hi * 1e-6)));
            }
        }
        let best = eyes.iter().map(|e| e.1).fold(0.0, f64::max);
        if best < 1.6 {
            return None;
        }
        eyes.iter()
            .filter(|e| e.1 >= 0.85 * best)
            .map(|e| e.0)
            .min_by(f64::total_cmp)
    }

    /// The baud from the line of the transition times at each of `bauds`, if one stands out.
    fn transition_baud(&mut self, f0: f64, f1: f64, bauds: &[f64]) -> Option<f64> {
        let len = self.hist.len().min(self.max_eye);
        self.mix(f0, f1, len);
        self.slide(16, false);
        let mean = self.d.iter().map(|v| f64::from(v.abs())).sum::<f64>() / len.max(1) as f64;
        let th = (0.6 * mean) as f32;
        let mut state = 0i8;
        let mut times = Vec::new();
        for (i, &v) in self.d.iter().enumerate() {
            let s = if v > th {
                1
            } else if v < -th {
                -1
            } else {
                state
            };
            if s != state && state != 0 {
                times.push(i);
            }
            state = s;
        }
        if times.len() < 20 {
            return None;
        }
        let mags: Vec<(f64, f64)> = bauds
            .iter()
            .map(|&b| {
                let w = TAU * b / self.cfg.rate;
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for &t in &times {
                    re += (w * t as f64).cos();
                    im -= (w * t as f64).sin();
                }
                (b, re.hypot(im))
            })
            .collect();
        let best = mags.iter().map(|m| m.1).fold(0.0, f64::max);
        if best < 0.25 * times.len() as f64 {
            return None;
        }
        mags.iter()
            .filter(|m| m.1 >= 0.7 * best)
            .map(|m| m.0)
            .min_by(f64::total_cmp)
    }

    /// The complex sum of the kept samples `[a, a + len)` (absolute indices) mixed down by `f`.
    fn tone_sum(&self, f: f64, a: u64, len: usize) -> Complex32 {
        let w = -TAU * f / self.cfg.rate;
        let mut rot = Complex32::from_polar(1.0, (w * a as f64).rem_euclid(TAU) as f32);
        let step = Complex32::from_polar(1.0, w as f32);
        let s = (a - self.base) as usize;
        let mut acc = Complex32::default();
        for &x in &self.hist[s..s + len] {
            acc += x * rot;
            rot *= step;
        }
        acc
    }

    /// The window `[t, t + sps)` as absolute start and length.
    fn window(t: f64, sps: f64) -> (u64, usize) {
        let a = t.round().max(0.0) as u64;
        let b = (t + sps).round().max(0.0) as u64;
        (a, (b - a).max(1) as usize)
    }

    /// The clock metric of `nsym` symbols from `t0` with tones `f0`, `f1`.
    fn metric(&self, f0: f64, f1: f64, sps: f64, t0: f64, nsym: usize) -> f64 {
        (0..nsym)
            .map(|k| {
                let (a, l) = Self::window(t0 + k as f64 * sps, sps);
                let e0 = self.tone_sum(f0, a, l).norm_sqr();
                let e1 = self.tone_sum(f1, a, l).norm_sqr();
                f64::from((e1 - e0).abs())
            })
            .sum()
    }

    /// The start of the best of 12 clock phases over the newest symbols (at most 160), and the number of symbols.
    fn clock_phase(&self, f0: f64, f1: f64, sps: f64) -> (f64, usize) {
        let end = self.n as f64 - 1.0;
        let nsym = (((self.hist.len() as f64 - 2.0) / sps).floor() as usize)
            .saturating_sub(1)
            .min(160);
        let anchor = end - (nsym as f64 + 1.0) * sps;
        let best = (0..12)
            .map(|p| {
                let t = anchor + p as f64 * sps / 12.0;
                (t, self.metric(f0, f1, sps, t, nsym))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(anchor, |b| b.0);
        (best, nsym)
    }

    fn take_lock(&mut self, f0: f64, f1: f64, baud: f64) {
        let sps = self.cfg.rate / baud;
        let (t, nsym) = self.clock_phase(f0, f1, sps);
        // The tones: 13 offsets each, the window sums of every symbol kept so the 169 pairs cost no more sums.
        let offs: Vec<f64> = (0..13).map(|i| -36.0 + 6.0 * i as f64).collect();
        let energies = |f: f64| -> Vec<Vec<f32>> {
            offs.iter()
                .map(|&o| {
                    (0..nsym)
                        .map(|k| {
                            let (a, l) = Self::window(t + k as f64 * sps, sps);
                            self.tone_sum(f + o, a, l).norm_sqr()
                        })
                        .collect()
                })
                .collect()
        };
        let (e0, e1) = (energies(f0), energies(f1));
        let mut best = (0usize, 0usize, f64::MIN);
        for i in 0..13 {
            for j in 0..13 {
                if f1 + offs[j] - (f0 + offs[i]) < 20.0 {
                    continue;
                }
                let m: f64 = (0..nsym)
                    .map(|k| f64::from((e1[j][k] - e0[i][k]).abs()))
                    .sum();
                if m > best.2 {
                    best = (i, j, m);
                }
            }
        }
        let (f0, f1) = (f0 + offs[best.0], f1 + offs[best.1]);
        let (t, _) = self.clock_phase(f0, f1, sps);
        // Back to the oldest symbol that is at least three quarters kept.
        let first = self.base as f64 - 0.25 * sps;
        let k = ((t - first) / sps).floor().max(0.0);
        self.state = State::Locked {
            lock: FskLock { f0, f1, baud },
            t: t - k * sps,
            prev: 0.0,
        };
        self.fresh = true;
    }

    /// Decides every symbol whose window is in.
    fn decide(&mut self, soft: &mut Vec<f32>) {
        let State::Locked {
            lock,
            mut t,
            mut prev,
        } = self.state
        else {
            return;
        };
        let sps = self.cfg.rate / lock.baud;
        while t + sps + 1.0 <= self.n as f64 {
            if t < self.base as f64 - 0.25 * sps {
                t += sps;
                continue;
            }
            let (a, l) = Self::window(t, sps);
            let (a, l) = if a < self.base {
                (self.base, l - (self.base - a) as usize)
            } else {
                (a, l)
            };
            let e0 = self.tone_sum(lock.f0, a, l).norm_sqr();
            let e1 = self.tone_sum(lock.f1, a, l).norm_sqr();
            let s = (e1 - e0) / (e1 + e0).max(1e-30);
            soft.push(s);
            let mut next = t + sps;
            if s * prev < 0.0
                && s.abs() > 0.3
                && prev.abs() > 0.3
                && t - 0.5 * sps >= self.base as f64
            {
                let (am, lm) = Self::window(t - 0.5 * sps, sps);
                let m0 = self.tone_sum(lock.f0, am, lm).norm_sqr();
                let m1 = self.tone_sum(lock.f1, am, lm).norm_sqr();
                let dm = (m1 - m0) / (m1 + m0).max(1e-30);
                next -= 0.02 * f64::from(dm * s.signum()) * sps;
            }
            prev = s;
            t = next;
        }
        self.state = State::Locked { lock, t, prev };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fsk(
        rate: f64,
        baud: f64,
        f0: f64,
        f1: f64,
        nbits: usize,
        seed: u32,
        ppm: f64,
    ) -> (Vec<Complex32>, Vec<u8>) {
        let mut s = seed;
        let mut ph = 0.0f64;
        let mut x = Vec::new();
        let mut bits = Vec::new();
        let sps = rate / baud * (1.0 + ppm * 1e-6);
        let mut t = 0.0;
        for k in 0..nbits {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let b = (s >> 16 & 1) as u8;
            bits.push(b);
            let f = if b == 1 { f1 } else { f0 };
            let end = (k + 1) as f64 * sps;
            while t < end {
                ph += TAU * f / rate;
                x.push(Complex32::from_polar(1.0, ph as f32));
                t += 1.0;
            }
        }
        (x, bits)
    }

    fn run(x: &[Complex32], cfg: BinaryFskConfig, piece: usize) -> (Option<FskLock>, Vec<u8>) {
        let mut f = BinaryFskFront::new(cfg).unwrap();
        let mut soft = Vec::new();
        for c in x.chunks(piece) {
            f.process(c, &mut soft);
        }
        (f.lock(), soft.iter().map(|&s| u8::from(s > 0.0)).collect())
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn finds_the_baud_among_multiples_and_reads_the_bits() {
        for &(baud, f0, f1) in &[
            (96.0, 1300.0, 1700.0),
            (184.6, 1100.0, 1500.0),
            (48.0, 1415.0, 1585.0),
        ] {
            let (x, bits) = fsk(8000.0, baud, f0, f1, 1200, 7, 0.0);
            let cfg = BinaryFskConfig::new(
                8000.0,
                &[
                    46.2, 48.0, 50.0, 64.0, 72.0, 86.0, 96.0, 144.0, 184.6, 192.0, 288.0,
                ],
            );
            let (lock, got) = run(&x, cfg, 1 << 20);
            let lock = lock.expect("a lock");
            assert_eq!(lock.baud, baud);
            assert!(
                (lock.f0 - f0).abs() < 8.0 && (lock.f1 - f1).abs() < 8.0,
                "{lock:?}"
            );
            assert!(contains(&got, &bits[400..1100]), "{baud}");
        }
    }

    #[test]
    fn the_cut_does_not_matter_and_the_clock_follows() {
        let (x, bits) = fsk(8000.0, 100.0, 1000.0, 1200.0, 3000, 3, 60.0);
        let cfg = || BinaryFskConfig::new(8000.0, &[50.0, 100.0, 200.0]);
        let (_, a) = run(&x, cfg(), 1 << 20);
        let (_, b) = run(&x, cfg(), 77);
        assert_eq!(a, b);
        assert!(contains(&a, &bits[2500..2990]));
    }

    #[test]
    fn a_known_baud_and_shift() {
        let (x, bits) = fsk(8000.0, 200.0, 1330.0, 1670.0, 1500, 9, 0.0);
        let mut cfg = BinaryFskConfig::new(8000.0, &[]);
        cfg.baud = Some(200.0);
        cfg.shift_hz = Some(340.0);
        let (lock, got) = run(&x, cfg, 4096);
        assert_eq!(lock.unwrap().baud, 200.0);
        assert!(contains(&got, &bits[600..1450]));
    }

    #[test]
    fn noise_does_not_lock() {
        let mut s = 1u32;
        let x: Vec<Complex32> = (0..80_000)
            .map(|_| {
                let mut g = || {
                    s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                    (s >> 8) as f32 / 16_777_216.0 - 0.5
                };
                Complex32::new(g() + g() + g(), g() + g() + g())
            })
            .collect();
        let (lock, _) = run(&x, BinaryFskConfig::new(8000.0, &[50.0, 100.0]), 1 << 20);
        assert!(lock.is_none());
    }
}
