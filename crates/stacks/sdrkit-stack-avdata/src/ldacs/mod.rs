//! LDACS (`ldacs`): structure only. The OFDM of the L-band digital aeronautical communication system is found by its cyclic prefix (symbol 120 us, prefix 17.6 us): the symbol timing, the carrier offset and whether the power comes in bursts or continuously over the 240 ms super-frame. Nothing is decoded.
//! Standard: EUROCONTROL / SESAR LDACS A/G specification (numerology).

use crate::ofdm_shape::{self, Row, ShapeDecoder, ShapeGenerator};
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, DecoderComponent, GraphBuilder, GraphError, StackGraph};

/// The stack id.
pub const ID: &str = "ldacs";
/// The working rate, Hz.
pub const WORK_RATE: f64 = 625e3;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 625e3;
/// The frequency of the test signal, Hz. The session gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 1000e6;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the numerology (a 64-point FFT at 625 ksps, a carrier spacing of 9.765625 kHz, a prefix of 11 samples or 17.6 us, of which 12.8 us are a shaped window, so part of the prefix correlates less than a plain prefix) and the 240 ms super-frame. They are settings. The content is not decoded.";

/// The FFT length.
pub const FFT_LEN: Setting<field::I64> =
    Setting::new(ofdm_shape::FFT_LEN_KEY, "FFT length", "samples", 64)
        .range(8.0, 4096.0, 1.0)
        .help("The FFT length of the numerology at 625 ksps.")
        .advanced();
/// The cyclic prefix.
pub const PREFIX: Setting<field::I64> =
    Setting::new(ofdm_shape::PREFIX_KEY, "Cyclic prefix", "samples", 11)
        .range(1.0, 4096.0, 1.0)
        .help("The cyclic prefix of the numerology at 625 ksps.")
        .advanced();
/// The frame period of the envelope fold.
pub const FRAME_MS: Setting<field::F64> =
    Setting::new(ofdm_shape::FRAME_MS_KEY, "Frame period", "ms", 240.0)
        .range(0.1, 10_000.0, 0.1)
        .help("The period over which the power is folded to tell bursts from a continuous signal.")
        .advanced();
/// The share of each frame period with the test signal on.
pub const GEN_DUTY: Setting<field::F64> =
    Setting::new(ofdm_shape::GEN_DUTY_KEY, "Test signal duty", "%", 100.0)
        .range(1.0, 100.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `ldacs` stack.
    pub enum LdacsSetting {
        /// The FFT length.
        FftLen = FFT_LEN,
        /// The cyclic prefix.
        Prefix = PREFIX,
        /// The frame period of the envelope fold.
        FrameMs = FRAME_MS,
        /// The share of each frame period with the test signal on.
        GenDuty = GEN_DUTY,
    }
}

sdrkit_core::settings! {
    /// The settings of the decoder.
    pub enum LdacsRxSetting {
        /// The FFT length.
        FftLen = FFT_LEN,
        /// The cyclic prefix.
        Prefix = PREFIX,
        /// The frame period of the envelope fold.
        FrameMs = FRAME_MS,
    }
}

/// The stack as the structure detector sees it.
pub static ROW: Row = Row {
    id: ID,
    decoder: "avdata.ldacs",
    title: "LDACS",
    rate_hz: WORK_RATE,
    min_rate_hz: MIN_RATE_HZ,
    fft_len: 64,
    prefix: 11,
    frame_ms: 240.0,
    used: 0.78,
    schema: LdacsSetting::schema,
    rx_schema: LdacsRxSetting::schema,
    note: NOTE,
};

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("ofdm", DecoderComponent::new(ShapeDecoder::new(&ROW)))
        .input("ofdm")
        .status_from("ofdm")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE_HZ, "LDACS needs a sample rate of at least 625 kHz")
        .route(FFT_LEN.key(), &["ofdm.fft_len"])?
        .route(PREFIX.key(), &["ofdm.prefix"])?
        .route(FRAME_MS.key(), &["ofdm.frame_ms"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("avdata.ldacs", || {
        Box::new(DecoderComponent::new(ShapeDecoder::new(&ROW)))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ShapeGenerator::from_config(&ROW, cfg)?))
}

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 500.0);
    c.seconds = o.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let d = r
            .fields()
            .at(&crate::typed::OfdmShapeFields::DETECTED)
            .unwrap_or(false);
        vec![CheckLine::new(
            crate::typed::OfdmShapeFields::DETECTED.key,
            d,
            format!("detected: {d}"),
        )]
    });
    vec![c]
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The LDACS structure detector.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "LDACS",
    group: "Aviation",
    blurb: "Structure only: LDACS OFDM found by its cyclic prefix (symbol 120 us, prefix 17.6 us), with the carrier offset and the frame duty; nothing is decoded.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        ..Tuning::anywhere(TEST_SIGNAL_HZ, 2e6, MIN_RATE_HZ, 0.5e6)
    },
    submodes: &[],
    settings: LdacsSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&HOOKS),
    benchmarks: None,
};
