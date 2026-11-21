//! Spektrum DSM2 and DSMX: the 64-chip codes, a byte as a spread symbol and back, the hop list of DSMX, the data packet, and
//! the search of a packet in the chips of a burst. No public document gives them; the chip polarity and the CRC (not checked)
//! are not confirmed.

/// The chip rate, chips/s.
pub const CHIP_RATE: f64 = 1e6;
/// The preamble chips: bytes 33 33 04, each least significant bit first.
pub const PREAMBLE: [u8; 3] = [0x33, 0x33, 0x04];
/// Symbols after the start-of-packet code: the length byte, 16 data bytes, 2 CRC bytes.
pub const SYMBOLS: usize = 19;
/// Chips of a whole packet.
pub const PACKET_CHIPS: usize = 24 + 128 + SYMBOLS * 64;
/// The most chip errors of the start-of-packet pattern (128 chips).
pub const SOP_ERRORS: u32 = 16;
/// The most chip errors of a data symbol.
pub const SYMBOL_ERRORS: u32 = 14;

/// The 41 codes: 8 of each of the 5 blocks, then one more. Chip `j` of a code is bit `j mod 8` of byte `j / 8`.
pub const CODES: [[u8; 8]; 41] = [
    [0x83, 0xF7, 0xA8, 0x2D, 0x7A, 0x44, 0x64, 0xD3],
    [0x3F, 0x2C, 0x4E, 0xAA, 0x71, 0x48, 0x7A, 0xC9],
    [0x17, 0xFF, 0x9E, 0x21, 0x36, 0x90, 0xC7, 0x82],
    [0xBC, 0x5D, 0x9A, 0x5B, 0xEE, 0x7F, 0x42, 0xEB],
    [0x24, 0xF5, 0xDD, 0xF8, 0x7A, 0x77, 0x74, 0xE7],
    [0x3D, 0x70, 0x7C, 0x94, 0xDC, 0x84, 0xAD, 0x95],
    [0x1E, 0x6A, 0xF0, 0x37, 0x52, 0x7B, 0x11, 0xD4],
    [0x62, 0xF5, 0x2B, 0xAA, 0xFC, 0x33, 0xBF, 0xAF],
    [0x40, 0x56, 0x32, 0xD9, 0x0F, 0xD9, 0x5D, 0x97],
    [0x8E, 0x4A, 0xD0, 0xA9, 0xA7, 0xFF, 0x20, 0xCA],
    [0x4C, 0x97, 0x9D, 0xBF, 0xB8, 0x3D, 0xB5, 0xBE],
    [0x0C, 0x5D, 0x24, 0x30, 0x9F, 0xCA, 0x6D, 0xBD],
    [0x50, 0x14, 0x33, 0xDE, 0xF1, 0x78, 0x95, 0xAD],
    [0x0C, 0x3C, 0xFA, 0xF9, 0xF0, 0xF2, 0x10, 0xC9],
    [0xF4, 0xDA, 0x06, 0xDB, 0xBF, 0x4E, 0x6F, 0xB3],
    [0x9E, 0x08, 0xD1, 0xAE, 0x59, 0x5E, 0xE8, 0xF0],
    [0xC0, 0x90, 0x8F, 0xBB, 0x7C, 0x8E, 0x2B, 0x8E],
    [0x80, 0x69, 0x26, 0x80, 0x08, 0xF8, 0x49, 0xE7],
    [0x7D, 0x2D, 0x49, 0x54, 0xD0, 0x80, 0x40, 0xC1],
    [0xB6, 0xF2, 0xE6, 0x1B, 0x80, 0x5A, 0x36, 0xB4],
    [0x42, 0xAE, 0x9C, 0x1C, 0xDA, 0x67, 0x05, 0xF6],
    [0x9B, 0x75, 0xF7, 0xE0, 0x14, 0x8D, 0xB5, 0x80],
    [0xBF, 0x54, 0x98, 0xB9, 0xB7, 0x30, 0x5A, 0x88],
    [0x35, 0xD1, 0xFC, 0x97, 0x23, 0xD4, 0xC9, 0x88],
    [0xE1, 0xD6, 0x31, 0x26, 0x5F, 0xBD, 0x40, 0x93],
    [0xDC, 0x68, 0x08, 0x99, 0x97, 0xAE, 0xAF, 0x8C],
    [0xC3, 0x0E, 0x01, 0x16, 0x0E, 0x32, 0x06, 0xBA],
    [0xE0, 0x83, 0x01, 0xFA, 0xAB, 0x3E, 0x8F, 0xAC],
    [0x5C, 0xD5, 0x9C, 0xB8, 0x46, 0x9C, 0x7D, 0x84],
    [0xF1, 0xC6, 0xFE, 0x5C, 0x9D, 0xA5, 0x4F, 0xB7],
    [0x58, 0xB5, 0xB3, 0xDD, 0x0E, 0x28, 0xF1, 0xB0],
    [0x5F, 0x30, 0x3B, 0x56, 0x96, 0x45, 0xF4, 0xA1],
    [0x03, 0xBC, 0x6E, 0x8A, 0xEF, 0xBD, 0xFE, 0xF8],
    [0x88, 0x17, 0x13, 0x3B, 0x2D, 0xBF, 0x06, 0xD6],
    [0xF1, 0x94, 0x30, 0x21, 0xA1, 0x1C, 0x88, 0xA9],
    [0xD0, 0xD2, 0x8E, 0xBC, 0x82, 0x2F, 0xE3, 0xB4],
    [0x8C, 0xFA, 0x47, 0x9B, 0x83, 0xA5, 0x66, 0xD0],
    [0x07, 0xBD, 0x9F, 0x26, 0xC8, 0x31, 0x0F, 0xB8],
    [0xEF, 0x03, 0x95, 0x89, 0xB4, 0x71, 0x61, 0x9D],
    [0x40, 0xBA, 0x97, 0xD5, 0x86, 0x4F, 0xCC, 0xD1],
    [0xD7, 0xA1, 0x54, 0xB1, 0x5E, 0x89, 0xAE, 0x86],
];

/// A code as a 64-bit word, chip `j` in bit `j`.
pub fn code(i: usize) -> u64 {
    u64::from_le_bytes(CODES[i])
}

/// The codes of one pn row and SOP column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeSet {
    /// The pn row, 0 to 4.
    pub row: u8,
    /// The SOP column, 0 to 7.
    pub col: u8,
    /// The start-of-packet code.
    pub sop: u64,
    /// Data code A (bit 7 of a byte 0).
    pub a: u64,
    /// Data code B (bit 7 of a byte 1).
    pub b: u64,
}

impl CodeSet {
    /// The codes of row `row` and column `col`: block 4 for row 0, else block `row - 1`.
    pub fn new(row: u8, col: u8) -> CodeSet {
        let block = if row == 0 { 4 } else { usize::from(row) - 1 };
        let c = usize::from(col & 7);
        CodeSet {
            row,
            col: col & 7,
            sop: code(8 * block + c),
            a: code(8 * block + 7 - c),
            b: code(8 * block + 8 - c),
        }
    }

    /// Every set: 5 rows by 8 columns.
    pub fn all() -> impl Iterator<Item = CodeSet> {
        (0..5u8).flat_map(|r| (0..8u8).map(move |c| CodeSet::new(r, c)))
    }

    /// The 64 chips of byte `v` (chip `j` in bit `j`): code A or B by bit 7, turned by `v & 63`, complemented when bit 6 is 0.
    pub fn symbol(&self, v: u8) -> u64 {
        let base = if v & 0x80 != 0 { self.b } else { self.a };
        let s = base.rotate_right(u32::from(v & 63));
        if v & 0x40 != 0 {
            s
        } else {
            !s
        }
    }

    /// The byte whose symbol is nearest `chips`, and the chips that differ.
    pub fn despread(&self, chips: u64) -> (u8, u32) {
        let mut best = (0u8, u32::MAX);
        for (bit7, base) in [(0u8, self.a), (0x80, self.b)] {
            for s in 0..64u8 {
                let d = (base.rotate_right(u32::from(s)) ^ chips).count_ones();
                // The complement differs in the other chips.
                let (v, e) = if d <= 32 {
                    (bit7 | 0x40 | s, d)
                } else {
                    (bit7 | s, 64 - d)
                };
                if e < best.1 {
                    best = (v, e);
                }
            }
        }
        best
    }
}

/// The SOP column of a transmitter of id bytes `id`: `(id0 + id1 + id2 + 2) mod 8`.
pub fn sop_column(id: [u8; 4]) -> u8 {
    ((u32::from(id[0]) + u32::from(id[1]) + u32::from(id[2]) + 2) % 8) as u8
}

/// The pn row of rf channel `rf` (2400 + rf MHz).
pub fn pn_row(rf: u8, dsmx: bool) -> u8 {
    if dsmx {
        ((i32::from(rf) - 2).rem_euclid(5)) as u8
    } else {
        rf % 5
    }
}

/// The 23 rf channels of a DSMX transmitter of id bytes `id`.
pub fn dsmx_channels(id: [u8; 4]) -> Vec<u8> {
    let mut t = !u32::from_be_bytes(id);
    let mut out: Vec<u8> = Vec::with_capacity(23);
    let mut groups = [0u8; 3];
    let mut guard = 0;
    while out.len() < 23 && guard < 100_000 {
        guard += 1;
        t = t.wrapping_mul(0x0019_660D).wrapping_add(0x3C6E_F35F);
        let next = ((t >> 8) % 73 + 3) as u8;
        let g = if next < 28 {
            0
        } else if next < 52 {
            1
        } else {
            2
        };
        let cap = [8, 7, 8][g];
        if (next ^ id[3]) & 1 == 1 || out.contains(&next) || groups[g] >= cap {
            continue;
        }
        groups[g] += 1;
        out.push(next);
    }
    out
}

/// The chips of a packet (one chip a byte, 1 the higher frequency): the preamble, the SOP code and its complement, the length
/// byte 16, the 16 data bytes and two CRC bytes `crc`.
pub fn packet_chips(set: &CodeSet, data: &[u8; 16], crc: [u8; 2], out: &mut Vec<u8>) {
    for &b in &PREAMBLE {
        out.extend((0..8).map(|k| (b >> k) & 1));
    }
    let word = |w: u64, out: &mut Vec<u8>| out.extend((0..64).map(|j| ((w >> j) & 1) as u8));
    word(set.sop, out);
    word(!set.sop, out);
    word(set.symbol(16), out);
    for &b in data {
        word(set.symbol(b), out);
    }
    for b in crc {
        word(set.symbol(b), out);
    }
}

/// What a data packet holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DsmData {
    /// Bytes 0 and 1 (id2 and id3 for DSMX, their complements for DSM2).
    pub head: [u8; 2],
    /// Value bits of a word: 11 or 10.
    pub bits: u8,
    /// The used words as (channel index, value).
    pub channels: Vec<(u8, u16)>,
    /// Words of the packet that are used.
    pub words: u8,
    /// The second frame of a pair.
    pub second: bool,
}

/// Reads a data packet: the first value width of 11 and 10 bits for which no word but the first has bit 15, the channel
/// indexes are distinct and at most 12, and at least 3 words are used.
pub fn parse_data(p: &[u8; 16]) -> Option<DsmData> {
    let words: Vec<u16> = (0..7)
        .map(|i| u16::from_be_bytes([p[2 + 2 * i], p[3 + 2 * i]]))
        .collect();
    if words[1..].iter().any(|&w| w != 0xFFFF && w & 0x8000 != 0) {
        return None;
    }
    for bits in [11u8, 10] {
        let mut ch = Vec::with_capacity(7);
        let mut ok = true;
        for &w in &words {
            if w == 0xFFFF {
                continue;
            }
            let v = w & 0x7FFF;
            let idx = (v >> bits) as u8;
            if idx > 12 || ch.iter().any(|&(i, _)| i == idx) {
                ok = false;
                break;
            }
            ch.push((idx, v & ((1 << bits) - 1)));
        }
        if ok && ch.len() >= 3 {
            return Some(DsmData {
                head: [p[0], p[1]],
                bits,
                words: ch.len() as u8,
                channels: ch,
                second: words[0] != 0xFFFF && words[0] & 0x8000 != 0,
            });
        }
    }
    None
}

/// The 16 data bytes of a packet with head bytes `head`, value width `bits` and up to 7 channels `(index, value)`; the first
/// word gets bit 15 for the second frame of a pair.
pub fn data(head: [u8; 2], bits: u8, ch: &[(u8, u16)], second: bool) -> [u8; 16] {
    let mut p = [0xFFu8; 16];
    p[0] = head[0];
    p[1] = head[1];
    for (i, &(idx, v)) in ch.iter().take(7).enumerate() {
        let mut w = u16::from(idx) << bits | (v & ((1 << bits) - 1));
        if i == 0 && second {
            w |= 0x8000;
        }
        p[2 + 2 * i..4 + 2 * i].copy_from_slice(&w.to_be_bytes());
    }
    p
}

/// A packet found in hard chips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The code set of the SOP code.
    pub set: CodeSet,
    /// The chip index of the first chip of the SOP code.
    pub at: usize,
    /// Chip errors of the SOP pattern.
    pub sop_errors: u32,
    /// The 16 data bytes.
    pub data: [u8; 16],
    /// The two CRC bytes (not checked).
    pub crc: [u8; 2],
    /// The most chip errors of a symbol.
    pub worst: u32,
}

/// The 64 chips from `at` of `chips` (one chip a byte) as a word, chip `j` in bit `j`.
fn word_at(chips: &[u8], at: usize) -> u64 {
    chips[at..at + 64]
        .iter()
        .enumerate()
        .fold(0u64, |w, (j, &c)| w | u64::from(c & 1) << j)
}

/// The search of a packet in hard chips, with its buffers.
#[derive(Clone, Debug)]
pub struct DsmSearch {
    sets: Vec<CodeSet>,
    words: Vec<u64>,
}

impl Default for DsmSearch {
    fn default() -> Self {
        DsmSearch {
            sets: CodeSet::all().collect(),
            words: Vec::new(),
        }
    }
}

impl DsmSearch {
    /// Searches `chips` (one chip a byte) for a packet: the SOP pattern of every code set with at most [`SOP_ERRORS`] errors
    /// (the fewest win), then the 19 symbols, each with at most [`SYMBOL_ERRORS`] errors and a length byte of 16.
    pub fn find(&mut self, chips: &[u8]) -> Option<Found> {
        let need = 128 + SYMBOLS * 64;
        if chips.len() < need {
            return None;
        }
        // Sliding words of 64 chips: `words[k]` holds chips k..k+64.
        let n = chips.len() - 63;
        self.words.clear();
        let mut w = word_at(chips, 0);
        self.words.push(w);
        for k in 1..n {
            w = w >> 1 | u64::from(chips[k + 63] & 1) << 63;
            self.words.push(w);
        }
        let mut best: Option<(u32, usize, CodeSet)> = None;
        for at in 0..=chips.len() - need {
            let (x, y) = (self.words[at], self.words[at + 64]);
            for s in &self.sets {
                let e = (x ^ s.sop).count_ones() + (y ^ !s.sop).count_ones();
                if e <= SOP_ERRORS && best.is_none_or(|b| e < b.0) {
                    best = Some((e, at, *s));
                }
            }
        }
        let (sop_errors, at, set) = best?;
        let mut bytes = [0u8; SYMBOLS];
        let mut worst = 0;
        for (i, b) in bytes.iter_mut().enumerate() {
            let (v, e) = set.despread(self.words[at + 128 + 64 * i]);
            if e > SYMBOL_ERRORS {
                return None;
            }
            worst = worst.max(e);
            *b = v;
        }
        if bytes[0] != 16 {
            return None;
        }
        let mut data = [0u8; 16];
        data.copy_from_slice(&bytes[1..17]);
        Some(Found {
            set,
            at,
            sop_errors,
            data,
            crc: [bytes[17], bytes[18]],
            worst,
        })
    }
}
