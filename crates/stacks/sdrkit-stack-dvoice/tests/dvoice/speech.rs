//! The speech decoders of the stacks: a DMR signal and a P25 Phase 1 signal whose voice frames come from the encoder of
//! a vowel decode, through the whole graph of the stack and its voice monitor, to sound with the pitch of the vowel.

use crate::common::*;
use sdrkit_codec_mbe::{Coder, Encoder, FRAME};
use sdrkit_core::codec::CodecSet;
use sdrkit_core::Complex32;
use sdrkit_stack_dvoice::dmr::{DmrGen, DmrTx};
use sdrkit_stack_dvoice::gen::Prepared;

/// The rate of the sessions of these tests.
const RATE: f64 = 96_000.0;
/// The pitch of the vowel, Hz.
pub const VOWEL_F0: f64 = 120.0;

/// Sample `n` at 8 kHz of a vowel of pitch [`VOWEL_F0`]: its harmonics shaped by formants near 700 Hz and 1200 Hz.
pub fn vowel(n: usize) -> i16 {
    let t = n as f64 / 8000.0;
    let formant = |f: f64, c: f64, w: f64| 1.0 / (1.0 + ((f - c) / w).powi(2));
    let v: f64 = (1..30)
        .map(|k| {
            let f = VOWEL_F0 * k as f64;
            let a = formant(f, 700.0, 150.0) + 0.6 * formant(f, 1200.0, 200.0) + 0.05;
            a * (std::f64::consts::TAU * f * t).sin()
        })
        .sum();
    (2500.0 * v).round().clamp(-32767.0, 32767.0) as i16
}

/// `frames` frames of the vowel through the encoder of `coder`: the channel bits of each, as sent.
pub fn vowel_frames<const N: usize>(coder: Coder, frames: usize) -> Vec<[u8; N]> {
    let mut enc = Encoder::new(coder);
    (0..frames)
        .map(|k| {
            let pcm: [i16; FRAME] = std::array::from_fn(|i| vowel(k * FRAME + i));
            let mut bits = [0u8; N];
            assert_eq!(enc.encode(&pcm, &mut bits), N);
            bits
        })
        .collect()
}

/// The pitch of `x` at `rate`, Hz: the shortest lag between 2.5 ms and 16 ms whose normalised autocorrelation is within
/// 10 % of the best.
pub fn pitch_hz(x: &[f32], rate: f64) -> f64 {
    let lo = (rate / 400.0) as usize;
    let hi = (rate / 62.5) as usize;
    let n = x.len() - hi;
    let r = |lag: usize| -> f64 {
        let (mut xy, mut xx, mut yy) = (0.0f64, 0.0f64, 0.0f64);
        for i in 0..n {
            let (a, b) = (f64::from(x[i]), f64::from(x[i + lag]));
            xy += a * b;
            xx += a * a;
            yy += b * b;
        }
        xy / (xx * yy).sqrt().max(1e-12)
    };
    let rs: Vec<f64> = (lo..=hi).map(r).collect();
    let best = rs.iter().copied().fold(f64::MIN, f64::max);
    let k = rs.iter().position(|&v| v >= 0.9 * best).unwrap();
    rate / (lo + k) as f64
}

/// The codec set with the speech decoders of the half-rate and full-rate coders.
pub fn speech_codecs() -> CodecSet {
    let mut set = CodecSet::default();
    sdrkit_stack_dvoice::mbe::register(&mut set);
    set
}

/// The audio of the stack `id` for the 48 kHz signal `s48`: its samples and their rate.
fn sound(id: &str, s48: Vec<Complex32>) -> (Vec<f32>, f64) {
    let mut g = Prepared::new(s48, RATE, false);
    let x = generate_all(&mut g);
    let (_, out) = run(
        receiver_with_codecs(id, &[], speech_codecs()).as_mut(),
        RATE,
        &x,
        4800,
    );
    let mut audio = Vec::new();
    let mut rate = 0.0;
    for p in out.audio_runs() {
        audio.extend_from_slice(p.samples);
        rate = p.rate_hz;
    }
    (audio, rate)
}

/// The loudest second of `x` at `rate`.
fn loudest_second(x: &[f32], rate: f64) -> &[f32] {
    let n = rate as usize;
    assert!(x.len() > n, "{} samples of sound", x.len());
    let step = n / 10;
    let energy = |k: usize| x[k..k + n].iter().map(|v| v * v).sum::<f32>();
    let k = (0..(x.len() - n) / step)
        .map(|i| i * step)
        .max_by(|&a, &b| energy(a).total_cmp(&energy(b)))
        .unwrap();
    &x[k..k + n]
}

#[test]
fn a_dmr_call_of_coded_vowel_frames_sounds_with_the_pitch_of_the_vowel() {
    let g = DmrGen {
        cc: 1,
        schedule: vec![(
            0.1,
            DmrTx::Voice {
                slot: 0,
                src: 3_101_234,
                dst: 91,
                group: true,
                superframes: 6,
                alias: String::new(),
                emergency: false,
                privacy: false,
            },
        )],
        speech: vowel_frames::<72>(Coder::HalfRate, 108),
    };
    let (audio, rate) = sound("dmr", g.render(1));
    let x = loudest_second(&audio, rate);
    let f0 = pitch_hz(x, rate);
    println!(
        "DMR: {} s of sound, pitch {f0:.1} Hz",
        audio.len() as f64 / rate
    );
    assert!((f0 - VOWEL_F0).abs() < 0.04 * VOWEL_F0, "pitch {f0} Hz");
}

#[test]
fn a_p25_call_of_coded_vowel_frames_sounds_with_the_pitch_of_the_vowel() {
    let frames = vowel_frames::<144>(Coder::FullRate, 120);
    let (audio, rate) = sound(
        "p25",
        sdrkit_stack_dvoice::p25::render_with_speech(0x293, 1, &frames),
    );
    let x = loudest_second(&audio, rate);
    let f0 = pitch_hz(x, rate);
    println!(
        "P25: {} s of sound, pitch {f0:.1} Hz",
        audio.len() as f64 / rate
    );
    assert!((f0 - VOWEL_F0).abs() < 0.04 * VOWEL_F0, "pitch {f0} Hz");
}

/// The audio of the stack `id` for its own test signal with the default settings: its samples and their rate.
fn test_signal_sound(id: &str) -> (Vec<f32>, f64) {
    let mut g = generator(id, RATE, 1, &[]);
    let x = generate_all(g.as_mut());
    let (_, out) = run(
        receiver_with_codecs(id, &[], speech_codecs()).as_mut(),
        RATE,
        &x,
        4800,
    );
    let mut audio = Vec::new();
    let mut rate = 0.0;
    for p in out.audio_runs() {
        audio.extend_from_slice(p.samples);
        rate = p.rate_hz;
    }
    (audio, rate)
}

/// The pitch of each loud 160 ms window of `x` at `rate`, a window every 80 ms. A window is loud when its RMS is at least
/// a third of the loudest window's.
fn pitch_track(x: &[f32], rate: f64) -> Vec<f64> {
    let n = (0.16 * rate) as usize;
    let hop = n / 2;
    let rms = |w: &[f32]| (w.iter().map(|v| v * v).sum::<f32>() / w.len() as f32).sqrt();
    let starts: Vec<usize> = (0..x.len().saturating_sub(n) / hop).map(|i| i * hop).collect();
    let loudest = starts.iter().map(|&k| rms(&x[k..k + n])).fold(0.0f32, f32::max);
    starts
        .iter()
        .filter(|&&k| rms(&x[k..k + n]) >= loudest / 3.0)
        .map(|&k| pitch_hz(&x[k..k + n], rate))
        .collect()
}

#[test]
fn the_test_signals_of_the_speech_stacks_sound_with_both_pitches_of_the_test_speech() {
    use sdrkit_stack_dvoice::voice::TEST_SPEECH_F0;
    for id in ["dmr", "nxdn", "dpmr", "p25", "p25p2"] {
        let (audio, rate) = test_signal_sound(id);
        assert!(rate > 0.0, "{id}: no sound");
        let track = pitch_track(&audio, rate);
        let near = |f0: f64| track.iter().filter(|&&f| (f - f0).abs() < 0.05 * f0).count();
        let (lo, hi) = (near(TEST_SPEECH_F0[0]), near(TEST_SPEECH_F0[1]));
        println!(
            "{id}: {:.1} s of sound, {} loud windows, {lo} at {} Hz, {hi} at {} Hz",
            audio.len() as f64 / rate,
            track.len(),
            TEST_SPEECH_F0[0],
            TEST_SPEECH_F0[1]
        );
        assert!(lo >= 3 && hi >= 3, "{id}: pitches {track:?}");
        assert!(
            (lo + hi) as f64 >= 0.7 * track.len() as f64,
            "{id}: pitches {track:?}"
        );
    }
}
