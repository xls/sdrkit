//! DVB-C2: the format layers (sequences, pilots, cell orders, interleavers, headers, L1, constellations, reserved tones), the
//! coding of the PLPs through noise, and the round trips from the test signal through the receiver.

use sdrkit_algo_fec::reedmuller::{rm25_encode, Rm25Decoder};
use sdrkit_algo_fec::twistblock::TwistedBlock;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::Complex32;
use sdrkit_stack_dvbcs::dvbc2::bicm::{bit_cells, point_set, PlpDecoder, PlpEncoder};
use sdrkit_stack_dvbcs::dvbc2::header::{
    self, branches, preamble_bits, preamble_fields, FecFrameHeader, HeaderDecoder,
};
use sdrkit_stack_dvbcs::dvbc2::l1::{L1Codec, L1Part2, L1Sizes, Notch, Plp, Slice};
use sdrkit_stack_dvbcs::dvbc2::packets::header_modcod;
use sdrkit_stack_dvbcs::dvbc2::signal::{
    freq_addresses, l1_cell, reserved_position, sequences, Cell, Guard, Layout, BLOCK, L1_CELLS,
};
use sdrkit_stack_dvbcs::dvbc2::tables::{self, xfecframe_len, CodeRate, ModCod, Qam, W_RM_DEFAULT};

/// Adds complex white noise of variance `n0` to every cell.
fn noisy(rng: &mut Rng, z: &mut [Complex32], n0: f32) {
    for c in z {
        *c += rng.complex_normal(f64::from(n0));
    }
}

// 1. Sequences.

#[test]
fn scrambling_sequences() {
    // The first 32 values of w and w' (clauses 9.2.1 and 9.2.2, figures 31 and 32).
    let s = sequences();
    let w32: Vec<u8> = (0..32).map(|k| s.w(k)).collect();
    assert_eq!(
        w32,
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1]
    );
    let wp32: Vec<u8> = (0..32).map(|i| s.w_prime(i)).collect();
    assert_eq!(
        wp32,
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 1]
    );
    let w = |k: usize| s.w(k);
    let w_prime = |i: usize| s.w_prime(i);
    // The recurrences of the figures: w_(n+11) = w_(n+2) xor w_n, w'_(n+10) = w'_(n+3) xor w'_n.
    assert!((0..3000).all(|n| w(n + 11) == w(n + 2) ^ w(n)));
    assert!((0..3000).all(|n| w_prime(n + 10) == w_prime(n + 3) ^ w_prime(n)));
    let periodic = |f: &dyn Fn(usize) -> u8, n: usize, p: usize| (0..n).all(|k| f(k) == f(k + p));
    assert!(periodic(&w, 4000, 2047));
    assert!(periodic(&w_prime, 2000, 1023));
    for p in [23, 89] {
        assert!(!periodic(&w, 2047, p));
    }
    for p in [33, 93, 341] {
        assert!(!periodic(&w_prime, 1023, p));
    }
    assert_eq!((0..2047).map(|k| u32::from(w(k))).sum::<u32>(), 1024);
    assert_eq!((0..1023).map(|i| u32::from(w_prime(i))).sum::<u32>(), 512);
    // r_k at a block start is w_k xor w'_0; within a block neighbours differ by w^P_k.
    let mut r = Vec::new();
    let first = 3 * BLOCK;
    s.pilot_bits(first, 2 * BLOCK / 6, &mut r);
    assert_eq!(r[0], w(first) ^ w_prime(0));
    for j in 1..r.len() {
        let k = first + 6 * j;
        if !k.is_multiple_of(BLOCK) {
            assert_eq!(r[j] ^ r[j - 1], w(k) ^ w_prime((k % BLOCK) / 6));
            assert_eq!(r[j] ^ r[j - 1], s.w_p(k));
        } else {
            assert_eq!(r[j], s.w_p(k));
        }
    }
}

#[test]
fn frequency_interleaver_first_addresses() {
    let mut h = Vec::new();
    freq_addresses(false, L1_CELLS, &mut h);
    assert_eq!(h[..12], [0, 2048, 64, 2176, 1024, 2080, 256, 2050, 4, 2064, 512, 2049]);
    freq_addresses(true, L1_CELLS, &mut h);
    assert_eq!(h[..12], [0, 2048, 32, 2112, 4, 2176, 1024, 2304, 1, 2056, 16, 2050]);
}

// 2. Pilots.

fn wide_layout(guard: Guard) -> Layout {
    Layout {
        guard,
        k_min: 0,
        k_max: 10 * RESERVED_SPAN,
        notches: Vec::new(),
        reserved: false,
    }
}

const RESERVED_SPAN: usize = 8 * BLOCK;

#[test]
fn pilot_positions() {
    let l = wide_layout(Guard::G1_64);
    let pilot = |k: usize, sym: usize| l.class(k, sym) == Cell::Pilot;
    assert!(pilot(96, 0) && pilot(3366 + 3408 * 7, 3));
    assert!(!pilot(97, 1));
    for &c in &tables::CONTINUAL {
        assert_eq!(c % 6, 0);
    }
    let big = 48 * 1000;
    assert!(pilot(big, 0) && pilot(big + 12, 1) && pilot(big + 72, 2));
    assert!(!pilot(big + 12, 0) && !pilot(big + 13, 1));
    let l = wide_layout(Guard::G1_128);
    let pilot = |k: usize, sym: usize| l.class(k, sym) == Cell::Pilot;
    let big = 96 * 1000;
    assert!(pilot(big + 24, 1) && pilot(big + 96, 0));
    assert!(!pilot(big + 24, 0));
}

// 3. Preamble cell order.

#[test]
fn preamble_cells_are_a_bijection() {
    let mut seen = vec![false; L1_CELLS];
    for kk in 0..BLOCK {
        if let Some(q) = l1_cell(kk) {
            assert!(!seen[q]);
            seen[q] = true;
        } else {
            assert_eq!(kk % 6, 0);
        }
    }
    assert!(seen.iter().all(|&s| s));
}

// 4. Time interleaver.

#[test]
fn time_interleaver() {
    let mut one = TwistedBlock::new(1, 50).unwrap();
    let input: Vec<u32> = (0..50).collect();
    let mut out = vec![0u32; 50];
    one.interleave(None, &input, &mut out).unwrap();
    assert_eq!(out, input);
    let t = TwistedBlock::new(4, 50).unwrap();
    assert_eq!(t.position(1), (1, 1));
    assert_eq!(t.position(50), (1, 0));
    let mut seen = [false; 200];
    for i in 0..200 {
        let (r, c) = t.position(i);
        assert!(!seen[r * 50 + c]);
        seen[r * 50 + c] = true;
    }
}

// 5. Reed-Muller headers.

#[test]
fn reed_muller_code() {
    assert_eq!(rm25_encode(0), 0);
    assert_eq!(rm25_encode(0x8000), u32::MAX);
    let min = (1..=u16::MAX).map(|m| rm25_encode(m).count_ones()).min().unwrap();
    assert_eq!(min, 8);
    let d = Rm25Decoder::new();
    let mut rng = Rng::new(5);
    for _ in 0..300 {
        let msg = rng.next_u64() as u16;
        let cw = rm25_encode(msg);
        let mut soft = [0f32; 32];
        let flips: Vec<usize> = (0..3).map(|_| (rng.next_u64() % 32) as usize).collect();
        for (j, s) in soft.iter_mut().enumerate() {
            let bit = (cw >> (31 - j)) & 1 ^ u32::from(flips.contains(&j));
            *s = if bit == 0 { 1.0 } else { -1.0 };
        }
        assert_eq!(d.decode(&soft).unwrap().0, msg);
    }
}

#[test]
fn reed_muller_worked_headers() {
    let fec = FecFrameHeader {
        plp_id: 1,
        long: true,
        plp_mod: 3,
        plp_cod: 4,
        two: false,
    };
    // Clause 7.2.2: lambda, then v_i = lambda_((i - 2) mod 32) xor w^RM_i with w^RM = 0x1D8D3BF3.
    assert_eq!(W_RM_DEFAULT, 0x1D8D_3BF3);
    assert_eq!(fec.bits(), 0x01B8);
    assert_eq!(branches(0x01B8, W_RM_DEFAULT), (0x0069_3C55, 0x5D97_74E6));
    assert_eq!(preamble_bits(130, 0), 0x0208);
    assert_eq!(branches(0x0208, W_RM_DEFAULT), (0x0055_00AA, 0x9D98_7BD9));
    assert_eq!(branches(0x0000, W_RM_DEFAULT), (0, 0x1D8D_3BF3));
    assert_eq!(branches(0x8000, W_RM_DEFAULT), (0xFFFF_FFFF, 0xE272_C40C));
    // u_((i + 2) mod 32) = lambda_i, bit by bit.
    for bits in [0x01B8u16, 0x0208, 0x7FFF, 0x1234] {
        let (l, v) = branches(bits, W_RM_DEFAULT);
        let bit = |x: u32, i: usize| (x >> (31 - i)) & 1;
        for i in 0..32 {
            assert_eq!(bit(v, (i + 2) % 32) ^ bit(W_RM_DEFAULT, (i + 2) % 32), bit(l, i), "{bits:04X} {i}");
        }
    }
}

/// The header decoder on cells built from the literal words of the worked examples (not from `header::cells`).
#[test]
fn header_decoder_reads_the_worked_words() {
    let set = point_set(Qam::Q16);
    let qpsk = header::constellation(true);
    let mut dec = HeaderDecoder::new(W_RM_DEFAULT);
    for (bits, lambda, v) in [(0x01B8u16, 0x0069_3C55u32, 0x5D97_74E6u32), (0x0208, 0x0055_00AA, 0x9D98_7BD9)] {
        let bit = |x: u32, i: usize| ((x >> (31 - i)) & 1) as usize;
        // Robust: lambda_i on the real axis, v_i on the imaginary axis (bit 1 negative).
        let z: Vec<Complex32> = (0..32).map(|i| qpsk.points()[(bit(lambda, i) << 1) | bit(v, i)]).collect();
        let p = qpsk.points()[0b01];
        assert!(p.re > 0.0 && p.im < 0.0, "QPSK label 01 is +re, -im");
        let w = vec![10.0; 32];
        let d = dec.decode(&z, &w, true).unwrap();
        assert_eq!((d.bits, d.agree), (bits, 32));
        assert!(dec.candidate(&z, &w, true) > 0.99);
        // High efficiency: (y0 y1 y2 y3) = (lambda_2i, lambda_2i+1, v_2i, v_2i+1).
        let z: Vec<Complex32> = (0..16)
            .map(|i| {
                set.points()[(bit(lambda, 2 * i) << 3) | (bit(lambda, 2 * i + 1) << 2) | (bit(v, 2 * i) << 1) | bit(v, 2 * i + 1)]
            })
            .collect();
        let d = dec.decode(&z, &[10.0; 16], false).unwrap();
        assert_eq!((d.bits, d.agree), (bits, 32));
    }
}

/// Headers through white noise at `esn0_db`: at most `max_wrong` of `n` decode wrong.
fn headers_through_noise(robust: bool, esn0_db: f32, n: usize, max_wrong: usize) {
    let cons = header::constellation(robust);
    let mut dec = HeaderDecoder::new(W_RM_DEFAULT);
    let mut rng = Rng::new(if robust { 11 } else { 12 });
    let n0 = 10f32.powf(-esn0_db / 10.0);
    let mut wrong = 0;
    let mut cells = Vec::new();
    for _ in 0..n {
        let bits = rng.next_u64() as u16;
        header::cells(bits, robust, &cons, W_RM_DEFAULT, &mut cells);
        noisy(&mut rng, &mut cells, n0);
        let w = vec![1.0 / n0; cells.len()];
        if dec.decode(&cells, &w, robust).map(|d| d.bits) != Some(bits) {
            wrong += 1;
        }
    }
    assert!(wrong <= max_wrong, "{wrong} of {n} headers wrong at {esn0_db} dB");
}

#[test]
fn headers_through_noise_qpsk_and_16qam() {
    headers_through_noise(true, 8.0, 60, 1);
    headers_through_noise(false, 14.0, 60, 1);
}

#[test]
fn random_cells_rarely_look_like_a_header() {
    let cons = point_set(Qam::Q256);
    let mut dec = HeaderDecoder::new(W_RM_DEFAULT);
    let mut rng = Rng::new(21);
    let mut hits = 0;
    for _ in 0..100 {
        let z: Vec<Complex32> = (0..32)
            .map(|_| cons.points()[(rng.next_u64() % 256) as usize])
            .collect();
        let w = vec![100.0; 32];
        let looks = dec.candidate(&z, &w, true) >= 0.45
            && dec.decode(&z, &w, true).is_some_and(|d| {
                let h = FecFrameHeader::from_bits(d.bits);
                d.agree >= 22 && (h.plp_mod == 0 && h.long || header_modcod(&h).is_some_and(|m| m.valid()))
            });
        hits += usize::from(looks);
    }
    assert!(hits <= 3, "{hits} of 100");
}

// 6. Header fields.

#[test]
fn header_fields_pack_and_unpack() {
    assert_eq!(preamble_fields(preamble_bits(1234, 2)), (1234, 2));
    let h = FecFrameHeader {
        plp_id: 200,
        long: false,
        plp_mod: 3,
        plp_cod: 5,
        two: true,
    };
    assert_eq!(FecFrameHeader::from_bits(h.bits()), h);
}

// 7. XFECFrame lengths.

#[test]
fn xfecframe_lengths() {
    assert_eq!(xfecframe_len(true, 3), Some(8100));
    assert_eq!(xfecframe_len(false, 1), Some(4050));
    assert_eq!(xfecframe_len(true, 0), Some(900));
    assert_eq!(xfecframe_len(false, 5), Some(1350));
}

// 8. L1 part 2.

fn one_slice(guard: Guard, offsets: (i16, i16), plps: usize) -> L1Part2 {
    let plp = |id: u8| Plp {
        id,
        plp_type: 2,
        payload: 3,
        start: 77,
        long: true,
        plp_mod: 3,
        plp_cod: 4,
        ts_id: 0x1234,
        on_id: 0x2345,
        ..Plp::default()
    };
    L1Part2 {
        network_id: 0x3001,
        system_id: 7,
        start: 153_300,
        bandwidth: 284,
        guard,
        frame_length: 448,
        change_counter: 0,
        slices: vec![Slice {
            id: 3,
            tune_pos: 142,
            offset_left: offsets.0,
            offset_right: offsets.1,
            ti_depth: 1,
            plps: (0..plps).map(|i| plp(i as u8 + 1)).collect(),
            ..Slice::default()
        }],
        notches: Vec::new(),
        reserved_tones: false,
        ews: false,
        version: 1,
    }
}

#[test]
fn l1_fields_and_sizes() {
    let l1 = one_slice(Guard::G1_64, (-142, 142), 1);
    assert_eq!(l1.field_bits().len(), 260);
    assert_eq!(l1.info_size(), 130);
    // 1/128: tuning position, offsets one bit shorter each.
    let mut l2 = one_slice(Guard::G1_128, (-71, 71), 1);
    l2.slices[0].tune_pos = 71;
    assert_eq!(l2.field_bits().len(), 257);
    assert_eq!(l2.to_bits().len(), 258 + 32);
    for (guard, l) in [(Guard::G1_64, one_slice(Guard::G1_64, (-30, 26), 2)), (Guard::G1_128, l2)] {
        let mut l = l;
        l.notches.push(Notch { start: 40, width: 3 });
        l.slices[0].type2 = guard == Guard::G1_128;
        if l.slices[0].type2 {
            for p in &mut l.slices[0].plps {
                (p.start, p.long, p.plp_mod, p.plp_cod) = (0, false, 0, 0);
            }
        }
        let back = L1Part2::from_bits(&l.to_bits()).unwrap();
        assert_eq!(back, l);
    }
    // Carriers of a slice and a notch (clause 9.4.1).
    let l = one_slice(Guard::G1_64, (-30, 26), 1);
    let s = &l.slices[0];
    assert_eq!(s.carriers(153_300, 12), Some((153_300 + 12 * 112, 153_300 + 12 * 168 - 1)));
    let n = Notch { start: 40, width: 3 };
    assert_eq!(n.carriers(153_300, 12), Some((153_300 + 481, 153_300 + 515)));
    assert_eq!(Notch { start: 40, width: 0 }.carriers(153_300, 12), None);
    // Worked sizes of 5.4.6.
    let z = L1Sizes::new(130, 0).unwrap();
    assert_eq!((z.blocks, z.k_sig, z.n_l1, z.n_punc, z.cells_per_block()), (1, 292, 1376, 8084, 344));
    let z = L1Sizes::new(130, 2).unwrap();
    assert_eq!((z.n_l1, z.n_punc, z.cells_per_block()), (1376, 8084, 344));
}

#[test]
fn l1_coding_decodes_at_9_db_and_refuses_damage() {
    let mut rng = Rng::new(31);
    let n0 = 10f32.powf(-0.9);
    for plps in [1usize, 62] {
        let l1 = one_slice(Guard::G1_64, (-142, 142), plps);
        let bits = l1.to_bits();
        for mode in 0..4u8 {
            let sizes = L1Sizes::new(l1.info_size(), mode).unwrap();
            if plps > 1 {
                assert_eq!(sizes.blocks, 2);
            }
            let mut codec = L1Codec::new(sizes).unwrap();
            let mut rows = Vec::new();
            codec.encode(&bits, &mut rows).unwrap();
            assert_eq!(rows.len(), sizes.symbols);
            let mut z: Vec<Complex32> = rows.concat();
            noisy(&mut rng, &mut z, n0);
            let wts = vec![1.0 / n0; z.len()];
            let mut out = Vec::new();
            assert_eq!(codec.decode(&z, &wts, &mut out), Some(true), "{plps} PLPs, mode {mode}");
            assert_eq!(L1Part2::from_bits(&out), Some(l1.clone()));
            let mut bad = out.clone();
            bad[40] ^= 1;
            assert_eq!(L1Part2::from_bits(&bad), None);
        }
    }
}

// 9. Constellations.

/// The level of an axis label (sign bit first, then the reflected Gray code of the distance from the outermost point).
fn rule_level(bits: &[u8]) -> i32 {
    let b = bits.len() as i32;
    let mut g = 0u32;
    for &x in &bits[1..] {
        g = (g << 1) | u32::from(x);
    }
    let mut d = g;
    let mut s = g >> 1;
    while s > 0 {
        d ^= s;
        s >>= 1;
    }
    let mag = (1 << b) - 1 - 2 * d as i32;
    if bits[0] == 1 {
        -mag
    } else {
        mag
    }
}

#[test]
fn constellations_follow_the_rule() {
    for qam in [Qam::Q16, Qam::Q64, Qam::Q256, Qam::Q1024, Qam::Q4096] {
        let set = point_set(qam);
        let m = qam.bits();
        let norm = (2.0 * ((1u32 << m) - 1) as f64 / 3.0).sqrt() as f32;
        let mut power = 0f64;
        for (label, p) in set.points().iter().enumerate() {
            let bit = |i: usize| ((label >> (m - 1 - i)) & 1) as u8;
            let re: Vec<u8> = (0..m).step_by(2).map(bit).collect();
            let im: Vec<u8> = (1..m).step_by(2).map(bit).collect();
            assert!((p.re * norm - rule_level(&re) as f32).abs() < 1e-3, "{qam:?} {label}");
            assert!((p.im * norm - rule_level(&im) as f32).abs() < 1e-3, "{qam:?} {label}");
            power += p.norm_sqr() as f64;
        }
        assert!((power / set.points().len() as f64 - 1.0).abs() < 1e-4);
        // Gray: neighbouring levels on an axis differ in one bit.
        let half = m / 2;
        let mut by_level: Vec<(i32, u32)> = (0..1u32 << half)
            .map(|g| {
                let bits: Vec<u8> = (0..half).map(|i| ((g >> (half - 1 - i)) & 1) as u8).collect();
                (rule_level(&bits), g)
            })
            .collect();
        by_level.sort();
        for w2 in by_level.windows(2) {
            assert_eq!((w2[0].1 ^ w2[1].1).count_ones(), 1);
        }
    }
    let lv = |s: &str| rule_level(&s.bytes().map(|c| c - b'0').collect::<Vec<u8>>());
    assert_eq!([lv("10000"), lv("10001"), lv("10011"), lv("11000"), lv("01000"), lv("00000")], [-31, -29, -27, -1, 1, 31]);
    assert_eq!([lv("100000"), lv("100001"), lv("101100"), lv("000000")], [-63, -61, -47, 63]);
    // The first demultiplexed word.
    let first_bit = |m: ModCod| bit_cells(&m).unwrap().dest(0).unwrap();
    let m1024 = ModCod { long: true, qam: Qam::Q1024, rate: CodeRate::R3_4 };
    assert_eq!(first_bit(m1024), 8);
    let m4096 = ModCod { long: false, qam: Qam::Q4096, rate: CodeRate::R5_6 };
    assert_eq!(first_bit(m4096), 10);
}

// 10. Reserved tones.

#[test]
fn reserved_tones() {
    assert_eq!(tables::RESERVED_S0.len(), 288);
    for guard in [Guard::G1_64, Guard::G1_128] {
        let dx = guard.d_x();
        assert!(reserved_position(161, 0, guard));
        assert!(reserved_position(161 + dx, 1, guard));
        assert!(reserved_position(161 + 8 * BLOCK, 0, guard));
        assert!(!reserved_position(162, 0, guard));
        for l in 0..4 {
            let count = (0..RESERVED_SPAN).filter(|&k| reserved_position(k + 10 * RESERVED_SPAN, l, guard)).count();
            assert_eq!(count, 288);
            for k in 0..RESERVED_SPAN {
                if reserved_position(k, l, guard) {
                    assert_ne!(k % (4 * dx), dx * l, "{k} {l}");
                }
            }
        }
    }
}

// The FEC tables against table 3.

#[test]
fn fec_parameters() {
    let rows: [(bool, Option<CodeRate>, usize, usize, usize); 11] = [
        (true, Some(CodeRate::R2_3), 43_040, 43_200, 10),
        (true, Some(CodeRate::R3_4), 48_408, 48_600, 12),
        (true, Some(CodeRate::R4_5), 51_648, 51_840, 12),
        (true, Some(CodeRate::R5_6), 53_840, 54_000, 10),
        (true, Some(CodeRate::R9_10), 58_192, 58_320, 8),
        (false, None, 7032, 7200, 12),
        (false, Some(CodeRate::R2_3), 10_632, 10_800, 12),
        (false, Some(CodeRate::R3_4), 11_712, 11_880, 12),
        (false, Some(CodeRate::R4_5), 12_432, 12_600, 12),
        (false, Some(CodeRate::R5_6), 13_152, 13_320, 12),
        (false, Some(CodeRate::R9_10), 14_232, 14_400, 12),
    ];
    for (long, rate, k_bch, k_ldpc, t) in rows {
        let f = tables::fec(long, rate);
        assert_eq!((f.k_bch, f.k_ldpc, f.bch.t, f.n), (k_bch, k_ldpc, t, if long { 64_800 } else { 16_200 }));
    }
}

/// FEC blocks of `m` through white noise at `esn0_db`: how many of `n` decode.
fn blocks_through_noise(m: ModCod, esn0_db: f32, n: usize, seed: u64) -> usize {
    let mut enc = PlpEncoder::new(m).unwrap();
    let mut dec = PlpDecoder::new(m, 50).unwrap();
    let mut rng = Rng::new(seed);
    let n0 = 10f32.powf(-esn0_db / 10.0);
    let mut ok = 0;
    let mut frame = vec![0u8; enc.frame_bytes()];
    let mut cells = vec![Complex32::default(); m.cells()];
    let mut back = vec![0u8; frame.len()];
    for _ in 0..n {
        for b in frame.iter_mut() {
            *b = rng.next_u64() as u8;
        }
        enc.encode(&frame, &mut cells).unwrap();
        noisy(&mut rng, &mut cells, n0);
        let w = vec![1.0 / n0; cells.len()];
        let r = dec.decode(&cells, &w, &mut back).unwrap();
        ok += usize::from(r.bch_ok && back == frame);
    }
    ok
}

#[test]
fn large_constellations_decode_at_their_targets() {
    let m = |long, qam, rate| ModCod { long, qam, rate };
    let cases = [
        (m(true, Qam::Q1024, CodeRate::R3_4), 27.0, true),
        (m(false, Qam::Q1024, CodeRate::R5_6), 30.0, true),
        (m(true, Qam::Q4096, CodeRate::R5_6), 33.5, true),
        (m(false, Qam::Q4096, CodeRate::R4_5), 32.5, true),
        (m(true, Qam::Q1024, CodeRate::R3_4), 21.0, false),
        (m(true, Qam::Q4096, CodeRate::R5_6), 27.0, false),
    ];
    for (i, (mc, snr, pass)) in cases.into_iter().enumerate() {
        let n = 4;
        let ok = blocks_through_noise(mc, snr, n, 40 + i as u64);
        if pass {
            assert_eq!(ok, n, "{} at {snr} dB", mc.text());
        } else {
            assert_eq!(ok, 0, "{} at {snr} dB", mc.text());
        }
    }
}

#[test]
fn every_coding_round_trips_without_noise() {
    for long in [true, false] {
        for qam in [Qam::Q16, Qam::Q64, Qam::Q256, Qam::Q1024, Qam::Q4096] {
            for rate in [CodeRate::R2_3, CodeRate::R3_4, CodeRate::R4_5, CodeRate::R5_6, CodeRate::R9_10] {
                let mc = ModCod { long, qam, rate };
                if mc.valid() {
                    assert_eq!(blocks_through_noise(mc, 60.0, 1, 3), 1, "{}", mc.text());
                }
            }
        }
    }
}


// Round trips.

use sdrkit_algo_sim::chain::{Chain, ChainConfig, SignalInfo as ChainSignal};
use sdrkit_algo_sim::fading::profiles::{EchoPath, Profile};
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::Generator;
use sdrkit_proto_ipcast::adapt::UpDeframer;
use sdrkit_proto_ipcast::bbheader::{BbHeader, Mode, System};
use sdrkit_proto_ipcast::gse::GseReader;
use sdrkit_proto_ipcast::merge::{is_null, CommonMerge, MergeStats};
use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck};
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_stack_dvbcs::dvbc2::gen::{
    gcs_test_byte, gse_test_packet, C2Generator, GenParams, GroupInput, COMMON_PLP, GFPS_SYNC, GROUP_SHARED_PID,
};
use sdrkit_stack_dvbcs::dvbc2::rx::HALF_WINDOW;
use sdrkit_stack_dvbcs::dvbc2::decode::{BbFrame, C2Decoder};
use sdrkit_stack_dvbcs::dvbc2::rx::{C2Receiver, RxConfig, Status};

/// The impairments of a round trip.
#[derive(Clone, Copy, Debug, Default)]
struct Channel {
    esn0_db: Option<f64>,
    offset_hz: f64,
    clock_ppm: f64,
    echo: Option<(f64, f64)>,
}

/// What a round trip delivered.
struct Trip {
    status: Status,
    /// The numbered packets of each PLP stream (stream id, check).
    ts: NumberedCheck,
    /// The numbers of the good numbered packets, in order.
    numbers: Vec<u32>,
    /// IP packets in order.
    ip: Vec<Vec<u8>>,
    frames_ok: u64,
    /// Good BBFRAMEs of MODE 1.
    hem_frames: u64,
    /// The transport packets of the common PLP.
    common_ts: Vec<[u8; 188]>,
    /// The counters of the merge of the common PLP into the data PLP.
    merge: MergeStats,
    /// The transport stream delivered: the data PLP with its common PLP put back.
    merged: Vec<[u8; 188]>,
    /// The numbers of the good GFPS user packets.
    gfps: Vec<u32>,
    /// The data fields of the good GCS BBFRAMEs.
    gcs: Vec<Vec<u8>>,
}

/// The test signal of `p` at `rate` through `ch` for `seconds`, into the receiver with `cfg`.
fn round_trip(p: GenParams, ch: Channel, rate: f64, seconds: f64, cfg: RxConfig) -> Trip {
    let mut g = C2Generator::new(p, rate).unwrap();
    let info = g.signal_info().unwrap();
    let sig = ChainSignal {
        rate_hz: rate,
        power: 1.0,
        bandwidth_hz: Some(info.bandwidth_hz),
        symbol_rate_hz: info.symbol_rate_hz,
        bit_rate_hz: None,
    };
    let chain_cfg = ChainConfig {
        offset_hz: ch.offset_hz,
        clock_ppm: ch.clock_ppm,
        noise: match ch.esn0_db {
            Some(e) => NoiseLevel::EsN0Db {
                esn0_db: e,
                symbol_rate_hz: info.symbol_rate_hz.unwrap(),
            },
            None => NoiseLevel::Off,
        },
        fading: match ch.echo {
            Some((delay, level)) => Profile::echoes(&[EchoPath::new(delay, level, 30.0)]).unwrap(),
            None => Profile::None,
        },
        ..ChainConfig::default()
    };
    let mut chain = Chain::new(&chain_cfg, &sig).unwrap();
    let mut rx = C2Receiver::new(RxConfig { rate_hz: rate, centre_hz: p.freq_hz, raster: p.raster, ..cfg });
    let mut dec = C2Decoder::default();
    let mut cells = Vec::new();
    let mut deframer = UpDeframer::new();
    let mut gse = GseReader::default();
    let mut trip = Trip {
        status: Status::default(),
        ts: NumberedCheck::default(),
        numbers: Vec::new(),
        ip: Vec::new(),
        frames_ok: 0,
        hem_frames: 0,
        common_ts: Vec::new(),
        merge: MergeStats::default(),
        merged: Vec::new(),
        gfps: Vec::new(),
        gcs: Vec::new(),
    };
    let mut common = UpDeframer::new();
    // The transport stream is the data PLP with its common PLP put back, as `ipcast.bbframe` makes it.
    let merge = std::cell::RefCell::new(CommonMerge::default());
    let mut gfps = UpDeframer::new();
    let total = (seconds * rate) as usize;
    let mut x = vec![Complex32::default(); 65536];
    let mut y = Vec::new();
    let mut done = 0;
    let mut take = |f: BbFrame<'_>, trip: &mut Trip| {
        let merge = &mut *merge.borrow_mut();
        if !f.ok {
            deframer.reset();
            gse.reset();
            common.reset();
            merge.reset();
            gfps.reset();
            return;
        }
        trip.frames_ok += 1;
        let Ok(h) = BbHeader::parse(f.bytes) else { return };
        let ts = |merge: &mut CommonMerge, trip: &mut Trip| {
            merge.drain(|p, _| {
                trip.merged.push(*p);
                trip.ts.push(&Numbered::SATELLITE_CABLE, p, 0.0);
                if p[1] & 0x80 == 0 {
                    if let Some(n) = Numbered::SATELLITE_CABLE.number_of(p) {
                        trip.numbers.push(n);
                    }
                }
            });
            trip.merge = merge.stats;
        };
        if f.common {
            let _ = common.push_frame(f.bytes, &mut |u| {
                if let Ok(a) = u.bytes.try_into() {
                    trip.common_ts.push(a);
                }
                merge.push_common(u.bytes, u.crc_ok);
            });
            ts(merge, trip);
            return;
        }
        match f.payload {
            0 => {
                let _ = gfps.push_frame(f.bytes, &mut |u| {
                    if u.crc_ok && u.bytes[0] == GFPS_SYNC {
                        trip.gfps.push(u32::from_be_bytes([u.bytes[1], u.bytes[2], u.bytes[3], u.bytes[4]]));
                    }
                });
                return;
            }
            1 => {
                let n = h.data_field_len().unwrap_or(0);
                trip.gcs.push(f.bytes[10..10 + n].to_vec());
                return;
            }
            _ => {}
        }
        trip.hem_frames += u64::from(h.mode == Mode::HighEfficiency);
        if h.carries_gse() {
            let n = h.data_field_len().unwrap_or(0);
            let df = &f.bytes[10..10 + n];
            if h.gse_sliced(System::DvbT2) {
                gse.push_sliced(df, h.syncd, &mut |pdu| trip.ip.push(pdu.pdu.to_vec()));
            } else {
                gse.push(df, &mut |pdu| trip.ip.push(pdu.pdu.to_vec()));
            }
        } else {
            let _ = deframer.push_frame(f.bytes, &mut |u| merge.push_data(u.bytes, u.crc_ok));
            ts(merge, trip);
        }
    };
    while done < total {
        let n = g.generate(&mut x);
        y.clear();
        chain.process(&x[..n], &mut y);
        rx.feed(&y, &mut cells);
        // The merge waits for the common PLP only when the receiver decodes one, as `ipcast.bbframe` does.
        merge.borrow_mut().set_common(rx.status.common.is_some());
        for c in cells.drain(..) {
            dec.push(&c.value, c.time, &mut |f| take(f, &mut trip));
        }
        done += n;
    }
    dec.finish(&mut |f| take(f, &mut trip));
    for l in rx.take_log() {
        println!("{l}");
    }
    println!("{:?}", dec.status);
    trip.status = rx.status.clone();
    trip
}

/// The transport stream of a round trip: losses only among the first 400 packets, no wrong or flagged packet, and at least `min`
/// packets.
fn ts_ok(t: &Trip, min: u64) {
    println!("{:?}\n{:?}", t.status, t.ts);
    assert_eq!((t.ts.wrong, t.ts.flagged), (0, 0));
    let n = &t.numbers;
    assert!(n.first().is_some_and(|&f| f <= 400), "first packet {:?}", n.first());
    for w in n.windows(2) {
        assert!(w[1] == w[0] + 1 || w[1] <= 400, "packets {} to {} lost", w[0] + 1, w[1] - 1);
    }
    assert!(t.ts.good >= min, "{} packets", t.ts.good);
}

/// The IP packets of a round trip: byte-exact, in order, losses only among the first 100, at least `min` of them.
fn ip_ok(t: &Trip, min: usize) {
    println!("{:?}\n{} IP packets", t.status, t.ip.len());
    let mut want = Vec::new();
    let mut next = 0u32;
    for (i, got) in t.ip.iter().enumerate() {
        let found = (next..=next.max(100)).find(|&n| {
            gse_test_packet(n, &mut want);
            &want == got
        });
        match found {
            Some(n) => {
                assert!(n == next || n <= 100, "IP packets {next} to {} lost", n - 1);
                next = n + 1;
            }
            None => panic!("IP packet {i} is not the next test packet {next}"),
        }
    }
    assert!(t.ip.len() >= min, "{} IP packets", t.ip.len());
}

fn params(f: impl FnOnce(&mut GenParams)) -> GenParams {
    let mut p = GenParams::default();
    f(&mut p);
    p
}

use sdrkit_stack_dvbcs::dvbc2::settings::{GenModCod, GenSliceType, GenStream, GenSystem};
use sdrkit_stack_dvbcs::dvbc2::signal::Raster;

#[test]
fn round_trip_type1_64qam_offsets_and_clock() {
    let p = params(|p| p.modcod = GenModCod::Q64R23.modcod(true));
    let ch = Channel {
        esn0_db: Some(18.0),
        offset_hz: 18.3e3,
        clock_ppm: 25.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    ts_ok(&t, 10_000);
}

#[test]
fn round_trip_type2_two_plps_echo() {
    let p = params(|p| {
        p.guard = Guard::G1_128;
        p.slice = GenSliceType::Type2Robust;
        p.ti_depth = 1;
        p.system = GenSystem::TwoPlps;
    });
    let ch = Channel {
        esn0_db: Some(31.0),
        offset_hz: -7e3,
        clock_ppm: -20.0,
        echo: Some((1.8e-6, -15.0)),
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig { plp: Some(2), ..RxConfig::default() });
    assert_eq!(t.status.chosen, Some((0, 2)));
    ts_ok(&t, 5_000);
}

#[test]
fn round_trip_type2_efficient_16k_notch_native_rate_centre_unknown() {
    let p = params(|p| {
        p.slice = GenSliceType::Type2Efficient;
        p.modcod = GenModCod::Q16R45.modcod(false);
        p.ti_depth = 3;
        p.notch = true;
    });
    let ch = Channel {
        esn0_db: Some(16.0),
        offset_hz: 3.1e3,
        clock_ppm: 12.0,
        echo: None,
    };
    let t = round_trip(p, ch, Raster::R8.rate(), 1.2, RxConfig { centre_known: false, ..RxConfig::default() });
    ts_ok(&t, 5_000);
}

#[test]
fn round_trip_1024qam_reserved_tones_echo() {
    let p = params(|p| {
        p.modcod = GenModCod::Q1024R34.modcod(true);
        p.reserved_tones = true;
        p.ti_depth = 1;
    });
    let ch = Channel {
        esn0_db: Some(30.0),
        offset_hz: 9e3,
        clock_ppm: 15.0,
        echo: Some((1.0e-6, -20.0)),
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    ts_ok(&t, 10_000);
}

#[test]
fn round_trip_4096qam_gse() {
    let p = params(|p| {
        p.slice = GenSliceType::Type2Robust;
        p.modcod = GenModCod::Q4096R56.modcod(false);
        p.stream = GenStream::Gse;
    });
    let ch = Channel {
        esn0_db: Some(37.0),
        offset_hz: -4e3,
        clock_ppm: -10.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    ip_ok(&t, 1_000);
}

#[test]
fn round_trip_type1_gse_notch_reserved_tones_ti8() {
    let p = params(|p| {
        p.guard = Guard::G1_128;
        p.modcod = GenModCod::Q64R23.modcod(true);
        p.stream = GenStream::Gse;
        p.notch = true;
        p.reserved_tones = true;
        p.ti_depth = 2;
    });
    let ch = Channel {
        esn0_db: Some(19.0),
        offset_hz: 12.5e3,
        clock_ppm: 20.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    ip_ok(&t, 500);
}

#[test]
fn round_trip_6mhz_raster() {
    let p = params(|p| {
        p.raster = Raster::R6;
        p.freq_hz = 123e6;
        p.modcod = GenModCod::Q64R23.modcod(true);
        p.ti_depth = 1;
    });
    let ch = Channel {
        esn0_db: Some(19.0),
        offset_hz: 6.5e3,
        clock_ppm: 15.0,
        echo: None,
    };
    let t = round_trip(p, ch, 7e6, 1.6, RxConfig::default());
    // The window centre is the carrier sent at the tuned frequency less the offset: 6.5 kHz is 3.9 carriers of 1674 Hz.
    let k = t.status.window_carrier.unwrap() as f64;
    assert!((k - (123e6 - 6.5e3) / Raster::R6.spacing()).abs() < 1.0, "carrier {k}");
    assert!((t.status.cfo_hz - 6.5e3).abs() < 200.0, "{} Hz", t.status.cfo_hz);
    ts_ok(&t, 6_000);
}

#[test]
fn round_trip_high_efficiency_mode_ts_and_gse() {
    let p = params(|p| {
        p.hem = true;
        p.numbered = true;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let ch = Channel {
        esn0_db: Some(18.0),
        offset_hz: -5e3,
        clock_ppm: 10.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.0, RxConfig::default());
    assert!(t.hem_frames > 0 && t.hem_frames == t.frames_ok, "{} of {}", t.hem_frames, t.frames_ok);
    ts_ok(&t, 8_000);
    let p = params(|p| {
        p.hem = true;
        p.stream = GenStream::Gse;
        p.slice = GenSliceType::Type2Robust;
    });
    let ch = Channel {
        esn0_db: Some(30.0),
        ..ch
    };
    let t = round_trip(p, ch, 10e6, 1.0, RxConfig::default());
    assert!(t.hem_frames > 0 && t.hem_frames == t.frames_ok);
    ip_ok(&t, 500);
}

/// Numbers that run on by one after the first `skip`, at least `min` of them.
fn consecutive(n: &[u32], skip: usize, min: usize) {
    assert!(n.len() >= min, "{} packets", n.len());
    for w in n[skip.min(n.len())..].windows(2) {
        assert_eq!(w[1], w[0] + 1, "packets {} to {} lost", w[0] + 1, w[1]);
    }
}

#[test]
fn round_trip_gfps_and_gcs() {
    let ch = Channel {
        esn0_db: Some(19.0),
        offset_hz: 8e3,
        clock_ppm: -12.0,
        echo: None,
    };
    let p = params(|p| {
        p.stream = GenStream::Gfps;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let t = round_trip(p, ch, 10e6, 1.0, RxConfig::default());
    println!("{:?}", t.status);
    // 96-byte user packets: the PLP carries about 40 Mbit/s.
    consecutive(&t.gfps, 0, 30_000);
    assert!(t.ip.is_empty() && t.ts.good == 0);
    let p = params(|p| {
        p.stream = GenStream::Gcs;
        p.slice = GenSliceType::Type2Robust;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let t = round_trip(p, ch, 10e6, 1.0, RxConfig::default());
    assert!(t.gcs.len() >= 400, "{} data fields", t.gcs.len());
    let all = t.gcs.concat();
    let at = (0..4u64)
        .find(|&a| {
            let c = u32::from_be_bytes(all[a as usize..a as usize + 4].try_into().unwrap());
            let first = 4 * u64::from(c) - a;
            all.iter().enumerate().all(|(i, &b)| b == gcs_test_byte(first + i as u64))
        })
        .is_some();
    assert!(at, "the GCS bytes do not run on");
}

#[test]
fn round_trip_common_plp() {
    let p = params(|p| {
        p.system = GenSystem::Common;
        p.numbered = true;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let ch = Channel {
        esn0_db: Some(20.0),
        offset_hz: -9e3,
        clock_ppm: 8.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    assert_eq!((t.status.chosen, t.status.common), (Some((0, 1)), Some(COMMON_PLP)));
    println!("{} common packets, merge {:?}", t.common_ts.len(), t.merge);
    let c = &t.common_ts;
    let common_share = c.iter().filter(|p| !is_null(&p[..])).count() as f64 / c.len() as f64;
    assert!((common_share - 0.25).abs() < 0.02, "{common_share}");
    // The whole merged stream: the numbered packets of the test stream from the start, about a quarter of them out of the
    // common PLP (annex D of EN 302 755).
    ts_ok(&t, 10_000);
    assert!(t.merge.common as f64 > 0.2 * t.ts.good as f64, "{:?}", t.merge);
}

#[test]
fn round_trip_bundled_plp() {
    let p = params(|p| {
        p.system = GenSystem::Bundled;
        p.numbered = true;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let ch = Channel {
        esn0_db: Some(20.0),
        offset_hz: 4e3,
        clock_ppm: -15.0,
        echo: None,
    };
    let t = round_trip(p, ch, 10e6, 1.2, RxConfig::default());
    assert_eq!(t.status.bundled_slices, 2);
    assert!(t.hem_frames > 0);
    ts_ok(&t, 10_000);
    // Without the hold the frames of the two slices go out of order.
    let t = round_trip(p, ch, 10e6, 0.6, RxConfig { bundle_hold: 0, ..RxConfig::default() });
    let back = t.numbers.windows(2).filter(|w| w[1] <= w[0]).count();
    assert!(back > 0, "no packet out of order without the hold");
}

#[test]
fn round_trip_slice_outside_the_window() {
    let p = params(|p| {
        p.system = GenSystem::TwoSlices;
        p.numbered = true;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let ch = Channel {
        esn0_db: Some(20.0),
        offset_hz: 3e3,
        clock_ppm: 10.0,
        echo: None,
    };
    // 20 Msps hold the whole 15.2 MHz system: the window moves onto slice 1 at 3.8 MHz.
    let t = round_trip(p, ch, 20e6, 1.2, RxConfig { plp: Some(2), ..RxConfig::default() });
    let l1 = p.l1();
    let tune = l1.slices[1].tune_carrier(l1.k_min(), p.guard.d_x());
    assert_eq!(t.status.chosen, Some((1, 2)));
    assert_eq!(t.status.window_carrier, Some(tune));
    assert!((t.status.cfo_hz - 3e3).abs() < 100.0, "{} Hz", t.status.cfo_hz);
    // The L1 that names slice 1 comes in the preamble of a frame whose data symbols follow at once, before the window has
    // moved: the data of that frame is lost, and the receiver goes on from the next preamble. So the first packet comes at
    // most the packets of one C2 frame of slice 1 later than the 400 that the other round trips allow, with no loss after it.
    let layout = p.layout();
    let ds = l1.slices[1].carriers(l1.k_min(), p.guard.d_x()).unwrap();
    let mut cells = Vec::new();
    let frame_cells: usize = (0..sdrkit_stack_dvbcs::dvbc2::signal::DATA_SYMBOLS)
        .map(|l| {
            sdrkit_stack_dvbcs::dvbc2::gen::slice_payload(&layout, ds, l, p.ti_full_width, &mut cells);
            cells.len()
        })
        .sum();
    // A data slice packet of type 2: the robust header of 32 cells and one XFECFrame.
    let blocks = frame_cells as f64 / (p.modcod.cells() + 32) as f64;
    let per_frame = (blocks * (p.modcod.fec().k_bch / 8 - 10) as f64 / 188.0).ceil() as u32;
    let first = t.numbers.first().copied();
    println!("first packet {first:?}, {per_frame} packets per frame, {} packets", t.numbers.len());
    assert!(first.is_some_and(|f| f <= per_frame + 400), "first packet {first:?}");
    assert_eq!((t.ts.wrong, t.ts.flagged), (0, 0));
    consecutive(&t.numbers, 0, 8_000);
    // 10 Msps do not hold it: the receiver names the frequency to tune to.
    let t = round_trip(p, ch, 10e6, 0.4, RxConfig { plp: Some(2), ..RxConfig::default() });
    assert_eq!(t.status.retune_hz, Some(tune as f64 * Raster::R8.spacing()));
    assert_eq!(t.status.chosen, None);
}

/// The transport stream `got` is the input stream of data PLP `plp` of the group, byte for byte, from a packet among its
/// first `settle` to its end, and at least `min` packets long from there. Returns the PIDs it holds.
fn equals_group_input(got: &[[u8; 188]], plp: u8, settle: usize, min: usize) -> Vec<u16> {
    let mut src = GroupInput::new(plp);
    let input: Vec<[u8; 188]> = (0..got.len() + 40_000)
        .map(|_| {
            let mut p = [0u8; 188];
            src.next_packet(&mut p);
            p
        })
        .collect();
    let last = got.last().expect("no packet");
    let at = input.iter().position(|p| p == last).expect("the last packet is not a packet of the input");
    // The length of the run that ends with the last packet and equals the input.
    let run = got.iter().rev().zip(input[..=at].iter().rev()).take_while(|(a, b)| a == b).count();
    let first = got.len() - run;
    println!("PLP {plp}: {} packets, equal to the input from packet {first}, input slot {}", got.len(), at + 1 - run);
    assert!(first <= settle, "packet {} differs from the input", first.saturating_sub(1));
    assert!(run >= min, "{run} packets");
    let pid = |p: &[u8; 188]| (u16::from(p[1]) & 0x1F) << 8 | u16::from(p[2]);
    let mut pids: Vec<u16> = got[first..].iter().map(pid).collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[test]
fn round_trip_type1_and_type2_slices_of_one_system() {
    let ch = Channel {
        esn0_db: Some(20.0),
        offset_hz: 5e3,
        clock_ppm: 10.0,
        echo: None,
    };
    let system = |header| {
        params(|p| {
            p.system = GenSystem::MixedSlices;
            p.slice = header;
            p.numbered = true;
            p.modcod = GenModCod::Q64R23.modcod(true);
        })
    };
    // The first data PLP: PLP 1 in the type 1 slice.
    let t = round_trip(system(GenSliceType::Type1), ch, 10e6, 1.0, RxConfig::default());
    assert_eq!(t.status.chosen, Some((0, 1)));
    ts_ok(&t, 3_000);
    // PLP 2 in the type 2 slice, with each form of the FECFrame header.
    for header in [GenSliceType::Type2Robust, GenSliceType::Type2Efficient] {
        let p = system(header);
        assert_eq!(p.l1().slices[1].efficient_header, header == GenSliceType::Type2Efficient);
        let t = round_trip(p, ch, 10e6, 1.0, RxConfig { plp: Some(2), ..RxConfig::default() });
        assert_eq!(t.status.chosen, Some((1, 2)));
        ts_ok(&t, 3_000);
    }
}

#[test]
fn round_trip_group_gives_the_input_stream_of_the_chosen_plp() {
    let p = params(|p| {
        p.system = GenSystem::Group;
        p.modcod = GenModCod::Q64R23.modcod(true);
    });
    let ch = Channel {
        esn0_db: Some(20.0),
        offset_hz: -6e3,
        clock_ppm: 8.0,
        echo: None,
    };
    // The common PLP lies in a slice of its own inside the window.
    let own = Numbered::SATELLITE_CABLE.pid;
    for plp in [1u8, 2] {
        let t = round_trip(p, ch, 10e6, 1.2, RxConfig { plp: Some(plp), ..RxConfig::default() });
        assert_eq!((t.status.chosen, t.status.common), (Some((0, plp)), Some(COMMON_PLP)));
        assert_eq!((t.status.outside, t.status.retune_hz), (None, None));
        println!("merge {:?}", t.merge);
        let pids = equals_group_input(&t.merged, plp, 2_000, 2_500);
        assert_eq!(pids, [own + u16::from(plp) - 1, GROUP_SHARED_PID]);
        assert!(t.merge.common as usize > t.merged.len() / 5, "{:?}", t.merge);
    }
}

#[test]
fn round_trip_group_over_two_blocks_names_the_slice_outside_and_the_tuning_position() {
    let p = params(|p| {
        p.system = GenSystem::FarSlice;
        p.modcod = GenModCod::Q64R23.modcod(false);
    });
    let ch = Channel {
        esn0_db: Some(20.0),
        ..Channel::default()
    };
    let l1 = p.l1();
    let (start, dx, sp) = (l1.k_min(), p.guard.d_x(), Raster::R8.spacing());
    let ds = |i: usize| l1.slices[i].carriers(start, dx).unwrap();
    // The window centre: the carrier at the centre of the samples, in the lower L1 block.
    let k0 = p.carriers().0;
    // PLP 1: its slice is in the window, the slice of its common PLP reaches out at the upper edge, and no tuning position
    // holds both. It is decoded without the common PLP: its own packets run on, the slots of the common PLP stay null.
    let t = round_trip(p, ch, 10e6, 1.0, RxConfig::default());
    let by = (ds(1).1 - (k0 + HALF_WINDOW)) as f64 * sp;
    assert_eq!((t.status.chosen, t.status.common), (Some((0, 1)), None));
    assert_eq!((t.status.outside, t.status.retune_hz), (Some((1, by)), None));
    assert_eq!((t.ts.wrong, t.ts.flagged), (0, 0));
    consecutive(&t.numbers, 0, 500);
    assert!(t.merged.iter().all(|p| is_null(p) || Numbered::SATELLITE_CABLE.number_of(p).is_some()));
    // PLP 2: its slice lies outside the samples; the window at its tuning position holds it and the common PLP.
    let t = round_trip(p, ch, 10e6, 0.5, RxConfig { plp: Some(2), ..RxConfig::default() });
    let tune = l1.slices[2].tune_carrier(start, dx);
    let by = (ds(2).1 - (k0 + HALF_WINDOW)) as f64 * sp;
    assert_eq!((t.status.chosen, t.status.retune_hz), (None, Some(tune as f64 * sp)));
    assert_eq!(t.status.outside, Some((2, by)));
    assert!(t.merged.is_empty());
    // The same system with the samples centred on that position.
    let q = GenParams { system_hz: Some(p.freq_hz), freq_hz: tune as f64 * sp, ..p };
    assert_eq!(q.l1(), l1);
    let t = round_trip(q, ch, 10e6, 1.5, RxConfig { plp: Some(2), ..RxConfig::default() });
    assert_eq!((t.status.chosen, t.status.common), (Some((2, 2)), Some(COMMON_PLP)));
    assert_eq!((t.status.outside, t.status.retune_hz), (None, None));
    let pids = equals_group_input(&t.merged, 2, 1_000, 1_000);
    assert_eq!(pids, [Numbered::SATELLITE_CABLE.pid + 1, GROUP_SHARED_PID]);
    // PLP 1 from there: its own slice reaches out at the lower edge and no tuning position holds it with the common PLP.
    let t = round_trip(q, ch, 10e6, 0.5, RxConfig { plp: Some(1), ..RxConfig::default() });
    let by = (q.carriers().0 - HALF_WINDOW - ds(0).0) as f64 * sp;
    assert_eq!((t.status.chosen, t.status.retune_hz, t.status.outside), (None, None, Some((0, by))));
    assert!(t.merged.is_empty());
}

#[test]
fn round_trip_at_the_least_rate() {
    let t = round_trip(GenParams::default(), Channel::default(), 8e6, 0.8, RxConfig::default());
    ts_ok(&t, 10_000);
}

// Through the stack's graph: the PLP setting, a PLP change while running, the IP output.

use super::common::{generate, recording, Rx};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Value;
use sdrkit_stack_dvbcs::dvbc2::settings::{GEN_SLICE_TYPE, GEN_STREAM, GEN_SYSTEM, PLP};
use sdrkit_stack_dvbcs::dvbc2::DVBC2;
use sdrkit_core::interface::Hertz;
use sdrkit_stack_dvbcs::dvbc2::comp::{HeaderKind, PayloadType, PlpType, SliceKind};
use sdrkit_stack_dvbcs::typed::{Dvbc2PlpRow, Dvbc2SliceRow, DVBC2_PLPS, DVBC2_SLICES};

#[test]
fn graph_decodes_plp_1_of_the_default_signal() {
    let s = [CaseSetting::new(&PLP, 1)];
    let x = generate(&DVBC2, 10e6, 0.8, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    let c = rx.check();
    println!("{c:?}");
    assert!(c.good > 5000 && c.wrong == 0, "{c:?}");
}

#[test]
fn graph_on_the_6mhz_raster_reports_frequencies_of_its_spacing() {
    use sdrkit_stack_dvbcs::dvbc2::settings::RASTER;
    let s = [CaseSetting::new(&RASTER, Raster::R6)];
    let x = generate(&DVBC2, 7e6, 0.8, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(7e6, 346e6));
    rx.feed(&x, 65536);
    let g = GenParams { raster: Raster::R6, ..GenParams::default() };
    let want = g.l1().k_min() as f64 * Raster::R6.spacing();
    let got = rx.num("start_frequency_hz");
    assert!((got - want).abs() < 1.0, "{got} against {want}");
    assert!(rx.num("blocks_ok") > 200.0 && rx.num("blocks_bad") == 0.0);
}

#[test]
fn header_scrambling_word_is_a_setting_of_both_sides() {
    use sdrkit_stack_dvbcs::dvbc2::settings::RM_SCRAMBLING;
    let other = 0x1234_5678i64;
    let s = [
        CaseSetting::new(&RM_SCRAMBLING, other),
        CaseSetting::new(&GEN_SLICE_TYPE, GenSliceType::Type2Robust),
    ];
    let x = generate(&DVBC2, 10e6, 0.6, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    assert!(rx.num("blocks_ok") > 200.0, "{} blocks", rx.num("blocks_ok"));
    // The word decides the lower branch: headers of this word read with the default word lose agreement.
    let cons = header::constellation(true);
    let mut cells = Vec::new();
    header::cells(0x01B8, true, &cons, other as u32, &mut cells);
    let w = [10.0f32; 32];
    let right = HeaderDecoder::new(other as u32).decode(&cells, &w, true).unwrap();
    let wrong = HeaderDecoder::new(W_RM_DEFAULT).decode(&cells, &w, true).unwrap();
    assert_eq!((right.bits, right.agree), (0x01B8, 32));
    assert!(wrong.agree < 32, "{wrong:?}");
}

#[test]
fn graph_follows_a_plp_change() {
    let s = [
        CaseSetting::new(&GEN_SLICE_TYPE, GenSliceType::Type2Robust),
        CaseSetting::new(&GEN_SYSTEM, GenSystem::TwoPlps),
        CaseSetting::new(&PLP, 1),
    ];
    let x = generate(&DVBC2, 10e6, 1.2, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    let half = x.len() / 2;
    rx.feed(&x[..half], 65536);
    let before = rx.packets.len();
    rx.apply("plp", &Value::I64(2));
    rx.feed(&x[half..], 65536);
    let after = rx.packets.len() - before;
    println!("{before} packets of PLP 1, {after} after the change");
    assert!(before > 2000 && after > 2000);
    assert_eq!(rx.num("chosen_plp"), 2.0);
}

#[test]
fn graph_delivers_the_generic_stream_and_names_the_common_plp() {
    let s = [CaseSetting::new(&GEN_STREAM, GenStream::Gfps)];
    let x = generate(&DVBC2, 10e6, 0.6, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    let n = rx.num("generic_packets");
    assert!(n > 10_000.0 && rx.num("generic_bytes") == 96.0 * n, "{n} packets");
    let ts = rx.num("ts_packets");
    assert!(ts.is_nan() || ts == 0.0, "{ts} transport packets");
    // The transport stream of the stack is the data PLP with its common PLP put back: the numbered stream, whole.
    let s = [
        CaseSetting::new(&GEN_SYSTEM, GenSystem::Common),
        CaseSetting::new(&sdrkit_proto_dvbs2::transmit::GEN_PAYLOAD, sdrkit_proto_dvbs2::transmit::Payload::Numbered),
    ];
    let x = generate(&DVBC2, 10e6, 0.6, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    assert_eq!(rx.num("common_plp"), f64::from(COMMON_PLP));
    assert!(rx.num("ts_packets") > 5000.0);
    let c = rx.check();
    assert!(c.good > 5000 && (c.wrong, c.flagged, c.gaps) == (0, 0, 0), "{c:?}");
}

#[test]
fn graph_delivers_ip_packets_of_gse() {
    let s = [CaseSetting::new(&GEN_STREAM, GenStream::Gse)];
    let x = generate(&DVBC2, 10e6, 0.8, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    let n = rx.num("ip_packets");
    println!("{n} IP packets");
    assert!(n > 500.0, "{n} IP packets");
}

#[test]
fn graph_decodes_with_the_verification_impairments() {
    use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
    use sdrkit_stack_dvbcs::dvbc2::settings::GEN_MODCOD;
    let s = [
        CaseSetting::new(&GEN_MODCOD, GenModCod::Q64R23),
        CaseSetting::new(&GEN_NOISE, NoiseKind::Snr),
        CaseSetting::new(&GEN_SNR_DB, 20.0),
        CaseSetting::new(&GEN_OFFSET_HZ, 3e3),
    ];
    let x = generate(&DVBC2, 10e6, 0.8, &s);
    let mut rx = Rx::new(&DVBC2, &s[..1], &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    let (ok, bad) = (rx.num("blocks_ok"), rx.num("blocks_bad"));
    rx.dump();
    assert!(ok > 300.0 && bad < 10.0, "{ok} good, {bad} bad");
}


/// The slice and PLP tables of the stack on the test signal of `settings`: every slice and PLP of the L1 of the generator
/// with its values. `decoded`: the PLPs of type 2 slices whose FECFrame headers the receiver reads at this tuning.
fn tables_list_the_l1(settings: &[CaseSetting], decoded: &[u8]) {
    let x = generate(&DVBC2, 10e6, 0.5, settings);
    let mut rx = Rx::new(&DVBC2, settings, &recording(10e6, 346e6));
    rx.feed(&x, 65536);
    let p = GenParams::from_values(&super::common::config(&DVBC2, settings).settings);
    let l1 = p.l1();
    let (start, dx, sp) = (l1.k_min(), p.guard.d_x(), p.raster.spacing());
    let r = rx.results();
    let slices: Vec<(&str, Dvbc2SliceRow)> = DVBC2_SLICES.rows(r).collect();
    let plps: Vec<(&str, Dvbc2PlpRow)> = DVBC2_PLPS.rows(r).collect();
    assert_eq!(slices.len(), l1.slices.len());
    assert_eq!(plps.len(), l1.slices.iter().map(|s| s.plps.len()).sum::<usize>());
    let window = rx.num("window_carrier") as usize;
    for s in &l1.slices {
        let row = &slices.iter().find(|r| r.0 == s.id.to_string()).unwrap_or_else(|| panic!("no row of slice {}", s.id)).1;
        let ds = s.carriers(start, dx).unwrap();
        let want = Dvbc2SliceRow {
            slice_id: i64::from(s.id),
            slice_type: if s.type2 { SliceKind::Type2 } else { SliceKind::Type1 },
            header_type: match (s.type2, s.efficient_header) {
                (false, _) => HeaderKind::None,
                (true, false) => HeaderKind::Robust,
                (true, true) => HeaderKind::Efficient,
            },
            tune_hz: Hertz(s.tune_carrier(start, dx) as f64 * sp),
            start_hz: Some(Hertz(ds.0 as f64 * sp)),
            end_hz: Some(Hertz(ds.1 as f64 * sp)),
            ti_depth: s.depth() as i64,
            plp_count: s.plps.len() as i64,
            in_window: ds.0 + HALF_WINDOW >= window && ds.1 <= window + HALF_WINDOW,
            width_hz: Some(Hertz((ds.1 - ds.0 + 1) as f64 * sp)),
        };
        assert_eq!(*row, want);
        for q in &s.plps {
            let key = format!("{}/{}", s.id, q.id);
            let row = &plps.iter().find(|r| r.0 == key).unwrap_or_else(|| panic!("no row of PLP {key}")).1;
            // The coding: from the L1 in a type 1 slice, from the FECFrame headers in a type 2 slice that is decoded.
            let coded = !s.type2 || decoded.contains(&q.id);
            let want = Dvbc2PlpRow {
                plp_slice: i64::from(s.id),
                plp_id: i64::from(q.id),
                plp_type: [PlpType::Common, PlpType::Grouped, PlpType::Normal][usize::from(q.plp_type)],
                payload: PayloadType::Ts,
                fec_frame_bits: coded.then_some(if p.modcod.long { 64_800 } else { 16_200 }),
                modulation: coded.then_some(p.modcod.qam),
                code_rate: coded.then_some(p.modcod.rate),
                ts_id: Some(0x0C21),
                on_id: Some(0x2F00),
                chosen: rx.num("chosen_slice") == f64::from(s.id) && rx.num("chosen_plp") == f64::from(q.id),
                group: (q.plp_type <= 1).then_some(i64::from(q.group)),
                bundled: q.bundled,
            };
            assert_eq!(*row, want);
        }
    }
}

#[test]
fn tables_list_every_slice_and_plp_of_the_l1() {
    let system = |s| CaseSetting::new(&GEN_SYSTEM, s);
    tables_list_the_l1(&[], &[]);
    tables_list_the_l1(
        &[system(GenSystem::MixedSlices), CaseSetting::new(&GEN_SLICE_TYPE, GenSliceType::Type2Efficient), CaseSetting::new(&PLP, 2)],
        &[2],
    );
    // The headers of every PLP of a decoded slice are read: PLP 2 beside the chosen PLP 1, and the common PLP.
    tables_list_the_l1(&[system(GenSystem::Group)], &[1, 2, COMMON_PLP]);
    // Slices 1 and 2 of the wide system are listed and not decoded.
    tables_list_the_l1(&[system(GenSystem::FarSlice)], &[1]);
}

#[test]
fn graph_requests_the_tuning_position_that_holds_the_slice_and_its_common_plp() {
    use sdrkit_core::{HostRequest, Output, Receiver};
    let s = [CaseSetting::new(&GEN_SYSTEM, GenSystem::FarSlice)];
    let x = generate(&DVBC2, 10e6, 0.9, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    // Feeds `x` and returns the requests of the stack.
    let feed = |rx: &mut Rx, x: &[Complex32]| {
        let mut out = Output::default();
        let mut requests = Vec::new();
        for c in x.chunks(65536) {
            rx.rx.feed(c, &mut out);
            requests.append(&mut out.requests);
            out.clear();
        }
        requests
    };
    let p = GenParams::from_values(&super::common::config(&DVBC2, &s).settings);
    let l1 = p.l1();
    let (start, dx, sp) = (l1.k_min(), p.guard.d_x(), p.raster.spacing());
    // PLP 1 is decoded; the slice of its common PLP is out of the window and no tuning position holds both: no request.
    let requests = feed(&mut rx, &x[..x.len() / 2]);
    let by = (l1.slices[1].carriers(start, dx).unwrap().1 - (p.carriers().0 + HALF_WINDOW)) as f64 * sp;
    assert_eq!((rx.num("chosen_plp"), rx.num("outside_slice"), rx.num("outside_hz")), (1.0, 1.0, by));
    assert!(rx.num("common_plp").is_nan() && rx.num("retune_hz").is_nan() && rx.num("blocks_ok") > 20.0);
    assert_eq!(requests, []);
    // PLP 2: one request for the tuning position of its slice.
    rx.apply("plp", &Value::I64(2));
    let requests = feed(&mut rx, &x[x.len() / 2..]);
    let tune = l1.slices[2].tune_carrier(start, dx) as f64 * sp;
    assert_eq!(requests, [HostRequest::retune(tune, "DVB-C2 tuning position of the data slice")]);
    assert_eq!((rx.num("retune_hz"), rx.num("outside_slice")), (tune, 2.0));
    assert!(rx.num("chosen_plp").is_nan());
    // The host tunes there: the stream of PLP 2 with the common PLP put back is the input stream of the generator.
    let q = GenParams { system_hz: Some(p.freq_hz), freq_hz: tune, ..p };
    let mut g = C2Generator::new(q, 10e6).unwrap();
    let mut y = vec![Complex32::default(); 12_000_000];
    g.generate(&mut y);
    let s = [s[0].clone(), CaseSetting::new(&PLP, 2)];
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, tune));
    rx.feed(&y, 65536);
    assert_eq!((rx.num("chosen_slice"), rx.num("chosen_plp"), rx.num("common_plp")), (2.0, 2.0, f64::from(COMMON_PLP)));
    assert!(rx.num("outside_slice").is_nan() && rx.num("retune_hz").is_nan());
    let pids = equals_group_input(&rx.packets, 2, 1_000, 500);
    assert_eq!(pids, [Numbered::SATELLITE_CABLE.pid + 1, GROUP_SHARED_PID]);
}

#[test]
fn graph_retuned_to_the_requested_position_decodes_without_a_second_request() {
    use sdrkit_core::{Output, Receiver};
    let s = [CaseSetting::new(&GEN_SYSTEM, GenSystem::FarSlice), CaseSetting::new(&PLP, 2)];
    let x = generate(&DVBC2, 10e6, 0.4, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    let mut out = Output::default();
    for c in x.chunks(65536) {
        rx.rx.feed(c, &mut out);
    }
    let tune = match out.requests.as_slice() {
        [sdrkit_core::HostRequest::Retune { tuned_hz, .. }] => *tuned_hz,
        other => panic!("{other:?}"),
    };
    out.clear();
    let p = GenParams::from_values(&super::common::config(&DVBC2, &s).settings);
    let mut g = C2Generator::new(GenParams { system_hz: Some(p.freq_hz), freq_hz: tune, ..p }, 10e6).unwrap();
    let mut y = vec![Complex32::default(); 8_000_000];
    g.generate(&mut y);
    rx.rx.retuned(&recording(10e6, tune));
    for c in y.chunks(65536) {
        rx.rx.feed(c, &mut out);
    }
    assert_eq!(out.requests, []);
    assert_eq!((rx.num("chosen_slice"), rx.num("chosen_plp"), rx.num("common_plp")), (2.0, 2.0, f64::from(COMMON_PLP)));
    assert!(rx.num("outside_slice").is_nan() && rx.num("retune_hz").is_nan());
    assert_eq!(DVBC2_SLICES.rows::<Dvbc2SliceRow>(rx.results()).count(), 3);
    assert!(rx.num("ts_packets") > 500.0, "{} packets", rx.num("ts_packets"));
}

#[test]
fn graph_switches_the_data_plp_of_a_group_and_keeps_its_common_plp() {
    let s = [CaseSetting::new(&GEN_SYSTEM, GenSystem::Group), CaseSetting::new(&PLP, 1)];
    let x = generate(&DVBC2, 10e6, 1.6, &s);
    let mut rx = Rx::new(&DVBC2, &s, &recording(10e6, 346e6));
    let half = x.len() / 2;
    rx.feed(&x[..half], 65536);
    let before = rx.packets.len();
    let own = Numbered::SATELLITE_CABLE.pid;
    assert_eq!(equals_group_input(&rx.packets, 1, 1_000, 2_000), [own, GROUP_SHARED_PID]);
    rx.apply("plp", &Value::I64(2));
    rx.feed(&x[half..], 65536);
    assert_eq!((rx.num("chosen_plp"), rx.num("common_plp")), (2.0, f64::from(COMMON_PLP)));
    assert_eq!(equals_group_input(&rx.packets[before..], 2, 1_000, 2_000), [own + 1, GROUP_SHARED_PID]);
}

#[test]
fn recovers_after_the_signal_restarts() {
    let x = generate(&DVBC2, 10e6, 2.0, &[]);
    let mut rx = Rx::new(&DVBC2, &[], &recording(10e6, 346e6));
    // The signal starts again from its beginning 0.53 s in, as a test source does when it is made again.
    rx.feed(&x[..5_300_000], 65536);
    let before = rx.num("blocks_ok");
    rx.feed(&x, 65536);
    let after = rx.num("blocks_ok");
    rx.dump();
    println!("{before} blocks before, {after} after");
    assert!(after - before > 1000.0, "{before} then {after}");
}

/// Input rounded to 8 bits like a low-cost radio (full scale 12 dB above the mean level).
#[test]
fn eight_bit_input_decodes() {
    let x = generate(&DVBC2, 10e6, 0.8, &[]);
    let q = |v: f32| (v * 32.0).clamp(-128.0, 127.0).round() / 32.0;
    let y: Vec<Complex32> = x.iter().map(|c| Complex32::new(q(c.re), q(c.im))).collect();
    let mut rx = Rx::new(&DVBC2, &[], &recording(10e6, 346e6));
    rx.feed(&y, 65536);
    let (ok, bad) = (rx.num("blocks_ok"), rx.num("blocks_bad"));
    assert!(ok > 300.0 && bad == 0.0, "{ok} good, {bad} bad");
}

/// Prints the processor share of the front and of the decoding on the default test signal (64K 256-QAM 5/6, 10 Msps, 30 dB):
/// seconds of work per second of signal. Run it with `--release -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn processor_share_of_the_default_signal() {
    let p = GenParams::default();
    let rate = 10e6;
    let mut g = C2Generator::new(p, rate).unwrap();
    let mut x = vec![Complex32::default(); (2.0 * rate) as usize];
    g.generate(&mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(1e-3, 0x4332).unwrap().process(&mut x);
    let mut best = (f64::MAX, f64::MAX);
    for _ in 0..3 {
        let mut rx = C2Receiver::new(RxConfig { rate_hz: rate, centre_hz: p.freq_hz, ..RxConfig::default() });
        let mut dec = C2Decoder::default();
        let mut cells = Vec::new();
        let (mut front, mut fec) = (0f64, 0f64);
        for c in x.chunks(16_384) {
            let t = std::time::Instant::now();
            rx.feed(c, &mut cells);
            front += t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            for c in cells.drain(..) {
                dec.push(&c.value, c.time, &mut |_| {});
            }
            fec += t.elapsed().as_secs_f64();
        }
        assert!(dec.status.blocks_ok > 300 && dec.status.blocks_bad == 0, "{:?}", dec.status);
        best = (best.0.min(front), best.1.min(fec));
    }
    let s = x.len() as f64 / rate;
    println!("front {:.3}, fec {:.3}, both {:.3}", best.0 / s, best.1 / s, (best.0 + best.1) / s);
}
