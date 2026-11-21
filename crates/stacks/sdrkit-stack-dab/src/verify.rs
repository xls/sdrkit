//! What this crate registers beside its descriptor ([`HOOKS`]): the verify cases on the test signal and the comparison of
//! the recorded vector of `tests/vectors/dab` with its `expected.json`. The checks read the typed results only.

use crate::results::AudioCodec;
use crate::typed::*;
use sdrkit_core::publish::TableDef;
use sdrkit_core::verify::{CheckLine, VectorRun, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{Choice, FieldDecl, LockState, Results, Value};
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// The verification of the DAB stack.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: Some(check_vector),
};

/// A value as JSON for comparing with `expected.json`: a choice becomes its key.
fn json_of(decl: &FieldDecl, v: &Value) -> Json {
    match v {
        Value::Text(t) => Json::from(t.as_str()),
        Value::Choice(i) => decl
            .choices
            .get(usize::from(*i))
            .map_or(Json::Null, |c| Json::from(c.key.as_ref())),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Json::from(*x),
        Value::I64(x) => Json::from(*x),
        Value::Bool(b) => Json::from(*b),
        _ => Json::Null,
    }
}

/// The value of result field `key` as JSON, `Null` when it has none.
fn value(r: &Results, key: &str) -> Json {
    let d = r.decl();
    d.field_id(key)
        .and_then(|id| Some((d.fields().field(id)?, r.fields().value(id)?)))
        .map_or(Json::Null, |(decl, v)| json_of(decl, v))
}

/// The rows of table `t` as (row key, cells by column key).
fn rows(r: &Results, t: &TableDef) -> Vec<(String, BTreeMap<&'static str, Json>)> {
    let Some(table) = r.decl().table_id(t.key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    table
        .rows_newest_first()
        .map(|(k, rec)| {
            let cells = t
                .columns
                .iter()
                .map(|c| {
                    let v = rec
                        .decl()
                        .id(c.key())
                        .and_then(|id| rec.value(id))
                        .map_or(Json::Null, |v| json_of(&c.decl(), v));
                    (c.key(), v)
                })
                .collect();
            (k.to_owned(), cells)
        })
        .collect()
}

fn within(key: &str, got: &Json, lo: f64, hi: f64) -> CheckLine {
    match got.as_f64() {
        Some(v) => CheckLine::new(
            key,
            (lo..=hi).contains(&v),
            format!("{v} (expected {lo} to {hi})"),
        ),
        None => CheckLine::new(key, false, format!("missing (expected {lo} to {hi})")),
    }
}

fn equal(key: &str, got: &Json, want: &Json) -> CheckLine {
    let ok = got == want
        || matches!((got.as_f64(), want.as_f64()), (Some(a), Some(b)) if (a - b).abs() < 1e-9);
    CheckLine::new(key, ok, format!("{got} (expected {want})"))
}

fn at_least(key: &str, got: &Json, lo: f64) -> CheckLine {
    within(key, got, lo, f64::INFINITY)
}

fn locked(r: &Results) -> CheckLine {
    let lock_state = r.status().lock_state;
    CheckLine::new(
        "locked",
        lock_state == LockState::Locked,
        format!("lock_state {lock_state:?}"),
    )
}

/// The first DAB+ service, which the receiver plays when no service is selected. Its access units leave as media for the
/// host's AAC decoder.
fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut plus = VerifyCase::new("test signal, DAB+ service", o, 1000.0);
    plus.seconds = o.seconds.max(5.0);
    plus.check = Some(check_plus);
    vec![plus]
}

fn ensemble(r: &Results) -> Vec<CheckLine> {
    let rows = rows(r, &SERVICES);
    let labels: Vec<&str> = rows
        .iter()
        .filter_map(|(_, c)| c[ServiceRow::LABEL.key].as_str())
        .collect();
    vec![
        locked(r),
        equal(
            "ensemble",
            &value(r, EnsembleFields::ENSEMBLE_LABEL.key),
            &Json::from("Test Ensemble"),
        ),
        CheckLine::new(
            "services",
            labels.len() == 4 && labels.contains(&"Test MP2"),
            format!("{labels:?}"),
        ),
    ]
}

fn check_plus(r: &Results) -> Vec<CheckLine> {
    let mut v = ensemble(r);
    v.push(equal(
        "codec",
        &value(r, AudioFields::AUDIO_CODEC.key),
        &Json::from("aac"),
    ));
    v.push(at_least(
        "access units",
        &value(r, AudioFields::AUDIO_UNITS_OK.key),
        1.0,
    ));
    v
}

fn check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let r = v.results;
    let e = v.section;
    let mut out = vec![locked(r)];
    // Signal.
    let s = &e["signal"];
    out.push(equal(
        "signal.sync",
        &value(r, SyncFields::SYNC.key),
        &s["sync"],
    ));
    out.push(equal(
        "signal.detection",
        &value(r, SyncFields::DETECTION.key),
        &s["detection"],
    ));
    for (k, got) in [
        ("fib_bad", &value(r, SyncFields::FIB_BAD.key)),
        (
            "fib_ok_last_frame",
            &value(r, SyncFields::FIB_OK_LAST_FRAME.key),
        ),
        ("sync_losses", &value(r, SyncFields::SYNC_LOSSES.key)),
        ("erased_cifs", &value(r, SyncFields::ERASED_CIFS.key)),
        (
            "impulses_blanked",
            &value(r, SyncFields::IMPULSES_BLANKED.key),
        ),
    ] {
        out.push(equal(&format!("signal.{k}"), got, &s[k]));
    }
    out.push(within(
        "signal.frames_decoded",
        &value(r, SyncFields::FRAMES.key),
        23.0,
        25.0,
    ));
    out.push(at_least(
        "signal.fib_ok",
        &value(r, SyncFields::FIB_OK.key),
        276.0,
    ));
    out.push(within(
        "signal.cfo_hz",
        &value(r, SignalFields::OFFSET_HZ.key),
        1175.0,
        1225.0,
    ));
    out.push(at_least(
        "signal.snr_db",
        &value(r, SignalFields::SNR_DB.key),
        18.0,
    ));
    out.push(within(
        "signal.sample_clock_ppm",
        &value(r, SignalFields::CLOCK_PPM.key),
        -5.0,
        5.0,
    ));
    // Ensemble.
    let ee = &e["ensemble"];
    let eid = r
        .fields()
        .at(&EnsembleFields::EID)
        .map_or(Json::Null, |x| Json::from(format!("{x:04X}")));
    out.push(equal("ensemble.eid", &eid, &ee["eid"]));
    out.push(equal(
        "ensemble.label",
        &value(r, EnsembleFields::ENSEMBLE_LABEL.key),
        &ee["label"],
    ));
    out.push(equal(
        "ensemble.short_label",
        &value(r, EnsembleFields::ENSEMBLE_SHORT_LABEL.key),
        &ee["short_label"],
    ));
    out.push(within(
        "ensemble.utc_seconds",
        &value(r, EnsembleFields::UTC.key),
        1_700_000_000.0,
        1_700_000_003.0,
    ));
    out.push(compare(
        "subchannels",
        ee,
        rows(r, &SUBCHANNELS),
        |w| w["id"].to_string(),
        &[
            ("id", SubchannelRow::ID.key),
            ("start_cu", SubchannelRow::START_CU.key),
            ("size_cu", SubchannelRow::SIZE_CU.key),
            ("protection", SubchannelRow::PROTECTION.key),
            ("bitrate_kbps", SubchannelRow::BITRATE_KBPS.key),
        ],
    ));
    out.push(compare(
        "services",
        ee,
        rows(r, &SERVICES),
        |w| w["sid"].as_str().unwrap_or("").to_string(),
        &[
            ("label", ServiceRow::LABEL.key),
            ("short_label", ServiceRow::SHORT_LABEL.key),
            ("ascty", ServiceRow::COMPONENT_TYPE.key),
            ("subchannel", ServiceRow::SUBCHANNEL.key),
            ("pty", ServiceRow::PTY.key),
            ("pty_static", ServiceRow::PTY_STATIC.key),
            ("language", ServiceRow::LANGUAGE.key),
        ],
    ));
    // The service played: sub-channel 1, the first service.
    let p = &e["per_selection"]["1"];
    let codec = &value(r, AudioFields::AUDIO_CODEC.key);
    let profile = AudioCodec::CHOICES
        .iter()
        .find(|c| codec.as_str() == Some(c.key.as_ref()))
        .map_or(Json::Null, |c| Json::from(c.label.as_ref()));
    out.push(equal("per_selection.1.codec", codec, &p["codec"]));
    out.push(equal("per_selection.1.profile", &profile, &p["profile"]));
    for (k, got) in [
        ("sample_rate", &value(r, AudioFields::AUDIO_RATE.key)),
        ("channels", &value(r, AudioFields::AUDIO_CHANNELS.key)),
        ("sbr", &value(r, AudioFields::AUDIO_SBR.key)),
        ("ps", &value(r, AudioFields::AUDIO_PS.key)),
    ] {
        out.push(equal(&format!("per_selection.1.{k}"), got, &p[k]));
    }
    let lf = p["logical_frames"].as_f64().unwrap_or(0.0);
    out.push(within(
        "per_selection.1.logical_frames",
        &value(r, AudioFields::AUDIO_LOGICAL_FRAMES.key),
        lf - 2.0,
        lf + 2.0,
    ));
    out.push(at_least(
        "per_selection.1.superframes_ok",
        &value(r, AudioFields::AUDIO_SUPERFRAMES_OK.key),
        15.0,
    ));
    out.push(equal(
        "per_selection.1.superframes_bad",
        &value(r, AudioFields::AUDIO_SUPERFRAMES_BAD.key),
        &Json::from(0),
    ));
    let au = p["access_units_ok"].as_f64().unwrap_or(0.0);
    out.push(within(
        "per_selection.1.access_units_ok",
        &value(r, AudioFields::AUDIO_UNITS_OK.key),
        0.85 * au,
        au + 6.0,
    ));
    out.push(equal(
        "per_selection.1.access_units_bad",
        &value(r, AudioFields::AUDIO_UNITS_BAD.key),
        &Json::from(0),
    ));
    out.push(CheckLine::not_checked(
        "per_selection",
        "the other sub-channels need a run each (the DAB crate's vector test runs them)",
    ));
    // Timing: the first report at or before the expected time, within the 0.1 s pieces the host feeds.
    for (k, got) in [
        ("t_lock", &value(r, TimingFields::TIMING_LOCK.key)),
        (
            "t_ensemble_valid",
            &value(r, TimingFields::TIMING_ENSEMBLE_VALID.key),
        ),
        (
            "t_all_labels",
            &value(r, TimingFields::TIMING_ALL_LABELS.key),
        ),
        (
            "t_first_access_unit",
            &value(r, TimingFields::TIMING_FIRST_ACCESS_UNIT.key),
        ),
    ] {
        let want = e["timing_s"][k].as_f64().unwrap_or(0.0);
        out.push(within(&format!("timing_s.{k}"), got, 0.0, want + 0.15));
    }
    out.push(CheckLine::not_checked(
        "timing_s",
        "the label and slide times belong to sub-channel 3",
    ));
    out
}

/// Compares the list `list` of the expected section `ee` with the rows of a table: one row for each entry, keyed by `key`,
/// with each pair of (expected key, column key) equal.
fn compare(
    list: &str,
    ee: &Json,
    got: Vec<(String, BTreeMap<&'static str, Json>)>,
    key: impl Fn(&Json) -> String,
    columns: &[(&str, &str)],
) -> CheckLine {
    let want = ee[list].as_array().cloned().unwrap_or_default();
    let mut bad = Vec::new();
    for (i, w) in want.iter().enumerate() {
        let k = key(w);
        let row = got.iter().find(|(rk, _)| *rk == k).map(|(_, c)| c);
        for &(wk, ck) in columns {
            let gv = row.and_then(|c| c.get(ck)).unwrap_or(&Json::Null);
            if gv != &w[wk] {
                bad.push(format!("{i}.{wk}"));
            }
        }
    }
    CheckLine::new(
        format!("ensemble.{list}"),
        got.len() == want.len() && bad.is_empty(),
        if bad.is_empty() {
            format!("{} as expected", want.len())
        } else {
            format!("differ: {}", bad.join(", "))
        },
    )
}
