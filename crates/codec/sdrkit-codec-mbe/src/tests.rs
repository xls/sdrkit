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
        Coder::Dstar => (&codes().dstar_layout, &crate::dstar::WORDS),
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


/// The RMS of one output frame.
fn rms(x: &[i16; FRAME]) -> f64 {
    (x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / FRAME as f64).sqrt()
}

#[test]
fn one_lost_frame_in_a_vowel_repeats_its_sound_and_pitch() {
    for coder in Coder::ALL {
        let fr = frames(coder, 30);
        let mut dec = Decoder::new(coder);
        let mut out = [0i16; FRAME];
        for f in &fr[..20] {
            dec.decode(f, &mut out);
        }
        let voiced = rms(&out);
        let before = *dec.params();
        let s = dec.lost(&mut out);
        assert_eq!(s.kind, FrameKind::Repeated, "{coder:?}");
        assert_eq!(*dec.params(), before, "{coder:?}");
        assert!((dec.params().f0_hz() - 120.0).abs() < 3.0, "{coder:?}: {} Hz", dec.params().f0_hz());
        assert!(rms(&out) > 0.5 * voiced, "{coder:?}: RMS {} after {voiced}", rms(&out));
        assert_eq!(dec.decode(&fr[20], &mut out).kind, FrameKind::Voice, "{coder:?}");
    }
}

#[test]
fn a_run_of_lost_frames_mutes_at_the_fourth() {
    for coder in Coder::ALL {
        let fr = frames(coder, 30);
        let mut dec = Decoder::new(coder);
        let mut out = [0i16; FRAME];
        for f in &fr[..20] {
            dec.decode(f, &mut out);
        }
        let rate = dec.error_rate();
        for k in 0..10 {
            let s = dec.lost(&mut out);
            let want = if k < 3 { FrameKind::Repeated } else { FrameKind::Muted };
            assert_eq!(s.kind, want, "{coder:?}, lost frame {k}");
            if want == FrameKind::Muted && coder == Coder::Dstar {
                // The first muted frame ends the sound; the later ones are zeros.
                assert!(k == 3 || out.iter().all(|&v| v == 0), "{coder:?}: silence");
            } else if want == FrameKind::Muted {
                assert!(out.iter().all(|&v| v.abs() <= 5), "{coder:?}: comfort noise");
            }
        }
        assert_eq!(dec.error_rate(), rate, "{coder:?}: no bits were received");
        assert_eq!(dec.decode(&fr[20], &mut out).kind, FrameKind::Voice, "{coder:?}");
        // A lost frame after three invalid received frames is the fourth invalid frame.
        let mut sb = [0u8; 88];
        if coder == Coder::Dstar {
            // No speech bits of the D-STAR coder are invalid: a word A with four errors is.
            let mut f = fr[21].clone();
            corrupt(coder, &mut f, &[4]);
            for _ in 0..3 {
                assert_eq!(dec.decode(&f, &mut out).kind, FrameKind::Repeated);
            }
            assert_eq!(dec.lost(&mut out).kind, FrameKind::Muted);
            continue;
        }
        if coder == Coder::FullRate {
            let mut v = full::Values::default();
            v.b[0] = 230;
            full::pack(&v, 30, &mut sb);
        } else {
            sb[..49].copy_from_slice(&half_bits([120, 0, 0, 0, 0, 0, 0, 0, 0]));
        }
        for _ in 0..3 {
            let s = dec.decode_speech(&sb[..coder.speech_bits()], &mut out);
            assert_eq!(s.kind, FrameKind::Repeated, "{coder:?}");
        }
        assert_eq!(dec.lost(&mut out).kind, FrameKind::Muted, "{coder:?}");
    }
}

#[test]
fn dstar_repeat_and_mute_limits() {
    let fr = frames(Coder::Dstar, 20);
    // Errors in the data bits of the words A and B: more than three in all, or four in word A, repeat the frame.
    for (errs, want, corrected) in [
        ([4, 0], FrameKind::Repeated, 4),
        ([3, 3], FrameKind::Repeated, 6),
        ([2, 2], FrameKind::Repeated, 4),
        ([3, 0], FrameKind::Voice, 3),
        ([2, 1], FrameKind::Voice, 3),
        ([0, 3], FrameKind::Voice, 3),
    ] {
        let mut dec = Decoder::new(Coder::Dstar);
        let mut clean = Decoder::new(Coder::Dstar);
        for f in &fr[..10] {
            run(&mut dec, f);
            run(&mut clean, f);
        }
        let before = *dec.params();
        let mut f = fr[10].clone();
        corrupt(Coder::Dstar, &mut f, &errs);
        let s = run(&mut dec, &f);
        assert_eq!((s.kind, s.errors), (want, corrected), "{errs:?}");
        run(&mut clean, &fr[10]);
        if want == FrameKind::Repeated {
            assert_eq!(*dec.params(), before, "{errs:?}");
        } else {
            assert_eq!(dec.params(), clean.params(), "{errs:?}: the errors are corrected");
        }
    }
    // The fourth repeat in a row is muted; the next good frame is voice again, from the start state.
    let mut dec = Decoder::new(Coder::Dstar);
    for f in &fr[..10] {
        run(&mut dec, f);
    }
    let kinds: Vec<FrameKind> = (0..5)
        .map(|k| {
            let mut f = fr[10 + k].clone();
            corrupt(Coder::Dstar, &mut f, &[4]);
            run(&mut dec, &f).kind
        })
        .collect();
    assert_eq!(kinds[..3], [FrameKind::Repeated; 3]);
    assert_eq!(kinds[3..], [FrameKind::Muted; 2]);
    assert_eq!(dec.half_amp.gamma, 0.0);
    assert_eq!(run(&mut dec, &fr[15]).kind, FrameKind::Voice);
    assert_eq!(dec.error_rate(), 0.0);
}

#[test]
fn the_dstar_silence_pattern_is_quiet_and_a_tone_frame_is_silence() {
    let fr = frames(Coder::Dstar, 12);
    let mut dec = Decoder::new(Coder::Dstar);
    let mut out = [0i16; FRAME];
    for f in &fr {
        dec.decode(f, &mut out);
    }
    let voiced = rms(&out);
    assert!(voiced > 500.0, "RMS {voiced}");
    // The silence pattern is a voice frame with no error. Its gain is a difference to the frame before it, so the level
    // falls over a few frames, then stays below 8 of 32768.
    let silence = crate::dstar_silence_bits();
    for k in 0..50 {
        let s = dec.decode(&silence, &mut out);
        assert_eq!((s.kind, s.errors), (FrameKind::Voice, 0));
        assert!(k < 3 || rms(&out) < 8.0, "frame {k}: RMS {}", rms(&out));
    }
    assert_eq!((dec.params().l, dec.params().voiced.iter().any(|&v| v)), (56, false));
    // A tone frame gives its index and level, silence, and the start state.
    for f in &fr {
        dec.decode(f, &mut out);
    }
    assert!(dec.half_amp.gamma > 1.0);
    let mut enc = Encoder::new(Coder::Dstar);
    let mut tone = [0u8; 72];
    assert_eq!(enc.encode_tone(32, 200, &mut tone), 72);
    for k in 0..3 {
        let s = dec.decode(&tone, &mut out);
        assert_eq!(s.kind, FrameKind::Tone { id: 32, amplitude: 200 });
        assert!(k == 0 || out.iter().all(|&v| v == 0), "tone frame {k}");
    }
    assert_eq!((dec.half_amp.gamma, dec.half_amp.l), (0.0, 30));
    assert_eq!(dec.decode(&fr[5], &mut out).kind, FrameKind::Voice);
}

#[test]
fn dstar_speech_bits_decode_as_the_channel_bits_do() {
    let mut enc_a = Encoder::new(Coder::Dstar);
    let mut enc_b = Encoder::new(Coder::Dstar);
    let mut a = Decoder::new(Coder::Dstar);
    let mut b = Decoder::new(Coder::Dstar);
    let mut pcm = [0i16; FRAME];
    for k in 0..12 {
        voiced(150.0, k, &mut pcm);
        let (mut chan, mut speech) = ([0u8; 72], [0u8; 48]);
        assert_eq!(enc_a.encode(&pcm, &mut chan), 72);
        assert_eq!(enc_b.encode_speech(&pcm, &mut speech), 48);
        let (mut x, mut y) = ([0i16; FRAME], [0i16; FRAME]);
        assert_eq!(a.decode(&chan, &mut x), b.decode_speech(&speech, &mut y));
        assert_eq!(x, y);
        assert_eq!(enc_a.coded().l, a.params().l);
    }
    assert!((a.params().f0_hz() - 150.0).abs() < 2.0, "{} Hz", a.params().f0_hz());
}
