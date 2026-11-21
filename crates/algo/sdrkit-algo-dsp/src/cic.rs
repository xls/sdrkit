//! Cascaded integrator-comb (CIC) decimators in integer arithmetic, with a register-width check and droop compensation.
//!
//! A CIC of order `N`, decimation `R` and differential delay `M` is `N` integrators at the input rate, a decimation by `R`, then `N`
//! combs `y[n] = x[n] - x[n - M]` at the output rate (E. B. Hogenauer, "An economical class of digital filters for decimation and
//! interpolation", IEEE Trans. ASSP 29(2), 1981). Its response is
//!
//! ```text
//! H(f) = ( sin(pi R M f) / (R M sin(pi f)) )^N        (f in cycles per input sample; unity gain at DC)
//! ```
//!
//! It costs `2N` integer additions per input sample per lane and no multiplies. The registers are `i64` with wrap-around arithmetic:
//! the integrators may wrap, and the output is still exact provided the full output range fits the register. Hogenauer's condition is
//! `B_in + N log2(R M) <= 64` bits. This type quantises each input lane to `S` fractional bits (`|x| <= 1` is full scale, larger
//! values are clipped unless [`Cic::with_full_scale`] sets a larger full scale) and **refuses** any configuration where `(2^S - 1) (R M)^N` does not fit in 63 bits, so it can never wrap
//! (a decimator of order 5 with `S = 14` overflows from `R = 892`, an input of about 42.8 MS/s at 48 kS/s out). [`Cic::new`] picks the
//! largest `S` up to 24 that fits and refuses a configuration that leaves fewer than 12 bits.
//!
//! ```
//! use sdrkit_algo_dsp::cic::Cic;
//! use sdrkit_algo_dsp::Complex32;
//! let mut cic = Cic::<Complex32>::new(4, 16).unwrap();
//! let mut out = Vec::new();
//! cic.process(&vec![Complex32::new(0.5, -0.25); 160], &mut out);
//! assert_eq!(out.len(), 10);
//! assert!((out[9] - Complex32::new(0.5, -0.25)).norm() < 1e-4);
//! ```

use crate::{Complex32, DspError, Param};
use std::f64::consts::PI;

/// The largest input resolution [`Cic::new`] uses, in fractional bits.
pub const MAX_INPUT_BITS: u32 = 24;
/// The smallest input resolution [`Cic::new`] accepts, in fractional bits.
pub const MIN_INPUT_BITS: u32 = 12;

/// A sample a [`Cic`] can process: `f32` (one lane) or [`Complex32`] (two).
pub trait CicSample: Copy + Default + std::fmt::Debug + Send + Sync + 'static {
    /// The number of lanes.
    const LANES: usize;
    /// The lanes, as `[f32; 2]` (the second is 0 for one lane).
    fn lanes(self) -> [f32; 2];
    /// A sample from its lanes.
    fn from_lanes(v: [f32; 2]) -> Self;
}

impl CicSample for f32 {
    const LANES: usize = 1;
    #[inline]
    fn lanes(self) -> [f32; 2] {
        [self, 0.0]
    }
    #[inline]
    fn from_lanes(v: [f32; 2]) -> Self {
        v[0]
    }
}

impl CicSample for Complex32 {
    const LANES: usize = 2;
    #[inline]
    fn lanes(self) -> [f32; 2] {
        [self.re, self.im]
    }
    #[inline]
    fn from_lanes(v: [f32; 2]) -> Self {
        Complex32::new(v[0], v[1])
    }
}

/// The register bits a CIC needs beyond its input: `N log2(R M)` (Hogenauer's growth).
pub fn growth_bits(order: usize, factor: usize, delay: usize) -> f64 {
    order as f64 * ((factor * delay) as f64).log2()
}

/// Whether `(2^input_bits - 1) (R M)^N` fits in 63 bits (exact integer check).
fn fits(order: usize, factor: usize, delay: usize, input_bits: u32) -> bool {
    if input_bits > 62 {
        return false;
    }
    let mut v: u128 = (1u128 << input_bits) - 1;
    for _ in 0..order {
        v = match v.checked_mul((factor * delay) as u128) {
            Some(x) if x <= i64::MAX as u128 => x,
            _ => return false,
        };
    }
    v <= i64::MAX as u128
}

/// The magnitude response of a CIC at `f` cycles per input sample (unity at DC).
pub fn response(order: usize, factor: usize, delay: usize, f: f64) -> f64 {
    let rm = (factor * delay) as f64;
    let s = (PI * f).sin();
    let r = if s.abs() < 1e-15 {
        1.0
    } else {
        (PI * rm * f).sin() / (rm * s)
    };
    r.abs().powi(order as i32)
}

/// A CIC decimator (see the module docs). Generic over `f32` and [`Complex32`].
#[derive(Clone, Debug)]
pub struct Cic<T: CicSample> {
    order: usize,
    factor: usize,
    delay: usize,
    input_bits: u32,
    in_scale: f32,
    in_max: f32,
    out_scale: f32,
    integ: Vec<[i64; 2]>,
    /// For each comb stage, its last `delay` inputs (a ring per stage, `delay` entries each).
    comb: Vec<[i64; 2]>,
    comb_pos: usize,
    phase: usize,
    _t: std::marker::PhantomData<T>,
}

impl<T: CicSample> Cic<T> {
    /// A decimator of order `order` (1 to 10) by `factor` (at least 1), differential delay 1, with the finest input resolution that
    /// fits (up to [`MAX_INPUT_BITS`]). Errors when fewer than [`MIN_INPUT_BITS`] would fit: lower the order, or decimate in two
    /// stages.
    pub fn new(order: usize, factor: usize) -> Result<Self, DspError> {
        Self::with_delay(order, factor, 1)
    }

    /// The same with differential delay `delay` (1 or 2; 2 deepens the nulls at the cost of a wider droop).
    pub fn with_delay(order: usize, factor: usize, delay: usize) -> Result<Self, DspError> {
        Self::check_shape(order, factor, delay)?;
        let bits = (MIN_INPUT_BITS..=MAX_INPUT_BITS)
            .rev()
            .find(|&b| fits(order, factor, delay, b));
        match bits {
            Some(b) => Self::with_input_bits(order, factor, delay, b),
            None => Param::Decimation.conflict(Param::Order),
        }
    }

    /// A decimator with `input_bits` fractional bits of input resolution. Errors when `(2^input_bits - 1) (R M)^N` does not fit in
    /// 63 bits (the register would overflow on a full-scale input), or for a bad order, factor or delay.
    pub fn with_input_bits(
        order: usize,
        factor: usize,
        delay: usize,
        input_bits: u32,
    ) -> Result<Self, DspError> {
        Self::check_shape(order, factor, delay)?;
        if !(1..=62).contains(&input_bits) {
            return Param::InputBits.range(1.0, 62.0, input_bits as f64);
        }
        if !fits(order, factor, delay, input_bits) {
            return Param::InputBits.conflict(Param::Order);
        }
        let in_scale = (1u64 << input_bits) as f32;
        let gain = ((factor * delay) as f64).powi(order as i32) * (1u64 << input_bits) as f64;
        Ok(Cic {
            order,
            factor,
            delay,
            input_bits,
            in_scale,
            in_max: ((1u64 << input_bits) - 1) as f32,
            out_scale: (1.0 / gain) as f32,
            integ: vec![[0; 2]; order],
            comb: vec![[0; 2]; order * delay],
            comb_pos: 0,
            phase: 0,
            _t: std::marker::PhantomData,
        })
    }

    /// The same decimator with `full_scale` as the largest input magnitude, in place of 1. A lane above it is clipped. The input
    /// keeps the same number of bits of resolution, spread over the larger range. The scale costs nothing per sample: it is folded
    /// into the input and output scale factors. Errors when `full_scale` is not finite and above 0.
    ///
    /// ```
    /// use sdrkit_algo_dsp::cic::Cic;
    /// let mut cic = Cic::<f32>::new(4, 4).unwrap().with_full_scale(8.0).unwrap();
    /// let mut out = Vec::new();
    /// cic.process(&[5.0; 40], &mut out);
    /// assert!((out[9] - 5.0).abs() < 1e-3);
    /// ```
    pub fn with_full_scale(mut self, full_scale: f32) -> Result<Self, DspError> {
        if !(full_scale.is_finite() && full_scale > 0.0) {
            return Param::FullScale.positive(full_scale as f64);
        }
        let unit = (1u64 << self.input_bits) as f32;
        let gain = ((self.factor * self.delay) as f64).powi(self.order as i32) * unit as f64;
        self.in_scale = unit / full_scale;
        self.out_scale = (full_scale as f64 / gain) as f32;
        Ok(self)
    }

    fn check_shape(order: usize, factor: usize, delay: usize) -> Result<(), DspError> {
        check_cic_shape(order, factor, delay)
    }

    /// The order `N`.
    pub fn order(&self) -> usize {
        self.order
    }

    /// The decimation factor `R`.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The differential delay `M`.
    pub fn delay(&self) -> usize {
        self.delay
    }

    /// The input resolution in fractional bits.
    pub fn input_bits(&self) -> u32 {
        self.input_bits
    }

    /// The group delay in input samples: `N (R M - 1) / 2`.
    pub fn group_delay(&self) -> f64 {
        self.order as f64 * ((self.factor * self.delay) as f64 - 1.0) / 2.0
    }

    /// The magnitude response at `f` cycles per input sample.
    pub fn response(&self, f: f64) -> f64 {
        response(self.order, self.factor, self.delay, f)
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.integ.iter_mut().for_each(|v| *v = [0; 2]);
        self.comb.iter_mut().for_each(|v| *v = [0; 2]);
        self.comb_pos = 0;
        self.phase = 0;
    }

    #[inline]
    fn quantise(&self, x: f32) -> i64 {
        // `as` saturates and maps NaN to 0.
        (x * self.in_scale).round().clamp(-self.in_max, self.in_max) as i64
    }

    #[inline]
    fn integrate(&mut self, x: T) {
        let l = x.lanes();
        let mut v = [self.quantise(l[0]), self.quantise(l[1])];
        for s in self.integ.iter_mut() {
            s[0] = s[0].wrapping_add(v[0]);
            s[1] = s[1].wrapping_add(v[1]);
            v = *s;
        }
    }

    #[inline]
    fn dump(&mut self) -> T {
        let mut v = self.integ[self.order - 1];
        let m = self.delay;
        for st in 0..self.order {
            let slot = &mut self.comb[st * m + self.comb_pos];
            let old = *slot;
            *slot = v;
            v = [v[0].wrapping_sub(old[0]), v[1].wrapping_sub(old[1])];
        }
        self.comb_pos += 1;
        if self.comb_pos == m {
            self.comb_pos = 0;
        }
        T::from_lanes([v[0] as f32 * self.out_scale, v[1] as f32 * self.out_scale])
    }

    /// Takes one sample; returns an output every `factor` inputs.
    #[inline]
    pub fn push(&mut self, x: T) -> Option<T> {
        self.integrate(x);
        self.phase += 1;
        if self.phase < self.factor {
            return None;
        }
        self.phase = 0;
        Some(self.dump())
    }

    /// Appends one output per `factor` inputs (counted across calls) to `out`.
    pub fn process(&mut self, input: &[T], out: &mut Vec<T>) {
        out.reserve((self.phase + input.len()) / self.factor);
        for &x in input {
            if let Some(y) = self.push(x) {
                out.push(y);
            }
        }
    }
}

/// Refuses a CIC shape outside order 1 to 10, a factor of at least 1 and a delay of 1 or 2.
fn check_cic_shape(order: usize, factor: usize, delay: usize) -> Result<(), DspError> {
    if !(1..=10).contains(&order) {
        return Param::Order.range(1.0, 10.0, order as f64);
    }
    if factor == 0 {
        return Param::Decimation.at_least(1.0, 0.0);
    }
    if !(1..=2).contains(&delay) {
        return Param::Delay.range(1.0, 2.0, delay as f64);
    }
    Ok(())
}

/// Taps of a symmetric FIR at the CIC's output rate that undoes its droop: the least-squares fit (on a dense grid) of `1 / H` over
/// `0..pass` cycles per output sample (`pass` in `(0, 0.5)`), with `num_taps` taps (odd, 3 to 255). Put it after the CIC (it can be
/// folded into the next decimating filter). Errors for bad parameters.
pub fn droop_taps(
    order: usize,
    factor: usize,
    delay: usize,
    num_taps: usize,
    pass: f64,
) -> Result<Vec<f32>, DspError> {
    if !(3..=255).contains(&num_taps) {
        return Param::Taps.range(3.0, 255.0, num_taps as f64);
    }
    if num_taps.is_multiple_of(2) {
        return Param::Taps.not_allowed(num_taps as f64);
    }
    if !(pass > 0.0 && pass < 0.5) {
        return Param::Pass.range(0.0, 0.5, pass);
    }
    check_cic_shape(order, factor, delay)?;
    let k = num_taps / 2;
    let m = k + 1;
    // Basis: c_0 + sum_j 2 c_j cos(2 pi j f). Normal equations A^T A c = A^T b, with a small ridge for conditioning.
    let grid = 64 * m;
    let mut ata = vec![0f64; m * m];
    let mut atb = vec![0f64; m];
    let mut row = vec![0f64; m];
    for g in 0..=grid {
        let f = pass * g as f64 / grid as f64;
        let h = response(order, factor, delay, f / factor as f64);
        let target = 1.0 / h;
        for (j, r) in row.iter_mut().enumerate() {
            *r = if j == 0 {
                1.0
            } else {
                2.0 * (2.0 * PI * j as f64 * f).cos()
            };
        }
        for a in 0..m {
            atb[a] += row[a] * target;
            for b in 0..m {
                ata[a * m + b] += row[a] * row[b];
            }
        }
    }
    let ridge = 1e-9 * (0..m).map(|i| ata[i * m + i]).fold(0.0, f64::max);
    for i in 0..m {
        ata[i * m + i] += ridge;
    }
    let c = solve(&mut ata, &mut atb, m)?;
    let mut h = vec![0f32; num_taps];
    for j in 0..m {
        h[k + j] = c[j] as f32;
        h[k - j] = c[j] as f32;
    }
    Ok(h)
}

/// Solves the `m x m` system `a x = b` by Gaussian elimination with partial pivoting.
fn solve(a: &mut [f64], b: &mut [f64], m: usize) -> Result<Vec<f64>, DspError> {
    for col in 0..m {
        let piv = (col..m)
            .max_by(|&i, &j| a[i * m + col].abs().total_cmp(&a[j * m + col].abs()))
            .unwrap_or(col);
        if a[piv * m + col].abs() < 1e-300 {
            return Param::Taps.degenerate();
        }
        if piv != col {
            for j in 0..m {
                a.swap(piv * m + j, col * m + j);
            }
            b.swap(piv, col);
        }
        for i in col + 1..m {
            let f = a[i * m + col] / a[col * m + col];
            for j in col..m {
                a[i * m + j] -= f * a[col * m + j];
            }
            b[i] -= f * b[col];
        }
    }
    let mut x = vec![0f64; m];
    for i in (0..m).rev() {
        let s: f64 = (i + 1..m).map(|j| a[i * m + j] * x[j]).sum();
        x[i] = (b[i] - s) / a[i * m + i];
    }
    Ok(x)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn the_register_check_refuses_what_would_overflow() {
        // Order 5, 14 input bits: (2^14 - 1) 891^5 = 9.1999e18 fits in 2^63 - 1 = 9.2234e18; with 892^5 it is 9.2516e18 and would wrap.
        assert!(Cic::<Complex32>::with_input_bits(5, 891, 1, 14).is_ok());
        assert!(Cic::<Complex32>::with_input_bits(5, 892, 1, 14).is_err());
        // The automatic choice reduces the resolution instead, down to 12 bits.
        assert_eq!(Cic::<Complex32>::new(5, 891).unwrap().input_bits(), 14);
        assert_eq!(Cic::<Complex32>::new(5, 1000).unwrap().input_bits(), 13);
        assert!(Cic::<Complex32>::new(5, 1200).is_err());
        assert_eq!(Cic::<f32>::new(3, 8).unwrap().input_bits(), MAX_INPUT_BITS);
        assert!(Cic::<f32>::new(0, 8).is_err());
        assert!(Cic::<f32>::new(3, 0).is_err());
        assert!(Cic::<f32>::with_delay(3, 8, 3).is_err());
        assert!((growth_bits(5, 875, 1) - 5.0 * 875f64.log2()).abs() < 1e-12);
    }

    #[test]
    fn full_scale_input_never_wraps() {
        // The worst case: full-scale DC of alternating sign blocks, at the largest factor that fits.
        let mut c = Cic::<f32>::new(5, 1100).unwrap();
        let mut out = Vec::new();
        for k in 0..20 {
            let v = if k % 2 == 0 { 1.0 } else { -1.0 };
            c.process(&vec![v; 1100 * 6], &mut out);
            let last = *out.last().unwrap();
            assert!((last - v).abs() < 1e-3, "{k}: {last}");
        }
        // Beyond full scale is clipped, not wrapped.
        c.reset();
        out.clear();
        c.process(&vec![7.0; 1100 * 6], &mut out);
        assert!((out.last().unwrap() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn the_response_is_hogenauers_formula() {
        for (order, factor, delay) in [(3, 8, 1), (4, 16, 1), (5, 10, 2)] {
            let mut c = Cic::<Complex32>::with_delay(order, factor, delay).unwrap();
            for f_out in [0.0, 0.05, 0.13, 0.27, 0.41] {
                let f = f_out / factor as f64;
                c.reset();
                let x = ctone(factor * 4000, f, 0.9);
                let mut y = Vec::new();
                c.process(&x, &mut y);
                let got = camp(&y[200..], f_out) / 0.9;
                let want = response(order, factor, delay, f);
                assert!(
                    (got - want).abs() < 2e-4,
                    "{order} {factor} {delay} {f_out}: {got} vs {want}"
                );
            }
            // Nulls at the multiples of the output rate / M.
            assert!(response(order, factor, delay, 1.0 / (factor * delay) as f64) < 1e-12);
        }
    }

    #[test]
    fn a_larger_full_scale_passes_peaks_above_1() {
        let x: Vec<Complex32> = (0..400)
            .map(|i| Complex32::from_polar(5.0, i as f32 * 0.01))
            .collect();
        let mut clipped = Cic::<Complex32>::new(4, 8).unwrap();
        let mut scaled = Cic::<Complex32>::new(4, 8)
            .unwrap()
            .with_full_scale(8.0)
            .unwrap();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        clipped.process(&x, &mut a);
        scaled.process(&x, &mut b);
        // Full scale 1 clips each lane to 1, so the magnitude stays below 1.5.
        assert!(a[40].norm() < 1.5);
        assert!((b[40].norm() - 5.0).abs() < 0.01);
        // The same as scaling the input down and the output up by a power of two.
        let mut manual = Cic::<Complex32>::new(4, 8).unwrap();
        let small: Vec<Complex32> = x.iter().map(|v| v * 0.125).collect();
        let mut c = Vec::new();
        manual.process(&small, &mut c);
        for (p, q) in b.iter().zip(&c) {
            assert_eq!(*p, q * 8.0);
        }
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(Cic::<f32>::new(4, 8).unwrap().with_full_scale(bad).is_err());
        }
    }

    #[test]
    fn push_and_process_agree_across_cuts() {
        let x = ctone(5000, 0.003, 0.7);
        let mut a = Cic::<Complex32>::new(4, 7).unwrap();
        let mut ya = Vec::new();
        a.process(&x, &mut ya);
        let mut b = Cic::<Complex32>::new(4, 7).unwrap();
        let mut yb = Vec::new();
        for c in x.chunks(13) {
            for &v in c {
                if let Some(y) = b.push(v) {
                    yb.push(y);
                }
            }
        }
        assert_eq!(ya, yb);
        assert_eq!(ya.len(), 5000 / 7);
    }

    #[test]
    fn droop_compensation_flattens_the_passband() {
        let (order, factor) = (4, 16);
        let h = droop_taps(order, factor, 1, 15, 0.25).unwrap();
        let mut worst_before = 0.0f64;
        let mut worst_after = 0.0f64;
        for i in 0..=100 {
            let f = 0.25 * i as f64 / 100.0;
            let c = response(order, factor, 1, f / factor as f64);
            let comp = crate::fir::response(&h, f).norm();
            worst_before = worst_before.max(db(c).abs());
            worst_after = worst_after.max(db(c * comp).abs());
        }
        assert!(worst_before > 3.0, "{worst_before}");
        assert!(worst_after < 0.05, "{worst_after}");
        assert!(droop_taps(order, factor, 1, 14, 0.25).is_err());
        assert!(droop_taps(order, factor, 1, 15, 0.6).is_err());
    }
}
