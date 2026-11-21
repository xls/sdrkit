//! Radiosondes (`sonde`): RS41, RS92, DFM-06, DFM-09, DFM-17, M10, M20 and LMS6-1680. The graph is a carrier search, a bank of
//! channels with their frame decoders, and the sonde table.

pub mod channel;
pub mod dfm;
pub mod frame;
pub mod generator;
pub mod landing;
pub mod lms6;
pub mod m10;
pub mod rs41;
pub mod rs92;
pub mod search;
pub mod slicer;
pub mod table;

use crate::family::REPORT_S;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{ConfigError, GenConfig, Generator, Receiver, Setting, StackConfig};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use sdrkit_util_objects::commands as objcmd;
use crate::typed::{SONDE_REMOVE, SONDE_SELECT};
use table::{ASCENT_WEIGHT, GROUND, LANDING_LAYER, LANDING_LAYERS, LANDING_WINDOW};

/// The lowest input rate.
pub const MIN_RATE: f64 = 250e3;

/// The number of sondes in the test signal.
pub const GEN_COUNT: Setting<field::I64> = Setting::new("gen_count", "Test signal: sondes", "", 3)
    .range(1.0, 7.0, 1.0)
    .section("Test signal")
    .advanced();
/// The families of the test signal.
pub const GEN_TYPES: Setting<field::Text> = Setting::new(
    "gen_types",
    "Test signal: types",
    "",
    "rs41,dfm,m10",
)
.help("The keys of the sonde families, separated by commas (rs41, rs92, dfm, m10, m20, lms6).")
.section("Test signal")
.advanced();
/// The east drift of the test signal's sondes.
pub const GEN_DRIFT: Setting<field::F64> =
    Setting::new("gen_drift_ms", "Test signal: east drift", "m/s", 10.0)
        .range(-50.0, 50.0, 0.1)
        .section("Test signal")
        .advanced();
/// The offset of the second RS41 of the test signal.
pub const GEN_RS41B: Setting<field::F64> = Setting::new(
    "gen_rs41b_khz",
    "Test signal: second RS41 offset",
    "kHz",
    450.0,
)
.range(-3000.0, 3000.0, 1.0)
.section("Test signal")
.advanced();
/// The test signal is the LMS6 sonde of the 1680 MHz band.
pub const GEN_LBAND: Setting<field::Bool> =
    Setting::new("gen_lband", "Test signal: 1680 MHz (LMS6)", "", false)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the radiosonde stack.
    pub enum SondeSetting {
        /// The sondes followed at once.
        Channels = channel::CHANNELS,
        /// The time over which landing predictions are smoothed.
        LandingWindow = LANDING_WINDOW,
        /// Use the winds measured on the ascent.
        LandingLayers = LANDING_LAYERS,
        /// The height of a wind layer.
        LandingLayer = LANDING_LAYER,
        /// The weight of the ascent winds.
        AscentWeight = ASCENT_WEIGHT,
        /// The height of the ground.
        Ground = GROUND,
        /// The number of sondes in the test signal.
        GenCount = GEN_COUNT,
        /// The families of the test signal.
        GenTypes = GEN_TYPES,
        /// The east drift of the test signal's sondes.
        GenDrift = GEN_DRIFT,
        /// The offset of the second RS41 of the test signal.
        GenRs41b = GEN_RS41B,
        /// The test signal is the LMS6 sonde.
        GenLband = GEN_LBAND,
    }
}

/// The receiver: the carrier search, the channel bank and the sonde table.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build).
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("search", search::CarrierSearch::default())
        .add("bank", channel::ChannelBank::default())
        .add("table", table::SondeTable::default())
        .input("search")
        .input("bank.in")
        .input("table.in")
        .connect("search", "bank.carriers")
        .connect("bank.frames", "table.frames")
        .connect("bank.report", "table.report")
        .output("table.events")
        .clock(REPORT_S)
        .status_from("table")
        .declare(&table::declaration())
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let mut rx = StackGraph::of(crate::typed::shape("sonde"), graph)
        .min_rate(
            MIN_RATE,
            "Radiosondes need a sample rate of at least 250 ksps",
        )
        .route(channel::CHANNELS.key(), &["bank.channels"])?;
    for k in [
        LANDING_WINDOW.key(),
        LANDING_LAYERS.key(),
        LANDING_LAYER.key(),
        ASCENT_WEIGHT.key(),
        GROUND.key(),
    ] {
        rx = rx.route(k, &[&format!("table.{k}")])?;
    }
    rx.command(
        objcmd::CLEAR.key,
        sdrkit_graph::CommandTarget::of("table", "clear"),
    )
    .command(
        SONDE_REMOVE.command.key,
        sdrkit_graph::CommandTarget::of("table", "remove"),
    )
    .command(
        SONDE_SELECT.command.key,
        sdrkit_graph::CommandTarget::of("table", "select"),
    )
    .with_settings(&cfg.settings)
    .build()
}

/// The receiver as the engine takes it.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

/// The generator of the test signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator::SondeGenerator::from_config(cfg)))
}

/// The verify case: the default scene of three sondes (RS41, DFM-17, M10).
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three sondes", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = crate::family::field(r, crate::typed::SondeFields::SONDES.key);
        vec![CheckLine::new(
            crate::typed::SondeFields::SONDES.key,
            n >= 3.0,
            format!("{n}, 3 expected"),
        )]
    });
    vec![c]
}
