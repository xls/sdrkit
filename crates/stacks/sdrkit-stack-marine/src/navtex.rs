//! NAVTEX: the SITOR-B receiver (bit-phase lock and slips in front of the shared FEC mode B decoder), the message framing, the
//! message list, and the words of a test transmission.
//!
//! Standard: ITU-R M.625-4 section 4, ITU-R M.540-2, IMO NAVTEX Manual

use sdrkit_util_objects::{LogRing, Shift, Stamp};
use sdrkit_util_text::ccir476::{self, Fec, FecBDecoder, FecBEncoder, Word};
use sdrkit_util_text::ita2;
use serde::{Deserialize, Serialize};

/// Words kept per bit phase.
const HISTORY: usize = 32;
/// Places held back before the FEC decoder, so a slip found later can still rewrite them.
const DELAY: usize = 24;
/// Places looked at for the lost-signal rule.
const RECENT: usize = 24;
/// Valid words in a row a neighbouring phase needs to be taken as a bit slip when the locked phase fails.
const SLIP_RUN: usize = 6;
/// Bits without a lock after which a message in progress ends (15 s at 100 Bd).
const IDLE_BITS: u64 = 1500;

/// One received word with its soft units (positive means B, bit position 1 first).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoftWord {
    /// The hard word (B = 0, Y = 1).
    pub word: Word,
    /// The soft units.
    pub soft: [f32; 7],
}

impl SoftWord {
    /// The word of seven soft units (positive means B).
    pub fn new(soft: [f32; 7]) -> SoftWord {
        let mut v = 0u8;
        for (i, &s) in soft.iter().enumerate() {
            if s < 0.0 {
                v |= 1 << i;
            }
        }
        SoftWord {
            word: Word(v),
            soft,
        }
    }
}

/// Whether a word is alpha, beta or the repetition signal (ITU-R M.625-4 Table 2).
pub fn service(w: Word) -> bool {
    w == ccir476::ALPHA || w == ccir476::BETA || w == ccir476::RQ
}

/// The last words of one bit phase and the run of valid words that ends with the newest.
#[derive(Clone, Copy, Debug)]
struct Phase {
    words: [SoftWord; HISTORY],
    len: usize,
    head: usize,
    run: usize,
}

impl Default for Phase {
    fn default() -> Self {
        Phase {
            words: [SoftWord::default(); HISTORY],
            len: 0,
            head: 0,
            run: 0,
        }
    }
}

impl Phase {
    fn push(&mut self, w: SoftWord) {
        self.words[(self.head + self.len) % HISTORY] = w;
        if self.len < HISTORY {
            self.len += 1;
        } else {
            self.head = (self.head + 1) % HISTORY;
        }
        self.run = if w.word.is_valid() { self.run + 1 } else { 0 };
    }

    /// Word `i`, oldest first.
    fn get(&self, i: usize) -> SoftWord {
        self.words[(self.head + i) % HISTORY]
    }

    /// Which places of the history (0 even, 1 odd) look like DX places: those that hold the repetition signal (sent in DX places
    /// only while phasing), or a character repeated five places later.
    fn dx_parity(&self) -> usize {
        let mut ev = [0usize; 2];
        for i in 0..self.len {
            let w = self.get(i).word;
            let repeated =
                i + 5 < self.len && w.is_valid() && !service(w) && w == self.get(i + 5).word;
            if w == ccir476::RQ || repeated {
                ev[i % 2] += 1;
            }
        }
        usize::from(ev[1] > ev[0])
    }

    /// Whether the phase is good to lock on, and its score.
    fn good(&self) -> (bool, usize) {
        let n = self.len;
        let svc = (n.saturating_sub(16)..n)
            .filter(|&i| service(self.get(i).word))
            .count();
        let (mut matches, mut checked) = (0, 0);
        for i in (0..n.saturating_sub(5)).rev() {
            if checked >= 20 {
                break;
            }
            let w = self.get(i).word;
            if !w.is_valid() || service(w) {
                continue;
            }
            checked += 1;
            if w == self.get(i + 5).word {
                matches += 1;
            }
        }
        let ok = self.run >= 16 && (svc >= 12 || matches >= 6);
        (ok, self.run + 4 * svc + 4 * matches)
    }
}

/// A character the receiver outputs: the character, or `*` with `error` set for one that both copies lost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavtexChar {
    /// The character (`\n` for a line feed, `\r` for a carriage return).
    pub ch: char,
    /// Both copies were mutilated.
    pub error: bool,
}

/// The SITOR-B receiver of one polarity: finds the bit phase, locks, follows bit slips, and feeds whole words to the shared FEC
/// mode B decoder (`sdrkit_util_text::ccir476::FecBDecoder`, with soft repair) after a short delay line.
pub struct SitorDecoder {
    bits: [f32; 7],
    count: u64,
    phases: [Phase; 7],
    locked: Option<usize>,
    /// Places not yet given to the FEC decoder, oldest first.
    delay: [SoftWord; DELAY],
    delay_len: usize,
    delay_head: usize,
    recent: [bool; RECENT],
    recent_len: usize,
    recent_pos: usize,
    unlocked_bits: u64,
    invert: bool,
    fec: FecBDecoder,
    /// The characters accepted so far, oldest first. The caller drains it.
    pub chars: Vec<NavtexChar>,
    /// The lock was lost (the pending characters were flushed) since the caller last cleared it.
    pub lost: bool,
    /// No lock for 15 s: a message in progress has ended.
    pub idle_timeout: bool,
}

fn fec_decoder() -> FecBDecoder {
    let mut f = FecBDecoder::new();
    f.repair = true;
    f
}

impl SitorDecoder {
    /// A receiver. `invert` swaps the tones.
    pub fn new(invert: bool) -> Self {
        SitorDecoder {
            bits: [0.0; 7],
            count: 0,
            phases: [Phase::default(); 7],
            locked: None,
            delay: [SoftWord::default(); DELAY],
            delay_len: 0,
            delay_head: 0,
            recent: [false; RECENT],
            recent_len: 0,
            recent_pos: 0,
            unlocked_bits: 0,
            invert,
            fec: fec_decoder(),
            chars: Vec::with_capacity(64),
            lost: false,
            idle_timeout: false,
        }
    }

    /// Whether a bit phase is locked.
    pub fn locked(&self) -> bool {
        self.locked.is_some()
    }

    /// Starts again (the characters not yet drained stay).
    pub fn reset(&mut self) {
        let chars = std::mem::take(&mut self.chars);
        *self = SitorDecoder::new(self.invert);
        self.chars = chars;
    }

    /// Feeds soft bits (positive for the higher tone, B).
    pub fn push(&mut self, soft: &[f32]) {
        for &s in soft {
            let s = if self.invert { -s } else { s };
            self.bits.copy_within(1.., 0);
            self.bits[6] = if s.is_finite() { s } else { 0.0 };
            self.count += 1;
            if self.count < 7 {
                continue;
            }
            let ph = (self.count % 7) as usize;
            let w = SoftWord::new(self.bits);
            self.phases[ph].push(w);
            match self.locked {
                None => {
                    self.unlocked_bits += 1;
                    if self.unlocked_bits > IDLE_BITS {
                        self.idle_timeout = true;
                    }
                    if self.phases[ph].good().0 {
                        let mut best = (ph, 0usize);
                        for (k, q) in self.phases.iter().enumerate() {
                            let (g, sc) = q.good();
                            if g && sc > best.1 {
                                best = (k, sc);
                            }
                        }
                        self.lock(best.0);
                    }
                }
                Some(lp) if ph == lp => self.place(w),
                Some(lp) => {
                    // A bit slip: another phase suddenly produces a long run while the locked one does not.
                    if self.phases[ph].run >= 24 && self.phases[lp].run < 4 {
                        self.slip(ph, lp);
                    }
                }
            }
        }
    }

    fn lock(&mut self, ph: usize) {
        self.locked = Some(ph);
        self.unlocked_bits = 0;
        self.idle_timeout = false;
        self.delay_len = 0;
        self.delay_head = 0;
        self.recent_len = 0;
        self.recent_pos = 0;
        self.fec.reset();
        // The FEC decoder takes its first place as a DX place. When the history says the DX places are the odd ones, one place
        // of no information goes first.
        let p = self.phases[ph];
        if p.dx_parity() == 1 {
            let r = self.fec.push(Word(0));
            self.take(r);
        }
        // Replay the words this phase holds, with their own soft units.
        for i in 0..p.len {
            self.place(p.get(i));
        }
    }

    /// Moves to phase `ph` after a bit slip. The places since the slip take the words of the new phase, with their own soft
    /// units. The newest word of the new phase belongs to the
    /// place whose old word ended within half a word of it: the newest place, the one before it (the newest is then dropped, the
    /// next word of the new phase brings it), or a place not yet held.
    fn slip(&mut self, ph: usize, lp: usize) {
        self.locked = Some(ph);
        let p = self.phases[ph];
        let c = self.count as i64;
        let ended = |phase: usize| c - (c - phase as i64).rem_euclid(7);
        let mut d = lp as i64 - ph as i64;
        if d > 3 {
            d -= 7;
        } else if d < -3 {
            d += 7;
        }
        // Places between the newest one held and the one the new phase's newest word belongs to.
        let m = (ended(lp) - (ended(ph) + d)).div_euclid(7);
        if m > 0 && self.delay_len > 0 {
            self.delay_len -= 1;
        }
        // The slip point: the split of the places held where the old words before it and the new words after it are valid most
        // often (before a slip the new phase's words are misaligned, after it the old phase's are).
        let newest = usize::from(m < 0);
        let held = self.delay_len;
        let n = held.min(p.len.saturating_sub(newest));
        let first = held - n;
        let new_word = |k: usize| p.get(p.len - newest - (held - k));
        let (mut best, mut start) = (0usize, first);
        let mut score: usize = (first..self.delay_len)
            .filter(|&k| new_word(k).word.is_valid())
            .count();
        for k in first..=self.delay_len {
            if score > best || (score == best && k == first) {
                best = score;
                start = k;
            }
            if k < self.delay_len {
                score += usize::from(self.delay_word(k).word.is_valid());
                score -= usize::from(new_word(k).word.is_valid());
            }
        }
        for k in start..self.delay_len {
            let w = new_word(k);
            self.delay[(self.delay_head + k) % DELAY] = w;
        }
        self.recent = [false; RECENT];
        if newest == 1 && p.len > 0 {
            self.place(p.get(p.len - 1));
        }
    }

    fn delay_word(&self, i: usize) -> SoftWord {
        self.delay[(self.delay_head + i) % DELAY]
    }

    fn place(&mut self, w: SoftWord) {
        if self.delay_len == DELAY {
            let old = self.delay[self.delay_head];
            self.delay_head = (self.delay_head + 1) % DELAY;
            self.delay_len -= 1;
            self.decode(&old.soft);
        }
        self.delay[(self.delay_head + self.delay_len) % DELAY] = w;
        self.delay_len += 1;
        self.recent[self.recent_pos] = !w.word.is_valid();
        self.recent_pos = (self.recent_pos + 1) % RECENT;
        self.recent_len = (self.recent_len + 1).min(RECENT);
        // Lost signal: 14 of the last 24 places invalid. A neighbouring phase with a run of valid words is a bit slip instead.
        if self.recent_len == RECENT && self.recent.iter().filter(|x| **x).count() >= 14 {
            if let Some(lp) = self.locked {
                let next = [(lp + 1) % 7, (lp + 6) % 7]
                    .into_iter()
                    .filter(|&q| self.phases[q].run >= SLIP_RUN)
                    .max_by_key(|&q| self.phases[q].run);
                if let Some(q) = next {
                    self.slip(q, lp);
                    return;
                }
            }
            self.flush();
            self.locked = None;
            self.lost = true;
            for p in &mut self.phases {
                p.run = 0;
            }
        }
    }

    fn decode(&mut self, soft: &[f32; 7]) {
        let r = self.fec.push_soft(soft);
        self.take(r);
    }

    fn take(&mut self, r: Option<Fec>) {
        match r {
            Some(Fec::Char(c)) => {
                // The figures case holds WRU and the bell, which are not printed.
                if !c.is_control() || c == '\n' || c == '\r' {
                    self.chars.push(NavtexChar {
                        ch: c,
                        error: false,
                    });
                }
            }
            Some(Fec::Mutilated) => self.chars.push(NavtexChar {
                ch: '*',
                error: true,
            }),
            Some(Fec::Idle) | None => {}
        }
    }

    /// Returns the places still held to the FEC decoder, then five places of no information so that the last DX places are
    /// decoded, and starts the FEC decoder again.
    fn flush(&mut self) {
        while self.delay_len > 0 {
            let w = self.delay[self.delay_head];
            self.delay_head = (self.delay_head + 1) % DELAY;
            self.delay_len -= 1;
            self.decode(&w.soft);
        }
        for _ in 0..5 {
            let r = self.fec.push(Word(0));
            self.take(r);
        }
        self.fec.reset();
        self.recent_len = 0;
    }
}

/// A NAVTEX message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NavtexMessage {
    /// B1 B2 B3 B4 as received (`*` where unreadable).
    pub header: String,
    /// B1, the transmitter identity (`?` when damaged).
    pub station: char,
    /// B2, the subject indicator.
    pub subject: char,
    /// B3 B4, the serial number (-1 when damaged).
    pub number: i32,
    /// The name of the subject.
    pub subject_name: String,
    /// The text, lines separated by `\n`, `*` where unreadable.
    pub text: String,
    /// Characters of the text.
    pub chars: usize,
    /// Unreadable characters of the header and the text.
    pub errors: usize,
    /// The character error rate: errors / (characters + 4).
    pub cer: f64,
    /// NNNN was seen.
    pub complete: bool,
    /// Times the same message came again.
    pub repeats: u32,
    /// When it ended.
    pub at: Stamp,
    /// The text was cut to 160 characters to keep the list small.
    pub shortened: bool,
    /// The dial, when known.
    pub freq_hz: Option<f64>,
}

impl Shift for NavtexMessage {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// The name of a subject indicator (IMO NAVTEX Manual).
pub fn subject_name(b2: char) -> &'static str {
    match b2 {
        'A' => "Navigational warning",
        'B' => "Meteorological warning",
        'C' => "Ice report",
        'D' => "Search and rescue information, piracy warning",
        'E' => "Meteorological forecast",
        'F' => "Pilot service message",
        'G' => "AIS message",
        'H' => "LORAN message",
        'I' => "Spare",
        'J' => "Satellite navigation system message",
        'K' => "Other electronic navaid message",
        'L' => "Navigational warning (additional to A)",
        'V' | 'W' | 'X' | 'Y' => "Special service",
        'Z' => "No messages on hand",
        _ => "Unknown",
    }
}

/// The message framer: finds ZCZC, the header and NNNN (ITU-R M.540-2).
#[derive(Clone, Debug, Default)]
pub struct Framer {
    recent: [char; 4],
    in_msg: bool,
    header: String,
    header_done: bool,
    skip_space: bool,
    body: String,
    header_errors: usize,
}

fn matches_with_one(tail: &[char; 4], pat: &[char; 4]) -> bool {
    let mut bad = 0;
    for (a, b) in tail.iter().zip(pat) {
        if a != b {
            if *a == '*' {
                bad += 1;
            } else {
                return false;
            }
        }
    }
    bad <= 1
}

const ZCZC: [char; 4] = ['Z', 'C', 'Z', 'C'];
const NNNN: [char; 4] = ['N', 'N', 'N', 'N'];
/// The longest body before a message ends as cut short.
const MAX_BODY: usize = 20_000;

impl Framer {
    /// Whether a message is in progress.
    pub fn in_message(&self) -> bool {
        self.in_msg
    }

    /// Takes a character (`*` for an unreadable one). Returns a finished message.
    pub fn push(&mut self, c: char, error: bool) -> Option<NavtexMessage> {
        if c == '\r' {
            return None;
        }
        self.recent.copy_within(1.., 0);
        self.recent[3] = c;
        let mut done = None;
        if matches_with_one(&self.recent, &ZCZC) && !(self.in_msg && !self.header_done) {
            if self.in_msg {
                // A new ZCZC before NNNN ends the old message as incomplete.
                let n = self.body.chars().count();
                self.body = self.body.chars().take(n.saturating_sub(3)).collect();
                done = Some(self.finish(false));
            }
            self.in_msg = true;
            self.header.clear();
            self.header_done = false;
            self.skip_space = true;
            self.body.clear();
            self.header_errors = 0;
            return done;
        }
        if !self.in_msg {
            return None;
        }
        if !self.header_done {
            if self.skip_space && (c == ' ' || c == '\n') && self.header.is_empty() {
                self.skip_space = false;
                return None;
            }
            self.skip_space = false;
            if c == '\n' || c == ' ' {
                if self.header.chars().count() >= 4 {
                    self.header_done = true;
                }
                return None;
            }
            self.header.push(c);
            if error || c == '*' {
                self.header_errors += 1;
            }
            if self.header.chars().count() >= 4 {
                self.header_done = true;
            }
            return None;
        }
        self.body.push(c);
        if matches_with_one(&self.recent, &NNNN) {
            let n = self.body.chars().count();
            self.body = self.body.chars().take(n.saturating_sub(4)).collect();
            return Some(self.finish(true));
        }
        if self.body.len() > MAX_BODY {
            return Some(self.finish(false));
        }
        None
    }

    /// Ends a message in progress (lost signal, end of the stream) as incomplete.
    pub fn end(&mut self) -> Option<NavtexMessage> {
        if self.in_msg && self.header_done {
            Some(self.finish(false))
        } else {
            self.in_msg = false;
            None
        }
    }

    fn finish(&mut self, complete: bool) -> NavtexMessage {
        self.in_msg = false;
        let h: Vec<char> = self.header.chars().collect();
        let station = h
            .first()
            .copied()
            .filter(|c| c.is_ascii_uppercase())
            .unwrap_or('?');
        let subject = h.get(1).copied().unwrap_or('?');
        let number = match (
            h.get(2).and_then(|c| c.to_digit(10)),
            h.get(3).and_then(|c| c.to_digit(10)),
        ) {
            (Some(a), Some(b)) => (a * 10 + b) as i32,
            _ => -1,
        };
        let lines: Vec<&str> = self.body.lines().map(str::trim).collect();
        let text = lines
            .join("\n")
            .trim_matches(|c: char| c == '\n' || c == ' ')
            .to_string();
        let chars = text.chars().count();
        let errors = text.chars().filter(|c| *c == '*').count() + self.header_errors;
        NavtexMessage {
            header: self.header.clone(),
            station,
            subject,
            number,
            subject_name: subject_name(subject).into(),
            chars,
            errors,
            cer: errors as f64 / (chars + 4) as f64,
            text,
            complete,
            ..Default::default()
        }
    }
}

/// The most messages listed.
pub const LIST_CAP: usize = 100;
/// The text the newest messages keep in full, characters.
const TEXT_BUDGET: usize = 60_000;
/// The length an older message shrinks to, characters.
const SHORT_TEXT: usize = 160;

/// The message list, newest last, with the totals.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageList {
    /// The messages, oldest first.
    pub messages: LogRing<NavtexMessage>,
    /// Complete messages with no unreadable character, since the start.
    pub good: u64,
    /// Other messages since the start.
    pub bad: u64,
}

impl Default for MessageList {
    fn default() -> Self {
        MessageList {
            messages: LogRing::new(LIST_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for MessageList {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
    }
}

impl MessageList {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a message. Returns false when it only repeated a kept one (its repeat count and time move).
    pub fn add(&mut self, m: NavtexMessage) -> bool {
        if let Some(k) = self.messages.iter_mut().find(|k| {
            k.header == m.header
                && k.text == m.text
                && !k.shortened
                && !m.shortened
                && k.complete == m.complete
        }) {
            k.repeats += 1;
            k.at = m.at;
            return false;
        }
        if m.complete && m.errors == 0 {
            self.good += 1;
        } else {
            self.bad += 1;
        }
        self.messages.push(m);
        let mut budget = TEXT_BUDGET;
        for k in self.messages.iter_mut().rev() {
            let n = k.text.chars().count();
            if n <= budget {
                budget -= n;
            } else if !k.shortened {
                k.text = k.text.chars().take(SHORT_TEXT).collect();
                k.shortened = true;
                budget = 0;
            }
        }
        true
    }

    /// Forgets every message and the totals.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.good = 0;
        self.bad = 0;
    }

    /// The copy text of a message: `ZCZC <header>`, the text, "NNNN".
    pub fn copy_text(m: &NavtexMessage) -> String {
        format!("ZCZC {}\n{}\nNNNN", m.header, m.text)
    }
}

/// The demo messages of station A: a navigational warning, a gale warning and a forecast.
pub fn demo_messages() -> Vec<String> {
    vec![
        "ZCZC AA01\n011200 UTC JAN\nNAVAREA I 001\nENGLISH CHANNEL. DOVER STRAIT.\nBUOY 51-05N 001-30E UNLIT.\nNNNN".into(),
        "ZCZC AB02\n011300 UTC JAN\nGALE WARNING\nTHAMES DOVER WIGHT SOUTHWEST GALE 8 IMMINENT.\nNNNN".into(),
        "ZCZC AE03\n011400 UTC JAN\nFORECAST FOR THE NEXT 24 HOURS\nWIND WEST 4 OR 5. SEA MODERATE. VISIBILITY GOOD.\nNNNN".into(),
    ]
}

/// The short test message.
pub fn test_message() -> String {
    "ZCZC AZ99\nTEST\nNNNN".into()
}

/// The words on the air for one message (ZCZC and NNNN added when missing), after `idle_s` of phasing and before idle alpha. Text
/// is upper-cased. A character the ITU alphabet does not have is left out.
pub fn message_words(text: &str, idle_s: f64) -> Vec<Word> {
    let mut t = text.trim().to_ascii_uppercase();
    if !t.starts_with("ZCZC") {
        t = format!("ZCZC {t}");
    }
    if !t.ends_with("NNNN") {
        t.push_str("\nNNNN");
    }
    t.push('\n');
    let t: String = t
        .chars()
        .filter(|&c| c == '\n' || ita2::ITU.code_of(c).is_ok())
        .collect();
    let mut enc = FecBEncoder::new();
    // A DX and RX pair lasts 140 ms.
    enc.phasing_pairs = ((idle_s / 0.14).round() as usize).max(16);
    let mut out = Vec::new();
    // Every character left can be encoded, so the encoder does not fail.
    let _ = enc.message(&t, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn soft_of(words: &[Word]) -> Vec<f32> {
        let mut bits = Vec::with_capacity(words.len() * 7);
        for w in words {
            for b in w.bits() {
                // B (0) is the higher tone: positive.
                bits.push(if b == 0 { 1.0f32 } else { -1.0 });
            }
        }
        bits
    }

    fn receive(bits: &[f32], invert: bool) -> (String, Vec<NavtexMessage>) {
        let mut d = SitorDecoder::new(invert);
        d.push(bits);
        let mut f = Framer::default();
        let mut text = String::new();
        let mut out = Vec::new();
        for c in d.chars.drain(..) {
            text.push(c.ch);
            if let Some(m) = f.push(c.ch, c.error) {
                out.push(m);
            }
        }
        (text, out)
    }

    #[test]
    fn a_message_round_trips_with_one_polarity_only() {
        let t = "ZCZC XA01\n011200 UTC JAN 30\nTEST VECTOR FOR A SITOR-B RECEIVER.\nPOSITION 12-34.5N 012-34.5E, FIGURES 0123456789.\nNNNN";
        let words = message_words(t, 3.0);
        let bits = soft_of(&words);
        let (_, m) = receive(&bits, false);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].header, "XA01");
        assert!(m[0].complete);
        assert_eq!(m[0].errors, 0);
        assert_eq!(m[0].text, "011200 UTC JAN 30\nTEST VECTOR FOR A SITOR-B RECEIVER.\nPOSITION 12-34.5N 012-34.5E, FIGURES 0123456789.");
        assert_eq!(m[0].subject_name, "Navigational warning");
        // The generator's polarity is the receiver's: the inverted receiver finds nothing in these bits.
        let (_, inv) = receive(&bits, true);
        assert!(inv.is_empty());
        // Inverted bits decode with the inverted receiver.
        let neg: Vec<f32> = bits.iter().map(|b| -b).collect();
        let (_, m3) = receive(&neg, true);
        assert_eq!(m3[0].text, m[0].text);
        // Bit errors in DX copies are repaired by the RX copies.
        let start = 2 * 22 * 7;
        let mut hit = bits.clone();
        for k in 0..12 {
            let i = start + k * 7 * 6 + 3;
            hit[i] = -hit[i] * 0.3;
        }
        let (_, m2) = receive(&hit, false);
        assert_eq!(m2[0].text, m[0].text);
    }

    /// ITU-R M.625-4 Table 1 takes its figures case from ITU-T S.1: D is WRU, and F, G and H have no character. Through the
    /// receiver, figures D, F, G and H print nothing.
    #[test]
    fn the_figures_case_is_the_itu_one() {
        let w = |c: u8| ccir476::TRAFFIC[c as usize];
        let code = |c: char| ita2::ITU.code_of(c).unwrap().0;
        let mut dx = vec![w(ita2::LTRS), w(code('A')), w(ita2::FIGS)];
        for c in ['D', 'F', 'G', 'H', 'E'] {
            dx.push(w(code(c)));
        }
        let mut air = Vec::new();
        let mut enc = FecBEncoder::new();
        for _ in 0..20 {
            enc.push_dx(ccir476::PHASING_2, &mut |x| air.push(x));
        }
        for d in dx {
            enc.push_dx(d, &mut |x| air.push(x));
        }
        for _ in 0..30 {
            enc.push_dx(ccir476::ALPHA, &mut |x| air.push(x));
        }
        let (text, _) = receive(&soft_of(&air), false);
        assert_eq!(text, "A3", "{text:?}");
        assert!(!text.contains('$'));
    }

    /// When the receiver locks, it replays the words its bit phase holds with their own soft units. A character whose DX and RX
    /// copies are both damaged in that history (one weak wrong bit each) comes out right from the sum of the two copies.
    #[test]
    fn a_character_damaged_in_both_copies_before_the_lock_is_repaired_from_the_soft_sum() {
        let code = |c: char| ccir476::TRAFFIC[ita2::ITU.code_of(c).unwrap().0 as usize];
        let mut air = Vec::new();
        let mut enc = FecBEncoder::new();
        for _ in 0..4 {
            enc.push_dx(ccir476::PHASING_2, &mut |x| air.push(x));
        }
        enc.push_dx(ccir476::LTRS, &mut |x| air.push(x));
        for c in "ETESTAAA".chars() {
            enc.push_dx(code(c), &mut |x| air.push(x));
        }
        for _ in 0..40 {
            enc.push_dx(ccir476::ALPHA, &mut |x| air.push(x));
        }
        let mut bits = soft_of(&air);
        // The DX copy of the first E is at place 10, its RX copy at place 15 (the run of valid words starts again at each).
        assert_eq!(air[10], code('E'));
        assert_eq!(air[15], code('E'));
        bits[10 * 7 + 1] *= -0.3;
        bits[15 * 7 + 4] *= -0.3;
        let (text, _) = receive(&bits, false);
        assert!(text.starts_with("ETESTAAA"), "{text:?}");
    }

    /// After a bit slip the words of the new phase replace the places in the delay line with their own soft units, so a character
    /// damaged in both copies just before the slip is found still comes out right.
    #[test]
    fn a_character_damaged_in_both_copies_across_a_slip_is_repaired() {
        let code = |c: char| ccir476::TRAFFIC[ita2::ITU.code_of(c).unwrap().0 as usize];
        let mut air = Vec::new();
        let mut enc = FecBEncoder::new();
        for _ in 0..20 {
            enc.push_dx(ccir476::PHASING_2, &mut |x| air.push(x));
        }
        enc.push_dx(ccir476::LTRS, &mut |x| air.push(x));
        let text = "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG";
        for c in text.chars() {
            enc.push_dx(code(c), &mut |x| air.push(x));
        }
        for _ in 0..40 {
            enc.push_dx(ccir476::ALPHA, &mut |x| air.push(x));
        }
        let mut bits = soft_of(&air);
        // A bit lost four places before the DX copy of Q: every later word arrives one bit early.
        let first = air.iter().position(|&w| w == code('Q')).unwrap();
        let slip = (first - 4) * 7 + 3;
        bits[first * 7 + 2] *= -0.3;
        bits[(first + 5) * 7 + 5] *= -0.3;
        bits.remove(slip);
        let (got, _) = receive(&bits, false);
        assert!(got.contains("QUICK BROWN FOX"), "{got:?}");
    }

    #[test]
    fn list_rules() {
        let mut l = MessageList::new();
        let m = NavtexMessage {
            header: "XA01".into(),
            text: "T".into(),
            complete: true,
            ..Default::default()
        };
        assert!(l.add(m.clone()));
        assert!(!l.add(m.clone()));
        assert_eq!(l.messages.iter().last().unwrap().repeats, 1);
        assert_eq!((l.good, l.bad), (1, 0));
        assert_eq!(MessageList::copy_text(&m), "ZCZC XA01\nT\nNNNN");
    }
}
