//! Viterbi decoder: known answers from the standards, every end mode, streaming against whole frames, scalar against vector
//! kernels, and the bit-error rate of the K = 7 rate 1/2 code against the textbook curve.

mod common;
use common::*;
use sdrkit_algo_fec::conv::{
    check_backend, codes, depuncture_frame, encode_tail_biting, puncture, ConvCode, Depuncturer,
    Encoder, Ends, Kernel, Viterbi, BACKENDS,
};

/// Galileo OS SIS ICD issue 2.1 annex D.2: the I/NAV numerical example (120 input bits, the last 6 tail zeros) and its 240
/// encoded symbols, as printed.
const GAL_INAV_IN: &str =
    "11111111 11110000 11001100 10101010 00000000 00001111 00110011 01010101 \
    11100011 11101100 11011111 10001010 00011100 00010011 01000000";
const GAL_INAV_OUT: &str =
    "10001100 00011010 10101010 01110011 00110001 01011010 01101111 01011001 \
    01111000 10010101 01010101 10001100 11001110 10100101 10010000 10100110 \
    10000100 00000010 00000010 00011011 10011001 11101011 01011100 00011000 \
    10111011 11111101 11111101 11100100 01010011 00100010";
/// Annex D.1: the F/NAV example (244 input bits, 488 symbols).
const GAL_FNAV_IN: &str =
    "11111111 11110000 11001100 10101010 00000000 00001111 00110011 01010101 \
    11100011 11101100 11011111 10001010 00011100 00010011 00100000 01110101 \
    01010101 01100001 01100010 01100011 10101010 10011110 10011101 10011100 \
    00011100 00100011 00001101 00111001 11011100 11101100 0000";
const GAL_FNAV_OUT: &str =
    "10001100 00011010 10101010 01110011 00110001 01011010 01101111 01011001 \
    01111000 10010101 01010101 10001100 11001110 10100101 10010000 10100110 \
    10000100 00000010 00000010 00011011 10011001 11101011 01011100 00011000 \
    10111011 11111101 11111101 11100100 01100110 00010100 10100011 11100111 \
    10100110 01100110 01101011 00010000 00011100 00011101 01010001 10101110 \
    00111001 11101001 10010100 11101111 11100011 11100010 10101110 01010001 \
    11111101 00111101 11110000 10101001 11011000 00110010 00111111 10000100 \
    10010010 10010110 00100100 10101011 01001110";

#[test]
fn galileo_annex_d_encoder_and_decoder() {
    for (input, output) in [(GAL_INAV_IN, GAL_INAV_OUT), (GAL_FNAV_IN, GAL_FNAV_OUT)] {
        let bits = bits_of(input);
        let want = bits_of(output);
        assert_eq!(want.len(), 2 * bits.len());
        let mut coded = Vec::new();
        Encoder::new(codes::GALILEO_K7).encode(&bits, &mut coded);
        assert_eq!(coded, want);
        // Decode the printed symbols with 12 of them wrong; the tail is part of the input.
        let mut llr = hard_llr(&want, 30);
        for i in (5..llr.len()).step_by(41) {
            llr[i] = -llr[i];
        }
        let mut v = Viterbi::new(codes::GALILEO_K7, 512).unwrap();
        let info = bits.len() - 6;
        let mut out = vec![0u8; info];
        let r = v.decode(&llr, Ends::Zero, &mut out).unwrap();
        assert_eq!(out, &bits[..info]);
        assert_eq!(r.corrected as usize, (llr.len() - 5).div_ceil(41));
    }
}

/// Every kernel available here, for codes it can run.
fn kernels(code: &ConvCode) -> Vec<Kernel> {
    BACKENDS
        .all()
        .iter()
        .map(|&b| Kernel::Butterfly(b))
        .chain([Kernel::Generic32])
        .filter(|&k| Viterbi::with_kernel(*code, 16, k).is_ok())
        .collect()
}

#[test]
fn every_backend_agrees_with_the_plain_one() {
    for &b in BACKENDS.all() {
        if b.available() {
            check_backend(b);
        }
    }
}

#[test]
fn every_kernel_returns_the_same_bits() {
    let mut rng = Rng::new(7);
    let codes_to_test = [
        codes::DVB_K7,
        codes::CCSDS_K7,
        codes::WIFI_K7,
        codes::LTE_K7_R3,
        codes::DAB_K7_R4,
        codes::DRM_K7_R6,
        codes::GSM_K5,
        codes::TETRA_K5_R4,
        codes::HD_AM_E1,
        ConvCode::new(6, &[0o65, 0o57]),
        ConvCode::new(8, &[0o247, 0o371]),
    ];
    for code in codes_to_test {
        let ks = kernels(&code);
        assert!(ks.contains(&Kernel::Generic32));
        assert!(ks.len() >= 2, "{code:?}: {ks:?}");
        for trial in 0..4 {
            let bits = rng.bits(700);
            let mut coded = Vec::new();
            Encoder::new(code).encode(&bits, &mut coded);
            // Very noisy, so that many decisions are close calls.
            let ebn0 = [-1.0, 1.0, 2.0, 4.0][trial];
            let llr = bpsk_llr8(&mut rng, &coded, 1.0 / code.n() as f64, ebn0, 25.0);
            let mut first: Option<(Vec<u8>, i32, f32)> = None;
            for &k in &ks {
                for ends in [Ends::Open, Ends::Start(0), Ends::TailBiting] {
                    let mut v = Viterbi::with_kernel(code, 700, k).unwrap();
                    let mut out = vec![0u8; 700];
                    let r = v.decode(&llr, ends, &mut out).unwrap();
                    if ends == Ends::Open {
                        match &first {
                            None => first = Some((out.clone(), r.metric, v.mean_gain())),
                            Some((o, m, g)) => {
                                assert_eq!(&out, o, "{code:?} {k:?}");
                                assert_eq!(r.metric, *m, "{code:?} {k:?}");
                                assert_eq!(v.mean_gain(), *g);
                            }
                        }
                    }
                }
                // Streaming too.
                let mut v = Viterbi::with_kernel(code, 0, k).unwrap();
                let mut s = Vec::new();
                v.push(&llr, &mut s);
                v.flush(&mut s);
                assert_eq!(s.len(), 700);
            }
        }
    }
}

#[test]
fn streaming_kernels_agree_and_any_cut_returns_the_same_bits() {
    let mut rng = Rng::new(99);
    let code = codes::DVB_K7;
    let bits = rng.bits(3000);
    let mut coded = Vec::new();
    Encoder::new(code).encode(&bits, &mut coded);
    let llr = bpsk_llr8(&mut rng, &coded, 0.5, 2.5, 25.0);
    let mut whole: Option<Vec<u8>> = None;
    for k in kernels(&code) {
        let mut v = Viterbi::with_kernel(code, 0, k).unwrap();
        let mut out = Vec::new();
        v.push(&llr, &mut out);
        v.flush(&mut out);
        match &whole {
            None => whole = Some(out),
            Some(w) => assert_eq!(&out, w, "{k:?}"),
        }
    }
    let whole = whole.unwrap();
    assert!(errors(&whole, &bits) < 15);
    let mut v = Viterbi::new(code, 0).unwrap();
    for cut in [1usize, 7, 100, 191, 192, 193, 1999, 4321, 5999] {
        v.reset();
        let mut out = Vec::new();
        v.push(&llr[..cut], &mut out);
        v.push(&llr[cut..], &mut out);
        v.flush(&mut out);
        assert_eq!(out, whole, "cut {cut}");
    }
    // Tiny pieces.
    v.reset();
    let mut out = Vec::new();
    for c in llr.chunks(3) {
        v.push(c, &mut out);
    }
    v.flush(&mut out);
    assert_eq!(out, whole);
}

#[test]
fn every_end_mode_corrects_errors() {
    let mut rng = Rng::new(3);
    for code in [
        codes::DVB_K7,
        codes::LTE_K7_R3,
        codes::GSM_K5,
        codes::DSTAR_K3,
        codes::THOR_K15,
        codes::HD_AM_E2,
    ] {
        let k = code.k() as usize;
        let n = code.n();
        let bits = rng.bits(300);
        // Terminated.
        let mut enc = Encoder::new(code);
        let mut coded = Vec::new();
        enc.encode(&bits, &mut coded);
        enc.terminate(&mut coded);
        let llr = bpsk_llr8(&mut rng, &coded, 1.0 / n as f64, 4.0, 25.0);
        let mut v = Viterbi::new(code, 400).unwrap();
        let mut out = vec![0u8; 300];
        v.decode(&llr, Ends::Zero, &mut out).unwrap();
        assert_eq!(out, bits, "zero {code:?}");
        // Known start, open end: the last bits are weak, so compare all but the last 3K.
        let mut coded = Vec::new();
        Encoder::with_state(code, 5).encode(&bits, &mut coded);
        let llr = bpsk_llr8(&mut rng, &coded, 1.0 / n as f64, 5.0, 25.0);
        v.decode(&llr, Ends::Start(5), &mut out).unwrap();
        assert_eq!(out[..300 - 3 * k], bits[..300 - 3 * k], "start {code:?}");
        // Tail-biting.
        let mut coded = vec![0u8; 300 * n];
        encode_tail_biting(&code, &bits, &mut coded).unwrap();
        let llr = bpsk_llr8(&mut rng, &coded, 1.0 / n as f64, 4.0, 25.0);
        let r = v.decode(&llr, Ends::TailBiting, &mut out).unwrap();
        assert_eq!(out, bits, "tail-biting {code:?}");
        assert!(r.iterations <= 13);
    }
}

#[test]
fn tail_biting_short_lte_frames_with_flips() {
    // LTE PBCH-sized frames (40 bits), 5 % of the code bits wrong.
    let mut rng = Rng::new(11);
    let mut v = Viterbi::new(codes::LTE_K7_R3, 64).unwrap();
    for _ in 0..50 {
        let bits = rng.bits(40);
        let mut coded = [0u8; 120];
        encode_tail_biting(&codes::LTE_K7_R3, &bits, &mut coded).unwrap();
        let mut llr = hard_llr(&coded, 20);
        for _ in 0..6 {
            let i = rng.below(120);
            llr[i] = -llr[i];
        }
        let mut out = [0u8; 40];
        v.decode(&llr, Ends::TailBiting, &mut out).unwrap();
        assert_eq!(&out[..], &bits[..]);
    }
}

#[test]
fn punctured_rates_decode_through_noise() {
    let mut rng = Rng::new(21);
    for (p, rate, ebn0) in [
        (codes::DVB_2_3, 2.0 / 3.0, 5.0),
        (codes::DVB_3_4, 0.75, 5.5),
        (codes::DVB_5_6, 5.0 / 6.0, 6.5),
        (codes::DVB_7_8, 7.0 / 8.0, 7.0),
    ] {
        let bits = rng.bits(4000);
        let mut enc = Encoder::new(codes::DVB_K7);
        let mut coded = Vec::new();
        enc.encode(&bits, &mut coded);
        enc.terminate(&mut coded);
        let mut sent = Vec::new();
        puncture(&p, 0, &coded, &mut sent);
        let llr = bpsk_llr8(&mut rng, &sent, rate, ebn0, 25.0);
        let mut full = vec![0i8; coded.len()];
        depuncture_frame(&p, 0, &llr, &mut full).unwrap();
        let mut v = Viterbi::new(codes::DVB_K7, 5000).unwrap();
        v.set_traceback(160, 96).unwrap();
        let mut out = vec![0u8; 4000];
        v.decode(&full, Ends::Zero, &mut out).unwrap();
        assert!(errors(&out, &bits) <= 2, "{p:?}: {}", errors(&out, &bits));
        // The streaming path with the depuncturer.
        let mut d = Depuncturer::new(p);
        let mut stream = Vec::new();
        d.push(&llr, &mut stream);
        v.reset_to(0);
        let mut s = Vec::new();
        v.push(&stream, &mut s);
        v.flush_terminated(&mut s);
        assert_eq!(s.len(), 4000);
        assert!(
            errors(&s, &bits) <= 4,
            "{p:?} stream: {}",
            errors(&s, &bits)
        );
    }
}

#[test]
fn mean_gain_separates_right_from_wrong_hypotheses() {
    let mut rng = Rng::new(5);
    let bits = rng.bits(2000);
    let mut coded = Vec::new();
    Encoder::new(codes::DVB_K7).encode(&bits, &mut coded);
    let llr = bpsk_llr8(&mut rng, &coded, 0.5, 6.0, 25.0);
    let mut v = Viterbi::new(codes::DVB_K7, 0).unwrap();
    let mut out = Vec::new();
    v.push(&llr, &mut out);
    let right = v.mean_gain();
    // Wrong pairing (off by one code bit) and inverted polarity of one stream.
    v.reset();
    v.push(&llr[1..], &mut out);
    let shifted = v.mean_gain();
    v.reset();
    let flipped: Vec<i8> = llr
        .iter()
        .enumerate()
        .map(|(i, &x)| if i % 2 == 1 { -x } else { x })
        .collect();
    v.push(&flipped, &mut out);
    let inverted = v.mean_gain();
    eprintln!("mean gain: right {right}, shifted {shifted}, inverted {inverted}");
    assert!(
        right > 1.2 * shifted && right > 1.2 * inverted,
        "{right} {shifted} {inverted}"
    );
}

#[test]
fn bad_input_returns_errors() {
    let mut v = Viterbi::new(codes::DVB_K7, 10).unwrap();
    let mut out = [0u8; 20];
    assert!(v.decode(&[0i8; 3], Ends::Open, &mut out).is_err());
    assert!(v.decode(&[0i8; 40], Ends::Open, &mut out).is_err());
    assert!(v.decode(&[0i8; 8], Ends::Zero, &mut out).is_err());
    assert!(v.decode(&[0i8; 20], Ends::Open, &mut out[..5]).is_err());
    assert!(Viterbi::new(codes::WSPR_K32, 10).is_err());
    let plain = Kernel::Butterfly(BACKENDS.plain());
    assert!(Viterbi::with_kernel(codes::DSTAR_K3, 10, plain).is_err());
    assert!(v.set_traceback(0, 10).is_err());
    // Empty frame.
    assert_eq!(v.decode(&[], Ends::Open, &mut out).unwrap().corrected, 0);
}

/// BER of the K = 7 rate 1/2 code with soft decisions (8-bit LLRs). Textbook figure (e.g. J. Proakis, *Digital Communications*,
/// 5th ed., figure 8.2-21; Heller and Jacobs, IEEE Trans. Commun. 1971): about 1e-5 near Eb/N0 = 4.4 dB and about 1e-3 near
/// 3 dB with soft decisions.
#[test]
fn k7_rate_half_ber_matches_the_textbook_curve() {
    let mut rng = Rng::new(2024);
    let code = codes::DVB_K7;
    let frame = 20_000;
    let mut v = Viterbi::new(code, frame + 6).unwrap();
    let mut out = vec![0u8; frame];
    let mut ber = |ebn0: f64, frames: usize| -> f64 {
        let mut errs = 0usize;
        for _ in 0..frames {
            let bits = rng.bits(frame);
            let mut enc = Encoder::new(code);
            let mut coded = Vec::with_capacity(2 * frame + 12);
            enc.encode(&bits, &mut coded);
            enc.terminate(&mut coded);
            let llr = bpsk_llr8(&mut rng, &coded, 0.5, ebn0, 25.0);
            v.decode(&llr, Ends::Zero, &mut out).unwrap();
            errs += errors(&out, &bits);
        }
        errs as f64 / (frames * frame) as f64
    };
    let b3 = ber(3.0, 10);
    let b44 = ber(4.4, 60);
    eprintln!("BER at 3.0 dB: {b3:.2e}, at 4.4 dB: {b44:.2e}");
    assert!((5e-5..2e-3).contains(&b3), "{b3}");
    assert!((3e-7..2e-5).contains(&b44), "{b44}");
}

#[test]
fn last_gain_is_the_growth_of_each_pushed_step() {
    let mut rng = Rng::new(9);
    let bits = rng.bits(400);
    let mut coded = Vec::new();
    Encoder::new(codes::INMARSAT_K7).encode(&bits, &mut coded);
    // Clean hard values of size 40: once the paths agree, the best metric grows by 2 x 40 each step.
    let llr: Vec<i8> = coded
        .iter()
        .map(|&b| if b == 0 { 40 } else { -40 })
        .collect();
    let mut v = Viterbi::new(codes::INMARSAT_K7, 0).unwrap();
    assert_eq!(v.last_gain(), 0.0);
    let mut out = Vec::new();
    let mut total = 0f64;
    for (i, step) in llr.chunks(2).enumerate() {
        v.push(step, &mut out);
        total += v.last_gain() as f64;
        if i > 20 {
            assert_eq!(v.last_gain(), 80.0, "step {i}");
        }
    }
    // The per-step values add up to the mean over the stream.
    assert!((total / v.steps() as f64 - v.mean_gain() as f64).abs() < 1e-3);
    // A push of half a step decodes no step and keeps the value; after a bigger push the value is the mean per step of that push.
    v.push(&llr[..1], &mut out);
    assert_eq!(v.last_gain(), 80.0);
    v.reset();
    assert_eq!(v.last_gain(), 0.0);
    let noise: Vec<i8> = (0..200).map(|_| (rng.gauss() * 40.0) as i8).collect();
    v.push(&noise, &mut out);
    assert!(v.last_gain() < 60.0, "{}", v.last_gain());
}
