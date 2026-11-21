//! The DominoEX varicode (M. Greenman ZL1BPU, published with the DominoEX description): a character is one to three 4-bit symbols;
//! the first has bit 3 clear and every continuation has bit 3 set, so the next character starts at the next symbol below 8. A
//! secondary alphabet (for a beacon text sent in the idle time) uses the same structure.
//!
//! ```
//! use sdrkit_util_text::varicode::dominoex::{self, Decoder, Symbol};
//!
//! let mut d = Decoder::new();
//! let mut got = Vec::new();
//! for &b in b"de" {
//!     for &n in dominoex::code(b).symbols() {
//!         got.extend(d.push(n));
//!     }
//! }
//! got.extend(d.flush());
//! assert_eq!(got, [Ok(Symbol::Primary(b'd')), Ok(Symbol::Primary(b'e'))]);
//! ```

use crate::error::{Error, Result};

/// One code: up to three symbols (a 0 after the first means "absent", as continuations are 8 or more).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Code(pub [u8; 3]);

impl Code {
    /// The symbols sent.
    pub fn symbols(&self) -> &[u8] {
        let n = if self.0[1] == 0 {
            1
        } else if self.0[2] == 0 {
            2
        } else {
            3
        };
        &self.0[..n]
    }

    const fn key(&self) -> usize {
        self.0[0] as usize | (self.0[1] as usize) << 4 | (self.0[2] as usize) << 8
    }
}

/// The primary codes of bytes 0 to 255.
pub const PRIMARY: [Code; 256] = {
    const T: [[u8; 3]; 256] = [
        [1, 15, 9],
        [1, 15, 10],
        [1, 15, 11],
        [1, 15, 12],
        [1, 15, 13],
        [1, 15, 14],
        [1, 15, 15],
        [2, 8, 8],
        [2, 12, 0],
        [2, 8, 9],
        [2, 8, 10],
        [2, 8, 11],
        [2, 8, 12],
        [2, 13, 0],
        [2, 8, 13],
        [2, 8, 14],
        [2, 8, 15],
        [2, 9, 8],
        [2, 9, 9],
        [2, 9, 10],
        [2, 9, 11],
        [2, 9, 12],
        [2, 9, 13],
        [2, 9, 14],
        [2, 9, 15],
        [2, 10, 8],
        [2, 10, 9],
        [2, 10, 10],
        [2, 10, 11],
        [2, 10, 12],
        [2, 10, 13],
        [2, 10, 14],
        [0, 0, 0],
        [7, 11, 0],
        [0, 8, 14],
        [0, 10, 11],
        [0, 9, 10],
        [0, 9, 9],
        [0, 8, 15],
        [7, 10, 0],
        [0, 8, 12],
        [0, 8, 11],
        [0, 9, 13],
        [0, 8, 8],
        [2, 11, 0],
        [7, 14, 0],
        [7, 13, 0],
        [0, 8, 9],
        [3, 15, 0],
        [4, 10, 0],
        [4, 15, 0],
        [5, 9, 0],
        [6, 8, 0],
        [5, 12, 0],
        [5, 14, 0],
        [6, 12, 0],
        [6, 11, 0],
        [6, 14, 0],
        [0, 8, 10],
        [0, 8, 13],
        [0, 10, 8],
        [7, 15, 0],
        [0, 9, 15],
        [7, 12, 0],
        [0, 9, 8],
        [3, 9, 0],
        [4, 14, 0],
        [3, 12, 0],
        [3, 14, 0],
        [3, 8, 0],
        [4, 12, 0],
        [5, 8, 0],
        [5, 10, 0],
        [3, 10, 0],
        [7, 8, 0],
        [6, 10, 0],
        [4, 11, 0],
        [4, 8, 0],
        [4, 13, 0],
        [3, 11, 0],
        [4, 9, 0],
        [6, 15, 0],
        [3, 13, 0],
        [2, 15, 0],
        [2, 14, 0],
        [5, 11, 0],
        [6, 13, 0],
        [5, 13, 0],
        [5, 15, 0],
        [6, 9, 0],
        [7, 9, 0],
        [0, 10, 14],
        [0, 10, 9],
        [0, 10, 15],
        [0, 10, 10],
        [0, 9, 12],
        [0, 9, 11],
        [4, 0, 0],
        [1, 11, 0],
        [0, 12, 0],
        [0, 11, 0],
        [1, 0, 0],
        [0, 15, 0],
        [1, 9, 0],
        [0, 10, 0],
        [5, 0, 0],
        [2, 10, 0],
        [1, 14, 0],
        [0, 9, 0],
        [0, 14, 0],
        [6, 0, 0],
        [3, 0, 0],
        [1, 8, 0],
        [2, 8, 0],
        [7, 0, 0],
        [0, 8, 0],
        [2, 0, 0],
        [0, 13, 0],
        [1, 13, 0],
        [1, 12, 0],
        [1, 15, 0],
        [1, 10, 0],
        [2, 9, 0],
        [0, 10, 12],
        [0, 9, 14],
        [0, 10, 13],
        [0, 11, 8],
        [2, 10, 15],
        [2, 11, 8],
        [2, 11, 9],
        [2, 11, 10],
        [2, 11, 11],
        [2, 11, 12],
        [2, 11, 13],
        [2, 11, 14],
        [2, 11, 15],
        [2, 12, 8],
        [2, 12, 9],
        [2, 12, 10],
        [2, 12, 11],
        [2, 12, 12],
        [2, 12, 13],
        [2, 12, 14],
        [2, 12, 15],
        [2, 13, 8],
        [2, 13, 9],
        [2, 13, 10],
        [2, 13, 11],
        [2, 13, 12],
        [2, 13, 13],
        [2, 13, 14],
        [2, 13, 15],
        [2, 14, 8],
        [2, 14, 9],
        [2, 14, 10],
        [2, 14, 11],
        [2, 14, 12],
        [2, 14, 13],
        [2, 14, 14],
        [2, 14, 15],
        [0, 11, 9],
        [0, 11, 10],
        [0, 11, 11],
        [0, 11, 12],
        [0, 11, 13],
        [0, 11, 14],
        [0, 11, 15],
        [0, 12, 8],
        [0, 12, 9],
        [0, 12, 10],
        [0, 12, 11],
        [0, 12, 12],
        [0, 12, 13],
        [0, 12, 14],
        [0, 12, 15],
        [0, 13, 8],
        [0, 13, 9],
        [0, 13, 10],
        [0, 13, 11],
        [0, 13, 12],
        [0, 13, 13],
        [0, 13, 14],
        [0, 13, 15],
        [0, 14, 8],
        [0, 14, 9],
        [0, 14, 10],
        [0, 14, 11],
        [0, 14, 12],
        [0, 14, 13],
        [0, 14, 14],
        [0, 14, 15],
        [0, 15, 8],
        [0, 15, 9],
        [0, 15, 10],
        [0, 15, 11],
        [0, 15, 12],
        [0, 15, 13],
        [0, 15, 14],
        [0, 15, 15],
        [1, 8, 8],
        [1, 8, 9],
        [1, 8, 10],
        [1, 8, 11],
        [1, 8, 12],
        [1, 8, 13],
        [1, 8, 14],
        [1, 8, 15],
        [1, 9, 8],
        [1, 9, 9],
        [1, 9, 10],
        [1, 9, 11],
        [1, 9, 12],
        [1, 9, 13],
        [1, 9, 14],
        [1, 9, 15],
        [1, 10, 8],
        [1, 10, 9],
        [1, 10, 10],
        [1, 10, 11],
        [1, 10, 12],
        [1, 10, 13],
        [1, 10, 14],
        [1, 10, 15],
        [1, 11, 8],
        [1, 11, 9],
        [1, 11, 10],
        [1, 11, 11],
        [1, 11, 12],
        [1, 11, 13],
        [1, 11, 14],
        [1, 11, 15],
        [1, 12, 8],
        [1, 12, 9],
        [1, 12, 10],
        [1, 12, 11],
        [1, 12, 12],
        [1, 12, 13],
        [1, 12, 14],
        [1, 12, 15],
        [1, 13, 8],
        [1, 13, 9],
        [1, 13, 10],
        [1, 13, 11],
        [1, 13, 12],
        [1, 13, 13],
        [1, 13, 14],
        [1, 13, 15],
        [1, 14, 8],
        [1, 14, 9],
        [1, 14, 10],
        [1, 14, 11],
        [1, 14, 12],
        [1, 14, 13],
        [1, 14, 14],
        [1, 14, 15],
        [1, 15, 8],
    ];
    let mut out = [Code([0; 3]); 256];
    let mut i = 0;
    while i < 256 {
        out[i] = Code(T[i]);
        i += 1;
    }
    out
};

/// The secondary codes of bytes 0x20 to 0x7A.
pub const SECONDARY: [Code; 91] = {
    const T: [[u8; 3]; 91] = [
        [3, 8, 8],
        [4, 15, 11],
        [5, 8, 14],
        [5, 10, 11],
        [5, 9, 10],
        [5, 9, 9],
        [5, 8, 15],
        [4, 15, 10],
        [5, 8, 12],
        [5, 8, 11],
        [5, 9, 13],
        [5, 8, 8],
        [4, 10, 11],
        [4, 15, 14],
        [4, 15, 13],
        [5, 8, 9],
        [4, 11, 15],
        [4, 12, 10],
        [4, 12, 15],
        [4, 13, 9],
        [4, 14, 8],
        [4, 13, 12],
        [4, 13, 14],
        [4, 14, 12],
        [4, 14, 11],
        [4, 14, 14],
        [5, 8, 10],
        [5, 8, 13],
        [5, 10, 8],
        [4, 15, 15],
        [5, 9, 15],
        [4, 15, 12],
        [5, 9, 8],
        [4, 11, 9],
        [4, 12, 14],
        [4, 11, 12],
        [4, 11, 14],
        [4, 11, 8],
        [4, 12, 12],
        [4, 13, 8],
        [4, 13, 10],
        [4, 11, 10],
        [4, 15, 8],
        [4, 14, 10],
        [4, 12, 11],
        [4, 12, 8],
        [4, 12, 13],
        [4, 11, 11],
        [4, 12, 9],
        [4, 14, 15],
        [4, 11, 13],
        [4, 10, 15],
        [4, 10, 14],
        [4, 13, 11],
        [4, 14, 13],
        [4, 13, 13],
        [4, 13, 15],
        [4, 14, 9],
        [4, 15, 9],
        [5, 10, 14],
        [5, 10, 9],
        [5, 10, 15],
        [5, 10, 10],
        [5, 9, 12],
        [5, 9, 11],
        [3, 8, 12],
        [4, 9, 11],
        [4, 8, 12],
        [4, 8, 11],
        [3, 8, 9],
        [4, 8, 15],
        [4, 9, 9],
        [4, 8, 10],
        [3, 8, 13],
        [4, 10, 10],
        [4, 9, 14],
        [4, 8, 9],
        [4, 8, 14],
        [3, 8, 14],
        [3, 8, 11],
        [4, 9, 8],
        [4, 10, 8],
        [3, 8, 15],
        [4, 8, 8],
        [3, 8, 10],
        [4, 8, 13],
        [4, 9, 13],
        [4, 9, 12],
        [4, 9, 15],
        [4, 9, 10],
        [4, 10, 9],
    ];
    let mut out = [Code([0; 3]); 91];
    let mut i = 0;
    while i < 91 {
        out[i] = Code(T[i]);
        i += 1;
    }
    out
};

/// A decoded character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// A byte of the main text.
    Primary(u8),
    /// A secondary character (0x20 to 0x7A).
    Secondary(u8),
}

/// Code key to symbol: 0 none, byte + 1 for primary, 0x8000 | byte for secondary.
const INDEX: [u16; 4096] = {
    let mut t = [0u16; 4096];
    let mut i = 0;
    while i < 256 {
        t[PRIMARY[i].key()] = i as u16 + 1;
        i += 1;
    }
    let mut i = 0;
    while i < 91 {
        t[SECONDARY[i].key()] = 0x8000 | (0x20 + i as u16);
        i += 1;
    }
    t
};

/// The primary code of a byte.
pub fn code(b: u8) -> Code {
    PRIMARY[b as usize]
}

/// The secondary code of a character (0x20 to 0x7A).
pub fn secondary_code(b: u8) -> Result<Code> {
    match b {
        0x20..=0x7A => Ok(SECONDARY[(b - 0x20) as usize]),
        _ => Err(Error::Unencodable(b as char)),
    }
}

/// The symbol of a code.
pub fn lookup(code: Code) -> Option<Symbol> {
    if code.0.iter().any(|&n| n > 15) || code.0[0] > 7 {
        return None;
    }
    match INDEX[code.key()] {
        0 => None,
        v if v & 0x8000 != 0 => Some(Symbol::Secondary(v as u8)),
        v => Some(Symbol::Primary((v - 1) as u8)),
    }
}

/// The symbol decoder: a character ends when the next one starts (or at [`Decoder::flush`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct Decoder {
    code: [u8; 3],
    len: u8,
}

impl Decoder {
    /// A decoder between characters.
    pub fn new() -> Self {
        Self::default()
    }

    /// One symbol (0 to 15). Returns the previous character when this symbol starts a new one.
    pub fn push(&mut self, nibble: u8) -> Option<Result<Symbol>> {
        let n = nibble & 15;
        if n < 8 {
            let done = self.flush();
            self.code = [n, 0, 0];
            self.len = 1;
            return done;
        }
        if self.len == 0 {
            return None; // a continuation without a start
        }
        if (self.len as usize) < 3 {
            self.code[self.len as usize] = n;
        }
        self.len = self.len.saturating_add(1);
        None
    }

    /// Ends the character being received.
    pub fn flush(&mut self) -> Option<Result<Symbol>> {
        if self.len == 0 {
            return None;
        }
        let c = Code(self.code);
        let long = self.len > 3;
        self.len = 0;
        self.code = [0; 3];
        if long {
            return Some(Err(Error::Invalid));
        }
        Some(lookup(c).ok_or(Error::UnknownCode(c.key() as u32)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_codes() {
        // The commonest characters take one symbol, in the published frequency order.
        for (b, s) in [
            (b' ', &[0u8][..]),
            (b'e', &[1]),
            (b't', &[2]),
            (b'o', &[3]),
            (b'a', &[4]),
            (b'i', &[5]),
            (b'n', &[6]),
            (b'r', &[7]),
            (b's', &[0, 8]),
            (b'A', &[3, 9]),
            (b'0', &[3, 15]),
            (b'\r', &[2, 13]),
            (0, &[1, 15, 9]),
        ] {
            assert_eq!(code(b).symbols(), s, "{}", b as char);
        }
    }

    #[test]
    fn table_rules_and_round_trip() {
        let mut seen = std::collections::HashSet::new();
        let mut d = Decoder::new();
        let mut got = vec![];
        for b in 0..=255u8 {
            let c = code(b);
            assert!(
                c.0[0] < 8 && c.symbols()[1..].iter().all(|&n| n >= 8),
                "{b}"
            );
            assert!(seen.insert(c), "{b}");
            for &n in c.symbols() {
                got.extend(d.push(n));
            }
        }
        for b in 0x20..=0x7Au8 {
            let c = secondary_code(b).unwrap();
            assert!(seen.insert(c), "secondary {b}");
            for &n in c.symbols() {
                got.extend(d.push(n));
            }
        }
        got.extend(d.flush());
        let mut want: Vec<_> = (0..=255u8).map(|b| Ok(Symbol::Primary(b))).collect();
        want.extend((0x20..=0x7Au8).map(|b| Ok(Symbol::Secondary(b))));
        assert_eq!(got, want);
        assert_eq!(d.push(9), None);
    }
}
