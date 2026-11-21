//! Shifting, filtering and rate changes.

use super::{audio_engine, dsp, AudioEngine, PerKind};
use crate::component::values_of;
use crate::component::{
    Applied, ChannelsFrom, Component, ComponentInfo, Io, Port, RateFrom, StreamSample, TagCarry,
};
use crate::stream::{Format, Kind, TagKind};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_dsp::decim::Decimator;
use sdrkit_algo_dsp::fir::{self, ComplexFir, Fir};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs, Deemphasis};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::publish::field;
use sdrkit_core::Values;
use sdrkit_core::{Complex32, ConfigError, Setting, SettingError, SettingId, SettingSet, Value};

const SHIFT_FREQ: Setting<field::Hz> = Setting::new("freq", "Shift", "Hz", 0.0)
    .range(-1e10, 1e10, 0.0)
    .help("Moves the spectrum up by this much (a signal at minus this moves to the centre).");

const FOLLOW: Setting<field::Bool> = Setting::new("follow", "Follow the dial", "", false)
    .help("Puts the tuned frequency at the centre, and follows each retune.");
const OUT_OF_VIEW: Setting<field::Bool> =
    Setting::new("out_of_view", "Silent out of view", "", false)
        .help("Gives silence while the shifted centre lies outside the capture.");

sdrkit_core::settings! {
    /// The settings of [`Shift`].
    pub enum ShiftSetting {
        /// The shift.
        Freq = SHIFT_FREQ,
        /// Follow the tuned frequency.
        Follow = FOLLOW,
        /// Silence while the centre is outside the capture.
        OutOfView = OUT_OF_VIEW,
    }
}

sdrkit_core::choice_enum! {
    /// The response of a FIR filter.
    pub enum FirResponse {
        /// Low-pass.
        Lowpass = ("lowpass", "Low-pass"),
        /// High-pass.
        Highpass = ("highpass", "High-pass"),
        /// Band-pass.
        Bandpass = ("bandpass", "Band-pass"),
    }
}

const RESPONSE: Setting<field::Choice<FirResponse>> =
    Setting::choice("response", "Response", FirResponse::Lowpass);
const FIR_LOW: Setting<field::Hz> =
    Setting::new("low", "Low edge", "Hz", 0.0).range(-1e9, 1e9, 0.0);
const FIR_HIGH: Setting<field::Hz> =
    Setting::new("high", "High edge", "Hz", 3000.0).range(-1e9, 1e9, 0.0);
const TRANSITION: Setting<field::Hz> =
    Setting::new("transition", "Transition", "Hz", 500.0).range(1.0, 1e9, 0.0);
const ATTEN: Setting<field::F64> =
    Setting::new("atten", "Stopband", "dB", 60.0).range(20.0, 120.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`FirFilter`].
    pub enum FirSetting {
        /// The response.
        Response = RESPONSE,
        /// The high-pass edge, or the lower edge of a band-pass.
        Low = FIR_LOW,
        /// The low-pass edge, or the upper edge of a band-pass.
        High = FIR_HIGH,
        /// The transition width.
        Transition = TRANSITION,
        /// The stopband attenuation.
        Atten = ATTEN,
    }
}

const FACTOR: Setting<field::I64> = Setting::new("factor", "Factor", "", 2).range(1.0, 1024.0, 1.0);
const PASS: Setting<field::F64> = Setting::new("pass", "Passband", "x rate", 0.4)
    .range(0.05, 0.45, 0.0)
    .help("Flat up to this fraction of the output rate.");

sdrkit_core::settings! {
    /// The settings of [`Decimate`].
    pub enum DecimateSetting {
        /// The decimation factor.
        Factor = FACTOR,
        /// The flat part, as a fraction of the output rate.
        Pass = PASS,
    }
}

const RESAMPLE_RATE: Setting<field::Hz> =
    Setting::new("rate", "Rate", "Hz", 48e3).range(1.0, 1e9, 0.0);

sdrkit_core::settings! {
    /// The settings of [`Resample`].
    pub enum ResampleSetting {
        /// The output rate.
        Rate = RESAMPLE_RATE,
    }
}

sdrkit_core::choice_enum! {
    /// The filter a channel uses.
    pub enum ChannelFilter {
        /// The narrow width.
        Narrow = ("narrow", "Narrow"),
        /// The normal width.
        Normal = ("normal", "Normal"),
        /// The wide width.
        Wide = ("wide", "Wide"),
        /// The custom width.
        Custom = ("custom", "Custom"),
    }
}

const OFFSET: Setting<field::Hz> = Setting::new("offset", "Offset", "Hz", 0.0)
    .range(-1e10, 1e10, 0.0)
    .help("Where the signal is, from the tuned frequency.");
const FILTER: Setting<field::Choice<ChannelFilter>> =
    Setting::choice("filter", "Filter", ChannelFilter::Normal);
const NARROW: Setting<field::Hz> =
    Setting::new("narrow", "Narrow width", "Hz", 5e3).range(1.0, 1e9, 0.0);
const NORMAL: Setting<field::Hz> =
    Setting::new("normal", "Normal width", "Hz", 10e3).range(1.0, 1e9, 0.0);
const WIDE: Setting<field::Hz> =
    Setting::new("wide", "Wide width", "Hz", 20e3).range(1.0, 1e9, 0.0);
const CUSTOM: Setting<field::Hz> =
    Setting::new("custom", "Custom width", "Hz", 10e3).range(1.0, 1e9, 0.0);
const CHANNEL_RATE: Setting<field::Hz> = Setting::new("rate", "Output rate", "Hz", 48e3)
    .range(1.0, 1e9, 0.0)
    .help("The lowest output rate wanted.");
const EXACT: Setting<field::Bool> = Setting::new("exact", "Exact rate", "", false);

sdrkit_core::settings! {
    /// The settings of [`Channel`].
    pub enum ChannelSetting {
        /// The offset of the signal from the tuned frequency.
        Offset = OFFSET,
        /// The filter in use.
        Filter = FILTER,
        /// The narrow width.
        Narrow = NARROW,
        /// The normal width.
        Normal = NORMAL,
        /// The wide width.
        Wide = WIDE,
        /// The custom width.
        Custom = CUSTOM,
        /// The lowest output rate.
        Rate = CHANNEL_RATE,
        /// Resample to the rate exactly.
        Exact = EXACT,
    }
}

const DEEMPHASIS_US: Setting<field::F64> = Setting::new("deemphasis_us", "De-emphasis", "µs", 0.0)
    .range(0.0, 750.0, 0.0)
    .help("The time constant. 0 is off. FM broadcasting uses 50 µs or 75 µs.");

sdrkit_core::settings! {
    /// The settings of [`DeemphasisComponent`].
    pub enum DeemphasisSetting {
        /// The time constant (µs, 0 for off).
        TimeConstant = DEEMPHASIS_US,
    }
}

sdrkit_core::choice_enum! {
    /// The shape of a biquad section.
    pub enum BiquadShape {
        /// Low-pass.
        Lowpass = ("lowpass", "Low-pass"),
        /// High-pass.
        Highpass = ("highpass", "High-pass"),
        /// Band-pass.
        Bandpass = ("bandpass", "Band-pass"),
        /// Notch.
        Notch = ("notch", "Notch"),
        /// All-pass.
        Allpass = ("allpass", "All-pass"),
        /// Peaking.
        Peaking = ("peaking", "Peaking"),
        /// Low shelf.
        LowShelf = ("lowshelf", "Low shelf"),
        /// High shelf.
        HighShelf = ("highshelf", "High shelf"),
    }
}

const SHAPE: Setting<field::Choice<BiquadShape>> =
    Setting::choice("type", "Type", BiquadShape::Lowpass);
const BQ_FREQ: Setting<field::Hz> =
    Setting::new("freq", "Frequency", "Hz", 1000.0).range(1.0, 1e9, 0.0);
const Q: Setting<field::F64> =
    Setting::new("q", "Q", "", std::f64::consts::FRAC_1_SQRT_2).range(0.01, 100.0, 0.0);
const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "dB", 0.0).range(-60.0, 60.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`BiquadComponent`].
    pub enum BiquadSetting {
        /// The shape.
        Shape = SHAPE,
        /// The frequency.
        Freq = BQ_FREQ,
        /// The quality.
        Q = Q,
        /// The gain of a peaking or shelf section.
        Gain = GAIN,
    }
}

const SAMPLES: [Kind; 3] = [Kind::Iq, Kind::Real, Kind::Audio];

/// A change of the shift of a [`Shift`] at an exact sample, on its `control` input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShiftChange {
    /// The index of the first input sample of the new shift, counted from the start of the stream.
    pub at: u64,
    /// The new shift, in Hz.
    pub freq_hz: f64,
}

impl crate::stream::Item for ShiftChange {
    const KIND: &'static str = "dsp.shift_change";
}

/// The oscillator of a shift: a fixed shift or one that follows the tuned frequency, phase continuous across changes, and silent
/// while the shift leaves the capture when that is requested.
#[derive(Debug, Default)]
pub(crate) struct Mixer {
    pub(crate) freq: f64,
    pub(crate) follow: bool,
    pub(crate) out_of_view: bool,
    rate: f64,
    nco: Option<Nco>,
    /// The changes of this call: the input index and the new shift.
    points: Vec<(u64, f64)>,
    /// The shift at the start of this call.
    start_freq: f64,
}

impl Mixer {
    /// Starts for input `f`. A shift that follows takes the tuned frequency to 0 Hz.
    pub(crate) fn configure(&mut self, f: &Format) {
        if self.follow {
            self.freq = f.centre_hz - f.tuned_hz;
        }
        self.rate = f.rate;
        self.nco = Some(Nco::new(f.rate, self.freq));
        self.points.clear();
    }
    /// Changes the shift from the next sample on.
    pub(crate) fn set(&mut self, freq: f64) {
        self.freq = freq;
        if let Some(n) = &mut self.nco {
            n.set_freq(freq);
        }
    }
    fn silent(&self) -> bool {
        self.out_of_view && self.freq.abs() > self.rate / 2.0
    }
    fn write(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        let start = out.len();
        if self.silent() {
            out.resize(start + x.len(), Complex32::default());
        } else if let Some(n) = &mut self.nco {
            out.extend_from_slice(x);
            n.mix_in_place(&mut out[start..]);
        }
    }
    /// Appends `x` (first index `first`) shifted to `out`. The shift changes at each `Retuned` tag when it follows, and at
    /// each item of `control`, at its sample.
    pub(crate) fn run(
        &mut self,
        x: &[Complex32],
        first: u64,
        tags: &[crate::stream::Tag],
        control: &[crate::stream::Timed<ShiftChange>],
        out: &mut Vec<Complex32>,
    ) {
        self.points.clear();
        self.start_freq = self.freq;
        let end = first + x.len() as u64;
        let follow = self.follow;
        let retunes = tags.iter().filter_map(|t| match t.kind {
            TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } if follow => Some((t.at, centre_hz - tuned_hz)),
            _ => None,
        });
        let mut retunes = retunes.peekable();
        let mut changes = control
            .iter()
            .map(|c| (c.value.at, c.value.freq_hz))
            .peekable();
        let mut done = 0usize;
        loop {
            let next = match (retunes.peek(), changes.peek()) {
                (Some(a), Some(b)) if a.0 <= b.0 => retunes.next(),
                (Some(_), Some(_)) | (None, Some(_)) => changes.next(),
                (Some(_), None) => retunes.next(),
                (None, None) => None,
            };
            let Some((at, freq)) = next else { break };
            let at = at.clamp(first, end);
            let k = (at - first) as usize;
            if k > done {
                self.write(&x[done..k], out);
                done = k;
            }
            self.set(freq);
            self.points.push((at, freq));
        }
        self.write(&x[done..], out);
    }
    /// The shift in effect at input index `at` of the last call.
    pub(crate) fn freq_at(&self, at: u64) -> f64 {
        self.points
            .iter()
            .rev()
            .find(|p| p.0 <= at)
            .map_or(self.start_freq, |p| p.1)
    }
    /// The changes of the last call, in sample order: their input index and shift.
    pub(crate) fn points(&self) -> &[(u64, f64)] {
        &self.points
    }
}

/// Frequency shift (`dsp.shift`): multiplies by an oscillator at `freq` Hz, so a signal at `-freq` moves to 0 Hz. The phase is
/// continuous across calls and across changes. The output's centre is `centre - freq`. A change while running is announced by
/// a `Retuned` tag at its sample, and `Retuned` tags from upstream are passed on with the shift applied.
///
/// With `follow`, the shift puts the tuned frequency at 0 Hz and moves at the sample of each `Retuned` tag. With `out_of_view`,
/// the output is silence while the shift is more than half the rate. The optional `control` input takes [`ShiftChange`] items:
/// each one changes the shift at its exact input sample.
///
/// Settings: `freq` (Hz, live), `follow` (reformats), `out_of_view` (live).
#[derive(Debug, Default)]
pub struct Shift {
    mixer: Mixer,
    centre: f64,
    tuned: f64,
    announce: bool,
}

impl Shift {
    /// A shift by `freq_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `freq` setting refuses `freq_hz`.
    pub fn new(freq_hz: f64) -> Result<Self, SettingError> {
        ShiftSetting::Freq.check(&Value::Hz(freq_hz))?;
        let mut s = Shift::default();
        s.mixer.freq = freq_hz;
        Ok(s)
    }
}

impl Component for Shift {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.shift",
            vec![
                Port::iq("in"),
                Port::typed::<ShiftChange>("control").optional("the `freq` setting"),
            ],
            vec![Port::iq("out").same_format_as(0)],
        )
        .with_settings(ShiftSetting::schema())
        .manual_tags()
    }
    fn values(&self) -> Values {
        values_of([
            (ShiftSetting::Freq, Value::Hz(self.mixer.freq)),
            (ShiftSetting::Follow, Value::Bool(self.mixer.follow)),
            (ShiftSetting::OutOfView, Value::Bool(self.mixer.out_of_view)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.mixer.configure(f);
        (self.centre, self.tuned) = (f.centre_hz, f.tuned_hz);
        self.announce = false;
        Ok(vec![Format {
            centre_hz: f.centre_hz - self.mixer.freq,
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let control: &[crate::stream::Timed<ShiftChange>] =
            if io.is_connected(1) { io.items(1) } else { &[] };
        let (input, out) = io.pair(0, 0);
        let first = input.first();
        let base = out.end();
        let Some(v) = Complex32::vec_mut(&mut out.data) else {
            return;
        };
        self.mixer.run(x, first, input.tags(), control, v);
        // Same rate: input index `first + k` is output index `base + k`.
        let to_out = |at: u64| base + at.saturating_sub(first);
        if std::mem::take(&mut self.announce) {
            let freq = self.mixer.freq_at(first);
            out.push_tag(
                base,
                TagKind::Retuned {
                    centre_hz: self.centre - freq,
                    tuned_hz: self.tuned,
                },
            );
        }
        let mut changes = self.mixer.points().iter().peekable();
        for t in input.tags() {
            while let Some(&&(at, freq)) = changes.peek() {
                if at > t.at {
                    break;
                }
                changes.next();
                if !input
                    .tags()
                    .iter()
                    .any(|u| u.at == at && matches!(u.kind, TagKind::Retuned { .. }))
                {
                    out.push_tag(
                        to_out(at),
                        TagKind::Retuned {
                            centre_hz: self.centre - freq,
                            tuned_hz: self.tuned,
                        },
                    );
                }
            }
            let kind = match t.kind {
                TagKind::Retuned {
                    centre_hz,
                    tuned_hz,
                } => {
                    (self.centre, self.tuned) = (centre_hz, tuned_hz);
                    TagKind::Retuned {
                        centre_hz: centre_hz - self.mixer.freq_at(t.at),
                        tuned_hz,
                    }
                }
                ref k => k.clone(),
            };
            out.push_tag(to_out(t.at), kind);
        }
        for &(at, freq) in changes {
            out.push_tag(
                to_out(at),
                TagKind::Retuned {
                    centre_hz: self.centre - freq,
                    tuned_hz: self.tuned,
                },
            );
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ShiftSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ShiftSetting::Freq => {
                let freq = SHIFT_FREQ.read(id, v)?;
                if self.mixer.follow {
                    return Err(SettingError::NotPermitted { id });
                }
                self.mixer.set(freq);
                self.announce = true;
                Ok(Applied::Live)
            }
            ShiftSetting::Follow => {
                self.mixer.follow = FOLLOW.read(id, v)?;
                Ok(Applied::Reformat)
            }
            ShiftSetting::OutOfView => {
                self.mixer.out_of_view = OUT_OF_VIEW.read(id, v)?;
                Ok(Applied::Live)
            }
        }
    }
}

enum FirEngine {
    None,
    Real(Fir<f32>),
    RealOnIq(Fir<Complex32>),
    Complex(ComplexFir),
}

/// FIR filter (`dsp.fir`) on `Iq` or `Real`: low-pass, high-pass or band-pass, Kaiser-windowed (sdrkit-algo-dsp `fir`). On `Iq` a band-pass
/// is complex, so its edges may be negative or asymmetric.
///
/// Settings (live): `response` (`lowpass`, `highpass`, `bandpass`), `low` (Hz: the high-pass edge, the band-pass lower edge),
/// `high` (Hz: the low-pass edge, the band-pass upper edge), `transition` (Hz), `atten` (dB).
pub struct FirFilter {
    response: FirResponse,
    low: f64,
    high: f64,
    transition: f64,
    atten: f64,
    rate: f64,
    kind: Option<Kind>,
    engine: FirEngine,
}

impl Default for FirFilter {
    fn default() -> Self {
        FirFilter {
            response: RESPONSE.default,
            low: FIR_LOW.default,
            high: FIR_HIGH.default,
            transition: TRANSITION.default,
            atten: ATTEN.default,
            rate: 0.0,
            kind: None,
            engine: FirEngine::None,
        }
    }
}

impl FirFilter {
    /// A filter of `response` with these edges and `transition_hz` wide, 60 dB.
    fn of(
        response: FirResponse,
        low: f64,
        high: f64,
        transition: f64,
    ) -> Result<Self, SettingError> {
        FirSetting::Low.check(&Value::Hz(low))?;
        FirSetting::High.check(&Value::Hz(high))?;
        FirSetting::Transition.check(&Value::Hz(transition))?;
        Ok(FirFilter {
            response,
            low,
            high,
            transition,
            ..Self::default()
        })
    }
    /// A low-pass flat to `pass_hz`, `transition_hz` wide, 60 dB.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `high` or `transition` setting refuses its value.
    pub fn lowpass(pass_hz: f64, transition_hz: f64) -> Result<Self, SettingError> {
        Self::of(FirResponse::Lowpass, 0.0, pass_hz, transition_hz)
    }
    /// A band-pass from `low_hz` to `high_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `low`, `high` or `transition` setting refuses its value.
    pub fn bandpass(low_hz: f64, high_hz: f64, transition_hz: f64) -> Result<Self, SettingError> {
        Self::of(FirResponse::Bandpass, low_hz, high_hz, transition_hz)
    }
    /// A high-pass from `edge_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `low` or `transition` setting refuses its value.
    pub fn highpass(edge_hz: f64, transition_hz: f64) -> Result<Self, SettingError> {
        Self::of(FirResponse::Highpass, edge_hz, 0.0, transition_hz)
    }

    /// The filter for the present settings.
    ///
    /// # Errors
    ///
    /// [`ConfigError::SettingAtRate`] for an edge that does not fit the rate.
    fn design(&self) -> Result<FirEngine, ConfigError> {
        let (rate, t, a) = (self.rate, self.transition, self.atten);
        let nyq = rate / 2.0;
        let iq = self.kind == Some(Kind::Iq);
        let at_rate = |s: FirSetting| ConfigError::SettingAtRate {
            id: s.id(),
            rate_hz: rate,
        };
        let real = |h: Vec<f32>| {
            if iq {
                FirEngine::RealOnIq(Fir::new(&h))
            } else {
                FirEngine::Real(Fir::new(&h))
            }
        };
        Ok(match self.response {
            FirResponse::Lowpass => {
                if !(self.high > 0.0 && self.high + t < nyq) {
                    return Err(at_rate(FirSetting::High));
                }
                real(fir::lowpass(rate, self.high, self.high + t, a).map_err(dsp)?)
            }
            FirResponse::Highpass => {
                if !(self.low - t > 0.0 && self.low < nyq) {
                    return Err(at_rate(FirSetting::Low));
                }
                real(fir::highpass(rate, self.low - t, self.low, a).map_err(dsp)?)
            }
            FirResponse::Bandpass if iq => {
                if !(self.low < self.high && self.low - t > -nyq && self.high + t < nyq) {
                    return Err(at_rate(FirSetting::High));
                }
                FirEngine::Complex(ComplexFir::new(
                    &fir::bandpass_complex(rate, self.low, self.high, t, a).map_err(dsp)?,
                ))
            }
            FirResponse::Bandpass => {
                if !(self.low > 0.0 && self.low < self.high && self.high < nyq) {
                    return Err(at_rate(FirSetting::High));
                }
                FirEngine::Real(Fir::new(
                    &fir::bandpass_real(rate, self.low, self.high, t, a).map_err(dsp)?,
                ))
            }
        })
    }
}

impl Component for FirFilter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.fir",
            vec![Port::one_of("in", &[Kind::Iq, Kind::Real])],
            vec![Port::same_as("out", 0).same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(FirSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                FirSetting::Response,
                Value::Choice(sdrkit_core::Choice::index(self.response)),
            ),
            (FirSetting::Low, Value::Hz(self.low)),
            (FirSetting::High, Value::Hz(self.high)),
            (FirSetting::Transition, Value::Hz(self.transition)),
            (FirSetting::Atten, Value::F64(self.atten)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.kind = Some(inputs[0].kind);
        self.engine = self.design()?;
        let bw = match self.response {
            FirResponse::Lowpass if self.kind == Some(Kind::Iq) => 2.0 * self.high,
            FirResponse::Lowpass => self.high,
            FirResponse::Bandpass => self.high - self.low,
            FirResponse::Highpass => inputs[0].bandwidth_hz,
        };
        Ok(vec![Format {
            bandwidth_hz: bw.min(inputs[0].bandwidth_hz),
            ..inputs[0].clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        match &mut self.engine {
            FirEngine::Real(f) => f.process_in_place(io.pass_through::<f32>(0, 0)),
            FirEngine::RealOnIq(f) => f.process_in_place(io.pass_through::<Complex32>(0, 0)),
            FirEngine::Complex(f) => f.process_in_place(io.pass_through::<Complex32>(0, 0)),
            FirEngine::None => {}
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let old = (
            self.response,
            self.low,
            self.high,
            self.transition,
            self.atten,
        );
        match FirSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FirSetting::Response => self.response = RESPONSE.read(id, v)?,
            FirSetting::Low => self.low = FIR_LOW.read(id, v)?,
            FirSetting::High => self.high = FIR_HIGH.read(id, v)?,
            FirSetting::Transition => self.transition = TRANSITION.read(id, v)?,
            FirSetting::Atten => self.atten = ATTEN.read(id, v)?,
        }
        if self.kind.is_some() {
            match self.design() {
                Ok(e) => self.engine = e,
                Err(e) => {
                    (
                        self.response,
                        self.low,
                        self.high,
                        self.transition,
                        self.atten,
                    ) = old;
                    return Err(SettingError::Config {
                        id,
                        source: Box::new(e),
                    });
                }
            }
        }
        Ok(Applied::Live)
    }
}

fn run_sampled<T: StreamSample>(io: &mut Io, f: impl FnOnce(&[T], &mut Vec<T>)) {
    let x = io.samples::<T>(0);
    f(x, io.samples_out::<T>(0))
}

/// Runs `engine` from audio input 0 into audio output 0.
fn run_audio(io: &mut Io, engine: &mut dyn AudioEngine) {
    let x = &io.audio(0).samples;
    engine.run(x, &mut io.audio_out(0).samples);
}

/// Decimator (`dsp.decimator`) on `Iq`, `Real` or `Audio`: keeps one item in `factor`, after a multi-stage anti-alias filter flat to
/// `pass` times the output rate (sdrkit-algo-dsp `Decimator`). Exactly one output per `factor` inputs, counted across calls.
///
/// Settings: `factor` (1 to 1024; changes the rate), `pass` (fraction of the output rate, 0.05 to 0.45, default 0.4).
pub struct Decimate {
    factor: usize,
    pass: f64,
    engine: PerKind<Decimator<Complex32>, Decimator<f32>>,
}

impl Default for Decimate {
    fn default() -> Self {
        Decimate {
            factor: FACTOR.default as usize,
            pass: PASS.default,
            engine: PerKind::None,
        }
    }
}

impl Decimate {
    /// Decimation by `factor`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `factor` setting refuses `factor`.
    pub fn new(factor: usize) -> Result<Self, SettingError> {
        let as_i64 = i64::try_from(factor).unwrap_or(i64::MAX);
        DecimateSetting::Factor.check(&Value::I64(as_i64))?;
        Ok(Decimate {
            factor,
            ..Self::default()
        })
    }
}

impl Component for Decimate {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.decimator",
            vec![Port::one_of("in", &SAMPLES)],
            vec![Port::same_as("out", 0)
                .rate_from(RateFrom::DividedBySetting {
                    input: 0,
                    by: DecimateSetting::Factor.id(),
                })
                .channels_from(ChannelsFrom::Input(0))],
        )
        .with_settings(DecimateSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (DecimateSetting::Factor, Value::I64(self.factor as i64)),
            (DecimateSetting::Pass, Value::F64(self.pass)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        let out = f.rate / self.factor as f64;
        let pass = self.pass * out;
        self.engine = match (self.factor, &f.kind) {
            (1, _) => PerKind::None,
            (d, Kind::Iq) => PerKind::Iq(Decimator::new(f.rate, d, pass, 80.0)),
            (d, Kind::Real) => PerKind::Real(Decimator::new(f.rate, d, pass, 80.0)),
            (d, _) => PerKind::Audio(
                audio_engine!(f.channels, T => Ok(Decimator::<T>::new(f.rate, d, pass, 80.0)))
                    .map_err(dsp)?,
            ),
        };
        let bw = if f.kind == Kind::Iq { 2.0 * pass } else { pass };
        Ok(vec![Format {
            rate: out,
            bandwidth_hz: f.bandwidth_hz.min(bw),
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        match &mut self.engine {
            PerKind::Iq(d) => run_sampled(io, |x, o| d.process(x, o)),
            PerKind::Real(d) => run_sampled(io, |x, o| d.process(x, o)),
            PerKind::Audio(d) => run_audio(io, d.as_mut()),
            PerKind::None => match &io.input_format(0).kind {
                Kind::Iq => drop(io.pass_through::<Complex32>(0, 0)),
                Kind::Real => drop(io.pass_through::<f32>(0, 0)),
                _ => drop(io.audio_pass_through(0, 0)),
            },
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DecimateSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DecimateSetting::Factor => self.factor = FACTOR.read(id, v)?.max(1) as usize,
            DecimateSetting::Pass => self.pass = PASS.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

type Resamplers = PerKind<Resampler<Complex32>, Resampler<f32>>;

/// Resampler (`dsp.resampler`) on `Iq`, `Real` or `Audio`, to a set rate (sdrkit-algo-dsp `Resampler`: integer decimation where it can, then
/// a fractional stage; flat to 0.42 of the lower rate, 80 dB).
///
/// Settings: `rate` (Hz; changes the rate).
pub struct Resample {
    rate: f64,
    engine: Resamplers,
    latency: crate::component::Latency,
}

impl Default for Resample {
    fn default() -> Self {
        Resample {
            rate: RESAMPLE_RATE.default,
            engine: PerKind::None,
            latency: crate::component::Latency::default(),
        }
    }
}

impl Resample {
    /// To `rate_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `rate` setting refuses `rate_hz`.
    pub fn new(rate_hz: f64) -> Result<Self, SettingError> {
        ResampleSetting::Rate.check(&Value::Hz(rate_hz))?;
        Ok(Resample {
            rate: rate_hz,
            ..Self::default()
        })
    }
}

fn resampler_for(f: &Format, out_rate: f64) -> Result<Resamplers, ConfigError> {
    let in_rate = f.rate;
    Ok(match f.kind {
        Kind::Iq => PerKind::Iq(Resampler::new(in_rate, out_rate).map_err(dsp)?),
        Kind::Real => PerKind::Real(Resampler::new(in_rate, out_rate).map_err(dsp)?),
        _ => PerKind::Audio(
            audio_engine!(f.channels, T => Resampler::<T>::new(in_rate, out_rate)).map_err(dsp)?,
        ),
    })
}

/// The latency of a [`Resampler`] from `in_rate` to `out_rate`: the delay of its filters, and its first output with full
/// filters (after twice the delay of input).
pub(crate) fn resampler_latency(
    in_rate: f64,
    out_rate: f64,
) -> Result<crate::component::Latency, ConfigError> {
    let delay = Resampler::<f32>::new(in_rate, out_rate)
        .map_err(dsp)?
        .delay();
    Ok(crate::component::Latency {
        group_delay_s: delay / in_rate,
        first_valid: (2.0 * delay * out_rate / in_rate).ceil() as u64,
    })
}

impl Component for Resample {
    fn latency(&self) -> crate::component::Latency {
        self.latency
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.resampler",
            vec![Port::one_of("in", &SAMPLES)],
            vec![Port::same_as("out", 0)
                .rate_from(RateFrom::Setting(ResampleSetting::Rate.id()))
                .channels_from(ChannelsFrom::Input(0))],
        )
        .with_settings(ResampleSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(ResampleSetting::Rate, Value::Hz(self.rate))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.engine = resampler_for(f, self.rate)?;
        self.latency = resampler_latency(f.rate, self.rate)?;
        let limit = 0.42 * f.rate.min(self.rate) * if f.kind == Kind::Iq { 2.0 } else { 1.0 };
        Ok(vec![Format {
            rate: self.rate,
            bandwidth_hz: f.bandwidth_hz.min(limit),
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        match &mut self.engine {
            PerKind::Iq(d) => run_sampled(io, |x, o| d.process(x, o)),
            PerKind::Real(d) => run_sampled(io, |x, o| d.process(x, o)),
            PerKind::Audio(d) => run_audio(io, d.as_mut()),
            PerKind::None => {}
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ResampleSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ResampleSetting::Rate => self.rate = RESAMPLE_RATE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
}

/// Channel (`dsp.channel`): takes one signal out of a wide `Iq` stream. It shifts the signal to 0 Hz, decimates it (sdrkit-algo-dsp
/// `Channelizer`) and applies a sharp channel filter at the output rate.
///
/// The signal is at the input's `tuned_hz - centre_hz` plus `offset`; `Retuned` tags move the shift at the exact sample, phase
/// continuous, and are passed on. The output rate is the input rate divided by the largest integer that keeps it at or above
/// `rate` and 1.25 times the widest filter that the input keeps (`exact` resamples to `rate` exactly). The input rate must be at
/// least 1.25 times the width in use. A filter wider than the input rate leaves is refused. The filter in use (`filter`:
/// `narrow`, `normal`, `wide` or `custom`, with widths `narrow`, `normal`, `wide` and `custom` in Hz) changes live without
/// touching the rate. Changing a width or the rate reformats.
pub struct Channel {
    offset: f64,
    filter: ChannelFilter,
    widths: [f64; 4],
    rate: f64,
    exact: bool,
    chan: Option<Channelizer>,
    sharp: Option<Fir<Complex32>>,
    out_rate: f64,
    signal: f64,
    /// The width the channelizer keeps: the widest filter, or less when the input rate is low.
    kept: f64,
    carry: TagCarry,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            offset: OFFSET.default,
            filter: FILTER.default,
            widths: [NARROW.default, NORMAL.default, WIDE.default, CUSTOM.default],
            rate: CHANNEL_RATE.default,
            exact: EXACT.default,
            chan: None,
            sharp: None,
            out_rate: 0.0,
            signal: 0.0,
            kept: 0.0,
            carry: TagCarry::default(),
        }
    }
}

impl Channel {
    /// A channel `width_hz` wide (the `normal` filter; `narrow` is half of it, `wide` twice, `custom` the same) at an output rate of
    /// at least `rate_hz`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if a width setting refuses its part of `width_hz`, or the `rate` setting refuses `rate_hz`.
    pub fn new(width_hz: f64, rate_hz: f64) -> Result<Self, SettingError> {
        let widths = [width_hz / 2.0, width_hz, width_hz * 2.0, width_hz];
        let settings = [
            ChannelSetting::Narrow,
            ChannelSetting::Normal,
            ChannelSetting::Wide,
            ChannelSetting::Custom,
        ];
        for (s, w) in settings.into_iter().zip(widths) {
            s.check(&Value::Hz(w))?;
        }
        ChannelSetting::Rate.check(&Value::Hz(rate_hz))?;
        Ok(Channel {
            widths,
            rate: rate_hz,
            ..Self::default()
        })
    }
    /// With the signal `offset_hz` from the tuned frequency.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `offset` setting refuses `offset_hz`.
    pub fn with_offset(mut self, offset_hz: f64) -> Result<Self, SettingError> {
        ChannelSetting::Offset.check(&Value::Hz(offset_hz))?;
        self.offset = offset_hz;
        Ok(self)
    }
    /// The width in use.
    pub fn width(&self) -> f64 {
        use sdrkit_core::Choice;
        self.widths[usize::from(self.filter.index())]
    }
    fn widest(&self) -> f64 {
        self.widths.iter().copied().fold(0.0, f64::max)
    }
    fn sharp_taps(&self) -> Result<Vec<f32>, sdrkit_algo_dsp::DspError> {
        let pass = self.width() / 2.0;
        let stop = (pass * 1.25)
            .min(self.out_rate / 2.0 * 0.999)
            .max(pass * 1.01);
        fir::lowpass(self.out_rate, pass, stop, 70.0)
    }
}

impl Component for Channel {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("dsp.channel", vec![Port::iq("in")], vec![Port::iq("out")])
            .with_settings(ChannelSetting::schema())
            .manual_tags()
    }
    fn values(&self) -> Values {
        values_of([
            (ChannelSetting::Narrow, Value::Hz(self.widths[0])),
            (ChannelSetting::Normal, Value::Hz(self.widths[1])),
            (ChannelSetting::Wide, Value::Hz(self.widths[2])),
            (ChannelSetting::Custom, Value::Hz(self.widths[3])),
            (ChannelSetting::Offset, Value::Hz(self.offset)),
            (
                ChannelSetting::Filter,
                Value::Choice(sdrkit_core::Choice::index(self.filter)),
            ),
            (ChannelSetting::Rate, Value::Hz(self.rate)),
            (ChannelSetting::Exact, Value::Bool(self.exact)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.signal = f.signal_offset_hz() + self.offset;
        // The width in use and its transition band must fit. The wider filters keep what the rate leaves.
        let need = 1.25 * self.width();
        if f.rate < need {
            return Err(ConfigError::RateTooLow { need, got: f.rate });
        }
        self.kept = self.widest().min(f.rate / 1.25);
        let mut cfg = ChannelConfig::new(f.rate, self.signal, self.kept, self.rate);
        cfg.exact_rate = self.exact;
        let chan = Channelizer::new(cfg).map_err(dsp)?;
        self.out_rate = chan.output_rate();
        self.chan = Some(chan);
        self.sharp = Some(Fir::new(&self.sharp_taps().map_err(dsp)?));
        self.carry = TagCarry::new(f.rate, self.out_rate);
        let centre = f.tuned_hz + self.offset;
        Ok(vec![Format {
            rate: self.out_rate,
            centre_hz: centre,
            tuned_hz: centre,
            bandwidth_hz: self.width(),
            ..f.clone()
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let io_iq = io.iq(0);
        let (input, out) = io.pair(0, 0);
        let (Some(chan), Some(sharp)) = (&mut self.chan, &mut self.sharp) else {
            return;
        };
        let x = io_iq;
        let v = Complex32::vec_mut(&mut out.data).expect("iq output");
        let start = v.len();
        // The shift moves at the exact sample of each Retuned tag (the channelizer's oscillator follows the stream's sample count, so
        // the cuts change nothing).
        let mut done = 0usize;
        for t in input.tags() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = t.kind
            {
                let at = ((t.at - input.first()) as usize).min(x.len());
                chan.process(&x[done..at], v);
                done = at;
                self.signal = tuned_hz - centre_hz + self.offset;
                chan.set_offset(self.signal);
            }
        }
        chan.process(&x[done..], v);
        sharp.process_in_place(&mut v[start..]);
        let off = self.offset;
        self.carry.forward(input, out);
        for t in out.tags_mut() {
            if let TagKind::Retuned {
                centre_hz,
                tuned_hz,
            } = &mut t.kind
            {
                *tuned_hz += off;
                *centre_hz = *tuned_hz;
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let mut width = |i: usize, s: &Setting<field::Hz>| -> Result<Applied, SettingError> {
            self.widths[i] = s.read(id, v)?;
            Ok(Applied::Reformat)
        };
        match ChannelSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ChannelSetting::Narrow => width(0, &NARROW),
            ChannelSetting::Normal => width(1, &NORMAL),
            ChannelSetting::Wide => width(2, &WIDE),
            ChannelSetting::Custom => width(3, &CUSTOM),
            ChannelSetting::Offset => {
                let new = OFFSET.read(id, v)?;
                self.signal += new - self.offset;
                self.offset = new;
                if let Some(c) = &mut self.chan {
                    c.set_offset(self.signal);
                }
                Ok(Applied::Reformat)
            }
            ChannelSetting::Filter => {
                let old = std::mem::replace(&mut self.filter, FILTER.read(id, v)?);
                if self.chan.is_some() {
                    if self.width() >= self.out_rate || self.width() > self.kept {
                        self.filter = old;
                        return Err(SettingError::NotPermitted { id });
                    }
                    let Ok(taps) = self.sharp_taps() else {
                        self.filter = old;
                        return Err(SettingError::NotPermitted { id });
                    };
                    if let Some(s) = &mut self.sharp {
                        s.set_taps(&taps);
                    }
                }
                Ok(Applied::Live)
            }
            ChannelSetting::Rate => {
                self.rate = CHANNEL_RATE.read(id, v)?;
                Ok(Applied::Reformat)
            }
            ChannelSetting::Exact => {
                self.exact = EXACT.read(id, v)?;
                Ok(Applied::Reformat)
            }
        }
    }
}

/// De-emphasis (`dsp.deemphasis`) on `Real` or `Audio` (each channel): the FM de-emphasis curve `1 / (1 + j 2 pi f tau)` (sdrkit-algo-dsp
/// `Deemphasis`).
///
/// Settings: `deemphasis_us` (the time constant tau in µs, 0 to 750; 0 is off and passes the signal through; live).
pub struct DeemphasisComponent {
    tau_us: f64,
    rate: f64,
    channels: u16,
    /// One filter per channel, when on.
    filters: Vec<Deemphasis>,
}

impl Default for DeemphasisComponent {
    fn default() -> Self {
        DeemphasisComponent {
            tau_us: DEEMPHASIS_US.default,
            rate: 0.0,
            channels: 1,
            filters: Vec::new(),
        }
    }
}

impl DeemphasisComponent {
    /// With the time constant `tau_us` (µs; 0 or less is off).
    pub fn new(tau_us: f64) -> Self {
        DeemphasisComponent {
            tau_us,
            ..Self::default()
        }
    }
    fn build(&mut self) {
        self.filters.clear();
        if self.tau_us > 0.0 && self.rate > 0.0 {
            let d = Deemphasis::new(self.tau_us * 1e-6, self.rate);
            self.filters.resize(usize::from(self.channels.max(1)), d);
        }
    }
}

impl Component for DeemphasisComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.deemphasis",
            vec![Port::one_of("in", &[Kind::Real, Kind::Audio])],
            vec![Port::same_as("out", 0).same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(DeemphasisSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([(DeemphasisSetting::TimeConstant, Value::F64(self.tau_us))])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.channels = inputs[0].channels;
        self.build();
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = if io.input_format(0).kind == Kind::Real {
            io.pass_through::<f32>(0, 0)
        } else {
            io.audio_pass_through(0, 0)
        };
        match self.filters.as_mut_slice() {
            [] => {}
            [one] => one.process(buf),
            many => {
                for frame in buf.chunks_exact_mut(many.len()) {
                    for (s, d) in frame.iter_mut().zip(many.iter_mut()) {
                        *s = d.process_sample(*s);
                    }
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DeemphasisSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DeemphasisSetting::TimeConstant => self.tau_us = DEEMPHASIS_US.read(id, v)?,
        }
        self.build();
        Ok(Applied::Live)
    }
}

/// Biquad (`dsp.biquad`) on `Real` or `Audio` (each channel): one RBJ second-order section (sdrkit-algo-dsp `BiquadCoeffs`).
///
/// Settings (live): `type` (`lowpass`, `highpass`, `bandpass`, `notch`, `allpass`, `peaking`, `lowshelf`, `highshelf`), `freq` (Hz),
/// `q`, `gain` (dB, for peaking and shelves).
pub struct BiquadComponent {
    shape: BiquadShape,
    freq: f64,
    q: f64,
    gain_db: f64,
    rate: f64,
    /// One section per channel.
    filters: Vec<Biquad>,
}

impl Default for BiquadComponent {
    fn default() -> Self {
        BiquadComponent {
            shape: SHAPE.default,
            freq: BQ_FREQ.default,
            q: Q.default,
            gain_db: GAIN.default,
            rate: 0.0,
            filters: Vec::new(),
        }
    }
}

impl BiquadComponent {
    /// A section of `shape` at `freq_hz` with quality `q`.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the `freq` or `q` setting refuses its value.
    pub fn new(shape: BiquadShape, freq_hz: f64, q: f64) -> Result<Self, SettingError> {
        BiquadSetting::Freq.check(&Value::Hz(freq_hz))?;
        BiquadSetting::Q.check(&Value::F64(q))?;
        Ok(BiquadComponent {
            shape,
            freq: freq_hz,
            q,
            ..Self::default()
        })
    }
    fn coeffs(&self) -> Option<BiquadCoeffs> {
        let (r, f, q, g) = (self.rate, self.freq, self.q, self.gain_db);
        if !(f > 0.0 && f < r / 2.0 && q > 0.0) {
            return None;
        }
        Some(match self.shape {
            BiquadShape::Lowpass => BiquadCoeffs::lowpass(r, f, q),
            BiquadShape::Highpass => BiquadCoeffs::highpass(r, f, q),
            BiquadShape::Bandpass => BiquadCoeffs::bandpass(r, f, q),
            BiquadShape::Notch => BiquadCoeffs::notch(r, f, q),
            BiquadShape::Allpass => BiquadCoeffs::allpass(r, f, q),
            BiquadShape::Peaking => BiquadCoeffs::peaking(r, f, q, g),
            BiquadShape::LowShelf => BiquadCoeffs::low_shelf(r, f, q, g),
            BiquadShape::HighShelf => BiquadCoeffs::high_shelf(r, f, q, g),
        })
    }
}

impl Component for BiquadComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dsp.biquad",
            vec![Port::one_of("in", &[Kind::Real, Kind::Audio])],
            vec![Port::same_as("out", 0).same_format_as(0)],
        )
        .in_place(0, 0)
        .with_settings(BiquadSetting::schema())
    }
    fn values(&self) -> Values {
        values_of([
            (
                BiquadSetting::Shape,
                Value::Choice(sdrkit_core::Choice::index(self.shape)),
            ),
            (BiquadSetting::Freq, Value::Hz(self.freq)),
            (BiquadSetting::Q, Value::F64(self.q)),
            (BiquadSetting::Gain, Value::F64(self.gain_db)),
        ])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        let c = self.coeffs().ok_or(ConfigError::SettingAtRate {
            id: BiquadSetting::Freq.id(),
            rate_hz: self.rate,
        })?;
        self.filters.clear();
        self.filters
            .resize_with(usize::from(inputs[0].channels.max(1)), || Biquad::new(c));
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let buf = if io.input_format(0).kind == Kind::Real {
            io.pass_through::<f32>(0, 0)
        } else {
            io.audio_pass_through(0, 0)
        };
        match self.filters.as_mut_slice() {
            [] => {}
            [one] => one.process(buf),
            many => {
                for frame in buf.chunks_exact_mut(many.len()) {
                    for (s, b) in frame.iter_mut().zip(many.iter_mut()) {
                        *s = b.process_sample(*s);
                    }
                }
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let old = (self.shape, self.freq, self.q, self.gain_db);
        match BiquadSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BiquadSetting::Shape => self.shape = SHAPE.read(id, v)?,
            BiquadSetting::Freq => self.freq = BQ_FREQ.read(id, v)?,
            BiquadSetting::Q => self.q = Q.read(id, v)?,
            BiquadSetting::Gain => self.gain_db = GAIN.read(id, v)?,
        }
        if self.rate > 0.0 {
            match self.coeffs() {
                Some(c) => {
                    for b in &mut self.filters {
                        b.set_coeffs(c);
                    }
                }
                None => {
                    (self.shape, self.freq, self.q, self.gain_db) = old;
                    return Err(SettingError::Config {
                        id,
                        source: Box::new(ConfigError::SettingAtRate {
                            id: BiquadSetting::Freq.id(),
                            rate_hz: self.rate,
                        }),
                    });
                }
            }
        }
        Ok(Applied::Live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::testing::Harness;
    use crate::stream::Data;

    fn real_amp(x: &[f32], f: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let p = std::f64::consts::TAU * f * i as f64;
            re += *v as f64 * p.cos();
            im += *v as f64 * p.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f64
    }

    fn run_real(c: &mut dyn Component, x: &[f32], rate: f64) -> Vec<f32> {
        let mut h = Harness::new(c, vec![Format::new(Kind::Real, rate)]);
        let mut out = Vec::new();
        for chunk in x.chunks(1000) {
            h.run(c, |ins| {
                if let Data::Real(v) = &mut ins[0].data {
                    v.extend_from_slice(chunk)
                }
            });
            if let Data::Real(v) = &h.outs[0].data {
                out.extend_from_slice(v)
            }
        }
        out
    }

    fn rtone(n: usize, f: f64) -> Vec<f32> {
        (0..n)
            .map(|i| (std::f64::consts::TAU * f * i as f64).cos() as f32)
            .collect()
    }

    #[test]
    fn a_lowpass_fir_passes_low_and_stops_high() {
        let y = run_real(
            &mut FirFilter::lowpass(3000.0, 1000.0).unwrap(),
            &rtone(48000, 1000.0 / 48e3),
            48e3,
        );
        assert!((real_amp(&y[4800..], 1000.0 / 48e3) - 1.0).abs() < 0.02);
        let y = run_real(
            &mut FirFilter::lowpass(3000.0, 1000.0).unwrap(),
            &rtone(48000, 8000.0 / 48e3),
            48e3,
        );
        assert!(real_amp(&y[4800..], 8000.0 / 48e3) < 2e-3);
    }

    #[test]
    fn a_bandpass_fir_on_real_passes_its_band() {
        let mut f = FirFilter::bandpass(2000.0, 4000.0, 500.0).unwrap();
        let y = run_real(&mut f, &rtone(48000, 3000.0 / 48e3), 48e3);
        assert!((real_amp(&y[4800..], 3000.0 / 48e3) - 1.0).abs() < 0.02);
        let y = run_real(
            &mut FirFilter::bandpass(2000.0, 4000.0, 500.0).unwrap(),
            &rtone(48000, 500.0 / 48e3),
            48e3,
        );
        assert!(real_amp(&y[4800..], 500.0 / 48e3) < 2e-3);
    }

    #[test]
    fn deemphasis_follows_the_curve_of_its_time_constant_and_zero_is_off() {
        for (tau_us, corner) in [(50.0, 3183.1), (75.0, 2122.1)] {
            let x = rtone(48000, corner / 48e3); // the corner: -3 dB
            let y = run_real(&mut DeemphasisComponent::new(tau_us), &x, 48e3);
            let db = 20.0 * real_amp(&y[4800..], corner / 48e3).log10();
            assert!((db + 3.0).abs() < 0.3, "{tau_us} µs: {db} dB");
        }
        let x = rtone(48000, 3183.1 / 48e3);
        let y = run_real(&mut DeemphasisComponent::new(0.0), &x, 48e3);
        assert_eq!(y, x);
    }

    #[test]
    fn a_biquad_notch_removes_its_frequency() {
        let y = run_real(
            &mut BiquadComponent::new(BiquadShape::Notch, 1000.0, 2.0).unwrap(),
            &rtone(48000, 1000.0 / 48e3),
            48e3,
        );
        assert!(real_amp(&y[24000..], 1000.0 / 48e3) < 0.01);
    }

    #[test]
    fn decimation_by_four_keeps_one_in_four_and_a_low_tone() {
        let x = rtone(40_000, 1000.0 / 48e3);
        let mut d = Decimate::new(4).unwrap();
        let y = run_real(&mut d, &x, 48e3);
        assert_eq!(y.len(), 10_000);
        assert!((real_amp(&y[1000..], 1000.0 / 12e3) - 1.0).abs() < 0.01);
    }
}
