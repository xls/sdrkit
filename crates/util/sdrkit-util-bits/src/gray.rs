//! Gray code: neighbouring values differ in one bit.
//!
//! ```
//! use sdrkit_util_bits::gray::{gray, ungray};
//! assert_eq!((0..8).map(gray).collect::<Vec<_>>(), [0, 1, 3, 2, 6, 7, 5, 4]);
//! assert_eq!(ungray(gray(1000)), 1000);
//! ```

/// The Gray code of `x` (binary-reflected): `x XOR (x >> 1)`. Works at any width.
#[inline]
pub const fn gray(x: u32) -> u32 {
    x ^ (x >> 1)
}

/// The value whose Gray code is `g`: a prefix XOR in five shifts, no loop over bits.
#[inline]
pub const fn ungray(g: u32) -> u32 {
    let mut x = g;
    x ^= x >> 16;
    x ^= x >> 8;
    x ^= x >> 4;
    x ^= x >> 2;
    x ^= x >> 1;
    x
}

/// [`gray`] for 64-bit values.
#[inline]
pub const fn gray64(x: u64) -> u64 {
    x ^ (x >> 1)
}

/// [`ungray`] for 64-bit values.
#[inline]
pub const fn ungray64(g: u64) -> u64 {
    let mut x = g;
    x ^= x >> 32;
    x ^= x >> 16;
    x ^= x >> 8;
    x ^= x >> 4;
    x ^= x >> 2;
    x ^= x >> 1;
    x
}

/// The table of Gray codes of `0 .. N` (N a power of two up to 256), for symbol mappers that index by symbol (WSJT `GRAY8`,
/// `GRAY4`; the `[0, 1, 3, 2]` axis of square QAM).
pub const fn table<const N: usize>() -> [u8; N] {
    let mut t = [0u8; N];
    let mut i = 0;
    while i < N {
        t[i] = gray(i as u32) as u8;
        i += 1;
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_one_bit_steps() {
        for x in 0..70_000u32 {
            assert_eq!(ungray(gray(x)), x);
            assert_eq!((gray(x) ^ gray(x + 1)).count_ones(), 1);
        }
        for x in [0u64, 1, u64::MAX, 0x8000_0000_0000_0001, 123_456_789_012] {
            assert_eq!(ungray64(gray64(x)), x);
        }
        assert_eq!(table::<4>(), [0, 1, 3, 2]);
        assert_eq!(table::<8>(), [0, 1, 3, 2, 6, 7, 5, 4]);
    }
}
