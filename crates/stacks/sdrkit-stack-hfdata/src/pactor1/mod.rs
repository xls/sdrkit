//! PACTOR-I, experimental, receive only.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz] --+--> [tone pair finder] -> pactor.pair --+
//!                                                      +--> [burst detector (audio history)] <--+-> pactor.burst -> [burst reader] -> pactor.frame -> [link] -> results
//!                                                      +--> [listen] -> audio
//! ```

pub mod gen;
pub mod link;
pub mod proto;
pub mod rx;

use crate::frontend::add_front;
use crate::listen::Listen;
use crate::listen::LISTEN;
use crate::receiver::GEN_SECTION;
use crate::sideband::SidebandAudio;
use crate::sideband::DEMOD;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// The presets shared by the four PACTOR stacks.
pub const PRESETS: &[(&str, f64)] = &[
    ("80 m", 3.590e6),
    ("40 m", 7.045e6),
    ("30 m", 10.140e6),
    ("20 m", 14.080e6),
    ("17 m", 18.100e6),
    ("15 m", 21.090e6),
    ("10 m", 28.120e6),
];

/// The tooltip of the presets.
pub const PRESETS_HELP: &str =
    "PACTOR has no calling frequency: these buttons only jump to the digital part of each band.";

sdrkit_core::choice_enum! {
    /// The script of the test signal.
    pub enum GenScript {
        /// An ARQ link.
        Arq = ("arq", "ARQ"),
        /// Unproto (FEC) broadcasts.
        Fec = ("fec", "Unproto (FEC)"),
    }
}

/// The script of the test signal.
pub const GEN_SCRIPT: Setting<field::Choice<GenScript>> =
    Setting::choice("gen_script", "Script", GenScript::Arq)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the PACTOR-I stack.
    pub enum Pactor1Setting {
        /// Listen.
        Listen = LISTEN,
        /// Demodulator.
        Demod = DEMOD,
        /// Huffman table.
        HuffmanTable = link::HUFFMAN_TABLE,
        /// Script of the test signal.
        GenScript = GEN_SCRIPT,
    }
}

/// The PACTOR-I receiver graph.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    add_front(GraphBuilder::new(), "front")
        .add("sb", SidebandAudio::default())
        .add("pair", rx::TonePairFinder::default())
        .add("bursts", rx::BurstDetectorComponent::default())
        .add("reader", rx::BurstReaderComponent::default())
        .add("link", link::PactorLink::default())
        .add("listen", Listen::default())
        .input("front")
        .connect("front", "sb")
        .connect("sb.audio", "pair")
        .connect("sb.audio", "bursts.audio")
        .connect("pair", "bursts.pair")
        .connect("sb.audio", "reader.audio")
        .connect("bursts", "reader.bursts")
        .connect("reader", "link.frames")
        .connect("pair", "link.pair")
        .connect("sb.power", "link.power")
        .connect("sb.audio", "listen")
        .output("link")
        .output("listen")
        .clock(0.25)
        .status_from("link")
}

/// The PACTOR-I receiver.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("pactor1"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        24e3,
        "PACTOR-I: the sample rate is too low (24 kHz at least)",
    )
    .min_rate_live(
        250e3,
        "PACTOR-I: the sample rate of a live radio is too low (250 kHz at least)",
    )
    .route("listen", &["listen.listen"])?
    .route("demod", &["sb.demod"])?
    .route("huffman_table", &["link.huffman_table"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command("remove", sdrkit_graph::CommandTarget::of("link", "remove"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::Pactor1Generator::from_config(cfg)))
}

/// PACTOR-I (experimental).
pub static PACTOR1: StackDescriptor = StackDescriptor {
    id: "pactor1",
    name: "PACTOR-I (experimental)",
    group: "HF utility",
    blurb: "PACTOR-I monitor: both stations of an ARQ link and unproto broadcasts, with memory ARQ, text and callsigns (experimental).",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(14.080e6, 2e6, 250e3, 4e3)
    },
    submodes: &[],
    settings: Pactor1Setting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Pactor1],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PACTOR1),
    benchmarks: None,
};
