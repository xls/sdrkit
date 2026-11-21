//! `iscat`: messages of every length in both variants at several offsets, the pass marks in noise, nothing from noise, a
//! carrier or an FSK441 signal, and the automatic variant on a stream.

use crate::common::noise;
use crate::scatter::*;
use sdrkit_stack_wsjt::scatter::fsk441;
use sdrkit_stack_wsjt::scatter::iscat::{self, read, Variant};
use sdrkit_stack_wsjt::typed::IscatDecodeEvent;
use sdrkit_stack_wsjt::ISCAT;

fn signal(msg: &str, v: Variant, df: f64, snr: f64, seed: u64) -> Vec<f32> {
    let x = iscat::audio(msg, v, 8000.0, 14.0, df, amp_at(snr, 8000.0));
    in_noise(&x, 16.0, 0.37, seed)
}

#[test]
fn every_message_in_both_variants_at_each_offset() {
    let msgs = [
        "CQ DX K1JT FN20",
        "TEST",
        "W9XYZ 73",
        "A",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
    ];
    for v in [Variant::A, Variant::B] {
        for (i, m) in msgs.iter().enumerate() {
            for df in [0.0, -70.0, 45.0] {
                let a = signal(m, v, df, 15.0, i as u64);
                let r = read(&a, 8000.0, v, 100.0)
                    .unwrap_or_else(|| panic!("{} {m} {df}: nothing", v.name()));
                assert_eq!(r.text, *m, "{} {df}", v.name());
                assert!(
                    (r.df_hz - df).abs() <= 11.0,
                    "{} {m} {df}: {}",
                    v.name(),
                    r.df_hz
                );
            }
        }
    }
}

fn passes(snr: f64) -> usize {
    (0..6)
        .filter(|&seed| {
            let a = signal("CQ DX K1JT FN20", Variant::A, 20.0, snr, 60 + seed);
            read(&a, 8000.0, Variant::A, 100.0).is_some_and(|r| r.text == "CQ DX K1JT FN20")
        })
        .count()
}

#[test]
fn pass_marks_in_noise() {
    assert_eq!(passes(10.0), 6);
    assert_eq!(passes(0.0), 6);
    for snr in [-5.0, -10.0, -14.0] {
        eprintln!("ISCAT-A at {snr} dB: {} of 6", passes(snr));
    }
}

#[test]
fn noise_a_carrier_and_fsk441_give_nothing() {
    for seed in 0..12 {
        let a = noise(16 * 8000, SIGMA, 70 + seed);
        for v in [Variant::A, Variant::B] {
            assert!(read(&a, 8000.0, v, 100.0).is_none(), "{seed} {}", v.name());
        }
    }
    let c: Vec<f32> = (0..14 * 8000)
        .map(|i| {
            amp_at(20.0, 8000.0) * (std::f64::consts::TAU * 1300.0 * i as f64 / 8000.0).cos() as f32
        })
        .collect();
    let a = in_noise(&c, 16.0, 1.0, 90);
    let f = fsk441::audio(
        &"W9XYZ K1JT 26  ".repeat(40),
        8000.0,
        0.0,
        amp_at(20.0, 8000.0),
    );
    let b = in_noise(&f, 16.0, 1.0, 91);
    for x in [a, b] {
        for v in [Variant::A, Variant::B] {
            assert!(read(&x, 8000.0, v, 100.0).is_none(), "{}", v.name());
        }
    }
}

#[test]
fn the_automatic_variant_finds_b_on_a_stream() {
    let x = iscat::audio(
        "TEST 123",
        Variant::B,
        8000.0,
        14.0,
        -30.0,
        amp_at(5.0, 8000.0),
    );
    let a = in_noise(&x, 21.0, 5.0, 92);
    let mut rx = receiver(&ISCAT, &[]);
    let ev = feed_audio(rx.as_mut(), &a, 1000);
    let d = decodes(&ev);
    let last = d.last().expect("a decode");
    assert_eq!(text(last), "TEST 123");
    assert_eq!(last.at(&IscatDecodeEvent::VARIANT), Some("ISCAT-B"));
    assert!((last.at(&IscatDecodeEvent::AUDIO_HZ).unwrap() + 30.0).abs() <= 11.0);
    // The same message is not reported twice in a row.
    assert_eq!(d.iter().filter(|e| text(e) == "TEST 123").count(), 1);
}

#[test]
fn a_named_submode_sets_the_variant() {
    let x = iscat::audio(
        "TEST 123",
        Variant::B,
        8000.0,
        14.0,
        0.0,
        amp_at(10.0, 8000.0),
    );
    let a = in_noise(&x, 16.0, 1.0, 93);
    let schema = (ISCAT.settings)();
    let cfg =
        sdrkit_core::StackConfig::new(sdrkit_core::Values::defaults(&schema)).with_submode("a");
    let mut rx = (ISCAT.make_receiver)(&cfg).unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(8000.0, 50e6, 50e6).recording())
        .unwrap();
    assert!(decodes(&feed_audio(rx.as_mut(), &a, 4000)).is_empty());
}
