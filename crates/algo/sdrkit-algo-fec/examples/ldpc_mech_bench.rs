//! Times the LDPC mechanisms: `cargo run --release -p sdrkit-algo-fec --example ldpc_mech_bench`. Each line is the best of several
//! runs.

use sdrkit_algo_fec::ldpc::alist::Alist;
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, wsjt};
use sdrkit_algo_fec::ldpc::{BpDecoder, InfoAt, LdpcCode, LdpcEncoder};
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

fn bits(n: usize, seed: u32) -> Vec<u8> {
    (0..n as u32)
        .map(|i| (i.wrapping_add(seed).wrapping_mul(2_654_435_761) >> 13) as u8 & 1)
        .collect()
}

fn main() {
    // The sparse encoder against the existing encoders, and on codes that had none.
    let codes = [
        LdpcCode::from_checks("ft8 (174,91)", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap(),
        LdpcCode::from_dvb(&dvb_s2::S2_N_1_2).unwrap(),
        LdpcCode::nr(1, 64).unwrap(),
        LdpcCode::nr(1, 384).unwrap(),
        LdpcCode::nr(2, 384).unwrap(),
    ];
    for code in &codes {
        let t0 = Instant::now();
        let enc = LdpcEncoder::new(code).unwrap();
        let build = t0.elapsed().as_secs_f64();
        let info = bits(code.k(), 7);
        let mut cw = vec![0u8; code.n()];
        let t = best(|| enc.encode(black_box(&info), &mut cw).unwrap());
        println!(
            "ldpc sparse encode {:>14} (n {:5}, gap {:4}): {:9.1} us/word, {:6.2} ns/bit, built in {:7.1} ms",
            code.name(),
            code.n(),
            enc.gap(),
            t * 1e6,
            t * 1e9 / code.n() as f64,
            build * 1e3
        );
        if code.can_encode() {
            let t = best(|| code.encode(black_box(&info), &mut cw).unwrap());
            println!(
                "ldpc LdpcCode::encode {:>11} (n {:5}):           {:9.1} us/word, {:6.2} ns/bit",
                code.name(),
                code.n(),
                t * 1e6,
                t * 1e9 / code.n() as f64
            );
        }
    }

    // Belief propagation on a hopeless word, with and without early stop.
    let ft8 = &codes[0];
    let mut seed = 1u64;
    let llr: Vec<f32> = (0..174)
        .map(|_| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 40) as f32 / (1u64 << 23) as f32 - 1.0
        })
        .collect();
    let mut out = vec![0u8; 174];
    for patience in [0u32, 5] {
        let mut dec = BpDecoder::new(ft8).with_early_stop(patience);
        let t = best(|| {
            let _ = dec.decode(black_box(&llr), 100, &mut out);
        });
        println!(
            "bp ft8 hopeless word, max 100 iterations, patience {patience}: {:8.1} us ({} iterations)",
            t * 1e6,
            dec.last_iterations()
        );
    }

    // The alist reader and writer on the FT8 matrix.
    let text = Alist::from_code(ft8).to_text();
    let t = best(|| {
        black_box(Alist::parse(black_box(&text)).unwrap());
    });
    println!("alist parse ft8 ({} bytes): {:8.1} us", text.len(), t * 1e6);
}
