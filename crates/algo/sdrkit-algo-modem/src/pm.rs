//! Phase modulation with a residual carrier: the data move the phase of the carrier by plus or minus an index below 90 degrees,
//! so a share `cos^2(index)` of the power stays in a carrier line. Low-rate satellite beacons and telemetry links use it, often
//! with split-phase data.
//!
//! [`PmDemod`] finds the carrier line in a spectrum of the signal itself, tracks it with a second-order loop, and reads the data
//! from the quadrature part: a matched filter, a Gardner timing loop and soft symbols (LLRs, positive means 0; a symbol of
//! positive phase is a 1). A split-phase link gives one soft value for each chip.
//!
//! [`PmBurst`] is the burst form for a split-phase (Manchester) burst behind a bare-carrier preamble: no loop; the phase of each
//! bit comes from a sliding sum of the carrier vectors of the bits around it, and the soft bits are LLRs with the same
//! convention (a bit whose first half has positive phase is a 1).
//!
//! ```
//! use sdrkit_algo_modem::pm::{PmConfig, PmDemod};
//! use sdrkit_core::Complex32;
//! let mut d = PmDemod::new(PmConfig::new(64_000.0, 16_000.0, 1.0)).unwrap();
//! let mut soft = Vec::new();
//! d.process(&[Complex32::new(1.0, 0.0); 8192], &mut soft);
//! assert!(!d.tracking()); // The search needs more samples.
//! ```

use crate::timing::{Gardner, SymbolSync, SymbolSyncConfig};
use crate::{ModemError, Param};
use num_complex::Complex64;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_algo_dsp::pulse::{rrc, Shape};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::collections::VecDeque;

/// The most samples of one carrier search.
const SEARCH_MAX: usize = 32_768;
/// The carrier line must stand this many times over the mean of the searched bins.
const LINE_RATIO: f32 = 6.0;
/// Smoothing of the lock value, for each sample.
const LOCK_ALPHA: f32 = 0.0005;
/// Smoothing of the symbol size, for each symbol.
const AMP_ALPHA: f32 = 0.002;
/// Smoothing of the Es/N0 moments, for each symbol.
const SNR_ALPHA: f32 = 0.003;
/// Symbols kept for the constellation.
const POINTS: usize = 256;

/// The options of a [`PmDemod`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PmConfig {
    /// Input samples per second.
    pub rate: f64,
    /// Symbols per second (chips per second for split-phase data).
    pub symbol_rate: f64,
    /// The roll-off of the root raised cosine matched filter.
    pub roll_off: f64,
    /// The carrier is searched within this many Hz of the centre, and never beyond 0.45 of the symbol rate or of the rate.
    pub search_hz: f64,
    /// The loop noise bandwidth for each sample. [`PmConfig::new`] sets 0.004 at 4 samples a symbol, in proportion to the
    /// samples a symbol.
    pub loop_bn_t: f64,
}

impl PmConfig {
    /// Options with the widest search and the usual loop bandwidth.
    pub fn new(rate: f64, symbol_rate: f64, roll_off: f64) -> Self {
        PmConfig {
            rate,
            symbol_rate,
            roll_off,
            search_hz: f64::INFINITY,
            loop_bn_t: 0.004 * (rate / symbol_rate) / 4.0,
        }
    }
}

/// The demodulator of a residual-carrier PM signal. See the [module documentation](self).
#[derive(Debug)]
pub struct PmDemod {
    cfg: PmConfig,
    fft: Fft,
    window: Vec<f32>,
    search: Vec<Complex32>,
    tracking: bool,
    searches: u32,
    nco: PhaseAcc,
    lf: LoopFilter,
    mean_i: f32,
    mean_power: f32,
    matched: Fir<f32>,
    data: Vec<f32>,
    sync: SymbolSync<f32, Gardner, Cubic>,
    symbols: Vec<f32>,
    amp: f32,
    m1: f32,
    m2: f32,
    count: u64,
    /// The newest symbols at unit size, on the real axis.
    pub constellation: VecDeque<[f32; 2]>,
}

impl PmDemod {
    /// A demodulator.
    ///
    /// # Errors
    ///
    /// [`ModemError`] if the rate or the symbol rate is not positive, there are fewer than 2 samples a symbol, or the roll-off
    /// or the loop bandwidth is out of range.
    pub fn new(cfg: PmConfig) -> Result<Self, ModemError> {
        if !(cfg.rate > 0.0 && cfg.rate.is_finite()) {
            return Param::Rate.positive(cfg.rate);
        }
        if !(cfg.symbol_rate > 0.0 && cfg.symbol_rate.is_finite()) {
            return Param::SymbolRate.positive(cfg.symbol_rate);
        }
        let sps = cfg.rate / cfg.symbol_rate;
        if sps < 2.0 {
            return Param::Sps.at_least(2.0, sps);
        }
        if !(0.0..=1.0).contains(&cfg.roll_off) {
            return Param::RollOff.range(0.0, 1.0, cfg.roll_off);
        }
        let n = SEARCH_MAX;
        let window = (0..n)
            .map(|i| Window::Hann.value_symmetric(i, n) as f32)
            .collect();
        let mut lf = LoopFilter::new(cfg.loop_bn_t, std::f64::consts::FRAC_1_SQRT_2)?;
        lf.set_clamp(0.45);
        // The matched filter: +-4 symbols, unit energy.
        let half = (4.0 * sps).ceil() as i64;
        let mut taps: Vec<f32> = (-half..=half)
            .map(|i| rrc(i as f64 / sps, cfg.roll_off) as f32)
            .collect();
        let energy: f32 = taps.iter().map(|t| t * t).sum();
        let scale = 1.0 / energy.sqrt().max(1e-12);
        taps.iter_mut().for_each(|t| *t *= scale);
        let mut t = SymbolSyncConfig::new(sps);
        t.bn_t = 0.006;
        t.zeta = 0.9;
        t.max_ppm = 8000.0;
        t.pulse = Shape::Rc { beta: cfg.roll_off };
        Ok(PmDemod {
            cfg,
            fft: Fft::forward(n),
            window,
            search: Vec::with_capacity(n),
            tracking: false,
            searches: 0,
            nco: PhaseAcc::new(0.0),
            lf,
            mean_i: 0.0,
            mean_power: 0.0,
            matched: Fir::new(&taps),
            data: Vec::new(),
            sync: SymbolSync::new(Gardner, Cubic, t)?,
            symbols: Vec::new(),
            amp: 1.0,
            m1: 1.0,
            m2: 1.0,
            count: 0,
            constellation: VecDeque::with_capacity(POINTS),
        })
    }

    /// The options.
    pub fn config(&self) -> &PmConfig {
        &self.cfg
    }

    /// The carrier search has finished and the loops run.
    pub fn tracking(&self) -> bool {
        self.tracking
    }

    /// Carrier searches finished so far.
    pub fn searches(&self) -> u32 {
        self.searches
    }

    /// The lock value: the square of the mean in-phase part over the mean power, after the loop. Near 0 in noise; the square
    /// of the mean of `cos(index d(t))` on a clean signal.
    pub fn lock_value(&self) -> f32 {
        if self.tracking && self.mean_power > 1e-20 {
            self.mean_i * self.mean_i / self.mean_power
        } else {
            0.0
        }
    }

    /// The carrier offset the loop holds, Hz.
    pub fn carrier_hz(&self) -> f64 {
        self.nco.freq() * self.cfg.rate
    }

    /// The symbol clock against nominal, ppm.
    pub fn clock_ppm(&self) -> f64 {
        self.sync.clock_ppm()
    }

    /// Symbols since the last search.
    pub fn symbols(&self) -> u64 {
        self.count
    }

    /// Es/N0 from the mean `m` and the variance `v` of the symbol sizes, `10 log10(m^2 / (2 v))`, not below -30 dB.
    pub fn es_n0_db(&self) -> f32 {
        let v = (self.m2 - self.m1 * self.m1).max(1e-9);
        (10.0 * (self.m1 * self.m1 / (2.0 * v)).log10()).max(-30.0)
    }

    /// Searches the carrier again from the next sample.
    pub fn restart(&mut self) {
        self.tracking = false;
        self.search.clear();
        self.lf.reset();
        self.mean_i = 0.0;
        self.mean_power = 0.0;
        self.sync.reset();
        self.count = 0;
    }

    /// The strongest line of the search buffer, in cycles a sample, when it stands clear of the mean.
    fn find_line(&mut self) -> Option<f64> {
        let n = self.search.len();
        for (z, w) in self.search.iter_mut().zip(&self.window) {
            let m = z.norm();
            *z = if m > 1e-20 {
                *z * (w / m)
            } else {
                Complex32::default()
            };
        }
        self.fft.process(&mut self.search);
        let limit_hz = self
            .cfg
            .search_hz
            .min(0.45 * self.cfg.symbol_rate)
            .min(0.45 * self.cfg.rate);
        let reach = ((limit_hz / self.cfg.rate * n as f64) as usize).clamp(2, n / 2 - 1);
        let power = |k: i64| self.search[k.rem_euclid(n as i64) as usize].norm_sqr();
        let (mut best, mut best_p, mut sum) = (0i64, 0f32, 0f32);
        for k in -(reach as i64)..=reach as i64 {
            let p = power(k);
            sum += p;
            if p > best_p {
                best_p = p;
                best = k;
            }
        }
        let mean = sum / (2 * reach + 1) as f32;
        if best_p <= LINE_RATIO * mean {
            return None;
        }
        // A parabola over the two neighbour bins, on the magnitudes.
        let (a, b, c) = (
            power(best - 1).sqrt(),
            best_p.sqrt(),
            power(best + 1).sqrt(),
        );
        let den = a - 2.0 * b + c;
        let frac = if den.abs() > 1e-12 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some((best as f64 + f64::from(frac)) / n as f64)
    }

    /// Takes samples and appends one soft value for each symbol to `soft`.
    pub fn process(&mut self, input: &[Complex32], soft: &mut Vec<i8>) {
        let mut x = input;
        if !self.tracking {
            let take = (SEARCH_MAX - self.search.len()).min(x.len());
            self.search.extend_from_slice(&x[..take]);
            x = &x[take..];
            if self.search.len() < SEARCH_MAX {
                return;
            }
            // Without a clear line the loop starts where it is.
            if let Some(f) = self.find_line() {
                self.nco.set_freq(f);
            }
            self.search.clear();
            self.tracking = true;
            self.searches += 1;
        }
        self.data.clear();
        for &s in x {
            let z = self.nco.derotate(s);
            self.nco.advance();
            // The data are in this angle too. Balanced data average out inside the loop bandwidth.
            let err = f64::from(z.im.atan2(z.re)) / std::f64::consts::TAU / 2.0;
            let before = self.lf.integrator();
            let step = self.lf.update(err);
            let after = self.lf.integrator();
            self.nco.set_freq(self.nco.freq() + (after - before));
            self.nco.steer(step - after);
            self.mean_i += LOCK_ALPHA * (z.re - self.mean_i);
            self.mean_power += LOCK_ALPHA * (z.norm_sqr() - self.mean_power);
            self.data.push(self.matched.process_sample(z.im));
        }
        self.symbols.clear();
        self.sync.process(&self.data, &mut self.symbols);
        for &s in &self.symbols {
            self.amp += AMP_ALPHA * (s.abs() - self.amp);
            let y = s / self.amp.max(1e-6);
            self.m1 += SNR_ALPHA * (y.abs() - self.m1);
            self.m2 += SNR_ALPHA * (y * y - self.m2);
            self.count += 1;
            if self.constellation.len() == POINTS {
                self.constellation.pop_front();
            }
            self.constellation.push_back([y, 0.0]);
            soft.push((-y * 40.0).clamp(-127.0, 127.0) as i8);
        }
    }
}

/// Bit periods of bare carrier before the first bit that [`PmBurst`] reads for the phase.
pub const BURST_PREAMBLE_BITS: usize = 20;
/// Bits each side of a bit in the phase sum of [`PmBurst`] (21 bits in all).
pub const BURST_PHASE_HALF: usize = 10;

/// What [`PmBurst::soft_bits`] measured on the burst.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BurstStats {
    /// `mean |y|` over the bits read.
    pub m1: f32,
    /// The variance of `|y|` used for the LLRs.
    pub var: f32,
    /// `sum |y|` over the bits read (a measure of a candidate timing).
    pub sum_abs: f32,
    /// Bits that lay inside the samples.
    pub bits: usize,
}

/// Coherent detection of a split-phase (Manchester) phase-modulated burst of `+-beta` rad behind a bare-carrier preamble, for
/// the 406 MHz beacons and the like. Input: the burst mixed to 0 Hz, a candidate start `t0` and a bit period `T` (both in
/// samples, fractional).
///
/// 1. Prefix sums of the samples (once per burst, [`load`](Self::load)); for bit `k`, from `t0 + k T`, the half sums `h1` and
///    `h2` (prefix sums read at fractional positions by linear interpolation).
/// 2. Carrier vector `c_k = h1 + h2`, data vector `d_k = h1 - h2`: with `+-beta` every bit keeps `cos(beta)` of the carrier
///    whatever its value.
/// 3. Phase for bit `k`: `R_k = sum c_j`, `j = k - 10 ..= k + 10`, over the bits from 20 bit periods before `t0` (the preamble) to
///    the end, leaving out bits that do not lie wholly inside the samples.
/// 4. Soft bit `y_k = Im(d_k conj(R_k)) / |R_k|` (0 when `|R_k|` is under 1e-12): positive when the first half has positive
///    phase.
/// 5. LLR from the burst's own statistics: `m1 = mean |y|`, `m2 = mean y^2`, `var = max(m2 - m1^2, 1e-3 m2 + 1e-30)`,
///    `llr_k = -y_k 2 m1 / var`: positive means 0, and a bit whose first half has positive phase is a 1, as for [`PmDemod`].
///
/// Many candidate timings of one burst cost one pass over the bits each. Nothing is allocated per candidate once the buffers
/// have grown to the burst.
#[derive(Clone, Debug, Default)]
pub struct PmBurst {
    prefix: Vec<Complex64>,
    carrier: Vec<Complex64>,
    data: Vec<Complex64>,
    inside: Vec<bool>,
}

impl PmBurst {
    /// An empty detector.
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes a burst (mixed to 0 Hz) and computes its prefix sums.
    pub fn load(&mut self, x: &[Complex32]) {
        self.prefix.clear();
        self.prefix.reserve(x.len() + 1);
        let mut acc = Complex64::default();
        self.prefix.push(acc);
        for z in x {
            acc += Complex64::new(z.re as f64, z.im as f64);
            self.prefix.push(acc);
        }
    }

    /// The samples loaded.
    pub fn len(&self) -> usize {
        self.prefix.len().saturating_sub(1)
    }

    /// Whether no sample is loaded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The sum of the samples before position `t` (fractional, within the samples).
    fn at(&self, t: f64) -> Complex64 {
        let i = t.floor() as usize;
        let f = t - i as f64;
        if i + 1 >= self.prefix.len() {
            return self.prefix[self.prefix.len() - 1];
        }
        self.prefix[i] + (self.prefix[i + 1] - self.prefix[i]) * f
    }

    /// Writes the LLRs of `out.len()` bits from `t0` with bit period `period` (module docs, [`PmBurst`]); bits not wholly inside
    /// the samples get 0. Returns the burst's statistics. Errors for a period under 2 samples or a `t0` that is not finite.
    pub fn soft_bits(
        &mut self,
        t0: f64,
        period: f64,
        out: &mut [f32],
    ) -> Result<BurstStats, ModemError> {
        Param::Period.check_in(period, 2.0, f64::MAX)?;
        Param::Start.check_finite(t0)?;
        let len = self.len() as f64;
        let nb = out.len();
        let total = BURST_PREAMBLE_BITS + nb;
        self.carrier.clear();
        self.data.clear();
        self.inside.clear();
        for j in 0..total {
            let s = t0 + (j as f64 - BURST_PREAMBLE_BITS as f64) * period;
            let e = s + period;
            if s < 0.0 || e > len {
                self.carrier.push(Complex64::default());
                self.data.push(Complex64::default());
                self.inside.push(false);
                continue;
            }
            let (a, m, b) = (self.at(s), self.at(s + 0.5 * period), self.at(e));
            let (h1, h2) = (m - a, b - m);
            self.carrier.push(h1 + h2);
            self.data.push(h1 - h2);
            self.inside.push(true);
        }
        // The sliding phase sum over j = k - 10 ..= k + 10 (outside bits hold zero vectors).
        let h = BURST_PHASE_HALF;
        let mut r: Complex64 = self.carrier[..(h + 1).min(total)].iter().sum();
        let (mut s1, mut s2, mut count) = (0f64, 0f64, 0usize);
        for k in 0..total {
            if k > 0 {
                if k + h < total {
                    r += self.carrier[k + h];
                }
                if k > h {
                    r -= self.carrier[k - h - 1];
                }
            }
            if k < BURST_PREAMBLE_BITS {
                continue;
            }
            let o = &mut out[k - BURST_PREAMBLE_BITS];
            let rn = r.norm();
            *o = if self.inside[k] && rn >= 1e-12 {
                ((self.data[k] * r.conj()).im / rn) as f32
            } else {
                0.0
            };
            if self.inside[k] {
                let y = *o as f64;
                s1 += y.abs();
                s2 += y * y;
                count += 1;
            }
        }
        if count == 0 {
            out.fill(0.0);
            return Ok(BurstStats::default());
        }
        let (m1, m2) = (s1 / count as f64, s2 / count as f64);
        let var = (m2 - m1 * m1).max(1e-3 * m2 + 1e-30);
        let scale = (-2.0 * m1 / var) as f32;
        for o in out.iter_mut() {
            *o *= scale;
        }
        Ok(BurstStats {
            m1: m1 as f32,
            var: var as f32,
            sum_abs: s1 as f32,
            bits: count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_fec::bch::{codes, Bch};
    use sdrkit_algo_fec::chase::{ChaseDecoder, DEFAULT_MAX_FAILED};
    use sdrkit_algo_sim::noise::Awgn;
    use sdrkit_algo_sim::rng::Rng;
    use std::f64::consts::TAU;

    const RATE: f64 = 32_000.0;
    const T: usize = 80;

    /// A 406 MHz first-generation burst at 0 Hz: 160 ms of carrier, then `bits` in split phase of +-1.1 rad (a 1 is +1.1 rad in
    /// its first half), unit power, carrier phase `phase`, a residual offset of `hz`.
    fn burst(bits: &[u8], phase: f64, hz: f64) -> (Vec<Complex32>, usize) {
        let pre = (0.16 * RATE) as usize;
        let n = pre + bits.len() * T + 400;
        let mut x = Vec::with_capacity(n);
        for i in 0..n {
            let a = if i < pre || i >= pre + bits.len() * T {
                0.0
            } else {
                let k = (i - pre) / T;
                let first = (i - pre) % T < T / 2;
                let up = (bits[k] == 1) == first;
                if up {
                    1.1
                } else {
                    -1.1
                }
            };
            x.push(Complex32::from_polar(
                1.0,
                (phase + TAU * hz * i as f64 / RATE + a) as f32,
            ));
        }
        (x, pre)
    }

    /// 112 bits: the 24 sync bits, the BCH-1 word (61 + 21 bits), 6 more bits.
    fn message(bch: &Bch, rng: &mut Rng) -> Vec<u8> {
        let mut m = vec![0u8; 112];
        m[..15].fill(1);
        m[15..24].copy_from_slice(&[0, 0, 0, 1, 0, 1, 1, 1, 1]);
        for (i, b) in m.iter_mut().enumerate().skip(24) {
            if !(85..106).contains(&i) {
                *b = rng.bit();
            }
        }
        bch.encode(&mut m[24..106]).unwrap();
        m
    }

    #[test]
    fn burst_soft_bits_follow_the_convention() {
        let bch = Bch::from_code(&codes::SARSAT_82_61).unwrap();
        let mut rng = Rng::new(3);
        let sent = message(&bch, &mut rng);
        let (x, pre) = burst(&sent, 2.0, 3.0);
        let mut b = PmBurst::new();
        b.load(&x);
        let mut llr = vec![0f32; 112];
        let st = b.soft_bits(pre as f64, T as f64, &mut llr).unwrap();
        assert_eq!(st.bits, 112);
        for (l, &bit) in llr.iter().zip(&sent) {
            assert_eq!(*l < 0.0, bit == 1);
            assert!(l.abs() > 10.0);
        }
        // Bits past the end of the samples read 0.
        let mut long = vec![0f32; 130];
        let st = b.soft_bits(pre as f64, T as f64, &mut long).unwrap();
        // 400 samples follow the message: 5 more bits lie inside.
        assert_eq!(st.bits, 117);
        assert!(long[117..].iter().all(|&v| v == 0.0));
        assert!(b.soft_bits(0.0, 1.0, &mut llr).is_err());
    }

    #[test]
    fn coherent_chase_decodes_where_differential_hard_decoding_fails() {
        // SNR -3 dB in 3 kHz (burst power over the noise in 3 kHz), 32 kHz, random carrier phase, 2 Hz left over; the timing is
        // known. Coherent soft bits with Chase (7 weakest) against the differential bits of the half sums with hard BCH.
        let mut bch = Bch::from_code(&codes::SARSAT_82_61).unwrap();
        let mut chase = ChaseDecoder::new(7, 7, DEFAULT_MAX_FAILED).unwrap();
        let mut rng = Rng::new(17);
        let noise = RATE / 3000.0 / 10f64.powf(-0.3);
        let trials = 100;
        let (mut coh, mut diff) = (0, 0);
        let mut b = PmBurst::new();
        let mut llr = vec![0f32; 112];
        let mut out = vec![0u8; 112];
        for t in 0..trials {
            let sent = message(&bch, &mut rng);
            let (mut x, pre) = burst(&sent, TAU * rng.uniform(), 2.0);
            Awgn::with_power(noise, 100 + t).unwrap().process(&mut x);
            b.load(&x);
            b.soft_bits(pre as f64, T as f64, &mut llr).unwrap();
            let hard: Vec<u8> = llr.iter().map(|&v| (v < 0.0) as u8).collect();
            if chase
                .decode(&mut bch, &hard, &llr, 24..106, 24, |_| true, &mut out)
                .is_some()
                && out[24..106] == sent[24..106]
            {
                coh += 1;
            }
            let mut w: Vec<u8> = (24..106)
                .map(|k| {
                    let s = pre + k * T;
                    let h1: Complex32 = x[s..s + T / 2].iter().sum();
                    let h2: Complex32 = x[s + T / 2..s + T].iter().sum();
                    ((h1 * h2.conj()).im > 0.0) as u8
                })
                .collect();
            if bch.decode(&mut w).is_ok() && w == sent[24..106] {
                diff += 1;
            }
        }
        assert!(
            coh >= 90 && diff <= 30,
            "coherent {coh}, differential {diff}"
        );
    }
}
