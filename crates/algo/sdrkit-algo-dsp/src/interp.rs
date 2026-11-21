//! Fractional interpolators, and a [`Reader`] that reads a stored slice at any fractional position.
//!
//! Every [`Interpolator`] works on a window of `TAPS` consecutive samples `hist[0..TAPS]` and returns the value a fraction `mu` (in
//! `[0, 1)`) of the way from `hist[TAPS/2 - 1]` to `hist[TAPS/2]`. So a timing loop that keeps a history calls `at` with the newest
//! `TAPS` samples, and a burst receiver uses [`Reader`] to read a capture at `start + k step` with no copy. The batch forms
//! ([`Reader::read_grid`] on a uniform [`Grid`], [`Reader::read_positions`] at given positions) read a whole row with the
//! results of `at` bit for bit.
//!
//! | Interpolator | Taps | Use |
//! |---|---|---|
//! | [`Linear`] | 2 | many samples per symbol, or cheap envelopes |
//! | [`Cubic`] | 4 | Lagrange cubic (exact for cubic polynomials); the Farrow interpolator of most timing loops |
//! | [`CatmullRom`] | 4 | Keys' cubic convolution with `a = -1/2` (exact for quadratics, continuous slope); 2 to 4 samples per symbol |
//! | [`Polyphase`] | any even | a windowed-sinc bank of `PHASES` fractional delays, adjacent phases blended linearly: near-ideal up to its cut-off |
//!
//! ```
//! use sdrkit_algo_dsp::interp::{Cubic, Interpolator, Reader};
//! let x: Vec<f32> = (0..10).map(|i| (i * i) as f32).collect();
//! // Half way between x[4] = 16 and x[5] = 25: 4.5^2 = 20.25.
//! assert_eq!(Cubic.at(&x[3..7], 0.5), 20.25);
//! assert_eq!(Reader::new(Cubic).at(&x, 4.5), 20.25);
//! ```

use crate::window::bessel_i0;
use crate::{DspError, Param, Sample};

/// The largest `TAPS` a [`Reader`] supports (the size of its stack window at the edges of a slice).
pub const MAX_TAPS: usize = 64;

/// An interpolator over a window of [`TAPS`](Interpolator::TAPS) samples (see the module docs for the convention).
pub trait Interpolator {
    /// The window length (even, at least 2, at most [`MAX_TAPS`]).
    const TAPS: usize;

    /// The value a fraction `mu` of the way from `hist[TAPS/2 - 1]` to `hist[TAPS/2]`. `hist` holds at least `TAPS` samples (extra
    /// ones are ignored; a shorter one returns zero); `mu` is clamped to `[0, 1]`.
    fn at<T: Sample>(&self, hist: &[T], mu: f32) -> T;

    /// Writes `out[k] = at(&x[base[k]..base[k] + TAPS], mu[k])` for each `k`, bit for bit, as one batch: the kernel of the
    /// batch forms [`at_grid`](Self::at_grid) and [`at_positions`](Self::at_positions). Panics when the three lengths differ
    /// or a window lies outside `x`.
    #[inline]
    fn at_windows<T: Sample>(&self, x: &[T], base: &[usize], mu: &[f32], out: &mut [T]) {
        check_windows(x.len(), Self::TAPS, base, mu, out.len());
        windows_plain(self, x, base, mu, out);
    }

    /// Writes `x` read at the positions of `grid` into `out`: output `k` is `at` on the window and fraction of
    /// [`Grid::at(k)`](Grid::at), bit for bit. The window of every output lies inside `x` ([`Grid::inside`]); panics otherwise.
    fn at_grid<T: Sample>(&self, x: &[T], grid: &Grid, out: &mut [T]) {
        let mut g = grid.walk(x.len(), Self::TAPS, out.len());
        let (mut base, mut mu) = ([0usize; BLOCK], [0f32; BLOCK]);
        for o in out.chunks_mut(BLOCK) {
            let n = o.len();
            for (b, m) in base[..n].iter_mut().zip(&mut mu[..n]) {
                (*b, *m) = (g.base, g.mu());
                g.next();
            }
            self.at_windows(x, &base[..n], &mu[..n], o);
        }
    }

    /// Writes `x` read at `pos(k)` into `out[k]`: `at` on the window and the fraction of each position, taken as
    /// [`Reader::at`] takes them (`pos.floor()` and `(pos - pos.floor()) as f32`), bit for bit, in blocks of 64 outputs. A
    /// block with a position that is not finite or a window outside `x` takes `outside(pos(k))` for each of its outputs.
    fn at_positions<T: Sample>(
        &self,
        x: &[T],
        pos: impl Fn(usize) -> f64,
        outside: impl Fn(f64) -> T,
        out: &mut [T],
    ) {
        let before = (Self::TAPS / 2 - 1) as f64;
        let top = x.len() as f64 - Self::TAPS as f64;
        let (mut base, mut mu) = ([0usize; BLOCK], [0f32; BLOCK]);
        for (c, o) in out.chunks_mut(BLOCK).enumerate() {
            let (k0, n) = (c * BLOCK, o.len());
            let mut inside = true;
            for (j, (b, m)) in base[..n].iter_mut().zip(&mut mu[..n]).enumerate() {
                let p = pos(k0 + j);
                let fl = p.floor();
                let first = fl - before;
                // False for a NaN or an infinite position.
                inside &= first >= 0.0 && first <= top;
                (*b, *m) = (first as usize, (p - fl) as f32);
            }
            if inside {
                self.at_windows(x, &base[..n], &mu[..n], o);
            } else {
                for (j, v) in o.iter_mut().enumerate() {
                    *v = outside(pos(k0 + j));
                }
            }
        }
    }
}

/// The outputs of a block of the batch forms.
const BLOCK: usize = 64;

/// Panics unless `base`, `mu` and the output have one length and every window `base[k]..base[k] + taps` lies inside `len`.
#[inline]
fn check_windows(len: usize, taps: usize, base: &[usize], mu: &[f32], n: usize) {
    assert!(
        base.len() == n && mu.len() == n,
        "the lengths of the windows differ"
    );
    let last = base.iter().copied().fold(0, usize::max);
    assert!(
        n == 0 || last.saturating_add(taps) <= len,
        "a window lies outside the slice"
    );
}

/// The plain form of [`Interpolator::at_windows`], on every target, after [`check_windows`].
#[inline]
fn windows_plain<I: Interpolator + ?Sized, T: Sample>(
    i: &I,
    x: &[T],
    base: &[usize],
    mu: &[f32],
    out: &mut [T],
) {
    for ((o, &b), &m) in out.iter_mut().zip(base).zip(mu) {
        // SAFETY: `check_windows` holds every window `b..b + TAPS` inside `x`.
        *o = i.at(unsafe { x.get_unchecked(b..b + I::TAPS) }, m);
    }
}

/// The positions `start + k step` (`k = 0, 1, ...`, `step >= 0`) of the batch forms ([`Interpolator::at_grid`],
/// [`Reader::read_grid`]), on a grid of 2^-32 of a sample. The start and the step are rounded down to 2^-64; output `k` is at
/// the whole part and the top 32 bits of the 64-bit fraction of `start + k step`, which the batch forms advance by integer
/// addition, so the positions do not drift over a long row. [`Grid::at`] is exact in `f64` for positions below 2^21.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    index: i64,
    frac: u64,
    step_index: u64,
    step_frac: u64,
}

/// 2^64 as `f64`.
const TWO64: f64 = 18_446_744_073_709_551_616.0;

impl Grid {
    /// The grid from `start` with `step`. `None` for a value that is not finite, a negative step, or a start or a step of
    /// 2^52 or more in size.
    pub fn new(start: f64, step: f64) -> Option<Grid> {
        let lim = 4_503_599_627_370_496.0; // 2^52
        if !(start.is_finite()
            && step.is_finite()
            && step >= 0.0
            && start.abs() < lim
            && step < lim)
        {
            return None;
        }
        let (s, d) = (start.floor(), step.floor());
        Some(Grid {
            index: s as i64,
            frac: ((start - s) * TWO64) as u64,
            step_index: d as u64,
            step_frac: ((step - d) * TWO64) as u64,
        })
    }

    /// The whole part and the 64-bit fraction of output `k`.
    #[inline]
    fn advance(&self, k: usize) -> (i64, u64) {
        let f = self.frac as u128 + k as u128 * self.step_frac as u128;
        let whole = self.index as i128 + k as i128 * self.step_index as i128 + (f >> 64) as i128;
        (
            whole.clamp(i64::MIN as i128, i64::MAX as i128) as i64,
            f as u64,
        )
    }

    /// The whole part of the position of output `k`.
    pub fn index(&self, k: usize) -> i64 {
        self.advance(k).0
    }

    /// The position of output `k`.
    pub fn at(&self, k: usize) -> f64 {
        let (i, f) = self.advance(k);
        i as f64 + (f >> 32) as f64 * (1.0 / 4_294_967_296.0)
    }

    /// The grid that starts at output `k` of this one.
    pub fn skip(&self, k: usize) -> Grid {
        let (index, frac) = self.advance(k);
        Grid {
            index,
            frac,
            ..*self
        }
    }

    /// The outputs among `0..n` whose window of `taps` samples (`taps/2 - 1` before the position, `taps/2` after) lies inside
    /// a slice of `len` samples: one run, as the positions never decrease.
    pub fn inside(&self, n: usize, taps: usize, len: usize) -> std::ops::Range<usize> {
        let h = (taps / 2) as i64;
        // The first output whose index is at least `at`.
        let first = |at: i64| {
            let (mut lo, mut hi) = (0usize, n);
            while lo < hi {
                let m = lo + (hi - lo) / 2;
                if self.index(m) >= at {
                    hi = m;
                } else {
                    lo = m + 1;
                }
            }
            lo
        };
        let a = first(h - 1);
        let b = first(len as i64 - h);
        a..b.max(a)
    }

    /// A walk over `n` outputs whose windows of `taps` samples lie inside a slice of `len`; panics otherwise.
    #[inline]
    fn walk(&self, len: usize, taps: usize, n: usize) -> Walk {
        let h = (taps / 2) as i64;
        if n > 0 {
            let (first, last) = (self.index(0), self.index(n - 1));
            assert!(
                first >= h - 1 && (last as i128 + h as i128) < len as i128,
                "a window of the grid lies outside the slice"
            );
        }
        Walk {
            base: (self.index - (h - 1)).max(0) as usize,
            frac: self.frac,
            step_index: self.step_index as usize,
            step_frac: self.step_frac,
        }
    }
}

/// The state of a batch form along a [`Grid`]: the first sample of the window and the fraction.
struct Walk {
    base: usize,
    frac: u64,
    step_index: usize,
    step_frac: u64,
}

impl Walk {
    /// The fraction as `f32`: the top 32 bits rounded to nearest, as `(pos - pos.floor()) as f32` rounds it.
    #[inline(always)]
    fn mu(&self) -> f32 {
        ((self.frac >> 32) as u32) as f32 * (1.0 / 4_294_967_296.0)
    }

    /// Moves to the next output.
    #[inline(always)]
    fn next(&mut self) {
        let (f, carry) = self.frac.overflowing_add(self.step_frac);
        self.frac = f;
        self.base += self.step_index + carry as usize;
    }
}

/// Straight-line interpolation between two samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Linear;

impl Interpolator for Linear {
    const TAPS: usize = 2;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        if h.len() < 2 {
            return T::default();
        }
        let mu = mu.clamp(0.0, 1.0);
        h[0] + (h[1] - h[0]) * mu
    }
}

/// Cubic Lagrange interpolation through four samples (exact for any cubic polynomial), evaluated in Farrow form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cubic;

impl Cubic {
    /// The four weights for `mu` (on `hist[0..4]`, the point between `hist[1]` and `hist[2]`).
    #[inline]
    pub fn weights(mu: f32) -> [f32; 4] {
        let m = mu;
        [
            -m * (m - 1.0) * (m - 2.0) / 6.0,
            (m + 1.0) * (m - 1.0) * (m - 2.0) / 2.0,
            -(m + 1.0) * m * (m - 2.0) / 2.0,
            (m + 1.0) * m * (m - 1.0) / 6.0,
        ]
    }
}

impl Interpolator for Cubic {
    const TAPS: usize = 4;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        if h.len() < 4 {
            return T::default();
        }
        let w = Self::weights(mu.clamp(0.0, 1.0));
        h[0] * w[0] + h[1] * w[1] + h[2] * w[2] + h[3] * w[3]
    }
}

/// Catmull-Rom (Keys' cubic convolution with `a = -1/2`): passes through the samples with a continuous slope, exact for quadratics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CatmullRom;

impl Interpolator for CatmullRom {
    const TAPS: usize = 4;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        if h.len() < 4 {
            return T::default();
        }
        let t = mu.clamp(0.0, 1.0);
        let (p0, p1, p2, p3) = (h[0], h[1], h[2], h[3]);
        let a = p1 * 2.0;
        let b = p2 - p0;
        let c = p0 * 2.0 - p1 * 5.0 + p2 * 4.0 - p3;
        let d = (p1 - p2) * 3.0 + p3 - p0;
        (a + b * t + c * (t * t) + d * (t * t * t)) * 0.5
    }

    #[inline]
    fn at_windows<T: Sample>(&self, x: &[T], base: &[usize], mu: &[f32], out: &mut [T]) {
        check_windows(x.len(), 4, base, mu, out.len());
        #[cfg(target_arch = "aarch64")]
        if T::LANES <= 2 {
            if let (Some(xv), Some(ov)) = (T::values(x), T::values_mut(out)) {
                // SAFETY: `check_windows` holds every window inside `x`, and `xv` and `ov` are the values of `x` and `out`.
                unsafe {
                    if T::LANES == 1 {
                        grid_neon::catmull_rom_real(xv, base, mu, ov)
                    } else {
                        grid_neon::catmull_rom_pairs(xv, base, mu, ov)
                    }
                }
                return;
            }
        }
        windows_plain(self, x, base, mu, out);
    }
}

/// A polyphase windowed-sinc interpolator: `PHASES + 1` rows of `TAPS` taps (Kaiser-windowed sinc, each row scaled to unity DC
/// gain), the two rows either side of `mu` blended linearly into one row, then one dot product. With 32 taps and 64 phases
/// (`Polyphase::<32, 64>::design(0.85, 60.0)`) the error is below 3e-3 of full scale up to 0.35 of the rate; 8 taps suit a signal
/// oversampled 5 times or more (about 1 % error at 0.1 of the rate with `new(0.93, 4.5)`).
#[derive(Clone, Debug)]
pub struct Polyphase<const TAPS: usize, const PHASES: usize> {
    table: Vec<f32>,
}

impl<const TAPS: usize, const PHASES: usize> Polyphase<TAPS, PHASES> {
    const CHECK: () = assert!(
        TAPS >= 2 && TAPS.is_multiple_of(2) && TAPS <= MAX_TAPS && PHASES >= 1,
        "TAPS must be even, 2 to MAX_TAPS, and PHASES at least 1"
    );

    /// A bank with its cut-off at `cutoff` of Nyquist (in `(0, 1]`) and a Kaiser window of `beta`.
    pub fn new(cutoff: f64, beta: f64) -> Result<Self, DspError> {
        #[allow(clippy::let_unit_value)]
        let _ = Self::CHECK;
        if !(cutoff > 0.0 && cutoff <= 1.0) {
            return Param::Cutoff.range(0.0, 1.0, cutoff);
        }
        if !(beta >= 0.0 && beta.is_finite()) {
            return Param::Beta.at_least(0.0, beta);
        }
        let half = (TAPS / 2) as f64;
        let i0b = bessel_i0(beta);
        let mut table = vec![0f32; (PHASES + 1) * TAPS];
        let mut row = [0f64; MAX_TAPS];
        for p in 0..=PHASES {
            let mu = p as f64 / PHASES as f64;
            let mut sum = 0.0;
            for (k, r) in row[..TAPS].iter_mut().enumerate() {
                // hist[k] is at k - (TAPS/2 - 1) - mu from the requested point.
                let x = k as f64 - (half - 1.0) - mu;
                let s = cutoff * crate::pulse::sinc(cutoff * x);
                let wx = x / (half + 0.5);
                let w = if wx.abs() >= 1.0 {
                    0.0
                } else {
                    bessel_i0(beta * (1.0 - wx * wx).sqrt()) / i0b
                };
                *r = s * w;
                sum += *r;
            }
            for k in 0..TAPS {
                table[p * TAPS + k] = (row[k] / sum) as f32;
            }
        }
        Ok(Polyphase { table })
    }

    /// A bank with its cut-off at `cutoff` of Nyquist and the Kaiser beta for `atten_db` of stop-band attenuation.
    pub fn design(cutoff: f64, atten_db: f64) -> Result<Self, DspError> {
        Self::new(cutoff, crate::window::kaiser_beta(atten_db))
    }

    /// Row `p` (`0..=PHASES`) of the table: the taps for `mu = p / PHASES`.
    pub fn row(&self, p: usize) -> &[f32] {
        &self.table[p * TAPS..(p + 1) * TAPS]
    }
}

impl<const TAPS: usize, const PHASES: usize> Interpolator for Polyphase<TAPS, PHASES> {
    const TAPS: usize = TAPS;
    #[inline]
    fn at<T: Sample>(&self, h: &[T], mu: f32) -> T {
        if h.len() < TAPS {
            return T::default();
        }
        let pf = mu.clamp(0.0, 1.0) * PHASES as f32;
        let p = (pf as usize).min(PHASES - 1);
        let f = pf - p as f32;
        let a = &self.table[p * TAPS..(p + 1) * TAPS];
        let b = &self.table[(p + 1) * TAPS..(p + 2) * TAPS];
        let mut row = [0f32; TAPS];
        for k in 0..TAPS {
            row[k] = a[k] + (b[k] - a[k]) * f;
        }
        T::dot(&row, &h[..TAPS])
    }

    #[inline]
    fn at_windows<T: Sample>(&self, x: &[T], base: &[usize], mu: &[f32], out: &mut [T]) {
        check_windows(x.len(), TAPS, base, mu, out.len());
        #[cfg(target_arch = "aarch64")]
        if T::LANES == 1 && TAPS.is_multiple_of(8) && TAPS < 32 {
            if let (Some(xv), Some(ov)) = (T::values(x), T::values_mut(out)) {
                // SAFETY: `check_windows` holds every window inside `x`, and the table holds `PHASES + 1` rows of `TAPS`.
                unsafe { grid_neon::polyphase_real::<TAPS, PHASES>(&self.table, xv, base, mu, ov) };
                return;
            }
        }
        windows_plain(self, x, base, mu, out);
    }
}

/// Reads a stored slice at absolute fractional positions through an interpolator: the burst form of every synchroniser (timing
/// trials and data-aided fits read the capture where they need it, with no copy). Samples outside the slice read as zero.
#[derive(Clone, Debug, Default)]
pub struct Reader<I> {
    interp: I,
}

impl<I: Interpolator> Reader<I> {
    /// A reader using `interp`.
    pub fn new(interp: I) -> Self {
        Reader { interp }
    }

    /// The interpolator.
    pub fn interpolator(&self) -> &I {
        &self.interp
    }

    /// The value of `x` at position `pos` (in samples from `x[0]`). Zero for a position whose window lies wholly outside, or NaN.
    #[inline]
    pub fn at<T: Sample>(&self, x: &[T], pos: f64) -> T {
        if !pos.is_finite() {
            return T::default();
        }
        let fl = pos.floor();
        let mu = (pos - fl) as f32;
        let base = fl as i64 - (I::TAPS as i64 / 2 - 1);
        if base >= 0 && (base as usize).saturating_add(I::TAPS) <= x.len() {
            let b = base as usize;
            return self.interp.at(&x[b..b + I::TAPS], mu);
        }
        if base + I::TAPS as i64 <= 0 || base >= x.len() as i64 {
            return T::default();
        }
        let mut w = [T::default(); MAX_TAPS];
        for (k, v) in w[..I::TAPS].iter_mut().enumerate() {
            let j = base + k as i64;
            if j >= 0 && (j as usize) < x.len() {
                *v = x[j as usize];
            }
        }
        self.interp.at(&w[..I::TAPS], mu)
    }

    /// The positions [`read`](Self::read) uses: `start + k step (1 + ppm 1e-6)`.
    #[inline]
    pub fn position(start: f64, step: f64, ppm: f64, k: usize) -> f64 {
        start + k as f64 * step * (1.0 + ppm * 1e-6)
    }

    /// Fills `out` with `x` read at `start + k step (1 + ppm 1e-6)` for `k` in `0..out.len()`: symbols at `step` samples per symbol
    /// from `start`, for a capture whose clock runs `ppm` fast. Each position is computed from `k` (no drift over long reads), and
    /// the reads go through [`read_positions`](Self::read_positions), so each equals `at` at its position bit for bit.
    pub fn read<T: Sample>(&self, x: &[T], start: f64, step: f64, ppm: f64, out: &mut [T]) {
        self.read_positions(x, |k| Self::position(start, step, ppm, k), out);
    }

    /// Fills `out` with `x` read at the positions of `grid`: output `k` is [`at`](Self::at) at [`Grid::at(k)`](Grid::at), bit
    /// for bit, through [`Interpolator::at_grid`] where the window lies inside `x`.
    pub fn read_grid<T: Sample>(&self, x: &[T], grid: &Grid, out: &mut [T]) {
        let r = grid.inside(out.len(), I::TAPS, x.len());
        for k in (0..r.start).chain(r.end..out.len()) {
            out[k] = self.at(x, grid.at(k));
        }
        self.interp
            .at_grid(x, &grid.skip(r.start), &mut out[r.start..r.end]);
    }

    /// Fills `out` with `x` read at `pos(k)`: output `k` is [`at`](Self::at) at `pos(k)`, bit for bit, through
    /// [`Interpolator::at_positions`].
    pub fn read_positions<T: Sample>(&self, x: &[T], pos: impl Fn(usize) -> f64, out: &mut [T]) {
        self.interp.at_positions(x, pos, |p| self.at(x, p), out);
    }

    /// Whether every one of `n` reads from `start` lies wholly inside a slice of `len` samples (so none is zero-padded).
    pub fn fits(&self, len: usize, start: f64, step: f64, ppm: f64, n: usize) -> bool {
        if n == 0 {
            return true;
        }
        let a = start;
        let b = Self::position(start, step, ppm, n - 1);
        let (lo, hi) = (a.min(b), a.max(b));
        let half = (I::TAPS / 2) as f64;
        lo.is_finite()
            && hi.is_finite()
            && lo.floor() - (half - 1.0) >= 0.0
            && hi.floor() + half < len as f64
    }
}

/// The batch forms on NEON (ARM64). Each makes the operations of the plain form in the same order, with no fused step that
/// the plain form does not fuse, so the results are equal bit for bit.
#[cfg(target_arch = "aarch64")]
mod grid_neon {
    use super::{CatmullRom, Interpolator};
    use std::arch::aarch64::{
        float32x4_t, vaddq_f32, vbslq_f32, vcgtq_f32, vcltq_f32, vcombine_f32, vdupq_n_f32,
        vfmaq_f32, vgetq_lane_f32, vld1_f32, vld1q_f32, vmulq_f32, vmulq_n_f32,
        vreinterpretq_f32_f64, vreinterpretq_f64_f32, vst1q_f32, vsubq_f32, vtrn1q_f32, vtrn1q_f64,
        vtrn2q_f32, vtrn2q_f64,
    };

    /// [`super::Polyphase::at`] for `f32` on each window, for `TAPS` a multiple of 8 below 32: the blend `a + (b - a) f`, then the
    /// sums of `Sample::dot` for `f32` (one fused sum per tap over 8 lanes, then `((0+4)+(1+5)) + ((2+6)+(3+7))`).
    ///
    /// # Safety
    /// `base`, `mu` and `out` have one length, every window `base[k]..base[k] + TAPS` lies inside `x`, and `table` holds
    /// `PHASES + 1` rows of `TAPS`.
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn polyphase_real<const TAPS: usize, const PHASES: usize>(
        table: &[f32],
        x: &[f32],
        base: &[usize],
        mu: &[f32],
        out: &mut [f32],
    ) {
        debug_assert!(table.len() >= (PHASES + 1) * TAPS);
        let zero = vdupq_n_f32(0.0);
        for ((o, &first), &m) in out.iter_mut().zip(base).zip(mu) {
            let pf = m.clamp(0.0, 1.0) * PHASES as f32;
            let p = (pf as usize).min(PHASES - 1);
            let fv = vdupq_n_f32(pf - p as f32);
            // SAFETY: `p + 1 <= PHASES`, so rows `p` and `p + 1` are inside `table`; the window `base..base + TAPS` is inside
            // `x` (the caller's contract). The loads need no alignment.
            unsafe {
                let a = table.as_ptr().add(p * TAPS);
                let b = a.add(TAPS);
                let w = x.as_ptr().add(first);
                let mut acc = [zero; 2];
                for c in 0..TAPS / 8 {
                    for (j, s) in acc.iter_mut().enumerate() {
                        let q = 8 * c + 4 * j;
                        let ta = vld1q_f32(a.add(q));
                        let t = vaddq_f32(ta, vmulq_f32(vsubq_f32(vld1q_f32(b.add(q)), ta), fv));
                        *s = vfmaq_f32(*s, t, vld1q_f32(w.add(q)));
                    }
                }
                let v = vaddq_f32(vaddq_f32(acc[0], zero), vaddq_f32(acc[1], zero));
                *o = (vgetq_lane_f32::<0>(v) + vgetq_lane_f32::<1>(v))
                    + (vgetq_lane_f32::<2>(v) + vgetq_lane_f32::<3>(v));
            }
        }
    }

    /// [`CatmullRom::at`] on four lanes: `p0..p3` are the four samples of each lane's window and `t` its fraction.
    #[target_feature(enable = "neon")]
    fn catmull_rom4(
        p0: float32x4_t,
        p1: float32x4_t,
        p2: float32x4_t,
        p3: float32x4_t,
        t: float32x4_t,
    ) -> float32x4_t {
        // `f32::clamp(0, 1)` lane by lane: a NaN and a negative zero stay as they are.
        let (zero, one) = (vdupq_n_f32(0.0), vdupq_n_f32(1.0));
        let t = vbslq_f32(vcltq_f32(t, zero), zero, t);
        let t = vbslq_f32(vcgtq_f32(t, one), one, t);
        let a = vmulq_n_f32(p1, 2.0);
        let b = vsubq_f32(p2, p0);
        let c = vsubq_f32(
            vaddq_f32(
                vsubq_f32(vmulq_n_f32(p0, 2.0), vmulq_n_f32(p1, 5.0)),
                vmulq_n_f32(p2, 4.0),
            ),
            p3,
        );
        let d = vsubq_f32(vaddq_f32(vmulq_n_f32(vsubq_f32(p1, p2), 3.0), p3), p0);
        let tt = vmulq_f32(t, t);
        let ttt = vmulq_f32(tt, t);
        let s = vaddq_f32(
            vaddq_f32(vaddq_f32(a, vmulq_f32(b, t)), vmulq_f32(c, tt)),
            vmulq_f32(d, ttt),
        );
        vmulq_n_f32(s, 0.5)
    }

    /// [`CatmullRom::at`] for `f32` on each window, four outputs at a time.
    ///
    /// # Safety
    /// `base`, `mu` and `out` have one length, and every window `base[k]..base[k] + 4` lies inside `x`.
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn catmull_rom_real(x: &[f32], base: &[usize], mu: &[f32], out: &mut [f32]) {
        let (quads, rest) = out.as_chunks_mut::<4>();
        let (bq, br) = base.as_chunks::<4>();
        let (mq, mr) = mu.as_chunks::<4>();
        for ((q, b), m) in quads.iter_mut().zip(bq).zip(mq) {
            // SAFETY: each window `b[j]..b[j] + 4` is inside `x` (the caller's contract); `m` holds four values.
            let (r, t) = unsafe {
                (
                    b.map(|first| vld1q_f32(x.as_ptr().add(first))),
                    vld1q_f32(m.as_ptr()),
                )
            };
            // The rows are the windows of four outputs; the columns are the samples p0..p3 of each.
            let (e0, o0) = (vtrn1q_f32(r[0], r[1]), vtrn2q_f32(r[0], r[1]));
            let (e1, o1) = (vtrn1q_f32(r[2], r[3]), vtrn2q_f32(r[2], r[3]));
            let col = |lo: float32x4_t, hi: float32x4_t, second: bool| {
                let (l, h) = (vreinterpretq_f64_f32(lo), vreinterpretq_f64_f32(hi));
                vreinterpretq_f32_f64(if second {
                    vtrn2q_f64(l, h)
                } else {
                    vtrn1q_f64(l, h)
                })
            };
            let y = catmull_rom4(
                col(e0, e1, false),
                col(o0, o1, false),
                col(e0, e1, true),
                col(o0, o1, true),
                t,
            );
            // SAFETY: `q` holds four values.
            unsafe { vst1q_f32(q.as_mut_ptr(), y) };
        }
        for ((o, &first), &m) in rest.iter_mut().zip(br).zip(mr) {
            // SAFETY: the window `first..first + 4` is inside `x` (the caller's contract).
            *o = CatmullRom.at(unsafe { x.get_unchecked(first..first + 4) }, m);
        }
    }

    /// [`CatmullRom::at`] for two-lane samples (`[re, im]`, flat in `x` and `out`) on each window, two outputs at a time.
    ///
    /// # Safety
    /// `base`, `mu` and the samples of `out` have one length, and every window of samples `base[k]..base[k] + 4` lies inside
    /// the samples of `x`.
    #[target_feature(enable = "neon")]
    pub(super) unsafe fn catmull_rom_pairs(x: &[f32], base: &[usize], mu: &[f32], out: &mut [f32]) {
        let (twos, rest) = out.as_chunks_mut::<4>();
        let (bq, br) = base.as_chunks::<2>();
        let (mq, mr) = mu.as_chunks::<2>();
        let xp = x.as_ptr();
        for ((q, b), m) in twos.iter_mut().zip(bq).zip(mq) {
            let (b0, b1) = (2 * b[0], 2 * b[1]);
            // SAFETY: the windows of values `b..b + 8` are inside `x` (the caller's contract); `q` holds four values.
            unsafe {
                let p = |j: usize| {
                    vcombine_f32(vld1_f32(xp.add(b0 + 2 * j)), vld1_f32(xp.add(b1 + 2 * j)))
                };
                let t = [m[0], m[0], m[1], m[1]];
                let y = catmull_rom4(p(0), p(1), p(2), p(3), vld1q_f32(t.as_ptr()));
                vst1q_f32(q.as_mut_ptr(), y);
            }
        }
        if let (Some(o), Some(&first), Some(&t)) = (
            rest.as_chunks_mut::<2>().0.first_mut(),
            br.first(),
            mr.first(),
        ) {
            let b = 2 * first;
            for (lane, v) in o.iter_mut().enumerate() {
                let w: [f32; 4] = std::array::from_fn(|j| x[b + 2 * j + lane]);
                *v = CatmullRom.at(&w, t);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Complex32;
    use std::f64::consts::TAU;

    #[test]
    fn cubic_is_exact_for_cubics_and_catmull_rom_for_quadratics() {
        let cubic = |t: f64| 0.3 * t * t * t - 2.0 * t * t + t - 5.0;
        let quad = |t: f64| -1.5 * t * t + 0.25 * t + 3.0;
        let xc: Vec<f32> = (0..8).map(|i| cubic(i as f64) as f32).collect();
        let xq: Vec<f32> = (0..8).map(|i| quad(i as f64) as f32).collect();
        for k in 0..=10 {
            let mu = k as f32 / 10.0;
            let t = 3.0 + mu as f64;
            assert!(
                (Cubic.at(&xc[2..6], mu) as f64 - cubic(t)).abs() < 1e-4,
                "{mu}"
            );
            assert!(
                (CatmullRom.at(&xq[2..6], mu) as f64 - quad(t)).abs() < 1e-5,
                "{mu}"
            );
            // Linear is exact for lines.
            let xl = [1.0f32, 3.0];
            assert!((Linear.at(&xl, mu) - (1.0 + 2.0 * mu)).abs() < 1e-6);
        }
        // Each passes through the samples.
        assert_eq!(Cubic.at(&xc[2..6], 0.0), xc[3]);
        assert_eq!(CatmullRom.at(&xc[2..6], 0.0), xc[3]);
        let w = Cubic::weights(0.3);
        assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    fn tone(n: usize, f: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| Complex32::from_polar(1.0, (TAU * f * i as f64) as f32))
            .collect()
    }

    fn worst_error<I: Interpolator>(i: I, f: f64) -> f64 {
        let x = tone(400, f);
        let r = Reader::new(i);
        let mut worst = 0.0f64;
        for k in 0..1000 {
            let pos = 100.0 + k as f64 * 0.1937;
            let want = Complex32::from_polar(1.0, (TAU * f * pos) as f32);
            worst = worst.max((r.at(&x, pos) - want).norm() as f64);
        }
        worst
    }

    #[test]
    fn polyphase_is_near_ideal_below_its_cutoff() {
        let p = Polyphase::<32, 64>::design(0.85, 60.0).unwrap();
        for f in [0.01, 0.1, 0.25, 0.35] {
            let e = worst_error(p.clone(), f);
            assert!(e < 3e-3, "f {f}: {e}");
        }
        let p8 = Polyphase::<8, 64>::new(0.93, 4.5).unwrap();
        let e8 = worst_error(p8, 0.1);
        assert!(e8 < 0.015, "{e8}");
        // The cubic needs more samples per cycle.
        assert!(worst_error(Cubic, 0.05) < 2e-3);
        assert!(worst_error(CatmullRom, 0.05) < 5e-3);
        // Rows have unity DC gain, and phase 0 is (close to) a pure delay.
        for p_ in 0..=64 {
            let s: f32 = p.row(p_).iter().sum();
            assert!((s - 1.0).abs() < 1e-5);
        }
        // With the cut-off at Nyquist, phase 0 is a pure delay (the sinc is 0 at the other samples).
        let full = Polyphase::<16, 32>::new(1.0, 5.0).unwrap();
        assert!((full.row(0)[7] - 1.0).abs() < 1e-6);
        assert!(full
            .row(0)
            .iter()
            .enumerate()
            .all(|(k, &v)| k == 7 || v.abs() < 1e-6));
        assert!(Polyphase::<4, 8>::new(0.0, 5.0).is_err());
    }

    #[test]
    fn reader_reads_with_start_step_and_ppm() {
        let f = 0.02;
        let x = tone(5000, f);
        let r = Reader::new(Polyphase::<16, 128>::design(0.8, 60.0).unwrap());
        let mut out = vec![Complex32::default(); 100];
        let (start, step, ppm) = (123.4, 37.25, 150.0);
        assert!(r.fits(x.len(), start, step, ppm, out.len()));
        r.read(&x, start, step, ppm, &mut out);
        for (k, v) in out.iter().enumerate() {
            let pos = start + k as f64 * step * (1.0 + 150e-6);
            let want = Complex32::from_polar(1.0, (TAU * f * pos) as f32);
            assert!((v - want).norm() < 3e-3, "{k}");
        }
        // Off the ends: zero-padded, never a panic.
        assert!(!r.fits(x.len(), 2.0, 1.0, 0.0, 10));
        assert!(!r.fits(x.len(), 4990.0, 1.0, 0.0, 10));
        assert_eq!(r.at(&x, -100.0), Complex32::default());
        assert_eq!(r.at(&x, 1e9), Complex32::default());
        assert_eq!(r.at(&x, f64::NAN), Complex32::default());
        let edge = r.at(&x, 0.0);
        assert!(edge.norm() > 0.3 && edge.norm() < 1.0);
        let empty: [f32; 0] = [];
        assert_eq!(Reader::new(Cubic).at(&empty, 0.5), 0.0);
    }

    /// A small linear congruential generator, uniform in `[0, 1)`.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// The steps of the picture rows of the television stacks at 10 and 20 Msps: luminance (52 µs over 768 pixels, 52.656 µs
    /// over 640), chrominance (half of it) and SECAM chrominance (a quarter and a eighth).
    fn raster_steps() -> Vec<f64> {
        let mut s = Vec::new();
        for fv in [10e6, 20e6] {
            for (us, w) in [(52.0, 768.0), (52.656, 640.0)] {
                let dx: f64 = us * 1e-6 * fv / w;
                s.extend([dx, dx / 2.0, dx / 4.0, dx / 8.0]);
            }
        }
        s
    }

    fn noise_real(n: usize, rng: &mut Lcg) -> Vec<f32> {
        (0..n).map(|_| (rng.next() * 2.0 - 1.0) as f32).collect()
    }

    fn noise_complex(n: usize, rng: &mut Lcg) -> Vec<Complex32> {
        (0..n)
            .map(|_| Complex32::new((rng.next() * 2.0 - 1.0) as f32, (rng.next() - 0.5) as f32))
            .collect()
    }

    /// Checks the batch form of `r` against `at` at every position of `grid`, bit for bit, over `n` outputs.
    fn same_as_per_call<I: Interpolator, T: Sample>(r: &Reader<I>, x: &[T], grid: &Grid, n: usize) {
        let mut out = vec![T::default(); n];
        r.read_grid(x, grid, &mut out);
        for (k, o) in out.iter().enumerate() {
            let want = r.at(x, grid.at(k));
            assert!(
                format!("{o:?}") == format!("{want:?}") && *o == want,
                "output {k} of {n} at {}: {o:?} against {want:?}",
                grid.at(k)
            );
        }
    }

    #[test]
    fn the_batch_form_equals_the_per_call_form_bit_for_bit() {
        let mut rng = Lcg(7);
        let poly8 = Reader::new(Polyphase::<8, 64>::new(0.93, 4.5).unwrap());
        let poly16 = Reader::new(Polyphase::<16, 32>::design(0.8, 60.0).unwrap());
        let cr = Reader::new(CatmullRom);
        let mut steps = raster_steps();
        steps.extend([0.0, 1.0, 0.5, 2.0, 3.75]);
        for _ in 0..40 {
            steps.push(0.05 + rng.next() * 3.0);
        }
        let xr = noise_real(9000, &mut rng);
        let xc = noise_complex(9000, &mut rng);
        for (i, &d) in steps.iter().enumerate() {
            for _ in 0..6 {
                let n = (rng.next() * 2000.0) as usize;
                // Starts before the slice, inside it, and near its end, so the edges are read too.
                let start = rng.next() * 60.0 - 10.0 + if i % 3 == 0 { 6000.0 } else { 0.0 };
                let g = Grid::new(start, d).unwrap();
                same_as_per_call(&poly8, &xr, &g, n);
                same_as_per_call(&poly16, &xr, &g, n);
                same_as_per_call(&poly8, &xc, &g, n);
                same_as_per_call(&cr, &xr, &g, n);
                same_as_per_call(&cr, &xc, &g, n);
            }
        }
        // Fractions whose top 32 bits round in `f32` onto a phase of the bank or onto 1.
        for m in [
            0u64,
            1,
            0x03FF_FFFF,
            0x0400_0000,
            0x7FFF_FFC0,
            0xFFFF_FF80,
            0xFFFF_FFFF,
        ] {
            let g = Grid::new(100.0 + m as f64 / 4_294_967_296.0, 0.0).unwrap();
            for n in [1, 5] {
                same_as_per_call(&poly8, &xr, &g, n);
                same_as_per_call(&cr, &xc, &g, n);
            }
        }
        // A long row: the positions are those of `start + k step` within the grid, with no drift.
        let long = noise_real(400_000, &mut rng);
        let (start, d) = (3.3, raster_steps()[0]);
        let g = Grid::new(start, d).unwrap();
        let n = 580_000;
        same_as_per_call(&poly8, &long, &g, n);
        same_as_per_call(&cr, &long, &g, n);
        for k in [0, 1, 1000, n - 1] {
            assert!((g.at(k) - (start + k as f64 * d)).abs() < 1e-9, "{k}");
        }
        assert!(Grid::new(0.0, -1.0).is_none() && Grid::new(f64::NAN, 1.0).is_none());
    }

    /// The plain form along `g`, with the windows and fractions taken from [`Grid::at`].
    fn plain_grid<I: Interpolator, T: Sample>(i: &I, x: &[T], g: &Grid, out: &mut [T]) {
        let n = out.len();
        let base: Vec<usize> = (0..n)
            .map(|k| (g.index(k) - (I::TAPS / 2 - 1) as i64) as usize)
            .collect();
        let mu: Vec<f32> = (0..n).map(|k| (g.at(k) - g.at(k).floor()) as f32).collect();
        windows_plain(i, x, &base, &mu, out);
    }

    #[test]
    fn the_positions_form_equals_the_per_call_form_bit_for_bit() {
        let mut rng = Lcg(5);
        let xr = noise_real(3000, &mut rng);
        let xc = noise_complex(3000, &mut rng);
        let poly = Reader::new(Polyphase::<8, 64>::new(0.93, 4.5).unwrap());
        let cr = Reader::new(CatmullRom);
        for &d in raster_steps().iter().chain(&[0.0, 1.0, 2.3]) {
            for _ in 0..8 {
                // A row at an absolute start near 1e7, read from a segment that starts at `a`, running off both ends at times.
                let x0 = 1e7 + rng.next() * 1e6;
                let a = x0.floor() - 40.0 + (rng.next() * 120.0 - 60.0).round();
                let n = (rng.next() * 1500.0) as usize;
                let pos = |k: usize| x0 + (k as f64 + 0.5) * d - a;
                let mut out = vec![0f32; n];
                poly.read_positions(&xr, pos, &mut out);
                assert!(out
                    .iter()
                    .enumerate()
                    .all(|(k, o)| o.to_bits() == poly.at(&xr, pos(k)).to_bits()));
                cr.read_positions(&xr, pos, &mut out);
                assert!(out
                    .iter()
                    .enumerate()
                    .all(|(k, o)| o.to_bits() == cr.at(&xr, pos(k)).to_bits()));
                let mut out = vec![Complex32::default(); n];
                cr.read_positions(&xc, pos, &mut out);
                assert!(out.iter().enumerate().all(|(k, o)| *o == cr.at(&xc, pos(k))
                    && o.re.to_bits() == cr.at(&xc, pos(k)).re.to_bits()));
            }
        }
        // A position that is not finite reads as `at` reads it.
        let pos = |k: usize| {
            if k == 70 {
                f64::NAN
            } else if k == 3 {
                f64::INFINITY
            } else {
                50.0 + k as f64 * 0.7
            }
        };
        let mut out = vec![1f32; 200];
        poly.read_positions(&xr, pos, &mut out);
        assert!(out
            .iter()
            .enumerate()
            .all(|(k, o)| o.to_bits() == poly.at(&xr, pos(k)).to_bits()));
        assert_eq!((out[3], out[70]), (0.0, 0.0));
        // `read` with a clock offset, over the ends of the slice.
        let (start, step, ppm) = (-5.3, 1.37, 150.0);
        let mut out = vec![Complex32::default(); 2300];
        cr.read(&xc, start, step, ppm, &mut out);
        for (k, o) in out.iter().enumerate() {
            let want = cr.at(&xc, Reader::<CatmullRom>::position(start, step, ppm, k));
            assert_eq!(
                [o.re.to_bits(), o.im.to_bits()],
                [want.re.to_bits(), want.im.to_bits()],
                "{k}"
            );
        }
    }

    #[test]
    fn the_window_kernels_equal_at_for_any_fraction() {
        let mut rng = Lcg(13);
        let xr = noise_real(400, &mut rng);
        let xc = noise_complex(400, &mut rng);
        let p8 = Polyphase::<8, 64>::new(0.93, 4.5).unwrap();
        let n = 203;
        let base: Vec<usize> = (0..n).map(|_| (rng.next() * 380.0) as usize).collect();
        let special = [0.0f32, -0.0, 1.0, -0.25, 1.5, 0.999_999_94, 1e-30];
        let mu: Vec<f32> = (0..n)
            .map(|k| special.get(k % 11).copied().unwrap_or(rng.next() as f32))
            .collect();
        let mut out = vec![0f32; n];
        p8.at_windows(&xr, &base, &mu, &mut out);
        for k in 0..n {
            assert_eq!(
                out[k].to_bits(),
                p8.at(&xr[base[k]..], mu[k]).to_bits(),
                "{k}"
            );
        }
        CatmullRom.at_windows(&xr, &base, &mu, &mut out);
        for k in 0..n {
            assert_eq!(
                out[k].to_bits(),
                CatmullRom.at(&xr[base[k]..], mu[k]).to_bits(),
                "{k}"
            );
        }
        let mut out = vec![Complex32::default(); n];
        CatmullRom.at_windows(&xc, &base, &mu, &mut out);
        for k in 0..n {
            let want = CatmullRom.at(&xc[base[k]..], mu[k]);
            assert_eq!(
                [out[k].re.to_bits(), out[k].im.to_bits()],
                [want.re.to_bits(), want.im.to_bits()],
                "{k}"
            );
        }
        let xs: Vec<crate::Stereo> = xc.iter().map(|c| crate::Stereo::new(c.re, c.im)).collect();
        let mut out = vec![crate::Stereo::default(); n];
        CatmullRom.at_windows(&xs, &base, &mu, &mut out);
        for k in 0..n {
            let want = CatmullRom.at(&xs[base[k]..], mu[k]);
            assert_eq!(
                [out[k].l.to_bits(), out[k].r.to_bits()],
                [want.l.to_bits(), want.r.to_bits()],
                "{k}"
            );
        }
    }

    #[test]
    fn the_neon_forms_equal_the_plain_form_bit_for_bit() {
        let mut rng = Lcg(11);
        let xr = noise_real(5000, &mut rng);
        let mut xz = xr.clone();
        // Zeros of both signs, so the sign of a zero result is checked too.
        for (k, v) in xz.iter_mut().enumerate() {
            if k % 3 != 0 {
                *v = if k % 2 == 0 { 0.0 } else { -0.0 };
            }
        }
        let xc = noise_complex(5000, &mut rng);
        let p8 = Polyphase::<8, 64>::new(0.93, 4.5).unwrap();
        let p16 = Polyphase::<16, 32>::design(0.8, 60.0).unwrap();
        let p24 = Polyphase::<24, 16>::design(0.7, 50.0).unwrap();
        let bits_r = |v: &[f32]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        let bits_c = |v: &[Complex32]| {
            v.iter()
                .flat_map(|x| [x.re.to_bits(), x.im.to_bits()])
                .collect::<Vec<_>>()
        };
        for &d in raster_steps().iter().chain(&[0.0, 1.0, 2.7]) {
            for _ in 0..5 {
                let n = 1 + (rng.next() * 1500.0) as usize;
                let g = Grid::new(20.0 + rng.next() * 30.0, d).unwrap();
                for x in [&xr, &xz] {
                    let (mut a, mut b) = (vec![0f32; n], vec![0f32; n]);
                    p8.at_grid(x, &g, &mut a);
                    plain_grid(&p8, x, &g, &mut b);
                    assert_eq!(bits_r(&a), bits_r(&b), "8 taps, step {d}");
                    p16.at_grid(x, &g, &mut a);
                    plain_grid(&p16, x, &g, &mut b);
                    assert_eq!(bits_r(&a), bits_r(&b), "16 taps, step {d}");
                    p24.at_grid(x, &g, &mut a);
                    plain_grid(&p24, x, &g, &mut b);
                    assert_eq!(bits_r(&a), bits_r(&b), "24 taps, step {d}");
                    CatmullRom.at_grid(x, &g, &mut a);
                    plain_grid(&CatmullRom, x, &g, &mut b);
                    assert_eq!(bits_r(&a), bits_r(&b), "Catmull-Rom, step {d}");
                }
                let (mut a, mut b) = (vec![Complex32::default(); n], vec![Complex32::default(); n]);
                CatmullRom.at_grid(&xc, &g, &mut a);
                plain_grid(&CatmullRom, &xc, &g, &mut b);
                assert_eq!(bits_c(&a), bits_c(&b), "complex Catmull-Rom, step {d}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "outside the slice")]
    fn the_batch_form_refuses_a_window_outside_the_slice() {
        let x = [0f32; 10];
        let mut out = [0f32; 4];
        CatmullRom.at_grid(&x, &Grid::new(0.5, 1.0).unwrap(), &mut out);
    }

    /// The cost of one output of the per-call form and of the batch form at the steps of the picture rows.
    #[test]
    #[ignore = "a measurement: run it alone in a release build"]
    fn the_cost_of_an_output_per_call_and_in_a_batch() {
        use std::hint::black_box;
        use std::time::Instant;
        let mut rng = Lcg(3);
        let xr = noise_real(2000, &mut rng);
        let xc = noise_complex(2000, &mut rng);
        let poly = Reader::new(Polyphase::<8, 64>::new(0.93, 4.5).unwrap());
        let cr = Reader::new(CatmullRom);
        let (rows, n) = (4000usize, 768usize);
        let time = |f: &mut dyn FnMut()| {
            (0..5)
                .map(|_| {
                    let t = Instant::now();
                    for _ in 0..rows {
                        f();
                    }
                    t.elapsed().as_secs_f64() * 1e9 / (rows * n) as f64
                })
                .fold(f64::MAX, f64::min)
        };
        for (name, d) in [("luma 625", 520.0 / 768.0), ("luma 525", 526.56 / 640.0)] {
            let mut out = vec![0f32; n];
            let p0 = 40.3;
            let call = time(&mut || {
                for (k, o) in out.iter_mut().enumerate() {
                    *o = poly.at(black_box(&xr), p0 + (k as f64 + 0.5) * d);
                }
                black_box(&out);
            });
            let g = Grid::new(p0 + 0.5 * d, d).unwrap();
            let batch = time(&mut || {
                poly.read_grid(black_box(&xr), &g, &mut out);
                black_box(&out);
            });
            let given = time(&mut || {
                poly.read_positions(black_box(&xr), |k| p0 + (k as f64 + 0.5) * d, &mut out);
                black_box(&out);
            });
            eprintln!(
                "polyphase 8x64 f32, {name} (step {d:.4}): {call:.2} ns per call, {batch:.2} on a grid, {given:.2} at given positions"
            );
        }
        for (name, d) in [
            ("chroma 625", 260.0 / 768.0),
            ("chroma 525", 263.28 / 640.0),
        ] {
            let mut out = vec![Complex32::default(); n];
            let p0 = 20.1;
            let call = time(&mut || {
                for (k, o) in out.iter_mut().enumerate() {
                    *o = cr.at(black_box(&xc), p0 + (k as f64 + 0.5) * d);
                }
                black_box(&out);
            });
            let g = Grid::new(p0 + 0.5 * d, d).unwrap();
            let batch = time(&mut || {
                cr.read_grid(black_box(&xc), &g, &mut out);
                black_box(&out);
            });
            let given = time(&mut || {
                cr.read_positions(black_box(&xc), |k| p0 + (k as f64 + 0.5) * d, &mut out);
                black_box(&out);
            });
            eprintln!(
                "Catmull-Rom complex, {name} (step {d:.4}): {call:.2} ns per call, {batch:.2} on a grid, {given:.2} at given positions"
            );
        }
        let d = 130.0 / 768.0;
        let mut out = vec![0f32; n];
        let call = time(&mut || {
            for (k, o) in out.iter_mut().enumerate() {
                *o = cr.at(black_box(&xr), 10.0 + (k as f64 + 0.5) * d);
            }
            black_box(&out);
        });
        let g = Grid::new(10.0 + 0.5 * d, d).unwrap();
        let batch = time(&mut || {
            cr.read_grid(black_box(&xr), &g, &mut out);
            black_box(&out);
        });
        let given = time(&mut || {
            cr.read_positions(black_box(&xr), |k| 10.0 + (k as f64 + 0.5) * d, &mut out);
            black_box(&out);
        });
        eprintln!(
            "Catmull-Rom f32, SECAM (step {d:.4}): {call:.2} ns per call, {batch:.2} on a grid, {given:.2} at given positions"
        );
        let mut ring = crate::ring::SampleRing::<f32>::new(4096).unwrap();
        let block = noise_real(1000, &mut rng);
        let t = Instant::now();
        for _ in 0..20_000 {
            ring.extend(black_box(&block));
        }
        let ns = t.elapsed().as_secs_f64() * 1e9 / (20_000.0 * 1000.0);
        let t = Instant::now();
        for _ in 0..20_000 {
            black_box(&block).iter().for_each(|&s| ring.push(s));
        }
        let push = t.elapsed().as_secs_f64() * 1e9 / (20_000.0 * 1000.0);
        eprintln!("SampleRing: {push:.3} ns per sample pushed, {ns:.3} ns per sample by extend");
    }
}
