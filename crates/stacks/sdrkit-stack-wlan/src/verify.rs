//! The checks the engine runs on each stack's test signal: the cases and what each must decode. The checks read the typed
//! result fields.

use crate::typed::{
    BleFields, BletrackerFields, BtclassicFields, EspnowFields, HalowFields, WfbFields, WifiFields,
};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;
use serde_json::Value as Json;

/// A check line: result field `c` is at least `min`. A field without a value counts as 0.
fn at_least(r: &Results, c: &Column<field::I64>, min: f64) -> CheckLine {
    let v = r.fields().at(c).unwrap_or(0) as f64;
    CheckLine::new(c.key, v >= min, format!("{v} (at least {min})"))
}

/// The recordings of this crate run with the stack's defaults: their sections of `expected.json` describe them, they are not
/// settings.
fn no_settings(_name: &str, _section: &Json, _all: &Json) -> Vec<CaseSetting> {
    Vec::new()
}

fn case(
    name: &str,
    o: &VerifyOptions,
    seconds: f64,
    check: fn(&Results) -> Vec<CheckLine>,
) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(name, o, 0.0);
    c.seconds = o.seconds.max(seconds);
    c.expect_audio = Some(false);
    c.check = Some(check);
    vec![c]
}

fn ble_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, eleven packets", o, 1.0, |r| {
        vec![
            at_least(r, &BleFields::FRAMES_OK, 20.0),
            at_least(r, &BleFields::DEVICES, 4.0),
        ]
    })
}

fn tracker_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, five trackers and a phone", o, 2.0, |r| {
        vec![at_least(r, &BletrackerFields::TRACKERS, 5.0)]
    })
}

fn wifi_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, 2.4 GHz scene", o, 0.5, |r| {
        vec![
            at_least(r, &WifiFields::FRAMES_OK, 30.0),
            at_least(r, &WifiFields::NETWORKS, 3.0),
        ]
    })
}

fn halow_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, S1G schedule", o, 1.0, |r| {
        vec![
            at_least(r, &HalowFields::FRAMES_OK, 15.0),
            at_least(r, &HalowFields::NETWORKS, 3.0),
        ]
    })
}

fn btclassic_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case(
        "test signal, two inquiry responses and three LAPs",
        o,
        2.0,
        |r| {
            vec![
                at_least(r, &BtclassicFields::INQUIRY_RESPONSES, 2.0),
                at_least(r, &BtclassicFields::DEVICES, 5.0),
            ]
        },
    )
}

fn wfb_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, four links", o, 0.3, |r| {
        vec![
            at_least(r, &WfbFields::LINKS_HEARD, 4.0),
            at_least(r, &WfbFields::FRAMES_OK, 39.0),
        ]
    })
}

fn espnow_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    case("test signal, mixed scene of five senders", o, 0.5, |r| {
        vec![
            at_least(r, &EspnowFields::ESPNOW_FRAMES, 20.0),
            at_least(r, &EspnowFields::DEVICES, 5.0),
            at_least(r, &EspnowFields::BROADCAST, 10.0),
            at_least(r, &EspnowFields::ENCRYPTED, 2.0),
        ]
    })
}

/// The checks of the `ble` stack.
pub static BLE: VerifyHooks = VerifyHooks {
    cases: ble_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `bletracker` stack.
pub static TRACKER: VerifyHooks = VerifyHooks {
    cases: tracker_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `wifi` stack.
pub static WIFI: VerifyHooks = VerifyHooks {
    cases: wifi_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `halow` stack.
pub static HALOW: VerifyHooks = VerifyHooks {
    cases: halow_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `btclassic` stack.
pub static BTCLASSIC: VerifyHooks = VerifyHooks {
    cases: btclassic_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `wfb` stack.
pub static WFB: VerifyHooks = VerifyHooks {
    cases: wfb_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};

/// The checks of the `espnow` stack.
pub static ESPNOW: VerifyHooks = VerifyHooks {
    cases: espnow_cases,
    vector_settings: Some(no_settings),
    check_vector: None,
};
