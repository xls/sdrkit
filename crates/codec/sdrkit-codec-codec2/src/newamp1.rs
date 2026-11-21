//! The rate-K model of mode 700C: harmonic amplitudes resampled to 20 mel-spaced frequencies, a two-stage vector quantiser with
//! an M-best search, a post filter, and minimum-phase synthesis from the amplitudes.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), section 3.8.

use crate::analysis::Model;
use crate::consts::{
    cosf, expf, log10f, logf, pow10f, sinf, wo_max, wo_min, K, LN10F, MAX_AMP, M_PI, PHASE_NFFT,
};
use sdrkit_algo_dsp::fft_exact::ExactFft;
use sdrkit_algo_dsp::Complex32;

/// The number of vectors in each VQ stage.
const VQ_ENTRIES: usize = 512;
/// The number of survivors of the M-best search.
const MBEST: usize = 5;

/// The 20 rate-K sample frequencies in kHz.
pub(crate) fn sample_freqs() -> [f32; K] {
    let mel = |f: f64| ((2595.0 * log10f((1.0 + f / 700.0) as f32) as f64 + 0.5) as f32).floor();
    let start = mel(200.0);
    let end = mel(3700.0);
    let step = (end - start) / (K - 1) as f32;
    let mut fk = [0.0f32; K];
    let mut m = start;
    for f in &mut fk {
        let p = expf((LN10F as f64 * m as f64 / 2595.0) as f32);
        *f = (0.7 * (p as f64 - 1.0)) as f32;
        m += step;
    }
    fk
}

/// Piecewise parabolic interpolation through the points `(xp, yp)` (`xp` rising, at least 3 points), at the rising query
/// points `x`. Each output uses the parabola through three neighbouring points. Outside the range of `xp` the end parabolas
/// extrapolate.
pub(crate) fn interp_para(xp: &[f32], yp: &[f32], x: &[f32], y: &mut [f32]) {
    let np = xp.len().min(yp.len());
    if np < 3 {
        y.fill(0.0);
        return;
    }
    let mut s = 0;
    for (xi, yi) in x.iter().zip(y.iter_mut()) {
        while s < np - 3 && xp[s + 1] < *xi {
            s += 1;
        }
        let (x1, y1) = (xp[s], yp[s]);
        let (x2, y2) = (xp[s + 1], yp[s + 1]);
        let (x3, y3) = (xp[s + 2], yp[s + 2]);
        let a = ((y3 - y2) / (x3 - x2) - (y2 - y1) / (x2 - x1)) / (x3 - x1);
        let b = ((y3 - y2) / (x3 - x2) * (x2 - x1) + (y2 - y1) / (x2 - x1) * (x3 - x2)) / (x3 - x1);
        let d = *xi - x2;
        *yi = a * d * d + b * d + y2;
    }
}

/// A list of the best candidates of the M-best search, sorted by error.
#[derive(Clone, Copy)]
struct MBest {
    err: [f32; MBEST],
    idx: [[usize; 2]; MBEST],
}

impl MBest {
    fn new() -> Self {
        MBEST_EMPTY
    }

    fn insert(&mut self, idx: [usize; 2], err: f32) {
        if err >= self.err[MBEST - 1] {
            return;
        }
        let pos = self.err.iter().position(|&e| err < e).unwrap_or(MBEST - 1);
        for i in (pos + 1..MBEST).rev() {
            self.err[i] = self.err[i - 1];
            self.idx[i] = self.idx[i - 1];
        }
        self.err[pos] = err;
        self.idx[pos] = idx;
    }
}

const MBEST_EMPTY: MBest = MBest {
    err: [1e32; MBEST],
    idx: [[0; 2]; MBEST],
};

/// The 700C encoder quantiser.
pub(crate) struct RateKEncoder {
    fk: [f32; K],
    vq1: Box<[f32]>,
    vq2: Box<[f32]>,
    energy: Box<[f32]>,
}

/// The four 700C indexes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Indexes700 {
    pub vq1: u32,
    pub vq2: u32,
    pub energy: u32,
    pub pitch: u32,
}

/// The 700C pitch index of a voiced frame.
pub(crate) fn encode_pitch(wo: f32) -> u32 {
    let lo = log10f(wo_min());
    let hi = log10f(wo_max());
    let norm = (log10f(wo) - lo) / (hi - lo);
    let i = ((((64.0 * norm) as f64 + 0.5) as f32).floor() as i64).clamp(0, 63) as u32;
    i.max(1)
}

/// The pitch of a nonzero 700C pitch index.
pub(crate) fn decode_pitch(index: u32) -> f32 {
    let lo = log10f(wo_min());
    let hi = log10f(wo_max());
    let s = (hi - lo) / 64.0;
    pow10f(lo + s * index as f32)
}

impl RateKEncoder {
    pub fn new(vq1: Box<[f32]>, vq2: Box<[f32]>, energy: Box<[f32]>) -> Self {
        RateKEncoder {
            fk: sample_freqs(),
            vq1,
            vq2,
            energy,
        }
    }

    /// Quantises the model of the last frame.
    pub fn encode(&self, model: &Model) -> Indexes700 {
        let l = model.l.min(MAX_AMP);
        let mut amdb = [0.0f32; MAX_AMP];
        let mut fm = [0.0f32; MAX_AMP];
        let mut peak = -100.0f32;
        for m in 1..=l {
            let v = (20.0 * log10f((model.a[m] as f64 + 1e-16) as f32) as f64) as f32;
            amdb[m - 1] = v;
            if v > peak {
                peak = v;
            }
            fm[m - 1] = harmonic_khz(m, model.wo);
        }
        let floor = (peak as f64 - 50.0) as f32;
        for v in &mut amdb[..l] {
            if *v < floor {
                *v = floor;
            }
        }
        let mut b = [0.0f32; K];
        interp_para(&fm[..l], &amdb[..l], &self.fk, &mut b);
        let mut sum = 0.0f32;
        for v in &b {
            sum += v;
        }
        let mean = sum / K as f32;
        let mut d = [0.0f32; K];
        for k in 0..K {
            d[k] = b[k] - mean;
        }

        let mut list1 = MBest::new();
        for (j, cb) in self.vq1.as_chunks::<K>().0.iter().enumerate() {
            list1.insert([j, 0], dist(cb, &d));
        }
        let mut list2 = MBest::new();
        for n in 0..MBEST {
            let n1 = list1.idx[n][0];
            let cb1 = &self.vq1[n1 * K..n1 * K + K];
            let mut target = [0.0f32; K];
            for k in 0..K {
                target[k] = d[k] - cb1[k];
            }
            for (j, cb) in self
                .vq2
                .as_chunks::<K>()
                .0
                .iter()
                .enumerate()
                .take(VQ_ENTRIES)
            {
                list2.insert([j, n1], dist(cb, &target));
            }
        }

        let mut energy = 0;
        let mut best = f32::MAX;
        for (i, &e) in self.energy.iter().enumerate() {
            let diff = e - mean;
            if diff * diff < best {
                best = diff * diff;
                energy = i;
            }
        }
        Indexes700 {
            vq1: list2.idx[0][1] as u32,
            vq2: list2.idx[0][0] as u32,
            energy: energy as u32,
            pitch: if model.voiced {
                encode_pitch(model.wo)
            } else {
                0
            },
        }
    }
}

fn dist(cb: &[f32], target: &[f32; K]) -> f32 {
    let mut e = 0.0f32;
    for k in 0..K {
        let d = cb[k] - target[k];
        e += d * d;
    }
    e
}

/// The frequency in kHz of harmonic `m` of `wo`.
fn harmonic_khz(m: usize, wo: f32) -> f32 {
    ((m as f32 * wo) as f64 * 4.0 / M_PI) as f32
}

/// The 700C decoder state.
pub(crate) struct RateKDecoder {
    fk: [f32; K],
    pre: [f32; K],
    vq1: Box<[f32]>,
    vq2: Box<[f32]>,
    energy: Box<[f32]>,
    prev: [f32; K],
    wo_left: f32,
    voiced_left: bool,
    fwd: ExactFft,
    inv: ExactFft,
    buf: Box<[Complex32]>,
}

/// The fundamental of an unvoiced 700C frame (100 Hz period).
fn wo_unvoiced() -> f32 {
    (2.0 * M_PI / 100.0) as f32
}

/// `exp(ln(10) x / d)` with the product and the division in `f64`.
fn exp10_over(x: f32, d: f64) -> f32 {
    expf((LN10F as f64 * x as f64 / d) as f32)
}

impl RateKDecoder {
    pub fn new(vq1: Box<[f32]>, vq2: Box<[f32]>, energy: Box<[f32]>) -> Self {
        let fk = sample_freqs();
        let mut pre = [0.0f32; K];
        for k in 0..K {
            pre[k] = (20.0 * log10f((fk[k] as f64 / 0.3) as f32) as f64) as f32;
        }
        RateKDecoder {
            fk,
            pre,
            vq1,
            vq2,
            energy,
            prev: [0.0; K],
            wo_left: 0.0,
            voiced_left: false,
            fwd: ExactFft::new(PHASE_NFFT, false).expect("128 is a valid FFT size"),
            inv: ExactFft::new(PHASE_NFFT, true).expect("128 is a valid FFT size"),
            buf: vec![Complex32::new(0.0, 0.0); PHASE_NFFT].into_boxed_slice(),
        }
    }

    /// Decodes the indexes into four frames of models and phase vectors.
    pub fn decode(
        &mut self,
        ix: Indexes700,
        models: &mut [Model; 4],
        h: &mut [[Complex32; MAX_AMP + 1]; 4],
    ) {
        let i1 = (ix.vq1 as usize).min(VQ_ENTRIES - 1) * K;
        let i2 = (ix.vq2 as usize).min(VQ_ENTRIES - 1) * K;
        let mut dq = [0.0f32; K];
        for k in 0..K {
            dq[k] = self.vq1[i1 + k] + self.vq2[i2 + k];
        }
        let (mut e_before, mut e_after) = (0.0f32, 0.0f32);
        for k in 0..K {
            dq[k] += self.pre[k];
            e_before += exp10_over(dq[k], 10.0);
            dq[k] *= 1.5f32;
            e_after += exp10_over(dq[k], 10.0);
        }
        let gain_db = 10.0f32 * log10f(e_after / e_before);
        for k in 0..K {
            dq[k] -= gain_db;
            dq[k] -= self.pre[k];
        }
        let mean = self.energy[(ix.energy as usize).min(self.energy.len() - 1)];
        let mut right = [0.0f32; K];
        for k in 0..K {
            right[k] = dq[k] + mean;
        }

        let (wo_right, voiced_right) = if ix.pitch > 0 {
            (decode_pitch(ix.pitch), true)
        } else {
            (wo_unvoiced(), false)
        };

        let mut c = 1.0f32;
        for (i, (model, hv)) in models.iter_mut().zip(h.iter_mut()).enumerate() {
            let mut vec = [0.0f32; K];
            for k in 0..K {
                vec[k] = ((self.prev[k] * c) as f64 + right[k] as f64 * (1.0 - c as f64)) as f32;
            }
            let (wo, voiced) = match (self.voiced_left, voiced_right) {
                (false, false) => (wo_unvoiced(), false),
                (true, false) => {
                    if i < 2 {
                        (self.wo_left, true)
                    } else {
                        (wo_unvoiced(), false)
                    }
                }
                (false, true) => {
                    if i < 2 {
                        (wo_unvoiced(), false)
                    } else {
                        (wo_right, true)
                    }
                }
                (true, true) => (
                    ((self.wo_left * c) as f64 + wo_right as f64 * (1.0 - c as f64)) as f32,
                    true,
                ),
            };
            model.wo = wo;
            model.voiced = voiced;
            model.l = (((M_PI / wo as f64) as f32).floor() as usize).min(MAX_AMP);
            self.rate_l(&vec, model);
            self.min_phase(model, hv);
            c = (c as f64 - 1.0 / 4.0) as f32;
        }
        self.prev = right;
        self.wo_left = wo_right;
        self.voiced_left = voiced_right;
    }

    /// Resamples the rate-K vector `vec` (dB) to the harmonic amplitudes of `model`.
    fn rate_l(&self, vec: &[f32; K], model: &mut Model) {
        let mut xp = [0.0f32; K + 2];
        let mut yp = [0.0f32; K + 2];
        xp[1..=K].copy_from_slice(&self.fk);
        yp[1..=K].copy_from_slice(vec);
        xp[K + 1] = 4.0;
        let l = model.l;
        let mut fm = [0.0f32; MAX_AMP];
        for m in 1..=l {
            fm[m - 1] = harmonic_khz(m, model.wo);
        }
        let mut amdb = [0.0f32; MAX_AMP];
        interp_para(&xp, &yp, &fm[..l], &mut amdb[..l]);
        for m in 1..=l {
            model.a[m] = exp10_over(amdb[m - 1], 20.0);
        }
        for a in &mut model.a[l + 1..] {
            *a = 0.0;
        }
    }

    /// The minimum-phase spectrum of the amplitudes of `model`, sampled at the harmonics.
    fn min_phase(&mut self, model: &Model, h: &mut [Complex32; MAX_AMP + 1]) {
        let l = model.l;
        let n = PHASE_NFFT;
        let half = n / 2;
        let mut amdb = [0.0f32; MAX_AMP];
        let mut fm = [0.0f32; MAX_AMP];
        for m in 1..=l {
            amdb[m - 1] = (20.0 * log10f(model.a[m]) as f64) as f32;
            fm[m - 1] = harmonic_khz(m, model.wo);
        }
        let mut fq = [0.0f32; PHASE_NFFT / 2 + 1];
        for (i, f) in fq.iter_mut().enumerate() {
            *f = (8.0 * i as f64 / n as f64) as f32;
        }
        let mut g = [0.0f32; PHASE_NFFT / 2 + 1];
        interp_para(&fm[..l], &amdb[..l], &fq, &mut g);

        // The complex cepstrum of the log magnitude.
        self.buf.fill(Complex32::new(0.0, 0.0));
        self.buf[0].re = g[0];
        for i in 1..=half {
            self.buf[i].re = g[i];
            self.buf[n - i].re = g[i];
        }
        let _ = self.inv.process(&mut self.buf);
        let mut c = [Complex32::new(0.0, 0.0); PHASE_NFFT];
        for (ci, b) in c.iter_mut().zip(self.buf.iter()) {
            *ci = Complex32::new(b.re / n as f32, b.im / n as f32);
        }
        // Fold to a causal cepstrum and take the spectrum: its imaginary part is the minimum phase.
        self.buf.fill(Complex32::new(0.0, 0.0));
        self.buf[0] = c[0];
        for i in 1..half {
            self.buf[i] = c[i] + c[n - i];
        }
        self.buf[half] = c[half];
        let _ = self.fwd.process(&mut self.buf);
        let scale = (20.0 / logf(10.0) as f64) as f32;
        let mut phase = [0.0f32; PHASE_NFFT / 2 + 1];
        for (p, b) in phase.iter_mut().zip(self.buf.iter()) {
            *p = b.im / scale;
        }
        for m in 1..=l {
            let mw = m as f32 * model.wo * n as f32;
            let b = ((0.5 + mw as f64 / (2.0 * M_PI)) as f32).floor() as usize;
            let p = phase[b.min(half)];
            h[m] = Complex32::new(cosf(p), sinf(p));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_frequencies_span_200_to_3700_hz() {
        let fk = sample_freqs();
        assert!((fk[0] - 0.2).abs() < 0.002, "{}", fk[0]);
        assert!((fk[K - 1] - 3.7).abs() < 0.01, "{}", fk[K - 1]);
        assert!(fk.windows(2).all(|w| w[1] > w[0]));
    }

    #[test]
    fn parabolic_interpolation_is_exact_on_a_parabola() {
        let xp: Vec<f32> = (0..8).map(|i| i as f32 * 0.5 + 0.1).collect();
        let yp: Vec<f32> = xp.iter().map(|x| 2.0 * x * x - 3.0 * x + 1.0).collect();
        let x = [0.0f32, 0.3, 1.0, 2.2, 3.9, 5.0];
        let mut y = [0.0f32; 6];
        interp_para(&xp, &yp, &x, &mut y);
        for (xi, yi) in x.iter().zip(y) {
            let want = 2.0 * xi * xi - 3.0 * xi + 1.0;
            assert!((yi - want).abs() < 1e-3, "{xi}: {yi} {want}");
        }
    }

    #[test]
    fn pitch_index_round_trip() {
        for i in 1..64 {
            assert_eq!(encode_pitch(decode_pitch(i)), i);
        }
    }

    #[test]
    fn mbest_keeps_the_lowest_errors_in_order() {
        let mut m = MBest::new();
        for (i, e) in [5.0f32, 1.0, 4.0, 1.0, 9.0, 0.5, 3.0].iter().enumerate() {
            m.insert([i, 0], *e);
        }
        assert_eq!(m.err, [0.5, 1.0, 1.0, 3.0, 4.0]);
        assert_eq!(m.idx.map(|p| p[0]), [5, 1, 3, 6, 2]);
    }
}
