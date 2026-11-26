//! The wave-buoy HF link (Datawell Waverider), receive only: the 2FSK HXV format decoded, the 4FSK HVA
//! format measured.
//!
//! ```text
//! iq -> [front ~16 kHz] --+--> [carrier finder] -> wavebuoy.carrier --+--> [FSK demodulator] -> wavebuoy.symbol --+--> [HXV decoder] -> wavebuoy.vector --+
//!                         +---------------------------------------------+                                        +--> [frame analyser] -> wavebuoy.frame -+--> [results]
//! ```

pub mod frames;
pub mod gen;
pub mod hxv;
pub mod results;
pub mod rx;

use crate::receiver::GEN_SECTION;
use gen::BuoySignal;
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// The signal of the test signal.
pub const GEN_SIGNAL: Setting<field::Choice<BuoySignal>> =
    Setting::choice("gen_signal", "Signal", BuoySignal::Fsk2)
        .section(GEN_SECTION)
        .advanced();
/// The test signal sends its tones the other way round.
pub const GEN_INVERTED_TONES: Setting<field::Bool> =
    Setting::new("gen_inverted_tones", "Inverted tones", "", false)
        .section(GEN_SECTION)
        .advanced();
/// The test signal sends WR-SG vectors.
pub const GEN_WRSG: Setting<field::Bool> = Setting::new("gen_wrsg", "WR-SG", "", false)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the wave-buoy stack. Noise, clock error and frequency errors of the test signal come from the standard
    /// `gen_*` settings.
    pub enum BuoySetting {
        /// Format.
        Format = rx::FORMAT,
        /// Signal of the test signal.
        GenSignal = GEN_SIGNAL,
        /// Inverted tones of the test signal.
        GenInvertedTones = GEN_INVERTED_TONES,
        /// WR-SG vectors of the test signal.
        GenWrsg = GEN_WRSG,
    }
}

/// The wave-buoy receiver graph.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("front", rx::BuoyFront::default())
        .add("carrier", rx::CarrierFinder::default())
        .add("demod", rx::FskDemod::default())
        .add("hxv", rx::HxvDecoder::default())
        .add("frames", rx::FrameAnalyser::default())
        .add("results", results::BuoyResults::default())
        .input("front")
        .connect("front", "carrier")
        .connect("front", "demod.in")
        .connect("carrier", "demod.carrier")
        .connect("demod", "hxv")
        .connect("demod", "frames")
        .connect("carrier", "results.carrier")
        .connect("demod", "results.symbols")
        .connect("hxv", "results.vectors")
        .connect("frames", "results.frames")
        .output("results")
        .clock(0.25)
        .status_from("results")
}

/// The wave-buoy receiver.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("wavebuoy"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        24e3,
        "Wave buoy: the sample rate is too low (24 kHz at least)",
    )
    .route("format", &["carrier.format"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::BuoyGenerator::from_config(cfg)))
}

/// The wave-buoy HF link.
pub static WAVEBUOY: StackDescriptor = StackDescriptor {
    id: "wavebuoy",
    name: "Wave-buoy HF link (Datawell)",
    group: "HF utility",
    blurb: "Datawell wave buoys on HF: heave, north and west, the sea state and the wave spectrum from the 2FSK HXV link (receive only).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..Tuning::anywhere(30.475e6, 250e3, 24e3, 250e3)
    },
    submodes: &[],
    settings: BuoySetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::WAVEBUOY),
    benchmarks: None,
};
