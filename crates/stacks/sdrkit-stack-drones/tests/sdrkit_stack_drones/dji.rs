//! DJI drones: the recorded vector, the acceptance cases, the DroneID coding chain and the generator round trips.

use super::{count, row, rows, run};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_LEVEL_DBFS, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, Results};
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

/// A harness whose test signal has DroneID bursts of `frame` on the DroneID frequency nearest to `centre`, one each 0.1 s.
fn on_droneid_frequency(frame: GenFrame, centre: f64) -> Harness {
    h().set(&GEN_SCENARIO, GenScenario::DroneIdOnly)
        .set(&GEN_PERIOD_S, 0.02)
        .set(&GEN_FRAME, frame)
        .set(&GEN_DRONEID_FREQUENCY, true)
        .set(&GEN_CENTRE_HZ, centre)
        .freq(centre)
        .seconds(0.6)
}

/// One burst of `frame` at `offset_hz` from the centre of a capture at `rate`, 0.02 s into 0.03 s of noise `snr_db` below
/// it, without the carriers outside the capture.
fn capture_with_burst(
    rate: f64,
    frame: &[u8; droneid::FRAME],
    offset_hz: f64,
    snr_db: f64,
) -> Vec<Complex32> {
    let carriers = droneid::visible_carriers(offset_hz, 0.5 * rate);
    let burst = droneid::modulate_carriers(frame, 0, true, carriers);
    let mut at_rate = Vec::new();
    sdrkit_algo_dsp::resample::resample_span(&burst, droneid::FS, rate, &mut at_rate);
    sdrkit_algo_dsp::nco::Nco::new(rate, offset_hz).mix_in_place(&mut at_rate);
    let power = at_rate.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / at_rate.len() as f64;
    // The noise power is over the 9 MHz of the burst.
    let noise = power * 10f64.powf(-snr_db / 10.0) * rate / 9e6;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(4);
    let mut x: Vec<Complex32> = (0..(rate * 0.03) as usize)
        .map(|_| rng.complex_normal(noise))
        .collect();
    let at = (rate * 0.02) as usize;
    for (o, v) in x[at..].iter_mut().zip(&at_rate) {
        *o += *v;
    }
    x
}

fn hits(rate: f64, x: &[Complex32], offset_hz: f64) -> Vec<trigger::Hit> {
    let mut t = trigger::PairTrigger::new(rate, [offset_hz].into_iter());
    let mut out = Vec::new();
    for c in x.chunks(16_384) {
        t.feed(c, &mut out);
    }
    out
}

#[test]
fn a_frame_is_classed_by_its_layout() {
    use droneid::FrameKind;
    let key = [1, 2, 3, 4];
    let f = droneid::encrypted_frame(0x10, key, [9; 8], &[0x5a; 109], [7; 4]);
    let FrameKind::Encrypted(head) = droneid::classify(&f) else {
        panic!("not the encrypted format")
    };
    assert_eq!((head.frame_type, head.key, head.cryp, head.length), (0x10, key, false, 128));
    // A body that does not match the inner CRC-16 is not the encrypted format, and its length byte is no readable packet.
    let mut bad = f;
    bad[40] ^= 1;
    assert_eq!(droneid::classify(&bad), FrameKind::Other);
    assert_eq!(droneid::classify(&droneid::empty_frame([7; 4])), FrameKind::Empty);
    assert_eq!(droneid::classify(&droneid::empty_frame([0; 4])), FrameKind::Empty);
    assert!(matches!(
        droneid::classify(&packet(0, 0.0).frame()),
        FrameKind::Readable(p) if p.serial == SERIAL
    ));
}

#[test]
fn the_encrypted_format_shows_an_aircraft_and_no_drone() {
    let out = run(on_droneid_frequency(GenFrame::Encrypted, 2444.5e6));
    let r = &out.results;
    // Six bursts are sent. The burst detector and the pair trigger both find each: it counts once.
    let frames = count(r, &typed::DjiDroneidFields::DRONEID_ENCRYPTED);
    assert!((5..=6).contains(&frames), "{frames}");
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_CRC_FAILED), 0);
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_DECODED), 0);
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_EMPTY), 0);
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_OTHER), 0);
    assert!(rows(r, &typed::DJI_DRONES).is_empty());
    assert_eq!(rows(r, &typed::DJI_AIRCRAFT), ["session-5d4b01a7"]);
    let a = row(r, &typed::DJI_AIRCRAFT, "session-5d4b01a7").unwrap();
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::SESSION_KEY), Some("5d4b01a7"));
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::FORMAT), Some(FrameFormat::Encrypted));
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::FRAMES), Some(frames));
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::DRONE), None);
    assert!(matches!(a.at(&typed::DjiDroneidAircraftRow::FRAME_TYPE), Some(0x10 | 0x11)));
    let f = a.at(&typed::DjiDroneidAircraftRow::FREQ_HZ).unwrap();
    assert!((f - 2444.5e6).abs() < 1.0, "{f}");
    assert!(
        a.at(&typed::DjiDroneidAircraftRow::LAST_S).unwrap()
            > a.at(&typed::DjiDroneidAircraftRow::FIRST_S).unwrap()
    );
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(
        r.status().summary.starts_with("DJI DroneID, encrypted format (type 0x1")
            && r.status().summary.ends_with("aircraft present, position and serial not readable"),
        "{}",
        r.status().summary
    );
    let seen: Vec<_> = out.events.iter().filter(|e| e.is(&typed::DJI_AIRCRAFT_SEEN)).collect();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].get(&keys::SOURCE), Some("session-5d4b01a7"));
    assert_eq!(seen[0].at(&typed::DjiDroneidAircraftEvent::SESSION_KEY), Some("5d4b01a7"));
    assert!(seen[0].get(&keys::POSITION).is_none());
    assert!(!out.events.iter().any(|e| e.is(&typed::DJI_DRONEID)));
}

#[test]
fn two_aircraft_of_the_encrypted_format_are_told_apart() {
    let out = run(on_droneid_frequency(GenFrame::Encrypted, 2444.5e6).set(&GEN_TWO_AIRCRAFT, true));
    let r = &out.results;
    let mut names = rows(r, &typed::DJI_AIRCRAFT);
    names.sort();
    assert_eq!(names, ["session-5d4b01a7", "session-5d4b023c"]);
    assert_eq!(count(r, &typed::DjiDroneidFields::AIRCRAFT), 2);
    for n in &names {
        let frames = row(r, &typed::DJI_AIRCRAFT, n).unwrap().at(&typed::DjiDroneidAircraftRow::FRAMES);
        assert!(frames >= Some(2), "{n}: {frames:?}");
    }
    assert_eq!(out.events.iter().filter(|e| e.is(&typed::DJI_AIRCRAFT_SEEN)).count(), 2);
}

#[test]
fn empty_frames_show_that_an_aircraft_is_present() {
    let out = run(on_droneid_frequency(GenFrame::Empty, 2444.5e6)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_OFFSET_HZ, 13e3));
    let r = &out.results;
    let frames = count(r, &typed::DjiDroneidFields::DRONEID_EMPTY);
    assert!((5..=6).contains(&frames), "{frames}");
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_REFUSED), 0);
    assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_CRC_FAILED), 0);
    assert!(rows(r, &typed::DJI_DRONES).is_empty());
    assert_eq!(rows(r, &typed::DJI_AIRCRAFT), [EMPTY_ROW]);
    let a = row(r, &typed::DJI_AIRCRAFT, EMPTY_ROW).unwrap();
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::FORMAT), Some(FrameFormat::Empty));
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::SESSION_KEY), None);
    assert_eq!(
        r.status().summary,
        "DJI DroneID present, empty frame (no data sent in this state)"
    );
}

#[test]
fn the_readable_packet_on_a_droneid_frequency_keeps_its_drone_row() {
    let out = run(on_droneid_frequency(GenFrame::Readable, 2444.5e6));
    let r = &out.results;
    check_drone(r, 5);
    assert_eq!(rows(r, &typed::DJI_AIRCRAFT), [SERIAL]);
    let a = row(r, &typed::DJI_AIRCRAFT, SERIAL).unwrap();
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::FORMAT), Some(FrameFormat::Readable));
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::DRONE), Some(SERIAL));
    let frames = count(r, &typed::DjiDroneidFields::DRONEID_DECODED);
    assert!((5..=6).contains(&frames), "{frames}");
    assert_eq!(a.at(&typed::DjiDroneidAircraftRow::FRAMES), Some(frames));
}

#[test]
fn a_burst_cut_by_the_edge_of_the_capture_decodes() {
    // 2429.5 MHz in a capture centred on 2436 MHz: the lowest carriers are outside.
    let out = run(on_droneid_frequency(GenFrame::Encrypted, 2436e6)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0));
    let r = &out.results;
    let frames = count(r, &typed::DjiDroneidFields::DRONEID_ENCRYPTED);
    assert!((5..=6).contains(&frames), "{frames}");
    let a = row(r, &typed::DJI_AIRCRAFT, "session-5d4b01a7").unwrap();
    let f = a.at(&typed::DjiDroneidAircraftRow::FREQ_HZ).unwrap();
    assert!((f - 2429.5e6).abs() < 1.0, "{f}");
    // With less than half of the carriers inside, only the pair trigger finds the burst.
    let frame = droneid::encrypted_frame(0x11, [8, 7, 6, 5], [3; 8], &[0xc3; 109], [0; 4]);
    let offset = -10.5e6;
    let found = hits(20e6, &capture_with_burst(20e6, &frame, offset, 15.0), offset);
    assert_eq!(found.len(), 1);
    assert!(found[0].visible < 0.45, "{}", found[0].visible);
    assert_eq!(found[0].decode.frame, Some(frame));
    assert!((found[0].start_s - 0.02).abs() < 100e-6, "{}", found[0].start_s);
}

#[test]
fn the_pair_trigger_finds_a_burst_below_the_noise() {
    let frame = droneid::encrypted_frame(0x10, [8, 7, 6, 5], [3; 8], &[0x3c; 109], [1; 4]);
    let found = hits(20e6, &capture_with_burst(20e6, &frame, 0.0, -1.0), 0.0);
    assert_eq!(found.len(), 1);
    assert!(found[0].decode.zc_found);
    assert_eq!(found[0].decode.frame, Some(frame));
    assert!((found[0].decode.snr_db + 1.0).abs() < 3.0, "{}", found[0].decode.snr_db);
}

#[test]
fn a_weak_empty_frame_is_accepted() {
    // At 0 dB about one hard decision in six is wrong. The frame is still an empty frame.
    let frame = droneid::empty_frame([5, 6, 7, 8]);
    let found = hits(20e6, &capture_with_burst(20e6, &frame, 0.0, 0.0), 0.0);
    assert_eq!(found.len(), 1);
    let d = &found[0].decode;
    assert_eq!(d.frame, Some(frame));
    assert!((0.75..0.93).contains(&d.agreement), "{}", d.agreement);
    assert!(d.accepted());
}

#[test]
fn the_pair_trigger_runs_at_other_rates() {
    let frame = droneid::encrypted_frame(0x10, [1, 9, 8, 4], [6; 8], &[0x96; 109], [2; 4]);
    for (rate, offset) in [(16e6, 0.0), (25e6, 4.5e6), (30.72e6, -7.5e6), (61.44e6, 15e6)] {
        let found = hits(rate, &capture_with_burst(rate, &frame, offset, 15.0), offset);
        assert_eq!(found.len(), 1, "{rate}");
        assert_eq!(found[0].decode.frame, Some(frame), "{rate}");
        assert!((found[0].start_s - 0.02).abs() < 100e-6, "{rate}: {}", found[0].start_s);
        assert!((found[0].offset_hz - offset).abs() < 1.0 && found[0].visible > 0.99, "{rate}");
    }
}

#[test]
fn a_frame_of_zeros_from_noise_is_refused() {
    // The decoder makes a frame of zeros from soft bits of no weight, and its CRC-24A holds.
    let (zeros, _) = Coder::new().decode(&vec![0.0; droneid::E], 0, 8).expect("the CRC of zeros holds");
    assert_eq!(zeros, [0u8; droneid::FRAME]);
    let mut d = droneid::BurstDecode {
        zc_found: true,
        score: [0.5, 0.5],
        agreement: 0.5,
        frame: Some(zeros),
        ..Default::default()
    };
    assert!(!d.accepted(), "the bits do not agree");
    d.agreement = 0.99;
    d.score = [0.5, 0.01];
    assert!(!d.accepted(), "one symbol is missing");
    d.score = [0.5, 0.5];
    assert!(d.accepted());
    // Noise alone, and silence: no frame, no aircraft.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(11);
    let noise: Vec<Complex32> = (0..10_000_000).map(|_| rng.complex_normal(1e-4)).collect();
    for x in [noise, vec![Complex32::new(0.0, 0.0); 4_000_000]] {
        let out = super::feed(&h().freq(2444.5e6), x);
        let r = &out.results;
        assert_eq!(count(r, &typed::DjiDroneidFields::DRONEID_EMPTY), 0);
        assert_eq!(count(r, &typed::DjiDroneidFields::AIRCRAFT), 0);
        assert!(rows(r, &typed::DJI_AIRCRAFT).is_empty());
    }
}

#[test]
fn the_stack_asks_for_the_droneid_frequencies_only_when_set_to_step() {
    let mhz = |v: &[f64]| v.iter().map(|f| f * 1e6).collect::<Vec<_>>();
    assert_eq!(step_centres(20e6, 2440e6), mhz(&[2399.5, 2414.5, 2429.5, 2444.5, 2459.5]));
    assert_eq!(step_centres(30.72e6, 2440e6), mhz(&[2407.0, 2437.0, 2459.5]));
    assert_eq!(step_centres(61.44e6, 2440e6), mhz(&[2422.0, 2459.5]));
    assert_eq!(step_centres(80e6, 2440e6), mhz(&[2429.5]));
    assert_eq!(step_centres(20e6, 5800e6), mhz(&[5756.5, 5776.5, 5796.5]));
    assert!(step_centres(20e6, 5200e6).is_empty() && step_centres(20e6, 915e6).is_empty());

    let quiet = vec![Complex32::new(0.0, 0.0); 2_000_000];
    let out = super::feed(&h(), quiet.clone());
    assert!(out.requests.is_empty(), "{:?}", out.requests);
    let out = super::feed(&h().set(&TUNING, Stepping::Step).set(&DWELL_S, 12.0), quiet.clone());
    assert_eq!(
        out.requests,
        [sdrkit_core::HostRequest::Hop {
            channels: mhz(&[2399.5, 2414.5, 2429.5, 2444.5, 2459.5]),
            dwell_s: 12.0,
            stop_on_lock: false,
        }]
    );
    // A capture that holds every frequency of the band needs one centre.
    let wide: Vec<Complex32> = vec![Complex32::new(0.0, 0.0); 8_000_000];
    let out = super::feed(&h().set(&TUNING, Stepping::Step).rate(80e6), wide);
    assert_eq!(
        out.requests,
        [sdrkit_core::HostRequest::retune(2429.5e6, "DroneID frequencies")]
    );
}


/// The level of the aircraft heard in the test signal of `h`, dBFS.
fn aircraft_level(h: Harness) -> f64 {
    let out = run(h);
    let r = &out.results;
    let id = rows(r, &typed::DJI_AIRCRAFT).into_iter().next().expect("an aircraft");
    row(r, &typed::DJI_AIRCRAFT, &id)
        .and_then(|a| a.at(&typed::DjiDroneidAircraftRow::LEVEL_DBFS))
        .expect("a level")
}

#[test]
fn the_level_of_an_aircraft_is_the_power_in_its_own_band() {
    // The test signal sends each burst at its set level, 0 dBFS. The OcuSync-like bursts next to the DroneID
    // bursts of the full test signal add nothing to the level.
    let alone = aircraft_level(h().set(&GEN_SCENARIO, GenScenario::DroneIdOnly).seconds(3.0));
    let full = aircraft_level(h().seconds(3.0));
    assert!(alone < 0.05 && full < 0.05, "{alone} {full}");
    assert!((full - alone).abs() < 0.1, "{alone} {full}");
    // A test signal below full scale reads below 0 dBFS, also on a DroneID frequency.
    for h in [h().seconds(3.0), on_droneid_frequency(GenFrame::Encrypted, 2440e6)] {
        let level = aircraft_level(h.set(&GEN_LEVEL_DBFS, -20.0));
        assert!((level + 20.0).abs() < 0.3, "{level}");
    }
}

#[test]
fn the_pair_trigger_measures_the_level_in_the_band_of_the_burst() {
    let rate = 30.72e6;
    let frame = droneid::encrypted_frame(0x10, [4, 3, 2, 1], [5; 8], &[0x69; 109], [3; 4]);
    let mut x = capture_with_burst(rate, &frame, 0.0, 15.0);
    let power = |v: &[Complex32]| v.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / v.len() as f64;
    let at = (rate * 0.02) as usize;
    let alone = power(&x[at..at + (rate * 0.6e-3) as usize]);
    // An OFDM-like signal of the same power 9.5 MHz above it, all through the burst.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let other = sdrkit_stack_drones::dji::generator::ofdm_burst(1024, 600, 72, 14, false, &mut rng);
    let mut beside = Vec::new();
    sdrkit_algo_dsp::resample::resample_span(&other, droneid::FS, rate, &mut beside);
    sdrkit_algo_dsp::nco::Nco::new(rate, 9.5e6).mix_in_place(&mut beside);
    let g = (alone / power(&beside)).sqrt() as f32;
    for (o, v) in x[at - 2000..].iter_mut().zip(&beside) {
        *o += *v * g;
    }
    let found = hits(rate, &x, 0.0);
    assert_eq!(found.len(), 1);
    let want = sdrkit_algo_dsp::meter::power_to_db(alone);
    assert!((found[0].level_dbfs - want).abs() < 0.3, "{} {want}", found[0].level_dbfs);
}
