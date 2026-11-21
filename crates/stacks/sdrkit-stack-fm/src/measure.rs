//! The MPX spectrum: the SNR measure (noise above all programme content) and the multiplex scope.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// Points of the MPX FFT.
pub const MPX_FFT: usize = 2048;
/// The upper frequency of the multiplex scope.
pub const MPX_TOP_HZ: f64 = 80e3;

/// The averaged MPX power spectrum, from the last 2048 MPX samples at each report.
#[derive(Clone, Debug)]
pub struct MpxSpectrum {
    rate: f64,
    fft: Fft,
    win: Vec<f32>,
    win_sum: f64,
    win_sq: f64,
    ring: Vec<f32>,
    pos: usize,
    buf: Vec<Complex32>,
    avg: Vec<f64>,
    seeded: bool,
}

impl MpxSpectrum {
    /// A spectrum for MPX samples at `rate`.
    pub fn new(rate: f64) -> Self {
        let win = Window::Hann.coefficients(MPX_FFT);
        let win_sum = win.iter().map(|&w| w as f64).sum();
        let win_sq = win.iter().map(|&w| (w as f64) * (w as f64)).sum();
        MpxSpectrum {
            rate,
            fft: Fft::forward(MPX_FFT),
            win,
            win_sum,
            win_sq,
            ring: vec![0.0; MPX_FFT],
            pos: 0,
            buf: vec![Complex32::default(); MPX_FFT],
            avg: vec![0.0; MPX_FFT / 2 + 1],
            seeded: false,
        }
    }

    /// One MPX sample.
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.ring[self.pos] = x;
        self.pos = (self.pos + 1) % MPX_FFT;
    }

    /// MPX samples.
    pub fn process(&mut self, x: &[f32]) {
        for &v in x {
            self.push(v);
        }
    }

    /// Transforms the last 2048 samples and folds them into the average (smoothing 0.3; the first spectrum seeds it).
    pub fn update(&mut self) {
        for i in 0..MPX_FFT {
            let x = self.ring[(self.pos + i) % MPX_FFT];
            self.buf[i] = Complex32::new(x * self.win[i], 0.0);
        }
        self.fft.process(&mut self.buf);
        for (a, b) in self.avg.iter_mut().zip(&self.buf) {
            let p = b.norm_sqr() as f64;
            *a = if self.seeded { *a + 0.3 * (p - *a) } else { p };
        }
        self.seeded = true;
    }

    fn bin_hz(&self) -> f64 {
        self.rate / MPX_FFT as f64
    }

    /// The mean-square MPX noise in 85 to 110 kHz.
    pub fn band_noise(&self, lo: f64, hi: f64) -> f64 {
        let bw = self.bin_hz();
        let (a, b) = (
            (lo / bw).ceil() as usize,
            ((hi / bw).floor() as usize).min(MPX_FFT / 2),
        );
        let s: f64 = self.avg[a..=b].iter().sum();
        2.0 * s / (MPX_FFT as f64 * self.win_sq)
    }

    /// The SNR measure in dB: the noise in 85 to 110 kHz scaled to the 0 to 15 kHz audio band (discriminator noise rises with f^2),
    /// against 0.15^2. Clamped to 0 to 60.
    pub fn snr_db(&self) -> f32 {
        let hi = 110e3f64.min(self.rate / 2.0 - self.bin_hz());
        let noise = self.band_noise(85e3, hi) * 15e3f64.powi(3) / (hi.powi(3) - 85e3f64.powi(3));
        let snr = 10.0 * (0.0225 / noise.max(1e-30)).log10();
        snr.clamp(0.0, 60.0) as f32
    }

    /// [`points`](Self::points) into `out` (at most `out.len()` of them); returns how many.
    pub fn points_into(&self, out: &mut [[f32; 2]]) -> usize {
        let bw = self.bin_hz();
        let full = (self.win_sum / 2.0).powi(2);
        let n = (((MPX_TOP_HZ / bw).floor() as usize).min(MPX_FFT / 2) + 1).min(out.len());
        for (k, o) in out[..n].iter_mut().enumerate() {
            *o = [
                (k as f64 * bw) as f32,
                (10.0 * (self.avg[k] / full).max(1e-12).log10()).max(-120.0) as f32,
            ];
        }
        n
    }

    /// The multiplex scope: (Hz, dB against a full-deviation sine), 0 to 80 kHz, floored at -120 dB.
    pub fn points(&self) -> Vec<[f32; 2]> {
        let bw = self.bin_hz();
        let full = (self.win_sum / 2.0).powi(2);
        let n = ((MPX_TOP_HZ / bw).floor() as usize).min(MPX_FFT / 2);
        (0..=n)
            .map(|k| {
                [
                    (k as f64 * bw) as f32,
                    (10.0 * (self.avg[k] / full).max(1e-12).log10()).max(-120.0) as f32,
                ]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn a_full_deviation_sine_reads_0_db_and_noise_reads_its_level() {
        let rate = 250e3;
        let mut s = MpxSpectrum::new(rate);
        let f = 100.0 * rate / MPX_FFT as f64;
        for i in 0..MPX_FFT {
            s.push((TAU * f * i as f64 / rate).sin() as f32);
        }
        s.update();
        let pts = s.points();
        let peak = pts.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(peak.abs() < 0.1, "{peak}");
        // White noise of variance v: the band 85..110 kHz holds v * 25 / 125.
        let mut s = MpxSpectrum::new(rate);
        let mut r = sdrkit_algo_sim::rng::Rng::new(2);
        for _ in 0..40 {
            for _ in 0..MPX_FFT {
                s.push(0.01 * r.normal() as f32);
            }
            s.update();
        }
        let n = s.band_noise(85e3, 110e3);
        let want = 1e-4 * 25.0 / 125.0;
        assert!((n / want - 1.0).abs() < 0.2, "{n} against {want}");
    }
}
