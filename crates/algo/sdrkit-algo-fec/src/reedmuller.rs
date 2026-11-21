//! The second-order Reed-Muller code RM(2,5): 16 information bits, 32 code bits, minimum distance 8, with a
//! maximum-likelihood soft decoder (one fast Hadamard transform for each of the 1024 cosets of the first-order code).
//!
//! The generator is the one of ETSI EN 302 769 clause 7.2.2: row 0 all ones; rows 1 to 5 the coordinate vectors
//! `v_m(j)` = bit `5 - m` of the code bit index `j`; rows 6 to 15 the products `v_a v_b` for (a, b) = (1,2) (1,3) (1,4) (1,5)
//! (2,3) (2,4) (2,5) (3,4) (3,5) (4,5). Message bit `b_0` is the most significant bit of a `u16`. Code bit `j` is bit `31 - j`
//! of a `u32`, so code bit 0 is the most significant.

use crate::error::Result;
use crate::small::fht_decode;

/// The pairs (a, b) of the product rows 6 to 15.
const PAIRS: [(u32, u32); 10] = [
    (1, 2),
    (1, 3),
    (1, 4),
    (1, 5),
    (2, 3),
    (2, 4),
    (2, 5),
    (3, 4),
    (3, 5),
    (4, 5),
];

/// Bit `j` (code bit order, bit 31 - j of the word) of the coordinate vector `v_m`.
const fn coordinate(m: u32, j: u32) -> u32 {
    (j >> (5 - m)) & 1
}

/// The generator row of message bit `r` (0 to 15) as a code word.
const fn row(r: usize) -> u32 {
    let mut w = 0u32;
    let mut j = 0u32;
    while j < 32 {
        let bit = if r == 0 {
            1
        } else if r <= 5 {
            coordinate(r as u32, j)
        } else {
            let (a, b) = PAIRS[r - 6];
            coordinate(a, j) & coordinate(b, j)
        };
        w |= bit << (31 - j);
        j += 1;
    }
    w
}

/// The 16 generator rows.
const ROWS: [u32; 16] = {
    let mut t = [0u32; 16];
    let mut r = 0;
    while r < 16 {
        t[r] = row(r);
        r += 1;
    }
    t
};

/// The code word of `msg` (`b_0` the most significant bit): `lambda = b G` over GF(2).
pub fn rm25_encode(msg: u16) -> u32 {
    let mut w = 0u32;
    for (r, g) in ROWS.iter().enumerate() {
        if (msg >> (15 - r)) & 1 != 0 {
            w ^= g;
        }
    }
    w
}

/// The maximum-likelihood soft decoder of RM(2,5). It holds the sign pattern of each coset of the first-order code and
/// allocates nothing after [`Rm25Decoder::new`].
#[derive(Clone, Debug)]
pub struct Rm25Decoder {
    /// For each value of the ten product bits (`b_6` the most significant), the coset word in code bit order (bit `j`
    /// of the value is code bit `j`).
    cosets: Vec<u32>,
}

impl Default for Rm25Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Rm25Decoder {
    /// The decoder.
    pub fn new() -> Self {
        let cosets = (0..1024u32)
            .map(|c| {
                let mut w = 0u32;
                for (i, g) in ROWS[6..].iter().enumerate() {
                    if (c >> (9 - i)) & 1 != 0 {
                        w ^= g;
                    }
                }
                // Code bit j at bit j, for indexing the soft values.
                w.reverse_bits()
            })
            .collect();
        Rm25Decoder { cosets }
    }

    /// Decodes 32 LLRs (`soft[j]` for code bit `j`, positive means 0). Returns the message and its correlation with the
    /// received values, `sum_j soft[j] (1 - 2 lambda_j)` (larger is more certain). Errors when a value is not finite.
    pub fn decode(&self, soft: &[f32; 32]) -> Result<(u16, f32)> {
        crate::llr::check_finite(soft)?;
        let (mut best, mut best_v) = (0u16, f32::NEG_INFINITY);
        let mut buf = [0f32; 32];
        for (c, &mask) in self.cosets.iter().enumerate() {
            for (j, (o, &s)) in buf.iter_mut().zip(soft).enumerate() {
                *o = if (mask >> j) & 1 != 0 { -s } else { s };
            }
            let (first, v) = fht_decode(&mut buf)?;
            if v > best_v {
                best_v = v;
                // fht_decode: low 5 bits a with bit (5 - m) = b_m, bit 5 = b_0.
                let a = first & 31;
                let top = (first >> 5) & 1;
                best = ((top << 15) | (a << 10) | c as u32) as u16;
            }
        }
        Ok((best, best_v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_distance() {
        assert_eq!(rm25_encode(0), 0);
        assert_eq!(rm25_encode(0x8000), u32::MAX);
        // v_1 is 0 for j < 16, v_5 alternates.
        assert_eq!(ROWS[1], 0x0000_FFFF);
        assert_eq!(ROWS[5], 0x5555_5555);
        let min = (1..=u16::MAX)
            .map(|m| rm25_encode(m).count_ones())
            .min()
            .unwrap();
        assert_eq!(min, 8);
    }

    #[test]
    fn soft_decoding_corrects_three_errors() {
        let d = Rm25Decoder::new();
        let mut s = 0x1234_5678u32;
        for _ in 0..200 {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            let msg = s as u16;
            let cw = rm25_encode(msg);
            let flips = [(s >> 16) % 32, (s >> 21) % 32, (s >> 26) % 32];
            let mut soft = [0f32; 32];
            for (j, v) in soft.iter_mut().enumerate() {
                let mut bit = (cw >> (31 - j)) & 1;
                if flips.contains(&(j as u32)) {
                    bit ^= 1;
                }
                *v = if bit == 0 { 1.0 } else { -1.0 };
            }
            assert_eq!(d.decode(&soft).unwrap().0, msg);
        }
    }
}
