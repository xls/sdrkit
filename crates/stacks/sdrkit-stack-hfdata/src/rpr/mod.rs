//! Robust Packet: the physical layer only, receive only. The waveform is eight differential PSK carriers 60 Hz apart around
//! 1500 Hz on upper-sideband audio, 50 Bd on every carrier, D-BPSK at speed level 1 and D-QPSK at speed level 2 (the maker's
//! data sheet). The convolutional code, its puncturing, the scrambler, the interleaver, the CRC and the preamble content are
//! not public, so nothing is decoded: the stack finds the comb, the bursts and the symbol clock, measures them, and gives the
//! differential phase steps of every carrier.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz, gain 1] -> [audio buffer 30 s] -> (every 4 s) [analyser] -> rpr.analysis -> [events, tables] -> results
//! ```

pub mod analyse;
pub mod gen;
pub mod rx;

use crate::frontend::add_front;
use crate::receiver::GEN_SECTION;
use crate::sideband::{Demod, SidebandAudio, SIDEBAND};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// The carriers.
pub const CARRIERS: usize = 8;
/// The carrier spacing, Hz.
pub const SPACING_HZ: f64 = 60.0;
/// The nominal audio centre of the comb, Hz.
pub const CENTRE_HZ: f64 = 1500.0;
/// The symbol rate of every carrier, Bd.
pub const BAUD: f64 = 50.0;
/// The guard of a symbol: the symbol period less the active part of 1/60 s, seconds.
pub const GUARD_S: f64 = 1.0 / BAUD - 1.0 / SPACING_HZ;
/// The preamble symbols of each carrier.
pub const PREAMBLE: usize = 8;
/// The ramp symbols before and after a burst.
pub const RAMP: usize = 3;

sdrkit_core::choice_enum! {
    /// The speed level of the test signal.
    pub enum GenSpeed {
        /// D-BPSK, 200 bit/s after the code.
        Level1 = ("level1", "Level 1 (D-BPSK)"),
        /// D-QPSK, 600 bit/s after the code.
        Level2 = ("level2", "Level 2 (D-QPSK)"),
    }
}

/// The speed level of the test signal.
pub const GEN_SPEED: Setting<field::Choice<GenSpeed>> =
    Setting::choice("gen_speed", "Speed level", GenSpeed::Level1)
        .section(GEN_SECTION)
        .advanced();

/// The silence between two bursts of the test signal.
pub const GEN_GAP_S: Setting<field::Duration> = Setting::new("gen_gap_s", "Gap between bursts", "s", 1.5)
    .range(0.1, 30.0, 0.1)
    .section(GEN_SECTION)
    .advanced();

/// The nominal audio centre of the comb: the comb is searched within 500 Hz of it and its offset is measured from it.
pub const AUDIO_CENTRE: Setting<field::Hz> =
    Setting::new("audio_centre_hz", "Audio centre", "Hz", CENTRE_HZ)
        .range(1000.0, 2600.0, 1.0)
        .help("The audio frequency of the middle of the eight carriers when the dial is right.");

sdrkit_core::settings! {
    /// The settings of the Robust Packet stack.
    pub enum RprSetting {
        /// Analysis interval.
        Interval = crate::pactorhf::rx::INTERVAL,
        /// Audio centre.
        AudioCentre = AUDIO_CENTRE,
        /// Upper or lower sideband.
        Sideband = SIDEBAND,
        /// Speed level of the test signal.
        GenSpeed = GEN_SPEED,
        /// Gap between the bursts of the test signal.
        GenGap = GEN_GAP_S,
    }
}

/// The graph of the stack.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    add_front(GraphBuilder::new(), "front")
        .add("sb", SidebandAudio::linear(Demod::Usb))
        .add("buffer", rx::RprBuffer::default())
        .add("results", rx::RprResults::default())
        .input("front")
        .connect("front", "sb")
        .connect("sb.audio", "buffer")
        .connect("buffer", "results")
        .output("results")
        .clock(0.25)
        .status_from("results")
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("rpr"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(24e3, "Robust Packet: the sample rate is too low (24 kHz at least)")
    .min_rate_live(
        250e3,
        "Robust Packet: the sample rate of a live radio is too low (250 kHz at least)",
    )
    .route("analysis_interval_s", &["buffer.analysis_interval_s"])?
    .route("audio_centre_hz", &["buffer.audio_centre_hz"])?
    .route("sideband", &["sb.sideband"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_rx(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::RprGenerator::from_config(cfg)))
}

/// Robust Packet (physical layer only).
pub static RPR: StackDescriptor = StackDescriptor {
    id: "rpr",
    name: "Robust Packet (physical layer)",
    group: "HF utility",
    blurb: "Robust Packet physical layer: the eight-carrier comb, the bursts, D-BPSK or D-QPSK and the phase steps (the coding is not public; no frames).",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(10.1473e6, 2e6, 250e3, 4e3)
    },
    submodes: &[],
    settings: RprSetting::schema,
    make_receiver: make_rx,
    make_generator: Some(make_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::RPR),
    benchmarks: None,
};
