//! HF ALE: the recording, round trips through the standard impairments, saved state, and the receiver contract.

use crate::common::*;
use sdrkit_core::{Complex32, Generator, InputInfo, Receiver, Value};
use sdrkit_stack_hfdata::ale;
use sdrkit_stack_hfdata::ale::gen::{AleGenOptions, AleGenerator};
use sdrkit_stack_hfdata::ale::tables as t;
use sdrkit_stack_hfdata::typed::{
    AleFields, ALE_CALL, ALE_EVENTS, ALE_LEARNED_NETS, ALE_STATIONS, ALE_WORDS,
};

fn events(rx: &dyn Receiver) -> Vec<Row> {
    rows(rx.results(), &ALE_EVENTS)
}

#[test]
fn r6_the_recording() {
    let rec = vector("ale_24k");
    let exp = expected("ale_24k");
    let mut rx = receiver(&ale::ALE, &[]);
    run_vector(rx.as_mut(), &rec);
    let ev = events(rx.as_ref());
    let want = exp["events"].as_array().unwrap();
    assert_eq!(ev.len(), want.len());
    for (g, w) in ev.iter().zip(want) {
        assert_eq!(text(g, "type"), w["type"].as_str().unwrap());
        assert_eq!(text(g, "from"), w["from"].as_str().unwrap());
        assert_eq!(text(g, "to"), w["to"].as_str().unwrap());
        assert_eq!(cell(g, "tis"), Value::Bool(w["tis"].as_bool().unwrap()));
        assert!(
            (num(g, "start") - w["t_s"].as_f64().unwrap()).abs() <= 0.02,
            "time {g:?}"
        );
        assert!((num(g, "words") - w["words"].as_f64().unwrap()).abs() <= 1.0);
        if let Some(c) = w.get("cmd") {
            assert_eq!(text(g, "commands"), c.as_str().unwrap());
        }
        if let Some(a) = w.get("amd") {
            assert_eq!(text(g, "amd"), a.as_str().unwrap());
        }
        let off = num(g, "offset_hz");
        assert!((35.0..=45.0).contains(&off), "cfo {off}");
        let snr = num(g, "snr_db");
        assert!((12.0..=18.0).contains(&snr), "snr {snr}");
        let ber = num(g, "ber");
        assert!((0.003..=0.02).contains(&ber), "ber {ber}");
    }
    let st = rows(rx.results(), &ALE_STATIONS);
    let want = exp["stations"].as_array().unwrap();
    assert_eq!(st.len(), want.len());
    // The table lists the station heard last first, as the expected list does.
    for (g, w) in st.iter().rev().zip(want) {
        assert_eq!(text(g, "address"), w["address"].as_str().unwrap());
        assert_eq!(num(g, "heard"), w["count"].as_f64().unwrap());
        assert_eq!(text(g, "last_type"), w["last"].as_str().unwrap());
    }
    let nets: Vec<String> = rows(rx.results(), &ALE_LEARNED_NETS)
        .iter()
        .map(|r| text(r, "address"))
        .collect();
    let want: Vec<String> = exp["learned_nets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(nets, want);
    let ok = field(rx.as_ref(), AleFields::WORDS_OK.key).unwrap();
    assert!((96.0..=104.0).contains(&ok), "words {ok}");
    assert_eq!(
        value(rx.as_ref(), AleFields::IN_SYNC.key),
        Value::Bool(false)
    );
}

/// The clean test calls at `rate`, the dial `dial_off` from the radio centre, through the standard impairments: a tuning error
/// of `err_hz`, the SNR in 3 kHz `snr_3k`, and `ber` of the channel bits turned over.
fn round_trip(
    rate: f64,
    dial_off: f64,
    err_hz: f64,
    snr_3k: f64,
    ber: f64,
    seed: u64,
) -> Box<dyn Receiver> {
    let g = AleGenerator::new(AleGenOptions {
        rate,
        ber,
        seed,
        ..Default::default()
    });
    let v = values(
        &ale::ALE,
        &[
            ("gen_seed", json!(seed + 10)),
            ("gen_offset_hz", json!(dial_off + err_hz)),
            ("gen_noise", json!("snr")),
            ("gen_snr_db", json!(snr_3k)),
        ],
    );
    let mut g = sdrkit_components::sim::impaired(Box::new(g), &v).expect("impairments");
    let x = generate(g.as_mut(), 60.0);
    let mut rx = receiver(&ale::ALE, &[]);
    rx.configure(&InputInfo::new(rate, 14.109e6 - dial_off, 14.109e6).recording())
        .unwrap();
    feed(rx.as_mut(), &x, (rate / 10.0) as usize);
    rx
}

fn kinds(rx: &dyn Receiver) -> Vec<(String, String, String)> {
    events(rx)
        .iter()
        .map(|e| (text(e, "type"), text(e, "from"), text(e, "to")))
        .collect()
}

fn want() -> Vec<(String, String, String)> {
    [
        ("Sounding", "TSTA", ""),
        ("Individual", "TSTA", "TSTB"),
        ("Response", "TSTB", "TSTA"),
        ("Ack", "TSTA", "TSTB"),
        ("Net", "NETC", "NET"),
        ("Net response", "MEMA", "NETC"),
        ("Net response", "MEMB", "NETC"),
        ("Net", "NETC", "NET"),
        ("Individual", "TSTD", "TSTA"),
        ("Group", "GRPE", "GRPF, GRPG"),
        ("Sounding", "LONGADDR1", ""),
    ]
    .iter()
    .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
    .collect()
}

#[test]
fn r1_clean_250k() {
    let rx = round_trip(250e3, -20e3, 0.0, 25.0, 0.0, 1);
    assert_eq!(kinds(rx.as_ref()), want());
    let ev = events(rx.as_ref());
    assert_eq!(text(&ev[1], "commands"), "LQA BER 2 SINAD 24 MP 1");
    assert_eq!(cell(&ev[1], "dial"), Value::Hz(14.109e6));
    assert_eq!(text(&ev[8], "amd"), "QSY 14109 USB PSE");
    let st = rows(rx.results(), &ALE_STATIONS);
    let tsta = st.iter().find(|s| text(s, "address") == "TSTA").unwrap();
    assert_eq!(num(tsta, "heard"), 3.0);
}

#[test]
fn r2_r3_impaired() {
    for (err, seed) in [(50.0, 2), (-50.0, 4)] {
        let rx = round_trip(24e3, 0.0, err, 10.0, 0.02, seed);
        assert_eq!(kinds(rx.as_ref()), want(), "{err}");
        let ev = events(rx.as_ref());
        assert!((num(&ev[1], "offset_hz") - err).abs() <= 20.0);
        let mean_ber = ev.iter().map(|e| num(e, "ber")).sum::<f64>() / ev.len() as f64;
        assert!((0.01..=0.04).contains(&mean_ber), "{mean_ber}");
    }
}

#[test]
fn r4_noise_only() {
    let rate = 24e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let x: Vec<Complex32> = (0..(rate * 30.0) as usize)
        .map(|_| rng.complex_normal(2.0 * 0.02 * 0.02))
        .collect();
    let mut rx = receiver(&ale::ALE, &[]);
    rx.configure(&InputInfo::new(rate, 14.109e6, 14.109e6).recording())
        .unwrap();
    feed(rx.as_mut(), &x, 2400);
    assert_eq!(field(rx.as_ref(), AleFields::WORDS_OK.key), Some(0.0));
    assert!(events(rx.as_ref()).is_empty());
}

#[test]
fn every_event_is_declared() {
    let mut g = AleGenerator::new(AleGenOptions::default());
    let x = generate(&mut g, 60.0);
    let mut rx = receiver(&ale::ALE, &[]);
    rx.configure(&InputInfo::new(24e3, 14.109e6, 14.109e6).recording())
        .unwrap();
    let out = feed(rx.as_mut(), &x, 2400);
    let d = t::declaration();
    assert_eq!(out.events.len(), 11);
    for e in &out.events {
        assert!(e.is(&ALE_CALL));
        assert!(e.get(&sdrkit_core::publish::keys::TEXT).is_some());
        assert_eq!(e.freq_hz, 14.109e6);
    }
    assert!(d.table_id(ALE_EVENTS.key).is_some());
}

#[test]
fn the_generator_is_clean_at_unit_power() {
    let mut g = generator(&ale::ALE, 24e3, 1);
    assert!(g.signal_info().is_some());
    assert_eq!(g.station_offset_hz(20e3), 0.0);
    let x = generate(g.as_mut(), 4.0);
    // The first sounding runs from 0.6 s; take its steady part.
    let on = &x[(0.7 * 24e3) as usize..(2.0 * 24e3) as usize];
    let p = on.iter().map(|c| c.norm_sqr() as f64).sum::<f64>() / on.len() as f64;
    assert!((p - 1.0).abs() < 1e-3, "power {p}");
    assert!(x[..(0.5 * 24e3) as usize]
        .iter()
        .all(|c| c.norm_sqr() == 0.0));
}

#[test]
fn listen_returns_audio_and_decoding_is_unchanged() {
    let rec = vector("ale_24k");
    let x = &rec.samples[..24_000 * 6];
    let mut a = receiver(&ale::ALE, &[("listen", json!(true))]);
    a.configure(&InputInfo::new(24e3, rec.centre_hz, rec.centre_hz).recording())
        .unwrap();
    let out = feed(a.as_mut(), x, 2400);
    assert!(out.audio().len() >= 8_000 * 5);
    let mut b = receiver(&ale::ALE, &[]);
    b.configure(&InputInfo::new(24e3, rec.centre_hz, rec.centre_hz).recording())
        .unwrap();
    let out_b = feed(b.as_mut(), x, 2400);
    assert!(out_b.audio().is_empty());
    assert_eq!(rows(a.results(), &ALE_WORDS), rows(b.results(), &ALE_WORDS));
}

#[test]
fn save_restore_and_clear() {
    let rec = vector("ale_24k");
    let mut rx = receiver(&ale::ALE, &[]);
    run_vector(rx.as_mut(), &rec);
    let state = rx.save_state().expect("a saved state");
    let mut b = receiver(&ale::ALE, &[]);
    b.configure(&InputInfo::new(24e3, 14.109e6, 14.109e6).recording())
        .unwrap();
    b.restore_state(&state).unwrap();
    assert_eq!(events(b.as_ref()).len(), 11);
    assert_eq!(rows(b.results(), &ALE_LEARNED_NETS).len(), 1);
    assert!(b.restore_state(&half(&state)).is_err());
    assert_eq!(events(b.as_ref()).len(), 11);
    invoke(b.as_mut(), "clear", None);
    assert!(events(b.as_ref()).is_empty());
    assert!(rows(b.results(), &ALE_LEARNED_NETS).is_empty());
}
