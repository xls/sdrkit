//! The ERMES modem (`ermes.modem`): the paging front end, an FM discriminator, 25 000 samples/s (8 a symbol), a one-symbol
//! moving mean, the preamble and sync search on all 8 sample phases, early-late symbol tracking, and the batch reader.
//!
//! Standard: ETS 300 133-4.

use super::code::{
    self, deinterleave, is_special, level_bits, level_index, read_headers, Category, SystemInfo,
    APT, CODEBLOCK, DELIMITER, PREAMBLE, PREAMBLE_MEAN_HZ, SYNC, WORD_SYMBOLS,
};
use crate::front::{CarrierState, PagerFront};
use num_complex::Complex32;
use sdrkit_algo_dsp::approx::atan2;
use sdrkit_algo_dsp::resample::RationalResampler;
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Decoder, Format, Item, Outbox, Port, Timed};
use std::f64::consts::TAU;

/// The working rate, Hz.
pub const RATE: f64 = 25_000.0;
/// Samples a symbol at [`RATE`].
pub const SPS: usize = 8;
/// The symbols the search compares: preamble and sync.
const SEARCH_SYMBOLS: usize = 2 * WORD_SYMBOLS;
/// The least mean |level| of a search hit, Hz.
const SEARCH_MIN_HZ: f32 = 1800.0;
/// The most differing symbols of a search hit.
const SEARCH_MAX_ERRORS: usize = 2;
/// The early-late threshold, Hz.
const TRACK_HZ: f32 = 120.0;
/// Symbols the early-late error is averaged over.
const TRACK_SYMBOLS: usize = 16;
/// The most bad address words of a batch.
const MAX_BAD_ADDRESSES: usize = 4;
/// The ring of moving means, samples.
const RING: usize = 512;

/// One message of a batch.
#[derive(Clone, Debug, PartialEq)]
pub struct ErmesMessage {
    /// The 22-bit local address.
    pub address: u32,
    /// The category.
    pub category: Category,
    /// The additional-information flag.
    pub aii: bool,
    /// The text.
    pub text: String,
}

/// A batch read to its end.
#[derive(Clone, Debug, PartialEq)]
pub struct ErmesBatch {
    /// The system information.
    pub si: SystemInfo,
    /// The initial addresses, 18 bits each.
    pub addresses: Vec<u32>,
    /// The messages.
    pub messages: Vec<ErmesMessage>,
    /// Code words repaired.
    pub repaired: u32,
}

/// What the modem passes on.
#[derive(Clone, Debug, PartialEq)]
pub enum ErmesHeard {
    /// A preamble and sync.
    Sync,
    /// A good batch.
    Batch(ErmesBatch),
    /// A batch dropped.
    Bad,
    /// The carrier state of the front end.
    Carrier(CarrierState),
}

impl Item for ErmesHeard {
    const KIND: &'static str = "ermes.heard";
}

/// The preamble and sync as level indices, and the level of each.
fn search_pattern() -> [usize; SEARCH_SYMBOLS] {
    let p = code::word_levels(PREAMBLE);
    let s = code::word_levels(SYNC);
    std::array::from_fn(|i| {
        if i < WORD_SYMBOLS {
            p[i]
        } else {
            s[i - WORD_SYMBOLS]
        }
    })
}

/// A batch being read.
struct Reading {
    /// The sample of the next symbol.
    next: u64,
    /// The carrier offset removed, Hz.
    offset: f32,
    /// The symbols of the word being read.
    word: u32,
    symbols: usize,
    /// The words read since the sync (the sync is word 0).
    words: Vec<u32>,
    /// The early-late errors of the last symbols.
    track: [f32; TRACK_SYMBOLS],
    track_at: usize,
}

/// The `ermes.modem` decoder.
pub struct ErmesModem {
    front: PagerFront,
    resampler: Option<RationalResampler<f32>>,
    chan: Vec<Complex32>,
    disc: Vec<f32>,
    worked: Vec<f32>,
    states: Vec<CarrierState>,
    prev: Complex32,
    /// The last 8 discriminator values and their sum.
    window: [f32; SPS],
    sum: f32,
    /// The moving means, by sample.
    ring: Vec<f32>,
    n: u64,
    pattern: [usize; SEARCH_SYMBOLS],
    /// The first sample of a search hit and the best hit since: (errors, -mean, sample, offset).
    hit: Option<(u64, (usize, f32, u64, f32))>,
    reading: Option<Reading>,
}

impl Default for ErmesModem {
    fn default() -> Self {
        ErmesModem {
            front: PagerFront::new(false),
            resampler: None,
            chan: Vec::new(),
            disc: Vec::new(),
            worked: Vec::new(),
            states: Vec::new(),
            prev: Complex32::new(0.0, 0.0),
            window: [0.0; SPS],
            sum: 0.0,
            ring: vec![0.0; RING],
            n: 0,
            pattern: search_pattern(),
            hit: None,
            reading: None,
        }
    }
}

impl ErmesModem {
    fn at(&self, m: u64) -> f32 {
        self.ring[(m as usize) % RING]
    }

    /// The search at sample `n`: the last 30 symbols of this sample's phase against the preamble and sync.
    fn search(&mut self, n: u64) {
        let back = (SEARCH_SYMBOLS as u64 - 1) * SPS as u64;
        if n < back + 16 {
            return;
        }
        let first = n - back;
        let mut mean = 0.0f32;
        for k in 0..WORD_SYMBOLS {
            mean += self.at(first + (k * SPS) as u64);
        }
        let offset = mean / WORD_SYMBOLS as f32 - PREAMBLE_MEAN_HZ as f32;
        let mut errors = 0;
        let mut size = 0.0f32;
        for k in 0..SEARCH_SYMBOLS {
            let v = self.at(first + (k * SPS) as u64) - offset;
            size += v.abs();
            if level_index(v) != self.pattern[k] {
                errors += 1;
                if errors > SEARCH_MAX_ERRORS {
                    return;
                }
            }
        }
        let size = size / SEARCH_SYMBOLS as f32;
        if size <= SEARCH_MIN_HZ {
            return;
        }
        let cand = (errors, -size, n, offset);
        match self.hit.as_mut() {
            None => self.hit = Some((n, cand)),
            Some((_, best)) => {
                if (cand.0, cand.1) < (best.0, best.1) {
                    *best = cand;
                }
            }
        }
    }

    /// The symbol at `m`, with the early-late error.
    fn symbol(&mut self, out: &mut Vec<Timed<ErmesHeard>>) {
        let Some(r) = self.reading.as_mut() else {
            return;
        };
        let m = r.next;
        let v = self.ring[(m as usize) % RING] - r.offset;
        let late = self.ring[((m + 2) as usize) % RING] - r.offset;
        let early = self.ring[((m - 2) as usize) % RING] - r.offset;
        let i = level_index(v);
        r.word = (r.word << 2) | level_bits(i);
        r.symbols += 1;
        r.track[r.track_at % TRACK_SYMBOLS] = (late - early) * v.signum();
        r.track_at += 1;
        let mut step = SPS as u64;
        if r.track_at >= TRACK_SYMBOLS {
            let e = r.track.iter().sum::<f32>() / TRACK_SYMBOLS as f32;
            if e > TRACK_HZ {
                step += 1;
                r.track = [0.0; TRACK_SYMBOLS];
                r.track_at = 0;
            } else if e < -TRACK_HZ {
                step -= 1;
                r.track = [0.0; TRACK_SYMBOLS];
                r.track_at = 0;
            }
        }
        r.next = m + step;
        if r.symbols == WORD_SYMBOLS {
            let w = r.word & 0x3FFF_FFFF;
            r.word = 0;
            r.symbols = 0;
            r.words.push(w);
            self.word_done(out);
        }
    }

    /// A word ended: the batch is read once its words are in, or dropped early when SI 1 or SI 2 fails.
    fn word_done(&mut self, out: &mut Vec<Timed<ErmesHeard>>) {
        let Some(r) = self.reading.as_mut() else {
            return;
        };
        let k = r.words.len();
        if k < 2 {
            return;
        }
        let Some((si2, _)) =
            code::decode(r.words[1]).filter(|_| code::decode(r.words[0]).is_some())
        else {
            self.end(None, out);
            return;
        };
        let total = if (si2 & 0xF) as u8 == code::LAST_BATCH {
            code::LAST_BATCH_WORDS
        } else {
            code::BATCH_WORDS
        };
        // The words after the preamble and the sync.
        if k >= total - 2 {
            let words = std::mem::take(&mut r.words);
            let batch = read_batch(&words);
            self.end(Some(batch), out);
        }
    }

    fn end(&mut self, batch: Option<Option<ErmesBatch>>, out: &mut Vec<Timed<ErmesHeard>>) {
        let t = self.n as f64 / RATE;
        self.reading = None;
        self.front.set_busy(false);
        let value = match batch {
            Some(Some(b)) => ErmesHeard::Batch(b),
            _ => ErmesHeard::Bad,
        };
        out.push(Timed { time: t, value });
    }

    fn sample(&mut self, d: f32, out: &mut Vec<Timed<ErmesHeard>>) {
        let slot = (self.n as usize) % SPS;
        self.sum += d - self.window[slot];
        self.window[slot] = d;
        let mean = self.sum / SPS as f32;
        let n = self.n;
        self.ring[(n as usize) % RING] = mean;
        if n.is_multiple_of(4096) {
            // The running sum is summed again now and then, so rounding does not build up.
            self.sum = self.window.iter().sum();
        }
        match self.reading.as_ref() {
            Some(r) => {
                if n >= r.next + 2 {
                    self.symbol(out);
                }
            }
            None => {
                self.search(n);
                if let Some((start, best)) = self.hit {
                    if n >= start + SPS as u64 {
                        self.hit = None;
                        let (_, _, at, offset) = best;
                        self.reading = Some(Reading {
                            next: at + SPS as u64,
                            offset,
                            word: 0,
                            symbols: 0,
                            words: Vec::with_capacity(code::LAST_BATCH_WORDS),
                            track: [0.0; TRACK_SYMBOLS],
                            track_at: 0,
                        });
                        self.front.set_busy(true);
                        out.push(Timed {
                            time: n as f64 / RATE,
                            value: ErmesHeard::Sync,
                        });
                    }
                }
            }
        }
        self.n += 1;
    }

    /// Drops what is half read (a retune).
    fn reset_sync(&mut self) {
        self.reading = None;
        self.hit = None;
        self.front.set_busy(false);
    }
}

/// Reads the words of a batch after the sync (SI 1 first). `None` when the batch is dropped.
pub fn read_batch(words: &[u32]) -> Option<ErmesBatch> {
    let mut repaired = 0u32;
    let mut info = |w: u32| {
        code::decode(w).map(|(i, n)| {
            repaired += u32::from(n);
            i
        })
    };
    let si = SystemInfo::from_words([info(words[0])?, info(words[1])?, info(*words.get(2)?)?]);
    // Word k of the count from the sync is words[k - 1].
    let mut addresses = Vec::new();
    let mut bad = 0;
    let mut k = 4;
    let apt = loop {
        let w = *words.get(k - 1)?;
        if is_special(w, APT) {
            break k;
        }
        match info(w) {
            Some(a) => addresses.push(a),
            None => {
                bad += 1;
                if bad > MAX_BAD_ADDRESSES {
                    return None;
                }
            }
        }
        k += 1;
    };
    let start = (apt / CODEBLOCK + 1) * CODEBLOCK;
    let mut reader = MessageReader::default();
    let mut c = start;
    while c + CODEBLOCK - 1 <= words.len() {
        let sent: [u32; CODEBLOCK] = std::array::from_fn(|i| words[c - 1 + i]);
        for row in deinterleave(&sent) {
            reader.row(row, &mut repaired);
        }
        c += CODEBLOCK;
    }
    reader.finish();
    Some(ErmesBatch {
        si,
        addresses,
        messages: reader.messages,
        repaired,
    })
}

/// Where the message reader is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum At {
    /// Before the first delimiter.
    #[default]
    Start,
    /// After a delimiter: a header word 1 or another delimiter.
    Header1,
    /// After header word 1.
    Header2,
    /// In the data words.
    Data,
    /// A row failed: the rest of the message is skipped.
    Skip,
}

/// Reads the rows of the message partition.
#[derive(Default)]
struct MessageReader {
    at: At,
    h1: u32,
    head: Option<(u32, Category, bool)>,
    data: Vec<u32>,
    messages: Vec<ErmesMessage>,
}

impl MessageReader {
    fn row(&mut self, row: u32, repaired: &mut u32) {
        if is_special(row, DELIMITER) {
            self.finish();
            self.at = At::Header1;
            return;
        }
        if matches!(self.at, At::Start | At::Skip) {
            return;
        }
        let Some((info, n)) = code::decode(row) else {
            self.head = None;
            self.data.clear();
            self.at = At::Skip;
            return;
        };
        *repaired += u32::from(n);
        match self.at {
            At::Header1 => {
                self.h1 = info;
                self.at = At::Header2;
            }
            At::Header2 => {
                self.head = Some(read_headers(self.h1, info));
                self.data.clear();
                self.at = At::Data;
            }
            At::Data => self.data.push(info),
            At::Start | At::Skip => {}
        }
    }

    /// Ends the open message.
    fn finish(&mut self) {
        if let Some((address, category, aii)) = self.head.take() {
            let text = if aii {
                "(additional information, not read)".to_string()
            } else {
                code::message_text(category, &self.data)
            };
            self.messages.push(ErmesMessage {
                address,
                category,
                aii,
                text,
            });
        }
        self.data.clear();
    }
}

impl Decoder for ErmesModem {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "ermes.modem"
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<ErmesHeard>("out")]
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        *self = ErmesModem::default();
        self.front.setup(f.rate, f.signal_offset_hz())?;
        self.resampler = Some(RationalResampler::with_spec(25, 48, 0.2, 0.35, 60.0)?);
        Ok(vec![crate::typed_format::<ErmesHeard>(f)])
    }
    fn retune(&mut self, f: &Format) {
        // A new channel: the front end starts again, the batch half read is dropped.
        let _ = self.front.setup(f.rate, f.signal_offset_hz());
        self.reset_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let mut chan = std::mem::take(&mut self.chan);
        chan.clear();
        let mut states = std::mem::take(&mut self.states);
        states.clear();
        self.front.run(x, &mut chan, |_, s| states.push(s));
        let k = (48_000.0 / TAU) as f32;
        let mut disc = std::mem::take(&mut self.disc);
        disc.clear();
        for &z in &chan {
            let d = z * self.prev.conj();
            self.prev = z;
            disc.push(atan2(d.im, d.re) * k);
        }
        let mut worked = std::mem::take(&mut self.worked);
        worked.clear();
        if let Some(r) = self.resampler.as_mut() {
            r.process(&disc, &mut worked);
        }
        let items = out.items::<ErmesHeard>(0);
        if let Some(s) = states.last() {
            items.push(Timed {
                time: self.n as f64 / RATE,
                value: ErmesHeard::Carrier(*s),
            });
        }
        for &v in &worked {
            self.sample(v, items);
        }
        self.chan = chan;
        self.disc = disc;
        self.worked = worked;
        self.states = states;
    }
}
