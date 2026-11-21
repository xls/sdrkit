//! DCS (digital coded squelch, Motorola "DPL") codes.
//!
//! A code is three octal digits (nine bits). On the air it is a 23-bit word sent again and again at 134.4 bit/s: the nine code
//! bits, the fixed bits 0 0 1 (the octal digit 4 in front of the code), and eleven check bits of the (23,12) Golay code with
//! generator x<sup>11</sup> + x<sup>10</sup> + x<sup>6</sup> + x<sup>5</sup> + x<sup>4</sup> + x<sup>2</sup> + 1 (TIA-603;
//! Midian Electronics "Tone Signaling Charts", "Standard 83 codes used in the DCS scheme": "a fixed octal digit 4 as the 1st
//! digit, followed by three octal digits ... 12 bits of octal code, followed by 11 bits of CRC. Each bit is 7.5 ms"). The 23-bit
//! word here has bit `i` = the `i`-th bit on the air, code bits first, least significant first.
//!
//! Because the word repeats, a receiver can start anywhere in it, and because the Golay code is cyclic, a rotation of one code's
//! word (or of its complement, the "inverted" polarity) can be the word of another code. Only 83 codes are in common use
//! ([`STANDARD`]); [`decode`] tries every rotation and both polarities and reports the alias, so "D023N" is also read as "D047I".
//!
//! ```
//! use sdrkit_util_ids::tones::dcs::{self, Dcs};
//!
//! let d023 = Dcs::from_octal("023").unwrap();
//! assert_eq!(d023.word(), 0x76_3813);
//! let r = dcs::decode(d023.word()).unwrap();
//! assert_eq!((r.code, r.inverted), (d023, false));
//! assert_eq!(r.alias, Some((Dcs::from_octal("047").unwrap(), true)));
//! assert_eq!(d023.to_string(), "D023");
//! ```

use std::fmt;

/// The bit rate, bit/s.
pub const BIT_RATE: f32 = 134.4;
/// The tone a transmitter sends for 200 ms at the end of a DCS transmission to close the receiver's squelch, Hz (Midian:
/// "turn off code is 200 milliseconds of 134 hertz").
pub const TURN_OFF_HZ: f32 = 134.0;
/// The length of the turn-off tone, s.
pub const TURN_OFF_S: f32 = 0.2;

/// The Golay generator x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1.
const GOLAY: u32 = 0xC75;
const MASK23: u32 = (1 << 23) - 1;

/// The 83 standard codes, as three octal digits read as one octal number (Midian; TIA-603).
pub static STANDARD: [u16; 83] = [
    0o023, 0o025, 0o026, 0o031, 0o032, 0o043, 0o047, 0o051, 0o054, 0o065, 0o071, 0o072, 0o073,
    0o074, 0o114, 0o115, 0o116, 0o125, 0o131, 0o132, 0o134, 0o143, 0o152, 0o155, 0o156, 0o162,
    0o165, 0o172, 0o174, 0o205, 0o223, 0o226, 0o243, 0o244, 0o245, 0o251, 0o261, 0o263, 0o265,
    0o271, 0o306, 0o311, 0o315, 0o331, 0o343, 0o346, 0o351, 0o364, 0o365, 0o371, 0o411, 0o412,
    0o413, 0o423, 0o431, 0o432, 0o445, 0o464, 0o465, 0o466, 0o503, 0o506, 0o516, 0o532, 0o546,
    0o565, 0o606, 0o612, 0o624, 0o627, 0o631, 0o632, 0o654, 0o662, 0o664, 0o703, 0o712, 0o723,
    0o731, 0o732, 0o734, 0o743, 0o754,
];

/// A DCS code: a nine-bit value written as three octal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dcs(u16);

impl Dcs {
    /// A code from its nine-bit value (0 to 0o777).
    pub const fn new(value: u16) -> Option<Dcs> {
        if value <= 0o777 {
            Some(Dcs(value))
        } else {
            None
        }
    }

    /// A code from its written form: three octal digits, with or without a leading `D` ("023", "D023").
    pub fn from_octal(s: &str) -> Option<Dcs> {
        let t = s.trim();
        let t = t.strip_prefix(['D', 'd']).unwrap_or(t);
        if t.len() != 3 || !t.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
            return None;
        }
        Dcs::new(t.bytes().fold(0u16, |a, b| a * 8 + (b - b'0') as u16))
    }

    /// The nine-bit value.
    pub const fn value(self) -> u16 {
        self.0
    }

    /// Whether the code is one of the 83 standard codes.
    pub fn is_standard(self) -> bool {
        STANDARD.binary_search(&self.0).is_ok()
    }

    /// The 23-bit word sent on the air, normal polarity (bit `i` = `i`-th bit sent).
    pub const fn word(self) -> u32 {
        let data = (self.0 as u32) | (0b100 << 9);
        (parity(data) << 12) | data
    }

    /// The code an inverted transmission of this code is read as at normal polarity, preferring a standard code (`None` when no
    /// rotation of the inverted word is a valid word).
    pub fn inverted_alias(self) -> Option<Dcs> {
        let w = !self.word() & MASK23;
        let mut best: Option<Dcs> = None;
        for r in 0..23 {
            if let Some(c) = valid(rotate(w, r)) {
                if best.is_none() || (c.is_standard() && !best.is_some_and(Dcs::is_standard)) {
                    best = Some(c);
                }
            }
        }
        best
    }
}

impl fmt::Display for Dcs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "D{:03o}", self.0)
    }
}

/// The eleven check bits: the remainder of data(x) * x^11 divided by the generator.
const fn parity(data12: u32) -> u32 {
    let mut r = (data12 & 0xFFF) << 11;
    let mut i = 22;
    while i >= 11 {
        if r & (1 << i) != 0 {
            r ^= GOLAY << (i - 11);
        }
        i -= 1;
    }
    r & 0x7FF
}

/// Rotates a 23-bit word right by `k` (the bit sent `k` bits later becomes bit 0).
const fn rotate(w: u32, k: u32) -> u32 {
    let k = k % 23;
    if k == 0 {
        w & MASK23
    } else {
        ((w >> k) | (w << (23 - k))) & MASK23
    }
}

/// The code of a word aligned at its start, when the fixed bits and the check bits are right.
fn valid(w: u32) -> Option<Dcs> {
    let data = w & 0xFFF;
    (data >> 9 == 0b100 && parity(data) == (w >> 12) & 0x7FF).then_some(Dcs((data & 0x1FF) as u16))
}

/// What a received word reads as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reading {
    /// The code chosen: normal polarity first, then a standard code, then the first rotation.
    pub code: Dcs,
    /// Whether the code was read at inverted polarity.
    pub inverted: bool,
    /// Another reading of the same word that is a standard code (normal polarity preferred), when there is one.
    pub alias: Option<(Dcs, bool)>,
}

impl Reading {
    /// The name with its polarity, "D023N" or "D047I".
    pub fn name(&self) -> String {
        format!("{}{}", self.code, if self.inverted { 'I' } else { 'N' })
    }
}

/// Reads a received 23-bit word at every rotation and both polarities. `None` when no reading is valid. Does not allocate.
pub fn decode(word: u32) -> Option<Reading> {
    let mut found = [(Dcs(0), false); 46];
    let mut n = 0;
    for inverted in [false, true] {
        let w = if inverted {
            !word & MASK23
        } else {
            word & MASK23
        };
        for r in 0..23 {
            if let Some(c) = valid(rotate(w, r)) {
                if !found[..n].contains(&(c, inverted)) {
                    found[n] = (c, inverted);
                    n += 1;
                }
            }
        }
    }
    let found = &found[..n];
    let rank = |r: &(Dcs, bool)| (r.1 as u8, !r.0.is_standard() as u8);
    let best = *found
        .iter()
        .enumerate()
        .min_by_key(|(i, r)| (rank(r), *i))?
        .1;
    let alias = found
        .iter()
        .filter(|r| **r != best && r.0.is_standard())
        .min_by_key(|r| r.1 as u8)
        .copied();
    Some(Reading {
        code: best.0,
        inverted: best.1,
        alias,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dcs {
        Dcs::from_octal(s).unwrap()
    }

    #[test]
    fn words_are_golay_codewords() {
        assert!(STANDARD.windows(2).all(|w| w[0] < w[1]));
        // Every word is a multiple of the generator: the remainder of the whole 23-bit polynomial (data in the high part) is 0.
        for v in 0..=0o777 {
            let w = Dcs(v).word();
            let mut r = ((w & 0xFFF) << 11) | ((w >> 12) & 0x7FF);
            for i in (11..23).rev() {
                if r & (1 << i) != 0 {
                    r ^= GOLAY << (i - 11);
                }
            }
            assert_eq!(r, 0, "{v:o}");
        }
        assert_eq!(d("023").word(), 0x76_3813);
    }

    #[test]
    fn published_inverted_pairs() {
        // Pairs printed in radio manuals: transmitting the first code inverted is received as the second at normal polarity.
        for (a, b) in [
            ("023", "047"),
            ("025", "244"),
            ("026", "464"),
            ("114", "712"),
            ("116", "754"),
            ("032", "051"),
        ] {
            assert_eq!(d(a).inverted_alias(), Some(d(b)), "{a}");
            assert_eq!(d(b).inverted_alias(), Some(d(a)), "{b}");
        }
        // Every standard code's inverted word reads as some code at normal polarity; for all but one it is another standard
        // code. The exception is 172, whose inverted word reads as 036, a code outside the 83.
        let mut outside = Vec::new();
        for &c in STANDARD.iter() {
            let a = Dcs(c).inverted_alias().unwrap();
            if !a.is_standard() {
                outside.push((Dcs(c), a));
            }
        }
        assert_eq!(outside, [(d("172"), d("036"))]);
    }

    #[test]
    fn decoding_any_rotation_and_polarity() {
        for v in 0..=0o777u16 {
            let w = Dcs(v).word();
            for r in [0, 5, 11, 22] {
                let got = decode(rotate(w, r)).unwrap();
                assert!(!got.inverted, "{v:o}");
            }
        }
        let r = decode(!d("114").word() & MASK23).unwrap();
        assert_eq!(
            (r.code, r.inverted),
            (d("712"), false),
            "inverted 114 is read as 712 first"
        );
        assert_eq!(r.alias, Some((d("114"), true)));
        assert_eq!(r.name(), "D712N");
        assert_eq!(decode(0), None);
        assert_eq!(decode(d("023").word() ^ 0x100), None);
    }

    #[test]
    fn written_forms() {
        assert_eq!(d("D754").value(), 0o754);
        assert_eq!(d("d023"), d("023"));
        for bad in ["", "23", "0238", "089", "D0233", "x23"] {
            assert_eq!(Dcs::from_octal(bad), None, "{bad:?}");
        }
        assert_eq!(Dcs::new(0o1000), None);
        assert!(d("023").is_standard() && !d("024").is_standard());
    }
}
