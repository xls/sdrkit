//! The clock every marine display keeps and the reception state each shows. The clock provides signal time that runs on across
//! retunes, and the wall-clock time of a signal time when it is known.

use sdrkit_core::LockState;
use sdrkit_util_objects::Stamp;

/// Signal time across inputs, and wall-clock time when the samples are live or the user supplied the start of a recording.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Clock {
    base_s: f64,
    last_s: f64,
    live: bool,
    wall_at_zero: Option<f64>,
    time_base: Option<f64>,
}

impl Clock {
    /// The input starts again at time 0. `live` is true when the samples come from a radio as they arrive. `unix_s` is the
    /// host's time now, seconds since 1970 (UTC).
    pub fn restart(&mut self, live: bool, unix_s: f64) {
        self.base_s += self.last_s;
        self.last_s = 0.0;
        self.live = live;
        self.wall_at_zero = if live {
            Some(unix_s - self.base_s)
        } else {
            self.time_base
        };
    }

    /// Sets the wall-clock time of signal time 0, the first sample since the display started (seconds since 1970, UTC), or forgets
    /// it with `None`. Live samples keep the computer's clock.
    pub fn set_time_base(&mut self, unix: Option<f64>) {
        self.time_base = unix.filter(|t| t.is_finite() && *t > 0.0);
        if !self.live {
            self.wall_at_zero = self.time_base;
        }
    }

    /// The signal time of an item time of the current input. A time that is not finite returns the current time.
    pub fn at(&mut self, item_s: f64) -> f64 {
        if !item_s.is_finite() {
            return self.now();
        }
        if item_s > self.last_s {
            self.last_s = item_s;
        }
        self.base_s + item_s
    }

    /// The newest signal time seen.
    pub fn now(&self) -> f64 {
        self.base_s + self.last_s
    }

    /// The wall-clock time of a signal time, when it is known.
    pub fn wall(&self, signal_s: f64) -> Option<f64> {
        self.wall_at_zero.map(|w| w + signal_s)
    }

    /// The moment of a signal time, with its wall-clock time when it is known.
    pub fn stamp(&self, signal_s: f64) -> Stamp {
        match self.wall(signal_s) {
            Some(w) => Stamp::at(signal_s).with_wall(w),
            None => Stamp::at(signal_s),
        }
    }
}

sdrkit_core::choice_enum! {
    /// What a marine display hears.
    pub enum Hearing {
        /// No signal.
        Searching = ("searching", "Searching"),
        /// A signal, but no good message.
        Signal = ("signal", "Signal"),
        /// Good messages.
        Decoding = ("decoding", "Decoding"),
    }
}

impl Hearing {
    /// The status lock state of the state.
    pub fn lock_state(self) -> LockState {
        match self {
            Hearing::Decoding => LockState::Locked,
            Hearing::Signal => LockState::Tracking,
            Hearing::Searching => LockState::Searching,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_time_runs_on_across_inputs_and_wall_time_follows_the_base() {
        let mut c = Clock::default();
        c.set_time_base(Some(1_700_000_000.0));
        c.restart(false, 0.0);
        assert_eq!(c.at(10.0), 10.0);
        assert_eq!(c.wall(10.0), Some(1_700_000_010.0));
        c.restart(false, 0.0);
        assert_eq!(c.at(2.0), 12.0);
        assert_eq!(c.wall(12.0), Some(1_700_000_012.0));
        c.set_time_base(None);
        assert_eq!(c.wall(12.0), None);
        assert_eq!(c.at(f64::NAN), 12.0);
    }
}
