//! LTE turbo code and rate matching, the LTE tail-biting chain, and NR polar codes.

mod common;
use common::*;
use sdrkit_algo_fec::conv::{codes, encode_tail_biting, Ends, Viterbi};
use sdrkit_algo_fec::polar::{Polar, PolarParams};
use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
use sdrkit_algo_fec::tables_3gpp::LTE_QPP;
use sdrkit_algo_fec::turbo::{size_at_least, sizes, Turbo};
use sdrkit_util_bits::crc::catalog::CRC24_NR_C;

#[test]
fn qpp_table_holds_every_block_size() {
    assert_eq!(sizes().count(), 188);
    assert_eq!(LTE_QPP[0], (40, 3, 10));
    assert_eq!(LTE_QPP[187], (6144, 263, 480));
    for &(k, _, _) in LTE_QPP.iter().step_by(7) {
        let t = Turbo::new(k as usize).unwrap();
        let mut seen = vec![false; k as usize];
        for &p in t.interleaver() {
            assert!(!seen[p as usize]);
            seen[p as usize] = true;
        }
    }
    assert_eq!(size_at_least(41), Some(48));
    assert!(Turbo::new(41).is_err());
}

fn turbo_frame(rng: &mut Rng, t: &mut Turbo, ebn0: f64, iters: u32) -> (bool, u32) {
    let k = t.k();
    let bits = rng.bits(k);
    let mut s = vec![0u8; 3 * (k + 4)];
    t.encode(&bits, &mut s).unwrap();
    let llr = bpsk_llr8(rng, &s, 1.0 / 3.0, ebn0, 20.0);
    let mut out = vec![0u8; k];
    match t.decode(&llr, iters, |b| b == &bits[..], &mut out) {
        Ok(r) => (true, r.iterations),
        Err(_) => (false, iters),
    }
}

/// Near-capacity behaviour (rate 1/3, max-log-MAP): K = 6144 decodes at 0.8 dB Eb/N0 within 8 iterations; the short K = 40
/// block needs more SNR.
#[test]
fn turbo_decodes_near_capacity() {
    let mut rng = Rng::new(1);
    let mut t = Turbo::new(6144).unwrap();
    let mut ok = 0;
    for _ in 0..4 {
        ok += turbo_frame(&mut rng, &mut t, 0.8, 8).0 as usize;
    }
    assert_eq!(ok, 4);
    // Well below capacity it fails.
    assert!(!turbo_frame(&mut rng, &mut t, -1.0, 8).0);
    for k in [40usize, 512, 1024] {
        let mut t = Turbo::new(k).unwrap();
        let ebn0 = if k == 40 { 3.0 } else { 1.0 };
        let ok = (0..10)
            .filter(|_| turbo_frame(&mut rng, &mut t, ebn0, 8).0)
            .count();
        assert!(ok >= 9, "K {k}: {ok}/10");
    }
}

#[test]
fn turbo_rate_matching_every_rv_with_fillers_and_combining() {
    let mut rng = Rng::new(2);
    let k = 1056;
    let filler = 16;
    let mut t = Turbo::new(k).unwrap();
    let rm = LteRateMatch::new(LteCode::Turbo, k + 4, filler).unwrap();
    let mut bits = rng.bits(k);
    bits[..filler].fill(0);
    let mut s = vec![0u8; 3 * (k + 4)];
    t.encode(&bits, &mut s).unwrap();
    let e = 1500; // rate about 0.7 per transmission
    let mut soft = vec![0i8; 3 * (k + 4)];
    let mut decoded_after = None;
    for (n, rv) in [0u8, 2, 3, 1].into_iter().enumerate() {
        let mut tx = vec![0u8; e];
        rm.rate_match(&s, rv, rm.buffer_len(), &mut tx).unwrap();
        let llr = bpsk_llr8(&mut rng, &tx, k as f64 / e as f64, -0.5, 12.0);
        rm.dematch(&llr, rv, rm.buffer_len(), &mut soft).unwrap();
        // Filler bits are known zeros.
        for v in &mut soft[..filler] {
            *v = 127;
        }
        let mut out = vec![0u8; k];
        if t.decode(&soft, 8, |b| b == &bits[..], &mut out).is_ok() {
            decoded_after = Some(n + 1);
            break;
        }
    }
    let n = decoded_after.expect("decodes after combining");
    assert!(
        n >= 2,
        "a single weak transmission should not be enough ({n})"
    );
    assert!(LteRateMatch::new(LteCode::Turbo, 0, 0).is_err());
    assert!(rm
        .rate_match(&s, 4, rm.buffer_len(), &mut [0u8; 10])
        .is_err());
}

#[test]
fn lte_pbch_chain_tail_biting_with_rate_matching() {
    let mut rng = Rng::new(3);
    let rm = LteRateMatch::new(LteCode::Convolutional, 40, 0).unwrap();
    let mut v = Viterbi::new(codes::LTE_K7_R3, 40).unwrap();
    for e in [1920usize, 1728, 480] {
        for _ in 0..10 {
            let bits = rng.bits(40);
            let mut coded = [0u8; 120];
            encode_tail_biting(&codes::LTE_K7_R3, &bits, &mut coded).unwrap();
            // Rate matching works on the streams d0 | d1 | d2, the coded output is interleaved per bit.
            let mut streams = [0u8; 120];
            for i in 0..40 {
                for s in 0..3 {
                    streams[s * 40 + i] = coded[3 * i + s];
                }
            }
            let mut tx = vec![0u8; e];
            rm.rate_match(&streams, 0, rm.buffer_len(), &mut tx)
                .unwrap();
            let llr = bpsk_llr8(&mut rng, &tx, 40.0 / e as f64, 3.0, 10.0);
            let mut soft = [0i8; 120];
            rm.dematch(&llr, 0, rm.buffer_len(), &mut soft).unwrap();
            let mut inter = [0i8; 120];
            for i in 0..40 {
                for s in 0..3 {
                    inter[3 * i + s] = soft[s * 40 + i];
                }
            }
            let mut out = [0u8; 40];
            v.decode(&inter, Ends::TailBiting, &mut out).unwrap();
            assert_eq!(&out[..], &bits[..], "E {e}");
        }
    }
}

fn with_crc(rng: &mut Rng, k: usize) -> Vec<u8> {
    let mut b = rng.bits(k - 24);
    let c = CRC24_NR_C.bits(&b);
    for i in (0..24).rev() {
        b.push((c >> i) as u8 & 1);
    }
    b
}

fn crc_ok(b: &[u8]) -> bool {
    CRC24_NR_C.bits(b) == 0
}

#[test]
fn nr_pbch_polar_decodes_at_0_db() {
    let mut rng = Rng::new(4);
    let mut p = Polar::new(PolarParams::PBCH, 8).unwrap();
    assert_eq!(p.n(), 512); // TS 38.212: the PBCH mother code has N = 512
    assert_eq!(p.info_positions().len(), 56);
    let mut ok = 0;
    for _ in 0..20 {
        let bits = with_crc(&mut rng, 56);
        let mut e = [0u8; 864];
        p.encode(&bits, &mut e).unwrap();
        let llr = bpsk_llr(&mut rng, &e, 56.0 / 864.0, 0.0);
        let mut out = [0u8; 56];
        if p.decode(&llr, crc_ok, &mut out).is_ok() && out[..] == bits[..] {
            ok += 1;
        }
    }
    assert!(ok >= 19, "{ok}/20");
}

#[test]
fn polar_puncturing_shortening_repetition() {
    let mut rng = Rng::new(5);
    // K/E <= 7/16: puncturing; K/E > 7/16 with E < N: shortening; E > N: repetition.
    for (k, e, ebn0) in [
        (40usize, 108usize, 3.0),
        (64, 100, 4.0),
        (30, 1000, 0.0),
        (140, 400, 3.0),
        (164, 216, 5.0),
    ] {
        let mut p = Polar::new(PolarParams::downlink(k, e), 8).unwrap();
        let mut ok = 0;
        for _ in 0..10 {
            let bits = with_crc(&mut rng, k);
            let mut tx = vec![0u8; e];
            p.encode(&bits, &mut tx).unwrap();
            let llr = bpsk_llr(&mut rng, &tx, k as f64 / e as f64, ebn0);
            let mut out = vec![0u8; k];
            if p.decode(&llr, crc_ok, &mut out).is_ok() && out == bits {
                ok += 1;
            }
        }
        assert!(ok >= 9, "K {k} E {e}: {ok}/10");
    }
    assert!(Polar::new(PolarParams::downlink(200, 400), 8).is_err());
    assert!(Polar::new(PolarParams::downlink(40, 30), 8).is_err());
}
