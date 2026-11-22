//! What the family registers beside its descriptors: one verification case per mode (the mode's test signal, which must fill the
//! decode table), and the settings and comparison of the recorded vectors.

use crate::mode::Mode;
use crate::slots::TimeChoice;
use crate::typed::{DecodeEvent, Js8FrameEvent, Js8MessageEvent, DECODE, JS8_FRAME, JS8_MESSAGE};
use sdrkit_core::verify::{
    CaseSetting, CheckLine, VectorRun, VerifyCase, VerifyHooks, VerifyOptions,
};
use sdrkit_core::{Choice, Event, Results, Value};
use serde_json::Value as Json;

pub(crate) static COMBINED: VerifyHooks = VerifyHooks {
    cases: fast_cases,
    vector_settings: Some(combined_vector_settings),
    check_vector: Some(check_vector),
};
pub(crate) static FAST: VerifyHooks = VerifyHooks {
    cases: fast_cases,
    vector_settings: Some(vector_settings),
    check_vector: Some(check_vector),
};
pub(crate) static SLOW: VerifyHooks = VerifyHooks {
    cases: slow_cases,
    vector_settings: Some(vector_settings),
    check_vector: Some(check_vector),
};

/// The slot clock starts at `utc`, the time of the first sample.
fn clock_from(utc: &str) -> [CaseSetting; 2] {
    [
        CaseSetting {
            key: "time_source",
            value: Value::Choice(TimeChoice::Start.index()),
        },
        CaseSetting {
            key: "time_start",
            value: Value::Text(utc.to_owned()),
        },
    ]
}

/// The slot clock starts with the test signal's first sample.
fn case(name: &str, o: &VerifyOptions, seconds: f64) -> VerifyCase {
    let mut c = VerifyCase::new(name, o, 50.0);
    c.settings.extend(clock_from("2026-01-01T00:00:00Z"));
    c.seconds = c.seconds.max(seconds);
    c.expect_audio = Some(false);
    c.check = Some(check_decodes);
    c
}

/// Two slots of 15 s or less.
fn fast_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case("test signal", o, 31.0)]
}

/// One slot of 60 or 120 s.
fn slow_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    vec![case("test signal", o, 125.0)]
}

fn rows(r: &Results) -> usize {
    ["decodes", "js8_frames"]
        .iter()
        .filter_map(|name| r.table(r.decl().table_id(name)?))
        .map(|t| t.len())
        .sum()
}

/// The decode table (or the JS8 frames) holds at least one row.
fn check_decodes(r: &Results) -> Vec<CheckLine> {
    let n = rows(r);
    vec![CheckLine::new(
        "decodes",
        n > 0,
        format!("{n} decodes listed"),
    )]
}

/// The key of the sub-mode setting of `mode`, if it has one.
fn submode_key(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Jt65 => Some("jt65_submode"),
        Mode::Jt4 => Some("jt4_submode"),
        Mode::Q65 => Some("q65_submode"),
        _ => None,
    }
}

/// The key of the period setting of `mode`, if it has one.
fn period_key(mode: Mode) -> Option<&'static str> {
    match mode {
        Mode::Q65 => Some("q65_period"),
        Mode::Fst4 => Some("fst4_period"),
        Mode::Fst4w => Some("fst4w_period"),
        Mode::Msk144 => Some("msk144_period"),
        _ => None,
    }
}

/// The key of the switch that turns the decoder of `mode` on in the combined stack, if it has one.
fn enable_key(mode: Mode) -> Option<&'static str> {
    Some(match mode {
        Mode::Ft8 => "enable_ft8",
        Mode::Ft4 => "enable_ft4",
        Mode::Ft2 => "enable_ft2",
        Mode::Wspr => "enable_wspr",
        Mode::Jt65 => "enable_jt65",
        Mode::Jt9 => "enable_jt9",
        Mode::Jt4 => "enable_jt4",
        Mode::Q65 => "enable_q65",
        Mode::Fst4 => "enable_fst4",
        Mode::Fst4w => "enable_fst4w",
        Mode::Msk144 => "enable_msk144",
        Mode::Js8 => return None,
    })
}

/// The settings of the receiver of the vector's stack for a recorded vector: the slot clock from the recording's start, and
/// the sub-mode and period that the section names when the stack lets the user choose them.
fn vector_settings(_name: &str, section: &Json, _expected: &Json) -> Vec<CaseSetting> {
    let rc = &section["receiver"];
    let mut m = Vec::new();
    match rc["start_utc"].as_str() {
        Some(t) => m.extend(clock_from(t)),
        None => m.push(CaseSetting {
            key: "time_source",
            value: Value::Choice(TimeChoice::Start.index()),
        }),
    }
    let Some(mode) = Mode::from_id(rc["mode"].as_str().unwrap_or("")) else {
        return m;
    };
    let schema = crate::settings::schema_for(Some(mode));
    if let (Some(key), Some(s)) = (submode_key(mode), rc["submode"].as_str()) {
        if let (true, Some(i)) = (
            schema.id(key).is_ok(),
            mode.submodes().iter().position(|k| *k == s),
        ) {
            m.push(CaseSetting {
                key,
                value: Value::Choice(u16::try_from(i).unwrap_or_default()),
            });
        }
    }
    if let (Some(key), Some(p)) = (period_key(mode), rc["period_s"].as_f64()) {
        if schema.id(key).is_ok() {
            m.push(CaseSetting {
                key,
                value: Value::Duration(p),
            });
        }
    }
    m
}

/// The settings of the combined receiver for a recorded vector: those of [`vector_settings`], and only the decoder of the
/// vector's stack switched on, as the section's expectations are those of that stack's receiver.
fn combined_vector_settings(name: &str, section: &Json, expected: &Json) -> Vec<CaseSetting> {
    let mut m = vector_settings(name, section, expected);
    let mode = section["receiver"]["mode"].as_str().unwrap_or("");
    if let Some(this) = Mode::from_id(mode) {
        for other in Mode::ALL {
            if let Some(key) = enable_key(other) {
                m.push(CaseSetting {
                    key,
                    value: Value::Bool(other == this),
                });
            }
        }
        for key in ["js8_normal", "js8_fast", "js8_turbo"] {
            m.push(CaseSetting {
                key,
                value: Value::Bool(this == Mode::Js8),
            });
        }
    }
    m
}

/// The UTC of the first sample of the recordings: 2026-01-01T00:00:00Z.
const START: f64 = 1_767_225_600.0;

fn num(v: &Json, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(f64::NAN)
}

/// The decodes of the run against the list `decodes` of the section: every one in its slot within the tolerances, nothing else.
fn vector_decodes(run: &VectorRun) -> CheckLine {
    let sec = run.section;
    let tol = &sec["tolerance"];
    let want = sec["decodes"].as_array().cloned().unwrap_or_default();
    let got: Vec<(&Event, DecodeEvent)> = run
        .events
        .iter()
        .filter_map(|e| Some((e, DECODE.read::<DecodeEvent>(e)?)))
        .collect();
    let mut bad = Vec::new();
    for x in &want {
        let msg = x["msg"].as_str().unwrap_or("");
        let slot = START + num(x, "slot_offset_s");
        match got
            .iter()
            .find(|(e, d)| d.text == msg && (e.time - slot).abs() < 0.01)
        {
            Some((_, d)) => {
                let (hz, dt, snr) = (d.audio_hz.0, d.dt_s, d.snr_db);
                if !((hz - num(x, "hz")).abs() <= num(tol, "hz")
                    && (dt - num(x, "dt_s")).abs() <= num(tol, "dt_s")
                    && (snr - num(x, "snr_db")).abs() <= num(tol, "snr_db"))
                {
                    bad.push(format!("{msg}: {hz:.2} Hz {dt:.3} s {snr} dB"));
                }
            }
            None => bad.push(format!("missing {msg}")),
        }
    }
    for (e, d) in &got {
        let known = want.iter().any(|x| {
            x["msg"].as_str() == Some(d.text)
                && (e.time - START - num(x, "slot_offset_s")).abs() < 0.01
        });
        if !known {
            bad.push(format!("extra {}", d.text));
        }
    }
    CheckLine::new(
        "decodes",
        bad.is_empty(),
        if bad.is_empty() {
            format!("{} decodes as listed", want.len())
        } else {
            bad.join("; ")
        },
    )
}

/// The JS8 frames, the assembled message and the stations of the section.
fn check_js8(run: &VectorRun, out: &mut Vec<CheckLine>) {
    let sec = run.section;
    let frames: Vec<(&Event, Js8FrameEvent)> = run
        .events
        .iter()
        .filter_map(|e| Some((e, JS8_FRAME.read::<Js8FrameEvent>(e)?)))
        .collect();
    let want = sec["frames"].as_array().cloned().unwrap_or_default();
    let missing: Vec<String> = want
        .iter()
        .filter(|x| {
            let slot = START + num(x, "slot_start_s");
            !frames
                .iter()
                .any(|(e, f)| Some(f.chars) == x["chars"].as_str() && (e.time - slot).abs() < 0.01)
        })
        .map(|x| x["text"].to_string())
        .collect();
    out.push(CheckLine::new(
        "frames",
        missing.is_empty() && frames.len() == want.len(),
        format!(
            "{} frames of {} listed; missing {missing:?}",
            frames.len(),
            want.len()
        ),
    ));
    let msgs: Vec<Js8MessageEvent> = run
        .events
        .iter()
        .filter_map(|e| JS8_MESSAGE.read::<Js8MessageEvent>(e))
        .filter(|m| m.complete)
        .collect();
    let all = sec["messages"].as_array().cloned().unwrap_or_default();
    let ok = all
        .iter()
        .all(|m| msgs.iter().any(|e| m["text"].as_str() == Some(e.text)));
    out.push(CheckLine::new(
        "messages",
        ok,
        format!("{} complete messages", msgs.len()),
    ));
    out.push(CheckLine::not_checked(
        "stations",
        "the stations table is checked by the family's tests",
    ));
}

/// A recorded vector: the decodes (or JS8 frames) of its section, and its status when the section gives one.
fn check_vector(run: &VectorRun) -> Vec<CheckLine> {
    let sec = run.section;
    let mut out = Vec::new();
    if sec.get("frames").is_some() {
        check_js8(run, &mut out);
    } else {
        out.push(vector_decodes(run));
    }
    if let Some(st) = sec.get("status") {
        let status = run.results.status();
        let lock_state = format!("{:?}", status.lock_state).to_lowercase();
        let summary = status.summary.as_str();
        let words = st["summary_contains"].as_str().unwrap_or("");
        out.push(CheckLine::new(
            "status",
            st["lock_state"].as_str() == Some(lock_state.as_str()) && summary.contains(words),
            format!("lock_state {lock_state}, {summary}"),
        ));
    }
    for (k, why) in [
        ("sent", "what was sent; the decodes are checked"),
        ("verified", "a note"),
        ("receiver", "the stack's settings, applied"),
    ] {
        if sec.get(k).is_some() {
            out.push(CheckLine::not_checked(k, why));
        }
    }
    out
}

/// The burst, ping and beacon stacks: the test signal must fill the decode table. The case runs long enough for the slowest
/// of them (PI4 sends one message a minute).
pub(crate) static SCATTER: VerifyHooks = VerifyHooks {
    cases: scatter_cases,
    vector_settings: None,
    check_vector: None,
};

fn scatter_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 50.0);
    c.seconds = c.seconds.max(65.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let n = r
            .fields()
            .at(&crate::typed::ScatterFields::DECODES_TOTAL)
            .unwrap_or(0);
        vec![CheckLine::new("decodes", n > 0, format!("{n} decodes"))]
    });
    vec![c]
}
