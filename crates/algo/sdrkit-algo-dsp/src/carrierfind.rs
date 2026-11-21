//! A carrier finder in a narrow window around 0 Hz: one Hann-windowed FFT of a block, the strongest bin within a search
//! half width, the noise per bin from the median of the bins of a band around 0 Hz away from the peak (divided by the
//! median-to-mean ratio `ln 2` of an exponential bin), a presence test against that noise, and the frequency refined by a
//! parabola through the logarithms of the peak bin and its neighbours.
//!
//! The carrier to noise density follows from the refined peak power `P` and the noise per bin `N`: a Hann window puts
//! `A^2 n^2 / 4` of a sine of amplitude `A` into its bin and `sigma^2 sum w^2 = sigma^2 3 n / 8` of complex noise of variance
//! `sigma^2` into every bin, so `P / N x enbw x rate / n` is `A^2 / (sigma^2 / rate)` with `enbw = 1.5` bins.
//!
//! ```
//! use sdrkit_algo_dsp::carrierfind::{CarrierFinder, FinderConfig};
//! use sdrkit_algo_dsp::Complex32;
//! let (rate, n) = (16_000.0, 16_384);
//! let x: Vec<Complex32> = (0..n)
//!     .map(|i| Complex32::from_polar(1.0, (std::f64::consts::TAU * 123.4 * i as f64 / rate) as f32))
//!     .collect();
//! let mut f = CarrierFinder::new(n, rate, FinderConfig::new(2800.0, 3000.0, 8, 40.0)).unwrap();
//! let look = f.look(&x).unwrap();
//! assert!(look.present && (look.offset_hz - 123.4).abs() < 0.1);
//! ```

use crate::fft::Fft;
use crate::peak::log_parabolic;
use crate::window::Window;
use crate::{Complex32, DspError, Param};

/// The search and the test of a [`CarrierFinder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinderConfig {
    /// The search half width around 0 Hz, Hz.
    pub search_hz: f64,
    /// The half width of the band whose bins give the noise, Hz.
    pub band_hz: f64,
    /// Every `band_step`-th bin of the band counts for the noise (1: every bin).
    pub band_step: usize,
    /// Bins within this circular distance of the peak do not count for the noise.
    pub exclude_bins: usize,
    /// The carrier is present when the peak bin is more than this many times the noise per bin.
    pub ratio: f64,
}

impl FinderConfig {
    /// A search of `search_hz` each side, noise from every bin within `band_hz` that is more than `exclude_bins` from the
    /// peak, presence at `ratio` times the noise per bin.
    pub const fn new(search_hz: f64, band_hz: f64, exclude_bins: usize, ratio: f64) -> Self {
        FinderConfig {
            search_hz,
            band_hz,
            band_step: 1,
            exclude_bins,
            ratio,
        }
    }

    /// The same with every `step`-th bin of the band counted for the noise.
    pub const fn every(mut self, step: usize) -> Self {
        self.band_step = step;
        self
    }
}

/// One look of a [`CarrierFinder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarrierLook {
    /// The peak bin is more than the ratio times the noise per bin.
    pub present: bool,
    /// The signed index of the peak bin.
    pub bin: i64,
    /// The refined frequency of the peak, Hz, in `-rate / 2 .. rate / 2`.
    pub offset_hz: f64,
    /// The power of the peak bin (unscaled transform).
    pub peak_power: f64,
    /// The refined peak power from the log parabola.
    pub refined_power: f64,
    /// The mean noise power per bin.
    pub noise_per_bin: f64,
    /// The carrier to noise density, dB-Hz, from the refined peak power.
    pub cn0_dbhz: f64,
}

/// The carrier finder of blocks of `n` samples at one rate. Nothing is allocated after [`CarrierFinder::new`].
#[derive(Debug)]
pub struct CarrierFinder {
    n: usize,
    rate: f64,
    cfg: FinderConfig,
    fft: Fft,
    window: Vec<f32>,
    enbw: f64,
    buf: Vec<Complex32>,
    power: Vec<f64>,
    noise: Vec<f64>,
}

/// The largest power of two not over `seconds x rate_hz` (at least 16).
pub fn block_len(rate_hz: f64, seconds: f64) -> usize {
    let want = (seconds * rate_hz).max(16.0) as usize;
    1usize << (usize::BITS - 1 - want.leading_zeros())
}

impl CarrierFinder {
    /// A finder of `n`-sample blocks at `rate_hz`. Errors when `n` is below 16, the rate is not above 0 or the ratio is not
    /// above 0.
    pub fn new(n: usize, rate_hz: f64, cfg: FinderConfig) -> Result<Self, DspError> {
        if n < 16 {
            return Param::FftSize.at_least(16.0, n as f64);
        }
        if !(rate_hz.is_finite() && rate_hz > 0.0) {
            return Param::Rate.positive(rate_hz);
        }
        if !(cfg.ratio.is_finite() && cfg.ratio > 0.0) {
            return Param::Margin.positive(cfg.ratio);
        }
        Ok(CarrierFinder {
            n,
            rate: rate_hz,
            cfg,
            fft: Fft::forward(n),
            window: Window::Hann.coefficients(n),
            enbw: Window::Hann.enbw_bins(n),
            buf: vec![Complex32::default(); n],
            power: vec![0.0; n],
            noise: Vec::with_capacity(n),
        })
    }

    /// The block length.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Always false: a finder has at least 16 bins.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// The configuration.
    pub fn config(&self) -> FinderConfig {
        self.cfg
    }

    /// Sets the configuration.
    pub fn set_config(&mut self, cfg: FinderConfig) {
        self.cfg = cfg;
    }

    /// Looks at the block `x` (its first `n` samples) over the whole search width. `None` when `x` is shorter than `n`.
    pub fn look(&mut self, x: &[Complex32]) -> Option<CarrierLook> {
        self.look_near(x, 0.0, self.cfg.search_hz)
    }

    /// Looks at `x` with the peak searched within `half_hz` of `centre_hz` (and within the search width). `None` when `x` is
    /// shorter than `n`.
    pub fn look_near(
        &mut self,
        x: &[Complex32],
        centre_hz: f64,
        half_hz: f64,
    ) -> Option<CarrierLook> {
        let n = self.n;
        if x.len() < n {
            return None;
        }
        for ((b, &s), &w) in self.buf.iter_mut().zip(x).zip(&self.window) {
            *b = s * w;
        }
        self.fft.process(&mut self.buf);
        for (p, b) in self.power.iter_mut().zip(&self.buf) {
            *p = f64::from(b.norm_sqr());
        }
        let bin_hz = self.bin_hz();
        let ni = n as i64;
        let wide = (self.cfg.search_hz / bin_hz).floor() as i64;
        let c = (centre_hz / bin_hz).round() as i64;
        let h = (half_hz / bin_hz).floor() as i64;
        let (lo, hi) = ((c - h).max(-wide), (c + h).min(wide));
        let at = |j: i64| j.rem_euclid(ni) as usize;
        let mut best: Option<(i64, f64)> = None;
        for j in lo..=hi {
            let p = self.power[at(j)];
            if best.is_none_or(|(_, q)| p > q) {
                best = Some((j, p));
            }
        }
        let (j, p) = best?;
        let band = (self.cfg.band_hz / bin_hz).floor() as i64;
        let step = self.cfg.band_step.max(1);
        self.noise.clear();
        let mut k = -band;
        while k <= band {
            let d = (k - j).rem_euclid(ni);
            if d.min(ni - d) as usize > self.cfg.exclude_bins {
                self.noise.push(self.power[at(k)]);
            }
            k += step as i64;
        }
        if self.noise.is_empty() {
            return None;
        }
        let m = self.noise.len() / 2;
        let (_, med, _) = self.noise.select_nth_unstable_by(m, f64::total_cmp);
        let noise = (*med / std::f64::consts::LN_2).max(1e-300);
        let fit = log_parabolic(self.power[at(j - 1)], p, self.power[at(j + 1)]);
        let mut f = (j as f64 + fit.offset) * bin_hz;
        if f >= self.rate / 2.0 {
            f -= self.rate;
        } else if f < -self.rate / 2.0 {
            f += self.rate;
        }
        Some(CarrierLook {
            present: p > self.cfg.ratio * noise,
            bin: j,
            offset_hz: f,
            peak_power: p,
            refined_power: fit.value,
            noise_per_bin: noise,
            cn0_dbhz: 10.0 * (fit.value / noise * self.enbw * bin_hz).max(1e-30).log10(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Complex Gaussian noise of power `p` per sample from SplitMix64 and Box-Muller.
    fn noise(n: usize, p: f64, seed: u64) -> Vec<Complex32> {
        let mut s = seed;
        let mut u = || {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            (((z ^ (z >> 31)) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| {
                let r = (-p * u().ln()).sqrt();
                let a = TAU * u();
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn a_tone_at_60_dbhz_reads_within_a_tenth_of_a_bin_and_its_cn0() {
        let (rate, n) = (16_000.0, 16_384);
        let p = rate / 1e6;
        let mut f =
            CarrierFinder::new(n, rate, FinderConfig::new(2800.0, 3000.0, 8, 40.0)).unwrap();
        let mut worst = 0.0f64;
        for seed in 0..5 {
            let mut x = noise(n, p, seed);
            for (i, z) in x.iter_mut().enumerate() {
                *z += Complex32::from_polar(1.0, (TAU * 123.4 * i as f64 / rate) as f32);
            }
            let l = f.look(&x).unwrap();
            assert!(l.present);
            worst = worst.max((l.offset_hz - 123.4).abs() / f.bin_hz());
            assert!((l.cn0_dbhz - 60.0).abs() < 1.5, "{}", l.cn0_dbhz);
        }
        assert!(worst < 0.1, "{worst} bins");
    }

    #[test]
    fn noise_alone_is_absent_in_1000_looks() {
        let (rate, n) = (16_000.0, 4096);
        let mut f =
            CarrierFinder::new(n, rate, FinderConfig::new(2800.0, 3000.0, 8, 40.0)).unwrap();
        let x = noise(n * 1000, 1.0, 9);
        let hits = x.chunks(n).filter(|b| f.look(b).unwrap().present).count();
        assert_eq!(hits, 0);
    }
}
