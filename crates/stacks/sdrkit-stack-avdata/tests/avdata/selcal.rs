//! SELCAL: the scene through the receiver, codes to tones, sideband and sound card input, and the recording.

use crate::common::{self, num, rows, text, Row};
use sdrkit_algo_sim::chain::ChainConfig;
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::{Complex32, Generator, Receiver};
use sdrkit_stack_avdata::selcal::generator::{Carry, AMPLITUDE};
use sdrkit_stack_avdata::selcal::{self, GenParams, SelcalGenerator};
use sdrkit_stack_avdata::typed::{SelcalFields, SELCAL_CALLS, SELCAL_CODES};

fn rx(rate: f64, centre: f64, dial: f64, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    common::receiver(&selcal::DESCRIPTOR, rate, centre, dial, settings)
}

fn calls(r: &dyn Receiver) -> Vec<Row> {
    rows(r.results(), &SELCAL_CALLS)
}

/// The scene with noise at `snr` dB in 6 kHz and a carrier offset, fed to a receiver.
fn run(
    p: GenParams,
    snr: f64,
    offset: f64,
    secs: f64,
    centre: f64,
    dial: f64,
    settings: &[(&str, &str)],
) -> Box<dyn Receiver> {
    let rate = p.rate_hz;
    let mut g = SelcalGenerator::new(p);
    let cfg = ChainConfig {
        noise: NoiseLevel::SnrDb {
            snr_db: snr,
            bandwidth_hz: 6e3,
        },
        offset_hz: offset,
        ..ChainConfig::default()
    };
    let mut ch = common::chain(&g, cfg);
    let mut r = rx(rate, centre, dial, settings);
    common::stream(&mut g, &mut ch, r.as_mut(), secs, 65_536);
    r
}

#[test]
fn three_calls() {
    for (snr, off) in [(40.0, 0.0), (25.0, 300.0)] {
        let dial = 8.891e6;
        let p = GenParams {
            rate_hz: 2e6,
            ..Default::default()
        };
        let r = run(
            p,
            snr,
            off,
            24.0,
            dial + 50e3,
            dial,
            &[
                ("demod", "fm"),
                ("aircraft_table", "AMCP\tN123AB\nDH-KS\tD-ABCD"),
            ],
        );
        let cs = calls(r.as_ref());
        let codes: Vec<&str> = cs.iter().rev().map(|c| text(c, "code")).collect();
        assert_eq!(codes, ["AM-CP", "DH-KS", "AM-CP"], "{snr} {off}");
        for c in &cs {
            let want = if text(c, "code") == "AM-CP" {
                "N123AB"
            } else {
                "D-ABCD"
            };
            assert_eq!(text(c, "aircraft"), want);
            assert_eq!(num(c, "dial"), dial);
        }
        let codes = rows(r.results(), &SELCAL_CODES);
        assert_eq!(codes.len(), 2);
        assert_eq!(text(&codes[0], "code"), "AM-CP");
    }
}

#[test]
fn code_to_tones() {
    assert_eq!(
        selcal::generator::tones("AM-CP").unwrap()[0],
        312.6f32 as f64
    );
    assert!(selcal::generator::tones("AM-C").is_none());
    assert!(selcal::generator::tones("AI-CP").is_none());
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("avdata/selcal_500k").expect("the SELCAL recording");
    let mut r = rx(rec.rate_hz, rec.centre_hz, 124.0e6, &[("demod", "am")]);
    common::run(r.as_mut(), &rec.samples, 65_536);
    let cs = calls(r.as_ref());
    assert_eq!(cs.len(), 1, "{cs:?}");
    let c = &cs[0];
    assert_eq!(text(c, "code"), "AM-CP");
    assert_eq!(text(c, "tones"), "312.6 977.2 / 384.6 1083.9");
    assert_eq!(num(c, "dial"), 124.0e6);
    assert!(
        (num(c, "signal_s") - 2.6).abs() < 0.3,
        "{}",
        num(c, "signal_s")
    );
    let codes = rows(r.results(), &SELCAL_CODES);
    assert_eq!(common::int(&codes[0], "heard"), 1);
}

#[test]
fn ssb_calls_with_a_mistuned_radio() {
    let dial = 8.891e6;
    for (carry, demod, mistune) in [
        (Carry::Usb, "usb", 15.0),
        (Carry::Usb, "auto", -20.0),
        (Carry::Lsb, "lsb", 10.0),
    ] {
        let p = GenParams {
            rate_hz: 500e3,
            carry,
            ..Default::default()
        };
        let r = run(
            p,
            25.0,
            mistune,
            14.0,
            dial + 50e3,
            dial,
            &[("demod", demod)],
        );
        let cs = calls(r.as_ref());
        let codes: Vec<&str> = cs.iter().rev().map(|c| text(c, "code")).collect();
        assert_eq!(codes, ["AM-CP", "DH-KS"], "{carry:?} {demod} {mistune} Hz");
    }
}

#[test]
fn sound_card_audio() {
    // 8 kHz audio in the real part: the calls as tones, nothing else.
    let rate = 8000.0;
    let mut x = vec![Complex32::default(); (14.0 * rate) as usize];
    for (i, v) in x.iter_mut().enumerate() {
        let t = i as f64 / rate;
        let tn = |code: &str, start: f64| -> f32 {
            let f = selcal::generator::tones(code).unwrap();
            let pair = if (start..start + 1.0).contains(&t) {
                Some((f[0], f[1]))
            } else if (start + 1.2..start + 2.2).contains(&t) {
                Some((f[2], f[3]))
            } else {
                None
            };
            pair.map_or(0.0, |(a, b)| {
                (0.25
                    * ((std::f64::consts::TAU * a * t).sin()
                        + (std::f64::consts::TAU * b * t).sin())) as f32
            })
        };
        *v = Complex32::new(tn("AM-CP", 1.0) + tn("DH-KS", 6.0), 0.0);
    }
    let mut r = rx(rate, 0.0, 0.0, &[("audio_input", "1")]);
    common::run(r.as_mut(), &x, 1000);
    let cs = calls(r.as_ref());
    let codes: Vec<&str> = cs.iter().rev().map(|c| text(c, "code")).collect();
    assert_eq!(codes, ["AM-CP", "DH-KS"]);
    assert!(cs.iter().all(|c| num(c, "dial") == 0.0));
}

#[test]
fn any_cut_returns_the_same_calls() {
    let mut g = SelcalGenerator::new(GenParams {
        rate_hz: 500e3,
        ..Default::default()
    });
    let mut x = vec![Complex32::default(); 500_000 * 8];
    g.generate(&mut x);
    common::noise(&mut x, AMPLITUDE * AMPLITUDE, 30.0, 6e3, 500e3, 5);
    let mut got = Vec::new();
    for chunk in [65_536usize, 4096, 7] {
        let mut r = rx(500e3, 8.941e6, 8.891e6, &[("demod", "fm")]);
        common::run(r.as_mut(), &x, chunk);
        got.push(
            calls(r.as_ref())
                .iter()
                .map(|c| {
                    (
                        text(c, "code").to_owned(),
                        (num(c, "signal_s") * 1e3).round() as i64,
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    assert!(!got[0].is_empty());
    assert_eq!(got[0], got[1]);
    assert_eq!(got[0], got[2]);
}

#[test]
fn every_event_is_declared() {
    let p = GenParams {
        rate_hz: 500e3,
        ..Default::default()
    };
    let mut g = SelcalGenerator::new(p);
    let mut x = vec![Complex32::default(); 500_000 * 8];
    g.generate(&mut x);
    let mut r = rx(500e3, 8.941e6, 8.891e6, &[("demod", "fm")]);
    let out = common::run(r.as_mut(), &x, 65_536);
    assert!(!out.events.is_empty());
    for e in &out.events {
        assert!(e.freq_hz > 0.0);
    }
}

#[test]
fn a_change_of_frequency_keeps_the_calls_and_the_codes() {
    let rec = sdrkit_dev_test::read_recording("avdata/selcal_500k").expect("the SELCAL recording");
    let mut r = rx(rec.rate_hz, rec.centre_hz, 124.0e6, &[("demod", "am")]);
    common::run(r.as_mut(), &rec.samples, 65_536);
    assert_eq!(calls(r.as_ref()).len(), 1);
    assert_eq!(rows(r.results(), &SELCAL_CODES).len(), 1);
    r.forget();
    assert_eq!(calls(r.as_ref()).len(), 1);
    assert_eq!(rows(r.results(), &SELCAL_CODES).len(), 1);
    assert_eq!(common::field(r.results(), "calls"), 1.0);
    assert_eq!(r.results().fields().at(&SelcalFields::TONE), None);
}
