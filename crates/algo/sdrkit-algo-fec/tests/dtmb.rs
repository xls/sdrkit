//! The codes of DTMB (GB 20600-2006): the three LDPC codes, BCH(762, 752), the Nordstrom-Robinson code and the frequency
//! interleaver's digit reversal, with the acceptance numbers and worked values of the DTMB frame specification.

mod common;
use common::*;
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::digitrev::digit_reverse;
use sdrkit_algo_fec::ldpc::{tables::dtmb, InfoAt, LdpcCode, LdpcDecoder, LdpcEncoder};
use sdrkit_algo_fec::nordstrom;
use sdrkit_algo_fec::Quantizer;

/// The block rows of a code.
type Rows = &'static [&'static [(u16, u16)]];

/// Per rate: the rows, block rows, information bits and edges.
const RATES: [(Rows, usize, usize, usize); 3] = [
    (dtmb::RATE_0_4, 35, 3048, 34_925),
    (dtmb::RATE_0_6, 23, 4572, 37_592),
    (dtmb::RATE_0_8, 11, 6096, 37_338),
];

use dtmb::RULE;

fn code(rows: &[&[(u16, u16)]]) -> LdpcCode {
    LdpcCode::dtmb(rows, InfoAt::End).unwrap()
}

#[test]
fn ldpc_tables_have_their_structure() {
    for (rows, r, k, edges) in RATES {
        let c = code(rows);
        assert_eq!((rows.len(), c.n(), c.k(), c.edges()), (r, 7493, k, edges));
        // No 4-cycles: two block rows that share two column groups differ in their shift differences.
        for a in 0..rows.len() {
            for b in a + 1..rows.len() {
                for &(c1, s1) in rows[a] {
                    for &(c2, s2) in rows[a] {
                        if c2 <= c1 {
                            continue;
                        }
                        let t1 = rows[b].iter().find(|e| e.0 == c1);
                        let t2 = rows[b].iter().find(|e| e.0 == c2);
                        if let (Some(&(_, u1)), Some(&(_, u2))) = (t1, t2) {
                            let d = (s1 as i32 - s2 as i32 - u1 as i32 + u2 as i32).rem_euclid(127);
                            assert_ne!(d, 0, "4-cycle in rows {a}, {b} at groups {c1}, {c2}");
                        }
                    }
                }
            }
        }
    }
}

fn encoder_and_words(
    rows: &[&[(u16, u16)]],
    k: usize,
    count: usize,
    seed: u64,
) -> (LdpcCode, Vec<Vec<u8>>) {
    let c = code(rows);
    let enc = LdpcEncoder::new(&c).unwrap();
    let mut rng = Rng::new(seed);
    let words = (0..count)
        .map(|i| {
            let info: Vec<u8> = match i {
                0 => vec![0; k],
                1 => vec![1; k],
                _ => rng.bits(k),
            };
            let mut cw = vec![0u8; 7493];
            enc.encode(&info, &mut cw).unwrap();
            assert_eq!(
                &cw[7493 - k..],
                &info[..],
                "the information bits follow the parity"
            );
            cw
        })
        .collect();
    (c, words)
}

#[test]
fn ldpc_encoder_meets_every_check() {
    for (rows, _, k, _) in RATES {
        let (c, words) = encoder_and_words(rows, k, 20, 7);
        for w in &words {
            assert!(c.is_codeword(w));
        }
    }
}

/// The LLRs of the 7493 code bits of `cw` sent as BPSK at `ebn0_db` (information bits over the 7488 sent bits), quantised;
/// the five unsent bits get 0.
fn received(rng: &mut Rng, cw: &[u8], k: usize, ebn0_db: f64) -> Vec<i8> {
    let rate = k as f64 / 7488.0;
    let mut f = vec![0f32; 7493];
    f[5..].copy_from_slice(&bpsk_llr(rng, &cw[5..], rate, ebn0_db));
    let mut q = vec![0i8; 7493];
    Quantizer::default().quantize_auto(&f, &mut q).unwrap();
    q
}

#[test]
fn ldpc_decoder_meets_the_thresholds() {
    // (rate index, Eb/N0 dB, words, least and most failures).
    let cases = [
        (0, 1.4, 40, 30, 40),
        (0, 2.4, 400, 0, 0),
        (0, 3.4, 1500, 0, 0),
        (1, 1.4, 40, 30, 40),
        (1, 2.6, 400, 0, 0),
        (1, 3.5, 1500, 0, 0),
        (2, 2.4, 40, 30, 40),
        (2, 3.6, 400, 0, 0),
        (2, 4.5, 1500, 0, 0),
    ];
    for (ri, ebn0, count, lo, hi) in cases {
        let (rows, _, k, _) = RATES[ri];
        let (c, words) = encoder_and_words(rows, k, 8, 11);
        let mut dec = LdpcDecoder::new(&c, RULE);
        let mut rng = Rng::new(100 + ri as u64);
        let mut out = vec![0u8; 7493];
        let mut fails = 0;
        for i in 0..count {
            let cw = &words[i % words.len()];
            let llr = received(&mut rng, cw, k, ebn0);
            match dec.decode(&llr, 100, &mut out) {
                Ok(_) => assert_eq!(&out, cw, "an accepted word is the sent one"),
                Err(_) => fails += 1,
            }
        }
        assert!(
            (lo..=hi).contains(&fails),
            "rate {ri} at {ebn0} dB: {fails} of {count} failed"
        );
    }
}

#[test]
fn ldpc_decoder_accepts_only_the_sent_word() {
    for (rows, _, k, _) in RATES {
        let (c, words) = encoder_and_words(rows, k, 30, 3);
        let mut dec = LdpcDecoder::new(&c, RULE);
        let mut out = vec![0u8; 7493];
        // Clean input: one iteration.
        let mut llr: Vec<i8> = words[2]
            .iter()
            .map(|&b| if b == 0 { 6 } else { -6 })
            .collect();
        llr[..5].fill(0);
        let d = dec.decode(&llr, 60, &mut out).unwrap();
        assert_eq!((d.iterations, &out), (1, &words[2]));
        // Two values turned weakly the wrong way.
        llr[100] = if words[2][100] == 0 { -1 } else { 1 };
        llr[3000] = if words[2][3000] == 0 { -1 } else { 1 };
        dec.decode(&llr, 60, &mut out).unwrap();
        assert_eq!(out, words[2]);
        // 30 words at noise sigma 0.25 to 0.83.
        let mut rng = Rng::new(9);
        for (i, w) in words.iter().enumerate() {
            let sigma = 0.25 + 0.02 * i as f64;
            let mut f = vec![0f32; 7493];
            for j in 5..7493 {
                let x = if w[j] == 0 { 1.0 } else { -1.0 } + sigma * rng.gauss();
                f[j] = (2.0 * x / (sigma * sigma)) as f32;
            }
            let mut q = vec![0i8; 7493];
            Quantizer::default().quantize_auto(&f, &mut q).unwrap();
            if let Ok(d) = dec.decode(&q, 60, &mut out) {
                assert_eq!(&out, w, "sigma {sigma}");
                assert!(d.iterations <= 60);
            }
        }
        // Random LLRs are not a code word.
        let f: Vec<f32> = (0..7493).map(|_| rng.gauss() as f32).collect();
        let mut q = vec![0i8; 7493];
        Quantizer::default().quantize_auto(&f, &mut q).unwrap();
        assert!(dec.decode(&q, 60, &mut out).is_err());
    }
}

fn bch() -> Bch {
    Bch::from_code(&dtmb::BCH).unwrap()
}

#[test]
fn bch_762_752_parity_and_correction() {
    let mut b = bch();
    assert_eq!(b.k(), 752);
    let parity = |msg: &[u8]| {
        let mut w = msg.to_vec();
        w.resize(762, 0);
        bch().encode(&mut w).unwrap();
        w[752..].to_vec()
    };
    let mut m = vec![0u8; 752];
    m[751] = 1;
    assert_eq!(parity(&m), bits_of("0000001001"));
    let mut m = vec![0u8; 752];
    m[0] = 1;
    assert_eq!(parity(&m), bits_of("0011011001"));
    assert_eq!(parity(&[1u8; 752]), bits_of("1101101110"));
    let mut rng = Rng::new(5);
    for _ in 0..200 {
        let mut w = rng.bits(752);
        w.resize(762, 0);
        b.encode(&mut w).unwrap();
        let sent = w.clone();
        let mut x = w.clone();
        assert_eq!(b.decode(&mut x).unwrap().corrected, 0);
        let i = rng.below(762);
        x[i] ^= 1;
        assert_eq!(b.decode(&mut x).unwrap().corrected, 1);
        assert_eq!(x, sent);
        let j = (i + 1 + rng.below(761)) % 762;
        x[i] ^= 1;
        x[j] ^= 1;
        assert!(!matches!(b.decode(&mut x), Ok(d) if d.corrected == 0));
    }
}

#[test]
fn nordstrom_robinson_words_and_soft_decoding() {
    use std::collections::HashSet;
    let words: HashSet<u16> = (0..=255u8).map(nordstrom::encode).collect();
    assert_eq!(words.len(), 256);
    let mut weights = [0usize; 17];
    for w in &words {
        weights[w.count_ones() as usize] += 1;
    }
    assert_eq!(
        (weights[0], weights[6], weights[8], weights[10], weights[16]),
        (1, 112, 30, 112, 1)
    );
    assert_eq!(weights.iter().sum::<usize>(), 256);
    let min = words
        .iter()
        .flat_map(|a| {
            words
                .iter()
                .filter(move |b| *b != a)
                .map(move |b| (a ^ b).count_ones())
        })
        .min();
    assert_eq!(min, Some(6));
    let p = |x: usize| nordstrom::PARITY[x];
    assert_eq!(
        (p(0x01), p(0x02), p(0xFF), p(0xFD)),
        (0xFE, 0x97, 0xFF, 0x68)
    );
    let mut rng = Rng::new(21);
    for _ in 0..400 {
        let x = rng.below(256) as u8;
        let mut bits = [0u8; 16];
        nordstrom::encode_bits(x, &mut bits);
        let mut llr = [0f32; 16];
        for (l, &b) in llr.iter_mut().zip(&bits) {
            *l = if b == 0 { 3.0 } else { -3.0 };
        }
        let i = rng.below(16);
        let j = (i + 1 + rng.below(15)) % 16;
        llr[i] = -llr[i];
        llr[j] = -llr[j];
        let mut out = [0f32; 8];
        nordstrom::decode_soft(&llr, &mut out);
        let got = out.iter().fold(0u8, |a, &l| (a << 1) | (l < 0.0) as u8);
        assert_eq!(got, x);
        bits[i] ^= 1;
        bits[j] ^= 1;
        assert_eq!(nordstrom::decode_hard(&bits), (x, 2));
    }
}

#[test]
fn digit_reversal_of_3780_carriers() {
    let p = digit_reverse(&[3, 3, 3, 2, 2, 5, 7]).unwrap();
    assert_eq!(p.len(), 3780);
    let first: Vec<usize> = (0..40).map(|l| p.dest(l).unwrap()).collect();
    assert_eq!(
        first,
        [
            0, 540, 1080, 1620, 2160, 2700, 3240, 108, 648, 1188, 1728, 2268, 2808, 3348, 216, 756,
            1296, 1836, 2376, 2916, 3456, 324, 864, 1404, 1944, 2484, 3024, 3564, 432, 972, 1512,
            2052, 2592, 3132, 3672, 54, 594, 1134, 1674, 2214
        ]
    );
    let last: Vec<usize> = (3740..3780).map(|l| p.dest(l).unwrap()).collect();
    assert_eq!(
        last,
        [
            1565, 2105, 2645, 3185, 3725, 107, 647, 1187, 1727, 2267, 2807, 3347, 215, 755, 1295,
            1835, 2375, 2915, 3455, 323, 863, 1403, 1943, 2483, 3023, 3563, 431, 971, 1511, 2051,
            2591, 3131, 3671, 539, 1079, 1619, 2159, 2699, 3239, 3779
        ]
    );
    assert!(digit_reverse(&[]).is_err());
    assert!(digit_reverse(&[3, 1]).is_err());
}
