//! The channelizer: one signal out of a wide input, moved to DC and brought down to a low rate.

use crate::decim::Decimator;
use crate::nco::Nco;
use crate::resample::FractionalResampler;
use crate::{Complex32, DspError, Param};

/// What a [`Channelizer`] extracts.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelConfig {
    /// The input sample rate.
    pub input_rate: f64,
    /// Where the signal is, in Hz from the centre of the input (the signal's offset, not the shift).
    pub offset_hz: f64,
    /// The two-sided width kept flat and alias-free: `-bandwidth/2 .. +bandwidth/2` around the signal.
    pub bandwidth_hz: f64,
    /// The output rate must be at least this.
    pub min_rate_hz: f64,
    /// Resample to exactly `min_rate_hz` (adds a fractional stage when `input_rate / min_rate_hz` is not an integer).
    pub exact_rate: bool,
    /// Stopband attenuation of the filters.
    pub atten_db: f64,
}

impl ChannelConfig {
    /// A channel of `bandwidth_hz` at `offset_hz`, at an output rate of `input_rate / D >= min_rate_hz`, 80 dB.
    pub fn new(input_rate: f64, offset_hz: f64, bandwidth_hz: f64, min_rate_hz: f64) -> Self {
        ChannelConfig {
            input_rate,
            offset_hz,
            bandwidth_hz,
            min_rate_hz,
            exact_rate: false,
            atten_db: 80.0,
        }
    }
}

/// Moves a signal at an offset to DC (with an [`Nco`]) and decimates it (with a multi-stage [`Decimator`], plus a
/// [`FractionalResampler`] when an exact rate is requested).
///
/// The output rate is `input_rate / D` with the largest integer `D` that keeps the rate at or above both `min_rate_hz` and
/// `1.25 * bandwidth_hz` (so the passband fits within 0.4 of the output rate). `-bandwidth/2 .. +bandwidth/2` is flat and alias-free;
/// beyond it, up to the output Nyquist, the output still holds the filters' transition band, so a stack that needs sharp selectivity
/// applies its own channel filter at the (cheap) output rate.
#[derive(Clone, Debug)]
pub struct Channelizer {
    cfg: ChannelConfig,
    nco: Nco,
    decim: Decimator<Complex32>,
    frac: Option<FractionalResampler<Complex32>>,
    mixed: Vec<Complex32>,
    mid: Vec<Complex32>,
    output_rate: f64,
}

impl Channelizer {
    /// Builds the chain.
    pub fn new(cfg: ChannelConfig) -> Result<Self, DspError> {
        Self::build(cfg, None)
    }

    /// Builds the chain so that every input component at least `stop_hz` away from the signal is attenuated by at least
    /// `cfg.atten_db`, wherever it lands in the output. [`Channelizer::new`] keeps only the passband free of aliases, so a strong
    /// signal outside the channel can fold into the output's transition band. Use this form when such a signal is near (for
    /// example a strong carrier just outside the channel).
    ///
    /// `stop_hz` must be above `bandwidth_hz / 2` and at most `output_rate - bandwidth_hz / 2`. Half the output rate makes the
    /// whole output band clean. Errors as [`Channelizer::new`] does, and for a `stop_hz` outside that range.
    ///
    /// ```
    /// use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
    /// let mut cfg = ChannelConfig::new(2e6, 0.0, 100e3, 153.6e3);
    /// cfg.exact_rate = true;
    /// let ch = Channelizer::with_stop(cfg, 76.8e3).unwrap();
    /// assert_eq!(ch.output_rate(), 153.6e3);
    /// ```
    pub fn with_stop(cfg: ChannelConfig, stop_hz: f64) -> Result<Self, DspError> {
        if !(stop_hz > 0.0 && stop_hz.is_finite()) {
            return Param::Stop.positive(stop_hz);
        }
        Self::build(cfg, Some(stop_hz))
    }

    fn build(cfg: ChannelConfig, stop: Option<f64>) -> Result<Self, DspError> {
        let ChannelConfig {
            input_rate,
            bandwidth_hz,
            min_rate_hz,
            exact_rate,
            atten_db,
            ..
        } = cfg;
        if !(input_rate > 0.0 && input_rate.is_finite()) {
            return Param::InputRate.positive(input_rate);
        }
        if !(bandwidth_hz > 0.0 && bandwidth_hz <= input_rate) {
            return Param::Bandwidth.range(0.0, input_rate, bandwidth_hz);
        }
        if min_rate_hz.is_nan() || min_rate_hz <= 0.0 {
            return Param::MinRate.positive(min_rate_hz);
        }
        let pass = bandwidth_hz / 2.0;
        // The decimator's alias-free edge for a stop band at `stop` from the signal: at its output rate `rate_d`, a component at
        // `stop` lands at `stop - rate_d`, so aliases must stay out up to `rate_d - stop`.
        let alias_free = |rate_d: f64, pass_d: f64, out: f64| -> Result<f64, DspError> {
            match stop {
                None => Ok(pass_d),
                Some(s) if s > pass && s <= out - pass => Ok(rate_d - s),
                Some(s) => Param::Stop.range(pass, out - pass, s),
            }
        };
        let need = min_rate_hz.max(bandwidth_hz * 1.25);
        let (decim, frac, output_rate) = if exact_rate {
            let out = min_rate_hz;
            if bandwidth_hz >= 0.9 * out {
                return Param::Bandwidth.range(0.0, 0.9 * out, bandwidth_hz);
            }
            let r = input_rate / out;
            let d = if (r - r.round()).abs() < 1e-12 * r && r.round() >= 1.0 {
                r.round() as usize
            } else {
                (r.floor() as usize).max(1)
            };
            let mid = input_rate / d as f64;
            let pass_d = pass.min(mid * 0.45);
            let af = alias_free(mid, pass_d, out)?;
            let decim = Decimator::with_alias_free(input_rate, d, pass_d, af, atten_db);
            let frac = if (mid - out).abs() > 1e-9 * out {
                let low = mid.min(out);
                Some(FractionalResampler::new(
                    mid,
                    out,
                    pass,
                    low - pass,
                    atten_db,
                )?)
            } else {
                None
            };
            (decim, frac, out)
        } else {
            let d = ((input_rate / need).floor() as usize).max(1);
            let out = input_rate / d as f64;
            let pass_d = pass.min(out * 0.45);
            let af = alias_free(out, pass_d, out)?;
            (
                Decimator::with_alias_free(input_rate, d, pass_d, af, atten_db),
                None,
                out,
            )
        };
        Ok(Channelizer {
            nco: Nco::new(input_rate, -cfg.offset_hz),
            cfg,
            decim,
            frac,
            mixed: Vec::new(),
            mid: Vec::new(),
            output_rate,
        })
    }

    /// The delay of the filters, in input samples: output sample `m` holds the signal as it was at input sample
    /// `m * input_rate / output_rate - delay()`. The mixer adds no delay.
    pub fn delay(&self) -> f64 {
        // A decimation by `d` produces its output `m` at input `m d + d - 1`, so that much of the filter delay is already counted.
        let d = self.decim.factor() as f64;
        self.decim.delay() - (d - 1.0) + self.frac.as_ref().map_or(0.0, |f| f.delay() * d)
    }

    /// The configuration (with the current offset).
    pub fn config(&self) -> &ChannelConfig {
        &self.cfg
    }

    /// The output rate.
    pub fn output_rate(&self) -> f64 {
        self.output_rate
    }

    /// The integer decimation.
    pub fn decimation(&self) -> usize {
        self.decim.factor()
    }

    /// Retunes live: the signal is now at `offset_hz`. The mixer's phase is continuous and the filters keep their state.
    pub fn set_offset(&mut self, offset_hz: f64) {
        self.cfg.offset_hz = offset_hz;
        self.nco.set_freq(-offset_hz);
    }

    /// The signal offset.
    pub fn offset(&self) -> f64 {
        self.cfg.offset_hz
    }

    /// Clock-drift correction on the fractional stage (only with `exact_rate`; see [`FractionalResampler::set_correction`]).
    pub fn set_correction(&mut self, factor: f64) {
        if let Some(f) = &mut self.frac {
            f.set_correction(factor);
        }
    }

    /// Clears the filters (after a retune of the radio).
    pub fn reset(&mut self) {
        self.decim.reset();
        if let Some(f) = &mut self.frac {
            f.reset();
        }
    }

    /// Appends the channel's samples for `input` to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        self.mixed.resize(input.len(), Complex32::default());
        self.nco.mix(input, &mut self.mixed);
        match &mut self.frac {
            None => self.decim.process(&self.mixed, out),
            Some(f) => {
                self.mid.clear();
                self.mid.reserve(input.len() / self.decim.factor() + 1);
                self.decim.process(&self.mixed, &mut self.mid);
                f.process(&self.mid, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn moves_a_tone_at_an_offset_to_dc() {
        let rate = 2.4e6;
        let mut ch =
            Channelizer::new(ChannelConfig::new(rate, 300_000.0, 20_000.0, 48_000.0)).unwrap();
        assert_eq!(ch.output_rate(), 48_000.0);
        assert_eq!(ch.decimation(), 50);
        let mut x = ctone(240_000, 301_000.0 / rate, 0.5);
        // An interferer at the mirror image and one next door.
        for (v, w) in x.iter_mut().zip(ctone(240_000, -300_000.0 / rate, 0.5)) {
            *v += w;
        }
        for (v, w) in x.iter_mut().zip(ctone(240_000, 340_000.0 / rate, 0.5)) {
            *v += w;
        }
        let mut out = Vec::new();
        for c in x.chunks(10_000) {
            ch.process(c, &mut out);
        }
        assert_eq!(out.len(), 4800);
        let tail = &out[1200..];
        assert!(db(camp(tail, 1000.0 / 48_000.0) / 0.5).abs() < 0.02);
        // 40 kHz above the channel lands on -8 kHz at 48 kHz: suppressed.
        assert!(db(camp(tail, -8_000.0 / 48_000.0)) < -80.0);

        // Retune to the interferer at -300 kHz: now it is at DC.
        ch.set_offset(-300_000.0);
        out.clear();
        ch.process(&x, &mut out);
        assert!(db(camp(&out[1200..], 0.0) / 0.5).abs() < 0.02);
    }

    #[test]
    fn rate_choice() {
        let ch = Channelizer::new(ChannelConfig::new(2.048e6, 0.0, 200_000.0, 200_000.0)).unwrap();
        assert_eq!(ch.output_rate(), 256_000.0);
        let ch = Channelizer::new(ChannelConfig::new(1e6, 0.0, 10_000.0, 2e6)).unwrap();
        assert_eq!(ch.output_rate(), 1e6);
        let mut cfg = ChannelConfig::new(2.4e6, 100_000.0, 12_000.0, 44_100.0);
        cfg.exact_rate = true;
        let mut ch = Channelizer::new(cfg).unwrap();
        assert_eq!(ch.output_rate(), 44_100.0);
        let x = ctone(240_000, 102_000.0 / 2.4e6, 1.0);
        let mut out = Vec::new();
        ch.process(&x, &mut out);
        assert!((out.len() as f64 - 4410.0).abs() <= 1.0);
        assert!(db(camp(&out[1000..], 2000.0 / 44_100.0)).abs() < 0.05);
        assert!(Channelizer::new(ChannelConfig::new(0.0, 0.0, 1.0, 1.0)).is_err());
    }

    /// Measures the delay from the phase of a slow tone through the chain: out[m] = x(m * in / out - delay).
    fn measured_delay(ch: &mut Channelizer, rate: f64) -> f64 {
        let f0 = 1000.0;
        let x = ctone(400_000, f0 / rate, 0.5);
        let mut out = Vec::new();
        ch.process(&x, &mut out);
        let step = rate / ch.output_rate();
        let mut sum = 0.0;
        let mut n = 0.0;
        let from = out.len() / 2;
        for (m, y) in out.iter().enumerate().take(out.len() - 10).skip(from) {
            let want = std::f64::consts::TAU * f0 / rate * (m as f64 * step);
            let got = (y.im as f64).atan2(y.re as f64);
            // Phase lag in input samples.
            let lag = ((want - got + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI)
                / (std::f64::consts::TAU * f0 / rate);
            sum += lag;
            n += 1.0;
        }
        sum / n
    }

    #[test]
    fn the_delay_is_what_the_filters_say() {
        let rate = 2e6;
        let mut ch = Channelizer::new(ChannelConfig::new(rate, 0.0, 80e3, 100e3)).unwrap();
        let d = measured_delay(&mut ch, rate);
        assert!(
            (d - ch.delay()).abs() < 0.5,
            "measured {d}, said {}",
            ch.delay()
        );
        let mut cfg = ChannelConfig::new(rate, 0.0, 80e3, 153.6e3);
        cfg.exact_rate = true;
        let mut ch = Channelizer::new(cfg).unwrap();
        let d = measured_delay(&mut ch, rate);
        assert!(
            (d - ch.delay()).abs() < 0.5,
            "measured {d}, said {}",
            ch.delay()
        );
    }

    #[test]
    fn the_stop_band_keeps_a_strong_neighbour_out() {
        // A strong carrier 100 kHz away from an 80 kHz channel at exactly 153.6 kHz, from 2 Msps: the decimation by 13 folds it
        // to -53.8 kHz, inside the transition band that `new` leaves open. With the stop band at half the output rate it is gone.
        let rate = 2e6;
        let mut cfg = ChannelConfig::new(rate, 0.0, 80e3, 153.6e3);
        cfg.exact_rate = true;
        let mut x = ctone(400_000, 10e3 / rate, 1e-3);
        for (v, w) in x.iter_mut().zip(ctone(400_000, 100e3 / rate, 1.0)) {
            *v += w;
        }
        let alias = 100e3 - rate / 13.0;
        let level = |ch: &mut Channelizer| {
            let mut out = Vec::new();
            ch.process(&x, &mut out);
            let tail = &out[out.len() / 2..];
            (
                db(camp(tail, 10e3 / 153.6e3) / 1e-3),
                db(camp(tail, alias / 153.6e3)),
            )
        };
        let (_, leak) = level(&mut Channelizer::new(cfg.clone()).unwrap());
        let mut clean = Channelizer::with_stop(cfg.clone(), 76.8e3).unwrap();
        let (wanted, kept_out) = level(&mut clean);
        assert!(wanted.abs() < 0.05, "wanted tone {wanted} dB");
        assert!(
            kept_out < -80.0,
            "neighbour at {kept_out} dB (without the stop band {leak} dB)"
        );
        assert!(leak > -40.0, "{leak}");
        // The same on the integer path, and values out of range.
        let mut ch =
            Channelizer::with_stop(ChannelConfig::new(rate, 0.0, 80e3, 150e3), 70e3).unwrap();
        let mut out = Vec::new();
        ch.process(&x, &mut out);
        let r = ch.output_rate();
        let fold = 100e3 - r * (100e3 / r).round();
        assert!(db(camp(&out[out.len() / 2..], fold / r)) < -80.0);
        assert!(Channelizer::with_stop(cfg.clone(), 40e3).is_err());
        assert!(Channelizer::with_stop(cfg.clone(), 120e3).is_err());
        assert!(Channelizer::with_stop(cfg, f64::NAN).is_err());
    }
}
