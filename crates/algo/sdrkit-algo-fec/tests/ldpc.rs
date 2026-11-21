//! LDPC: every published table builds, its encoder (the standard's accumulator) agrees with the decoder's quasi-cyclic layout,
//! the DVB BCH polynomials and parameters, the WSJT generators, NR base graphs, the DVB-S2 threshold, and BP with OSD.

mod common;
use common::*;
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::gf::Gf;
use sdrkit_algo_fec::ldpc::tables::{dvb_bch, dvb_s2, dvb_s2x, dvb_t2, wsjt, DvbTable};
use sdrkit_algo_fec::ldpc::{BpDecoder, InfoAt, LdpcCode, LdpcDecoder, MinSum, Osd};

fn all_dvb() -> Vec<&'static DvbTable> {
    dvb_s2::ALL
        .iter()
        .chain(dvb_s2x::ALL)
        .chain(dvb_t2::ALL)
        .copied()
        .collect()
}

#[test]
fn every_dvb_table_is_consistent_with_its_standard() {
    for t in all_dvb() {
        assert_eq!(t.rows.len() as u32, t.k / 360, "{}", t.name);
        assert_eq!(t.q * 360, t.n - t.k, "{}", t.name);
        assert!(
            t.rows
                .iter()
                .all(|r| r.iter().all(|&x| (x as u32) < t.n - t.k)),
            "{}",
            t.name
        );
    }
    // EN 302 307-1 tables B.4 and 7a: normal 1/2 has q = 90 and rows 36 x 8 then 54 x 3.
    let t = &dvb_s2::S2_N_1_2;
    assert_eq!(t.q, 90);
    assert!(t.rows[..36].iter().all(|r| r.len() == 8) && t.rows[36..].iter().all(|r| r.len() == 3));
    assert_eq!(
        t.rows[0],
        [54, 9318, 14392, 27561, 26909, 10219, 2534, 8597]
    );
    assert_eq!(dvb_s2::ALL.len(), 21);
    assert_eq!(dvb_s2x::ALL.len(), 34);
}

#[test]
fn dvb_encoder_and_quasi_cyclic_checks_agree_for_every_table() {
    let mut rng = Rng::new(9);
    for t in all_dvb() {
        let code = LdpcCode::from_dvb(t).unwrap();
        // Every check has the degree the table implies: n - k checks, each from the table plus two parity bits (one for row 0).
        assert_eq!(code.checks(), (t.n - t.k) as usize);
        let table_ones: usize = t.rows.iter().map(|r| r.len()).sum::<usize>() * 360;
        assert_eq!(
            code.edges(),
            table_ones + 2 * (t.n - t.k) as usize - 1,
            "{}",
            t.name
        );
        let info = rng.bits(code.k());
        let mut cw = vec![0u8; code.n()];
        code.encode(&info, &mut cw).unwrap();
        assert_eq!(&cw[..code.k()], &info[..]);
        assert!(code.is_codeword(&cw), "{}", t.name);
        cw[17] ^= 1;
        assert!(!code.is_codeword(&cw));
    }
}

#[test]
fn dvb_short_frames_decode_with_errors() {
    let mut rng = Rng::new(10);
    for t in dvb_s2::ALL.iter().filter(|t| t.n == 16_200) {
        let code = LdpcCode::from_dvb(t).unwrap();
        let mut dec = LdpcDecoder::new(&code, MinSum::default());
        let info = rng.bits(code.k());
        let mut cw = vec![0u8; code.n()];
        code.encode(&info, &mut cw).unwrap();
        let rate = code.k() as f64 / code.n() as f64;
        let llr = bpsk_llr8(&mut rng, &cw, rate, 4.5, 16.0);
        let mut out = vec![0u8; code.n()];
        let r = dec
            .decode(&llr, 50, &mut out)
            .unwrap_or_else(|e| panic!("{}: {e}", t.name));
        assert_eq!(out, cw, "{}", t.name);
        assert!(r.iterations < 50);
    }
}

#[test]
fn dvb_s2_bch_polynomials_and_parameters() {
    // EN 302 307-1 table 6a / 6b: g_i is the minimal polynomial of α^(2i-1).
    for (m, poly, mins) in [
        (16u8, 0x1002Du32, &dvb_bch::S2_NORMAL_MINPOLYS),
        (14, 0x402B, &dvb_bch::S2_SHORT_MINPOLYS),
        (
            15,
            dvb_bch::S2X_MEDIUM_MINPOLYS[0],
            &dvb_bch::S2X_MEDIUM_MINPOLYS,
        ),
    ] {
        assert_eq!(mins[0], poly);
        let f = Gf::new(m, poly).unwrap();
        for (i, &g) in mins.iter().enumerate() {
            assert_eq!(f.minimal_poly(2 * i + 1), g as u64, "m {m} g{}", i + 1);
        }
    }
    for &(name, kb, nb, t) in dvb_bch::S2_BCH_PARAMS.iter().chain(dvb_bch::T2_BCH_PARAMS) {
        let (m, poly) = if name.starts_with("normal") {
            (16, 0x1002D)
        } else {
            (14, 0x402B)
        };
        let b = Bch::new(m, poly, nb as usize, t as usize).unwrap();
        assert_eq!(b.k() as u32, kb, "{name}");
    }
    // Encode and correct t errors for normal 2/3 (t = 10).
    let mut rng = Rng::new(3);
    let mut b = Bch::new(16, 0x1002D, 43_200, 10).unwrap();
    let mut w = rng.bits(43_200);
    b.encode(&mut w).unwrap();
    let sent = w.clone();
    for _ in 0..10 {
        let i = rng.below(43_200);
        w[i] ^= 1;
    }
    b.decode(&mut w).unwrap();
    assert_eq!(w, sent);
}

fn hex_bits(s: &str, n: usize) -> Vec<u8> {
    s.chars()
        .flat_map(|c| {
            let v = c.to_digit(16).unwrap();
            (0..4).rev().map(move |i| ((v >> i) & 1) as u8)
        })
        .take(n)
        .collect()
}

#[test]
fn ft8_checks_give_the_published_generator() {
    let code = LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap();
    assert!(code.can_encode());
    let gen: Vec<Vec<u8>> = wsjt::FT8_GENERATOR_HEX
        .iter()
        .map(|h| hex_bits(h, 91))
        .collect();
    for j in 0..91 {
        let mut u = vec![0u8; 91];
        u[j] = 1;
        let mut cw = vec![0u8; 174];
        code.encode(&u, &mut cw).unwrap();
        for i in 0..83 {
            assert_eq!(cw[91 + i], gen[i][j], "parity {i} info {j}");
        }
    }
}

#[test]
fn js8_checks_give_the_published_generator() {
    let code = LdpcCode::from_checks("js8", 174, 87, &wsjt::JS8_CHECKS, InfoAt::End).unwrap();
    let gen: Vec<Vec<u8>> = wsjt::JS8_GENERATOR_HEX
        .iter()
        .map(|h| hex_bits(h, 87))
        .collect();
    for j in 0..87 {
        let mut u = vec![0u8; 87];
        u[j] = 1;
        let mut cw = vec![0u8; 174];
        code.encode(&u, &mut cw).unwrap();
        for i in 0..87 {
            assert_eq!(
                cw[wsjt::JS8_COLUMN_ORDER[i] as usize],
                gen[i][j],
                "parity {i} info {j}"
            );
        }
    }
}

#[test]
fn ft8_belief_propagation_and_osd() {
    let mut rng = Rng::new(12);
    let code = LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap();
    let mut bp = BpDecoder::new(&code);
    let mut osd = Osd::new(&code).unwrap();
    let mut out = [0u8; 174];
    let (mut bp_ok, mut osd_ok) = (0, 0);
    for _ in 0..40 {
        let info = rng.bits(91);
        let mut cw = vec![0u8; 174];
        code.encode(&info, &mut cw).unwrap();
        let llr = bpsk_llr(&mut rng, &cw, 91.0 / 174.0, 2.5);
        if bp.decode(&llr, 30, &mut out).is_ok() && out[..] == cw[..] {
            bp_ok += 1;
        } else {
            let post = bp.posterior().to_vec();
            osd.decode(&post, 2, &mut out).unwrap();
            if out[..] == cw[..] {
                osd_ok += 1;
            }
        }
    }
    assert!(bp_ok + osd_ok >= 38, "{bp_ok} {osd_ok}");
    assert!(osd_ok > 0 || bp_ok == 40);
}

#[test]
fn nr_base_graphs_lift_and_decode() {
    let mut rng = Rng::new(13);
    for (bg, z) in [(1u8, 2usize), (1, 15), (1, 64), (2, 3), (2, 36), (2, 52)] {
        let code = LdpcCode::nr(bg, z).unwrap();
        assert_eq!(code.n(), if bg == 1 { 68 * z } else { 52 * z });
        assert!(code.can_encode(), "bg {bg} z {z}");
        let info = rng.bits(code.k());
        let mut cw = vec![0u8; code.n()];
        code.encode(&info, &mut cw).unwrap();
        assert!(code.is_codeword(&cw));
        let mut llr = bpsk_llr8(&mut rng, &cw, 0.4, 3.0, 16.0);
        // The first 2 z bits are never sent.
        for v in &mut llr[..2 * z] {
            *v = 0;
        }
        let mut dec = LdpcDecoder::new(&code, MinSum::default());
        let mut out = vec![0u8; code.n()];
        dec.decode(&llr, 50, &mut out)
            .unwrap_or_else(|e| panic!("bg {bg} z {z}: {e}"));
        assert_eq!(out, cw);
    }
    // The largest lifting: no dense encoder, but the all-zero word decodes through noise.
    let code = LdpcCode::nr(1, 384).unwrap();
    assert!(!code.can_encode());
    let zero = vec![0u8; code.n()];
    let llr = bpsk_llr8(&mut rng, &zero, 1.0 / 3.0, 2.5, 16.0);
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut out = vec![0u8; code.n()];
    dec.decode(&llr, 30, &mut out).unwrap();
    assert!(out.iter().all(|&b| b == 0));
    assert!(LdpcCode::nr(1, 7 * 64).is_err());
    assert!(LdpcCode::nr(3, 8).is_err());
}

/// The posterior LLRs of the min-sum decoder: after a decode their signs are the hard bits, they are stronger than the input
/// on a decoded frame, and on a frame that fails they still mark the bits that the channel flipped as weaker than the rest.
#[test]
fn min_sum_posteriors() {
    let mut rng = Rng::new(14);
    let code = LdpcCode::nr(2, 16).unwrap();
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut post = vec![0i16; code.n()];
    assert!(dec.posterior(&mut post[..code.n() - 1]).is_err());
    let mut cw = vec![0u8; code.n()];
    code.encode(&rng.bits(code.k()), &mut cw).unwrap();
    let mut out = vec![0u8; code.n()];
    for (esn0, decodes) in [(3.0, true), (-3.0, false)] {
        let mut llr = bpsk_llr8(&mut rng, &cw, 0.2, esn0, 16.0);
        for v in &mut llr[..2 * code.z()] {
            *v = 0;
        }
        assert_eq!(dec.decode(&llr, 30, &mut out).is_ok(), decodes, "{esn0} dB");
        dec.posterior(&mut post).unwrap();
        assert!(post.iter().zip(&out).all(|(&p, &h)| (p < 0) as u8 == h), "{esn0} dB: signs");
        let mean = |w: &dyn Fn(usize) -> bool| {
            let v: Vec<f64> = (0..code.n()).filter(|&i| w(i)).map(|i| f64::from(post[i]) * if cw[i] == 0 { 1.0 } else { -1.0 }).collect();
            v.iter().sum::<f64>() / v.len().max(1) as f64
        };
        let input = llr.iter().map(|&v| f64::from(v).abs()).sum::<f64>() / code.n() as f64;
        if decodes {
            assert!(mean(&|_| true) > 2.0 * input, "{esn0} dB: {} against {input}", mean(&|_| true));
        } else {
            let flipped = |i: usize| i >= 2 * code.z() && (llr[i] < 0) != (cw[i] == 1);
            assert!(mean(&|i| !flipped(i)) > mean(&flipped), "{esn0} dB");
        }
    }
}

/// DVB-S2 normal frame rate 1/2: EN 302 307-1 table 13 lists quasi-error-free QPSK 1/2 at an ideal Es/N0 of 1.0 dB (Eb/N0 of the
/// coded bits about 1.0 dB at rate 1/2). With 8-bit messages and normalised min-sum, frames decode at 1.2 dB and fail at 0.4 dB.
#[test]
fn dvb_s2_normal_half_rate_threshold_near_1_db() {
    let mut rng = Rng::new(14);
    let code = LdpcCode::from_dvb(&dvb_s2::S2_N_1_2).unwrap();
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut out = vec![0u8; code.n()];
    let mut run = |ebn0: f64, frames: usize| -> usize {
        let mut ok = 0;
        for _ in 0..frames {
            let info = rng.bits(code.k());
            let mut cw = vec![0u8; code.n()];
            code.encode(&info, &mut cw).unwrap();
            let llr = bpsk_llr8(&mut rng, &cw, 0.5, ebn0, 14.0);
            if dec.decode(&llr, 80, &mut out).is_ok() && out == cw {
                ok += 1;
            }
        }
        ok
    };
    let good = run(1.2, 4);
    let bad = run(0.4, 3);
    eprintln!("frames decoded: {good}/4 at 1.2 dB, {bad}/3 at 0.4 dB");
    assert_eq!(good, 4);
    assert_eq!(bad, 0);
}

#[test]
fn bad_input() {
    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut out = vec![0u8; code.n()];
    assert!(dec.decode(&[0i8; 10], 5, &mut out).is_err());
    assert!(code.encode(&[0u8; 3], &mut out).is_err());
    assert!(LdpcCode::from_checks("x", 10, 12, &[&[1u16, 2][..]], InfoAt::Start).is_err());
    assert!(LdpcCode::from_checks("x", 10, 4, &[&[1u16, 20][..]], InfoAt::Start).is_err());
}
