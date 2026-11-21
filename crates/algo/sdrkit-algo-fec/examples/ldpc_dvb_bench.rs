//! Times LDPC decoding of the DVB-T2 normal frame (64800 bits) at rates 1/2 and 3/4, with every backend of the platform:
//! `cargo run --release -p sdrkit-algo-fec --example ldpc_dvb_bench`. The frames are BPSK in white noise at an Eb/N0 about
//! 1 dB above the threshold of the code. Each line is the best of three passes over the same frames. With a backend name
//! as the argument, only that backend runs.

use sdrkit_algo_fec::ldpc::{tables::dvb_s2, LdpcCode, LdpcKernel, MinSum, BACKENDS};
use sdrkit_algo_fec::Quantizer;
use std::hint::black_box;
use std::time::Instant;

const FRAMES: usize = 24;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    /// Uniform in (0, 1].
    fn uniform(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 1.0) / (1u64 << 53) as f64
    }
    fn gauss(&mut self) -> f64 {
        let (a, b) = (self.uniform(), self.uniform());
        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
    }
}

/// A random code word of `code` as 8-bit LLRs (mean magnitude 14) after BPSK in white noise at `ebn0_db`.
fn frame(code: &LdpcCode, ebn0_db: f64, rng: &mut Rng) -> Vec<i8> {
    let info: Vec<u8> = (0..code.k()).map(|_| (rng.next() >> 63) as u8).collect();
    let mut cw = vec![0u8; code.n()];
    code.encode(&info, &mut cw).unwrap();
    let rate = code.k() as f64 / code.n() as f64;
    let sigma2 = 1.0 / (2.0 * rate * 10f64.powf(ebn0_db / 10.0));
    let f: Vec<f32> = cw
        .iter()
        .map(|&b| {
            let x = if b == 0 { 1.0 } else { -1.0 };
            (2.0 * (x + sigma2.sqrt() * rng.gauss()) / sigma2) as f32
        })
        .collect();
    let q = Quantizer {
        target: 14.0,
        ..Default::default()
    };
    let mut out = vec![0i8; f.len()];
    q.quantize_auto(&f, &mut out).unwrap();
    out
}

fn run(code: &LdpcCode, ebn0_db: f64, max_iter: u32) {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let frames: Vec<Vec<i8>> = (0..FRAMES)
        .map(|_| frame(code, ebn0_db, &mut rng))
        .collect();
    let mut out = vec![0u8; code.n()];
    let only = std::env::args().nth(1);
    for b in BACKENDS.all() {
        if only.as_deref().is_some_and(|name| name != b.name()) {
            continue;
        }
        let Some(mut dec): Option<Box<dyn LdpcKernel>> = b.decoder(code, MinSum::default()) else {
            continue;
        };
        let (mut best, mut iters, mut failed) = (f64::INFINITY, 0u32, 0u32);
        for _ in 0..3 {
            (iters, failed) = (0, 0);
            let t = Instant::now();
            for f in &frames {
                match dec.decode(code, black_box(f), max_iter, &mut out) {
                    Ok(d) => iters += d.iterations,
                    Err(_) => {
                        iters += max_iter;
                        failed += 1;
                    }
                }
            }
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!(
            "ldpc {} at {ebn0_db:.1} dB, {:5}: {:5.1} iterations, {failed} of {FRAMES} frames failed, {:7.2} Mbit/s info, {:7.1} us per iteration",
            code.name(),
            b.name(),
            iters as f64 / FRAMES as f64,
            (code.k() * FRAMES) as f64 / best / 1e6,
            best * 1e6 / iters.max(1) as f64
        );
    }
}

fn main() {
    let half = LdpcCode::from_dvb(&dvb_s2::S2_N_1_2).unwrap();
    let three_quarters = LdpcCode::from_dvb(&dvb_s2::S2_N_3_4).unwrap();
    run(&half, 2.0, 50);
    run(&three_quarters, 3.3, 50);
    // Below the threshold no frame decodes: the cost of a frame that uses every iteration.
    run(&half, 0.0, 50);
}
