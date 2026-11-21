//! HD Radio: generator, the impairments of the framework and the receiver, the interleavers of NRSC-5 and Layer 2 alone.

use super::support::*;
use sdrkit_core::publish::TableDef;
use sdrkit_core::{
    Complex32, Event, InputInfo, LockState, Output, Results, SettingSet, StackConfig, Value,
};
use serde_json::json;

/// The configuration of the HD Radio stack at its defaults.
fn defaults() -> StackConfig {
    stack_config(hd::HdSetting::schema(), &[])
}
use sdrkit_stack_broadcast::hd::l2::*;
use sdrkit_stack_broadcast::hd::results::{self as res, DATA_SERVICES, FILES, PROGRAMS};
use sdrkit_stack_broadcast::hd::{self, l1, GenOptions, HdGenerator, Waveform};

fn generate(o: GenOptions, rate: f64, seconds: f64, settings: &[Set]) -> Vec<Complex32> {
    let g = HdGenerator::new(o, rate).expect("a generator at this rate");
    impair(Box::new(g), seconds, settings)
}

fn with_seed(mut s: Vec<Set<'static>>) -> Vec<Set<'static>> {
    s.push(("gen_seed", json!(3)));
    s
}

fn receive(x: &[Complex32], rate: f64, dial: f64, piece: usize) -> (Results, Vec<Event>) {
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    rx.configure(&InputInfo::new(rate, dial, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(piece) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    (rx.results().clone(), ev)
}

fn prog(r: &Results, label: &str, key: &str) -> String {
    cell_text(r, &PROGRAMS, label, key)
}

fn prog_num(r: &Results, label: &str, key: &str) -> f64 {
    cell_num(r, &PROGRAMS, label, key)
}

fn table_has(r: &Results, def: &TableDef, key: &str, value: &str) -> bool {
    row_ids(r, def.key)
        .iter()
        .any(|id| cell_text(r, def, id, key) == value)
}

fn complete(r: &Results, band: &str, mode: &str, programs: usize, strict: bool) {
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{}",
        r.status().summary
    );
    assert_eq!(label(r, res::HdradioFields::BAND.key), band);
    assert_eq!(label(r, res::HdradioFields::SERVICE_MODE.key), mode);
    assert_eq!(text(r, res::HdradioFields::CALL_SIGN.key), "WTST");
    // A short name and no universal short name (1020s clause 5.2), and a long name equal to the slogan (1020s clause 4.2.2).
    assert_eq!(text(r, res::HdradioFields::UNIVERSAL_NAME.key), "");
    assert_eq!(
        text(r, res::HdradioFields::LONG_NAME.key),
        "Station data you can see"
    );
    assert_eq!(
        text(r, res::HdradioFields::SLOGAN.key),
        "Station data you can see"
    );
    assert_eq!(
        text(r, res::HdradioFields::MESSAGE.key),
        "This is a test signal. HD Radio sound uses the HDC codec."
    );
    assert_eq!(text(r, res::HdradioFields::COUNTRY.key), "US");
    assert_eq!(int(r, res::HdradioFields::FACILITY_ID.key), 54321);
    match r.fields().at(&res::HdradioFields::LOCATION) {
        Some((lat, lon)) => {
            assert!((lat - 40.75).abs() < 2e-4, "{lat}");
            assert!((lon + 73.99).abs() < 2e-4, "{lon}");
        }
        v => panic!("the location is {v:?}"),
    }
    assert_eq!(num(r, res::HdradioFields::ALTITUDE_M.key), 96.0);
    let rows = row_ids(r, PROGRAMS.key);
    assert_eq!(rows.len(), programs, "{rows:?}");
    assert_eq!(prog(r, "HD1", "name"), "Test HD");
    assert_eq!(prog_num(r, "HD1", "program_type"), 5.0);
    assert_eq!(prog(r, "HD1", "title"), "Test Pattern");
    assert_eq!(prog(r, "HD1", "artist"), "The Test Band");
    assert_eq!(prog(r, "HD1", "album"), "Signals Vol. 1");
    assert_eq!(prog(r, "HD1", "genre"), "Rock");
    assert_eq!(prog(r, "HD2", "name"), "Test HD2");
    assert_eq!(prog_num(r, "HD2", "program_type"), 14.0);
    assert_eq!(prog(r, "HD2", "title"), "Second Program");
    for id in &rows {
        for k in ["in_sis", "in_sig", "broadcasting"] {
            assert_eq!(cell(r, &PROGRAMS, id, k), Value::Bool(true), "{id} {k}");
        }
        assert!(prog_num(r, id, "packets_ok") > 0.0, "{id}");
        if strict {
            assert_eq!(prog_num(r, id, "packets_bad"), 0.0, "{id}");
        }
    }
    assert_eq!(prog_num(r, "HD1", "art_lot"), 1.0);
    assert_eq!(prog_num(r, "HD1", "art_port"), 4096.0);
    assert_eq!(cell(r, &FILES, "4096:1", "complete"), Value::Bool(true));
    assert_eq!(
        cell_num(r, &FILES, "4096:1", "size") as usize,
        hd::test_art().len()
    );
    assert!(table_has(r, &DATA_SERVICES, "name", "Test Traffic"));
}

#[test]
fn hd_r1_fm_mp1_with_offsets() {
    for cfo in [1000.0, -1000.0] {
        let o = GenOptions {
            waveform: Waveform::Mp1,
            analog_db: 0.0,
        };
        let mut s = snr(15.0).to_vec();
        s.push(("gen_offset_hz", json!(cfo)));
        let x = generate(o, l1::FM_RATE, 12.0, &with_seed(s));
        let (r, _) = receive(&x, l1::FM_RATE, 98.5e6, 100_000);
        complete(&r, "FM hybrid", "MP1", 2, true);
        let off = num(&r, "offset_hz");
        assert!((off - cfo).abs() < 10.0, "{off}");
        assert_eq!(int(&r, res::HdradioFields::P1_BAD.key), 0);
        assert!(flag(&r, res::HdradioFields::P1_ON.key));
    }
}

#[test]
fn hd_r3_fm_mp3() {
    let o = GenOptions {
        waveform: Waveform::Mp3,
        analog_db: 0.0,
    };
    let mut s = snr(15.0).to_vec();
    s.push(("gen_offset_hz", json!(600.0)));
    let x = generate(o, l1::FM_RATE, 12.0, &with_seed(s));
    let (r, _) = receive(&x, l1::FM_RATE, 98.5e6, 100_000);
    complete(&r, "FM hybrid", "MP3", 3, true);
    assert!(int(&r, res::HdradioFields::P3_OK.key) > 0);
    assert_eq!(int(&r, res::HdradioFields::P3_BAD.key), 0);
}

#[test]
fn hd_a1_am_modes_and_rates() {
    for (wf, name, band, rate) in [
        (Waveform::Ma1, "MA1", "AM hybrid", 200e3),
        (Waveform::Ma3, "MA3", "AM all-digital", 912e3),
    ] {
        let o = GenOptions {
            waveform: wf,
            analog_db: 0.0,
        };
        let x = generate(o, rate, 18.0, &with_seed(snr(15.0).to_vec()));
        let (r, _) = receive(&x, rate, 1.03e6, 65_536);
        complete(&r, band, name, 2, false);
        assert!(int(&r, res::HdradioFields::P1_OK.key) >= 10, "{name}");
        // The PCI is spread over the whole transfer frame (1014s clause 5.2.1): the frames decoded while the receiver starts
        // can lose it.
        assert!(int(&r, res::HdradioFields::P1_BAD.key) <= 2, "{name}");
    }
}

#[test]
fn hd_w_mirrored_and_faults() {
    // Swapped I/Q with +230 Hz, and samples that are not numbers.
    let o = GenOptions {
        waveform: Waveform::Ma1,
        analog_db: 0.0,
    };
    let mut s = snr(30.0).to_vec();
    s.push(("gen_offset_hz", json!(230.0)));
    let mut x = generate(o, 1e6, 16.0, &with_seed(s));
    for v in &mut x {
        *v = Complex32::new(v.im, v.re);
    }
    for v in &mut x[3_000_000..3_004_000] {
        *v = Complex32::new(f32::NAN, f32::INFINITY);
    }
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    rx.configure(&InputInfo::new(1e6, 1.03e6, 1.03e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(65_536) {
        rx.feed(c, &mut out);
        out.clear();
    }
    // The framework's guard replaces the bad samples and says so; the decode goes on.
    let r = rx.results();
    assert_eq!(int(r, "bad_samples"), 4000);
    assert_eq!(
        r.status().lock_state,
        LockState::Locked,
        "{}",
        r.status().summary
    );
    assert!(int(r, res::HdradioFields::P1_BAD.key) <= 3);
    // P1 and P3 each carry their own HDLC stream of fixed data: none is cut by the other. Only the burst of bad samples cuts
    // a few packets.
    assert!(int(r, res::HdradioFields::AAS_OK.key) > 100);
    assert!(
        int(r, res::HdradioFields::AAS_BAD.key) <= 4,
        "{}",
        int(r, res::HdradioFields::AAS_BAD.key)
    );
}

#[test]
fn rate_rule() {
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    assert!(rx.configure(&InputInfo::new(1e6, 98.5e6, 98.5e6)).is_err());
    assert!(rx.configure(&InputInfo::new(1e6, 1.03e6, 1.03e6)).is_ok());
    assert!(rx
        .configure(&InputInfo::new(l1::FM_RATE, 98.5e6, 98.5e6).recording())
        .is_ok());
}

#[test]
fn unknown_settings_are_refused() {
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    assert!(apply(rx.as_mut(), "no_such_key", json!(true)).is_err());
    assert!(invoke(rx.as_mut(), "service.select", &[("row", json!("HD9"))]).is_err());
    assert!(invoke(rx.as_mut(), "service.select", &[("row", json!("HD2"))]).is_ok());
}

// ---------------------------------------------------------------------------------------------------------------------------
// The interleavers of NRSC-5 (documents 1011s and 1012s)

#[test]
fn the_fm_pm_interleaver_starts_where_the_standard_starts() {
    // Coded P1 bit 0 goes to partition v[0] = 10 of block (0 + 7 x 10) mod 16 = 6, row 0, column 0: 6 x 23 040 + 10 x 36.
    assert_eq!(l1::pm_p1_position(0), 138_600);
}

#[test]
fn every_pm_position_is_used_once() {
    let mut seen = vec![0u8; 16 * l1::FM_BLOCK_BITS];
    for i in 0..365_440 {
        seen[l1::pm_p1_position(i)] += 1;
    }
    for b in 0..16 {
        for n in 0..200 {
            seen[b * l1::FM_BLOCK_BITS + l1::pm_pids_position(b, n)] += 1;
        }
    }
    assert!(seen.iter().all(|&c| c == 1));
}

#[test]
fn the_am_interleaver_fills_each_matrix_once() {
    for mode in [l1::ServiceMode::Ma1, l1::ServiceMode::Ma3] {
        let (p1, p3, pids) = l1::AmFrames::new(mode).usage();
        assert!(p1.iter().all(|&c| c == 1), "{mode:?} P1");
        assert!(p3.iter().all(|&c| c == 1), "{mode:?} P3");
        assert!(pids.iter().all(|&c| c == 1), "{mode:?} PIDS");
    }
}

#[test]
fn the_am_matrices_follow_figures_10_2_and_10_3() {
    // Figure 10-2: row 1, column 0 holds k = 150. Row 0, column 1 and row 3, column 10 hold training (k of 750 and up).
    let at = |row: usize, col: usize| (0..800).find(|&k| l1::am_row_col(k) == (row, col));
    assert_eq!(at(1, 0), Some(150));
    assert!(at(0, 1).is_some_and(|k| k >= 750));
    assert!(at(3, 10).is_some_and(|k| k >= 750));
    // Figure 10-3: k = 0 is in row 3, k = 22 in row 0, and rows 8 and 24 hold the training.
    assert_eq!(l1::am_pids_row(0), 3);
    assert_eq!(l1::am_pids_row(22), 0);
    let mut rows: Vec<usize> = (0..30).map(l1::am_pids_row).collect();
    rows.extend(l1::AM_PIDS_TRAINING_ROWS);
    rows.sort_unstable();
    assert_eq!(rows, (0..32).collect::<Vec<_>>());
    assert_eq!(l1::AM_PIDS_TRAINING_ROWS, [8, 24]);
}

#[test]
fn the_px1_interleaver_is_a_permutation() {
    let px = l1::Px1::new();
    let mut seen = vec![false; l1::Px1::N];
    for i in 0..l1::Px1::N {
        let p = px.write_position(i);
        assert!(p < l1::Px1::N && !seen[p], "{i} -> {p}");
        seen[p] = true;
    }
}

#[test]
fn a_short_transfer_frame_is_refused_not_a_panic() {
    for mut ch in [
        l1::Channel::fm_p1(),
        l1::Channel::fm_pids(),
        l1::Channel::am_p1(),
    ] {
        let mut out = Vec::new();
        assert_eq!(ch.decode(&[0.5; 7], &mut out), None);
        assert_eq!(ch.decode(&[], &mut out), None);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The FM system control data sequence (NRSC-5 1011s clause 11)

/// A 32-bit sequence written field by field, bit 31 first.
fn fields(f: &str) -> u32 {
    f.chars()
        .filter(|c| !c.is_whitespace())
        .fold(0, |a, c| a << 1 | (c == '1') as u32)
}

#[test]
fn the_fm_control_sequence_follows_table_11_1() {
    // Block 5 of MP1 on column 0 (RSID 10): sync 0110010, SCI 0, parity 0, sync 1, RSID 10, ASM1 0, parity 1, sync 0,
    // reserved 0, BC 0101, parity 0, sync 11, ASM0 1, reserved 0, PSMI 000001, parity 0.
    let c = l1::FmControl::standard(5, 1);
    assert_eq!(
        c.sequence(0),
        fields("0110010 0 0 1 10 0 1 0 0 0101 0 11 1 0 000001 0")
    );
    assert_eq!(c.sequence(0), 0x6464_5702);
    // Every sync bit of table 11-1 is in place on every column, and the parse returns the fields.
    for col in 0..61 {
        let r = c.sequence(col);
        for (bit, v) in l1::SYNC_BITS {
            assert_eq!((r >> bit) & 1, v as u32, "column {col} bit {bit}");
        }
        assert_eq!((r >> 20) & 3, l1::FM_RSID[col] as u32);
        assert_eq!(l1::FmControl::parse(r), Some(c));
    }
    // Table 11-4: BC3:0 is the block count, most significant bit first in bit 15.
    for bc in 0..16u8 {
        assert_eq!(
            (l1::FmControl::standard(bc, 3).sequence(7) >> 12) & 15,
            bc as u32
        );
    }
    // A wrong parity bit or sync bit is refused.
    for bit in [0, 11, 23, 25, 9] {
        assert_eq!(
            l1::FmControl::parse(c.sequence(0) ^ 1 << bit),
            None,
            "bit {bit}"
        );
    }
}

#[test]
fn the_fm_differential_encoder_follows_clause_11_3() {
    // The output reverses each time the input bit is 1, from a state of 0: the sync 0110010 becomes 0100011.
    let big_r = l1::differential(0x6464_5702);
    assert_eq!(big_r, 0x47B8_65FC);
    assert_eq!(big_r >> 25, 0b0100011);
    // The parity bits make every sequence end in state 0, and R ^ (R >> 1) recovers r.
    for (bc, psmi) in [(0, 1), (7, 3), (15, 63), (3, 0)] {
        for col in [0, 30, 60] {
            let r = l1::FmControl::standard(bc, psmi).sequence(col);
            let big_r = l1::differential(r);
            assert_eq!(big_r & 1, 0);
            assert_eq!(big_r ^ (big_r >> 1), r);
        }
    }
    // The rows known to a receiver: the sync rows and the rows after each parity bit, whatever the station sets.
    let rows: Vec<u32> = (0..32)
        .filter(|r| (l1::fm_fixed_rows() >> (31 - r)) & 1 == 1)
        .collect();
    assert_eq!(
        rows,
        [0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 13, 14, 20, 21, 22, 31]
    );
    // Table 12-18: 0 is -1 - j, 1 is 1 + j, at the level of a QPSK cell.
    let a = std::f32::consts::FRAC_1_SQRT_2;
    assert_eq!(l1::fm_pilot_cell(0), Complex32::new(-a, -a));
    assert_eq!(l1::fm_pilot_cell(1), Complex32::new(a, a));
}

#[test]
fn the_fm_pilot_tables_follow_tables_11_3_and_12_24() {
    // Table 11-3, printed rows.
    for (col, rsid) in [
        (0, 0b10),
        (1, 0b01),
        (2, 0b00),
        (3, 0b11),
        (30, 0b00),
        (31, 0b01),
        (33, 0b11),
        (60, 0b10),
    ] {
        assert_eq!(l1::FM_RSID[col], rsid, "column {col}");
    }
    // The identification is the same on the columns 30 - n and 30 + n.
    for n in 0..=30 {
        assert_eq!(l1::FM_RSID[30 - n], l1::FM_RSID[30 + n], "{n}");
    }
    // Table 12-24: -546 is column 0, -356 column 10, 280 column 46, 546 column 60; a step of 19 subcarriers is one column.
    assert_eq!(l1::fm_pilot_column(-546), Some(0));
    assert_eq!(l1::fm_pilot_column(-356), Some(10));
    assert_eq!(l1::fm_pilot_column(280), Some(46));
    assert_eq!(l1::fm_pilot_column(546), Some(60));
    assert_eq!(l1::fm_pilot_column(0), None);
    for w in l1::FM_PILOT_COLUMNS
        .windows(2)
        .filter(|w| w[0].0 * w[1].0 > 0)
    {
        assert_eq!((w[1].0 - w[0].0, w[1].1 - w[0].1), (19, 1));
    }
    // The band plan's pilots are the MP3 set: ±318 and out (26 subcarriers).
    let mut refs = l1::plan(l1::Band::Fm).refs;
    refs.sort_unstable();
    let mp3: Vec<i32> = l1::FM_PILOT_COLUMNS
        .iter()
        .map(|p| p.0)
        .filter(|k| k.abs() >= 318)
        .collect();
    assert_eq!(refs, mp3);
}

#[test]
fn the_fm_service_mode_follows_tables_6_2_and_6_5() {
    use l1::ServiceMode as M;
    assert_eq!(M::Mp1.psmi(), Some(1));
    assert_eq!(M::Mp3.psmi(), Some(3));
    // Table 6-5: MP4 decodes as MP1, MP11 and MP19 as MP3, MP27 as MP11 (so MP3); MP7 is MP5, which sdrkit does not decode.
    assert_eq!(M::from_psmi(0b000100), Some(M::Mp1));
    assert_eq!(M::from_psmi(0b001011), Some(M::Mp3));
    assert_eq!(M::from_psmi(0b010011), Some(M::Mp3));
    assert_eq!(M::from_psmi(0b011011), Some(M::Mp3));
    assert_eq!(M::from_psmi(0b000111), None);
    assert_eq!(M::from_psmi(0), None);
    assert_eq!(l1::FM_PSMI_DEFAULT[63], 5);
    assert!(l1::FM_PSMI_DEFAULT
        .iter()
        .all(|m| [0, 1, 2, 3, 5, 6, 11].contains(m)));
}

// ---------------------------------------------------------------------------------------------------------------------------
// The AM subcarriers, constellations, levels and system control data sequence (NRSC-5 1012s clauses 11 and 12, 1082s)

fn close(a: Complex32, b: Complex32) -> bool {
    (a - b).norm() < 1e-4
}

#[test]
fn the_am_control_sequence_follows_table_11_1() {
    use l1::{AmControl, ServiceMode as M};
    // MA1, block 5: sync 0110010, PLI 0, parity 0, sync 1, reserved 0, HPPI 0, AABI 0, parity 0, sync 0, RDBI 0, reserved 0,
    // BC 101, parity 0, sync 11, reserved 000, SMI 00001, parity 1.
    let c = AmControl::standard(5, M::Ma1);
    assert_eq!(
        c.sequence(),
        fields("0110010 0 0 1 0 0 0 0 0 0 0 101 0 11 000 00001 1")
    );
    assert_eq!(c.sequence(), 0x6440_5603);
    assert_eq!(AmControl::parse(c.sequence()), Some(c));
    // MA3, block 7, high-power PIDS: HPPI 1 sets parity 2, BC 111 sets parity 1, SMI 00010 (table 11-3).
    let c = AmControl {
        hppi: true,
        ..AmControl::standard(7, M::Ma3)
    };
    assert_eq!(c.sequence(), 0x6454_7E05);
    assert_eq!(AmControl::parse(c.sequence()), Some(c));
    for bit in [0, 9, 11, 18, 22, 23, 31] {
        assert_eq!(AmControl::parse(c.sequence() ^ 1 << bit), None, "bit {bit}");
    }
    // Table 11-3, and the known rows are the sync rows (the sequence is not differential).
    assert_eq!(
        (M::Ma1.smi(), M::Ma3.smi(), M::Mp1.smi()),
        (Some(1), Some(2), None)
    );
    assert_eq!(M::from_smi(2), Some(M::Ma3));
    assert_eq!(M::from_smi(3), None);
    let sync = l1::SYNC_BITS.iter().fold(0u32, |m, &(b, _)| m | 1 << b);
    assert_eq!(l1::AM_FIXED_ROWS, sync);
    // Table 12-10: 0 is -j, 1 is +j (at unit level before the scale factor).
    assert_eq!(l1::am_pilot_cell(0), Complex32::new(0.0, -1.0));
    assert_eq!(l1::am_pilot_cell(1), Complex32::new(0.0, 1.0));
}

#[test]
fn the_am_constellations_follow_tables_12_1_12_4_and_12_5() {
    use sdrkit_stack_broadcast::qam::Qam;
    // Bit position p of an element is xp: the word's bits go to the map x0 first.
    let word = |w: u32, n: usize| -> Vec<u8> { (0..n).map(|p| ((w >> p) & 1) as u8).collect() };
    // Table 12-1 (64-QAM, mean energy 10.5).
    let s64 = 10.5f32.sqrt();
    for (w, re, im) in [
        (0x00, -3.5, -3.5),
        (0x01, 3.5, -3.5),
        (0x02, -0.5, -3.5),
        (0x08, -3.5, 3.5),
        (0x13, 0.5, -0.5),
        (0x1B, 0.5, 0.5),
        (0x25, 2.5, -2.5),
        (0x2D, 2.5, 2.5),
        (0x36, -1.5, -1.5),
        (0x3F, 1.5, 1.5),
    ] {
        assert!(
            close(Qam::Q64.map(&word(w, 6)) * s64, Complex32::new(re, im)),
            "{w:02X}"
        );
    }
    // Table 12-5 (16-QAM, mean energy 2.5).
    let s16 = 2.5f32.sqrt();
    for (w, re, im) in [
        (0x0, -1.5, -1.5),
        (0x1, 1.5, -1.5),
        (0x2, -0.5, -1.5),
        (0x4, -1.5, 1.5),
        (0x8, -1.5, -0.5),
        (0x9, 1.5, -0.5),
        (0xA, -0.5, -0.5),
        (0xF, 0.5, 0.5),
    ] {
        assert!(
            close(Qam::Q16.map(&word(w, 4)) * s16, Complex32::new(re, im)),
            "{w:X}"
        );
    }
    // Table 12-4 (QPSK, mean energy 0.5).
    let s4 = 0.5f32.sqrt();
    for (w, re, im) in [
        (0, -0.5, -0.5),
        (1, 0.5, -0.5),
        (2, -0.5, 0.5),
        (3, 0.5, 0.5),
    ] {
        assert!(
            close(Qam::Q4.map(&word(w, 2)) * s4, Complex32::new(re, im)),
            "{w}"
        );
    }
    // Table 10-5: the training words 100101, 1001 and 10 (x5 first) are the points 0x25, 0x9 and 0x2.
    assert_eq!(l1::am_training(6), word(0b100101, 6));
    assert_eq!(l1::am_training(4), word(0b1001, 4));
    assert_eq!(l1::am_training(2), word(0b10, 2));
    assert!(close(
        l1::am_training_cell(Qam::Q64) * s64,
        Complex32::new(2.5, -2.5)
    ));
    assert!(close(
        l1::am_training_cell(Qam::Q16) * s16,
        Complex32::new(1.5, -0.5)
    ));
    assert!(close(
        l1::am_training_cell(Qam::Q4) * s4,
        Complex32::new(-0.5, 0.5)
    ));
}

#[test]
fn the_am_subcarriers_follow_tables_12_2_to_12_12() {
    use l1::{Place, ServiceMode as M};
    let p = |k: i32, conj: bool, twin: bool| Place { k, conj, twin };
    let a = l1::am_layout(M::Ma1);
    assert_eq!(
        (a.pu[0], a.pu[24]),
        (p(57, false, false), p(81, false, false))
    );
    assert_eq!(
        (a.pl[0], a.pl[24]),
        (p(-57, true, false), p(-81, true, false))
    );
    assert_eq!((a.t[0], a.t[24]), (p(2, false, true), p(26, false, true)));
    assert_eq!((a.s[0], a.s[24]), (p(28, false, true), p(52, false, true)));
    assert_eq!(a.pids, [p(27, false, true), p(53, false, true)]);
    let a = l1::am_layout(M::Ma3);
    assert_eq!(
        (a.pu[0], a.pl[24]),
        (p(2, false, false), p(-26, true, false))
    );
    assert_eq!(
        (a.s[0], a.t[24]),
        (p(28, false, false), p(-52, true, false))
    );
    assert_eq!(a.pids, [p(-27, true, false), p(27, false, false)]);
    // Every subcarrier of -81 to 81 is used once or is unused: MA1 leaves 0 and ±54 to ±56, MA3 also ±53 and out.
    for (mode, unused) in [(M::Ma1, 7), (M::Ma3, 7 + 2 * 26)] {
        let a = l1::am_layout(mode);
        let mut seen = std::collections::BTreeMap::new();
        for q in
            a.pu.iter()
                .chain(&a.pl)
                .chain(&a.s)
                .chain(&a.t)
                .chain(&a.pids)
        {
            *seen.entry(q.k).or_insert(0) += 1;
            if q.twin {
                *seen.entry(-q.k).or_insert(0) += 1;
            }
        }
        for k in l1::AM_PILOTS {
            *seen.entry(k).or_insert(0) += 1;
        }
        assert!(seen.values().all(|&n| n == 1), "{mode:?}");
        assert_eq!(163 - seen.len(), unused, "{mode:?}");
        assert!(
            l1::AM_UNUSED.iter().all(|k| !seen.contains_key(k)),
            "{mode:?}"
        );
        // The plan's unused subcarriers are the same.
        assert_eq!(l1::mode_plan(mode).layout.null.len(), unused, "{mode:?}");
    }
}

#[test]
fn a_complementary_pair_cancels_the_analogue_programme() {
    // MA1 T column 4 on subcarriers 6 and -6: an analogue component a on 6 comes with conj(a) on -6; the pair reads back the
    // value, the analogue cancelled.
    let cells = l1::cells(l1::ServiceMode::Ma1, false, false);
    let width = 163;
    let mut grid = vec![Complex32::new(0.0, 0.0); 32 * width];
    let c = cells.p3[1 + 2 * 4];
    let v = Complex32::new(0.3, -0.7);
    c.put(v, &mut grid);
    let a = Complex32::new(5.0, 2.0);
    grid[6 + 81] += a;
    grid[81 - 6] += a.conj();
    let w = vec![1.0; grid.len()];
    assert!(close(c.value(&grid, &w).0, v));
    // On a mirrored spectrum, the mirrored plan reads every cell negated (which the channel estimate absorbs).
    for c in cells
        .p1
        .iter()
        .chain(&cells.p3)
        .chain(&cells.pids)
        .take(400)
    {
        let mut g = vec![Complex32::new(0.0, 0.0); 32 * width];
        c.put(v, &mut g);
        let mirrored: Vec<Complex32> = (0..g.len())
            .map(|i| {
                let (s, k) = (i / width, i % width);
                g[s * width + (width - 1 - k)].conj()
            })
            .collect();
        let m = cells.mirrored(width, -81);
        let mc = m.p1.iter().chain(&m.p3).chain(&m.pids).find(|x| {
            x.at / width == c.at / width && {
                let k = (x.at % width) as i32 - 81;
                k == -((c.at % width) as i32 - 81)
            }
        });
        assert!(close(mc.unwrap().value(&mirrored, &w).0, -v));
    }
}

#[test]
fn the_am_levels_follow_1082s_table_4_6() {
    use l1::ServiceMode as M;
    let lv = l1::am_levels(M::Ma1, false, false);
    assert_eq!((lv.primary, lv.secondary, lv.pilot), (-30.0, -43.0, -26.0));
    assert_eq!(
        (
            lv.tertiary[0],
            lv.tertiary[11],
            lv.tertiary[12],
            lv.tertiary[24]
        ),
        (-44.0, -49.5, -50.0, -50.0)
    );
    assert_eq!(lv.pids, [-43.0, -43.0]);
    // PL 1: CHS2, CHT2, CHI2 and CHI4 (1012s table 6-4).
    let lv = l1::am_levels(M::Ma1, true, false);
    assert_eq!(
        (lv.secondary, lv.tertiary, lv.pids),
        (-37.0, [-44.0; 25], [-37.0, -37.0])
    );
    // HPP 1: PIDS2 at the primary level (CHI5).
    assert_eq!(l1::am_levels(M::Ma1, false, true).pids, [-43.0, -30.0]);
    let lv = l1::am_levels(M::Ma3, false, false);
    assert_eq!(
        (lv.primary, lv.secondary, lv.tertiary[7], lv.pids, lv.pilot),
        (-15.0, -30.0, -30.0, [-30.0, -30.0], -15.0)
    );
    assert_eq!(l1::am_levels(M::Ma3, false, true).pids, [-15.0, -15.0]);
    // The MA1 pilot is 4 dB above a primary subcarrier.
    let lv = l1::am_levels(M::Ma1, false, false);
    assert!((lv.gain(lv.pilot) - 10f32.powf(0.2)).abs() < 1e-6);
}

// ---------------------------------------------------------------------------------------------------------------------------
// Layer 2 and above (NRSC-5 documents 1014s, 1017s, 1019s, 1020s, 1028s and 1085s)

#[test]
fn the_header_spread_follows_table_5_7() {
    // L < 72 000: Nstart 120, Noffset 8 INT(CEIL((L - 120) / 8) / H) - 1. L >= 72 000: Nstart 8 CEIL((L - 30 000) / 8).
    for (l, start, offset, len) in [
        (146_176, 116_176, 1247, 24), // FM P1
        (4608, 120, 183, 24),         // FM P3
        (3750, 120, 159, 22),         // AM P1: 3750 MOD 8 = 6
        (24_000, 120, 991, 24),       // MA1 P3
        (30_000, 120, 1239, 24),      // MA3 P3
        (72_007, 42_008, 1303, 23),
        (72_001, 42_008, 1359, 22),
        (3751, 120, 8 * (454 / 23) - 1, 23),
    ] {
        let s = header_spread(l).unwrap();
        assert_eq!((s.start, s.offset, s.len), (start, offset, len), "L = {l}");
        assert!(s.position(len - 1) < l, "L = {l}");
        // Header bits and whole payload bytes fit in the frame.
        assert!(payload_bytes(l) * 8 + len <= l);
    }
    assert_eq!(header_spread(100), None);
    // The header bits sit at Nstart + i (Noffset + 1), and the payload comes back whole.
    for l in [146_176, 4608, 3750, 24_000] {
        let s = header_spread(l).unwrap();
        let payload: Vec<u8> = (0..payload_bytes(l)).map(|i| (i * 31 + 7) as u8).collect();
        for cw in pci::ALL {
            let mut bits = Vec::new();
            spread(cw, &payload, l, &mut bits);
            assert_eq!(bits.len(), l);
            for i in 0..s.len {
                assert_eq!(bits[s.position(i)] as u32, (cw >> (23 - i)) & 1);
            }
            let d = despread(&bits).unwrap();
            assert_eq!(d.payload, payload);
            assert_eq!(match_pci(d.word, d.mask), Some(cw), "L = {l}");
        }
        // Payload bit 0 is bit 0 (the least significant bit) of byte 0 (1014s clause 5.3).
        let mut bits = Vec::new();
        spread(pci::AUDIO, &[0x01], l, &mut bits);
        assert_eq!(&bits[..8], &[1, 0, 0, 0, 0, 0, 0, 0]);
    }
}

#[test]
fn the_codec_modes_follow_table_5_2() {
    // Rows of 1017s table 5-2: (mode, stream, PDUs a frame, N, last sequence number, Lc, kbit/s).
    for (m, s, pdus, n, seq, lc, kbps) in [
        (0b0000, 0, 1, 32, 1, 16, 96),
        (0b0001, 0, 8, 4, 7, 12, 48),
        (0b0001, 1, 1, 32, 1, 16, 48),
        (0b0010, 0, 8, 4, 7, 12, 20),
        (0b0010, 1, 1, 32, 1, 16, 16),
        (0b0011, 1, 1, 32, 1, 16, 72),
        (0b1010, 0, 1, 32, 1, 12, 22),
        (0b1010, 1, 8, 4, 7, 12, 24),
        (0b1101, 0, 8, 4, 7, 12, 24),
    ] {
        let c = codec_stream(m, s).unwrap();
        assert_eq!(
            (c.pdus_per_frame, c.packets, c.seq_max, c.lc, c.max_kbps),
            (pdus, n, seq, lc, kbps),
            "{m:04b} {s}"
        );
        // A long PDU (one a frame) carries 32 packets, a short one 4: 32 packets a frame either way.
        assert_eq!(c.pdus_per_frame as u32 * c.packets as u32, 32);
    }
    for reserved in [0b0100, 0b1001, 0b1011, 0b1110, 0b1111] {
        assert_eq!(codec_stream(reserved, 0), None);
    }
}

/// Header bit `h` (47 to 0) of a PDU control word, as 1017s figure 5-2 numbers them: byte 0 holds bits 47 (MSB) to 40 (LSB).
fn header_bit(cw: &[u8; 6], h: usize) -> u8 {
    let k = (47 - h) / 8;
    (cw[k] >> (h - (40 - 8 * k))) & 1
}

#[test]
fn the_pdu_control_word_follows_figure_5_2() {
    let p = Pdu {
        codec_mode: 0b1010,
        stream: 0b01,
        seq: 0b101,
        blend: 0b10,
        gain: 0b11001,
        common_delay: 0b100110,
        latency: 0b110,
        pfirst: true,
        plast: false,
        start_seq: 0b100001,
        ..Default::default()
    };
    let cw = control_word(&p, 0b010111, true, 0xA5);
    let field = |bits: &[usize]| {
        bits.iter()
            .fold(0u32, |a, &h| a << 1 | header_bit(&cw, h) as u32)
    };
    // Each field, most significant bit first, at the bit numbers of figure 5-2.
    assert_eq!(field(&[43, 42, 41, 40]), 0b1010, "codec mode");
    assert_eq!(field(&[45, 44]), 0b01, "stream id");
    assert_eq!(field(&[32, 47, 46]), 0b101, "PDU sequence");
    assert_eq!(field(&[34, 33]), 0b10, "blend control");
    assert_eq!(field(&[39, 38, 37, 36, 35]), 0b11001, "gain");
    assert_eq!(field(&[29, 28, 27, 26, 25, 24]), 0b100110, "common delay");
    assert_eq!(field(&[16, 31, 30]), 0b110, "latency");
    assert_eq!(field(&[17]), 1, "Pfirst");
    assert_eq!(field(&[18]), 0, "Plast");
    assert_eq!(
        field(&[8, 23, 22, 21, 20, 19]),
        0b100001,
        "starting sequence"
    );
    assert_eq!(field(&[14, 13, 12, 11, 10, 9]), 0b010111, "NOP");
    assert_eq!(field(&[15]), 1, "HEF");
    assert_eq!(field(&[7, 6, 5, 4, 3, 2, 1, 0]), 0xA5, "La");
}

fn test_pdu(codec_mode: u8, sizes: &[usize]) -> Pdu {
    Pdu {
        codec_mode,
        program: 1,
        program_type: 14,
        seq: 3,
        psd: b"psd bytes".to_vec(),
        packets: sizes.iter().map(|&n| vec![n as u8; n]).collect(),
        ..Default::default()
    }
}

#[test]
fn the_locators_follow_figures_5_4_and_5_5() {
    // 12-bit locators (codec mode 0b0010): b0 7:0, then b1 3:0 above b0 11:8, then b1 11:4, ... and a reserved nibble when NOP
    // is odd.
    let p = test_pdu(0b0010, &[100, 120, 90]);
    let b = p.to_bytes().unwrap();
    let la = 14 + 5 + 3 + 9 - 1; // locators, two expansions (3 bytes), PSD
    let ends = [la + 101, la + 101 + 121, la + 101 + 121 + 91];
    assert_eq!(b[14], ends[0] as u8);
    assert_eq!(b[15], ((ends[1] & 15) << 4 | ends[0] >> 8) as u8);
    assert_eq!(b[16], (ends[1] >> 4) as u8);
    assert_eq!(b[17], ends[2] as u8);
    assert_eq!(b[18], (ends[2] >> 8) as u8);
    // Each locator points to the CRC byte of its packet, and the last one ends the PDU.
    assert_eq!(b.len(), ends[2] + 1);
    // 16-bit locators (codec mode 0b0000): low byte first.
    let p = test_pdu(0b0000, &[100, 120]);
    let b = p.to_bytes().unwrap();
    let la = 14 + 4 + 3 + 9 - 1;
    assert_eq!(u16::from_le_bytes([b[14], b[15]]) as usize, la + 101);
    assert_eq!(u16::from_le_bytes([b[16], b[17]]) as usize, la + 101 + 121);
    // The expansions (figures 5-8 and 5-10): HEF 1, ID 001, program number, 0. Then HEF 1, ID 010, access, 00, d7.
    // Then HEF 0, d6..d0.
    assert_eq!(&b[18..21], &[0x92, 0xA0, 0x0E]);
    assert_eq!(expansion_bytes(3, 200, true), [0x96, 0xA9, 0x48]);
}

#[test]
fn the_header_code_follows_clause_5_2_3_1() {
    use sdrkit_algo_fec::rs::{codes, Rs, RsCode, SymbolOrder};
    let rs = header_rs();
    // g(x) = a^36 + a^203 x + a^3 x^2 + a^220 x^3 + a^253 x^4 + a^211 x^5 + a^240 x^6 + a^176 x^7 + x^8.
    let g = rs.generator();
    for (i, &e) in HEADER_GENERATOR_POWERS.iter().enumerate() {
        assert_eq!(g[i], rs.field().exp(e as usize), "g{i}");
    }
    assert_eq!(g[8], 1);
    assert_eq!(rs.field().poly(), 0b1_0001_1101);
    // The codeword read from PDU byte 95 down to byte 0 is a systematic codeword with its parity last: the last parity byte is
    // PDU byte 0 and PDU byte 8 is the first byte of the control word.
    let b = test_pdu(0b0010, &[100, 120, 90]).to_bytes().unwrap();
    let mut word: Vec<u8> = b[..HEADER_LEN].iter().rev().copied().collect();
    let high = Rs::new(RsCode {
        order: SymbolOrder::HighFirst,
        ..codes::HD_RADIO_96_88
    })
    .unwrap();
    let sent = word.clone();
    high.encode_word(&mut word).unwrap();
    assert_eq!(word, sent);
}

#[test]
fn pdu_round_trip_and_rs_protection() {
    let p = test_pdu(0b0010, &[100, 120, 90]);
    let mut b = p.to_bytes().unwrap();
    // Four byte errors inside the 96-byte codeword are corrected: header, PSD and the start of packet 0.
    for i in [3, 17, 40, 90] {
        b[i] ^= 0x55;
    }
    let mut rs = header_rs();
    let (r, n) = parse_pdu(&b, &mut rs).unwrap();
    assert_eq!(n, b.len());
    assert_eq!(
        (r.header.program, r.header.program_type, r.header.seq),
        (1, 14, 3)
    );
    assert_eq!(r.header.psd, b"psd bytes");
    assert_eq!(r.packets.iter().filter(|x| x.is_some()).count(), 3);
    assert_eq!(r.packets[1].as_deref(), Some(&[120u8; 120][..]));
    let mut bad = p.to_bytes().unwrap();
    bad[HEADER_LEN + 30] ^= 1;
    let (r, _) = parse_pdu(&bad, &mut rs).unwrap();
    assert!(r.packets[0].is_none() && r.packets[1].is_some());
    // A PSD field that ends after byte 255 is refused (La is 8 bits).
    let mut long = test_pdu(0b0000, &[100]);
    long.psd = vec![1; 300];
    assert_eq!(long.to_bytes(), None);
}

#[test]
fn the_pci_words_follow_table_5_6() {
    // 1014s table 5-6: CW0 001110001101100011010011 and its cyclic permutations.
    let cw0 = 0b0011_1000_1101_1000_1101_0011;
    assert_eq!(pci::AUDIO, cw0);
    let rot = |x: u32, r: u32| ((x << r) | (x >> (24 - r))) & 0xFF_FFFF;
    let mut all = pci::ALL.to_vec();
    all.extend(pci::RESERVED);
    assert_eq!(
        all,
        [0x38D8D3, 0xCE3634, 0xE3634C, 0x8D8D33, 0x3634CE, 0x8D338D, 0xD8D338, 0x634CE3]
    );
    for w in &all {
        assert!((0..24).any(|r| rot(cw0, r) == *w), "{w:06X}");
    }
    // Two words in use differ in more than 8 bits, so 4 errors never match the wrong one.
    for (i, a) in pci::ALL.iter().enumerate() {
        for b in &pci::ALL[i + 1..] {
            assert!((a ^ b).count_ones() > 8, "{a:06X} {b:06X}");
        }
    }
    // The content columns of table 5-6.
    assert!(pci::has_audio(pci::AUDIO) && !pci::has_fixed(pci::AUDIO));
    assert!(pci::has_audio(pci::AUDIO_FIXED) && pci::has_fixed(pci::AUDIO_FIXED));
    assert!(!pci::has_audio(pci::FIXED) && pci::has_fixed(pci::FIXED));
}

#[test]
fn pci_matching() {
    assert_eq!(match_pci(pci::AUDIO ^ 0b1011, 0xFF_FFFF), Some(pci::AUDIO));
    assert_eq!(match_pci(pci::AUDIO ^ 0b11111, 0xFF_FFFF), None);
    // Missing low bits count as zero.
    assert_eq!(
        match_pci(pci::FIXED & 0xFF_FF00, 0xFF_FF00),
        Some(pci::FIXED)
    );
}

/// The encapsulated packet of 1019s figure 6-3 (also 1085s figure 5-1): an ID3 tag on port 0x5100, sequence 0.
const FIGURE_6_3: &str = "7e 21 00 51 00 00 49 44 33 03 00 00 00 00 00 4a 54 49 54 32 00 00 00 0d 00 00 00 41 6e 61 6c \
    6f 67 20 42 6c 75 65 73 54 50 45 31 00 00 00 0d 00 00 00 4a 2e 20 51 2e 20 50 75 62 6c 69 63 54 \
    41 4c 42 00 00 00 12 00 00 00 54 68 65 20 4c 6f 73 74 20 53 65 73 73 69 6f 6e 73 f5 27 7e";

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|h| u8::from_str_radix(h, 16).unwrap())
        .collect()
}

#[test]
fn the_encapsulated_packet_follows_figure_6_3() {
    let pdu = hex(FIGURE_6_3);
    let tag = &pdu[6..pdu.len() - 3];
    let psd = Psd::from_id3(tag).unwrap();
    assert_eq!(psd.title, "Analog Blues");
    assert_eq!(psd.artist, "J. Q. Public");
    assert_eq!(psd.album, "The Lost Sessions");
    // The ID3v2.3.0 tag of the title, artist and album, and the HDLC frame around it, byte for byte.
    assert_eq!(psd.to_id3(), tag);
    assert_eq!(hdlc_frame(psd_port(0), 0, tag), pdu);
    let mut h = AasReceiver::default();
    let mut out = Vec::new();
    h.push(&pdu, &mut out);
    assert_eq!((out[0].port, out[0].seq), (0x5100, 0));
    assert_eq!(out[0].payload, tag);
    // Table 5-2 of 1019s: SPSD ports 0x5201 to 0x5207.
    assert_eq!((psd_port(1), psd_port(7)), (0x5201, 0x5207));
}

#[test]
fn psd_round_trip() {
    let p = Psd {
        title: "Title A".into(),
        artist: "Artist A".into(),
        album: "Album A".into(),
        genre: "Rock".into(),
        comment: "c".into(),
        ufid: "owner\0id".into(),
        commercial: Some((
            "USD1.99".into(),
            "Seller".into(),
            "Desc".into(),
            "http://x".into(),
            "20261231".into(),
        )),
        xhdr: Some((7, 0xBE4B_7536)),
    };
    assert_eq!(Psd::from_id3(&p.to_id3()), Some(p));
    // Empty text frames are not sent (1028s clause 6).
    let t = Psd::default().to_id3();
    assert_eq!(t, [0x49, 0x44, 0x33, 3, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn hdlc_aas_round_trip_with_stuffing() {
    let payload = vec![0x7E, 0x7D, 1, 2, 3];
    let f = hdlc_frame(0x1000, 5, &payload);
    // DTPF 0x21, then port and sequence little-endian (1019s table 5-1).
    assert_eq!(&f[..6], &[0x7E, 0x21, 0x00, 0x10, 0x05, 0x00]);
    let mut h = AasReceiver::default();
    let mut out = Vec::new();
    h.push(&f, &mut out);
    assert_eq!(
        out,
        vec![AasPacket {
            port: 0x1000,
            seq: 5,
            payload
        }]
    );
    let mut bad = f.clone();
    bad[4] ^= 1;
    h.push(&bad, &mut out);
    assert_eq!(h.bad, 1);
}

#[test]
fn the_fixed_bearer_follows_clause_7_5() {
    // Table 7-2: a width byte 0xNN means 2N bytes, 0x44 is 8 bytes, 0x00 one byte.
    assert_eq!(sync_width_byte(8), Some(0x44));
    assert_eq!(sync_width_byte(1), Some(0x00));
    assert_eq!(sync_width_byte(30), Some(0xFF));
    assert_eq!((sync_width_byte(3), sync_width_byte(32)), (None, None));
    assert_eq!(
        (ccc_width(0x44), ccc_width(0x00), ccc_width(0x45)),
        (Some(8), Some(1), None)
    );
    // Table 7-4: one message of 1 to 4 sub-channels takes 8, 12, 16 or 20 bytes with its flag. Figure 7-3: Np, Rw, then the
    // length low byte first.
    for n in 1..=4 {
        let cfg = FixedConfig {
            mpl: 0b01,
            subs: vec![
                SubChannel {
                    parity: 32,
                    depth: 4,
                    len: 0x0102,
                };
                n
            ],
        };
        let m = ccc_message(&cfg);
        assert_eq!(m.len(), CCC_LENGTH[n - 1]);
        assert_eq!(&m[..6], &[0x7E, 0b01, 32, 4, 0x02, 0x01]);
        assert_eq!(parse_ccc(&m[1..]), Some(cfg));
    }
    // The synchronization channel sends count, width, width, width, the count stepping by 4. The reader learns the width and
    // the configuration and finds the sub-channel bytes.
    let cfg = FixedConfig {
        mpl: 0b01,
        subs: vec![SubChannel {
            parity: 0,
            depth: 0,
            len: 50,
        }],
    };
    let mut w = FixedWriter::new(cfg.clone(), 8).unwrap();
    let mut r = FixedReader::default();
    let mut syncs = Vec::new();
    for k in 0..9u8 {
        let mut pdu = vec![0xAA; 20];
        let stream = vec![k; 50];
        w.region(&[&stream], &mut pdu);
        assert_eq!(pdu.len(), 20 + w.region_len());
        syncs.push(*pdu.last().unwrap());
        let subs = r.push(&pdu);
        // The width is known at the second width byte (PDU 2), the configuration at the flag after the message (PDU 3).
        if k >= 3 {
            assert_eq!(subs, vec![(0, 20, 70)], "PDU {k}");
            assert!(pdu[20..70].iter().all(|&x| x == k));
        }
    }
    assert_eq!(
        syncs,
        [0x00, 0x44, 0x44, 0x44, 0x04, 0x44, 0x44, 0x44, 0x08]
    );
    assert_eq!(r.config, Some(cfg));
}

#[test]
fn lot_and_sig_round_trip() {
    let body: Vec<u8> = (0..1000).map(|i| i as u8).collect();
    let mut c = LotCollector::default();
    let mut done = None;
    for f in lot_fragments(7, "logo.png", 0x4F32_8CA0, 61400, &body, 256) {
        done = done.or(c.push(0x1234, &f, 1.0));
    }
    let f = &c.files[&done.unwrap()];
    assert_eq!(&f.bytes.as_ref().unwrap()[..], &body[..]);
    assert_eq!(f.name, "logo.png");
    let s = vec![SigService {
        data: false,
        number: 1,
        name: "Main".into(),
        components: vec![SigComponent {
            data: true,
            id: 1,
            port: 0x1234,
            kind: 3,
            mime: 0xBE4B_7536,
            sdt: 0,
        }],
    }];
    assert_eq!(parse_sig(&sig_bytes(&s)), Some(s));
}

/// Bits as a string of 0 and 1.
fn bit_text(b: &[u8]) -> String {
    b.iter().map(|&x| if x == 1 { '1' } else { '0' }).collect()
}

#[test]
fn the_sis_messages_follow_clause_4() {
    // Table 4-1: payload bits of MSG IDs 0 to 10.
    let sizes: Vec<Option<usize>> = (0..12).map(msg_payload_bits).collect();
    assert_eq!(
        sizes,
        [32, 22, 58, 32, 27, 58, 27, 22, 58, 58, 27]
            .map(Some)
            .into_iter()
            .chain([None])
            .collect::<Vec<_>>()
    );
    // Clause 4.2.1: "ABCD" is 00000 00001 00010 00011 00. The extension 01 appends "-FM".
    assert_eq!(
        bit_text(&short_name_msg("ABCD", false)),
        "0001".to_string() + "0000000001000100001100"
    );
    assert!(bit_text(&short_name_msg("ABCD", true)).ends_with("01"));
    // Figure 4-2: country code (two letters of table 4-3), 3 reserved bits, facility ID.
    let m = station_id_msg("US", 54321);
    assert_eq!(
        bit_text(&m[4..]),
        format!("{:05b}{:05b}000{:019b}", 20, 18, 54321)
    );
    // Clause 5.1: N 39 11' 46.32", W 76 49' 6.59", 90.7 m give the high portion 0x89CC8E0 and the low portion 0x6CCB9EC (27
    // bits, left-justified).
    let [hi, lo] = location_msgs(39.1962, -76.8185, 90.7);
    let value = |b: &[u8]| b[4..].iter().fold(0u32, |a, &x| a << 1 | x as u32);
    assert_eq!((hi.len(), lo.len()), (31, 31));
    assert_eq!(value(&hi) << 1, 0x089C_C8E0);
    assert_eq!(value(&lo) << 1, 0x06CC_B9EC);
    // Figure 4-7: the checksum. 'A' + 'B' + ... is 0x41 + 0x42 = 0x83, and 0x0083 folds to 0x00 + 0x83 = 0x83, 7 bits 0x03.
    assert_eq!(message_checksum(b"AB"), 0x03);
    let big = vec![0xFF; 190]; // sum 0xBD42: 0x3D + 0x42 = 0x7F
    assert_eq!(message_checksum(&big), 0x7F);
    // Every message is its MSG ID and a payload of the length of table 4-1.
    let mut all = vec![
        station_id_msg("US", 1),
        short_name_msg("WTST", true),
        audio_service_msg(1, 14, false, 0),
        data_service_msg(65, 0x2D7, false),
        parameter_msg(0, DEFAULT_LEAP_SECONDS),
        hi,
        lo,
    ];
    all.extend(long_name_msgs(b"Station data you can see", 1));
    all.extend(message_msgs(
        b"This is a station message of some length.",
        2,
        true,
    ));
    all.extend(slogan_msgs(b"Station data you can see"));
    all.extend(universal_name_msgs(b"WXYZ-LP Town", false));
    for m in &all {
        let id = m[..4].iter().fold(0u8, |a, &x| a << 1 | x);
        assert_eq!(Some(m.len() - 4), msg_payload_bits(id), "MSG ID {id}");
    }
}

#[test]
fn the_sis_pdu_follows_figure_4_1_and_clause_4_11() {
    // g(x) = x^12 + x^11 + x^3 + x + 1.
    assert_eq!(SIS_CRC_POLY, 1 << 12 | 1 << 11 | 1 << 3 | 1 << 1 | 1);
    let a = short_name_msg("WTST", false);
    let b = station_id_msg("US", 54321);
    let p = sis_pdu(&a, Some(&b), true, 0b10).unwrap();
    assert_eq!(p.len(), 80);
    // Type 0 in bit 0, Ext 1 in bit 1, MSG ID 1 in bits 2:5, reserved bit 64, time locked bit 65, ADV ALFN bits 66:67.
    assert_eq!((p[0], p[1]), (0, 1));
    assert_eq!(bit_text(&p[2..6]), "0001");
    assert_eq!(bit_text(&p[26..30]), "0000");
    assert_eq!((p[64], p[65], p[66], p[67]), (0, 1, 1, 0));
    // The CRC is the remainder of bits 79 to 0 (bit 79 the highest power, bits 68 to 79 as zeros), bit 68 most significant.
    let crc = p[68..].iter().fold(0u16, |a, &x| a << 1 | x as u16);
    assert_eq!(crc, sis_crc(&p));
    let poly: Vec<u8> = (0..80)
        .rev()
        .map(|i| if i >= 68 { 0 } else { p[i] })
        .collect();
    assert_eq!(
        crc as u128,
        sdrkit_util_bits::crc::cyclic::remainder(&poly, 0, 0x180B, 12)
    );
    // Two payloads of 54 bits fit, 55 do not, and one of 58 fits alone.
    assert!(sis_pdu(&[0; 31], Some(&[0; 31]), false, 0).is_some());
    assert!(sis_pdu(&[0; 31], Some(&[0; 32]), false, 0).is_none());
    assert!(sis_pdu(&[0; 62], None, false, 0).is_some());
    // A changed bit fails the CRC.
    let mut r = SisReceiver::default();
    let mut q = p.clone();
    q[20] ^= 1;
    r.push(&q, 0, true);
    assert_eq!((r.good, r.bad), (0, 1));
    r.push(&p, 0, true);
    assert_eq!((r.good, r.sis.short_name.as_str()), (1, "WTST"));
    assert!(r.sis.time_locked);
}

#[test]
fn the_sis_schedules_follow_tables_5_1_and_5_5() {
    use Msg::*;
    // Required in every schedule: station ID, location, service information, SIS parameter and the short name (clause 5.2).
    for sched in [&FM_SCHEDULE[..], &AM_SCHEDULE[..]] {
        for m in [StationId, Location, ServiceInfo, Parameter, ShortName] {
            assert!(sched.iter().any(|s| s.0 == m || s.1 == Some(m)), "{m:?}");
        }
        assert!(sched
            .iter()
            .all(|s| s.0 != UniversalName && s.1 != Some(UniversalName)));
        // Two messages fit in 54 bits.
        for &(a, b) in sched {
            if let Some(b) = b {
                let n = msg_payload_bits(a.id()).unwrap() + msg_payload_bits(b.id()).unwrap();
                assert!(n <= 54, "{a:?} {b:?}");
            }
        }
    }
    assert_eq!(FM_SCHEDULE[3], (Slogan, None));
    assert_eq!(FM_SCHEDULE[7], (Location, Some(Location)));
    assert_eq!(FM_SCHEDULE[12], (Parameter, Some(StationId)));
    assert_eq!(AM_SCHEDULE[1], (Message, None));
    assert_eq!(AM_SCHEDULE[3], (Parameter, Some(Location)));
    assert_eq!(AM_SCHEDULE[7], (LongName, None));
}

#[test]
fn the_adv_alfn_follows_clause_5_3() {
    // FM: block 0 carries d30:31 (the two lowest bits), block 7 d16:17, block 8 d14:15 and block 15 d0:1.
    let alfn = 0x9E37_79B9u32;
    assert_eq!(adv_alfn(alfn, 0, true), (alfn & 3) as u8);
    assert_eq!(adv_alfn(alfn, 7, true), ((alfn >> 14) & 3) as u8);
    assert_eq!(adv_alfn(alfn, 8, true), ((alfn >> 16) & 3) as u8);
    assert_eq!(adv_alfn(alfn, 15, true), (alfn >> 30) as u8);
    // AM: when d30:31 are 00 the frame carries the 16 most significant bits, d14:15 in block 0 and d0:1 in block 7.
    let am = 0x1234_5678u32;
    assert_eq!(am & 3, 0);
    assert_eq!(adv_alfn(am, 0, false), ((am >> 16) & 3) as u8);
    assert_eq!(adv_alfn(am, 7, false), (am >> 30) as u8);
    assert_eq!(adv_alfn(am + 1, 7, false), (((am + 1) >> 14) & 3) as u8);
}

fn test_sis() -> Sis {
    Sis {
        short_name: "WTST".into(),
        fm_suffix: true,
        universal_name: String::new(),
        long_name: "Station data you can see".into(),
        slogan: "Station data you can see".into(),
        message: "This is a test signal. HD Radio sound uses the HDC codec.".into(),
        message_priority: true,
        country: "US".into(),
        facility_id: 54321,
        lat: 40.75,
        lon: -73.99,
        alt_m: 96,
        audio_services: vec![(0, 5, false, 0), (1, 14, false, 0)],
        data_services: vec![(1, 0x2D7, false)],
        params: [(0, DEFAULT_LEAP_SECONDS)].into(),
        time_locked: false,
        alfn: None,
    }
}

#[test]
fn sis_round_trip() {
    for fm in [true, false] {
        let sis = test_sis();
        let blocks = if fm { 16 } else { 8 };
        let mut tx = SisTransmitter::new(&sis, fm, 1);
        let mut r = SisReceiver::default();
        let start = 0x0001_0000u32 - 3;
        for f in 0..12 {
            for b in 0..blocks {
                r.push(&tx.pdu(b, start + f), b, fm);
            }
        }
        assert_eq!(r.bad, 0);
        let mut want = sis.clone();
        want.alfn = Some(start + 11);
        // S9.13 positions: steps of 1/8192 degree.
        want.lon = (want.lon * 8192.0).round() / 8192.0;
        assert_eq!(r.sis, want, "fm {fm}");
    }
}

/// The receiver does not design its filters again for each block.
#[test]
fn the_l1_steady_state_allocates_little() {
    let o = GenOptions {
        waveform: Waveform::Mp1,
        analog_db: 0.0,
    };
    let x = generate(o, l1::FM_RATE, 6.0, &[]);
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    rx.configure(&InputInfo::new(l1::FM_RATE, 98.5e6, 98.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    let half = x.len() / 2;
    for c in x[..half].chunks(8192) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let blocks0 = int(rx.results(), res::HdradioFields::BLOCKS.key);
    let n = allocations(|| {
        for c in x[half..].chunks(8192) {
            rx.feed(c, &mut out);
            out.clear();
        }
    });
    let blocks = int(rx.results(), res::HdradioFields::BLOCKS.key) - blocks0;
    assert!(blocks >= 30, "{blocks}");
    // The band plan and the cell plan are made once, not for each block (each made some 400 allocations).
    assert!(
        (n as i64) < 200 * blocks,
        "{n} allocations for {blocks} blocks"
    );
}

/// A change of frequency forgets the station: its programs, files, station data, text, counts and measurements.
#[test]
fn a_change_of_frequency_forgets_the_station() {
    let o = GenOptions {
        waveform: Waveform::Mp1,
        analog_db: 0.0,
    };
    let x = generate(o, l1::FM_RATE, 12.0, &with_seed(snr(15.0).to_vec()));
    let mut rx = hd::make_receiver(&defaults()).unwrap();
    rx.configure(&InputInfo::new(l1::FM_RATE, 98.5e6, 98.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(100_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert_eq!(
        text(rx.results(), res::HdradioFields::CALL_SIGN.key),
        "WTST"
    );
    assert!(!row_ids(rx.results(), PROGRAMS.key).is_empty());
    let p1 = int(rx.results(), res::HdradioFields::P1_OK.key);
    let blocks = int(rx.results(), res::HdradioFields::BLOCKS.key);
    rx.forget();
    let r = rx.results();
    assert!(row_ids(r, PROGRAMS.key).is_empty());
    assert!(row_ids(r, FILES.key).is_empty());
    for key in [
        "call_sign",
        "band",
        "offset_hz",
        "mer_lower_db",
        "location",
        "text",
    ] {
        assert!(is_none(r, key), "{key}");
    }
    assert_eq!(int(r, res::HdradioFields::P1_OK.key), 0);
    // The counts start from zero: the same signal again counts as much as the first time, not twice as much.
    for c in x.chunks(100_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = rx.results();
    let again = int(r, res::HdradioFields::P1_OK.key);
    assert!(again > 0 && again <= p1 + 1, "{again} after {p1}");
    assert!(int(r, res::HdradioFields::BLOCKS.key) <= blocks + 2);
}
