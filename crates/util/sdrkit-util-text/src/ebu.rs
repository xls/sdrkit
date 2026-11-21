//! The EBU Latin based character repertoires of RDS and DAB, and RDS text in GB 2312.
//!
//! The two standards share the EBU table of EBU Tech 3232 as their base but differ, so there are two named tables:
//!
//! - [`RDS`]: EN 50067:1998 Annex E, Figure E.1 (the "complete EBU Latin based repertoire", code table G0 of RDS, carried on in
//!   IEC 62106 Annex E). Bytes 0x20 to 0x7F follow the international version (IRV) of ISO 646, so 0x24 is `¤`, 0x5E `―`, 0x60
//!   `‖` and 0x7E `¯`, and 0x00 to 0x1F are control codes.
//! - [`DAB`]: ETSI TS 101 756 V2.4.1 (and V2.5.1, identical) Annex C, Table C.2, charset 0000. Its 2015 revision assigned 0x00 to 0x1F
//!   and about thirty other positions to Central and Eastern European letters: 0x24 is `ł`, 0x5C `Ů`, 0x5E `Ł`, 0x60 `Ą`, 0x7B to
//!   0x7F `«ů»ĽĦ`, and 0xA0 `Ķ` where RDS has `ª`.
//!
//! In both, 0x0A is a line break; RDS uses 0x0D to end a RadioText, DAB keeps 0x0B (end of headline) and 0x1F (preferred word
//! break). Decoding returns those as `'\n'`, `'\r'`, `'\u{B}'` and the soft hyphen `'\u{AD}'`; the caller decides what they mean.
//!
//! RDS position 0x8D is drawn as a Greek-looking beta in Figure E.1; it is the German `ß` of the EBU common-core repertoire. 0xA9 is
//! the euro sign.
//!
//! ```
//! use sdrkit_util_text::ebu::{DAB, RDS};
//!
//! assert_eq!(RDS.char_of(0x24), Some('¤'));
//! assert_eq!(DAB.char_of(0x24), Some('ł'));
//! assert_eq!(RDS.byte_of('Ñ'), Ok(0x8A));
//! ```

use crate::error::{Error, Result};

/// A single-byte character table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Table {
    /// The Unicode scalar of each byte (0: no character).
    pub chars: [u32; 256],
    /// The name of the table.
    pub name: &'static str,
}

impl Table {
    /// The character of a byte (`None` when the byte has none).
    pub fn char_of(&self, b: u8) -> Option<char> {
        match self.chars[b as usize] {
            0 => None,
            u => char::from_u32(u),
        }
    }

    /// The byte of a character.
    pub fn byte_of(&self, c: char) -> Result<u8> {
        self.chars
            .iter()
            .position(|&u| u == c as u32 && u != 0)
            .map(|i| i as u8)
            .ok_or(Error::Unencodable(c))
    }

    /// Appends the characters of `bytes` (bytes without a character are dropped).
    pub fn decode_into(&self, bytes: &[u8], out: &mut String) {
        out.extend(bytes.iter().filter_map(|&b| self.char_of(b)));
    }

    /// Appends the bytes of a text; stops at the first character the table lacks.
    pub fn encode_into(&self, text: &str, out: &mut Vec<u8>) -> Result<()> {
        for c in text.chars() {
            out.push(self.byte_of(c)?);
        }
        Ok(())
    }
}

const fn rds() -> [u32; 256] {
    let mut t = [0u32; 256];
    t[0x0A] = '\n' as u32;
    t[0x0B] = 0x0B; // end of headline (IEC 62106)
    t[0x0D] = '\r' as u32;
    t[0x1F] = 0xAD; // soft hyphen (IEC 62106)
    let mut i = 0x20;
    while i < 0x7F {
        t[i] = i as u32;
        i += 1;
    }
    // ISO 646 IRV positions of Figure E.1.
    t[0x24] = 0xA4; // ¤
    t[0x5E] = 0x2015; // ―
    t[0x60] = 0x2016; // ‖
    t[0x7E] = 0xAF; // ¯
    const HIGH: [char; 127] = [
        'á', 'à', 'é', 'è', 'í', 'ì', 'ó', 'ò', 'ú', 'ù', 'Ñ', 'Ç', 'Ş', 'ß', '¡', 'Ĳ', //
        'â', 'ä', 'ê', 'ë', 'î', 'ï', 'ô', 'ö', 'û', 'ü', 'ñ', 'ç', 'ş', 'ğ', 'ı', 'ĳ', //
        'ª', 'α', '©', '‰', 'Ğ', 'ě', 'ň', 'ő', 'π', '€', '£', '$', '←', '↑', '→', '↓', //
        'º', '¹', '²', '³', '±', 'İ', 'ń', 'ű', 'µ', '¿', '÷', '°', '¼', '½', '¾', '§', //
        'Á', 'À', 'É', 'È', 'Í', 'Ì', 'Ó', 'Ò', 'Ú', 'Ù', 'Ř', 'Č', 'Š', 'Ž', 'Ð', 'Ŀ', //
        'Â', 'Ä', 'Ê', 'Ë', 'Î', 'Ï', 'Ô', 'Ö', 'Û', 'Ü', 'ř', 'č', 'š', 'ž', 'đ', 'ŀ', //
        'Ã', 'Å', 'Æ', 'Œ', 'ŷ', 'Ý', 'Õ', 'Ø', 'Þ', 'Ŋ', 'Ŕ', 'Ć', 'Ś', 'Ź', 'Ŧ', 'ð', //
        'ã', 'å', 'æ', 'œ', 'ŵ', 'ý', 'õ', 'ø', 'þ', 'ŋ', 'ŕ', 'ć', 'ś', 'ź', 'ŧ', //
    ];
    let mut i = 0;
    while i < 127 {
        t[0x80 + i] = HIGH[i] as u32;
        i += 1;
    }
    t
}

/// The RDS table (EN 50067:1998 Annex E, Figure E.1).
pub const RDS: Table = Table {
    chars: rds(),
    name: "EN 50067 Annex E",
};

const fn dab() -> [u32; 256] {
    const T: [u16; 256] = [
        0x0000, 0x0118, 0x012E, 0x0172, 0x0102, 0x0116, 0x010E, 0x0218, 0x021A, 0x010A, 0x0000,
        0x0000, 0x0120, 0x0139, 0x017B, 0x0143, 0x0105, 0x0119, 0x012F, 0x0173, 0x0103, 0x0117,
        0x010F, 0x0219, 0x021B, 0x010B, 0x0147, 0x011A, 0x0121, 0x013A, 0x017C, 0x0000, 0x0020,
        0x0021, 0x0022, 0x0023, 0x0142, 0x0025, 0x0026, 0x0027, 0x0028, 0x0029, 0x002A, 0x002B,
        0x002C, 0x002D, 0x002E, 0x002F, 0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036,
        0x0037, 0x0038, 0x0039, 0x003A, 0x003B, 0x003C, 0x003D, 0x003E, 0x003F, 0x0040, 0x0041,
        0x0042, 0x0043, 0x0044, 0x0045, 0x0046, 0x0047, 0x0048, 0x0049, 0x004A, 0x004B, 0x004C,
        0x004D, 0x004E, 0x004F, 0x0050, 0x0051, 0x0052, 0x0053, 0x0054, 0x0055, 0x0056, 0x0057,
        0x0058, 0x0059, 0x005A, 0x005B, 0x016E, 0x005D, 0x0141, 0x005F, 0x0104, 0x0061, 0x0062,
        0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B, 0x006C, 0x006D,
        0x006E, 0x006F, 0x0070, 0x0071, 0x0072, 0x0073, 0x0074, 0x0075, 0x0076, 0x0077, 0x0078,
        0x0079, 0x007A, 0x00AB, 0x016F, 0x00BB, 0x013D, 0x0126, 0x00E1, 0x00E0, 0x00E9, 0x00E8,
        0x00ED, 0x00EC, 0x00F3, 0x00F2, 0x00FA, 0x00F9, 0x00D1, 0x00C7, 0x015E, 0x00DF, 0x00A1,
        0x0178, 0x00E2, 0x00E4, 0x00EA, 0x00EB, 0x00EE, 0x00EF, 0x00F4, 0x00F6, 0x00FB, 0x00FC,
        0x00F1, 0x00E7, 0x015F, 0x011F, 0x0131, 0x00FF, 0x0136, 0x0145, 0x00A9, 0x0122, 0x011E,
        0x011B, 0x0148, 0x0151, 0x0150, 0x20AC, 0x00A3, 0x0024, 0x0100, 0x0112, 0x012A, 0x016A,
        0x0137, 0x0146, 0x013B, 0x0123, 0x013C, 0x0130, 0x0144, 0x0171, 0x0170, 0x00BF, 0x013E,
        0x00B0, 0x0101, 0x0113, 0x012B, 0x016B, 0x00C1, 0x00C0, 0x00C9, 0x00C8, 0x00CD, 0x00CC,
        0x00D3, 0x00D2, 0x00DA, 0x00D9, 0x0158, 0x010C, 0x0160, 0x017D, 0x00D0, 0x013F, 0x00C2,
        0x00C4, 0x00CA, 0x00CB, 0x00CE, 0x00CF, 0x00D4, 0x00D6, 0x00DB, 0x00DC, 0x0159, 0x010D,
        0x0161, 0x017E, 0x0111, 0x0140, 0x00C3, 0x00C5, 0x00C6, 0x0152, 0x0177, 0x00DD, 0x00D5,
        0x00D8, 0x00DE, 0x014A, 0x0154, 0x0106, 0x015A, 0x0179, 0x0164, 0x00F0, 0x00E3, 0x00E5,
        0x00E6, 0x0153, 0x0175, 0x00FD, 0x00F5, 0x00F8, 0x00FE, 0x014B, 0x0155, 0x0107, 0x015B,
        0x017A, 0x0165, 0x0127,
    ];
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = T[i] as u32;
        i += 1;
    }
    t[0x0A] = '\n' as u32; // preferred line break
    t[0x0B] = 0x0B; // end of headline
    t[0x1F] = 0xAD; // preferred word break (shown as a soft hyphen)
    t
}

/// The DAB table (ETSI TS 101 756 Annex C, charset 0000).
pub const DAB: Table = Table {
    chars: dab(),
    name: "TS 101 756 Annex C",
};

/// How RDS text bytes are read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RdsCharset {
    /// Always the RDS table.
    EbuLatin,
    /// GB 2312 when the bytes look like it, the RDS table otherwise.
    #[default]
    Auto,
    /// GB 2312 for every pair of bytes 0xA1 or higher.
    Gb2312,
}

fn is_gb(b: u8) -> bool {
    (0xA1..=0xFE).contains(&b)
}

/// Whether RDS bytes read as GB 2312 under `mode`. In [`RdsCharset::Auto`]: at least one pair of high bytes, and either no stray
/// high byte or at least two pairs with stray bytes at most half the pairs (some Chinese stations send GB 2312 without signalling).
pub fn reads_as_gb2312(bytes: &[u8], mode: RdsCharset) -> bool {
    match mode {
        RdsCharset::EbuLatin => false,
        RdsCharset::Gb2312 => true,
        RdsCharset::Auto => {
            let (mut pairs, mut stray, mut i) = (0, 0, 0);
            while i < bytes.len() {
                if is_gb(bytes[i]) && i + 1 < bytes.len() && is_gb(bytes[i + 1]) {
                    pairs += 1;
                    i += 2;
                } else {
                    stray += (bytes[i] >= 0x80) as usize;
                    i += 1;
                }
            }
            pairs >= 1 && (stray == 0 || (pairs >= 2 && stray * 2 <= pairs))
        }
    }
}

/// One GB 2312 pair (rows 0xA1 to 0xA9 symbols, 0xB0 to 0xF7 hanzi), through encoding_rs.
fn gb_pair(a: u8, b: u8) -> Option<char> {
    if !matches!(a, 0xA1..=0xA9 | 0xB0..=0xF7) {
        return None;
    }
    let mut dec = encoding_rs::GBK.new_decoder_without_bom_handling();
    let mut buf = [0u8; 8];
    let (res, _, n, bad) = dec.decode_to_utf8(&[a, b], &mut buf, true);
    if bad || n == 0 || res != encoding_rs::CoderResult::InputEmpty {
        return None;
    }
    let c = std::str::from_utf8(&buf[..n]).ok()?.chars().next()?;
    (!('\u{E000}'..='\u{F8FF}').contains(&c)).then_some(c)
}

/// Appends RDS text (PS, RadioText, PTYN) decoded with the RDS table or GB 2312. A byte or pair with no character becomes `?`.
pub fn rds_text_into(bytes: &[u8], mode: RdsCharset, out: &mut String) {
    if !reads_as_gb2312(bytes, mode) {
        out.extend(bytes.iter().map(|&b| RDS.char_of(b).unwrap_or('?')));
        return;
    }
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if is_gb(b) && i + 1 < bytes.len() && is_gb(bytes[i + 1]) {
            out.push(gb_pair(b, bytes[i + 1]).unwrap_or('?'));
            i += 2;
        } else {
            out.push(if b < 0x80 {
                RDS.char_of(b).unwrap_or('?')
            } else {
                '?'
            });
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dab_table_c2_entries() {
        // Spot values of TS 101 756 Table C.2 (the whole table is transcribed from it).
        for (b, u) in [
            (0x01, 0x0118),
            (0x1E, 0x017C),
            (0x24, 0x0142),
            (0x5C, 0x016E),
            (0x5E, 0x0141),
            (0x60, 0x0104),
            (0x7B, 0x00AB),
            (0x7F, 0x0126),
            (0x8F, 0x0178),
            (0xA0, 0x0136),
            (0xA3, 0x0122),
            (0xA9, 0x20AC),
            (0xAB, 0x0024),
            (0xEE, 0x0164),
            (0xFF, 0x0127),
        ] {
            assert_eq!(DAB.chars[b], u, "{b:#x}");
        }
        assert_eq!(DAB.char_of(0x00), None);
        assert_eq!(DAB.char_of(0x0A), Some('\n'));
    }

    #[test]
    fn rds_figure_e1_entries() {
        for (b, c) in [
            (0x24, '¤'),
            (0x5E, '―'),
            (0x60, '‖'),
            (0x7E, '¯'),
            (0x80, 'á'),
            (0x8D, 'ß'),
            (0x8E, '¡'),
            (0xA0, 'ª'),
            (0xA9, '€'),
            (0xAB, '$'),
            (0xBB, '°'),
            (0xCF, 'Ŀ'),
            (0xEE, 'Ŧ'),
            (0xFE, 'ŧ'),
        ] {
            assert_eq!(RDS.char_of(b), Some(c), "{b:#x}");
        }
        assert_eq!(RDS.char_of(0x7F), None);
        assert_eq!(RDS.char_of(0xFF), None);
        assert_eq!(RDS.char_of(0x01), None);
    }

    #[test]
    fn the_two_tables_differ_where_the_standards_do() {
        let differ: Vec<usize> = (0x20..0x100)
            .filter(|&b| RDS.chars[b] != DAB.chars[b])
            .collect();
        // ASCII range: 0x24, 0x5C, 0x5E, 0x60, 0x7B to 0x7F.
        assert_eq!(
            &differ[..9],
            &[0x24, 0x5C, 0x5E, 0x60, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F]
        );
        assert_eq!(differ.len(), 33);
        // Every table round trips.
        for t in [&RDS, &DAB] {
            for b in 0..=255u8 {
                if let Some(c) = t.char_of(b) {
                    assert_eq!(t.byte_of(c), Ok(b), "{} {b:#x}", t.name);
                }
            }
        }
    }

    #[test]
    fn rds_gb2312() {
        let gb = [0xD6, 0xD0, 0xB9, 0xFA, 0xD6, 0xAE, 0xC9, 0xF9];
        let mut s = String::new();
        rds_text_into(&gb, RdsCharset::Auto, &mut s);
        assert_eq!(s, "中国之声");
        s.clear();
        let mut rt = b"FM 93.9 ".to_vec();
        rt.extend_from_slice(&[0xBD, 0xBB, 0xCD, 0xA8, 0xB5, 0xE7, 0xCC, 0xA8]);
        rds_text_into(&rt, RdsCharset::Auto, &mut s);
        assert_eq!(s, "FM 93.9 交通电台");
        s.clear();
        rds_text_into(b"B\x8aR FM", RdsCharset::Auto, &mut s);
        assert_eq!(s, "BÑR FM");
        s.clear();
        rds_text_into(&[0xAA, 0xA1, 0xF8, 0xA1], RdsCharset::Gb2312, &mut s);
        assert_eq!(s, "??");
    }
}
