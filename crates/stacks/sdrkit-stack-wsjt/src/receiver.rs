//! The receiver of a stack, or of the combined stack, as a `StackGraph` of the family's components. The stack's settings and
//! commands reach their components through routes and command aliases.

use crate::components::*;
use crate::mode::Mode;
use crate::settings::{
    early_too, hop_too, qso_too, sub_of, transmits_too, wide_too, with_sub, GEN_LIVE,
};
use sdrkit_core::StackConfig;
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph};
use std::borrow::Cow;

/// Results are reported every quarter of a second of signal.
pub const REPORT_S: f64 = 0.25;

/// The id of the receiver's stack (`wsjt` for the combined stack).
pub fn stack_id(mode: Option<Mode>) -> &'static str {
    mode.map_or("wsjt", Mode::id)
}

fn modes_of(mode: Option<Mode>) -> Vec<Mode> {
    match mode {
        Some(m) => vec![m],
        None => Mode::ALL.to_vec(),
    }
}

/// The receiver of `mode` (`None`: the combined stack). A sub-mode that the host names (`jt65b`, `q65-30c`) wins over the
/// sub-mode settings. The slot clock reads the wall clock of the run context ([`StackGraph::set_context`]).
///
/// # Errors
///
/// [`GraphError::Setting`] if a component refuses a value of `cfg`.
pub fn receiver(mode: Option<Mode>, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let modes = modes_of(mode);
    let mut b = GraphBuilder::new()
        .add("front", FrontComponent::default())
        .add("clock", ClockComponent::default())
        .input("front.in")
        .connect("front.audio", "clock.in");
    for (k, &m) in modes.iter().enumerate() {
        let sub = sub_of(m, &cfg.settings, cfg.submode.as_deref());
        let (cut, dec) = (format!("cut_{}", m.id()), crate::typed::decoder_name(m));
        let queue = crate::runner::SlotQueue::new();
        b = b
            .add(&cut, CutterComponent::new(sub).with_queue(queue.clone()))
            .add(dec, decoder(m, Some(queue)))
            .connect("front.audio", &format!("{cut}.in"))
            .connect("clock.time", &format!("{cut}.time"))
            .connect(&format!("{cut}.slots"), &format!("{dec}.in"))
            .connect(&format!("{dec}.results"), &format!("merge.in{k}"));
    }
    let mut n_merge = modes.len();
    if wide_too(mode) {
        b = b
            .add("wide", WideComponent::default())
            .input("wide.in")
            .connect("clock.time", "wide.time")
            .connect("wide.results", &format!("merge.in{n_merge}"));
        n_merge += 1;
    }
    if let Some(m) = mode.filter(|_| hop_too(mode)) {
        b = b
            .add("hop", HopComponent::new(m))
            .connect("clock.time", "hop.time")
            .output("hop.requests");
    }
    b = b
        .add("merge", MergeComponent::new(n_merge))
        .add("display", DisplayComponent::of(mode))
        .connect("merge.out", "display.slots")
        .connect("clock.time", "display.time")
        .connect("front.levels", "display.levels")
        .feedback("display.clock", "clock.clock")
        .output("front.listen")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune();
    // The transmit side: the message makers of the modes of the stack, each into the sender, the sender into the transmit
    // sink. Each message also leaves the graph as an event.
    let mut makers: Vec<&str> = Vec::new();
    if qso_too(mode) {
        b = b
            .add("qso", SequencerComponent::new(mode))
            .connect("merge.out", "qso.slots")
            .connect("clock.time", "qso.time");
        makers.push("qso");
    }
    if modes.contains(&Mode::Wspr) {
        b = b
            .add("beacon", BeaconComponent::default())
            .connect("clock.time", "beacon.time");
        makers.push("beacon");
    }
    if modes.contains(&Mode::Js8) {
        b = b
            .add("js8tx", Js8SenderComponent::default())
            .connect("clock.time", "js8tx.time");
        makers.push("js8tx");
    }
    if !makers.is_empty() {
        b = b
            .add("sender", SenderComponent::new(makers.len()))
            .add("tx", sdrkit_graph::components::TxSink::default())
            .input("sender.in")
            .connect("clock.time", "sender.time")
            .connect("sender.out", "tx.in");
        for (k, m) in makers.iter().enumerate() {
            b = b
                .connect(&format!("{m}.tx"), &format!("sender.msg{k}"))
                .output(&format!("{m}.tx"));
        }
    }
    if modes.contains(&Mode::Js8) {
        b = b.connect("front.audio", "display.waterfall");
    }
    let mut rx = StackGraph::of(
        crate::typed::shape_of(mode),
        b.catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        crate::front::MIN_RATE,
        "Weak-signal stacks need a sample rate of at least 8 ksps",
    );
    for (key, targets) in routes(&modes, mode) {
        let t: Vec<&str> = targets.iter().map(String::as_str).collect();
        rx = rx.route(&key, &t)?;
    }
    let settings = match (cfg.submode.as_deref(), modes.as_slice()) {
        (Some(id), &[m]) if m != Mode::Js8 => Cow::Owned(with_sub(
            cfg.settings.clone(),
            sub_of(m, &cfg.settings, Some(id)),
        )),
        _ => Cow::Borrowed(&cfg.settings),
    };
    let mut rx = rx
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("display", "remove"),
        );
    if qso_too(mode) {
        rx = rx.command("cq", sdrkit_graph::CommandTarget::of("qso", "cq"));
    }
    if modes.contains(&Mode::Wspr) {
        rx = rx.command(
            "beacon",
            sdrkit_graph::CommandTarget::of("beacon", "beacon"),
        );
    }
    if modes.contains(&Mode::Js8) {
        rx = rx.command("send", sdrkit_graph::CommandTarget::of("js8tx", "send"));
    }
    if transmits_too(mode) {
        rx = rx.command("halt", sdrkit_graph::CommandTarget::every("halt"));
    }
    rx.with_settings(&settings).build()
}

/// The routes of the stack's settings to its components.
fn routes(modes: &[Mode], mode: Option<Mode>) -> Vec<(String, Vec<String>)> {
    let mut r: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |k: &str, t: Vec<String>| r.push((k.to_string(), t));
    let decs = |key: &str| -> Vec<String> {
        modes
            .iter()
            .map(|m| format!("dec_{}.{key}", m.id()))
            .collect()
    };
    if mode.is_none() {
        for m in modes.iter().filter(|&&m| m != Mode::Js8) {
            add(
                &format!("enable_{}", m.id()),
                vec![format!("cut_{}.enable", m.id())],
            );
        }
    }
    for (k, m, t) in [
        ("jt65_submode", Mode::Jt65, "submode"),
        ("jt4_submode", Mode::Jt4, "submode"),
        ("q65_submode", Mode::Q65, "submode"),
        ("q65_period", Mode::Q65, "period"),
        ("fst4_period", Mode::Fst4, "period"),
        ("fst4w_period", Mode::Fst4w, "period"),
        ("msk144_period", Mode::Msk144, "period"),
        ("js8_normal", Mode::Js8, "normal"),
        ("js8_fast", Mode::Js8, "fast"),
        ("js8_turbo", Mode::Js8, "turbo"),
    ] {
        if modes.contains(&m) {
            let mut to = vec![format!("cut_{}.{t}", m.id())];
            // The sequencer sends the sub-mode that the stack listens to.
            if qso_too(mode) && matches!(m, Mode::Jt65 | Mode::Jt4) && t == "submode" {
                to.push(format!("qso.{k}"));
            }
            add(k, to);
        }
    }
    if mode.is_none_or(Mode::searches_mirrored) {
        add("mirrored", decs("mirrored"));
    }
    let early: Vec<String> = modes
        .iter()
        .filter(|m| early_too(Some(**m)))
        .map(|m| format!("cut_{}.early_decode", m.id()))
        .collect();
    if !early.is_empty() {
        add("early_decode", early);
    }
    add("deep", decs("deep"));
    for k in ["demod", "listen"] {
        add(k, vec![format!("front.{k}")]);
    }
    add("time_source", vec!["clock.source".into()]);
    add("time_start", vec!["clock.start".into()]);
    add(GEN_LIVE.key(), vec!["clock.on_clock".into()]);
    if hop_too(mode) {
        for k in ["enabled", "coordinated", "hours", "bands", "cycle"] {
            add(&format!("hop.{k}"), vec![format!("hop.{k}")]);
        }
        add("wide", vec!["wide.wide".into(), "hop.wide".into()]);
    } else if wide_too(mode) {
        add("wide", vec!["wide.wide".into()]);
    }
    let mut edges = vec!["display.band_edges".to_string()];
    if wide_too(mode) {
        edges.push("wide.band_edges".into());
        let mut dials = vec!["wide.dials".to_string()];
        if hop_too(mode) {
            edges.push("hop.band_edges".into());
            dials.push("hop.dials".into());
        }
        add("dials", dials);
    }
    add("band_edges", edges);
    if wide_too(mode) {
        add("wide_span", vec!["wide.span".into()]);
        add("multiband", vec!["wide.multiband".into()]);
        add("multiband_select", vec!["wide.select".into()]);
    }
    add("spots", vec!["display.spots".into()]);
    if qso_too(mode) {
        let keys: &[&str] = if mode.is_none() {
            &["mode", "even", "auto", "hold", "hz", "retries", "watchdog"]
        } else {
            &["even", "auto", "hold", "hz", "retries", "watchdog"]
        };
        for k in keys {
            add(&format!("qso_{k}"), vec![format!("qso.{k}")]);
        }
    }
    if modes.contains(&Mode::Wspr) {
        for k in ["dbm", "every", "offset", "hz"] {
            add(&format!("beacon_{k}"), vec![format!("beacon.{k}")]);
        }
    }
    if modes.contains(&Mode::Js8) {
        for k in ["speed", "hz"] {
            add(&format!("js8_{k}"), vec![format!("js8tx.{k}")]);
        }
    }
    r
}
