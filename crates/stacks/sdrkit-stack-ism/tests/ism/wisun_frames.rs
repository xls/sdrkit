//! `wisun` at the octet level: frames sent by real devices, the whitening, the FCS and the channel plans.

use sdrkit_dev_test::sigmf::read_expected;
use sdrkit_stack_ism::family::from_hex;
use sdrkit_stack_ism::mac154::{self, eui, Addr};
use sdrkit_stack_ism::wisun::frame::{
    self, channel_number, fcs_ok, parse_ies, phr, pn9, summary, Plan,
};
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::{seq, BitOrder};

/// The whitened PSDUs of two frames heard from Wi-SUN devices (expected.json).
fn offair() -> Vec<serde_json::Value> {
    let exp = read_expected("ism").unwrap();
    exp["byte_vectors"]["wisun_offair_psdus_whitened"]
        .as_array()
        .unwrap()
        .clone()
}

/// The Wi-SUN PN9 drops its first 9 outputs: frames sent by real devices pass their CRC-32 only when de-whitened with it, and fail
/// with the PN9 started at its first output.
#[test]
fn the_whitening_matches_frames_sent_by_real_devices() {
    for x in offair() {
        let raw = from_hex(x["psdu_hex"].as_str().unwrap()).unwrap();
        let mut p = raw.clone();
        pn9(&mut p);
        assert!(fcs_ok(&p, false));
        let mut q = raw.clone();
        Additive::new(seq::PN9_WHITENING).apply_bytes(&mut q, BitOrder::Lsb);
        assert!(
            !fcs_ok(&q, false),
            "the PN9 from its first output does not fit"
        );
    }
    let mut z = [0u8; 8];
    pn9(&mut z);
    let exp = read_expected("ism").unwrap();
    assert_eq!(
        sdrkit_stack_ism::family::hex(&z),
        exp["byte_vectors"]["wisun_pn9_first_octets"]
            .as_str()
            .unwrap()
            .to_uppercase()
    );
    let mut d: Vec<u8> = (0..50).collect();
    let o = d.clone();
    pn9(&mut d);
    pn9(&mut d);
    assert_eq!(d, o);
}

#[test]
fn offair_psdus() {
    assert_eq!(CRC32_ISO_HDLC.checksum(b"123456789"), 0xCBF43926);
    let exp = read_expected("ism").unwrap();
    let v = exp["byte_vectors"]["wisun_offair_psdus_whitened"]
        .as_array()
        .unwrap();
    let anz = plan("ANZ");
    for (n, x) in v.iter().enumerate() {
        let ph = u16::from_str_radix(x["phr_hex"].as_str().unwrap(), 16).unwrap();
        let (ms, fcs2, white, len) = phr(ph);
        let mut p = from_hex(x["psdu_hex"].as_str().unwrap()).unwrap();
        assert_eq!(p.len(), len);
        assert!(!ms && !fcs2 && white);
        assert!(!fcs_ok(&p, false), "passes only after de-whitening");
        pn9(&mut p);
        assert!(fcs_ok(&p, false));
        let body = &p[..p.len() - 4];
        let m = mac154::parse(body).unwrap();
        let (ies, rest) = parse_ies(body, &m);
        let e = &x["expect"];
        let mhz = x["mhz"].as_f64().unwrap();
        assert_eq!(
            channel_number(&anz, mhz * 1e6, 100).unwrap().1 as u64,
            e["anz_400khz_channel"].as_u64().unwrap()
        );
        assert_eq!(
            eui(frame::src_eui(&m).unwrap()),
            e["src_eui"].as_str().unwrap()
        );
        assert_eq!(
            m.aux.as_ref().unwrap().counter.unwrap() as u64,
            e["frame_counter"].as_u64().unwrap()
        );
        assert_eq!(
            format!("{:04X}", ies.bsn.unwrap()),
            e["bsn_hex"].as_str().unwrap()
        );
        assert_eq!(
            frame::utt_name(ies.utt.unwrap()).0,
            e["utt"].as_str().unwrap()
        );
        if n == 0 {
            assert_eq!(m.version, 2);
            assert_eq!(m.type_name(), "Data");
            assert!(m.security && m.seq_suppressed && m.ie_present);
            assert_eq!(m.dst, Addr::None);
            assert_eq!(m.src_pan, Some(0x0000));
            let a = m.aux.as_ref().unwrap();
            assert_eq!((a.level, a.key_id_mode, a.key_index), (6, 1, Some(1)));
            assert_eq!(
                format!("{:06X}", ies.ufsi.unwrap()),
                e["ufsi_hex"].as_str().unwrap()
            );
            assert_eq!(ies.bio.unwrap() as u64, e["bio"].as_u64().unwrap());
            assert_eq!(ies.lbc.unwrap() as u64, e["lfn_bcast_ms"].as_u64().unwrap());
            assert_eq!(ies.list.join(" "), e["ies"].as_str().unwrap());
            assert_eq!(rest as u64, e["encrypted_payload_len"].as_u64().unwrap());
            assert_eq!(summary(&m, &ies, rest), e["info"].as_str().unwrap());
        } else {
            assert_eq!(m.seq.unwrap() as u64, e["seq"].as_u64().unwrap());
            // a broadcast frame carries no destination address
            assert_eq!(m.dst, Addr::None);
            assert_eq!(e["dest"].as_str().unwrap(), "broadcast");
        }
    }
}

/// The plan `id` of the data folder, through the settings of the stack.
fn plan(id: &str) -> Plan {
    let p = crate::util::wisun_plan(id);
    Plan::parse(&p.ranges_text(), &p.classes_text()).expect("the plan settings parse")
}

#[test]
fn coverage_follows_the_rate_and_brazil_gap() {
    let (ranges, plans) = crate::util::wisun_plans_of_frequency();
    let at = sdrkit_presets::ism::wisun_plan_at(&ranges, &plans, 921.5e6).unwrap();
    assert_eq!(at.id, "ANZ");
    let anz = plan(&at.id);
    let ch = frame::channels(&anz, 921.5e6, 0.45 * 2e6);
    assert!(ch.iter().all(|c| (c.0 - 921.5e6).abs() <= 0.9e6));
    assert!(!ch.is_empty());
    let bz = plan("BZ");
    let all = frame::channels(&bz, 915e6, 20e6);
    assert!(all.iter().all(|c| !(907.5e6 < c.0 && c.0 < 915e6)));
}
