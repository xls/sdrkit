//! MSK144 short messages: the 40-bit frame from the encoder and the test signal, the decode of a continuous
//! transmission and of meteor bursts for the operator's call and the other station's call, and nothing for other
//! calls, for the reports below R-03, or from noise.

use crate::common::*;
use sdrkit_stack_wsjt::encode::{audio_of as tones_audio, msk144_symbols, Transmission};
use sdrkit_stack_wsjt::proto::msk144code::short_frame;
use sdrkit_stack_wsjt::runner::DecodeState;
use sdrkit_stack_wsjt::{Mode, Sub};

const TEXT: &str = "<K1ABC W9XYZ> RRR";

/// A decoder state for the operator K1ABC that heard W9XYZ.
fn state() -> DecodeState {
    let mut s = DecodeState::default();
    s.short.set_own("K1ABC");
    s.short.heard("K1ABC W9XYZ R-05");
    s
}

/// `seconds` of 8 kHz audio of the short frames of `text` at `hz` and `snr`, from `at_s` for `len_s` seconds.
fn short_audio(
    text: &str,
    hz: f64,
    snr: f64,
    at_s: f64,
    len_s: f64,
    seconds: f64,
    seed: u64,
) -> Vec<f32> {
    let sub = Sub::new(Mode::Msk144);
    let mut a = noise((seconds * 8000.0) as usize, SIGMA, seed);
    let mut w = Vec::new();
    let t = Transmission {
        sub,
        hz,
        amp: amp(snr),
        mirrored: false,
    };
    tones_audio(
        &t,
        &msk144_symbols(text, true).unwrap(),
        8000.0,
        len_s,
        &mut w,
    );
    let st = (at_s * 8000.0) as usize;
    for (o, v) in a[st..].iter_mut().zip(&w) {
        *o += v;
    }
    a
}

fn texts(sub: Sub, a: &[f32], st: DecodeState) -> Vec<String> {
    decode_with(sub, a, st)
        .into_iter()
        .map(|d| d.text)
        .collect()
}

#[test]
fn the_encoder_sends_the_short_frame_when_asked() {
    let f = msk144_symbols(TEXT, true).unwrap();
    assert_eq!(f, short_frame(0xFA2E).to_vec());
    // Off, the text goes as a 77-bit message or not at all.
    assert!(msk144_symbols(TEXT, false).map_or(true, |s| s.len() == 144));
    // A report outside the 16 short reports is no short message.
    assert_eq!(msk144_symbols("CQ K1ABC FN42", true).unwrap().len(), 144);
    // The frame repeats to fill the period: 20 ms per frame.
    let sub = Sub::new(Mode::Msk144);
    let t = Transmission {
        sub,
        hz: 1500.0,
        amp: 0.5,
        mirrored: false,
    };
    let mut w = Vec::new();
    tones_audio(&t, &f, 8000.0, 14.9, &mut w);
    assert_eq!(w.len(), (14.9f64 * 8000.0).round() as usize);
    assert!(w[w.len() - 100..].iter().any(|v| v.abs() > 0.1));
}

#[test]
fn a_continuous_short_transmission_decodes() {
    let sub = Sub::new(Mode::Msk144);
    for (snr, seed) in [(10.0, 1), (0.0, 2), (-4.0, 3)] {
        let a = short_audio(TEXT, 1500.0, snr, 0.0, 14.9, 15.0, seed);
        let mut st = state();
        let r = sdrkit_stack_wsjt::runner::decode_slot(&slots(sub, &a)[0], &mut st);
        eprintln!("short at {snr} dB: decoded in {:.0} ms", r.ms);
        let got: Vec<&str> = r.decodes.iter().map(|d| d.text.as_str()).collect();
        assert_eq!(got, [TEXT], "{snr} dB");
    }
}

#[test]
fn meteor_bursts_of_short_frames_decode_at_their_time() {
    let sub = Sub::new(Mode::Msk144);
    for (len_s, at_s, hz, seed) in [
        (0.1, 3.0, 1200.0, 11),
        (0.06, 9.7, 1900.0, 12),
        (0.4, 12.0, 1500.0, 13),
    ] {
        let a = short_audio("<K1ABC W9XYZ> R+06", hz, 4.0, at_s, len_s, 15.0, seed);
        let d = decode_with(sub, &a, state());
        assert_eq!(d.len(), 1, "{len_s} s at {at_s} s: {d:?}");
        assert_eq!(d[0].text, "<K1ABC W9XYZ> R+06");
        assert!(
            (d[0].dt - at_s).abs() <= len_s + 0.1,
            "{} against {at_s}",
            d[0].dt
        );
        assert!((d[0].hz - (hz - 500.0)).abs() < 10.0, "{}", d[0].hz);
        assert_eq!(d[0].call, "W9XYZ");
    }
}

#[test]
fn a_short_message_for_other_calls_or_a_low_report_is_not_shown() {
    let sub = Sub::new(Mode::Msk144);
    // Bursts: a 144-bit search of a transmission that never stops takes long.
    let a = short_audio(TEXT, 1500.0, 10.0, 4.0, 0.3, 15.0, 21);
    // No call of the operator, or no other station heard.
    assert!(texts(sub, &a, DecodeState::default()).is_empty());
    let mut s = DecodeState::default();
    s.short.set_own("K1ABC");
    assert!(texts(sub, &a, s).is_empty());
    // Another pair of calls.
    let mut s = DecodeState::default();
    s.short.set_own("G4ABC");
    s.short.heard("G4ABC W9XYZ -10");
    assert!(texts(sub, &a, s).is_empty());
    // The reports below R-03 are not shown.
    let a = short_audio("<K1ABC W9XYZ> +06", 1500.0, 10.0, 4.0, 0.3, 15.0, 22);
    assert!(texts(sub, &a, state()).is_empty());
}

#[test]
fn noise_and_full_frames_give_no_short_message() {
    let sub = Sub::new(Mode::Msk144);
    let mut st = state();
    for k in 0..4 {
        st.short.add_other(&format!("W{k}AB"));
    }
    for seed in 0..16 {
        let a = noise(15 * 8000, SIGMA, 300 + seed);
        let got = texts(sub, &a, st.clone());
        assert!(got.is_empty(), "seed {seed}: {got:?}");
    }
    // A 144-bit frame gives its own text, and no short message.
    let a = audio_of(
        sub,
        &[("K1ABC W9XYZ R-05", 1500.0, -2.0, 0.0, false)],
        15.0,
        true,
        330,
    );
    assert_eq!(texts(sub, &a, st), ["K1ABC W9XYZ R-05"]);
}

#[test]
fn the_test_signal_sends_short_frames_with_the_setting() {
    use sdrkit_core::{Complex32, GenConfig, Value, Values};
    let d = sdrkit_stack_wsjt::descriptors()
        .into_iter()
        .find(|d| d.id == "msk144")
        .unwrap();
    let schema = (d.settings)();
    for short in [false, true] {
        let mut v = Values::defaults(&schema);
        v.set(schema.id("gen_single").unwrap(), &Value::Text(TEXT.into()))
            .unwrap();
        v.set(schema.id("gen_short").unwrap(), &Value::Bool(short))
            .unwrap();
        let mut g = (d.make_generator.unwrap())(&GenConfig::new(8000.0, v)).unwrap();
        let mut iq = vec![Complex32::default(); 15 * 8000];
        g.generate(&mut iq);
        let a: Vec<f32> = iq.iter().map(|z| 0.3 * z.re).collect();
        let got = texts(Sub::new(Mode::Msk144), &a, state());
        if short {
            assert_eq!(got, [TEXT]);
        } else {
            assert!(got.is_empty(), "{got:?}");
        }
    }
}
