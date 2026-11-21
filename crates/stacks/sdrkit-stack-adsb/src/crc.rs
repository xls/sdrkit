//! The Mode S parity on the shared CRC-24, the syndromes of single-bit errors for error correction, and the uplink address/parity
//! overlay.
//!
//! Standard: ICAO Annex 10 Vol. IV 3.1.2.3.3

use sdrkit_util_bits::crc::catalog::CRC24_MODE_S;
use std::sync::OnceLock;

/// The generator polynomial without its x^24 term.
pub const POLY: u32 = 0xFF_F409;

/// The parity of `data`: the remainder of D(x)·x^24 divided by G(x) (`CRC-24/MODE-S` of `sdrkit-util-bits`).
pub fn parity(data: &[u8]) -> u32 {
    CRC24_MODE_S.checksum(data)
}

/// The remainder of a whole message (data then 24 parity bits): the parity of the data XOR the parity field. 0 for a clean PI
/// message; the address for an address/parity message.
pub fn remainder(msg: &[u8]) -> u32 {
    if msg.len() < 4 {
        return 0;
    }
    let n = msg.len() - 3;
    let p = ((msg[n] as u32) << 16) | ((msg[n + 1] as u32) << 8) | msg[n + 2] as u32;
    parity(&msg[..n]) ^ p
}

/// The syndromes of every single-bit error, for 56-bit and 112-bit messages (index = bit number - 1).
pub struct Syndromes {
    /// 56-bit messages.
    pub short: Vec<u32>,
    /// 112-bit messages.
    pub long: Vec<u32>,
}

impl Syndromes {
    /// The syndromes of a message of `bits` bits.
    pub fn for_len(&self, bits: usize) -> &[u32] {
        if bits == 56 {
            &self.short
        } else {
            &self.long
        }
    }
    /// The bit (0-based) whose syndrome is `s` in a message of `bits` bits.
    pub fn locate(&self, bits: usize, s: u32) -> Option<usize> {
        let idx = if bits == 56 {
            &short_index().0
        } else {
            &short_index().1
        };
        idx.binary_search_by_key(&s, |e| e.0).ok().map(|i| idx[i].1)
    }
}

fn build(bits: usize) -> Vec<u32> {
    let mut msg = vec![0u8; bits / 8];
    (0..bits)
        .map(|i| {
            msg.iter_mut().for_each(|b| *b = 0);
            msg[i / 8] = 0x80 >> (i % 8);
            remainder(&msg)
        })
        .collect()
}

type Index = (Vec<(u32, usize)>, Vec<(u32, usize)>);

fn short_index() -> &'static Index {
    static I: OnceLock<Index> = OnceLock::new();
    I.get_or_init(|| {
        let s = syndromes();
        let mk = |v: &[u32]| {
            let mut x: Vec<(u32, usize)> = v.iter().enumerate().map(|(i, &s)| (s, i)).collect();
            x.sort_unstable();
            x
        };
        (mk(&s.short), mk(&s.long))
    })
}

/// The single-bit syndromes (computed once).
pub fn syndromes() -> &'static Syndromes {
    static S: OnceLock<Syndromes> = OnceLock::new();
    S.get_or_init(|| Syndromes {
        short: build(56),
        long: build(112),
    })
}

/// The uplink address/parity overlay (Annex 10 Vol. IV, 3.1.2.3.3.2): the sequence XORed into the parity of an interrogation is the
/// remainder of A(x)·x^24 divided by G(x), A being the 24-bit address of the interrogated transponder.
pub fn uplink_overlay(address: u32) -> u32 {
    parity(&[(address >> 16) as u8, (address >> 8) as u8, address as u8])
}

/// The address an interrogation is addressed to, from the remainder of the received message (the inverse of [`uplink_overlay`]).
pub fn uplink_address(remainder: u32) -> u32 {
    static INV: OnceLock<[u32; 24]> = OnceLock::new();
    let inv = INV.get_or_init(|| {
        // Columns of the linear map address -> overlay, inverted by Gauss-Jordan elimination over GF(2).
        let mut rows = [0u64; 24]; // row r: 24 bits of the map's row, then 24 bits of the identity
        let cols: Vec<u32> = (0..24).map(|j| uplink_overlay(1 << j)).collect();
        for (r, row) in rows.iter_mut().enumerate() {
            let mut v = 0u64;
            for (j, c) in cols.iter().enumerate() {
                if c >> r & 1 == 1 {
                    v |= 1 << j;
                }
            }
            *row = v | (1u64 << (24 + r));
        }
        for c in 0..24 {
            let p = (c..24)
                .find(|&r| rows[r] >> c & 1 == 1)
                .expect("the overlay is invertible");
            rows.swap(c, p);
            for r in 0..24 {
                if r != c && rows[r] >> c & 1 == 1 {
                    rows[r] ^= rows[c];
                }
            }
        }
        // rows[i] now has unit i in its left half; its right half is row i of the inverse.
        let mut out = [0u32; 24];
        for (i, o) in out.iter_mut().enumerate() {
            *o = (rows[i] >> 24) as u32;
        }
        out
    });
    let mut a = 0u32;
    for (i, row) in inv.iter().enumerate() {
        if (row & remainder).count_ones() & 1 == 1 {
            a |= 1 << i;
        }
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn published_remainders() {
        assert_eq!(remainder(&hex("8D406B902015A678D4D220AA4BDA")), 0);
        assert_eq!(parity(&hex("8D406B902015A678D4D220")), 0xAA4BDA);
        assert_eq!(remainder(&hex("8D4CA251204994B1C36E60A5343D")), 16);
        assert_eq!(remainder(&hex("5D484FDEA248F5")), 22);
        assert_eq!(remainder(&hex("2000171806A983")), 0x4CA7E8);
    }

    #[test]
    fn syndromes_are_unique_and_non_zero() {
        let s = syndromes();
        for v in [&s.short, &s.long] {
            let mut x = v.clone();
            x.sort_unstable();
            x.dedup();
            assert_eq!(x.len(), v.len());
            assert!(!x.contains(&0));
        }
        assert_eq!(s.locate(112, 16), Some(107));
    }

    #[test]
    fn the_uplink_overlay_inverts() {
        for a in [0u32, 1, 0x4840D6, 0xFFFFFF, 0xABCDEF, 0x800000] {
            assert_eq!(uplink_address(uplink_overlay(a)), a);
        }
    }
}
