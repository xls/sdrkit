//! The stacks of the family, and what they register beside their descriptors.

pub mod aero;
pub mod argos;
pub mod bgan;
pub mod cubesat;
pub mod fleet;
pub mod globalstar;
pub mod goes;
pub mod hrdcp;
pub mod inmarsat;
pub mod inmarsatc;
pub mod iridium;
pub mod lorasat;
pub mod orbcomm;
pub mod psk;
pub mod qo100;
pub mod sarsat;
pub mod sat_survey;

use sdrkit_core::{Benchmark, StackDescriptor};
use sdrkit_graph::Registry;

/// Every stack's descriptor, in the order the stack list shows them.
pub fn all() -> Vec<&'static StackDescriptor> {
    vec![
        &inmarsatc::DESCRIPTOR,
        &aero::DESCRIPTOR,
        &inmarsat::DESCRIPTOR,
        &iridium::DESCRIPTOR,
        &orbcomm::DESCRIPTOR,
        &globalstar::DESCRIPTOR,
        &sarsat::DESCRIPTOR,
        &goes::DESCRIPTOR,
        &hrdcp::DESCRIPTOR,
        &argos::DESCRIPTOR,
        &cubesat::DESCRIPTOR,
        &fleet::DESCRIPTOR,
        &lorasat::DESCRIPTOR,
        &qo100::DESCRIPTOR,
        &psk::DESCRIPTOR,
        &bgan::DESCRIPTOR,
        &sat_survey::DESCRIPTOR,
    ]
}

/// Adds every component of the family to `r`, each under the name its `info()` returns.
pub fn register(r: &mut Registry) {
    inmarsatc::register(r);
    aero::register(r);
    inmarsat::register(r);
    iridium::register(r);
    orbcomm::register(r);
    globalstar::register(r);
    sarsat::register(r);
    goes::register(r);
    hrdcp::register(r);
    argos::register(r);
    cubesat::register(r);
    fleet::register(r);
    lorasat::register(r);
    qo100::register(r);
    psk::register(r);
    bgan::register(r);
    sat_survey::register(r);
}

/// The family's benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    let mut b = cubesat::benchmarks();
    b.extend(hrdcp::benchmarks());
    b.extend(fleet::benchmarks());
    b.extend(bgan::benchmarks());
    b.extend(sat_survey::benchmarks());
    b
}
