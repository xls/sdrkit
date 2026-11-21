//! The recorded vectors of `tests/vectors/wsjt/` against `expected.json`, at the decoder level: each recording's upper sideband
//! audio cut into slots on the grid of its start time, every slot decoded, and the decodes compared with the expected list
//! (message exact, frequency, DT and SNR within the recording's tolerances, nothing else decoded).

use sdrkit_stack_wsjt::runner::{decode_slot, DecodeState};
use sdrkit_stack_wsjt::rx::js8;
use sdrkit_stack_wsjt::rx::slot::{Decode, Slot};
use sdrkit_stack_wsjt::{Mode, Sub};
use serde_json::Value;
use std::sync::Arc;

/// `None` when the recording is missing.
fn audio_of(name: &str) -> Option<Vec<f32>> {
    let rec = sdrkit_dev_test::read_recording(&format!("wsjt/{name}"))?;
    Some(rec.samples.iter().map(|c| c.re).collect())
}

fn slots(sub: Sub, audio: &[f32]) -> Vec<Slot> {
    let p = sub.period;
    let (w0, w1) = sub.window();
    let len = audio.len() as f64 / 8000.0;
    let mut out = Vec::new();
    let mut k = 0;
    loop {
        let s0 = k as f64 * p;
        let a = (s0 + w0).max(0.0);
        let b = (s0 + w1).min(len);
        // A slot is decoded when its window is complete, or (end of the recording) when the audio reaches t0 + half the
        // transmission.
        if s0 + sub.t0() + sub.tx_s() / 2.0 > len || (sub.mode == Mode::Msk144 && s0 + 1.0 > len) {
            break;
        }
        let i0 = (a * 8000.0) as usize;
        let i1 = ((b * 8000.0) as usize).min(audio.len());
        out.push(Slot {
            sub,
            utc: s0,
            audio: Arc::new(audio[i0..i1].to_vec()),
            rate: 8000.0,
            t_first: a - s0,
            search: false,
            dial_hz: 0.0,
            index: k,
            early: false,
            seq: 0,
        });
        k += 1;
    }
    out
}

fn decode_all(sub: Sub, audio: &[f32]) -> Vec<Decode> {
    let mut st = DecodeState::default();
    slots(sub, audio)
        .iter()
        .flat_map(|s| decode_slot(s, &mut st).decodes)
        .collect()
}

fn expected() -> Value {
    sdrkit_dev_test::sigmf::read_expected("wsjt").unwrap()
}

/// Compares; returns the lines of the report and whether everything matched (`None` when the recording is missing).
fn compare(key: &str, sub: Sub) -> Option<(Vec<String>, bool)> {
    let e = expected();
    let r = &e["recordings"][key];
    let name = r["recording"]
        .as_str()
        .unwrap()
        .trim_end_matches(".sigmf-meta");
    let tol = &r["tolerance"];
    let (thz, tdt, tsnr) = (
        tol["hz"].as_f64().unwrap(),
        tol["dt_s"].as_f64().unwrap(),
        tol["snr_db"].as_f64().unwrap(),
    );
    let got = decode_all(sub, &audio_of(name)?);
    let mut lines = Vec::new();
    let mut ok = true;
    let want = r["decodes"].as_array().unwrap();
    for x in want {
        let msg = x["msg"].as_str().unwrap();
        let slot = x["slot_offset_s"].as_f64().unwrap();
        match got
            .iter()
            .find(|d| d.text == msg && (d.slot - slot).abs() < 0.01)
        {
            Some(d) => {
                let good = (d.hz - x["hz"].as_f64().unwrap()).abs() <= thz
                    && (d.dt - x["dt_s"].as_f64().unwrap()).abs() <= tdt
                    && (d.snr as f64 - x["snr_db"].as_f64().unwrap()).abs() <= tsnr
                    && d.mirrored == x["mirrored"].as_bool().unwrap()
                    && d.cq == x["cq"].as_bool().unwrap()
                    && d.call == x["call"].as_str().unwrap()
                    && d.grid == x["grid"].as_str().unwrap()
                    && d.dbm == x["dbm"].as_i64().unwrap() as i32;
                ok &= good;
                lines.push(format!(
                    "{} {slot:5.1} {msg:24} hz {:.2}/{} dt {:.3}/{} snr {}/{} call {}/{} grid {}/{}",
                    if good { "ok " } else { "BAD" },
                    d.hz,
                    x["hz"],
                    d.dt,
                    x["dt_s"],
                    d.snr,
                    x["snr_db"],
                    d.call,
                    x["call"],
                    d.grid,
                    x["grid"]
                ));
            }
            None => {
                ok = false;
                lines.push(format!("MISSING {slot:5.1} {msg}"));
            }
        }
    }
    for d in &got {
        if !want.iter().any(|x| {
            x["msg"].as_str() == Some(&d.text)
                && (x["slot_offset_s"].as_f64().unwrap() - d.slot).abs() < 0.01
        }) {
            ok = false;
            lines.push(format!("EXTRA {:5.1} {} {:.1} Hz", d.slot, d.text, d.hz));
        }
    }
    Some((lines, ok))
}

fn check(key: &str, sub: Sub) {
    let Some((lines, ok)) = compare(key, sub) else {
        return;
    };
    for l in &lines {
        eprintln!("{key}: {l}");
    }
    assert!(ok, "{key} differs from expected.json");
}

#[test]
fn ft8_recording() {
    check("ft8", Sub::new(Mode::Ft8));
}

#[test]
fn ft4_recording() {
    check("ft4", Sub::new(Mode::Ft4));
}

#[test]
fn ft2_recording() {
    check("ft2", Sub::new(Mode::Ft2));
}

#[test]
fn wspr_recording() {
    check("wspr", Sub::new(Mode::Wspr));
}

#[test]
fn jt65a_recording() {
    check("jt65a", Sub::new(Mode::Jt65));
}

#[test]
fn jt9_recording() {
    check("jt9", Sub::new(Mode::Jt9));
}

#[test]
fn jt4a_recording() {
    check("jt4a", Sub::new(Mode::Jt4));
}

/// FST4, FST4W, MSK144 and Q65 without their codes: nothing is decoded, so nothing false either.
#[test]
fn modes_without_their_codes_give_nothing() {
    let e = expected();
    for (key, sub) in [
        ("msk144_15", Sub::new(Mode::Msk144)),
        ("fst4_60", Sub::new(Mode::Fst4).with_period(60.0)),
        ("fst4w_120", Sub::new(Mode::Fst4w).with_period(120.0)),
        ("q65_60a", Sub::new(Mode::Q65)),
    ] {
        let r = &e["recordings"][key];
        let name = r["recording"]
            .as_str()
            .unwrap()
            .trim_end_matches(".sigmf-meta");
        let Some(audio) = audio_of(name) else {
            return;
        };
        assert!(decode_all(sub, &audio).is_empty(), "{key}");
    }
}

#[test]
fn js8_recording() {
    let e = expected();
    let r = &e["recordings"]["js8"];
    let Some(audio) = audio_of("wsjt_js8_8k") else {
        return;
    };
    let mut frames = Vec::new();
    for sm in 0..3u8 {
        let sub = Sub::new(Mode::Js8).with_sub(sm);
        for slot in slots(sub, &audio) {
            let mut failed = 0;
            frames.extend(js8::decode(&slot, &mut failed));
        }
    }
    let want = r["frames"].as_array().unwrap();
    let names = ["Normal", "Fast", "Turbo"];
    let mut missing = 0;
    for x in want {
        let sm = names
            .iter()
            .position(|n| Some(*n) == x["submode"].as_str())
            .unwrap() as u8;
        let slot = x["slot_start_s"].as_f64().unwrap();
        let chars = x["chars"].as_str().unwrap();
        match frames
            .iter()
            .find(|f| f.submode == sm && (f.slot - slot).abs() < 0.01 && f.chars == chars)
        {
            Some(f) => {
                eprintln!(
                    "js8: ok  {} {slot:5.1} {} {:.1}/{} dt {:.2}/{} snr {}/{}",
                    names[sm as usize], f.text, f.hz, x["hz"], f.dt, x["dt_s"], f.snr, x["snr_db"]
                );
                assert_eq!(f.text, x["text"].as_str().unwrap());
                assert_eq!(f.flags as u64, x["flags"].as_u64().unwrap());
                assert!((f.hz - x["hz"].as_f64().unwrap()).abs() <= 2.0);
                assert!((f.dt - x["dt_s"].as_f64().unwrap()).abs() <= 0.1);
            }
            None => {
                missing += 1;
                eprintln!(
                    "js8: MISSING {} {slot:5.1} {}",
                    names[sm as usize], x["text"]
                );
            }
        }
    }
    for f in &frames {
        let known = want.iter().any(|x| {
            x["chars"].as_str() == Some(&f.chars)
                && (x["slot_start_s"].as_f64().unwrap() - f.slot).abs() < 0.01
        });
        assert!(known, "false frame {} at {}", f.text, f.slot);
    }
    assert_eq!(missing, 0, "{missing} JS8 frames missing");
}
