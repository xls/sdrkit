//! MPT 1327 codewords: 48 information bits, 15 check bits and a parity bit, on the shared code of `sdrkit-algo-fec`. A codeword is
//! held in a `u64` with bit 1 (sent first) as the most significant bit.
//!
//! Standard: MPT 1327 clause 3.2.3.

use sdrkit_algo_fec::small::SmallCode;
use std::sync::OnceLock;

/// The control channel sync word.
pub const SYNC: u16 = 0xC4D7;
/// The traffic channel sync word (not used by the receiver).
pub const SYNT: u16 = 0x3B28;

/// The MPT 1327 code word code: the shared (63,48) cyclic code with its last check bit inverted and an even parity bit
/// (MPT 1327 clause 3.2.3), decoded by its syndrome table.
pub fn code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(SmallCode::mpt1327_64_48)
}

/// The codeword of 48 information bits (the low 48 bits of `info`).
pub fn encode(info: u64) -> u64 {
    code().encode((info & ((1u64 << 48) - 1)) as u128) as u64
}

/// The 48 information bits of a codeword.
pub fn info(cw: u64) -> u64 {
    cw >> 16
}

/// The check and parity bits of a codeword (its last 16 bits).
pub fn tail(cw: u64) -> u16 {
    cw as u16
}

/// Whether the check bits and the parity are right.
pub fn valid(cw: u64) -> bool {
    code().syndrome(cw as u128) == 0
}

/// How a received word checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// Right as received.
    Good,
    /// One wrong bit, repaired.
    Repaired,
    /// Two or more wrong bits (detected, never turned into another codeword).
    Failed,
}

/// Checks a word and repairs a single wrong bit by the syndrome table. A word with two or more wrong bits fails: the code's
/// distance is 6, and MPT 1327 clause 3.2.3 corrects one error only.
pub fn check_and_repair(cw: u64) -> (u64, Check) {
    match code().decode(cw as u128) {
        Some((w, 0)) => (w as u64, Check::Good),
        Some((w, 1)) => (w as u64, Check::Repaired),
        _ => (cw, Check::Failed),
    }
}

/// Bits `a..=b` (1-based, bit 1 first sent) of a codeword, most significant first.
pub fn field(cw: u64, a: u32, b: u32) -> u32 {
    let n = b - a + 1;
    ((cw >> (64 - b)) & ((1u64 << n) - 1)) as u32
}

/// Puts `value` into bits `a..=b` of 48 information bits (held as the top 48 bits of a codeword, i.e. `info << 16`).
pub fn put(info: &mut u64, a: u32, b: u32, value: u32) {
    let n = b - a + 1;
    let mask = ((1u64 << n) - 1) << (48 - b);
    *info = (*info & !mask) | (((value as u64) << (48 - b)) & mask);
}

/// The control channel system codeword of system `sys` whose check and parity bits equal `sync` (normally [`SYNC`]): bit 1 = 0,
/// SYS in bits 2 to 16, the CCS field (bits 17 to 32) solved so that the last 16 bits equal `sync`, the preamble 1010… in bits 33 to
/// 48. The check bits are affine in the information bits, so CCS is the solution of a 16 × 16 system over GF(2).
pub fn ccsc(sys: u32, sync: u16) -> Option<u64> {
    let mut base = 0u64;
    put(&mut base, 2, 16, sys & 0x7FFF);
    put(&mut base, 33, 48, 0xAAAA);
    let f = |ccs: u32| {
        let mut i = base;
        put(&mut i, 17, 32, ccs);
        tail(encode(i))
    };
    let b = f(0);
    // Columns: the effect of each CCS bit.
    let cols: Vec<u16> = (0..16).map(|k| f(1 << k) ^ b).collect();
    let target = sync ^ b;
    // Solve sum x_k cols[k] = target by Gaussian elimination over GF(2): rows are the 16 output bits.
    let mut rows: Vec<(u16, u8)> = (0..16)
        .map(|r| {
            let mut m = 0u16;
            for (k, c) in cols.iter().enumerate() {
                if (c >> r) & 1 != 0 {
                    m |= 1 << k;
                }
            }
            (m, ((target >> r) & 1) as u8)
        })
        .collect();
    let mut x = 0u32;
    let mut pivots = Vec::new();
    let mut row = 0;
    for col in 0..16 {
        let Some(p) = (row..16).find(|&r| (rows[r].0 >> col) & 1 != 0) else {
            continue;
        };
        rows.swap(row, p);
        for r in 0..16 {
            if r != row && (rows[r].0 >> col) & 1 != 0 {
                rows[r].0 ^= rows[row].0;
                rows[r].1 ^= rows[row].1;
            }
        }
        pivots.push((row, col));
        row += 1;
    }
    if rows[row..].iter().any(|r| r.1 != 0) {
        return None;
    }
    for (r, c) in pivots {
        if rows[r].1 != 0 {
            x |= 1 << c;
        }
    }
    let mut i = base;
    put(&mut i, 17, 32, x);
    let cw = encode(i);
    (tail(cw) == sync).then_some(cw)
}

/// Whether a word is a control channel system codeword (valid, bit 1 = 0, last 16 bits = SYNC).
pub fn is_ccsc(cw: u64) -> bool {
    valid(cw) && cw >> 63 == 0 && tail(cw) == SYNC
}

/// The system identity of a CCSC (bits 2 to 16).
pub fn ccsc_system(cw: u64) -> u32 {
    field(cw, 2, 16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn round_trip_and_detection() {
        let mut r = Rng::new(3);
        for _ in 0..200 {
            let i = r.next_u64() & ((1 << 48) - 1);
            let cw = encode(i);
            assert!(valid(cw));
            assert_eq!(info(cw), i);
            for a in 0..64 {
                assert!(!valid(cw ^ (1 << a)));
                for b in a + 1..64 {
                    assert!(!valid(cw ^ (1 << a) ^ (1 << b)));
                }
            }
        }
    }

    #[test]
    fn single_errors_repaired_doubles_detected() {
        let mut r = Rng::new(9);
        for _ in 0..400 {
            let cw = encode(r.next_u64());
            let a = r.below(64);
            assert_eq!(check_and_repair(cw ^ (1 << a)), (cw, Check::Repaired));
            let mut b = r.below(64);
            while b == a {
                b = r.below(64);
            }
            assert_eq!(check_and_repair(cw ^ (1 << a) ^ (1 << b)).1, Check::Failed);
        }
    }

    #[test]
    fn ccsc_for_several_systems() {
        for sys in [0u32, 1, 100, 12345, 20001, 32767] {
            let cw = ccsc(sys, SYNC).expect("solvable");
            assert!(is_ccsc(cw));
            assert_eq!(ccsc_system(cw), sys);
            assert_eq!(tail(cw), 0xC4D7);
            assert_eq!(field(cw, 33, 48), 0xAAAA);
        }
        assert_eq!((SYNC, SYNT), (0xC4D7, 0x3B28));
        assert!(ccsc(20001, 0x35A9).is_some());
    }
}
