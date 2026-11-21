//! The character sets of the paging codes.
//!
//! - POCSAG numeric messages use 4 bits per character (ITU-R M.584-2 annex 1 clause 2.1, Table 3). The bits of a character go
//!   out first bit least significant. The values 0 to 9 are the digits. Then come spare (10), `U` for urgency (11), space (12),
//!   hyphen (13) and two brackets (14, 15). Clause 2.1 names an opening and a closing bracket. Table 3 draws them as glyphs.
//!   [`NumericSet::Itu`] shows them as `]` and `[`. The glyphs in Table 3 are images, so
//!   this order follows the drawing. [`NumericSet::Parentheses`] shows them as `)` and `(`, as the LBJ train
//!   messages of the cell family expect. Both sets show spare as `*`.
//! - POCSAG text messages use 7-bit characters of International Alphabet No. 5 (ITU-T T.50), first bit least significant
//!   (clause 2.2). A character can cross from one message word into the next.
//! - FLEX numeric messages use the same 4-bit order with the set [`FLEX_NUMERIC`]. Value 12 is fill there and is not shown. The FLEX set follows public descriptions of the protocol (RCR STD-43 is not freely available).
//!
//! National variants of IA5 (for example the German DIN 66003 letters `Ä Ö Ü ä ö ü ß` at `[ \ ] { | } ~`) are not in this crate.
//! A page holds the raw 7-bit characters in [`crate::Page::raw`], so a caller can apply a variant.
//!
//! ```
//! use sdrkit_proto_pager::charset::NumericSet;
//! assert_eq!(NumericSet::Itu.char(11), 'U');
//! assert_eq!(NumericSet::Itu.value('-'), Some(13));
//! assert_eq!(NumericSet::Parentheses.char(15), '(');
//! ```

use crate::{Error, Result};

/// A POCSAG numeric character set (values 0 to 15).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumericSet {
    /// ITU-R M.584-2 Table 3: `0123456789*U -][`.
    #[default]
    Itu,
    /// The same with parentheses for the brackets: `0123456789*U -)(`.
    Parentheses,
}

impl NumericSet {
    /// The 16 characters of the set.
    pub const fn chars(self) -> &'static [u8; 16] {
        match self {
            NumericSet::Itu => b"0123456789*U -][",
            NumericSet::Parentheses => b"0123456789*U -)(",
        }
    }

    /// The character of a value (only the low 4 bits are read).
    pub const fn char(self, v: u8) -> char {
        self.chars()[(v & 15) as usize] as char
    }

    /// The value of a character, or `None` when the set does not hold it.
    pub fn value(self, c: char) -> Option<u8> {
        self.chars()
            .iter()
            .position(|&x| x as char == c)
            .map(|v| v as u8)
    }

    /// The name used in settings: "itu" or "parentheses".
    pub const fn name(self) -> &'static str {
        match self {
            NumericSet::Itu => "itu",
            NumericSet::Parentheses => "parentheses",
        }
    }

    /// The set with this name.
    pub fn by_name(name: &str) -> Option<NumericSet> {
        match name {
            "itu" => Some(NumericSet::Itu),
            "parentheses" => Some(NumericSet::Parentheses),
            _ => None,
        }
    }
}

/// The FLEX numeric set. Value 12 is fill (not shown)..
pub const FLEX_NUMERIC: &[u8; 16] = b"0123456789 U -][";

/// The FLEX fill value of numeric messages.
pub const FLEX_FILL: u8 = 12;

/// The value of a character in the FLEX numeric set. A space is sent as 10, so that it is not read as fill.
pub fn flex_value(c: char) -> Result<u8> {
    match c {
        ' ' => Ok(10),
        _ => FLEX_NUMERIC
            .iter()
            .position(|&x| x as char == c)
            .filter(|&v| v != FLEX_FILL as usize)
            .map(|v| v as u8)
            .ok_or(Error::BadCharacter(c)),
    }
}

/// Reads characters of `width` bits (4 or 7) from bits (`u8`, 0 or 1, first sent first). The first bit of each character is its
/// least significant bit. A partial character at the end is not read.
pub fn chars_lsb_first(bits: &[u8], width: usize) -> impl Iterator<Item = u8> + '_ {
    bits.chunks_exact(width.max(1)).map(|c| {
        c.iter()
            .enumerate()
            .fold(0u8, |a, (i, &b)| a | ((b & 1) << i))
    })
}

/// Appends the `n` low bits of `v` to `out`, least significant first.
pub fn push_lsb_first(out: &mut Vec<u8>, v: u32, n: usize) {
    out.extend((0..n).map(|i| ((v >> i) & 1) as u8));
}
