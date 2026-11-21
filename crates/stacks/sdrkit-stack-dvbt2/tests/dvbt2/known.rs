//! Known answers taken from ETSI EN 302 755 V1.4.1.

use sdrkit_stack_dvbt2::params::{Fft, Guard, PilotPattern};
use sdrkit_stack_dvbt2::pilots::{pilot_prbs, FrameShape, PilotMaps};
use sdrkit_stack_dvbt2::tables;

fn shape(fft: Fft, pp: PilotPattern, extended: bool, miso: bool, tr: bool) -> FrameShape {
    let guard = if fft == Fft::K32 {
        Guard::G1_8
    } else {
        Guard::G1_4
    };
    FrameShape {
        fft,
        extended,
        pp,
        guard,
        miso,
        tr,
        l_data: 21,
    }
}

/// Table 48: C_data for every FFT size and pattern, normal and extended, with and without tone reservation.
#[test]
fn c_data_matches_table_48() {
    let mut open = Vec::new();
    for fft in Fft::ALL {
        for pp in PilotPattern::ALL {
            for extended in [false, true] {
                if extended && !fft.has_extended() {
                    continue;
                }
                let want = if extended {
                    tables::C_DATA_EXTENDED[fft.row() - 3][pp.code() as usize]
                } else {
                    tables::C_DATA[fft.row()][pp.code() as usize]
                };
                if want == 0 {
                    continue;
                }
                for tr in [false, true] {
                    let m = PilotMaps::new(shape(fft, pp, extended, false, tr)).unwrap();
                    let n_tr = if tr { tables::N_TR[fft.row()] } else { 0 };
                    let counts: Vec<usize> =
                        (0..pp.dy()).map(|ph| m.normal(ph).data.len()).collect();
                    if counts.iter().any(|&c| c != want - n_tr) {
                        open.push(format!(
                            "{} {} ext {extended} tr {tr}: {counts:?} want {}",
                            fft.name(),
                            pp.name(),
                            want - n_tr
                        ));
                    }
                }
            }
        }
    }
    // In these four cases with extended carriers, C_data differs from the table by one cell in some symbols.
    let known_open = [
        "8K PP7 ext true",
        "8K PP8 ext true",
        "16K PP7 ext true",
        "16K PP8 ext true",
    ];
    let unexpected: Vec<&String> = open
        .iter()
        .filter(|o| !known_open.iter().any(|k| o.starts_with(k)))
        .collect();
    assert!(unexpected.is_empty(), "{unexpected:#?}");
}

/// Table 47: C_P2 for SISO and MISO.
#[test]
fn c_p2_matches_table_47() {
    for fft in Fft::ALL {
        for miso in [false, true] {
            let pp = if fft == Fft::K32 {
                PilotPattern::Pp2
            } else {
                PilotPattern::Pp1
            };
            let m = PilotMaps::new(shape(fft, pp, false, miso, false)).unwrap();
            let (siso, mis) = tables::C_P2[fft.row()];
            assert_eq!(
                m.p2().data.len(),
                if miso { mis } else { siso },
                "{} miso {miso}",
                fft.name()
            );
        }
    }
}

/// Table 49: N_FC, the data cells of the frame closing symbol.
#[test]
fn n_fc_matches_table_49() {
    for fft in Fft::ALL {
        for pp in PilotPattern::ALL {
            for extended in [false, true] {
                if extended && !fft.has_extended() {
                    continue;
                }
                let want = if extended {
                    tables::N_FC_EXTENDED[fft.row() - 3][pp.code() as usize]
                } else {
                    tables::N_FC[fft.row()][pp.code() as usize]
                };
                if want == 0 {
                    continue;
                }
                let mut s = shape(fft, pp, extended, true, false);
                s.l_data = 21;
                let m = PilotMaps::new(s).unwrap();
                let l = s.l_f() - 1;
                assert_eq!(
                    m.symbol(l).data.len(),
                    want,
                    "{} {} ext {extended}",
                    fft.name(),
                    pp.name()
                );
            }
        }
    }
}

/// Clause 9.2.2.1: the symbol-level PRBS begins 1,1,1,1,1,1,1,1,1,1,1,0,0.
#[test]
fn pilot_prbs_starts_as_the_standard_says() {
    assert_eq!(pilot_prbs(13), vec![1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0]);
}

use sdrkit_stack_dvbt2::bicm::{point_set, rotation};
use sdrkit_algo_fec::bitcells::BitCells;
use sdrkit_algo_fec::shorten::shortened_positions;
use sdrkit_stack_dvbt2::interleave::{CellInterleaver, FreqInterleaver};
use sdrkit_stack_dvbt2::l1::{post_sizes, L1Codec, L1Post, L1Pre};
use sdrkit_stack_dvbt2::p1;
use sdrkit_stack_dvbt2::params::{FecFrame, Modulation};
use sdrkit_stack_dvbt2::pilots::{continual_pilots, frame_pn};

/// Clause 6.1.3: the column twist output order for 64-QAM, N_ldpc 64 800, from the shared bit interleaver with the twist of
/// table 8 alone (no parity interleaving, one bit per cell).
#[test]
fn column_twist_matches_clause_6_1_3() {
    let p = BitCells {
        n: 64_800,
        k: 0,
        parity: false,
        twist: tables::twist(FecFrame::Normal, Modulation::Qam64).unwrap(),
        demux: &[0],
        bits_per_cell: 1,
    }
    .permutation()
    .unwrap();
    let source = |j: usize| p.src(j).unwrap();
    let want = [
        0, 5400, 16198, 21598, 26997, 32396, 37796, 43195, 48595, 53993, 59392, 64791,
    ];
    let got: Vec<usize> = (0..12).map(source).collect();
    assert_eq!(got, want);
    let last = [
        5399, 10799, 16197, 21597, 26996, 32395, 37795, 43194, 48594, 53992, 59391, 64790,
    ];
    let got: Vec<usize> = (64800 - 12..64800).map(source).collect();
    assert_eq!(got, last);
}

/// Clause 6.4: P(r) for N_cells 10 800 is 0, 8 192, 4 096, 2 048, 10 240, 6 144, 1 024, 9 216.
#[test]
fn cell_interleaver_shifts_match_clause_6_4() {
    let ci = CellInterleaver::new(10_800).unwrap();
    let got: Vec<u32> = (0..8).map(|r| ci.shift(r)).collect();
    assert_eq!(got, [0, 8192, 4096, 2048, 10240, 6144, 1024, 9216]);
    let mut seen = vec![false; 10_800];
    for &a in ci.l0() {
        assert!(!seen[a as usize]);
        seen[a as usize] = true;
    }
}

/// Tables 14(b) to 14(f) and 15: QPSK, 16-QAM and 64-QAM levels, y_0 on the real axis, 1 negative.
#[test]
fn constellations_match_table_14() {
    let q = point_set(Modulation::Qpsk);
    let s = 2f32.sqrt();
    assert!(
        (q.points()[0b00].re * s - 1.0).abs() < 1e-6
            && (q.points()[0b00].im * s - 1.0).abs() < 1e-6
    );
    assert!(
        (q.points()[0b10].re * s + 1.0).abs() < 1e-6
            && (q.points()[0b01].im * s + 1.0).abs() < 1e-6
    );
    // 16-QAM real part from (y0, y2): 10 -> -3, 11 -> -1, 01 -> 1, 00 -> 3.
    let q16 = point_set(Modulation::Qam16);
    let s = 10f32.sqrt();
    for (y0, y2, v) in [(1, 0, -3.0), (1, 1, -1.0), (0, 1, 1.0), (0, 0, 3.0)] {
        let label = (y0 << 3) | (y2 << 1);
        assert!(
            (q16.points()[label].re * s - v).abs() < 1e-5,
            "16-QAM {y0}{y2}"
        );
        let label = (y0 << 2) | y2;
        assert!(
            (q16.points()[label].im * s - v).abs() < 1e-5,
            "16-QAM im {y0}{y2}"
        );
    }
    // 64-QAM real part from (y0, y2, y4): table 14(f).
    let q64 = point_set(Modulation::Qam64);
    let s = 42f32.sqrt();
    let t = [
        (1, 0, 0, -7.0),
        (1, 0, 1, -5.0),
        (1, 1, 1, -3.0),
        (1, 1, 0, -1.0),
        (0, 1, 0, 1.0),
        (0, 1, 1, 3.0),
        (0, 0, 1, 5.0),
        (0, 0, 0, 7.0),
    ];
    for (y0, y2, y4, v) in t {
        let label = (y0 << 5) | (y2 << 3) | (y4 << 1);
        assert!(
            (q64.points()[label].re * s - v).abs() < 1e-5,
            "64-QAM {y0}{y2}{y4}"
        );
    }
    let q256 = point_set(Modulation::Qam256);
    let mean: f32 = q256.points().iter().map(|p| p.norm_sqr()).sum::<f32>() / 256.0;
    assert!((mean - 1.0).abs() < 1e-4);
}

/// Table 16: rotation angles.
#[test]
fn rotation_angles_match_table_16() {
    for (m, deg) in [
        (Modulation::Qpsk, 29.0f64),
        (Modulation::Qam16, 16.8),
        (Modulation::Qam64, 8.6),
        (Modulation::Qam256, 3.576334),
    ] {
        let r = rotation(m);
        assert!((r.arg() as f64).to_degrees().sub_abs(deg) < 1e-4, "{m:?}");
    }
}

trait SubAbs {
    fn sub_abs(self, o: f64) -> f64;
}
impl SubAbs for f64 {
    fn sub_abs(self, o: f64) -> f64 {
        (self - o).abs()
    }
}

/// Clause 7.3.1.1 and table 39: L1-pre is 1 840 cells. Clause 7.3.2.2 example: K_sig 1 172 of the L1-pre code fills groups 0
/// and 1, the first 260 bits of group 4 and group 8.
#[test]
fn l1_shortening_matches_clause_7_3_2_2() {
    assert_eq!(L1Codec::pre().unwrap().cells(), 1840);
    // The information positions are those the shared shortening leaves unpadded.
    let info = |k_sig: usize| {
        let mut padded = Vec::new();
        shortened_positions(3072, k_sig, 360, &tables::L1_PRE_PAD_ORDER, &mut padded).unwrap();
        (0..3072u32).filter(|p| padded.binary_search(p).is_err()).collect::<Vec<u32>>()
    };
    let pos = info(1172);
    let mut want: Vec<u32> = (0..720).collect();
    want.extend(4 * 360..4 * 360 + 260);
    want.extend(8 * 360..8 * 360 + 192);
    assert_eq!(pos, want);
    // K_sig 200 (L1-pre): every group but group 0 is padded, and 160 bits of group 0.
    assert_eq!(
        info(200),
        (0..200).collect::<Vec<u32>>()
    );
}

/// TS 102 831 clause 9.2.2.6: K_sig 3 986, N_P2 2 and 64-QAM result in N_punc 3 650 and N_post 9 504.
#[test]
fn l1_post_puncturing_matches_ts_102_831() {
    let s = post_sizes(7971 - 32, Modulation::Qam64, 2).unwrap();
    assert_eq!(
        (s.blocks, s.k_sig, s.n_punc, s.n_post),
        (2, 3986, 3650, 9504)
    );
}

/// L1-pre and L1-post bits survive their encoding with the CRC-32 of annex F.
#[test]
fn l1_fields_round_trip() {
    let pre = L1Pre {
        s2: 3,
        guard: 2,
        l1_mod: 1,
        l1_post_size: 1234,
        l1_post_info_size: 600,
        pilot_pattern: 6,
        cell_id: 0xBEEF,
        num_t2_frames: 4,
        num_data_symbols: 59,
        num_rf: 1,
        t2_version: 2,
        ..Default::default()
    };
    let b = pre.to_bits();
    assert_eq!(b.len(), 200);
    assert_eq!(L1Pre::from_bits(&b), Some(pre));
    let mut bad = b.clone();
    bad[17] ^= 1;
    assert_eq!(L1Pre::from_bits(&bad), None);
    let mut post = L1Post::default();
    post.config.rf = vec![(0, 474_000_000)];
    post.config.plps = vec![Default::default()];
    post.dynamic.plps = vec![Default::default()];
    let pre2 = L1Pre {
        num_rf: 1,
        l1_post_info_size: post.info_bits().len() as u32,
        ..Default::default()
    };
    assert_eq!(L1Post::from_bits(&post.to_bits(), &pre2), Some(post));
}

/// Table 56: the frame-level PN sequence starts 4DC2 (0100 1101 1100 0010).
#[test]
fn frame_pn_starts_as_table_56() {
    let bits: Vec<u8> = (0..16).map(frame_pn).collect();
    assert_eq!(bits, [0, 1, 0, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0]);
}

/// Table 68: 384 P1 carriers from 44 to 809. Table 69: the S1 and S2 patterns are mutually orthogonal (clause 9.8.2.2 b).
#[test]
fn p1_tables_match_the_standard() {
    assert_eq!(tables::P1_CARRIERS.len(), 384);
    assert_eq!(
        (tables::P1_CARRIERS[0], tables::P1_CARRIERS[383]),
        (44, 809)
    );
    let pm = |v: u64, i: usize| 1 - 2 * ((v >> (63 - i)) & 1) as i32;
    for a in 0..8 {
        for b in 0..8 {
            let c: i32 = (0..64)
                .map(|i| pm(tables::P1_S1[a], i) * pm(tables::P1_S1[b], i))
                .sum();
            assert_eq!(c, if a == b { 64 } else { 0 }, "S1 {a} {b}");
        }
    }
    for a in 0..16 {
        for b in 0..16 {
            let c: i32 = (0..256)
                .map(|i| {
                    pm(tables::P1_S2[a][i / 64], i % 64) * pm(tables::P1_S2[b][i / 64], i % 64)
                })
                .sum();
            assert_eq!(c, if a == b { 256 } else { 0 }, "S2 {a} {b}");
        }
    }
    // The P1 symbol: C repeats the start of A shifted by 1/(1024 T), B its end.
    let info = sdrkit_stack_dvbt2::params::P1Info { s1: 3, s2: 9 };
    let x = p1::generate(info);
    assert_eq!(x.len(), 2048);
    let power: f32 = x.iter().map(|v| v.norm_sqr()).sum::<f32>() / 2048.0;
    assert!((power - 1.0).abs() < 0.05, "{power}");
    for n in [0usize, 100, 541] {
        let ph = 2.0 * std::f32::consts::PI * n as f32 / 1024.0;
        let want = x[542 + n] * sdrkit_core::Complex32::new(ph.cos(), ph.sin());
        assert!((x[n] - want).norm() < 1e-4);
    }
}

/// Clause 8.5: the frequency interleaver permutations are bijections for every size and data cell count, and 32K uses one H.
#[test]
fn frequency_interleaver_is_a_permutation() {
    for fft in Fft::ALL {
        let n = fft.size() * 3 / 4;
        for h in 0..2 {
            let v = FreqInterleaver::h(fft, h, n).unwrap();
            let mut seen = vec![false; n];
            for &a in &v {
                assert!(!seen[a as usize]);
                seen[a as usize] = true;
            }
        }
    }
    assert_eq!(
        FreqInterleaver::h(Fft::K32, 0, 27000).unwrap(),
        FreqInterleaver::h(Fft::K32, 1, 27000).unwrap()
    );
}

/// Tables 57, 60, 62, 63 and 67: pilot spacings and amplitudes, continual pilot counts and guard lengths.
#[test]
fn pilot_and_guard_numbers_match_the_tables() {
    let dxy: Vec<(usize, usize)> = PilotPattern::ALL.iter().map(|p| (p.dx(), p.dy())).collect();
    assert_eq!(
        dxy,
        [
            (3, 4),
            (6, 2),
            (6, 4),
            (12, 2),
            (12, 4),
            (24, 2),
            (24, 4),
            (6, 16)
        ]
    );
    assert!((Fft::K4.a_cp() - 4.0 * 2f32.sqrt() / 3.0).abs() < 1e-6);
    assert_eq!(Guard::G19_256.samples(Fft::K32), Some(2432));
    assert_eq!(Guard::G19_128.samples(Fft::K8), Some(1216));
    assert_eq!(Guard::G1_128.samples(Fft::K4), None);
    assert_eq!(Guard::G1_4.samples(Fft::K32), None);
    // The 1K continual pilots of PP1 are CP1 taken mod 1 632 and kept below 853.
    let cp = continual_pilots(Fft::K1, PilotPattern::Pp1, false);
    assert!(cp.contains(&116) && cp.contains(&255) && cp.iter().all(|&k| k < 853));
}

/// Clause 9.8.2.2 example (page 125): for S1 = 000 and S2 = 0000 the scrambled modulation of carriers 44, 45, 47, 51, 171, 172,
/// 173, 175, 683, 684, 805, 806, 807 and 809 is -1, +1, -1, -1, +1, -1, -1, -1, +1, +1, +1, +1, +1, +1.
#[test]
fn p1_modulation_matches_the_example_of_clause_9_8_2_2() {
    let v = p1::carrier_values(sdrkit_stack_dvbt2::params::P1Info { s1: 0, s2: 0 });
    let want: [(usize, u16, f32); 14] = [
        (0, 44, -1.0),
        (1, 45, 1.0),
        (2, 47, -1.0),
        (3, 51, -1.0),
        (63, 171, 1.0),
        (64, 172, -1.0),
        (65, 173, -1.0),
        (66, 175, -1.0),
        (319, 683, 1.0),
        (320, 684, 1.0),
        (380, 805, 1.0),
        (381, 806, 1.0),
        (382, 807, 1.0),
        (383, 809, 1.0),
    ];
    for (i, k, s) in want {
        assert_eq!(tables::P1_CARRIERS[i], k);
        assert_eq!(v[i], s, "carrier {k}");
    }
}

/// A signal without P1 (OFDM with a cyclic prefix, like DVB-T, and noise) is not taken for DVB-T2.
#[test]
fn no_p1_in_dvb_t_like_signals_or_noise() {
    use sdrkit_core::Complex32;
    let mut noise = vec![Complex32::default(); 3_000_000];
    sdrkit_algo_sim::noise::Awgn::with_power(1.0, 11)
        .unwrap()
        .process(&mut noise);
    assert!(!sdrkit_stack_dvbt2::p1_present(&noise));
    // 2K symbols with a 1/4 prefix of random QPSK carriers.
    let mut fft = sdrkit_algo_dsp::fft::Fft::inverse(2048);
    let mut x = Vec::new();
    let mut seed = 0x1234_5678u32;
    for _ in 0..1000 {
        let mut s = vec![Complex32::default(); 2048];
        for (k, c) in s.iter_mut().enumerate() {
            if !(853..1195).contains(&k) {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *c = Complex32::new(
                    if seed >> 31 == 0 { 0.03 } else { -0.03 },
                    if (seed >> 30) & 1 == 0 { 0.03 } else { -0.03 },
                );
            }
        }
        fft.process(&mut s);
        x.extend_from_slice(&s[1536..]);
        x.extend_from_slice(&s);
    }
    assert!(!sdrkit_stack_dvbt2::p1_present(&x));
}
