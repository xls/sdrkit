//! The `espnow` stack: the worked frames of the specification, CCMP, the vendor content reader, and the test signal through
//! the receiver. Keys in these tests are made up (the bytes 00 to 0F, or the published test vector of IEEE Std 802.11).

use super::help::{config, feed, impaired, num, rows, snr, text, value};
use sdrkit_core::{Complex32, Event, InputInfo, Owner, Receiver, SettingSet, Value};
use sdrkit_graph::StackGraph;
use sdrkit_stack_wlan::espnow::ccmp;
use sdrkit_stack_wlan::espnow::frame::{
    self, action_header, is_text, read, read_content, summary, Reading,
};
use sdrkit_stack_wlan::espnow::gen::{
    ap_beacon, espnow_frame, on_phy, scene, EspNowSpec, GenPhy, SENDER, TEST_KEY,
};
use sdrkit_stack_wlan::espnow::{self, EspnowSetting};
use sdrkit_stack_wlan::typed::{
    EspnowDeviceRow as D, EspnowFrameRow as F, ESPNOW_DEVICE, ESPNOW_FRAME,
};
use sdrkit_stack_wlan::wifi::gen::{GenFrame, WifiGenParams, WifiGenerator};

fn hex(t: &str) -> Vec<u8> {
    let d: Vec<u8> = t.bytes().filter(u8::is_ascii_hexdigit).collect();
    d.chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

const FRAME_1: &str = "D0 00 00 00 FF FF FF FF FF FF 24 6F 28 11 22 33 FF FF FF FF FF FF A0 00
    7F 18 FE 34 5A 17 C0 DE DD 0E 18 FE 34 04 01 74 65 6D 70 3D 32 31 2E 35 77 34 12 03";
const FRAME_2: &str = "D0 00 00 00 84 CC A8 01 02 03 24 6F 28 11 22 33 FF FF FF FF FF FF 30 12
    7F 18 FE 34 A1 B2 C3 D4 DD 11 18 FE 34 04 01 01 02 03 04 2A 00 00 00 10 27 00 00 BC AC 23 F5";
const PEER: [u8; 6] = [0x84, 0xCC, 0xA8, 0x01, 0x02, 0x03];
const BINARY: [u8; 12] = [1, 2, 3, 4, 0x2A, 0, 0, 0, 0x10, 0x27, 0, 0];

fn no_keys() -> Vec<ccmp::CcmpCipher> {
    Vec::new()
}

fn reading(mpdu_with_fcs: &[u8], keys: &[ccmp::CcmpCipher]) -> Reading {
    let mut scratch = Vec::new();
    read(
        &mpdu_with_fcs[..mpdu_with_fcs.len() - 4],
        keys,
        espnow::table::is_espressif,
        &mut scratch,
    )
}

fn message(mpdu_with_fcs: &[u8], keys: &[ccmp::CcmpCipher]) -> frame::Message {
    match reading(mpdu_with_fcs, keys) {
        Reading::EspNow(m) => m,
        r => panic!("not ESP-NOW: {r:?}"),
    }
}

#[test]
fn the_worked_frames_build_and_read_as_the_specification_gives() {
    let f1 = hex(FRAME_1);
    let f2 = hex(FRAME_2);
    assert_eq!((f1.len(), f2.len()), (52, 55));
    assert!(sdrkit_proto_wlan_phy::mac::fcs_ok(&f1) && sdrkit_proto_wlan_phy::mac::fcs_ok(&f2));
    let built1 = espnow_frame(&EspNowSpec {
        seq: 10,
        ..EspNowSpec::new(SENDER, [0xFF; 6], b"temp=21.5")
    });
    assert_eq!(built1, f1);
    let built2 = espnow_frame(&EspNowSpec {
        seq: 291,
        random: 0xA1B2_C3D4,
        ..EspNowSpec::new(SENDER, PEER, &BINARY)
    });
    assert_eq!(built2, f2);

    let m = message(&f1, &no_keys());
    assert_eq!(m.header.from, SENDER);
    assert!(espnow::table::is_espressif(&m.header.from));
    assert!(m.header.broadcast());
    assert_eq!((m.header.seq, m.header.fragment, m.header.subtype), (10, 0, 13));
    let c = m.content.as_ref().unwrap();
    assert_eq!((c.random, c.version, c.elements), (0x5A17_C0DE, 1, 1));
    assert_eq!(c.payload, b"temp=21.5");
    assert_eq!(summary(&m), "v1, 9 bytes to broadcast: \"temp=21.5\"");

    let m = message(&f2, &no_keys());
    assert_eq!((m.header.to, m.header.seq), (PEER, 291));
    assert!(!m.header.broadcast());
    let c = m.content.as_ref().unwrap();
    assert_eq!((c.random, c.payload.as_slice()), (0xA1B2_C3D4, &BINARY[..]));
    assert!(!is_text(&c.payload));
    assert_eq!(summary(&m), "v1, 12 bytes to 84:CC:A8:01:02:03");
}

#[test]
fn frame_3_splits_600_bytes_into_three_elements_and_joins_them() {
    let body: Vec<u8> = (0..600u32).map(|i| i as u8).collect();
    let f = espnow_frame(&EspNowSpec {
        version: 2,
        ..EspNowSpec::new(SENDER, [0xFF; 6], &body)
    });
    let content = &f[24..f.len() - 4];
    let mut at = 8;
    let mut seen = Vec::new();
    while at < content.len() {
        let len = usize::from(content[at + 1]);
        seen.push((len - 5, content[at + 6]));
        at += 2 + len;
    }
    assert_eq!(seen, [(250, 0x12), (250, 0x12), (100, 0x02)]);
    let m = message(&f, &no_keys());
    let c = m.content.as_ref().unwrap();
    assert_eq!((c.version, c.elements, c.more_data), (2, 3, false));
    assert_eq!(c.payload, body);
    assert!(summary(&m).contains(", several elements"), "{}", summary(&m));
}

#[test]
fn ccmp_holds_the_test_vector_of_ieee_802_11_2012_m_6_4() {
    let key: [u8; 16] = hex("C97C1F67CE371185514A8A19F2BDD52F").try_into().unwrap();
    let pn = 0xB503_9776_E70C;
    let header: [u8; 24] = hex("08 48 c3 2c 0f d2 e1 28 a5 7c 50 30 f1 84 44 08 ab ae a5 b8 fc ba 80 33")
        .try_into()
        .unwrap();
    assert_eq!(ccmp::ccmp_header(pn, 0).to_vec(), hex("0c e7 00 20 76 97 03 b5"));
    assert_eq!(
        ccmp::nonce(0, &header, pn).to_vec(),
        hex("00 50 30 f1 84 44 08 b5 03 97 76 e7 0c")
    );
    assert_eq!(
        ccmp::aad(&header).to_vec(),
        hex("08 40 0f d2 e1 28 a5 7c 50 30 f1 84 44 08 ab ae a5 b8 fc ba 00 00")
    );
    let mut body = hex("f8 ba 1a 55 d0 2f 85 ae 96 7b b6 2f b6 cd a8 eb 7e 78 a0 50");
    let plain = body.clone();
    let c = ccmp::cipher(&key);
    let mic = ccmp::seal(&c, 0, &header, pn, &mut body).unwrap();
    assert_eq!(body, hex("f3 d0 a2 fe 9a 3d bf 23 42 a6 43 e4 32 46 e8 0c 3c 04 d0 19"));
    assert_eq!(mic.to_vec(), hex("78 45 ce 0b 16 f9 76 23"));
    let mut out = vec![0; body.len()];
    assert!(ccmp::open(&c, 0, &header, pn, &body, &mic, &mut out));
    assert_eq!(out, plain);
    body[3] ^= 0x01;
    assert!(!ccmp::open(&c, 0, &header, pn, &body, &mic, &mut out));
}

#[test]
fn sealed_frames_open_with_the_key_and_stay_encrypted_without_it() {
    let key = [ccmp::cipher(&TEST_KEY)];
    let wrong = [ccmp::cipher(&[0x55; 16])];
    for len in [0usize, 1, 7, 100, 249, 250, 251, 500, 750, 1000, 1470] {
        let body: Vec<u8> = (0..len).map(|i| (i * 13 + 5) as u8).collect();
        for seq in [0u16, 1, 4094, 4095] {
            let spec = EspNowSpec {
                version: if len > 250 { 2 } else { 1 },
                seq,
                sealed: Some((TEST_KEY, 1 + u64::from(seq))),
                ..EspNowSpec::new(SENDER, PEER, &body)
            };
            let f = espnow_frame(&spec);
            assert!(sdrkit_proto_wlan_phy::mac::fcs_ok(&f));
            assert_eq!(f[1] & 0x40, 0x40);
            let m = message(&f, &key);
            assert!(m.decrypted && m.sealed.is_some(), "{len} {seq}");
            assert_eq!(m.header.seq, seq);
            assert_eq!(m.content.as_ref().unwrap().payload, body, "{len} {seq}");
            assert!(summary(&m).contains(", decrypted"));
            let shut = message(&f, &wrong);
            assert!(shut.content.is_none() && !shut.decrypted);
            assert_eq!(
                summary(&shut),
                format!("encrypted (CCMP, PN {}) to 84:CC:A8:01:02:03", 1 + u64::from(seq))
            );
            let clear = espnow_frame(&EspNowSpec {
                sealed: None,
                ..spec
            });
            assert_eq!(message(&clear, &no_keys()).content.unwrap().payload, body);
        }
    }
    // A flipped bit of the ciphertext fails the MIC: the frame stays encrypted.
    let mut f = espnow_frame(&EspNowSpec {
        sealed: Some((TEST_KEY, 1)),
        ..EspNowSpec::new(SENDER, PEER, b"secret door code 4711")
    });
    f[40] ^= 0x04;
    let m = message(&f, &key);
    assert!(m.content.is_none() && !m.decrypted);
}

#[test]
fn other_vendors_other_action_frames_and_truncated_elements_are_not_espnow() {
    let f1 = hex(FRAME_1);
    let mpdu = &f1[..f1.len() - 4];
    // Another organisation identifier of the action frame: an action frame, not ESP-NOW.
    let mut other = mpdu.to_vec();
    other[25..28].copy_from_slice(&[0x00, 0x50, 0xF2]);
    let mut s = Vec::new();
    assert!(matches!(read(&other, &[], |_| true, &mut s), Reading::Action(_)));
    // Another category.
    let mut cat = mpdu.to_vec();
    cat[24] = 4;
    assert!(matches!(read(&cat, &[], |_| true, &mut s), Reading::Action(_)));
    // A vendor element of another type.
    let mut typ = mpdu.to_vec();
    typ[37] = 5;
    assert!(matches!(read(&typ, &[], |_| true, &mut s), Reading::Action(_)));
    // Another identifier inside the element is accepted and reported.
    let mut inner = mpdu.to_vec();
    inner[34..37].copy_from_slice(&[0xAC, 0xDE, 0x48]);
    let Reading::EspNow(m) = read(&inner, &[], |_| true, &mut s) else {
        panic!("the element identifier is not checked");
    };
    assert_eq!(m.content.as_ref().unwrap().element_oui, [0xAC, 0xDE, 0x48]);
    assert!(summary(&m).ends_with(", element identifier ACDE48"));
    // An element that runs past the end: nothing is read past it.
    for cut in 1..=12 {
        let t = &mpdu[..mpdu.len() - cut];
        assert!(read_content(&t[24..]).is_none(), "cut {cut}");
        assert!(matches!(read(t, &[], |_| true, &mut s), Reading::Action(_)));
    }
    // A beacon, a data frame and a short frame.
    let b = ap_beacon();
    assert!(matches!(read(&b[..b.len() - 4], &[], |_| true, &mut s), Reading::Other));
    assert!(action_header(&mpdu[..23]).is_none());
    assert!(read_content(&mpdu[24..31]).is_none());
    // Subtype 14 (action no ack) is read as well.
    let mut noack = mpdu.to_vec();
    noack[0] = 0xE0;
    assert!(matches!(read(&noack, &[], |_| true, &mut s), Reading::EspNow(_)));
}

#[test]
fn the_text_rule() {
    assert!(!is_text(b"ok"));
    assert!(is_text(b"abc"));
    assert!(is_text(b"line\r\n\0"));
    assert!(!is_text(&[0x80, 0x81, b'a', b'b', b'c']));
    assert!(is_text(&[0x80, b'a', b'b', b'c', b'd']));
    assert_eq!(frame::quoted(&[b'a', 0x01, b'b']), "a.b");
    assert_eq!(frame::quoted(&[b'x'; 100]).len(), 60);
    assert_eq!(
        frame::parse_keys("000102030405060708090a0b0c0d0e0f, 00:01:02:03:04:05:06:07:08:09:0A:0B:0C:0D:0E:0F"),
        Some(vec![TEST_KEY, TEST_KEY])
    );
    assert_eq!(frame::parse_keys(""), Some(vec![]));
    assert_eq!(frame::parse_keys("0001"), None);
}

fn rx(settings: &[(&str, Value)]) -> StackGraph {
    espnow::receiver(&config(EspnowSetting::schema(), settings)).unwrap()
}

/// Runs `frames` (looped) for `seconds` with noise at `snr_db` through a receiver with `settings`, the radio at `centre`.
fn run(
    frames: Vec<GenFrame>,
    snr_db: f64,
    seconds: f64,
    centre: f64,
    settings: &[(&str, Value)],
) -> (StackGraph, Vec<Event>) {
    let g = WifiGenerator::new(WifiGenParams {
        rate_hz: 20e6,
        frames,
        ..Default::default()
    });
    let x: Vec<Complex32> = impaired(Box::new(g), &snr(snr_db), 3, seconds);
    let mut r = rx(settings);
    r.configure(&InputInfo::new(20e6, centre, centre).recording())
        .unwrap();
    let fed = feed(&mut r, &x, 32_768);
    (r, fed.events)
}

fn first() -> EspNowSpec {
    EspNowSpec {
        seq: 10,
        ..EspNowSpec::new(SENDER, [0xFF; 6], b"temp=21.5")
    }
}

#[test]
fn the_mixed_scene_lists_five_devices_and_no_beacon() {
    let (r, events) = run(scene(&first(), GenPhy::Dsss1), 30.0, 0.4, 2412e6, &[]);
    let res = r.results();
    assert_eq!(num(res, "channel"), Some(1.0));
    assert_eq!(num(res, "devices"), Some(5.0));
    assert_eq!(value(res, "heard"), Some(&Value::Bool(true)));
    let devices = rows(res, "devices");
    assert!(devices.iter().all(|d| text(d, &D::ADDRESS) != "00:04:0E:12:34:56"));
    let espnow_frames = num(res, "espnow_frames").unwrap();
    assert!(espnow_frames >= 25.0, "{espnow_frames}");
    let per_rate: Vec<(String, f64)> = rows(res, "by_rate")
        .into_iter()
        .map(|row| {
            (
                text(row, &sdrkit_stack_wlan::typed::EspnowRateRow::RATE),
                super::help::cnum(row, &sdrkit_stack_wlan::typed::EspnowRateRow::FRAMES),
            )
        })
        .collect();
    for want in ["1 Mbit/s", "2 Mbit/s long", "2 Mbit/s short", "OFDM 6 and 9", "OFDM 12 and above"] {
        let n = per_rate.iter().find(|(k, _)| k == want).map_or(0.0, |r| r.1);
        assert!(n >= 3.0, "{want}: {per_rate:?}");
    }
    let by_channel: f64 = rows(res, "by_channel")
        .into_iter()
        .map(|row| super::help::cnum(row, &sdrkit_stack_wlan::typed::EspnowChannelRow::FRAMES))
        .sum();
    assert_eq!(by_channel, espnow_frames);
    let log = rows(res, "frames");
    for want in [
        "v1, 9 bytes to broadcast: \"temp=21.5\"",
        "v1, 12 bytes to 84:CC:A8:01:02:03",
        "v2, 600 bytes to broadcast, several elements",
        "encrypted (CCMP, PN 1) to 24:6F:28:11:22:33",
    ] {
        assert!(log.iter().any(|l| text(l, &F::DETAILS) == want), "{want}");
    }
    let sealed = log
        .iter()
        .find(|l| text(l, &F::FROM) == "24:0A:C4:AA:BB:CC")
        .unwrap();
    assert_eq!(text(sealed, &F::PAYLOAD), "");
    assert_eq!(num(res, "decrypted"), Some(0.0));
    assert!(events.iter().filter(|e| e.is(ESPNOW_DEVICE.def())).count() == 5);
    assert!(events.iter().filter(|e| e.is(ESPNOW_FRAME.def())).count() as f64 == espnow_frames);
}

#[test]
fn the_key_opens_the_encrypted_sender() {
    let key = "00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f".replace(' ', "");
    let (r, _) = run(
        scene(&first(), GenPhy::Dsss1),
        25.0,
        0.4,
        2412e6,
        &[("keys", Value::Text(key))],
    );
    let res = r.results();
    assert!(num(res, "decrypted").unwrap() >= 3.0);
    let log = rows(res, "frames");
    let opened = log
        .iter()
        .find(|l| text(l, &F::FROM) == "24:0A:C4:AA:BB:CC")
        .unwrap();
    assert_eq!(
        text(opened, &F::DETAILS),
        "v1, 21 bytes to 24:6F:28:11:22:33: \"secret door code 4711\", decrypted"
    );
    let payload = text(opened, &F::PAYLOAD);
    assert_eq!(hex(&payload), b"secret door code 4711");
}

#[test]
fn a_bad_fcs_and_a_truncated_frame_are_not_listed() {
    let bad = on_phy(espnow_frame(&first()), GenPhy::Dsss1).bad_fcs();
    let mut cut = espnow_frame(&first());
    cut.truncate(cut.len() - 6);
    sdrkit_proto_wlan_phy::mac::push_fcs(&mut cut);
    let good = espnow_frame(&EspNowSpec::new(PEER, [0xFF; 6], b"still here"));
    let frames = vec![
        bad,
        on_phy(cut, GenPhy::Dsss1),
        on_phy(good, GenPhy::Ofdm6),
    ];
    let (r, _) = run(frames, 30.0, 0.2, 2412e6, &[]);
    let res = r.results();
    assert_eq!(num(res, "devices"), Some(1.0));
    let devices = rows(res, "devices");
    assert_eq!(text(devices[0], &D::ADDRESS), "84:CC:A8:01:02:03");
    assert!(num(res, "frames_bad").unwrap() >= 5.0);
    assert!(num(res, "action_frames").unwrap() > num(res, "espnow_frames").unwrap());
}

#[test]
fn the_longest_payload_passes_the_receiver_on_dsss_and_ofdm() {
    let body: Vec<u8> = (0..1470u32).map(|i| (i * 31 + 7) as u8).collect();
    let spec = EspNowSpec {
        version: 2,
        ..EspNowSpec::new(SENDER, PEER, &body)
    };
    let f = espnow_frame(&spec);
    assert_eq!(f.len(), 24 + 8 + 6 * 7 + 1470 + 4);
    for phy in [GenPhy::Dsss1, GenPhy::Ofdm6, GenPhy::Ofdm12] {
        let (r, _) = run(vec![on_phy(f.clone(), phy)], 30.0, 0.1, 2412e6, &[]);
        let log = rows(r.results(), "frames");
        assert!(!log.is_empty(), "{phy:?}");
        assert_eq!(hex(&text(log[0], &F::PAYLOAD)), body, "{phy:?}");
        assert!(text(log[0], &F::DETAILS).starts_with("v2, 1470 bytes to 84:CC:A8:01:02:03"));
    }
}

#[test]
fn the_radio_2_mhz_above_channel_1() {
    let frames = vec![on_phy(espnow_frame(&first()), GenPhy::Dsss1).offset(-2e6)];
    let (r, _) = run(frames, 30.0, 0.1, 2414e6, &[]);
    let res = r.results();
    assert_eq!(num(res, "channel"), Some(1.0));
    assert_eq!(num(res, "channel_offset_hz"), Some(-2e6));
    assert_eq!(num(res, "channel_hz"), Some(2412e6));
    assert!(num(res, "espnow_frames").unwrap() >= 5.0);
}

#[test]
fn a_channel_and_hopping_ask_the_host() {
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(20e6, 2412e6, 2412e6).recording())
        .unwrap();
    super::help::set(&mut r, "channel", Value::F64(6.0)).unwrap();
    super::help::set(&mut r, "channels_24", Value::Text("1 6 11".into())).unwrap();
    super::help::set(&mut r, "hop", Value::Bool(true)).unwrap();
    assert!(super::help::set(&mut r, "keys", Value::Text("not a key".into())).is_err());
    let fed = feed(&mut r, &vec![Complex32::default(); 20_000], 10_000);
    let s = format!("{:?}", fed.requests);
    assert!(s.contains("2437000000"), "{s}");
    assert!(s.contains("2462000000"), "{s}");
}

#[test]
fn too_low_a_rate_is_refused() {
    let mut r = rx(&[]);
    assert!(r
        .configure(&InputInfo::new(2.5e6, 2412e6, 2412e6).recording())
        .is_err());
}

#[test]
fn the_harness_runs_the_descriptor() {
    let d = &espnow::ESPNOW;
    assert_eq!(
        (d.tuning.min_hz, d.tuning.max_hz, d.tuning.default_hz, d.tuning.min_rate_hz),
        (2400e6, 2500e6, 2412e6, 20e6)
    );
    let run = sdrkit_dev_test::Harness::new(d)
        .set(&espnow::GEN_SCENE, espnow::GenScene::Single)
        .set(&espnow::GEN_PEER, "84:CC:A8:01:02:03")
        .set(&espnow::GEN_PAYLOAD, "hello peer")
        .seconds(0.2)
        .run()
        .unwrap();
    assert!(num(&run.results, "espnow_frames").unwrap() >= 20.0);
    let log = rows(&run.results, "frames");
    assert_eq!(
        text(log[0], &F::DETAILS),
        "v1, 10 bytes to 84:CC:A8:01:02:03: \"hello peer\""
    );
}

#[test]
fn levels_of_the_full_power_senders_stay_at_or_below_full_scale() {
    let levels = |phy: GenPhy, snr_db: Option<f64>| {
        let g = WifiGenerator::new(WifiGenParams {
            rate_hz: 20e6,
            frames: scene(&first(), phy),
            ..Default::default()
        });
        let imp = snr_db.map(snr).unwrap_or_default();
        let x: Vec<Complex32> = impaired(Box::new(g), &imp, 3, 0.4);
        let mut r = rx(&[]);
        r.configure(&InputInfo::new(20e6, 2412e6, 2412e6).recording())
            .unwrap();
        feed(&mut r, &x, 32_768);
        let mut l: Vec<(String, f64)> = rows(r.results(), "devices")
            .into_iter()
            .map(|d| (text(d, &D::ADDRESS), super::help::cnum(d, &D::LEVEL_DBFS)))
            .collect();
        l.sort_by(|a, b| a.0.cmp(&b.0));
        l
    };
    for phy in [GenPhy::Dsss1, GenPhy::Ofdm6] {
        let clean = levels(phy, None);
        assert_eq!(clean.len(), 5);
        assert!(clean.iter().all(|(_, v)| *v <= 0.0), "{phy:?}: {clean:?}");
        // With noise the level is the signal's own: the noise in the band is taken out.
        let noisy = levels(phy, Some(30.0));
        assert_eq!(noisy.len(), 5);
        for ((_, x), (_, y)) in clean.iter().zip(&noisy) {
            assert!((x - y).abs() < 0.1, "{phy:?}: {clean:?} {noisy:?}");
        }
    }
}
