//! IA5, the International Alphabet No. 5 (ITU-T T.50; its international version, IRV, is US-ASCII), with the parity
//! conventions of the links that carry it, the filters that make received bytes printable, and the ACARS character rules.
//!
//! ```
//! use sdrkit_util_text::ia5::{self, Parity};
//!
//! assert_eq!(ia5::add_parity(b'A', Parity::Odd), 0xC1);
//! assert_eq!(ia5::check_parity(0xC1, Parity::Odd), Ok(b'A'));
//! let mut s = String::new();
//! ia5::printable_into(b"OK\r\n\x01\x7fGO", &ia5::Printable::default(), &mut s);
//! assert_eq!(s, "OK\n..GO");
//! ```

use crate::error::{Error, Result};

/// The parity bit in bit 7.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Parity {
    /// No parity: bit 7 is ignored on receive and sent as 0.
    #[default]
    None,
    /// Odd parity over all eight bits (ACARS, teletext, GOES).
    Odd,
    /// Even parity over all eight bits.
    Even,
    /// Bit 7 always 1.
    Mark,
    /// Bit 7 always 0.
    Space,
}

/// A 7-bit character with its parity bit set.
pub fn add_parity(c: u8, p: Parity) -> u8 {
    let c = c & 0x7F;
    let ones = c.count_ones();
    let bit = match p {
        Parity::None | Parity::Space => false,
        Parity::Mark => true,
        Parity::Odd => ones.is_multiple_of(2),
        Parity::Even => ones % 2 == 1,
    };
    c | (bit as u8) << 7
}

/// The 7-bit character of a received byte, or [`Error::Parity`] when its parity bit is wrong.
pub fn check_parity(b: u8, p: Parity) -> Result<u8> {
    if add_parity(b, p) == b || p == Parity::None {
        Ok(b & 0x7F)
    } else {
        Err(Error::Parity)
    }
}

/// How received bytes are made printable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Printable {
    /// The parity to remove first.
    pub parity: Parity,
    /// What a byte with a parity error, a control character or DEL becomes (`None`: dropped).
    pub replacement: Option<char>,
    /// CR LF, a lone CR and a lone LF become one line break (otherwise CR and LF are control characters too).
    pub line_breaks: bool,
    /// Tab is kept.
    pub tabs: bool,
    /// Stop at the first ETX (0x03).
    pub stop_at_etx: bool,
    /// At most this many characters are written.
    pub max_chars: usize,
}

impl Default for Printable {
    fn default() -> Self {
        Printable {
            parity: Parity::None,
            replacement: Some('.'),
            line_breaks: true,
            tabs: false,
            stop_at_etx: false,
            max_chars: usize::MAX,
        }
    }
}

/// Appends the printable reading of `bytes` to `out`. Returns the number of characters written.
pub fn printable_into(bytes: &[u8], opt: &Printable, out: &mut String) -> usize {
    let mut n = 0;
    let mut after_cr = false;
    for &b in bytes {
        if n >= opt.max_chars {
            break;
        }
        let c = match check_parity(b, opt.parity) {
            Ok(c) => c,
            Err(_) => {
                after_cr = false;
                if let Some(r) = opt.replacement {
                    out.push(r);
                    n += 1;
                }
                continue;
            }
        };
        if c == 0x03 && opt.stop_at_etx {
            break;
        }
        let ch = match c {
            b'\r' if opt.line_breaks => {
                after_cr = true;
                Some('\n')
            }
            b'\n' if opt.line_breaks => {
                if std::mem::take(&mut after_cr) {
                    continue;
                }
                Some('\n')
            }
            b'\t' if opt.tabs => Some('\t'),
            0x20..=0x7E => Some(c as char),
            _ => opt.replacement,
        };
        if c != b'\r' {
            after_cr = false;
        }
        if let Some(ch) = ch {
            out.push(ch);
            n += 1;
        }
    }
    n
}

/// Whether a byte is printable IA5 (0x20 to 0x7E).
pub fn is_printable(b: u8) -> bool {
    (0x20..=0x7E).contains(&b)
}

/// The German reference version of ISO 646 (DIN 66003, ISO-IR 21): the character that a 7-bit code shown as `c` in IA5 stands
/// for. `@ [ \ ] { | } ~` become `§ Ä Ö Ü ä ö ü ß`. Other characters stay as they are.
///
/// ```
/// assert_eq!(sdrkit_util_text::ia5::din66003('}'), 'ü');
/// ```
pub const fn din66003(c: char) -> char {
    match c {
        '@' => '§',
        '[' => 'Ä',
        '\\' => 'Ö',
        ']' => 'Ü',
        '{' => 'ä',
        '|' => 'ö',
        '}' => 'ü',
        '~' => 'ß',
        c => c,
    }
}

/// Appends `text` to `out` with every character mapped by [`din66003`].
pub fn din66003_into(text: &str, out: &mut String) {
    out.extend(text.chars().map(din66003));
}

/// The ACARS character rules (ARINC 618): IA5 with odd parity.
pub mod acars {
    use super::Parity;

    /// Start of heading, as the 7-bit character.
    pub const SOH: u8 = 0x01;
    /// Start of text.
    pub const STX: u8 = 0x02;
    /// End of text (the last block of a message).
    pub const ETX: u8 = 0x03;
    /// End of transmission block (more blocks follow).
    pub const ETB: u8 = 0x17;
    /// DEL, the block suffix.
    pub const DEL: u8 = 0x7F;
    /// The parity ACARS uses.
    pub const PARITY: Parity = Parity::Odd;

    /// A 7-bit character as sent on the air (with odd parity): ETX becomes 0x83 and ETB 0x97.
    pub const fn sent(c: u8) -> u8 {
        let c = c & 0x7F;
        if c.count_ones().is_multiple_of(2) {
            c | 0x80
        } else {
            c
        }
    }

    /// The printing rules of an ACARS message text: odd parity removed, CR LF as a line break, other control characters and
    /// parity errors as `.`.
    pub fn printable() -> super::Printable {
        super::Printable {
            parity: PARITY,
            ..super::Printable::default()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::add_parity;
        use super::*;

        #[test]
        fn sent_values() {
            assert_eq!(sent(ETX), 0x83);
            assert_eq!(sent(ETB), 0x97);
            assert_eq!(sent(SOH), 0x01);
            assert_eq!(sent(STX), 0x02);
            assert_eq!(sent(DEL), 0x7F);
            assert_eq!(sent(b'+'), add_parity(b'+', Parity::Odd));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_kinds() {
        // 'A' = 0x41 has two ones.
        assert_eq!(add_parity(b'A', Parity::Odd), 0xC1);
        assert_eq!(add_parity(b'A', Parity::Even), 0x41);
        assert_eq!(add_parity(b'C', Parity::Even), 0xC3);
        assert_eq!(add_parity(b'A', Parity::Mark), 0xC1);
        assert_eq!(add_parity(0xC1, Parity::Space), 0x41);
        for p in [Parity::Odd, Parity::Even, Parity::Mark, Parity::Space] {
            for c in 0..128u8 {
                assert_eq!(check_parity(add_parity(c, p), p), Ok(c));
                if p == Parity::Odd || p == Parity::Even {
                    assert_eq!(check_parity(add_parity(c, p) ^ 1, p), Err(Error::Parity));
                }
            }
        }
        assert_eq!(check_parity(0xFF, Parity::None), Ok(0x7F));
    }

    #[test]
    fn printable_rules() {
        let mut s = String::new();
        let opt = Printable {
            parity: Parity::Odd,
            stop_at_etx: true,
            ..Printable::default()
        };
        let bytes: Vec<u8> = b"AB\r\nC\rD\nE\x07F\x03G"
            .iter()
            .map(|&c| add_parity(c, Parity::Odd))
            .collect();
        printable_into(&bytes, &opt, &mut s);
        assert_eq!(s, "AB\nC\nD\nE.F");
        let mut s = String::new();
        let mut bad = bytes.clone();
        bad[0] ^= 0x80;
        printable_into(
            &bad,
            &Printable {
                max_chars: 3,
                ..opt
            },
            &mut s,
        );
        assert_eq!(s, ".B\n");
        let mut s = String::new();
        printable_into(
            b"a\tb\x01",
            &Printable {
                tabs: true,
                replacement: None,
                ..Printable::default()
            },
            &mut s,
        );
        assert_eq!(s, "a\tb");
    }

    /// DIN 66003 (ISO-IR 21) positions 0x40, 0x5B to 0x5D and 0x7B to 0x7E, and one IRV position that does not change.
    #[test]
    fn din66003_positions() {
        let irv: Vec<char> = [0x40u8, 0x5B, 0x5C, 0x5D, 0x7B, 0x7C, 0x7D, 0x7E, 0x41]
            .iter()
            .map(|&b| b as char)
            .collect();
        let de: String = irv.iter().map(|&c| din66003(c)).collect();
        assert_eq!(de, "§ÄÖÜäöüßA");
        let mut s = String::new();
        din66003_into("Gr}~e", &mut s);
        assert_eq!(s, "Grüße");
    }
}
