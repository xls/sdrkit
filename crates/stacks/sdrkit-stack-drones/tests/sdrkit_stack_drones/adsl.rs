//! ADS-L: the worked packets on the air on each band, the tolerances of the channel receivers, the repair and the trust
//! rule, FLARM beside ADS-L, the plan data file and the test signal.

use super::{count, feed, row, rows};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::{field, keys, Declare};
use sdrkit_core::{Complex32, Generator, LockState, Results, Station};
use sdrkit_dev_test::Harness;
use sdrkit_stack_drones::typed;
use sdrkit_stack_drones::adsl::channels::{Band, BANDS, M1_HZ, M2_HZ, O_HZ, TONE_CHIPS};
use sdrkit_stack_drones::adsl::generator::{AdslScene, Burst};
use sdrkit_stack_drones::adsl::proto::{
    self, network_packet, Address, DataHeader, NetHeader, Status, Traffic,
};
use sdrkit_stack_drones::adsl::*;
use sdrkit_stack_drones::flarm::{START_TIME, WALL_CLOCK};
use sdrkit_stack_drones::ADSL;
use sdrkit_util_geo::Position;

const WIDE: f64 = 868.875e6;
/// The packet counts of the four bands, in band order.
const BAND_GOOD: [Column<field::I64>; 4] = [
    typed::AdslFields::PACKETS_868_2,
    typed::AdslFields::PACKETS_868_4,
    typed::AdslFields::PACKETS_LDR,
    typed::AdslFields::PACKETS_HDR,
];
/// Whether each band is received, in band order.
const BAND_ON: [Column<field::Bool>; 4] = [
    typed::AdslFields::ON_868_2,
    typed::AdslFields::ON_868_4,
    typed::AdslFields::ON_LDR,
    typed::AdslFields::ON_HDR,
];
const STATION: (f64, f64) = (54.70, 25.30);

fn station() -> Station {
    Station::at(Position::new(STATION.0, STATION.1))
}

fn h(rate: f64, centre: f64) -> Harness {
    Harness::new(&ADSL)
        .rate(rate)
        .freq(centre)
        .station(station())
}

/// A Traffic payload of an airborne glider at `lat`.
fn traffic(lat: f64) -> Traffic {
    Traffic {
        flight_state: 2,
        category: 4,
        emergency: 1,
        position: Some((lat, 25.3)),
        speed_m_s: Some(31.5),
        alt_m: Some(1234.0),
        climb_m_s: Some(-1.25),
        track_deg: Some(247.5),
        hfom: 6,
        vfom: 2,
        velocity_accuracy: 2,
        ..Traffic::default()
    }
}

fn head(version: u8, key_index: u8, rs: bool) -> NetHeader {
    NetHeader {
        version,
        signed: false,
        key_index,
        reed_solomon: rs,
    }
}

/// The network packet of a Traffic payload from `address` under the address mapping table `amt`.
fn traffic_packet(amt: u8, address: u32, t: &Traffic, head: NetHeader) -> Vec<u8> {
    let d = proto::data(
        DataHeader {
            payload_type: 2,
            address: Address { amt, address },
            relay: false,
        },
        &t.bytes(),
    );
    network_packet(head, &d)
}

/// The worked Traffic packet: FLARM DD4711, a glider at 54.7 N 25.3 E.
fn a1() -> Vec<u8> {
    traffic_packet(6, 0xDD4711, &traffic(54.7), head(1, 0, false))
}

/// The scene at `rate` centred on `centre` with white noise for `snr_db` in 200 kHz (for a packet at 0 dB).
fn air(rate: f64, centre: f64, bursts: Vec<Burst>, snr_db: f64, secs: f64) -> Vec<Complex32> {
    let mut g = AdslScene::new(rate, centre, bursts, Some(secs));
    let mut x = vec![Complex32::default(); (secs * rate) as usize];
    g.generate(&mut x);
    let p = 10f64.powf(-snr_db / 10.0) * rate / 200e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(17);
    for v in &mut x {
        *v += rng.complex_normal(p);
    }
    x
}

fn aircraft(r: &Results) -> usize {
    rows(r, &typed::ADSL_AIRCRAFT).len()
}

fn key(amt: u8, address: u32) -> String {
    format!("{amt}/{address:06X}")
}

#[test]
fn the_worked_packet_decodes_on_every_band() {
    assert_eq!(
        a1(),
        [
            0x01, 0x57, 0xA8, 0xE2, 0x6B, 0x5A, 0x73, 0xBA, 0x8D, 0x52, 0x77, 0xB4, 0x7E, 0x4B,
            0x6D, 0xD6, 0xEF, 0xF3, 0x5E, 0xDC, 0xDA, 0xF0, 0x65, 0xE0
        ]
    );
    for band in BANDS {
        let x = air(2e6, WIDE, vec![Burst::new(0.05, band, a1())], 25.0, 0.3);
        let run = feed(&h(2e6, WIDE), x);
        let r = &run.results;
        let a = row(r, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711)).unwrap_or_else(|| panic!("{band:?}"));
        assert_eq!(a.get(&keys::SOURCE), Some("DD4711"));
        assert_eq!(a.at(&typed::AdslAircraftRow::ADDRESS_SOURCE), Some("FLARM"));
        assert_eq!(a.at(&typed::AdslAircraftRow::CATEGORY), Some("glider"));
        assert_eq!(a.at(&typed::AdslAircraftRow::FLIGHT_STATE), Some("airborne"));
        let (lat, lon) = a.get(&keys::POSITION).unwrap();
        assert!((lat - 54.7).abs() < 2e-5 && (lon - 25.3).abs() < 4e-5);
        assert!((a.at(&typed::FlarmAircraftRow::ALT_M).unwrap() - 1234.0).abs() <= 2.0);
        let kmh = a.get(&keys::SPEED_KT).unwrap() * 1.852;
        assert!((kmh - 113.4).abs() < 0.6, "{kmh}");
        assert!((a.at(&typed::FlarmAircraftRow::CLIMB_M_S).unwrap() + 1.25).abs() < 0.07);
        assert!((a.get(&keys::COURSE_DEG).unwrap() - 247.5).abs() < 0.5);
        assert_eq!(a.at(&typed::AdslAircraftRow::HFOM), Some(6));
        assert_eq!(
            a.at(&typed::FlarmAircraftRow::CHANNEL_HZ),
            Some(band.frequency_hz())
        );
        assert_eq!(count(r, &BAND_GOOD[band.slot()]), 1, "{band:?}");
        assert_eq!(count(r, &typed::AdslFields::TRAFFIC), 1);
        assert_eq!(r.status().lock_state, LockState::Locked);
        // The station is at the centre of the scene: the aircraft is within a few metres of it.
        assert!(a.at(&typed::FlarmAircraftRow::DISTANCE_KM).unwrap() < 0.1);
        assert!(r.points().contains_key("aircraft/6/DD4711"));
        assert_eq!(
            run.events.iter().filter(|e| e.is(&typed::ADSL_AIRCRAFT_SEEN)).count(),
            1
        );
    }
}

#[test]
fn every_form_of_the_packet_decodes_with_the_default_and_the_shortest_preamble() {
    for band in BANDS {
        for short in [false, true] {
            let mut bursts = Vec::new();
            let mut n = 0u32;
            for version in [0, 1] {
                for key_index in [0, 3] {
                    for rs in [false, true] {
                        if rs && band == Band::Ldr {
                            continue;
                        }
                        let p = traffic_packet(
                            5,
                            0x400000 + n,
                            &traffic(54.0),
                            head(version, key_index, rs),
                        );
                        let mut b = Burst::new(0.03 + 0.025 * f64::from(n), band, p);
                        if short {
                            b.preamble = match band {
                                Band::Ldr => 20,
                                Band::Hdr => 6,
                                _ => 5,
                            };
                        }
                        bursts.push(b);
                        n += 1;
                    }
                }
            }
            let x = air(2e6, WIDE, bursts, 25.0, 0.3);
            let run = feed(&h(2e6, WIDE), x);
            assert_eq!(aircraft(&run.results), n as usize, "{band:?} short {short}");
            assert_eq!(count(&run.results, &typed::AdslFields::CORRECTED), 0);
        }
    }
}

#[test]
fn the_channels_take_carrier_and_clock_errors_and_an_inverted_spectrum() {
    for band in BANDS {
        let step = if band == Band::Ldr { 5e3 } else { 10e3 };
        let mut bursts = Vec::new();
        let mut n = 0u32;
        for k in -2..=2 {
            for ppm in [-40.0, 0.0, 40.0] {
                let p = traffic_packet(5, 0x410000 + n, &traffic(54.0), head(1, 0, false));
                let mut b = Burst::new(0.03 + 0.025 * f64::from(n), band, p);
                b.cfo_hz = step * f64::from(k);
                b.ppm = ppm;
                bursts.push(b);
                n += 1;
            }
        }
        let mut inv = Burst::new(0.03 + 0.025 * f64::from(n), band, a1());
        inv.inverted = true;
        bursts.push(inv);
        let x = air(2e6, WIDE, bursts, 25.0, 0.5);
        let run = feed(&h(2e6, WIDE), x);
        assert_eq!(aircraft(&run.results), n as usize + 1, "{band:?}");
        assert!(row(&run.results, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711)).is_some());
    }
}

#[test]
fn every_band_decodes_at_25_and_18_db() {
    for snr in [25.0, 18.0] {
        let mut bursts = Vec::new();
        for (j, band) in BANDS.iter().enumerate() {
            for k in 0..6u32 {
                let n = j as u32 * 16 + k;
                let p = traffic_packet(5, 0x420000 + n, &traffic(54.0), head(1, 0, false));
                // The two O-band forms share a frequency: they are sent one after the other.
                let mut b = Burst::new(
                    0.03 + 0.03 * f64::from(k) + 0.2 * (j / 2) as f64 * (j % 2) as f64,
                    *band,
                    p,
                );
                b.cfo_hz = 3e3;
                bursts.push(b);
            }
        }
        let x = air(2e6, WIDE, bursts, snr, 0.5);
        let run = feed(&h(2e6, WIDE), x);
        assert_eq!(aircraft(&run.results), 24, "{snr} dB");
    }
}

/// Thirty M-band packets with the carrier offset `cfo` at `snr_db`, through a receiver with these settings. Returns the
/// aircraft decoded.
fn m_band(snr_db: f64, cfo: f64, tone: bool) -> usize {
    let bursts = (0..30u32)
        .map(|n| {
            let p = traffic_packet(5, 0x430000 + n, &traffic(54.0), head(1, 0, false));
            let band = if n % 2 == 0 { Band::M1 } else { Band::M2 };
            let mut b = Burst::new(0.03 + 0.012 * f64::from(n), band, p);
            b.cfo_hz = cfo;
            b
        })
        .collect();
    let x = air(2e6, WIDE, bursts, snr_db, 0.45);
    let run = feed(&h(2e6, WIDE).set(&TONE_CHIPS, tone), x);
    aircraft(&run.results)
}

#[test]
fn the_tone_detector_of_the_m_band_decodes_more_near_the_threshold() {
    assert_eq!(m_band(30.0, 0.0, true), 30);
    assert_eq!(m_band(20.0, 15e3, true), 30);
    assert_eq!(m_band(20.0, -15e3, true), 30);
    let (tone, disc) = (m_band(5.0, 2e3, true), m_band(5.0, 2e3, false));
    assert!(tone > disc, "tone {tone}, discriminator {disc}");
}

#[test]
fn packets_that_overlap_or_follow_closely_are_decoded() {
    let p = |n: u32| traffic_packet(5, 0x440000 + n, &traffic(54.0), head(1, 0, false));
    // A second packet 1.5 ms later and 14 dB weaker on the same channel: the stronger one is decoded.
    let mut weak = Burst::new(0.0515, Band::M1, p(1));
    weak.level_db = -14.0;
    let x = air(
        2e6,
        WIDE,
        vec![Burst::new(0.05, Band::M1, p(0)), weak],
        30.0,
        0.2,
    );
    let run = feed(&h(2e6, WIDE), x);
    assert!(row(&run.results, &typed::ADSL_AIRCRAFT, &key(5, 0x440000)).is_some());
    // The four channels at the same time (LDR 30 ms later, on the frequency of HDR).
    let bursts = vec![
        Burst::new(0.05, Band::M1, p(2)),
        Burst::new(0.05, Band::M2, p(3)),
        Burst::new(0.05, Band::Hdr, p(4)),
        Burst::new(0.08, Band::Ldr, p(5)),
    ];
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, bursts, 25.0, 0.2));
    assert_eq!(aircraft(&run.results), 4);
    // Two M-band packets 4.8 ms apart (a packet lasts 4.56 ms).
    let bursts = vec![
        Burst::new(0.05, Band::M1, p(6)),
        Burst::new(0.0548, Band::M1, p(7)),
    ];
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, bursts, 25.0, 0.2));
    assert_eq!(aircraft(&run.results), 2);
    // The neighbour M channel 20 dB stronger at the same time.
    let mut quiet = Burst::new(0.05, Band::M1, p(8));
    quiet.level_db = -20.0;
    let bursts = vec![quiet, Burst::new(0.05, Band::M2, p(9))];
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, bursts, 45.0, 0.2));
    assert_eq!(aircraft(&run.results), 2);
}

#[test]
fn a_wrong_length_byte_a_signature_and_a_reserved_key_are_handled() {
    for band in [Band::M1, Band::Hdr] {
        // The length byte 24 with two wrong bits.
        let mut b = Burst::new(0.05, band, a1());
        b.length = Some(24 ^ 0x41);
        // A Status packet with a wrong length byte, after a Traffic packet of the same aircraft.
        let st = proto::data(
            DataHeader {
                payload_type: 3,
                address: Address {
                    amt: 6,
                    address: 0xDD4711,
                },
                relay: false,
            },
            &Status {
                version: 1,
                rx_m: 3,
                ..Status::default()
            }
            .bytes(),
        );
        let mut s = Burst::new(0.1, band, network_packet(head(1, 0, false), &st));
        s.length = Some(16 ^ 0x05);
        let run = feed(&h(2e6, WIDE), air(2e6, WIDE, vec![b, s], 25.0, 0.2));
        let r = &run.results;
        assert_eq!(aircraft(r), 1, "{band:?}");
        assert_eq!(count(r, &typed::AdslFields::STATUS), 1, "{band:?}");
        let caps = row(r, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711))
            .and_then(|a| a.at(&typed::AdslAircraftRow::CAPABILITIES))
            .unwrap();
        assert!(caps.starts_with("version 1, receives M 3"), "{caps}");
        // A signature after the parity: decoded as without it.
        let mut signed = head(1, 0, false);
        signed.signed = true;
        let mut b = Burst::new(
            0.05,
            band,
            traffic_packet(5, 0x450000, &traffic(54.0), signed),
        );
        b.tail = (0..68u8).map(|i| i.wrapping_mul(37) ^ 0x5A).collect();
        let run = feed(&h(2e6, WIDE), air(2e6, WIDE, vec![b], 25.0, 0.2));
        assert_eq!(aircraft(&run.results), 1);
        assert_eq!(count(&run.results, &typed::AdslFields::SIGNED), 1);
    }
    // Key index 1 has no published key: the packet is counted and not read.
    let p = traffic_packet(5, 0x450001, &traffic(54.0), head(1, 1, false));
    let run = feed(
        &h(2e6, WIDE),
        air(2e6, WIDE, vec![Burst::new(0.05, Band::M1, p)], 25.0, 0.2),
    );
    assert_eq!(count(&run.results, &typed::AdslFields::ENCRYPTED), 1);
    assert_eq!(aircraft(&run.results), 0);
    // A Status packet of an aircraft that is not in the table changes nothing.
    let st = proto::data(
        DataHeader {
            payload_type: 3,
            address: Address {
                amt: 6,
                address: 0xDD4711,
            },
            relay: false,
        },
        &Status::default().bytes(),
    );
    let b = Burst::new(0.05, Band::M2, network_packet(head(1, 0, false), &st));
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, vec![b], 25.0, 0.2));
    assert_eq!(
        (count(&run.results, &typed::AdslFields::STATUS), aircraft(&run.results)),
        (1, 0)
    );
}

#[test]
fn a_repaired_packet_updates_an_aircraft_and_never_makes_one() {
    let mut wrong = a1();
    wrong[9] ^= 0x08;
    let run = feed(
        &h(2e6, WIDE),
        air(
            2e6,
            WIDE,
            vec![Burst::new(0.05, Band::M1, wrong.clone())],
            25.0,
            0.2,
        ),
    );
    assert_eq!(count(&run.results, &typed::AdslFields::CORRECTED), 1);
    assert_eq!(aircraft(&run.results), 0);
    let bursts = vec![
        Burst::new(0.05, Band::M1, a1()),
        Burst::new(0.10, Band::M2, wrong),
    ];
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, bursts, 25.0, 0.2));
    let r = &run.results;
    assert_eq!(count(r, &typed::AdslFields::CORRECTED), 1);
    let a = row(r, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711)).unwrap();
    assert_eq!(
        a.get(&sdrkit_util_objects::publish::columns::HEARD),
        Some(2)
    );
}

#[test]
fn an_uplink_gives_its_aircraft_as_relayed_and_a_random_address_is_private() {
    let list: Vec<(Address, Traffic)> = (1..=3u32)
        .map(|i| {
            (
                Address {
                    amt: 5,
                    address: 0x500000 + i,
                },
                traffic(50.0 + f64::from(i)),
            )
        })
        .collect();
    let d = proto::uplink_data(
        Address {
            amt: 7,
            address: 0xF00001,
        },
        &list,
    );
    let bursts = vec![
        Burst::new(0.05, Band::Hdr, network_packet(head(1, 0, true), &d)),
        Burst::new(
            0.10,
            Band::M1,
            traffic_packet(0, 0x123456, &traffic(54.0), head(1, 0, false)),
        ),
    ];
    let run = feed(&h(2e6, WIDE), air(2e6, WIDE, bursts, 25.0, 0.2));
    let r = &run.results;
    assert_eq!(count(r, &typed::AdslFields::UPLINK), 1);
    for i in 1..=3u32 {
        let a = row(r, &typed::ADSL_AIRCRAFT, &key(5, 0x500000 + i)).unwrap();
        assert_eq!(a.at(&typed::AdslAircraftRow::RELAYED), Some(true));
        let (lat, _) = a.get(&keys::POSITION).unwrap();
        assert!((lat - 50.0 - f64::from(i)).abs() < 2e-5);
    }
    let a = row(r, &typed::ADSL_AIRCRAFT, &key(0, 0x123456)).unwrap();
    assert_eq!(a.get(&keys::PRIVATE), Some(true));
    assert_eq!(a.at(&typed::AdslAircraftRow::ADDRESS_SOURCE), Some("random"));
    let seen: Vec<_> = run.events.iter().filter(|e| e.is(&typed::ADSL_AIRCRAFT_SEEN)).collect();
    assert_eq!(seen.len(), 4);
    assert!(seen
        .iter()
        .all(|e| e.get(&keys::PRIVATE) == Some(e.get(&keys::SOURCE) == Some("123456"))));
}

#[test]
fn each_capture_receives_the_bands_that_fit() {
    let packets = |bands: &[Band]| -> Vec<Burst> {
        let mut v = Vec::new();
        for (j, b) in bands.iter().enumerate() {
            for k in 0..3u32 {
                let n = j as u32 * 8 + k;
                let p = traffic_packet(5, 0x460000 + n, &traffic(54.0), head(1, 0, false));
                v.push(Burst::new(0.03 + 0.03 * f64::from(n), *b, p));
            }
        }
        v
    };
    for rate in [2e6, 2.4e6, 2.56e6, 3.2e6] {
        let run = feed(&h(rate, WIDE), air(rate, WIDE, packets(&BANDS), 25.0, 1.0));
        assert_eq!(aircraft(&run.results), 12, "{rate}");
        assert_eq!(
            BAND_ON.map(|f| run.results.fields().at(&f)),
            [Some(true); 4]
        );
    }
    for rate in [1e6, 1.024e6, 1.2e6] {
        let m = [Band::M1, Band::M2];
        let run = feed(
            &h(rate, 868.3e6),
            air(rate, 868.3e6, packets(&m), 25.0, 0.6),
        );
        assert_eq!(aircraft(&run.results), 6, "{rate}");
        assert_eq!(
            BAND_ON.map(|f| run.results.fields().at(&f)),
            [Some(true), Some(true), Some(false), Some(false)]
        );
    }
    let o = [Band::Ldr, Band::Hdr];
    let run = feed(
        &h(1e6, 869.525e6),
        air(1e6, 869.525e6, packets(&o), 25.0, 0.6),
    );
    assert_eq!(aircraft(&run.results), 6);
}

#[test]
fn noise_at_full_level_gives_no_packet() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<Complex32> = (0..16_000_000).map(|_| rng.complex_normal(1.0)).collect();
    let run = feed(&h(2e6, WIDE), x);
    let r = &run.results;
    assert_eq!(count(r, &keys::FRAMES_OK), 0);
    assert_eq!(count(r, &typed::AdslFields::CORRECTED), 0);
    assert_eq!(aircraft(r), 0);
}

#[test]
fn flarm_of_the_same_capture_is_decoded_beside_adsl() {
    use sdrkit_stack_drones::flarm::generator::{burst_samples, Burst as FlarmBurst};
    use sdrkit_stack_drones::flarm::proto::V6Tx;
    const T0: f64 = 1_800_000_020.0;
    let at = Position::new(STATION.0, STATION.1).destination(307.0, 2500.0);
    let tx = V6Tx {
        address: 0xDD1234,
        addr_type: 2,
        aircraft_type: 1,
        lat: at.lat_deg,
        lon: at.lon_deg,
        alt_m: 800.0,
        speed: 25.0,
        track: 90.0,
        climb: 1.0,
        airborne: true,
        stealth: false,
        no_track: false,
    };
    let rate = 2e6;
    let mut scene = sdrkit_algo_sim::scene::Scene::new(rate)
        .unwrap()
        .with_duration(1.6)
        .unwrap();
    // Two FLARM packets (the second confirms the first) on 868.2 and 868.4 MHz, and one ADS-L packet.
    for (k, ch) in [0u8, 1].into_iter().enumerate() {
        let t = 0.1 + 0.9 * k as f64;
        let b = FlarmBurst::v6(t, ch, &tx, T0 + t);
        let offset = 868.2e6 + 200e3 * f64::from(ch) - WIDE;
        scene
            .add_burst(t, offset, 0.0, burst_samples(&b, rate))
            .unwrap();
    }
    let adsl = Burst::new(0.5, Band::M1, a1()).in_scene(rate, WIDE);
    scene
        .add_burst(adsl.start_s, adsl.offset_hz, 0.0, adsl.samples)
        .unwrap();
    let mut x = vec![Complex32::default(); (1.6 * rate) as usize];
    scene.generate(&mut x);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    for v in &mut x {
        *v += rng.complex_normal(0.03);
    }
    let hx = h(rate, WIDE).set(&WALL_CLOCK, false).set(&START_TIME, T0);
    let run = feed(&hx, x.clone());
    let r = &run.results;
    let f = row(r, &typed::ADSL_AIRCRAFT, &key(FLARM_RADIO, 0xDD1234)).expect("the FLARM aircraft");
    assert_eq!(f.at(&typed::AdslAircraftRow::ADDRESS_SOURCE), Some("FLARM (old protocol)"));
    let (lat, lon) = f.get(&keys::POSITION).expect("a position");
    assert!((lat - at.lat_deg).abs() < 1e-3 && (lon - at.lon_deg).abs() < 1e-3);
    assert!(row(r, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711)).is_some());
    assert_eq!(count(r, &typed::AdslFields::FLARM_PACKETS), 2);
    // Without the FLARM part only the ADS-L aircraft shows.
    let run = feed(&hx.set(&channels::WITH_FLARM, false), x);
    assert_eq!(rows(&run.results, &typed::ADSL_AIRCRAFT), [key(6, 0xDD4711)]);
}

#[test]
fn the_data_file_has_the_channels_of_the_standard_and_its_settings_decode() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/adsl.json");
    let text = std::fs::read_to_string(path).expect("the data file");
    let file = sdrkit_data_plans::data::parse("adsl.json", &text).expect("a plan file");
    let mut plans = (*sdrkit_dev_test::plans()).clone();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    assert_eq!(file.groups.len(), 1);
    let bands = &file.groups[0].bands;
    assert_eq!(bands.len(), 4);
    assert_eq!(file.countries.len(), 46);
    // ADS-L exists in Europe only: the frequencies of that row are set as the consumer of the library sets them.
    let centre = |i: usize| (bands[i].start_hz + bands[i].end_hz) / 2.0;
    assert_eq!((centre(2), centre(3)), (869.525e6, 869.525e6));
    let hx = h(2e6, WIDE)
        .set(&M1_HZ, centre(0))
        .set(&M2_HZ, centre(1))
        .set(&O_HZ, centre(2))
        .seconds(4.0);
    let run = super::run(hx);
    assert!(aircraft(&run.results) >= 4);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
    assert!(run.requests.is_empty());
    // A channel that is set to another frequency is received there, and no longer on the old one.
    let moved = Burst::new(0.05, Band::M1, a1()).in_scene(2e6, WIDE - 600e3);
    let mut scene = sdrkit_algo_sim::scene::Scene::new(2e6)
        .unwrap()
        .with_duration(0.3)
        .unwrap();
    scene
        .add_burst(moved.start_s, moved.offset_hz, 0.0, moved.samples)
        .unwrap();
    let mut x = vec![Complex32::default(); 600_000];
    scene.generate(&mut x);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(6);
    for v in &mut x {
        *v += rng.complex_normal(0.03);
    }
    assert_eq!(aircraft(&feed(&h(2e6, WIDE), x.clone()).results), 0);
    let run = feed(&h(2e6, WIDE).set(&M1_HZ, 868.8e6), x);
    let a = row(&run.results, &typed::ADSL_AIRCRAFT, &key(6, 0xDD4711)).expect("the aircraft");
    assert_eq!(
        a.at(&typed::FlarmAircraftRow::CHANNEL_HZ),
        Some(868.8e6)
    );
}

#[test]
fn the_test_signal_gives_four_aircraft_on_all_bands_and_the_state_survives() {
    use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
    let hx = h(2e6, WIDE)
        .seconds(4.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 25.0);
    let run = super::run(hx);
    let r = &run.results;
    assert!(aircraft(r) >= 4);
    for f in &BAND_GOOD {
        assert!(count(r, f) >= 1, "{}", Declare::key(f));
    }
    assert!(count(r, &typed::AdslFields::STATUS) >= 1 && count(r, &typed::AdslFields::UPLINK) >= 1);
    assert_eq!(count(r, &typed::AdslFields::CORRECTED), 0);
    assert!(!rows(r, &typed::ADSL_TRACKS).is_empty());
    // The narrow capture holds the M-band only.
    let run = super::run(h(1e6, 868.3e6).seconds(3.0));
    assert_eq!(aircraft(&run.results), 4);
    assert_eq!(count(&run.results, &BAND_GOOD[2]), 0);
    // Wrong bits in the test signal are repaired.
    let run = super::run(h(2e6, WIDE).seconds(4.0).set(&GEN_BAD_BIT, 3));
    assert!(count(&run.results, &typed::AdslFields::CORRECTED) >= 2);
    assert_eq!(aircraft(&run.results), 4);
}
