//! The components that the graph inserts as adapters: a shift and a rate change in one pass, downmix, channel select,
//! duplicate, the analytic signal, the hard decision, the LLR rescale and raw samples in bytes to `Iq` or `Real`.

use super::dsp;
use super::filters::{resampler_latency, Mixer};
use crate::component::{
    no_values, values_of, Applied, ChannelsFrom, Component, ComponentInfo, Io, Latency, Port,
    RateFrom, TagCarry,
};
use crate::stream::{Format, Kind, SampleType, TagKind};
use sdrkit_algo_dsp::fir;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use std::collections::VecDeque;

const SR_FREQ: Setting<field::Hz> =
    Setting::new("freq", "Shift", "Hz", 0.0).range(-1e10, 1e10, 0.0);
const SR_RATE: Setting<field::Hz> = Setting::new("rate", "Rate", "Hz", 48e3).range(1.0, 1e9, 0.0);
const SR_FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow the dial", "", false);
const SR_OUT_OF_VIEW: Setting<field::Bool> =
    Setting::new("out_of_view", "Silent out of view", "", false);

sdrkit_core::settings! {
    /// The settings of [`ShiftResample`].
    pub enum ShiftResampleSetting {
        /// The shift, as the `freq` of the `dsp.shift` component.
        Freq = SR_FREQ,
        /// The output rate.
        Rate = SR_RATE,
        /// Follow the tuned frequency, as the `follow` of the `dsp.shift` component.
        Follow = SR_FOLLOW,
        /// Silence while the centre is outside the capture.
        OutOfView = SR_OUT_OF_VIEW,
    }
}

/// Shift and rate change (`dsp.shift_resample`): an `Iq` stream shifted by `freq` (as `dsp.shift` does) and then brought to
/// `rate` (as `dsp.resampler` does), in one pass. With `follow` the shift puts the tuned frequency at 0 Hz and moves at the
/// sample of each `Retuned` tag. With `out_of_view` the output is silence while the shift is more than half the input rate.
///
/// Settings: `freq` (Hz), `rate` (Hz), `follow`, `out_of_view`. All reformat.
pub struct ShiftResample {
    rate: f64,
    mixer: Mixer,
    resampler: Option<Resampler<Complex32>>,
    mixed: Vec<Complex32>,
    carry: TagCarry,
    centre: f64,
    /// The output centre of each `Retuned` tag forwarded and not yet emitted, in order.
    centres: VecDeque<f64>,
    latency: Latency,
}

impl Default for ShiftResample {
    fn default() -> Self {
        let mut mixer = Mixer::default();
        (mixer.freq, mixer.follow, mixer.out_of_view) =
            (SR_FREQ.default, SR_FOLLOW.default, SR_OUT_OF_VIEW.default);
        ShiftResample {
            rate: SR_RATE.default,
            mixer,
            resampler: None,
            mixed: Vec::new(),
            carry: TagCarry::default(),
            centre: 0.0,
            centres: VecDeque::with_capacity(TagCarry::MAX_PENDING),
            latency: Latency::default(),
        }
    }
}

impl Component for ShiftResample {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.shift_resample",
            vec![Port::iq("in")],
            vec![Port::iq("out")
                .rate_from(RateFrom::Setting(ShiftResampleSetting::Rate.id()))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
        .with_settings(ShiftResampleSetting::schema())
        .manual_tags()
    }
    fn values(&self) -> Values {
        values_of([
            (ShiftResampleSetting::Freq, Value::Hz(self.mixer.freq)),
            (ShiftResampleSetting::Rate, Value::Hz(self.rate)),
            (ShiftResampleSetting::Follow, Value::Bool(self.mixer.follow)),
            (
                ShiftResampleSetting::OutOfView,
                Value::Bool(self.mixer.out_of_view),
            ),
        ])
    }
    fn latency(&self) -> Latency {
        self.latency
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.mixer.configure(f);
        self.resampler = Some(Resampler::new(f.rate, self.rate).map_err(dsp)?);
        self.latency = resampler_latency(f.rate, self.rate)?;
        self.carry = TagCarry::new(f.rate, self.rate);
        self.centres.clear();
        self.centre = f.centre_hz;
        let limit = 0.84 * f.rate.min(self.rate);
        Ok(vec![Format {
            rate: self.rate,
            centre_hz: f.centre_hz - self.mixer.freq,
            bandwidth_hz: f.bandwidth_hz.min(limit),
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(r) = &mut self.resampler else {
            return;
        };
        let x = io.iq(0);
        let (input, out) = io.pair(0, 0);
        self.mixed.clear();
        self.mixer
            .run(x, input.first(), input.tags(), &[], &mut self.mixed);
        let Some(v) = <Complex32 as crate::component::StreamSample>::vec_mut(&mut out.data) else {
            return;
        };
        r.process(&self.mixed, v);
        for t in input.tags() {
            if let TagKind::Retuned { centre_hz, .. } = t.kind {
                self.centre = centre_hz;
                if self.centres.len() < TagCarry::MAX_PENDING {
                    self.centres.push_back(centre_hz - self.mixer.freq_at(t.at));
                }
            }
        }
        let before = out.tags().len();
        self.carry.forward(input, out);
        for t in &mut out.tags_mut()[before..] {
            if let TagKind::Retuned { centre_hz, .. } = &mut t.kind {
                if let Some(c) = self.centres.pop_front() {
                    *centre_hz = c;
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ShiftResampleSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ShiftResampleSetting::Freq => self.mixer.freq = SR_FREQ.read(id, v)?,
            ShiftResampleSetting::Rate => self.rate = SR_RATE.read(id, v)?,
            ShiftResampleSetting::Follow => self.mixer.follow = SR_FOLLOW.read(id, v)?,
            ShiftResampleSetting::OutOfView => {
                self.mixer.out_of_view = SR_OUT_OF_VIEW.read(id, v)?
            }
        }
        Ok(Applied::Reformat)
    }
}

/// Appends the interleaved frames of `input`, which has `from` channels, to `out` as frames of `to` channels. The
/// channel counts are at least 1. One channel out is the mean of the channels of the frame. One channel in goes to
/// every channel out. Otherwise the first `to` channels are kept, and channels that the input does not have are
/// silent. A part of a frame at the end of `input` is left out.
pub fn convert_channels(input: &[f32], from: u16, to: u16, out: &mut Vec<f32>) {
    let (from, to) = (usize::from(from.max(1)), usize::from(to.max(1)));
    out.reserve(input.len() / from * to);
    if from == to {
        out.extend_from_slice(&input[..input.len() - input.len() % from]);
    } else if to == 1 {
        let k = 1.0 / from as f32;
        out.extend(input.chunks_exact(from).map(|f| f.iter().sum::<f32>() * k));
    } else if from == 1 {
        for &s in input {
            out.extend(std::iter::repeat_n(s, to));
        }
    } else {
        for f in input.chunks_exact(from) {
            out.extend((0..to).map(|c| f.get(c).copied().unwrap_or(0.0)));
        }
    }
}

/// Downmix (`dsp.downmix`): `Audio` of any channel count to mono, the mean of the channels of each frame.
#[derive(Debug, Default)]
pub struct Downmix;

impl Component for Downmix {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.downmix",
            vec![Port::audio("in")],
            vec![Port::audio("out")
                .rate_from(RateFrom::Input(0))
                .channels_from(ChannelsFrom::Fixed(1))],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            channels: 1,
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_format(0).channels;
        let x = &io.audio(0).samples;
        convert_channels(x, n, 1, &mut io.audio_out(0).samples);
    }
}

const CHANNELS: Setting<field::I64> =
    Setting::new("channels", "Channels", "", 2).range(1.0, 8.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`SelectChannels`] and [`Duplicate`].
    pub enum ChannelCountSetting {
        /// The channel count of the output.
        Channels = CHANNELS,
    }
}

/// Channel select (`dsp.select_channels`): the first `channels` channels of each `Audio` frame.
///
/// Settings: `channels` (1 to 8; reformats).
#[derive(Debug)]
pub struct SelectChannels {
    channels: u16,
}

impl Default for SelectChannels {
    fn default() -> Self {
        SelectChannels { channels: 2 }
    }
}

impl Component for SelectChannels {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.select_channels",
            vec![Port::audio("in")],
            vec![Port::audio("out").rate_from(RateFrom::Input(0))],
        )
        .with_settings(ChannelCountSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(
            ChannelCountSetting::Channels,
            Value::I64(i64::from(self.channels)),
        )])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            channels: self.channels.min(inputs[0].channels.max(1)),
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let n = usize::from(io.input_format(0).channels.max(1));
        let m = usize::from(io.output_format(0).channels);
        let x = &io.audio(0).samples;
        let out = &mut io.audio_out(0).samples;
        for f in x.chunks_exact(n) {
            out.extend_from_slice(&f[..m]);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelCountSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelCountSetting::Channels => self.channels = CHANNELS.read(id, v)? as u16,
        }
        Ok(Applied::Reformat)
    }
}

/// Duplicate (`dsp.duplicate`): mono `Audio` to `channels` equal channels.
///
/// Settings: `channels` (1 to 8; reformats).
#[derive(Debug)]
pub struct Duplicate {
    channels: u16,
}

impl Default for Duplicate {
    fn default() -> Self {
        Duplicate { channels: 2 }
    }
}

impl Component for Duplicate {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.duplicate",
            vec![Port::audio("in").channels(crate::component::ChannelRule::Exact(1))],
            vec![Port::audio("out").rate_from(RateFrom::Input(0))],
        )
        .with_settings(ChannelCountSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(
            ChannelCountSetting::Channels,
            Value::I64(i64::from(self.channels)),
        )])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            channels: self.channels,
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = &io.audio(0).samples;
        convert_channels(x, 1, self.channels, &mut io.audio_out(0).samples);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ChannelCountSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelCountSetting::Channels => self.channels = CHANNELS.read(id, v)? as u16,
        }
        Ok(Applied::Reformat)
    }
}

const LOW: Setting<field::Hz> = Setting::new("low", "Lowest frequency", "Hz", 1200.0)
    .range(1e-3, 1e9, 0.0)
    .help(
        "The Hilbert transform is flat from this frequency to half the rate less this frequency.",
    );

sdrkit_core::settings! {
    /// The settings of [`Analytic`].
    pub enum AnalyticSetting {
        /// The lowest frequency kept.
        Low = LOW,
    }
}

/// Analytic signal (`dsp.analytic`): a `Real` stream to `Iq`. The real part is the input delayed by the Hilbert filter's group
/// delay, the imaginary part is the Hilbert transform (flat from `low` to half the rate less `low`, 60 dB).
///
/// Settings: `low` (Hz; reformats).
pub struct Analytic {
    low: f64,
    hilbert: Option<fir::Fir<f32>>,
    delay: VecDeque<f32>,
    latency: Latency,
}

impl Default for Analytic {
    fn default() -> Self {
        Analytic {
            low: LOW.default,
            hilbert: None,
            delay: VecDeque::new(),
            latency: Latency::default(),
        }
    }
}

impl Component for Analytic {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.analytic",
            vec![Port::real("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
        .with_settings(AnalyticSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(AnalyticSetting::Low, Value::Hz(self.low))])
    }
    fn latency(&self) -> Latency {
        self.latency
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AnalyticSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AnalyticSetting::Low => self.low = LOW.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let taps = fir::hilbert_design(f.rate, self.low, 60.0).map_err(dsp)?;
        let half = (taps.len() - 1) / 2;
        self.latency = Latency {
            group_delay_s: half as f64 / f.rate,
            first_valid: (taps.len() - 1) as u64,
        };
        self.hilbert = Some(fir::Fir::new(&taps));
        self.delay = VecDeque::with_capacity(taps.len());
        self.delay
            .extend(std::iter::repeat_n(0.0, (taps.len() - 1) / 2));
        Ok(vec![f.with_kind(Kind::Iq)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(h) = &mut self.hilbert else {
            return;
        };
        let x = io.real(0);
        let out = io.iq_out(0);
        out.reserve(x.len());
        for &s in x {
            self.delay.push_back(s);
            let re = self
                .delay
                .pop_front()
                .expect("the delay line holds the sample just pushed");
            out.push(Complex32::new(re, h.process_sample(s)));
        }
    }
}

/// Hard decision (`dsp.hard_decision`): `Llr` soft bits to `HardBits`. A negative value produces 1, any other value produces 0.
#[derive(Debug, Default)]
pub struct HardDecision;

impl Component for HardDecision {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.hard_decision",
            vec![Port::llr("in")],
            vec![Port::hard_bits("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format {
            llr_scale: None,
            ..inputs[0].with_kind(Kind::HardBits)
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.llrs(0);
        io.hard_bits_out(0)
            .extend(x.iter().map(|&l| u8::from(l < 0)));
    }
}

const LLR_SCALE: Setting<field::F64> =
    Setting::new("scale", "Scale", "", 8.0).range(1e-6, 1e6, 0.0);

sdrkit_core::settings! {
    /// The settings of [`LlrRescale`].
    pub enum LlrRescaleSetting {
        /// The scale of the output.
        Scale = LLR_SCALE,
    }
}

/// LLR rescale (`dsp.llr_rescale`): soft bits of one scale ([`Format::llr_scale`]) to soft bits of scale `scale`, rounded
/// and saturated at ±127. The input must state its scale.
///
/// Settings: `scale` (reformats).
#[derive(Debug)]
pub struct LlrRescale {
    scale: f64,
    factor: f32,
}

impl Default for LlrRescale {
    fn default() -> Self {
        LlrRescale {
            scale: LLR_SCALE.default,
            factor: 1.0,
        }
    }
}

impl Component for LlrRescale {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.llr_rescale",
            vec![Port::llr("in")],
            vec![Port::llr("out").same_format_as(0)],
        )
        .with_settings(LlrRescaleSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(LlrRescaleSetting::Scale, Value::F64(self.scale))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let Some(from) = inputs[0].llr_scale.filter(|s| *s > 0.0) else {
            return Err(stage(AdapterInput::NoLlrScale));
        };
        self.factor = (self.scale / f64::from(from)) as f32;
        Ok(vec![Format {
            llr_scale: Some(self.scale as f32),
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let k = self.factor;
        let x = io.llrs(0);
        io.llrs_out(0).extend(
            x.iter()
                .map(|&v| (f32::from(v) * k).round().clamp(-127.0, 127.0) as i8),
        );
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match LlrRescaleSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            LlrRescaleSetting::Scale => self.scale = LLR_SCALE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

/// The value of sample `i` of `b`, a little-endian buffer of samples of type `t`, scaled to about ±1.0 at full scale.
fn sample(b: &[u8], t: SampleType, i: usize) -> f32 {
    match t {
        SampleType::I8 | SampleType::Cs8 => f32::from(b[i] as i8) / 128.0,
        SampleType::U8 | SampleType::Cu8 => (f32::from(b[i]) - 127.5) / 127.5,
        SampleType::I16 | SampleType::Cs16 => {
            f32::from(i16::from_le_bytes([b[2 * i], b[2 * i + 1]])) / 32768.0
        }
        SampleType::I32 | SampleType::Cs32 => {
            i32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]) as f32
                / 2_147_483_648.0
        }
        SampleType::F32 | SampleType::Cf32 => {
            f32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]])
        }
    }
}

/// Appends the whole complex samples in `bytes` (interleaved little-endian values of type `t`, I then Q) to `out`, full scale
/// at about ±1.0. Returns the number of bytes it read: a part of a sample at the end is left.
pub fn iq_from_bytes(t: SampleType, bytes: &[u8], out: &mut Vec<Complex32>) -> usize {
    let size = 2 * t.value_bytes();
    let whole = bytes.len() / size;
    out.extend(
        (0..whole).map(|i| Complex32::new(sample(bytes, t, 2 * i), sample(bytes, t, 2 * i + 1))),
    );
    whole * size
}

/// The sample type of a `Bytes` input of raw samples, if it is one of `types`.
fn raw_type(f: &Format, types: &[SampleType]) -> Result<SampleType, ConfigError> {
    f.sample_type
        .filter(|t| types.contains(t))
        .ok_or(stage(AdapterInput::NotSamples {
            sample_type: f.sample_type,
        }))
}

/// Why an adapter refuses the format of its input.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum AdapterInput {
    /// Soft bits that do not state their scale cannot be rescaled.
    #[error("the soft bits state no scale")]
    NoLlrScale,
    /// Bytes that hold no samples of a type the converter reads.
    #[error("the bytes hold no samples this converter reads (sample type {sample_type:?})")]
    NotSamples {
        /// The sample type of the input.
        sample_type: Option<SampleType>,
    },
}

fn stage(e: AdapterInput) -> ConfigError {
    ConfigError::Stage(sdrkit_core::StageError::new(e))
}

/// Raw samples to `Iq` (`dsp.to_iq`): `Bytes` packets of interleaved complex samples (`cs8`, `cu8`, `cs16`, `cs32` or `cf32`,
/// little-endian, as [`Format::sample_type`] states) to `Iq` at the same rate, full scale at about ±1.0. A sample split
/// between two packets is joined.
#[derive(Debug, Default)]
pub struct ToIq {
    from: Option<SampleType>,
    rest: Vec<u8>,
}

impl Component for ToIq {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.to_iq",
            vec![Port::bytes("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        use SampleType as T;
        self.from = Some(raw_type(
            &inputs[0],
            &[T::Cs8, T::Cu8, T::Cs16, T::Cs32, T::Cf32],
        )?);
        self.rest.clear();
        Ok(vec![inputs[0].with_kind(Kind::Iq)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(t) = self.from else { return };
        let size = 2 * t.value_bytes();
        let packets = io.bytes(0);
        let out = io.iq_out(0);
        for p in packets {
            let mut b = p.bytes();
            if !self.rest.is_empty() {
                let take = (size - self.rest.len()).min(b.len());
                self.rest.extend_from_slice(&b[..take]);
                b = &b[take..];
                if self.rest.len() == size {
                    out.push(Complex32::new(
                        sample(&self.rest, t, 0),
                        sample(&self.rest, t, 1),
                    ));
                    self.rest.clear();
                }
            }
            let used = iq_from_bytes(t, b, out);
            self.rest.extend_from_slice(&b[used..]);
        }
    }
}

/// Raw samples to `Real` (`dsp.to_real`): `Bytes` packets of samples (`i8`, `u8`, `i16`, `i32` or `f32`, little-endian, as
/// [`Format::sample_type`] states) to `Real` at the same rate, full scale at about ±1.0. A sample split between two packets
/// is joined.
#[derive(Debug, Default)]
pub struct ToReal {
    from: Option<SampleType>,
    rest: Vec<u8>,
}

impl Component for ToReal {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.to_real",
            vec![Port::bytes("in")],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        use SampleType as T;
        self.from = Some(raw_type(
            &inputs[0],
            &[T::I8, T::U8, T::I16, T::I32, T::F32],
        )?);
        self.rest.clear();
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(t) = self.from else { return };
        let size = t.value_bytes();
        let packets = io.bytes(0);
        let out = io.real_out(0);
        for p in packets {
            let mut b = p.bytes();
            if !self.rest.is_empty() {
                let take = (size - self.rest.len()).min(b.len());
                self.rest.extend_from_slice(&b[..take]);
                b = &b[take..];
                if self.rest.len() == size {
                    out.push(sample(&self.rest, t, 0));
                    self.rest.clear();
                }
            }
            let whole = b.len() / size;
            out.extend((0..whole).map(|i| sample(b, t, i)));
            self.rest.extend_from_slice(&b[whole * size..]);
        }
    }
}
