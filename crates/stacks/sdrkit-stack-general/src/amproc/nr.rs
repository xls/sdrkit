//! The noise reduction of the AM processor: minimum statistics noise tracking (Martin 2001) and the log-spectral amplitude gain
//! (Ephraim and Malah) on short-time spectra of 1024 with a hop of 512. The transform always runs, so the delay never changes.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_core::Complex32;

const N: usize = 1024;
const HOP: usize = 512;
const BINS: usize = N / 2 + 1;
const SUB: usize = 8;
const SUB_LEN: u32 = 12;

/// The exponential integral E1(x) for x > 0 (Abramowitz and Stegun 5.1.53 and 5.1.56).
pub fn expint_e1(x: f64) -> f64 {
    if x <= 0.0 {
        return 50.0;
    }
    if x <= 1.0 {
        -x.ln() - 0.577_215_66
            + x * (0.999_991_93
                + x * (-0.249_910_55 + x * (0.055_199_68 + x * (-0.009_760_04 + x * 0.001_078_57))))
    } else {
        let num = x * x + 2.334_733 * x + 0.250_621;
        let den = x * x + 3.330_657 * x + 1.681_534;
        num / den / (x * x.exp())
    }
}

/// The noise reduction.
pub struct LsaNoiseReduction {
    rate: f64,
    fft: Fft,
    ifft: Fft,
    win: Vec<f32>,
    inbuf: Vec<f32>,
    fill: usize,
    outacc: Vec<f32>,
    outq: Vec<f32>,
    outpos: usize,
    spec: Vec<Complex32>,
    smooth: Vec<f32>,
    sub_min: Vec<f32>,
    cur_min: Vec<f32>,
    sub_i: usize,
    sub_n: u32,
    noise: Vec<f32>,
    prev_amp2: Vec<f32>,
    gain: Vec<f32>,
    g2: Vec<f32>,
    frames: u64,
    scratch: Vec<f32>,
    /// The gain floor (dB of reduction allowed, 0 = off).
    pub floor_db: f32,
    /// The upper edge of the programme (Hz) for the noise ceiling.
    pub bandwidth: f64,
}

impl LsaNoiseReduction {
    /// At `rate`.
    pub fn new(rate: f64) -> Self {
        let win: Vec<f32> = (0..N)
            .map(|i| {
                (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos()).sqrt() as f32
            })
            .collect();
        LsaNoiseReduction {
            rate,
            fft: Fft::forward(N),
            ifft: Fft::inverse(N),
            win,
            inbuf: vec![0.0; N],
            fill: N - HOP,
            outacc: vec![0.0; N],
            outq: vec![0.0; HOP],
            outpos: 0,
            spec: vec![Complex32::default(); N],
            smooth: vec![0.0; BINS],
            sub_min: vec![f32::MAX; SUB * BINS],
            cur_min: vec![f32::MAX; BINS],
            sub_i: 0,
            sub_n: 0,
            noise: vec![0.0; BINS],
            prev_amp2: vec![0.0; BINS],
            gain: vec![1.0; BINS],
            g2: vec![1.0; BINS],
            frames: 0,
            scratch: Vec::with_capacity(BINS),
            floor_db: 0.0,
            bandwidth: 4500.0,
        }
    }
    /// The delay in samples.
    pub fn delay(&self) -> usize {
        N
    }
    fn frame(&mut self) {
        for i in 0..N {
            self.spec[i] = Complex32::new(self.inbuf[i] * self.win[i], 0.0);
        }
        self.fft.process(&mut self.spec);
        self.frames += 1;
        let first = self.frames == 1;
        // Smoothed periodogram and minimum statistics.
        for k in 0..BINS {
            let p = self.spec[k].norm_sqr();
            self.smooth[k] = if first {
                p
            } else {
                0.85 * self.smooth[k] + 0.15 * p
            };
            self.cur_min[k] = self.cur_min[k].min(self.smooth[k]);
        }
        self.sub_n += 1;
        if self.sub_n >= SUB_LEN || first {
            let base = self.sub_i * BINS;
            self.sub_min[base..base + BINS].copy_from_slice(&self.cur_min);
            if !first {
                self.sub_i = (self.sub_i + 1) % SUB;
                self.cur_min.iter_mut().for_each(|v| *v = f32::MAX);
                self.sub_n = 0;
            }
        }
        for k in 0..BINS {
            let mut m = self.cur_min[k];
            for s in 0..SUB {
                m = m.min(self.sub_min[s * BINS + k]);
            }
            self.noise[k] = m;
        }
        // A held note is not noise: the noise stays below 1.2 x the 20th percentile of the smoothed power in the programme band.
        let b = self.rate / N as f64;
        let (k0, k1) = (
            ((300.0 / b) as usize).max(1),
            ((self.bandwidth / b) as usize).clamp(2, BINS - 1),
        );
        let ceiling = 1.2 * percentile(&self.smooth[k0..=k1.max(k0)], 0.2, &mut self.scratch);
        let gmin = 10f32.powf(-self.floor_db / 20.0);
        for k in 0..BINS {
            let lambda = self.noise[k].min(ceiling).max(1e-20);
            let p = self.spec[k].norm_sqr();
            let gamma = (p / lambda).min(1e4);
            let xi = if first {
                (gamma - 1.0).max(0.0)
            } else {
                0.98 * self.prev_amp2[k] / lambda + 0.02 * (gamma - 1.0).max(0.0)
            }
            .max(1e-4);
            let v = (xi * gamma / (1.0 + xi)) as f64;
            let g = (xi / (1.0 + xi)) as f64 * (0.5 * expint_e1(v)).exp();
            let g = (g as f32).clamp(gmin, 1.0);
            self.gain[k] = g;
            self.prev_amp2[k] = g * g * p;
        }
        // Gains below 0.5 are smoothed across neighbours.
        self.g2.copy_from_slice(&self.gain);
        for k in 1..BINS - 1 {
            if self.gain[k] < 0.5 {
                self.g2[k] =
                    (0.25 * self.gain[k - 1] + 0.5 * self.gain[k] + 0.25 * self.gain[k + 1])
                        .max(gmin);
            }
        }
        if self.floor_db <= 0.0 {
            self.g2.iter_mut().for_each(|g| *g = 1.0);
        }
        for k in 0..BINS {
            let g = self.g2[k];
            self.spec[k] *= g;
            if k > 0 && k < N / 2 {
                self.spec[N - k] *= g;
            }
        }
        self.ifft.process(&mut self.spec);
        let scale = 1.0 / N as f32;
        for i in 0..N {
            self.outacc[i] += self.spec[i].re * scale * self.win[i];
        }
        // The first HOP samples are complete.
        self.outq.copy_from_slice(&self.outacc[..HOP]);
        self.outacc.copy_within(HOP.., 0);
        self.outacc[N - HOP..].iter_mut().for_each(|v| *v = 0.0);
        self.inbuf.copy_within(HOP.., 0);
        self.outpos = 0;
    }
    /// One sample in, one (delayed by [`delay`](Self::delay)) out.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.inbuf[self.fill] = x;
        self.fill += 1;
        let y = self.outq[self.outpos];
        self.outpos += 1;
        if self.fill == N {
            self.frame();
            self.fill = N - HOP;
        }
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_values() {
        assert!((expint_e1(1.0) - 0.219_383_9).abs() < 1e-5);
        assert!((expint_e1(0.1) - 1.822_923_9).abs() < 1e-5);
        assert!((expint_e1(5.0) - 0.001_148_3).abs() < 1e-5);
    }

    #[test]
    fn off_is_a_pure_delay() {
        let mut nr = LsaNoiseReduction::new(48e3);
        let x: Vec<f32> = (0..20_000)
            .map(|i| ((i as f32) * 0.05).sin() * 0.3)
            .collect();
        let y: Vec<f32> = x.iter().map(|&v| nr.push(v)).collect();
        let d = nr.delay();
        for i in d + 2000..x.len() {
            assert!((y[i] - x[i - d]).abs() < 1e-4, "{i}: {} {}", y[i], x[i - d]);
        }
    }
}
