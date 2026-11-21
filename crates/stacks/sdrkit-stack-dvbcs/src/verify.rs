//! What this crate registers beside its descriptors: the verify hooks of each stack (a test-signal case, and the recorded
//! vectors against their `expected.json`) and the benchmarks.

use crate::beacon::BeaconState;
use crate::typed::{DvbcFields, SatbeaconFields};
use sdrkit_core::verify::{
    check_against, CaseSetting, CheckLine, Outcome, Tolerance, VectorRun, VerifyCase, VerifyHooks,
    VerifyOptions,
};
use sdrkit_core::Results;
use sdrkit_proto_mpegts::publish::{NAME, SERVICES};
use serde_json::Value as Json;

pub(crate) static DVBC_HOOKS: VerifyHooks = VerifyHooks {
    cases: dvbc_cases,
    vector_settings: Some(defaults),
    check_vector: Some(check_vector),
};

/// The lowest SNR of the DVB-C case, dB in the occupied bandwidth. 64-QAM with RS(204,188) alone needs about 23 dB for
/// words free of errors; this leaves a margin of 3 dB.
const DVBC_MIN_SNR_DB: f64 = 26.0;

/// The DVB-C case: the TV case at no less than [`DVBC_MIN_SNR_DB`].
fn dvbc_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut cases = tv_case(o, "test signal, 64-QAM test multiplex", 3.0);
    for c in &mut cases {
        c.snr_db = c.snr_db.map(|s| s.max(DVBC_MIN_SNR_DB));
    }
    cases
}

pub(crate) static DVBS_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| tv_case(o, "test signal, DVB-S2 QPSK 2/3 test multiplex", 3.0),
    vector_settings: Some(defaults),
    check_vector: Some(check_vector),
};

pub(crate) static ISDBS_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| tv_case(o, "test signal, three streams", 1.0),
    vector_settings: Some(defaults),
    check_vector: Some(check_vector),
};

pub(crate) static BEACON_HOOKS: VerifyHooks = VerifyHooks {
    cases: beacon_cases,
    vector_settings: Some(defaults),
    check_vector: Some(check_vector),
};

/// A TV case: the test multiplex must lock and name its service.
fn tv_case(o: &VerifyOptions, name: &str, seconds: f64) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(name, o, 20e3);
    c.seconds = o.seconds.max(seconds);
    c.expect_audio = Some(false);
    c.check = Some(check_service);
    vec![c]
}

fn check_service(r: &Results) -> Vec<CheckLine> {
    let lock = r.fields().at(&DvbcFields::TS_LOCK);
    let mut names: Vec<&str> = Vec::new();
    if let Some(table) = r.decl().table_id(SERVICES.key).and_then(|id| r.table(id)) {
        for (_, row) in table.rows_newest_first() {
            if let Some(name) = row.get(&NAME) {
                names.push(name);
            }
        }
    }
    let named = names.join(", ");
    vec![
        CheckLine::new("ts_lock", lock == Some(true), format!("{lock:?}")),
        CheckLine::new("services", named.contains("sdrkit test"), named),
    ]
}

/// The beacon case: frames must be found.
fn beacon_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, bi-phase telemetry", o, 60e3);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let s = r.fields().at(&SatbeaconFields::STATE);
        vec![CheckLine::new(
            "state",
            s == Some(BeaconState::FramesFound),
            format!("{s:?}"),
        )]
    });
    vec![c]
}

/// The recordings decode at the stack's default settings.
fn defaults(_: &str, _: &Json, _: &Json) -> Vec<CaseSetting> {
    Vec::new()
}

/// The keys of `expect` that are counts of the numbered packets, and the monitor field that holds each.
const MONITOR_KEYS: [(&str, &str); 3] = [
    ("packets_delivered", "numbered_good"),
    ("bad_packets", "numbered_wrong"),
    ("counter_jumps", "numbered_gaps"),
];

/// Compares each key of the section's `expect` with the field of the same name (or the monitor count it names). Keys that no
/// field holds (the counters of the first and last packet, the frames of the beacon) are left to the stack's tests. The check
/// of `expect` covers every key under it.
fn check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    let Some(expect) = v.section.get("expect").and_then(Json::as_object) else {
        return vec![CheckLine::new("expect", false, "no expect section")];
    };
    for (k, exp) in expect {
        let field = MONITOR_KEYS
            .iter()
            .find(|m| m.0 == k)
            .map_or(k.as_str(), |m| m.1);
        let Some(id) = v.results.decl().field_id(field) else {
            continue;
        };
        let (Some(actual), Some(decl)) = (
            v.results.fields().value(id).filter(|x| !x.is_none()),
            v.results.decl().fields().field(id),
        ) else {
            continue;
        };
        if exp.is_array() || exp.is_object() {
            continue;
        }
        let tol = Tolerance::for_key(v.section, v.expected, k);
        out.push(check_against(
            &format!("expect.{k}"),
            exp,
            decl,
            Some(actual),
            tol.as_ref(),
        ));
    }
    let ok = out.iter().all(|c| c.outcome != Outcome::Fail);
    out.push(CheckLine::new("expect", ok, "the keys that fields hold"));
    out
}
