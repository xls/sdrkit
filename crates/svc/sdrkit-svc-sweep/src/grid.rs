//! The bin grid of a sweep, and the FFT size for a resolution. Bin `k` covers `low + k·Δ` to `low + (k + 1)·Δ`.

use crate::SweepError;

/// The most bins of a grid. A finer resolution over a wider range is widened to fit.
pub const MAX_BINS: usize = 4_000_000;

/// The level of a bin no hop covered, dBFS.
pub const UNCOVERED_DB: f32 = -200.0;

/// The smallest FFT a hop uses.
pub const MIN_FFT: usize = 64;

/// The largest FFT a hop uses.
pub const MAX_FFT: usize = 65_536;

/// The largest FFT with a radio's own fast sweep (its blocks are short).
pub const MAX_FFT_RADIO_SWEEP: usize = 4096;

/// A bin grid over a frequency range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    /// The low edge of bin 0, Hz.
    pub low_hz: f64,
    /// The width of a bin, Hz.
    pub bin_hz: f64,
    /// The number of bins.
    pub bins: usize,
}

impl Grid {
    /// A grid from `low_hz` to (at least) `high_hz` with bins of `bin_hz`, widened if it needs more than [`MAX_BINS`] bins.
    pub fn new(low_hz: f64, high_hz: f64, bin_hz: f64) -> Result<Self, SweepError> {
        if !(low_hz.is_finite() && high_hz.is_finite() && high_hz > low_hz) {
            return Err(SweepError::EmptyRange { low_hz, high_hz });
        }
        if !(bin_hz.is_finite() && bin_hz > 0.0) {
            return Err(SweepError::BadResolution { bin_hz });
        }
        let span = high_hz - low_hz;
        let bin = if span / bin_hz > MAX_BINS as f64 {
            span / MAX_BINS as f64
        } else {
            bin_hz
        };
        let bins = ((span / bin) - 1e-9).ceil().max(1.0) as usize;
        Ok(Grid {
            low_hz,
            bin_hz: bin,
            bins,
        })
    }

    /// The high edge of the last bin, Hz.
    pub fn high_hz(&self) -> f64 {
        self.low_hz + self.bin_hz * self.bins as f64
    }

    /// The centre of bin `k`, Hz.
    pub fn centre_hz(&self, k: usize) -> f64 {
        self.low_hz + (k as f64 + 0.5) * self.bin_hz
    }

    /// The bin holding `hz`, if the grid covers it.
    pub fn index_of(&self, hz: f64) -> Option<usize> {
        let x = (hz - self.low_hz) / self.bin_hz;
        if x < 0.0 || x >= self.bins as f64 || !x.is_finite() {
            None
        } else {
            Some(x as usize)
        }
    }

    /// The bins whose centres lie in `lo_hz ..= hi_hz`, as `start..end` (empty when none).
    pub fn bins_between(&self, lo_hz: f64, hi_hz: f64) -> std::ops::Range<usize> {
        let a = ((lo_hz - self.low_hz) / self.bin_hz - 0.5).ceil().max(0.0);
        let b = ((hi_hz - self.low_hz) / self.bin_hz - 0.5).floor() + 1.0;
        let a = (a as usize).min(self.bins);
        let b = (b.max(0.0) as usize).min(self.bins);
        a..b.max(a)
    }

    /// The same grid shape at another place: true when the two grids line up bin for bin.
    pub fn same_as(&self, other: &Grid) -> bool {
        self.bins == other.bins
            && (self.low_hz - other.low_hz).abs() < self.bin_hz * 1e-3
            && (self.bin_hz - other.bin_hz).abs() < self.bin_hz * 1e-6
    }
}

/// The FFT size for a resolution: the smallest power of two from [`MIN_FFT`] whose bins are no wider than `bin_hz` (with 1 %
/// slack), capped at `max_fft`.
pub fn fft_size_for(rate_hz: f64, bin_hz: f64, max_fft: usize) -> usize {
    let mut n = MIN_FFT;
    while n < max_fft && rate_hz / n as f64 > bin_hz * 1.01 {
        n *= 2;
    }
    n.min(max_fft.max(MIN_FFT))
}

/// The resolutions offered at a rate: every `rate / 2^k` no wider than 1 MHz, coarsest first, down to the finest FFT allowed.
pub fn resolution_choices(rate_hz: f64, max_fft: usize) -> Vec<f64> {
    let mut out = Vec::new();
    let mut n = 1usize;
    while n <= max_fft {
        let b = rate_hz / n as f64;
        if b <= 1e6 * (1.0 + 1e-9) {
            out.push(b);
        }
        n *= 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_sizes() {
        assert_eq!(Grid::new(100e6, 110e6, 100e3).unwrap().bins, 100);
        assert_eq!(Grid::new(1e6, 6000e6, 5e3).unwrap().bins, 1_199_800);
        let g = Grid::new(1e6, 6000e6, 2.8e3).unwrap();
        assert_eq!(g.bins, 2_142_500);
        assert_eq!(g.bin_hz, 2.8e3);
        // Too fine: widened to the cap.
        let g = Grid::new(1e6, 6000e6, 100.0).unwrap();
        assert_eq!(g.bins, MAX_BINS);
        assert!(g.bin_hz > 100.0);
        assert!(Grid::new(5.0, 5.0, 1.0).is_err());
        assert!(Grid::new(1.0, 5.0, 0.0).is_err());
    }

    #[test]
    fn grid_indexing() {
        let g = Grid::new(100e6, 101e6, 100e3).unwrap();
        assert_eq!(g.centre_hz(0), 100.05e6);
        assert_eq!(g.index_of(100.05e6), Some(0));
        assert_eq!(g.index_of(100.999e6), Some(9));
        assert_eq!(g.index_of(101e6), None);
        assert_eq!(g.index_of(99e6), None);
        assert_eq!(g.bins_between(100.0e6, 100.3e6), 0..3);
        assert_eq!(g.bins_between(100.06e6, 100.3e6), 1..3);
        assert_eq!(g.bins_between(200e6, 300e6), 10..10);
        assert!(g.same_as(&Grid::new(100e6, 101e6, 100e3).unwrap()));
    }

    #[test]
    fn fft_sizes_and_choices() {
        assert_eq!(fft_size_for(2.4e6, 100e3, MAX_FFT), MIN_FFT);
        assert_eq!(fft_size_for(2.4e6, 2.4e6 / 1024.0, MAX_FFT), 1024);
        assert_eq!(fft_size_for(2.4e6, 2.4e6 / 1030.0, MAX_FFT), 1024); // within the 1 % slack
        assert_eq!(fft_size_for(20e6, 1.0, MAX_FFT_RADIO_SWEEP), 4096);
        assert_eq!(fft_size_for(20e6, 1.0, MAX_FFT), MAX_FFT);
        let c = resolution_choices(2.048e6, MAX_FFT);
        assert_eq!(c[0], 512e3);
        assert!(c.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(*c.last().unwrap(), 2.048e6 / 65536.0);
    }
}
