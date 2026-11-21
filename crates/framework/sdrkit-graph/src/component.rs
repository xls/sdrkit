//! The component contract: [`Component`], what it declares ([`ComponentInfo`], [`Port`]), what it sees when it runs ([`Io`]), and the
//! tag helper ([`TagCarry`]).

use crate::media::MediaSpec;
use crate::stream::{
    AudioFrames, BitOrder, Buffer, Data, Format, Frames, Item, Kind, Packet, SampleType, Tag,
    TagKind, Timed,
};
use sdrkit_core::{
    CommandError, CommandId, Commands, Complex32, ConfigError, Event, Record, Results, Schema,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Value, ValueKind, Values,
};
use std::num::NonZeroU32;
use std::sync::{Arc, LazyLock, OnceLock};

/// What a component is for. The graph checks it: a generator has no inputs, a sink has no outputs, a processor has both.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Outputs only: a radio feed, a file, a test signal.
    Generator,
    /// Inputs and outputs.
    Processor,
    /// Inputs only: audio out, results, a recorder, media out.
    Sink,
}

/// The stream kinds a port carries or accepts.
#[derive(Clone, Debug, PartialEq)]
pub enum KindRule {
    /// Exactly this kind.
    Is(Kind),
    /// An input that accepts any of these kinds and works on each the same way: a generic filter or a meter, which no adapter
    /// can replace (the graph never turns `Iq` into `Real` by itself). A component that needs one kind declares it and lets the
    /// graph adapt.
    OneOf(Vec<Kind>),
    /// An input that accepts anything: a sink that only counts.
    Any,
    /// An output of the same kind as input `n` (a generic filter or resampler): the rule that derives its kind.
    SameAs(usize),
}

impl KindRule {
    /// Whether an input of this rule accepts `kind`.
    pub fn accepts(&self, kind: &Kind) -> bool {
        match self {
            KindRule::Is(k) => k == kind,
            KindRule::OneOf(ks) => ks.contains(kind),
            KindRule::Any => true,
            KindRule::SameAs(_) => false,
        }
    }
}

/// The sample rates an input accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RateRule {
    /// Any rate.
    #[default]
    Any,
    /// Exactly this rate, in Hz.
    Exact(f64),
    /// This rate in Hz or more.
    AtLeast(f64),
}

impl RateRule {
    /// Whether a stream at `rate` Hz fits. A stream without a rate (0) fits only [`RateRule::Any`].
    pub fn accepts(&self, rate: f64) -> bool {
        match *self {
            RateRule::Any => true,
            RateRule::Exact(r) => rate == r,
            RateRule::AtLeast(r) => rate >= r,
        }
    }
}

/// The channel counts an input accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelRule {
    /// Any number of channels.
    #[default]
    Any,
    /// Exactly this many.
    Exact(u16),
}

impl ChannelRule {
    /// Whether a stream of `channels` channels fits.
    pub fn accepts(&self, channels: u16) -> bool {
        match *self {
            ChannelRule::Any => true,
            ChannelRule::Exact(n) => channels == n,
        }
    }
}

/// The formats a port accepts (an input) or produces (an output). An input's rule is checked at connect for the kind, the
/// item type and the codec, and at configure for the rate, the channels and the sample type. An output states its kind, or
/// the input it takes its kind from.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatConstraint {
    /// The kinds. For a typed stream the kind holds the item type.
    pub kind: KindRule,
    /// The sample rates.
    pub rate: RateRule,
    /// The channel counts.
    pub channels: ChannelRule,
    /// The sample type, when the input takes only one.
    pub sample_type: Option<SampleType>,
    /// For a `Bytes` port of compressed media or pictures: the codecs (and packagings) it produces (an output, when known before
    /// the stream starts) or accepts (an input). Empty for any. [`Codec::Pixels`](crate::Codec) names a pixel layout.
    pub media: Vec<MediaSpec>,
    /// For an `Iq` input: the radio frequency that must sit at 0 Hz. `None` for any.
    pub centre: Option<CentreRule>,
    /// For an `Iq` input: the two-sided width of the channel it needs around its centre, in Hz. With [`RateRule::Any`] the
    /// input takes a rate from the width up to twice the width, and the graph changes a rate outside that.
    pub width: Option<f64>,
    /// For an `Iq` input that the graph makes from a real stream: the lowest frequency of the real signal that the input
    /// needs, in Hz. It sets the transition band of the analytic signal. `None`: 2.5 % of the rate.
    pub lowest_hz: Option<f64>,
    /// For a [`Kind::Llr`] input: the scale it needs ([`Format::llr_scale`]). `None` for any.
    pub llr_scale: Option<f32>,
    /// For a `Bytes` input of packed bits: the order of the bits in a byte. `None` for plain bytes.
    pub bit_order: Option<BitOrder>,
    /// The input takes a declared-lossy conversion (a downmix, a hard decision) when the graph needs one to join its producer.
    pub lossy: bool,
    /// For a `Spectrum` input: the spectrum it needs. The graph makes it from the producer once, and every input that requests
    /// for the same spectrum of the same port reads it.
    pub spectrum: Option<SpectrumSpec>,
}

/// The radio frequency that an `Iq` input needs at 0 Hz ([`FormatConstraint::centre`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CentreRule {
    /// This frequency, in Hz. A frequency outside the capture fails at validate.
    Fixed(f64),
    /// This frequency, in Hz, which can lie outside the capture. Then the input reads silence (zeros at its rate).
    MayLeave(f64),
    /// The tuned frequency. The shift that the graph inserts follows each retune at its sample, with no new validate.
    Tuned,
}

impl CentreRule {
    /// Whether a stream of format `f` has the centre this rule needs.
    pub fn fits(self, f: &Format) -> bool {
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-6 * b.abs().max(1.0);
        match self {
            CentreRule::Fixed(c) | CentreRule::MayLeave(c) => near(f.centre_hz, c),
            CentreRule::Tuned => near(f.centre_hz, f.tuned_hz),
        }
    }
}

/// The parameters of a spectrum that an input requires ([`FormatConstraint::spectrum`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectrumSpec {
    /// The bins per frame.
    pub size: SizeRule,
    /// The window.
    pub window: crate::components::SpectrumWindow,
    /// The frames averaged, 1 or more.
    pub average: u32,
    /// When frames start.
    pub frames: FrameRule,
    /// dB or linear power.
    pub scale: crate::components::SpectrumScale,
}

/// How many bins a spectrum frame has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SizeRule {
    /// This size.
    Bins(crate::components::SpectrumSize),
    /// The size whose frame lasts nearest to this many seconds at the rate of the source.
    Seconds(f64),
}

impl SizeRule {
    /// The size for a source of `rate` samples per second.
    pub fn size(self, rate: f64) -> crate::components::SpectrumSize {
        match self {
            SizeRule::Bins(s) => s,
            SizeRule::Seconds(t) => crate::components::SpectrumSize::about(rate, t),
        }
    }
}

/// When the frames of a spectrum start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameRule {
    /// This many frames per second.
    PerSecond(f64),
    /// One frame after the other at the rate of the source: `rate / bins` frames per second.
    BackToBack,
}

impl SpectrumSpec {
    /// Whether a spectrum stream of format `f` is this spectrum. Its `bandwidth_hz` is the rate of its source, or half that
    /// rate for a one-sided spectrum.
    fn fits(&self, f: &Format) -> bool {
        let sides = if f.one_sided { 2 } else { 1 };
        let source_hz = f.bandwidth_hz * sides as f64;
        let bins = self.size.size(source_hz).bins();
        let rate = match self.frames {
            FrameRule::PerSecond(r) => r,
            FrameRule::BackToBack => source_hz / bins as f64,
        };
        f.frame_len * sides == bins
            && f.scale == Some(self.scale)
            && (f.rate - rate).abs() <= 1e-9 * rate.abs().max(1.0)
    }
}

impl FormatConstraint {
    /// The rule for `kind`, with any rate, any channel count and any sample type.
    pub fn of(kind: KindRule) -> Self {
        FormatConstraint {
            kind,
            rate: RateRule::Any,
            channels: ChannelRule::Any,
            sample_type: None,
            media: Vec::new(),
            centre: None,
            width: None,
            lowest_hz: None,
            llr_scale: None,
            bit_order: None,
            lossy: false,
            spectrum: None,
        }
    }

    /// Whether a stream of format `f` fits this rule as it is: the kind, the rate, the width, the channels, the sample type,
    /// the centre, the codec or pixel layout, the LLR scale, the bit order and the spectrum. A stream whose codec is not known
    /// fits any codec.
    pub fn accepts_format(&self, f: &Format) -> bool {
        self.kind.accepts(&f.kind)
            && self.rate.accepts(f.rate)
            && self.width_accepts(f.rate)
            && self.channels.accepts(f.channels)
            && self.sample_type.is_none_or(|t| f.sample_type == Some(t))
            && self.centre.is_none_or(|c| c.fits(f))
            && (self.media.is_empty()
                || f.media
                    .as_ref()
                    .is_none_or(|m| self.media.iter().any(|a| m.matches(a))))
            && self
                .llr_scale
                .is_none_or(|s| f.llr_scale.is_some_and(|g| (g - s).abs() <= 1e-6 * s.abs()))
            && self.bit_order.is_none_or(|o| f.bit_order == Some(o))
            && self.spectrum.is_none_or(|s| s.fits(f))
    }

    /// Whether `rate` fits the width rule: from the width up to twice the width, when the rate rule takes any rate.
    pub(crate) fn width_accepts(&self, rate: f64) -> bool {
        match (self.width, self.rate) {
            (Some(w), RateRule::Any) => rate >= w && rate < 2.0 * w,
            _ => true,
        }
    }
}

/// How the rate of an output follows from the inputs and the settings of its component. A number in a rule is a constant of the
/// component kind. A rate that a setting changes names that setting, so the graph always reads its present value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RateFrom {
    /// [`Component::configure`] sets it. The rate is known only after configure.
    #[default]
    Configure,
    /// The rate of input `n`.
    Input(usize),
    /// The rate of input `input` divided by the constant `by`.
    Divided {
        /// The input.
        input: usize,
        /// The divisor.
        by: NonZeroU32,
    },
    /// The rate of input `input` divided by the present value of the whole-number setting `by`. Its range must start at 1 or more.
    DividedBySetting {
        /// The input.
        input: usize,
        /// The setting that holds the divisor.
        by: SettingId,
    },
    /// This rate in Hz. 0 for a stream without a rate.
    Fixed(f64),
    /// The present value of this number setting of the component, in Hz.
    Setting(SettingId),
}

impl RateFrom {
    /// The input that the rule reads, if it reads one.
    fn input(self) -> Option<usize> {
        match self {
            RateFrom::Input(n)
            | RateFrom::Divided { input: n, .. }
            | RateFrom::DividedBySetting { input: n, .. } => Some(n),
            RateFrom::Configure | RateFrom::Fixed(_) | RateFrom::Setting(_) => None,
        }
    }
    /// The setting that the rule reads, the value kinds it accepts and the least lower limit its range must have, if the rule
    /// reads a setting.
    fn setting(self) -> Option<(SettingId, &'static [ValueKind], Option<f64>)> {
        match self {
            RateFrom::DividedBySetting { by, .. } => Some((by, &[ValueKind::I64], Some(1.0))),
            RateFrom::Setting(id) => Some((id, &[ValueKind::Hz, ValueKind::F64], None)),
            RateFrom::Configure
            | RateFrom::Input(_)
            | RateFrom::Divided { .. }
            | RateFrom::Fixed(_) => None,
        }
    }
}

/// How the channel count of an output follows from the inputs of its component.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChannelsFrom {
    /// [`Component::configure`] sets it.
    #[default]
    Configure,
    /// The channel count of input `n`.
    Input(usize),
    /// This many channels, a constant of the component kind.
    Fixed(u16),
}

/// The rule that derives the format of an output port from the inputs: its rate and its channel count. The kind and the
/// sample type come from the port's [`FormatConstraint`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OutputRule {
    /// The rate.
    pub rate: RateFrom,
    /// The channel count.
    pub channels: ChannelsFrom,
}

/// A fault in an [`OutputRule`] that a component declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuleFault {
    /// The rule reads an input that the component does not have.
    Input(usize),
    /// The rule reads a setting that the component does not declare, or one of a kind or range it cannot use.
    Setting(SettingId),
}

impl OutputRule {
    /// Makes sure that the rule reads only inputs below `inputs` and settings of `schema` of a kind and range it can use.
    pub(crate) fn check(&self, inputs: usize, schema: &Schema) -> Result<(), RuleFault> {
        let input = self.rate.input().into_iter().chain(match self.channels {
            ChannelsFrom::Input(n) => Some(n),
            ChannelsFrom::Configure | ChannelsFrom::Fixed(_) => None,
        });
        if let Some(n) = input.into_iter().find(|&n| n >= inputs) {
            return Err(RuleFault::Input(n));
        }
        match self.rate.setting() {
            Some((id, kinds, least))
                if !schema.setting(id).is_some_and(|d| {
                    kinds.contains(&d.field.kind)
                        && least.is_none_or(|m| d.range.is_some_and(|r| r.min >= m))
                }) =>
            {
                Err(RuleFault::Setting(id))
            }
            _ => Ok(()),
        }
    }

    /// The rate and the channel count that this rule produces for the input formats `inputs` and the present settings `values`.
    /// `None` where [`Component::configure`] decides. The graph has made sure that the inputs and settings the rule reads exist.
    pub fn derive(&self, inputs: &[Format], values: &Values) -> (Option<f64>, Option<u16>) {
        let rate_of = |n: usize| inputs.get(n).map(|f| f.rate);
        let rate = match self.rate {
            RateFrom::Configure => None,
            RateFrom::Input(n) => rate_of(n),
            RateFrom::Divided { input, by } => rate_of(input).map(|r| r / f64::from(by.get())),
            RateFrom::DividedBySetting { input, by } => match values.value(by) {
                Some(&Value::I64(d)) if d >= 1 => rate_of(input).map(|r| r / d as f64),
                _ => None,
            },
            RateFrom::Fixed(r) => Some(r),
            RateFrom::Setting(id) => match values.value(id) {
                Some(Value::Hz(r) | Value::F64(r)) => Some(*r),
                _ => None,
            },
        };
        let channels = match self.channels {
            ChannelsFrom::Configure => None,
            ChannelsFrom::Input(n) => inputs.get(n).map(|f| f.channels),
            ChannelsFrom::Fixed(c) => Some(c),
        };
        (rate, channels)
    }
}

/// A named port with its format rule.
#[derive(Clone, Debug, PartialEq)]
pub struct Port {
    /// The name, unique among the component's inputs (or outputs): `"in"`, `"out"`, `"guide"`. A host looks it up once to get
    /// the port's index.
    pub name: String,
    /// What it accepts (an input) or produces (an output).
    pub format: FormatConstraint,
    /// For an optional input ([`Port::optional`]): what the component does when the input is left unconnected (its documented
    /// default), in words. `None` for a required input and for every output.
    pub optional: Option<String>,
    /// For an output: the rule that derives its rate and channel count. The graph makes sure that the format which
    /// [`Component::configure`] states agrees with it. Inputs leave it at its default.
    pub output: OutputRule,
}

impl Port {
    fn with_kind(name: &str, kind: KindRule) -> Port {
        Port {
            name: name.into(),
            format: FormatConstraint::of(kind),
            optional: None,
            output: OutputRule::default(),
        }
    }
    /// A port of exactly `kind`.
    pub fn new(name: &str, kind: Kind) -> Port {
        Port::with_kind(name, KindRule::Is(kind))
    }
    /// An `Iq` port.
    pub fn iq(name: &str) -> Port {
        Port::new(name, Kind::Iq)
    }
    /// A `Real` port.
    pub fn real(name: &str) -> Port {
        Port::new(name, Kind::Real)
    }
    /// An `Audio` port.
    pub fn audio(name: &str) -> Port {
        Port::new(name, Kind::Audio)
    }
    /// A `Symbols` port.
    pub fn symbols(name: &str) -> Port {
        Port::new(name, Kind::Symbols)
    }
    /// A [`Kind::HardBits`] port (`u8` bits).
    pub fn hard_bits(name: &str) -> Port {
        Port::new(name, Kind::HardBits)
    }
    /// A [`Kind::Llr`] port (`i8` soft bits).
    pub fn llr(name: &str) -> Port {
        Port::new(name, Kind::Llr)
    }
    /// A `Bytes` port of plain byte packets (frames, not compressed media).
    pub fn bytes(name: &str) -> Port {
        Port::new(name, Kind::Bytes)
    }
    /// A control port carrying [`Command`](crate::Command)s (see [`crate::command`]).
    pub fn command(name: &str) -> Port {
        Port::typed::<crate::Command>(name)
    }
    /// A typed port carrying the Rust type `T` (see [`Item`]): `Port::typed::<EpgEvent>("guide")`.
    pub fn typed<T: Item>(name: &str) -> Port {
        Port::new(name, Kind::typed::<T>())
    }
    /// A `Bytes` port of compressed media: what an output produces, or what an input (a decoder) accepts.
    pub fn media(name: &str, specs: &[MediaSpec]) -> Port {
        let mut p = Port::new(name, Kind::Bytes);
        p.format.media = specs.to_vec();
        p
    }
    /// An input accepting any of `kinds`, for a generic filter or meter (see [`KindRule::OneOf`]).
    pub fn one_of(name: &str, kinds: &[Kind]) -> Port {
        Port::with_kind(name, KindRule::OneOf(kinds.to_vec()))
    }
    /// An input accepting any kind, for a sink that only counts.
    pub fn any(name: &str) -> Port {
        Port::with_kind(name, KindRule::Any)
    }
    /// An output of the same kind as input `input`.
    pub fn same_as(name: &str, input: usize) -> Port {
        Port::with_kind(name, KindRule::SameAs(input))
    }
    /// The same input, taking only rates that `rate` accepts.
    pub fn rate(mut self, rate: RateRule) -> Port {
        self.format.rate = rate;
        self
    }
    /// The same input, taking only channel counts that `channels` accepts.
    pub fn channels(mut self, channels: ChannelRule) -> Port {
        self.format.channels = channels;
        self
    }
    /// The same output, with its rate derived by `rate`.
    pub fn rate_from(mut self, rate: RateFrom) -> Port {
        self.output.rate = rate;
        self
    }
    /// The same output, with its channel count derived by `channels`.
    pub fn channels_from(mut self, channels: ChannelsFrom) -> Port {
        self.output.channels = channels;
        self
    }
    /// The same output, with the rate and the channel count of input `n`.
    pub fn same_format_as(self, n: usize) -> Port {
        self.rate_from(RateFrom::Input(n))
            .channels_from(ChannelsFrom::Input(n))
    }
    /// The same `Iq` input, taking only a stream with `centre_hz` at 0 Hz.
    pub fn centre(mut self, centre_hz: f64) -> Port {
        self.format.centre = Some(CentreRule::Fixed(centre_hz));
        self
    }
    /// The same `Iq` input, taking a stream with `centre_hz` at 0 Hz, and silence while `centre_hz` lies outside the capture.
    pub fn centre_may_leave(mut self, centre_hz: f64) -> Port {
        self.format.centre = Some(CentreRule::MayLeave(centre_hz));
        self
    }
    /// The same `Iq` input, taking a channel `width_hz` wide centred on the tuned frequency. The graph shifts the stream and
    /// changes its rate, and the shift follows each retune.
    pub fn tuned_channel(mut self, width_hz: f64) -> Port {
        self.format.centre = Some(CentreRule::Tuned);
        self.format.width = Some(width_hz);
        self
    }
    /// The same `Iq` input, made from a real stream whose lowest needed frequency is `hz`.
    pub fn lowest(mut self, hz: f64) -> Port {
        self.format.lowest_hz = Some(hz);
        self
    }
    /// The same `Llr` input, taking only soft bits of scale `scale` ([`Format::llr_scale`]).
    pub fn llr_scale(mut self, scale: f32) -> Port {
        self.format.llr_scale = Some(scale);
        self
    }
    /// The same `Bytes` port, of bits packed in `order`: an input takes only these, an output produces them.
    pub fn bit_order(mut self, order: BitOrder) -> Port {
        self.format.bit_order = Some(order);
        self
    }
    /// The same input, taking a declared-lossy conversion when the graph needs one.
    pub fn lossy(mut self) -> Port {
        self.format.lossy = true;
        self
    }
    /// A `Spectrum` input that needs the spectrum `spec` of the port it connects to.
    pub fn spectrum(name: &str, spec: SpectrumSpec) -> Port {
        let mut p = Port::new(name, Kind::Spectrum);
        p.format.spectrum = Some(spec);
        p
    }
    /// The same port, of samples of type `sample_type`: an input takes only these, an output produces them.
    pub fn sample_type(mut self, sample_type: SampleType) -> Port {
        self.format.sample_type = Some(sample_type);
        self
    }
    /// Makes this input optional: a graph may leave it unconnected, and the component then does what `when_unconnected` says
    /// (its documented default, for example "the noise variance of the `noise_var` setting"). An unconnected optional input holds
    /// no items and has the format `Format::new(kind, 0.0)` of its declared kind (the first, for an input of several kinds);
    /// [`Io::is_connected`] reports which case applies. An optional input must declare its kind (not
    /// [`Port::any`]) and be among the first 64 inputs. An edge that names a component without a port goes to its one required
    /// input when the others are optional.
    ///
    /// ```
    /// use sdrkit_graph::Port;
    /// let p = Port::real("noise_var").optional("the `noise_var` setting");
    /// assert_eq!(p.optional.as_deref(), Some("the `noise_var` setting"));
    /// ```
    pub fn optional(mut self, when_unconnected: &str) -> Port {
        self.optional = Some(when_unconnected.into());
        self
    }
    /// Whether this (input) port accepts `kind`.
    pub fn accepts(&self, kind: &Kind) -> bool {
        self.format.kind.accepts(kind)
    }
}

/// How the graph treats a component's tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagPolicy {
    /// The graph passes every input tag on to every output, moved to the output position by the ratio of the rates (see
    /// [`TagCarry`]). The default; right for almost every component. Tags of a fed-back input are not passed on,
    /// nor tags of an input without a rate (bytes, messages, typed items) onto an output with one.
    #[default]
    Auto,
    /// The component handles tags itself in [`Component::process`], usually with a [`TagCarry`], and may consume them.
    Manual,
}

/// What a component declares about itself.
#[derive(Clone, Debug)]
pub struct ComponentInfo {
    /// The type name, as the [`Registry`](crate::Registry) and a [`Description`](crate::Description) know it (`"channel"`).
    pub name: String,
    /// What it is for.
    pub role: Role,
    /// Input ports, in order.
    pub inputs: Vec<Port>,
    /// Output ports, in order.
    pub outputs: Vec<Port>,
    /// The settings it accepts through [`Component::apply`].
    pub settings: Arc<Schema>,
    /// How its tags are handled.
    pub tags: TagPolicy,
    /// Input and output that may share one buffer: when the graph allows it (the input feeds only this component, is not tapped and
    /// is not a graph output) the input's items are moved into the output, and [`Io::pass_through`] works on them in place, with no
    /// copy. Declare it only when the component reads that input through [`Io::pass_through`].
    pub in_place: Option<(usize, usize)>,
}

static EMPTY: OnceLock<Arc<Schema>> = OnceLock::new();

/// The shared empty schema.
pub(crate) fn no_settings() -> Arc<Schema> {
    Arc::clone(empty_schema_arc())
}

fn empty_schema_arc() -> &'static Arc<Schema> {
    EMPTY.get_or_init(|| Arc::new(Schema::default()))
}

fn empty_schema() -> &'static Schema {
    empty_schema_arc()
}

/// The values of a component without settings.
pub fn no_values() -> Values {
    Values::defaults(empty_schema_arc())
}

/// The values of the settings `S` with these present values, for [`Component::values`]. A setting left out has its default.
///
/// # Panics
///
/// In debug builds, if a value is outside the schema of its setting. A component never holds such a value, because its
/// constructor and [`Component::apply`] take only values that the schema accepts. In release builds the value is left out and
/// counted in [`sdrkit_core::record::dropped_writes`].
///
/// ```
/// use sdrkit_core::{publish::field, Setting, SettingSet, Value};
/// const RATE: Setting<field::Hz> = Setting::new("rate", "Rate", "Hz", 48e3);
/// sdrkit_core::settings! {
///     /// The settings of a resampler.
///     pub enum RateSetting {
///         /// The output rate.
///         Rate = RATE,
///     }
/// }
/// let v = sdrkit_graph::values_of([(RateSetting::Rate, Value::Hz(24e3))]);
/// assert_eq!(v.value(RateSetting::Rate.id()), Some(&Value::Hz(24e3)));
/// ```
pub fn values_of<S: SettingSet>(present: impl IntoIterator<Item = (S, Value)>) -> Values {
    let mut v = Values::defaults(&S::schema());
    for (s, x) in present {
        if v.set(s.id(), &x).is_err() {
            let key = S::SETTINGS[s.id().index()].key();
            sdrkit_core::record::refuse_write(key, "a present value that its schema refuses");
        }
    }
    v
}

/// The values of the settings `S` with these present values, for values that a host or a service provides. A setting left out
/// has its default.
///
/// # Errors
///
/// The [`SettingError`] of the first value that is outside the schema of its setting.
pub fn checked_values_of<S: SettingSet>(
    present: impl IntoIterator<Item = (S, Value)>,
) -> Result<Values, SettingError> {
    let mut v = Values::defaults(&S::schema());
    for (s, x) in present {
        v.set(s.id(), &x)?;
    }
    Ok(v)
}

impl ComponentInfo {
    /// A processor with these ports and no settings.
    pub fn processor(name: &str, inputs: Vec<Port>, outputs: Vec<Port>) -> Self {
        ComponentInfo {
            name: name.into(),
            role: Role::Processor,
            inputs,
            outputs,
            settings: no_settings(),
            tags: TagPolicy::Auto,
            in_place: None,
        }
    }
    /// A sink with these inputs and no settings.
    pub fn sink(name: &str, inputs: Vec<Port>) -> Self {
        ComponentInfo {
            name: name.into(),
            role: Role::Sink,
            inputs,
            outputs: vec![],
            settings: no_settings(),
            tags: TagPolicy::Auto,
            in_place: None,
        }
    }
    /// A generator with these outputs and no settings.
    pub fn generator(name: &str, outputs: Vec<Port>) -> Self {
        ComponentInfo {
            name: name.into(),
            role: Role::Generator,
            inputs: vec![],
            outputs,
            settings: no_settings(),
            tags: TagPolicy::Auto,
            in_place: None,
        }
    }
    /// With this settings schema (usually `MySetting::schema()` of a [`settings!`](sdrkit_core::settings!) enum).
    pub fn with_settings(mut self, settings: Arc<Schema>) -> Self {
        self.settings = settings;
        self
    }
    /// Input `i` and output `o` may share a buffer (see [`ComponentInfo::in_place`]).
    pub fn in_place(mut self, i: usize, o: usize) -> Self {
        self.in_place = Some((i, o));
        self
    }
    /// With manual tag handling.
    pub fn manual_tags(mut self) -> Self {
        self.tags = TagPolicy::Manual;
        self
    }
}

/// The delay that a component adds to the signal ([`Component::latency`]).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Latency {
    /// The group delay, in seconds: output time `t` holds the input of time `t - group_delay_s`.
    pub group_delay_s: f64,
    /// The index of the first output item whose filters are full, counted from the configure.
    pub first_valid: u64,
}

/// What a setting change needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    /// The change takes effect at once; the output formats are unchanged.
    Live,
    /// It changes the output formats (a new rate): the graph runs [`Component::configure`] again for this component and every
    /// component whose input format changes as a result.
    Reformat,
}

/// What a component keeps when the host changes the frequency: the keys of its own fields and tables that the
/// description of its stack keeps ([`StackShape::kept`](sdrkit_core::interface::StackShape::kept)).
///
/// ```
/// use sdrkit_graph::Kept;
///
/// let kept = Kept::of(&["messages"], &["log"]);
/// assert!(kept.table("log") && kept.field("messages") && !kept.field("level_db"));
/// assert!(Kept::default().is_empty());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Kept {
    fields: Vec<String>,
    tables: Vec<String>,
}

impl Kept {
    /// The fields `fields` and the tables `tables`, by the keys that the component declares.
    pub fn of(fields: &[&str], tables: &[&str]) -> Self {
        Kept {
            fields: fields.iter().map(|k| (*k).to_owned()).collect(),
            tables: tables.iter().map(|k| (*k).to_owned()).collect(),
        }
    }
    /// True if nothing of the component is kept.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.tables.is_empty()
    }
    /// True if the field `key` of the component is kept.
    pub fn field(&self, key: &str) -> bool {
        self.fields.iter().any(|k| k == key)
    }
    /// True if the table `key` of the component is kept.
    pub fn table(&self, key: &str) -> bool {
        self.tables.iter().any(|k| k == key)
    }
}

/// A unit of a graph: it does one job, has typed ports, declares its settings and may publish results.
///
/// # Example
///
/// ```
/// use sdrkit_core::publish::field;
/// use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value, Values};
/// use sdrkit_graph::{values_of, Applied, Component, ComponentInfo, Format, Io, Port};
///
/// const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);
/// sdrkit_core::settings! {
///     /// The settings of [`Gain`].
///     enum GainSetting { Gain = GAIN }
/// }
///
/// /// Multiplies a real signal by a gain.
/// struct Gain(f32);
///
/// impl Component for Gain {
///     fn info(&self) -> ComponentInfo {
///         ComponentInfo::processor("gain", vec![Port::real("in")], vec![Port::real("out")])
///             .with_settings(GainSetting::schema())
///     }
///     fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
///         Ok(vec![inputs[0].clone()])
///     }
///     fn process(&mut self, io: &mut Io) {
///         let g = self.0;
///         for s in io.pass_through::<f32>(0, 0) {
///             *s *= g;
///         }
///     }
///     fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
///         match GainSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
///             GainSetting::Gain => self.0 = GAIN.read(id, v)? as f32,
///         }
///         Ok(Applied::Live)
///     }
///     fn values(&self) -> Values {
///         values_of([(GainSetting::Gain, Value::F64(f64::from(self.0)))])
///     }
/// }
/// ```
pub trait Component: Send {
    /// The type name, role, ports, settings and tag policy. Called when the component is added to a graph.
    fn info(&self) -> ComponentInfo;
    /// Learns the formats of its inputs (in port order) and states those of its outputs. Called when the graph is configured and
    /// again after any change of rate upstream. It (re)builds the DSP and clears the state.
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError>;
    /// Consumes everything its inputs hold and appends to its outputs. It accepts any number of items (including none) and keeps its
    /// own state, so a stream cut into pieces of any size produces the same result. It never waits.
    fn process(&mut self, io: &mut Io);
    /// The input ended ([`Graph::finish`](crate::Graph::finish)). Like [`Component::process`], it consumes what its inputs
    /// hold. Then it writes to its outputs what it still holds (a frame half assembled, the samples in a filter's delay line,
    /// a batch on a helper thread) and brings its results up to date. It can wait for its helper threads, as `process` must
    /// not. The default calls `process`, so the items that the components before it flushed still pass through.
    fn finish(&mut self, io: &mut Io) {
        self.process(io)
    }
    /// Applies a new value of setting `id` (an id of [`ComponentInfo::settings`]), between two calls, on the graph's thread. The
    /// graph has passed the value through [`Schema::check`], so the component does not check the kind, the range or the choice
    /// again.
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the component refuses the value. Its state does not change. The default (for a component without
    /// settings) returns [`SettingError::NoSuchId`].
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        let _ = value;
        Err(SettingError::NoSuchId { id })
    }
    /// The present value of every setting of [`ComponentInfo::settings`], made from the state the component runs with, so it
    /// includes the values its constructor received. The graph describes itself, saves settings and shows them from these values.
    /// A component without settings returns [`no_values`].
    fn values(&self) -> Values;
    /// The delay it adds to the signal, as configured. The graph adds it up along each path ([`Format::delay_s`]) and the
    /// validate report shows it for each inserted adapter. The default is no delay.
    fn latency(&self) -> Latency {
        Latency::default()
    }
    /// What it publishes, for a component that measures or decodes.
    fn results(&self) -> Option<&Results> {
        None
    }
    /// The commands it declares. The default has none.
    fn commands(&self) -> &Commands {
        static NONE: LazyLock<Commands> = LazyLock::new(Commands::default);
        &NONE
    }
    /// Runs command `id` of [`Component::commands`], between two calls, on the graph's thread. The graph has passed the
    /// arguments through [`Commands::check`], so the component does not check them again.
    ///
    /// # Errors
    ///
    /// [`CommandError`] if the component refuses the command or it fails. The default returns [`CommandError::NoSuchId`].
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let _ = args;
        Err(CommandError::NoSuchId { id })
    }
    /// The host changed the frequency: forget every result of the frequency before, except the fields and tables of
    /// `kept`, which the description of the stack keeps. It is no command, so every component can have it. A
    /// component that holds kept and other results implements it and clears the others. The default runs the
    /// `clear` command of the component, if it has one without a required argument and `kept` is empty, and else
    /// leaves the component as it is.
    fn forget(&mut self, kept: &Kept) {
        if !kept.is_empty() {
            return;
        }
        let commands = self.commands();
        let clear = commands.id("clear").ok().and_then(|id| {
            let args = commands.check(id, &commands.def(id)?.new_args()).ok()?;
            Some((id, args))
        });
        if let Some((id, args)) = clear {
            // A component whose command fails stays as it was.
            let _ = self.invoke(id, &args);
        }
    }
    /// The graph's input frequency: `centre_hz` is the centre of its samples and `tuned_hz` where the user tuned. Called after
    /// the graph is configured and after each retune that keeps the components ([`Graph`](crate::Graph) as a
    /// [`Receiver`](sdrkit_core::Receiver)), between two passes. A component that shows frequencies (a table of channels) keeps
    /// them. The samples of the new frequency start at the next pass, marked by a `Retuned` tag. The default ignores it.
    fn retuned(&mut self, centre_hz: f64, tuned_hz: f64) {
        let _ = (centre_hz, tuned_hz);
    }
    /// Learns the run context: live or recorded samples, the station, the helper threads ([`RunContext`](crate::RunContext)).
    /// Called when the graph's context is set or changes (at configure, and when the host supplies a new station), between two calls.
    /// The default ignores it.
    fn attach(&mut self, ctx: &crate::RunContext) {
        let _ = ctx;
    }
    /// The data the component has gathered and that must persist across sessions (a list of stations heard), in its own format.
    /// `None` (the default) for nothing. The host saves it.
    fn save_state(&self) -> Option<StateBlob> {
        None
    }
    /// Restores what [`Component::save_state`] returned.
    ///
    /// # Errors
    ///
    /// [`StateError`] if the blob is of another owner or version, or damaged. Nothing changes. The default returns
    /// [`StateError::NoState`].
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let _ = state;
        Err(StateError::NoState)
    }
    /// The component as [`std::any::Any`], so the code that owns the graph can reach its concrete type between two passes
    /// ([`Graph::component_as`](crate::Graph::component_as)). The default (`None`) keeps the component closed. A component
    /// that opens itself returns `Some(self)`.
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        None
    }
    /// The component as mutable [`std::any::Any`] ([`Graph::component_as_mut`](crate::Graph::component_as_mut)). The default
    /// (`None`) keeps the component closed. A component that opens itself returns `Some(self)`.
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

/// A sample type a generic component can read and write: `Complex32` (`Iq` and `Symbols`) and `f32` (`Real`). Audio has its own
/// accessors ([`Io::audio`]), because its frames hold [`Format::channels`] samples.
pub trait StreamSample: sdrkit_algo_dsp::Sample {
    /// The samples of `data`, when it holds this type.
    fn slice(data: &Data) -> Option<&[Self]>;
    /// The vector of `data`, when it holds this type.
    fn vec_mut(data: &mut Data) -> Option<&mut Vec<Self>>;
    /// The graph's own input as this type, when it is this type (`Complex32` only).
    fn graph_input(x: &[Complex32]) -> Option<&[Self]> {
        let _ = x;
        None
    }
}

impl StreamSample for Complex32 {
    fn slice(data: &Data) -> Option<&[Self]> {
        match data {
            Data::Iq(v) | Data::Symbols(v) => Some(v),
            _ => None,
        }
    }
    fn vec_mut(data: &mut Data) -> Option<&mut Vec<Self>> {
        match data {
            Data::Iq(v) | Data::Symbols(v) => Some(v),
            _ => None,
        }
    }
    fn graph_input(x: &[Complex32]) -> Option<&[Self]> {
        Some(x)
    }
}

impl StreamSample for f32 {
    fn slice(data: &Data) -> Option<&[Self]> {
        match data {
            Data::Real(v) => Some(v),
            _ => None,
        }
    }
    fn vec_mut(data: &mut Data) -> Option<&mut Vec<Self>> {
        match data {
            Data::Real(v) => Some(v),
            _ => None,
        }
    }
}

/// What a component sees in [`Component::process`]: its input buffers (shared, read-only, valid for the whole call) and its output
/// buffers (empty at the start of the call, positioned at the right index).
///
/// Input accessors return data borrowed for the whole call, so a component can hold an input slice while it writes an output. A
/// component reads the kind it declared; reading another kind is a programming error and panics with the port's number.
pub struct Io<'a> {
    pub(crate) bufs: &'a [Buffer],
    pub(crate) formats: &'a [Format],
    pub(crate) inputs: &'a [usize],
    pub(crate) outputs: &'a [usize],
    pub(crate) outs: &'a mut [Buffer],
    pub(crate) moved: Option<(usize, usize)>,
    pub(crate) tick: bool,
    /// The graph's input samples, borrowed from the caller (stream 0 holds only their index and tags).
    pub(crate) input0: &'a [Complex32],
    /// Bit `i` set: input `i` is an optional input left unconnected.
    pub(crate) unconnected: u64,
    /// Seconds of graph input this pass holds (0 for a standalone `Io`).
    pub(crate) pass_s: f64,
    /// Set by [`Io::request_reformat`]: the graph configures the component again at the end of the pass.
    pub(crate) reformat: bool,
    /// The settings schema of the component that runs.
    pub(crate) schema: &'a Schema,
    /// The most items of a stream without a rate that one output keeps in one pass.
    pub(crate) max_items: usize,
    /// The work counters of the component that runs, while the metrics of the graph are on.
    pub(crate) meter: Option<&'a std::sync::Arc<crate::meter::Counters>>,
}

fn wrong(what: &str, port: usize, data: &Data) -> ! {
    panic!("{what} {port} does not hold this kind of item (it holds {data:?})")
}

impl<'a> Io<'a> {
    /// Builds an `Io` over loose buffers, for testing a component on its own. `formats` holds the input formats then the output
    /// formats.
    pub fn standalone(
        inputs: &'a [Buffer],
        formats: &'a [Format],
        input_ids: &'a [usize],
        output_ids: &'a [usize],
        outputs: &'a mut [Buffer],
    ) -> Io<'a> {
        Io {
            bufs: inputs,
            formats,
            inputs: input_ids,
            outputs: output_ids,
            outs: outputs,
            moved: None,
            tick: false,
            input0: &[],
            unconnected: 0,
            pass_s: 0.0,
            reformat: false,
            schema: empty_schema(),
            max_items: 4096,
            meter: None,
        }
    }

    /// Starts the timer of the section `name` of the component that runs. The time until the timer is dropped shows as
    /// a row under the component in the table of component loads.
    pub fn section(&self, name: &'static str) -> crate::meter::SectionTimer {
        self.meter().section(name)
    }

    /// The work counters of the component that runs, for code inside it that times its sections.
    pub fn meter(&self) -> crate::meter::Meter {
        crate::meter::Meter(self.meter.cloned())
    }

    /// The most items of a stream without a rate (bytes, messages, typed items) that one output keeps in one pass
    /// ([`Graph::set_max_items`](crate::Graph::set_max_items)).
    pub fn max_items(&self) -> usize {
        self.max_items
    }

    /// Sets the settings schema of the component under test on a standalone `Io`, so that [`apply_commands`](crate::apply_commands)
    /// can check commands against it.
    pub fn with_schema(mut self, schema: &'a Schema) -> Self {
        self.schema = schema;
        self
    }

    /// The settings schema of the component that runs.
    pub fn schema(&self) -> &'a Schema {
        self.schema
    }

    /// Asks the graph to configure this component again at the end of the pass, and then every later component whose input
    /// format changes as a result. Use it when the component decides by itself, while it runs, that its output formats change
    /// (an automatic channel width returns a new natural rate). The outputs of this pass keep the old formats. The new formats
    /// apply from the next pass. A component that refuses its new configuration stops the graph, and the graph's status says
    /// why.
    pub fn request_reformat(&mut self) {
        self.reformat = true;
    }

    /// Whether [`Io::request_reformat`] was called in this pass (for testing a component on its own).
    pub fn reformat_requested(&self) -> bool {
        self.reformat
    }

    /// The seconds of graph input that this pass holds: the input samples of the pass divided by the input rate. A component
    /// whose own streams have no rate (frames, typed items) can count signal time with it. It is 0 for a standalone `Io` and
    /// for a pass without input samples.
    pub fn pass_seconds(&self) -> f64 {
        self.pass_s
    }

    /// Sets the seconds that [`Io::pass_seconds`] returns (for testing a component on its own, with [`Io::standalone`]).
    pub fn set_pass_seconds(&mut self, seconds: f64) {
        self.pass_s = seconds;
    }

    /// Marks input `i` of a standalone `Io` as an optional input left unconnected (for testing a component on its own; inputs
    /// 64 and above cannot be marked).
    pub fn with_unconnected(mut self, i: usize) -> Self {
        if i < 64 {
            self.unconnected |= 1 << i;
        }
        self
    }

    /// Whether input `i` is connected. Only an optional input ([`Port::optional`]) can be unconnected; it then holds no items,
    /// and the component uses its documented default.
    pub fn is_connected(&self, i: usize) -> bool {
        i >= 64 || self.unconnected & (1 << i) == 0
    }

    /// The number of items input `i` holds this call (also for the graph's own input, whose samples are borrowed, not in its
    /// buffer).
    pub fn input_len(&self, i: usize) -> usize {
        if self.inputs[i] == 0 && !self.input0.is_empty() {
            self.input0.len()
        } else {
            self.input(i).len()
        }
    }

    /// Whether this pass ends a period of the graph's clock (see [`GraphBuilder::clock`](crate::GraphBuilder::clock)): the moment
    /// for periodic decisions, after this pass's items are processed. Always false in a graph without a clock.
    pub fn tick(&self) -> bool {
        self.tick
    }
    /// Marks this pass as ending a clock period (for testing a component on its own, with [`Io::standalone`]).
    pub fn set_tick(&mut self, tick: bool) {
        self.tick = tick;
    }

    /// Number of inputs.
    pub fn num_inputs(&self) -> usize {
        self.inputs.len()
    }
    /// Number of outputs.
    pub fn num_outputs(&self) -> usize {
        self.outs.len()
    }
    /// Input buffer `i`: its index, tags and items. For the graph's own input ([`INPUT`](crate::INPUT)) the samples are borrowed from
    /// the caller and are not in the buffer (its `len()` is 0, `end()` is right): read them with [`Io::iq`] or [`Io::samples`], and
    /// count them with [`Io::input_len`].
    pub fn input(&self, i: usize) -> &'a Buffer {
        &self.bufs[self.inputs[i]]
    }
    /// The format of input `i`.
    pub fn input_format(&self, i: usize) -> &'a Format {
        &self.formats[self.inputs[i]]
    }
    /// The format of output `o`.
    pub fn output_format(&self, o: usize) -> &'a Format {
        &self.formats[self.outputs[o]]
    }
    /// Output buffer `o`.
    pub fn output(&mut self, o: usize) -> &mut Buffer {
        &mut self.outs[o]
    }
    /// Input `i` and output `o` together.
    pub fn pair(&mut self, i: usize, o: usize) -> (&'a Buffer, &mut Buffer) {
        (self.input(i), &mut self.outs[o])
    }

    /// The samples of input `i` (`Iq`, `Symbols` or `Real`, by the requested type `T`). Empty for an input the graph moved into
    /// an output (see [`ComponentInfo::in_place`]); read it with [`Io::pass_through`] instead.
    pub fn samples<T: StreamSample>(&self, i: usize) -> &'a [T] {
        if self.inputs[i] == 0 && !self.input0.is_empty() {
            if let Some(x) = T::graph_input(self.input0) {
                return x;
            }
        }
        let d = &self.input(i).data;
        T::slice(d).unwrap_or_else(|| wrong("input", i, d))
    }
    /// The sample vector of output `o`, to append to.
    pub fn samples_out<T: StreamSample>(&mut self, o: usize) -> &mut Vec<T> {
        let d = &mut self.outs[o].data;
        if T::slice(d).is_none() {
            wrong("output", o, d)
        }
        T::vec_mut(d).unwrap()
    }
    /// Input `i`'s items in output `o`, to work on in place: moved there by the graph when the component declared
    /// [`ComponentInfo::in_place`] and the graph allowed it, copied otherwise.
    pub fn pass_through<T: StreamSample>(&mut self, i: usize, o: usize) -> &mut [T] {
        if self.moved == Some((i, o)) {
            return self.samples_out::<T>(o);
        }
        let src = self.samples::<T>(i);
        let out = self.samples_out::<T>(o);
        let start = out.len();
        out.extend_from_slice(src);
        &mut out[start..]
    }

    /// The samples of `Iq` input `i`.
    pub fn iq(&self, i: usize) -> &'a [Complex32] {
        self.samples(i)
    }
    /// `Iq` output `o`.
    pub fn iq_out(&mut self, o: usize) -> &mut Vec<Complex32> {
        self.samples_out(o)
    }
    /// The samples of `Real` input `i`.
    pub fn real(&self, i: usize) -> &'a [f32] {
        self.samples(i)
    }
    /// `Real` output `o`.
    pub fn real_out(&mut self, o: usize) -> &mut Vec<f32> {
        self.samples_out(o)
    }
    /// The frames of `Audio` input `i`, interleaved. Empty for an input the graph moved into an output (see
    /// [`ComponentInfo::in_place`]). Read it with [`Io::audio_pass_through`] instead.
    pub fn audio(&self, i: usize) -> &'a AudioFrames {
        match &self.input(i).data {
            Data::Audio(a) => a,
            d => wrong("input", i, d),
        }
    }
    /// `Audio` output `o`, to append interleaved frames to. Its channel count is set from the output's format.
    pub fn audio_out(&mut self, o: usize) -> &mut AudioFrames {
        let channels = self.output_format(o).channels;
        match &mut self.outs[o].data {
            Data::Audio(a) => {
                a.channels = channels;
                a
            }
            d => wrong("output", o, d),
        }
    }
    /// The samples of `Audio` input `i` in output `o`, interleaved, to work on in place: moved there by the graph when the
    /// component declared [`ComponentInfo::in_place`] and the graph allowed it, copied otherwise.
    pub fn audio_pass_through(&mut self, i: usize, o: usize) -> &mut [f32] {
        if self.moved == Some((i, o)) {
            return &mut self.audio_out(o).samples;
        }
        let src = &self.audio(i).samples;
        let out = &mut self.audio_out(o).samples;
        let start = out.len();
        out.extend_from_slice(src);
        &mut out[start..]
    }
    /// The bits of [`Kind::HardBits`] input `i` (`u8`, 0 or 1).
    pub fn hard_bits(&self, i: usize) -> &'a [u8] {
        match &self.input(i).data {
            Data::HardBits(v) => v,
            d => wrong("input", i, d),
        }
    }
    /// [`Kind::HardBits`] output `o`.
    pub fn hard_bits_out(&mut self, o: usize) -> &mut Vec<u8> {
        match &mut self.outs[o].data {
            Data::HardBits(v) => v,
            d => wrong("output", o, d),
        }
    }
    /// The soft bits of [`Kind::Llr`] input `i` (`i8`, positive means 0; scale in the input format's `llr_scale`).
    pub fn llrs(&self, i: usize) -> &'a [i8] {
        match &self.input(i).data {
            Data::Llr(v) => v,
            d => wrong("input", i, d),
        }
    }
    /// [`Kind::Llr`] output `o`.
    pub fn llrs_out(&mut self, o: usize) -> &mut Vec<i8> {
        match &mut self.outs[o].data {
            Data::Llr(v) => v,
            d => wrong("output", o, d),
        }
    }
    /// The packets of `Bytes` input `i`.
    pub fn bytes(&self, i: usize) -> &'a [Packet] {
        match &self.input(i).data {
            Data::Bytes(v) => v,
            d => wrong("input", i, d),
        }
    }
    /// `Bytes` output `o`.
    pub fn bytes_out(&mut self, o: usize) -> &mut Vec<Packet> {
        match &mut self.outs[o].data {
            Data::Bytes(v) => v,
            d => wrong("output", o, d),
        }
    }
    /// The records of `Messages` input `i`.
    pub fn messages(&self, i: usize) -> &'a [Event] {
        match &self.input(i).data {
            Data::Messages(v) => v,
            d => wrong("input", i, d),
        }
    }
    /// `Messages` output `o`.
    pub fn messages_out(&mut self, o: usize) -> &mut Vec<Event> {
        match &mut self.outs[o].data {
            Data::Messages(v) => v,
            d => wrong("output", o, d),
        }
    }
    /// The frames of `Spectrum` input `i`.
    pub fn spectrum(&self, i: usize) -> &'a Frames {
        match &self.input(i).data {
            Data::Spectrum(v) => v,
            d => wrong("input", i, d),
        }
    }
    /// `Spectrum` output `o` (its `bins` is set from the format).
    pub fn spectrum_out(&mut self, o: usize) -> &mut Frames {
        let bins = self.output_format(o).frame_len;
        match &mut self.outs[o].data {
            Data::Spectrum(v) => {
                v.bins = bins;
                v
            }
            d => wrong("output", o, d),
        }
    }
    /// The items of typed input `i`.
    pub fn items<T: Item>(&self, i: usize) -> &'a [Timed<T>] {
        match &self.input(i).data {
            Data::Items(None) => &[],
            Data::Items(Some(b)) => match b.as_any().downcast_ref::<Vec<Timed<T>>>() {
                Some(v) => v,
                None => panic!(
                    "input {i} holds {}, not {}",
                    b.item_type().rust_name(),
                    std::any::type_name::<T>()
                ),
            },
            d => wrong("input", i, d),
        }
    }
    /// Typed output `o`, to append to.
    pub fn items_out<T: Item>(&mut self, o: usize) -> &mut Vec<Timed<T>> {
        match &mut self.outs[o].data {
            Data::Items(slot) => {
                if slot.is_none() {
                    *slot = Some(Box::new(Vec::<Timed<T>>::new()));
                }
                let b = slot.as_mut().unwrap();
                let held = b.item_type().rust_name();
                match b.as_any_mut().downcast_mut::<Vec<Timed<T>>>() {
                    Some(v) => v,
                    None => panic!(
                        "output {o} holds {held}, not {}",
                        std::any::type_name::<T>()
                    ),
                }
            }
            d => wrong("output", o, d),
        }
    }
}

/// Moves tags from an input to an output, through any change of rate, holding back those whose output item does not exist yet.
///
/// A tag at input index `i` lands on output index `out0 + ceil((i - in0) * out_rate / in_rate)`, where `(in0, out0)` are the
/// first indices of the first call after the last [`reset`](Self::reset). Filter delay is not added. The position does not depend
/// on how the stream is cut into calls. When either side has no rate, a tag lands on the next output item. A tag whose output
/// item has not been produced yet waits for a later call; `EndOfStream` may be one past the last item.
///
/// The graph uses one per input and output pair for [`TagPolicy::Auto`]; a component with [`TagPolicy::Manual`] keeps its own.
#[derive(Clone, Debug, Default)]
pub struct TagCarry {
    ratio: Option<f64>,
    anchor: Option<(u64, u64)>,
    pending: Vec<Tag>,
}

impl TagCarry {
    /// The most tags that wait for their output item.
    pub const MAX_PENDING: usize = 256;

    /// For a component from `in_rate` to `out_rate` (0 for a side without a rate).
    pub fn new(in_rate: f64, out_rate: f64) -> TagCarry {
        let mut c = TagCarry::default();
        c.reset(in_rate, out_rate);
        c
    }
    /// For a component that keeps the rate.
    pub fn same_rate() -> TagCarry {
        TagCarry::new(1.0, 1.0)
    }
    /// After a configure: new rates, anchored again at the next call. Tags already waiting keep their output positions.
    pub fn reset(&mut self, in_rate: f64, out_rate: f64) {
        self.ratio = (in_rate > 0.0 && out_rate > 0.0).then(|| out_rate / in_rate);
        self.anchor = None;
    }
    /// The output position of input position `at` (anchoring at this call if needed).
    pub fn map(&mut self, input: &Buffer, output: &Buffer, at: u64) -> u64 {
        let (i0, o0) = *self.anchor.get_or_insert((input.first(), output.first()));
        match self.ratio {
            Some(1.0) => o0 + at.saturating_sub(i0),
            Some(r) => o0 + (at.saturating_sub(i0) as f64 * r - 1e-9).ceil().max(0.0) as u64,
            None => output.end(),
        }
    }
    /// Forwards every tag of `input`, then emits what is due on `output`. Call it on every call (also when there are no tags), after
    /// appending this call's items.
    pub fn forward(&mut self, input: &Buffer, output: &mut Buffer) {
        self.forward_if(input, output, |_| true)
    }
    /// Forwards the tags of `input` that `keep` accepts (the others are consumed), then emits what is due.
    pub fn forward_if(&mut self, input: &Buffer, output: &mut Buffer, keep: impl Fn(&Tag) -> bool) {
        // Anchored at the start of the first call after a reset, whether or not it has tags, so where a stream is cut into
        // calls never moves a tag.
        self.anchor.get_or_insert((input.first(), output.first()));
        for t in input.tags() {
            if keep(t) {
                let at = self.map(input, output, t.at);
                self.queue(at, t.kind.clone());
            }
        }
        self.flush(output);
    }
    /// Queues a tag at an output position the component chose. At most [`TagCarry::MAX_PENDING`] tags wait; beyond that the oldest
    /// is dropped.
    pub fn queue(&mut self, at: u64, kind: TagKind) {
        if self.pending.len() >= Self::MAX_PENDING {
            self.pending.remove(0);
        }
        self.pending.push(Tag { at, kind });
    }
    /// Emits the waiting tags whose output item exists (in order).
    pub fn flush(&mut self, output: &mut Buffer) {
        if self.pending.is_empty() {
            return;
        }
        let (first, end) = (output.first(), output.end());
        let mut i = 0;
        while i < self.pending.len() {
            let t = &self.pending[i];
            let due = t.at < end || (t.at == end && t.kind == TagKind::EndOfStream);
            if due {
                let t = self.pending.remove(i);
                output.push_tag(t.at.max(first), t.kind);
            } else {
                i += 1;
            }
        }
    }
    /// Tags still waiting for their output item.
    pub fn pending(&self) -> &[Tag] {
        &self.pending
    }
}
