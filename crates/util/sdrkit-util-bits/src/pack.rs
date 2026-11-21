//! Bit order, packing and unpacking, and fields on one-bit-per-byte slices.
//!
//! Hard bits in flight are held one per byte (0 or 1); frames at rest are packed bytes with an explicit [`BitOrder`]. The
//! conversions here work 8 bits at a time: unpacking writes 8 output bytes from one table entry, and packing gathers 8 input bytes
//! with one 64-bit multiply.
//!
//! ```
//! use sdrkit_util_bits::pack::{pack, unpack, to_u64};
//! use sdrkit_util_bits::{BitOrder, Pad};
//! let mut bits = Vec::new();
//! unpack(&[0xA5], BitOrder::Msb, &mut bits);
//! assert_eq!(bits, [1, 0, 1, 0, 0, 1, 0, 1]);
//! assert_eq!(to_u64(&bits), 0xA5);
//! let mut bytes = Vec::new();
//! pack(&bits, BitOrder::Lsb, Pad::Zero, &mut bytes);
//! assert_eq!(bytes, [0xA5]);
//! ```

use crate::Param;
use crate::{Error, Result};

/// The order of bits inside a byte: which bit is first in time (or first in the bit string).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BitOrder {
    /// The most significant bit first (most framing, CRC-on-bytes of non-reflected CRCs, MPEG, CCSDS).
    Msb,
    /// The least significant bit first (HDLC, AX.25, UARTs, reflected CRCs).
    Lsb,
}

/// What [`pack`] does with a final partial byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pad {
    /// Fill it with zero bits and keep it.
    Zero,
    /// Leave the extra bits out.
    Drop,
}

/// The 256 bytes with their bits reversed.
pub const REVERSED: [u8; 256] = {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = (i as u8).reverse_bits();
        i += 1;
    }
    t
};

/// Each byte spread to 8 bytes, MSB first: little-endian byte `k` of entry `b` is bit `7 - k` of `b`.
const SPREAD_MSB: [u64; 256] = {
    let mut t = [0u64; 256];
    let mut b = 0;
    while b < 256 {
        let mut v = 0u64;
        let mut k = 0;
        while k < 8 {
            v |= (((b >> (7 - k)) & 1) as u64) << (8 * k);
            k += 1;
        }
        t[b] = v;
        b += 1;
    }
    t
};

/// Each byte spread to 8 bytes, LSB first.
const SPREAD_LSB: [u64; 256] = {
    let mut t = [0u64; 256];
    let mut b = 0;
    while b < 256 {
        let mut v = 0u64;
        let mut k = 0;
        while k < 8 {
            v |= (((b >> k) & 1) as u64) << (8 * k);
            k += 1;
        }
        t[b] = v;
        b += 1;
    }
    t
};

#[inline]
fn spread(b: u8, order: BitOrder) -> [u8; 8] {
    match order {
        BitOrder::Msb => SPREAD_MSB[b as usize].to_le_bytes(),
        BitOrder::Lsb => SPREAD_LSB[b as usize].to_le_bytes(),
    }
}

/// Gathers 8 hard bits (only the lowest bit of each is read) into a byte.
#[inline]
pub fn gather8(bits: [u8; 8], order: BitOrder) -> u8 {
    let v = u64::from_le_bytes(bits) & 0x0101_0101_0101_0101;
    // Every partial product lands on a distinct power of two, so no carries mix the bits.
    match order {
        BitOrder::Msb => (v.wrapping_mul(0x8040_2010_0804_0201) >> 56) as u8,
        BitOrder::Lsb => (v.wrapping_mul(0x0102_0408_1020_4080) >> 56) as u8,
    }
}

/// Appends 8 hard bits per byte of `bytes` to `out`.
pub fn unpack(bytes: &[u8], order: BitOrder, out: &mut Vec<u8>) {
    out.reserve(bytes.len() * 8);
    for &b in bytes {
        out.extend_from_slice(&spread(b, order));
    }
}

/// Appends the first `n` bits of `bytes` to `out`; an error when `bytes` holds fewer than `n` bits.
pub fn unpack_n(bytes: &[u8], n: usize, order: BitOrder, out: &mut Vec<u8>) -> Result<()> {
    if n > bytes.len() * 8 {
        return Err(Error::InputTooShort);
    }
    unpack(&bytes[..n / 8], order, out);
    if !n.is_multiple_of(8) {
        out.extend_from_slice(&spread(bytes[n / 8], order)[..n % 8]);
    }
    Ok(())
}

/// Writes 8 hard bits per byte of `bytes` into `out`; returns the number written (`8 * bytes.len()`).
pub fn unpack_into(bytes: &[u8], order: BitOrder, out: &mut [u8]) -> Result<usize> {
    let need = bytes.len() * 8;
    if out.len() < need {
        return Err(Error::OutputTooShort { need });
    }
    for (o, &b) in out.as_chunks_mut::<8>().0.iter_mut().zip(bytes) {
        o.copy_from_slice(&spread(b, order));
    }
    Ok(need)
}

/// Appends `bits` packed into bytes to `out`; returns the number of bytes appended. A final partial byte is padded with zeros or
/// dropped, as `pad` says.
pub fn pack(bits: &[u8], order: BitOrder, pad: Pad, out: &mut Vec<u8>) -> usize {
    let start = out.len();
    let (chunks, rest) = bits.as_chunks::<8>();
    out.reserve(bits.len() / 8 + 1);
    for c in chunks {
        let mut a = [0u8; 8];
        a.copy_from_slice(c);
        out.push(gather8(a, order));
    }
    if !rest.is_empty() && pad == Pad::Zero {
        let mut a = [0u8; 8];
        a[..rest.len()].copy_from_slice(rest);
        out.push(gather8(a, order));
    }
    out.len() - start
}

/// Writes `bits` packed into `out`; returns the number of bytes written.
pub fn pack_into(bits: &[u8], order: BitOrder, pad: Pad, out: &mut [u8]) -> Result<usize> {
    let need = match pad {
        Pad::Zero => bits.len().div_ceil(8),
        Pad::Drop => bits.len() / 8,
    };
    if out.len() < need {
        return Err(Error::OutputTooShort { need });
    }
    for (o, c) in out.iter_mut().zip(bits.chunks(8)).take(need) {
        let mut a = [0u8; 8];
        a[..c.len()].copy_from_slice(c);
        *o = gather8(a, order);
    }
    Ok(need)
}

/// The value of `bits`, first bit most significant. It works as a shift register: with more than 64 bits, the value is that of the
/// last 64.
#[inline]
pub fn to_u64(bits: &[u8]) -> u64 {
    bits.iter().fold(0u64, |v, &b| (v << 1) | (b & 1) as u64)
}

/// The value of `bits`, first bit least significant. Bits past the 64th are ignored.
#[inline]
pub fn to_u64_lsb(bits: &[u8]) -> u64 {
    bits.iter()
        .take(64)
        .enumerate()
        .fold(0u64, |v, (i, &b)| v | ((b & 1) as u64) << i)
}

/// Appends the `n` low bits of `v` to `out`, most significant first. For `n` above 64 the extra leading bits are zero.
pub fn push_u64(out: &mut Vec<u8>, v: u64, n: u32) {
    out.reserve(n as usize);
    for i in (0..n).rev() {
        out.push(if i < 64 { ((v >> i) & 1) as u8 } else { 0 });
    }
}

/// Appends the `n` low bits of `v` to `out`, least significant first. For `n` above 64 the extra trailing bits are zero.
pub fn push_u64_lsb(out: &mut Vec<u8>, v: u64, n: u32) {
    out.reserve(n as usize);
    for i in 0..n {
        out.push(if i < 64 { ((v >> i) & 1) as u8 } else { 0 });
    }
}

/// Writes the `n` low bits of `v` into `bits[start..start + n]`, most significant first. An error when the slice is too short or
/// `n` is above 64.
pub fn put_u64(bits: &mut [u8], start: usize, v: u64, n: u32) -> Result<()> {
    if n > 64 {
        return Err(Error::BadParameter(Param::BitCount));
    }
    let end = start.checked_add(n as usize).ok_or(Error::BadLength)?;
    let dst = bits
        .get_mut(start..end)
        .ok_or(Error::OutputTooShort { need: end })?;
    for (i, d) in dst.iter_mut().enumerate() {
        *d = ((v >> (n as usize - 1 - i)) & 1) as u8;
    }
    Ok(())
}

/// The unsigned field of `n` bits (at most 64) at `start`, most significant first; `None` when it does not fit in `bits`.
#[inline]
pub fn field(bits: &[u8], start: usize, n: u32) -> Option<u64> {
    if n > 64 {
        return None;
    }
    bits.get(start..start.checked_add(n as usize)?).map(to_u64)
}

/// The unsigned field of `n` bits at `start`, least significant bit first.
#[inline]
pub fn field_lsb(bits: &[u8], start: usize, n: u32) -> Option<u64> {
    if n > 64 {
        return None;
    }
    bits.get(start..start.checked_add(n as usize)?)
        .map(to_u64_lsb)
}

/// The two's complement field of `n` bits at `start`, most significant first.
#[inline]
pub fn field_signed(bits: &[u8], start: usize, n: u32) -> Option<i64> {
    field(bits, start, n).map(|u| sign_extend(u, n))
}

/// The sign-and-magnitude field of `n` bits at `start` (first bit 1 means negative), as GLONASS and the wave buoys use.
#[inline]
pub fn field_sm(bits: &[u8], start: usize, n: u32) -> Option<i64> {
    field(bits, start, n).map(|u| sign_magnitude(u, n))
}

/// A field that the standard sends in two parts, as one unsigned number: the `hi.2` bits at `hi.1` of slice `hi.0` are the
/// most significant part, then the `lo.2` bits at `lo.1` of slice `lo.0`. The parts can be in the same slice or in two (two
/// subframes). Returns `None` when a part does not fit in its slice or the parts hold more than 64 bits together.
/// GPS LNAV (IS-GPS-200 clause 20.3.3) and BeiDou D1 (BDS-SIS-ICD-B1I clause 5.2.4) split several fields this way.
///
/// ```
/// use sdrkit_util_bits::pack::field_split;
/// let a = [1, 0, 0, 1];
/// let b = [1, 1, 0];
/// assert_eq!(field_split((&a, 0, 2), (&b, 0, 3)), Some(0b10_110));
/// ```
#[inline]
pub fn field_split(hi: (&[u8], usize, u32), lo: (&[u8], usize, u32)) -> Option<u64> {
    if hi.2 + lo.2 > 64 {
        return None;
    }
    let h = field(hi.0, hi.1, hi.2)?;
    let l = field(lo.0, lo.1, lo.2)?;
    Some(if lo.2 >= 64 { l } else { (h << lo.2) | l })
}

/// A two's complement field sent in two parts (see [`field_split`]); the sign bit is the first bit of the `hi` part.
#[inline]
pub fn field_split_signed(hi: (&[u8], usize, u32), lo: (&[u8], usize, u32)) -> Option<i64> {
    field_split(hi, lo).map(|u| sign_extend(u, hi.2 + lo.2))
}

/// `u` read as an `n`-bit two's complement number.
#[inline]
pub const fn sign_extend(u: u64, n: u32) -> i64 {
    if n == 0 {
        0
    } else if n >= 64 {
        u as i64
    } else {
        let s = 64 - n;
        ((u << s) as i64) >> s
    }
}

/// `u` read as an `n`-bit sign-and-magnitude number (the top bit is the sign).
#[inline]
pub const fn sign_magnitude(u: u64, n: u32) -> i64 {
    if n == 0 {
        return 0;
    }
    let mag = (u & mask(n - 1)) as i64;
    if n <= 64 && (u >> (n - 1)) & 1 == 1 {
        -mag
    } else {
        mag
    }
}

/// `v` as an `n`-bit sign-and-magnitude number (the top bit is the sign, 1 for negative), the inverse of
/// [`sign_magnitude`]. A magnitude above the `n − 1` bits saturates at the largest one. Zero and negative zero give 0.
/// For `n` of 0 the result is 0.
///
/// ```
/// use sdrkit_util_bits::pack::{sign_magnitude, to_sign_magnitude};
/// assert_eq!(to_sign_magnitude(-5, 12), 0x805);
/// assert_eq!(to_sign_magnitude(5000, 12), 0x7FF);
/// assert_eq!(sign_magnitude(to_sign_magnitude(-300, 12), 12), -300);
/// ```
#[inline]
pub const fn to_sign_magnitude(v: i64, n: u32) -> u64 {
    if n == 0 {
        return 0;
    }
    let max = mask(n - 1);
    let mag = if v.unsigned_abs() > max {
        max
    } else {
        v.unsigned_abs()
    };
    if v < 0 && mag != 0 && n <= 64 {
        (1 << (n - 1)) | mag
    } else {
        mag
    }
}

/// The `n` low bits set (all 64 for `n >= 64`).
#[inline]
pub const fn mask(n: u32) -> u64 {
    if n >= 64 {
        u64::MAX
    } else {
        (1u64 << n) - 1
    }
}

/// The `n` low bits of `x` in reverse order (bit 0 swaps with bit `n - 1`). Bits above `n` are dropped.
#[inline]
pub const fn reverse_bits(x: u64, n: u32) -> u64 {
    if n == 0 {
        0
    } else if n >= 64 {
        x.reverse_bits()
    } else {
        x.reverse_bits() >> (64 - n)
    }
}

/// Reverses the order of the bits inside each whole group of 8 (the HDLC octet order flip). A final partial group is left as it is.
pub fn reverse_each_octet(bits: &mut [u8]) {
    for c in bits.as_chunks_mut::<8>().0 {
        c.reverse();
    }
}

/// Reverses the bits of each byte in place.
pub fn reverse_each_byte(bytes: &mut [u8]) {
    for b in bytes {
        *b = REVERSED[*b as usize];
    }
}

/// Appends hex digits (upper case) for `bits`, most significant first; a final partial digit is padded with zero bits.
pub fn bits_to_hex(bits: &[u8], out: &mut String) {
    for c in bits.chunks(4) {
        let mut d = 0u32;
        for i in 0..4 {
            d = (d << 1) | c.get(i).map_or(0, |b| (b & 1) as u32);
        }
        out.push(char::from(b"0123456789ABCDEF"[d as usize]));
    }
}

/// Appends 4 bits per hex digit of `hex` to `out`, most significant first. An error (with the index of the digit) for any other
/// character.
pub fn hex_to_bits(hex: &str, out: &mut Vec<u8>) -> Result<()> {
    for (i, c) in hex.bytes().enumerate() {
        let d = (c as char).to_digit(16).ok_or(Error::Invalid { at: i })?;
        for k in (0..4).rev() {
            out.push(((d >> k) & 1) as u8);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn naive_unpack(b: u8, order: BitOrder) -> [u8; 8] {
        let mut o = [0u8; 8];
        for (k, x) in o.iter_mut().enumerate() {
            *x = match order {
                BitOrder::Msb => (b >> (7 - k)) & 1,
                BitOrder::Lsb => (b >> k) & 1,
            };
        }
        o
    }

    #[test]
    fn pack_and_unpack_every_byte_both_orders() {
        for order in [BitOrder::Msb, BitOrder::Lsb] {
            for b in 0..=255u8 {
                let bits = naive_unpack(b, order);
                let mut v = Vec::new();
                unpack(&[b], order, &mut v);
                assert_eq!(v, bits);
                assert_eq!(gather8(bits, order), b);
            }
        }
    }

    #[test]
    fn partial_bytes_are_padded_or_dropped() {
        let bits = [1u8, 1, 0, 1, 1, 1, 1, 1, 1, 0, 1];
        let mut v = Vec::new();
        assert_eq!(pack(&bits, BitOrder::Msb, Pad::Zero, &mut v), 2);
        assert_eq!(v, [0xDF, 0xA0]);
        v.clear();
        assert_eq!(pack(&bits, BitOrder::Msb, Pad::Drop, &mut v), 1);
        assert_eq!(v, [0xDF]);
        let mut o = [0u8; 1];
        assert_eq!(
            pack_into(&bits, BitOrder::Msb, Pad::Zero, &mut o),
            Err(Error::OutputTooShort { need: 2 })
        );
        let mut u = Vec::new();
        unpack_n(&[0xDF, 0xA0], 11, BitOrder::Msb, &mut u).unwrap();
        assert_eq!(u, bits);
        assert!(unpack_n(&[0], 9, BitOrder::Msb, &mut u).is_err());
        // Only the lowest bit of each byte counts.
        assert_eq!(gather8([3, 2, 2, 2, 2, 2, 2, 2], BitOrder::Msb), 0x80);
    }

    #[test]
    fn fields_signed_and_sign_magnitude() {
        let mut bits = vec![0u8; 20];
        put_u64(&mut bits, 3, 0b10110, 5).unwrap();
        assert_eq!(field(&bits, 3, 5), Some(0b10110));
        assert_eq!(field_signed(&bits, 3, 5), Some(-10));
        assert_eq!(field_sm(&bits, 3, 5), Some(-6));
        assert_eq!(field(&bits, 18, 5), None);
        assert!(put_u64(&mut bits, 18, 0, 5).is_err());
        assert_eq!(field_lsb(&[1, 0, 0, 1], 0, 4), Some(9));
        assert_eq!(sign_extend(0xFFFF_FFFF_FFFF_FFFF, 64), -1);
        assert_eq!(sign_magnitude(0b1000, 4), 0);
        let mut v = Vec::new();
        push_u64(&mut v, 0x2757, 14);
        assert_eq!(to_u64(&v), 0x2757);
        v.clear();
        push_u64_lsb(&mut v, 0x2757, 14);
        assert_eq!(to_u64_lsb(&v), 0x2757);
    }

    #[test]
    fn reversals() {
        assert_eq!(reverse_bits(0b0001, 4), 0b1000);
        assert_eq!(reverse_bits(0x1, 64), 1 << 63);
        assert_eq!(REVERSED[0x01], 0x80);
        let mut b = [1u8, 0, 0, 0, 0, 0, 0, 0, 1, 1];
        reverse_each_octet(&mut b);
        assert_eq!(b, [0, 0, 0, 0, 0, 0, 0, 1, 1, 1]);
        let mut x = [0x01u8, 0xF0];
        reverse_each_byte(&mut x);
        assert_eq!(x, [0x80, 0x0F]);
    }

    #[test]
    fn hex_both_ways() {
        let mut bits = Vec::new();
        hex_to_bits("1ACF", &mut bits).unwrap();
        assert_eq!(to_u64(&bits), 0x1ACF);
        let mut s = String::new();
        bits_to_hex(&bits[..15], &mut s);
        assert_eq!(s, "1ACE");
        assert_eq!(hex_to_bits("1G", &mut bits), Err(Error::Invalid { at: 1 }));
    }

    #[test]
    fn sign_magnitude_encoder_inverts_the_reader() {
        for n in [2u32, 8, 12, 24, 64] {
            let max = mask(n - 1) as i64;
            for v in [0i64, 1, -1, max, -max, max / 3, -(max / 3)] {
                assert_eq!(sign_magnitude(to_sign_magnitude(v, n), n), v, "n {n} v {v}");
            }
            assert_eq!(to_sign_magnitude(max.saturating_add(1), n), max as u64);
            assert_eq!(to_sign_magnitude(i64::MIN, n), mask(n));
        }
        // The wave buoy displacement form: 12 bits, bit 11 the sign.
        assert_eq!(to_sign_magnitude(-1, 12), 0x801);
        assert_eq!(to_sign_magnitude(-0, 12), 0);
        assert_eq!(to_sign_magnitude(-9999, 12), 0xFFF);
        assert_eq!(to_sign_magnitude(7, 1), 0);
        assert_eq!(to_sign_magnitude(7, 0), 0);
    }

    #[test]
    fn split_fields_join_their_parts() {
        // IS-GPS-200 table 20-I: the IODC of subframe 1 is 10 bits, its 2 MSBs at subframe bits 83 and 84 (word 3) and its 8
        // LSBs at bits 211 to 218 (word 8). Bits are counted from 1.
        let mut sf = [0u8; 300];
        let iodc = 0b10_1010_0101u64;
        put_u64(&mut sf, 82, iodc >> 8, 2).unwrap();
        put_u64(&mut sf, 210, iodc & 0xFF, 8).unwrap();
        assert_eq!(field_split((&sf, 82, 2), (&sf, 210, 8)), Some(iodc));
        // BDS-SIS-ICD-B1I: toe is 17 bits, 2 MSBs at the end of subframe 2 and 15 LSBs at the start of subframe 3.
        let (mut f2, mut f3) = ([0u8; 186], [0u8; 30]);
        put_u64(&mut f2, 184, 0b11, 2).unwrap();
        put_u64(&mut f3, 0, 0x1234, 15).unwrap();
        assert_eq!(
            field_split((&f2, 184, 2), (&f3, 0, 15)),
            Some((0b11 << 15) | 0x1234)
        );
        // Signed: the sign is the first bit of the first part.
        assert_eq!(
            field_split_signed((&f2, 184, 2), (&f3, 0, 15)),
            Some(sign_extend((0b11 << 15) | 0x1234, 17))
        );
        assert_eq!(
            field_split_signed((&[0u8, 1], 0, 2), (&[1u8], 0, 1)),
            Some(3)
        );
        // Parts that do not fit, or more than 64 bits.
        assert_eq!(field_split((&f2, 185, 2), (&f3, 0, 1)), None);
        assert_eq!(field_split((&[0u8; 40], 0, 40), (&[0u8; 40], 0, 25)), None);
        assert_eq!(
            field_split((&[1u8; 64], 0, 0), (&[1u8; 64], 0, 64)),
            Some(u64::MAX)
        );
    }
}
