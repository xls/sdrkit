//! The hypothesis search and the parallel helpers.

mod common;
use common::*;
use sdrkit_algo_fec::conv::{codes, puncture, Depuncturer, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::ldpc::{tables::dvb_s2, LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::par::{for_each_frame, viterbi_chunks};
use sdrkit_algo_fec::search::{best_of, viterbi_error_rate};

#[test]
fn puncturing_phase_search_finds_the_right_phase() {
    let mut rng = Rng::new(1);
    let bits = rng.bits(3000);
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let mut sent = Vec::new();
    puncture(&codes::DVB_3_4, 0, &coded, &mut sent);
    let llr = bpsk_llr8(&mut rng, &sent, 0.75, 5.0, 25.0);
    // The receiver joins the stream 2 values in: the right depuncturing phase is the pattern position of value 2.
    let window = &llr[2..2 + 600];
    let mut v = Viterbi::new(codes::DVB_K7, 1000).unwrap();
    let mut out = vec![0u8; 1000];
    let mut full = Vec::new();
    let r = best_of(4, |k| {
        // Hypothesis k: the window starts at kept index k of the period (0, 1, 3, 4 in the serial pattern).
        let phase = [0usize, 1, 3, 4][k];
        let mut d = Depuncturer::with_phase(codes::DVB_3_4, phase);
        full.clear();
        if phase % 2 == 1 {
            full.push(0); // pad to a step boundary
        }
        d.push(window, &mut full);
        viterbi_error_rate(&mut v, &full, &mut out).ok()
    })
    .unwrap();
    assert_eq!(r.best, 2, "{r:?}");
    // The wrong phases decode to valid-looking paths of a related code, so the margin is a few percent of the values.
    assert!(r.score < 0.05 && r.margin > 0.03, "{r:?}");
}

#[test]
fn chunked_viterbi_equals_one_long_decode() {
    let mut rng = Rng::new(2);
    let bits = rng.bits(20_000);
    let mut coded = Vec::new();
    Encoder::new(codes::CCSDS_K7).encode(&bits, &mut coded);
    let llr = bpsk_llr8(&mut rng, &coded, 0.5, 4.0, 25.0);
    let mut one = Viterbi::new(codes::CCSDS_K7, 20_000).unwrap();
    let mut whole = vec![0u8; 20_000];
    one.decode(&llr, Ends::Open, &mut whole).unwrap();
    let mut decs: Vec<Viterbi> = (0..4)
        .map(|_| Viterbi::new(codes::CCSDS_K7, 3000 + 200).unwrap())
        .collect();
    let mut out = vec![0u8; 20_000];
    viterbi_chunks(&mut decs, &llr, 3000, 100, &mut out).unwrap();
    assert!(errors(&out, &whole) <= 2, "{}", errors(&out, &whole));
    assert!(errors(&out[50..], &bits[50..]) <= 5);
}

#[test]
fn frames_decode_on_several_threads() {
    let mut rng = Rng::new(3);
    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let mut frames: Vec<(Vec<i8>, Vec<u8>, Vec<u8>)> = (0..8)
        .map(|_| {
            let info = rng.bits(code.k());
            let mut cw = vec![0u8; code.n()];
            code.encode(&info, &mut cw).unwrap();
            let llr = bpsk_llr8(&mut rng, &cw, 0.44, 3.5, 16.0);
            (llr, cw, vec![0u8; code.n()])
        })
        .collect();
    let mut decs: Vec<LdpcDecoder> = (0..3)
        .map(|_| LdpcDecoder::new(&code, MinSum::default()))
        .collect();
    for_each_frame(&mut decs, &mut frames, |d, _, (llr, _, out)| {
        d.decode(llr, 50, out).map(|_| ())
    })
    .unwrap();
    for (_, cw, out) in &frames {
        assert_eq!(out, cw);
    }
}
