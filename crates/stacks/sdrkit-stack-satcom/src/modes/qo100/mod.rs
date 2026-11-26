//! QO-100 (`qo100`): Es'hail-2 through a Ku-band LNB. Beacon lock and LNB correction, the narrowband and wideband spectra, the
//! signal tracker with its classes and conversations, and the DATV carriers. The stack sends SSB voice on the uplink of the
//! narrowband transponder ([`tx`]).

pub mod display;
pub mod gen;
pub mod rx;
pub mod tx;

use crate::typed::{QO100_CONVERSATION_LISTEN, QO100_SPEECH_TEXT};
use display::QoDisplay;
use rx::QoReceiver;
use sdrkit_algo_dsp::converter::Converter;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, ConfigError, DeclareSetting, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{
    Catalog, Decoder, DecoderComponent, GraphBuilder, GraphError, Registry, StackGraph,
};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "qo100";
/// The stack name.
pub const NAME: &str = "QO-100";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 2e6;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The low edge of the narrowband transponder, Hz.
pub const NB_LO: f64 = 10489.5e6;
/// The high edge of the narrowband transponder, Hz.
pub const NB_HI: f64 = 10490.0e6;
/// The lower CW beacon, Hz.
pub const BEACON_LOW: f64 = 10489.5e6;
/// The 400 bit/s BPSK beacon, Hz.
pub const BEACON_PSK: f64 = 10489.75e6;
/// The upper CW beacon, Hz.
pub const BEACON_HIGH: f64 = 10490.0e6;
/// The DATV beacon, Hz.
pub const DATV_BEACON: f64 = 10491.5e6;
/// The narrowband channel's rate: 500 kHz around the transponder and room for the LNB's error.
pub const NB_RATE: f64 = 640e3;

sdrkit_core::choice_enum! {
    /// The local oscillator of the LNB.
    pub enum LoChoice {
        /// 9750 MHz.
        L9750 = ("9750", "9750 MHz"),
        /// 9739 MHz.
        L9739 = ("9739", "9739 MHz"),
        /// 10000 MHz.
        L10000 = ("10000", "10000 MHz"),
        /// 10600 MHz, which mirrors the spectrum.
        L10600 = ("10600", "10600 MHz (inverted)"),
        /// The custom value.
        Custom = ("custom", "Custom"),
    }
}

/// Returns the LNB of a choice of the setting `lo`, or of the custom LO `custom_mhz` in MHz.
pub fn converter_of(choice: LoChoice, custom_mhz: f64) -> Converter {
    Converter::down(lo_of(choice, custom_mhz), NB_HI)
}

/// Returns the LO of a choice of the setting `lo`, Hz, or the custom value given in MHz.
pub fn lo_of(choice: LoChoice, custom_mhz: f64) -> f64 {
    match choice {
        LoChoice::L9750 => 9750e6,
        LoChoice::L9739 => 9739e6,
        LoChoice::L10000 => 10000e6,
        LoChoice::L10600 => 10600e6,
        LoChoice::Custom => custom_mhz * 1e6,
    }
}

/// The radio's centre (RF) for a sample rate: the whole downlink at 9 Msps and more, else the narrowband transponder.
pub fn default_centre_rf(rate: f64) -> f64 {
    if rate >= 9e6 {
        10494.0e6
    } else {
        10490.1e6
    }
}

/// The settings of the receiver, the display and the test signal, in that order.
pub(crate) const SETTINGS: &[&dyn DeclareSetting] = &[
    &rx::LO,
    &rx::LO_CUSTOM_MHZ,
    &rx::AUTO_CORRECTION,
    &rx::LNB_PPM,
    &display::DIAL,
    &display::AUTO_LISTEN,
    &display::CONVERSATION_GAP,
    &tx::TX_LEVEL,
    &tx::TX_TIMEOUT,
    &gen::GEN_EMPTY,
    &gen::GEN_CENTRE_MHZ,
    &gen::GEN_LO,
];

/// Every setting of the stack. Noise and the LNB error of the test signal come from the standard `gen_*` settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(Schema::of(SETTINGS)))
        .clone()
}

/// The receiver: both transponders, then the display, in one graph; the voice sender takes the same input and the audio
/// of the host, and writes into the transmit sink.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut b = GraphBuilder::new()
        .add("receiver", DecoderComponent::new(QoReceiver::default()))
        .add("display", DecoderComponent::new(QoDisplay::default()))
        .add("mic", sdrkit_graph::components::HostIn::default())
        .add("sender", tx::QoSender::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("receiver.in")
        .input("mic.in")
        .input("sender.in")
        .connect("mic.out", "sender.mic")
        .connect("sender.out", "tx.in")
        .connect("receiver.out", "display.in")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S);
    for s in QoReceiver::default().settings().settings() {
        b = b.alias(&s.field.key, &format!("receiver.{}", s.field.key));
    }
    for s in QoDisplay::default().settings().settings() {
        b = b.alias(&s.field.key, &format!("display.{}", s.field.key));
    }
    for key in [tx::TX_LEVEL.key(), tx::TX_TIMEOUT.key()] {
        b = b.alias(key, &format!("sender.{key}"));
    }
    let g = b.catalog(&Catalog::of(cfg)?).build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(MIN_RATE, "QO-100 needs a sample rate of at least 2000 ksps")
        .route(display::DIAL.key(), &["display.dial_hz", "sender.dial_hz"])?
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            QO100_SPEECH_TEXT.command.key,
            sdrkit_graph::CommandTarget::of("display", QO100_SPEECH_TEXT.command.key),
        )
        .command(
            QO100_CONVERSATION_LISTEN.command.key,
            sdrkit_graph::CommandTarget::of("display", QO100_CONVERSATION_LISTEN.command.key),
        )
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        )
        .command(
            crate::typed::QO100_PTT.command.key,
            sdrkit_graph::CommandTarget::of("sender", crate::typed::QO100_PTT.command.key),
        )
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::QoGenerator::from_config(cfg)))
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// Adds the components of the stack.
pub fn register(r: &mut Registry) {
    r.add("satcom.qo100_receiver", || {
        Box::new(DecoderComponent::new(QoReceiver::default()))
    });
    r.add("satcom.qo100_display", || {
        Box::new(DecoderComponent::new(QoDisplay::default()))
    });
}

/// The verification of the stack: the beacon lock on the test signal with a 27 kHz LNB error.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, an LNB 27 kHz high", o, -27e3);
    c.seconds = c.seconds.max(6.0);
    c.expect_audio = Some(false);
    // The LNB 27 kHz high moves the IF 27 kHz down (the 9750 MHz LO is below the downlink), at the stack's rate and centre.
    c.offset_hz = -27e3;
    c.check = Some(check_lock);
    vec![c]
}

fn check_lock(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let err = crate::field_opt(r, "lnb_error_hz").unwrap_or(f64::NAN);
    vec![CheckLine::new(
        "lnb_error_hz",
        (err - 27e3).abs() < 200.0,
        format!("LNB error {err:.0} Hz, 27000 Hz sent"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "QO-100 through a Ku LNB: beacon lock and LNB correction, the transponders' signals, conversations and DATV carriers. Sends SSB voice on the narrowband uplink.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 50e6,
        max_hz: 1200e6,
        default_hz: 744.0e6,
        rate_hz: 10e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 10e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn q1_helpers() {
        let low = converter_of(LoChoice::L9750, 0.0);
        let high = converter_of(LoChoice::L10600, 0.0);
        assert!((low.if_of(10489.5e6) - 739.5e6).abs() < 1.0);
        assert!((high.if_of(10489.5e6) - 110.5e6).abs() < 1.0);
        assert!((low.rf_of(739.5e6) - 10489.5e6).abs() < 1.0);
        assert!((high.rf_of(110.5e6) - 10489.5e6).abs() < 1.0);
        assert!((DESCRIPTOR.tuning.default_hz - 744.0e6).abs() < 1.0);
        assert!((low.if_of(default_centre_rf(10e6)) - 744.0e6).abs() < 1.0);
        assert_eq!(lo_of(LoChoice::Custom, 9751.5), 9751.5e6);
    }
}
