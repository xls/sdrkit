//! The filter bank of the pulsar stack: Hann-windowed transforms of `C` samples without overlap, channel `c` the bin
//! `(c + C / 2) mod C` (channel 0 the lowest frequency), and a row the mean of `R` such spectra.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// The fewest and the most channels.
pub const CHANNELS: (usize, usize) = (8, 1024);

/// The shape of the rows at one input rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Channels `C`: the next power of two from the setting, 8 to 1024.
    pub channels: usize,
    /// Spectra a row `R = max(1, round(sample time x rate / C))`.
    pub spectra: usize,
    /// The time of a row, `R C / rate`, s.
    pub row_s: f64,
}

impl Layout {
    /// The layout for input at `rate` Hz, `channels` asked and a sample time of `sample_ms`.
    pub fn new(rate: f64, channels: usize, sample_ms: f64) -> Layout {
        let c = channels
            .max(CHANNELS.0)
            .next_power_of_two()
            .min(CHANNELS.1);
        let r = ((sample_ms * 1e-3 * rate / c as f64).round() as usize).max(1);
        Layout {
            channels: c,
            spectra: r,
            row_s: (r * c) as f64 / rate,
        }
    }

    /// The radio frequency of the centre of each channel for a radio centred at `centre_hz` at `rate` Hz: the frequency of its
    /// bin, `centre - rate / 2 + c rate / C`.
    pub fn freqs(&self, centre_hz: f64, rate: f64) -> Vec<f64> {
        let c = self.channels as f64;
        (0..self.channels)
            .map(|i| centre_hz - rate / 2.0 + i as f64 * rate / c)
            .collect()
    }
}

/// The filter bank: fed IQ in pieces of any size, it calls back with each row. Rows start at its first sample.
pub struct FilterBank {
    layout: Layout,
    fft: WindowedFft,
    frame: Vec<Complex32>,
    acc: Vec<f32>,
    spectra: usize,
    row: Vec<f32>,
}

impl std::fmt::Debug for FilterBank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterBank")
            .field("layout", &self.layout)
            .finish()
    }
}

impl FilterBank {
    /// A filter bank of `layout`.
    pub fn new(layout: Layout) -> FilterBank {
        let c = layout.channels;
        FilterBank {
            layout,
            // The length is a power of two from 8 to 1024, which the transform always takes.
            fft: WindowedFft::new(c, c, Window::Hann).unwrap_or_else(|_| unreachable!()),
            frame: Vec::with_capacity(c),
            acc: vec![0.0; c],
            spectra: 0,
            row: vec![0.0; c],
        }
    }

    /// Its layout.
    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Feeds `x` and calls `row` with each row made, its values by channel.
    pub fn push(&mut self, x: &[Complex32], mut row: impl FnMut(&[f32])) {
        let c = self.layout.channels;
        let mut rest = x;
        while !rest.is_empty() {
            let take = (c - self.frame.len()).min(rest.len());
            self.frame.extend(rest[..take].iter().map(|z| {
                if z.re.is_finite() && z.im.is_finite() {
                    *z
                } else {
                    Complex32::default()
                }
            }));
            rest = &rest[take..];
            if self.frame.len() < c {
                break;
            }
            let power = self.fft.run_power(&self.frame);
            self.frame.clear();
            for (k, a) in self.acc.iter_mut().enumerate() {
                *a += power[(k + c / 2) % c];
            }
            self.spectra += 1;
            if self.spectra == self.layout.spectra {
                let n = self.spectra as f32;
                for (r, a) in self.row.iter_mut().zip(self.acc.iter_mut()) {
                    *r = *a / n;
                    *a = 0.0;
                }
                self.spectra = 0;
                row(&self.row);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_of_the_defaults() {
        let l = Layout::new(2.4e6, 64, 0.5);
        assert_eq!((l.channels, l.spectra), (64, 19));
        assert!((l.row_s - 0.000_506_666).abs() < 1e-9);
        assert_eq!(Layout::new(2.4e6, 5, 0.5).channels, 8);
        assert_eq!(Layout::new(2.4e6, 100, 0.5).channels, 128);
        assert_eq!(Layout::new(1e6, 1024, 0.1).spectra, 1);
    }

    #[test]
    fn a_tone_lands_in_its_channel() {
        let l = Layout::new(2.4e6, 64, 0.5);
        let mut b = FilterBank::new(l);
        let f = 300e3;
        let x: Vec<Complex32> = (0..64 * 19 * 4)
            .map(|n| {
                let p = std::f64::consts::TAU * f * n as f64 / 2.4e6;
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect();
        let mut rows = Vec::new();
        b.push(&x, |r| rows.push(r.to_vec()));
        assert_eq!(rows.len(), 4);
        let freqs = l.freqs(0.0, 2.4e6);
        let top = (0..64)
            .max_by(|&a, &b| rows[0][a].total_cmp(&rows[0][b]))
            .unwrap();
        assert!((freqs[top] - f).abs() < 1.0, "{} {}", freqs[top], f);
    }
}
