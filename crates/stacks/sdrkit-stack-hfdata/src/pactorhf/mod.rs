//! PACTOR-II, -III and -IV: the physical layer only, receive only. Their channel coding is
//! proprietary and not published, so nothing is decoded: the stack finds the carriers, the symbol clock, the phase steps and the
//! repeating frame structure.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz, USB] -> [audio buffer 40 s] -> (every 4 s) [analyser] -> pactorhf.analysis -> [events, tables] -> results
//! ```

pub mod analyse;
pub mod gen;
pub mod rx;

use crate::frontend::add_front;
use crate::receiver::GEN_SECTION;
use crate::sideband::Demod;
use crate::sideband::SidebandAudio;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// The PACTOR-III speed level of the test signal.
pub const GEN_LEVEL: Setting<field::I64> = Setting::new("gen_level", "Speed level", "", 3)
    .range(1.0, 6.0, 1.0)
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::choice_enum! {
    /// The PACTOR-IV constellation of the test signal.
    pub enum GenConstellation {
        /// 2 points.
        C2 = ("2", "2 points"),
        /// 4 points.
        C4 = ("4", "4 points"),
        /// 8 points.
        C8 = ("8", "8 points"),
        /// 16 points.
        C16 = ("16", "16 points"),
    }
}

impl GenConstellation {
    /// The number of points.
    pub fn points(self) -> u32 {
        match self {
            GenConstellation::C2 => 2,
            GenConstellation::C4 => 4,
            GenConstellation::C8 => 8,
            GenConstellation::C16 => 16,
        }
    }
}

/// The PACTOR-IV constellation of the test signal.
pub const GEN_CONSTELLATION: Setting<field::Choice<GenConstellation>> =
    Setting::choice("gen_constellation", "Constellation", GenConstellation::C4)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the PACTOR-II stack.
    pub enum Pactor2Setting {
        /// Analysis interval.
        Interval = rx::INTERVAL,
    }
}

sdrkit_core::settings! {
    /// The settings of the PACTOR-III stack.
    pub enum Pactor3Setting {
        /// Analysis interval.
        Interval = rx::INTERVAL,
        /// Speed level of the test signal.
        GenLevel = GEN_LEVEL,
    }
}

sdrkit_core::settings! {
    /// The settings of the PACTOR-IV stack.
    pub enum Pactor4Setting {
        /// Analysis interval.
        Interval = rx::INTERVAL,
        /// Constellation of the test signal.
        GenConstellation = GEN_CONSTELLATION,
    }
}

/// The graph for the requested variant (2, 3 or 4).
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph(asked: u8) -> Result<Graph, GraphError> {
    builder(asked).build()
}

fn builder(asked: u8) -> GraphBuilder {
    add_front(GraphBuilder::new(), "front")
        .add("sb", SidebandAudio::new(Demod::Usb))
        .add("buffer", rx::AudioBuffer::new(asked))
        .add("results", rx::PactorHfResults::new(asked))
        .input("front")
        .connect("front", "sb")
        .connect("sb.audio", "buffer")
        .connect("buffer", "results")
        .output("results")
        .clock(0.25)
        .status_from("results")
}

/// The receiver for a variant.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(asked: u8, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let (id, low, live) = match asked {
        2 => (
            "pactor2",
            "PACTOR-II: the sample rate is too low (24 kHz at least)",
            "PACTOR-II: the sample rate of a live radio is too low (250 kHz at least)",
        ),
        3 => (
            "pactor3",
            "PACTOR-III: the sample rate is too low (24 kHz at least)",
            "PACTOR-III: the sample rate of a live radio is too low (250 kHz at least)",
        ),
        _ => (
            "pactor4",
            "PACTOR-IV: the sample rate is too low (24 kHz at least)",
            "PACTOR-IV: the sample rate of a live radio is too low (250 kHz at least)",
        ),
    };
    StackGraph::of(
        crate::typed::shape(id),
        builder(asked).catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(24e3, low)
    .min_rate_live(250e3, live)
    .route("analysis_interval_s", &["buffer.analysis_interval_s"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .with_settings(&cfg.settings)
    .build()
}

fn rx2(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(2, cfg)?))
}
fn rx3(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(3, cfg)?))
}
fn rx4(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(4, cfg)?))
}
fn gen2(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::PactorHfGenerator::from_config(cfg, 2)))
}
fn gen3(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::PactorHfGenerator::from_config(cfg, 3)))
}
fn gen4(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::PactorHfGenerator::from_config(cfg, 4)))
}

const TUNING: Tuning = Tuning {
    tune_offset_hz: 20e3,
    radio_filter_hz: Some(1.75e6),
    ..Tuning::anywhere(14.072e6, 2e6, 250e3, 4e3)
};

/// PACTOR-II (physical layer only).
pub static PACTOR2: StackDescriptor = StackDescriptor {
    id: "pactor2",
    name: "PACTOR-II (physical layer)",
    group: "HF utility",
    blurb: "PACTOR-II physical layer: the two carriers, the symbol clock and the frame structure (the coding is proprietary; no text).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..TUNING
    },
    submodes: &[],
    settings: Pactor2Setting::schema,
    make_receiver: rx2,
    make_generator: Some(gen2),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Pactor2],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PACTOR2),
    benchmarks: None,
};

/// PACTOR-III (physical layer only).
pub static PACTOR3: StackDescriptor = StackDescriptor {
    id: "pactor3",
    name: "PACTOR-III (physical layer)",
    group: "HF utility",
    blurb: "PACTOR-III physical layer: the carriers and speed level, the symbol clock and the frame structure (no text).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..TUNING
    },
    submodes: &[],
    settings: Pactor3Setting::schema,
    make_receiver: rx3,
    make_generator: Some(gen3),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Pactor3],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PACTOR3),
    benchmarks: None,
};

/// PACTOR-IV (physical layer only).
pub static PACTOR4: StackDescriptor = StackDescriptor {
    id: "pactor4",
    name: "PACTOR-IV (physical layer)",
    group: "HF utility",
    blurb: "PACTOR-IV physical layer: the 1800 baud carrier and an estimate of its constellation (no text).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        ..TUNING
    },
    submodes: &[],
    settings: Pactor4Setting::schema,
    make_receiver: rx4,
    make_generator: Some(gen4),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Pactor4],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::PACTOR4),
    benchmarks: None,
};
