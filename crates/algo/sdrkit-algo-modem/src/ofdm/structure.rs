//! The OFDM structure detector: whether a stream holds cyclic-prefix OFDM of a given numerology, found by the correlation of
//! each sample with the one an FFT length earlier, folded over the symbol period; the symbol timing and the carrier offset from
//! the fold; and the shape of the power over a frame period (bursts or continuous).
//!
//! The running sum `S(t)` over the last `w` samples of `x(t) conj(x(t - N))` (`w` the shortest prefix) and the power of the
//! same samples are added into one bin per sample of the fold period (the sum of `N + prefix` over the symbols of the period).
//! At every report the fold is read and cleared: the peak bin against the mean of the bins farther than `w` from it (the ratio),
//! the peak against the mean power (the correlation, 0 to 1), the phase of the folded sum at the peak (the carrier offset within
//! half a carrier spacing). A half-carrier shift of the carriers (SC-FDMA) adds half a turn to that phase, which is taken out.
//! The power summed in blocks of 64 samples is folded over the frame period into 200 bins.
//!
//! ```
//! use sdrkit_algo_modem::ofdm::{Numerology, StructureDetector};
//! let n = Numerology::new(625e3, 64, &[11], 0.24).unwrap();
//! assert_eq!(n.period(), 75);
//! assert!((n.symbol_s() - 120e-6).abs() < 1e-12);
//! let mut d = StructureDetector::new(n).unwrap();
//! let x = vec![sdrkit_algo_modem::Complex32::new(0.0, 0.0); 40_000];
//! let mut reports = 0;
//! d.process(&x, |r| {
//!     assert!(!r.detected);
//!     reports += 1;
//! });
//! assert_eq!(reports, 1);
//! ```

use crate::{Complex32, ModemError, Param};
use std::f64::consts::TAU;

/// The bins of the frame envelope fold.
pub const FRAME_BINS: usize = 200;
/// Samples of a block of the frame envelope.
pub const FRAME_BLOCK: usize = 64;

/// One OFDM numerology as the detector looks for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Numerology {
    /// The sample rate the stream has, Hz.
    pub rate_hz: f64,
    /// The FFT length, samples.
    pub fft_len: usize,
    /// The cyclic prefix of each symbol of the fold period, samples: one entry for a fixed prefix, several for a slot whose
    /// symbols differ (a long first prefix).
    pub prefixes: Vec<usize>,
    /// The period of the frame envelope fold, s (a TDD frame, a super-frame, a 1 ms subframe).
    pub frame_s: f64,
    /// The carriers lie half a carrier spacing off the FFT grid (SC-FDMA uplinks).
    pub half_shift: bool,
}

impl Numerology {
    /// A numerology without a half-carrier shift.
    ///
    /// # Errors
    ///
    /// [`ModemError`] for a rate that is not positive, an FFT length under 8, no prefix, a prefix of 0 or longer than the FFT,
    /// or a frame period that is not positive.
    pub fn new(
        rate_hz: f64,
        fft_len: usize,
        prefixes: &[usize],
        frame_s: f64,
    ) -> Result<Self, ModemError> {
        Param::Rate.check_positive(rate_hz)?;
        Param::FftSize.check_count(fft_len, 8, 1 << 16)?;
        Param::Guard.check_count(prefixes.len(), 1, 64)?;
        for &p in prefixes {
            Param::Guard.check_count(p, 1, fft_len)?;
        }
        Param::Period.check_positive(frame_s)?;
        Ok(Numerology {
            rate_hz,
            fft_len,
            prefixes: prefixes.to_vec(),
            frame_s,
            half_shift: false,
        })
    }

    /// The same numerology with the carriers half a carrier spacing off the FFT grid.
    pub fn with_half_shift(mut self) -> Self {
        self.half_shift = true;
        self
    }

    /// The fold period: the sum of `N + prefix` over the symbols, samples.
    pub fn period(&self) -> usize {
        self.prefixes.iter().map(|p| self.fft_len + p).sum()
    }

    /// The shortest prefix, samples: the window of the running sum.
    pub fn window(&self) -> usize {
        self.prefixes.iter().copied().min().unwrap_or(1)
    }

    /// The carrier spacing, Hz.
    pub fn spacing_hz(&self) -> f64 {
        self.rate_hz / self.fft_len as f64
    }

    /// The duration of the first symbol with its prefix, s.
    pub fn symbol_s(&self) -> f64 {
        (self.fft_len + self.prefixes[0]) as f64 / self.rate_hz
    }

    /// The duration of the first prefix, s.
    pub fn prefix_s(&self) -> f64 {
        self.prefixes[0] as f64 / self.rate_hz
    }
}

/// What the power over the frame period looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameShape {
    /// Not enough of the stream yet.
    Unknown,
    /// About the same power over the whole period.
    Continuous,
    /// Bursts: on for this share of each period.
    Bursts {
        /// The share of the period with the power on, 0 to 1.
        duty: f64,
    },
}

/// The settings of the decision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructureConfig {
    /// Seconds of stream per report.
    pub report_s: f64,
    /// A detection holds this long after the last hit, s.
    pub hold_s: f64,
    /// The peak over the far bins that a hit needs.
    pub min_ratio: f64,
    /// The correlation a hit needs.
    pub min_correlation: f64,
    /// The mean power per sample a hit needs (not silence).
    pub min_power: f64,
    /// Every bin's power must reach this share of the mean bin power (the report is not the edge of a burst).
    pub min_bin_share: f64,
    /// The ratio that counts as energy without a hit.
    pub energy_ratio: f64,
}

impl Default for StructureConfig {
    fn default() -> Self {
        StructureConfig {
            report_s: 0.05,
            hold_s: 2.0,
            min_ratio: 2.2,
            min_correlation: 0.05,
            min_power: 1e-7,
            min_bin_share: 0.3,
            energy_ratio: 1.3,
        }
    }
}

/// One report of the detector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StructureReport {
    /// Stream time of the end of the report, s.
    pub time_s: f64,
    /// This report found the numerology.
    pub hit: bool,
    /// A hit in this report or within the hold time.
    pub detected: bool,
    /// No hit, but the peak stands out somewhat: energy of another kind.
    pub energy: bool,
    /// The peak over the mean of the far bins.
    pub ratio: f64,
    /// The prefix correlation, 0 to 1.
    pub correlation: f64,
    /// The bin of the peak: the symbol timing within the fold period, samples.
    pub timing: usize,
    /// The carrier offset, Hz (valid on a hit).
    pub cfo_hz: f64,
    /// The mean power per sample.
    pub mean_power: f64,
    /// The shape of the power over the frame period.
    pub frame: FrameShape,
}

/// The detector (module documentation).
#[derive(Clone, Debug)]
pub struct StructureDetector {
    num: Numerology,
    cfg: StructureConfig,
    period: usize,
    window: usize,
    /// The last `N + window` inputs.
    hist: Vec<Complex32>,
    hpos: usize,
    /// The last `window` products and powers.
    prods: Vec<(Complex32, f32)>,
    ppos: usize,
    s: (f64, f64),
    p: f64,
    n: u64,
    fold_mag: Vec<f64>,
    fold_s: Vec<(f64, f64)>,
    fold_p: Vec<f64>,
    in_report: usize,
    per_report: usize,
    last_hit: Option<f64>,
    frame_len: f64,
    block: f64,
    block_n: usize,
    block_start: u64,
    env: Vec<f64>,
    env_n: Vec<u32>,
    env_blocks: u64,
    shape: FrameShape,
}

impl StructureDetector {
    /// A detector of `num` with the default decision settings.
    ///
    /// # Errors
    ///
    /// [`ModemError`] when the numerology has a fold period over 2^20 samples or a report period under one fold period.
    pub fn new(num: Numerology) -> Result<Self, ModemError> {
        Self::with_config(num, StructureConfig::default())
    }

    /// A detector of `num` with the decision settings `cfg`.
    ///
    /// # Errors
    ///
    /// As [`StructureDetector::new`].
    pub fn with_config(num: Numerology, cfg: StructureConfig) -> Result<Self, ModemError> {
        let period = num.period();
        Param::Period.check_count(period, 1, 1 << 20)?;
        let per_report = (cfg.report_s * num.rate_hz).round() as usize;
        Param::Window.check_count(per_report, period, usize::MAX)?;
        let window = num.window();
        let frame_len = num.frame_s * num.rate_hz;
        Ok(StructureDetector {
            hist: vec![Complex32::default(); num.fft_len + window],
            hpos: 0,
            prods: vec![(Complex32::default(), 0.0); window],
            ppos: 0,
            s: (0.0, 0.0),
            p: 0.0,
            n: 0,
            fold_mag: vec![0.0; period],
            fold_s: vec![(0.0, 0.0); period],
            fold_p: vec![0.0; period],
            in_report: 0,
            per_report,
            last_hit: None,
            frame_len,
            block: 0.0,
            block_n: 0,
            block_start: 0,
            env: vec![0.0; FRAME_BINS],
            env_n: vec![0; FRAME_BINS],
            env_blocks: 0,
            shape: FrameShape::Unknown,
            num,
            cfg,
            period,
            window,
        })
    }

    /// The numerology.
    pub fn numerology(&self) -> &Numerology {
        &self.num
    }

    /// Forgets the stream.
    pub fn reset(&mut self) {
        let num = self.num.clone();
        if let Ok(d) = Self::with_config(num, self.cfg) {
            *self = d;
        }
    }

    /// Takes samples at the numerology's rate; calls `on_report` at the end of each report period.
    pub fn process(&mut self, x: &[Complex32], mut on_report: impl FnMut(&StructureReport)) {
        let n_fft = self.num.fft_len;
        let hlen = self.hist.len();
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            let old = self.hist[(self.hpos + hlen - n_fft) % hlen];
            self.hist[self.hpos] = v;
            self.hpos = (self.hpos + 1) % hlen;
            let prod = v * old.conj();
            let pw = 0.5 * (v.norm_sqr() + old.norm_sqr());
            let (gone, gone_p) = self.prods[self.ppos];
            self.prods[self.ppos] = (prod, pw);
            self.ppos = (self.ppos + 1) % self.window;
            self.s.0 += f64::from(prod.re) - f64::from(gone.re);
            self.s.1 += f64::from(prod.im) - f64::from(gone.im);
            self.p += f64::from(pw) - f64::from(gone_p);
            let bin = (self.n % self.period as u64) as usize;
            self.fold_mag[bin] += self.s.0.hypot(self.s.1);
            self.fold_s[bin].0 += self.s.0;
            self.fold_s[bin].1 += self.s.1;
            self.fold_p[bin] += self.p.max(0.0);
            self.block += f64::from(v.norm_sqr());
            self.block_n += 1;
            if self.block_n == FRAME_BLOCK {
                self.end_block();
            }
            self.n += 1;
            self.in_report += 1;
            if self.in_report == self.per_report {
                let r = self.report();
                on_report(&r);
            }
        }
    }

    fn end_block(&mut self) {
        if self.frame_len >= FRAME_BLOCK as f64 {
            let at = (self.block_start as f64).rem_euclid(self.frame_len) / self.frame_len;
            let b = ((at * FRAME_BINS as f64) as usize).min(FRAME_BINS - 1);
            self.env[b] += self.block;
            self.env_n[b] += 1;
            self.env_blocks += 1;
        }
        self.block = 0.0;
        self.block_n = 0;
        self.block_start = self.n + 1;
        // Eight periods held: the older half is forgotten.
        let per_frame = self.frame_len / FRAME_BLOCK as f64;
        if self.env_blocks as f64 >= 8.0 * per_frame {
            self.env.iter_mut().for_each(|v| *v *= 0.5);
            self.env_n.iter_mut().for_each(|v| *v /= 2);
            self.env_blocks /= 2;
        }
    }

    fn frame_shape(&self) -> FrameShape {
        let per_frame = self.frame_len / FRAME_BLOCK as f64;
        if (self.env_blocks as f64) < 2.0 * per_frame || self.env_n.contains(&0) {
            return FrameShape::Unknown;
        }
        let bins: Vec<f64> = self
            .env
            .iter()
            .zip(&self.env_n)
            .map(|(v, &n)| v / f64::from(n))
            .collect();
        let mean = bins.iter().sum::<f64>() / bins.len() as f64;
        let max = bins.iter().copied().fold(0.0, f64::max);
        let min = bins.iter().copied().fold(f64::INFINITY, f64::min);
        let over_half = bins.iter().filter(|&&b| b > 0.5 * mean).count();
        if mean > 0.0 && max > 1.6 * mean && (over_half as f64) < 0.8 * bins.len() as f64 {
            let mid = 0.5 * (max + min);
            let on = bins.iter().filter(|&&b| b > mid).count();
            FrameShape::Bursts {
                duty: on as f64 / bins.len() as f64,
            }
        } else {
            FrameShape::Continuous
        }
    }

    fn report(&mut self) -> StructureReport {
        let time_s = self.n as f64 / self.num.rate_hz;
        let (peak, &pv) = self
            .fold_mag
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap_or((0, &0.0));
        let (mut far, mut far_n) = (0.0, 0usize);
        for (i, &v) in self.fold_mag.iter().enumerate() {
            let d = i.abs_diff(peak);
            if d.min(self.period - d) > self.window {
                far += v;
                far_n += 1;
            }
        }
        let far = far / far_n.max(1) as f64;
        let ratio = if far > 0.0 { pv / far } else { 0.0 };
        let total_p: f64 = self.fold_p.iter().sum();
        let correlation = if total_p > 0.0 {
            (pv * self.period as f64 / total_p).min(1.0)
        } else {
            0.0
        };
        let mean_power = total_p / (self.in_report.max(1) * self.window) as f64;
        let mean_bin = total_p / self.period as f64;
        let even = self
            .fold_p
            .iter()
            .all(|&p| p >= self.cfg.min_bin_share * mean_bin);
        let hit = ratio > self.cfg.min_ratio
            && correlation > self.cfg.min_correlation
            && mean_power > self.cfg.min_power
            && even;
        let mut cfo_hz = 0.0;
        if hit {
            self.last_hit = Some(time_s);
            let (re, im) = self.fold_s[peak];
            let mut turn = im.atan2(re) / TAU;
            if self.num.half_shift {
                turn -= 0.5;
            }
            turn -= turn.round();
            cfo_hz = turn * self.num.spacing_hz();
        }
        let detected = self.last_hit.is_some_and(|t| time_s - t <= self.cfg.hold_s);
        self.shape = self.frame_shape();
        self.fold_mag.iter_mut().for_each(|v| *v = 0.0);
        self.fold_s.iter_mut().for_each(|v| *v = (0.0, 0.0));
        self.fold_p.iter_mut().for_each(|v| *v = 0.0);
        self.in_report = 0;
        StructureReport {
            time_s,
            hit,
            detected,
            energy: !hit && ratio > self.cfg.energy_ratio && mean_power > self.cfg.min_power,
            ratio,
            correlation,
            timing: peak,
            cfo_hz,
            mean_power,
            frame: self.shape,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CP-OFDM of `num` with random carriers, at `cfo` Hz, on for `duty` of each frame period.
    fn ofdm(num: &Numerology, seconds: f64, cfo: f64, duty: f64, seed: u64) -> Vec<Complex32> {
        let n = num.fft_len;
        let mut s = seed;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s
        };
        let total = (seconds * num.rate_hz) as usize;
        let mut x = Vec::with_capacity(total + num.period());
        let mut k = 0;
        while x.len() < total {
            let cp = num.prefixes[k % num.prefixes.len()];
            k += 1;
            let mut sym = vec![Complex32::default(); n];
            // Random samples: an OFDM symbol with random values on every carrier.
            for v in sym.iter_mut() {
                let q = next();
                *v = Complex32::new(
                    ((q & 0xFFFF) as f32 / 32768.0) - 1.0,
                    (((q >> 16) & 0xFFFF) as f32 / 32768.0) - 1.0,
                );
            }
            let start = x.len();
            x.extend_from_slice(&sym[n - cp..]);
            x.extend_from_slice(&sym);
            if num.half_shift {
                // The half-carrier shift runs on through the prefix: each sample of the symbol turns by a half carrier.
                for (i, v) in x[start..].iter_mut().enumerate() {
                    let ph = TAU * 0.5 * (i as f64 - cp as f64) / n as f64;
                    *v *= Complex32::from_polar(1.0, ph as f32);
                }
            }
        }
        x.truncate(total);
        let frame = num.frame_s * num.rate_hz;
        for (i, v) in x.iter_mut().enumerate() {
            let ph = TAU * cfo * i as f64 / num.rate_hz;
            *v *= Complex32::from_polar(1.0, ph as f32);
            if (i as f64).rem_euclid(frame) >= duty * frame {
                *v = Complex32::default();
            }
        }
        x
    }

    fn last(d: &mut StructureDetector, x: &[Complex32]) -> StructureReport {
        let mut last = None;
        d.process(x, |r| last = Some(*r));
        last.unwrap()
    }

    #[test]
    fn finds_the_numerology_its_timing_and_offset() {
        let num = Numerology::new(625e3, 64, &[11], 0.24).unwrap();
        let x = ofdm(&num, 0.6, 2000.0, 1.0, 3);
        let mut d = StructureDetector::new(num.clone()).unwrap();
        let r = last(&mut d, &x);
        assert!(r.hit && r.detected, "{r:?}");
        assert!(r.correlation > 0.5, "{r:?}");
        assert!((r.cfo_hz - 2000.0).abs() < 100.0, "{r:?}");
        assert_eq!(r.frame, FrameShape::Continuous);
        // Another FFT length on the same signal finds nothing.
        let other = Numerology::new(625e3, 48, &[11], 0.24).unwrap();
        let mut d = StructureDetector::new(other).unwrap();
        assert!(!last(&mut d, &x).hit);
    }

    #[test]
    fn bursts_over_the_frame_period() {
        let num = Numerology::new(5.6e6, 512, &[64], 0.005).unwrap();
        let x = ofdm(&num, 0.1, 0.0, 0.6, 5);
        let mut d = StructureDetector::new(num).unwrap();
        let r = last(&mut d, &x);
        assert!(r.detected, "{r:?}");
        match r.frame {
            FrameShape::Bursts { duty } => assert!((duty - 0.6).abs() < 0.05, "{duty}"),
            f => panic!("{f:?}"),
        }
    }

    #[test]
    fn several_prefixes_and_a_half_shift() {
        // A slot of seven symbols, the first prefix longer, the carriers half a spacing off the grid.
        let num = Numerology::new(1.92e6, 128, &[10, 9, 9, 9, 9, 9, 9], 0.001)
            .unwrap()
            .with_half_shift();
        assert_eq!(num.period(), 960);
        let x = ofdm(&num, 0.2, 1500.0, 1.0, 7);
        let mut d = StructureDetector::new(num).unwrap();
        let r = last(&mut d, &x);
        assert!(r.hit, "{r:?}");
        assert!((r.cfo_hz - 1500.0).abs() < 200.0, "{r:?}");
    }

    #[test]
    fn noise_and_silence_are_not_ofdm() {
        let num = Numerology::new(625e3, 64, &[11], 0.24).unwrap();
        let mut s = 99u64;
        let x: Vec<Complex32> = (0..400_000)
            .map(|_| {
                s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                Complex32::new(
                    ((s >> 33) as f32 / 2_147_483_648.0) - 1.0,
                    ((s >> 11 & 0x3F_FFFF) as f32 / 2_097_152.0) - 1.0,
                )
            })
            .collect();
        let mut d = StructureDetector::new(num.clone()).unwrap();
        let mut any = false;
        d.process(&x, |r| any |= r.hit);
        assert!(!any);
        let mut d = StructureDetector::new(num).unwrap();
        let r = last(&mut d, &vec![Complex32::default(); 100_000]);
        assert!(!r.hit && !r.energy);
        assert!(Numerology::new(625e3, 64, &[], 0.24).is_err());
        assert!(Numerology::new(625e3, 64, &[65], 0.24).is_err());
    }
}
