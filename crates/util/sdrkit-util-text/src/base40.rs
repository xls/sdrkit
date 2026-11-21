//! Base-40 packing of callsigns: several characters from a 40-character alphabet held as one integer.
//!
//! Three alphabets are in use, and none of them is D-STAR's (D-STAR sends callsigns as eight plain ASCII characters):
//!
//! | Variant | Alphabet (value 0 first) | Order | Length | Standard |
//! |---|---|---|---|---|
//! | [`M17`] | space, A-Z, 0-9, `-`, `/`, `.` | first character least significant | 9 (48-bit field) | M17 Protocol Specification, "Address encoding" |
//! | [`SSDV`] | (none), 0-9 at 1, A-Z at 14 | first character least significant | 6 (32-bit field) | SSDV packet format (callsign field) |
//! | UAT ([`uat_ident`]) | 0-9, A-Z, space at 36 | three 16-bit words, two or three characters each, most significant first | 8 | RTCA DO-282B Mode Status |
//!
//! ```
//! use sdrkit_util_text::base40::M17;
//!
//! assert_eq!(M17.encode("AB1CD").unwrap(), 0x9F_DD51);
//! let mut s = String::new();
//! M17.decode_into(0x9F_DD51, &mut s).unwrap();
//! assert_eq!(s, "AB1CD");
//! ```

use crate::error::{Error, Result};

/// A base-40 alphabet with the first character in the least significant place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Base40 {
    /// The character of each value (`\0`: unassigned; value 0 is the padding).
    pub alphabet: [u8; 40],
    /// The most characters a value holds.
    pub max_len: usize,
}

const fn alphabet(s: &[u8]) -> [u8; 40] {
    let mut a = [0u8; 40];
    let mut i = 0;
    while i < 40 && i < s.len() {
        a[i] = s[i];
        i += 1;
    }
    a
}

/// The M17 address alphabet (nine characters in 48 bits).
pub const M17: Base40 = Base40 {
    alphabet: alphabet(b" ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-/."),
    max_len: 9,
};

/// The SSDV callsign alphabet (six characters in 32 bits; values 11 to 13 unassigned).
pub const SSDV: Base40 = Base40 {
    alphabet: alphabet(b"\x000123456789\0\0\0ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    max_len: 6,
};

/// The largest M17 callsign value (40^9 - 1); larger values are special.
pub const M17_MAX: u64 = 262_143_999_999_999;
/// The M17 broadcast address ("@ALL").
pub const M17_BROADCAST: u64 = 0xFFFF_FFFF_FFFF;
/// The first M17 value of a callsign that starts with `#` (40^9); up to eight characters follow.
pub const M17_HASH: u64 = 262_144_000_000_000;

impl Base40 {
    fn value(&self, c: char) -> Result<u64> {
        let u = c.to_ascii_uppercase();
        if u == '\0' || !u.is_ascii() {
            return Err(Error::Unencodable(c));
        }
        self.alphabet
            .iter()
            .position(|&a| a == u as u8 && a != 0)
            .map(|i| i as u64)
            .ok_or(Error::Unencodable(c))
    }

    /// The value of a callsign (letters folded to upper case).
    pub fn encode(&self, s: &str) -> Result<u64> {
        if s.chars().count() > self.max_len {
            return Err(Error::TooLong);
        }
        let mut v = 0u64;
        for c in s.chars().rev() {
            v = v * 40 + self.value(c)?;
        }
        Ok(v)
    }

    /// Appends the callsign of a value (padding and trailing spaces removed).
    pub fn decode_into(&self, mut v: u64, out: &mut String) -> Result<()> {
        let max = 40u64.checked_pow(self.max_len as u32).unwrap_or(u64::MAX);
        if v >= max {
            return Err(Error::OutOfRange);
        }
        let start = out.len();
        while v > 0 {
            let d = (v % 40) as usize;
            v /= 40;
            match self.alphabet[d] {
                0 if d == 0 => out.push(' '),
                0 => {
                    out.truncate(start);
                    return Err(Error::UnknownCode(d as u32));
                }
                a => out.push(a as char),
            }
        }
        let keep = out[start..].trim_end().len();
        out.truncate(start + keep);
        Ok(())
    }
}

/// An M17 address: a callsign, a `#` callsign, or the broadcast address.
pub fn m17_decode_into(v: u64, out: &mut String) -> Result<()> {
    match v {
        M17_BROADCAST => {
            out.push_str("@ALL");
            Ok(())
        }
        0..=M17_MAX => M17.decode_into(v, out),
        _ if v < M17_HASH + 40u64.pow(8) => {
            out.push('#');
            M17.decode_into(v - M17_HASH, out)
        }
        _ => Err(Error::OutOfRange),
    }
}

/// The M17 address of a callsign, `#` callsign or "@ALL".
pub fn m17_encode(s: &str) -> Result<u64> {
    if s == "@ALL" {
        return Ok(M17_BROADCAST);
    }
    match s.strip_prefix('#') {
        Some(rest) if rest.chars().count() <= 8 => Ok(M17_HASH + M17.encode(rest)?),
        Some(_) => Err(Error::TooLong),
        None => M17.encode(s),
    }
}

/// The UAT character of a value: 0-9, A-Z, space at 36 (37 to 39 are not assigned here).
pub fn uat_char(v: u16) -> Result<char> {
    match v {
        0..=9 => Ok((b'0' + v as u8) as char),
        10..=35 => Ok((b'A' + v as u8 - 10) as char),
        36 => Ok(' '),
        _ => Err(Error::UnknownCode(v as u32)),
    }
}

fn uat_value(c: char) -> Result<u16> {
    match c.to_ascii_uppercase() {
        d @ '0'..='9' => Ok(d as u16 - '0' as u16),
        u @ 'A'..='Z' => Ok(u as u16 - 'A' as u16 + 10),
        ' ' => Ok(36),
        _ => Err(Error::Unencodable(c)),
    }
}

/// Appends the eight-character identification of a UAT Mode Status payload from its three 16-bit words; the first word also holds
/// the emitter category, returned. Trailing spaces are removed.
pub fn uat_ident(words: [u16; 3], out: &mut String) -> Result<u8> {
    let [a, b, c] = words;
    let digits = [
        a / 40 % 40,
        a % 40,
        b / 1600 % 40,
        b / 40 % 40,
        b % 40,
        c / 1600 % 40,
        c / 40 % 40,
        c % 40,
    ];
    let start = out.len();
    for d in digits {
        match uat_char(d) {
            Ok(ch) => out.push(ch),
            Err(e) => {
                out.truncate(start);
                return Err(e);
            }
        }
    }
    let keep = out[start..].trim_end().len();
    out.truncate(start + keep);
    Ok((a / 1600 % 40) as u8)
}

/// The three words of a UAT identification (up to eight characters, padded with spaces) with an emitter category (0 to 39).
pub fn uat_ident_encode(category: u8, s: &str) -> Result<[u16; 3]> {
    if category >= 40 {
        return Err(Error::OutOfRange);
    }
    if s.chars().count() > 8 {
        return Err(Error::TooLong);
    }
    let mut v = [36u16; 8];
    for (d, c) in v.iter_mut().zip(s.chars()) {
        *d = uat_value(c)?;
    }
    Ok([
        category as u16 * 1600 + v[0] * 40 + v[1],
        v[2] * 1600 + v[3] * 40 + v[4],
        v[5] * 1600 + v[6] * 40 + v[7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m17_address_encoding() {
        // The worked example of the M17 specification: "AB1CD" is 0x00000009FDD51.
        assert_eq!(m17_encode("AB1CD"), Ok(0x0000_0000_009F_DD51));
        assert_eq!(M17.encode("AB1CD"), Ok(0x9F_DD51));
        // A = 1, B = 2, '1' = 28, C = 3, D = 4.
        assert_eq!(
            1 + 2 * 40 + 28 * 1600 + 3 * 64_000 + 4 * 2_560_000,
            0x9F_DD51
        );
        let mut s = String::new();
        m17_decode_into(0x9F_DD51, &mut s).unwrap();
        assert_eq!(s, "AB1CD");
        s.clear();
        m17_decode_into(M17_BROADCAST, &mut s).unwrap();
        assert_eq!(s, "@ALL");
        assert_eq!(m17_encode("@ALL"), Ok(M17_BROADCAST));
        let h = m17_encode("#PARROT").unwrap();
        assert!(h > M17_MAX);
        s.clear();
        m17_decode_into(h, &mut s).unwrap();
        assert_eq!(s, "#PARROT");
        assert_eq!(
            M17.encode("W2FBI/M.-"),
            Ok(M17.encode("w2fbi/m.-").unwrap())
        );
        assert_eq!(M17.encode("TOOLONGCALL"), Err(Error::TooLong));
        assert_eq!(M17.encode("A_B"), Err(Error::Unencodable('_')));
        assert_eq!(M17.encode("ZZZZZZZZZ").map(|v| v <= M17_MAX), Ok(true));
        assert_eq!(
            m17_decode_into(0xFFFF_FFFF_FFFE, &mut s),
            Err(Error::OutOfRange)
        );
    }

    #[test]
    fn ssdv_callsign() {
        // '0' = 1, 'A' = 14, first character least significant.
        assert_eq!(SSDV.encode("A0"), Ok(14 + 40));
        let v = SSDV.encode("M0XYZ").unwrap();
        let mut s = String::new();
        SSDV.decode_into(v, &mut s).unwrap();
        assert_eq!(s, "M0XYZ");
        assert_eq!(SSDV.encode("A-B"), Err(Error::Unencodable('-')));
        assert_eq!(SSDV.decode_into(11, &mut s), Err(Error::UnknownCode(11)));
        assert_eq!(
            SSDV.decode_into(40u64.pow(6), &mut s),
            Err(Error::OutOfRange)
        );
    }

    #[test]
    fn uat_mode_status_ident() {
        let w = uat_ident_encode(1, "N123AB").unwrap();
        // N = 23, '1' = 1: the first word is category 1 * 1600 + 23 * 40 + 1.
        assert_eq!(w[0], 1600 + 23 * 40 + 1);
        let mut s = String::new();
        assert_eq!(uat_ident(w, &mut s), Ok(1));
        assert_eq!(s, "N123AB");
        assert_eq!(uat_ident([37, 0, 0], &mut s), Err(Error::UnknownCode(37)));
        assert_eq!(uat_ident_encode(40, ""), Err(Error::OutOfRange));
    }
}
