//! LTR trunking (`ltr`): the sub-audible 300 bit/s words of a repeater and the repeater table. Receive only, with no voice.
//!
//! Standard: US patent 6,529,486 B1.

pub mod code;
pub mod gen;
pub mod rx;

use crate::follow::{FollowComponent, FOLLOW, FOLLOW_GROUPS, FOLLOW_MAX};
use crate::front::NfmFront;
use rx::*;
use sdrkit_core::{ResultsDecl, StackConfig};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "ltr";

sdrkit_core::settings! {
    /// The settings of the LTR stack.
    pub enum LtrSetting {
        /// Follow calls.
        Follow = FOLLOW,
        /// The talkgroups followed.
        FollowGroups = FOLLOW_GROUPS,
        /// The longest follow.
        FollowMax = FOLLOW_MAX,
    }
}

/// The receiver graph: narrow FM front end (9600 Hz, ±3.5 kHz, carrier follow-up, lag product
/// low-passed at 1200 Hz), low-pass, slicer and sync, checks, tables, follower.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add(
            "front",
            NfmFront::new(rx::WORK_RATE, rx::HALF_WIDTH_HZ)
                .follow(rx::SEARCH_HZ, rx::MAX_CORRECTION_HZ)
                .product_lowpass(rx::PRODUCT_LOWPASS_HZ),
        )
        .add("lowpass", LtrLowpass::default())
        .add("sync", LtrSync::default())
        .add("check", LtrCheck)
        .add("tables", LtrTables::default())
        .add("follow", FollowComponent::default())
        .input("front")
        .connect("front.out", "lowpass")
        .connect("lowpass", "sync")
        .connect("sync", "check")
        .connect("sync", "tables.candidates")
        .connect("check", "tables.words")
        .connect("front.level", "tables.level")
        .connect("tables.grants", "follow")
        .output("follow.requests")
        .output("follow.events")
        .output("tables.events")
        .clock(0.25)
        .status_from("tables")
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
        .min_rate(240e3, "LTR needs at least 240 ksps")
        .route(FOLLOW.key(), &["follow.follow"])?
        .route(FOLLOW_GROUPS.key(), &["follow.follow_groups"])?
        .route(FOLLOW_MAX.key(), &["follow.follow_max_s"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}
