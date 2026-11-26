//! ISDB-TSB: narrow-band digital sound and data broadcasting of 1 or 3 OFDM segments, the centre of an ISDB-T band. The stack
//! is the ISDB-T receiver with a band of 1 or 3 segments: the same TMCC, layers, equaliser and decoder. With one segment it also
//! takes the one-segment partial-reception service out of a 3-segment ISDB-TSB signal or a 13-segment ISDB-T signal.
//!
//! Standard: ITU-R BS.1114 Annex 3 (System F); ARIB STD-B31 for the parts ISDB-T shares.

use crate::isdbt::gen::{config, layer, IsdbtGenParams, IsdbtGenerator, Payload};
use crate::isdbt::graph::{DeintComp, EqComp, FecComp, MergeComp, Report, SyncComp};
use crate::isdbt::layout::Band;
use crate::isdbt::tmcc::LayerParams;
use crate::isdbt::{EdgeChoice, Guard, SegmentCount, Width, EDGE_PILOT, GUARDS, SEGMENTS, WIDTH};
use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, StageError, Tuning,
};
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_mpegts::layer::{self, TsLayer, DECODE_VIDEO};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "isdbtsb";

/// What the stack decodes and which of its values no public document confirms.
pub const NOTE: &str = "Decodes the TMCC and every layer to the transport stream; the AC bits and the phase-shift field of connected \
    segment transmission are not read, and one channel of a connected transmission is taken alone. Not confirmed by a public \
    document (ARIB STD-B29 was not available): the segment numbers of the 3-segment signal (1, 0, 2 from low to high, the ISDB-T \
    numbers at those places), the value of the continual pilot at the upper edge (setting edge_pilot, default: the pilot \
    sequence's value at that carrier), the TMCC use of the partial-reception flag for three segments, and the TMCC code of the \
    fifth time-interleave length (I = 32, 16, 8 in modes 1, 2, 3), which the stack refuses.";

sdrkit_core::choice_enum! {
    /// The mode of the test signal.
    pub enum GenMode {
        /// Mode 1.
        M1 = ("1", "Mode 1"),
        /// Mode 2.
        M2 = ("2", "Mode 2"),
        /// Mode 3.
        M3 = ("3", "Mode 3"),
    }
}

sdrkit_core::choice_enum! {
    /// The modulation of layer B of the test signal.
    pub enum GenModulation {
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// 16-QAM.
        Qam16 = ("16qam", "16QAM"),
        /// 64-QAM.
        Qam64 = ("64qam", "64QAM"),
    }
}

sdrkit_core::choice_enum! {
    /// The code rate of layer B of the test signal.
    pub enum GenRate {
        /// 1/2.
        R12 = ("1/2", "1/2"),
        /// 2/3.
        R23 = ("2/3", "2/3"),
        /// 3/4.
        R34 = ("3/4", "3/4"),
        /// 5/6.
        R56 = ("5/6", "5/6"),
        /// 7/8.
        R78 = ("7/8", "7/8"),
    }
}

/// The mode of the test signal.
pub const GEN_MODE: Setting<field::Choice<GenMode>> =
    Setting::choice("gen_mode", "Test signal mode", GenMode::M3)
        .section("Test signal")
        .advanced();
/// The guard of the test signal.
pub const GEN_GUARD: Setting<field::Choice<Guard>> =
    Setting::choice("gen_guard", "Test signal guard", Guard::G8)
        .section("Test signal")
        .advanced();
/// The modulation of layer B of the test signal.
pub const GEN_MODULATION: Setting<field::Choice<GenModulation>> = Setting::choice(
    "gen_modulation",
    "Test signal layer B modulation",
    GenModulation::Qam64,
)
.section("Test signal")
.advanced();
/// The code rate of layer B of the test signal.
pub const GEN_CODE_RATE: Setting<field::Choice<GenRate>> = Setting::choice(
    "gen_code_rate",
    "Test signal layer B code rate",
    GenRate::R34,
)
.section("Test signal")
.advanced();

sdrkit_core::settings! {
    /// The settings of the ISDB-TSB stack.
    pub enum IsdbtsbSetting {
        /// The channel raster.
        Width = WIDTH,
        /// The segment count.
        Segments = SEGMENTS,
        /// The upper-edge pilot.
        EdgePilot = EDGE_PILOT,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
        /// The mode of the test signal.
        GenMode = GEN_MODE,
        /// The guard of the test signal.
        GenGuard = GEN_GUARD,
        /// The modulation of layer B of the test signal.
        GenModulation = GEN_MODULATION,
        /// The code rate of layer B of the test signal.
        GenCodeRate = GEN_CODE_RATE,
    }
}

/// The raster width in MHz a width setting starts with (6 for automatic).
fn raster(w: Width) -> f64 {
    match w {
        Width::W7 => 7.0,
        Width::W8 => 8.0,
        Width::W6 | Width::Auto => 6.0,
    }
}

/// The least input rate: one segment of the raster with 5 % margin (automatic widths start at 6 MHz).
fn min_rate(w: Width) -> f64 {
    1.05 * Band::of(1, Default::default()).occupied_hz(raster(w))
}

/// The ISDB-TSB receiver graph, with the transport stream layer after it.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(
    width: Width,
    segments: SegmentCount,
    edge: EdgeChoice,
    catalog: &Arc<Catalog>,
) -> Result<sdrkit_graph::Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("dc", sdrkit_graph::components::DcRemoval::default())
        .add("sync", SyncComp::narrow(width, segments, edge))
        .add("equalise", EqComp::default())
        .add("deinterleave", DeintComp::default())
        .add("fec", FecComp::default())
        .add("merge", MergeComp::default())
        .add("report", Report::narrow(width, NOTE))
        .input("dc.in")
        .connect("dc.out", "sync.in")
        .connect("sync.carriers", "equalise.in")
        .connect("equalise.out", "deinterleave.in")
        .connect("deinterleave.out", "fec.in")
        .connect("fec.out", "merge.in")
        .connect("sync.state", "report.sync")
        .connect("equalise.state", "report.eq")
        .connect("fec.state", "report.fec")
        .output("sync.tmcc")
        .status_from("report");
    layer::attach(b, "merge.ts", &TsLayer::basic())
        .catalog(catalog)
        .build()
}

/// The ISDB-TSB receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let width = cfg.settings.get(&WIDTH);
    let segments = cfg.settings.get(&SEGMENTS);
    let edge = cfg.settings.get(&EDGE_PILOT);
    StackGraph::of(
        crate::typed::shape(ID),
        graph(width, segments, edge, &Catalog::of(cfg)?)?,
    )
    .min_rate(
        min_rate(width),
        "the sample rate is too low for one ISDB-TSB segment",
    )
    .route("width", &["sync.width", "report.width"])?
    .route("segments", &["sync.segments"])?
    .route("edge_pilot", &["sync.edge_pilot"])?
    .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?
    .default_centre(DESCRIPTOR.tuning.default_hz)
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The parameters of the test signal: three segments with partial reception, layer A one segment QPSK 2/3 with interleave code
/// 2 carrying null packets, layer B two segments with interleave code 1 carrying the test programme; mode, guard, layer B's
/// modulation and code rate, raster and upper-edge pilot from the settings.
pub fn test_params(settings: &sdrkit_core::Values, rate_hz: f64, seed: u64) -> IsdbtGenParams {
    let mode = match settings.get(&GEN_MODE) {
        GenMode::M1 => 1,
        GenMode::M2 => 2,
        GenMode::M3 => 3,
    };
    let guard = GUARDS[settings.get(&GEN_GUARD).index() as usize];
    let m = match settings.get(&GEN_MODULATION) {
        GenModulation::Qpsk => 1,
        GenModulation::Qam16 => 2,
        GenModulation::Qam64 => 3,
    };
    let r = settings.get(&GEN_CODE_RATE).index() as u8;
    IsdbtGenParams {
        rate_hz,
        mode,
        guard,
        width_mhz: raster(settings.get(&WIDTH)),
        config: config(
            true,
            [layer(1, 1, 2, 1), layer(m, r, 1, 2), LayerParams::UNUSED],
        ),
        payload: Payload::Programme,
        programme_layer: 1,
        seed,
        band: Band::of(3, settings.get(&EDGE_PILOT).edge()),
        next_unset: true,
        ..Default::default()
    }
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g = IsdbtGenerator::new(test_params(&cfg.settings, cfg.rate_hz, cfg.seed))
        .map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

/// The ISDB-TSB stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "ISDB-TSB",
    group: "TV",
    blurb: "ISDB-TSB narrow-band digital sound and data broadcasting (ITU-R BS.1114 System F), 1 or 3 segments; also the one-segment service of ISDB-T",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 90e6,
        max_hz: 770e6,
        ..Tuning::anywhere(100e6, 2.048e6, 0.5e6, 1.3e6)
    },
    submodes: &[],
    settings: IsdbtsbSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[sdrkit_core::SignalType::IsdbTsb],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::ISDBTSB),
    benchmarks: Some(crate::benchmarks),
};
