//! MPT 1327 trunking (`mpt1327`): the control channel's codewords and address messages, the system identity and the channel
//! table. Receive only, with no traffic audio. Voice calls are followed through the host.
//!
//! Standard: MPT 1327.

pub mod code;
pub mod gen;
pub mod msg;
pub mod rx;

use crate::follow::{FollowComponent, FOLLOW, FOLLOW_GROUPS, FOLLOW_MAX};
use crate::{RealLevel, RealPart};
use rx::*;
use sdrkit_core::{ResultsDecl, StackConfig};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "mpt1327";

sdrkit_core::settings! {
    /// The settings of the MPT 1327 stack.
    pub enum MptSetting {
        /// How codewords are found.
        Framing = FRAMING,
        /// The control channel's own number.
        ControlChannel = CONTROL_CHANNEL,
        /// The channel spacing.
        Spacing = SPACING,
        /// Follow calls.
        Follow = FOLLOW,
        /// The talkgroups followed.
        FollowGroups = FOLLOW_GROUPS,
        /// The longest follow.
        FollowMax = FOLLOW_MAX,
    }
}

/// The graph from the discriminator audio on: FFSK, framing, messages, tables and the follower.
fn back_end(b: GraphBuilder, audio: &str, level: &str) -> GraphBuilder {
    b.add("ffsk", MptFfsk::default())
        .add("frame", MptFramer::default())
        .add("msg", MptDecoder)
        .add("tables", MptTables::default())
        .add("follow", FollowComponent::default())
        .connect(audio, "ffsk")
        .connect("ffsk", "frame")
        .connect("frame", "msg")
        .connect("frame", "tables.codewords")
        .connect("msg", "tables.messages")
        .connect(level, "tables.level")
        .connect("tables.grants", "follow")
        .output("follow.requests")
        .output("follow.events")
        .output("tables.events")
        .clock(0.2)
        .status_from("tables")
        .reconfigure_on_retune()
}

/// The receiver graph for radio samples: channel, discriminator, then [`audio_graph`]'s chain.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("channel", MptChannel::default())
        .add("disc", MptDisc::default())
        .input("channel")
        .connect("channel.out", "disc");
    back_end(b, "disc", "channel.level")
        .catalog(catalog)
        .build()
}

/// The receiver graph for audio: the real part of the input is the discriminator audio at the input rate.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn audio_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let b = GraphBuilder::new()
        .add("audio", RealPart)
        .add("level", RealLevel::default())
        .input("audio")
        .connect("audio", "level");
    back_end(b, "audio", "level").catalog(catalog).build()
}

/// A receiver of `graph` with the stack's settings from `cfg`.
fn build(
    graph: Graph,
    min_rate: f64,
    why: &str,
    cfg: &StackConfig,
) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(min_rate, why)
        .route(FRAMING.key(), &["frame.framing"])?
        .route(CONTROL_CHANNEL.key(), &["tables.control_channel"])?
        .route(SPACING.key(), &["tables.spacing_khz"])?
        .route(FOLLOW.key(), &["follow.follow"])?
        .route(FOLLOW_GROUPS.key(), &["follow.follow_groups"])?
        .route(FOLLOW_MAX.key(), &["follow.follow_max_s"])?
        .with_settings(&cfg.settings)
        .build()
}

/// The receiver for radio samples.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build(
        graph(&Catalog::of(cfg)?)?,
        48e3,
        "MPT 1327 needs at least 48 ksps",
        cfg,
    )
}

/// The receiver for discriminator audio given as samples.
///
/// # Errors
///
/// As [`receiver`].
pub fn audio_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build(
        audio_graph(&Catalog::of(cfg)?)?,
        6000.0,
        "MPT 1327 audio needs at least 6 kHz",
        cfg,
    )
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}
