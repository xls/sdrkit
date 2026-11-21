//! Weather satellite pictures: Meteor-M LRPT, GOES HRIT, GK-2A LRIT and HRIT, Elektro-L, FY-4, FY-2 S-VISSR, HRPT and AHRPT,
//! FY-3 AHRPT and MPT, and the DVB-S2 links (GOES-R Rebroadcast, GK-2A UHRIT, HimawariCast and GEONETCAST file broadcasts)
//! through the DVB-S2 receiver of `sdrkit-proto-dvbs2`. Each stack is a graph of the components in [`components`], with a
//! generator of its test signal.

#![warn(missing_docs)]

mod bench;
pub mod components;
pub mod files;
pub mod gate;
pub mod gen;
pub mod instruments;
pub mod items;
pub mod modes;
pub mod psk;
pub mod typed;
pub mod verify;
pub mod xrit;

/// The heading of these stacks in the stack list.
pub const GROUP: &str = "Weather satellites";
/// Seconds of signal between reports (the clock of the graphs).
pub const REPORT_S: f64 = 0.25;

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static sdrkit_core::StackDescriptor> {
    vec![
        &modes::meteor::DESCRIPTOR,
        &modes::xrit::GOES,
        &modes::xrit::GK2A_LRIT,
        &modes::xrit::GK2A_HRIT,
        &modes::xrit::ELEKTRO,
        &modes::fy4::LRIT,
        &modes::fy4::HRIT,
        &modes::fy2::DESCRIPTOR,
        &modes::hrpt::DESCRIPTOR,
        &modes::fy3::AHRPT,
        &modes::fy3::MPT,
        &modes::apt::DESCRIPTOR,
        &modes::dsb::DESCRIPTOR,
        &modes::gvar::DESCRIPTOR,
        &modes::aws::DESCRIPTOR,
        &modes::gac::DESCRIPTOR,
        &modes::msg::DESCRIPTOR,
        &modes::goes_cda::DESCRIPTOR,
        &modes::goes_mdl::DESCRIPTOR,
        &modes::gk2a_cdas::DESCRIPTOR,
        &modes::sband::DESCRIPTOR,
        &modes::grb::DESCRIPTOR,
        &modes::uhrit::DESCRIPTOR,
        &modes::filecast::DESCRIPTOR,
        &modes::rdas::DESCRIPTOR,
        &modes::eodb::DESCRIPTOR,
    ]
}

/// Adds the components of these stacks to `r`.
pub fn register(r: &mut sdrkit_graph::Registry) {
    verify::register(r)
}
