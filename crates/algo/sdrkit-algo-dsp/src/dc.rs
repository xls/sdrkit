//! DC removal and blind IQ-imbalance correction.
//!
//! - [`DcEstimator`] and [`IqImbalance`]: the front end's estimators (a running mean settling to a
//!   2^20-sample average; second-difference statistics in fixed batches). Both work sample by sample, so the result does not depend
//!   on how the stream is cut.
//! - [`DcBlocker`] and [`IqCorrector`]: simpler general-purpose versions (a one-pole notch; Gram-Schmidt with an exponential average).

use crate::Complex32;
use num_complex::Complex64;
use std::f64::consts::TAU;

/// A DC blocker `H(z) = g (1 - z^-1) / (1 - r z^-1)`, `r = e^{-2 pi fc / rate}`, `g = (1 + r) / 2` (unity gain at Nyquist): a zero
/// exactly at DC and a -3 dB corner near `fc`. State is `f64`.
#[derive(Clone, Debug)]
pub struct DcBlocker {
    r: f64,
    g: f64,
    x1: Complex64,
    y1: Complex64,
}

impl DcBlocker {
    /// A blocker with its corner at `corner_hz`.
    pub fn new(rate: f64, corner_hz: f64) -> Self {
        assert!(rate > 0.0 && corner_hz > 0.0 && corner_hz < rate / 4.0);
        let r = (-TAU * corner_hz / rate).exp();
        DcBlocker {
            r,
            g: (1.0 + r) / 2.0,
            x1: Complex64::default(),
            y1: Complex64::default(),
        }
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.x1 = Complex64::default();
        self.y1 = Complex64::default();
    }

    /// The DC the filter has removed (an estimate of the input's DC level).
    pub fn dc(&self) -> Complex32 {
        let d = self.x1 - self.y1 / self.g;
        Complex32::new(d.re as f32, d.im as f32)
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for v in buf {
            let x = Complex64::new(v.re as f64, v.im as f64);
            let y = x - self.x1 + self.y1 * self.r;
            self.x1 = x;
            self.y1 = y;
            let o = y * self.g;
            *v = Complex32::new(o.re as f32, o.im as f32);
        }
    }
}

/// [`DcBlocker`] for real samples.
#[derive(Clone, Debug)]
pub struct RealDcBlocker {
    r: f64,
    g: f64,
    x1: f64,
    y1: f64,
}

impl RealDcBlocker {
    /// A blocker with its corner at `corner_hz`.
    pub fn new(rate: f64, corner_hz: f64) -> Self {
        assert!(rate > 0.0 && corner_hz > 0.0 && corner_hz < rate / 4.0);
        let r = (-TAU * corner_hz / rate).exp();
        RealDcBlocker {
            r,
            g: (1.0 + r) / 2.0,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Clears the state.
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }

    /// Filters `buf` in place.
    pub fn process(&mut self, buf: &mut [f32]) {
        for v in buf {
            let x = *v as f64;
            let y = x - self.x1 + self.r * self.y1;
            self.x1 = x;
            self.y1 = y;
            *v = (y * self.g) as f32;
        }
    }
}

/// Blind IQ-imbalance correction by Gram-Schmidt orthogonalisation.
///
/// It keeps running averages of `I^2`, `Q^2` and `I Q` (time constant `tau_s`), then outputs `I' = I` and
/// `Q' = (Q - (E[IQ]/E[I^2]) I) * sqrt(E[I^2] / (E[Q^2] - E[IQ]^2/E[I^2]))`, so `I'` and `Q'` are uncorrelated and of equal power. It
/// assumes the true signal is circular (true of noise, of most modulations and of any tone away from DC) and that DC has been removed
/// first. The coefficients are updated every 32 samples.
#[derive(Clone, Debug)]
pub struct IqCorrector {
    a: f64,
    ii: f64,
    qq: f64,
    iq: f64,
    cross: f32,
    scale: f32,
}

const IQ_CHUNK: usize = 32;

impl IqCorrector {
    /// A corrector averaging over `tau_s` seconds at `rate`.
    pub fn new(rate: f64, tau_s: f64) -> Self {
        assert!(rate > 0.0 && tau_s > 0.0);
        IqCorrector {
            a: 1.0 - (-1.0 / (tau_s * rate)).exp(),
            ii: 0.0,
            qq: 0.0,
            iq: 0.0,
            cross: 0.0,
            scale: 1.0,
        }
    }

    /// Forgets the estimate.
    pub fn reset(&mut self) {
        self.ii = 0.0;
        self.qq = 0.0;
        self.iq = 0.0;
        self.cross = 0.0;
        self.scale = 1.0;
    }

    /// The measured amplitude imbalance, Q relative to I, in dB.
    pub fn amplitude_imbalance_db(&self) -> f64 {
        if self.ii <= 0.0 {
            return 0.0;
        }
        10.0 * (self.qq / self.ii).log10()
    }

    /// The measured phase error (departure of Q from quadrature), in degrees.
    pub fn phase_error_deg(&self) -> f64 {
        if self.ii <= 0.0 || self.qq <= 0.0 {
            return 0.0;
        }
        (self.iq / (self.ii * self.qq).sqrt())
            .clamp(-1.0, 1.0)
            .asin()
            .to_degrees()
    }

    fn update_coeffs(&mut self) {
        if self.ii > 1e-30 {
            let c = self.iq / self.ii;
            let resid = self.qq - self.iq * c;
            if resid > 1e-30 {
                self.cross = c as f32;
                self.scale = (self.ii / resid).sqrt() as f32;
            }
        }
    }

    /// Corrects `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        for chunk in buf.chunks_mut(IQ_CHUNK) {
            let (c, s) = (self.cross, self.scale);
            let a = self.a;
            for v in chunk.iter_mut() {
                let (i, q) = (v.re as f64, v.im as f64);
                self.ii += a * (i * i - self.ii);
                self.qq += a * (q * q - self.qq);
                self.iq += a * (i * q - self.iq);
                v.im = (v.im - c * v.re) * s;
            }
            self.update_coeffs();
        }
    }
}

/// The running mean of the front end, sample by sample: the DC of a zero-IF radio.
///
/// The spec starts from the plain mean of the first block; so that the result does not depend on how the stream is cut, the first
/// [`DcEstimator::START`] samples use a plain growing mean instead. After that the weight is `1 / max(n, 2^17)`, `n` counting the
/// samples, until `n` reaches `2^20`, and `2^-20` from then on (an exponential average with a time constant of 2^20 samples: 0.5 s
/// at 2 Msps). The notch it makes is a fraction of a hertz wide once it has settled. The mean is kept in `f64`.
#[derive(Clone, Debug, Default)]
pub struct DcEstimator {
    n: u64,
    re: f64,
    im: f64,
}

impl DcEstimator {
    /// Samples averaged plainly at the start.
    pub const START: u64 = 4096;
    const MIN: f64 = (1u64 << 17) as f64;
    const MAX: f64 = (1u64 << 20) as f64;

    /// A fresh estimate.
    pub fn new() -> Self {
        Self::default()
    }

    /// Forgets everything (the next sample starts it again).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Takes one sample and returns the mean including it.
    #[inline]
    pub fn update(&mut self, x: Complex32) -> Complex32 {
        self.n += 1;
        let w = if self.n <= Self::START {
            1.0 / self.n as f64
        } else if self.n >= 1 << 20 {
            1.0 / Self::MAX
        } else {
            1.0 / (self.n as f64).max(Self::MIN)
        };
        self.re += (x.re as f64 - self.re) * w;
        self.im += (x.im as f64 - self.im) * w;
        Complex32::new(self.re as f32, self.im as f32)
    }

    /// The mean so far.
    pub fn mean(&self) -> Complex32 {
        Complex32::new(self.re as f32, self.im as f32)
    }

    /// Samples taken.
    pub fn count(&self) -> u64 {
        self.n
    }

    /// Subtracts the running mean from each sample, in place.
    pub fn remove(&mut self, buf: &mut [Complex32]) {
        for x in buf {
            let m = self.update(*x);
            *x -= m;
        }
    }
}

/// The blind IQ-imbalance estimate and correction of the front end.
///
/// Statistics of the second difference `d[n] = x[n] - 2x[n-1] + x[n-2]` (a high-pass that keeps what is not circular near the centre
/// out of the estimate) are taken over batches of [`IqImbalance::BATCH`] samples (a fixed size, so the result does not depend on
/// how the stream is split). Each batch's means of `dI^2`, `dQ^2`, `dI dQ` are averaged into `PII`, `PQQ`, `PIQ` with a weight
/// of `BATCH / 2^20` (the first batch sets them), and the coefficients are updated after each batch for the samples that follow:
/// `a = PIQ / PII` (clamped to +-0.2), `g = sqrt(PII / (PQQ - a PIQ))` (clamped to 0.708..1.413), and the power scale `s`.
/// [`IqImbalance::correct`] applies `Q' = g (Q - a I)` and scales both by `s`, keeping the total power.
///
/// It expects a signal without DC: the caller removes the running mean first (and adds it back after, when DC removal is off, so a
/// carrier on the centre passes untouched), as [`IqImbalance::process`] does with a [`DcEstimator`].
#[derive(Clone, Debug)]
pub struct IqImbalance {
    y1: Complex32,
    y2: Complex32,
    seen: u64,
    acc: [f64; 3],
    count: usize,
    stats: Option<[f64; 3]>,
    a: f32,
    g: f32,
    s: f32,
}

impl Default for IqImbalance {
    fn default() -> Self {
        Self::new()
    }
}

impl IqImbalance {
    /// Samples per statistics batch.
    pub const BATCH: usize = 4096;

    /// No correction yet (`a = 0`, `g = s = 1`).
    pub fn new() -> Self {
        IqImbalance {
            y1: Complex32::default(),
            y2: Complex32::default(),
            seen: 0,
            acc: [0.0; 3],
            count: 0,
            stats: None,
            a: 0.0,
            g: 1.0,
            s: 1.0,
        }
    }

    /// Forgets the estimate.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// The coefficients `(a, g, s)` in use.
    pub fn coefficients(&self) -> (f32, f32, f32) {
        (self.a, self.g, self.s)
    }

    /// The gain error in dB (`-20 log10 g`) and the phase error in degrees (`asin(a g)`), as measured so far.
    pub fn imbalance(&self) -> (f64, f64) {
        let g = self.g as f64;
        (
            -20.0 * g.log10(),
            (self.a as f64 * g).clamp(-1.0, 1.0).asin().to_degrees(),
        )
    }

    fn update(&mut self) {
        let n = self.count as f64;
        let m = [self.acc[0] / n, self.acc[1] / n, self.acc[2] / n];
        let p = match self.stats {
            None => m,
            Some(p) => {
                let w = (self.count as f64 / (1u64 << 20) as f64).min(1.0);
                [
                    p[0] + (m[0] - p[0]) * w,
                    p[1] + (m[1] - p[1]) * w,
                    p[2] + (m[2] - p[2]) * w,
                ]
            }
        };
        self.stats = Some(p);
        self.acc = [0.0; 3];
        self.count = 0;
        let [pii, pqq, piq] = p;
        if pii <= 0.0 {
            return;
        }
        let a = (piq / pii).clamp(-0.2, 0.2);
        let rest = pqq - a * piq;
        let g = if rest > 0.0 {
            (pii / rest).sqrt().clamp(0.708, 1.413)
        } else {
            1.0
        };
        let qpow = pqq - 2.0 * a * piq + a * a * pii;
        let out = pii + g * g * qpow;
        let s = if out > 0.0 {
            ((pii + pqq) / out).sqrt()
        } else {
            1.0
        };
        (self.a, self.g, self.s) = (a as f32, g as f32, s as f32);
    }

    /// Takes one sample of a signal without DC into the statistics (the coefficients change after each full batch).
    #[inline]
    pub fn observe(&mut self, y: Complex32) {
        if self.seen >= 2 {
            let d = y - self.y1 * 2.0 + self.y2;
            let (i, q) = (d.re as f64, d.im as f64);
            self.acc[0] += i * i;
            self.acc[1] += q * q;
            self.acc[2] += i * q;
            self.count += 1;
        }
        self.seen += 1;
        self.y2 = self.y1;
        self.y1 = y;
        if self.count == Self::BATCH {
            self.update();
        }
    }

    /// The correction of one sample with the coefficients in use.
    #[inline]
    pub fn correct(&self, y: Complex32) -> Complex32 {
        Complex32::new(self.s * y.re, self.s * self.g * (y.im - self.a * y.re))
    }

    /// Corrects `buf` in place, sample by sample: the running mean `dc` is taken out, the sample is observed and corrected (with the
    /// coefficients of the batches before it), and the mean is added back when `keep_dc` is set.
    pub fn process(&mut self, dc: &mut DcEstimator, buf: &mut [Complex32], keep_dc: bool) {
        for x in buf {
            let m = dc.update(*x);
            let y = *x - m;
            let c = self.correct(y);
            self.observe(y);
            *x = if keep_dc { c + m } else { c };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn dc_blocker_removes_dc_and_keeps_signal() {
        let rate = 48_000.0;
        let mut b = DcBlocker::new(rate, 10.0);
        let mut x = ctone(48_000, 1000.0 / rate, 0.5);
        x.iter_mut().for_each(|v| *v += Complex32::new(0.3, -0.2));
        b.process(&mut x);
        let tail = &x[24_000..];
        assert!(
            db(camp(tail, 0.0)) < -60.0,
            "dc left {}",
            db(camp(tail, 0.0))
        );
        assert!(db(camp(tail, 1000.0 / rate) / 0.5).abs() < 0.01);
        let dc = b.dc();
        assert!(
            (dc.re - 0.3).abs() < 0.05 && (dc.im + 0.2).abs() < 0.05,
            "{dc}"
        );
        let mut r = RealDcBlocker::new(rate, 10.0);
        let mut y = vec![1.0f32; 48_000];
        r.process(&mut y);
        assert!(y[47_999].abs() < 1e-3);
    }

    #[test]
    fn the_spec_estimators_remove_dc_and_an_image_whatever_the_cuts() {
        let n = 1 << 20;
        let (ph, g) = (5f32.to_radians(), 1.1f32);
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                let t = i as f64;
                let a = TAU * 0.0123 * t;
                let b = TAU * -0.2711 * t;
                let (i_, q) = (
                    (a.cos() + 0.5 * b.cos()) as f32,
                    (a.sin() + 0.5 * b.sin()) as f32,
                );
                Complex32::new(i_ + 0.2, g * (q * ph.cos() + i_ * ph.sin()) - 0.1)
            })
            .collect();
        let run = |piece: usize| {
            let (mut dc, mut iq) = (DcEstimator::new(), IqImbalance::new());
            let mut y = x.clone();
            for c in y.chunks_mut(piece) {
                iq.process(&mut dc, c, false);
            }
            (y, iq)
        };
        let (y, iq) = run(7777);
        let tail = &y[n - 65536..];
        let image = db(camp(tail, -0.0123) / camp(tail, 0.0123));
        assert!(image < -40.0, "image at {image} dB");
        assert!(db(camp(tail, 0.0)) < -50.0, "dc {}", db(camp(tail, 0.0)));
        let (ge, pe) = iq.imbalance();
        assert!(
            ge.abs() > 0.3 && pe.abs() > 2.0,
            "measured {ge} dB {pe} deg"
        );
        assert_eq!(run(1000).0, y, "the cuts change nothing");
    }

    #[test]
    fn iq_corrector_restores_image_rejection() {
        let rate = 1e6;
        let f = 0.0731;
        let clean = ctone(200_000, f, 0.5);
        // Q gain 1.1 and 5 degrees of skew.
        let (g, phi) = (1.1f32, 5f32.to_radians());
        let mut x: Vec<Complex32> = clean
            .iter()
            .map(|v| Complex32::new(v.re, g * (v.im * phi.cos() + v.re * phi.sin())))
            .collect();
        let before = db(camp(&x[100_000..], -f) / camp(&x[100_000..], f));
        let mut c = IqCorrector::new(rate, 0.01);
        c.process(&mut x);
        let after = db(camp(&x[100_000..], -f) / camp(&x[100_000..], f));
        assert!(before > -30.0, "before {before}");
        assert!(after < -60.0, "after {after}");
        assert!((c.amplitude_imbalance_db() - 20.0 * 1.1f64.log10()).abs() < 0.05);
        assert!((c.phase_error_deg() - 5.0).abs() < 0.1);
    }
}
