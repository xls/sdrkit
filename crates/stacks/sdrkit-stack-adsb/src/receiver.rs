//! The ADS-B / Mode S receiver: the graph of the 1090 MHz chain in a `StackGraph`, with the stack's settings and commands routed
//! to the components.

use crate::checks::FrameChecks;
use crate::demod::ModeSDemod;
use crate::feeds::{FeedOut, RemoteFeeds};
use crate::frontend::{HalfSampleInterpolator, PulseDcRemoval};
use crate::modeac::ModeAcDecoder;
use crate::table::Tracker;
use sdrkit_core::StackConfig;
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 2e6;
/// The report period, seconds of signal: every component reports at the same sample whatever the cuts of the input.
pub const REPORT_S: f64 = 0.25;

/// Builds the graph of the 1090 MHz receiver: interpolation, DC removal, the demodulator and the Mode A/C decoder side by side,
/// the frame checks (with the feeds in), the aircraft table, and the feeds out.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("interp", HalfSampleInterpolator::default())
        .add("dc", PulseDcRemoval::default())
        .add("demod", ModeSDemod::default())
        .add("remote", RemoteFeeds::default())
        .add("checks", FrameChecks::default())
        .add("tracker", Tracker::default())
        .add("feeds", FeedOut::default())
        .add("modeac", ModeAcDecoder::default())
        .input("interp")
        .connect("interp.out", "dc")
        .connect("dc", "demod.in")
        .connect("dc", "modeac.in")
        .connect("interp.delay", "demod.delay")
        .connect("interp.delay", "modeac.delay")
        .connect("demod", "checks.candidates")
        .connect("remote", "checks.remote")
        .connect("checks", "tracker")
        .connect("tracker.feed", "feeds")
        .output("tracker.events")
        .clock(REPORT_S)
        .status_from("tracker")
        .catalog(catalog)
        .build()
}

/// The ADS-B / Mode S receiver with these settings.
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("adsb"),
        build_graph(&Catalog::of(cfg)?)?,
    )
    .min_rate(MIN_RATE_HZ, "ADS-B needs a sample rate of at least 2 Msps")
    .route("correction", &["checks.correction"])?
    .route("repair_unknown", &["checks.repair_unknown"])?
    .route("expiry_s", &["checks.expiry_s", "tracker.expiry_s"])?
    .route("drop_s", &["checks.drop_s", "tracker.drop_s"])?
    .route("gps_layer", &["tracker.gps_layer"])?
    .route("modeac", &["modeac.modeac"])?
    .route("effort", &["demod.effort"])?
    .route("feeds_in", &["remote.feeds_in"])?
    .route("allow_remote", &["remote.allow_remote"])?
    .route("beast_server", &["feeds.beast_server"])?
    .route("beast_port", &["feeds.beast_port"])?
    .route("sbs_server", &["feeds.sbs_server"])?
    .route("sbs_port", &["feeds.sbs_port"])?
    .route("asterix_server", &["feeds.asterix_server"])?
    .route("asterix_port", &["feeds.asterix_port"])?
    .route("asterix_cat", &["feeds.asterix_cat"])?
    .route("sac", &["feeds.sac"])?
    .route("sic", &["feeds.sic"])?
    .route("allow_lan", &["feeds.allow_lan"])?
    .route("targets", &["feeds.targets"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command(
        "reset_stats",
        sdrkit_graph::CommandTarget::of("tracker", "reset_stats"),
    )
    .on_retune(sdrkit_graph::CommandTarget::of("tracker", "clear"))?
    .on_retune(sdrkit_graph::CommandTarget::of("checks", "clear"))?
    .with_settings(&cfg.settings)
    .build()
}
