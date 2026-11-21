//! Bits from the times between level changes, with a clock that follows the intervals: Manchester half-bits and biphase mark
//! (differential bi-phase) bits.
//!
//! An [`IntervalClock`] holds the length `T` of one unit and sorts each interval `dt` by `r = dt / T` into one of two windows
//! ([`IntervalWindows`]). A sorted interval moves the clock towards it: `T += g (dt / u - T)` with the gain `g` and the units
//! `u` of its window, clamped to a share of the nominal unit. An interval outside both windows sets `T` back to the nominal.
//!
//! - [`ManchesterHalves`]: the unit is a half-bit; one unit is one half-bit of the level that lasted, two units are two. Every
//!   Manchester bit has a change in its middle, so no other interval occurs.
//! - [`BiphaseMarkEdges`]: the unit is a bit; a change starts every bit and a 1 has one more in its middle, so a full interval
//!   is a 0 and two half intervals are a 1. [`BiphaseMarkEdges::idle`] takes the last 1 of a stream, whose second half has no
//!   change after it. `sdrkit_util_bits::line::Biphase` with `BiphaseKind::Mark` makes the chips of this code.
//!
//! ```
//! use sdrkit_algo_modem::halfbit::{BiphaseMarkEdges, BiphaseOut, IntervalWindows};
//! let mut d = BiphaseMarkEdges::new(IntervalWindows::biphase(500.0, 0.12));
//! // A 0 (one full bit), then a 1 (two halves).
//! assert_eq!(d.edge(500.0), BiphaseOut::Bit(0));
//! assert_eq!(d.edge(250.0), BiphaseOut::Half);
//! assert_eq!(d.edge(250.0), BiphaseOut::Bit(1));
//! assert_eq!(d.edge(2000.0), BiphaseOut::Lost);
//! ```

/// The two windows of an [`IntervalClock`] and the rules of its clock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervalWindows {
    /// The nominal unit, in the caller's time unit.
    pub nominal: f64,
    /// The first window of `dt / T`: from (included) and to (excluded).
    pub one: (f64, f64),
    /// The second window of `dt / T`: from (included) and to (included).
    pub two: (f64, f64),
    /// The units of an interval of the first window, for the clock.
    pub one_units: f64,
    /// The units of an interval of the second window.
    pub two_units: f64,
    /// The gain of the clock on an interval of the first window.
    pub one_gain: f64,
    /// The gain on an interval of the second window.
    pub two_gain: f64,
    /// The clock stays within this share of the nominal unit, either way.
    pub clamp: f64,
}

impl IntervalWindows {
    /// Manchester half-bits of nominal length `half`: one half-bit for `0.6 <= r < 1.45`, two for `1.6 <= r <= 2.45`; gains
    /// 0.05 and 0.02; the clock within 15 %.
    pub fn manchester(half: f64) -> IntervalWindows {
        IntervalWindows {
            nominal: half,
            one: (0.6, 1.45),
            two: (1.6, 2.45),
            one_units: 1.0,
            two_units: 2.0,
            one_gain: 0.05,
            two_gain: 0.02,
            clamp: 0.15,
        }
    }

    /// Biphase mark bits of nominal length `bit`: a half for `0.3 <= r < 0.75`, a full bit for `0.75 <= r <= 1.4`; gain 0.05;
    /// the clock within the share `tolerance`.
    pub fn biphase(bit: f64, tolerance: f64) -> IntervalWindows {
        IntervalWindows {
            nominal: bit,
            one: (0.3, 0.75),
            two: (0.75, 1.4),
            one_units: 0.5,
            two_units: 1.0,
            one_gain: 0.05,
            two_gain: 0.05,
            clamp: tolerance,
        }
    }
}

/// The window of an interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interval {
    /// The first window.
    One,
    /// The second window.
    Two,
    /// Neither.
    Outside,
}

/// A unit clock that follows the intervals it sorts (module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IntervalClock {
    w: IntervalWindows,
    t: f64,
}

impl IntervalClock {
    /// A clock at the nominal unit of `w`.
    pub fn new(w: IntervalWindows) -> IntervalClock {
        IntervalClock { w, t: w.nominal }
    }

    /// The windows.
    pub fn windows(&self) -> &IntervalWindows {
        &self.w
    }

    /// The unit now.
    pub fn unit(&self) -> f64 {
        self.t
    }

    /// Sets the unit back to the nominal.
    pub fn reset(&mut self) {
        self.t = self.w.nominal;
    }

    /// The window of `dt`.
    pub fn classify(&self, dt: f64) -> Interval {
        let r = dt / self.t;
        if r >= self.w.one.0 && r < self.w.one.1 {
            Interval::One
        } else if r >= self.w.two.0 && r <= self.w.two.1 {
            Interval::Two
        } else {
            Interval::Outside
        }
    }

    /// Moves the clock towards `dt` of window `which`; an interval outside sets it back to the nominal.
    pub fn follow(&mut self, dt: f64, which: Interval) {
        let (u, g) = match which {
            Interval::One => (self.w.one_units, self.w.one_gain),
            Interval::Two => (self.w.two_units, self.w.two_gain),
            Interval::Outside => {
                self.reset();
                return;
            }
        };
        self.t += g * (dt / u - self.t);
        let n = self.w.nominal;
        self.t = self
            .t
            .clamp(n * (1.0 - self.w.clamp), n * (1.0 + self.w.clamp));
    }
}

/// Manchester half-bits from intervals (module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManchesterHalves {
    clock: IntervalClock,
}

impl ManchesterHalves {
    /// A reader with the windows `w` (see [`IntervalWindows::manchester`]).
    pub fn new(w: IntervalWindows) -> ManchesterHalves {
        ManchesterHalves {
            clock: IntervalClock::new(w),
        }
    }

    /// The clock.
    pub fn clock(&self) -> &IntervalClock {
        &self.clock
    }

    /// Sets the clock back to the nominal.
    pub fn reset(&mut self) {
        self.clock.reset();
    }

    /// The level changed `dt` after the previous change: the half-bits that lasted (1 or 2), or `None` for an interval
    /// outside both windows (the half-bits so far no longer join the next ones, and the clock is back at the nominal).
    pub fn edge(&mut self, dt: f64) -> Option<usize> {
        let w = self.clock.classify(dt);
        self.clock.follow(dt, w);
        match w {
            Interval::One => Some(1),
            Interval::Two => Some(2),
            Interval::Outside => None,
        }
    }
}

/// What one level change gives to a [`BiphaseMarkEdges`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BiphaseOut {
    /// A bit.
    Bit(u8),
    /// The first half of a 1.
    Half,
    /// A full interval after a held half: the half is dropped and the full interval is a 0. A framing error.
    Framing,
    /// The interval fits neither window: the bit stream starts again.
    Lost,
}

/// Biphase mark bits from intervals (module docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiphaseMarkEdges {
    clock: IntervalClock,
    half: bool,
}

impl BiphaseMarkEdges {
    /// A reader with the windows `w` (see [`IntervalWindows::biphase`]).
    pub fn new(w: IntervalWindows) -> BiphaseMarkEdges {
        BiphaseMarkEdges {
            clock: IntervalClock::new(w),
            half: false,
        }
    }

    /// The clock.
    pub fn clock(&self) -> &IntervalClock {
        &self.clock
    }

    /// A half of a 1 is held.
    pub fn is_holding(&self) -> bool {
        self.half
    }

    /// Drops a held half and sets the clock back to the nominal.
    pub fn reset(&mut self) {
        self.half = false;
        self.clock.reset();
    }

    /// The level changed `dt` after the previous change. The clock follows a full interval and the second half of a 1.
    pub fn edge(&mut self, dt: f64) -> BiphaseOut {
        match self.clock.classify(dt) {
            Interval::Outside => {
                self.reset();
                BiphaseOut::Lost
            }
            Interval::Two => {
                self.clock.follow(dt, Interval::Two);
                if std::mem::take(&mut self.half) {
                    BiphaseOut::Framing
                } else {
                    BiphaseOut::Bit(0)
                }
            }
            Interval::One => {
                if std::mem::take(&mut self.half) {
                    self.clock.follow(dt, Interval::One);
                    BiphaseOut::Bit(1)
                } else {
                    self.half = true;
                    BiphaseOut::Half
                }
            }
        }
    }

    /// No change has come for `since` after the last one: with a half held and `since` at least 0.8 of a bit, the 1 is taken
    /// (the last bit of a stream).
    pub fn idle(&mut self, since: f64) -> Option<u8> {
        (self.half && since >= 0.8 * self.clock.unit()).then(|| {
            self.half = false;
            1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manchester_halves_follow_a_clock_4_percent_fast() {
        let mut d = ManchesterHalves::new(IntervalWindows::manchester(256.0));
        let k = 256.0 / 1.04;
        let mut n = 0;
        for i in 0..400 {
            let dt = if i % 3 == 0 { 2.0 * k } else { k };
            n += d.edge(dt).expect("inside a window");
        }
        assert_eq!(n, 400 + 134);
        assert!((d.clock().unit() - k).abs() < 2.0, "{}", d.clock().unit());
        assert_eq!(d.edge(5.0 * k), None);
        assert_eq!(d.clock().unit(), 256.0);
    }

    #[test]
    fn biphase_mark_bits_come_back_from_their_intervals() {
        let bits = [1u8, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1];
        let mut iv = Vec::new();
        for &b in &bits {
            if b == 1 {
                iv.extend([250.0, 250.0]);
            } else {
                iv.push(500.0);
            }
        }
        // The last 1 has no change after its second half.
        iv.pop();
        let mut d = BiphaseMarkEdges::new(IntervalWindows::biphase(500.0, 0.12));
        let mut got: Vec<u8> = iv
            .iter()
            .filter_map(|&i| match d.edge(i * 0.96) {
                BiphaseOut::Bit(b) => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(d.idle(300.0), None);
        got.extend(d.idle(420.0));
        assert_eq!(got, bits);
        // A full interval after a held half is a framing error.
        let mut d = BiphaseMarkEdges::new(IntervalWindows::biphase(500.0, 0.12));
        assert_eq!(d.edge(250.0), BiphaseOut::Half);
        assert_eq!(d.edge(500.0), BiphaseOut::Framing);
    }
}
