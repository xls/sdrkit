//! Level control of speech: a leveller that brings the short-term RMS towards a target, and a look-ahead limiter whose
//! gain has no steps.

use crate::iir::OnePole;
use std::collections::VecDeque;

/// The factor of a one-pole smoother with time constant `tau_s` at `rate`; 1 (no smoothing) for a time or rate of 0 or less.
fn factor(tau_s: f64, rate: f64) -> f32 {
    if tau_s <= 0.0 || rate <= 0.0 {
        return 1.0;
    }
    OnePole::with_time_constant(rate, tau_s).factor() as f32
}

/// A leveller: the gain moves the RMS over 30 ms towards a target, within +-`range_db`. The gain falls with a time constant
/// of 60 ms; it rises with 1 s, but only 0.5 s after the last fall. Below an RMS of 1e-4 the gain is held.
pub struct Leveller {
    ms: f32,
    ms_a: f32,
    gain: f32,
    fall_a: f32,
    rise_a: f32,
    wait: u32,
    wait_len: u32,
    target: f32,
    least: f32,
    most: f32,
}

impl Leveller {
    /// A leveller at `rate` towards the RMS `target`, the gain within +-12 dB.
    pub fn new(rate: f64, target: f32) -> Self {
        Self::with_range(rate, target, 12.0)
    }

    /// A leveller at `rate` towards the RMS `target`, the gain within +-`range_db`.
    pub fn with_range(rate: f64, target: f32, range_db: f32) -> Self {
        let most = 10f32.powf(range_db / 20.0);
        Leveller {
            ms: 0.0,
            ms_a: factor(0.03, rate),
            gain: 1.0,
            fall_a: factor(0.06, rate),
            rise_a: factor(1.0, rate),
            wait: 0,
            wait_len: (0.5 * rate) as u32,
            target,
            least: 10f32.powf(-range_db / 20.0),
            most,
        }
    }

    /// The gain now, dB.
    pub fn gain_db(&self) -> f32 {
        20.0 * self.gain.max(1e-9).log10()
    }

    /// One sample: `x` with the gain.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        self.ms += self.ms_a * (x * x - self.ms);
        let rms = self.ms.sqrt();
        if rms >= 1e-4 {
            let want = (self.target / rms).clamp(self.least, self.most);
            if want < self.gain {
                self.gain += self.fall_a * (want - self.gain);
                self.wait = self.wait_len;
            } else if self.wait > 0 {
                self.wait -= 1;
            } else {
                self.gain += self.rise_a * (want - self.gain);
            }
        }
        x * self.gain
    }
}

/// A look-ahead limiter for real or complex samples whose gain has no steps. Each sample wants the gain `ceiling / |x|` (at
/// most 1). The least wanted gain over the look-ahead is held, rises back with the release time, and is averaged over the
/// look-ahead. The output lags the input by the look-ahead less one sample; its magnitude never passes the ceiling. The gain
/// widens the spectrum by about one over the look-ahead.
pub struct SmoothLimiter<T> {
    line: VecDeque<T>,
    wanted: VecDeque<(u64, f32)>,
    gains: Vec<f32>,
    next: usize,
    total: f64,
    held: f32,
    release_a: f32,
    ceiling: f32,
    count: u64,
    lowest: f32,
}

impl<T: Copy + Default + std::ops::Mul<f32, Output = T>> SmoothLimiter<T> {
    /// A limiter at `rate` with `ahead_s` of look-ahead, the magnitude `ceiling` and the release time `release_s`.
    pub fn new(rate: f64, ahead_s: f64, ceiling: f32, release_s: f64) -> Self {
        let len = ((ahead_s * rate).round() as usize).max(1);
        let mut line = VecDeque::with_capacity(len);
        line.extend(std::iter::repeat_n(T::default(), len - 1));
        SmoothLimiter {
            line,
            wanted: VecDeque::with_capacity(len + 1),
            gains: vec![1.0; len],
            next: 0,
            total: len as f64,
            held: 1.0,
            release_a: factor(release_s, rate),
            ceiling,
            count: 0,
            lowest: 1.0,
        }
    }

    /// The delay, samples.
    pub fn delay(&self) -> usize {
        self.gains.len() - 1
    }

    /// The least gain since the last call, dB (0 or below); starts the next interval.
    pub fn take_reduction_db(&mut self) -> f32 {
        let g = std::mem::replace(&mut self.lowest, 1.0);
        20.0 * g.max(1e-9).log10()
    }

    /// One sample `x` of magnitude `mag`: returns the sample that leaves the look-ahead, with its gain.
    #[inline]
    pub fn push(&mut self, x: T, mag: f32) -> T {
        let len = self.gains.len();
        let want = if mag > self.ceiling {
            self.ceiling / mag
        } else {
            1.0
        };
        // The wanted gains of the look-ahead, rising from the front: the front is the least.
        while self.wanted.back().is_some_and(|&(_, w)| w >= want) {
            self.wanted.pop_back();
        }
        self.wanted.push_back((self.count, want));
        while self
            .wanted
            .front()
            .is_some_and(|&(i, _)| i + len as u64 <= self.count)
        {
            self.wanted.pop_front();
        }
        self.count += 1;
        let least = self.wanted.front().map_or(1.0, |p| p.1);
        self.held = if least < self.held {
            least
        } else {
            self.held + self.release_a * (least - self.held)
        };
        self.total += f64::from(self.held) - f64::from(self.gains[self.next]);
        self.gains[self.next] = self.held;
        self.next = (self.next + 1) % len;
        let g = ((self.total / len as f64) as f32).min(1.0);
        self.lowest = self.lowest.min(g);
        self.line.push_back(x);
        self.line.pop_front().unwrap_or_default() * g
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Complex32;

    /// A sine at 0.05 settles near the RMS target 0.12: its peak at 0.12 times the square root of 2.
    #[test]
    fn the_leveller_brings_the_rms_to_its_target() {
        let mut l = Leveller::new(48e3, 0.12);
        let mut peak = 0.0f32;
        for i in 0..5 * 48_000 {
            let y = l.push(0.05 * (i as f32 * 0.13).sin());
            if i > 4 * 48_000 {
                peak = peak.max(y.abs());
            }
        }
        assert!((peak - 0.12 * 2f32.sqrt()).abs() < 0.03, "{peak}");
    }

    /// The gain stays within the range; silence holds it.
    #[test]
    fn the_leveller_gain_stays_in_its_range_and_holds_in_silence() {
        let mut l = Leveller::with_range(8e3, 0.5, 6.0);
        for i in 0..8_000 * 10 {
            l.push(1e-3 * (i as f32 * 0.3).sin());
        }
        assert!((l.gain_db() - 6.0).abs() < 0.05, "{}", l.gain_db());
        for _ in 0..8_000 {
            l.push(0.0);
        }
        assert!((l.gain_db() - 6.0).abs() < 0.05, "held: {}", l.gain_db());
        for i in 0..8_000 * 4 {
            l.push(0.9 * (i as f32 * 0.3).sin());
        }
        assert!(l.gain_db() < -1.0 && l.gain_db() >= -6.0 - 1e-3, "{}", l.gain_db());
    }

    /// Complex peaks 15.6 dB over the ceiling stay under it, and the gain moves in steps no larger than the drop spread
    /// over the 96 samples of the look-ahead.
    #[test]
    fn the_limiter_keeps_peaks_under_its_ceiling_without_gain_steps() {
        let loud = |i: usize| if (20_000..20_050).contains(&i) { 3.0 } else { 0.5 };
        let mut l = SmoothLimiter::<Complex32>::new(48e3, 0.002, 0.9, 0.05);
        assert_eq!(l.delay(), 95);
        let (mut top, mut prev, mut step) = (0.0f32, 1.0f32, 0.0f32);
        for i in 0..48_000 {
            let p = i as f32 * 0.07;
            let x = Complex32::new(p.cos(), p.sin()) * loud(i);
            let y = l.push(x, x.norm());
            top = top.max(y.norm());
            if i > l.delay() {
                let g = y.norm() / loud(i - l.delay());
                step = step.max((g - prev).abs());
                prev = g;
            }
        }
        assert!(top <= 0.9 + 1e-5, "{top}");
        assert!(step < 0.7 / 96.0 + 1e-3, "{step}");
        assert!(l.take_reduction_db() < -9.0);
        assert_eq!(l.take_reduction_db(), 0.0, "a new interval");
    }

    /// Real samples under the ceiling pass unchanged after the delay.
    #[test]
    fn the_limiter_passes_quiet_samples_after_its_delay() {
        let mut l = SmoothLimiter::<f32>::new(8e3, 0.001, 1.0, 0.05);
        let x: Vec<f32> = (0..200).map(|i| 0.5 * (i as f32 * 0.2).sin()).collect();
        let y: Vec<f32> = x.iter().map(|&v| l.push(v, v.abs())).collect();
        let d = l.delay();
        assert_eq!(d, 7);
        assert!(y[..d].iter().all(|v| *v == 0.0));
        assert_eq!(&y[d..], &x[..x.len() - d]);
    }
}
