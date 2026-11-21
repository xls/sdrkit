//! EDACS trunking (`edacs`): the control channel frames and messages, calls and talkgroups, a power scan of the voice channels
//! and the following of one analog voice channel. ProVoice (digital) voice is not decoded and nothing encrypted is handled.
//!
//! Standard: TIA TSB-69.3.

pub mod calls;
pub mod code;
pub mod gen;
pub mod msg;
pub mod rx;

use crate::follow::{FOLLOW, FOLLOW_GROUPS};
use crate::front::NfmFront;
use calls::{FOLLOW_CARRIERS, LCN_TABLE};
use rx::{ESK, EXTENDED};
use sdrkit_core::{ResultsDecl, StackConfig};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "edacs";

sdrkit_core::settings! {
    /// The settings of the EDACS stack.
    pub enum EdacsSetting {
        /// Extended Addressing.
        Extended = EXTENDED,
        /// Remove the ESK byte.
        Esk = ESK,
        /// The voice channel table.
        LcnTable = LCN_TABLE,
        /// Follow calls.
        Follow = FOLLOW,
        /// Also follow carriers.
        FollowCarriers = FOLLOW_CARRIERS,
        /// The groups followed.
        FollowGroups = FOLLOW_GROUPS,
    }
}

/// The receiver graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add(
            "front",
            NfmFront::new(rx::RATE, rx::HALF_WIDTH_HZ).follow(rx::SEARCH_HZ, rx::MAX_CORRECTION_HZ),
        )
        .add("post", rx::EdacsPost::default())
        .add("frame", rx::EdacsFramer::default())
        .add("msg", rx::EdacsMsg::default())
        .add("scan", calls::EdacsScan::default())
        .add("calls", calls::EdacsCalls::default())
        .input("front")
        .input("scan")
        .connect("front.out", "post")
        .connect("post", "frame")
        .connect("frame", "msg")
        .connect("frame", "calls.frames")
        .connect("msg", "calls.messages")
        .connect("scan", "calls.scan")
        .connect("front.level", "calls.level")
        .output("calls.requests")
        .output("calls.events")
        .clock(0.25)
        .status_from("calls")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// The receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(307.2e3, "EDACS needs at least 307.2 ksps")
        .route(EXTENDED.key(), &["msg.extended"])?
        .route(ESK.key(), &["msg.esk"])?
        .route(LCN_TABLE.key(), &["scan.lcn_table"])?
        .route(FOLLOW.key(), &["calls.follow"])?
        .route(FOLLOW_CARRIERS.key(), &["calls.follow_carriers"])?
        .route(FOLLOW_GROUPS.key(), &["calls.follow_groups"])?
        .command("clear", sdrkit_graph::CommandTarget::of("calls", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}
