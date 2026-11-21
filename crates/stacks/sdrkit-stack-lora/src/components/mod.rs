//! The components of the family, each a stage with typed ports, so the receivers are graphs and a graph description can use
//! them.

pub mod aprs;
pub mod channels;
pub mod elrs;
pub mod fanet;
pub mod lorawan;
pub mod mesh;
pub mod modulate;
pub mod raw;
pub mod reticulum;

use channels::{LoraChannels, MeshPlanner, RawPlanner, WidePlanner};
use sdrkit_core::{LockState, Published};

/// Adds the family's component types to a registry, and the shared `lora.bank`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    sdrkit_proto_lora_phy::components::register(r);
    r.add("mesh.channels", || {
        Box::new(LoraChannels::new(MeshPlanner::new("mesh")))
    });
    r.add("lorawan.channels", || {
        Box::new(LoraChannels::new(WidePlanner::default()))
    });
    r.add("loraraw.channels", || {
        Box::new(LoraChannels::new(RawPlanner::default()))
    });
    r.add("fanet.channels", || {
        Box::new(LoraChannels::new(fanet::FanetPlanner::default()))
    });
    r.add("fanet.decode", || Box::new(fanet::FanetDecode::default()));
    r.add("elrs.channels", || {
        Box::new(LoraChannels::new(elrs::ElrsPlanner::default()))
    });
    r.add("elrs.decode", || Box::new(elrs::ElrsDecode::default()));
    r.add("reticulum.channels", || {
        Box::new(LoraChannels::new(reticulum::RnsPlanner::default()))
    });
    r.add("reticulum.decode", || {
        Box::new(reticulum::RnsDecode::default())
    });
    r.add("mesh.decode", || Box::new(mesh::MeshDecode::default()));
    r.add("mt.client", || Box::new(mesh::MtClientComp::default()));
    r.add("mc.client", || Box::new(mesh::McClientComp::default()));
    r.add("ham.client", || Box::new(mesh::HamClientComp::default()));
    r.add("ham.client.ax25", || Box::new(mesh::HamClientComp::afsk()));
    r.add("tx.sched", || Box::new(mesh::TxSchedComp::default()));
    r.add("lora.modulate", || {
        Box::new(modulate::LoraModulate::default())
    });
    r.add("loraraw.log", || Box::new(raw::RawLog::default()));
    r.add("lorawan.fsk", || Box::new(lorawan::FskComp::default()));
    r.add("lorawan.mac", || Box::new(lorawan::LorawanMac::default()));
    r.add("aprs.demod", || Box::new(aprs::AprsDemod::default()));
    r.add("aprs.tables", || Box::new(aprs::AprsTablesComp::default()));
}

/// Writes the status of a component: its lock state, measurements and summary. Only a change moves the change counter, and an equal
/// summary is not copied.
fn show_status(
    p: &mut Published,
    lock_state: LockState,
    snr_db: Option<f64>,
    offset_hz: Option<f64>,
    level_dbfs: Option<f64>,
    summary: &str,
) {
    p.results_mut().update_status(|s| {
        let snr = snr_db.map(|x| x as f32);
        let level = level_dbfs.map(|x| x as f32);
        let changed = s.lock_state != lock_state
            || s.snr_db != snr
            || s.offset_hz != offset_hz
            || s.level_dbfs != level
            || s.summary != summary;
        if changed {
            s.lock_state = lock_state;
            s.snr_db = snr;
            s.offset_hz = offset_hz;
            s.level_dbfs = level;
            summary.clone_into(&mut s.summary);
        }
        changed
    });
}

/// A count as a cell value. A count above `i64::MAX` shows as `i64::MAX`.
fn count<T: TryInto<i64>>(n: T) -> i64 {
    n.try_into().unwrap_or(i64::MAX)
}
