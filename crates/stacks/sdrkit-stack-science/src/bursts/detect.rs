//! The spectrogram of the burst monitors and the burst-run detector on it.
//!
//! [`Spectrogram`]: frames of `n` samples without overlap, a Hann window, the power of each bin divided by the sum of the squared
//! window (white noise of variance s² gives s² in every bin), the mean of `frames` frames as one row, bins in increasing
//! frequency with bin 0 at -rate / 2.
//!
//! [`BurstDetector`]: per bin a background `bg` (dB) and a variance `v` (dB²) that start at the first row (`v` = 0.04). The excess
//! of a bin is its dB value minus `bg`. A bin is active when 20 or more quiet rows were learned and its excess is over
//! max(threshold, 5 sqrt(v)). A row is active when at least `min_active` bins are. Only rows that are not active, while no burst
//! is open, update the background: `bg += a e`, `v += a (e² - v)` with `v` at least 0.0025 and
//! `a = max(1 / (learned + 1), row time / 20 s)`. A burst opens on an active row and closes after two inactive rows.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::peak::line_fit;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The transform size for a wanted bin width: from 8, doubled while it is below 256 and rate / (2 n) is at least 0.7 of the
/// wanted bin.
pub fn fft_size(rate: f64, bin_hz: f64) -> usize {
    let mut n = 8usize;
    while n < 256 && rate / (2.0 * n as f64) >= 0.7 * bin_hz {
        n *= 2;
    }
    n
}

/// Rows of averaged power spectra from a stream of complex samples.
pub struct Spectrogram {
    n: usize,
    frames: usize,
    fft: WindowedFft,
    norm: f32,
    frame: Vec<Complex32>,
    filled: usize,
    acc: Vec<f32>,
    counted: usize,
    row: Vec<f32>,
}

impl Spectrogram {
    /// Rows of `frames` frames of `n` points (`n` a power of two, at least 8; `frames` at least 1).
    ///
    /// # Panics
    ///
    /// When `n` is not a power of two of at least 8.
    pub fn new(n: usize, frames: usize) -> Self {
        assert!(n >= 8 && n.is_power_of_two(), "the size is a power of two");
        let fft = WindowedFft::new(n, n, Window::Hann).expect("a power of two is a valid size");
        let norm = 1.0 / fft.window().iter().map(|w| w * w).sum::<f32>();
        Spectrogram {
            n,
            frames: frames.max(1),
            fft,
            norm,
            frame: vec![Complex32::default(); n],
            filled: 0,
            acc: vec![0.0; n],
            counted: 0,
            row: vec![0.0; n],
        }
    }

    /// Bins of a row.
    pub fn bins(&self) -> usize {
        self.n
    }

    /// Samples of a row.
    pub fn row_samples(&self) -> usize {
        self.n * self.frames
    }

    /// Feeds samples; calls `f` with each row completed, in order. Samples that are not finite count as 0.
    pub fn push(&mut self, x: &[Complex32], mut f: impl FnMut(&[f32])) {
        let mut pos = 0;
        while self.fill(x, &mut pos) {
            f(&self.row);
        }
    }

    /// The last row completed.
    pub fn row(&self) -> &[f32] {
        &self.row
    }

    /// Takes samples of `x` from `*pos` on until a row completes (returns `true`; the row is [`Spectrogram::row`]) or `x` ends
    /// (returns `false`). `*pos` moves past the samples taken.
    pub fn fill(&mut self, x: &[Complex32], pos: &mut usize) -> bool {
        while *pos < x.len() {
            let z = x[*pos];
            *pos += 1;
            self.frame[self.filled] = if z.re.is_finite() && z.im.is_finite() {
                z
            } else {
                Complex32::default()
            };
            self.filled += 1;
            if self.filled < self.n {
                continue;
            }
            self.filled = 0;
            let half = self.n / 2;
            let p = self.fft.run_power(&self.frame);
            for (k, a) in self.acc.iter_mut().enumerate() {
                *a += p[(k + half) % self.n];
            }
            self.counted += 1;
            if self.counted == self.frames {
                let s = self.norm / self.frames as f32;
                for (r, a) in self.row.iter_mut().zip(self.acc.iter_mut()) {
                    *r = *a * s;
                    *a = 0.0;
                }
                self.counted = 0;
                return true;
            }
        }
        false
    }
}

/// The rules of a [`BurstDetector`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BurstConfig {
    /// Bins of a row.
    pub bins: usize,
    /// The width of a bin, Hz.
    pub bin_hz: f64,
    /// The lower edge of bin 0, Hz from the centre.
    pub f0_hz: f64,
    /// The time of a row, s.
    pub row_s: f64,
    /// The least excess of an active bin, dB.
    pub threshold_db: f64,
    /// The least active bins of an active row.
    pub min_active: usize,
}

/// A finished burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Burst {
    /// The start of its first active row, s.
    pub start_s: f64,
    /// max(row time, last active row - first active row + row time), s.
    pub duration_s: f64,
    /// The largest mean excess of the active bins of a row, dB.
    pub peak_db: f64,
    /// The largest active bins × bin, Hz.
    pub bandwidth_hz: f64,
    /// The centroid of the first active row, Hz from the centre.
    pub centroid_hz: f64,
    /// The least-squares slope of the centroid against time, Hz/s; `None` with fewer than 4 active rows.
    pub drift_hz_s: Option<f64>,
    /// Active rows.
    pub rows: usize,
}

/// Quiet rows learned before bins can be active.
const LEARN_ROWS: u64 = 20;
/// The variance a background starts with, dB².
const V_START: f64 = 0.04;
/// The least variance, dB².
const V_MIN: f64 = 0.0025;
/// The time constant of the background after the first rows, s.
const BG_TAU_S: f64 = 20.0;
/// The spread factor of the bin rule.
const SPREAD: f64 = 5.0;
/// Inactive rows that close a burst.
const CLOSE_ROWS: u32 = 2;
/// Active rows needed for a drift.
pub const DRIFT_ROWS: usize = 4;

#[derive(Clone, Debug, Default)]
struct Open {
    first_s: f64,
    last_s: f64,
    peak: f64,
    width: f64,
    first_centroid: f64,
    t: Vec<f64>,
    f: Vec<f64>,
    inactive: u32,
}

/// The burst-run detector (module docs).
pub struct BurstDetector {
    cfg: BurstConfig,
    bg: Vec<f64>,
    v: Vec<f64>,
    learned: u64,
    started: bool,
    open: Option<Open>,
    excess: Vec<f32>,
    level_db: Option<f64>,
}

impl BurstDetector {
    /// A detector with the rules `cfg`.
    pub fn new(cfg: BurstConfig) -> Self {
        BurstDetector {
            bg: vec![0.0; cfg.bins],
            v: vec![V_START; cfg.bins],
            excess: vec![0.0; cfg.bins],
            cfg,
            learned: 0,
            started: false,
            open: None,
            level_db: None,
        }
    }

    /// The rules.
    pub fn config(&self) -> &BurstConfig {
        &self.cfg
    }

    /// Sets the threshold of a bin, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.cfg.threshold_db = db;
    }

    /// The excess of each bin of the last row, dB.
    pub fn excess(&self) -> &[f32] {
        &self.excess
    }

    /// dB of the mean bin power of the last row.
    pub fn level_db(&self) -> Option<f64> {
        self.level_db
    }

    /// Whether quiet rows enough were learned for bins to be active.
    pub fn learned(&self) -> bool {
        self.learned >= LEARN_ROWS
    }

    /// Whether a burst is open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// The start of the open burst, s.
    pub fn open_start(&self) -> Option<f64> {
        self.open.as_ref().map(|o| o.first_s)
    }

    /// Takes the row of power values `row` (one per bin, increasing frequency) that starts at `t_s`. Returns the burst that
    /// this row closes.
    pub fn push(&mut self, t_s: f64, row: &[f32]) -> Option<Burst> {
        let c = self.cfg;
        let mean = row.iter().map(|&p| f64::from(p)).sum::<f64>() / row.len().max(1) as f64;
        self.level_db = Some(10.0 * mean.max(1e-30).log10());
        if !self.started {
            for (b, &p) in self.bg.iter_mut().zip(row) {
                *b = 10.0 * f64::from(p).max(1e-30).log10();
            }
            self.started = true;
        }
        let ready = self.learned >= LEARN_ROWS;
        let (mut n_active, mut sum_e, mut sum_ek) = (0usize, 0.0f64, 0.0f64);
        for (k, &p) in row.iter().enumerate() {
            let e = 10.0 * f64::from(p).max(1e-30).log10() - self.bg[k];
            self.excess[k] = e as f32;
            if ready && e > c.threshold_db.max(SPREAD * self.v[k].sqrt()) {
                n_active += 1;
                sum_e += e;
                sum_ek += e * k as f64;
            }
        }
        let active = n_active >= c.min_active.max(1);
        if active {
            let excess = sum_e / n_active as f64;
            let width = n_active as f64 * c.bin_hz;
            let centroid = c.f0_hz + (sum_ek / sum_e + 0.5) * c.bin_hz;
            let o = self.open.get_or_insert_with(|| Open {
                first_s: t_s,
                first_centroid: centroid,
                peak: f64::NEG_INFINITY,
                ..Open::default()
            });
            o.last_s = t_s;
            o.peak = o.peak.max(excess);
            o.width = o.width.max(width);
            o.t.push(t_s);
            o.f.push(centroid);
            o.inactive = 0;
            return None;
        }
        if let Some(o) = self.open.as_mut() {
            o.inactive += 1;
            if o.inactive >= CLOSE_ROWS {
                let o = self.open.take().expect("open");
                return Some(self.finish(o));
            }
            return None;
        }
        let a = (1.0 / (self.learned + 1) as f64).max(c.row_s / BG_TAU_S);
        for (k, &e) in self.excess.iter().enumerate() {
            let e = f64::from(e);
            self.bg[k] += a * e;
            self.v[k] = (self.v[k] + a * (e * e - self.v[k])).max(V_MIN);
        }
        self.learned += 1;
        None
    }

    fn finish(&self, o: Open) -> Burst {
        let rows = o.t.len();
        let drift = (rows >= DRIFT_ROWS)
            .then(|| line_fit(&o.t, &o.f, None).ok().map(|l| l.slope))
            .flatten();
        Burst {
            start_s: o.first_s,
            duration_s: (o.last_s - o.first_s + self.cfg.row_s).max(self.cfg.row_s),
            peak_db: o.peak,
            bandwidth_hz: o.width,
            centroid_hz: o.first_centroid,
            drift_hz_s: drift,
            rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_wanted_bin() {
        assert_eq!(fft_size(2e6, 31_250.0), 64);
        assert_eq!(fft_size(2e6, 62_500.0), 32);
        assert_eq!(fft_size(250e3, 62_500.0), 8);
    }

    #[test]
    fn white_noise_gives_its_variance_in_every_bin() {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let x: Vec<Complex32> = (0..64 * 400)
            .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32))
            .collect();
        let mut s = Spectrogram::new(64, 400);
        let mut rows = Vec::new();
        s.push(&x, |r| rows.push(r.to_vec()));
        assert_eq!(rows.len(), 1);
        let mean = rows[0].iter().sum::<f32>() / 64.0;
        assert!((mean - 2.0).abs() < 0.05, "{mean}");
    }

    #[test]
    fn a_run_of_wide_rows_is_one_burst_with_its_drift() {
        let cfg = BurstConfig {
            bins: 16,
            bin_hz: 1000.0,
            f0_hz: -8000.0,
            row_s: 0.25,
            threshold_db: 1.0,
            min_active: 2,
        };
        let mut d = BurstDetector::new(cfg);
        let mut out = Vec::new();
        for r in 0..60 {
            let mut row = vec![1.0f32; 16];
            if (30..38).contains(&r) {
                let k = 4 + (r - 30);
                row[k] = 8.0;
                row[k + 1] = 8.0;
            }
            out.extend(d.push(r as f64 * 0.25, &row));
        }
        assert_eq!(out.len(), 1, "{out:?}");
        let b = &out[0];
        assert_eq!(b.start_s, 7.5);
        assert_eq!(b.rows, 8);
        assert!((b.duration_s - 2.0).abs() < 1e-9);
        // One bin a row: 4 kHz/s.
        assert!((b.drift_hz_s.unwrap() - 4000.0).abs() < 1.0, "{b:?}");
    }
}
