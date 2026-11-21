//! The page that the POCSAG and FLEX decoders output, and the text readings.
//!
//! A decoder keeps one [`Page`] and writes each new page into it. The decoder passes the page to the caller as `&Page`, and a
//! caller that keeps pages clones them. Each character that holds a bit of a lost message word shows as `?`, and the page is
//! marked `damaged`.

use crate::charset::{chars_lsb_first, NumericSet};
use sdrkit_util_text::ia5::{self, Parity, Printable};

/// The paging code of a page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// POCSAG (ITU-R M.584-2 annex 1).
    #[default]
    Pocsag,
    /// FLEX (ITU-R M.584-2 annex 2 and RCR STD-43).
    Flex,
}

/// What a page carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Kind {
    /// No message: an alert only.
    #[default]
    Tone,
    /// Digits and the other numeric characters.
    Numeric,
    /// Text.
    Alpha,
    /// A FLEX secure message. The words are in [`Page::data`].
    Secure,
    /// FLEX binary data. The words are in [`Page::data`].
    Binary,
    /// A FLEX instruction (for example a temporary address). The vector word is in [`Page::data`].
    Instruction,
}

impl Kind {
    /// "tone", "numeric", "alpha", "secure", "binary" or "instruction".
    pub const fn name(self) -> &'static str {
        match self {
            Kind::Tone => "tone",
            Kind::Numeric => "numeric",
            Kind::Alpha => "alpha",
            Kind::Secure => "secure",
            Kind::Binary => "binary",
            Kind::Instruction => "instruction",
        }
    }
}

/// How a POCSAG page is read as numeric or text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ReadMode {
    /// Text when the 7-bit reading looks like text, or when the function is 1 to 3 and the numeric reading is not plain digits.
    #[default]
    Auto,
    /// Function 0 is numeric, functions 1 to 3 are text (M.584-2 clauses 2.1 and 2.2 give 00 and 11).
    Function,
    /// Always text.
    Alpha,
    /// Always numeric.
    Numeric,
}

impl ReadMode {
    /// The name used in settings.
    pub const fn name(self) -> &'static str {
        match self {
            ReadMode::Auto => "auto",
            ReadMode::Function => "function",
            ReadMode::Alpha => "alpha",
            ReadMode::Numeric => "numeric",
        }
    }

    /// The mode with this name, or `None` for an unknown name.
    pub fn by_name(name: &str) -> Option<ReadMode> {
        [
            ReadMode::Auto,
            ReadMode::Function,
            ReadMode::Alpha,
            ReadMode::Numeric,
        ]
        .into_iter()
        .find(|m| m.name() == name)
    }
}

/// Why a POCSAG page was read as it was.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Reason {
    /// The page has no message, or the protocol says what it is (FLEX).
    #[default]
    Protocol,
    /// The function code.
    Function,
    /// The automatic rule.
    Auto,
    /// The read mode forces it.
    Forced,
    /// A rule for this address.
    Address,
}

/// Pages that a decoder did not give because they failed an acceptance rule, since the decoder was made or cleared.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refusals {
    /// All pages refused.
    pub total: u64,
    /// Of them, the pages refused by a rule that no public document states.
    pub unconfirmed: u64,
}

impl Refusals {
    /// Counts one page refused; `unconfirmed` when the rule is not stated by a public document.
    pub fn count(&mut self, unconfirmed: bool) {
        self.total += 1;
        self.unconfirmed += unconfirmed as u64;
    }
}

/// FLEX details of a page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlexInfo {
    /// The phase (0 to 3 in the order of [`crate::flex::Speed::phase_letters`]).
    pub phase: u8,
    /// The cycle number of the frame (0 to 14).
    pub cycle: u8,
    /// The frame number (0 to 127).
    pub frame: u8,
    /// The vector type (0 to 7).
    pub vector: u8,
    /// The address takes two words.
    pub long_address: bool,
    /// Text and binary: the fragment number (0 to 3).
    pub fragment: u8,
    /// Text and binary: more fragments follow.
    pub continued: bool,
    /// Text and binary: the message number (0 to 63).
    pub message_number: u8,
    /// The number of fragments joined into this page (1 when not joined).
    pub fragments: u8,
}

/// One page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    /// POCSAG or FLEX.
    pub protocol: Protocol,
    /// "POCSAG 1200", "FLEX 3200/4" and so on.
    pub rate: &'static str,
    /// The time the caller provided with the input, in seconds.
    pub time: f64,
    /// The POCSAG RIC or the FLEX capcode.
    pub address: u32,
    /// The POCSAG function code (0 to 3). `None` for FLEX.
    pub function: Option<u8>,
    /// What the page carries.
    pub kind: Kind,
    /// The text as read: the numeric or the text reading.
    pub text: String,
    /// POCSAG: the numeric reading.
    pub numeric: String,
    /// POCSAG: the text reading.
    pub alpha: String,
    /// The raw 7-bit characters of a text reading. A character that holds a bit of a lost word has bit 7 set.
    pub raw: Vec<u8>,
    /// FLEX secure, binary and instruction pages: the 21-bit values of the words.
    pub data: Vec<u32>,
    /// POCSAG: the 7-bit reading looks like text.
    pub looks_like_text: bool,
    /// Why the page was read as it was.
    pub reason: Reason,
    /// Bits that the repairs changed over the page.
    pub bits_repaired: u32,
    /// Message words that were lost.
    pub lost_words: u32,
    /// True when a word of the message was lost, or the end of the message is not sure.
    pub damaged: bool,
    /// Copies voted into this page (1 when no other copy helped).
    pub copies: u32,
    /// True when other copies changed a word of this page.
    pub combined: bool,
    /// FLEX details. `None` for POCSAG.
    pub flex: Option<FlexInfo>,
}

impl Page {
    /// Empties the page and keeps the capacity of its buffers.
    pub fn clear(&mut self) {
        self.protocol = Protocol::Pocsag;
        self.rate = "";
        self.time = 0.0;
        self.address = 0;
        self.function = None;
        self.kind = Kind::Tone;
        self.text.clear();
        self.numeric.clear();
        self.alpha.clear();
        self.raw.clear();
        self.data.clear();
        self.looks_like_text = false;
        self.reason = Reason::Protocol;
        self.bits_repaired = 0;
        self.lost_words = 0;
        self.damaged = false;
        self.copies = 1;
        self.combined = false;
        self.flex = None;
    }
}

/// The options of the printable text reading: CR, LF and tab kept, other control characters dropped.
pub const PRINTABLE: Printable = Printable {
    parity: Parity::None,
    replacement: None,
    line_breaks: true,
    tabs: true,
    stop_at_etx: false,
    max_chars: usize::MAX,
};

/// Message bits with a mark for each bit of a lost word.
#[derive(Clone, Debug, Default)]
pub struct MessageBits {
    /// The bits (0 or 1), first sent first.
    pub bits: Vec<u8>,
    /// 1 where the bit belongs to a lost word.
    pub lost: Vec<u8>,
}

impl MessageBits {
    /// Bits for messages of up to `capacity` bits without allocation.
    pub fn with_capacity(capacity: usize) -> MessageBits {
        MessageBits {
            bits: Vec::with_capacity(capacity),
            lost: Vec::with_capacity(capacity),
        }
    }

    /// Empties both buffers.
    pub fn clear(&mut self) {
        self.bits.clear();
        self.lost.clear();
    }

    /// Appends the `n` low bits of `v`, most significant first.
    pub fn push_msb(&mut self, v: u32, n: usize) {
        self.bits.extend((0..n).rev().map(|i| ((v >> i) & 1) as u8));
        self.lost.extend(std::iter::repeat_n(0, n));
    }

    /// Appends the `n` low bits of `v`, least significant first.
    pub fn push_lsb(&mut self, v: u32, n: usize) {
        self.bits.extend((0..n).map(|i| ((v >> i) & 1) as u8));
        self.lost.extend(std::iter::repeat_n(0, n));
    }

    /// Appends `n` bits of a lost word.
    pub fn push_lost(&mut self, n: usize) {
        self.bits.extend(std::iter::repeat_n(0, n));
        self.lost.extend(std::iter::repeat_n(1, n));
    }

    /// Bits held.
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// True when no bit is held.
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    fn char_lost(&self, i: usize, width: usize) -> bool {
        self.lost[i * width..(i + 1) * width]
            .iter()
            .any(|&l| l != 0)
    }

    /// Writes the numeric reading from bit `skip` on into `out`: lost characters as `?`, values listed in `drop` left out, and
    /// trailing spaces trimmed.
    pub fn numeric_into(&self, skip: usize, set: &[u8; 16], drop: Option<u8>, out: &mut String) {
        let bits = self.bits.get(skip..).unwrap_or(&[]);
        for (i, v) in chars_lsb_first(bits, 4).enumerate() {
            if self.char_lost_at(skip, i, 4) {
                out.push('?');
            } else if Some(v) != drop {
                out.push(set[v as usize] as char);
            }
        }
        let t = out.trim_end_matches(' ').len();
        out.truncate(t);
    }

    fn char_lost_at(&self, skip: usize, i: usize, width: usize) -> bool {
        let a = skip + i * width;
        self.lost[a..a + width].iter().any(|&l| l != 0)
    }

    /// Writes the raw 7-bit characters into `raw`. A character that holds a lost bit has bit 7 set.
    pub fn alpha_raw_into(&self, raw: &mut Vec<u8>) {
        for (i, c) in chars_lsb_first(&self.bits, 7).enumerate() {
            raw.push(if self.char_lost(i, 7) { c | 0x80 } else { c });
        }
    }
}

/// Writes the printable reading of raw 7-bit characters into `out`. A lost character (bit 7 set) becomes `?`. CR, LF and tab
/// stay. Other control characters (NUL, ETX, EOT and the rest) are dropped.
pub fn printable_into(raw: &[u8], out: &mut String) {
    for run in raw.split_inclusive(|&c| c & 0x80 != 0) {
        match run.split_last() {
            Some((&last, head)) if last & 0x80 != 0 => {
                ia5::printable_into(head, &PRINTABLE, out);
                out.push('?');
            }
            _ => {
                ia5::printable_into(run, &PRINTABLE, out);
            }
        }
    }
}

/// True when a 7-bit reading looks like text. The rule counts letters, digits, spaces, control characters and other signs.
/// Short readings (under 12 characters) must be mostly letters with no control character.
pub fn looks_like_text(raw: &[u8]) -> bool {
    let mut end = raw.len();
    while end > 0 && matches!(raw[end - 1], 0 | 3 | 4) {
        end -= 1;
    }
    let s = &raw[..end];
    let n = s.len();
    if n < 3 {
        return false;
    }
    let is_letter = |c: u8| c.is_ascii_alphabetic();
    let is_space = |c: u8| c == b' ' || c == b'\r' || c == b'\n';
    let is_ctrl = |c: u8| (c < 0x20 && c != b'\r' && c != b'\n') || c >= 0x7F;
    let is_ok_sign = |c: u8| b".,:;-/()#&'!?@+=*\"".contains(&c);
    let (mut letters, mut digits, mut spaces, mut ctrls, mut odd, mut ctrl_runs) =
        (0, 0, 0, 0, 0, 0);
    let (mut pairs, mut lower_upper) = (0, 0);
    let mut prev_ctrl = false;
    for (i, &c) in s.iter().enumerate() {
        if is_letter(c) {
            letters += 1;
        } else if c.is_ascii_digit() {
            digits += 1;
        } else if is_space(c) {
            spaces += 1;
        } else if is_ctrl(c) {
            ctrls += 1;
        } else if !is_ok_sign(c) {
            odd += 1;
        }
        let ctrl = is_ctrl(c);
        if ctrl && prev_ctrl {
            ctrl_runs += 1;
        }
        prev_ctrl = ctrl;
        if i > 0 && is_letter(s[i - 1]) && is_letter(c) {
            pairs += 1;
            if s[i - 1].is_ascii_lowercase() && c.is_ascii_uppercase() {
                lower_upper += 1;
            }
        }
    }
    let lu_ok = lower_upper * 8 <= pairs;
    if n < 12 {
        let other = n - letters - digits - spaces;
        return n >= 6 && ctrls == 0 && odd == 0 && other <= 1 && letters * 2 >= n && lu_ok;
    }
    if ctrl_runs > 0
        || ctrls * 10 > n
        || (letters + digits + spaces) * 100 < n * 85
        || odd * 10 > n
        || !lu_ok
    {
        return false;
    }
    letters * 4 >= n
}

/// Chooses the reading of a POCSAG page with a message: (text?, why). A rule for the address wins over the mode.
pub fn choose_reading(
    mode: ReadMode,
    function: u8,
    numeric: &str,
    looks: bool,
    by_address: Option<bool>,
) -> (bool, Reason) {
    if let Some(alpha) = by_address {
        return (alpha, Reason::Address);
    }
    match mode {
        ReadMode::Alpha => (true, Reason::Forced),
        ReadMode::Numeric => (false, Reason::Forced),
        ReadMode::Function => (function != 0, Reason::Function),
        ReadMode::Auto => {
            let plain = numeric
                .chars()
                .all(|c| c.is_ascii_digit() || c == ' ' || c == '-');
            (
                looks || ((1..=3).contains(&function) && !plain),
                Reason::Auto,
            )
        }
    }
}

/// The numeric reading of a POCSAG message with a numeric set (a helper for callers with their own bits).
pub fn numeric_text(bits: &MessageBits, set: NumericSet, out: &mut String) {
    bits.numeric_into(0, set.chars(), None, out);
}
