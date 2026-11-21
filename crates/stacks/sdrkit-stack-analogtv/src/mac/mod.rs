//! The `mac` receiver: D2-MAC/packet and D-MAC/packet as a stack graph.
//!
//! Standard: ETS 300 250.

pub mod codes;
pub mod frame;
pub mod gen;
pub mod packet;
pub mod rx;

use rx::{MacDemod, MacDisplay, MacLines, MacMux, MacPicture};
use sdrkit_core::StackConfig;
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, Registry, StackGraph};
use std::sync::Arc;

/// The lowest rate the receiver accepts.
pub const MIN_RATE_HZ: f64 = 16e6;

/// The MAC graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("demod", MacDemod::default())
        .add("lines", MacLines::default())
        .add("mux", MacMux::default())
        .add("picture", MacPicture::default())
        .add("display", MacDisplay::default())
        .input("demod.in")
        .connect("demod.out", "lines.in")
        .connect("demod.level", "lines.level")
        .connect("lines.lines", "mux.lines")
        .connect("lines.lines", "picture.lines")
        .connect("mux.report", "picture.mux")
        .connect("lines.report", "display.lines")
        .connect("mux.report", "display.mux")
        .connect("picture.report", "display.picture")
        .feedback("display.control", "demod.control")
        .output("mux.out")
        .output("display.events")
        .output("picture.out")
        .status_from("display")
        .clock(crate::atv::TICK_S)
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// The standard components and the `mac.*` ones.
pub fn registry() -> Registry {
    let mut r = sdrkit_components::registry();
    register(&mut r);
    r
}

/// Adds the `mac.*` components to `r`.
pub fn register(r: &mut Registry) {
    r.add("mac.demod", || Box::new(MacDemod::default()));
    r.add("mac.lines", || Box::new(MacLines::default()));
    r.add("mac.mux", || Box::new(MacMux::default()));
    r.add("mac.picture", || Box::new(MacPicture::default()));
    r.add("mac.display", || Box::new(MacDisplay::default()));
}

/// The MAC receiver: the graph with the stack's settings, all of them the test signal's.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape("mac"), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "MAC needs a sample rate of at least 16 Msps")
        .with_settings(&cfg.settings)
        .build()
}
