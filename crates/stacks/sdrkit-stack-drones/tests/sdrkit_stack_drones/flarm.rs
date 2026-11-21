//! FLARM: the recorded vector, the acceptance cases and the generator round trips.

use super::{count, feed, row, rows};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, Generator, Record, Results, Station};
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::dji::WALL_CLOCK;
use sdrkit_stack_drones::flarm::generator::{Burst, FlarmScene};
use sdrkit_stack_drones::flarm::proto::V6Tx;
use sdrkit_stack_drones::flarm::START_TIME;
use sdrkit_core::publish::Declare;
use sdrkit_stack_drones::FLARM;
use sdrkit_util_geo::{Position, KNOT_M_S};
use sdrkit_util_objects::publish::columns;

const STATION: (f64, f64) = (54.70, 25.30);
const T0: f64 = 1_800_000_020.0;

#[allow(clippy::too_many_arguments)]
fn tx(
    address: u32,
    addr_type: u8,
    ty: u8,
    bearing: f64,
    dist: f64,
    speed: f64,
    track: f64,
    climb: f64,
    alt: f64,
) -> V6Tx {
    let p = Position::new(STATION.0, STATION.1).destination(bearing, dist);
    V6Tx {
        address,
        addr_type,
        aircraft_type: ty,
        lat: p.lat_deg,
        lon: p.lon_deg,
        alt_m: alt,
        speed,
        track,
        climb,
        airborne: true,
        stealth: false,
        no_track: false,
    }
}

fn glider() -> V6Tx {
    tx(0xDD1234, 2, 1, 307.0, 2500.0, 25.0, 90.0, 1.0, 800.0)
}

/// The scene's samples with white noise for `snr_db` in 200 kHz (a 0 dB packet).
fn samples(rate: f64, bursts: Vec<Burst>, snr_db: f64, secs: f64, seed: u64) -> Vec<Complex32> {
    let mut g = FlarmScene::new(rate, bursts, Some(secs));
    let mut x = vec![Complex32::default(); (secs * rate) as usize];
    g.generate(&mut x);
    let p = 10f64.powf(-snr_db / 10.0) * rate / 200e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    for v in &mut x {
        *v += rng.complex_normal(p);
    }
    x
}

fn h(station: bool, rate: f64) -> Harness {
    let mut h = Harness::new(&FLARM)
        .rate(rate)
        .set(&WALL_CLOCK, false)
        .set(&START_TIME, T0);
    if station {
        h = h.station(Station::at(Position::new(STATION.0, STATION.1)));
    }
    h
}

fn id(a: u32) -> String {
    format!("{a:06X}")
}

fn aircraft(r: &Results, a: u32) -> &Record {
    row(r, &typed::FLARM_AIRCRAFT, &id(a)).unwrap_or_else(|| panic!("no aircraft {}", id(a)))
}

/// The code in a text such as "ICAO (1)".
fn code_in(text: &str) -> i64 {
    let a = text.rfind('(').unwrap() + 1;
    let b = text.rfind(')').unwrap();
    text[a..b].parse().unwrap()
}

#[test]
fn a_scene_with_flags_bad_and_off_key_packets() {
    for rate in [1e6, 2.4e6] {
        let mut stealth = tx(0xDD8888, 2, 7, 10.0, 4000.0, 10.0, 0.0, 0.0, 700.0);
        stealth.stealth = true;
        let mut notrack = tx(0xDD9999, 2, 1, 200.0, 3000.0, 20.0, 45.0, 0.0, 900.0);
        notrack.no_track = true;
        let anon = tx(0xA1B2C3, 3, 1, 100.0, 5000.0, 20.0, 45.0, 0.0, 900.0);
        let plain = [
            glider(),
            tx(0xDDA5B6, 2, 2, 142.8, 5013.0, 38.0, 180.0, 3.2, 1020.0),
            tx(0x3F7C11, 1, 8, 45.0, 1130.0, 50.0, 270.0, -0.6, 1140.0),
        ];
        let off = [
            tx(0xDD4444, 2, 1, 250.0, 2000.0, 20.0, 0.0, 0.0, 600.0),
            tx(0xDD4444, 2, 1, 250.0, 2060.0, 20.0, 0.0, 0.0, 604.0),
        ];
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let mut bursts = Vec::new();
        for (round, off) in off.iter().enumerate() {
            let base = 0.1 + 2.4 * round as f64;
            let mut k = 0;
            let mut at = || {
                k += 1;
                (base + 0.12 * k as f64, (k % 2) as u8)
            };
            for p in plain.iter().chain([&notrack, &stealth, &anon]) {
                let (t, ch) = at();
                bursts.push(Burst::v6(t, ch, p, T0 + t));
            }
            let (t, ch) = at();
            bursts.push(Burst::newer(t, ch, 0xDD7777, &mut rng));
            let (t, ch) = at();
            let mut b = Burst::v6(
                t,
                ch,
                &tx(0xDD5555, 2, 1, 0.0, 1000.0, 10.0, 0.0, 0.0, 500.0),
                T0 + t,
            );
            b.corrupt_crc = true;
            bursts.push(b);
            let (t, ch) = at();
            bursts.push(Burst::v6(t, ch, off, T0 + t + 40.0 * 64.0));
        }
        let x = samples(rate, bursts, 25.0, 5.0, 7);
        let run = feed(&h(true, rate), x);
        let r = &run.results;
        assert_eq!(count(r, &typed::FlarmFields::AIRCRAFT), 3, "{rate}");
        assert_eq!(count(r, &typed::FlarmFields::V6_DECODED), 6, "{rate}");
        assert_eq!(count(r, &typed::FlarmFields::PRIVACY_AIRCRAFT), 3);
        assert_eq!(count(r, &typed::FlarmFields::NEWER_PACKETS), 2);
        assert_eq!(count(r, &typed::FlarmFields::NEWER_DEVICES), 1);
        assert!(count(r, &keys::FRAMES_BAD) >= 1);
        let gl = glider();
        let a = aircraft(r, 0xDD1234);
        let (lat, _) = a.get(&keys::POSITION).unwrap();
        assert!((lat - gl.lat).abs() < 3e-4);
        assert_eq!(a.at(&typed::FlarmAircraftRow::ALT_M), Some(800.0));
        assert!((a.get(&keys::SPEED_KT).unwrap() * KNOT_M_S * 3.6 - 90.0).abs() <= 4.0);
        assert!((a.at(&typed::FlarmAircraftRow::CLIMB_M_S).unwrap() - 1.0).abs() <= 0.3);
        assert!((a.at(&typed::FlarmAircraftRow::DISTANCE_KM).unwrap() - 2.5).abs() <= 0.2);
        assert_eq!(aircraft(r, 0x3F7C11).at(&typed::FlarmAircraftRow::ADDRESS_TYPE), Some(1), "ICAO");
        let all = serde_json::to_string(&run.results).unwrap() + &format!("{:?}", run.events);
        for bad in ["DD9999", "DD8888", "A1B2C3", "DD7777", "DD5555", "DD4444"] {
            assert!(!all.contains(bad), "{bad} appears");
        }
    }
}

#[test]
fn noise_alone_returns_nothing() {
    let x = samples(1e6, vec![], 25.0, 4.0, 9);
    let run = feed(&h(true, 1e6), x);
    let r = &run.results;
    assert_eq!(count(r, &typed::FlarmFields::AIRCRAFT), 0);
    assert_eq!(count(r, &typed::FlarmFields::V6_DECODED), 0);
    assert_eq!(count(r, &keys::FRAMES_BAD), 0);
}

#[test]
fn without_a_station_position_there_are_no_positions() {
    let bursts = vec![
        Burst::v6(0.1, 0, &glider(), T0 + 0.1),
        Burst::v6(0.5, 1, &glider(), T0 + 0.5),
    ];
    let x = samples(1e6, bursts, 25.0, 1.0, 1);
    let run = feed(&h(false, 1e6), x);
    let r = &run.results;
    assert_eq!(count(r, &typed::FlarmFields::AIRCRAFT), 1);
    assert_eq!(aircraft(r, 0xDD1234).get(&keys::POSITION), None);
    assert_eq!(r.fields().at(&typed::FlarmFields::NEEDS_STATION), Some(true));
}

#[test]
fn carrier_offsets_and_chip_clock_errors() {
    for (cfo, ppm) in [(-25e3, 0.0), (25e3, 0.0), (0.0, 200.0), (0.0, -200.0)] {
        let bursts: Vec<Burst> = (0..6)
            .map(|i| {
                let t = 0.05 + 0.3 * i as f64;
                let mut b = Burst::v6(t, (i % 2) as u8, &glider(), T0 + t);
                b.cfo_hz = cfo;
                b.ppm = ppm;
                b
            })
            .collect();
        let x = samples(1e6, bursts, 20.0, 2.0, 4);
        let run = feed(&h(true, 1e6), x);
        assert_eq!(count(&run.results, &typed::FlarmFields::V6_DECODED), 6, "{cfo} Hz {ppm} ppm");
    }
}

#[test]
fn the_demo_source_shows_its_open_devices() {
    let x = Harness::new(&FLARM)
        .set(&START_TIME, T0)
        .seconds(4.0)
        .generate()
        .unwrap();
    let run = feed(&h(true, 1e6), x);
    let r = &run.results;
    let ids = rows(r, &typed::FLARM_AIRCRAFT);
    for want in ["DD1234", "DDA5B6", "3F7C11"] {
        assert!(ids.iter().any(|i| i == want), "{ids:?}");
    }
    for bad in ["DD9999", "DD8888", "DD7777"] {
        assert!(!ids.iter().any(|i| i == bad), "{bad}");
    }
}

#[test]
fn an_aircraft_on_a_collision_course_is_urgent() {
    let toward = tx(0xDD2222, 2, 1, 0.0, 600.0, 40.0, 180.0, 0.0, 0.0);
    let away = tx(0xDD3333, 2, 1, 90.0, 3000.0, 40.0, 90.0, 0.0, 0.0);
    let bursts = vec![
        Burst::v6(0.1, 0, &toward, T0 + 0.1),
        Burst::v6(0.2, 1, &away, T0 + 0.2),
        Burst::v6(0.5, 1, &toward, T0 + 0.5),
        Burst::v6(0.6, 0, &away, T0 + 0.6),
    ];
    let x = samples(1e6, bursts, 25.0, 1.0, 2);
    let run = feed(&h(true, 1e6), x);
    let r = &run.results;
    assert_eq!(
        aircraft(r, 0xDD2222).get(&keys::URGENCY),
        Some(keys::Urgency::Urgent)
    );
    assert_eq!(
        aircraft(r, 0xDD3333).get(&keys::URGENCY),
        Some(keys::Urgency::None)
    );
    let urgent = run
        .events
        .iter()
        .filter(|e| e.is(&typed::FLARM_AIRCRAFT_SEEN) && e.get(&keys::URGENCY) == Some(keys::Urgency::Urgent))
        .count();
    assert!(urgent >= 1);
}

#[test]
fn the_recording_decodes_as_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("drones/flarm_868_1m") else {
        assert!(std::env::var("SDRKIT_RECORDINGS").is_err());
        return;
    };
    let run = h(true, rec.rate_hz)
        .freq(rec.centre_hz)
        .feed(&rec.samples)
        .unwrap();
    let exp = sigmf::read_expected("drones").unwrap();
    let e = &exp["recordings"]["flarm_868_1m"];
    let r = &run.results;
    let c = &e["counters"];
    let eq = |f: &sdrkit_core::interface::Column<sdrkit_core::publish::field::I64>, j: &str| {
        assert_eq!(count(r, f), c[j].as_i64().unwrap(), "{}", Declare::key(f))
    };
    eq(&typed::FlarmFields::V6_DECODED, "v6_decoded");
    eq(&typed::FlarmFields::NEWER_PACKETS, "newer_protocol_packets");
    eq(&typed::FlarmFields::NEWER_DEVICES, "newer_protocol_devices");
    eq(&typed::FlarmFields::PRIVACY_AIRCRAFT, "privacy_flagged_aircraft");
    eq(&typed::FlarmFields::PRIVACY_PACKETS, "privacy_packets");
    eq(&typed::FlarmFields::OTHER_TYPE, "other_type");
    eq(&typed::FlarmFields::AIRCRAFT, "aircraft");
    eq(&typed::FlarmFields::WITH_POSITION, "with_position");
    assert!(count(r, &typed::FlarmFields::SYNCS) >= c["syncs_min"].as_i64().unwrap());
    assert_eq!(
        count(r, &typed::FlarmFields::FRAMES_868_2),
        c["per_channel"][0].as_i64().unwrap()
    );
    assert_eq!(
        count(r, &typed::FlarmFields::FRAMES_868_4),
        c["per_channel"][1].as_i64().unwrap()
    );
    let far = r.fields().at(&typed::FlarmFields::FARTHEST_KM).unwrap();
    assert!((far - c["farthest_km"].as_f64().unwrap()).abs() <= 0.05);
    for a in e["aircraft"].as_array().unwrap() {
        let i = a["id"].as_str().unwrap();
        let g = row(r, &typed::FLARM_AIRCRAFT, i).unwrap_or_else(|| panic!("no aircraft {i}"));
        let f = |k: &str| a[k].as_f64().unwrap();
        assert_eq!(
            g.at(&typed::FlarmAircraftRow::ADDRESS_TYPE),
            Some(code_in(a["address_type"].as_str().unwrap()))
        );
        assert_eq!(
            g.at(&typed::FlarmAircraftRow::TYPE),
            Some(code_in(a["type"].as_str().unwrap()))
        );
        let (lat, lon) = g.get(&keys::POSITION).unwrap();
        assert!((lat - f("lat")).abs() < 2e-5 && (lon - f("lon")).abs() < 2e-5);
        assert_eq!(g.at(&typed::FlarmAircraftRow::ALT_M), Some(f("alt_m")));
        assert!((g.get(&keys::SPEED_KT).unwrap() * KNOT_M_S * 3.6 - f("speed_kmh")).abs() <= 4.0);
        let dt = (g.get(&keys::COURSE_DEG).unwrap() - f("track_deg") + 540.0) % 360.0 - 180.0;
        assert!(dt.abs() <= 4.0);
        assert!((g.at(&typed::FlarmAircraftRow::CLIMB_M_S).unwrap() - f("climb_ms")).abs() <= 0.3);
        assert!((g.at(&typed::FlarmAircraftRow::DISTANCE_KM).unwrap() - f("distance_km")).abs() <= 0.05);
        let db = (g.at(&typed::FlarmAircraftRow::BEARING_DEG).unwrap() - f("bearing_deg") + 540.0) % 360.0 - 180.0;
        assert!(db.abs() <= 1.0);
        assert_eq!(g.get(&columns::HEARD), a["packets"].as_i64());
        let ch = if a["channel"].as_str().unwrap().starts_with("868.2") {
            868.2e6
        } else {
            868.4e6
        };
        assert!((g.at(&typed::FlarmAircraftRow::CHANNEL_HZ).unwrap() - ch).abs() < 1.0);
        assert!((g.at(&typed::FlarmAircraftRow::LEVEL_DB).unwrap() - f("level_db")).abs() <= 4.0);
    }
    let all = serde_json::to_string(&run.results).unwrap() + &format!("{:?}", run.events);
    for bad in ["DD9999", "DD8888", "DD7777", "DD5555"] {
        assert!(!all.contains(bad), "{bad} appears");
    }
}
