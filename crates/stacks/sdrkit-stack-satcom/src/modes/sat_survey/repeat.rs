//! The repeated-sequence method: a sequence sent again every period makes the product of each sample with the sample one
//! period earlier coherent where the sequence lies. The products at three lags (the period and one sample either side, for
//! the clock error) are folded over the period into bins; a bin far over the others marks the sequence.

use super::framegap::FrameLine;
use sdrkit_core::Complex32;

/// Bins of the fold.
pub const BINS: usize = 64;
/// Periods of one fold; the fold restarts after them.
pub const FOLD_PERIODS: u32 = 400;
/// A detection needs a level over the threshold plus this, dB.
pub const MARGIN_DB: f64 = 3.0;

/// The repeated-sequence method at one input rate (module documentation).
pub struct RepeatSeq {
    rate: f64,
    period: usize,
    threshold_db: f64,
    ring: Vec<Complex32>,
    n: u64,
    pos: usize,
    at: usize,
    bin_of: Vec<u8>,
    prod: [[Complex32; BINS]; 3],
    pow: [f64; BINS],
    acc: [[f64; BINS]; 3],
    periods: u32,
    line: FrameLine,
    scratch: Vec<f64>,
}

impl RepeatSeq {
    /// The method for input at `rate`, a sequence repeated `repeat_hz` times a second, and the threshold `threshold_db`.
    pub fn new(rate: f64, repeat_hz: f64, threshold_db: f64) -> Self {
        let period = ((rate / repeat_hz).round() as usize).max(BINS);
        RepeatSeq {
            rate,
            period,
            threshold_db,
            ring: vec![Complex32::default(); period + 2],
            n: 0,
            pos: 0,
            at: 0,
            bin_of: (0..period).map(|i| (i * BINS / period) as u8).collect(),
            prod: [[Complex32::default(); BINS]; 3],
            pow: [0.0; BINS],
            acc: [[0.0; BINS]; 3],
            periods: 0,
            line: FrameLine {
                searched_hz: rate / period as f64,
                ..FrameLine::default()
            },
            scratch: Vec::with_capacity(BINS),
        }
    }

    /// The period, samples.
    pub fn period(&self) -> usize {
        self.period
    }

    /// Sets the threshold, dB.
    pub fn set_threshold(&mut self, db: f64) {
        self.threshold_db = db;
    }

    /// The frame line after the last fold.
    pub fn line(&self) -> FrameLine {
        self.line
    }

    /// Forgets the fold and the line.
    pub fn clear(&mut self) {
        self.acc = [[0.0; BINS]; 3];
        self.periods = 0;
        self.line = FrameLine {
            searched_hz: self.line.searched_hz,
            ..FrameLine::default()
        };
    }

    /// Takes samples (finite values).
    pub fn process(&mut self, x: &[Complex32]) {
        let l = self.period;
        let r = self.ring.len();
        for &z in x {
            let (pos, at) = (self.pos, self.at);
            let b = usize::from(self.bin_of[pos]);
            self.pow[b] += f64::from(z.norm_sqr());
            // Lags l + 1, l and l - 1.
            for (d, lag) in [l + 1, l, l - 1].into_iter().enumerate() {
                if self.n >= lag as u64 {
                    let k = at + r - lag;
                    let old = self.ring[if k >= r { k - r } else { k }];
                    self.prod[d][b] += z * old.conj();
                }
            }
            self.ring[at] = z;
            self.n += 1;
            self.at = if at + 1 == r { 0 } else { at + 1 };
            if pos + 1 == l {
                self.pos = 0;
                self.end_period();
            } else {
                self.pos = pos + 1;
            }
        }
    }

    fn end_period(&mut self) {
        for d in 0..3 {
            for b in 0..BINS {
                if self.pow[b] > 0.0 {
                    self.acc[d][b] += f64::from(self.prod[d][b].norm()) / self.pow[b];
                }
                self.prod[d][b] = Complex32::default();
            }
        }
        self.pow = [0.0; BINS];
        self.periods += 1;
        if self.periods == FOLD_PERIODS {
            self.evaluate();
            self.acc = [[0.0; BINS]; 3];
            self.periods = 0;
        }
    }

    /// The best lag of the fold: (peak − median) / mean absolute deviation from the median.
    fn evaluate(&mut self) {
        let mut best: Option<(f64, usize)> = None;
        for (d, lag) in [self.period + 1, self.period, self.period - 1]
            .into_iter()
            .enumerate()
        {
            self.scratch.clear();
            self.scratch.extend_from_slice(&self.acc[d]);
            let (_, med, _) = self
                .scratch
                .select_nth_unstable_by(BINS / 2, f64::total_cmp);
            let med = *med;
            let peak = self.acc[d].iter().copied().fold(f64::MIN, f64::max);
            let dev = self.acc[d].iter().map(|v| (v - med).abs()).sum::<f64>() / BINS as f64;
            let z = if dev > 0.0 { (peak - med) / dev } else { 0.0 };
            if best.is_none_or(|(bz, _)| z > bz) {
                best = Some((z, lag));
            }
        }
        if let Some((z, lag)) = best {
            self.line.level_db = 10.0 * z.max(1e-6).log10();
            self.line.measured_hz = self.rate / lag as f64;
            self.line.detected = self.line.level_db > self.threshold_db + MARGIN_DB;
            self.line.searches += 1;
        }
    }
}
