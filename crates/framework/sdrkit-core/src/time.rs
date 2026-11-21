//! The wall clock that the host provides.

use std::fmt;
use std::sync::Arc;

/// The host's wall clock. A library crate reads the time of day only through this type.
#[derive(Clone)]
pub struct Clock(Arc<dyn Fn() -> f64 + Send + Sync>);

impl Clock {
    /// A clock that calls `now` for the seconds since 1970 (UTC).
    pub fn new(now: impl Fn() -> f64 + Send + Sync + 'static) -> Self {
        Clock(Arc::new(now))
    }
    /// A clock that always returns `unix_s`.
    pub fn fixed(unix_s: f64) -> Self {
        Clock::new(move || unix_s)
    }
    /// Seconds since 1970 (UTC).
    pub fn now(&self) -> f64 {
        (self.0)()
    }
}

impl fmt::Debug for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Clock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn a_clock_returns_what_its_source_gives() {
        let t = Arc::new(AtomicU64::new(10));
        let source = t.clone();
        let clock = Clock::new(move || source.load(Ordering::Relaxed) as f64);
        assert_eq!(clock.now(), 10.0);
        t.store(25, Ordering::Relaxed);
        assert_eq!(clock.now(), 25.0);
        assert_eq!(Clock::fixed(3.5).now(), 3.5);
    }
}
