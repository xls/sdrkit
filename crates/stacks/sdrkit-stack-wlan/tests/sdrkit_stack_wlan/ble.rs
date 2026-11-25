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
    // The five devices of the legacy schedule and the extended advertiser whose data is on channel 20.
    let n = rows(Owner::results(&r), "devices").len();
    assert_eq!(n, 6);
    let log = rows(Owner::results(&r), "packets").len();
    let state = r.save_state().unwrap();
    let mut other = rx(&[]);
    other
        .configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    other.restore_state(&state).unwrap();
    assert_eq!(rows(Owner::results(&other), "devices").len(), 6);
    assert_eq!(rows(Owner::results(&other), "packets").len(), log);
    assert!(other
        .restore_state(&StateBlob::new("wlan.wifi_networks", 2, b"{}".to_vec()))
        .is_err());
    assert_eq!(rows(Owner::results(&other), "devices").len(), 6);
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
    assert_eq!(count(&[]), 6);
    assert_eq!(count(&[("filter_text", Value::Text("c0:ff:EE".into()))]), 1);
    assert_eq!(count(&[("named_only", Value::Bool(true))]), 1);
    // The test signal is at full scale: at most a device or two reach 0 dBFS, none above.
    assert!(count(&[("min_level_dbfs", Value::F64(0.0))]) <= 2);
    assert_eq!(count(&[("min_level_dbfs", Value::F64(-1.0))]), 6);
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
