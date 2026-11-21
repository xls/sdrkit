//! The two lines of the square of an MSK signal, measured over a report of averaged spectra: the bit rate and the carrier
//! without demodulating. MSK puts its two tones a quarter of the bit rate `R` each side of the carrier `fc`; the square of
//! the signal has lines at `2 fc - R / 2` and `2 fc + R / 2`, so their spacing is `R` and their middle `2 fc`.
//!
//! [`SquaredLines`] sums, over Hann segments with half overlap, the power spectrum of the samples and that of their squares.
//! [`SquaredLines::report`] finds the strongest line of the squared spectrum within a span, a second line within a range of
//! distances from it, both refined by a parabola, and the level of the signal over the noise from the median of the plain
//! spectrum. It then starts a new report.
//!
//! ```
//! use sdrkit_algo_modem::msklines::SquaredLines;
//! use sdrkit_algo_modem::Complex32;
//! let (rate, r, fc) = (4000.0, 200.0, 30.0);
//! let mut s = SquaredLines::new(4096);
//! let (mut ph, mut bits) = (0.0f64, 7u32);
//! for i in 0..40_000 {
//!     if i % 20 == 0 {
//!         bits = bits.wrapping_mul(1_103_515_245).wrapping_add(12_345);
//!     }
//!     let d = if bits >> 16 & 1 == 1 { 0.25 } else { -0.25 };
//!     ph += std::f64::consts::TAU * (fc + d * r) / rate;
//!     s.push(Complex32::from_polar(1.0, ph as f32));
//! }
//! let l = s.report(rate, 1200.0, 20.0, 420.0, 9.0).unwrap();
//! let (f1, f2) = (l.line1.unwrap(), l.line2.unwrap());
//! assert!(((f2 - f1).abs() - r).abs() < 1.0 && ((f1 + f2) / 4.0 - fc).abs() < 1.0);
//! ```

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The measure of one report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineReport {
    /// The level of the signal over the noise, dB.
    pub level_db: f64,
    /// The strongest line of the squared spectrum, Hz, when it stands over the threshold.
    pub line1: Option<f64>,
    /// The second line, Hz, when it stands over the threshold and over a fifth of the first.
    pub line2: Option<f64>,
}

/// The spectra of a report (module documentation).
pub struct SquaredLines {
    n: usize,
    fft: Fft,
    window: Vec<f32>,
    ring: Vec<Complex32>,
    pos: usize,
    filled: usize,
    since: usize,
    work: Vec<Complex32>,
    p: Vec<f64>,
    q: Vec<f64>,
    segments: u32,
}

impl SquaredLines {
    /// A measure over segments of `n` samples (a power of two).
    pub fn new(n: usize) -> Self {
        let n = n.next_power_of_two().max(16);
        SquaredLines {
            n,
            fft: Fft::forward(n),
            window: (0..n)
                .map(|i| (0.5 - 0.5 * (TAU * (i as f64 + 0.5) / n as f64).cos()) as f32)
                .collect(),
            ring: vec![Complex32::default(); n],
            pos: 0,
            filled: 0,
            since: 0,
            work: vec![Complex32::default(); n],
            p: vec![0.0; n],
            q: vec![0.0; n],
            segments: 0,
        }
    }

    /// Segments in the report so far.
    pub fn segments(&self) -> u32 {
        self.segments
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.ring.iter_mut().for_each(|v| *v = Complex32::default());
        self.pos = 0;
        self.filled = 0;
        self.since = 0;
        self.p.iter_mut().for_each(|v| *v = 0.0);
        self.q.iter_mut().for_each(|v| *v = 0.0);
        self.segments = 0;
    }

    /// Takes one sample; a segment is taken every `n / 2` samples once `n` were seen.
    pub fn push(&mut self, x: Complex32) {
        self.ring[self.pos] = x;
        self.pos = (self.pos + 1) % self.n;
        self.filled = (self.filled + 1).min(self.n);
        self.since += 1;
        if self.filled < self.n || self.since < self.n / 2 {
            return;
        }
        self.since = 0;
        let n = self.n;
        for i in 0..n {
            self.work[i] = self.ring[(self.pos + i) % n] * self.window[i];
        }
        let sq: Vec<Complex32> = self.work.iter().map(|v| v * v).collect();
        self.fft.process(&mut self.work);
        for (p, v) in self.p.iter_mut().zip(&self.work) {
            *p += f64::from(v.norm_sqr());
        }
        self.work.copy_from_slice(&sq);
        self.fft.process(&mut self.work);
        for (q, v) in self.q.iter_mut().zip(&self.work) {
            *q += f64::from(v.norm_sqr());
        }
        self.segments += 1;
    }

    fn freq(&self, k: f64, rate: f64) -> f64 {
        let n = self.n as f64;
        let k = if k >= n / 2.0 { k - n } else { k };
        k * rate / n
    }

    fn refined(&self, k: usize, rate: f64) -> f64 {
        let n = self.n;
        let (a, b, c) = (self.q[(k + n - 1) % n], self.q[k], self.q[(k + 1) % n]);
        let den = a - 2.0 * b + c;
        let d = if den < 0.0 {
            (0.5 * (a - c) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        self.freq(k as f64 + d, rate)
    }

    /// The measure of the report (`None` with fewer than 3 segments), and a new report begins. Lines are searched within
    /// `span_hz` of 0 Hz, the second `min_sep_hz` to `max_sep_hz` from the first, both over `factor` times the median bin.
    pub fn report(
        &mut self,
        rate: f64,
        span_hz: f64,
        min_sep_hz: f64,
        max_sep_hz: f64,
        factor: f64,
    ) -> Option<LineReport> {
        if self.segments < 3 {
            return None;
        }
        let n = self.n;
        let segs = f64::from(self.segments);
        let mut sorted: Vec<f64> = self.p.iter().map(|v| v / segs).collect();
        let total: f64 = sorted.iter().sum();
        sorted.sort_by(f64::total_cmp);
        let noise_total = sorted[n / 2] / 0.693 * n as f64 * 0.9;
        let level_db = 10.0 * (total / noise_total.max(1e-30) - 1.0).max(1e-6).log10();
        let mut qs = self.q.clone();
        qs.sort_by(f64::total_cmp);
        let qmed = qs[n / 2].max(1e-30);
        let within = |k: usize| self.freq(k as f64, rate).abs() <= span_hz;
        let best = (0..n)
            .filter(|&k| within(k))
            .max_by(|&a, &b| self.q[a].total_cmp(&self.q[b]));
        let mut line1 = None;
        let mut line2 = None;
        if let Some(k1) = best.filter(|&k| self.q[k] > factor * qmed) {
            let f1 = self.freq(k1 as f64, rate);
            line1 = Some(self.refined(k1, rate));
            let k2 = (0..n)
                .filter(|&k| {
                    let d = (self.freq(k as f64, rate) - f1).abs();
                    within(k) && d >= min_sep_hz && d <= max_sep_hz
                })
                .max_by(|&a, &b| self.q[a].total_cmp(&self.q[b]));
            if let Some(k2) =
                k2.filter(|&k| self.q[k] > 0.2 * self.q[k1] && self.q[k] > factor * qmed)
            {
                line2 = Some(self.refined(k2, rate));
            }
        }
        self.p.iter_mut().for_each(|v| *v = 0.0);
        self.q.iter_mut().for_each(|v| *v = 0.0);
        self.segments = 0;
        Some(LineReport {
            level_db,
            line1,
            line2,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_gives_no_line() {
        let mut s = SquaredLines::new(4096);
        let mut seed = 99u64;
        for _ in 0..16_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let a = (seed >> 40) as f32 / 16_777_216.0 - 0.5;
            let b = ((seed >> 16) & 0xFF_FFFF) as f32 / 16_777_216.0 - 0.5;
            s.push(Complex32::new(a, b));
        }
        let r = s.report(4000.0, 1200.0, 20.0, 420.0, 9.0).unwrap();
        assert!(r.line1.is_none() && r.level_db < 1.0, "{r:?}");
    }
}
