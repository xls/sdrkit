//! The Viterbi decoder for any [`ConvCode`] with `K` up to 16: whole frames ([`Viterbi::decode`]) and streams with a sliding
//! window ([`Viterbi::push`]).
//!
//! Symmetric codes with `K` 5 to 9 run on a 16-bit butterfly kernel (NEON on `aarch64`, AVX2 or SSE2 on `x86_64`, scalar on other
//! targets); every other code (`K` 2 to 16, any generators, `n` up to 8) runs on a 32-bit kernel. For a branch whose outputs are
//! `c_i`, the metric is `Σ (c_i == 0 ? +L_i : −L_i)` with `L` the `i8` LLRs (positive means 0), and the decoder keeps the path
//! with the largest total. An erased or punctured value (0) adds nothing.

use super::{state_mask, ConvCode, Encoder};
use crate::error::{Decoded, FecError, Param, Result};
use crate::llr::Llr8;
use sdrkit_core::backend::{Backend, Registry};

/// How the start and the end of a frame are known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ends {
    /// Terminated: the encoder starts in state 0 and the frame ends with `K − 1` zero tail bits that bring it back to 0. The
    /// output holds the information bits only (the tail is not written).
    Zero,
    /// Nothing known: any start state, and the frame is cut anywhere. The output holds every step.
    Open,
    /// The encoder starts in the given state (0 for an unterminated frame that starts from reset); the end is free.
    Start(u32),
    /// Tail-biting: the encoder starts in the state its last `K − 1` inputs leave it in (TS 36.212 clause 5.1.3.1). Decoded with
    /// the wrap-around Viterbi algorithm: the trellis is run round the frame again until the best path starts where it ends (at
    /// most four times).
    TailBiting,
}

/// The add-compare-select kernel a [`Viterbi`] uses.
#[derive(Clone, Copy)]
pub enum Kernel {
    /// 16-bit butterflies, run by a backend of [`BACKENDS`]. Symmetric codes with `K` 5 to 9 only.
    Butterfly(&'static dyn ViterbiBackend),
    /// 32-bit metrics, any code.
    Generic32,
}

impl Kernel {
    /// The name: the backend's name, or `"generic32"`.
    pub fn name(&self) -> &'static str {
        match self {
            Kernel::Butterfly(b) => b.name(),
            Kernel::Generic32 => "generic32",
        }
    }
}

impl PartialEq for Kernel {
    fn eq(&self, other: &Self) -> bool {
        self.name() == other.name()
    }
}

impl Eq for Kernel {}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A backend of the Viterbi family: the 16-bit add-compare-select butterflies. It runs `llr.len() / n` trellis steps on
/// `state` and writes the decisions of step `i` to `decisions[t * words..(t + 1) * words]` with `t = (t0 + i) & mask`. Bit
/// `s` of a step's decisions is 1 when new state `s` comes from its odd predecessor, which wins only when it is strictly
/// better. The caller keeps the metrics near zero between calls, so a call never makes them wrap. Every backend produces the
/// same decisions and metrics as the plain one, bit for bit.
///
/// A minimal backend that runs the plain kernel:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_fec::conv::{check_backend, ButterflyState, ViterbiBackend, BACKENDS};
///
/// struct Delegate;
/// impl Backend for Delegate {
///     fn name(&self) -> &'static str { "delegate" }
/// }
/// impl ViterbiBackend for Delegate {
///     fn run(&self, state: &mut ButterflyState, llr: &[i8], decisions: &mut [u64], words: usize, t0: usize,
///            mask: usize) {
///         BACKENDS.plain().run(state, llr, decisions, words, t0, mask);
///     }
/// }
///
/// check_backend(&Delegate);
/// ```
pub trait ViterbiBackend: Backend {
    /// Whether the backend runs codes with `half` butterflies (half the number of states, 8 to 128). The default is true.
    fn supports(&self, half: usize) -> bool {
        let _ = half;
        true
    }

    /// Runs the trellis steps of `llr` on `state`.
    fn run(
        &self,
        state: &mut ButterflyState,
        llr: &[i8],
        decisions: &mut [u64],
        words: usize,
        t0: usize,
        mask: usize,
    );
}

/// The Viterbi backends of this platform, the default first and the plain backend last.
pub static BACKENDS: Registry<dyn ViterbiBackend> = Registry::new(TABLE);

#[cfg(target_arch = "aarch64")]
const TABLE: &[&dyn ViterbiBackend] = &[&Neon16, &Scalar16];
#[cfg(target_arch = "x86_64")]
const TABLE: &[&dyn ViterbiBackend] = &[&Avx2_16, &Sse2_16, &Scalar16];
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const TABLE: &[&dyn ViterbiBackend] = &[&Scalar16];

/// Makes sure that `backend` agrees with the plain backend, bit for bit: the decisions and the metrics of 300 steps of noisy
/// LLRs on the codes with `K` 5, 7 and 9 (rates 1/2 and 1/3) that it supports, run in batches of 1, 17 and 50 steps. As in
/// the decoder, the metrics are moved back near zero after each batch, so they never wrap.
///
/// # Panics
/// When a decision or a metric differs, or when the backend supports none of the codes.
pub fn check_backend(backend: &dyn ViterbiBackend) {
    let mut seed = 0x0123_4567_89ab_cdefu64;
    let mut supported = 0;
    for code in [
        super::codes::GSM_K5,
        super::codes::DVB_K7,
        super::codes::LTE_K7_R3,
        ConvCode::new(9, &[0o561, 0o753]),
    ] {
        let half = (code.states() / 2) as usize;
        if !backend.supports(half) {
            continue;
        }
        supported += 1;
        let n = code.n();
        let llr: Vec<i8> = (0..300 * n)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                (seed >> 56) as i8
            })
            .collect();
        let words = (2 * half).div_ceil(64);
        for batch in [1usize, 17, 50] {
            let run = |b: &dyn ViterbiBackend| {
                let mut st = ButterflyState::new(&code);
                st.set_all(Some(0), 2 * n as i32 * 128 * 8 + 1);
                let mut dec = vec![0u64; 512 * words];
                let mut metrics = Vec::new();
                for (i, part) in llr.chunks(batch * n).enumerate() {
                    b.run(&mut st, part, &mut dec, words, i * batch, 511);
                    metrics.extend_from_slice(&st.bufs[st.cur][..2 * half]);
                    // Keep the metrics near zero, as the decoder does between batches.
                    let base = st.bufs[st.cur][0];
                    for v in st.bufs[st.cur][..2 * half].iter_mut() {
                        *v = v.wrapping_sub(base);
                    }
                }
                (dec, metrics)
            };
            let want = run(&Scalar16);
            let got = run(backend);
            assert!(
                got == want,
                "{} differs from the plain kernel on K = {}, batches of {batch}",
                backend.name(),
                code.k()
            );
        }
    }
    assert!(supported > 0, "{} supports no code", backend.name());
}

/// The plain backend: 16-bit butterflies in plain Rust.
struct Scalar16;

impl Backend for Scalar16 {
    fn name(&self) -> &'static str {
        "plain"
    }
}

impl ViterbiBackend for Scalar16 {
    fn run(
        &self,
        state: &mut ButterflyState,
        llr: &[i8],
        decisions: &mut [u64],
        words: usize,
        t0: usize,
        mask: usize,
    ) {
        run_scalar16(state, llr, decisions, words, t0, mask);
    }
}

/// The NEON backend (`aarch64`).
#[cfg(target_arch = "aarch64")]
struct Neon16;

#[cfg(target_arch = "aarch64")]
impl Backend for Neon16 {
    fn name(&self) -> &'static str {
        "neon"
    }
}

#[cfg(target_arch = "aarch64")]
impl ViterbiBackend for Neon16 {
    fn run(
        &self,
        state: &mut ButterflyState,
        llr: &[i8],
        decisions: &mut [u64],
        words: usize,
        t0: usize,
        mask: usize,
    ) {
        super::viterbi_simd::run_neon16(state, llr, decisions, words, t0, mask);
    }
}

/// The SSE2 backend (`x86_64`).
#[cfg(target_arch = "x86_64")]
struct Sse2_16;

#[cfg(target_arch = "x86_64")]
impl Backend for Sse2_16 {
    fn name(&self) -> &'static str {
        "sse2"
    }
}

#[cfg(target_arch = "x86_64")]
impl ViterbiBackend for Sse2_16 {
    fn run(
        &self,
        state: &mut ButterflyState,
        llr: &[i8],
        decisions: &mut [u64],
        words: usize,
        t0: usize,
        mask: usize,
    ) {
        super::viterbi_simd::run_sse2_16(state, llr, decisions, words, t0, mask);
    }
}

/// The AVX2 backend (`x86_64` with AVX2, codes with at least 32 states).
#[cfg(target_arch = "x86_64")]
struct Avx2_16;

#[cfg(target_arch = "x86_64")]
impl Backend for Avx2_16 {
    fn name(&self) -> &'static str {
        "avx2"
    }
    fn available(&self) -> bool {
        avx2_present()
    }
}

#[cfg(target_arch = "x86_64")]
impl ViterbiBackend for Avx2_16 {
    fn supports(&self, half: usize) -> bool {
        half >= 16
    }
    fn run(
        &self,
        state: &mut ButterflyState,
        llr: &[i8],
        decisions: &mut [u64],
        words: usize,
        t0: usize,
        mask: usize,
    ) {
        super::viterbi_simd::run_avx2_16(state, llr, decisions, words, t0, mask);
    }
}

#[cfg(target_arch = "x86_64")]
fn avx2_present() -> bool {
    std::is_x86_feature_detected!("avx2")
}

/// End states tried when the wrap-around tail-biting passes do not settle.
const TB_CANDIDATES: usize = 8;

/// Most butterflies of the 16-bit kernel (K = 9).
pub const MAX_HALF: usize = 128;

/// The state of the 16-bit butterfly kernel that a [`ViterbiBackend`] runs on. Each metric buffer holds the metrics of the
/// even states, then of the odd states: index `j` is state `2j`, index `half + j` is state `2j + 1`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ButterflyState {
    /// Butterflies per step: half the number of states.
    pub half: usize,
    /// Outputs per step (LLRs per step).
    pub n: usize,
    /// The two metric buffers. A step reads `bufs[cur]`, writes the other one and flips `cur`.
    pub bufs: [[i16; 2 * MAX_HALF]; 2],
    /// The buffer that holds the current metrics: 0 or 1.
    pub cur: usize,
    /// `sgn[i][j]`: +1 when output `i` of the branch from state `2j` with input 0 is 0, else −1.
    pub sgn: [[i16; MAX_HALF]; 8],
}

impl ButterflyState {
    pub(super) fn new(code: &ConvCode) -> Self {
        let half = (code.states() / 2) as usize;
        let mut sgn = [[0i16; MAX_HALF]; 8];
        for (j, _) in (0..half).enumerate() {
            let l = code.label((2 * j) as u32);
            for (i, row) in sgn.iter_mut().enumerate().take(code.n()) {
                row[j] = if (l >> i) & 1 == 0 { 1 } else { -1 };
            }
        }
        ButterflyState {
            half,
            n: code.n(),
            bufs: [[0; 2 * MAX_HALF]; 2],
            cur: 0,
            sgn,
        }
    }

    #[inline]
    fn metric(&self, s: usize) -> i16 {
        let b = &self.bufs[self.cur];
        if s.is_multiple_of(2) {
            b[s / 2]
        } else {
            b[self.half + s / 2]
        }
    }

    fn set_all(&mut self, start: Option<u32>, penalty: i32) {
        let h = self.half;
        let b = &mut self.bufs[self.cur];
        let fill = if start.is_some() { -penalty as i16 } else { 0 };
        b[..2 * h].fill(fill);
        if let Some(s) = start {
            let s = s as usize & (2 * h - 1);
            if s.is_multiple_of(2) {
                b[s / 2] = 0;
            } else {
                b[h + s / 2] = 0;
            }
        }
    }
}

/// The scalar 16-bit kernel: one trellis step per `n` LLRs, decisions written to `dec` (`words` per step, at
/// `((t0 + i) & mask) * words`).
fn run_scalar16(
    f: &mut ButterflyState,
    llr: &[i8],
    dec: &mut [u64],
    words: usize,
    t0: usize,
    mask: usize,
) {
    let n = f.n;
    let half = f.half;
    for (i, l) in llr.chunks_exact(n).enumerate() {
        let t = (t0 + i) & mask;
        let (src, dst) = if f.cur == 0 {
            let (a, b) = f.bufs.split_at_mut(1);
            (&a[0], &mut b[0])
        } else {
            let (a, b) = f.bufs.split_at_mut(1);
            (&b[0], &mut a[0])
        };
        let mut lo = [0i16; MAX_HALF];
        let mut hi = [0i16; MAX_HALF];
        let mut bits = [0u8; 2 * MAX_HALF / 8];
        for j in 0..half {
            let mut bm = 0i16;
            for (k, &lk) in l.iter().enumerate() {
                bm = bm.wrapping_add(f.sgn[k][j].wrapping_mul(lk as i16));
            }
            let (e, o) = (src[j], src[half + j]);
            let a0 = e.wrapping_add(bm);
            let a1 = o.wrapping_sub(bm);
            let b0 = e.wrapping_sub(bm);
            let b1 = o.wrapping_add(bm);
            let d0 = a1.wrapping_sub(a0) > 0;
            let d1 = b1.wrapping_sub(b0) > 0;
            lo[j] = if d0 { a1 } else { a0 };
            hi[j] = if d1 { b1 } else { b0 };
            bits[j / 8] |= (d0 as u8) << (j % 8);
            bits[(half + j) / 8] |= (d1 as u8) << (j % 8);
        }
        // New state j comes from `lo[j]`, state j + half from `hi[j]`; split them into even and odd states again.
        let q = half / 2;
        for i2 in 0..q {
            dst[i2] = lo[2 * i2];
            dst[half + i2] = lo[2 * i2 + 1];
            dst[q + i2] = hi[2 * i2];
            dst[half + q + i2] = hi[2 * i2 + 1];
        }
        store_bits(&bits[..2 * half / 8], &mut dec[t * words..(t + 1) * words]);
        f.cur ^= 1;
    }
}

/// Copies a little-endian bit set into decision words.
#[inline]
pub(super) fn store_bits(bits: &[u8], words: &mut [u64]) {
    if bits.len() < 8 {
        let mut b = [0u8; 8];
        b[..bits.len()].copy_from_slice(bits);
        words[0] = u64::from_le_bytes(b);
    } else {
        for (w, c) in words.iter_mut().zip(bits.as_chunks::<8>().0) {
            let mut b = [0u8; 8];
            b.copy_from_slice(c);
            *w = u64::from_le_bytes(b);
        }
    }
}

/// State of the 32-bit generic kernel.
#[derive(Clone)]
struct Generic32 {
    n: usize,
    m: Vec<i32>,
    nx: Vec<i32>,
    /// Labels of the two branches into each state: from the even predecessor and from the odd one.
    lab0: Vec<u8>,
    lab1: Vec<u8>,
}

impl Generic32 {
    fn new(code: &ConvCode) -> Self {
        let k = code.k();
        let s = code.states() as usize;
        let mut lab0 = vec![0u8; s];
        let mut lab1 = vec![0u8; s];
        for ns in 0..s {
            let b = (ns >> (k - 2)) as u32 & 1;
            let p0 = ((ns << 1) & (s - 1)) as u32;
            lab0[ns] = code.label((b << (k - 1)) | p0);
            lab1[ns] = code.label((b << (k - 1)) | p0 | 1);
        }
        Generic32 {
            n: code.n(),
            m: vec![0; s],
            nx: vec![0; s],
            lab0,
            lab1,
        }
    }

    fn run(&mut self, llr: &[i8], dec: &mut [u64], words: usize, t0: usize, mask: usize) {
        let s = self.m.len();
        let n = self.n;
        let mut tab = [0i32; 256];
        for (i, l) in llr.chunks_exact(n).enumerate() {
            let t = (t0 + i) & mask;
            // tab[label] = Σ ±L: start from all outputs 0 and flip the sign of each set bit.
            let total: i32 = l.iter().map(|&v| v as i32).sum();
            tab[0] = total;
            for lab in 1usize..(1 << n) {
                let low = lab.trailing_zeros() as usize;
                tab[lab] = tab[lab & (lab - 1)] - 2 * l[low] as i32;
            }
            let d = &mut dec[t * words..(t + 1) * words];
            d.fill(0);
            for ns in 0..s {
                let p0 = (ns << 1) & (s - 1);
                let m0 = self.m[p0].wrapping_add(tab[self.lab0[ns] as usize]);
                let m1 = self.m[p0 | 1].wrapping_add(tab[self.lab1[ns] as usize]);
                let take1 = m1.wrapping_sub(m0) > 0;
                self.nx[ns] = if take1 { m1 } else { m0 };
                d[ns / 64] |= (take1 as u64) << (ns % 64);
            }
            std::mem::swap(&mut self.m, &mut self.nx);
        }
    }

    fn set_all(&mut self, start: Option<u32>, penalty: i32) {
        let fill = if start.is_some() { -penalty } else { 0 };
        self.m.fill(fill);
        if let Some(st) = start {
            let st = st as usize & (self.m.len() - 1);
            self.m[st] = 0;
        }
    }
}

#[derive(Clone)]
enum Engine {
    Fast(Box<ButterflyState>),
    Generic(Box<Generic32>),
}

/// A Viterbi decoder for one [`ConvCode`], with every buffer sized at construction.
///
/// ```
/// use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Viterbi};
/// let bits: Vec<u8> = (0..200).map(|i| (i * 7 % 3 == 0) as u8).collect();
/// let mut enc = Encoder::new(codes::DVB_K7);
/// let mut coded = Vec::new();
/// enc.encode(&bits, &mut coded);
/// enc.terminate(&mut coded);
/// // Hard bits to LLRs (positive means 0), with a few errors.
/// let mut llr: Vec<i8> = coded.iter().map(|&b| if b == 0 { 40 } else { -40 }).collect();
/// for i in [10, 77, 150, 300] {
///     llr[i] = -llr[i];
/// }
/// let mut v = Viterbi::new(codes::DVB_K7, 1024).unwrap();
/// let mut out = vec![0u8; bits.len()];
/// let r = v.decode(&llr, Ends::Zero, &mut out).unwrap();
/// assert_eq!(out, bits);
/// assert_eq!(r.corrected, 4);
/// ```
#[derive(Clone)]
pub struct Viterbi {
    code: ConvCode,
    kernel: Kernel,
    engine: Engine,
    words: usize,
    states: usize,
    max_steps: usize,
    penalty: i32,
    batch: usize,
    decisions: Vec<u64>,
    // Streaming.
    depth: usize,
    chunk: usize,
    ring_mask: usize,
    steps: u64,
    released: u64,
    pending: [i8; 8],
    npending: usize,
    gain: i64,
    gain_steps: u64,
    last_best: i32,
    /// Metric growth and steps of the last push that decoded a whole step.
    push_gain: (i64, u64),
}

impl std::fmt::Debug for Viterbi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Viterbi")
            .field("code", &self.code)
            .field("kernel", &self.kernel)
            .field("max_steps", &self.max_steps)
            .field("steps", &self.steps)
            .finish()
    }
}

/// The best kernel for `code` on this machine.
fn pick_kernel(code: &ConvCode) -> Kernel {
    if !fast_ok(code) {
        return Kernel::Generic32;
    }
    let half = (code.states() / 2) as usize;
    BACKENDS
        .build(|b| b.supports(half).then_some(b))
        .map_or(Kernel::Generic32, Kernel::Butterfly)
}

/// Whether the 16-bit kernel can run `code`: K 5 to 9, symmetric generators, and metrics that stay inside 16 bits.
fn fast_ok(code: &ConvCode) -> bool {
    let k = code.k();
    (5..=9).contains(&k) && code.is_symmetric() && 4 * branch_bound(code) * (k as i32 - 1) < 32_000
}

/// The largest branch metric magnitude: `n * 128`.
fn branch_bound(code: &ConvCode) -> i32 {
    code.n() as i32 * 128
}

impl Viterbi {
    /// A decoder for `code` that accepts frames of up to `max_steps` trellis steps (input bits, tail included), with the best
    /// kernel for this machine. An error when `K` is above 16.
    pub fn new(code: ConvCode, max_steps: usize) -> Result<Self> {
        Self::with_kernel(code, max_steps, pick_kernel(&code))
    }

    /// Like [`Viterbi::new`] with a chosen kernel (for tests and benchmarks). An error when the kernel cannot run the code or is not
    /// available on this machine.
    pub fn with_kernel(code: ConvCode, max_steps: usize, kernel: Kernel) -> Result<Self> {
        let k = code.k();
        if k > 16 {
            return Param::ConstraintLength.range(2, 16, k);
        }
        let engine = match kernel {
            Kernel::Generic32 => Engine::Generic(Box::new(Generic32::new(&code))),
            Kernel::Butterfly(b) => {
                if !b.available() {
                    return Param::Kernel.unsupported();
                }
                if !b.supports((code.states() / 2) as usize) {
                    return Param::Kernel.conflict(Param::ConstraintLength);
                }
                if !fast_ok(&code) {
                    return Param::Kernel.conflict(Param::Generator);
                }
                Engine::Fast(Box::new(ButterflyState::new(&code)))
            }
        };
        let states = code.states() as usize;
        let words = states.div_ceil(64);
        let b = branch_bound(&code);
        let depth = 16 * (k as usize - 1);
        let chunk = depth;
        let ring = (depth + 2 * chunk + 1).next_power_of_two();
        let mut v = Viterbi {
            code,
            kernel,
            engine,
            words,
            states,
            max_steps,
            penalty: 2 * b * (k as i32 - 1) + 1,
            // After each batch the metrics are renormalised to state 0 (spread at most 2 (K - 1) b); a batch then grows them by at
            // most `batch b`, which keeps them inside 16 bits without wrapping.
            batch: ((30_000 - 2 * b * (k as i32 - 1)) / b).max(1) as usize,
            decisions: vec![0; max_steps.max(ring) * words],
            depth,
            chunk,
            ring_mask: ring - 1,
            steps: 0,
            released: 0,
            pending: [0; 8],
            npending: 0,
            gain: 0,
            gain_steps: 0,
            last_best: 0,
            push_gain: (0, 0),
        };
        v.reset();
        Ok(v)
    }

    /// The code.
    pub fn code(&self) -> &ConvCode {
        &self.code
    }

    /// The kernel in use.
    pub fn kernel(&self) -> Kernel {
        self.kernel
    }

    /// The longest frame [`Viterbi::decode`] accepts, in trellis steps.
    pub fn max_steps(&self) -> usize {
        self.max_steps
    }

    fn init_metrics(&mut self, start: Option<u32>) {
        let p = self.penalty;
        match &mut self.engine {
            Engine::Fast(f) => f.set_all(start, p),
            Engine::Generic(g) => g.set_all(start, p),
        }
        self.last_best = self.best().1;
    }

    /// The best state and its (wrapped) metric, compared modulo the metric width.
    fn best(&self) -> (u32, i32) {
        match &self.engine {
            Engine::Fast(f) => {
                // The greatest metric first, then the lowest state that has it (the even states, then the odd ones).
                let (even, odd) = f.bufs[f.cur][..2 * f.half].split_at(f.half);
                let base = even[0];
                let top = even
                    .iter()
                    .chain(odd)
                    .fold(0i16, |m, v| m.max(v.wrapping_sub(base)));
                let at = |half: &[i16]| half.iter().position(|v| v.wrapping_sub(base) == top);
                let best = match (at(even), at(odd)) {
                    (Some(e), Some(o)) => (2 * e).min(2 * o + 1),
                    (Some(e), None) => 2 * e,
                    (None, Some(o)) => 2 * o + 1,
                    (None, None) => 0,
                };
                (best as u32, f.metric(best) as i32)
            }
            Engine::Generic(g) => {
                let base = g.m[0];
                let mut best = 0usize;
                let mut bd = 0i32;
                for (s, &m) in g.m.iter().enumerate().skip(1) {
                    let d = m.wrapping_sub(base);
                    if d > bd {
                        bd = d;
                        best = s;
                    }
                }
                (best as u32, g.m[best])
            }
        }
    }

    /// Runs the kernel over `llr` (a whole number of steps), decisions from step `t0` (ring-indexed by `mask`), tracking the gain.
    fn run(&mut self, llr: &[i8], t0: usize, mask: usize) {
        let n = self.code.n();
        let mut t = t0;
        for part in llr.chunks(self.batch * n) {
            let words = self.words;
            match &mut self.engine {
                Engine::Generic(g) => g.run(part, &mut self.decisions, words, t, mask),
                Engine::Fast(f) => {
                    if let Kernel::Butterfly(b) = self.kernel {
                        b.run(f, part, &mut self.decisions, words, t, mask);
                    }
                }
            }
            let steps = part.len() / n;
            t += steps;
            let (_, b) = self.best();
            let delta = match self.engine {
                Engine::Fast(_) => (b as i16).wrapping_sub(self.last_best as i16) as i64,
                Engine::Generic(_) => b.wrapping_sub(self.last_best) as i64,
            };
            self.gain += delta;
            self.gain_steps += steps as u64;
            self.last_best = b;
            // Keep the 16-bit metrics near zero (subtract state 0's), so no kernel ever sees them wrap.
            if let Engine::Fast(f) = &mut self.engine {
                let base = f.metric(0);
                let h = f.half;
                for v in f.bufs[f.cur][..2 * h].iter_mut() {
                    *v = v.wrapping_sub(base);
                }
                self.last_best = self.last_best.wrapping_sub(base as i32);
            }
        }
    }

    /// Traces back from `state` at the end of step `end - 1` down to step `begin`, writing the bits of steps
    /// `[begin, end)` that fall in `[out_from, out_from + out.len())` into `out`. Returns the state before step `begin`.
    fn traceback(
        &self,
        mut state: u32,
        end: u64,
        begin: u64,
        mask: usize,
        out_from: u64,
        out: &mut [u8],
    ) -> u32 {
        let k = self.code.k();
        let smask = (self.states - 1) as u32;
        let w = self.words;
        let mut t = end;
        while t > begin {
            t -= 1;
            if t >= out_from && t < out_from + out.len() as u64 {
                out[(t - out_from) as usize] = (state >> (k - 2)) as u8 & 1;
            }
            let idx = ((t as usize) & mask) * w + (state as usize / 64);
            let d = (self.decisions[idx] >> (state % 64)) & 1;
            state = ((state << 1) & smask) | d as u32;
        }
        state
    }

    /// Decodes one whole frame of `llr` (`n` values per step, punctured places 0) into `out` (one bit per byte).
    ///
    /// `out` must hold the steps (less the `K − 1` tail steps with [`Ends::Zero`]). `metric` in the result is the growth of the
    /// best path metric over the frame (about `n × mean|L|` per step on a clean signal); `corrected` counts the received hard
    /// decisions (non-zero LLRs) that disagree with the re-encoded output; `iterations` is the number of passes (more than 1 only
    /// for tail-biting).
    pub fn decode(&mut self, llr: &[Llr8], ends: Ends, out: &mut [u8]) -> Result<Decoded> {
        let Some((first_state, info, passes)) = self.decode_path(llr, ends, out)? else {
            return Ok(Decoded::corrected(0));
        };
        let corrected = count_disagreements(
            &self.code,
            first_state,
            &out[..info],
            ends == Ends::Zero,
            llr,
        );
        Ok(Decoded {
            corrected,
            metric: self.gain.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            iterations: passes,
        })
    }

    /// Decodes one whole frame as [`decode`](Self::decode) does and writes the same bits, without the count of corrected
    /// decisions (which costs one pass of the encoder over the frame). For a caller that checks the frame by other means.
    pub fn decode_only(&mut self, llr: &[Llr8], ends: Ends, out: &mut [u8]) -> Result<()> {
        self.decode_path(llr, ends, out).map(|_| ())
    }

    /// The decoding of [`decode`](Self::decode): the first state of the path, the count of bits written and the passes; `None`
    /// for an empty frame.
    fn decode_path(
        &mut self,
        llr: &[Llr8],
        ends: Ends,
        out: &mut [u8],
    ) -> Result<Option<(u32, usize, u32)>> {
        let n = self.code.n();
        let k = self.code.k() as usize;
        if !llr.len().is_multiple_of(n) {
            return Err(FecError::InputLength {
                expected: n,
                got: llr.len(),
            });
        }
        let steps = llr.len() / n;
        if steps > self.max_steps {
            return Err(FecError::TooLong {
                max: self.max_steps,
            });
        }
        let info = match ends {
            Ends::Zero => {
                if steps < k - 1 {
                    return Err(FecError::InputLength {
                        expected: (k - 1) * n,
                        got: llr.len(),
                    });
                }
                steps - (k - 1)
            }
            _ => steps,
        };
        if out.len() < info {
            return Err(FecError::OutputTooShort { need: info });
        }
        if steps == 0 {
            return Ok(None);
        }
        let out = &mut out[..info];
        // Any streaming state is lost.
        self.steps = 0;
        self.released = 0;
        self.npending = 0;
        self.gain = 0;
        self.gain_steps = 0;
        let start = match ends {
            Ends::Zero => Some(0),
            Ends::Start(s) => Some(s & state_mask(k as u32)),
            Ends::Open | Ends::TailBiting => None,
        };
        self.init_metrics(start);
        let mut passes = 1;
        self.run(llr, 0, usize::MAX);
        let (end_state, first_state) = match ends {
            Ends::Zero => {
                let s0 = self.traceback(0, steps as u64, 0, usize::MAX, 0, out);
                (0, s0)
            }
            Ends::TailBiting => loop {
                let (best, _) = self.best();
                let s0 = self.traceback(best, steps as u64, 0, usize::MAX, 0, out);
                if s0 == best {
                    break (best, s0);
                }
                if passes == 4 {
                    // The wrap-around passes did not settle: decode with start and end forced to each of the best few end
                    // states and keep the best tail-biting path.
                    let s = self.tail_biting_fallback(llr, out);
                    passes += TB_CANDIDATES as u32 + 1;
                    break (s, s);
                }
                // Go round again from the end metrics (wrap-around Viterbi).
                self.gain = 0;
                self.gain_steps = 0;
                self.run(llr, 0, usize::MAX);
                passes += 1;
            },
            _ => {
                let (best, _) = self.best();
                let s0 = self.traceback(best, steps as u64, 0, usize::MAX, 0, out);
                (best, s0)
            }
        };
        let _ = end_state;
        Ok(Some((first_state, info, passes)))
    }

    /// Tail-biting by trial: for each of the best end states `s` of the last pass (at most `TB_CANDIDATES`, chosen among at most
    /// 64 states spread evenly), decode from `s` and read the metric back at `s`; the best such closed path is traced back into
    /// `out`. Returns its state.
    fn tail_biting_fallback(&mut self, llr: &[i8], out: &mut [u8]) -> u32 {
        let steps = llr.len() / self.code.n();
        let stride = self.states.div_ceil(64);
        let take = self.states.min(64);
        let mut cand = [(i64::MIN, 0u32); TB_CANDIDATES];
        for i in 0..take {
            let st = i * stride;
            // Metrics wrap: compare relative to state 0.
            let rel = match &self.engine {
                Engine::Fast(f) => f.metric(st).wrapping_sub(f.metric(0)) as i64,
                Engine::Generic(g) => g.m[st].wrapping_sub(g.m[0]) as i64,
            };
            if rel > cand[TB_CANDIDATES - 1].0 {
                let mut j = TB_CANDIDATES - 1;
                while j > 0 && cand[j - 1].0 < rel {
                    cand[j] = cand[j - 1];
                    j -= 1;
                }
                cand[j] = (rel, st as u32);
            }
        }
        let mut best = (i64::MIN, 0u32);
        for &(rel, st) in &cand {
            if rel == i64::MIN {
                continue;
            }
            self.gain = 0;
            self.gain_steps = 0;
            self.init_metrics(Some(st));
            self.run(llr, 0, usize::MAX);
            let m = self.metric_of(st);
            if m > best.0 {
                best = (m, st);
            }
        }
        self.gain = 0;
        self.gain_steps = 0;
        self.init_metrics(Some(best.1));
        self.run(llr, 0, usize::MAX);
        self.traceback(best.1, steps as u64, 0, usize::MAX, 0, out);
        best.1
    }

    /// The true metric of state `st` after the last run (the best metric's growth plus the wrapped difference to the best).
    fn metric_of(&self, st: u32) -> i64 {
        let (_, bm) = self.best();
        let d = match &self.engine {
            Engine::Fast(f) => f.metric(st as usize).wrapping_sub(bm as i16) as i64,
            Engine::Generic(g) => g.m[st as usize].wrapping_sub(bm) as i64,
        };
        self.gain + d
    }

    // ---- Streaming ----

    /// Sets the sliding window: bits are released `chunk` at a time once `depth` further steps have been decoded after them. The
    /// default is `depth = chunk = 16 (K − 1)` (96 for K = 7); punctured high rates (7/8) want a longer depth. Resets the stream.
    /// This is a configuration call: it may allocate.
    pub fn set_traceback(&mut self, depth: usize, chunk: usize) -> Result<()> {
        Param::Depth.check(depth, 1, 1 << 20)?;
        Param::ChunkLen.check(chunk, 1, 1 << 20)?;
        let ring = (depth + 2 * chunk + 1).next_power_of_two();
        let need = self.max_steps.max(ring) * self.words;
        if self.decisions.len() < need {
            self.decisions.resize(need, 0);
        }
        self.depth = depth;
        self.chunk = chunk;
        self.ring_mask = ring - 1;
        self.reset();
        Ok(())
    }

    /// The sliding window: `(depth, chunk)`.
    pub fn window(&self) -> (usize, usize) {
        (self.depth, self.chunk)
    }

    /// Starts a new stream with every state equally likely.
    pub fn reset(&mut self) {
        self.steps = 0;
        self.released = 0;
        self.npending = 0;
        self.gain = 0;
        self.gain_steps = 0;
        self.push_gain = (0, 0);
        self.init_metrics(None);
    }

    /// Starts a new stream from a known encoder state.
    pub fn reset_to(&mut self, state: u32) {
        self.reset();
        self.init_metrics(Some(state & state_mask(self.code.k())));
    }

    /// Steps decoded since the last reset.
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// Bits released since the last reset.
    pub fn released(&self) -> u64 {
        self.released
    }

    /// The best path's metric growth per step since the last reset (or over the last [`Viterbi::decode`]): about `n × mean|L|`
    /// for a clean signal, much less for noise or a wrong hypothesis (phase, polarity, puncturing phase). 0 before any step.
    pub fn mean_gain(&self) -> f32 {
        if self.gain_steps == 0 {
            0.0
        } else {
            self.gain as f32 / self.gain_steps as f32
        }
    }

    /// The best path's metric growth per step over the steps of the last [`Viterbi::push`] that decoded at least one whole
    /// step. A decoder fed one step per push gets the growth of each step: high on a clean signal, near 0 or below on noise.
    /// 0 after a reset.
    pub fn last_gain(&self) -> f32 {
        match self.push_gain {
            (_, 0) => 0.0,
            (g, n) => g as f32 / n as f32,
        }
    }

    /// Decodes a piece of a stream (`n` values per step; a partial step is kept for the next call) and appends the bits whose
    /// window is complete to `out`. The bits do not depend on how the input is split across calls.
    pub fn push(&mut self, llr: &[Llr8], out: &mut Vec<u8>) {
        let (g0, s0) = (self.gain, self.gain_steps);
        self.push_inner(llr, out);
        if self.gain_steps > s0 {
            self.push_gain = (self.gain - g0, self.gain_steps - s0);
        }
    }

    fn push_inner(&mut self, llr: &[Llr8], out: &mut Vec<u8>) {
        let n = self.code.n();
        let mut rest = llr;
        if self.npending > 0 {
            let take = (n - self.npending).min(rest.len());
            self.pending[self.npending..self.npending + take].copy_from_slice(&rest[..take]);
            self.npending += take;
            rest = &rest[take..];
            if self.npending < n {
                return;
            }
            let p = self.pending;
            self.npending = 0;
            self.stream_steps(&p[..n], out);
        }
        let whole = rest.len() - rest.len() % n;
        self.stream_steps(&rest[..whole], out);
        let tail = &rest[whole..];
        self.pending[..tail.len()].copy_from_slice(tail);
        self.npending = tail.len();
    }

    fn stream_steps(&mut self, mut llr: &[i8], out: &mut Vec<u8>) {
        let n = self.code.n();
        while !llr.is_empty() {
            // Stop exactly at the next release point, so releases do not depend on how the input was cut.
            let until_release =
                (self.released + (self.depth + self.chunk) as u64 - self.steps) as usize;
            let take = (llr.len() / n).min(until_release.max(1));
            let (now, later) = llr.split_at(take * n);
            let t0 = self.steps as usize;
            let mask = self.ring_mask;
            self.run(now, t0, mask);
            self.steps += take as u64;
            llr = later;
            while self.steps - self.released >= (self.depth + self.chunk) as u64 {
                self.release(self.chunk, None, out);
            }
        }
    }

    /// Traces back from the best state (or `from`) at the newest step and appends `count` bits from the oldest unreleased one.
    fn release(&mut self, count: usize, from: Option<u32>, out: &mut Vec<u8>) {
        let state = from.unwrap_or_else(|| self.best().0);
        let start = out.len();
        out.resize(start + count, 0);
        let released = self.released;
        let steps = self.steps;
        let mask = self.ring_mask;
        self.traceback(state, steps, released, mask, released, &mut out[start..]);
        self.released += count as u64;
    }

    /// Releases every bit decided so far, the newest ones from the best state with a short window (the end of a stream).
    pub fn flush(&mut self, out: &mut Vec<u8>) {
        let count = (self.steps - self.released) as usize;
        self.release(count, None, out);
    }

    /// Releases every bit of a terminated stream: traces back from state 0 and drops the `K − 1` tail bits.
    pub fn flush_terminated(&mut self, out: &mut Vec<u8>) {
        let tail = (self.code.k() - 1) as u64;
        let first = self.released;
        let count = (self.steps - first) as usize;
        let start = out.len();
        self.release(count, Some(0), out);
        // Drop the bits of the tail steps (those not already released).
        let keep = self.steps.saturating_sub(tail).saturating_sub(first) as usize;
        out.truncate(start + keep.min(count));
    }
}

/// Re-encodes `bits` from `start_state` (adding the `K − 1` zero tail when `terminated`) and counts the non-zero values of `llr`
/// whose hard decision disagrees.
fn count_disagreements(
    code: &ConvCode,
    start_state: u32,
    bits: &[u8],
    terminated: bool,
    llr: &[i8],
) -> u32 {
    let n = code.n();
    let mut enc = Encoder::with_state(*code, start_state);
    let mut errs = 0u32;
    let tail = if terminated { code.k() as usize - 1 } else { 0 };
    let mut chunks = llr.chunks_exact(n);
    for i in 0..bits.len() + tail {
        let b = if i < bits.len() { bits[i] } else { 0 };
        let lab = enc.push_bit(b);
        if let Some(l) = chunks.next() {
            for (j, &v) in l.iter().enumerate() {
                if v != 0 && ((v < 0) as u8) != (lab >> j) & 1 {
                    errs += 1;
                }
            }
        }
    }
    errs
}

/// Re-encode error counting (bit-error estimation): re-encodes the decoded `bits` from `start_state` and counts the non-zero
/// values of `llr` (the depunctured input, `n` per bit) whose hard decision disagrees. With [`Ends::TailBiting`] pass the
/// tail-biting state ([`super::tail_biting_state`]); for a terminated frame include the tail bits in `bits`.
///
/// ```
/// use sdrkit_algo_fec::conv::{codes, reencode_errors, Encoder};
/// let bits = [1u8, 0, 1, 1, 0, 1];
/// let mut coded = Vec::new();
/// Encoder::new(codes::GSM_K5).encode(&bits, &mut coded);
/// let mut llr: Vec<i8> = coded.iter().map(|&b| if b == 0 { 9 } else { -9 }).collect();
/// llr[3] = -llr[3];
/// llr[4] = 0; // erased: not counted
/// assert_eq!(reencode_errors(&codes::GSM_K5, 0, &bits, &llr).unwrap(), 1);
/// ```
pub fn reencode_errors(
    code: &ConvCode,
    start_state: u32,
    bits: &[u8],
    llr: &[Llr8],
) -> Result<u32> {
    if llr.len() != bits.len() * code.n() {
        return Err(FecError::InputLength {
            expected: bits.len() * code.n(),
            got: llr.len(),
        });
    }
    Ok(count_disagreements(code, start_state, bits, false, llr))
}
