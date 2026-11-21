//! Drone Remote ID: the recorded vector, the receiver acceptance cases and the generator round trips.

use super::{count, row, rows, run};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::keys;
use sdrkit_core::{Choice, InputInfo, Output, Receiver, Record, Results, Value};
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::remote_id::table::*;
use sdrkit_stack_drones::remote_id::{plan, receiver, Band};
use sdrkit_stack_drones::remote_id::{
    GenCarriers, GenWifiPhy, GEN_CARRIERS, GEN_CENTRE_MHZ, GEN_WIFI_PHY,
};
use sdrkit_stack_drones::REMOTE_ID;
use sdrkit_util_geo::KNOT_M_S;

const DRONE1: &str = "C6:12:34:56:78:9A";
const DRONE2: &str = "60:60:1F:AA:BB:CC";

fn drone<'r>(r: &'r Results, id: &str) -> &'r Record {
    row(r, &typed::REMOTE_ID_DRONES, id).unwrap_or_else(|| panic!("no drone {id}: {:?}", rows(r, &typed::REMOTE_ID_DRONES)))
}

fn check_drone1(r: &Results) {
    let d = drone(r, DRONE1);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::SERIAL), Some("1581F5FKD229400B"));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::SESSION_ID), Some("SESSION-7F3A"));
    assert_eq!(d.get(&keys::SOURCE), Some("1581F5FKD229400B"));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::UA_TYPE), Some(2), "multirotor / helicopter");
    assert_eq!(d.at(&typed::RemoteIdDroneRow::STATUS), Some(2), "airborne");
    let (lat, lon) = d.get(&keys::POSITION).expect("a position");
    assert!((lat - 54.6872).abs() < 1e-6 && (lon - 25.2797).abs() < 1e-6);
    assert!((d.at(&typed::RemoteIdDroneRow::ALT_GEO_M).unwrap() - 245.0).abs() <= 0.5);
    assert!((d.at(&typed::RemoteIdDroneRow::HEIGHT_M).unwrap() - 120.0).abs() <= 0.5);
    assert!((d.get(&keys::SPEED_KT).unwrap() * KNOT_M_S - 8.5).abs() <= 0.25);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::HORIZ_ACC), Some(10), "< 10 m");
    let (a, o) = d.at(&typed::RemoteIdDroneRow::OPERATOR_POSITION).expect("an operator");
    assert!((a - 54.686).abs() < 1e-6 && (o - 25.277).abs() < 1e-6);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::CLASS), Some(2), "C1");
    assert_eq!(d.at(&typed::RemoteIdDroneRow::OPERATOR_ID), Some("LTUabcd1234efgh"));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::SELF_ID), Some("Survey flight"));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::AUTHENTICATION), Some(1), "UAS ID signature");
    assert_eq!(d.at(&typed::RemoteIdDroneRow::VIA), Some(Via::Ble));
}

fn check_drone2(r: &Results) {
    let d = drone(r, DRONE2);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::CAA_REGISTRATION), Some("FIN87astrdge12k8"));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::SERIAL), None);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::UA_TYPE), Some(1), "aeroplane");
    assert_eq!(d.at(&typed::RemoteIdDroneRow::HEIGHT_TYPE), Some(1), "above ground");
    assert!((d.get(&keys::SPEED_KT).unwrap() * KNOT_M_S - 69.75).abs() <= 0.25);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::VSPEED_M_S), None);
    assert_eq!(d.at(&typed::RemoteIdDroneRow::CATEGORY), Some(2), "specific");
    assert_eq!(d.at(&typed::RemoteIdDroneRow::VIA), Some(Via::Wifi));
    assert_eq!(d.at(&typed::RemoteIdDroneRow::AUTHENTICATION), None);
}

#[test]
fn plans_follow_the_centre() {
    let p = plan(2432e6);
    assert_eq!(p.ble, Some((38, -6e6)));
    assert_eq!(p.wifi, (6, 5e6));
    assert_eq!(plan(2412e6).wifi, (1, 0.0));
    assert_eq!(plan(2478e6).ble.map(|b| b.0), Some(39));
    assert_eq!(plan(2437e6).ble, None);
    assert_eq!(plan(0.0).wifi, (0, 0.0));
}

#[test]
fn both_carriers_decode_from_the_test_signal() {
    let out = run(Harness::new(&REMOTE_ID).seconds(0.1));
    let r = &out.results;
    check_drone1(r);
    check_drone2(r);
    assert!(count(r, &typed::RemoteIdFields::BLE_GOOD) > count(r, &typed::RemoteIdFields::BLE_REMOTE_ID));
    assert!(count(r, &typed::RemoteIdFields::WIFI_GOOD) > count(r, &typed::RemoteIdFields::WIFI_REMOTE_ID));
    assert_eq!(count(r, &typed::RemoteIdFields::DRONES), 2);
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(!rows(r, &typed::REMOTE_ID_TRACKS).is_empty());
    assert!(r.points().contains_key(&format!("drone/{DRONE1}")));
}

#[test]
fn ble_only_and_wifi_only_in_noise() {
    let out = run(Harness::new(&REMOTE_ID)
        .set(&GEN_CARRIERS, GenCarriers::Ble)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .seconds(0.1));
    check_drone1(&out.results);
    assert_eq!(rows(&out.results, &typed::REMOTE_ID_DRONES), [DRONE1]);
    let out = run(Harness::new(&REMOTE_ID)
        .set(&GEN_CARRIERS, GenCarriers::Wifi)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 12.0)
        .seconds(0.1));
    check_drone2(&out.results);
}

#[test]
fn ofdm_beacons_reach_remote_id() {
    let out = run(Harness::new(&REMOTE_ID)
        .set(&GEN_CARRIERS, GenCarriers::Wifi)
        .set(&GEN_WIFI_PHY, GenWifiPhy::Ofdm)
        .seconds(0.05));
    check_drone2(&out.results);
}

#[test]
fn a_file_without_a_centre_tries_every_advertising_channel() {
    let out = run(Harness::new(&REMOTE_ID)
        .set(&GEN_CENTRE_MHZ, 0.0)
        .freq(0.0)
        .seconds(0.06));
    check_drone1(&out.results);
    check_drone2(&out.results);
}

#[test]
fn noise_alone_returns_nothing() {
    let x = Harness::new(&REMOTE_ID)
        .set(&GEN_CARRIERS, GenCarriers::Ble)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, -50.0)
        .seconds(0.05)
        .generate()
        .unwrap();
    let run = super::feed(&Harness::new(&REMOTE_ID), x);
    assert_eq!(count(&run.results, &typed::RemoteIdFields::DRONES), 0);
    assert_eq!(
        run.results.status().lock_state,
        sdrkit_core::LockState::Searching
    );
}

#[test]
fn other_rates_and_cuts_give_the_same_drones() {
    let a = run(Harness::new(&REMOTE_ID).seconds(0.06).piece(4099));
    let b = run(Harness::new(&REMOTE_ID).seconds(0.06).piece(65536));
    assert_eq!(count(&a.results, &typed::RemoteIdFields::MESSAGES), count(&b.results, &typed::RemoteIdFields::MESSAGES));
    let c = super::run(Harness::new(&REMOTE_ID).rate(25e6).seconds(0.06));
    check_drone1(&c.results);
    check_drone2(&c.results);
}

#[test]
fn a_band_change_asks_the_host_to_retune() {
    let mut rx = receiver(&sdrkit_core::StackConfig::new(
        Harness::new(&REMOTE_ID).values().unwrap(),
    ))
    .unwrap();
    rx.configure(&InputInfo::new(20e6, 2432e6, 2432e6).recording())
        .unwrap();
    let band = rx.schema().id("band").unwrap();
    owner::apply(&mut rx, band, &Value::Choice(Band::B2412.index())).unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Default::default(); 200_000], &mut out);
    assert!(
        out.requests
            .iter()
            .any(|r| format!("{r:?}").contains("2412000000")),
        "{:?}",
        out.requests
    );
    assert!(owner::apply(&mut rx, band, &Value::Choice(99)).is_err());
}

#[test]
fn a_drone_with_a_position_sends_an_event() {
    let out = run(Harness::new(&REMOTE_ID).seconds(0.1));
    let e = out
        .events
        .iter()
        .find(|e| e.is(&typed::REMOTE_ID_DRONE) && e.at(&typed::RemoteIdDroneRow::ADDRESS) == Some(DRONE1))
        .expect("an event of drone 1");
    assert_eq!(e.get(&keys::SOURCE), Some("1581F5FKD229400B"));
    assert!(e.get(&keys::POSITION).is_some());
    assert_eq!(e.get(&keys::URGENCY), Some(keys::Urgency::None));
    assert_eq!(e.get(&keys::FAMILY), Some(keys::Family::Aircraft));
}

#[test]
fn remove_forgets_one_drone() {
    let h = Harness::new(&REMOTE_ID).seconds(0.1);
    let x = h.generate().unwrap();
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(20e6, 2432e6, 2432e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&x, &mut out);
    rx.feed(&[], &mut out);
    let id = rx.commands().id("remove").unwrap();
    let mut args = rx.commands().defs()[id.index()].new_args();
    let row_arg = args.decl().id("row").unwrap();
    args.set_value(row_arg, &Value::Text(DRONE1.into()))
        .unwrap();
    owner::invoke(&mut *rx, id, &args).unwrap();
    rx.feed(&[], &mut out);
    assert_eq!(rows(Owner::results(&*rx), &typed::REMOTE_ID_DRONES), [DRONE2]);
    args.set_value(row_arg, &Value::Text("nobody".into()))
        .unwrap();
    assert!(owner::invoke(&mut *rx, id, &args).is_err());
}

#[test]
fn the_recording_decodes_as_expected() {
    let exp = sigmf::read_expected("drones").unwrap();
    let e = &exp["recordings"]["remoteid_2432_20m"];
    let Some(run) = super::recording(&REMOTE_ID, "remoteid_2432_20m") else {
        return;
    };
    let r = &run.results;
    let c = &e["counters"];
    let min = |f: &sdrkit_core::interface::Column<sdrkit_core::publish::field::I64>,
               want: Option<i64>| {
        assert!(count(r, f) >= want.unwrap(), "{}: {}", sdrkit_core::publish::Declare::key(f), count(r, f))
    };
    min(&typed::RemoteIdFields::BLE_GOOD, c["ble_packets_good_min"].as_i64());
    min(&typed::RemoteIdFields::BLE_REMOTE_ID, c["ble_remote_id_min"].as_i64());
    min(&typed::RemoteIdFields::WIFI_GOOD, c["wifi_frames_good_min"].as_i64());
    min(&typed::RemoteIdFields::WIFI_REMOTE_ID, c["wifi_remote_id_min"].as_i64());
    min(&typed::RemoteIdFields::MESSAGES, c["messages_decoded_min"].as_i64());
    let pt = &c["per_type_min"];
    for (k, f) in [
        ("basic_id", &typed::RemoteIdDroneRow::BASIC_ID_MESSAGES),
        ("location", &typed::RemoteIdDroneRow::LOCATION_MESSAGES),
        ("authentication", &typed::RemoteIdDroneRow::AUTHENTICATION_MESSAGES),
        ("self_id", &typed::RemoteIdDroneRow::SELF_ID_MESSAGES),
        ("system", &typed::RemoteIdDroneRow::SYSTEM_MESSAGES),
        ("operator_id", &typed::RemoteIdDroneRow::OPERATOR_ID_MESSAGES),
    ] {
        min(f, pt[k].as_i64());
    }
    let snr = r.status().snr_db.unwrap() as f64;
    assert!(
        (snr - e["status"]["snr_db"].as_f64().unwrap()).abs() <= 4.0,
        "{snr}"
    );
    let cfo = r.status().offset_hz.unwrap();
    assert!(
        (cfo - e["status"]["cfo_hz"].as_f64().unwrap()).abs() <= 3000.0,
        "{cfo}"
    );
    check_drone1(r);
    check_drone2(r);
}
