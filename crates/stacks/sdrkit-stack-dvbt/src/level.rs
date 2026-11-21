//! Watch on the input level: counts sudden steps of the power. A radio whose gain control hunts changes its gain, and
//! with it the phase of the signal, many times a second; no symbol that holds such a step can be demodulated.

use sdrkit_core::Complex32;

/// Samples of one power measurement.
const GROUP: usize = 256;
/// The least change between two measurements, one group apart, that counts as a step, dB. The power of an OFDM signal
/// over one group varies by about 0.3 dB.
const STEP_DB: f64 = 2.0;
/// The least steps in one second that are reported.
pub const HUNTING_STEPS: u32 = 20;

/// The step counter.
#[derive(Default)]
pub struct LevelSteps {
    acc: f64,
    n: usize,
    /// The two measurements before, newest last, dB.
    before: [Option<f64>; 2],
    quiet: u8,
    steps: u32,
    largest: f64,
    samples: u64,
}

impl LevelSteps {
    /// Takes samples at `rate`. Returns, once for each second, the steps of that second and the largest of them in dB.
    pub fn push(&mut self, x: &[Complex32], rate: f64) -> Option<(u32, f64)> {
        let mut report = None;
        for s in x {
            self.acc += f64::from(s.norm_sqr());
            self.n += 1;
            if self.n < GROUP {
                continue;
            }
            let db = 10.0 * (self.acc / GROUP as f64).max(1e-20).log10();
            self.acc = 0.0;
            self.n = 0;
            // The group between the two that are compared may hold the step itself.
            if let Some(old) = self.before[0] {
                let d = (db - old).abs();
                if self.quiet > 0 {
                    self.quiet -= 1;
                } else if d >= STEP_DB {
                    self.steps += 1;
                    self.largest = self.largest.max(d);
                    self.quiet = 2;
                }
            }
            self.before = [self.before[1], Some(db)];
            self.samples += GROUP as u64;
            if self.samples as f64 >= rate {
                report = Some((self.steps, self.largest));
                self.samples = 0;
                self.steps = 0;
                self.largest = 0.0;
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Noise of unit power from a fixed generator, scaled by `gain(i)`.
    fn noise(n: usize, gain: impl Fn(usize) -> f32) -> Vec<Complex32> {
        let mut state = 0x1234_5678_9abc_def0u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
        };
        (0..n)
            .map(|i| Complex32::new(next(), next()) * (gain(i) * 6f32.sqrt()))
            .collect()
    }

    #[test]
    fn a_steady_level_has_no_step() {
        let rate = 1e6;
        let x = noise(2_000_000, |_| 0.2);
        let mut w = LevelSteps::default();
        let r = w.push(&x, rate);
        assert_eq!(r.map(|r| r.0), Some(0));
    }

    #[test]
    fn a_gain_that_hunts_is_counted() {
        // 4.5 dB up and down every 1.5 ms: 667 steps in a second.
        let rate = 1e6;
        let x = noise(1_100_000, |i| {
            if (i / 1500).is_multiple_of(2) {
                0.2
            } else {
                0.336
            }
        });
        let mut w = LevelSteps::default();
        let (steps, largest) = w.push(&x, rate).expect("one second");
        assert!((600..=700).contains(&steps), "{steps} steps");
        // The largest of 667 differences of two measurements, each 0.3 dB uncertain: 4.5 dB and up to four deviations.
        assert!((4.5..6.2).contains(&largest), "{largest} dB");
    }
}
