//! Six-bit character sets: the "6-bit ASCII" of AIS (ITU-R M.1371-5 Annex 8, Table 47) and the aircraft identification set of
//! Mode S and ADS-B (ICAO Annex 10 Volume IV, 3.1.2.9.1.2, a subset of IA-5).
//!
//! The two sets put the same characters on the same codes, but AIS assigns all 64 codes while the ICAO set leaves everything
//! except letters, digits and space undefined.
//!
//! ```
//! use sdrkit_util_text::sixbit;
//!
//! let mut s = String::new();
//! sixbit::icao_ident(0x2C_C371_C32C_E0, &mut s).unwrap();
//! assert_eq!(s, "KLM1023");
//! ```

use crate::error::{Error, Result};

/// The AIS character of a 6-bit value: 0 to 31 are `@` to `_`, 32 to 63 are space to `?`.
pub fn ais_char(v: u8) -> Result<char> {
    match v {
        0..=31 => Ok((b'@' + v) as char),
        32..=63 => Ok(v as char),
        _ => Err(Error::OutOfRange),
    }
}

/// The AIS 6-bit value of a character (lower case letters are folded to upper case).
pub fn ais_code(c: char) -> Result<u8> {
    let u = c.to_ascii_uppercase() as u32;
    match u {
        64..=95 => Ok((u - 64) as u8),
        32..=63 => Ok(u as u8),
        _ => Err(Error::Unencodable(c)),
    }
}

/// Appends AIS text of `n` characters taken from `bits` (one bit per byte, most significant first). Trailing `@` (the padding)
/// and trailing spaces are removed, as M.1371 Table 47 note says `@` marks "not available".
pub fn ais_text_into(bits: &[u8], n: usize, out: &mut String) -> Result<()> {
    if bits.len() < 6 * n {
        return Err(Error::TooShort);
    }
    let start = out.len();
    for ch in bits.as_chunks::<6>().0.iter().take(n) {
        let v = ch.iter().fold(0u8, |a, &b| (a << 1) | (b & 1));
        out.push(ais_char(v)?);
    }
    let keep = out[start..].trim_end_matches(['@', ' ']).len();
    out.truncate(start + keep);
    Ok(())
}

/// The ICAO identification character of a 6-bit value: letters (1 to 26), space (32) and digits (48 to 57); other codes are
/// undefined.
pub fn icao_char(v: u8) -> Result<char> {
    match v {
        1..=26 => Ok((b'A' + v - 1) as char),
        32 => Ok(' '),
        48..=57 => Ok(v as char),
        _ => Err(Error::UnknownCode(v as u32)),
    }
}

/// The ICAO 6-bit value of a letter, digit or space.
pub fn icao_code(c: char) -> Result<u8> {
    match c.to_ascii_uppercase() {
        u @ 'A'..='Z' => Ok(u as u8 - b'A' + 1),
        d @ '0'..='9' => Ok(d as u8),
        ' ' => Ok(32),
        _ => Err(Error::Unencodable(c)),
    }
}

/// Appends the eight characters of a 48-bit identification field (first character in the top bits), without trailing spaces.
/// An undefined code is an error.
pub fn icao_ident(v: u64, out: &mut String) -> Result<()> {
    if v >> 48 != 0 {
        return Err(Error::OutOfRange);
    }
    let start = out.len();
    for i in 0..8 {
        out.push(icao_char(((v >> (42 - 6 * i)) & 0x3F) as u8)?);
    }
    let keep = out[start..].trim_end().len();
    out.truncate(start + keep);
    Ok(())
}

/// The 48-bit identification field of up to eight characters (padded with spaces).
pub fn icao_ident_encode(s: &str) -> Result<u64> {
    if s.chars().count() > 8 {
        return Err(Error::TooLong);
    }
    let mut v = 0u64;
    let mut it = s.chars();
    for _ in 0..8 {
        v = (v << 6) | icao_code(it.next().unwrap_or(' '))? as u64;
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ais_table_47() {
        // Table 47: value, character.
        for (v, c) in [
            (0u8, '@'),
            (1, 'A'),
            (26, 'Z'),
            (27, '['),
            (28, '\\'),
            (29, ']'),
            (30, '^'),
            (31, '_'),
            (32, ' '),
            (33, '!'),
            (47, '/'),
            (48, '0'),
            (57, '9'),
            (63, '?'),
        ] {
            assert_eq!(ais_char(v), Ok(c));
            assert_eq!(ais_code(c), Ok(v));
        }
        assert_eq!(ais_code('a'), Ok(1));
        assert_eq!(ais_code('`'), Err(Error::Unencodable('`')));
        assert_eq!(ais_char(64), Err(Error::OutOfRange));
        // "PILOT@@@" over 8 characters.
        let mut bits = vec![];
        for c in "PILOT@@@".chars() {
            let v = ais_code(c).unwrap();
            bits.extend((0..6).rev().map(|i| (v >> i) & 1));
        }
        let mut s = String::new();
        ais_text_into(&bits, 8, &mut s).unwrap();
        assert_eq!(s, "PILOT");
        assert_eq!(ais_text_into(&bits, 9, &mut s), Err(Error::TooShort));
    }

    #[test]
    fn icao_worked_example() {
        // ADS-B identification message 8D4840D6202CC371C32CE0576098: the 48-bit field 0x2CC371C32CE0 is "KLM1023 ".
        let mut s = String::new();
        icao_ident(0x2CC3_71C3_2CE0, &mut s).unwrap();
        assert_eq!(s, "KLM1023");
        assert_eq!(icao_ident_encode("KLM1023"), Ok(0x2CC3_71C3_2CE0));
        assert_eq!(icao_char(0), Err(Error::UnknownCode(0)));
        assert_eq!(icao_char(27), Err(Error::UnknownCode(27)));
        assert!(icao_ident(1 << 48, &mut s).is_err());
        assert_eq!(icao_ident_encode("ABCDEFGHI"), Err(Error::TooLong));
        assert_eq!(icao_code('-'), Err(Error::Unencodable('-')));
    }
}
