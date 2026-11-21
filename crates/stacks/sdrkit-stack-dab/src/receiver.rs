//! The DAB receiver: the graph of the DAB components, as the engine runs it in a `StackGraph`. DAB+ audio, DMB media and
//! the coded Layer II frames leave the graph as media packets. Layer II also decodes in the graph to stereo audio. Slides
//! leave on the picture stream.

use crate::components::audio::{Mp2DecoderComponent, Mp2FramerComponent, SuperframeComponent};
use crate::components::data::{MotComponent, PadComponent};
use crate::components::fic::{EnsembleComponent, FicComponent, FigParser};
use crate::components::front::{Centring, SampleGuard};
use crate::components::msc::MscComponent;
use crate::components::ofdm::OfdmComponent;
use crate::components::report::ReportComponent;
use crate::components::services::{
    DmbComponent, JournalineComponent, PacketComponent, SpiComponent, TiiComponent,
};
use crate::settings::DabSetting;
use sdrkit_core::{CostTable, SettingSet, StackConfig};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, Stage, StackGraph};
use sdrkit_proto_bcdata::component::PlayoutComponent;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dab";
/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 1.6e6;
/// Samples run through the graph per pass.
pub const CHUNK: usize = 65_536;

/// The component that takes stack setting `s`, `None` for a setting of the generator.
fn component_of(s: DabSetting) -> Option<&'static str> {
    use DabSetting as S;
    Some(match s {
        S::Detection | S::Tii => "ofdm",
        S::Centring => "centring",
        S::ImpulseBlanker => "guard",
        S::EarlyStart | S::DataSubchannel => "msc",
        S::AacPackaging => "superframe",
        S::Channels | S::AnnouncementTypes => "report",
        S::GenTii | S::GenFig2 | S::GenPty => return None,
    })
}

/// The recorded cost of each stage, written by the test `cost`.
pub const COSTS: &str = include_str!("../cost.txt");

/// The DAB receiver graph at its defaults, with the stack's settings as aliases.
pub fn builder() -> GraphBuilder {
    let mut b = GraphBuilder::new()
        .add("centring", Centring::default())
        .add("guard", SampleGuard::default())
        .add("ofdm", OfdmComponent::default())
        .add("fic", FicComponent::default())
        .add("figs", FigParser::default())
        .add("ensemble", EnsembleComponent::default())
        .add("msc", MscComponent::default())
        .add("superframe", SuperframeComponent::default())
        .add("mp2", Mp2FramerComponent::default())
        .add("mp2dec", Mp2DecoderComponent::default())
        .add("playout", PlayoutComponent::default())
        .add("pad", PadComponent::default())
        .add("mot", MotComponent::default())
        .add("packet", PacketComponent::default())
        .add("mot_packet", MotComponent::default())
        .add("journaline", JournalineComponent::default())
        .add("spi", SpiComponent::default())
        .add("tii", TiiComponent::default())
        .add("dmb", DmbComponent::default())
        .add("report", ReportComponent::default())
        .input("centring.in")
        .feedback("ofdm.status", "centring.sync")
        .connect("centring", "guard")
        .connect("guard.out", "ofdm")
        .connect("ofdm.fic", "fic")
        .connect("fic", "figs")
        .connect("figs", "ensemble")
        .connect("ofdm.cif", "msc.cif")
        .connect("ensemble", "msc.ens")
        .connect("msc.out", "superframe")
        .connect("msc.out", "mp2")
        .connect("msc.out", "packet.in")
        .connect("msc.out", "dmb")
        .connect("ensemble", "packet.ens")
        .connect("mp2.mp2", "mp2dec")
        .connect("mp2dec", "playout")
        .connect("superframe.pad", "pad.aac")
        .connect("mp2.pad", "pad.mp2")
        .connect("ensemble", "pad.ens")
        .connect("pad.datagroup", "mot")
        .connect("packet.out", "mot_packet")
        .connect("pad.datagroup", "journaline.xpad")
        .connect("packet.out", "journaline.packet")
        .connect("ensemble", "journaline.ens")
        .connect("mot.object", "spi.xpad")
        .connect("mot_packet.object", "spi.packet")
        .connect("ofdm.null", "tii.in")
        .connect("ensemble", "tii.ens")
        .connect("ofdm.status", "report.ofdm")
        .connect("guard.status", "report.front")
        .connect("ensemble", "report.ens")
        .connect("msc.status", "report.msc")
        .connect("superframe.status", "report.aac")
        .connect("mp2.status", "report.mp2")
        .connect("pad.label", "report.label")
        .connect("mot.object", "report.mot")
        .connect("tii", "report.tii")
        .connect("spi", "report.spi")
        .connect("journaline", "report.journaline")
        .connect("dmb.status", "report.dmb")
        .connect("packet.status", "report.packet")
        .output("playout.out")
        .output("superframe.aac")
        .output("mp2.mp2")
        .output("dmb.media")
        .output("report.picture")
        .output("report.events")
        .command_alias(
            "service.select",
            sdrkit_graph::CommandTarget::of("msc", "service.select"),
        )
        .status_from("report")
        .reconfigure_on_retune()
        // Soft bits of whole transmission frames cross into the channel decoding, and whole logical frames into the
        // audio and data decoding.
        .first_stage("ofdm")
        .stage("ofdm", "fic", Stage::new("decode").ahead_bytes(4 << 20))
        .stage("ofdm", "msc", Stage::new("decode").ahead_bytes(4 << 20))
        .stage("msc", "superframe", Stage::new("media").ahead_bytes(1 << 20))
        .stage("msc", "mp2", Stage::new("media").ahead_bytes(1 << 20))
        .stage("msc", "packet", Stage::new("media").ahead_bytes(1 << 20))
        .stage("msc", "dmb", Stage::new("media").ahead_bytes(1 << 20))
        .costs(&CostTable::parse(COSTS), "dab", "-");
    for s in DabSetting::SETTINGS {
        let key = s.key();
        let Some(c) = DabSetting::schema()
            .id(key)
            .ok()
            .and_then(DabSetting::from_id)
            .and_then(component_of)
        else {
            continue;
        };
        b = b.alias(key, &format!("{c}.{key}"));
    }
    b
}

/// The DAB graph at its defaults.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn dab_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let mut g = builder().catalog(catalog).build()?;
    g.set_max_chunk(CHUNK);
    Ok(g)
}

/// The DAB receiver: the DAB graph as the engine runs it, with the stack's settings applied.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(), dab_graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "DAB needs at least 1.6 Msps")
        .command(
            "service.select",
            sdrkit_graph::CommandTarget::of("msc", "service.select"),
        )
        .with_settings(&cfg.settings)
        .build()
}
