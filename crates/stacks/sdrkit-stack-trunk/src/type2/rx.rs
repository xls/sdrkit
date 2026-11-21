//! `type2.osw`: the 3600 bit/s words of the control channel from the discriminator at 28 800 Hz (8 samples a bit): a
//! low-pass at 2000 Hz and a mean over 5 samples, the eye tracker and slicer, the sync search on each sample phase, the soft
//! decoding of each word, and the locked word clock that follows the best alignment of each word. No public document confirms the word layout.

use super::code::{Decoded, WordDecoder, SYNC, WORD_BITS};
use crate::front::{Eye, Level};
use crate::{typed_format, typed_port};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_core::{ConfigError, Values};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Item, Port, Timed};

/// The working rate, Hz.
pub const WORK_RATE: f64 = 28_800.0;
/// The channel half-width of the front end, Hz.
pub const HALF_WIDTH_HZ: f64 = 5000.0;
/// The carrier follow-up searches this far from the channel centre, Hz.
pub const SEARCH_HZ: f64 = 9000.0;
/// The carrier follow-up corrects at most this much, Hz.
pub const MAX_CORRECTION_HZ: f64 = 20_000.0;
/// Samples a bit.
pub const SPB: u64 = 8;
/// The cut-off of the low-pass on the discriminator, Hz.
const LOWPASS_HZ: f64 = 2000.0;
/// The soft value is the mean of this many low-passed samples.
const MEAN_SAMPLES: usize = 5;
/// Samples a word.
pub const FRAME: u64 = SPB * WORD_BITS as u64;
/// The least data swing of a search, Hz.
const MIN_SWING_HZ: f32 = 250.0;
/// The most candidates the search keeps.
const MAX_CANDIDATES: usize = 32;
/// Bad words in a row that drop the lock.
const MAX_BAD: u32 = 6;
/// The ring of soft values, samples (a power of two above a word and a half).
const RING: usize = 2048;

/// What `type2.osw` passes on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OswItem {
    /// A good word.
    Good(Decoded),
    /// A word expected and not read.
    Lost,
    /// The level of the channel, dBFS.
    Level(f32),
    /// The state of the word clock: locked, inverted polarity, data swing (Hz), SNR of the good words (dB).
    State {
        /// The word clock is locked.
        locked: bool,
        /// The polarity is inverted.
        inverted: bool,
        /// The data swing, Hz.
        swing_hz: f32,
        /// The SNR of the eye of the good words, dB.
        snr_db: Option<f32>,
    },
}

impl Item for OswItem {
    const KIND: &'static str = "type2.osw";
}

/// A good word found by the search.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    end: u64,
    inverted: bool,
    word: Decoded,
}

/// The `type2.osw` component.
pub struct OswReader {
    lp: Biquad,
    eye: Eye,
    /// The last [`MEAN_SAMPLES`] filtered values.
    hist: [f32; MEAN_SAMPLES],
    hk: usize,
    /// Soft value minus the threshold, by sample.
    ring: Vec<f32>,
    /// The bit histories of the 8 phases.
    regs: [u128; SPB as usize],
    n: u64,
    candidates: Vec<Candidate>,
    /// Locked: the predicted end of the next word, the polarity and the bad words in a row.
    lock: Option<(u64, bool, u32)>,
    decoder: WordDecoder,
    snr_sum: f64,
    snr_n: u32,
    out: Vec<Timed<OswItem>>,
    since_state: u64,
}

impl Default for OswReader {
    fn default() -> Self {
        OswReader {
            lp: Biquad::new(BiquadCoeffs::lowpass(
                WORK_RATE,
                LOWPASS_HZ,
                std::f64::consts::FRAC_1_SQRT_2,
            )),
            eye: Eye::default(),
            hist: [0.0; MEAN_SAMPLES],
            hk: 0,
            ring: vec![0.0; RING],
            regs: [0; SPB as usize],
            n: 0,
            candidates: Vec::with_capacity(MAX_CANDIDATES),
            lock: None,
            decoder: WordDecoder::default(),
            snr_sum: 0.0,
            snr_n: 0,
            out: Vec::new(),
            since_state: 0,
        }
    }
}

impl OswReader {
    fn soft(&self, m: u64) -> f32 {
        self.ring[(m as usize) & (RING - 1)]
    }

    /// The 84 soft values that end at sample `end` (polarity applied, positive is a 1), and the eye: the sum of their sizes.
    fn word_at(&self, end: u64, inverted: bool) -> ([f32; WORD_BITS], f32) {
        let mut soft = [0f32; WORD_BITS];
        let mut eye = 0.0;
        for (k, b) in soft.iter_mut().enumerate() {
            let v = self.soft(end - SPB * (WORD_BITS as u64 - 1 - k as u64));
            *b = if inverted { -v } else { v };
            eye += v.abs();
        }
        (soft, eye)
    }

    /// The SNR of the eye of the word that ends at `end`: (mean difference / 2)² over the mean variance of the two classes.
    fn note_snr(&mut self, end: u64) {
        let (mut s1, mut q1, mut n1, mut s0, mut q0, mut n0) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
        for k in 0..WORD_BITS as u64 {
            let v = f64::from(self.soft(end - SPB * k));
            if v > 0.0 {
                s1 += v;
                q1 += v * v;
                n1 += 1.0;
            } else {
                s0 += v;
                q0 += v * v;
                n0 += 1.0;
            }
        }
        if n1 < 2.0 || n0 < 2.0 {
            return;
        }
        let (m1, m0) = (s1 / n1, s0 / n0);
        let var = 0.5 * ((q1 / n1 - m1 * m1) + (q0 / n0 - m0 * m0));
        if var > 0.0 {
            let snr = 10.0 * ((0.5 * (m1 - m0)).powi(2) / var).log10();
            self.snr_sum += snr;
            self.snr_n += 1;
        }
    }

    fn push(&mut self, item: OswItem) {
        self.out.push(Timed {
            time: self.n as f64 / WORK_RATE,
            value: item,
        });
    }

    /// The search at sample `n` of phase `ph`.
    fn search(&mut self, n: u64, ph: usize) {
        let reg = self.regs[ph];
        let head = ((reg >> (WORD_BITS - 8)) & 0xFF) as u8;
        let inverted = match head {
            SYNC => false,
            h if h == !SYNC => true,
            _ => return,
        };
        let (soft, _) = self.word_at(n, inverted);
        let Some(word) = self.decoder.decode(&soft[8..]) else {
            return;
        };
        self.candidates.retain(|c| n - c.end <= FRAME + 6);
        let before = self
            .candidates
            .iter()
            .find(|c| c.inverted == inverted && (n - c.end).abs_diff(FRAME) <= 2)
            .copied();
        if let Some(first) = before {
            self.candidates.clear();
            self.lock = Some((n + FRAME, inverted, 0));
            self.push(OswItem::Good(first.word));
            self.push(OswItem::Good(word));
            self.note_snr(n);
            return;
        }
        if self.candidates.len() >= MAX_CANDIDATES {
            self.candidates.remove(0);
        }
        self.candidates.push(Candidate {
            end: n,
            inverted,
            word,
        });
    }

    /// The word of a locked clock that ends near `end`.
    fn locked_word(&mut self, end: u64, inverted: bool, bad: u32) {
        let mut best: Option<(f32, u64, Decoded)> = None;
        for e in end - 3..=end + 3 {
            let (soft, eye) = self.word_at(e, inverted);
            let head = soft[..8]
                .iter()
                .fold(0u8, |v, &b| (v << 1) | u8::from(b > 0.0));
            if (head ^ SYNC).count_ones() > 1 {
                continue;
            }
            if let Some(w) = self.decoder.decode(&soft[8..]) {
                if best.is_none_or(|(be, _, _)| eye > be) {
                    best = Some((eye, e, w));
                }
            }
        }
        match best {
            Some((_, e, w)) => {
                self.lock = Some((e + FRAME, inverted, 0));
                self.push(OswItem::Good(w));
                self.note_snr(e);
            }
            None => {
                self.push(OswItem::Lost);
                self.lock = (bad + 1 < MAX_BAD).then_some((end + FRAME, inverted, bad + 1));
            }
        }
    }

    fn sample(&mut self, x: f32) {
        let f = self.lp.process_sample(x);
        self.eye.update(f);
        self.hist[self.hk] = f;
        self.hk = (self.hk + 1) % MEAN_SAMPLES;
        let soft = self.hist.iter().sum::<f32>() / MEAN_SAMPLES as f32;
        let thr = self.eye.mid();
        let n = self.n;
        self.ring[(n as usize) & (RING - 1)] = soft - thr;
        let ph = (n % SPB) as usize;
        self.regs[ph] = (self.regs[ph] << 1) | u128::from(soft > thr);
        match self.lock {
            Some((end, inverted, bad)) => {
                if n == end + 4 {
                    self.locked_word(end, inverted, bad);
                }
            }
            None => {
                if n >= 1024 && self.eye.range() >= MIN_SWING_HZ {
                    self.search(n, ph);
                }
            }
        }
        self.n += 1;
    }
}

impl Component for OswReader {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "type2.osw",
            vec![Port::real("in"), Port::typed::<Level>("level")],
            vec![typed_port::<OswItem>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let out = std::mem::take(&mut self.out);
        *self = OswReader {
            out,
            ..OswReader::default()
        };
        self.out.clear();
        Ok(vec![typed_format::<OswItem>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(l) = io.items::<Level>(1).last() {
            self.push(OswItem::Level(l.value.dbfs));
        }
        let x = io.real(0);
        for &v in x {
            self.sample(v);
        }
        self.since_state += x.len() as u64;
        if self.since_state >= (WORK_RATE * 0.2) as u64 {
            self.since_state = 0;
            let snr_db = (self.snr_n > 0).then(|| (self.snr_sum / f64::from(self.snr_n)) as f32);
            self.snr_sum = 0.0;
            self.snr_n = 0;
            let (locked, inverted) = self.lock.map_or((false, false), |(_, i, _)| (true, i));
            let swing_hz = self.eye.range();
            self.push(OswItem::State {
                locked,
                inverted,
                swing_hz,
                snr_db,
            });
        }
        io.items_out::<OswItem>(0).append(&mut self.out);
    }
    fn values(&self) -> Values {
        no_values()
    }
}
