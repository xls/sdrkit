//! `pi4`: messages at several tone-0 frequencies and starts, the pass marks in noise, nothing from noise, one decode of one
//! transmission on a stream, and the test signal.

use crate::common::noise;
use crate::scatter::*;
use sdrkit_stack_wsjt::scatter::pi4::{self, read};
use sdrkit_stack_wsjt::PI4;

fn window(msg: &str, tone0: f64, start: f64, snr: f64, seed: u64) -> Vec<f32> {
    let x = pi4::audio(msg, 40, 8000.0, tone0, amp_at(snr, 8000.0)).unwrap();
    in_noise(&x, 30.0, start, seed)
}

#[test]
fn messages_at_each_frequency_and_start() {
    let msgs = ["PI4ABC", "K1JT", "DB0KO/B", "12345678", "A"];
    let mut k = 0;
    for m in msgs {
        for f in [700.0, 1500.0, 2200.0] {
            let start = [0.0, 1.3, 2.9][k % 3];
            k += 1;
            let a = window(m, f, start, 0.0, k as u64);
            let r = read(&a, 8000.0, 40, 400.0, 2600.0)
                .unwrap_or_else(|| panic!("{m} {f} {start}: nothing"));
            assert_eq!(r.text, m);
            assert!((r.tone0_hz - f).abs() <= 3.0, "{m} {f}: {}", r.tone0_hz);
            assert!(
                (r.start_s - start).abs() <= 0.1,
                "{m} {start}: {}",
                r.start_s
            );
        }
    }
}

fn passes(snr: f64) -> usize {
    (0..6)
        .filter(|&seed| {
            let a = window("PI4ABC", 1500.0, 2.0, snr, 100 + seed);
            read(&a, 8000.0, 40, 400.0, 2600.0).is_some_and(|r| r.text == "PI4ABC")
        })
        .count()
}

#[test]
fn pass_marks_in_noise() {
    assert_eq!(passes(5.0), 6);
    assert_eq!(passes(-8.0), 6);
    assert_eq!(passes(-15.0), 6);
    for snr in [-20.0, -23.0, -26.0] {
        eprintln!("PI4 at {snr} dB: {} of 6", passes(snr));
    }
}

#[test]
fn noise_gives_nothing() {
    for seed in 0..8 {
        let a = noise(32 * 8000, SIGMA, 120 + seed);
        assert!(read(&a, 8000.0, 40, 400.0, 2600.0).is_none(), "{seed}");
    }
}

#[test]
fn one_transmission_on_a_stream_is_one_decode() {
    let x = pi4::audio("K1JT", 40, 8000.0, 1200.0, amp_at(0.0, 8000.0)).unwrap();
    let a = in_noise(&x, 45.0, 7.0, 130);
    let mut rx = receiver(&PI4, &[]);
    let ev = feed_audio(rx.as_mut(), &a, 1700);
    let d = decodes(&ev);
    assert_eq!(
        d.len(),
        1,
        "{:?}",
        d.iter().map(|e| text(e)).collect::<Vec<_>>()
    );
    assert_eq!(text(d[0]), "K1JT");
    let carrier = d[0]
        .at(&sdrkit_stack_wsjt::typed::Pi4DecodeEvent::CARRIER_HZ)
        .unwrap();
    assert!((carrier - (1200.0 + 117.1875)).abs() < 3.0, "{carrier}");
}

#[test]
fn the_test_signal_decodes_each_minute() {
    let mut g = (PI4.make_generator.unwrap())(&sdrkit_core::GenConfig::new(
        48_000.0,
        sdrkit_core::Values::defaults(&(PI4.settings)()),
    ))
    .unwrap();
    let mut iq = vec![sdrkit_core::Complex32::default(); 125 * 48_000];
    g.generate(&mut iq);
    let mut rx = receiver(&PI4, &[]);
    rx.configure(&sdrkit_core::InputInfo::new(48_000.0, 50e6 + 20e3, 50e6).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    let mut ev = Vec::new();
    for c in iq.chunks(9600) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    let t: Vec<&str> = decodes(&ev).iter().map(|e| text(e)).collect();
    assert_eq!(t, ["K0TST", "K0TST"]);
    let tone0 = decodes(&ev)[0]
        .at(&sdrkit_stack_wsjt::typed::Pi4DecodeEvent::AUDIO_HZ)
        .unwrap();
    assert!((tone0 - 682.8125).abs() < 3.0, "{tone0}");
}

/// PI4 has no checksum: in noise the test signal (with its CW identification and carrier) gives its own text or nothing.
#[test]
fn the_test_signal_in_noise_gives_only_its_text() {
    for snr in [0.0, -15.0] {
        let mut g = (PI4.make_generator.unwrap())(&sdrkit_core::GenConfig::new(
            48_000.0,
            sdrkit_core::Values::defaults(&(PI4.settings)()),
        ))
        .unwrap();
        let mut iq = vec![sdrkit_core::Complex32::default(); 125 * 48_000];
        g.generate(&mut iq);
        crate::common::add_noise(&mut iq, snr, 48_000.0, 140);
        let mut rx = receiver(&PI4, &[]);
        rx.configure(&sdrkit_core::InputInfo::new(48_000.0, 50e6 + 20e3, 50e6).recording())
            .unwrap();
        let mut out = sdrkit_core::Output::default();
        let mut ev = Vec::new();
        for c in iq.chunks(9600) {
            rx.feed(c, &mut out);
            ev.append(&mut out.events);
            out.clear();
        }
        let t: Vec<&str> = decodes(&ev).iter().map(|e| text(e)).collect();
        assert!(
            !t.is_empty() && t.iter().all(|x| *x == "K0TST"),
            "{snr} dB: {t:?}"
        );
    }
}
