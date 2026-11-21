//! `fec.bch_decode`, `fec.ldpc_decode` and `fec.turbo_decode`: code words from the plain encoders (DVB-S2 short frames, the
//! FT8 code, LTE turbo blocks with their CRC), errors added, decoded through the components with exact bits, packets and status;
//! frame starts, any cut, and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, wsjt};
use sdrkit_algo_fec::ldpc::{InfoAt, LdpcCode};
use sdrkit_algo_fec::turbo::Turbo;
use sdrkit_components::fec::{BchDecode, LdpcDecode, TurboDecode};
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, TagKind};
use sdrkit_util_bits::crc::catalog::CRC24_LTE_B;

/// DVB-S2 short frame rate 1/2 BCH (EN 302 307-1 table 5b: N_bch 7200, K_bch 7032, t 12).
fn bch_frames(rng: &mut Rng, frames: usize, errors: usize) -> (Vec<u8>, Vec<Vec<u8>>) {
    let bch = Bch::new(14, 0x402B, 7200, 12).unwrap();
    assert_eq!(bch.k(), 7032);
    let mut stream = Vec::new();
    let mut infos = Vec::new();
    for _ in 0..frames {
        let mut w = rng.bits(7200);
        bch.encode(&mut w).unwrap();
        infos.push(w[..7032].to_vec());
        for p in rng.positions(7200, errors) {
            w[p] ^= 1;
        }
        stream.extend_from_slice(&w);
    }
    (stream, infos)
}

#[test]
fn bch_corrects_twelve_errors_from_hard_bits_or_llrs_for_any_cut() {
    let mut rng = Rng(1);
    let (stream, infos) = bch_frames(&mut rng, 3, 12);
    for (input, fmt) in [
        (Data::HardBits(stream.clone()), Format::hard_bits(4.0)),
        (Data::Llr(llr_of(&stream, 9)), Format::llr(4.0, None)),
    ] {
        for per_pass in [1000usize, 7200, 30_000] {
            let mut g = Tb::new()
                .add(
                    "src",
                    Source::new(fmt.clone(), input_clone(&input), per_pass),
                )
                .add("bch", BchDecode::default())
                .connect("src", "bch")
                .set("bch.code", txt("dvb_s2_short_1_2"))
                .build()
                .unwrap();
            let got = run(
                &mut g,
                stream.len() / per_pass + 2,
                &["bch.out", "bch.bits", "bch.status"],
            );
            let want_packets: Vec<Vec<u8>> = infos.iter().map(|b| pack(b)).collect();
            assert_eq!(packets(&got[0].data), want_packets, "pieces of {per_pass}");
            assert_eq!(hard_bits(&got[1].data), infos.concat());
            assert!(got[2].status.iter().all(|s| s.ok && s.corrected == 12));
            assert_eq!(got[2].status.len(), 3);
            assert_eq!(field(&g, "bch", "bits_corrected"), 36.0);
            let starts: Vec<u64> = got[1]
                .tags
                .iter()
                .filter(|t| matches!(t.kind, TagKind::FrameStart { .. }))
                .map(|t| t.at)
                .collect();
            assert_eq!(starts, [0, 7032, 14_064]);
        }
    }
}

fn input_clone(d: &Data) -> Data {
    match d {
        Data::HardBits(b) => Data::HardBits(b.clone()),
        Data::Llr(b) => Data::Llr(b.clone()),
        _ => unreachable!(),
    }
}

#[test]
fn bch_by_a_catalogue_name_sarsat_bch1_from_t001() {
    // C/S T.001 Annex B: PDF-1 and BCH-1 of the sample short message (bits 25 to 106), with 3 errors.
    let msg: Vec<u8> = "56E6804002202009655250"
        .chars()
        .flat_map(|c| {
            let d = c.to_digit(16).unwrap() as u8;
            (0..4).rev().map(move |i| (d >> i) & 1)
        })
        .collect();
    let mut word = msg[..82].to_vec();
    for p in [0, 40, 81] {
        word[p] ^= 1;
    }
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::hard_bits(4.0), Data::HardBits(word), 82),
        )
        .add("bch", BchDecode::default())
        .connect("src", "bch")
        .set("bch.code", txt("sarsat_82_61"))
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["bch.bits", "bch.status"]);
    assert_eq!(hard_bits(&got[0].data), msg[..61]);
    assert!(got[1].status[0].ok);
    assert_eq!(got[1].status[0].corrected, 3);
    // Every catalogue name is a choice.
    for c in sdrkit_components::fec::bch_codes() {
        let mut d = BchDecode::default();
        assert!(
            set(&mut d, "code", &Value::Text(c.key.into())).is_ok(),
            "{}",
            c.key
        );
    }
}

#[test]
fn bch_beyond_its_limit_is_reported_and_can_be_dropped() {
    let mut rng = Rng(2);
    let (stream, infos) = bch_frames(&mut rng, 2, 0);
    let mut stream = stream;
    // Frame 0: 40 errors (far beyond t = 12; BCH detects this with overwhelming likelihood).
    for p in rng.positions(7200, 40) {
        stream[p] ^= 1;
    }
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::hard_bits(4.0), Data::HardBits(stream), 7200),
        )
        .add("bch", BchDecode::default())
        .connect("src", "bch")
        .set("bch.code", txt("dvb_s2_short_1_2"))
        .set("bch.drop_failed", Value::Bool(true))
        .build()
        .unwrap();
    let got = run(&mut g, 3, &["bch.out", "bch.status"]);
    assert_eq!(packets(&got[0].data), [pack(&infos[1])]);
    assert!(!got[1].status[0].ok && got[1].status[1].ok);
    assert_eq!(field(&g, "bch", "frames_bad"), 1.0);
    assert_eq!(field(&g, "bch", "frames_ok"), 1.0);
}

#[test]
fn dvb_s2_short_ldpc_frames_after_junk_and_a_frame_start() {
    let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
    let (n, k) = (code.n(), code.k());
    let mut rng = Rng(3);
    let mut stream = vec![5i8; 333];
    let mut infos = Vec::new();
    for _ in 0..2 {
        let info = rng.bits(k);
        let mut cw = vec![0u8; n];
        code.encode(&info, &mut cw).unwrap();
        let mut llr = llr_of(&cw, 12);
        for p in rng.positions(n, 300) {
            llr[p] = -llr[p];
        }
        stream.extend_from_slice(&llr);
        infos.push(info);
    }
    for per_pass in [999usize, 16_200, 50_000] {
        let mut g = Tb::new()
            .add(
                "src",
                TaggedSource::new(
                    Format::llr(4.0, Some(4.0)),
                    Data::Llr(stream.clone()),
                    &[333],
                    per_pass,
                ),
            )
            .add("ldpc", LdpcDecode::default())
            .connect("src", "ldpc")
            .set("ldpc.code", txt("dvb_s2_short_1_2"))
            .build()
            .unwrap();
        let got = run(
            &mut g,
            stream.len() / per_pass + 2,
            &["ldpc.out", "ldpc.bits", "ldpc.status"],
        );
        assert_eq!(
            hard_bits(&got[1].data),
            infos.concat(),
            "pieces of {per_pass}"
        );
        assert_eq!(
            packets(&got[0].data),
            infos.iter().map(|b| pack(b)).collect::<Vec<_>>()
        );
        assert!(got[2]
            .status
            .iter()
            .all(|s| s.ok && s.corrected == 300 && s.iterations > 1));
        assert!(field(&g, "ldpc", "iterations") > 1.0);
        assert_eq!(
            g.format("ldpc.bits").unwrap().rate,
            4.0 * k as f64 / n as f64
        );
    }
}

#[test]
fn ft8_by_sum_product_needs_the_llr_scale() {
    let code = LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap();
    let mut rng = Rng(4);
    let info = rng.bits(91);
    let mut cw = vec![0u8; 174];
    code.encode(&info, &mut cw).unwrap();
    let mut llr = llr_of(&cw, 16);
    for p in rng.positions(174, 8) {
        llr[p] = -llr[p];
    }
    let build = |scale: Option<f32>| {
        Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, scale), Data::Llr(llr.clone()), 50),
            )
            .add("ldpc", LdpcDecode::default())
            .connect("src", "ldpc")
            .set("ldpc.code", txt("ft8"))
            .set("ldpc.algo", txt("sum_product"))
            .build()
            .unwrap()
    };
    let mut g = build(Some(4.0));
    let got = run(&mut g, 5, &["ldpc.bits", "ldpc.status"]);
    assert_eq!(hard_bits(&got[0].data), info);
    assert!(got[1].status[0].ok);
    let mut g = build(None);
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
}

/// LTE code blocks of `k` bits ending in their CRC-24B, encoded, with `flips` hard errors among the 3 (k + 4) values.
fn turbo_blocks(rng: &mut Rng, k: usize, blocks: usize, flips: usize) -> (Vec<i8>, Vec<Vec<u8>>) {
    let t = Turbo::new(k).unwrap();
    let mut stream = Vec::new();
    let mut infos = Vec::new();
    for _ in 0..blocks {
        let mut bits = rng.bits(k - 24);
        CRC24_LTE_B.append_bits(&mut bits);
        assert!(CRC24_LTE_B.verify_bits(&bits));
        let mut coded = vec![0u8; 3 * (k + 4)];
        t.encode(&bits, &mut coded).unwrap();
        let mut llr = llr_of(&coded, 20);
        for p in rng.positions(llr.len(), flips) {
            llr[p] = -llr[p];
        }
        stream.extend_from_slice(&llr);
        infos.push(bits);
    }
    (stream, infos)
}

#[test]
fn lte_turbo_blocks_stop_on_their_crc_for_any_cut() {
    let mut rng = Rng(5);
    let (stream, infos) = turbo_blocks(&mut rng, 1024, 3, 60);
    for per_pass in [100usize, 3084, 10_000] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(Format::llr(4.0, None), Data::Llr(stream.clone()), per_pass),
            )
            .add("t", TurboDecode::default())
            .connect("src", "t")
            .set("t.k", Value::F64(1024.0))
            .set("t.stop", txt("crc24b"))
            .build()
            .unwrap();
        let got = run(
            &mut g,
            stream.len() / per_pass + 2,
            &["t.out", "t.bits", "t.status"],
        );
        assert_eq!(
            hard_bits(&got[1].data),
            infos.concat(),
            "pieces of {per_pass}"
        );
        assert_eq!(
            packets(&got[0].data),
            infos.iter().map(|b| pack(b)).collect::<Vec<_>>()
        );
        assert!(got[2].status.iter().all(|s| s.ok && s.iterations <= 8));
    }
    // Stopping when the decisions settle produces the same bits.
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::llr(4.0, None), Data::Llr(stream.clone()), 5000),
        )
        .add("t", TurboDecode::default())
        .connect("src", "t")
        .set("t.k", Value::F64(1024.0))
        .set("t.stop", txt("stable"))
        .build()
        .unwrap();
    let got = run(&mut g, 3, &["t.bits"]);
    assert_eq!(hard_bits(&got[0].data), infos.concat());
}

#[test]
fn a_block_whose_crc_never_passes_is_failed() {
    let mut rng = Rng(6);
    let (mut stream, _) = turbo_blocks(&mut rng, 40, 1, 0);
    // Random values: no code word.
    for v in stream.iter_mut() {
        *v = if rng.below(2) == 0 { 20 } else { -20 };
    }
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::llr(4.0, None), Data::Llr(stream), 1000),
        )
        .add("t", TurboDecode::default())
        .connect("src", "t")
        .set("t.k", Value::F64(40.0))
        .set("t.max_iter", Value::F64(3.0))
        .build()
        .unwrap();
    let got = run(&mut g, 2, &["t.status"]);
    assert_eq!(got[0].status.len(), 1);
    assert!(!got[0].status[0].ok);
    assert_eq!(got[0].status[0].iterations, 3);
}

#[test]
fn settings_are_strict() {
    let mut t = TurboDecode::default();
    assert!(set(&mut t, "k", &Value::F64(41.0)).is_err());
    assert!(set(&mut t, "k", &Value::F64(6152.0)).is_err());
    assert!(set(&mut t, "stop", &Value::Text("crc16".into())).is_err());
    assert!(set(&mut t, "max_iter", &Value::F64(0.0)).is_err());
    let mut l = LdpcDecode::default();
    assert!(set(&mut l, "code", &Value::Text("dvb_s2_short_9_10".into())).is_err());
    assert!(set(&mut l, "algo", &Value::Text("bit_flip".into())).is_err());
    assert!(set(&mut l, "min_sum_scale", &Value::F64(0.3)).is_err());
    assert!(set(&mut l, "min_sum_scale", &Value::F64(0.75)).is_ok());
    assert!(set(&mut l, "max_iter", &Value::F64(1000.0)).is_err());
    let mut b = BchDecode::default();
    assert!(set(&mut b, "code", &Value::Text("bch_63_51".into())).is_err());
    assert!(set(&mut b, "t", &Value::F64(8.0)).is_err());
    // A lifting size not in TS 38.212 table 5.3.2-1 is refused when the graph is configured.
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::llr(4.0, None), Data::Llr(vec![]), 1),
        )
        .add("ldpc", LdpcDecode::default())
        .connect("src", "ldpc")
        .set("ldpc.code", txt("nr_bg2"))
        .set("ldpc.lifting", Value::F64(17.0))
        .build()
        .unwrap();
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
}
