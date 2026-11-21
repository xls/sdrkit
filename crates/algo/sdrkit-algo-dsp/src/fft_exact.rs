//! A portable FFT with a fixed order of operations, for codecs that must produce bit-exact output on every platform. It is the
//! mixed radix-4 and radix-2 decimation-in-time FFT of KISS FFT (BSD licence) and its real-input wrapper. Each twiddle factor
//! is computed in `f64` and rounded once to `f32`. No product is fused with an addition.
//!
//! Standard: J. W. Cooley and J. W. Tukey, "An Algorithm for the Machine Calculation of Complex Fourier Series", Math. Comp.
//! 19, 1965 (mixed-radix decimation in time).

use crate::{Complex32, DspError, Param};
use std::f64::consts::PI;

/// `a b`, with each product rounded to `f32` before the sum.
#[inline]
fn cmul(a: Complex32, b: Complex32) -> Complex32 {
    let rr = a.re * b.re;
    let ii = a.im * b.im;
    let ri = a.re * b.im;
    let ir = a.im * b.re;
    Complex32::new(rr - ii, ri + ir)
}

/// The twiddle factor `exp(j theta)` with `theta` rounded to `f32` first and each part computed in `f64`.
fn expj(theta: f32) -> Complex32 {
    let t = theta as f64;
    Complex32::new(t.cos() as f32, t.sin() as f32)
}

/// A complex FFT of a power-of-two size with a fixed operation order. It is unnormalised in both directions. The forward
/// transform uses `exp(-j 2 pi k n / N)`.
#[derive(Clone, Debug)]
pub struct ExactFft {
    n: usize,
    inverse: bool,
    /// The radix and the remaining length of each stage.
    stages: Vec<(usize, usize)>,
    tw: Vec<Complex32>,
    scratch: Vec<Complex32>,
}

impl ExactFft {
    /// A transform of `n` points. The radices are 4 while 4 divides the remaining length, then 2. Errors unless `n` is a power
    /// of two from 2 to 2^20.
    ///
    /// ```
    /// use sdrkit_algo_dsp::fft_exact::ExactFft;
    /// use sdrkit_algo_dsp::Complex32;
    /// let mut f = ExactFft::new(8, false).unwrap();
    /// let mut x = [Complex32::new(0.0, 0.0); 8];
    /// x[0] = Complex32::new(1.0, 0.0);
    /// f.process(&mut x).unwrap();
    /// assert!(x.iter().all(|v| *v == Complex32::new(1.0, 0.0)));
    /// ```
    pub fn new(n: usize, inverse: bool) -> Result<Self, DspError> {
        if !n.is_power_of_two() || !(2..=1 << 20).contains(&n) {
            return Param::FftSize.not_allowed(n as f64);
        }
        let mut stages = Vec::new();
        let mut rest = n;
        while rest > 1 {
            let p = if rest.is_multiple_of(4) { 4 } else { 2 };
            rest /= p;
            stages.push((p, rest));
        }
        let tw = (0..n)
            .map(|i| {
                let theta = (-2.0 * PI * i as f64 / n as f64) as f32;
                expj(if inverse { -theta } else { theta })
            })
            .collect();
        Ok(ExactFft {
            n,
            inverse,
            stages,
            tw,
            scratch: vec![Complex32::new(0.0, 0.0); n],
        })
    }

    /// The size.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Always false. A transform has at least 2 points.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Transforms `buf` in place. Nothing is allocated. Errors if `buf` does not hold [`ExactFft::len`] values.
    pub fn process(&mut self, buf: &mut [Complex32]) -> Result<(), DspError> {
        if buf.len() != self.n {
            return Param::Input.length(self.n, buf.len());
        }
        let mut input = std::mem::take(&mut self.scratch);
        input.copy_from_slice(buf);
        self.work(buf, 0, &input, 0, 1, 0);
        self.scratch = input;
        Ok(())
    }

    fn work(
        &self,
        out: &mut [Complex32],
        oo: usize,
        x: &[Complex32],
        xo: usize,
        s: usize,
        stage: usize,
    ) {
        let (p, m) = self.stages[stage];
        if m == 1 {
            for j in 0..p {
                out[oo + j] = x[xo + j * s];
            }
        } else {
            for j in 0..p {
                self.work(out, oo + j * m, x, xo + j * s, s * p, stage + 1);
            }
        }
        if p == 2 {
            self.radix2(out, oo, s, m);
        } else {
            self.radix4(out, oo, s, m);
        }
    }

    fn radix2(&self, out: &mut [Complex32], o: usize, s: usize, m: usize) {
        for k in 0..m {
            let t = cmul(out[o + k + m], self.tw[k * s]);
            out[o + k + m] = out[o + k] - t;
            out[o + k] += t;
        }
    }

    fn radix4(&self, out: &mut [Complex32], o: usize, s: usize, m: usize) {
        for k in 0..m {
            let s0 = cmul(out[o + k + m], self.tw[k * s]);
            let s1 = cmul(out[o + k + 2 * m], self.tw[2 * k * s]);
            let s2 = cmul(out[o + k + 3 * m], self.tw[3 * k * s]);
            let s5 = out[o + k] - s1;
            out[o + k] += s1;
            let s3 = s0 + s2;
            let s4 = s0 - s2;
            out[o + k + 2 * m] = out[o + k] - s3;
            out[o + k] += s3;
            let (a, b) = (
                Complex32::new(s5.re + s4.im, s5.im - s4.re),
                Complex32::new(s5.re - s4.im, s5.im + s4.re),
            );
            if self.inverse {
                out[o + k + m] = b;
                out[o + k + 3 * m] = a;
            } else {
                out[o + k + m] = a;
                out[o + k + 3 * m] = b;
            }
        }
    }
}

/// A real-input FFT of `2 N` points through a complex FFT of `N` points, with a fixed operation order. The forward transform
/// takes `2 N` reals and returns `N + 1` bins. The inverse takes `N + 1` bins, ignores the imaginary parts of the first and last
/// bin, and returns `2 N` reals. Both are unnormalised.
#[derive(Clone, Debug)]
pub struct ExactRealFft {
    cfft: ExactFft,
    super_tw: Vec<Complex32>,
    tmp: Vec<Complex32>,
}

impl ExactRealFft {
    /// A transform of `n` real points. Errors unless `n` is a power of two from 4 to 2^21.
    ///
    /// ```
    /// use sdrkit_algo_dsp::fft_exact::ExactRealFft;
    /// use sdrkit_algo_dsp::Complex32;
    /// let mut f = ExactRealFft::new(8, false).unwrap();
    /// let mut bins = [Complex32::new(0.0, 0.0); 5];
    /// f.forward(&[1.0; 8], &mut bins).unwrap();
    /// assert_eq!(bins[0], Complex32::new(8.0, 0.0));
    /// ```
    pub fn new(n: usize, inverse: bool) -> Result<Self, DspError> {
        if !n.is_power_of_two() || !(4..=1 << 21).contains(&n) {
            return Param::FftSize.not_allowed(n as f64);
        }
        let nc = n / 2;
        let super_tw = (0..nc / 2)
            .map(|i| {
                let frac = (i + 1) as f32 / nc as f32;
                let phi = (-PI * (frac as f64 + 0.5)) as f32;
                expj(if inverse { -phi } else { phi })
            })
            .collect();
        Ok(ExactRealFft {
            cfft: ExactFft::new(nc, inverse)?,
            super_tw,
            tmp: vec![Complex32::new(0.0, 0.0); nc],
        })
    }

    /// The number of real points.
    pub fn len(&self) -> usize {
        2 * self.cfft.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The forward transform of `x` (`len()` reals) into `bins` (`len() / 2 + 1` values). Errors on a wrong length or if the
    /// transform was made for the inverse direction.
    pub fn forward(&mut self, x: &[f32], bins: &mut [Complex32]) -> Result<(), DspError> {
        let nc = self.cfft.len();
        if self.cfft.inverse {
            return Param::FftSize.conflict(Param::Input);
        }
        if x.len() != 2 * nc {
            return Param::Input.length(2 * nc, x.len());
        }
        if bins.len() != nc + 1 {
            return Param::Bins.length(nc + 1, bins.len());
        }
        for (z, p) in self.tmp.iter_mut().zip(x.as_chunks::<2>().0) {
            *z = Complex32::new(p[0], p[1]);
        }
        self.cfft.process(&mut self.tmp)?;
        let z0 = self.tmp[0];
        bins[0] = Complex32::new(z0.re + z0.im, 0.0);
        bins[nc] = Complex32::new(z0.re - z0.im, 0.0);
        for k in 1..=nc / 2 {
            let a = self.tmp[k];
            let b = self.tmp[nc - k].conj();
            let f1 = a + b;
            let f2 = a - b;
            let t = cmul(f2, self.super_tw[k - 1]);
            bins[k] = Complex32::new((f1.re + t.re) * 0.5, (f1.im + t.im) * 0.5);
            bins[nc - k] = Complex32::new((f1.re - t.re) * 0.5, (t.im - f1.im) * 0.5);
        }
        Ok(())
    }

    /// The inverse transform of `bins` (`len() / 2 + 1` values) into `x` (`len()` reals). Errors on a wrong length or if the
    /// transform was made for the forward direction.
    pub fn inverse(&mut self, bins: &[Complex32], x: &mut [f32]) -> Result<(), DspError> {
        let nc = self.cfft.len();
        if !self.cfft.inverse {
            return Param::FftSize.conflict(Param::Output);
        }
        if x.len() != 2 * nc {
            return Param::Output.length(2 * nc, x.len());
        }
        if bins.len() != nc + 1 {
            return Param::Bins.length(nc + 1, bins.len());
        }
        self.tmp[0] = Complex32::new(bins[0].re + bins[nc].re, bins[0].re - bins[nc].re);
        for k in 1..=nc / 2 {
            let a = bins[k];
            let b = bins[nc - k].conj();
            let e = a + b;
            let d = a - b;
            let o = cmul(d, self.super_tw[k - 1]);
            self.tmp[k] = e + o;
            let r = e - o;
            self.tmp[nc - k] = Complex32::new(r.re, -r.im);
        }
        self.cfft.process(&mut self.tmp)?;
        for (p, t) in x.as_chunks_mut::<2>().0.iter_mut().zip(self.tmp.iter()) {
            p[0] = t.re;
            p[1] = t.im;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The DFT by its definition, in f64.
    fn dft(x: &[Complex32], inverse: bool) -> Vec<(f64, f64)> {
        let n = x.len();
        let sign = if inverse { 1.0 } else { -1.0 };
        (0..n)
            .map(|k| {
                x.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, v)| {
                    let a = sign * 2.0 * PI * ((k * i) % n) as f64 / n as f64;
                    let (s, c) = a.sin_cos();
                    (
                        re + v.re as f64 * c - v.im as f64 * s,
                        im + v.re as f64 * s + v.im as f64 * c,
                    )
                })
            })
            .collect()
    }

    fn signal(n: usize) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                Complex32::new(
                    ((i * 37 % 101) as f32 - 50.0) / 10.0,
                    ((i * 59 % 97) as f32 - 48.0) / 10.0,
                )
            })
            .collect()
    }

    #[test]
    fn complex_transform_agrees_with_the_definition() {
        for n in [2, 4, 8, 32, 128, 256, 512] {
            for inverse in [false, true] {
                let x = signal(n);
                let want = dft(&x, inverse);
                let mut got = x.clone();
                ExactFft::new(n, inverse)
                    .unwrap()
                    .process(&mut got)
                    .unwrap();
                for (g, w) in got.iter().zip(&want) {
                    assert!(
                        (g.re as f64 - w.0).abs() < 1e-3 * n as f64
                            && (g.im as f64 - w.1).abs() < 1e-3 * n as f64
                    );
                }
            }
        }
    }

    #[test]
    fn real_transform_agrees_with_the_complex_one_and_inverts() {
        let n = 512;
        let x: Vec<f32> = signal(n).iter().map(|c| c.re).collect();
        let xc: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v, 0.0)).collect();
        let want = dft(&xc, false);
        let mut bins = vec![Complex32::new(0.0, 0.0); n / 2 + 1];
        ExactRealFft::new(n, false)
            .unwrap()
            .forward(&x, &mut bins)
            .unwrap();
        for (g, w) in bins.iter().zip(&want) {
            assert!((g.re as f64 - w.0).abs() < 0.05 && (g.im as f64 - w.1).abs() < 0.05);
        }
        let mut back = vec![0.0f32; n];
        ExactRealFft::new(n, true)
            .unwrap()
            .inverse(&bins, &mut back)
            .unwrap();
        for (b, v) in back.iter().zip(&x) {
            assert!((b / n as f32 - v).abs() < 1e-4);
        }
    }

    #[test]
    fn the_same_input_returns_the_same_bits() {
        let x = signal(512);
        let mut a = x.clone();
        let mut b = x;
        ExactFft::new(512, false).unwrap().process(&mut a).unwrap();
        ExactFft::new(512, false).unwrap().process(&mut b).unwrap();
        assert!(a
            .iter()
            .zip(&b)
            .all(|(u, v)| u.re.to_bits() == v.re.to_bits() && u.im.to_bits() == v.im.to_bits()));
    }

    #[test]
    fn bad_sizes_and_directions_are_errors() {
        assert!(ExactFft::new(12, false).is_err());
        assert!(ExactFft::new(1, false).is_err());
        let mut f = ExactFft::new(8, false).unwrap();
        assert!(f.process(&mut [Complex32::new(0.0, 0.0); 4]).is_err());
        let mut r = ExactRealFft::new(8, false).unwrap();
        assert!(r
            .inverse(&[Complex32::new(0.0, 0.0); 5], &mut [0.0; 8])
            .is_err());
        assert!(r
            .forward(&[0.0; 6], &mut [Complex32::new(0.0, 0.0); 5])
            .is_err());
    }
}
