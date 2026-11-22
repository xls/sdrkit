//! The parameters of one 20 ms frame of the multi-band excitation speech model, shared by both coders, and the transforms
//! that both quantisers use: the block DCT and the log-amplitude prediction.

use std::f64::consts::PI;

/// Samples per frame (20 ms at 8 kHz).
pub const FRAME: usize = 160;

/// The largest number of harmonics.
pub const MAX_L: usize = 56;

/// The smallest number of harmonics.
pub const MIN_L: usize = 9;

/// The largest number of voicing bands (full rate).
pub const MAX_K: usize = 12;

/// The model parameters of one frame.
///
/// Arrays are indexed by harmonic number `l = 1..=l`; index 0 and the places above `l` are not used and hold 0 or false.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The fundamental in radians per sample.
    pub w0: f64,
    /// The number of harmonics, 0 to [`MAX_L`].
    pub l: usize,
    /// The voicing decision of each harmonic.
    pub voiced: [bool; MAX_L + 1],
    /// The spectral amplitude of each harmonic, in sample units: a voiced harmonic of amplitude `M` is the cosine `2 M cos`.
    pub amp: [f64; MAX_L + 1],
}

impl Params {
    /// A frame with no harmonic sound.
    pub const fn silent(w0: f64, l: usize) -> Self {
        Params {
            w0,
            l,
            voiced: [false; MAX_L + 1],
            amp: [0.0; MAX_L + 1],
        }
    }

    /// The fundamental in Hz at 8 kHz.
    pub fn f0_hz(&self) -> f64 {
        self.w0 * 8000.0 / (2.0 * PI)
    }
}

/// The voicing band of harmonic `l` (Equations 50 and 34 of TIA-102.BABA): 1-based, 3 harmonics per band, band 12 above 36.
pub fn band_of(l: usize) -> usize {
    if l <= 36 {
        l.div_ceil(3)
    } else {
        12
    }
}

/// The number of voicing bands for `l` harmonics (Equation 34): the band of the highest harmonic.
pub fn bands_for(l: usize) -> usize {
    band_of(l)
}

/// The DCT of one block (Equation 60): `out[k - 1] = (1 / J) sum_j c_j cos(pi (k - 1)(j - 1/2) / J)` for `k = 1..=J`,
/// `J = c.len() = out.len()`.
pub fn dct(c: &[f64], out: &mut [f64]) {
    let j = c.len() as f64;
    for (k, o) in out.iter_mut().enumerate() {
        let mut s = 0.0;
        for (n, &x) in c.iter().enumerate() {
            s += x * (PI * k as f64 * (n as f64 + 0.5) / j).cos();
        }
        *o = s / j;
    }
}

/// The inverse of [`dct`] (Equation 73): `out[j - 1] = sum_k alpha(k) C_k cos(pi (k - 1)(j - 1/2) / J)` with
/// `alpha(1) = 1`, `alpha(k) = 2` above.
pub fn idct(cf: &[f64], out: &mut [f64]) {
    let j = cf.len() as f64;
    for (n, o) in out.iter_mut().enumerate() {
        let mut s = 0.0;
        for (k, &x) in cf.iter().enumerate() {
            let a = if k == 0 { 1.0 } else { 2.0 };
            s += a * x * (PI * k as f64 * (n as f64 + 0.5) / j).cos();
        }
        *o = s;
    }
}

/// The mean-removed prediction of the log amplitudes of a frame with `cur_l` harmonics from the previous frame's values
/// `prev[1..=prev_l]` (Equations 52 to 57 of TIA-102.BABA, 11 to 15 of TIA-102.BABA-1): `out[l] = rho P_l - (rho / L)
/// sum P`, where `P_l` interpolates `prev` at `l prev_l / cur_l`. `prev[0]` is the value below harmonic 1 and the values
/// above `prev_l` repeat `prev[prev_l]`. The residual is the log amplitude minus `out[l]`.
pub fn predict(prev: &[f64; MAX_L + 2], prev_l: usize, cur_l: usize, rho: f64, out: &mut [f64; MAX_L + 1]) {
    let at = |i: usize| -> f64 {
        if i > prev_l {
            prev[prev_l]
        } else {
            prev[i]
        }
    };
    let ratio = prev_l as f64 / cur_l.max(1) as f64;
    let mut sum = 0.0;
    for l in 1..=cur_l {
        let k = ratio * l as f64;
        let i = k.floor();
        let d = k - i;
        let p = (1.0 - d) * at(i as usize) + d * at(i as usize + 1);
        out[l] = p;
        sum += p;
    }
    let mean = sum / cur_l.max(1) as f64;
    for o in out.iter_mut().take(cur_l + 1).skip(1) {
        *o = rho * (*o - mean);
    }
    out[0] = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dct_inverts() {
        for n in 1..=17 {
            let c: Vec<f64> = (0..n).map(|i| ((i * 7 + 3) % 11) as f64 - 5.0).collect();
            let mut f = vec![0.0; n];
            let mut back = vec![0.0; n];
            dct(&c, &mut f);
            idct(&f, &mut back);
            for (a, b) in c.iter().zip(&back) {
                assert!((a - b).abs() < 1e-9, "n = {n}");
            }
        }
    }

    #[test]
    fn band_mapping() {
        assert_eq!(band_of(1), 1);
        assert_eq!(band_of(3), 1);
        assert_eq!(band_of(4), 2);
        assert_eq!(band_of(36), 12);
        assert_eq!(band_of(56), 12);
        assert_eq!(bands_for(9), 3);
        assert_eq!(bands_for(34), 12);
        assert_eq!(bands_for(56), 12);
    }
}
