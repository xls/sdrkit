//! The DVB-T receiver: the DVB-T components, then the transport stream layer of `sdrkit-proto-mpegts`, as a `StackGraph`
//! that runs in passes of a fixed length. Video and audio leave as media packets for the host to decode. The stack `dvbt`
//! adds its sender (`transmit`) on the session's samples into the transmit sink (`tx`).

use crate::equaliser::DvbtEqualiser;
use crate::fecstage::DvbtFec;
use crate::fft::DvbtFft;
use crate::finder::DvbtFinder;
use crate::input::DvbtInput;
use crate::items::Link;
use crate::params::Bandwidth;
use crate::report::DvbtReport;
use crate::results::TS_LAYER;
use crate::sync::DvbtSync;
use crate::tpsdec::DvbtTps;
use crate::transmit::{DvbtTransmit, TxSetting, COMMANDS};
use sdrkit_core::interface::StackShape;
use sdrkit_core::owner::Owner;
use sdrkit_core::{CostTable, SettingSet, StackConfig};
use sdrkit_graph::components::TxSink;
use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, GraphError, StackGraph, Stage};
use sdrkit_proto_mpegts::layer;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dvbt";
/// Input samples per graph pass.
pub const PASS: usize = 8192;
/// The recorded cost of the stages, per variant: the mode, the constellation and the code rate of the multiplex.
/// The cost of the stage `ts` is without `decode_video`: the test signal has no video, so the file has no variant
/// for it. The section `mpeg2_video` of `ts_demux` shows the load of the decoder in a session.
const COSTS: &str = include_str!("../cost.txt");
/// The variant whose cost the graph declares before it knows the multiplex: the heaviest one.
pub const COST_VARIANT: &str = "8k-qam64-7/8";
/// The most typed items one port holds in a pass.
const MAX_ITEMS: usize = 1 << 16;

/// Where each stack setting goes: the stack key and its component settings. A setting with no target is read by the host
/// (`standard`).
const ROUTES: &[(&str, &[&str])] = &[
    ("bandwidth", &["front.bandwidth"]),
    ("channels", &["front.channels"]),
    ("stream", &["fec.stream", "report.stream"]),
    ("viterbi", &["fec.viterbi"]),
    ("time_interp", &["eq.time_interp"]),
    ("interferer_erase", &["eq.interferer_erase"]),
    ("test_flat_csi", &["eq.flat_csi"]),
    ("test_full_window", &["eq.full_window"]),
    ("test_peak_only", &["sync.peak_only"]),
    ("audio_lang", &["ts_demux.audio_lang"]),
    ("audio_description", &["ts_demux.audio_description"]),
    ("downmix", &["ts_demux.downmix"]),
    layer::DECODE_VIDEO_ROUTE,
    ("subtitle_lang", &["ts_demux.subtitle_lang"]),
    ("hard_of_hearing", &["ts_demux.hard_of_hearing"]),
    ("country", &["ts_demux.country", "psi_si.country"]),
    ("guide_lang", &["ts_demux.guide_lang", "psi_si.guide_lang"]),
    ("standard", &[]),
];

/// The DVB-T graph at its defaults: the components share one control link. Whole symbols cross into the stages
/// `equalise` (carriers) and `fec` (equalised cells), whole transport packets into the stage `ts`.
pub fn builder() -> GraphBuilder {
    builder_on(Link::new())
}

/// [`builder`] with `l` as the control link of the components.
pub fn builder_on(l: Arc<Link>) -> GraphBuilder {
    let g = GraphBuilder::new()
        .add("front", DvbtInput::new(Arc::clone(&l)))
        .add("finder", DvbtFinder::new(Arc::clone(&l)))
        .add("sync", DvbtSync::new(Arc::clone(&l)))
        .add("fft", DvbtFft::with_link(Arc::clone(&l)))
        .add("tps", DvbtTps::new(Arc::clone(&l)))
        .add("eq", DvbtEqualiser::with_link(Arc::clone(&l)))
        .add("fec", DvbtFec::new(Arc::clone(&l)))
        .add("report", DvbtReport::new(l))
        .input("front")
        .connect("front.out", "finder.in")
        .connect("finder.out", "sync.in")
        .connect("sync.out", "fft.in")
        .connect("fft.out", "tps.in")
        .connect("tps.out", "eq.in")
        .connect("eq.out", "fec.in");
    layer::attach(g, "fec.out", &TS_LAYER)
        .connect("fec.out", "report.ts")
        .connect("psi_si.services", "report.services")
        .output("front.requests")
        .status_from("report")
        .reconfigure_on_retune()
        .first_stage("ofdm")
        .stage("tps", "eq", Stage::new("equalise"))
        .stage("eq", "fec", Stage::new("fec"))
        .stage("fec", "ts_demux", Stage::new("ts").ahead_bytes(1 << 20))
        .stage("fec", "ts_media", Stage::new("ts").ahead_bytes(1 << 20))
        .stage("fec", "report", Stage::new("ts").ahead_bytes(1 << 20))
        .costs(&CostTable::parse(COSTS), ID, COST_VARIANT)
}

/// The receiver of the description `shape` (that of `dvbt`, or the receiver of `dvb_terrestrial`) with the values of
/// `cfg`; the stack `dvbt` has its sender.
///
/// # Errors
///
/// [`GraphError::NamedSetting`] if `cfg` holds a value that a component refuses, [`GraphError::Shape`] if the graph differs
/// from `shape`, or another [`GraphError`] if the graph does not build.
pub fn receiver_with(shape: &StackShape, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver_on(Link::new(), shape, cfg)
}

/// [`receiver_with`] with `link` as the control link of the components.
///
/// # Errors
///
/// As [`receiver_with`].
pub fn receiver_on(
    link: Arc<Link>,
    shape: &StackShape,
    cfg: &StackConfig,
) -> Result<StackGraph, GraphError> {
    build(builder_on(link), shape, cfg, shape.id == ID)
}

/// The graph of `g` as the stack of `shape` with the values of `cfg`; with `transmit`, with the sender and its settings
/// and commands.
fn build(
    g: GraphBuilder,
    shape: &StackShape,
    cfg: &StackConfig,
    transmit: bool,
) -> Result<StackGraph, GraphError> {
    let mut g = g.catalog(&Catalog::of(cfg)?);
    if transmit {
        g = g
            .add("transmit", DvbtTransmit::default())
            .add("tx", TxSink::default())
            .input("transmit.in")
            .connect("transmit.out", "tx.in");
    }
    let mut g = g.build()?;
    g.set_max_items(MAX_ITEMS);
    let layer_commands: Vec<String> = Owner::commands(&g)
        .defs()
        .iter()
        .map(|d| d.key.to_string())
        .filter(|k| !COMMANDS.contains(&k.as_str()))
        .collect();
    let mut b = StackGraph::of(shape, g).fixed_passes(PASS).min_rate(
        Bandwidth::B5.min_rate(),
        "DVB-T needs a sample rate of at least 4.94 Msps",
    );
    for (key, targets) in ROUTES {
        b = b.route(key, targets)?;
    }
    if transmit {
        for s in TxSetting::schema().settings() {
            let key = &s.field.key;
            b = b.route(key, &[&format!("transmit.{key}")])?;
        }
        // Once the stack names a command it names all: the commands of the layer first, then those of the sender.
        for key in layer_commands {
            b = b.command(&key, CommandTarget::every(&key));
        }
        for c in COMMANDS {
            b = b.command(c, CommandTarget::of("transmit", c));
        }
    }
    b.with_settings(&cfg.settings).build()
}

/// The DVB-T stack: the receiver and the sender.
///
/// # Errors
///
/// As [`receiver_with`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver_with(crate::typed::shape(ID), cfg)
}
