//! The front end of a PSK receiver: bad samples to 0, blind IQ correction, the mixer, the anti-alias decimation, the input level,
//! and the band locator that moves the mixer onto the signal in a wide capture.

use super::BandLocator;
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::dc::{DcEstimator, IqImbalance};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::nco::Nco;

/// The decimating front end of a PSK receiver.
pub struct PskFrontEnd {
    rate: f64,
    factor: usize,
    nco: Nco,
    decim: Option<FirDecimator<Complex32>>,
    mixer_hz: f64,
    user_offset_hz: f64,
    located_hz: f64,
    locate: Option<BandLocator>,
    scratch: Vec<Complex32>,
    power_sum: f64,
    power_n: u64,
    peak: f32,
    iq: Option<(DcEstimator, IqImbalance)>,
}

impl PskFrontEnd {
    /// A front end for input at `rate` with the signal at `offset_hz` from the centre. `decimation` sets the anti-alias taps and
    /// the whole factor. `locate` moves the mixer onto the strongest band once. `blind_iq` turns on the DC and IQ correction.
    /// Errors for a rate that is not positive, a factor of 0 or no taps.
    pub fn new(
        rate: f64,
        offset_hz: f64,
        decimation: Option<(&[f32], usize)>,
        locate: Option<BandLocator>,
        blind_iq: bool,
    ) -> Result<Self, ModemError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Param::Rate.positive(rate);
        }
        let (decim, factor) = match decimation {
            Some((taps, f)) if f >= 1 && !taps.is_empty() => {
                ((f > 1).then(|| FirDecimator::new(taps, f)), f)
            }
            Some(_) => return Param::Decimation.degenerate(),
            None => (None, 1),
        };
        Ok(PskFrontEnd {
            rate,
            factor,
            nco: Nco::new(rate, -offset_hz),
            decim,
            mixer_hz: offset_hz,
            user_offset_hz: offset_hz,
            located_hz: 0.0,
            locate,
            scratch: Vec::new(),
            power_sum: 0.0,
            power_n: 0,
            peak: 0.0,
            iq: blind_iq.then(|| (DcEstimator::new(), IqImbalance::new())),
        })
    }

    /// The output rate, Hz.
    pub fn output_rate(&self) -> f64 {
        self.rate / self.factor as f64
    }

    /// The decimation factor.
    pub fn factor(&self) -> usize {
        self.factor
    }

    /// The band locator's move, Hz (0 before it decides or when it does not move).
    pub fn located_hz(&self) -> f64 {
        self.located_hz
    }

    /// Moves the mixer: the signal is now expected `extra_hz` from the given offset.
    pub fn set_extra(&mut self, extra_hz: f64) {
        self.mixer_hz = self.user_offset_hz + extra_hz;
        self.nco.set_freq(-self.mixer_hz);
    }

    /// The total mixer offset from the centre, Hz.
    pub fn mixer_hz(&self) -> f64 {
        self.mixer_hz
    }

    /// Starts the band location again, skipping where it looked before.
    pub fn relocate(&mut self) {
        if let Some(l) = &mut self.locate {
            l.restart(self.located_hz);
            self.located_hz = 0.0;
            self.set_extra(0.0);
        }
    }

    /// The mean input power since the last call (dBFS) and whether any rail came near full scale (above 0.95).
    pub fn take_level(&mut self) -> (f32, bool) {
        let p = if self.power_n > 0 {
            self.power_sum / self.power_n as f64
        } else {
            0.0
        };
        let over = self.peak > 0.95;
        self.power_sum = 0.0;
        self.power_n = 0;
        self.peak = 0.0;
        ((10.0 * p.max(1e-20).log10()) as f32, over)
    }

    /// Runs `input` and appends the output to `out`. A move of the band locator acts from the sample after its average.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        self.scratch.clear();
        self.scratch.extend_from_slice(input);
        let mut sum = 0.0f64;
        let mut peak = self.peak;
        // The power of a piece is summed in single precision: 64 samples lose nothing that the level shows.
        for piece in self.scratch.chunks_mut(64) {
            let mut power = 0.0f32;
            for s in piece.iter_mut() {
                if !s.re.is_finite() || !s.im.is_finite() {
                    *s = Complex32::default();
                }
                power += s.norm_sqr();
                peak = peak.max(s.re.abs()).max(s.im.abs());
            }
            sum += power as f64;
        }
        self.peak = peak;
        self.power_sum += sum;
        self.power_n += input.len() as u64;
        if let Some((dc, iq)) = &mut self.iq {
            iq.process(dc, &mut self.scratch, false);
        }
        let mut split = None;
        if let Some(l) = &mut self.locate {
            if let Some((at, hz)) = l.push(&self.scratch) {
                split = Some(at);
                self.located_hz = hz.unwrap_or(0.0);
            }
        }
        match split {
            Some(at) => {
                let (a, b) = self.scratch.split_at_mut(at);
                self.nco.mix_in_place(a);
                self.mixer_hz = self.user_offset_hz + self.located_hz;
                self.nco.set_freq(-self.mixer_hz);
                self.nco.mix_in_place(b);
            }
            // A mixer at 0 Hz that never turned multiplies by 1.
            None if self.nco.freq() == 0.0 && self.nco.phase_cycles() == 0.0 => {}
            None => self.nco.mix_in_place(&mut self.scratch),
        }
        match &mut self.decim {
            Some(d) => d.process(&self.scratch, out),
            None => out.extend_from_slice(&self.scratch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_decimates_and_measures() {
        // A tone at the offset comes out at DC, at a third of the rate.
        let rate = 48e3;
        let taps = sdrkit_algo_dsp::fir::lowpass(rate, 4e3, 8e3, 60.0).unwrap();
        let mut fe = PskFrontEnd::new(rate, 5e3, Some((&taps, 3)), None, false).unwrap();
        assert_eq!(fe.output_rate(), 16e3);
        let x: Vec<Complex32> = (0..4800)
            .map(|n| {
                Complex32::from_polar(0.5, (std::f64::consts::TAU * 5e3 * n as f64 / rate) as f32)
            })
            .collect();
        let mut out = Vec::new();
        fe.process(&x, &mut out);
        assert_eq!(out.len(), 1600);
        assert!((out[1000] - Complex32::new(0.5, 0.0)).norm() < 1e-2);
        let (db, over) = fe.take_level();
        assert!((db - 20.0 * 0.5f32.log10()).abs() < 0.01 && !over);
        assert!(PskFrontEnd::new(rate, 0.0, Some((&taps, 0)), None, false).is_err());
    }
}
