//! The 4-bit character set field of DAB and DRM text: FIG 1 and FIG 2 labels, the dynamic label, the MOT ContentName and
//! Journaline. It turns the bytes of a text into a `String` by the code the field carries, and back for generators.
//!
//! Standard: ETSI TS 101 756 clause 5.2, Table 1 (codes) and Annex C, Table C.2 (the EBU Latin repertoire, [`crate::ebu::DAB`]).

use crate::ebu;

/// A character set code of ETSI TS 101 756 Table 1.
///
/// | Code | Variant | Bytes |
/// |---|---|---|
/// | 0b0000 | [`Charset::EbuLatin`] | the complete EBU Latin based repertoire, TS 101 756 Annex C, Table C.2 |
/// | 0b0100 | [`Charset::Latin1`] | ISO/IEC 8859-1 |
/// | 0b0110 | [`Charset::Ucs2`] | ISO/IEC 10646 in UCS-2, big endian |
/// | 0b1111 | [`Charset::Utf8`] | ISO/IEC 10646 in UTF-8 |
///
/// The other codes are reserved or "not to be used". Their bytes are read as EBU Latin, the default, and [`Charset::Other`]
/// keeps the code so the caller can tell.
///
/// ```
/// use sdrkit_util_text::charset_field::Charset;
///
/// // FIG 1, the dynamic label prefix and the MOT ContentName carry the code in bits b7 to b4 of a byte.
/// assert_eq!(Charset::from_high_nibble(0xF0), Charset::Utf8);
/// assert_eq!(Charset::from_code(6), Charset::Ucs2);
/// assert_eq!(Charset::Ucs2.code(), 6);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Charset {
    /// 0b0000: the complete EBU Latin based repertoire (TS 101 756 Annex C).
    #[default]
    EbuLatin,
    /// 0b0100: ISO/IEC 8859-1.
    Latin1,
    /// 0b0110: UCS-2, big endian.
    Ucs2,
    /// 0b1111: UTF-8.
    Utf8,
    /// Any other (reserved) code. Its bytes are read as EBU Latin.
    Other(u8),
}

impl Charset {
    /// The character set of a 4-bit code. The bits above the low four are not used.
    pub const fn from_code(code: u8) -> Charset {
        match code & 15 {
            0 => Charset::EbuLatin,
            4 => Charset::Latin1,
            6 => Charset::Ucs2,
            15 => Charset::Utf8,
            c => Charset::Other(c),
        }
    }

    /// The character set of the field in bits b7 to b4 of `byte`. FIG 1 (EN 300 401), the first dynamic label segment (EN 300 401
    /// clause 7.4.5.2) and the MOT ContentName parameter (EN 301 234) carry the field there.
    pub const fn from_high_nibble(byte: u8) -> Charset {
        Charset::from_code(byte >> 4)
    }

    /// The 4-bit code.
    pub const fn code(self) -> u8 {
        match self {
            Charset::EbuLatin => 0,
            Charset::Latin1 => 4,
            Charset::Ucs2 => 6,
            Charset::Utf8 => 15,
            Charset::Other(c) => c & 15,
        }
    }
}

/// Appends the characters of `bytes` in `charset` to `out`, as sent.
///
/// The EBU Latin control codes are kept: 0x0A (preferred line break) is decoded as `'\n'`, 0x0B (end of headline) as `'\u{B}'`
/// and 0x1F (preferred word break) as the soft hyphen `'\u{AD}'`. A byte without a character is dropped. Bad UTF-8 and a lone
/// UCS-2 surrogate are decoded as U+FFFD. An odd last UCS-2 byte is dropped.
///
/// ```
/// use sdrkit_util_text::charset_field::{decode_into, Charset};
///
/// let mut s = String::new();
/// decode_into(&[0x24, b'o', b'd', b'z'], Charset::EbuLatin, &mut s);
/// assert_eq!(s, "łodz");
/// ```
pub fn decode_into(bytes: &[u8], charset: Charset, out: &mut String) {
    match charset {
        Charset::EbuLatin | Charset::Other(_) => ebu::DAB.decode_into(bytes, out),
        Charset::Latin1 => out.extend(bytes.iter().map(|&b| b as char)),
        Charset::Ucs2 => out.extend(bytes.as_chunks::<2>().0.iter().map(|c| {
            char::from_u32(u16::from_be_bytes([c[0], c[1]]) as u32).unwrap_or('\u{FFFD}')
        })),
        Charset::Utf8 => {
            for chunk in bytes.utf8_chunks() {
                out.push_str(chunk.valid());
                if !chunk.invalid().is_empty() {
                    out.push('\u{FFFD}');
                }
            }
        }
    }
}

/// Appends the text of `bytes` in `charset` for display.
///
/// It decodes as [`decode_into`] does, then keeps `'\n'`, drops every other control character, the soft hyphen and U+FFFD, and
/// removes the white space at the end of what this call added.
///
/// ```
/// use sdrkit_util_text::charset_field::{display_into, Charset};
///
/// let mut s = String::new();
/// display_into(&[0x00, 0x41, 0x00, 0xE9], Charset::Ucs2, &mut s);
/// assert_eq!(s, "Aé");
/// ```
pub fn display_into(bytes: &[u8], charset: Charset, out: &mut String) {
    let start = out.len();
    decode_into(bytes, charset, out);
    clean_for_display(out, start);
}

/// Appends the display text of `bytes` whose 4-bit character set field is `code` (see [`display_into`] and
/// [`Charset::from_code`]).
///
/// ```
/// use sdrkit_util_text::charset_field::display_field_into;
///
/// let mut s = String::new();
/// display_field_into(0b1111, "Café ".as_bytes(), &mut s);
/// assert_eq!(s, "Café");
/// ```
pub fn display_field_into(code: u8, bytes: &[u8], out: &mut String) {
    display_into(bytes, Charset::from_code(code), out);
}

/// Cleans `out[start..]` for display in place: keeps `'\n'`, drops the other control characters, the soft hyphen and U+FFFD,
/// then removes the spaces, tabs and line breaks at the end. A `start` past the end or inside a character changes nothing. It
/// allocates nothing.
pub fn clean_for_display(out: &mut String, start: usize) {
    if !out.is_char_boundary(start) {
        return;
    }
    let mut at = 0;
    out.retain(|c| {
        let before = at;
        at += c.len_utf8();
        before < start || c == '\n' || !(c.is_control() || c == '\u{AD}' || c == '\u{FFFD}')
    });
    let kept = out[start..].trim_end_matches([' ', '\n', '\t']).len();
    out.truncate(start + kept);
}

/// Appends the bytes of `text` in `charset`, for generators. A character the set lacks is written as `?` in EBU Latin and
/// ISO 8859-1. A character beyond the basic plane is written as U+FFFD in UCS-2.
///
/// ```
/// use sdrkit_util_text::charset_field::{encode_into, Charset};
///
/// let mut b = Vec::new();
/// encode_into("Łódź", Charset::EbuLatin, &mut b);
/// assert_eq!(b, [0x5E, 0x86, 0x64, 0xFD]);
/// ```
pub fn encode_into(text: &str, charset: Charset, out: &mut Vec<u8>) {
    match charset {
        Charset::EbuLatin | Charset::Other(_) => {
            out.extend(text.chars().map(|c| ebu::DAB.byte_of(c).unwrap_or(b'?')))
        }
        Charset::Latin1 => {
            out.extend(
                text.chars()
                    .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }),
            )
        }
        Charset::Ucs2 => {
            for c in text.chars() {
                let u = if (c as u32) < 0x10000 {
                    c as u32 as u16
                } else {
                    0xFFFD
                };
                out.extend_from_slice(&u.to_be_bytes());
            }
        }
        Charset::Utf8 => out.extend_from_slice(text.as_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_of_ts_101_756_table_1() {
        for (c, cs) in [
            (0b0000, Charset::EbuLatin),
            (0b0100, Charset::Latin1),
            (0b0110, Charset::Ucs2),
            (0b1111, Charset::Utf8),
            (0b0001, Charset::Other(1)),
        ] {
            assert_eq!(Charset::from_code(c), cs);
            assert_eq!(cs.code(), c);
            assert_eq!(Charset::from_high_nibble((c << 4) | 0x0F), cs);
        }
        assert_eq!(Charset::default(), Charset::EbuLatin);
    }

    #[test]
    fn each_charset_decodes_its_published_code_points() {
        let mut s = String::new();
        // TS 101 756 Table C.2: 0x24 is U+0142 (ł, ¤ in RDS), 0x8A U+00D1 (Ñ), 0xA9 U+20AC (€), 0xAB U+0024 ($).
        decode_into(&[0x24, 0x8A, 0xA9, 0xAB], Charset::EbuLatin, &mut s);
        assert_eq!(s, "łÑ€$");
        // A reserved code reads as EBU Latin.
        s.clear();
        decode_into(&[0x24], Charset::Other(3), &mut s);
        assert_eq!(s, "ł");
        // ISO/IEC 8859-1: 0xE9 is U+00E9.
        s.clear();
        decode_into(&[0xE9, 0x41], Charset::Latin1, &mut s);
        assert_eq!(s, "éA");
        // UCS-2 big endian: 0x041F is U+041F (П). The odd last byte is dropped. 0xD800 alone is a surrogate.
        s.clear();
        decode_into(
            &[0x04, 0x1F, 0x00, 0x20, 0xD8, 0x00, 0x00],
            Charset::Ucs2,
            &mut s,
        );
        assert_eq!(s, "\u{41F} \u{FFFD}");
        // UTF-8: C5 81 is U+0141 (Ł). 0xFF is never UTF-8.
        s.clear();
        decode_into(b"\xC5\x81ok\xFFok", Charset::Utf8, &mut s);
        assert_eq!(s, "Łok\u{FFFD}ok");
    }

    #[test]
    fn the_ebu_latin_controls_are_kept_or_dropped_for_display() {
        let mut s = String::new();
        decode_into(b"a\x0Ab\x0Bc\x1Fd", Charset::EbuLatin, &mut s);
        assert_eq!(s, "a\nb\u{B}c\u{AD}d");
        let mut s = String::from("keep ");
        display_into(
            b"News\x0Bheadline\x0Aline\x1Fbreak  ",
            Charset::EbuLatin,
            &mut s,
        );
        assert_eq!(s, "keep Newsheadline\nlinebreak");
        let mut s = String::from("x");
        display_into(&[0x00, 0x0B, 0xD8, 0x00, 0x20], Charset::Ucs2, &mut s);
        assert_eq!(s, "x");
        let mut s = String::from("ab");
        display_field_into(0, b" \x0A ", &mut s);
        assert_eq!(s, "ab");
    }

    #[test]
    fn clean_for_display_leaves_text_before_start_alone() {
        let mut s = String::from("a\u{AD}b \u{AD}c\u{1} ");
        clean_for_display(&mut s, 4);
        assert_eq!(s, "a\u{AD}b c");
        let mut s = String::from("é");
        clean_for_display(&mut s, 1);
        assert_eq!(s, "é");
        clean_for_display(&mut s, 9);
        assert_eq!(s, "é");
    }

    #[test]
    fn encoding_returns_the_published_bytes() {
        let mut b = Vec::new();
        // Table C.2: Ł 0x5E, ó 0x86, d 0x64, ź 0xFD. A character outside the table provides '?'.
        encode_into("Łódź中", Charset::EbuLatin, &mut b);
        assert_eq!(b, [0x5E, 0x86, 0x64, 0xFD, b'?']);
        b.clear();
        encode_into("é€", Charset::Latin1, &mut b);
        assert_eq!(b, [0xE9, b'?']);
        b.clear();
        encode_into("П😀", Charset::Ucs2, &mut b);
        assert_eq!(b, [0x04, 0x1F, 0xFF, 0xFD]);
        b.clear();
        encode_into("Ł", Charset::Utf8, &mut b);
        assert_eq!(b, [0xC5, 0x81]);
        for cs in [
            Charset::EbuLatin,
            Charset::Latin1,
            Charset::Ucs2,
            Charset::Utf8,
        ] {
            let mut b = Vec::new();
            encode_into("Café Łódź", cs, &mut b);
            let mut s = String::new();
            decode_into(&b, cs, &mut s);
            let want = if cs == Charset::Latin1 {
                "Café ?ód?"
            } else {
                "Café Łódź"
            };
            assert_eq!(s, want, "{cs:?}");
        }
    }
}
