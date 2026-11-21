//! Level measurement in dBFS, and dB helpers.
//!
//! Full scale: a complex sample of magnitude 1.0 is 0 dBFS (power `|x|^2`); a real sine of amplitude 1.0 is 0 dBFS (power `2 x^2`, the
//! AES17 convention), so the same tone reads the same as IQ or as audio.

use crate::Complex32;

/// A power ratio in dB, `10 log10(p)`. A power at or below `1e-30` (also zero, a negative value and NaN) reads -300 dB.
pub fn power_to_db(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

/// The power ratio of `db` dB, `10^(db / 10)`. [`crate::approx::power_of_db`] is the fast `f32` form within 1e-3 dB.
pub fn db_to_power(db: f64) -> f64 {
    10f64.powf(db / 10.0)
}

/// `20 log10(a)`, with a floor at -300 dB for zero.
pub fn amplitude_to_db(a: f64) -> f64 {
    20.0 * a.max(1e-15).log10()
}

/// The mean power `|x|^2` of a block (0 for an empty one).
pub fn mean_power(x: &[Complex32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64
}

/// The level of a complex block in dBFS.
pub fn power_dbfs(x: &[Complex32]) -> f64 {
    power_to_db(mean_power(x))
}

/// The level of a real block in dBFS (a full-scale sine reads 0).
pub fn real_power_dbfs(x: &[f32]) -> f64 {
    if x.is_empty() {
        return power_to_db(0.0);
    }
    power_to_db(2.0 * x.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / x.len() as f64)
}

/// The RMS of a complex block (`sqrt(mean |x|^2)`).
pub fn rms(x: &[Complex32]) -> f64 {
    mean_power(x).sqrt()
}

/// A running level meter: an exponential average of power with time constant `tau_s`, plus the peak `|x|^2` since the last read.
#[derive(Clone, Debug)]
pub struct PowerMeter {
    a: f64,
    avg: f64,
    peak: f64,
    primed: bool,
}

impl PowerMeter {
    /// A meter averaging over `tau_s` seconds at `rate`.
    pub fn new(rate: f64, tau_s: f64) -> Self {
        assert!(rate > 0.0 && tau_s > 0.0);
        PowerMeter {
            a: 1.0 - (-1.0 / (tau_s * rate)).exp(),
            avg: 0.0,
            peak: 0.0,
            primed: false,
        }
    }

    fn feed(&mut self, mut p: impl FnMut(usize) -> f64, n: usize) {
        for i in 0..n {
            let v = p(i);
            if !self.primed {
                self.avg = v;
                self.primed = true;
            }
            self.avg += self.a * (v - self.avg);
            self.peak = self.peak.max(v);
        }
    }

    /// Measures complex samples.
    pub fn update(&mut self, x: &[Complex32]) {
        self.feed(|i| x[i].norm_sqr() as f64, x.len());
    }

    /// Measures real samples (power `2 x^2`).
    pub fn update_real(&mut self, x: &[f32]) {
        self.feed(|i| 2.0 * (x[i] as f64) * (x[i] as f64), x.len());
    }

    /// The average power (1.0 is full scale).
    pub fn power(&self) -> f64 {
        self.avg
    }

    /// The average level in dBFS.
    pub fn dbfs(&self) -> f64 {
        power_to_db(self.avg)
    }

    /// The peak level in dBFS since the last call, then clears the peak.
    pub fn take_peak_dbfs(&mut self) -> f64 {
        let p = self.peak;
        self.peak = 0.0;
        power_to_db(p)
    }

    /// Clears everything.
    pub fn reset(&mut self) {
        self.avg = 0.0;
        self.peak = 0.0;
        self.primed = false;
    }
}

/// The mean power of a complex stream over one report interval.
#[derive(Clone, Copy, Debug, Default)]
pub struct IntervalLevel {
    sum: f64,
    count: u64,
}

impl IntervalLevel {
    /// Adds samples. A sample that is not finite counts as 0.
    pub fn push(&mut self, x: &[Complex32]) {
        for s in x {
            let v = f64::from(s.norm_sqr());
            self.sum += if v.is_finite() { v } else { 0.0 };
        }
        self.count += x.len() as u64;
    }

    /// Returns the level in dBFS of the samples added since the last call, or `None` if none were added, and starts a new
    /// interval.
    pub fn take_dbfs(&mut self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        let db = power_to_db(self.sum / self.count as f64);
        *self = IntervalLevel::default();
        Some(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn full_scale_reads_zero() {
        assert!(power_dbfs(&ctone(1000, 0.1, 1.0)).abs() < 1e-4);
        assert!(real_power_dbfs(&rtone(1000, 0.1, 1.0)).abs() < 1e-3);
        assert!((power_dbfs(&ctone(1000, 0.1, 0.1)) + 20.0).abs() < 1e-3);
        let mut m = PowerMeter::new(48_000.0, 0.05);
        m.update(&ctone(48_000, 0.01, 0.5));
        assert!((m.dbfs() + 6.0206).abs() < 0.01);
        assert!((m.take_peak_dbfs() + 6.0206).abs() < 0.01);
        assert!((rms(&ctone(100, 0.2, 0.5)) - 0.5).abs() < 1e-6);
        assert!((db_to_power(power_to_db(0.37)) - 0.37).abs() < 1e-12);
    }

    #[test]
    fn decibels_and_power_ratios_of_known_values() {
        assert_eq!(db_to_power(0.0), 1.0);
        assert_eq!(db_to_power(10.0), 10.0);
        assert_eq!(db_to_power(20.0), 100.0);
        assert!((db_to_power(-30.0) - 1e-3).abs() < 1e-18);
        assert!((db_to_power(3.0) - 1.995_262_314_968_88).abs() < 1e-12);
        assert_eq!(power_to_db(1.0), 0.0);
        assert_eq!(power_to_db(100.0), 20.0);
        assert!((power_to_db(0.5) + 3.010_299_956_639_812).abs() < 1e-12);
        assert_eq!(power_to_db(0.0), -300.0);
        assert_eq!(power_to_db(-1.0), -300.0);
        assert_eq!(power_to_db(f64::NAN), -300.0);
        for db in [-120.0, -7.5, 0.25, 42.0] {
            assert!((power_to_db(db_to_power(db)) - db).abs() < 1e-12);
        }
    }

    #[test]
    fn an_interval_level_is_the_mean_power_of_its_samples() {
        let mut l = IntervalLevel::default();
        assert_eq!(l.take_dbfs(), None);
        let a = ctone(1000, 0.1, 0.5);
        let b = ctone(300, 0.03, 0.1);
        l.push(&a);
        l.push(&b);
        let all: Vec<Complex32> = a.iter().chain(&b).copied().collect();
        let db = l.take_dbfs().unwrap();
        assert!((db - power_dbfs(&all)).abs() < 1e-9, "{db}");
        l.push(&[Complex32::new(f32::NAN, 0.0), Complex32::new(1.0, 0.0)]);
        assert!((l.take_dbfs().unwrap() + 3.0103).abs() < 1e-3);
        assert_eq!(l.take_dbfs(), None);
    }
}
