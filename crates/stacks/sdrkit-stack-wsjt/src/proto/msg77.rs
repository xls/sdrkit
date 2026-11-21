//! The 77-bit message of FT8, FT4, FT2, Q65, FST4 and MSK144: packing, unpacking and the callsign hash table.
//! Standard: QEX July/August 2020, "The FT4 and FT8 Communication Protocols".

use sdrkit_util_bits::pack::{push_u64 as put_bits, to_u64 as get_bits};
use std::collections::{HashMap, VecDeque};

const NTOKENS: u32 = 2_063_592;
const MAX22: u32 = 4_194_304;
const MAXGRID4: u32 = 32_400;
const A1: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const A2: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const A4: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ";
/// The alphabet of hashed and nonstandard calls.
pub const A38: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ/";
/// The free-text alphabet.
pub const A42: &str = " 0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ+-./?";

/// The ARRL and RAC sections of the Field Day message, in protocol order.
pub const ARRL_SECTIONS: [&str; 86] = [
    "AB", "AK", "AL", "AR", "AZ", "BC", "CO", "CT", "DE", "EB", "EMA", "ENY", "EPA", "EWA", "GA",
    "GH", "IA", "ID", "IL", "IN", "KS", "KY", "LA", "LAX", "NS", "MB", "MDC", "ME", "MI", "MN",
    "MO", "MS", "MT", "NC", "ND", "NE", "NFL", "NH", "NL", "NLI", "NM", "NNJ", "NNY", "TER", "NTX",
    "NV", "OH", "OK", "ONE", "ONN", "ONS", "OR", "ORG", "PAC", "PR", "QC", "RI", "SB", "SC", "SCV",
    "SD", "SDG", "SF", "SFL", "SJV", "SK", "SNJ", "STX", "SV", "TN", "UT", "VA", "VI", "VT", "WCF",
    "WI", "WMA", "WNY", "WPA", "WTX", "WV", "WWA", "WY", "DX", "PE", "NB",
];

/// The RTTY Roundup multipliers: US states, Canadian provinces and territories (the 171-entry list ends with the numbered DX
/// entries, which [`rtty_mult`] makes).
const RTTY_NAMED: [&str; 72] = [
    "AL", "AK", "AZ", "AR", "CA", "CO", "CT", "DE", "FL", "GA", "HI", "ID", "IL", "IN", "IA", "KS",
    "KY", "LA", "ME", "MD", "MA", "MI", "MN", "MS", "MO", "MT", "NE", "NV", "NH", "NJ", "NM", "NY",
    "NC", "ND", "OH", "OK", "OR", "PA", "RI", "SC", "SD", "TN", "TX", "UT", "VT", "VA", "WA", "WV",
    "WI", "WY", "NB", "NS", "QC", "ON", "MB", "SK", "AB", "BC", "NWT", "NF", "LB", "NU", "YT",
    "PEI", "DC", "DR", "FR", "GD", "GR", "OV", "ZH", "ZL",
];

/// Multiplier `i` (0-based) of the 171-entry RTTY Roundup list.
pub fn rtty_mult(i: usize) -> Option<String> {
    if i < RTTY_NAMED.len() {
        Some(RTTY_NAMED[i].to_string())
    } else if i < 171 {
        Some(format!("X{:02}", i - RTTY_NAMED.len() + 1))
    } else {
        None
    }
}

/// Why a text cannot be packed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PackError {
    /// The message is empty.
    #[error("an empty message cannot be sent")]
    Empty,
    /// A word that must be a callsign is not one.
    #[error("a word of the message is not a callsign")]
    NotACallsign,
    /// The word after CQ cannot be packed.
    #[error("the word after CQ cannot be packed")]
    Cq,
    /// Free text is longer than 13 characters.
    #[error("free text holds at most 13 characters")]
    TooLong,
    /// Free text holds a character that the mode cannot send.
    #[error("the message has characters that free text cannot carry")]
    Characters,
    /// The text is not a WSPR message (call, locator, power).
    #[error("the text is not a WSPR message")]
    NotWspr,
    /// WSPR cannot send this power.
    #[error("WSPR sends 0 to 60 dBm, ending in 0, 3 or 7")]
    Power,
    /// A heartbeat cannot carry this callsign or locator.
    #[error("this callsign or locator cannot be sent in a heartbeat")]
    Heartbeat,
    /// JS8 cannot send the message.
    #[error("the message cannot be sent in JS8")]
    NotJs8,
    /// The JS8 command is not known.
    #[error("unknown JS8 command")]
    UnknownCommand,
    /// The text does not fit in one JS8 frame.
    #[error("the text does not fit in one frame")]
    FrameFull,
    /// JS8 frames are sent by the JS8 sender, not as text.
    #[error("JS8 sends frames: use the JS8 sender")]
    Js8Frames,
}

fn err<T>(e: PackError) -> Result<T, PackError> {
    Err(e)
}

/// The 22-bit hash of a call, and its 12- and 10-bit tops.
pub fn hash22(call: &str) -> u32 {
    let c = call.trim().to_ascii_uppercase();
    let mut n: u64 = 0;
    for i in 0..11 {
        let ch = c.as_bytes().get(i).copied().unwrap_or(b' ') as char;
        let j = A38.find(ch).unwrap_or(0) as u64;
        n = n.wrapping_mul(38).wrapping_add(j);
    }
    (47_055_833_459u64.wrapping_mul(n) >> 42) as u32 & 0x3F_FFFF
}

/// The 12-bit hash.
pub fn hash12(call: &str) -> u32 {
    hash22(call) >> 10
}

/// The 10-bit hash.
pub fn hash10(call: &str) -> u32 {
    hash22(call) >> 12
}

/// The callsign hash table of a receiver: every full call seen, by its 22-bit hash. The oldest learnt is forgotten
/// first beyond the cap.
#[derive(Clone, Debug, Default)]
pub struct CallHashes {
    by22: HashMap<u32, String>,
    order: VecDeque<u32>,
}

impl CallHashes {
    /// The most calls kept.
    pub const CAP: usize = 4096;

    /// Learns a call (ignored when it is not a plausible call).
    pub fn learn(&mut self, call: &str) {
        let c = call.trim().trim_matches(|ch| ch == '<' || ch == '>');
        if c.len() < 3 || c.len() > 11 || c == "..." || !c.chars().any(|ch| ch.is_ascii_digit()) {
            return;
        }
        if !c.chars().all(|ch| A38.contains(ch) && ch != ' ') {
            return;
        }
        let h = hash22(c);
        if self.by22.insert(h, c.to_string()).is_none() {
            self.order.push_back(h);
            while self.order.len() > Self::CAP {
                if let Some(old) = self.order.pop_front() {
                    self.by22.remove(&old);
                }
            }
        }
    }
    /// The call with this 22-bit hash.
    pub fn get22(&self, h: u32) -> Option<&str> {
        self.by22.get(&h).map(|s| s.as_str())
    }
    /// The call with this 12-bit hash.
    pub fn get12(&self, h: u32) -> Option<&str> {
        self.by22
            .iter()
            .find(|(k, _)| *k >> 10 == h)
            .map(|(_, v)| v.as_str())
    }
    /// The call with this 10-bit hash.
    pub fn get10(&self, h: u32) -> Option<&str> {
        self.by22
            .iter()
            .find(|(k, _)| *k >> 12 == h)
            .map(|(_, v)| v.as_str())
    }
    /// Calls known.
    pub fn len(&self) -> usize {
        self.by22.len()
    }
    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.by22.is_empty()
    }
    /// Forgets everything.
    pub fn clear(&mut self) {
        self.by22.clear();
        self.order.clear();
    }
    /// Every call known, oldest first.
    pub fn calls(&self) -> Vec<String> {
        self.order
            .iter()
            .filter_map(|h| self.by22.get(h).cloned())
            .collect()
    }
}

/// Whether `c` is a standard call: up to 6 characters after the special prefix rules, with a digit in the 2nd or 3rd place, letters
/// after it (up to 3).
pub fn is_standard_call(c: &str) -> bool {
    std_call_value(c).is_some()
}

fn std_call_value(call: &str) -> Option<u32> {
    let mut c = call.to_string();
    if c.starts_with("3DA0") && c.len() <= 7 {
        c = format!("3D0{}", &c[4..]);
    } else if c.len() >= 3 && c.starts_with("3X") && c.as_bytes()[2].is_ascii_alphabetic() {
        c = format!("Q{}", &c[2..]);
    }
    let b = c.as_bytes();
    if b.is_empty() || b.len() > 6 {
        return None;
    }
    let six: Vec<u8> = if b.len() >= 3 && b[2].is_ascii_digit() {
        let mut v = b.to_vec();
        v.resize(6, b' ');
        v
    } else if b.len() >= 2 && b[1].is_ascii_digit() && b.len() <= 5 {
        let mut v = vec![b' '];
        v.extend_from_slice(b);
        v.resize(6, b' ');
        v
    } else {
        return None;
    };
    // The suffix: letters then spaces only.
    let suffix = &six[3..];
    let letters = suffix.iter().take_while(|c| c.is_ascii_uppercase()).count();
    if suffix[letters..].iter().any(|&c| c != b' ') {
        return None;
    }
    let i1 = A1.find(six[0] as char)? as u32;
    let i2 = A2.find(six[1] as char)? as u32;
    if !six[2].is_ascii_digit() {
        return None;
    }
    let i3 = (six[2] - b'0') as u32;
    let i4 = A4.find(six[3] as char)? as u32;
    let i5 = A4.find(six[4] as char)? as u32;
    let i6 = A4.find(six[5] as char)? as u32;
    // A call must have at least one letter or digit before the area digit.
    if six[0] == b' ' && six[1] == b' ' {
        return None;
    }
    Some(((((i1 * 36 + i2) * 10 + i3) * 27 + i4) * 27 + i5) * 27 + i6)
}

fn std_call_text(n: u32) -> String {
    let mut n = n;
    let i6 = n % 27;
    n /= 27;
    let i5 = n % 27;
    n /= 27;
    let i4 = n % 27;
    n /= 27;
    let i3 = n % 10;
    n /= 10;
    let i2 = n % 36;
    n /= 36;
    let i1 = n;
    let ch = |a: &str, i: u32| a.as_bytes().get(i as usize).copied().unwrap_or(b'?') as char;
    let s: String = [
        ch(A1, i1),
        ch(A2, i2),
        (b'0' + i3 as u8) as char,
        ch(A4, i4),
        ch(A4, i5),
        ch(A4, i6),
    ]
    .iter()
    .collect();
    let s = s.trim().to_string();
    if let Some(rest) = s.strip_prefix("3D0") {
        format!("3DA0{rest}")
    } else if s.starts_with('Q') && s.len() > 1 && s.as_bytes()[1].is_ascii_alphabetic() {
        format!("3X{}", &s[1..])
    } else {
        s
    }
}

/// Packs a call field (28 bits): tokens, standard calls, or the 22-bit hash of anything else.
fn pack28(t: &str) -> Result<u32, PackError> {
    match t {
        "DE" => return Ok(0),
        "QRZ" => return Ok(1),
        "CQ" => return Ok(2),
        _ => {}
    }
    if let Some(rest) = t.strip_prefix("CQ_").or_else(|| t.strip_prefix("CQ ")) {
        if rest.len() == 3 && rest.bytes().all(|c| c.is_ascii_digit()) {
            let n = rest.bytes().fold(0u32, |n, c| 10 * n + u32::from(c - b'0'));
            return Ok(3 + n);
        }
        if (1..=4).contains(&rest.len()) && rest.bytes().all(|c| c.is_ascii_uppercase()) {
            let m = rest
                .bytes()
                .fold(0u32, |m, c| 27 * m + u32::from(c - b'A') + 1);
            return Ok(1003 + m);
        }
        return err(PackError::Cq);
    }
    if let Some(v) = std_call_value(t) {
        return Ok(NTOKENS + MAX22 + v);
    }
    let bare = t.trim_matches(|c| c == '<' || c == '>');
    if is_plausible_call(bare) {
        return Ok(NTOKENS + hash22(bare));
    }
    err(PackError::NotACallsign)
}

/// Whether a text looks like a callsign (3 to 11 characters of letters, digits and `/`, with a digit).
pub fn is_plausible(c: &str) -> bool {
    is_plausible_call(c)
}

fn is_plausible_call(c: &str) -> bool {
    (3..=11).contains(&c.len())
        && c.chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '/')
        && c.chars().any(|ch| ch.is_ascii_digit())
        && !c.starts_with('/')
        && !c.ends_with('/')
}

fn unpack28(n: u32, hashes: &CallHashes) -> Option<String> {
    if n < NTOKENS {
        return Some(match n {
            0 => "DE".into(),
            1 => "QRZ".into(),
            2 => "CQ".into(),
            3..=1002 => format!("CQ {:03}", n - 3),
            1003..=532_443 => {
                let mut m = n - 1003;
                let mut s = Vec::new();
                while m > 0 {
                    s.push(A4.as_bytes()[(m % 27) as usize]);
                    m /= 27;
                }
                s.reverse();
                let s = String::from_utf8(s).ok()?;
                let s = s.trim();
                if s.is_empty() || s.contains(' ') {
                    return None;
                }
                format!("CQ {s}")
            }
            _ => return None,
        });
    }
    let n = n - NTOKENS;
    if n < MAX22 {
        return Some(match hashes.get22(n) {
            Some(c) => format!("<{c}>"),
            None => "<...>".into(),
        });
    }
    let v = n - MAX22;
    if v >= 37 * 36 * 10 * 27 * 27 * 27 {
        return None;
    }
    let s = std_call_text(v);
    (s.len() >= 3).then_some(s)
}

/// A 4-character locator as a number below 32400, if it is one (not RR73).
fn grid4_value(g: &str) -> Option<u32> {
    let b = g.as_bytes();
    if b.len() != 4 || g == "RR73" {
        return None;
    }
    if !(b'A'..=b'R').contains(&b[0])
        || !(b'A'..=b'R').contains(&b[1])
        || !b[2].is_ascii_digit()
        || !b[3].is_ascii_digit()
    {
        return None;
    }
    let (j1, j2, j3, j4) = (
        (b[0] - b'A') as u32,
        (b[1] - b'A') as u32,
        (b[2] - b'0') as u32,
        (b[3] - b'0') as u32,
    );
    Some(j1 * 18 * 10 * 10 + j2 * 10 * 10 + j3 * 10 + j4)
}

fn grid4_text(v: u32) -> String {
    let (j1, r) = (v / 1800, v % 1800);
    let (j2, r) = (r / 100, r % 100);
    let (j3, j4) = (r / 10, r % 10);
    format!(
        "{}{}{}{}",
        (b'A' + j1 as u8) as char,
        (b'A' + j2 as u8) as char,
        j3,
        j4
    )
}

/// Whether a text is a 4-character Maidenhead locator.
pub fn is_grid4(g: &str) -> bool {
    grid4_value(g).is_some()
}

/// Whether a text is a 6-character Maidenhead locator.
pub fn is_grid6(g: &str) -> bool {
    let b = g.as_bytes();
    b.len() == 6
        && is_grid4(&g[..4])
        && (b'A'..=b'X').contains(&b[4].to_ascii_uppercase())
        && (b'A'..=b'X').contains(&b[5].to_ascii_uppercase())
}

/// Whether a word of a decoded message is a call: 3 to 13 characters of A-Z, 0-9 and `/` with at least one digit and one
/// letter, not a locator (4 or 6 characters, the first two A-R, the next two 0-9) and not `RR73`.
pub fn is_call_word(w: &str) -> bool {
    let b = w.as_bytes();
    if !(3..=13).contains(&b.len())
        || !b
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'/')
        || !b.iter().any(u8::is_ascii_digit)
        || !b.iter().any(u8::is_ascii_uppercase)
        || w == "RR73"
    {
        return false;
    }
    let locator = matches!(b.len(), 4 | 6)
        && b[..2].iter().all(|c| (b'A'..=b'R').contains(c))
        && b[2..4].iter().all(u8::is_ascii_digit);
    !locator
}

fn parse_report(s: &str) -> Option<i32> {
    let (sign, digits) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => return None,
    };
    if digits.len() != 2 || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(sign * digits.parse::<i32>().ok()?)
}

/// The 15-bit field of a report (-50..+49; -50..-31 wrap above +49).
fn report_value(r: i32) -> u32 {
    let r = if r <= -31 { r + 101 } else { r };
    (MAXGRID4 as i32 + 35 + r) as u32
}

fn report_text(r: i32) -> String {
    if r >= 0 {
        format!("+{r:02}")
    } else {
        format!("-{:02}", -r)
    }
}

/// The 77 message bits (0 or 1) of `text`, or why it cannot be packed.
pub fn pack77(text: &str) -> Result<Vec<u8>, PackError> {
    let text = normalise(text);
    if text.is_empty() {
        return err(PackError::Empty);
    }
    // Telemetry: 18 hex digits, the first 0..7.
    if text.len() <= 18
        && !text.is_empty()
        && !text.contains(' ')
        && text
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_lowercase())
        && text.len() == 18
    {
        let first = text.as_bytes()[0];
        if first <= b'7' {
            let mut bits = Vec::with_capacity(77);
            let v = u128::from_str_radix(&text, 16).map_err(|_| PackError::Characters)?;
            for i in (0..71).rev() {
                bits.push(((v >> i) & 1) as u8);
            }
            put_bits(&mut bits, 5, 3);
            put_bits(&mut bits, 0, 3);
            return Ok(bits);
        }
    }
    if let Some(b) = pack_standard(&text)? {
        return Ok(b);
    }
    if let Some(b) = pack_nonstandard(&text)? {
        return Ok(b);
    }
    pack_free(&text)
}

fn normalise(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_uppercase()
}

fn pack_free(text: &str) -> Result<Vec<u8>, PackError> {
    if text.len() > 13 {
        return err(PackError::TooLong);
    }
    if !text.chars().all(|c| A42.contains(c)) {
        return err(PackError::Characters);
    }
    let padded = format!("{text:>13}");
    let mut v: u128 = 0;
    for c in padded.chars() {
        let i = A42
            .find(c)
            .expect("every character was checked against the free-text alphabet");
        v = v * 42 + i as u128;
    }
    let mut bits = Vec::with_capacity(77);
    for i in (0..71).rev() {
        bits.push(((v >> i) & 1) as u8);
    }
    put_bits(&mut bits, 0, 3);
    put_bits(&mut bits, 0, 3);
    Ok(bits)
}

/// Types 1 and 2: two calls (or a CQ form), an optional R, and a locator, report or RRR / RR73 / 73.
fn pack_standard(text: &str) -> Result<Option<Vec<u8>>, PackError> {
    let mut w: Vec<String> = text.split(' ').map(|s| s.to_string()).collect();
    // Join the CQ modifier into one token.
    if w.len() >= 3 && w[0] == "CQ" && w[1] != "DE" {
        let m = &w[1];
        let is_mod = (m.len() == 3 && m.bytes().all(|c| c.is_ascii_digit()))
            || ((1..=4).contains(&m.len())
                && m.bytes().all(|c| c.is_ascii_uppercase())
                && !(w.len() == 3
                    && is_grid4(&w[2])
                    && m.len() >= 3
                    && std_call_value(m).is_some()));
        if is_mod && w.len() >= 3 {
            let joined = format!("CQ {}", w[1]);
            w.splice(0..2, [joined]);
        }
    }
    if w.len() < 2 || w.len() > 4 {
        return Ok(None);
    }
    let first = &w[0];
    let (c1, r1, p1) = split_flag(first);
    let (c2, r2, p2) = split_flag(&w[1]);
    if (r1 || r2) && (p1 || p2) {
        return Ok(None);
    }
    let i3: u64 = if p1 || p2 { 2 } else { 1 };
    // The second call must be standard (or a hashed call in angle brackets for the first only).
    let is_tok = |s: &str| s == "CQ" || s == "QRZ" || s == "DE" || s.starts_with("CQ ");
    let first_ok =
        is_tok(&c1) || std_call_value(&c1).is_some() || (c1.starts_with('<') && c1.ends_with('>'));
    let second_ok = std_call_value(&c2).is_some()
        || (c2.starts_with('<') && c2.ends_with('>') && !c1.starts_with('<'));
    if !first_ok || !second_ok {
        return Ok(None);
    }
    if c2.starts_with('<') && is_tok(&c1) {
        return Ok(None);
    }
    let rest = &w[2..];
    let (ir, igrid) = match rest {
        [] => (0u64, MAXGRID4 + 1),
        [g] if g == "RRR" => (0, MAXGRID4 + 2),
        [g] if g == "RR73" => (0, MAXGRID4 + 3),
        [g] if g == "73" => (0, MAXGRID4 + 4),
        [g] => {
            if let Some(v) = grid4_value(g) {
                (0, v)
            } else {
                let (ir, report) = match (parse_report(g), g.strip_prefix('R')) {
                    (Some(r), _) => (0, Some(r)),
                    (None, Some(tail)) => (1, parse_report(tail)),
                    (None, None) => (0, None),
                };
                match report {
                    Some(r) if (-50..=49).contains(&r) => (ir, report_value(r)),
                    _ => return Ok(None),
                }
            }
        }
        [r, g] if r == "R" => match grid4_value(g) {
            Some(v) => (1, v),
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    if is_tok(&c1) && ir == 1 {
        return Ok(None);
    }
    let n1 = pack28(&c1)?;
    let n2 = pack28(&c2)?;
    let mut bits = Vec::with_capacity(77);
    put_bits(&mut bits, n1 as u64, 28);
    put_bits(&mut bits, (r1 || p1) as u64, 1);
    put_bits(&mut bits, n2 as u64, 28);
    put_bits(&mut bits, (r2 || p2) as u64, 1);
    put_bits(&mut bits, ir, 1);
    put_bits(&mut bits, igrid as u64, 15);
    put_bits(&mut bits, i3, 3);
    Ok(Some(bits))
}

fn split_flag(s: &str) -> (String, bool, bool) {
    if let Some(c) = s.strip_suffix("/R") {
        if std_call_value(c).is_some() {
            return (c.to_string(), true, false);
        }
    }
    if let Some(c) = s.strip_suffix("/P") {
        if std_call_value(c).is_some() {
            return (c.to_string(), false, true);
        }
    }
    (s.to_string(), false, false)
}

/// Type 4: one nonstandard call (up to 11 characters) and one hashed call, or CQ with the nonstandard call.
fn pack_nonstandard(text: &str) -> Result<Option<Vec<u8>>, PackError> {
    let w: Vec<&str> = text.split(' ').collect();
    let (icq, c1, c2, rpt) = match w.as_slice() {
        ["CQ", c] => (1u64, "", *c, ""),
        [a, b] => (0, *a, *b, ""),
        [a, b, r] if ["RRR", "RR73", "73"].contains(r) => (0, *a, *b, *r),
        _ => return Ok(None),
    };
    let nrpt = match rpt {
        "" => 0,
        "RRR" => 1,
        "RR73" => 2,
        _ => 3,
    };
    let (hashed, full, iflip) = if icq == 1 {
        ("", c2, 0u64)
    } else if c1.starts_with('<') && c1.ends_with('>') {
        (c1.trim_matches(|c| c == '<' || c == '>'), c2, 0)
    } else if c2.starts_with('<') && c2.ends_with('>') {
        (c2.trim_matches(|c| c == '<' || c == '>'), c1, 1)
    } else {
        return Ok(None);
    };
    if !is_plausible_call(full) || full.len() > 11 {
        return Ok(None);
    }
    if icq == 1 && std_call_value(full).is_some() {
        return Ok(None);
    }
    let mut n58: u64 = 0;
    for c in format!("{full:>11}").chars() {
        let i = A38
            .find(c)
            .expect("a plausible call has only characters of the call alphabet");
        n58 = n58 * 38 + i as u64;
    }
    let n12 = if hashed.is_empty() {
        0
    } else {
        hash12(hashed) as u64
    };
    let mut bits = Vec::with_capacity(77);
    put_bits(&mut bits, n12, 12);
    put_bits(&mut bits, n58, 58);
    put_bits(&mut bits, iflip, 1);
    put_bits(&mut bits, nrpt, 2);
    put_bits(&mut bits, icq, 1);
    put_bits(&mut bits, 4, 3);
    Ok(Some(bits))
}

/// What unpacking found besides the text: calls to learn.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unpacked {
    /// The message text.
    pub text: String,
    /// The message type `i3` (and `n3` for type 0).
    pub i3: u8,
    /// `n3` of type 0.
    pub n3: u8,
    /// Calls in the message (full, not hashed), for the hash table.
    pub calls: Vec<String>,
    /// A hashed call the table did not know.
    pub unknown_hash: bool,
}

/// Unpacks 77 bits. `None` when the bits are not a valid message.
pub fn unpack77(bits: &[u8], hashes: &CallHashes) -> Option<Unpacked> {
    if bits.len() < 77 {
        return None;
    }
    let i3 = get_bits(&bits[74..77]) as u8;
    let n3 = get_bits(&bits[71..74]) as u8;
    let mut u = Unpacked {
        i3,
        n3,
        ..Default::default()
    };
    let text = match i3 {
        0 => match n3 {
            0 => {
                let mut v: u128 = 0;
                for &b in &bits[..71] {
                    v = (v << 1) | b as u128;
                }
                let mut s = [b' '; 13];
                for i in (0..13).rev() {
                    s[i] = A42.as_bytes()[(v % 42) as usize];
                    v /= 42;
                }
                if v != 0 {
                    return None;
                }
                let t = String::from_utf8_lossy(&s).trim().to_string();
                if t.is_empty() {
                    return None;
                }
                t
            }
            1 => {
                let n1 = get_bits(&bits[0..28]) as u32;
                let n2 = get_bits(&bits[28..56]) as u32;
                let h10 = get_bits(&bits[56..66]) as u32;
                let r5 = get_bits(&bits[66..71]) as i32;
                let c1 = unpack28(n1, hashes)?;
                let c2 = unpack28(n2, hashes)?;
                if n1 < NTOKENS + MAX22 || n2 < NTOKENS + MAX22 {
                    return None;
                }
                let c3 = match hashes.get10(h10) {
                    Some(c) => format!("<{c}>"),
                    None => {
                        u.unknown_hash = true;
                        "<...>".into()
                    }
                };
                u.calls.push(c1.clone());
                u.calls.push(c2.clone());
                format!("{c1} RR73; {c2} {c3} {}", report_text(2 * r5 - 30))
            }
            3 | 4 => {
                let n1 = get_bits(&bits[0..28]) as u32;
                let n2 = get_bits(&bits[28..56]) as u32;
                let ir = bits[56];
                let intx = get_bits(&bits[57..61]) as u32;
                let nclass = get_bits(&bits[61..64]) as u32;
                let isec = get_bits(&bits[64..71]) as usize;
                if isec == 0 || isec > ARRL_SECTIONS.len() {
                    return None;
                }
                let c1 = unpack28(n1, hashes)?;
                let c2 = unpack28(n2, hashes)?;
                if n1 < NTOKENS + MAX22 || n2 < NTOKENS + MAX22 {
                    return None;
                }
                let ntx = intx + 1 + if n3 == 4 { 16 } else { 0 };
                u.calls.push(c1.clone());
                u.calls.push(c2.clone());
                let r = if ir == 1 { "R " } else { "" };
                format!(
                    "{c1} {c2} {r}{ntx}{} {}",
                    (b'A' + nclass as u8) as char,
                    ARRL_SECTIONS[isec - 1]
                )
            }
            5 => {
                let mut v: u128 = 0;
                for &b in &bits[..71] {
                    v = (v << 1) | b as u128;
                }
                format!("{v:018X}")
            }
            _ => return None,
        },
        1 | 2 => {
            let n1 = get_bits(&bits[0..28]) as u32;
            let f1 = bits[28];
            let n2 = get_bits(&bits[29..57]) as u32;
            let f2 = bits[57];
            let ir = bits[58];
            let ig = get_bits(&bits[59..74]) as u32;
            let mut c1 = unpack28(n1, hashes)?;
            let mut c2 = unpack28(n2, hashes)?;
            if n2 < NTOKENS {
                return None;
            }
            if n1 >= NTOKENS + MAX22 {
                u.calls.push(c1.clone());
            }
            if n2 >= NTOKENS + MAX22 {
                u.calls.push(c2.clone());
            }
            if c1 == "<...>" || c2 == "<...>" {
                u.unknown_hash = true;
            }
            let flag = if i3 == 1 { "/R" } else { "/P" };
            if f1 == 1 {
                if n1 < NTOKENS {
                    return None;
                }
                c1.push_str(flag);
            }
            if f2 == 1 {
                c2.push_str(flag);
            }
            let tail = if ig < MAXGRID4 {
                let g = grid4_text(ig);
                if ir == 1 {
                    format!(" R {g}")
                } else {
                    format!(" {g}")
                }
            } else {
                let irpt = ig - MAXGRID4;
                match irpt {
                    1 => String::new(),
                    2 => " RRR".into(),
                    3 => " RR73".into(),
                    4 => " 73".into(),
                    5.. => {
                        let mut r = irpt as i32 - 35;
                        if r > 50 {
                            r -= 101;
                        }
                        if !(-50..=49).contains(&r) {
                            return None;
                        }
                        if ir == 1 {
                            format!(" R{}", report_text(r))
                        } else {
                            format!(" {}", report_text(r))
                        }
                    }
                    _ => return None,
                }
            };
            if ir == 1 && n1 < NTOKENS {
                return None;
            }
            format!("{c1} {c2}{tail}")
        }
        3 => {
            let itu = bits[0];
            let n1 = get_bits(&bits[1..29]) as u32;
            let n2 = get_bits(&bits[29..57]) as u32;
            let ir = bits[57];
            let irst = get_bits(&bits[58..61]) as u32;
            let nexch = get_bits(&bits[61..74]) as u32;
            let c1 = unpack28(n1, hashes)?;
            let c2 = unpack28(n2, hashes)?;
            if n1 < NTOKENS || n2 < NTOKENS + MAX22 {
                return None;
            }
            u.calls.push(c2.clone());
            let rst = 529 + 10 * irst;
            let exch = if nexch <= 7999 {
                format!("{nexch:04}")
            } else {
                rtty_mult((nexch - 8001) as usize)?
            };
            let tu = if itu == 1 { "TU; " } else { "" };
            let r = if ir == 1 { "R " } else { "" };
            format!("{tu}{c1} {c2} {r}{rst} {exch}")
        }
        4 => {
            let n12 = get_bits(&bits[0..12]) as u32;
            let n58 = get_bits(&bits[12..70]);
            let iflip = bits[70];
            let nrpt = get_bits(&bits[71..73]);
            let icq = bits[73];
            let mut m = n58;
            let mut s = [b' '; 11];
            for i in (0..11).rev() {
                s[i] = A38.as_bytes()[(m % 38) as usize];
                m /= 38;
            }
            let full = String::from_utf8_lossy(&s).trim().to_string();
            if full.len() < 3 || full.contains(' ') {
                return None;
            }
            u.calls.push(full.clone());
            if icq == 1 {
                format!("CQ {full}")
            } else {
                let hashed = match hashes.get12(n12) {
                    Some(c) => format!("<{c}>"),
                    None => {
                        u.unknown_hash = true;
                        "<...>".into()
                    }
                };
                let (a, b) = if iflip == 0 {
                    (hashed, full)
                } else {
                    (full, hashed)
                };
                let r = match nrpt {
                    1 => " RRR",
                    2 => " RR73",
                    3 => " 73",
                    _ => "",
                };
                format!("{a} {b}{r}")
            }
        }
        5 => {
            let h12 = get_bits(&bits[0..12]) as u32;
            let h22 = get_bits(&bits[12..34]) as u32;
            let ir = bits[34];
            let irpt = get_bits(&bits[35..38]) as u32;
            let iserial = get_bits(&bits[38..49]) as u32;
            let ig6 = get_bits(&bits[49..74]) as u32;
            let c1 = match hashes.get12(h12) {
                Some(c) => format!("<{c}>"),
                None => {
                    u.unknown_hash = true;
                    "<...>".into()
                }
            };
            let c2 = match hashes.get22(h22) {
                Some(c) => format!("<{c}>"),
                None => {
                    u.unknown_hash = true;
                    "<...>".into()
                }
            };
            let g = grid6_text(ig6)?;
            let r = if ir == 1 { "R " } else { "" };
            format!("{c1} {c2} {r}{}{:04} {g}", 52 + irpt, iserial)
        }
        _ => return None,
    };
    u.text = text;
    Some(u)
}

fn grid6_text(v: u32) -> Option<String> {
    let (j1, r) = (v / (18 * 10 * 10 * 24 * 24), v % (18 * 10 * 10 * 24 * 24));
    let (j2, r) = (r / (10 * 10 * 24 * 24), r % (10 * 10 * 24 * 24));
    let (j3, r) = (r / (10 * 24 * 24), r % (10 * 24 * 24));
    let (j4, r) = (r / (24 * 24), r % (24 * 24));
    let (j5, j6) = (r / 24, r % 24);
    if j1 > 17 {
        return None;
    }
    Some(format!(
        "{}{}{}{}{}{}",
        (b'A' + j1 as u8) as char,
        (b'A' + j2 as u8) as char,
        j3,
        j4,
        (b'A' + j5 as u8) as char,
        (b'A' + j6 as u8) as char
    ))
}

/// Unpacks and learns the full calls into `hashes`.
pub fn unpack77_learn(bits: &[u8], hashes: &mut CallHashes) -> Option<Unpacked> {
    let u = unpack77(bits, hashes)?;
    for c in &u.calls {
        let c = c.trim_end_matches("/R").trim_end_matches("/P");
        if !c.starts_with('<') && !c.starts_with("CQ") && c != "QRZ" && c != "DE" {
            hashes.learn(c);
        }
    }
    Some(u)
}

/// The FT4 / FT2 / FST4 scrambling vector (QEX 2020, "The FT4 and FT8 Communication Protocols").
pub const RVEC: [u8; 77] = [
    0, 1, 0, 0, 1, 0, 1, 0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 0, 0,
    1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1,
    1, 0, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 1,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn hex80(bits: &[u8]) -> String {
        let mut b = bits.to_vec();
        b.resize(80, 0);
        b.chunks(4).map(|c| format!("{:x}", get_bits(c))).collect()
    }

    #[test]
    fn round_trips_of_the_spec() {
        let mut h = CallHashes::default();
        for t in [
            "CQ K1ABC FN42",
            "K1ABC W9XYZ -12",
            "W9XYZ K1ABC R-05",
            "K1ABC W9XYZ RR73",
            "K1ABC W9XYZ 73",
            "K1ABC W9XYZ RRR",
            "CQ DX K1ABC FN42",
            "CQ 123 K1ABC FN42",
            "K1ABC/R W9XYZ EN37",
            "K1ABC W9XYZ/P R JO62",
            "QRZ W9XYZ EN37",
            "K1ABC W9XYZ +07",
            "K1ABC W9XYZ -45",
            "CQ PJ4/K1ABC",
            "TNX BOB 73 GL",
            "7123456789ABCDEF01",
            "DE W9XYZ",
            "CQ 3DA0XYZ KG53",
            "K1ABC W9XYZ R EN37",
            "CQ TEST DL1XYZ JO62",
            "G4ABC DL1XYZ RR73",
        ] {
            let b = pack77(t).unwrap_or_else(|e| panic!("{t}: {e}"));
            assert_eq!(b.len(), 77);
            let u = unpack77_learn(&b, &mut h).unwrap_or_else(|| panic!("{t}: unpack"));
            assert_eq!(u.text, t);
        }
    }

    #[test]
    fn hashed_calls_need_the_table() {
        let b = pack77("<PJ4/K1ABC> W9XYZ RR73").unwrap();
        let mut h = CallHashes::default();
        assert_eq!(unpack77(&b, &h).unwrap().text, "<...> W9XYZ RR73");
        h.learn("PJ4/K1ABC");
        assert_eq!(unpack77(&b, &h).unwrap().text, "<PJ4/K1ABC> W9XYZ RR73");
        assert_eq!(hex80(&b), "0352b0a06149dc1fa4c8");
    }

    #[test]
    fn refuses_what_cannot_be_packed() {
        assert!(pack77("K1ABC/ABCD W9XYZ").is_err());
        assert!(pack77("THIS IS TWENTY CHARS").is_err());
    }

    #[test]
    fn table_is_capped() {
        let mut h = CallHashes::default();
        for i in 0..5000 {
            h.learn(
                &format!("K{}A{}", i % 10, (b'A' + (i / 10 % 26) as u8) as char)
                    .replace('A', &format!("{}", (b'A' + (i / 260 % 26) as u8) as char)),
            );
        }
        assert!(h.len() <= CallHashes::CAP);
    }
}
