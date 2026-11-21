//! `v2x`: the message readers on the published BSM example and a CAM, the half-clock OFDM path, the receiver on its test signal
//! with offsets and noise, and the data file.

use crate::{feed, noise, plan_file, receiver, results, rows, shift, signal};
use sdrkit_core::publish::keys;
use sdrkit_core::{Choice, InputInfo, LockState, SettingSet, Value, Values};
use sdrkit_proto_wlan_phy::ofdm::{build_ppdu, OfdmReceiver, Rate};
use sdrkit_stack_road::v2x::gen::{bsm_psdu, cam_psdu, GenRate, V2xSetting};
use sdrkit_stack_road::v2x::its::{read_bsm, read_cam, scan_mpdu, write_bsm, BsmTail, Found};
use sdrkit_stack_road::typed::{V2xFields, V2X_FRAMES, V2X_VEHICLES};
use sdrkit_stack_road::v2x::NOTE;
use sdrkit_stack_road::{descriptors, V2X, V2X_TEST_SIGNAL_HZ};

/// The BSM example frame: a MessageFrame with messageId 20 and a 37-octet BasicSafetyMessage.
const BSM_EXAMPLE: [u8; 40] = [
    0x00, 0x14, 0x25, 0x12, 0x00, 0x00, 0x0A, 0x25, 0x2F, 0x24, 0xE6, 0xD0, 0x3F, 0xEF, 0x9C, 0xCC,
    0x88, 0x74, 0x0D, 0x5F, 0x7F, 0xFF, 0xFF, 0xFF, 0xF0, 0x98, 0xD5, 0xF0, 0xFD, 0xFA, 0x1F, 0xA1,
    0x00, 0x7F, 0xFF, 0x00, 0x00, 0x4B, 0x10, 0x90,
];

/// A CAM built from the field layout: protocol 2, station 77, a car at -12.3, 45.6.
const CAM_EXAMPLE: [u8; 18] = [
    0x02, 0x02, 0x00, 0x00, 0x00, 0x4D, 0x12, 0x34, 0x00, 0x55, 0xCA, 0x02, 0x88, 0x10, 0xCE, 0xFA,
    0x80, 0x00,
];

#[test]
fn the_bsm_example_reads_as_published() {
    let b = read_bsm(&BSM_EXAMPLE).expect("a BSM");
    assert_eq!(b.msg_count, 72);
    assert_eq!(format!("{:08X}", b.id), "00002894");
    assert_eq!(b.sec_mark, 48_275);
    let (lat, lon) = b.position.expect("a position");
    assert!((lat - 40.236_412_7).abs() < 1e-9 && (lon + 83.366_735_1).abs() < 1e-9);
    assert_eq!(b.elevation_m, Some(275.0));
    assert_eq!(b.transmission, 7);
    assert!((b.speed_m_s.unwrap() - 6.1).abs() < 1e-9);
    assert!((b.heading_deg.unwrap() - 275.0).abs() < 1e-9);
    assert_eq!(
        b.text(),
        "message 72, elevation 275.0 m, speed 6.1 m/s, heading 275.0"
    );
    // The writer gives the example back, with the tail of the example (angle, accelerations, brakes, size).
    let mut w = Vec::new();
    write_bsm(&b, &BsmTail::default(), &mut w);
    assert_eq!(w, BSM_EXAMPLE);
    // A length form over two octets that the frame does not hold, and a third length form, are refused.
    let mut long = BSM_EXAMPLE.to_vec();
    long[2] = 0x80;
    assert!(read_bsm(&long).is_none());
    long[2] = 0xC0;
    assert!(read_bsm(&long).is_none());
}

#[test]
fn the_cam_reads_its_header_and_basic_container() {
    let c = read_cam(&CAM_EXAMPLE).expect("a CAM");
    assert_eq!((c.protocol, c.station_id, c.station_type), (2, 77, 5));
    let (lat, lon) = c.position.expect("a position");
    assert!((lat + 12.3).abs() < 1e-9 && (lon - 45.6).abs() < 1e-9);
    assert_eq!(c.text(), "station type car");
    for (at, v) in [(0, 0), (0, 4), (1, 1), (1, 3)] {
        let mut x = CAM_EXAMPLE;
        x[at] = v;
        assert!(read_cam(&x).is_none(), "byte {at} = {v}");
    }
}

#[test]
fn the_test_frames_hold_the_messages() {
    let strip = |p: Vec<u8>| p[..p.len() - 4].to_vec();
    match scan_mpdu(&strip(bsm_psdu())) {
        Some(Found::Bsm(b)) => assert_eq!(b, read_bsm(&BSM_EXAMPLE).unwrap()),
        other => panic!("{other:?}"),
    }
    match scan_mpdu(&strip(cam_psdu())) {
        Some(Found::Cam(c)) => {
            assert_eq!(c.station_id, 0xCA_FE01);
            let (lat, lon) = c.position.unwrap();
            assert!((lat - 54.68).abs() < 1e-9 && (lon - 25.28).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    // The DENM port inside a header does not hide the CAM after it; alone it is a DENM.
    let mut f = strip(cam_psdu());
    f[50] = 0x07;
    f[51] = 0xD2;
    assert!(matches!(scan_mpdu(&f), Some(Found::Cam(_))));
    // The CAM port sits before the 30 octets of the CAM.
    let at = f.len() - 34;
    assert_eq!(f[at..at + 2], [0x07, 0xD1]);
    f[at] = 0x00;
    f[at + 1] = 0x00;
    assert!(matches!(scan_mpdu(&f), Some(Found::Denm)), "{:?}", scan_mpdu(&f));
    // A frame that is not data, and data with another EtherType, are ignored.
    let mut f = strip(cam_psdu());
    f[0] = 0x80;
    assert!(scan_mpdu(&f).is_none());
    let mut f = strip(cam_psdu());
    f[32] = 0x08;
    f[33] = 0x00;
    assert!(scan_mpdu(&f).is_none());
}

#[test]
fn the_20_msps_receiver_takes_a_half_clock_frame() {
    // The PPDU built for 20 Msps and played at 10 Msps is the 10 MHz frame: the receiver sees the same samples.
    let psdu = bsm_psdu();
    let mut x = vec![sdrkit_core::Complex32::default(); 2000];
    build_ppdu(&psdu, Rate::R12, 0x5D, &mut x).unwrap();
    x.extend(std::iter::repeat_n(sdrkit_core::Complex32::default(), 2000));
    let mut rx = OfdmReceiver::new();
    let mut got = Vec::new();
    rx.process(&x, |f| got.push(f.mpdu.to_vec()));
    assert_eq!(got, [psdu[..psdu.len() - 4].to_vec()]);
}

fn settings(rate: GenRate) -> Values {
    let mut v = Values::defaults(&V2xSetting::schema());
    v.set(V2xSetting::GenRate.id(), &Value::Choice(rate.index()))
        .unwrap();
    v
}

#[test]
fn both_frames_at_both_rates_both_sample_rates_and_offsets() {
    for rate in [GenRate::Mbps3, GenRate::Mbps6] {
        for fs in [10e6, 20e6] {
            for cfo in [0.0, 40e3] {
                let mut x = signal(&V2X, fs, 0.25, settings(rate));
                shift(&mut x, cfo, fs);
                noise(&mut x, 30.0, fs, 3);
                let mut rx = receiver(&V2X, fs, V2X_TEST_SIGNAL_HZ, V2X_TEST_SIGNAL_HZ);
                feed(rx.as_mut(), &x, fs);
                let r = results(rx.as_ref());
                let what = format!("{rate:?} at {fs}, {cfo} Hz");
                let v = rows(r, &V2X_VEHICLES);
                assert_eq!(v.len(), 2, "{what}");
                let bsm = v
                    .iter()
                    .find(|row| row.get(&keys::SOURCE) == Some("00002894"))
                    .expect("the BSM vehicle");
                let (lat, _) = bsm.get(&keys::POSITION).unwrap();
                assert!((lat - 40.236_412_7).abs() < 1e-7, "{what}");
                let cam = v
                    .iter()
                    .find(|row| row.get(&keys::SOURCE) == Some("13303297"))
                    .expect("the CAM station");
                let (lat, _) = cam.get(&keys::POSITION).unwrap();
                assert!((lat - 54.68).abs() < 1e-7, "{what}");
                let off = r.fields().get(&keys::OFFSET_HZ).unwrap();
                assert!((off - cfo).abs() < 2e3, "{what}: offset {off}");
                assert_eq!(r.points().len(), 2, "{what}");
            }
        }
    }
}

#[test]
fn qpsk_at_8_db_and_noise_alone() {
    let fs = 10e6;
    let mut x = signal(&V2X, fs, 0.4, settings(GenRate::Mbps6));
    noise(&mut x, 8.0, fs, 5);
    let mut rx = receiver(&V2X, fs, V2X_TEST_SIGNAL_HZ, V2X_TEST_SIGNAL_HZ);
    feed(rx.as_mut(), &x, fs);
    let r = results(rx.as_ref());
    assert_eq!(r.fields().at(&V2xFields::SAFETY_MESSAGES), Some(8));
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(r.status().summary, "8 safety messages");

    let mut z = vec![sdrkit_core::Complex32::default(); (fs * 0.5) as usize];
    noise(&mut z, 0.0, fs, 9);
    let mut rx = receiver(&V2X, fs, V2X_TEST_SIGNAL_HZ, V2X_TEST_SIGNAL_HZ);
    let out = feed(rx.as_mut(), &z, fs);
    let r = results(rx.as_ref());
    assert!(out.events.is_empty());
    assert!(rows(r, &V2X_FRAMES).is_empty());
    assert_eq!(r.fields().get(&keys::FRAMES_OK), Some(0));
    assert_ne!(r.status().lock_state, LockState::Locked);
}

#[test]
fn the_stack_rules() {
    assert_eq!(descriptors().len(), 1);
    let mut rx = (V2X.make_receiver)(&sdrkit_core::StackConfig::new(Values::defaults(
        &V2xSetting::schema(),
    )))
    .unwrap();
    assert!(rx
        .configure(&InputInfo::new(5e6, V2X_TEST_SIGNAL_HZ, V2X_TEST_SIGNAL_HZ).recording())
        .is_err());
    assert!(results(rx.as_ref())
        .status()
        .summary
        .contains("needs a sample rate of at least"));
    for k in ["clear", "remove"] {
        assert!(rx.commands().id(k).is_ok(), "no command {k}");
    }
    // The receiver says what no public document confirms, and the vehicle event carries the position and the id only.
    let x = signal(&V2X, 10e6, 0.25, settings(GenRate::Mbps6));
    let mut rx = receiver(&V2X, 10e6, V2X_TEST_SIGNAL_HZ, V2X_TEST_SIGNAL_HZ);
    let out = feed(rx.as_mut(), &x, 10e6);
    let r = results(rx.as_ref());
    assert_eq!(
        r.fields().at(&V2xFields::NOTE),
        Some(NOTE)
    );
    assert!(NOTE.starts_with("Not confirmed by a public document"));
    assert_eq!(out.events.len(), 2);
    for e in &out.events {
        assert!(e.get(&keys::POSITION).is_some());
        assert!(e.get(&keys::SOURCE).is_some());
    }
    // The saved tables come back after a clear.
    let saved = rx.save_state().expect("a saved state");
    let id = rx.commands().id("clear").unwrap();
    let args = rx.commands().defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), id, &args).unwrap();
    feed(rx.as_mut(), &[], 10e6);
    assert!(rows(results(rx.as_ref()), &V2X_VEHICLES).is_empty());
    rx.restore_state(&saved).unwrap();
    feed(rx.as_mut(), &[], 10e6);
    assert_eq!(rows(results(rx.as_ref()), &V2X_VEHICLES).len(), 2);
}

#[test]
fn the_data_file_has_the_its_channels() {
    let file = plan_file("v2x.json");
    assert_eq!(file.groups.len(), 3);
    let eu = &file.groups[1].bands[0];
    assert_eq!(eu.channels.len(), 7);
    assert_eq!(eu.channels[4].centre_hz, 5900e6);
    assert_eq!(file.groups[2].bands[0].channels.len(), 3);
    assert_eq!(file.groups[0].bands[0].start_hz, 5800e6);
}
