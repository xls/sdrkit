//! The Golay sequential code: the Golay (23,12) words and their bit order, the preamble, start code and Word 1 tables, the
//! pager code, the BCH (15,7) data blocks, the character sets and the bit patterns of the 600 bit/s grid.
//!
//! A word is held as `u32` with sent bit `k` (0 the first sent) in bit `k`: `d0 .. d11` in bits 0 to 11, `p0 .. p10` in bits 12 to
//! 22. Such a word is a cyclic shift of the code word `x^11 d(x) + p(x)` of the generator `0xC75`, so the shared Golay (23,12)
//! code decodes it as it is.

use sdrkit_algo_fec::small::{Chase, SmallCode};
use std::sync::OnceLock;

/// All 23 bits of a word.
pub const MASK: u32 = 0x7F_FFFF;
/// Grid bits of a block (600 bit/s).
pub const BLOCK_BITS: usize = 121;
/// Grid bits of a comma.
pub const COMMA_BITS: usize = 28;
/// The grid bit of Word 1 in a two-word block.
pub const WORD1_AT: usize = 28;
/// The separator bit.
pub const SEPARATOR_AT: usize = 74;
/// The grid bit of Word 2.
pub const WORD2_AT: usize = 75;
/// Copies of the preamble word.
pub const PREAMBLE_COPIES: usize = 18;
/// The start code value.
pub const START_CODE: u16 = 713;
/// The activation word of the test signal; the real value is not published.
pub const TEST_ACTIVATION: u16 = 3061;
/// The deactivation word of the test signal; the real value is not published.
pub const TEST_DEACTIVATION: u16 = 2457;
/// The ten preamble values (index 0 to 9).
pub const PREAMBLE: [u16; 10] = [2030, 1628, 3198, 647, 191, 3315, 1949, 2540, 1560, 2335];
/// The Word 1 values, by G mod 50.
pub const WORD1: [u16; 50] = [
    721, 2731, 2952, 1387, 1578, 1708, 2650, 1747, 2580, 1376, 2692, 696, 1667, 3800, 3552, 3424,
    1384, 3595, 876, 3124, 2285, 2608, 899, 3684, 3129, 2124, 1287, 2616, 1647, 3216, 375, 1232,
    2824, 1840, 408, 3127, 3387, 882, 3468, 3267, 1575, 3463, 3152, 2572, 1252, 2592, 1552, 835,
    1440, 160,
];
/// The illegal addresses A for G below 50.
pub const ILLEGAL_LOW: [u16; 16] = [
    0, 25, 51, 103, 206, 340, 363, 412, 445, 530, 642, 726, 782, 810, 825, 877,
];
/// The illegal addresses A for G of 50 and above.
pub const ILLEGAL_HIGH: [u16; 7] = [0, 292, 425, 584, 631, 841, 851];
/// The most blocks of a data page.
pub const MAX_DATA_BLOCKS: usize = 20;
/// Bits of the character field of a data block.
pub const FIELD_BITS: u32 = 49;

/// The shared Golay (23,12) code.
pub fn golay() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(SmallCode::golay_23_12)
}

/// The BCH (15,7) code of the data blocks: generator x^8 + x^7 + x^6 + x^4 + 1, information bit `i0` sent first.
pub fn bch() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(|| SmallCode::cyclic(15, 0x1D1).expect("a valid BCH generator"))
}

/// The word of the 12-bit value `d`.
pub fn encode(d: u16) -> u32 {
    let d = u32::from(d & 0xFFF);
    let parity = golay().encode(u128::from(d)) as u32 & 0x7FF;
    d | parity << 12
}

/// The 12-bit value of a word.
pub fn value(w: u32) -> u16 {
    (w & 0xFFF) as u16
}

/// The complement of a word.
pub fn inverse(w: u32) -> u32 {
    !w & MASK
}

/// The number of bits in which two words differ.
pub fn distance(a: u32, b: u32) -> u32 {
    ((a ^ b) & MASK).count_ones()
}

/// The hard word of 23 soft bits (positive is 1).
pub fn hard(soft: &[f32; 23]) -> u32 {
    let mut w = 0;
    for (k, &s) in soft.iter().enumerate() {
        if s > 0.0 {
            w |= 1 << k;
        }
    }
    w
}

/// Soft decoding of a word: the code word nearest the soft bits (positive is 1) and the number of bits it changes. Every
/// word decodes (the code is perfect); the caller limits the corrections.
pub fn decode_soft(soft: &[f32; 23]) -> (u32, u32) {
    let w = hard(soft);
    // The shared code holds the first sent bit in its most significant place: sent bit k is place k here, so its
    // reliability is entry 22 - k.
    let mut rel = [0f32; 23];
    for (k, &s) in soft.iter().enumerate() {
        rel[22 - k] = s.abs();
    }
    let chase = Chase {
        weakest: 7,
        max_flips: 3,
    };
    match golay().decode_soft(u128::from(w), &rel, chase) {
        Some((cw, changed, _)) => (cw as u32, u32::from(changed)),
        None => {
            let (cw, changed) = golay()
                .decode(u128::from(w))
                .expect("the Golay code decodes every word");
            (cw as u32, u32::from(changed))
        }
    }
}

/// The code word of the BCH information number `i` (0 to 127): bit `j` holds `c_j`; `c14` (= `i0`) is sent first.
pub fn bch_encode(i: u8) -> u16 {
    let i = i & 0x7F;
    let rev = (i.reverse_bits() >> 1) & 0x7F;
    bch().encode(u128::from(rev)) as u16
}

/// The BCH code word of every information number.
pub fn bch_words() -> &'static [u16; 128] {
    static WORDS: OnceLock<[u16; 128]> = OnceLock::new();
    WORDS.get_or_init(|| std::array::from_fn(|i| bch_encode(i as u8)))
}

/// The information number of a BCH word decided by soft correlation against all 128 code words, and the number of its bits
/// that disagree with the hard decisions. `soft[k]` is code bit `c_(14-k)` in sent order, positive for 1.
pub fn bch_decide(soft: &[f32; 15]) -> (u8, u32) {
    let words = bch_words();
    let mut best = (0usize, f32::NEG_INFINITY);
    for (i, &w) in words.iter().enumerate() {
        let mut c = 0f32;
        for (k, &s) in soft.iter().enumerate() {
            if w >> (14 - k) & 1 != 0 {
                c += s;
            } else {
                c -= s;
            }
        }
        if c > best.1 {
            best = (i, c);
        }
    }
    let w = words[best.0];
    let mut wrong = 0;
    for (k, &s) in soft.iter().enumerate() {
        if (w >> (14 - k) & 1 != 0) != (s > 0.0) {
            wrong += 1;
        }
    }
    (best.0 as u8, wrong)
}

/// A pager code: the digit I (unknown without a preamble), G (0 to 99) and A (0 to 999).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PagerCode {
    /// The first digit.
    pub i: Option<u8>,
    /// The group digits G1 G0.
    pub g: u8,
    /// The address digits A2 A1 A0.
    pub a: u16,
}

impl PagerCode {
    /// The code of six digits.
    pub fn from_digits(code: u32) -> PagerCode {
        PagerCode {
            i: Some((code / 100_000 % 10) as u8),
            g: (code / 1000 % 100) as u8,
            a: (code % 1000) as u16,
        }
    }

    /// The preamble index (I + G0) mod 10; `None` without I.
    pub fn preamble_index(&self) -> Option<usize> {
        self.i.map(|i| usize::from((i + self.g % 10) % 10))
    }

    /// The Word 1 value.
    pub fn word1(&self) -> u16 {
        WORD1[usize::from(self.g % 50)]
    }

    /// The Word 2 value (0 to 1999).
    pub fn word2(&self) -> u16 {
        100 * (self.a / 50) + self.a % 50 + if self.g >= 50 { 50 } else { 0 }
    }

    /// Whether A is a legal address of its G.
    pub fn is_legal(&self) -> bool {
        self.a <= 999
            && if self.g < 50 {
                !ILLEGAL_LOW.contains(&self.a)
            } else {
                !ILLEGAL_HIGH.contains(&self.a)
            }
    }

    /// G and A from the Word 1 index and the Word 2 value, with I from a preamble index; `None` when the address is not legal.
    pub fn from_words(
        word1_index: usize,
        word2: u16,
        preamble: Option<usize>,
    ) -> Option<PagerCode> {
        if word1_index >= 50 || word2 >= 2000 {
            return None;
        }
        let low = word2 % 100;
        let g = word1_index as u8 + if low >= 50 { 50 } else { 0 };
        let a = 50 * (word2 / 100) + low % 50;
        let i = preamble.map(|p| ((p + 10 - usize::from(g % 10)) % 10) as u8);
        let c = PagerCode { i, g, a };
        c.is_legal().then_some(c)
    }

    /// The code as shown: six digits, or N and five digits without a preamble.
    pub fn text(&self) -> String {
        match self.i {
            Some(i) => format!("{i}{:02}{:03}", self.g, self.a),
            None => format!("N{:05}", u32::from(self.g) * 1000 + u32::from(self.a)),
        }
    }
}

/// The Word 1 index of a hard word: the table entry within `max` bits, and whether the word is inverted.
pub fn word1_index(w: u32, max: u32) -> Option<(usize, bool, u32)> {
    let table = word1_words();
    let mut best: Option<(usize, bool, u32)> = None;
    for (i, &t) in table.iter().enumerate() {
        let d = distance(w, t);
        let di = 23 - d;
        for (inv, dd) in [(false, d), (true, di)] {
            if dd <= max && best.is_none_or(|b| dd < b.2) {
                best = Some((i, inv, dd));
            }
        }
    }
    best
}

/// The words of the Word 1 table.
pub fn word1_words() -> &'static [u32; 50] {
    static WORDS: OnceLock<[u32; 50]> = OnceLock::new();
    WORDS.get_or_init(|| std::array::from_fn(|i| encode(WORD1[i])))
}

/// The words of the preamble table.
pub fn preamble_words() -> &'static [u32; 10] {
    static WORDS: OnceLock<[u32; 10]> = OnceLock::new();
    WORDS.get_or_init(|| std::array::from_fn(|i| encode(PREAMBLE[i])))
}

/// The start code word.
pub fn start_word() -> u32 {
    encode(START_CODE)
}

/// Whether a word lies within `max` bits of a known word (a preamble word, a Word 1 table word or the start code) or of
/// its inverse.
pub fn near_known(w: u32, max: u32) -> bool {
    let near = |k: u32| {
        let d = distance(w, k);
        d <= max || 23 - d <= max
    };
    near(start_word())
        || preamble_words().iter().any(|&k| near(k))
        || word1_words().iter().any(|&k| near(k))
}

/// The address function from the senses of the two words: 1 normal and normal, 2 normal and inverted, 3 inverted and
/// normal, 4 inverted and inverted.
pub fn function(word1_inverted: bool, word2_inverted: bool) -> u8 {
    match (word1_inverted, word2_inverted) {
        (false, false) => 1,
        (false, true) => 2,
        (true, false) => 3,
        (true, true) => 4,
    }
}

/// The senses of the two words of function `f` (1 to 4).
pub fn function_senses(f: u8) -> (bool, bool) {
    match f {
        2 => (false, true),
        3 => (true, false),
        4 => (true, true),
        _ => (false, false),
    }
}

/// Appends a comma of 28 alternating grid bits whose first bit is `first`.
pub fn push_comma(first: u8, out: &mut Vec<u8>) {
    for k in 0..COMMA_BITS {
        out.push((first ^ (k as u8 & 1)) & 1);
    }
}

/// Appends a word at 300 bit/s: each bit twice on the grid.
pub fn push_slow(w: u32, out: &mut Vec<u8>) {
    for k in 0..23 {
        let b = (w >> k & 1) as u8;
        out.push(b);
        out.push(b);
    }
}

/// Appends a two-word block: comma, Word 1, separator, Word 2.
pub fn push_two_words(w1: u32, w2: u32, out: &mut Vec<u8>) {
    push_comma((w1 & 1) as u8, out);
    push_slow(w1, out);
    out.push(((w2 & 1) as u8) ^ 1);
    push_slow(w2, out);
}

/// Appends the preamble: a comma and the word 18 times.
pub fn push_preamble(w: u32, out: &mut Vec<u8>) {
    push_comma((w & 1) as u8, out);
    for _ in 0..PREAMBLE_COPIES {
        push_slow(w, out);
    }
}

/// The 8 information numbers of a block of the 49-bit field `p` (characters and the continue bit in bit 48).
pub fn block_info(p: u64) -> [u8; 8] {
    let mut info = [0u8; 8];
    let mut sum = 0u32;
    for (m, slot) in info.iter_mut().take(7).enumerate() {
        *slot = (p >> (7 * m) & 0x7F) as u8;
        sum += u32::from(*slot);
    }
    info[7] = (sum % 128) as u8;
    info
}

/// The 49-bit field of 7 information numbers, if the sum check of the eighth holds.
pub fn block_field(info: &[u8; 8]) -> Option<u64> {
    let sum: u32 = info[..7].iter().map(|&v| u32::from(v)).sum();
    if sum % 128 != u32::from(info[7]) {
        return None;
    }
    let mut p = 0u64;
    for (m, &v) in info[..7].iter().enumerate() {
        p |= u64::from(v & 0x7F) << (7 * m);
    }
    Some(p)
}

/// Appends the 121 grid bits of a data block: the comma bit, then the 8 BCH words by columns, `c14` of every word first.
pub fn push_data_block(info: &[u8; 8], out: &mut Vec<u8>) {
    let words: [u16; 8] = std::array::from_fn(|w| bch_encode(info[w]));
    out.push(((words[0] >> 14) as u8 & 1) ^ 1);
    for j in (0..15).rev() {
        for w in words {
            out.push((w >> j & 1) as u8);
        }
    }
}

/// Reads a data block from 121 soft grid bits: the 8 information numbers, or `None` when the comma bit equals the first
/// data bit or a word has more than `max_wrong` hard disagreements with its chosen code word. Returns also the
/// disagreements in all.
pub fn read_data_block(soft: &[f32], max_wrong: u32) -> Option<([u8; 8], u32)> {
    if (soft[0] > 0.0) == (soft[1] > 0.0) {
        return None;
    }
    let mut info = [0u8; 8];
    let mut wrong_all = 0;
    for (w, slot) in info.iter_mut().enumerate() {
        let bits: [f32; 15] = std::array::from_fn(|k| soft[1 + 8 * k + w]);
        let (i, wrong) = bch_decide(&bits);
        if wrong > max_wrong {
            return None;
        }
        wrong_all += wrong;
        *slot = i;
    }
    Some((info, wrong_all))
}

/// Characters of a numeric block.
pub const NUMERIC_CHARS: usize = 12;
/// Characters of an alphanumeric block.
pub const ALPHA_CHARS: usize = 8;
/// The numeric fill character.
pub const NUMERIC_NULL: u8 = 10;
/// The alphanumeric fill character.
pub const ALPHA_NULL: u8 = 0x3E;

/// The numeric code of a character: digits, `U`, space, `-`, `=`, `E`; others are a space.
pub fn numeric_code(c: char) -> u8 {
    match c {
        '0'..='9' => c as u8 - b'0',
        'U' => 11,
        '-' => 13,
        '=' => 14,
        'E' => 15,
        _ => 12,
    }
}

/// The character of a numeric code; `None` for the fill.
pub fn numeric_char(v: u8) -> Option<char> {
    match v & 0xF {
        d @ 0..=9 => Some((b'0' + d) as char),
        10 => None,
        11 => Some('U'),
        12 => Some(' '),
        13 => Some('-'),
        14 => Some('='),
        _ => Some('E'),
    }
}

/// The 6-bit code of a character: lower-case ASCII with bit 6 dropped; new line is 3C.
pub fn alpha_code(c: char) -> u8 {
    if c == '\n' {
        return 0x3C;
    }
    let b = c.to_ascii_lowercase() as u32;
    match b {
        0x20..=0x3F => (b - 0x20) as u8,
        0x60..=0x7D => (b - 0x60 + 0x20) as u8,
        _ => 0x00,
    }
}

/// The character of a 6-bit code; `None` for the fill and the spare code.
pub fn alpha_char(v: u8) -> Option<char> {
    match v & 0x3F {
        0x3C => Some('\n'),
        0x3E | 0x3F => None,
        0x20 => Some('\\'),
        v if v < 0x20 => Some((0x20 + v) as char),
        v => Some(((0x60 + v - 0x20) as char).to_ascii_uppercase()),
    }
}

/// The 49-bit fields of a text: characters of `bits` bits (4 numeric, 6 alphanumeric), the fill after the last, the
/// continue bit set on every block but the last.
pub fn text_fields(text: &str, numeric: bool) -> Vec<u64> {
    let (bits, per, null) = if numeric {
        (4, NUMERIC_CHARS, NUMERIC_NULL)
    } else {
        (6, ALPHA_CHARS, ALPHA_NULL)
    };
    let codes: Vec<u8> = text
        .chars()
        .map(|c| {
            if numeric {
                numeric_code(c)
            } else {
                alpha_code(c)
            }
        })
        .collect();
    let blocks = codes.len().div_ceil(per).max(1);
    (0..blocks)
        .map(|b| {
            let mut p = 0u64;
            for k in 0..per {
                let v = codes.get(b * per + k).copied().unwrap_or(null);
                p |= u64::from(v) << (k * bits);
            }
            if b + 1 < blocks {
                p |= 1 << 48;
            }
            p
        })
        .collect()
}

/// Whether a 49-bit field says that another block follows.
pub fn continues(p: u64) -> bool {
    p >> 48 & 1 != 0
}

/// The numeric and the alphanumeric reading of the fields of a page.
pub fn readings(fields: &[u64]) -> (String, String) {
    let mut numeric = String::new();
    let mut alpha = String::new();
    for &p in fields {
        for k in 0..NUMERIC_CHARS {
            if let Some(c) = numeric_char((p >> (4 * k)) as u8 & 0xF) {
                numeric.push(c);
            }
        }
        for k in 0..ALPHA_CHARS {
            if let Some(c) = alpha_char((p >> (6 * k)) as u8 & 0x3F) {
                alpha.push(c);
            }
        }
    }
    (numeric.trim_end().to_string(), alpha.trim_end().to_string())
}

/// The share of plausible characters of a reading (0 for an empty one).
pub fn plausible(text: &str, numeric: bool) -> f64 {
    let total = text.chars().count();
    if total == 0 {
        return 0.0;
    }
    let good = text
        .chars()
        .filter(|&c| {
            if numeric {
                c.is_ascii_digit() || c == ' ' || c == '-'
            } else {
                c.is_ascii_uppercase() || c.is_ascii_digit() || " .,?!-:/\n".contains(c)
            }
        })
        .count();
    good as f64 / total as f64
}

/// Whether the last field ends with the fill character of a reading (numeric 10, alphanumeric 3E): the sender fills the
/// rest of a short last block with its own fill, so the other reading seldom ends with one.
fn ends_with_fill(fields: &[u64], numeric: bool) -> bool {
    let Some(&p) = fields.last() else {
        return false;
    };
    if numeric {
        (p >> (4 * (NUMERIC_CHARS - 1))) as u8 & 0xF == NUMERIC_NULL
    } else {
        (p >> (6 * (ALPHA_CHARS - 1))) as u8 & 0x3F == ALPHA_NULL
    }
}

/// The reading of a data page: numeric (true) or alphanumeric, and its text. Each reading scores its share of plausible
/// characters, and 0.2 more when its last block ends with its own fill. The higher score wins, numeric on a tie; when the
/// two lie within 0.15, the other reading follows in brackets.
pub fn choose_reading(fields: &[u64]) -> (bool, String) {
    let (numeric, alpha) = readings(fields);
    let bonus = |n: bool| if ends_with_fill(fields, n) { 0.2 } else { 0.0 };
    let sn = plausible(&numeric, true) + bonus(true);
    let sa = plausible(&alpha, false) + bonus(false);
    let is_numeric = sn >= sa;
    let (main, other) = if is_numeric {
        (numeric, alpha)
    } else {
        (alpha, numeric)
    };
    if (sn - sa).abs() < 0.15 && !other.is_empty() && other != main {
        (is_numeric, format!("{main} [{other}]"))
    } else {
        (is_numeric, main)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_text(w: u32) -> String {
        (0..23)
            .map(|k| if w >> k & 1 != 0 { '1' } else { '0' })
            .collect()
    }

    #[test]
    fn the_preamble_parities_are_table_v() {
        let parity = [
            "10000000011",
            "00001111110",
            "11000001001",
            "01111111000",
            "00001111010",
            "00000111001",
            "00011110000",
            "01000001111",
            "01111111001",
            "11100010001",
        ];
        for (d, p) in PREAMBLE.iter().zip(parity) {
            let w = encode(*d);
            assert_eq!(format!("{:011b}", w >> 12), p, "{d}");
        }
        assert_eq!(format!("{:011b}", start_word() >> 12), "01000000011");
        assert_eq!(bits_text(start_word()), "10010011010011000000010");
        assert_eq!(bits_text(encode(647)), "11100001010000011111110");
    }

    #[test]
    fn the_worked_example_954853() {
        let c = PagerCode::from_digits(954_853);
        assert_eq!(c.preamble_index(), Some(3));
        assert_eq!(c.word1(), 1578);
        assert_eq!(c.word2(), 1753);
        assert_eq!(format!("{:011b}", encode(1753) >> 12), "00010101101");
        assert_eq!(bits_text(encode(1578)), "01010100011011001101101");
        assert_eq!(bits_text(encode(1753)), "10011011011010110101000");
        let back = PagerCode::from_words(4, 1753, Some(3)).unwrap();
        assert_eq!(back.text(), "954853");
        let c = PagerCode::from_digits(123_456);
        assert_eq!(c.preamble_index(), Some(4));
        assert_eq!(c.word1(), 3684);
        assert_eq!(bits_text(encode(3684)), "00100110011101001110010");
        assert_eq!(c.word2(), 906);
    }

    #[test]
    fn every_pager_code_maps_back_and_the_illegal_ones_are_refused() {
        for code in 0..1_000_000u32 {
            let c = PagerCode::from_digits(code);
            if !c.is_legal() {
                continue;
            }
            let idx = WORD1.iter().position(|&v| v == c.word1()).unwrap();
            let back = PagerCode::from_words(idx, c.word2(), c.preamble_index()).unwrap();
            assert_eq!(back, c, "{code}");
        }
        assert!(!PagerCode::from_digits(10_363).is_legal());
        assert!(!PagerCode::from_digits(51_292).is_legal());
        assert!(PagerCode::from_digits(10_362).is_legal());
        assert_eq!(
            PagerCode {
                i: None,
                g: 54,
                a: 853
            }
            .text(),
            "N54853"
        );
    }

    #[test]
    fn the_codes_have_their_distances() {
        let g = golay();
        let mut min = u32::MAX;
        for d in 1..4096u128 {
            min = min.min((g.encode(d) as u32).count_ones());
        }
        assert_eq!(min, 7);
        let mut min = u32::MAX;
        for i in 1..128u8 {
            min = min.min(u32::from(bch_encode(i)).count_ones());
        }
        assert_eq!(min, 5);
        // The complement of a word is the word of the complemented value.
        assert_eq!(inverse(encode(1753)), encode(!1753 & 0xFFF));
        // The known words lie 7 bits or more from the stand-in activation and deactivation words.
        for w in [TEST_ACTIVATION, TEST_DEACTIVATION] {
            assert!(!near_known(encode(w), 6), "{w}");
        }
    }

    #[test]
    fn bch_information_round_trips_and_soft_decisions_repair() {
        assert_eq!(format!("{:015b}", bch_encode(85)), "101010111100101");
        assert_eq!(format!("{:015b}", bch_encode(72)), "000100111001100");
        for i in 0..128u8 {
            let w = bch_encode(i);
            let mut soft: [f32; 15] =
                std::array::from_fn(|k| if w >> (14 - k) & 1 != 0 { 1.0 } else { -1.0 });
            soft[3] = -soft[3] * 0.2;
            soft[9] = -soft[9] * 0.3;
            assert_eq!(bch_decide(&soft), (i, 2));
        }
    }

    fn block_text(info: &[u8; 8]) -> String {
        let mut v = Vec::new();
        push_data_block(info, &mut v);
        v.iter().map(|b| if *b != 0 { '1' } else { '0' }).collect()
    }

    #[test]
    fn the_worked_data_blocks() {
        let f = text_fields("5551234", true);
        assert_eq!(f.len(), 1);
        let info = block_info(f[0]);
        assert_eq!(info, [85, 42, 72, 33, 42, 85, 42, 17]);
        assert_eq!(
            block_text(&info),
            "0100101010100101010000100011010101000010101011010101001001011010010100100100001000101101001111011101001010101101110010100"
        );
        let f = text_fields("CALL HQ", false);
        let info = block_info(f[0]);
        assert_eq!(info, [99, 16, 75, 5, 32, 99, 62, 4]);
        assert_eq!(
            block_text(&info),
            "0101101001010011000010011001000100100001010001110101001000101001001111010110011011011110110000101110001101101111100001000"
        );
        assert_eq!(alpha_code('A'), 0x21);
        assert_eq!(alpha_code(' '), 0x00);
        assert_eq!(alpha_code('0'), 0x10);
        assert_eq!(
            choose_reading(&text_fields("5551234", true)),
            (true, "5551234".into())
        );
        assert_eq!(
            choose_reading(&text_fields("CALL HQ", false)),
            (false, "CALL HQ".into())
        );
    }

    #[test]
    fn corrupted_checks_and_illegal_addresses_are_refused() {
        let mut info = block_info(text_fields("5551234", true)[0]);
        info[7] ^= 1;
        assert_eq!(block_field(&info), None);
        info[7] ^= 1;
        let mut bits = Vec::new();
        push_data_block(&info, &mut bits);
        let soft: Vec<f32> = bits
            .iter()
            .map(|&b| if b != 0 { 1.0 } else { -1.0 })
            .collect();
        // Three hard errors in BCH word 1 (bits c14, c13 and c12 of word 1).
        let mut three = soft.clone();
        for k in [1, 9, 17] {
            three[k] = -three[k];
        }
        assert!(read_data_block(&three, 2).is_none());
        // The comma bit equal to the first data bit.
        let mut comma = soft.clone();
        comma[0] = -comma[0];
        assert!(read_data_block(&comma, 2).is_none());
        assert!(read_data_block(&soft, 2).is_some());
        // 954851: A = 851 is illegal for G of 50 and above.
        let c = PagerCode::from_digits(954_851);
        assert!(PagerCode::from_words(4, c.word2(), Some(3)).is_none());
    }

    #[test]
    fn blocks_and_text_round_trip() {
        let text = "THE QUICK BROWN FOX, 12:30 / OK?";
        let fields = text_fields(text, false);
        assert_eq!(fields.len(), 4);
        let mut back = Vec::new();
        for &p in &fields {
            let mut bits = Vec::new();
            push_data_block(&block_info(p), &mut bits);
            let soft: Vec<f32> = bits
                .iter()
                .map(|&b| if b != 0 { 1.0 } else { -1.0 })
                .collect();
            let (info, wrong) = read_data_block(&soft, 0).unwrap();
            assert_eq!(wrong, 0);
            back.push(block_field(&info).unwrap());
        }
        assert_eq!(back, fields);
        assert!(continues(back[0]) && !continues(back[3]));
        assert_eq!(readings(&back).1, text);
        let n = text_fields("0123456789 U-=E99", true);
        assert_eq!(readings(&n).0, "0123456789 U-=E99");
    }
}
