//! Times the encoder and the decoder of each mode: `cargo run --release -p sdrkit-codec-codec2 --example bench`.
//!
//! The tables come from the folder in the first argument when it holds them. Otherwise the benchmark uses tables of the right
//! shape with made-up values. The time of each stage does not depend on the table values.

use sdrkit_codec_codec2::{Codebooks, Decoder, Encoder, Mode, Table};
use std::time::Instant;

fn books() -> Codebooks {
    if let Some(dir) = std::env::args().nth(1) {
        if let Ok(b) = Codebooks::load_dir(std::path::Path::new(&dir)) {
            if Mode::ALL.iter().all(|&m| b.check(m).is_ok()) {
                return b;
            }
        }
    }
    let mut b = Codebooks::new();
    for t in Table::ALL {
        let v: Vec<f32> = (0..t.len()).map(|i| (i % 97) as f32 * 10.0).collect();
        b.set(t, &v).expect("right shape");
    }
    b
}

fn main() {
    let books = books();
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
    for mode in Mode::ALL {
        let mut enc = Encoder::new(mode, &books).expect("tables");
        let mut dec = Decoder::new(mode, &books).expect("tables");
        let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
        let frames = speech.len() / n;
        let mut bits = vec![0u8; frames * b];
        let mut out = vec![0i16; n];
        let reps = 5;
        let t0 = Instant::now();
        for _ in 0..reps {
            for (s, f) in speech.chunks_exact(n).zip(bits.chunks_exact_mut(b)) {
                enc.encode(s, f).expect("lengths");
            }
        }
        let te = t0.elapsed().as_secs_f64() / (reps * frames) as f64;
        let t0 = Instant::now();
        for _ in 0..reps {
            for f in bits.chunks_exact(b) {
                dec.decode(f, &mut out).expect("lengths");
            }
        }
        let td = t0.elapsed().as_secs_f64() / (reps * frames) as f64;
        let frame_s = n as f64 / 8000.0;
        println!(
            "{:6}  encode {:7.1} us/frame ({:5.2} % of real time)  decode {:7.1} us/frame ({:5.2} % of real time)",
            format!("{mode:?}"),
            te * 1e6,
            100.0 * te / frame_s,
            td * 1e6,
            100.0 * td / frame_s
        );
    }
}
