//! The receivers: each stack is a graph of the family's components in a `StackGraph`, which takes the host's settings under
//! their plain keys and the host's commands under the stack's command names.

use crate::components::aprs::{AprsDemod, AprsTablesComp};
use crate::components::channels::{LoraChannels, MeshPlanner, RawPlanner, WidePlanner};
use crate::components::elrs::{
    ElrsDecode, ElrsDecodeSetting, ElrsPlanSetting, ElrsPlanner, ElrsSetting,
};
use crate::components::fanet::{
    FanetDecode, FanetDecodeSetting, FanetPlanSetting, FanetPlanner, FanetSetting,
};
use crate::components::lorawan::{FskComp, LorawanMac, LorawanMacSetting};
use crate::components::mesh::{HamClientComp, McClientComp, MeshDecode, MtClientComp, TxSchedComp};
use crate::components::modulate::LoraModulate;
use crate::components::raw::RawLog;
use crate::components::reticulum::{RnsDecode, RnsPlanSetting, RnsPlanner, RnsSetting};
use crate::settings::{
    AprsDemodSetting, AprsSetting, AprsTablesSetting, HamClientSetting, McClientSetting,
    MeshDecodeSetting, MeshPlanSetting, MtClientSetting, RawLogSetting, RawPlanSetting, RawSetting,
    SchedSetting, WidePlanSetting, WideSetting, AUDIO_INPUT, WIDE_INPUT,
};
use sdrkit_core::{Schema, SettingSet, StackConfig};
use sdrkit_graph::components::TxSink;
use sdrkit_graph::{Catalog, GraphBuilder, GraphError, StackGraph, StackGraphBuilder};
use std::collections::BTreeMap;
use std::sync::Arc;

/// The report period, seconds (four reports a second of signal).
pub const REPORT_S: f64 = 0.25;

/// The least rate of a LoRa stack: one 250 kHz channel.
pub const LORA_MIN_RATE_HZ: f64 = 250e3;

/// The least rate of `aprs` on radio samples.
pub const APRS_MIN_RATE_HZ: f64 = 240e3;

/// Routes each setting of the stack to the settings of the same key of the components: `(component, its schema)`.
fn route_all(
    mut g: StackGraphBuilder,
    stack: &Schema,
    components: &[(&str, Arc<Schema>)],
) -> Result<StackGraphBuilder, GraphError> {
    let mut targets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (comp, schema) in components {
        for s in schema.settings() {
            let key = s.field.key.as_ref();
            if stack.id(key).is_ok() {
                targets
                    .entry(key.to_owned())
                    .or_default()
                    .push(format!("{comp}.{key}"));
            }
        }
    }
    for (key, ts) in &targets {
        let ts: Vec<&str> = ts.iter().map(String::as_str).collect();
        g = g.route(key, &ts)?;
    }
    Ok(g)
}

/// The display name of a stack of the mesh receiver.
pub fn mesh_name(stack: &str) -> &'static str {
    match stack {
        "meshtastic" => "Meshtastic",
        "meshcore" => "MeshCore",
        "lorawan" => "LoRaWAN",
        "loraaprs" => "LoRa APRS",
        "meshcom" => "MeshCom",
        _ => "Mesh (LoRa)",
    }
}

/// The mesh receiver of stack `stack` (`mesh` or one of the protocol stacks). The sub-mode of the configuration chooses the
/// band plan; the settings, the results and the commands are those of `stack`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn mesh_receiver(stack: &'static str, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let sub = cfg.submode.as_deref().unwrap_or(stack);
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(MeshPlanner::new(sub)))
        .add("mesh", MeshDecode::default())
        .add("mt", MtClientComp::default())
        .add("mc", McClientComp::default())
        .add("ham", HamClientComp::default())
        .add("sched", TxSchedComp::default())
        .add("modulate", LoraModulate::default())
        .add("tx", TxSink::default())
        .input("channels")
        .input("modulate.in")
        .connect("channels.frames", "mesh.frames")
        .connect("channels.state", "mesh.state")
        .connect("channels.plan", "mesh.plan")
        .feedback("mt.own", "mesh.own_mt")
        .feedback("mc.own", "mesh.own_mc")
        .feedback("sched.own", "mesh.own_tx")
        .feedback("ham.own", "mesh.own_ham")
        .connect("mesh.mt", "mt.mt")
        .connect("channels.state", "mt.state")
        .connect("channels.plan", "mt.plan")
        .feedback("sched.notes", "mt.notes")
        .connect("mesh.mc", "mc.mc")
        .connect("channels.state", "mc.state")
        .connect("channels.plan", "mc.plan")
        .feedback("sched.notes", "mc.notes")
        .connect("channels.state", "ham.state")
        .connect("channels.plan", "ham.plan")
        .feedback("sched.notes", "ham.notes")
        .connect("mesh.ham", "ham.heard")
        .connect("mt.tx", "sched.mt")
        .connect("mc.tx", "sched.mc")
        .connect("ham.tx", "sched.ham")
        .connect("channels.state", "sched.state")
        .connect("channels.plan", "sched.plan")
        .connect("sched.out", "modulate.frames")
        .connect("modulate.out", "tx.in")
        .output("mesh.events")
        .output("mesh.texts")
        .output("sched.out")
        .output("channels.requests")
        .clock(REPORT_S)
        .status_from("mesh")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = crate::settings::mesh_schema(stack);
    let g = StackGraph::of(crate::typed::shape(stack), graph).min_rate(
        LORA_MIN_RATE_HZ,
        &format!(
            "{} needs a sample rate of at least 250 ksps",
            mesh_name(sub)
        ),
    );
    let g = route_all(
        g,
        &schema,
        &[
            ("channels", MeshPlanSetting::schema()),
            ("mesh", MeshDecodeSetting::schema()),
            ("mt", MtClientSetting::schema()),
            ("mc", McClientSetting::schema()),
            ("ham", HamClientSetting::schema()),
            ("sched", SchedSetting::schema()),
        ],
    )?;
    // The commands of the clients of the stack, as its description lists them: the ones a person uses most first.
    let (mt, mc, aprs) = (
        matches!(stack, "mesh" | "meshtastic"),
        matches!(stack, "mesh" | "meshcore"),
        matches!(stack, "mesh" | "loraaprs"),
    );
    let of = sdrkit_graph::CommandTarget::of;
    let every = sdrkit_graph::CommandTarget::every;
    let mut list: Vec<(&str, sdrkit_graph::CommandTarget)> = Vec::new();
    if mt {
        list.push(("mt.send", of("mt", "mt.send")));
    }
    if mc {
        list.push(("mc.send", of("mc", "mc.send")));
    }
    if aprs {
        list.push(("aprs.send", of("ham", "aprs.send")));
    }
    if mt {
        list.push(("mt.channel.add", every("mt.channel.add")));
    }
    if mc {
        list.push(("mc.channel.add", every("mc.channel.add")));
    }
    if mt || mc {
        list.push(("channel.remove", every("channel.remove")));
    }
    if mc {
        list.push(("mc.advert", of("mc", "mc.advert")));
        list.push(("mc.contact.add", of("mc", "mc.contact.add")));
    }
    if aprs {
        list.push(("aprs.beacon", of("ham", "aprs.beacon")));
    }
    if mt {
        list.push(("mt.request", of("mt", "mt.request")));
    }
    list.push(("clear", every("clear")));
    list.push(("remove", of("mesh", "remove")));
    list.push(("reset", every("reset")));
    if mt {
        list.push(("mt.channel", of("mt", "mt.channel")));
        list.push(("mt.identity", of("mt", "mt.identity")));
    }
    if mc {
        list.push(("mc.identity", of("mc", "mc.identity")));
    }
    list.into_iter()
        .fold(g, |g, (name, target)| g.command(name, target))
        .on_retune(sdrkit_graph::CommandTarget::of("mesh", "reset"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The `loraraw` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn raw_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(RawPlanner::default()))
        .add("raw", RawLog::default())
        .add("modulate", LoraModulate::default())
        .add("tx", TxSink::default())
        .input("channels")
        .input("modulate.in")
        .connect("channels.frames", "raw.frames")
        .connect("channels.state", "raw.state")
        .connect("channels.plan", "raw.plan")
        .connect("raw.tx", "modulate.frames")
        .connect("modulate.out", "tx.in")
        .output("raw.events")
        .output("raw.tx")
        .clock(REPORT_S)
        .status_from("raw")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = RawSetting::schema();
    let g = StackGraph::of(crate::typed::shape("loraraw"), graph).min_rate(
        LORA_MIN_RATE_HZ,
        "LoRa (raw) needs a sample rate of at least 250 ksps",
    );
    route_all(
        g,
        &schema,
        &[
            ("channels", RawPlanSetting::schema()),
            ("raw", RawLogSetting::schema()),
        ],
    )?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command("reset", sdrkit_graph::CommandTarget::every("reset"))
    .command("send", sdrkit_graph::CommandTarget::of("raw", "send"))
    .with_settings(&cfg.settings)
    .build()
}

/// The `fanet` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn fanet_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(FanetPlanner::default()))
        .add("fanet", FanetDecode::default())
        .input("channels")
        .connect("channels.frames", "fanet.frames")
        .connect("channels.state", "fanet.state")
        .connect("channels.plan", "fanet.plan")
        .output("fanet.events")
        .output("channels.requests")
        .clock(REPORT_S)
        .status_from("fanet")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = FanetSetting::schema();
    let g = StackGraph::of(crate::typed::shape("fanet"), graph)
        .min_rate(
            LORA_MIN_RATE_HZ,
            "FANET needs a sample rate of at least 250 ksps",
        )
        .min_rate_live(500e3, "FANET needs a sample rate of at least 500 ksps");
    route_all(
        g,
        &schema,
        &[
            ("channels", FanetPlanSetting::schema()),
            ("fanet", FanetDecodeSetting::schema()),
        ],
    )?
    .command("clear", sdrkit_graph::CommandTarget::of("fanet", "clear"))
    .with_settings(&cfg.settings)
    .build()
}

/// The `elrs` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn elrs_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(ElrsPlanner::default()))
        .add("elrs", ElrsDecode::default())
        .input("channels")
        .connect("channels.frames", "elrs.frames")
        .connect("channels.state", "elrs.state")
        .connect("channels.plan", "elrs.plan")
        .output("elrs.events")
        .output("channels.requests")
        .clock(REPORT_S)
        .status_from("elrs")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = ElrsSetting::schema();
    let g = StackGraph::of(crate::typed::shape("elrs"), graph).min_rate(
        crate::components::elrs::MIN_RATE_HZ,
        "ExpressLRS needs a sample rate of at least 1100 ksps",
    );
    route_all(
        g,
        &schema,
        &[
            ("channels", ElrsPlanSetting::schema()),
            ("elrs", ElrsDecodeSetting::schema()),
        ],
    )?
    .command("clear", sdrkit_graph::CommandTarget::of("elrs", "clear"))
    .with_settings(&cfg.settings)
    .build()
}

/// The `reticulum` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn reticulum_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(RnsPlanner::default()))
        .add("reticulum", RnsDecode::default())
        .input("channels")
        .connect("channels.frames", "reticulum.frames")
        .connect("channels.state", "reticulum.state")
        .connect("channels.plan", "reticulum.plan")
        .output("reticulum.events")
        .output("channels.requests")
        .clock(REPORT_S)
        .status_from("reticulum")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = RnsSetting::schema();
    let g = StackGraph::of(crate::typed::shape("reticulum"), graph)
        .min_rate(
            LORA_MIN_RATE_HZ,
            "Reticulum (RNode) needs a sample rate of at least 250 ksps",
        )
        .min_rate_live(
            1e6,
            "Reticulum (RNode) needs a sample rate of at least 1000 ksps",
        );
    route_all(g, &schema, &[("channels", RnsPlanSetting::schema())])?
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("reticulum", "clear"),
        )
        .with_settings(&cfg.settings)
        .build()
}

/// The `lorawanwide` receiver.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn wide_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("channels", LoraChannels::new(WidePlanner::default()))
        .add("fsk", FskComp::default())
        .add("mac", LorawanMac::default())
        .input("channels")
        .input("fsk.in")
        .connect("channels.plan", "fsk.plan")
        .connect("channels.frames", "mac.frames")
        .connect("fsk.frames", "mac.fsk")
        .connect("channels.state", "mac.state")
        .connect("fsk.state", "mac.fsk_state")
        .connect("channels.plan", "mac.plan")
        .output("mac.events")
        .clock(REPORT_S)
        .status_from("mac")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = WideSetting::schema();
    let g = StackGraph::of(crate::typed::shape("lorawanwide"), graph).min_rate(
        LORA_MIN_RATE_HZ,
        "LoRaWAN (all channels) needs a sample rate of at least 250 ksps",
    );
    route_all(
        g,
        &schema,
        &[
            ("channels", WidePlanSetting::schema()),
            ("mac", LorawanMacSetting::schema()),
        ],
    )?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command("reset", sdrkit_graph::CommandTarget::every("reset"))
    .with_settings(&cfg.settings)
    .build()
}

/// The `aprs` receiver. With `audio_input` or `wide_input` set, it takes any rate. The amateur client sends own messages
/// and beacons as AX.25 frames, which the scheduler sends when no carrier is heard, `lora.modulate` turns into FM at the
/// rate of the input, and `tx.sink` hands to the host.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`StackGraphBuilder::build`], among them a refused setting.
pub fn aprs_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("demod", AprsDemod::default())
        .add("tables", AprsTablesComp::default())
        .add("ham", HamClientComp::afsk())
        .add("sched", TxSchedComp::default())
        .add("modulate", LoraModulate::default())
        .add("tx", TxSink::default())
        .input("demod")
        .input("modulate.in")
        .connect("demod.frames", "tables.frames")
        .connect("demod.signal", "tables.signal")
        .feedback("ham.own", "tables.own")
        .connect("demod.signal", "ham.signal")
        .connect("tables.heard", "ham.heard")
        .feedback("sched.notes", "ham.notes")
        .connect("ham.tx", "sched.ham")
        .connect("demod.signal", "sched.signal")
        .connect("sched.out", "modulate.frames")
        .connect("modulate.out", "tx.in")
        .output("tables.events")
        .output("sched.out")
        .output("demod.listen")
        .clock(REPORT_S)
        .status_from("tables")
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    let schema = AprsSetting::schema();
    let v = &cfg.settings;
    let any_rate = v.get(&AUDIO_INPUT) || v.get(&WIDE_INPUT);
    let min = if any_rate { 0.0 } else { APRS_MIN_RATE_HZ };
    let g = StackGraph::of(crate::typed::shape("aprs"), graph)
        .min_rate(min, "APRS (AFSK) needs a sample rate of at least 240 ksps");
    let of = sdrkit_graph::CommandTarget::of;
    route_all(
        g,
        &schema,
        &[
            ("demod", AprsDemodSetting::schema()),
            ("tables", AprsTablesSetting::schema()),
            ("ham", HamClientSetting::schema()),
        ],
    )?
    // `wide_input` decides the minimum rate when the receiver is made: no component takes it.
    .route(WIDE_INPUT.key(), &[])?
    .command("aprs.send", of("ham", "aprs.send"))
    .command("aprs.beacon", of("ham", "aprs.beacon"))
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command("remove", of("tables", "remove"))
    .with_settings(&cfg.settings)
    .build()
}
