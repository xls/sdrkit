//! FM0 (bi-phase space) from the times between level changes: the level changes at every bit boundary and once more in the
//! middle of a 0 bit, so a full bit between two changes is a 1 and two half bits are a 0. The bits are those of
//! `sdrkit_util_bits::line::Biphase` with `BiphaseKind::Space`, read from edge timing instead of from chips.
//!
//! ```
//! use sdrkit_algo_modem::fm0::{Fm0Edges, Fm0Out};
//! let mut d = Fm0Edges::new(8.0);
//! // A 1 (8 samples), then a 0 (4 + 4 samples).
//! assert_eq!(d.edge(8.0), Fm0Out::Bit(1));
//! assert_eq!(d.edge(4.0), Fm0Out::Nothing);
//! assert_eq!(d.edge(4.0), Fm0Out::Bit(0));
//! assert_eq!(d.edge(20.0), Fm0Out::Lost);
//! ```

/// What one level change gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fm0Out {
    /// A bit.
    Bit(u8),
    /// The first half of a 0.
    Nothing,
    /// The interval fits no bit: the bit stream starts again.
    Lost,
}

/// The FM0 reader of edge intervals.
#[derive(Clone, Debug)]
pub struct Fm0Edges {
    bit: f64,
    half: bool,
}

impl Fm0Edges {
    /// A reader for bits of `samples_per_bit` samples (at least 2).
    pub fn new(samples_per_bit: f64) -> Fm0Edges {
        Fm0Edges {
            bit: samples_per_bit.max(2.0),
            half: false,
        }
    }

    /// Forgets a pending half bit.
    pub fn reset(&mut self) {
        self.half = false;
    }

    /// The level changed `samples` after the previous change. More than 1.25 or less than 0.3 bits is lost; more than 0.75
    /// bits is a 1; else it is a half bit, and two halves are a 0.
    pub fn edge(&mut self, samples: f64) -> Fm0Out {
        let b = samples / self.bit;
        if !(0.3..=1.25).contains(&b) {
            self.half = false;
            return Fm0Out::Lost;
        }
        if b > 0.75 {
            if self.half {
                // A half bit then a full bit: the boundary was missed.
                self.half = false;
                return Fm0Out::Lost;
            }
            return Fm0Out::Bit(1);
        }
        if self.half {
            self.half = false;
            Fm0Out::Bit(0)
        } else {
            self.half = true;
            Fm0Out::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_come_back_from_their_edges() {
        let bits = [1u8, 0, 0, 1, 1, 0, 1, 0, 0, 0, 1];
        // FM0 edges: a change at each boundary, and one in the middle of each 0.
        let mut intervals = Vec::new();
        for &b in &bits {
            if b == 1 {
                intervals.push(10.0);
            } else {
                intervals.extend([5.0, 5.0]);
            }
        }
        let mut d = Fm0Edges::new(10.0);
        let got: Vec<u8> = intervals
            .iter()
            .filter_map(|&i| match d.edge(i * 1.04) {
                Fm0Out::Bit(b) => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(got, bits);
    }
}
