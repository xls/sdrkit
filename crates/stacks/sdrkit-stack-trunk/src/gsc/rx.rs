//! The GSC modem (`gsc.modem`): the paging front end, an FM discriminator, 24 000 samples/s, a moving mean of one 600 bit/s
//! bit, the upper and lower level means, the bit clock on the edges, the preamble and block searches while unlocked, and the
//! block reader while locked (start code, data block, address, activation word).

use super::code::{
    self, decode_soft, distance, hard, inverse, near_known, read_data_block, value, word1_index,
    PagerCode, BLOCK_BITS, MAX_DATA_BLOCKS, WORD1_AT, WORD2_AT,
};
use crate::front::{CarrierState, PagerFront};
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Decoder, Format, Item, Outbox, Port, Timed};
use std::f64::consts::TAU;

/// The working rate, Hz.
pub const RATE: f64 = 24_000.0;
/// Samples of one grid bit at the working rate.
pub const SPB: f64 = 40.0;
/// The samples of the moving mean.
const MEAN_LEN: usize = 40;
/// The ring of moving means, samples.
const RING: usize = 256;
/// The ring of soft grid bits.
const GRID: usize = 512;
/// A preamble is remembered this long, s.
pub const PREAMBLE_HOLD_S: f64 = 4.0;
/// A voice page closes after this long without its end word, s.
pub const VOICE_TIMER_S: f64 = 16.0;
/// Status items, every this many seconds.
const STATUS_EVERY_S: f64 = 0.25;

/// How a call was sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    /// A preamble in the sense of the start code.
    Single,
    /// A preamble in the opposite sense of the start code.
    Batch,
    /// No preamble was seen.
    NoPreamble,
}

impl CallKind {
    /// Its text.
    pub fn label(self) -> &'static str {
        match self {
            CallKind::Single => "single call",
            CallKind::Batch => "batch",
            CallKind::NoPreamble => "no preamble seen",
        }
    }
}

/// What a page carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    /// No data.
    Tone,
    /// Numeric data.
    Numeric,
    /// Alphanumeric data.
    Alpha,
    /// Tone and voice.
    Voice,
}

/// An address read from a block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Address {
    /// The pager code.
    pub code: PagerCode,
    /// The address function, 1 to 4.
    pub function: u8,
    /// The preamble index seen before it.
    pub preamble: Option<usize>,
    /// How the call was sent.
    pub call: CallKind,
    /// Bits corrected in the two words.
    pub corrected: u32,
    /// The quality of the block, dB.
    pub quality_db: f32,
}

/// A page read to its end.
#[derive(Clone, Debug, PartialEq)]
pub struct GscPage {
    /// What it carries.
    pub kind: PageKind,
    /// Its address.
    pub address: Address,
    /// The text of a data page.
    pub text: String,
    /// The data blocks.
    pub blocks: u32,
    /// The last block said that another would follow.
    pub more: bool,
    /// The activation word of a voice page.
    pub activation: Option<u16>,
}

/// How a voice page ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceEndBy {
    /// Its activation word came again.
    Activation,
    /// Another word of the same form (a deactivation word).
    Deactivation,
    /// The timer ran out.
    Timer,
}

/// The end of a voice page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceEnd {
    /// The address of the page.
    pub address: Address,
    /// The end word, if one came.
    pub word: Option<u16>,
    /// How it ended.
    pub by: VoiceEndBy,
    /// The length of the voice, s.
    pub length_s: f64,
}

/// What the modem measures.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GscStatus {
    /// Block lock.
    pub locked: bool,
    /// The spectrum is inverted (from the last start code).
    pub inverted: bool,
    /// The deviation, Hz.
    pub deviation_hz: f32,
    /// The carrier offset, Hz.
    pub offset_hz: f32,
    /// The sender's bit clock error, ppm.
    pub clock_ppm: f32,
    /// The quality of the last block read, dB.
    pub quality_db: Option<f32>,
    /// The preamble index remembered and whether it was a batch preamble.
    pub preamble: Option<(usize, Option<bool>)>,
}

/// What the modem passes on.
#[derive(Clone, Debug, PartialEq)]
pub enum GscHeard {
    /// The carrier state of the front end.
    Carrier(CarrierState),
    /// A preamble word: its index and whether it was received inverted.
    Preamble(usize, bool),
    /// A page.
    Page(GscPage),
    /// The end of a voice page.
    VoiceEnd(VoiceEnd),
    /// The measurements.
    Status(GscStatus),
}

impl Item for GscHeard {
    const KIND: &'static str = "gsc.heard";
}

/// A preamble remembered.
#[derive(Clone, Copy, Debug)]
struct Preamble {
    index: usize,
    inverted: bool,
    t: f64,
}

/// A page whose address was read.
#[derive(Clone, Debug)]
struct Waiting {
    address: Address,
    fields: Vec<u64>,
}

/// An open voice page.
#[derive(Clone, Copy, Debug)]
struct Voice {
    address: Address,
    word: u16,
    t: f64,
}

/// Block lock.
#[derive(Clone, Copy, Debug)]
struct Lock {
    /// The grid bit that ends the next block.
    end: u64,
    misses: u8,
    /// The preamble of the call, and whether it was a batch preamble.
    preamble: Option<(usize, bool)>,
}

/// The `gsc.modem` decoder.
pub struct GscModem {
    front: PagerFront,
    chan: Vec<Complex32>,
    states: Vec<CarrierState>,
    prev: Complex32,
    half: Option<f32>,
    window: [f32; MEAN_LEN],
    sum: f32,
    ring: [f32; RING],
    n: u64,
    next: f64,
    rate: f64,
    last_soft: f32,
    last_mid: f32,
    /// Bits in a row read on the edges of alternating bits.
    on_edges: u8,
    upper: f32,
    lower: f32,
    grid: [f32; GRID],
    gn: u64,
    /// The candidates of the last five positions, with the correlation of their ideal block: start code (its sense),
    /// address, end word.
    start_hist: [(f32, Option<bool>); 5],
    addr_hist: [(f32, Option<Address>); 5],
    end_hist: [(f32, Option<u16>); 5],
    inverted: bool,
    lock: Option<Lock>,
    preamble: Option<Preamble>,
    last_preamble_event: f64,
    pending_preamble: Option<(usize, bool)>,
    waiting: Option<Waiting>,
    voice: Option<Voice>,
    quality: Option<f32>,
    next_status: f64,
}

impl Default for GscModem {
    fn default() -> Self {
        GscModem {
            front: PagerFront::new(false),
            chan: Vec::new(),
            states: Vec::new(),
            prev: Complex32::new(0.0, 0.0),
            half: None,
            window: [0.0; MEAN_LEN],
            sum: 0.0,
            ring: [0.0; RING],
            n: 0,
            next: SPB,
            rate: 0.0,
            last_soft: 0.0,
            last_mid: 0.0,
            on_edges: 0,
            upper: 0.0,
            lower: 0.0,
            grid: [0.0; GRID],
            gn: 0,
            start_hist: [(f32::NEG_INFINITY, None); 5],
            addr_hist: [(f32::NEG_INFINITY, None); 5],
            end_hist: [(f32::NEG_INFINITY, None); 5],
            inverted: false,
            lock: None,
            preamble: None,
            last_preamble_event: f64::NEG_INFINITY,
            pending_preamble: None,
            waiting: None,
            voice: None,
            quality: None,
            next_status: 0.0,
        }
    }
}

/// Pushes `v` into a history of five, oldest first.
fn shift<T: Copy>(h: &mut [(f32, Option<T>); 5], v: (f32, Option<T>)) {
    h.copy_within(1.., 0);
    h[4] = v;
}

/// The candidate of the middle position when its ideal block correlates best of the five.
fn best_middle<T: Copy>(h: &[(f32, Option<T>); 5]) -> Option<T> {
    let (c, v) = h[2];
    let v = v?;
    h.iter().all(|&(o, _)| o <= c).then_some(v)
}

impl GscModem {
    fn now(&self) -> f64 {
        self.n as f64 / RATE
    }

    fn mean_at(&self, x: f64) -> f32 {
        let i = x.floor();
        let f = (x - i) as f32;
        let i = i as u64;
        let a = self.ring[(i as usize) % RING];
        let b = self.ring[((i + 1) as usize) % RING];
        a + (b - a) * f
    }

    fn centre(&self) -> f32 {
        0.5 * (self.upper + self.lower)
    }

    fn half_swing(&self) -> f32 {
        (0.5 * (self.upper - self.lower)).max(400.0)
    }

    fn g(&self, k: u64) -> f32 {
        self.grid[(k as usize) % GRID]
    }

    /// The soft 300 bit/s bits of a word from grid bit `at`, times `sign`.
    fn slow(&self, at: u64, sign: f32) -> [f32; 23] {
        std::array::from_fn(|k| sign * (self.g(at + 2 * k as u64) + self.g(at + 2 * k as u64 + 1)))
    }

    /// The transitions of the comma of the block that starts at `s`.
    fn transitions(&self, s: u64) -> u32 {
        let mut t = 0;
        let mut last = self.g(s) > 0.0;
        for k in 1..code::COMMA_BITS as u64 {
            let b = self.g(s + k) > 0.0;
            t += u32::from(b != last);
            last = b;
        }
        t
    }

    /// The start code distance of the block at `s` and the sense that gives it (true: inverted spectrum).
    fn start_score(&self, s: u64) -> (u32, bool) {
        let w1 = hard(&self.slow(s + WORD1_AT as u64, 1.0));
        let w2 = hard(&self.slow(s + WORD2_AT as u64, 1.0));
        let sc = code::start_word();
        let normal = distance(w1, sc) + distance(w2, inverse(sc));
        let inv = distance(w1, inverse(sc)) + distance(w2, sc);
        if inv < normal {
            (inv, true)
        } else {
            (normal, false)
        }
    }

    /// The correlation of the block at `s` with the ideal two-word block of `w1` and `w2` (in the sense `sign`).
    fn block_corr(&self, s: u64, w1: u32, w2: u32, sign: f32) -> f32 {
        let first = if w1 & 1 != 0 { 1.0 } else { -1.0 };
        let mut c = 0f32;
        for k in 0..code::COMMA_BITS as u64 {
            let want = if k % 2 == 0 { first } else { -first };
            c += want * self.g(s + k);
        }
        let slow = |w: u32, at: u64| {
            let mut c = 0f32;
            for k in 0..23u64 {
                let want = if w >> k & 1 != 0 { 1.0 } else { -1.0 };
                c += want * (self.g(at + 2 * k) + self.g(at + 2 * k + 1));
            }
            c
        };
        let sep = if w2 & 1 != 0 { -1.0 } else { 1.0 };
        c += sep * self.g(s + 74);
        sign * (c + slow(w1, s + WORD1_AT as u64) + slow(w2, s + WORD2_AT as u64))
    }

    /// The comma rules: the first comma bit equals the first bit of Word 1, the separator differs from the first bit of Word 2.
    fn comma_rules(&self, s: u64) -> bool {
        let first = self.g(s) > 0.0;
        let w1 = self.g(s + 28) + self.g(s + 29) > 0.0;
        let sep = self.g(s + 74) > 0.0;
        let w2 = self.g(s + 75) + self.g(s + 76) > 0.0;
        first == w1 && sep != w2
    }

    /// The preamble remembered now: its index and whether it was a batch preamble (`None` before a start code).
    fn preamble_now(&self) -> Option<(usize, Option<bool>)> {
        if let Some((i, batch)) = self.lock.and_then(|l| l.preamble) {
            return Some((i, Some(batch)));
        }
        self.preamble
            .filter(|p| self.now() - p.t <= PREAMBLE_HOLD_S)
            .map(|p| (p.index, None))
    }

    /// An address in the block at `s`: Word 1 within `max1` bits of the table, Word 2 with at most `max2` corrections, the
    /// comma rules. Returns the correlation of its ideal block and the address.
    fn address(&self, s: u64, max1: u32, max2: u32) -> Option<(f32, Address)> {
        if !self.comma_rules(s) {
            return None;
        }
        let sign = if self.inverted { -1.0 } else { 1.0 };
        let w1 = hard(&self.slow(s + WORD1_AT as u64, sign));
        let (index, inv1, d1) = word1_index(w1, max1)?;
        let (cw, changed) = decode_soft(&self.slow(s + WORD2_AT as u64, sign));
        if changed > max2 {
            return None;
        }
        let data = value(cw);
        if data == 0 || data == 0xFFF {
            return None;
        }
        let inv2 = data >= 2048;
        let w2 = if inv2 { !data & 0xFFF } else { data };
        let (preamble, call) = match self.preamble_now() {
            Some((i, Some(batch))) => (
                Some(i),
                if batch {
                    CallKind::Batch
                } else {
                    CallKind::Single
                },
            ),
            Some((i, None)) => (Some(i), CallKind::Single),
            None => (None, CallKind::NoPreamble),
        };
        let code = PagerCode::from_words(index, w2, preamble)?;
        let table = code::word1_words()[index];
        let ideal1 = if inv1 { inverse(table) } else { table };
        Some((
            self.block_corr(s, ideal1, cw, sign),
            Address {
                code,
                function: code::function(inv1, inv2),
                preamble,
                call,
                corrected: d1 + changed,
                quality_db: self.block_quality(s),
            },
        ))
    }

    /// An activation (or deactivation) word in the block at `s`: the two words decoded as one from their soft difference,
    /// both within 6 bits in all, and not near a known word. Returns the correlation of its ideal block and the value.
    fn end_word(&self, s: u64) -> Option<(f32, u16)> {
        let sign = if self.inverted { -1.0 } else { 1.0 };
        let a = self.slow(s + WORD1_AT as u64, sign);
        let b = self.slow(s + WORD2_AT as u64, sign);
        let diff: [f32; 23] = std::array::from_fn(|k| a[k] - b[k]);
        let (cw, _) = decode_soft(&diff);
        let d = distance(hard(&a), cw) + distance(hard(&b), inverse(cw));
        if d > 6 || near_known(cw, 4) {
            return None;
        }
        Some((self.block_corr(s, cw, inverse(cw), sign), value(cw)))
    }

    /// The quality of the block at `s`, dB: the mean of |soft| squared over its variance.
    fn block_quality(&self, s: u64) -> f32 {
        let mut m = 0f32;
        let mut m2 = 0f32;
        for k in 0..BLOCK_BITS as u64 {
            let a = self.g(s + k).abs();
            m += a;
            m2 += a * a;
        }
        let n = BLOCK_BITS as f32;
        m /= n;
        let v = (m2 / n - m * m).max(1e-6);
        10.0 * (m * m / v).log10()
    }

    /// Ends the waiting page as a tone page or a data page.
    fn finish_waiting(&mut self, out: &mut Vec<Timed<GscHeard>>) {
        let Some(w) = self.waiting.take() else {
            return;
        };
        let page = if w.fields.is_empty() {
            GscPage {
                kind: PageKind::Tone,
                address: w.address,
                text: String::new(),
                blocks: 0,
                more: false,
                activation: None,
            }
        } else {
            let (numeric, text) = code::choose_reading(&w.fields);
            GscPage {
                kind: if numeric {
                    PageKind::Numeric
                } else {
                    PageKind::Alpha
                },
                address: w.address,
                text,
                blocks: w.fields.len() as u32,
                more: w.fields.last().is_some_and(|&p| code::continues(p)),
                activation: None,
            }
        };
        out.push(Timed {
            time: self.now(),
            value: GscHeard::Page(page),
        });
    }

    fn end_voice(&mut self, word: Option<u16>, out: &mut Vec<Timed<GscHeard>>) {
        let Some(v) = self.voice.take() else {
            return;
        };
        let by = match word {
            None => VoiceEndBy::Timer,
            Some(w) if w == v.word => VoiceEndBy::Activation,
            Some(_) => VoiceEndBy::Deactivation,
        };
        let t = self.now();
        out.push(Timed {
            time: t,
            value: GscHeard::VoiceEnd(VoiceEnd {
                address: v.address,
                word,
                by,
                length_s: if word.is_some() {
                    t - v.t
                } else {
                    VOICE_TIMER_S
                },
            }),
        });
    }

    fn unlock(&mut self, out: &mut Vec<Timed<GscHeard>>) {
        self.finish_waiting(out);
        self.lock = None;
        self.start_hist = [(f32::NEG_INFINITY, None); 5];
        self.addr_hist = [(f32::NEG_INFINITY, None); 5];
        self.end_hist = [(f32::NEG_INFINITY, None); 5];
    }

    /// Locks on a start code whose block ends at grid bit `end`.
    fn lock_start(&mut self, end: u64, inverted: bool, out: &mut Vec<Timed<GscHeard>>) {
        self.finish_waiting(out);
        self.inverted = inverted;
        let preamble = self
            .preamble
            .filter(|p| self.now() - p.t <= PREAMBLE_HOLD_S)
            .map(|p| (p.index, p.inverted != inverted));
        self.lock = Some(Lock {
            end: end + BLOCK_BITS as u64,
            misses: 0,
            preamble,
        });
        self.start_hist = [(f32::NEG_INFINITY, None); 5];
        self.addr_hist = [(f32::NEG_INFINITY, None); 5];
    }

    /// The searches of an unlocked modem after grid bit `e`. A candidate block is taken two bits later, when its ideal
    /// block correlates best of the five positions around it (a block one 300 bit/s bit early or late can also decode).
    fn search(&mut self, out: &mut Vec<Timed<GscHeard>>) {
        let e = self.gn;
        if e < 2 * BLOCK_BITS as u64 {
            return;
        }
        self.search_preamble(e);
        let s = e + 1 - BLOCK_BITS as u64;
        let t = self.transitions(s);
        let none = f32::NEG_INFINITY;
        // Start code.
        let cand = if t >= 14 {
            let (d, inv) = self.start_score(s);
            let sc = code::start_word();
            let sign = if inv { -1.0 } else { 1.0 };
            if d <= 6 {
                (self.block_corr(s, sc, inverse(sc), sign), Some(inv))
            } else {
                (none, None)
            }
        } else {
            (none, None)
        };
        shift(&mut self.start_hist, cand);
        if let Some(inv) = best_middle(&self.start_hist) {
            self.lock_start(e - 2, inv, out);
            return;
        }
        // An address without a preamble.
        let cand = if t >= 24 {
            self.address(s, 2, 2)
                .map_or((none, None), |(c, a)| (c, Some(a)))
        } else {
            (none, None)
        };
        shift(&mut self.addr_hist, cand);
        if let Some(addr) = best_middle(&self.addr_hist) {
            self.finish_waiting(out);
            self.waiting = Some(Waiting {
                address: addr,
                fields: Vec::new(),
            });
            self.start_hist = [(none, None); 5];
            self.addr_hist = [(none, None); 5];
            self.lock = Some(Lock {
                end: e - 2 + BLOCK_BITS as u64,
                misses: 0,
                preamble: None,
            });
            return;
        }
        // The end word of an open voice page.
        let cand = if self.voice.is_some() && t >= 20 {
            self.end_word(s).map_or((none, None), |(c, w)| (c, Some(w)))
        } else {
            (none, None)
        };
        shift(&mut self.end_hist, cand);
        if let Some(w) = best_middle(&self.end_hist) {
            self.end_hist = [(none, None); 5];
            self.end_voice(Some(w), out);
        }
    }

    /// The preamble search after grid bit `e`, both senses.
    fn search_preamble(&mut self, e: u64) {
        let w = hard(&self.slow(e - 45, 1.0));
        let words = code::preamble_words();
        for (i, &p) in words.iter().enumerate() {
            let d = distance(w, p);
            let inv = if d <= 2 {
                false
            } else if 23 - d <= 2 {
                true
            } else {
                continue;
            };
            let earlier = hard(&self.slow(e - 91, 1.0));
            let want = if inv { inverse(p) } else { p };
            if distance(earlier, want) > 3 {
                continue;
            }
            let t = self.now();
            self.preamble = Some(Preamble {
                index: i,
                inverted: inv,
                t,
            });
            if t - self.last_preamble_event >= 1.0 {
                self.last_preamble_event = t;
                self.pending_preamble = Some((i, inv));
            }
            return;
        }
    }

    /// Reads the block that ends at the present grid bit while locked.
    fn read_block(&mut self, out: &mut Vec<Timed<GscHeard>>) {
        let Some(mut lock) = self.lock else {
            return;
        };
        let e = self.gn;
        let s = e + 1 - BLOCK_BITS as u64;
        lock.end = e + BLOCK_BITS as u64;
        self.lock = Some(lock);
        let t = self.transitions(s);
        self.quality = Some(self.block_quality(s));
        // 1. A start code of the same sense.
        if t >= 10 {
            let (d, inv) = self.start_score(s);
            if d <= 8 && inv == self.inverted {
                self.finish_waiting(out);
                self.set_misses(0);
                return;
            }
        }
        // 2. A data block of the waiting page.
        if self.waiting.is_some() {
            let sign = if self.inverted { -1.0 } else { 1.0 };
            let soft: [f32; BLOCK_BITS] = std::array::from_fn(|k| sign * self.g(s + k as u64));
            if let Some(p) =
                read_data_block(&soft, 2).and_then(|(info, _)| code::block_field(&info))
            {
                let done = {
                    let w = self.waiting.as_mut().expect("a waiting page");
                    w.fields.push(p);
                    !code::continues(p) || w.fields.len() >= MAX_DATA_BLOCKS
                };
                if done {
                    self.finish_waiting(out);
                }
                self.set_misses(0);
                return;
            }
        }
        // 3. An address.
        if t >= 20 {
            if let Some((_, a)) = self.address(s, 3, 3) {
                self.finish_waiting(out);
                self.waiting = Some(Waiting {
                    address: a,
                    fields: Vec::new(),
                });
                self.set_misses(0);
                return;
            }
        }
        // 4. An activation or deactivation word.
        if t >= 16 {
            if let Some((_, word)) = self.end_word(s) {
                let opens = self.waiting.as_ref().is_some_and(|w| w.fields.is_empty());
                if opens {
                    let w = self.waiting.take().expect("a waiting page");
                    out.push(Timed {
                        time: self.now(),
                        value: GscHeard::Page(GscPage {
                            kind: PageKind::Voice,
                            address: w.address,
                            text: String::new(),
                            blocks: 0,
                            more: false,
                            activation: Some(word),
                        }),
                    });
                    self.voice = Some(Voice {
                        address: w.address,
                        word,
                        t: self.now(),
                    });
                    self.set_misses(0);
                    return;
                }
                if self.voice.is_some() {
                    self.end_voice(Some(word), out);
                    self.set_misses(0);
                    return;
                }
            }
        }
        self.finish_waiting(out);
        let misses = self.lock.map_or(0, |l| l.misses) + 1;
        if misses >= 2 {
            self.unlock(out);
        } else {
            self.set_misses(misses);
        }
    }

    fn set_misses(&mut self, m: u8) {
        if let Some(l) = self.lock.as_mut() {
            l.misses = m;
        }
    }

    /// One grid bit read at the clock.
    fn bit(&mut self, out: &mut Vec<Timed<GscHeard>>) {
        let x = self.next;
        let v = self.mean_at(x);
        let h = self.mean_at(x - 0.5 * SPB);
        let centre = self.centre();
        let half = self.half_swing();
        let soft = ((v - centre) / half).clamp(-2.0, 2.0);
        let mid = ((h - centre) / half).clamp(-2.0, 2.0);
        let locked = self.lock.is_some();
        let k = if locked { 0.01 } else { 0.04 };
        // A value near the centre (voice, a bare carrier, a read between two bits) moves neither mean.
        if soft > 0.3 {
            self.upper += k * (v - self.upper);
        } else if soft < -0.3 {
            self.lower += k * (v - self.lower);
        }
        let mut adjust = 0.0;
        if (soft > 0.0) != (self.last_soft > 0.0) && (soft - self.last_soft).abs() > 0.8 {
            let err = f64::from(((soft - self.last_soft) * mid).clamp(-2.0, 2.0));
            adjust = -0.04 * err * SPB;
            self.rate = (self.rate - 4e-5 * err).clamp(-0.004, 0.004);
        }
        if !locked {
            self.rate *= 0.9995;
        }
        // Read between the bits of an alternating pattern, the means read at the clock change little from bit to bit while
        // those half a bit earlier swing from one level to the other, and the edge rule sees no edge: three such bits in a
        // row move the clock by half a bit.
        let swing = (mid - self.last_mid).abs();
        if swing > 1.5 && (soft - self.last_soft).abs() < 0.4 * swing {
            self.on_edges += 1;
            if self.on_edges >= 3 {
                adjust += 0.5 * SPB;
                self.on_edges = 0;
            }
        } else {
            self.on_edges = 0;
        }
        self.last_soft = soft;
        self.last_mid = mid;
        self.next = x + SPB * (1.0 + self.rate) + adjust;
        self.gn += 1;
        self.grid[(self.gn as usize) % GRID] = soft;
        if let Some(l) = self.lock {
            if self.gn >= l.end {
                self.read_block(out);
            }
        } else {
            self.search(out);
        }
        if let Some(v) = self.voice {
            if self.now() - v.t > VOICE_TIMER_S {
                self.end_voice(None, out);
            }
        }
        if let Some((i, inv)) = self.pending_preamble.take() {
            out.push(Timed {
                time: self.now(),
                value: GscHeard::Preamble(i, inv),
            });
        }
    }

    fn sample(&mut self, d: f32, out: &mut Vec<Timed<GscHeard>>) {
        let slot = (self.n as usize) % MEAN_LEN;
        self.sum += d - self.window[slot];
        self.window[slot] = d;
        if self.n.is_multiple_of(4096) {
            // The running sum is summed again now and then, so rounding does not build up.
            self.sum = self.window.iter().sum();
        }
        self.ring[(self.n as usize) % RING] = self.sum / MEAN_LEN as f32;
        while self.next + 1.0 <= self.n as f64 {
            if self.next < self.n as f64 - (RING / 2) as f64 {
                // Far behind (a gap in the stream): start the clock again here.
                self.next = self.n as f64;
                break;
            }
            self.bit(out);
        }
        self.n += 1;
    }

    fn status(&self) -> GscStatus {
        GscStatus {
            locked: self.lock.is_some(),
            inverted: self.inverted,
            deviation_hz: self.half_swing(),
            offset_hz: self.centre() + self.front.afc_hz() as f32,
            clock_ppm: (-self.rate * 1e6) as f32,
            quality_db: self.quality,
            preamble: self.preamble_now(),
        }
    }

    /// Drops what is half read (a retune).
    fn reset_sync(&mut self) {
        let front = std::mem::replace(&mut self.front, PagerFront::new(false));
        *self = GscModem {
            front,
            ..GscModem::default()
        };
    }
}

impl Decoder for GscModem {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "gsc.modem"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<GscHeard>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        *self = GscModem::default();
        self.front.setup(f.rate, f.signal_offset_hz())?;
        Ok(vec![crate::typed_format::<GscHeard>(f)])
    }
    fn retune(&mut self, f: &Format) {
        // A new channel: the front end starts again, the block half read is dropped.
        self.reset_sync();
        let _ = self.front.setup(f.rate, f.signal_offset_hz());
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let mut chan = std::mem::take(&mut self.chan);
        chan.clear();
        let mut states = std::mem::take(&mut self.states);
        states.clear();
        self.front.run(x, &mut chan, |_, s| states.push(s));
        let items = out.items::<GscHeard>(0);
        if let Some(s) = states.last() {
            items.push(Timed {
                time: self.now(),
                value: GscHeard::Carrier(*s),
            });
        }
        let k = (48_000.0 / TAU) as f32;
        for &z in &chan {
            let d = z * self.prev.conj();
            self.prev = z;
            let hz = atan2(d.im, d.re) * k;
            match self.half.take() {
                None => self.half = Some(hz),
                Some(a) => self.sample(0.5 * (a + hz), items),
            }
        }
        if self.now() >= self.next_status {
            self.next_status = self.now() + STATUS_EVERY_S;
            items.push(Timed {
                time: self.now(),
                value: GscHeard::Status(self.status()),
            });
        }
        self.chan = chan;
        self.states = states;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_of_the_start_code_scores_zero() {
        let mut m = GscModem::default();
        let mut bits = Vec::new();
        let sc = code::encode(code::START_CODE);
        code::push_two_words(sc, inverse(sc), &mut bits);
        for (k, &b) in bits.iter().enumerate() {
            m.grid[(k + 1) % GRID] = if b != 0 { 1.0 } else { -1.0 };
        }
        assert_eq!(m.start_score(1), (0, false));
        assert_eq!(m.transitions(1), 27);
        assert!(m.comma_rules(1));
    }
}
