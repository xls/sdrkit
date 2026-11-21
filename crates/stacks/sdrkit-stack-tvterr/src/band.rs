//! The front-end helpers of the receivers of this crate: an averaged power spectrum of the band, the search for the channel in
//! it, and the resampler with clock steering.

use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;

/// The frame size of an [`AvgSpectrum`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandFft {
    /// 1024 points.
    N1024,
    /// 4096 points.
    N4096,
    /// 8192 points.
    N8192,
}

impl BandFft {
    /// The number of points.
    pub fn len(self) -> usize {
        match self {
            BandFft::N1024 => 1024,
            BandFft::N4096 => 4096,
            BandFft::N8192 => 8192,
        }
    }

    /// Never empty.
    pub fn is_empty(self) -> bool {
        false
    }
}

/// A power spectrum averaged over many calls (Hann window, frames without overlap, non-finite samples zeroed), in natural
/// frequency order (index 0 is −rate/2). Each call's whole frames go through the shared Welch estimator.
#[derive(Debug)]
pub struct AvgSpectrum {
    welch: Welch,
    frame: Vec<f32>,
    clean: Vec<Complex32>,
    /// Accumulated power per bin.
    pub power: Vec<f64>,
    /// Frames averaged.
    pub frames: usize,
}

impl AvgSpectrum {
    /// An empty average of frames of `size` points.
    pub fn new(size: BandFft) -> AvgSpectrum {
        let n = size.len();
        let welch = match Welch::new(n, Window::Hann, 0.0, Scaling::Power) {
            Ok(w) => w,
            // Every `BandFft` size is a power of two that the estimator takes.
            Err(e) => unreachable!("{e}"),
        };
        AvgSpectrum {
            welch,
            frame: vec![0.0; n],
            clean: Vec::new(),
            power: vec![0.0; n],
            frames: 0,
        }
    }

    /// Bins.
    pub fn len(&self) -> usize {
        self.power.len()
    }

    /// Whether it has no bins.
    pub fn is_empty(&self) -> bool {
        self.power.is_empty()
    }

    /// Adds every whole frame of `x`.
    pub fn add_all(&mut self, x: &[Complex32]) {
        let n = self.len();
        let whole = x.len() / n * n;
        if whole == 0 {
            return;
        }
        let x = &x[..whole];
        let src = if x.iter().all(|s| s.re.is_finite() && s.im.is_finite()) {
            x
        } else {
            self.clean.clear();
            self.clean.extend(x.iter().map(|s| {
                if s.re.is_finite() && s.im.is_finite() {
                    *s
                } else {
                    Complex32::new(0.0, 0.0)
                }
            }));
            &self.clean
        };
        if let Ok(k) = self.welch.compute(src, &mut self.frame) {
            for (p, &f) in self.power.iter_mut().zip(&self.frame) {
                *p += f as f64 * k as f64;
            }
            self.frames += k;
        }
    }

    /// Clears the average.
    pub fn clear(&mut self) {
        self.power.iter_mut().for_each(|p| *p = 0.0);
        self.frames = 0;
    }

    /// The mean power per bin.
    pub fn mean(&self) -> Vec<f64> {
        let f = self.frames.max(1) as f64;
        self.power.iter().map(|p| p / f).collect()
    }
}

/// A streaming resampler from `in_rate` to `out_rate`, flat to `pass_hz` and 80 dB down from `stop_hz`, whose step can be scaled
/// for a clock error. A stop band that the input rate cannot hold moves inside it.
///
/// # Errors
///
/// [`DspError`] if the rates cannot make a filter (a rate that is not positive, or a pass band wider than the input rate).
pub fn resampler(
    in_rate: f64,
    out_rate: f64,
    pass_hz: f64,
    stop_hz: f64,
) -> Result<FractionalResampler<Complex32>, DspError> {
    let stop = stop_hz.min(in_rate * 0.999 - pass_hz).max(pass_hz * 1.001);
    FractionalResampler::new(in_rate, out_rate, pass_hz, stop, 80.0)
}

/// Finds the box of `width_bins` with the most power in `p` (natural order). Returns the box start, the mean power inside, and
/// whether the bands just outside its edges (each about 1/25 of the box wide, 2 bins from the edge) average less than a quarter
/// of the inside (or lie outside the band).
pub fn strongest_box(p: &[f64], width_bins: usize) -> Option<(usize, f64, bool)> {
    let n = p.len();
    if width_bins == 0 || width_bins > n {
        return None;
    }
    let mut csum = vec![0.0; n + 1];
    for i in 0..n {
        csum[i + 1] = csum[i] + p[i];
    }
    let mut best = (0usize, -1.0f64);
    for s in 0..=n - width_bins {
        let v = csum[s + width_bins] - csum[s];
        if v > best.1 {
            best = (s, v);
        }
    }
    let (s, v) = best;
    let inside = v / width_bins as f64;
    let edge = (width_bins / 25).max(1);
    let lo = s.checked_sub(2 + edge);
    let hi_start = s + width_bins + 2;
    let mut ok = true;
    if let Some(lo) = lo {
        let m = (csum[lo + edge] - csum[lo]) / edge as f64;
        ok &= m < 0.25 * inside;
    }
    if hi_start + edge <= n {
        let m = (csum[hi_start + edge] - csum[hi_start]) / edge as f64;
        ok &= m < 0.25 * inside;
    }
    Some((s, inside, ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_averaged_spectrum_finds_a_tone_and_ignores_bad_samples() {
        let n = 1024;
        let mut s = AvgSpectrum::new(BandFft::N1024);
        let mut x: Vec<Complex32> = (0..4 * n)
            .map(|i| Complex32::from_polar(1.0, std::f32::consts::TAU * 32.0 * i as f32 / n as f32))
            .collect();
        x[5] = Complex32::new(f32::NAN, 0.0);
        s.add_all(&x);
        assert_eq!(s.frames, 4);
        let m = s.mean();
        let peak = (0..n).max_by(|&a, &b| m[a].total_cmp(&m[b])).unwrap_or(0);
        assert_eq!(peak, n / 2 + 32);
        assert!(m.iter().all(|v| v.is_finite()));
    }
}
