//! Times LDPC decoding of the ATSC 3.0 codes and of a synthetic code of the DTMB shape:
//! `cargo run --release -p sdrkit-algo-fec --example ldpc_broadcast_bench`. Each line is the best of several runs.

use sdrkit_algo_fec::ldpc::{InfoAt, LdpcCode, LdpcDecoder, LdpcEncoder, MinSum};
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

fn next(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed >> 33
}

/// Hard bits at magnitude `mag` with Gaussian-like noise of `sigma * mag`.
fn noisy(bits: &[u8], mag: f32, sigma: f32, seed: &mut u64) -> Vec<i8> {
    bits.iter()
        .map(|&b| {
            let s: f32 = (0..4)
                .map(|_| next(seed) as f32 / (1u64 << 31) as f32 - 0.5)
                .sum();
            let v = if b == 0 { mag } else { -mag } + sigma * mag * s * 1.732;
            v.round().clamp(-127.0, 127.0) as i8
        })
        .collect()
}

fn run(code: &LdpcCode, sigma: f32, seed: &mut u64) {
    let info: Vec<u8> = (0..code.k()).map(|_| (next(seed) & 1) as u8).collect();
    let mut cw = vec![0u8; code.n()];
    LdpcEncoder::new(code)
        .unwrap()
        .encode(&info, &mut cw)
        .unwrap();
    let ll = noisy(&cw, 10.0, sigma, seed);
    let mut dec = LdpcDecoder::new(code, MinSum::default());
    let mut o = vec![0u8; code.n()];
    let r = dec.decode(&ll, 50, &mut o).unwrap();
    let t = best(|| {
        dec.decode(black_box(&ll), 50, &mut o).unwrap();
    });
    println!(
        "ldpc {}: {} iterations, {:8.2} Mbit/s info, {:8.2} Mbit/s per iteration",
        code.name(),
        r.iterations,
        code.k() as f64 / t / 1e6,
        code.k() as f64 * r.iterations as f64 / t / 1e6
    );
}

fn main() {
    let mut seed = 0x5EED_u64;
    for (n, r, sigma) in [
        (64800, 2, 1.6),
        (64800, 8, 0.75),
        (64800, 13, 0.4),
        (16200, 4, 1.2),
        (16200, 10, 0.55),
    ] {
        run(&LdpcCode::atsc3(n, r).unwrap(), sigma, &mut seed);
    }
    // A synthetic code of the DTMB shape (rate 0.8): dual-diagonal parity and three circulants per information group.
    let mut rows: Vec<Vec<(u16, u16)>> = (0..11u16)
        .map(|r| {
            if r == 0 {
                vec![(0, 0)]
            } else {
                vec![(r, 0), (r - 1, 0)]
            }
        })
        .collect();
    for c in 11..59u16 {
        for j in 0..3u16 {
            let r = ((c as usize * 7 + j as usize * 4) % 11) as u16;
            let s = (next(&mut seed) % 127) as u16;
            rows[r as usize].push((c, s));
        }
    }
    let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
    run(
        &LdpcCode::dtmb(&refs, InfoAt::End).unwrap(),
        0.45,
        &mut seed,
    );
}
