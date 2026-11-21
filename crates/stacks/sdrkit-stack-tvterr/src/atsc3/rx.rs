//! The demodulation of one ATSC 3.0 frame at 6.144 Msps: the carrier offset from the guard interval, the OFDM symbols, the
//! channel estimate from the pilots, the equalised data cells, L1-Basic and L1-Detail, and the cells of one PLP in its
//! subslices, subframe by subframe.
//!
//! Standard: ATSC A/322:2023-03 clauses 6.5, 7.2, 7.3, 8.1 and 9.

use super::bicm::{constellation, PlpCoding};
use super::bootstrap::{self, Fields};
use super::frame::{excess_samples, preamble_symbols, subframe_symbols, NotDecoded, Symbol};
use super::l1::{
    detail_segments, DetailSubframe, FrameLength, L1Basic, L1Detail, Plp, Scrambler, SubframeParams, L1_BASIC_BITS,
};
use super::layout::{
    common_pilots, frequency_interleaver, guard_samples, FftSize, PreambleStructure,
};
use super::ti::TiFrame;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_fec::a322::{L1Code, L1Kind};
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_core::Complex32;
use std::sync::Arc;

/// Samples of the guard interval that the FFT window starts before the useful part of a symbol.
pub const BACKOFF: usize = 16;

/// The longest first part of a frame (bootstrap, Preamble and first subframe) that the decoder takes, samples: 1 s.
pub const MAX_SAMPLES: usize = 6_144_000;

/// The gain from true LLRs to the 8-bit LLRs of the L1 decoder.
const LLR_GAIN: f32 = 4.0;

/// The share of the noise of a grid estimate that a straight line between two of them keeps on average: 2/3. Half of the grid
/// estimates of a subframe are the mean of two symbols.
const PREAMBLE_SPREAD: f64 = 2.0 / 3.0;
const SUBFRAME_SPREAD: f64 = 0.5;

/// The LDPC iterations of an L1 block: a first try, then the full count.
const L1_ITERATIONS: [u32; 2] = [10, 50];

/// One PLP as L1-Detail lists it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlpInfo {
    /// Its id.
    pub id: u8,
    /// Its coding, when it is one of tables 9.17, 9.18 and 9.20 with 256 points or fewer.
    pub coding: Option<PlpCoding>,
    /// Its data cells in the subframe.
    pub cells: u32,
    /// Its layer: 0 core, above 0 enhanced.
    pub layer: u8,
    /// Its time interleaver mode (table 9.21).
    pub ti_mode: u8,
    /// The subframe that this entry is of, from 0.
    pub subframe: u16,
}

impl PlpInfo {
    fn of(subframe: usize, p: &Plp) -> PlpInfo {
        PlpInfo {
            subframe: subframe as u16,
            id: p.id,
            coding: PlpCoding::of(p),
            cells: p.size,
            layer: p.layer,
            ti_mode: p.ti_mode,
        }
    }
}

/// What the signalling of a frame says.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameInfo {
    /// The FFT size of the Preamble and of the first subframe.
    pub fft: FftSize,
    /// The guard interval, samples.
    pub guard: usize,
    /// The L1-Basic mode.
    pub l1_basic_mode: u8,
    /// The L1-Detail mode.
    pub l1_detail_mode: u8,
    /// The Preamble symbols.
    pub preamble_symbols: u8,
    /// The subframes of the frame.
    pub subframes: u16,
    /// The data and subframe boundary symbols of the first subframe.
    pub symbols: u16,
    /// The scattered pilot pattern code of the first subframe (table 9.12).
    pub pilot_pattern: u8,
    /// The PLPs of every subframe, in the order of L1-Detail. Empty when L1-Detail was not read.
    pub plps: Vec<PlpInfo>,
}

impl FrameInfo {
    /// Whether this is what `structure`, `basic` and the subframes `subframes` say.
    fn says(
        &self,
        structure: &PreambleStructure,
        basic: &L1Basic,
        subframes: &[DetailSubframe],
    ) -> bool {
        let mut plps = subframes
            .iter()
            .enumerate()
            .flat_map(|(k, s)| s.plps.iter().map(move |p| PlpInfo::of(k, p)));
        self.fft == structure.fft
            && self.guard == structure.guard
            && self.l1_basic_mode == structure.l1_basic_mode
            && self.l1_detail_mode == basic.detail_mode
            && self.preamble_symbols == basic.preamble_symbols
            && self.subframes == basic.subframes
            && self.symbols == basic.first.symbols
            && self.pilot_pattern == basic.first.pilot_pattern
            && self.plps.iter().all(|a| plps.next() == Some(*a))
            && plps.next().is_none()
    }
}

/// The chosen PLP in one subframe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartPlp {
    /// Its coding.
    pub coding: PlpCoding,
    /// The index of its first data cell in the subframe and its data cells.
    pub span: (usize, usize),
    /// Its subslices (1 for a non-dispersed PLP) and the cells from the start of one to the start of the next.
    pub subslices: (usize, usize),
    /// Its time interleaving in this subframe.
    pub ti: TiFrame,
    /// The first cell of its first whole FEC block, `None` when no block begins in this subframe or the PLP is time
    /// interleaved.
    pub fec_block_start: Option<usize>,
}

/// One subframe of the frame read last.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    /// Its number in the frame, from 0.
    pub subframe: usize,
    /// The samples from the start of the bootstrap to its first symbol.
    pub from: usize,
    /// The samples from the start of the bootstrap to its end.
    pub samples: usize,
    /// The chosen PLP in it. `None` when the subframe does not carry the PLP, or carries it in a form that the stack does
    /// not decode (`refused`).
    pub plp: Option<PartPlp>,
    /// The subframe carries the chosen PLP and the stack does not decode it there.
    pub refused: bool,
}

/// The first part of a frame, read: the signalling and the PLP to decode. The fields of the PLP are those of the first
/// subframe that carries it.
#[derive(Clone, Debug)]
pub struct Head {
    /// What the signalling says.
    pub info: Arc<FrameInfo>,
    /// The id of the PLP that is decoded.
    pub id: u8,
    /// The first subframe that carries it: its place in [`FrameDecoder::parts`].
    pub part: usize,
    /// The PLP that is decoded: its place among the PLPs of that subframe.
    pub plp: usize,
    /// Its coding.
    pub coding: PlpCoding,
    /// The index of its first data cell and its data cells.
    pub span: (usize, usize),
    /// Its subslices (1 for a non-dispersed PLP) and the cells from the start of one to the start of the next.
    pub subslices: (usize, usize),
    /// Its time interleaving in this frame.
    pub ti: TiFrame,
    /// The first cell of its first whole FEC block, `None` when no block begins in this frame or the PLP is time
    /// interleaved.
    pub fec_block_start: Option<usize>,
    /// The carrier offset of the frame, Hz.
    pub cfo_hz: f64,
    /// The samples from the start of the bootstrap to the end of the first subframe.
    pub samples: usize,
    /// The samples of the whole frame, when the signalling gives them: the next bootstrap starts after them.
    pub frame_samples: Option<usize>,
    /// The samples from the start of the bootstrap to the first symbol of the subframe.
    body_at: usize,
}

/// What [`FrameDecoder::head`] gives.
#[derive(Clone, Debug)]
pub enum HeadResult {
    /// More samples are needed: this many from the start of the bootstrap.
    Need(usize),
    /// The frame is not decoded, with what was read of its signalling.
    Failed(NotDecoded, Option<Arc<FrameInfo>>),
    /// The signalling is read.
    Ready(Head),
}

/// The cells of the decoded PLP in one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlpCells {
    /// The equalised cells.
    pub cells: Vec<Complex32>,
    /// Their weights `|H|^2 / sigma^2`; 0 erases a cell.
    pub weights: Vec<f32>,
    /// The signal to noise ratio of the pilots of the subframe, dB.
    pub pilot_snr_db: f32,
}

/// One symbol with what its demodulation needs, made once per frame layout.
struct SymbolPlan {
    symbol: Symbol,
    /// The carrier of each data cell, lowest first.
    data_at: Vec<u32>,
    /// The frequency interleaving sequence, empty when the symbol is not interleaved.
    sequence: Vec<u32>,
    /// The continual pilots off the grid: they measure the noise.
    noise_at: Vec<u32>,
}

impl SymbolPlan {
    fn new(symbol: Symbol) -> SymbolPlan {
        let fft = symbol.carriers.fft;
        let cred = ((fft.max_carriers() - symbol.carriers.len()) / fft.reduction_unit()) as u8;
        SymbolPlan {
            data_at: symbol.carriers.data_carriers().map(|k| k as u32).collect(),
            sequence: if symbol.interleaved {
                frequency_interleaver(fft, symbol.l, symbol.carriers.data_cells())
            } else {
                Vec::new()
            },
            noise_at: common_pilots(fft, cred)
                .into_iter()
                .filter(|k| k % symbol.grid != 0)
                .map(|k| k as u32)
                .collect(),
            symbol,
        }
    }
}

/// The Preamble symbols of a structure.
struct PreamblePlan {
    code: u8,
    cred: u8,
    symbols: Vec<SymbolPlan>,
}

/// What the symbols of a subframe follow from: its waveform, the PAPR code, the excess samples of each guard, the
/// frequency interleaver, the null cells and the number of its first symbol.
type SubframeKey = (SubframeParams, u8, usize, bool, Option<u16>, usize);

/// The symbols of a subframe.
struct SubframePlan {
    key: SubframeKey,
    symbols: Vec<SymbolPlan>,
}

/// Buffers that the decoder keeps from frame to frame.
#[derive(Default)]
struct Scratch {
    mixed: Vec<Complex32>,
    window: Vec<Complex32>,
    y: Vec<Vec<Complex32>>,
    h: Vec<Vec<Complex32>>,
    grid: Vec<Complex32>,
    z: Vec<Complex32>,
    w: Vec<f32>,
    zx: Vec<Complex32>,
    wx: Vec<f32>,
    soft: Vec<f32>,
    llr: Vec<i8>,
    bits: Vec<u8>,
}

/// Demodulates frames. After the first frames of a layout it allocates nothing.
#[derive(Default)]
pub struct FrameDecoder {
    ffts: [Option<Fft>; 3],
    basic_code: Option<L1Code>,
    detail_code: Option<L1Code>,
    points: Vec<(usize, u8, Constellation)>,
    preamble: Option<PreamblePlan>,
    /// The symbols of each subframe of the frame layout read last.
    subframes: Vec<SubframePlan>,
    /// The subframes of the frame read last that the stack takes.
    parts: Vec<Part>,
    basic: Option<L1Basic>,
    detail: L1Detail,
    /// The last signalling given out, with and without the PLPs of L1-Detail.
    info: [Option<Arc<FrameInfo>>; 2],
    s: Scratch,
    /// The cells of the Preamble symbols after L1-Basic, and the cells of L1-Detail in the order of its blocks.
    l1: (Vec<Complex32>, Vec<f32>),
    blocks: (Vec<Complex32>, Vec<f32>),
    detail_bits: Vec<u8>,
    /// Where the data cells begin in `l1`.
    data_from: usize,
    /// The id of the PLP to decode. `None`: see [`FrameDecoder::select`].
    want: Option<u8>,
}

/// The subslices of `p` and their interval when its cells lie within the first `available` data cells of the subframe
/// (clause 7.2.6: a non-dispersed PLP is one run of cells; a dispersed PLP is cut into subslices of `ceil(size / count)`
/// cells, the last one of what is left, that do not touch).
fn subslices_of(p: &Plp, available: usize) -> Option<(usize, usize)> {
    let (start, size) = (p.start as usize, p.size as usize);
    let (count, interval) = match (p.dispersed?, p.subslices) {
        (false, _) => (1, 0),
        (true, Some((n, interval))) => (usize::from(n), interval as usize),
        (true, None) => return None,
    };
    if size == 0 || count == 0 {
        return None;
    }
    let each = size.div_ceil(count);
    let last = size.checked_sub((count - 1) * each).filter(|&n| n > 0)?;
    let fits = (count == 1 || (count > 1 && interval > each))
        && start + (count - 1) * interval + last <= available;
    fits.then_some((count, interval))
}

/// Fills the channel `out` of every carrier from the estimates `grid` at every `step`-th carrier (the last carrier is one of
/// them): a straight line between neighbours after the mean delay is taken out.
fn across_carriers(step: usize, grid: &[Complex32], out: &mut [Complex32]) {
    let turn: Complex32 = grid.windows(2).map(|w| w[1] * w[0].conj()).sum();
    let slope = if turn.norm_sqr() > 0.0 {
        f64::from(turn.arg()) / step as f64
    } else {
        0.0
    };
    // One turn per carrier, taken out of the grid and put back on every carrier.
    let (sin, cos) = slope.sin_cos();
    let per_carrier = Complex32::new(cos as f32, sin as f32);
    let per_step = {
        let (s, c) = (slope * step as f64).sin_cos();
        Complex32::new(c as f32, -s as f32)
    };
    let mut back = Complex32::new(1.0, 0.0);
    let mut a = grid[0];
    let mut fwd = Complex32::new(1.0, 0.0);
    for (i, w) in grid.windows(2).enumerate() {
        back *= per_step;
        let b = w[1] * back;
        let k = i * step;
        for c in 0..step {
            let t = c as f32 / step as f32;
            out[k + c] = (a + (b - a) * t) * fwd;
            fwd *= per_carrier;
        }
        // The products drift from the unit circle over thousands of carriers.
        fwd /= fwd.norm();
        back /= back.norm();
        a = b;
    }
    if let (Some(o), Some(&g)) = (out.get_mut((grid.len() - 1) * step), grid.last()) {
        *o = g;
    }
}

/// The carriers of symbol `s` whose guard starts at sample `at` of `x`, into `y`.
fn carriers(
    fft: &mut Fft,
    window: &mut Vec<Complex32>,
    s: &Symbol,
    x: &[Complex32],
    at: usize,
    y: &mut Vec<Complex32>,
) {
    let n = s.carriers.fft.size();
    let start = at + s.guard - BACKOFF.min(s.guard);
    window.clear();
    window.extend_from_slice(&x[start..start + n]);
    fft.process(window);
    y.clear();
    y.extend((0..s.carriers.len()).map(|k| window[s.carriers.bin(k)]));
}

/// Adds the pilot noise of one symbol to `(signal, noise, pilots)`. Returns the excess of the error over the noise of a
/// pilot: the error holds the noise of the estimate under it too. `spread` is the share of the noise of one grid estimate
/// that the estimate between grid carriers keeps.
fn pilot_noise(
    p: &SymbolPlan,
    y: &[Complex32],
    h: &[Complex32],
    spread: f64,
    sum: &mut (f64, f64, usize),
) -> f64 {
    let mut excess = 1.0;
    let grid = f64::from(p.symbol.carriers.pilot[0]);
    for &k in &p.noise_at {
        let k = k as usize;
        let pilot = p.symbol.carriers.pilot[k];
        let e = y[k] / pilot - h[k];
        sum.0 += f64::from(h[k].norm_sqr());
        sum.1 += f64::from(e.norm_sqr()) * f64::from(pilot * pilot);
        sum.2 += 1;
        excess = 1.0 + f64::from(pilot * pilot) * spread / (grid * grid);
    }
    excess
}

/// The signal to noise ratio (linear, for a data cell of unit power) of the sums of [`pilot_noise`].
fn snr_of(sum: (f64, f64, usize), excess: f64) -> f32 {
    if sum.2 == 0 || sum.1 <= 0.0 {
        return 1e6;
    }
    (sum.0 * excess / sum.1).clamp(1e-3, 1e6) as f32
}

/// The equalised data cells of a demodulated symbol and their weights, in the order before the frequency interleaver, into
/// `s.zx` and `s.wx`.
fn data_cells(p: &SymbolPlan, y: &[Complex32], h: &[Complex32], snr: f32, s: &mut Scratch) {
    let mean: f32 = h.iter().map(|h| h.norm_sqr()).sum::<f32>() / h.len().max(1) as f32;
    let floor = mean * 1e-4;
    s.z.clear();
    s.w.clear();
    for &k in &p.data_at {
        let k = k as usize;
        let power = h[k].norm_sqr();
        if power > floor && power.is_finite() && y[k].re.is_finite() && y[k].im.is_finite() {
            s.z.push(y[k] / h[k]);
            s.w.push(power / mean * snr);
        } else {
            s.z.push(Complex32::default());
            s.w.push(0.0);
        }
    }
    if p.sequence.is_empty() {
        std::mem::swap(&mut s.z, &mut s.zx);
        std::mem::swap(&mut s.w, &mut s.wx);
        return;
    }
    let n = s.z.len();
    s.zx.clear();
    s.zx.resize(n, Complex32::default());
    s.wx.clear();
    s.wx.resize(n, 0.0);
    // Clause 7.3: the even symbol of a 32K pair is written by the sequence, every other symbol is read by it.
    if p.symbol.carriers.fft == FftSize::K32 && p.symbol.l.is_multiple_of(2) {
        for (i, &to) in p.sequence.iter().enumerate() {
            s.zx[i] = s.z[to as usize];
            s.wx[i] = s.w[to as usize];
        }
    } else {
        for (i, &to) in p.sequence.iter().enumerate() {
            s.zx[to as usize] = s.z[i];
            s.wx[to as usize] = s.w[i];
        }
    }
}

/// Decodes one L1 block of `k_sig` bits from its cells into `s.bits` (descrambled). False when it does not decode.
fn l1_block(
    code: &mut L1Code,
    points: &Constellation,
    cells: &[Complex32],
    weights: &[f32],
    k_sig: usize,
    s: &mut Scratch,
) -> bool {
    s.soft.clear();
    s.soft.resize(cells.len() * code.bits_per_cell(), 0.0);
    if points
        .demap_block_weighted(cells, weights, Demap::MaxLog, &mut s.soft)
        .is_err()
    {
        return false;
    }
    s.llr.clear();
    s.llr.extend(
        s.soft
            .iter()
            .map(|&l| (l * LLR_GAIN).round().clamp(-127.0, 127.0) as i8),
    );
    s.bits.clear();
    s.bits.resize(k_sig, 0);
    for iterations in L1_ITERATIONS {
        if code.decode(&s.llr, k_sig, iterations, &mut s.bits).is_ok() {
            Scrambler::apply_bits(&mut s.bits);
            return true;
        }
    }
    false
}

/// The samples of the whole frame from the start of its bootstrap (clauses 7.2.2.2 and 8.5.1), where `first` samples reach
/// the end of the first subframe. `None` when a later subframe has a reserved FFT size or guard interval.
fn frame_samples(basic: &L1Basic, detail: &L1Detail, first: usize) -> Option<usize> {
    match basic.frame_length {
        FrameLength::TimeAligned { units_5ms, .. } => {
            Some(usize::from(units_5ms) * 30_720).filter(|&n| n >= first)
        }
        FrameLength::SymbolAligned {
            additional_samples, ..
        } => detail.subframes.iter().skip(1).try_fold(
            first + usize::from(additional_samples),
            |n, s| {
                let p = s.params?;
                let symbol =
                    FftSize::from_code(p.fft_size)?.size() + guard_samples(p.guard_interval)?;
                Some(n + usize::from(p.symbols) * symbol)
            },
        ),
    }
}

impl FrameDecoder {
    /// Chooses the PLP to decode by its id. `None`: the first PLP that the stack decodes and that carries low level
    /// signalling, or without one the first PLP that the stack decodes.
    pub fn select(&mut self, id: Option<u8>) {
        self.want = id;
    }

    /// The L1-Basic and the L1-Detail of the last frame whose [`FrameDecoder::head`] was `Ready`.
    pub fn signalling(&self) -> Option<(&L1Basic, &L1Detail)> {
        self.basic.as_ref().map(|b| (b, &self.detail))
    }

    /// The place of the constellation of `bits` bits per cell at `rate` in `self.points`.
    fn points(&mut self, bits: usize, rate: u8) -> Option<usize> {
        if let Some(i) = self.points.iter().position(|p| p.0 == bits && p.1 == rate) {
            return Some(i);
        }
        self.points.push((bits, rate, constellation(bits, rate)?));
        Some(self.points.len() - 1)
    }

    /// The signalling of `structure`, `basic` and `plps` as a shared value: the one given out last when nothing changed.
    fn info(
        &mut self,
        structure: &PreambleStructure,
        basic: &L1Basic,
        subframes: Option<&[DetailSubframe]>,
    ) -> Arc<FrameInfo> {
        let slot = &mut self.info[usize::from(subframes.is_some())];
        let plps = subframes.unwrap_or(&[]);
        match slot {
            Some(i) if i.says(structure, basic, plps) => i.clone(),
            _ => {
                let i = Arc::new(FrameInfo {
                    fft: structure.fft,
                    guard: structure.guard,
                    l1_basic_mode: structure.l1_basic_mode,
                    l1_detail_mode: basic.detail_mode,
                    preamble_symbols: basic.preamble_symbols,
                    subframes: basic.subframes,
                    symbols: basic.first.symbols,
                    pilot_pattern: basic.first.pilot_pattern,
                    plps: plps
                        .iter()
                        .enumerate()
                        .flat_map(|(k, s)| s.plps.iter().map(move |p| PlpInfo::of(k, p)))
                        .collect(),
                });
                *slot = Some(i.clone());
                i
            }
        }
    }

    /// Demodulates Preamble symbol `i` of the plan from the mixed samples at `at` and appends its data cells to `self.l1`.
    /// Returns the cells added.
    fn preamble_symbol(&mut self, i: usize, at: usize) -> usize {
        let Some(plan) = &self.preamble else {
            return 0;
        };
        let p = &plan.symbols[i];
        let s = &mut self.s;
        if s.y.is_empty() {
            s.y.push(Vec::new());
            s.h.push(Vec::new());
        }
        let fft = self.ffts[usize::from(p.symbol.carriers.fft.code())]
            .get_or_insert_with(|| Fft::forward(p.symbol.carriers.fft.size()));
        let (mut y, mut h) = (std::mem::take(&mut s.y[0]), std::mem::take(&mut s.h[0]));
        carriers(fft, &mut s.window, &p.symbol, &s.mixed, at, &mut y);
        s.grid.clear();
        s.grid.extend(
            (0..y.len())
                .step_by(p.symbol.grid)
                .map(|k| y[k] / p.symbol.carriers.pilot[k]),
        );
        h.clear();
        h.resize(y.len(), Complex32::default());
        across_carriers(p.symbol.grid, &s.grid, &mut h);
        let mut sum = (0.0, 0.0, 0);
        let excess = pilot_noise(p, &y, &h, PREAMBLE_SPREAD, &mut sum);
        data_cells(p, &y, &h, snr_of(sum, excess), s);
        self.l1.0.extend_from_slice(&s.zx);
        self.l1.1.extend_from_slice(&s.wx);
        let n = s.zx.len();
        s.y[0] = y;
        s.h[0] = h;
        n
    }

    /// Reads the first part of the frame whose bootstrap with fields `fields` starts at `x[0]` (6.144 Msps) and has the
    /// carrier offset `cfo_hz` within a third of a carrier spacing: the carrier offset from the guard of the first Preamble
    /// symbol, L1-Basic, L1-Detail, and the PLP to decode.
    pub fn head(&mut self, fields: &Fields, x: &[Complex32], cfo_hz: f64) -> HeadResult {
        self.basic = None;
        let Some(structure) = PreambleStructure::of(fields.preamble) else {
            return HeadResult::Failed(NotDecoded::Structure, None);
        };
        if fields.rate_n != 0 {
            return HeadResult::Failed(NotDecoded::SampleRate, None);
        }
        if self
            .preamble
            .as_ref()
            .is_none_or(|p| p.code != fields.preamble)
        {
            self.preamble = Some(PreamblePlan {
                code: fields.preamble,
                cred: 0,
                symbols: preamble_symbols(&structure, 1, 0)
                    .into_iter()
                    .map(SymbolPlan::new)
                    .collect(),
            });
        }
        let start = 4 * bootstrap::SYM;
        let symbol = structure.fft.size() + structure.guard;
        let need = start + symbol;
        if x.len() < need {
            return HeadResult::Need(need);
        }
        // The guard repeats the end of the useful part: the turn between them is the carrier offset left.
        let n = structure.fft.size();
        let mut osc = Nco::new(bootstrap::RATE, -cfo_hz);
        self.s.mixed.clear();
        self.s.mixed.resize(need, Complex32::default());
        osc.mix(&x[..need], &mut self.s.mixed);
        let edge = (structure.guard / 8).min(BACKOFF);
        let turn: Complex32 = (start + edge..start + structure.guard - edge)
            .map(|i| self.s.mixed[i] * self.s.mixed[i + n].conj())
            .sum();
        let cfo_hz =
            cfo_hz - f64::from(turn.arg()) * bootstrap::RATE / (std::f64::consts::TAU * n as f64);
        let mut osc = Nco::new(bootstrap::RATE, -cfo_hz);
        osc.mix(&x[..need], &mut self.s.mixed);

        // L1-Basic: the first cells of the first Preamble symbol.
        self.l1.0.clear();
        self.l1.1.clear();
        self.preamble_symbol(0, start);
        if self
            .basic_code
            .as_ref()
            .is_none_or(|c| c.mode() != structure.l1_basic_mode)
        {
            self.basic_code = L1Code::new(L1Kind::Basic, structure.l1_basic_mode).ok();
        }
        let Some(code) = &self.basic_code else {
            return HeadResult::Failed(NotDecoded::Structure, None);
        };
        let (bits, rate) = (code.bits_per_cell(), code.constellation_rate());
        let basic_cells = code
            .lengths(L1_BASIC_BITS)
            .map_or(usize::MAX, |l| l.sent() / bits);
        let Some(points) = self.points(bits, rate) else {
            return HeadResult::Failed(NotDecoded::Structure, None);
        };
        if basic_cells > self.l1.0.len() {
            return HeadResult::Failed(NotDecoded::Structure, None);
        }
        let read = match &mut self.basic_code {
            Some(code) => l1_block(
                code,
                &self.points[points].2,
                &self.l1.0[..basic_cells],
                &self.l1.1[..basic_cells],
                L1_BASIC_BITS,
                &mut self.s,
            ),
            None => false,
        };
        let basic = match L1Basic::parse(&self.s.bits) {
            Ok(b) if read => b,
            _ => return HeadResult::Failed(NotDecoded::L1Basic, None),
        };
        if basic.preamble_reduced_carriers > 4 {
            let info = self.info(&structure, &basic, None);
            return HeadResult::Failed(NotDecoded::Signalling, Some(info));
        }

        // The other Preamble symbols.
        let count = usize::from(basic.preamble_symbols);
        if self.preamble.as_ref().is_none_or(|p| {
            p.symbols.len() != count || (count > 1 && p.cred != basic.preamble_reduced_carriers)
        }) {
            self.preamble = Some(PreamblePlan {
                code: fields.preamble,
                cred: basic.preamble_reduced_carriers,
                symbols: preamble_symbols(
                    &structure,
                    basic.preamble_symbols,
                    basic.preamble_reduced_carriers,
                )
                .into_iter()
                .map(SymbolPlan::new)
                .collect(),
            });
        }
        let body_at = start + count * symbol;
        if x.len() < body_at {
            return HeadResult::Need(body_at);
        }
        if count > 1 {
            self.s.mixed.resize(body_at, Complex32::default());
            osc.mix(&x[need..body_at], &mut self.s.mixed[need..]);
        }
        let mut last_from = 0;
        for i in 1..count {
            last_from = self.l1.0.len() - basic_cells;
            self.preamble_symbol(i, start + i * symbol);
        }
        let room = self.l1.0.len() - basic_cells;
        let total = basic.detail_cells as usize;
        if total > room {
            let info = self.info(&structure, &basic, None);
            return HeadResult::Failed(NotDecoded::Signalling, Some(info));
        }
        // Only the last Preamble symbol carries data cells after the signalling.
        self.data_from = basic_cells + total.max(last_from);

        match self.read_detail(&basic, basic_cells, count) {
            Ok(()) => {}
            Err(why) => {
                let info = self.info(&structure, &basic, None);
                return HeadResult::Failed(why, Some(info));
            }
        }
        let detail = std::mem::take(&mut self.detail);
        let result = self.plan(&structure, &basic, &detail, cfo_hz, body_at, count);
        self.detail = detail;
        self.basic = matches!(result, HeadResult::Ready(_)).then_some(basic);
        result
    }

    /// Reads L1-Detail into `self.detail` from the `total` cells after L1-Basic in `self.l1`.
    fn read_detail(
        &mut self,
        basic: &L1Basic,
        basic_cells: usize,
        columns: usize,
    ) -> Result<(), NotDecoded> {
        // Clause 7.2.5.2: the cells of L1-Detail were written row by row into one column per Preamble symbol.
        let total = basic.detail_cells as usize;
        let rows = total / columns;
        self.blocks.0.clear();
        self.blocks.0.resize(total, Complex32::default());
        self.blocks.1.clear();
        self.blocks.1.resize(total, 0.0);
        for n in 0..total {
            let m = if n < rows * columns {
                (n % rows) * columns + n / rows
            } else {
                n
            };
            self.blocks.0[m] = self.l1.0[basic_cells + n];
            self.blocks.1[m] = self.l1.1[basic_cells + n];
        }
        if self
            .detail_code
            .as_ref()
            .is_none_or(|c| c.mode() != basic.detail_mode)
        {
            self.detail_code = L1Code::new(L1Kind::Detail, basic.detail_mode).ok();
        }
        let Some(code) = &self.detail_code else {
            return Err(NotDecoded::Signalling);
        };
        let size = usize::from(basic.detail_bytes) * 8;
        let (blocks, k_sig) = detail_segments(size, code.max_k_sig());
        let (bits, rate) = (code.bits_per_cell(), code.constellation_rate());
        let block_cells = code
            .lengths(k_sig)
            .map(|l| l.sent() / bits)
            .map_err(|_| NotDecoded::Signalling)?;
        let points = self.points(bits, rate).ok_or(NotDecoded::Signalling)?;
        if blocks * block_cells > total {
            return Err(NotDecoded::Signalling);
        }
        if blocks > 1 && basic.detail_additional_parity != 0 {
            return Err(NotDecoded::AdditionalParity);
        }
        self.detail_bits.clear();
        for b in 0..blocks {
            let r = b * block_cells..(b + 1) * block_cells;
            let read = match &mut self.detail_code {
                Some(code) => l1_block(
                    code,
                    &self.points[points].2,
                    &self.blocks.0[r.clone()],
                    &self.blocks.1[r],
                    k_sig,
                    &mut self.s,
                ),
                None => false,
            };
            if !read {
                return Err(NotDecoded::L1Detail);
            }
            self.detail_bits.extend_from_slice(&self.s.bits);
        }
        self.detail
            .parse_into(&self.detail_bits, basic)
            .map_err(|_| NotDecoded::L1Detail)
    }

    /// The subframes of the frame whose [`FrameDecoder::head`] was `Ready` last, in order. The list ends before a subframe
    /// that the stack does not take: one with a waveform that it does not decode, or one that ends more than 1 s after the
    /// bootstrap.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// The symbols of every subframe, and the PLP to decode in each.
    fn plan(
        &mut self,
        structure: &PreambleStructure,
        basic: &L1Basic,
        detail: &L1Detail,
        cfo_hz: f64,
        body_at: usize,
        preamble: usize,
    ) -> HeadResult {
        self.parts.clear();
        let Some(first) = detail.subframes.first() else {
            let info = self.info(structure, basic, None);
            return HeadResult::Failed(NotDecoded::Signalling, Some(info));
        };
        let info = self.info(structure, basic, Some(&detail.subframes));
        let failed = |why| HeadResult::Failed(why, Some(info.clone()));
        let count = usize::from(basic.subframes).min(detail.subframes.len());
        let mut at = body_at;
        for (k, sub) in detail.subframes[..count].iter().enumerate() {
            let params = match (k, sub.params) {
                (0, _) => basic.first,
                (_, Some(p)) => p,
                (_, None) => break,
            };
            // Clause 8.5.1: every symbol after the Preamble holds the excess samples of a time-aligned frame. The symbols
            // of a later subframe are numbered from 0 (clauses 7.3 and 8.1.3).
            let extra = excess_samples(basic);
            let first_l = if k == 0 { preamble } else { 0 };
            let key: SubframeKey = (
                params,
                basic.papr,
                extra,
                sub.frequency_interleaver,
                sub.sbs_null_cells,
                first_l,
            );
            if self.subframes.get(k).is_none_or(|p| p.key != key) {
                let symbols = match subframe_symbols(&params, basic.papr, extra, sub, first_l) {
                    Ok(s) => s,
                    Err(why) if k == 0 => return failed(why),
                    Err(_) => break,
                };
                let plan = SubframePlan {
                    key,
                    symbols: symbols.into_iter().map(SymbolPlan::new).collect(),
                };
                self.subframes.truncate(k);
                self.subframes.push(plan);
            }
            let samples = at
                + self.subframes[k]
                    .symbols
                    .iter()
                    .map(|p| p.symbol.samples())
                    .sum::<usize>();
            if samples > MAX_SAMPLES {
                if k == 0 {
                    return failed(NotDecoded::Length);
                }
                break;
            }
            self.parts.push(Part {
                subframe: k,
                from: at,
                samples,
                plp: None,
                refused: false,
            });
            at = samples;
        }
        if first.plps.iter().any(|p| p.layer > 0) {
            return failed(NotDecoded::Ldm);
        }
        // The data cells of a subframe: the active cells of its symbols and, in the first subframe, the cells of the last
        // Preamble symbol after the signalling before them.
        let leftover = self.l1.0.len() - self.data_from;
        let (plans, parts) = (&self.subframes, &mut self.parts);
        let decodable = |k: usize, p: &Plp| {
            let sub = &detail.subframes[k];
            if p.layer != 0 || p.scrambler != 0 || sub.plps.iter().any(|p| p.layer > 0) {
                return None;
            }
            let available = if k == 0 { leftover } else { 0 }
                + plans[k]
                    .symbols
                    .iter()
                    .map(|p| p.symbol.active)
                    .sum::<usize>();
            Some(PartPlp {
                coding: PlpCoding::of(p)?,
                span: (p.start as usize, p.size as usize),
                subslices: subslices_of(p, available)?,
                ti: TiFrame::of(p)?,
                fec_block_start: p.fec_block_start.filter(|&s| s != 0x7FFF).map(usize::from),
            })
        };
        let plps = |k: usize| detail.subframes[k].plps.iter();
        let id = self.want.or_else(|| {
            let taken = || (0..parts.len()).flat_map(|k| plps(k).map(move |p| (k, p)));
            taken()
                .find(|(k, p)| p.lls && decodable(*k, p).is_some())
                .or_else(|| taken().find(|(k, p)| decodable(*k, p).is_some()))
                .map(|(_, p)| p.id)
        });
        let Some(id) = id else {
            return failed(NotDecoded::Plp);
        };
        let mut carried = None;
        for (k, part) in parts.iter_mut().enumerate() {
            let Some(n) = plps(k).position(|p| p.id == id) else {
                continue;
            };
            part.plp = decodable(k, &detail.subframes[k].plps[n]);
            part.refused = part.plp.is_none();
            if let (None, Some(plp)) = (carried, part.plp) {
                carried = Some((k, n, plp));
            }
        }
        let Some((part, plp, first)) = carried else {
            return failed(NotDecoded::Plp);
        };
        let samples = parts[0].samples;
        HeadResult::Ready(Head {
            info,
            id,
            part,
            plp,
            coding: first.coding,
            span: first.span,
            subslices: first.subslices,
            ti: first.ti,
            fec_block_start: first.fec_block_start,
            cfo_hz,
            samples,
            frame_samples: frame_samples(basic, detail, samples),
            body_at,
        })
    }

    /// The cells of the PLP of `head` in the first subframe that carries it, into `out`: [`FrameDecoder::part`] for that
    /// subframe.
    pub fn body(&mut self, head: &Head, x: &[Complex32], out: &mut PlpCells) -> bool {
        self.part(head, head.part, x, out)
    }

    /// The cells of the PLP of `head` in subframe `k` of [`FrameDecoder::parts`] into `out`, from the samples `x` of the
    /// frame (from the start of the bootstrap, at least [`Part::samples`]). `head` must be the last one that
    /// [`FrameDecoder::head`] gave. False when `x` is too short, `head` is not the last one or the subframe does not carry
    /// the PLP in a form that the stack decodes.
    pub fn part(&mut self, head: &Head, k: usize, x: &[Complex32], out: &mut PlpCells) -> bool {
        out.cells.clear();
        out.weights.clear();
        let (Some(part), Some(plan)) = (self.parts.get(k).copied(), self.subframes.get(k)) else {
            return false;
        };
        let Some(plp) = part.plp else {
            return false;
        };
        let samples: usize = plan.symbols.iter().map(|p| p.symbol.samples()).sum();
        if x.len() < part.samples
            || part.from + samples != part.samples
            || self.parts[0].from != head.body_at
        {
            return false;
        }
        let s = &mut self.s;
        s.mixed.clear();
        s.mixed.resize(samples, Complex32::default());
        Nco::new(bootstrap::RATE, -head.cfo_hz).mix(&x[part.from..part.samples], &mut s.mixed);
        let count = plan.symbols.len();
        while s.y.len() < count {
            s.y.push(Vec::new());
            s.h.push(Vec::new());
        }
        let Some(first) = plan.symbols.first() else {
            return false;
        };
        let fft = self.ffts[usize::from(first.symbol.carriers.fft.code())]
            .get_or_insert_with(|| Fft::forward(first.symbol.carriers.fft.size()));
        let mut at = 0;
        for (p, y) in plan.symbols.iter().zip(&mut s.y) {
            carriers(fft, &mut s.window, &p.symbol, &s.mixed, at, y);
            at += p.symbol.samples();
        }
        // The channel of every symbol on the grid: every carrier that bears a scattered pilot in some symbol.
        let step = first.symbol.grid;
        let noc = first.symbol.carriers.len();
        let raw = |y: &[Vec<Complex32>], i: usize, k: usize| -> Option<Complex32> {
            let pilot = plan.symbols[i].symbol.carriers.pilot[k];
            (pilot != 0.0).then(|| y[i][k] / pilot)
        };
        let mut sum = (0.0, 0.0, 0);
        let mut excess = 1.0;
        for i in 0..count {
            s.grid.clear();
            for k in (0..noc).step_by(step) {
                let g = match raw(&s.y, i, k) {
                    Some(h) => h,
                    None => {
                        // The nearest symbols before and after with a pilot on this carrier.
                        let before = (0..i)
                            .rev()
                            .take(4)
                            .find_map(|j| raw(&s.y, j, k).map(|h| (i - j, h)));
                        let after = (i + 1..count)
                            .take(4)
                            .find_map(|j| raw(&s.y, j, k).map(|h| (j - i, h)));
                        match (before, after) {
                            (Some((a, ha)), Some((b, hb))) => {
                                (ha * b as f32 + hb * a as f32) / (a + b) as f32
                            }
                            (Some((_, h)), None) | (None, Some((_, h))) => h,
                            (None, None) => Complex32::default(),
                        }
                    }
                };
                s.grid.push(g);
            }
            let h = &mut s.h[i];
            h.clear();
            h.resize(noc, Complex32::default());
            across_carriers(step, &s.grid, h);
            excess = pilot_noise(&plan.symbols[i], &s.y[i], h, SUBFRAME_SPREAD, &mut sum);
        }
        let snr = snr_of(sum, excess);
        // The data cells of the subframe in order: in the first subframe the last Preamble symbol after the signalling,
        // then the active cells of each symbol. The PLP is one run of them, or one run per subslice.
        let (from, size) = plp.span;
        let (slices, interval) = plp.subslices;
        let each = size.div_ceil(slices.max(1));
        let (mut index, mut slice) = (0, 0);
        let mut take = |z: &[Complex32], w: &[f32], out: &mut PlpCells| {
            let end = index + z.len();
            while slice < slices {
                let start = from + slice * interval;
                let stop = start + each.min(size - slice * each);
                let (a, b) = (start.max(index), stop.min(end));
                if a < b {
                    out.cells.extend_from_slice(&z[a - index..b - index]);
                    out.weights.extend_from_slice(&w[a - index..b - index]);
                }
                if stop > end {
                    break;
                }
                slice += 1;
            }
            index = end;
        };
        if k == 0 {
            take(
                &self.l1.0[self.data_from..],
                &self.l1.1[self.data_from..],
                out,
            );
        }
        for i in 0..count {
            let p = &plan.symbols[i];
            let (y, h) = (std::mem::take(&mut s.y[i]), std::mem::take(&mut s.h[i]));
            data_cells(p, &y, &h, snr, s);
            s.y[i] = y;
            s.h[i] = h;
            let r = p.symbol.null_low..p.symbol.null_low + p.symbol.active;
            take(&s.zx[r.clone()], &s.wx[r], out);
        }
        out.pilot_snr_db = 10.0 * snr.log10();
        out.cells.len() == size
    }
}

#[cfg(test)]
mod tests {
    use super::super::bicm::{Outer, PlpDecoder, PlpStream};
    use super::super::gen::{Atsc3GenParams, FrameBuilder, Payload};
    use super::*;

    /// Builds `frames` frames of `p` and decodes them without a channel: every baseband packet must come back.
    fn loop_back(p: Atsc3GenParams, frames: usize) -> usize {
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let mut rx = FrameDecoder::default();
        let mut stream = PlpStream::new(PlpDecoder::new(p.coding).unwrap());
        let mut want = Payload::new(p.seed);
        let mut packets = 0;
        let mut cells = PlpCells::default();
        for f in 0..frames {
            let x = b.frame();
            assert_eq!(x.len(), b.frame_samples());
            let head = match rx.head(&p.fields, &x, 0.0) {
                HeadResult::Ready(h) => h,
                other => panic!("frame {f}: {other:?}"),
            };
            // The signalling reads back as the generator set it.
            let (basic, detail) = rx.signalling().unwrap();
            assert_eq!((basic, detail), (b.basic(), b.detail()), "frame {f}");
            assert_eq!(head.frame_samples, Some(x.len()));
            assert!(head.samples <= x.len() && x.len() - head.samples < p.symbols);
            assert_eq!(head.span.0, p.plp_start);
            assert!(head.cfo_hz.abs() < 1.0, "{}", head.cfo_hz);
            assert_eq!(head.coding, p.coding);
            assert_eq!(head.info.symbols as usize, p.symbols);
            assert_eq!(
                head.fec_block_start,
                b.detail().subframes[0].plps[0]
                    .fec_block_start
                    .map(usize::from)
            );
            assert!(rx.body(&head, &x, &mut cells));
            assert_eq!(cells.cells.len(), head.span.1);
            assert!(cells.pilot_snr_db > 30.0, "{}", cells.pilot_snr_db);
            stream.frame(
                &cells.cells,
                &cells.weights,
                head.fec_block_start,
                50,
                |r, packet| {
                    assert!(r.good, "frame {f}");
                    assert_eq!(r.outer_corrected, 0);
                    assert_eq!(
                        packet,
                        &want.packet(p.coding.packet_bytes())[..],
                        "frame {f}"
                    );
                    packets += 1;
                },
            );
        }
        packets
    }

    /// Builds `frames` frames of `p`, adds white noise at `snr_db` (the signal has unit power in 6.144 MHz) and decodes them.
    /// Returns the frames whose signalling was read, the frames with every block good, the good blocks and all blocks. Every
    /// good baseband packet must be one of the generator, in order.
    fn through_noise(p: &Atsc3GenParams, snr_db: f64, seed: u64, frames: usize) -> [usize; 4] {
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let mut rx = FrameDecoder::default();
        let mut stream = PlpStream::new(PlpDecoder::new(p.coding).unwrap());
        let mut payload = Payload::new(p.seed);
        let bytes = p.coding.packet_bytes();
        let want: Vec<Vec<u8>> = (0..16 * frames + 16)
            .map(|_| payload.packet(bytes))
            .collect();
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        let power = 10f64.powf(-snr_db / 10.0);
        let mut at = 0;
        let mut count = [0usize; 4];
        let mut cells = PlpCells::default();
        for _ in 0..frames {
            let mut x = b.frame();
            x.iter_mut().for_each(|v| *v += rng.complex_normal(power));
            let HeadResult::Ready(head) = rx.head(&p.fields, &x, 0.0) else {
                stream.restart();
                continue;
            };
            count[0] += 1;
            assert!(rx.body(&head, &x, &mut cells));
            let (mut blocks, mut good) = (0, 0);
            stream.frame(
                &cells.cells,
                &cells.weights,
                head.fec_block_start,
                50,
                |r, packet| {
                    blocks += 1;
                    if r.good {
                        good += 1;
                        let found = want[at..].iter().position(|w| w == packet);
                        at += found.expect("a packet of the generator, in order") + 1;
                    }
                },
            );
            count[1] += usize::from(blocks == good);
            count[2] += good;
            count[3] += blocks;
        }
        count
    }

    /// The default frame in white noise at 7 dB (0.74 dB more on each carrier, since 6913 of 8192 carry the power): every
    /// frame of 3 seeds decodes and every baseband packet is the one sent. L1-Basic mode 4 of preamble structure 3 sets this
    /// limit: with L1-Basic mode 1 and L1-Detail mode 1 the same PLP decodes at 3.5 dB, and the signalling reads at -2 dB.
    #[test]
    fn frames_decode_in_noise() {
        let p = Atsc3GenParams::default();
        let mut robust = p.clone();
        robust.fields.preamble = 0;
        robust.l1_detail_mode = 1;
        for seed in [1, 2, 3] {
            assert_eq!(
                through_noise(&p, 7.0, seed, 3),
                [3, 3, 44, 44],
                "seed {seed}"
            );
            assert_eq!(
                through_noise(&robust, 3.5, seed, 3),
                [3, 3, 44, 44],
                "seed {seed}"
            );
        }
        let c = through_noise(&robust, -2.0, 1, 3);
        assert_eq!((c[0], c[1]), (3, 0));
    }

    #[test]
    fn the_default_frame_returns_every_baseband_packet() {
        let p = Atsc3GenParams::default();
        // 482596 cells a frame, 32400 cells a block: 14 blocks in the first frame and the block across the frames after.
        assert_eq!(loop_back(p, 3), 44);
    }

    #[test]
    fn other_options_return_every_baseband_packet() {
        let base = Atsc3GenParams::default();
        let coding = |ldpc_bits, rate, bits, outer| PlpCoding {
            ldpc_bits,
            rate,
            bits,
            outer,
        };
        let structure = |fft, guard, dx, mode| {
            let mut f = base.fields;
            f.preamble = PreambleStructure::code_of(fft, guard, dx, mode).unwrap();
            f
        };
        for (n, p) in [
            // 16 points, a 16200-bit code with a CRC, no frequency interleaver, L1-Basic mode 1 and L1-Detail mode 1, which
            // need two Preamble symbols.
            Atsc3GenParams {
                fields: structure(FftSize::K8, 1024, 3, 1),
                symbols: 12,
                pilots: (6, 2),
                pilot_boost: 3,
                reduced_carriers: 2,
                frequency_interleaver: false,
                coding: coding(16200, 6, 4, Outer::Crc),
                l1_detail_mode: 1,
                ..base.clone()
            },
            // 16K with 64 points, a type A code, L1-Basic mode 5 and L1-Detail mode 5.
            Atsc3GenParams {
                fields: structure(FftSize::K16, 768, 8, 5),
                symbols: 16,
                pilots: (16, 4),
                pilot_boost: 0,
                coding: coding(64800, 5, 6, Outer::None),
                l1_detail_mode: 5,
                ..base.clone()
            },
            // 32K with 256 points, L1-Basic mode 3 and L1-Detail mode 7.
            Atsc3GenParams {
                fields: structure(FftSize::K32, 2048, 6, 3),
                symbols: 9,
                pilots: (12, 2),
                pilot_boost: 4,
                reduced_carriers: 4,
                coding: coding(64800, 10, 8, Outer::Bch),
                l1_detail_mode: 7,
                ..base.clone()
            },
            // A time-aligned frame whose PLP begins after 1000 dummy cells, with an L1-Detail of two coded blocks (mode 1,
            // 400 reserved bytes) in four Preamble symbols.
            Atsc3GenParams {
                fields: structure(FftSize::K8, 192, 16, 2),
                symbols: 20,
                l1_detail_mode: 1,
                l1_detail_reserved: 400,
                time_aligned: true,
                plp_start: 1000,
                ..base.clone()
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert!(loop_back(p, 2) >= 3, "case {n}");
        }
        // The last case: its signalling is as the comment says.
        let mut last = base.clone();
        last.fields = structure(FftSize::K8, 192, 16, 2);
        (last.symbols, last.l1_detail_mode, last.l1_detail_reserved) = (20, 1, 400);
        (last.time_aligned, last.plp_start) = (true, 1000);
        let b = FrameBuilder::new(last).unwrap();
        assert_eq!(b.basic().preamble_symbols, 4);
        // 421 bytes are two blocks of 1688 bits: 9972 bits and 13644 repeated bits each, 11808 cells.
        assert_eq!(b.basic().detail_bytes, 421);
        assert_eq!(b.basic().detail_cells, 2 * 11808);
        // 12288 + 24 x 8384 = 213504 samples are 6.95 periods of 5 ms: 7 periods, 1536 samples more, 76 a symbol, 16 after.
        assert_eq!(
            b.basic().frame_length,
            FrameLength::TimeAligned {
                units_5ms: 7,
                excess_samples: 76
            }
        );
        assert_eq!(b.frame_samples(), 7 * 30_720);
    }

    /// Builds `frames` frames of `p` and decodes the PLP chosen by `want` through its time de-interleaver, without a
    /// channel. Every block must be good and every baseband packet one of the generator, in order. Returns the id of the
    /// PLP, the packets of all frames and those of the last frame.
    fn loop_back_plp(p: &Atsc3GenParams, want: Option<u8>, frames: usize) -> (u8, usize, usize) {
        use super::super::ti::TimeDeinterleaver;
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let mut rx = FrameDecoder::default();
        rx.select(want);
        // Without a choice: the PLP marked as a carrier of low level signalling, or the first.
        let marked = p.more_plps.iter().position(|m| m.lls);
        let id = want.unwrap_or(marked.map_or(0, |i| i as u8 + 1));
        let (coding, seed) = match id {
            0 => (p.coding, p.seed),
            n => {
                let more = &p.more_plps[usize::from(n) - 1];
                (more.coding, more.seed)
            }
        };
        let mut stream = PlpStream::new(PlpDecoder::new(coding).unwrap());
        let mut deinterleaver = TimeDeinterleaver::default();
        let mut payload = Payload::new(seed);
        let mut want_packets: Vec<Vec<u8>> = Vec::new();
        let mut at = 0;
        let mut cells = PlpCells::default();
        let (mut packets, mut last, mut records) = (0, 0, 0);
        for f in 0..frames {
            let x = b.frame();
            let head = match rx.head(&p.fields, &x, 0.0) {
                HeadResult::Ready(h) => h,
                other => panic!("frame {f}: {other:?}"),
            };
            let (basic, detail) = rx.signalling().unwrap();
            assert_eq!((basic, detail), (b.basic(), b.detail()), "frame {f}");
            let sent = &b.detail().subframes[head.part].plps[head.plp];
            assert_eq!(sent.id, id);
            assert_eq!(head.id, id);
            assert_eq!(head.coding, coding);
            assert_eq!(head.span, (sent.start as usize, sent.size as usize));
            assert_eq!(head.frame_samples, Some(x.len()));
            assert_eq!(rx.parts().len(), b.detail().subframes.len());
            last = 0;
            for k in 0..rx.parts().len() {
                let part = rx.parts()[k];
                assert!(!part.refused);
                let Some(plp) = part.plp else {
                    continue;
                };
                assert!(rx.part(&head, k, &x, &mut cells), "frame {f} subframe {k}");
                assert_eq!(cells.cells.len(), plp.span.1);
                assert!(cells.pilot_snr_db > 30.0, "{}", cells.pilot_snr_db);
                let d = deinterleaver
                    .frame(
                        &plp.ti,
                        records > 0,
                        coding.cells(),
                        &cells.cells,
                        &cells.weights,
                        plp.fec_block_start,
                    )
                    .unwrap();
                records += 1;
                if d.restart {
                    stream.restart();
                }
                while want_packets.len() < at + 2 * plp.span.1 / coding.cells() + 2 {
                    want_packets.push(payload.packet(coding.packet_bytes()));
                }
                stream.frame(d.cells, d.weights, d.block_start, 50, |r, packet| {
                    assert!(r.good, "frame {f} subframe {k}");
                    assert_eq!(r.outer_corrected, 0);
                    let found = want_packets[at..].iter().position(|w| w == packet);
                    let skipped = found.expect("a packet of the generator, in order");
                    // Only the blocks before the first whole one are lost.
                    assert!(packets == 0 || skipped == 0, "frame {f}: {skipped} lost");
                    at += skipped + 1;
                    packets += 1;
                    last += 1;
                });
            }
        }
        (id, packets, last)
    }

    fn short(rate: u8, bits: usize, outer: Outer) -> PlpCoding {
        PlpCoding {
            ldpc_bits: 16200,
            rate,
            bits,
            outer,
        }
    }

    /// One PLP through the convolutional time interleaver at each depth: after the lines are full, every FEC block of a
    /// frame decodes. A frame holds 482 596 cells, 59.6 blocks of 8100.
    #[test]
    fn frames_decode_through_the_convolutional_time_interleaver_at_every_depth() {
        use super::super::gen::GenTi;
        for (depth, extended, frames) in [
            (0u8, false, 2usize),
            (1, false, 3),
            (2, false, 3),
            (3, false, 4),
            (2, true, 5),
            (3, true, 6),
        ] {
            let p = Atsc3GenParams {
                coding: short(8, 2, Outer::Bch),
                time_interleaver: GenTi::Convolutional { depth, extended },
                ..Atsc3GenParams::default()
            };
            let (_, packets, last) = loop_back_plp(&p, None, frames);
            assert!(last >= 59, "depth {depth}: {last} blocks in the last frame");
            assert!(packets >= last, "depth {depth}");
            let b = FrameBuilder::new(p).unwrap();
            let rows = [[512, 724, 887, 1024], [512, 724, 1254, 1448]];
            assert_eq!(
                super::super::ti::TiFrame::of(&b.detail().subframes[0].plps[0]),
                Some(super::super::ti::TiFrame::Cti {
                    rows: rows[usize::from(extended)][usize::from(depth)],
                    start_row: 0,
                    fec_block_start: 0
                })
            );
        }
        // 16 points take the four depths without extended interleaving only, and a depth code above 3 is reserved.
        let p = Atsc3GenParams {
            coding: short(6, 4, Outer::Crc),
            time_interleaver: GenTi::Convolutional {
                depth: 1,
                extended: false,
            },
            ..Atsc3GenParams::default()
        };
        assert!(loop_back_plp(&p, None, 4).2 >= 119);
        for (depth, extended) in [(2, true), (4, false)] {
            let refused = Atsc3GenParams {
                time_interleaver: GenTi::Convolutional { depth, extended },
                ..p.clone()
            };
            assert!(FrameBuilder::new(refused).is_err());
        }
    }

    /// One PLP through the hybrid time interleaver: one and several TI blocks, with and without the cell interleaver, with
    /// and without room for more FEC blocks than the subframe carries. Every block of every frame decodes.
    #[test]
    fn frames_decode_through_the_hybrid_time_interleaver() {
        use super::super::gen::GenTi;
        for (n, (ti_blocks, cell_interleaver, spare, coding, blocks)) in [
            (1u8, false, 0u16, short(8, 2, Outer::Bch), 59usize),
            (1, true, 5, short(8, 2, Outer::Bch), 59),
            (4, true, 2, short(6, 4, Outer::Crc), 119),
            (3, true, 0, Atsc3GenParams::default().coding, 14),
            (16, false, 1, short(10, 8, Outer::None), 238),
        ]
        .into_iter()
        .enumerate()
        {
            let p = Atsc3GenParams {
                coding,
                time_interleaver: GenTi::Hybrid {
                    inter_subframe: false,
                    ti_blocks,
                    cell_interleaver,
                    spare,
                },
                ..Atsc3GenParams::default()
            };
            assert_eq!(loop_back_plp(&p, None, 2), (0, 2 * blocks, blocks), "case {n}");
            let b = FrameBuilder::new(p).unwrap();
            let plp = &b.detail().subframes[0].plps[0];
            assert_eq!(plp.size as usize, blocks * coding.cells());
            let h = plp.hti.unwrap();
            assert_eq!(usize::from(h.fec_blocks[0]) + 1, blocks);
            assert_eq!(h.fec_blocks_max - h.fec_blocks[0], spare);
        }
    }

    /// One PLP through the hybrid time interleaver across subframes: a TI block of 59 FEC blocks is spread over 1, 2, 3
    /// and 16 subframes, and every block decodes once the last subframe of its TI block is in. In the first subframes the
    /// PLP is smaller: the TI blocks before the first count as one FEC block each.
    #[test]
    fn frames_decode_through_the_hybrid_time_interleaver_across_subframes() {
        use super::super::gen::GenTi;
        for (units, cell_interleaver, spare) in [(1u8, true, 0u16), (2, false, 3), (3, true, 2), (16, true, 0)] {
            let p = Atsc3GenParams {
                coding: short(8, 2, Outer::Bch),
                time_interleaver: GenTi::Hybrid {
                    inter_subframe: true,
                    ti_blocks: units,
                    cell_interleaver,
                    spare,
                },
                ..Atsc3GenParams::default()
            };
            let frames = usize::from(units) + 2;
            assert_eq!(loop_back_plp(&p, None, frames), (0, 3 * 59, 59), "{units} subframes");
            let mut b = FrameBuilder::new(p).unwrap();
            b.frame();
            let plp = &b.detail().subframes[0].plps[0];
            let h = plp.hti.unwrap();
            assert!(h.inter_subframe && h.ti_blocks == units - 1);
            assert_eq!((h.fec_blocks[0], h.fec_blocks_max), (58, 58 + spare));
            // The first subframe: 8100 groups, one of 59 cells for each of 1 cell from every TI block before.
            let groups = |j: usize| 8100 / usize::from(units) + usize::from(j < 8100 % usize::from(units));
            let before: usize = (1..usize::from(units)).map(groups).sum();
            assert_eq!(plp.size as usize, 59 * groups(0) + before);
            assert!(units == 1 || h.fec_blocks[1] == 0);
        }
    }

    /// Two PLPs in one subframe, each decoded when it is chosen: one after the other (the second hybrid time interleaved),
    /// and both dispersed in 8 subslices with dummy cells between them.
    #[test]
    fn each_of_two_plps_decodes() {
        use super::super::gen::{GenPlp, GenTi};
        let second = GenPlp {
            coding: short(6, 4, Outer::Crc),
            seed: 77,
            start: 250_000,
            cells: None,
            subslices: 1,
            interval: 0,
            time_interleaver: GenTi::Hybrid {
                inter_subframe: false,
                ti_blocks: 2,
                cell_interleaver: true,
                spare: 0,
            },
            lls: false,
        };
        let after = Atsc3GenParams {
            coding: short(8, 2, Outer::Bch),
            plp_cells: Some(250_000),
            more_plps: vec![second],
            ..Atsc3GenParams::default()
        };
        // 250 000 cells are 30.9 blocks of 8100; 232 596 cells are 57 blocks of 4050.
        assert_eq!(loop_back_plp(&after, None, 2).0, 0);
        let (_, packets, last) = loop_back_plp(&after, Some(0), 2);
        assert!(packets >= 60 && last >= 30, "{packets} {last}");
        assert_eq!(loop_back_plp(&after, Some(1), 2), (1, 114, 57));
        // Without a choice the PLP with low level signalling is taken.
        let mut marked = after.clone();
        marked.more_plps[0].lls = true;
        assert_eq!(loop_back_plp(&marked, None, 1), (1, 57, 57));
        let dispersed = Atsc3GenParams {
            coding: short(8, 2, Outer::Bch),
            plp_cells: Some(240_000),
            subslices: (8, 60_000),
            more_plps: vec![GenPlp {
                start: 30_000,
                // 8 subslices of 25 001 cells, the last of 24 996.
                cells: Some(200_003),
                subslices: 8,
                interval: 60_000,
                time_interleaver: GenTi::None,
                ..second
            }],
            ..Atsc3GenParams::default()
        };
        let (_, packets, last) = loop_back_plp(&dispersed, Some(0), 2);
        assert!(packets >= 58 && last >= 29, "{packets} {last}");
        let (_, packets, last) = loop_back_plp(&dispersed, Some(1), 2);
        assert!(packets >= 97 && last >= 49, "{packets} {last}");
        // A PLP that the subframe does not hold, and PLPs that share cells.
        let mut b = FrameBuilder::new(dispersed.clone()).unwrap();
        let x = b.frame();
        let mut rx = FrameDecoder::default();
        rx.select(Some(2));
        assert!(matches!(
            rx.head(&dispersed.fields, &x, 0.0),
            HeadResult::Failed(NotDecoded::Plp, Some(_))
        ));
        let mut overlap = dispersed;
        overlap.more_plps[0].start = 29_999;
        assert!(FrameBuilder::new(overlap).is_err());
    }

    /// Two subframes after the first: 16K with a guard of 768 samples and SP16_4, and 32K with a guard of 2048 samples,
    /// SP12_2 and reduced carriers. Each carries `plps`.
    fn more_subframes(plps: &[super::super::gen::GenPlace]) -> Vec<super::super::gen::GenSubframe> {
        use super::super::gen::GenSubframe;
        vec![
            GenSubframe {
                fft: FftSize::K16,
                guard: 768,
                symbols: 16,
                reduced_carriers: 0,
                pilots: (16, 4),
                pilot_boost: 0,
                frequency_interleaver: true,
                boundary: (true, true),
                plps: plps.to_vec(),
            },
            GenSubframe {
                fft: FftSize::K32,
                guard: 2048,
                symbols: 9,
                reduced_carriers: 4,
                pilots: (12, 2),
                pilot_boost: 4,
                frequency_interleaver: false,
                boundary: (true, true),
                plps: plps.to_vec(),
            },
        ]
    }

    /// Frames of three subframes with three FFT sizes: a PLP that continues from subframe to subframe decodes without a
    /// time interleaver, through the convolutional time interleaver and through the hybrid one within and across
    /// subframes, in symbol-aligned and in time-aligned frames.
    #[test]
    fn a_plp_decodes_through_every_subframe_of_a_frame() {
        use super::super::gen::{GenPlace, GenTi};
        let whole = GenPlace {
            plp: 0,
            start: 0,
            cells: None,
            subslices: 1,
            interval: 0,
        };
        let base = Atsc3GenParams {
            symbols: 24,
            coding: short(8, 2, Outer::Bch),
            more_subframes: more_subframes(&[whole]),
            ..Atsc3GenParams::default()
        };
        let b = FrameBuilder::new(base.clone()).unwrap();
        let cells: Vec<u32> = b.detail().subframes.iter().map(|s| s.plps[0].size).collect();
        assert_eq!(cells.len(), 3);
        assert_eq!(b.basic().subframes, 3);
        // 8384 x 25 + 17152 x 16 + 34816 x 9 samples after the bootstrap.
        assert_eq!(b.frame_samples(), 12_288 + 209_600 + 274_432 + 313_344);
        // Time-aligned: 809 664 samples go into 27 periods of 5 ms, 19 776 samples more. Each of the 49 symbols after the
        // Preamble takes 403 of them (the 9 symbols of the last subframe alone would take 2197 each), and 29 follow.
        let aligned = FrameBuilder::new(Atsc3GenParams {
            time_aligned: true,
            ..base.clone()
        })
        .unwrap();
        assert_eq!(
            aligned.basic().frame_length,
            FrameLength::TimeAligned {
                units_5ms: 27,
                excess_samples: 403
            }
        );
        assert_eq!(aligned.frame_samples(), 27 * 30_720);
        let mut rx = FrameDecoder::default();
        let x = FrameBuilder::new(aligned.params().clone()).unwrap().frame();
        let HeadResult::Ready(_) = rx.head(&base.fields, &x, 0.0) else {
            panic!("the frame reads");
        };
        let ends: Vec<usize> = rx.parts().iter().map(|p| p.samples).collect();
        let first = 12_288 + 8384 + 24 * (8384 + 403);
        let second = first + 16 * (17_152 + 403);
        assert_eq!(ends, [first, second, second + 9 * (34_816 + 403)]);
        assert_eq!(ends[2] + 29, x.len());
        let per_frame = cells.iter().sum::<u32>() as usize / 8100;
        for (n, (time_interleaver, time_aligned, frames, least)) in [
            (GenTi::None, false, 2, per_frame),
            (GenTi::None, true, 2, per_frame),
            (
                GenTi::Convolutional {
                    depth: 0,
                    extended: false,
                },
                false,
                2,
                per_frame,
            ),
            (
                GenTi::Hybrid {
                    inter_subframe: false,
                    ti_blocks: 2,
                    cell_interleaver: true,
                    spare: 1,
                },
                true,
                2,
                per_frame - 3,
            ),
            (
                GenTi::Hybrid {
                    inter_subframe: true,
                    ti_blocks: 2,
                    cell_interleaver: true,
                    spare: 0,
                },
                false,
                3,
                3 * (*cells.iter().min().unwrap() as usize / 8100),
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let p = Atsc3GenParams {
                time_interleaver,
                time_aligned,
                ..base.clone()
            };
            let (_, packets, last) = loop_back_plp(&p, None, frames);
            assert!(last >= least, "case {n}: {last} blocks in the last frame");
            assert!(packets >= last, "case {n}");
        }
    }

    /// A PLP that only the second subframe carries is chosen by its id, and without a choice when it is the one with low
    /// level signalling. A PLP that is in the first and the third subframe only decodes in both.
    #[test]
    fn a_plp_of_a_later_subframe_is_chosen() {
        use super::super::gen::{GenPlace, GenPlp, GenTi};
        let place = |plp, start, cells| GenPlace {
            plp,
            start,
            cells,
            subslices: 1,
            interval: 0,
        };
        let mut subframes = more_subframes(&[]);
        subframes[0].plps = vec![place(1, 500, Some(160_000))];
        subframes[1].plps = vec![place(0, 0, Some(100_000))];
        let p = Atsc3GenParams {
            symbols: 24,
            coding: short(8, 2, Outer::Bch),
            more_plps: vec![GenPlp {
                coding: short(6, 4, Outer::Crc),
                seed: 5,
                start: 0,
                cells: Some(0),
                subslices: 1,
                interval: 0,
                time_interleaver: GenTi::Hybrid {
                    inter_subframe: false,
                    ti_blocks: 1,
                    cell_interleaver: true,
                    spare: 0,
                },
                lls: false,
            }],
            more_subframes: subframes,
            ..Atsc3GenParams::default()
        };
        // 160 000 cells hold 39 blocks of 4050.
        assert_eq!(loop_back_plp(&p, Some(1), 2), (1, 78, 39));
        let (_, packets, last) = loop_back_plp(&p, None, 2);
        assert!(packets > last && last >= 31, "{packets} {last}");
        let mut marked = p.clone();
        marked.more_plps[0].lls = true;
        assert_eq!(loop_back_plp(&marked, None, 1), (1, 39, 39));
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let x = b.frame();
        let mut rx = FrameDecoder::default();
        rx.select(Some(1));
        let HeadResult::Ready(head) = rx.head(&p.fields, &x, 0.0) else {
            panic!("the frame reads");
        };
        assert_eq!((head.part, head.plp, head.span), (1, 0, (500, 157_950)));
        let carried: Vec<bool> = rx.parts().iter().map(|p| p.plp.is_some()).collect();
        assert_eq!(carried, [false, true, false]);
        assert_eq!(rx.parts()[1].from, 12_288 + 209_600);
        let mut cells = PlpCells::default();
        assert!(!rx.part(&head, 0, &x, &mut cells));
        assert!(!rx.part(&head, 3, &x, &mut cells));
        assert!(rx.body(&head, &x, &mut cells));
    }

    #[test]
    fn the_head_asks_for_samples_and_names_what_it_does_not_decode() {
        let p = Atsc3GenParams::default();
        let mut b = FrameBuilder::new(p.clone()).unwrap();
        let x = b.frame();
        let mut rx = FrameDecoder::default();
        let need = 4 * bootstrap::SYM + 8192 + 192;
        assert!(
            matches!(rx.head(&p.fields, &x[..need - 1], 0.0), HeadResult::Need(n) if n == need)
        );
        assert!(matches!(
            rx.head(&p.fields, &x[..need], 0.0),
            HeadResult::Ready(_)
        ));
        let mut f = p.fields;
        f.rate_n = 2;
        assert!(matches!(
            rx.head(&f, &x, 0.0),
            HeadResult::Failed(NotDecoded::SampleRate, None)
        ));
        f = p.fields;
        f.preamble = 200;
        assert!(matches!(
            rx.head(&f, &x, 0.0),
            HeadResult::Failed(NotDecoded::Structure, None)
        ));
        // Another L1-Basic mode than the one sent: nothing reads.
        f.preamble = 0;
        assert!(matches!(
            rx.head(&f, &x, 0.0),
            HeadResult::Failed(NotDecoded::L1Basic, None)
        ));
        let noise: Vec<Complex32> = (0..x.len())
            .map(|i| {
                Complex32::new(
                    ((i * 7919) % 13) as f32 - 6.0,
                    ((i * 104729) % 11) as f32 - 5.0,
                )
            })
            .collect();
        assert!(matches!(
            rx.head(&p.fields, &noise, 0.0),
            HeadResult::Failed(NotDecoded::L1Basic, None)
        ));
        // Samples that are not numbers read as nothing.
        let nan = vec![Complex32::new(f32::NAN, f32::INFINITY); x.len()];
        assert!(matches!(
            rx.head(&p.fields, &nan, 0.0),
            HeadResult::Failed(NotDecoded::L1Basic, None)
        ));
        assert!(rx.signalling().is_none());
        let HeadResult::Ready(head) = rx.head(&p.fields, &x, 0.0) else {
            panic!("the frame reads");
        };
        // The fields of the default test signal.
        let (basic, detail) = rx.signalling().unwrap();
        assert_eq!((basic.preamble_symbols, basic.subframes), (1, 1));
        assert_eq!(
            (basic.detail_mode, basic.detail_bytes, basic.detail_cells),
            (2, 25, 774)
        );
        assert_eq!((basic.first.fft_size, basic.first.guard_interval), (0, 1));
        assert_eq!(
            (
                basic.first.symbols,
                basic.first.pilot_pattern,
                basic.first.pilot_boost
            ),
            (72, 10, 1)
        );
        assert!(basic.first.sbs_first && basic.first.sbs_last);
        let sub = &detail.subframes[0];
        assert_eq!(
            (sub.frequency_interleaver, sub.sbs_null_cells),
            (true, Some(300))
        );
        let plp = &sub.plps[0];
        assert_eq!((plp.id, plp.size, plp.fec_type), (0, 482_596, 1));
        assert_eq!(
            (plp.modulation, plp.code_rate, plp.ti_mode),
            (Some(0), Some(6), 0)
        );
        assert_eq!(plp.fec_block_start, Some(0));
        let mut cells = PlpCells::default();
        assert!(!rx.body(&head, &x[..x.len() - 1], &mut cells));
        assert!(rx.body(&head, &x, &mut cells));
        assert_eq!(head.info.plps.len(), 1);
        assert_eq!(head.info.plps[0].cells as usize, head.span.1);
    }

    /// Blocks of `k_sig` bits of one L1 mode through its constellation in white noise, max-log demapping and the 8-bit LLRs
    /// of [`l1_block`], decoded with each check rule of `rules` and, last, with the rule that [`L1Code::decode`] chooses,
    /// 50 iterations at most. A block is lost when the decoder returns an error or wrong bits.
    struct L1Trial {
        codes: Vec<L1Code>,
        rules: Vec<sdrkit_algo_fec::ldpc::MinSum>,
        points: Constellation,
        k_sig: usize,
        /// Mixed into the seed of every point, so that each row draws its own blocks.
        job: u64,
        n0: f64,
        rng: sdrkit_algo_sim::rng::Rng,
        noise: sdrkit_algo_sim::noise::Awgn,
        sig: Vec<u8>,
        bits: Vec<u8>,
        cells: Vec<Complex32>,
        soft: Vec<f32>,
        llr: Vec<i8>,
        out: Vec<u8>,
    }

    impl L1Trial {
        fn new(kind: L1Kind, mode: u8, k_sig: usize, rules: Vec<sdrkit_algo_fec::ldpc::MinSum>, job: u64) -> Self {
            let codes: Vec<L1Code> = (0..=rules.len()).map(|_| L1Code::new(kind, mode).unwrap()).collect();
            let points = constellation(codes[0].bits_per_cell(), codes[0].constellation_rate()).unwrap();
            L1Trial {
                codes,
                rules,
                points,
                k_sig,
                job,
                n0: 1.0,
                rng: sdrkit_algo_sim::rng::Rng::new(0),
                noise: sdrkit_algo_sim::noise::Awgn::with_power(1.0, 0).unwrap(),
                sig: Vec::new(),
                bits: Vec::new(),
                cells: Vec::new(),
                soft: Vec::new(),
                llr: Vec::new(),
                out: vec![0; k_sig],
            }
        }
    }

    impl sdrkit_dev_test::fer::Trial for L1Trial {
        fn start(&mut self, db: f64, seed: u64) {
            let seed = self.job << 40 ^ seed;
            self.n0 = 10f64.powf(-db / 10.0);
            self.rng = sdrkit_algo_sim::rng::Rng::new(seed);
            self.noise = sdrkit_algo_sim::noise::Awgn::with_power(self.n0, seed ^ 0x5DEE_CE66).unwrap();
        }

        fn send(&mut self, _: u64) {
            self.sig.clear();
            self.sig.extend((0..self.k_sig).map(|_| self.rng.bit()));
            self.codes[0].encode(&self.sig, &mut self.bits).unwrap();
            self.cells.clear();
            self.points.set().map_bits(&self.bits, &mut self.cells).unwrap();
            self.noise.process(&mut self.cells);
            let weights = vec![(1.0 / self.n0) as f32; self.cells.len()];
            self.soft.resize(self.bits.len(), 0.0);
            self.points
                .demap_block_weighted(&self.cells, &weights, Demap::MaxLog, &mut self.soft)
                .unwrap();
            self.llr.clear();
            self.llr
                .extend(self.soft.iter().map(|&l| (l * LLR_GAIN).round().clamp(-127.0, 127.0) as i8));
        }

        fn lost(&mut self, i: usize) -> bool {
            let (code, k) = (&mut self.codes[i], self.k_sig);
            let r = match self.rules.get(i) {
                Some(&rule) => code.decode_with(rule, &self.llr, k, 50, &mut self.out),
                None => code.decode(&self.llr, k, 50, &mut self.out),
            };
            r.is_err() || self.out != self.sig
        }
    }

    /// Measures the block error rate of each L1 mode against the LDPC check rule (scale 12 to 16 sixteenths, offset 0
    /// or 1) in white noise, through [`L1Trial`]:
    /// `cargo test --release -p sdrkit-stack-tvterr --lib l1_scale -- --ignored --nocapture`.
    /// L1-Basic at 200 bits and L1-Detail at the sizes of `L1_SCALE_SIZES`, each at most `K_seg` (default
    /// `200,500,1000,2000,4000,6312`), 300 blocks a point (`L1_SCALE_FRAMES`); `L1_SCALE_ONLY` limits the modes
    /// (`Basic3,Detail7`). Every rule decodes the same blocks. Each line gives the share of the parity bits that is
    /// punctured, the Es/N0 at a block error rate of 0.1 per rule and for the rule of [`L1Code::decode`], the best rule and
    /// its gain over the rule in use, and the failures of each rule in ten times the blocks 0.3 dB above the crossing of the
    /// rule in use. The best rule is the one of the lowest crossing among those with no more of these failures than the rule
    /// in use.
    #[test]
    #[ignore = "a measurement of minutes"]
    fn l1_scale() {
        use super::super::bicm::tests::{SCALES, TARGET};
        use sdrkit_algo_fec::ldpc::MinSum;
        use sdrkit_dev_test::fer::{point, search, threshold};
        let env = |name: &str| std::env::var(name).ok();
        let frames: u64 = env("L1_SCALE_FRAMES")
            .and_then(|f| f.parse().ok())
            .unwrap_or(300);
        let sizes: Vec<usize> = env("L1_SCALE_SIZES")
            .map(|l| l.split(',').filter_map(|k| k.trim().parse().ok()).collect())
            .unwrap_or_else(|| vec![200, 500, 1000, 2000, 4000, 6312]);
        let only = env("L1_SCALE_ONLY");
        let rules: Vec<MinSum> = [0, 1]
            .into_iter()
            .flat_map(|offset| SCALES.map(|scale_q4| MinSum { scale_q4, offset }))
            .collect();
        let mut jobs = Vec::new();
        for mode in 1..=7u8 {
            jobs.push((L1Kind::Basic, mode, 200));
            let top = L1Code::new(L1Kind::Detail, mode).unwrap().max_k_sig();
            let mut ks: Vec<usize> = sizes.iter().map(|&k| k.min(top)).collect();
            ks.dedup();
            jobs.extend(ks.into_iter().map(|k| (L1Kind::Detail, mode, k)));
        }
        jobs.retain(|&(kind, mode, _)| {
            only.as_ref()
                .is_none_or(|o| o.split(',').any(|m| m.trim() == format!("{kind:?}{mode}")))
        });
        let next = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
        let mut lines: Vec<(usize, String)> = std::thread::scope(|s| {
            let run = || {
                let mut out = Vec::new();
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(kind, mode, k_sig)) = jobs.get(i) else {
                        break;
                    };
                    let mut t = L1Trial::new(kind, mode, k_sig, rules.clone(), i as u64);
                    let n = rules.len() + 1;
                    let len = t.codes[0].lengths(k_sig).unwrap();
                    let bits = (k_sig * t.codes[0].bits_per_cell()) as f64 / len.sent() as f64;
                    let first = (10.0 * (2f64.powf(bits) - 1.0).log10() * 10.0).floor() / 10.0;
                    let pts = search(&mut t, n, first, first + 20.0, frames, TARGET);
                    let at: Vec<Option<f64>> = (0..n).map(|c| threshold(&pts, c, TARGET)).collect();
                    let used = n - 1;
                    let base = at[used].expect("the rule in use crosses the target");
                    let floor = point(&mut t, n, base + 0.3, 999, 10 * frames, None).lost;
                    let best = (0..rules.len())
                        .filter(|&c| floor[c] <= floor[used])
                        .filter_map(|c| Some((c, at[c]?)))
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(c, _)| c)
                        .unwrap_or(used);
                    // The parity bits of the 3/15 code of L1-Basic and L1-Detail modes 1 and 2, and of the 6/15 code.
                    let parity = if kind == L1Kind::Basic || mode <= 2 {
                        12960
                    } else {
                        9720
                    };
                    let line = format!(
                        "{kind:?} {mode} k {k_sig:4} punc {:.4} bits/cell {bits:.2}: Es/N0 at {TARGET}:{} | rule {base:6.2} | best {:?} gain {:+.2} dB | {:.2} dB fails:{}",
                        len.n_punc as f64 / f64::from(parity),
                        at[..used]
                            .iter()
                            .map(|a| a.map_or("    -- ".into(), |a| format!(" {a:6.2}")))
                            .collect::<String>(),
                        rules.get(best).map(|r| (r.scale_q4, r.offset)),
                        base - at[best].unwrap_or(base),
                        base + 0.3,
                        floor.iter().map(|f| format!(" {f}")).collect::<String>(),
                    );
                    eprintln!("{line}");
                    out.push((i, line));
                }
                out
            };
            let handles: Vec<_> = (0..workers).map(|_| s.spawn(run)).collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap())
                .collect()
        });
        lines.sort();
        println!(
            "Es/N0 in dB at a block error rate of {TARGET}, rules (scale/16, offset) = {:?}, then the rule in use",
            rules.iter().map(|r| (r.scale_q4, r.offset)).collect::<Vec<_>>()
        );
        for (_, l) in lines {
            println!("{l}");
        }
    }
}
