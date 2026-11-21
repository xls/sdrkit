//! A grid of equal channels over the middle of the span of an FFT: each channel is a run of whole bins, the outer part of the
//! span is left out, and the bins of the DC spike belong to no channel.
//!
//! With `N` points, bin width `b = rate / N` and a channel width `W`, a channel holds `c = max(2, round(W / b))` bins and the
//! grid has `n = floor(share N / c)` channels. Signed bin `j` belongs to channel `floor((j + n c / 2) / c)` when that lies in
//! `0..n`. With an even `n` the grid straddles the centre and no channel is centred on it.
//!
//! ```
//! use sdrkit_algo_dsp::channelgrid::ChannelGrid;
//! let g = ChannelGrid::new(2048, 2e6, 25e3, 0.9).unwrap();
//! assert_eq!((g.channels(), g.bins_per_channel()), (70, 26));
//! ```

use crate::{DspError, Param};

/// A channel grid (module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelGrid {
    n_fft: usize,
    bin_hz: f64,
    per: usize,
    channels: usize,
    /// The channel of each bin in signed order, `u32::MAX` for none.
    of_bin: Vec<u32>,
}

impl ChannelGrid {
    /// The grid of channels `channel_hz` wide over the share `share` (0 to 1) of the span of an `n_fft`-point FFT at `rate_hz`.
    /// Signed bins -1, 0 and +1 belong to no channel.
    ///
    /// # Errors
    ///
    /// [`DspError::OutOfRange`] for an FFT of fewer than 8 points, a rate or width that is not positive, a share outside 0 to 1,
    /// or a grid of no channel.
    pub fn new(n_fft: usize, rate_hz: f64, channel_hz: f64, share: f64) -> Result<Self, DspError> {
        if n_fft < 8 {
            return Param::FftSize.at_least(8.0, n_fft as f64);
        }
        if rate_hz.is_nan() || rate_hz <= 0.0 {
            return Param::Rate.positive(rate_hz);
        }
        if channel_hz.is_nan() || channel_hz <= 0.0 {
            return Param::Bandwidth.positive(channel_hz);
        }
        if !(share > 0.0 && share <= 1.0) {
            return Param::Span.range(0.0, 1.0, share);
        }
        let bin_hz = rate_hz / n_fft as f64;
        let per = ((channel_hz / bin_hz).round() as usize).max(2);
        let channels = (share * n_fft as f64 / per as f64).floor() as usize;
        if channels == 0 {
            return Param::Channels.at_least(1.0, 0.0);
        }
        Ok(ChannelGrid {
            n_fft,
            bin_hz,
            per,
            channels,
            of_bin: vec![u32::MAX; n_fft],
        }
        .without_dc(1))
    }

    /// The same grid with the signed bins `-dc..=dc` in no channel (1 by default).
    pub fn without_dc(mut self, dc: i64) -> Self {
        let half = self.n_fft as i64 / 2;
        let offset = self.channels as f64 * self.per as f64 / 2.0;
        for (i, k) in self.of_bin.iter_mut().enumerate() {
            let j = i as i64 - half;
            let c = ((j as f64 + offset) / self.per as f64).floor();
            *k = if j.abs() <= dc || c < 0.0 || c >= self.channels as f64 {
                u32::MAX
            } else {
                c as u32
            };
        }
        self
    }

    /// The bins of channel `k` that count, as indices in signed order (index `i` is bin `i - n_fft / 2`).
    pub fn members(&self, k: usize) -> Vec<usize> {
        (0..self.n_fft)
            .filter(|&i| self.of_bin[i] as usize == k)
            .collect()
    }

    /// The number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }

    /// The bins of a channel.
    pub fn bins_per_channel(&self) -> usize {
        self.per
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }

    /// The width of a channel, Hz.
    pub fn channel_hz(&self) -> f64 {
        self.per as f64 * self.bin_hz
    }

    /// The span that the channels cover, Hz.
    pub fn span_hz(&self) -> f64 {
        self.channels as f64 * self.channel_hz()
    }

    /// The channel of signed bin `j`, if any.
    pub fn channel_of(&self, j: i64) -> Option<usize> {
        let i = j + self.n_fft as i64 / 2;
        if i < 0 || i >= self.n_fft as i64 {
            return None;
        }
        let k = self.of_bin[i as usize];
        (k != u32::MAX).then_some(k as usize)
    }

    /// The centre of channel `k` from the centre of the span, Hz: the mean frequency of the bins that the channel covers.
    pub fn centre_hz(&self, k: usize) -> f64 {
        let lo = k as f64 * self.per as f64 - self.channels as f64 * self.per as f64 / 2.0;
        let first = lo.ceil();
        let last = (lo + self.per as f64).ceil() - 1.0;
        0.5 * (first + last) * self.bin_hz
    }

    /// The bins of each channel that count (without the DC bins).
    pub fn counted_bins(&self) -> Vec<usize> {
        let mut n = vec![0; self.channels];
        for &k in &self.of_bin {
            if k != u32::MAX {
                n[k as usize] += 1;
            }
        }
        n
    }

    /// Adds each power of `power` (signed order, `n_fft` values) to the sum of its channel in `sums` (`channels` values).
    pub fn accumulate(&self, power: &[f32], sums: &mut [f64]) {
        for (&k, &p) in self.of_bin.iter().zip(power) {
            if k != u32::MAX {
                sums[k as usize] += f64::from(p);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_msps_with_25_khz_channels_gives_70_channels_of_26_bins() {
        let g = ChannelGrid::new(2048, 2e6, 25e3, 0.9).unwrap();
        assert_eq!(g.channels(), 70);
        assert_eq!(g.bins_per_channel(), 26);
        assert!((g.channel_hz() - 25_390.6).abs() < 0.1);
        assert!((g.span_hz() - 1.777e6).abs() < 1e3);
        // The grid straddles the centre: channels 34 and 35 meet at bin 0.
        assert_eq!(g.channel_of(-2), Some(34));
        assert_eq!(g.channel_of(2), Some(35));
        assert_eq!(g.channel_of(0), None);
        assert_eq!(g.channel_of(-910), Some(0));
        assert_eq!(g.channel_of(-911), None);
        assert_eq!(g.channel_of(909), Some(69));
        assert_eq!(g.channel_of(910), None);
        assert!((g.centre_hz(35) - 12.5 * g.bin_hz()).abs() < 1e-9);
        let n = g.counted_bins();
        assert_eq!((n[0], n[34], n[35]), (26, 25, 24));
        assert_eq!(g.members(35).len(), 24);
        let g3 = g.clone().without_dc(2);
        assert_eq!((g3.channel_of(2), g3.channel_of(3)), (None, Some(35)));
        let mut sums = vec![0.0; 70];
        g.accumulate(&vec![1.0; 2048], &mut sums);
        assert_eq!(sums[0], 26.0);
    }

    #[test]
    fn a_grid_that_holds_no_channel_is_refused() {
        assert!(ChannelGrid::new(64, 1e3, 1e3, 0.9).is_err());
        assert!(ChannelGrid::new(2048, 2e6, 25e3, 1.5).is_err());
    }
}
