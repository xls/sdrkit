//! The generic PSK demodulator (`psk_demod`): a blind BPSK, QPSK, OQPSK and 8PSK analyser and demodulator for any narrow signal,
//! with bit output.

pub mod bits;
pub mod display;
pub mod gen;
pub mod rx;

use bits::BitStage;
use display::PskDisplay;
use rx::PskRx;
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, DeclareSetting, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{
    Catalog, Decoder, DecoderComponent, GraphBuilder, GraphError, Registry, StackGraph,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "psk_demod";
/// The stack name.
pub const NAME: &str = "PSK demodulator";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 8e3;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// Samples of the channel the analyser looks at.
pub const ANALYSIS_LEN: usize = 8192;
/// Samples per symbol the demodulator works at.
pub const SPS: f64 = 8.0;
/// The most bits kept for the screen.
pub const BITS_CAP: usize = 16_384;

/// The settings of the receiver, the bit chain, the display and the test signal, in that order.
pub(crate) const SETTINGS: &[&dyn DeclareSetting] = &[
    &rx::ORDER,
    &rx::BAUD,
    &rx::SHAPE,
    &rx::SPAN,
    &rx::SNAP_STANDARD,
    &rx::FOLLOW_DOPPLER,
    &rx::CLICK_HZ,
    &bits::DIFFERENTIAL,
    &bits::DESCRAMBLER,
    &bits::SYNC,
    &bits::INVERT,
    &display::SAVE_BITS,
    &gen::GEN_ORDER,
    &gen::GEN_BAUD,
    &gen::GEN_ROLLOFF,
    &gen::GEN_CODING,
    &gen::GEN_SYNC,
    &gen::GEN_CONJUGATE,
];

/// Every setting of the stack. Noise and other impairments of the test signal come from the standard `gen_*` settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(SETTINGS)))
        .clone()
}

/// The receiver: the finder, analyser and demodulator, the bit chain, then the display, in one graph.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut b = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(PskRx::default()))
        .add("bits", DecoderComponent::new(BitStage::default()))
        .add("display", DecoderComponent::new(PskDisplay::default()))
        .input("receiver.in")
        .connect("receiver.out", "bits.in")
        .connect("bits.out", "display.in")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S);
    for (id, schema) in [
        ("receiver", PskRx::default().settings()),
        ("bits", BitStage::default().settings()),
        ("display", PskDisplay::default().settings()),
    ] {
        for s in schema.settings() {
            b = b.alias(&s.field.key, &format!("{id}.{}", s.field.key));
        }
    }
    let g = b.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "PSK demodulator needs a sample rate of at least 8 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + DESCRIPTOR.tuning.tune_offset_hz)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::PskGenerator::from_config(cfg)))
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Adds the components of the stack.
pub fn register(r: &mut Registry) {
    r.add("satcom.psk_demod_receiver", || {
        Box::new(DecoderComponent::new(PskRx::default()))
    });
    r.add("satcom.psk_demod_bits", || {
        Box::new(DecoderComponent::new(BitStage::default()))
    });
    r.add("satcom.psk_demod_display", || {
        Box::new(DecoderComponent::new(PskDisplay::default()))
    });
}

/// The verification of the stack: the test signal (QPSK 4800 Bd) is found, locked and its sync words read.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, QPSK 4800 Bd at the dial", o, 0.0);
    c.seconds = c.seconds.max(3.0);
    c.expect_audio = Some(false);
    c.settings.push(CaseSetting::new(&bits::SYNC, "1ACFFC1D"));
    c.check = Some(check_syncs);
    vec![c]
}

fn check_syncs(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, "frames_ok");
    vec![CheckLine::new(
        "frames_ok",
        n >= 3.0,
        format!("{n} sync words found"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "A blind BPSK / QPSK / OQPSK / 8PSK analyser and demodulator for any narrow signal, with bit output. Receive only.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 6000e6,
        default_hz: 435e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 0.2e6,
        tune_offset_hz: 250e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::NarrowDigital],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
