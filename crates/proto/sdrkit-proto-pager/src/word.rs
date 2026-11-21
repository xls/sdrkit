//! The 32-bit code word of POCSAG and FLEX.
//!
//! ITU-R M.584-2 annex 1 clause 1.4 defines it. A code word has 21 information bits and 10 check bits of the BCH(31,21) code
//! with generator `x^10 + x^9 + x^8 + x^6 + x^5 + x^3 + 1`. The 32nd bit is an even parity bit over the whole word. FLEX uses the same
//! word (annex 2 clause 1.4.1). The first bit sent is the most significant bit of the `u32`.
//!
//! The code itself is [`sdrkit_algo_fec::small::SmallCode::pocsag_32_21`]. This module adds two paging rules on top of it. The soft
//! repair never changes a confident bit. Each repair records what it did.
//!
//! ```
//! use sdrkit_proto_pager::word;
//! let cw = word::encode(0x12345);
//! assert_eq!(word::data(cw), 0x12345);
//! assert_eq!(word::repair(cw ^ 0x8000_0001), Some((cw, 2)));
//! assert_eq!(word::repair(cw ^ 0x8000_0101), None); // three errors: detected, not repaired
//! ```

use sdrkit_algo_fec::small::{Chase, SmallCode};
use std::sync::OnceLock;

/// The 21 information bits.
pub const DATA_MASK: u32 = 0x1F_FFFF;

/// The code, built once for the whole process.
pub fn code() -> &'static SmallCode {
    static CODE: OnceLock<SmallCode> = OnceLock::new();
    CODE.get_or_init(SmallCode::pocsag_32_21)
}

/// The code word of 21 information bits (the low 21 bits of `data21`).
pub fn encode(data21: u32) -> u32 {
    code().encode((data21 & DATA_MASK) as u128) as u32
}

/// The 21 information bits of a code word (bit 20 is the first bit sent).
pub fn data(cw: u32) -> u32 {
    cw >> 11
}

/// True when `cw` is a code word as received (check bits and parity correct).
pub fn is_valid(cw: u32) -> bool {
    code().syndrome(cw as u128) == 0
}

/// Repairs up to two wrong bits. The parity bit counts as one bit. Returns the code word and the number of bits changed, or
/// `None` when the word has more errors than the code can repair.
pub fn repair(cw: u32) -> Option<(u32, u8)> {
    code().decode(cw as u128).map(|(c, n)| (c as u32, n))
}

/// The rules of a soft repair.
///
/// The `doubt` limit is in the units of the reliabilities that the caller provides. A demodulator that returns true LLRs must scale
/// the limit with its LLRs. The defaults suit reliabilities where a clean symbol at the decision level has the value 1.0, as a
/// paging receiver measures them (`|v - centre| / amplitude`). For true LLRs `L = 2 y / σ²` with the clean
/// level `y = 1`, multiply the limits by `2 / σ²`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoftRepair {
    /// The bits to try. The default tries up to 3 of the 6 least reliable bits.
    pub chase: Chase,
    /// A repair can change a bit only when its reliability is below this limit.
    pub doubt: f32,
    /// A repair can change at most this many bits.
    pub max_changed: u8,
}

impl SoftRepair {
    /// The limits for message words: changed bits below 1.5, at most 4 bits.
    pub const MESSAGE: SoftRepair = SoftRepair {
        chase: Chase {
            weakest: 6,
            max_flips: 3,
        },
        doubt: 1.5,
        max_changed: 4,
    };

    /// The limits for header words (FLEX block information, addresses and vectors): changed bits below 0.6, at most 4 bits.
    pub const HEADER: SoftRepair = SoftRepair {
        doubt: 0.6,
        ..SoftRepair::MESSAGE
    };
}

impl Default for SoftRepair {
    fn default() -> Self {
        SoftRepair::MESSAGE
    }
}

/// Repairs a word with the hard repair first. If the hard repair fails, it runs a Chase search over the least reliable bits.
/// `rel[i]` is the reliability of bit `i`, and bit 0 is the first bit sent.
///
/// The Chase search finds the code word nearest to the received bits. If that word changes a bit at or above `doubt`, or
/// more than `max_changed` bits, the word is refused. No other candidate is then tried. This is stricter than a search that
/// filters the candidates first.
pub fn repair_soft(cw: u32, rel: &[f32; 32], rules: SoftRepair) -> Option<(u32, u8)> {
    if let Some(r) = repair(cw) {
        return Some(r);
    }
    let (c, n, _) = code().decode_soft(cw as u128, rel, rules.chase)?;
    let c = c as u32;
    let diff = c ^ cw;
    if n > rules.max_changed {
        return None;
    }
    let mut d = diff;
    while d != 0 {
        let bit = 31 - d.leading_zeros() as usize;
        // NaN is never below the limit.
        if rel[31 - bit].partial_cmp(&rules.doubt) != Some(std::cmp::Ordering::Less) {
            return None;
        }
        d &= !(1 << bit);
    }
    Some((c, n))
}

/// A received word after the repair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Checked {
    /// The word: repaired when the repair worked, else as received.
    pub cw: u32,
    /// The word as received.
    pub raw: u32,
    /// True when the word is a code word after the repair.
    pub valid: bool,
    /// The number of bits that the repair changed.
    pub fixes: u8,
}

impl Checked {
    /// Reads a word with the hard repair only.
    pub fn hard(raw: u32) -> Checked {
        match repair(raw) {
            Some((cw, fixes)) => Checked {
                cw,
                raw,
                valid: true,
                fixes,
            },
            None => Checked::lost(raw),
        }
    }

    /// Reads a word with the hard repair. If `rel` is given, it then tries the soft repair.
    pub fn soft(raw: u32, rel: Option<&[f32; 32]>, rules: SoftRepair) -> Checked {
        let r = match rel {
            Some(rel) => repair_soft(raw, rel, rules),
            None => repair(raw),
        };
        match r {
            Some((cw, fixes)) => Checked {
                cw,
                raw,
                valid: true,
                fixes,
            },
            None => Checked::lost(raw),
        }
    }

    /// A word that no repair can save.
    pub fn lost(raw: u32) -> Checked {
        Checked {
            cw: raw,
            raw,
            valid: false,
            fixes: 0,
        }
    }

    /// True when the word passed as received.
    pub fn clean(&self) -> bool {
        self.valid && self.fixes == 0
    }
}

/// Hard bits and reliabilities of a word from 32 LLRs (positive means 0). Bit 0 of the slice is the first bit sent. NaN
/// counts as an erasure (reliability 0).
pub fn from_llrs(llr: &[f32; 32]) -> (u32, [f32; 32]) {
    let mut w = 0u32;
    let mut rel = [0f32; 32];
    for (i, &l) in llr.iter().enumerate() {
        w = (w << 1) | (l < 0.0) as u32;
        rel[i] = if l.is_nan() { 0.0 } else { l.abs() };
    }
    (w, rel)
}

/// Reverses the low 21 bits. FLEX sends its information bits least significant first (M.584-2 annex 2 clause 1.4.1).
pub fn rev21(x: u32) -> u32 {
    (x & DATA_MASK).reverse_bits() >> 11
}
