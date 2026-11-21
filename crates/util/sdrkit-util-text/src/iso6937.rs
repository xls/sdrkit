//! ISO/IEC 6937 (2001), the Latin text table of DVB SI and of many teletext-derived services: ASCII in 0x20 to 0x7E, a second
//! half with symbols and letters, and non-spacing diacritical marks (0xC1 to 0xCF) sent before the letter they go on.
//!
//! The table here is the one reproduced in ETSI EN 300 468 Figure A.1 ("table 00"), which is ISO/IEC 6937 plus the euro sign at
//! 0xA4; [`Variant::Iso`] leaves 0xA4 unassigned as ISO/IEC 6937 does. 0xA6, 0xC0, 0xC9, 0xCC, 0xD8 to 0xDB and 0xE5 are unassigned
//! in both. 0x80 to 0x9F are control codes and are passed to the caller.
//!
//! A mark followed by a letter becomes the precomposed letter when Unicode has one, otherwise the letter followed by the combining
//! mark, so nothing is lost.
//!
//! ```
//! use sdrkit_util_text::iso6937::{self, Variant};
//!
//! let mut s = String::new();
//! iso6937::decode_into(b"Pr\xC8ufbild \xA4", Variant::Dvb, &mut s);
//! assert_eq!(s, "Prüfbild €");
//! ```

use crate::compose::Mark;
use crate::error::{Error, Result};

/// Which version of the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    /// ISO/IEC 6937 itself (0xA4 unassigned).
    Iso,
    /// EN 300 468 Figure A.1: with the euro sign at 0xA4.
    #[default]
    Dvb,
}

/// 0xA0 to 0xFF (0: unassigned or a diacritical mark).
const HIGH: [u32; 96] = [
    0x00A0, 0x00A1, 0x00A2, 0x00A3, 0x20AC, 0x00A5, 0, 0x00A7, 0x00A4, 0x2018, 0x201C, 0x00AB,
    0x2190, 0x2191, 0x2192, 0x2193, //
    0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x00D7, 0x00B5, 0x00B6, 0x00B7, 0x00F7, 0x2019, 0x201D, 0x00BB,
    0x00BC, 0x00BD, 0x00BE, 0x00BF, //
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, //
    0x2015, 0x00B9, 0x00AE, 0x00A9, 0x2122, 0x266A, 0x00AC, 0x00A6, 0, 0, 0, 0, 0x215B, 0x215C,
    0x215D, 0x215E, //
    0x2126, 0x00C6, 0x0110, 0x00AA, 0x0126, 0, 0x0132, 0x013F, 0x0141, 0x00D8, 0x0152, 0x00BA,
    0x00DE, 0x0166, 0x014A, 0x0149, //
    0x0138, 0x00E6, 0x0111, 0x00F0, 0x0127, 0x0131, 0x0133, 0x0140, 0x0142, 0x00F8, 0x0153, 0x00DF,
    0x00FE, 0x0167, 0x014B, 0x00AD,
];

/// The mark of a byte 0xC1 to 0xCF (0xC9 and 0xCC are unassigned).
fn mark_of(b: u8) -> Option<Mark> {
    match b {
        0xC1..=0xCF if b != 0xC9 && b != 0xCC => Mark::from_index(b - 0xC0),
        _ => None,
    }
}

/// The character of a single byte (not a mark, not a control code).
pub fn char_of(b: u8, v: Variant) -> Option<char> {
    match b {
        0x20..=0x7E => Some(b as char),
        0xA4 if v == Variant::Iso => None,
        0xA0..=0xFF => match HIGH[(b - 0xA0) as usize] {
            0 => None,
            u => char::from_u32(u),
        },
        _ => None,
    }
}

/// One decoded unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    /// A character.
    Char(char),
    /// A letter with a mark Unicode has no precomposed form for: the letter, then the combining mark.
    Combined(char, char),
    /// A control byte (0x00 to 0x1F, 0x7F, 0x80 to 0x9F), for the caller to interpret.
    Control(u8),
}

/// The units of ISO/IEC 6937 bytes. Unassigned bytes and a mark with no letter after it are skipped.
pub fn units(bytes: &[u8], v: Variant) -> impl Iterator<Item = Unit> + '_ {
    let mut i = 0;
    core::iter::from_fn(move || loop {
        let b = *bytes.get(i)?;
        i += 1;
        match b {
            0x00..=0x1F | 0x7F..=0x9F => return Some(Unit::Control(b)),
            0xC0..=0xCF => {
                let Some(m) = mark_of(b) else { continue };
                let base = match bytes.get(i) {
                    Some(&n) if (0x20..0x7F).contains(&n) => n as char,
                    _ => continue,
                };
                i += 1;
                return Some(match m.compose(base) {
                    Some(c) => Unit::Char(c),
                    None => Unit::Combined(base, m.combining()),
                });
            }
            _ => {
                if let Some(c) = char_of(b, v) {
                    return Some(Unit::Char(c));
                }
            }
        }
    })
}

/// Appends the text of ISO/IEC 6937 bytes; control bytes are dropped.
pub fn decode_into(bytes: &[u8], v: Variant, out: &mut String) {
    for u in units(bytes, v) {
        match u {
            Unit::Char(c) => out.push(c),
            Unit::Combined(a, m) => {
                out.push(a);
                out.push(m);
            }
            Unit::Control(_) => {}
        }
    }
}

/// Appends the ISO/IEC 6937 bytes of a text: precomposed letters become a mark and the letter.
pub fn encode_into(text: &str, v: Variant, out: &mut Vec<u8>) -> Result<()> {
    for c in text.chars() {
        if (' '..='~').contains(&c) {
            out.push(c as u8);
            continue;
        }
        if let Some(i) = HIGH.iter().position(|&u| u == c as u32 && u != 0) {
            let b = 0xA0 + i as u8;
            if !(b == 0xA4 && v == Variant::Iso) {
                out.push(b);
                continue;
            }
        }
        match Mark::decompose(c) {
            Some((base, m)) if !matches!(m, Mark::DotBelow | Mark::LowLine) => {
                out.push(0xC0 + m as u8);
                out.push(base as u8);
            }
            _ => return Err(Error::Unencodable(c)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_a1_entries() {
        for (b, u) in [
            (0xA0, 0x00A0),
            (0xA4, 0x20AC),
            (0xA8, 0x00A4),
            (0xB4, 0x00D7),
            (0xD0, 0x2015),
            (0xD4, 0x2122),
            (0xD5, 0x266A),
            (0xE0, 0x2126),
            (0xEF, 0x0149),
            (0xF0, 0x0138),
            (0xFF, 0x00AD),
        ] {
            assert_eq!(
                char_of(b, Variant::Dvb).map(|c| c as u32),
                Some(u),
                "{b:#x}"
            );
        }
        for b in [0xA6, 0xC0, 0xC9, 0xCC, 0xD8, 0xDB, 0xE5] {
            assert_eq!(char_of(b, Variant::Dvb), None, "{b:#x}");
        }
        assert_eq!(char_of(0xA4, Variant::Iso), None);
    }

    #[test]
    fn diacritics() {
        let mut s = String::new();
        decode_into(
            b"caf\xC2e \xCFS \xCDo \xCBc \xC1q \xC9u \xC8",
            Variant::Dvb,
            &mut s,
        );
        assert_eq!(s, "café Š ő ç q\u{300} u ");
        let mut v = vec![];
        encode_into("Prüfbild café Łódź €", Variant::Dvb, &mut v).unwrap();
        assert_eq!(v, b"Pr\xC8ufbild caf\xC2e \xE8\xC2od\xC2z \xA4");
        assert_eq!(
            encode_into("€", Variant::Iso, &mut v),
            Err(Error::Unencodable('€'))
        );
        assert_eq!(
            encode_into("中", Variant::Dvb, &mut v),
            Err(Error::Unencodable('中'))
        );
        let u: Vec<Unit> = units(b"a\x86b", Variant::Dvb).collect();
        assert_eq!(u, [Unit::Char('a'), Unit::Control(0x86), Unit::Char('b')]);
    }
}
