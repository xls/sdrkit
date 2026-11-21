//! What the stacks register beside their descriptors: one test-signal case for each stack id, with checks of what
//! the test scene sends.

use crate::typed::{AprsFields, ElrsFields, FanetFields, MeshFields, RnsFields};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

macro_rules! hooks {
    ($name:ident, $cases:ident) => {
        pub(crate) static $name: VerifyHooks = VerifyHooks {
            cases: $cases,
            vector_settings: None,
            check_vector: None,
        };
    };
}

hooks!(MESH, mesh_cases);
hooks!(MESHTASTIC, meshtastic_cases);
hooks!(MESHCORE, meshcore_cases);
hooks!(LORAWAN, lorawan_cases);
hooks!(LORAAPRS, loraaprs_cases);
hooks!(MESHCOM, meshcom_cases);
hooks!(LORAWANWIDE, wide_cases);
hooks!(LORARAW, raw_cases);
hooks!(APRS, aprs_cases);
hooks!(FANET, fanet_cases);
hooks!(RETICULUM, reticulum_cases);
hooks!(ELRS, elrs_cases);

/// A check that the count of good frames is at least `min`.
fn frames_ok(r: &Results, min: i64) -> CheckLine {
    let f = &keys::FRAMES_OK;
    at_least_value(f.key, r.fields().get(f).unwrap_or(0), min)
}

/// A check that the count `key` of the mesh fields, which `count` reads, is at least `min`.
fn mesh_count(r: &Results, key: &str, count: fn(&MeshFields) -> i64, min: i64) -> CheckLine {
    let v = MeshFields::read(r.fields()).map_or(0, |m| count(&m));
    at_least_value(key, v, min)
}

/// A check that the count `v` of the field `key` is at least `min`.
fn at_least_value(key: &str, v: i64, min: i64) -> CheckLine {
    CheckLine::new(key, v >= min, format!("{v}, at least {min} expected"))
}

fn case(
    o: &VerifyOptions,
    name: &str,
    seconds: f64,
    check: fn(&Results) -> Vec<CheckLine>,
) -> VerifyCase {
    let mut c = VerifyCase::new(name, o, 0.0);
    c.seconds = o.seconds.max(seconds);
    c.check = Some(check);
    c.expect_audio = Some(false);
    c
}

/// One cycle of the texting scene, seconds: the end of its last frame (the MeshCore frames come after the Meshtastic ones), plus
/// 2 s for the receiver's report.
fn texting_cycle_s() -> f64 {
    let s = crate::gen::MeshScene::texting();
    crate::gen::mesh_cycle(&s, 0, 0.0, 2e6)
        .iter()
        .map(|f| f.0 + f.2.airtime(f.3.len()))
        .fold(0.0, f64::max)
        + 2.0
}

/// The texting scene: Meshtastic LongFast and MeshCore nodes, with texts.
fn mesh_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, mesh scene", texting_cycle_s(), |r| {
        vec![
            frames_ok(r, 5),
            mesh_count(r, "mt_nodes", |m| m.mt_nodes, 1),
            mesh_count(r, "mc_nodes", |m| m.mc_nodes, 1),
        ]
    })]
}

/// The Meshtastic part of the texting scene: nodes and channel texts on LongFast with the default key.
fn meshtastic_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(
        o,
        "test signal, Meshtastic LongFast",
        texting_cycle_s(),
        |r| {
            vec![
                mesh_count(r, "mt_nodes", |m| m.mt_nodes, 1),
                mesh_count(r, "messages", |m| m.messages, 1),
            ]
        },
    )]
}

/// The MeshCore part of the texting scene: adverts and Public channel texts.
fn meshcore_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, MeshCore", texting_cycle_s(), |r| {
        vec![mesh_count(r, "mc_nodes", |m| m.mc_nodes, 1)]
    })]
}

/// LoRaWAN uplinks on the channel at the dial.
fn lorawan_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, LoRaWAN", 20.0, |r| {
        vec![mesh_count(r, "devices", |m| m.devices, 1)]
    })]
}

/// LoRa APRS stations at the dial.
fn loraaprs_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, LoRa APRS", 30.0, |r| {
        vec![frames_ok(r, 1)]
    })]
}

/// MeshCom texts and positions at the dial.
fn meshcom_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, MeshCom", 30.0, |r| {
        vec![frames_ok(r, 1)]
    })]
}

/// Uplinks and downlinks on the channel at the dial.
fn wide_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, LoRaWAN wide", 8.0, |r| {
        vec![frames_ok(r, 3)]
    })]
}

/// The nine frames of the raw scene at SF7 to SF12 and 125 to 500 kHz.
fn raw_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, raw scene", 8.0, |r| {
        vec![frames_ok(r, 3)]
    })]
}

/// The APRS scene at 1200 and 9600 bit/s.
fn aprs_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(
        o,
        "test signal, APRS at 1200 and 9600 bit/s",
        12.0,
        |r| {
            vec![
                frames_ok(r, 3),
                at_least_value(
                    "stations",
                    AprsFields::read(r.fields()).map_or(0, |f| f.stations),
                    2,
                ),
            ]
        },
    )]
}

/// The FANET scene: tracking, ground tracking, service, name and message frames.
fn fanet_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, five stations", 8.0, |r| {
        vec![
            frames_ok(r, 5),
            at_least_value(
                "stations_heard",
                FanetFields::read(r.fields()).map_or(0, |f| f.stations_heard),
                3,
            ),
        ]
    })]
}

/// The Reticulum scene: announces of three destinations, data packets and a proof.
fn reticulum_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, three destinations", 8.0, |r| {
        vec![
            frames_ok(r, 4),
            at_least_value(
                "destinations_heard",
                RnsFields::read(r.fields()).map_or(0, |f| f.destinations_heard),
                2,
            ),
        ]
    })]
}

/// The ExpressLRS scene: one link with sync frames and radio-control frames on the hop channels.
fn elrs_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case(o, "test signal, one link", 4.0, |r| {
        vec![
            frames_ok(r, 50),
            at_least_value(
                "links_heard",
                ElrsFields::read(r.fields()).map_or(0, |f| f.links_heard),
                1,
            ),
        ]
    })]
}
