//! The CCSDS pseudo-randomisers.
//!
//! - **TM** (CCSDS 131.0-B-5 clause 10): `h(x) = x^8 + x^7 + x^5 + x^3 + 1`, all ones at the first bit after each attached sync
//!   marker, period 255 bits. The sequence is `sdrkit_util_bits::seq::CCSDS`. This module applies it to bytes, hard bits and LLRs from
//!   any bit offset, so a receiver that removes it before the frame is cut into bytes (or before soft decoding) uses the same
//!   table.
//! - **TC** (CCSDS 231.0-B-4 clause 6.3): `h(x) = x^8 + x^6 + x^4 + x^3 + x^2 + x + 1`, all ones at the start of each CLTU's
//!   data, applied before the BCH code.
//!
//! The polynomials are written as the CCSDS standards write them, which is the reciprocal of the delay form `sdrkit-util-bits` uses. The TM register is `Lfsr::fibonacci(0x12B, 0xFF)`, the TC register
//! `Lfsr::fibonacci(0x1F5, 0xFF)`, both with the oldest stage as output.
//!
//! ```
//! use sdrkit_proto_ccsds::randomiser;
//! let mut data = [0u8; 4];
//! randomiser::tm_bytes(&mut data);
//! assert_eq!(data, [0xFF, 0x48, 0x0E, 0xC0]);
//! ```

use sdrkit_util_bits::lfsr::{Lfsr, Period};
use sdrkit_util_bits::{seq, BitOrder};

/// The TM sequence as 255 bytes: FF 48 0E C0 9A 0D 70 BC ... (CCSDS 131.0-B-5 clause 10.4).
pub fn tm_table() -> &'static [u8; 255] {
    seq::CCSDS.bytes()
}

/// XORs the TM sequence onto `data`, the frame bytes after the marker, starting at the sequence's beginning.
pub fn tm_bytes(data: &mut [u8]) {
    seq::CCSDS.apply(data, 0);
}

/// XORs the TM sequence onto `data`, which starts `offset` bytes after the marker.
pub fn tm_bytes_at(data: &mut [u8], offset: usize) {
    seq::CCSDS.apply(data, offset % 255);
}

/// Bit `i` (0 is the first bit after the marker) of the TM sequence.
#[inline]
pub fn tm_bit(i: usize) -> u8 {
    let i = i % (255 * 8);
    (tm_table()[i / 8] >> (7 - i % 8)) & 1
}

/// XORs the TM sequence onto hard bits (0 or 1), the first of which is bit `offset` after the marker.
pub fn tm_hard_bits(bits: &mut [u8], offset: usize) {
    let mut i = offset % (255 * 8);
    let t = tm_table();
    for b in bits {
        *b ^= (t[i / 8] >> (7 - i % 8)) & 1;
        i += 1;
        if i == 255 * 8 {
            i = 0;
        }
    }
}

/// Removes the TM sequence from LLRs (positive means 0), the first of which is bit `offset` after the marker: where the sequence
/// has a 1 the LLR changes sign (saturating, so -128 never appears).
pub fn tm_llrs(llrs: &mut [i8], offset: usize) {
    let mut i = offset % (255 * 8);
    let t = tm_table();
    for l in llrs {
        if (t[i / 8] >> (7 - i % 8)) & 1 == 1 {
            *l = l.saturating_neg().max(-127);
        }
        i += 1;
        if i == 255 * 8 {
            i = 0;
        }
    }
}

/// The TC randomiser register: `h(x) = x^8 + x^6 + x^4 + x^3 + x^2 + x + 1` (CCSDS 231.0-B-4 clause 6.3), all ones, oldest
/// stage out.
pub const TC_RANDOMISER: Lfsr = Lfsr::fibonacci(0x1F5, 0xFF).with_output(0x80);

/// The TC sequence as 255 bytes (its period is 255 bits, so the table repeats every 255 bytes): FF 39 9E 5A 68 ...
pub static TC: Period<255> = Period::of(TC_RANDOMISER, BitOrder::Msb);

/// XORs the TC sequence onto `data` (a CLTU's data before BCH coding, or after decoding), from the sequence's beginning.
pub fn tc_bytes(data: &mut [u8]) {
    TC.apply(data, 0);
}
