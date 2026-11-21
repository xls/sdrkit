//! Motorola Type II trunking (`motorola_type2`): the 3600 bit/s outbound control channel: talkgroups, units, sites, patches and
//! voice grants. Grants go to the follower, which asks the host to follow a call (analogue FM, or P25 Phase 1 for a digital
//! call). The channel plan is a plan file that the host names; the stack holds none. Receive only.
//!
//! No public document describes this control channel; the `note` field says what is not confirmed.

pub mod code;
pub mod gen;
pub mod msg;
pub mod plan;
pub mod rx;
pub mod tables;

use crate::follow::{FOLLOW, FOLLOW_GROUPS, FOLLOW_MAX};
use crate::front::NfmFront;
use sdrkit_core::{ConfigError, GenConfig, Generator, Receiver, ResultsDecl, Schema, StackConfig};
use sdrkit_graph::{Catalog, DecoderComponent, GraphBuilder, GraphError, StackGraph};
use std::sync::{Arc, OnceLock};
use tables::{FOLLOW_PRIVATE, PLAN};

/// The stack id.
pub const ID: &str = "motorola_type2";
/// The least input rate, Hz.
pub const MIN_RATE: f64 = 48e3;

/// The settings of the stack and of its test signal.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| {
            let mut all: Vec<&dyn sdrkit_core::DeclareSetting> =
                vec![&PLAN, &FOLLOW, &FOLLOW_PRIVATE, &FOLLOW_GROUPS, &FOLLOW_MAX];
            all.extend_from_slice(gen::SETTINGS);
            Arc::new(Schema::of(&all))
        })
        .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The receiver: front end, word reader, tables and follower.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add(
            "front",
            NfmFront::new(rx::WORK_RATE, rx::HALF_WIDTH_HZ)
                .follow(rx::SEARCH_HZ, rx::MAX_CORRECTION_HZ),
        )
        .add("osw", rx::OswReader::default())
        .add(
            "tables",
            DecoderComponent::new(tables::Type2Tables::default()),
        )
        .add("follow", tables::Type2Follow::default())
        .input("front")
        .connect("front.out", "osw.in")
        .connect("front.level", "osw.level")
        .connect("osw", "tables")
        .connect("tables.grants", "follow.in")
        .connect("tables.ends", "follow.ends")
        .output("tables.grant_events")
        .output("tables.messages")
        .output("follow.requests")
        .output("follow.events")
        .clock(0.2)
        .status_from("tables")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(MIN_RATE, "Motorola Type II needs at least 48 ksps")
        .route(PLAN.key(), &["tables.plan"])?
        .route(FOLLOW_PRIVATE.key(), &["tables.follow_private"])?
        .route(FOLLOW.key(), &["follow.follow"])?
        .route(FOLLOW_GROUPS.key(), &["follow.follow_groups"])?
        .route(FOLLOW_MAX.key(), &["follow.follow_max_s"])?
        .command("clear", sdrkit_graph::CommandTarget::of("tables", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`receiver`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// Makes the looping test signal, the control channel where the receiver looks.
///
/// # Errors
///
/// [`ConfigError`] if the rate cannot hold the scene.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::Type2Generator::from_config(
        cfg,
        crate::MOTOROLA_TYPE2.tuning.tune_offset_hz,
    )?))
}
