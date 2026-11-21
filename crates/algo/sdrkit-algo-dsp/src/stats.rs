//! Statistics of lists and running sums: the quantile of `f64` values, a centred moving average, the M-th power concentration of
//! phases, a sliding window sum that does not drift, and the second and fourth moments and plain kurtosis of complex samples.

use crate::{Complex32, DspError, Param};
use num_complex::Complex64;
use std::ops::{AddAssign, SubAssign};

/// The `p` quantile (`0 <= p <= 1`) of `values` by the nearest rank: the value of rank `round(p (n - 1))` in increasing order. This is
/// the rule of `crate::spectrum::percentile`, for `f64` values (times of day, for example, need the precision). `scratch` holds a
/// copy, so nothing is allocated once it has grown. NaN sorts above every number. Returns `None` for an empty list or a `p` outside
/// `[0, 1]`.
///
/// ```
/// use sdrkit_algo_dsp::stats::quantile_f64;
/// let mut scratch = Vec::new();
/// let v = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0];
/// assert_eq!(quantile_f64(&v, 0.5, &mut scratch), Some(4.0));
/// assert_eq!(quantile_f64(&v, 1.0, &mut scratch), Some(9.0));
/// ```
pub fn quantile_f64(values: &[f64], p: f64, scratch: &mut Vec<f64>) -> Option<f64> {
    if values.is_empty() || !(0.0..=1.0).contains(&p) {
        return None;
    }
    scratch.clear();
    scratch.extend_from_slice(values);
    let k = ((p * (values.len() - 1) as f64).round() as usize).min(values.len() - 1);
    let (_, v, _) = scratch.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    Some(*v)
}

/// A real value that [`centred_mean`] averages: `f32` or `f64`.
pub trait Real: Copy {
    /// The value as `f64`.
    fn to_f64(self) -> f64;
    /// The value of type `Self` nearest to `v`.
    fn from_f64(v: f64) -> Self;
}

impl Real for f32 {
    #[inline]
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        v as f32
    }
}

impl Real for f64 {
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn from_f64(v: f64) -> Self {
        v
    }
}

/// The centred moving average of `x` over `2 half + 1` values, written to `out` (cleared first, one value for each of `x`). Near
/// the ends the window is cut short, and each value is the mean of the values the window still covers. `half = 0` copies `x`.
/// The sum is kept in `f64`.
/// Nothing is allocated once `out` has grown.
///
/// ```
/// use sdrkit_algo_dsp::stats::centred_mean;
/// let mut out = Vec::new();
/// centred_mean(&[0.0, 3.0, 0.0, 6.0], 1, &mut out);
/// assert_eq!(out, [1.5, 1.0, 3.0, 3.0]);
/// ```
pub fn centred_mean<T: Real>(x: &[T], half: usize, out: &mut Vec<T>) {
    out.clear();
    let n = x.len();
    let half = half.min(n);
    // The window of value i is a..b. Start with the part of the first window that lies after it.
    let mut sum: f64 = x[..half].iter().map(|v| v.to_f64()).sum();
    let (mut a, mut b) = (0usize, half);
    for i in 0..n {
        let hi = (i + half + 1).min(n);
        while b < hi {
            sum += x[b].to_f64();
            b += 1;
        }
        let lo = i.saturating_sub(half);
        while a < lo {
            sum -= x[a].to_f64();
            a += 1;
        }
        out.push(T::from_f64(sum / (b - a) as f64));
    }
}

/// The M-th power concentration of the phases of `values`: the magnitude and angle of the mean of `(z / |z|)^m`. A signal of
/// `m`-ary phase steps (M-PSK) has a magnitude near 1, noise has a magnitude near 0, and the angle is `m` times the phase
/// offset. The magnitude is in `0..=1` and the angle in radians in `-pi..=pi`. Values that are zero or not finite do not count.
/// Returns `None` when no value counts.
///
/// ```
/// use num_complex::Complex64;
/// use sdrkit_algo_dsp::stats::power_concentration;
/// // QPSK at 45 degrees: the fourth power of every point is -1.
/// let qpsk = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].map(|(a, b)| Complex64::new(a, b));
/// let (mag, angle) = power_concentration(qpsk, 4).unwrap();
/// assert!((mag - 1.0).abs() < 1e-12 && (angle.abs() - std::f64::consts::PI).abs() < 1e-9);
/// ```
pub fn power_concentration(
    values: impl IntoIterator<Item = Complex64>,
    m: u32,
) -> Option<(f64, f64)> {
    let (mut re, mut im) = (0.0, 0.0);
    let mut n = 0u64;
    for z in values {
        if !(z.re.is_finite() && z.im.is_finite()) || (z.re == 0.0 && z.im == 0.0) {
            continue;
        }
        let ph = z.im.atan2(z.re) * m as f64;
        re += ph.cos();
        im += ph.sin();
        n += 1;
    }
    if n == 0 {
        return None;
    }
    Some(((re * re + im * im).sqrt() / n as f64, im.atan2(re)))
}

/// A value [`WindowSum`] can add up, with the wider type its sum is kept in.
pub trait Summable: Copy + Default {
    /// The type of the sum (`f64` or `Complex64`).
    type Acc: Copy + Default + AddAssign + SubAssign + PartialEq + std::fmt::Debug;
    /// The value in the wider type. A value that is not finite returns 0.
    fn widen(self) -> Self::Acc;
}

impl Summable for f32 {
    type Acc = f64;
    #[inline]
    fn widen(self) -> f64 {
        if self.is_finite() {
            self as f64
        } else {
            0.0
        }
    }
}

impl Summable for f64 {
    type Acc = f64;
    #[inline]
    fn widen(self) -> f64 {
        if self.is_finite() {
            self
        } else {
            0.0
        }
    }
}

impl Summable for Complex32 {
    type Acc = Complex64;
    #[inline]
    fn widen(self) -> Complex64 {
        if self.re.is_finite() && self.im.is_finite() {
            Complex64::new(self.re as f64, self.im as f64)
        } else {
            Complex64::default()
        }
    }
}

impl Summable for Complex64 {
    type Acc = Complex64;
    #[inline]
    fn widen(self) -> Complex64 {
        if self.re.is_finite() && self.im.is_finite() {
            self
        } else {
            Complex64::default()
        }
    }
}

/// The sum of the newest `len` values of a stream, updated in constant time per value.
///
/// The sum is kept in `f64` (or `Complex64`). It adds the new value and takes away the value that leaves, and it is summed again
/// exactly every `4 len` values, so rounding cannot build up over a long stream. A value that is not finite counts as 0. Before
/// `len` values have come, the sum covers the values so far. Nothing is allocated after [`WindowSum::new`].
///
/// ```
/// use sdrkit_algo_dsp::stats::WindowSum;
/// let mut w = WindowSum::<f32>::new(3).unwrap();
/// let sums: Vec<f64> = [1.0, 2.0, 3.0, 4.0, 5.0].iter().map(|&x| w.push(x)).collect();
/// assert_eq!(sums, [1.0, 3.0, 6.0, 9.0, 12.0]);
/// ```
#[derive(Clone, Debug)]
pub struct WindowSum<T: Summable> {
    ring: Vec<T::Acc>,
    pos: usize,
    seen: u64,
    sum: T::Acc,
    since: usize,
}

impl<T: Summable> WindowSum<T> {
    /// A window of `len` values. Errors for a length of 0.
    pub fn new(len: usize) -> Result<Self, DspError> {
        if len == 0 {
            return Param::Window.at_least(1.0, 0.0);
        }
        Ok(WindowSum {
            ring: vec![T::Acc::default(); len],
            pos: 0,
            seen: 0,
            sum: T::Acc::default(),
            since: 0,
        })
    }

    /// The window length.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Whether the window has a length of 0 (never: [`WindowSum::new`] refuses it).
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Whether `len` values have come since the start or the last [`reset`](Self::reset).
    pub fn is_full(&self) -> bool {
        self.seen >= self.ring.len() as u64
    }

    /// Values taken since the start or the last reset.
    pub fn count(&self) -> u64 {
        self.seen
    }

    /// The sum of the newest `len` values.
    pub fn sum(&self) -> T::Acc {
        self.sum
    }

    /// Forgets every value.
    pub fn reset(&mut self) {
        self.ring.fill(T::Acc::default());
        self.pos = 0;
        self.seen = 0;
        self.sum = T::Acc::default();
        self.since = 0;
    }

    /// Takes one value and returns the sum of the newest `len` values.
    #[inline]
    pub fn push(&mut self, x: T) -> T::Acc {
        let v = x.widen();
        let old = self.ring[self.pos];
        self.ring[self.pos] = v;
        self.pos += 1;
        if self.pos == self.ring.len() {
            self.pos = 0;
        }
        self.seen += 1;
        self.sum += v;
        self.sum -= old;
        self.since += 1;
        if self.since >= 4 * self.ring.len() {
            self.since = 0;
            let mut s = T::Acc::default();
            for &r in &self.ring {
                s += r;
            }
            self.sum = s;
        }
        self.sum
    }

    /// Takes every value of `x`, calling `f` with each sum.
    pub fn process(&mut self, x: &[T], mut f: impl FnMut(T::Acc)) {
        for &v in x {
            f(self.push(v));
        }
    }
}

/// The second and fourth moments about zero of complex samples given by their powers `p = |x|^2`: `(E|x|^2, E|x|^4)`, the
/// means of `p` and `p^2` summed in `f64` in order. `None` for no powers.
pub fn power_moments(powers: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let (mut m2, mut m4, mut n) = (0.0, 0.0, 0usize);
    for p in powers {
        m2 += p;
        m4 += p * p;
        n += 1;
    }
    (n > 0).then(|| (m2 / n as f64, m4 / n as f64))
}

/// The plain kurtosis (not the excess) of complex samples about zero, `E|x|^4 / (E|x|^2)^2`: 1 for a constant envelope (PSK),
/// 1.32 for 16-QAM, 2 for circular complex Gaussian noise. `None` for no samples or no power.
pub fn plain_kurtosis_iq(x: &[Complex32]) -> Option<f64> {
    let (m2, m4) = power_moments(x.iter().map(|v| v.norm_sqr() as f64))?;
    (m2 > 0.0).then(|| m4 / (m2 * m2))
}

/// The plain kurtosis (not the excess) of the signal part of complex samples with moments `m2 = E|x|^2` and `m4 = E|x|^4`
/// (from [`power_moments`]) that hold the signal plus independent circular complex Gaussian noise of power `noise`:
/// `(m4 - 4 S N - 2 N^2) / S^2` with the signal power `S = m2 - N`, held at least `1e-3 m2`.
pub fn plain_kurtosis_less_noise(m2: f64, m4: f64, noise: f64) -> f64 {
    let s = (m2 - noise).max(m2 * 1e-3);
    (m4 - 4.0 * s * noise - 2.0 * noise * noise) / (s * s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;

    #[test]
    fn the_centred_mean_cuts_the_window_at_the_ends() {
        let mut out = vec![9.0; 7];
        centred_mean(&[0.0, 3.0, 0.0], 1, &mut out);
        assert_eq!(out, [1.5, 1.0, 1.5]);
        let x: Vec<f64> = (0..20).map(|i| ((i * 7) % 5) as f64).collect();
        for half in [0usize, 1, 3, 9, 25] {
            centred_mean(&x, half, &mut out);
            assert_eq!(out.len(), x.len());
            for (i, v) in out.iter().enumerate() {
                let (a, b) = (i.saturating_sub(half), (i + half + 1).min(x.len()));
                let want = x[a..b].iter().sum::<f64>() / (b - a) as f64;
                assert!((v - want).abs() < 1e-12, "half {half} at {i}");
            }
        }
        centred_mean(&[], 2, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn the_centred_mean_of_f32_values_is_the_f64_mean_rounded() {
        let x: Vec<f32> = (0..50)
            .map(|i| ((i * 13) % 7) as f32 * 0.37 - 1.0)
            .collect();
        let wide: Vec<f64> = x.iter().map(|&v| f64::from(v)).collect();
        let (mut out, mut want) = (Vec::new(), Vec::new());
        for half in [0usize, 1, 2, 6, 60] {
            centred_mean(&x, half, &mut out);
            centred_mean(&wide, half, &mut want);
            let rounded: Vec<f32> = want.iter().map(|&v| v as f32).collect();
            assert_eq!(out, rounded, "half {half}");
        }
    }

    #[test]
    fn the_power_concentration_finds_psk_and_its_offset() {
        let pts = |m: usize, off: f64| -> Vec<Complex64> {
            (0..64)
                .map(|k| {
                    Complex64::from_polar(
                        1.0 + k as f64,
                        off + std::f64::consts::TAU * ((k * 5) % m) as f64 / m as f64,
                    )
                })
                .collect()
        };
        // BPSK, QPSK and 8PSK turned by 0.1 rad: the M-th power is fully concentrated at M times the offset.
        for m in [2u32, 4, 8] {
            let (mag, ang) = power_concentration(pts(m as usize, 0.1), m).unwrap();
            assert!(
                (mag - 1.0).abs() < 1e-9 && (ang - 0.1 * m as f64).abs() < 1e-9,
                "{m}"
            );
        }
        // QPSK seen in the second power: the phases cancel.
        let (mag, _) = power_concentration(pts(4, 0.0), 2).unwrap();
        assert!(mag < 1e-9);
        assert_eq!(
            power_concentration([Complex64::new(0.0, 0.0), Complex64::new(f64::NAN, 1.0)], 4),
            None
        );
    }

    #[test]
    fn quantiles_by_the_nearest_rank() {
        let mut s = Vec::new();
        // Nine values sorted: 1 1 2 3 4 5 5 6 9. Ranks round(p * 8).
        let v = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0];
        assert_eq!(quantile_f64(&v, 0.0, &mut s), Some(1.0));
        assert_eq!(quantile_f64(&v, 0.25, &mut s), Some(2.0));
        assert_eq!(quantile_f64(&v, 0.5, &mut s), Some(4.0));
        assert_eq!(quantile_f64(&v, 0.75, &mut s), Some(5.0));
        assert_eq!(quantile_f64(&v, 0.9, &mut s), Some(6.0));
        assert_eq!(quantile_f64(&v, 1.0, &mut s), Some(9.0));
        assert_eq!(quantile_f64(&[], 0.5, &mut s), None);
        assert_eq!(quantile_f64(&v, 1.5, &mut s), None);
        assert_eq!(quantile_f64(&v, f64::NAN, &mut s), None);
        // The same rule as the f32 percentile of the spectrum module.
        let f: Vec<f32> = v.iter().map(|&x| x as f32).collect();
        let mut fs = Vec::new();
        for p in [0.1, 0.33, 0.5, 0.84] {
            assert_eq!(
                quantile_f64(&v, p, &mut s).unwrap() as f32,
                crate::spectrum::percentile(&f, p, &mut fs)
            );
        }
        // Times of day keep their precision: 1e9 s plus microseconds.
        let t: Vec<f64> = (0..5).map(|i| 1.0e9 + i as f64 * 1e-6).collect();
        assert_eq!(quantile_f64(&t, 0.5, &mut s), Some(1.0e9 + 2e-6));
        // NaN sorts last.
        assert_eq!(quantile_f64(&[f64::NAN, 1.0, 2.0], 0.5, &mut s), Some(2.0));
    }

    #[test]
    fn window_sums_match_the_plain_sum_and_do_not_drift() {
        let mut rng = Rng(7);
        let x: Vec<f32> = (0..200_000)
            .map(|i| (rng.uniform() * 1e3) as f32 + if i % 1000 == 0 { 1e7 } else { 0.0 })
            .collect();
        let len = 37;
        let mut w = WindowSum::<f32>::new(len).unwrap();
        for (i, &v) in x.iter().enumerate() {
            let s = w.push(v);
            if i % 997 == 0 || i + 1 == x.len() {
                let from = (i + 1).saturating_sub(len);
                let want: f64 = x[from..=i].iter().map(|&v| v as f64).sum();
                assert!(
                    (s - want).abs() <= 1e-6 * want.abs().max(1.0),
                    "{i}: {s} {want}"
                );
            }
        }
        assert!(w.is_full());
        // Complex values, a bad sample counted as 0, and reset.
        let mut c = WindowSum::<Complex32>::new(2).unwrap();
        c.push(Complex32::new(1.0, 2.0));
        c.push(Complex32::new(f32::NAN, 0.0));
        assert_eq!(c.push(Complex32::new(3.0, -1.0)), Complex64::new(3.0, -1.0));
        c.reset();
        assert_eq!(c.sum(), Complex64::default());
        assert!(!c.is_full());
        assert!(WindowSum::<f64>::new(0).is_err());
    }

    #[test]
    fn the_plain_kurtosis_of_known_sets() {
        // QPSK: 1. 16-QAM on the odd grid: E|a|^2 = 10, E|a|^4 = 132, so 1.32.
        let qpsk = [
            Complex32::new(1.0, 1.0),
            Complex32::new(-1.0, 1.0),
            Complex32::new(-1.0, -1.0),
            Complex32::new(1.0, -1.0),
        ];
        assert_eq!(plain_kurtosis_iq(&qpsk), Some(1.0));
        let lv = [-3.0f32, -1.0, 1.0, 3.0];
        let qam: Vec<Complex32> = lv
            .iter()
            .flat_map(|&i| lv.iter().map(move |&q| Complex32::new(i, q)))
            .collect();
        assert!((plain_kurtosis_iq(&qam).unwrap() - 1.32).abs() < 1e-12);
        // Powers 1 and 3: m2 = 2, m4 = 5.
        assert_eq!(power_moments([1.0, 3.0]), Some((2.0, 5.0)));
        assert_eq!(power_moments(std::iter::empty()), None);
        assert_eq!(plain_kurtosis_iq(&[]), None);
        assert_eq!(plain_kurtosis_iq(&[Complex32::default(); 3]), None);
        // Circular complex Gaussian noise: 2.
        let mut r = Rng(7);
        let g: Vec<Complex32> = (0..200_000)
            .map(|_| Complex32::new(r.gauss() as f32, r.gauss() as f32))
            .collect();
        let k = plain_kurtosis_iq(&g).unwrap();
        assert!((k - 2.0).abs() < 0.03, "{k}");
    }

    #[test]
    fn the_noise_comes_out_of_the_plain_kurtosis() {
        // A constant envelope of power 1 in noise of power N: m2 = 1 + N, m4 = 1 + 4 N + 2 N^2, the signal reads 1.
        for n in [0.0, 0.1, 1.0, 3.0] {
            let k = plain_kurtosis_less_noise(1.0 + n, 1.0 + 4.0 * n + 2.0 * n * n, n);
            assert!((k - 1.0).abs() < 1e-12, "{n} {k}");
        }
        // Noise alone: the signal power is held at 1e-3 m2.
        let k = plain_kurtosis_less_noise(1.0, 2.0, 1.0);
        assert!((k - (2.0 - 4e-3 - 2.0) / 1e-6).abs() < 1e-6, "{k}");
    }
}
