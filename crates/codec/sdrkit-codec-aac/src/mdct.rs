//! The modified discrete cosine transform of the AAC filterbank and its inverse, by a complex FFT of a quarter of the window
//! length.
//!
//! Standard: ISO/IEC 14496-3 clause 4.6.11.

use crate::{Error, Result};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::Complex32 as C;

/// An MDCT of 2N inputs to N outputs, and its inverse, by an N/2-point complex FFT (N even). It allocates nothing after
/// [`Mdct::new`].
#[derive(Clone, Debug)]
pub struct Mdct {
    fft: Fft,
    pre: Vec<C>,
    post: Vec<C>,
    v: Vec<f32>,
    u: Vec<f32>,
    c: Vec<C>,
}

impl Mdct {
    /// An MDCT of `n` outputs. `n` must be even.
    pub fn new(n: usize) -> Self {
        let n = n.max(2) & !1;
        let m = n / 2;
        let pre = (0..m)
            .map(|i| C::from_polar(1.0, -(std::f64::consts::PI * i as f64 / n as f64) as f32))
            .collect();
        let post = (0..m)
            .map(|p| {
                C::from_polar(
                    1.0,
                    -(std::f64::consts::PI * (4 * p + 1) as f64 / (4 * n) as f64) as f32,
                )
            })
            .collect();
        Mdct {
            fft: Fft::forward(m),
            pre,
            post,
            v: vec![0.0; n],
            u: vec![0.0; n],
            c: vec![C::default(); m],
        }
    }

    /// `X[k] = sum of z[n] cos(pi/N (n + 1/2 + N/2)(k + 1/2))` over the 2N inputs `z` (ISO/IEC 14496-3 clause 4.6.11.3.2, the
    /// inverse of the IMDCT there up to scale). `out` gets the N outputs. Extra inputs and outputs are ignored.
    ///
    /// # Errors
    /// [`Error::InputLength`] when `z` has fewer than 2N values or `out` fewer than N.
    pub fn forward(&mut self, z: &[f32], out: &mut [f32]) -> Result<()> {
        let n = self.v.len();
        if z.len() < 2 * n || out.len() < n {
            return Err(Error::InputLength);
        }
        let h = n / 2;
        for i in 0..h {
            self.v[i] = -z[3 * h - 1 - i] - z[3 * h + i];
            self.v[h + i] = z[i] - z[n - 1 - i];
        }
        Self::dct4(
            &mut self.fft,
            &self.pre,
            &self.post,
            &self.v,
            &mut self.c,
            out,
        );
        Ok(())
    }

    /// The inverse: `y[n] = sum of X[k] cos(pi/N (n + 1/2 + N/2)(k + 1/2))` over the N inputs `x`, for the 2N outputs `out`
    /// (ISO/IEC 14496-3 clause 4.6.11.3.2 without its factor 2 / window length). Extra inputs and outputs are ignored.
    ///
    /// # Errors
    /// [`Error::InputLength`] when `x` has fewer than N values or `out` fewer than 2N.
    pub fn inverse(&mut self, x: &[f32], out: &mut [f32]) -> Result<()> {
        let n = self.v.len();
        if x.len() < n || out.len() < 2 * n {
            return Err(Error::InputLength);
        }
        let h = n / 2;
        Self::dct4(
            &mut self.fft,
            &self.pre,
            &self.post,
            &x[..n],
            &mut self.c,
            &mut self.u,
        );
        // The transpose of the folding of `forward`.
        for i in 0..h {
            let a = self.u[i];
            out[3 * h - 1 - i] = -a;
            out[3 * h + i] = -a;
            let b = self.u[h + i];
            out[i] = b;
            out[n - 1 - i] = -b;
        }
        Ok(())
    }

    /// The DCT-IV of `v` into `out`, `out[k] = sum of v[j] cos(pi/N (j + 1/2)(k + 1/2))`.
    fn dct4(fft: &mut Fft, pre: &[C], post: &[C], v: &[f32], c: &mut [C], out: &mut [f32]) {
        let n = v.len();
        let h = n / 2;
        for m in 0..h {
            c[m] = C::new(v[2 * m], v[n - 1 - 2 * m]) * pre[m];
        }
        fft.process(c);
        for p in 0..h {
            let y = c[p] * post[p];
            out[2 * p] = y.re;
            out[n - 1 - 2 * p] = -y.im;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdct_matches_the_definition() {
        let n = 16;
        let mut m = Mdct::new(n);
        let z: Vec<f32> = (0..2 * n)
            .map(|i| ((i * 7 + 3) % 11) as f32 - 5.0)
            .collect();
        let mut out = vec![0f32; n];
        m.forward(&z, &mut out).unwrap();
        for (k, &o) in out.iter().enumerate() {
            let d: f64 = (0..2 * n)
                .map(|i| {
                    z[i] as f64
                        * (std::f64::consts::PI / n as f64
                            * (i as f64 + 0.5 + n as f64 / 2.0)
                            * (k as f64 + 0.5))
                            .cos()
                })
                .sum();
            assert!((d - o as f64).abs() < 1e-3, "{k}: {d} {o}");
        }
    }

    #[test]
    fn inverse_matches_the_definition() {
        for n in [16, 120, 128] {
            let mut m = Mdct::new(n);
            let x: Vec<f32> = (0..n).map(|i| ((i * 5 + 1) % 7) as f32 - 3.0).collect();
            let mut out = vec![0f32; 2 * n];
            m.inverse(&x, &mut out).unwrap();
            for (i, &o) in out.iter().enumerate() {
                let d: f64 = (0..n)
                    .map(|k| {
                        x[k] as f64
                            * (std::f64::consts::PI / n as f64
                                * (i as f64 + 0.5 + n as f64 / 2.0)
                                * (k as f64 + 0.5))
                                .cos()
                    })
                    .sum();
                assert!((d - o as f64).abs() < 1e-3 * n as f64, "{n} {i}: {d} {o}");
            }
        }
    }
}
