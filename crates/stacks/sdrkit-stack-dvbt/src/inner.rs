//! The inner decoder: symbol de-interleaver, soft demapper, bit de-interleaver, hierarchical stream choice, soft-value scaling,
//! depuncturing with the puncturing-phase search, and the Viterbi decoder in blocks of 16 symbols with history, in chunks that
//! the worker pool of the host decodes. It also measures the BER before Viterbi and the Viterbi margin.
//!
//! Standard: ETSI EN 300 744 clauses 4.3.3 to 4.3.5.

use crate::coding::{self, branches, DVBT_BIT_OFFSETS, INNER};
use crate::items::DataCells;
use crate::params::{CodeRate, Hierarchy, Mode, TpsParams};
use sdrkit_algo_fec::conv::{Depuncturer, Ends, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::{Complex32, Constellation, Demap};
use sdrkit_graph::{Meter, Pool};
use std::collections::VecDeque;
use std::sync::Arc;

/// Symbols per Viterbi block.
pub const BLOCK_SYMBOLS: usize = 16;
/// Trellis steps of history and of tail.
pub const TAIL: usize = 192;
/// Output steps per parallel chunk.
pub const CHUNK: usize = 16384;
/// The longest window one decoder takes: a chunk with its history and tail.
const WINDOW: usize = CHUNK + 2 * TAIL;
/// The largest soft-value scale (for nearly all-zero input).
const MAX_SCALE: f32 = 40.0;

sdrkit_core::choice_enum! {
    /// The Viterbi decoder variant.
    #[derive(Default)]
    pub enum ViterbiChoice {
        /// Fast for QPSK and 16-QAM, exact for 64-QAM.
        #[default]
        Auto = ("auto", "Auto"),
        /// Soft values to ±127 (mean 22).
        Exact = ("exact", "Exact"),
        /// Soft values to ±31 (mean 14).
        Fast = ("fast", "Fast (8-bit)"),
    }
}

/// What selects the de-interleaver tables: the mode, the constellation, the hierarchy and the low-priority stream.
type TableKey = (Mode, crate::params::Constellation, Hierarchy, bool);

/// The de-interleavers of one setting as index tables.
struct Tables {
    key: TableKey,
    /// Per de-interleaved cell of an even symbol: the cell of the symbol it comes from.
    even: Vec<u16>,
    /// The same for an odd symbol.
    odd: Vec<u16>,
    /// Per soft value of the stream: its place in the soft values of the de-interleaved cells (`v` per cell).
    bits: Vec<u32>,
}

impl Tables {
    fn new(key: TableKey) -> Self {
        let (mode, constellation, hierarchy, lp) = key;
        let nd = mode.data_carriers();
        let even = coding::symbol_permutation(mode);
        let mut odd = vec![0u16; even.len()];
        for (q, &h) in even.iter().enumerate() {
            odd[h as usize] = q as u16;
        }
        let v = constellation.bits();
        let map = branches(constellation, hierarchy, lp);
        let mut bits = Vec::with_capacity(nd * map.len());
        for blk in 0..nd / 126 {
            for w in 0..126 {
                for &e in map {
                    let from = (w + 126 - DVBT_BIT_OFFSETS[e]) % 126;
                    bits.push(((blk * 126 + from) * v + e) as u32);
                }
            }
        }
        Tables {
            key,
            even,
            odd,
            bits,
        }
    }
}

/// Measurements of the inner decoder.
#[derive(Clone, Copy, Debug, Default)]
pub struct InnerStats {
    /// Normalised path metric of the last block (1 = noiseless).
    pub margin: f32,
    /// The puncturing phase in use.
    pub phase: Option<usize>,
    /// Re-encoded errors over compared bits in the last second.
    pub ber_pre: f64,
    /// Blocks decoded.
    pub blocks: u64,
}

/// The inner decoder.
pub struct InnerDecoder {
    params: Option<TpsParams>,
    /// Decode the low-priority stream of a hierarchical multiplex.
    pub lp: bool,
    /// The Viterbi variant.
    pub choice: ViterbiChoice,
    /// The pool that decodes the chunks of a block (the output does not depend on it). `None` decodes in line.
    pub pool: Option<Pool>,
    /// The work counters of the component that runs the decoder.
    pub meter: Meter,
    constellation: Option<Constellation>,
    tables: Option<Tables>,
    z: Vec<Complex32>,
    w: Vec<f32>,
    cell_llr: Vec<f32>,
    llr: Vec<f32>,
    symbols: usize,
    soft: Vec<i8>,
    enc: Vec<u8>,
    history: Vec<i8>,
    work: Vec<i8>,
    started: bool,
    phase: Option<usize>,
    depuncture: Option<Depuncturer<'static>>,
    viterbi: Vec<Viterbi>,
    chunk_out: Vec<Vec<u8>>,
    chunk_spans: Vec<(usize, usize)>,
    ber: VecDeque<(f64, u64, u64)>,
    /// Measurements.
    pub stats: InnerStats,
    time: f64,
    duration_per_symbol: f64,
}

impl Default for InnerDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decodes the output steps `a..b` of `work` (two soft values per step, `n_steps` steps) into `o`, with the history
/// and the tail of the window. `o` then holds up to six bits of history before the steps: the start state of the
/// re-encoder.
fn decode_chunk(v: &mut Viterbi, o: &mut Vec<u8>, work: &[i8], n_steps: usize, a: usize, b: usize) {
    let lo = a.saturating_sub(TAIL);
    let hi = (b + TAIL).min(n_steps);
    o.clear();
    o.resize(hi - lo, 0);
    if v.decode_only(&work[2 * lo..2 * hi], Ends::Open, o).is_err() {
        o.fill(0);
    }
    o.drain(b - lo..);
    o.drain(..(a - lo).saturating_sub(6));
}

fn decoder() -> Viterbi {
    Viterbi::new(INNER, WINDOW).unwrap_or_else(|_| unreachable!("the DVB code has K = 7"))
}

impl InnerDecoder {
    /// A new decoder.
    pub fn new() -> Self {
        InnerDecoder {
            params: None,
            lp: false,
            choice: ViterbiChoice::Auto,
            pool: None,
            meter: Meter::default(),
            constellation: None,
            tables: None,
            z: Vec::new(),
            w: Vec::new(),
            cell_llr: Vec::new(),
            llr: Vec::new(),
            symbols: 0,
            soft: Vec::new(),
            enc: Vec::new(),
            history: Vec::new(),
            work: Vec::new(),
            started: false,
            phase: None,
            depuncture: None,
            viterbi: vec![decoder()],
            chunk_out: Vec::new(),
            chunk_spans: Vec::new(),
            ber: VecDeque::new(),
            stats: InnerStats::default(),
            time: 0.0,
            duration_per_symbol: 0.0,
        }
    }

    /// Starts afresh (new parameters, a new acquisition, a stream change): the next block searches the puncturing phase again.
    pub fn reset(&mut self) {
        self.llr.clear();
        self.symbols = 0;
        self.history.clear();
        self.started = false;
        self.phase = None;
        self.depuncture = None;
        self.stats.phase = None;
    }

    /// Picks the puncturing phase again at the next block (packet sync was not found).
    pub fn search_phase_again(&mut self) {
        self.phase = None;
        self.depuncture = None;
        self.history.clear();
        self.started = false;
    }

    /// The parameters in use.
    pub fn params(&self) -> Option<TpsParams> {
        self.params
    }

    /// The soft-value quantizer for these parameters.
    fn quantizer(&self, p: &TpsParams) -> Quantizer {
        let fast = match self.choice {
            ViterbiChoice::Exact => false,
            ViterbiChoice::Fast => true,
            ViterbiChoice::Auto => p.constellation != crate::params::Constellation::Qam64,
        };
        let (target, limit) = if fast { (14.0, 31) } else { (22.0, 127) };
        Quantizer {
            target,
            limit,
            max_scale: MAX_SCALE,
        }
    }

    /// One symbol of cells. Decoded bits (one per byte) are appended to `out` at the end of each block. Returns whether a block
    /// ended.
    pub fn push(&mut self, c: &DataCells, out: &mut Vec<u8>) -> bool {
        let p = c.params;
        let changed = self
            .params
            .map(|q| (q.mode, q.constellation, q.hierarchy, q.rate(self.lp)))
            != Some((p.mode, p.constellation, p.hierarchy, p.rate(self.lp)));
        if changed || c.fresh {
            self.reset();
            self.params = Some(p);
            self.constellation = Some(coding::constellation(p.constellation, p.hierarchy));
        }
        self.duration_per_symbol = p.symbol_samples() as f64 / c.rate.max(1.0);
        self.time = c.time;
        let nd = p.mode.data_carriers();
        let lp = self.lp && p.hierarchy.hierarchical();
        let key = (p.mode, p.constellation, p.hierarchy, lp);
        if self.tables.as_ref().is_none_or(|t| t.key != key) {
            self.tables = Some(Tables::new(key));
        }
        let (Some(t), Some(cons)) = (self.tables.as_ref(), self.constellation.as_ref()) else {
            return false;
        };
        let v = cons.bits();
        // Symbol de-interleaver (clause 4.3.4.2): even symbols y'[q] = y[H(q)], odd symbols y'[H(q)] = y[q].
        let from = if c.l.is_multiple_of(2) {
            &t.even
        } else {
            &t.odd
        };
        let timer = self.meter.section("deinterleave");
        self.z.clear();
        self.z.extend(from.iter().map(|&i| c.cells[i as usize]));
        // The noise variance is per real dimension, so the weight of a max-log LLR is 1 / (2 var).
        self.w.clear();
        self.w
            .extend(from.iter().map(|&i| 0.5 / c.noise[i as usize].max(1e-12)));
        drop(timer);
        let timer = self.meter.section("demap");
        self.cell_llr.clear();
        self.cell_llr.resize(nd * v, 0.0);
        if cons
            .demap_block_weighted(&self.z, &self.w, Demap::MaxLog, &mut self.cell_llr)
            .is_err()
        {
            // A cell that is not finite is an erasure; the others are demapped one by one.
            for ((&z, &w), o) in self
                .z
                .iter()
                .zip(&self.w)
                .zip(self.cell_llr.chunks_exact_mut(v))
            {
                if cons.demap_weighted(z, w, Demap::MaxLog, o).is_err() {
                    o.fill(0.0);
                }
            }
        }
        // Bit de-interleaver per 126-cell block (clause 4.3.4.1) and the choice of the stream.
        drop(timer);
        let timer = self.meter.section("deinterleave");
        let cell_llr = &self.cell_llr;
        self.llr
            .extend(t.bits.iter().map(|&i| cell_llr[i as usize]));
        drop(timer);
        self.symbols += 1;
        if self.symbols < BLOCK_SYMBOLS {
            return false;
        }
        self.symbols = 0;
        let _timer = self.meter.section("viterbi");
        self.decode_block(&p, out);
        true
    }

    fn decode_block(&mut self, p: &TpsParams, out: &mut Vec<u8>) {
        let rate = p.rate(self.lp && p.hierarchy.hierarchical());
        let quant = self.quantizer(p);
        self.soft.clear();
        self.soft.resize(self.llr.len(), 0);
        let scale = quant.scale_for(&self.llr);
        // A NaN is written as an erasure. The rest of the block is still good.
        let _ = quant.quantize(&self.llr, scale, &mut self.soft);
        self.llr.clear();
        // The buffer: history (2 x 192 steps, the first 192 already output) and the new steps. An odd value waits for its pair
        // at the next block.
        self.work.clear();
        if self.phase.is_none() {
            let ph = self.search_phase(rate, quant.target);
            self.phase = Some(ph);
            self.stats.phase = Some(ph);
            self.history.clear();
            self.started = false;
            let mut d =
                Depuncturer::with_phase(coding::puncture(rate), coding::pattern_index(rate, ph));
            // The places of the first period before the phase are not received: they are erasures.
            self.work.resize(coding::pattern_index(rate, ph), 0);
            d.push(&self.soft, &mut self.work);
            self.depuncture = Some(d);
        } else if let Some(d) = self.depuncture.as_mut() {
            self.work.extend_from_slice(&self.history);
            d.push(&self.soft, &mut self.work);
        }
        let pending = self.work.split_off(self.work.len() - self.work.len() % 2);
        let n_steps = self.work.len() / 2;
        let from = if self.started { TAIL } else { 0 };
        let to = n_steps.saturating_sub(TAIL);
        if to <= from {
            self.history.clone_from(&self.work);
            self.history.extend_from_slice(&pending);
            return;
        }
        // Chunks of 16384 output steps with 192 steps of overlap on each side.
        let mut chunks = std::mem::take(&mut self.chunk_spans);
        chunks.clear();
        chunks.extend((from..to).step_by(CHUNK).map(|a| (a, (a + CHUNK).min(to))));
        while self.viterbi.len() < chunks.len() {
            self.viterbi.push(decoder());
        }
        self.chunk_out.resize(chunks.len(), Vec::new());
        let pool = self
            .pool
            .as_ref()
            .filter(|p| p.workers() > 0 && chunks.len() > 1);
        if let Some(pool) = pool {
            let work = Arc::new(std::mem::take(&mut self.work));
            let shared = Arc::clone(&work);
            let items: Vec<_> = self
                .viterbi
                .drain(..chunks.len())
                .zip(self.chunk_out.drain(..))
                .zip(chunks.iter().copied())
                .collect();
            let done = pool.map(items, move |_, ((mut v, mut o), (a, b))| {
                decode_chunk(&mut v, &mut o, &shared, n_steps, a, b);
                (v, o)
            });
            for (v, o) in done {
                self.viterbi.push(v);
                self.chunk_out.push(o);
            }
            self.work = Arc::try_unwrap(work).unwrap_or_else(|w| w.to_vec());
        } else {
            for (i, &(a, b)) in chunks.iter().enumerate() {
                let (v, o) = (&mut self.viterbi[i], &mut self.chunk_out[i]);
                decode_chunk(v, o, &self.work, n_steps, a, b);
            }
        }
        // BER before Viterbi and the margin, on the re-encoded bits.
        let (mut err, mut cmp) = (0u64, 0u64);
        let (mut mnum, mut mden) = (0f64, 0f64);
        for (i, &(a, b)) in chunks.iter().enumerate() {
            let o = &self.chunk_out[i];
            let lead = o.len() - (b - a);
            let (state, bits) = (start_state(&o[..lead]), &o[lead..]);
            let r = reencode(
                state,
                bits,
                &self.work[2 * a..2 * b],
                quant.target,
                &mut self.enc,
            );
            err += r.errors;
            cmp += r.compared;
            mnum += r.metric;
            mden += r.full;
            out.extend_from_slice(bits);
        }
        self.chunk_spans = chunks;
        self.stats.margin = if mden > 0.0 {
            (mnum / mden) as f32
        } else {
            0.0
        };
        self.stats.blocks += 1;
        let now = self.time;
        self.ber.push_back((now, err, cmp));
        while self.ber.front().is_some_and(|f| now - f.0 > 1.0) {
            self.ber.pop_front();
        }
        let (e, c): (u64, u64) = self.ber.iter().fold((0, 0), |a, b| (a.0 + b.1, a.1 + b.2));
        self.stats.ber_pre = if c > 0 { e as f64 / c as f64 } else { 0.0 };
        // Keep the last 2 x 192 steps.
        let keep = (4 * TAIL).min(self.work.len());
        self.history.clear();
        self.history
            .extend_from_slice(&self.work[self.work.len() - keep..]);
        self.history.extend_from_slice(&pending);
        self.started = true;
    }

    /// The puncturing phase (a received-bit index in the period) whose decoding has the best path metric, on the start of the
    /// block.
    fn search_phase(&mut self, rate: CodeRate, target: f32) -> usize {
        let n_out = rate.ratio().1;
        let mut best = (0usize, f64::MIN);
        let mut steps = Vec::new();
        let mut bits = Vec::new();
        for ph in 0..n_out {
            let at = coding::pattern_index(rate, ph);
            let mut d = Depuncturer::with_phase(coding::puncture(rate), at);
            steps.clear();
            steps.resize(at, 0);
            d.push(&self.soft, &mut steps);
            let n = (steps.len() / 2).min(WINDOW);
            bits.clear();
            bits.resize(n, 0);
            if self.viterbi[0]
                .decode_only(&steps[..2 * n], Ends::Open, &mut bits)
                .is_err()
            {
                continue;
            }
            let from = TAIL.min(n);
            let r = reencode(
                start_state(&bits[..from]),
                &bits[from..],
                &steps[2 * from..2 * n],
                target,
                &mut self.enc,
            );
            let m = if r.full > 0.0 { r.metric / r.full } else { 0.0 };
            if m > best.1 {
                best = (ph, m);
            }
        }
        best.0
    }

    /// Signal seconds of one block.
    pub fn block_seconds(&self) -> f64 {
        self.duration_per_symbol * BLOCK_SYMBOLS as f64
    }
}

/// The encoder state after `bits` (the last 6 of them, the newest in the top place).
fn start_state(bits: &[u8]) -> u32 {
    bits.iter()
        .rev()
        .take(6)
        .rev()
        .fold(0u32, |s, &b| (s >> 1) | (((b & 1) as u32) << 5))
}

/// What re-encoding the decoded bits says about the soft values that went in.
struct Reencoded {
    /// Hard decisions that disagree with the re-encoded bits.
    errors: u64,
    /// Soft values compared (not punctured or erased).
    compared: u64,
    /// The path metric: the sum of the soft values signed by the re-encoded bits.
    metric: f64,
    /// The metric of a noiseless signal at the quantizer's target.
    full: f64,
}

/// Re-encodes `bits` from `state` and compares the result with `soft` (two values per step), skipping zeros (punctured or
/// erased places). `ext` is scratch.
fn reencode(state: u32, bits: &[u8], soft: &[i8], target: f32, ext: &mut Vec<u8>) -> Reencoded {
    let taps = &*TAPS;
    // The six bits before the first, oldest first, then the bits.
    ext.clear();
    ext.extend((0..6).map(|k| ((state >> k) & 1) as u8));
    ext.extend(bits.iter().map(|b| b & 1));
    let (mut errors, mut compared, mut metric) = (0u64, 0u64, 0i64);
    let steps = bits.len().min(soft.len() / 2);
    // The sums of a part fit 32 bits.
    const PART: usize = 1 << 12;
    for from in (0..steps).step_by(PART) {
        let to = (from + PART).min(steps);
        let (mut e, mut c, mut m) = (0u32, 0u32, 0i32);
        let wins = ext[from..to + 6].windows(7);
        for (win, s) in wins.zip(soft[2 * from..2 * to].as_chunks::<2>().0) {
            for ((t, zeros), &v) in taps.iter().zip(s) {
                let one = (0..7).fold(*zeros, |x, k| x ^ (win[k] & t[k])) == 1;
                let used = v != 0;
                c += used as u32;
                e += (used & ((v < 0) != one)) as u32;
                m += if one { -(v as i32) } else { v as i32 };
            }
        }
        errors += e as u64;
        compared += c as u64;
        metric += m as i64;
    }
    Reencoded {
        errors,
        compared,
        metric: metric as f64,
        full: compared as f64 * target as f64,
    }
}

/// The inner encoder as taps: per output, whether the bit at each place of a window of seven bits (oldest first) is part
/// of its sum, and the output for a window of zeros.
static TAPS: std::sync::LazyLock<[([u8; 7], u8); 2]> = std::sync::LazyLock::new(|| {
    let zeros = INNER.label(0);
    std::array::from_fn(|j| {
        let taps = std::array::from_fn(|k| ((INNER.label(1 << k) ^ zeros) >> j) & 1);
        (taps, (zeros >> j) & 1)
    })
});
