//! The sample types the streaming blocks are generic over.

use crate::nco::Nco;
use crate::Complex32;
use std::ops::{Add, AddAssign, Mul, Sub};

/// A sample a real-tap filter can process: `f32`, [`Complex32`], [`Stereo`] or an audio [`Frame`].
pub trait Sample:
    Copy
    + Default
    + Send
    + Sync
    + PartialEq
    + std::fmt::Debug
    + 'static
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<f32, Output = Self>
    + AddAssign
{
    /// `f32` values per sample (1 for `f32`, 2 for [`Complex32`] and [`Stereo`]).
    const LANES: usize;
    /// `sum taps[k] * x[k]` over the shorter of the two slices.
    fn dot(taps: &[f32], x: &[Self]) -> Self;
    /// The same dot product with the taps laid out per lane ([`lane_taps`]: each tap `LANES` times), which runs as one plain `f32`
    /// loop over the samples' values: the fastest form for a filter that keeps its taps.
    fn dot_lanes(lane_taps: &[f32], x: &[Self]) -> Self;
    /// [`Sample::dot`] with the taps `a[k] + d[k] * f`, over the shortest of the three slices: a row of a polyphase filter
    /// and its step to the next row, read at the fraction `f` between them. The result is that of `dot` with these taps.
    #[inline]
    fn dot_stepped(a: &[f32], d: &[f32], f: f32, x: &[Self]) -> Self {
        dot_stepped_plain(a, d, f, x)
    }
    /// Multiplies `buf` by the next `buf.len()` samples of `nco`, which shifts a complex signal in frequency. A real sample has
    /// no sign of frequency, so the default leaves `buf` and `nco` as they are. Only [`Complex32`] shifts.
    #[inline]
    fn mix_in_place(nco: &mut Nco, buf: &mut [Self]) {
        let _ = (nco, buf);
    }
    /// The values of `x` as one `f32` slice, `LANES` per sample, for an acceleration backend. `None` (the default) keeps the
    /// type on the plain kernels.
    #[inline]
    fn values(x: &[Self]) -> Option<&[f32]> {
        let _ = x;
        None
    }
    /// The values of `x` as one mutable `f32` slice, when [`Sample::values`] returns them.
    #[inline]
    fn values_mut(x: &mut [Self]) -> Option<&mut [f32]> {
        let _ = x;
        None
    }
}

/// [`Sample::dot_stepped`] in plain code: the taps `a[k] + d[k] * f` are made first, then [`Sample::dot`] runs on them.
pub(crate) fn dot_stepped_plain<T: Sample>(a: &[f32], d: &[f32], f: f32, x: &[T]) -> T {
    const ROW: usize = 64;
    let n = a.len().min(d.len()).min(x.len());
    let step = |row: &mut [f32]| {
        for ((r, a), d) in row.iter_mut().zip(a).zip(d) {
            *r = a + d * f;
        }
    };
    if n <= ROW {
        let mut row = [0f32; ROW];
        step(&mut row[..n]);
        T::dot(&row[..n], &x[..n])
    } else {
        let mut row = vec![0f32; n];
        step(&mut row);
        T::dot(&row, &x[..n])
    }
}

/// Taps laid out for [`Sample::dot_lanes`]: each tap `T::LANES` times.
pub fn lane_taps<T: Sample>(taps: &[f32]) -> Vec<f32> {
    taps.iter()
        .flat_map(|&t| std::iter::repeat_n(t, T::LANES))
        .collect()
}

/// `c + a * b`, fused where the CPU always has it (ARM64), so a filter costs one instruction per tap and lane.
#[inline(always)]
fn fmadd(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(target_arch = "aarch64")]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        c + a * b
    }
}

/// `sum t[k] x[k]` in 32 partial sums (returned), for slices of the same length that is a multiple of 32; the rest is the caller's.
#[inline(always)]
fn dot32(t: &[f32], x: &[f32]) -> [f32; 32] {
    let mut acc = [0.0f32; 32];
    for (a, b) in t.as_chunks::<32>().0.iter().zip(x.as_chunks::<32>().0) {
        for i in 0..32 {
            acc[i] = fmadd(a[i], b[i], acc[i]);
        }
    }
    acc
}

// The dot products keep 16 (or 32) independent partial sums, so a multiply-add never waits for the one before it: on a core with two
// 4-lane multiply-add units and a latency of 4 cycles that is what keeps them busy (NEON on ARM, SSE/AVX on x86, by
// auto-vectorisation). The sums are combined in a fixed order, so the result does not depend on anything but the inputs.

impl Sample for f32 {
    const LANES: usize = 1;
    #[inline]
    fn values(x: &[Self]) -> Option<&[f32]> {
        Some(x)
    }
    #[inline]
    fn values_mut(x: &mut [Self]) -> Option<&mut [f32]> {
        Some(x)
    }
    #[inline]
    fn dot_lanes(taps: &[f32], x: &[f32]) -> f32 {
        Self::dot(taps, x)
    }
    #[inline]
    fn dot(taps: &[f32], x: &[f32]) -> f32 {
        let n = taps.len().min(x.len());
        let (t, x) = (&taps[..n], &x[..n]);
        let mut acc = [0.0f32; 32];
        let (tc, tr) = t.as_chunks::<32>();
        let (xc, xr) = x.as_chunks::<32>();
        for (a, b) in tc.iter().zip(xc) {
            for i in 0..32 {
                acc[i] = fmadd(a[i], b[i], acc[i]);
            }
        }
        let mut acc8 = [0.0f32; 8];
        let (tc, tr) = tr.as_chunks::<8>();
        let (xc, xr) = xr.as_chunks::<8>();
        for (a, b) in tc.iter().zip(xc) {
            for i in 0..8 {
                acc8[i] = fmadd(a[i], b[i], acc8[i]);
            }
        }
        for k in 0..8 {
            acc8[k] += (acc[k] + acc[k + 8]) + (acc[k + 16] + acc[k + 24]);
        }
        let mut s = ((acc8[0] + acc8[4]) + (acc8[1] + acc8[5]))
            + ((acc8[2] + acc8[6]) + (acc8[3] + acc8[7]));
        for (a, b) in tr.iter().zip(xr) {
            s += a * b;
        }
        s
    }
}

/// `sum taps[k] * x[k]` for two interleaved channels (`[re, im]` of a complex sample, `[l, r]` of a stereo frame), as `(first,
/// second)`.
#[inline]
fn dot2(taps: &[f32], x: &[[f32; 2]]) -> (f32, f32) {
    let n = taps.len().min(x.len());
    let (t, x) = (&taps[..n], &x[..n]);
    let mut a = [0.0f32; 16];
    let mut b = [0.0f32; 16];
    let (tc, tr) = t.as_chunks::<16>();
    let (xc, xr) = x.as_chunks::<16>();
    for (c, v) in tc.iter().zip(xc) {
        for i in 0..16 {
            a[i] = fmadd(c[i], v[i][0], a[i]);
            b[i] = fmadd(c[i], v[i][1], b[i]);
        }
    }
    let mut a4 = [0.0f32; 4];
    let mut b4 = [0.0f32; 4];
    let (tc, tr) = tr.as_chunks::<4>();
    let (xc, xr) = xr.as_chunks::<4>();
    for (c, v) in tc.iter().zip(xc) {
        for i in 0..4 {
            a4[i] = fmadd(c[i], v[i][0], a4[i]);
            b4[i] = fmadd(c[i], v[i][1], b4[i]);
        }
    }
    for k in 0..4 {
        a4[k] += (a[k] + a[k + 4]) + (a[k + 8] + a[k + 12]);
        b4[k] += (b[k] + b[k + 4]) + (b[k + 8] + b[k + 12]);
    }
    let mut sa = (a4[0] + a4[2]) + (a4[1] + a4[3]);
    let mut sb = (b4[0] + b4[2]) + (b4[1] + b4[3]);
    for (c, v) in tr.iter().zip(xr) {
        sa += c * v[0];
        sb += c * v[1];
    }
    (sa, sb)
}

/// The dot product of two-lane samples with lane taps, as one flat `f32` loop: (sum of the even lanes, sum of the odd lanes).
#[inline]
pub(crate) fn dot_flat2(taps: &[f32], x: &[[f32; 2]]) -> (f32, f32) {
    let n = (taps.len() / 2).min(x.len());
    let t = &taps[..2 * n];
    let x = x[..n].as_flattened();
    let body = 2 * n / 32 * 32;
    let acc = dot32(&t[..body], &x[..body]);
    let mut e = [0.0f32; 4];
    let mut o = [0.0f32; 4];
    for k in 0..4 {
        e[k] = (acc[2 * k] + acc[2 * k + 8]) + (acc[2 * k + 16] + acc[2 * k + 24]);
        o[k] = (acc[2 * k + 1] + acc[2 * k + 9]) + (acc[2 * k + 17] + acc[2 * k + 25]);
    }
    // What is left, eight values (four samples) at a time, then one sample at a time.
    let mid = body + (2 * n - body) / 8 * 8;
    let mut acc8 = [0.0f32; 8];
    for (c, v) in t[body..mid]
        .as_chunks::<8>()
        .0
        .iter()
        .zip(x[body..mid].as_chunks::<8>().0)
    {
        for i in 0..8 {
            acc8[i] = fmadd(c[i], v[i], acc8[i]);
        }
    }
    for k in 0..4 {
        e[k] += acc8[2 * k];
        o[k] += acc8[2 * k + 1];
    }
    let (mut a, mut b) = ((e[0] + e[2]) + (e[1] + e[3]), (o[0] + o[2]) + (o[1] + o[3]));
    for (c, v) in t[mid..]
        .as_chunks::<2>()
        .0
        .iter()
        .zip(x[mid..].as_chunks::<2>().0)
    {
        a = fmadd(c[0], v[0], a);
        b = fmadd(c[1], v[1], b);
    }
    (a, b)
}

/// `x` (pairs of values) as complex samples. A last value without its pair is left out.
pub(crate) fn complex_values(x: &[f32]) -> &[Complex32] {
    let x = x.as_chunks::<2>().0;
    // SAFETY: `num_complex::Complex<f32>` is `#[repr(C)]` with fields `re: f32, im: f32`: the same size (8), alignment (4) and
    // layout as `[f32; 2]`, with no padding; the length and lifetime are those of `x`.
    unsafe { std::slice::from_raw_parts(x.as_ptr().cast::<Complex32>(), x.len()) }
}

/// `x` (pairs of values) as mutable complex samples. A last value without its pair is left out.
pub(crate) fn complex_values_mut(x: &mut [f32]) -> &mut [Complex32] {
    let x = x.as_chunks_mut::<2>().0;
    // SAFETY: the layout is that of `[f32; 2]` (see `complex_values`); the borrow of `x` is exclusive for the lifetime of the
    // result.
    unsafe { std::slice::from_raw_parts_mut(x.as_mut_ptr().cast::<Complex32>(), x.len()) }
}

/// Types laid out as two `f32`s.
trait Pairs: Sized {
    fn pairs(x: &[Self]) -> &[[f32; 2]];
    fn pairs_mut(x: &mut [Self]) -> &mut [[f32; 2]];
}

impl Pairs for Complex32 {
    #[inline]
    fn pairs(x: &[Self]) -> &[[f32; 2]] {
        // SAFETY: `num_complex::Complex<f32>` is `#[repr(C)]` with fields `re: f32, im: f32`: the same size (8), alignment (4) and
        // layout as `[f32; 2]`, with no padding; the length and lifetime are those of `x`.
        unsafe { std::slice::from_raw_parts(x.as_ptr().cast::<[f32; 2]>(), x.len()) }
    }
    #[inline]
    fn pairs_mut(x: &mut [Self]) -> &mut [[f32; 2]] {
        // SAFETY: the layout is that of `[f32; 2]` (see `pairs`); the borrow of `x` is exclusive for the lifetime of the result.
        unsafe { std::slice::from_raw_parts_mut(x.as_mut_ptr().cast::<[f32; 2]>(), x.len()) }
    }
}

impl Sample for Complex32 {
    const LANES: usize = 2;
    #[inline]
    fn values(x: &[Self]) -> Option<&[f32]> {
        Some(Pairs::pairs(x).as_flattened())
    }
    #[inline]
    fn values_mut(x: &mut [Self]) -> Option<&mut [f32]> {
        Some(Pairs::pairs_mut(x).as_flattened_mut())
    }
    #[inline]
    fn mix_in_place(nco: &mut Nco, buf: &mut [Complex32]) {
        nco.mix_in_place(buf);
    }
    #[cfg(not(target_arch = "aarch64"))]
    #[inline]
    fn dot_lanes(taps: &[f32], x: &[Complex32]) -> Complex32 {
        let (re, im) = dot_flat2(taps, Pairs::pairs(x));
        Complex32::new(re, im)
    }
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn dot_lanes(taps: &[f32], x: &[Complex32]) -> Complex32 {
        let [re, im] = crate::neon::dot_lanes_pairs(taps, Pairs::pairs(x).as_flattened());
        Complex32::new(re, im)
    }
    #[inline]
    fn dot(taps: &[f32], x: &[Complex32]) -> Complex32 {
        let (re, im) = dot2(taps, Pairs::pairs(x));
        Complex32::new(re, im)
    }
    #[cfg(target_arch = "aarch64")]
    #[inline]
    fn dot_stepped(a: &[f32], d: &[f32], f: f32, x: &[Complex32]) -> Complex32 {
        let [re, im] = crate::neon::dot_pairs_stepped(a, d, f, Pairs::pairs(x).as_flattened());
        Complex32::new(re, im)
    }
}

/// One stereo audio frame. `#[repr(C)]`, so a slice of frames has the layout of interleaved `[l, r, l, r, ...]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stereo {
    /// Left.
    pub l: f32,
    /// Right.
    pub r: f32,
}

impl Stereo {
    /// A frame from its two channels.
    pub const fn new(l: f32, r: f32) -> Self {
        Stereo { l, r }
    }

    /// Appends the frames of interleaved `[l, r, ...]` samples to `out`. A trailing odd sample is ignored.
    pub fn from_interleaved(src: &[f32], out: &mut Vec<Stereo>) {
        out.extend(
            src.as_chunks::<2>()
                .0
                .iter()
                .map(|c| Stereo::new(c[0], c[1])),
        );
    }

    /// Appends the frames to `out` as interleaved `[l, r, ...]` samples.
    pub fn to_interleaved(src: &[Stereo], out: &mut Vec<f32>) {
        out.reserve(src.len() * 2);
        for s in src {
            out.push(s.l);
            out.push(s.r);
        }
    }
}

impl Add for Stereo {
    type Output = Stereo;
    #[inline]
    fn add(self, o: Stereo) -> Stereo {
        Stereo::new(self.l + o.l, self.r + o.r)
    }
}
impl Sub for Stereo {
    type Output = Stereo;
    #[inline]
    fn sub(self, o: Stereo) -> Stereo {
        Stereo::new(self.l - o.l, self.r - o.r)
    }
}
impl Mul<f32> for Stereo {
    type Output = Stereo;
    #[inline]
    fn mul(self, k: f32) -> Stereo {
        Stereo::new(self.l * k, self.r * k)
    }
}
impl AddAssign for Stereo {
    #[inline]
    fn add_assign(&mut self, o: Stereo) {
        self.l += o.l;
        self.r += o.r;
    }
}

impl Pairs for Stereo {
    #[inline]
    fn pairs(x: &[Self]) -> &[[f32; 2]] {
        // SAFETY: `Stereo` is `#[repr(C)]` with fields `l: f32, r: f32`: the same size (8), alignment (4) and layout as `[f32; 2]`,
        // with no padding; the length and lifetime are those of `x`.
        unsafe { std::slice::from_raw_parts(x.as_ptr().cast::<[f32; 2]>(), x.len()) }
    }
    #[inline]
    fn pairs_mut(x: &mut [Self]) -> &mut [[f32; 2]] {
        // SAFETY: the layout is that of `[f32; 2]` (see `pairs`); the borrow of `x` is exclusive for the lifetime of the result.
        unsafe { std::slice::from_raw_parts_mut(x.as_mut_ptr().cast::<[f32; 2]>(), x.len()) }
    }
}

impl Sample for Stereo {
    const LANES: usize = 2;
    #[inline]
    fn values(x: &[Self]) -> Option<&[f32]> {
        Some(Pairs::pairs(x).as_flattened())
    }
    #[inline]
    fn values_mut(x: &mut [Self]) -> Option<&mut [f32]> {
        Some(Pairs::pairs_mut(x).as_flattened_mut())
    }
    #[inline]
    fn dot_lanes(taps: &[f32], x: &[Stereo]) -> Stereo {
        let (l, r) = dot_flat2(taps, Pairs::pairs(x));
        Stereo::new(l, r)
    }
    #[inline]
    fn dot(taps: &[f32], x: &[Stereo]) -> Stereo {
        let (l, r) = dot2(taps, Pairs::pairs(x));
        Stereo::new(l, r)
    }
}

/// Where a filter or a rate change appends its output: a `Vec`, or interleaved audio through [`FrameWriter`].
pub trait Append<T> {
    /// Makes room for `n` more items.
    fn reserve(&mut self, n: usize);
    /// Appends one item.
    fn push(&mut self, x: T);
    /// Appends the items of `x`.
    fn extend_from_slice(&mut self, x: &[T]);
}

impl<T: Clone> Append<T> for Vec<T> {
    #[inline]
    fn reserve(&mut self, n: usize) {
        Vec::reserve(self, n);
    }
    #[inline]
    fn push(&mut self, x: T) {
        Vec::push(self, x);
    }
    #[inline]
    fn extend_from_slice(&mut self, x: &[T]) {
        Vec::extend_from_slice(self, x);
    }
}

/// Interleaved audio samples of `N` channels, to which frames are appended with no copy through a buffer of frames.
#[derive(Debug)]
pub struct FrameWriter<'a, const N: usize>(pub &'a mut Vec<f32>);

impl<const N: usize> Append<Frame<N>> for FrameWriter<'_, N> {
    #[inline]
    fn reserve(&mut self, n: usize) {
        self.0.reserve(n * N);
    }
    #[inline]
    fn push(&mut self, x: Frame<N>) {
        self.0.extend_from_slice(&x.0);
    }
    #[inline]
    fn extend_from_slice(&mut self, x: &[Frame<N>]) {
        self.0.extend_from_slice(Frame::flat(x));
    }
}

/// One audio frame of `N` channels. `#[repr(transparent)]` over `[f32; N]`, so a slice of frames has the layout of interleaved
/// samples `[c0, c1, ..., c0, c1, ...]`. Filters and rate changes run on frames directly, with one state for all channels.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame<const N: usize>(pub [f32; N]);

impl<const N: usize> Default for Frame<N> {
    fn default() -> Self {
        Frame([0.0; N])
    }
}

impl<const N: usize> Frame<N> {
    /// The whole frames of interleaved samples `x`. A trailing part frame is left out.
    pub fn view(x: &[f32]) -> &[Frame<N>] {
        let whole = x.as_chunks::<N>().0;
        // SAFETY: `Frame<N>` is `#[repr(transparent)]` over `[f32; N]`, so both have the same size, alignment and layout; the
        // length and lifetime are those of `whole`.
        unsafe { std::slice::from_raw_parts(whole.as_ptr().cast::<Frame<N>>(), whole.len()) }
    }

    /// The interleaved samples of `frames`.
    pub fn flat(frames: &[Frame<N>]) -> &[f32] {
        // SAFETY: `Frame<N>` is `#[repr(transparent)]` over `[f32; N]`, so `len` frames are `len * N` contiguous `f32`s with no
        // padding; the lifetime is that of `frames`.
        unsafe { std::slice::from_raw_parts(frames.as_ptr().cast::<f32>(), frames.len() * N) }
    }
}

impl<const N: usize> Add for Frame<N> {
    type Output = Frame<N>;
    #[inline]
    fn add(mut self, o: Frame<N>) -> Frame<N> {
        self += o;
        self
    }
}
impl<const N: usize> Sub for Frame<N> {
    type Output = Frame<N>;
    #[inline]
    fn sub(mut self, o: Frame<N>) -> Frame<N> {
        for (a, b) in self.0.iter_mut().zip(o.0) {
            *a -= b;
        }
        self
    }
}
impl<const N: usize> Mul<f32> for Frame<N> {
    type Output = Frame<N>;
    #[inline]
    fn mul(mut self, k: f32) -> Frame<N> {
        for a in &mut self.0 {
            *a *= k;
        }
        self
    }
}
impl<const N: usize> AddAssign for Frame<N> {
    #[inline]
    fn add_assign(&mut self, o: Frame<N>) {
        for (a, b) in self.0.iter_mut().zip(o.0) {
            *a += b;
        }
    }
}

impl<const N: usize> Sample for Frame<N> {
    const LANES: usize = N;
    #[inline]
    fn dot_lanes(lane_taps: &[f32], x: &[Frame<N>]) -> Frame<N> {
        let mut acc = [0.0f32; N];
        for (t, s) in lane_taps.as_chunks::<N>().0.iter().zip(x) {
            for ((a, &ti), &si) in acc.iter_mut().zip(t).zip(&s.0) {
                *a = fmadd(ti, si, *a);
            }
        }
        Frame(acc)
    }
    #[inline]
    fn dot(taps: &[f32], x: &[Frame<N>]) -> Frame<N> {
        let mut acc = [0.0f32; N];
        for (&t, s) in taps.iter().zip(x) {
            for (a, &si) in acc.iter_mut().zip(&s.0) {
                *a = fmadd(t, si, *a);
            }
        }
        Frame(acc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_agree_with_naive_sums() {
        for n in [1usize, 5, 37, 64, 117, 230] {
            dots_of(n);
        }
    }

    fn dots_of(n: usize) {
        let taps: Vec<f32> = (0..n).map(|i| (i as f32 * 0.37).sin()).collect();
        let x: Vec<f32> = (0..n).map(|i| (i as f32 * 1.3).cos()).collect();
        let naive: f32 = taps.iter().zip(&x).map(|(a, b)| a * b).sum();
        let tol = 1e-5 * (1.0 + naive.abs());
        assert!((f32::dot(&taps, &x) - naive).abs() < tol, "{n}");
        let cx: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v, -2.0 * v)).collect();
        let c = Complex32::dot(&taps, &cx);
        assert!(
            (c.re - naive).abs() < tol && (c.im + 2.0 * naive).abs() < 4.0 * tol,
            "{n}"
        );
        let sx: Vec<Stereo> = x.iter().map(|&v| Stereo::new(v, 3.0 * v)).collect();
        let s = Stereo::dot(&taps, &sx);
        assert!(
            (s.l - naive).abs() < tol && (s.r - 3.0 * naive).abs() < 4.0 * tol,
            "{n}"
        );
        // The lane form returns the same value.
        let c2 = Complex32::dot_lanes(&lane_taps::<Complex32>(&taps), &cx);
        assert!(
            (c2.re - naive).abs() < tol && (c2.im + 2.0 * naive).abs() < 4.0 * tol,
            "{n}"
        );
        let s2 = Stereo::dot_lanes(&lane_taps::<Stereo>(&taps), &sx);
        assert!(
            (s2.l - naive).abs() < tol && (s2.r - 3.0 * naive).abs() < 4.0 * tol,
            "{n}"
        );
    }

    #[test]
    fn interleave_round_trip() {
        let src = [1.0, 2.0, 3.0, 4.0];
        let mut frames = Vec::new();
        Stereo::from_interleaved(&src, &mut frames);
        assert_eq!(frames, vec![Stereo::new(1.0, 2.0), Stereo::new(3.0, 4.0)]);
        let mut back = Vec::new();
        Stereo::to_interleaved(&frames, &mut back);
        assert_eq!(back, src);
    }
}
