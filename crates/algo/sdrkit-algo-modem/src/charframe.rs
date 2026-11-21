//! The asynchronous character framer of start-stop serial links (RTTY, FSK teleprinters, UART-style data): a start bit of
//! space, 5 to 8 data bits least significant first, then a stop bit of mark. The input is one soft decision per sample, positive
//! for mark. The framer waits for a fall from mark to space, finds its time between two samples, reads each bit at its middle,
//! and gives the character when the first stop bit reads mark, with the time and the value of each bit it read. Several
//! framers at other rates, widths or senses can read one stream side by side.
//!
//! Characters sent back to back keep one bit clock. The framer follows it: every crossing of the decisions that lies near the
//! expected place of a bit edge moves that place a part of the way, and once a few crossings agreed, a start edge and the
//! bits after it are read on the clock rather than on the single crossing.

use crate::error::{ModemError, Param};

/// Where a framer is in a character.
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    /// Waiting for the edge of a start bit.
    Idle,
    /// In a character: the bit `k` (0 the start bit, then the data bits, then the stop bit) is read at sample time `at`.
    Bit { k: u8, at: f64, value: u8 },
}

/// One framer: its bit length in samples, its data bits and its sense.
#[derive(Clone, Debug)]
pub struct CharFramer {
    spb: f64,
    bits: u8,
    sense: f32,
    state: State,
    prev: f32,
    t: f64,
    read: Vec<(f64, bool)>,
    decided: Option<bool>,
    framing: bool,
    clock: bool,
    /// The time of a recent bit edge of the clock, and how many crossings in a row agreed with it.
    grid: f64,
    agreed: u32,
    last_crossing: f64,
}

/// A crossing this close to an edge of the clock, in bits, agrees with it.
const NEAR: f64 = 1.0 / 3.0;
/// The part of the distance from the clock to a crossing that the clock moves.
const GAIN: f64 = 0.25;
/// Crossings that must agree before the clock places the bits.
const LOCK: u32 = 4;
/// Agreeing crossings counted at most.
const MAX_AGREED: u32 = 16;
/// A clock without a crossing for this many bits is forgotten (a pause between transmissions).
const STALE_BITS: f64 = 12.0;

impl CharFramer {
    /// A framer of `samples_per_bit` samples a bit (at least 2) and `data_bits` data bits (5 to 8). `inverted`: the decisions are
    /// read with the opposite sign (mark negative).
    pub fn new(samples_per_bit: f64, data_bits: u8, inverted: bool) -> Result<Self, ModemError> {
        if !(samples_per_bit >= 2.0 && samples_per_bit.is_finite()) {
            return Param::Sps.at_least(2.0, samples_per_bit);
        }
        if !(5..=8).contains(&data_bits) {
            return Param::Width.range(5.0, 8.0, f64::from(data_bits));
        }
        Ok(CharFramer {
            spb: samples_per_bit,
            bits: data_bits,
            sense: if inverted { -1.0 } else { 1.0 },
            state: State::Idle,
            prev: 0.0,
            t: 0.0,
            read: Vec::with_capacity(10),
            decided: None,
            framing: false,
            clock: true,
            grid: 0.0,
            agreed: 0,
            last_crossing: 0.0,
        })
    }

    /// The same framer timing every character from its own start edge only, without the bit clock: for a link whose stop
    /// element is not a whole number of bits (1.5 units), where the start edges do not fall on the clock of the bits before.
    pub fn without_clock(mut self) -> Self {
        self.clock = false;
        self
    }

    /// The data bits of a character.
    pub fn data_bits(&self) -> u8 {
        self.bits
    }

    /// The bits of the character that [`CharFramer::push`] returned last: for the start bit, each data bit and the stop
    /// bit, the sample at which it was read (counted from the first sample pushed) and whether it read mark.
    pub fn bits(&self) -> &[(f64, bool)] {
        &self.read
    }

    /// Whether the last [`CharFramer::push`] read a bit, and its value in the stream's sense (true for a positive decision).
    pub fn decided(&self) -> Option<bool> {
        self.decided
    }

    /// Whether the last [`CharFramer::push`] read a stop bit of space: a framing error, the character dropped. Its bits are
    /// in [`CharFramer::bits`].
    pub fn framing_error(&self) -> bool {
        self.framing
    }

    /// Forgets the character in progress and the clock.
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.prev = 0.0;
        self.agreed = 0;
    }

    /// The clock's edge nearest to time `t`.
    fn edge_near(&self, t: f64) -> f64 {
        self.grid + ((t - self.grid) / self.spb).round() * self.spb
    }

    /// Follows the clock with a crossing at time `c`.
    fn follow(&mut self, c: f64) {
        if c - self.last_crossing > STALE_BITS * self.spb {
            self.agreed = 0;
        }
        self.last_crossing = c;
        let e = self.edge_near(c);
        if self.agreed > 0 && (c - e).abs() <= NEAR * self.spb {
            self.grid = e + GAIN * (c - e);
            self.agreed = (self.agreed + 1).min(MAX_AGREED);
        } else if self.agreed > 1 {
            self.agreed -= 1;
        } else {
            self.grid = c;
            self.agreed = 1;
        }
    }

    /// The time at which the bit after the edge near `t` is read: on the clock once it is locked and not stale.
    fn read_time(&self, t: f64) -> f64 {
        let fresh = self.clock && t - self.last_crossing <= STALE_BITS * self.spb;
        let e = if fresh && self.agreed >= LOCK && (t - self.edge_near(t)).abs() <= NEAR * self.spb
        {
            self.edge_near(t)
        } else {
            t
        };
        e + self.spb / 2.0
    }

    /// Takes the decision of the next sample (positive: mark). Returns a character when its stop bit reads mark; a character
    /// whose start bit reads mark is dropped at once, one whose stop bit reads space when the stop bit is read.
    pub fn push(&mut self, d: f32) -> Option<u8> {
        self.decided = None;
        self.framing = false;
        let raw = d;
        let d = d * self.sense;
        let now = self.t;
        self.t += 1.0;
        let prev = std::mem::replace(&mut self.prev, d);
        // A crossing lies between the last sample and this one.
        let crossing = ((prev > 0.0) != (d > 0.0))
            .then(|| now - 1.0 + f64::from(prev / (prev - d)).clamp(0.0, 1.0));
        if let Some(c) = crossing {
            self.follow(c);
        }
        match self.state {
            State::Idle => {
                if let (Some(c), true) = (crossing, d <= 0.0) {
                    self.state = State::Bit {
                        k: 0,
                        at: self.read_time(c),
                        value: 0,
                    };
                }
                None
            }
            State::Bit { k, at, value } => {
                if now + 0.5 < at {
                    return None;
                }
                let mark = d > 0.0;
                let next = self.read_time(at + self.spb / 2.0);
                if k == 0 {
                    self.read.clear();
                }
                self.read.push((now, mark));
                self.decided = Some(raw > 0.0);
                if k == 0 {
                    self.state = if mark {
                        State::Idle
                    } else {
                        State::Bit {
                            k: 1,
                            at: next,
                            value: 0,
                        }
                    };
                    return None;
                }
                if k <= self.bits {
                    let value = value | (u8::from(mark) << (k - 1));
                    self.state = State::Bit {
                        k: k + 1,
                        at: next,
                        value,
                    };
                    return None;
                }
                self.state = State::Idle;
                self.framing = !mark;
                mark.then_some(value)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The decisions of `chars` at `spb` samples a bit: mark idle, then each character with a start bit, the data bits and
    /// `stops` stop bits.
    fn line(chars: &[u8], bits: u8, spb: f64, stops: usize) -> Vec<f32> {
        let mut levels = vec![1.0f32; 3];
        for &c in chars {
            levels.push(-1.0);
            levels.extend((0..bits).map(|k| if c >> k & 1 == 1 { 1.0 } else { -1.0 }));
            levels.extend(std::iter::repeat_n(1.0, stops));
        }
        levels.extend([1.0; 3]);
        let n = (levels.len() as f64 * spb) as usize;
        (0..n).map(|i| levels[(i as f64 / spb) as usize]).collect()
    }

    #[test]
    fn without_the_clock_a_stop_of_one_and_a_half_bits_is_read() {
        // Five data bits, 1.5 stop bits: each start edge lies half a bit off the clock of the character before.
        let (spb, codes) = (40.0, [23u8, 19, 1, 10, 16, 4, 31, 0, 27]);
        let mut levels: Vec<(f32, f64)> = vec![(1.0, 3.0)];
        for &c in &codes {
            levels.push((-1.0, 1.0));
            levels.extend((0..5).map(|k| (if c >> k & 1 == 1 { 1.0 } else { -1.0 }, 1.0)));
            levels.push((1.0, 1.5));
        }
        levels.push((1.0, 3.0));
        let mut d = Vec::new();
        let mut t = 0.0f64;
        for (v, bits) in levels {
            let end = t + bits * spb;
            while (d.len() as f64) < end.round() {
                d.push(v);
            }
            t = end;
        }
        let mut f = CharFramer::new(spb, 5, false).unwrap().without_clock();
        let got: Vec<u8> = d.iter().filter_map(|&x| f.push(x)).collect();
        assert_eq!(got, codes);
    }

    #[test]
    fn characters_of_7_and_8_bits_in_both_senses() {
        let text = b"$$HORUS,1*AB\n";
        for (bits, stops, spb) in [
            (7u8, 2usize, 96.0),
            (8, 2, 32.0),
            (7, 1, 192.0),
            (8, 1, 13.7),
        ] {
            for inverted in [false, true] {
                let mut f = CharFramer::new(spb, bits, inverted).unwrap();
                let sign = if inverted { -1.0 } else { 1.0 };
                let mut got = Vec::new();
                for &d in &line(text, bits, spb, stops) {
                    if let Some(c) = f.push(d * sign) {
                        got.push(c);
                        // The start bit, the data bits least significant first and the stop bit, one bit apart.
                        let b = f.bits();
                        assert_eq!(b.len(), usize::from(bits) + 2);
                        assert!(!b[0].1 && b[b.len() - 1].1);
                        for (k, w) in b.windows(2).enumerate() {
                            assert!((w[1].0 - w[0].0 - spb).abs() <= 1.0, "bit {k}: {w:?}");
                        }
                        let value = (0..bits)
                            .fold(0u8, |v, k| v | (u8::from(b[1 + usize::from(k)].1) << k));
                        assert_eq!(value, c);
                    }
                }
                assert_eq!(
                    got, text,
                    "{bits} bits, {stops} stop bits, {spb} samples a bit, inverted {inverted}"
                );
            }
        }
        let mut f = CharFramer::new(10.0, 8, false).unwrap();
        // A stop bit of space drops the character.
        let mut d = line(b"A", 8, 10.0, 1);
        for v in &mut d[3 * 10 + 9 * 10..3 * 10 + 10 * 10] {
            *v = -1.0;
        }
        let mut errors = 0;
        assert!(d
            .iter()
            .filter_map(|&x| {
                let c = f.push(x);
                errors += usize::from(f.framing_error());
                c
            })
            .next()
            .is_none());
        assert_eq!(errors, 1);
        assert!(
            CharFramer::new(1.5, 8, false).is_err() && CharFramer::new(10.0, 9, false).is_err()
        );
    }
}
