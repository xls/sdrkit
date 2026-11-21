//! The framing components in small graphs: packing, HDLC, CRC checks and byte descrambling, each against a known answer from its
//! standard, cut into pieces of several sizes, and with their settings refused when bad.

mod bits_util;

use bits_util::*;
use sdrkit_components::bits::CrcStatus;
use sdrkit_core::{Complex32, Output, SettingError, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Kind, Packet, Timed};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::{hdlc_encode, stuff_bits, FLAG};
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::line::Nrzi;
use sdrkit_util_bits::{pack, seq, BitOrder};
use std::sync::Arc;

fn txt(s: &str) -> Value {
    Value::Text(s.into())
}

fn num(x: f64) -> Value {
    Value::F64(x)
}

fn bytes_format() -> Format {
    Format::new(Kind::Bytes, 0.0)
}

fn packet(b: &[u8]) -> Packet {
    Packet::new(Arc::from(b.to_vec()), 0.0)
}

fn msb(bytes: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    pack::unpack(bytes, BitOrder::Msb, &mut v);
    v
}

// ----- bits.pack -----

#[test]
fn pack_makes_bytes_in_either_order() {
    let bytes = [0xA5u8, 0x3C, 0x01, 0xFE, 0x7E, 0x80];
    let bits = msb(&bytes);
    for &piece in PIECES {
        let r = run_chain(
            Format::hard_bits(1000.0),
            Data::HardBits(bits.clone()),
            piece,
            &[("p", "bits.pack")],
            &[("p.frame_bits", num(16.0)), ("p.start", txt("continuous"))],
            "p.out",
        );
        assert_eq!(
            packets(&r.data),
            vec![vec![0xA5, 0x3C], vec![0x01, 0xFE], vec![0x7E, 0x80]],
            "piece {piece}"
        );
        let Data::Bytes(p) = &r.data else { panic!() };
        // A packet is timed at its first bit.
        assert_eq!(p[1].time, 16.0 / 1000.0);
        let r = run_chain(
            Format::hard_bits(1000.0),
            Data::HardBits(bits.clone()),
            piece,
            &[("p", "bits.pack")],
            &[
                ("p.frame_bits", num(24.0)),
                ("p.start", txt("continuous")),
                ("p.order", txt("lsb")),
            ],
            "p.out",
        );
        let rev: Vec<u8> = bytes.iter().map(|b| b.reverse_bits()).collect();
        assert_eq!(packets(&r.data), vec![rev[..3].to_vec(), rev[3..].to_vec()]);
    }
    // A frame that is not whole bytes is padded with zeros.
    let r = run_chain(
        Format::hard_bits(1000.0),
        Data::HardBits(bits.clone()),
        5,
        &[("p", "bits.pack")],
        &[("p.frame_bits", num(12.0)), ("p.start", txt("continuous"))],
        "p.out",
    );
    assert_eq!(packets(&r.data)[0], vec![0xA5, 0x30]);
}

#[test]
fn pack_takes_frames_from_sync_words() {
    // Two CCSDS-style frames of 4 bytes after the attached sync marker, with stray bits around them.
    let asm = msb(&[0x1A, 0xCF, 0xFC, 0x1D]);
    let mut line = random_bits(19, 1);
    line.extend(&asm);
    line.extend(msb(&[1, 2, 3, 4]));
    line.extend(random_bits(9, 2));
    line.extend(&asm);
    line.extend(msb(&[5, 6, 7, 8]));
    line.extend(random_bits(3, 3));
    for &piece in PIECES {
        let r = run_chain(
            Format::hard_bits(1000.0),
            Data::HardBits(line.clone()),
            piece,
            &[("s", "bits.sync"), ("p", "bits.pack")],
            &[("p.frame_bits", num(32.0))],
            "p.out",
        );
        assert_eq!(
            packets(&r.data),
            vec![vec![1, 2, 3, 4], vec![5, 6, 7, 8]],
            "piece {piece}"
        );
    }
    // With no frame length, a frame runs from one sync word's end to the next one's (the next word included).
    let r = run_chain(
        Format::hard_bits(1000.0),
        Data::HardBits(line.clone()),
        11,
        &[("s", "bits.sync"), ("p", "bits.pack")],
        &[("p.frame_bits", num(0.0))],
        "p.out",
    );
    let p = packets(&r.data);
    assert_eq!(p.len(), 1);
    assert_eq!(&p[0][..4], &[1, 2, 3, 4]);
    assert_eq!(p[0].len(), (32 + 9 + 32usize).div_ceil(8));
}

// ----- bits.hdlc -----

/// An AX.25 UI frame: destination and source addresses (callsigns shifted left one bit), control 0x03, PID 0xF0, information.
fn ax25(dest: &str, src: &str, info: &[u8]) -> Vec<u8> {
    let mut f = Vec::new();
    for (k, call) in [dest, src].iter().enumerate() {
        let mut c = format!("{call:<6}").into_bytes();
        c.truncate(6);
        f.extend(c.iter().map(|b| b << 1));
        f.push(if k == 1 { 0x61 } else { 0x60 });
    }
    f.extend([0x03, 0xF0]);
    f.extend(info);
    f
}

#[test]
fn hdlc_finds_ax25_frames_after_nrzi() {
    let a = ax25("APRS", "N0CALL", b"!4903.50N/07201.75W-Test 1");
    let b = ax25("CQ", "VK2XYZ", &[0x7E, 0x7D, 0xFF, 0xFF, 0x1F]); // flags and ones inside need stuffing
    let mut bits = Vec::new();
    hdlc_encode(&[&a, &b], &CRC16_IBM_SDLC, 4, &mut bits);
    bits.extend(
        std::iter::repeat_n(0u8, 400)
            .enumerate()
            .map(|(i, _)| (FLAG >> (i % 8)) & 1),
    ); // idle flags
    let mut line = bits.clone();
    Nrzi::new(0).encode(&mut line);
    for &piece in PIECES {
        let mut g = chain(
            Format::hard_bits(1200.0),
            Data::HardBits(line.clone()),
            piece,
            &[("n", "bits.nrzi"), ("h", "bits.hdlc")],
            &[("h.fcs", txt("crc16_ibm_sdlc"))],
        );
        let r = run(&mut g, line.len() / piece + 3, "h.out");
        assert_eq!(
            packets(&r.data),
            vec![a.clone(), b.clone()],
            "piece {piece}"
        );
        assert_eq!(result_i64(&g, "h", "frames_ok"), 2);
        assert_eq!(result_i64(&g, "h", "frames_bad"), 0);
    }
}

#[test]
fn hdlc_checks_the_x25_fcs_check_value_and_counts_bad_frames() {
    // "123456789" followed by its CRC-16/IBM-SDLC, 0x906E, low byte first, as HDLC sends it: built by hand from the check value.
    let mut frame = b"123456789".to_vec();
    frame.extend([0x6E, 0x90]);
    let mut bits = Vec::new();
    let flag = |out: &mut Vec<u8>| pack::unpack(&[FLAG], BitOrder::Lsb, out);
    flag(&mut bits);
    let mut body = Vec::new();
    pack::unpack(&frame, BitOrder::Lsb, &mut body);
    let mut ones = 0;
    stuff_bits(&body, &mut ones, &mut bits);
    flag(&mut bits);
    let closed = bits.len() - 1;
    // The same frame with one bit wrong, and a frame too short for min_len.
    let mut wrong = frame.clone();
    wrong[2] ^= 0x10;
    let mut body = Vec::new();
    pack::unpack(&wrong, BitOrder::Lsb, &mut body);
    let mut ones = 0;
    stuff_bits(&body, &mut ones, &mut bits);
    flag(&mut bits);
    hdlc_encode(&[b"abc"], &CRC16_IBM_SDLC, 1, &mut bits);
    for _ in 0..40 {
        flag(&mut bits);
    }
    let mut g = chain(
        Format::hard_bits(1200.0),
        Data::HardBits(bits.clone()),
        97,
        &[("h", "bits.hdlc")],
        &[("h.min_len", num(4.0))],
    );
    let r = run(&mut g, bits.len() / 97 + 3, "h.out");
    assert_eq!(packets(&r.data), vec![b"123456789".to_vec()]);
    assert_eq!(result_i64(&g, "h", "frames_ok"), 1);
    assert_eq!(result_i64(&g, "h", "frames_bad"), 1);
    // A packet is timed at the bit that closed the frame.
    let Data::Bytes(p) = &r.data else { panic!() };
    assert_eq!(p[0].time, closed as f64 / 1200.0);
}

// ----- bytes.crc -----

/// Runs packets through `bytes.crc` and returns the packets out and the statuses.
fn crc_run(input: &[Vec<u8>], settings: &[(&str, Value)]) -> (Vec<Vec<u8>>, Vec<bool>, i64, i64) {
    let pk: Vec<Packet> = input.iter().map(|b| packet(b)).collect();
    let mut g = chain(
        bytes_format(),
        Data::Bytes(pk),
        2,
        &[("c", "bytes.crc")],
        settings,
    );
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let (mut out, mut status) = (Vec::new(), Vec::new());
    let mut o = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..input.len() + 2 {
        o.clear();
        g.process(&empty, &mut o);
        out.extend(packets(&g.buffer("c.out").unwrap().data));
        if let Data::Items(Some(v)) = &g.buffer("c.status").unwrap().data {
            let v = v.as_any().downcast_ref::<Vec<Timed<CrcStatus>>>().unwrap();
            status.extend(v.iter().map(|s| s.value.good));
        }
    }
    let ok = result_i64(&g, "c", "frames_ok");
    let bad = result_i64(&g, "c", "frames_bad");
    (out, status, ok, bad)
}

#[test]
fn crc_check_accepts_the_published_check_values() {
    // CRC-16/IBM-SDLC of "123456789" is 0x906E, sent low byte first.
    let mut good = b"123456789".to_vec();
    good.extend([0x6E, 0x90]);
    let mut bad = good.clone();
    bad[0] ^= 1;
    let (out, status, ok, nok) = crc_run(&[good.clone(), bad.clone(), good.clone()], &[]);
    assert_eq!(out, vec![b"123456789".to_vec(), b"123456789".to_vec()]);
    assert_eq!(status, vec![true, true]);
    assert_eq!((ok, nok), (2, 1));
    // Marked rather than dropped, and the CRC kept.
    let (out, status, ..) = crc_run(
        &[good.clone(), bad.clone()],
        &[("c.action", txt("mark")), ("c.strip", Value::Bool(false))],
    );
    assert_eq!(out, vec![good.clone(), bad]);
    assert_eq!(status, vec![true, false]);
    // CRC-32/MPEG-2 of "123456789" is 0x0376E6E7, sent most significant byte first.
    let mut mpeg = b"123456789".to_vec();
    mpeg.extend([0x03, 0x76, 0xE6, 0xE7]);
    let (out, _, ok, _) = crc_run(&[mpeg.clone()], &[("c.crc", txt("crc32_mpeg_2"))]);
    assert_eq!((out, ok), (vec![b"123456789".to_vec()], 1));
    // The wrong byte order is a bad CRC; a packet shorter than the CRC is bad.
    let (out, _, ok, nok) = crc_run(
        &[mpeg, vec![1, 2]],
        &[("c.crc", txt("crc32_mpeg_2")), ("c.trailer", txt("little"))],
    );
    assert_eq!((out.len(), ok, nok), (0, 0, 2));
}

// ----- bytes.descramble -----

fn descramble_packets(
    input: Vec<Vec<u8>>,
    piece: usize,
    settings: &[(&str, Value)],
) -> Vec<Vec<u8>> {
    let pk: Vec<Packet> = input.iter().map(|b| packet(b)).collect();
    let r = run_chain(
        bytes_format(),
        Data::Bytes(pk),
        piece,
        &[("d", "bytes.descramble")],
        settings,
        "d.out",
    );
    packets(&r.data)
}

#[test]
fn ccsds_byte_randomiser_returns_its_published_bytes() {
    // CCSDS 131.0-B clause 10: FF 48 0E C0 9A ...
    let out = descramble_packets(
        vec![vec![0; 300], vec![0; 10]],
        1,
        &[("d.sequence", txt("ccsds"))],
    );
    assert_eq!(&out[0][..5], &[0xFF, 0x48, 0x0E, 0xC0, 0x9A]);
    assert_eq!(&out[0][..255], &seq::CCSDS.bytes()[..]);
    assert_eq!(&out[0][255..], &seq::CCSDS.bytes()[..45]);
    // Each packet starts the sequence again; with a period of 2, the second packet goes on from the first.
    assert_eq!(&out[1][..], &seq::CCSDS.bytes()[..10]);
    let out = descramble_packets(vec![vec![0; 10], vec![0; 10]], 1, &[("d.period", num(2.0))]);
    assert_eq!(&out[1][..], &seq::CCSDS.bytes()[10..20]);
}

#[test]
fn dvb_energy_dispersal_over_eight_transport_packets() {
    // EN 300 421 clause 4.4.1: the PRBS restarts every 8 packets; the first sync byte is inverted (0xB8) and not scrambled, and
    // the sequence runs on unused through the other seven sync bytes.
    let mut tsp = Vec::new();
    for k in 0..9 {
        let mut p = vec![0u8; 188];
        p[0] = if k % 8 == 0 { 0xB8 } else { 0x47 };
        tsp.push(p);
    }
    let mut prbs = vec![0u8; 8 * 188 - 1];
    Additive::new(seq::DVB_DISPERSAL).apply_bytes(&mut prbs, BitOrder::Msb);
    let set = [
        ("d.sequence", txt("dvb_dispersal")),
        ("d.period", num(8.0)),
        ("d.skip", num(1.0)),
    ];
    for &piece in &[1, 3, 9] {
        let out = descramble_packets(tsp.clone(), piece, &set);
        assert_eq!(&out[0][..3], &[0xB8, 0x03, 0xF6], "piece {piece}");
        for k in 0..8 {
            assert_eq!(out[k][0], tsp[k][0]);
            assert_eq!(&out[k][1..], &prbs[188 * k..188 * k + 187], "packet {k}");
        }
        assert_eq!(out[8], out[0]);
    }
}

// ----- strict settings -----

#[test]
fn bad_settings_are_refused() {
    let r = sdrkit_components::registry();
    let cases: &[(&str, &str, Value)] = &[
        ("bits.pack", "order", txt("big")),
        ("bits.pack", "frame_bits", num(-8.0)),
        ("bits.pack", "start", txt("sometimes")),
        ("bits.pack", "max_bits", num(4.0)),
        ("bits.hdlc", "fcs", txt("CRC-16/IBM-SDLC")), // the key, not the catalogue name
        ("bits.hdlc", "fcs", txt("crc32_iso_hdlc")),  // not 16 bits
        ("bits.hdlc", "min_len", num(1000.0)),        // above max_len
        ("bits.hdlc", "max_len", num(0.0)),
        ("bytes.crc", "crc", txt("crc12_dect")), // not whole bytes
        ("bytes.crc", "trailer", txt("middle")),
        ("bytes.crc", "action", txt("repair")),
        ("bytes.crc", "strip", txt("perhaps")),
        ("bytes.descramble", "sequence", txt("dvb")),
        ("bytes.descramble", "period", num(0.0)),
        ("bytes.descramble", "skip", num(1.5)),
    ];
    for (name, key, bad) in cases {
        let mut c = r.make(name).unwrap();
        assert!(c.info().settings.id(key).is_ok(), "{name} has no {key}");
        assert!(set(&mut *c, key, bad).is_err(), "{name}.{key} took {bad:?}");
    }
    for name in ["bits.pack", "bits.hdlc", "bytes.crc", "bytes.descramble"] {
        let mut c = r.make(name).unwrap();
        assert!(
            matches!(
                set(&mut *c, "colour", &num(1.0)),
                Err(SettingError::Unknown { .. })
            ),
            "{name}"
        );
    }
    // Every choice a component lists is one it takes.
    for name in [
        "bits.hdlc",
        "bytes.crc",
        "bytes.descramble",
        "bits.descramble",
        "bits.deinterleave",
    ] {
        let mut c = r.make(name).unwrap();
        let schema = c.info().settings;
        for s in schema.settings() {
            for v in s.field.choices.iter() {
                // A kind that needs other parameters first may refuse; every other choice is taken.
                let res = set(&mut *c, &s.field.key, &txt(&v.key));
                assert!(
                    res.is_ok() || s.field.key == "kind",
                    "{name}.{} refused its own choice {}",
                    s.field.key,
                    v.key
                );
            }
        }
    }
}
