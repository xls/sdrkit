//! Short-frame spectra of a complex stream: every `n / 2` new samples, the Hann-windowed FFT of the last `n` samples (oldest
//! first), with the bin powers `|X|^2` in signed order and their median.
//!
//! A single-frame bin of complex Gaussian noise is exponential, so the median of a frame of noise is `ln 2 = 0.693` of the mean
//! noise per bin: a threshold of `D` dB over the median is `D - 1.6` dB over the mean noise. A frame lasts `n` samples and
//! frames step by `n / 2`, so a burst of a few frame steps is seen in several frames.
//!
//! ```
//! use sdrkit_algo_dsp::shortframe::ShortFrames;
//! use num_complex::Complex32;
//! let mut s = ShortFrames::new(256).unwrap();
//! let x: Vec<Complex32> = (0..4096).map(|i| Complex32::from_polar(1.0, 0.25 * std::f32::consts::PI * i as f32)).collect();
//! let mut peak = 0;
//! s.push(&x, |f| peak = f.power.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).unwrap().0);
//! assert_eq!(peak as i64 - 128, 32);
//! ```

use crate::fft::Fft;
use crate::window::Window;
use crate::{DspError, Param};
use num_complex::Complex32;

/// One frame of a [`ShortFrames`].
#[derive(Debug)]
pub struct Frame<'a> {
    /// The frame number from 0. Frame `k` ends at input sample `n + k n / 2` (exclusive).
    pub index: u64,
    /// The bin powers `|X|^2` in signed order: element `i` is bin `i - n / 2`, at `(i - n / 2) rate / n` Hz.
    pub power: &'a [f32],
    /// The median of the powers (the upper middle element).
    pub median: f32,
}

impl Frame<'_> {
    /// The power of signed bin `j` (from `-n / 2` to `n / 2 - 1`).
    pub fn bin(&self, j: i64) -> f32 {
        self.power[(j + self.power.len() as i64 / 2) as usize]
    }
}

/// Short-frame spectra (module docs).
#[derive(Clone, Debug)]
pub struct ShortFrames {
    n: usize,
    hist: Vec<Complex32>,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    power: Vec<f32>,
    scratch: Vec<f32>,
    frames: u64,
}

impl ShortFrames {
    /// Frames of `n` points, a power of two from 8 to 65536.
    ///
    /// # Errors
    ///
    /// [`DspError::OutOfRange`] for another `n`.
    pub fn new(n: usize) -> Result<Self, DspError> {
        if !n.is_power_of_two() || !(8..=65_536).contains(&n) {
            return Param::FftSize.range(8.0, 65_536.0, n as f64);
        }
        Ok(ShortFrames {
            n,
            hist: Vec::with_capacity(2 * n),
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            buf: vec![Complex32::default(); n],
            power: vec![0.0; n],
            scratch: Vec::with_capacity(n),
            frames: 0,
        })
    }

    /// The frame length, points.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Never true: a frame has at least 8 points.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The frame step, samples (`n / 2`).
    pub fn hop(&self) -> usize {
        self.n / 2
    }

    /// The frames made so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Forgets the held samples; the next frame needs `n` new samples. Frame numbers go on.
    pub fn reset(&mut self) {
        self.hist.clear();
    }

    /// Takes `x` and calls `f` for each frame that it completes.
    pub fn push(&mut self, x: &[Complex32], mut f: impl FnMut(&Frame)) {
        let n = self.n;
        let hop = n / 2;
        let mut rest = x;
        while !rest.is_empty() {
            let take = (n - self.hist.len()).min(rest.len());
            self.hist.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.hist.len() < n {
                break;
            }
            for ((b, s), w) in self.buf.iter_mut().zip(&self.hist).zip(&self.win) {
                *b = s * *w;
            }
            self.fft.process(&mut self.buf);
            for (i, p) in self.power.iter_mut().enumerate() {
                *p = self.buf[(i + hop) % n].norm_sqr();
            }
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.power);
            let (_, m, _) = self.scratch.select_nth_unstable_by(hop, f32::total_cmp);
            let median = *m;
            f(&Frame {
                index: self.frames,
                power: &self.power,
                median,
            });
            self.frames += 1;
            self.hist.drain(..hop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u64) -> Vec<Complex32> {
        // Box-Muller on a 64-bit LCG: unit mean power.
        let mut s = seed;
        let mut u = || {
            s = s
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| {
                let r = (-u().ln()).sqrt();
                let a = std::f64::consts::TAU * u();
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn a_tone_in_noise_gives_its_bin_and_frames_step_by_half() {
        let mut s = ShortFrames::new(256).unwrap();
        let mut x = noise(256 * 40, 3);
        for (i, z) in x.iter_mut().enumerate() {
            *z += Complex32::from_polar(3.0, -std::f32::consts::TAU * 37.0 * i as f32 / 256.0);
        }
        let mut peaks = Vec::new();
        // Pieces of odd length: the frame count does not depend on them.
        for c in x.chunks(77) {
            s.push(c, |f| {
                let k = f
                    .power
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0;
                peaks.push(k as i64 - 128);
                assert!(f.bin(-37) > 100.0 * f.median);
            });
        }
        assert_eq!(peaks.len(), 79);
        assert!(peaks.iter().all(|&k| k == -37), "{peaks:?}");
    }

    #[test]
    fn the_median_of_white_noise_frames_is_0_693_of_the_mean() {
        let mut s = ShortFrames::new(256).unwrap();
        let x = noise(128 * 1001, 9);
        let (mut med, mut mean, mut k) = (0.0f64, 0.0f64, 0);
        s.push(&x, |f| {
            med += f64::from(f.median);
            mean += f.power.iter().map(|&p| f64::from(p)).sum::<f64>() / 256.0;
            k += 1;
        });
        assert_eq!(k, 1000);
        let r = med / mean;
        assert!((r / std::f64::consts::LN_2 - 1.0).abs() < 0.03, "{r}");
    }
}
