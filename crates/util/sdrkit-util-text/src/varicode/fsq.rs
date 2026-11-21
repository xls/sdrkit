//! The base-32 symbol varicodes of FSQ and IFKP (M. Greenman ZL1BPU and C. Pidgeon ZL2AFP, published with the FSQ and IFKP
//! descriptions). A character is one symbol (0 to 28) or a symbol followed by a marker (29, 30 or 31). The two alphabets have the
//! same shape but differ in a few places: FSQ sends space as symbol 0 and line feed as 28; IFKP sends space as 28.
//!
//! ```
//! use sdrkit_util_text::varicode::fsq::{Alphabet, Decoder};
//!
//! let a = Alphabet::Fsq;
//! let mut d = Decoder::new(a);
//! let mut out = Vec::new();
//! for &b in b"CQ cq" {
//!     for s in a.code(b).unwrap().symbols() {
//!         out.extend(d.push(*s));
//!     }
//! }
//! out.extend(d.flush());
//! let text: Vec<u8> = out.into_iter().map(|r| r.unwrap()).collect();
//! assert_eq!(text, b"CQ cq");
//! ```

use crate::error::{Error, Result};

/// One code: a symbol, and a marker when the character needs two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    /// The first symbol (0 to 28).
    pub first: u8,
    /// The marker (29 to 31), when there is one.
    pub marker: Option<u8>,
}

impl Code {
    /// The symbols sent.
    pub fn symbols(&self) -> impl Iterator<Item = &u8> {
        core::iter::once(&self.first).chain(self.marker.as_ref())
    }
}

/// (byte, first symbol, marker or 0) of IFKP, sorted by byte.
const IFKP: [(u8, u8, u8); 103] = [
    (8, 27, 31),
    (10, 28, 30),
    (32, 28, 0),
    (33, 11, 30),
    (34, 12, 30),
    (35, 13, 30),
    (36, 14, 30),
    (37, 15, 30),
    (38, 16, 30),
    (39, 17, 30),
    (40, 18, 30),
    (41, 19, 30),
    (42, 20, 30),
    (43, 21, 30),
    (44, 27, 29),
    (45, 22, 30),
    (46, 27, 0),
    (47, 23, 30),
    (48, 10, 30),
    (49, 1, 30),
    (50, 2, 30),
    (51, 3, 30),
    (52, 4, 30),
    (53, 5, 30),
    (54, 6, 30),
    (55, 7, 30),
    (56, 8, 30),
    (57, 9, 30),
    (58, 24, 30),
    (59, 25, 30),
    (60, 26, 30),
    (61, 0, 31),
    (62, 27, 30),
    (63, 28, 29),
    (64, 0, 29),
    (65, 1, 29),
    (66, 2, 29),
    (67, 3, 29),
    (68, 4, 29),
    (69, 5, 29),
    (70, 6, 29),
    (71, 7, 29),
    (72, 8, 29),
    (73, 9, 29),
    (74, 10, 29),
    (75, 11, 29),
    (76, 12, 29),
    (77, 13, 29),
    (78, 14, 29),
    (79, 15, 29),
    (80, 16, 29),
    (81, 17, 29),
    (82, 18, 29),
    (83, 19, 29),
    (84, 20, 29),
    (85, 21, 29),
    (86, 22, 29),
    (87, 23, 29),
    (88, 24, 29),
    (89, 25, 29),
    (90, 26, 29),
    (91, 1, 31),
    (92, 2, 31),
    (93, 3, 31),
    (94, 4, 31),
    (95, 5, 31),
    (96, 9, 31),
    (97, 1, 0),
    (98, 2, 0),
    (99, 3, 0),
    (100, 4, 0),
    (101, 5, 0),
    (102, 6, 0),
    (103, 7, 0),
    (104, 8, 0),
    (105, 9, 0),
    (106, 10, 0),
    (107, 11, 0),
    (108, 12, 0),
    (109, 13, 0),
    (110, 14, 0),
    (111, 15, 0),
    (112, 16, 0),
    (113, 17, 0),
    (114, 18, 0),
    (115, 19, 0),
    (116, 20, 0),
    (117, 21, 0),
    (118, 22, 0),
    (119, 23, 0),
    (120, 24, 0),
    (121, 25, 0),
    (122, 26, 0),
    (123, 6, 31),
    (124, 7, 31),
    (125, 8, 31),
    (126, 0, 30),
    (127, 28, 31),
    (163, 14, 31),
    (176, 12, 31),
    (177, 10, 31),
    (215, 13, 31),
    (247, 11, 31),
];

/// (byte, first symbol, marker or 0) of FSQ, sorted by byte.
const FSQ: [(u8, u8, u8); 103] = [
    (8, 27, 31),
    (10, 28, 0),
    (32, 0, 0),
    (33, 11, 30),
    (34, 12, 30),
    (35, 13, 30),
    (36, 14, 30),
    (37, 15, 30),
    (38, 16, 30),
    (39, 17, 30),
    (40, 18, 30),
    (41, 19, 30),
    (42, 20, 30),
    (43, 21, 30),
    (44, 27, 29),
    (45, 22, 30),
    (46, 27, 0),
    (47, 23, 30),
    (48, 10, 30),
    (49, 1, 30),
    (50, 2, 30),
    (51, 3, 30),
    (52, 4, 30),
    (53, 5, 30),
    (54, 6, 30),
    (55, 7, 30),
    (56, 8, 30),
    (57, 9, 30),
    (58, 24, 30),
    (59, 25, 30),
    (60, 26, 30),
    (61, 0, 31),
    (62, 27, 30),
    (63, 28, 29),
    (64, 0, 29),
    (65, 1, 29),
    (66, 2, 29),
    (67, 3, 29),
    (68, 4, 29),
    (69, 5, 29),
    (70, 6, 29),
    (71, 7, 29),
    (72, 8, 29),
    (73, 9, 29),
    (74, 10, 29),
    (75, 11, 29),
    (76, 12, 29),
    (77, 13, 29),
    (78, 14, 29),
    (79, 15, 29),
    (80, 16, 29),
    (81, 17, 29),
    (82, 18, 29),
    (83, 19, 29),
    (84, 20, 29),
    (85, 21, 29),
    (86, 22, 29),
    (87, 23, 29),
    (88, 24, 29),
    (89, 25, 29),
    (90, 26, 29),
    (91, 1, 31),
    (92, 2, 31),
    (93, 3, 31),
    (94, 4, 31),
    (95, 5, 31),
    (96, 9, 31),
    (97, 1, 0),
    (98, 2, 0),
    (99, 3, 0),
    (100, 4, 0),
    (101, 5, 0),
    (102, 6, 0),
    (103, 7, 0),
    (104, 8, 0),
    (105, 9, 0),
    (106, 10, 0),
    (107, 11, 0),
    (108, 12, 0),
    (109, 13, 0),
    (110, 14, 0),
    (111, 15, 0),
    (112, 16, 0),
    (113, 17, 0),
    (114, 18, 0),
    (115, 19, 0),
    (116, 20, 0),
    (117, 21, 0),
    (118, 22, 0),
    (119, 23, 0),
    (120, 24, 0),
    (121, 25, 0),
    (122, 26, 0),
    (123, 6, 31),
    (124, 7, 31),
    (125, 8, 31),
    (126, 0, 30),
    (127, 28, 31),
    (163, 14, 31),
    (176, 12, 31),
    (177, 10, 31),
    (215, 13, 31),
    (247, 11, 31),
];

const fn index(t: &[(u8, u8, u8); 103]) -> [u8; 1024] {
    let mut out = [0u8; 1024];
    let mut i = 0;
    while i < 103 {
        let (b, s, m) = t[i];
        let k = if m == 0 {
            s as usize
        } else {
            s as usize * 32 + m as usize
        };
        out[k] = b;
        i += 1;
    }
    out
}

const IFKP_INDEX: [u8; 1024] = index(&IFKP);
const FSQ_INDEX: [u8; 1024] = index(&FSQ);

/// Which alphabet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alphabet {
    /// FSQ (Fast Simple QSO).
    Fsq,
    /// IFKP (Incremental Frequency Keying Plus).
    Ifkp,
}

impl Alphabet {
    fn table(self) -> (&'static [(u8, u8, u8); 103], &'static [u8; 1024]) {
        match self {
            Alphabet::Fsq => (&FSQ, &FSQ_INDEX),
            Alphabet::Ifkp => (&IFKP, &IFKP_INDEX),
        }
    }

    /// The code of a byte.
    pub fn code(self, b: u8) -> Result<Code> {
        let (t, _) = self.table();
        let i = t
            .binary_search_by_key(&b, |e| e.0)
            .map_err(|_| Error::Unencodable(b as char))?;
        let (_, first, m) = t[i];
        Ok(Code {
            first,
            marker: (m != 0).then_some(m),
        })
    }

    /// The byte of a code.
    pub fn lookup(self, code: Code) -> Option<u8> {
        let (_, idx) = self.table();
        let k = match code.marker {
            None if code.first < 29 => code.first as usize,
            Some(m) if (29..32).contains(&m) && code.first < 29 => {
                code.first as usize * 32 + m as usize
            }
            _ => return None,
        };
        let b = idx[k];
        // Byte 0 is never a character here; IFKP's symbol 0 alone means nothing.
        (b != 0).then_some(b)
    }
}

/// The symbol decoder: a symbol is held until the next one shows whether a marker follows.
#[derive(Clone, Copy, Debug)]
pub struct Decoder {
    alphabet: Alphabet,
    pending: Option<u8>,
}

impl Decoder {
    /// A decoder for one alphabet.
    pub fn new(alphabet: Alphabet) -> Self {
        Decoder {
            alphabet,
            pending: None,
        }
    }

    /// One symbol (0 to 31). Returns a character when one is complete.
    pub fn push(&mut self, symbol: u8) -> Option<Result<u8>> {
        let s = symbol & 31;
        if s >= 29 {
            let first = self.pending.take()?;
            let code = Code {
                first,
                marker: Some(s),
            };
            return Some(
                self.alphabet
                    .lookup(code)
                    .ok_or(Error::UnknownCode(first as u32 * 32 + s as u32)),
            );
        }
        let done = self.flush();
        self.pending = Some(s);
        done
    }

    /// Ends a held symbol (at the end of a transmission).
    pub fn flush(&mut self) -> Option<Result<u8>> {
        let s = self.pending.take()?;
        Some(
            self.alphabet
                .lookup(Code {
                    first: s,
                    marker: None,
                })
                .ok_or(Error::UnknownCode(s as u32)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_codes() {
        let c = |a: Alphabet, b: u8| {
            let k = a.code(b).unwrap();
            (k.first, k.marker.unwrap_or(0))
        };
        for a in [Alphabet::Fsq, Alphabet::Ifkp] {
            // Lower case letters are the single symbols 1 to 26, upper case the same symbols with marker 29.
            assert_eq!(c(a, b'a'), (1, 0));
            assert_eq!(c(a, b'z'), (26, 0));
            assert_eq!(c(a, b'A'), (1, 29));
            assert_eq!(c(a, b'.'), (27, 0));
            assert_eq!(c(a, b'1'), (1, 30));
            assert_eq!(c(a, b'0'), (10, 30));
            assert_eq!(c(a, b'@'), (0, 29));
        }
        assert_eq!(c(Alphabet::Fsq, b' '), (0, 0));
        assert_eq!(c(Alphabet::Fsq, b'\n'), (28, 0));
        assert_eq!(c(Alphabet::Ifkp, b' '), (28, 0));
        assert_eq!(c(Alphabet::Ifkp, b'\n'), (28, 30));
        // IFKP sends the degree sign as 12 then 31.
        assert_eq!(c(Alphabet::Ifkp, 0xB0), (12, 31));
    }

    #[test]
    fn round_trip_every_character() {
        for a in [Alphabet::Fsq, Alphabet::Ifkp] {
            let (t, _) = a.table();
            let mut d = Decoder::new(a);
            let mut got = vec![];
            for &(b, _, _) in t.iter() {
                let code = a.code(b).unwrap();
                assert_eq!(a.lookup(code), Some(b));
                for &s in code.symbols() {
                    got.extend(d.push(s));
                }
            }
            got.extend(d.flush());
            let want: Vec<Result<u8>> = t.iter().map(|e| Ok(e.0)).collect();
            assert_eq!(got, want, "{a:?}");
        }
        assert!(Alphabet::Fsq.code(0x01).is_err());
        // A marker with nothing before it is ignored.
        assert_eq!(Decoder::new(Alphabet::Fsq).push(30), None);
    }
}
