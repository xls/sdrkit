//! When a thing was heard.

use crate::persist::Shift;
use serde::{Deserialize, Serialize};

/// A time stamp: signal time, and the wall-clock time when the host supplies it.
///
/// ```
/// use sdrkit_util_objects::Stamp;
/// let s = Stamp::at(12.5).with_wall(1_762_646_400.0);
/// assert_eq!((s.t, s.wall), (12.5, Some(1_762_646_400.0)));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// Signal time, seconds.
    pub t: f64,
    /// Wall-clock time, seconds since 1970 (UTC), when known.
    pub wall: Option<f64>,
}

impl Stamp {
    /// Signal time `t` seconds, wall time unknown.
    pub const fn at(t: f64) -> Stamp {
        Stamp { t, wall: None }
    }
    /// The same moment with its wall-clock time (seconds since 1970, UTC).
    pub const fn with_wall(self, wall: f64) -> Stamp {
        Stamp {
            t: self.t,
            wall: Some(wall),
        }
    }
    /// Whether the signal time (and the wall time, when given) are finite.
    pub fn is_valid(&self) -> bool {
        self.t.is_finite() && self.wall.is_none_or(f64::is_finite)
    }
}

/// When a row was first and last heard, and how many times.
///
/// ```
/// use sdrkit_util_objects::{Seen, Stamp};
/// let mut s = Seen::new(Stamp::at(10.0));
/// s.heard(Stamp::at(25.0));
/// assert_eq!((s.first_s, s.last_s, s.count), (10.0, 25.0, 2));
/// assert_eq!(s.age(40.0), 15.0);
/// assert!(s.within(40.0, 20.0) && !s.within(40.0, 10.0));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    /// First heard, signal seconds.
    pub first_s: f64,
    /// Last heard, signal seconds (the latest time given. A late report never moves it back).
    pub last_s: f64,
    /// First heard, wall-clock seconds since 1970, when known.
    pub first_wall: Option<f64>,
    /// Last heard, wall-clock seconds since 1970, when known.
    pub last_wall: Option<f64>,
    /// Times heard.
    pub count: u64,
}

impl Seen {
    /// Heard once, at `at`.
    pub fn new(at: Stamp) -> Seen {
        Seen {
            first_s: at.t,
            last_s: at.t,
            first_wall: at.wall,
            last_wall: at.wall,
            count: 1,
        }
    }
    /// Heard again at `at`.
    pub fn heard(&mut self, at: Stamp) {
        self.count = self.count.saturating_add(1);
        if at.t >= self.last_s {
            self.last_s = at.t;
            if at.wall.is_some() {
                self.last_wall = at.wall;
            }
        }
        if at.t < self.first_s {
            self.first_s = at.t;
        }
        if self.first_wall.is_none() {
            self.first_wall = at.wall;
        }
    }
    /// When it was last heard, with the wall time when known.
    pub fn last(&self) -> Stamp {
        Stamp {
            t: self.last_s,
            wall: self.last_wall,
        }
    }
    /// Seconds since it was last heard, at signal time `now_s` (never negative).
    pub fn age(&self, now_s: f64) -> f64 {
        (now_s - self.last_s).max(0.0)
    }
    /// Whether it was heard within the last `window_s` seconds at `now_s` (a lock state's "last good within N seconds").
    pub fn within(&self, now_s: f64, window_s: f64) -> bool {
        now_s - self.last_s <= window_s
    }
    /// Seconds from first to last heard.
    pub fn span(&self) -> f64 {
        self.last_s - self.first_s
    }
}

impl Shift for Seen {
    fn shift_times(&mut self, by_s: f64) {
        self.first_s += by_s;
        self.last_s += by_s;
    }
}

impl Shift for Stamp {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}
