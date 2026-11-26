//! The settings of every driver, as a host sees them before it opens a radio, against the standard form
//! ([`sdrkit_driver_core::form`]).
//!
//! `cargo test -p sdrkit-driver-registry --test settings_form -- --nocapture print_the_settings` prints the table.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::settings::SettingDecl;
use sdrkit_core::{Clock, MemStorage, Values};
use sdrkit_driver_core::SourceDriver;
use std::sync::Arc;

fn drivers() -> Vec<Box<dyn SourceDriver>> {
    sdrkit_driver_registry::all(
        Arc::new(|_: &str, _: &Values, _| None),
        Arc::new(MemStorage::new()),
        Arc::new(FakeTransport::new()),
        Clock::new(|| 0.0),
        Arc::new(sdrkit_driver_usb::NoUsb),
    )
}

/// One line of the table for `d`.
fn line(d: &SettingDecl) -> String {
    let range = d
        .range
        .map(|r| format!("{}..{} /{}", r.min, r.max, r.step))
        .unwrap_or_default();
    let mut flags = Vec::new();
    if d.fixed {
        flags.push("fixed".to_string());
    }
    if d.restart {
        flags.push("restart".to_string());
    }
    if d.advanced {
        flags.push("advanced".to_string());
    }
    if !d.zero_means.is_empty() {
        flags.push(format!("0={}", d.zero_means));
    }
    if !d.field.choices.is_empty() {
        flags.push(format!("{} choices", d.field.choices.len()));
    }
    format!(
        "  {:<22} {:<26} {:<4} {:<7} {:<24} {:<6} {:<22} {}",
        d.field.key,
        d.field.label,
        d.field.unit,
        format!("{:?}", d.field.kind),
        range,
        if d.common.is_empty() { "" } else { "common" },
        d.section,
        flags.join(",")
    )
}

#[test]
fn print_the_settings() {
    for driver in drivers() {
        let kind = sdrkit_driver_core::DriverKind::new(driver);
        println!("{}", kind.descriptor().key);
        for d in kind.decl().schema().settings() {
            println!("{}", line(d));
        }
    }
}

/// The settings that do not follow the form yet: driver key and setting key. Each one is owned elsewhere and changes
/// there; a problem of any other setting fails the test.
const PENDING: &[(&str, &str)] = &[
    // The settings of the test signal come from the generator of the signal (sdrkit-algo-sim) without a step.
    ("synth", "gen_"),
    // The antenna and the clock source of a SoapyRemote radio are lists that the server gives only once it is open.
    ("soapyremote", "antenna"),
    ("soapyremote", "clocksrc"),
];

/// True if `problem` of driver `key` is a setting of [`PENDING`].
fn pending(key: &str, problem: &str) -> bool {
    PENDING
        .iter()
        .any(|(d, k)| *d == key && problem.starts_with(k))
}

#[test]
fn every_driver_follows_the_form() {
    let mut problems = Vec::new();
    for driver in drivers() {
        let kind = sdrkit_driver_core::DriverKind::new(driver);
        let key = kind.descriptor().key;
        for p in sdrkit_driver_core::form::check(kind.decl().schema()) {
            if !pending(key, &p) {
                problems.push(format!("{key}: {p}"));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
