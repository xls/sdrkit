//! The front end of the weak-signal stacks: the channel at 24 kHz, then the shared analogue demodulator to 8 kHz audio with its
//! slow AGC, and the level measurements.

use num_complex::Complex32;
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_modem::analog::{
    AgcConfig, AnalogConfig, AnalogDemod, AnalogMode, AudioFilter, BandPass, Gain, LowPass,
};
use sdrkit_core::ConfigError;

/// The audio rate of the decoders.
pub const AUDIO_RATE: f64 = 8000.0;
/// The channel rate.
pub const CHANNEL_RATE: f64 = 24000.0;
/// The lowest input rate.
pub const MIN_RATE: f64 = 8000.0;

sdrkit_core::choice_enum! {
    /// The demodulator of the front end.
    pub enum Demod {
        /// Upper sideband, the usual choice.
        Auto = ("auto", "Auto"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// Narrow FM.
        Nfm = ("nfm", "NFM"),
        /// AM.
        Am = ("am", "AM"),
    }
}

/// Measurements of the front end since they were last taken.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrontLevels {
    /// Mean power of the input, dBFS (-200: nothing measured).
    pub level_dbfs: f32,
    /// Power of the sideband before the AGC, dBFS.
    pub audio_dbfs: f32,
}

/// The front end: complex samples at any rate in, 8 kHz audio out.
pub struct FrontEnd {
    demod: Demod,
    plain: bool,
    rate: f64,
    offset_hz: f64,
    chan: Option<Channelizer>,
    up: Option<(Nco, Resampler<Complex32>)>,
    audio: AnalogDemod,
    ch: Vec<Complex32>,
    shifted: Vec<Complex32>,
    power: Vec<f32>,
    p_in: f64,
    n_in: u64,
    p_audio: f64,
    n_audio: u64,
}

/// The demodulator at the channel rate: 24 kHz in, 8 kHz out. USB and LSB cover 200 to 3800 Hz. NFM and AM take a
/// channel low-pass (6.25 or 3.5 kHz), the detector, then an audio band-pass of 300 to 3000 Hz. Then comes the slow AGC (attack
/// 0.01, decay 0.0001 per audio sample, the envelope held at 0.5) and a clamp at plus and minus 8.
fn demodulator(demod: Demod, plain: bool) -> Result<AnalogDemod, ConfigError> {
    let mode = match demod {
        Demod::Auto | Demod::Usb => AnalogMode::Usb,
        Demod::Lsb => AnalogMode::Lsb,
        Demod::Nfm => AnalogMode::Nfm,
        Demod::Am => AnalogMode::Am,
    };
    let mut c = AnalogConfig::new(mode, CHANNEL_RATE);
    c.decimation = 3;
    if mode.is_sideband() {
        c.sideband = Some(BandPass {
            low_hz: 200.0,
            high_hz: 3800.0,
            transition_hz: 270.0,
            atten_db: 60.0,
        });
    } else {
        let w = if mode == AnalogMode::Nfm {
            6250.0
        } else {
            3500.0
        };
        c.channel = Some(LowPass::Edges {
            pass_hz: w,
            stop_hz: w + 1000.0,
            atten_db: 60.0,
        });
        c.audio = Some(AudioFilter::BandPass(BandPass {
            low_hz: 300.0,
            high_hz: 3000.0,
            transition_hz: 270.0,
            atten_db: 60.0,
        }));
        // The AM carrier average: 0.0005 per channel sample.
        c.am_carrier_s = -1.0 / (CHANNEL_RATE * 0.9995f64.ln());
    }
    if plain {
        c.gain = Gain::Manual(1.0);
    } else {
        c.gain = Gain::Auto(AgcConfig::from_factors(AUDIO_RATE, 0.5, 0.01, 0.0001, 1e6));
        c.clamp = Some(8.0);
    }
    Ok(AnalogDemod::new(c)?)
}

impl FrontEnd {
    /// A front end for input at `rate`, the dial at `offset_hz` from the centre, with this demodulator.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`MIN_RATE`], and the errors of the channel filter, the resampler and the demodulator.
    pub fn new(rate: f64, offset_hz: f64, demod: Demod) -> Result<FrontEnd, ConfigError> {
        Self::build(rate, offset_hz, demod, false)
    }

    /// The same front end without the slow AGC and the clamp: the audio keeps the level of the input (the burst and ping
    /// receivers measure their own noise level).
    ///
    /// # Errors
    ///
    /// As [`FrontEnd::new`].
    pub fn plain(rate: f64, offset_hz: f64, demod: Demod) -> Result<FrontEnd, ConfigError> {
        Self::build(rate, offset_hz, demod, true)
    }

    fn build(
        rate: f64,
        offset_hz: f64,
        demod: Demod,
        plain: bool,
    ) -> Result<FrontEnd, ConfigError> {
        if rate.is_nan() || rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: rate,
            });
        }
        let (chan, up) = if rate >= CHANNEL_RATE {
            let mut cfg = ChannelConfig::new(rate, offset_hz, 19_000.0, CHANNEL_RATE);
            cfg.exact_rate = true;
            (Some(Channelizer::new(cfg)?), None)
        } else {
            let nco = Nco::new(rate, -offset_hz);
            let r = Resampler::new(rate, CHANNEL_RATE)?;
            (None, Some((nco, r)))
        };
        Ok(FrontEnd {
            demod,
            plain,
            rate,
            offset_hz,
            chan,
            up,
            audio: demodulator(demod, plain)?,
            ch: Vec::new(),
            shifted: Vec::new(),
            power: Vec::new(),
            p_in: 0.0,
            n_in: 0,
            p_audio: 0.0,
            n_audio: 0,
        })
    }

    /// The input rate.
    pub fn rate(&self) -> f64 {
        self.rate
    }
    /// The dial's offset from the centre.
    pub fn offset_hz(&self) -> f64 {
        self.offset_hz
    }
    /// The demodulator.
    pub fn demod(&self) -> Demod {
        self.demod
    }
    /// True without the AGC ([`FrontEnd::plain`]).
    pub fn is_plain(&self) -> bool {
        self.plain
    }

    /// Processes `iq`, appending audio at 8 kHz to `out` (after the AGC, about -1 to 1; at the input's level without it).
    pub fn process(&mut self, iq: &[Complex32], out: &mut Vec<f32>) {
        // Bad samples become 0 before any filter.
        self.shifted.clear();
        for &x in iq {
            let x = if x.re.is_finite() && x.im.is_finite() {
                x
            } else {
                Complex32::default()
            };
            self.p_in += x.norm_sqr() as f64;
            self.shifted.push(x);
        }
        self.n_in += iq.len() as u64;
        self.ch.clear();
        if let Some(c) = self.chan.as_mut() {
            c.process(&self.shifted, &mut self.ch);
        } else if let Some((nco, r)) = self.up.as_mut() {
            nco.mix_in_place(&mut self.shifted);
            r.process(&self.shifted, &mut self.ch);
        }
        self.power.clear();
        self.audio.process(&self.ch, out, Some(&mut self.power));
        self.p_audio += self.power.iter().map(|&p| p as f64).sum::<f64>();
        self.n_audio += self.power.len() as u64;
    }

    /// The levels since the last call.
    pub fn take_levels(&mut self) -> FrontLevels {
        let db = |p: f64, n: u64| {
            if n == 0 || p <= 0.0 {
                -200.0
            } else {
                (10.0 * (p / n as f64).log10()) as f32
            }
        };
        let l = FrontLevels {
            level_dbfs: db(self.p_in, self.n_in),
            audio_dbfs: db(self.p_audio, self.n_audio),
        };
        self.p_in = 0.0;
        self.n_in = 0;
        self.p_audio = 0.0;
        self.n_audio = 0;
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_through(rate: f64, offset: f64, f_audio: f64, demod: Demod) -> (Vec<f32>, f64) {
        let mut fe = FrontEnd::new(rate, offset, demod).unwrap();
        let n = (rate * 2.0) as usize;
        let iq: Vec<Complex32> = (0..n)
            .map(|i| {
                let p = 2.0 * std::f64::consts::PI * (offset + f_audio) * i as f64 / rate;
                Complex32::new(0.1 * p.cos() as f32, 0.1 * p.sin() as f32)
            })
            .collect();
        let mut out = Vec::new();
        fe.process(&iq, &mut out);
        let tail = &out[out.len() / 2..];
        let a = sdrkit_test_free_amp(tail, f_audio);
        (out, a)
    }

    fn sdrkit_test_free_amp(x: &[f32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, &v) in x.iter().enumerate() {
            let p = 2.0 * std::f64::consts::PI * f * i as f64 / AUDIO_RATE;
            re += v as f64 * p.cos();
            im += v as f64 * p.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    #[test]
    fn upper_sideband_passes_and_lower_is_gone() {
        for rate in [8000.0, 48000.0, 250_000.0] {
            let (out, a) = tone_through(
                rate,
                if rate > 10_000.0 {
                    -20_000.0f64.max(-rate / 4.0)
                } else {
                    0.0
                },
                1500.0,
                Demod::Usb,
            );
            assert!(
                (out.len() as f64 - 16000.0).abs() < 200.0,
                "{rate}: {}",
                out.len()
            );
            assert!(a > 0.3, "{rate}: {a}");
        }
        let (_, a) = tone_through(48000.0, 0.0, -1500.0, Demod::Usb);
        assert!(a < 0.01, "{a}");
        let (_, a) = tone_through(48000.0, 0.0, -1500.0, Demod::Lsb);
        assert!(a > 0.3, "{a}");
    }

    #[test]
    fn refuses_a_low_rate() {
        assert!(FrontEnd::new(4000.0, 0.0, Demod::Usb).is_err());
    }
}
