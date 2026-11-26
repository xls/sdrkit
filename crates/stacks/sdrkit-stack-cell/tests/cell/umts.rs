//! UMTS: the synchronisation and scrambling codes, the BCH coding and the system information, the search on generated cells,
//! and the stack end to end.

use crate::common::*;
use sdrkit_algo_modem::spreading::wcdma::{ssc, DlScrambling, SSC_GROUPS};
use sdrkit_core::InputInfo;
use sdrkit_stack_cell::decoder::Capture;
use sdrkit_stack_cell::umts::sib::{parse_sib3, sib3_data, Sib3};
use sdrkit_stack_cell::umts::bch::{
    complete_sib_block, crc_bits, decode, encode, mib_data, parse_mib, Mib,
    SibAssembly, SIB_3, SIB_MIB,
};
use sdrkit_stack_cell::umts::gen::{UmtsCell, UmtsGenerator, NATIVE_RATE};
use sdrkit_stack_cell::umts::search::{UmtsFound, UmtsSearcher};
use sdrkit_stack_cell::UMTS;
use sdrkit_util_bits::pack::push_u64;
use serde_json::Value as J;

fn hex(bits: &[u8]) -> String {
    let mut b = bits.to_vec();
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    b.chunks(4)
        .map(|q| format!("{:X}", q.iter().fold(0u8, |a, &v| a << 1 | v)))
        .collect()
}

fn chips_hex(c: impl Iterator<Item = i8>) -> String {
    hex(&c.map(|v| u8::from(v < 0)).collect::<Vec<_>>())
}

#[test]
fn synchronisation_and_scrambling_codes() {
    assert_eq!(
        chips_hex(ssc(2).into_iter()),
        "03A9FC5603A903A903A9FC56FC5603A903A903A903A903A9FC5603A9FC5603A9"
    );
    let c = DlScrambling::primary(1);
    assert_eq!(chips_hex(c[..32].iter().map(|v| v.0)), "DFFBC8B9");
}

#[test]
fn the_ssc_table_is_comma_free() {
    assert_eq!(SSC_GROUPS.len(), 64);
    // Every cyclic shift of every row differs from every other shift of every row.
    let mut all = std::collections::HashSet::new();
    for row in SSC_GROUPS.iter() {
        for o in 0..15 {
            let s: [u8; 15] = std::array::from_fn(|k| row[(k + o) % 15]);
            assert!(all.insert(s));
        }
    }
    assert_eq!(all.len(), 64 * 15);
}

fn mib_246() -> Vec<u8> {
    complete_sib_block(
        0,
        SIB_MIB,
        &mib_data(&Mib {
            mcc: 246,
            mnc: 3,
            mnc_digits: 2,
        }),
    )
}

/// A block of the first 30 bits of SIB 3 (to the cell identity), the coding vector of the test.
fn sib3_0abcdef() -> Vec<u8> {
    complete_sib_block(0, SIB_3, &sib3_data(&s3_0abcdef())[..30])
}

#[test]
fn bch_coding_of_two_blocks() {
    let pad = "0".repeat(62 - 14);
    for (block, want, crc, sent, f1, f2) in [
        (
            mib_246(),
            format!("00120009180600{pad}"),
            "E5FB",
            "DFA7",
            "402050200C000501",
            "402010200C080401",
        ),
        (
            sib3_0abcdef(),
            format!("0012302AF37BC0{pad}"),
            "6013",
            "C806",
            "404040280C0A0001",
            "0020100008040501",
        ),
    ] {
        assert_eq!(block.len(), 246);
        assert_eq!(hex(&block), want);
        let parity = crc_bits(&block);
        let remainder: Vec<u8> = parity.iter().rev().copied().collect();
        assert_eq!(hex(&remainder), crc);
        assert_eq!(hex(&parity), sent);
        let frames = encode(&block);
        assert_eq!(hex(&frames[0][..64]), f1);
        assert_eq!(hex(&frames[1][..64]), f2);
        // Hard decisions without noise decode back to the block.
        let soft = |f: &[u8]| -> Vec<f32> { f.iter().map(|&b| 1.0 - 2.0 * f32::from(b)).collect() };
        assert_eq!(decode(&soft(&frames[0]), &soft(&frames[1])), Some(block));
    }
}

#[test]
fn the_mib_and_sib3_are_read() {
    let mut a = SibAssembly::default();
    let (_, sibs) = a.push(&mib_246()).unwrap();
    assert_eq!(sibs.len(), 1);
    assert_eq!(
        parse_mib(&sibs[0].1),
        Some(Mib {
            mcc: 246,
            mnc: 3,
            mnc_digits: 2
        })
    );
    let (_, sibs) = a.push(&sib3_0abcdef()).unwrap();
    let (cell, _) = parse_sib3(&sibs[0].1).unwrap();
    assert_eq!(cell, 11_259_375);
    assert_eq!((cell >> 16, cell & 0xFFFF), (171, 52_719));
}

#[test]
fn a_segmented_sib_is_joined_and_a_missing_segment_discards_it() {
    // SIB 3 data split: a first segment of 222 bits (count 2), then a last short segment.
    let mut data = sib3_data(&s3_0abcdef());
    data.resize(230, 1);
    let first = {
        let mut b = Vec::new();
        push_u64(&mut b, 0, 11);
        push_u64(&mut b, 1, 4);
        push_u64(&mut b, u64::from(SIB_3), 5);
        push_u64(&mut b, 1, 4);
        b.extend_from_slice(&data[..222]);
        b.resize(246, 0);
        b
    };
    let last = {
        let mut b = Vec::new();
        push_u64(&mut b, 1, 11);
        push_u64(&mut b, 3, 4);
        push_u64(&mut b, u64::from(SIB_3), 5);
        push_u64(&mut b, 0, 4);
        push_u64(&mut b, 8 - 1, 8);
        b.extend_from_slice(&data[222..230]);
        b.resize(246, 0);
        b
    };
    let mut a = SibAssembly::default();
    assert!(a.push(&first).unwrap().1.is_empty());
    let (_, sibs) = a.push(&last).unwrap();
    assert_eq!(sibs.len(), 1);
    assert_eq!(sibs[0].1, data);
    assert_eq!(parse_sib3(&sibs[0].1).map(|x| x.0), Some(0x0AB_CDEF));
    // Without the first segment the last one gives nothing.
    let mut b = SibAssembly::default();
    assert!(b.push(&last).unwrap().1.is_empty());
}

/// A cell of a receiver case: (code, offset Hz, amplitude, chip offset, data power).
fn cell(code: usize, offset: f64, amp: f32, chip_offset: i64, data: f32) -> UmtsCell {
    UmtsCell {
        code,
        offset_hz: offset,
        amp,
        chip_offset,
        data_power: data,
        broadcast: None,
        sttd: false,
    }
}

fn plmn(mcc: u16, mnc: u16) -> Mib {
    Mib {
        mcc,
        mnc,
        mnc_digits: 2,
    }
}

fn run(cells: Vec<UmtsCell>, snr_db: f64, ppm: f64, range: f64, seconds: f64) -> Vec<UmtsFound> {
    let gen = UmtsGenerator::new(NATIVE_RATE, cells, 11, 0.22);
    let mut keys = snr(snr_db);
    keys.push(("gen_clock_ppm", J::from(ppm)));
    let x = impaired(gen, &keys, seconds);
    let cap = Capture::new(x, NATIVE_RATE, 0.0, seconds);
    UmtsSearcher::default().search(&cap, range, 6, true)
}

fn check(cells: Vec<UmtsCell>, snr_db: f64, range: f64) -> Vec<UmtsFound> {
    let found = run(cells.clone(), snr_db, 0.0, range, 0.08);
    assert!(found.len() <= 6);
    for c in &cells {
        let f = found
            .iter()
            .find(|f| f.code == c.code)
            .unwrap_or_else(|| panic!("code {} not found: {found:?}", c.code));
        assert!((f.offset_hz - c.offset_hz).abs() <= 1400.0, "{f:?}");
    }
    found
}

#[test]
fn case_1_one_cell() {
    check(vec![cell(0, 0.0, 1.0, 137, 0.0)], 30.0, 2500.0);
}

#[test]
fn case_2_code_511() {
    check(vec![cell(511, 1800.0, 1.0, 900, 0.5)], 10.0, 7500.0);
}

#[test]
fn case_3_at_3_db() {
    check(vec![cell(97, -4200.0, 1.0, 3000, 0.4)], 3.0, 7500.0);
}

#[test]
fn case_4_three_cells() {
    check(
        vec![
            cell(12, 0.0, 1.0, 500, 0.3),
            cell(200, 2100.0, 0.8, 7000, 0.3),
            cell(401, -3000.0, 0.7, 22_000, 0.3),
        ],
        15.0,
        7500.0,
    );
}

#[test]
fn cases_5_to_12_across_the_groups() {
    for g in (0..64).step_by(9) {
        let offset = 1000.0 * ((g % 3) as f64 - 1.0);
        check(vec![cell(8 * g + 3, offset, 1.0, 1000 + 500 * g as i64, 0.5)], 8.0, 5000.0);
    }
}

#[test]
fn case_13_noise_gives_no_cell() {
    let x = sdrkit_dev_test::Awgn::with_sigma(0.3, 7).noise((0.08 * NATIVE_RATE) as usize);
    let cap = Capture::new(x, NATIVE_RATE, 0.0, 0.08);
    assert!(UmtsSearcher::default().search(&cap, 7500.0, 6, true).is_empty());
}

fn broadcast_case(offset: f64, snr_db: f64, sttd: bool) {
    let mut c = cell(33, offset, 1.0, 1700, 0.3);
    c.broadcast = Some((plmn(246, 3), 0x0AB_CDEF));
    c.sttd = sttd;
    let found = check(vec![c], snr_db, 7500.0);
    let f = found.iter().find(|f| f.code == 33).unwrap();
    assert_eq!(f.mib, Some(plmn(246, 3)), "{f:?}");
    assert_eq!(f.cell_id, Some(0x0AB_CDEF), "{f:?}");
    assert!(f.good_blocks >= 2, "{f:?}");
    assert_eq!(f.sttd, Some(sttd), "{f:?}");
}

#[test]
fn cases_14_to_17_the_broadcast_channel() {
    for offset in [0.0, 2100.0] {
        for snr_db in [25.0, 5.0] {
            broadcast_case(offset, snr_db, false);
        }
    }
}

#[test]
fn case_14_with_the_sch_symbol_a_positive() {
    broadcast_case(0.0, 25.0, true);
}

#[test]
fn cases_18_to_22_sample_clock_errors() {
    for ppm in [-50.0, -15.0, -5.0, 5.0, 30.0] {
        let mut a = cell(17, -2000.0, 1.0, 500, 0.4);
        a.broadcast = Some((plmn(1, 1), 0x12_3456));
        let mut b = cell(280, 300.0, 0.7, 9000, 0.3);
        b.broadcast = Some((plmn(1, 1), 0x23_4567));
        let found = run(vec![a, b], 30.0, ppm, 7500.0, 0.08);
        assert!(found.iter().any(|f| f.code == 17) && found.iter().any(|f| f.code == 280), "{ppm}: {found:?}");
        let good: u32 = found.iter().map(|f| f.good_blocks).sum();
        assert!(good >= 3, "{ppm}: {found:?}");
    }
}

/// The test signal made at 7.68, 10.52 and 15.36 Msps (made at two samples a chip and resampled): both cells are found
/// through the stack at each rate.
#[test]
fn both_cells_at_three_rates() {
    for rate in [7.68e6, 10.52e6, 15.36e6] {
        let a = cell(17, 800.0, 1.0, 500, 0.4);
        let b = cell(280, 3000.0, 0.7, 9000, 0.3);
        let x = impaired(UmtsGenerator::new(rate, vec![a, b], 4, 0.22), &snr(12.0), 0.2);
        let mut rx = stack_graph(&UMTS, &[]);
        let input = InputInfo::new(rate, 2112.8e6, 2112.8e6).recording();
        let s = run_graph(&mut rx, input, &x, &[8192]);
        let codes: Vec<u64> = cells(&s).iter().filter_map(|c| c["code"].as_u64()).collect();
        assert!(codes.contains(&17) && codes.contains(&280), "{rate}: {codes:?}");
    }
}

#[test]
fn case_23_the_stack_end_to_end() {
    let mut a = cell(17, 800.0, 1.0, 500, 0.4);
    a.broadcast = Some((plmn(1, 1), 0x12_3456));
    let mut b = cell(280, 3000.0, 0.7, 9000, 0.3);
    b.broadcast = Some((plmn(1, 1), 0x23_4567));
    let x = impaired(UmtsGenerator::new(NATIVE_RATE, vec![a, b], 4, 0.22), &snr(12.0), 0.4);
    let mut rx = stack_graph(&UMTS, &[]);
    let input = InputInfo::new(NATIVE_RATE, 2112.8e6, 2112.8e6).recording();
    let s = run_graph(&mut rx, input, &x, &[8192]);
    let codes: Vec<u64> = cells(&s).iter().filter_map(|c| c["code"].as_u64()).collect();
    assert!(codes.contains(&17) && codes.contains(&280), "{codes:?}");
    let c17 = cells(&s).into_iter().find(|c| c["code"] == 17).unwrap();
    assert_eq!(c17["plmn"]["mcc"], 1);
    assert_eq!(c17["cell_id"], 0x12_3456);
    assert_eq!(c17["uarfcn"], 10_564);
    // SIB 1: the location area (upper 16 bits of the cell identity in the test signal) and the routing area; SIB 3: the
    // access restriction.
    assert_eq!((c17["lac"].as_u64(), c17["rac"].as_u64()), (Some(0x123), Some(0x56)), "{c17}");
    assert_eq!(c17["access"], serde_json::json!([false, false]));
    assert!((c17["centre_hz"].as_f64().unwrap() - 2112.8e6).abs() < 1.0);
    // The typed rows: the cells with their frequency and areas, and the survey.
    use sdrkit_core::Owner;
    use sdrkit_stack_cell::scan::Generation;
    use sdrkit_stack_cell::typed::{SurveyRow, UmtsCellRow, UMTS_CELLS, UMTS_SURVEY};
    let r = Owner::results(&rx);
    let rows: Vec<_> = UMTS_CELLS.rows::<UmtsCellRow>(r).map(|x| x.1).collect();
    let c = rows.iter().find(|c| c.code == 17).unwrap();
    assert!(c.frequency.is_some_and(|f| (f.0 - 2112.8008e6).abs() < 50.0), "{c:?}");
    assert_eq!((c.lac, c.rac, c.barred, c.reserved_operator), (Some(0x123), Some(0x56), Some(false), Some(false)));
    assert_eq!((c.generation, c.name), (Generation::Third, "Test network 3G PSC 17"));
    let survey: Vec<_> = UMTS_SURVEY.rows::<SurveyRow>(r).map(|x| x.1).collect();
    assert_eq!(survey.len(), 2);
    let s = survey.iter().find(|s| s.pci == 17).unwrap();
    assert_eq!((s.area_code, s.cell_identity, s.channel), (Some(0x123), Some(0x12_3456), Some(10_564)));
}

/// The SIB 3 of cell 0x0ABCDEF: barred, not reserved.
fn s3_0abcdef() -> Sib3 {
    Sib3 {
        cell_id: 0x0AB_CDEF,
        barred: true,
        ..Sib3::default()
    }
}
