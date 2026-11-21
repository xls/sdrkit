//! MIL-STD-188-110 serial tone: the test signal at every rate through noise at the levels of the target quality, a multipath
//! channel with a clock error, a D_PDU, noise alone.

use crate::common::*;
use sdrkit_algo_sim::fading::Profile;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::ms110::{self, signal::Rate, MS110};
use sdrkit_stack_hfdata::serial::gen::SymbolGen;
use sdrkit_stack_hfdata::serial::{psk8, TEST_TEXT};

fn messages(out: &sdrkit_core::Output) -> Vec<String> {
    event_texts(out, "ms110.message")
}

fn signal(bps: &str, il: &str, secs: f64) -> Vec<Complex32> {
    test_signal(
        &MS110,
        24e3,
        &[("gen_bps", json!(bps)), ("gen_interleaver", json!(il))],
        secs,
    )
}

#[test]
fn the_test_signal_is_read() {
    let x = signal("600", "short", 5.0);
    let (rx, out) = run_24k(&MS110, &x, 4801);
    let m = messages(&out);
    assert!(m.len() >= 2, "{m:?}");
    assert!(m.iter().all(|t| t == TEST_TEXT), "{m:?}");
    let ev = event_texts(&out, "ms110.event");
    assert!(
        ev.iter()
            .any(|e| e == "110 sync: 600 bit/s, short (0.6 s) interleaver"),
        "{ev:?}"
    );
    assert!(field(rx.as_ref(), "messages_total").unwrap() >= 2.0);
}

#[test]
fn every_rate_at_its_level() {
    let cases = [
        ("4800", "short", 25.0, 4.0),
        ("2400", "short", 18.0, 4.0),
        ("2400", "long", 18.0, 12.0),
        ("1200", "short", 12.0, 4.0),
        ("1200", "long", 12.0, 12.0),
        ("600", "short", 8.0, 4.0),
        ("300", "short", 5.0, 5.0),
        ("150", "short", 3.0, 7.0),
        ("75", "short", 1.0, 11.0),
        ("75", "long", 1.0, 16.0),
    ];
    for (bps, il, snr, secs) in cases {
        let x = signal(bps, il, secs);
        let ch = Channel {
            snr_db: Some(snr),
            offset_hz: 40.0,
            seed: 3,
            ..Channel::default()
        };
        let (_, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 4801);
        let m = messages(&out);
        assert!(
            !m.is_empty() && m[0] == TEST_TEXT,
            "{bps} {il} at {snr} dB: {m:?} {:?}",
            event_texts(&out, "ms110.event")
        );
    }
}

#[test]
fn a_second_path_and_a_clock_error() {
    let x = signal("1200", "short", 5.0);
    let ch = Channel {
        snr_db: Some(25.0),
        ppm: 20.0,
        fading: Some(Profile::Echo {
            delay_s: 0.002,
            level_db: -3.0,
            phase_deg: 70.0,
        }),
        seed: 5,
        ..Channel::default()
    };
    let (_, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 1000);
    let m = messages(&out);
    assert!(!m.is_empty() && m[0] == TEST_TEXT, "{m:?}");
}

#[test]
fn a_dpdu_is_found() {
    use sdrkit_proto_link::s5066::{encode, DPduSpec};
    let payload: Vec<u8> = (0..50u8).map(|i| i.wrapping_mul(7)).collect();
    let b = encode(&DPduSpec {
        kind: 7,
        address: vec![0x56, 0x78, 0x12, 0x34],
        rest: vec![0, 0, 0, 0, 0],
        segment: payload,
        ..DPduSpec::default()
    })
    .unwrap();
    let mut sym: Vec<Complex32> = ms110::test_tribits(Rate::B1200, false, &b)
        .into_iter()
        .map(psk8)
        .collect();
    sym.extend(vec![Complex32::default(); 1200]);
    let mut g = SymbolGen::serial(24e3, sym);
    let x = generate(&mut g, 3.0);
    let ch = Channel {
        snr_db: Some(12.0),
        seed: 9,
        ..Channel::default()
    };
    let (rx, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 4801);
    let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::DPDUS);
    assert_eq!(rows.len(), 1, "{:?}", event_texts(&out, "ms110.event"));
    let r = &rows[0];
    assert_eq!(text(r, "type"), "NON_ARQ_DATA");
    assert_eq!(text(r, "data_crc"), "good");
    assert_eq!(num(r, "payload_bytes"), 50.0);
    assert_eq!(text(r, "source"), "1234");
    assert_eq!(text(r, "destination"), "5678");
}

#[test]
fn noise_alone_gives_nothing() {
    let x = vec![Complex32::default(); 24_000 * 6];
    let ch = Channel {
        snr_db: Some(0.0),
        seed: 11,
        ..Channel::default()
    };
    let (rx, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 4801);
    assert!(messages(&out).is_empty());
    assert_eq!(field_text(rx.as_ref(), "text"), "");
}

/// The acquisition range is +-130 Hz: the products of neighbouring 8-symbol blocks of the preamble are unambiguous to +-150 Hz.
#[test]
fn acquisition_at_the_edges_of_the_range() {
    let x = signal("1200", "short", 5.0);
    for off in [-130.0, -96.0, 96.0, 130.0] {
        for (fading, ppm, seed) in [
            (None, 0.0, 13),
            (
                Some(Profile::Echo {
                    delay_s: 0.002,
                    level_db: -3.0,
                    phase_deg: 70.0,
                }),
                20.0,
                5,
            ),
        ] {
            let ch = Channel {
                snr_db: Some(25.0),
                offset_hz: off,
                ppm,
                fading: fading.clone(),
                seed,
                ..Channel::default()
            };
            let (rx, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 4801);
            let m = messages(&out);
            assert!(
                !m.is_empty() && m[0] == TEST_TEXT,
                "{off} Hz {fading:?}: {m:?} {:?}",
                event_texts(&out, "ms110.event")
            );
            let f = field(rx.as_ref(), "offset_hz");
            assert!(
                f.is_some_and(|f| (f - off).abs() < 3.0),
                "{off} Hz: shown {f:?}"
            );
        }
    }
}

/// Every rate at its level of the target quality at the edges of the acquisition range.
#[test]
fn every_rate_at_the_edges_of_the_range() {
    let cases = [
        ("4800", "short", 25.0, 4.0),
        ("2400", "short", 18.0, 4.0),
        ("2400", "long", 18.0, 12.0),
        ("1200", "short", 12.0, 4.0),
        ("1200", "long", 12.0, 12.0),
        ("600", "short", 8.0, 4.0),
        ("300", "short", 5.0, 5.0),
        ("150", "short", 3.0, 7.0),
        ("75", "short", 1.0, 11.0),
        ("75", "long", 1.0, 16.0),
    ];
    for (bps, il, snr, secs) in cases {
        let x = signal(bps, il, secs);
        for off in [-130.0, -96.0, 96.0, 130.0] {
            let ch = Channel {
                snr_db: Some(snr),
                offset_hz: off,
                seed: 3,
                ..Channel::default()
            };
            let (_, out) = run_24k(&MS110, &through(&x, 24e3, &ch), 4801);
            let m = messages(&out);
            assert!(
                !m.is_empty() && m[0] == TEST_TEXT,
                "{bps} {il} at {snr} dB, {off} Hz: {m:?}"
            );
        }
    }
}
