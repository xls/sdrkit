//! Protocol cases that run on bytes: Meshtastic slots, decoding, keys and ports, MeshCore packets and adverts, LoRa APRS and
//! MeshCom frames, and the channels of the data folder.

#![allow(clippy::field_reassign_with_default)]

use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_proto_lora_phy::{Ldro, LoraParams};
use sdrkit_stack_lora::mesh::decode::{Decoded, MeshDecoder};
use sdrkit_stack_lora::mesh::plan::PlanEntry;
use sdrkit_stack_lora::proto::error::{ParseError, Part};
use sdrkit_stack_lora::proto::hamlora::{self as ham};
use sdrkit_stack_lora::proto::meshcore::{self as mc, Identity};
use sdrkit_stack_lora::proto::meshtastic::{self as mt, Data, Header};
use sdrkit_util_bits::protobuf::Writer;

use crate::common::{data as folder, slot_band};

fn entry(mask: u32, sync: u8) -> PlanEntry {
    PlanEntry {
        mask,
        name: "test".into(),
        freq_hz: 869.525e6,
        params: LoraParams::new(11, 250e3, 5, 16, sync),
        in_band: true,
        detector: 0,
    }
}

fn frame(payload: &[u8], sync: u8) -> LoraFrame {
    LoraFrame {
        payload: payload.to_vec(),
        sf: 11,
        bw: 250e3,
        cr: 5,
        crc_present: true,
        crc_ok: true,
        sync,
        snr_db: 5.0,
        channel_hz: 869.525e6,
        ..Default::default()
    }
}

fn mt_rx(dec: &mut MeshDecoder, b: &[u8]) -> Decoded {
    dec.frame(&frame(b, 0x2B), &entry(1, 0x2B), 1.0)
}

fn mc_rx(dec: &mut MeshDecoder, b: &[u8]) -> Decoded {
    dec.frame(&frame(b, 0x12), &entry(2, 0x12), 1.0)
}

fn ham_rx(dec: &mut MeshDecoder, b: &[u8]) -> Decoded {
    dec.frame(&frame(b, 0x12), &entry(4 | 8, 0x12), 1.0)
}

fn meshcom_rx(dec: &mut MeshDecoder, b: &[u8]) -> Decoded {
    dec.frame(&frame(b, 0x2B), &entry(8, 0x2B), 1.0)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Meshtastic.

#[test]
fn the_slot_table_exactly() {
    let presets = [
        "ShortTurbo",
        "ShortFast",
        "ShortSlow",
        "MediumFast",
        "MediumSlow",
        "LongTurbo",
        "LongFast",
        "LongModerate",
        "LongSlow",
    ];
    let table: &[(&str, [f64; 9])] = &[
        (
            "US",
            [
                926.750, 918.875, 920.625, 913.125, 914.875, 908.750, 906.875, 902.6875, 905.3125,
            ],
        ),
        (
            "EU_433",
            [
                433.750, 433.875, 433.625, 433.125, 433.875, 433.750, 433.875, 433.6875, 433.3125,
            ],
        ),
        (
            "EU_868",
            [
                0.0, 869.525, 869.525, 869.525, 869.525, 0.0, 869.525, 869.5875, 869.4625,
            ],
        ),
        (
            "CN",
            [
                502.750, 494.875, 492.625, 507.125, 504.875, 470.750, 478.875, 476.6875, 493.3125,
            ],
        ),
        (
            "JP",
            [
                923.250, 922.375, 923.125, 920.625, 921.375, 923.250, 923.375, 922.1875, 920.8125,
            ],
        ),
        (
            "ANZ",
            [
                926.750, 918.875, 920.625, 926.125, 927.875, 921.750, 919.875, 915.6875, 918.3125,
            ],
        ),
        (
            "ANZ_433",
            [
                434.300, 433.425, 434.175, 433.175, 433.925, 434.300, 434.425, 433.7375, 433.1125,
            ],
        ),
        (
            "RU",
            [
                868.950, 869.075, 868.825, 868.825, 869.075, 868.950, 869.075, 868.8875, 869.0125,
            ],
        ),
        (
            "KR",
            [
                922.750, 921.875, 922.625, 920.125, 920.875, 922.750, 922.875, 921.6875, 920.3125,
            ],
        ),
        (
            "TW",
            [
                922.750, 924.875, 922.625, 922.125, 924.875, 920.750, 923.875, 921.6875, 923.3125,
            ],
        ),
        (
            "IN",
            [
                865.750, 865.875, 865.625, 866.125, 865.875, 865.750, 865.875, 865.6875, 866.3125,
            ],
        ),
        (
            "NZ_865",
            [
                864.750, 864.875, 866.625, 865.125, 866.875, 864.750, 864.875, 866.6875, 867.3125,
            ],
        ),
        (
            "TH",
            [
                922.750, 924.875, 922.625, 922.125, 924.875, 920.750, 923.875, 921.6875, 923.3125,
            ],
        ),
        (
            "UA_433",
            [
                434.250, 433.375, 434.125, 433.125, 433.875, 434.250, 434.375, 433.6875, 433.0625,
            ],
        ),
        (
            "UA_868",
            [
                868.250, 868.375, 868.125, 868.125, 868.375, 868.250, 868.375, 868.1875, 868.3125,
            ],
        ),
        (
            "MY_433",
            [
                433.750, 433.875, 433.625, 434.125, 433.875, 433.750, 433.875, 433.6875, 434.3125,
            ],
        ),
        (
            "MY_919",
            [
                921.750, 923.875, 921.625, 921.125, 923.875, 919.750, 922.875, 920.6875, 922.3125,
            ],
        ),
        (
            "SG_923",
            [
                917.750, 917.875, 923.625, 922.125, 919.875, 917.750, 917.875, 923.6875, 924.3125,
            ],
        ),
        (
            "PH_433",
            [
                434.250, 433.375, 434.125, 433.125, 433.875, 434.250, 434.375, 433.6875, 433.0625,
            ],
        ),
        (
            "PH_868",
            [
                868.750, 869.125, 868.125, 868.875, 869.125, 868.750, 868.125, 868.5625, 869.3125,
            ],
        ),
        (
            "PH_915",
            [
                917.750, 916.875, 917.625, 915.125, 915.875, 917.750, 917.875, 916.6875, 915.3125,
            ],
        ),
        (
            "KZ_433",
            [
                434.325, 433.450, 434.200, 433.200, 433.950, 434.325, 434.450, 433.7625, 433.1375,
            ],
        ),
        (
            "KZ_863",
            [
                865.750, 867.875, 865.625, 865.125, 867.875, 863.750, 866.875, 864.6875, 866.3125,
            ],
        ),
        (
            "NP_865",
            [
                867.750, 866.875, 867.625, 865.125, 865.875, 867.750, 867.875, 866.6875, 865.3125,
            ],
        ),
        (
            "BR_902",
            [
                905.750, 904.375, 905.125, 906.625, 907.375, 906.250, 903.875, 906.6875, 903.3125,
            ],
        ),
        (
            "LORA_24",
            [
                2447.9375,
                2445.09375,
                2476.78125,
                2410.15625,
                2441.84375,
                2423.5625,
                2404.46875,
                2452.203125,
                2435.140625,
            ],
        ),
    ];
    for (region, row) in table {
        let r = &slot_band(region);
        for (p, want) in presets.iter().zip(row) {
            let got = mt::slot_hz(mt::preset(p).unwrap(), r, "", 0, 0.0, 0.0);
            if *want == 0.0 {
                assert!(got.is_none(), "{region} {p}: {got:?}");
            } else {
                assert_eq!(got, Some((want * 1e6).round()), "{region} {p}");
            }
        }
    }
    assert_eq!(mt::djb2("LongFast"), 0x07C6_3403);
    // EdgeFastLow: its override in EU_868 (a setting of the row), a slot elsewhere.
    let efl = mt::preset("EdgeFastLow").unwrap();
    let fixed = folder()
        .meshtastic_settings("EU_868", "EdgeFastLow")
        .unwrap();
    let over = fixed
        .settings
        .iter()
        .find_map(|(k, v)| match v {
            sdrkit_core::Value::Hz(h) if k == "mt_override_freq" => Some(*h),
            _ => None,
        })
        .unwrap_or(0.0);
    assert_eq!(
        mt::slot_hz(efl, &slot_band("EU_868"), "", 0, 0.0, over),
        Some(869_431_274.0)
    );
    assert!(mt::slot_hz(efl, &slot_band("US"), "", 0, 0.0, 0.0).is_some());
    // Name matching.
    assert_eq!(mt::preset("long_fast").unwrap().name, "LongFast");
    assert_eq!(mt::preset("LongMod").unwrap().name, "LongModerate");
    // Every preset: preamble 16, sync 0x2B, LDRO by the rule (on for LongModerate and LongSlow).
    for p in presets {
        let lp = mt::lora_params(mt::preset(p).unwrap(), &slot_band("US"));
        assert_eq!((lp.preamble, lp.sync), (16, 0x2B), "{p}");
        assert_eq!(
            lp.ldro_on(),
            matches!(p, "LongModerate" | "LongSlow"),
            "{p}"
        );
    }
}

#[test]
fn decoding_the_default_key_packet() {
    let pkt = mt::hex_decode("8c646d7a2909000062d6b2136b08002946f2ecd0b02ac4cf2be5").unwrap();
    let mut dec = MeshDecoder::default();
    let d = mt_rx(&mut dec, &pkt);
    let p = &d.packet;
    assert_eq!(p.ptype, "TEXT_MESSAGE_APP");
    assert_eq!(p.channel, "LongFast");
    assert_eq!(p.from, "!00000929");
    assert_eq!(p.to, "!7a6d648c");
    assert_eq!(p.hops, 0);
    assert_eq!(p.relay, Some(0x29));
    assert_eq!(p.next_hop, Some(0));
    assert!(p.decrypted);
    assert_eq!(d.texts[0].text, "test");
    let h = Header::parse(&pkt).unwrap();
    assert_eq!(h.channel, 0x08);
    assert!(h.want_ack);
}

#[test]
fn the_public_key_test_packet_stays_encrypted() {
    let pkt = mt::hex_decode(
        "8c646d7a2909000062d6b2136b00000040df24abfcc30a17a3d9046726099e796a1c036a792b",
    )
    .unwrap();
    let mut dec = MeshDecoder::default();
    let d = mt_rx(&mut dec, &pkt);
    let p = &d.packet;
    assert_eq!(p.to, "!7a6d648c");
    assert_eq!(p.from, "!00000929");
    assert_eq!(p.packet_id.to_lowercase(), "0x13b2d662");
    assert_eq!((p.hop_limit, p.hop_start), (Some(3), Some(3)));
    assert!(!p.via_mqtt);
    let h = Header::parse(&pkt).unwrap();
    assert!(h.want_ack);
    assert_eq!(h.channel, 0);
    assert_eq!(p.note, "encrypted (direct)");
    assert!(!p.decrypted);
    assert!(d.texts.is_empty());
    assert!(dec
        .store
        .nodes
        .iter()
        .all(|n| n.record.long_name.is_empty()));
}

fn data(port: u32, payload: Vec<u8>) -> Data {
    Data {
        port,
        payload,
        bitfield: Some(0),
        ..Default::default()
    }
}

fn send(dec: &mut MeshDecoder, from: u32, to: u32, hl: u8, hs: u8, d: &Data) -> Decoded {
    send_id(dec, from, to, hl, hs, d, 0x55)
}

fn send_id(
    dec: &mut MeshDecoder,
    from: u32,
    to: u32,
    hl: u8,
    hs: u8,
    d: &Data,
    id: u32,
) -> Decoded {
    let b = sdrkit_stack_lora::gen::mt_packet(from, to, id, hs, hl, false, from as u8, d);
    mt_rx(dec, &b)
}

#[test]
fn every_port() {
    let mut dec = MeshDecoder::default();
    let bc = mt::BROADCAST;
    // Text with UTF-8, hops 1, to "^all".
    let x = send(
        &mut dec,
        0x101,
        bc,
        2,
        3,
        &data(1, "Hello mesh, tést ✓".as_bytes().to_vec()),
    );
    assert_eq!(x.packet.hops, 1);
    assert_eq!(x.packet.to, "^all");
    assert_eq!(x.texts[0].text, "Hello mesh, tést ✓");
    // A reply.
    let mut r = data(1, b"sure".to_vec());
    r.reply_id = 0x1234;
    let x = send(&mut dec, 0x102, bc, 3, 3, &r);
    assert!(
        x.packet.summary.ends_with(" (reply)"),
        "{}",
        x.packet.summary
    );
    // NodeInfo.
    for (hw, hwn, role, rn) in [(4, "TBEAM", 0, "CLIENT"), (43, "HELTEC_V3", 2, "ROUTER")] {
        let u = mt::User {
            id: "!00000103".into(),
            long_name: "Desert Fox".into(),
            short_name: "DF".into(),
            hw_model: hw,
            role,
            ..Default::default()
        };
        let info = mt::decode_port(&data(4, u.encode()));
        assert!(info.summary.contains("Desert Fox (DF)"), "{}", info.summary);
        assert!(info.summary.contains(hwn), "{}", info.summary);
        assert!(info.summary.contains(rn), "{}", info.summary);
    }
    // Positions.
    let info = mt::decode_port(&data(
        3,
        mt::Position::encode(25.0805, 55.1403, 12, 1760000000, 0, false),
    ));
    let p = info.position.unwrap();
    assert!((p.lat.unwrap() - 25.0805).abs() < 1e-6);
    assert!((p.lon.unwrap() - 55.1403).abs() < 1e-6);
    assert_eq!((p.alt, p.time), (Some(12), 1760000000));
    let p = mt::decode_port(&data(
        3,
        mt::Position::encode(-33.9, 18.4, -20, 1, 0, false),
    ))
    .position
    .unwrap();
    assert!(p.lat.unwrap() < 0.0);
    assert_eq!(p.alt, Some(-20));
    let info = mt::decode_port(&data(
        3,
        mt::Position::encode(25.0805, 55.1403, 12, 1, 13, false),
    ));
    let p = info.position.clone().unwrap();
    let e = (p.lat.unwrap() - 25.0805).abs();
    assert!(e < 0.03 && e > 0.0, "{e}");
    assert!(info.summary.contains("13 bit"), "{}", info.summary);
    let info = mt::decode_port(&data(3, mt::Position::encode(0.0, 0.0, 0, 1, 0, false)));
    assert!(info.position.is_none());
    assert_eq!(info.summary, "no position fix");
    // Unknown fields are skipped.
    let mut w_buf = Vec::new();
    let mut w = Writer::new(&mut w_buf);
    w.string(1, "!00000104")
        .string(2, "Odd Node")
        .uint(99, 7)
        .string(3, "ON")
        .bytes(120, &[1, 2, 3]);
    let u = mt::decode_port(&data(4, w_buf)).user.unwrap();
    assert_eq!(
        (u.long_name.as_str(), u.short_name.as_str()),
        ("Odd Node", "ON")
    );
    // Telemetry.
    let t = mt::decode_port(&data(
        67,
        sdrkit_stack_lora::gen::telemetry_device(1, 87, 4.07, 12.5, 1.25, 86400),
    ));
    let tv = t.telemetry.unwrap();
    assert_eq!(tv.battery, Some(87));
    assert!((tv.voltage.unwrap() - 4.07).abs() < 1e-3);
    assert!((tv.ch_util.unwrap() - 12.5).abs() < 1e-3);
    assert!((tv.air_tx.unwrap() - 1.25).abs() < 1e-3);
    assert_eq!(tv.uptime, Some(86400));
    assert!(t.summary.contains("battery 87 %"), "{}", t.summary);
    let t = mt::decode_port(&data(
        67,
        sdrkit_stack_lora::gen::telemetry_device(1, 101, 5.0, 1.0, 1.0, 1),
    ));
    assert!(t.summary.contains("powered"), "{}", t.summary);
    let t = mt::decode_port(&data(
        67,
        sdrkit_stack_lora::gen::telemetry_env(1, 31.5, 48.0, 1005.5),
    ));
    for s in ["31.5 C", "48 %", "1005.5 hPa"] {
        assert!(t.summary.contains(s), "{}", t.summary);
    }
    // Routing.
    let mut ack = data(5, mt::Routing::encode_error(0));
    ack.request_id = 0x1234;
    assert_eq!(mt::decode_port(&ack).summary, "ack of 00001234");
    let mut err = data(
        5,
        mt::Routing::encode_error(mt::routing_error_number("NO_ROUTE").unwrap()),
    );
    err.request_id = 0x1234;
    assert!(mt::decode_port(&err).summary.contains("NO_ROUTE"));
    // Traceroute request and reply.
    let req = mt::Route {
        route: vec![0x11223344],
        ..Default::default()
    };
    let mut d = data(70, req.encode());
    d.want_response = true;
    d.source = 0x101;
    let s = mt::decode_port(&d).summary;
    assert!(s.contains("request") && s.contains("!11223344"), "{s}");
    let rep = mt::Route {
        route: vec![0x11223344],
        snr_towards: vec![-8, 10],
        ..Default::default()
    };
    let mut d = data(70, rep.encode());
    d.request_id = 77;
    let s = mt::decode_port(&d).summary;
    assert!(s.contains("-2.0") && s.contains("2.5"), "{s}");
    // Neighbor info.
    let nb = |id: u32, snr: f32| {
        let mut w_buf = Vec::new();
        let mut w = Writer::new(&mut w_buf);
        w.uint(1, id as u64).float(2, snr);
        w_buf
    };
    let mut w_buf = Vec::new();
    let mut w = Writer::new(&mut w_buf);
    w.uint(1, 0x101)
        .bytes(4, &nb(0x11223344, 5.5))
        .bytes(4, &nb(0x55667788, -1.0));
    let s = mt::decode_port(&data(71, w_buf)).summary;
    assert!(s.starts_with("2 neighbors"), "{s}");
    assert!(s.contains("!11223344 5.5 dB"), "{s}");
}

#[test]
fn via_mqtt_flag() {
    let h = Header {
        to: mt::BROADCAST,
        from: 5,
        id: 9,
        hop_limit: 7,
        hop_start: 7,
        via_mqtt: true,
        ..Default::default()
    };
    let p = Header::parse(&h.bytes()).unwrap();
    assert!(p.via_mqtt);
    assert_eq!((p.hop_limit, p.hop_start), (7, 7));
    let mut dec = MeshDecoder::default();
    let b = mt::build_channel_packet(&h, &mt::DEFAULT_KEY, &data(1, b"x".to_vec()).encode());
    assert!(mt_rx(&mut dec, &b).packet.via_mqtt);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// MeshCore.

#[test]
fn decoded_fields() {
    let b = mt::hex_decode(
        "1501ab11f920a61f33a7906d3a9dcd1751d8c3318196e47c7d5dbda404b869f4b2ed4e459842",
    )
    .unwrap();
    let mut dec = MeshDecoder::default();
    let d = mc_rx(&mut dec, &b);
    assert_eq!(d.packet.ptype, "GRP_TXT");
    assert_eq!(d.packet.channel, "Public");
    assert_eq!(d.packet.path, "ab");
    assert_eq!(d.packet.hops, 1);
    let t = &d.texts[0];
    assert_eq!(
        (t.from.as_str(), t.text.as_str()),
        ("Dubai Repeater", "hello mesh")
    );
    let p = mc::McPacket::parse(&b).unwrap();
    assert_eq!((p.route, p.ptype, p.version), (1, 5, 0));
    assert_eq!(p.payload[0], 0x11);
}

#[test]
fn advert_layout_and_truncations() {
    let id = Identity::from_seed(&[9; 32]);
    let adv = mc::build_advert(&id, 1760000000, 2, Some((25.2, 55.3)), "Dubai Repeater");
    let pkt = mc::McPacket {
        route: 1,
        ptype: 4,
        version: 0,
        hash_size: 1,
        path: vec![],
        payload: adv,
    }
    .encode();
    assert_eq!(pkt[0], 0x11);
    assert_eq!(pkt[1], 0);
    assert_eq!(&pkt[2..34], &id.public());
    assert_eq!(&pkt[34..38], &[0x00, 0x78, 0xE7, 0x68]);
    assert_eq!(pkt[102], 0x92);
    let mut dec = MeshDecoder::default();
    let d = mc_rx(&mut dec, &pkt);
    assert_eq!(d.packet.ptype, "ADVERT");
    let a = mc::parse_advert(&pkt[2..]).unwrap();
    assert!(a.signature_ok);
    assert_eq!(mc::role_name(a.role), "Repeater");
    assert_eq!(a.name, "Dubai Repeater");
    let (lat, lon) = a.position.unwrap();
    assert!((lat - 25.2).abs() < 1e-5 && (lon - 55.3).abs() < 1e-5);
    let node_id: String = mt::hex(&id.public())[..12].to_string();
    assert_eq!(d.packet.from, node_id);
    let n = dec
        .store
        .node(sdrkit_stack_lora::mesh::types::Protocol::MeshCore, &node_id)
        .unwrap();
    assert_eq!(n.long_name, "Dubai Repeater");
    assert_eq!(n.role, "Repeater");
    assert!(n.position.is_some());
    // A changed signature byte.
    let mut bad = pkt.clone();
    bad[40] ^= 1;
    let d = mc_rx(&mut MeshDecoder::default(), &bad);
    assert!(
        d.packet.note.contains("signature not valid")
            || d.packet.summary.contains("signature not valid"),
        "{} / {}",
        d.packet.note,
        d.packet.summary
    );
    // A chat advert with a negative longitude.
    let chat = mc::build_advert(&id, 1, 1, Some((51.5, -0.12)), "Ann");
    let a = mc::parse_advert(&chat).unwrap();
    assert!(a.position.unwrap().1 < 0.0);
    assert_eq!(mc::role_name(a.role), "Chat");
    // Every truncation from 2 to 101 bytes produces no node.
    for n in 2..=101 {
        let mut dd = MeshDecoder::default();
        let d = mc_rx(&mut dd, &pkt[..n]);
        assert!(d.nodes.is_empty() && dd.store.nodes.is_empty(), "{n}");
    }
}

#[test]
fn two_byte_hashes_and_transport_codes() {
    let p = mc::McPacket {
        route: 1,
        ptype: 3,
        version: 0,
        hash_size: 2,
        path: vec![vec![0xaa, 0xbb], vec![0xcc, 0xdd]],
        payload: vec![1, 2, 3, 4],
    };
    let b = p.encode();
    assert_eq!(b[1], 0x42);
    let q = mc::McPacket::parse(&b).unwrap();
    assert_eq!(q.path_text(), "aabb,ccdd");
    // Route 0: four transport-code bytes are skipped.
    let mut t = vec![(3 << 2), 9, 9, 9, 9, 0x01, 0xab];
    t.extend_from_slice(&[1, 2, 3, 4]);
    let q = mc::McPacket::parse(&t).unwrap();
    assert_eq!((q.route, q.path_text().as_str()), (0, "ab"));
    assert_eq!(q.payload, vec![1, 2, 3, 4]);
    // Refusals.
    assert_eq!(
        mc::McPacket::parse(&[0x05, 0xC1, 1, 2, 3]).unwrap_err(),
        ParseError::PathLength
    );
    assert!(mc::McPacket::parse(&[0x05, 0x05, 1, 2]).is_err());
    assert_eq!(
        mc::McPacket::parse(&[0x05, 0x01, 0xab]).unwrap_err(),
        ParseError::Short(Part::MeshCorePayload)
    );
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRa APRS and MeshCom.

#[test]
fn message_object_mice_and_status() {
    let p = |s: &str| ham::parse_lora_aprs(&ham::build_lora_aprs(s)).unwrap();
    let a = p("A61AB-7>APLRT1,WIDE1-1:!2504.83N/05508.42E>Marina tracker 12.6V");
    assert!((a.info.lat.unwrap() - (25.0 + 4.83 / 60.0)).abs() < 1e-6);
    assert!((a.info.lon.unwrap() - (55.0 + 8.42 / 60.0)).abs() < 1e-6);
    let m = p("A61AB-7>APLRT1::A61EF-9  :Meet at the Marina at 9");
    assert_eq!(m.info.kind, "Message");
    assert_eq!(m.info.addressee.as_deref(), Some("A61EF-9"));
    assert_eq!(m.info.text.as_deref(), Some("Meet at the Marina at 9"));
    let o = p("A61AB-7>APLRT1:;DXBMTG-01*111111z2513.00N/05519.00E/meeting");
    assert_eq!(o.info.kind, "Object");
    assert!(o.info.name.as_deref().unwrap().starts_with("DXBMTG"));
    assert!((o.info.lat.unwrap() - (25.0 + 13.0 / 60.0)).abs() < 1e-6);
    assert!((o.info.lon.unwrap() - (55.0 + 19.0 / 60.0)).abs() < 1e-6);
    let mut mice = ham::build_lora_aprs("A61AB-7>ECCU8Y:'cZ");
    mice.push(0x7F);
    mice.extend_from_slice(b"l#H>/]Go fly a kite!");
    let x = ham::parse_lora_aprs(&mice).unwrap();
    assert!(x.info.kind.contains("Mic-E"), "{}", x.info.kind);
    assert!(x.info.lat.is_some());
    let s = p("A61AB-7>APLRT1:>status");
    assert_eq!(s.info.kind, "Status");
    assert_eq!(s.info.comment, "status");
    let mut b = ham::build_lora_aprs("A61AB-7>APLRT1:>35°C ");
    b.push(0xFF);
    b.extend_from_slice(b" end");
    let x = ham::parse_lora_aprs(&b).unwrap();
    assert!(x.info.comment.contains("35°C"), "{}", x.info.comment);
    assert!(x.info.comment.ends_with(" end"), "{}", x.info.comment);
    assert!(x.info.comment.contains('?'), "{}", x.info.comment);
}

#[test]
fn bad_inputs() {
    for bad in [
        b"A61AB>APRS:!x".to_vec(),
        ham::build_lora_aprs("A61AB APRS:!x"),
        ham::build_lora_aprs("A61AB>APRS !x"),
        ham::build_lora_aprs("A6#AB>APRS:!x"),
        ham::build_lora_aprs("A61AB>AP RS:!x"),
        ham::build_lora_aprs("A61AB>APRS,WI%DE:!x"),
    ] {
        assert!(ham::parse_lora_aprs(&bad).is_err(), "{bad:?}");
    }
    assert_eq!(
        ham::parse_lora_aprs(&ham::build_lora_aprs("A61AB>APRS:%%unknown"))
            .unwrap()
            .info
            .kind,
        "Other"
    );
}

#[test]
fn meshcom_position_text_checksum_ack() {
    let pos = ham::build_meshcom(
        b'!',
        0x11223344,
        4,
        0x10,
        "A61MC-2,A61MC-1",
        "*",
        "2506.00N/05512.00E[ /B=064 /A=000123",
    );
    let m = ham::parse_meshcom(&pos).unwrap();
    assert_eq!(m.source, "A61MC-2");
    assert_eq!(m.relays, "A61MC-1");
    assert_eq!(m.msg_id, 0x11223344);
    assert_eq!(m.max_hop, 4);
    assert_eq!(m.battery, Some(64));
    let i = m.info.unwrap();
    assert!((i.lat.unwrap() - 25.1).abs() < 1e-6);
    assert!((i.lon.unwrap() - 55.2).abs() < 1e-6);
    assert_eq!(i.symbol_code, Some('['));
    assert!((i.alt_m.unwrap() - 37.49).abs() < 0.01);
    let t = ham::build_meshcom(b':', 7, 3, 0x10, "A61MC-1", "A61MC-2", "QSL, 59 here");
    let m = ham::parse_meshcom(&t).unwrap();
    assert_eq!(m.kind, "TEXT");
    assert_eq!(m.text, "QSL, 59 here");
    assert_eq!(m.dest_call, "A61MC-2");
    let mut bad = t.clone();
    let n = bad.len();
    bad[n - 6] ^= 0x55;
    assert_eq!(
        ham::parse_meshcom(&bad).unwrap_err(),
        ParseError::MeshComChecksum
    );
    let a = ham::parse_meshcom(&ham::build_meshcom_ack(9, 2, 0x11223344)).unwrap();
    assert_eq!(a.acked, Some(0x11223344));
    let mut dec = MeshDecoder::default();
    let d = meshcom_rx(&mut dec, &ham::build_meshcom_ack(9, 2, 0x11223344));
    assert!(
        d.packet.summary.contains("11223344"),
        "{}",
        d.packet.summary
    );
}

#[test]
fn lora_aprs_and_meshcom_are_told_apart() {
    let la = ham::build_lora_aprs("A61AB-7>APLRT1:>status");
    let mcm = ham::build_meshcom(b':', 7, 3, 0x10, "A61MC-1", "*", "hi");
    assert!(ham::parse_meshcom(&la).is_err());
    assert!(ham::parse_lora_aprs(&mcm).is_err());
    let mut dec = MeshDecoder::default();
    assert_eq!(
        format!("{:?}", ham_rx(&mut dec, &la).packet.protocol),
        format!("{:?}", sdrkit_stack_lora::mesh::types::Protocol::LoraAprs)
    );
    assert_eq!(
        format!("{:?}", meshcom_rx(&mut dec, &mcm).packet.protocol),
        format!("{:?}", sdrkit_stack_lora::mesh::types::Protocol::MeshCom)
    );
}

#[test]
fn presets() {
    let all = folder().ham_lora_channels().unwrap();
    let param = |c: &&sdrkit_presets::lora::LoraChannel| {
        ham::params(
            c.sf,
            c.bandwidth_hz,
            c.cr,
            c.preamble.unwrap(),
            c.sync.unwrap(),
        )
    };
    let l: Vec<_> = all.iter().filter(|c| c.protocol == "loraaprs").collect();
    assert_eq!(l.len(), 4);
    let eu = l.iter().find(|p| p.frequency_hz == 433.775e6).unwrap();
    assert_eq!(
        (eu.sf, eu.bandwidth_hz / 1e3, eu.cr, eu.sync.unwrap()),
        (12, 125.0, 5, 0x12)
    );
    assert!(param(eu).ldro_on());
    let pl = l.iter().find(|p| p.frequency_hz == 434.855e6).unwrap();
    assert_eq!((pl.sf, pl.cr), (9, 7));
    assert!(!param(pl).ldro_on());
    let m = all.iter().find(|c| c.protocol == "meshcom").unwrap();
    assert_eq!(
        (
            m.frequency_hz / 1e6,
            m.sf,
            m.bandwidth_hz / 1e3,
            m.cr,
            m.sync.unwrap()
        ),
        (433.175, 11, 250.0, 6, 0x2B)
    );
    assert_eq!(param(&m).ldro, Ldro::Auto);
}

#[test]
fn the_meshcore_presets() {
    let b = folder().meshcore_presets().unwrap();
    assert!(b.len() >= 20);
    let eu = b.iter().find(|p| p.label == "EU/UK (Narrow)").unwrap();
    assert_eq!(
        (eu.frequency_hz, eu.bandwidth_hz, eu.sf, eu.cr),
        (869.618e6, 62.5e3, 8, 8)
    );
    let lp = mc::params(eu.sf, eu.bandwidth_hz, eu.cr);
    assert_eq!((lp.sync, lp.preamble, lp.ldro_on()), (0x12, 32, false));
    let mut r: Vec<&str> = Vec::new();
    for p in &b {
        if !r.contains(&p.group.as_str()) {
            r.push(&p.group);
        }
    }
    assert_eq!(r.len(), 5);
    assert_eq!(r[0], "Oceania");
    assert_eq!(
        b.iter()
            .find(|p| p.label.starts_with("Czech"))
            .unwrap()
            .note,
        "2B"
    );
}

#[test]
fn the_firmware_key_pair_decrypts_the_test_packet() {
    // The key pair of the firmware's public-key crypto test: the receiver's private key and the sender's public key.
    let private: [u8; 32] =
        mt::hex_decode("a00330633e63522f8a4d81ec6d9d1e6617f6c8ffd3a4c698229537d44e522277")
            .unwrap()
            .try_into()
            .unwrap();
    let public: [u8; 32] =
        mt::hex_decode("db18fc50eea47f00251cb784819a3cf5fc361882597f589f0d7ff820e8064457")
            .unwrap()
            .try_into()
            .unwrap();
    let shared = mt::pki_shared_key(&private, &public).unwrap();
    assert_eq!(mt::hex(&shared[..8]), "777b1545c9d6f9a2");
    let pkt = mt::hex_decode(
        "8c646d7a2909000062d6b2136b00000040df24abfcc30a17a3d9046726099e796a1c036a792b",
    )
    .unwrap();
    let plain = mt::pki_decrypt(&shared, 0x13b2d662, 0x929, &pkt[16..]).unwrap();
    assert_eq!(mt::hex(&plain), "08011204746573744800");
    // The decoder of the own node reads it once the sender's public key is in the node list.
    let mut dec = MeshDecoder::default();
    dec.mt_own = Some((0x7a6d648c, private));
    dec.store
        .node_mut(
            sdrkit_stack_lora::mesh::types::Protocol::Meshtastic,
            "!00000929",
            0.0,
        )
        .unwrap()
        .0
        .public_key = mt::hex(&public);
    let d = mt_rx(&mut dec, &pkt);
    assert!(d.packet.decrypted, "{}", d.packet.note);
    assert_eq!(d.packet.channel, "DM");
    assert_eq!(d.texts[0].text, "test");
    // A changed byte fails the tag check.
    let mut bad = pkt[16..].to_vec();
    bad[1] ^= 1;
    assert!(mt::pki_decrypt(&shared, 0x13b2d662, 0x929, &bad).is_none());
}
