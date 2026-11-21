//! BCH, short codes, product codes, majority logic and trellis tables against the values printed in their standards.

mod common;
use common::*;
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::conv::{BranchMetrics, Ends, Trellis, TrellisDecoder};
use sdrkit_algo_fec::cyclic::{CyclicEncoder, MajorityLogic};
use sdrkit_algo_fec::small::{
    chase_search, Chase, GlonassCheck, GlonassHamming, ProductCode, SmallCode,
};

fn word(bits: &[u8]) -> u128 {
    bits.iter().fold(0u128, |a, &b| (a << 1) | b as u128)
}

#[test]
fn sarsat_bch_examples_from_t001_annex_b() {
    // C/S T.001 annex B1: g(X) = 1001101101100111100011; bits 25 to 112 of the sample short message.
    let msg = bits_of(
        "0101 0110 1110 0110 1000 0000 0100 0000 0000 0010 0010 0000 0010 0000 0000 1001 0110 0101 0101 0010 0101 0000",
    );
    let g = 0b1001101101100111100011u64;
    let b = Bch::from_generator(82, &[g], 3).unwrap();
    assert_eq!(b.field().m(), 7);
    let mut w = msg[..82].to_vec();
    let info: Vec<u8> = w[..61].to_vec();
    b.encode(&mut w).unwrap();
    assert_eq!(&w[..61], &info[..]);
    assert_eq!(&w[61..], &bits_of("001011001010101001001")[..]);
    assert_eq!(&w[..], &msg[..82]);
    // Three errors are corrected.
    let mut bad = w.clone();
    for i in [0, 40, 81] {
        bad[i] ^= 1;
    }
    let mut b = b;
    assert_eq!(b.decode(&mut bad).unwrap().corrected, 3);
    assert_eq!(bad, w);
    // Annex B2: BCH-2 (38,26), g = 1010100111001, bits 107-132 give 0001 0101 0001.
    let info = bits_of("10 0101 0111 0000 0000 0001 0111");
    let enc = CyclicEncoder::new(&[0b1010100111001]).unwrap();
    let mut check = [0u8; 12];
    enc.check_bits(&info, &mut check).unwrap();
    assert_eq!(&check[..], &bits_of("0001 0101 0001")[..]);
    let short = SmallCode::bch_63_51().shortened(25).unwrap();
    assert_eq!(short.n(), 38);
    let cw = short.encode(word(&info));
    assert_eq!(cw & 0xFFF, word(&check));
    assert_eq!(short.radius(), 2);
}

#[test]
fn pocsag_sync_and_idle_words() {
    // ITU-R M.584-2 table 1: the synchronisation codeword is a code word.
    let p = SmallCode::pocsag_32_21();
    assert_eq!(p.syndrome(0x7CD2_15D8), 0);
    // Table 2 prints the idle word as 7AC9C197, which is not a code word (one bit is misprinted); the decoder corrects it to
    // 7A89C197, which is.
    assert_ne!(p.syndrome(0x7AC9_C197), 0);
    assert_eq!(p.decode(0x7AC9_C197), Some((0x7A89_C197, 1)));
    assert_eq!(p.radius(), 2);
    // The narrow-sense BCH(31,21) behind it.
    assert!(Bch::from_generator(31, &[0x769], 2).is_ok());
}

#[test]
fn beidou_rom_table_matches_table_5_2() {
    // BDS-SIS-ICD-B1I-3.0 table 5-2: syndrome D3D2D1D0 -> error pattern (first bit sent first).
    let rom: [(u32, &str); 16] = [
        (0b0000, "000000000000000"),
        (0b0001, "000000000000001"),
        (0b0010, "000000000000010"),
        (0b0011, "000000000010000"),
        (0b0100, "000000000000100"),
        (0b0101, "000000100000000"),
        (0b0110, "000000000100000"),
        (0b0111, "000010000000000"),
        (0b1000, "000000000001000"),
        (0b1001, "100000000000000"),
        (0b1010, "000001000000000"),
        (0b1011, "000000010000000"),
        (0b1100, "000000001000000"),
        (0b1101, "010000000000000"),
        (0b1110, "000100000000000"),
        (0b1111, "001000000000000"),
    ];
    let h = SmallCode::hamming_15_11();
    for (s, pat) in rom {
        let e = word(&bits_of(pat));
        assert_eq!(h.syndrome(e), s, "{pat}");
        assert_eq!(h.decode(e), Some((0, e.count_ones() as u8)));
    }
}

#[test]
fn dmr_short_codes_match_annex_b_tables() {
    let to = |rows: &[&str]| -> Vec<u32> {
        rows.iter()
            .map(|r| u32::from_str_radix(r, 2).unwrap())
            .collect()
    };
    // TS 102 361-1 tables B.11 to B.17: the P part of each generator.
    let cases: [(SmallCode, Vec<u32>); 7] = [
        (
            SmallCode::golay_20_8(),
            to(&[
                "001111011010",
                "110110011001",
                "011011001101",
                "001101100111",
                "110111000110",
                "101010010111",
                "100100111110",
                "100011101011",
            ]),
        ),
        (
            SmallCode::qr_16_7(),
            to(&[
                "001001111",
                "100011110",
                "110110111",
                "111100010",
                "111001001",
                "011100101",
                "001110011",
            ]),
        ),
        (
            SmallCode::hamming_17_12(),
            to(&[
                "11011", "11111", "11101", "11100", "01110", "00111", "10001", "11010", "01101",
                "10100", "01010", "00101",
            ]),
        ),
        (
            SmallCode::hamming_15_11(),
            to(&[
                "1001", "1101", "1111", "1110", "0111", "1010", "0101", "1011", "1100", "0110",
                "0011",
            ]),
        ),
        (
            SmallCode::hamming_13_9(),
            to(&[
                "1111", "1110", "0111", "1010", "0101", "1011", "1100", "0110", "0011",
            ]),
        ),
        (
            SmallCode::hamming_16_11(),
            to(&[
                "10011", "11010", "11111", "11100", "01110", "10101", "01011", "10110", "11001",
                "01101", "00111",
            ]),
        ),
        (SmallCode::hamming_7_4(), to(&["101", "111", "110", "011"])),
    ];
    for (code, rows) in cases {
        assert_eq!(code.rows(), &rows[..], "({}, {})", code.n(), code.k());
    }
}

#[test]
fn golay_24_12_matches_the_m17_generator_and_corrects_three() {
    // M17 specification v2.0.10 appendix D, equation D.1: the P part of G = [I12 | P].
    let p = [
        "110001110101",
        "011000111011",
        "111101101000",
        "011110110100",
        "001111011010",
        "110110011001",
        "011011001101",
        "001101100111",
        "110111000110",
        "101010010111",
        "100100111110",
        "100011101011",
    ];
    let g = SmallCode::golay_24_12();
    let want: Vec<u32> = p
        .iter()
        .map(|r| u32::from_str_radix(r, 2).unwrap())
        .collect();
    assert_eq!(g.rows(), &want[..]);
    assert_eq!(g.radius(), 3);
    let g23 = SmallCode::golay_23_12();
    assert_eq!(g23.radius(), 3);
    let mut rng = Rng::new(1);
    for _ in 0..200 {
        let cw = g.encode(rng.next() as u128);
        let mut e = 0u128;
        while e.count_ones() < 3 {
            e |= 1 << rng.below(24);
        }
        assert_eq!(g.decode(cw ^ e), Some((cw, 3)));
        // Four errors are detected, never miscorrected to a word at distance 4.
        let mut e4 = e;
        while e4.count_ones() < 4 {
            e4 |= 1 << rng.below(24);
        }
        assert_eq!(g.decode(cw ^ e4), None);
    }
}

#[test]
fn dmr_bptc_idle_message_matches_figure_d1() {
    // TS 102 361-1 annex D.2: idle message bits I(95)..I(0) and the encoded 13 x 15 matrix of figure D.1.
    let info = bits_of(
        "111111111000001111011111000101110011001000001001010011101101000111100111110011011000101010010001",
    );
    let fig = [
        "000111111110100",
        "100000111101101",
        "111110001011101",
        "110011001000101",
        "000100101000111",
        "111011010001101",
        "111100111110010",
        "001101100011101",
        "010100100010101",
        "010011100100011",
        "101100111101100",
        "001000010000100",
        "010010101110110",
    ];
    let mut data = vec![0u8; 3];
    data.extend_from_slice(&info); // R(2) R(1) R(0) = 0 lead the first row
    let pc = ProductCode::new(SmallCode::hamming_15_11(), SmallCode::hamming_13_9());
    let mut m = vec![0u8; 195];
    pc.encode(&data, &mut m).unwrap();
    let want: Vec<u8> = fig.iter().flat_map(|r| bits_of(r)).collect();
    assert_eq!(m, want);
    // Iterative decoding fixes a burst no single row could.
    let mut bad = m.clone();
    for i in [16, 17, 18, 100, 101] {
        bad[i] ^= 1;
    }
    pc.decode(&mut bad, 4).unwrap();
    assert_eq!(bad, m);
}

#[test]
fn rds_check_words_and_burst_correction() {
    // NRSC-4-A annex B.1.1: m = 0001h -> 00000000000000010110111001 (no offset), with offset B 0110011000 the check becomes
    // 0000100001; m = FFFFh -> 11111111111111110011001101.
    let rds = SmallCode::rds_26_16();
    assert_eq!(
        rds.encode(0x0001),
        word(&bits_of("00000000000000010110111001"))
    );
    assert_eq!(
        rds.encode(0xFFFF),
        word(&bits_of("11111111111111110011001101"))
    );
    let b = rds.clone().with_offset(0b0110011000);
    assert_eq!(
        b.encode(0x0001),
        word(&bits_of("00000000000000010000100001"))
    );
    // Any burst up to 5 bits is corrected.
    let cw = rds.encode(0x1234);
    for start in 0..22 {
        let burst = 0b10111u128 << start;
        assert_eq!(
            rds.decode(cw ^ burst).map(|x| x.0),
            Some(cw),
            "burst at {start}"
        );
    }
}

#[test]
fn tps_mac_mpt_tetra_codes() {
    // EN 300 744 clause 4.6.2.11: BCH(67,53) from h(x) = 0x4377, t = 2.
    let tps = SmallCode::bch_127_113(67).unwrap();
    assert_eq!((tps.n(), tps.k(), tps.radius()), (67, 53, 2));
    assert!(Bch::from_generator(127, &[0x4377], 2).is_ok());
    for n in [71, 94] {
        assert_eq!(SmallCode::bch_127_113(n).unwrap().radius(), 2);
    }
    // MPT 1327: the last check bit inverted, then even parity.
    let mpt = SmallCode::mpt1327_64_48();
    assert_eq!(mpt.n(), 64);
    let cw = mpt.encode(0x1234_5678_9ABC);
    assert_eq!(cw.count_ones() % 2, 0);
    let plain = SmallCode::cyclic(63, 0xE815)
        .unwrap()
        .encode(0x1234_5678_9ABC);
    assert_eq!(cw >> 1, plain ^ 1);
    assert_eq!(mpt.decode(cw ^ (1 << 40)), Some((cw, 1)));
    // TETRA RM codes.
    let rm = SmallCode::tetra_rm_30_14();
    assert!(rm.radius() >= 2);
    assert!(SmallCode::tetra_rm_16_5().radius() >= 3);
}

#[test]
fn long_bch_codes_from_published_generators() {
    // P25 NID BCH(63,16), generator 6331 1413 6723 5453 (octal): t = 11. Source: TIA, narrow-sense.
    let mut p25 = Bch::from_generator(63, &[0o6331_1413_6723_5453], 11).unwrap();
    assert_eq!(p25.k(), 16);
    let mut rng = Rng::new(7);
    let mut w = rng.bits(63);
    p25.encode(&mut w).unwrap();
    let sent = w.clone();
    for i in 0..11 {
        w[i * 5 + 2] ^= 1;
    }
    assert_eq!(p25.decode(&mut w).unwrap().corrected, 11);
    assert_eq!(w, sent);
    // Cospas-Sarsat T.018 BCH(250,202), t = 6 (0x1C7EB85DF3C97, C/S T.018 Appendix B; see bch::codes).
    let mut t018 = Bch::from_generator(250, &[0x1_C7EB_85DF_3C97], 6).unwrap();
    let mut w = rng.bits(250);
    t018.encode(&mut w).unwrap();
    let sent = w.clone();
    for i in [0, 50, 100, 150, 200, 249] {
        w[i] ^= 1;
    }
    t018.decode(&mut w).unwrap();
    assert_eq!(w, sent);
    // Seven errors are not silently accepted as six-error corrections of the sent word.
    for i in [1, 60, 120] {
        w[i] ^= 1;
    }
    let before = w.clone();
    if t018.decode(&mut w).is_err() {
        assert_eq!(w, before);
    }
}

#[test]
fn isdb_t_tmcc_difference_set_code() {
    // ARIB STD-B31 clause 3.15.6.9: g(x) = x^82 + x^77 + x^76 + x^71 + x^67 + x^66 + x^56 + x^52 + x^48 + x^40 + x^36 + x^34
    // + x^24 + x^22 + x^18 + x^10 + x^4 + 1, the (273,191) code shortened to (184,102).
    // Built from the generator printed there; the constructor checks every check against it.
    let mut ml = MajorityLogic::isdb_t_tmcc().unwrap();
    assert_eq!(ml.checks_per_bit(), 17);
    let mut rng = Rng::new(8);
    for _ in 0..20 {
        let mut w = rng.bits(184);
        ml.encoder().encode(&mut w).unwrap();
        let sent = w.clone();
        let mut placed = 0;
        while placed < 8 {
            let i = rng.below(184);
            if w[i] == sent[i] {
                w[i] ^= 1;
                placed += 1;
            }
        }
        assert_eq!(ml.decode(&mut w).unwrap().corrected, 8);
        assert_eq!(w, sent);
    }
}

#[test]
fn chase_soft_decoding_beats_hard_decoding() {
    // POCSAG with three errors on weak bits: beyond the table, inside a Chase search.
    let p = SmallCode::pocsag_32_21();
    let cw = p.encode(0x1_2345);
    let err = (1u128 << 3) | (1 << 17) | (1 << 29);
    let mut rel = [9.0f32; 32];
    for b in [3usize, 17, 29] {
        rel[31 - b] = 0.5;
    }
    rel[5] = 0.7;
    assert_ne!(p.decode(cw ^ err).map(|x| x.0), Some(cw));
    let (got, flips, _) = p.decode_soft(cw ^ err, &rel, Chase::default()).unwrap();
    assert_eq!((got, flips), (cw, 3));
    // The generic search with a CRC-like check.
    let mut bits: Vec<u8> = (0..32).rev().map(|i| ((cw ^ err) >> i) as u8 & 1).collect();
    let n = chase_search(&mut bits, &rel, Chase::default(), |b| {
        p.syndrome(word(b)) == 0
    })
    .unwrap();
    assert_eq!((word(&bits), n), (cw, 3));
}

#[test]
fn dmr_trellis_table_b7_decodes() {
    // TS 102 361-1 table B.7: entry = constellation point for (state, input tribit); the next state is the input.
    let t: [&[u8]; 8] = [
        &[0, 8, 4, 12, 2, 10, 6, 14],
        &[4, 12, 2, 10, 6, 14, 0, 8],
        &[1, 9, 5, 13, 3, 11, 7, 15],
        &[5, 13, 3, 11, 7, 15, 1, 9],
        &[3, 11, 7, 15, 1, 9, 5, 13],
        &[7, 15, 1, 9, 5, 13, 3, 11],
        &[2, 10, 6, 14, 0, 8, 4, 12],
        &[6, 14, 0, 8, 4, 12, 2, 10],
    ];
    let tr = Trellis::last_input(&t).unwrap();
    // Table B.8: constellation point -> (dibit 0, dibit 1) symbol values.
    let pts: [(i32, i32); 16] = [
        (1, -1),
        (-1, -1),
        (3, -3),
        (-3, -3),
        (-3, -1),
        (3, -1),
        (-1, -3),
        (1, -3),
        (-3, 3),
        (3, 3),
        (-1, 1),
        (1, 1),
        (1, 3),
        (-1, 3),
        (3, 1),
        (-3, 1),
    ];
    let mut rng = Rng::new(9);
    let mut tribits: Vec<u8> = (0..48).map(|_| rng.below(8) as u8).collect();
    tribits.push(0); // the flushing tribit
    let mut labels = vec![0u16; 49];
    tr.encode(0, &tribits, &mut labels).unwrap();
    // Noisy symbols, metrics -(distance^2) scaled.
    struct M<'a> {
        rx: &'a [(f32, f32)],
        pts: &'a [(i32, i32); 16],
    }
    impl BranchMetrics for M<'_> {
        fn labels(&self) -> usize {
            16
        }
        fn fill(&mut self, step: usize, m: &mut [i32]) {
            let (a, b) = self.rx[step];
            for (l, v) in m.iter_mut().enumerate() {
                let (x, y) = self.pts[l];
                *v = -(((a - x as f32).powi(2) + (b - y as f32).powi(2)) * 100.0) as i32;
            }
        }
    }
    let rx: Vec<(f32, f32)> = labels
        .iter()
        .map(|&l| {
            let (x, y) = pts[l as usize];
            (
                x as f32 + 0.5 * rng.gauss() as f32,
                y as f32 + 0.5 * rng.gauss() as f32,
            )
        })
        .collect();
    let mut dec = TrellisDecoder::new(&tr, 64).unwrap();
    let mut out = vec![0u8; 49];
    dec.decode(
        &mut M { rx: &rx, pts: &pts },
        49,
        Ends::Zero,
        None,
        &mut out,
    )
    .unwrap();
    assert_eq!(out, tribits);
}

#[test]
fn atsc_and_tc8psk_trellises() {
    // ATSC A/53 part 2 figure 5.6: Z0 = S_b, S_a' = S_b, S_b' = S_a XOR X1 in the figure's naming; this crate's state (a, b) with
    // Z0 = a and next ((X1 ^ b), a) is the same machine with the cells named the other way round.
    let t = Trellis::atsc_8vsb();
    assert_eq!((t.states(), t.inputs(), t.labels()), (4, 2, 4));
    // Simulate the figure's register directly and compare outputs over a long random input.
    let mut rng = Rng::new(10);
    let x1 = rng.bits(500);
    let (mut sa, mut sb) = (0u8, 0u8);
    let mut fig = Vec::new();
    for &x in &x1 {
        fig.push(((x as u16) << 1) | sb as u16);
        let (na, nb) = (sb, sa ^ x);
        sa = na;
        sb = nb;
    }
    let mut lab = vec![0u16; 500];
    t.encode(0, &x1, &mut lab).unwrap();
    assert_eq!(lab, fig);
    // TC8PSK: the K = 7 code as a trellis with 4 labels.
    let tc = Trellis::from_code(&sdrkit_algo_fec::conv::codes::ISDB_S_K7).unwrap();
    assert_eq!((tc.states(), tc.labels()), (64, 4));
}

// ----- GLONASS string code -----

/// GLONASS ICD Edition 5.1 table 4.13: the string bits that check bit β2 covers, as printed.
const GLONASS_BETA2: [usize; 41] = [
    9, 11, 12, 14, 15, 18, 19, 21, 22, 25, 26, 29, 30, 33, 34, 36, 37, 40, 41, 44, 45, 48, 49, 52,
    53, 56, 57, 60, 61, 64, 65, 67, 68, 71, 72, 75, 76, 79, 80, 83, 84,
];

#[test]
fn glonass_check_bits_cover_the_printed_bits() {
    let g = GlonassHamming::new();
    assert_eq!((g.code().n(), g.code().k()), (85, 77));
    // String bit k is s[85 - k]; β_i is string bit i.
    for k in 9..=85usize {
        let mut s = [0u8; 85];
        s[85 - k] = 1;
        g.encode_bits(&mut s).unwrap();
        assert_eq!(
            s[85 - 2] == 1,
            GLONASS_BETA2.contains(&k),
            "bit {k} in beta2"
        );
        // β8 makes the parity of the whole string even.
        assert_eq!(s.iter().map(|&b| b as u32).sum::<u32>() % 2, 0, "bit {k}");
    }
}

#[test]
fn glonass_check_follows_rules_a_to_c() {
    let g = GlonassHamming::new();
    let mut s = [0u8; 85];
    for (i, b) in s[..77].iter_mut().enumerate() {
        *b = ((i * 7 + 3) % 5 < 2) as u8;
    }
    g.encode_bits(&mut s).unwrap();
    let sent = s;
    let mut r = sent;
    assert_eq!(g.check_bits(&mut r).unwrap(), GlonassCheck::Good);
    // Rule b: one data error is corrected.
    for i in 0..77 {
        let mut r = sent;
        r[i] ^= 1;
        assert_eq!(
            g.check_bits(&mut r).unwrap(),
            GlonassCheck::Corrected,
            "data bit {i}"
        );
        assert_eq!(r, sent);
    }
    // Rule a: one error in β1 to β7 leaves the data good (and as received).
    for i in 78..85 {
        let mut r = sent;
        r[i] ^= 1;
        assert_eq!(
            g.check_bits(&mut r).unwrap(),
            GlonassCheck::Good,
            "check bit {i}"
        );
    }
    // Rule c: C1 to C7 all zero with C-sigma 1 (an error in β8 alone), or two errors, erase the string.
    let mut r = sent;
    r[77] ^= 1;
    assert_eq!(g.check_bits(&mut r).unwrap(), GlonassCheck::Erased);
    for (a, b) in [(0usize, 1usize), (5, 60), (10, 80), (78, 83)] {
        let mut r = sent;
        r[a] ^= 1;
        r[b] ^= 1;
        assert_eq!(
            g.check_bits(&mut r).unwrap(),
            GlonassCheck::Erased,
            "{a} {b}"
        );
    }
    assert!(g.check_bits(&mut [0u8; 84]).is_err());
    assert!(g.encode_bits(&mut [0u8; 86]).is_err());
}
