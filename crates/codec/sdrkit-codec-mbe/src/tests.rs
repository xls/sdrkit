//! Decoder checks that need the frame internals: error counts at the repeat and mute limits, special half-rate frames.

use crate::fec::{codes, FULL_WORDS, HALF_WORDS};
use crate::model::FRAME;
use crate::{full, half, Coder, Decoder, Encoder, FrameKind};
use std::f64::consts::TAU;

/// A vowel-like signal at `f` Hz, frame `k`.
fn voiced(f: f64, k: usize, out: &mut [i16; FRAME]) {
    for (i, o) in out.iter_mut().enumerate() {
        let t = (k * FRAME + i) as f64 / 8000.0;
        let mut v = 0.0;
        let mut h = 1.0;
        while h * f < 3800.0 {
            let hz = h * f;
            let a = 3000.0 * (-((hz - 500.0) / 250.0f64).powi(2)).exp() + 300.0 * (-hz / 1000.0).exp();
            v += a * (TAU * hz * t).cos();
            h += 1.0;
        }
        *o = v as i16;
    }
}

/// Channel frames of a 120 Hz vowel.
fn frames(coder: Coder, n: usize) -> Vec<Vec<u8>> {
    let mut enc = Encoder::new(coder);
    let mut pcm = [0i16; FRAME];
    (0..n)
        .map(|k| {
            voiced(120.0, k, &mut pcm);
            let mut f = vec![0u8; coder.frame_bits()];
            enc.encode(&pcm, &mut f);
            f
        })
        .collect()
}

/// Flips `count[j]` bits of code word `j` of a channel frame.
fn corrupt(coder: Coder, chan: &mut [u8], count: &[usize]) {
    let (layout, lens): (_, &[usize]) = match coder {
        Coder::FullRate => (&codes().full_layout, &FULL_WORDS),
        Coder::HalfRate => (&codes().half_layout, &HALF_WORDS),
    };
    let mut e = vec![0u8; chan.len()];
    let mut start = 0;
    for (j, &n) in lens.iter().enumerate() {
        for b in 0..count.get(j).copied().unwrap_or(0) {
            e[start + (5 * b + j) % n] = 1;
        }
        start += n;
    }
    let mut ce = vec![0u8; chan.len()];
    layout.join(&e, &mut ce).unwrap();
    for (c, x) in chan.iter_mut().zip(ce) {
        *c ^= x;
    }
}

fn run(dec: &mut Decoder, f: &[u8]) -> crate::FrameStatus {
    let mut out = [0i16; FRAME];
    dec.decode(f, &mut out)
}

#[test]
fn full_rate_repeat_limit() {
    let fr = frames(Coder::FullRate, 20);
    for (errs, want) in [([2, 3, 3, 3], FrameKind::Repeated), ([2, 3, 3, 2], FrameKind::Voice)] {
        let mut dec = Decoder::new(Coder::FullRate);
        for f in &fr[..10] {
            run(&mut dec, f);
        }
        let before = *dec.params();
        let mut f = fr[10].clone();
        corrupt(Coder::FullRate, &mut f, &errs);
        let s = run(&mut dec, &f);
        assert_eq!(s.kind, want, "{errs:?}");
        assert_eq!(s.errors, errs.iter().sum::<usize>() as u32);
        if want == FrameKind::Repeated {
            assert_eq!(*dec.params(), before);
        }
    }
}

#[test]
fn full_rate_mute_limit() {
    let fr = frames(Coder::FullRate, 8);
    // 13 errors a frame drive epsilon_R towards 0.0949 (mute); 10 towards 0.073 (no mute).
    for (errs, mutes) in [([1, 3, 3, 3, 1, 1, 1], true), ([1, 3, 3, 3, 0, 0, 0], false)] {
        let mut dec = Decoder::new(Coder::FullRate);
        let mut muted = false;
        for k in 0..300 {
            let mut f = fr[k % 8].clone();
            corrupt(Coder::FullRate, &mut f, &errs);
            let s = run(&mut dec, &f);
            assert_ne!(s.kind, FrameKind::Repeated);
            muted |= s.kind == FrameKind::Muted;
        }
        assert_eq!(muted, mutes, "{errs:?}, rate {}", dec.error_rate());
    }
}

#[test]
fn half_rate_repeat_and_mute_limits() {
    let fr = frames(Coder::HalfRate, 20);
    for (errs, want) in [
        ([4, 0], FrameKind::Repeated),
        ([3, 3], FrameKind::Repeated),
        ([2, 3], FrameKind::Voice),
        ([1, 3], FrameKind::Voice),
    ] {
        let mut dec = Decoder::new(Coder::HalfRate);
        for f in &fr[..10] {
            run(&mut dec, f);
        }
        let mut f = fr[10].clone();
        corrupt(Coder::HalfRate, &mut f, &errs);
        assert_eq!(run(&mut dec, &f).kind, want, "{errs:?}");
    }
    // The fourth invalid frame in a row is muted.
    let mut dec = Decoder::new(Coder::HalfRate);
    for f in &fr[..10] {
        run(&mut dec, f);
    }
    let kinds: Vec<FrameKind> = (0..5)
        .map(|k| {
            let mut f = fr[10 + k].clone();
            corrupt(Coder::HalfRate, &mut f, &[4]);
            run(&mut dec, &f).kind
        })
        .collect();
    assert_eq!(kinds[..3], [FrameKind::Repeated; 3]);
    assert_eq!(kinds[3..], [FrameKind::Muted; 2]);
    assert_eq!(run(&mut dec, &fr[15]).kind, FrameKind::Voice);
    // 5 errors a frame drive epsilon_R towards 0.1064 (mute); 4 towards 0.0851.
    for (errs, mutes) in [([2, 3], true), ([1, 3], false)] {
        let mut dec = Decoder::new(Coder::HalfRate);
        let mut muted = false;
        for k in 0..300 {
            let mut f = fr[k % 20].clone();
            corrupt(Coder::HalfRate, &mut f, &errs);
            muted |= run(&mut dec, &f).kind == FrameKind::Muted;
        }
        assert_eq!(muted, mutes, "{errs:?}");
    }
}

/// The 49 speech bits of b0..b8.
fn half_bits(b: [u32; 9]) -> [u8; half::SPEECH_BITS] {
    let mut sb = [0u8; half::SPEECH_BITS];
    half::pack(&b, &mut sb);
    sb
}

#[test]
fn half_rate_silence_is_quiet_and_keeps_the_state() {
    let fr = frames(Coder::HalfRate, 12);
    let mut out = [0i16; FRAME];
    // Silence with b2 = 0 (the lowest gain) and all other values 0.
    let sil = half_bits([124, 0, 0, 0, 0, 0, 0, 0, 0]);
    let mut fresh = Decoder::new(Coder::HalfRate);
    let mut e0 = 0.0;
    for _ in 0..6 {
        fresh.decode_speech(&sil, &mut out);
        e0 += out.iter().map(|&x| (x as f64).powi(2)).sum::<f64>();
    }
    let rms0 = (e0 / (6 * FRAME) as f64).sqrt();
    assert!(rms0 < 1.0, "silence RMS from the start state {rms0}");
    let mut dec = Decoder::new(Coder::HalfRate);
    let mut ev = 0.0;
    for f in &fr[..10] {
        dec.decode(f, &mut out);
        ev += out.iter().map(|&x| (x as f64).powi(2)).sum::<f64>();
    }
    let voice_rms = (ev / (10 * FRAME) as f64).sqrt();
    let st = dec.half_amp;
    let mut energy = 0.0;
    for k in 0..6 {
        let s = dec.decode_speech(&sil, &mut out);
        assert_eq!(s.kind, FrameKind::Silence);
        assert_eq!(dec.params().l, 14);
        assert!(dec.params().voiced.iter().all(|&v| !v));
        if k >= 2 {
            energy += out.iter().map(|&x| (x as f64).powi(2)).sum::<f64>();
        }
    }
    let rms = (energy / (4 * FRAME) as f64).sqrt();
    // The silence gain is relative to the last voice frame's gain (Equation 26): 30 dB or more below the voice.
    assert!(rms < 0.03 * voice_rms, "silence RMS {rms}, voice RMS {voice_rms}");
    assert_eq!(st.lambda, dec.half_amp.lambda);
    assert_eq!((st.l, st.gamma), (dec.half_amp.l, dec.half_amp.gamma));
    // The next voice frame decodes as if the silence frames had not been there.
    let mut a = Decoder::new(Coder::HalfRate);
    for f in &fr[..10] {
        run(&mut a, f);
    }
    run(&mut a, &fr[10]);
    run(&mut dec, &fr[10]);
    assert_eq!(a.half_amp.lambda, dec.half_amp.lambda);
}

#[test]
fn half_rate_erasure_repeats() {
    let fr = frames(Coder::HalfRate, 10);
    let mut dec = Decoder::new(Coder::HalfRate);
    for f in &fr {
        run(&mut dec, f);
    }
    let before = *dec.params();
    let mut out = [0i16; FRAME];
    let er = half_bits([120, 0, 31, 511, 127, 31, 15, 15, 7]);
    assert_eq!(dec.decode_speech(&er, &mut out).kind, FrameKind::Repeated);
    assert_eq!(*dec.params(), before);
}

#[test]
fn full_rate_invalid_pitch_repeats() {
    let fr = frames(Coder::FullRate, 10);
    let mut dec = Decoder::new(Coder::FullRate);
    for f in &fr {
        run(&mut dec, f);
    }
    let mut sb = [0u8; full::SPEECH_BITS];
    let mut v = full::Values::default();
    v.b[0] = 230;
    full::pack(&v, 30, &mut sb);
    let mut out = [0i16; FRAME];
    assert_eq!(dec.decode_speech(&sb, &mut out).kind, FrameKind::Repeated);
}

