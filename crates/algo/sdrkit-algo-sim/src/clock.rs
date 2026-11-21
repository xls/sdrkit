//! Sample-clock error: the signal as a receiver with a fast or slow sample clock reads it. A clock `ppm` parts per million fast
//! produces `1 + ppm 1e-6` output samples per input sample, so every frequency and symbol rate reads lower by that factor.
//! Bit-edge jitter ([`ClockError::with_jitter`]) moves each bit edge by its own random delay, at most the peak.

use crate::rng::Rng;
use crate::{Param, SimError};
use sdrkit_algo_dsp::interp::{Interpolator, Polyphase};
use sdrkit_core::Complex32;

/// Interpolator taps.
const TAPS: usize = 32;
/// Fractional bits of the position.
const FRAC: u32 = 40;
const ONE: u64 = 1 << FRAC;

/// A sample-clock error applied by fractional resampling.
///
/// ```
/// use sdrkit_algo_sim::clock::ClockError;
/// use sdrkit_core::Complex32;
/// let mut c = ClockError::new(1000.0).unwrap(); // 1000 ppm fast
/// let x = vec![Complex32::new(1.0, 0.0); 100_000];
/// let mut y = Vec::new();
/// c.process(&x, &mut y);
/// assert_eq!(y.len(), 100_100);
/// ```
#[derive(Clone, Debug)]
pub struct ClockError {
    interp: Polyphase<TAPS, 256>,
    /// The last `TAPS` inputs, written twice so the window is one slice: `hist[w..w + TAPS]`.
    hist: Vec<Complex32>,
    w: usize,
    /// Position of the next output past the newest input's window point, in input samples (Q24.40).
    pos: u64,
    step: u64,
    ppm: f64,
    jitter: Option<Jitter>,
}

/// A delay that moves in straight lines between random values, one value each bit period, in output samples.
#[derive(Clone, Debug)]
struct Jitter {
    period: f64,
    peak: f64,
    rng: Rng,
    from: f64,
    to: f64,
    t: f64,
    last: f64,
}

impl Jitter {
    /// The delay for the next output sample, and the change since the last one.
    fn step(&mut self) -> f64 {
        let d = self.from + (self.to - self.from) * self.t / self.period;
        self.t += 1.0;
        if self.t >= self.period {
            self.t -= self.period;
            self.from = self.to;
            self.to = self.peak * (2.0 * self.rng.uniform() - 1.0);
        }
        let change = d - self.last;
        self.last = d;
        change
    }
}

impl ClockError {
    /// A clock `ppm` parts per million fast (negative: slow). Errors outside `-100000..=100000` ppm (10 %) or for NaN. The signal
    /// must lie within 0.4 of the sample rate either side of 0 Hz (the interpolator is flat to within 0.01 dB there).
    pub fn new(ppm: f64) -> Result<Self, SimError> {
        if ppm.is_nan() || ppm.abs() > 100_000.0 {
            return Param::ClockPpm.range(-100_000.0, 100_000.0, ppm);
        }
        let interp = Polyphase::<TAPS, 256>::design(0.92, 90.0)?;
        let mut c = ClockError {
            interp,
            hist: vec![Complex32::default(); 2 * TAPS],
            w: 0,
            pos: 0,
            step: ONE,
            ppm: 0.0,
            jitter: None,
        };
        c.set_ppm(ppm)?;
        Ok(c)
    }

    /// Adds bit-edge jitter: every `period` output samples (one bit) the delay heads for a new random value, uniform within
    /// `peak` samples either side of 0, so no edge moves more than `peak`. `seed` picks the random values. Errors: a period
    /// under 2 samples, or a peak that is negative or over a quarter of the period.
    pub fn with_jitter(mut self, period: f64, peak: f64, seed: u64) -> Result<Self, SimError> {
        if !(period.is_finite() && period >= 2.0) {
            return Param::JitterPeriod.at_least(2.0, period);
        }
        if !(peak.is_finite() && (0.0..=0.25 * period).contains(&peak)) {
            return Param::JitterPeak.range(0.0, 0.25 * period, peak);
        }
        let mut rng = Rng::new(seed);
        let to = peak * (2.0 * rng.uniform() - 1.0);
        self.jitter = Some(Jitter {
            period,
            peak,
            rng,
            from: 0.0,
            to,
            t: 0.0,
            last: 0.0,
        });
        Ok(self)
    }

    /// Changes the error from the next output on (no jump in time).
    pub fn set_ppm(&mut self, ppm: f64) -> Result<(), SimError> {
        if ppm.is_nan() || ppm.abs() > 100_000.0 {
            return Param::ClockPpm.range(-100_000.0, 100_000.0, ppm);
        }
        self.ppm = ppm;
        // Rounded up, so a whole number of outputs per input span (1000 ppm on 100000 samples: 100100) comes out exactly.
        self.step = (ONE as f64 / (1.0 + ppm * 1e-6)).ceil() as u64;
        Ok(())
    }

    /// The error, ppm.
    pub fn ppm(&self) -> f64 {
        self.ppm
    }

    /// The fixed delay the interpolator adds, in input samples: output sample 0 is input sample `-delay` (zeros before the start).
    pub fn delay(&self) -> f64 {
        (TAPS / 2) as f64
    }

    /// How many outputs the next `n` inputs give: exact without jitter, at most this many with it.
    pub fn outputs_for(&self, n: usize) -> usize {
        let end = n as u128 * ONE as u128;
        let pos = self.pos as u128;
        let exact = if end <= pos {
            0
        } else {
            (end - pos).div_ceil(self.step as u128) as usize
        };
        exact
            + self
                .jitter
                .as_ref()
                .map_or(0, |j| 2 * j.peak.ceil() as usize + 2)
    }

    /// Appends the outputs for `input` to `out` (nothing is allocated once `out` has the capacity; [`outputs_for`](
    /// Self::outputs_for) says how much).
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        for &x in input {
            self.hist[self.w] = x;
            self.hist[self.w + TAPS] = x;
            self.w = (self.w + 1) % TAPS;
            let win = &self.hist[self.w..self.w + TAPS];
            while self.pos < ONE {
                let mu = (self.pos >> (FRAC - 24)) as f32 * (1.0 / (1u32 << 24) as f32);
                out.push(self.interp.at(win, mu));
                match &mut self.jitter {
                    // A later edge reads earlier input. The step stays above half a sample: the peak is a quarter bit at most.
                    Some(j) => {
                        let change = (j.step() * ONE as f64) as i64;
                        self.pos = (self.pos as i64 + self.step as i64 - change).max(1) as u64;
                    }
                    None => self.pos += self.step,
                }
            }
            self.pos -= ONE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn tone(n: usize, f: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * (f * i as f64).rem_euclid(1.0);
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect()
    }

    #[test]
    fn a_fast_clock_returns_more_samples_and_lower_frequencies() {
        // 250 ppm fast for a million samples: 250 more samples, and a tone at 0.2 of the rate reads 0.2 / (1 + 250e-6), matched
        // sample by sample against the exact tone at the scaled time (error below -70 dB).
        for ppm in [250.0, -250.0, 4000.0] {
            let f = 0.2;
            let x = tone(1_000_000, f);
            let mut c = ClockError::new(ppm).unwrap();
            let mut y = Vec::new();
            for piece in x.chunks(4093) {
                c.process(piece, &mut y);
            }
            let extra = y.len() as f64 - x.len() as f64;
            assert!(
                (extra - ppm).abs() <= 1.0,
                "{ppm} ppm: {extra} extra samples"
            );
            let k = 1.0 + ppm * 1e-6;
            let mut err = 0.0f64;
            let mut sig = 0.0f64;
            for (j, v) in y.iter().enumerate().skip(100).step_by(7) {
                let t = j as f64 / k - c.delay();
                let p = TAU * (f * t).rem_euclid(1.0);
                let want = Complex32::new(p.cos() as f32, p.sin() as f32);
                err += (*v - want).norm_sqr() as f64;
                sig += 1.0;
            }
            let db = 10.0 * (err / sig).log10();
            assert!(db < -70.0, "{ppm} ppm: error {db} dB");
        }
    }

    #[test]
    fn the_output_is_the_same_for_any_cut_and_counts_are_exact() {
        let x = tone(20_000, 0.013);
        let mut a = ClockError::new(-77.0).unwrap();
        let mut b = ClockError::new(-77.0).unwrap();
        let mut ya = Vec::new();
        let n = a.outputs_for(x.len());
        a.process(&x, &mut ya);
        assert_eq!(n, ya.len());
        let mut yb = Vec::new();
        for p in x.chunks(17) {
            b.process(p, &mut yb);
        }
        assert_eq!(ya, yb);
        assert!(ClockError::new(f64::NAN).is_err());
        assert!(ClockError::new(2e5).is_err());
    }

    /// Edges of a bit stream move by their own amounts, never more than the peak, and the cut of the input changes nothing.
    #[test]
    fn bit_edge_jitter_stays_within_its_peak() {
        let bit = 10usize;
        let mut lfsr = 0xACE1u16;
        let x: Vec<Complex32> = (0..4000)
            .flat_map(|_| {
                lfsr = (lfsr >> 1) ^ (if lfsr & 1 != 0 { 0xB400 } else { 0 });
                let v = if lfsr & 1 != 0 { 1.0 } else { -1.0 };
                std::iter::repeat_n(Complex32::new(v, 0.0), bit)
            })
            .collect();
        let peak = 2.0;
        let mut c = ClockError::new(0.0)
            .unwrap()
            .with_jitter(bit as f64, peak, 9)
            .unwrap();
        let mut y = Vec::new();
        c.process(&x, &mut y);
        let mut d = ClockError::new(0.0)
            .unwrap()
            .with_jitter(bit as f64, peak, 9)
            .unwrap();
        let mut z = Vec::new();
        for p in x.chunks(37) {
            d.process(p, &mut z);
        }
        assert_eq!(y, z);
        // Each zero crossing, against the nearest bit boundary (input edges at multiples of 10, plus the fixed delay).
        let (mut worst, mut sum, mut n) = (0f64, 0f64, 0usize);
        for k in 40..y.len() - 1 {
            let (a, b) = (y[k].re as f64, y[k + 1].re as f64);
            if a.signum() != b.signum() && a != 0.0 {
                let t = k as f64 + a / (a - b) + 0.5 - c.delay();
                let e = t - (t / bit as f64).round() * bit as f64;
                worst = worst.max(e.abs());
                sum += e * e;
                n += 1;
            }
        }
        let rms = (sum / n as f64).sqrt();
        assert!(worst <= peak + 0.2, "worst {worst}");
        assert!(rms > 0.6 && rms < 1.4, "rms {rms}");
        assert!(ClockError::new(0.0)
            .unwrap()
            .with_jitter(10.0, 3.0, 1)
            .is_err());
        assert!(ClockError::new(0.0)
            .unwrap()
            .with_jitter(1.0, 0.1, 1)
            .is_err());
    }
}
