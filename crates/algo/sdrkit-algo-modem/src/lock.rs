//! The one lock test every synchroniser uses, and the gear shift that changes a loop's bandwidth with it.
//!
//! [`LockDetector`] smooths a lock metric (each synchroniser says which: an eye-opening ratio, a phase-error statistic, a band SNR)
//! with a one-pole average and compares it with two thresholds: it locks when the smoothed metric rises to `on` and unlocks only
//! when it falls below `off`, so a metric near one threshold does not make the lock state flicker. [`Gears`] holds the pull-in and
//! tracking bandwidths of a loop; a synchroniser switches between them when its lock detector changes state.
//! [`HalfRateFalseLock`] finds a carrier loop that locked half its update rate off the true frequency.
//!
//! ```
//! use sdrkit_algo_modem::lock::LockDetector;
//! let mut l = LockDetector::new(20.0, 0.6, 0.4).unwrap();
//! for _ in 0..200 { l.update(0.9); }
//! assert!(l.locked());
//! for _ in 0..20 { l.update(0.5); } // between the thresholds: stays locked
//! assert!(l.locked());
//! ```

use crate::{Complex32, ModemError, Param};

/// Marks the path of a value that is not a number as rare: the test for it is then a branch, and the sum or mean that
/// the value goes into does not wait for a selection.
#[cold]
#[inline(never)]
pub(crate) fn rare() {}

/// A smoothed lock metric with hysteresis (module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct LockDetector {
    alpha: f32,
    on: f32,
    off: f32,
    metric: f32,
    locked: bool,
}

impl LockDetector {
    /// A detector averaging over about `updates` updates, locking at `on` and unlocking below `off`. Errors unless `updates >= 1`
    /// and `off <= on` (all finite).
    pub fn new(updates: f32, on: f32, off: f32) -> Result<Self, ModemError> {
        if !(updates >= 1.0 && updates.is_finite()) {
            return Param::Averaging.at_least(1.0, updates as f64);
        }
        Param::Threshold.check_finite(on as f64)?;
        Param::Threshold.check_finite(off as f64)?;
        if off > on {
            return Param::CloseLevel.conflict(Param::OpenLevel);
        }
        Ok(LockDetector {
            alpha: 1.0 / updates,
            on,
            off,
            metric: 0.0,
            locked: false,
        })
    }

    /// Takes one value of the metric (NaN is ignored); returns whether the loop is locked.
    #[inline]
    pub fn update(&mut self, x: f32) -> bool {
        if x.is_finite() {
            self.metric += self.alpha * (x - self.metric);
        } else {
            // A branch, not a selection: the mean of the next value does not wait for this test.
            rare();
        }
        if self.locked {
            if self.metric < self.off {
                self.locked = false;
            }
        } else if self.metric >= self.on {
            self.locked = true;
        }
        self.locked
    }

    /// The smoothed metric.
    pub fn metric(&self) -> f32 {
        self.metric
    }

    /// Whether locked.
    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Unlocked, metric 0.
    pub fn reset(&mut self) {
        self.metric = 0.0;
        self.locked = false;
    }
}

/// The two bandwidths of a loop: wide to pull in, narrow to track once locked (each normalised to the loop's update rate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gears {
    /// Noise bandwidth while acquiring.
    pub pull_in_bn_t: f64,
    /// Noise bandwidth once locked.
    pub track_bn_t: f64,
}

impl Gears {
    /// The bandwidth for a lock state.
    pub fn bn_t(&self, locked: bool) -> f64 {
        if locked {
            self.track_bn_t
        } else {
            self.pull_in_bn_t
        }
    }
}

/// A test for the half-rate false lock of a carrier loop on a data signal whose symbols last two or more loop updates (a GNSS
/// navigation bit over 20 code periods, for example).
///
/// A loop can lock half the update rate off the true frequency. Then the loop outputs change sign at every update, and the data
/// looks like a symbol per update. The test compares `sum |y[k] + y[k-1]|^2` with `sum |y[k] - y[k-1]|^2` over a window of updates.
/// On a true lock, consecutive outputs mostly carry the same symbol, so the sum is larger. On a half-rate false lock, the difference
/// is larger. If the test fires, move the loop frequency by half the update rate.
///
/// ```
/// use sdrkit_algo_modem::lock::HalfRateFalseLock;
/// use sdrkit_algo_modem::Complex32;
/// let mut f = HalfRateFalseLock::new(0, 100).unwrap();
/// let mut verdict = None;
/// for k in 0..100 {
///     // Symbols of 20 updates, turned by half a cycle per update.
///     let d = if (k / 20) % 2 == 0 { 1.0 } else { -1.0 };
///     let y = Complex32::new(if k % 2 == 0 { d } else { -d }, 0.0);
///     verdict = verdict.or(f.push(y));
/// }
/// assert_eq!(verdict, Some(true));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HalfRateFalseLock {
    skip: u32,
    window: u32,
    count: u32,
    sum: f64,
    diff: f64,
    prev: Complex32,
}

impl HalfRateFalseLock {
    /// A test that ignores the first `skip` updates (while the loop settles), then adds over `window` updates and decides once.
    /// Errors if `window` is 0.
    pub fn new(skip: u32, window: u32) -> Result<Self, ModemError> {
        if window == 0 {
            return Param::Window.at_least(1.0, 0.0);
        }
        Ok(HalfRateFalseLock {
            skip,
            window,
            count: 0,
            sum: 0.0,
            diff: 0.0,
            prev: Complex32::default(),
        })
    }

    /// Takes one loop output (a prompt correlation after the carrier loop). Returns `Some(true)` at the end of the window if the
    /// loop is half the update rate off, `Some(false)` at the end if it is not, and `None` at every other update. After the
    /// decision, the test does nothing until [`reset`](Self::reset). Outputs that are not finite count as zero.
    pub fn push(&mut self, y: Complex32) -> Option<bool> {
        let y = if y.re.is_finite() && y.im.is_finite() {
            y
        } else {
            Complex32::default()
        };
        let end = self.skip.saturating_add(self.window);
        if self.count >= end {
            return None;
        }
        self.count += 1;
        if self.count > self.skip {
            self.sum += (y + self.prev).norm_sqr() as f64;
            self.diff += (y - self.prev).norm_sqr() as f64;
        }
        self.prev = y;
        if self.count == end {
            Some(self.diff > self.sum)
        } else {
            None
        }
    }

    /// Starts again: the skip, then a new window.
    pub fn reset(&mut self) {
        self.count = 0;
        self.sum = 0.0;
        self.diff = 0.0;
        self.prev = Complex32::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hysteresis_and_errors() {
        let mut l = LockDetector::new(4.0, 0.5, 0.2).unwrap();
        assert!(!l.update(0.4));
        for _ in 0..30 {
            l.update(1.0);
        }
        assert!(l.locked());
        for _ in 0..30 {
            l.update(0.3);
        }
        assert!(l.locked());
        for _ in 0..30 {
            l.update(0.0);
        }
        assert!(!l.locked());
        l.update(f32::NAN);
        assert!(l.metric().is_finite());
        assert!(LockDetector::new(0.5, 0.5, 0.2).is_err());
        assert!(LockDetector::new(4.0, 0.1, 0.2).is_err());
        let g = Gears {
            pull_in_bn_t: 0.05,
            track_bn_t: 0.005,
        };
        assert_eq!(g.bn_t(true), 0.005);
    }

    #[test]
    fn half_rate_false_lock() {
        // GPS-like: 20 updates per bit, a settling time of 100 updates, a window of 500.
        let bits = |k: usize| {
            if (k / 20 * 7 + 3) % 5 < 2 {
                1.0f32
            } else {
                -1.0
            }
        };
        for (false_lock, want) in [(false, false), (true, true)] {
            let mut f = HalfRateFalseLock::new(100, 500).unwrap();
            let mut got = None;
            for k in 0..700 {
                let s = if false_lock && k % 2 == 1 { -1.0 } else { 1.0 };
                let noise = ((k * 37 % 11) as f32 - 5.0) * 0.05;
                let r = f.push(Complex32::new(bits(k) * s + noise, noise));
                if r.is_some() {
                    assert_eq!(k, 599);
                    got = r;
                }
            }
            assert_eq!(got, Some(want));
            f.reset();
            assert_eq!(f.push(Complex32::new(1.0, 0.0)), None);
        }
        assert!(HalfRateFalseLock::new(10, 0).is_err());
    }
}
