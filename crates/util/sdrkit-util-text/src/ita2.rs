//! ITA2, the International Telegraph Alphabet No. 2 (ITU-T S.1, formerly F.1 Division C), with its figures-case variants, and the
//! modified Baudot code of Cospas-Sarsat C/S T.001.
//!
//! # Codes
//!
//! A code is the 5-unit combination as a number with bit position 1 (the first bit sent after the start element) in the least
//! significant bit and stop polarity ("Z", mark) as 1. With this numbering E is 1, LF is 2, A is 3, space is 4, CR is 8, FIGS is 27
//! and LTRS is 31. The assignment is the one reproduced in ITU-R M.625-4 Table 1, which the tests check code by code.
//!
//! # Figures case variants
//!
//! The letters case is the same everywhere. The figures case differs:
//!
//! | Letter key | [`ITU`] (ITU-T S.1) | [`US_TTY`] (US teleprinters) |
//! |---|---|---|
//! | D | "Who are you?" (WRU, returned as ENQ U+0005) | `$` |
//! | F, G, H | not assigned (reserved for national use) | `!`, `&`, `#` |
//! | J | audible signal (BEL U+0007) | `'` |
//! | S | `'` | audible signal (BEL) |
//! | V | `=` | `;` |
//! | Z | `+` | `"` |
//!
//! The ITU table leaves F, G and H to national use; [`Table::with_national`] fills them. CCIR 476 (SITOR, NAVTEX) uses the ITU
//! figures case (ITU-R M.625-4 Table 1).
//!
//! ```
//! use sdrkit_util_text::ita2::{Decoder, Encoder, ITU};
//!
//! let mut codes = Vec::new();
//! Encoder::new(&ITU).encode_str("CQ 73", &mut codes).unwrap();
//! let mut d = Decoder::new(&ITU);
//! let text: String = codes.iter().filter_map(|&c| d.push(c)).collect();
//! assert_eq!(text, "CQ 73");
//! ```

use crate::error::{Error, Result};

/// The letters shift code (combination No. 29 of ITU-R M.625 Table 1).
pub const LTRS: u8 = 31;
/// The figures shift code (combination No. 30).
pub const FIGS: u8 = 27;
/// The "no information" code (combination No. 32, all start polarity).
pub const BLANK: u8 = 0;
/// Carriage return.
pub const CR: u8 = 8;
/// Line feed.
pub const LF: u8 = 2;
/// Space.
pub const SPACE: u8 = 4;

/// "Who are you?" (figures D in the ITU table), returned as this character.
pub const WRU: char = '\u{5}';
/// The audible signal (bell), returned as this character.
pub const BELL: char = '\u{7}';

/// The letters case, by code. `None` for the blank and the two shifts.
const LETTERS: [Option<char>; 32] = {
    let s = b"\0E\nA SIU\rDRJNFCKTZLWHYPQOBG\0MXV\0";
    let mut t = [None; 32];
    let mut i = 0;
    while i < 32 {
        if s[i] != 0 {
            t[i] = Some(s[i] as char);
        }
        i += 1;
    }
    t
};

/// The figures case of a table given as one byte per code (0: none).
const fn figures(s: &[u8; 32]) -> [Option<char>; 32] {
    let mut t = [None; 32];
    let mut i = 0;
    while i < 32 {
        if s[i] != 0 {
            t[i] = Some(s[i] as char);
        }
        i += 1;
    }
    t
}

/// One ITA2 table: the letters case and a figures case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    /// The letters case, by code.
    pub letters: [Option<char>; 32],
    /// The figures case, by code.
    pub figures: [Option<char>; 32],
    /// The name of the variant.
    pub name: &'static str,
}

/// The international table of ITU-T S.1 (as reproduced in ITU-R M.625-4 Table 1): F, G and H unassigned in figures case, D is
/// "Who are you?", J the bell.
pub const ITU: Table = Table {
    letters: LETTERS,
    figures: figures(b"\x003\n- '87\r\x054\x07,\0:(5+)2\x006019?\0\0./=\0"),
    name: "ITU-T S.1",
};

/// The figures case of US teleprinters: `$` on D, `!` `&` `#` on F G H, `'` on J, the bell on S, `;` on V and `"` on Z.
pub const US_TTY: Table = Table {
    letters: LETTERS,
    figures: figures(b"\x003\n- \x0787\r$4',!:(5\")2#6019?&\0./;\0"),
    name: "US TTY",
};

impl Table {
    /// This table with the national-use figures positions F, G and H filled (ITU-T S.1 leaves them to national use).
    pub const fn with_national(
        mut self,
        f: Option<char>,
        g: Option<char>,
        h: Option<char>,
    ) -> Table {
        self.figures[13] = f;
        self.figures[26] = g;
        self.figures[20] = h;
        self
    }

    /// The character of `code` in a case (`None` for the blank, the shifts, an unassigned position or a code above 31).
    pub fn char_of(&self, code: u8, figures: bool) -> Option<char> {
        let t = if figures {
            &self.figures
        } else {
            &self.letters
        };
        t.get(code as usize).copied().flatten()
    }

    /// The code of a character and the case it needs (`None` for a character in both cases: space, CR, LF). Letters are folded to
    /// upper case.
    pub fn code_of(&self, c: char) -> Result<(u8, Option<Shift>)> {
        let c = c.to_ascii_uppercase();
        let l = self.letters.iter().position(|&x| x == Some(c));
        let f = self.figures.iter().position(|&x| x == Some(c));
        match (l, f) {
            (Some(a), Some(b)) if a == b => Ok((a as u8, None)),
            (Some(a), _) => Ok((a as u8, Some(Shift::Letters))),
            (None, Some(b)) => Ok((b as u8, Some(Shift::Figures))),
            (None, None) => Err(Error::Unencodable(c)),
        }
    }
}

/// The case a receiver is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shift {
    /// Letters case.
    #[default]
    Letters,
    /// Figures case.
    Figures,
}

/// Decodes ITA2 codes to characters, keeping the case between calls.
#[derive(Clone, Debug)]
pub struct Decoder<'t> {
    table: &'t Table,
    shift: Shift,
    /// A space returns the receiver to letters case ("unshift on space", common on amateur RTTY; off in ITU-T S.1).
    pub unshift_on_space: bool,
}

impl<'t> Decoder<'t> {
    /// A decoder in letters case.
    pub fn new(table: &'t Table) -> Self {
        Decoder {
            table,
            shift: Shift::Letters,
            unshift_on_space: false,
        }
    }

    /// The current case.
    pub fn shift(&self) -> Shift {
        self.shift
    }

    /// One code (only the low five bits are used). Returns the character, or `None` for a shift, the blank or an unassigned
    /// position. Control characters come out as CR, LF, [`BELL`] and [`WRU`].
    pub fn push(&mut self, code: u8) -> Option<char> {
        match code & 31 {
            LTRS => {
                self.shift = Shift::Letters;
                None
            }
            FIGS => {
                self.shift = Shift::Figures;
                None
            }
            c => {
                if c == SPACE && self.unshift_on_space {
                    self.shift = Shift::Letters;
                }
                self.table.char_of(c, self.shift == Shift::Figures)
            }
        }
    }

    /// Back to letters case.
    pub fn reset(&mut self) {
        self.shift = Shift::Letters;
    }
}

/// Encodes text to ITA2 codes with every case change it needs.
#[derive(Clone, Debug)]
pub struct Encoder<'t> {
    table: &'t Table,
    shift: Option<Shift>,
    /// Send a newline (`'\n'`) as CR LF (on by default).
    pub newline_as_cr_lf: bool,
    /// Repeat FIGS after a space sent in figures case, for receivers that unshift on space (off by default).
    pub refigs_after_space: bool,
}

impl<'t> Encoder<'t> {
    /// An encoder that sends LTRS before the first character.
    pub fn new(table: &'t Table) -> Self {
        Encoder {
            table,
            shift: None,
            newline_as_cr_lf: true,
            refigs_after_space: false,
        }
    }

    /// Encodes one character, calling `out` for each code. Nothing is sent for an unencodable character.
    pub fn encode_char(&mut self, c: char, out: &mut impl FnMut(u8)) -> Result<()> {
        if c == '\n' && self.newline_as_cr_lf {
            self.encode_char('\r', out)?;
            return self.encode_one('\n', out);
        }
        self.encode_one(c, out)
    }

    fn encode_one(&mut self, c: char, out: &mut impl FnMut(u8)) -> Result<()> {
        let (code, need) = self.table.code_of(c)?;
        let need = match need {
            Some(s) => s,
            // A character in both cases still needs a known case at the start.
            None => self.shift.unwrap_or(Shift::Letters),
        };
        if self.shift != Some(need) {
            out(if need == Shift::Figures { FIGS } else { LTRS });
            self.shift = Some(need);
        }
        out(code);
        if code == SPACE && self.refigs_after_space && self.shift == Some(Shift::Figures) {
            // The receiver went back to letters: say figures again before the next figure.
            self.shift = Some(Shift::Letters);
        }
        Ok(())
    }

    /// Encodes a text, appending codes to `out`. Stops at the first character the table cannot send.
    pub fn encode_str(&mut self, text: &str, out: &mut Vec<u8>) -> Result<()> {
        for c in text.chars() {
            self.encode_char(c, &mut |b| out.push(b))?;
        }
        Ok(())
    }
}

/// The modified Baudot code of Cospas-Sarsat C/S T.001 (406 MHz beacons): six bits, the first (most significant) 1 for a letter
/// and 0 for a figure, then the ITA2 code with bit position 1 first (most significant). Only letters, space, digits, `-` and `/`
/// are used.
pub mod t001 {
    use super::{Error, Result, ITU};

    /// The 6-bit code of a character (letters folded to upper case).
    pub fn encode(c: char) -> Result<u8> {
        let c = c.to_ascii_uppercase();
        let ok = c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, ' ' | '-' | '/');
        if !ok {
            return Err(Error::Unencodable(c));
        }
        let (code, shift) = ITU.code_of(c)?;
        let letter = !matches!(shift, Some(super::Shift::Figures));
        Ok(((letter as u8) << 5) | (code.reverse_bits() >> 3))
    }

    /// The character of a 6-bit code.
    pub fn decode(v: u8) -> Result<char> {
        if v > 63 {
            return Err(Error::OutOfRange);
        }
        let code = (v & 31).reverse_bits() >> 3;
        let c = ITU
            .char_of(code, v & 32 == 0)
            .ok_or(Error::UnknownCode(v as u32))?;
        let ok = c.is_ascii_uppercase() || c.is_ascii_digit() || matches!(c, ' ' | '-' | '/');
        ok.then_some(c).ok_or(Error::UnknownCode(v as u32))
    }

    /// The 5-bit code of a letter (the aircraft operator designator field, letters only).
    pub fn encode5(c: char) -> Result<u8> {
        let v = encode(c)?;
        if v & 32 == 0 || c == ' ' {
            return Err(Error::Unencodable(c));
        }
        Ok(v & 31)
    }

    /// The letter of a 5-bit code.
    pub fn decode5(v: u8) -> Result<char> {
        if v > 31 {
            return Err(Error::OutOfRange);
        }
        decode(v | 32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ITU-R M.625-4 Table 1: (letters case, ITA2 code as A/Z with bit position 1 first). Z is stop polarity (1).
    const M625_TABLE1: [(char, &str); 32] = [
        ('A', "ZZAAA"),
        ('B', "ZAAZZ"),
        ('C', "AZZZA"),
        ('D', "ZAAZA"),
        ('E', "ZAAAA"),
        ('F', "ZAZZA"),
        ('G', "AZAZZ"),
        ('H', "AAZAZ"),
        ('I', "AZZAA"),
        ('J', "ZZAZA"),
        ('K', "ZZZZA"),
        ('L', "AZAAZ"),
        ('M', "AAZZZ"),
        ('N', "AAZZA"),
        ('O', "AAAZZ"),
        ('P', "AZZAZ"),
        ('Q', "ZZZAZ"),
        ('R', "AZAZA"),
        ('S', "ZAZAA"),
        ('T', "AAAAZ"),
        ('U', "ZZZAA"),
        ('V', "AZZZZ"),
        ('W', "ZZAAZ"),
        ('X', "ZAZZZ"),
        ('Y', "ZAZAZ"),
        ('Z', "ZAAAZ"),
        ('\r', "AAAZA"),
        ('\n', "AZAAA"),
        ('<', "ZZZZZ"), // letter shift
        ('>', "ZZAZZ"), // figure shift
        (' ', "AAZAA"),
        ('\0', "AAAAA"), // no information
    ];

    fn az(s: &str) -> u8 {
        s.bytes()
            .enumerate()
            .map(|(i, b)| ((b == b'Z') as u8) << i)
            .sum()
    }

    #[test]
    fn codes_match_m625_table_1() {
        for (c, s) in M625_TABLE1 {
            let code = az(s);
            match c {
                '<' => assert_eq!(code, LTRS),
                '>' => assert_eq!(code, FIGS),
                '\0' => assert_eq!(code, BLANK),
                _ => assert_eq!(ITU.char_of(code, false), Some(c), "{c:?}"),
            }
        }
    }

    #[test]
    fn itu_figures_match_m625_table_1() {
        // Figures case of Table 1, in letter order A..Z (None: not assigned; WRU and the bell as controls).
        let fig = [
            Some('-'),
            Some('?'),
            Some(':'),
            Some(WRU),
            Some('3'),
            None,
            None,
            None,
            Some('8'),
            Some(BELL),
            Some('('),
            Some(')'),
            Some('.'),
            Some(','),
            Some('9'),
            Some('0'),
            Some('1'),
            Some('4'),
            Some('\''),
            Some('5'),
            Some('7'),
            Some('='),
            Some('2'),
            Some('/'),
            Some('6'),
            Some('+'),
        ];
        for (i, f) in fig.iter().enumerate() {
            let code = az(M625_TABLE1[i].1);
            assert_eq!(ITU.char_of(code, true), *f, "letter {}", M625_TABLE1[i].0);
        }
    }

    #[test]
    fn us_tty_differs_only_on_its_positions() {
        let mut diff = vec![];
        for code in 0..32u8 {
            if ITU.figures[code as usize] != US_TTY.figures[code as usize] {
                diff.push(ITU.char_of(code, false).unwrap());
            }
        }
        assert_eq!(diff, ['S', 'D', 'J', 'F', 'Z', 'H', 'G', 'V']);
        assert_eq!(US_TTY.char_of(9, true), Some('$'));
        assert_eq!(US_TTY.char_of(5, true), Some(BELL));
        assert_eq!(US_TTY.char_of(17, true), Some('"'));
    }

    #[test]
    fn round_trip_and_shifts() {
        for t in [&ITU, &US_TTY] {
            let text = "RYRY CQ DE K0TST 599 1/2 -.:?\nTHE QUICK BROWN FOX";
            let mut codes = vec![];
            Encoder::new(t).encode_str(text, &mut codes).unwrap();
            assert_eq!(codes[0], LTRS);
            let mut d = Decoder::new(t);
            let s: String = codes.iter().filter_map(|&c| d.push(c)).collect();
            assert_eq!(s, text.replace('\n', "\r\n"));
        }
        assert_eq!(
            Encoder::new(&ITU).encode_str("$", &mut vec![]),
            Err(Error::Unencodable('$'))
        );
    }

    #[test]
    fn unshift_on_space() {
        let codes = [FIGS, 23, SPACE, 3]; // "1", space, then A's code
        let mut d = Decoder::new(&ITU);
        let s: String = codes.iter().filter_map(|&c| d.push(c)).collect();
        assert_eq!(s, "1 -");
        let mut d = Decoder::new(&ITU);
        d.unshift_on_space = true;
        let s: String = codes.iter().filter_map(|&c| d.push(c)).collect();
        assert_eq!(s, "1 A");
        // The encoder repeats FIGS for such a receiver.
        let mut e = Encoder::new(&ITU);
        e.refigs_after_space = true;
        let mut v = vec![];
        e.encode_str("1 2", &mut v).unwrap();
        assert_eq!(v, [FIGS, 23, SPACE, FIGS, 19]);
    }

    #[test]
    fn national_positions() {
        let t = ITU.with_national(Some('Ä'), Some('Ö'), Some('Ü'));
        assert_eq!(t.char_of(13, true), Some('Ä'));
        assert_eq!(t.char_of(26, true), Some('Ö'));
        assert_eq!(t.char_of(20, true), Some('Ü'));
        assert_eq!(t.char_of(40, true), None);
    }

    #[test]
    fn t001_modified_baudot() {
        // C/S T.001 modified Baudot code table (6 bits, most significant bit first).
        let table: [(char, u8); 39] = [
            ('A', 0b111000),
            ('B', 0b110011),
            ('C', 0b101110),
            ('D', 0b110010),
            ('E', 0b110000),
            ('F', 0b110110),
            ('G', 0b101011),
            ('H', 0b100101),
            ('I', 0b101100),
            ('J', 0b111010),
            ('K', 0b111110),
            ('L', 0b101001),
            ('M', 0b100111),
            ('N', 0b100110),
            ('O', 0b100011),
            ('P', 0b101101),
            ('Q', 0b111101),
            ('R', 0b101010),
            ('S', 0b110100),
            ('T', 0b100001),
            ('U', 0b111100),
            ('V', 0b101111),
            ('W', 0b111001),
            ('X', 0b110111),
            ('Y', 0b110101),
            ('Z', 0b110001),
            (' ', 0b100100),
            ('-', 0b011000),
            ('/', 0b010111),
            ('0', 0b001101),
            ('1', 0b011101),
            ('2', 0b011001),
            ('3', 0b010000),
            ('4', 0b001010),
            ('5', 0b000001),
            ('6', 0b010101),
            ('7', 0b011100),
            ('8', 0b001100),
            ('9', 0b000011),
        ];
        for (c, v) in table {
            assert_eq!(t001::encode(c), Ok(v), "{c}");
            assert_eq!(t001::decode(v), Ok(c), "{c}");
        }
        assert_eq!(t001::encode('?'), Err(Error::Unencodable('?')));
        assert_eq!(t001::decode(0b011010), Err(Error::UnknownCode(0b011010))); // figures J: the bell
        assert_eq!(t001::encode5('A'), Ok(0b11000));
        assert_eq!(t001::decode5(0b11000), Ok('A'));
        assert!(t001::encode5('1').is_err());
    }
}
