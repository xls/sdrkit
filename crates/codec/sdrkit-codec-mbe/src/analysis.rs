//! Speech analysis (TIA-102.BABA chapter 5): high-pass filter, initial pitch with look-back and look-ahead tracking, pitch
//! refinement, voicing decisions and spectral amplitudes. Both coders use it unchanged.

use crate::model::{bands_for, Params, FRAME, MAX_K, MAX_L};
use crate::tables::full::{H_LPF, W_I, W_R};
use sdrkit_algo_dsp::fft_exact::ExactFft;
use sdrkit_algo_dsp::Complex32;
use std::f64::consts::{PI, TAU};
use std::sync::OnceLock;

/// Candidate pitch periods 21, 21.5, ..., 122 (203 values); candidate `i` is the period `21 + i / 2`.
const NP: usize = 203;
/// Samples kept: the window of the newest look-ahead frame ends at the last one.
const BUF: usize = 5 * FRAME;
/// Centre of the newest (second look-ahead) frame in the buffer.
const C2: usize = BUF - 1 - 150 - 10;
/// Centre of the frame being analysed: two frames before the newest.
const C0: usize = C2 - 2 * FRAME;
/// DFT size of the analysis spectrum.
const N_DFT: usize = 256;
/// Size of the window spectrum W_R.
const N_WR: usize = 16384;
/// E(P) of a frame without energy: no periodicity. The look-ahead frames start with it, as the buffer starts silent.
const E_SILENT: f64 = 1.0;
/// The divisors tried for sub-multiples of the forward pitch, largest first (flow chart 4).
const SUBMULTIPLES: [usize; 4] = [5, 4, 3, 2];

/// The candidate `j` nearest a sub-multiple `P / n`, rounded as P = floor(2 P / n + 1/2) / 2 (flow chart 4, read with both
/// cut-off denominators as 2), or `None` below 21.
fn submultiple(i: usize, n: usize) -> Option<usize> {
    let two_p = 42 + i;
    if two_p < 42 * n {
        return None;
    }
    let j = (2 * two_p + n) / (2 * n);
    Some((j - 42).min(NP - 1))
}

/// Lags of the autocorrelation kept: 0..=152, enough for the 4-point interpolation up to lag 150.
const R_LEN: usize = 153;

/// The autocorrelation r(t) of one frame for t = 0..R_LEN (Equation 7) and its windowed energy; energy 0 when silent.
#[derive(Clone, Copy)]
struct Lags {
    r: [f64; R_LEN],
    energy: f64,
}

/// E(P) of Equation 5 for any period `p`. Lags between samples take r by the linear interpolation of Equation 8, or with
/// `cubic` by 4-point Lagrange interpolation (r(-t) = r(t)), which loses under 1 % of a 1 kHz component halfway between
/// lags where Equation 8 loses about 8 %.
fn e_at(lags: &Lags, p: f64, wi4_sum: f64, cubic: bool) -> f64 {
    if lags.energy <= 0.0 {
        return E_SILENT;
    }
    let r = &lags.r;
    let at = |k: isize| -> f64 { r.get(k.unsigned_abs()).copied().unwrap_or(0.0) };
    let interp = |t: f64| -> f64 {
        let f = t.floor();
        let k = f as isize;
        let d = t - f;
        if d == 0.0 {
            return at(k);
        }
        if !cubic {
            return (1.0 - d) * at(k) + d * at(k + 1);
        }
        let (a, b, c, e) = (at(k - 1), at(k), at(k + 1), at(k + 2));
        b + d * (c - a) / 2.0 + d * d * (a - 2.0 * b + c) / 2.0 + d * (d * d - 1.0) * (e - 3.0 * c + 3.0 * b - a) / 6.0
    };
    let nmax = (150.0 / p).floor() as usize;
    let mut sum = r[0];
    for n in 1..=nmax {
        sum += 2.0 * interp(n as f64 * p);
    }
    (lags.energy - p * sum) / (lags.energy * (1.0 - p * wi4_sum))
}

/// Half-width in samples and step of the search around a sub-multiple in [`Analyser::refined_submultiple`].
const SUB_SPAN: f64 = 0.5;
const SUB_STEP: f64 = 0.125;

/// The candidates within 0.8 P .. 1.2 P of candidate `i`, as an index range.
fn near(i: usize) -> std::ops::RangeInclusive<usize> {
    let two_p = 42 + i;
    let lo = (4 * two_p).div_ceil(5).saturating_sub(42);
    let hi = ((6 * two_p) / 5).saturating_sub(42).min(NP - 1);
    lo..=hi
}

/// W_R(m) for m = 0..=8192 (Equation 30); W_R(-m) = W_R(m).
fn window_spectrum() -> &'static [f64] {
    static WR: OnceLock<Vec<f64>> = OnceLock::new();
    WR.get_or_init(|| {
        let mut fft = ExactFft::new(N_WR, false).expect("power of two");
        let mut buf = vec![Complex32::new(0.0, 0.0); N_WR];
        for n in -110i32..=110 {
            buf[n.rem_euclid(N_WR as i32) as usize] = Complex32::new(W_R[n.unsigned_abs() as usize] as f32, 0.0);
        }
        let _ = fft.process(&mut buf);
        buf[..=N_WR / 2].iter().map(|c| c.re as f64).collect()
    })
}

/// The analysed parameters of one frame beyond those of [`Params`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bands {
    /// The number of voicing bands K.
    pub k: usize,
    /// The voicing decision of each band, `v[1..=k]`.
    pub v: [bool; MAX_K + 1],
}

/// The analysis state.
pub struct Analyser {
    hpf_x: f64,
    hpf_y: f64,
    s: [f64; BUF],
    e: [[f64; NP]; 3],
    lags: [Lags; 3],
    p_m1: usize,
    e_m1: f64,
    e_m2: f64,
    xi_max: f64,
    prev_bands: [bool; MAX_K + 1],
    e_pi: f64,
    m_xi: f64,
    sw: [(f64, f64); N_DFT / 2 + 1],
    wr: &'static [f64],
    fft: ExactFft,
    buf: [Complex32; N_DFT],
    wi2: [f64; 301],
    wi4_sum: f64,
}

impl Default for Analyser {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyser {
    /// An analyser with the initial values of Annex A.
    pub fn new() -> Self {
        let wi2: [f64; 301] = std::array::from_fn(|i| W_I[(i as i32 - 150).unsigned_abs() as usize].powi(2));
        Analyser {
            hpf_x: 0.0,
            hpf_y: 0.0,
            s: [0.0; BUF],
            e: [[E_SILENT; NP]; 3],
            lags: [Lags { r: [0.0; R_LEN], energy: 0.0 }; 3],
            p_m1: 2 * 100 - 42,
            e_m1: 0.0,
            e_m2: 0.0,
            xi_max: 100000.0,
            prev_bands: [false; MAX_K + 1],
            e_pi: 0.0,
            m_xi: 1.0,
            sw: [(0.0, 0.0); N_DFT / 2 + 1],
            wr: window_spectrum(),
            fft: ExactFft::new(N_DFT, false).expect("256 is a power of two"),
            buf: [Complex32::new(0.0, 0.0); N_DFT],
            wi4_sum: wi2.iter().map(|w| w * w).sum(),
            wi2,
        }
    }

    fn w_r(&self, idx: f64) -> f64 {
        let i = (idx.abs() as usize).min(self.wr.len() - 1);
        self.wr[i]
    }

    /// Takes the next 160 samples and returns the refined fundamental (radians per sample) of the frame two frames back.
    pub fn push(&mut self, pcm: &[i16; FRAME]) -> f64 {
        self.s.copy_within(FRAME.., 0);
        for (i, &x) in pcm.iter().enumerate() {
            let x = x as f64;
            let y = x - self.hpf_x + 0.99 * self.hpf_y;
            self.hpf_x = x;
            self.hpf_y = y;
            self.s[BUF - FRAME + i] = y;
        }
        self.e[0] = self.e[1];
        self.e[1] = self.e[2];
        self.lags[0] = self.lags[1];
        self.lags[1] = self.lags[2];
        let mut e2 = [0.0; NP];
        let mut l2 = Lags { r: [0.0; R_LEN], energy: 0.0 };
        self.error_function(C2, &mut e2, &mut l2);
        self.e[2] = e2;
        self.lags[2] = l2;
        let pi = self.initial_pitch();
        self.e_pi = self.e[0][pi];
        self.spectrum();
        self.refine(21.0 + pi as f64 / 2.0)
    }

    /// E(P) of the frame centred at `c` (Equations 5 to 9).
    fn error_function(&self, c: usize, out: &mut [f64; NP], lags: &mut Lags) {
        let mut x = [0.0f64; 301];
        for (j, xv) in x.iter_mut().enumerate() {
            let n = c + j - 150;
            let mut acc = self.s[n] * H_LPF[0];
            for t in 1..=10 {
                acc += (self.s[n - t] + self.s[n + t]) * H_LPF[t];
            }
            *xv = acc;
        }
        let y: [f64; 301] = std::array::from_fn(|j| x[j] * self.wi2[j]);
        let energy: f64 = x.iter().zip(&self.wi2).map(|(a, w)| a * a * w).sum();
        lags.energy = if energy > 1e-9 && energy.is_finite() { energy } else { 0.0 };
        for (t, rv) in lags.r.iter_mut().enumerate() {
            *rv = y[..301 - t].iter().zip(&y[t..]).map(|(a, b)| a * b).sum();
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = e_at(lags, 21.0 + i as f64 / 2.0, self.wi4_sum, false);
        }
    }

    /// The CE_F of the sub-multiple P^_0 / n for the test of Equations 18 to 20 (an unconfirmed correction; the rest of the
    /// search keeps Equations 5 to 8 as printed). In each of the three frames E is taken at its least value within 0.5
    /// sample of P^_0 / n on a 1/8-sample grid, with r between lags by 4-point interpolation. As printed, a period between
    /// half samples is rounded at every one of its multiples in the window and r there is linearly interpolated, while
    /// two or three periods fall near one integer lag; for a 220 Hz vowel CE_F is 0.10 at 36.5 against 0.021 at 109, a
    /// ratio of 4.9 where Equations 18 and 19 allow 1.7 and 3.5, so three periods are kept. Returns the half-sample
    /// candidate nearest the best period and its CE_F, or `None` below 21.
    fn refined_submultiple(&self, p_hat: usize, n: usize) -> Option<(usize, f64)> {
        submultiple(p_hat, n)?;
        let target = (21.0 + p_hat as f64 / 2.0) / n as f64;
        let steps = (2.0 * SUB_SPAN / SUB_STEP) as usize;
        let mut ce = 0.0;
        let mut best_p = target;
        for (f, lags) in self.lags.iter().enumerate() {
            let mut best = (f64::INFINITY, target);
            for s in 0..=steps {
                let p = (target - SUB_SPAN + s as f64 * SUB_STEP).max(21.0);
                let e = e_at(lags, p, self.wi4_sum, true);
                if e < best.0 {
                    best = (e, p);
                }
            }
            if f == 0 {
                best_p = best.1;
            }
            ce += best.0;
        }
        let j = ((2.0 * best_p).round() as usize).clamp(42, 42 + NP - 1) - 42;
        Some((j, ce))
    }

    /// The initial pitch candidate of the current frame (sections 5.1.3 to 5.1.4, Equations 10 to 23); updates the
    /// look-back state.
    fn initial_pitch(&mut self) -> usize {
        let [e0, e1, e2] = &self.e;
        // Look-back.
        let mut pb = *near(self.p_m1).start();
        for j in near(self.p_m1) {
            if e0[j] < e0[pb] {
                pb = j;
            }
        }
        let ceb = e0[pb] + self.e_m1 + self.e_m2;
        // Look-ahead: the best E2 for each P1, then the best E1 + E2 for each P0.
        let mut best2 = [0.0f64; NP];
        for (p1, b) in best2.iter_mut().enumerate() {
            *b = near(p1).map(|j| e2[j]).fold(f64::INFINITY, f64::min);
        }
        let mut cef = [0.0f64; NP];
        for (p0, c) in cef.iter_mut().enumerate() {
            let m = near(p0).map(|j| e1[j] + best2[j]).fold(f64::INFINITY, f64::min);
            *c = e0[p0] + m;
        }
        let mut p_hat = 0;
        for i in 1..NP {
            if cef[i] < cef[p_hat] {
                p_hat = i;
            }
        }
        let mut pf = p_hat;
        for &n in &SUBMULTIPLES {
            if let Some((j, c)) = self.refined_submultiple(p_hat, n) {
                let c0 = cef[p_hat];
                if (c <= 0.85 && c <= 1.7 * c0) || (c <= 0.4 && c <= 3.5 * c0) || c <= 0.05 {
                    pf = j;
                    break;
                }
            }
        }
        let pi = if ceb <= 0.48 || ceb <= cef[pf] { pb } else { pf };
        self.e_m2 = self.e_m1;
        self.e_m1 = e0[pi];
        self.p_m1 = pi;
        pi
    }

    /// S_w(m) for m = 0..=128 of the current frame (Equation 29), and the energy terms of Equations 38 to 42.
    fn spectrum(&mut self) {
        for v in self.buf.iter_mut() {
            *v = Complex32::new(0.0, 0.0);
        }
        for n in -110i32..=110 {
            let x = self.s[(C0 as i32 + n) as usize] * W_R[n.unsigned_abs() as usize];
            self.buf[n.rem_euclid(N_DFT as i32) as usize] = Complex32::new(x as f32, 0.0);
        }
        let _ = self.fft.process(&mut self.buf);
        for (m, v) in self.sw.iter_mut().enumerate() {
            *v = (self.buf[m].re as f64, self.buf[m].im as f64);
        }
        let w0sq = self.wr[0] * self.wr[0];
        let p = |v: &(f64, f64)| v.0 * v.0 + v.1 * v.1;
        let xi_lf: f64 = self.sw[..64].iter().map(p).sum::<f64>() / w0sq;
        let xi_hf: f64 = self.sw[64..].iter().map(p).sum::<f64>() / w0sq;
        let xi0 = xi_lf + xi_hf;
        self.xi_max = if xi0 > self.xi_max {
            0.5 * self.xi_max + 0.5 * xi0
        } else {
            let v = 0.99 * self.xi_max + 0.01 * xi0;
            if v > 20000.0 {
                v
            } else {
                20000.0
            }
        };
        let base = (0.0025 * self.xi_max + xi0) / (0.01 * self.xi_max + xi0);
        self.m_xi = if xi_lf >= 5.0 * xi_hf {
            base
        } else {
            base * (xi_lf / (5.0 * xi_hf)).sqrt()
        };
    }

    /// The fit of harmonic `l` at `w0` over bins `a..b`: (A_l, sum of |W_R|^2).
    fn fit(&self, l: usize, w0: f64, a: usize, b: usize) -> ((f64, f64), f64) {
        let lf = N_WR as f64 / TAU * l as f64 * w0;
        let (mut nr, mut ni, mut d) = (0.0, 0.0, 0.0);
        for m in a..b {
            let w = self.w_r((64.0 * m as f64 - lf + 0.5).floor());
            nr += self.sw[m].0 * w;
            ni += self.sw[m].1 * w;
            d += w * w;
        }
        if d > 0.0 {
            ((nr / d, ni / d), d)
        } else {
            ((0.0, 0.0), 0.0)
        }
    }

    /// The bins `[ceil(a_l), ceil(b_l))` of harmonic `l` at `w0`, within 0..=128.
    fn band(l: usize, w0: f64) -> (usize, usize) {
        let f = N_DFT as f64 / TAU * w0;
        let a = ((l as f64 - 0.5) * f).ceil().clamp(0.0, 129.0) as usize;
        let b = ((l as f64 + 0.5) * f).ceil().clamp(0.0, 129.0) as usize;
        (a, b.max(a))
    }

    /// The squared error of the synthetic spectrum of harmonic `l` over bins `a..b` (Equation 25).
    fn band_error(&self, l: usize, w0: f64, a: usize, b: usize, amp: (f64, f64)) -> f64 {
        let lf = N_WR as f64 / TAU * l as f64 * w0;
        let mut e = 0.0;
        for m in a..b {
            let w = self.w_r((64.0 * m as f64 - lf + 0.5).floor());
            let (dr, di) = (self.sw[m].0 - amp.0 * w, self.sw[m].1 - amp.1 * w);
            e += dr * dr + di * di;
        }
        e
    }

    /// Pitch refinement (section 5.1.5): the best of the ten candidates around `p_i`, as omega_0.
    fn refine(&self, p_i: f64) -> f64 {
        let mut best = (f64::INFINITY, TAU / p_i);
        for k in [-9.0, -7.0, -5.0, -3.0, -1.0, 1.0, 3.0, 5.0, 7.0, 9.0] {
            let w0 = TAU / (p_i + k / 8.0);
            let f = N_DFT as f64 / TAU * w0;
            let upper = (((0.9254 * PI / w0 - 0.5).floor()) * f).floor() as usize;
            let mut err = 0.0;
            let mut l = 1;
            loop {
                let (a, b) = Self::band(l, w0);
                if a > upper || a > 128 {
                    break;
                }
                let lo = a.max(50);
                let hi = b.min(upper + 1);
                if lo < hi {
                    let (amp, _) = self.fit(l, w0, a, b);
                    err += self.band_error(l, w0, lo, hi, amp);
                }
                l += 1;
            }
            if err < best.0 {
                best = (err, w0);
            }
        }
        best.1
    }

    /// Voicing decisions and spectral amplitudes of the current frame for `w0` and `l` harmonics (sections 5.2, 5.3);
    /// updates the previous band decisions.
    pub fn harmonics(&mut self, w0: f64, l: usize, out: &mut Params, bands: &mut Bands) {
        let l = l.min(MAX_L);
        let k = bands_for(l);
        *out = Params::silent(w0, l);
        *bands = Bands {
            k,
            v: [false; MAX_K + 1],
        };
        let mut err = [0.0f64; MAX_K + 1];
        let mut pow = [0.0f64; MAX_K + 1];
        let mut s2 = [0.0f64; MAX_L + 1];
        let mut w2 = [0.0f64; MAX_L + 1];
        let mut cnt = [0usize; MAX_L + 1];
        for li in 1..=l {
            let (a, b) = Self::band(li, w0);
            let (amp, d) = self.fit(li, w0, a, b);
            let kk = crate::model::band_of(li).min(k);
            err[kk] += self.band_error(li, w0, a, b, amp);
            let p: f64 = (a..b).map(|m| self.sw[m].0 * self.sw[m].0 + self.sw[m].1 * self.sw[m].1).sum();
            pow[kk] += p;
            s2[li] = p;
            w2[li] = d;
            cnt[li] = b - a;
        }
        let sum_wr = self.wr[0];
        for kk in 1..=k {
            let d = if pow[kk] > 0.0 { err[kk] / pow[kk] } else { 1.0 };
            let theta = if self.e_pi > 0.5 && kk >= 2 {
                0.0
            } else {
                let c = if self.prev_bands[kk] { 0.5625 } else { 0.45 };
                c * (1.0 - 0.3096 * (kk - 1) as f64 * w0) * self.m_xi
            };
            bands.v[kk] = d < theta;
        }
        for li in 1..=l {
            let v = bands.v[crate::model::band_of(li).min(k)];
            out.voiced[li] = v;
            out.amp[li] = if v {
                if w2[li] > 0.0 {
                    (s2[li] / w2[li]).sqrt()
                } else {
                    0.0
                }
            } else {
                (s2[li] / cnt[li].max(1) as f64).sqrt() / sum_wr
            };
        }
        self.prev_bands = bands.v;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_ranges() {
        // P = 100 (i = 158): 80 .. 120.
        let r = near(158);
        assert_eq!((21.0 + *r.start() as f64 / 2.0, 21.0 + *r.end() as f64 / 2.0), (80.0, 120.0));
        assert_eq!(*near(NP - 1).end(), NP - 1);
        // 100 / 2 = 50, 100 / 3 = 33.33 -> 33.5, 100 / 5 = 20 -> none.
        assert_eq!(submultiple(158, 2), Some(2 * 50 - 42));
        assert_eq!(submultiple(158, 3), Some(67 - 42));
        assert_eq!(submultiple(158, 5), None);
    }

    #[test]
    fn window_spectrum_peak() {
        let wr = window_spectrum();
        assert!((wr[0] - 110.019872).abs() < 1e-3, "{}", wr[0]);
    }

    /// A vowel-like envelope: two formants at 500 and 1500 Hz over a falling slope.
    pub(crate) fn formant(hz: f64) -> f64 {
        let g = |c: f64, w: f64| (-((hz - c) / w).powi(2)).exp();
        3000.0 * g(500.0, 250.0) + 1500.0 * g(1500.0, 300.0) + 200.0 * (-hz / 1000.0).exp()
    }

    fn tone(f: f64, n0: usize, out: &mut [i16; FRAME]) {
        for (i, o) in out.iter_mut().enumerate() {
            let t = (n0 + i) as f64 / 8000.0;
            let mut v = 0.0;
            for h in 1..=20 {
                if h as f64 * f < 3800.0 {
                    v += formant(f * h as f64) * (TAU * f * h as f64 * t).cos();
                }
            }
            *o = v as i16;
        }
    }

    #[test]
    fn finds_the_pitch_of_a_harmonic_series() {
        for f in [100.0, 150.0, 180.0, 220.0, 260.0, 300.0] {
            let mut a = Analyser::new();
            let mut pcm = [0i16; FRAME];
            let mut w0 = 0.0;
            for k in 0..10 {
                tone(f, k * FRAME, &mut pcm);
                w0 = a.push(&pcm);
            }
            let got = w0 * 8000.0 / TAU;
            assert!((got - f).abs() / f < 0.01, "{f}: {got}");
        }
    }
}
