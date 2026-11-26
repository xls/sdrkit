//! Resamplers: a rational L/M polyphase resampler, a fractional (arbitrary-ratio) polyphase resampler with exact time bookkeeping,
//! and [`Resampler`], which combines a multi-stage decimator with the fractional stage for any pair of rates.
//! The fractional resampler can also shift a complex input in frequency, start at a fractional input position and read a stored slice.
//!
//! Time is tracked in fixed point (64 integer and 64 fractional bits of input samples), and for integer rates the step is the exact
//! quotient, so the number of outputs for `n` inputs is `n * out / in` to within one sample after any length of time.

use crate::decim::Decimator;
use crate::fir::{design_beta, extend_with_room, kaiser_len, windowed_sinc, CHUNK};
use crate::nco::Nco;
use crate::sample::{complex_values, complex_values_mut};
use crate::{Append, Complex32, DspError, Param, Sample, Stereo};
use sdrkit_core::backend::{Backend, Registry};

const ONE: u128 = 1 << 64;

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// A polyphase bank as a [`ResampleBackend`] sees it: `taps.len() / k` phases laid out by [`ResampleBackend::prepare`], for
/// samples of `lanes` values.
#[derive(Clone, Copy, Debug)]
pub struct PolyBank<'a> {
    /// The prepared taps.
    pub taps: &'a [f32],
    /// Taps per phase.
    pub k: usize,
    /// Values per sample: 1 (real) or 2 (complex IQ or stereo).
    pub lanes: usize,
}

/// A backend of the resampling family: the polyphase loops of [`RationalResampler`] and [`FractionalResampler`] over a block of
/// input. In both calls, `x` holds `k - 1` older samples and then the new ones, `lanes` values per sample. Input `i` is the
/// sample `x[i + k - 1]`, and its window is the samples `i..i + k`. Each call stops when `out` is full or every input is used,
/// and returns `(values written to out / lanes, inputs used)`. An input is used when no output is left at it.
///
/// A minimal backend that runs the plain loops on real samples only:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_dsp::resample::{check_backend, PolyBank, ResampleBackend, BACKENDS};
///
/// struct RealOnly;
/// impl Backend for RealOnly {
///     fn name(&self) -> &'static str { "real-only" }
/// }
/// impl ResampleBackend for RealOnly {
///     fn prepare(&self, bank: &[f32], k: usize, lanes: usize) -> Option<Vec<f32>> {
///         (lanes == 1).then(|| bank.to_vec())
///     }
///     fn rational(&self, bank: PolyBank<'_>, x: &[f32], ratio: (usize, usize), phase: &mut usize,
///                 out: &mut [f32]) -> (usize, usize) {
///         BACKENDS.plain().rational(bank, x, ratio, phase, out)
///     }
///     fn fractional(&self, bank: PolyBank<'_>, x: &[f32], step: u128, pos: &mut u128,
///                   out: &mut [f32]) -> (usize, usize) {
///         BACKENDS.plain().fractional(bank, x, step, pos, out)
///     }
/// }
///
/// check_backend(&RealOnly);
/// ```
pub trait ResampleBackend: Backend {
    /// Lays out a bank of `bank.len() / k` phases of `k` taps (each phase oldest-sample first) for samples of `lanes` values.
    /// `None` when the backend cannot do it.
    fn prepare(&self, bank: &[f32], k: usize, lanes: usize) -> Option<Vec<f32>>;

    /// Up by `L`, down by `M`, with `ratio = (L, M)` and the `L` phases of `bank`. At each input, while `phase < L`, it writes
    /// the window filtered by phase `phase` and adds `M` to `phase`. Then it subtracts `L`.
    fn rational(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        ratio: (usize, usize),
        phase: &mut usize,
        out: &mut [f32],
    ) -> (usize, usize);

    /// Arbitrary ratio, with the `P + 1` phases of `bank` (`P` a power of two) and the time `pos` in Q64.64 input samples. At
    /// each input, while `pos < 2^64`, it writes `a + (b - a) mu` and adds `step` to `pos`. Then it subtracts `2^64`. With `f`
    /// the low 64 bits of `pos`, `a` is the window filtered by phase `f >> (64 - log2 P)`, `b` by the next phase, and `mu` is
    /// the next 24 bits of `f` divided by `2^24`.
    fn fractional(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        step: u128,
        pos: &mut u128,
        out: &mut [f32],
    ) -> (usize, usize);
}

/// The resampling backends of this platform, the default first and the plain backend last.
pub static BACKENDS: Registry<dyn ResampleBackend> = Registry::new(TABLE);

#[cfg(target_arch = "aarch64")]
const TABLE: &[&dyn ResampleBackend] = &[&NeonResample, &PlainResample];
#[cfg(not(target_arch = "aarch64"))]
const TABLE: &[&dyn ResampleBackend] = &[&PlainResample];

/// Makes sure that `backend` agrees with the plain backend: rational ratios 3/2, 2/3 and 160/147 and fractional steps 0.7,
/// 1.3 and 4.1 input samples, with 5 and 33 taps per phase, one and two lanes, on a fixed random signal of 300 inputs given in
/// blocks of 1, 7 and 300, with an output room of 1 and 64. Each output must be within `1e-5` times the sum of the tap
/// magnitudes of one phase pair, and the counts must be the same.
///
/// # Panics
/// When an output or a count differs, or when the backend prepares no layout.
pub fn check_backend(backend: &dyn ResampleBackend) {
    let mut seed = 0xd1b5_4a32_d192_ed03u64;
    let mut u = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    let mut prepared_any = false;
    for k in [5usize, 33] {
        for lanes in [1usize, 2] {
            let x: Vec<f32> = (0..(k - 1 + 300) * lanes).map(|_| u()).collect();
            for (phases, ratio, step) in [
                (3, (3, 2), 0),
                (2, (2, 3), 0),
                (160, (160, 147), 0),
                (65, (0, 0), (0.7 * (1u128 << 64) as f64) as u128),
                (129, (0, 0), (1.3 * (1u128 << 64) as f64) as u128),
                (17, (0, 0), (4.1 * (1u128 << 64) as f64) as u128),
            ] {
                let bank: Vec<f32> = (0..phases * k).map(|_| u()).collect();
                let bound = 2e-5 * bank.iter().map(|t| t.abs()).fold(0.0f32, f32::max) * k as f32;
                let Some(p) = backend.prepare(&bank, k, lanes) else {
                    continue;
                };
                prepared_any = true;
                let mine = PolyBank { taps: &p, k, lanes };
                let plain = PolyBank {
                    taps: &bank,
                    k,
                    lanes,
                };
                for (block, room) in [(1usize, 1usize), (7, 64), (300, 64)] {
                    let run = |b: &dyn ResampleBackend, bank: PolyBank<'_>| {
                        let (mut phase, mut pos) = (0usize, 0u128);
                        let mut all = Vec::new();
                        let mut out = vec![0.0f32; room * lanes];
                        let mut start = 0;
                        while start < 300 {
                            let end = (start + block).min(300);
                            let xs = &x[start * lanes..(end + k - 1) * lanes];
                            let mut used = 0;
                            while used < end - start {
                                let xs = &xs[used * lanes..];
                                let (w, u) = if step == 0 {
                                    b.rational(bank, xs, ratio, &mut phase, &mut out)
                                } else {
                                    b.fractional(bank, xs, step, &mut pos, &mut out)
                                };
                                all.extend_from_slice(&out[..w * lanes]);
                                used += u;
                            }
                            start = end;
                        }
                        all
                    };
                    let want = run(&PlainResample, plain);
                    let got = run(backend, mine);
                    assert_eq!(got.len(), want.len(), "{} output count", backend.name());
                    for (g, w) in got.iter().zip(&want) {
                        assert!(
                            (g - w).abs() <= bound,
                            "{} {k} taps, {lanes} lanes, {phases} phases: {g} for {w}",
                            backend.name()
                        );
                    }
                }
            }
        }
    }
    assert!(prepared_any, "{} prepares no layout", backend.name());
}

/// The plain resampling backend: the polyphase loops on the dot products of [`Sample`].
struct PlainResample;

impl Backend for PlainResample {
    fn name(&self) -> &'static str {
        "plain"
    }
}

impl ResampleBackend for PlainResample {
    fn prepare(&self, bank: &[f32], _k: usize, lanes: usize) -> Option<Vec<f32>> {
        matches!(lanes, 1 | 2).then(|| bank.to_vec())
    }

    fn rational(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        ratio: (usize, usize),
        phase: &mut usize,
        out: &mut [f32],
    ) -> (usize, usize) {
        if bank.lanes == 1 {
            rational_plain(bank.taps, bank.k, x, ratio, phase, out)
        } else {
            let (x, out) = (complex_values(x), complex_values_mut(out));
            rational_plain(bank.taps, bank.k, x, ratio, phase, out)
        }
    }

    fn fractional(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        step: u128,
        pos: &mut u128,
        out: &mut [f32],
    ) -> (usize, usize) {
        if bank.lanes == 1 {
            fractional_plain(bank.taps, bank.k, x, step, pos, out)
        } else {
            let (x, out) = (complex_values(x), complex_values_mut(out));
            fractional_plain(bank.taps, bank.k, x, step, pos, out)
        }
    }
}

/// The rational loop of [`ResampleBackend::rational`] on samples of type `T`.
#[inline]
fn rational_plain<T: Sample>(
    bank: &[f32],
    k: usize,
    x: &[T],
    (l, m): (usize, usize),
    phase: &mut usize,
    out: &mut [T],
) -> (usize, usize) {
    let inputs = (x.len() + 1).saturating_sub(k);
    let mut w = 0;
    for i in 0..inputs {
        while *phase < l {
            if w == out.len() {
                return (w, i);
            }
            let p = *phase * k;
            out[w] = T::dot(&bank[p..p + k], &x[i..i + k]);
            w += 1;
            *phase += m;
        }
        *phase -= l;
    }
    (w, inputs)
}

/// The fractional loop of [`ResampleBackend::fractional`] on samples of type `T`.
#[inline]
fn fractional_plain<T: Sample>(
    bank: &[f32],
    k: usize,
    x: &[T],
    step: u128,
    pos: &mut u128,
    out: &mut [T],
) -> (usize, usize) {
    let bits = (bank.len() / k - 1).trailing_zeros();
    let inputs = (x.len() + 1).saturating_sub(k);
    let mut w = 0;
    for i in 0..inputs {
        let win = &x[i..i + k];
        while *pos < ONE {
            if w == out.len() {
                return (w, i);
            }
            let frac = *pos as u64;
            let p = (frac >> (64 - bits)) as usize;
            let mu = ((frac << bits) >> 40) as f32 * (1.0 / (1u32 << 24) as f32);
            let a = T::dot(&bank[p * k..p * k + k], win);
            let b = T::dot(&bank[(p + 1) * k..(p + 2) * k], win);
            out[w] = a + (b - a) * mu;
            w += 1;
            *pos += step;
        }
        *pos -= ONE;
    }
    (w, inputs)
}

/// The NEON resampling backend (ARM64): the polyphase loops on the dot products of [`crate::neon`]. The fractional loop
/// makes one dot product for an output, with the taps `a + (b - a) mu` of the two phases.
#[cfg(target_arch = "aarch64")]
struct NeonResample;

#[cfg(target_arch = "aarch64")]
impl Backend for NeonResample {
    fn name(&self) -> &'static str {
        "neon"
    }
}

#[cfg(target_arch = "aarch64")]
impl ResampleBackend for NeonResample {
    fn prepare(&self, bank: &[f32], _k: usize, lanes: usize) -> Option<Vec<f32>> {
        matches!(lanes, 1 | 2).then(|| bank.to_vec())
    }

    fn rational(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        (l, m): (usize, usize),
        phase: &mut usize,
        out: &mut [f32],
    ) -> (usize, usize) {
        let PolyBank { taps, k, lanes } = bank;
        let inputs = (x.len() / lanes + 1).saturating_sub(k);
        let room = out.len() / lanes;
        let mut w = 0;
        for i in 0..inputs {
            let win = &x[i * lanes..(i + k) * lanes];
            while *phase < l {
                if w == room {
                    return (w, i);
                }
                let t = &taps[*phase * k..(*phase + 1) * k];
                if lanes == 1 {
                    out[w] = crate::neon::dot_real(t, win);
                } else {
                    out[2 * w..2 * w + 2].copy_from_slice(&crate::neon::dot_pairs(t, win));
                }
                w += 1;
                *phase += m;
            }
            *phase -= l;
        }
        (w, inputs)
    }

    fn fractional(
        &self,
        bank: PolyBank<'_>,
        x: &[f32],
        step: u128,
        pos: &mut u128,
        out: &mut [f32],
    ) -> (usize, usize) {
        let PolyBank { taps, k, lanes } = bank;
        let bits = (taps.len() / k - 1).trailing_zeros();
        let inputs = (x.len() / lanes + 1).saturating_sub(k);
        let room = out.len() / lanes;
        let mut w = 0;
        for i in 0..inputs {
            let win = &x[i * lanes..(i + k) * lanes];
            while *pos < ONE {
                if w == room {
                    return (w, i);
                }
                let frac = *pos as u64;
                let p = (frac >> (64 - bits)) as usize;
                let mu = ((frac << bits) >> 40) as f32 * (1.0 / (1u32 << 24) as f32);
                let (a, b) = taps[p * k..(p + 2) * k].split_at(k);
                if lanes == 1 {
                    out[w] = crate::neon::dot_real_blend(a, b, mu, win);
                } else {
                    let y = crate::neon::dot_pairs_blend(a, b, mu, win);
                    out[2 * w..2 * w + 2].copy_from_slice(&y);
                }
                w += 1;
                *pos += step;
            }
            *pos -= ONE;
        }
        (w, inputs)
    }
}

/// The bank of one resampler, laid out by the backend that runs it. Sample types without a value view ([`Sample::values`])
/// run on the plain loops.
#[derive(Clone)]
struct PolyKernel<T: Sample> {
    backend: &'static dyn ResampleBackend,
    bank: Vec<f32>,
    k: usize,
    /// Whether `backend` is the plain one.
    plain: bool,
    _t: std::marker::PhantomData<T>,
}

impl<T: Sample> std::fmt::Debug for PolyKernel<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PolyKernel")
            .field("backend", &self.backend.name())
            .field("k", &self.k)
            .finish()
    }
}

impl<T: Sample> PolyKernel<T> {
    /// A kernel of `bank` (phases of `k` taps, oldest-sample first), with the first backend of [`BACKENDS`] that prepares it.
    fn new(bank: &[f32], k: usize) -> Self {
        let has_values = T::values(&[T::default()]).is_some();
        let (backend, bank) = BACKENDS
            .build(|b| {
                if has_values {
                    b.prepare(bank, k, T::LANES).map(|p| (b, p))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| (BACKENDS.plain(), bank.to_vec()));
        PolyKernel {
            backend,
            bank,
            k,
            plain: BACKENDS.is_plain(backend),
            _t: std::marker::PhantomData,
        }
    }

    fn plain(&self) -> bool {
        self.plain
    }

    fn poly(&self) -> PolyBank<'_> {
        PolyBank {
            taps: &self.bank,
            k: self.k,
            lanes: T::LANES,
        }
    }

    fn rational(
        &self,
        x: &[T],
        ratio: (usize, usize),
        phase: &mut usize,
        out: &mut [T],
    ) -> (usize, usize) {
        if !self.plain() {
            if let (Some(xv), Some(ov)) = (T::values(x), T::values_mut(out)) {
                return self.backend.rational(self.poly(), xv, ratio, phase, ov);
            }
        }
        rational_plain(&self.bank, self.k, x, ratio, phase, out)
    }

    fn fractional(&self, x: &[T], step: u128, pos: &mut u128, out: &mut [T]) -> (usize, usize) {
        if !self.plain() {
            if let (Some(xv), Some(ov)) = (T::values(x), T::values_mut(out)) {
                return self.backend.fractional(self.poly(), xv, step, pos, ov);
            }
        }
        fractional_plain(&self.bank, self.k, x, step, pos, out)
    }
}

/// Rational resampling by `L / M` (up by L, down by M) with a polyphase bank of `L` branches; only the kept outputs are computed.
#[derive(Clone, Debug)]
pub struct RationalResampler<T: Sample> {
    l: usize,
    m: usize,
    /// `l` phases of `k` taps, each stored oldest-first.
    kernel: PolyKernel<T>,
    /// The last `k - 1` inputs.
    hist: Vec<T>,
    phase: usize,
}

impl<T: Sample> RationalResampler<T> {
    /// Up by `l`, down by `m` (reduced by their gcd), flat to 0.45 and clean above 0.5 of the lower rate, 80 dB. Errors unless
    /// `l` and `m` are at least 1.
    pub fn new(l: usize, m: usize) -> Result<Self, DspError> {
        Self::with_spec(l, m, 0.45, 0.5, 80.0)
    }

    /// Up by `l`, down by `m`, flat to `pass` and attenuated from `stop` (both fractions of the lower of the two rates, `stop <= 1 - pass`).
    /// Errors unless `l` and `m` are at least 1, `0 < pass < stop`, `atten_db` is finite and the filter has at most
    /// [`crate::fir::MAX_TAPS`] taps.
    pub fn with_spec(
        l: usize,
        m: usize,
        pass: f64,
        stop: f64,
        atten_db: f64,
    ) -> Result<Self, DspError> {
        if l == 0 {
            return Param::Interpolation.at_least(1.0, 0.0);
        }
        if m == 0 {
            return Param::Decimation.at_least(1.0, 0.0);
        }
        if pass <= 0.0 || pass.is_nan() {
            return Param::Pass.positive(pass);
        }
        if pass >= stop || pass.is_nan() || stop.is_nan() {
            return Param::Pass.conflict(Param::Stop);
        }
        let g = gcd(l, m);
        let (l, m) = (l / g, m / g);
        // In units of the input rate; the prototype runs at l times the input rate.
        let low = (l as f64 / m as f64).min(1.0);
        let (fp, fs) = (pass * low, stop * low);
        let k = kaiser_len(atten_db, (fs - fp).min(0.49))?;
        let n = k * l;
        let h = windowed_sinc(
            n,
            ((fp + fs) / 2.0 / l as f64).min(0.5),
            design_beta(atten_db),
        )?;
        let mut bank = vec![0.0f32; n];
        for p in 0..l {
            for j in 0..k {
                bank[p * k + (k - 1 - j)] = (h[p + j * l] * l as f64) as f32;
            }
        }
        Ok(RationalResampler {
            l,
            m,
            kernel: PolyKernel::new(&bank, k),
            hist: vec![T::default(); k - 1],
            phase: 0,
        })
    }

    /// The reduced (L, M).
    pub fn ratio(&self) -> (usize, usize) {
        (self.l, self.m)
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.hist.fill(T::default());
        self.phase = 0;
    }

    /// Appends the outputs for `input` to `out`: exactly `L / M` per input on average, counted across calls.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        out.reserve(input.len() * self.l / self.m + 1);
        extend_with_room(&mut self.hist, input);
        let mut chunk = [T::default(); CHUNK];
        let mut used = 0;
        while used < input.len() {
            let ratio = (self.l, self.m);
            let (w, u) =
                self.kernel
                    .rational(&self.hist[used..], ratio, &mut self.phase, &mut chunk);
            out.extend_from_slice(&chunk[..w]);
            used += u;
        }
        self.hist.drain(..used);
    }
}

const PHASE_BITS: u32 = 7;
const PHASES: usize = 1 << PHASE_BITS;

/// The step `num / den` in Q64.64, rounded up. For integer rates it is within 2^-64 of the exact ratio, and rounding up means an exact
/// multiple (for example 441000 inputs at 44.1 -> 48 kHz) produces exactly the ideal count, not one more.
fn step_q64(num: f64, den: f64) -> u128 {
    let int = |v: f64| v.fract() == 0.0 && v > 0.0 && v < 9.0e15;
    if int(num) && int(den) {
        ((num as u128) << 64).div_ceil(den as u128)
    } else {
        (num / den * ONE as f64) as u128
    }
}

/// The bank of [`FractionalResampler::new`] for these settings: the taps per phase `k` and the `PHASES + 1` phases of `k` taps,
/// each oldest-sample first. Errors for a pass edge that is not positive, a stop edge not above it, edges whose middle is
/// above half the input rate, or too many taps.
fn fractional_bank(
    in_rate: f64,
    pass_hz: f64,
    stop_hz: f64,
    atten_db: f64,
) -> Result<(usize, Vec<f32>), DspError> {
    if pass_hz <= 0.0 || pass_hz.is_nan() {
        return Param::Pass.positive(pass_hz);
    }
    if stop_hz <= pass_hz || stop_hz.is_nan() || pass_hz.is_nan() {
        return Param::Pass.conflict(Param::Stop);
    }
    if (pass_hz + stop_hz) / 2.0 > in_rate / 2.0 + 1e-9 {
        return Param::Stop.conflict(Param::InputRate);
    }
    let k = kaiser_len(atten_db, ((stop_hz - pass_hz) / in_rate).min(0.49))?;
    let n = k * PHASES + 1;
    let cutoff = (pass_hz + stop_hz) / 2.0 / in_rate / PHASES as f64;
    let h = windowed_sinc(n, cutoff, design_beta(atten_db))?;
    let mut bank = vec![0.0f32; (PHASES + 1) * k];
    for p in 0..=PHASES {
        for j in 0..k {
            bank[p * k + (k - 1 - j)] = (h[p + j * PHASES] * PHASES as f64) as f32;
        }
    }
    Ok((k, bank))
}

/// Arbitrary-ratio resampling with a 128-phase polyphase bank and linear interpolation between neighbouring phases. The ratio can be
/// changed at any time (clock-drift correction) without a glitch. Best for ratios between about 0.25 and 4; [`Resampler`] puts a
/// decimator in front for larger reductions.
///
/// The anti-alias low-pass has its own pass and stop edges, so it can be much narrower than the output rate. A complex input can be
/// shifted in frequency before the filter ([`set_shift`](Self::set_shift)). The first output can start at any fractional input
/// position ([`set_start`](Self::set_start)), and [`process_slice`](Self::process_slice) reads a stored capture from any position.
///
/// ```
/// use sdrkit_algo_dsp::resample::FractionalResampler;
/// use sdrkit_algo_dsp::Complex32;
/// // 1 Msps to 250 ksps, 20 kHz wide, a signal at +100 kHz moved to DC.
/// let mut r = FractionalResampler::<Complex32>::new(1e6, 250e3, 10e3, 30e3, 70.0).unwrap();
/// r.set_shift(100e3).unwrap();
/// let x: Vec<Complex32> = (0..4000)
///     .map(|i| Complex32::from_polar(1.0, (std::f64::consts::TAU * 0.1 * i as f64) as f32))
///     .collect();
/// let mut out = Vec::new();
/// r.process_slice(&x, 1000.0, 100, &mut out).unwrap();
/// assert_eq!(out.len(), 100);
/// assert!((out[50] - Complex32::new(1.0, 0.0)).norm() < 1e-3);
/// ```
#[derive(Clone, Debug)]
pub struct FractionalResampler<T: Sample> {
    k: usize,
    /// `PHASES + 1` phases of `k` taps, oldest-first.
    kernel: PolyKernel<T>,
    /// The last `k - 1` inputs.
    hist: Vec<T>,
    /// Time of the next output, in input samples from the next input to be pushed (Q64.64).
    pos: u128,
    step: u128,
    base_step: u128,
    correction: f64,
    in_rate: f64,
    out_rate: f64,
    /// The mixer in front of the filter, when there is a frequency shift.
    nco: Option<Nco>,
}

/// Inputs mixed at a time, on the stack.
const MIX_CHUNK: usize = 256;

impl<T: Sample> FractionalResampler<T> {
    /// From `in_rate` to `out_rate`, flat to `pass_hz`, attenuated by `atten_db` from `stop_hz` (both at or below half the input rate
    /// for the filter to make sense: `stop_hz <= in_rate`, and for a reduction `stop_hz <= out_rate - pass_hz` keeps the passband alias-free).
    pub fn new(
        in_rate: f64,
        out_rate: f64,
        pass_hz: f64,
        stop_hz: f64,
        atten_db: f64,
    ) -> Result<Self, DspError> {
        if in_rate <= 0.0 || in_rate.is_nan() {
            return Param::InputRate.positive(in_rate);
        }
        if out_rate <= 0.0 || out_rate.is_nan() {
            return Param::OutputRate.positive(out_rate);
        }
        let (k, bank) = fractional_bank(in_rate, pass_hz, stop_hz, atten_db)?;
        let base = step_q64(in_rate, out_rate);
        Ok(FractionalResampler {
            k,
            kernel: PolyKernel::new(&bank, k),
            hist: vec![T::default(); k - 1],
            pos: 0,
            step: base,
            base_step: base,
            correction: 1.0,
            in_rate,
            out_rate,
            nco: None,
        })
    }

    /// The default quality: flat to 0.42 and clean (no aliases or images) above 0.5 of the lower rate, 80 dB.
    pub fn with_rates(in_rate: f64, out_rate: f64) -> Result<Self, DspError> {
        let low = in_rate.min(out_rate);
        Self::new(in_rate, out_rate, 0.42 * low, 0.5 * low, 80.0)
    }

    /// Sets the nominal rates again (the filter is kept; fine for small changes). The correction factor stays.
    pub fn set_rates(&mut self, in_rate: f64, out_rate: f64) {
        self.in_rate = in_rate;
        self.out_rate = out_rate;
        self.base_step = step_q64(in_rate, out_rate);
        self.set_correction(self.correction);
    }

    /// Clock-drift correction: each output advances `factor * in/out` input samples, so `factor > 1` produces fewer outputs (use it when
    /// the input clock runs fast by that factor). `1.0` restores the exact nominal ratio. Takes effect at the next output, without a jump.
    pub fn set_correction(&mut self, factor: f64) {
        assert!(factor > 0.0);
        self.correction = factor;
        self.step = if factor == 1.0 {
            self.base_step
        } else {
            (self.base_step as f64 * factor) as u128
        };
    }

    /// The current correction factor.
    pub fn correction(&self) -> f64 {
        self.correction
    }

    /// Output samples per input sample, including the correction.
    pub fn ratio(&self) -> f64 {
        ONE as f64 / self.step as f64
    }

    /// Taps per phase.
    pub fn taps_per_phase(&self) -> usize {
        self.k
    }

    /// The filter delay in input samples.
    pub fn delay(&self) -> f64 {
        self.k as f64 / 2.0
    }

    /// Clears the history and the time, and sets the mixer phase to zero. The rates, the correction and the shift stay.
    pub fn reset(&mut self) {
        self.hist.fill(T::default());
        self.pos = 0;
        if let Some(n) = &mut self.nco {
            n.set_phase_cycles(0.0);
        }
    }

    /// The frequency shift in Hz: 0 when there is none. See [`FractionalResampler::set_shift`].
    pub fn shift(&self) -> f64 {
        self.nco.as_ref().map_or(0.0, |n| -n.freq())
    }

    /// Places the next output at `offset` input samples after the next input. The output at that time is the filtered input there,
    /// so the filter delay is already removed. Later outputs follow at the step of [`ratio`](Self::ratio). Returns an error if
    /// `offset` is not finite or is less than `-delay()`. After [`new`](Self::new) and [`reset`](Self::reset), the offset is
    /// `-delay()`.
    pub fn set_start(&mut self, offset: f64) -> Result<(), DspError> {
        let v = offset + self.delay();
        if !(0.0..9.0e15).contains(&v) {
            return Param::Start.range(-self.delay(), 9.0e15 - self.delay(), offset);
        }
        let whole = v.floor();
        self.pos = ((whole as u128) << 64) | ((v - whole) * ONE as f64) as u128;
        Ok(())
    }

    /// Clears `out` and fills it with exactly `n_out` outputs read from the stored slice `x`. Output `n` is the filtered input at
    /// position `start + n / ratio()` (in samples from `x[0]`). Input outside `x` counts as zero, so the output at a position far
    /// outside is zero. The mixer phase is zero at `x[0]`, whatever `start` is. The history and the time are reset first. Returns an error if
    /// `start` is not finite.
    pub fn process_slice(
        &mut self,
        x: &[T],
        start: f64,
        n_out: usize,
        out: &mut Vec<T>,
    ) -> Result<(), DspError> {
        out.clear();
        if !start.is_finite() || start.abs() > 4.0e15 {
            return Param::Start.range(-4.0e15, 4.0e15, start);
        }
        if n_out == 0 {
            return Ok(());
        }
        self.hist.truncate(self.k - 1);
        self.hist.fill(T::default());
        let half = self.delay();
        // The first input pushed: one sample before the first filter window.
        let first = (start - half).floor() as i64 - 1;
        let offset = start - first as f64;
        self.set_start(offset)?;
        if let Some(n) = &mut self.nco {
            n.set_phase_cycles(n.freq() / n.rate() * first as f64);
        }
        let step = self.step as f64 / ONE as f64;
        let last = offset + (n_out - 1) as f64 * step;
        let total = (last + half).floor() as i64 + 2;
        out.reserve(n_out + 4);
        let len = x.len() as i64;
        let zeros = [T::default(); MIX_CHUNK];
        let mut i = 0i64;
        while i < total && out.len() < n_out {
            let g = first + i;
            let n = if g < 0 {
                let n = (total - i).min(-g).min(MIX_CHUNK as i64);
                self.process(&zeros[..n as usize], out);
                n
            } else if g < len {
                let n = (total - i).min(len - g);
                self.process(&x[g as usize..(g + n) as usize], out);
                n
            } else {
                let n = (total - i).min(MIX_CHUNK as i64);
                self.process(&zeros[..n as usize], out);
                n
            };
            i += n;
        }
        out.resize(n_out, T::default());
        Ok(())
    }

    /// How many outputs the next `n` inputs will give (exact; does not change the state).
    pub fn outputs_for(&self, n: u64) -> u64 {
        let end = n as u128 * ONE;
        if end <= self.pos {
            0
        } else {
            (end - self.pos).div_ceil(self.step) as u64
        }
    }

    /// Appends the outputs for `input` to `out`. If there is a frequency shift, the input is mixed first.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        out.reserve(self.outputs_for(input.len() as u64) as usize);
        let Some(mut nco) = self.nco.take() else {
            self.run(input, out);
            return;
        };
        let mut buf = [T::default(); MIX_CHUNK];
        for c in input.chunks(MIX_CHUNK) {
            let b = &mut buf[..c.len()];
            b.copy_from_slice(c);
            T::mix_in_place(&mut nco, b);
            self.run(b, out);
        }
        self.nco = Some(nco);
    }

    fn run(&mut self, input: &[T], out: &mut impl Append<T>) {
        extend_with_room(&mut self.hist, input);
        let mut chunk = [T::default(); CHUNK];
        let mut used = 0;
        while used < input.len() {
            let (w, u) =
                self.kernel
                    .fractional(&self.hist[used..], self.step, &mut self.pos, &mut chunk);
            out.extend_from_slice(&chunk[..w]);
            used += u;
        }
        self.hist.drain(..used);
    }
}

impl FractionalResampler<Complex32> {
    /// Moves a signal at `+shift_hz` down to DC before the filter: the input is multiplied by `e^{-j 2 pi shift_hz t}`. The mixer
    /// phase is zero at the first input after [`new`](Self::new) or [`reset`](Self::reset), and a later change of the shift keeps
    /// the phase continuous. `0.0` removes the mixer. Returns an error if `shift_hz` is not finite.
    pub fn set_shift(&mut self, shift_hz: f64) -> Result<(), DspError> {
        if !shift_hz.is_finite() {
            return Param::Shift.not_finite();
        }
        match (&mut self.nco, shift_hz == 0.0) {
            (_, true) => self.nco = None,
            (Some(n), false) => n.set_freq(-shift_hz),
            (None, false) => self.nco = Some(Nco::new(self.in_rate, -shift_hz)),
        }
        Ok(())
    }
}

/// The most output times of one period for which [`FilterShiftResampler`] keeps a phase of its own.
pub const EXACT_PHASES: usize = 256;

/// `(L, M)` in lowest terms with `out_rate / in_rate = L / M`, when both rates are whole numbers and `L` is at most
/// [`EXACT_PHASES`].
fn exact_ratio(in_rate: f64, out_rate: f64) -> Option<(usize, usize)> {
    let whole = |v: f64| v.fract() == 0.0 && (1.0..9.0e15).contains(&v);
    if !(whole(in_rate) && whole(out_rate)) {
        return None;
    }
    let (i, o) = (in_rate as usize, out_rate as usize);
    let g = gcd(i, o);
    (o / g <= EXACT_PHASES).then_some((o / g, i / g))
}

/// A real signal through a FIR filter with complex taps, a frequency shift and a [`FractionalResampler`], as one polyphase
/// stage. Each output is one dot product of the real input with the combined taps of its time, turned by the shift at the
/// newest input of its window: the stage costs about one filter at the output rate. The output is that of the three stages
/// in a row ([`crate::fir::Fir`] on the real and on the imaginary taps, an [`Nco`] at the input rate from phase 0, and the
/// resampler) to rounding.
///
/// When both rates are whole numbers and the output rate over the input rate is `L / M` in lowest terms with `L` at most
/// [`EXACT_PHASES`], the stage keeps one phase for each of the `L` output times of a period, with the resampler's taps at
/// that time; otherwise it blends neighbouring phases as the resampler does.
///
/// ```
/// use sdrkit_algo_dsp::resample::FilterShiftResampler;
/// use sdrkit_algo_dsp::Complex32;
/// // 12 MHz to 10 MHz, the band above 0 Hz kept and moved down by 2.75 MHz.
/// let taps = [Complex32::new(0.5, 0.0), Complex32::new(0.0, 0.3), Complex32::new(0.5, 0.0)];
/// let mut r = FilterShiftResampler::new(&taps, -2.75e6, 12e6, 10e6, 4e6, 6e6, 55.0).unwrap();
/// assert_eq!(r.exact_ratio(), Some((5, 6)));
/// let mut out = Vec::new();
/// r.process(&[0.1; 1200], &mut out);
/// assert_eq!(out.len(), 1000);
/// ```
#[derive(Clone, Debug)]
pub struct FilterShiftResampler {
    form: ShiftForm,
    /// The last `taps - 1` inputs, then the new ones.
    hist: Vec<f32>,
    /// The shift, and its value at each new input.
    nco: Nco,
    turn: Vec<Complex32>,
}

/// Output `i` of `out_a` (of `out_b`) is the dot product of the taps `a` (`b`) with `x[i * stride..]`: the NEON loop on
/// ARM64, which loads each window once for both, and the plain dot products elsewhere.
fn fir_two(a: &[f32], b: &[f32], x: &[f32], stride: usize, out_a: &mut [f32], out_b: &mut [f32]) {
    #[cfg(target_arch = "aarch64")]
    crate::neon::fir_real_two(a, b, x, stride, out_a, out_b);
    #[cfg(not(target_arch = "aarch64"))]
    for (i, (p, q)) in out_a.iter_mut().zip(out_b.iter_mut()).enumerate() {
        let w = &x[i * stride..i * stride + a.len()];
        (*p, *q) = (f32::dot(a, w), f32::dot(b, w));
    }
}

/// The two forms of [`FilterShiftResampler`].
#[derive(Clone, Debug)]
enum ShiftForm {
    /// `L` phases, one for each output time of a period; output `j` of a call is at time `phase + j M` (in `1 / L` inputs).
    /// The outputs of one phase are one strided filter run of its real and imaginary taps ([`fir_two`]).
    Exact {
        l: usize,
        m: usize,
        /// The real and the imaginary taps of the phases, `q` a phase, each oldest-sample first.
        q: usize,
        re: Vec<f32>,
        im: Vec<f32>,
        phase: usize,
        /// The real and imaginary outputs of one phase, and the outputs of a call in order.
        a: Vec<f32>,
        b: Vec<f32>,
        out: Vec<Complex32>,
    },
    /// `PHASES + 1` phases blended as [`FractionalResampler`] blends them, at the time `pos` and the step `step` (Q64.64).
    Blended {
        re: PolyKernel<f32>,
        im: PolyKernel<f32>,
        pos: u128,
        step: u128,
    },
}

impl FilterShiftResampler {
    /// `taps` (`taps[0]` multiplies the newest sample, as in [`crate::fir::Fir`]), then a shift by `shift_hz`, then the
    /// resampler `FractionalResampler::new(in_rate, out_rate, pass_hz, stop_hz, atten_db)`. Errors as that resampler, or for
    /// no taps, a tap that is not finite or a shift that is not finite.
    pub fn new(
        taps: &[Complex32],
        shift_hz: f64,
        in_rate: f64,
        out_rate: f64,
        pass_hz: f64,
        stop_hz: f64,
        atten_db: f64,
    ) -> Result<Self, DspError> {
        use num_complex::Complex64;
        if taps.is_empty() {
            return Param::Taps.at_least(1.0, 0.0);
        }
        if taps.iter().any(|t| !(t.re.is_finite() && t.im.is_finite())) {
            return Param::Taps.not_finite();
        }
        if !shift_hz.is_finite() {
            return Param::Shift.not_finite();
        }
        // The checks of the resampler, then its bank.
        FractionalResampler::<f32>::new(in_rate, out_rate, pass_hz, stop_hz, atten_db)?;
        let (k, bank) = fractional_bank(in_rate, pass_hz, stop_hz, atten_db)?;
        let q = k + taps.len() - 1;
        // Output `y = sum_j r[j] e^{jw (i - j)} sum_t taps[t] x[i - j - t]` at the newest input `i` of its window, with the
        // resampler's taps `r` (newest first) of its time: the combined tap `n` is `sum_{j + t = n} r[j] e^{-jwj} taps[t]`.
        let w = std::f64::consts::TAU * shift_hz / in_rate;
        let (mut re, mut im) = (Vec::new(), Vec::new());
        let mut combine = |r: &[f32]| {
            let mut c = vec![Complex64::default(); q];
            for (j, &rj) in r.iter().enumerate() {
                let a = Complex64::from_polar(f64::from(rj), -w * j as f64);
                for (t, h) in taps.iter().enumerate() {
                    c[j + t] += a * Complex64::new(f64::from(h.re), f64::from(h.im));
                }
            }
            re.extend(c.iter().rev().map(|z| z.re as f32));
            im.extend(c.iter().rev().map(|z| z.im as f32));
        };
        let grid = |g: usize, j: usize| bank[g * k + (k - 1 - j)];
        let ratio = exact_ratio(in_rate, out_rate);
        match ratio {
            Some((l, _)) => {
                // The resampler's taps at time p / L: its two phases around it blended as its loop blends them.
                for p in 0..l {
                    let frac = (((p as u128) << 64) / l as u128) as u64;
                    let g = (frac >> (64 - PHASE_BITS)) as usize;
                    let mu = ((frac << PHASE_BITS) >> 40) as f32 * (1.0 / (1u32 << 24) as f32);
                    let r: Vec<f32> = (0..k)
                        .map(|j| (grid(g + 1, j) - grid(g, j)).mul_add(mu, grid(g, j)))
                        .collect();
                    combine(&r);
                }
            }
            None => {
                for g in 0..=PHASES {
                    let r: Vec<f32> = (0..k).map(|j| grid(g, j)).collect();
                    combine(&r);
                }
            }
        }
        let form = match ratio {
            Some((l, m)) => ShiftForm::Exact {
                l,
                m,
                q,
                re,
                im,
                phase: 0,
                a: Vec::new(),
                b: Vec::new(),
                out: Vec::new(),
            },
            None => ShiftForm::Blended {
                re: PolyKernel::new(&re, q),
                im: PolyKernel::new(&im, q),
                pos: 0,
                step: step_q64(in_rate, out_rate),
            },
        };
        Ok(FilterShiftResampler {
            form,
            hist: vec![0.0; q - 1],
            nco: Nco::new(in_rate, shift_hz),
            turn: Vec::new(),
        })
    }

    /// `(L, M)` when the stage keeps a phase for each output time of a period, `None` when it blends phases.
    pub fn exact_ratio(&self) -> Option<(usize, usize)> {
        match self.form {
            ShiftForm::Exact { l, m, .. } => Some((l, m)),
            ShiftForm::Blended { .. } => None,
        }
    }

    /// Appends the outputs for `input` to `out`: as many as the resampler gives for it, counted across calls.
    pub fn process(&mut self, input: &[f32], out: &mut impl Append<Complex32>) {
        let n = input.len();
        if self.turn.len() < n {
            self.turn.resize(n, Complex32::default());
        }
        self.nco.fill(&mut self.turn[..n]);
        extend_with_room(&mut self.hist, input);
        let turn = &self.turn[..n];
        match &mut self.form {
            ShiftForm::Exact {
                l,
                m,
                q,
                re,
                im,
                phase,
                a,
                b,
                out: all,
            } => {
                let (l, m, q) = (*l, *m, *q);
                // The outputs whose time is before the end of the input, at times `phase + j m`.
                let w = (n * l).saturating_sub(*phase).div_ceil(m);
                all.clear();
                all.resize(w, Complex32::default());
                for r in 0..w.min(l) {
                    let t = *phase + r * m;
                    let (i, p) = (t / l, t % l);
                    let c = (w - r).div_ceil(l);
                    a.resize(c, 0.0);
                    b.resize(c, 0.0);
                    let taps = p * q..(p + 1) * q;
                    fir_two(&re[taps.clone()], &im[taps], &self.hist[i..], m, a, b);
                    for (k, (&x, &y)) in a.iter().zip(b.iter()).enumerate() {
                        all[r + k * l] = Complex32::new(x, y) * turn[i + k * m];
                    }
                }
                *phase = *phase + w * m - n * l;
                out.extend_from_slice(all);
            }
            ShiftForm::Blended { re, im, pos, step } => {
                out.reserve((n as u128 * ONE / *step) as usize + 1);
                let (mut a, mut b) = ([0.0f32; CHUNK], [0.0f32; CHUNK]);
                let mut used = 0;
                while used < n {
                    let x = &self.hist[used..];
                    // Output `j` of a call is made at input `(start + j * step) / 2^64` of the call.
                    let start = *pos;
                    let (w, u) = re.fractional(x, *step, pos, &mut a);
                    let mut p = start;
                    im.fractional(x, *step, &mut p, &mut b);
                    let mut t = start;
                    for (&x, &y) in a[..w].iter().zip(&b[..w]) {
                        out.push(Complex32::new(x, y) * turn[used + (t >> 64) as usize]);
                        t += *step;
                    }
                    used += u;
                }
            }
        }
        self.hist.drain(..n);
    }
}

/// Resampling between any two rates (2.4 MHz to 48 kHz, 250 kHz to 48 kHz, 44.1 kHz to 48 kHz, ...).
///
/// The input first goes through a multi-stage [`Decimator`] by the largest integer `D <= in/out` whose prime factors are at most 7 (or
/// the exact ratio when it is an integer), then through a [`FractionalResampler`] unless the decimator already lands on the output rate.
/// The output is flat to `pass` of the lower rate and has no aliases or images above 0.5 of it.
#[derive(Clone, Debug)]
pub struct Resampler<T: Sample> {
    decim: Option<Decimator<T>>,
    frac: Option<FractionalResampler<T>>,
    scratch: Vec<T>,
    frames_in: Vec<T>,
    frames_out: Vec<T>,
    in_rate: f64,
    out_rate: f64,
    mid_rate: f64,
    pass: f64,
    atten_db: f64,
}

fn smooth_at_most(n: usize) -> usize {
    (1..=n)
        .rev()
        .find(|&c| {
            let mut v = c;
            for p in [2, 3, 5, 7] {
                while v % p == 0 {
                    v /= p;
                }
            }
            v == 1
        })
        .unwrap_or(1)
}

impl<T: Sample> Resampler<T> {
    /// The default quality: flat to 0.42 of the lower rate, 80 dB.
    pub fn new(in_rate: f64, out_rate: f64) -> Result<Self, DspError> {
        Self::with_quality(in_rate, out_rate, 0.42, 80.0)
    }

    /// Flat to `pass` (a fraction of the lower rate, below 0.5), stopband `atten_db`.
    pub fn with_quality(
        in_rate: f64,
        out_rate: f64,
        pass: f64,
        atten_db: f64,
    ) -> Result<Self, DspError> {
        if !(in_rate > 0.0 && in_rate.is_finite()) {
            return Param::InputRate.positive(in_rate);
        }
        if !(out_rate > 0.0 && out_rate.is_finite()) {
            return Param::OutputRate.positive(out_rate);
        }
        if !(pass > 0.0 && pass < 0.5) {
            return Param::Pass.range(0.0, 0.5, pass);
        }
        let r = in_rate / out_rate;
        let near = r.round();
        let exact_int = near >= 1.0 && (r - near).abs() < 1e-12 * r;
        let d = if exact_int {
            near as usize
        } else {
            smooth_at_most(r.floor().max(1.0) as usize)
        };
        let mid = in_rate / d as f64;
        let pass_hz = pass * in_rate.min(out_rate);
        let decim = (d > 1).then(|| {
            Decimator::with_alias_free(
                in_rate,
                d,
                pass_hz,
                (out_rate / 2.0).min(mid - pass_hz),
                atten_db,
            )
        });
        let mut s = Resampler {
            decim,
            frac: None,
            scratch: Vec::new(),
            frames_in: Vec::new(),
            frames_out: Vec::new(),
            in_rate,
            out_rate,
            mid_rate: mid,
            pass,
            atten_db,
        };
        if !exact_int {
            s.build_frac()?;
        }
        Ok(s)
    }

    fn build_frac(&mut self) -> Result<(), DspError> {
        let low = self.mid_rate.min(self.out_rate);
        let mut f = FractionalResampler::new(
            self.mid_rate,
            self.out_rate,
            self.pass * low,
            0.5 * low,
            self.atten_db,
        )?;
        // The step is in / (d * out), exact for integer rates, so long runs keep their count.
        f.base_step = step_q64(self.in_rate, self.decimation() as f64 * self.out_rate);
        f.step = f.base_step;
        self.frac = Some(f);
        Ok(())
    }

    /// The input rate.
    pub fn input_rate(&self) -> f64 {
        self.in_rate
    }

    /// The output rate.
    pub fn output_rate(&self) -> f64 {
        self.out_rate
    }

    /// The integer decimation in front of the fractional stage (1 when there is none).
    pub fn decimation(&self) -> usize {
        self.decim.as_ref().map_or(1, |d| d.factor())
    }

    /// The delay through all stages, in input samples: output `n` is the filtered input at `n * input_rate / output_rate - delay()`.
    pub fn delay(&self) -> f64 {
        // Each decimator stage outputs on the last input of its group, so the decimator output `j` is at input `j * d + d - 1`.
        let d = self.decimation() as f64;
        self.decim.as_ref().map_or(0.0, |s| s.delay() - (d - 1.0))
            + self.frac.as_ref().map_or(0.0, |f| d * f.delay())
    }

    /// Whether there is a fractional stage.
    pub fn has_fractional_stage(&self) -> bool {
        self.frac.is_some()
    }

    /// Drift correction: see [`FractionalResampler::set_correction`]. Adds the fractional stage if there was none (this allocates once).
    pub fn set_correction(&mut self, factor: f64) {
        if self.frac.is_none() {
            if factor == 1.0 {
                return;
            }
            self.build_frac()
                .expect("a fractional stage fits any rate pair that built");
        }
        if let Some(f) = &mut self.frac {
            f.set_correction(factor);
        }
    }

    /// Clears all state.
    pub fn reset(&mut self) {
        if let Some(d) = &mut self.decim {
            d.reset();
        }
        if let Some(f) = &mut self.frac {
            f.reset();
        }
    }

    /// Appends the outputs for `input` to `out`.
    pub fn process(&mut self, input: &[T], out: &mut impl Append<T>) {
        match (&mut self.decim, &mut self.frac) {
            (None, None) => out.extend_from_slice(input),
            (Some(d), None) => d.process(input, out),
            (None, Some(f)) => f.process(input, out),
            (Some(d), Some(f)) => {
                self.scratch.clear();
                d.process(input, &mut self.scratch);
                f.process(&self.scratch, out);
            }
        }
    }
}

impl Resampler<Stereo> {
    /// Resamples interleaved stereo `[l, r, ...]`, appending interleaved output. Blocks should hold whole frames.
    pub fn process_interleaved(&mut self, input: &[f32], out: &mut Vec<f32>) {
        let mut fi = std::mem::take(&mut self.frames_in);
        let mut fo = std::mem::take(&mut self.frames_out);
        fi.clear();
        fo.clear();
        Stereo::from_interleaved(input, &mut fi);
        self.process(&fi, &mut fo);
        Stereo::to_interleaved(&fo, out);
        self.frames_in = fi;
        self.frames_out = fo;
    }
}

/// Resamples a whole span of samples from `in_rate` to `out_rate` with a [`Resampler`], into `out` (cleared first). Output
/// sample `j` is the signal at input time `j * in_rate / out_rate`, and the span has `floor(len * out_rate / in_rate)` output
/// samples. The first and last samples are held beyond the ends, so the filter does not fade the edges. Equal rates copy the
/// span. If the resampler refuses the rates, `out` stays empty. [`SpanResampler`] does the same with the resampler and its
/// buffers kept from one span to the next.
pub fn resample_span(x: &[Complex32], in_rate: f64, out_rate: f64, out: &mut Vec<Complex32>) {
    out.clear();
    if let Ok(mut r) = SpanResampler::new(in_rate, out_rate) {
        r.run(x, out);
    }
}

/// [`resample_span`] for many spans at one pair of rates: the resampler and the buffers stay from one call to the next, so
/// a span allocates nothing once the buffers have the size of the longest span. Each span is resampled on its own: the
/// filter starts again at each one, and the output is the output of [`resample_span`] to the bit.
pub struct SpanResampler {
    /// `None` for equal rates: a span is copied.
    r: Option<Resampler<Complex32>>,
    /// Output samples per input sample.
    ratio: f64,
    input: Vec<Complex32>,
    y: Vec<Complex32>,
}

impl std::fmt::Debug for SpanResampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpanResampler")
            .field("ratio", &self.ratio)
            .finish_non_exhaustive()
    }
}

impl SpanResampler {
    /// A resampler of spans from `in_rate` to `out_rate`.
    ///
    /// # Errors
    ///
    /// The errors of [`Resampler::new`] for rates that are not equal.
    pub fn new(in_rate: f64, out_rate: f64) -> Result<Self, DspError> {
        let r = if (in_rate - out_rate).abs() < 1e-6 * in_rate {
            None
        } else {
            Some(Resampler::new(in_rate, out_rate)?)
        };
        Ok(SpanResampler {
            r,
            ratio: out_rate / in_rate,
            input: Vec::new(),
            y: Vec::new(),
        })
    }

    /// The same resampler with room for spans of up to `samples` input samples: such a span allocates nothing, the first
    /// one included. The room is made by resampling one span of that length.
    #[must_use]
    pub fn with_room(mut self, samples: usize) -> Self {
        if self.r.is_some() {
            let mut out = Vec::new();
            self.run(&vec![Complex32::default(); samples.max(1)], &mut out);
        }
        self
    }

    /// Resamples the span `x` into `out` (cleared first), as [`resample_span`] does.
    pub fn run(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        out.clear();
        let (Some(&first), Some(&last)) = (x.first(), x.last()) else {
            return;
        };
        let Some(r) = self.r.as_mut() else {
            out.extend_from_slice(x);
            return;
        };
        r.reset();
        let d = r.delay();
        let pad = d.ceil() as usize + 16;
        self.input.clear();
        self.input.resize(pad, first);
        self.input.extend_from_slice(x);
        self.input
            .resize(self.input.len() + pad + 2 * d.ceil() as usize, last);
        self.y.clear();
        r.process(&self.input, &mut self.y);
        let skip = ((pad as f64 + d) * self.ratio).round() as usize;
        let n = (x.len() as f64 * self.ratio).floor() as usize;
        out.extend(self.y.iter().skip(skip).take(n));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_backend_agrees_with_the_plain_one() {
        for &b in BACKENDS.all() {
            check_backend(b);
        }
    }

    #[test]
    fn frames_go_straight_into_interleaved_audio() {
        use crate::{Frame, FrameWriter};
        let x: Vec<f32> = (0..9600).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut a = Resampler::<Frame<2>>::new(48e3, 44.1e3).unwrap();
        let mut frames = Vec::new();
        a.process(Frame::view(&x), &mut frames);
        let mut b = Resampler::<Frame<2>>::new(48e3, 44.1e3).unwrap();
        let mut flat = Vec::new();
        b.process(Frame::view(&x), &mut FrameWriter::<2>(&mut flat));
        assert_eq!(flat, Frame::flat(&frames));
    }
    use crate::testutil::*;
    use crate::Complex32;

    fn check_tone(in_rate: f64, out_rate: f64, f: f64, secs: f64) {
        let n = (in_rate * secs) as usize;
        let x = ctone(n, f / in_rate, 0.8);
        let mut r = Resampler::<Complex32>::new(in_rate, out_rate).unwrap();
        let mut out = Vec::new();
        for c in x.chunks(1000) {
            r.process(c, &mut out);
        }
        let want = n as f64 * out_rate / in_rate;
        assert!(
            (out.len() as f64 - want).abs() <= 1.0,
            "{in_rate}->{out_rate}: {} outputs, want {want}",
            out.len()
        );
        let skip = out.len() / 4;
        let a = camp(&out[skip..], f / out_rate);
        assert!(
            db(a / 0.8).abs() < 0.05,
            "{in_rate}->{out_rate}: tone {}",
            db(a / 0.8)
        );
    }

    /// The three stages that [`FilterShiftResampler`] fuses, one after the other.
    fn three_stages(taps: &[Complex32], shift: f64, rates: (f64, f64), edges: (f64, f64, f64), x: &[f32]) -> Vec<Complex32> {
        let re: Vec<f32> = taps.iter().map(|t| t.re).collect();
        let im: Vec<f32> = taps.iter().map(|t| t.im).collect();
        let (mut a, mut b) = (crate::fir::Fir::<f32>::new(&re), crate::fir::Fir::<f32>::new(&im));
        let (mut ya, mut yb) = (vec![0.0; x.len()], vec![0.0; x.len()]);
        a.process(x, &mut ya);
        b.process(x, &mut yb);
        let mut z: Vec<Complex32> = ya.iter().zip(&yb).map(|(&r, &i)| Complex32::new(r, i)).collect();
        Nco::new(rates.0, shift).mix_in_place(&mut z);
        let mut r = FractionalResampler::new(rates.0, rates.1, edges.0, edges.1, edges.2).unwrap();
        let mut out = Vec::new();
        r.process(&z, &mut out);
        out
    }

    #[test]
    fn the_filter_shift_resampler_is_its_three_stages_to_rounding() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut u = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        };
        let taps: Vec<Complex32> = (0..97).map(|_| Complex32::new(0.02 * u(), 0.02 * u())).collect();
        let x: Vec<f32> = (0..30_000).map(|_| u()).collect();
        for (rates, edges, exact) in [
            ((12e6, 10e6), (4e6, 6e6, 55.0), Some((5, 6))),
            ((12e6, 10e6), (4.5e6, 5.5e6, 55.0), Some((5, 6))),
            ((24e6, 10e6), (4.7e6, 5.3e6, 55.0), Some((5, 12))),
            ((12e6, 8e6), (3e6, 4.5e6, 55.0), Some((2, 3))),
            ((12e6, 9_876_543.0), (4e6, 5.5e6, 55.0), None),
        ] {
            let want = three_stages(&taps, -2.75e6, rates, edges, &x);
            let mut f = FilterShiftResampler::new(&taps, -2.75e6, rates.0, rates.1, edges.0, edges.1, edges.2).unwrap();
            assert_eq!(f.exact_ratio(), exact);
            let mut got = Vec::new();
            let mut i = 0;
            for n in [1usize, 7, 4096, 333, 9999].iter().cycle() {
                if i == x.len() {
                    break;
                }
                let n = (*n).min(x.len() - i);
                f.process(&x[i..i + n], &mut got);
                i += n;
            }
            assert_eq!(got.len(), want.len(), "{rates:?}");
            let peak = want.iter().map(|z| z.norm()).fold(0.0, f32::max);
            let worst = got.iter().zip(&want).map(|(g, w)| (g - w).norm()).fold(0.0, f32::max);
            assert!(worst < 2e-6 * peak, "{rates:?}: {worst} against a peak of {peak}");
        }
    }

    #[test]
    fn rates_and_tones() {
        check_tone(2.4e6, 48_000.0, 5_000.0, 0.05);
        check_tone(250_000.0, 48_000.0, 12_000.0, 0.1);
        check_tone(44_100.0, 48_000.0, 1_000.0, 0.2);
        check_tone(48_000.0, 44_100.0, 15_000.0, 0.2);
        check_tone(48_000.0, 48_000.0 * 1.0007, 3_000.0, 0.2);
        let r = Resampler::<f32>::new(2.4e6, 48_000.0).unwrap();
        assert_eq!(r.decimation(), 50);
        assert!(!r.has_fractional_stage());
        let r = Resampler::<f32>::new(250_000.0, 48_000.0).unwrap();
        assert_eq!(r.decimation(), 5);
    }

    #[test]
    fn rejects_images_and_aliases() {
        // 44.1 -> 48 kHz: a 20 kHz tone must not image at 24.1 kHz (the mirror about 22.05 kHz shows at -23.9 kHz in complex output).
        let (fi, fo) = (44_100.0, 48_000.0);
        let x = ctone(44_100, 20_000.0 / fi, 1.0);
        let mut r = FractionalResampler::<Complex32>::with_rates(fi, fo).unwrap();
        let mut out = Vec::new();
        r.process(&x, &mut out);
        let img = camp(&out[2000..], (20_000.0 - 44_100.0) / fo);
        assert!(db(img) < -80.0, "image {}", db(img));
        // 250 -> 48 kHz: 30 kHz would alias to -18 kHz.
        let x = ctone(250_000, 30_000.0 / 250_000.0, 1.0);
        let mut r = Resampler::<Complex32>::new(250_000.0, 48_000.0).unwrap();
        out.clear();
        r.process(&x, &mut out);
        assert!(db(camp(&out[4000..], -18_000.0 / 48_000.0)) < -80.0);
    }

    #[test]
    fn long_run_count_is_exact() {
        // The bookkeeping over ten hours, by the exact count function...
        for (fi, fo) in [
            (300_000.0, 44_100.0),
            (250_000.0, 48_000.0),
            (44_100.0, 48_000.0),
            (48_000.0, 48_033.6),
        ] {
            let r = FractionalResampler::<f32>::with_rates(fi, fo).unwrap();
            let n = (fi * 36_000.0) as u64;
            let got = r.outputs_for(n) as f64;
            let want = n as f64 * fo / fi;
            assert!((got - want).abs() <= 1.0, "{fi}->{fo}: {got} vs {want}");
        }
        // ...which the streaming path follows exactly, whatever the block sizes.
        let mut r = FractionalResampler::<f32>::with_rates(44_100.0, 48_000.0).unwrap();
        let want = r.outputs_for(441_000);
        let mut out = Vec::new();
        let x = vec![0.0f32; 441_000];
        let mut i = 0;
        let mut len = 1;
        while i < x.len() {
            let e = (i + len).min(x.len());
            r.process(&x[i..e], &mut out);
            i = e;
            len = len * 7 % 1013 + 1;
        }
        assert_eq!(out.len() as u64, want);
        assert_eq!(want, 480_000);
    }

    #[test]
    fn drift_correction_changes_the_rate_smoothly() {
        let mut r = Resampler::<f32>::new(48_000.0, 48_000.0).unwrap();
        assert!(!r.has_fractional_stage());
        r.set_correction(1.001);
        let mut out = Vec::new();
        r.process(&vec![0.0; 100_000], &mut out);
        assert!((out.len() as f64 - 100_000.0 / 1.001).abs() <= 1.0);
    }

    #[test]
    fn rational_resampler() {
        let mut r = RationalResampler::<Complex32>::new(160, 147).unwrap();
        assert_eq!(r.ratio(), (160, 147));
        let x = ctone(44_100, 1000.0 / 44_100.0, 1.0);
        let mut out = Vec::new();
        for c in x.chunks(333) {
            r.process(c, &mut out);
        }
        assert_eq!(out.len(), 48_000);
        assert!(db(camp(&out[4000..], 1000.0 / 48_000.0)).abs() < 0.02);
    }

    #[test]
    fn stereo_interleaved() {
        let mut r = Resampler::<Stereo>::new(44_100.0, 48_000.0).unwrap();
        let mut x = Vec::new();
        for i in 0..44_100 {
            let v = (std::f64::consts::TAU * 1000.0 * i as f64 / 44_100.0).sin() as f32;
            x.push(v);
            x.push(-0.5 * v);
        }
        let mut out = Vec::new();
        for c in x.chunks(882) {
            r.process_interleaved(c, &mut out);
        }
        assert!((out.len() as f64 - 96_000.0).abs() <= 2.0);
        let l: Vec<f32> = out.iter().step_by(2).copied().collect();
        let rr: Vec<f32> = out.iter().skip(1).step_by(2).copied().collect();
        assert!(db(ramp(&l[4000..], 1000.0 / 48_000.0)).abs() < 0.05);
        assert!(db(ramp(&rr[4000..], 1000.0 / 48_000.0) / 0.5).abs() < 0.05);
    }

    #[test]
    fn exact_ratios_of_four_build_and_keep_a_tone() {
        for (from, to) in [
            (1e6, 0.25e6),
            (2.4e6, 0.6e6),
            (4e6, 1e6),
            (10e6, 2.5e6),
            (192e3, 48e3),
        ] {
            let mut r = Resampler::<Complex32>::new(from, to).expect("an exact ratio of 4 builds");
            let f = 0.1 * to;
            let x = ctone(40_000, f / from, 0.5);
            let mut out = Vec::new();
            r.process(&x, &mut out);
            assert_eq!(out.len(), 10_000, "{from} -> {to}");
            let a = camp(&out[2000..], f / to);
            assert!(
                (db(a / 0.5)).abs() < 0.05,
                "{from} -> {to}: {}",
                db(a / 0.5)
            );
            // A tone just above the new Nyquist does not alias into the passband.
            r.reset();
            out.clear();
            r.process(&ctone(40_000, 0.6 * to / from, 0.5), &mut out);
            assert!(db(camp(&out[2000..], -0.4)) < -70.0, "{from} -> {to}");
        }
    }

    /// The exact value of a unit tone of `f` cycles per sample at position `t`.
    fn at(f: f64, t: f64) -> Complex32 {
        let p = std::f64::consts::TAU * (f * t).rem_euclid(1.0);
        Complex32::new(p.cos() as f32, p.sin() as f32)
    }

    fn max_err(out: &[Complex32], want: impl Fn(usize) -> Complex32) -> f32 {
        out.iter()
            .enumerate()
            .map(|(n, v)| (v - want(n)).norm())
            .fold(0.0, f32::max)
    }

    #[test]
    fn start_phase_places_the_first_output_exactly() {
        // 48 -> 44.1 kHz. Output n is the tone at input position start + n * 480 / 441.
        let f = 2_000.0 / 48_000.0;
        let x = ctone(20_000, f, 1.0);
        for start in [0.0, 0.37, 41.25, 1000.875] {
            let mut r = FractionalResampler::<Complex32>::with_rates(48_000.0, 44_100.0).unwrap();
            r.set_start(start).unwrap();
            let mut out = Vec::new();
            r.process(&x, &mut out);
            let step = 48_000.0 / 44_100.0;
            let skip = 200;
            let e = max_err(&out[skip..out.len() - 200], |n| {
                at(f, start + (n + skip) as f64 * step)
            });
            assert!(e < 1e-3, "start {start}: error {e}");
        }
        let mut r = FractionalResampler::<f32>::with_rates(48_000.0, 44_100.0).unwrap();
        let d = r.delay();
        assert!(r.set_start(-d).is_ok());
        assert!(r.set_start(-d - 0.01).is_err());
        assert!(r.set_start(f64::NAN).is_err());
    }

    #[test]
    fn shift_moves_a_tone_to_dc_with_its_phase() {
        // A tone at +100 kHz in 1 Msps, shifted by 100 kHz and taken at 250 ksps: 1 + 0j, because the mixer phase is zero at x[0].
        let x = ctone(40_000, 0.1, 1.0);
        let mut r = FractionalResampler::<Complex32>::new(1e6, 250e3, 10e3, 30e3, 70.0).unwrap();
        r.set_shift(100e3).unwrap();
        assert_eq!(r.shift(), 100e3);
        let mut out = Vec::new();
        r.set_start(0.0).unwrap();
        r.process(&x, &mut out);
        let e = max_err(&out[100..9_000], |_| Complex32::new(1.0, 0.0));
        assert!(e < 1e-3, "error {e}");
        // A tone 5 kHz above the shift comes out at +5 kHz with the phase of the input at that position.
        let x = ctone(40_000, 0.105, 1.0);
        r.reset();
        r.set_start(0.0).unwrap();
        out.clear();
        r.process(&x, &mut out);
        let e = max_err(&out[100..9_000], |n| at(0.005, (n + 100) as f64 * 4.0));
        assert!(e < 1e-3, "error {e}");
        assert!(r.set_shift(f64::INFINITY).is_err());
        r.set_shift(0.0).unwrap();
        assert_eq!(r.shift(), 0.0);
    }

    #[test]
    fn narrow_cut_off_rejects_out_of_band_tones() {
        // 10 Msps to 1.92 Msps with a 200 kHz wide channel: flat to 100 kHz, 70 dB down from 300 kHz.
        let (fi, fo) = (10e6, 1.92e6);
        let mut r = FractionalResampler::<Complex32>::new(fi, fo, 100e3, 300e3, 70.0).unwrap();
        let mut out = Vec::new();
        r.process(&ctone(200_000, 80e3 / fi, 1.0), &mut out);
        assert!(db(camp(&out[2_000..], 80e3 / fo)).abs() < 0.01);
        for f in [300e3, 450e3, -700e3, 1.5e6, 2.5e6] {
            r.reset();
            out.clear();
            r.process(&ctone(200_000, f / fi, 1.0), &mut out);
            let p = out[2_000..]
                .iter()
                .map(|v| v.norm_sqr() as f64)
                .sum::<f64>()
                / (out.len() - 2_000) as f64;
            assert!(10.0 * p.log10() < -70.0, "{f} Hz: {} dB", 10.0 * p.log10());
        }
    }

    #[test]
    fn slice_reads_give_exact_counts_positions_and_zeros() {
        let f = 0.1;
        let x = ctone(20_000, f, 1.0);
        let mut r = FractionalResampler::<Complex32>::new(1e6, 250e3, 10e3, 30e3, 70.0).unwrap();
        r.set_shift(97e3).unwrap();
        r.set_correction(1.0 + 20e-6);
        let step = 4.0 * (1.0 + 20e-6);
        let mut out = Vec::new();
        for start in [500.0, 777.3, 12_345.678] {
            r.process_slice(&x, start, 1_000, &mut out).unwrap();
            assert_eq!(out.len(), 1_000);
            let e = max_err(&out, |n| at(f - 0.097, start + n as f64 * step));
            assert!(e < 1e-3, "start {start}: error {e}");
        }
        // Wholly outside the slice: zeros. Half outside: the edge reads half the tone.
        r.process_slice(&x, -5_000.0, 100, &mut out).unwrap();
        assert!(out.len() == 100 && out.iter().all(|v| v.norm() == 0.0));
        r.process_slice(&x, 30_000.0, 100, &mut out).unwrap();
        assert!(out.len() == 100 && out.iter().all(|v| v.norm() == 0.0));
        r.set_shift(0.0).unwrap();
        r.set_correction(1.0);
        r.process_slice(
            &vec![Complex32::new(1.0, 0.0); 20_000],
            20_000.0 - 1.0 + 0.5,
            1,
            &mut out,
        )
        .unwrap();
        assert!((out[0].norm() - 0.5).abs() < 0.02, "{}", out[0].norm());
        assert!(r.process_slice(&x, f64::NAN, 10, &mut out).is_err());
        r.process_slice(&x, 0.0, 0, &mut out).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn resampler_delay_is_the_time_of_each_output() {
        for (fi, fo) in [
            (8e6, 2.0833333e6),
            (2.4e6, 2.0833333e6),
            (20e6, 2.0833333e6),
            (2.4e6, 48e3),
        ] {
            let f = 0.05 * fo / fi;
            let x = ctone(200_000, f, 1.0);
            let mut r = Resampler::<Complex32>::with_quality(fi, fo, 0.4, 60.0).unwrap();
            let mut out = Vec::new();
            r.process(&x, &mut out);
            let d = r.delay();
            let step = fi / fo;
            let skip = out.len() / 4;
            let e = max_err(&out[skip..out.len() - 50], |n| {
                at(f, (n + skip) as f64 * step - d)
            });
            assert!(e < 3e-3, "{fi} -> {fo}: error {e}");
        }
    }

    #[test]
    fn a_resampled_span_keeps_its_edges() {
        let x = vec![Complex32::new(1.0, 0.0); 2000];
        let mut y = Vec::new();
        resample_span(&x, 20e6, 15.36e6, &mut y);
        assert_eq!(y.len(), 1536);
        for (i, v) in y.iter().enumerate() {
            assert!((v.norm() - 1.0).abs() < 0.01, "sample {i}: {}", v.norm());
        }
    }

    #[test]
    fn a_kept_span_resampler_gives_each_span_what_a_new_one_gives() {
        let x: Vec<Complex32> = (0..30_000)
            .map(|i| Complex32::new((i as f32 * 0.013).sin(), (i as f32 * 0.007).cos()))
            .collect();
        for (fi, fo) in [(20e6, 15.36e6), (30.72e6, 15.36e6), (15.36e6, 15.36e6)] {
            let mut kept = SpanResampler::new(fi, fo).unwrap().with_room(30_000);
            let (mut a, mut b) = (Vec::new(), Vec::new());
            for (from, len) in [
                (0, 30_000),
                (17, 4_000),
                (5_000, 1),
                (9_999, 12_345),
                (3, 0),
            ] {
                let span = &x[from..from + len];
                kept.run(span, &mut a);
                resample_span(span, fi, fo, &mut b);
                assert_eq!(a, b, "{fi} -> {fo}, {len} samples at {from}");
                assert_eq!(a.len(), (len as f64 * fo / fi).floor() as usize);
            }
        }
        assert!(SpanResampler::new(20e6, 0.0).is_err());
    }
}
