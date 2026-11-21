//! The clock that the time stacks compare the received time with: none for a file, the host clock for a radio, or a fixed
//! origin for tests. The settings of the comparison are shared by the stacks that use it.

use sdrkit_core::publish::field;
use sdrkit_core::Setting;

sdrkit_core::choice_enum! {
    /// What the received time is compared with.
    pub enum ClockChoice {
        /// No clock (a file).
        None = ("none", "None (a file)"),
        /// The host clock as the samples arrive (a radio).
        Live = ("live", "Host clock (a radio)"),
        /// A fixed origin: sample 0 was taken at the `clock_origin` setting (tests).
        Fixed = ("fixed", "Fixed origin (tests)"),
    }
}

/// What the received time is compared with.
pub const CLOCK_MODE: Setting<field::Choice<ClockChoice>> =
    Setting::choice("clock_mode", "Clock to compare", ClockChoice::Live);
/// The UTC of sample 0 for the fixed clock, Unix seconds.
pub const CLOCK_ORIGIN: Setting<field::F64> =
    Setting::new("clock_origin", "Sample 0 at (UTC, Unix s)", "s", 0.0).range(0.0, 1e11, 0.0);

/// The clock in use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ClockMode {
    /// A file: no clock to compare.
    None,
    /// A radio: the host clock as the samples arrive.
    Live,
    /// Tests: sample 0 was taken at this UTC epoch second.
    Fixed(f64),
}

impl ClockMode {
    /// The clock of a choice and the fixed origin.
    pub fn of(choice: ClockChoice, origin: f64) -> ClockMode {
        match choice {
            ClockChoice::None => ClockMode::None,
            ClockChoice::Live => ClockMode::Live,
            ClockChoice::Fixed => ClockMode::Fixed(origin),
        }
    }
    /// The choice of this clock.
    pub fn choice(self) -> ClockChoice {
        match self {
            ClockMode::None => ClockChoice::None,
            ClockMode::Live => ClockChoice::Live,
            ClockMode::Fixed(_) => ClockChoice::Fixed,
        }
    }
}

/// The live clock estimate: sample 0's wall time is the earliest arrival over the last 30 s window.
#[derive(Clone, Debug)]
pub struct LiveClock {
    cur_min: f64,
    prev_min: f64,
    window_start: f64,
}

impl Default for LiveClock {
    fn default() -> Self {
        LiveClock {
            cur_min: f64::INFINITY,
            prev_min: f64::INFINITY,
            window_start: f64::NEG_INFINITY,
        }
    }
}

impl LiveClock {
    /// Records an arrival: `wall` the host time now, `signal_t` the signal time of the newest sample.
    pub fn arrive(&mut self, wall: f64, signal_t: f64) {
        let origin = wall - signal_t;
        if wall - self.window_start > 30.0 {
            self.prev_min = self.cur_min;
            self.cur_min = f64::INFINITY;
            self.window_start = wall;
        }
        self.cur_min = self.cur_min.min(origin);
    }
    /// The wall time of sample 0.
    pub fn origin(&self) -> Option<f64> {
        let o = if self.prev_min.is_finite() {
            self.prev_min
        } else {
            self.cur_min
        };
        o.is_finite().then_some(o)
    }
}
