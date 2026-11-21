//! `mavlink_sik`: the MAVLink and SiK layers against their worked values, and the receiver on its test signal.

use super::{count, row, rows};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::keys;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::mavlink_sik::generator::{self, packet, vehicles, Vehicle};
use sdrkit_stack_drones::mavlink_sik::mavlink::{self, Parse, Stream};
use sdrkit_stack_drones::mavlink_sik::{sik, AirRate, AIR_RATE, MAVLINK_SIK};

const HEARTBEAT: [u8; 9] = [0, 0, 0, 0, 2, 3, 0x51, 4, 3];

#[test]
fn the_heartbeat_frames_have_their_worked_bytes() {
    let v1 = mavlink::build(1, 0, 1, 1, 0, &HEARTBEAT).unwrap();
    assert_eq!(
        v1,
        [0xFE, 0x09, 0x00, 0x01, 0x01, 0x00, 0, 0, 0, 0, 2, 3, 0x51, 4, 3, 0x7D, 0xDD]
    );
    let v2 = mavlink::build(2, 0, 1, 1, 0, &HEARTBEAT).unwrap();
    assert_eq!(
        v2,
        [
            0xFD, 0x09, 0, 0, 0x00, 0x01, 0x01, 0, 0, 0, 0, 0, 0, 0, 2, 3, 0x51, 4, 3, 0xE7, 0x1E
        ]
    );
    let Parse::Frame(f, n) = mavlink::parse_at(&v1) else {
        panic!("no frame");
    };
    assert_eq!(n, v1.len());
    assert_eq!(
        mavlink::describe(&f).text,
        "type 2, autopilot 3, base mode 51, status 4, not armed"
    );
}

#[test]
fn the_crc_extra_rule_gives_the_heartbeat_value() {
    let fields = [
        ("uint32_t", "custom_mode", 0),
        ("uint8_t", "type", 0),
        ("uint8_t", "autopilot", 0),
        ("uint8_t", "base_mode", 0),
        ("uint8_t", "system_status", 0),
        ("uint8_t", "mavlink_version", 0),
    ];
    assert_eq!(mavlink::crc_extra("HEARTBEAT", &fields), 50);
}

#[test]
fn the_stream_finds_frames_across_packets_and_drops_a_bad_one() {
    let hb = mavlink::build(1, 0, 1, 1, 0, &HEARTBEAT).unwrap();
    let mut s = Stream::new();
    let mut got = Vec::new();
    let mut data = vec![0x13, 0x37];
    data.extend(&hb);
    data.extend(&hb);
    s.push(&data, |f| got.push(f));
    assert_eq!(got.len(), 2);
    got.clear();
    let mut bad = hb.clone();
    bad[16] ^= 1;
    s.push(&bad, |f| got.push(f));
    assert!(got.is_empty());
    let (a, b) = hb.split_at(7);
    s.push(a, |f| got.push(f));
    assert!(got.is_empty() && s.waiting() == 7);
    s.push(b, |f| got.push(f));
    assert_eq!(got.len(), 1);
}

#[test]
fn the_sik_crc_has_its_check_value() {
    assert_eq!(sik::crc(b"123456789"), 0xBEEF);
}

#[test]
fn sik_packets_parse_with_and_without_ecc_and_golay_corrects() {
    let t = sik::Trailer {
        window: 777,
        ..sik::Trailer::default()
    };
    let mut data = vec![0x5A; 40];
    data.extend(t.bytes());
    let plain = sik::build_plain(0x1234, &data);
    let p = sik::parse_plain(&plain).unwrap();
    assert_eq!((p.net_id, p.data.len(), p.trailer.window), (0x1234, 40, 777));
    assert!(p.data.iter().all(|&b| b == 0x5A) && !p.ecc);
    let code = sik::golay();
    let mut ecc = sik::build_ecc(&code, 0x1234, &data);
    let p = sik::parse_ecc(&code, &ecc).unwrap();
    assert_eq!((p.net_id, p.data.len(), p.trailer.window, p.corrected), (0x1234, 40, 777, 0));
    ecc[20] ^= 0x10;
    ecc[31] ^= 0x02;
    let p = sik::parse_ecc(&code, &ecc).unwrap();
    assert!(p.corrected >= 2 && p.data.iter().all(|&b| b == 0x5A));
    let mut bad = plain.clone();
    bad[10] ^= 0xFF;
    assert!(sik::parse_plain(&bad).is_none());
    let mut bad = sik::build_ecc(&code, 0x1234, &data);
    bad[25] ^= 0xFF;
    bad[26] ^= 0xFF;
    assert!(sik::parse_ecc(&code, &bad).is_none());
}

/// The vehicles of `v` sent at `fs` with the place `place` (fraction of `fs`) and carrier offset `cfo`, in noise of `sigma`.
fn burst_signal(v: &[(Vehicle, f64, f64, f64)], fs: f64, sigma: f32, seed: u64) -> Vec<Complex32> {
    let n = (fs * 0.06) as usize;
    let mut rng = Rng::new(seed);
    let mut x: Vec<Complex32> = (0..n)
        .map(|_| Complex32::new(sigma * rng.normal_f32(), sigma * rng.normal_f32()))
        .collect();
    for (k, &(veh, place, cfo, at)) in v.iter().enumerate() {
        let s = packet(&veh, k as u32, fs).unwrap();
        let start = (at * fs) as usize;
        let f = place * fs + cfo;
        for (i, z) in s.iter().enumerate() {
            if start + i >= n {
                break;
            }
            let p = std::f64::consts::TAU * f * (start + i) as f64 / fs;
            x[start + i] += z * 0.4 * Complex32::new(p.cos() as f32, p.sin() as f32);
        }
    }
    x
}

#[test]
fn every_rate_and_offset_decodes_with_the_position_on_the_link_table() {
    for fs in [2.4e6, 8e6] {
        for rate in [64e3, 128e3, 250e3] {
            let base = Vehicle {
                rate,
                ..vehicles()[0]
            };
            let v: Vec<_> = generator::HOPS
                .iter()
                .zip(generator::CFO_HZ)
                .enumerate()
                .map(|(k, (&p, c))| (base, p, c, 0.004 + 0.015 * k as f64))
                .collect();
            let x = burst_signal(&v, fs, 0.04, 3);
            let run = Harness::new(&MAVLINK_SIK).rate(fs).feed(&x).unwrap();
            let r = &run.results;
            assert_eq!(count(r, &keys::FRAMES_OK), 6, "{fs} {rate}");
            let key = rows(r, &typed::LINKS)
                .into_iter()
                .find(|k| k.contains("sys 1"))
                .expect("the link of sys 1");
            let rec = row(r, &typed::LINKS, &key).unwrap();
            let (lat, lon) = rec.get(&keys::POSITION).unwrap();
            assert!((lat - 54.6872).abs() < 1e-6 && (lon - 25.2797).abs() < 1e-6);
        }
    }
}

#[test]
fn the_fastest_rate_decodes_with_noise_at_20_db_in_300_khz() {
    // Amplitude 0.4 and sigma 0.146 at 8 Msps: 20 dB of signal over the noise in 300 kHz. The dip between the two tones of
    // the GFSK spectrum falls under the burst threshold in some frames.
    let base = Vehicle {
        rate: 250e3,
        ..vehicles()[0]
    };
    let v: Vec<_> = (0..3)
        .map(|k| (base, generator::HOPS[k], generator::CFO_HZ[k], 0.004 + 0.015 * k as f64))
        .collect();
    let x = burst_signal(&v, 8e6, 0.146, 8);
    let run = Harness::new(&MAVLINK_SIK).rate(8e6).feed(&x).unwrap();
    assert_eq!(count(&run.results, &keys::FRAMES_OK), 6);
}

#[test]
fn two_links_at_once_decode_at_every_rate_from_16_kbits() {
    for rate in sik::AIR_RATES.into_iter().filter(|&r| r >= 16e3) {
        let a = Vehicle {
            rate,
            ..vehicles()[0]
        };
        let b = Vehicle {
            rate,
            net_id: 0x0303,
            sys: 9,
            ..vehicles()[1]
        };
        let x = burst_signal(&[(a, -0.2, 0.0, 0.005), (b, 0.25, 10e3, 0.006)], 8e6, 0.04, 5);
        let run = Harness::new(&MAVLINK_SIK).rate(8e6).feed(&x).unwrap();
        assert_eq!(count(&run.results, &typed::LinkFields::LINKS_HEARD), 2, "{rate}");
    }
}

#[test]
fn the_test_signal_gives_all_four_vehicles_and_error_corrected_packets() {
    let run = super::run(Harness::new(&MAVLINK_SIK).seconds(0.5));
    let r = &run.results;
    assert!(count(r, &typed::LinkFields::LINKS_HEARD) >= 4, "{:?}", rows(r, &typed::LINKS));
    assert!(count(r, &keys::FRAMES_OK) >= 30);
    assert!(rows(r, &typed::LINKS).iter().any(|k| k.contains("sys 4")));
}

#[test]
fn the_air_rate_setting_limits_the_search_and_noise_gives_nothing() {
    let run = super::run(
        Harness::new(&MAVLINK_SIK)
            .seconds(0.5)
            .set(&AIR_RATE, AirRate::R128),
    );
    assert_eq!(count(&run.results, &typed::LinkFields::LINKS_HEARD), 1);
    let mut rng = Rng::new(9);
    let x: Vec<Complex32> = (0..4_000_000)
        .map(|_| Complex32::new(0.05 * rng.normal_f32(), 0.05 * rng.normal_f32()))
        .collect();
    let run = Harness::new(&MAVLINK_SIK).feed(&x).unwrap();
    assert_eq!(count(&run.results, &keys::FRAMES_OK), 0);
}

#[test]
fn the_plan_file_has_the_bands_of_the_links_and_the_registry_takes_it() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans");
    let text = std::fs::read_to_string(dir.join("drone_links.json")).expect("the data file");
    let file = sdrkit_data_plans::data::parse("drone_links.json", &text).expect("a plan file");
    let mut plans = (*sdrkit_dev_test::plans()).clone();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    assert_eq!(file.groups.len(), 5);
    assert!(file.groups.iter().all(|g| !g.verified));
    let modes: Vec<&str> = file
        .groups
        .iter()
        .flat_map(|g| g.bands.iter())
        .flat_map(|b| b.modes.iter().map(String::as_str))
        .collect();
    for id in ["rc_links", "mavlink_sik", "wfb"] {
        assert!(modes.contains(&id), "{id}");
    }
}


