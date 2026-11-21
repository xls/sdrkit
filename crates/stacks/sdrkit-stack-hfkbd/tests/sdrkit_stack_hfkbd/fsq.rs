//! FSQ and IFKP: the round trips, a message to the station, any start tone, the recordings, and no allocation per sample.

use crate::common::*;
use sdrkit_core::{Choice, Record};
use sdrkit_dev_test::Run;
use sdrkit_stack_hfkbd as hf;

// Columns of the messages table.
const FROM: &str = "from";
const CRC_OK: &str = "crc_ok";
const TO: &str = "to";
const TRIGGER: &str = "trigger";
const TEXT: &str = "text";
const TO_ME: &str = "to_me";

fn station(call: &str) -> sdrkit_core::Station {
    sdrkit_core::Station {
        callsign: call.into(),
        ..Default::default()
    }
}

fn message_from(r: &Run, from: &str) -> Record {
    rows(r, "messages")
        .into_iter()
        .find(|m| cell_text(m, FROM) == from)
        .expect("a message")
}

#[test]
fn fsq_and_ifkp() {
    let r = run(&snr_2500(harness(&hf::FSQ), 10.0)
        .station(station("N0XYZ"))
        .seconds(40.0));
    let m = message_from(&r, "K0TST");
    assert_eq!(cell_text(&m, TO), "N0XYZ");
    assert_eq!(cell_flag(&m, CRC_OK), Some(true));
    assert_eq!(cell_flag(&m, TO_ME), Some(true));
    assert!((num(&r, "symbol_rate_hz") - 2.93).abs() < 0.05);
    let ifkp = snr_2500(harness(&hf::IFKP), 10.0).seconds(30.0);
    let r = run(&ifkp);
    assert!(
        text(&r).contains("K0TST: ifkp test message 0123456789"),
        "{:?}",
        text(&r)
    );
    assert!((num(&r, "symbol_rate_hz") - 3.906).abs() < 0.01);
    // An IFKP signal is never read as FSQ.
    let x = ifkp.generate().unwrap();
    let r = feed(&harness(&hf::FSQ), &x);
    assert!(!text(&r).contains("ifkp test"));
}

#[test]
fn fsq_hello_from_any_start_tone() {
    // "hello" decodes from start tones 0, 4, ..., 32, and a tone never repeats.
    use sdrkit_util_text::varicode::fsq::{Alphabet, Decoder};
    let mut syms = vec![];
    for b in b"hello" {
        let (a, m) = hf::fsq::code(hf::fsq::Kind::Fsq, *b).unwrap();
        syms.push(a);
        syms.extend(m);
    }
    for start in (0..33).step_by(4) {
        let mut tone = start;
        let mut tones = vec![tone];
        for &s in &syms {
            tone = (tone + s + 1) % 33;
            tones.push(tone);
        }
        assert!(tones.windows(2).all(|w| w[0] != w[1]) && tones.iter().all(|&t| t < 33));
        let mut d = Decoder::new(Alphabet::Fsq);
        let mut out = String::new();
        for w in tones.windows(2) {
            if let Some(Ok(b)) = d.push(((w[1] + 33 - w[0] - 1) % 33) as u8) {
                out.push(b as char);
            }
        }
        if let Some(Ok(b)) = d.flush() {
            out.push(b as char);
        }
        assert_eq!(out, "hello");
    }
    assert!(
        (hf::fsq::Kind::Ifkp.spacing() / hf::fsq::Kind::Fsq.spacing() - 4.0 / 3.0).abs() < 1e-12
    );
}

#[test]
fn fsq_recording() {
    let name = "hfkbd_fsq3_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(
            &harness(&hf::FSQ).piece(piece).station(station("N0XYZ")),
            name,
        );
        assert!(in_range(
            num(&r, "symbol_rate_hz"),
            &e["expect"]["symbol_rate_hz"]
        ));
        assert!(in_range(num(&r, "band_hz"), &e["expect"]["band_hz"]));
        assert!(in_range(num(&r, "snr_db"), &e["expect"]["snr_db"]));
        let want = &e["expect"]["message"];
        let m = message_from(&r, want["from"].as_str().unwrap());
        assert_eq!(cell_flag(&m, CRC_OK), want["crc_ok"].as_bool());
        assert_eq!(cell_text(&m, TO), want["to"].as_str().unwrap());
        let c = want["trigger"].as_str().unwrap().chars().next().unwrap();
        let t = hf::fsq::Trigger::from_char(c).unwrap();
        assert_eq!(
            cell(&m, TRIGGER),
            Some(&sdrkit_core::Value::Choice(t.index()))
        );
        assert!(
            cell_text(&m, TRIGGER).eq_ignore_ascii_case(want["trigger_meaning"].as_str().unwrap())
        );
        assert_eq!(cell_text(&m, TEXT), want["text"].as_str().unwrap());
        assert_eq!(cell_flag(&m, TO_ME), want["to_me"].as_bool());
    }
}

#[test]
fn ifkp_recording() {
    let name = "hfkbd_ifkp1_12k";
    let e = expected(name);
    for piece in [800, 4093] {
        let r = recording(&harness(&hf::IFKP).piece(piece), name);
        assert!(text(&r).contains(e["expect"]["contains"].as_str().unwrap()));
        assert!(in_range(
            num(&r, "symbol_rate_hz"),
            &e["expect"]["symbol_rate_hz"]
        ));
        assert!(in_range(num(&r, "band_hz"), &e["expect"]["band_hz"]));
        assert!(rows(&r, "messages").is_empty());
        assert!(rows(&r, "stations")
            .iter()
            .any(|s| cell_text(s, "callsign") == "K0TST"));
    }
}
