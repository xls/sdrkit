//! [`Every`]: a counter for results that a component updates a few times a second of signal.

/// Counts items and says when an interval of signal time has passed: for results updated a few times a second of signal, so a
/// stream cut into pieces of any size produces the same reports.
///
/// ```
/// use sdrkit_graph::Every;
/// let mut e = Every::new(1000.0, 0.1); // every 100 items at 1 kHz
/// assert!(!e.tick(60));
/// assert!(e.tick(60));
/// assert!(!e.tick(60));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Every {
    interval: u64,
    count: u64,
}

impl Every {
    /// Every `seconds` of a stream at `rate` items per second (at least every item).
    pub fn new(rate: f64, seconds: f64) -> Self {
        let n = rate * seconds;
        Every {
            interval: if n.is_finite() && n >= 1.0 {
                n as u64
            } else {
                1
            },
            count: 0,
        }
    }
    /// Counts `n` more items; true when an interval ended among them.
    pub fn tick(&mut self, n: usize) -> bool {
        self.count += n as u64;
        if self.count >= self.interval {
            self.count %= self.interval;
            true
        } else {
            false
        }
    }
    /// The items left before the end of the present interval (at least 1).
    pub fn left(&self) -> usize {
        (self.interval.max(1) - self.count) as usize
    }
    /// Starts counting again.
    pub fn reset(&mut self) {
        self.count = 0;
    }
}
