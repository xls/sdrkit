//! The CCIR 476 seven-unit code of SITOR and NAVTEX (ITU-R M.476-5 and M.625-4), and mode B forward error correction (FEC-B):
//! the time-diversity transmitter and the receiver that combines the two copies of every character.
//!
//! # Words
//!
//! A [`Word`] holds the seven units with bit position 1 (sent first) in the least significant bit, and **B = 0, Y = 1**, exactly as
//! ITU-R M.625-4 Table 1 note 3 defines them (B is the higher emitted frequency). A valid word has four B and three Y, so three bits
//! set. Some receivers count B as 1; such a word is `!w & 0x7F` here.
//!
//! Traffic words map one to one onto the ITA2 combinations (Table 1), so the characters come from an [`ita2::Table`]; CCIR 476 uses
//! the ITU figures case ([`ita2::ITU`]).
//!
//! # Mode B (M.625-4 section 4)
//!
//! Every word is sent twice: first in a DX position and again in the RX position four words later (280 ms), the two streams
//! interleaved DX, RX, DX, RX. While phasing, "phasing signal 2" (RQ) is sent in DX positions and "phasing signal 1" (alpha) in RX
//! positions. The receiver checks both copies and uses the unmutilated one; when both look valid but differ, both count as
//! mutilated (4.3). The transmission ends with idle alpha.
//!
//! ```
//! use sdrkit_util_text::ccir476::{FecBDecoder, FecBEncoder, Fec};
//!
//! let mut air = Vec::new();
//! FecBEncoder::new().message("ZCZC SA01", &mut air).unwrap();
//! let mut rx = FecBDecoder::new();
//! let text: String = air
//!     .iter()
//!     .filter_map(|&w| match rx.push(w) {
//!         Some(Fec::Char(c)) => Some(c),
//!         _ => None,
//!     })
//!     .collect();
//! assert!(text.ends_with("ZCZC SA01"));
//! ```

use crate::error::{Error, Result};
use crate::ita2::{self, Table};

/// A seven-unit word: bit position 1 in bit 0, B = 0, Y = 1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Word(pub u8);

/// Builds a word from its signal written as in ITU-R M.625-4 ("BBBYYYB", bit position 1 first).
pub const fn word(by: &str) -> Word {
    let b = by.as_bytes();
    let mut v = 0u8;
    let mut i = 0;
    while i < 7 {
        if b[i] == b'Y' {
            v |= 1 << i;
        }
        i += 1;
    }
    Word(v)
}

impl Word {
    /// Whether the word has the 4B/3Y ratio of a valid signal.
    pub fn is_valid(self) -> bool {
        self.0 < 0x80 && self.0.count_ones() == 3
    }

    /// The units, bit position 1 first (1 = Y).
    pub fn bits(self) -> [u8; 7] {
        core::array::from_fn(|i| (self.0 >> i) & 1)
    }

    /// The word of seven units, bit position 1 first (1 = Y; only the low bit of each is used).
    pub fn from_bits(bits: &[u8; 7]) -> Word {
        Word(bits.iter().enumerate().map(|(i, &b)| (b & 1) << i).sum())
    }

    /// The ITA2 code this traffic word stands for (`None` for a service signal or an invalid word).
    pub fn ita2(self) -> Option<u8> {
        TRAFFIC.iter().position(|&w| w == self).map(|i| i as u8)
    }

    /// The traffic word of an ITA2 code (0 to 31).
    pub fn from_ita2(code: u8) -> Result<Word> {
        TRAFFIC.get(code as usize).copied().ok_or(Error::OutOfRange)
    }
}

/// The traffic words by ITA2 code (M.625-4 Table 1; code 0 is combination 32, "no information").
pub const TRAFFIC: [Word; 32] = {
    // (ITA2 code, transmitted signal) in the order of Table 1.
    const T: [(u8, &str); 32] = [
        (3, "BBBYYYB"),  // A
        (25, "YBYYBBB"), // B
        (14, "BYBBBYY"), // C
        (9, "BBYYBYB"),  // D
        (1, "YBBYBYB"),  // E
        (13, "BBYBBYY"), // F
        (26, "BYBYBBY"), // G
        (20, "BYYBYBB"), // H
        (6, "BYBBYYB"),  // I
        (11, "BBBYBYY"), // J
        (15, "YBBBBYY"), // K
        (18, "BYBYYBB"), // L
        (28, "BYYBBBY"), // M
        (12, "BYYBBYB"), // N
        (24, "BYYYBBB"), // O
        (22, "BYBBYBY"), // P
        (23, "YBBBYBY"), // Q
        (10, "BYBYBYB"), // R
        (5, "BBYBYYB"),  // S
        (16, "YYBYBBB"), // T
        (7, "YBBBYYB"),  // U
        (30, "YYBBBBY"), // V
        (19, "BBBYYBY"), // W
        (29, "YBYBBBY"), // X
        (21, "BBYBYBY"), // Y
        (17, "BBYYYBB"), // Z
        (8, "YYYBBBB"),  // 27: carriage return
        (2, "YYBBYBB"),  // 28: line feed
        (31, "YBYBBYB"), // 29: letter shift
        (27, "YBBYBBY"), // 30: figure shift
        (4, "YYBBBYB"),  // 31: space
        (0, "YBYBYBB"),  // 32: no information
    ];
    let mut out = [Word(0); 32];
    let mut i = 0;
    while i < 32 {
        out[T[i].0 as usize] = word(T[i].1);
        i += 1;
    }
    out
};

/// Phasing signal 1, also idle signal alpha (M.625-4 Table 2).
pub const ALPHA: Word = word("BBBBYYY");
/// Idle signal beta.
pub const BETA: Word = word("BBYYBBY");
/// Signal repetition (RQ), also phasing signal 2 in mode B.
pub const RQ: Word = word("YBBYYBB");
/// Control signal 1 (mode A).
pub const CS1: Word = word("BYBYYBB");
/// Control signal 2 (mode A; the same signal as "no information").
pub const CS2: Word = word("YBYBYBB");
/// Control signal 3 (mode A).
pub const CS3: Word = word("BYYBBYB");
/// Control signal 4 (mode A).
pub const CS4: Word = word("BYBYBBY");
/// Control signal 5 (mode A).
pub const CS5: Word = word("BYYBYBB");
/// The letter shift word.
pub const LTRS: Word = TRAFFIC[ita2::LTRS as usize];
/// The figure shift word.
pub const FIGS: Word = TRAFFIC[ita2::FIGS as usize];
/// Mode B phasing signal 1 (sent in RX positions).
pub const PHASING_1: Word = ALPHA;
/// Mode B phasing signal 2 (sent in DX positions).
pub const PHASING_2: Word = RQ;

/// The 35 valid words in ascending order.
pub fn valid_words() -> impl Iterator<Item = Word> {
    (0u8..0x80).map(Word).filter(|w| w.is_valid())
}

/// The output of the mode B receiver for one character position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fec {
    /// A character.
    Char(char),
    /// Both copies were mutilated (or valid but different): M.625 prints an error character here.
    Mutilated,
    /// Idle alpha in a DX position (the end of a transmission when it repeats).
    Idle,
}

/// The mode B transmitter: turns DX words into the interleaved DX/RX stream on the air.
#[derive(Clone, Debug)]
pub struct FecBEncoder {
    /// DX words not yet repeated in an RX position (the last two).
    pending: [Word; 2],
    phase: usize,
    figs: Option<bool>,
    /// The table characters are taken from.
    pub table: Table,
    /// Phasing pairs sent before a message (at least 16, M.625-4 4.4.2).
    pub phasing_pairs: usize,
    /// Idle alpha words sent in DX positions after a message (2 s is about 29 positions, 4.6.7.1).
    pub idle_words: usize,
}

impl Default for FecBEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FecBEncoder {
    /// A transmitter with the ITU table, 16 phasing pairs and 15 idle words.
    pub fn new() -> Self {
        FecBEncoder {
            pending: [PHASING_2; 2],
            phase: 0,
            figs: None,
            table: ita2::ITU,
            phasing_pairs: 16,
            idle_words: 15,
        }
    }

    /// Sends one word in the next DX position, and the RX repetition of the word sent two DX positions earlier. Phasing signal 2 in
    /// DX is answered by phasing signal 1 in RX.
    pub fn push_dx(&mut self, dx: Word, out: &mut impl FnMut(Word)) {
        let old = self.pending[self.phase];
        self.pending[self.phase] = dx;
        self.phase ^= 1;
        out(dx);
        out(if old == PHASING_2 { PHASING_1 } else { old });
    }

    /// Sends text in DX positions (a newline as CR LF), with the shifts it needs.
    pub fn text(&mut self, text: &str, out: &mut Vec<Word>) -> Result<()> {
        let table = self.table;
        let mut enc = ita2::Encoder::new(&table);
        let mut codes = [0u8; 8];
        for c in text.chars() {
            let mut n = 0;
            enc.encode_char(c, &mut |b| {
                if n < codes.len() {
                    codes[n] = b;
                    n += 1;
                }
            })?;
            for &code in &codes[..n] {
                // The encoder starts with a shift; skip it when the case is already right.
                let is_shift = code == ita2::LTRS || code == ita2::FIGS;
                if is_shift {
                    let f = code == ita2::FIGS;
                    if self.figs == Some(f) {
                        continue;
                    }
                    self.figs = Some(f);
                }
                let w = TRAFFIC[code as usize];
                self.push_dx(w, &mut |x| out.push(x));
            }
        }
        Ok(())
    }

    /// A whole transmission: phasing, CR LF (M.625-4 4.6.1), the text, then idle alpha.
    pub fn message(&mut self, text: &str, out: &mut Vec<Word>) -> Result<()> {
        for _ in 0..self.phasing_pairs {
            self.push_dx(PHASING_2, &mut |x| out.push(x));
        }
        self.text("\n", out)?;
        self.text(text, out)?;
        for _ in 0..self.idle_words.max(2) {
            self.push_dx(ALPHA, &mut |x| out.push(x));
        }
        Ok(())
    }
}

/// One received position: the hard word and its soft units (LLR per unit, positive means B).
#[derive(Clone, Copy, Debug, Default)]
struct Slot {
    word: Word,
    soft: [f32; 7],
    has_soft: bool,
}

/// The mode B receiver: finds which positions are DX, combines each DX word with its RX copy five positions later, and decodes the
/// characters with their shifts. It keeps no allocation.
#[derive(Clone, Debug)]
pub struct FecBDecoder {
    slots: [Slot; 8],
    n: u64,
    votes: [f32; 2],
    dx: usize,
    next: u64,
    figs: bool,
    /// The table characters are taken from.
    pub table: Table,
    /// When both copies fail, or disagree, pick the valid word nearest to the sum of the two soft copies instead of reporting a
    /// mutilated character (needs soft input; off by default, as in M.625).
    pub repair: bool,
    /// A space returns to letters case.
    pub unshift_on_space: bool,
    /// Characters given.
    pub chars: u64,
    /// Positions reported as mutilated.
    pub mutilated: u64,
    /// Characters recovered from the soft copies.
    pub repaired: u64,
}

impl Default for FecBDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl FecBDecoder {
    /// A receiver with the ITU table.
    pub fn new() -> Self {
        FecBDecoder {
            slots: [Slot::default(); 8],
            n: 0,
            votes: [0.0; 2],
            dx: 0,
            next: 0,
            figs: false,
            table: ita2::ITU,
            repair: false,
            unshift_on_space: false,
            chars: 0,
            mutilated: 0,
            repaired: 0,
        }
    }

    /// Forgets the stream (a new signal).
    pub fn reset(&mut self) {
        let table = self.table;
        let (repair, uos) = (self.repair, self.unshift_on_space);
        *self = Self::new();
        self.table = table;
        self.repair = repair;
        self.unshift_on_space = uos;
    }

    /// Whether the DX positions are known with some confidence.
    pub fn locked(&self) -> bool {
        self.votes[self.dx] >= 3.0
    }

    /// One hard word from the air.
    pub fn push(&mut self, w: Word) -> Option<Fec> {
        self.slot(Slot {
            word: w,
            soft: [0.0; 7],
            has_soft: false,
        })
    }

    /// One word as soft units (LLR per unit, bit position 1 first, positive means B). Bad input (NaN) counts as no information.
    pub fn push_soft(&mut self, llr: &[f32; 7]) -> Option<Fec> {
        let mut soft = [0.0f32; 7];
        let mut v = 0u8;
        for (i, (&l, s)) in llr.iter().zip(soft.iter_mut()).enumerate() {
            *s = if l.is_finite() { l } else { 0.0 };
            if *s < 0.0 {
                v |= 1 << i;
            }
        }
        self.slot(Slot {
            word: Word(v),
            soft,
            has_soft: true,
        })
    }

    fn slot(&mut self, s: Slot) -> Option<Fec> {
        self.slots[(self.n % 8) as usize] = s;
        self.n += 1;
        if self.n < 6 {
            return None;
        }
        let j = self.n - 6;
        let a = self.slots[(j % 8) as usize];
        let b = self.slots[((j + 5) % 8) as usize];
        let par = (j % 2) as usize;
        // Evidence for "j is a DX position": its RX copy agrees, or it carries phasing signal 2 (sent only in DX positions).
        let agree = a.word.is_valid() && a.word == b.word && a.word != ALPHA && a.word != RQ;
        if agree || a.word == RQ {
            self.votes[par] += 1.0;
        }
        self.votes[par] *= 0.97;
        if self.votes[1 - self.dx] > self.votes[self.dx] + 2.0 {
            self.dx = 1 - self.dx;
            self.next = self.next.max(j);
        }
        if par != self.dx || j < self.next {
            return None;
        }
        self.next = j + 1;
        if a.word == PHASING_2 {
            // Phasing: its RX copy is phasing signal 1 by design.
            return None;
        }
        let w = self.combine(&a, &b);
        match w {
            None => {
                self.mutilated += 1;
                Some(Fec::Mutilated)
            }
            Some(w) => self.word(w),
        }
    }

    fn combine(&mut self, a: &Slot, b: &Slot) -> Option<Word> {
        let (va, vb) = (a.word.is_valid(), b.word.is_valid());
        match (va, vb) {
            (true, true) if a.word == b.word => return Some(a.word),
            (true, false) => return Some(a.word),
            (false, true) => return Some(b.word),
            _ => {}
        }
        if !(self.repair && a.has_soft && b.has_soft) {
            return None;
        }
        // Sum the two copies and take the valid word with the largest correlation.
        let sum: [f32; 7] = core::array::from_fn(|i| a.soft[i] + b.soft[i]);
        let best = valid_words().max_by(|x, y| {
            let m = |w: Word| -> f32 {
                (0..7)
                    .map(|i| if (w.0 >> i) & 1 == 1 { -sum[i] } else { sum[i] })
                    .sum()
            };
            m(*x).total_cmp(&m(*y))
        });
        self.repaired += best.is_some() as u64;
        best
    }

    /// Decodes a word already accepted (shifts, service signals and characters).
    fn word(&mut self, w: Word) -> Option<Fec> {
        if w == ALPHA {
            return Some(Fec::Idle);
        }
        let code = w.ita2()?;
        match code {
            ita2::LTRS => self.figs = false,
            ita2::FIGS => self.figs = true,
            ita2::BLANK => {}
            c => {
                if c == ita2::SPACE && self.unshift_on_space {
                    self.figs = false;
                }
                let ch = self.table.char_of(c, self.figs)?;
                self.chars += 1;
                return Some(Fec::Char(ch));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_has_35_valid_words_all_used_once() {
        assert_eq!(valid_words().count(), 35);
        let mut used: Vec<Word> = TRAFFIC.to_vec();
        used.extend([ALPHA, BETA, RQ, CS1, CS3, CS4, CS5]);
        // CS2 is the same signal as "no information" (Table 2 and Table 1 combination 32).
        assert_eq!(CS2, TRAFFIC[0]);
        used.sort_by_key(|w| w.0);
        used.dedup();
        // The mode A control signals 1, 3, 4 and 5 share their signals with L, N, G and H (Table 2), so 35 distinct words remain.
        assert_eq!(used, valid_words().collect::<Vec<_>>());
        for w in &TRAFFIC {
            assert!(w.is_valid(), "{w:?}");
        }
        for w in [ALPHA, BETA, RQ] {
            assert!(w.is_valid() && w.ita2().is_none(), "{w:?}");
        }
    }

    #[test]
    fn known_words() {
        // Bit position 1 in bit 0, Y = 1: A is BBBYYYB = 0b0111000.
        assert_eq!(TRAFFIC[3], Word(0b0111000));
        assert_eq!(ALPHA, Word(0b1110000));
        assert_eq!(RQ, Word(0b0011001));
        assert_eq!(Word::from_ita2(3).unwrap().ita2(), Some(3));
        assert_eq!(Word::from_ita2(32), Err(Error::OutOfRange));
        assert_eq!(Word::from_bits(&TRAFFIC[3].bits()), TRAFFIC[3]);
    }

    fn decode(air: &[Word], rx: &mut FecBDecoder) -> String {
        air.iter()
            .filter_map(|&w| match rx.push(w) {
                Some(Fec::Char(c)) => Some(c),
                Some(Fec::Mutilated) => Some('*'),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn interleave_follows_m625_figure_2() {
        let mut e = FecBEncoder::new();
        let mut air = vec![];
        for code in [3u8, 25, 14, 9] {
            e.push_dx(TRAFFIC[code as usize], &mut |w| air.push(w));
        }
        // DX positions 0, 2, 4, 6; the RX copy of position 0 is at position 5.
        assert_eq!(air[0], TRAFFIC[3]);
        assert_eq!(air[1], PHASING_1);
        assert_eq!(air[3], PHASING_1);
        assert_eq!(air[5], TRAFFIC[3]);
        assert_eq!(air[7], TRAFFIC[25]);
    }

    #[test]
    fn a_message_round_trips_with_errors_in_one_copy() {
        let text = "ZCZC SA01\nNAVIGATIONAL WARNING 1234/56 ";
        let mut air = vec![];
        FecBEncoder::new().message(text, &mut air).unwrap();
        let mut rx = FecBDecoder::new();
        assert!(decode(&air, &mut rx).ends_with(&text.replace('\n', "\r\n")));
        // Flip a unit in every DX copy of the text: the RX copies carry it.
        let mut bad = air.clone();
        for (i, w) in bad.iter_mut().enumerate().skip(40) {
            if i % 2 == 0 {
                w.0 ^= 1;
            }
        }
        let mut rx = FecBDecoder::new();
        assert!(decode(&bad, &mut rx).contains("NAVIGATIONAL WARNING 1234/56"));
        // Starting mid-stream on the odd position still locks to DX.
        let mut rx = FecBDecoder::new();
        assert!(decode(&air[7..], &mut rx).contains("WARNING"));
    }

    #[test]
    fn both_copies_bad_is_mutilated_and_soft_repair_recovers() {
        let mut air = vec![];
        let mut e = FecBEncoder::new();
        for _ in 0..16 {
            e.push_dx(PHASING_2, &mut |w| air.push(w));
        }
        e.text("TEST TEST TEST", &mut air).unwrap();
        for _ in 0..4 {
            e.push_dx(ALPHA, &mut |w| air.push(w));
        }
        // Damage both copies of the first "E" in different units.
        let dx = air.iter().position(|&w| w == TRAFFIC[1]).unwrap();
        let rx = dx + 5;
        assert_eq!(air[rx], TRAFFIC[1]);
        let mut hard = air.clone();
        hard[dx].0 ^= 1 << 0;
        hard[rx].0 ^= 1 << 6;
        let mut d = FecBDecoder::new();
        assert!(decode(&hard, &mut d).starts_with("T*ST"));
        assert_eq!(d.mutilated, 1);
        // Soft copies: the damaged units are weak, so their sum points to the right word.
        let soft = |w: Word, weak: usize| -> [f32; 7] {
            core::array::from_fn(|i| {
                let l = if (w.0 >> i) & 1 == 1 { -4.0 } else { 4.0 };
                if i == weak {
                    -l * 0.25
                } else {
                    l
                }
            })
        };
        let mut d = FecBDecoder::new();
        d.repair = true;
        let mut out = String::new();
        for (i, &w) in air.iter().enumerate() {
            let llr = if i == dx {
                soft(w, 0)
            } else if i == rx {
                soft(w, 6)
            } else {
                soft(w, 99)
            };
            if let Some(Fec::Char(c)) = d.push_soft(&llr) {
                out.push(c);
            }
        }
        assert!(out.starts_with("TEST TEST"), "{out}");
        assert_eq!(d.repaired, 1);
        // NaN input counts as no information.
        d.push_soft(&[f32::NAN; 7]);
    }

    #[test]
    fn idle_alpha_is_reported() {
        let mut air = vec![];
        FecBEncoder::new().message("A", &mut air).unwrap();
        let mut rx = FecBDecoder::new();
        let idle = air
            .iter()
            .filter(|&&w| rx.push(w) == Some(Fec::Idle))
            .count();
        assert!(idle >= 10);
    }
}
