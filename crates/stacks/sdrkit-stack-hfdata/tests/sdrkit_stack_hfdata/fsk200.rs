//! FSK200: the CRC and block vectors of the documents and the worked examples, the f01 text layer, both submodes on their
//! test signals at stated levels, offsets and clock errors, and no rows on noise, a carrier or the other submode.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::fsk200::{
    self, Case, F01Row, F01Text, F06Blocks, GenKind, Submode, FSK200,
};
use sdrkit_stack_hfdata::senders::Tone;
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;

fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap())
        .collect()
}

#[test]
fn the_crc_is_the_one_of_the_documented_example() {
    assert_eq!(CRC16_IBM_SDLC.checksum(b"123456789"), 0x906E);
    assert_eq!(fsk200::gen_crc16(b"123456789"), 0x906E);
    // The payload of the example of the profile has the CRC bbdd.
    let p = hex("1be980354f9b4a02d028ec5ebeda1979");
    assert_eq!(CRC16_IBM_SDLC.checksum(&p), 0xBBDD);
    assert_eq!(fsk200::gen_crc16(b"PLAIN TEXT BLOCK"), 0x12E3);
    assert_eq!(fsk200::gen_crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn the_worked_blocks_pack_and_unpack() {
    let data = fsk200::figure_data(
        &[123, 456, 789, 0, 999, 512, 34, 5, 678, 901, 234, 567],
        [8, 9],
    );
    assert_eq!(
        data.to_vec(),
        hex("1E DC 8C 54 00 F9 E0 00 88 05 A9 B8 53 AA 37 89")
    );
    let b = fsk200::pack(2, &data, &[0; 12]);
    assert_eq!(
        b[..24].to_vec(),
        hex("7D 12 B0 E6 00 40 10 0A EF 53 D9 A7 CE 98 80 B9 C0 80 50 5B 48 34 08 A0")
    );
    let u = fsk200::unpack(&b);
    assert_eq!((u.number, u.data, u.crc), (2, data, 0x0B40));
    assert!(u.crc_ok());
    let mut l = F06Blocks::new();
    let mut out = Vec::new();
    l.block(2, &u.data, true, "CRC OK", &mut out);
    assert_eq!(
        out[0].text,
        "12345678900099951203400567890123456789 | CRC OK"
    );
    // The header of 5.3.5 and the header example of the profile (67 blocks, 2 messages, at blocks 1 and 22).
    let h = fsk200::header_data(23, &[2]);
    assert_eq!(
        h.to_vec(),
        hex("02 E1 00 40 00 00 00 00 00 00 00 00 00 00 00 00")
    );
    let hb = fsk200::pack(0, &h, &[0; 12]);
    assert_eq!(
        hb[..24].to_vec(),
        hex("7D 12 B0 E6 00 00 00 00 20 00 E0 00 10 00 00 00 00 07 40 09 00 0F 00 0D")
    );
    assert_eq!(fsk200::unpack(&hb).crc, 0x79FD);
    out.clear();
    l.block(0, &h, true, "CRC OK", &mut out);
    assert_eq!(
        out[0].text,
        "23 blocks, 1 message(s) | CRC OK | Message 1 at block #2"
    );
    let mut d = [0u8; 16];
    d[..6].copy_from_slice(&hex("0862 0020 02c0"));
    assert_eq!(fsk200::header(&d), (67, 2, vec![1, 22]));
    // The first ten bits of a figure message are 111.
    let first = fsk200::figure_data(&[111, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], [0, 0]);
    assert_eq!(first[0], 0x1B);
}

#[test]
fn file_and_ascii_blocks_read_through_the_block_layer() {
    let mut l = F06Blocks::new();
    let mut out = Vec::new();
    l.block(0, &fsk200::header_data(4, &[1]), true, "CRC OK", &mut out);
    let mut b1 = [0u8; 16];
    b1[..4].copy_from_slice(&[0, 0, 0, 0x28]);
    b1[8..].copy_from_slice(b"NOTE.TXT");
    l.block(1, &b1, true, "CRC OK", &mut out);
    let mut b2 = [0u8; 16];
    b2[..4].copy_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    b2[4..].copy_from_slice(b"HELLO THERE!");
    l.block(2, &b2, true, "CRC OK", &mut out);
    let mut b3 = [0x99u8; 16];
    b3[..14].copy_from_slice(b"LINE TWO\r\nEND\0");
    l.block(3, &b3, true, "CRC OK", &mut out);
    let texts: Vec<(&str, &str)> = out.iter().map(|r| (r.kind, r.text.as_str())).collect();
    assert_eq!(
        texts[1],
        ("File", "file named NOTE.TXT, size 40 bytes | CRC OK")
    );
    assert_eq!(
        texts[2],
        ("File", "CRC of the file contents 0xdeadbeef | CRC OK")
    );
    assert_eq!(texts[3], ("Block", "HELLO THERE! | CRC OK"));
    assert_eq!(texts[4], ("Block", "LINE TWO<CR><LF>END<EOF> | CRC OK"));
    assert!(texts[5].1.contains("does not match"), "{texts:?}");
    let mut l = F06Blocks::new();
    out.clear();
    let mut a = [b'.'; 16];
    a[..2].copy_from_slice(&[0x34, 0x36]);
    a[2..14].copy_from_slice(b"ABC 123 XYZ.");
    l.block(1, &a, true, "CRC OK", &mut out);
    l.block(2, b"PLAIN TEXT BLOCK", true, "CRC OK", &mut out);
    assert_eq!(out[1].text, "PLAIN TEXT BLOCK | CRC OK");
}

#[test]
fn the_f01_text_layer_reads_the_worked_example() {
    // The figure 1 is code 23: elements 1, 1, 1, 0, 1.
    assert_eq!(fsk200::f01_code('1'), Some(23));
    assert_eq!(
        (0..5).map(|k| (23 >> k) & 1).collect::<Vec<u8>>(),
        [1, 1, 1, 0, 1]
    );
    let codes: Vec<u8> = "12345 "
        .chars()
        .map(|c| fsk200::f01_code(c).unwrap())
        .collect();
    assert_eq!(codes, [23, 19, 1, 10, 16, 4]);
    let mut t = F01Text::new(Case::Figures);
    let mut out = Vec::new();
    for c in codes.iter().chain([8u8, 2].iter()) {
        t.push(*c, true, &mut out);
    }
    assert_eq!(out, vec![F01Row::Text("12345".into())]);
    // A block of the document form.
    let groups = fsk200::test_groups(6, 1);
    let text = fsk200::f01_text(&groups, 1);
    let mut out = Vec::new();
    for c in text.chars() {
        t.push(fsk200::f01_code(c).unwrap(), true, &mut out);
    }
    assert_eq!(out.len(), 3, "{out:?}");
    assert!(matches!(&out[0], F01Row::Header(h) if h.contains(" 11177 ")));
    let lines: Vec<String> = out
        .iter()
        .filter_map(|r| match r {
            F01Row::Block(b) => Some(b.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(groups_right(&groups, &lines), 6, "{lines:?}");
}

fn ch(snr: Option<f64>, offset_hz: f64, drift_hz_s: f64, ppm: f64, seed: u64) -> Channel {
    Channel {
        snr_db: snr,
        offset_hz,
        drift_hz_s,
        ppm,
        seed,
        ..Channel::default()
    }
}

/// Groups of f01 read right, of `n` sent.
fn f01_case(n: usize, centre: f64, rev: bool, rate: f64, dial: f64, c: &Channel) -> usize {
    let groups = fsk200::test_groups(n, c.seed);
    let seq = fsk200::f01_sequence(&fsk200::f01_text(&groups, c.seed), centre, rev);
    let x = signal(&seq, rate, dial, c);
    let (rx, _) = run_at(&FSK200, &x, rate, dial, &[("submode", json!("f01"))]);
    let rows = sender_rows(rx.as_ref());
    groups_right(&groups, &texts_of(&rows, "Block"))
}

#[test]
fn f01_reads_its_test_signal_at_15_db() {
    let s = Some(15.0);
    for (centre, rev, rate, dial, c, what) in [
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 0.0, 1),
            "centre 1500 Hz",
        ),
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, 31.0, 0.0, 0.0, 2),
            "dial +31 Hz",
        ),
        (
            650.0,
            false,
            48e3,
            dial_of(48e3),
            ch(s, 0.0, 0.0, 0.0, 3),
            "centre 650 Hz at 48 ksps",
        ),
        (
            2600.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 0.0, 4),
            "centre 2600 Hz",
        ),
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, -20.0, 0.5, 10_000.0, 5),
            "clock +1 %, +0.5 Hz/s, -20 Hz",
        ),
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, 7.0, 0.0, -5000.0, 6),
            "clock -0.5 %, +7 Hz",
        ),
        (1500.0, true, 24e3, 0.0, ch(s, 0.0, 0.0, 0.0, 7), "reversed"),
        (
            1500.0,
            false,
            250e3,
            -60e3,
            ch(s, 0.0, 0.0, 0.0, 8),
            "250 ksps, offset -60 kHz",
        ),
    ] {
        let right = f01_case(60, centre, rev, rate, dial, &c);
        assert!(right >= 59, "{what}: {right} of 60");
    }
}

#[test]
fn f01_sweep_reaches_90_percent_at_10_db() {
    let curve: Vec<(f64, f64)> = (0..9)
        .map(|k| {
            let snr = -6.0 + 2.0 * k as f64;
            let r: usize = [51u64, 52]
                .iter()
                .map(|&seed| {
                    f01_case(
                        60,
                        1500.0,
                        false,
                        24e3,
                        0.0,
                        &ch(Some(snr), 12.0, 0.0, 200.0, seed),
                    )
                })
                .sum();
            (snr, r as f64 / 120.0)
        })
        .collect();
    println!("f01 sweep: {curve:?}");
    assert!(curve[8].1 >= 0.9, "{curve:?}");
}

struct F06Run {
    right: usize,
    wrong_ok: usize,
    headers: Vec<String>,
    corrected: usize,
    rows: Vec<(String, String)>,
    headline: String,
}

fn f06_case(
    set: &[(&str, Json)],
    centre: f64,
    rev: bool,
    flips: &[(usize, usize)],
    rate: f64,
    dial: f64,
    c: &Channel,
) -> F06Run {
    let v = values(&FSK200, set);
    let kind = v.get(&fsk200::GEN_KIND);
    let blocks = fsk200::f06_blocks(
        kind,
        20,
        v.get(&fsk200::GEN_TEXT),
        v.get(&fsk200::GEN_NAME),
        c.seed,
    );
    let seq = fsk200::f06_sequence(&blocks, flips, centre, rev);
    let x = signal(&seq, rate, dial, c);
    let (rx, _) = run_at(&FSK200, &x, rate, dial, &[("submode", json!("f06"))]);
    let rows = sender_rows(rx.as_ref());
    let mut right = 0;
    let mut wrong_ok = 0;
    for (_, text) in rows.iter().filter(|(k, _)| k == "Block") {
        let Some((digits, crc)) = text.split_once(" | ") else {
            continue;
        };
        if !crc.starts_with("CRC OK") {
            continue;
        }
        if blocks.iter().any(|(_, _, d)| d.as_deref() == Some(digits)) {
            right += 1;
        } else if kind == GenKind::F06 {
            wrong_ok += 1;
        }
    }
    F06Run {
        right,
        wrong_ok,
        headers: texts_of(&rows, "Header"),
        corrected: rows
            .iter()
            .filter(|(_, t)| t.contains("one bit corrected"))
            .count(),
        headline: rx.results().status().summary.clone(),
        rows,
    }
}

#[test]
fn f06_reads_its_test_signal_at_15_db() {
    let s = Some(15.0);
    for (centre, rev, rate, dial, c, what) in [
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 0.0, 1),
            "centre 1500 Hz",
        ),
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, -44.0, 0.0, 0.0, 2),
            "dial -44 Hz",
        ),
        (
            1100.0,
            false,
            48e3,
            dial_of(48e3),
            ch(s, 0.0, 0.0, 0.0, 3),
            "centre 1100 Hz at 48 ksps",
        ),
        (
            2700.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 0.0, 4),
            "centre 2700 Hz",
        ),
        (
            1500.0,
            false,
            24e3,
            0.0,
            ch(s, 10.0, -0.5, 500.0, 5),
            "+500 ppm, -0.5 Hz/s, +10 Hz",
        ),
        (1500.0, true, 24e3, 0.0, ch(s, 0.0, 0.0, 0.0, 6), "reversed"),
        (
            1500.0,
            false,
            250e3,
            30e3 + 5.0,
            ch(s, 0.0, 0.0, 0.0, 7),
            "250 ksps, offset +30 kHz",
        ),
    ] {
        let r = f06_case(&[], centre, rev, &[], rate, dial, &c);
        assert!(
            r.right >= 19 && r.headers.len() == 2 && r.wrong_ok == 0,
            "{what}: {} of 20, headers {:?}",
            r.right,
            r.headers
        );
    }
}

#[test]
fn f06_clean_header_corrections_and_traffic_kinds() {
    let clean = f06_case(
        &[],
        1500.0,
        false,
        &[],
        24e3,
        0.0,
        &ch(None, 0.0, 0.0, 0.0, 1),
    );
    assert!(
        clean.headers[0].starts_with("22 blocks, 1 message(s) | CRC OK")
            && clean.headers[0].contains("Message 1 at block #1"),
        "{:?}",
        clean.headers
    );
    // One bit flipped in three blocks (counted in the order sent: block 0 is the header).
    let r = f06_case(
        &[],
        1500.0,
        false,
        &[(3, 81), (8, 116), (15, 165)],
        24e3,
        0.0,
        &ch(Some(25.0), 0.0, 0.0, 0.0, 2),
    );
    assert_eq!((r.right, r.corrected), (20, 3), "{:?}", r.rows);
    // A file at 15 dB, dial +9 Hz.
    let r = f06_case(
        &[("gen_kind", json!("f06a"))],
        1500.0,
        false,
        &[],
        24e3,
        0.0,
        &ch(Some(15.0), 9.0, 0.0, 0.0, 3),
    );
    let files = texts_of(&r.rows, "File");
    assert!(
        files
            .iter()
            .any(|t| t == "file named TEST.TXT, size 42 bytes | CRC OK"),
        "{files:?}"
    );
    assert!(files
        .iter()
        .any(|t| t.starts_with("CRC of the file contents 0x") && t.ends_with("| CRC OK")));
    assert!(
        files
            .iter()
            .any(|t| t.contains("the complement of the CRC-32")),
        "{files:?}"
    );
    let blocks: String = texts_of(&r.rows, "Block")
        .iter()
        .map(|t| t.split(" | ").next().unwrap_or("").to_string())
        .collect();
    assert!(
        blocks.contains("FIRST LINE OF") && blocks.contains("<EOF>"),
        "{blocks}"
    );
    assert!(r.headline.contains("F06a"), "{}", r.headline);
    // ASCII blocks.
    let r = f06_case(
        &[
            ("gen_kind", json!("ascii")),
            ("gen_text", json!("PLAIN TEXT BLOCK")),
        ],
        1500.0,
        false,
        &[],
        24e3,
        0.0,
        &ch(Some(15.0), 0.0, 0.0, 0.0, 4),
    );
    assert!(
        texts_of(&r.rows, "Block")
            .iter()
            .any(|t| t == "PLAIN TEXT BLOCK | CRC OK"),
        "{:?}",
        r.rows
    );
}

#[test]
fn f06_sweep_reaches_90_percent_at_8_db_without_a_wrong_block() {
    let mut wrong = 0;
    let curve: Vec<(f64, f64)> = (0..8)
        .map(|k| {
            let snr = -6.0 + 2.0 * k as f64;
            let mut right = 0;
            for seed in [61u64, 62] {
                let r = f06_case(
                    &[],
                    1500.0,
                    false,
                    &[],
                    24e3,
                    0.0,
                    &ch(Some(snr), 17.0, 0.0, 200.0, seed),
                );
                right += r.right;
                wrong += r.wrong_ok;
            }
            (snr, right as f64 / 40.0)
        })
        .collect();
    println!("f06 sweep: {curve:?}, {wrong} wrong blocks with CRC OK");
    assert!(curve[7].1 >= 0.9 && wrong == 0, "{curve:?} {wrong}");
}

#[test]
fn noise_a_carrier_and_the_other_submode_give_no_row() {
    let f01_signal = {
        let g = fsk200::test_groups(60, 1);
        fsk200::f01_sequence(&fsk200::f01_text(&g, 1), 1500.0, false)
    };
    for (sub, carrier) in [(Submode::F01, 1250.0), (Submode::F06, 1000.0)] {
        let key = if sub == Submode::F01 { "f01" } else { "f06" };
        let mut cases = vec![
            (vec![Tone::silence(60.0)], 0.0, 71u64, "noise"),
            (vec![Tone::silence(60.0)], 0.0, 72, "noise"),
            (vec![Tone::new(40.0, carrier)], 0.0, 73, "carrier"),
        ];
        if sub == Submode::F06 {
            cases.push((f01_signal.clone(), 15.0, 74, "an f01 signal"));
        }
        for (seq, snr, seed, what) in cases {
            let x = signal(&seq, 24e3, 0.0, &ch(Some(snr), 0.0, 0.0, 0.0, seed));
            let (rx, _) = run_at(&FSK200, &x, 24e3, 0.0, &[("submode", json!(key))]);
            let rows = sender_rows(rx.as_ref());
            assert!(rows.is_empty(), "{key} {what}: {rows:?}");
        }
    }
}
