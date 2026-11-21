//! Q65: the message and CRC symbols of the worked examples, the whole encoder against the encoder vectors when the recordings
//! folder holds the code table, and the decoder on test signals: levels, sub-modes, periods, noise alone and decoding time.
//!
//! sdrkit does not ship the Q65 code table. The decoder tests use the table of the recordings folder
//! (`wsjt/codes/q65_65_15.nbcode`) when it is there, else a test code made from a fixed seed
//! ([`super::common::test_q65_code`]), which is not the protocol code.

use super::codes::test_codes;
use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::Complex32;
use sdrkit_stack_wsjt::encode::{self, channel_symbols};
use sdrkit_stack_wsjt::proto::codes::{CodeId, CodeStore};
use sdrkit_stack_wsjt::proto::msg77::pack77;
use sdrkit_stack_wsjt::runner::{decode_slot, DecodeState};
use sdrkit_stack_wsjt::rx::slot::Decode;
use sdrkit_stack_wsjt::{Mode, Sub};

/// The worked examples: message, the 13 message symbols, the two CRC symbols.
const EXAMPLES: [(&str, [u8; 13], [u8; 2]); 3] = [
    (
        "CQ K1ABC FN42",
        [0, 0, 0, 0, 8, 4, 55, 47, 6, 40, 40, 25, 34],
        [51, 31],
    ),
    (
        "K1ABC W9XYZ -12",
        [2, 27, 55, 35, 20, 6, 5, 9, 55, 1, 62, 41, 50],
        [52, 49],
    ),
    (
        "W9XYZ K1ABC RRR",
        [3, 2, 36, 59, 32, 4, 55, 47, 6, 41, 62, 36, 34],
        [52, 41],
    ),
];

/// The code table of the recordings folder, when it holds the Q65 table.
fn folder_code() -> Option<CodeStore> {
    let dir = sdrkit_dev_test::recordings_dir().join("wsjt").join("codes");
    let mut c = CodeStore::default();
    c.load_dir(&dir).ok()?;
    c.symbols(CodeId::Q65).is_some().then_some(c)
}

/// The codes of the decoder tests, and whether the Q65 code is the table of the recordings folder.
fn codes() -> (CodeStore, bool) {
    match folder_code() {
        Some(c) => (c, true),
        None => (test_codes(), false),
    }
}

fn q65(period: f64, letter: u8) -> Sub {
    Sub::new(Mode::Q65).with_period(period).with_sub(letter)
}

/// One period of `sub` with the stations and noise, decoded: the decodes and the decoding time in ms.
fn one_period(sub: Sub, st: &[St], seed: u64, codes: &CodeStore) -> (Vec<Decode>, f64) {
    let a = audio_with(sub, st, sub.period, true, seed, codes);
    let mut state = DecodeState {
        codes: codes.clone(),
        ..Default::default()
    };
    let slot = &slots(sub, &a)[0];
    let r = decode_slot(slot, &mut state);
    (r.decodes, r.ms)
}

/// How many of `seeds` periods of `sub` decode `text` sent at `snr` dB, the frequency and the start moved a little with each
/// seed. A decode of another text fails the test.
fn decoded(sub: Sub, text: &str, snr: f64, seeds: std::ops::Range<u64>, codes: &CodeStore) -> usize {
    let mut ok = 0;
    for seed in seeds {
        let hz = 1000.0 + 37.3 * (seed % 8) as f64;
        let dt = 0.1 * (seed % 4) as f64;
        let (d, _) = one_period(sub, &[(text, hz, snr, dt, false)], 100 + seed, codes);
        assert!(d.iter().all(|x| x.text == text), "{}: false decode {d:?}", sub.label());
        ok += d.len().min(1);
    }
    ok
}

#[test]
fn the_message_and_crc_symbols_of_the_worked_examples() {
    for (text, msg, crc) in EXAMPLES {
        let info = encode::q65_info(&pack77(text).unwrap());
        assert_eq!(info[..13], msg, "{text}");
        assert_eq!(info[13..], crc, "{text}");
        assert_eq!(encode::q65_crc(&msg), crc, "{text}");
        assert_eq!(info[12] & 1, 0, "the pad bit of {text}");
    }
    // The first example's CRC as a number, highest power first: 1100 1111 1110.
    let [a, b] = encode::q65_crc(&EXAMPLES[0].1);
    let bits = (0..6).map(|i| a >> i & 1).chain((0..6).map(|i| b >> i & 1));
    assert_eq!(bits.fold(0u16, |v, x| v << 1 | u16::from(x)), 0xCFE);
}

/// With the code table of the recordings folder: every Q65 frame of the encoder vectors, tone for tone, and the identity
/// that closes the accumulator of the table (the last check symbol is a fixed multiple of one information symbol).
#[test]
fn the_worked_examples_tone_for_tone_with_the_loaded_table() {
    let Some(codes) = folder_code() else {
        println!("skipped: the recordings folder holds no wsjt/codes/q65_65_15.nbcode");
        return;
    };
    let path = sdrkit_dev_test::sigmf::vectors_dir().join("wsjt/encode_vectors.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut n = 0;
    for v in v["vectors"].as_array().unwrap().iter().filter(|v| v["mode"] == "Q65") {
        let msg = v["msg"].as_str().unwrap();
        let want: Vec<u8> = v["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as u8)
            .collect();
        let got = channel_symbols(&Sub::new(Mode::Q65), msg, &codes).unwrap();
        assert_eq!(got, want, "{msg}");
        assert_eq!(got.len(), 85);
        assert_eq!(got.iter().filter(|&&t| t == 0).count(), 22, "{msg}");
        n += 1;
    }
    assert_eq!(n, 3);
    let code = codes.symbols(CodeId::Q65).unwrap();
    assert_eq!(code.checks(), 51);
    let mut r = rng(15);
    let mut word = [0u16; 65];
    for _ in 0..200 {
        let info: Vec<u16> = (0..15).map(|_| r.below(64) as u16).collect();
        code.encode(&info, &mut word).unwrap();
        assert_eq!(code.unsatisfied(&word).unwrap(), 0);
        assert_eq!(word[64], code.field().mul(code.field().exp(17), info[10]));
    }
}

/// The test signal of the generator (four stations, 0 to -8 dB) decodes in the default settings once a table is loaded.
#[test]
fn the_test_signal_decodes() {
    let (codes, _) = codes();
    let sub = Sub::new(Mode::Q65);
    let st: Vec<St> = sdrkit_stack_wsjt::generator::stations(Mode::Q65)
        .iter()
        .map(|&(t, hz, snr, dt)| (t, hz, snr, dt, false))
        .collect();
    let (d, _) = one_period(sub, &st, 1, &codes);
    assert_eq!(d.len(), st.len(), "{d:?}");
    for s in &st {
        let x = d.iter().find(|x| x.text == s.0).unwrap_or_else(|| panic!("{} missing", s.0));
        assert!((x.hz - s.1).abs() < 1.0, "{} at {} Hz", s.0, x.hz);
        assert!((x.dt - s.3).abs() < 0.15, "{} at {} s", s.0, x.dt);
        assert!((f64::from(x.snr) - s.2).abs() <= 3.5, "{} at {} dB", s.0, x.snr);
        assert_eq!((x.label.as_str(), x.mirrored), ("Q65-60A", false));
    }
}

/// The generator and the stack with the same `code_dir`: the stack decodes the generator's signal in the default settings.
#[test]
fn the_stack_decodes_its_generator_with_a_code_folder() {
    use sdrkit_core::{GenConfig, Receiver, StackConfig, Value};
    let d = std::env::temp_dir().join(format!("sdrkit-wsjt-{}-q65", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join(CodeId::Q65.file_name()), test_q65_code()).unwrap();
    let settings = settings(Some(Mode::Q65), &[("code_dir", Value::Text(d.display().to_string()))]);
    let mut g = sdrkit_stack_wsjt::generator::WsjtGenerator::from_config(
        Mode::Q65,
        &GenConfig::new(12000.0, settings.clone()).with_seed(1),
    );
    let mut iq = vec![Complex32::default(); 12000 * 62];
    assert_eq!(g.generate(&mut iq), iq.len());
    let mut rx = sdrkit_stack_wsjt::receiver::receiver(Some(Mode::Q65), &StackConfig::new(settings)).unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(12000.0, 50.275e6, 50.275e6).recording())
        .unwrap();
    let got = texts(&feed(&mut rx, &iq, 12000));
    for (text, ..) in sdrkit_stack_wsjt::generator::stations(Mode::Q65) {
        assert!(got.contains(&text.to_string()), "{text} missing: {got:?}");
    }
    let st = rx.results().status();
    assert_ne!(st.lock_state, sdrkit_core::LockState::Off);
    assert!(st.summary.contains("no public document"), "{}", st.summary);
}

/// -10 dB at 1500 Hz in the A sub-mode of every period: decoded, the start within 0.15 s, the frequency within 1 Hz, the
/// SNR within 3 dB. The decoding time of each period (of this test build, with the other tests running) goes to the test
/// output and must stay under half the period.
#[test]
fn every_period_decodes_and_in_time() {
    let (codes, _) = codes();
    for period in [15.0, 30.0, 60.0, 120.0, 300.0] {
        let sub = q65(period, 0);
        let (d, ms) = one_period(sub, &[("CQ K1ABC FN42", 1500.0, -10.0, 0.0, false)], 7, &codes);
        println!("{}: one period decoded in {ms:.0} ms", sub.label());
        assert_eq!(d.len(), 1, "{}: {d:?}", sub.label());
        assert_eq!(d[0].text, "CQ K1ABC FN42");
        assert!(d[0].dt.abs() < 0.15, "{}: dt {}", sub.label(), d[0].dt);
        assert!((d[0].hz - 1500.0).abs() < 1.0, "{}: {} Hz", sub.label(), d[0].hz);
        assert!((d[0].snr + 10).abs() <= 3, "{}: {} dB", sub.label(), d[0].snr);
        assert!(ms < 500.0 * period, "{}: {ms:.0} ms", sub.label());
        // Noise alone: every candidate runs every iteration.
        let (d, ms) = one_period(sub, &[], 8, &codes);
        println!("{}: one period of noise decoded in {ms:.0} ms", sub.label());
        assert!(d.is_empty(), "{}: {d:?}", sub.label());
        assert!(ms < 500.0 * period, "{}: {ms:.0} ms", sub.label());
    }
}

/// The wider sub-modes at -12 dB, 900 Hz, 0.3 s late.
#[test]
fn the_wider_sub_modes_decode() {
    let (codes, _) = codes();
    for sub in [q65(60.0, 1), q65(60.0, 2), q65(60.0, 3), q65(60.0, 4), q65(30.0, 1), q65(15.0, 2)] {
        let (d, _) = one_period(sub, &[("K1ABC W9XYZ -15", 900.0, -12.0, 0.3, false)], 11, &codes);
        assert_eq!(d.len(), 1, "{}: {d:?}", sub.label());
        assert_eq!(d[0].text, "K1ABC W9XYZ -15");
        assert!((d[0].dt - 0.3).abs() < 0.15, "{}: dt {}", sub.label(), d[0].dt);
    }
}

/// The levels, over 10 fixed seeds each. Q65-60A: 10 of 10 at -24 dB (the level the specification requires) and at least 4
/// of 10 at -27 dB. Q65-30B: 10 of 10 at -21 dB and at least 4 of 10 at -24 dB. The counts go to the test output. The
/// published level of half the decodes of one period without a-priori information is -27.6 dB (-24.8 dB at 30 s).
#[test]
fn the_levels_of_60a_and_30b() {
    let (codes, real) = codes();
    println!("Q65 code: {}", if real { "the table of the recordings folder" } else { "the test code" });
    for (sub, sure, edge) in [(q65(60.0, 0), -24.0, -27.0), (q65(30.0, 1), -21.0, -24.0)] {
        let a = decoded(sub, "CQ K1ABC FN42", sure, 0..10, &codes);
        let b = decoded(sub, "CQ K1ABC FN42", edge, 0..10, &codes);
        println!("{}: {a} of 10 at {sure} dB, {b} of 10 at {edge} dB", sub.label());
        assert_eq!(a, 10, "{} at {sure} dB", sub.label());
        assert!(b >= 4, "{}: {b} of 10 at {edge} dB", sub.label());
    }
}

/// Three stations in one period, two of them 40 Hz apart.
#[test]
fn three_stations_decode() {
    let (codes, _) = codes();
    let st: [St; 3] = [
        ("CQ K1ABC FN42", 1000.0, -18.0, 0.0, false),
        ("K1ABC W9XYZ -12", 1040.0, -18.0, 0.5, false),
        ("CQ DX G4ABC IO91", 2300.0, -20.0, -0.7, false),
    ];
    let (d, _) = one_period(Sub::new(Mode::Q65), &st, 5, &codes);
    for s in &st {
        assert!(d.iter().any(|x| x.text == s.0), "{} missing: {d:?}", s.0);
    }
    assert_eq!(d.len(), 3, "{d:?}");
}

/// A signal at -22 dB whose frequency moves by 0.15 Hz/s up and down (4.6 symbol rates over the transmission) and by
/// 0.2 Hz/s (6.1 symbol rates).
#[test]
fn drifting_signals_decode() {
    let (codes, _) = codes();
    let sub = Sub::new(Mode::Q65);
    let text = "W9XYZ K1ABC EN37";
    for (i, hz_per_s) in [0.15, -0.15, 0.2].into_iter().enumerate() {
        let t = encode::Transmission {
            sub,
            hz: 1500.0,
            amp: amp(-22.0),
            mirrored: false,
        };
        let tones = channel_symbols(&sub, text, &codes).unwrap();
        let mut z = Vec::new();
        encode::baseband_of(&t, &tones, 8000.0, 0.0, &mut z);
        let mut a = noise((sub.period * 8000.0) as usize, SIGMA, 31 + i as u64);
        let start = ((sub.t0() + 0.2) * 8000.0) as usize;
        let mid = z.len() as f64 / 2.0;
        for (j, v) in z.iter().enumerate() {
            // The phase of a frequency offset that grows in proportion to the time from the middle.
            let s = (j as f64 - mid) / 8000.0;
            let (sin, cos) = (std::f64::consts::PI * hz_per_s * s * s).sin_cos();
            a[start + j] += v.im * cos as f32 + v.re * sin as f32;
        }
        let mut state = DecodeState {
            codes: codes.clone(),
            ..Default::default()
        };
        let d = decode_slot(&slots(sub, &a)[0], &mut state).decodes;
        assert_eq!(d.len(), 1, "{hz_per_s} Hz/s: {d:?}");
        assert_eq!(d[0].text, text);
        assert!((d[0].dt - 0.2).abs() < 0.15, "{hz_per_s} Hz/s: dt {}", d[0].dt);
        assert!((d[0].hz - 1500.0).abs() < 1.5, "{hz_per_s} Hz/s: {} Hz", d[0].hz);
    }
}

/// Noise alone: no decode in 200 periods of 15 s (70 of Q65-15A, 65 each of 15B and 15C).
#[test]
fn noise_gives_no_decode_in_200_periods() {
    let (codes, _) = codes();
    let mut state = DecodeState {
        codes,
        ..Default::default()
    };
    let mut periods = 0;
    for (i, (sub, n)) in [(q65(15.0, 0), 70.0), (q65(15.0, 1), 65.0), (q65(15.0, 2), 65.0)]
        .into_iter()
        .enumerate()
    {
        let a = noise((n * sub.period * 8000.0) as usize, SIGMA, 650 + i as u64);
        for slot in slots(sub, &a) {
            let r = decode_slot(&slot, &mut state);
            assert!(r.decodes.is_empty(), "{}: {:?}", sub.label(), r.decodes);
            periods += 1;
        }
    }
    assert_eq!(periods, 200);
}

/// The sub-modes that the standard does not define are not taken: 15 s has A to C, 30 s has A to D.
#[test]
fn undefined_sub_modes_are_refused() {
    assert_eq!(q65(15.0, 3).sub, 0);
    assert_eq!(q65(15.0, 2).label(), "Q65-15C");
    assert_eq!(q65(30.0, 4).sub, 0);
    assert_eq!(q65(30.0, 3).label(), "Q65-30D");
    assert_eq!(q65(300.0, 4).label(), "Q65-300E");
    // In the stack: the letter E stays refused at 15 s, and 15 s stays refused at the letter E.
    let schema = sdrkit_stack_wsjt::settings::schema_for(Some(Mode::Q65));
    let mut rx = receiver(Some(Mode::Q65), &[("q65_period", sdrkit_core::Value::Duration(15.0))], 8000.0, 0.0);
    let id = schema.id("q65_submode").unwrap();
    assert!(sdrkit_core::owner::apply(&mut rx, id, &sdrkit_core::Value::Choice(4)).is_err());
    assert!(sdrkit_core::owner::apply(&mut rx, id, &sdrkit_core::Value::Choice(2)).is_ok());
}
