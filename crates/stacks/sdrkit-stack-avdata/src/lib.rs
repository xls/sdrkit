//! Aviation data links and aeronautical voice: ACARS on VHF, VDL Mode 2, VDL Mode 4, HFDL, Airband (AM voice with channel
//! lists, scanning and a band watch) and SELCAL. Each stack is a graph of components on the shared crates, with declared
//! results, verify cases and a generator of the clean test signal.
//! Standard: ARINC 618, 620, 622, 631, 635, 714, 745, ICAO Annex 10 Volumes III and V, ICAO Doc 9741, 9776, 9816, ETSI EN 302 842.

#![warn(missing_docs)]

mod bench;

pub mod acars;
pub mod aeromacs;
pub mod airband;
pub mod d8psk;
pub mod filters;
pub mod gbas_vdb;
pub mod hfdl;
pub mod history;
pub mod ldacs;
pub mod messages;
pub mod ofdm_shape;
pub mod report;
pub mod selcal;
pub mod typed;
pub mod vdl2;
pub mod vdl3;
pub mod vdl4;
pub mod verify;

pub use bench::benchmarks;
use sdrkit_core::StackDescriptor;
use sdrkit_graph::Registry;

/// Every stack of the crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &acars::DESCRIPTOR,
        &vdl2::DESCRIPTOR,
        &vdl4::DESCRIPTOR,
        &selcal::DESCRIPTOR,
        &airband::DESCRIPTOR,
        &hfdl::DESCRIPTOR,
        &vdl3::DESCRIPTOR,
        &gbas_vdb::DESCRIPTOR,
        &ldacs::DESCRIPTOR,
        &aeromacs::DESCRIPTOR,
    ]
}

/// Adds every component of the crate to `r`, each under the name its `info()` returns.
pub fn register(r: &mut Registry) {
    acars::register(r);
    vdl2::register(r);
    vdl4::register(r);
    selcal::register(r);
    airband::register(r);
    hfdl::register(r);
    vdl3::register(r);
    gbas_vdb::register(r);
    ldacs::register(r);
    aeromacs::register(r);
}
