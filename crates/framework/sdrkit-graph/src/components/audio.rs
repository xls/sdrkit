//! Audio: to the audio rate, real to audio, and the null sink.

use super::{audio_engine, dsp, AudioEngine};
use crate::component::{no_values, values_of};
use crate::component::{Applied, ChannelsFrom, Component, ComponentInfo, Io, Port, RateFrom};
use crate::stream::{Format, Kind};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::Values;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value};

const RATE: Setting<field::Hz> =
    Setting::new("rate", "Audio rate", "Hz", 48e3).range(1000.0, 768e3, 0.0);

sdrkit_core::settings! {
    /// The settings of [`AudioResampler`].
    pub enum AudioResamplerSetting {
        /// The output rate.
        Rate = RATE,
    }
}

const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 100.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`RealToAudio`].
    pub enum RealToAudioSetting {
        /// The gain.
        Gain = GAIN,
    }
}

/// Audio resampler (`dsp.audio_resampler`): `Audio` of 1 to 8 channels to `Audio` of the same channel count at `rate`
/// (sdrkit-algo-dsp `Resampler` on whole frames).
///
/// Settings: `rate` (Hz; reformats).
pub struct AudioResampler {
    rate: f64,
    engine: Option<Box<dyn AudioEngine>>,
}

impl Default for AudioResampler {
    fn default() -> Self {
        AudioResampler {
            rate: RATE.default,
            engine: None,
        }
    }
}

impl AudioResampler {
    /// To `rate_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `rate` setting refuses `rate_hz`.
    pub fn new(rate_hz: f64) -> Result<Self, SettingError> {
        AudioResamplerSetting::Rate.check(&Value::Hz(rate_hz))?;
        Ok(AudioResampler {
            rate: rate_hz,
            ..Self::default()
        })
    }
}

impl Component for AudioResampler {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.audio_resampler",
            vec![Port::audio("in")],
            vec![Port::audio("out")
                .rate_from(RateFrom::Setting(AudioResamplerSetting::Rate.id()))
                .channels_from(ChannelsFrom::Input(0))],
        )
        .with_settings(AudioResamplerSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(AudioResamplerSetting::Rate, Value::Hz(self.rate))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let channels = f.channels;
        let rate = self.rate;
        self.engine =
            Some(audio_engine!(channels, T => Resampler::<T>::new(f.rate, rate)).map_err(dsp)?);
        let bw = f.bandwidth_hz.min(0.42 * f.rate.min(self.rate));
        Ok(vec![Format {
            rate: self.rate,
            bandwidth_hz: bw,
            channels,
            ..f.with_kind(Kind::Audio)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(engine) = &mut self.engine else {
            return;
        };
        let x = &io.audio(0).samples;
        engine.run(x, &mut io.audio_out(0).samples);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AudioResamplerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AudioResamplerSetting::Rate => self.rate = RATE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

/// Real to audio (`dsp.real_to_audio`): a `Real` signal to mono `Audio`, times `gain`.
///
/// Settings: `gain` (live).
pub struct RealToAudio {
    gain: f32,
}

impl Default for RealToAudio {
    fn default() -> Self {
        RealToAudio {
            gain: GAIN.default as f32,
        }
    }
}

impl Component for RealToAudio {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.real_to_audio",
            vec![Port::real("in")],
            vec![Port::audio("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(RealToAudioSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(RealToAudioSetting::Gain, Value::F64(f64::from(self.gain)))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Audio)])
    }
    fn process(&mut self, io: &mut Io) {
        let g = self.gain;
        let x = io.real(0);
        io.audio_out(0).samples.extend(x.iter().map(|&s| s * g));
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match RealToAudioSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RealToAudioSetting::Gain => self.gain = GAIN.read(id, v)? as f32,
        }
        Ok(Applied::Live)
    }
}

/// Null sink (`dsp.null_sink`): takes any stream and discards it, counting the items. For an output that must be connected somewhere.
#[derive(Debug, Default)]
pub struct NullSink {
    items: u64,
}

impl NullSink {
    /// Items discarded so far.
    pub fn items(&self) -> u64 {
        self.items
    }
}

impl Component for NullSink {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("dsp.null_sink", vec![Port::any("in")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        self.items += io.input_len(0) as u64;
    }
}
