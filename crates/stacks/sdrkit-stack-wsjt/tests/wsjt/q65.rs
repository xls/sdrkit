//! Q65: the worked examples and the encoder vectors symbol for symbol, the tone frame, and the decoder on test signals:
//! levels, sub-modes, periods, drift, noise alone and decoding time, and the stack on its test signal and on a weak signal in
//! each of its sub-modes.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Complex32, GenConfig, LockState, Receiver, StackConfig};
use sdrkit_stack_wsjt::encode::{self, channel_symbols};
use sdrkit_stack_wsjt::proto::msg77::pack77;
use sdrkit_stack_wsjt::proto::q65code;
use sdrkit_stack_wsjt::runner::{decode_slot, DecodeState};
use sdrkit_stack_wsjt::rx::slot::Decode;
use sdrkit_stack_wsjt::{Mode, Sub};

/// A worked example: the message, its 77 bits, the 13 message symbols, the two CRC symbols, the 50 check symbols and the 85
/// tones.
struct Example {
    text: &'static str,
    bits: &'static str,
    info: [u8; 13],
    crc: [u8; 2],
    checks: [u8; 50],
    tones: [u8; 85],
}

#[rustfmt::skip]
const EXAMPLES: [Example; 3] = [
    Example {
        text: "CQ K1ABC FN42",
        bits: "00000000000000000000000000100000010011011110111100011010100010100001100110001",
        info: [0, 0, 0, 0, 8, 4, 55, 47, 6, 40, 40, 25, 34],
        crc: [51, 31],
        checks: [
            51, 51, 51, 59, 7, 38, 14, 50, 62, 60, 60, 19, 19, 0, 3, 29, 1, 5, 5, 45, 12, 8, 8, 39, 5,
            55, 14, 14, 14, 23, 23, 3, 28, 26, 45, 49, 59, 36, 19, 19, 49, 49, 28, 38, 34, 1, 7, 58, 58, 9
        ],
        tones: [
            0, 1, 1, 1, 1, 9, 5, 56, 0, 48, 7, 0, 0, 41, 0, 41, 26, 35, 52, 52, 52, 0,
            0, 60, 8, 0, 0, 39, 15, 51, 63, 61, 0, 61, 0, 20, 20, 0, 1, 4, 30, 2, 6, 6,
            46, 0, 13, 9, 9, 0, 40, 6, 56, 15, 0, 15, 15, 24, 24, 0, 4, 0, 29, 27, 46, 0,
            50, 60, 0, 37, 20, 20, 50, 0, 50, 0, 29, 39, 35, 2, 8, 59, 59, 10, 0
        ],
    },
    Example {
        text: "K1ABC W9XYZ -12",
        bits: "00001001101111011110001101010000011000010100100111011100000111111010100111001",
        info: [2, 27, 55, 35, 20, 6, 5, 9, 55, 1, 62, 41, 50],
        crc: [52, 49],
        checks: [
            52, 33, 2, 22, 9, 53, 52, 22, 34, 33, 18, 27, 21, 43, 17, 1, 27, 63, 41, 23, 0, 6, 7, 62, 12,
            50, 32, 17, 10, 35, 20, 53, 4, 1, 4, 9, 41, 61, 6, 48, 0, 2, 43, 1, 63, 44, 27, 55, 2, 35
        ],
        tones: [
            0, 3, 28, 56, 36, 21, 7, 6, 0, 10, 56, 0, 0, 2, 0, 63, 42, 51, 53, 34, 3, 0,
            0, 23, 10, 0, 0, 54, 53, 23, 35, 34, 0, 19, 0, 28, 22, 0, 44, 18, 2, 28, 64, 42,
            24, 0, 1, 7, 8, 0, 63, 13, 51, 33, 0, 18, 11, 36, 21, 0, 54, 0, 5, 2, 5, 0,
            10, 42, 0, 62, 7, 49, 1, 0, 3, 0, 44, 2, 64, 45, 28, 56, 3, 36, 0
        ],
    },
    Example {
        text: "W9XYZ K1ABC RRR",
        bits: "00001100001010010011101110000000010011011110111100011010100111111010010010001",
        info: [3, 2, 36, 59, 32, 4, 55, 47, 6, 41, 62, 36, 34],
        crc: [52, 41],
        checks: [
            52, 28, 39, 7, 59, 26, 51, 7, 51, 49, 58, 21, 63, 31, 28, 18, 14, 10, 56, 6, 39, 35, 52, 13, 47,
            34, 27, 19, 17, 53, 17, 36, 13, 11, 60, 49, 59, 36, 45, 30, 12, 15, 38, 11, 15, 18, 20, 25, 16, 35
        ],
        tones: [
            0, 4, 3, 37, 60, 33, 5, 56, 0, 48, 7, 0, 0, 42, 0, 63, 37, 35, 53, 29, 40, 0,
            0, 8, 60, 0, 0, 27, 52, 8, 52, 50, 0, 59, 0, 22, 64, 0, 32, 29, 19, 15, 11, 57,
            7, 0, 40, 36, 53, 0, 14, 48, 35, 28, 0, 20, 18, 54, 18, 0, 37, 0, 14, 12, 61, 0,
            50, 60, 0, 37, 46, 31, 13, 0, 16, 0, 39, 12, 16, 19, 21, 26, 17, 36, 0
        ],
    },
];

fn q65(period: f64, letter: u8) -> Sub {
    Sub::new(Mode::Q65).with_period(period).with_sub(letter)
}

/// One period of `sub` with the stations and noise, decoded: the decodes and the decoding time in ms.
fn one_period(sub: Sub, st: &[St], seed: u64) -> (Vec<Decode>, f64) {
    let a = audio_of(sub, st, sub.period, true, seed);
    let mut state = DecodeState::default();
    let slot = &slots(sub, &a)[0];
    let r = decode_slot(slot, &mut state);
    (r.decodes, r.ms)
}

/// How many of `seeds` periods of `sub` decode `text` sent at `snr` dB, the frequency and the start moved a little with each
/// seed. A decode of another text fails the test.
fn decoded(sub: Sub, text: &str, snr: f64, seeds: std::ops::Range<u64>) -> usize {
    let mut ok = 0;
    for seed in seeds {
        let hz = 1000.0 + 37.3 * (seed % 8) as f64;
        let dt = 0.1 * (seed % 4) as f64;
        let (d, _) = one_period(sub, &[(text, hz, snr, dt, false)], 100 + seed);
        assert!(d.iter().all(|x| x.text == text), "{}: false decode {d:?}", sub.label());
        ok += d.len().min(1);
    }
    ok
}

/// The worked examples: the bits, the message and CRC symbols, the check symbols, the channel symbols and the tones.
#[test]
fn the_worked_examples() {
    for x in &EXAMPLES {
        let bits = pack77(x.text).unwrap();
        let want: Vec<u8> = x.bits.bytes().map(|b| b - b'0').collect();
        assert_eq!(bits, want, "{}", x.text);
        let info = encode::q65_info(&bits);
        assert_eq!(info[..13], x.info, "{}", x.text);
        assert_eq!(info[13..], x.crc, "{}", x.text);
        assert_eq!(encode::q65_crc(&x.info), x.crc, "{}", x.text);
        assert_eq!(info[12] & 1, 0, "the pad bit of {}", x.text);
        assert_eq!(q65code::encode(&info)[15..], x.checks, "{}", x.text);
        let channel = encode::q65_channel(&info);
        assert_eq!(channel[..13], x.info, "{}", x.text);
        assert_eq!(channel[13..], x.checks, "{}", x.text);
        assert_eq!(encode::q65_tones(&channel), x.tones, "{}", x.text);
        assert_eq!(channel_symbols(&Sub::new(Mode::Q65), x.text, &Default::default()).unwrap(), x.tones);
    }
    // The first example's CRC as a number, highest power first: 1100 1111 1110.
    let [a, b] = encode::q65_crc(&EXAMPLES[0].info);
    let bits = (0..6).map(|i| a >> i & 1).chain((0..6).map(|i| b >> i & 1));
    assert_eq!(bits.fold(0u16, |v, x| v << 1 | u16::from(x)), 0xCFE);
}

/// The frame of `CQ K1ABC FN42`: 85 tones from 0 to 64, tone 0 exactly at the 22 sync positions, among them the first, the
/// ninth and the last.
#[test]
fn the_tone_frame() {
    let t = channel_symbols(&Sub::new(Mode::Q65), "CQ K1ABC FN42", &Default::default()).unwrap();
    assert_eq!(t.len(), 85);
    assert!(t.iter().all(|&x| x <= 64));
    assert_eq!(t.iter().filter(|&&x| x == 0).count(), 22);
    assert_eq!((t[0], t[8], t[84]), (0, 0, 0));
    for (i, &x) in t.iter().enumerate() {
        assert_eq!(x == 0, encode::Q65_SYNC.contains(&i), "position {i}");
    }
}

/// Every Q65 frame of the encoder vectors, made by an independent encoder, tone for tone.
#[test]
fn the_encoder_vectors_tone_for_tone() {
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
        assert_eq!(channel_symbols(&Sub::new(Mode::Q65), msg, &Default::default()).unwrap(), want, "{msg}");
        n += 1;
    }
    assert_eq!(n, 3);
}

/// `seconds` of the stack's test signal at 12 kHz with these settings and the sub-mode named `submode`, in noise that puts a
/// signal of the generator's full amplitude at `snr` dB, received by the stack with the same settings and sub-mode: the
/// decode events and the stack.
fn through_the_stack(
    extra: &[(&str, sdrkit_core::Value)],
    submode: Option<&str>,
    seconds: f64,
    snr: f64,
    seed: u64,
) -> (Vec<sdrkit_core::Event>, sdrkit_graph::StackGraph) {
    let settings = settings(Some(Mode::Q65), extra);
    let gen = GenConfig::new(12000.0, settings.clone()).with_seed(seed).with_submode_opt(submode.map(str::to_owned));
    let mut g = sdrkit_stack_wsjt::generator::WsjtGenerator::from_config(Mode::Q65, &gen);
    let mut iq = vec![Complex32::default(); (12000.0 * seconds) as usize];
    assert_eq!(g.generate(&mut iq), iq.len());
    add_noise(&mut iq, snr, 12000.0, seed);
    let cfg = StackConfig::new(settings).with_submode_opt(submode.map(str::to_owned));
    let mut rx = sdrkit_stack_wsjt::receiver::receiver(Some(Mode::Q65), &cfg).unwrap();
    rx.configure(&sdrkit_core::InputInfo::new(12000.0, 50.275e6, 50.275e6).recording())
        .unwrap();
    (feed(&mut rx, &iq, 12000), rx)
}

/// The test signal of the generator (four stations, 0 to -8 dB) through the stack in its default settings: every station
/// decoded at its frequency, start and level, and the lamp on.
#[test]
fn the_stack_locks_on_its_test_signal_with_default_settings() {
    let (ev, rx) = through_the_stack(&[], None, 62.0, 0.0, 1);
    let got = texts(&ev);
    let st = sdrkit_stack_wsjt::generator::stations(Mode::Q65);
    assert_eq!(got.len(), st.len(), "{got:?}");
    for &(text, hz, snr, dt) in st {
        let e = ev
            .iter()
            .find(|e| e.is(&sdrkit_stack_wsjt::typed::DECODE) && e.get(&sdrkit_core::publish::keys::TEXT) == Some(text))
            .unwrap_or_else(|| panic!("{text} missing: {got:?}"));
        assert!((num(e, "audio_hz") - hz).abs() < 1.0, "{text} at {} Hz", num(e, "audio_hz"));
        assert!((num(e, "dt_s") - dt).abs() < 0.15, "{text} at {} s", num(e, "dt_s"));
        assert!((num(e, "snr_db") - snr).abs() <= 3.5, "{text} at {} dB", num(e, "snr_db"));
    }
    let status = rx.results().status();
    assert_eq!(status.lock_state, LockState::Locked, "{}", status.summary);
    assert!(status.summary.contains("no public document"), "{}", status.summary);
}

/// Each sub-mode of the stack (60A, 30A, 120E): the stack's test signal with one station at -18 dB, sent and received with
/// the sub-mode named.
#[test]
fn each_sub_mode_of_the_stack_sends_and_receives_at_a_weak_level() {
    use sdrkit_core::Value;
    let d = sdrkit_stack_wsjt::descriptors().into_iter().find(|d| d.id == "q65").unwrap();
    assert_eq!(d.submodes.len(), 3);
    let text = "K1ABC W9XYZ -15";
    let extra = [("gen_single", Value::Text(text.into())), ("gen_hz", Value::Hz(1200.0))];
    for (i, s) in d.submodes.iter().enumerate() {
        let sub = sdrkit_stack_wsjt::settings::sub_of(Mode::Q65, &settings(Some(Mode::Q65), &[]), Some(s.id));
        assert_eq!(sub.label(), s.name);
        let (ev, _) = through_the_stack(&extra, Some(s.id), sub.period + 2.0, -18.0, 40 + i as u64);
        assert_eq!(texts(&ev), [text], "{}", s.name);
    }
}

/// The verification case of the stack on its test signal, through the test harness: the case's own checks pass and the
/// lamp comes on.
#[test]
fn the_verification_case_passes() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    let d = sdrkit_stack_wsjt::descriptors().into_iter().find(|d| d.id == "q65").unwrap();
    let cases = (d.verify.unwrap().cases)(&o);
    assert_eq!(cases.len(), 1);
    let c = &cases[0];
    let run = sdrkit_dev_test::Harness::new(d)
        .with_settings(&c.settings)
        .seconds(c.seconds)
        .run()
        .unwrap();
    for l in (c.check.unwrap())(&run.results) {
        assert_eq!(l.outcome, sdrkit_core::verify::Outcome::Pass, "{}: {}", l.key, l.detail);
    }
    assert_eq!(run.results.status().lock_state, LockState::Locked, "{}", run.results.status().summary);
}

/// -10 dB at 1500 Hz in the A sub-mode of every period: decoded, the start within 0.15 s, the frequency within 1 Hz, the
/// SNR within 3 dB. The decoding time of each period (of this test build, with the other tests running) goes to the test
/// output and must stay under half the period.
#[test]
fn every_period_decodes_and_in_time() {
    for period in [15.0, 30.0, 60.0, 120.0, 300.0] {
        let sub = q65(period, 0);
        let (d, ms) = one_period(sub, &[("CQ K1ABC FN42", 1500.0, -10.0, 0.0, false)], 7);
        println!("{}: one period decoded in {ms:.0} ms", sub.label());
        assert_eq!(d.len(), 1, "{}: {d:?}", sub.label());
        assert_eq!(d[0].text, "CQ K1ABC FN42");
        assert!(d[0].dt.abs() < 0.15, "{}: dt {}", sub.label(), d[0].dt);
        assert!((d[0].hz - 1500.0).abs() < 1.0, "{}: {} Hz", sub.label(), d[0].hz);
        assert!((d[0].snr + 10).abs() <= 3, "{}: {} dB", sub.label(), d[0].snr);
        assert!(ms < 500.0 * period, "{}: {ms:.0} ms", sub.label());
        // Noise alone: every candidate runs every iteration.
        let (d, ms) = one_period(sub, &[], 8);
        println!("{}: one period of noise decoded in {ms:.0} ms", sub.label());
        assert!(d.is_empty(), "{}: {d:?}", sub.label());
        assert!(ms < 500.0 * period, "{}: {ms:.0} ms", sub.label());
    }
}

/// The wider sub-modes at -12 dB, 900 Hz, 0.3 s late.
#[test]
fn the_wider_sub_modes_decode() {
    for sub in [q65(60.0, 1), q65(60.0, 2), q65(60.0, 3), q65(60.0, 4), q65(30.0, 1), q65(15.0, 2)] {
        let (d, _) = one_period(sub, &[("K1ABC W9XYZ -15", 900.0, -12.0, 0.3, false)], 11);
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
    for (sub, sure, edge) in [(q65(60.0, 0), -24.0, -27.0), (q65(30.0, 1), -21.0, -24.0)] {
        let a = decoded(sub, "CQ K1ABC FN42", sure, 0..10);
        let b = decoded(sub, "CQ K1ABC FN42", edge, 0..10);
        println!("{}: {a} of 10 at {sure} dB, {b} of 10 at {edge} dB", sub.label());
        assert_eq!(a, 10, "{} at {sure} dB", sub.label());
        assert!(b >= 4, "{}: {b} of 10 at {edge} dB", sub.label());
    }
}

/// Three stations in one period, two of them 40 Hz apart.
#[test]
fn three_stations_decode() {
    let st: [St; 3] = [
        ("CQ K1ABC FN42", 1000.0, -18.0, 0.0, false),
        ("K1ABC W9XYZ -12", 1040.0, -18.0, 0.5, false),
        ("CQ DX G4ABC IO91", 2300.0, -20.0, -0.7, false),
    ];
    let (d, _) = one_period(Sub::new(Mode::Q65), &st, 5);
    for s in &st {
        assert!(d.iter().any(|x| x.text == s.0), "{} missing: {d:?}", s.0);
    }
    assert_eq!(d.len(), 3, "{d:?}");
}

/// A signal at -22 dB whose frequency moves by 0.15 Hz/s up and down (4.6 symbol rates over the transmission) and by
/// 0.2 Hz/s (6.1 symbol rates).
#[test]
fn drifting_signals_decode() {
    let sub = Sub::new(Mode::Q65);
    let text = "W9XYZ K1ABC EN37";
    for (i, hz_per_s) in [0.15, -0.15, 0.2].into_iter().enumerate() {
        let t = encode::Transmission {
            sub,
            hz: 1500.0,
            amp: amp(-22.0),
            mirrored: false,
        };
        let tones = channel_symbols(&sub, text, &Default::default()).unwrap();
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
        let mut state = DecodeState::default();
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
    let mut state = DecodeState::default();
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
