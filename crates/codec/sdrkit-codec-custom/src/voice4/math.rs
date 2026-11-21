//! The voice codec 4 arithmetic: the elementary functions as series, the noise generator, the FFT and the MDCT.
//!
//! These functions are part of the voice codec 4 bitstream. The encoder decisions depend on their last bits, so they do not use the
//! platform functions or the shared FFT. Do not use them outside voice codec 4.

#![allow(clippy::excessive_precision, clippy::approx_constant)]

const INV_TWO_PI: f64 = 0.15915494309189533577;
const TWO_PI_HI: f64 = 6.28318530717958623200;
const TWO_PI_LO: f64 = 2.44929359829470635445e-16;
pub(crate) const PI: f64 = std::f64::consts::PI;
const HALF_PI: f64 = 1.57079632679489661923;
const LOG2_E: f64 = 1.44269504088896340736;
const LN_2: f64 = 0.69314718055994530942;
const LOG10_2: f64 = 0.30102999566398119521;
const LOG2_10: f64 = 3.32192809488736234787;
const SQRT_HALF: f64 = 0.70710678118654752440;

fn reduce(x: f64) -> f64 {
    let q = (x * INV_TWO_PI + 0.5).floor();
    (x - q * TWO_PI_HI) - q * TWO_PI_LO
}

fn sin_series(r: f64) -> f64 {
    let r2 = r * r;
    let mut s = r;
    let mut t = r;
    for k in 1..=11u32 {
        let d = ((2 * k) * (2 * k + 1)) as f64;
        t = t * -r2 / d;
        s += t;
    }
    s
}

/// The sine of `x` by the series of section 2.1.
pub(crate) fn sin(x: f64) -> f64 {
    let mut r = reduce(x);
    if r > HALF_PI {
        r = PI - r;
    } else if r < -HALF_PI {
        r = -PI - r;
    }
    sin_series(r)
}

/// The cosine of `x` by the series of section 2.1.
pub(crate) fn cos(x: f64) -> f64 {
    let r = reduce(x);
    sin_series(HALF_PI - r.abs())
}

/// Splits a positive finite `x` into `m * 2^e` with `m` in [0.5, 1).
fn frexp(x: f64) -> (f64, i32) {
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7FF) as i32;
    if exp == 0 {
        // Subnormal: scale up by 2^64 first.
        let (m, e) = frexp(x * f64::from_bits((1023u64 + 64) << 52));
        return (m, e - 64);
    }
    let m = f64::from_bits((bits & !(0x7FFu64 << 52)) | (1022u64 << 52));
    (m, exp - 1022)
}

/// Multiplies `s` by `2^n`.
fn ldexp(s: f64, n: i32) -> f64 {
    if n > 1023 {
        ldexp(s * f64::from_bits(2046u64 << 52), n - 1023)
    } else if n >= -1022 {
        s * f64::from_bits(((n + 1023) as u64) << 52)
    } else {
        ldexp(s * f64::from_bits(1u64 << 52), n + 1022)
    }
}

/// The base-2 logarithm of `x` by the series of section 2.1. If `x` is not above 0, it returns -1100.
pub(crate) fn log2(x: f64) -> f64 {
    if !(x > 0.0) {
        return -1100.0;
    }
    if x.is_infinite() {
        return 1024.0;
    }
    let (mut m, mut e) = frexp(x);
    if m < SQRT_HALF {
        m *= 2.0;
        e -= 1;
    }
    let s = (m - 1.0) / (m + 1.0);
    let s2 = s * s;
    let mut sum = s;
    let mut t = s;
    for k in 1..=12u32 {
        t *= s2;
        sum += t / (2 * k + 1) as f64;
    }
    e as f64 + 2.0 * sum * LOG2_E
}

/// 2 to the power `x` by the series of section 2.1.
pub(crate) fn exp2(x: f64) -> f64 {
    if x < -1070.0 {
        return 0.0;
    }
    let x = if x > 1020.0 { 1020.0 } else { x };
    if x.is_nan() {
        return f64::NAN;
    }
    let n = x.floor();
    let y = (x - n) * LN_2;
    let mut s = 1.0;
    let mut t = 1.0;
    for k in 1..=20u32 {
        t = t * y / k as f64;
        s += t;
    }
    ldexp(s, n as i32)
}

/// The base-10 logarithm of `x`.
pub(crate) fn log10(x: f64) -> f64 {
    log2(x) * LOG10_2
}

/// 10 to the power `x`.
pub(crate) fn pow10(x: f64) -> f64 {
    exp2(x * LOG2_10)
}

/// The level of a power `p` in dB: 10 log10(max(p, 1e-10)).
pub(crate) fn db(p: f64) -> f64 {
    10.0 * log10(if p > 1e-10 { p } else { 1e-10 })
}

/// The power of a level `d` in dB: 10^(d/10).
pub(crate) fn lin(d: f64) -> f64 {
    pow10(d / 10.0)
}

/// The 32-bit linear congruential noise generator of section 2.2.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lcg(pub u32);

impl Lcg {
    /// Steps the generator and returns the new state.
    #[inline]
    pub(crate) fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }

    /// A uniform value in [-1, 1).
    #[inline]
    pub(crate) fn uni(&mut self) -> f32 {
        (self.next() as i32) as f32 * (1.0 / 2_147_483_648.0)
    }

    /// A Gaussian value: the sum of four uniform values times 0.8660254.
    #[inline]
    pub(crate) fn gauss(&mut self) -> f32 {
        let a = self.uni();
        let b = self.uni();
        let c = self.uni();
        let d = self.uni();
        (((a + b) + c) + d) * 0.866_025_4
    }
}

/// The in-place complex FFT of section 2.3 for sizes 128 and 256, unscaled in both directions.
pub(crate) struct Fft {
    n: usize,
    // Twiddles for every stage, in stage order: for len = 2, 4, .., n, the len/2 values of cos and sin of (-2 pi / len) k.
    wr: Vec<f32>,
    wi: Vec<f32>,
}

impl Fft {
    /// Makes the tables for size `n` (a power of two).
    pub(crate) fn new(n: usize) -> Self {
        let mut wr = Vec::with_capacity(n);
        let mut wi = Vec::with_capacity(n);
        let mut len = 2;
        while len <= n {
            let ang = -2.0 * PI / len as f64;
            for k in 0..len / 2 {
                wr.push(cos(ang * k as f64) as f32);
                wi.push(sin(ang * k as f64) as f32);
            }
            len *= 2;
        }
        Self { n, wr, wi }
    }

    /// Transforms `re` and `im` in place. `inverse` selects the sign +2 pi / len.
    pub(crate) fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        let n = self.n;
        let mut j = 0usize;
        for i in 0..n {
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
            let mut bit = n >> 1;
            while bit > 0 && j & bit != 0 {
                j ^= bit;
                bit >>= 1;
            }
            j |= bit;
        }
        let mut len = 2;
        let mut base = 0;
        while len <= n {
            let half = len / 2;
            for k in 0..half {
                let wr = self.wr[base + k];
                let wi = if inverse {
                    -self.wi[base + k]
                } else {
                    self.wi[base + k]
                };
                let mut i = k;
                while i < n {
                    let j = i + half;
                    let xr = re[j] * wr - im[j] * wi;
                    let xi = re[j] * wi + im[j] * wr;
                    re[j] = re[i] - xr;
                    im[j] = im[i] - xi;
                    re[i] += xr;
                    im[i] += xi;
                    i += len;
                }
            }
            base += half;
            len *= 2;
        }
    }
}

/// The MDCT of section 3.6.1: block 80, window 160, float tables.
pub(crate) struct Mdct {
    win: [f32; 160],
    // cs[n * 80 + k]
    cs: Vec<f32>,
}

impl Mdct {
    pub(crate) fn new() -> Self {
        let mut win = [0f32; 160];
        for (n, w) in win.iter_mut().enumerate() {
            *w = sin(PI * (n as f64 + 0.5) / 160.0) as f32;
        }
        let scale = (2.0f64 / 80.0).sqrt();
        let mut cs = vec![0f32; 160 * 80];
        for n in 0..160 {
            for k in 0..80 {
                cs[n * 80 + k] =
                    (scale * cos(PI / 80.0 * (n as f64 + 0.5 + 40.0) * (k as f64 + 0.5))) as f32;
            }
        }
        Self { win, cs }
    }

    /// Forward transform of 160 samples into 80 bins.
    pub(crate) fn forward(&self, x: &[f32], out: &mut [f32; 80]) {
        let mut xw = [0f32; 160];
        for n in 0..160 {
            xw[n] = x[n] * self.win[n];
        }
        for (k, o) in out.iter_mut().enumerate() {
            let mut acc = 0f32;
            for n in 0..160 {
                acc += xw[n] * self.cs[n * 80 + k];
            }
            *o = acc;
        }
    }

    /// Inverse transform of 80 bins, overlap-added into `ola`. Writes 80 output samples.
    pub(crate) fn inverse_ola(&self, x: &[f32; 80], ola: &mut [f32; 80], out: &mut [f32]) {
        let mut y = [0f32; 160];
        for (n, yv) in y.iter_mut().enumerate() {
            let row = &self.cs[n * 80..n * 80 + 80];
            let mut acc = 0f32;
            for k in 0..80 {
                acc += x[k] * row[k];
            }
            *yv = acc * self.win[n];
        }
        for n in 0..80 {
            out[n] = ola[n] + y[n];
            ola[n] = y[80 + n];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_agree_with_the_platform() {
        for i in -200..200 {
            let x = i as f64 * 0.137;
            assert!((sin(x) - x.sin()).abs() < 1e-14, "sin {x}");
            assert!((cos(x) - x.cos()).abs() < 1e-14, "cos {x}");
        }
        for &x in &[1e-12, 0.3, 1.0, 2.0, 7.5, 1e5, 3e20] {
            assert!((log2(x) - f64::log2(x)).abs() < 1e-12, "log2 {x}");
        }
        for i in -100..100 {
            let x = i as f64 * 0.731;
            assert!((exp2(x) / x.exp2() - 1.0).abs() < 1e-14, "exp2 {x}");
        }
        assert_eq!(log2(0.0), -1100.0);
        assert_eq!(exp2(-2000.0), 0.0);
        assert!((db(1.0)).abs() < 1e-12);
        assert!((db(0.0) + 100.0).abs() < 1e-9);
    }

    #[test]
    fn lcg_known_values() {
        // Section 2.2: s = s * 1664525 + 1013904223 mod 2^32. From 0, the first state is 1013904223.
        let mut g = Lcg(0);
        assert_eq!(g.next(), 1_013_904_223);
        assert_eq!(g.next(), 1_196_435_762);
    }

    #[test]
    fn fft_matches_a_direct_dft() {
        let f = Fft::new(128);
        let mut re: Vec<f32> = (0..128).map(|i| ((i * 37 % 11) as f32) - 5.0).collect();
        let mut im = vec![0f32; 128];
        let x = re.clone();
        f.run(&mut re, &mut im, false);
        for k in [0usize, 1, 5, 64, 127] {
            let (mut a, mut b) = (0f64, 0f64);
            for (n, v) in x.iter().enumerate() {
                let p = -2.0 * PI * (k * n) as f64 / 128.0;
                a += *v as f64 * p.cos();
                b += *v as f64 * p.sin();
            }
            assert!((re[k] as f64 - a).abs() < 1e-3 && (im[k] as f64 - b).abs() < 1e-3);
        }
        f.run(&mut re, &mut im, true);
        for n in 0..128 {
            assert!((re[n] / 128.0 - x[n]).abs() < 1e-4);
        }
    }
}
