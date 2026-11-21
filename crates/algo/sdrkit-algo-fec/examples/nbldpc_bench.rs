//! Times the non-binary LDPC decoder: `cargo run --release -p sdrkit-algo-fec --example nbldpc_bench`. Each line is the best of
//! several runs.

use sdrkit_algo_fec::nbldpc::{NbCode, NbDecoder, RepeatAccumulate};
use std::hint::black_box;
use std::time::Instant;

fn best(mut step: impl FnMut()) -> f64 {
    step();
    let start = Instant::now();
    let mut b = f64::INFINITY;
    let mut runs = 0;
    while runs < 5 || start.elapsed().as_secs_f64() < 0.5 {
        let t = Instant::now();
        step();
        b = b.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    b
}

/// SplitMix64.
fn next(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn gauss(s: &mut u64) -> f64 {
    let u = ((next(s) >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let v = ((next(s) >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
}

/// Likelihood rows from coherent orthogonal signalling with amplitude `amp` and unit noise.
fn rows(word: &[u16], q: usize, amp: f64, s: &mut u64) -> Vec<f32> {
    let mut out = Vec::with_capacity(word.len() * q);
    for &x in word {
        let r: Vec<f64> = (0..q)
            .map(|a| amp * ((a == x as usize) as u8 as f64) + gauss(s))
            .collect();
        let top = r.iter().cloned().fold(f64::MIN, f64::max);
        out.extend(r.iter().map(|&v| (amp * (v - top)).exp() as f32));
    }
    out
}

fn main() {
    let mut s = 2016u64;
    // An (n = 65, k = 15) repeat-accumulate code over GF(64): five symbols repeated 4 times, ten repeated 3 times.
    let mut repeats = vec![4usize; 5];
    repeats.extend([3; 10]);
    let e: usize = repeats.iter().sum();
    let mut permutation: Vec<usize> = (0..e).collect();
    for i in (1..e).rev() {
        let j = (next(&mut s) % (i as u64 + 1)) as usize;
        permutation.swap(i, j);
    }
    let weights: Vec<u16> = (0..e).map(|_| 1 + (next(&mut s) % 63) as u16).collect();
    let code = NbCode::repeat_accumulate(
        6,
        0x43,
        &RepeatAccumulate {
            repeats: &repeats,
            permutation: &permutation,
            weights: &weights,
            combine: 1,
        },
    )
    .unwrap();
    let mut dec = NbDecoder::new(&code);
    let info: Vec<u16> = (0..15).map(|_| (next(&mut s) % 64) as u16).collect();
    let mut word = vec![0u16; 65];
    code.encode(&info, &mut word).unwrap();
    let mut hard = vec![0u16; 65];

    // Pure noise: take the first draw that satisfies no code word within 50 iterations, so every run takes all 50.
    let noise = loop {
        let noise = rows(&word, 64, 0.0, &mut s);
        if dec.decode(&noise, 50, &mut hard).is_err() {
            break noise;
        }
    };
    let t = best(|| {
        let _ = black_box(dec.decode(black_box(&noise), 50, &mut hard));
    });
    println!(
        "GF(64) (65, 15), 50 iterations:        {:8.1} µs per decode, {:6.2} µs per iteration",
        t * 1e6,
        t * 1e6 / 50.0
    );

    let noisy = rows(&word, 64, 3.0, &mut s);
    let r = dec.decode(&noisy, 50, &mut hard).unwrap();
    let t = best(|| {
        let _ = black_box(dec.decode(black_box(&noisy), 50, &mut hard));
    });
    println!(
        "GF(64) (65, 15), amplitude 3.0:        {:8.1} µs per decode, {} iterations",
        t * 1e6,
        r.iterations
    );

    let t = best(|| {
        code.encode(black_box(&info), &mut word).unwrap();
    });
    println!("GF(64) (65, 15) encode:                {:8.3} µs", t * 1e6);
}
