//! DJI drones: the recorded vector, the acceptance cases, the DroneID coding chain and the generator round trips.

use super::{count, row, rows, run};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::keys;
use sdrkit_core::Results;
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::dji::droneid::{self, Coder, DroneIdPacket, INNER_CRC};
use sdrkit_stack_drones::dji::generator::packet;
use sdrkit_stack_drones::dji::*;
use sdrkit_stack_drones::dji::{
    GenScenario, GEN_EIGHT_SYMBOLS, GEN_PERIOD_S, GEN_SCENARIO, GEN_WIDE,
};
use sdrkit_stack_drones::DJI_DRONEID;

const SERIAL: &str = "SDRKTESTDRONE001";

fn h() -> Harness {
    Harness::new(&DJI_DRONEID)
}

/// The row key of a track of class `class`.
fn track_with(r: &Results, class: Class) -> Option<String> {
    rows(r, &typed::DJI_SIGNALS)
        .into_iter()
        .find(|id| row(r, &typed::DJI_SIGNALS, id).and_then(|t| t.at(&typed::DjiDroneidSignalRow::CLASS)) == Some(class))
}

fn check_drone(r: &Results, min_packets: i64) {
    let d = row(r, &typed::DJI_DRONES, SERIAL).unwrap_or_else(|| panic!("{:?}", rows(r, &typed::DJI_DRONES)));
    assert!(
        d.get(&sdrkit_util_objects::publish::columns::HEARD)
            .unwrap()
            >= min_packets,
        "{:?}",
        rows(r, &typed::DJI_DRONES)
    );
    let (lat, lon) = d.get(&keys::POSITION).unwrap();
    assert!((lat - 51.4468).abs() < 0.01 && (lon - 7.2679).abs() < 0.01);
    assert!(d.at(&typed::DjiDroneidDroneRow::PILOT).is_some());
    assert!(d.at(&typed::DjiDroneidDroneRow::HOME).is_some());
    assert_eq!(d.at(&typed::DjiDroneidDroneRow::INNER_CRC_OK), Some(true));
}

#[test]
fn the_inner_crc_has_its_check_value() {
    assert_eq!(INNER_CRC.checksum(b"123456789"), 0x7109);
}

#[test]
fn the_frame_of_packet_1000_matches_the_published_layout() {
    let exp = sigmf::read_expected("drones").unwrap();
    let want = exp["recordings"]["droneid_2440_20m"]["frame_hex_seq_1000"]
        .as_str()
        .unwrap();
    let mut p = packet(0, 0.05);
    p.gps_ms = 1_650_894_651_295 + 30;
    let f = p.frame();
    let hex: String = f.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        &hex[..want.len().min(hex.len())],
        &want[..want.len().min(hex.len())]
    );
    let back = DroneIdPacket::parse(&f).unwrap();
    assert_eq!(back.serial, SERIAL);
    assert!(back.inner_crc_ok);
}

#[test]
fn the_coding_chain_round_trips_in_noise() {
    let mut c = Coder::new();
    let f = packet(3, 0.2).frame();
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    for variant in 0..2u8 {
        let bits = c.encode(&f, variant);
        assert_eq!(bits.len(), droneid::E);
        let soft: Vec<f32> = bits
            .iter()
            .map(|&b| (if b == 0 { 1.0 } else { -1.0 }) + 0.8 * rng.normal_f32())
            .collect();
        let (g, it) = c.decode(&soft, variant, 8).expect("decodes");
        assert_eq!(g, f);
        assert!(it <= 8);
        assert!(c.decode(&soft, variant ^ 1, 8).is_none());
    }
}

#[test]
fn droneid_alone_decodes_every_burst() {
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::DroneIdOnly)
        .set(&GEN_PERIOD_S, 0.06)
        .seconds(0.4));
    let r = &out.results;
    check_drone(r, 5);
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_CRC_FAILED), 0);
    assert!(track_with(r, Class::DroneId).is_some());
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(
        r.status().summary.starts_with("DroneID: SDRKTESTDRONE001"),
        "{}",
        r.status().summary
    );
    assert!(!rows(r, &typed::DJI_TRACKS).is_empty());
    let e = out
        .events
        .iter()
        .find(|e| e.is(&typed::DJI_DRONEID))
        .expect("a DroneID event");
    assert_eq!(e.get(&keys::SOURCE), Some(SERIAL));
    assert!(e.get(&keys::POSITION).is_some() && e.at(&typed::DjiDroneidDroneRow::PILOT).is_some());
}

#[test]
fn droneid_with_eight_symbols_offsets_and_noise() {
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::DroneIdOnly)
        .set(&GEN_PERIOD_S, 0.06)
        .set(&GEN_EIGHT_SYMBOLS, true)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_OFFSET_HZ, 20e3)
        .seconds(0.4));
    check_drone(&out.results, 4);
}

#[test]
fn other_rates_decode() {
    for rate in [25e6, 61.44e6] {
        let out = super::run(
            h().set(&GEN_SCENARIO, GenScenario::DroneIdOnly)
                .set(&GEN_PERIOD_S, 0.06)
                .rate(rate)
                .seconds(0.3),
        );
        check_drone(&out.results, 3);
    }
}

#[test]
fn collisions_invent_no_drone() {
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::Collisions)
        .set(&GEN_PERIOD_S, 0.06)
        .seconds(0.6));
    let r = &out.results;
    let ids = rows(r, &typed::DJI_DRONES);
    assert!(ids.iter().all(|s| s == SERIAL), "{ids:?}");
}

#[test]
fn ocusync_and_lightbridge_like_shapes_are_classed() {
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::OcuSyncOnly)
        .seconds(0.2));
    assert!(
        track_with(&out.results, Class::OcuSyncLike).is_some(),
        "{:?}",
        rows(&out.results, &typed::DJI_SIGNALS)
    );
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::OcuSyncOnly)
        .set(&GEN_WIDE, true)
        .seconds(0.2));
    assert!(track_with(&out.results, Class::OcuSyncLike).is_some());
    let out = run(h()
        .set(&GEN_SCENARIO, GenScenario::LightbridgeOnly)
        .seconds(0.2));
    assert!(track_with(&out.results, Class::LightbridgeLike).is_some());
}

#[test]
fn wifi_like_bursts_are_not_dji() {
    let out = run(h().set(&GEN_SCENARIO, GenScenario::WifiOnly).seconds(0.5));
    let r = &out.results;
    for id in rows(r, &typed::DJI_SIGNALS) {
        let c = row(r, &typed::DJI_SIGNALS, &id).unwrap().at(&typed::DjiDroneidSignalRow::CLASS).unwrap();
        assert!(c <= Class::UnknownOfdm, "{}", c.name());
    }
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONES), 0);
}

#[test]
fn the_recording_decodes_as_expected() {
    let Some(run) = super::recording(&DJI_DRONEID, "droneid_2440_20m") else {
        return;
    };
    let exp = sigmf::read_expected("drones").unwrap();
    let e = &exp["recordings"]["droneid_2440_20m"];
    let r = &run.results;
    let c = &e["counters"];
    let want = |j: &str| c[j].as_i64().unwrap();
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_DECODED), want("droneid_decoded"));
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_CRC_FAILED), 0);
    assert!(count(r, &typed::DjiDroneidFields::BURSTS_MEASURED) >= want("bursts_measured_min"));
    assert!(count(r, &typed::DjiDroneidFields::BURSTS_OFDM_LIKE) >= want("bursts_ofdm_like_min"));
    assert_eq!(r.status().summary, e["status"]["summary"].as_str().unwrap());
    let cfo = r.status().offset_hz.unwrap();
    assert!(
        (cfo - e["status"]["cfo_from_centre_hz"].as_f64().unwrap()).abs() <= 1000.0,
        "{cfo}"
    );
    let b = r.fields().at(&typed::DjiDroneidFields::BURST_CFO_HZ).unwrap();
    assert!(
        (b - e["status"]["burst_cfo_hz"].as_f64().unwrap()).abs() <= 1000.0,
        "{b}"
    );
    let d = &e["drones"][0];
    let l = &d["last"];
    let dr = row(r, &typed::DJI_DRONES, SERIAL).unwrap();
    let n = |v: Option<i64>| v.unwrap() as f64;
    assert_eq!(
        n(dr.get(&sdrkit_util_objects::publish::columns::HEARD)),
        d["packets"].as_f64().unwrap()
    );
    assert_eq!(n(dr.at(&typed::DjiDroneidDroneRow::SEQ)), l["seq"].as_f64().unwrap());
    let state = i64::from_str_radix(l["state"].as_str().unwrap(), 16).unwrap();
    assert_eq!(dr.at(&typed::DjiDroneidDroneRow::STATE_INFO), Some(state));
    assert_eq!(n(dr.at(&typed::DjiDroneidDroneRow::VERSION)), l["version"].as_f64().unwrap());
    let (lat, lon) = dr.get(&keys::POSITION).unwrap();
    assert!(
        (lat - l["lat"].as_f64().unwrap()).abs() < 1e-6
            && (lon - l["lon"].as_f64().unwrap()).abs() < 1e-6
    );
    assert!((dr.at(&typed::DjiDroneidDroneRow::ALT_M).unwrap() - l["alt_m"].as_f64().unwrap()).abs() < 1e-9);
    assert!((dr.at(&typed::DjiDroneidDroneRow::HEIGHT_M).unwrap() - l["height_m"].as_f64().unwrap()).abs() < 1e-9);
    for (f, j) in [
        (&typed::DjiDroneidDroneRow::V_NORTH, "v_north_raw"),
        (&typed::DjiDroneidDroneRow::V_EAST, "v_east_raw"),
        (&typed::DjiDroneidDroneRow::V_UP, "v_up_raw"),
        (&typed::DjiDroneidDroneRow::YAW, "yaw_raw"),
    ] {
        assert_eq!(n(dr.at(f)), l[j].as_f64().unwrap(), "{}", sdrkit_core::publish::Declare::key(f));
    }
    assert!((dr.at(&typed::DjiDroneidDroneRow::GPS_TIME).unwrap() * 1e3 - l["gps_ms"].as_f64().unwrap()).abs() < 1.0);
    let (hl, ho) = dr.at(&typed::DjiDroneidDroneRow::HOME).unwrap();
    assert!(
        (hl - l["home_lat"].as_f64().unwrap()).abs() < 1e-6
            && (ho - l["home_lon"].as_f64().unwrap()).abs() < 1e-6
    );
    let (pl, po) = dr.at(&typed::DjiDroneidDroneRow::PILOT).unwrap();
    assert!(
        (pl - l["pilot_lat"].as_f64().unwrap()).abs() < 1e-6
            && (po - l["pilot_lon"].as_f64().unwrap()).abs() < 1e-6
    );
    assert_eq!(n(dr.at(&typed::DjiDroneidDroneRow::DEVICE_TYPE)), l["device_type"].as_f64().unwrap());
    assert_eq!(
        n(dr.at(&typed::DjiDroneidDroneRow::TRAIL_POINTS)),
        d["trail_points"].as_f64().unwrap()
    );
    assert!((dr.at(&typed::DjiDroneidDroneRow::FREQ_HZ).unwrap() / 1e6 - d["freq_mhz"].as_f64().unwrap()).abs() <= 0.3);
    for t in e["tracks"].as_array().unwrap() {
        let label = t["class"].as_str().unwrap();
        let class = [
            Class::OtherWideband,
            Class::UnknownOfdm,
            Class::OcuSyncLike,
            Class::LightbridgeLike,
            Class::DroneId,
        ]
        .into_iter()
        .find(|c| c.name() == label)
        .unwrap_or_else(|| panic!("no class {label}"));
        let id = track_with(r, class).unwrap_or_else(|| panic!("no {label} track"));
        let g = row(r, &typed::DJI_SIGNALS, &id).unwrap();
        assert!(
            (g.at(&typed::DjiDroneidDroneRow::FREQ_HZ).unwrap() / 1e6 - t["freq_mhz"].as_f64().unwrap()).abs() <= 0.3,
            "{label}"
        );
        assert!(
            (g.at(&typed::DjiDroneidSignalRow::WIDTH_HZ).unwrap() / 1e6 - t["bw_mhz"].as_f64().unwrap()).abs() <= 1.0,
            "{label}"
        );
        assert!(
            (g.at(&typed::DjiDroneidSignalRow::PERIOD).unwrap() * 1e3 - t["period_ms"].as_f64().unwrap()).abs() <= 0.1,
            "{label}"
        );
        assert!(
            (g.at(&typed::DjiDroneidSignalRow::BURST).unwrap() * 1e6 - t["burst_us"].as_f64().unwrap()).abs() <= 40.0,
            "{label}"
        );
        assert!(
            g.at(&typed::DjiDroneidSignalRow::CONFIDENCE_PCT).unwrap() / 100.0 >= t["confidence_min"].as_f64().unwrap() - 0.05,
            "{label}"
        );
        assert_eq!(
            g.at(&typed::DjiDroneidSignalRow::BAND).map(|b| b == RadioBand::B2g4),
            t["band"].as_str().map(|b| b == "2.4 GHz")
        );
        if t["note_contains"].as_str() == Some("near a frequency reported for DroneID") {
            assert_eq!(g.at(&typed::DjiDroneidSignalRow::NEAR_DRONEID), Some(true), "{label}");
        }
    }
}
