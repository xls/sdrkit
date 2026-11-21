//! The step colours and the quality class that the receivers of this crate publish, and the quality score of ATSC 1.0.

use sdrkit_core::{LockState, Results};
use std::collections::VecDeque;

/// What a receiver shows in its status strip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StatusLine<'a> {
    /// The lock_state.
    pub lock_state: LockState,
    /// The SNR of what is decoded, dB.
    pub snr_db: Option<f32>,
    /// The carrier offset, Hz.
    pub offset_hz: Option<f64>,
    /// The input level, dBFS.
    pub level_dbfs: Option<f32>,
    /// The short line.
    pub summary: &'a str,
}

/// Writes `s` into the status of `r`. The status changes, and its counter moves, only when a part differs.
pub fn write_status(r: &mut Results, s: &StatusLine<'_>) {
    r.update_status(|x| {
        let same = x.lock_state == s.lock_state
            && x.snr_db == s.snr_db
            && x.offset_hz == s.offset_hz
            && x.level_dbfs == s.level_dbfs
            && x.summary == s.summary;
        if !same {
            x.lock_state = s.lock_state;
            x.snr_db = s.snr_db;
            x.offset_hz = s.offset_hz;
            x.level_dbfs = s.level_dbfs;
            x.summary.clear();
            x.summary.push_str(s.summary);
        }
        !same
    });
}

sdrkit_core::choice_enum! {
    /// The colour of one step of a receiver's step row.
    pub enum StepColour {
        /// Not reached yet.
        Off = ("off", "Off"),
        /// Partly there.
        Amber = ("amber", "Amber"),
        /// Good.
        Green = ("green", "Green"),
        /// Bad.
        Red = ("red", "Red"),
    }
}

impl StepColour {
    /// Green when `on`, amber when only `part` holds, otherwise off.
    pub fn of(on: bool, part: bool) -> StepColour {
        if on {
            StepColour::Green
        } else if part {
            StepColour::Amber
        } else {
            StepColour::Off
        }
    }
}

sdrkit_core::choice_enum! {
    /// The quality class of the reception.
    pub enum Quality {
        /// 85 % or more.
        Excellent = ("excellent", "Excellent"),
        /// 65 % or more.
        Good = ("good", "Good"),
        /// 40 % or more.
        Marginal = ("marginal", "Marginal"),
        /// Below 40 %.
        Poor = ("poor", "Poor"),
        /// No lock.
        NoLock = ("no_lock", "No lock"),
    }
}

/// The quality class of a score from 0 to 100, or [`Quality::NoLock`] when the receiver is not `locked`.
pub fn quality_of(score: f64, locked: bool) -> Quality {
    if !locked {
        Quality::NoLock
    } else if score >= 85.0 {
        Quality::Excellent
    } else if score >= 65.0 {
        Quality::Good
    } else if score >= 40.0 {
        Quality::Marginal
    } else {
        Quality::Poor
    }
}

/// Reports of good and bad words that the score keeps.
const HISTORY: usize = 12;

/// The quality score from the good and bad Reed-Solomon words of the last reports.
#[derive(Clone, Debug)]
pub struct QualityScore {
    hist: VecDeque<(u64, u64)>,
    last: (u64, u64),
    /// The smoothed score, 0 to 100.
    pub score: f64,
}

impl Default for QualityScore {
    fn default() -> Self {
        QualityScore {
            hist: VecDeque::with_capacity(HISTORY + 1),
            last: (0, 0),
            score: 0.0,
        }
    }
}

impl QualityScore {
    /// Adds the running totals of good and bad words. `payload` says whether packets come out.
    pub fn update(&mut self, good: u64, bad: u64, payload: bool) {
        let d = (
            good.saturating_sub(self.last.0),
            bad.saturating_sub(self.last.1),
        );
        self.last = (good, bad);
        self.hist.push_back(d);
        while self.hist.len() > HISTORY {
            self.hist.pop_front();
        }
        let (g, b) = self
            .hist
            .iter()
            .fold((0u64, 0u64), |a, x| (a.0 + x.0, a.1 + x.1));
        let share = if g + b == 0 {
            0.0
        } else {
            g as f64 / (g + b) as f64
        };
        let mut s = 100.0 * share * share;
        if !payload {
            s *= 0.5;
        }
        self.score += 0.25 * (s - self.score);
    }

    /// The quality class of the score.
    pub fn class(&self, locked: bool) -> Quality {
        quality_of(self.score, locked)
    }

    /// Starts again.
    pub fn reset(&mut self) {
        self.hist.clear();
        self.last = (0, 0);
        self.score = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_classes() {
        assert_eq!(quality_of(90.0, true), Quality::Excellent);
        assert_eq!(quality_of(70.0, true), Quality::Good);
        assert_eq!(quality_of(50.0, true), Quality::Marginal);
        assert_eq!(quality_of(10.0, true), Quality::Poor);
        let mut q = QualityScore::default();
        for i in 1..40 {
            q.update(i * 100, 0, true);
        }
        assert!(q.score > 95.0);
        assert_eq!(q.class(false), Quality::NoLock);
    }
}
