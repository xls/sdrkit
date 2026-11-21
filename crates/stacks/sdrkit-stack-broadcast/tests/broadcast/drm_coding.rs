//! DRM coding against ES 201 980 V4.3.1: the puncturing of tables 27 and 28, the bit interleavers of clause 7.3.3, the mapping of
//! clause 7.4, the cell interleaver of clause 7.6, the SDC lengths of table 21, the pilot, FAC and SDC cells of clause 8, the
//! cell counts of tables 25 and 41 to 45, and the AAC frame CRC of clause 5.4.1, each written out from the standard's text.

use sdrkit_algo_fec::conv::{codes, Encoder};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::Complex32;
use sdrkit_stack_broadcast::drm::audio::aac_crc;
use sdrkit_stack_broadcast::drm::fac::{sdc_data_len, Sdc};
use sdrkit_stack_broadcast::drm::interleave::CellInterleaver;
use sdrkit_stack_broadcast::drm::mlc::{Mlc, Rate};
use sdrkit_stack_broadcast::drm::params::{
    self as p, afs_pilot, fac_cells, frame_pilot, freq_pilot, gain_phase, level_rates, pilot,
    time_pilot, time_pilots, CellMap, Qam, Robustness,
};

/// The permutation of clause 7.3.3 written from its formula: `s = 2^ceil(log2 n)`, `q = s/4 - 1`, `Π(0) = 0`,
/// `Π(i) = (t Π(i-1) + q) mod s`, values of `n` or more skipped.
fn pi(n: usize, t: usize) -> Vec<usize> {
    let s = n.next_power_of_two();
    let q = s / 4 - 1;
    let mut out = vec![0usize];
    let mut p = 0;
    while out.len() < n {
        p = (t * p + q) % s;
        if p < n {
            out.push(p);
        }
    }
    out
}

/// The bits of level `p` of the cells, in the order `y(p, 0), y(p, 1), ...` (the in-phase bit of a cell, then its quadrature
/// bit), read back from the axis values of figure 26: value `(L - 1 - 2 idx) a`.
fn level_bits(q: Qam, cells: &[Complex32], p: usize) -> Vec<u8> {
    let l = q.levels() as f32;
    let idx = |v: f32| (((l - 1.0) - v / q.scale()) / 2.0).round() as usize;
    cells
        .iter()
        .flat_map(|c| [(idx(c.re) >> p) as u8 & 1, (idx(c.im) >> p) as u8 & 1])
        .collect()
}

/// The mother code of `bits` with its six tail steps: `6 (n + 6)` bits, output `b_k` of step `t` at `6 t + k`.
fn mother(bits: &[u8]) -> Vec<u8> {
    let mut m = Vec::new();
    let mut e = Encoder::new(codes::DRM_K7_R6);
    e.encode(bits, &mut m);
    e.terminate(&mut m);
    m
}

/// Selects mother bits step by step: for step `t`, the outputs `k` with `rows[k][t mod period]` set, in the order `b0, b1, ...`
/// (the "transmitted sequence" of tables 27 and 28).
fn select(m: &[u8], steps: std::ops::Range<usize>, rows: &[&[u8]], out: &mut Vec<u8>) {
    for (i, t) in steps.enumerate() {
        for (k, row) in rows.iter().enumerate() {
            if row[i % row.len()] == 1 {
                out.push(m[6 * t + k]);
            }
        }
    }
}

#[test]
fn drm_puncturing_follows_tables_27_and_28() {
    // 64-QAM, protection level 3: level 0 at rate 2/3, level 1 at 4/5 (table 32). N = 301 cells: 2N - 12 = 590, so level 0
    // has r_p = 590 mod 3 = 2 and level 1 has r_p = 590 mod 5 = 0.
    let n = 301;
    let (rates, _) = level_rates(Robustness::B, Qam::Q64, 3).unwrap();
    let mut mlc = Mlc::eep(Qam::Q64, rates, n).unwrap();
    let m0 = 2 * (590 / 3);
    let m1 = 4 * (590 / 5);
    assert_eq!(mlc.level_bits()[..2], [(0, m0), (0, m1)]);
    let mut rng = Rng::new(9);
    let bits: Vec<u8> = (0..mlc.info_bits()).map(|_| rng.bit()).collect();
    let mut cells = vec![Complex32::default(); n];
    mlc.encode(&bits, &mut cells);

    // Level 0 is not interleaved (figure 20): its bits are the punctured sequence itself.
    let m = mother(&bits[..m0]);
    let mut want = Vec::new();
    // Table 27, rate 2/3: B0 1 1, B1 1 0.
    select(&m, 0..m0, &[&[1, 1], &[1, 0]], &mut want);
    // Table 28, r_p = 2: B0 and B1 all ones, B2 1 0 0 1 0 0.
    select(
        &m,
        m0..m0 + 6,
        &[&[1; 6], &[1; 6], &[1, 0, 0, 1, 0, 0]],
        &mut want,
    );
    assert_eq!(want.len(), 2 * n);
    assert_eq!(level_bits(Qam::Q64, &cells, 0), want);

    // Level 1: table 27 rate 4/5 (B0 1 1 1 1, B1 1 0 0 0), table 28 r_p = 0 (B0 and B1 all ones), then the bit interleaver with
    // t = 13 over 2N bits: y(1, i) = v(1, Π(i)).
    let m = mother(&bits[m0..m0 + m1]);
    let mut v = Vec::new();
    select(&m, 0..m1, &[&[1, 1, 1, 1], &[1, 0, 0, 0]], &mut v);
    select(&m, m1..m1 + 6, &[&[1; 6], &[1; 6]], &mut v);
    assert_eq!(v.len(), 2 * n);
    let p = pi(2 * n, 13);
    let want: Vec<u8> = (0..2 * n).map(|i| v[p[i]]).collect();
    assert_eq!(level_bits(Qam::Q64, &cells, 1), want);

    // And the code decodes what it sent.
    let mut out = Vec::new();
    mlc.decode(&cells, &vec![10.0; n], 2, &mut out);
    assert_eq!(out, bits);
}

#[test]
fn the_fac_runs_its_pattern_over_the_tail() {
    // Clause 7.3.2: "For the FAC, all bits are punctured according to table 27". 72 bits + 6 tail steps at 3/5 give 130 bits on
    // 65 cells. Mode E: 116 + 6 steps at 1/4 give 488 bits on 244 cells. 4-QAM level 0 is interleaved with t = 21.
    for (rate, info, cells, rows) in [
        (
            Rate::new(3, 5),
            72usize,
            65usize,
            vec![vec![1u8, 1, 1], vec![1, 0, 1]],
        ),
        (
            Rate::new(1, 4),
            116,
            244,
            vec![vec![1u8], vec![1], vec![1], vec![1]],
        ),
    ] {
        let mut mlc = Mlc::fac(rate, info, cells).unwrap();
        let mut rng = Rng::new(3);
        let bits: Vec<u8> = (0..info).map(|_| rng.bit()).collect();
        let mut z = vec![Complex32::default(); cells];
        mlc.encode(&bits, &mut z);
        let m = mother(&bits);
        let rows: Vec<&[u8]> = rows.iter().map(|r| r.as_slice()).collect();
        let mut v = Vec::new();
        select(&m, 0..info + 6, &rows, &mut v);
        assert_eq!(v.len(), 2 * cells);
        let p = pi(2 * cells, 21);
        let want: Vec<u8> = (0..2 * cells).map(|i| v[p[i]]).collect();
        assert_eq!(level_bits(Qam::Q4, &z, 0), want);
    }
}

#[test]
fn the_mapping_is_that_of_figures_26_29_and_30() {
    // All zeros is the largest positive value. b0 = 1 is the next one down.
    let a64 = 1.0 / 42f32.sqrt();
    assert_eq!(Qam::Q64.axis(0), 7.0 * a64);
    assert_eq!(Qam::Q64.axis(1), 5.0 * a64);
    assert_eq!(Qam::Q64.axis(4), -a64);
    assert_eq!(Qam::Q64.axis(7), -7.0 * a64);
    assert_eq!(Qam::Q16.axis(0), 3.0 / 10f32.sqrt());
    assert_eq!(Qam::Q4.axis(1), -std::f32::consts::FRAC_1_SQRT_2);
    // Mean power 1.
    for q in [Qam::Q4, Qam::Q16, Qam::Q64] {
        let l = q.levels();
        let p: f32 = (0..l)
            .flat_map(|i| (0..l).map(move |k| q.map(i, k).norm_sqr()))
            .sum::<f32>()
            / (l * l) as f32;
        assert!((p - 1.0).abs() < 1e-5);
    }
}

#[test]
fn the_cell_interleaver_is_clause_7_6() {
    // ẑ(n, i) = z(n - (i mod D), Π(i)), Π with t0 = 5, for the long (D = 5) and short (D = 1) forms.
    let n = 23;
    let p = pi(n, 5);
    for d in [1usize, 5] {
        let mut il = CellInterleaver::new(n, d);
        let frame = |k: usize| -> Vec<Complex32> {
            (0..n).map(|j| Complex32::new(k as f32, j as f32)).collect()
        };
        for k in 0..12 {
            let mut out = Vec::new();
            il.push(&frame(k), &mut out, |_| Complex32::new(-1.0, -1.0));
            for (i, c) in out.iter().enumerate() {
                let from = k as i64 - (i % d) as i64;
                let want = if from < 0 {
                    Complex32::new(-1.0, -1.0)
                } else {
                    Complex32::new(from as f32, p[i] as f32)
                };
                assert_eq!(*c, want, "D {d} frame {k} cell {i}");
            }
        }
    }
}

#[test]
fn the_sdc_lengths_are_those_of_table_21() {
    // N_SDC of table 25 and the rates of tables 36 and 37 give the data field lengths of table 21.
    for (mode, sdc_mode, cells, bytes) in [
        (Robustness::B, 0u8, 322usize, 76usize),
        (Robustness::A, 1, 846, 102),
        (Robustness::A, 0, 167, 37),
        (Robustness::E, 1, 936, 55),
    ] {
        let q = sdrkit_stack_broadcast::drm::params::sdc_qam(mode, sdc_mode);
        let rates = sdrkit_stack_broadcast::drm::params::sdc_rates(mode, sdc_mode);
        let m = Mlc::eep(q, rates, cells).unwrap();
        assert_eq!(
            sdc_data_len(m.info_bits()),
            bytes,
            "{mode:?} {sdc_mode} {cells}"
        );
    }
    // The block sends the AFS index in 4 bits (clause 6.4.2): 20 bits of overhead around the data field.
    let s = Sdc::default();
    assert_eq!(s.to_bits(10).unwrap().len(), 4 + 80 + 16);
}

/// MOT header parameter 0x29 is the slideshow's Alert (ETSI TS 101 499), not a caption: an emergency slide reads as an alert.
#[test]
fn mot_parameter_0x29_is_the_alert() {
    use sdrkit_proto_bcdata::mot::{Header, HeaderWriter, Object};
    use sdrkit_stack_broadcast::drm::data::MotObject;
    let body = [0u8; 10];
    let mut h = Vec::new();
    HeaderWriter::new(body.len() as u32, 2, 1)
        .content_name("alert.jpg")
        .param(0x29, &[1])
        .write(&mut h)
        .unwrap();
    let o = MotObject::from_object(&Object {
        transport_id: 7,
        header: Header::parse(&h).unwrap(),
        body: &body,
    });
    assert_eq!(o.name, "alert.jpg");
    assert_eq!(o.content_type_text(), "image/JPEG");
    assert_eq!(o.slide.expect("a slide").alert, Some(1));
}

const SQRT2: f32 = std::f32::consts::SQRT_2;

/// The modes and occupancies the standard allows.
fn configurations() -> Vec<(Robustness, u8)> {
    Robustness::ALL
        .iter()
        .flat_map(|&r| {
            (0..6u8)
                .filter(move |&o| r.range(o).is_some())
                .map(move |o| (r, o))
        })
        .collect()
}

fn amp_phase(v: Complex32) -> (f32, f32) {
    (v.norm(), v.arg().rem_euclid(std::f32::consts::TAU))
}

fn phase_of(theta: u16) -> f32 {
    std::f32::consts::TAU * theta as f32 / 1024.0
}

fn close(v: Complex32, a: f32, theta: u16) -> bool {
    (v - Complex32::from_polar(a, phase_of(theta))).norm() < 1e-4
}

#[test]
fn drm_carriers_follow_tables_49_and_50() {
    // Table 49.
    assert_eq!(Robustness::A.range(4), Some((-98, 314)));
    assert_eq!(Robustness::B.range(5), Some((-99, 311)));
    assert_eq!(Robustness::C.range(5), Some((-67, 213)));
    assert_eq!(Robustness::D.range(3), Some((-44, 44)));
    assert_eq!(Robustness::E.range(0), Some((-106, 106)));
    assert!(Robustness::C.range(2).is_none() && Robustness::E.range(1).is_none());
    // Table 50 (its text is garbled in the extracted copy, so the count tests of tables 25 and 41 to 45 confirm it too).
    let unused = |r: Robustness| (-3..=3).filter(|&k| r.unused(k)).collect::<Vec<i32>>();
    assert_eq!(unused(Robustness::A), [-1, 0, 1]);
    for r in [Robustness::B, Robustness::C, Robustness::D] {
        assert_eq!(unused(r), [0]);
    }
    assert!(unused(Robustness::E).is_empty());
    // Clause 8.5.3.1: the SDC symbols. Clause 8.4.5.0: the AFS pilot symbols of mode E.
    assert_eq!(p::SDC_SYMBOLS, [2, 2, 3, 3, 5]);
    assert_eq!(p::AFS_SYMBOLS, [(0, 4), (3, 39)]);
}

#[test]
fn drm_frequency_pilots_follow_tables_51_and_52() {
    // Table 51 carriers and table 52 phase indices of symbol 0.
    assert_eq!(p::FREQ_PILOTS[0], [(18, 205), (54, 836), (72, 215)]);
    assert_eq!(p::FREQ_PILOTS[1], [(16, 331), (48, 651), (64, 555)]);
    assert_eq!(p::FREQ_PILOTS[2], [(11, 214), (33, 392), (44, 242)]);
    assert_eq!(p::FREQ_PILOTS[3], [(7, 788), (21, 1014), (28, 332)]);
    assert!(p::FREQ_PILOTS[4].is_empty());
    // Clause 8.4.2.2: the phase of symbol 0 in every symbol. Mode D carriers 7 and 21 add 512 in odd symbols.
    assert_eq!(freq_pilot(Robustness::A, 7, 54), Some(836));
    assert_eq!(freq_pilot(Robustness::D, 1, 7), Some(276));
    assert_eq!(freq_pilot(Robustness::D, 2, 7), Some(788));
    assert_eq!(freq_pilot(Robustness::D, 3, 21), Some(502));
    assert_eq!(freq_pilot(Robustness::D, 3, 28), Some(332));
    // A power gain of 2 in every symbol.
    for s in 0..15 {
        assert!(close(pilot(Robustness::B, 3, s, 48).unwrap(), SQRT2, 651));
    }
}

#[test]
fn drm_time_pilots_follow_tables_53_to_57() {
    let n: Vec<usize> = Robustness::ALL
        .iter()
        .map(|&r| time_pilots(r).len())
        .collect();
    assert_eq!(n, [21, 19, 19, 21, 21]);
    for (r, k, theta) in [
        (Robustness::A, 17, 973),
        (Robustness::A, 60, 1008),
        (Robustness::A, 73, 727),
        (Robustness::B, 14, 304),
        (Robustness::B, 68, 428),
        (Robustness::C, 28, 3),
        (Robustness::C, 46, 754),
        (Robustness::D, 6, 124),
        (Robustness::D, 32, 452),
        (Robustness::E, -80, 219),
        (Robustness::E, 68, 29),
        (Robustness::E, 80, 508),
    ] {
        assert_eq!(time_pilot(r, k), Some(theta), "{r:?} {k}");
        // Symbol 0 only, with a power gain of 2 (clause 8.4.3.2).
        assert!(close(
            pilot(r, r.usual_occupancy(), 0, k).unwrap(),
            SQRT2,
            theta
        ));
    }
    // The carriers marked "*" in tables 53 to 56 are frequency pilots too, with the same phase.
    for r in [Robustness::A, Robustness::B, Robustness::C, Robustness::D] {
        for &(k, theta) in p::FREQ_PILOTS[r as usize] {
            assert_eq!(time_pilot(r, k), Some(theta), "{r:?} {k}");
        }
    }
}

#[test]
fn drm_gain_pilots_follow_tables_58_and_60() {
    assert_eq!(
        p::GAIN_PATTERN,
        [(4, 5, 2), (2, 3, 1), (2, 2, 1), (1, 3, 1), (4, 4, 2)]
    );
    // Table 58: mode A symbol 0 has gain pilots on 2 + 20 p, symbol 3 on 14 + 20 p, mode D symbol 2 on 3 + 3 p.
    assert!(
        gain_phase(Robustness::A, 0, 22).is_some() && gain_phase(Robustness::A, 0, 26).is_none()
    );
    assert!(
        gain_phase(Robustness::A, 3, -6).is_some() && gain_phase(Robustness::A, 3, 2).is_none()
    );
    assert!(gain_phase(Robustness::D, 2, 0).is_some() && gain_phase(Robustness::D, 2, 1).is_none());
    // Spot values of the small tables of clauses 8.4.4.3.2 to 8.4.4.3.6.
    assert_eq!(
        (p::GAIN_W_A[3][1], p::GAIN_Z_A[4][2], p::GAIN_Q_A),
        (0, 111, 36)
    );
    assert_eq!(
        (p::GAIN_W_B[1][1], p::GAIN_Z_B[1][1], p::GAIN_Q_B),
        (512, 255, 12)
    );
    assert_eq!(
        (p::GAIN_W_C[1][9], p::GAIN_Z_C[0][7], p::GAIN_Q_C),
        (93, 248, 12)
    );
    assert_eq!(
        (p::GAIN_W_D[1][3], p::GAIN_Z_D[2][4], p::GAIN_Q_D),
        (951, 253, 14)
    );
    assert_eq!(
        (p::GAIN_R_E[1][2], p::GAIN_Z_E[1][1], p::GAIN_Q_E[1][1]),
        (402, 914, 1023)
    );
    // Mode A, s = 1, k = 6: n = 1, m = 0, p = 0, so the phase is 4 Z256[1][0] = 72. s = 0, k = 22: p = 1, so
    // 4 * 0 + 228 + 1 * 1 * 36 = 264.
    assert_eq!(gain_phase(Robustness::A, 1, 6), Some(72));
    assert_eq!(gain_phase(Robustness::A, 0, 22), Some(264));
    // Every symbol of every occupancy has gain pilots at the period x y of table 58, inside the occupancy.
    for (r, occ) in configurations() {
        let (x, y, _) = p::GAIN_PATTERN[r as usize];
        let (lo, hi) = r.range(occ).unwrap();
        for s in 0..r.nsym() {
            let ks: Vec<i32> = (lo..=hi)
                .filter(|&k| gain_phase(r, s, k).is_some())
                .collect();
            assert!(
                ks.windows(2).all(|w| w[1] - w[0] == x * y),
                "{r:?} {occ} {s}"
            );
            for k in ks {
                assert!(gain_phase(r, s, k).unwrap() < 1024);
                if !r.unused(k) {
                    assert!(pilot(r, occ, s, k).is_some(), "{r:?} {occ} {s} {k}");
                }
            }
        }
    }
}

#[test]
fn drm_mode_e_gain_phases_agree_with_the_afs_pilots_of_table_61() {
    // The "*" cells of table 61 are gain pilots too and the note says their phases agree: 13 in symbol 4 and 13 in symbol 39.
    let starred_4 = [-94, -78, -62, -46, -30, -14, 2, 18, 34, 50, 66, 82, 98];
    let starred_39 = [-98, -82, -66, -50, -34, -18, -2, 14, 30, 46, 62, 78, 94];
    let r = Robustness::E;
    let mut checked = 0;
    for &(k, t4, t39) in &p::AFS_PILOTS {
        for (s, theta, starred) in [(4, t4, &starred_4), (39, t39, &starred_39)] {
            assert_eq!(
                gain_phase(r, s, k).is_some(),
                starred.contains(&k),
                "{s} {k}"
            );
            if let Some(g) = gain_phase(r, s, k) {
                assert_eq!(g, theta, "{s} {k}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 26);
    // Table 61 values, and amplitude 1 for the cells that are AFS pilots only (clause 8.4.5.2).
    assert_eq!(p::AFS_PILOTS.len(), 54);
    assert_eq!(p::AFS_PILOTS[0], (-106, 134, 115));
    assert_eq!(p::AFS_PILOTS[31], (18, 1001, 610));
    assert_eq!(p::AFS_PILOTS[53], (106, 868, 134));
    assert!(close(afs_pilot(r, 0, 4, -106).unwrap(), 1.0, 134));
    assert!(close(afs_pilot(r, 3, 39, 102).unwrap(), 1.0, 115));
    assert!(afs_pilot(r, 1, 4, -106).is_none() && afs_pilot(r, 0, 39, -106).is_none());
    // A starred cell keeps the gain pilot's amplitude.
    assert!((amp_phase(frame_pilot(r, 0, 0, 4, 2).unwrap()).0 - SQRT2).abs() < 1e-5);
}

#[test]
fn drm_boosted_pilots_are_those_of_table_59() {
    assert_eq!(p::BOOSTED_PILOTS[0][4], Some([-98, -94, 310, 314]));
    assert_eq!(p::BOOSTED_PILOTS[1][3], Some([-103, -101, 101, 103]));
    assert_eq!(p::BOOSTED_PILOTS[3][5], Some([-43, -42, 134, 135]));
    assert_eq!(p::BOOSTED_PILOTS[4][0], Some([-106, -102, 102, 106]));
    for r in Robustness::ALL {
        for occ in 0..6u8 {
            let b = p::BOOSTED_PILOTS[r as usize][occ as usize];
            let Some((lo, hi)) = r.range(occ) else {
                assert!(b.is_none());
                continue;
            };
            // The two outermost gain pilot carriers at each edge of the occupancy.
            let ks: Vec<i32> = (lo..=hi)
                .filter(|&k| (0..r.nsym()).any(|s| gain_phase(r, s, k).is_some()))
                .collect();
            let want = [ks[0], ks[1], ks[ks.len() - 2], ks[ks.len() - 1]];
            assert_eq!(b, Some(want), "{r:?} {occ}");
            for k in want {
                assert!(time_pilot(r, k).is_none() && freq_pilot(r, 0, k).is_none());
                for s in 0..r.nsym() {
                    if gain_phase(r, s, k).is_some() {
                        assert!((amp_phase(pilot(r, occ, s, k).unwrap()).0 - 2.0).abs() < 1e-5);
                    }
                }
            }
        }
    }
}

#[test]
fn drm_fac_cells_follow_tables_62_to_66() {
    let count = |r: Robustness| fac_cells(r).iter().map(|x| x.1.len()).sum::<usize>();
    for r in Robustness::ALL {
        // Clause 8.5.2.1: 65 FAC cells, 244 in mode E.
        assert_eq!(count(r), r.fac_cells(), "{r:?}");
    }
    assert_eq!(fac_cells(Robustness::A)[0], (2, &[26, 46, 66, 86][..]));
    assert_eq!(
        fac_cells(Robustness::B)[11],
        (13, &[23, 35, 47, 65, 77][..])
    );
    assert_eq!(fac_cells(Robustness::C)[4], (7, &[5, 17, 29, 41, 53][..]));
    assert_eq!(fac_cells(Robustness::D)[1], (4, &[10, 19][..]));
    assert_eq!(fac_cells(Robustness::E)[21], (26, &[-90, -74, -58][..]));
    // Every FAC cell is a data cell of every frame and occupancy: in range, not unused, no pilot, not in an SDC symbol.
    for (r, occ) in configurations() {
        let (lo, hi) = r.range(occ).unwrap();
        for &(s, ks) in fac_cells(r) {
            assert!(s >= r.sdc_symbols() && s < r.nsym());
            assert!(ks.windows(2).all(|w| w[0] < w[1]));
            for &k in ks {
                assert!(k >= lo && k <= hi && !r.unused(k), "{r:?} {occ} {s} {k}");
                for f in 0..r.frames_per_superframe() {
                    assert!(
                        frame_pilot(r, occ, f, s, k).is_none(),
                        "{r:?} {occ} {f} {s} {k}"
                    );
                }
            }
        }
    }
}

#[test]
fn drm_cell_counts_follow_tables_25_and_41_to_45() {
    assert_eq!(p::N_SDC[1][3], Some(322));
    assert_eq!(p::N_SDC[4][0], Some(936));
    assert_eq!(
        p::MSC_CELLS[0][0].map(|m| (m.available, m.useful, m.mux, m.loss)),
        Some((3778, 3777, 1259, 1))
    );
    assert_eq!(p::MSC_CELLS[1][3].map(|m| m.mux), Some(2337));
    assert_eq!(p::MSC_CELLS[4][0].map(|m| m.loss), Some(2));
    for r in Robustness::ALL {
        for occ in 0..6u8 {
            let sdc = p::N_SDC[r as usize][occ as usize];
            let msc = p::MSC_CELLS[r as usize][occ as usize];
            let Some(map) = CellMap::new(r, occ) else {
                assert!(sdc.is_none() && msc.is_none(), "{r:?} {occ}");
                continue;
            };
            let msc = msc.unwrap();
            let mtf = r.frames_per_superframe() as u32;
            // The table's own arithmetic (clause 7.7): N_SFA = N_SFU + N_L, N_SFU = M_TF N_MUX.
            assert_eq!(msc.available, msc.useful + msc.loss);
            assert_eq!(msc.useful, mtf * msc.mux);
            // The layout produces the table's counts.
            assert_eq!(map.sdc.len() as u32, sdc.unwrap(), "{r:?} {occ} SDC");
            assert_eq!(
                map.msc_per_superframe() as u32,
                msc.available,
                "{r:?} {occ} MSC"
            );
            assert_eq!(map.mux_cells() as u32, msc.mux, "{r:?} {occ} N_MUX");
            assert_eq!(map.fac.len(), r.fac_cells());
        }
    }
}

#[test]
fn drm_cells_are_in_carrier_then_symbol_order() {
    // Clauses 8.5.2.2, 8.5.3.2 and 8.6.2: increasing carrier index, then time order. SDC symbols start at symbol 0.
    for (r, occ) in configurations() {
        let map = CellMap::new(r, occ).unwrap();
        let (lo, hi) = r.union_range();
        let k = (hi - lo + 1) as usize;
        assert!(map.sdc.windows(2).all(|w| w[0] < w[1]));
        assert!(map.sdc[0] / k == 0 && map.sdc.last().unwrap() / k == r.sdc_symbols() - 1);
        assert!(map.fac.windows(2).all(|w| w[0] < w[1]));
        for (f, m) in map.msc.iter().enumerate() {
            assert!(m.windows(2).all(|w| w[0] < w[1]));
            let first = if f == 0 { r.sdc_symbols() } else { 0 };
            assert_eq!(m[0] / k, first, "{r:?} {occ} {f}");
        }
    }
}

#[test]
fn drm_layout_known_cells_are_the_pilots_of_the_occupancy() {
    for (r, occ) in configurations() {
        let l = p::layout(r, occ).unwrap();
        let (lo, _) = r.union_range();
        for (s, row) in l.known.iter().enumerate() {
            for &(c, v) in row {
                assert_eq!(pilot(r, occ, s, lo + c as i32), Some(v));
            }
            let n = (0..l.ncarriers())
                .filter(|&c| pilot(r, occ, s, lo + c as i32).is_some())
                .count();
            assert_eq!(row.len(), n);
        }
    }
}

#[test]
fn drm_search_layout_keeps_the_pilots_every_occupancy_agrees_on() {
    for r in Robustness::ALL {
        let l = p::search_layout(r);
        let (lo, _) = r.union_range();
        let mut left_out = 0;
        for s in 0..r.nsym() {
            for c in 0..l.ncarriers() {
                let k = lo + c as i32;
                let vals: Vec<_> = configurations()
                    .into_iter()
                    .filter(|&(m, o)| m == r && r.range(o).is_some_and(|(a, b)| k >= a && k <= b))
                    .map(|(_, o)| pilot(r, o, s, k))
                    .collect();
                let agreed = vals[0].is_some() && vals.iter().all(|v| *v == vals[0]);
                let known = l.known[s].iter().find(|x| x.0 == c).map(|x| x.1);
                assert_eq!(known, if agreed { vals[0] } else { None }, "{r:?} {s} {k}");
                if vals[0].is_some() && !agreed {
                    left_out += 1;
                    // Only a carrier that table 59 boosts in some occupancy is left out.
                    assert!((0..6u8).any(|o| p::is_boosted(r, o, k)), "{r:?} {s} {k}");
                }
            }
        }
        // Mode E has one occupancy: nothing is left out.
        assert_eq!(left_out == 0, r == Robustness::E, "{r:?}");
    }
}

/// The CRC of annex D written from its text: a shift register of n stages set to all ones, the data entered MSb first, the
/// register read out and inverted. G(x) = x^8 + x^4 + x^3 + x^2 + 1 for the AAC frame CRC of clause 5.4.1.
fn annex_d_crc8(bits: &[u8]) -> u8 {
    let mut b = [1u8; 8];
    let g = [1u8, 0, 1, 1, 1, 0, 0, 0]; // g0 .. g7: x^0, x^2, x^3, x^4
    for &d in bits {
        let fb = d ^ b[7];
        for i in (1..8).rev() {
            b[i] = b[i - 1] ^ (fb & g[i]);
        }
        b[0] = fb;
    }
    b.iter()
        .enumerate()
        .fold(0u8, |acc, (i, &v)| acc | (v << i))
        ^ 0xFF
}

#[test]
fn drm_aac_crc_covers_the_bits_annex_d_says() {
    // The well-known check value of this CRC (CRC-8/SAE-J1850 parameters) over "123456789" is 0x4B.
    let msg = b"123456789";
    let bits: Vec<u8> = msg
        .iter()
        .flat_map(|&c| (0..8).rev().map(move |i| (c >> i) & 1))
        .collect();
    assert_eq!(annex_d_crc8(&bits), 0x4B);
    assert_eq!(aac_crc(msg, 72), Some(0x4B));
    // Only the first crc_bits bits count (clause 5.4.1: the side information and TNS bits), also when they end inside a byte.
    for n in [0usize, 1, 13, 45, 48, 71] {
        assert_eq!(aac_crc(msg, n), Some(annex_d_crc8(&bits[..n])), "{n}");
    }
    let mut changed = *msg;
    changed[6] ^= 0x01; // bit 55
    assert_eq!(aac_crc(&changed, 55), aac_crc(msg, 55));
    assert_ne!(aac_crc(&changed, 56), aac_crc(msg, 56));
    assert_eq!(aac_crc(msg, 73), None);
}
