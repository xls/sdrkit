//! The modulation probe of the automatic `atv` receiver: in short windows of the input it measures how strongly the
//! envelope and the instantaneous frequency repeat from one television line to the next. AM television repeats in the
//! envelope; FM video has a flat envelope and repeats in the frequency. Noise repeats in neither.

use super::Modulation;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_core::Complex32;

/// The rate of the averaged points, Hz.
const POINT_HZ: f64 = 2e6;
/// The length of a window, s (about 300 lines).
const WINDOW_S: f64 = 0.02;
/// The time from the start of one window to the start of the next, s.
const EVERY_S: f64 = 0.25;
/// The line periods tried: 625 and 525 lines, s.
const LINES_S: [f64; 2] = [64e-6, 1.0 / 15_734.264];
/// The line correlation at which a detected signal carries television lines.
const LINES_AT: f32 = 0.5;
/// How much more the line correlation of one detected signal must be than that of the other.
const MARGIN: f32 = 0.2;

/// The line correlations of one window: 1 for a signal that repeats exactly every line, about 0 for noise.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LineCorrelation {
    /// Of the envelope.
    pub envelope: f32,
    /// Of the instantaneous frequency.
    pub frequency: f32,
}

impl LineCorrelation {
    /// The modulation that carries the lines: [`Modulation::Am`] when the envelope does, [`Modulation::FmVideo`] when
    /// the frequency does and the envelope does not. `None` when neither does clearly (noise, no television).
    pub fn modulation(self) -> Option<Modulation> {
        let (e, f) = (self.envelope, self.frequency);
        if f >= LINES_AT && f >= e + MARGIN {
            Some(Modulation::FmVideo)
        } else if e >= LINES_AT && e >= f + MARGIN {
            Some(Modulation::Am)
        } else {
            None
        }
    }
}

/// The probe. It reads one window of [`WINDOW_S`] in every [`EVERY_S`] of input.
#[derive(Clone, Debug)]
pub struct ModulationProbe {
    /// Input samples per point.
    k: usize,
    /// Points per window.
    len: usize,
    /// Input samples from the start of one window to the next.
    every: usize,
    /// The lags of the line periods, points.
    lags: Vec<usize>,
    /// Input samples to skip before the next window.
    skip: usize,
    prev: Complex32,
    sum_e: f32,
    sum_f: f32,
    n: usize,
    env: Vec<f32>,
    freq: Vec<f32>,
}

impl ModulationProbe {
    /// A probe for input at `rate_hz`.
    pub fn new(rate_hz: f64) -> Self {
        let k = ((rate_hz / POINT_HZ).round() as usize).max(1);
        let fp = rate_hz / k as f64;
        let len = (WINDOW_S * fp).round() as usize;
        ModulationProbe {
            k,
            len,
            every: (EVERY_S * rate_hz).round() as usize,
            lags: LINES_S.iter().map(|t| (t * fp).round() as usize).collect(),
            skip: 0,
            prev: Complex32::default(),
            sum_e: 0.0,
            sum_f: 0.0,
            n: 0,
            env: Vec::with_capacity(len),
            freq: Vec::with_capacity(len),
        }
    }

    /// Starts again with the next sample (after a retune).
    pub fn reset(&mut self) {
        self.skip = 0;
        self.clear();
    }

    fn clear(&mut self) {
        self.env.clear();
        self.freq.clear();
        self.sum_e = 0.0;
        self.sum_f = 0.0;
        self.n = 0;
    }

    /// Reads `iq`. Returns the line correlations of a window that ends in it.
    pub fn feed(&mut self, iq: &[Complex32]) -> Option<LineCorrelation> {
        let mut i = 0;
        let mut found = None;
        while i < iq.len() {
            if self.skip > 0 {
                let n = self.skip.min(iq.len() - i);
                self.skip -= n;
                i += n;
                if self.skip == 0 && i > 0 {
                    self.prev = iq[i - 1];
                }
                continue;
            }
            let x = iq[i];
            let d = x * self.prev.conj();
            self.prev = x;
            self.sum_e += x.norm();
            self.sum_f += atan2(d.im, d.re);
            self.n += 1;
            i += 1;
            if self.n == self.k {
                self.env.push(self.sum_e);
                self.freq.push(self.sum_f);
                self.sum_e = 0.0;
                self.sum_f = 0.0;
                self.n = 0;
                if self.env.len() == self.len {
                    found = Some(LineCorrelation {
                        envelope: line_correlation(&mut self.env, &self.lags),
                        frequency: line_correlation(&mut self.freq, &self.lags),
                    });
                    self.clear();
                    self.skip = self.every.saturating_sub(self.len * self.k);
                }
            }
        }
        found
    }
}

/// The largest normalised correlation of `x` with itself one line later, over the lags `lags` and one point either side.
/// `x` loses its mean.
fn line_correlation(x: &mut [f32], lags: &[usize]) -> f32 {
    let n = x.len();
    let mean = x.iter().sum::<f32>() / n.max(1) as f32;
    x.iter_mut().for_each(|v| *v -= mean);
    let mut best = 0f32;
    for &lag in lags {
        for l in lag.saturating_sub(1)..=lag + 1 {
            if l == 0 || l >= n / 2 {
                continue;
            }
            let (mut xy, mut xx, mut yy) = (0f64, 0f64, 0f64);
            for j in 0..n - l {
                let (a, b) = (f64::from(x[j]), f64::from(x[j + l]));
                xy += a * b;
                xx += a * a;
                yy += b * b;
            }
            if xx > 0.0 && yy > 0.0 {
                best = best.max((xy / (xx * yy).sqrt()) as f32);
            }
        }
    }
    best
}
