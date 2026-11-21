//! Level control of the AM processor: the look-ahead limiter, and a bank of FIR filters with cross-fades for the audio
//! low-pass. The leveller is [`sdrkit_algo_dsp::dynamics::Leveller`].

use crate::smoothing::one_pole;
use sdrkit_algo_dsp::fir::Fir;
use sdrkit_algo_dsp::Sample;
use std::collections::VecDeque;

/// A look-ahead limiter: the signal is delayed by the look-ahead, and the gain drops before a peak above the ceiling arrives, then
/// recovers.
pub struct LookAheadLimiter {
    delay: VecDeque<f32>,
    peaks: VecDeque<(u64, f32)>,
    n: u64,
    len: usize,
    ceiling: f32,
    gain: f32,
    rec_a: f32,
    /// Limiting on (the delay stays either way).
    pub active: bool,
}

impl LookAheadLimiter {
    /// At `rate`, `ahead_s` of look-ahead, a ceiling and a recovery time.
    pub fn new(rate: f64, ahead_s: f64, ceiling: f32, recovery_s: f64) -> Self {
        let len = ((ahead_s * rate).round() as usize).max(1);
        let mut delay = VecDeque::with_capacity(len + 1);
        delay.extend(std::iter::repeat_n(0.0, len));
        LookAheadLimiter {
            delay,
            peaks: VecDeque::with_capacity(len + 2),
            n: 0,
            len,
            ceiling,
            gain: 1.0,
            rec_a: one_pole(recovery_s, rate),
            active: true,
        }
    }
    /// The delay in samples.
    pub fn delay(&self) -> usize {
        self.len
    }
    /// One sample.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let a = x.abs();
        while let Some(&(_, p)) = self.peaks.back() {
            if p <= a {
                self.peaks.pop_back();
            } else {
                break;
            }
        }
        self.peaks.push_back((self.n, a));
        let oldest = self.n.saturating_sub(self.len as u64);
        while let Some(&(i, _)) = self.peaks.front() {
            if i < oldest {
                self.peaks.pop_front();
            } else {
                break;
            }
        }
        self.n += 1;
        self.delay.push_back(x);
        let y = self.delay.pop_front().unwrap_or(0.0);
        if !self.active {
            self.gain = 1.0;
            return y;
        }
        let peak = self.peaks.front().map_or(0.0, |p| p.1);
        let want = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if want < self.gain {
            self.gain = want;
        } else {
            self.gain += self.rec_a * (want - self.gain);
        }
        y * self.gain
    }
}

/// A bank of symmetric FIR filters of the same length sharing one history, with cross-fades between them.
pub struct FirBank<T: Sample> {
    hist: Fir<T>,
    bank: Vec<Vec<f32>>,
    cur: usize,
    prev: usize,
    fade: u32,
    fade_len: u32,
}

impl<T: Sample> FirBank<T> {
    /// From `bank` (each symmetric, the same length), starting at `start`, with `fade_len` samples of cross-fade.
    pub fn new(bank: Vec<Vec<f32>>, start: usize, fade_len: u32) -> Self {
        let hist = Fir::new(&bank[0]);
        FirBank {
            hist,
            bank,
            cur: start,
            prev: start,
            fade: 0,
            fade_len: fade_len.max(1),
        }
    }
    /// The filter delay (samples).
    pub fn delay(&self) -> usize {
        (self.hist.len() - 1) / 2
    }
    /// The filter in use.
    pub fn current(&self) -> usize {
        self.cur
    }
    /// Moves to filter `i`, with a cross-fade.
    pub fn select(&mut self, i: usize) {
        let i = i.min(self.bank.len() - 1);
        if i != self.cur {
            self.prev = self.cur;
            self.cur = i;
            self.fade = self.fade_len;
        }
    }
    /// One sample.
    #[inline]
    pub fn push(&mut self, x: T) -> T {
        self.hist.push(x);
        let w = self.hist.window();
        let y = T::dot(&self.bank[self.cur], w);
        if self.fade == 0 {
            return y;
        }
        let z = T::dot(&self.bank[self.prev], w);
        let t = self.fade as f32 / self.fade_len as f32;
        self.fade -= 1;
        z * t + y * (1.0 - t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limiter_keeps_peaks_under_its_ceiling() {
        let mut l = LookAheadLimiter::new(48e3, 0.003, 0.9, 0.05);
        let mut max = 0.0f32;
        for i in 0..48_000 {
            let x = if (20_000..20_100).contains(&i) {
                2.0
            } else {
                0.3
            } * ((i as f32) * 0.1).sin();
            max = max.max(l.push(x).abs());
        }
        assert!(max <= 0.9 + 1e-4, "{max}");
    }
}
