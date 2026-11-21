//! TD-SCDMA: the code tables and code rules, the search on generated cells, and the stack end to end.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo};
use sdrkit_stack_cell::decoder::Capture;
use sdrkit_stack_cell::tdscdma::codes::{
    basic_midamble, midamble, scrambling, sync_dl, MIDAMBLE, SCRAMBLING, SYNC_DL,
};
use sdrkit_stack_cell::tdscdma::gen::{TdCell, TdGenerator, NATIVE_RATE};
use sdrkit_stack_cell::tdscdma::search::{TdFound, TdSearcher};
use sdrkit_stack_cell::TDSCDMA;
use serde_json::Value as J;

fn c(re: f32, im: f32) -> Complex32 {
    Complex32::new(re, im)
}

#[test]
fn complex_codes_follow_the_standard() {
    let j = c(0.0, 1.0);
    let one = c(1.0, 0.0);
    assert_eq!(&sync_dl(0)[..8], &[j, one, -j, one, -j, one, -j, one]);
    assert_eq!(&basic_midamble(0)[..8], &[j, one, -j, one, -j, one, -j, -one]);
}

#[test]
fn midamble_shifts_of_time_slot_0() {
    let b = basic_midamble(5);
    let m8 = midamble(&b, 8, 8);
    let m1 = midamble(&b, 1, 8);
    assert_eq!(m8[0], b[0]);
    assert_eq!(m1[0], b[112]);
    assert_eq!(m1[16], b[0]);
    assert_eq!(&m1[16..144], &b[..]);
}

#[test]
fn sync_dl_codes_cross_correlate_at_most_20_of_64() {
    let mut worst = 0f32;
    for a in 0..32 {
        for b in a + 1..32 {
            let x = sync_dl(a);
            let y = sync_dl(b);
            let s: Complex32 = x.iter().zip(&y).map(|(p, q)| p * q.conj()).sum();
            worst = worst.max(s.norm());
        }
    }
    assert!((worst - 20.0).abs() < 1e-3, "{worst}");
}

#[test]
fn scrambling_codes_and_cycling() {
    // Code 0 element 1 is -1, code 127 element 16 is -1 (binary elements before the j^i rule).
    assert_eq!(SCRAMBLING[0] >> 15, 0);
    assert_eq!(SCRAMBLING[127] & 1, 0);
    assert_eq!(scrambling(0)[0], c(0.0, -1.0));
    // The pair partner of parameter 4 is 5.
    assert_eq!(4 ^ 1, 5);
    assert_eq!(5 ^ 1, 4);
}

#[test]
fn tables_hold_the_standard_values() {
    assert_eq!(MIDAMBLE[0], 0xB2AC420F7C8DEBFA69505981BCD028C3);
    assert_eq!(MIDAMBLE[64], 0x06C848619AF1D6C910A8EAC4B622FC06);
    assert_eq!(MIDAMBLE[127], 0xD3ACF0078EDA9856BBB0AF8651132103);
    assert_eq!(SYNC_DL[0], 0xB3A7CC05A98688E4);
    assert_eq!(SYNC_DL[31], 0xABF0A0D905A939C4);
    assert_eq!(SCRAMBLING[0], 0x44B4);
    assert_eq!(SCRAMBLING[63], 0x5390);
    assert_eq!(SCRAMBLING[127], 0xA09C);
}

/// One receiver case: cells of `params` (with amplitudes), `offset` Hz plus 50 Hz a cell, `chip_offset` plus 3 chips a cell.
struct Case {
    params: &'static [(usize, f32)],
    offset: f64,
    ppm: f64,
    snr: f64,
    chip_offset: i64,
    s2: bool,
    sfn: i64,
}

fn cells_of(k: &Case) -> Vec<TdCell> {
    k.params
        .iter()
        .enumerate()
        .map(|(i, &(param, amp))| TdCell {
            param,
            sfn: k.sfn,
            offset_hz: k.offset + 50.0 * i as f64,
            amp,
            chip_offset: k.chip_offset + 3 * i as i64,
            s2_alternate: k.s2,
        })
        .collect()
}

fn run(cells: Vec<TdCell>, ppm: f64, snr_db: f64, conjugate: bool) -> Vec<TdFound> {
    let gen = TdGenerator::new(NATIVE_RATE, cells, 7, conjugate);
    let mut keys = snr(snr_db);
    keys.push(("gen_clock_ppm", J::from(ppm)));
    let x = impaired(gen, &keys, 0.08);
    let cap = Capture::new(x, NATIVE_RATE, 0.0, 0.08);
    TdSearcher::default().search(&cap, 6.0, 6)
}

/// The SFN parity of the first whole frame of a capture that starts at stream chip 0, for a cell whose first sub-frame starts
/// at `chip_offset` with SFN `sfn`. The pulse filters delay the signal by 6 chips.
fn first_parity(chip_offset: i64, sfn: i64) -> bool {
    let start = chip_offset + 6;
    let m = (-start).div_euclid(12_800) + i64::from((-start).rem_euclid(12_800) != 0);
    (sfn + m).rem_euclid(2) == 0
}

fn check(k: &Case) {
    let cells = cells_of(k);
    let found = run(cells.clone(), k.ppm, k.snr, false);
    let groups: Vec<usize> = cells.iter().map(|c| c.param / 4).collect();
    for f in &found {
        assert!(groups.contains(&f.code), "group {} was not sent: {found:?}", f.code);
    }
    for (i, cell) in cells.iter().enumerate() {
        let f = found
            .iter()
            .find(|f| f.code == cell.param / 4)
            .unwrap_or_else(|| panic!("cell {} not found: {found:?}", cell.param));
        assert_eq!(f.param, Some((cell.param, true)), "{f:?}");
        assert_eq!(f.parity_even, Some(first_parity(cell.chip_offset, cell.sfn)), "{f:?}");
        if i == 0 {
            assert!((f.offset_hz - cell.offset_hz).abs() < 500.0, "{f:?}");
            if k.s2 {
                assert!(f.s1 > 0 && f.s2 > 0, "{f:?}");
            } else {
                assert_eq!(f.s2, 0, "{f:?}");
            }
        }
    }
}

#[test]
fn case_1_one_cell_clean() {
    check(&Case { params: &[(0, 1.0)], offset: 0.0, ppm: 0.0, snr: 15.0, chip_offset: 0, s2: false, sfn: 0 });
}

#[test]
fn case_2_offset_and_5_db() {
    check(&Case { params: &[(37, 1.0)], offset: 2500.0, ppm: 0.0, snr: 5.0, chip_offset: 3001, s2: false, sfn: 7 });
}

#[test]
fn case_3_large_offset_clock_and_s2() {
    check(&Case { params: &[(90, 1.0)], offset: -6100.0, ppm: 20.0, snr: 0.0, chip_offset: -1777, s2: true, sfn: 12 });
}

#[test]
fn case_4_two_cells() {
    check(&Case {
        params: &[(127, 1.0), (66, 0.5)],
        offset: 900.0,
        ppm: -10.0,
        snr: 8.0,
        chip_offset: 501,
        s2: false,
        sfn: 3,
    });
}

#[test]
fn case_5_three_cells() {
    check(&Case {
        params: &[(13, 1.0), (54, 0.7), (109, 0.5)],
        offset: -300.0,
        ppm: 5.0,
        snr: 10.0,
        chip_offset: 4444,
        s2: true,
        sfn: 100,
    });
}

#[test]
fn case_6_minus_3_db() {
    check(&Case { params: &[(81, 1.0)], offset: 4000.0, ppm: 0.0, snr: -3.0, chip_offset: 6000, s2: false, sfn: 1 });
}

#[test]
fn case_7_noise_gives_no_cell() {
    let x = sdrkit_dev_test::Awgn::with_sigma(0.1, 3).noise((0.08 * NATIVE_RATE) as usize);
    let cap = Capture::new(x, NATIVE_RATE, 0.0, 0.08);
    assert!(TdSearcher::default().search(&cap, 6.0, 6).is_empty());
}

#[test]
fn the_conjugate_sync_dl_is_not_code_0() {
    let cells = vec![TdCell { param: 0, sfn: 0, offset_hz: 0.0, amp: 1.0, chip_offset: 0, s2_alternate: false }];
    let found = run(cells, 0.0, 15.0, true);
    assert!(found.iter().all(|f| f.code != 0), "{found:?}");
}

#[test]
fn case_8_the_stack_end_to_end() {
    let one = vec![TdCell { param: 50, sfn: 0, offset_hz: 0.0, amp: 1.0, chip_offset: 77, s2_alternate: false }];
    let x = impaired(TdGenerator::new(NATIVE_RATE, one, 3, false), &snr(12.0), 0.24);
    let mut rx = stack_graph(&TDSCDMA, &[]);
    let input = InputInfo::new(NATIVE_RATE, 2017.4e6, 2017.4e6).recording();
    let s = run_graph(&mut rx, input, &x, &[8192]);
    let cs = cells(&s);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["code"], 12);
    assert_eq!(cs[0]["param"], 50);
    assert_eq!(cs[0]["sure"], true);
}

#[test]
fn the_clean_test_signal_holds_two_cells() {
    let mut g = TdGenerator::new(NATIVE_RATE, sdrkit_stack_cell::tdscdma::gen::scene(), 0, false);
    let mut x = vec![Complex32::default(); (0.08 * NATIVE_RATE) as usize];
    sdrkit_core::Generator::generate(&mut g, &mut x);
    let cap = Capture::new(x, NATIVE_RATE, 0.0, 0.08);
    let found = TdSearcher::default().search(&cap, 6.0, 6);
    let mut codes: Vec<usize> = found.iter().map(|f| f.code).collect();
    codes.sort_unstable();
    assert_eq!(codes, vec![9, 22], "{found:#?}");
}
