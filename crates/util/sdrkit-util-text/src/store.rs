//! The decoded-text store every text stack keeps: the newest characters, the chat lines (each with its time, frequency and source),
//! and the callsigns seen. It is filled one character at a time and allocates nothing after it is built: its buffers are sized by
//! [`Limits`] and recycled in rings.
//!
//! ```
//! use sdrkit_util_text::store::{Limits, TextStore};
//!
//! let mut s = TextStore::new(Limits::default());
//! for (i, c) in "CQ DE K0TST K\n".chars().enumerate() {
//!     s.push(c, i as f64 * 0.1, 14_070_000.0, 1);
//! }
//! assert_eq!(s.lines().next().unwrap().text, "CQ DE K0TST K");
//! assert_eq!(s.callsigns().collect::<Vec<_>>(), ["K0TST"]);
//! ```

use crate::callsign;
use std::collections::VecDeque;

/// One line of decoded text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatLine {
    /// The signal time of its first character, seconds.
    pub start_s: f64,
    /// The frequency of the signal when the line started, Hz.
    pub freq_hz: f64,
    /// Who decoded it: an identifier the caller chooses (a signal number, a decoder slot).
    pub source: u32,
    /// The text, without the line end.
    pub text: String,
}

/// The sizes of a store.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Characters kept in the running text.
    pub chars: usize,
    /// Finished lines kept.
    pub lines: usize,
    /// The longest line, in characters; a longer one is cut there.
    pub line_chars: usize,
    /// A pause this long (seconds) between characters ends the line.
    pub pause_s: f64,
    /// Callsigns kept (the oldest is dropped first).
    pub callsigns: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            chars: 8000,
            lines: 80,
            line_chars: 200,
            pause_s: 4.0,
            callsigns: 100,
        }
    }
}

/// A fixed ring of reusable items.
#[derive(Clone, Debug)]
struct Ring<T> {
    items: Vec<T>,
    head: usize,
    len: usize,
}

impl<T> Ring<T> {
    fn iter(&self) -> impl Iterator<Item = &T> {
        (0..self.len).map(move |i| &self.items[(self.head + i) % self.items.len()])
    }
    /// The slot the next item goes into (the oldest is dropped when full).
    fn next_slot(&mut self) -> &mut T {
        let n = self.items.len();
        let i = if self.len < n {
            self.len += 1;
            (self.head + self.len - 1) % n
        } else {
            let i = self.head;
            self.head = (self.head + 1) % n;
            i
        };
        &mut self.items[i]
    }
}

/// The store.
#[derive(Clone, Debug)]
pub struct TextStore {
    limits: Limits,
    text: VecDeque<char>,
    lines: Ring<ChatLine>,
    current: ChatLine,
    current_chars: usize,
    last_t: f64,
    after_cr: bool,
    word: String,
    calls: Ring<String>,
    utf8: [u8; 4],
    utf8_len: u8,
    /// Characters received since the store was made (not reset by [`TextStore::clear`]).
    pub total: u64,
}

impl TextStore {
    /// A store with these limits (each at least 1).
    pub fn new(limits: Limits) -> Self {
        let l = Limits {
            chars: limits.chars.max(1),
            lines: limits.lines.max(1),
            line_chars: limits.line_chars.max(1),
            callsigns: limits.callsigns.max(1),
            pause_s: if limits.pause_s.is_nan() {
                f64::INFINITY
            } else {
                limits.pause_s
            },
        };
        let line = || ChatLine {
            text: String::with_capacity(4 * l.line_chars),
            ..ChatLine::default()
        };
        TextStore {
            limits: l,
            text: VecDeque::with_capacity(l.chars + 1),
            lines: Ring {
                items: (0..l.lines).map(|_| line()).collect(),
                head: 0,
                len: 0,
            },
            current: line(),
            current_chars: 0,
            last_t: f64::NEG_INFINITY,
            after_cr: false,
            word: String::with_capacity(4 * callsign::MAX_LEN),
            calls: Ring {
                items: (0..l.callsigns)
                    .map(|_| String::with_capacity(4 * callsign::MAX_LEN))
                    .collect(),
                head: 0,
                len: 0,
            },
            utf8: [0; 4],
            utf8_len: 0,
            total: 0,
        }
    }

    /// Adds one decoded byte of UTF-8 text (RFC 3629): the bytes of a character are held until it is whole, then it is passed to
    /// [`TextStore::push`]. A byte that cannot continue or start a character produces U+FFFD (the replacement character) for the
    /// bytes held, and an ASCII byte is added at once. Returns a callsign seen for the first time.
    ///
    /// ```
    /// use sdrkit_util_text::store::{Limits, TextStore};
    /// let mut s = TextStore::new(Limits::default());
    /// for &b in "Grüße\n".as_bytes() {
    ///     s.push_byte(b, 0.0, 0.0, 0);
    /// }
    /// assert_eq!(s.lines().next().unwrap().text, "Grüße");
    /// ```
    pub fn push_byte(&mut self, b: u8, t: f64, freq_hz: f64, source: u32) -> Option<&str> {
        let held = self.utf8_len as usize;
        let need = |lead: u8| match lead {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => 0,
        };
        if held > 0 && (b & 0xC0) == 0x80 {
            self.utf8[held] = b;
            self.utf8_len += 1;
            let n = need(self.utf8[0]);
            if self.utf8_len as usize == n {
                self.utf8_len = 0;
                let c = std::str::from_utf8(&self.utf8[..n])
                    .ok()
                    .and_then(|s| s.chars().next())
                    .unwrap_or(char::REPLACEMENT_CHARACTER);
                return self.push(c, t, freq_hz, source);
            }
            return None;
        }
        // The bytes held do not make a whole character.
        let broken = held > 0;
        self.utf8_len = 0;
        if broken {
            let _ = self.push(char::REPLACEMENT_CHARACTER, t, freq_hz, source);
        }
        match b {
            0x00..=0x7F => self.push(b as char, t, freq_hz, source),
            _ if need(b) > 0 => {
                self.utf8[0] = b;
                self.utf8_len = 1;
                None
            }
            _ => self.push(char::REPLACEMENT_CHARACTER, t, freq_hz, source),
        }
    }

    /// Adds one decoded character at signal time `t` (seconds) from a signal at `freq_hz`, decoded by `source`. CR LF, a lone CR
    /// and LF end the line; backspace (U+0008) removes the last character; other control characters are dropped. Returns a callsign
    /// seen for the first time.
    pub fn push(&mut self, c: char, t: f64, freq_hz: f64, source: u32) -> Option<&str> {
        let c = match c {
            '\r' => {
                self.after_cr = true;
                '\n'
            }
            '\n' if std::mem::take(&mut self.after_cr) => return None,
            '\u{8}' => {
                self.after_cr = false;
                self.backspace();
                return None;
            }
            '\n' | '\t' => c,
            c if c.is_control() => {
                self.after_cr = false;
                return None;
            }
            c => {
                self.after_cr = false;
                c
            }
        };
        self.total += 1;
        if self.text.len() == self.limits.chars {
            self.text.pop_front();
        }
        self.text.push_back(c);
        let pause = t - self.last_t > self.limits.pause_s;
        self.last_t = t;
        if pause || self.current_chars >= self.limits.line_chars {
            self.finish_line();
        }
        if c == '\n' {
            self.finish_line();
        } else {
            if self.current_chars == 0 {
                self.current.start_s = t;
                self.current.freq_hz = freq_hz;
                self.current.source = source;
            }
            self.current.text.push(c);
            self.current_chars += 1;
        }
        if c.is_ascii_alphanumeric() || c == '/' {
            if self.word.len() < 4 * callsign::MAX_LEN {
                self.word.push(c);
            }
            None
        } else {
            self.end_word()
        }
    }

    fn backspace(&mut self) {
        if matches!(self.text.back(), Some(&p) if p != '\n') {
            self.text.pop_back();
            if self.current.text.pop().is_some() {
                self.current_chars -= 1;
            }
            self.word.pop();
        }
    }

    /// Ends the line being written (also done by a line end or a pause).
    pub fn finish_line(&mut self) {
        if self.current_chars == 0 {
            return;
        }
        let slot = self.lines.next_slot();
        std::mem::swap(slot, &mut self.current);
        self.current.text.clear();
        self.current_chars = 0;
    }

    /// Ends the word being written (at a pause, or the end of a transmission). Returns a callsign seen for the first time.
    pub fn end_word(&mut self) -> Option<&str> {
        if self.word.is_empty() {
            return None;
        }
        let new = callsign::is_callsign(&self.word) && !self.calls.iter().any(|c| *c == self.word);
        if !new {
            self.word.clear();
            return None;
        }
        let slot = self.calls.next_slot();
        slot.clear();
        slot.push_str(&self.word);
        self.word.clear();
        Some(slot.as_str())
    }

    /// The newest characters, oldest first.
    pub fn text(&self) -> impl Iterator<Item = char> + '_ {
        self.text.iter().copied()
    }

    /// The finished lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &ChatLine> {
        self.lines.iter()
    }

    /// The line being written.
    pub fn current(&self) -> &ChatLine {
        &self.current
    }

    /// The callsigns seen, first seen first.
    pub fn callsigns(&self) -> impl Iterator<Item = &str> {
        self.calls.iter().map(|s| s.as_str())
    }

    /// Whether the newest text looks like text: of the last 40 characters (at least 12), 90 % or more are readable
    /// ([`crate::score::is_readable`]).
    pub fn plausible(&self) -> bool {
        let n = self.text.len().min(40);
        if n < 12 {
            return false;
        }
        let good = self
            .text
            .iter()
            .skip(self.text.len() - n)
            .filter(|c| crate::score::is_readable(**c))
            .count();
        good * 10 >= n * 9
    }

    /// Empties the text, the lines and the callsigns (the total stays).
    pub fn clear(&mut self) {
        self.text.clear();
        self.lines.len = 0;
        self.lines.head = 0;
        self.current.text.clear();
        self.current_chars = 0;
        self.calls.len = 0;
        self.calls.head = 0;
        self.word.clear();
        self.after_cr = false;
        self.utf8_len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(s: &mut TextStore, text: &str, t0: f64) -> Vec<String> {
        let mut calls = vec![];
        for (i, c) in text.chars().enumerate() {
            if let Some(cs) = s.push(c, t0 + i as f64 * 0.1, 7_040_000.0, 3) {
                calls.push(cs.to_string());
            }
        }
        calls
    }

    #[test]
    fn line_ends_backspace_and_caps() {
        let mut s = TextStore::new(Limits::default());
        feed(&mut s, "AB\r\nC\u{8}D\u{1}\nE", 0.0);
        assert_eq!(s.text().collect::<String>(), "AB\nD\nE");
        let l: Vec<&str> = s.lines().map(|l| l.text.as_str()).collect();
        assert_eq!(l, ["AB", "D"]);
        assert_eq!(s.current().text, "E");
        let mut s = TextStore::new(Limits {
            chars: 5,
            lines: 2,
            line_chars: 3,
            ..Limits::default()
        });
        feed(&mut s, "0123456789", 0.0);
        assert_eq!(s.text().collect::<String>(), "56789");
        assert_eq!(s.total, 10);
        let l: Vec<&str> = s.lines().map(|l| l.text.as_str()).collect();
        assert_eq!(l, ["345", "678"]);
        assert_eq!(s.current().text, "9");
    }

    #[test]
    fn pauses_cut_lines_and_lines_carry_their_source() {
        let mut s = TextStore::new(Limits::default());
        feed(&mut s, "CQ CQ", 0.0);
        s.push('X', 10.0, 7_041_000.0, 4);
        let first = s.lines().next().unwrap();
        assert_eq!(
            (
                first.text.as_str(),
                first.start_s,
                first.freq_hz,
                first.source
            ),
            ("CQ CQ", 0.0, 7_040_000.0, 3)
        );
        assert_eq!((s.current().text.as_str(), s.current().source), ("X", 4));
    }

    #[test]
    fn callsigns_are_reported_once() {
        let mut s = TextStore::new(Limits {
            callsigns: 2,
            ..Limits::default()
        });
        let seen = feed(&mut s, "CQ DE K0TST K0TST DL1XYZ 9A1AA ", 0.0);
        assert_eq!(seen, ["K0TST", "DL1XYZ", "9A1AA"]);
        assert_eq!(s.callsigns().collect::<Vec<_>>(), ["DL1XYZ", "9A1AA"]);
        assert!(s.plausible());
        s.clear();
        assert_eq!(s.text().count(), 0);
        assert_eq!(s.callsigns().count(), 0);
    }

    /// RFC 3629 section 3 encodings, and the bad sequences of its section 10.
    #[test]
    fn utf8_bytes() {
        let mut s = TextStore::new(Limits::default());
        let mut feed = |bytes: &[u8]| {
            for &b in bytes {
                s.push_byte(b, 0.0, 0.0, 0);
            }
            s.push_byte(b'\n', 0.0, 0.0, 0);
            s.lines().last().unwrap().text.clone()
        };
        // U+00E9, U+20AC, U+1F4E1, and ASCII.
        assert_eq!(
            feed(&[0xC3, 0xA9, 0xE2, 0x82, 0xAC, 0xF0, 0x9F, 0x93, 0xA1, b'A']),
            "é€📡A"
        );
        // A lead byte cut short, a lone continuation byte, an overlong form, a surrogate, and a byte never used.
        assert_eq!(feed(&[0xC3, b'A']), "\u{FFFD}A");
        assert_eq!(feed(&[0x80, b'B']), "\u{FFFD}B");
        assert_eq!(feed(&[0xC0, 0x80]), "\u{FFFD}\u{FFFD}");
        assert_eq!(feed(&[0xED, 0xA0, 0x80]), "\u{FFFD}");
        assert_eq!(feed(&[0xFF, b'C']), "\u{FFFD}C");
    }
}
