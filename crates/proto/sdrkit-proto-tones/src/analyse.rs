//! The tone analysis every decoder of this crate runs: a sliding window over real audio, and every `hop` samples the power of
//! each listed tone and of the whole window.

use crate::{Param, TonesError};
use num_complex::Complex64;
use sdrkit_algo_dsp::goertzel::goertzel_windowed;
use sdrkit_algo_dsp::window::Window;

/// One hop's result (see the module documentation).
#[derive(Clone, Copy, Debug)]
pub struct Hop<'a> {
    /// The position (samples since the analyser was made) of the window's first sample.
    pub start: u64,
    /// The mean power of each tone, in the order of the list.
    pub power: &'a [f64],
    /// The mean power of the whole window.
    pub total: f64,
}

impl Hop<'_> {
    /// Tone `k`'s share of the window's power (0 when the window is silent or `k` is past the list).
    pub fn share(&self, k: usize) -> f64 {
        match self.power.get(k) {
            Some(&p) if self.total > 1e-30 => p / self.total,
            _ => 0.0,
        }
    }

    /// The window's level, dB relative to a full-scale sine (`10 log10 (2 total)`).
    pub fn level_db(&self) -> f32 {
        power_db(self.total)
    }
}

/// A mean power as dB relative to a full-scale sine (a sine of amplitude 1 has mean power 1/2: 0 dB).
pub fn power_db(p: f64) -> f32 {
    (10.0 * (2.0 * p).max(1e-20).log10()) as f32
}

/// A level in dB relative to a full-scale sine as a mean power.
pub fn db_power(db: f32) -> f64 {
    10f64.powf(db as f64 / 10.0) / 2.0
}

/// A sliding-window multi-tone analyser.
///
/// A sine of amplitude `A` on a tone has a tone power of `A^2 / 2` with any window, and the window total is scaled the same
/// way. The share of a tone is its power divided by the total, so a decision by share does not change with the input level.
#[derive(Clone, Debug)]
pub struct Analyser {
    ring: Vec<f32>,
    head: usize,
    window: Vec<f32>,
    wsum: f64,
    wsq: f64,
    work: Vec<f32>,
    hop: usize,
    since: usize,
    filled: usize,
    freqs: Vec<f64>,
    power: Vec<f64>,
    total: f64,
    remove_mean: bool,
    position: u64,
}

impl Analyser {
    /// An analyser of `freqs` (cycles per sample, each in `(0, 0.5)`), over windows of `len` samples of `window` shape, every
    /// `hop` samples (`1 <= hop <= len`). With `remove_mean`, the window's mean is taken off first (for DC on a discriminator's
    /// output). Errors for bad values.
    pub fn new(
        freqs: &[f64],
        len: usize,
        hop: usize,
        window: Window,
        remove_mean: bool,
    ) -> Result<Self, TonesError> {
        if freqs.iter().any(|&f| !(f > 0.0 && f < 0.5)) {
            return Err(TonesError::BadParameter(Param::AnalysisTones));
        }
        if len < 4 || hop == 0 || hop > len {
            return Err(TonesError::BadParameter(Param::AnalysisWindow));
        }
        let mut w = vec![0f32; len];
        window.fill_periodic(&mut w);
        let wsum: f64 = w.iter().map(|&v| v as f64).sum();
        let wsq: f64 = w.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / len as f64;
        Ok(Analyser {
            ring: vec![0.0; len],
            head: 0,
            window: w,
            wsum,
            wsq,
            work: vec![0.0; len],
            hop,
            since: 0,
            filled: 0,
            freqs: freqs.to_vec(),
            power: vec![0.0; freqs.len()],
            total: 0.0,
            remove_mean,
            position: 0,
        })
    }

    /// The window length, samples.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Always false (a window holds at least 4 samples).
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The hop, samples.
    pub fn hop(&self) -> usize {
        self.hop
    }

    /// The tones, cycles per sample.
    pub fn freqs(&self) -> &[f64] {
        &self.freqs
    }

    /// The position (samples since the analyser was made) of the next sample.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Forgets the history (the next result comes a whole window later).
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.head = 0;
        self.since = 0;
        self.filled = 0;
    }

    /// Takes one sample (a value that is not a number counts as 0). Returns true when a hop with a full window ends. The result
    /// is then in [`Analyser::last`].
    #[inline]
    pub fn push(&mut self, v: f32) -> bool {
        self.ring[self.head] = if v.is_finite() { v } else { 0.0 };
        self.head = (self.head + 1) % self.ring.len();
        self.position += 1;
        self.filled = (self.filled + 1).min(self.ring.len());
        self.since += 1;
        if self.since >= self.hop && self.filled == self.ring.len() {
            self.since = 0;
            self.analyse();
            return true;
        }
        false
    }

    /// The result of the last hop (zeros before the first).
    pub fn last(&self) -> Hop<'_> {
        Hop {
            start: self.position.saturating_sub(self.ring.len() as u64),
            power: &self.power,
            total: self.total,
        }
    }

    /// The windowed DFT value at `f` cycles per sample over the window of the last hop, as an amplitude phasor. Its magnitude is
    /// the amplitude of a sine at `f`. Its phase is the phase at the window's first sample. Use it for a fine frequency search
    /// or a phase.
    pub fn phasor(&self, f: f64) -> Complex64 {
        goertzel_windowed(&self.work, &self.window, f) * (2.0 / self.wsum.max(1e-30))
    }

    /// The windowed DFT value at `f` over `len` samples of the last window from sample `start` (as [`Analyser::phasor`], with the
    /// phase at sample `start`). Zero when the part does not fit in the window.
    pub fn phasor_part(&self, f: f64, start: usize, len: usize) -> Complex64 {
        let Some(end) = start
            .checked_add(len)
            .filter(|&e| e <= self.work.len() && len > 0)
        else {
            return Complex64::default();
        };
        let w = &self.window[start..end];
        let sum: f64 = w.iter().map(|&v| v as f64).sum();
        goertzel_windowed(&self.work[start..end], w, f) * (2.0 / sum.max(1e-30))
    }

    /// The mean power at `f` over the window of the last hop (as [`Hop::power`]).
    pub fn power_at(&self, f: f64) -> f64 {
        self.phasor(f).norm_sqr() / 2.0
    }

    fn analyse(&mut self) {
        let n = self.ring.len();
        let (a, b) = self.ring.split_at(self.head);
        self.work[..b.len()].copy_from_slice(b);
        self.work[b.len()..].copy_from_slice(a);
        if self.remove_mean {
            let mean = self.work.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
            let m = mean as f32;
            self.work.iter_mut().for_each(|v| *v -= m);
        }
        self.total = self
            .work
            .iter()
            .zip(&self.window)
            .map(|(&x, &w)| {
                let y = (x * w) as f64;
                y * y
            })
            .sum::<f64>()
            / n as f64
            / self.wsq.max(1e-30);
        for k in 0..self.freqs.len() {
            self.power[k] = self.power_at(self.freqs[k]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sine_has_share_one_and_its_power() {
        let f = 0.1;
        let x: Vec<f32> = (0..400)
            .map(|n| 0.5 * (std::f64::consts::TAU * f * n as f64).cos() as f32)
            .collect();
        for w in [Window::Rectangular, Window::Hann] {
            let mut a = Analyser::new(&[f, 0.2], 100, 50, w, false).unwrap();
            let mut hops = 0;
            for &v in &x {
                if !a.push(v) {
                    continue;
                }
                let h = a.last();
                hops += 1;
                assert!((h.power[0] - 0.125).abs() < 1e-3, "{w:?} {}", h.power[0]);
                assert!((h.share(0) - 1.0).abs() < 0.02);
                assert!(h.share(1) < 1e-3);
                assert!((h.level_db() + 6.02).abs() < 0.1);
            }
            assert_eq!(hops, 7);
        }
    }
}
