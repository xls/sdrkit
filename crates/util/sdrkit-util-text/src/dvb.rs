//! DVB SI text (ETSI EN 300 468 Annex A): the character table chosen by the first bytes of a text field, the control codes
//! (emphasis on and off, CR/LF), and the clean-up every service name, event title and description gets.
//!
//! | First byte | Table |
//! |---|---|
//! | 0x20 to 0xFF | table 00: ISO/IEC 6937 with the euro sign ([`crate::iso6937`]), or GB 2312 by option |
//! | 0x01 to 0x07, 0x09 to 0x0B | ISO/IEC 8859-5 to -11, -13 to -15 (Table A.3) |
//! | 0x10, N (16 bits) | ISO/IEC 8859-N (Table A.4) |
//! | 0x11 | ISO/IEC 10646 BMP, two bytes per character |
//! | 0x12 | KS X 1001 (Korean) |
//! | 0x13 | GB 2312 (simplified Chinese) |
//! | 0x14 | Big5 (traditional Chinese) |
//! | 0x15 | UTF-8 |
//! | 0x1F, id | encoding_type_id of TS 101 162 (not supported: read as table 00, [`DvbText::unsupported`] set) |
//! | 0x08, 0x0C to 0x0F, 0x16 to 0x1E | reserved (read as table 00, [`DvbText::unsupported`] set) |
//!
//! Control codes (Tables A.1 and A.2): 0x86 / 0x87 (U+E086 / U+E087 in two-byte tables, U+0086 / U+0087 in UTF-8) turn emphasis
//! on and off, and 0x8A (U+E08A, U+008A) is CR/LF. The other codes in 0x80 to 0x9F are reserved or user defined and are dropped.
//!
//! The decoder writes into a [`DvbText`] the caller keeps, so decoding allocates nothing once its buffers have grown.
//!
//! ```
//! use sdrkit_util_text::dvb::{DvbText, Options, Usage};
//!
//! let mut t = DvbText::default();
//! t.decode(b"\x86BBC\x87 One", Usage::Name, &Options::default());
//! assert_eq!(t.text, "BBC One");
//! assert_eq!(t.emphasis, [(0, 3)]);
//! ```

use crate::iso6937::{self, Unit, Variant};

/// Where a text is shown, which decides what CR/LF becomes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Usage {
    /// A name or a title: CR/LF becomes a space.
    #[default]
    Name,
    /// A description: CR/LF becomes a line break.
    Description,
}

/// The table for text with no selector bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DefaultTable {
    /// Table 00 (ISO/IEC 6937 with the euro sign), as the standard says.
    #[default]
    Iso6937,
    /// GB 2312 (read with the GBK superset) for every unprefixed text, as some broadcasters in China send.
    Gbk,
    /// Table 00, but unprefixed text whose bytes above 0x7F all stand in GBK pairs of two such bytes (at least one pair) is
    /// read as GBK: networks in China send GB 2312 names with no selector, and table 00 text has no such pairs.
    GbkWhenValid,
}

/// Decoding options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// The table for unprefixed text.
    pub default_table: DefaultTable,
}

/// A decoded text: clean UTF-8 and the emphasised ranges.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DvbText {
    /// The text: controls removed, invisible characters removed, odd spaces made plain, runs of spaces collapsed, trimmed.
    pub text: String,
    /// Emphasised ranges `[start, end)`, in characters of `text`.
    pub emphasis: Vec<(usize, usize)>,
    /// The text used a reserved or unsupported selector and was read as table 00.
    pub unsupported: bool,
}

#[derive(Clone, Copy)]
enum Tok {
    Char(char),
    EmphOn,
    EmphOff,
    Break,
}

/// Collects tokens into the clean text.
struct Cleaner<'a> {
    out: &'a mut DvbText,
    usage: Usage,
    n: usize,
    last_space: bool,
    emph: Option<usize>,
}

impl Cleaner<'_> {
    fn push(&mut self, t: Tok) {
        let c = match t {
            Tok::EmphOn => {
                self.emph = Some(self.n);
                return;
            }
            Tok::EmphOff => {
                if let Some(s) = self.emph.take() {
                    if self.n > s {
                        self.out.emphasis.push((s, self.n));
                    }
                }
                return;
            }
            Tok::Break => {
                if self.usage == Usage::Description {
                    '\n'
                } else {
                    ' '
                }
            }
            Tok::Char(c) => match c {
                '\t' | '\r' | '\n' => ' ',
                '\0'..='\u{1F}' | '\u{7F}'..='\u{9F}' => return,
                '\u{AD}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}' => return,
                '\u{FEFF}' | '\u{FFFD}' | '\u{FDD0}'..='\u{FDEF}' | '\u{E080}'..='\u{E09F}' => {
                    return
                }
                c if (c as u32) & 0xFFFE == 0xFFFE => return,
                '\u{A0}' | '\u{2028}' | '\u{2029}' | '\u{3000}' => ' ',
                c => c,
            },
        };
        if c == ' ' {
            if self.last_space {
                return;
            }
            self.last_space = true;
        } else if c == '\n' {
            // A break eats the space before it.
            if self.out.text.ends_with(' ') {
                self.out.text.pop();
                self.n -= 1;
            }
            self.last_space = true;
        } else {
            self.last_space = false;
        }
        self.out.text.push(c);
        self.n += 1;
    }

    fn finish(mut self) {
        if let Some(s) = self.emph.take() {
            if self.n > s {
                self.out.emphasis.push((s, self.n));
            }
        }
        while self.out.text.ends_with(' ') || self.out.text.ends_with('\n') {
            self.out.text.pop();
            self.n -= 1;
        }
        let n = self.n;
        for e in self.out.emphasis.iter_mut() {
            e.1 = e.1.min(n);
        }
        self.out.emphasis.retain(|e| e.1 > e.0);
    }
}

/// The token of a character from a multi-byte table (where the controls are private-use or C1 code points).
fn tok_of(c: char) -> Tok {
    match c {
        '\u{E086}' | '\u{86}' => Tok::EmphOn,
        '\u{E087}' | '\u{87}' => Tok::EmphOff,
        '\u{E08A}' | '\u{8A}' => Tok::Break,
        c => Tok::Char(c),
    }
}

/// The token of a control byte of a one-byte table (`None`: dropped).
fn control(b: u8) -> Option<Tok> {
    match b {
        0x86 => Some(Tok::EmphOn),
        0x87 => Some(Tok::EmphOff),
        0x8A => Some(Tok::Break),
        _ => None,
    }
}

/// The encoding_rs encoding of ISO/IEC 8859 part `n` for bytes 0xA0 to 0xFF (0x80 to 0x9F never reach it). Parts 9 and 11 use
/// the Windows tables, which agree with ISO in that range; part 1 is read directly.
fn iso8859(n: u16) -> Option<&'static encoding_rs::Encoding> {
    use encoding_rs::*;
    Some(match n {
        2 => ISO_8859_2,
        3 => ISO_8859_3,
        4 => ISO_8859_4,
        5 => ISO_8859_5,
        6 => ISO_8859_6,
        7 => ISO_8859_7,
        8 => ISO_8859_8,
        9 => WINDOWS_1254,
        10 => ISO_8859_10,
        11 => WINDOWS_874,
        13 => ISO_8859_13,
        14 => ISO_8859_14,
        15 => ISO_8859_15,
        16 => ISO_8859_16,
        _ => return None,
    })
}

/// Decodes `bytes` with a multi-byte encoding through a small stack buffer, feeding the cleaner.
fn multibyte(enc: &'static encoding_rs::Encoding, bytes: &[u8], c: &mut Cleaner) {
    let mut dec = enc.new_decoder_without_bom_handling();
    let mut buf = [0u8; 64];
    let mut rest = bytes;
    loop {
        let (res, read, written, _) = dec.decode_to_utf8(rest, &mut buf, true);
        if let Ok(s) = std::str::from_utf8(&buf[..written]) {
            for ch in s.chars() {
                c.push(tok_of(ch));
            }
        }
        rest = &rest[read..];
        if res == encoding_rs::CoderResult::InputEmpty {
            break;
        }
    }
}

fn single_byte(n: u16, bytes: &[u8], c: &mut Cleaner) {
    let enc = iso8859(n);
    let mut dec = enc.map(|e| e.new_decoder_without_bom_handling());
    for &b in bytes {
        if (0x80..=0x9F).contains(&b) {
            if let Some(t) = control(b) {
                c.push(t);
            }
            continue;
        }
        if b < 0x80 || n == 1 {
            c.push(Tok::Char(b as char));
            continue;
        }
        if let Some(d) = dec.as_mut() {
            let mut buf = [0u8; 8];
            let (_, _, w, bad) = d.decode_to_utf8(&[b], &mut buf, false);
            if !bad {
                if let Some(ch) = std::str::from_utf8(&buf[..w])
                    .ok()
                    .and_then(|s| s.chars().next())
                {
                    c.push(Tok::Char(ch));
                }
            }
        }
    }
}

fn table00(bytes: &[u8], c: &mut Cleaner) {
    for u in iso6937::units(bytes, Variant::Dvb) {
        match u {
            Unit::Char(ch) => c.push(Tok::Char(ch)),
            Unit::Combined(a, m) => {
                c.push(Tok::Char(a));
                c.push(Tok::Char(m));
            }
            // C0 codes go to the cleaner (tab and line ends become spaces, the rest are dropped).
            Unit::Control(b) if b < 0x20 => c.push(Tok::Char(b as char)),
            Unit::Control(b) => {
                if let Some(t) = control(b) {
                    c.push(t);
                }
            }
        }
    }
}

fn ucs2(bytes: &[u8], c: &mut Cleaner) {
    let units = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_be_bytes([p[0], p[1]]));
    // A lone surrogate is dropped.
    for ch in char::decode_utf16(units).flatten() {
        c.push(tok_of(ch));
    }
}

fn utf8(bytes: &[u8], c: &mut Cleaner) {
    let mut rest = bytes;
    // Invalid sequences are dropped, valid ones kept.
    while !rest.is_empty() {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                s.chars().for_each(|ch| c.push(tok_of(ch)));
                break;
            }
            Err(e) => {
                let good = e.valid_up_to();
                if let Ok(s) = std::str::from_utf8(&rest[..good]) {
                    s.chars().for_each(|ch| c.push(tok_of(ch)));
                }
                let skip = e.error_len().unwrap_or(rest.len() - good);
                rest = &rest[good + skip..];
            }
        }
    }
}

/// Whether the bytes above 0x7F all stand in GBK pairs of two such bytes (lead 0x81 to 0xFE, trail 0x80 to 0xFE), with at
/// least one pair. GB 2312 text is made of such pairs. Table 00 writes an accented letter as a mark and an ASCII letter and
/// its other characters above 0x7F stand alone, so its text does not pass. A GBK pair with a trail byte below 0x80 does not
/// pass either: the same two bytes are a table 00 character and an ASCII letter.
fn all_gbk_pairs(b: &[u8]) -> bool {
    let (mut i, mut pairs) = (0, 0);
    while i < b.len() {
        if b[i] < 0x80 {
            i += 1;
            continue;
        }
        let pair = (0x81..=0xFE).contains(&b[i])
            && b.get(i + 1).is_some_and(|t| (0x80..=0xFE).contains(t));
        if !pair {
            return false;
        }
        pairs += 1;
        i += 2;
    }
    pairs > 0
}

impl DvbText {
    /// Decodes a DVB text field, replacing what this value held.
    pub fn decode(&mut self, bytes: &[u8], usage: Usage, opt: &Options) {
        self.text.clear();
        self.emphasis.clear();
        let mut sel_unsupported = false;
        let mut c = Cleaner {
            out: self,
            usage,
            n: 0,
            last_space: true,
            emph: None,
        };
        let Some(&b0) = bytes.first() else {
            c.finish();
            self.unsupported = false;
            return;
        };
        match b0 {
            0x01..=0x07 => single_byte(b0 as u16 + 4, &bytes[1..], &mut c),
            0x09..=0x0B => single_byte(b0 as u16 + 4, &bytes[1..], &mut c),
            0x10 => match bytes.get(1..3) {
                Some(&[0, n]) if matches!(n, 1..=11 | 13..=15) => {
                    single_byte(n as u16, &bytes[3..], &mut c)
                }
                Some(_) => {
                    sel_unsupported = true;
                    table00(&bytes[3..], &mut c)
                }
                None => sel_unsupported = true,
            },
            0x11 => ucs2(&bytes[1..], &mut c),
            0x12 => multibyte(encoding_rs::EUC_KR, &bytes[1..], &mut c),
            0x13 => multibyte(encoding_rs::GB18030, &bytes[1..], &mut c),
            0x14 => multibyte(encoding_rs::BIG5, &bytes[1..], &mut c),
            0x15 => utf8(&bytes[1..], &mut c),
            0x1F => {
                sel_unsupported = true;
                table00(bytes.get(2..).unwrap_or(&[]), &mut c)
            }
            0x00 | 0x08 | 0x0C..=0x0F | 0x16..=0x1E => {
                sel_unsupported = true;
                table00(&bytes[1..], &mut c)
            }
            _ => match opt.default_table {
                DefaultTable::Iso6937 => table00(bytes, &mut c),
                DefaultTable::Gbk => multibyte(encoding_rs::GB18030, bytes, &mut c),
                DefaultTable::GbkWhenValid if all_gbk_pairs(bytes) => {
                    multibyte(encoding_rs::GB18030, bytes, &mut c)
                }
                DefaultTable::GbkWhenValid => table00(bytes, &mut c),
            },
        }
        c.finish();
        self.unsupported = sel_unsupported;
    }
}

/// Appends a text as a DVB UTF-8 field (selector 0x15), for generators.
pub fn encode_utf8(text: &str, out: &mut Vec<u8>) {
    out.push(0x15);
    out.extend_from_slice(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(b: &[u8]) -> String {
        let mut t = DvbText::default();
        t.decode(b, Usage::Name, &Options::default());
        t.text
    }

    #[test]
    fn tables_by_selector() {
        assert_eq!(d(b"Pr\xC8ufbild"), "Prüfbild");
        assert_eq!(d(b"5 \xA4"), "5 €");
        // ISO 8859-5 "Привет".
        assert_eq!(d(&[0x01, 0xBF, 0xE0, 0xD8, 0xD2, 0xD5, 0xE2]), "Привет");
        // ISO 8859-6 "سلام".
        assert_eq!(d(&[0x02, 0xD3, 0xE4, 0xC7, 0xE5]), "سلام");
        // ISO 8859-7 "Ελλάδα".
        assert_eq!(d(&[0x03, 0xC5, 0xEB, 0xEB, 0xDC, 0xE4, 0xE1]), "Ελλάδα");
        // ISO 8859-9 "İstanbul" via 0x05, and 8859-15 euro via 0x0B.
        assert_eq!(d(&[0x05, 0xDD, b's', b't']), "İst");
        assert_eq!(d(&[0x0B, 0xA4]), "€");
        // 0x10 0x00 0x02: ISO 8859-2 "Łódź"; 0x10 0x00 0x01: Latin-1.
        assert_eq!(d(&[0x10, 0x00, 0x02, 0xA3, 0xF3, 0x64, 0xBC]), "Łódź");
        assert_eq!(d(&[0x10, 0x00, 0x01, 0xE9]), "é");
        // UCS-2, UTF-8, GB 2312, KS X 1001, Big5.
        assert_eq!(d(&[0x11, 0x00, 0x41, 0x4E, 0x2D]), "A中");
        let mut u = vec![];
        encode_utf8("Nachrichten ü", &mut u);
        assert_eq!(d(&u), "Nachrichten ü");
        assert_eq!(d(&[0x13, 0xD6, 0xD0, 0xCE, 0xC4]), "中文");
        assert_eq!(d(&[0x12, 0xC7, 0xD1, 0xB1, 0xDB]), "한글");
        assert_eq!(d(&[0x14, 0xA4, 0xA4, 0xA4, 0xE5]), "中文");
        // Reserved selectors.
        let mut t = DvbText::default();
        t.decode(&[0x08, b'x'], Usage::Name, &Options::default());
        assert!(t.unsupported && t.text == "x");
        t.decode(&[0x10, 0x00], Usage::Name, &Options::default());
        assert!(t.unsupported && t.text.is_empty());
        t.decode(b"", Usage::Name, &Options::default());
        assert!(!t.unsupported && t.text.is_empty());
    }

    #[test]
    fn gbk_options() {
        let raw = [0xD6, 0xD0, 0xCE, 0xC4, b'1'];
        let mut t = DvbText::default();
        t.decode(
            &raw,
            Usage::Name,
            &Options {
                default_table: DefaultTable::GbkWhenValid,
            },
        );
        assert_eq!(t.text, "中文1");
        t.decode(
            &[b'A', 0xA3],
            Usage::Name,
            &Options {
                default_table: DefaultTable::GbkWhenValid,
            },
        );
        assert_eq!(t.text, "A£");
        // Table 00 text stays table 00: an acute accent and "e", a sharp s before a letter, an o with stroke before a letter.
        for (raw, want) in [
            (&b"Caf\xC2e"[..], "Café"),
            (&b"Fu\xFBball"[..], "Fußball"),
            (&b"S\xF9ren"[..], "Søren"),
        ] {
            t.decode(
                raw,
                Usage::Name,
                &Options {
                    default_table: DefaultTable::GbkWhenValid,
                },
            );
            assert_eq!(t.text, want);
        }
        // A name of ASCII and GB 2312 pairs.
        t.decode(
            b"TV-6\xB5\xE7\xD3\xB0",
            Usage::Name,
            &Options {
                default_table: DefaultTable::GbkWhenValid,
            },
        );
        assert_eq!(t.text, "TV-6电影");
        t.decode(
            &[0x81, 0x40],
            Usage::Name,
            &Options {
                default_table: DefaultTable::Gbk,
            },
        );
        assert_eq!(t.text, "丂");
    }

    #[test]
    fn controls_and_cleanup() {
        let mut t = DvbText::default();
        t.decode(
            b"line one\x8Aline two",
            Usage::Description,
            &Options::default(),
        );
        assert_eq!(t.text, "line one\nline two");
        assert_eq!(d(b"line one\x8Aline two"), "line one line two");
        assert_eq!(d(b"  a\t\tb \x01c  "), "a b c");
        // Two-byte controls in UCS-2.
        assert_eq!(
            d(&[0x11, 0xE0, 0x86, 0x00, 0x58, 0xE0, 0x87, 0xE0, 0x8A, 0x00, 0x59]),
            "X Y"
        );
        // UTF-8 carries the controls as U+0086, U+0087 and U+008A (Table A.1, second column).
        t.decode(
            b"\x15\xC2\x86News\xC2\x87\xC2\x8Anext",
            Usage::Description,
            &Options::default(),
        );
        assert_eq!(t.text, "News\nnext");
        assert_eq!(t.emphasis, [(0, 4)]);
        let mut u = vec![];
        encode_utf8("a\u{200B}b\u{FEFF}\u{A0}\u{A0}c\u{AD}", &mut u);
        assert_eq!(d(&u), "ab c");
        // Invalid UTF-8 is dropped.
        assert_eq!(d(b"\x15a\xFFb"), "ab");
        // Emphasis left open runs to the end.
        t.decode(b"x \x86yz", Usage::Name, &Options::default());
        assert_eq!(t.emphasis, [(2, 4)]);
    }
}
