//! Dot products of real taps with real and two-lane samples on NEON (ARM64), and the filter loops made of them.
//!
//! A two-lane sample is two `f32` values (`[re, im]` or `[l, r]`); a slice of them is given flat, two values per sample.
//! Every kernel keeps eight partial sums of four values and adds them in a fixed order, so a result depends only on the
//! taps and the samples, not on where the slices lie in memory or on how many outputs one call makes. All loads are
//! unaligned.

use std::arch::aarch64::{
    float32x4_t, vadd_f32, vaddq_f32, vaddvq_f32, vdupq_n_f32, vfmaq_f32, vget_high_f32,
    vget_lane_f32, vget_low_f32, vgetq_lane_f32, vld1q_f32, vld2q_f32, vmulq_f32, vsubq_f32,
    vzip1q_f32, vzip2q_f32,
};

/// The four values of `x` from index `i`.
///
/// # Safety
/// `i + 4 <= x.len()`.
#[inline(always)]
unsafe fn load(x: &[f32], i: usize) -> float32x4_t {
    debug_assert!(i + 4 <= x.len());
    // SAFETY: the caller keeps `i + 4 <= x.len()`, so the four values are inside `x`; the load needs no alignment.
    unsafe { vld1q_f32(x.as_ptr().add(i)) }
}

/// The sum of eight partial sums, in a fixed order.
#[target_feature(enable = "neon")]
#[inline]
fn sum8(a: &[float32x4_t; 8]) -> float32x4_t {
    vaddq_f32(
        vaddq_f32(vaddq_f32(a[0], a[4]), vaddq_f32(a[2], a[6])),
        vaddq_f32(vaddq_f32(a[1], a[5]), vaddq_f32(a[3], a[7])),
    )
}

/// `O` dot products of the same `n` taps with `O` slices of real samples. `quad(q)` returns taps `4q..4q + 4` and `tap(k)`
/// returns tap `k`. Each slice of `x` holds at least `n` values.
#[target_feature(enable = "neon")]
#[inline]
#[allow(clippy::needless_range_loop)]
fn real<const O: usize>(
    n: usize,
    quad: impl Fn(usize) -> float32x4_t,
    tap: impl Fn(usize) -> f32,
    x: [&[f32]; O],
) -> [f32; O] {
    assert!(x.iter().all(|x| x.len() >= n));
    let mut acc = [[vdupq_n_f32(0.0); 8]; O];
    let quads = n / 4;
    let blocks = quads / 8;
    macro_rules! step {
        ($q:expr, $j:expr) => {{
            let t = quad($q);
            for o in 0..O {
                // SAFETY: `$q < n / 4`, so `4 * $q + 4 <= n`, and `n <= x[o].len()` (asserted above).
                let v = unsafe { load(x[o], 4 * $q) };
                acc[o][$j] = vfmaq_f32(acc[o][$j], t, v);
            }
        }};
    }
    for b in 0..blocks {
        for j in 0..8 {
            step!(8 * b + j, j);
        }
    }
    for j in 0..8 {
        if 8 * blocks + j < quads {
            step!(8 * blocks + j, j);
        }
    }
    let mut y = [0.0f32; O];
    for o in 0..O {
        let mut s = vaddvq_f32(sum8(&acc[o]));
        for k in 4 * quads..n {
            s = tap(k).mul_add(x[o][k], s);
        }
        y[o] = s;
    }
    y
}

/// `O` dot products of the same `n` taps with `O` flat slices of two-lane samples, each as `[first lane, second lane]`.
/// `quad(q)` returns taps `4q..4q + 4` and `tap(k)` returns tap `k`. Each slice of `x` holds at least `2 n` values.
#[target_feature(enable = "neon")]
#[inline]
fn pairs<const O: usize>(
    n: usize,
    quad: impl Fn(usize) -> float32x4_t,
    tap: impl Fn(usize) -> f32,
    x: [&[f32]; O],
) -> [[f32; 2]; O] {
    assert!(x.iter().all(|x| x.len() >= 2 * n));
    let mut acc = [[vdupq_n_f32(0.0); 8]; O];
    let quads = n / 4;
    let blocks = quads / 4;
    macro_rules! step {
        ($q:expr, $j:expr) => {{
            // Each tap twice, once per lane: [t0, t0, t1, t1] and [t2, t2, t3, t3].
            let t = quad($q);
            let (lo, hi) = (vzip1q_f32(t, t), vzip2q_f32(t, t));
            for o in 0..O {
                // SAFETY: `$q < n / 4`, so `8 * $q + 8 <= 2 * n`, and `2 * n <= x[o].len()` (asserted above).
                let (v0, v1) = unsafe { (load(x[o], 8 * $q), load(x[o], 8 * $q + 4)) };
                acc[o][2 * $j] = vfmaq_f32(acc[o][2 * $j], lo, v0);
                acc[o][2 * $j + 1] = vfmaq_f32(acc[o][2 * $j + 1], hi, v1);
            }
        }};
    }
    for b in 0..blocks {
        for j in 0..4 {
            step!(4 * b + j, j);
        }
    }
    for j in 0..4 {
        if 4 * blocks + j < quads {
            step!(4 * blocks + j, j);
        }
    }
    let mut y = [[0.0f32; 2]; O];
    for o in 0..O {
        // The partial sums hold [first, second, first, second].
        let s = sum8(&acc[o]);
        let s = vadd_f32(vget_low_f32(s), vget_high_f32(s));
        let (mut a, mut b) = (vget_lane_f32::<0>(s), vget_lane_f32::<1>(s));
        for k in 4 * quads..n {
            let t = tap(k);
            a = t.mul_add(x[o][2 * k], a);
            b = t.mul_add(x[o][2 * k + 1], b);
        }
        y[o] = [a, b];
    }
    y
}

/// The tap readers of [`real`] and [`pairs`] for the taps `t`.
macro_rules! plain_taps {
    ($t:ident) => {
        (
            // SAFETY: `real` and `pairs` ask for `q < n / 4` only, and `n <= $t.len()`.
            |q: usize| unsafe { load($t, 4 * q) },
            |k: usize| $t[k],
        )
    };
}

/// The tap readers of [`real`] and [`pairs`] for the taps `a + (b - a) mu`.
macro_rules! blend_taps {
    ($a:ident, $b:ident, $mu:ident) => {{
        let m = vdupq_n_f32($mu);
        (
            move |q: usize| {
                // SAFETY: `real` and `pairs` ask for `q < n / 4` only, and `n <= $a.len()` and `n <= $b.len()`.
                let (ta, tb) = unsafe { (load($a, 4 * q), load($b, 4 * q)) };
                vfmaq_f32(ta, vsubq_f32(tb, ta), m)
            },
            |k: usize| ($b[k] - $a[k]).mul_add($mu, $a[k]),
        )
    }};
}

#[target_feature(enable = "neon")]
#[inline]
fn dot_real_neon(taps: &[f32], x: &[f32]) -> f32 {
    let n = taps.len().min(x.len());
    let (quad, tap) = plain_taps!(taps);
    real(n, quad, tap, [x])[0]
}

#[target_feature(enable = "neon")]
#[inline]
fn dot_pairs_neon(taps: &[f32], x: &[f32]) -> [f32; 2] {
    let n = taps.len().min(x.len() / 2);
    let (quad, tap) = plain_taps!(taps);
    pairs(n, quad, tap, [x])[0]
}

#[target_feature(enable = "neon")]
#[inline]
fn dot_real_blend_neon(a: &[f32], b: &[f32], mu: f32, x: &[f32]) -> f32 {
    let n = a.len().min(b.len()).min(x.len());
    let (quad, tap) = blend_taps!(a, b, mu);
    real(n, quad, tap, [x])[0]
}

#[target_feature(enable = "neon")]
#[inline]
fn dot_pairs_blend_neon(a: &[f32], b: &[f32], mu: f32, x: &[f32]) -> [f32; 2] {
    let n = a.len().min(b.len()).min(x.len() / 2);
    let (quad, tap) = blend_taps!(a, b, mu);
    pairs(n, quad, tap, [x])[0]
}

#[target_feature(enable = "neon")]
#[inline]
fn fir_real_neon(taps: &[f32], x: &[f32], stride: usize, out: &mut [f32]) {
    let n = taps.len();
    let (quad, tap) = plain_taps!(taps);
    let (two, one) = out.as_chunks_mut::<2>();
    for (i, o) in two.iter_mut().enumerate() {
        let at = 2 * i * stride;
        *o = real(
            n,
            quad,
            tap,
            [&x[at..at + n], &x[at + stride..at + stride + n]],
        );
    }
    if let Some(o) = one.first_mut() {
        let at = 2 * two.len() * stride;
        *o = real(n, quad, tap, [&x[at..at + n]])[0];
    }
}

#[target_feature(enable = "neon")]
#[inline]
fn fir_pairs_neon(taps: &[f32], x: &[f32], stride: usize, out: &mut [f32]) {
    let n = taps.len();
    let (quad, tap) = plain_taps!(taps);
    let (two, one) = out.as_chunks_mut::<4>();
    for (i, o) in two.iter_mut().enumerate() {
        let at = 4 * i * stride;
        let next = at + 2 * stride;
        let y = pairs(n, quad, tap, [&x[at..at + 2 * n], &x[next..next + 2 * n]]);
        *o = [y[0][0], y[0][1], y[1][0], y[1][1]];
    }
    if let Some(o) = one.as_chunks_mut::<2>().0.first_mut() {
        let at = 4 * two.len() * stride;
        *o = pairs(n, quad, tap, [&x[at..at + 2 * n]])[0];
    }
}

/// The sums of [`crate::sample::dot2`] for the taps `a[k] + d[k] * f`, made with the same operations in the same order, so
/// the two lanes are equal to its result bit for bit: 16 partial sums per lane over 16 taps at a time, 4 over the next 4 at
/// a time, then one tap at a time.
#[target_feature(enable = "neon")]
#[inline]
fn dot_pairs_stepped_neon(a: &[f32], d: &[f32], f: f32, x: &[f32]) -> [f32; 2] {
    let n = a.len().min(d.len()).min(x.len() / 2);
    let (a, d, x) = (&a[..n], &d[..n], &x[..2 * n]);
    let fv = vdupq_n_f32(f);
    let zero = vdupq_n_f32(0.0);
    let (mut re, mut im) = ([zero; 4], [zero; 4]);
    // Taps `4q..4q + 4` and the two lanes of samples `4q..4q + 4`.
    let quad = |q: usize| {
        // SAFETY: the callers keep `4 * q + 4 <= n`, so the four taps are inside `a` and `d` and the eight values inside
        // `x`; the loads need no alignment.
        unsafe {
            let t = vaddq_f32(load(a, 4 * q), vmulq_f32(load(d, 4 * q), fv));
            (t, vld2q_f32(x.as_ptr().add(8 * q)))
        }
    };
    let wide = n / 16;
    for c in 0..wide {
        for j in 0..4 {
            let (t, v) = quad(4 * c + j);
            re[j] = vfmaq_f32(re[j], t, v.0);
            im[j] = vfmaq_f32(im[j], t, v.1);
        }
    }
    let (mut re4, mut im4) = (zero, zero);
    let quads = n / 4;
    for q in 4 * wide..quads {
        let (t, v) = quad(q);
        re4 = vfmaq_f32(re4, t, v.0);
        im4 = vfmaq_f32(im4, t, v.1);
    }
    let re4 = vaddq_f32(
        re4,
        vaddq_f32(vaddq_f32(re[0], re[1]), vaddq_f32(re[2], re[3])),
    );
    let im4 = vaddq_f32(
        im4,
        vaddq_f32(vaddq_f32(im[0], im[1]), vaddq_f32(im[2], im[3])),
    );
    let fold = |v: float32x4_t| {
        (vgetq_lane_f32::<0>(v) + vgetq_lane_f32::<2>(v))
            + (vgetq_lane_f32::<1>(v) + vgetq_lane_f32::<3>(v))
    };
    let (mut sa, mut sb) = (fold(re4), fold(im4));
    for k in 4 * quads..n {
        let t = a[k] + d[k] * f;
        sa += t * x[2 * k];
        sb += t * x[2 * k + 1];
    }
    [sa, sb]
}

/// The sums of [`crate::sample::dot_flat2`] made with the same operations in the same order, so the two lanes are equal to
/// its result bit for bit: `taps` holds each tap once per lane and `x` two values per sample; 32 partial sums over 32
/// values at a time, 8 over the next 8 at a time, then one sample at a time.
// Not inlined: inside a larger loop the compiler loads every tap and value first and keeps them on the stack, which doubles
// the work of the kernel.
#[target_feature(enable = "neon")]
#[inline(never)]
fn dot_lanes_pairs_neon(taps: &[f32], x: &[f32]) -> [f32; 2] {
    let n = (taps.len() / 2).min(x.len() / 2);
    let (t, x) = (&taps[..2 * n], &x[..2 * n]);
    // 64 taps, the length of a matched filter of the symbol timing, have their own code with every count fixed (the wrapper also sends 16 and 32 taps there).
    if let (Ok(t), Ok(x)) = (<&[f32; 128]>::try_from(t), <&[f32; 128]>::try_from(x)) {
        return dot_lanes_fixed_neon(t, x);
    }
    let zero = vdupq_n_f32(0.0);
    let mut acc = [zero; 8];
    let body = 2 * n / 32 * 32;
    for at in (0..body).step_by(32) {
        for (j, a) in acc.iter_mut().enumerate() {
            // SAFETY: `at + 4 * j + 4 <= at + 32 <= body <= 2 * n`, the length of `t` and of `x`.
            let (tv, xv) = unsafe { (load(t, at + 4 * j), load(x, at + 4 * j)) };
            *a = vfmaq_f32(*a, tv, xv);
        }
    }
    // Each sum holds [first lane, second lane, first lane, second lane].
    let mut lo = vaddq_f32(vaddq_f32(acc[0], acc[2]), vaddq_f32(acc[4], acc[6]));
    let mut hi = vaddq_f32(vaddq_f32(acc[1], acc[3]), vaddq_f32(acc[5], acc[7]));
    let mid = body + (2 * n - body) / 8 * 8;
    let (mut lo8, mut hi8) = (zero, zero);
    for at in (body..mid).step_by(8) {
        // SAFETY: `at + 8 <= mid <= 2 * n`, the length of `t` and of `x`.
        let (t0, x0, t1, x1) =
            unsafe { (load(t, at), load(x, at), load(t, at + 4), load(x, at + 4)) };
        lo8 = vfmaq_f32(lo8, t0, x0);
        hi8 = vfmaq_f32(hi8, t1, x1);
    }
    lo = vaddq_f32(lo, lo8);
    hi = vaddq_f32(hi, hi8);
    let s = vaddq_f32(lo, hi);
    let mut a = vgetq_lane_f32::<0>(s) + vgetq_lane_f32::<2>(s);
    let mut b = vgetq_lane_f32::<1>(s) + vgetq_lane_f32::<3>(s);
    for k in (mid..2 * n).step_by(2) {
        a = t[k].mul_add(x[k], a);
        b = t[k + 1].mul_add(x[k + 1], b);
    }
    [a, b]
}

/// [`dot_lanes_pairs_neon`] for `N / 2` taps, `N` a multiple of 32 (16, 32 or 64 taps, the lengths of a matched filter
/// of the symbol timing): the same sums in the same order, with no count left to test.
#[target_feature(enable = "neon")]
#[inline(never)]
fn dot_lanes_fixed_neon<const N: usize>(t: &[f32; N], x: &[f32; N]) -> [f32; 2] {
    const { assert!(N.is_multiple_of(32)) };
    let zero = vdupq_n_f32(0.0);
    let mut acc = [zero; 8];
    let mut at = 0;
    while at < N {
        for (j, a) in acc.iter_mut().enumerate() {
            // SAFETY: `at + 4 * j + 4 <= at + 32 <= N`, the length of `t` and of `x`.
            let (tv, xv) = unsafe { (load(t, at + 4 * j), load(x, at + 4 * j)) };
            *a = vfmaq_f32(*a, tv, xv);
        }
        at += 32;
    }
    // As in the general form, where the sums of a last part of 8 values (none here) are added.
    let lo = vaddq_f32(
        vaddq_f32(vaddq_f32(acc[0], acc[2]), vaddq_f32(acc[4], acc[6])),
        zero,
    );
    let hi = vaddq_f32(
        vaddq_f32(vaddq_f32(acc[1], acc[3]), vaddq_f32(acc[5], acc[7])),
        zero,
    );
    let s = vaddq_f32(lo, hi);
    // Lanes 0 + 2 and 1 + 3 in one add, which leaves the pair in one register.
    let p = vadd_f32(vget_low_f32(s), vget_high_f32(s));
    [vget_lane_f32::<0>(p), vget_lane_f32::<1>(p)]
}

// NEON is a required part of every ARM64 target that this module is compiled for, so the functions marked with it can
// always be called.

/// `sum taps[2k + c] * x[2k + c]` for the lanes `c` 0 and 1 (each tap is given once per lane), over the shorter of the
/// two: the result of [`crate::sample::dot_flat2`], bit for bit.
#[inline]
pub(crate) fn dot_lanes_pairs(taps: &[f32], x: &[f32]) -> [f32; 2] {
    // SAFETY (each fixed length): NEON is always present (see above).
    if let (Ok(t), Ok(x)) = (<&[f32; 32]>::try_from(taps), <&[f32; 32]>::try_from(x)) {
        return unsafe { dot_lanes_fixed_neon(t, x) };
    }
    if let (Ok(t), Ok(x)) = (<&[f32; 64]>::try_from(taps), <&[f32; 64]>::try_from(x)) {
        return unsafe { dot_lanes_fixed_neon(t, x) };
    }
    if let (Ok(t), Ok(x)) = (<&[f32; 128]>::try_from(taps), <&[f32; 128]>::try_from(x)) {
        return unsafe { dot_lanes_fixed_neon(t, x) };
    }
    // SAFETY: NEON is always present (see above).
    unsafe { dot_lanes_pairs_neon(taps, x) }
}

/// `sum (a[k] + d[k] * f) * x[2k + c]` for the lanes `c` 0 and 1, over the shortest of the taps and the samples of `x`:
/// the result of [`crate::sample::dot2`] with these taps, bit for bit.
#[inline]
pub(crate) fn dot_pairs_stepped(a: &[f32], d: &[f32], f: f32, x: &[f32]) -> [f32; 2] {
    // SAFETY: NEON is always present (see above).
    unsafe { dot_pairs_stepped_neon(a, d, f, x) }
}

/// `sum taps[k] * x[k]` over the shorter of the two slices.
#[inline]
pub(crate) fn dot_real(taps: &[f32], x: &[f32]) -> f32 {
    // SAFETY: NEON is always present (see above).
    unsafe { dot_real_neon(taps, x) }
}

/// `sum taps[k] * x[2k + c]` for the lanes `c` 0 and 1, over the shorter of the taps and the samples of `x`.
#[inline]
pub(crate) fn dot_pairs(taps: &[f32], x: &[f32]) -> [f32; 2] {
    // SAFETY: NEON is always present (see above).
    unsafe { dot_pairs_neon(taps, x) }
}

/// [`dot_real`] with the taps `a[k] + (b[k] - a[k]) mu`.
#[inline]
pub(crate) fn dot_real_blend(a: &[f32], b: &[f32], mu: f32, x: &[f32]) -> f32 {
    // SAFETY: NEON is always present (see above).
    unsafe { dot_real_blend_neon(a, b, mu, x) }
}

/// [`dot_pairs`] with the taps `a[k] + (b[k] - a[k]) mu`.
#[inline]
pub(crate) fn dot_pairs_blend(a: &[f32], b: &[f32], mu: f32, x: &[f32]) -> [f32; 2] {
    // SAFETY: NEON is always present (see above).
    unsafe { dot_pairs_blend_neon(a, b, mu, x) }
}

/// Output `i` of `out` is [`dot_real`] of `taps` with `x[i * stride..]`. Each output is the same as one call of [`dot_real`].
///
/// # Panics
/// When `x` holds less than `(out.len() - 1) * stride + taps.len()` values.
#[inline]
pub(crate) fn fir_real(taps: &[f32], x: &[f32], stride: usize, out: &mut [f32]) {
    // SAFETY: NEON is always present (see above).
    unsafe { fir_real_neon(taps, x, stride, out) }
}

/// Output `i` (two values of `out`) is [`dot_pairs`] of `taps` with the samples of `x` from sample `i * stride`. Each output
/// is the same as one call of [`dot_pairs`]. A last value of `out` without its pair is left as it is.
///
/// # Panics
/// When `x` holds less than `(out.len() / 2 - 1) * stride + taps.len()` samples.
#[inline]
pub(crate) fn fir_pairs(taps: &[f32], x: &[f32], stride: usize, out: &mut [f32]) {
    // SAFETY: NEON is always present (see above).
    unsafe { fir_pairs_neon(taps, x, stride, out) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Sample;
    use crate::Complex32;

    const LENGTHS: [usize; 14] = [0, 1, 3, 4, 5, 7, 8, 15, 16, 17, 63, 64, 65, 1000];

    fn noise(seed: &mut u64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|_| {
                *seed ^= *seed << 13;
                *seed ^= *seed >> 7;
                *seed ^= *seed << 17;
                (*seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
            })
            .collect()
    }

    /// Whether `got` is within `1e-5` of `want`, relative to the sum of the magnitudes of the products (`scale`).
    fn close(got: f32, want: f32, scale: f32) -> bool {
        (got - want).abs() <= 1e-5 * scale
    }

    fn complex(x: &[f32]) -> &[Complex32] {
        crate::sample::complex_values(x)
    }

    /// The sum of `|t[k] x[k * lanes + c]|` over the taps and lanes: the scale of the rounding error of a dot product.
    fn scale(t: &[f32], x: &[f32], lanes: usize) -> f32 {
        t.iter()
            .zip(x.chunks_exact(lanes))
            .map(|(t, x)| x.iter().map(|x| (t * x).abs()).sum::<f32>())
            .sum()
    }

    #[test]
    fn dots_agree_with_the_plain_kernels() {
        let mut seed = 0x1234_5678_9abc_def1u64;
        for n in LENGTHS {
            // Start offsets that leave the taps, the samples or both off a 16-byte boundary.
            for (toff, xoff) in [(0, 0), (1, 0), (0, 1), (3, 2), (2, 3)] {
                let t = noise(&mut seed, n + toff);
                let t = &t[toff..];
                let x = noise(&mut seed, 2 * n + xoff + 1);
                let xr = &x[xoff..xoff + n];
                let want = f32::dot(t, xr);
                let got = dot_real(t, xr);
                assert!(
                    close(got, want, scale(t, xr, 1)),
                    "real {n}: {got} for {want}"
                );

                let xc = &x[xoff..xoff + 2 * n];
                let want = Complex32::dot(t, complex(xc));
                let got = dot_pairs(t, xc);
                let s = scale(t, xc, 2);
                assert!(
                    close(got[0], want.re, s) && close(got[1], want.im, s),
                    "pairs {n}: {got:?} for {want}"
                );
            }
        }
    }

    #[test]
    fn stepped_dot_is_the_plain_dot_of_the_stepped_taps_bit_for_bit() {
        let mut seed = 0x51ed_270b_97f4_a7c1u64;
        for n in LENGTHS.into_iter().chain([12, 20, 32, 36, 48]) {
            for (toff, xoff) in [(0, 0), (1, 0), (0, 1), (3, 2)] {
                let a = noise(&mut seed, n + toff);
                let d = noise(&mut seed, n + toff);
                let (a, d) = (&a[toff..], &d[toff..]);
                let x = noise(&mut seed, 2 * n + xoff);
                let x = &x[xoff..];
                let f = noise(&mut seed, 1)[0].abs();
                let want = crate::sample::dot_stepped_plain(a, d, f, complex(x));
                let got = dot_pairs_stepped(a, d, f, x);
                assert_eq!(
                    (got[0].to_bits(), got[1].to_bits()),
                    (want.re.to_bits(), want.im.to_bits()),
                    "{n} taps: {got:?} for {want}"
                );
                assert_eq!(Complex32::dot_stepped(a, d, f, complex(x)), want);
            }
        }
    }

    #[test]
    fn lane_dot_is_the_plain_lane_dot_bit_for_bit() {
        let mut seed = 0x6a09_e667_f3bc_c908u64;
        for n in LENGTHS.into_iter().chain([12, 20, 32, 36, 48, 100]) {
            for (toff, xoff) in [(0, 0), (1, 0), (0, 1), (3, 2)] {
                let t = noise(&mut seed, 2 * n + toff);
                let t = &t[toff..];
                let x = noise(&mut seed, 2 * n + xoff);
                let x = &x[xoff..];
                let want = crate::sample::dot_flat2(t, x.as_chunks::<2>().0);
                let got = dot_lanes_pairs(t, x);
                assert_eq!(
                    (got[0].to_bits(), got[1].to_bits()),
                    (want.0.to_bits(), want.1.to_bits()),
                    "{n} taps: {got:?} for {want:?}"
                );
                let c = Complex32::dot_lanes(t, complex(x));
                assert_eq!(
                    (c.re.to_bits(), c.im.to_bits()),
                    (want.0.to_bits(), want.1.to_bits())
                );
            }
        }
        // Zero samples with negative taps make products of -0, which the sums must turn to +0 as the plain kernel does.
        for n in [16, 32, 64, 20] {
            let t = vec![-0.5f32; 2 * n];
            let x = vec![0f32; 2 * n];
            let want = crate::sample::dot_flat2(&t, x.as_chunks::<2>().0);
            let got = dot_lanes_pairs(&t, &x);
            assert_eq!(
                (got[0].to_bits(), got[1].to_bits()),
                (want.0.to_bits(), want.1.to_bits()),
                "{n} taps of zero samples"
            );
        }
    }

    #[test]
    fn dots_stop_at_the_shorter_slice() {
        let mut seed = 0x0bad_cafe_1357_9bdfu64;
        let t = noise(&mut seed, 40);
        let x = noise(&mut seed, 80);
        assert_eq!(dot_real(&t, &x[..13]), dot_real(&t[..13], &x[..13]));
        assert_eq!(dot_real(&t[..13], &x), dot_real(&t[..13], &x[..13]));
        assert_eq!(dot_pairs(&t, &x[..27]), dot_pairs(&t[..13], &x[..26]));
        assert_eq!(dot_pairs(&t[..13], &x), dot_pairs(&t[..13], &x[..26]));
        assert_eq!(
            dot_real_blend(&t, &t[..13], 0.3, &x),
            dot_real_blend(&t[..13], &t[..13], 0.3, &x[..13])
        );
        assert_eq!(
            dot_pairs_blend(&t[..13], &t, 0.3, &x),
            dot_pairs_blend(&t[..13], &t[..13], 0.3, &x[..26])
        );
    }

    #[test]
    fn blended_dots_agree_with_two_plain_dots() {
        let mut seed = 0xfeed_f00d_dead_beefu64;
        for n in LENGTHS {
            for (toff, xoff) in [(0, 0), (1, 3), (2, 1), (3, 2)] {
                let bank = noise(&mut seed, 2 * n + toff);
                let (a, b) = bank[toff..].split_at(n);
                let x = noise(&mut seed, 2 * n + xoff);
                for mu in [0.0f32, 0.25, 0.999] {
                    let xr = &x[xoff..xoff + n];
                    let (da, db) = (f32::dot(a, xr), f32::dot(b, xr));
                    let want = da + (db - da) * mu;
                    let got = dot_real_blend(a, b, mu, xr);
                    let s = scale(a, xr, 1) + scale(b, xr, 1);
                    assert!(close(got, want, s), "real {n}, mu {mu}: {got} for {want}");

                    let xc = &x[xoff..xoff + 2 * n];
                    let (da, db) = (
                        Complex32::dot(a, complex(xc)),
                        Complex32::dot(b, complex(xc)),
                    );
                    let want = da + (db - da) * mu;
                    let got = dot_pairs_blend(a, b, mu, xc);
                    let s = scale(a, xc, 2) + scale(b, xc, 2);
                    assert!(
                        close(got[0], want.re, s) && close(got[1], want.im, s),
                        "pairs {n}, mu {mu}: {got:?} for {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn filter_loops_give_the_dot_of_each_window() {
        let mut seed = 0x0f1e_2d3c_4b5a_6978u64;
        for n in LENGTHS {
            for stride in [1usize, 2, 5] {
                for outputs in [0usize, 1, 2, 3, 8] {
                    for off in [0usize, 1, 3] {
                        let t = noise(&mut seed, n + off);
                        let t = &t[off..];
                        let samples = if outputs == 0 {
                            0
                        } else {
                            (outputs - 1) * stride + n
                        };
                        let x = noise(&mut seed, 2 * samples + off);
                        let x = &x[off..];

                        let mut out = vec![f32::NAN; outputs];
                        fir_real(t, &x[..samples], stride, &mut out);
                        for (i, o) in out.iter().enumerate() {
                            assert_eq!(*o, dot_real(t, &x[i * stride..i * stride + n]), "real {n}");
                        }

                        let mut out = vec![f32::NAN; 2 * outputs];
                        fir_pairs(t, &x[..2 * samples], stride, &mut out);
                        for (i, o) in out.as_chunks::<2>().0.iter().enumerate() {
                            let w = &x[2 * i * stride..2 * (i * stride + n)];
                            assert_eq!(*o, dot_pairs(t, w), "pairs {n}");
                        }
                    }
                }
            }
        }
    }
}
