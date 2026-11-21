//! The structure of an OFDM signal with a cyclic prefix, from the signal alone: the useful length `Tu` (the lag where the
//! normalised autocorrelation peaks), the symbol rate (the moving correlation of the guard with the end of the symbol repeats once
//! a symbol), the symbol length refined by folding, and where the guard lies.
//!
//! ```
//! use sdrkit_algo_modem::guardscan::{symbol_timing, GuardScan};
//! use sdrkit_algo_modem::Complex32;
//! // 40 symbols of 64 useful samples and a 16-sample prefix.
//! let mut x = Vec::new();
//! let mut s = 7u32;
//! for _ in 0..40 {
//!     let body: Vec<Complex32> = (0..64)
//!         .map(|_| {
//!             s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
//!             Complex32::from_polar(1.0, (s >> 8) as f32 * 1e-4)
//!         })
//!         .collect();
//!     x.extend_from_slice(&body[48..]);
//!     x.extend_from_slice(&body);
//! }
//! let mut g = GuardScan::new(4096, 100).unwrap();
//! let tu = g.useful_length(&x, 40, 100, 0.06).unwrap();
//! assert!((tu.samples - 64.0).abs() < 0.5);
//! let t = symbol_timing(&x, 64, 8000.0, 50.0, 400.0).unwrap();
//! assert!((t.symbol - 80.0).abs() < 0.5);
//! ```

use crate::correlate::Autocorrelation;
use crate::error::ModemError;
use crate::Complex32;
use sdrkit_algo_dsp::fft::Fft;

/// The useful length found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsefulLength {
    /// The useful length, samples (refined by a parabola).
    pub samples: f64,
    /// The normalised correlation at it.
    pub height: f64,
    /// The phase of the correlation at it, radians: `-2 pi f Tu` for a carrier offset `f` (cycles a sample).
    pub phase: f32,
}

/// The symbol timing found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymbolTiming {
    /// The symbol rate, Hz.
    pub rate_hz: f64,
    /// The symbol length, samples.
    pub symbol: f64,
    /// Where the moving guard correlation peaks, samples from the start of the input, modulo the symbol length: the end of a
    /// guard less half the correlation length.
    pub peak: f64,
    /// The sharpness of the fold: its peak over its mean.
    pub sharpness: f64,
}

/// The useful-length search. See the module documentation.
pub struct GuardScan {
    ac: Autocorrelation,
    r: Vec<Complex32>,
}

impl GuardScan {
    /// A search over captures of up to `max_len` samples and lags up to `max_lag`.
    pub fn new(max_len: usize, max_lag: usize) -> Result<Self, ModemError> {
        Ok(GuardScan {
            ac: Autocorrelation::new(max_len, max_lag)?,
            r: vec![Complex32::default(); max_lag + 1],
        })
    }

    /// The lag in `lmin ..= lmax` (at most the largest lag) where `|r(L)| / r(0)` peaks, if the peak stands `min_height` above
    /// the median of the range.
    pub fn useful_length(
        &mut self,
        x: &[Complex32],
        lmin: usize,
        lmax: usize,
        min_height: f64,
    ) -> Option<UsefulLength> {
        let lmax = lmax.min(self.ac.max_lag());
        if lmin < 1 || lmax <= lmin + 2 || x.len() <= lmax {
            return None;
        }
        self.ac.process(x, &mut self.r).ok()?;
        let e = f64::from(self.r[0].re).max(1e-30);
        let v: Vec<f64> = (lmin..=lmax)
            .map(|l| f64::from(self.r[l].norm()) / e)
            .collect();
        let mut sorted = v.clone();
        sorted.sort_by(f64::total_cmp);
        let med = sorted[sorted.len() / 2];
        let (i, &h) = v.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
        if h - med < min_height {
            return None;
        }
        let d = if i > 0 && i + 1 < v.len() {
            let (l, r) = (v[i - 1], v[i + 1]);
            let den = l - 2.0 * h + r;
            if den.abs() > 1e-30 {
                (0.5 * (l - r) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            }
        } else {
            0.0
        };
        Some(UsefulLength {
            samples: (lmin + i) as f64 + d,
            height: h,
            phase: self.r[lmin + i].arg(),
        })
    }
}

/// The magnitude of the moving sum of `x(n) x*(n + tu)` over `g` samples: value `n` is the sum that ends at `n`.
pub fn guard_correlation(x: &[Complex32], tu: usize, g: usize) -> Vec<f32> {
    if x.len() <= tu + g {
        return Vec::new();
    }
    let n = x.len() - tu;
    let mut out = Vec::with_capacity(n);
    let mut acc = Complex32::default();
    for i in 0..n {
        acc += x[i] * x[i + tu].conj();
        if i >= g {
            acc -= x[i - g] * x[i - g + tu].conj();
        }
        out.push(acc.norm());
    }
    out
}

/// The symbol timing of `x` (at `rate`) with useful length `tu`: the line of the guard correlation (over `max(8, tu / 8)`
/// samples) in `lo_hz ..= hi_hz`, the line at half its frequency preferred when it is at least 0.3 of it; then the symbol length
/// within 0.5 % of it, in steps of 0.025 %, whose fold is sharpest.
pub fn symbol_timing(
    x: &[Complex32],
    tu: usize,
    rate: f64,
    lo_hz: f64,
    hi_hz: f64,
) -> Option<SymbolTiming> {
    let g = (tu / 8).max(8);
    let m = guard_correlation(x, tu, g);
    if m.len() < 64 {
        return None;
    }
    let nfft = m.len().next_power_of_two().max(32_768);
    let mean = m.iter().map(|&v| f64::from(v)).sum::<f64>() / m.len() as f64;
    let mut buf: Vec<Complex32> = m
        .iter()
        .map(|&v| Complex32::new((f64::from(v) - mean) as f32, 0.0))
        .collect();
    buf.resize(nfft, Complex32::default());
    let mut fft = Fft::forward(nfft);
    fft.process(&mut buf);
    let bin = rate / nfft as f64;
    let kl = (lo_hz / bin).ceil() as usize;
    let kh = ((hi_hz / bin).floor() as usize).min(nfft / 2 - 1);
    if kh <= kl {
        return None;
    }
    let p = |k: usize| buf[k].norm_sqr();
    let k = (kl..=kh).max_by(|&a, &b| p(a).total_cmp(&p(b)))?;
    let mut f = k as f64 * bin;
    // The fundamental: a line at half the frequency of at least 0.3 of the strongest.
    let half = (k as f64 / 2.0).round() as usize;
    if half >= 1 {
        let kh2 = (half.saturating_sub(2)..=half + 2).max_by(|&a, &b| p(a).total_cmp(&p(b)))?;
        if p(kh2) >= 0.3 * p(k) && kh2 as f64 * bin >= 0.5 * lo_hz {
            f = kh2 as f64 * bin;
        }
    }
    let ts0 = rate / f;
    let mut best = (ts0, 0.0f64, 0.0f64);
    let steps = 20;
    for s in -steps..=steps {
        let ts = ts0 * (1.0 + 0.00025 * f64::from(s));
        let nb = ts.round().max(2.0) as usize;
        let mut fold = vec![0.0f64; nb];
        let mut cnt = vec![0u32; nb];
        for (i, &v) in m.iter().enumerate() {
            let b = (((i as f64) % ts) / ts * nb as f64) as usize;
            let b = b.min(nb - 1);
            fold[b] += f64::from(v);
            cnt[b] += 1;
        }
        for (f, &c) in fold.iter_mut().zip(&cnt) {
            *f /= f64::from(c.max(1));
        }
        let mean = fold.iter().sum::<f64>() / nb as f64;
        let (pi, &pv) = fold.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1))?;
        let sharp = pv / mean.max(1e-30);
        if sharp > best.2 {
            best = (ts, pi as f64 / nb as f64 * ts, sharp);
        }
    }
    Some(SymbolTiming {
        rate_hz: rate / best.0,
        symbol: best.0,
        peak: best.1,
        sharpness: best.2,
    })
}
