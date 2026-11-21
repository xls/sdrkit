//! SITOR-A / AMTOR (ARQ, ITU-R M.476-5 Annex 1 section 3): a monitor that reads a two-station link from the hard unit decisions
//! of the SITOR receiver, and a two-station test link.
//!
//! The information sending station (ISS) sends a block of three words (21 units, 210 ms) at the start of every 450 ms cycle
//! (45 units). The information receiving station (IRS) answers in the pause with one control signal: CS1 or CS2 asks for the next
//! block, the same one again asks for a repeat, CS3 asks for a change of direction. The monitor transmits nothing: it finds the
//! cycle phase from blocks of three valid four-of-seven words, reads the control signal at the offset where it is found, drops
//! repeated blocks, and follows a change of direction to the other station.

use crate::decoder::Modulator;
use sdrkit_core::Complex32;
use sdrkit_util_text::ccir476::{FecBEncoder, Word, ALPHA, BETA, CS1, CS2, CS3, FIGS, LTRS, RQ};
use sdrkit_util_text::ita2;
use std::f64::consts::PI;

/// Units in a cycle.
pub const CYCLE: u64 = 45;
/// Units in a block.
pub const BLOCK: u64 = 21;
const RING: usize = 512;
const OFFSETS: usize = 18;
/// Characters of ARQ text kept.
pub const TEXT_KEEP: usize = 4000;

/// What the monitor reports as an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArqEvent {
    /// The monitor locked on a link.
    LinkFound,
    /// The monitor lost the link.
    LinkLost,
    /// A selective call (four letters).
    Call([u8; 4]),
    /// CS3: the receiving station asks to change direction.
    Cs3,
    /// The other station is sending now.
    NewSender,
    /// The change-over block (beta, alpha, beta).
    ChangeOverAck,
    /// "+?" in the figures case: the sending station hands over.
    HandsOver,
    /// End of communication (alpha, alpha, alpha).
    End,
}

impl ArqEvent {
    /// The event in words.
    pub fn text(&self) -> String {
        match self {
            ArqEvent::LinkFound => "link found".into(),
            ArqEvent::LinkLost => "link lost".into(),
            ArqEvent::Call(c) => format!("call to {}", String::from_utf8_lossy(c)),
            ArqEvent::Cs3 => "CS3, the receiving station asks to change direction".into(),
            ArqEvent::NewSender => "the other station is sending now".into(),
            ArqEvent::ChangeOverAck => "change-over acknowledged".into(),
            ArqEvent::HandsOver => "the sending station hands over".into(),
            ArqEvent::End => "end of communication".into(),
        }
    }
}

/// The ARQ monitor on one polarity.
#[derive(Clone, Debug)]
pub struct ArqMonitor {
    ring: [u8; RING],
    n: u64,
    hist: [u8; CYCLE as usize],
    /// Locked on a link.
    pub locked: bool,
    expect: u64,
    misses: u32,
    scan_at: Option<u64>,
    scores: [f32; OFFSETS],
    /// Units from the end of a block to the end of its control signal, minus 7.
    pub cs_offset: Option<usize>,
    last_cs: [u8; 2],
    last_block: Option<[Word; 3]>,
    call1: Option<[Word; 3]>,
    selcall: Option<[u8; 4]>,
    figures: bool,
    acked: bool,
    /// Valid blocks read while locked.
    pub blocks: u64,
    /// Blocks printed.
    pub new_blocks: u64,
    /// Blocks recognised as repeats.
    pub repeats: u64,
    /// Block slots without a valid block.
    pub bad: u64,
    /// Control signals read.
    pub control_signals: u64,
    /// Characters printed.
    pub chars: u64,
    /// Changes of direction seen.
    pub changes: u64,
    /// The text without repeats, newest at the end (the last [`TEXT_KEEP`] characters).
    pub text: String,
    /// Events not yet taken.
    pub events: Vec<ArqEvent>,
}

impl Default for ArqMonitor {
    fn default() -> Self {
        Self::new()
    }
}

fn circular_distance(a: u64, b: u64) -> u64 {
    let d = (a + CYCLE - b) % CYCLE;
    d.min(CYCLE - d)
}

impl ArqMonitor {
    /// A monitor that has seen nothing.
    pub fn new() -> Self {
        ArqMonitor {
            ring: [0; RING],
            n: 0,
            hist: [0; CYCLE as usize],
            locked: false,
            expect: 0,
            misses: 0,
            scan_at: None,
            scores: [0.0; OFFSETS],
            cs_offset: None,
            last_cs: [0; 2],
            last_block: None,
            call1: None,
            selcall: None,
            figures: false,
            acked: false,
            blocks: 0,
            new_blocks: 0,
            repeats: 0,
            bad: 0,
            control_signals: 0,
            chars: 0,
            changes: 0,
            text: String::with_capacity(TEXT_KEEP + 64),
            events: Vec::with_capacity(32),
        }
    }

    /// Forgets the link and the counts.
    pub fn reset(&mut self) {
        let mut text = std::mem::take(&mut self.text);
        let mut events = std::mem::take(&mut self.events);
        text.clear();
        events.clear();
        *self = ArqMonitor {
            text,
            events,
            ..Self::new()
        };
    }

    /// The selective call read from the call blocks.
    pub fn selcall(&self) -> Option<&str> {
        self.selcall
            .as_ref()
            .and_then(|c| std::str::from_utf8(c).ok())
    }

    /// The word of the 7 units ending at unit count `end`, when the units are still kept.
    fn word_at(&self, end: u64) -> Option<Word> {
        if end < 7 || end > self.n || self.n - end + 7 > RING as u64 {
            return None;
        }
        let mut w = 0u8;
        for i in 0..7 {
            w |= self.ring[((end - 7 + i) % RING as u64) as usize] << i;
        }
        Some(Word(w))
    }

    /// The block of three valid words ending at unit count `end`.
    fn block_at(&self, end: u64) -> Option<[Word; 3]> {
        if end < BLOCK {
            return None;
        }
        let mut b = [Word(0); 3];
        for (k, w) in b.iter_mut().enumerate() {
            *w = self.word_at(end - BLOCK + 7 * (k as u64 + 1))?;
            if !w.is_valid() {
                return None;
            }
        }
        Some(b)
    }

    /// One hard unit decision: 1 for Y (the lower tone), 0 for B.
    pub fn push(&mut self, y: u8) {
        self.ring[(self.n % RING as u64) as usize] = y & 1;
        self.n += 1;
        let e = self.n;
        let block = self.block_at(e);
        let phase = (e % CYCLE) as usize;
        self.hist[phase] = (self.hist[phase] << 1) | u8::from(block.is_some());
        if let Some(s) = self.scan_at {
            if e >= s + 24 {
                self.scan_at = None;
                self.scan(s);
            }
        }
        if !self.locked {
            if let Some(b) = block {
                if self.hist[phase] & 0xF == 0xF {
                    self.acquire(e, b);
                }
            }
            return;
        }
        if let Some(b) = block {
            if circular_distance(e, self.expect) > 2 {
                let after_ack = self.acked;
                let gone = self.misses >= 2 && self.hist[phase] & 0b11 == 0b11;
                if after_ack || gone {
                    self.new_sender(e, b);
                    return;
                }
            }
        }
        if e == self.expect + 1 {
            self.slot();
        }
    }

    fn acquire(&mut self, e: u64, b: [Word; 3]) {
        self.locked = true;
        self.events.push(ArqEvent::LinkFound);
        for k in [3, 2, 1] {
            let Some(end) = e.checked_sub(k * CYCLE) else {
                continue;
            };
            if let Some(w) = self.block_at(end) {
                self.blocks += 1;
                self.block(w);
            }
            self.scan(end);
        }
        self.take_block(e, b);
    }

    fn take_block(&mut self, end: u64, b: [Word; 3]) {
        self.blocks += 1;
        self.block(b);
        self.scan_at = Some(end);
        self.expect = end + CYCLE;
        self.misses = 0;
    }

    /// The slot of the expected block, judged one unit after it.
    fn slot(&mut self) {
        let x = self.expect;
        let found = if self.acked {
            None
        } else {
            [x, x - 1, x + 1]
                .into_iter()
                .find_map(|end| self.block_at(end).map(|b| (end, b)))
        };
        match found {
            Some((end, b)) => self.take_block(end, b),
            None => {
                self.bad += 1;
                self.misses += 1;
                self.scan_at = Some(x);
                self.expect = x + CYCLE;
                if self.misses >= 24 {
                    self.locked = false;
                    self.acked = false;
                    self.events.push(ArqEvent::LinkLost);
                }
            }
        }
    }

    fn new_sender(&mut self, e: u64, b: [Word; 3]) {
        self.events.push(ArqEvent::NewSender);
        self.changes += 1;
        self.figures = false;
        self.scores = [0.0; OFFSETS];
        self.cs_offset = None;
        self.last_cs = [0; 2];
        self.last_block = None;
        self.acked = false;
        let before = e.saturating_sub(CYCLE);
        if let Some(w) = self.block_at(before) {
            self.blocks += 1;
            self.block(w);
            self.scan(before);
        }
        self.take_block(e, b);
    }

    /// The control signal scan of the block that ended at unit count `end` (24 units later).
    fn scan(&mut self, end: u64) {
        let cs_of = |w: Option<Word>| match w {
            Some(w) if w == CS1 => 1,
            Some(w) if w == CS2 => 2,
            Some(w) if w == CS3 => 3,
            _ => 0,
        };
        for o in 0..OFFSETS {
            self.scores[o] *= 0.8;
            if cs_of(self.word_at(end + 7 + o as u64)) != 0 {
                self.scores[o] += 1.0;
            }
        }
        let mut best = (0.0f32, 0usize);
        let mut second = 0.0f32;
        for (o, &s) in self.scores.iter().enumerate() {
            if s > best.0 {
                second = best.0;
                best = (s, o);
            } else if s > second {
                second = s;
            }
        }
        self.cs_offset = (best.0 >= 2.0 && best.0 >= 1.4 * second).then_some(best.1);
        let cs = self
            .cs_offset
            .map_or(0, |o| cs_of(self.word_at(end + 7 + o as u64)));
        if cs != 0 {
            self.control_signals += 1;
        }
        if cs == 3 {
            self.changes += 1;
            self.events.push(ArqEvent::Cs3);
        }
        self.last_cs = [self.last_cs[1], cs];
    }

    fn letter(w: Word) -> u8 {
        w.ita2()
            .and_then(|c| ita2::ITU.char_of(c, false))
            .map_or(b'?', |c| c as u8)
    }

    /// One valid block (M.476-5 3.1.3 to 3.1.6).
    fn block(&mut self, b: [Word; 3]) {
        let rq = [b[0] == RQ, b[1] == RQ, b[2] == RQ];
        if rq.iter().any(|&r| r) {
            if rq == [false, true, false] {
                self.call1 = Some(b);
            } else if rq == [false, false, true] {
                if let Some(c) = self.call1 {
                    let call = [
                        Self::letter(c[0]),
                        Self::letter(c[2]),
                        Self::letter(b[0]),
                        Self::letter(b[1]),
                    ];
                    if self.selcall != Some(call) {
                        self.selcall = Some(call);
                        self.events.push(ArqEvent::Call(call));
                    }
                }
            }
            self.last_cs = [0; 2];
            self.last_block = None;
            return;
        }
        let same = self.last_block == Some(b);
        let [a, c] = self.last_cs;
        let repeat = if a != 0 && c != 0 && a != 3 && c != 3 {
            a == c && same
        } else {
            same
        };
        self.last_block = Some(b);
        if repeat {
            self.repeats += 1;
            return;
        }
        self.new_blocks += 1;
        if b == [ALPHA; 3] {
            self.events.push(ArqEvent::End);
            return;
        }
        if b == [BETA, ALPHA, BETA] {
            self.acked = true;
            self.events.push(ArqEvent::ChangeOverAck);
            return;
        }
        for w in b {
            if w == ALPHA || w == BETA || w == RQ {
                continue;
            }
            if w == LTRS || w == FIGS {
                self.figures = w == FIGS;
                continue;
            }
            let Some(c) = w
                .ita2()
                .and_then(|code| ita2::ITU.char_of(code, self.figures))
            else {
                continue;
            };
            if self.text.len() >= TEXT_KEEP {
                let cut = self.text.len() + 1 - TEXT_KEEP;
                let cut = (cut..=self.text.len())
                    .find(|&i| self.text.is_char_boundary(i))
                    .unwrap_or(self.text.len());
                self.text.drain(..cut);
            }
            self.text.push(c);
            self.chars += 1;
            if self.figures && self.text.ends_with("+?") {
                self.events.push(ArqEvent::HandsOver);
            }
        }
    }
}

/// The options of the two-station test link.
#[derive(Clone, Debug)]
pub struct LinkParams {
    /// FSK centre in the output, Hz.
    pub centre_hz: f64,
    /// Shift, Hz (B is centre + shift / 2).
    pub shift_hz: f64,
    /// Units a second.
    pub baud: f64,
    /// The second station's amplitude against the first (1.0).
    pub irs_level: f64,
    /// The second station keys this many units after the end of what it heard.
    pub irs_delay_bits: f64,
    /// The second transmitter's own frequency error, Hz.
    pub irs_offset_hz: f64,
    /// Slow drift of both, Hz a second.
    pub drift_hz_s: f64,
    /// The receiving station asks for a repeat after every n-th block (0: never).
    pub repeat_every: usize,
    /// Start with the call blocks and the answer.
    pub call: bool,
    /// The caller sends the pair of call blocks this many times in all.
    pub call_pairs: usize,
    /// The selective call, four letters.
    pub selcall: String,
    /// End the first text with "+?" and let the other station send `second_text`.
    pub change_over: bool,
    /// The text of the other station after a change-over.
    pub second_text: String,
    /// Finish with alpha, alpha, alpha.
    pub end: bool,
    /// Silence before, s.
    pub lead_s: f64,
    /// Silence after, s.
    pub tail_s: f64,
}

impl Default for LinkParams {
    fn default() -> Self {
        LinkParams {
            centre_hz: 0.0,
            shift_hz: 170.0,
            baud: 100.0,
            irs_level: 0.5,
            irs_delay_bits: 8.4,
            irs_offset_hz: 4.0,
            drift_hz_s: 0.0,
            repeat_every: 5,
            call: true,
            call_pairs: 4,
            selcall: "QCXT".into(),
            change_over: false,
            second_text: String::new(),
            end: true,
            lead_s: 0.5,
            tail_s: 0.5,
        }
    }
}

/// The traffic words of `text` with the letters and figures shifts, as mode B sends them; characters the code does not have
/// are left out.
pub fn text_words(text: &str) -> Vec<Word> {
    let clean: String = text
        .chars()
        .filter(|&c| c == '\n' || ita2::ITU.code_of(c).is_ok())
        .collect();
    let mut both = Vec::new();
    // The mode B encoder sends each word in a DX position followed by an RX position: the DX positions are the words in order.
    let _ = FecBEncoder::new().text(&clean, &mut both);
    both.into_iter().step_by(2).collect()
}

/// The traffic words padded with idle beta into blocks of three.
fn blocks_of(words: &[Word]) -> Vec<[Word; 3]> {
    words
        .chunks(3)
        .map(|c| {
            let mut b = [BETA; 3];
            b[..c.len()].copy_from_slice(c);
            b
        })
        .collect()
}

#[derive(Clone, Debug)]
struct Burst {
    start_s: f64,
    station: usize,
    words: Vec<Word>,
}

impl Burst {
    fn end_s(&self, baud: f64) -> f64 {
        self.start_s + 7.0 * self.words.len() as f64 / baud
    }
}

/// A two-station SITOR-A link in complex baseband, without noise: station A calls station B, sends `text`, and (with
/// `change_over`) hands over to B. Each burst is continuous-phase FSK with 1 ms raised-cosine edges.
pub struct ArqLink {
    p: LinkParams,
    rate: f64,
    bursts: Vec<Burst>,
    total: u64,
    t: u64,
    at: usize,
    phase: [f64; 2],
}

impl ArqLink {
    /// The link at `rate` samples a second.
    pub fn new(rate: f64, text: &str, p: LinkParams) -> Self {
        let tc = CYCLE as f64 / p.baud;
        let unit = 1.0 / p.baud;
        let mut bursts = Vec::new();
        let mut k = 0u32;
        let a_at = |k: u32| p.lead_s + f64::from(k) * tc;
        let a = |bursts: &mut Vec<Burst>, k: u32, words: &[Word]| {
            bursts.push(Burst {
                start_s: a_at(k),
                station: 0,
                words: words.to_vec(),
            });
        };
        let b_after = |bursts: &mut Vec<Burst>, words: &[Word]| {
            let end = bursts.last().map_or(0.0, |x: &Burst| x.end_s(p.baud));
            bursts.push(Burst {
                start_s: end + p.irs_delay_bits * unit,
                station: 1,
                words: words.to_vec(),
            });
        };
        if p.call {
            let mut sc = [LTRS; 4];
            let mut enc = ita2::Encoder::new(&ita2::ITU);
            for (i, c) in p.selcall.chars().take(4).enumerate() {
                let mut code = None;
                let _ = enc.encode_char(c, &mut |x| {
                    if x != ita2::LTRS && x != ita2::FIGS {
                        code = Some(x);
                    }
                });
                if let Some(w) = code.and_then(|x| Word::from_ita2(x).ok()) {
                    sc[i] = w;
                }
            }
            let call1 = [sc[0], RQ, sc[1]];
            let call2 = [sc[2], sc[3], RQ];
            for _ in 1..p.call_pairs.max(1) {
                a(&mut bursts, k, &call1);
                k += 1;
                a(&mut bursts, k, &call2);
                k += 1;
            }
            a(&mut bursts, k, &call1);
            k += 1;
            a(&mut bursts, k, &call2);
            b_after(&mut bursts, &[CS1]);
            k += 1;
            a(&mut bursts, k, &call1);
            b_after(&mut bursts, &[CS1]);
            k += 1;
        }
        let first = if p.change_over {
            format!("{text}+?")
        } else {
            text.to_owned()
        };
        let other = |c: Word| if c == CS1 { CS2 } else { CS1 };
        let mut cs = CS1;
        for (i, blk) in blocks_of(&text_words(&first)).iter().enumerate() {
            a(&mut bursts, k, blk);
            k += 1;
            if p.repeat_every > 0 && (i + 1).is_multiple_of(p.repeat_every) {
                b_after(&mut bursts, &[cs]);
                a(&mut bursts, k, blk);
                k += 1;
            }
            cs = other(cs);
            b_after(&mut bursts, &[cs]);
        }
        if p.change_over {
            a(&mut bursts, k, &[BETA; 3]);
            b_after(&mut bursts, &[CS3]);
            k += 1;
            a(&mut bursts, k, &[BETA, ALPHA, BETA]);
            k += 1;
            a(&mut bursts, k, &[CS1]);
            b_after(&mut bursts, &[RQ; 3]);
            k += 1;
            let mut cs = CS1;
            for (i, blk) in blocks_of(&text_words(&p.second_text)).iter().enumerate() {
                a(&mut bursts, k, &[cs]);
                b_after(&mut bursts, blk);
                k += 1;
                if p.repeat_every > 0 && (i + 1).is_multiple_of(p.repeat_every) {
                    a(&mut bursts, k, &[cs]);
                    b_after(&mut bursts, blk);
                    k += 1;
                }
                cs = other(cs);
            }
            a(&mut bursts, k, &[cs]);
            b_after(&mut bursts, &[ALPHA; 3]);
        } else if p.end {
            a(&mut bursts, k, &[ALPHA; 3]);
            b_after(&mut bursts, &[CS1]);
        }
        let last = bursts.iter().map(|b| b.end_s(p.baud)).fold(0.0, f64::max);
        let total = ((last + p.tail_s) * rate) as u64;
        ArqLink {
            p,
            rate,
            bursts,
            total,
            t: 0,
            at: 0,
            phase: [0.0; 2],
        }
    }

    /// Samples in the whole link.
    pub fn len(&self) -> u64 {
        self.total
    }

    /// True when the link has no samples.
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// The words station A sends in its blocks, in order (repeats included).
    pub fn words_of(&self, station: usize) -> impl Iterator<Item = &[Word]> {
        self.bursts
            .iter()
            .filter(move |b| b.station == station)
            .map(|b| b.words.as_slice())
    }
}

impl Modulator for ArqLink {
    /// Fills `out` with the next samples of the link; silence after its end.
    fn fill(&mut self, out: &mut [Complex32]) {
        let p = &self.p;
        let edge = 1e-3;
        for o in out {
            let t = self.t as f64 / self.rate;
            self.t += 1;
            while self.at < self.bursts.len() && t >= self.bursts[self.at].end_s(p.baud) {
                self.at += 1;
            }
            *o = Complex32::default();
            let Some(b) = self.bursts.get(self.at) else {
                continue;
            };
            if t < b.start_s {
                continue;
            }
            let into = t - b.start_s;
            let unit = ((into * p.baud) as usize).min(7 * b.words.len() - 1);
            let y = (b.words[unit / 7].0 >> (unit % 7)) & 1 == 1;
            let s = b.station;
            let off = if s == 1 { p.irs_offset_hz } else { 0.0 };
            let f = p.centre_hz + off + p.drift_hz_s * t + if y { -0.5 } else { 0.5 } * p.shift_hz;
            self.phase[s] = (self.phase[s] + 2.0 * PI * f / self.rate) % (2.0 * PI);
            let left = b.end_s(p.baud) - t;
            let env = if into < edge {
                0.5 * (1.0 - (PI * into / edge).cos())
            } else if left < edge {
                0.5 * (1.0 - (PI * left / edge).cos())
            } else {
                1.0
            };
            let amp = env * if s == 1 { p.irs_level } else { 1.0 };
            *o = Complex32::new(
                (amp * self.phase[s].cos()) as f32,
                (amp * self.phase[s].sin()) as f32,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b_is_one(w: Word) -> u8 {
        !w.0 & 0x7F
    }

    #[test]
    fn service_signals_and_text_blocks() {
        assert_eq!(
            [CS1, CS2, CS3, BETA, ALPHA, RQ].map(b_is_one),
            [0x65, 0x6A, 0x59, 0x33, 0x0F, 0x66]
        );
        for w in [CS1, CS2, CS3, BETA, ALPHA, RQ] {
            assert!(w.is_valid());
        }
        assert_eq!(
            Some(CS1.ita2().unwrap()),
            ita2::ITU.code_of('L').ok().map(|c| c.0)
        );
        assert_eq!(sdrkit_util_text::ccir476::word("BBBYYYB"), TRAFFIC_A);
        let blocks = blocks_of(&text_words("CQ DE TEST 123"));
        assert!(blocks.iter().flatten().all(|w| w.is_valid()));
    }

    const TRAFFIC_A: Word = sdrkit_util_text::ccir476::TRAFFIC[3];

    #[test]
    fn the_monitor_reads_a_clean_link() {
        let text = "CQ DE TEST VESSEL 1234567890 /";
        let link = ArqLink::new(100.0, text, LinkParams::default());
        // Feed the units of the link on the cycle grid: A's blocks and B's control signals at whole units.
        let mut m = ArqMonitor::new();
        let mut units = Vec::new();
        let mut t = 0.0;
        for b in &link.bursts {
            let start = (b.start_s * 100.0).round() as usize;
            while (units.len() as f64) < start as f64 {
                units.push(t as u8 & 1);
                t += 1.0;
            }
            for w in &b.words {
                for i in 0..7 {
                    units.push((w.0 >> i) & 1);
                }
            }
        }
        for &u in &units {
            m.push(u);
        }
        assert_eq!(m.text.matches(text).count(), 1, "{:?}", m.text);
        assert_eq!(m.selcall(), Some("QCXT"));
        assert!(m.repeats >= 1);
        assert!(m.events.contains(&ArqEvent::End));
    }
}
