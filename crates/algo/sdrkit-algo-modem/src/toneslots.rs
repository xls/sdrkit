//! The front of an MFSK signal that sends one tone at a time out of a few evenly spaced tones in slots of a fixed length: the
//! base tone from the spectrum, the slot length from a list of candidates and its phase, then the strongest tone of every slot.
//!
//! | Step | What it does |
//! |---|---|
//! | Base | Hann spectrum of the newest 1.024 s, each bin with its two neighbours; the base `f` of the largest sum at `f + k s` (`k` the tones, `s` the spacing), in 1 Hz steps, at least 20 times the mean. A signal that idles on two adjacent tones can name which they are, and the base moves to match: the idle shows as the strongest line midway between the two tones (a 2-FSK of index `s T`), or as exactly two strong adjacent tones |
//! | Slots | For each candidate length, 8 phases over the newest 70 slots: the mean of the strongest tone's energy over the energy of all tones. A candidate replaces the best when it is 1.05 times better, or 0.9 times as good with a longer slot (half a slot also scores well). At least 0.55 locks |
//! | Decisions | from the oldest kept sample, the strongest tone of each slot and its share of the energy |
//!
//! The tone sums come from running sums of each mixed tone, so a window costs the same at any length. Each search runs at a fixed
//! count of input samples, so the output does not depend on how the input is cut.
//!
//! ```
//! use sdrkit_algo_modem::toneslots::{ToneSlotConfig, ToneSlotFront};
//! use sdrkit_algo_modem::Complex32;
//! let rate = 8000.0;
//! let cfg = ToneSlotConfig::new(rate, 6, 20.0, &[0.05]);
//! let mut front = ToneSlotFront::new(cfg).unwrap();
//! let (mut ph, mut x) = (0.0f64, Vec::new());
//! for k in 0..300 {
//!     let f = 700.0 + 20.0 * ((k * 7 % 6) as f64);
//!     for _ in 0..400 {
//!         ph += std::f64::consts::TAU * f / rate;
//!         x.push(Complex32::from_polar(1.0, ph as f32));
//!     }
//! }
//! let mut slots = Vec::new();
//! front.process(&x, &mut slots);
//! let lock = front.lock().unwrap();
//! assert!((lock.base_hz - 700.0).abs() < 2.0);
//! assert!(slots.len() > 250);
//! ```

use crate::error::{ModemError, Param};
use crate::Complex32;
use num_complex::Complex64;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use std::f64::consts::TAU;

/// The settings of a [`ToneSlotFront`].
#[derive(Clone, Debug)]
pub struct ToneSlotConfig {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The number of tones (2 to 32).
    pub tones: usize,
    /// The tone spacing, Hz.
    pub spacing_hz: f64,
    /// The band all tones lie in, Hz.
    pub band_hz: (f64, f64),
    /// The candidate slot lengths, seconds.
    pub slots_s: Vec<f64>,
    /// The index of the first of two adjacent tones that the signal sends alone while it idles.
    pub idle_pair: Option<usize>,
    /// The smallest mean share of the strongest tone that locks.
    pub min_share: f64,
}

impl ToneSlotConfig {
    /// `tones` tones `spacing_hz` apart in 300 to 3000 Hz at `rate`, slots of the candidate lengths `slots_s`.
    pub fn new(rate: f64, tones: usize, spacing_hz: f64, slots_s: &[f64]) -> Self {
        ToneSlotConfig {
            rate,
            tones,
            spacing_hz,
            band_hz: (300.0, 3000.0),
            slots_s: slots_s.to_vec(),
            idle_pair: None,
            min_share: 0.55,
        }
    }
}

/// What a front has locked to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlotLock {
    /// The lowest tone, Hz.
    pub base_hz: f64,
    /// The slot length, seconds.
    pub slot_s: f64,
    /// The mean share of the strongest tone at the lock.
    pub share: f64,
}

/// The decision of one slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// The strongest tone, 0 the lowest.
    pub tone: u8,
    /// Its share of the energy of all tones.
    pub share: f32,
}

#[derive(Clone, Copy, Debug)]
enum State {
    Search,
    Based { base: f64, since: u64 },
    Locked { lock: SlotLock, t: f64 },
}

/// The tone-slot front. See the module documentation.
pub struct ToneSlotFront {
    cfg: ToneSlotConfig,
    nfft: usize,
    hop: u64,
    keep: usize,
    give_up: u64,
    fft: WindowedFft,
    hist: Vec<Complex32>,
    base_n: u64,
    n: u64,
    state: State,
    fresh: bool,
    /// Per tone: the running sum of the mixed tone; `sums[k][i]` is the sum of the kept samples before `i`.
    sums: Vec<Vec<Complex64>>,
    rot: Vec<(Complex64, Complex64)>,
    smooth: Vec<f64>,
    energies: Vec<f64>,
}

impl ToneSlotFront {
    /// A front with the settings `cfg`. Errors for a rate outside 2 to 192 kHz, fewer than 2 or more than 32 tones, a spacing
    /// that is not positive, or no slot length (each 2 ms to 2 s).
    pub fn new(cfg: ToneSlotConfig) -> Result<Self, ModemError> {
        if !(2000.0..=192_000.0).contains(&cfg.rate) {
            return Err(ModemError::OutOfRange {
                param: Param::Rate,
                min: 2000.0,
                max: 192_000.0,
                got: cfg.rate,
            });
        }
        if !(2..=32).contains(&cfg.tones) {
            return Err(ModemError::OutOfRange {
                param: Param::Tones,
                min: 2.0,
                max: 32.0,
                got: cfg.tones as f64,
            });
        }
        if cfg.spacing_hz.is_nan() || cfg.spacing_hz <= 0.0 {
            return Err(ModemError::OutOfRange {
                param: Param::Spacing,
                min: 0.0,
                max: cfg.rate / 2.0,
                got: cfg.spacing_hz,
            });
        }
        if cfg.slots_s.is_empty() || cfg.slots_s.iter().any(|s| !(0.002..=2.0).contains(s)) {
            return Err(ModemError::OutOfRange {
                param: Param::SymbolLen,
                min: 0.002,
                max: 2.0,
                got: cfg.slots_s.first().copied().unwrap_or(0.0),
            });
        }
        let nfft = ((cfg.rate * 1.024).round() as usize).next_power_of_two();
        let fft =
            WindowedFft::new(nfft, nfft, Window::Hann).map_err(|_| ModemError::OutOfRange {
                param: Param::FftSize,
                min: 64.0,
                max: 1e6,
                got: nfft as f64,
            })?;
        let longest = cfg.slots_s.iter().copied().fold(0.0, f64::max);
        let keep = ((cfg.rate * longest * 82.0) as usize).max(nfft);
        Ok(ToneSlotFront {
            hop: (cfg.rate * 0.25) as u64,
            give_up: (cfg.rate * 12.0) as u64,
            keep,
            nfft,
            fft,
            sums: vec![Vec::new(); cfg.tones],
            rot: Vec::new(),
            cfg,
            hist: Vec::new(),
            base_n: 0,
            n: 0,
            state: State::Search,
            fresh: false,
            smooth: Vec::new(),
            energies: Vec::new(),
        })
    }

    /// Forgets the signal and the kept samples.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.base_n = self.n;
        self.state = State::Search;
        self.fresh = false;
        self.rot.clear();
        self.sums.iter_mut().for_each(Vec::clear);
    }

    /// The lock, if any.
    pub fn lock(&self) -> Option<SlotLock> {
        match self.state {
            State::Locked { lock, .. } => Some(lock),
            _ => None,
        }
    }

    /// The base tone found while the slots are searched, or the locked one.
    pub fn base_hz(&self) -> Option<f64> {
        match self.state {
            State::Search => None,
            State::Based { base, .. } => Some(base),
            State::Locked { lock, .. } => Some(lock.base_hz),
        }
    }

    /// Whether a new lock was taken since the last call. The decisions of the kept samples follow a new lock at once.
    pub fn take_new_lock(&mut self) -> bool {
        std::mem::take(&mut self.fresh)
    }

    /// Takes `x` and appends the decision of every slot to `slots`.
    pub fn process(&mut self, x: &[Complex32], slots: &mut Vec<Slot>) {
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.hist.push(v);
            self.n += 1;
            self.extend_sums();
            if self.hist.len() >= 2 * self.keep {
                let cut = self.hist.len() - self.keep;
                self.hist.drain(..cut);
                for s in &mut self.sums {
                    if s.len() > cut {
                        s.drain(..cut);
                    }
                }
                self.base_n += cut as u64;
            }
            if self.n.is_multiple_of(self.hop) {
                self.step();
            }
            self.decide(slots);
        }
    }

    /// Starts the running sums of the tones from `base` over the kept samples.
    fn start_sums(&mut self, base: f64) {
        self.rot.clear();
        for k in 0..self.cfg.tones {
            let w = -TAU * (base + k as f64 * self.cfg.spacing_hz) / self.cfg.rate;
            let r0 = Complex64::from_polar(1.0, (w * self.base_n as f64).rem_euclid(TAU));
            self.rot.push((r0, Complex64::from_polar(1.0, w)));
            self.sums[k].clear();
            self.sums[k].push(Complex64::default());
        }
        let n = self.hist.len();
        for k in 0..self.cfg.tones {
            let (mut r, st) = self.rot[k];
            let mut acc = Complex64::default();
            for i in 0..n {
                let x = self.hist[i];
                acc += Complex64::new(f64::from(x.re), f64::from(x.im)) * r;
                r *= st;
                if i % 4096 == 4095 {
                    r /= r.norm();
                }
                self.sums[k].push(acc);
            }
            self.rot[k].0 = r;
        }
    }

    /// Adds the newest sample to the running sums.
    fn extend_sums(&mut self) {
        if self.rot.is_empty() {
            return;
        }
        let Some(&x) = self.hist.last() else { return };
        let x = Complex64::new(f64::from(x.re), f64::from(x.im));
        for k in 0..self.cfg.tones {
            let (r, st) = self.rot[k];
            let last = self.sums[k].last().copied().unwrap_or_default();
            self.sums[k].push(last + x * r);
            let mut r = r * st;
            if self.n.is_multiple_of(4096) {
                r /= r.norm();
            }
            self.rot[k].0 = r;
        }
    }

    /// The energy of tone `k` over the kept samples `[a, b)` (absolute indices).
    fn energy(&self, k: usize, a: u64, b: u64) -> f64 {
        let s = &self.sums[k];
        let ia = (a - self.base_n) as usize;
        let ib = (b - self.base_n) as usize;
        (s[ib] - s[ia]).norm_sqr()
    }

    /// The strongest tone of the slot `[t, t + l)` and its share.
    fn slot(&mut self, t: f64, l: f64) -> Slot {
        let a = t.round() as u64;
        let b = (t + l).round() as u64;
        self.energies.clear();
        for k in 0..self.cfg.tones {
            let e = self.energy(k, a, b);
            self.energies.push(e);
        }
        let tot: f64 = self.energies.iter().sum();
        let (tone, e) = self
            .energies
            .iter()
            .enumerate()
            .fold((0, -1.0), |m, (i, &e)| if e > m.1 { (i, e) } else { m });
        Slot {
            tone: tone as u8,
            share: (e / tot.max(1e-30)) as f32,
        }
    }

    fn step(&mut self) {
        match self.state {
            State::Search => {
                if let Some(base) = self.find_base() {
                    self.start_sums(base);
                    self.state = State::Based {
                        base,
                        since: self.n,
                    };
                }
            }
            State::Based { base, since } => {
                if let Some((slot_s, t, share)) = self.find_slots() {
                    let l = slot_s * self.cfg.rate;
                    let k = ((t - self.base_n as f64) / l).floor().max(0.0);
                    self.state = State::Locked {
                        lock: SlotLock {
                            base_hz: base,
                            slot_s,
                            share,
                        },
                        t: t - k * l,
                    };
                    self.fresh = true;
                } else if self.n - since > self.give_up {
                    self.state = State::Search;
                    self.rot.clear();
                }
            }
            State::Locked { .. } => {}
        }
    }

    /// The base tone of the newest spectrum, corrected for an idle pair.
    fn find_base(&mut self) -> Option<f64> {
        if self.hist.len() < self.nfft {
            return None;
        }
        let n = self.nfft;
        let bin = self.cfg.rate / n as f64;
        let p = self.fft.run_power(&self.hist[self.hist.len() - n..]);
        self.smooth.clear();
        self.smooth.push(0.0);
        for k in 1..n / 2 - 1 {
            self.smooth
                .push(f64::from(p[k - 1]) + f64::from(p[k]) + f64::from(p[k + 1]));
        }
        let span = self.cfg.tones as f64 * self.cfg.spacing_hz;
        let (lo, hi) = (self.cfg.band_hz.0, self.cfg.band_hz.1 - span);
        let at = |f: f64| -> f64 {
            let k = (f / bin).round() as usize;
            self.smooth.get(k).copied().unwrap_or(0.0)
        };
        let klo = ((self.cfg.band_hz.0 / bin) as usize).max(1);
        let khi = ((self.cfg.band_hz.1 / bin) as usize).min(self.smooth.len() - 1);
        if khi <= klo {
            return None;
        }
        let mean = self.smooth[klo..=khi].iter().sum::<f64>() / (khi - klo + 1) as f64;
        let mut best = (lo, 0.0f64);
        let mut f = lo;
        while f <= hi {
            let s: f64 = (0..self.cfg.tones)
                .map(|k| at(f + k as f64 * self.cfg.spacing_hz))
                .sum();
            if s > best.1 {
                best = (f, s);
            }
            f += 1.0;
        }
        if best.1 < 20.0 * mean || best.1 <= 0.0 {
            return None;
        }
        let mut base = best.0;
        if let Some(idle) = self.cfg.idle_pair {
            let sp = self.cfg.spacing_hz;
            let near = |f: f64| -> (f64, f64) {
                (-2..=2)
                    .map(|d| f + f64::from(d) * bin)
                    .map(|g| (g, at(g)))
                    .fold((f, -1.0), |m, v| if v.1 > m.1 { v } else { m })
            };
            // Two adjacent tones that alternate slot by slot keep the phase of a 2-FSK of index s T: the line midway between
            // them is the strongest, with the two tones at least 0.4 of it. In traffic the midway line is weak.
            let (kmax, pmax) = (klo..=khi).fold((klo, 0.0f64), |m, k| {
                if self.smooth[k] > m.1 {
                    (k, self.smooth[k])
                } else {
                    m
                }
            });
            let fmax = kmax as f64 * bin;
            let (lf, lp) = near(fmax - sp / 2.0);
            let (_, hp) = near(fmax + sp / 2.0);
            let strong: Vec<usize> = (0..self.cfg.tones)
                .filter(|&k| at(base + k as f64 * sp) > 0.25 * best.1)
                .collect();
            if lp >= 0.4 * pmax && hp >= 0.4 * pmax {
                base = lf - idle as f64 * sp;
            } else if strong.len() == 2 && strong[1] == strong[0] + 1 {
                base += (strong[0] as f64 - idle as f64) * sp;
            }
        }
        Some(base)
    }

    /// The best slot length, the start of a slot, and the mean share, when it locks.
    fn find_slots(&mut self) -> Option<(f64, f64, f64)> {
        let longest = self.cfg.slots_s.iter().copied().fold(0.0, f64::max);
        if (self.hist.len() as f64) < 80.0 * longest * self.cfg.rate + 2.0 {
            return None;
        }
        let mut best: Option<(f64, f64, f64)> = None;
        for si in 0..self.cfg.slots_s.len() {
            let slot_s = self.cfg.slots_s[si];
            let l = slot_s * self.cfg.rate;
            let end = self.n as f64 - 1.0;
            let anchor = end - 71.0 * l;
            let mut cand = (anchor, 0.0f64);
            for p in 0..8 {
                let t0 = anchor + p as f64 * l / 8.0;
                let mut s = 0.0f64;
                for k in 0..70 {
                    s += f64::from(self.slot(t0 + k as f64 * l, l).share);
                }
                let m = s / 70.0;
                if m > cand.1 {
                    cand = (t0, m);
                }
            }
            best = match best {
                None => Some((slot_s, cand.0, cand.1)),
                Some(b) if cand.1 > 1.05 * b.2 || (cand.1 > 0.9 * b.2 && slot_s > b.0) => {
                    Some((slot_s, cand.0, cand.1))
                }
                b => b,
            };
        }
        best.filter(|b| b.2 >= self.cfg.min_share)
    }

    fn decide(&mut self, slots: &mut Vec<Slot>) {
        let State::Locked { lock, mut t } = self.state else {
            return;
        };
        let l = lock.slot_s * self.cfg.rate;
        while (t + l).round() as u64 <= self.n {
            if t.round() < self.base_n as f64 {
                t += l;
                continue;
            }
            let s = self.slot(t, l);
            slots.push(s);
            t += l;
        }
        self.state = State::Locked { lock, t };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(tones: &[usize], base: f64, sp: f64, slot: f64, rate: f64) -> Vec<Complex32> {
        let mut ph = 0.0f64;
        let mut x = Vec::new();
        let l = (slot * rate) as usize;
        for &k in tones {
            for _ in 0..l {
                ph += TAU * (base + sp * k as f64) / rate;
                x.push(Complex32::from_polar(1.0, ph as f32));
            }
        }
        x
    }

    #[test]
    fn finds_the_slot_length_and_reads_every_slot() {
        let seq: Vec<usize> = (0..400).map(|i| (i * 5 + i / 3) % 8).collect();
        for slot in [0.0375, 0.05, 0.075] {
            let x = signal(&seq, 800.0, 26.67, slot, 8000.0);
            let cfg = ToneSlotConfig::new(8000.0, 8, 26.67, &[0.0375, 0.05, 0.075]);
            let mut a = Vec::new();
            let mut f = ToneSlotFront::new(cfg.clone()).unwrap();
            f.process(&x, &mut a);
            let lock = f.lock().unwrap();
            assert_eq!(lock.slot_s, slot);
            let got: Vec<usize> = a.iter().map(|s| usize::from(s.tone)).collect();
            assert!(got.windows(300).any(|w| w == &seq[80..380]), "{slot}");
            let mut b = Vec::new();
            let mut f = ToneSlotFront::new(cfg).unwrap();
            for c in x.chunks(77) {
                f.process(c, &mut b);
            }
            assert_eq!(a, b);
        }
    }

    #[test]
    fn an_idle_pair_names_its_tones() {
        let seq: Vec<usize> = (0..300).map(|i| 3 + i % 2).collect();
        let x = signal(&seq, 460.0, 20.0, 0.05, 8000.0);
        let mut cfg = ToneSlotConfig::new(8000.0, 6, 20.0, &[0.05]);
        cfg.idle_pair = Some(3);
        let mut f = ToneSlotFront::new(cfg).unwrap();
        let mut s = Vec::new();
        f.process(&x, &mut s);
        assert!((f.base_hz().unwrap() - 460.0).abs() < 2.0);
    }
}
