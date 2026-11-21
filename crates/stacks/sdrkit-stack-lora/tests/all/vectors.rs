//! The recorded test vectors (`tests/vectors/lora`): every recording decodes to its `expected.json`.

use crate::common::{count, data, events, feed, invoke, packets, receiver, row, rows, settings_of};
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Event, Receiver, Record, Results, Value};
use sdrkit_stack_lora::mesh::types::TxKind;
use sdrkit_stack_lora::settings::*;
use sdrkit_stack_lora::typed::{
    AprsFields, AprsFrameRow, AprsPacketEvent, AprsStationRow, LoraPacketEvent, MeshDeviceRow,
    MeshMessageRow, MeshNodeRow, MeshPacketRow, MeshStationRow, MeshTypeRow, TxRequestEvent,
    WideFrameRow, APRS_FRAMES, APRS_PACKET, APRS_STATIONS, MESH_DEVICES, MESH_MESSAGES, MESH_NODES,
    MESH_PACKETS, MESH_STATIONS, MESH_TYPES, TX_REQUEST, WIDE_FRAMES,
};
use sdrkit_util_objects::publish::columns;
use serde_json::Value as J;

/// The payload of an event in hex.
fn hex(e: &Event) -> String {
    e.at(&LoraPacketEvent::PAYLOAD)
        .unwrap_or(&[])
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The newest message of `res` with this text.
fn message<'a>(res: &'a Results, t: &str) -> Option<&'a Record> {
    row(res, &MESH_MESSAGES, |r| r.get(&keys::TEXT) == Some(t))
}

/// The sender as people see it: the name, or the id.
fn sender(r: &Record) -> &str {
    r.at(&MeshMessageRow::SENDER)
        .or(r.get(&keys::SOURCE))
        .unwrap_or("")
}

fn run(rx: &mut dyn Receiver, name: &str, tuned: f64, piece: usize) -> Vec<Event> {
    let rec = sdrkit_dev_test::read_recording(&format!("lora/{name}"))
        .unwrap_or_else(|| panic!("the recording lora/{name} is missing"));
    feed(rx, &rec.samples, rec.rate_hz, rec.centre_hz, tuned, piece)
}

fn expected(name: &str) -> J {
    let e = sdrkit_dev_test::sigmf::read_expected("lora").expect("expected.json");
    e["recordings"][name].clone()
}

/// Every listed frame decoded with a good CRC and no other frame. The start within 70 ms and the SNR within 2.5 dB when given.
fn check_frames(name: &str, ev: &[Event], x: &J) {
    let frames = x["frames"].as_array().unwrap();
    let heard = packets(ev, None);
    for f in frames {
        let want = f["payload"].as_str().unwrap();
        let e = heard
            .iter()
            .find(|e| hex(e) == want)
            .unwrap_or_else(|| panic!("{name}: frame {want} not decoded"));
        assert_eq!(e.at(&LoraPacketEvent::CRC_OK), Some(true), "{name}: {want}");
        if let Some(t) = f["start_s"].as_f64() {
            assert!((e.time - t).abs() < 0.07, "{name}: start {} vs {t}", e.time);
        }
        if let Some(snr) = f["snr_db"].as_f64() {
            let got = e.get(&keys::SNR_DB).unwrap();
            assert!((got - snr).abs() <= 2.5, "{name}: snr {got} vs {snr}");
        }
    }
    assert!(
        heard.len() <= frames.len(),
        "{name}: {} packets for {} frames",
        heard.len(),
        frames.len()
    );
}

fn offsets(name: &str, ev: &[Event], x: &J, channel_hz: f64) {
    for f in x["frames"].as_array().unwrap() {
        let want = f["payload"].as_str().unwrap();
        let e = packets(ev, None)
            .into_iter()
            .find(|e| hex(e) == want)
            .unwrap();
        let ch = f["channel_hz"].as_f64().unwrap_or(channel_hz);
        let cfo = e.freq_hz - ch;
        let want = f["cfo_hz"].as_f64().unwrap();
        assert!(
            (cfo - want).abs() <= 150.0,
            "{name}: offset {cfo} vs {want}"
        );
    }
}

#[test]
fn meshtastic_longfast() {
    let x = expected("meshtastic_longfast_600k");
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &[]);
    let ev = run(rx.as_mut(), "meshtastic_longfast_600k", 869.525e6, 65536);
    check_frames("meshtastic", &ev, &x);
    offsets("meshtastic", &ev, &x, 869.525e6);
    let res = rx.results();
    for n in x["tables_at_end"]["nodes"].as_array().unwrap() {
        let id = n["id"].as_str().unwrap();
        let r = row(res, &MESH_NODES, |r| r.at(&MeshNodeRow::ID) == Some(id))
            .unwrap_or_else(|| panic!("node {id}"));
        if let Some(l) = n["long_name"].as_str() {
            assert_eq!(r.at(&MeshNodeRow::NAME), Some(l));
            assert_eq!(r.at(&MeshNodeRow::SHORT), n["short_name"].as_str());
            assert_eq!(r.at(&MeshNodeRow::HW), n["hw_model"].as_str());
            assert_eq!(r.at(&MeshNodeRow::ROLE), n["role"].as_str());
        }
        assert_eq!(r.get(&columns::HEARD), n["packets"].as_i64(), "{id}");
        assert_eq!(r.at(&MeshNodeRow::HOPS), n["hops_away"].as_i64(), "{id}");
    }
    assert_eq!(rows(res, &MESH_MESSAGES).len(), 2);
    let m = message(res, "Hello from sdrkit").expect("the text");
    assert_eq!(m.get(&keys::CHANNEL), Some("LongFast"));
    assert_eq!(sender(m), "Test Node Alpha");
    assert_eq!(m.get(&keys::TARGET), Some("^all"));
    let m = message(res, "relayed reply").expect("the relayed text");
    assert_eq!(m.at(&MeshNodeRow::HOPS), Some(1));
    for (k, v) in x["tables_at_end"]["type_counts"].as_object().unwrap() {
        let r = row(res, &MESH_TYPES, |r| {
            r.at(&MeshPacketRow::TYPE) == Some(k.as_str())
        })
        .unwrap_or_else(|| panic!("type {k}"));
        assert_eq!(r.at(&MeshTypeRow::COUNT), v.as_i64(), "{k}");
    }
    // The PKI direct message stays encrypted, with no text.
    let p = row(res, &MESH_PACKETS, |r| {
        r.at(&MeshPacketRow::TYPE) == Some("ENCRYPTED")
    })
    .unwrap();
    assert_eq!(p.at(&MeshPacketRow::SUMMARY), Some("encrypted (direct)"));
    assert_eq!(p.get(&keys::TARGET), Some("!7a6d648c"));
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(!ev.is_empty());
}

#[test]
fn meshcore_eu_narrow() {
    let x = expected("meshcore_eu_narrow_250k");
    // Without the hashtag channel: the third frame stays unreadable.
    let mut rx = receiver(&sdrkit_stack_lora::MESHCORE, Some("meshcore"), &[]);
    let ev = run(rx.as_mut(), "meshcore_eu_narrow_250k", 869.618e6, 30_000);
    check_frames("meshcore", &ev, &x);
    offsets("meshcore", &ev, &x, 869.618e6);
    let res = rx.results();
    let n = row(res, &MESH_NODES, |r| {
        r.at(&MeshNodeRow::ID) == Some("81e1aef4531c")
    })
    .expect("the repeater");
    assert_eq!(n.at(&MeshNodeRow::NAME), Some("Test Repeater"));
    assert_eq!(n.at(&MeshNodeRow::ROLE), Some("Repeater"));
    let m = message(res, "hello from sdrkit").expect("the Public text");
    assert_eq!(m.get(&keys::CHANNEL), Some("Public"));
    assert_eq!(sender(m), "Test Repeater");
    let p = row(res, &MESH_PACKETS, |r| {
        r.at(&MeshPacketRow::SUMMARY) == Some("unknown channel 0xd9")
    })
    .expect("the hashtag text unread");
    assert_eq!(p.at(&MeshPacketRow::TYPE), Some("GRP_TXT"));
    // With "#test" added: the text and its two hops.
    let mut rx = receiver(&sdrkit_stack_lora::MESHCORE, Some("meshcore"), &[]);
    invoke(
        rx.as_mut(),
        "mc.channel.add",
        &[("name", Value::Text("test".into()))],
    )
    .unwrap();
    run(rx.as_mut(), "meshcore_eu_narrow_250k", 869.618e6, 4096);
    let res = rx.results();
    let m = message(res, "channel talk").expect("the hashtag text");
    assert_eq!(m.get(&keys::CHANNEL), Some("#test"));
    assert_eq!(sender(m), "Sara");
    assert_eq!(m.at(&MeshNodeRow::HOPS), Some(2));
}

#[test]
fn lora_aprs_eu() {
    let x = expected("lora_aprs_eu_300k");
    let mut rx = receiver(&sdrkit_stack_lora::LORAAPRS, Some("loraaprs"), &[]);
    let ev = run(rx.as_mut(), "lora_aprs_eu_300k", 433.775e6, 65536);
    check_frames("lora aprs", &ev, &x);
    offsets("lora aprs", &ev, &x, 433.775e6);
    let res = rx.results();
    let s = row(res, &MESH_STATIONS, |r| {
        r.at(&MeshStationRow::CALL) == Some("N0CALL-7")
    })
    .expect("the station");
    assert_eq!(s.at(&MeshStationRow::SYMBOL), Some("/>"));
    assert_eq!(s.get(&columns::HEARD), Some(2));
    assert_eq!(s.at(&MeshPacketRow::TYPE), Some("Message"));
    let (lat, _) = s.get(&keys::POSITION).unwrap();
    assert!((lat - 52.37).abs() < 0.01, "{lat}");
    let m = message(res, "Test message").expect("the message");
    assert_eq!(m.get(&keys::TARGET), Some("N0CALL-9"));
}

#[test]
fn meshcom_eu() {
    let x = expected("meshcom_eu_600k");
    let mut rx = receiver(&sdrkit_stack_lora::MESHCOM, Some("meshcom"), &[]);
    let ev = run(rx.as_mut(), "meshcom_eu_600k", 433.175e6, 65536);
    check_frames("meshcom", &ev, &x);
    offsets("meshcom", &ev, &x, 433.175e6);
    let res = rx.results();
    let m = message(res, "hello from sdrkit").expect("the text");
    assert_eq!(m.get(&keys::CHANNEL), Some("MeshCom all"));
    let s = row(res, &MESH_STATIONS, |r| {
        r.at(&MeshStationRow::CALL) == Some("N0CALL-2")
    })
    .expect("the station");
    assert_eq!(s.at(&MeshNodeRow::BATTERY_PCT), Some(64.0));
    let (lat, _) = s.get(&keys::POSITION).unwrap();
    assert!((lat - 52.37).abs() < 0.01, "{lat}");
    let p = row(res, &MESH_PACKETS, |r| {
        r.at(&MeshPacketRow::TYPE) == Some("POSITION")
    })
    .unwrap();
    let summary = p.at(&MeshPacketRow::SUMMARY).unwrap_or("");
    assert!(summary.contains("battery 64 %"), "{summary}");
}

#[test]
fn lorawan_wide_and_one_channel() {
    let x = expected("lorawan_eu868_700k");
    let eu868 = settings_of(&data().lorawan_settings("EU868", 0).unwrap());
    let mut rx = receiver(&sdrkit_stack_lora::LORAWANWIDE, None, &eu868);
    let ev = run(rx.as_mut(), "lorawan_eu868_700k", 868.3e6, 65536);
    let heard = packets(&ev, None);
    for f in x["frames"].as_array().unwrap() {
        let want = f["payload"].as_str().unwrap();
        let e = heard
            .iter()
            .find(|e| hex(e) == want)
            .unwrap_or_else(|| panic!("frame {want}"));
        let cfo = e.freq_hz - f["channel_hz"].as_f64().unwrap();
        assert!(
            (cfo - f["cfo_hz"].as_f64().unwrap()).abs() <= 150.0,
            "{want}: {cfo}"
        );
        assert_eq!(e.at(&LoraPacketEvent::SF), f["sf"].as_i64());
    }
    assert_eq!(heard.len(), 4);
    let res = rx.results();
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    let id = |want: &'static str| move |r: &Record| r.at(&MeshNodeRow::ID) == Some(want);
    let d = row(res, &MESH_DEVICES, id("260B1234")).expect("the device");
    assert_eq!(d.at(&MeshDeviceRow::NETWORK), Some("The Things Network"));
    assert_eq!(
        (
            d.at(&MeshDeviceRow::UPLINKS),
            d.at(&MeshDeviceRow::DOWNLINKS),
            d.at(&MeshDeviceRow::JOINS)
        ),
        (Some(2), Some(1), Some(0))
    );
    assert_eq!(d.at(&MeshDeviceRow::FCNT), Some(7));
    assert_eq!(d.at(&MeshDeviceRow::MISSED), Some(0));
    let j = row(res, &MESH_DEVICES, id("DevEUI 70B3D57ED0123456")).expect("the joining device");
    assert_eq!(j.at(&MeshDeviceRow::JOINS), Some(1));
    let ty = |want: &'static str| move |r: &Record| r.at(&WideFrameRow::TYPE) == Some(want);
    let f = row(res, &WIDE_FRAMES, ty("Unconfirmed down")).expect("the downlink");
    assert!(f
        .at(&WideFrameRow::MAC_COMMANDS)
        .unwrap_or("")
        .contains("LinkADRReq (DR5, TX power 1, channel mask 0x0007)"));
    let f = row(res, &WIDE_FRAMES, ty("Confirmed up")).unwrap();
    assert_eq!(
        f.at(&WideFrameRow::MAC_COMMANDS),
        Some("DevStatusAns (battery 31 %, margin 10 dB), LinkCheckReq")
    );
    // The one-channel stack at 868.1 MHz hears the first frame.
    let mut rx = receiver(&sdrkit_stack_lora::LORAWAN, Some("lorawan"), &[]);
    let ev = run(rx.as_mut(), "lorawan_eu868_700k", 868.1e6, 65536);
    assert!(packets(&ev, None)
        .iter()
        .any(|e| hex(e) == "4034120b26806400011011121314151617a1b2c3d4"));
}

#[test]
fn lora_raw() {
    let x = expected("lora_raw_600k");
    let mut rx = receiver(&sdrkit_stack_lora::LORARAW, None, &[]);
    let ev = run(rx.as_mut(), "lora_raw_600k", 868.1e6, 65536);
    let heard = packets(&ev, None);
    let frames = x["frames"].as_array().unwrap();
    for f in frames.iter().take(3) {
        let want = f["payload"].as_str().unwrap();
        let e = heard
            .iter()
            .find(|e| hex(e) == want)
            .unwrap_or_else(|| panic!("frame {want}"));
        assert_eq!(e.at(&LoraPacketEvent::SF), f["sf"].as_i64());
        assert_eq!(e.at(&LoraPacketEvent::CRC), f["crc"].as_bool());
        let cfo = e.freq_hz - 868.1e6;
        assert!(
            (cfo - f["cfo_hz"].as_f64().unwrap()).abs() <= 150.0,
            "{cfo}"
        );
    }
    assert_eq!(rx.results().fields().get(&keys::FRAMES_BAD), Some(0));
    // The implicit frame needs the implicit settings.
    let mut rx = receiver(
        &sdrkit_stack_lora::LORARAW,
        None,
        &[
            CaseSetting::new(&IMPLICIT, true),
            CaseSetting::new(&AUTO_SF, false),
            CaseSetting::new(&SF, 8),
            CaseSetting::new(&AUTO_BW, false),
            CaseSetting::new(&BW, LoraBandwidth::Bw250),
            CaseSetting::new(&CR, 6),
            CaseSetting::new(&LENGTH, 12),
        ],
    );
    let ev = run(rx.as_mut(), "lora_raw_600k", 868.1e6, 65536);
    assert!(packets(&ev, None)
        .iter()
        .any(|e| Some(hex(e).as_str()) == frames[3]["payload"].as_str()));
}

#[test]
fn aprs_afsk1200() {
    let x = expected("aprs_afsk1200_240k");
    let mut rx = receiver(
        &sdrkit_stack_lora::APRS,
        None,
        &[CaseSetting::new(&MATCH_SSID, true)],
    );
    let station = sdrkit_core::Station {
        callsign: "N0CALL-9".into(),
        ..Default::default()
    };
    rx.set_station(&station);
    // The transmitter is on, so that the acknowledgement goes to the scheduler (no host stream takes it).
    rx.set_transmit(true);
    let ev = run(rx.as_mut(), "aprs_afsk1200_240k", 144.8e6, 24_000);
    let heard = events(&ev, &APRS_PACKET);
    for f in x["frames"].as_array().unwrap() {
        let tnc2 = f["tnc2"].as_str().unwrap();
        let (head, info) = tnc2.split_once(':').unwrap();
        let from = head.split('>').next().unwrap();
        assert!(
            heard.iter().any(|e| e.get(&keys::SOURCE) == Some(from)
                && e.at(&AprsPacketEvent::INFO) == Some(info)),
            "{tnc2} not decoded"
        );
    }
    assert_eq!(heard.len(), 3);
    let res = rx.results();
    assert_eq!(res.fields().at(&AprsFields::FRAMES_1200), Some(3));
    let s = row(res, &APRS_STATIONS, |r| {
        r.at(&AprsStationRow::CALL) == Some("N0CALL-9")
    })
    .expect("the station");
    assert_eq!(s.get(&columns::HEARD), Some(2));
    let (lat, _) = s.get(&keys::POSITION).unwrap();
    assert!((lat - 52.37).abs() < 0.01, "{lat}");
    let p = row(res, &APRS_FRAMES, |r| {
        r.at(&AprsFrameRow::TYPE) == Some("Position")
    })
    .unwrap();
    let summary = p.get(&keys::TEXT).unwrap_or("");
    assert!(summary.contains("36 kn"), "{summary}");
    assert!(count(res, &APRS_STATIONS, |_| true) >= 1);
    // The own station (N0CALL-9, from the station context) acknowledges the message once.
    let acks = events(&ev, &TX_REQUEST);
    assert_eq!(acks.len(), 1, "{acks:?}");
    assert_eq!(acks[0].at(&TxRequestEvent::WHAT), Some(TxKind::Ack));
}

#[test]
fn meshtastic_anz_tbeam() {
    let x = expected("meshtastic_anz_tbeam_2m");
    let region = settings_of(&data().meshtastic_settings("ANZ", "LongFast").unwrap());
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &region);
    let ev = run(rx.as_mut(), "meshtastic_anz_tbeam_2m", 919.875e6, 65536);
    check_frames("meshtastic anz", &ev, &x);
    offsets("meshtastic anz", &ev, &x, 919.875e6);
    let res = rx.results();
    assert_eq!(rows(res, &MESH_MESSAGES).len(), 1);
    let m = message(res, "Hello").expect("the text");
    assert_eq!(m.get(&keys::CHANNEL), Some("LongFast"));
    assert_eq!(sender(m), "!7a6bf034");
    assert_eq!(m.get(&keys::TARGET), Some("^all"));
    assert_eq!(m.at(&MeshNodeRow::HOPS), Some(0));
    let r = row(res, &MESH_NODES, |r| {
        r.at(&MeshNodeRow::ID) == Some("!7a6bf034")
    })
    .expect("the node");
    assert_eq!(r.get(&columns::HEARD), Some(2));
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
}
