//! Code synchronisation and decoding of the CCSDS rate 1/2, K = 7 convolutional code. [`CodeSync`] finds the phase, the spectrum
//! sense, the pairing of the code symbols, the inversion of the second output and NRZ-M coding from the attached sync marker,
//! decodes with the streaming Viterbi decoder and estimates the channel bit error rate from the re-encoded bits.
//! [`blind_search`] covers links whose marker spacing is not known; it cannot detect NRZ-M. LLRs are positive for 0.
//!
//! Standard: CCSDS 131.0-B-5 clause 3.

use crate::marker;
use crate::{Error, Param, Result};
use sdrkit_algo_fec::conv::{codes, Encoder, Viterbi};
use sdrkit_algo_fec::search;

/// The code: G1 = 171, G2 = 133 (octal), G1 sent first, neither output inverted (inversion of G2 is one of the symmetries the
/// search finds, and [`CodeLock::g2_inverted`] reports it).
pub const CODE: sdrkit_algo_fec::conv::ConvCode = codes::K7_171_133;

const MASK26: u64 = (1 << 26) - 1;

/// The eight symmetries of a pair of soft code symbols `(a, b)` (the I and Q of one QPSK symbol, or two BPSK symbols): the four
/// quarter turns, then the same with the second output negated. Transform 0 is the identity, 2 a half turn (both inverted), 4 the
/// second output inverted (G2 inverted, as CCSDS sends it), 5 the outputs swapped (a mirrored spectrum).
#[inline]
pub fn transform(t: u8, a: i8, b: i8) -> (i8, i8) {
    let n = |v: i8| v.saturating_neg().max(-127);
    match t & 7 {
        0 => (a, b),
        1 => (b, n(a)),
        2 => (n(a), n(b)),
        3 => (n(b), a),
        4 => (a, n(b)),
        5 => (b, a),
        6 => (n(a), b),
        _ => (n(b), n(a)),
    }
}

/// For transform `t`: the hard rails `(a, b)` a channel must deliver for code rails `(r1, r2)`.
fn rails_for(t: u8, r1: u64, r2: u64) -> (u64, u64) {
    let n = |v: u64| !v & MASK26;
    match t & 7 {
        0 => (r1, r2),
        1 => (n(r2), r1),
        2 => (n(r1), n(r2)),
        3 => (r2, n(r1)),
        4 => (r1, n(r2)),
        5 => (r2, r1),
        6 => (n(r1), r2),
        _ => (n(r2), n(r1)),
    }
}

/// The 64 code bits of the 32 bits of `word` (first bit most significant) from encoder state `state` (the six bits before it,
/// newest in bit 5), the first code bit in the most significant place, outputs G1 then G2.
pub fn encode_word(word: u32, state: u32) -> u64 {
    let mut e = Encoder::with_state(CODE, state);
    let mut out = 0u64;
    for k in (0..32).rev() {
        let l = e.push_bit(((word >> k) & 1) as u8);
        out = (out << 2) | (((l & 1) as u64) << 1) | ((l >> 1) & 1) as u64;
    }
    out
}

/// The coded TM marker 1ACFFC1D in the state-independent places: `!0xFCA2_B63D_B00D_9794` in its low 52 bits (G2 not
/// inverted). Meteor-M, MetOp and GOES recordings provided this value.
pub const TM_MARKER_CODED: u64 = !0xFCA2_B63D_B00D_9794;

/// The number of code bits at the end of a coded 32-bit word that do not depend on the encoder state before it.
pub const STATE_FREE_BITS: u32 = 52;

/// NRZ-M coding of the bits of `word` (first bit most significant) after level `prev`: each 1 changes the level.
pub fn nrzm_word(word: u32, prev: u8) -> u32 {
    let mut level = (prev & 1) as u32;
    let mut out = 0u32;
    for k in (0..32).rev() {
        level ^= (word >> k) & 1;
        out = (out << 1) | level;
    }
    out
}

/// The two rails (G1 bits, G2 bits) of the state-free tail of a coded 32-bit word, as 26-bit values, first bit most significant.
pub fn coded_rails(word: u32) -> (u32, u32) {
    let c = encode_word(word, 0) & ((1u64 << 52) - 1);
    let (mut r1, mut r2) = (0u32, 0u32);
    for k in (0..26).rev() {
        let pair = (c >> (2 * k)) & 3;
        r1 = (r1 << 1) | (pair >> 1) as u32;
        r2 = (r2 << 1) | (pair & 1) as u32;
    }
    (r1, r2)
}

/// How a link is searched and tracked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeConfig {
    /// Bits from one marker to the next (marker included): 8192 for the 1024-byte CADU of a Reed-Solomon (255,223) link with
    /// interleave depth 4.
    pub frame_bits: u32,
    /// Wrong hard bits accepted among the 52 when searching.
    pub search_errors: u32,
    /// Wrong hard bits accepted among the 52 for each marker of a locked stream.
    pub track_errors: u32,
    /// Lock is lost after more than this many missed markers in a row.
    pub max_misses: u32,
    /// Also look for NRZ-M before the encoder.
    pub nrzm: bool,
}

impl CodeConfig {
    /// Low-orbit links with 1024-byte CADUs (Meteor-M, MetOp AHRPT, FY-3): 7 errors to search, 16 to track, 4 misses, NRZ-M too.
    pub const LEO: CodeConfig = CodeConfig {
        frame_bits: 8192,
        search_errors: 7,
        track_errors: 16,
        max_misses: 4,
        nrzm: true,
    };
    /// Geostationary links with 1024-byte CADUs (GOES, GK-2A, Elektro-L): as [`CodeConfig::LEO`] with 3 misses.
    pub const GEO: CodeConfig = CodeConfig {
        max_misses: 3,
        ..CodeConfig::LEO
    };

    fn frame_symbols(&self) -> u64 {
        2 * self.frame_bits as u64
    }

    fn check(&self) -> Result<()> {
        if self.frame_bits < 64 || self.frame_bits > 1 << 20 {
            return Err(Error::BadParameter(Param::CodeSyncFrameBits));
        }
        if self.search_errors >= 26 || self.track_errors >= 26 {
            return Err(Error::BadParameter(Param::CodeSyncErrors));
        }
        Ok(())
    }
}

/// What the synchroniser found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CodeLock {
    /// The code is locked.
    pub locked: bool,
    /// The symmetry undone (0 to 7, see [`transform`]).
    pub transform: u8,
    /// The pairing starts one symbol late (an offset QPSK receiver locked half a symbol late).
    pub shifted: bool,
    /// NRZ-M before the encoder.
    pub nrzm: bool,
    /// Code symbols decoded since the lock.
    pub symbols: u64,
    /// Re-encoded bit errors per code symbol compared (the channel bit error rate estimate), over about the last 10,000 symbols.
    /// It is updated each time 10,000 more symbols are compared, so it does not depend on how the input is cut.
    pub ber: f64,
    /// Locks found since construction.
    pub locks: u32,
    /// Hard bit errors of the last marker compared (of 52).
    pub last_marker_errors: u32,
}

impl CodeLock {
    /// The second code output arrives inverted (a mirrored transform, as CCSDS sends G2).
    pub fn g2_inverted(&self) -> bool {
        self.transform >= 4
    }

    /// The ambiguity as one number for display: the transform, plus 8 when the pairing was shifted.
    pub fn ambiguity(&self) -> u8 {
        self.transform + if self.shifted { 8 } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Candidate {
    end: u64,
    pairing: u8,
    t: u8,
    pattern: u8,
}

const HARD_RING: usize = 1024;
/// Code symbols compared before the error rate estimate is updated and the counts are halved.
const BER_WINDOW: u64 = 20_000;
const MAX_CANDIDATES: usize = 64;

/// The streaming code synchroniser and decoder. See the [module documentation](self).
///
/// ```
/// use sdrkit_proto_ccsds::codesync::{transform, CodeConfig, CodeSync};
/// use sdrkit_algo_fec::conv::{codes, Encoder};
/// // Four 1024-byte frames, each starting with the marker, coded as CCSDS sends them (G2 inverted).
/// let mut bits = Vec::new();
/// for f in 0..4u32 {
///     for k in (0..32).rev() { bits.push(((0x1ACF_FC1Du32 >> k) & 1) as u8); }
///     bits.extend((0..8160u32).map(|i| ((i * 7 + f) % 5 == 0) as u8));
/// }
/// let mut coded = Vec::new();
/// Encoder::new(codes::CCSDS_K7).encode(&bits, &mut coded);
/// let llr: Vec<i8> = coded.iter().map(|&c| if c == 0 { 50 } else { -50 }).collect();
/// let mut sync = CodeSync::new(CodeConfig::LEO).unwrap();
/// let mut out = Vec::new();
/// sync.push(&llr, &mut out);
/// assert!(sync.lock().locked && sync.lock().g2_inverted());
/// ```
pub struct CodeSync {
    cfg: CodeConfig,
    ring: Box<[i8]>,
    ring_mask: usize,
    n: u64,
    regs: [[u64; 2]; 2],
    /// (pattern, transform) -> (rail a, rail b) expected.
    expect: [[(u64, u64); 8]; 2],
    candidates: Vec<Candidate>,
    current: Option<Candidate>,
    viterbi: Viterbi,
    batch: [i8; 256],
    nbatch: usize,
    next_check: u64,
    misses: u32,
    nrzm_prev: u8,
    hard: Box<[u8; HARD_RING]>,
    vsteps: u64,
    reenc: Encoder,
    reenc_step: u64,
    ber_err: u64,
    ber_n: u64,
    lock: CodeLock,
}

impl std::fmt::Debug for CodeSync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeSync")
            .field("cfg", &self.cfg)
            .field("lock", &self.lock)
            .finish()
    }
}

impl CodeSync {
    /// A synchroniser for the TM marker with these rules. Allocates its symbol ring (about two frames) here.
    pub fn new(cfg: CodeConfig) -> Result<Self> {
        Self::with_word(cfg, marker::TM.value() as u32)
    }

    /// A synchroniser for the 32-bit sync word `asm` (first bit most significant) in place of the TM marker, with these rules.
    pub fn with_word(cfg: CodeConfig, asm: u32) -> Result<Self> {
        cfg.check()?;
        let (p1, p2) = coded_rails(asm);
        let mut expect = [[(0u64, 0u64); 8]; 2];
        // NRZ-M before the encoder: the coded marker after NRZ-M from level 0 (from level 1 it is the complement, which is the
        // half-turn transform).
        let (q1, q2) = coded_rails(nrzm_word(asm, 0));
        for t in 0..8u8 {
            expect[0][t as usize] = rails_for(t, p1 as u64, p2 as u64);
            expect[1][t as usize] = rails_for(t, q1 as u64, q2 as u64);
        }
        let ring = (2 * cfg.frame_symbols() as usize + 2048).next_power_of_two();
        Ok(CodeSync {
            cfg,
            ring: vec![0i8; ring].into_boxed_slice(),
            ring_mask: ring - 1,
            n: 0,
            regs: [[0; 2]; 2],
            expect,
            candidates: Vec::with_capacity(2 * MAX_CANDIDATES + 32),
            current: None,
            viterbi: Viterbi::new(CODE, 0)?,
            batch: [0; 256],
            nbatch: 0,
            next_check: 0,
            misses: 0,
            nrzm_prev: 0,
            hard: Box::new([0; HARD_RING]),
            vsteps: 0,
            reenc: Encoder::new(CODE),
            reenc_step: 0,
            ber_err: 0,
            ber_n: 0,
            lock: CodeLock::default(),
        })
    }

    /// The rules.
    pub fn config(&self) -> &CodeConfig {
        &self.cfg
    }

    /// What the synchroniser found so far.
    pub fn lock(&self) -> &CodeLock {
        &self.lock
    }

    /// Drops the lock and searches again (a retune, or the frame layer lost the frames).
    pub fn search_again(&mut self) {
        self.current = None;
        self.candidates.clear();
        self.lock.locked = false;
        self.misses = 0;
        self.nbatch = 0;
    }

    /// Code symbols taken since construction.
    pub fn symbols_seen(&self) -> u64 {
        self.n
    }

    #[inline]
    fn errors(&self, pairing: usize, pattern: usize, t: usize) -> u32 {
        let (ea, eb) = self.expect[pattern][t];
        let [ra, rb] = self.regs[pairing];
        ((ra & MASK26) ^ ea).count_ones() + ((rb & MASK26) ^ eb).count_ones()
    }

    /// Takes soft code symbols (LLRs, positive means 0) and appends the decoded bits (0 or 1) to `out`. The output does not depend
    /// on how the input is split between calls. A decoded bit is output about 200 symbols after its own (the Viterbi window).
    pub fn push(&mut self, llrs: &[i8], out: &mut Vec<u8>) {
        let mask = self.ring_mask;
        for &s in llrs {
            let i = self.n;
            self.ring[i as usize & mask] = s;
            self.n += 1;
            if i < 1 {
                continue;
            }
            // Pairing p completes a pair at index i when (i - p) is odd.
            let p = ((i + 1) & 1) as usize;
            let a = self.ring[(i - 1) as usize & mask];
            let hb = |v: i8| (v < 0) as u64;
            let r = &mut self.regs[p];
            r[0] = (r[0] << 1) | hb(a);
            r[1] = (r[1] << 1) | hb(s);
            match self.current {
                None => self.search_at(i, p as u8, out),
                Some(c) => {
                    if p as u8 == c.pairing {
                        self.queue_pair(a, s, c, out);
                        if i + 1 == self.next_check {
                            self.check(c, out);
                        }
                    }
                }
            }
        }
        if let Some(c) = self.current {
            self.drain(c, out);
        }
    }

    /// Releases the bits still inside the Viterbi window (the end of a recording).
    pub fn flush(&mut self, out: &mut Vec<u8>) {
        if let Some(c) = self.current {
            self.drain(c, out);
            let mark = out.len();
            self.viterbi.flush(out);
            self.after_release(mark, c, out);
        }
    }

    fn search_at(&mut self, i: u64, p: u8, out: &mut Vec<u8>) {
        let patterns = if self.cfg.nrzm { 2 } else { 1 };
        for pat in 0..patterns {
            for t in 0..8u8 {
                if self.errors(p as usize, pat, t as usize) > self.cfg.search_errors {
                    continue;
                }
                let end = i + 1;
                // With NRZ-M the marker arrives from either level (a half turn apart), which decodes the same: one class.
                let t = if pat == 1 { t & 0b101 } else { t };
                let frame = self.cfg.frame_symbols();
                let confirmed = self.candidates.iter().any(|c| {
                    c.pairing == p && c.t == t && c.pattern == pat as u8 && c.end + frame == end
                });
                let cand = Candidate {
                    end,
                    pairing: p,
                    t,
                    pattern: pat as u8,
                };
                if confirmed {
                    self.locked_at(cand, out);
                    return;
                }
                if self.candidates.len() < self.candidates.capacity() {
                    self.candidates.push(cand);
                }
            }
        }
        if self.candidates.len() > MAX_CANDIDATES {
            let horizon = (i + 1).saturating_sub(3 * self.cfg.frame_symbols());
            self.candidates.retain(|c| c.end >= horizon);
            if self.candidates.len() > MAX_CANDIDATES {
                let k = self.candidates.len() - MAX_CANDIDATES;
                self.candidates.drain(..k);
            }
        }
    }

    fn locked_at(&mut self, c: Candidate, out: &mut Vec<u8>) {
        self.current = Some(c);
        self.candidates.clear();
        self.misses = 0;
        self.lock = CodeLock {
            locked: true,
            transform: c.t,
            shifted: c.pairing == 1,
            nrzm: c.pattern == 1,
            locks: self.lock.locks + 1,
            ..CodeLock::default()
        };
        self.viterbi.reset();
        self.nbatch = 0;
        self.vsteps = 0;
        self.reenc = Encoder::new(CODE);
        self.reenc_step = 0;
        self.ber_err = 0;
        self.ber_n = 0;
        self.nrzm_prev = 0;
        let frame = self.cfg.frame_symbols();
        self.next_check = c.end + frame;
        // Decode from a little before the earlier marker, so its frame is kept; from the start of the stream when the
        // earlier marker is its first.
        let first_marker = c.end.saturating_sub(frame + 64);
        let oldest = self.n.saturating_sub(self.ring.len() as u64 - 8);
        let mut k = first_marker.saturating_sub(256).max(oldest);
        if k % 2 != c.pairing as u64 {
            k += 1;
        }
        let mask = self.ring_mask;
        while k + 1 < self.n {
            let a = self.ring[k as usize & mask];
            let b = self.ring[(k + 1) as usize & mask];
            self.queue_pair(a, b, c, out);
            k += 2;
        }
    }

    #[inline]
    fn queue_pair(&mut self, a: i8, b: i8, c: Candidate, out: &mut Vec<u8>) {
        let (x, y) = transform(c.t, a, b);
        self.hard[self.vsteps as usize & (HARD_RING - 1)] = (((x < 0) as u8) << 1) | (y < 0) as u8;
        self.vsteps += 1;
        self.batch[self.nbatch] = x;
        self.batch[self.nbatch + 1] = y;
        self.nbatch += 2;
        self.lock.symbols += 2;
        if self.nbatch == self.batch.len() {
            self.drain(c, out);
        }
    }

    fn drain(&mut self, c: Candidate, out: &mut Vec<u8>) {
        if self.nbatch == 0 {
            return;
        }
        let mark = out.len();
        let batch = self.batch;
        self.viterbi.push(&batch[..self.nbatch], out);
        self.nbatch = 0;
        self.after_release(mark, c, out);
    }

    fn after_release(&mut self, mark: usize, c: Candidate, out: &mut [u8]) {
        if out.len() == mark {
            return;
        }
        // Re-encode the released bits and compare with the hard symbols of the same steps.
        for &bit in &out[mark..] {
            let l = self.reenc.push_bit(bit);
            let h = self.hard[self.reenc_step as usize & (HARD_RING - 1)];
            self.ber_err += ((l & 1) != (h >> 1)) as u64 + (((l >> 1) & 1) != (h & 1)) as u64;
            self.ber_n += 2;
            self.reenc_step += 1;
            // The window moves at fixed bit counts, so the estimate does not depend on how the input was cut.
            if self.ber_n >= BER_WINDOW {
                self.lock.ber = self.ber_err as f64 / self.ber_n as f64;
                self.ber_err /= 2;
                self.ber_n /= 2;
            }
        }
        if c.pattern == 1 {
            for v in out[mark..].iter_mut() {
                let d = *v ^ self.nrzm_prev;
                self.nrzm_prev = *v;
                *v = d;
            }
        }
    }

    fn check(&mut self, c: Candidate, out: &mut Vec<u8>) {
        let e = self.errors(c.pairing as usize, c.pattern as usize, c.t as usize);
        // With NRZ-M the marker can arrive from either level: the complement counts too.
        let e = if c.pattern == 1 {
            e.min(self.errors(
                c.pairing as usize,
                1,
                (((c.t + 2) & 3) | (c.t & 4)) as usize,
            ))
        } else {
            e
        };
        self.lock.last_marker_errors = e;
        self.next_check += self.cfg.frame_symbols();
        if e <= self.cfg.track_errors {
            self.misses = 0;
        } else {
            self.misses += 1;
            if self.misses > self.cfg.max_misses {
                self.drain(c, out);
                self.search_again();
            }
        }
    }
}

/// A hypothesis of the blind search: the symmetry and the pairing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hypothesis {
    /// The symmetry to undo (see [`transform`]).
    pub transform: u8,
    /// Pair from the second symbol.
    pub shifted: bool,
}

/// The outcome of [`blind_search`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlindResult {
    /// The best hypothesis.
    pub best: Hypothesis,
    /// Its re-encode error rate (about the channel bit error rate when right, 0.3 to 0.5 when wrong).
    pub error_rate: f32,
    /// The runner-up's error rate minus the best's. Hypotheses that decode the same bits up to inversion (a half turn) tie, so
    /// the margin is taken over the other classes.
    pub margin: f32,
}

/// Scratch for [`blind_search`], sized for a window of `steps` symbol pairs.
#[derive(Debug)]
pub struct BlindScratch {
    viterbi: Viterbi,
    llr: Vec<i8>,
    bits: Vec<u8>,
    steps: usize,
}

impl BlindScratch {
    /// Scratch for windows of `steps` symbol pairs (at least 64, and a few hundred is enough at moderate error rates).
    pub fn new(steps: usize) -> Result<Self> {
        if steps < 64 {
            return Err(Error::BadParameter(Param::BlindSearchWindow));
        }
        Ok(BlindScratch {
            viterbi: Viterbi::new(CODE, steps)?,
            llr: vec![0; 2 * steps],
            bits: vec![0; steps],
            steps,
        })
    }
}

/// Decodes the first `steps` symbol pairs of `llrs` under each of the 16 symmetry and pairing hypotheses and returns the one with
/// the fewest re-encode errors (CCSDS 131.0 does not define this search. It is the usual blind acquisition of the code.). `llrs`
/// needs at least `2 * steps + 1` values. A half turn decodes to the inverted bits with the same error rate, so transform `t` and
/// `t ^ 2` tie. The frame synchroniser resolves that by the marker's polarity.
pub fn blind_search(llrs: &[i8], scratch: &mut BlindScratch) -> Result<BlindResult> {
    let steps = scratch.steps;
    if llrs.len() < 2 * steps + 1 {
        return Err(Error::InputTooShort {
            need: 2 * steps + 1,
        });
    }
    let mut rates = [f32::NAN; 16];
    for (h, rate) in rates.iter_mut().enumerate() {
        let (t, shift) = ((h & 7) as u8, h >> 3);
        for s in 0..steps {
            let (x, y) = transform(t, llrs[shift + 2 * s], llrs[shift + 2 * s + 1]);
            scratch.llr[2 * s] = x;
            scratch.llr[2 * s + 1] = y;
        }
        *rate = search::viterbi_error_rate(&mut scratch.viterbi, &scratch.llr, &mut scratch.bits)?;
    }
    let r = search::best_of(16, |h| Some(rates[h])).ok_or(Error::BadLength)?;
    // Margin over hypotheses outside the best's half-turn class.
    let twin = r.best ^ 2;
    let runner = (0..16)
        .filter(|&h| h != r.best && h != twin)
        .map(|h| rates[h])
        .fold(f32::INFINITY, f32::min);
    Ok(BlindResult {
        best: Hypothesis {
            transform: (r.best & 7) as u8,
            shifted: r.best >= 8,
        },
        error_rate: r.score,
        margin: runner - r.score,
    })
}
