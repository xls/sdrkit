//! SELCAL: the tuned voice channel demodulated (FM, USB, LSB or AM, or a sound card input), two-tone pair calls detected by
//! the SELCAL decoder of sdrkit-proto-tones and logged with the aircraft of each code. This crate owns the one `selcal` stack id.
//! Standard: ARINC 714.

pub mod detector;
pub mod front;
pub mod generator;
pub mod tables;

use crate::verify;
use sdrkit_components::dsp::AudioResampler;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, CommandError, CommandId, Commands, Complex32, ConfigError, EventDecl, GenConfig,
    Generator, InputInfo, LockProfile, Output, Owner, Receiver, Record, Results, ResultsDecl,
    Schema, Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, StateBlob,
    StateError, Station, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Port, Registry, StackGraph,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use generator::{GenParams, SelcalGenerator};

/// The stack id.
pub const ID: &str = sdrkit_proto_tones::selcal::STACK_ID;
/// The audio rate the detector works at.
pub const AUDIO_RATE: f64 = 8000.0;
/// Below this the receiver refuses radio samples.
pub const MIN_RATE_HZ: f64 = 500e3;
/// Below this the receiver refuses sound card audio.
pub const MIN_AUDIO_RATE_HZ: f64 = 8000.0;

/// What the front end measured at a report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Measures {
    /// Signal time.
    pub time_s: f64,
    /// The audio level, dB.
    pub level_db: f32,
    /// The audio spectrum, dB per bin.
    pub spectrum: Vec<f32>,
    /// The strongest pure tone: Hz and dB.
    pub tone: Option<(f64, f32)>,
    /// The dial frequency (0 for sound card audio).
    pub dial_hz: f64,
    /// An FM carrier is present.
    pub carrier: bool,
    /// Why the front end refuses the samples, empty when it takes them.
    pub refused: String,
    /// A sideband demodulator is in use.
    pub ssb: bool,
    /// The samples are sound card audio.
    pub audio_input: bool,
}

impl Item for Measures {
    const KIND: &'static str = "selcal.measures";
}

sdrkit_core::choice_enum! {
    /// The demodulator of the SELCAL front end.
    pub enum DemodChoice {
        /// USB below 30 MHz, AM above.
        Auto = ("auto", "Auto"),
        /// FM.
        Fm = ("fm", "FM"),
        /// Upper sideband.
        Usb = ("usb", "USB"),
        /// Lower sideband.
        Lsb = ("lsb", "LSB"),
        /// AM.
        Am = ("am", "AM"),
    }
}

/// The demodulator.
pub const DEMOD: Setting<field::Choice<DemodChoice>> =
    Setting::choice("demod", "Demodulator", DemodChoice::Auto)
        .help("Auto: USB below 30 MHz, AM above.");
/// Play the channel audio.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false)
    .help("Plays the channel audio. Decoding is the same either way.");
/// The samples are sound card audio.
pub const AUDIO_INPUT: Setting<field::Bool> = Setting::new("audio_input", "Audio input", "", false)
    .help("The samples are sound card audio (8 kHz or more in the real part).");
/// The table of codes and aircraft.
pub const AIRCRAFT_TABLE: Setting<field::Text> =
    Setting::new("aircraft_table", "Aircraft table", "", "")
        .help("Lines of a SELCAL code and the aircraft's registration, separated by a tab, a comma or a semicolon.");

sdrkit_core::settings! {
    /// The settings of the SELCAL stack.
    pub enum SelcalSetting {
        /// The demodulator.
        Demod = DEMOD,
        /// Play the channel audio.
        Listen = LISTEN,
        /// The samples are sound card audio.
        AudioInput = AUDIO_INPUT,
        /// The table of codes and aircraft.
        AircraftTable = AIRCRAFT_TABLE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Listen`].
    pub enum ListenSetting {
        /// Play the channel audio.
        Listen = LISTEN,
    }
}

/// Passes the channel audio on when listening, else silence of the same length.
#[derive(Default)]
pub struct Listen {
    on: bool,
}

impl Component for Listen {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.selcal_listen",
            vec![Port::real("in")],
            vec![Port::real("out").same_format_as(0)],
        )
        .with_settings(ListenSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let on = self.on;
        let o = io.real_out(0);
        if on {
            o.extend_from_slice(x);
        } else {
            o.resize(o.len() + x.len(), 0.0);
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ListenSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ListenSetting::Listen => self.on = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(ListenSetting::Listen, Value::Bool(self.on))])
    }
}

/// The SELCAL receiver graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`], and [`GraphError::Setting`] if the audio resampler refuses its rate.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let resample = AudioResampler::new(48e3).map_err(|error| GraphError::Setting {
        key: "resample.rate".into(),
        error,
    })?;
    GraphBuilder::new()
        .add("front", front::Front::default())
        .add("detector", detector::Detector::default())
        .add("tables", tables::Tables::default())
        .add("listen", Listen::default())
        .add("resample", resample)
        .input("front")
        .connect("front.audio", "detector.audio")
        .connect("front.info", "detector.info")
        .connect("detector.calls", "tables.calls")
        .connect("detector.measures", "tables.measures")
        .connect("front.audio", "listen")
        .connect("listen", "resample")
        .output("resample")
        .output("detector.calls")
        .clock(0.25)
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The SELCAL receiver: the stack graph, whose least rate follows the `audio_input` setting.
pub struct SelcalReceiver {
    inner: StackGraph,
}

impl SelcalReceiver {
    fn min_rate(audio_input: bool) -> f64 {
        if audio_input {
            MIN_AUDIO_RATE_HZ
        } else {
            MIN_RATE_HZ
        }
    }
}

impl Owner for SelcalReceiver {
    fn schema(&self) -> &Arc<Schema> {
        self.inner.schema()
    }
    fn values(&self) -> Option<&Values> {
        Owner::values(&self.inner)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        Owner::apply(&mut self.inner, id, value)?;
        if id == SelcalSetting::AudioInput.id() {
            self.inner
                .set_min_rate(Self::min_rate(AUDIO_INPUT.read(id, value)?));
        }
        Ok(())
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.inner.results_decl()
    }
    fn results(&self) -> &Results {
        Owner::results(&self.inner)
    }
    fn events(&self) -> &[EventDecl] {
        self.inner.events()
    }
    fn commands(&self) -> &Commands {
        Owner::commands(&self.inner)
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        Owner::invoke(&mut self.inner, id, args)
    }
}

impl Receiver for SelcalReceiver {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.inner.configure(input)
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        self.inner.feed(iq, out)
    }
    fn finish(&mut self, out: &mut Output) {
        self.inner.finish(out)
    }
    fn retuned(&mut self, input: &InputInfo) {
        self.inner.retuned(input)
    }
    fn forget(&mut self) {
        self.inner.forget()
    }
    fn set_station(&mut self, station: &Station) {
        self.inner.set_station(station)
    }
    fn set_facilities(&mut self, facilities: &sdrkit_core::Facilities) {
        self.inner.set_facilities(facilities)
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.inner.save_state()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.inner.restore_state(state)
    }
}

/// The SELCAL receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<SelcalReceiver, GraphError> {
    let audio = cfg.settings.get(&AUDIO_INPUT);
    let inner = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            SelcalReceiver::min_rate(audio),
            "SELCAL needs a sample rate of at least 500 kHz, or 8 kHz of sound card audio",
        )
        .default_centre(8.891e6)
        .route(DEMOD.key(), &["front.demod"])?
        .route(AUDIO_INPUT.key(), &["front.audio_input"])?
        .route(LISTEN.key(), &["listen.listen"])?
        .route(AIRCRAFT_TABLE.key(), &["tables.aircraft_table"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("tables", "remove"),
        )
        .with_settings(&cfg.settings)
        .build()?;
    Ok(SelcalReceiver { inner })
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("avdata.selcal_front", || Box::new(front::Front::default()));
    r.add("avdata.selcal_detector", || {
        Box::new(detector::Detector::default())
    });
    r.add("avdata.selcal_tables", || {
        Box::new(tables::Tables::default())
    });
    r.add("avdata.selcal_listen", || Box::new(Listen::default()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(SelcalGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c =
        VerifyCase::new("test signal, FM, three calls", o, 0.0).with(&DEMOD, DemodChoice::Fm);
    c.seconds = o.seconds.max(22.0);
    c.check = Some(check_calls);
    vec![c]
}

fn check_calls(r: &Results) -> Vec<CheckLine> {
    vec![verify::at_least(r, "calls", 2.0, "calls")]
}

/// The SELCAL stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "SELCAL (aircraft tone call)",
    group: "Aviation",
    blurb: "Detects the two-tone calls ground stations send to alert an aircraft's crew, on HF or VHF voice channels.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        min_hz: 1.5e6,
        max_hz: 400e6,
        default_hz: 8.891e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 6e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: SelcalSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Selcal],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
