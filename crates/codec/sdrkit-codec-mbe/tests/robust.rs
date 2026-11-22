//! Random channel and speech bits never panic, and a decoder fed with them still decodes valid frames afterwards.

use sdrkit_codec_mbe::{Coder, Decoder, Encoder, FrameKind, FRAME};
use std::f64::consts::TAU;

fn next(x: &mut u64) -> u64 {
    *x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    *x >> 33
}

#[test]
fn random_frames_never_panic_and_the_decoder_recovers() {
    for coder in Coder::ALL {
        let mut dec = Decoder::new(coder);
        let mut x = 5u64;
        let mut out = [0i16; FRAME];
        let mut chan = vec![0u8; coder.frame_bits()];
        let mut speech = vec![0u8; coder.speech_bits()];
        for _ in 0..5000 {
            for b in chan.iter_mut() {
                *b = next(&mut x) as u8;
            }
            dec.decode(&chan, &mut out);
            for b in speech.iter_mut() {
                *b = next(&mut x) as u8 & 1;
            }
            dec.decode_speech(&speech, &mut out);
        }
        // Wrong lengths are muted.
        assert_eq!(dec.decode(&chan[1..], &mut out).kind, FrameKind::Muted);
        assert_eq!(dec.decode_speech(&[], &mut out).kind, FrameKind::Muted);
        // Valid voiced frames after the noise: the error rate falls, then voice comes out.
        let mut enc = Encoder::new(coder);
        let mut pcm = [0i16; FRAME];
        let mut energy = 0.0;
        let mut last = FrameKind::Muted;
        for k in 0..200 {
            for (i, p) in pcm.iter_mut().enumerate() {
                let t = (k * FRAME + i) as f64 / 8000.0;
                *p = (2000.0 * (TAU * 125.0 * t).cos() + 1500.0 * (TAU * 500.0 * t).cos()) as i16;
            }
            enc.encode(&pcm, &mut chan);
            last = dec.decode(&chan, &mut out).kind;
            if k >= 180 {
                energy += out.iter().map(|&v| (v as f64).powi(2)).sum::<f64>();
            }
        }
        let rms = (energy / (20 * FRAME) as f64).sqrt();
        assert_eq!(last, FrameKind::Voice, "{coder:?}");
        assert!(rms > 300.0, "{coder:?}: RMS {rms} after recovery");
        assert!(dec.params().amp.iter().all(|a| a.is_finite()) && dec.params().w0.is_finite());
    }
}

#[test]
fn the_encoder_takes_any_input() {
    let inputs: [&dyn Fn(usize) -> i16; 7] = [
        &|_| i16::MIN,
        &|_| i16::MAX,
        &|n| if n % 2 == 0 { i16::MAX } else { i16::MIN },
        &|n| if n % 977 == 0 { i16::MAX } else { 0 },
        &|n| (n % 3) as i16 - 1,
        &|n| if (n / 20) % 2 == 0 { i16::MAX } else { i16::MIN },
        &|n| if n < 2000 { 0 } else { ((n * 7919) % 65536) as i32 as i16 },
    ];
    for coder in Coder::ALL {
        for input in inputs {
            let mut enc = Encoder::new(coder);
            let mut dec = Decoder::new(coder);
            let mut chan = vec![0u8; coder.frame_bits()];
            let mut out = [0i16; FRAME];
            for k in 0..40 {
                let pcm: [i16; FRAME] = std::array::from_fn(|i| input(k * FRAME + i));
                assert_eq!(enc.encode(&pcm, &mut chan), coder.frame_bits());
                assert_eq!(dec.decode(&chan, &mut out).errors, 0);
                let p = enc.analysed();
                assert!(p.w0.is_finite() && p.w0 > 0.0 && p.amp.iter().all(|a| a.is_finite()));
            }
        }
    }
}
