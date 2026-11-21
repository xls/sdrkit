//! Small parts that several stacks of this crate use: a score kernel and the time of the first sample.

use sdrkit_core::results::SeriesId;
use sdrkit_core::Results;

/// `max(0, 1 - |x - t| / tol)`.
pub fn close_to(x: f64, t: f64, tol: f64) -> f64 {
    (1.0 - (x - t).abs() / tol).max(0.0)
}

sdrkit_core::choice_enum! {
    /// Where the times of a receiver come from.
    pub enum ClockSource {
        /// The computer clock (a live radio), UTC.
        System = ("system", "System clock (UTC)"),
        /// The start time of a recording.
        Recording = ("recording", "The recording's start time"),
        /// Seconds since the first sample (no clock).
        Stream = ("stream", "Stream time (no clock)"),
    }
}

/// The time of a receiver's first sample: the system clock for a live radio, else stream time 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Origin {
    /// Seconds since 1970 UTC of sample 0 (0 for stream time).
    pub unix_s: f64,
    /// Whether the origin is a real clock.
    pub valid: bool,
    /// The origin is the computer clock (a live radio).
    pub system: bool,
}

impl Origin {
    /// The origin of samples that start now: the system clock if `live`, else stream time.
    pub fn new(live: bool) -> Origin {
        if !live {
            return Origin::default();
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        Origin {
            unix_s: now,
            valid: true,
            system: true,
        }
    }

    /// Where the times come from.
    pub fn source(&self) -> ClockSource {
        if self.system {
            ClockSource::System
        } else if self.valid {
            ClockSource::Recording
        } else {
            ClockSource::Stream
        }
    }

    /// The absolute time of stream second `t`.
    pub fn at(&self, t: f64) -> f64 {
        self.unix_s + t
    }

    /// The wall time of stream second `t`, if the origin is a real clock.
    pub fn wall(&self, t: f64) -> Option<f64> {
        self.valid.then(|| self.at(t))
    }
}

/// Makes series `id` of `r` hold `points` (drawn as a line if `line`), if they differ from what it holds. Allocates nothing once
/// the series has reached its size.
pub fn update_points<I>(r: &mut Results, id: SeriesId, line: bool, points: I)
where
    I: Iterator<Item = [f32; 2]> + Clone,
{
    r.update_series(id, |s| {
        let same = s.line == line
            && s.points.len() == points.clone().count()
            && s.points.iter().zip(points.clone()).all(|(a, b)| *a == b);
        if same {
            return false;
        }
        s.line = line;
        s.points.clear();
        s.points.extend(points);
        true
    });
}

/// The points of a level history, one a second: `[i, level]`.
pub fn level_points(
    level: &std::collections::VecDeque<f32>,
) -> impl Iterator<Item = [f32; 2]> + Clone + '_ {
    level.iter().enumerate().map(|(i, &v)| [i as f32, v])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(close_to(2400.0, 2400.0, 80.0), 1.0);
        assert_eq!(close_to(2600.0, 2400.0, 80.0), 0.0);
        assert_eq!(Origin::new(false).source(), ClockSource::Stream);
    }
}
