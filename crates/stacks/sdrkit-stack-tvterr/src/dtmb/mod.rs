//! DTMB: the receiver graph and the stack. The graph is `dtmb.front` (level, coarse offset, resampling to the symbol rate with a
//! clock correction), `dtmb.search` (the header grid), `dtmb.track` (header fit, PN phase, carrier offset, timing loop, the grid
//! after lost samples, the frames to erase), `dtmb.body` (channel fits, equalisers), `dtmb.si` (system information), `dtmb.fec` (de-interleaver, LDPC, BCH,
//! transport stream) and `dtmb.report`, with the transport stream layer after `dtmb.fec`. The stages are `front` (up to
//! `dtmb.si`, which the feedback edges hold together), `fec`, and `ts` (the transport stream layer and the report).
//!
//! Standard: GB 20600-2006.

pub mod body;
pub mod chain;
pub(crate) mod estimate;
pub mod fec;
pub mod front;
pub mod gen;
pub mod pn;
pub mod profile;
pub mod results;
pub mod si;
pub mod track;

use sdrkit_core::publish::field;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, CostTable, GenConfig, Generator, Receiver, Setting, SettingSet,
    SignalType, StackConfig, StackDescriptor, StageError, Tuning, Value, Values,
};
use sdrkit_graph::{values_of, Catalog, GraphBuilder, GraphError, StackGraph, Stage};
use sdrkit_proto_mpegts::layer::{self, TsLayer, DECODE_VIDEO};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dtmb";

/// Symbol rate of an 8 MHz channel.
pub const FSYM_8: f64 = 7.56e6;
/// Symbol rate of a 6 MHz channel.
pub const FSYM_6: f64 = 5.67e6;
/// Frame body symbols.
pub const BODY: usize = 3780;
/// Square-root raised-cosine roll-off.
pub const ROLLOFF: f64 = 0.05;

sdrkit_core::choice_enum! {
    /// The channel width.
    pub enum Width {
        /// 8 MHz.
        W8 = ("8", "8 MHz"),
        /// 6 MHz (Cuba).
        W6 = ("6", "6 MHz (Cuba)"),
    }
}

impl Width {
    /// The symbol rate of the width.
    pub fn symbol_rate(self) -> f64 {
        match self {
            Width::W8 => FSYM_8,
            Width::W6 => FSYM_6,
        }
    }

    /// The width in Hz.
    pub fn hz(self) -> f64 {
        match self {
            Width::W8 => 8e6,
            Width::W6 => 6e6,
        }
    }
}

/// The channel width. A stack set to the wrong width finds nothing.
pub const WIDTH: Setting<field::Choice<Width>> = Setting::choice("width", "Channel", Width::W8)
    .help("A stack set to the wrong width finds nothing.")
    .restart();

sdrkit_core::settings! {
    /// The settings of `dtmb.track`, `dtmb.fec` and `dtmb.report`.
    pub enum DtmbSetting {
        /// The channel width.
        Width = WIDTH,
    }
}

sdrkit_core::settings! {
    /// The settings of the DTMB stack.
    pub enum DtmbStackSetting {
        /// The channel width.
        Width = WIDTH,
        /// MPEG-2 video decoded to frames of raw pixels.
        DecodeVideo = DECODE_VIDEO,
    }
}

/// The values of a component whose only setting is the width.
pub(crate) fn width_values(width: Width) -> Values {
    values_of([(DtmbSetting::Width, Value::Choice(width.index()))])
}

/// The recorded cost of each stage on the test signal (`cost.txt` of the crate).
const COSTS: &str = include_str!("../../cost.txt");

/// The DTMB receiver graph for a channel width, with the transport stream layer after it. Whole data frames cross into the
/// stage `fec`, whole transport packets into the stage `ts`.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(width: Width, catalog: &Arc<Catalog>) -> Result<sdrkit_graph::Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("front", front::Front::new(width))
        .add("search", track::Search::new(width))
        .add("track", track::Track::new(width))
        .add("body", body::Body::default())
        .add("si", si::Si::default())
        .add("fec", fec::Fec::new(width))
        .add("report", results::Report::new(width))
        .input("front.in")
        .connect("front.out", "search.in")
        .connect("front.out", "track.in")
        .connect("search.found", "track.found")
        .connect("track.frames", "body.in")
        .connect("body.out", "si.in")
        .connect("si.out", "fec.in")
        .connect("front.state", "report.front")
        .connect("track.state", "report.track")
        .connect("si.state", "report.si")
        .connect("fec.state", "report.fec")
        .feedback("track.front", "front.control")
        .feedback("track.search", "search.control")
        .feedback("si.control", "body.control")
        .feedback("si.track", "track.control")
        .status_from("report");
    layer::attach(b, "fec.ts", &TsLayer::basic())
        .first_stage("front")
        .stage("si", "fec", Stage::new("fec"))
        .stage("fec", layer::DEMUX, Stage::new("ts").ahead_bytes(1 << 20))
        .stage(
            "fec",
            layer::TS_MEDIA,
            Stage::new("ts").ahead_bytes(1 << 20),
        )
        .stage("fec", "report", Stage::new("ts").ahead_bytes(1 << 20))
        .costs(&CostTable::parse(COSTS), ID, "-")
        .catalog(catalog)
        .build()
}

/// The DTMB receiver: the graph in a [`StackGraph`] that refuses a rate below the channel width.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that the stack refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let width = cfg.settings.get(&WIDTH);
    StackGraph::of(crate::typed::shape(ID), graph(width, &Catalog::of(cfg)?)?)
        .min_rate(
            width.hz(),
            "the sample rate is too low for the DTMB channel width",
        )
        .route(
            "width",
            &[
                "front.width",
                "search.width",
                "track.width",
                "fec.width",
                "report.width",
            ],
        )?
        .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?
        .default_centre(DESCRIPTOR.tuning.default_hz)
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let g =
        gen::DtmbGenerator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?;
    Ok(Box::new(g))
}

/// The DTMB stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "DTMB",
    group: "TV",
    blurb:
        "DTMB digital terrestrial TV (China, Hong Kong, Macau, Cuba, Pakistan), 8 or 6 MHz channel",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 170e6,
        max_hz: 870e6,
        not_circular: true,
        radio_filter_hz: Some(8e6),
        ..Tuning::anywhere(530e6, 10e6, 8e6, 8e6)
    },
    submodes: &[],
    settings: DtmbStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: crate::TV_LOCK_PROFILE,
    signal_types: &[SignalType::Dtmb],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::DTMB),
    benchmarks: None,
};
