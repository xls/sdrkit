//! Symbol timing recovery: a streaming synchroniser generic over its timing error detector and its interpolator, feed-forward
//! estimators for bursts, and a polyphase matched filter that doubles as the interpolator.
//!
//! # The streaming synchroniser
//!
//! [`SymbolSync`] reads its input at fractional instants through any `sdrkit_algo_dsp::interp::Interpolator` (linear, cubic, Catmull-Rom,
//! a windowed-sinc polyphase bank, or [`MatchedInterp`], which filters and interpolates in one dot product at the output instant).
//! It evaluates the signal twice per symbol, at the symbol instant and half way between, so every detector has what it needs, and
//! steers the instants with a second-order loop built by `LoopFilter` from a noise bandwidth `bn_t` (per symbol) and a damping.
//!
//! | Detector | Error for the symbol between `prev` and `cur` (`mid` half way) | Needs |
//! |---|---|---|
//! | [`Gardner`] | `Re{(prev - cur) mid*}` | 2 samples per symbol; blind; carrier phase not needed |
//! | [`ZeroCrossing`] | `Re{(sgn prev - sgn cur) mid*} / 2` (Gardner with decisions; only transitions count) | binary or QPSK signals, discriminator output |
//! | [`EarlyLate`] | `Re{prev* (late - early)}` (the slope of the eye energy at the symbol: the tracking form of max-energy) | any |
//! | [`MuellerMuller`] | `Re{cur sgn(prev)* - prev sgn(cur)*}` | 1 sample per symbol decisions; carrier locked |
//! | [`MuellerMullerDd`] | the same with decisions from any [`Constellation`] | QAM, APSK |
//!
//! The detector's gain (its S-curve slope) depends on the pulse, so [`SymbolSync::new`] measures it from the overall pulse given in
//! the configuration ([`detector_gain`]); the loop then has the requested bandwidth. An AGC holds the symbols at unit mean power, so
//! the gain does not depend on the signal level, and the outputs come out at unit power.
//!
//! The clock range is a setting (`max_ppm`: the integrator, which holds the clock error, is clamped there): a large clock error is
//! tracked if allowed. **OQPSK** (`oqpsk = true`): Q is read half a symbol after I, both
//! halves feed the detector, and each output joins I of one instant with Q of the next half. **Two outputs per symbol** produces the
//! T/2 stream a fractionally spaced equaliser requires (mid-point, then symbol). The lock test compares the energy at the symbol
//! instants with the energy half way between, which is what an open eye means for any pulse. For OQPSK that energy is the same at
//! both instants, so the test compares their squares instead, which needs no carrier. [`SymbolSync::set_carrier_phase`] and
//! [`SymbolSync::process_each`] take a carrier loop's phase off before the detector (the OQPSK detector needs it), and
//! [`SymbolSync::shift_half_symbol`] moves the instants by half a symbol.
//!
//! Every instant is computed from the loop state, never accumulated from an earlier output, and the input is kept in a small ring,
//! so a signal in one piece or in pieces of any length produces the same output.
//!
//! [`RunSymbolSync`] is the Gardner loop for high symbol rates: it steers the clock once per run of symbols, so the
//! interpolations of a run do not wait for each other. [`MatchedBank`] is the matched filter for it: the nearest row of the
//! polyphase filter, with no blend.
//!
//! # Bursts
//!
//! [`spectral_line`] (the Oerder-Meyr estimator: the phase of the symbol-rate line of `|x|^2`) and [`max_energy`] (the instant of the
//! widest eye over trial phases) estimate the timing of a stored burst without a loop; `sdrkit_algo_dsp::interp::Reader` then reads the
//! symbols at `start + k sps (1 + ppm)` with no copy. [`SymbolSync::run`] runs the tracking loop over a stored burst from a given
//! first instant.
//!
//! ```
//! use sdrkit_algo_dsp::interp::Cubic;
//! use sdrkit_algo_dsp::pulse::Shape;
//! use sdrkit_algo_modem::timing::{Gardner, SymbolSync, SymbolSyncConfig};
//! use sdrkit_algo_modem::Complex32;
//! // A BPSK signal with a raised-cosine pulse at 4 samples per symbol, started at a quarter-symbol offset.
//! let bits: Vec<f32> = (0..400).map(|k| if (k * 7 + k / 3) % 5 < 2 { 1.0 } else { -1.0 }).collect();
//! let x: Vec<Complex32> = (0..1600)
//!     .map(|n| {
//!         let t = n as f64 / 4.0 - 0.25;
//!         let v: f64 = (0..400).map(|k| bits[k] as f64 * Shape::Rc { beta: 0.5 }.value(t - k as f64)).sum();
//!         Complex32::new(v as f32, 0.0)
//!     })
//!     .collect();
//! let mut cfg = SymbolSyncConfig::new(4.0);
//! cfg.pulse = Shape::Rc { beta: 0.5 };
//! cfg.bn_t = 0.02;
//! let mut sync = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
//! let mut out = Vec::new();
//! sync.process(&x, &mut out);
//! // Once locked, every symbol is close to +-1 (unit power).
//! assert!(out[300..390].iter().all(|z| (z.re.abs() - 1.0).abs() < 0.1));
//! ```

use crate::lock::{rare, Gears, LockDetector};
use crate::{Complex32, Constellation, ModemError, Param};
use sdrkit_algo_dsp::interp::{CatmullRom, Cubic, Interpolator, Linear, Polyphase, Reader, MAX_TAPS};
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::{approx, Sample};

/// A value a timing loop can work on: `f32` (a real signal, a discriminator output) or [`Complex32`].
pub trait Symbol: Sample {
    /// A value from its parts (`f32` keeps `re`).
    fn from_parts(re: f32, im: f32) -> Self;
    /// `Re{self conj(other)}`.
    fn re_dot(self, other: Self) -> f32;
    /// `|self|^2`.
    fn energy(self) -> f32;
    /// The sign of each part (`+-1`, or `+-1 +-1j`); 0 stays 0.
    fn sign(self) -> Self;
    /// The in-phase part alone.
    fn i_part(self) -> Self;
    /// The quadrature part alone (0 for `f32`).
    fn q_part(self) -> Self;
    /// The square `self * self` (for [`Complex32`] the complex square).
    fn square(self) -> Self {
        let re = self.i_part().re_dot(Self::from_parts(1.0, 0.0));
        let im = self.q_part().re_dot(Self::from_parts(0.0, 1.0));
        Self::from_parts(re * re - im * im, 2.0 * re * im)
    }
    /// The value turned by the unit phasor `c` (for [`Complex32`] the product `self * c`). A real value is not turned.
    fn rotate(self, c: Complex32) -> Self {
        let _ = c;
        self
    }
}

#[inline]
fn sgn(x: f32) -> f32 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

impl Symbol for f32 {
    #[inline]
    fn from_parts(re: f32, _im: f32) -> Self {
        re
    }
    #[inline]
    fn re_dot(self, other: Self) -> f32 {
        self * other
    }
    #[inline]
    fn energy(self) -> f32 {
        self * self
    }
    #[inline]
    fn sign(self) -> Self {
        sgn(self)
    }
    #[inline]
    fn i_part(self) -> Self {
        self
    }
    #[inline]
    fn q_part(self) -> Self {
        0.0
    }
    #[inline]
    fn square(self) -> Self {
        self * self
    }
}

impl Symbol for Complex32 {
    #[inline]
    fn from_parts(re: f32, im: f32) -> Self {
        Complex32::new(re, im)
    }
    #[inline]
    fn re_dot(self, other: Self) -> f32 {
        self.re * other.re + self.im * other.im
    }
    #[inline]
    fn energy(self) -> f32 {
        self.norm_sqr()
    }
    #[inline]
    fn sign(self) -> Self {
        Complex32::new(sgn(self.re), sgn(self.im))
    }
    #[inline]
    fn i_part(self) -> Self {
        Complex32::new(self.re, 0.0)
    }
    #[inline]
    fn q_part(self) -> Self {
        Complex32::new(0.0, self.im)
    }
    #[inline]
    fn square(self) -> Self {
        self * self
    }
    #[inline]
    fn rotate(self, c: Complex32) -> Self {
        self * c
    }
}

/// A timing error detector: called once per symbol with the previous symbol sample, the sample half way, and the current symbol
/// sample (all at unit mean power). A sampling instant late by a small `tau` produces a negative mean error.
pub trait TimingDetector<T: Symbol>: Clone {
    /// The error for the symbol interval ending at `cur`.
    fn error(&mut self, prev: T, mid: T, cur: T) -> f32;
    /// Forgets any state.
    fn reset(&mut self) {}
}

/// The Gardner detector `Re{(prev - cur) mid*}` (F. M. Gardner, "A BPSK/QPSK timing-error detector for sampled receivers", IEEE
/// Trans. Commun. 34(5), 1986).
#[derive(Clone, Copy, Debug, Default)]
pub struct Gardner;

impl<T: Symbol> TimingDetector<T> for Gardner {
    #[inline]
    fn error(&mut self, prev: T, mid: T, cur: T) -> f32 {
        (prev - cur).re_dot(mid)
    }
}

/// Gardner with hard decisions: `Re{(sgn prev - sgn cur) mid*} / 2`, non-zero only across a transition (the zero-crossing bit clock
/// of FSK and AFSK receivers, applied to discriminator output or to a binary or QPSK signal).
#[derive(Clone, Copy, Debug, Default)]
pub struct ZeroCrossing;

impl<T: Symbol> TimingDetector<T> for ZeroCrossing {
    #[inline]
    fn error(&mut self, prev: T, mid: T, cur: T) -> f32 {
        0.5 * (prev.sign() - cur.sign()).re_dot(mid)
    }
}

/// The early-late detector `Re{prev* (late - early)}`, with `early` and `late` the half-symbol samples before and after the symbol
/// `prev`: the slope of `|y(t)|^2` at the symbol instant, so it climbs the eye's energy (the tracking form of [`max_energy`]). It
/// needs no decisions; a carrier offset small against the symbol rate does not matter.
#[derive(Clone, Copy, Debug, Default)]
pub struct EarlyLate<T> {
    early: T,
}

impl<T: Symbol> TimingDetector<T> for EarlyLate<T> {
    #[inline]
    fn error(&mut self, prev: T, mid: T, _cur: T) -> f32 {
        // `mid` is the late sample of `prev`; the previous call's `mid` was its early sample.
        let e = (mid - self.early).re_dot(prev);
        self.early = mid;
        e
    }
    fn reset(&mut self) {
        self.early = T::default();
    }
}

/// The Mueller and Muller detector with sign decisions, `Re{cur sgn(prev)* - prev sgn(cur)*}` (K. H. Mueller and M. Muller, "Timing
/// recovery in digital synchronous data receivers", IEEE Trans. Commun. 24(5), 1976). Needs the carrier locked.
#[derive(Clone, Copy, Debug, Default)]
pub struct MuellerMuller;

impl<T: Symbol> TimingDetector<T> for MuellerMuller {
    #[inline]
    fn error(&mut self, prev: T, _mid: T, cur: T) -> f32 {
        cur.re_dot(prev.sign()) - prev.re_dot(cur.sign())
    }
}

/// Mueller and Muller with decisions from a constellation (QAM, APSK, any map).
#[derive(Clone, Debug)]
pub struct MuellerMullerDd {
    c: Constellation,
}

impl MuellerMullerDd {
    /// A detector deciding on `c` (whose points have unit mean energy, as the synchroniser's AGC outputs).
    pub fn new(c: Constellation) -> Self {
        MuellerMullerDd { c }
    }
}

impl TimingDetector<Complex32> for MuellerMullerDd {
    #[inline]
    fn error(&mut self, prev: Complex32, _mid: Complex32, cur: Complex32) -> f32 {
        let (dp, dc) = (self.c.nearest(prev), self.c.nearest(cur));
        cur.re_dot(dp) - prev.re_dot(dc)
    }
}

/// A polyphase matched filter that is also the interpolator: `PHASES + 1` rows of `TAPS` taps of `shape` at `sps` samples per
/// symbol, one row per fractional delay, adjacent rows blended; one dot product computes the filtered signal at the output instant
/// (the cheapest form at high sps: `TAPS` multiply-adds per output, none per input sample). `TAPS` should cover the pulse's span
/// times `sps` (at most `sdrkit_algo_dsp::interp::MAX_TAPS`).
#[derive(Clone, Debug)]
pub struct MatchedInterp<const TAPS: usize, const PHASES: usize> {
    /// For each of the `PHASES` delays: its row, then the difference of the next row to it.
    table: Vec<f32>,
}

/// The rows of a polyphase matched filter: `PHASES + 1` rows of `TAPS` taps of `shape` at `sps` samples per symbol, row `p` for
/// the fractional delay `p / PHASES`, scaled to unit energy. Errors for a bad shape or `sps`, `TAPS` odd, below 2 or above
/// `MAX_TAPS`, or `PHASES` 0.
fn matched_rows<const TAPS: usize, const PHASES: usize>(
    shape: Shape,
    sps: f64,
) -> Result<Vec<f32>, ModemError> {
    shape.check()?;
    Param::Sps.check_positive(sps)?;
    if TAPS < 2 || !TAPS.is_multiple_of(2) || TAPS > MAX_TAPS {
        return Param::Taps.not_allowed(TAPS as f64);
    }
    if PHASES == 0 {
        return Param::Phases.at_least(1.0, 0.0);
    }
    let mut table = vec![0f32; (PHASES + 1) * TAPS];
    let mut energy = 0.0f64;
    for p in 0..=PHASES {
        let mu = p as f64 / PHASES as f64;
        for k in 0..TAPS {
            let d = k as f64 - (TAPS / 2 - 1) as f64 - mu;
            let v = shape.value(d / sps);
            table[p * TAPS + k] = v as f32;
            if p == 0 {
                energy += v * v;
            }
        }
    }
    if energy <= 0.0 {
        return Param::Pulse.degenerate();
    }
    let g = (1.0 / energy.sqrt()) as f32;
    table.iter_mut().for_each(|t| *t *= g);
    Ok(table)
}

impl<const TAPS: usize, const PHASES: usize> MatchedInterp<TAPS, PHASES> {
    /// The filter of `shape` at `sps` samples per symbol, scaled to unit energy. Errors for a bad shape or `sps`, `TAPS` odd, below 2
    /// or above `MAX_TAPS`, or `PHASES` 0.
    pub fn new(shape: Shape, sps: f64) -> Result<Self, ModemError> {
        let table = matched_rows::<TAPS, PHASES>(shape, sps)?;
        let mut steps = vec![0f32; PHASES * 2 * TAPS];
        for (p, s) in steps.chunks_exact_mut(2 * TAPS).enumerate() {
            let (a, b) = table[p * TAPS..(p + 2) * TAPS].split_at(TAPS);
            for k in 0..TAPS {
                s[k] = a[k];
                s[TAPS + k] = b[k] - a[k];
            }
        }
        Ok(MatchedInterp { table: steps })
    }
}

/// The polyphase matched filter of [`MatchedInterp`] read at the nearest of its `PHASES` fractional delays, with no blend
/// between two rows: the cheapest form at a high symbol rate. A row keeps each tap once per value of a two-lane sample, so
/// the dot product of a complex signal is one plain loop over its values. The instant is off by at most `1 / (2 PHASES)` of
/// a sample, which at 128 delays leaves an error about 50 dB under the signal.
#[derive(Clone, Debug)]
pub struct MatchedBank<const TAPS: usize, const PHASES: usize> {
    /// `PHASES + 1` rows of `TAPS` taps.
    rows: Vec<f32>,
    /// The same rows with each tap twice.
    pairs: Vec<f32>,
}

impl<const TAPS: usize, const PHASES: usize> MatchedBank<TAPS, PHASES> {
    /// The filter of `shape` at `sps` samples per symbol, scaled to unit energy. Errors as [`MatchedInterp::new`].
    pub fn new(shape: Shape, sps: f64) -> Result<Self, ModemError> {
        let rows = matched_rows::<TAPS, PHASES>(shape, sps)?;
        let pairs = rows.iter().flat_map(|&t| [t, t]).collect();
        Ok(MatchedBank { rows, pairs })
    }
}

impl<const TAPS: usize, const PHASES: usize> Interpolator for MatchedBank<TAPS, PHASES> {
    const TAPS: usize = TAPS;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        self.read(h, self.place(mu))
    }
}

/// An interpolator that a [`RunSymbolSync`] reads a run with, in two steps: the place of every instant of the run first,
/// then the windows. A read then waits only for its window, not for the arithmetic of its instant, so the reads of a
/// run are in work together.
pub trait RunInterpolator: Interpolator {
    /// What a read needs of the fraction of an instant.
    type Place: Copy + Default + std::fmt::Debug;
    /// The place of the fraction `mu` (0 to 1).
    fn place(&self, mu: f32) -> Self::Place;
    /// [`Interpolator::at`] for the fraction of `place`.
    fn read<T: Sample>(&self, hist: &[T], place: Self::Place) -> T;
    /// Appends to `out` the [`read`](Self::read) of each instant of `places`: the first sample of its window in `held`,
    /// and its place. Panics if a window does not fit in `held`.
    #[inline]
    fn read_run<T: Sample>(&self, held: &[T], places: &[(u32, Self::Place)], out: &mut Vec<T>) {
        out.extend(places.iter().map(|&(from, place)| {
            let from = from as usize;
            self.read(&held[from..from + Self::TAPS], place)
        }));
    }
}

impl<const TAPS: usize, const PHASES: usize> RunInterpolator for MatchedBank<TAPS, PHASES> {
    /// The row of the bank.
    type Place = u32;
    #[inline]
    fn place(&self, mu: f32) -> u32 {
        // The conversion takes a value below 0 or not a number as 0, so this is the row of `mu` limited to 0 to 1.
        ((mu * PHASES as f32 + 0.5) as u32).min(PHASES as u32)
    }
    #[inline]
    fn read<T: Sample>(&self, h: &[T], place: u32) -> T {
        let p = (place as usize).min(PHASES);
        if h.len() < TAPS {
            return T::default();
        }
        if T::LANES == 2 {
            T::dot_lanes(&self.pairs[p * 2 * TAPS..(p + 1) * 2 * TAPS], &h[..TAPS])
        } else {
            T::dot(&self.rows[p * TAPS..(p + 1) * TAPS], &h[..TAPS])
        }
    }
    /// The reads of [`read`](RunInterpolator::read), with the rows of the bank checked once for the whole run.
    #[inline]
    fn read_run<T: Sample>(&self, held: &[T], places: &[(u32, u32)], out: &mut Vec<T>) {
        if T::LANES == 2 {
            let pairs = &self.pairs[..(PHASES + 1) * 2 * TAPS];
            out.extend(places.iter().map(|&(from, place)| {
                let (from, p) = (from as usize, (place as usize).min(PHASES));
                T::dot_lanes(
                    &pairs[p * 2 * TAPS..(p + 1) * 2 * TAPS],
                    &held[from..from + TAPS],
                )
            }));
        } else {
            let rows = &self.rows[..(PHASES + 1) * TAPS];
            out.extend(places.iter().map(|&(from, place)| {
                let (from, p) = (from as usize, (place as usize).min(PHASES));
                T::dot(&rows[p * TAPS..(p + 1) * TAPS], &held[from..from + TAPS])
            }));
        }
    }
}

/// The interpolators that work from the fraction itself.
macro_rules! place_is_fraction {
    ($([$($g:tt)*] $t:ty),* $(,)?) => {$(
        impl<$($g)*> RunInterpolator for $t {
            type Place = f32;
            #[inline]
            fn place(&self, mu: f32) -> f32 {
                mu
            }
            #[inline]
            fn read<T: Sample>(&self, hist: &[T], place: f32) -> T {
                self.at(hist, place)
            }
        }
    )*};
}

place_is_fraction!(
    [] Linear,
    [] Cubic,
    [] CatmullRom,
    [const TAPS: usize, const PHASES: usize] Polyphase<TAPS, PHASES>,
    [const TAPS: usize, const PHASES: usize] MatchedInterp<TAPS, PHASES>,
);

impl<const TAPS: usize, const PHASES: usize> Interpolator for MatchedInterp<TAPS, PHASES> {
    const TAPS: usize = TAPS;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        if h.len() < TAPS {
            return T::default();
        }
        let pf = mu.clamp(0.0, 1.0) * PHASES as f32;
        let p = (pf as usize).min(PHASES - 1);
        let f = pf - p as f32;
        let (a, d) = self.table[p * 2 * TAPS..(p + 1) * 2 * TAPS].split_at(TAPS);
        T::dot_stepped(a, d, f, &h[..TAPS])
    }
}

/// Settings of a [`SymbolSync`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolSyncConfig {
    /// Input samples per symbol (any value above 1; fractional allowed).
    pub sps: f64,
    /// Loop noise bandwidth per symbol (0 < bn_t < 0.5; 0.005 to 0.02 is usual).
    pub bn_t: f64,
    /// Loop damping.
    pub zeta: f64,
    /// The largest clock error the loop may pull to, in ppm of the symbol period.
    pub max_ppm: f64,
    /// Pull-in and tracking bandwidths, switched by the lock test (overrides `bn_t`).
    pub gears: Option<Gears>,
    /// Output the half-symbol samples too (mid-point, then symbol): the T/2 stream of a fractionally spaced equaliser.
    pub two_per_symbol: bool,
    /// Offset QPSK: Q is read half a symbol after I.
    pub oqpsk: bool,
    /// The overall pulse at the detector (transmit filter and matched filter together: `Rc` for an RRC link), used to measure the
    /// detector's gain and the eye's energy ratio at lock.
    pub pulse: Shape,
    /// The detector gain (S-curve slope per symbol), if known; `None` measures it from `pulse`.
    pub detector_gain: Option<f64>,
    /// Symbols the lock test averages over.
    pub lock_symbols: f32,
}

impl SymbolSyncConfig {
    /// Defaults for `sps`: `bn_t` 0.01, damping 1/sqrt(2), clock range +-2000 ppm, no gears, one output per symbol, an RC 0.35
    /// overall pulse, lock averaged over 200 symbols.
    pub fn new(sps: f64) -> Self {
        SymbolSyncConfig {
            sps,
            bn_t: 0.01,
            zeta: std::f64::consts::FRAC_1_SQRT_2,
            max_ppm: 2000.0,
            gears: None,
            two_per_symbol: false,
            oqpsk: false,
            pulse: Shape::Rc { beta: 0.35 },
            detector_gain: None,
            lock_symbols: 200.0,
        }
    }
}

/// The detector error for one symbol, with OQPSK handled: `pm` is the half-symbol sample before `prev`.
#[inline]
fn loop_error<T: Symbol, D: TimingDetector<T>>(
    d: &mut D,
    dq: &mut D,
    oqpsk: bool,
    pm: T,
    prev: T,
    mid: T,
    cur: T,
) -> f32 {
    if oqpsk {
        // I at the symbol instants; Q at the half instants, whose own "mid" is the I instant between them.
        d.error(prev.i_part(), mid.i_part(), cur.i_part())
            + dq.error(pm.q_part(), prev.q_part(), mid.q_part())
    } else {
        d.error(prev, mid, cur)
    }
}

/// The eye's energy ratio `(Es - Em) / (Es + Em)` of one symbol: the energy at the symbol instant against the energy half way.
#[inline]
fn eye<T: Symbol>(mid: T, cur: T) -> f32 {
    let (es, em) = (cur.energy(), mid.energy());
    (es - em) / (es + em + 1e-30)
}

/// The OQPSK lock value of one symbol, `(cur^2 - mid^2) / (|cur|^2 + |mid|^2)` (complex squares). For OQPSK the energy at the
/// symbol and half way is the same (Q peaks where I crosses), but the squares differ: its mean is `e^(2 j phi)` times the eye ratio
/// `(Es - Em) / (Es + Em)` for a carrier phase `phi`, with the sign of the eye flipped when I and Q swap roles (a quarter turn of
/// the carrier, or a lock half a symbol late).
#[inline]
fn oqpsk_eye<T: Symbol>(mid: T, cur: T) -> T {
    let n = cur.energy() + mid.energy();
    (cur.square() - mid.square()) * (1.0 / (n + 1e-30))
}

/// The lock metric of one symbol, kept with the state it needs.
///
/// For a plain signal it is the eye ratio of [`eye`]. For OQPSK it is the magnitude of the running mean of [`oqpsk_eye`] (one-pole,
/// factor `alpha`): it does not need the carrier and does not depend on which rail the lock found, and a slow carrier (much less
/// than `alpha / (4 pi)` cycles per symbol) costs nothing.
#[derive(Clone, Copy, Debug, Default)]
struct EyeMetric<T> {
    w: T,
    alpha: f32,
}

impl<T: Symbol> EyeMetric<T> {
    fn new(alpha: f32) -> Self {
        EyeMetric {
            w: T::default(),
            alpha,
        }
    }

    #[inline]
    fn update(&mut self, oqpsk: bool, mid: T, cur: T) -> f32 {
        if !oqpsk {
            return eye(mid, cur);
        }
        let v = oqpsk_eye(mid, cur);
        if v.energy().is_finite() {
            self.w = self.w + (v - self.w) * self.alpha;
        }
        self.w.energy().sqrt()
    }
}

/// A deterministic pseudo-random sign sequence for calibration.
fn prbs(n: usize) -> impl Iterator<Item = (f32, f32)> {
    let mut s = 0x2545_F491_4F6C_DD1Du64;
    (0..n).map(move |_| {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let a = if s & 1 == 0 { 1.0 } else { -1.0 };
        let b = if s & 2 == 0 { 1.0 } else { -1.0 };
        (a, b)
    })
}

/// The mean loop error at timing offset `tau` (symbols, positive = late) for random symbols through `pulse`, and the eye ratio.
fn s_curve<T: Symbol, D: TimingDetector<T>>(
    det: &D,
    pulse: Shape,
    oqpsk: bool,
    tau: f64,
) -> (f64, f64) {
    const SPAN: i64 = 8;
    const N: usize = 2048;
    let lanes = if T::LANES == 1 {
        1.0
    } else {
        std::f32::consts::FRAC_1_SQRT_2
    };
    let syms: Vec<(f32, f32)> = prbs(N + 2 * SPAN as usize + 4)
        .map(|(a, b)| (a * lanes, b * lanes))
        .collect();
    let qd = if oqpsk { 0.5 } else { 0.0 };
    // y at time t (symbols): sum over nearby symbols.
    let y = |t: f64| -> T {
        let c = t.round() as i64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for n in (c - SPAN).max(0)..=(c + SPAN).min(syms.len() as i64 - 1) {
            let (a, b) = syms[n as usize];
            re += a as f64 * pulse.value(t - n as f64);
            im += b as f64 * pulse.value(t - n as f64 - qd);
        }
        T::from_parts(re as f32, im as f32)
    };
    let (mut d, mut dq) = (det.clone(), det.clone());
    d.reset();
    dq.reset();
    let (mut sum, mut eyes) = (0.0f64, 0.0f64);
    let k0 = SPAN as usize + 2;
    let mut w = T::default();
    for k in k0..k0 + N {
        let t = k as f64 + tau;
        let e = loop_error(
            &mut d,
            &mut dq,
            oqpsk,
            y(t - 1.5),
            y(t - 1.0),
            y(t - 0.5),
            y(t),
        );
        sum += e as f64;
        if oqpsk {
            w += oqpsk_eye(y(t - 0.5), y(t));
        } else {
            eyes += eye(y(t - 0.5), y(t)) as f64;
        }
    }
    if oqpsk {
        eyes = w.energy().sqrt() as f64;
    }
    (sum / N as f64, eyes / N as f64)
}

/// The gain of `det` for symbols with the overall pulse `pulse`: minus the slope of its mean error against the timing offset in
/// symbols, at zero offset, for unit-power random symbols (BPSK for `f32`, QPSK for `Complex32`). Errors when the slope is not
/// positive (the detector cannot track this pulse).
pub fn detector_gain<T: Symbol, D: TimingDetector<T>>(
    det: &D,
    pulse: Shape,
    oqpsk: bool,
) -> Result<f64, ModemError> {
    pulse.check()?;
    let h = 0.02;
    let (sp, _) = s_curve::<T, D>(det, pulse, oqpsk, h);
    let (sm, _) = s_curve::<T, D>(det, pulse, oqpsk, -h);
    let g = -(sp - sm) / (2.0 * h);
    if g > 1e-6 && g.is_finite() {
        Ok(g)
    } else {
        Param::Pulse.degenerate()
    }
}

/// Ring of the newest input samples, written twice so a window of `TAPS` is always one slice.
#[derive(Clone, Debug)]
struct Ring<T> {
    buf: Vec<T>,
    len: usize,
    w: usize,
}

impl<T: Sample> Ring<T> {
    fn new(len: usize) -> Self {
        Ring {
            buf: vec![T::default(); 2 * len],
            len,
            w: len - 1,
        }
    }
    #[inline]
    fn push(&mut self, x: T) {
        self.w = if self.w + 1 == self.len {
            0
        } else {
            self.w + 1
        };
        self.buf[self.w] = x;
        self.buf[self.w + self.len] = x;
    }
    /// `taps` samples starting `back` samples before the newest (back >= taps - 1 ... 0 means the window ends at the newest).
    #[inline]
    fn window(&self, start_rel: i64, taps: usize) -> &[T] {
        let len = self.len as i64;
        let s = if (-len..=0).contains(&start_rel) {
            // The usual reach: one conditional subtraction, no division.
            let s = self.w + (len + start_rel) as usize;
            if s >= self.len {
                s - self.len
            } else {
                s
            }
        } else {
            (self.w as i64 + len + start_rel) as usize % self.len
        };
        &self.buf[s..s + taps]
    }
    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = T::default());
        self.w = self.len - 1;
    }
}

/// The streaming symbol synchroniser (module documentation).
#[derive(Clone, Debug)]
pub struct SymbolSync<T: Symbol, D: TimingDetector<T>, I: Interpolator> {
    det: D,
    det_q: D,
    interp: I,
    cfg: SymbolSyncConfig,
    lf: LoopFilter,
    gain: f64,
    lock: LockDetector,
    ring: Ring<T>,
    /// Position of the next half-symbol instant, in samples relative to the newest input sample.
    t: f64,
    next_is_symbol: bool,
    prev_mid: T,
    prev: T,
    mid: T,
    pending_i: T,
    have_prev: bool,
    power: f64,
    n_power: u64,
    /// `1 / sqrt(power)`, or 1 before any power was measured.
    level: f32,
    symbols: u64,
    last_error: f32,
    n_in: u64,
    last_instant: f64,
    metric: EyeMetric<T>,
    /// The carrier phasor removed before the detector (`e^(-j 2 pi phase)`).
    derot: Complex32,
    carrier_phase: f64,
}

impl<T: Symbol, D: TimingDetector<T>, I: Interpolator> SymbolSync<T, D, I> {
    /// A synchroniser with detector `det` and interpolator `interp`. Errors for `sps` not above 1, a loop bandwidth outside
    /// (0, 0.5), a negative clock range, or a detector that cannot track the configured pulse.
    pub fn new(det: D, interp: I, cfg: SymbolSyncConfig) -> Result<Self, ModemError> {
        if !(cfg.sps > 1.0 && cfg.sps.is_finite()) {
            return Param::Sps.at_least(1.0, cfg.sps);
        }
        if !(cfg.max_ppm >= 0.0 && cfg.max_ppm < 2e5) {
            return Param::MaxPpm.range(0.0, 2e5, cfg.max_ppm);
        }
        let gain = match cfg.detector_gain {
            Some(g) if g > 0.0 && g.is_finite() => g,
            Some(g) => return Param::Gain.positive(g),
            None => detector_gain::<T, D>(&det, cfg.pulse, cfg.oqpsk)?,
        };
        let (_, eye0) = s_curve::<T, D>(&det, cfg.pulse, cfg.oqpsk, 0.0);
        let eye0 = (eye0 as f32).max(0.02);
        let bn = cfg.gears.map_or(cfg.bn_t, |g| g.pull_in_bn_t);
        let mut lf = LoopFilter::with_gain(bn, cfg.zeta, gain)?;
        if let Some(g) = cfg.gears {
            LoopFilter::with_gain(g.track_bn_t, cfg.zeta, gain)?;
        }
        lf.set_clamp(cfg.max_ppm * 1e-6);
        let lock = LockDetector::new(cfg.lock_symbols.max(1.0), 0.5 * eye0, 0.25 * eye0)?;
        let mut s = SymbolSync {
            det_q: det.clone(),
            det,
            interp,
            cfg,
            lf,
            gain,
            lock,
            ring: Ring::new(I::TAPS + 4),
            t: 0.0,
            next_is_symbol: false,
            prev_mid: T::default(),
            prev: T::default(),
            mid: T::default(),
            pending_i: T::default(),
            have_prev: false,
            power: 0.0,
            n_power: 0,
            level: 1.0,
            symbols: 0,
            last_error: 0.0,
            n_in: 0,
            last_instant: 0.0,
            metric: EyeMetric::new(1.0 / cfg.lock_symbols.max(1.0)),
            derot: Complex32::new(1.0, 0.0),
            carrier_phase: 0.0,
        };
        s.reset_to(0.0);
        Ok(s)
    }

    /// Restarts with the first symbol instant at input sample `start` (fractional) of the next input.
    fn reset_to(&mut self, start: f64) {
        self.ring.clear();
        self.det.reset();
        self.det_q.reset();
        self.lf.reset();
        if let Some(g) = self.cfg.gears {
            // Back to the pull-in gear; the bandwidth was checked in `new`.
            let _ = self
                .lf
                .set_bandwidth(g.pull_in_bn_t, self.cfg.zeta, self.gain);
        }
        self.lock.reset();
        // The newest sample is at index -1; the first half instant is the mid-point before `start`.
        self.t = start - 0.5 * self.cfg.sps + 1.0;
        self.next_is_symbol = false;
        self.prev_mid = T::default();
        self.prev = T::default();
        self.mid = T::default();
        self.pending_i = T::default();
        self.have_prev = false;
        self.power = 0.0;
        self.n_power = 0;
        self.level = 1.0;
        self.symbols = 0;
        self.last_error = 0.0;
        self.n_in = 0;
        self.last_instant = 0.0;
        self.metric = EyeMetric::new(self.metric.alpha);
    }

    /// Forgets the signal: clock error 0, unlocked, the next input starts a new stream.
    pub fn reset(&mut self) {
        self.reset_to(0.0);
    }

    /// The measured detector gain the loop was designed with.
    pub fn detector_gain(&self) -> f64 {
        self.gain
    }

    /// The clock error the loop has found, in ppm of the symbol period (positive: symbols longer than `sps`).
    pub fn clock_ppm(&self) -> f64 {
        self.lf.integrator() * 1e6
    }

    /// Samples per symbol the loop runs at now.
    pub fn sps_now(&self) -> f64 {
        self.cfg.sps * (1.0 + self.lf.integrator())
    }

    /// The smoothed eye-energy ratio the lock test uses.
    pub fn lock_metric(&self) -> f32 {
        self.lock.metric()
    }

    /// Whether the lock test passes.
    pub fn locked(&self) -> bool {
        self.lock.locked()
    }

    /// Where the newest symbol was read: its position in input samples counted from the start of the stream (or of the burst given
    /// to [`run`](Self::run)).
    pub fn last_instant(&self) -> f64 {
        self.last_instant
    }

    /// The last detector error (normalised).
    pub fn last_error(&self) -> f32 {
        self.last_error
    }

    /// Symbols produced since the start.
    pub fn symbols(&self) -> u64 {
        self.symbols
    }

    /// The configuration.
    pub fn config(&self) -> &SymbolSyncConfig {
        &self.cfg
    }

    #[inline]
    fn read(&self) -> T {
        let fl = self.t.floor();
        let mu = (self.t - fl) as f32;
        let start = fl as i64 - (I::TAPS as i64 / 2 - 1);
        self.interp.at(self.ring.window(start, I::TAPS), mu)
    }

    #[inline]
    fn agc(&mut self, y: T) -> f32 {
        let pw = y.energy() as f64;
        if pw.is_finite() {
            if self.n_power < 256 {
                self.n_power += 1;
                self.power += (pw - self.power) / self.n_power as f64;
            } else {
                self.power += (pw - self.power) * (1.0 / 1024.0);
            }
        }
        self.level = if self.power > 1e-30 {
            (1.0 / self.power.sqrt()) as f32
        } else {
            1.0
        };
        self.level
    }

    /// Takes input samples and appends symbols (or mid-points and symbols) at unit mean power to `out`.
    pub fn process(&mut self, x: &[T], out: &mut Vec<T>) {
        self.process_each(x, |y| {
            out.push(y);
            None
        });
    }

    /// Sets the carrier phase, in cycles, that is taken off every sample before the timing detector and the lock test (a complex
    /// signal only, a real signal is not turned). The outputs stay in the frame of the input, so the carrier loop after the
    /// synchroniser sees the same symbols as without it. The offset QPSK detector needs this: with the carrier on, part of each
    /// rail falls on the other, half a symbol away, and the detector gain falls as `cos 2 phi`.
    pub fn set_carrier_phase(&mut self, cycles: f64) {
        if cycles.is_finite() {
            self.carrier_phase = cycles.rem_euclid(1.0);
            let (s, c) = approx::sin_cos(-std::f64::consts::TAU * self.carrier_phase);
            self.derot = Complex32::new(c, s);
        }
    }

    /// The carrier phase taken off before the detector, in cycles.
    pub fn carrier_phase(&self) -> f64 {
        self.carrier_phase
    }

    /// Moves the symbol instants by half a symbol: the next instant, due as a half-way point, becomes a symbol instant. The clock
    /// error and the gain are kept. The detector starts again. For OQPSK this is the weak-signal rule: a lock found half a symbol
    /// late (pairs of Q and the next I) is moved back, together with a quarter turn of the carrier (`set_carrier_phase`, or the
    /// carrier loop's own phase), so the pairs are (I, Q) again.
    pub fn shift_half_symbol(&mut self) {
        self.next_is_symbol = !self.next_is_symbol;
        self.det.reset();
        self.det_q.reset();
        self.have_prev = false;
        self.prev_mid = T::default();
        self.prev = T::default();
        self.mid = T::default();
        self.pending_i = T::default();
    }

    /// Takes input samples and calls `on_output` with each output (as [`process`](Self::process) appends them). When the call
    /// returns `Some(phase)`, the carrier phase removed before the detector becomes `phase` cycles from the next sample on (see
    /// [`set_carrier_phase`](Self::set_carrier_phase)): a carrier loop run on each symbol closes its loop through here.
    pub fn process_each(&mut self, x: &[T], mut on_output: impl FnMut(T) -> Option<f64>) {
        let half = 0.5 * self.cfg.sps;
        let limit = -((I::TAPS / 2) as f64);
        for &v in x {
            self.ring.push(if v.energy().is_finite() {
                v
            } else {
                T::default()
            });
            self.t -= 1.0;
            self.n_in += 1;
            while self.t <= limit {
                let raw = self.read().rotate(self.derot);
                let i_rate = self.lf.integrator();
                let mut emitted: [Option<T>; 2] = [None, None];
                if self.next_is_symbol {
                    let g = self.agc(raw);
                    let cur = raw * g;
                    // The stored samples carry the gain of their own time; at steady state it is the same.
                    let (pm, prev, mid) = (self.prev_mid, self.prev, self.mid);
                    let mut step = half * (1.0 + i_rate);
                    if self.have_prev {
                        let e = loop_error(
                            &mut self.det,
                            &mut self.det_q,
                            self.cfg.oqpsk,
                            pm,
                            prev,
                            mid,
                            cur,
                        );
                        let e = if e.is_finite() {
                            e.clamp(-1.0, 1.0)
                        } else {
                            0.0
                        };
                        self.last_error = e;
                        let v = self.lf.update(e as f64);
                        let prop = v - self.lf.integrator();
                        step = half * (1.0 + self.lf.integrator()) + self.cfg.sps * prop;
                        let was = self.lock.locked();
                        let m = self.metric.update(self.cfg.oqpsk, mid, cur);
                        let now = self.lock.update(m);
                        if was != now {
                            if let Some(gs) = self.cfg.gears {
                                let _ =
                                    self.lf
                                        .set_bandwidth(gs.bn_t(now), self.cfg.zeta, self.gain);
                            }
                        }
                    }
                    self.have_prev = true;
                    self.last_instant = (self.n_in - 1) as f64 + self.t;
                    if self.cfg.oqpsk {
                        self.pending_i = cur;
                    } else {
                        emitted[0] = Some(cur);
                        self.symbols += 1;
                    }
                    self.prev_mid = mid;
                    self.prev = cur;
                    self.t += step.max(0.25 * half);
                    self.next_is_symbol = false;
                } else {
                    let mid = raw * self.level;
                    self.mid = mid;
                    if self.cfg.oqpsk {
                        if self.have_prev {
                            if self.cfg.two_per_symbol {
                                emitted[0] = Some(self.prev_mid.i_part() + self.prev.q_part());
                            }
                            emitted[1] = Some(self.pending_i.i_part() + mid.q_part());
                            self.symbols += 1;
                        }
                    } else if self.cfg.two_per_symbol {
                        emitted[0] = Some(mid);
                    }
                    self.t += half * (1.0 + i_rate);
                    self.next_is_symbol = true;
                }
                // Back to the frame of the input. A carrier phase of 0 is the exact phasor 1, so it changes nothing.
                let back = self.derot.conj();
                for y in emitted.into_iter().flatten() {
                    if let Some(ph) = on_output(y.rotate(back)) {
                        self.set_carrier_phase(ph);
                    }
                }
            }
        }
    }

    /// The burst form: forgets the signal, then runs over the stored `x` with the first symbol instant at `start` (samples, from a
    /// feed-forward estimate such as [`spectral_line`]), appending the symbols to `out`.
    pub fn run(&mut self, x: &[T], start: f64, out: &mut Vec<T>) -> Result<(), ModemError> {
        if !start.is_finite() {
            return Err(ModemError::NotFinite {
                param: Param::Input,
            });
        }
        self.reset_to(start);
        self.process(x, out);
        Ok(())
    }
}

/// A symbol synchroniser for high symbol rates: the Gardner loop of [`SymbolSync`] with the clock steered once per run of
/// symbols.
///
/// In [`SymbolSync`] the instant of a symbol depends on the error of the symbol before it, so each symbol waits for the one
/// before: interpolation, gain, detector and loop filter in a line. Here the instants of a run of symbols are laid out from
/// the clock as it is at the start of the run, so their interpolations do not wait for each other. The sum of the detector
/// errors of the run goes through the same loop filter in one step (the integrator and the proportional part are those of
/// one step per symbol, as long as the integrator stays inside its range), and the proportional part moves the start of
/// the next run. The gain of a run is the one found up to the run before, so it is not in the line either. Until the lock
/// test holds a run is one symbol, which is the loop of [`SymbolSync`]; the run length applies once it holds, when the
/// loop is narrow. A run of `run` symbols delays the correction by up to `run` symbols, so `run` times the tracking
/// bandwidth should stay under about 0.1.
///
/// [`RunSymbolSync::hold`] keeps the loop in its tracking state for as long as a later stage follows the frames of the
/// signal.
///
/// A run is made when the input holds all of its samples, so the output is the same for a signal in one piece and in
/// pieces of any length. The outputs are symbols at unit mean power, one per symbol. Two outputs per symbol are not
/// offered.
///
/// Offset QPSK: the detector reads the I rail at the symbol instants and the Q rail half a symbol later, with the carrier
/// of [`RunSymbolSync::set_carrier`] taken off first. An output pairs the I of one instant with the Q of the next half-way
/// instant, so it leaves with the pair after its own. The lock test is that of [`SymbolSync`] for offset QPSK.
#[derive(Clone, Debug)]
pub struct RunSymbolSync<T: Symbol, I: RunInterpolator> {
    interp: I,
    cfg: SymbolSyncConfig,
    run: usize,
    lf: LoopFilter,
    gain: f64,
    lock: LockDetector,
    /// The input still in reach of a window.
    buf: Vec<T>,
    /// The place of `buf[0]` in the stream.
    base: u64,
    /// The next half-way instant: the sample at or before it (a place in the stream) and the fraction of a sample after
    /// that sample. A whole number and a small fraction, so the arithmetic does not depend on how far the stream has run
    /// or on where `buf` starts.
    at: u64,
    frac: f64,
    prev: T,
    have_prev: bool,
    power: f64,
    n_power: u64,
    level: f32,
    symbols: u64,
    /// The loop is held in its tracking state ([`RunSymbolSync::hold`]).
    held: bool,
    /// The interpolated samples of the run in work: half-way point, then symbol.
    raw: Vec<T>,
    /// The instants of the run in work: the first sample of the window in the samples held for the run, and the place
    /// of the fraction.
    places: Vec<(u32, I::Place)>,
    /// Offset QPSK: the half-way sample before `prev`, the lock value, and the carrier of the next output (cycles, and
    /// cycles per symbol).
    prev_mid: T,
    metric: EyeMetric<T>,
    carrier: (f64, f64),
    /// Offset QPSK: the phasor that took the carrier off `prev`, whose I rail waits for its Q rail.
    prev_rot: Complex32,
}

/// Input samples behind the next window that a [`RunSymbolSync`] keeps before it moves the rest of its buffer down.
const RUN_KEEP: usize = 1024;

/// The place of the next run in the input of a [`RunSymbolSync`].
struct RunSpan {
    /// Symbols of the run.
    n: usize,
    /// Half a symbol, samples.
    half: f64,
    /// The place in the buffer of the sample at or before the first instant.
    first: usize,
    /// The least length of the buffer that holds the window of the last instant.
    end: usize,
}

impl RunSpan {
    /// A run of `n` symbols at `sps (1 + rate)` samples per symbol whose first half-way instant lies `frac` of a sample
    /// after sample `first` of the buffer.
    #[inline(always)]
    fn new<I: Interpolator>(n: usize, sps: f64, rate: f64, first: u64, frac: f64) -> RunSpan {
        let half = 0.5 * sps * (1.0 + rate);
        // `first` is `reach` or more.
        let first = first as usize;
        // The window of the last instant of the run ends `TAPS / 2` samples after the sample before the instant.
        let last = frac + (2 * n - 1) as f64 * half;
        RunSpan {
            n,
            half,
            first,
            end: first + last as usize + I::TAPS / 2 + 1,
        }
    }

    /// Instant `j` of the run, whose first instant lies `frac` of a sample after its first sample: the first sample of
    /// its window counted from the window of the first instant, and the place of its fraction in `interp`.
    #[inline(always)]
    fn instant<I: RunInterpolator>(&self, interp: &I, frac: f64, j: usize) -> (u32, I::Place) {
        // `after` is 0 or more.
        let after = frac + j as f64 * self.half;
        let whole = after.floor();
        (whole as u32, interp.place((after - whole) as f32))
    }

    /// The start of the next run, as `at` and `frac` of a [`RunSymbolSync`], when the loop moves this one by `moved`
    /// symbols of `sps` samples.
    #[inline(always)]
    fn advance(&self, at: &mut u64, frac: &mut f64, sps: f64, moved: f64) {
        let step = 2.0 * self.n as f64 * self.half + sps * moved;
        let after = *frac + step.max(0.5 * self.n as f64 * self.half);
        let whole = after.floor();
        *at += whole as u64;
        *frac = after - whole;
    }
}

/// Adds the energy `pw` of a symbol to the mean `power` of `count` symbols: a plain mean over the first 256, then a
/// mean that forgets with a time constant of 1024 symbols. A value that is not finite is left out.
#[inline(always)]
fn mean_power(power: &mut f64, count: &mut u64, pw: f64) {
    if pw.is_finite() {
        if *count < 256 {
            *count += 1;
            *power += (pw - *power) / *count as f64;
        } else {
            *power += (pw - *power) * (1.0 / 1024.0);
        }
    }
}

/// The gain that brings symbols of mean power `power` to unit power; 1 for no power.
#[inline(always)]
fn level_of(power: f64) -> f32 {
    if power > 1e-30 {
        (1.0 / power.sqrt()) as f32
    } else {
        1.0
    }
}

impl<T: Symbol, I: RunInterpolator> RunSymbolSync<T, I> {
    /// A synchroniser with interpolator `interp` that steers the clock every `run` symbols once locked. Errors as
    /// [`SymbolSync::new`], and for `run` 0 or two outputs per symbol.
    pub fn new(interp: I, cfg: SymbolSyncConfig, run: usize) -> Result<Self, ModemError> {
        if run == 0 {
            return Param::Window.at_least(1.0, 0.0);
        }
        if cfg.two_per_symbol {
            return Param::Pulse.degenerate();
        }
        // The checks, the detector gain and the lock test are those of the per-symbol loop.
        let plain = SymbolSync::<T, Gardner, Cubic>::new(Gardner, Cubic, cfg)?;
        let mut s = RunSymbolSync {
            interp,
            cfg,
            run,
            lf: plain.lf,
            gain: plain.gain,
            lock: plain.lock,
            buf: Vec::new(),
            base: 0,
            at: 0,
            frac: 0.0,
            prev: T::default(),
            have_prev: false,
            power: 0.0,
            n_power: 0,
            level: 1.0,
            symbols: 0,
            held: false,
            raw: Vec::with_capacity(2 * run),
            places: Vec::with_capacity(2 * run),
            prev_mid: T::default(),
            metric: plain.metric,
            carrier: (0.0, 0.0),
            prev_rot: Complex32::new(1.0, 0.0),
        };
        s.reset();
        Ok(s)
    }

    /// Forgets the signal: clock error 0, unlocked and not held, the next input starts a new stream.
    pub fn reset(&mut self) {
        self.lf.reset();
        if let Some(g) = self.cfg.gears {
            // Back to the pull-in gear; the bandwidth was checked in `new`.
            let _ = self
                .lf
                .set_bandwidth(g.pull_in_bn_t, self.cfg.zeta, self.gain);
        }
        self.lock.reset();
        self.held = false;
        // Samples of value 0 before the stream, so that the first window is whole.
        self.buf.clear();
        self.buf.resize(I::TAPS / 2 - 1, T::default());
        self.base = 0;
        self.at = (I::TAPS / 2 - 1) as u64;
        self.frac = 0.0;
        self.prev = T::default();
        self.have_prev = false;
        self.power = 0.0;
        self.n_power = 0;
        self.level = 1.0;
        self.symbols = 0;
        self.prev_mid = T::default();
        self.metric = EyeMetric::new(self.metric.alpha);
        self.prev_rot = Complex32::new(1.0, 0.0);
    }

    /// [`reset`](Self::reset), with the symbol instants at sample `start` of the next input (fractional, 0 or more) and
    /// whole symbols after it. The first output is the first of these instants that has a half-way instant in the input
    /// before it. A `start` that is not finite or below 0 counts as 0.
    pub fn reset_to(&mut self, start: f64) {
        self.reset();
        let start = if start.is_finite() && start > 0.0 { start } else { 0.0 };
        // The first half-way instant: half a symbol before the first symbol instant that leaves it at 0 or later.
        let half = 0.5 * self.cfg.sps;
        let first = (start + half).rem_euclid(self.cfg.sps);
        self.at += first.floor() as u64;
        self.frac = first - first.floor();
    }

    /// The clock error the loop has found, in ppm of the symbol period (positive: symbols longer than `sps`).
    pub fn clock_ppm(&self) -> f64 {
        self.lf.integrator() * 1e6
    }

    /// Samples per symbol the loop runs at now.
    pub fn sps_now(&self) -> f64 {
        self.cfg.sps * (1.0 + self.lf.integrator())
    }

    /// Whether the lock test passes.
    pub fn locked(&self) -> bool {
        self.lock.locked()
    }

    /// Offset QPSK: the carrier that is taken off before the rails are split. `phase` is the carrier of the next output,
    /// cycles, and `per_symbol` its growth from one output to the next. The outputs stay in the frame of the input. Without
    /// offset QPSK it changes nothing.
    pub fn set_carrier(&mut self, phase: f64, per_symbol: f64) {
        if phase.is_finite() && per_symbol.is_finite() {
            self.carrier = (phase, per_symbol);
        }
    }

    /// The input samples that the next run still waits for; 0 when the input held holds the run.
    pub fn need(&self) -> usize {
        self.span().end.saturating_sub(self.buf.len())
    }

    /// Where the next run lies.
    #[inline]
    fn span(&self) -> RunSpan {
        let n = if self.tracking() { self.run } else { 1 };
        RunSpan::new::<I>(
            n,
            self.cfg.sps,
            self.lf.integrator(),
            self.at - self.base,
            self.frac,
        )
    }

    /// Offset QPSK: the outputs, the sum of the detector errors and the lock test for the run in `raw`. Returns the sum and
    /// whether a symbol had one before it.
    fn offset_run(&mut self, out: &mut Vec<T>) -> (f64, bool) {
        let level = self.level;
        let (phase, per_symbol) = self.carrier;
        // The first symbol instant of the run is the next output, or the one after it when an I rail waits.
        let lead = if self.have_prev { per_symbol } else { 0.0 };
        let (s, c) = approx::sin_cos(-std::f64::consts::TAU * (phase + lead));
        let mut rot = Complex32::new(c, s);
        let (s, c) = approx::sin_cos(-std::f64::consts::TAU * per_symbol);
        let step = Complex32::new(c, s);
        let (mut errors, mut any) = (0.0f64, false);
        for pair in self.raw.as_chunks::<2>().0 {
            let mid = (pair[0] * level).rotate(self.prev_rot);
            let cur = (pair[1] * level).rotate(rot);
            if self.have_prev {
                let e = Gardner.error(self.prev.i_part(), mid.i_part(), cur.i_part())
                    + Gardner.error(self.prev_mid.q_part(), self.prev.q_part(), mid.q_part());
                if e.is_finite() {
                    errors += e.clamp(-1.0, 1.0) as f64;
                }
                any = true;
                let m = self.metric.update(true, mid, cur);
                self.lock.update(m);
                out.push((self.prev.i_part() + mid.q_part()).rotate(self.prev_rot.conj()));
                self.symbols += 1;
            }
            self.have_prev = true;
            self.prev_mid = mid;
            self.prev = cur;
            self.prev_rot = rot;
            rot *= step;
        }
        (errors, any)
    }

    /// Holds the loop in its tracking state (narrow bandwidth, whole runs) whatever the lock test says, or with `false`
    /// lets the lock test decide again. A stage after the loop that follows frames knows better than the eye of the
    /// symbols whether the clock is right: the eye of a constellation with several rings stays near the levels of the lock
    /// test, and the loop would go back to its pull-in bandwidth again and again.
    pub fn hold(&mut self, on: bool) {
        let was = self.tracking();
        self.held = on;
        self.gear(was);
    }

    /// Whether the loop is in its tracking state: the lock test passes, or the loop is held.
    pub fn tracking(&self) -> bool {
        self.held || self.lock.locked()
    }

    /// Sets the bandwidth of the loop filter if the tracking state is not `was` any more.
    fn gear(&mut self, was: bool) {
        let now = self.tracking();
        if was != now {
            if let Some(gs) = self.cfg.gears {
                // Both bandwidths were checked in `new`.
                let _ = self
                    .lf
                    .set_bandwidth(gs.bn_t(now), self.cfg.zeta, self.gain);
            }
        }
    }

    /// Symbols produced since the start.
    pub fn symbols(&self) -> u64 {
        self.symbols
    }

    /// Takes input samples and appends symbols at unit mean power to `out`.
    pub fn process(&mut self, x: &[T], out: &mut Vec<T>) {
        self.take(x);
        loop {
            if (self.run == 1 || !self.tracking()) && !self.cfg.oqpsk && !self.single_runs(out) {
                break;
            }
            let span = self.span();
            if span.end > self.buf.len() {
                break;
            }
            self.make_run(span, out);
        }
        self.trim();
    }

    /// The runs of one symbol of [`make_run`](Self::make_run) without offset QPSK (while the loop searches, or for `run`
    /// 1), with the state of the loop in registers from symbol to symbol. Returns `true` when the next run is longer and
    /// `false` when the next run is not whole in the buffer.
    fn single_runs(&mut self, out: &mut Vec<T>) -> bool {
        let RunSymbolSync {
            interp,
            cfg,
            lf,
            gain,
            lock,
            held: hold,
            run,
            buf,
            base,
            at,
            frac,
            prev,
            have_prev,
            power,
            n_power,
            level,
            symbols,
            ..
        } = self;
        let reach = I::TAPS / 2 - 1;
        let (mut lf_now, mut lock_now) = (lf.clone(), lock.clone());
        let (mut at_now, mut frac_now, mut prev_now, mut have) = (*at, *frac, *prev, *have_prev);
        let (mut power_now, mut count, mut level_now) = (*power, *n_power, *level);
        let made = out.len();
        let longer = loop {
            let span = RunSpan::new::<I>(1, cfg.sps, lf_now.integrator(), at_now - *base, frac_now);
            if span.end > buf.len() {
                break false;
            }
            let held = &buf[span.first - reach..span.end];
            let read = |j| {
                let (from, place) = span.instant(&*interp, frac_now, j);
                let from = from as usize;
                interp.read(&held[from..from + I::TAPS], place)
            };
            let (raw_mid, raw_cur) = (read(0), read(1));
            let was = *hold || lock_now.locked();
            // The symbol, its error and the lock test, as in `plain_run`.
            let cur = raw_cur * level_now;
            let mut errors = 0.0f64;
            let any = have;
            if have {
                let mid = raw_mid * level_now;
                let e = Gardner.error(prev_now, mid, cur);
                if e.is_finite() {
                    errors += e.clamp(-1.0, 1.0) as f64;
                } else {
                    rare();
                }
                lock_now.update(eye(mid, cur));
            }
            prev_now = cur;
            have = true;
            out.push(cur);
            let mut moved = 0.0f64;
            if any {
                let v = lf_now.update(errors);
                moved = v - lf_now.integrator();
                // The gear, as in `gear`; both bandwidths were checked in `new`.
                let now = *hold || lock_now.locked();
                if now != was {
                    if let Some(gs) = cfg.gears {
                        let _ = lf_now.set_bandwidth(gs.bn_t(now), cfg.zeta, *gain);
                    }
                }
            }
            mean_power(&mut power_now, &mut count, raw_cur.energy() as f64);
            level_now = level_of(power_now);
            span.advance(&mut at_now, &mut frac_now, cfg.sps, moved);
            if *run != 1 && (*hold || lock_now.locked()) {
                break true;
            }
        };
        (*lf, *lock) = (lf_now, lock_now);
        (*at, *frac, *prev, *have_prev) = (at_now, frac_now, prev_now, have);
        (*power, *n_power, *level) = (power_now, count, level_now);
        *symbols += (out.len() - made) as u64;
        longer
    }

    /// Takes samples from the front of `x` until the next run is whole, then makes that run and appends its symbols to
    /// `out`; at most one run. Returns the samples taken and whether the run was made. The symbols are those of
    /// [`process`](Self::process) on the same samples; a caller that steers something from each run (a mixer, a carrier
    /// loop) gives the rest of `x` again.
    pub fn process_run(&mut self, x: &[T], out: &mut Vec<T>) -> (usize, bool) {
        let span = self.span();
        let taken = span.end.saturating_sub(self.buf.len()).min(x.len());
        self.take(&x[..taken]);
        if span.end > self.buf.len() {
            return (taken, false);
        }
        self.make_run(span, out);
        self.trim();
        (taken, true)
    }

    /// Appends input samples to the buffer; a sample that is not finite counts as 0.
    #[inline]
    fn take(&mut self, x: &[T]) {
        self.buf.extend(x.iter().map(|&v| {
            if v.energy().is_finite() {
                v
            } else {
                T::default()
            }
        }));
    }

    /// Drops the samples before the next window, once they are many: input in small pieces does not move the rest each
    /// time.
    #[inline]
    fn trim(&mut self) {
        let reach = I::TAPS / 2 - 1;
        let done = ((self.at - self.base) as usize - reach).min(self.buf.len());
        if done >= RUN_KEEP || done == self.buf.len() {
            self.buf.drain(..done);
            self.base += done as u64;
        }
    }

    /// Makes the run at `span`, whose samples the buffer holds: the symbols to `out`, one step of the loop.
    #[inline]
    fn make_run(&mut self, span: RunSpan, out: &mut Vec<T>) {
        let reach = I::TAPS / 2 - 1;
        // The samples of every window of the run.
        let held = &self.buf[span.first - reach..span.end];
        let (frac, interp) = (self.frac, &self.interp);
        self.places.clear();
        self.places
            .extend((0..2 * span.n).map(|j| span.instant(interp, frac, j)));
        self.raw.clear();
        interp.read_run(held, &self.places, &mut self.raw);
        // The symbols of the run and the sum of their errors, with the gain found up to the run before.
        let was = self.tracking();
        let (errors, any) = if self.cfg.oqpsk {
            self.offset_run(out)
        } else {
            self.plain_run(out)
        };
        // One step of the loop filter for the run: its proportional part moves the start of the next run.
        let mut moved = 0.0f64;
        if any {
            let v = self.lf.update(errors);
            moved = v - self.lf.integrator();
            self.gear(was);
        }
        // The gain for the next run. The mean stays in a register from symbol to symbol.
        let (mut power, mut n_power) = (self.power, self.n_power);
        for pair in self.raw.as_chunks::<2>().0 {
            mean_power(&mut power, &mut n_power, pair[1].energy() as f64);
        }
        (self.power, self.n_power) = (power, n_power);
        self.level = level_of(power);
        span.advance(&mut self.at, &mut self.frac, self.cfg.sps, moved);
    }

    /// Without offset QPSK: the outputs, the sum of the detector errors and the lock test for the run in `raw`. Returns
    /// the sum and whether a symbol had one before it. The state of a symbol stays in registers from symbol to symbol.
    #[inline]
    fn plain_run(&mut self, out: &mut Vec<T>) -> (f64, bool) {
        let level = self.level;
        let pairs = self.raw.as_chunks::<2>().0;
        let (mut errors, any) = (0.0f64, self.have_prev || pairs.len() > 1);
        let mut lock = self.lock.clone();
        let mut prev = self.prev;
        // The first symbol of a stream has none before it.
        let skip = usize::from(!self.have_prev).min(pairs.len());
        if let Some(pair) = pairs[..skip].first() {
            prev = pair[1] * level;
        }
        for pair in &pairs[skip..] {
            let (mid, cur) = (pair[0] * level, pair[1] * level);
            let e = Gardner.error(prev, mid, cur);
            if e.is_finite() {
                errors += e.clamp(-1.0, 1.0) as f64;
            } else {
                rare();
            }
            lock.update(eye(mid, cur));
            prev = cur;
        }
        self.lock = lock;
        self.prev = prev;
        self.have_prev |= !pairs.is_empty();
        out.extend(pairs.iter().map(|pair| pair[1] * level));
        self.symbols += pairs.len() as u64;
        (errors, any)
    }
}

/// A bit clock for hard-limited levels (a digital phase-locked loop of the kind packet radio TNCs use). The phase advances one bit
/// period per bit. At each change of level the phase is multiplied by the inertia (between 0 and 1), which pulls it towards the
/// edge (1 never moves). A bit is sampled half a bit after the edges, at the centre of the bit. The inertia is
/// `search` until [`EdgeClock::hold`] requests `locked` over a number of bits (after a flag or a good frame).
///
/// ```
/// use sdrkit_algo_modem::timing::EdgeClock;
/// // 1200 baud at 48 kHz (40 samples per bit), and the bits 1, 0, 0, 1, 1, 1, 0.
/// let bits = [1u8, 0, 0, 1, 1, 1, 0];
/// let mut c = EdgeClock::new(1200.0, 48_000.0, 0.5, 0.8).unwrap();
/// let got: Vec<u8> = bits.iter().flat_map(|&b| std::iter::repeat_n(b, 40)).filter_map(|l| c.sample(l)).collect();
/// assert_eq!(got, bits);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct EdgeClock {
    phase: f32,
    step: f32,
    search: f32,
    locked: f32,
    lock_bits: u32,
    last: u8,
}

impl EdgeClock {
    /// A clock for `baud` at `rate` samples per second, with inertia `search` before a frame and `locked` for the bits given to
    /// [`EdgeClock::hold`]. Errors unless `0 < baud < rate / 2` and both inertias are in `(0, 1]`.
    pub fn new(baud: f64, rate: f64, search: f32, locked: f32) -> Result<EdgeClock, ModemError> {
        let ok = |k: f32| k > 0.0 && k <= 1.0;
        Param::Baud.check_positive(baud)?;
        if !(rate.is_finite() && rate > 2.0 * baud) {
            return Param::Rate.at_least(2.0 * baud, rate);
        }
        if !ok(search) {
            return Param::Inertia.range(0.0, 1.0, search as f64);
        }
        if !ok(locked) {
            return Param::Inertia.range(0.0, 1.0, locked as f64);
        }
        Ok(EdgeClock {
            phase: 0.0,
            step: (baud / rate) as f32,
            search,
            locked,
            lock_bits: 0,
            last: 0,
        })
    }

    /// Uses the locked inertia for the next `bits` bits.
    pub fn hold(&mut self, bits: u32) {
        self.lock_bits = bits;
    }

    /// Bits left at the locked inertia.
    pub fn held(&self) -> u32 {
        self.lock_bits
    }

    /// The phase within the bit, in bits (`-0.5` just after a bit is sampled, `0` at an edge in step).
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Back to phase 0, level 0, the search inertia.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.lock_bits = 0;
        self.last = 0;
    }

    /// Takes one level (0 or 1, and any other value counts as 1). Returns the level at the centre of a bit, once per bit.
    #[inline]
    pub fn sample(&mut self, level: u8) -> Option<u8> {
        let level = u8::from(level != 0);
        if level != self.last {
            let k = if self.lock_bits > 0 {
                self.locked
            } else {
                self.search
            };
            self.phase *= k;
            self.last = level;
        }
        self.phase += self.step;
        if self.phase >= 0.5 {
            self.phase -= 1.0;
            self.lock_bits = self.lock_bits.saturating_sub(1);
            return Some(level);
        }
        None
    }
}

/// The Oerder-Meyr estimate of a burst's timing: the phase of the symbol-rate line of `|x|^2` (M. Oerder and H. Meyr, "Digital filter
/// and square timing recovery", IEEE Trans. Commun. 36(5), 1988). Returns the first symbol instant in `[0, sps)` samples. `x` should
/// be matched-filtered. Errors for `sps` not above 2, fewer than 4 symbols, or no energy.
pub fn spectral_line<T: Symbol>(x: &[T], sps: f64) -> Result<f64, ModemError> {
    if !(sps > 2.0 && sps.is_finite()) {
        return Param::Sps.at_least(2.0, sps);
    }
    if (x.len() as f64) < 4.0 * sps {
        return Err(ModemError::InputTooShort {
            need: (4.0 * sps).ceil() as usize,
        });
    }
    let mut rot = PhaseAcc::new(-1.0 / sps);
    let (mut re, mut im) = (0.0f64, 0.0f64);
    for &v in x {
        let p = rot.next_phasor();
        let e = v.energy();
        if e.is_finite() {
            re += (e * p.re) as f64;
            im += (e * p.im) as f64;
        }
    }
    if re == 0.0 && im == 0.0 {
        return Param::Input.degenerate();
    }
    let a = approx::atan2(im as f32, re as f32) as f64;
    Ok((-a / std::f64::consts::TAU * sps).rem_euclid(sps))
}

/// The instant of the widest eye: the energy of `x` read every `sps` samples from each of `phases` trial offsets in `[0, sps)`, the
/// best refined by a parabola through its circular neighbours. Returns the first symbol instant in `[0, sps)`. Works at any
/// `sps` above 1 and on short bursts. Errors for fewer than 3 phases, `sps` not above 1, or a burst shorter than 2 symbols.
pub fn max_energy<T: Symbol>(x: &[T], sps: f64, phases: usize) -> Result<f64, ModemError> {
    if !(sps > 1.0 && sps.is_finite()) {
        return Param::Sps.at_least(1.0, sps);
    }
    Param::Phases.check_count(phases, 3, 1024)?;
    if (x.len() as f64) < 2.0 * sps {
        return Err(ModemError::InputTooShort {
            need: (2.0 * sps).ceil() as usize,
        });
    }
    let r = Reader::new(Cubic);
    let n = ((x.len() as f64 - 2.0) / sps).floor() as usize;
    let energy = |p: usize| -> f64 {
        let o = p as f64 / phases as f64 * sps;
        (0..n)
            .map(|k| r.at(x, o + 1.0 + k as f64 * sps).energy() as f64)
            .filter(|e| e.is_finite())
            .sum()
    };
    let (mut best, mut be) = (0usize, f64::NEG_INFINITY);
    // Energies on a stack array for the refinement (phases <= 1024).
    let mut es = [0f64; 1024];
    for (p, e) in es[..phases].iter_mut().enumerate() {
        *e = energy(p);
        if *e > be {
            be = *e;
            best = p;
        }
    }
    let em = es[(best + phases - 1) % phases];
    let ep = es[(best + 1) % phases];
    let pk = sdrkit_algo_dsp::peak::parabolic(em, be, ep);
    let off = if pk.offset.is_finite() {
        pk.offset.clamp(-0.5, 0.5)
    } else {
        0.0
    };
    Ok(((best as f64 + off) / phases as f64 * sps + 1.0).rem_euclid(sps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::interp::{CatmullRom, Linear, Polyphase};
    use sdrkit_algo_sim::rng::Rng;

    /// Symbols `a` through `pulse` at `sps` (with clock error `ppm`), first symbol at sample `t0`, plus complex noise of variance n0
    /// per symbol energy... (the noise is added per sample with variance `n0`).
    fn signal(
        a: &[Complex32],
        pulse: Shape,
        sps: f64,
        t0: f64,
        ppm: f64,
        n0: f64,
        seed: u64,
    ) -> Vec<Complex32> {
        let mut rng = Rng::new(seed);
        let n = ((a.len() as f64 + 2.0) * sps * (1.0 + ppm * 1e-6)) as usize;
        let span = 8i64;
        (0..n)
            .map(|i| {
                let t = (i as f64 - t0) / (sps * (1.0 + ppm * 1e-6));
                let c = t.round() as i64;
                let mut v = Complex32::default();
                for k in (c - span).max(0)..=(c + span).min(a.len() as i64 - 1) {
                    v += a[k as usize] * pulse.value(t - k as f64) as f32;
                }
                v + rng.complex_normal(n0)
            })
            .collect()
    }

    fn qpsk(n: usize, seed: u64) -> Vec<Complex32> {
        let mut rng = Rng::new(seed);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        (0..n)
            .map(|_| {
                Complex32::new(
                    if rng.bit() == 0 { s } else { -s },
                    if rng.bit() == 0 { s } else { -s },
                )
            })
            .collect()
    }

    /// Mean squared error of the outputs from the nearest QPSK point over a range.
    fn mse(out: &[Complex32]) -> f64 {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        out.iter()
            .map(|z| {
                let d = Complex32::new(s * z.re.signum(), s * z.im.signum());
                (z - d).norm_sqr() as f64
            })
            .sum::<f64>()
            / out.len() as f64
    }

    #[test]
    fn detectors_have_the_right_sign_and_a_gain() {
        let rc = Shape::Rc { beta: 0.35 };
        let g = detector_gain::<Complex32, _>(&Gardner, rc, false).unwrap();
        assert!(g > 0.1, "{g}");
        assert!(detector_gain::<Complex32, _>(&ZeroCrossing, rc, false).unwrap() > 0.1);
        assert!(detector_gain::<Complex32, _>(&EarlyLate::default(), rc, false).unwrap() > 0.01);
        assert!(detector_gain::<Complex32, _>(&MuellerMuller, rc, false).unwrap() > 0.1);
        assert!(detector_gain::<f32, _>(&Gardner, rc, false).unwrap() > 0.1);
        assert!(detector_gain::<Complex32, _>(&Gardner, rc, true).unwrap() > 0.1);
        let c = Constellation::new(sdrkit_algo_dsp::constellation::PointSet::qpsk()).unwrap();
        assert!(detector_gain::<Complex32, _>(&MuellerMullerDd::new(c), rc, false).unwrap() > 0.1);
        // Gardner for BPSK with a raised cosine: S'(0) = 2 g'(1/2) - 2 g(1/2) g'(1) over the two neighbours (plus small terms), so
        // the measured gain must be near the analytic one.
        let gp = |t: f64| (rc.value(t + 1e-5) - rc.value(t - 1e-5)) / 2e-5;
        let mut want = 0.0;
        for m in -8..8 {
            let m = m as f64;
            // d/dtau of g(m + 1/2 + tau) (g(m + tau) - g(m + 1 + tau)).
            want += gp(m + 0.5) * (rc.value(m) - rc.value(m + 1.0))
                + rc.value(m + 0.5) * (gp(m) - gp(m + 1.0));
        }
        let got = detector_gain::<f32, _>(&Gardner, rc, false).unwrap();
        assert!((got + want).abs() < 0.03 * want.abs(), "{got} vs {}", -want);
    }

    #[test]
    fn pulls_in_any_offset_and_clock_error_and_cuts_do_not_matter() {
        let a = qpsk(3000, 1);
        let rc = Shape::Rc { beta: 0.35 };
        for (t0, ppm) in [(0.3, 0.0), (1.9, 300.0), (2.7, -1500.0)] {
            let x = signal(&a, rc, 4.0, t0, ppm, 0.0, 2);
            let mut cfg = SymbolSyncConfig::new(4.0);
            cfg.bn_t = 0.01;
            let mut s = SymbolSync::new(
                Gardner,
                Polyphase::<16, 64>::design(0.8, 60.0).unwrap(),
                cfg,
            )
            .unwrap();
            let mut out = Vec::new();
            s.process(&x, &mut out);
            let tail = &out[out.len() - 1000..out.len() - 10];
            assert!(mse(tail) < 2e-3, "t0 {t0} ppm {ppm}: {}", mse(tail));
            assert!(
                (s.clock_ppm() - ppm).abs() < 250.0,
                "{} vs {ppm}",
                s.clock_ppm()
            );
            assert!(s.locked());
            // The same input in uneven pieces produces the same symbols.
            let mut s2 = SymbolSync::new(
                Gardner,
                Polyphase::<16, 64>::design(0.8, 60.0).unwrap(),
                cfg,
            )
            .unwrap();
            let mut out2 = Vec::new();
            let mut i = 0;
            let mut k = 1;
            while i < x.len() {
                let j = (i + k).min(x.len());
                s2.process(&x[i..j], &mut out2);
                i = j;
                k = (k * 7 + 3) % 61;
            }
            assert_eq!(out, out2);
        }
    }

    #[test]
    fn a_clock_error_beyond_the_range_is_refused_and_inside_it_is_tracked() {
        let a = qpsk(4000, 3);
        let rc = Shape::Rc { beta: 0.5 };
        // 3 % clock error: a fixed clamp of +-2 % could not follow it.
        let x = signal(&a, rc, 4.0, 1.0, 30_000.0, 0.0, 4);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.pulse = rc;
        cfg.max_ppm = 50_000.0;
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.03,
            track_bn_t: 0.01,
        });
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.process(&x, &mut out);
        assert!(
            (s.clock_ppm() - 30_000.0).abs() < 500.0,
            "{}",
            s.clock_ppm()
        );
        assert!(mse(&out[out.len() - 500..out.len() - 10]) < 5e-3);
        cfg.max_ppm = 1000.0;
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        out.clear();
        s.process(&x, &mut out);
        assert!(s.clock_ppm() <= 1000.0 + 1e-6);
    }

    /// The steady-state timing jitter: for a loop of one-sided noise bandwidth `Bn T` driven by a detector whose error has variance
    /// `var(e)` per symbol (white) and gain `K`, the timing variance is `2 Bn T var(e) / K^2` symbols^2 (F. M. Gardner, *Phaselock
    /// Techniques*, 3rd ed., eq. 4.20; U. Mengali and A. N. D'Andrea, *Synchronization Techniques for Digital Receivers*, 8.4).
    #[test]
    fn jitter_matches_the_loop_bandwidth() {
        let a = qpsk(40_000, 5);
        let rc = Shape::Rc { beta: 0.5 };
        let (sps, t0) = (4.0, 1.3);
        // Es/N0 = 10 dB after the (noiseless) pulse: noise per sample of 0.1.
        let x = signal(&a, rc, sps, t0, 0.0, 0.1, 6);
        for bn in [0.002, 0.005, 0.01] {
            let mut cfg = SymbolSyncConfig::new(sps);
            cfg.pulse = rc;
            cfg.bn_t = bn;
            let mut s = SymbolSync::new(
                Gardner,
                Polyphase::<16, 64>::design(0.8, 60.0).unwrap(),
                cfg,
            )
            .unwrap();
            let (mut n, mut sum, mut sum2, mut esum2) = (0usize, 0.0f64, 0.0f64, 0.0f64);
            let mut out = Vec::new();
            for (c, chunk) in x.chunks(1024).enumerate() {
                out.clear();
                s.process(chunk, &mut out);
                if c < 24 {
                    continue;
                }
                // One check per call is enough: the instant of the newest symbol against where it was sent.
                let k = ((s.last_instant() - t0) / sps).round();
                let tau = (s.last_instant() - t0 - k * sps) / sps;
                sum += tau;
                sum2 += tau * tau;
                esum2 += (s.last_error() as f64).powi(2);
                n += 1;
            }
            let mean = sum / n as f64;
            let var = sum2 / n as f64 - mean * mean;
            let var_e = esum2 / n as f64;
            let want = 2.0 * bn * var_e / s.detector_gain().powi(2);
            assert!(mean.abs() < 0.02, "bn {bn}: bias {mean}");
            assert!(
                var > want / 2.5 && var < want * 2.5,
                "bn {bn}: var {var:.3e} want {want:.3e}"
            );
        }
    }

    /// Pull-in time is inversely proportional to the loop bandwidth (the loop's time constant is about `1 / (4 Bn T)` symbols).
    #[test]
    fn pull_in_time_scales_with_bandwidth() {
        let a = qpsk(20_000, 21);
        let rc = Shape::Rc { beta: 0.5 };
        let x = signal(&a, rc, 4.0, 2.0, 0.0, 0.01, 22);
        let settle = |bn: f64| -> usize {
            let mut cfg = SymbolSyncConfig::new(4.0);
            cfg.pulse = rc;
            cfg.bn_t = bn;
            let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
            let mut out = Vec::new();
            for (k, chunk) in x.chunks(4).enumerate() {
                s.process(chunk, &mut out);
                let tau = ((s.last_instant() - 2.0) / 4.0 + 0.5).rem_euclid(1.0) - 0.5;
                if k > 4 && tau.abs() < 0.05 {
                    return out.len();
                }
            }
            usize::MAX
        };
        let (slow, fast) = (settle(0.004), settle(0.016));
        let r = slow as f64 / fast as f64;
        assert!(r > 2.0 && r < 8.0, "slow {slow} fast {fast}");
    }

    #[test]
    fn every_detector_and_interpolator_locks() {
        let a = qpsk(3000, 7);
        let rc = Shape::Rc { beta: 0.5 };
        let x = signal(&a, rc, 3.3, 0.7, 100.0, 0.01, 8);
        fn go<D: TimingDetector<Complex32>, I: Interpolator>(d: D, i: I, x: &[Complex32]) -> f64 {
            let mut cfg = SymbolSyncConfig::new(3.3);
            cfg.pulse = Shape::Rc { beta: 0.5 };
            let mut s = SymbolSync::new(d, i, cfg).unwrap();
            let mut out = Vec::new();
            s.process(x, &mut out);
            if out.len() < 2500 {
                return f64::INFINITY;
            }
            mse(&out[out.len() - 800..out.len() - 5])
        }
        let c = Constellation::new(sdrkit_algo_dsp::constellation::PointSet::qpsk()).unwrap();
        for (name, e) in [
            ("gardner linear", go(Gardner, Linear, &x)),
            ("gardner catmull", go(Gardner, CatmullRom, &x)),
            ("zero crossing", go(ZeroCrossing, Cubic, &x)),
            ("early late", go(EarlyLate::default(), Cubic, &x)),
            ("mm", go(MuellerMuller, Cubic, &x)),
            ("mm dd", go(MuellerMullerDd::new(c), Cubic, &x)),
        ] {
            assert!(e < 0.03, "{name}: {e}");
        }
    }

    #[test]
    fn matched_interpolator_filters_an_rrc_link_and_two_per_symbol() {
        let a = qpsk(3000, 9);
        let beta = 0.35;
        let x = signal(&a, Shape::Rrc { beta }, 4.0, 2.2, 50.0, 0.02, 10);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.pulse = Shape::Rc { beta };
        cfg.two_per_symbol = true;
        let mi = MatchedInterp::<48, 64>::new(Shape::Rrc { beta }, 4.0).unwrap();
        let mut s = SymbolSync::new(Gardner, mi, cfg).unwrap();
        let mut out = Vec::new();
        s.process(&x, &mut out);
        let syms: Vec<Complex32> = out.iter().skip(1).step_by(2).copied().collect();
        // The noise after a matched filter at 4 sps: 0.02 * 4 per symbol energy of about 1 → SNR ~ 11 dB; the error is near the noise.
        assert!(
            mse(&syms[syms.len() - 1000..syms.len() - 5]) < 0.12,
            "{}",
            mse(&syms[syms.len() - 1000..])
        );
        assert!(MatchedInterp::<7, 4>::new(Shape::Rect, 2.0).is_err());
    }

    #[test]
    fn a_run_at_a_time_gives_the_symbols_of_process_and_a_start_sets_the_instants() {
        // Symbol k of the signal lies at sample 1.6 k.
        let a = qpsk(3000, 31);
        let (beta, sps) = (0.5, 1.6);
        let x = signal(&a, Shape::Rrc { beta }, sps, 0.0, 0.0, 0.01, 32);
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.pulse = Shape::Rc { beta };
        let start = || {
            let bank = MatchedBank::<16, 128>::new(Shape::Rrc { beta }, sps).unwrap();
            let mut s = RunSymbolSync::<Complex32, _>::new(bank, cfg, 4).unwrap();
            s.reset_to(0.0);
            s
        };
        let mut want = Vec::new();
        start().process(&x, &mut want);
        // The instants start on the symbols: the first outputs are symbols 1, 2, ... of the signal, before the loop has
        // moved (symbol 0 has no half-way instant before it).
        for k in 0..20 {
            let c = want[k] * a[k + 1].conj();
            assert!(c.re > 0.9 * c.norm() && c.re > 0.0, "output {k}: {c}");
        }
        let mut by_run = start();
        let mut got = Vec::new();
        let mut i = 0;
        loop {
            let need = by_run.need();
            let (taken, made) = by_run.process_run(&x[i..], &mut got);
            assert_eq!(taken, need.min(x.len() - i));
            i += taken;
            if !made {
                break;
            }
        }
        assert_eq!(i, x.len());
        assert_eq!(got, want);
    }

    #[test]
    fn the_run_wise_loop_follows_the_per_symbol_loop_and_cuts_do_not_matter() {
        let a = qpsk(8000, 21);
        let (beta, sps) = (0.25, 1.92);
        let x = signal(&a, Shape::Rrc { beta }, sps, 3.3, 300.0, 0.02, 22);
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.pulse = Shape::Rc { beta };
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.04,
            track_bn_t: 0.008,
        });
        cfg.max_ppm = 40_000.0;
        let mi = || MatchedInterp::<64, 128>::new(Shape::Rrc { beta }, sps).unwrap();
        let mut plain = SymbolSync::new(Gardner, mi(), cfg).unwrap();
        let mut want = Vec::new();
        plain.process(&x, &mut want);
        let mut runs = RunSymbolSync::<Complex32, _>::new(mi(), cfg, 8).unwrap();
        let mut got = Vec::new();
        runs.process(&x, &mut got);
        assert!(plain.locked() && runs.locked());
        assert!(
            got.len() + 40 > want.len(),
            "{} for {}",
            got.len(),
            want.len()
        );
        // The clock is followed (a loop that did not follow it would lose two symbols and the eye), and the symbols are as
        // clean as those of the per-symbol loop.
        let tail = |v: &[Complex32]| mse(&v[v.len() - 3000..v.len() - 5]);
        let (per_symbol, per_run) = (tail(&want), tail(&got));
        assert!(
            per_run < 1.05 * per_symbol + 1e-4,
            "{per_run} for {per_symbol}"
        );
        assert!(runs.clock_ppm().abs() < 2000.0, "{}", runs.clock_ppm());
        // Pieces of any length give the same symbols.
        let mut cut = RunSymbolSync::<Complex32, _>::new(mi(), cfg, 8).unwrap();
        let mut pieces = Vec::new();
        let sizes = [1usize, 7, 4096, 333, 65];
        let (mut i, mut k) = (0, 0);
        while i < x.len() {
            let n = sizes[k % sizes.len()].min(x.len() - i);
            cut.process(&x[i..i + n], &mut pieces);
            i += n;
            k += 1;
        }
        assert_eq!(pieces, got);
        assert_eq!(cut.symbols(), got.len() as u64);
        // Held, the loop tracks from its first symbol with whole runs, and comes back to the lock test when let go.
        let mut held = RunSymbolSync::<Complex32, _>::new(mi(), cfg, 8).unwrap();
        held.hold(true);
        let mut kept = Vec::new();
        held.process(&x, &mut kept);
        assert!(tail(&kept) < 1.05 * per_symbol + 1e-4, "{}", tail(&kept));
        held.hold(false);
        assert!(held.locked());
        assert!(RunSymbolSync::<Complex32, _>::new(mi(), cfg, 0).is_err());
        cfg.two_per_symbol = true;
        assert!(RunSymbolSync::<Complex32, _>::new(mi(), cfg, 8).is_err());
    }

    #[test]
    fn any_split_of_the_input_gives_the_same_symbols_and_state() {
        // The stream as a whole: outputs bit for bit, and the state counted from the start of the stream.
        fn key<I: RunInterpolator>(s: &RunSymbolSync<Complex32, I>, out: &[Complex32]) -> Vec<u64> {
            let bits = |z: Complex32| (z.re.to_bits() as u64) << 32 | z.im.to_bits() as u64;
            let reach = I::TAPS / 2 - 1;
            let from = (s.at - s.base) as usize - reach;
            let mut k: Vec<u64> = out.iter().map(|&z| bits(z)).collect();
            k.extend(s.buf[from..].iter().map(|&z| bits(z)));
            k.extend([
                s.at,
                s.frac.to_bits(),
                s.base + s.buf.len() as u64,
                s.lf.integrator().to_bits(),
            ]);
            k.extend([
                s.lf.gains().0.to_bits(),
                s.lf.gains().1.to_bits(),
                s.lock.metric().to_bits() as u64,
            ]);
            k.extend([
                s.lock.locked() as u64,
                s.have_prev as u64,
                bits(s.prev),
                bits(s.prev_mid),
            ]);
            k.extend([
                s.power.to_bits(),
                s.n_power,
                s.level.to_bits() as u64,
                s.symbols,
            ]);
            k.extend([
                s.prev_rot.re.to_bits() as u64,
                s.prev_rot.im.to_bits() as u64,
            ]);
            k
        }
        let (beta, sps) = (0.25, 1.92);
        let mut rng = Rng::new(61);
        // Noise, a signal that the loop locks to, then noise again that the loop loses the lock on.
        let mut x: Vec<Complex32> = (0..3000).map(|_| rng.complex_normal(1.0)).collect();
        x.extend(signal(
            &qpsk(9000, 62),
            Shape::Rrc { beta },
            sps,
            0.7,
            150.0,
            0.02,
            63,
        ));
        let tail = x.len();
        x.extend((0..12_000).map(|_| rng.complex_normal(1.0)));
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.pulse = Shape::Rc { beta };
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.01,
            track_bn_t: 0.001,
        });
        cfg.max_ppm = 300.0;
        let bank = || MatchedBank::<64, 128>::new(Shape::Rrc { beta }, sps).unwrap();
        for (run, hold, oqpsk) in [
            (32, false, false),
            (1, false, false),
            (1, true, false),
            (8, true, false),
            (4, false, true),
        ] {
            let mut c = cfg;
            c.oqpsk = oqpsk;
            let start = || {
                let mut s = RunSymbolSync::<Complex32, _>::new(bank(), c, run).unwrap();
                s.hold(hold);
                s.set_carrier(0.2, 0.003);
                s
            };
            let mut probe = start();
            probe.process(&x[..tail], &mut Vec::new());
            assert!(probe.locked() || oqpsk, "run {run}");
            let mut whole = start();
            let mut want = Vec::new();
            whole.process(&x, &mut want);
            assert!(!whole.locked(), "run {run}");
            let want = key(&whole, &want);
            let mut lengths: Vec<Vec<usize>> = vec![vec![1], vec![3], vec![7]];
            lengths.push(
                (0..400)
                    .map(|_| [0, 1, 2, 5, 40, 700, 1500, 3000][rng.next_u32() as usize % 8])
                    .collect(),
            );
            for sizes in lengths {
                let mut s = start();
                let mut got = Vec::new();
                let (mut i, mut k) = (0, 0);
                while i < x.len() {
                    let n = sizes[k % sizes.len()].min(x.len() - i);
                    s.process(&x[i..i + n], &mut got);
                    i += n;
                    k += 1;
                }
                assert!(
                    key(&s, &got) == want,
                    "run {run}, hold {hold}, pieces {:?}",
                    &sizes[..3.min(sizes.len())]
                );
            }
            // A run at a time.
            let mut s = start();
            let (mut got, mut i) = (Vec::new(), 0);
            while i < x.len() {
                let (taken, made) = s.process_run(&x[i..], &mut got);
                i += taken;
                assert!(made || i == x.len());
            }
            assert!(
                key(&s, &got) == want,
                "run {run}, hold {hold}, a run at a time"
            );
        }
    }

    /// Prints the cost per output symbol of a [`RunSymbolSync`] with a 64-tap bank at 1.92 samples per symbol, for runs
    /// of 1 to 4096 symbols (held, so every run is whole) and for the search state (not held, one symbol per run), on
    /// noise and on a clean signal; the fastest of several passes. Run with `--release --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cost_per_symbol_by_run_length() {
        let (beta, sps) = (0.25, 1.92);
        let a = qpsk(30_000, 41);
        let clean = signal(&a, Shape::Rrc { beta }, sps, 0.0, 30.0, 0.0, 42);
        let mut rng = Rng::new(43);
        let noise: Vec<Complex32> = (0..clean.len()).map(|_| rng.complex_normal(1.0)).collect();
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.pulse = Shape::Rc { beta };
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.01,
            track_bn_t: 0.001,
        });
        cfg.max_ppm = 300.0;
        let cost = |x: &[Complex32], run: usize, held: bool, piece: usize, frozen: bool| {
            let bank = MatchedBank::<64, 128>::new(Shape::Rrc { beta }, sps).unwrap();
            let mut best = f64::INFINITY;
            let mut out = Vec::with_capacity(8192);
            for _ in 0..40 {
                let mut s = RunSymbolSync::<Complex32, _>::new(bank.clone(), cfg, run).unwrap();
                s.hold(held);
                s.lf.set_frozen(frozen);
                let t = std::time::Instant::now();
                for piece in x.chunks(piece) {
                    out.clear();
                    s.process(piece, &mut out);
                }
                let ns = t.elapsed().as_nanos() as f64 / s.symbols().max(1) as f64;
                best = best.min(ns);
            }
            best
        };
        println!("run      noise ns   clean ns");
        for run in [1usize, 2, 8, 64, 4096] {
            let (n, c) = (
                cost(&noise, run, true, 8192, false),
                cost(&clean, run, true, 8192, false),
            );
            println!("{run:<8} {n:>8.1} {c:>10.1}");
        }
        let (n, c) = (
            cost(&noise, 32, false, 8192, false),
            cost(&clean, 32, false, 8192, false),
        );
        println!("search   {n:>8.1} {c:>10.1}");
        // With the loop filter frozen no symbol waits for the error of the one before it.
        let (n, c) = (
            cost(&noise, 1, true, 8192, true),
            cost(&noise, 32, false, 8192, true),
        );
        println!("frozen   run 1 {n:.1}, search {c:.1}");
        for piece in [2usize, 4, 16, 128] {
            let (n, c) = (
                cost(&noise, 32, false, piece, false),
                cost(&clean, 32, true, piece, false),
            );
            println!(
                "pieces of {piece}: search on noise {n:.1}, held runs of 32 on the signal {c:.1}"
            );
        }
    }

    #[test]
    fn the_matched_bank_reads_what_the_blended_filter_reads() {
        let a = qpsk(600, 5);
        let (beta, sps) = (0.25, 1.92);
        let x = signal(&a, Shape::Rrc { beta }, sps, 0.0, 0.0, 0.0, 6);
        let blend = MatchedInterp::<64, 128>::new(Shape::Rrc { beta }, sps).unwrap();
        let bank = MatchedBank::<64, 128>::new(Shape::Rrc { beta }, sps).unwrap();
        let re: Vec<f32> = x.iter().map(|z| z.re).collect();
        let (mut err, mut pow, mut err_re) = (0f64, 0f64, 0f64);
        for i in 0..800 {
            let mu = (i as f32 * 0.37) % 1.0;
            let (want, got) = (blend.at(&x[i..i + 64], mu), bank.at(&x[i..i + 64], mu));
            err += (want - got).norm_sqr() as f64;
            pow += want.norm_sqr() as f64;
            let (want, got) = (blend.at(&re[i..i + 64], mu), bank.at(&re[i..i + 64], mu));
            err_re += ((want - got) * (want - got)) as f64;
        }
        // Half a step of 1/128 of a sample: the error stays more than 45 dB under the signal.
        assert!(err < 3e-5 * pow, "{err} of {pow}");
        assert!(err_re < 3e-5 * pow, "{err_re} of {pow}");
        // The ends of the range are the first and the last row.
        assert!((bank.at(&x[..64], 0.0) - blend.at(&x[..64], 0.0)).norm() < 1e-4);
        assert!((bank.at(&x[..64], 1.0) - blend.at(&x[..64], 1.0)).norm() < 1e-4);
        assert!(MatchedBank::<7, 4>::new(Shape::Rect, 2.0).is_err());
    }

    #[test]
    fn oqpsk_and_real_signals() {
        // OQPSK: Q delayed by half a symbol.
        let a = qpsk(3000, 11);
        let rc = Shape::Rc { beta: 0.5 };
        let sps = 4.0;
        let xi = signal(
            &a.iter()
                .map(|z| Complex32::new(z.re, 0.0))
                .collect::<Vec<_>>(),
            rc,
            sps,
            1.1,
            0.0,
            0.0,
            12,
        );
        let xq = signal(
            &a.iter()
                .map(|z| Complex32::new(0.0, z.im))
                .collect::<Vec<_>>(),
            rc,
            sps,
            1.1 + sps / 2.0,
            0.0,
            0.0,
            12,
        );
        let x: Vec<Complex32> = xi.iter().zip(&xq).map(|(i, q)| i + q).collect();
        let mut cfg = SymbolSyncConfig::new(sps);
        cfg.pulse = rc;
        cfg.oqpsk = true;
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.process(&x, &mut out);
        assert!(
            mse(&out[out.len() - 800..out.len() - 5]) < 5e-3,
            "{}",
            mse(&out[out.len() - 800..])
        );
        // Real (f32) input, the zero-crossing clock of an FSK receiver.
        let r: Vec<f32> = signal(&a, rc, 5.5, 0.4, -200.0, 0.0, 13)
            .iter()
            .map(|z| z.re)
            .collect();
        let mut cfg = SymbolSyncConfig::new(5.5);
        cfg.pulse = rc;
        let mut s = SymbolSync::new(ZeroCrossing, Cubic, cfg).unwrap();
        let mut out: Vec<f32> = Vec::new();
        s.process(&r, &mut out);
        let tail = &out[out.len() - 500..out.len() - 5];
        assert!(
            tail.iter().all(|v| (v.abs() - 1.0).abs() < 0.15),
            "{:?}",
            &tail[..10]
        );
    }

    #[test]
    fn burst_estimators_find_the_instant() {
        let a = qpsk(200, 13);
        let rc = Shape::Rc { beta: 0.35 };
        for t0 in [0.4, 2.0, 3.6] {
            let x = signal(&a, rc, 4.0, t0, 0.0, 0.01, 14);
            let est = spectral_line(&x[16..16 + 600], 4.0).unwrap();
            let d = ((est + 16.0 - t0) / 4.0 + 0.5).rem_euclid(1.0) - 0.5;
            assert!(d.abs() < 0.03, "spectral line t0 {t0}: {est}");
            let est = max_energy(&x[16..16 + 600], 4.0, 16).unwrap();
            let d = ((est + 16.0 - t0) / 4.0 + 0.5).rem_euclid(1.0) - 0.5;
            assert!(d.abs() < 0.04, "max energy t0 {t0}: {est}");
        }
        assert!(spectral_line(&[Complex32::default(); 100], 4.0).is_err());
        assert!(spectral_line(&[Complex32::new(1.0, 0.0); 10], 4.0).is_err());
        assert!(max_energy(&[0f32; 100], 4.0, 2).is_err());
        // The burst form of the loop starts at the given instant.
        let x = signal(&a, rc, 4.0, 1.5, 0.0, 0.0, 15);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.bn_t = 0.002;
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.run(&x, 1.5, &mut out).unwrap();
        assert!(mse(&out[10..150]) < 2e-3, "{}", mse(&out[10..150]));
        assert!((out[20] - a[20]).norm() < 0.1);
        assert!(s.run(&x, f64::NAN, &mut out).is_err());
    }

    #[test]
    fn bad_settings_are_refused() {
        let cfg = SymbolSyncConfig {
            sps: 0.9,
            ..SymbolSyncConfig::new(4.0)
        };
        assert!(SymbolSync::<Complex32, _, _>::new(Gardner, Cubic, cfg).is_err());
        let cfg = SymbolSyncConfig {
            bn_t: 0.7,
            ..SymbolSyncConfig::new(4.0)
        };
        assert!(SymbolSync::<Complex32, _, _>::new(Gardner, Cubic, cfg).is_err());
        let cfg = SymbolSyncConfig {
            detector_gain: Some(-1.0),
            ..SymbolSyncConfig::new(4.0)
        };
        assert!(SymbolSync::<Complex32, _, _>::new(Gardner, Cubic, cfg).is_err());
        // NaN input does not poison the loop.
        let mut s =
            SymbolSync::<Complex32, _, _>::new(Gardner, Cubic, SymbolSyncConfig::new(4.0)).unwrap();
        let mut out = Vec::new();
        s.process(&[Complex32::new(f32::NAN, 0.0); 50], &mut out);
        assert!(s.clock_ppm().is_finite());
    }

    /// An OQPSK signal (Q half a symbol after I) through `pulse`, turned by the carrier `phi0 + 2 pi f n` (f in cycles per sample).
    fn oqpsk_signal(
        a: &[Complex32],
        pulse: Shape,
        sps: f64,
        t0: f64,
        phi0: f64,
        f: f64,
        n0: f64,
    ) -> Vec<Complex32> {
        let i: Vec<Complex32> = a.iter().map(|z| Complex32::new(z.re, 0.0)).collect();
        let q: Vec<Complex32> = a.iter().map(|z| Complex32::new(0.0, z.im)).collect();
        let xi = signal(&i, pulse, sps, t0, 0.0, 0.0, 1);
        let xq = signal(&q, pulse, sps, t0 + sps / 2.0, 0.0, 0.0, 1);
        let mut rng = Rng::new(99);
        xi.iter()
            .zip(&xq)
            .enumerate()
            .map(|(n, (u, v))| {
                let ph = phi0 + std::f64::consts::TAU * ((f * n as f64) % 1.0);
                (u + v) * Complex32::from_polar(1.0, ph as f32) + rng.complex_normal(n0)
            })
            .collect()
    }

    #[test]
    fn the_oqpsk_lock_test_does_not_need_the_carrier() {
        // The lock metric of an OQPSK eye at the right timing is the same at every carrier phase (the plain eye ratio falls to 0
        // at 45 degrees), and it is near 0 a quarter symbol off.
        let rc = Shape::Rc { beta: 0.5 };
        let (_, m0) = s_curve::<Complex32, _>(&Gardner, rc, true, 0.0);
        let (_, mq) = s_curve::<Complex32, _>(&Gardner, rc, true, 0.25);
        let (_, mh) = s_curve::<Complex32, _>(&Gardner, rc, true, 0.5);
        assert!(m0 > 0.2, "{m0}");
        assert!(mq.abs() < 0.2 * m0, "a quarter symbol off: {mq}");
        assert!(
            (mh - m0).abs() < 0.1 * m0,
            "half a symbol late reads as a lock: {mh}"
        );
        let a = qpsk(400, 31);
        let x0 = oqpsk_signal(&a, rc, 4.0, 2.0, 0.0, 0.0, 0.0);
        let mut per_phase = Vec::new();
        for phi in [0.0, 0.4, std::f64::consts::FRAC_PI_4, 2.0] {
            let rot = Complex32::from_polar(1.0, phi as f32);
            let mut w = Complex32::default();
            let mut n = 0;
            let r = Reader::new(Cubic);
            for k in 20..380 {
                let t = 2.0 + 4.0 * k as f64;
                w += oqpsk_eye(r.at(&x0, t - 2.0) * rot, r.at(&x0, t) * rot);
                n += 1;
            }
            let sum = w.norm() as f64;
            per_phase.push(sum / n as f64);
        }
        for v in &per_phase {
            assert!((v - per_phase[0]).abs() < 1e-3, "{per_phase:?}");
        }
    }

    #[test]
    fn oqpsk_locks_with_the_carrier_taken_off_before_the_detector() {
        // OQPSK with a carrier offset of 0.002 cycles per sample (0.008 per symbol) and a phase of 1 rad, at Es/N0 near 17 dB. A
        // decision-directed carrier loop runs on each symbol and passes its phase back to the timing loop.
        let a = qpsk(6000, 41);
        let rc = Shape::Rc { beta: 0.5 };
        let x = oqpsk_signal(&a, rc, 4.0, 1.3, 1.0, 0.002, 0.005);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.pulse = rc;
        cfg.oqpsk = true;
        cfg.gears = Some(Gears {
            pull_in_bn_t: 0.02,
            track_bn_t: 0.005,
        });
        let c = Constellation::new(sdrkit_algo_dsp::constellation::PointSet::qpsk()).unwrap();
        let mut ccfg = crate::carrier::CarrierConfig::new(0.02);
        ccfg.fll_bn_t = Some(0.01);
        let mut cs =
            crate::carrier::CarrierSync::new(crate::carrier::DecisionDirected::new(c), ccfg)
                .unwrap();
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.process_each(&x, |y| {
            out.push(cs.push(y));
            Some(cs.phase())
        });
        assert!(s.locked(), "lock metric {}", s.lock_metric());
        assert!(cs.locked());
        // Up to the loop's quarter-turn ambiguity the symbols are clean.
        let tail = &out[out.len() - 1000..out.len() - 5];
        assert!(mse(tail) < 0.03, "{}", mse(tail));
        // The same input, split anywhere, produces the same symbols.
        let mut s2 = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut cs2 = crate::carrier::CarrierSync::new(
            crate::carrier::DecisionDirected::new(
                Constellation::new(sdrkit_algo_dsp::constellation::PointSet::qpsk()).unwrap(),
            ),
            ccfg,
        )
        .unwrap();
        let mut out2 = Vec::new();
        for chunk in x.chunks(777) {
            s2.process_each(chunk, |y| {
                out2.push(cs2.push(y));
                Some(cs2.phase())
            });
        }
        assert_eq!(out, out2);
        // A carrier phase of 0 changes nothing for a plain signal.
        let b = qpsk(500, 3);
        let xb = signal(&b, rc, 4.0, 0.5, 0.0, 0.0, 1);
        let mut p = SymbolSync::new(Gardner, Cubic, SymbolSyncConfig::new(4.0)).unwrap();
        let mut q = p.clone();
        q.set_carrier_phase(0.0);
        let (mut o1, mut o2) = (Vec::new(), Vec::new());
        p.process(&xb, &mut o1);
        q.process(&xb, &mut o2);
        assert_eq!(o1.len(), o2.len());
        assert!(o1.iter().zip(&o2).all(|(u, v)| (u - v).norm() < 1e-5));
    }

    #[test]
    fn half_a_symbol_moves_the_instants_and_the_oqpsk_pairs() {
        // BPSK at 4 samples per symbol: after the move the next symbol is read 2 samples from the old grid.
        let a = qpsk(2000, 51);
        let rc = Shape::Rc { beta: 0.5 };
        let x = signal(&a, rc, 4.0, 1.0, 0.0, 0.0, 1);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.pulse = rc;
        cfg.bn_t = 0.002;
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.process(&x[..6000], &mut out);
        let before = s.last_instant();
        s.shift_half_symbol();
        out.clear();
        s.process(&x[6000..6010], &mut out);
        let moved = (s.last_instant() - before).rem_euclid(4.0);
        assert!((moved - 2.0).abs() < 0.05, "{moved}");
        // OQPSK locked on time, then moved by half a symbol with a quarter turn of the carrier: the pairs are (Q, next I) turned,
        // which is again a clean QPSK point.
        let x = oqpsk_signal(&a, rc, 4.0, 1.0, 0.0, 0.0, 0.0);
        let mut cfg = SymbolSyncConfig::new(4.0);
        cfg.pulse = rc;
        cfg.oqpsk = true;
        let mut s = SymbolSync::new(Gardner, Cubic, cfg).unwrap();
        let mut out = Vec::new();
        s.process(&x[..5000], &mut out);
        assert!(mse(&out[out.len() - 200..out.len() - 2]) < 1e-2);
        s.shift_half_symbol();
        s.set_carrier_phase(0.25);
        out.clear();
        s.process_each(&x[5000..7000], |y| {
            // The outputs come back in the frame of the input: turn them the same quarter turn to see the pairs.
            out.push(y * Complex32::new(0.0, -1.0));
            None
        });
        assert!(mse(&out[50..out.len() - 2]) < 1e-2, "{}", mse(&out[50..]));
    }

    #[test]
    fn the_edge_clock_samples_at_the_centre_of_each_bit() {
        let mut rng = Rng::new(61);
        let bits: Vec<u8> = (0..2000).map(|_| rng.bit()).collect();
        // 1200 baud from a transmitter 1 % fast, at 48 kHz, starting 17 samples into a bit: after a few edges every bit comes out.
        let spb = 40.0 / 1.01;
        let n = ((bits.len() - 1) as f64 * spb) as usize - 17;
        let levels: Vec<u8> = (0..n)
            .map(|i| bits[((i as f64 + 17.0) / spb) as usize])
            .collect();
        let mut c = EdgeClock::new(1200.0, 48_000.0, 0.5, 0.8).unwrap();
        let got: Vec<u8> = levels.iter().filter_map(|&l| c.sample(l)).collect();
        // Up to the first bit (the clock starts anywhere), the bits match.
        let off = (0..3)
            .find(|&o| got[20..60] == bits[20 + o..60 + o])
            .expect("aligned");
        let errs = got[10..got.len() - 5]
            .iter()
            .zip(&bits[10 + off..])
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(errs, 0);
        // An inertia of 1 never moves the phase at an edge.
        let mut c = EdgeClock::new(1200.0, 48_000.0, 0.5, 1.0).unwrap();
        c.hold(3);
        assert_eq!(c.held(), 3);
        for _ in 0..13 {
            c.sample(0);
        }
        let before = c.phase();
        c.sample(1);
        assert!((c.phase() - before - 1.0 / 40.0).abs() < 1e-6);
        assert!(EdgeClock::new(1200.0, 2000.0, 0.5, 0.5).is_err());
        assert!(EdgeClock::new(1200.0, 48_000.0, 1.5, 0.5).is_err());
        assert!(EdgeClock::new(1200.0, 48_000.0, 0.0, 0.5).is_err());
    }
}
