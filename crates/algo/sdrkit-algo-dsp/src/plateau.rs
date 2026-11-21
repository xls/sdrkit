//! Plateaus, edges, steps and settling of measured series: the level of the flat middle of a pulse, the crossings of its
//! rising and falling edges by linear interpolation, the test of a new reading against the recent ones for a step, and the
//! settling time of a series to its final value.
//!
//! ```
//! use sdrkit_algo_dsp::plateau::{crossing_up, plateau_level};
//! let a = [0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0, 1.0, 0.5, 0.0];
//! assert_eq!(plateau_level(&a, 0, 10), Some(1.0));
//! assert_eq!(crossing_up(&a, 0.25, 0, 5), Some(1.5));
//! ```

/// The median of the middle third of `x[start..end]`: from `start + (end - start) / 3` to `start + 2 (end - start) / 3`.
/// `None` with fewer than 3 samples there.
pub fn plateau_level(x: &[f64], start: usize, end: usize) -> Option<f64> {
    let end = end.min(x.len());
    if end <= start {
        return None;
    }
    let len = end - start;
    let (a, b) = (start + len / 3, start + 2 * len / 3);
    if b < a + 3 {
        return None;
    }
    let mut v = x[a..b].to_vec();
    let m = v.len() / 2;
    let (_, med, _) = v.select_nth_unstable_by(m, f64::total_cmp);
    Some(*med)
}

/// The first upward crossing of `level` in `x[from..to]`, as a fractional index by linear interpolation.
pub fn crossing_up(x: &[f64], level: f64, from: usize, to: usize) -> Option<f64> {
    let to = to.min(x.len());
    (from.max(1)..to).find_map(|i| {
        (x[i - 1] < level && x[i] >= level)
            .then(|| (i - 1) as f64 + (level - x[i - 1]) / (x[i] - x[i - 1]))
    })
}

/// The first downward crossing of `level` in `x[from..to]`, searching forward, as a fractional index.
pub fn crossing_down(x: &[f64], level: f64, from: usize, to: usize) -> Option<f64> {
    let to = to.min(x.len());
    (from.max(1)..to).find_map(|i| {
        (x[i - 1] >= level && x[i] < level)
            .then(|| (i - 1) as f64 + (x[i - 1] - level) / (x[i - 1] - x[i]))
    })
}

/// The last downward crossing of `level` in `x[from..to]`, searching back from `to`, as a fractional index.
pub fn crossing_down_back(x: &[f64], level: f64, from: usize, to: usize) -> Option<f64> {
    let to = to.min(x.len());
    (from.max(1)..to).rev().find_map(|i| {
        (x[i - 1] >= level && x[i] < level)
            .then(|| (i - 1) as f64 + (x[i - 1] - level) / (x[i - 1] - x[i]))
    })
}

/// The mean and the sample standard deviation (divisor `n - 1`; 0 for one value). `None` for an empty list.
pub fn mean_and_scatter(v: &[f64]) -> Option<(f64, f64)> {
    let n = v.len();
    if n == 0 {
        return None;
    }
    let m = v.iter().sum::<f64>() / n as f64;
    let s = if n > 1 {
        (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1) as f64).sqrt()
    } else {
        0.0
    };
    Some((m, s))
}

/// The step test of a series of readings.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepRule {
    /// The readings whose median is the old value.
    pub median_of: usize,
    /// The readings whose scatter sets the threshold.
    pub scatter_of: usize,
    /// The scatter used with fewer than 3 readings.
    pub default_scatter: f64,
    /// The threshold in scatter widths.
    pub sigmas: f64,
    /// The smallest step.
    pub min_step: f64,
}

impl StepRule {
    /// A step is a reading `f` that differs from the median of the last `median_of` readings of `history` (the newest last) by
    /// more than `max(min_step, sigmas x s)`, with `s` the sample standard deviation of the last `scatter_of`. Returns the old
    /// value (that median) for a step. `None` without a step or with fewer than `median_of` readings.
    pub fn test(&self, history: &[f64], f: f64) -> Option<f64> {
        let n = history.len();
        if n < self.median_of.max(1) {
            return None;
        }
        let mut last = history[n - self.median_of..].to_vec();
        let m = last.len() / 2;
        let (_, med, _) = last.select_nth_unstable_by(m, f64::total_cmp);
        let med = *med;
        let k = self.scatter_of.min(n);
        let s = if k >= 3 {
            mean_and_scatter(&history[n - k..]).map_or(self.default_scatter, |(_, s)| s)
        } else {
            self.default_scatter
        };
        ((f - med).abs() > self.min_step.max(self.sigmas * s)).then_some(med)
    }
}

/// The settling time of the readings `(t, f)` to `final_value`: the time of the last reading outside `final_value +- band`,
/// plus `after`; 0 when every reading is inside.
pub fn settling_time(readings: &[(f64, f64)], final_value: f64, band: f64, after: f64) -> f64 {
    readings
        .iter()
        .rev()
        .find(|&&(_, f)| (f - final_value).abs() > band)
        .map_or(0.0, |&(t, _)| t + after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ramped_pulse_gives_its_plateau_and_edges_a_step_is_found_once() {
        let a: Vec<f64> = (0..100)
            .map(|i| match i {
                0..10 => 0.0,
                10..20 => (i - 10) as f64 / 10.0,
                20..80 => 1.0,
                80..90 => (90 - i) as f64 / 10.0,
                _ => 0.0,
            })
            .collect();
        assert_eq!(plateau_level(&a, 0, 100), Some(1.0));
        let (r10, r90) = (
            crossing_up(&a, 0.1, 0, 50).unwrap(),
            crossing_up(&a, 0.9, 0, 50).unwrap(),
        );
        assert!((r90 - r10 - 8.0).abs() < 1e-9);
        let f90 = crossing_down_back(&a, 0.9, 50, 100).unwrap();
        let f10 = crossing_down(&a, 0.1, f90 as usize, 100).unwrap();
        assert!((f10 - f90 - 8.0).abs() < 1e-9);
        let rule = StepRule {
            median_of: 5,
            scatter_of: 10,
            default_scatter: 0.1,
            sigmas: 8.0,
            min_step: 0.5,
        };
        let h = [50.0, 50.02, 49.98, 50.01, 49.99, 50.0];
        assert_eq!(rule.test(&h, 50.05), None);
        assert_eq!(rule.test(&h, 53.0), Some(50.0));
        assert_eq!(rule.test(&h[..4], 53.0), None);
        let r = [(0.1, 5.0), (0.2, 2.0), (0.3, 0.5), (0.4, 0.2)];
        assert_eq!(settling_time(&r, 0.0, 1.0, 0.05), 0.25);
        assert_eq!(settling_time(&r[2..], 0.0, 1.0, 0.05), 0.0);
    }
}
