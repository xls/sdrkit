//! What the Wi-Fi decoder leaves out when live input comes faster than it works. The decoder gives the time each piece took
//! and the signal time it held; the level goes up when the work is behind and down again after a time of light work.

/// What the decoder leaves out, in the order it sheds work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Shed {
    /// Nothing.
    #[default]
    Nothing,
    /// The chains on the channels next to the tuned one.
    Neighbours,
    /// Also the HT and VHT data fields. Management frames are non-HT or DSSS frames: they stay.
    Data,
}

impl Shed {
    /// What is left out, in plain words; `None` when nothing is.
    pub fn text(self) -> Option<&'static str> {
        match self {
            Shed::Nothing => None,
            Shed::Neighbours => Some("decoding the tuned channel only: the processor is behind"),
            Shed::Data => {
                Some("decoding only the DSSS and non-HT frames of the tuned channel, the beacons among them: the processor is behind")
            }
        }
    }

    fn up(self) -> Shed {
        match self {
            Shed::Nothing => Shed::Neighbours,
            _ => Shed::Data,
        }
    }

    fn down(self) -> Shed {
        match self {
            Shed::Data => Shed::Neighbours,
            _ => Shed::Nothing,
        }
    }
}

/// The work may be this far behind the signal before the level goes up, s.
pub const LAG_S: f64 = 0.15;
/// The level goes down only while the work takes less than this share of the signal time.
pub const LIGHT_LOAD: f64 = 0.45;
/// The first time of light work before the level goes down, s of signal.
pub const HOLD_S: f64 = 5.0;
/// The longest such time, s of signal.
pub const MAX_HOLD_S: f64 = 80.0;
/// The time over which the load is averaged, s of signal.
const LOAD_S: f64 = 1.0;

/// Chooses the level from the work and signal time of each piece.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shedder {
    level: Shed,
    /// The work beyond the signal time, s (never below 0).
    behind_s: f64,
    /// The share of the signal time that the work takes, averaged over [`LOAD_S`].
    load: f64,
    /// The signal time of light work in a row, s.
    light_s: f64,
    /// The time of light work that lowers the level, s. It doubles when the level goes up again soon after it went down.
    hold_s: f64,
    /// The signal time since the level last went down, s.
    since_down_s: f64,
}

impl Default for Shedder {
    fn default() -> Self {
        Shedder {
            level: Shed::Nothing,
            behind_s: 0.0,
            load: 0.0,
            light_s: 0.0,
            hold_s: HOLD_S,
            since_down_s: f64::INFINITY,
        }
    }
}

impl Shedder {
    /// The level now.
    pub fn level(&self) -> Shed {
        self.level
    }

    /// Takes a piece that held `signal_s` of signal and took `work_s` to decode. Returns the level for the next piece.
    pub fn step(&mut self, work_s: f64, signal_s: f64) -> Shed {
        if signal_s <= 0.0 {
            return self.level;
        }
        self.behind_s = (self.behind_s + work_s - signal_s).max(0.0);
        let a = (signal_s / LOAD_S).min(1.0);
        self.load += a * (work_s / signal_s - self.load);
        self.since_down_s += signal_s;
        if self.behind_s > LAG_S {
            // The samples the work was behind are gone: the count starts again at the new level.
            self.behind_s = 0.0;
            self.light_s = 0.0;
            if self.level < Shed::Data {
                self.level = self.level.up();
                if self.since_down_s < 2.0 * self.hold_s {
                    self.hold_s = (2.0 * self.hold_s).min(MAX_HOLD_S);
                }
            }
            return self.level;
        }
        if self.load < LIGHT_LOAD {
            self.light_s += signal_s;
        } else {
            self.light_s = 0.0;
        }
        if self.level > Shed::Nothing && self.light_s >= self.hold_s {
            self.level = self.level.down();
            self.light_s = 0.0;
            self.since_down_s = 0.0;
        } else if self.level == Shed::Nothing && self.since_down_s > 4.0 * self.hold_s {
            self.hold_s = HOLD_S;
        }
        self.level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Feeds `seconds` of signal in pieces of 10 ms that each take `load` times their signal time.
    fn run(s: &mut Shedder, load: f64, seconds: f64) -> Shed {
        for _ in 0..(seconds * 100.0).round() as usize {
            s.step(0.01 * load, 0.01);
        }
        s.level()
    }

    #[test]
    fn the_level_goes_up_while_behind_and_down_after_light_work() {
        let mut s = Shedder::default();
        // Work that keeps up sheds nothing, however long.
        assert_eq!(run(&mut s, 0.9, 60.0), Shed::Nothing);
        // 1.5 times the signal time: behind by 0.15 s after 0.3 s, and again after 0.3 s more.
        assert_eq!(run(&mut s, 1.5, 0.2), Shed::Nothing);
        assert_eq!(run(&mut s, 1.5, 0.2), Shed::Neighbours);
        assert_eq!(run(&mut s, 1.5, 0.4), Shed::Data);
        assert_eq!(run(&mut s, 1.5, 10.0), Shed::Data);
        // Work between the two limits holds the level.
        assert_eq!(run(&mut s, 0.7, 60.0), Shed::Data);
        // Light work lowers it one step after the hold time, not before.
        assert_eq!(run(&mut s, 0.2, 4.0), Shed::Data);
        assert_eq!(run(&mut s, 0.2, 3.0), Shed::Neighbours);
        assert_eq!(run(&mut s, 0.2, 3.0), Shed::Neighbours);
        assert_eq!(run(&mut s, 0.2, 2.0), Shed::Nothing);
    }

    #[test]
    fn a_level_that_comes_back_at_once_is_held_longer() {
        let mut s = Shedder::default();
        assert_eq!(run(&mut s, 2.0, 0.2), Shed::Neighbours);
        // Light at this level; the full work is too much: each try to go back fails, and the next comes later.
        let mut tries: Vec<f64> = Vec::new();
        let mut t = 0.0f64;
        while t < 100.0 {
            let load = if s.level() == Shed::Nothing { 2.0 } else { 0.2 };
            let before = s.level();
            s.step(0.01 * load, 0.01);
            t += 0.01;
            if before == Shed::Neighbours && s.level() == Shed::Nothing {
                tries.push(t);
            }
        }
        let gaps: Vec<f64> = tries.windows(2).map(|w| (w[1] - w[0]).round()).collect();
        assert_eq!((tries[0].round(), gaps), (5.0, vec![10.0, 20.0, 40.0]));
        assert_eq!(Shed::Neighbours.text(), Some("decoding the tuned channel only: the processor is behind"));
    }
}
