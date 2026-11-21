//! Non-spacing diacritical marks and their composition with a base letter. ISO/IEC 6937 (columns C1 to CF) and the teletext G2
//! set (column 4, used by packet X/26) put the same fifteen marks in the same order, so both use this table.

/// The marks, in code order (ISO/IEC 6937 0xC1 to 0xCF; teletext G2 0x41 to 0x4F).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// Grave accent.
    Grave = 1,
    /// Acute accent.
    Acute,
    /// Circumflex.
    Circumflex,
    /// Tilde.
    Tilde,
    /// Macron.
    Macron,
    /// Breve.
    Breve,
    /// Dot above.
    DotAbove,
    /// Diaeresis (umlaut).
    Diaeresis,
    /// Dot below (teletext G2 4/9; unused in ISO/IEC 6937, where 0xC9 is not assigned).
    DotBelow,
    /// Ring above.
    Ring,
    /// Cedilla.
    Cedilla,
    /// Low line (teletext G2 4/C; unused in ISO/IEC 6937, where 0xCC is not assigned).
    LowLine,
    /// Double acute accent.
    DoubleAcute,
    /// Ogonek.
    Ogonek,
    /// Caron.
    Caron,
}

impl Mark {
    /// The mark of a position 1 to 15 in the column.
    pub fn from_index(i: u8) -> Option<Mark> {
        use Mark::*;
        const ALL: [Mark; 15] = [
            Grave,
            Acute,
            Circumflex,
            Tilde,
            Macron,
            Breve,
            DotAbove,
            Diaeresis,
            DotBelow,
            Ring,
            Cedilla,
            LowLine,
            DoubleAcute,
            Ogonek,
            Caron,
        ];
        ALL.get((i as usize).wrapping_sub(1)).copied()
    }

    /// The Unicode combining character.
    pub fn combining(self) -> char {
        const C: [char; 15] = [
            '\u{300}', '\u{301}', '\u{302}', '\u{303}', '\u{304}', '\u{306}', '\u{307}', '\u{308}',
            '\u{323}', '\u{30A}', '\u{327}', '\u{332}', '\u{30B}', '\u{328}', '\u{30C}',
        ];
        C[self as usize - 1]
    }

    /// The precomposed letter of `base` with this mark, when Unicode has one (Latin letters only).
    pub fn compose(self, base: char) -> Option<char> {
        let (bases, composed) = COMPOSE[self as usize - 1];
        let k = bases.bytes().position(|b| b as char == base)?;
        composed.chars().nth(k)
    }

    /// The base letter and mark of a precomposed letter (the inverse of [`Mark::compose`]).
    pub fn decompose(c: char) -> Option<(char, Mark)> {
        for (m, (bases, composed)) in COMPOSE.iter().enumerate() {
            if let Some(k) = composed.chars().position(|x| x == c) {
                let mark = Mark::from_index(m as u8 + 1)?;
                return bases.chars().nth(k).map(|b| (b, mark));
            }
        }
        None
    }
}

/// For each mark in order: the base letters and the precomposed letters (Unicode canonical composition of ASCII letters).
const COMPOSE: [(&str, &str); 15] = [
    // grave U+0300
    ("AEINOUWYaeinouwy", "ÀÈÌǸÒÙẀỲàèìǹòùẁỳ"),
    // acute U+0301
    (
        "ACEGIKLMNOPRSUWYZacegiklmnoprsuwyz",
        "ÁĆÉǴÍḰĹḾŃÓṔŔŚÚẂÝŹáćéǵíḱĺḿńóṕŕśúẃýź",
    ),
    // circumflex U+0302
    ("ACEGHIJOSUWYZaceghijosuwyz", "ÂĈÊĜĤÎĴÔŜÛŴŶẐâĉêĝĥîĵôŝûŵŷẑ"),
    // tilde U+0303
    ("AEINOUVYaeinouvy", "ÃẼĨÑÕŨṼỸãẽĩñõũṽỹ"),
    // macron U+0304
    ("AEGIOUYaegiouy", "ĀĒḠĪŌŪȲāēḡīōūȳ"),
    // breve U+0306
    ("AEGIOUaegiou", "ĂĔĞĬŎŬăĕğĭŏŭ"),
    // dot_above U+0307
    (
        "ABCDEFGHIMNOPRSTWXYZabcdefghmnoprstwxyz",
        "ȦḂĊḊĖḞĠḢİṀṄȮṖṘṠṪẆẊẎŻȧḃċḋėḟġḣṁṅȯṗṙṡṫẇẋẏż",
    ),
    // diaeresis U+0308
    ("AEHIOUWXYaehiotuwxy", "ÄËḦÏÖÜẄẌŸäëḧïöẗüẅẍÿ"),
    // dot_below U+0323
    (
        "ABDEHIKLMNORSTUVWYZabdehiklmnorstuvwyz",
        "ẠḄḌẸḤỊḲḶṂṆỌṚṢṬỤṾẈỴẒạḅḍẹḥịḳḷṃṇọṛṣṭụṿẉỵẓ",
    ),
    // ring U+030A
    ("AUauwy", "ÅŮåůẘẙ"),
    // cedilla U+0327
    ("CDEGHKLNRSTcdeghklnrst", "ÇḐȨĢḨĶĻŅŖŞŢçḑȩģḩķļņŗşţ"),
    // low_line U+0332
    ("", ""),
    // double_acute U+030B
    ("OUou", "ŐŰőű"),
    // ogonek U+0328
    ("AEIOUaeiou", "ĄĘĮǪŲąęįǫų"),
    // caron U+030C
    (
        "ACDEGHIKLNORSTUZacdeghijklnorstuz",
        "ǍČĎĚǦȞǏǨĽŇǑŘŠŤǓŽǎčďěǧȟǐǰǩľňǒřšťǔž",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_and_decompose() {
        assert_eq!(Mark::Diaeresis.compose('u'), Some('ü'));
        assert_eq!(Mark::Caron.compose('S'), Some('Š'));
        assert_eq!(Mark::DoubleAcute.compose('o'), Some('ő'));
        assert_eq!(Mark::Ogonek.compose('E'), Some('Ę'));
        assert_eq!(Mark::Cedilla.compose('T'), Some('Ţ'));
        assert_eq!(Mark::Ring.compose('q'), None);
        assert_eq!(Mark::LowLine.compose('a'), None);
        assert_eq!(Mark::decompose('Ł'), None);
        assert_eq!(Mark::decompose('é'), Some(('e', Mark::Acute)));
        for i in 1..=15 {
            let m = Mark::from_index(i).unwrap();
            assert_eq!(m as u8, i);
            let (bases, composed) = COMPOSE[i as usize - 1];
            assert_eq!(bases.chars().count(), composed.chars().count());
            for (b, c) in bases.chars().zip(composed.chars()) {
                assert_eq!(m.compose(b), Some(c));
                assert_eq!(Mark::decompose(c).map(|x| x.0), Some(b));
            }
        }
        assert_eq!(Mark::from_index(0), None);
        assert_eq!(Mark::from_index(16), None);
    }
}
