//! The energies of the tones of a slow MFSK signal on a grid of time and frequency, and the fold of a message that repeats.
//!
//! [`ToneGrid`] takes one window of a symbol's length every `step` samples (a quarter symbol, say), zero-padded to a power of two,
//! and keeps the power of the bins of a band. [`ToneGrid::power`] reads it at any frequency by linear interpolation between bins,
//! so a receiver can try tone sets on a grid of fractions of the tone spacing. [`Fold`] sums values of the tones by position
//! modulo a period over many repeats of a message, and scores how well one tone stands out at each position.

use crate::error::{ModemError, Param};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_core::Complex32;

/// The window of a column of a [`ToneGrid`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridWindow {
    /// Hann.
    Hann,
    /// No window.
    Rectangular,
}

/// How a [`ToneGrid`] is made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSpec {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The window length: one symbol, samples (rounded).
    pub symbol: f64,
    /// The FFT size is `pad` times the next power of two at or above the window length.
    pub pad: usize,
    /// The window.
    pub window: GridWindow,
    /// Samples between two columns (fractional steps are rounded per column, without drift).
    pub step: f64,
    /// The lowest frequency kept, Hz.
    pub f_lo: f64,
    /// The highest frequency kept, Hz.
    pub f_hi: f64,
}

/// The power of every bin of a band, one column every `step` samples.
#[derive(Clone, Debug)]
pub struct ToneGrid {
    step: f64,
    window: usize,
    bin_hz: f64,
    first_bin: usize,
    bins: usize,
    cols: usize,
    data: Vec<f32>,
}

impl ToneGrid {
    /// The grid of `x` as `spec` asks. Column `c` holds the window that starts at sample `round(c × step)`; the last column is
    /// the last whole window.
    ///
    /// # Errors
    ///
    /// [`ModemError::OutOfRange`] for a rate, window length, padding or step that is not positive, or a band outside 0 to half
    /// the rate.
    pub fn new(x: &[f32], spec: &GridSpec) -> Result<ToneGrid, ModemError> {
        let range = |param, min: f64, max: f64, got: f64| {
            if got.is_finite() && got >= min && got <= max {
                Ok(())
            } else {
                Err(ModemError::OutOfRange {
                    param,
                    min,
                    max,
                    got,
                })
            }
        };
        range(Param::Rate, 1.0, 1e9, spec.rate)?;
        range(Param::SymbolLen, 2.0, 1e7, spec.symbol)?;
        range(Param::GridStep, 1.0, 1e7, spec.step)?;
        range(Param::Window, 1.0, 64.0, spec.pad as f64)?;
        range(Param::Frequency, 0.0, spec.rate / 2.0, spec.f_lo)?;
        range(Param::Frequency, spec.f_lo, spec.rate / 2.0, spec.f_hi)?;
        let window = spec.symbol.round() as usize;
        let n = spec.pad * window.next_power_of_two();
        let bin_hz = spec.rate / n as f64;
        let first_bin = (spec.f_lo / bin_hz).floor() as usize;
        let last_bin = ((spec.f_hi / bin_hz).ceil() as usize).min(n / 2);
        let bins = last_bin + 1 - first_bin;
        let cols = if x.len() >= window {
            ((x.len() - window) as f64 / spec.step).floor() as usize + 1
        } else {
            0
        };
        let w: Vec<f32> = match spec.window {
            GridWindow::Hann => (0..window)
                .map(|i| {
                    (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / window as f64).cos()) as f32
                })
                .collect(),
            GridWindow::Rectangular => vec![1.0; window],
        };
        let mut fft = Fft::forward(n);
        let mut buf = vec![Complex32::default(); n];
        let mut data = Vec::with_capacity(cols * bins);
        for c in 0..cols {
            let s = (c as f64 * spec.step).round() as usize;
            buf.fill(Complex32::default());
            for (i, b) in buf.iter_mut().take(window).enumerate() {
                let v = x.get(s + i).copied().unwrap_or(0.0);
                *b = Complex32::new(if v.is_finite() { v * w[i] } else { 0.0 }, 0.0);
            }
            fft.process(&mut buf);
            data.extend(
                buf[first_bin..first_bin + bins]
                    .iter()
                    .map(Complex32::norm_sqr),
            );
        }
        Ok(ToneGrid {
            step: spec.step,
            window,
            bin_hz,
            first_bin,
            bins,
            cols,
            data,
        })
    }

    /// The columns.
    pub fn columns(&self) -> usize {
        self.cols
    }
    /// The bins kept in each column.
    pub fn bins(&self) -> usize {
        self.bins
    }
    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }
    /// The frequency of kept bin 0, Hz.
    pub fn first_hz(&self) -> f64 {
        self.first_bin as f64 * self.bin_hz
    }
    /// The window length, samples.
    pub fn window(&self) -> usize {
        self.window
    }
    /// The first sample of column `c`.
    pub fn start_of(&self, c: usize) -> usize {
        (c as f64 * self.step).round() as usize
    }
    /// The power of every kept bin of column `c` (empty past the last column).
    pub fn column(&self, c: usize) -> &[f32] {
        if c >= self.cols {
            return &[];
        }
        &self.data[c * self.bins..(c + 1) * self.bins]
    }
    /// The power of column `c` at `hz`, linear between the two nearest bins; 0 outside the band or past the last column.
    pub fn power(&self, c: usize, hz: f64) -> f32 {
        let col = self.column(c);
        let pos = hz / self.bin_hz - self.first_bin as f64;
        if col.is_empty() || pos < 0.0 || pos > (self.bins - 1) as f64 {
            return 0.0;
        }
        let i = pos.floor() as usize;
        let f = (pos - i as f64) as f32;
        match col.get(i + 1) {
            Some(&b) => col[i] * (1.0 - f) + b * f,
            None => col[i],
        }
    }
}

/// Values of the tones summed by position modulo a period: the fold of a message that repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct Fold {
    period: usize,
    tones: usize,
    sums: Vec<f32>,
    counts: Vec<u32>,
}

impl Fold {
    /// An empty fold of `period` positions of `tones` values each.
    pub fn new(period: usize, tones: usize) -> Fold {
        let period = period.max(1);
        Fold {
            period,
            tones,
            sums: vec![0.0; period * tones],
            counts: vec![0; period],
        }
    }
    /// The period.
    pub fn period(&self) -> usize {
        self.period
    }
    /// Adds the values of the tones at item `index` (position `index` modulo the period). Values past the tone count are
    /// ignored.
    pub fn add(&mut self, index: usize, values: &[f32]) {
        let p = index % self.period;
        for (s, &v) in self.sums[p * self.tones..(p + 1) * self.tones]
            .iter_mut()
            .zip(values)
        {
            *s += v;
        }
        self.counts[p] += 1;
    }
    /// The sums of position `p`.
    pub fn sums(&self, p: usize) -> &[f32] {
        let p = p % self.period;
        &self.sums[p * self.tones..(p + 1) * self.tones]
    }
    /// How many items position `p` holds.
    pub fn count(&self, p: usize) -> u32 {
        self.counts[p % self.period]
    }
    /// The strongest tone of position `p`, with its sum and the mean of the other sums. `None` for a position without items.
    pub fn best(&self, p: usize) -> Option<(usize, f32, f32)> {
        if self.count(p) == 0 || self.tones == 0 {
            return None;
        }
        let s = self.sums(p);
        let (mut k, mut top) = (0, f32::MIN);
        for (i, &v) in s.iter().enumerate() {
            if v > top {
                (k, top) = (i, v);
            }
        }
        let rest = if self.tones > 1 {
            (s.iter().sum::<f32>() - top) / (self.tones - 1) as f32
        } else {
            0.0
        };
        Some((k, top, rest))
    }
    /// The agreement of the fold: the mean over the positions with at least `min_count` items of (largest sum / total sum), and
    /// how many positions that is.
    pub fn agreement(&self, min_count: u32) -> (f64, usize) {
        let (mut acc, mut n) = (0.0f64, 0usize);
        for p in 0..self.period {
            if self.counts[p] < min_count.max(1) {
                continue;
            }
            let s = self.sums(p);
            let total: f32 = s.iter().sum();
            let top = s.iter().copied().fold(0.0f32, f32::max);
            if total > 0.0 {
                acc += f64::from(top / total);
            }
            n += 1;
        }
        (if n > 0 { acc / n as f64 } else { 0.0 }, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(f: f64, rate: f64, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| (std::f64::consts::TAU * f * i as f64 / rate).cos() as f32)
            .collect()
    }

    #[test]
    fn a_tone_shows_at_its_frequency_in_every_column() {
        let x = tone(1234.5, 8000.0, 8000);
        let spec = GridSpec {
            rate: 8000.0,
            symbol: 371.5,
            pad: 4,
            window: GridWindow::Hann,
            step: 371.5 / 4.0,
            f_lo: 1000.0,
            f_hi: 2000.0,
        };
        let g = ToneGrid::new(&x, &spec).unwrap();
        assert_eq!(g.window(), 372);
        assert!((g.bin_hz() - 8000.0 / 2048.0).abs() < 1e-9);
        assert_eq!(g.columns(), ((8000 - 372) as f64 / 92.875) as usize + 1);
        for c in [0, 10, g.columns() - 1] {
            let on = g.power(c, 1234.5);
            assert!(on > 1000.0 * g.power(c, 1300.0), "{c}");
            assert!(on > 0.9 * g.power(c, 1234.5 + g.bin_hz() / 2.0));
        }
        assert_eq!(g.power(0, 500.0), 0.0);
        assert_eq!(g.power(g.columns(), 1234.5), 0.0);
        assert_eq!(g.start_of(4), 372);
    }

    #[test]
    fn a_bad_spec_is_refused() {
        let spec = GridSpec {
            rate: 8000.0,
            symbol: 100.0,
            pad: 2,
            window: GridWindow::Rectangular,
            step: 0.0,
            f_lo: 0.0,
            f_hi: 4000.0,
        };
        assert!(ToneGrid::new(&[0.0; 10], &spec).is_err());
        let ok = GridSpec { step: 25.0, ..spec };
        assert_eq!(ToneGrid::new(&[0.0; 10], &ok).unwrap().columns(), 0);
        assert!(ToneGrid::new(&[0.0; 10], &GridSpec { f_hi: 5000.0, ..ok }).is_err());
    }

    #[test]
    fn a_fold_finds_the_repeating_tones() {
        let msg = [3usize, 1, 4, 1, 5];
        let mut f = Fold::new(5, 6);
        for i in 0..23 {
            let mut v = [0.1f32; 6];
            v[msg[i % 5]] = 1.0;
            f.add(i, &v);
        }
        for (p, &t) in msg.iter().enumerate() {
            assert_eq!(f.best(p).unwrap().0, t);
        }
        assert_eq!(f.count(0), 5);
        assert_eq!(f.count(4), 4);
        let (score, n) = f.agreement(2);
        assert_eq!(n, 5);
        assert!((score - 1.0 / 1.5).abs() < 1e-6, "{score}");
        // A wrong period agrees less.
        let mut g = Fold::new(4, 6);
        for i in 0..23 {
            let mut v = [0.1f32; 6];
            v[msg[i % 5]] = 1.0;
            g.add(i, &v);
        }
        assert!(g.agreement(2).0 < score);
        assert_eq!(Fold::new(3, 2).best(0), None);
    }
}
