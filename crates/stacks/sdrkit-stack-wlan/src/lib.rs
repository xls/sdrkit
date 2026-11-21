//! Bluetooth and Wi-Fi monitors, receive only: BLE advertising, a BLE tracker detector, Wi-Fi networks, ESP-NOW frames,
//! Wi-Fi HaLow, Bluetooth Classic discovery, and the wfb-ng and OpenHD video links on the HT-mixed PHY.
//! Standard: Bluetooth Core Specification v5.4 and its Supplement; IEEE Std 802.11-2020.

#![warn(missing_docs)]

pub mod ble;
pub mod btclassic;
pub mod espnow;
pub mod halow;
pub mod ht;
pub mod report;
pub mod testgen;
pub mod tracker;
pub mod typed;
pub mod verify;
pub mod wfb;
pub mod wifi;

use sdrkit_core::StackDescriptor;
use sdrkit_graph::Registry;

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &ble::BLE,
        &tracker::BLETRACKER,
        &wifi::WIFI,
        &halow::HALOW,
        &btclassic::BTCLASSIC,
        &wfb::WFB,
        &espnow::ESPNOW,
    ]
}

/// Adds every component of this crate to `r`, each under the name its `info()` returns.
pub fn register(r: &mut Registry) {
    ble::register(r);
    tracker::register(r);
    wifi::register(r);
    halow::register(r);
    btclassic::register(r);
    wfb::register(r);
    espnow::register(r);
}
