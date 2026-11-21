//! FLEX at 1600/2, 3200/2, 3200/4 and 6400/4 (speed in bit/s, then the number of FSK levels).
//!
//! ITU-R M.584-2 annex 2 (FLEX-TD) defines the frame: 15 cycles of 128 frames an hour, each frame 1.875 s. A frame is a sync part
//! of 115 ms and 11 interleaved blocks of 160 ms. Sync 1 and the frame information word (FIW) go at 1600 bit/s, 2 levels, and
//! together they are 144 bits. Sync 2 (25 ms) goes at the speed of the frame. Each block holds 8 words per phase, interleaved so
//! that successive bits of a word are 5 ms apart. The code word is the POCSAG word ([`crate::word`]). Information bits go out
//! least significant first. FIW, block information and vector words carry a 4-bit checksum
//! ([`sdrkit_util_bits::checksum::flex_check_ok`]).
//!
//! A phase is one stream of 88 words (2816 bits) a frame. The fast speeds carry two or four phases.
//!
//! ARIB RCR STD-43A (FLEX-TD) chapter 3 gives the field layouts of the FIW, the block information words, the address ranges
//! and long-address sets, the vector words, the message headers and the check sums of messages. The sync codes, the sync 2
//! pattern, the letters of the phases and the 4-level bit mapping follow public descriptions of the protocol.
//!
//! # Acceptance rules
//!
//! A page is given only when it passes, after the code word repair (at most 2 bits, [`crate::word`]):
//!
//! - the FIW: its 4-bit check and a cycle number of 0 to 14;
//! - the first block information word: its 4-bit check, not an idle value, the address field before the vector field;
//! - the address: a short address of capcode 1 to [`MAX_SHORT_CAPCODE`], or one of the long-address sets 1-2, 1-3, 1-4 and
//!   2-3 ([`address`], [`long_capcode`]); other ranges are not pages;
//! - the vector word: its 4-bit check, a message start of 3 to 87, 2 to 85 words for secure, text and binary messages; for a
//!   long address a one-word numeric message starts at the second vector word;
//! - the message: the fragment check of text, secure and binary messages ([`fragment_check`]), the 6-bit check of numeric
//!   messages ([`numeric_check`]), the signature of a whole text message ([`signature`]), a fragment control of a later text
//!   fragment that is not the reserved value 01, the reserved bits of a binary first fragment.
//!
//! These rules are not stated by a public document and are counted apart ([`Refusals::unconfirmed`]): the message words
//! lie below word 88 and after the vector field; a short message of type 11 and short instructions 010 to 110 are not pages;
//! in a frame whose number disagrees with a frame 1 or 2 frames before it, a page with no message check is not given.
//!
//! A page that fails a rule is counted in [`FlexDecoder::refusals`]. A message with a lost word cannot be checked and is
//! given as damaged.
//!
//! ```
//! use sdrkit_proto_pager::flex::{self, FlexDecoder, FlexTx, Fiw, Speed};
//! use sdrkit_proto_pager::Kind;
//!
//! let tx = [FlexTx { capcode: 1_234_567, kind: Kind::Alpha, text: "Hello" }];
//! let mut words = [0u32; flex::PHASE_WORDS];
//! flex::encode_phase(&tx, &mut words).unwrap();
//! let mut dec = FlexDecoder::new(flex::Config::default()).unwrap();
//! let mut got = Vec::new();
//! let fiw = Fiw { cycle: 0, frame: 5, rest: 0 };
//! dec.decode_words(Speed::F1600_2, fiw, 0, &words, None, 0.0, |p| got.push(p.text.clone()));
//! assert_eq!(got, ["Hello"]);
//! ```

use crate::charset::{flex_value, FLEX_FILL, FLEX_NUMERIC};
use crate::page::{self, FlexInfo, Kind, MessageBits, Page, Protocol, Refusals};
use crate::vote::vote;
use crate::word::{self, Checked, SoftRepair};
use crate::{Error, Result, Setting};
use sdrkit_util_bits::checksum::{flex_check_ok, flex_with_check};
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use sdrkit_util_bits::sync::{SoftSync, SyncWord};

/// Words in a phase of one frame.
pub const PHASE_WORDS: usize = 88;
/// Bits in a phase of one frame.
pub const PHASE_BITS: usize = PHASE_WORDS * 32;
/// Blocks in a frame.
pub const BLOCKS: usize = 11;
/// Words per block and phase.
pub const BLOCK_WORDS: usize = 8;

/// The 16-bit marker after the speed code in word A of sync 1.
pub const MARKER: u16 = 0xA6C6;
/// Word B of sync 1 (16 bits).
pub const B_WORD: u16 = 0xAAAA;
/// Bits of sync 1 after its bit sync: A (32), B (16) and A inverted (32).
pub const SYNC1_BITS: u32 = 80;
/// Bits of the bit sync (reversals) at the start of sync 1.
pub const BIT_SYNC_BITS: usize = 32;
/// The value of an idle word (either value). Idle phases send 0 and 0x1FFFFF in turn.
pub const IDLE_VALUES: [u32; 2] = [0, 0x1F_FFFF];
/// The highest short-address capcode (0x1E0000 - 0x8000, 1 933 312).
pub const MAX_SHORT_CAPCODE: u32 = 0x1D_8000;
/// The lowest long-address capcode (set 1-2).
pub const MIN_LONG_CAPCODE: u32 = 2_101_249;
/// The lowest capcode of the long-address sets 1-3 and 1-4.
const MIN_SET_13_CAPCODE: u32 = 1_075_843_073;
/// The lowest capcode of the long-address set 2-3.
const MIN_SET_23_CAPCODE: u32 = 3_223_326_721;
/// The time of one frame, s (128 frames a cycle of 4 minutes).
pub const FRAME_S: f64 = 1.875;
/// Frames in the 15 cycles of an hour.
const FRAMES_AN_HOUR: u32 = 15 * 128;

/// A FLEX speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Speed {
    /// 1600 bit/s: 1600 symbols/s, 2 levels, phase A.
    #[default]
    F1600_2,
    /// 3200 bit/s: 3200 symbols/s, 2 levels, phases A and C on alternate symbols.
    F3200_2,
    /// 3200 bit/s: 1600 symbols/s, 4 levels, phases A and B in each symbol.
    F3200_4,
    /// 6400 bit/s: 3200 symbols/s, 4 levels, A and B on even symbols, C and D on odd symbols.
    F6400_4,
}

/// The sync 1 speed codes (the first 16 bits of word A).
pub const CODES: [(u16, Speed); 5] = [
    (0x870C, Speed::F1600_2),
    (0xB068, Speed::F3200_4),
    (0x7B18, Speed::F3200_2),
    (0xDEA0, Speed::F6400_4),
    (0x4C7C, Speed::F6400_4),
];

impl Speed {
    /// All four speeds.
    pub const ALL: [Speed; 4] = [
        Speed::F1600_2,
        Speed::F3200_2,
        Speed::F3200_4,
        Speed::F6400_4,
    ];

    /// "FLEX 1600/2" and so on.
    pub const fn name(self) -> &'static str {
        match self {
            Speed::F1600_2 => "FLEX 1600/2",
            Speed::F3200_2 => "FLEX 3200/2",
            Speed::F3200_4 => "FLEX 3200/4",
            Speed::F6400_4 => "FLEX 6400/4",
        }
    }

    /// Symbols per second of the blocks.
    pub const fn baud(self) -> u32 {
        match self {
            Speed::F1600_2 | Speed::F3200_4 => 1600,
            _ => 3200,
        }
    }

    /// FSK levels of the blocks (2 or 4).
    pub const fn levels(self) -> u8 {
        match self {
            Speed::F1600_2 | Speed::F3200_2 => 2,
            _ => 4,
        }
    }

    /// The letters of the phases, in the order the decoder numbers them.
    pub const fn phase_letters(self) -> &'static [char] {
        match self {
            Speed::F1600_2 => &['A'],
            Speed::F3200_2 => &['A', 'C'],
            Speed::F3200_4 => &['A', 'B'],
            Speed::F6400_4 => &['A', 'B', 'C', 'D'],
        }
    }

    /// The number of phases.
    pub const fn phases(self) -> usize {
        self.phase_letters().len()
    }

    /// Data symbols in a frame (11 blocks of 160 ms).
    pub const fn data_symbols(self) -> usize {
        BLOCKS * 160 * self.baud() as usize / 1000
    }

    /// Symbols of sync 2 (25 ms at the block speed).
    pub const fn sync2_symbols(self) -> usize {
        25 * self.baud() as usize / 1000
    }

    /// The first speed code of this speed.
    pub fn code(self) -> u16 {
        CODES.iter().find(|c| c.1 == self).map(|c| c.0).unwrap_or(0)
    }

    /// The speed whose code is nearest to `code`, within `max_errors` bits.
    pub fn from_code(code: u16, max_errors: u32) -> Option<Speed> {
        CODES
            .iter()
            .map(|&(c, s)| ((c ^ code).count_ones(), s))
            .filter(|&(d, _)| d <= max_errors)
            .min_by_key(|&(d, _)| d)
            .map(|(_, s)| s)
    }
}

/// The 80 bits of sync 1 after its bit sync, first sent most significant: A = code and marker, B, and A inverted.
pub const fn sync1_pattern(code: u16) -> u128 {
    let a = ((code as u32) << 16) | MARKER as u32;
    ((a as u128) << 48) | ((B_WORD as u128) << 32) | (!a) as u128
}

/// Finds sync 1 in bits at 1600 bit/s and returns the speed: hard bits within `max_errors`, or soft bits by normalised
/// correlation. It allocates only in `new`.
#[derive(Clone, Debug)]
pub struct Sync1Search {
    words: [SyncWord; 5],
    soft: [SoftSync; 5],
}

/// A sync 1 found by [`Sync1Search`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sync1 {
    /// The speed of the frame.
    pub speed: Speed,
    /// The pattern came inverted. The bits after it are inverted too.
    pub inverted: bool,
    /// Differing hard bits (the signs of the LLRs for soft input).
    pub errors: u32,
    /// The normalised correlation, 0 to 1 (1 for a hard match with no errors).
    pub score: f32,
}

impl Sync1Search {
    /// A search that allows `max_errors` differing hard bits of the 80 (0 to 39), and a soft correlation of at least
    /// `min_score` (0 to 1). An error for values out of these ranges.
    pub fn new(max_errors: u32, min_score: f32) -> Result<Sync1Search> {
        if max_errors > 39 {
            return Err(Error::BadSetting(Setting::FlexSyncErrors));
        }
        let mut words = Vec::with_capacity(5);
        let mut soft = Vec::with_capacity(5);
        for (c, _) in CODES {
            words.push(
                SyncWord::new(sync1_pattern(c), SYNC1_BITS, max_errors)
                    .map_err(|_| Error::BadSetting(Setting::FlexSyncErrors))?
                    .with_inverted(true),
            );
            soft.push(
                SoftSync::new(sync1_pattern(c), SYNC1_BITS, min_score)
                    .map_err(|_| Error::BadSetting(Setting::FlexSyncScore))?
                    .with_inverted(true),
            );
        }
        let words: [SyncWord; 5] = words.try_into().map_err(|_| Error::BadLength(5))?;
        let soft: [SoftSync; 5] = soft.try_into().map_err(|_| Error::BadLength(5))?;
        Ok(Sync1Search { words, soft })
    }

    /// Takes one hard bit. Reports sync 1 when it ends at this bit. The FIW follows.
    pub fn push(&mut self, bit: u8) -> Option<Sync1> {
        let mut best: Option<Sync1> = None;
        for (w, &(_, speed)) in self.words.iter_mut().zip(CODES.iter()) {
            if let Some(h) = w.push(bit) {
                if best.is_none_or(|b| h.errors() < b.errors) {
                    best = Some(Sync1 {
                        speed,
                        inverted: h.inverted(),
                        errors: h.errors(),
                        score: 1.0 - 2.0 * h.errors() as f32 / SYNC1_BITS as f32,
                    });
                }
            }
        }
        best
    }

    /// Takes one soft bit (an LLR, positive means 0). Reports sync 1 when it ends at this bit.
    pub fn push_llr(&mut self, llr: f32) -> Option<Sync1> {
        let l = if llr.is_nan() { 0.0 } else { llr };
        let bit = (l < 0.0) as u8;
        let mut best: Option<Sync1> = None;
        for ((w, s), &(_, speed)) in self
            .words
            .iter_mut()
            .zip(self.soft.iter_mut())
            .zip(CODES.iter())
        {
            let _ = w.push(bit);
            if let Some(h) = s.push(l) {
                let inverted = h.score < 0.0;
                let d = w.distance();
                let errors = if inverted { SYNC1_BITS - d } else { d };
                if best.is_none_or(|b| h.score.abs() > b.score) {
                    best = Some(Sync1 {
                        speed,
                        inverted,
                        errors,
                        score: h.score.abs(),
                    });
                }
            }
        }
        best
    }

    /// Forgets the bits seen.
    pub fn reset(&mut self) {
        for w in &mut self.words {
            w.reset();
        }
        for s in &mut self.soft {
            s.reset();
        }
    }
}

/// The FLEX value (21 bits, bit 0 the first sent) of a code word.
pub fn value(cw: u32) -> u32 {
    word::rev21(word::data(cw))
}

/// The code word of a FLEX value.
pub fn word(v: u32) -> u32 {
    word::encode(word::rev21(v))
}

/// True for the two idle values.
pub fn is_idle(v: u32) -> bool {
    IDLE_VALUES.contains(&v)
}

/// The frame information word (RCR STD-43A 3.6). Field layout: checksum (bits 0 to 3), cycle (4 to 7) and frame (8 to 14).
/// Bit 15 is the roaming network flag, bit 16 the multiple transmission flag and bits 17 to 20 the traffic or transmission
/// flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fiw {
    /// The cycle number (0 to 14).
    pub cycle: u8,
    /// The frame number (0 to 127).
    pub frame: u8,
    /// Bits 15 to 20, not interpreted.
    pub rest: u8,
}

impl Fiw {
    /// The FIW of a value, or `None` when the checksum fails or the cycle number is 15.
    pub fn from_value(v: u32) -> Option<Fiw> {
        (flex_check_ok(v) && (v >> 4) & 0xF < 15).then_some(Fiw {
            cycle: ((v >> 4) & 0xF) as u8,
            frame: ((v >> 8) & 0x7F) as u8,
            rest: ((v >> 15) & 0x3F) as u8,
        })
    }

    /// The FIW of a received word: hard repair, then the soft repair with `rel`, then the checksum.
    pub fn from_word(raw: u32, rel: Option<&[f32; 32]>, soft: SoftRepair) -> Option<Fiw> {
        let c = Checked::soft(raw, rel, soft);
        if !c.valid {
            return None;
        }
        Fiw::from_value(value(c.cw))
    }

    /// The value with its checksum.
    pub fn value(&self) -> u32 {
        flex_with_check(
            ((self.cycle as u32 & 0xF) << 4)
                | ((self.frame as u32 & 0x7F) << 8)
                | ((self.rest as u32 & 0x3F) << 15),
        )
    }

    /// The frame's place in the hour (0 to 1919).
    pub fn index(&self) -> u32 {
        (self.cycle as u32 % 15) * 128 + (self.frame as u32 & 0x7F)
    }
}

/// The first block information word of a phase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Biw {
    /// Priority addresses at the start of the address field (bits 4 to 7).
    pub priority: u8,
    /// The number of block information words (1 to 4, from bits 8 and 9).
    pub count: u8,
    /// The word where the vector field starts (bits 10 to 15).
    pub vector_start: u8,
    /// Carry-on (bits 16 and 17).
    pub carry_on: u8,
    /// The system collapse value (bits 18 to 20).
    pub collapse: u8,
}

impl Biw {
    /// The first block information word of a value, or `None` when the checksum fails or the value is idle.
    pub fn from_value(v: u32) -> Option<Biw> {
        if is_idle(v) || !flex_check_ok(v) {
            return None;
        }
        Some(Biw {
            priority: ((v >> 4) & 0xF) as u8,
            count: (((v >> 8) & 3) + 1) as u8,
            vector_start: ((v >> 10) & 0x3F) as u8,
            carry_on: ((v >> 16) & 3) as u8,
            collapse: ((v >> 18) & 7) as u8,
        })
    }

    /// The value with its checksum.
    pub fn value(&self) -> u32 {
        flex_with_check(
            ((self.priority as u32 & 0xF) << 4)
                | (((self.count.max(1) as u32 - 1) & 3) << 8)
                | ((self.vector_start as u32 & 0x3F) << 10)
                | ((self.carry_on as u32 & 3) << 16)
                | ((self.collapse as u32 & 7) << 18),
        )
    }
}

/// A later block information word (2 to 4): its function (bits 4 to 6) and the data bits 7 to 20, not interpreted. Public
/// descriptions name these functions: 0 local id and zone, 1 date, 2 time, 5 system message, 7 country and traffic management.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BiwExtra {
    /// The function (0 to 7).
    pub function: u8,
    /// Bits 7 to 20.
    pub data: u16,
}

impl BiwExtra {
    /// The word of a value, or `None` when the checksum fails.
    pub fn from_value(v: u32) -> Option<BiwExtra> {
        flex_check_ok(v).then_some(BiwExtra {
            function: ((v >> 4) & 7) as u8,
            data: ((v >> 7) & 0x3FFF) as u16,
        })
    }
}

/// A one-word address that is not an individual short address (RCR STD-43A table 3.8.1-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    /// 0x1F2800 to 0x1F67FF.
    InformationService,
    /// 0x1F6800 to 0x1F77FF.
    Network,
    /// 0x1F7800 to 0x1F780F.
    Temporary,
    /// 0x1F7810 to 0x1F781F.
    OperatorMessaging,
}

/// What an address word is (RCR STD-43A table 3.8.1-1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Address {
    /// An idle value (0 or 0x1FFFFF).
    Idle,
    /// A short address (0x8001 to 0x1E0000) with its capcode.
    Short(u32),
    /// The first word of a long address: long 1 (1 to 0x8000) or long 2 (0x1F7FFF to 0x1FFFFE). The next word completes it
    /// ([`long_capcode`]).
    LongFirst(u32),
    /// A value that is only the second word of a long address: long 3 (0x1E0001 to 0x1E8000) or long 4 (0x1E8001 to
    /// 0x1F0000).
    LongSecond(u32),
    /// A one-word address that is not decoded as a page.
    Special(Special, u32),
    /// A reserved short address (0x1F0001 to 0x1F27FF, 0x1F7820 to 0x1F7FFE).
    Reserved(u32),
}

/// The kind of an address value.
pub fn address(v: u32) -> Address {
    match v {
        0 | 0x1F_FFFF => Address::Idle,
        1..=0x8000 | 0x1F_7FFF..=0x1F_FFFE => Address::LongFirst(v),
        0x8001..=0x1E_0000 => Address::Short(v - 0x8000),
        0x1E_0001..=0x1F_0000 => Address::LongSecond(v),
        0x1F_2800..=0x1F_67FF => Address::Special(Special::InformationService, v),
        0x1F_6800..=0x1F_77FF => Address::Special(Special::Network, v),
        0x1F_7800..=0x1F_780F => Address::Special(Special::Temporary, v),
        0x1F_7810..=0x1F_781F => Address::Special(Special::OperatorMessaging, v),
        _ => Address::Reserved(v),
    }
}

/// The capcode of a long address from its two words (TI SPRA193 2.9.4). The sets are 1-2, 1-3 and 1-4 (a long 1 first word)
/// and 2-3 (a long 2 first word) (RCR STD-43A table 3.8.2.2-1). `None` for any other pair, for set 2-4, whose capcodes the
/// documents do not give, and for a set 2-3 capcode above 32 bits.
pub fn long_capcode(first: u32, second: u32) -> Option<u32> {
    // Capcode = base + first + 32768 x the second word's offset in its range.
    const BASE: u32 = 2_068_480;
    let long1 = (1..=0x8000).contains(&first);
    let long2 = (0x1F_7FFF..=0x1F_FFFE).contains(&first);
    let c = match second {
        0x1F_7FFF..=0x1F_FFFE if long1 => BASE + first + ((0x1F_FFFF - second) << 15),
        0x1E_0001..=0x1F_0000 if long1 => {
            ((second - MAX_SHORT_CAPCODE) << 15).checked_add(BASE + first)?
        }
        0x1E_0001..=0x1E_8000 if long2 => {
            let c = (u64::from(second - 1_867_776) << 15) + u64::from(first - 2_064_383 + BASE - 1);
            return u32::try_from(c).ok().filter(|&c| c >= MIN_SET_23_CAPCODE);
        }
        _ => return None,
    };
    Some(c)
}

/// The address words of a capcode (TI SPRA193 2.9.4). A short capcode (1 to [`MAX_SHORT_CAPCODE`]) takes one word. A long
/// capcode ([`MIN_LONG_CAPCODE`] or more) takes two words of set 1-2, 1-3 or 2-3. Capcodes between the two ranges are refused.
pub fn address_words(capcode: u32) -> Result<(u32, Option<u32>)> {
    if (1..=MAX_SHORT_CAPCODE).contains(&capcode) {
        return Ok((capcode + 0x8000, None));
    }
    if capcode < MIN_LONG_CAPCODE {
        return Err(Error::BadCapcode(capcode));
    }
    if capcode >= MIN_SET_23_CAPCODE {
        let y = capcode - 2_068_479;
        return Ok((2_064_383 + (y & 0x7FFF), Some(1_867_776 + (y >> 15))));
    }
    let x = capcode - 2_068_481;
    let first = (x & 0x7FFF) + 1;
    let second = if capcode < MIN_SET_13_CAPCODE {
        0x1F_FFFF - (x >> 15)
    } else {
        MAX_SHORT_CAPCODE + (x >> 15)
    };
    Ok((first, Some(second)))
}

/// A vector word (RCR STD-43A 3.9). Bits 0 to 3 are the checksum and bits 4 to 6 the type. `len` counts the words of the
/// message, the second vector word of a long address included.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vector {
    /// Type 0: a secure message in words `start` to `start + len`.
    Secure {
        /// The first message word.
        start: u8,
        /// Words (with the header).
        len: u8,
    },
    /// Type 1: an instruction (for example a temporary address). The value is kept.
    Instruction(u32),
    /// Type 2: a short message. Sub-type 0 holds three numeric characters. Other sub-types are alerts.
    Short {
        /// The sub-type (bits 7 and 8).
        sub: u8,
        /// Bits 9 to 20.
        data: u16,
    },
    /// Types 3, 4 and 7: numeric messages.
    Numeric {
        /// The type (3 standard, 4 special format, 7 numbered).
        kind: u8,
        /// The first message word.
        start: u8,
        /// Message words (1 to 8).
        count: u8,
        /// K0 to K3 of the 6-bit message check (bits 17 to 20).
        check: u8,
    },
    /// Type 5: text.
    Alpha {
        /// The first message word (the header).
        start: u8,
        /// Words with the header.
        len: u8,
    },
    /// Type 6: binary data (HEX).
    Binary {
        /// The first message word (the header).
        start: u8,
        /// Words with the header.
        len: u8,
    },
}

impl Vector {
    /// The vector of a value, or `None` when the checksum fails.
    pub fn from_value(v: u32) -> Option<Vector> {
        if !flex_check_ok(v) {
            return None;
        }
        let start = ((v >> 7) & 0x7F) as u8;
        let len = ((v >> 14) & 0x7F) as u8;
        Some(match (v >> 4) & 7 {
            0 => Vector::Secure { start, len },
            1 => Vector::Instruction(v),
            2 => Vector::Short {
                sub: ((v >> 7) & 3) as u8,
                data: ((v >> 9) & 0xFFF) as u16,
            },
            5 => Vector::Alpha { start, len },
            6 => Vector::Binary { start, len },
            k => Vector::Numeric {
                kind: k as u8,
                start,
                count: (((v >> 14) & 7) + 1) as u8,
                check: ((v >> 17) & 0xF) as u8,
            },
        })
    }

    /// The type number (0 to 7).
    pub fn kind(&self) -> u8 {
        match *self {
            Vector::Secure { .. } => 0,
            Vector::Instruction(_) => 1,
            Vector::Short { .. } => 2,
            Vector::Numeric { kind, .. } => kind,
            Vector::Alpha { .. } => 5,
            Vector::Binary { .. } => 6,
        }
    }

    /// The value with its checksum.
    pub fn value(&self) -> u32 {
        let v = match *self {
            Vector::Secure { start, len } => ((start as u32) << 7) | ((len as u32 & 0x7F) << 14),
            Vector::Instruction(v) => v & !0x7F | (1 << 4),
            Vector::Short { sub, data } => {
                (2 << 4) | ((sub as u32 & 3) << 7) | ((data as u32 & 0xFFF) << 9)
            }
            Vector::Numeric {
                kind,
                start,
                count,
                check,
            } => {
                ((kind as u32 & 7) << 4)
                    | ((start as u32 & 0x7F) << 7)
                    | (((count.max(1) as u32 - 1) & 7) << 14)
                    | ((check as u32 & 0xF) << 17)
            }
            Vector::Alpha { start, len } => {
                (5 << 4) | ((start as u32 & 0x7F) << 7) | ((len as u32 & 0x7F) << 14)
            }
            Vector::Binary { start, len } => {
                (6 << 4) | ((start as u32 & 0x7F) << 7) | ((len as u32 & 0x7F) << 14)
            }
        };
        flex_with_check(v)
    }
}

/// The header word of a text or secure message (RCR STD-43A 3.10.1.3). Layout: fragment check (bits 0 to 9), continued (10),
/// fragment number (11 and 12), message number (13 to 18), retrieval (19), maildrop (20). In a later text fragment bits 19 and
/// 20 are the fragment control U and V.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Header {
    /// The 10-bit fragment check ([`fragment_check`]).
    pub check: u16,
    /// More fragments follow.
    pub continued: bool,
    /// The fragment number. 3 is the first fragment, which starts with a signature character.
    pub fragment: u8,
    /// The message number.
    pub number: u8,
    /// The retrieval flag.
    pub retrieval: bool,
    /// The maildrop flag.
    pub maildrop: bool,
}

impl Header {
    /// The header of a value.
    pub fn from_value(v: u32) -> Header {
        Header {
            check: (v & 0x3FF) as u16,
            continued: (v >> 10) & 1 == 1,
            fragment: ((v >> 11) & 3) as u8,
            number: ((v >> 13) & 0x3F) as u8,
            retrieval: (v >> 19) & 1 == 1,
            maildrop: (v >> 20) & 1 == 1,
        }
    }

    /// The value.
    pub fn value(&self) -> u32 {
        (self.check as u32 & 0x3FF)
            | ((self.continued as u32) << 10)
            | ((self.fragment as u32 & 3) << 11)
            | ((self.number as u32 & 0x3F) << 13)
            | ((self.retrieval as u32) << 19)
            | ((self.maildrop as u32) << 20)
    }
}

/// The sum of the three groups of a word: bits 0 to 7, 8 to 15 and 16 to 20, each read with its lowest bit as the least
/// significant (RCR STD-43A 3.10).
pub fn group_sum(v: u32) -> u32 {
    (v & 0xFF) + ((v >> 8) & 0xFF) + ((v >> 16) & 0x1F)
}

/// The fragment check of a text or secure message (`bits` = 10) or a binary message (`bits` = 12): the ones' complement of the
/// group sums of every word of the fragment, the header `words[0]` with its check field as zero, low `bits` bits (RCR
/// STD-43A 3.10.1.2 to 3.10.1.4). The vector word is not in the sum.
pub fn fragment_check(words: &[u32], bits: u32) -> u16 {
    let mask = (1u32 << bits) - 1;
    let sum: u32 = words
        .iter()
        .enumerate()
        .map(|(i, &w)| group_sum(if i == 0 { w & !mask } else { w }))
        .sum();
    (!sum & mask) as u16
}

/// The 6-bit check of a numeric message (RCR STD-43A 3.10.1.1.1): the group sums of the message words, the first with K4 and K5
/// (bits 0 and 1) as zero; the low 8 bits of the sum; its top 2 bits added to its low 6; the ones' complement, 6 bits. K0 to K3
/// are the low 4 bits.
pub fn numeric_check(words: &[u32]) -> u8 {
    let s: u32 = words
        .iter()
        .enumerate()
        .map(|(i, &w)| group_sum(if i == 0 { w & !3 } else { w }))
        .sum::<u32>()
        & 0xFF;
    (!((s & 0x3F) + (s >> 6)) & 0x3F) as u8
}

/// The 7-bit signature of a text message: the ones' complement of the sum of its 7-bit characters after the signature, over
/// all fragments, ETX and NUL left out (RCR STD-43A 3.10.1.3).
pub fn signature(chars: impl IntoIterator<Item = u8>) -> u8 {
    let s: u32 = chars
        .into_iter()
        .filter(|&c| c != 0 && c != 3)
        .map(u32::from)
        .sum();
    (!s & 0x7F) as u8
}

/// Whether the vector of an address at word `vi` breaks a rule of its values: `None` when it does not, else whether the rule is
/// one that no public document states. `field_end` is the first word after the vector field.
fn vector_refusal(vec: Vector, vi: usize, long: bool, field_end: usize) -> Option<bool> {
    let (start, words) = match vec {
        // Short instructions 010 to 110 are reserved.
        Vector::Instruction(v) => return (2..=6).contains(&((v >> 7) & 7)).then_some(true),
        // Short message type 11 is reserved.
        Vector::Short { sub, .. } => return (sub == 3).then_some(true),
        Vector::Numeric { start, count, .. } => {
            if long && count == 1 {
                // The message is the second vector word, and the start word points at it.
                return (start as usize != vi + 1).then_some(false);
            }
            (start as usize, (count as usize) - long as usize)
        }
        Vector::Secure { start, len }
        | Vector::Alpha { start, len }
        | Vector::Binary { start, len } => {
            if !(2..=85).contains(&len) {
                return Some(false);
            }
            (start as usize, len as usize - long as usize)
        }
    };
    if !(3..=87).contains(&start) {
        return Some(false);
    }
    (start < field_end || start + words > PHASE_WORDS).then_some(true)
}

/// The interleaving of a phase. Each block of 256 bits holds 8 words, written as rows of 32 bits and sent column by column.
/// Successive bits of a word are thus 8 bits (5 ms at 1600 bit/s) apart.
pub fn phase_permutation() -> &'static Permutation {
    static P: std::sync::OnceLock<Permutation> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let block = Permutation::row_column(BLOCK_WORDS, 32, ReadOrder::Columns)
            .expect("8 x 32 is a valid matrix");
        Permutation::from_fn(PHASE_BITS, |q| {
            let (b, i) = (q / 256, q % 256);
            b * 256 + block.dest(i).unwrap_or(i)
        })
        .expect("the per-block permutation is a bijection")
    })
}

/// Splits the data symbols of a frame into phases as LLRs (positive means 0). `y` holds the symbols normalised so that the
/// outer levels are ±1 (and the inner levels of 4-level FSK ±1/3). A positive symbol is the upper frequency. `phases[p]` must
/// hold [`PHASE_BITS`] values for each phase of the speed.
///
/// The bit mapping follows public descriptions. With 2 levels a bit 1 is the upper level. With 4 levels the first bit (phase A or C) is 1
/// for the two upper levels. The second bit (B or D) is 1 for the two inner levels. This is Gray: -3 = 00, -1 = 01, +1 = 11,
/// +3 = 10.
/// The LLR of a 2-level bit is `-y`. The LLRs of a 4-level symbol are `-3 y` and `3 (|y| - 2/3)`.
pub fn split_phases(speed: Speed, y: &[f32], phases: &mut [&mut [f32]]) -> Result<()> {
    if y.len() != speed.data_symbols() {
        return Err(Error::BadLength(speed.data_symbols()));
    }
    if phases.len() < speed.phases()
        || phases
            .iter()
            .take(speed.phases())
            .any(|p| p.len() != PHASE_BITS)
    {
        return Err(Error::BadLength(PHASE_BITS));
    }
    let two = |v: f32| -v;
    let four = |v: f32| (-3.0 * v, 3.0 * (v.abs() - 2.0 / 3.0));
    match speed {
        Speed::F1600_2 => {
            for (o, &v) in phases[0].iter_mut().zip(y) {
                *o = two(v);
            }
        }
        Speed::F3200_2 => {
            for (i, pair) in y.as_chunks::<2>().0.iter().enumerate() {
                phases[0][i] = two(pair[0]);
                phases[1][i] = two(pair[1]);
            }
        }
        Speed::F3200_4 => {
            for (i, &v) in y.iter().enumerate() {
                let (a, b) = four(v);
                phases[0][i] = a;
                phases[1][i] = b;
            }
        }
        Speed::F6400_4 => {
            for (i, pair) in y.as_chunks::<2>().0.iter().enumerate() {
                let (a, b) = four(pair[0]);
                let (c, d) = four(pair[1]);
                phases[0][i] = a;
                phases[1][i] = b;
                phases[2][i] = c;
                phases[3][i] = d;
            }
        }
    }
    Ok(())
}

/// The level (-3, -1, +1 or +3) of a 4-level symbol carrying bit `a` (phase A or C) and bit `b` (phase B or D).
/// mapping (see [`split_phases`]).
pub const fn level4(a: u8, b: u8) -> i8 {
    match (a & 1, b & 1) {
        (0, 0) => -3,
        (0, _) => -1,
        (_, 1) => 1,
        _ => 3,
    }
}

/// The settings of a FLEX decoder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// The soft repair of block information, address and vector words.
    pub header_soft: SoftRepair,
    /// The soft repair of message words.
    pub message_soft: SoftRepair,
    /// Vote lost words against earlier copies of the same phase.
    pub combine: bool,
    /// Copies older than this (seconds) do not take part in a vote.
    pub max_age_s: f64,
    /// Phases kept for voting.
    pub history: usize,
    /// Join text fragments into one page.
    pub join_fragments: bool,
    /// Messages waiting for their next fragment.
    pub fragment_slots: usize,
    /// A waiting fragment older than this (seconds) is given as a damaged page.
    pub fragment_age_s: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            header_soft: SoftRepair::HEADER,
            message_soft: SoftRepair::MESSAGE,
            combine: true,
            max_age_s: 900.0,
            history: 48,
            join_fragments: true,
            fragment_slots: 16,
            fragment_age_s: 240.0,
        }
    }
}

#[derive(Clone, Debug)]
struct PhaseCopy {
    used: bool,
    speed: Speed,
    phase: u8,
    time: f64,
    words: [(u32, bool); PHASE_WORDS],
}

#[derive(Clone, Debug)]
struct Fragment {
    used: bool,
    capcode: u32,
    number: u8,
    time: f64,
    rate: &'static str,
    count: u8,
    damaged: bool,
    /// The fragment number that comes next.
    next: u8,
    /// The signature of the first fragment, when it was checked.
    signature: Option<u8>,
    bits_repaired: u32,
    lost_words: u32,
    raw: Vec<u8>,
}

/// Counts of the words of every phase since the decoder was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WordCounts {
    /// Words that passed as received.
    pub good: u64,
    /// Words that a repair saved.
    pub repaired: u64,
    /// Words that were lost.
    pub lost: u64,
}

/// The longest joined text in characters.
const MAX_TEXT: usize = 4096;

/// Decodes FLEX phases into pages, with voting of repeated phases and joining of text fragments. It allocates only in `new`.
#[derive(Clone, Debug)]
pub struct FlexDecoder {
    cfg: Config,
    hard: Vec<u8>,
    soft: Vec<f32>,
    words: [u32; PHASE_WORDS],
    rels: Vec<[f32; 32]>,
    checked: [Checked; PHASE_WORDS],
    history: Vec<PhaseCopy>,
    fragments: Vec<Fragment>,
    bits: MessageBits,
    page: Page,
    spare: Page,
    counts: WordCounts,
    /// The words of the message being read.
    msg: Vec<Checked>,
    refusals: Refusals,
    /// The FIW and time of the last frame.
    frame: Option<(Fiw, f64)>,
    /// The frame number disagrees with the frame before it.
    unconfirmed_frame: bool,
}

impl FlexDecoder {
    /// A decoder with every buffer sized from `cfg`. An error when `history` or `fragment_slots` is above 1024, or an age or a
    /// soft limit is not a finite number of 0 or more.
    pub fn new(cfg: Config) -> Result<FlexDecoder> {
        if cfg.history > 1024 || cfg.fragment_slots > 1024 {
            return Err(Error::BadSetting(Setting::FlexPhasesKept));
        }
        let finite = |v: f64| v.is_finite() && v >= 0.0;
        if !finite(cfg.max_age_s)
            || !finite(cfg.fragment_age_s)
            || !cfg.header_soft.doubt.is_finite()
            || !cfg.message_soft.doubt.is_finite()
        {
            return Err(Error::BadSetting(Setting::AgeAndSoftLimits));
        }
        let _ = phase_permutation();
        let mut page = Page {
            text: String::with_capacity(MAX_TEXT),
            numeric: String::with_capacity(64),
            alpha: String::with_capacity(MAX_TEXT),
            raw: Vec::with_capacity(MAX_TEXT),
            data: Vec::with_capacity(PHASE_WORDS),
            ..Page::default()
        };
        page.clear();
        let spare = page.clone();
        Ok(FlexDecoder {
            hard: vec![0; PHASE_BITS],
            soft: vec![0.0; PHASE_BITS],
            words: [0; PHASE_WORDS],
            rels: vec![[0.0; 32]; PHASE_WORDS],
            checked: [Checked::default(); PHASE_WORDS],
            history: (0..cfg.history)
                .map(|_| PhaseCopy {
                    used: false,
                    speed: Speed::F1600_2,
                    phase: 0,
                    time: 0.0,
                    words: [(0, false); PHASE_WORDS],
                })
                .collect(),
            fragments: (0..cfg.fragment_slots)
                .map(|_| Fragment {
                    used: false,
                    capcode: 0,
                    number: 0,
                    time: 0.0,
                    rate: "",
                    count: 0,
                    damaged: false,
                    next: 3,
                    signature: None,
                    bits_repaired: 0,
                    lost_words: 0,
                    raw: Vec::with_capacity(MAX_TEXT),
                })
                .collect(),
            bits: MessageBits::with_capacity(PHASE_BITS),
            page,
            spare,
            counts: WordCounts::default(),
            msg: Vec::with_capacity(PHASE_WORDS),
            refusals: Refusals::default(),
            frame: None,
            unconfirmed_frame: false,
            cfg,
        })
    }

    /// The word counts.
    pub fn counts(&self) -> WordCounts {
        self.counts
    }

    /// Forgets the voting history and the waiting fragments.
    pub fn clear(&mut self) {
        for c in &mut self.history {
            c.used = false;
        }
        for f in &mut self.fragments {
            f.used = false;
        }
        self.counts = WordCounts::default();
        self.refusals = Refusals::default();
        self.frame = None;
        self.unconfirmed_frame = false;
    }

    /// The pages refused by the acceptance rules (module documentation).
    pub fn refusals(&self) -> Refusals {
        self.refusals
    }

    /// Compares the FIW of a new frame with the frame before it. When that frame came 1 or 2 frame times earlier, the frame
    /// number must have advanced by as many frames, or the frame is unconfirmed. The phases of one frame share FIW and time.
    fn frame_check(&mut self, fiw: Fiw, time: f64) {
        if let Some((f, t)) = self.frame {
            if f == fiw && (time - t).abs() < 0.5 {
                return;
            }
            let dt = time - t;
            let k = (dt / FRAME_S).round();
            let near = (1.0..=2.0).contains(&k) && (dt - k * FRAME_S).abs() < 0.1;
            self.unconfirmed_frame = near && fiw.index() != (f.index() + k as u32) % FRAMES_AN_HOUR;
        } else {
            self.unconfirmed_frame = false;
        }
        self.frame = Some((fiw, time));
    }

    /// Decodes one phase from its [`PHASE_BITS`] LLRs (positive means 0) in the order sent. Pages go to `sink`.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_llrs(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        llr: &[f32],
        time: f64,
        sink: impl FnMut(&Page),
    ) -> Result<()> {
        if llr.len() != PHASE_BITS {
            return Err(Error::BadLength(PHASE_BITS));
        }
        phase_permutation()
            .invert(llr, &mut self.soft)
            .map_err(|_| Error::BadLength(PHASE_BITS))?;
        for k in 0..PHASE_WORDS {
            let mut w = 0u32;
            for j in 0..32 {
                let l = self.soft[k * 32 + j];
                w = (w << 1) | (l < 0.0) as u32;
                self.rels[k][j] = if l.is_nan() { 0.0 } else { l.abs() };
            }
            self.words[k] = w;
        }
        let words = self.words;
        let rels = std::mem::take(&mut self.rels);
        self.decode_inner(speed, fiw, phase, &words, Some(&rels), time, sink);
        self.rels = rels;
        Ok(())
    }

    /// Decodes one phase from its [`PHASE_BITS`] hard bits in the order sent. Pages go to `sink`.
    pub fn decode_bits(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        bits: &[u8],
        time: f64,
        sink: impl FnMut(&Page),
    ) -> Result<()> {
        if bits.len() != PHASE_BITS {
            return Err(Error::BadLength(PHASE_BITS));
        }
        phase_permutation()
            .invert(bits, &mut self.hard)
            .map_err(|_| Error::BadLength(PHASE_BITS))?;
        for k in 0..PHASE_WORDS {
            self.words[k] = self.hard[k * 32..k * 32 + 32]
                .iter()
                .fold(0u32, |a, &b| (a << 1) | (b & 1) as u32);
        }
        let words = self.words;
        self.decode_inner(speed, fiw, phase, &words, None, time, sink);
        Ok(())
    }

    /// Decodes one phase from its 88 code words (deinterleaved), with optional reliabilities per word. Pages go to `sink`.
    #[allow(clippy::too_many_arguments)]
    pub fn decode_words(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        words: &[u32; PHASE_WORDS],
        rel: Option<&[[f32; 32]]>,
        time: f64,
        sink: impl FnMut(&Page),
    ) {
        let rel = rel.filter(|r| r.len() >= PHASE_WORDS);
        self.decode_inner(speed, fiw, phase, words, rel, time, sink);
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_inner(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        words: &[u32; PHASE_WORDS],
        rel: Option<&[[f32; 32]]>,
        time: f64,
        mut sink: impl FnMut(&Page),
    ) {
        self.expire_fragments(time, &mut sink);
        self.frame_check(fiw, time);
        // The first block information word decides which words are header words.
        let biw_word = Checked::soft(words[0], rel.map(|r| &r[0]), self.cfg.header_soft);
        let (a0, v0) = match biw_word
            .valid
            .then(|| Biw::from_value(value(biw_word.cw)))
            .flatten()
        {
            Some(b) => (
                b.count as usize,
                (b.vector_start as usize).clamp(b.count as usize, PHASE_WORDS),
            ),
            None => (1, PHASE_WORDS),
        };
        self.checked[0] = biw_word;
        for k in 1..PHASE_WORDS {
            // Address words take the header limits. Later block information, vector and message words take the message limits.
            let header = (a0..v0).contains(&k);
            let rules = if header {
                self.cfg.header_soft
            } else {
                self.cfg.message_soft
            };
            self.checked[k] = Checked::soft(words[k], rel.map(|r| &r[k]), rules);
        }
        for c in &self.checked {
            if c.clean() {
                self.counts.good += 1;
            } else if c.valid {
                self.counts.repaired += 1;
            } else {
                self.counts.lost += 1;
            }
        }
        let combined = self.cfg.combine && self.combine(speed, phase, time);
        self.pages(speed, fiw, phase, time, combined, &mut sink);
    }

    /// Votes lost words against earlier copies of this phase. Returns true when a word came back.
    fn combine(&mut self, speed: Speed, phase: u8, time: f64) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let max_age = self.cfg.max_age_s;
        let mut found = [usize::MAX; 4];
        let mut nf = 0;
        for (i, c) in self.history.iter_mut().enumerate() {
            if c.used && (time - c.time > max_age || time < c.time - max_age) {
                c.used = false;
            }
            if !c.used || c.speed != speed || c.phase != phase {
                continue;
            }
            let (mut eq, mut diff) = (0, 0);
            for (a, b) in self.checked.iter().zip(&c.words) {
                if a.valid && b.1 {
                    if a.cw == b.0 {
                        eq += 1;
                    } else {
                        diff += 1;
                    }
                }
            }
            if eq >= 16 && diff * 10 <= eq && nf < 4 {
                found[nf] = i;
                nf += 1;
            }
        }
        let mut recovered = false;
        for k in 0..PHASE_WORDS {
            if nf == 0 || self.checked[k].valid {
                continue;
            }
            let mut cands = [Checked::default(); 5];
            cands[0] = self.checked[k];
            for (j, &i) in found[..nf].iter().enumerate() {
                let (w, ok) = self.history[i].words[k];
                cands[j + 1] = if ok {
                    Checked {
                        cw: w,
                        raw: w,
                        valid: true,
                        fixes: 0,
                    }
                } else {
                    Checked::lost(w)
                };
            }
            if let Some(v) = vote(&cands[..nf + 1]) {
                self.checked[k] = Checked {
                    raw: self.checked[k].raw,
                    ..v
                };
                recovered = true;
            }
        }
        let slot = self.history.iter().position(|c| !c.used).or_else(|| {
            self.history
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.time.total_cmp(&b.1.time))
                .map(|(i, _)| i)
        });
        if let Some(i) = slot {
            let c = &mut self.history[i];
            c.used = true;
            c.speed = speed;
            c.phase = phase;
            c.time = time;
            for (d, s) in c.words.iter_mut().zip(&self.checked) {
                *d = (s.cw, s.valid);
            }
        }
        recovered
    }

    #[allow(clippy::too_many_arguments)]
    fn start_page(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        time: f64,
        capcode: u32,
        long: bool,
        vector: u8,
    ) {
        let p = &mut self.page;
        p.clear();
        p.protocol = Protocol::Flex;
        p.rate = speed.name();
        p.time = time;
        p.address = capcode;
        p.flex = Some(FlexInfo {
            phase,
            cycle: fiw.cycle,
            frame: fiw.frame,
            vector,
            long_address: long,
            fragments: 1,
            ..FlexInfo::default()
        });
    }

    /// Steps 3 to 8: block information, addresses, vectors and messages.
    fn pages(
        &mut self,
        speed: Speed,
        fiw: Fiw,
        phase: u8,
        time: f64,
        combined: bool,
        sink: &mut impl FnMut(&Page),
    ) {
        let biw = self.checked[0];
        let Some(b) = biw.valid.then(|| Biw::from_value(value(biw.cw))).flatten() else {
            return;
        };
        let a0 = b.count as usize;
        let v0 = b.vector_start as usize;
        if v0 <= a0 || v0 >= PHASE_WORDS {
            return;
        }
        let n_addr = v0 - a0;
        // The vector field holds one word for each address word; the message field follows it.
        let field_end = v0 + n_addr;
        let mut k = 0;
        while k < n_addr {
            let aw = self.checked[a0 + k];
            if !aw.valid {
                k += 1;
                continue;
            }
            let (capcode, long, fixes) = match address(value(aw.cw)) {
                Address::Short(c) => (c, false, aw.fixes as u32),
                Address::LongFirst(v1) => {
                    if k + 1 >= n_addr {
                        break;
                    }
                    let w2 = self.checked[a0 + k + 1];
                    if !w2.valid {
                        k += 2;
                        continue;
                    }
                    match long_capcode(v1, value(w2.cw)) {
                        Some(c) => (c, true, (aw.fixes + w2.fixes) as u32),
                        None => {
                            // Not one of the long-address sets: both words and both vector words are skipped.
                            self.refusals.count(false);
                            k += 2;
                            continue;
                        }
                    }
                }
                Address::Idle
                | Address::LongSecond(_)
                | Address::Special(..)
                | Address::Reserved(_) => {
                    k += 1;
                    continue;
                }
            };
            let vi = v0 + k;
            k += if long { 2 } else { 1 };
            if vi >= PHASE_WORDS {
                break;
            }
            let vw = self.checked[vi];
            if !vw.valid {
                continue;
            }
            let Some(vec) = Vector::from_value(value(vw.cw)) else {
                continue;
            };
            if let Some(unconfirmed) = vector_refusal(vec, vi, long, field_end) {
                self.refusals.count(unconfirmed);
                continue;
            }
            let second = (long && vi + 1 < PHASE_WORDS).then(|| self.checked[vi + 1]);
            self.start_page(speed, fiw, phase, time, capcode, long, vec.kind());
            self.page.bits_repaired = fixes + vw.fixes as u32;
            self.page.combined = combined;
            if combined {
                self.page.copies = 2;
            }
            if self.message(vec, second, time, sink) {
                sink(&self.page);
            }
        }
    }

    /// Collects the words of a message in `msg`: the second vector word of a long address, then `n` words from `start`.
    fn gather(&mut self, second: Option<Checked>, start: usize, n: usize) {
        self.msg.clear();
        self.msg.extend(second);
        for i in start..start + n {
            let w = self.checked.get(i).copied();
            self.msg.push(w.unwrap_or(Checked::lost(0)));
        }
    }

    /// The values of the words of `msg` in `out`, or `None` when a word is lost.
    fn msg_values(&self, out: &mut [u32; PHASE_WORDS]) -> Option<usize> {
        for (o, w) in out.iter_mut().zip(&self.msg) {
            if !w.valid {
                return None;
            }
            *o = value(w.cw);
        }
        Some(self.msg.len())
    }

    /// Counts a refused page and returns false (no page).
    fn refuse(&mut self, unconfirmed: bool) -> bool {
        self.refusals.count(unconfirmed);
        false
    }

    /// Fills the page from its vector and applies the message checks. Returns true when the page is ready now, false while a
    /// fragment waits or when the page is refused.
    fn message(
        &mut self,
        vec: Vector,
        second: Option<Checked>,
        time: f64,
        sink: &mut impl FnMut(&Page),
    ) -> bool {
        let shaky = self.unconfirmed_frame;
        let long = second.is_some() as usize;
        let mut vals = [0u32; PHASE_WORDS];
        self.bits.clear();
        match vec {
            Vector::Instruction(v) => {
                if shaky {
                    return self.refuse(true);
                }
                self.page.kind = Kind::Instruction;
                self.page.data.push(v);
                true
            }
            Vector::Short { sub, data } => {
                if shaky {
                    return self.refuse(true);
                }
                let p = &mut self.page;
                if sub == 0 {
                    p.kind = Kind::Numeric;
                    for s in [0, 4, 8] {
                        let d = ((data >> s) & 0xF) as u8;
                        if d != FLEX_FILL {
                            p.numeric.push(FLEX_NUMERIC[d as usize] as char);
                        }
                    }
                    if let Some(v2) = second {
                        if v2.valid {
                            let x = value(v2.cw);
                            for s in [0, 4, 8, 12, 16] {
                                let d = ((x >> s) & 0xF) as u8;
                                if d != FLEX_FILL {
                                    p.numeric.push(FLEX_NUMERIC[d as usize] as char);
                                }
                            }
                        } else {
                            p.damaged = true;
                        }
                    }
                    let t = p.numeric.trim_end_matches(' ').len();
                    p.numeric.truncate(t);
                    p.text.push_str(&p.numeric);
                } else {
                    p.kind = Kind::Tone;
                    p.data.push(data as u32 | (sub as u32) << 12);
                }
                true
            }
            Vector::Numeric {
                kind,
                start,
                count,
                check,
            } => {
                // A one-word message of a long address is the second vector word alone.
                self.gather(second, start as usize, count as usize - long);
                match self.msg_values(&mut vals) {
                    Some(n) => {
                        let sent = check | ((vals[0] & 3) as u8) << 4;
                        if numeric_check(&vals[..n]) != sent {
                            return self.refuse(false);
                        }
                    }
                    None if shaky => return self.refuse(true),
                    None => {}
                }
                let p = &mut self.page;
                for w in &self.msg {
                    if w.valid {
                        p.bits_repaired += w.fixes as u32;
                        self.bits.push_lsb(value(w.cw), 21);
                    } else {
                        p.damaged = true;
                        p.lost_words += 1;
                        self.bits.push_lost(21);
                    }
                }
                let skip = if kind == 7 { 10 } else { 2 };
                p.kind = Kind::Numeric;
                self.bits
                    .numeric_into(skip, FLEX_NUMERIC, Some(FLEX_FILL), &mut p.numeric);
                p.text.push_str(&p.numeric);
                true
            }
            Vector::Secure { start, len } | Vector::Binary { start, len } => {
                let binary = matches!(vec, Vector::Binary { .. });
                self.gather(second, start as usize, len as usize - long);
                let mut head = None;
                match self.msg_values(&mut vals) {
                    Some(n) => {
                        let h = vals[0];
                        let (bits, fragment) = if binary {
                            (12, (h >> 13) & 3)
                        } else {
                            (10, (h >> 11) & 3)
                        };
                        if u32::from(fragment_check(&vals[..n], bits)) != h & ((1 << bits) - 1) {
                            return self.refuse(false);
                        }
                        // A binary first fragment: bits 8 to 12 of its second word are reserved as zero.
                        if binary && fragment == 3 && (vals[1] >> 8) & 0x1F != 0 {
                            return self.refuse(false);
                        }
                        head = Some(h);
                    }
                    None if shaky => return self.refuse(true),
                    None => {}
                }
                let p = &mut self.page;
                p.kind = if binary { Kind::Binary } else { Kind::Secure };
                for w in &self.msg {
                    if w.valid {
                        p.data.push(value(w.cw));
                    } else {
                        p.damaged = true;
                        p.lost_words += 1;
                        p.data.push(0);
                    }
                }
                if let (Some(h), Some(f)) = (head.or(p.data.first().copied()), p.flex.as_mut()) {
                    (f.continued, f.fragment, f.message_number) = if binary {
                        (
                            (h >> 12) & 1 == 1,
                            ((h >> 13) & 3) as u8,
                            ((h >> 15) & 0x3F) as u8,
                        )
                    } else {
                        let h = Header::from_value(h);
                        (h.continued, h.fragment, h.number)
                    };
                }
                true
            }
            Vector::Alpha { start, len } => {
                self.gather(second, start as usize, len as usize - long);
                let checked = self.msg_values(&mut vals);
                let mut signature_sent = None;
                match checked {
                    Some(n) => {
                        let h = Header::from_value(vals[0]);
                        if fragment_check(&vals[..n], 10) != h.check {
                            return self.refuse(false);
                        }
                        // The fragment control U, V of a later fragment: 0, 1 is reserved.
                        if h.fragment != 3 && (vals[0] >> 19) & 3 == 0b10 {
                            return self.refuse(false);
                        }
                        if h.fragment == 3 {
                            signature_sent = Some((vals[1] & 0x7F) as u8);
                        }
                    }
                    None if shaky => return self.refuse(true),
                    None => {}
                }
                let p = &mut self.page;
                let head = match self.msg.first() {
                    Some(h) if h.valid => {
                        p.bits_repaired += h.fixes as u32;
                        Some(Header::from_value(value(h.cw)))
                    }
                    _ => {
                        p.damaged = true;
                        None
                    }
                };
                for w in self.msg.iter().skip(1) {
                    if w.valid {
                        p.bits_repaired += w.fixes as u32;
                        self.bits.push_lsb(value(w.cw), 21);
                    } else {
                        p.damaged = true;
                        p.lost_words += 1;
                        self.bits.push_lost(21);
                    }
                }
                // Each word holds three 7-bit characters at bits 0, 7 and 14. A first fragment starts with the signature,
                // which is not text.
                let skip_signature = head.is_some_and(|h| h.fragment == 3);
                let n_words = self.bits.len() / 21;
                for wi in 0..n_words {
                    let base = wi * 21;
                    let lost = self.bits.lost[base] != 0;
                    for (n, s) in [0usize, 7, 14].iter().enumerate() {
                        if wi == 0 && n == 0 && skip_signature {
                            continue;
                        }
                        let c = self.bits.bits[base + s..base + s + 7]
                            .iter()
                            .enumerate()
                            .fold(0u8, |a, (i, &b)| a | (b << i));
                        if lost {
                            p.raw.push(c | 0x80);
                        } else if c != 0x03 {
                            p.raw.push(c);
                        }
                    }
                }
                p.kind = Kind::Alpha;
                if let (Some(h), Some(f)) = (head, p.flex.as_mut()) {
                    f.fragment = h.fragment;
                    f.continued = h.continued;
                    f.message_number = h.number;
                }
                if let Some(h) = head {
                    // A whole message in one fragment: its signature.
                    if !h.continued
                        && signature_sent.is_some_and(|s| s != signature(p.raw.iter().copied()))
                    {
                        return self.refuse(false);
                    }
                    if self.cfg.join_fragments {
                        return self.join(h, signature_sent, time, sink);
                    }
                }
                page::printable_into(&self.page.raw, &mut self.page.alpha);
                self.page.text.push_str(&self.page.alpha);
                true
            }
        }
    }

    /// Joins a text fragment with the fragments before it. `signature` is the signature that a checked first fragment
    /// carries. Returns true when the page is complete, false while it waits or when the whole message fails its signature.
    /// A fragment out of the order 3, 0, 1, 2, 0 ... marks the message damaged.
    fn join(
        &mut self,
        h: Header,
        signature_sent: Option<u8>,
        time: f64,
        sink: &mut impl FnMut(&Page),
    ) -> bool {
        let cap = self.page.address;
        let slot = self
            .fragments
            .iter()
            .position(|f| f.used && f.capcode == cap && f.number == h.number);
        let after = |f: u8| if f == 3 { 0 } else { (f + 1) % 3 };
        if h.continued {
            let i = match slot {
                Some(i) => i,
                None => {
                    let free = self.fragments.iter().position(|f| !f.used).or_else(|| {
                        self.fragments
                            .iter()
                            .enumerate()
                            .min_by(|a, b| a.1.time.total_cmp(&b.1.time))
                            .map(|(i, _)| i)
                    });
                    let Some(i) = free else {
                        // No slots: give the fragment as it is.
                        self.finish_text();
                        return true;
                    };
                    if self.fragments[i].used {
                        self.flush_fragment(i, sink);
                    }
                    let f = &mut self.fragments[i];
                    f.used = true;
                    f.capcode = cap;
                    f.number = h.number;
                    f.count = 0;
                    // A first fragment opens the message; any other means that its start was lost.
                    f.damaged = h.fragment != 3;
                    f.next = 3;
                    f.signature = signature_sent;
                    f.bits_repaired = 0;
                    f.lost_words = 0;
                    f.raw.clear();
                    i
                }
            };
            let f = &mut self.fragments[i];
            f.damaged |= f.count > 0 && h.fragment != f.next;
            f.next = after(h.fragment);
            f.time = time;
            f.rate = self.page.rate;
            f.count = f.count.saturating_add(1);
            f.damaged |= self.page.damaged;
            f.bits_repaired += self.page.bits_repaired;
            f.lost_words += self.page.lost_words;
            let room = MAX_TEXT.saturating_sub(f.raw.len());
            let n = self.page.raw.len().min(room);
            f.raw.extend_from_slice(&self.page.raw[..n]);
            return false;
        }
        if let Some(i) = slot {
            // The last fragment: the earlier text comes first.
            let f = &mut self.fragments[i];
            f.used = false;
            let earlier = f.raw.len();
            let room = MAX_TEXT.saturating_sub(earlier);
            let n = self.page.raw.len().min(room);
            f.raw.extend_from_slice(&self.page.raw[..n]);
            std::mem::swap(&mut f.raw, &mut self.page.raw);
            f.raw.clear();
            self.page.damaged |= f.damaged || h.fragment != f.next;
            self.page.bits_repaired += f.bits_repaired;
            self.page.lost_words += f.lost_words;
            if let Some(fi) = self.page.flex.as_mut() {
                fi.fragments = f.count.saturating_add(1);
            }
            let sent = f.signature;
            if !self.page.damaged
                && sent.is_some_and(|s| s != signature(self.page.raw.iter().copied()))
            {
                return self.refuse(false);
            }
        } else if h.fragment != 3 {
            // A later fragment with no earlier one waiting: the start of the message was lost or aged out.
            self.page.damaged = true;
        }
        self.finish_text();
        true
    }

    fn finish_text(&mut self) {
        page::printable_into(&self.page.raw, &mut self.page.alpha);
        self.page.text.push_str(&self.page.alpha);
    }

    /// Returns a waiting fragment as a damaged page (built in the spare page, so the current page stays).
    fn flush_fragment(&mut self, i: usize, sink: &mut impl FnMut(&Page)) {
        let f = &mut self.fragments[i];
        f.used = false;
        let p = &mut self.spare;
        p.clear();
        p.protocol = Protocol::Flex;
        p.rate = f.rate;
        p.time = f.time;
        p.address = f.capcode;
        p.kind = Kind::Alpha;
        p.damaged = true;
        p.bits_repaired = f.bits_repaired;
        p.lost_words = f.lost_words;
        p.flex = Some(FlexInfo {
            vector: 5,
            continued: true,
            message_number: f.number,
            fragments: f.count,
            ..FlexInfo::default()
        });
        std::mem::swap(&mut f.raw, &mut p.raw);
        page::printable_into(&p.raw, &mut p.alpha);
        p.text.push_str(&p.alpha);
        sink(p);
        std::mem::swap(&mut f.raw, &mut p.raw);
        f.raw.clear();
    }

    fn expire_fragments(&mut self, time: f64, sink: &mut impl FnMut(&Page)) {
        for i in 0..self.fragments.len() {
            let f = &self.fragments[i];
            if f.used && (time - f.time > self.cfg.fragment_age_s || time < f.time) {
                self.flush_fragment(i, sink);
            }
        }
    }

    /// Returns every waiting fragment as a damaged page (for example when the input ends).
    pub fn flush(&mut self, mut sink: impl FnMut(&Page)) {
        for i in 0..self.fragments.len() {
            if self.fragments[i].used {
                self.flush_fragment(i, &mut sink);
            }
        }
    }
}

/// A page to send in a FLEX phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlexTx<'a> {
    /// The capcode: 1 to [`MAX_SHORT_CAPCODE`] (short) or [`MIN_LONG_CAPCODE`] and above (long).
    pub capcode: u32,
    /// [`Kind::Tone`], [`Kind::Numeric`] or [`Kind::Alpha`].
    pub kind: Kind,
    /// The message.
    pub text: &'a str,
}

/// Writes the 88 code words of a phase that carries `pages`. The words are the block information word, the address field, the
/// vector field, the message field and idle words. It returns an error for a capcode that is neither short nor long. It also
/// returns an error for a character outside the set, a kind it does not send, or pages that do not fit.
pub fn encode_phase(pages: &[FlexTx], out: &mut [u32; PHASE_WORDS]) -> Result<()> {
    let mut vals = [0u32; PHASE_WORDS];
    for (i, v) in vals.iter_mut().enumerate() {
        *v = IDLE_VALUES[i % 2];
    }
    let mut addrs = 0usize;
    for p in pages {
        addrs += if address_words(p.capcode)?.1.is_some() {
            2
        } else {
            1
        };
    }
    let a0 = 1usize;
    let v0 = a0 + addrs;
    if v0 + addrs > PHASE_WORDS || v0 > 63 {
        return Err(Error::PhaseFull);
    }
    vals[0] = Biw {
        count: 1,
        vector_start: v0 as u8,
        ..Biw::default()
    }
    .value();
    let mut m = v0 + addrs;
    let mut k = 0;
    let put = |vals: &mut [u32; PHASE_WORDS], at: usize, v: u32| -> Result<()> {
        *vals.get_mut(at).ok_or(Error::PhaseFull)? = v;
        Ok(())
    };
    for p in pages {
        let (w1, w2) = address_words(p.capcode)?;
        vals[a0 + k] = w1;
        if let Some(w2) = w2 {
            vals[a0 + k + 1] = w2;
        }
        let long = w2.is_some();
        let vi = v0 + k;
        match p.kind {
            Kind::Alpha => {
                // The header, the signature and the text, then ETX fill to a whole word.
                let mut msg = [0u32; PHASE_WORDS];
                let n_chars = 1 + p.text.chars().count();
                let len = 1 + n_chars.div_ceil(3);
                if len > 85 || m + len - long as usize > PHASE_WORDS {
                    return Err(Error::PhaseFull);
                }
                let mut chars = [3u8; 3 * PHASE_WORDS];
                for (i, c) in p.text.chars().enumerate() {
                    if !c.is_ascii() {
                        return Err(Error::BadCharacter(c));
                    }
                    chars[1 + i] = c as u8;
                }
                chars[0] = signature(chars[1..n_chars].iter().copied());
                for (w, c) in msg[1..len].iter_mut().zip(chars.chunks(3)) {
                    *w = c[0] as u32 | (c[1] as u32) << 7 | (c[2] as u32) << 14;
                }
                let mut header = Header {
                    fragment: 3,
                    ..Header::default()
                };
                msg[0] = header.value();
                header.check = fragment_check(&msg[..len], 10);
                msg[0] = header.value();
                vals[vi] = Vector::Alpha {
                    start: m as u8,
                    len: len as u8,
                }
                .value();
                let mut words = msg[..len].iter();
                if long {
                    vals[vi + 1] = *words.next().unwrap_or(&0);
                }
                for &w in words {
                    put(&mut vals, m, w)?;
                    m += 1;
                }
            }
            Kind::Numeric => {
                let mut bits: [u8; 8 * 21] = [0; 8 * 21];
                let mut nb = 2usize;
                for c in p.text.chars() {
                    let v = flex_value(c)?;
                    if nb + 4 > bits.len() {
                        return Err(Error::PhaseFull);
                    }
                    for i in 0..4 {
                        bits[nb + i] = (v >> i) & 1;
                    }
                    nb += 4;
                }
                let n_words = nb.div_ceil(21).max(1);
                let mut fill = nb;
                while fill + 4 <= n_words * 21 {
                    for i in 0..4 {
                        bits[fill + i] = (FLEX_FILL >> i) & 1;
                    }
                    fill += 4;
                }
                let mut msg = [0u32; 8];
                for (w, b) in msg.iter_mut().zip(bits.chunks(21)).take(n_words) {
                    *w = b
                        .iter()
                        .enumerate()
                        .fold(0u32, |a, (i, &b)| a | (b as u32) << i);
                }
                let k = numeric_check(&msg[..n_words]);
                msg[0] |= u32::from(k >> 4) & 3;
                // A one-word message of a long address is the second vector word, and the start word points at it.
                let start = if long && n_words == 1 { vi + 1 } else { m };
                vals[vi] = Vector::Numeric {
                    kind: 3,
                    start: start as u8,
                    count: n_words as u8,
                    check: k & 0xF,
                }
                .value();
                let mut words = msg[..n_words].iter();
                if long {
                    vals[vi + 1] = *words.next().unwrap_or(&0);
                }
                for &w in words {
                    put(&mut vals, m, w)?;
                    m += 1;
                }
            }
            Kind::Tone => {
                vals[vi] = Vector::Short { sub: 1, data: 0 }.value();
                if long {
                    vals[vi + 1] = 0;
                }
            }
            other => return Err(Error::BadKind(other)),
        }
        k += if long { 2 } else { 1 };
    }
    for (o, &v) in out.iter_mut().zip(vals.iter()) {
        *o = word(v);
    }
    Ok(())
}

/// Interleaves the 88 words of a phase into [`PHASE_BITS`] bits in the order sent.
pub fn interleave(words: &[u32; PHASE_WORDS], out: &mut [u8]) -> Result<()> {
    if out.len() != PHASE_BITS {
        return Err(Error::BadLength(PHASE_BITS));
    }
    let mut flat = [0u8; PHASE_BITS];
    for (k, &w) in words.iter().enumerate() {
        for j in 0..32 {
            flat[k * 32 + j] = ((w >> (31 - j)) & 1) as u8;
        }
    }
    phase_permutation()
        .apply(&flat, out)
        .map_err(|_| Error::BadLength(PHASE_BITS))
}

/// Appends the 144 bits of sync 1 and the FIW to `out`. These bits go at 1600 bit/s with 2 levels. They are 32 bits of
/// reversals, then A, B, A inverted, and the FIW code word.
pub fn header_bits(speed: Speed, fiw: Fiw, out: &mut Vec<u8>) {
    out.extend((0..BIT_SYNC_BITS).map(|i| (i % 2 == 0) as u8));
    let s = sync1_pattern(speed.code());
    out.extend((0..SYNC1_BITS).rev().map(|i| ((s >> i) & 1) as u8));
    let w = word(fiw.value());
    out.extend((0..32).rev().map(|i| ((w >> i) & 1) as u8));
}

/// Appends the symbols of sync 2 and the blocks of a frame to `out` as levels (-3, -1, +1, +3). Sync 2 is reversals at the
/// block speed. `phases` holds the 88 words of each phase of the speed.
#[allow(clippy::needless_range_loop)]
pub fn data_symbols(speed: Speed, phases: &[[u32; PHASE_WORDS]], out: &mut Vec<i8>) -> Result<()> {
    let np = speed.phases();
    if phases.len() != np {
        return Err(Error::BadLength(np));
    }
    out.extend((0..speed.sync2_symbols()).map(|i| if i % 2 == 0 { 3 } else { -3 }));
    let mut bits = [[0u8; PHASE_BITS]; 4];
    for (p, b) in phases.iter().zip(bits.iter_mut()) {
        interleave(p, b)?;
    }
    let two = |b: u8| if b == 1 { 3 } else { -3 };
    for i in 0..PHASE_BITS {
        match speed {
            Speed::F1600_2 => out.push(two(bits[0][i])),
            Speed::F3200_2 => {
                out.push(two(bits[0][i]));
                out.push(two(bits[1][i]));
            }
            Speed::F3200_4 => out.push(level4(bits[0][i], bits[1][i])),
            Speed::F6400_4 => {
                out.push(level4(bits[0][i], bits[1][i]));
                out.push(level4(bits[2][i], bits[3][i]));
            }
        }
    }
    Ok(())
}
