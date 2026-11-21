//! Times the hot paths: `cargo run --release -p sdrkit-algo-fec --example bench`. Each line is the best of several runs.

use sdrkit_algo_fec::bch::{codes as bch_codes, Bch};
use sdrkit_algo_fec::conv::tcm::{self, TcmCodec};
use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Kernel, Viterbi, BACKENDS};
use sdrkit_algo_fec::ldpc::{tables::dvb_s2, LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::polar::{Polar, PolarParams};
use sdrkit_algo_fec::rs::{codes as rscodes, Rs, BACKENDS as rsbackends};
use sdrkit_algo_fec::turbo::Turbo;
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

/// A deterministic noisy LLR stream: hard bits at magnitude `mag` with Gaussian-like noise from a simple generator.
fn noisy(bits: &[u8], mag: f32, sigma: f32, seed: &mut u64) -> Vec<i8> {
    bits.iter()
        .map(|&b| {
            let mut s = 0f32;
            for _ in 0..4 {
                *seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                s += (*seed >> 40) as f32 / (1u64 << 24) as f32 - 0.5;
            }
            let v = if b == 0 { mag } else { -mag } + sigma * mag * s * 1.732;
            v.round().clamp(-127.0, 127.0) as i8
        })
        .collect()
}

fn main() {
    let mut seed = 1u64;
    let bits: Vec<u8> = (0..100_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8 & 1)
        .collect();

    // Viterbi K = 7 rate 1/2.
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let llr = noisy(&coded, 20.0, 0.6, &mut seed);
    let mut out = vec![0u8; bits.len()];
    let kernels = BACKENDS.all().iter().map(|&b| Kernel::Butterfly(b));
    for k in kernels.chain([Kernel::Generic32]) {
        let Ok(mut v) = Viterbi::with_kernel(codes::DVB_K7, bits.len(), k) else {
            continue;
        };
        let t = best(|| {
            v.decode(black_box(&llr), Ends::Open, &mut out).unwrap();
        });
        println!(
            "viterbi K7 r1/2 frame, {k:?}: {:8.2} ns/bit",
            t * 1e9 / bits.len() as f64
        );
    }
    let mut v = Viterbi::new(codes::DVB_K7, 0).unwrap();
    let mut s = Vec::with_capacity(bits.len() + 1000);
    let t = best(|| {
        v.reset();
        s.clear();
        v.push(black_box(&llr), &mut s);
    });
    println!(
        "viterbi K7 r1/2 streaming ({:?}): {:8.2} ns/bit",
        v.kernel(),
        t * 1e9 / bits.len() as f64
    );
    // Streaming one step per push (the keyboard protocols feed each symbol as it comes), K = 7 and K = 15, with the per-step gain.
    for code in [codes::INMARSAT_K7, codes::THOR_K15] {
        let mut c = Vec::new();
        Encoder::new(code).encode(&bits[..20_000], &mut c);
        let l = noisy(&c, 20.0, 0.6, &mut seed);
        let mut v = Viterbi::new(code, 0).unwrap();
        let depth = 5 * code.k() as usize;
        v.set_traceback(depth, depth.div_ceil(4)).unwrap();
        let mut s = Vec::with_capacity(64);
        let mut g = 0f32;
        let t = best(|| {
            v.reset();
            for step in l.chunks(2) {
                s.clear();
                v.push(black_box(step), &mut s);
                g += v.last_gain();
            }
        });
        black_box(g);
        println!(
            "viterbi K{} r1/2 streaming one step per push ({:?}): {:8.2} ns/bit",
            code.k(),
            v.kernel(),
            t * 1e9 / 20_000.0
        );
    }
    let mut c3 = Vec::new();
    Encoder::new(codes::HD_AM_E1).encode(&bits[..20_000], &mut c3);
    let l3 = noisy(&c3, 20.0, 0.6, &mut seed);
    let mut v9 = Viterbi::new(codes::HD_AM_E1, 20_000).unwrap();
    let t = best(|| {
        v9.decode(black_box(&l3), Ends::Open, &mut out).unwrap();
    });
    println!(
        "viterbi K9 r1/3 frame ({:?}): {:8.2} ns/bit",
        v9.kernel(),
        t * 1e9 / 20_000.0
    );

    // LDPC.
    for (name, table) in [
        ("DVB-S2 normal 1/2", &dvb_s2::S2_N_1_2),
        ("DVB-S2 short 1/2", &dvb_s2::S2_S_1_2),
    ] {
        let code = LdpcCode::from_dvb(table).unwrap();
        let info: Vec<u8> = bits[..code.k()].to_vec();
        let mut cw = vec![0u8; code.n()];
        code.encode(&info, &mut cw).unwrap();
        let ll = noisy(&cw, 10.0, 0.75, &mut seed);
        let mut dec = LdpcDecoder::new(&code, MinSum::default());
        let mut o = vec![0u8; code.n()];
        let r = dec.decode(&ll, 50, &mut o).unwrap();
        let t = best(|| {
            dec.decode(black_box(&ll), 50, &mut o).unwrap();
        });
        println!(
            "ldpc {name}: {} iterations, {:8.2} Mbit/s info, {:8.2} Mbit/s per iteration",
            r.iterations,
            code.k() as f64 / t / 1e6,
            code.k() as f64 * r.iterations as f64 / t / 1e6
        );
    }

    // Reed-Solomon (255,223).
    for &backend in rsbackends.all() {
        let mut rs = Rs::new(rscodes::CCSDS_255_223).unwrap();
        if rs.set_backend(backend).is_err() {
            continue;
        }
        let mut word: Vec<u8> = bits[..255].iter().map(|&b| b * 0x5A).collect();
        rs.encode_word(&mut word).unwrap();
        let clean = word.clone();
        let t = best(|| {
            for _ in 0..1000 {
                black_box(rs.check(black_box(&clean)).unwrap());
            }
        });
        let t2 = best(|| {
            for _ in 0..100 {
                word.copy_from_slice(&clean);
                for i in 0..16 {
                    word[i * 15 + 3] ^= 0x77;
                }
                rs.decode(&mut word, &[]).unwrap();
            }
        });
        println!(
            "rs(255,223) {}: {:10.0} clean words/s, {:8.0} words/s with 16 errors",
            rs.backend(),
            1000.0 / t,
            100.0 / t2
        );
    }

    // BCH DVB-S2 normal 1/2.
    let mut bch = Bch::new(16, 0x1002D, 32_400, 12).unwrap();
    let mut bw = bits[..32_400].to_vec();
    bch.encode(&mut bw).unwrap();
    let t = best(|| bch.encode(black_box(&mut bw)).unwrap());
    let t2 = best(|| {
        bw[100] ^= 1;
        bch.decode(&mut bw).unwrap();
    });
    println!(
        "bch dvb-s2 (32400): encode {:8.1} us, decode one error {:8.1} us",
        t * 1e6,
        t2 * 1e6
    );

    // Short BCH: POCSAG (31,21) by the algebraic decoder.
    let mut pb = Bch::from_code(&bch_codes::POCSAG_31_21).unwrap();
    let mut pw = bits[..31].to_vec();
    pb.encode(&mut pw).unwrap();
    let t = best(|| {
        for _ in 0..1000 {
            pw[3] ^= 1;
            pw[20] ^= 1;
            pb.decode(black_box(&mut pw)).unwrap();
        }
    });
    println!(
        "bch pocsag (31,21), two errors: {:8.0} ns per word",
        t * 1e6
    );

    // DMR rate 3/4 trellis words.
    let mut tc = TcmCodec::new(tcm::DMR_3_4).unwrap();
    let mut sent = [0u8; 196];
    tc.encode(&bits[..144], &mut sent).unwrap();
    let levels: Vec<f32> = sent
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| tcm::dibit_symbol((c[0] << 1) | c[1]) as f32 * 0.9)
        .collect();
    let mut got = [0u8; 144];
    let t = best(|| {
        for _ in 0..100 {
            tc.decode_bits(black_box(&sent), &mut got).unwrap();
        }
    });
    let t2 = best(|| {
        for _ in 0..100 {
            tc.decode_levels(black_box(&levels), &mut got).unwrap();
        }
    });
    println!(
        "tcm dmr 3/4 word: hard {:8.2} us, soft {:8.2} us",
        t * 1e4,
        t2 * 1e4
    );

    // Turbo.
    let mut tu = Turbo::new(6144).unwrap();
    let tb = &bits[..6144];
    let mut ts = vec![0u8; 3 * 6148];
    tu.encode(tb, &mut ts).unwrap();
    let tl = noisy(&ts, 10.0, 0.7, &mut seed);
    let mut to = vec![0u8; 6144];
    let t = best(|| {
        let _ = tu.decode(black_box(&tl), 8, |_| false, &mut to);
    });
    println!(
        "turbo K 6144, 8 iterations: {:8.2} Mbit/s",
        6144.0 / t / 1e6
    );

    // Polar PBCH.
    let mut p = Polar::new(PolarParams::PBCH, 8).unwrap();
    let pb = &bits[..56];
    let mut pe = [0u8; 864];
    p.encode(pb, &mut pe).unwrap();
    let pl: Vec<f32> = pe
        .iter()
        .map(|&b| if b == 0 { 1.0 } else { -1.0 })
        .collect();
    let mut po = [0u8; 56];
    let t = best(|| {
        p.decode(black_box(&pl), |c| c == pb, &mut po).unwrap();
    });
    println!("polar PBCH list 8: {:8.1} us per decode", t * 1e6);
}
