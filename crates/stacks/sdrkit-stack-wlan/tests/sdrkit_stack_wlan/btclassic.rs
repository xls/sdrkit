//! Bluetooth Classic: the test signal through the receiver at several rates and centres, with noise, a wideband burst and
//! noise alone; the rate refused below 4 Msps; the commands, the saved state and the data file.

use super::help::{config, feed, impaired, num, rows, snr, text};
use sdrkit_core::{Complex32, Generator, InputInfo, LockState, Receiver, SettingSet, Value};
use sdrkit_stack_wlan::btclassic::gen::{BtGenerator, TEST_CENTRE_HZ};
use sdrkit_stack_wlan::btclassic::{self as bt, BtSetting};
use sdrkit_stack_wlan::typed::{BtclassicDeviceRow as Dev, BtclassicFields, BtclassicPacketRow};

fn generator(rate: f64, settings: &[(&str, Value)]) -> Box<dyn Generator> {
    let v = super::help::values(&BtSetting::schema(), settings);
    Box::new(BtGenerator::from_config(&sdrkit_core::GenConfig::new(rate, v)).unwrap())
}

fn receiver(rate: f64, centre: f64) -> Box<dyn Receiver> {
    let mut rx = bt::receiver(&config(BtSetting::schema(), &[])).unwrap();
    rx.configure(&InputInfo::new(rate, centre, centre).recording())
        .unwrap();
    Box::new(rx)
}

/// The LAPs of the device table.
fn laps(rx: &dyn Receiver) -> Vec<String> {
    let mut v: Vec<String> = rows(rx.results(), "devices")
        .iter()
        .map(|r| text(r, &Dev::LAP))
        .collect();
    v.sort();
    v
}

const ALL: [&str; 5] = ["0F0E0D", "3C5A96", "9E8B33", "A1B2C3", "E17B24"];

fn check_all(rx: &dyn Receiver) {
    assert_eq!(laps(rx), ALL);
    let r = rx.results();
    let phone = rows(r, "devices")
        .into_iter()
        .find(|d| text(d, &Dev::LAP) == "A1B2C3")
        .unwrap();
    assert_eq!(text(phone, &Dev::ADDRESS), "12:34:5E:A1:B2:C3");
    assert_eq!(
        text(phone, &Dev::CLASS),
        "Phone: smartphone (Object transfer, Telephony)"
    );
    let headset = rows(r, "devices")
        .into_iter()
        .find(|d| text(d, &Dev::LAP) == "0F0E0D")
        .unwrap();
    assert_eq!(text(headset, &Dev::ADDRESS), "C0:DE:7A:0F:0E:0D");
    assert!(num(r, "inquiry_responses").unwrap() >= 4.0);
    assert!(num(r, "inquiry_ids").unwrap() >= 2.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!(text(rows(r, "packets")[0], &BtclassicPacketRow::TEXT).len() > 5);
    assert!(matches!(
        super::help::value(r, "note"),
        Some(Value::Text(t)) if t.starts_with("Not confirmed by a public document")
    ));
}

#[test]
fn every_lap_and_every_response_at_20_8_and_4_msps() {
    for (rate, centre) in [
        (20e6, TEST_CENTRE_HZ),
        (8e6, TEST_CENTRE_HZ),
        (4e6, TEST_CENTRE_HZ),
    ] {
        let x = impaired(generator(rate, &[]), &[], 1, 2.2);
        let mut rx = receiver(rate, centre);
        feed(rx.as_mut(), &x, 65_536);
        check_all(rx.as_ref());
        let covered = num(rx.results(), "channels_covered").unwrap();
        assert!((3.0..=19.0).contains(&covered), "{covered}");
    }
}

#[test]
fn a_radio_offset_and_noise_lose_nothing_and_add_nothing() {
    let rate = 20e6;
    for (offset, db) in [(50e3, 22.0), (-100e3, 16.0)] {
        let mut s = snr(db);
        s.push(("gen_offset_hz", Value::F64(offset)));
        let x = impaired(generator(rate, &[]), &s, 3, 2.2);
        let mut rx = receiver(rate, TEST_CENTRE_HZ);
        feed(rx.as_mut(), &x, 65_536);
        assert_eq!(laps(rx.as_ref()), ALL, "offset {offset}, {db} dB");
    }
    // 10 dB: some heard, nothing wrong.
    let x = impaired(generator(rate, &[]), &snr(10.0), 5, 2.2);
    let mut rx = receiver(rate, TEST_CENTRE_HZ);
    feed(rx.as_mut(), &x, 65_536);
    assert!(laps(rx.as_ref()).iter().all(|l| ALL.contains(&l.as_str())));
}

#[test]
fn wideband_bursts_are_skipped_and_counted() {
    let rate = 20e6;
    let x = impaired(
        generator(rate, &[("gen_wideband", Value::Bool(true))]),
        &[],
        1,
        2.2,
    );
    let mut rx = receiver(rate, TEST_CENTRE_HZ);
    feed(rx.as_mut(), &x, 65_536);
    assert!(num(rx.results(), "bursts_wideband").unwrap() >= 2.0);
    assert_eq!(laps(rx.as_ref()), ALL);
}

#[test]
fn noise_alone_gives_nothing() {
    let rate = 20e6;
    let mut x = vec![Complex32::default(); (rate * 1.0) as usize];
    let mut n = sdrkit_algo_sim::noise::Awgn::with_power(1e-3, 7).unwrap();
    n.process(&mut x);
    let mut rx = receiver(rate, TEST_CENTRE_HZ);
    feed(rx.as_mut(), &x, 65_536);
    assert!(laps(rx.as_ref()).is_empty());
    assert_eq!(num(rx.results(), "access_codes"), Some(0.0));
}

#[test]
fn a_rate_below_4_msps_is_refused_and_an_unknown_centre_is_relative() {
    let mut rx = bt::receiver(&config(BtSetting::schema(), &[])).unwrap();
    assert!(rx
        .configure(&InputInfo::new(2e6, TEST_CENTRE_HZ, TEST_CENTRE_HZ))
        .is_err());
    let rate = 8e6;
    let x = impaired(generator(rate, &[]), &[], 1, 1.2);
    let mut rx = receiver(rate, 0.0);
    feed(rx.as_mut(), &x, 65_536);
    let r = rx.results();
    assert_eq!(
        r.fields().at(&BtclassicFields::CHANNELS_RELATIVE),
        Some(true),
        "no centre: channel numbers relative"
    );
    assert!(!laps(rx.as_ref()).is_empty());
}

#[test]
fn clear_and_saved_state() {
    let rate = 8e6;
    let x = impaired(generator(rate, &[]), &[], 1, 1.2);
    let mut rx = receiver(rate, TEST_CENTRE_HZ);
    feed(rx.as_mut(), &x, 65_536);
    let n = laps(rx.as_ref()).len();
    assert!(n >= 4);
    let saved = rx.save_state().unwrap();
    super::help::command(rx.as_mut(), "clear_devices", &[]).unwrap();
    feed(rx.as_mut(), &vec![Complex32::default(); 100_000], 65_536);
    assert!(laps(rx.as_ref()).is_empty());
    let mut back = receiver(rate, TEST_CENTRE_HZ);
    back.restore_state(&saved).unwrap();
    feed(back.as_mut(), &vec![Complex32::default(); 100_000], 65_536);
    assert_eq!(laps(back.as_ref()).len(), n);
}

#[test]
fn the_data_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/btclassic.json");
    let text = std::fs::read_to_string(path).unwrap();
    let file = sdrkit_data_plans::data::parse("btclassic.json", &text).unwrap();
    let bands = &file.groups[0].bands;
    assert_eq!(bands.len(), 1);
    assert_eq!((bands[0].start_hz, bands[0].end_hz), (2401.5e6, 2480.5e6));
}
