//! Benchmarks of the error-correction hot paths of `sdrkit-algo-fec` (that crate has no benchmark type of its own), for
//! `sdrkit_benchmark` on the device. Each builds its test frames first, then times the decoder on them. `items_per_second` counts
//! what each one's documentation says; `cpu_fraction_of_realtime` is for the rate it names.

use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::ldpc::tables::dvb_s2;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::rs::{codes as rs_codes, Rs, RsCode};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_algo_fec::Quantizer;
use sdrkit_core::bench::measure;
use sdrkit_core::{BenchResult, Benchmark};

/// A small repeatable generator.
struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
    fn bit(&mut self) -> u8 {
        (self.next() >> 7) as u8 & 1
    }
    /// Roughly Gaussian, mean 0, standard deviation 1 (a sum of four uniforms).
    fn gauss(&mut self) -> f32 {
        let s: f32 = (0..4)
            .map(|_| self.next() as f32 / (1u32 << 24) as f32)
            .sum();
        (s - 2.0) * 1.732
    }
}

fn ok<T, E>(r: Result<T, E>) -> T {
    // The parameters below are fixed and valid.
    r.unwrap_or_else(|_| unreachable!("benchmark parameters are valid"))
}

/// BPSK soft values of `coded` at noise standard deviation `sigma`, scaled into `i8` LLRs (positive means 0).
fn noisy_llrs(rng: &mut Lcg, coded: &[u8], sigma: f32, scale: f32) -> Vec<i8> {
    coded
        .iter()
        .map(|&c| {
            let x = if c == 0 { 1.0 } else { -1.0 } + sigma * rng.gauss();
            (x * scale).round().clamp(-127.0, 127.0) as i8
        })
        .collect()
}

/// A K = 7 rate 1/2 frame (the DVB code) of `info` bits, terminated, through noise at Eb/N0 about 4 dB.
fn k7_frame(info: usize) -> (Vec<u8>, Vec<i8>) {
    let mut rng = Lcg(0x1234_5678);
    let bits: Vec<u8> = (0..info).map(|_| rng.bit()).collect();
    let mut coded = Vec::new();
    let mut enc = Encoder::new(codes::DVB_K7);
    enc.encode(&bits, &mut coded);
    enc.terminate(&mut coded);
    let llr = noisy_llrs(&mut rng, &coded, 0.63, 20.0);
    (bits, llr)
}

/// K = 7 Viterbi, whole frames of 4096 bits; items are decoded bits, real time is 10 Mbit/s.
fn viterbi_k7(seconds: f64) -> BenchResult {
    let (bits, llr) = k7_frame(4096);
    let mut v = ok(Viterbi::new(codes::DVB_K7, bits.len() + 6));
    let mut out = vec![0u8; bits.len()];
    measure(seconds, bits.len(), bits.len() as f64 / 10e6, || {
        // The benchmark times the call, its result is not needed.
        let _ = v.decode(&llr, Ends::Zero, &mut out);
    })
}

/// K = 7 Viterbi on a stream (the sliding window of `fec.viterbi`); items are decoded bits, real time is 10 Mbit/s.
fn viterbi_k7_stream(seconds: f64) -> BenchResult {
    let (bits, llr) = k7_frame(16_384);
    let mut v = ok(Viterbi::new(codes::DVB_K7, 0));
    let mut out = Vec::with_capacity(bits.len() + 1024);
    measure(seconds, bits.len(), bits.len() as f64 / 10e6, || {
        out.clear();
        v.push(&llr, &mut out);
    })
}

/// Reed-Solomon words of `code` with `errors` symbol errors each (16 words per run); items are words.
fn rs_words(code: RsCode, errors: usize) -> (Rs, Vec<Vec<u8>>) {
    let rs = ok(Rs::new(code));
    let n = code.n as usize;
    let mut rng = Lcg(0xBEEF);
    let words = (0..16)
        .map(|_| {
            let mut w: Vec<u8> = (0..n).map(|_| (rng.next() & 0xFF) as u8).collect();
            ok(rs.encode_word(&mut w));
            for e in 0..errors {
                let i = (e * 13 + rng.next() as usize % 7) % n;
                w[i] ^= 0x5A;
            }
            w
        })
        .collect();
    (rs, words)
}

fn rs_run(seconds: f64, code: RsCode, errors: usize, bits_per_s: f64) -> BenchResult {
    let (mut rs, words) = rs_words(code, errors);
    let n = code.n as usize;
    let mut w = vec![0u8; n];
    measure(
        seconds,
        words.len(),
        (words.len() * n * 8) as f64 / bits_per_s,
        || {
            for word in &words {
                w.copy_from_slice(word);
                // The benchmark times the call, its result is not needed.
                let _ = rs.decode(&mut w, &[]);
            }
        },
    )
}

/// CCSDS RS(255,223) with 16 symbol errors per word (the most it corrects); items are words, real time is 10 Mbit/s.
fn rs_255_223(seconds: f64) -> BenchResult {
    rs_run(seconds, rs_codes::CCSDS_255_223, 16, 10e6)
}

/// DVB RS(204,188) with 8 symbol errors per packet; items are packets, real time is DVB-T's 31.67 Mbit/s.
fn rs_204_188(seconds: f64) -> BenchResult {
    rs_run(seconds, rs_codes::DVB_204_188, 8, 31.67e6)
}

/// DVB-S2 short frame, rate 1/2, layered min-sum (at most 50 iterations) at Eb/N0 about 2 dB; items are code bits, real time
/// is 10 MBd QPSK (20 Mbit/s of code bits).
fn ldpc_s2_short(seconds: f64) -> BenchResult {
    let code = ok(LdpcCode::from_dvb(&dvb_s2::S2_S_1_2));
    let mut rng = Lcg(77);
    let info: Vec<u8> = (0..code.k()).map(|_| rng.bit()).collect();
    let mut cw = vec![0u8; code.n()];
    ok(code.encode(&info, &mut cw));
    let llr = noisy_llrs(&mut rng, &cw, 0.79, 8.0);
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut out = vec![0u8; code.n()];
    measure(seconds, code.n(), code.n() as f64 / 20e6, || {
        // The benchmark times the call, its result is not needed.
        let _ = dec.decode(&llr, 50, &mut out);
    })
}

/// LTE turbo code, K = 6144, at most 8 iterations, stopping when the decisions are right; items are
/// information bits, real time is 50 Mbit/s.
fn turbo_lte(seconds: f64) -> BenchResult {
    let mut t = ok(Turbo::new(6144));
    let mut rng = Lcg(99);
    let bits: Vec<u8> = (0..6144).map(|_| rng.bit()).collect();
    let mut streams = vec![0u8; 3 * (6144 + 4)];
    ok(t.encode(&bits, &mut streams));
    let llr = noisy_llrs(&mut rng, &streams, 0.8, 8.0);
    let mut out = vec![0u8; 6144];
    measure(seconds, bits.len(), bits.len() as f64 / 50e6, || {
        // The benchmark times the call, its result is not needed.
        let _ = t.decode(&llr, 8, |b| b == &bits[..], &mut out);
    })
}

/// The adaptive quantizer (scale estimate and `i8` conversion) over 65,536 values; items are values, real time is 10 M values/s.
fn quantize(seconds: f64) -> BenchResult {
    let mut rng = Lcg(5);
    let x: Vec<f32> = (0..65_536).map(|_| 4.0 + 2.0 * rng.gauss()).collect();
    let q = Quantizer::default();
    let mut out = vec![0i8; x.len()];
    measure(seconds, x.len(), x.len() as f64 / 10e6, || {
        // The benchmark times the call, its result is not needed.
        let _ = q.quantize_auto(&x, &mut out);
    })
}

/// The `fec.*` benchmarks.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "fec.viterbi_k7",
            run: viterbi_k7,
        },
        Benchmark {
            name: "fec.viterbi_k7_stream",
            run: viterbi_k7_stream,
        },
        Benchmark {
            name: "fec.rs_255_223",
            run: rs_255_223,
        },
        Benchmark {
            name: "fec.rs_204_188",
            run: rs_204_188,
        },
        Benchmark {
            name: "fec.ldpc_dvbs2_short_1_2",
            run: ldpc_s2_short,
        },
        Benchmark {
            name: "fec.turbo_lte_6144",
            run: turbo_lte,
        },
        Benchmark {
            name: "fec.quantize",
            run: quantize,
        },
    ]
}
