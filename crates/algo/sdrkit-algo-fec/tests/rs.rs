//! Reed-Solomon: generators and dual-basis tables from the standards, decoding capability with errors and erasures, every code
//! form (shortened, low-first, dual basis, interleaved, GF(2^m) for m = 2 .. 16), and scalar against vector kernels.

mod common;
use common::*;
use sdrkit_algo_fec::gf::Gf;
use sdrkit_algo_fec::rs::{
    check_backend, codes, Basis, Rs, RsCode, SymbolOrder, BACKENDS, FROM_DUAL, TO_DUAL,
};
use sdrkit_algo_fec::FecError;

/// CCSDS 131.0-B-5 annex F table F-1: for 0 then α^0 .. α^254, the conventional byte (α^7 .. α^0) and the dual-basis byte
/// (l0 .. l7), as printed.
const TABLE_F1: [(u8, u8); 256] = [
    (0x00, 0x00),
    (0x01, 0x7B),
    (0x02, 0xAF),
    (0x04, 0x99),
    (0x08, 0xFA),
    (0x10, 0x86),
    (0x20, 0xEC),
    (0x40, 0xEF),
    (0x80, 0x8D),
    (0x87, 0xC0),
    (0x89, 0x0C),
    (0x95, 0xE9),
    (0xAD, 0x79),
    (0xDD, 0xFC),
    (0x3D, 0x72),
    (0x7A, 0xD0),
    (0xF4, 0x91),
    (0x6F, 0xB4),
    (0xDE, 0x28),
    (0x3B, 0x44),
    (0x76, 0xB3),
    (0xEC, 0xED),
    (0x5F, 0xDE),
    (0xBE, 0x2B),
    (0xFB, 0x26),
    (0x71, 0xFE),
    (0xE2, 0x21),
    (0x43, 0x3B),
    (0x86, 0xBB),
    (0x8B, 0xA3),
    (0x91, 0x70),
    (0xA5, 0x83),
    (0xCD, 0x7A),
    (0x1D, 0x9E),
    (0x3A, 0x3F),
    (0x74, 0x1C),
    (0xE8, 0x74),
    (0x57, 0x24),
    (0xAE, 0xAD),
    (0xDB, 0xCA),
    (0x31, 0x11),
    (0x62, 0xAC),
    (0xC4, 0xFB),
    (0x0F, 0xB7),
    (0x1E, 0x4A),
    (0x3C, 0x09),
    (0x78, 0x7F),
    (0xF0, 0x08),
    (0x67, 0x4E),
    (0xCE, 0xAE),
    (0x1B, 0xA8),
    (0x36, 0x5C),
    (0x6C, 0x60),
    (0xD8, 0x1E),
    (0x37, 0x27),
    (0x6E, 0xCF),
    (0xDC, 0x87),
    (0x3F, 0xDD),
    (0x7E, 0x49),
    (0xFC, 0x6B),
    (0x7F, 0x32),
    (0xFE, 0xC4),
    (0x7B, 0xAB),
    (0xF6, 0x3E),
    (0x6B, 0x2D),
    (0xD6, 0xD2),
    (0x2B, 0xC2),
    (0x56, 0x5F),
    (0xAC, 0x02),
    (0xDF, 0x53),
    (0x39, 0xEB),
    (0x72, 0x2A),
    (0xE4, 0x17),
    (0x4F, 0x58),
    (0x9E, 0xC7),
    (0xBB, 0xC9),
    (0xF1, 0x73),
    (0x65, 0xE1),
    (0xCA, 0x37),
    (0x13, 0x52),
    (0x26, 0xDA),
    (0x4C, 0x8C),
    (0x98, 0xF1),
    (0xB7, 0xAA),
    (0xE9, 0x0F),
    (0x55, 0x8B),
    (0xAA, 0x34),
    (0xD3, 0x30),
    (0x21, 0x97),
    (0x42, 0x40),
    (0x84, 0x14),
    (0x8F, 0x3A),
    (0x99, 0x8A),
    (0xB5, 0x05),
    (0xED, 0x96),
    (0x5D, 0x71),
    (0xBA, 0xB2),
    (0xF3, 0xDC),
    (0x61, 0x78),
    (0xC2, 0xCD),
    (0x03, 0xD4),
    (0x06, 0x36),
    (0x0C, 0x63),
    (0x18, 0x7C),
    (0x30, 0x6A),
    (0x60, 0x03),
    (0xC0, 0x62),
    (0x07, 0x4D),
    (0x0E, 0xCC),
    (0x1C, 0xE5),
    (0x38, 0x90),
    (0x70, 0x85),
    (0xE0, 0x8E),
    (0x47, 0xA2),
    (0x8E, 0x41),
    (0x9B, 0x25),
    (0xB1, 0x9C),
    (0xE5, 0x6C),
    (0x4D, 0xF7),
    (0x9A, 0x5E),
    (0xB3, 0x33),
    (0xE1, 0xF5),
    (0x45, 0x0D),
    (0x8A, 0xD8),
    (0x93, 0xDF),
    (0xA1, 0x1A),
    (0xC5, 0x80),
    (0x0D, 0x18),
    (0x1A, 0xD3),
    (0x34, 0xF3),
    (0x68, 0xF9),
    (0xD0, 0xE4),
    (0x27, 0xA1),
    (0x4E, 0x23),
    (0x9C, 0x68),
    (0xBF, 0x50),
    (0xF9, 0x89),
    (0x75, 0x67),
    (0xEA, 0xDB),
    (0x53, 0xBD),
    (0xA6, 0x57),
    (0xCB, 0x4C),
    (0x11, 0xFD),
    (0x22, 0x43),
    (0x44, 0x76),
    (0x88, 0x77),
    (0x97, 0x46),
    (0xA9, 0xE0),
    (0xD5, 0x06),
    (0x2D, 0xF4),
    (0x5A, 0x3C),
    (0xB4, 0x7E),
    (0xEF, 0x39),
    (0x59, 0xE8),
    (0xB2, 0x48),
    (0xE3, 0x5A),
    (0x41, 0x94),
    (0x82, 0x22),
    (0x83, 0x59),
    (0x81, 0xF6),
    (0x85, 0x6F),
    (0x8D, 0x95),
    (0x9D, 0x13),
    (0xBD, 0xFF),
    (0xFD, 0x10),
    (0x7D, 0x9D),
    (0xFA, 0x5D),
    (0x73, 0x51),
    (0xE6, 0xB8),
    (0x4B, 0xC1),
    (0x96, 0x3D),
    (0xAB, 0x4F),
    (0xD1, 0x9F),
    (0x25, 0x0E),
    (0x4A, 0xBA),
    (0x94, 0x92),
    (0xAF, 0xD6),
    (0xD9, 0x65),
    (0x35, 0x88),
    (0x6A, 0x56),
    (0xD4, 0x7D),
    (0x2F, 0x5B),
    (0x5E, 0xA5),
    (0xBC, 0x84),
    (0xFF, 0xBF),
    (0x79, 0x04),
    (0xF2, 0xA7),
    (0x63, 0xD7),
    (0xC6, 0x54),
    (0x0B, 0x2E),
    (0x16, 0xB0),
    (0x2C, 0x8F),
    (0x58, 0x93),
    (0xB0, 0xE7),
    (0xE7, 0xC3),
    (0x49, 0x6E),
    (0x92, 0xA4),
    (0xA3, 0xB5),
    (0xC1, 0x19),
    (0x05, 0xE2),
    (0x0A, 0x55),
    (0x14, 0x1F),
    (0x28, 0x16),
    (0x50, 0x69),
    (0xA0, 0x61),
    (0xC7, 0x2F),
    (0x09, 0x81),
    (0x12, 0x29),
    (0x24, 0x75),
    (0x48, 0x15),
    (0x90, 0x0B),
    (0xA7, 0x2C),
    (0xC9, 0xE3),
    (0x15, 0x64),
    (0x2A, 0xB9),
    (0x54, 0xF0),
    (0xA8, 0x9B),
    (0xD7, 0xA9),
    (0x29, 0x6D),
    (0x52, 0xC6),
    (0xA4, 0xF8),
    (0xCF, 0xD5),
    (0x19, 0x07),
    (0x32, 0xC5),
    (0x64, 0x9A),
    (0xC8, 0x98),
    (0x17, 0xCB),
    (0x2E, 0x20),
    (0x5C, 0x0A),
    (0xB8, 0x1D),
    (0xF7, 0x45),
    (0x69, 0x82),
    (0xD2, 0x4B),
    (0x23, 0x38),
    (0x46, 0xD9),
    (0x8C, 0xEE),
    (0x9F, 0xBC),
    (0xB9, 0x66),
    (0xF5, 0xEA),
    (0x6D, 0x1B),
    (0xDA, 0xB1),
    (0x33, 0xBE),
    (0x66, 0x35),
    (0xCC, 0x01),
    (0x1F, 0x31),
    (0x3E, 0xA6),
    (0x7C, 0xE6),
    (0xF8, 0xF2),
    (0x77, 0xC8),
    (0xEE, 0x42),
    (0x5B, 0x47),
    (0xB6, 0xD1),
    (0xEB, 0xA0),
    (0x51, 0x12),
    (0xA2, 0xCE),
    (0xC3, 0xB6),
];

#[test]
fn ccsds_dual_basis_matches_table_f1_and_the_annex_f_examples() {
    let f = Gf::new(8, 0x187).unwrap();
    for (i, &(conv, dual)) in TABLE_F1.iter().enumerate() {
        if i > 0 {
            assert_eq!(f.exp(i - 1), conv as u16, "power {}", i - 1);
        }
        assert_eq!(TO_DUAL[conv as usize], dual, "row {i}");
        assert_eq!(FROM_DUAL[dual as usize], conv, "row {i}");
    }
    // Annex F example 1: dual 10111001 is conventional 00101010 = α^213.
    assert_eq!(FROM_DUAL[0b1011_1001], 0b0010_1010);
    assert_eq!(f.exp(213), 0b0010_1010);
    // Example 2: α^152 = 01011001 is sent as 11101000.
    assert_eq!(f.exp(152), 0b0101_1001);
    assert_eq!(TO_DUAL[0b0101_1001], 0b1110_1000);
}

#[test]
fn ccsds_generators_match_annex_g() {
    // CCSDS 131.0-B-5 annex G: G_i = G_(2E - i) as powers of α.
    let e16 = [
        0usize, 249, 59, 66, 4, 43, 126, 251, 97, 30, 3, 213, 50, 66, 170, 5, 24,
    ];
    let e8 = [0usize, 30, 230, 49, 235, 129, 81, 76, 173];
    for (code, want) in [
        (codes::CCSDS_255_223, &e16[..]),
        (codes::CCSDS_255_239, &e8[..]),
    ] {
        let rs = Rs::new(code).unwrap();
        let f = rs.field();
        let g = rs.generator();
        let e2 = g.len() - 1;
        for (i, &p) in want.iter().enumerate() {
            assert_eq!(g[i], f.exp(p), "{} G{i}", code.name);
            assert_eq!(g[e2 - i], f.exp(p), "{} G{}", code.name, e2 - i);
        }
    }
}

#[test]
fn atsc_generator_matches_figure_5_4() {
    // A/53 part 2 figure 5.4, from X^20 down to X^0.
    let want = [
        1u16, 152, 185, 240, 5, 111, 99, 6, 220, 112, 150, 69, 36, 187, 22, 228, 198, 121, 121,
        165, 174,
    ];
    let rs = Rs::new(codes::ATSC_207_187).unwrap();
    let g: Vec<u16> = rs.generator().iter().rev().copied().collect();
    assert_eq!(g, want);
}

#[test]
fn dmr_rs_12_9_matches_table_b18() {
    // TS 102 361-1 equation B.10: g(x) = x^3 + 0E x^2 + 38 x + 40.
    let rs = Rs::new(codes::DMR_12_9).unwrap();
    assert_eq!(rs.generator(), &[0x40, 0x38, 0x0E, 1]);
    // Table B.18: the parity (p2 p1 p0) of each message unit vector, m8 first.
    let rows = [
        [0x1C, 0xBC, 0xFD],
        [0x89, 0x31, 0x08],
        [0xAD, 0x41, 0x36],
        [0x7D, 0x71, 0x16],
        [0xF3, 0xA6, 0x3A],
        [0x08, 0x83, 0x7B],
        [0x3F, 0x6F, 0x02],
        [0x6C, 0x0D, 0xA7],
        [0x0E, 0x38, 0x40],
    ];
    for (i, want) in rows.iter().enumerate() {
        let mut data = [0u8; 9];
        data[i] = 1;
        let mut p = [0u8; 3];
        rs.encode(&data, &mut p).unwrap();
        assert_eq!(&p, want, "row {}", i + 1);
    }
}

#[test]
fn hd_radio_header_code_matches_1017s() {
    // NRSC-5 1017s clause 5.2.3.1: g(x) = x^8 + a^176 x^7 + a^240 x^6 + a^211 x^5 + a^253 x^4 + a^220 x^3 + a^3 x^2
    // + a^203 x + a^36.
    let code = codes::HD_RADIO_HEADER_96_88;
    let mut rs = Rs::new(code).unwrap();
    let gf = Gf::new(8, 0x11D).unwrap();
    let want: Vec<u16> = [36u32, 203, 3, 220, 253, 211, 240, 176]
        .iter()
        .map(|&p| gf.exp(p as usize))
        .chain([1])
        .collect();
    assert_eq!(rs.generator(), &want[..]);
    assert_eq!(
        Rs::new(codes::HD_RADIO_96_88).unwrap().generator(),
        &want[..]
    );
    // The parity leads the PDU: bytes 0 to 7 are the coefficients of x^0 to x^7, and the 88 data bytes follow.
    assert_eq!(code.order, SymbolOrder::LowFirst);
    let mut word = [0u8; 96];
    for (i, b) in word[8..].iter_mut().enumerate() {
        *b = (i * 29 + 7) as u8;
    }
    let data = word[8..].to_vec();
    rs.encode_word(&mut word).unwrap();
    assert_eq!(&word[8..], &data[..]);
    assert!(word[..8].iter().any(|&b| b != 0));
    // As a polynomial with PDU byte i at x^i, the word is a multiple of g: it is zero at every root a^1 .. a^8.
    for j in 1..=8usize {
        let root = gf.exp(j);
        let v = word
            .iter()
            .rev()
            .fold(0u16, |acc, &c| gf.mul(acc, root) ^ c as u16);
        assert_eq!(v, 0, "root a^{j}");
    }
    assert!(rs.check(&word).unwrap());
    // The data-first entry does not accept the same PDU.
    let mut old = Rs::new(codes::HD_RADIO_96_88).unwrap();
    assert!(!old.check(&word).unwrap());
}

#[test]
fn galileo_fec2_generator_matches_table_108() {
    let want = [
        193u16, 91, 190, 154, 101, 58, 231, 197, 152, 88, 73, 62, 169, 88, 188, 23, 36, 202, 63,
        20, 102, 230, 131, 141, 214, 45, 101, 94, 62, 65, 66, 46, 131, 42, 187, 9, 122, 3, 19, 118,
        6, 154, 14, 193, 79, 251, 124, 18, 186, 244, 166, 235, 167, 108, 41, 19, 76, 48, 42, 208,
        1,
    ];
    let rs = Rs::new(codes::GALILEO_FEC2_118_58).unwrap();
    assert_eq!(rs.generator(), &want);
}

fn random_word(rng: &mut Rng, rs: &Rs) -> Vec<u8> {
    let c = rs.code();
    let mask = ((1u32 << c.m) - 1) as u8;
    let mut w: Vec<u8> = (0..c.n).map(|_| rng.next() as u8 & mask).collect();
    rs.encode_word(&mut w).unwrap();
    w
}

/// Adds `errors` random errors at distinct random places, avoiding `avoid`; returns the places.
fn corrupt(rng: &mut Rng, w: &mut [u8], m: u8, errors: usize, avoid: &[u16]) -> Vec<u16> {
    let mask = ((1u32 << m) - 1) as u8;
    let mut at = Vec::new();
    while at.len() < errors {
        let i = rng.below(w.len()) as u16;
        if at.contains(&i) || avoid.contains(&i) {
            continue;
        }
        let e = loop {
            let e = rng.next() as u8 & mask;
            if e != 0 {
                break e;
            }
        };
        w[i as usize] ^= e;
        at.push(i);
    }
    at
}

#[test]
fn every_named_code_corrects_up_to_its_limit() {
    let mut rng = Rng::new(1);
    let all = [
        codes::DVB_204_188,
        codes::ISDB_T_204_196,
        codes::ATSC_207_187,
        codes::DAB_PLUS_120_110,
        codes::ISDB_S_TMCC_64_48,
        codes::DMR_12_9,
        codes::HD_RADIO_96_88,
        codes::HD_RADIO_HEADER_96_88,
        codes::RS41_255_231.shortened(156),
        codes::RS92_234_210,
        codes::CCSDS_255_223,
        codes::CCSDS_255_239,
        codes::CCSDS_255_223_CONVENTIONAL,
        codes::CCSDS_255_223.shortened(100),
        codes::UAT_30_18,
        codes::UAT_48_34,
        codes::UAT_92_72,
        codes::vdl2(255),
        codes::vdl2(40),
        codes::ardop(120, 32),
        codes::GALILEO_FEC2_118_58,
        codes::P25_24_12,
        codes::P25_24_16,
        codes::P25_36_20,
        codes::P25_45_26,
        codes::P25_52_30,
        codes::JT65_63_12,
    ];
    for code in all {
        let mut rs = Rs::new(code).unwrap();
        let t = code.nroots() / 2;
        for trial in 0..20 {
            let sent = random_word(&mut rng, &rs);
            assert!(rs.check(&sent).unwrap(), "{}", code.name);
            let mut w = sent.clone();
            let nerr = if trial == 0 { t } else { rng.below(t + 1) };
            corrupt(&mut rng, &mut w, code.m, nerr, &[]);
            let r = rs
                .decode(&mut w, &[])
                .unwrap_or_else(|e| panic!("{} {nerr}: {e}", code.name));
            assert_eq!(w, sent, "{}", code.name);
            assert_eq!(r.corrected as usize, nerr);
            // Errors and erasures: 2 e + s <= n - k.
            let mut w = sent.clone();
            let s = rng.below(code.nroots() + 1);
            let er = corrupt(&mut rng, &mut w, code.m, s, &[]);
            let e = (code.nroots() - s) / 2;
            corrupt(&mut rng, &mut w, code.m, e, &er);
            rs.decode(&mut w, &er)
                .unwrap_or_else(|x| panic!("{} s {s} e {e}: {x}", code.name));
            assert_eq!(w, sent, "{} erasures", code.name);
        }
    }
}

#[test]
fn beyond_the_limit_fails_cleanly_and_leaves_the_word() {
    let mut rng = Rng::new(2);
    let mut rs = Rs::new(codes::CCSDS_255_223).unwrap();
    let (mut failed, mut wrong) = (0, 0);
    for _ in 0..200 {
        let sent = random_word(&mut rng, &rs);
        let mut w = sent.clone();
        let ne = 17 + rng.below(20);
        corrupt(&mut rng, &mut w, 8, ne, &[]);
        let before = w.clone();
        match rs.decode(&mut w, &[]) {
            Err(FecError::Uncorrectable) => {
                failed += 1;
                assert_eq!(w, before);
            }
            Ok(_) => {
                // A miscorrection lands on another code word.
                assert!(rs.check(&w).unwrap());
                if w != sent {
                    wrong += 1;
                }
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert!(failed >= 195 && wrong <= 5, "{failed} {wrong}");
}

#[test]
fn interleaved_frames_and_failed_code_words() {
    let mut rng = Rng::new(3);
    let mut rs = Rs::new(codes::CCSDS_255_223).unwrap();
    for depth in [1usize, 2, 4, 5, 8] {
        let words: Vec<Vec<u8>> = (0..depth).map(|_| random_word(&mut rng, &rs)).collect();
        let mut frame = vec![0u8; 255 * depth];
        for (cw, w) in words.iter().enumerate() {
            for (i, &s) in w.iter().enumerate() {
                frame[i * depth + cw] = s;
            }
        }
        let sent = frame.clone();
        // A burst of 16 * depth symbols: 16 per code word.
        for b in frame[100..100 + 16 * depth].iter_mut() {
            *b ^= 0xA5;
        }
        let r = rs.decode_interleaved(&mut frame, depth, &[]).unwrap();
        assert_eq!(frame, sent);
        assert_eq!((r.corrected as usize, r.failed), (16 * depth, 0));
    }
    // Code word 1 of 2 beyond repair: it is left alone, code word 0 is corrected.
    let words: Vec<Vec<u8>> = (0..2).map(|_| random_word(&mut rng, &rs)).collect();
    let mut frame: Vec<u8> = (0..510).map(|i| words[i % 2][i / 2]).collect();
    let sent = frame.clone();
    frame[0] ^= 1;
    for i in 0..40 {
        frame[2 * i + 1] ^= 0x33;
    }
    let damaged = frame.clone();
    let r = rs.decode_interleaved(&mut frame, 2, &[]).unwrap();
    assert_eq!((r.failed, r.failed_mask), (1, 0b10));
    assert_eq!(frame[0], sent[0]);
    for i in 0..255 {
        assert_eq!(frame[2 * i + 1], damaged[2 * i + 1]);
    }
}

#[test]
fn all_field_sizes_and_both_orders() {
    let mut rng = Rng::new(4);
    let polys = [
        0x7u32, 0xB, 0x13, 0x25, 0x43, 0x89, 0x11D, 0x211, 0x409, 0x805, 0x1053, 0x201B, 0x4443,
        0x8003, 0x1100B,
    ];
    for (i, &poly) in polys.iter().enumerate() {
        let m = (i + 2) as u8;
        let nn = (1u32 << m) - 1;
        let n = nn.min(300) as u16;
        let nroots = (n / 4).max(2);
        for order in [SymbolOrder::HighFirst, SymbolOrder::LowFirst] {
            let code = RsCode {
                name: "test",
                m,
                poly,
                n,
                k: n - nroots,
                fcr: 1,
                prim: if m == 8 { 7 } else { 1 },
                order,
                basis: Basis::Conventional,
            };
            let mut rs = Rs::new(code).unwrap_or_else(|e| panic!("m {m}: {e}"));
            let q = 1u32 << m;
            let mut w: Vec<u16> = (0..n).map(|_| (rng.next() as u32 % q) as u16).collect();
            let k = (n - nroots) as usize;
            let (d, p) = match order {
                SymbolOrder::HighFirst => {
                    let (d, p) = w.split_at_mut(k);
                    (d.to_vec(), p)
                }
                SymbolOrder::LowFirst => {
                    let (p, d) = w.split_at_mut(nroots as usize);
                    (d.to_vec(), p)
                }
            };
            rs.encode16(&d, p).unwrap();
            let sent = w.clone();
            let t = nroots as usize / 2;
            let mut placed = Vec::new();
            while placed.len() < t {
                let at = rng.below(n as usize);
                if !placed.contains(&at) {
                    placed.push(at);
                    w[at] ^= 1 + (rng.next() as u32 % (q - 1)) as u16;
                }
            }
            let r = rs.decode16(&mut w, &[]).unwrap();
            assert_eq!(w, sent, "m {m} {order:?}");
            assert_eq!(r.corrected as usize, t);
        }
    }
    assert!(Rs::new(RsCode {
        prim: 5,
        ..codes::DVB_204_188
    })
    .is_err());
    assert!(Rs::new(RsCode {
        basis: Basis::Dual,
        ..codes::DVB_204_188
    })
    .is_err());
    assert!(Rs::new(RsCode {
        n: 256,
        ..codes::DVB_204_188
    })
    .is_err());
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
fn vector_and_scalar_kernels_agree() {
    let mut rng = Rng::new(5);
    for code in [
        codes::DVB_204_188,
        codes::CCSDS_255_223,
        codes::JT65_63_12,
        codes::DMR_12_9,
        codes::ATSC_207_187,
    ] {
        let mut a = Rs::new(code).unwrap();
        let mut b = Rs::new(code).unwrap();
        b.set_backend(BACKENDS.plain()).unwrap();
        for _ in 0..100 {
            let sent = random_word(&mut rng, &a);
            let mut w = sent.clone();
            let ne = rng.below(code.nroots() + 4);
            corrupt(&mut rng, &mut w, code.m, ne, &[]);
            let mut w2 = w.clone();
            let ra = a.decode(&mut w, &[]);
            let rb = b.decode(&mut w2, &[]);
            assert_eq!(ra, rb);
            assert_eq!(w, w2);
        }
    }
}

#[test]
fn bad_erasures_and_lengths() {
    let mut rs = Rs::new(codes::DVB_204_188).unwrap();
    let mut w = vec![0u8; 204];
    assert_eq!(
        rs.decode(&mut w, &[204]),
        Err(FecError::BadErasure { at: 204 })
    );
    assert_eq!(
        rs.decode(&mut w, &[3, 3]),
        Err(FecError::BadErasure { at: 3 })
    );
    assert!(rs.decode(&mut w[..200], &[]).is_err());
    let many: Vec<u16> = (0..17).collect();
    assert_eq!(rs.decode(&mut w, &many), Err(FecError::Uncorrectable));
    assert!(rs.encode(&[0u8; 10], &mut [0u8; 16]).is_err());
    // An all-zero word with erasures needs no change.
    assert_eq!(rs.decode(&mut w, &[1, 2, 3]).unwrap().corrected, 0);
}
