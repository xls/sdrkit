//! Small adapters over the shared DSP types that the video components use: reads of a sample ring at signed and fractional
//! positions, window means, a two-stage running mean, and the frequency-sampled filter designs of the vision and colour filters.

use sdrkit_algo_dsp::interp::{CatmullRom, Interpolator, Polyphase, Reader};
use sdrkit_algo_dsp::ring::SampleRing;
use sdrkit_algo_dsp::stats::WindowSum;
use sdrkit_algo_dsp::Sample;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// A quadrature discriminator (`sdrkit_algo_dsp::fm::Discriminator`) that outputs the phase step between samples in
/// radians.
pub fn phase_steps() -> sdrkit_algo_dsp::fm::Discriminator {
    sdrkit_algo_dsp::fm::Discriminator::try_new(1.0, 1.0 / TAU)
        .expect("a positive rate and deviation")
}

/// A ring of the newest samples, addressed by absolute index (see [`SampleRing`]).
pub type Ring<T> = SampleRing<T>;

/// A ring of at least `n` samples (rounded up to a power of two, at least 16).
pub fn ring<T: Sample>(n: usize) -> Ring<T> {
    SampleRing::new(n.max(16).next_power_of_two()).expect("the capacity is at least 16")
}

/// Reads of a ring at signed absolute indices. An index that is not held reads as zero.
pub trait RingRead<T: Sample> {
    /// Whether index `i` is held.
    fn has(&self, i: i64) -> bool;
    /// The sample at index `i`, or zero.
    fn at(&self, i: i64) -> T;
    /// The mean over indices `[a, b)`, or zero for an empty range.
    fn mean_of(&self, a: i64, b: i64) -> T;
}

impl<T: Sample> RingRead<T> for SampleRing<T> {
    #[inline]
    fn has(&self, i: i64) -> bool {
        i >= 0 && self.get(i as u64).is_some()
    }
    #[inline]
    fn at(&self, i: i64) -> T {
        if i < 0 {
            return T::default();
        }
        self.get(i as u64).unwrap_or_default()
    }
    fn mean_of(&self, a: i64, b: i64) -> T {
        if b <= a {
            return T::default();
        }
        let mut s = T::default();
        for i in a..b {
            s += self.at(i);
        }
        s * (1.0 / (b - a) as f32)
    }
}

/// The newest samples of a stream in one piece, addressed by absolute index: a ring that is appended to by blocks and
/// read by slices. It reads as a [`Ring`] of `cap` samples reads.
#[derive(Clone, Debug)]
pub struct Held<T> {
    /// The index of `buf[0]`.
    first: u64,
    buf: Vec<T>,
    /// The samples that a reader may look back.
    cap: usize,
}

impl<T: Sample> Held<T> {
    /// With `cap` samples of memory.
    pub fn new(cap: usize) -> Self {
        Held {
            first: 0,
            buf: Vec::new(),
            cap,
        }
    }

    /// The absolute index that the next sample gets.
    pub fn end(&self) -> u64 {
        self.first + self.buf.len() as u64
    }

    /// Appends `x`. At least `cap` samples before `x` stay.
    pub fn extend(&mut self, x: &[T]) {
        if self.buf.len() > 3 * self.cap {
            let drop = self.buf.len() - self.cap;
            self.buf.drain(..drop);
            self.first += drop as u64;
        }
        self.buf.extend_from_slice(x);
    }

    /// The samples `[a, b)` as a reader that held `held` samples saw them: a sample from `held` on, or more than `cap`
    /// before it, or before index 0, reads as zero. `scratch` holds the copy when a part reads as zero.
    pub fn span<'a>(&'a self, a: i64, b: i64, held: u64, scratch: &'a mut Vec<T>) -> &'a [T] {
        let lo = held.saturating_sub(self.cap as u64).max(self.first);
        if a >= 0 && a as u64 >= lo && b >= a && b as u64 <= held {
            let i = (a as u64 - self.first) as usize;
            if let Some(s) = self.buf.get(i..i + (b - a) as usize) {
                return s;
            }
        }
        scratch.clear();
        for k in a..b {
            let v = if k >= 0 && k as u64 >= lo && (k as u64) < held {
                self.buf.get((k as u64 - self.first) as usize).copied()
            } else {
                None
            };
            scratch.push(v.unwrap_or_default());
        }
        scratch
    }
}

impl<T: Sample> RingRead<T> for Held<T> {
    #[inline]
    fn has(&self, i: i64) -> bool {
        let end = self.end();
        i >= 0 && (i as u64) < end && i as u64 >= end.saturating_sub(self.cap as u64)
    }
    #[inline]
    fn at(&self, i: i64) -> T {
        if self.has(i) {
            self.buf[(i as u64 - self.first) as usize]
        } else {
            T::default()
        }
    }
    fn mean_of(&self, a: i64, b: i64) -> T {
        if b <= a {
            return T::default();
        }
        let mut s = T::default();
        for i in a..b {
            s += self.at(i);
        }
        s * (1.0 / (b - a) as f32)
    }
}

/// The mean and the variance of a real ring over indices `[a, b)` (zeros for an empty range).
pub fn mean_var(ring: &Ring<f32>, a: i64, b: i64) -> (f32, f32) {
    if b <= a {
        return (0.0, 0.0);
    }
    let n = (b - a) as f32;
    let (mut s, mut s2) = (0.0f32, 0.0f32);
    for i in a..b {
        let v = ring.at(i);
        s += v;
        s2 += v * v;
    }
    let m = s / n;
    (m, (s2 / n - m * m).max(0.0))
}

/// An 8-tap windowed-sinc interpolator with 64 phases (`sdrkit_algo_dsp::interp::Polyphase`), read from a slice or a ring.
#[derive(Clone, Debug)]
pub struct Interp8 {
    bank: Reader<Polyphase<8, 64>>,
}

impl Default for Interp8 {
    fn default() -> Self {
        Self::new(0.93)
    }
}

impl Interp8 {
    /// With the cut-off as a fraction of Nyquist (Kaiser beta 4.5).
    pub fn new(cutoff: f64) -> Self {
        Interp8 {
            bank: Reader::new(Polyphase::new(cutoff, 4.5).expect("the cut-off is in (0, 1]")),
        }
    }

    /// The complex gain of the interpolation at fraction `frac` for a tone of `w` radians a sample: the tone `e^{jwn}` read at
    /// `n + frac` is `gain e^{jw (n + frac)}`.
    pub fn gain(&self, frac: f64, w: f64) -> Complex32 {
        let mu = frac.rem_euclid(1.0);
        let mut g = num_complex::Complex64::new(0.0, 0.0);
        // The response to each unit sample of the window, read back through the interpolator.
        let mut unit = [0f32; 8];
        for k in 0..8 {
            unit.fill(0.0);
            unit[k] = 1.0;
            let tap = self.bank.interpolator().at(&unit, mu as f32) as f64;
            g += num_complex::Complex64::from_polar(tap, w * (k as f64 - 3.0 - mu));
        }
        Complex32::new(g.re as f32, g.im as f32)
    }

    /// The value of `ring` at the fractional absolute position `pos`.
    #[inline]
    pub fn at<T: Sample, R: RingRead<T>>(&self, ring: &R, pos: f64) -> T {
        let i = pos.floor();
        let mu = (pos - i) as f32;
        let base = i as i64 - 3;
        let mut w = [T::default(); 8];
        for (k, v) in w.iter_mut().enumerate() {
            *v = ring.at(base + k as i64);
        }
        self.bank.interpolator().at(&w, mu)
    }

    /// The value of a slice at position `pos` from its start (outside reads as zero).
    #[inline]
    pub fn at_slice<T: Sample>(&self, x: &[T], pos: f64) -> T {
        self.bank.at(x, pos)
    }

    /// [`at_slice`](Self::at_slice) at `pos(k)` for each `out[k]`, bit for bit, in batches.
    pub fn row<T: Sample>(&self, x: &[T], pos: impl Fn(usize) -> f64, out: &mut [T]) {
        self.bank.read_positions(x, pos, out);
    }
}

/// Cubic (Catmull-Rom) interpolation of a slice at `pos` (`sdrkit_algo_dsp::interp::CatmullRom`). A window that reaches past an end
/// repeats the end sample.
#[inline]
pub fn cubic<T: Sample>(x: &[T], pos: f64) -> T {
    if x.is_empty() || !pos.is_finite() {
        return T::default();
    }
    let i = pos.floor() as i64;
    let mu = (pos - i as f64) as f32;
    let last = x.len() as i64 - 1;
    if i >= 1 && i + 2 <= last {
        let j = i as usize - 1;
        return CatmullRom.at(&x[j..j + 4], mu);
    }
    let w = [-1i64, 0, 1, 2].map(|d| x[(i + d).clamp(0, last) as usize]);
    CatmullRom.at(&w, mu)
}

/// [`cubic`] at `pos(k)` for each `out[k]`, bit for bit, in batches (`CatmullRom::at_positions`).
pub fn cubic_row<T: Sample>(x: &[T], pos: impl Fn(usize) -> f64, out: &mut [T]) {
    CatmullRom.at_positions(x, pos, |p| cubic(x, p), out);
}

/// The Kaiser window beta of the family's filters designed by frequency sampling (`sdrkit_algo_dsp::fir::design_by_response`).
pub const DESIGN_BETA: f64 = 6.0;

/// A raised-cosine step from 0 to 1 whose 10 to 90 % rise time is `rise` (the step is `rise / 0.59` long), centred on 0.
#[inline]
pub fn rc_step(t: f64, rise: f64) -> f64 {
    let len = (rise / 0.59).max(1e-9);
    sdrkit_algo_dsp::pulse::ramp(t / len + 0.5)
}

/// Moves `x` towards `v` by the weight `w` (a one-pole smoother with a weight that can change at each step).
#[inline]
pub fn smooth(x: &mut f64, v: f64, w: f64) {
    *x += w * (v - *x);
}

/// Two running means of `w` samples in a row, from two `sdrkit_algo_dsp::stats::WindowSum`. The delay is `w - 1` samples.
#[derive(Clone, Debug)]
pub struct DoubleMean {
    a: WindowSum<f32>,
    b: WindowSum<f32>,
}

impl DoubleMean {
    /// Of width `w` (at least 1).
    pub fn new(w: usize) -> Self {
        let w = w.max(1);
        DoubleMean {
            a: WindowSum::new(w).expect("the width is at least 1"),
            b: WindowSum::new(w).expect("the width is at least 1"),
        }
    }
    /// The delay in samples.
    pub fn delay(&self) -> f64 {
        (self.a.len() - 1) as f64
    }
    /// The width.
    pub fn width(&self) -> usize {
        self.a.len()
    }
    /// Takes one sample and returns the output.
    #[inline]
    pub fn push(&mut self, x: f32) -> f32 {
        let w = self.a.len() as f64;
        let m1 = (self.a.push(x) / w) as f32;
        (self.b.push(m1) / w) as f32
    }
}

/// The mean of a slice (zero for an empty one).
pub fn mean(x: &[f32]) -> f32 {
    if x.is_empty() {
        0.0
    } else {
        x.iter().sum::<f32>() / x.len() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_reads_signed_indices() {
        let mut r = ring::<f32>(16);
        for i in 0..40 {
            r.push(i as f32);
        }
        assert_eq!(r.at(39), 39.0);
        assert_eq!(r.at(24), 24.0);
        assert_eq!(r.at(23), 0.0);
        assert_eq!(r.at(-1), 0.0);
        assert_eq!(r.mean_of(30, 32), 30.5);
    }

    #[test]
    fn held_samples_read_as_a_ring_of_the_same_size() {
        let mut h = Held::<f32>::new(16);
        let mut r = ring::<f32>(16);
        let mut scratch = Vec::new();
        for block in 0..9 {
            let x: Vec<f32> = (0..11).map(|i| (block * 11 + i) as f32 + 1.0).collect();
            h.extend(&x);
            r.extend(&x);
            let held = r.end();
            assert_eq!(h.end(), held);
            for a in [
                -3i64,
                0,
                held as i64 - 20,
                held as i64 - 16,
                held as i64 - 5,
            ] {
                let b = a + 9;
                let want: Vec<f32> = (a..b).map(|k| r.at(k)).collect();
                assert_eq!(h.span(a, b, held, &mut scratch), &want[..], "{a} of {held}");
                let got: Vec<f32> = (a..b).map(|k| h.at(k)).collect();
                assert_eq!(got, want, "{a} of {held}");
                assert_eq!(h.mean_of(a, b), r.mean_of(a, b));
            }
        }
    }

    #[test]
    fn the_rows_equal_a_read_per_pixel_bit_for_bit() {
        let x: Vec<f32> = (0..1200)
            .map(|i| ((i * 7919) % 1000) as f32 / 500.0 - 1.0)
            .collect();
        let xc: Vec<Complex32> = x
            .iter()
            .zip(x.iter().rev())
            .map(|(&a, &b)| Complex32::new(a, b))
            .collect();
        let interp = Interp8::default();
        for (k, (x0, dx)) in [
            (1.0e7 + 0.3f64, 520.0 / 768.0),
            (2.3e7 + 0.71, 526.56 / 640.0),
            (0.5, 0.2),
        ]
        .into_iter()
        .enumerate()
        {
            for shift in [-60.0, -40.0, 0.0, 300.0] {
                let a = x0.floor() - 40.0 + shift;
                let w = 768 + 300 * k;
                let luma = |i: usize| x0 + (i as f64 + 0.5) * dx - a;
                let chroma = |i: usize| (x0 + (i as f64 + 0.5) * dx + 1.7 - a) / 2.0;
                let secam = |i: usize| (x0 + (i as f64 + 0.5) * dx - a) / 4.0;
                let mut y = vec![0f32; w];
                interp.row(&x, luma, &mut y);
                assert!(y
                    .iter()
                    .enumerate()
                    .all(|(i, v)| v.to_bits() == interp.at_slice(&x, luma(i)).to_bits()));
                cubic_row(&x, secam, &mut y);
                assert!(y
                    .iter()
                    .enumerate()
                    .all(|(i, v)| v.to_bits() == cubic(&x, secam(i)).to_bits()));
                let mut c = vec![Complex32::default(); w];
                cubic_row(&xc, chroma, &mut c);
                assert!(c.iter().enumerate().all(|(i, v)| {
                    let want = cubic(&xc, chroma(i));
                    (v.re.to_bits(), v.im.to_bits()) == (want.re.to_bits(), want.im.to_bits())
                }));
            }
        }
    }

    #[test]
    fn the_interpolators_reproduce_a_slow_sine() {
        let x: Vec<f32> = (0..200).map(|i| (i as f32 * 0.1).sin()).collect();
        let ip = Interp8::default();
        for k in 0..50 {
            let p = 50.0 + k as f64 * 0.37;
            assert!((ip.at_slice(&x, p) - (p as f32 * 0.1).sin()).abs() < 0.01);
        }
        assert!((cubic(&x, 60.5) - (6.05f32).sin()).abs() < 1e-3);
        let g = ip.gain(0.25, 0.1);
        assert!((g.norm() - 1.0).abs() < 0.01 && g.arg().abs() < 0.01, "{g}");
    }

    #[test]
    fn the_step_and_the_double_mean_hold_their_values() {
        assert!((rc_step(0.0, 0.2) - 0.5).abs() < 1e-9);
        let mut d = DoubleMean::new(3);
        let out: Vec<f32> = (0..6).map(|_| d.push(3.0)).collect();
        assert_eq!(out[5], 3.0);
    }
}
