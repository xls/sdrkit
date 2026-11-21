//! The Goertzel algorithm: one DFT value at any frequency (not only a bin centre) for one multiply and two adds per sample, over a
//! slice ([`goertzel`]), streaming in blocks ([`Goertzel`]), or for many tones in one pass ([`ToneBank`]).
//!
//! The recursion is `s[n] = x[n] + 2 cos(w) s[n-1] - s[n-2]`; after `N` samples `X(w) = sum x[n] e^(-j w n) = e^(-j w (N-1)) (s[N-1] -
//! e^(-j w) s[N-2])` (G. Goertzel, "An algorithm for the evaluation of finite trigonometric series", Amer. Math. Monthly 65, 1958).
//! The state is `f64`: in `f32` the error grows with `N^2` near DC, which a 0.2 s CTCSS window at 8 kHz would feel.
//!
//! The streaming forms return the **amplitude phasor** `2 X / N`: for a real input `A cos(w n + phi)` with a whole number of cycles in
//! the block it is `A e^(j phi)` (phase at the block's first sample), so `|.|` is the tone's amplitude and `|.|^2 / 2` its mean power.
//!
//! ```
//! use sdrkit_algo_dsp::goertzel::goertzel;
//! let x: Vec<f32> = (0..100).map(|n| (std::f64::consts::TAU * 0.1 * n as f64).cos() as f32).collect();
//! let v = goertzel(&x, 0.1);
//! assert!((v.norm() - 50.0).abs() < 1e-4); // N A / 2
//! ```

use crate::{Complex32, DspError, Param};
use num_complex::Complex64;
use std::f64::consts::TAU;

/// `sum x[n] e^(-j 2 pi f n)` over the slice, `f` in cycles per sample.
pub fn goertzel(x: &[f32], f: f64) -> Complex64 {
    let w = TAU * f;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &v in x {
        let s = v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    finish(s1, s2, w, x.len())
}

/// The same over `x[n] w[n]` (a window applied on the fly; the shorter length counts).
pub fn goertzel_windowed(x: &[f32], window: &[f32], f: f64) -> Complex64 {
    let w = TAU * f;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    let n = x.len().min(window.len());
    for (&v, &g) in x.iter().zip(window) {
        let s = (v * g) as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    finish(s1, s2, w, n)
}

/// `|goertzel(x, f)|^2`, without the phase rotation.
pub fn goertzel_power(x: &[f32], f: f64) -> f64 {
    let c = 2.0 * (TAU * f).cos();
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    for &v in x {
        let s = v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

#[inline]
fn finish(s1: f64, s2: f64, w: f64, n: usize) -> Complex64 {
    if n == 0 {
        return Complex64::default();
    }
    let y = Complex64::new(s1 - w.cos() * s2, w.sin() * s2);
    y * Complex64::from_polar(1.0, -w * (n - 1) as f64)
}

fn check(f: f64, n: usize) -> Result<(), DspError> {
    if !(f.is_finite() && f.abs() <= 0.5) {
        return Param::Frequency.range(-0.5, 0.5, f);
    }
    if n == 0 {
        return Param::BlockLen.at_least(1.0, 0.0);
    }
    Ok(())
}

/// A streaming Goertzel filter over blocks of `n` samples. Allocates nothing.
#[derive(Clone, Debug)]
pub struct Goertzel {
    c: f64,
    /// `e^(-j w)` parts.
    cw: f64,
    sw: f64,
    /// `2/N e^(-j w (N-1))`.
    rot: Complex64,
    s1: f64,
    s2: f64,
    n: usize,
    k: usize,
}

impl Goertzel {
    /// At `f` cycles per sample (`|f| <= 0.5`), over blocks of `n` samples.
    pub fn new(f: f64, n: usize) -> Result<Self, DspError> {
        check(f, n)?;
        let w = TAU * f;
        Ok(Goertzel {
            c: 2.0 * w.cos(),
            cw: w.cos(),
            sw: w.sin(),
            rot: Complex64::from_polar(2.0 / n as f64, -w * (n - 1) as f64),
            s1: 0.0,
            s2: 0.0,
            n,
            k: 0,
        })
    }

    /// At `freq` Hz for samples at `rate` Hz.
    pub fn with_rate(rate: f64, freq: f64, n: usize) -> Result<Self, DspError> {
        if rate.is_nan() || rate <= 0.0 {
            return Param::Rate.positive(rate);
        }
        Self::new(freq / rate, n)
    }

    /// The block length.
    pub fn block(&self) -> usize {
        self.n
    }

    /// Starts the block again.
    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
        self.k = 0;
    }

    /// Takes one sample; at the end of each block returns the amplitude phasor `2 X / N` (module docs).
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<Complex32> {
        let s = x as f64 + self.c * self.s1 - self.s2;
        self.s2 = self.s1;
        self.s1 = s;
        self.k += 1;
        if self.k < self.n {
            return None;
        }
        let y = Complex64::new(self.s1 - self.cw * self.s2, self.sw * self.s2) * self.rot;
        self.reset();
        Some(Complex32::new(y.re as f32, y.im as f32))
    }
}

/// Many Goertzel filters over the same blocks of `n` samples, run in one pass: per sample, one multiply-add per tone across arrays
/// that vectorise (DTMF, CTCSS, SELCAL and selcall tone sets).
#[derive(Clone, Debug)]
pub struct ToneBank {
    c: Vec<f64>,
    cw: Vec<f64>,
    sw: Vec<f64>,
    rot: Vec<Complex64>,
    s1: Vec<f64>,
    s2: Vec<f64>,
    out: Vec<Complex32>,
    n: usize,
    k: usize,
}

impl ToneBank {
    /// Tones at `freqs` cycles per sample (each `|f| <= 0.5`), over blocks of `n` samples.
    pub fn new(freqs: &[f64], n: usize) -> Result<Self, DspError> {
        if freqs.is_empty() {
            return Param::Tones.at_least(1.0, 0.0);
        }
        for &f in freqs {
            check(f, n)?;
        }
        let m = freqs.len();
        Ok(ToneBank {
            c: freqs.iter().map(|f| 2.0 * (TAU * f).cos()).collect(),
            cw: freqs.iter().map(|f| (TAU * f).cos()).collect(),
            sw: freqs.iter().map(|f| (TAU * f).sin()).collect(),
            rot: freqs
                .iter()
                .map(|f| Complex64::from_polar(2.0 / n as f64, -TAU * f * (n - 1) as f64))
                .collect(),
            s1: vec![0.0; m],
            s2: vec![0.0; m],
            out: vec![Complex32::default(); m],
            n,
            k: 0,
        })
    }

    /// Tones at `freqs` Hz for samples at `rate` Hz.
    pub fn with_rate(rate: f64, freqs: &[f64], n: usize) -> Result<Self, DspError> {
        if rate.is_nan() || rate <= 0.0 {
            return Param::Rate.positive(rate);
        }
        let norm: Vec<f64> = freqs.iter().map(|f| f / rate).collect();
        Self::new(&norm, n)
    }

    /// The number of tones.
    pub fn len(&self) -> usize {
        self.c.len()
    }

    /// Always false (a bank has at least one tone).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Starts the block again.
    pub fn reset(&mut self) {
        self.s1.fill(0.0);
        self.s2.fill(0.0);
        self.k = 0;
    }

    /// The amplitude phasors of the last complete block, one per tone.
    pub fn amplitudes(&self) -> &[Complex32] {
        &self.out
    }

    /// Feeds samples. After each complete block, `on_block(end, amplitudes)` is called with the index in `x` just past the block's
    /// last sample and the amplitude phasors (`2 X / N`) of every tone. Allocates nothing.
    pub fn process(&mut self, x: &[f32], mut on_block: impl FnMut(usize, &[Complex32])) {
        let mut i = 0;
        while i < x.len() {
            let take = (self.n - self.k).min(x.len() - i);
            for &v in &x[i..i + take] {
                let v = v as f64;
                for ((c, s1), s2) in self
                    .c
                    .iter()
                    .zip(self.s1.iter_mut())
                    .zip(self.s2.iter_mut())
                {
                    let s = v + c * *s1 - *s2;
                    *s2 = *s1;
                    *s1 = s;
                }
            }
            self.k += take;
            i += take;
            if self.k == self.n {
                for j in 0..self.c.len() {
                    let y = Complex64::new(
                        self.s1[j] - self.cw[j] * self.s2[j],
                        self.sw[j] * self.s2[j],
                    ) * self.rot[j];
                    self.out[j] = Complex32::new(y.re as f32, y.im as f32);
                }
                self.reset();
                on_block(i, &self.out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dft(x: &[f32], f: f64) -> Complex64 {
        x.iter()
            .enumerate()
            .map(|(n, &v)| Complex64::from_polar(v as f64, -TAU * f * n as f64))
            .sum()
    }

    #[test]
    fn goertzel_is_the_dft_at_any_frequency() {
        let x: Vec<f32> = (0..1000)
            .map(|n| ((n * 7919) % 1013) as f32 / 1013.0 - 0.5)
            .collect();
        for f in [0.0, 0.001, 0.0123, 0.25, 0.3337, 0.5, -0.17] {
            let g = goertzel(&x, f);
            let d = dft(&x, f);
            assert!((g - d).norm() < 1e-9 * (1.0 + d.norm()), "{f}: {g} vs {d}");
            assert!((goertzel_power(&x, f) - d.norm_sqr()).abs() < 1e-8 * (1.0 + d.norm_sqr()));
        }
        let w: Vec<f32> = crate::window::Window::Hann.coefficients(1000);
        let xw: Vec<f32> = x.iter().zip(&w).map(|(a, b)| a * b).collect();
        assert!((goertzel_windowed(&x, &w, 0.1) - dft(&xw, 0.1)).norm() < 1e-9 * 100.0);
        assert_eq!(goertzel(&[], 0.1), Complex64::default());
    }

    #[test]
    fn streaming_reads_amplitude_and_phase() {
        let (a, phi, f, n) = (0.3, 0.7, 0.05, 400);
        let mut g = Goertzel::new(f, n).unwrap();
        let mut got = Vec::new();
        for i in 0..3 * n {
            let v = (a * (TAU * f * i as f64 + phi).cos()) as f32;
            if let Some(z) = g.push(v) {
                got.push(z);
            }
        }
        assert_eq!(got.len(), 3);
        for (b, z) in got.iter().enumerate() {
            // Each block starts n samples later; 20 cycles per block, so the phase is the same.
            assert!((z.norm() as f64 - a).abs() < 1e-5, "{b}");
            assert!((z.arg() as f64 - phi).abs() < 1e-5, "{b}");
        }
        assert!(Goertzel::new(0.7, 10).is_err());
        assert!(Goertzel::new(0.1, 0).is_err());
    }

    #[test]
    fn tone_bank_decodes_dtmf() {
        // ITU-T Q.23 frequencies; the classic 205-sample block at 8 kHz.
        let rows = [697.0, 770.0, 852.0, 941.0];
        let cols = [1209.0, 1336.0, 1477.0, 1633.0];
        let freqs: Vec<f64> = rows.iter().chain(cols.iter()).copied().collect();
        let mut bank = ToneBank::with_rate(8000.0, &freqs, 205).unwrap();
        // Digit 5 = 770 Hz + 1336 Hz.
        let x: Vec<f32> = (0..205 * 4)
            .map(|n| {
                let t = n as f64 / 8000.0;
                (0.4 * (TAU * 770.0 * t).cos() + 0.4 * (TAU * 1336.0 * t).cos()) as f32
            })
            .collect();
        let mut blocks = 0;
        let mut single = [Goertzel::with_rate(8000.0, 770.0, 205).unwrap()];
        let mut ref_amp = Complex32::default();
        for c in x.chunks(77) {
            for &v in c {
                if let Some(z) = single[0].push(v) {
                    ref_amp = z;
                }
            }
            bank.process(c, |_, amps| {
                blocks += 1;
                let mag: Vec<f32> = amps.iter().map(|z| z.norm()).collect();
                let best_row = (0..4).max_by(|&a, &b| mag[a].total_cmp(&mag[b])).unwrap();
                let best_col = (4..8).max_by(|&a, &b| mag[a].total_cmp(&mag[b])).unwrap();
                assert_eq!((best_row, best_col), (1, 5));
                assert!(mag[1] > 0.35 && mag[5] > 0.35);
            });
        }
        assert_eq!(blocks, 4);
        // The bank and a single filter agree.
        assert!((bank.amplitudes()[1] - ref_amp).norm() < 1e-6);
        assert!(ToneBank::new(&[], 10).is_err());
    }
}
