//! Plateaus of a series of window means and the steps between them, as of a noise source switched on and off before a
//! receiver (the Y-factor method of the IRE Standards on Methods of Measuring Noise in Linear Twoports, 1959). A window
//! within `agree` dB of the open plateau joins it (the plateau level is the mean of its window means); any other window ends
//! it and opens a new one. An ended plateau of 2 windows or more counts: when it differs from the last stored plateau by
//! `step` dB or more it gives a step, and it is stored when none is stored or when it differs from the last stored by
//! `agree` dB or more. A plateau of one window (a transition) is dropped.
//!
//! ```
//! use sdrkit_algo_dsp::stepfind::StepFinder;
//! let mut f = StepFinder::new(0.15, 0.3, 8);
//! let mut steps = Vec::new();
//! for w in [0.0, 0.0, 0.0, 5.0, 10.0, 10.0, 10.0, 0.0, 0.0] {
//!     steps.extend(f.push(w));
//! }
//! assert_eq!(steps.len(), 1);
//! assert!((steps[0].to_db - 10.0).abs() < 1e-12 && steps[0].from_db.abs() < 1e-12);
//! ```

use std::collections::VecDeque;

/// A step between two stored plateaus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The level of the plateau before, dB.
    pub from_db: f64,
    /// The level of the plateau that ended now, dB.
    pub to_db: f64,
}

impl Step {
    /// The size of the step, dB.
    pub fn size_db(&self) -> f64 {
        (self.to_db - self.from_db).abs()
    }

    /// The level rose.
    pub fn rose(&self) -> bool {
        self.to_db > self.from_db
    }
}

/// The finder of plateaus and steps (module docs).
#[derive(Clone, Debug)]
pub struct StepFinder {
    agree_db: f64,
    step_db: f64,
    keep: usize,
    open: Option<(f64, usize)>,
    stored: VecDeque<f64>,
}

impl StepFinder {
    /// A finder: windows join within `agree_db`, steps from `step_db`, the last `keep` plateaus stored.
    pub fn new(agree_db: f64, step_db: f64, keep: usize) -> Self {
        StepFinder {
            agree_db,
            step_db,
            keep: keep.max(1),
            open: None,
            stored: VecDeque::new(),
        }
    }

    /// Forgets the plateaus.
    pub fn reset(&mut self) {
        self.open = None;
        self.stored.clear();
    }

    /// The level of the open plateau, dB.
    pub fn level_db(&self) -> Option<f64> {
        self.open.map(|(sum, n)| sum / n as f64)
    }

    /// Takes the next window mean, dB. Returns the step that the plateau ended by it gives.
    pub fn push(&mut self, w: f64) -> Option<Step> {
        match self.open {
            None => {
                self.open = Some((w, 1));
                None
            }
            Some((sum, n)) if (w - sum / n as f64).abs() <= self.agree_db => {
                self.open = Some((sum + w, n + 1));
                None
            }
            Some((sum, n)) => {
                self.open = Some((w, 1));
                if n < 2 {
                    return None;
                }
                let level = sum / n as f64;
                let last = self.stored.back().copied();
                let step = last
                    .filter(|l| (level - l).abs() >= self.step_db)
                    .map(|l| Step {
                        from_db: l,
                        to_db: level,
                    });
                if last.is_none_or(|l| (level - l).abs() >= self.agree_db) {
                    self.stored.push_back(level);
                    if self.stored.len() > self.keep {
                        self.stored.pop_front();
                    }
                }
                step
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_follow_plateaus_and_small_changes_give_none() {
        let mut f = StepFinder::new(0.15, 0.3, 8);
        let mut steps = Vec::new();
        for k in 0..6 {
            let level = if k % 2 == 1 { 10.0 } else { 0.05 * k as f64 };
            for _ in 0..4 {
                steps.extend(f.push(level));
            }
            steps.extend(f.push(5.0));
        }
        assert_eq!(steps.len(), 5, "{steps:?}");
        assert!(steps[0].rose() && !steps[1].rose());
        assert!(steps.iter().all(|s| (s.size_db() - 10.0).abs() < 0.3));
        let mut g = StepFinder::new(0.15, 0.3, 8);
        let mut n = 0;
        for k in 0..20 {
            for _ in 0..10 {
                n += usize::from(g.push(if k % 2 == 1 { 0.1 } else { 0.0 }).is_some());
            }
        }
        assert_eq!(n, 0);
    }
}
