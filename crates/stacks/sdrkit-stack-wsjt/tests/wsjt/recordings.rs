//! The recorded vectors through the whole receiver (front end, slot clock, cutters, decoders, decode table), fed in pieces of 8000
//! samples with the slot clock set from the recording's start time, against `expected.json`.

//!
//! FST4, FST4W and MSK144 decode nothing without their code tables, which sdrkit cannot ship: their recordings must give no
//! decode and the refusal in the status. When the recordings folder holds the tables (`wsjt/codes`, the alist files), they
//! must also give every listed decode with them. Q65 has its code built in and gives every decode of its list.

use super::common::{choice_key, table};
use sdrkit_core::owner::Owner;
use sdrkit_core::publish::keys;
use sdrkit_core::{
    Choice, Event, InputInfo, LockState, Output, Receiver, StackConfig, Value, Values,
};
use sdrkit_graph::StackGraph;
use sdrkit_stack_wsjt::descriptors;
use sdrkit_stack_wsjt::proto::codes::{CodeId, CodeStore};
use sdrkit_stack_wsjt::slots::TimeChoice;
use sdrkit_stack_wsjt::typed::{
    DecodeEvent, Js8FrameEvent, Js8MessageEvent, DECODE, JS8_FRAME, JS8_MESSAGE,
};
use serde_json::Value as Json;

fn expected() -> Json {
    sdrkit_dev_test::sigmf::read_expected("wsjt").unwrap()
}

/// The code tables of the recordings folder, when it holds them.
fn folder_codes() -> Option<CodeStore> {
    let dir = sdrkit_dev_test::recordings_dir().join("wsjt").join("codes");
    let mut c = CodeStore::default();
    let loaded = c.load_dir(&dir).ok()?;
    (!loaded.is_empty()).then_some(c)
}

/// Runs a recording through the receiver of its mode; returns the decode events, and the receiver.
/// `None` when the recording is missing.
fn run(key: &str, piece: usize) -> Option<(Vec<Event>, StackGraph)> {
    run_with(key, piece, CodeStore::default())
}

fn run_with(key: &str, piece: usize, codes: CodeStore) -> Option<(Vec<Event>, StackGraph)> {
    let e = expected();
    let r = &e["recordings"][key];
    let name = r["recording"]
        .as_str()
        .unwrap()
        .trim_end_matches(".sigmf-meta");
    let rec = sdrkit_dev_test::read_recording(&format!("wsjt/{name}"))?;
    let rc = &r["receiver"];
    let mode = rc["mode"].as_str().unwrap();
    let d = descriptors().into_iter().find(|d| d.id == mode).unwrap();
    let schema = (d.settings)();
    let mut settings = Values::defaults(&schema);
    let mut put = |k: &str, v: Value| {
        if let Ok(id) = schema.id(k) {
            settings.set(id, &v).unwrap();
        }
    };
    put("time_source", Value::Choice(TimeChoice::Start.index()));
    put(
        "time_start",
        Value::Text(rc["start_utc"].as_str().unwrap().into()),
    );
    // A sub-mode or period the mode does not let the user choose is its only one.
    if let Some(s) = rc["submode"].as_str() {
        let k = format!("{mode}_submode");
        if let Ok(id) = schema.id(&k) {
            let i = schema.setting(id).unwrap().field.choice_index(s).unwrap();
            put(&k, Value::Choice(i));
        }
    }
    if let Some(p) = rc["period_s"].as_f64() {
        put(&format!("{mode}_period"), Value::Duration(p));
    }
    let cfg = StackConfig::new(settings);
    let mode_enum = sdrkit_stack_wsjt::Mode::from_id(mode);
    let mut rx = sdrkit_stack_wsjt::receiver::receiver_with_codes(mode_enum, &cfg, codes).unwrap();
    let dial = rc["dial_hz"].as_f64().unwrap();
    rx.configure(&InputInfo::new(rec.rate_hz, dial, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let mut events = Vec::new();
    for chunk in rec.samples.chunks(piece) {
        rx.feed(chunk, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    rx.finish(&mut out);
    events.append(&mut out.events);
    Some((events, rx))
}

fn check(key: &str) {
    let Some((events, rx)) = run(key, 8000) else {
        return;
    };
    compare(key, "decodes", &events, &rx);
}

/// The decodes of the recording against the list `list` of its section.
fn compare(key: &str, list: &str, events: &[Event], rx: &StackGraph) {
    let e = expected();
    let r = &e["recordings"][key];
    let start = 1_767_225_600.0;
    let decodes: Vec<&Event> = events.iter().filter(|e| e.is(&DECODE)).collect();
    let text = |d: &Event| d.get(&keys::TEXT).unwrap_or_default().to_string();
    let want = r[list].as_array().unwrap();
    let tol = &r["tolerance"];
    let mut bad = Vec::new();
    for x in want {
        let msg = x["msg"].as_str().unwrap();
        let slot = start + x["slot_offset_s"].as_f64().unwrap();
        match decodes
            .iter()
            .find(|d| text(d) == msg && (d.time - slot).abs() < 0.01)
        {
            Some(d) => {
                let hz = d.at(&DecodeEvent::AUDIO_HZ).unwrap();
                let dt = d.at(&DecodeEvent::DT_S).unwrap();
                let snr = d.get(&keys::SNR_DB).unwrap();
                if (hz - x["hz"].as_f64().unwrap()).abs() > tol["hz"].as_f64().unwrap()
                    || (dt - x["dt_s"].as_f64().unwrap()).abs() > tol["dt_s"].as_f64().unwrap()
                    || (snr - x["snr_db"].as_f64().unwrap()).abs() > tol["snr_db"].as_f64().unwrap()
                {
                    bad.push(format!(
                        "{msg}: {hz:.2} Hz {dt:.3} s {snr} dB, expected {x}"
                    ));
                }
            }
            None => bad.push(format!("missing {msg} at {}", x["slot_offset_s"])),
        }
    }
    for d in &decodes {
        if !want.iter().any(|x| {
            x["msg"].as_str() == Some(text(d).as_str())
                && (d.time - start - x["slot_offset_s"].as_f64().unwrap()).abs() < 0.01
        }) {
            bad.push(format!("extra {} at {}", text(d), d.time - start));
        }
    }
    assert!(bad.is_empty(), "{key}: {bad:#?}");
    // The decode table holds them.
    assert_eq!(table(rx.results(), "decodes").len(), want.len());
    assert_eq!(choice_key(rx.results(), "time_source"), "recording_start");
}

/// A recording of a mode whose code is not loaded: no decode, and the status says why.
fn check_refused(key: &str, words: &str) {
    let Some((events, rx)) = run(key, 8000) else {
        return;
    };
    let n = events.iter().filter(|e| e.is(&DECODE)).count();
    assert_eq!(n, 0, "{key}: decodes without a code table");
    let st = rx.results().status();
    assert_eq!(st.lock_state, LockState::Off, "{key}");
    assert!(st.summary.contains(words), "{key}: {}", st.summary);
    let want = &expected()["recordings"][key];
    assert_eq!(want["decodes"].as_array().map_or(0, Vec::len), 0, "{key}");
    assert!(want["decodes_with_code_table"].is_array(), "{key}");
    // With the tables of the recordings folder, every listed decode.
    if let Some(codes) = folder_codes() {
        let id = match key {
            k if k.starts_with("fst4w") => CodeId::Fst4w,
            k if k.starts_with("fst4") => CodeId::Fst4,
            _ => CodeId::Msk144,
        };
        if !codes.has(id) {
            return;
        }
        let Some((events, rx)) = run_with(key, 8000, codes) else {
            return;
        };
        compare(key, "decodes_with_code_table", &events, &rx);
    }
}

#[test]
fn ft8_through_the_receiver() {
    check("ft8");
}

#[test]
fn ft4_through_the_receiver() {
    check("ft4");
}

#[test]
fn ft2_through_the_receiver() {
    check("ft2");
}

#[test]
fn wspr_through_the_receiver() {
    check("wspr");
}

#[test]
fn jt65_through_the_receiver() {
    check("jt65a");
}

#[test]
fn jt9_through_the_receiver() {
    check("jt9");
}

#[test]
fn jt4_through_the_receiver() {
    check("jt4a");
}

#[test]
fn fst4_through_the_receiver() {
    check_refused("fst4_60", "fst4_240_101.alist");
}

#[test]
fn fst4w_through_the_receiver() {
    check_refused("fst4w_120", "fst4w_240_74.alist");
}

#[test]
fn msk144_through_the_receiver() {
    check_refused("msk144_15", "msk144_128_90.alist");
}

#[test]
fn q65_through_the_receiver() {
    check("q65_60a");
}

#[test]
fn js8_through_the_receiver() {
    let e = expected();
    let r = &e["recordings"]["js8"];
    let Some((events, rx)) = run("js8", 8000) else {
        return;
    };
    let frames: Vec<&Event> = events.iter().filter(|e| e.is(&JS8_FRAME)).collect();
    let want = r["frames"].as_array().unwrap();
    for x in want {
        let slot = 1_767_225_600.0 + x["slot_start_s"].as_f64().unwrap();
        assert!(
            frames
                .iter()
                .any(|f| f.at(&Js8FrameEvent::CHARS) == x["chars"].as_str()
                    && (f.time - slot).abs() < 0.01),
            "missing {}",
            x["text"]
        );
    }
    assert_eq!(frames.len(), want.len());
    // The four-frame message put together, complete.
    let msg = &r["messages"][0];
    let got = events
        .iter()
        .filter(|e| e.is(&JS8_MESSAGE))
        .find(|m| m.get(&keys::TEXT) == msg["text"].as_str());
    assert!(
        got.is_some_and(|m| m.at(&Js8MessageEvent::COMPLETE) == Some(true)),
        "message {}",
        msg["text"]
    );
    // Stations on the map.
    let res = rx.results();
    assert!(res.points().contains_key("LY2ABC"));
    assert!(table(res, "js8_stations").row("ES1ABC").is_some());
}

#[test]
fn results_are_the_same_for_any_cut() {
    let Some((a, _)) = run("ft4", 8000) else {
        return;
    };
    let Some((b, _)) = run("ft4", 777) else {
        return;
    };
    let texts = |v: &[Event]| -> Vec<String> {
        let mut t: Vec<String> = v
            .iter()
            .filter(|e| e.is(&DECODE))
            .map(|e| format!("{} {}", e.time, e.get(&keys::TEXT).unwrap_or_default()))
            .collect();
        t.sort();
        t
    };
    assert_eq!(texts(&a), texts(&b));
}

/// The family's verification hooks on recorded vectors (what `sdrkit verify` runs): the settings they give the receiver and their
/// comparison pass for a mode with its code, for JS8 and for the modes without their code.
#[test]
fn the_vector_hooks_pass() {
    use sdrkit_core::verify::Outcome;
    let e = expected();
    let dir = sdrkit_dev_test::Harness::vectors("wsjt");
    for key in ["ft8", "js8", "fst4_60", "q65_60a"] {
        let section = &e["recordings"][key];
        let file = section["recording"].as_str().unwrap();
        let name = file.trim_end_matches(".sigmf-meta");
        let mode = section["receiver"]["mode"].as_str().unwrap();
        let desc = descriptors().into_iter().find(|d| d.id == mode).unwrap();
        let hooks = desc.verify.unwrap();
        let mut h = sdrkit_dev_test::Harness::new(desc);
        h = h.with_settings(&(hooks.vector_settings.unwrap())(name, section, &e));
        let Some(run) = h.run_vector("wsjt", name).unwrap() else {
            return;
        };
        let lines = (hooks.check_vector.unwrap())(&run.vector(name, section, &e, &dir));
        assert!(!lines.is_empty());
        for l in &lines {
            assert_ne!(l.outcome, Outcome::Fail, "{key} {}: {}", l.key, l.detail);
        }
        assert!(lines.iter().any(|l| l.outcome == Outcome::Pass), "{key}");
    }
}
