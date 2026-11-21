//! The graph components `tones.subaudio`, `tones.dtmf`, `tones.selcall`, `tones.selcal` and `tones.two_tone`: `Real` audio in,
//! typed items out, timed at the decision.

use crate::ctcss::{CtcssConfig, CtcssDecoder, CtcssEvent, CtcssEventKind};
use crate::dcs::{DcsConfig, DcsDecoder, DcsEvent, DcsEventKind};
use crate::dtmf::{DtmfConfig, DtmfDecoder, DtmfEvent};
use crate::selcal::{SelcalCall, SelcalConfig, SelcalDecoder};
use crate::selcall::{SelcallCall, SelcallConfig, SelcallDecoder};
use crate::twotone::{TwoToneConfig, TwoToneDecoder, TwoTonePage};
use crate::TonesError;
use sdrkit_core::interface::{EventShape, Fields, Hertz, Seconds, StackShape};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, keys, Field};
use sdrkit_core::{
    CatalogueSetting, Choice, ChoiceDecl, ConfigError, EventDef, Record, Schema, Setting,
    SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Registry,
    Timed,
};
use sdrkit_util_ids::tones::selcall::TONE_SETS;
use std::fmt::Write;
use std::sync::Arc;

/// The interface of the event types of this crate, which stacks of several families return. A family whose stacks
/// list one of them names this interface with `Family::uses`.
pub fn interface() -> &'static sdrkit_core::interface::Family {
    static SHARED: std::sync::OnceLock<sdrkit_core::interface::Family> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        sdrkit_core::interface::Family::shared("tones", 1, 0).stack(
            StackShape::new("tones")
                .event(subaudio())
                .event(dtmf())
                .event(selcall())
                .event(selcal())
                .event(two_tone()),
        )
    })
}

/// The event type `tones.subaudio` with its shape, for the description of a stack.
pub fn subaudio() -> EventShape {
    EventShape::of::<SubAudioFields<'static>>(&SUBAUDIO_EVENT)
}

/// The event type `tones.dtmf` with its shape, for the description of a stack.
pub fn dtmf() -> EventShape {
    EventShape::of::<DtmfFields<'static>>(&DTMF_EVENT)
}

/// The event type `tones.selcall` with its shape, for the description of a stack.
pub fn selcall() -> EventShape {
    EventShape::of::<SelcallFields<'static>>(&SELCALL_EVENT)
}

/// The event type `tones.selcal` with its shape, for the description of a stack.
pub fn selcal() -> EventShape {
    EventShape::of::<SelcalFields<'static>>(&SELCAL_EVENT)
}

/// The event type `tones.two_tone` with its shape, for the description of a stack.
pub fn two_tone() -> EventShape {
    EventShape::of::<TwoToneFields<'static>>(&TWO_TONE_EVENT)
}

/// Adds the `tones.*` components.
///
/// ```
/// use sdrkit_graph::Registry;
/// let mut r = Registry::new();
/// sdrkit_proto_tones::components::register(&mut r);
/// assert!(r.make("tones.selcal").is_ok());
/// ```
pub fn register(r: &mut Registry) {
    r.add("tones.subaudio", || Box::new(SubAudioComponent::default()));
    r.add("tones.dtmf", || Box::new(DtmfComponent::default()));
    r.add("tones.selcall", || Box::new(SelcallComponent::default()));
    r.add("tones.selcal", || Box::new(SelcalComponent::default()));
    r.add("tones.two_tone", || Box::new(TwoToneComponent::default()));
}

/// The text of an event for people, written into `out` (cleared first).
fn text<'a>(out: &'a mut String, args: std::fmt::Arguments<'_>) -> &'a str {
    out.clear();
    // Writing into a String cannot fail.
    let _ = out.write_fmt(args);
    out
}

/// What a component of this module needs from its decoder.
pub trait Spec: Send + Default + 'static {
    /// The output item.
    type Out: Item;
    /// The decoder.
    type Dec: Send;
    /// The registered name.
    const NAME: &'static str;
    /// The settings.
    fn schema() -> Arc<Schema>;
    /// Takes one setting. The host made sure that the value fits the schema.
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError>;
    /// The settings now.
    fn values(&self) -> Values;
    /// The decoder for audio at `rate` Hz.
    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError>;
    /// Runs the decoder over `x`. `out(at, item)` takes each item with its input sample position.
    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out));
}

/// A component over a decoder (the types below name each one).
pub struct ToneComponent<S: Spec> {
    spec: S,
    dec: Option<S::Dec>,
    rate: f64,
    origin: Option<u64>,
    consumed: u64,
}

impl<S: Spec> Default for ToneComponent<S> {
    fn default() -> Self {
        ToneComponent {
            spec: S::default(),
            dec: None,
            rate: 0.0,
            origin: None,
            consumed: 0,
        }
    }
}

impl<S: Spec> Component for ToneComponent<S> {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            S::NAME,
            vec![Port::real("in")],
            vec![Port::typed::<S::Out>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(S::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        self.dec = Some(self.spec.build(rate)?);
        self.rate = rate;
        self.origin = None;
        self.consumed = 0;
        let mut out = inputs[0].with_kind(Kind::typed::<S::Out>());
        out.rate = 0.0;
        Ok(vec![out])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let first = io.input(0).first();
        let Some(dec) = self.dec.as_mut() else { return };
        let origin = *self.origin.get_or_insert(first - self.consumed);
        self.consumed += x.len() as u64;
        let rate = self.rate;
        let out = io.items_out::<S::Out>(0);
        S::run(dec, x, &mut |at, value| {
            out.push(Timed {
                time: (origin + at) as f64 / rate,
                value,
            })
        });
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        self.spec.apply(id, value)?;
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        self.spec.values()
    }
}

// ----- tones.subaudio -----

sdrkit_core::choice_enum! {
    /// The signalling of a [`SubAudioEvent`].
    pub enum SubAudioKind {
        /// CTCSS.
        Ctcss = ("ctcss", "CTCSS"),
        /// DCS.
        Dcs = ("dcs", "DCS"),
    }
}

sdrkit_core::choice_enum! {
    /// The change a [`SubAudioEvent`] reports.
    pub enum SubAudioChange {
        /// Locked on a tone or a code.
        Locked = ("locked", "Locked"),
        /// Released.
        Released = ("released", "Released"),
        /// Released by the turn-off burst or code.
        TurnOff = ("turn_off", "Turn-off"),
    }
}

sdrkit_core::choice_enum! {
    /// The CTCSS tones `tones.subaudio` listens for.
    pub enum CtcssTones {
        /// All 50 tones.
        All = ("all", "All 50"),
        /// The 38 EIA tones.
        Eia = ("eia", "The 38 EIA tones"),
    }
}

/// The change of a `tones.subaudio` event.
pub const CHANGE: Field<field::Choice<SubAudioChange>> = Field::new("event", "Change", "");
/// The CTCSS tone of a `tones.subaudio` event.
pub const TONE_HZ: Field<field::Hz> = Field::new("tone_hz", "Tone", "Hz");

/// The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubAudioFields<'a> {
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// One line for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signalling.
    #[field(label = "Kind")]
    pub kind: SubAudioKind,
    /// The change.
    #[field(shared = CHANGE)]
    pub event: SubAudioChange,
    /// The CTCSS tone.
    #[field(shared = TONE_HZ)]
    pub tone_hz: Option<Hertz>,
    /// The CTCSS tone is an EIA tone.
    #[field(label = "EIA tone")]
    pub eia: Option<bool>,
    /// The Motorola code of the CTCSS tone.
    #[field(label = "Motorola code")]
    pub motorola: Option<&'a str>,
    /// The measured CTCSS tone.
    #[field(label = "Measured", unit = "Hz")]
    pub measured_hz: Option<Hertz>,
    /// The DCS code with its polarity ("D023N").
    #[field(label = "Code")]
    pub code: Option<&'a str>,
    /// The DCS code is a standard code.
    #[field(label = "Standard code")]
    pub standard: Option<bool>,
    /// A standard code that the same word also reads as.
    #[field(label = "Alias")]
    pub alias: Option<&'a str>,
    /// The level of the tone, dB relative to a full-scale sine.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The event of a CTCSS or DCS change.
pub static SUBAUDIO_EVENT: EventDef = EventDef::new(
    "tones.subaudio",
    "CTCSS or DCS",
    <SubAudioFields<'static> as Fields>::SHAPE.fields,
);

/// A CTCSS or DCS change, the output of `tones.subaudio`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SubAudioEvent {
    /// A CTCSS change.
    Ctcss(CtcssEvent),
    /// A DCS change.
    Dcs(DcsEvent),
}

fn ctcss_change(k: CtcssEventKind) -> SubAudioChange {
    match k {
        CtcssEventKind::Locked => SubAudioChange::Locked,
        CtcssEventKind::Released => SubAudioChange::Released,
        CtcssEventKind::TurnOff => SubAudioChange::TurnOff,
    }
}

fn dcs_change(k: DcsEventKind) -> SubAudioChange {
    match k {
        DcsEventKind::Locked => SubAudioChange::Locked,
        DcsEventKind::Released => SubAudioChange::Released,
        DcsEventKind::TurnOff => SubAudioChange::TurnOff,
    }
}

impl Item for SubAudioEvent {
    const KIND: &'static str = "tones.subaudio";
    fn event_def() -> Option<&'static EventDef> {
        Some(&SUBAUDIO_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let mut s = String::new();
        match self {
            SubAudioEvent::Ctcss(e) => {
                let change = ctcss_change(e.kind);
                let label = &SubAudioChange::CHOICES[usize::from(change.index())].label;
                SubAudioFields {
                    family: Family::Receiver,
                    text: text(&mut s, format_args!("CTCSS {} Hz {label}", e.tone.hz)),
                    kind: SubAudioKind::Ctcss,
                    event: change,
                    tone_hz: Some(Hertz(f64::from(e.tone.hz))),
                    eia: Some(e.tone.eia),
                    motorola: e.tone.motorola,
                    measured_hz: Some(Hertz(f64::from(e.measured_hz))),
                    code: None,
                    standard: None,
                    alias: None,
                    level_db: f64::from(e.level_db),
                }
                .write(r);
            }
            SubAudioEvent::Dcs(e) => {
                let change = dcs_change(e.kind);
                let name = e.reading.name();
                let label = &SubAudioChange::CHOICES[usize::from(change.index())].label;
                let alias = e.reading.alias.map(|(c, inverted)| {
                    let pol = if inverted { 'I' } else { 'N' };
                    format!("{c}{pol}")
                });
                SubAudioFields {
                    family: Family::Receiver,
                    text: text(&mut s, format_args!("DCS {name} {label}")),
                    kind: SubAudioKind::Dcs,
                    event: change,
                    tone_hz: None,
                    eia: None,
                    motorola: None,
                    measured_hz: None,
                    code: Some(&name),
                    standard: Some(e.reading.code.is_standard()),
                    alias: alias.as_deref(),
                    level_db: f64::from(e.level_db),
                }
                .write(r);
            }
        }
    }
}

const CTCSS: Setting<field::Bool> =
    Setting::new("ctcss", "CTCSS", "", true).help("Read CTCSS tones.");
const CTCSS_TONES: Setting<field::Choice<CtcssTones>> =
    Setting::choice("ctcss_tones", "CTCSS tones", CtcssTones::All)
        .help("The CTCSS tones to listen for.");
const DCS: Setting<field::Bool> = Setting::new("dcs", "DCS", "", true).help("Read DCS codes.");
const TURN_OFF: Setting<field::Bool> = Setting::new("turn_off", "Turn-off", "", true)
    .help("Release at once on the CTCSS reverse burst and the DCS turn-off code.");

sdrkit_core::settings! {
    /// The settings of `tones.subaudio`.
    pub enum SubAudioSetting {
        /// CTCSS.
        Ctcss = CTCSS,
        /// CTCSS tones.
        CtcssTones = CTCSS_TONES,
        /// DCS.
        Dcs = DCS,
        /// Turn-off.
        TurnOff = TURN_OFF,
    }
}

/// The settings of `tones.subaudio`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubAudioSpec {
    ctcss: bool,
    tones: CtcssTones,
    dcs: bool,
    turn_off: bool,
}

impl Default for SubAudioSpec {
    fn default() -> Self {
        SubAudioSpec {
            ctcss: CTCSS.default,
            tones: CTCSS_TONES.default,
            dcs: DCS.default,
            turn_off: TURN_OFF.default,
        }
    }
}

/// The decoders of `tones.subaudio`.
pub struct SubAudioDecoders {
    ctcss: Option<CtcssDecoder>,
    dcs: Option<DcsDecoder>,
}

impl Spec for SubAudioSpec {
    type Out = SubAudioEvent;
    type Dec = SubAudioDecoders;
    const NAME: &'static str = "tones.subaudio";

    fn schema() -> Arc<Schema> {
        SubAudioSetting::schema()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SubAudioSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SubAudioSetting::Ctcss => self.ctcss = CTCSS.read(id, value)?,
            SubAudioSetting::CtcssTones => self.tones = CTCSS_TONES.read(id, value)?,
            SubAudioSetting::Dcs => self.dcs = DCS.read(id, value)?,
            SubAudioSetting::TurnOff => self.turn_off = TURN_OFF.read(id, value)?,
        }
        Ok(())
    }

    fn values(&self) -> Values {
        values_of([
            (SubAudioSetting::Ctcss, Value::Bool(self.ctcss)),
            (
                SubAudioSetting::CtcssTones,
                Value::Choice(self.tones.index()),
            ),
            (SubAudioSetting::Dcs, Value::Bool(self.dcs)),
            (SubAudioSetting::TurnOff, Value::Bool(self.turn_off)),
        ])
    }

    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError> {
        let ctcss = self
            .ctcss
            .then(|| {
                CtcssDecoder::new(
                    rate,
                    CtcssConfig {
                        eia_only: self.tones == CtcssTones::Eia,
                        turn_off: self.turn_off,
                        ..CtcssConfig::default()
                    },
                )
            })
            .transpose()?;
        let dcs = self
            .dcs
            .then(|| {
                DcsDecoder::new(
                    rate,
                    DcsConfig {
                        turn_off: self.turn_off,
                        ..DcsConfig::default()
                    },
                )
            })
            .transpose()?;
        Ok(SubAudioDecoders { ctcss, dcs })
    }

    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out)) {
        // One sample at a time keeps the two decoders in step, so a DCS lock acts on CTCSS at once.
        for v in x.chunks(1) {
            if let Some(d) = dec.dcs.as_mut() {
                let mut locked_now = false;
                d.process(v, |e| {
                    locked_now |= e.kind == DcsEventKind::Locked;
                    out(e.at, SubAudioEvent::Dcs(e));
                });
                if locked_now {
                    if let Some(c) = dec.ctcss.as_mut() {
                        if let Some((tone, hz, db)) = c.locked() {
                            out(
                                d_at(d),
                                SubAudioEvent::Ctcss(CtcssEvent {
                                    kind: CtcssEventKind::Released,
                                    tone,
                                    at: d_at(d),
                                    measured_hz: hz,
                                    level_db: db,
                                }),
                            );
                        }
                        c.reset();
                    }
                }
            }
            let dcs_locked = dec.dcs.as_ref().is_some_and(|d| d.locked().is_some());
            if let Some(c) = dec.ctcss.as_mut() {
                if dcs_locked {
                    c.skip(v.len());
                } else {
                    c.process(v, |e| out(e.at, SubAudioEvent::Ctcss(e)));
                }
            }
        }
    }
}

fn d_at(d: &DcsDecoder) -> u64 {
    d.position()
}

/// `tones.subaudio`: CTCSS and DCS together (module documentation).
pub type SubAudioComponent = ToneComponent<SubAudioSpec>;

// ----- tones.dtmf -----

/// The fields of a `tones.dtmf` event: a DTMF key that went down or up.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DtmfFields<'a> {
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// One line for people: the key.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The DTMF key.
    #[field(label = "Key")]
    pub key: &'a str,
    /// The key went down (or up).
    #[field(label = "Down")]
    pub down: bool,
    /// The level of the row tone.
    #[field(label = "Row tone", unit = "dB")]
    pub row_db: f64,
    /// The level of the column tone.
    #[field(label = "Column tone", unit = "dB")]
    pub column_db: f64,
}

/// The event of a DTMF key.
pub static DTMF_EVENT: EventDef = EventDef::new(
    "tones.dtmf",
    "DTMF key",
    <DtmfFields<'static> as Fields>::SHAPE.fields,
);

impl Item for DtmfEvent {
    const KIND: &'static str = "tones.dtmf";
    fn event_def() -> Option<&'static EventDef> {
        Some(&DTMF_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let mut b = [0u8; 4];
        let key: &str = self.key.encode_utf8(&mut b);
        DtmfFields {
            family: Family::Receiver,
            text: key,
            key,
            down: self.down,
            row_db: f64::from(self.row_db),
            column_db: f64::from(self.column_db),
        }
        .write(r);
    }
}

const MIN_LEVEL_DB: Setting<field::F64> =
    Setting::new("min_level_db", "Smallest level", "dB", -45.0)
        .range(-90.0, 0.0, 0.0)
        .help("The smallest level of each tone, relative to a full-scale sine.");
const HARMONICS: Setting<field::Bool> = Setting::new("harmonics", "Refuse speech", "", true)
    .help("Refuse tones with strong second harmonics (speech).");

sdrkit_core::settings! {
    /// The settings of `tones.dtmf`.
    pub enum DtmfSetting {
        /// Smallest level.
        MinLevelDb = MIN_LEVEL_DB,
        /// Refuse speech.
        Harmonics = HARMONICS,
    }
}

/// The settings of `tones.dtmf`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DtmfSpec(DtmfConfig);

impl Spec for DtmfSpec {
    type Out = DtmfEvent;
    type Dec = DtmfDecoder;
    const NAME: &'static str = "tones.dtmf";

    fn schema() -> Arc<Schema> {
        DtmfSetting::schema()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match DtmfSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DtmfSetting::MinLevelDb => self.0.min_level_db = MIN_LEVEL_DB.read(id, value)? as f32,
            DtmfSetting::Harmonics => self.0.harmonics = HARMONICS.read(id, value)?,
        }
        Ok(())
    }

    fn values(&self) -> Values {
        values_of([
            (
                DtmfSetting::MinLevelDb,
                Value::F64(f64::from(self.0.min_level_db)),
            ),
            (DtmfSetting::Harmonics, Value::Bool(self.0.harmonics)),
        ])
    }
    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError> {
        DtmfDecoder::new(rate, self.0)
    }

    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out)) {
        dec.process(x, |e| out(e.at, e));
    }
}

/// `tones.dtmf`: DTMF keys.
pub type DtmfComponent = ToneComponent<DtmfSpec>;

// ----- tones.selcall -----

/// The fields of a `tones.selcall` event: a selcall call.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SelcallFields<'a> {
    /// The digits of the call.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// One line for people: the digits.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The digits of a call as heard.
    #[field(label = "Digits")]
    pub digits: &'a str,
    /// The digits as sent, with the repeat tones.
    #[field(label = "Sent")]
    pub sent: &'a str,
    /// The tone set the call was read as.
    #[field(label = "Tone set")]
    pub set: &'a str,
    /// The other tone sets the call fits, separated by commas.
    #[field(label = "Also fits")]
    pub also_fits: Option<&'a str>,
    /// The length of the call.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The mean length of a tone.
    #[field(label = "Tone length", unit = "s")]
    pub tone_s: Seconds,
}

/// The event of a selcall call.
pub static SELCALL_EVENT: EventDef = EventDef::new(
    "tones.selcall",
    "Selcall call",
    <SelcallFields<'static> as Fields>::SHAPE.fields,
);

impl Item for SelcallCall {
    const KIND: &'static str = "tones.selcall";
    fn event_def() -> Option<&'static EventDef> {
        Some(&SELCALL_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let mut also = String::new();
        for (i, s) in TONE_SETS.iter().enumerate() {
            if self.also_fits & (1 << i) != 0 {
                if !also.is_empty() {
                    also.push_str(", ");
                }
                also.push_str(s.name);
            }
        }
        SelcallFields {
            source: self.digits(),
            text: self.digits(),
            digits: self.digits(),
            sent: self.sent(),
            set: self.set.name,
            also_fits: (!also.is_empty()).then_some(also.as_str()),
            length_s: Seconds(self.length_s),
            tone_s: Seconds(self.tone_s),
        }
        .write(r);
    }
}

/// The `set` setting of `tones.selcall`: `all`, then the tone sets of `sdrkit_util_ids::tones::selcall::TONE_SETS` in order.
const TONE_SET: CatalogueSetting = CatalogueSetting {
    key: "set",
    label: "Tone set",
    help: "The tone set to read calls as, or all of them (the best fit wins).",
    choices: || {
        std::iter::once(ChoiceDecl::new("all", "All sets"))
            .chain(TONE_SETS.iter().map(|s| ChoiceDecl::new(s.name, s.name)))
            .collect()
    },
    default: "all",
    advanced: false,
};

sdrkit_core::settings! {
    /// The settings of `tones.selcall`. `set` is a choice of `all` (index 0) and the tone sets of
    /// `sdrkit_util_ids::tones::selcall::TONE_SETS` (index 1 on).
    pub enum SelcallSetting {
        /// Tone set.
        Set = TONE_SET,
    }
}

/// The settings of `tones.selcall`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelcallSpec {
    /// The index in `TONE_SETS`, or `None` for all sets.
    set: Option<usize>,
}

impl Spec for SelcallSpec {
    type Out = SelcallCall;
    type Dec = SelcallDecoder;
    const NAME: &'static str = "tones.selcall";

    fn schema() -> Arc<Schema> {
        SelcallSetting::schema()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SelcallSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelcallSetting::Set => {
                self.set = TONE_SET.read(id, value)?.checked_sub(1);
            }
        }
        Ok(())
    }

    fn values(&self) -> Values {
        let i = self.set.map_or(0, |i| i + 1) as u16;
        values_of([(SelcallSetting::Set, Value::Choice(i))])
    }

    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError> {
        let cfg = match self.set {
            None => SelcallConfig::default(),
            Some(i) => SelcallConfig {
                sets: 1 << i,
                ..SelcallConfig::default()
            },
        };
        SelcallDecoder::new(rate, cfg)
    }
    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out)) {
        dec.process(x, |c| out(c.start, c));
    }
}

/// `tones.selcall`: sequential tone calls.
pub type SelcallComponent = ToneComponent<SelcallSpec>;

// ----- tones.selcal -----

sdrkit_core::choice_enum! {
    /// The SELCAL tones `tones.selcal` listens for.
    pub enum SelcalTones {
        /// All 32 tones.
        All = ("all", "All 32"),
        /// The 16 classic tones.
        Classic = ("classic", "The 16 classic tones"),
    }
}

/// The fields of a `tones.selcal` event: a SELCAL call.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SelcalFields<'a> {
    /// The code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// One line for people: the code.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The SELCAL code ("AB-CD").
    #[field(label = "SELCAL code")]
    pub code: &'a str,
    /// The code uses only the 16 classic tones.
    #[field(label = "Classic tones")]
    pub classic: bool,
    /// The level of the tones, dB relative to a full-scale sine.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The shift of the tones from their frequencies (a sideband receiver off tune).
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Hertz,
}

/// The event of a SELCAL call.
pub static SELCAL_EVENT: EventDef = EventDef::new(
    "tones.selcal",
    "SELCAL call",
    <SelcalFields<'static> as Fields>::SHAPE.fields,
);

impl Item for SelcalCall {
    const KIND: &'static str = "tones.selcal";
    fn event_def() -> Option<&'static EventDef> {
        Some(&SELCAL_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let mut s = String::new();
        let code = text(&mut s, format_args!("{}", self.code));
        SelcalFields {
            source: code,
            text: code,
            family: Family::Aircraft,
            code,
            classic: self.code.is_classic(),
            level_db: f64::from(self.level_db),
            shift_hz: Hertz(f64::from(self.shift_hz)),
        }
        .write(r);
    }
}

const TONES: Setting<field::Choice<SelcalTones>> =
    Setting::choice("tones", "Tones", SelcalTones::All).help("The SELCAL tones to listen for.");
const WINDOW_S: Setting<field::Duration> = Setting::new("window_s", "Window", "s", 0.2)
    .range(0.05, 0.5, 0.0)
    .help("The analysis window: 0.2 s separates the 32 tones, 0.1 s is enough for the 16 classic tones.");
const SSB_SEARCH: Setting<field::Bool> = Setting::new("ssb_search", "Sideband search", "", false)
    .help("Also try the tones moved by up to 25 Hz (a sideband receiver off tune).");

sdrkit_core::settings! {
    /// The settings of `tones.selcal`.
    pub enum SelcalSetting {
        /// Tones.
        Tones = TONES,
        /// Window.
        WindowS = WINDOW_S,
        /// Sideband search.
        SsbSearch = SSB_SEARCH,
    }
}

/// The settings of `tones.selcal`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SelcalSpec(SelcalConfig);

impl Spec for SelcalSpec {
    type Out = SelcalCall;
    type Dec = SelcalDecoder;
    const NAME: &'static str = "tones.selcal";

    fn schema() -> Arc<Schema> {
        SelcalSetting::schema()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match SelcalSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SelcalSetting::Tones => {
                self.0.classic_only = TONES.read(id, value)? == SelcalTones::Classic;
            }
            SelcalSetting::WindowS => {
                let w = WINDOW_S.read(id, value)?;
                self.0.window_s = w;
                self.0.hop_s = (w / 4.0).max(0.01);
            }
            SelcalSetting::SsbSearch => self.0.ssb_search = SSB_SEARCH.read(id, value)?,
        }
        Ok(())
    }

    fn values(&self) -> Values {
        let tones = if self.0.classic_only {
            SelcalTones::Classic
        } else {
            SelcalTones::All
        };
        values_of([
            (SelcalSetting::Tones, Value::Choice(tones.index())),
            (SelcalSetting::WindowS, Value::Duration(self.0.window_s)),
            (SelcalSetting::SsbSearch, Value::Bool(self.0.ssb_search)),
        ])
    }
    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError> {
        SelcalDecoder::new(rate, self.0)
    }

    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out)) {
        dec.process(x, |c| out(c.end, c));
    }
}

/// `tones.selcal`: aviation SELCAL calls.
pub type SelcalComponent = ToneComponent<SelcalSpec>;

// ----- tones.two_tone -----

/// The fields of a `tones.two_tone` event: a two-tone page.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TwoToneFields<'a> {
    /// One line for people: the two tones.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The frequency of tone A.
    #[field(label = "Tone A", unit = "Hz")]
    pub a_hz: Hertz,
    /// The frequency of tone B.
    #[field(label = "Tone B", unit = "Hz")]
    pub b_hz: Hertz,
    /// The length of tone A.
    #[field(label = "Tone A length", unit = "s")]
    pub a_s: Seconds,
    /// The length of tone B.
    #[field(label = "Tone B length", unit = "s")]
    pub b_s: Seconds,
    /// Tone B ended (it did not run past the end of the input).
    #[field(label = "Ended")]
    pub ended: bool,
}

/// The event of a two-tone page.
pub static TWO_TONE_EVENT: EventDef = EventDef::new(
    "tones.two_tone",
    "Two-tone page",
    <TwoToneFields<'static> as Fields>::SHAPE.fields,
);

impl Item for TwoTonePage {
    const KIND: &'static str = "tones.two_tone";
    fn event_def() -> Option<&'static EventDef> {
        Some(&TWO_TONE_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let mut s = String::new();
        TwoToneFields {
            text: text(
                &mut s,
                format_args!("{:.1} Hz / {:.1} Hz", self.a_hz, self.b_hz),
            ),
            a_hz: Hertz(self.a_hz),
            b_hz: Hertz(self.b_hz),
            a_s: Seconds(self.a_s),
            b_s: Seconds(self.b_s),
            ended: self.ended,
        }
        .write(r);
    }
}

const B_MIN_S: Setting<field::Duration> = Setting::new("b_min_s", "Shortest B tone", "s", 0.8)
    .range(0.1, 10.0, 0.0)
    .help("The shortest tone B of a page.");

sdrkit_core::settings! {
    /// The settings of `tones.two_tone`.
    pub enum TwoToneSetting {
        /// Shortest B tone.
        BMinS = B_MIN_S,
    }
}

/// The settings of `tones.two_tone`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TwoToneSpec(TwoToneConfig);

impl Spec for TwoToneSpec {
    type Out = TwoTonePage;
    type Dec = TwoToneDecoder;
    const NAME: &'static str = "tones.two_tone";

    fn schema() -> Arc<Schema> {
        TwoToneSetting::schema()
    }

    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match TwoToneSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TwoToneSetting::BMinS => self.0.b_min_s = B_MIN_S.read(id, value)?,
        }
        Ok(())
    }

    fn values(&self) -> Values {
        values_of([(TwoToneSetting::BMinS, Value::Duration(self.0.b_min_s))])
    }
    fn build(&self, rate: f64) -> Result<Self::Dec, TonesError> {
        TwoToneDecoder::new(rate, self.0)
    }

    fn run(dec: &mut Self::Dec, x: &[f32], out: &mut dyn FnMut(u64, Self::Out)) {
        dec.process(x, |p| out(p.start, p));
    }
}

/// `tones.two_tone`: two-tone sequential pages.
pub type TwoToneComponent = ToneComponent<TwoToneSpec>;
