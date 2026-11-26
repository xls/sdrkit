//! The `ble` stack: the advertising receiver through the graph, the device table, the settings and the recording.

use super::help::{
    cnum, command, config, feed, impaired, num, row_with, rows, set, snr, text, value,
};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, Results, SettingSet, StateBlob, Value};
use sdrkit_dev_test::Harness;
use sdrkit_proto_wlan_phy::ble::{ad, build_adv, PduType};
use sdrkit_stack_wlan::ble::gen::{
    addr, packet_samples, test_schedule, BleGenParams, BleGenerator, GenPacket, Whitening,
};
use sdrkit_stack_wlan::ble::table::AddressType;
use sdrkit_stack_wlan::ble::{self, adv, BleSetting};
use sdrkit_stack_wlan::typed::{BleDeviceRow as D, BlePacketRow as P, BlePduTypeRow, BLE_DEVICE};
use std::collections::BTreeMap;

const RATE: f64 = 4e6;

fn rx(settings: &[(&str, Value)]) -> sdrkit_graph::StackGraph {
    ble::receiver(&config(BleSetting::schema(), settings)).unwrap()
}

/// The good packets of each PDU type, from the `pdu_types` table.
fn pdu_types(r: &Results) -> BTreeMap<String, i64> {
    rows(r, "pdu_types")
        .into_iter()
        .map(|row| {
            (
                text(row, &P::PDU),
                row.at(&BlePduTypeRow::PACKETS).unwrap_or(0),
            )
        })
        .collect()
}

fn kitchen(gap: f64) -> GenPacket {
    let mut d = Vec::new();
    ad::push(&mut d, ad::FLAGS, &[6]).unwrap();
    ad::push(&mut d, ad::COMPLETE_NAME, b"Kitchen scale").unwrap();
    ad::push_manufacturer(&mut d, 0x0059, &[0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
    let mut p = Vec::new();
    build_adv(
        &mut p,
        PduType::AdvInd,
        &addr("C0:FF:EE:12:34:56", true),
        &d,
    )
    .unwrap();
    GenPacket::new(p, gap)
}

fn signal(p: BleGenParams, imp: &[(&str, Value)], seed: u64, seconds: f64) -> Vec<Complex32> {
    impaired(Box::new(BleGenerator::new(p)), imp, seed, seconds)
}

#[test]
fn a_change_of_frequency_keeps_the_devices_and_the_log_and_clears_the_measurements() {
    let x = signal(
        BleGenParams {
            packets: vec![kitchen(2e-3)],
            ..Default::default()
        },
        &snr(30.0),
        5,
        0.25,
    );
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 16384);
    let before = Owner::results(&r);
    let (devices, packets) = (rows(before, "devices").len(), rows(before, "packets").len());
    assert_eq!(devices, 1);
    assert!(packets > 10, "{packets}");
    assert!(num(before, "frames_ok").unwrap() > 10.0);
    assert!(value(before, "snr_db").is_some() && value(before, "level_dbfs").is_some());
    assert!(!pdu_types(before).is_empty());
    r.forget();
    let after = Owner::results(&r);
    assert_eq!(rows(after, "devices").len(), devices);
    assert_eq!(rows(after, "packets").len(), packets);
    assert!(!rows(after, "device_levels").is_empty());
    assert_eq!(num(after, "devices"), Some(1.0));
    assert_eq!(num(after, "frames_ok"), Some(0.0));
    assert_eq!(num(after, "aa_hits"), Some(0.0));
    assert_eq!(value(after, "snr_db"), None);
    assert_eq!(value(after, "offset_hz"), None);
    assert_eq!(value(after, "level_dbfs"), None);
    assert_eq!(value(after, "last_channel"), None);
    assert!(pdu_types(after).is_empty());
}

#[test]
fn each_advertising_channel_alone_with_the_live_hint() {
    for ch in [37u8, 38, 39] {
        let hz = sdrkit_proto_wlan_phy::ble::channel_hz(ch).unwrap();
        let p = BleGenParams {
            packets: vec![kitchen(2e-3)],
            whitening: Whitening::Fixed(ch),
            ..Default::default()
        };
        let x = signal(p, &snr(30.0), 3, 0.25);
        let mut r = rx(&[]);
        r.configure(&InputInfo::new(RATE, hz, hz)).unwrap();
        feed(&mut r, &x, 16384);
        let res = Owner::results(&r);
        assert_eq!(num(res, "channel"), Some(ch as f64));
        let devices = rows(res, "devices");
        assert_eq!(devices.len(), 1, "{ch}");
        let k = devices[0];
        assert_eq!(text(k, &D::NAME), "Kitchen scale");
        assert_eq!(text(k, &D::VENDOR), "Nordic Semiconductor ASA");
        assert_eq!(text(k, &D::MANUFACTURER_DATA), "DEADBEEF");
        assert_eq!(k.at(&D::ADDRESS_TYPE), Some(AddressType::RandomStatic));
        assert_eq!(k.at(&D::CONNECTABLE), Some(true));
        assert_eq!(text(k, &D::CHANNELS), ch.to_string());
        let ok = num(res, "frames_ok").unwrap();
        assert!(ok >= 90.0, "{ch}: {ok}");
        assert_eq!(num(res, &format!("packets_{ch}")), Some(ok));
        assert_eq!(num(res, "last_channel"), Some(ch as f64));
        assert_eq!(num(res, "frames_bad"), Some(0.0));
    }
}

#[test]
fn the_whole_schedule_without_a_hint() {
    let run = Harness::new(&ble::BLE)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 30.0)
        .seconds(0.55)
        .run()
        .unwrap();
    let r = &run.results;
    let b = row_with(r, "devices", &D::ADDRESS, "00:0B:A1:5E:00:01").unwrap();
    assert_eq!(b.at(&D::ADDRESS_TYPE), Some(AddressType::Public));
    assert!(text(b, &D::BEACON)
        .contains("iBeacon E2C56DB5-DFFB-48D2-B060-D0F5A71096E0 major 100 minor 7, -59 dBm"));
    let e = row_with(r, "devices", &D::ADDRESS, "3A:55:44:33:22:02").unwrap();
    assert_eq!(
        e.at(&D::ADDRESS_TYPE),
        Some(AddressType::NonResolvablePrivate)
    );
    let log: Vec<String> = rows(r, "packets")
        .iter()
        .map(|c| text(c, &P::DETAILS))
        .collect();
    for want in [
        "Eddystone URL https://example.com",
        "Eddystone TLM 3000 mV, 21.5 C",
        "Eddystone UID EDD1EBEAC04E5DEFA017",
        "connection request from 66:55:44:33:22:11 (not followed)",
        "extended, data on channel 12 (LE 2M), set 2; channel 12 is outside the capture",
    ] {
        assert!(log.iter().any(|l| l.contains(want)), "{want}");
    }
    let a = row_with(r, "devices", &D::ADDRESS, "C0:FF:EE:12:34:56").unwrap();
    // The advertising and the scan response give two complete names; the last one heard stays.
    assert!(["Test phone", "TestDev"].contains(&text(a, &D::NAME).as_str()));
    assert_eq!(a.at(&D::SCANNED), Some(true));
    assert!(text(a, &D::BEACON).contains("tx 4 dBm"));
    let types = pdu_types(r);
    for t in ["SCAN_REQ", "CONNECT_IND", "ADV_EXT_IND", "SCAN_RSP"] {
        assert!(types.contains_key(t), "{t} in {types:?}");
    }
    for ch in [37, 38, 39] {
        assert!(num(r, &format!("packets_{ch}")).unwrap() > 0.0);
    }
    let en = row_with(r, "devices", &D::ADDRESS, "5B:12:34:56:78:9A").unwrap();
    assert!(text(en, &D::BEACON).contains("Exposure Notification"));
    assert_eq!(text(en, &D::VENDOR), "Apple: Nearby Info");
    assert_eq!(num(r, "frames_bad"), Some(0.0));
    assert!(!run.events.is_empty());
    for e in &run.events {
        assert!(e.is(&BLE_DEVICE));
        assert!(e.freq_hz > 0.0);
    }
    assert!(r.status().summary.contains("devices"));
}

#[test]
fn a_bad_crc_is_counted_and_not_listed() {
    let mut d = Vec::new();
    ad::push(&mut d, ad::COMPLETE_NAME, b"broken").unwrap();
    let mut p = Vec::new();
    build_adv(
        &mut p,
        PduType::AdvNonconnInd,
        &addr("00:0B:A1:5E:00:01", false),
        &d,
    )
    .unwrap();
    let x = signal(
        BleGenParams {
            packets: vec![kitchen(2e-3), GenPacket::new(p, 2e-3).bad_crc()],
            ..Default::default()
        },
        &[],
        1,
        0.25,
    );
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 4096);
    let res = Owner::results(&r);
    assert!(row_with(res, "devices", &D::ADDRESS, "C0:FF:EE:12:34:56").is_some());
    assert!(row_with(res, "devices", &D::ADDRESS, "00:0B:A1:5E:00:01").is_none());
    assert!(num(res, "frames_ok").unwrap() >= 40.0);
    assert!(num(res, "frames_bad").unwrap() >= 40.0);
}

#[test]
fn carrier_offsets_at_low_snr() {
    for off in [50e3, -50e3] {
        let mut d = Vec::new();
        ad::push(&mut d, ad::FLAGS, &[4]).unwrap();
        ad::push(&mut d, ad::COMPLETE_NAME, b"Door sensor").unwrap();
        ad::push_manufacturer(&mut d, 0x0006, &[1, 2]).unwrap();
        let mut p = Vec::new();
        build_adv(
            &mut p,
            PduType::AdvNonconnInd,
            &addr("00:0B:A1:5E:00:01", false),
            &d,
        )
        .unwrap();
        let sched = vec![
            kitchen(1.5e-3),
            GenPacket::new(p, 1.5e-3).offset(-off / 5.0),
        ];
        let mut imp = snr(12.0);
        imp.push(("gen_offset_hz", Value::F64(off)));
        let x = signal(
            BleGenParams {
                packets: sched,
                whitening: Whitening::Fixed(38),
                ..Default::default()
            },
            &imp,
            4,
            0.5,
        );
        let mut r = rx(&[]);
        r.configure(&InputInfo::new(RATE, 2426e6, 2426e6)).unwrap();
        feed(&mut r, &x, 8192);
        let res = Owner::results(&r);
        let k = row_with(res, "devices", &D::ADDRESS, "C0:FF:EE:12:34:56").unwrap();
        let s = row_with(res, "devices", &D::ADDRESS, "00:0B:A1:5E:00:01").unwrap();
        assert_eq!(text(s, &D::NAME), "Door sensor");
        assert_eq!(text(s, &D::VENDOR), "Microsoft");
        let sent = 0.5 / (2.0 * 1.5e-3 + 0.25e-3 + 0.21e-3 + 12e-6);
        assert!(
            cnum(k, &D::HEARD) > 0.7 * sent,
            "{off}: {} of {sent}",
            cnum(k, &D::HEARD)
        );
        assert!(
            cnum(s, &D::HEARD) > 0.7 * sent,
            "{off}: {} of {sent}",
            cnum(s, &D::HEARD)
        );
    }
}

#[test]
fn ten_msps_with_offset_and_clock_error() {
    let rate = 10e6;
    let p = BleGenParams {
        rate_hz: rate,
        packets: vec![kitchen(2e-3)],
        whitening: Whitening::Fixed(39),
        ..Default::default()
    };
    let mut imp = snr(20.0);
    imp.push(("gen_clock_ppm", Value::F64(40.0)));
    // The station is 1 MHz below the centre, as the stack's tune offset puts it.
    imp.push(("gen_offset_hz", Value::F64(-1e6)));
    let x = signal(p, &imp, 2, 0.25);
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(rate, 2481e6, 2480e6)).unwrap();
    feed(&mut r, &x, 50_000);
    let res = Owner::results(&r);
    assert_eq!(rows(res, "devices").len(), 1);
    assert!(num(res, "frames_ok").unwrap() >= 80.0);
}

#[test]
fn noise_alone_decodes_nothing() {
    // The floor is -17 dBFS over 4 MHz: about -22 dBFS in the 1.3 MHz channel.
    let x = signal(
        BleGenParams {
            packets: Vec::new(),
            ..Default::default()
        },
        &[
            ("gen_noise", Value::Text("floor".into())),
            ("gen_noise_floor_dbfs", Value::F64(-17.0)),
        ],
        7,
        2.0,
    );
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    let res = Owner::results(&r);
    assert!(rows(res, "devices").is_empty());
    assert!(rows(res, "packets").is_empty());
    assert_eq!(num(res, "frames_ok"), Some(0.0));
    let l = num(res, "level_dbfs").unwrap();
    assert!((-24.0..-19.0).contains(&l), "{l}");
}

#[test]
fn cuts_do_not_change_the_result() {
    let x = signal(BleGenParams::default(), &[], 1, 0.12);
    let log = |piece: usize| {
        let mut r = rx(&[]);
        r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
            .unwrap();
        feed(&mut r, &x, piece);
        let res = Owner::results(&r);
        (
            num(res, "frames_ok"),
            num(res, "aa_hits"),
            rows(res, "packets")
                .iter()
                .map(|c| (cnum(c, &P::SIGNAL_S), text(c, &P::DETAILS)))
                .collect::<Vec<_>>(),
        )
    };
    let a = log(1);
    assert!(a.0.unwrap() > 20.0);
    assert_eq!(a, log(77_777));
}

#[test]
fn too_low_a_rate_is_refused() {
    let mut r = rx(&[]);
    let e = r.configure(&InputInfo::new(2e6, 0.0, 0.0)).unwrap_err();
    assert!(e.to_string().contains("2"), "{e}");
    assert_eq!(
        Owner::results(&r).status().summary,
        "Bluetooth LE needs a sample rate of at least 4 Msps"
    );
    assert!(!r.ready());
}

#[test]
fn clear_save_and_restore() {
    let x = signal(BleGenParams::default(), &[], 1, 0.2);
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    // The five devices of the legacy schedule, the extended advertiser whose data is on channel 20 and the nine of the kinds
    // schedule.
    let n = rows(Owner::results(&r), "devices").len();
    assert_eq!(n, 15);
    let log = rows(Owner::results(&r), "packets").len();
    let state = r.save_state().unwrap();
    let mut other = rx(&[]);
    other
        .configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    other.restore_state(&state).unwrap();
    assert_eq!(rows(Owner::results(&other), "devices").len(), 15);
    assert_eq!(rows(Owner::results(&other), "packets").len(), log);
    assert!(other
        .restore_state(&StateBlob::new("wlan.wifi_networks", 2, b"{}".to_vec()))
        .is_err());
    assert_eq!(rows(Owner::results(&other), "devices").len(), 15);
    command(&mut r, "clear", &[]).unwrap();
    assert!(rows(Owner::results(&r), "devices").is_empty());
    assert!(rows(Owner::results(&r), "packets").is_empty());
    assert_eq!(num(Owner::results(&r), "frames_ok"), Some(0.0));
    assert!(set(&mut r, "nonsense", Value::Bool(true)).is_err());
}

#[test]
fn display_filters_limit_the_device_rows() {
    let x = signal(BleGenParams::default(), &[], 1, 0.2);
    let count = |s: &[(&str, Value)]| {
        let mut r = rx(s);
        r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
            .unwrap();
        feed(&mut r, &x, 65536);
        rows(Owner::results(&r), "devices").len()
    };
    assert_eq!(count(&[]), 15);
    assert_eq!(count(&[("filter_text", Value::Text("c0:ff:EE".into()))]), 1);
    assert_eq!(count(&[("named_only", Value::Bool(true))]), 1);
    // The test signal is at full scale and the channel filter takes a little of it: no device reaches 0 dBFS.
    assert_eq!(count(&[("min_level_dbfs", Value::F64(0.0))]), 0);
    assert_eq!(count(&[("min_level_dbfs", Value::F64(-1.0))]), 2);
    assert_eq!(count(&[("min_level_dbfs", Value::F64(-20.0))]), 12);
}

#[test]
fn settings_are_strict_and_ask_the_host() {
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 2403e6, 2402e6)).unwrap();
    assert!(set(&mut r, "hop_seconds", Value::F64(0.5)).is_err());
    assert!(set(&mut r, "channel", Value::Text("40".into())).is_err());
    set(&mut r, "channel", Value::Text("38".into())).unwrap();
    set(&mut r, "hop", Value::Bool(true)).unwrap();
    let fed = feed(&mut r, &vec![Complex32::default(); 40_000], 40_000);
    assert_eq!(fed.requests.len(), 2);
    assert!(fed.requests.iter().all(|q| q.validate().is_ok()));
    match &fed.requests[0] {
        sdrkit_core::HostRequest::Retune { tuned_hz, .. } => assert_eq!(*tuned_hz, 2426e6),
        q => panic!("{q:?}"),
    }
    match &fed.requests[1] {
        sdrkit_core::HostRequest::Hop {
            channels, dwell_s, ..
        } => {
            assert_eq!(channels, &vec![2402e6, 2426e6, 2480e6]);
            assert_eq!(*dwell_s, 3.0);
        }
        q => panic!("{q:?}"),
    }
    assert_eq!(
        value(Owner::results(&r), "hopping"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn the_advertising_data_reader() {
    let mut d = Vec::new();
    ad::push(&mut d, ad::SHORT_NAME, b"Sho").unwrap();
    ad::push(&mut d, ad::COMPLETE_NAME, b"Full name").unwrap();
    ad::push(&mut d, ad::TX_POWER, &[127]).unwrap();
    ad::push_uuids16(&mut d, &[0x180F]).unwrap();
    let a = adv::parse_ad(&d);
    assert_eq!(a.name.as_deref(), Some("Full name"));
    assert!(a.name_complete);
    assert_eq!(a.tx_power, None);
    assert_eq!(a.services, vec!["Battery (0x180F)".to_string()]);
    assert_eq!(adv::eddystone_url(1, b"abc\x07"), "https://www.abc.com");
    // A structure that runs past the end stops the walk without reading beyond it.
    assert_eq!(
        adv::parse_ad(&[0x09, 0xFF, 0x4C, 0x00, 0x12]),
        adv::AdvInfo::default()
    );
}

#[test]
fn the_generator_declares_its_signal() {
    let g = BleGenerator::new(BleGenParams::default());
    let s = sdrkit_core::Generator::signal_info(&g).unwrap();
    assert_eq!(s.symbol_rate_hz, Some(1e6));
    let one = packet_samples(&test_schedule()[0], 37, RATE).unwrap();
    let p: f64 = one.iter().map(|x| x.norm_sqr() as f64).sum::<f64>() / one.len() as f64;
    assert!((p - 1.0).abs() < 1e-3, "{p}");
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec =
        sdrkit_dev_test::read_recording("wlan/ble_adv_4m").expect("the recording wlan/ble_adv_4m");
    let exp = sdrkit_dev_test::sigmf::read_expected("wlan").unwrap();
    let e = &exp["ble_adv_4m"];
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz - 1e6).recording())
        .unwrap();
    feed(&mut r, &rec.samples, 16384);
    let res = Owner::results(&r);
    let ok = num(res, "frames_ok").unwrap() as i64;
    assert!((80..=88).contains(&ok), "{ok}");
    assert!(num(res, "frames_bad").unwrap() <= 1.0);
    assert!(num(res, "aa_hits").unwrap() >= e["counters"]["aa_hits_min"].as_f64().unwrap());
    let types = pdu_types(res);
    for (name, want) in e["counters"]["per_pdu_type"].as_object().unwrap() {
        let want = want.as_i64().unwrap();
        let got = types.get(name).copied().unwrap_or(0);
        assert!((got - want).abs() <= 1, "{name}: {got} vs {want}");
    }
    for ch in ["37", "38", "39"] {
        let want = e["counters"]["per_channel"][ch].as_f64().unwrap();
        assert!(
            (num(res, &format!("packets_{ch}")).unwrap() - want).abs() <= 2.0,
            "{ch}"
        );
    }
    assert!(num(res, "snr_db").unwrap() >= 14.0);
    let dev = num(res, "deviation_hz").unwrap();
    assert!((160e3..=260e3).contains(&dev), "{dev}");
    let devs = e["devices"].as_array().unwrap();
    assert_eq!(rows(res, "devices").len(), devs.len());
    for w in devs {
        let a = w["addr"].as_str().unwrap();
        let g = row_with(res, "devices", &D::ADDRESS, a).unwrap_or_else(|| panic!("{a}"));
        for (k, col) in [
            ("name", &D::NAME),
            ("vendor", &D::VENDOR),
            ("mfr_hex", &D::MANUFACTURER_DATA),
        ] {
            if let Some(v) = w.get(k) {
                assert_eq!(text(g, col), v.as_str().unwrap(), "{a} {k}");
            }
        }
        if let Some(b) = w.get("beacon") {
            assert!(text(g, &D::BEACON).contains(b.as_str().unwrap()), "{a}");
        }
        let chans: Vec<String> = w["channels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .collect();
        assert_eq!(text(g, &D::CHANNELS), chans.join(" "), "{a}");
        let want = w["packets"].as_f64().unwrap();
        assert!(
            (cnum(g, &D::HEARD) - want).abs() <= 2.0,
            "{a}: {}",
            cnum(g, &D::HEARD)
        );
        let l = cnum(g, &D::LEVEL_DBFS);
        assert!((-12.0..=-9.0).contains(&l), "{a}: {l}");
    }
    let log: Vec<_> = rows(res, "packets").into_iter().rev().collect();
    for (i, w) in e["first_log_lines"].as_array().unwrap().iter().enumerate() {
        let g = log[i];
        assert!(
            (cnum(g, &P::SIGNAL_S) - w["t"].as_f64().unwrap()).abs() <= 0.0005,
            "line {i}: {}",
            cnum(g, &P::SIGNAL_S)
        );
        assert_eq!(cnum(g, &P::CHANNEL), w["ch"].as_f64().unwrap());
        assert_eq!(text(g, &P::PDU), w["pdu"].as_str().unwrap());
        assert_eq!(text(g, &D::ADDRESS), w["addr"].as_str().unwrap());
        assert_eq!(cnum(g, &P::LENGTH), w["len"].as_f64().unwrap());
        assert_eq!(
            text(g, &P::DETAILS),
            w["info"].as_str().unwrap(),
            "line {i}"
        );
    }
}

/// `x` moved by `hz` at `rate`.
fn shifted(x: &[Complex32], hz: f64, rate: f64) -> Vec<Complex32> {
    x.iter()
        .enumerate()
        .map(|(n, &s)| {
            let ph = std::f64::consts::TAU * hz * n as f64 / rate;
            s * Complex32::new(ph.cos() as f32, ph.sin() as f32)
        })
        .collect()
}

#[test]
fn extended_advertising_is_followed_on_the_secondary_channel_and_the_phys() {
    let rate = ble::PREFERRED_RATE_HZ;
    let p = BleGenParams {
        rate_hz: rate,
        ..Default::default()
    };
    // The radio 1 MHz above channel 37, as the stack asks: the test signal's channel 37 lands at -1 MHz.
    let x = shifted(&signal(p, &snr(30.0), 4, 0.6), -1e6, rate);
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(rate, 2403e6, 2402e6).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    let res = Owner::results(&r);
    assert_eq!(num(res, "channels_in_capture"), Some(2.0));
    let types = pdu_types(res);
    for t in ["ADV_EXT_IND", "AUX_ADV_IND", "AUX_CHAIN_IND", "AUX_SYNC_IND"] {
        assert!(types.get(t).copied().unwrap_or(0) > 0, "{t}: {types:?}");
    }
    let s = row_with(res, "devices", &D::ADDRESS, "E5:01:02:03:04:05").unwrap();
    assert_eq!(text(s, &D::NAME), "Ext sensor");
    assert_eq!(text(s, &D::PHYS), "LE 2M");
    assert_eq!(s.at(&D::EXTENDED), Some(true));
    assert_eq!(s.at(&D::SET_ID), Some(2));
    // The manufacturer data came in the AUX_CHAIN_IND.
    assert_eq!(text(s, &D::MANUFACTURER_DATA), "E77E010203040506");
    assert!(text(s, &D::BEACON).contains("tx -4 dBm"), "{}", text(s, &D::BEACON));
    let c = row_with(res, "devices", &D::ADDRESS, "C3:11:22:33:44:55").unwrap();
    assert_eq!(text(c, &D::NAME), "Coded beacon");
    assert_eq!(text(c, &D::PHYS), "LE Coded");
    assert_eq!(c.at(&D::PERIODIC_INTERVAL_MS), Some(80.0));
    let o = row_with(res, "devices", &D::ADDRESS, "D7:66:55:44:33:22").unwrap();
    assert_eq!(o.at(&D::DATA_OUTSIDE), Some(true));
    assert_eq!(s.at(&D::DATA_OUTSIDE), Some(false));
    assert!(num(res, "aux_followed").unwrap() >= 4.0);
    assert!(num(res, "aux_outside").unwrap() >= 1.0);
    assert!(num(res, "packets_2m").unwrap() >= 2.0);
    assert!(num(res, "packets_coded").unwrap() >= 2.0);
    assert!(num(res, "packets_secondary").unwrap() >= 4.0);
    assert_eq!(num(res, "periodic_trains"), Some(1.0));
    assert_eq!(num(res, "frames_bad"), Some(0.0));
    let log = rows(res, "packets");
    assert!(log.iter().any(|l| text(l, &P::PHY) == "LE Coded S=2"));
    // The train hops over channels 0 and 1 by channel selection algorithm #2 with the round as its event counter; only
    // channel 0 is in the capture, and only the train's predicted events open it.
    let sync: Vec<_> = log.iter().filter(|l| text(l, &P::PDU) == "AUX_SYNC_IND").collect();
    assert!(sync.iter().all(|l| cnum(l, &P::CHANNEL) == 0.0));
    let id = sdrkit_proto_wlan_phy::ble::csa::channel_identifier(ble::gen::TRAIN_AA);
    let map = sdrkit_proto_wlan_phy::ble::csa::ChannelMap::from_bytes([3, 0, 0, 0, 0]).unwrap();
    // Seven whole rounds in 0.6 s; the first event follows the SyncInfo of round 0.
    let on_0 = (0..7u16)
        .filter(|&k| sdrkit_proto_wlan_phy::ble::csa::csa2_channel(k, id, &map) == 0)
        .count();
    assert!(on_0 > 0);
    assert!(
        (sync.len() as i64 - on_0 as i64).abs() <= 1,
        "{} AUX_SYNC_IND, {on_0} events on channel 0",
        sync.len()
    );
}

#[test]
fn the_level_counts_the_channel_only_and_stays_at_or_below_full_scale() {
    let clean = signal(BleGenParams::default(), &[], 1, 0.2);
    let noisy = signal(BleGenParams::default(), &snr(25.0), 1, 0.2);
    // A tone at 1.8 MHz, outside the channel, 10 dB below the signal.
    let tone = |x: &[Complex32], a: f32| -> Vec<Complex32> {
        let mut ph = 0.0f64;
        x.iter()
            .map(|&s| {
                ph += std::f64::consts::TAU * 1.8e6 / RATE;
                s + Complex32::new(ph.cos() as f32, ph.sin() as f32) * a
            })
            .collect()
    };
    let levels = |x: &[Complex32]| {
        let mut r = rx(&[]);
        r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
            .unwrap();
        feed(&mut r, x, 65536);
        let mut v: Vec<(String, f64)> = rows(Owner::results(&r), "devices")
            .into_iter()
            .map(|d| (text(d, &D::ADDRESS), cnum(d, &D::LEVEL_DBFS)))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    let a = levels(&clean);
    assert_eq!(a.len(), 15);
    assert!(a.iter().all(|(_, l)| *l <= 0.0), "{a:?}");
    assert!(a.iter().filter(|(_, l)| *l > -0.5).count() == 2, "{a:?}");
    // The devices of `y` (at least `least` of them) have the levels of the clean signal.
    let close = |y: &[(String, f64)], least: usize, tol: f64, what: &str| {
        assert!(y.len() >= least, "{what}: {y:?}");
        for (k, q) in y {
            let p = a.iter().find(|(c, _)| c == k).map(|c| c.1).unwrap();
            assert!((p - q).abs() < tol, "{what}: {p} {q}");
        }
    };
    // The noise inside the channel is taken out: the level is the signal's, within the scatter of one packet. At 25 dB the
    // weakest devices are lost.
    close(&levels(&noisy), 11, 0.3, "noise");
    close(&levels(&tone(&noisy, 0.316)), 11, 0.3, "noise and a tone");
    // Without noise the faint residue of the tone that the channel filter lets through is all there is between packets.
    for amp in [0.0003, 0.003, 0.01, 0.316] {
        close(&levels(&tone(&clean, amp)), 15, 0.1, "a tone and no noise");
    }
}

#[test]
fn the_test_signal_has_each_kind_a_tracker_and_a_spread_of_levels() {
    let x = signal(BleGenParams::default(), &[], 1, 0.2);
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    let devices = rows(Owner::results(&r), "devices");
    for (i, k) in <ble::adv::DeviceKind as sdrkit_core::value::Choice>::CHOICES
        .iter()
        .enumerate()
    {
        assert!(
            devices
                .iter()
                .any(|d| super::help::cell(d, &D::DEVICE_KIND) == Some(&Value::Choice(i as u16))),
            "no device of kind {}",
            k.key
        );
    }
    let trackers: Vec<_> = devices
        .iter()
        .filter(|d| super::help::cell(d, &D::TRACKER) == Some(&Value::Bool(true)))
        .collect();
    assert_eq!(trackers.len(), 1);
    let l: Vec<f64> = devices.iter().map(|d| cnum(d, &D::LEVEL_DBFS)).collect();
    let (hi, lo) = l.iter().fold((f64::MIN, f64::MAX), |(h, m), &v| (h.max(v), m.min(v)));
    assert!(hi - lo > 28.0 && hi <= 0.0, "{hi} {lo}");
}

/// The connection schedule at 40 Msps, moved so that channel 37 lands at -9 MHz of a capture centred on 2411 MHz: channel 37
/// and data channels 0 to 8 are inside.
fn connection_capture(ch_sel: bool, snr_db: f64) -> sdrkit_graph::StackGraph {
    connection_capture_with(
        &ble::gen::ConnSchedule {
            ch_sel,
            ..Default::default()
        },
        snr_db,
        &[],
    )
}

/// As [`connection_capture`] for the schedule `c`, with the stack settings `settings`.
fn connection_capture_with(
    c: &ble::gen::ConnSchedule,
    snr_db: f64,
    settings: &[(&str, Value)],
) -> sdrkit_graph::StackGraph {
    let rate = 40e6;
    let p = BleGenParams {
        rate_hz: rate,
        looped: false,
        whitening: Whitening::Fixed(37),
        packets: ble::gen::connection_schedule_with(c),
    };
    let x = shifted(&signal(p, &snr(snr_db), 2, 0.15), -9e6, rate);
    let mut r = rx(settings);
    r.configure(&InputInfo::new(rate, 2411e6, 2402e6).recording())
        .unwrap();
    feed(&mut r, &x, 65536);
    r
}

#[test]
fn a_connection_is_followed_over_its_data_channels_until_it_is_encrypted() {
    use sdrkit_stack_wlan::typed::{BleAttRow as A, BleConnectionRow as C};
    for ch_sel in [false, true] {
        let r = connection_capture(ch_sel, 30.0);
        let res = Owner::results(&r);
        let conns = rows(res, "connections");
        assert_eq!(conns.len(), 1, "ChSel {ch_sel}");
        let c = conns[0];
        assert_eq!(text(c, &C::ACCESS_ADDRESS), "5A3C9617");
        assert_eq!(text(c, &C::CENTRAL), "C1:23:45:67:89:AB");
        assert_eq!(text(c, &C::PERIPHERAL), "D2:34:56:78:9A:BC");
        assert_eq!(cnum(c, &C::INTERVAL_MS), 10.0);
        assert_eq!(cnum(c, &C::CHANNELS_USED), 9.0);
        assert_eq!(cnum(c, &C::CSA), if ch_sel { 2.0 } else { 1.0 }, "ChSel {ch_sel}");
        assert_eq!(cnum(c, &C::PACKETS), 20.0, "ChSel {ch_sel}");
        assert_eq!(c.at(&C::ENCRYPTED), Some(true));
        let procs = text(c, &C::PROCEDURES);
        for p in ["LL_VERSION_IND", "LL_ENC_REQ", "LL_ENC_RSP", "LL_START_ENC_REQ"] {
            assert!(procs.contains(p), "{procs}");
        }
        assert_eq!(num(res, "connection_packets"), Some(20.0));
        // The ATT PDUs in the clear, oldest first; the ciphertext after the encryption start is not read.
        let mut att = rows(res, "att");
        att.reverse();
        let ops: Vec<String> = att.iter().map(|a| text(a, &A::OPCODE)).collect();
        assert_eq!(
            ops,
            [
                "ATT_READ_BY_GROUP_TYPE_REQ",
                "ATT_READ_BY_GROUP_TYPE_RSP",
                "ATT_READ_REQ",
                "ATT_READ_RSP",
                "ATT_HANDLE_VALUE_NTF"
            ]
        );
        let from: Vec<bool> = att
            .iter()
            .map(|a| a.at(&A::FROM_CENTRAL) == Some(true))
            .collect();
        assert_eq!(from, [true, false, true, false, false]);
        assert_eq!(text(att[4], &A::SERVICE), "Battery");
        assert_eq!(cnum(att[4], &A::HANDLE), 8.0);
        assert_eq!(text(att[4], &A::VALUE), "59");
    }
}

#[test]
fn a_connection_update_is_applied_at_its_instant() {
    use sdrkit_stack_wlan::typed::BleConnectionRow as C;
    let c = ble::gen::ConnSchedule {
        update: Some(ble::gen::ConnUpdate {
            instant: 6,
            interval: 6,
            win_offset: 1,
        }),
        ..Default::default()
    };
    let r = connection_capture_with(&c, 30.0, &[]);
    let res = Owner::results(&r);
    let row = rows(res, "connections")[0];
    // Every packet after the instant is heard on the new timing.
    assert_eq!(cnum(row, &C::PACKETS), 20.0);
    assert_eq!(cnum(row, &C::INTERVAL_MS), 7.5);
    assert!(text(row, &C::PROCEDURES).contains("LL_CONNECTION_UPDATE_IND"));
}

#[test]
fn a_connection_keeps_its_place_over_the_events_outside_the_capture() {
    use sdrkit_stack_wlan::typed::BleConnectionRow as C;
    // All 37 data channels: the capture holds channels 0 to 12 only.
    let c = ble::gen::ConnSchedule {
        map: [0xFF, 0xFF, 0xFF, 0xFF, 0x1F],
        ..Default::default()
    };
    let inside: Vec<u8> = ble::bank::channels_in_capture(2411e6, 40e6)
        .into_iter()
        .map(|(ch, _)| ch)
        .collect();
    let heard = (0..10u16)
        .filter(|&k| inside.contains(&ble::gen::conn_channel(&c, k)))
        .count();
    assert!(heard > 0 && heard < 10, "{heard}");
    let r = connection_capture_with(&c, 30.0, &[]);
    let row = rows(Owner::results(&r), "connections")[0];
    assert_eq!(cnum(row, &C::PACKETS), 2.0 * heard as f64);
    assert_eq!(cnum(row, &C::CHANNELS_USED), 37.0);
}

#[test]
fn without_att_values_the_log_keeps_handles_types_and_lengths() {
    use sdrkit_stack_wlan::typed::BleAttRow as A;
    let r = connection_capture_with(
        &ble::gen::ConnSchedule::default(),
        30.0,
        &[("att_values", Value::Bool(false))],
    );
    let att = rows(Owner::results(&r), "att");
    assert_eq!(att.len(), 5);
    assert!(att.iter().all(|a| text(a, &A::VALUE).is_empty()));
    let ntf = att
        .iter()
        .find(|a| text(a, &A::OPCODE) == "ATT_HANDLE_VALUE_NTF")
        .unwrap();
    assert_eq!((cnum(ntf, &A::HANDLE), cnum(ntf, &A::LENGTH)), (8.0, 1.0));
}

