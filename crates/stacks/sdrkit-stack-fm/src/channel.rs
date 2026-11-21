//! Channel selection: IQ at the input rate in, IQ of one FM channel at the channel rate (near 500 kHz) out.
//!
//! Bad samples (NaN, infinite) become zero; an oscillator moves the station to 0 Hz; the channel filter keeps it and decimates.
//! Settings: the shift (where the station is) and the channel width. Result: the channel level (true channel power).

use crate::frontend::ChannelFilter;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::Complex32;

/// Channel selection.
#[derive(Clone, Debug)]
pub struct ChannelSelect {
    rate: f64,
    nco: Nco,
    filter: ChannelFilter,
    buf: Vec<Complex32>,
    power: f64,
    n: u64,
}

impl ChannelSelect {
    /// For input at `rate`, a station at `shift_hz` from the input centre, a channel `width_hz` wide (0 = the standard 220 kHz).
    pub fn new(rate: f64, shift_hz: f64, width_hz: f64) -> Self {
        ChannelSelect {
            rate,
            nco: Nco::new(rate, -shift_hz),
            filter: ChannelFilter::new(rate, width_hz),
            buf: Vec::new(),
            power: 0.0,
            n: 0,
        }
    }

    /// The channel rate.
    pub fn output_rate(&self) -> f64 {
        self.filter.output_rate()
    }

    /// The channel width in use, Hz.
    pub fn width(&self) -> f64 {
        self.filter.width()
    }

    /// A new channel width; the filter starts again with empty history.
    pub fn set_width(&mut self, width_hz: f64) {
        self.filter = ChannelFilter::new(self.rate, width_hz);
    }

    /// Moves the station: it is now at `shift_hz` (the oscillator's phase carries on).
    pub fn set_shift(&mut self, shift_hz: f64) {
        self.nco.set_freq(-shift_hz);
    }

    /// Appends the channel samples for `iq` to `out`.
    pub fn process(&mut self, iq: &[Complex32], out: &mut Vec<Complex32>) {
        self.buf.clear();
        self.buf.extend(iq.iter().map(|&v| {
            if v.re.is_finite() && v.im.is_finite() {
                v
            } else {
                Complex32::default()
            }
        }));
        self.nco.mix_in_place(&mut self.buf);
        let start = out.len();
        self.filter.process(&self.buf, out);
        for v in &out[start..] {
            self.power += v.norm_sqr() as f64;
        }
        self.n += (out.len() - start) as u64;
    }

    /// The mean channel power since the last call, in dBFS (before any normalisation or equaliser).
    pub fn take_level_dbfs(&mut self) -> f32 {
        let db = if self.n > 0 {
            (10.0 * (self.power / self.n as f64).max(1e-20).log10()) as f32
        } else {
            -200.0
        };
        self.power = 0.0;
        self.n = 0;
        db
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_a_shifted_tone_and_reads_its_level() {
        let rate = 2e6;
        let x: Vec<Complex32> = (0..200_000)
            .map(|i| {
                let p = std::f64::consts::TAU * 300e3 * i as f64 / rate;
                Complex32::new(0.1 * p.cos() as f32, 0.1 * p.sin() as f32)
            })
            .collect();
        let mut c = ChannelSelect::new(rate, 300e3, 0.0);
        let mut out = Vec::new();
        c.process(&x, &mut out);
        assert_eq!(c.output_rate(), 500e3);
        assert_eq!(out.len(), 50_000);
        assert!((c.take_level_dbfs() + 20.0).abs() < 0.2);
        let mut far = ChannelSelect::new(rate, 0.0, 0.0);
        out.clear();
        far.process(&x, &mut out);
        assert!(far.take_level_dbfs() < -75.0);
        let mut bad = ChannelSelect::new(rate, 0.0, 0.0);
        out.clear();
        bad.process(&[Complex32::new(f32::NAN, 1.0); 100], &mut out);
        assert!(out.iter().all(|v| v.re.is_finite()));
    }
}
