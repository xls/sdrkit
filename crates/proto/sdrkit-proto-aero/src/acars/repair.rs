//! The BCS test and the repair of bit errors (see [`check_and_repair`]).

use sdrkit_util_bits::crc::catalog::CRC16_KERMIT;
use std::sync::OnceLock;

use super::parity_ok;

/// The longest body (mode to ETX or ETB) that the receiver accepts. A full block has 12 header characters, STX, 220 text
/// characters and the end character. The limit is a little higher for links that send more.
pub const MAX_BODY: usize = 250;

/// The syndrome count: one per bit of the longest body and its two BCS bytes.
const SYNDROMES: usize = (MAX_BODY + 2) * 8;

/// The result of [`check_and_repair`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    /// The block is good after the repair changed `bits_fixed` bits (0 when it was good as received). A bit fixed in the BCS
    /// counts too.
    Good {
        /// Bits changed.
        bits_fixed: u32,
    },
    /// The BCS is bad and no single repair fits.
    Bad,
}

/// The syndrome of bit `k`, counted from the end of the frame (bit 0 of the last BCS byte is 0). It is the BCS register that
/// one wrong bit there leaves. `None` past the longest frame. Bits 0 to 7 are the CRC table entries `T[1 << k]` of the reflected
/// CCITT polynomial that RFC 1662 Appendix C prints.
pub fn syndrome(k: usize) -> Option<u16> {
    syndromes().get(k).copied()
}

/// The syndrome table. Index `k` is the bit at distance `k` from the end of the frame. Bit 0 of the last BCS byte is index 0.
pub(crate) fn syndromes() -> &'static [u16; SYNDROMES] {
    static T: OnceLock<[u16; SYNDROMES]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u16; SYNDROMES];
        for bit in 0..8 {
            // The CRC of the byte with this bit set, then of the same byte followed by zero bytes, one more each step.
            let mut d = CRC16_KERMIT.digest();
            d.update(&[1u8 << bit]);
            let mut k = bit;
            while k < SYNDROMES {
                t[k] = d.finalize();
                d.update(&[0]);
                k += 8;
            }
        }
        t
    })
}

/// Tests `body` (mode to ETX or ETB, parity bits on) against its BCS `bcs` (low byte first). If the errors fit one of the
/// patterns below, it repairs `body` in place.
///
/// The repair accepts these error patterns:
///
/// - one wrong bit in each of up to three bytes with bad parity, plus at most one wrong bit in the BCS itself,
/// - two wrong bits in one byte (the parity stays good),
/// - one wrong bit in the BCS.
///
/// A pattern is accepted only when exactly one pattern of its class matches. An exact match is tried before a match that also
/// needs one bit in the BCS. If two patterns of one class match, the block stays bad. [`Check::Good`] reports the number of
/// bits fixed.
pub fn check_and_repair(body: &mut [u8], bcs: [u8; 2]) -> Check {
    let n = body.len();
    if n == 0 || n > MAX_BODY {
        return Check::Bad;
    }
    let mut d = CRC16_KERMIT.digest();
    d.update(body);
    d.update(&bcs);
    let r = d.finalize();
    let mut wrong = [0usize; 3];
    let mut k = 0;
    for (i, &b) in body.iter().enumerate() {
        if !parity_ok(b) {
            if k == 3 {
                return Check::Bad;
            }
            wrong[k] = i;
            k += 1;
        }
    }
    if r == 0 && k == 0 {
        return Check::Good { bits_fixed: 0 };
    }
    let syn = syndromes();
    // The index of bit `b` of body byte `i`, counted from the end (the BCS holds the last 16 bits).
    let pos = |i: usize, b: usize| (n - 1 - i) * 8 + 16 + b;
    let in_bcs = |s: u16| syn[..16].contains(&s);
    if k == 0 {
        if in_bcs(r) {
            return Check::Good { bits_fixed: 1 };
        }
        let mut found = None;
        for i in 0..n {
            for a in 0..8 {
                for b in a + 1..8 {
                    if syn[pos(i, a)] ^ syn[pos(i, b)] == r {
                        if found.is_some() {
                            return Check::Bad;
                        }
                        found = Some((i, (1u8 << a) | (1 << b)));
                    }
                }
            }
        }
        return match found {
            Some((i, mask)) => {
                body[i] ^= mask;
                Check::Good { bits_fixed: 2 }
            }
            None => Check::Bad,
        };
    }
    let wrong = &wrong[..k];
    // One bit in each bad byte. An exact match wins. Without one, a match that also needs one bit in the BCS is taken.
    let mut exact: Option<[u8; 3]> = None;
    let mut with_bcs: Option<[u8; 3]> = None;
    let mut exact_twice = false;
    let mut with_bcs_twice = false;
    for combo in 0..8usize.pow(k as u32) {
        let mut s = 0u16;
        let mut c = combo;
        let mut bits = [0u8; 3];
        for (j, &i) in wrong.iter().enumerate() {
            bits[j] = (c % 8) as u8;
            c /= 8;
            s ^= syn[pos(i, bits[j] as usize)];
        }
        if s == r {
            exact_twice |= exact.is_some();
            exact = Some(bits);
        } else if in_bcs(s ^ r) {
            with_bcs_twice |= with_bcs.is_some();
            with_bcs = Some(bits);
        }
    }
    let (bits, extra) = match (exact, with_bcs) {
        (Some(b), _) if !exact_twice => (b, false),
        (None, Some(b)) if !with_bcs_twice => (b, true),
        _ => return Check::Bad,
    };
    for (j, &i) in wrong.iter().enumerate() {
        body[i] ^= 1 << bits[j];
    }
    Check::Good {
        bits_fixed: k as u32 + u32::from(extra),
    }
}
