//! GW FSK: the parser on the documented and worked bursts in every reading, the MMSI codings, the receiver on its test signal
//! at stated levels, offsets and clock errors, and no rows on noise, a carrier or an FSK200/500 signal.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::gwfsk::{self, GwParser, Polarity, GWFSK};
use sdrkit_stack_hfdata::senders::Tone;

/// The station names of the data file as the setting text.
fn stations() -> String {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/hfdata/gw_shore_stations.csv"),
    )
    .unwrap();
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .filter_map(|l| {
            let (code, rest) = l.split_once(',')?;
            let name = rest.trim_start_matches('"');
            let name = &name[..name.find('"')?];
            Some(format!("{code}={name}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn bits(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b - b'0').collect()
}

fn read(p: &mut GwParser, b: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    assert!(p.read(b, Polarity::Auto, &mut out), "{b:?}");
    out.into_iter()
        .map(|r| (r.kind.to_string(), r.text))
        .collect()
}

#[test]
fn the_parser_reads_the_worked_bursts_in_every_reading() {
    let mut p = GwParser::new(&stations());
    let text = bits("1000100110101000100111010001110010000001000000001101110000000011001010");
    assert_eq!(
        text,
        gwfsk::packet_bits(2, 0, 106, &[0x27, 0x47, 0x20, 0x40, 0x37, 0x00])
    );
    let marker = gwfsk::marker_bits(0xFA);
    for b in [text.clone(), marker.clone()] {
        let inv: Vec<u8> = b.iter().map(|v| v ^ 1).collect();
        let mut lost = vec![1 - b[0]];
        lost.extend(&b);
        for v in [b.clone(), inv, lost] {
            let r = read(&mut p, &v);
            assert_eq!(r.len(), 1, "{r:?}");
        }
    }
    let r = read(&mut p, &text);
    assert_eq!(
        r[0].1,
        "Type=2 Count=0 Subtype=106 (10001001101010) (0x27,0x47,0x20,0x40,0x37,0x0) | final packet AB12E3"
    );
    let r = read(&mut p, &marker);
    assert_eq!(
        r[0].1,
        "free channel marker from station 0xfa (KPH, San Francisco, USA) | 20 38 a3 1f 6c 35 3b 4b f2 f2 f2 fa fa fa fa fa fa ff"
    );
    let r = read(
        &mut p,
        &gwfsk::packet_bits(2, 1, 101, &[0x04, 0x63, 0x42, 0x68, 0x02, 0x80]),
    );
    assert!(r[0].1.ends_with("| MMSI : 235012389"), "{r:?}");
    let r = read(
        &mut p,
        &gwfsk::packet_bits(2, 1, 101, &[0xE2, 0x72, 0xFF, 0xFF, 0xE7, 0xE6]),
    );
    assert!(r[0].1.ends_with("| 545800000 | Command=444"), "{r:?}");
    let r = read(
        &mut p,
        &gwfsk::packet_bits(5, 1, 41, &[0x03, 0x66, 0xA6, 0x04, 0x37, 0x15]),
    );
    assert!(r[0].1.ends_with("| connection opening"), "{r:?}");
    let r = read(&mut p, &gwfsk::ACK);
    assert_eq!(r, vec![("ACK".to_string(), "GW ACK".to_string())]);
    // The position report of ten fragments.
    let mut rows = Vec::new();
    for b in gwfsk::test_bursts(true).iter().take(12) {
        rows.extend(read(&mut p, b));
    }
    let pos: Vec<&(String, String)> = rows.iter().filter(|r| r.0 == "Position").collect();
    assert_eq!(pos.len(), 1);
    assert_eq!(
        pos[0].1,
        "234.56,N,01234.56,E,123456,A*4B<<<<<1234567890,.**<<..AABBEE<<<<<<"
    );
}

#[test]
fn the_mmsi_codings_read_the_worked_bytes() {
    assert_eq!(
        gwfsk::ship_bytes("235012389"),
        [0x04, 0x63, 0x42, 0x68, 0x02, 0x80]
    );
    assert_eq!(
        gwfsk::mmsi_digits(&[0x04, 0x63, 0x42, 0x68, 0x02, 0x80], 9, false),
        "235012389"
    );
    let b = [0x03, 0x66, 0xA6, 0x04, 0x37, 0x15];
    assert_eq!(gwfsk::mmsi_digits(&b, 12, false), "538001234567");
    assert_eq!(gwfsk::mmsi_digits(&b, 12, true), "974445678923");
    assert_eq!(gwfsk::ship_bytes("538001234567"), b);
    assert_eq!(
        gwfsk::mmsi_digits(&[0xE2, 0x72, 0xFF, 0xFF, 0xE7, 0xE6], 12, true),
        "545800000444"
    );
}

fn ch(snr: Option<f64>, offset_hz: f64, ppm: f64, seed: u64) -> Channel {
    Channel {
        snr_db: snr,
        offset_hz,
        ppm,
        seed,
        ..Channel::default()
    }
}

fn run(
    bursts: &[Vec<u8>],
    centre: f64,
    rev: bool,
    rate: f64,
    dial: f64,
    c: &Channel,
) -> Vec<(String, String)> {
    let b: Vec<(f64, Vec<u8>)> = bursts.iter().map(|b| (gwfsk::BAUD, b.clone())).collect();
    let seq = gwfsk::test_sequence(&b, centre, rev, 0.6);
    let x = signal(&seq, rate, dial, c);
    let (rx, _) = run_at(&GWFSK, &x, rate, dial, &[("stations", json!(stations()))]);
    sender_rows(rx.as_ref())
}

#[test]
fn the_receiver_reads_the_five_bursts_at_15_db() {
    let s = Some(15.0);
    let bursts = gwfsk::test_bursts(false);
    for (centre, rev, rate, dial, c, what) in [
        (
            1700.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 1),
            "centre 1700 Hz",
        ),
        (1700.0, false, 24e3, 0.0, ch(s, 27.0, 0.0, 2), "dial +27 Hz"),
        (
            600.0,
            false,
            48e3,
            dial_of(48e3),
            ch(s, 0.0, 0.0, 3),
            "centre 600 Hz at 48 ksps",
        ),
        (
            2900.0,
            false,
            24e3,
            0.0,
            ch(s, 0.0, 0.0, 4),
            "centre 2900 Hz",
        ),
        (
            1700.0,
            false,
            24e3,
            0.0,
            ch(s, -8.0, 10_000.0, 5),
            "clock +1 %, dial -8 Hz",
        ),
        (1700.0, true, 24e3, 0.0, ch(s, 0.0, 0.0, 6), "reversed"),
        (
            1700.0,
            false,
            250e3,
            -10e3 + 3.0,
            ch(s, 0.0, 0.0, 7),
            "250 ksps, offset -10 kHz",
        ),
    ] {
        let rows = run(&bursts, centre, rev, rate, dial, &c);
        let read: Vec<&(String, String)> = rows
            .iter()
            .filter(|r| ["Packet", "Channel marker", "ACK", "Position"].contains(&r.0.as_str()))
            .collect();
        let has = |s: &str| read.iter().any(|r| r.1.contains(s));
        assert!(
            read.len() == 5
                && has("0xfa (KPH")
                && has("0xcc (HEC")
                && has("final packet AB12E3")
                && has("final packet WN,7.<")
                && has("MMSI : 235012389"),
            "{what}: {rows:?}"
        );
    }
}

#[test]
fn the_receiver_reads_the_position_bursts() {
    let rows = run(
        &gwfsk::test_bursts(true),
        1700.0,
        false,
        24e3,
        0.0,
        &ch(Some(15.0), 5.0, 0.0, 8),
    );
    let pos = texts_of(&rows, "Position");
    assert!(
        pos == ["234.56,N,01234.56,E,123456,A*4B<<<<<1234567890,.**<<..AABBEE<<<<<<"]
            || pos == ["position report incomplete: 9 fragments"],
        "{rows:?}"
    );
    assert_eq!(texts_of(&rows, "ACK").len(), 1, "{rows:?}");
    assert!(texts_of(&rows, "Packet")
        .iter()
        .any(|t| t.ends_with("connection opening")));
}

/// Packets read right of 10 sent for each of `seeds` at `snr`, dial -15 Hz, clock +300 ppm.
fn sweep_point(snr: f64, seeds: std::ops::Range<u64>) -> f64 {
    let n = seeds.end - seeds.start;
    let mut right = 0;
    for seed in seeds {
        let p = gwfsk::random_packets(10, seed);
        let b: Vec<Vec<u8>> = p.iter().map(|x| x.0.clone()).collect();
        let rows = run(
            &b,
            1700.0,
            false,
            24e3,
            0.0,
            &ch(Some(snr), -15.0, 300.0, seed),
        );
        let got = texts_of(&rows, "Packet");
        assert!(
            texts_of(&rows, "Frame").is_empty(),
            "{snr} dB seed {seed}: {rows:?}"
        );
        right += p
            .iter()
            .filter(|(_, t)| {
                got.iter()
                    .any(|g| g.ends_with(&format!("final packet {t}")))
            })
            .count();
    }
    right as f64 / (10 * n) as f64
}

#[test]
fn the_sweep_reaches_90_percent_at_5_db() {
    let curve: Vec<(f64, f64)> = [-2.0, 0.0, 2.0, 3.0, 4.0, 5.0]
        .iter()
        .map(|&snr| (snr, sweep_point(snr, 81..89)))
        .collect();
    println!("gwfsk sweep: {curve:?}");
    assert!(curve[5].1 >= 0.9, "{curve:?}");
}

#[test]
fn the_content_setting_gives_the_bursts() {
    let b = gwfsk::content_bursts(gwfsk::DEFAULT_CONTENT).unwrap();
    assert!(b.iter().all(|x| x.0 == gwfsk::BAUD));
    assert_eq!(
        b.into_iter().map(|x| x.1).collect::<Vec<_>>(),
        gwfsk::test_bursts(false)
    );
    let b = gwfsk::content_bursts(
        "ack; packet 5/41 1 0366A6043715; frame200 000102030405060708090A0B0C0D",
    )
    .unwrap();
    assert_eq!(b[0].1, gwfsk::ACK);
    assert_eq!(
        b[1].1,
        gwfsk::packet_bits(5, 1, 41, &[0x03, 0x66, 0xA6, 0x04, 0x37, 0x15])
    );
    assert_eq!((b[2].0, b[2].1.len()), (200.0, 112));
    assert_eq!(
        gwfsk::content_bursts("marker FA; text 2/106 0 AB12"),
        Err(1)
    );
    assert_eq!(gwfsk::content_bursts("frame200 0001"), Err(0));
}

#[test]
fn frames_of_the_200_bd_form_are_read_at_15_db() {
    let frames = [
        "8d3c0f5aa5e1700012345678abcd",
        "f00f55aa0123456789abcdef1357",
        "2468ace013579bdf0f1e2d3c4b5a",
    ];
    let text = frames
        .iter()
        .map(|f| format!("frame200 {f}"))
        .chain(["marker FA".to_string()])
        .collect::<Vec<_>>()
        .join("; ");
    for seed in 1..=4u64 {
        let seq = gwfsk::test_sequence(&gwfsk::content_bursts(&text).unwrap(), 1700.0, false, 0.6);
        let x = signal(&seq, 24e3, 0.0, &ch(Some(15.0), 12.0, 0.0, seed));
        let (rx, _) = run_at(&GWFSK, &x, 24e3, 0.0, &[("polarity", json!("normal"))]);
        let rows = sender_rows(rx.as_ref());
        let got = texts_of(&rows, "Frame");
        for f in frames {
            let hex: Vec<String> = (0..14).map(|i| f[2 * i..2 * i + 2].to_string()).collect();
            let want = format!("14 bytes: {}", hex.join(" "));
            assert!(
                got.iter().any(|g| g.starts_with(&want)),
                "seed {seed}: {want}: {rows:?}"
            );
        }
        assert_eq!(
            texts_of(&rows, "Channel marker").len(),
            1,
            "seed {seed}: {rows:?}"
        );
    }
}

#[test]
fn noise_a_carrier_and_fsk200_give_no_row() {
    let f01 = {
        let g = sdrkit_stack_hfdata::fsk200::test_groups(60, 1);
        sdrkit_stack_hfdata::fsk200::f01_sequence(
            &sdrkit_stack_hfdata::fsk200::f01_text(&g, 1),
            1500.0,
            false,
        )
    };
    for (seq, snr, seed, what) in [
        (vec![Tone::silence(60.0)], 0.0, 91u64, "noise"),
        (vec![Tone::silence(60.0)], 0.0, 92, "noise"),
        (
            vec![
                Tone::silence(1.0),
                Tone::new(20.0, 1600.0),
                Tone::silence(1.0),
            ],
            0.0,
            93,
            "carrier",
        ),
        (f01, 15.0, 94, "FSK200/500"),
    ] {
        let x = signal(&seq, 24e3, 0.0, &ch(Some(snr), 0.0, 0.0, seed));
        let (rx, _) = run_at(&GWFSK, &x, 24e3, 0.0, &[]);
        let rows: Vec<(String, String)> = sender_rows(rx.as_ref())
            .into_iter()
            .filter(|r| r.0 != "Unknown")
            .collect();
        assert!(rows.is_empty(), "{what}: {rows:?}");
    }
}

#[test]
fn noise_over_eight_seeds_gives_no_row() {
    for seed in 101..109u64 {
        let x = signal(
            &[Tone::silence(30.0)],
            24e3,
            0.0,
            &ch(Some(0.0), 0.0, 0.0, seed),
        );
        let (rx, _) = run_at(&GWFSK, &x, 24e3, 0.0, &[]);
        let rows: Vec<(String, String)> = sender_rows(rx.as_ref())
            .into_iter()
            .filter(|r| r.0 != "Unknown")
            .collect();
        assert!(rows.is_empty(), "seed {seed}: {rows:?}");
    }
}

#[test]
fn the_generator_sends_the_content_setting() {
    let set = [(
        "gen_content",
        json!("marker 9C; text 2/106 0 A1B2E3; frame200 00112233445566778899aabbccdd"),
    )];
    let x = test_signal(&GWFSK, 24e3, &set, 6.0);
    let x = through(&x, 24e3, &ch(Some(20.0), 0.0, 0.0, 3));
    let (rx, _) = run_at(&GWFSK, &x, 24e3, 0.0, &[]);
    let rows = sender_rows(rx.as_ref());
    assert!(
        texts_of(&rows, "Channel marker")
            .iter()
            .any(|t| t.contains("0x9c")),
        "{rows:?}"
    );
    assert!(
        texts_of(&rows, "Packet")
            .iter()
            .any(|t| t.ends_with("final packet A1B2E3")),
        "{rows:?}"
    );
    assert!(
        texts_of(&rows, "Frame")
            .iter()
            .any(|t| t.starts_with("14 bytes: 00 11 22 33 44 55 66 77 88 99 aa bb cc dd")),
        "{rows:?}"
    );
    let values = values(&GWFSK, &[("gen_content", json!("marker FA; nothing"))]);
    let made = (GWFSK.make_generator.unwrap())(&sdrkit_core::GenConfig::new(24e3, values));
    assert!(made.is_err());
}
