//! Four-phase differential coding of uncoded QPSK: two data bits a symbol, carried by the change between the hard bits of
//! consecutive symbols, so that a quarter turn of the carrier does not change the data.
//!
//! A symbol gives the hard bits `X` (I positive) and `Y` (Q positive); `X1`, `Y1` are those of the symbol before. The two data
//! bits are, in this order:
//!
//! | `X XOR Y` | First bit | Second bit |
//! |---|---|---|
//! | 1 | `X1 XOR X` | `Y1 XOR Y` |
//! | 0 | `Y1 XOR Y` | `X1 XOR X` |
//!
//! [`QuadDiffEncoder`] is the exact inverse. A mirrored spectrum (I and Q swapped) swaps the two bits of every symbol.
//!
//! Some links invert the second data bit of every symbol: [`QuadDiffDecoder::second_inverted`] and
//! [`QuadDiffEncoder::second_inverted`] make the pair for them.
//!
//! ```
//! use sdrkit_proto_ccsds::quaddiff::{QuadDiffDecoder, QuadDiffEncoder};
//! let data = [1u8, 0, 0, 1, 1, 1, 0, 0];
//! let mut e = QuadDiffEncoder::default();
//! let mut d = QuadDiffDecoder::default();
//! let mut out = Vec::new();
//! for p in data.chunks(2) {
//!     let (x, y) = e.encode(p[0], p[1]);
//!     // A quarter turn: (I, Q) becomes (-Q, I).
//!     let (a, b) = d.decode(y ^ 1, x);
//!     out.extend([a, b]);
//! }
//! assert_eq!(out[2..], data[2..]);
//! ```

/// The decoder: hard bits of each symbol in, two data bits out. It starts from the symbol (0, 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuadDiffDecoder {
    x1: u8,
    y1: u8,
    invert_second: u8,
}

impl QuadDiffDecoder {
    /// A decoder that inverts the second data bit of every symbol.
    pub fn second_inverted() -> Self {
        QuadDiffDecoder {
            invert_second: 1,
            ..Self::default()
        }
    }

    /// The two data bits of the symbol with hard bits `x`, `y` (0 or 1).
    #[inline]
    pub fn decode(&mut self, x: u8, y: u8) -> (u8, u8) {
        let (x, y) = (x & 1, y & 1);
        let (dx, dy) = (self.x1 ^ x, self.y1 ^ y);
        self.x1 = x;
        self.y1 = y;
        if x ^ y == 1 {
            (dx, dy ^ self.invert_second)
        } else {
            (dy, dx ^ self.invert_second)
        }
    }

    /// Decodes hard bits in place: each pair (X then Y) becomes its two data bits. A last odd bit is left as it is.
    pub fn decode_pairs(&mut self, bits: &mut [u8]) {
        for [x, y] in bits.as_chunks_mut::<2>().0 {
            (*x, *y) = self.decode(*x, *y);
        }
    }
}

/// The encoder: two data bits in, the hard bits of the next symbol out. It starts from the symbol (0, 0).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuadDiffEncoder {
    x1: u8,
    y1: u8,
    invert_second: u8,
}

impl QuadDiffEncoder {
    /// The inverse of [`QuadDiffDecoder::second_inverted`].
    pub fn second_inverted() -> Self {
        QuadDiffEncoder {
            invert_second: 1,
            ..Self::default()
        }
    }

    /// The hard bits (X, Y) of the symbol that carries the data bits `a` then `b`.
    #[inline]
    pub fn encode(&mut self, a: u8, b: u8) -> (u8, u8) {
        let (a, b) = (a & 1, (b ^ self.invert_second) & 1);
        let (x, y) = if self.x1 ^ self.y1 ^ a ^ b == 1 {
            (self.x1 ^ a, self.y1 ^ b)
        } else {
            (self.x1 ^ b, self.y1 ^ a)
        };
        self.x1 = x;
        self.y1 = y;
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pair_round_trips_from_every_state() {
        for s in 0..4u8 {
            for d in 0..4u8 {
                let mut e = QuadDiffEncoder {
                    x1: s >> 1,
                    y1: s & 1,
                    invert_second: 0,
                };
                let mut q = QuadDiffDecoder {
                    x1: s >> 1,
                    y1: s & 1,
                    invert_second: 0,
                };
                let (x, y) = e.encode(d >> 1, d & 1);
                assert_eq!(q.decode(x, y), (d >> 1, d & 1));
            }
        }
    }

    #[test]
    fn a_quarter_turn_keeps_the_data() {
        let mut rng = 0x1234_5678u32;
        let mut e = QuadDiffEncoder::default();
        let (mut d0, mut d1) = (QuadDiffDecoder::default(), QuadDiffDecoder::default());
        for k in 0..200 {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let (a, b) = ((rng & 1) as u8, ((rng >> 1) & 1) as u8);
            let (x, y) = e.encode(a, b);
            assert_eq!(d0.decode(x, y), (a, b));
            let got = d1.decode(y ^ 1, x);
            if k > 0 {
                assert_eq!(got, (a, b));
            }
        }
    }

    /// The written-out rule of the links that swap I and Q and invert the second bit: with X, Y the hard bits of a symbol
    /// and X1, Y1 those of the symbol before, X XOR Y = 1 gives (Y1 XOR Y, NOT (X1 XOR X)), else (X1 XOR X, NOT (Y1 XOR Y)).
    #[test]
    fn swapped_rails_with_the_second_bit_inverted() {
        for s in 0..4u8 {
            for v in 0..4u8 {
                let (x1, y1, x, y) = (s >> 1, s & 1, v >> 1, v & 1);
                let mut d = QuadDiffDecoder {
                    x1: y1,
                    y1: x1,
                    invert_second: 1,
                };
                let want = if x ^ y == 1 {
                    (y1 ^ y, 1 ^ x1 ^ x)
                } else {
                    (x1 ^ x, 1 ^ y1 ^ y)
                };
                assert_eq!(d.decode(y, x), want);
            }
        }
        let mut e = QuadDiffEncoder::second_inverted();
        let mut d = QuadDiffDecoder::second_inverted();
        for k in 0..64u8 {
            let (a, b) = (k & 1, (k >> 2) & 1);
            let (x, y) = e.encode(a, b);
            assert_eq!(d.decode(x, y), (a, b));
        }
    }
}
