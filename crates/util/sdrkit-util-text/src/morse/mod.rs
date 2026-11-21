//! International Morse code (ITU-R M.1677-1): the table with the procedure signals, a keyer that turns text into timed key-down and
//! key-up intervals, a decoder that reads mark and space lengths and follows the sending speed, and a list of identifications heard.
//!
//! Standard: ITU-R M.1677-1 Part I (the code in section 1, the timing in section 2).
//!
//! ```
//! use sdrkit_util_text::morse::{Decoder, DecoderConfig, Event, Keyer};
//!
//! let keyer = Keyer::new(20.0).unwrap();
//! let mut d = Decoder::new(DecoderConfig::following(20.0)).unwrap();
//! let mut text = String::new();
//! let mut put = |e: Event| match e {
//!     Event::Char(s) => text.push_str(s.text_or("*")),
//!     Event::Space => text.push(' '),
//! };
//! for k in keyer.key("CQ DE K0TST") {
//!     d.push(k.on, k.seconds, &mut put);
//! }
//! d.flush(&mut put);
//! assert_eq!(text, "CQ DE K0TST");
//! ```

use crate::error::{Error, Result};

/// A Morse character: up to 16 elements, the first in bit `len - 1`, 1 for a dash.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Code {
    /// The elements, the first sent in the highest of the `len` low bits; 1 is a dash.
    pub bits: u16,
    /// The number of elements.
    pub len: u8,
}

impl Code {
    /// The code written as dots and dashes (`".-"`); `-` and `−` are dashes.
    pub const fn parse(s: &str) -> Code {
        let b = s.as_bytes();
        let mut bits = 0u16;
        let mut len = 0u8;
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'.' || b[i] == b'-' {
                bits = (bits << 1) | (b[i] == b'-') as u16;
                len += 1;
            }
            i += 1;
        }
        Code { bits, len }
    }

    /// The elements in the order sent (`true` is a dash).
    pub fn elements(self) -> impl Iterator<Item = bool> {
        (0..self.len).rev().map(move |i| (self.bits >> i) & 1 == 1)
    }

    /// The length in dot units, element spaces included (not the trailing character space).
    pub fn units(self) -> u32 {
        self.elements().map(|d| if d { 3 } else { 1 }).sum::<u32>()
            + self.len.saturating_sub(1) as u32
    }

    const fn key(self) -> usize {
        (1usize << self.len) | self.bits as usize
    }
}

/// (text, code). Procedure signals are written in angle brackets. The first part is ITU-R M.1677-1 Part I section 1.1; the rest
/// (marked) are in common use but not in the Recommendation.
pub const TABLE: &[(&str, Code)] = &[
    ("A", Code::parse(".-")),
    ("B", Code::parse("-...")),
    ("C", Code::parse("-.-.")),
    ("D", Code::parse("-..")),
    ("E", Code::parse(".")),
    ("É", Code::parse("..-..")),
    ("F", Code::parse("..-.")),
    ("G", Code::parse("--.")),
    ("H", Code::parse("....")),
    ("I", Code::parse("..")),
    ("J", Code::parse(".---")),
    ("K", Code::parse("-.-")),
    ("L", Code::parse(".-..")),
    ("M", Code::parse("--")),
    ("N", Code::parse("-.")),
    ("O", Code::parse("---")),
    ("P", Code::parse(".--.")),
    ("Q", Code::parse("--.-")),
    ("R", Code::parse(".-.")),
    ("S", Code::parse("...")),
    ("T", Code::parse("-")),
    ("U", Code::parse("..-")),
    ("V", Code::parse("...-")),
    ("W", Code::parse(".--")),
    ("X", Code::parse("-..-")),
    ("Y", Code::parse("-.--")),
    ("Z", Code::parse("--..")),
    ("1", Code::parse(".----")),
    ("2", Code::parse("..---")),
    ("3", Code::parse("...--")),
    ("4", Code::parse("....-")),
    ("5", Code::parse(".....")),
    ("6", Code::parse("-....")),
    ("7", Code::parse("--...")),
    ("8", Code::parse("---..")),
    ("9", Code::parse("----.")),
    ("0", Code::parse("-----")),
    (".", Code::parse(".-.-.-")),
    (",", Code::parse("--..--")),
    (":", Code::parse("---...")),
    ("?", Code::parse("..--..")),
    ("'", Code::parse(".----.")),
    ("-", Code::parse("-....-")),
    ("/", Code::parse("-..-.")),
    ("(", Code::parse("-.--.")),
    (")", Code::parse("-.--.-")),
    ("\"", Code::parse(".-..-.")),
    ("=", Code::parse("-...-")),
    ("<SN>", Code::parse("...-.")),    // understood
    ("<HH>", Code::parse("........")), // error
    ("+", Code::parse(".-.-.")),       // cross; also end of telegram <AR>
    ("<AS>", Code::parse(".-...")),    // wait
    ("<SK>", Code::parse("...-.-")),   // end of work
    ("<KA>", Code::parse("-.-.-")),    // starting signal
    ("@", Code::parse(".--.-.")),
    // In common use, not in M.1677-1:
    ("!", Code::parse("-.-.--")),
    (";", Code::parse("-.-.-.")),
    ("_", Code::parse("..--.-")),
    ("$", Code::parse("...-..-")),
    ("<SOS>", Code::parse("...---...")),
];

/// Other spellings of entries above: the prosign names of `+`, `=` and `(`, the multiplication sign (sent as X, M.1677-1 3.2),
/// `&` for wait, and lower case letters (folded before lookup).
const ALIASES: &[(&str, &str)] = &[
    ("<AR>", "+"),
    ("<BT>", "="),
    ("<KN>", "("),
    ("<CT>", "<KA>"),
    ("<VE>", "<SN>"),
    ("×", "X"),
    ("&", "<AS>"),
];

/// Code key to table index + 1 (0: none). Keys are `1 << len | bits`, len up to 9.
const INDEX: [u8; 1024] = {
    let mut t = [0u8; 1024];
    let mut i = 0;
    while i < TABLE.len() {
        t[TABLE[i].1.key()] = i as u8 + 1;
        i += 1;
    }
    t
};

/// The text of a code (`None` when the table has no such character).
pub fn decode(code: Code) -> Option<&'static str> {
    if code.len == 0 || code.len > 9 {
        return None;
    }
    match INDEX[code.key()] {
        0 => None,
        i => Some(TABLE[i as usize - 1].0),
    }
}

/// The code of a symbol: one character (letters in either case) or a procedure signal in angle brackets.
pub fn encode(symbol: &str) -> Result<Code> {
    let mut buf = [0u8; 8];
    let up: &str = if symbol.len() <= buf.len() && symbol.is_ascii() {
        buf[..symbol.len()].copy_from_slice(symbol.as_bytes());
        buf[..symbol.len()].make_ascii_uppercase();
        std::str::from_utf8(&buf[..symbol.len()]).unwrap_or(symbol)
    } else if symbol == "é" {
        "É"
    } else {
        symbol
    };
    let up = ALIASES.iter().find(|a| a.0 == up).map_or(up, |a| a.1);
    TABLE
        .iter()
        .find(|e| e.0 == up)
        .map(|e| e.1)
        .ok_or_else(|| Error::Unencodable(symbol.chars().next().unwrap_or(' ')))
}

/// Splits text into symbols: single characters, and `<XX>` procedure signals.
pub fn symbols(text: &str) -> impl Iterator<Item = &str> {
    let mut rest = text;
    core::iter::from_fn(move || {
        let c = rest.chars().next()?;
        let n = if c == '<' {
            rest.find('>').map_or(c.len_utf8(), |e| e + 1)
        } else {
            c.len_utf8()
        };
        let (s, r) = rest.split_at(n);
        rest = r;
        Some(s)
    })
}

/// One key interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    /// Key down (a mark) or up (a space).
    pub on: bool,
    /// The length, seconds.
    pub seconds: f64,
}

/// Turns text into key intervals.
#[derive(Clone, Copy, Debug)]
pub struct Keyer {
    dot: f64,
    char_gap: f64,
    word_gap: f64,
}

impl Keyer {
    /// A keyer at `wpm` words a minute (PARIS timing). Speeds outside 1 to 200 are refused.
    pub fn new(wpm: f64) -> Result<Keyer> {
        Self::farnsworth(wpm, wpm)
    }

    /// Characters at `char_wpm`, gaps stretched to an overall `wpm` (Farnsworth). `wpm` must not exceed `char_wpm`.
    pub fn farnsworth(char_wpm: f64, wpm: f64) -> Result<Keyer> {
        if !(1.0..=200.0).contains(&char_wpm) || !(1.0..=char_wpm).contains(&wpm) {
            return Err(Error::OutOfRange);
        }
        let dot = 1.2 / char_wpm;
        // PARIS has 31 units in its characters and 19 in its gaps; the slower speed stretches only the gaps.
        let gaps = (60.0 / wpm - 31.0 * dot) / 19.0;
        Ok(Keyer {
            dot,
            char_gap: 3.0 * gaps,
            word_gap: 7.0 * gaps,
        })
    }

    /// The dot length, seconds.
    pub fn dot(&self) -> f64 {
        self.dot
    }

    /// The key intervals of a text, starting with a mark and with no trailing space. Spaces are word gaps; symbols the table does
    /// not have are skipped.
    pub fn key<'a>(&self, text: &'a str) -> impl Iterator<Item = Key> + 'a {
        let k = *self;
        let mut syms = symbols(text);
        let mut cur: Option<(Code, u8)> = None; // code and next element index
        let mut gap: f64 = 0.0; // a space owed before the next mark
        let mut started = false;
        let mut pending_mark: Option<f64> = None;
        core::iter::from_fn(move || loop {
            if let Some(m) = pending_mark.take() {
                return Some(Key {
                    on: true,
                    seconds: m,
                });
            }
            if let Some((code, i)) = cur {
                if i < code.len {
                    let dash = (code.bits >> (code.len - 1 - i)) & 1 == 1;
                    cur = Some((code, i + 1));
                    let mark = if dash { 3.0 * k.dot } else { k.dot };
                    let g = if i > 0 { k.dot } else { gap };
                    gap = 0.0;
                    if started && g > 0.0 {
                        pending_mark = Some(mark);
                        return Some(Key {
                            on: false,
                            seconds: g,
                        });
                    }
                    started = true;
                    return Some(Key {
                        on: true,
                        seconds: mark,
                    });
                }
                cur = None;
                gap = gap.max(k.char_gap);
            }
            let s = syms.next()?;
            if s == " " {
                gap = k.word_gap;
                continue;
            }
            if let Ok(code) = encode(s) {
                if gap == 0.0 {
                    gap = k.char_gap;
                }
                cur = Some((code, 0));
            }
        })
    }
}

mod decoder;
mod ident;

pub use decoder::{
    fit_dot, Classifier, Decoder, DecoderConfig, Event, Symbol, WordFit, HIST_BINS, HIST_MIN,
    HIST_SPAN,
};
pub use ident::IdentTracker;

#[cfg(test)]
mod tests {
    use super::*;

    /// ITU-R M.1677-1 Part I 1.1, as printed (dots and dashes).
    const M1677: &[(&str, &str)] = &[
        ("A", ".-"),
        ("B", "-..."),
        ("C", "-.-."),
        ("D", "-.."),
        ("E", "."),
        ("É", "..-.."),
        ("F", "..-."),
        ("G", "--."),
        ("H", "...."),
        ("I", ".."),
        ("J", ".---"),
        ("K", "-.-"),
        ("L", ".-.."),
        ("M", "--"),
        ("N", "-."),
        ("O", "---"),
        ("P", ".--."),
        ("Q", "--.-"),
        ("R", ".-."),
        ("S", "..."),
        ("T", "-"),
        ("U", "..-"),
        ("V", "...-"),
        ("W", ".--"),
        ("X", "-..-"),
        ("Y", "-.--"),
        ("Z", "--.."),
        ("1", ".----"),
        ("2", "..---"),
        ("3", "...--"),
        ("4", "....-"),
        ("5", "....."),
        ("6", "-...."),
        ("7", "--..."),
        ("8", "---.."),
        ("9", "----."),
        ("0", "-----"),
        (".", ".-.-.-"),
        (",", "--..--"),
        (":", "---..."),
        ("?", "..--.."),
        ("'", ".----."),
        ("-", "-....-"),
        ("/", "-..-."),
        ("(", "-.--."),
        (")", "-.--.-"),
        ("\"", ".-..-."),
        ("=", "-...-"),
        ("<SN>", "...-."),
        ("<HH>", "........"),
        ("+", ".-.-."),
        ("<KN>", "-.-"),
        ("<AS>", ".-..."),
        ("<SK>", "...-.-"),
        ("<KA>", "-.-.-"),
        ("×", "-..-"),
        ("@", ".--.-."),
    ];

    #[test]
    fn table_matches_m1677() {
        for (s, c) in M1677 {
            // "Invitation to transmit" is K itself; the table names "(" <KN>.
            let want = Code::parse(c);
            if *s == "<KN>" {
                assert_eq!(encode("K").unwrap(), want);
                continue;
            }
            assert_eq!(encode(s).unwrap(), want, "{s}");
        }
        for (s, c) in TABLE {
            assert_eq!(decode(*c), Some(*s), "{s}");
        }
        assert_eq!(encode("<AR>").unwrap(), Code::parse(".-.-."));
        assert_eq!(encode("<BT>").unwrap(), Code::parse("-...-"));
        assert_eq!(encode("k").unwrap(), Code::parse("-.-"));
        assert_eq!(decode(Code::parse("......---")), None);
        assert!(encode("#").is_err());
        let s: Vec<&str> = symbols("A<BT>é").collect();
        assert_eq!(s, ["A", "<BT>", "é"]);
    }

    #[test]
    fn paris_is_fifty_units() {
        let k = Keyer::new(12.0).unwrap();
        let total: f64 = k.key("PARIS").map(|i| i.seconds).sum::<f64>() + k.word_gap;
        assert!((total - 5.0).abs() < 1e-9, "{total}");
        // Farnsworth: 18 wpm characters at 10 wpm overall still take 6 s for PARIS and its word space.
        let f = Keyer::farnsworth(18.0, 10.0).unwrap();
        let total: f64 = f.key("PARIS").map(|i| i.seconds).sum::<f64>() + f.word_gap;
        assert!((total - 6.0).abs() < 1e-9, "{total}");
        assert!(Keyer::farnsworth(10.0, 18.0).is_err());
        assert!(Keyer::new(0.0).is_err());
    }

    #[test]
    fn keying_shape() {
        let k = Keyer::new(20.0).unwrap();
        let d = k.dot();
        let v: Vec<Key> = k.key("ET E").collect();
        let want = [
            Key {
                on: true,
                seconds: d,
            },
            Key {
                on: false,
                seconds: 3.0 * d,
            },
            Key {
                on: true,
                seconds: 3.0 * d,
            },
            Key {
                on: false,
                seconds: 7.0 * d,
            },
            Key {
                on: true,
                seconds: d,
            },
        ];
        assert_eq!(v.len(), want.len());
        for (a, b) in v.iter().zip(want.iter()) {
            assert_eq!(a.on, b.on);
            assert!((a.seconds - b.seconds).abs() < 1e-12);
        }
    }
}
