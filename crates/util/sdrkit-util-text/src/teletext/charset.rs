//! Teletext character sets (ETS 300 706 clause 15): the Latin G0 primary set (Table 35) with its thirteen national option
//! sub-sets (Table 36), the Latin G2 supplementary set (Table 37), and the designation of the default sets (Table 32).

/// The thirteen national positions of the Latin G0 set.
pub const NATIONAL_POSITIONS: [u8; 13] = [
    0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x5F, 0x60, 0x7B, 0x7C, 0x7D, 0x7E,
];

/// A Latin national option sub-set (Table 36).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum National {
    /// Czech and Slovak.
    CzechSlovak,
    /// English.
    #[default]
    English,
    /// Estonian.
    Estonian,
    /// French.
    French,
    /// German.
    German,
    /// Italian.
    Italian,
    /// Lettish (Latvian) and Lithuanian.
    LettishLithuanian,
    /// Polish.
    Polish,
    /// Portuguese and Spanish.
    PortugueseSpanish,
    /// Rumanian.
    Rumanian,
    /// Serbian, Croatian and Slovenian.
    SerbianCroatianSlovenian,
    /// Swedish, Finnish and Hungarian.
    SwedishFinnishHungarian,
    /// Turkish.
    Turkish,
}

impl National {
    /// The characters at [`NATIONAL_POSITIONS`] (Table 36). Turkish 2/3 is the Turkish lira sign; Table 36 note 2 says some
    /// decoders show `$` instead of `Ë` at 2/4 of the Serbian/Croatian/Slovenian set.
    pub fn chars(self) -> [char; 13] {
        let s = match self {
            National::CzechSlovak => "#ůčťžýířéáěúš",
            National::English => "£$@←½→↑#―¼‖¾÷",
            National::Estonian => "#õŠÄÖŽÜÕšäöžü",
            National::French => "éïàëêùî#èâôûç",
            National::German => "#$§ÄÖÜ^_°äöüß",
            National::Italian => "£$é°ç→↑#ùàòèì",
            National::LettishLithuanian => "#$ŠėęŽčūšąųžį",
            National::Polish => "#ńąƵŚŁćóężśłź",
            National::PortugueseSpanish => "ç$¡áéíóú¿üñèà",
            National::Rumanian => "#¤ŢÂŞĂÎıţâşăî",
            National::SerbianCroatianSlovenian => "#ËČĆŽĐŠëčćžđš",
            National::SwedishFinnishHungarian => "#¤ÉÄÖÅÜ_éäöåü",
            National::Turkish => "₺ğİŞÖÇÜĞışöçü",
        };
        let mut out = [' '; 13];
        for (o, c) in out.iter_mut().zip(s.chars()) {
            *o = c;
        }
        out
    }
}

/// A G0 primary set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum G0 {
    /// Latin with a national option sub-set.
    Latin(National),
    /// Cyrillic option 1 (Serbian/Croatian).
    Cyrillic1,
    /// Cyrillic option 2 (Russian/Bulgarian).
    Cyrillic2,
    /// Cyrillic option 3 (Ukrainian).
    Cyrillic3,
    /// Greek.
    Greek,
    /// Arabic.
    Arabic,
    /// Hebrew.
    Hebrew,
}

/// A G2 supplementary set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum G2 {
    /// Latin (Table 37).
    Latin,
    /// Cyrillic.
    Cyrillic,
    /// Greek.
    Greek,
    /// Arabic.
    Arabic,
}

/// The default G0 and G2 sets of a 7-bit designation (Table 32): bits 14 to 11 from packet X/28 or M/29 (0 when there is none) and
/// bits 10 to 8 the C12, C13 and C14 bits of the page header. `None` for a reserved entry.
pub fn designation(code: u8) -> Option<(G0, G2)> {
    use National::*;
    let latin = |n| Some((G0::Latin(n), G2::Latin));
    match (code >> 3 & 15, code & 7) {
        (0, 0) => latin(English),
        (0..=2, 1) => latin(German),
        (0..=2, 2) => latin(SwedishFinnishHungarian),
        (0..=2, 3) => latin(Italian),
        (0..=2, 4) => latin(French),
        (0 | 2, 5) => latin(PortugueseSpanish),
        (0 | 1, 6) => latin(CzechSlovak),
        (1, 0) => latin(Polish),
        (2, 0) => latin(English),
        (2, 6) => latin(Turkish),
        (3, 5) => latin(SerbianCroatianSlovenian),
        (3, 7) => latin(Rumanian),
        (4, 0) => Some((G0::Cyrillic1, G2::Cyrillic)),
        (4, 1) => latin(German),
        (4, 2) => latin(Estonian),
        (4, 3) => latin(LettishLithuanian),
        (4, 4) => Some((G0::Cyrillic2, G2::Cyrillic)),
        (4, 5) => Some((G0::Cyrillic3, G2::Cyrillic)),
        (4, 6) => latin(CzechSlovak),
        (6, 6) => latin(Turkish),
        (6, 7) => Some((G0::Greek, G2::Greek)),
        (8, 0) => Some((G0::Latin(English), G2::Arabic)),
        (8, 4) => Some((G0::Latin(French), G2::Arabic)),
        (8 | 10, 7) => Some((G0::Arabic, G2::Arabic)),
        (10, 5) => Some((G0::Hebrew, G2::Arabic)),
        _ => None,
    }
}

/// The Latin G0 character of a code 0x20 to 0x7F under a national option (Tables 35 and 36; 0x7F is a full block).
pub fn latin_g0(code: u8, national: National) -> Option<char> {
    if let Some(i) = NATIONAL_POSITIONS.iter().position(|&p| p == code) {
        return Some(national.chars()[i]);
    }
    match code {
        0x7F => Some('■'),
        0x20..=0x7E => Some(code as char),
        _ => None,
    }
}

/// The Latin G2 supplementary set, 0x20 to 0x7F (Table 37); 0x40 to 0x4F are the non-spacing diacritical marks, shown here as
/// their spacing forms; 0 for an unassigned position.
const LATIN_G2: [u32; 96] = [
    0x00A0, 0x00A1, 0x00A2, 0x00A3, 0x0024, 0x00A5, 0x0023, 0x00A7, 0x00A4, 0x2018, 0x201C, 0x00AB,
    0x2190, 0x2191, 0x2192, 0x2193, //
    0x00B0, 0x00B1, 0x00B2, 0x00B3, 0x00D7, 0x00B5, 0x00B6, 0x00B7, 0x00F7, 0x2019, 0x201D, 0x00BB,
    0x00BC, 0x00BD, 0x00BE, 0x00BF, //
    0x0020, 0x0060, 0x00B4, 0x02C6, 0x02DC, 0x00AF, 0x02D8, 0x02D9, 0x00A8, 0x002E, 0x02DA, 0x00B8,
    0x005F, 0x02DD, 0x02DB, 0x02C7, //
    0x2015, 0x00B9, 0x00AE, 0x00A9, 0x2122, 0x266A, 0x20A0, 0x2030, 0x03B1, 0, 0, 0, 0x215B,
    0x215C, 0x215D, 0x215E, //
    0x2126, 0x00C6, 0x0110, 0x00AA, 0x0126, 0, 0x0132, 0x013F, 0x0141, 0x00D8, 0x0152, 0x00BA,
    0x00DE, 0x0166, 0x014A, 0x0149, //
    0x0138, 0x00E6, 0x0111, 0x00F0, 0x0127, 0x0131, 0x0133, 0x0140, 0x0142, 0x00F8, 0x0153, 0x00DF,
    0x00FE, 0x0167, 0x014B, 0x25A0,
];

/// The Latin G2 character of a code 0x20 to 0x7F.
pub fn latin_g2(code: u8) -> Option<char> {
    match code {
        0x20..=0x7F => match LATIN_G2[(code - 0x20) as usize] {
            0 => None,
            u => char::from_u32(u),
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_36() {
        let all = [
            National::CzechSlovak,
            National::English,
            National::Estonian,
            National::French,
            National::German,
            National::Italian,
            National::LettishLithuanian,
            National::Polish,
            National::PortugueseSpanish,
            National::Rumanian,
            National::SerbianCroatianSlovenian,
            National::SwedishFinnishHungarian,
            National::Turkish,
        ];
        for n in all {
            assert!(n.chars().iter().all(|&c| c != ' '), "{n:?}");
        }
        assert_eq!(latin_g0(0x23, National::English), Some('£'));
        assert_eq!(latin_g0(0x7E, National::German), Some('ß'));
        assert_eq!(latin_g0(0x5B, National::Polish), Some('Ƶ'));
        assert_eq!(latin_g0(0x41, National::French), Some('A'));
        assert_eq!(latin_g0(0x7F, National::English), Some('■'));
        assert_eq!(latin_g0(0x1F, National::English), None);
    }

    #[test]
    fn table_32() {
        // Without packet 28 the group is 0000 and C12 C13 C14 pick the sub-set.
        let n = |c| match designation(c) {
            Some((G0::Latin(n), G2::Latin)) => Some(n),
            _ => None,
        };
        assert_eq!(n(0b000_0000), Some(National::English));
        assert_eq!(n(0b000_0001), Some(National::German));
        assert_eq!(n(0b000_0010), Some(National::SwedishFinnishHungarian));
        assert_eq!(n(0b000_0011), Some(National::Italian));
        assert_eq!(n(0b000_0100), Some(National::French));
        assert_eq!(n(0b000_0101), Some(National::PortugueseSpanish));
        assert_eq!(n(0b000_0110), Some(National::CzechSlovak));
        assert_eq!(designation(0b000_0111), None);
        assert_eq!(n(0b000_1000), Some(National::Polish));
        assert_eq!(n(0b001_0110), Some(National::Turkish));
        assert_eq!(n(0b001_1101), Some(National::SerbianCroatianSlovenian));
        assert_eq!(n(0b001_1111), Some(National::Rumanian));
        assert_eq!(designation(0b010_0100), Some((G0::Cyrillic2, G2::Cyrillic)));
        assert_eq!(n(0b010_0010), Some(National::Estonian));
        assert_eq!(n(0b010_0011), Some(National::LettishLithuanian));
        assert_eq!(designation(0b011_0111), Some((G0::Greek, G2::Greek)));
        assert_eq!(
            designation(0b100_0000),
            Some((G0::Latin(National::English), G2::Arabic))
        );
        assert_eq!(designation(0b101_0101), Some((G0::Hebrew, G2::Arabic)));
        assert_eq!(designation(0b010_1000), None);
    }

    #[test]
    fn table_37() {
        for (c, u) in [
            (0x24, '$'),
            (0x26, '#'),
            (0x28, '¤'),
            (0x34, '×'),
            (0x41, '`'),
            (0x48, '¨'),
            (0x50, '―'),
            (0x56, '₠'),
            (0x60, '\u{2126}'),
            (0x7B, 'ß'),
            (0x7F, '■'),
        ] {
            assert_eq!(latin_g2(c), Some(u), "{c:#x}");
        }
        assert_eq!(latin_g2(0x59), None);
        assert_eq!(latin_g2(0x65), None);
    }
}
