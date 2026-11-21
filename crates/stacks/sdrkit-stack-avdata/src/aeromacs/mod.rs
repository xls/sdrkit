//! AeroMACS (`aeromacs`): structure only. The 5 MHz OFDMA of the airport surface link is found by its cyclic prefix (512-point FFT and a prefix of 1/8 at 5.6 Msps): the symbol timing, the carrier offset and the bursts of the 5 ms TDD frame. Nothing is decoded.
//! Standard: IEEE Std 802.16-2009 (OFDMA PHY); RTCA DO-346; ICAO Doc 10044.

use crate::ofdm_shape::{self, Row, ShapeDecoder, ShapeGenerator};
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, DecoderComponent, GraphBuilder, GraphError, StackGraph};

/// The stack id.
pub const ID: &str = "aeromacs";
/// The working rate, Hz.
pub const WORK_RATE: f64 = 5.6e6;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 5.6e6;
/// The frequency of the test signal, Hz. The session gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 5095e6;
/// What no public document confirms.
pub const NOTE: &str = "Not confirmed by a public document: the 5 MHz profile (a sampling factor of 28/25 or 5.6 Msps, a 512-point FFT, a carrier spacing of 10.9375 kHz, a prefix of 1/8 or 64 samples) and the 5 ms TDD frame. They are settings. The preamble, FCH and DL-MAP are not decoded.";

/// The FFT length.
pub const FFT_LEN: Setting<field::I64> =
    Setting::new(ofdm_shape::FFT_LEN_KEY, "FFT length", "samples", 512)
        .range(8.0, 4096.0, 1.0)
        .help("The FFT length of the numerology at 5.6 Msps.")
        .advanced();
/// The cyclic prefix.
pub const PREFIX: Setting<field::I64> =
    Setting::new(ofdm_shape::PREFIX_KEY, "Cyclic prefix", "samples", 64)
        .range(1.0, 4096.0, 1.0)
        .help("The cyclic prefix of the numerology at 5.6 Msps.")
        .advanced();
/// The frame period of the envelope fold.
pub const FRAME_MS: Setting<field::F64> =
    Setting::new(ofdm_shape::FRAME_MS_KEY, "Frame period", "ms", 5.0)
        .range(0.1, 10_000.0, 0.1)
        .help("The period over which the power is folded to tell bursts from a continuous signal.")
        .advanced();
/// The share of each frame period with the test signal on.
pub const GEN_DUTY: Setting<field::F64> =
    Setting::new(ofdm_shape::GEN_DUTY_KEY, "Test signal duty", "%", 60.0)
        .range(1.0, 100.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `aeromacs` stack.
    pub enum AeromacsSetting {
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
    pub enum AeromacsRxSetting {
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
    decoder: "avdata.aeromacs",
    title: "AeroMACS",
    rate_hz: WORK_RATE,
    min_rate_hz: MIN_RATE_HZ,
    fft_len: 512,
    prefix: 64,
    frame_ms: 5.0,
    used: 0.82,
    schema: AeromacsSetting::schema,
    rx_schema: AeromacsRxSetting::schema,
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
        .min_rate(
            MIN_RATE_HZ,
            "AeroMACS needs a sample rate of at least 5.6 MHz",
        )
        .route(FFT_LEN.key(), &["ofdm.fft_len"])?
        .route(PREFIX.key(), &["ofdm.prefix"])?
        .route(FRAME_MS.key(), &["ofdm.frame_ms"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("avdata.aeromacs", || {
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

/// The AeroMACS structure detector.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "AeroMACS",
    group: "Aviation",
    blurb: "Structure only: AeroMACS 5 MHz OFDM found by its cyclic prefix, with the carrier offset and the bursts of the 5 ms frame; nothing is decoded.",
    tuning: Tuning::anywhere(TEST_SIGNAL_HZ, 5.6e6, MIN_RATE_HZ, 5e6),
    submodes: &[],
    settings: AeromacsSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Hold,
    verify: Some(&HOOKS),
    benchmarks: None,
};
