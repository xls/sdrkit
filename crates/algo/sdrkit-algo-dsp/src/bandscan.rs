//! An averaged spectrum of a complex stream with the measures that identify a signal in it: the noise floor (the median bin of a
//! band), the peak of a band refined by a parabola, the occupied band (the widest run above a threshold, short gaps bridged), the
//! power of a band and its SNR.
//!
//! Frames of `n` points with a Hann window overlap by half. The first four frames are averaged equally, the later ones with a
//! weight `a` for the new frame. Bin `k` lies at `k rate / n` Hz for `k < n / 2` and at `(k - n) rate / n` Hz above.
//!
//! ```
//! use sdrkit_algo_dsp::bandscan::BandScan;
//! use num_complex::Complex32;
//! let mut s = BandScan::new(1024, 8000.0, 0.25).unwrap();
//! let x: Vec<Complex32> = (0..8192)
//!     .map(|i| Complex32::from_polar(1.0, (std::f64::consts::TAU * 1000.0 * i as f64 / 8000.0) as f32))
//!     .collect();
//! s.push(&x);
//! let (f, _) = s.peak(500.0, 1500.0).unwrap();
//! assert!((f - 1000.0).abs() < 1.0);
//! ```

use crate::error::{DspError, Param};
use crate::fft::WindowedFft;
use crate::window::Window;
use num_complex::Complex32;

/// The averaged spectrum. See the module documentation.
pub struct BandScan {
    n: usize,
    rate: f64,
    alpha: f32,
    fft: WindowedFft,
    ring: Vec<Complex32>,
    at: usize,
    fill: usize,
    since: usize,
    frame_buf: Vec<Complex32>,
    avg: Vec<f32>,
    frames: u64,
    scratch: Vec<f32>,
    smooth: Vec<f32>,
}

impl BandScan {
    /// Frames of `n` points (a power of two, 16 to 65536) at `rate`, the new frame weighted `alpha` (0 to 1) after the first four.
    pub fn new(n: usize, rate: f64, alpha: f32) -> Result<Self, DspError> {
        if !n.is_power_of_two() || !(16..=65536).contains(&n) {
            return Param::FftSize.range(16.0, 65536.0, n as f64);
        }
        if rate.is_nan() || rate <= 0.0 {
            return Param::InputRate.positive(rate);
        }
        if !(alpha > 0.0 && alpha <= 1.0) {
            return Param::Alpha.range(0.0, 1.0, f64::from(alpha));
        }
        Ok(BandScan {
            n,
            rate,
            alpha,
            fft: WindowedFft::new(n, n, Window::Hann)?,
            ring: vec![Complex32::default(); n],
            at: 0,
            fill: 0,
            since: 0,
            frame_buf: Vec::with_capacity(n),
            avg: vec![0.0; n],
            frames: 0,
            scratch: Vec::new(),
            smooth: Vec::new(),
        })
    }

    /// Forgets the average.
    pub fn reset(&mut self) {
        self.fill = 0;
        self.at = 0;
        self.since = 0;
        self.frames = 0;
        self.avg.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Takes samples; returns the frames completed.
    pub fn push(&mut self, x: &[Complex32]) -> usize {
        let mut done = 0;
        for &v in x {
            let v = if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            };
            self.ring[self.at] = v;
            self.at = (self.at + 1) % self.n;
            self.fill = (self.fill + 1).min(self.n);
            self.since += 1;
            if self.fill == self.n && self.since >= self.n / 2 {
                self.since = 0;
                self.frame();
                done += 1;
            }
        }
        done
    }

    fn frame(&mut self) {
        self.frame_buf.clear();
        self.frame_buf.extend_from_slice(&self.ring[self.at..]);
        self.frame_buf.extend_from_slice(&self.ring[..self.at]);
        let p = self.fft.run_power(&self.frame_buf);
        self.frames += 1;
        let w = if self.frames <= 4 {
            1.0 / self.frames as f32
        } else {
            self.alpha
        };
        for (a, &v) in self.avg.iter_mut().zip(p) {
            *a += w * (v - *a);
        }
    }

    /// Frames averaged so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The averaged power of each bin.
    pub fn power(&self) -> &[f32] {
        &self.avg
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }

    /// The bin nearest `hz` (negative frequencies wrap).
    pub fn bin(&self, hz: f64) -> usize {
        let k = (hz / self.bin_hz()).round() as i64;
        k.rem_euclid(self.n as i64) as usize
    }

    /// The frequency of bin `k`, Hz.
    pub fn freq(&self, k: usize) -> f64 {
        let k = if k >= self.n / 2 {
            k as f64 - self.n as f64
        } else {
            k as f64
        };
        k * self.bin_hz()
    }

    /// The bins of `lo_hz ..= hi_hz` in frequency order.
    fn bins(&self, lo_hz: f64, hi_hz: f64) -> impl Iterator<Item = usize> + '_ {
        let a = (lo_hz / self.bin_hz()).ceil() as i64;
        let b = (hi_hz / self.bin_hz()).floor() as i64;
        let n = self.n as i64;
        (a..=b).map(move |k| k.rem_euclid(n) as usize)
    }

    /// The noise floor: the median bin of `lo_hz ..= hi_hz`.
    pub fn noise(&mut self, lo_hz: f64, hi_hz: f64) -> f32 {
        let v: Vec<f32> = self.bins(lo_hz, hi_hz).map(|k| self.avg[k]).collect();
        if v.is_empty() {
            return 0.0;
        }
        crate::spectrum::percentile(&v, 0.5, &mut self.scratch)
    }

    /// The strongest bin of `lo_hz ..= hi_hz` refined by a parabola through it and its neighbours: the frequency and the power.
    pub fn peak(&self, lo_hz: f64, hi_hz: f64) -> Option<(f64, f32)> {
        let k = self
            .bins(lo_hz, hi_hz)
            .max_by(|&a, &b| self.avg[a].total_cmp(&self.avg[b]))?;
        let p = self.avg[k];
        let l = self.avg[(k + self.n - 1) % self.n];
        let r = self.avg[(k + 1) % self.n];
        let den = l - 2.0 * p + r;
        let d = if den.abs() > 1e-30 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some((self.freq(k) + f64::from(d) * self.bin_hz(), p))
    }

    /// The power at `hz` (the nearest bin).
    pub fn at(&self, hz: f64) -> f32 {
        self.avg[self.bin(hz)]
    }

    /// The summed power of `lo_hz ..= hi_hz`.
    pub fn band_power(&self, lo_hz: f64, hi_hz: f64) -> f64 {
        self.bins(lo_hz, hi_hz)
            .map(|k| f64::from(self.avg[k]))
            .sum()
    }

    /// The number of bins of `lo_hz ..= hi_hz`.
    pub fn band_bins(&self, lo_hz: f64, hi_hz: f64) -> usize {
        self.bins(lo_hz, hi_hz).count()
    }

    /// The SNR of `lo_hz ..= hi_hz` over the noise floor `noise` per bin, dB.
    pub fn snr_db(&self, lo_hz: f64, hi_hz: f64, noise: f32) -> f64 {
        let n = f64::from(noise.max(1e-30)) * self.band_bins(lo_hz, hi_hz) as f64;
        10.0 * ((self.band_power(lo_hz, hi_hz) - n).max(1e-30) / n).log10()
    }

    /// The occupied band of `lo_hz ..= hi_hz`: bins smoothed over 9, the widest run above `ratio` times `noise`, gaps up to `gap_hz`
    /// bridged, at least `min_hz` wide. Returns its edges, Hz.
    pub fn occupied(
        &mut self,
        lo_hz: f64,
        hi_hz: f64,
        noise: f32,
        ratio: f32,
        gap_hz: f64,
        min_hz: f64,
    ) -> Option<(f64, f64)> {
        let bins: Vec<usize> = self.bins(lo_hz, hi_hz).collect();
        if bins.len() < 9 {
            return None;
        }
        self.smooth.clear();
        for i in 0..bins.len() {
            let a = i.saturating_sub(4);
            let b = (i + 4).min(bins.len() - 1);
            let s: f32 = bins[a..=b].iter().map(|&k| self.avg[k]).sum();
            self.smooth.push(s / (b - a + 1) as f32);
        }
        let th = ratio * noise;
        let gap = (gap_hz / self.bin_hz()).round() as usize;
        let mut best: Option<(usize, usize)> = None;
        let mut start: Option<usize> = None;
        let mut last = 0usize;
        for i in 0..=bins.len() {
            let on = i < bins.len() && self.smooth[i] > th;
            if on {
                if start.is_none() || i - last > gap + 1 {
                    if let Some(s) = start {
                        if best.is_none_or(|b| last - s > b.1 - b.0) {
                            best = Some((s, last));
                        }
                    }
                    start = Some(i);
                }
                last = i;
            }
        }
        if let Some(s) = start {
            if best.is_none_or(|b| last - s > b.1 - b.0) {
                best = Some((s, last));
            }
        }
        let (a, b) = best?;
        let (fa, fb) = (self.freq(bins[a]), self.freq(bins[b]));
        (fb - fa >= min_hz).then_some((fa, fb))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_band_of_noise_is_found_with_its_snr() {
        let mut s = BandScan::new(1024, 12_000.0, 0.2).unwrap();
        let mut r = 1u32;
        let mut g = || {
            r = r.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (r >> 8) as f32 / 16_777_216.0 - 0.5
        };
        // Wide noise plus a band from 1000 to 2000 Hz made of tones 10 Hz apart.
        let x: Vec<Complex32> = (0..48_000)
            .map(|i| {
                let t = i as f64 / 12_000.0;
                let mut v = Complex32::new(0.3 * g(), 0.3 * g());
                for k in 0..101 {
                    let f = 1000.0 + 10.0 * k as f64;
                    v += Complex32::from_polar(
                        0.05,
                        (std::f64::consts::TAU * f * t + k as f64 * 1.3) as f32,
                    );
                }
                v
            })
            .collect();
        s.push(&x);
        let n = s.noise(100.0, 5900.0);
        let (a, b) = s.occupied(100.0, 5900.0, n, 6.0, 70.0, 300.0).unwrap();
        assert!(
            (a - 1000.0).abs() < 80.0 && (b - 2000.0).abs() < 80.0,
            "{a} {b}"
        );
        assert!(s.snr_db(1000.0, 2000.0, n) > 20.0);
    }
}
