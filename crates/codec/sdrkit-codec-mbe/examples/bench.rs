//! Times the encoder and the decoder of each coder: `cargo run --release -p sdrkit-codec-mbe --example bench`.

use sdrkit_codec_mbe::{Coder, Decoder, Encoder, FRAME};
use std::time::Instant;

fn main() {
    // 8 s of a 140 Hz vowel-like signal with noise.
    let mut x = 1u32;
    let speech: Vec<i16> = (0..64_000)
        .map(|n| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let t = n as f64 / 8000.0;
            let v = 4000.0 * (std::f64::consts::TAU * 140.0 * t).sin()
                + 2000.0 * (std::f64::consts::TAU * 700.0 * t).sin()
                + ((x >> 16) % 400) as f64
                - 200.0;
            v as i16
        })
        .collect();
    for coder in Coder::ALL {
        let mut enc = Encoder::new(coder);
        let mut dec = Decoder::new(coder);
        let bits = coder.frame_bits();
        let frames = speech.len() / FRAME;
        let mut chan = vec![0u8; frames * bits];
        let mut out = [0i16; FRAME];
        // The fastest of several passes, which a busy machine disturbs least.
        let passes = 200;
        let (mut te, mut td) = (f64::INFINITY, f64::INFINITY);
        for _ in 0..passes {
            let t0 = Instant::now();
            for (pcm, f) in speech.as_chunks::<FRAME>().0.iter().zip(chan.chunks_exact_mut(bits)) {
                enc.encode(pcm, f);
            }
            te = te.min(t0.elapsed().as_secs_f64() / frames as f64);
            let t0 = Instant::now();
            for f in chan.chunks_exact(bits) {
                dec.decode(f, &mut out);
            }
            td = td.min(t0.elapsed().as_secs_f64() / frames as f64);
        }
        println!(
            "{coder:?}: encode {:.1} us/frame, decode {:.1} us/frame (frame = 20 ms)",
            te * 1e6,
            td * 1e6
        );
    }
}
