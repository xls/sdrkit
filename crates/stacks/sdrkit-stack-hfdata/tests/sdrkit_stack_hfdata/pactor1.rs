//! PACTOR-I: the recordings, round trips through the standard impairments, saved state and the receiver contract.

use crate::common::*;
use sdrkit_core::{Complex32, Generator, InputInfo, Receiver, SettingError, Value};
use sdrkit_stack_hfdata::pactor1;
use sdrkit_stack_hfdata::pactor1::gen::{self as pgen, Pactor1GenOptions, Pactor1Generator};
use sdrkit_stack_hfdata::pactor1::link::{self as l};
use sdrkit_stack_hfdata::pactor1::proto::Status;
use sdrkit_stack_hfdata::typed::{Pactor1Fields, PACTOR1_FRAMES, PACTOR1_STATIONS};

fn collapse(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == ' ' && out.ends_with(' ') {
            continue;
        }
        out.push(c);
    }
    out
}

fn the_text(rx: &dyn Receiver) -> String {
    field_text(rx, Pactor1Fields::TEXT.key)
}

fn count(rx: &dyn Receiver, key: &str) -> u64 {
    field(rx, key).unwrap_or(f64::NAN) as u64
}

fn pair_low(rx: &dyn Receiver) -> Option<f64> {
    field(rx, Pactor1Fields::PAIR_LOW.key)
}

#[test]
fn r9_arq_recording() {
    let rec = vector("pactor1_arq_24k");
    let exp = expected("pactor1_arq_24k");
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    run_vector(rx.as_mut(), &rec);
    let frames = rows(rx.results(), &PACTOR1_FRAMES);
    let want = exp["frames_oldest_first"].as_array().unwrap();
    assert_eq!(frames.len(), want.len());
    for (g, w) in frames.iter().zip(want) {
        assert_eq!(text(g, "kind"), w["kind"].as_str().unwrap());
        assert!(
            (num(g, "start") - w["t_s"].as_f64().unwrap()).abs() <= 0.01,
            "{g:?} {w}"
        );
        for (k, ours) in [
            ("baud", "symbol_rate_hz"),
            ("count", "number"),
            ("side", "side"),
        ] {
            if let Some(v) = w.get(k) {
                assert_eq!(num(g, ours), v.as_f64().unwrap(), "{k} of {w}");
            }
        }
        for (k, ours) in [
            ("huffman", "huffman"),
            ("inverted", "inverted"),
            ("repeat", "repeat"),
            ("fec", "fec_repeat"),
            ("break_in", "break_in"),
            ("qrt", "qrt"),
        ] {
            if let Some(v) = w.get(k) {
                assert_eq!(
                    cell(g, ours),
                    Value::Bool(v.as_bool().unwrap()),
                    "{k} of {w}"
                );
            }
        }
        if let Some(v) = w.get("header") {
            let got = format!("{:02X}", num(g, "header") as i64);
            assert_eq!(got, v.as_str().unwrap(), "header of {w}");
        }
        if let Some(v) = w.get("call") {
            assert_eq!(text(g, "call"), v.as_str().unwrap(), "call of {w}");
        }
        if let Some(t) = w.get("text") {
            assert_eq!(
                collapse(&text(g, "text")).trim(),
                t.as_str().unwrap(),
                "{w}"
            );
        }
    }
    assert_eq!(
        collapse(&the_text(rx.as_ref())).trim_end(),
        exp["text"].as_str().unwrap()
    );
    for (k, ours) in [
        ("packets_ok", "packets_ok"),
        ("packets_bad", "packets_bad"),
        ("control_signals", "control_signals"),
        ("memory_arq_saves", "memory_arq_saves"),
        ("repeats", "repeats"),
        ("fast", "packets_fast"),
        ("slow", "packets_slow"),
        ("changeovers", "changeovers"),
        ("connects", "connects"),
    ] {
        assert_eq!(
            count(rx.as_ref(), ours),
            exp["counters"][k].as_u64().unwrap(),
            "{k}"
        );
    }
    assert_eq!(
        value(rx.as_ref(), Pactor1Fields::FEC_MODE.key),
        Value::Bool(exp["fec_mode"].as_bool().unwrap())
    );
    assert!((pair_low(rx.as_ref()).unwrap() - 1433.0).abs() <= 3.0);
    let st = rows(rx.results(), &PACTOR1_STATIONS);
    let ws = exp["stations"].as_array().unwrap();
    assert_eq!(st.len(), ws.len());
    // The table lists the station heard last first, as the expected list does.
    for (g, w) in st.iter().rev().zip(ws) {
        assert_eq!(text(g, "call"), w["call"].as_str().unwrap());
        assert_eq!(num(g, "heard"), w["count"].as_f64().unwrap());
        assert_eq!(text(g, "seen_as"), w["how"].as_str().unwrap());
    }
}

#[test]
fn r9_fec_recording() {
    let rec = vector("pactor1_fec_24k");
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    run_vector(rx.as_mut(), &rec);
    let rx = rx.as_ref();
    assert!(count(rx, "packets_ok") >= 17);
    assert!(count(rx, "packets_bad") <= 1);
    assert_eq!(count(rx, "control_signals"), 0);
    assert!(count(rx, "repeats") >= 10);
    assert_eq!(value(rx, Pactor1Fields::FEC_MODE.key), Value::Bool(true));
    assert!((pair_low(rx).unwrap() - 1360.0).abs() <= 3.0);
    let text_all = collapse(&the_text(rx));
    assert_eq!(text_all.matches("CQ CQ DE").count(), 1, "{text_all}");
    assert!(
        text_all.contains("CQ CQ DE XX2BB XX2BB PSE K"),
        "{text_all}"
    );
    for f in rows(rx.results(), &PACTOR1_FRAMES) {
        if cell(&f, "repeat") == Value::Bool(true) {
            assert_eq!(cell(&f, "fec_repeat"), Value::Bool(true));
        }
    }
}

/// The clean script of `o` through the standard impairments: `gen_offset_hz` (tuning error and dial place), `gen_snr_db` (tone
/// SNR in 2.5 kHz) when given, and `extra`.
fn round_trip(
    o: Pactor1GenOptions,
    offset_hz: f64,
    snr_2k5: Option<f64>,
    extra: &[(&str, Json)],
    seed: u64,
) -> Box<dyn Receiver> {
    let rate = o.rate;
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(seed as f64));
    put(&mut v, "gen_offset_hz", json!(offset_hz));
    if let Some(snr) = snr_2k5 {
        put(&mut v, "gen_noise", json!("snr"));
        put(&mut v, "gen_snr_db", json!(snr));
    }
    for (k, x) in extra {
        put(&mut v, k, x.clone());
    }
    let g = Pactor1Generator::new(o);
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &v).expect("impairments");
    let x = generate(g.as_mut(), 60.0);
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    rx.configure(&InputInfo::new(rate, 14.08e6, 14.08e6).recording())
        .unwrap();
    feed(rx.as_mut(), &x, (rate / 10.0) as usize);
    rx
}

fn check_arq(rx: &dyn Receiver, low: f64, tol: f64) {
    let c = |k: &str| count(rx, k);
    assert_eq!(
        (c("packets_ok"), c("repeats"), c("control_signals")),
        (7, 1, 7)
    );
    assert_eq!(
        (
            c("changeovers"),
            c("connects"),
            c("packets_fast"),
            c("packets_slow")
        ),
        (1, 1, 2, 5)
    );
    assert_eq!(c("packets_bad"), 0);
    assert_eq!(value(rx, Pactor1Fields::FEC_MODE.key), Value::Bool(false));
    let all = collapse(&the_text(rx));
    for t in [
        "XX2BB de XX1AA",
        "tea set at sea",
        "ASCII at 200 baud 73",
        "a tea set",
        "[2]",
    ] {
        assert!(all.contains(t), "{t} in {all}");
    }
    assert!(rows(rx.results(), &PACTOR1_STATIONS)
        .iter()
        .any(|s| text(s, "call") == "XX1AA"));
    let frames = rows(rx.results(), &PACTOR1_FRAMES);
    assert!(
        frames
            .iter()
            .any(|f| cell(f, "inverted") == Value::Bool(true))
            && frames
                .iter()
                .any(|f| cell(f, "inverted") == Value::Bool(false))
    );
    assert!(
        frames.iter().any(|f| num(f, "header") == 85.0)
            && frames.iter().any(|f| num(f, "header") == 170.0)
    );
    let got = pair_low(rx).unwrap();
    assert!((got - low).abs() <= tol, "{got}");
}

#[test]
fn r1_arq_clean_250k() {
    let o = Pactor1GenOptions {
        rate: 250e3,
        ..Default::default()
    };
    // The dial is 20 kHz below the radio centre.
    let mut v = gen_values();
    put(&mut v, "gen_offset_hz", json!(-20e3));
    let mut g = sdrkit_components::sim::impaired(Box::new(Pactor1Generator::new(o)), &v).unwrap();
    let x = generate(g.as_mut(), 60.0);
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    rx.configure(&InputInfo::new(250e3, 14.08e6 + 20e3, 14.08e6).recording())
        .unwrap();
    feed(rx.as_mut(), &x, 25_000);
    check_arq(rx.as_ref(), 1400.0, 3.0);
}

#[test]
fn r2_r4_arq_impaired() {
    for (err, snr) in [
        (-100.0, 14.0),
        (-47.0, 14.0),
        (0.0, 14.0),
        (33.0, 14.0),
        (100.0, 14.0),
        (0.0, 10.0),
    ] {
        let rx = round_trip(Pactor1GenOptions::default(), err, Some(snr), &[], 5);
        check_arq(rx.as_ref(), 1400.0 + err, 4.0);
    }
}

#[test]
fn r3_sender_clock() {
    for ppm in [-150.0, 150.0] {
        let clock = [("gen_clock_ppm", json!(ppm))];
        let rx = round_trip(Pactor1GenOptions::default(), 0.0, Some(16.0), &clock, 5);
        check_arq(rx.as_ref(), 1400.0, 4.0);
    }
}

#[test]
fn r5_fec_clean() {
    let o = Pactor1GenOptions {
        script: pgen::fec_script(),
        ..Default::default()
    };
    let rx = round_trip(o, 0.0, None, &[], 1);
    let rx = rx.as_ref();
    assert_eq!(count(rx, "control_signals"), 0);
    assert_eq!(value(rx, Pactor1Fields::FEC_MODE.key), Value::Bool(true));
    assert!(count(rx, "packets_ok") >= 15);
    assert!(count(rx, "repeats") >= 10);
    let all = collapse(&the_text(rx));
    assert_eq!(
        all.matches("CQ CQ DE XX2BB XX2BB PSE K").count(),
        1,
        "{all}"
    );
}

#[test]
fn r7_noise_only() {
    let rate = 24e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
    let x: Vec<Complex32> = (0..(rate * 26.0) as usize)
        .map(|_| rng.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    rx.configure(&InputInfo::new(rate, 14.08e6, 14.08e6).recording())
        .unwrap();
    feed(rx.as_mut(), &x, 2400);
    assert!(rows(rx.results(), &PACTOR1_FRAMES).is_empty());
    assert_eq!(count(rx.as_ref(), "packets_ok"), 0);
    assert!(pair_low(rx.as_ref()).is_none());
}

#[test]
fn r8_lone_packet() {
    let ev = vec![pgen::packet(
        0,
        200,
        0xAA,
        &pgen::ascii("XX2BB test", 20),
        Status::default(),
    )];
    let o = Pactor1GenOptions {
        script: ev,
        centre_hz: 2100.0,
        ..Default::default()
    };
    let rx = round_trip(o, 0.0, Some(20.0), &[], 1);
    assert_eq!(count(rx.as_ref(), "packets_ok"), 1);
    assert!(collapse(&the_text(rx.as_ref())).contains("XX2BB test"));
    assert!((pair_low(rx.as_ref()).unwrap() - 2000.0).abs() <= 3.0);
}

#[test]
fn r6_memory_arq() {
    // One 200 baud packet sent six times (CS2 after the first five, CS1 after the last), the SNR stepping from -2.5 to 0 dB.
    let data = pgen::ascii("ABCDEFGHIJKLMNOPQRST", 20);
    let mut script = Vec::new();
    for k in 0..6 {
        script.push(pgen::packet(
            k,
            200,
            0xAA,
            &data,
            Status {
                number: 1,
                ..Default::default()
            },
        ));
        script.push(pgen::control(k, if k < 5 { 1 } else { 0 }));
    }
    let mut saved = false;
    for (i, snr) in [-2.5f64, -2.0, -1.5, -1.0, -0.5, 0.0]
        .iter()
        .cycle()
        .take(18)
        .enumerate()
    {
        let o = Pactor1GenOptions {
            script: script.clone(),
            ..Default::default()
        };
        let rx = round_trip(o, 0.0, Some(*snr), &[], 40 + i as u64);
        if count(rx.as_ref(), "memory_arq_saves") >= 1
            && collapse(&the_text(rx.as_ref())).contains("ABCDEFGHIJKLMNOPQRST")
        {
            saved = true;
            break;
        }
    }
    assert!(saved, "no memory ARQ save at any step");
}

#[test]
fn every_event_is_declared() {
    let mut g = Pactor1Generator::new(Pactor1GenOptions::default());
    let x = generate(&mut g, 60.0);
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    rx.configure(&InputInfo::new(24e3, 14.08e6, 14.08e6).recording())
        .unwrap();
    let out = feed(rx.as_mut(), &x, 2400);
    let d = l::declaration();
    assert_eq!(out.events.len(), 14);
    for e in &out.events {
        assert!(declared(&d, e));
        assert_eq!(e.freq_hz, 14.08e6);
    }
}

#[test]
fn the_generator_is_clean_at_unit_power() {
    let mut g = generator(&pactor1::PACTOR1, 24e3, 1);
    assert!(g.signal_info().is_some());
    assert_eq!(g.station_offset_hz(20e3), 0.0);
    let x = generate(g.as_mut(), 1.4);
    // The connect packet runs from 0.5 s to 1.46 s.
    let on = &x[(0.55 * 24e3) as usize..(1.4 * 24e3) as usize];
    let p = on.iter().map(|c| c.norm_sqr() as f64).sum::<f64>() / on.len() as f64;
    assert!((p - 1.0).abs() < 1e-3, "power {p}");
    assert!(x[..(0.45 * 24e3) as usize]
        .iter()
        .all(|c| c.norm_sqr() == 0.0));
}

#[test]
fn save_restore_and_clear() {
    let rec = vector("pactor1_arq_24k");
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    run_vector(rx.as_mut(), &rec);
    let state = rx.save_state().expect("a saved state");
    let mut b = receiver(&pactor1::PACTOR1, &[]);
    b.configure(&InputInfo::new(24e3, 14.08e6, 14.08e6).recording())
        .unwrap();
    b.restore_state(&state).unwrap();
    assert_eq!(rows(b.results(), &PACTOR1_FRAMES).len(), 14);
    assert_eq!(rows(b.results(), &PACTOR1_STATIONS).len(), 2);
    assert!(b.restore_state(&half(&state)).is_err());
    invoke(b.as_mut(), "clear", None);
    assert!(rows(b.results(), &PACTOR1_FRAMES).is_empty());
    assert!(the_text(b.as_ref()).is_empty());
}

#[test]
fn huffman_table_text() {
    let table =
        " \t10\ne\t011\nt\t00000\ns\t00100\na\t01000\n13\t001100\n10\t001101\nn\t110\no\t111\n";
    let mut rx = receiver(&pactor1::PACTOR1, &[]);
    rx.configure(&InputInfo::new(24e3, 14.08e6, 14.08e6).recording())
        .unwrap();
    set(rx.as_mut(), "huffman_table", json!(table)).unwrap();
    assert_eq!(
        field(rx.as_ref(), Pactor1Fields::HUFFMAN_ENTRIES.key),
        Some(9.0)
    );
    // A text that is not a table is refused, and the table stays.
    assert!(matches!(
        set(rx.as_mut(), "huffman_table", json!("not a table")),
        Err(SettingError::Malformed { .. })
    ));
    assert_eq!(
        field(rx.as_ref(), Pactor1Fields::HUFFMAN_ENTRIES.key),
        Some(9.0)
    );
}
