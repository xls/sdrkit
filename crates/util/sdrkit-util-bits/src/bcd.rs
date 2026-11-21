//! Binary-coded decimal: two decimal digits per byte. Every decoder here refuses a nibble above 9 (instead of hiding it with
//! `% 10`) unless the format provides that nibble a meaning (a filler or a sign).
//!
//! ```
//! use sdrkit_util_bits::bcd;
//! assert_eq!(bcd::decode(&[0x12, 0x34], 4), Ok(1234));
//! assert!(bcd::decode(&[0x1A], 2).is_err());
//! assert_eq!(bcd::plmn(&[0x32, 0xF4, 0x51]), Ok((234, 15, 2))); // MCC 234, MNC 15 (3GPP TS 24.008)
//! ```

use crate::Param;
use crate::{Error, Result};

/// The value of one BCD byte (00 to 99), or `None` when a nibble is above 9.
#[inline]
pub const fn byte(b: u8) -> Option<u8> {
    let (h, l) = (b >> 4, b & 0xF);
    if h > 9 || l > 9 {
        None
    } else {
        Some(h * 10 + l)
    }
}

/// The BCD byte of `v` (0 to 99); `None` above 99.
#[inline]
pub const fn to_byte(v: u8) -> Option<u8> {
    if v > 99 {
        None
    } else {
        Some(((v / 10) << 4) | (v % 10))
    }
}

/// The value of the first `digits` digits of `bytes`, most significant first, high nibble first (MPEG and DVB times, MJD as BCD,
/// most protocols). An error for a nibble above 9 (`at` is the digit index), too few bytes, or more than 19 digits.
pub fn decode(bytes: &[u8], digits: usize) -> Result<u64> {
    if digits > 19 {
        return Err(Error::BadParameter(Param::BcdDigits));
    }
    if bytes.len() * 2 < digits {
        return Err(Error::InputTooShort);
    }
    let mut v = 0u64;
    for i in 0..digits {
        let b = bytes[i / 2];
        let d = if i % 2 == 0 { b >> 4 } else { b & 0xF };
        if d > 9 {
            return Err(Error::Invalid { at: i });
        }
        v = v * 10 + d as u64;
    }
    Ok(v)
}

/// Writes `v` as `digits` BCD digits (high nibble first) into `out`, padding the last nibble with 0 for an odd count. An error
/// when `v` has more digits or `out` is too short.
pub fn encode(v: u64, digits: usize, out: &mut [u8]) -> Result<()> {
    let need = digits.div_ceil(2);
    if out.len() < need {
        return Err(Error::OutputTooShort { need });
    }
    if digits < 20 && v >= 10u64.pow(digits as u32) {
        return Err(Error::BadParameter(Param::Digits));
    }
    out[..need].fill(0);
    let mut x = v;
    for i in (0..digits).rev() {
        let d = (x % 10) as u8;
        x /= 10;
        out[i / 2] |= if i % 2 == 0 { d << 4 } else { d };
    }
    Ok(())
}

/// The value of little-endian BCD bytes (EN 13757-3, M-Bus data type A: the first byte holds the two lowest digits). A high
/// nibble 0xF in the last byte means negative (the magnitude is the other digits). An error for any other nibble above 9.
pub fn decode_le(bytes: &[u8]) -> Result<i64> {
    if bytes.len() > 9 {
        return Err(Error::BadParameter(Param::ByteCount));
    }
    let mut v = 0i64;
    let mut neg = false;
    for (i, &b) in bytes.iter().enumerate().rev() {
        let (h, l) = (b >> 4, b & 0xF);
        if i + 1 == bytes.len() && h == 0xF {
            neg = true;
        } else if h > 9 {
            return Err(Error::Invalid { at: 2 * i + 1 });
        } else {
            v = v * 10 + h as i64;
        }
        if l > 9 {
            return Err(Error::Invalid { at: 2 * i });
        }
        v = v * 10 + l as i64;
    }
    Ok(if neg { -v } else { v })
}

/// Telephony BCD digits (3GPP TS 24.008 clause 10.5.1.4 and TS 29.002 TBCD): each byte holds two digits, **low nibble first**,
/// and a 0xF nibble is a filler that ends the digits. Appends the digits (0 to 9) to `out` and returns how many; an error for a
/// nibble 0xA to 0xE.
pub fn tbcd(bytes: &[u8], out: &mut Vec<u8>) -> Result<usize> {
    let start = out.len();
    for (i, &b) in bytes.iter().enumerate() {
        for (k, d) in [b & 0xF, b >> 4].into_iter().enumerate() {
            match d {
                0..=9 => out.push(d),
                0xF => return Ok(out.len() - start),
                _ => return Err(Error::Invalid { at: 2 * i + k }),
            }
        }
    }
    Ok(out.len() - start)
}

/// MCC and MNC from the 3-byte PLMN identity of 3GPP TS 24.008 figure 10.5.13 (byte 1: MCC digit 2, MCC digit 1; byte 2: MNC
/// digit 3 or 0xF, MCC digit 3; byte 3: MNC digit 2, MNC digit 1). Returns `(mcc, mnc, mnc_digits)`.
pub fn plmn(b: &[u8; 3]) -> Result<(u16, u16, u8)> {
    let d = |x: u8, at: usize| {
        if x > 9 {
            Err(Error::Invalid { at })
        } else {
            Ok(x as u16)
        }
    };
    let mcc = d(b[0] & 0xF, 0)? * 100 + d(b[0] >> 4, 1)? * 10 + d(b[1] & 0xF, 2)?;
    let m1 = d(b[2] & 0xF, 3)?;
    let m2 = d(b[2] >> 4, 4)?;
    if b[1] >> 4 == 0xF {
        Ok((mcc, m1 * 10 + m2, 2))
    } else {
        let m3 = d(b[1] >> 4, 5)?;
        Ok((mcc, m1 * 100 + m2 * 10 + m3, 3))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_bcd() {
        assert_eq!(byte(0x59), Some(59));
        assert_eq!(byte(0x5A), None);
        assert_eq!(to_byte(42), Some(0x42));
        assert_eq!(to_byte(100), None);
        // ETSI EN 300 468 annex C: 12:45:00 is coded 0x124500.
        assert_eq!(decode(&[0x12, 0x45, 0x00], 6), Ok(124_500));
        assert_eq!(decode(&[0x12, 0xA5], 4), Err(Error::Invalid { at: 2 }));
        assert_eq!(decode(&[0x12], 3), Err(Error::InputTooShort));
        let mut o = [0u8; 3];
        encode(12345, 5, &mut o).unwrap();
        assert_eq!(o, [0x12, 0x34, 0x50]);
        assert!(encode(123_456, 5, &mut o).is_err());
        assert!(encode(1, 7, &mut o).is_err());
    }

    #[test]
    fn little_endian_with_sign() {
        assert_eq!(decode_le(&[0x78, 0x56, 0x34, 0x12]), Ok(12_345_678));
        assert_eq!(decode_le(&[0x34, 0xF2]), Ok(-234));
        assert!(decode_le(&[0x3B]).is_err());
    }

    #[test]
    fn telephony_digits() {
        // 3GPP TS 24.008 examples: MCC 234 MNC 15 is 32 F4 51; MCC 310 MNC 260 is 13 00 62.
        assert_eq!(plmn(&[0x32, 0xF4, 0x51]), Ok((234, 15, 2)));
        assert_eq!(plmn(&[0x13, 0x00, 0x62]), Ok((310, 260, 3)));
        assert!(plmn(&[0x3A, 0xF4, 0x51]).is_err());
        let mut d = Vec::new();
        assert_eq!(tbcd(&[0x21, 0x43, 0xF5], &mut d), Ok(5));
        assert_eq!(d, [1, 2, 3, 4, 5]);
        assert!(tbcd(&[0xB1], &mut d).is_err());
    }
}
