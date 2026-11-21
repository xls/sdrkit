//! The ERMES air interface as the stack reads it: the code words, the special words, the batch layout, the codeblock
//! interleave, the system information and the messages of the message partition.
//!
//! Standard: ETS 300 133-4 (air interface), ETS 300 133-2 annex B (numeric characters).

use sdrkit_algo_fec::small::SmallCode;
use std::sync::OnceLock;

/// The symbol rate, symbols/s.
pub const SYMBOL_RATE: f64 = 3125.0;
/// The four levels, Hz, of the symbols 00, 01, 11 and 10 (Gray).
pub const LEVELS_HZ: [f64; 4] = [-4687.5, -1562.5, 1562.5, 4687.5];
/// The bits of a code word.
pub const WORD_BITS: u32 = 30;
/// The symbols of a code word.
pub const WORD_SYMBOLS: usize = 15;
/// The preamble word.
pub const PREAMBLE: u32 = 0b001000100010001000100010001000;
/// The sync word.
pub const SYNC: u32 = 0b100010100010000010100000101010;
/// The address partition terminator.
pub const APT: u32 = 0b100100111000011000100011100100;
/// The message delimiter.
pub const DELIMITER: u32 = 0b110101011110011111101110111011;
/// The mean level of the preamble word, Hz.
pub const PREAMBLE_MEAN_HZ: f64 = -312.5;
/// Words of a batch, preamble and sync included.
pub const BATCH_WORDS: usize = 154;
/// Words of the last batch of a subsequence (batch P).
pub const LAST_BATCH_WORDS: usize = 190;
/// The number of the last batch of a subsequence.
pub const LAST_BATCH: u8 = 15;
/// Words of a codeblock.
pub const CODEBLOCK: usize = 9;
/// A special word within this many bits is taken.
pub const SPECIAL_TOLERANCE: u32 = 2;

/// The (30,18) code, made once.
pub fn code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(SmallCode::ermes_30_18)
}

/// The code word of 18 information bits.
pub fn encode(info: u32) -> u32 {
    code().encode(u128::from(info & 0x3FFFF)) as u32
}

/// The information bits of a received word and the bits repaired, or `None` when the code refuses it.
pub fn decode(word: u32) -> Option<(u32, u8)> {
    let (w, n) = code().decode(u128::from(word))?;
    Some(((code().info(w)) as u32, n))
}

/// `word` is within [`SPECIAL_TOLERANCE`] bits of the special word `special`.
pub fn is_special(word: u32, special: u32) -> bool {
    (word ^ special).count_ones() <= SPECIAL_TOLERANCE
}

/// The Gray symbol (0 to 3: bits 00, 01, 11, 10 in level order) of a level, Hz, with the decisions at ±3125 Hz and 0.
pub fn level_index(hz: f32) -> usize {
    if hz < -3125.0 {
        0
    } else if hz < 0.0 {
        1
    } else if hz < 3125.0 {
        2
    } else {
        3
    }
}

/// The two bits of level index `i` (00, 01, 11, 10).
pub fn level_bits(i: usize) -> u32 {
    [0b00, 0b01, 0b11, 0b10][i & 3]
}

/// The level index of two bits.
pub fn bits_level(b: u32) -> usize {
    match b & 3 {
        0b00 => 0,
        0b01 => 1,
        0b11 => 2,
        _ => 3,
    }
}

/// The 15 level indices of a word, first sent first (bits 29 and 28 first).
pub fn word_levels(word: u32) -> [usize; WORD_SYMBOLS] {
    std::array::from_fn(|s| bits_level(word >> (WORD_BITS as usize - 2 - 2 * s)))
}

/// The nine sent words of nine code words: their bits column by column (bit 29 of each word, then bit 28, and so on).
pub fn interleave(rows: &[u32; CODEBLOCK]) -> [u32; CODEBLOCK] {
    let mut sent = [0u32; CODEBLOCK];
    for c in 0..WORD_BITS as usize {
        for (r, row) in rows.iter().enumerate() {
            let k = c * CODEBLOCK + r;
            let bit = (row >> (WORD_BITS as usize - 1 - c)) & 1;
            sent[k / 30] |= bit << (29 - k % 30);
        }
    }
    sent
}

/// The nine code words of nine sent words.
pub fn deinterleave(sent: &[u32; CODEBLOCK]) -> [u32; CODEBLOCK] {
    let mut rows = [0u32; CODEBLOCK];
    for c in 0..WORD_BITS as usize {
        for (r, row) in rows.iter_mut().enumerate() {
            let k = c * CODEBLOCK + r;
            let bit = (sent[k / 30] >> (29 - k % 30)) & 1;
            *row |= bit << (WORD_BITS as usize - 1 - c);
        }
    }
    rows
}

/// The system information of a batch: SI 1, SI 2 and the supplementary word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemInfo {
    /// Country code, 7 bits.
    pub country: u8,
    /// Operator code, 3 bits.
    pub operator: u8,
    /// Paging area, 6 bits.
    pub area: u8,
    /// ETI bit.
    pub eti: bool,
    /// BAI bit.
    pub bai: bool,
    /// FSI, 5 bits.
    pub fsi: u8,
    /// Cycle number, 6 bits.
    pub cycle: u8,
    /// Subsequence number, 3 bits.
    pub subsequence: u8,
    /// Batch number, 0 (A) to 15 (P).
    pub batch: u8,
    /// SSI type, 4 bits.
    pub ssi_type: u8,
    /// SSI field, 14 bits.
    pub ssi_field: u16,
}

impl SystemInfo {
    /// The information bits of SI 1, SI 2 and SSI.
    pub fn words(&self) -> [u32; 3] {
        let si1 = (u32::from(self.country & 0x7F) << 11)
            | (u32::from(self.operator & 7) << 8)
            | (u32::from(self.area & 0x3F) << 2)
            | (u32::from(self.eti) << 1)
            | u32::from(self.bai);
        let si2 = (u32::from(self.fsi & 0x1F) << 13)
            | (u32::from(self.cycle & 0x3F) << 7)
            | (u32::from(self.subsequence & 7) << 4)
            | u32::from(self.batch & 0xF);
        let ssi = (u32::from(self.ssi_type & 0xF) << 14) | u32::from(self.ssi_field & 0x3FFF);
        [si1, si2, ssi]
    }

    /// The system information of the information bits of SI 1, SI 2 and SSI.
    pub fn from_words(w: [u32; 3]) -> SystemInfo {
        SystemInfo {
            country: ((w[0] >> 11) & 0x7F) as u8,
            operator: ((w[0] >> 8) & 7) as u8,
            area: ((w[0] >> 2) & 0x3F) as u8,
            eti: (w[0] >> 1) & 1 == 1,
            bai: w[0] & 1 == 1,
            fsi: ((w[1] >> 13) & 0x1F) as u8,
            cycle: ((w[1] >> 7) & 0x3F) as u8,
            subsequence: ((w[1] >> 4) & 7) as u8,
            batch: (w[1] & 0xF) as u8,
            ssi_type: ((w[2] >> 14) & 0xF) as u8,
            ssi_field: (w[2] & 0x3FFF) as u16,
        }
    }

    /// The batch letter, A to P.
    pub fn batch_letter(&self) -> char {
        char::from(b'A' + (self.batch & 0xF))
    }

    /// The SSI type 0 fields: zone, hour, day.
    pub fn zone_hour_day(&self) -> Option<(u8, u8, u8)> {
        (self.ssi_type == 0).then(|| {
            let f = self.ssi_field;
            (
                ((f >> 11) & 7) as u8,
                ((f >> 6) & 0x1F) as u8,
                ((f >> 1) & 0x1F) as u8,
            )
        })
    }

    /// The SSI type 0 field of a zone, hour and day.
    pub fn ssi_time(zone: u8, hour: u8, day: u8) -> u16 {
        (u16::from(zone & 7) << 11) | (u16::from(hour & 0x1F) << 6) | (u16::from(day & 0x1F) << 1)
    }

    /// The words of the batch, preamble and sync included.
    pub fn batch_words(&self) -> usize {
        if self.batch == LAST_BATCH {
            LAST_BATCH_WORDS
        } else {
            BATCH_WORDS
        }
    }

    /// The network for people: "country 40 operator 2 area 5, cycle 12 subsequence 3 batch E, zone 3 hour 18 day 21".
    pub fn text(&self) -> String {
        let mut s = format!(
            "country {} operator {} area {}, cycle {} subsequence {} batch {}",
            self.country,
            self.operator,
            self.area,
            self.cycle,
            self.subsequence,
            self.batch_letter()
        );
        match self.zone_hour_day() {
            Some((z, h, d)) => s.push_str(&format!(", zone {z} hour {h} day {d}")),
            None => s.push_str(&format!(
                ", SSI type {} field {:04X}",
                self.ssi_type, self.ssi_field
            )),
        }
        s
    }
}

/// The APT words that follow `addresses` initial addresses: the message partition then starts on a multiple of 9 words
/// counted from the sync word.
pub fn apt_words(addresses: usize) -> usize {
    CODEBLOCK - (4 + addresses) % CODEBLOCK
}

/// The category of a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// A tone: no data.
    Tone,
    /// Numeric.
    Numeric,
    /// Alphanumeric.
    Alphanumeric,
    /// Transparent data.
    Transparent,
}

impl Category {
    /// The category of the two category bits.
    pub fn of(bits: u32) -> Category {
        match bits & 3 {
            0 => Category::Tone,
            1 => Category::Numeric,
            2 => Category::Alphanumeric,
            _ => Category::Transparent,
        }
    }
    /// The two category bits.
    pub fn bits(self) -> u32 {
        match self {
            Category::Tone => 0,
            Category::Numeric => 1,
            Category::Alphanumeric => 2,
            Category::Transparent => 3,
        }
    }
    /// The name for people.
    pub fn label(self) -> &'static str {
        match self {
            Category::Tone => "tone only",
            Category::Numeric => "numeric",
            Category::Alphanumeric => "alphanumeric",
            Category::Transparent => "transparent",
        }
    }
}

/// The end-of-message character of alphanumeric text.
pub const EOM: u8 = 0x11;

/// The two header words of a message to the 22-bit local `address`.
pub fn headers(address: u32, category: Category, aii: bool) -> [u32; 2] {
    let address = address & 0x3F_FFFF;
    [
        address >> 4,
        ((address & 0xF) << 14) | (u32::from(aii) << 7) | (category.bits() << 3),
    ]
}

/// The address, category and additional-information flag of the two header words.
pub fn read_headers(h1: u32, h2: u32) -> (u32, Category, bool) {
    (
        ((h1 & 0x3FFFF) << 4) | ((h2 >> 14) & 0xF),
        Category::of(h2 >> 3),
        (h2 >> 7) & 1 == 1,
    )
}

/// Packs `values` of `width` bits each, most significant first, into 18-bit data words, the last word filled with `fill`
/// values (cut at the word end).
pub fn pack(values: &[u32], width: u32, fill: u32) -> Vec<u32> {
    let mut bits: Vec<u8> = Vec::new();
    let put = |v: u32, bits: &mut Vec<u8>| {
        for i in (0..width).rev() {
            bits.push(((v >> i) & 1) as u8);
        }
    };
    for &v in values {
        put(v, &mut bits);
    }
    while !bits.len().is_multiple_of(18) {
        put(fill, &mut bits);
    }
    bits.truncate(bits.len() / 18 * 18);
    bits.chunks(18)
        .map(|c| c.iter().fold(0u32, |w, &b| (w << 1) | u32::from(b)))
        .collect()
}

/// The `width`-bit values of 18-bit data words, most significant first; leftover bits at the end are dropped.
pub fn unpack(words: &[u32], width: u32) -> Vec<u32> {
    let total = words.len() * 18;
    let bit = |k: usize| (words[k / 18] >> (17 - k % 18)) & 1;
    (0..total / width as usize)
        .map(|i| (0..width as usize).fold(0u32, |v, j| (v << 1) | bit(i * width as usize + j)))
        .collect()
}

/// The data words of numeric text: digits and the hex digits A to F as codes 10 to 15, filled with C.
pub fn numeric_words(text: &str) -> Vec<u32> {
    let codes: Vec<u32> = text.chars().filter_map(|c| c.to_digit(16)).collect();
    pack(&codes, 4, 0xC)
}

/// The data words of alphanumeric text, ended by the EOM character, which fills the last word.
pub fn alpha_words(text: &str) -> Vec<u32> {
    let mut codes: Vec<u32> = text.bytes().map(|b| u32::from(b & 0x7F)).collect();
    codes.push(u32::from(EOM));
    pack(&codes, 7, u32::from(EOM))
}

/// The text of the data words of a message.
pub fn message_text(category: Category, words: &[u32]) -> String {
    match category {
        Category::Tone => String::new(),
        Category::Numeric => {
            let digits: String = unpack(words, 4)
                .iter()
                .map(|&d| char::from_digit(d, 16).unwrap_or('?').to_ascii_uppercase())
                .collect();
            digits.trim_end_matches(['C', 'F']).to_string()
        }
        Category::Alphanumeric => unpack(words, 7)
            .into_iter()
            .take_while(|&c| c != u32::from(EOM))
            .map(|c| {
                if (0x20..=0x7E).contains(&c) {
                    char::from(c as u8)
                } else {
                    '.'
                }
            })
            .collect(),
        Category::Transparent => words
            .iter()
            .map(|w| format!("{w:05X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_words_of_the_examples() {
        assert_eq!(encode(1), 0b000000000000000001101011001101);
        assert_eq!(encode(0x3FFFF), 0b111111111111111111110001001011);
        assert_eq!(encode(0x12345), 0x1234_57F6);
        let w = encode(0x12345) ^ (1 << 3) ^ (1 << 20);
        assert_eq!(decode(w), Some((0x12345, 2)));
        assert_eq!(decode(w ^ (1 << 28)), None);
    }

    #[test]
    fn the_preamble_has_its_mean() {
        let l = word_levels(PREAMBLE);
        let mean = l.iter().map(|&i| LEVELS_HZ[i]).sum::<f64>() / 15.0;
        assert_eq!(mean, PREAMBLE_MEAN_HZ);
    }

    #[test]
    fn interleave_round_trip() {
        let rows: [u32; 9] = std::array::from_fn(|i| encode(0x1111 * i as u32 + 7));
        let sent = interleave(&rows);
        assert_eq!(deinterleave(&sent), rows);
        // The first sent word holds bit 29 of the nine rows, then bit 28.
        assert_eq!(
            sent[0] >> 21,
            rows.iter().fold(0, |w, r| (w << 1) | (r >> 29))
        );
    }

    #[test]
    fn texts() {
        assert_eq!(
            message_text(Category::Alphanumeric, &alpha_words("HELLO")),
            "HELLO"
        );
        assert_eq!(
            message_text(Category::Numeric, &numeric_words("1234567890")),
            "1234567890"
        );
        assert_eq!(apt_words(3), 2);
        assert_eq!(apt_words(5), 9);
        let (a, c, aii) = {
            let h = headers(0x2ABCD5, Category::Alphanumeric, false);
            read_headers(h[0], h[1])
        };
        assert_eq!((a, c, aii), (0x2ABCD5, Category::Alphanumeric, false));
    }

    #[test]
    fn system_information_round_trip() {
        let si = SystemInfo {
            country: 40,
            operator: 2,
            area: 5,
            cycle: 12,
            subsequence: 3,
            batch: 4,
            ssi_field: SystemInfo::ssi_time(3, 18, 21),
            ..SystemInfo::default()
        };
        assert_eq!(SystemInfo::from_words(si.words()), si);
        assert_eq!(
            si.text(),
            "country 40 operator 2 area 5, cycle 12 subsequence 3 batch E, zone 3 hour 18 day 21"
        );
    }
}
