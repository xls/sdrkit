//! What a stream from a server across a network reads of its link while it runs: over each window, the part of the
//! rate that arrived. It keeps no figure of what the link carries: a window that is short gives a notice, and the
//! status line names the samples that the link drops until the stream gets its rate again.

use crate::Hz;
use std::sync::Mutex;

/// A window that gets less than this part of the rate is short of the link.
pub const SHORT: f64 = 0.9;
/// Short windows in a row before the notice. One short window is a stall of the link.
const SHORT_RUN: u32 = 2;

#[derive(Debug, Default)]
struct State {
    run: u32,
    /// The notice was given for the present run of short windows.
    told: bool,
    /// Windows still to skip after a start or a change of rate.
    skip: u32,
    /// The part of the rate that arrived over the last window.
    part: Option<f64>,
}

/// The live reading of the link of one stream.
#[derive(Debug, Default)]
pub struct LinkWatch {
    state: Mutex<State>,
}

impl LinkWatch {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The part of the rate that arrived over the last window, if one was counted.
    pub fn part(&self) -> Option<f64> {
        self.lock().part
    }

    /// A stream started or took another rate: the next window is not counted (the connection starts up, or what the
    /// server queued at the rate before arrives), and the reading starts again.
    pub fn restarted(&self) {
        let mut s = self.lock();
        *s = State {
            skip: 1,
            ..State::default()
        };
    }

    /// Takes a window over which `arrived_hz` samples a second arrived of a stream at `rate_hz`. True once for each run
    /// of [`SHORT_RUN`] or more short windows: the driver then gives its notice.
    pub fn window(&self, arrived_hz: Hz, rate_hz: Hz) -> bool {
        let mut s = self.lock();
        if s.skip > 0 {
            s.skip -= 1;
            return false;
        }
        if !(rate_hz > 0.0 && arrived_hz.is_finite() && arrived_hz >= 0.0) {
            return false;
        }
        let part = arrived_hz / rate_hz;
        s.part = Some(part);
        if part >= SHORT {
            (s.run, s.told) = (0, false);
            return false;
        }
        s.run += 1;
        if s.run < SHORT_RUN || s.told {
            return false;
        }
        s.told = true;
        true
    }
}

/// The status of a stream at `asked_hz` that loses part of its samples on the link, or `None` while nearly all arrive:
/// "2.1 of 2.4 Msps arrive (88 %): samples are lost" ([`crate::arrive_text`]).
pub fn loss_text(part: Option<f64>, asked_hz: f64) -> Option<String> {
    let p = part?;
    (p < 0.97).then(|| format!("{}: samples are lost", crate::arrive_text(p * asked_hz, asked_hz)))
}

/// The status of a stream that loses part of its samples on the link, or `None` while nearly all arrive: "the link
/// drops 12 % of the samples". For a driver that does not know the asked rate; [`loss_text`] where it does.
pub fn drop_text(part: Option<f64>) -> Option<String> {
    let p = part?;
    (p < 0.97).then(|| {
        format!(
            "the link drops {:.0} % of the samples",
            ((1.0 - p) * 100.0).clamp(0.0, 100.0)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_short_windows_give_one_notice_and_the_reading_clears_by_itself() {
        let w = LinkWatch::default();
        w.restarted();
        // The first window after a start is not counted.
        assert!(!w.window(0.1e6, 2.4e6));
        assert_eq!(w.part(), None);
        // A stall, then the stream catches up: no notice.
        assert!(!w.window(1.0e6, 2.4e6));
        assert!(!w.window(3.0e6, 2.4e6));
        // Two short windows in a row: one notice for the run.
        assert!(!w.window(1.2e6, 2.4e6));
        assert!(w.window(1.0e6, 2.4e6));
        assert!(!w.window(1.0e6, 2.4e6));
        assert_eq!(
            drop_text(w.part()).as_deref(),
            Some("the link drops 58 % of the samples")
        );
        // The rate arrives again: the status line clears.
        assert!(!w.window(2.4e6, 2.4e6));
        assert_eq!(drop_text(w.part()), None);
    }
}
