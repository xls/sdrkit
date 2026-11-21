//! Inmarsat BGAN: the 118 published tables, the bearers and their frames, every table through the turbo code and back, the
//! scrambler, flush, CRC and PDU worked values, each bearer alone at 6 dB over its Annex B C/N0, and a group capture.

use crate::common::{cell, field_text, rows, values};
use sdrkit_algo_fec::turbo::RSC_SL;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Receiver, StackConfig};
use sdrkit_stack_satcom::modes::bgan::codec::{encode_block, BlockDecoder, IqLabels};
use sdrkit_stack_satcom::modes::bgan::pdu::{self, Avp, Bulletin, Sdu};
use sdrkit_stack_satcom::modes::bgan::tables::{self, Modulation, BEARERS, UW_LEN};
use sdrkit_stack_satcom::modes::bgan::{self, gen};
use sdrkit_util_bits::lfsr::Lfsr;
use std::collections::BTreeSet;

const SATELLITE_FILE: &str = include_str!("../../../../../data/links/inmarsat_satellites.csv");

/// Acceptance test 1: the 118 forward tables load and are what Annex C publishes.
#[test]
fn the_118_forward_tables_load_and_check() {
    let t = tables::tables();
    assert_eq!(t.len(), 118);
    for x in t {
        let n = x.n;
        let mut seen = vec![false; n];
        for &v in &x.tci {
            assert!(!seen[v as usize], "{}: interleaver repeats", x.name);
            seen[v as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "{}: not a permutation", x.name);
        let family = BEARERS
            .iter()
            .flat_map(|b| [b.family, b.qam_family])
            .flatten()
            .find(|f| x.name.starts_with(f.prefix) && x.name.len() - f.prefix.len() <= 5)
            .expect("a family");
        let teo = BEARERS
            .iter()
            .find(|b| b.family == Some(family) || b.qam_family == Some(family))
            .unwrap()
            .teo;
        assert_eq!(x.symbols, teo, "{}: rows", x.name);
        assert_eq!(x.bits, family.modulation.bits(), "{}", x.name);
        let mut used = BTreeSet::new();
        for &v in &x.cipm {
            assert!((v as usize) < 3 * n, "{}: index {v}", x.name);
            assert!(used.insert(v), "{}: {v} twice", x.name);
        }
        assert!((0..n as u32).all(|d| used.contains(&d)), "{}: a data bit is not sent", x.name);
        assert_eq!(n + 2 * x.parity_sent(), x.symbols * x.bits, "{}", x.name);
    }
    // Rows quoted from the published files.
    let by = |name: &str| t.iter().find(|x| x.name == name).unwrap();
    let h1 = by("F80T45XH1");
    assert_eq!((h1.n, h1.parity_sent()), (3444, 1270));
    assert_eq!(&h1.tci[..4], &[34, 2236, 974, 3200]);
    assert_eq!(&h1.cipm[..8], &[0, 814, 2129, 2943, 1628, 6885, 313, 6889]);
    let l8 = by("F80T025QL8");
    assert_eq!((l8.n, l8.symbols), (364, 544));
    assert_eq!(&l8.tci[..4], &[3, 235, 104, 338]);
    assert_eq!(&l8.cipm[..4], &[727, 729, 0, 587]);
    let x64 = by("FR80T25X64L3");
    assert_eq!(&x64.cipm[..6], &[0, 1442, 5210, 1163, 721, 3769]);
    assert_eq!(by("FR80T5X64H6").n, 5284);
    assert_eq!(by("F80T1QRE_4B").parity_sent(), 82);
}

/// Acceptance test 2: the 13 bearers, their frames of 80 ms and their layout; the constellations.
#[test]
fn bearers_frames_and_constellations() {
    assert_eq!(BEARERS.len(), 13);
    let tails = [16, 8, 8, 8, 20, 20, 56, 77, 0, 50, 50, 80, 24];
    for (b, tail) in BEARERS.iter().zip(tails) {
        assert_eq!(b.mos, UW_LEN + b.ps + b.teo * b.blocks, "{}", b.name);
        assert!((b.mos as f64 / b.rate - 0.08).abs() < 1e-9, "{}", b.name);
        assert_eq!(b.tail(), tail, "{}", b.name);
        let mut kind = vec![0u8; b.mos];
        for k in 0..b.ps {
            kind[b.pilot_position(k)] += 1;
        }
        for d in 0..b.data_symbols() {
            kind[b.data_position(d)] += 2;
        }
        assert!(kind[..UW_LEN].iter().all(|&k| k == 0), "{}", b.name);
        assert_eq!(kind.iter().filter(|&&k| k == 1).count(), b.ps, "{}", b.name);
        assert_eq!(kind.iter().filter(|&&k| k == 2).count(), b.teo * b.blocks, "{}", b.name);
        let mut cells = BTreeSet::new();
        for d in 0..b.data_symbols() {
            assert!(cells.insert(b.block_symbol(d)), "{}", b.name);
        }
    }
    assert_eq!(tables::bearer_by_name("F80T0.25Q-1B").unwrap().data_position(528), 656);
    for m in [Modulation::Qpsk, Modulation::Qam16, Modulation::Qam32, Modulation::Qam64] {
        let p = m.points();
        assert_eq!(p.len(), 1 << m.bits());
        let e: f64 = p.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / p.len() as f64;
        assert!((e - 1.0).abs() < 1e-6, "{m:?}: {e}");
    }
    let d2 = 0.1f32.sqrt();
    let q16 = Modulation::Qam16.points();
    assert!((q16[0b0101] - Complex32::new(-3.0 * d2, -3.0 * d2)).norm() < 1e-6);
    assert!((q16[0b1010] - Complex32::new(d2, d2)).norm() < 1e-6);
    let q64 = Modulation::Qam64.points();
    let d64 = (1.0f32 / 42.0).sqrt();
    assert!((q64[1] - Complex32::new(3.0 * d64, d64)).norm() < 1e-6);
    assert!((q64[63] - Complex32::new(-7.0 * d64, -7.0 * d64)).norm() < 1e-6);
    assert_eq!(tables::UNIQUE_WORDS[9], 0x51_4BB8_BA62);
    assert_eq!(tables::uw_bit(0, 0, true), 1);
    assert_eq!(tables::uw_bit(0, 0, false), 1);
    assert_eq!(tables::uw_bit(0, 1, true), 1);
    assert_eq!(tables::uw_bit(0, 1, false), 0);
}

/// Acceptance test 3: a random block through every one of the 118 tables and back, without noise.
#[test]
fn every_table_decodes_its_own_block() {
    let mut rng = Rng::new(5);
    for (i, t) in tables::tables().iter().enumerate() {
        for labels in [IqLabels::InOrder, IqLabels::Swapped] {
            let data: Vec<u8> = (0..t.k() / 8).map(|_| rng.next_u32() as u8).collect();
            let syms = encode_block(t, &data, labels);
            let m = match t.bits {
                2 => Modulation::Qpsk,
                4 => Modulation::Qam16,
                5 => Modulation::Qam32,
                _ => Modulation::Qam64,
            };
            let points = m.points();
            let mut d = BlockDecoder::new(i, labels).unwrap();
            for (s, &v) in syms.iter().enumerate() {
                d.put(s, points[usize::from(v)], 100.0);
            }
            let want = data.clone();
            let got = d.decode(8, |o| o == want.as_slice()).map(<[u8]>::to_vec);
            assert_eq!(got.as_deref(), Some(data.as_slice()), "{} {labels:?}", t.name);
        }
    }
}

/// Acceptance test 4: the scrambler, the flush table and its example, the CRC, the worked PDU and AVPs.
#[test]
fn worked_values() {
    let mut s: Lfsr = sdrkit_util_bits::seq::BGAN;
    let first = (0..32).fold(0u32, |a, _| (a << 1) | u32::from(s.bit()));
    assert_eq!(first, 0x131B_C425);
    // Flush table 5.11: state T1 T2 T3 T4 (T1 the newest, the top bit here).
    let flush = [
        0b0000, 0b1000, 0b1100, 0b0100, 0b0110, 0b1110, 0b1010, 0b0010, 0b0011, 0b1011, 0b1111, 0b0111, 0b0101, 0b1101,
        0b1001, 0b0001,
    ];
    for (state, f) in flush.iter().enumerate() {
        let mut st = state;
        let mut bits = 0u32;
        for _ in 0..4 {
            let u = RSC_SL.zeroing_input(st);
            bits = (bits << 1) | u32::from(u);
            st = RSC_SL.step(st, u).1;
        }
        assert_eq!((bits, st), (*f, 0), "state {state:04b}");
    }
    let mut st = 0usize;
    let mut parity = 0u32;
    for b in [1u8, 0, 1, 1, 0, 0, 1, 0] {
        let (p, n) = RSC_SL.step(st, b);
        parity = (parity << 1) | u32::from(p);
        st = n;
    }
    assert_eq!(st, 0b1011);
    for _ in 0..4 {
        let u = RSC_SL.zeroing_input(st);
        let (p, n) = RSC_SL.step(st, u);
        parity = (parity << 1) | u32::from(p);
        st = n;
    }
    assert_eq!((parity, st), (0b1101_1010_0001, 0));
    assert_eq!(
        sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC.checksum(b"123456789"),
        0x906E
    );
    let worked = [0xC9, 0x06, 0x0C, 0x07, 0x20, 0x64, 0x93, 0x2A, 0x94, 0x8C];
    assert!(pdu::pdu_check(&worked));
    let mut block = worked.to_vec();
    block.resize(45, 0);
    assert!(pdu::block_check(&block));
    let c = pdu::read_block(&block);
    assert_eq!((c.pdus_ok, c.broadcast, c.pdus_bad), (1, 1, 0));
    let want = Bulletin {
        rnc: 7,
        net_ver: 2,
        frame: 100,
        f_bearer: 1,
        bct_id: 3,
        spot: Some(42),
        avps: Vec::new(),
    };
    assert_eq!(c.sdus, vec![Sdu::Bulletin(want.clone())]);
    let made = pdu::broadcast_pdu(&pdu::sdu(1, &pdu::bulletin(&want, &[]), false));
    assert_eq!(made, worked);
    assert_eq!(pdu::avp_satellite(3, 249), [0xA9, 0x30, 0xF9]);
    assert_eq!(pdu::avp_plmn("901", "11"), [0xF2, 0x90, 0x11, 0x1F]);
    assert_eq!(
        pdu::read_avps(&[0xA9, 0x30, 0xF9, 0xF2, 0x90, 0x11, 0x1F]),
        vec![
            Avp::SatelliteLocation {
                sat: 3,
                lon: 249,
                channels: None
            },
            Avp::Plmn {
                mcc: "901".into(),
                mnc: "11".into(),
                index: None
            },
        ]
    );
    // A damaged PDU stops the block.
    let mut bad = block.clone();
    bad[4] ^= 1;
    assert!(!pdu::block_check(&bad));
    assert_eq!(pdu::read_block(&bad).pdus_bad, 1);
    assert_eq!(tables::forward_hz(tables::forward_channel(1545.0e6)), 1545.0e6);
    assert_eq!(tables::return_hz(0), None);
}

/// Feeds `seconds` of the stack's test signal with `set` to a receiver at 2 Msps tuned to `dial`.
fn run(set: &[CaseSetting], seconds: f64, dial: f64) -> Box<dyn Receiver> {
    let rate = 2e6;
    let v = values(&bgan::DESCRIPTOR, set);
    let cfg = GenConfig::new(rate, v.clone()).with_seed(3);
    let mut g = (bgan::DESCRIPTOR.make_generator.unwrap())(&cfg).unwrap();
    let mut r = (bgan::DESCRIPTOR.make_receiver)(&StackConfig::new(v)).unwrap();
    r.configure(&InputInfo::new(rate, dial, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let mut x = vec![Complex32::default(); (rate * 0.05) as usize];
    for _ in 0..(seconds / 0.05).round() as usize {
        g.generate(&mut x);
        out.clear();
        r.feed(&x, &mut out);
    }
    r
}

/// The Annex B C/N0 of each bearer at subtype R (the QPSK R of the X4/16 bearers), dB-Hz.
const NOMINAL: [f64; 13] = [
    48.3, 53.1, 53.6, 53.5, 56.95, 57.34, 60.04, 60.74, 59.6, 60.57, 60.03, 62.25, 63.95,
];

/// Acceptance test 5: each bearer at subtype R, 6 dB over its Annex B C/N0, 1.2 s, 150 Hz off: at least 8 good blocks, the
/// right bearer, the spot beam and the MCC of the test signal; F80T1Q-1B at least 8 frames in sync, structure only.
#[test]
fn each_bearer_at_r_decodes() {
    let mut failed = Vec::new();
    for (i, b) in BEARERS.iter().enumerate() {
        let set = [
            CaseSetting::new(
                &gen::GEN_BEARER,
                <gen::GenBearer as sdrkit_core::value::Choice>::from_index(i as u16 + 1).unwrap(),
            ),
            CaseSetting::new(&gen::GEN_CARRIER_CN0, NOMINAL[i] + 6.0),
            CaseSetting::new(&gen::GEN_OFFSET_HZ, 150.0),
            CaseSetting::new(&gen::GEN_SPOT, 77),
        ];
        let r = run(&set, 1.2, 1545.5e6);
        let res = r.results();
        let carriers = rows(res, sdrkit_stack_satcom::typed::BGAN_CARRIERS.key);
        let Some((_, row)) = carriers.iter().find(|(_, c)| cell(c, "kind") == "BGAN forward bearer") else {
            failed.push(format!("{}: no carrier ({})", b.name, res.status().summary));
            continue;
        };
        let num = |k: &str| cell(row, k).parse::<f64>().unwrap_or(0.0);
        if b.family.is_none() {
            if num("frames") < 8.0 || cell(row, "structure_only") != "true" || cell(row, "bearer") != b.name {
                failed.push(format!("{}: frames {} bearer {}", b.name, num("frames"), cell(row, "bearer")));
            }
            continue;
        }
        let ok = num("blocks_ok");
        // A bearer with dynamic modulation misses its 16-QAM blocks only in the first frame, before it has read the code rates.
        let dynamic_bad = b.dynamic() && num("blocks_bad") > b.blocks as f64;
        if ok < 8.0 || dynamic_bad || cell(row, "bearer") != b.name || cell(row, "spot_beam") != "77" || !cell(row, "network").starts_with("901-") {
            failed.push(format!(
                "{}: blocks {ok} bad {} bearer {} spot {} network {} cn0 {}",
                b.name,
                num("blocks_bad"),
                cell(row, "bearer"),
                cell(row, "spot_beam"),
                cell(row, "network"),
                cell(row, "cn0_dbhz")
            ));
        }
    }
    assert!(failed.is_empty(), "{failed:#?}");
}

/// Acceptance test 6: the group at 2 Msps centred on 1545.5 MHz: at least 5 carriers decoded, a satellite and a network in the
/// summary, the beam table with global and narrow beams, and the OQPSK carrier labelled GMR-2 class.
#[test]
fn group_capture() {
    let names: String = SATELLITE_FILE
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    let set = [CaseSetting::new(&bgan::SATELLITES, names.as_str())];
    let r = run(&set, 4.0, 1545.5e6);
    let res = r.results();
    let summary = res.status().summary.to_string();
    let num = |k: &str| field_text(res, k).parse::<f64>().unwrap_or(0.0);
    assert!(num("decoded") >= 5.0, "{summary}");
    assert!(summary.contains("901-11"), "{summary}");
    assert!(summary.contains("Alphasat"), "{summary}");
    let beams = rows(res, sdrkit_stack_satcom::typed::BGAN_BEAMS.key);
    assert!(!beams.is_empty());
    let carriers = rows(res, sdrkit_stack_satcom::typed::BGAN_CARRIERS.key);
    let types: BTreeSet<String> = carriers.iter().map(|(_, c)| cell(c, "beam_type")).collect();
    assert!(types.contains("global") && types.contains("narrow"), "{types:?}");
    assert!(
        carriers.iter().any(|(_, c)| cell(c, "kind").starts_with("GMR-2 class")),
        "{:?}",
        carriers.iter().map(|(_, c)| (cell(c, "offset_hz"), cell(c, "kind"), cell(c, "width_hz"))).collect::<Vec<_>>()
    );
    assert!(!rows(res, sdrkit_stack_satcom::typed::BGAN_LOG.key).is_empty());
    assert!(field_text(res, "note").contains("uw_order"));
}
