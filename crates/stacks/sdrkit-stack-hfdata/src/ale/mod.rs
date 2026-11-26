//! HF ALE, second generation.
//!
//! Standard: MIL-STD-188-141 Appendix A, FED-STD-1045.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [sideband audio 8 kHz] --+--> [word sync] -> ale.word --+--> [call assembler] -> ale.call --+
//!                                                      |                              +-----------------------------------+--> [tables]
//!                                                      +--> [listen] -> audio
//! iq, ale.call -> [transmitter: call, sound, amd, stop; answers] -> [tx sink]
//! ```

pub mod calls;
pub mod gen;
pub mod sync;
pub mod tables;
pub mod tx;
pub mod word;

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

/// The presets: band and dial frequency.
pub const PRESETS: &[(&str, f64)] = &[
    ("80 m", 3.596e6),
    ("40 m", 7.102e6),
    ("30 m", 10.145e6),
    ("20 m", 14.109e6),
    ("17 m", 18.106e6),
    ("15 m", 21.096e6),
    ("12 m", 24.926e6),
    ("10 m", 28.146e6),
];

sdrkit_core::choice_enum! {
    /// The calls of the test signal.
    pub enum GenCalls {
        /// The test calls and their answers.
        TestCalls = ("test_calls", "The test calls"),
        /// A sounding only.
        Sounding = ("sounding", "The sounding only"),
    }
}

/// The calls of the test signal.
pub const GEN_CALLS: Setting<field::Choice<GenCalls>> =
    Setting::choice("gen_calls", "Calls", GenCalls::TestCalls)
        .section(GEN_SECTION)
        .advanced();
/// The share of channel bits that the test signal turns over.
pub const GEN_BER: Setting<field::F64> = Setting::new("gen_ber", "Channel bit errors", "", 0.0)
    .range(0.0, 0.2, 0.0)
    .help("The share of channel bits turned over before they become tones.")
    .section(GEN_SECTION)
    .advanced();

sdrkit_core::settings! {
    /// The settings of the ALE stack.
    pub enum AleSetting {
        /// Net addresses.
        Nets = tables::NETS,
        /// Listen.
        Listen = LISTEN,
        /// Demodulator.
        Demod = DEMOD,
        /// Own address.
        SelfAddress = tx::SELF_ADDRESS,
        /// Calling cycle repeats.
        CallRepeats = tx::CALL_REPEATS,
        /// Sounding word.
        Sounding = tx::SOUNDING,
        /// Channels of the called station.
        ScanChannels = tx::SCAN_CHANNELS,
        /// Dwell of the called station.
        ScanDwell = tx::SCAN_DWELL_S,
        /// Answer calls.
        AutoAnswer = tx::AUTO_ANSWER,
        /// Exchange link quality.
        Lqa = tx::LQA_EXCHANGE,
        /// Wait for an answer.
        AnswerWait = tx::ANSWER_WAIT_S,
        /// Calls of the test signal.
        GenCalls = GEN_CALLS,
        /// Channel bit errors of the test signal.
        GenBer = GEN_BER,
    }
}

/// The ALE receiver graph.
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
        .add("sync", sync::AleWordSync::default())
        .add("asm", calls::CallAssembler::default())
        .add("tables", tables::AleTables::default())
        .add("listen", Listen::default())
        .add("sender", tx::AleTransmitter::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front", "sb")
        .connect("sb.audio", "sync.audio")
        .connect("sb.power", "sync.power")
        .connect("sync.words", "asm")
        .connect("sync.words", "tables.words")
        .connect("asm", "tables.calls")
        .connect("asm", "sender.calls")
        .connect("sb.audio", "listen")
        .connect("sender.out", "tx.in")
        .output("tables")
        .output("listen")
        .output("sender.events")
        .clock(0.25)
        .status_from("tables")
}

/// The ALE receiver and transmitter: the settings of the transmit section go to the transmitter, and so do `call`,
/// `sound`, `amd` and `stop`.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("ale"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(24e3, "ALE: the sample rate is too low (24 kHz at least)")
    .min_rate_live(
        250e3,
        "ALE: the sample rate of a live radio is too low (250 kHz at least)",
    )
    .route("nets", &["tables.nets"])?
    .route("listen", &["listen.listen"])?
    .route("demod", &["sb.demod"])?
    .route("self_address", &["sender.self_address", "tables.self_address"])?
    .route("call_repeats", &["sender.call_repeats"])?
    .route("sounding", &["sender.sounding"])?
    .route("scan_channels", &["sender.scan_channels"])?
    .route("scan_dwell_s", &["sender.scan_dwell_s"])?
    .route("auto_answer", &["sender.auto_answer"])?
    .route("lqa", &["sender.lqa"])?
    .route("answer_wait_s", &["sender.answer_wait_s"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command(
        "remove",
        sdrkit_graph::CommandTarget::of("tables", "remove"),
    )
    .command("call", sdrkit_graph::CommandTarget::of("sender", "call"))
    .command("sound", sdrkit_graph::CommandTarget::of("sender", "sound"))
    .command("stop", sdrkit_graph::CommandTarget::of("sender", "stop"))
    .command("amd", sdrkit_graph::CommandTarget::of("sender", "amd"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::AleGenerator::from_config(cfg)))
}

/// HF ALE (2G).
pub static ALE: StackDescriptor = StackDescriptor {
    id: "ale",
    name: "HF ALE",
    group: "HF utility",
    blurb: "Automatic link establishment (2G, 8-FSK): soundings, calls, nets and AMD messages, with the stations heard.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Usb, 3e3)),
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(14.109e6, 2e6, 250e3, 3e3)
    },
    submodes: &[],
    settings: AleSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ale2g],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ALE),
    benchmarks: Some(crate::benchmarks),
};
