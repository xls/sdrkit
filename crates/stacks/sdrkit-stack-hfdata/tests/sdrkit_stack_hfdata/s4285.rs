//! STANAG 4285: the test signal, every coded profile found by its start of message through noise at the levels of the target
//! quality, the uncoded profiles when set, a D_PDU, noise alone.

use crate::common::*;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::s4285::signal::{ScramblerStart, PROFILES};
use sdrkit_stack_hfdata::s4285::{self, S4285};
use sdrkit_stack_hfdata::serial::gen::SymbolGen;
use sdrkit_stack_hfdata::serial::{psk8, TEST_TEXT};

fn messages(out: &sdrkit_core::Output) -> Vec<String> {
    event_texts(out, "s4285.message")
}

fn signal(profile: &str, secs: f64) -> Vec<Complex32> {
    test_signal(&S4285, 24e3, &[("gen_profile", json!(profile))], secs)
}

#[test]
fn the_test_signal_is_read() {
    let x = signal("600_short", 8.0);
    let (rx, out) = run_24k(&S4285, &x, 4801);
    let m = messages(&out);
    assert!(m.len() >= 2, "{m:?} {:?}", event_texts(&out, "s4285.event"));
    assert!(m.iter().all(|t| t == TEST_TEXT), "{m:?}");
    let ev = event_texts(&out, "s4285.event");
    assert!(ev.iter().any(|e| e == "4285 frame sync"), "{ev:?}");
    assert!(
        ev.iter()
            .any(|e| e == "4285: 600 bit/s coded (BPSK), short (start of message found)"),
        "{ev:?}"
    );
    assert!(ev.iter().any(|e| e == "4285: end of message"), "{ev:?}");
    assert!(field(rx.as_ref(), "messages_total").unwrap() >= 2.0);
}

#[test]
fn every_coded_profile_at_its_level() {
    let cases = [
        ("2400_short", 18.0),
        ("1200_short", 12.0),
        ("600_short", 8.0),
        ("300_short", 5.0),
        ("150_short", 3.0),
        ("75_short", 2.0),
        ("2400_long", 18.0),
        ("1200_long", 12.0),
        ("600_long", 8.0),
        ("300_long", 5.0),
        ("150_long", 3.0),
        ("75_long", 2.0),
    ];
    for (profile, snr) in cases {
        let p = PROFILES
            .iter()
            .find(|p| {
                p.name().starts_with(&format!(
                    "{} bit/s coded",
                    profile.split('_').next().unwrap()
                )) && p.long == profile.ends_with("long")
            })
            .unwrap();
        let frames = s4285::test_tribits(p, TEST_TEXT.as_bytes(), ScramblerStart::AllOnes).len();
        let secs = frames as f64 / 2400.0 + 1.0;
        let x = signal(profile, secs);
        let ch = Channel {
            snr_db: Some(snr),
            offset_hz: 60.0,
            seed: 4,
            ..Channel::default()
        };
        let (_, out) = run_24k(&S4285, &through(&x, 24e3, &ch), 4801);
        let m = messages(&out);
        assert!(
            !m.is_empty() && m[0] == TEST_TEXT,
            "{profile} at {snr} dB: {m:?} {:?}",
            event_texts(&out, "s4285.event")
        );
    }
}

#[test]
fn uncoded_profiles_when_set() {
    for (profile, snr) in [
        ("3600_uncoded", 28.0),
        ("2400_uncoded", 22.0),
        ("1200_uncoded", 16.0),
    ] {
        let x = signal(profile, 1.5);
        let ch = Channel {
            snr_db: Some(snr),
            offset_hz: 60.0,
            seed: 6,
            ..Channel::default()
        };
        let mut rx = receiver(&S4285, &[("profile", json!(profile))]);
        let f = S4285.tuning.default_hz;
        rx.configure(&sdrkit_core::InputInfo::new(24e3, f, f).recording())
            .unwrap();
        let mut out = feed(rx.as_mut(), &through(&x, 24e3, &ch), 4801);
        out.events.append(&mut end(rx.as_mut()).events);
        let m = messages(&out);
        let got: Vec<u8> = m.first().map(|s| s.bytes().collect()).unwrap_or_default();
        let want = TEST_TEXT.as_bytes();
        let wrong = want
            .iter()
            .enumerate()
            .filter(|(i, b)| got.get(*i) != Some(b))
            .count();
        assert!(wrong <= 3, "{profile}: {m:?}");
    }
}

#[test]
fn a_dpdu_is_found() {
    use sdrkit_proto_link::s5066::{encode, DPduSpec};
    let payload: Vec<u8> = (0..50u8).map(|i| i.wrapping_mul(11)).collect();
    let b = encode(&DPduSpec {
        kind: 7,
        address: vec![0x56, 0x78, 0x12, 0x34],
        rest: vec![0, 0, 0, 0, 0],
        segment: payload,
        ..DPduSpec::default()
    })
    .unwrap();
    let p = &PROFILES[1];
    let mut sym: Vec<Complex32> = s4285::test_tribits(p, &b, ScramblerStart::AllOnes)
        .into_iter()
        .map(psk8)
        .collect();
    sym.extend(vec![Complex32::default(); 2400]);
    let mut g = SymbolGen::serial(24e3, sym);
    let x = generate(&mut g, 4.0);
    let ch = Channel {
        snr_db: Some(12.0),
        seed: 9,
        ..Channel::default()
    };
    let (rx, out) = run_24k(&S4285, &through(&x, 24e3, &ch), 4801);
    let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::DPDUS);
    assert_eq!(rows.len(), 1, "{:?}", event_texts(&out, "s4285.event"));
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
        seed: 12,
        ..Channel::default()
    };
    let (rx, out) = run_24k(&S4285, &through(&x, 24e3, &ch), 4801);
    assert!(messages(&out).is_empty());
    assert_eq!(field_text(rx.as_ref(), "text"), "");
}

/// The verify case lasts long enough for a start inside a transmission: the start of message of that transmission is
/// missed, and the next transmission is read to its end within the case.
#[test]
fn the_verify_case_reads_a_message_when_it_starts_inside_a_transmission() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 5.0,
        clean: true,
        snr_db: 20.0,
    };
    let case = (S4285.verify.unwrap().cases)(&o).remove(0);
    let cycle = s4285::test_cycle_s(&s4285::GEN_DEFAULT);
    assert!(case.seconds >= 2.0 * cycle, "{} s for a cycle of {cycle} s", case.seconds);
    let late = 0.2;
    let x = signal("600_short", late + case.seconds);
    let (_, out) = run_24k(&S4285, &x[(late * 24e3) as usize..], 4801);
    let m = messages(&out);
    assert_eq!(m.first().map(String::as_str), Some(TEST_TEXT), "{m:?}");
}
