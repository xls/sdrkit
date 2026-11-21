//! LDPC backends: every backend of the platform gives the result of the plain backend (hard bits, success and iteration
//! count) on frames that decode and on frames that do not, for codes of every layout the decoder knows.

mod common;
use common::*;
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, dvb_t2};
use sdrkit_algo_fec::ldpc::{check_backend, InfoAt, LdpcCode, LdpcDecoder, MinSum, BACKENDS};

/// A code word of `code`: of random information bits when the code has an encoder, else the zero word.
fn word(code: &LdpcCode, rng: &mut Rng) -> Vec<u8> {
    let mut cw = vec![0u8; code.n()];
    if code.can_encode() {
        code.encode(&rng.bits(code.k()), &mut cw).unwrap();
    }
    cw
}

/// LLRs of `cw` at magnitude 16 with uniform noise of up to `noise`, limited to the 8-bit range.
fn uniform_llr(rng: &mut Rng, cw: &[u8], noise: usize) -> Vec<i8> {
    cw.iter()
        .map(|&b| {
            let v = if b == 0 { 16 } else { -16 };
            (v + rng.below(2 * noise + 1) as i32 - noise as i32).clamp(-128, 127) as i8
        })
        .collect()
}

/// Decodes `llr` with every backend and compares each result with that of the plain backend. Returns whether the frame
/// decoded.
fn same_on_every_backend(code: &LdpcCode, rule: MinSum, llr: &[i8], max_iter: u32) -> bool {
    let mut want = vec![0u8; code.n()];
    let mut plain = BACKENDS.plain().decoder(code, rule).unwrap();
    let a = plain.decode(code, llr, max_iter, &mut want);
    let mut post = vec![0i16; code.n()];
    assert!(plain.posterior(code, &mut post), "{}: plain posteriors", code.name());
    assert!(post.iter().zip(&want).all(|(&p, &h)| (p < 0) as u8 == h), "{}: posterior signs", code.name());
    for b in BACKENDS.all() {
        let Some(mut dec) = b.decoder(code, rule) else {
            continue;
        };
        let mut got = vec![0u8; code.n()];
        let mut soft = vec![0i16; code.n()];
        // Twice: the second run starts from the state the first one left.
        for _ in 0..2 {
            let r = dec.decode(code, llr, max_iter, &mut got);
            assert_eq!(r, a, "{} with {}: result", code.name(), b.name());
            assert!(got == want, "{} with {}: hard bits", code.name(), b.name());
            assert!(dec.posterior(code, &mut soft) && soft == post, "{} with {}: posteriors", code.name(), b.name());
        }
    }
    a.is_ok()
}

/// A code with a column group that occurs twice in a layer, and with lanes that have no edge: one in the first 16 lanes
/// and one in the last, partly used, 16.
fn code_with_repeats_and_holes(z: usize) -> LdpcCode {
    let last = (z - 1) as u16;
    let rows: Vec<Vec<(u16, u16, Option<u16>)>> = vec![
        vec![
            (0, 1, None),
            (0, 5, None),
            (1, 2, Some(3)),
            (2, 0, None),
            (3, 0, None),
        ],
        vec![
            (0, 3, None),
            (1, 4, None),
            (1, 0, None),
            (2, 4, None),
            (3, 0, None),
            (4, 0, None),
        ],
        vec![
            (0, 2, Some(last)),
            (1, 1, None),
            (2, 3, None),
            (4, 0, None),
            (5, 0, None),
        ],
    ];
    let refs: Vec<&[(u16, u16, Option<u16>)]> = rows.iter().map(|r| r.as_slice()).collect();
    LdpcCode::from_qc_masked("repeats and holes", z, 6, 3 * z, &refs).unwrap()
}

/// A code of the DTMB shape (59 groups of 127): a dual-diagonal parity part and three circulants per information group.
fn dtmb_shape() -> LdpcCode {
    let mut rng = Rng::new(40);
    let mut rows: Vec<Vec<(u16, u16)>> = (0..11u16)
        .map(|r| {
            if r > 0 {
                vec![(r, 0), (r - 1, 0)]
            } else {
                vec![(r, 0)]
            }
        })
        .collect();
    for c in 11..59u16 {
        let mut used = Vec::new();
        while used.len() < 3 {
            let r = rng.below(11);
            if !used.contains(&r) {
                used.push(r);
                rows[r].push((c, rng.below(127) as u16));
            }
        }
    }
    let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
    LdpcCode::dtmb(&refs, InfoAt::End).unwrap()
}

#[test]
fn every_backend_passes_the_backend_check() {
    for b in BACKENDS.all() {
        check_backend(*b);
    }
}

#[test]
fn the_default_decoder_names_its_backend() {
    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let dec = LdpcDecoder::new(&code, MinSum::default());
    let want = if cfg!(target_arch = "aarch64") {
        "neon"
    } else {
        "plain"
    };
    assert_eq!(dec.backend(), want);
}

#[test]
fn backends_agree_on_every_code_layout() {
    let codes = [
        LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap(),
        LdpcCode::from_dvb(&dvb_t2::T2_S_3_5).unwrap(),
        LdpcCode::atsc3(16200, 5).unwrap(),
        LdpcCode::atsc3(16200, 11).unwrap(),
        LdpcCode::nr(1, 20).unwrap(),
        LdpcCode::nr(2, 6).unwrap(),
        LdpcCode::nr(2, 36).unwrap(),
        dtmb_shape(),
        code_with_repeats_and_holes(5),
        code_with_repeats_and_holes(16),
        code_with_repeats_and_holes(37),
        code_with_repeats_and_holes(360),
    ];
    let rules = [
        MinSum::default(),
        MinSum {
            scale_q4: 16,
            offset: 1,
        },
        MinSum {
            scale_q4: 40,
            offset: 0,
        },
        MinSum {
            scale_q4: 255,
            offset: 200,
        },
    ];
    let mut rng = Rng::new(77);
    for code in &codes {
        let (mut decoded, mut failed) = (0, 0);
        for (i, noise) in [0, 6, 12, 16, 20, 24, 30, 60, 200].into_iter().enumerate() {
            let cw = word(code, &mut rng);
            let llr = uniform_llr(&mut rng, &cw, noise);
            for rule in [rules[0], rules[1 + i % 3]] {
                if same_on_every_backend(code, rule, &llr, 25) {
                    decoded += 1;
                } else {
                    failed += 1;
                }
            }
        }
        // Full-scale input, both signs.
        let cw = word(code, &mut rng);
        let llr: Vec<i8> = cw
            .iter()
            .map(|&b| if b == 0 { 127 } else { -128 })
            .collect();
        assert!(same_on_every_backend(code, rules[0], &llr, 25));
        eprintln!("{}: {decoded} frames decoded, {failed} not", code.name());
        assert!(decoded >= 2 && failed >= 2, "{}", code.name());
    }
}

#[test]
fn backends_agree_on_dvb_normal_frames() {
    let mut rng = Rng::new(78);
    for (table, ebn0) in [
        (&dvb_s2::S2_N_1_2, [2.0, 0.6]),
        (&dvb_s2::S2_N_3_4, [3.3, 1.8]),
        (&dvb_t2::T2_N_2_3, [3.0, 1.5]),
    ] {
        let code = LdpcCode::from_dvb(table).unwrap();
        let rate = code.k() as f64 / code.n() as f64;
        // The first Eb/N0 is above the threshold of the code and the second below it.
        for (e, decodes) in ebn0.into_iter().zip([true, false]) {
            let cw = word(&code, &mut rng);
            let llr = bpsk_llr8(&mut rng, &cw, rate, e, 14.0);
            let ok = same_on_every_backend(&code, MinSum::default(), &llr, 20);
            assert_eq!(ok, decodes, "{} at {e} dB", code.name());
        }
    }
}

/// Frame errors of each backend on the DVB normal frame at rates 1/2 and 3/4, 200 frames at an Eb/N0 in the waterfall and
/// 200 above it, 50 iterations. Slow without optimisation:
/// `cargo test --release -p sdrkit-algo-fec --test ldpc_backends -- --ignored --nocapture`.
#[test]
#[ignore]
fn backends_have_the_same_frame_errors_on_dvb_normal_frames() {
    let mut rng = Rng::new(79);
    for (table, points) in [
        (&dvb_s2::S2_N_1_2, [0.85, 1.3]),
        (&dvb_s2::S2_N_3_4, [2.15, 2.9]),
    ] {
        let code = LdpcCode::from_dvb(table).unwrap();
        let rate = code.k() as f64 / code.n() as f64;
        let rule = MinSum::default();
        for ebn0 in points {
            let mut errors = vec![0u32; BACKENDS.all().len()];
            let mut iterations = vec![0u32; BACKENDS.all().len()];
            let mut out = vec![0u8; code.n()];
            let mut decs: Vec<_> = BACKENDS
                .all()
                .iter()
                .map(|b| b.decoder(&code, rule).unwrap())
                .collect();
            for _ in 0..200 {
                let cw = word(&code, &mut rng);
                let llr = bpsk_llr8(&mut rng, &cw, rate, ebn0, 14.0);
                for (i, dec) in decs.iter_mut().enumerate() {
                    match dec.decode(&code, &llr, 50, &mut out) {
                        Ok(d) if out == cw => iterations[i] += d.iterations,
                        _ => {
                            errors[i] += 1;
                            iterations[i] += 50;
                        }
                    }
                }
            }
            for (i, b) in BACKENDS.all().iter().enumerate() {
                eprintln!(
                    "{} at {ebn0} dB, {}: {} of 200 frames in error, {} iterations",
                    code.name(),
                    b.name(),
                    errors[i],
                    iterations[i]
                );
                assert_eq!((errors[i], iterations[i]), (errors[0], iterations[0]));
            }
        }
    }
}
