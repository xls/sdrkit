//! The FM demodulator: the IQ of one channel in, the multiplex (MPX, 75 kHz deviation = 1.0) out at half the channel rate. It
//! holds the DC-spike removal, the equaliser and the multipath detector, the discriminators, the decimation by 2 and the MPX DC
//! removal, and returns one MPX stream for the sound and one for RDS and the measurements.

use crate::frontend::{self, Cma, MultipathDetector, CMA_MAIN};
use sdrkit_algo_dsp::dc::RealDcBlocker;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::fm::Discriminator;
use sdrkit_algo_dsp::DspError;
use sdrkit_core::Complex32;

/// The corner of the MPX DC removal: a one-pole high-pass with a time constant of 0.3 s.
const MPX_DC_HZ: f64 = 1.0 / (std::f64::consts::TAU * 0.3);
#[cfg(test)]
use std::f64::consts::TAU;

/// The two MPX streams one call produces.
#[derive(Clone, Debug, Default)]
pub struct Mpx {
    /// For the stereo decoder and the sound.
    pub sound: Vec<f32>,
    /// For RDS, the SNR measure and the MPX spectrum.
    pub meas: Vec<f32>,
}

impl Mpx {
    /// Empties both.
    pub fn clear(&mut self) {
        self.sound.clear();
        self.meas.clear();
    }
}

/// The demodulator's measurements over one report interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DemodReport {
    /// The envelope is constant enough for an FM carrier (coefficient of variation below 0.4).
    pub carrier: bool,
    /// The carrier offset within the channel, Hz (smoothed across reports).
    pub offset_hz: f64,
    /// Peak deviation, kHz (noise included).
    pub dev_khz: f32,
    /// Multipath ripple, %.
    pub multipath_pct: f32,
    /// The sound uses the equaliser.
    pub equaliser: bool,
}

/// The FM demodulator.
#[derive(Clone, Debug)]
pub struct FmDemod {
    fm: f64,
    dc_mean: Complex32,
    dc_a: f32,
    delay: [Complex32; CMA_MAIN],
    delay_i: usize,
    cma: Cma,
    disc_r: Discriminator,
    disc_e: Discriminator,
    delayed: Vec<Complex32>,
    equalised: Vec<Complex32>,
    disc_raw: Vec<f32>,
    disc_eq: Vec<f32>,
    mp: MultipathDetector,
    dec_raw: FirDecimator<f32>,
    dec_eq: FirDecimator<f32>,
    mpx_raw: Vec<f32>,
    mpx_eq: Vec<f32>,
    hp_sound: RealDcBlocker,
    hp_meas: RealDcBlocker,
    sound_eq: bool,
    meas_eq: bool,
    threshold_ext: bool,
    offset: Option<f64>,
    snr: f32,
    env: f64,
    env2: f64,
    n: u64,
    disc: f64,
    peak: f32,
}

/// The stop edge of the channel filter, Hz.
const STOP_HZ: f64 = 138e3;
/// The lowest channel rate, samples a second: twice the stop edge of the channel filter.
pub const MIN_RATE_HZ: f64 = 2.0 * STOP_HZ;

impl FmDemod {
    /// A demodulator for channel samples at `fo`. `decimation` is the input decimation that produced `fo` (for the droop
    /// correction). Errors when `fo` is below [`MIN_RATE_HZ`] or not finite.
    pub fn new(fo: f64, decimation: usize) -> Result<Self, DspError> {
        let taps = sdrkit_algo_dsp::fm::droop_compensated(
            &sdrkit_algo_dsp::fir::lowpass(fo, 112e3, STOP_HZ, 62.0)?,
            decimation,
        );
        let fm = fo / 2.0;
        Ok(FmDemod {
            fm,
            dc_mean: Complex32::default(),
            dc_a: (1.0 - (-1.0 / (0.5 * fo)).exp()) as f32,
            delay: [Complex32::default(); CMA_MAIN],
            delay_i: 0,
            cma: Cma::new(),
            disc_r: Discriminator::new(fo, 75e3)?,
            disc_e: Discriminator::new(fo, 75e3)?,
            delayed: Vec::new(),
            equalised: Vec::new(),
            disc_raw: Vec::new(),
            disc_eq: Vec::new(),
            mp: MultipathDetector::new(fo),
            dec_raw: FirDecimator::new(&taps, 2),
            dec_eq: FirDecimator::new(&taps, 2),
            mpx_raw: Vec::new(),
            mpx_eq: Vec::new(),
            hp_sound: RealDcBlocker::new(fm, MPX_DC_HZ),
            hp_meas: RealDcBlocker::new(fm, MPX_DC_HZ),
            sound_eq: true,
            meas_eq: true,
            threshold_ext: false,
            offset: None,
            snr: 0.0,
            env: 0.0,
            env2: 0.0,
            n: 0,
            disc: 0.0,
            peak: 0.0,
        })
    }

    /// For the channel of a [`crate::channel::ChannelSelect`] at input rate `input_rate`. Errors as [`FmDemod::new`].
    pub fn for_input(input_rate: f64) -> Result<Self, DspError> {
        let d = frontend::decimation(input_rate);
        Self::new(input_rate / d as f64, d)
    }

    /// The MPX rate.
    pub fn output_rate(&self) -> f64 {
        self.fm
    }

    /// Demodulates channel samples, appending MPX samples to `out`.
    pub fn process(&mut self, ch: &[Complex32], out: &mut Mpx) {
        let subtract = self.offset.is_some_and(|o| o.abs() > 50.0);
        // The equaliser and its path run only while the sound or the measurements use them (chosen at each report).
        let need_eq = self.sound_eq || self.meas_eq;
        // The DC, the envelope, the delay that aligns the raw path with the equaliser's output, and the equaliser.
        self.delayed.clear();
        self.equalised.clear();
        for &x in ch {
            self.dc_mean += (x - self.dc_mean) * self.dc_a;
            let x = if subtract { x - self.dc_mean } else { x };
            let p = x.norm_sqr();
            // Summed sample by sample, so the sums do not depend on how the stream is cut.
            self.env += p.sqrt() as f64;
            self.env2 += p as f64;
            self.delayed.push(self.delay[self.delay_i]);
            self.delay[self.delay_i] = x;
            self.delay_i = if self.delay_i + 1 == CMA_MAIN {
                0
            } else {
                self.delay_i + 1
            };
            if need_eq {
                self.equalised.push(self.cma.process(x));
            } else {
                self.cma.observe(x);
            }
        }
        self.n += ch.len() as u64;
        // Both discriminators, then the multipath detector on the raw one.
        self.disc_raw.clear();
        self.disc_eq.clear();
        self.disc_r.process(&self.delayed, &mut self.disc_raw);
        if need_eq {
            self.disc_e.process(&self.equalised, &mut self.disc_eq);
        }
        for (&fr, d) in self.disc_raw.iter().zip(&self.delayed) {
            self.disc += fr as f64;
            self.mp.push(fr * 75e3, d.norm_sqr(), self.snr);
        }
        self.mpx_raw.clear();
        self.mpx_eq.clear();
        self.dec_raw.process(&self.disc_raw, &mut self.mpx_raw);
        if need_eq {
            self.dec_eq.process(&self.disc_eq, &mut self.mpx_eq);
        }
        let (s0, m0) = (out.sound.len(), out.meas.len());
        let pick = |eq: bool| if eq { &self.mpx_eq } else { &self.mpx_raw };
        out.sound.extend_from_slice(pick(self.sound_eq));
        out.meas.extend_from_slice(pick(self.meas_eq));
        self.hp_sound.process(&mut out.sound[s0..]);
        self.hp_meas.process(&mut out.meas[m0..]);
        self.peak = out.meas[m0..].iter().fold(self.peak, |p, m| p.max(m.abs()));
    }

    /// Ends a report interval: the carrier, the offset and the deviation since the last call.
    pub fn measure(&mut self) -> DemodReport {
        let carrier = if self.n > 0 && self.env > 0.0 {
            let m1 = self.env / self.n as f64;
            let m2 = self.env2 / self.n as f64;
            (m2 / (m1 * m1) - 1.0).max(0.0).sqrt() < 0.4
        } else {
            false
        };
        if self.n > 0 {
            let o = self.disc / self.n as f64 * 75e3;
            self.offset = Some(match self.offset {
                Some(p) => p + 0.3 * (o - p),
                None => o,
            });
        }
        let r = DemodReport {
            carrier,
            offset_hz: self.offset.unwrap_or(0.0),
            dev_khz: self.peak * 75.0,
            multipath_pct: self.mp.ripple_pct,
            equaliser: self.sound_eq,
        };
        self.env = 0.0;
        self.env2 = 0.0;
        self.n = 0;
        self.disc = 0.0;
        self.peak = 0.0;
        r
    }

    /// The latest SNR measure: chooses the paths for what follows (the equaliser for multipath, and for the sound below 22 dB until
    /// above 24 dB).
    pub fn set_snr(&mut self, snr_db: f32) {
        self.snr = snr_db;
        if snr_db < 22.0 {
            self.threshold_ext = true;
        } else if snr_db > 24.0 {
            self.threshold_ext = false;
        }
        let was = self.sound_eq || self.meas_eq;
        self.meas_eq = self.mp.detected;
        self.sound_eq = self.mp.detected || self.threshold_ext;
        if !was && (self.sound_eq || self.meas_eq) {
            // The equaliser path starts again from the raw path's state: the identity outputs the raw signal, so the switch is
            // seamless, and the equaliser then adapts.
            self.cma.identity();
            self.disc_e.set_last(self.disc_r.last());
            self.dec_eq.copy_state_from(&self.dec_raw);
        }
    }

    /// Whether the sound uses the equaliser.
    pub fn equaliser(&self) -> bool {
        self.sound_eq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tone_comes_out_at_its_deviation_and_offset_reads() {
        let fo = 500e3;
        let mut d = FmDemod::new(fo, 1).unwrap();
        // A carrier 2 kHz up, modulated by a 1 kHz tone at 37.5 kHz deviation (0.5).
        let mut ph = 0.0f64;
        let x: Vec<Complex32> = (0..200_000)
            .map(|i| {
                let f = 2000.0 + 37.5e3 * (TAU * 1000.0 * i as f64 / fo).sin();
                ph += TAU * f / fo;
                Complex32::new(0.3 * ph.cos() as f32, 0.3 * ph.sin() as f32)
            })
            .collect();
        let mut out = Mpx::default();
        d.process(&x, &mut out);
        let r = d.measure();
        assert!(r.carrier);
        assert!((r.offset_hz - 2000.0).abs() < 20.0, "{}", r.offset_hz);
        assert_eq!(out.sound.len(), 100_000);
        let a = sdrkit_dev_test::measure::tone_amplitude(&out.meas[50_000..], 250e3, 1000.0);
        assert!((a - 0.5).abs() < 0.01, "{a}");
        assert!((r.dev_khz - 37.5).abs() < 3.0, "{}", r.dev_khz);
    }
}
