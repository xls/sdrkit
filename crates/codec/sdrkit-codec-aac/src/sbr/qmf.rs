//! The QMF banks of SBR: the 32-band analysis of the core output, and the 64-band synthesis (32 bands when SBR runs
//! down-sampled).
//!
//! Standard: ISO/IEC 14496-3 clauses 4.6.18.4.1 and 4.6.18.4.2.

use super::tables::QMF_WINDOW;
use sdrkit_algo_dsp::Complex32 as C;
use std::f64::consts::PI;
use std::sync::OnceLock;

/// The modulation of the analysis bank: `2 exp(i pi/64 (k + 1/2)(2n - 1/2))` by band `k` (32) and tap `n` (64).
fn analysis_kernel() -> &'static [C] {
    static T: OnceLock<Vec<C>> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = Vec::with_capacity(32 * 64);
        for k in 0..32 {
            for n in 0..64 {
                let a = PI / 64.0 * (k as f64 + 0.5) * (2.0 * n as f64 - 0.5);
                t.push(C::new((2.0 * a.cos()) as f32, (2.0 * a.sin()) as f32));
            }
        }
        t
    })
}

/// The modulation of a synthesis bank of `m` bands (64 or 32): `exp(i pi/(2m) (k + 1/2)(2n - (4m - 1)))` divided by 64, by
/// output `n` (2m) and band `k` (m).
fn synthesis_kernel(m: usize) -> &'static [C] {
    static T64: OnceLock<Vec<C>> = OnceLock::new();
    static T32: OnceLock<Vec<C>> = OnceLock::new();
    let make = || {
        let mut t = Vec::with_capacity(2 * m * m);
        for n in 0..2 * m {
            for k in 0..m {
                let a =
                    PI / (2 * m) as f64 * (k as f64 + 0.5) * (2.0 * n as f64 - (4 * m - 1) as f64);
                t.push(C::new((a.cos() / 64.0) as f32, (a.sin() / 64.0) as f32));
            }
        }
        t
    };
    if m == 64 {
        T64.get_or_init(make)
    } else {
        T32.get_or_init(make)
    }
}

/// The 32-band analysis bank of one channel.
#[derive(Clone, Debug)]
pub(crate) struct Analysis {
    x: [f32; 320],
}

impl Default for Analysis {
    fn default() -> Self {
        Analysis { x: [0.0; 320] }
    }
}

impl Analysis {
    /// Reads 32 samples and writes the 32 subband samples of one time slot to `out`.
    pub(crate) fn run(&mut self, input: &[f32], out: &mut [C]) {
        self.x.copy_within(0..288, 32);
        for (i, &v) in input[..32].iter().enumerate() {
            self.x[31 - i] = v;
        }
        let mut u = [0f32; 64];
        for (n, un) in u.iter_mut().enumerate() {
            let mut s = 0.0;
            for j in 0..5 {
                let i = n + 64 * j;
                s += self.x[i] * QMF_WINDOW[2 * i];
            }
            *un = s;
        }
        let kern = analysis_kernel();
        for (k, o) in out[..32].iter_mut().enumerate() {
            let row = &kern[k * 64..(k + 1) * 64];
            let (mut re, mut im) = (0.0, 0.0);
            for (&un, w) in u.iter().zip(row) {
                re += un * w.re;
                im += un * w.im;
            }
            *o = C::new(re, im);
        }
    }
}

/// A synthesis bank of 64 or 32 bands for one channel.
#[derive(Clone, Debug)]
pub(crate) struct Synthesis {
    m: usize,
    v: Vec<f32>,
}

impl Synthesis {
    /// A bank of `m` bands (64, or 32 down-sampled).
    pub(crate) fn new(m: usize) -> Synthesis {
        Synthesis {
            m,
            v: vec![0.0; 20 * m],
        }
    }

    /// Reads the subband samples of one time slot (`m` values) and writes `m` output samples.
    pub(crate) fn run(&mut self, x: &[C], out: &mut [f32]) {
        let m = self.m;
        let len = self.v.len();
        self.v.copy_within(0..len - 2 * m, 2 * m);
        let kern = synthesis_kernel(m);
        for n in 0..2 * m {
            let row = &kern[n * m..(n + 1) * m];
            let mut s = 0.0;
            for (xk, w) in x[..m].iter().zip(row) {
                s += xk.re * w.re - xk.im * w.im;
            }
            self.v[n] = s;
        }
        let step = 640 / (10 * m);
        for (k, o) in out[..m].iter_mut().enumerate() {
            let mut s = 0.0;
            for j in 0..5 {
                // g[2m j + k] = v[4m j + k] and g[2m j + m + k] = v[4m j + 3m + k].
                s += self.v[4 * m * j + k] * QMF_WINDOW[step * (2 * m * j + k)];
                s += self.v[4 * m * j + 3 * m + k] * QMF_WINDOW[step * (2 * m * j + m + k)];
            }
            *o = s;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_then_synthesis_doubles_the_rate() {
        // A tone through the 32-band analysis and the 64-band synthesis with the upper bands empty comes out at twice the
        // rate with its amplitude.
        let mut a = Analysis::default();
        let mut s = Synthesis::new(64);
        let w = std::f32::consts::TAU * 0.03;
        let x: Vec<f32> = (0..32 * 128)
            .map(|i| 1000.0 * (w * i as f32).sin())
            .collect();
        let mut y = vec![0f32; 2 * x.len()];
        let mut sub = [C::default(); 64];
        for (xi, yi) in x.chunks(32).zip(y.chunks_mut(64)) {
            a.run(xi, &mut sub);
            sub[32..].fill(C::default());
            s.run(&sub, yi);
        }
        let (err, d) = (0..1200)
            .map(|d| {
                let e: f32 = (4000..6000)
                    .map(|m| (y[m] - 1000.0 * (w / 2.0 * (m as f32 - d as f32)).sin()).powi(2))
                    .sum();
                (e, d)
            })
            .fold((f32::MAX, 0), |a, b| if b.0 < a.0 { b } else { a });
        let p = 2000.0 * 1000.0 * 1000.0 / 2.0;
        assert!(err / p < 1e-6, "delay {d}: error {}", err / p);
    }
}
