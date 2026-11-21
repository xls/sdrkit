//! Short-range devices: ISM sensors, Wireless M-Bus meters, Z-Wave, Zigbee / IEEE 802.15.4, Thread, Wi-SUN FAN, Sigfox, ANT+, nRF24L01,
//! the Sub-GHz chat of Flipper Zero devices, DECT base-station identities, NFC tag identities, mioty uplinks, DECT NR+
//! control fields, New Packet Radio sync words and the
//! frame structure of DSRC road toll beacons.
//!
//! Each stack receives only what a transmitter broadcasts openly. Encrypted content is shown as encrypted. The Sub-GHz chat
//! decrypts only with the key that the user types in.

#![warn(missing_docs)]
// Signal-processing loops index several arrays by one sample position. Argument lists follow the parameter lists of the standards.
#![allow(
    clippy::needless_range_loop,
    clippy::type_complexity,
    clippy::too_many_arguments
)]

pub mod ant;
pub mod dect;
pub mod dsrc;
pub mod family;
pub mod ism;
pub mod mac154;
pub mod mioty;
pub mod nfc;
pub mod npr70;
pub mod nrf24;
pub mod nrplus;
pub mod sigfox;
pub mod subghzchat;
pub mod thread;
pub mod typed;
pub mod wisun;
pub mod wmbus;
pub mod zigbee;
pub mod zwave;

use sdrkit_core::{Benchmark, StackDescriptor};

/// The recorded cost of each stage of the stacks that the test `cost` measures, written by that test.
pub const COSTS: &str = include_str!("../cost.txt");

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &ism::ISM,
        &wmbus::WMBUS,
        &zwave::ZWAVE,
        &zigbee::ZIGBEE,
        &sigfox::SIGFOX,
        &ant::ANT,
        &nrf24::NRF24,
        &subghzchat::SUBGHZCHAT,
        &dect::DECT,
        &nfc::NFC,
        &wisun::WISUN,
        &thread::THREAD,
        &mioty::MIOTY,
        &nrplus::DECT_NRPLUS,
        &npr70::NPR70,
        &dsrc::DSRC_TOLL,
    ]
}

/// The benchmarks of the crate: the Zigbee, Thread, Wi-SUN, NFC, mioty, DECT NR+, New Packet Radio and DSRC receivers, and the ANT and DECT benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    let mut v = zigbee::benchmarks();
    v.extend(thread::benchmarks());
    v.extend(wisun::benchmarks());
    v.extend(nfc::benchmarks());
    v.push(ant::benchmark());
    v.push(dect::benchmark());
    v.extend(mioty::benchmarks());
    v.extend(nrplus::benchmarks());
    v.extend(npr70::benchmarks());
    v.extend(dsrc::benchmarks());
    v.extend(ism::benchmarks());
    v
}
