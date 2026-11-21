//! What this crate registers beside its descriptors: the verify hooks of each stack (test-signal cases, and the recorded vector
//! of `tests/vectors/adsb/` against its `expected.json`), and the component types ([`register`]).

use crate::checks::{Correction, FrameChecks};
use crate::demod::{EffortChoice, ModeSDemod};
use crate::feeds::{FeedOut, RemoteFeeds};
use crate::frontend::{HalfSampleInterpolator, PulseDcRemoval};
use crate::modeac::ModeAcDecoder;
use crate::table::Tracker;
use crate::typed::{AircraftRow, ModeAcFields, TrackerFields, UplinkFields, AIRCRAFT};
use crate::uat::demod::UatDemod;
use crate::uat::UatTracker;
use crate::uplink::UplinkDecoder;
use sdrkit_core::publish::TableDef;
use sdrkit_core::verify::{
    check_against, expected_value, json_at, json_number, matches, short, CaseSetting, CheckLine,
    Tolerance, VectorRun, VerifyCase, VerifyHooks, VerifyOptions,
};
use sdrkit_core::{FieldDecl, LockState, Record, Results, Value, ValueKind};
use serde_json::Value as Json;
use std::collections::BTreeMap;

/// Adds every component of this crate to `r`, each under the name its `info()` returns, so a description can use them.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("adsb.half_sample_interp", || {
        Box::new(HalfSampleInterpolator::default())
    });
    r.add("adsb.pulse_dc_removal", || {
        Box::new(PulseDcRemoval::default())
    });
    r.add("adsb.demod", || Box::new(ModeSDemod::default()));
    r.add("adsb.remote", || Box::new(RemoteFeeds::default()));
    r.add("adsb.checks", || Box::new(FrameChecks::default()));
    r.add("adsb.tracker", || Box::new(Tracker::default()));
    r.add("adsb.feeds", || Box::new(FeedOut::default()));
    r.add("adsb.modeac", || Box::new(ModeAcDecoder::default()));
    r.add("adsb.uplink_decoder", || Box::new(UplinkDecoder::default()));
    r.add("adsb.uat_demod", || Box::new(UatDemod::default()));
    r.add("adsb.uat_tracker", || Box::new(UatTracker::default()));
}

pub(crate) static ADSB_HOOKS: VerifyHooks = VerifyHooks {
    cases: adsb_cases,
    vector_settings: Some(adsb_vector_settings),
    check_vector: Some(adsb_check_vector),
};
pub(crate) static UAT_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| one_case(o, "test signal, UAT airspace", 0.0, check_tracks),
    vector_settings: None,
    check_vector: None,
};
pub(crate) static MODEAC_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| one_case(o, "test signal, Mode A/C replies", 0.0, check_tracks),
    vector_settings: None,
    check_vector: None,
};
pub(crate) static UPLINK_HOOKS: VerifyHooks = VerifyHooks {
    cases: |o| one_case(o, "test signal, interrogations", 0.0, check_tracks),
    vector_settings: None,
    check_vector: None,
};

/// One case at the defaults (no sound: these stacks make none).
fn one_case(
    o: &VerifyOptions,
    name: &str,
    offset_hz: f64,
    check: fn(&Results) -> Vec<CheckLine>,
) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new(name, o, offset_hz);
    c.expect_audio = Some(false);
    c.check = Some(check);
    vec![c]
}

/// A field's value, if it has one.
fn value<'a>(r: &'a Results, key: &str) -> Option<&'a Value> {
    let id = r.decl().field_id(key)?;
    r.fields().value(id).filter(|v| !v.is_none())
}

/// A value as a number (an integer, a number, a frequency or a time).
fn as_number(v: &Value) -> Option<f64> {
    match v {
        Value::I64(v) => Some(*v as f64),
        Value::F64(v) | Value::Hz(v) | Value::Duration(v) | Value::Time(v) => Some(*v),
        _ => None,
    }
}

/// A field's value as a number.
fn number(r: &Results, key: &str) -> Option<f64> {
    as_number(value(r, key)?)
}

/// A column of a row, if it has a value.
fn cell<'a>(row: &'a Record, key: &str) -> Option<&'a Value> {
    row.value(row.decl().id(key)?).filter(|v| !v.is_none())
}

/// A column of a row as a number.
fn cell_number(row: &Record, key: &str) -> Option<f64> {
    as_number(cell(row, key)?)
}

/// The rows of a declared table: row key to row.
fn rows<'a>(r: &'a Results, def: &TableDef) -> BTreeMap<&'a str, &'a Record> {
    r.decl()
        .table_id(def.key)
        .and_then(|id| r.table(id))
        .map(|t| t.rows_newest_first().collect())
        .unwrap_or_default()
}

/// The decl of a column of a declared table.
fn column(def: &TableDef, key: &str) -> Option<FieldDecl> {
    def.columns
        .iter()
        .find(|c| c.key() == key)
        .map(|c| c.decl())
}

/// The results hold something decoded: a table with rows, and a count above zero (UAT frames, Mode A/C replies, interrogations).
fn check_tracks(r: &Results) -> Vec<CheckLine> {
    let rows: usize = r
        .decl()
        .tables()
        .iter()
        .filter_map(|t| r.table(r.decl().table_id(&t.key)?))
        .map(|t| t.len())
        .sum();
    let (name, count) = [
        "frames_ok",
        ModeAcFields::MODEAC_REPLIES.key,
        UplinkFields::INTERROGATIONS.key,
        TrackerFields::AIRCRAFT_COUNT.key,
    ]
    .iter()
    .find_map(|k| Some((*k, number(r, k)?)))
    .unwrap_or(("count", 0.0));
    vec![CheckLine::new(
        "decoded",
        rows > 0 && count > 0.0,
        format!("{name} {count}, {rows} table rows"),
    )]
}

/// ADS-B: the airspace at the defaults (12 aircraft).
fn adsb_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, 12 aircraft", o, 0.0);
    c.expect_audio = Some(false);
    c.seconds = o.seconds.max(3.0);
    c.check = Some(check_airspace);
    vec![c]
}

/// The airspace test signal: most of the 12 aircraft are seen, some with a position.
fn check_airspace(r: &Results) -> Vec<CheckLine> {
    let n = number(r, TrackerFields::AIRCRAFT_COUNT.key).unwrap_or(0.0);
    let p = number(r, TrackerFields::WITH_POSITION.key).unwrap_or(0.0);
    vec![
        CheckLine::new("aircraft", n >= 8.0, format!("{n} of 12, need 8")),
        CheckLine::new(
            "positions",
            p >= 4.0,
            format!("{p} with a position, need 4"),
        ),
    ]
}

/// The receiver as the vector's notes say: its `receiver_settings` (correction, expiry), with every decoding pass on so the load of
/// the computer cannot change the result. The receiver position of the notes is the station, which the host supplies.
fn adsb_vector_settings(_name: &str, section: &Json, expected: &Json) -> Vec<CaseSetting> {
    let mut s = vec![CaseSetting::new(
        &crate::settings::EFFORT,
        EffortChoice::All,
    )];
    let rs = section
        .get("receiver_settings")
        .or_else(|| expected.get("receiver_settings"));
    if let Some(rs) = rs {
        if let Some(c) = rs["correction"].as_str() {
            let c = match c {
                "off" | "none" => Correction::Off,
                "1_bit" | "1bit" | "one_bit" => Correction::One,
                _ => Correction::Three,
            };
            s.push(CaseSetting::new(&crate::settings::CORRECTION, c));
        }
        if let Some(v) = rs.get("expiry_s").and_then(Json::as_f64) {
            s.push(CaseSetting::new(&crate::settings::EXPIRY_S, v));
        }
    }
    s
}

/// The signal time of the first report (`time_s`, else the seconds fed) at which `f` holds.
fn first_time(v: &VectorRun, f: impl Fn(&Results) -> bool) -> Option<f64> {
    v.reports
        .iter()
        .find(|(_, r)| f(r))
        .map(|(t, r)| number(r, TrackerFields::TIME_S.key).unwrap_or(*t))
}

/// The expected value of `key` in an aircraft of `expected.json`, as this crate publishes it, and the column it is in.
const AIRCRAFT_KEYS: [(&str, &str); 16] = [
    ("callsign", "callsign"),
    ("category", "category"),
    ("squawk", "squawk"),
    ("on_ground", "on_ground"),
    ("adsb_version", "adsb_version"),
    ("nacp", "nacp"),
    ("sil", "sil"),
    ("nic", "nic"),
    ("position_kind", "fix"),
    ("speed_kind", "speed_kind"),
    ("heading_is_track", "heading_is_track"),
    ("tisb", "tisb"),
    ("baro_alt_ft", "alt_ft"),
    ("speed_kt", "speed_kt"),
    ("vertical_rate_fpm", "vertical_rate_fpm"),
    ("selected_alt_ft", "selected_alt_ft"),
];

/// A number against an expected number, for the report line: the value or "null".
fn show(v: Option<f64>) -> String {
    v.map_or_else(|| "null".into(), |x| x.to_string())
}

/// Each expected aircraft against the decoded one, with the tolerances of `expected.json`. Without a station the distances are not
/// checked, and an aircraft whose notes say it has no position without the receiver position must have none.
fn check_aircraft(v: &VectorRun, r: &Results, out: &mut Vec<CheckLine>) {
    let Some(want) = v.section["aircraft"].as_array() else {
        return;
    };
    let mine = rows(r, &AIRCRAFT);
    let station = mine
        .values()
        .any(|a| cell(a, AircraftRow::DISTANCE_M.key).is_some());
    let tol = |k: &str| Tolerance::for_key(v.section, v.expected, k);
    let want_set: Vec<&str> = want.iter().filter_map(|e| e["icao"].as_str()).collect();
    let extra: Vec<&&str> = mine.keys().filter(|k| !want_set.contains(*k)).collect();
    let missing: Vec<&&str> = want_set
        .iter()
        .filter(|k| !mine.contains_key(**k))
        .collect();
    out.push(CheckLine::new(
        "aircraft",
        extra.is_empty() && missing.is_empty(),
        format!(
            "{} of {} expected addresses, {} others{}",
            want_set.len() - missing.len(),
            want_set.len(),
            extra.len(),
            if missing.is_empty() {
                String::new()
            } else {
                format!(", missing {missing:?}")
            }
        ),
    ));
    for e in want {
        let Some(icao) = e["icao"].as_str() else {
            continue;
        };
        let Some(a) = mine.get(icao) else { continue };
        let has_pos = station || e["position_without_receiver_position"] != false;
        let mut bad = Vec::new();
        let mut n = 0;
        for (k, c) in AIRCRAFT_KEYS {
            let Some(x) = e.get(k) else { continue };
            if !has_pos && k == "position_kind" {
                continue;
            }
            n += 1;
            let decl = column(&AIRCRAFT, c);
            let got = cell(a, c);
            let t = match k {
                "baro_alt_ft" | "speed_kt" | "vertical_rate_fpm" | "selected_alt_ft" => tol(k),
                _ => Some(Tolerance::Exact),
            };
            let ok = match (x.is_null(), got) {
                (true, None) => true,
                (true, Some(_)) | (false, None) => false,
                (false, Some(g)) => decl
                    .as_ref()
                    .and_then(|d| expected_value(x, d))
                    .is_some_and(|ev| matches(&ev, g, t.as_ref())),
            };
            if !ok {
                let shown = got.map_or_else(
                    || "null".into(),
                    |g| short(g, decl.as_ref().map_or(&[], |d| &d.choices)),
                );
                bad.push(format!("{k} {shown} (expected {x})"));
            }
        }
        let pos = match cell(a, AircraftRow::POSITION.key) {
            Some(Value::Position { lat, lon }) => Some([*lat, *lon]),
            _ => None,
        };
        for (i, k) in ["lat_deg", "lon_deg"].iter().enumerate() {
            let Some(x) = e.get(*k) else { continue };
            n += 1;
            let got = pos.map(|p| p[i]);
            let ok = if has_pos {
                got.zip(json_number(x)).is_some_and(|(g, xn)| {
                    matches(
                        &Value::F64(xn),
                        &Value::F64(g),
                        tol("position_deg").as_ref(),
                    )
                })
            } else {
                got.is_none()
            };
            if !ok {
                bad.push(format!("{k} {} (expected {x})", show(got)));
            }
        }
        if let Some(x) = e.get("heading_deg").filter(|x| !x.is_null()) {
            n += 1;
            let got = cell_number(a, AircraftRow::HEADING_DEG.key);
            let t = Tolerance::PlusMinus(if e["on_ground"] == true { 3.0 } else { 2.0 });
            let ok = got
                .zip(json_number(x))
                .is_some_and(|(g, xn)| matches(&Value::F64(xn), &Value::F64(g), Some(&t)));
            if !ok {
                bad.push(format!("heading_deg {} (expected {x})", show(got)));
            }
        }
        if station {
            for (k, c, scale) in [
                (
                    "distance_nm",
                    AircraftRow::DISTANCE_M.key,
                    1.0 / sdrkit_util_geo::NAUTICAL_MILE_M,
                ),
                ("bearing_deg", AircraftRow::BEARING_DEG.key, 1.0),
            ] {
                let Some(x) = e.get(k).filter(|x| !x.is_null()) else {
                    continue;
                };
                n += 1;
                let got = cell_number(a, c).map(|g| g * scale);
                let ok = got.zip(json_number(x)).is_some_and(|(g, xn)| {
                    matches(&Value::F64(xn), &Value::F64(g), tol(k).as_ref())
                });
                if !ok {
                    bad.push(format!("{k} {} (expected {x})", show(got)));
                }
            }
        }
        if let (Some(want_m), Some(got_m)) = (e["messages"].as_f64(), cell_number(a, "heard")) {
            n += 1;
            if got_m < 0.85 * want_m {
                bad.push(format!(
                    "messages {got_m} (expected at least 85 % of {want_m})"
                ));
            }
        }
        if let (Some(want_l), Some(got_l)) = (
            e["level_dbfs"].as_f64(),
            cell_number(a, AircraftRow::LEVEL_DBFS.key),
        ) {
            n += 1;
            if (got_l - want_l).abs() > 4.0 {
                bad.push(format!("level {got_l:.1} dBFS (expected {want_l} +- 4)"));
            }
        }
        out.push(CheckLine::new(
            format!("aircraft.{icao}"),
            bad.is_empty(),
            if bad.is_empty() {
                format!("{n} values as expected")
            } else {
                bad.join("; ")
            },
        ));
    }
}

/// The recorded airspace: the summary, the timing and each aircraft. The frame counts need the decoded frames themselves, which
/// the results do not carry: the crate's own test program checks them.
fn adsb_check_vector(v: &VectorRun) -> Vec<CheckLine> {
    let mut out = Vec::new();
    let r = v.results;
    let station = rows(r, &AIRCRAFT)
        .values()
        .any(|a| cell(a, AircraftRow::DISTANCE_M.key).is_some());
    let s = |k: &str| format!("summary.{k}");
    let mut put = |path: String, actual: Option<Value>, kind: ValueKind, tol: Option<Tolerance>| {
        if let Some(exp) = json_at(v.section, &path) {
            let tol = tol.or_else(|| Tolerance::for_key(v.section, v.expected, &path));
            let decl = FieldDecl::new(path.clone(), path.clone(), "", kind);
            out.push(check_against(
                &path,
                exp,
                &decl,
                actual.as_ref(),
                tol.as_ref(),
            ));
        }
    };
    macro_rules! num {
        ($path:expr, $actual:expr, $tol:expr $(,)?) => {
            put($path, ($actual).map(Value::F64), ValueKind::F64, $tol)
        };
    }
    num!(
        s("aircraft_count"),
        number(r, TrackerFields::AIRCRAFT_COUNT.key),
        Some(Tolerance::Exact),
    );
    let with_pos = if station {
        "with_position"
    } else {
        "with_position_without_receiver_position"
    };
    num!(
        s(with_pos),
        number(r, TrackerFields::WITH_POSITION.key),
        Some(Tolerance::Exact),
    );
    if station {
        num!(
            s("max_range_nm"),
            number(r, TrackerFields::MAX_RANGE_M.key).map(|m| m / sdrkit_util_geo::NAUTICAL_MILE_M),
            Some(Tolerance::PlusMinus(2.0)),
        );
    }
    num!(s("noise_dbfs"), number(r, "noise_dbfs"), None);
    num!(
        s("messages_per_second_last_report"),
        number(r, TrackerFields::MESSAGE_RATE.key),
        None,
    );
    let state = match r.status().lock_state {
        LockState::Locked => "receiving",
        LockState::Tracking | LockState::Searching => "searching",
        LockState::Off => "off",
    };
    put(
        s("state_at_end"),
        Some(Value::Text(state.into())),
        ValueKind::Text,
        None,
    );
    let t_msg = first_time(v, |r| {
        number(r, TrackerFields::AIRCRAFT_COUNT.key).unwrap_or(0.0) > 0.0
    });
    num!(
        s("first_report_with_a_message_s"),
        t_msg,
        Some(Tolerance::AtMost(
            json_at(v.section, "summary.first_report_with_a_message_s")
                .and_then(Json::as_f64)
                .unwrap_or(0.25),
        )),
    );
    let want_n = number(r, TrackerFields::AIRCRAFT_COUNT.key)
        .unwrap_or(0.0)
        .max(1.0);
    let want_p = number(r, TrackerFields::WITH_POSITION.key)
        .unwrap_or(0.0)
        .max(1.0);
    let t_pos = first_time(v, |r| {
        number(r, TrackerFields::WITH_POSITION.key) == Some(want_p)
    });
    if station {
        num!(s("all_positions_by_s"), t_pos, None);
    }
    let t_cs = first_time(v, |r| {
        let a = rows(r, &AIRCRAFT);
        a.len() as f64 >= want_n
            && a.values()
                .all(|x| matches!(cell(x, AircraftRow::CALLSIGN.key), Some(Value::Text(_))))
    });
    num!(s("all_callsigns_by_s"), t_cs, None);
    if !station {
        out.push(CheckLine::not_checked(
            s("with_position"),
            "needs the station (the taxiing aircraft has no position without it)",
        ));
    }
    out.push(CheckLine::not_checked(
        s("timestamp_error_ns"),
        "needs the decoded frames (the crate's test program checks them)",
    ));
    out.push(CheckLine::not_checked(
        "counts",
        "frame counts against transmissions.csv need the decoded frames (the crate's test program checks them)",
    ));
    check_aircraft(v, r, &mut out);
    out
}
