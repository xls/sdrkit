//! POCSAG ("Radio-paging code No. 1") at 512, 1200 and 2400 bit/s. A batch is a sync word and 8 frames of 2 code words, and a
//! pager listens to the frame selected by the 3 low bits of its 21-bit identity (the RIC). An address word holds a 0 flag, the
//! 18 high bits of the RIC and 2 function bits. A message word holds a 1 flag and 20 message bits.
//!
//! Standard: ITU-R M.584-2 annex 1 clauses 1.2 and 1.3.
//!
//! # Acceptance rules
//!
//! The standard defines no message check, only the code word (at most 2 repaired bits, [`crate::word`]). The deframer takes
//! a sync word within `sync_errors` bits, keeps the first batch only when `first_batch_valid` of its words pass, and ends the
//! transmission at a sync word beyond `resync_errors` bits. A page ends at the idle word, the next address word, two lost words
//! in a row or the end of the transmission. Text pages keep printable characters, and the numeric or text reading follows
//! [`ReadMode`].
//!
//! These rules are not stated by a public document. A word within [`IDLE_NEAR`] bits of the idle word, or one more when the
//! hard repair fails, is the idle word, whatever another code word lies nearer. An address word that needed 2 repaired bits gives a page only when the word after it
//! is a message word that passed, or, for a tone page, when both repaired bits are among the [`WEAK_BITS`] least reliable bits
//! of the word (always for hard input). A refused page is counted in [`PocsagAssembler::refusals`] as unconfirmed.

use crate::charset::push_lsb_first;
use crate::page::{
    self, choose_reading, looks_like_text, Kind, MessageBits, Page, Protocol, ReadMode, Refusals,
};
use crate::vote::vote;
use crate::word::{self, Checked, SoftRepair};
use crate::{Error, Result, Setting};
use sdrkit_util_bits::sync::{SoftSync, SyncWord};

pub use crate::charset::NumericSet;

/// The synchronisation code word (M.584-2 annex 1 Table 1).
pub const SYNC: u32 = 0x7CD2_15D8;

/// The idle code word as transmitters send it. M.584-2 annex 1 Table 2 prints 0x7AC9C197, which is not a code word of the
/// clause 1.4 code. 0x7A89C197 is (one bit differs), and it is the word on the air.
pub const IDLE: u32 = 0x7A89_C197;

/// The word that Table 2 prints, kept for the test that shows the misprint.
pub const IDLE_AS_PRINTED: u32 = 0x7AC9_C197;

/// Bits of the preamble (reversals 1, 0, 1, 0 ...) before the first sync word.
pub const PREAMBLE_BITS: usize = 576;

/// A received word within this many bits of [`IDLE`] is read as the idle word. Two other code words are at least 6 bits apart,
/// so no message or address word with fewer than 2 wrong bits is read so.
pub const IDLE_NEAR: u32 = 4;

/// The two repaired bits of the address word of a tone page must be among this many least reliable bits of the word. Nearly
/// half of all words of noise, and of words with three wrong bits, are two bits from a code word at any two places.
pub const WEAK_BITS: usize = 8;

/// Code words in a batch after the sync word.
pub const BATCH_WORDS: usize = 16;

/// The largest RIC (21 bits).
pub const MAX_RIC: u32 = (1 << 21) - 1;

/// A POCSAG bit rate. The rate does not change the code. The demodulator finds the rate and passes it to the decoder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Rate {
    /// 512 bit/s.
    B512,
    /// 1200 bit/s.
    #[default]
    B1200,
    /// 2400 bit/s.
    B2400,
}

impl Rate {
    /// All three rates.
    pub const ALL: [Rate; 3] = [Rate::B512, Rate::B1200, Rate::B2400];

    /// Bits per second.
    pub const fn bits_per_s(self) -> u32 {
        match self {
            Rate::B512 => 512,
            Rate::B1200 => 1200,
            Rate::B2400 => 2400,
        }
    }

    /// "POCSAG 512", "POCSAG 1200" or "POCSAG 2400".
    pub const fn name(self) -> &'static str {
        match self {
            Rate::B512 => "POCSAG 512",
            Rate::B1200 => "POCSAG 1200",
            Rate::B2400 => "POCSAG 2400",
        }
    }
}

/// The address word of a RIC and function code (clause 1.3.2).
pub fn address_word(ric: u32, function: u8) -> Result<u32> {
    if ric > MAX_RIC {
        return Err(Error::RicTooLarge(ric));
    }
    if function > 3 {
        return Err(Error::BadFunction(function));
    }
    Ok(word::encode(((ric >> 3) << 2) | function as u32))
}

/// The message word of 20 message bits.
pub fn message_word(data20: u32) -> u32 {
    word::encode((1 << 20) | (data20 & 0xF_FFFF))
}

/// The RIC and function code of an address word received in batch slot `slot` (0 to 15).
pub fn address_of(cw: u32, slot: usize) -> (u32, u8) {
    let d = word::data(cw);
    (
        (((d >> 2) & 0x3_FFFF) << 3) | ((slot as u32 / 2) & 7),
        (d & 3) as u8,
    )
}

/// What a received word is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordKind {
    /// The idle word, or a word of all zeros or all ones (a silent channel and a stuck discriminator produce these). It ends a page.
    Idle,
    /// An address word.
    Address,
    /// A message word.
    Message,
    /// A word that no repair can save.
    Lost,
}

/// The kind of a word after the repair.
pub fn kind_of(w: &Checked) -> WordKind {
    if !w.valid {
        WordKind::Lost
    } else if w.cw == IDLE || w.cw == 0 || w.cw == u32::MAX {
        WordKind::Idle
    } else if w.cw >> 31 == 0 {
        WordKind::Address
    } else {
        WordKind::Message
    }
}

/// True when every bit that the repair of `w` changed is among the [`WEAK_BITS`] least reliable bits of `rel` (index 0 the first
/// bit sent).
fn repaired_bits_weak(w: Checked, rel: &[f32; 32]) -> bool {
    let mut changed = w.cw ^ w.raw;
    while changed != 0 {
        let i = changed.leading_zeros() as usize;
        changed &= !(0x8000_0000 >> i);
        if rel.iter().filter(|&&r| r < rel[i]).count() >= WEAK_BITS {
            return false;
        }
    }
    true
}

/// The settings of a POCSAG decoder.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The rate name the pages carry.
    pub rate: Rate,
    /// How pages are read as numeric or text.
    pub read: ReadMode,
    /// Rules per address: (RIC, text?). A rule wins over `read`.
    pub by_address: Vec<(u32, bool)>,
    /// The numeric set.
    pub numeric: NumericSet,
    /// Vote repeated copies of a message.
    pub combine: bool,
    /// Copies older than this (seconds) do not take part in a vote.
    pub max_age_s: f64,
    /// Messages kept for voting.
    pub history: usize,
    /// The longest message in words. Longer messages are cut and marked damaged.
    pub max_words: usize,
    /// Bit errors allowed in the first sync word.
    pub sync_errors: u32,
    /// Bit errors allowed in the sync word of each later batch.
    pub resync_errors: u32,
    /// Soft input: the normalised correlation (0 to 1) that finds the first sync word, beside `sync_errors`.
    pub soft_sync_score: f32,
    /// Words of the first batch that must pass the repair, or the sync word was false.
    pub first_batch_valid: u32,
    /// The soft repair of message words. Address words never take a soft repair.
    pub soft: SoftRepair,
    /// The page ends after this many lost words in a row (M.584-2 annex 1 clause 2.5.1 says two). 0 turns the rule off.
    pub end_after_lost: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rate: Rate::B1200,
            read: ReadMode::Auto,
            by_address: Vec::new(),
            numeric: NumericSet::Itu,
            combine: true,
            max_age_s: 900.0,
            history: 32,
            max_words: 256,
            sync_errors: 2,
            resync_errors: 4,
            soft_sync_score: 0.8,
            first_batch_valid: 10,
            soft: SoftRepair::MESSAGE,
            end_after_lost: 2,
        }
    }
}

/// The output of the deframer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A word of a batch: its slot (0 to 15), the hard bits, and the reliabilities (all 1.0 for hard input).
    Word {
        /// The slot in the batch.
        slot: usize,
        /// The bits as received (after the polarity of the sync word).
        raw: u32,
        /// The reliability of each bit, bit 0 the first sent.
        rel: [f32; 32],
    },
    /// The batch ended. `valid` words of it passed the hard repair.
    BatchEnd {
        /// Words that passed.
        valid: u32,
    },
    /// The transmission ended: no sync word after a batch, or a batch with no good word.
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Search,
    Words,
    Sync,
}

/// Finds batches in a stream of bits and cuts them into words. It allocates nothing.
#[derive(Clone, Debug)]
pub struct PocsagDeframer {
    sync: SyncWord,
    soft: SoftSync,
    resync_errors: u32,
    first_valid: u32,
    state: State,
    inverted: bool,
    first: bool,
    reg: u32,
    rel: [f32; 32],
    nbits: usize,
    slot: usize,
    valid: u32,
    held: [(u32, [f32; 32]); BATCH_WORDS],
}

impl PocsagDeframer {
    /// A deframer with the sync rules of `cfg`. An error when `sync_errors` or `resync_errors` is above 15, when
    /// `first_batch_valid` is above 16, or when `soft_sync_score` is not 0 to 1.
    pub fn new(cfg: &Config) -> Result<PocsagDeframer> {
        if cfg.sync_errors > 15 || cfg.resync_errors > 15 {
            return Err(Error::BadSetting(Setting::PocsagSyncErrors));
        }
        if cfg.first_batch_valid > BATCH_WORDS as u32 {
            return Err(Error::BadSetting(Setting::FirstBatchValidWords));
        }
        let sync = SyncWord::new(SYNC as u128, 32, cfg.sync_errors)
            .map_err(|_| Error::BadSetting(Setting::PocsagSyncErrors))?
            .with_inverted(true);
        let soft = SoftSync::new(SYNC as u128, 32, cfg.soft_sync_score)
            .map_err(|_| Error::BadSetting(Setting::PocsagSoftSyncScore))?
            .with_inverted(true);
        Ok(PocsagDeframer {
            sync,
            soft,
            resync_errors: cfg.resync_errors,
            first_valid: cfg.first_batch_valid,
            state: State::Search,
            inverted: false,
            first: true,
            reg: 0,
            rel: [0.0; 32],
            nbits: 0,
            slot: 0,
            valid: 0,
            held: [(0, [0.0; 32]); BATCH_WORDS],
        })
    }

    /// True while the deframer is inside a transmission.
    pub fn locked(&self) -> bool {
        self.state != State::Search
    }

    /// True when the sync word came inverted (the bits after it are inverted back).
    pub fn inverted(&self) -> bool {
        self.inverted
    }

    /// Forgets the transmission (for example after a retune).
    pub fn reset(&mut self) {
        self.sync.reset();
        self.soft.reset();
        self.state = State::Search;
        self.nbits = 0;
        self.reg = 0;
    }

    /// Takes one hard bit.
    pub fn push_bit(&mut self, bit: u8, sink: impl FnMut(Event)) {
        self.push(bit & 1, 1.0, None, sink);
    }

    /// Takes one soft bit (an LLR, positive means 0).
    pub fn push_llr(&mut self, llr: f32, sink: impl FnMut(Event)) {
        let rel = if llr.is_nan() { 0.0 } else { llr.abs() };
        self.push((llr < 0.0) as u8, rel, Some(llr), sink);
    }

    fn push(&mut self, bit: u8, rel: f32, llr: Option<f32>, mut sink: impl FnMut(Event)) {
        if self.state == State::Search {
            // Hard bits: the sync word within `sync_errors`. Soft bits: also the soft correlation.
            let hard = self.sync.push(bit).map(|h| h.inverted());
            let soft = llr
                .map(|l| if l.is_nan() { 0.0 } else { l })
                .and_then(|l| self.soft.push(l))
                .map(|h| h.score < 0.0);
            if let Some(inverted) = hard.or(soft) {
                self.inverted = inverted;
                self.state = State::Words;
                self.first = true;
                self.slot = 0;
                self.valid = 0;
                self.nbits = 0;
                self.reg = 0;
            }
            return;
        }
        let bit = bit ^ self.inverted as u8;
        self.reg = (self.reg << 1) | bit as u32;
        self.rel[self.nbits] = rel;
        self.nbits += 1;
        if self.nbits < 32 {
            return;
        }
        self.nbits = 0;
        let w = std::mem::take(&mut self.reg);
        match self.state {
            State::Sync => {
                if (w ^ SYNC).count_ones() <= self.resync_errors {
                    self.state = State::Words;
                    self.slot = 0;
                    self.valid = 0;
                } else {
                    sink(Event::End);
                    self.reset();
                }
            }
            State::Words => {
                if word::repair(w).is_some() {
                    self.valid += 1;
                }
                if self.first {
                    self.held[self.slot] = (w, self.rel);
                } else {
                    sink(Event::Word {
                        slot: self.slot,
                        raw: w,
                        rel: self.rel,
                    });
                }
                self.slot += 1;
                if self.slot == BATCH_WORDS {
                    if self.first {
                        self.first = false;
                        if self.valid < self.first_valid {
                            self.reset();
                            return;
                        }
                        for (slot, &(raw, rel)) in self.held.iter().enumerate() {
                            sink(Event::Word { slot, raw, rel });
                        }
                    }
                    sink(Event::BatchEnd { valid: self.valid });
                    if self.valid == 0 {
                        sink(Event::End);
                        self.reset();
                    } else {
                        self.state = State::Sync;
                    }
                }
            }
            State::Search => {}
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Copy {
    used: bool,
    ric: u32,
    function: u8,
    addr: u32,
    words: Vec<Checked>,
    doubtful_end: bool,
    time: f64,
}

/// Counts of words since the decoder was made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WordCounts {
    /// Words that passed as received.
    pub good: u64,
    /// Words that a repair saved.
    pub repaired: u64,
    /// Words that were lost.
    pub lost: u64,
}

/// Makes pages from the words of one paging channel, with voting of repeated copies.
#[derive(Clone, Debug)]
pub struct PocsagAssembler {
    cfg: Config,
    open: bool,
    ric: u32,
    function: u8,
    addr: Checked,
    /// The repaired bits of the address word are among its [`WEAK_BITS`] weakest.
    addr_weak: bool,
    words: Vec<Checked>,
    cut: bool,
    lost_run: u32,
    history: Vec<Copy>,
    merged: Vec<Checked>,
    bits: MessageBits,
    page: Page,
    counts: WordCounts,
    refusals: Refusals,
}

impl PocsagAssembler {
    /// An assembler with every buffer sized from `cfg`. It returns an error when `max_words` is 0 or above 4096, or `history`
    /// is above 1024. It also returns an error when `max_age_s` or a soft limit is not a finite number of 0 or more.
    pub fn new(cfg: Config) -> Result<PocsagAssembler> {
        if cfg.max_words == 0 || cfg.max_words > 4096 {
            return Err(Error::BadSetting(Setting::LongestMessage));
        }
        if cfg.history > 1024 {
            return Err(Error::BadSetting(Setting::MessagesKept));
        }
        if !(cfg.max_age_s >= 0.0 && cfg.max_age_s.is_finite()) || !cfg.soft.doubt.is_finite() {
            return Err(Error::BadSetting(Setting::AgeAndSoftLimits));
        }
        let n = cfg.max_words;
        let history = (0..cfg.history)
            .map(|_| Copy {
                words: Vec::with_capacity(n),
                ..Copy::default()
            })
            .collect();
        let mut page = Page {
            text: String::with_capacity(n * 3),
            numeric: String::with_capacity(n * 5),
            alpha: String::with_capacity(n * 3),
            raw: Vec::with_capacity(n * 3),
            ..Page::default()
        };
        page.clear();
        Ok(PocsagAssembler {
            open: false,
            ric: 0,
            function: 0,
            addr: Checked::default(),
            addr_weak: true,
            words: Vec::with_capacity(n),
            cut: false,
            lost_run: 0,
            history,
            merged: Vec::with_capacity(n),
            bits: MessageBits::with_capacity(n * 20),
            page,
            counts: WordCounts::default(),
            refusals: Refusals::default(),
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &Config {
        &self.cfg
    }

    /// Changes how pages are read. It takes effect from the next page.
    pub fn set_read(&mut self, read: ReadMode) {
        self.cfg.read = read;
    }

    /// Turns voting on or off.
    pub fn set_combine(&mut self, on: bool) {
        self.cfg.combine = on;
    }

    /// The word counts.
    pub fn counts(&self) -> WordCounts {
        self.counts
    }

    /// The pages refused by the acceptance rules (module documentation).
    pub fn refusals(&self) -> Refusals {
        self.refusals
    }

    /// Processes the word of batch slot `slot` (0 to 15). `rel` holds the bit reliabilities for the soft repair. `time` is the caller's
    /// time of the word. A page that the word closes goes to `sink`.
    pub fn push(
        &mut self,
        slot: usize,
        raw: u32,
        rel: Option<&[f32; 32]>,
        time: f64,
        mut sink: impl FnMut(&Page),
    ) {
        let near_idle = (raw ^ IDLE).count_ones();
        let mut w = Checked::hard(raw);
        if near_idle <= IDLE_NEAR + !w.valid as u32 {
            w = Checked {
                cw: IDLE,
                raw,
                valid: true,
                fixes: near_idle as u8,
            };
        } else if !w.valid {
            w = Checked::soft(raw, rel, self.cfg.soft);
            // A soft repair never makes an address word: a false address would start a false page.
            if w.valid && w.cw >> 31 == 0 {
                w = Checked::lost(raw);
            }
        }
        if w.clean() {
            self.counts.good += 1;
        } else if w.valid {
            self.counts.repaired += 1;
        } else {
            self.counts.lost += 1;
        }
        match kind_of(&w) {
            WordKind::Address => {
                if self.close(time, w.fixes > 0) {
                    sink(&self.page);
                }
                let (ric, function) = address_of(w.cw, slot);
                self.open = true;
                self.ric = ric;
                self.function = function;
                self.addr = w;
                self.addr_weak = rel.is_none_or(|r| repaired_bits_weak(w, r));
                self.words.clear();
                self.cut = false;
                self.lost_run = 0;
            }
            WordKind::Message => {
                if self.open {
                    self.lost_run = 0;
                    self.add(w);
                }
            }
            WordKind::Lost => {
                if self.open {
                    self.lost_run += 1;
                    self.add(w);
                    if self.cfg.end_after_lost > 0
                        && self.lost_run >= self.cfg.end_after_lost
                        && self.close(time, true)
                    {
                        sink(&self.page);
                    }
                }
            }
            WordKind::Idle => {
                if self.close(time, w.fixes > 0) {
                    sink(&self.page);
                }
            }
        }
    }

    fn add(&mut self, w: Checked) {
        if self.words.len() < self.cfg.max_words {
            self.words.push(w);
        } else {
            self.cut = true;
        }
    }

    /// The transmission ended. The open page (if any) goes to `sink`, marked as having a doubtful end.
    pub fn end(&mut self, time: f64, mut sink: impl FnMut(&Page)) {
        if self.close(time, true) {
            sink(&self.page);
        }
    }

    /// Forgets the open page (for example after a retune). The voting history stays.
    pub fn reset(&mut self) {
        self.open = false;
        self.words.clear();
    }

    /// Forgets the open page and the voting history.
    pub fn clear(&mut self) {
        self.reset();
        for c in &mut self.history {
            c.used = false;
        }
        self.counts = WordCounts::default();
        self.refusals = Refusals::default();
    }

    fn close(&mut self, time: f64, doubtful_end: bool) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        // An address word with 2 repaired bits needs a message word that passed after it, or weak repaired bits.
        let follows = match self.words.first() {
            Some(w) => w.valid,
            None => self.addr_weak,
        };
        if self.addr.fixes >= 2 && !follows {
            self.words.clear();
            self.refusals.count(true);
            return false;
        }
        let mut cut = doubtful_end || self.cut;
        let sent = self.words.len();
        while self.words.last().is_some_and(|w| !w.valid) {
            self.words.pop();
            cut = true;
        }
        let (mut copies, mut combined) = (1, false);
        if self.cfg.combine && !self.history.is_empty() {
            (copies, combined) = self.combine(time, cut);
        }
        // Lost words at the end that no copy provided back: the message went on past what the page shows.
        let lost_at_end = sent.saturating_sub(self.words.len()) as u32;
        let truncated = self.cut;
        self.build(time, copies, combined, truncated);
        if lost_at_end > 0 {
            self.page.lost_words += lost_at_end;
            self.page.damaged = true;
        }
        true
    }

    fn same_message(&self, c: &Copy, cut: bool) -> bool {
        if !c.used || c.ric != self.ric || c.function != self.function || c.addr != self.addr.cw {
            return false;
        }
        let (a, b) = (&self.words, &c.words);
        if a.is_empty() || b.is_empty() {
            // An address word alone matches a longer copy only when what ended it was doubtful.
            let short_cut = if a.is_empty() { cut } else { c.doubtful_end };
            return short_cut && !(a.is_empty() && b.is_empty());
        }
        let n = a.len().min(b.len());
        let (mut eq, mut diff) = (0usize, 0usize);
        for (x, y) in a.iter().zip(b.iter()).take(n) {
            if x.valid && y.valid {
                if x.cw == y.cw {
                    eq += 1;
                } else {
                    diff += 1;
                }
            }
        }
        eq >= 1 && eq * 3 >= n && diff * 10 <= eq
    }

    /// Votes the open page against the history, then stores it. Returns (copies, combined).
    fn combine(&mut self, time: f64, cut: bool) -> (u32, bool) {
        let max_age = self.cfg.max_age_s;
        for c in &mut self.history {
            if c.used && (time - c.time > max_age || time < c.time - max_age) {
                c.used = false;
            }
        }
        // Up to 4 matching copies, newest first.
        let mut found = [usize::MAX; 4];
        let mut nf = 0;
        let mut order = [usize::MAX; 4];
        for (i, c) in self.history.iter().enumerate() {
            if !self.same_message(c, cut) {
                continue;
            }
            if nf < 4 {
                found[nf] = i;
                nf += 1;
            } else {
                // Replace the oldest kept match when this one is newer.
                let (k, _) = found
                    .iter()
                    .enumerate()
                    .min_by(|a, b| self.history[*a.1].time.total_cmp(&self.history[*b.1].time))
                    .unwrap_or((0, &0));
                if self.history[found[k]].time < c.time {
                    found[k] = i;
                }
            }
        }
        order[..nf].copy_from_slice(&found[..nf]);
        order[..nf]
            .sort_unstable_by(|a, b| self.history[*b].time.total_cmp(&self.history[*a].time));
        let mut result = (1, false);
        if nf > 0 {
            let longest = order[..nf]
                .iter()
                .map(|&i| self.history[i].words.len())
                .max()
                .unwrap_or(0);
            let len = if cut {
                self.words.len().max(longest)
            } else {
                self.words.len()
            };
            let len = len.min(self.cfg.max_words);
            self.merged.clear();
            let mut changed = false;
            for k in 0..len {
                let mut cands = [Checked::default(); 5];
                let mut nc = 0;
                if let Some(w) = self.words.get(k) {
                    cands[nc] = *w;
                    nc += 1;
                }
                for &i in &order[..nf] {
                    if let Some(w) = self.history[i].words.get(k) {
                        cands[nc] = *w;
                        nc += 1;
                    }
                }
                let current = self.words.get(k).copied();
                let w = vote(&cands[..nc]).unwrap_or(current.unwrap_or_default());
                if current.is_none_or(|c| c.cw != w.cw || c.valid != w.valid) {
                    changed = true;
                }
                self.merged.push(w);
            }
            while self.merged.last().is_some_and(|w| !w.valid) {
                self.merged.pop();
            }
            if changed {
                std::mem::swap(&mut self.words, &mut self.merged);
                result = (1 + nf as u32, true);
            }
        }
        // Store this copy: at most 4 per address and function, else into a free slot or over the oldest.
        let (ric, function) = (self.ric, self.function);
        let mut in_group = 0;
        let mut oldest_in_group: Option<usize> = None;
        for (i, c) in self.history.iter().enumerate() {
            if c.used && c.ric == ric && c.function == function {
                in_group += 1;
                if oldest_in_group.is_none_or(|o| c.time < self.history[o].time) {
                    oldest_in_group = Some(i);
                }
            }
        }
        let slot = if in_group >= 4 {
            oldest_in_group
        } else {
            self.history.iter().position(|c| !c.used).or_else(|| {
                self.history
                    .iter()
                    .enumerate()
                    .min_by(|a, b| a.1.time.total_cmp(&b.1.time))
                    .map(|(i, _)| i)
            })
        };
        if let Some(i) = slot {
            let addr = self.addr.cw;
            let c = &mut self.history[i];
            c.used = true;
            c.ric = ric;
            c.function = function;
            c.addr = addr;
            c.doubtful_end = cut;
            c.time = time;
            c.words.clear();
            c.words.extend_from_slice(&self.words);
        }
        result
    }

    fn build(&mut self, time: f64, copies: u32, combined: bool, truncated: bool) {
        let p = &mut self.page;
        p.clear();
        p.protocol = Protocol::Pocsag;
        p.rate = self.cfg.rate.name();
        p.time = time;
        p.address = self.ric;
        p.function = Some(self.function);
        p.copies = copies;
        p.combined = combined;
        p.bits_repaired = self.addr.fixes as u32;
        p.damaged = truncated;
        self.bits.clear();
        for w in &self.words {
            if w.valid {
                self.bits.push_msb(word::data(w.cw), 20);
                p.bits_repaired += w.fixes as u32;
            } else {
                self.bits.push_lost(20);
                p.lost_words += 1;
                p.damaged = true;
            }
        }
        if self.words.is_empty() {
            p.kind = Kind::Tone;
            return;
        }
        self.bits
            .numeric_into(0, self.cfg.numeric.chars(), None, &mut p.numeric);
        self.bits.alpha_raw_into(&mut p.raw);
        page::printable_into(&p.raw, &mut p.alpha);
        p.looks_like_text = looks_like_text(&p.raw);
        let by = self
            .cfg
            .by_address
            .iter()
            .find(|a| a.0 == self.ric)
            .map(|a| a.1);
        let (alpha, why) = choose_reading(
            self.cfg.read,
            self.function,
            &p.numeric,
            p.looks_like_text,
            by,
        );
        p.reason = why;
        if alpha {
            p.kind = Kind::Alpha;
            p.text.push_str(&p.alpha);
        } else {
            p.kind = Kind::Numeric;
            p.text.push_str(&p.numeric);
        }
    }
}

/// A POCSAG decoder: the deframer and the assembler in one.
#[derive(Clone, Debug)]
pub struct PocsagDecoder {
    deframer: PocsagDeframer,
    assembler: PocsagAssembler,
}

impl PocsagDecoder {
    /// A decoder with every buffer sized from `cfg`. The errors are those of [`PocsagDeframer::new`] and
    /// [`PocsagAssembler::new`].
    pub fn new(cfg: Config) -> Result<PocsagDecoder> {
        Ok(PocsagDecoder {
            deframer: PocsagDeframer::new(&cfg)?,
            assembler: PocsagAssembler::new(cfg)?,
        })
    }

    /// The deframer.
    pub fn deframer(&self) -> &PocsagDeframer {
        &self.deframer
    }

    /// The assembler.
    pub fn assembler_mut(&mut self) -> &mut PocsagAssembler {
        &mut self.assembler
    }

    /// The word counts.
    pub fn counts(&self) -> WordCounts {
        self.assembler.counts()
    }

    /// The pages refused by the acceptance rules.
    pub fn refusals(&self) -> Refusals {
        self.assembler.refusals()
    }

    /// Takes one hard bit at the caller's time `time`. Pages go to `sink`.
    pub fn push_bit(&mut self, bit: u8, time: f64, mut sink: impl FnMut(&Page)) {
        let asm = &mut self.assembler;
        self.deframer
            .push_bit(bit, |e| Self::route(asm, e, false, time, &mut sink));
    }

    /// Takes one soft bit (an LLR, positive means 0). Pages go to `sink`.
    pub fn push_llr(&mut self, llr: f32, time: f64, mut sink: impl FnMut(&Page)) {
        let asm = &mut self.assembler;
        self.deframer
            .push_llr(llr, |e| Self::route(asm, e, true, time, &mut sink));
    }

    fn route(
        asm: &mut PocsagAssembler,
        e: Event,
        soft: bool,
        time: f64,
        sink: &mut impl FnMut(&Page),
    ) {
        match e {
            Event::Word { slot, raw, rel } => {
                asm.push(slot, raw, soft.then_some(&rel), time, &mut *sink)
            }
            Event::BatchEnd { .. } => {}
            Event::End => asm.end(time, &mut *sink),
        }
    }

    /// The input ended (or the carrier dropped). The open page goes to `sink`.
    pub fn finish(&mut self, time: f64, sink: impl FnMut(&Page)) {
        self.deframer.reset();
        self.assembler.end(time, sink);
    }

    /// Forgets the transmission (for example after a retune). The voting history stays.
    pub fn reset(&mut self) {
        self.deframer.reset();
        self.assembler.reset();
    }
}

/// The message of a page to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Content<'a> {
    /// No message: the address word only.
    Tone,
    /// Numeric characters of the chosen set.
    Numeric(&'a str),
    /// 7-bit text (IA5). It ends with EOT (0x04).
    Alpha(&'a str),
}

/// A page to send.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PocsagTx<'a> {
    /// The RIC (21 bits).
    pub ric: u32,
    /// The function code (0 to 3). M.584-2 assigns 00 to numeric and 11 to text.
    pub function: u8,
    /// The message.
    pub content: Content<'a>,
}

/// The message bits of a page. Numeric messages are filled with spaces to whole words (clause 2.1). Text ends with EOT and
/// is filled with zero bits (clause 2.2).
fn message_bits(p: &PocsagTx, set: NumericSet, out: &mut Vec<u8>) -> Result<()> {
    out.clear();
    match p.content {
        Content::Tone => {}
        Content::Numeric(s) => {
            for c in s.chars() {
                let v = set.value(c).ok_or(Error::BadCharacter(c))?;
                push_lsb_first(out, v as u32, 4);
            }
            while !out.len().is_multiple_of(20) || out.is_empty() {
                push_lsb_first(out, 12, 4);
            }
        }
        Content::Alpha(s) => {
            for c in s.chars() {
                if !c.is_ascii() {
                    return Err(Error::BadCharacter(c));
                }
                push_lsb_first(out, c as u32, 7);
            }
            push_lsb_first(out, 4, 7);
            while !out.len().is_multiple_of(20) {
                out.push(0);
            }
        }
    }
    Ok(())
}

/// Appends the words of a transmission to `out`: batches of the sync word and 16 words. Each address word goes into the frame
/// that its RIC selects (clause 1.2). Message words follow their address. After a message at least one idle word comes before
/// the next address (clause 1.2). Idle words fill the rest. An error leaves `out` as it was.
pub fn encode_batches(pages: &[PocsagTx], set: NumericSet, out: &mut Vec<u32>) -> Result<()> {
    let mut slots: Vec<u32> = Vec::new();
    let mut bits = Vec::new();
    for p in pages {
        let addr = address_word(p.ric, p.function)?;
        message_bits(p, set, &mut bits)?;
        let target = ((p.ric & 7) * 2) as usize;
        if slots.len() % BATCH_WORDS > target {
            while !slots.len().is_multiple_of(BATCH_WORDS) {
                slots.push(IDLE);
            }
        }
        while slots.len() % BATCH_WORDS != target {
            slots.push(IDLE);
        }
        slots.push(addr);
        for chunk in bits.chunks(20) {
            let v = chunk.iter().fold(0u32, |a, &b| (a << 1) | b as u32);
            slots.push(message_word(v));
        }
        if !bits.is_empty() {
            slots.push(IDLE);
        }
    }
    while !slots.len().is_multiple_of(BATCH_WORDS) || slots.is_empty() {
        slots.push(IDLE);
    }
    out.reserve(slots.len() / BATCH_WORDS * (BATCH_WORDS + 1));
    for b in slots.chunks(BATCH_WORDS) {
        out.push(SYNC);
        out.extend_from_slice(b);
    }
    Ok(())
}

/// Appends the bits of a transmission to `out`: `preamble` bits of reversals (1, 0, 1 ...), then each word first bit first.
pub fn transmission_bits(words: &[u32], preamble: usize, out: &mut Vec<u8>) {
    out.extend((0..preamble).map(|i| (i % 2 == 0) as u8));
    for &w in words {
        out.extend((0..32).rev().map(|i| ((w >> i) & 1) as u8));
    }
}
