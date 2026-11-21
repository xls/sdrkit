//! Inmarsat Aero: the generator's P channels through the receiver (ACARS, ADS-C, log-ons, aircraft, system facts), and the known
//! answers of the coding layer.

use crate::common::{cell, number, set, values};
use sdrkit_algo_fec::conv::codes::INMARSAT_K7;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, InputInfo, Output, Receiver, StackConfig, TableDef};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satcom::modes::aero;
use sdrkit_stack_satcom::modes::aero::coding::{self, FrameDecoder, Layout};
use sdrkit_stack_satcom::typed;
use serde_json::json;

const RATE: f64 = 480e3;
const CENTRE: f64 = 1545.62e6;
const DIAL: f64 = 1545.57e6;

fn rx_for(settings: &[CaseSetting]) -> Box<dyn Receiver> {
    let values = values(&aero::DESCRIPTOR, settings);
    let mut rx = (aero::DESCRIPTOR.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, DIAL).recording())
        .unwrap();
    rx
}

fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    ev
}

fn signal(seconds: f64, settings: &[CaseSetting]) -> Vec<Complex32> {
    let mut h = Harness::new(&aero::DESCRIPTOR).rate(RATE).seconds(seconds);
    h = h.with_settings(settings);
    h.generate().unwrap()
}

fn num(rx: &dyn Receiver, key: &str) -> f64 {
    let r = rx.results();
    r.decl()
        .field_id(key)
        .and_then(|id| r.fields().value(id))
        .and_then(number)
        .unwrap_or_else(|| panic!("no number {key}"))
}

/// The table declaration of `table`.
fn def(table: &str) -> &'static TableDef {
    match table {
        "channels" => &typed::AERO_CHANNELS,
        "facts" => &typed::AERO_FACTS,
        "logons" => &typed::AERO_LOGONS,
        "messages" => &typed::AERO_MESSAGES,
        "aircraft" => &typed::AERO_AIRCRAFT,
        _ => &typed::AERO_SU_TYPES,
    }
}

/// The cells of a table as text, newest row first, in the order of the declared columns.
fn rows(rx: &dyn Receiver, table: &str) -> Vec<Vec<String>> {
    crate::common::rows(rx.results(), table)
        .into_iter()
        .map(|(_, r)| {
            def(table)
                .columns
                .iter()
                .map(|c| cell(r, c.key()))
                .collect()
        })
        .collect()
}

fn col(table: &str, key: &str) -> usize {
    def(table)
        .columns
        .iter()
        .position(|c| c.key() == key)
        .unwrap_or_else(|| panic!("no column {key} in {table}"))
}

fn noisy(mask: f64, seconds: f64) -> Vec<Complex32> {
    signal(
        seconds,
        &[
            CaseSetting::new(
                &sdrkit_stack_satcom::modes::aero::gen::GEN_CHANNELS,
                mask as i64,
            ),
            CaseSetting::new(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::EbN0,
            ),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_EBN0_DB, 12.0),
        ],
    )
}

#[test]
fn each_rate_decodes() {
    for (mask, rate, secs) in [(1.0, 600u32, 14.0), (2.0, 1200, 9.0), (4.0, 10500, 6.0)] {
        let x = noisy(mask, secs);
        let mut r = rx_for(&[]);
        feed(r.as_mut(), &x, 4096);
        let ch = rows(r.as_ref(), "channels");
        let (rc, sc, good, bad) = (
            col("channels", "rate"),
            col("channels", "state"),
            col("channels", "sus_good"),
            col("channels", "sus_bad"),
        );
        let c = ch
            .iter()
            .find(|c| c[rc] == rate.to_string())
            .unwrap_or_else(|| panic!("{rate}: {ch:?}"));
        assert_eq!(c[sc], "data", "{rate}");
        assert!(c[good].parse::<u64>().unwrap() >= 12, "{rate}: {c:?}");
        assert_eq!(c[bad], "0", "{rate}");
        let facts = rows(r.as_ref(), "facts");
        let ft = col("facts", "text");
        assert!(
            facts
                .iter()
                .any(|f| f[ft].starts_with("satellite 3 at 25.5E")),
            "{rate}"
        );
        assert!(
            facts
                .iter()
                .any(|f| f[ft].ends_with(&format!("at {rate} bit/s"))),
            "{rate}: {facts:?}"
        );
        assert!(!rows(r.as_ref(), "logons").is_empty());
        assert!(num(r.as_ref(), "frames_ok") > 0.0);
    }
}

#[test]
fn messages_positions_aircraft_and_retune_keeps_tables() {
    let x = signal(10.0, &[]);
    let mut r = rx_for(&[]);
    let ev = feed(r.as_mut(), &x, 32_768);
    assert_eq!(
        r.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    let st = col("channels", "state");
    assert_eq!(
        rows(r.as_ref(), "channels")
            .iter()
            .filter(|c| c[st] == "data")
            .count(),
        2
    );
    let msgs = rows(r.as_ref(), "messages");
    assert!(msgs.len() >= 8, "{}", msgs.len());
    assert!(msgs
        .iter()
        .all(|m| m[col("messages", "check_ok")] == "true"));
    let pos = col("messages", "position");
    let with_pos: Vec<_> = msgs.iter().filter(|m| !m[pos].is_empty()).collect();
    assert!(with_pos.len() >= 3, "{msgs:?}");
    let adsc = col("messages", "adsc");
    assert!(msgs.iter().any(|m| m[adsc].contains("basic")), "{msgs:?}");
    let ap = col("aircraft", "position");
    assert!(rows(r.as_ref(), "aircraft")
        .iter()
        .any(|a| !a[ap].is_empty()));
    assert!(!r.results().points().is_empty());
    assert!(ev.iter().any(|e| e.is(&typed::AERO_MESSAGE)));
    for e in &ev {
        assert!(
            e.is(&typed::AERO_MESSAGE) || e.is(&typed::AERO_LOGON),
            "{e:?}"
        );
        assert!(e.freq_hz > 1.5e9, "{e:?}");
    }
    let before = msgs.len();
    r.retuned(&InputInfo::new(RATE, 1545.72e6, 1545.67e6).recording());
    feed(r.as_mut(), &x[..(0.5 * RATE) as usize], 30_000);
    assert!(rows(r.as_ref(), "messages").len() >= before);
    // A change of frequency by the host keeps the logs, the aircraft and their counts. The channels and the count of
    // their frames go.
    let kept = |r: &dyn Receiver| {
        (
            rows(r, "messages").len(),
            rows(r, "logons").len(),
            rows(r, "aircraft").len(),
            rows(r, "facts").len(),
            rows(r, "su_types").len(),
            num(r, "frames_ok"),
            num(r, "positions"),
        )
    };
    let held = kept(r.as_ref());
    assert!(num(r.as_ref(), "frames") > 0.0);
    r.forget();
    assert_eq!(kept(r.as_ref()), held);
    assert!(rows(r.as_ref(), "channels").is_empty());
    assert_eq!(num(r.as_ref(), "frames"), 0.0);
    set(r.as_mut(), "filter_registration", json!("HB-JHM")).unwrap();
    feed(r.as_mut(), &x[..(0.3 * RATE) as usize], 30_000);
    let reg = col("messages", "registration");
    assert!(rows(r.as_ref(), "messages")
        .iter()
        .all(|m| m[reg] == "HB-JHM"));
}

#[test]
fn noise_has_no_channel_in_sync() {
    let x = Harness::new(&aero::DESCRIPTOR)
        .rate(RATE)
        .seconds(5.0)
        .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Floor,
        )
        .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -34.0)
        .generate()
        .unwrap();
    let mut r = rx_for(&[]);
    feed(r.as_mut(), &x, 65_536);
    let st = col("channels", "state");
    assert!(rows(r.as_ref(), "channels")
        .iter()
        .all(|c| c[st] == "seen" || c[st] == "carrier"));
    assert!(rows(r.as_ref(), "messages").is_empty());
}

#[test]
fn bad_setting_values_are_refused() {
    let mut r = rx_for(&[]);
    assert!(set(r.as_mut(), "satellite", json!("moon")).is_err());
    assert!(set(r.as_mut(), "show_failed", json!("maybe")).is_err());
    assert!(set(r.as_mut(), "search_limit_hz", json!(-1.0)).is_err());
}

#[test]
fn saved_state_restores_the_messages() {
    let x = signal(8.0, &[]);
    let mut a = rx_for(&[]);
    feed(a.as_mut(), &x, 32_768);
    let n = rows(a.as_ref(), "messages").len();
    assert!(n >= 1);
    let state = a.save_state().unwrap();
    let mut b = rx_for(&[]);
    b.restore_state(&state).unwrap();
    feed(b.as_mut(), &x[..(0.3 * RATE) as usize], 30_000);
    assert_eq!(rows(b.as_ref(), "messages").len(), n);
}

/// Frames of `l` with known information, coded on one continuous stream, as soft bits (positive = 1) of the coded part.
fn coded_frames(l: &Layout, count: usize) -> (Vec<Vec<u8>>, Vec<Vec<f32>>) {
    let mut enc = Encoder::new(INMARSAT_K7);
    let infos: Vec<Vec<u8>> = (0..count)
        .map(|k| {
            (0..l.info_bytes())
                .map(|i| (i * 7 + k * 31) as u8)
                .collect()
        })
        .collect();
    let soft = infos
        .iter()
        .map(|info| {
            let bits = coding::encode_frame(l, info, coding::header(0, 1, 2), &mut enc).unwrap();
            bits[l.uw_bits + l.header_bits + l.unused_bits..]
                .iter()
                .map(|&b| if b == 1 { 1.0 } else { -1.0 })
                .collect()
        })
        .collect();
    (infos, soft)
}

#[test]
fn aero_continuous_frames_decode_every_byte() {
    // The continuous code: the last bits of a frame are decided with the next frame's bits (the traceback margin), so every one of
    // the 72 bytes of every frame comes back, the last ones too.
    let l = Layout::of(1200);
    let (infos, soft) = coded_frames(&l, 4);
    let mut d = FrameDecoder::default();
    let mut out = Vec::new();
    for (k, s) in soft.iter().enumerate() {
        d.push(&l, s, k, &mut out);
    }
    d.flush(&mut out);
    assert_eq!(out.len(), 4);
    for (dec, k) in &out {
        assert_eq!(dec.bytes, infos[*k], "frame {k}");
        assert_eq!(dec.channel_errors, 0);
    }
}

#[test]
fn aero_soft_bits_reach_the_decoder_with_the_llr_sign() {
    // The demodulators give soft bits with positive meaning 1; the decoder takes LLRs with positive meaning 0. One sign change
    // between them: a clean frame with a few weak wrong bits still decodes, and the channel errors are counted.
    let l = Layout::of(600);
    let (infos, mut soft) = coded_frames(&l, 3);
    for k in [5usize, 100, 400] {
        soft[1][k] *= -0.2;
    }
    let mut d = FrameDecoder::default();
    let mut out = Vec::new();
    for (k, s) in soft.iter().enumerate() {
        d.push(&l, s, k, &mut out);
    }
    d.flush(&mut out);
    let f1 = out.iter().find(|(_, k)| *k == 1).unwrap();
    assert_eq!(f1.0.bytes, infos[1]);
    assert_eq!(f1.0.channel_errors, 3);
}

#[test]
fn aero_scrambler_known_answer() {
    // Inmarsat Aeronautical SDM: 1 + x + x^15 loaded with 110100101011001, the first 48 output bits.
    let mut b = [0u8; 48];
    coding::scrambler(&mut b);
    let s: String = b.iter().map(|v| char::from(b'0' + v)).collect();
    assert_eq!(s, "000100110001101111000100001001010000111110001100");
}

#[test]
fn every_test_signal_event_is_declared() {
    let x = signal(8.0, &[]);
    let mut r = rx_for(&[]);
    let ev = feed(r.as_mut(), &x, 32_768);
    assert!(ev.iter().any(|e| e.is(&typed::AERO_LOGON)));
    for e in &ev {
        assert!(
            e.is(&typed::AERO_MESSAGE) || e.is(&typed::AERO_LOGON),
            "{e:?}"
        );
        assert!(e.freq_hz > 1.5e9, "{e:?}");
    }
}
