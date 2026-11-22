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
/// The divisors tried for sub-multiples of the forward pitch, largest first (flow chart 4; 6 is explained at
/// [`submultiple`]).
const SUBMULTIPLES: [usize; 5] = [6, 5, 4, 3, 2];

/// The shortest period the coders carry: the lowest candidate less the reach of the refinement, 21 - 1.125.
const MIN_PERIOD: f64 = 19.875;

/// The candidate nearest the sub-multiple `P / n` of candidate `i`, rounded as P = floor(2 P / n + 1/2) / 2 (flow chart
/// 4, read with both cut-off denominators as 2), or `None` when `P / n` is below [`MIN_PERIOD`].
///
/// The text drops a sub-multiple below 21 and tries the divisors up to 5, which leaves a voice above 381 Hz at two or
/// more of its periods, though the refinement reaches 19.875 from the candidate 21. Here a sub-multiple from 19.875 to
/// 21 gives the candidate 21, and the divisor 6 serves the periods from 119.25.
fn submultiple(i: usize, n: usize) -> Option<usize> {
    let two_p = 42 + i;
    if (two_p as f64) < 2.0 * MIN_PERIOD * n as f64 {
        return None;
    }
    let j = (2 * two_p + n) / (2 * n);
    Some(j.clamp(42, 42 + NP - 1) - 42)
}

/// The autocorrelation r(t) of one frame for t = 0..=300 (Equation 7) and its windowed energy; energy 0 when silent.
#[derive(Clone, Copy)]
struct Lags {
    r: [f64; 301],
    energy: f64,
}

impl Lags {
    const SILENT: Lags = Lags { r: [0.0; 301], energy: 0.0 };

    /// r at lag `k`, with r(-t) = r(t) and 0 above lag 300.
    fn r(&self, k: isize) -> f64 {
        self.r.get(k.unsigned_abs()).copied().unwrap_or(0.0)
    }

    /// r at any lag by Equation 8: between two lags in a straight line.
    fn linear(&self, t: f64) -> f64 {
        let k = t.floor() as isize;
        let d = t - t.floor();
        (1.0 - d) * self.r(k) + d * self.r(k + 1)
    }

    /// r at any lag through the four nearest lags (a cubic).
    fn cubic(&self, t: f64) -> f64 {
        let k = t.floor() as isize;
        let d = t - t.floor();
        let (a, b, c, e) = (self.r(k - 1), self.r(k), self.r(k + 1), self.r(k + 2));
        b + d * (c - a) / 2.0 + d * d * (a - 2.0 * b + c) / 2.0 + d * (d * d - 1.0) * (e - 3.0 * c + 3.0 * b - a) / 6.0
    }

    /// E(P) of Equation 5 for any period `p`, with the sum of r(n P) taken over every lag up to 300 and r between lags
    /// by `at`.
    ///
    /// The text stops that sum at lag 150. E is the squared error between the windowed signal and its best fit of
    /// period P, over the energy, only when every lag with r not 0 counts, and r reaches lag 300. The lost terms are
    /// above 0 for a periodic signal and largest just above P = 150 / n, which makes a multiple of the period fit
    /// better than the period.
    fn error(&self, p: f64, wi4_sum: f64, at: impl Fn(&Self, f64) -> f64) -> f64 {
        if self.energy <= 0.0 {
            return E_SILENT;
        }
        let mut sum = self.r[0];
        let mut t = p;
        while t <= 300.0 {
            sum += 2.0 * at(self, t);
            t += p;
        }
        (self.energy - p * sum) / (self.energy * (1.0 - p * wi4_sum))
    }
}

/// The first candidate whose E(P) takes the window overlap into account, P = 80.5; see [`fit_error`].
const FIRST_FIT: usize = 119;
/// The longest fold: twice the longest half-sample period, 121.5.
const MAX_FOLD: usize = 243;

/// The fold length of candidate `i` in samples: P for a whole period, 2 P for a half-sample one.
fn fold_len(i: usize) -> usize {
    let two_p = 42 + i;
    if two_p.is_multiple_of(2) {
        two_p / 2
    } else {
        two_p
    }
}

/// Adds the samples of `y` that lie a multiple of `q` apart: `f[k]` = sum over m of y(k + m q) for k < q, stored twice
/// in a row so that `f[k + d]` reads the sum at phase (k + d) mod q for d <= q.
fn fold(y: &[f64; 301], q: usize, f: &mut [f64; 2 * MAX_FOLD]) {
    let (first, rest) = y.split_at(q);
    f[..q].copy_from_slice(first);
    for chunk in rest.chunks(q) {
        for (a, v) in f[..chunk.len()].iter_mut().zip(chunk) {
            *a += v;
        }
    }
    f.copy_within(..q, q);
}

/// The sums of a folded signal over the period P of candidate `i`, one for each phase of the fold: for a whole period
/// the fold itself; for a half-sample period the fold of length 2 P plus its value half a fold away, the mean of the two
/// nearest samples as in Equation 8.
fn comb(i: usize, f: &[f64; 2 * MAX_FOLD], out: &mut [f64]) {
    let q = out.len();
    if (42 + i).is_multiple_of(2) {
        out.copy_from_slice(&f[..q]);
        return;
    }
    let h = (41 + i) / 2;
    for (((o, a), b), c) in out.iter_mut().zip(&f[..q]).zip(&f[h..h + q]).zip(&f[h + 1..h + 1 + q]) {
        *o = a + 0.5 * (b + c);
    }
}

/// The sum of the products of two equally long slices.
fn dot(a: &[f64], b: &[f64]) -> f64 {
    let ((ca, ra), (cb, rb)) = (a.as_chunks::<8>(), b.as_chunks::<8>());
    // Partial sums, so that the additions do not wait on each other.
    let mut acc = [0.0f64; 8];
    for (x, y) in ca.iter().zip(cb) {
        for i in 0..8 {
            acc[i] += x[i] * y[i];
        }
    }
    let tail: f64 = ra.iter().zip(rb).map(|(x, y)| x * y).sum();
    acc.iter().sum::<f64>() + tail
}

/// For each candidate from [`FIRST_FIT`] the reciprocal of the window overlap, 1 / sum over n of w_I^2(k + n P), at each
/// phase k of its fold.
struct Overlap {
    start: [usize; NP + 1],
    inv: Vec<f64>,
}

fn overlap() -> &'static Overlap {
    static T: OnceLock<Overlap> = OnceLock::new();
    T.get_or_init(|| {
        let wi2: [f64; 301] = std::array::from_fn(|i| W_I[(i as i32 - 150).unsigned_abs() as usize].powi(2));
        let mut start = [0; NP + 1];
        let mut inv = Vec::new();
        let mut f = [0.0; 2 * MAX_FOLD];
        for i in FIRST_FIT..NP {
            let q = fold_len(i);
            fold(&wi2, q, &mut f);
            start[i] = inv.len();
            let mut sums = [0.0; MAX_FOLD];
            comb(i, &f, &mut sums[..q]);
            inv.extend(sums[..q].iter().map(|v| 1.0 / v));
        }
        start[NP] = inv.len();
        Overlap { start, inv }
    })
}

/// E(P) of candidate `i` for the frame `y` = s_LPF w_I^2 of windowed energy `energy`: the squared error between the
/// windowed signal and its best fit of period P, over the energy, with the bias term of Equation 5.
///
/// Equation 5 takes the window overlap, P sum over n of w_I^2(k + n P), as 1 at every phase k. It is 1 within 0.0003
/// up to P = 80, but 0.99 to 1.01 at P = 100 and 0.93 to 1.07 at P = 120, where the printed E goes below 0 for a signal
/// with its energy near the window centre. Here each phase is divided by its own overlap; E of a whole period then
/// cannot go below 0. With an overlap of 1 the value is that of [`Lags::error`].
fn fit_error(i: usize, y: &[f64; 301], energy: f64, wi4_sum: f64, ov: &Overlap, work: &mut FitWork) -> f64 {
    let q = fold_len(i);
    let (f, g) = (&mut work.fold, &mut work.sums[..q]);
    fold(y, q, f);
    comb(i, f, g);
    for (v, w) in g.iter_mut().zip(&ov.inv[ov.start[i]..ov.start[i + 1]]) {
        *v *= w;
    }
    let fitted = dot(&f[..q], g);
    let p = 21.0 + i as f64 / 2.0;
    (energy - fitted) / (energy * (1.0 - p * wi4_sum))
}

/// The working arrays of [`fit_error`].
struct FitWork {
    fold: [f64; 2 * MAX_FOLD],
    sums: [f64; MAX_FOLD],
}

/// The first candidate of `range` with the least `value`, and that value.
fn least(range: std::ops::RangeInclusive<usize>, value: impl Fn(usize) -> f64) -> (usize, f64) {
    let first = *range.start();
    range.fold((first, f64::INFINITY), |best, j| {
        let v = value(j);
        if v < best.1 {
            (j, v)
        } else {
            best
        }
    })
}

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
    overlap: &'static Overlap,
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
            lags: [Lags::SILENT; 3],
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
            overlap: overlap(),
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
        let mut l2 = Lags::SILENT;
        self.error_function(C2, &mut e2, &mut l2);
        self.e[2] = e2;
        self.lags[2] = l2;
        let pi = self.initial_pitch();
        self.spectrum();
        self.refine(21.0 + pi as f64 / 2.0)
    }

    /// E(P) of the frame centred at `c` for every candidate and its autocorrelation (Equations 5 to 9 with
    /// [`Lags::error`] and [`fit_error`]); 1 for a frame without energy.
    fn error_function(&self, c: usize, out: &mut [f64; NP], lags: &mut Lags) {
        let mut y = [0.0f64; 301];
        let mut energy = 0.0;
        for (j, yv) in y.iter_mut().enumerate() {
            let n = c + j - 150;
            let mut acc = self.s[n] * H_LPF[0];
            for t in 1..=10 {
                acc += (self.s[n - t] + self.s[n + t]) * H_LPF[t];
            }
            energy += acc * acc * self.wi2[j];
            *yv = acc * self.wi2[j];
        }
        if !(energy > 1e-9 && energy.is_finite()) {
            out.fill(E_SILENT);
            *lags = Lags::SILENT;
            return;
        }
        lags.energy = energy;
        for (t, rv) in lags.r.iter_mut().enumerate() {
            *rv = dot(&y[..301 - t], &y[t..]);
        }
        let mut work = FitWork { fold: [0.0; 2 * MAX_FOLD], sums: [0.0; MAX_FOLD] };
        for (i, o) in out.iter_mut().enumerate() {
            *o = if i < FIRST_FIT {
                lags.error(21.0 + i as f64 / 2.0, self.wi4_sum, Lags::linear)
            } else {
                fit_error(i, &y, energy, self.wi4_sum, self.overlap, &mut work)
            };
        }
    }

    /// CE_F of the sub-multiple P^_0 / n for the test of Equations 18 to 20, and its E in the current frame; `path` holds
    /// the candidates P^_0, P^_1, P^_2 of the forward estimate.
    ///
    /// The text takes CE_F at P^_0 / n rounded to the half-sample grid. The rounding moves every multiple of the period
    /// by up to n / 4 sample, an error the multiple P^_0 itself does not have; for an exactly periodic signal
    /// CE_F(P^_0) is 0 and no ratio of Equations 18 and 19 can hold. Here each frame's E is taken at the period not
    /// rounded: P^_0 / n in the current frame, and in a look-ahead frame its path period over the whole number that
    /// brings it nearest P^_0 / n (the path may pass from one multiple of the pitch to another). r between lags is a
    /// cubic, since the straight line of Equation 8 loses up to 7.6 % of a 1 kHz component at these lags.
    fn submultiple_error(&self, path: [usize; 3], n: usize) -> (f64, f64) {
        let period = |f: usize| 21.0 + path[f] as f64 / 2.0;
        let p = period(0) / n as f64;
        let e = |f: usize, p: f64| self.lags[f].error(p, self.wi4_sum, Lags::cubic);
        let ahead = |f: usize| e(f, period(f) / (period(f) / p).round().max(1.0));
        let e0 = e(0, p);
        (e0 + ahead(1) + ahead(2), e0)
    }

    /// The initial pitch candidate of the current frame (sections 5.1.3 to 5.1.4, Equations 10 to 23); updates the
    /// look-back state and the E(P_I) of the voicing threshold.
    fn initial_pitch(&mut self) -> usize {
        let [e0, e1, e2] = &self.e;
        // Look-back.
        let pb = least(near(self.p_m1), |j| e0[j]).0;
        let ceb = e0[pb] + self.e_m1 + self.e_m2;
        // Look-ahead: the least E2 for each P1, then the least E1 + E2 for each P0.
        let mut best2 = [0.0f64; NP];
        for (p1, b) in best2.iter_mut().enumerate() {
            *b = e2[near(p1)].iter().copied().fold(f64::INFINITY, f64::min);
        }
        let mut cef = [0.0f64; NP];
        for (p0, c) in cef.iter_mut().enumerate() {
            let r = near(p0);
            *c = e0[p0] + e1[r.clone()].iter().zip(&best2[r]).map(|(a, b)| a + b).fold(f64::INFINITY, f64::min);
        }
        let p_hat = least(0..=NP - 1, |j| cef[j]).0;
        let p1 = least(near(p_hat), |j| e1[j] + best2[j]).0;
        let path = [p_hat, p1, least(near(p1), |j| e2[j]).0];
        let (mut pf, mut cef_pf, mut e_pf) = (p_hat, cef[p_hat], e0[p_hat]);
        for &n in &SUBMULTIPLES {
            if let Some(j) = submultiple(p_hat, n) {
                let (c, e) = self.submultiple_error(path, n);
                let c0 = cef[p_hat];
                if (c <= 0.85 && c <= 1.7 * c0) || (c <= 0.4 && c <= 3.5 * c0) || c <= 0.05 {
                    (pf, cef_pf, e_pf) = (j, c, e);
                    break;
                }
            }
        }
        let (pi, e_pi) = if ceb <= 0.48 || ceb <= cef_pf { (pb, e0[pb]) } else { (pf, e_pf) };
        self.e_m2 = self.e_m1;
        self.e_m1 = e_pi;
        self.e_pi = e_pi;
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

    /// Voicing decisions and spectral amplitudes of the current frame for `l` harmonics of `w0` (sections 5.2, 5.3);
    /// updates the previous band decisions. The voicing measure fits the harmonics of `w0_fit`, the analysed
    /// fundamental; `w0` differs from it when the coder sends a fundamental from a table.
    pub fn harmonics(&mut self, w0_fit: f64, w0: f64, l: usize, out: &mut Params, bands: &mut Bands) {
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
            let power = |a: usize, b: usize| -> f64 { self.sw[a..b].iter().map(|v| v.0 * v.0 + v.1 * v.1).sum() };
            let kk = crate::model::band_of(li).min(k);
            let (a, b) = Self::band(li, w0_fit);
            let (amp, _) = self.fit(li, w0_fit, a, b);
            err[kk] += self.band_error(li, w0_fit, a, b, amp);
            pow[kk] += power(a, b);
            let (a, b) = Self::band(li, w0);
            s2[li] = power(a, b);
            w2[li] = self.fit(li, w0, a, b).1;
            cnt[li] = b - a;
        }
        let sum_wr = self.wr[0];
        for kk in 1..=k {
            let d = if pow[kk] > 0.0 { err[kk] / pow[kk] } else { 1.0 };
            let theta = if self.e_pi > 0.5 && kk >= 2 {
                0.0
            } else {
                let c = if self.prev_bands[kk] { 0.5625 } else { 0.45 };
                c * (1.0 - 0.3096 * (kk - 1) as f64 * w0_fit) * self.m_xi
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
        assert_eq!(submultiple(158, 5), Some(0));
        assert_eq!(submultiple(158, 6), None);
        assert_eq!(submultiple(2 * 120 - 42, 6), Some(0));
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
        for f in [100.0, 150.0, 180.0, 200.0, 203.0, 220.0, 260.0, 300.0] {
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

    /// E(P) of every candidate for a frame holding `x(n)` around the centre.
    fn errors(x: impl Fn(i32) -> f64) -> [f64; NP] {
        let mut a = Analyser::new();
        for (i, v) in a.s.iter_mut().enumerate() {
            *v = x(i as i32 - C0 as i32);
        }
        let mut e = [0.0; NP];
        let mut l = Lags::SILENT;
        a.error_function(C0, &mut e, &mut l);
        // Where the two forms of E(P) meet they agree.
        let d = (l.error(21.0 + FIRST_FIT as f64 / 2.0, a.wi4_sum, Lags::linear) - e[FIRST_FIT]).abs();
        assert!(d < 2e-3, "the forms differ by {d} at P = 80.5");
        e
    }

    fn at(p: f64) -> usize {
        (2.0 * p) as usize - 42
    }

    #[test]
    fn a_periodic_signal_has_no_error_at_its_period_and_multiples() {
        let wave = |p: f64| move |n: i32| (1..=5).map(|h| (TAU * h as f64 * n as f64 / p + h as f64).cos() / h as f64).sum::<f64>();
        let e = errors(wave(40.0));
        for p in [40.0, 80.0, 120.0] {
            assert!(e[at(p)].abs() < 1e-3, "E({p}) = {}", e[at(p)]);
        }
        assert!(e[at(60.0)] > 0.5 && e[at(21.0)] > 0.5);
        // A half-sample period loses what the mean of two samples loses of each harmonic; its whole multiple loses nothing.
        let e = errors(wave(40.5));
        assert!(e[at(40.5)].abs() < 0.01, "E(40.5) = {}", e[at(40.5)]);
        assert!(e[at(81.0)].abs() < 1e-3, "E(81) = {}", e[at(81.0)]);
    }

    #[test]
    fn the_error_stays_in_its_range() {
        let a = Analyser::new();
        let check = |e: &[f64; NP], what: &str| {
            for (i, &v) in e.iter().enumerate() {
                let p = 21.0 + i as f64 / 2.0;
                let top = 1.0 / (1.0 - p * a.wi4_sum);
                assert!((-1e-3..=top + 1e-9).contains(&v), "{what}: E({p}) = {v}, limit {top}");
            }
        };
        // Pulse trains, where the form of Equation 5 goes most below 0: one pulse on the centre and others.
        for period in [57, 61, 100, 113, 120, 122] {
            for shift in [0, 7, period / 2] {
                let e = errors(|n| if (n - shift).rem_euclid(period) == 0 { 1000.0 } else { 0.0 });
                check(&e, "pulses");
                assert!(e[at(period as f64)].abs() < 1e-3);
            }
        }
        // White noise: the mean over the candidates and frames is 1.
        let mut x = 1u64;
        let (mut sum, mut n) = (0.0, 0);
        for _ in 0..40 {
            let noise: Vec<f64> = (0..BUF)
                .map(|_| {
                    x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                    (x >> 40) as f64 - (1u64 << 23) as f64
                })
                .collect();
            let e = errors(|i| noise[(i + C0 as i32) as usize]);
            check(&e, "noise");
            sum += e.iter().sum::<f64>();
            n += NP;
        }
        let mean = sum / n as f64;
        assert!((mean - 1.0).abs() < 0.05, "mean E of noise {mean}");
    }
}
