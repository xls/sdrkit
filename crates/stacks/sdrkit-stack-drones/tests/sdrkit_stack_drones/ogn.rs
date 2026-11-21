//! OGN tracker: the worked packets in the tracker and on the air, the tolerances of the channel receiver, the repair, the plan
//! data file and the test signal.

use super::{count, row, rows};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, Event, Generator, InputInfo, LockState, Output, Receiver, Results};
use sdrkit_dev_test::Harness;
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::ogn::channels::{OgnPacket, CHANNELS, CHANNEL_STEP_HZ};
use sdrkit_stack_drones::ogn::generator::{scene_report, Burst, OgnScene};
use sdrkit_stack_drones::ogn::proto::{self, PACKET};
use sdrkit_stack_drones::ogn::*;
use sdrkit_stack_drones::OGN;
use sdrkit_util_geo::Position;

const DIAL: f64 = 868.3e6;
/// The two channels of the default plan as offsets from the dial.
const LOW: f64 = -100e3;
const HIGH: f64 = 100e3;

const AIR_1: [u8; PACKET] = [
    0x56, 0x34, 0x12, 0x0B, 0xBD, 0x1A, 0x5B, 0x4F, 0x14, 0xCB, 0xCF, 0x87, 0x9C, 0x6B, 0xF5, 0x5D,
    0x11, 0x32, 0x55, 0x35, 0xBE, 0xBC, 0x80, 0x43, 0x00, 0xE1,
];
const AIR_2: [u8; PACKET] = [
    0xEF, 0xCD, 0xAB, 0x0B, 0xD3, 0x33, 0x87, 0x3D, 0xF1, 0x3B, 0xB2, 0xB9, 0x02, 0x8B, 0xD6, 0x59,
    0xAF, 0x9B, 0xDD, 0x5B, 0x5A, 0x9E, 0xE4, 0xFD, 0x07, 0x3B,
];
const AIR_3: [u8; PACKET] = [
    0x11, 0x47, 0xDD, 0x0A, 0xE4, 0xF1, 0x98, 0x52, 0xD2, 0x9B, 0x17, 0x7A, 0xA2, 0xA6, 0x1C, 0x77,
    0x9C, 0xB3, 0xBB, 0xF4, 0xFB, 0xF9, 0x5C, 0x85, 0x15, 0x2A,
];
/// The length of a transmission, seconds.
const AIR_S: f64 = 4.96e-3;

/// The scene of `bursts` at `rate`, `secs` long, with white noise for `snr_db` in 200 kHz (`None`: no noise).
fn air(rate: f64, bursts: Vec<Burst>, snr_db: Option<f64>, secs: f64, seed: u64) -> Vec<Complex32> {
    let mut g = OgnScene::new(rate, bursts, Some(secs));
    let mut x = vec![Complex32::default(); (secs * rate) as usize];
    g.generate(&mut x);
    if let Some(snr) = snr_db {
        let p = 10f64.powf(-snr / 10.0) * rate / 200e3;
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        for v in &mut x {
            *v += rng.complex_normal(p);
        }
    }
    x
}

/// Feeds `x`, centred on `centre` and tuned to `dial`, to the receiver of `h`.
fn receive(
    h: &Harness,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    dial: f64,
) -> (Box<dyn Receiver>, Vec<Event>) {
    let mut rx = h.receiver().expect("the receiver");
    rx.configure(&InputInfo::new(rate, centre, dial).recording())
        .expect("configure");
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(16384) {
        out.clear();
        rx.feed(c, &mut out);
        events.append(&mut out.events);
    }
    out.clear();
    rx.finish(&mut out);
    events.append(&mut out.events);
    (rx, events)
}

/// The good packets of `x` at the default plan with the dial at the centre.
fn good(x: &[Complex32], rate: f64) -> i64 {
    let (rx, _) = receive(&Harness::new(&OGN), x, rate, DIAL, DIAL);
    count(rx.results(), &keys::FRAMES_OK)
}

/// The decoded lines of the packet log, newest first.
fn lines(r: &Results) -> Vec<String> {
    rows(r, &typed::OGN_PACKETS)
        .iter()
        .filter_map(|k| row(r, &typed::OGN_PACKETS, k)?.at(&typed::OgnPacketRow::DECODED).map(str::to_string))
        .collect()
}

fn packet(bytes: [u8; PACKET], freq_hz: f64) -> OgnPacket {
    OgnPacket {
        bytes,
        channel: 1,
        freq_hz,
        level_db: 20.0,
        power_dbfs: -3.0,
        repaired: 0,
    }
}

/// Packet 1 with byte 3 of its header changed and the check bits made again.
fn with_header_byte(b: u8) -> [u8; PACKET] {
    let mut p = AIR_1;
    p[3] = b;
    proto::seal(&mut p);
    p
}

#[test]
fn the_tracker_reads_the_worked_packets() {
    let mut t = OgnTracker::default();
    assert!(t.take_packet(1.0, &packet(AIR_1, 868.4e6)));
    let s = t.store();
    let r = s.packets.newest_first().next().unwrap();
    assert_eq!(
        (r.kind, r.id.as_str()),
        (PacketKind::Position, "123456 OGN")
    );
    assert_eq!(
        r.decoded,
        "glider, 1500 m, 12.3 m/s, heading 180, climb 0.0 m/s, relay 0, 868.4 MHz"
    );
    // The same bytes 2 ms later are the same packet. 6 ms later they are a second one.
    assert!(!t.take_packet(1.002, &packet(AIR_1, 868.4e6)));
    assert_eq!(t.store().packets.len(), 1);
    assert!(t.take_packet(1.006, &packet(AIR_1, 868.4e6)));
    assert_eq!(t.store().packets.len(), 2);
    // A wrong header parity, an encrypted position and a status packet.
    assert_eq!(proto::failed_checks(&with_header_byte(0x03)), 0);
    assert!(!t.take_packet(2.0, &packet(with_header_byte(0x03), 868.4e6)));
    assert!(t.take_packet(3.0, &packet(with_header_byte(0x4B), 868.2e6)));
    let r = t.store().packets.newest_first().next().unwrap();
    assert_eq!((r.kind, r.position), (PacketKind::Encrypted, None));
    assert_eq!(r.decoded, "868.2 MHz, position encrypted, not read");
    assert_eq!(t.store().counters.encrypted, 1);
    assert!(t.take_packet(4.0, &packet(with_header_byte(0x07), 868.2e6)));
    let r = t.store().packets.newest_first().next().unwrap();
    assert_eq!(r.kind, PacketKind::Status);
    assert_eq!(r.decoded, "868.2 MHz, status or information packet");
    assert_eq!(t.store().counters.good, 4);
    // The copy of a packet from the channel beside its own, with less power, changes nothing. With more power it names the
    // channel.
    let mut copy = packet(with_header_byte(0x07), 868.4e6);
    copy.power_dbfs = -50.0;
    assert!(!t.take_packet(4.0001, &copy));
    assert_eq!(
        t.store().packets.newest_first().next().unwrap().channel_hz,
        868.2e6
    );
    copy.power_dbfs = 0.0;
    assert!(!t.take_packet(4.0001, &copy));
    let r = t.store().packets.newest_first().next().unwrap();
    assert_eq!(r.channel_hz, 868.4e6);
    assert_eq!(r.decoded, "868.4 MHz, status or information packet");
    assert_eq!(t.store().counters.good, 4);
    // One tracker: the address 123456 of type OGN.
    assert_eq!(t.store().aircraft.len(), 1);
    assert!(t.take_packet(5.0, &packet(AIR_3, 868.2e6)));
    let key = TrackerKey {
        address_type: 2,
        address: 0xDD4711,
    };
    let a = t.store().aircraft.get(&key).expect("the tracker");
    let rep = a.report.expect("a position");
    assert!((rep.position.lat_deg + 36.84999).abs() < 1e-5);
    assert_eq!((rep.alt_m, rep.speed_m_s, rep.second), (1000.0, 34.5, 30));
}

#[test]
fn one_packet_decodes_on_each_channel_with_carrier_and_clock_errors() {
    for (ch, freq) in [(LOW, 868.2e6), (HIGH, 868.4e6)] {
        for cfo in [-20e3, -5e3, 0.0, 15e3] {
            for ppm in [-40.0, 0.0, 40.0] {
                let mut b = Burst::new(0.01, ch, AIR_1);
                b.cfo_hz = cfo;
                b.ppm = ppm;
                let x = air(1e6, vec![b], None, 0.03, 1);
                let (rx, _) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
                let r = rx.results();
                let what = format!("{freq} Hz, offset {cfo}, clock {ppm}");
                assert_eq!(count(r, &keys::FRAMES_OK), 1, "{what}");
                let a = row(r, &typed::OGN_AIRCRAFT, "3/123456").expect("the tracker");
                assert_eq!(a.at(&typed::OgnAircraftRow::CHANNEL_HZ), Some(freq), "{what}");
                assert_eq!(a.at(&typed::OgnAircraftRow::ALT_M), Some(1500.0));
            }
        }
    }
}

#[test]
fn the_packet_shows_in_the_tables_and_on_the_map() {
    let x = air(1e6, vec![Burst::new(0.01, HIGH, AIR_1)], Some(25.0), 0.3, 1);
    let (rx, ev) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
    let r = rx.results();
    assert_eq!(
        lines(r),
        ["glider, 1500 m, 12.3 m/s, heading 180, climb 0.0 m/s, relay 0, 868.4 MHz"]
    );
    let p = row(r, &typed::OGN_PACKETS, &rows(r, &typed::OGN_PACKETS)[0]).unwrap();
    assert_eq!(p.at(&typed::OgnPacketRow::KIND), Some(PacketKind::Position));
    assert_eq!(p.at(&typed::OgnPacketRow::ID), Some("123456 OGN"));
    assert_eq!(p.at(&typed::OgnPacketRow::REPAIRED), Some(false));
    assert!(p.at(&typed::OgnPacketRow::HEX).unwrap().starts_with("56 34 12 0B BD 1A"));
    assert!((p.at(&typed::OgnPacketRow::TIME_S).unwrap() - (0.01 + AIR_S)).abs() < 2e-4);
    let a = row(r, &typed::OGN_AIRCRAFT, "3/123456").unwrap();
    assert_eq!(a.get(&keys::SOURCE), Some("123456"));
    assert_eq!(a.at(&typed::OgnAircraftRow::ADDRESS_TYPE), Some("OGN"));
    assert_eq!(a.at(&typed::OgnAircraftRow::AIRCRAFT_TYPE), Some("glider"));
    let (lat, lon) = a.get(&keys::POSITION).unwrap();
    assert!((lat - 54.68001).abs() < 1e-5 && (lon - 25.28001).abs() < 1e-5);
    assert!((a.get(&keys::SPEED_KT).unwrap() * 0.514_444 - 12.3).abs() < 0.01);
    assert_eq!(a.get(&keys::COURSE_DEG), Some(180.0));
    assert_eq!(a.at(&typed::OgnAircraftRow::CLIMB_M_S), Some(0.0));
    assert_eq!((a.at(&typed::OgnAircraftRow::SECOND), a.at(&typed::OgnAircraftRow::FIX_QUALITY)), (Some(12), Some(0)));
    assert!(r.points().contains_key("aircraft/3/123456"));
    assert_eq!(ev.iter().filter(|e| e.is(&typed::OGN_TRACKER)).count(), 1);
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(
        r.fields().at(&typed::OgnFields::CHANNELS),
        Some("868.2 MHz on, 868.4 MHz on")
    );
    assert!(r.status().summary.starts_with("1 trackers, 1 packets"));
    assert!(r
        .status()
        .summary
        .contains("not confirmed by a public document"));
    assert!(r
        .fields()
        .at(&typed::OgnFields::NOTE)
        .unwrap()
        .contains("not confirmed by a public document"));
}

#[test]
fn two_packets_decode_at_each_rate_and_in_a_capture_beside_the_plan() {
    for rate in [1e6, 2e6, 2.4e6] {
        let b = vec![Burst::new(0.01, HIGH, AIR_1), Burst::new(0.03, HIGH, AIR_2)];
        assert_eq!(good(&air(rate, b, None, 0.05, 1), rate), 2, "{rate}");
    }
    // The capture is centred on 868.875 MHz at 2 Msps: 868.2 MHz is inside, 868.4 MHz also.
    let centre = 868.875e6;
    let b = vec![Burst::new(0.01, 868.2e6 - centre, AIR_1)];
    let x = air(2e6, b, None, 0.3, 1);
    let (rx, _) = receive(&Harness::new(&OGN), &x, 2e6, centre, DIAL);
    let r = rx.results();
    assert_eq!(count(r, &keys::FRAMES_OK), 1);
    assert_eq!(
        row(r, &typed::OGN_AIRCRAFT, "3/123456").unwrap().at(&typed::OgnAircraftRow::CHANNEL_HZ),
        Some(868.2e6)
    );
    assert_eq!(count(r, &typed::OgnFields::CHANNELS_ON), 2);
    // At 1 Msps on that centre no channel is inside the capture, and the stack runs.
    let (rx, _) = receive(&Harness::new(&OGN), &x[..300_000], 1e6, centre, DIAL);
    assert_eq!(count(rx.results(), &typed::OgnFields::CHANNELS_ON), 0);
    assert_eq!(
        rx.results().fields().at(&typed::OgnFields::CHANNELS),
        Some("868.2 MHz off, 868.4 MHz off")
    );
}

#[test]
fn a_mirrored_spectrum_decodes() {
    let mut x = air(1e6, vec![Burst::new(0.01, LOW, AIR_1)], None, 0.03, 1);
    for v in &mut x {
        *v = v.conj();
    }
    assert_eq!(good(&x, 1e6), 1);
}

#[test]
fn packets_decode_in_noise() {
    for snr in [25.0, 18.0, 14.0] {
        for seed in 0..8 {
            let b = vec![Burst::new(0.01, HIGH, AIR_2)];
            let x = air(1e6, b, Some(snr), 0.03, 100 + seed);
            assert_eq!(good(&x, 1e6), 1, "{snr} dB, seed {seed}");
        }
    }
}

#[test]
fn packets_close_together_and_at_the_same_time_decode() {
    // 0.5 ms between the end of one and the start of the next.
    let b = vec![
        Burst::new(0.01, HIGH, AIR_1),
        Burst::new(0.01 + AIR_S + 0.5e-3, HIGH, AIR_2),
    ];
    assert_eq!(good(&air(1e6, b, None, 0.04, 1), 1e6), 2);
    // One on each channel at the same time.
    let b = vec![Burst::new(0.01, LOW, AIR_1), Burst::new(0.01, HIGH, AIR_2)];
    assert_eq!(good(&air(1e6, b, None, 0.04, 1), 1e6), 2);
    // Two on one channel at the same time, the second 26 dB weaker: the stronger one is read.
    let mut weak = Burst::new(0.0101, HIGH, AIR_2);
    weak.level_db = -26.0;
    let x = air(1e6, vec![Burst::new(0.01, HIGH, AIR_1), weak], None, 0.3, 1);
    let (rx, _) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
    assert!(row(rx.results(), &typed::OGN_AIRCRAFT, "3/123456").is_some());
}

#[test]
fn noise_gives_no_packet() {
    let x = air(1e6, Vec::new(), Some(0.0), 2.0, 5)
        .iter()
        .map(|v| v * (200e3f32 / 1e6).sqrt())
        .collect::<Vec<_>>();
    let (rx, _) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
    let r = rx.results();
    assert_eq!(count(r, &keys::FRAMES_OK), 0);
    assert!(rows(r, &typed::OGN_PACKETS).is_empty() && rows(r, &typed::OGN_AIRCRAFT).is_empty());
    assert_ne!(r.status().lock_state, LockState::Locked);
}

#[test]
fn wrong_bits_on_the_air_are_repaired() {
    let mut p = AIR_2;
    for at in [7usize, 44, 81] {
        p[at / 8] ^= 0x80 >> (at % 8);
    }
    let x = air(1e6, vec![Burst::new(0.01, HIGH, p)], Some(25.0), 0.3, 1);
    let (rx, _) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
    let r = rx.results();
    assert_eq!(count(r, &keys::FRAMES_OK), 1);
    assert_eq!(count(r, &typed::OgnFields::CORRECTED), 1);
    let row0 = row(r, &typed::OGN_PACKETS, &rows(r, &typed::OGN_PACKETS)[0]).unwrap();
    assert_eq!(row0.at(&typed::OgnPacketRow::REPAIRED), Some(true));
    assert_eq!(row0.at(&typed::OgnPacketRow::ID), Some("ABCDEF OGN"));
    assert!(row0.at(&typed::OgnPacketRow::HEX).unwrap().starts_with("EF CD AB 0B D3 33"));
    // Six wrong bits are more than the repair changes.
    for at in [118usize, 130, 150] {
        p[at / 8] ^= 0x80 >> (at % 8);
    }
    let x = air(1e6, vec![Burst::new(0.01, HIGH, p)], Some(25.0), 0.3, 1);
    let (rx, _) = receive(&Harness::new(&OGN), &x, 1e6, DIAL, DIAL);
    assert_eq!(count(rx.results(), &keys::FRAMES_OK), 0);
    assert_eq!(count(rx.results(), &keys::FRAMES_BAD), 1);
}

/// The scene of the stack for `secs` with the settings of `h`, with noise for 20 dB in 200 kHz.
fn scene(h: &Harness, rate: f64, secs: f64) -> Vec<Complex32> {
    let mut x = h
        .clone()
        .rate(rate)
        .seconds(secs)
        .generate()
        .expect("the test signal");
    let p = 10f64.powf(-2.0) * rate / 200e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    for v in &mut x {
        *v += rng.complex_normal(p);
    }
    x
}

/// Makes sure that the four trackers of the scene are in the table, each within 5 m of where it was sent in second
/// `second`.
fn four_trackers(r: &Results, second: f64) {
    let keys4 = ["1/A12345", "2/DD4711", "3/7B0042", "0/5C3901"];
    assert_eq!(rows(r, &typed::OGN_AIRCRAFT).len(), 4);
    for (i, k) in keys4.iter().enumerate() {
        let a = row(r, &typed::OGN_AIRCRAFT, k).unwrap_or_else(|| panic!("no tracker {k}"));
        let (lat, lon) = a.get(&keys::POSITION).expect("a position");
        let sent = scene_report(i, second + 0.05 + 0.2 * i as f64 + 0.014, (54.70, 25.30));
        let d = Position::new(lat, lon).distance_m(&Position::new(sent.lat_deg, sent.lon_deg));
        assert!(d < 5.0, "{k}: {d} m");
        assert!(r.points().contains_key(&format!("aircraft/{k}")));
    }
    assert_eq!(
        row(r, &typed::OGN_AIRCRAFT, "3/7B0042").unwrap().at(&typed::OgnAircraftRow::AIRCRAFT_TYPE),
        Some("paraglider")
    );
    assert_eq!(
        row(r, &typed::OGN_AIRCRAFT, "0/5C3901").unwrap().get(&keys::PRIVATE),
        Some(true)
    );
}

#[test]
fn the_test_signal_gives_four_trackers_a_status_and_an_encrypted_packet() {
    let h = Harness::new(&OGN);
    let x = scene(&h, 1e6, 5.0);
    let (mut rx, ev) = receive(&h, &x, 1e6, DIAL, DIAL);
    let r = rx.results();
    four_trackers(r, 4.0);
    assert_eq!(count(r, &keys::FRAMES_OK), 22);
    assert_eq!(count(r, &keys::FRAMES_BAD), 0);
    assert_eq!(count(r, &typed::OgnFields::ENCRYPTED), 1);
    let kinds: Vec<PacketKind> = rows(r, &typed::OGN_PACKETS)
        .iter()
        .filter_map(|k| row(r, &typed::OGN_PACKETS, k)?.at(&typed::OgnPacketRow::KIND))
        .collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == PacketKind::Status).count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == PacketKind::Encrypted)
            .count(),
        1
    );
    let on: Vec<f64> = rows(r, &typed::OGN_PACKETS)
        .iter()
        .filter_map(|k| row(r, &typed::OGN_PACKETS, k)?.at(&typed::OgnAircraftRow::CHANNEL_HZ))
        .collect();
    assert!(on.contains(&868.2e6) && on.contains(&868.4e6));
    assert_eq!(ev.iter().filter(|e| e.is(&typed::OGN_TRACKER)).count(), 4);
    assert_eq!(ev.iter().filter(|e| e.is(&typed::OGN_EMERGENCY)).count(), 0);
    assert!(!rows(r, &typed::OGN_TRACKS).is_empty());
    // A new tuning keeps the tables. `clear` empties them and the counters.
    rx.configure(&InputInfo::new(1e6, DIAL + 50e3, DIAL + 50e3).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 300_000], &mut out);
    assert_eq!(rows(rx.results(), &typed::OGN_AIRCRAFT).len(), 4);
    assert_eq!(count(rx.results(), &keys::FRAMES_OK), 22);
    let id = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(&mut *rx, id, &args).unwrap();
    assert!(rows(rx.results(), &typed::OGN_AIRCRAFT).is_empty() && rows(rx.results(), &typed::OGN_PACKETS).is_empty());
    assert_eq!(count(rx.results(), &keys::FRAMES_OK), 0);
}

#[test]
fn the_emergency_flag_gives_one_event_and_wrong_bits_are_repaired() {
    let h = Harness::new(&OGN)
        .set(&GEN_EMERGENCY, true)
        .set(&GEN_BAD_BITS, 3);
    let x = scene(&h, 1e6, 4.0);
    let (rx, ev) = receive(&h, &x, 1e6, DIAL, DIAL);
    let r = rx.results();
    let alarms: Vec<&Event> = ev.iter().filter(|e| e.is(&typed::OGN_EMERGENCY)).collect();
    assert_eq!(alarms.len(), 1);
    assert_eq!(alarms[0].get(&keys::SOURCE), Some("A12345"));
    assert!(alarms[0].get(&keys::POSITION).is_some());
    assert_eq!(count(r, &keys::FRAMES_OK), 18);
    assert_eq!(count(r, &typed::OgnFields::CORRECTED), 6);
    assert_eq!(count(r, &keys::FRAMES_BAD), 0);
}

/// The dial, the count of channels and the spacing of the plan that the country `code` of the data file names first.
fn row_of(file: &sdrkit_data_plans::data::PlanFile, code: &str) -> (f64, i64, f64) {
    let entry = file
        .countries
        .iter()
        .find(|c| c.code == code)
        .expect("the country");
    let group = file
        .groups
        .iter()
        .find(|g| g.id == entry.groups[0])
        .expect("its group");
    let n = group.bands[0]
        .numbering
        .as_ref()
        .expect("numbered channels");
    let count = i64::from(n.last - n.first + 1);
    let dial = n.first_centre_hz + n.step_hz * (count - 1) as f64 / 2.0;
    (dial, count, n.step_hz)
}

#[test]
fn the_data_file_has_the_channel_plans_and_the_row_of_australia_decodes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/drones/ogn.json");
    let text = std::fs::read_to_string(path).expect("the data file");
    let file = sdrkit_data_plans::data::parse("ogn.json", &text).expect("a plan file");
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
    assert_eq!((file.countries.len(), file.regions.len()), (5, 3));
    assert!(file
        .groups
        .iter()
        .all(|g| g.bands.len() == 1 && !g.verified));
    // Europe: the plan of the test signal.
    let eu = file.groups.iter().find(|g| g.id == "ogn_eu868").unwrap();
    let n = eu.bands[0].numbering.as_ref().unwrap();
    assert_eq!((n.first_centre_hz, n.step_hz, n.last), (868.2e6, 200e3, 1));
    // The row of Australia: 24 channels 400 kHz apart around 921.6 MHz. The consumer sets the count and the spacing and
    // tunes the session there.
    let (dial, channels, step) = row_of(&file, "AU");
    assert!((dial - 921.6e6).abs() < 1.0);
    assert_eq!((channels, step), (24, 400e3));
    let h = Harness::new(&OGN)
        .set(&CHANNELS, channels)
        .set(&CHANNEL_STEP_HZ, step);
    // At the rate of the stack the two middle channels are inside the capture.
    let x = scene(&h, 1e6, 5.0);
    let (rx, _) = receive(&h, &x, 1e6, dial, dial);
    let r = rx.results();
    four_trackers(r, 4.0);
    assert_eq!(count(r, &typed::OgnFields::CHANNELS_ON), 2);
    assert_eq!(
        r.fields().at(&typed::OgnFields::CHANNELS),
        Some("24 channels, 917.0 to 926.2 MHz, 2 on, 22 outside the capture")
    );
    let a = row(r, &typed::OGN_AIRCRAFT, "1/A12345").unwrap();
    let f = a.at(&typed::OgnAircraftRow::CHANNEL_HZ).unwrap();
    assert!(
        (f - 921.4e6).abs() < 1.0 || (f - 921.8e6).abs() < 1.0,
        "{f}"
    );
    // At 10 Msps every channel is inside, and the trackers send on all of them in turn.
    let x = scene(&h, 10e6, 2.0);
    let (rx, _) = receive(&h, &x, 10e6, dial, dial);
    let r = rx.results();
    assert_eq!(count(r, &typed::OgnFields::CHANNELS_ON), 24);
    four_trackers(r, 1.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    let on: std::collections::BTreeSet<i64> = rows(r, &typed::OGN_PACKETS)
        .iter()
        .filter_map(|k| row(r, &typed::OGN_PACKETS, k)?.at(&typed::OgnAircraftRow::CHANNEL_HZ))
        .map(|f| f.round() as i64)
        .collect();
    assert!(on.len() >= 5 && on.contains(&917_000_000), "{on:?}");
}
