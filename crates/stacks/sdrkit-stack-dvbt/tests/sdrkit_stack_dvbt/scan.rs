//! The channel scan: its stopping rules on the receiver's results, without signals (fast), and a scan of a whole regional plan
//! with generated multiplexes on some channels and noise on the rest.

use crate::common::{channel_scan, choice, config, country_channels, report_fields};
use sdrkit_core::interface::Hertz;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Receiver, Results, Status};
use sdrkit_dev_test::Awgn;
use sdrkit_proto_mpegts::demux::PlayerStatus;
use sdrkit_proto_mpegts::publish::{NAME, SERVICES, SERVICE_ID, SERVICE_KIND};
use sdrkit_proto_mpegts::si::ServiceKind;
use sdrkit_proto_mpegts::source::{TestMux, TestNetwork, TestService};
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::params::{Bandwidth, CodeRate, Constellation, Guard, Mode, TpsParams};
use sdrkit_stack_dvbt::results::Decoding;
use sdrkit_stack_dvbt::scan::{
    ChannelScan, ScanCommand, ScanNote, ScanResult, ScanService, LOCK_S, NAMES_S, SERVICES_S,
};
use sdrkit_stack_dvbt::settings::BandwidthChoice;
use sdrkit_stack_dvbt::typed::DvbTerrestrialFields;
use sdrkit_stack_dvbt::typed::{DvbtFields, DvbtMultiplexFields};
use std::sync::Arc;

/// The services of a scan result as people read them: "name [kind]", or "service N" without a name.
fn names(s: &[ScanService]) -> Vec<String> {
    use sdrkit_core::Choice;
    s.iter()
        .map(|x| {
            if x.name.is_empty() || x.name == format!("Service {}", x.id) {
                format!("service {}", x.id)
            } else {
                match x.kind {
                    Some(k) => format!(
                        "{} [{}]",
                        x.name,
                        ServiceKind::CHOICES[usize::from(k.index())].label
                    ),
                    None => x.name.clone(),
                }
            }
        })
        .collect()
}

/// The mode of a scan result.
fn mode(
    r: &ScanResult,
) -> (
    Option<Mode>,
    Option<Guard>,
    Option<Constellation>,
    Option<CodeRate>,
) {
    (r.mode, r.guard, r.constellation, r.code_rate)
}

fn published() -> Published {
    Published::from_decl(Arc::new(
        sdrkit_stack_dvbt::typed::shape("dvb_terrestrial").results_decl(),
    ))
}

fn empty(occ: Option<f64>) -> Results {
    let mut p = published();
    p.results_mut()
        .set_status(&Status::new(LockState::Searching));
    p.put(&report_fields(0, occ));
    p.results().clone()
}

/// The fields of a locked report.
fn locked_fields() -> DvbtFields<'static> {
    DvbtFields {
        data_valid: true,
        fec_sync: true,
        fft: Some(Mode::K8),
        guard: Some(Guard::G8),
        constellation: Some(Constellation::Qam64),
        code_rate_hp: Some(CodeRate::R23),
        snr_db: Some(25.0),
        cell_id: Some(7),
        channel_width: Hertz(8e6),
        net_bitrate_bps: Some(19.9e6),
        rs_blocks: 100,
        rs_failed: 5,
        ..report_fields(3, Some(20.0))
    }
}

fn locked_published(services: &[(u16, &str, ServiceKind)], network: &str) -> Published {
    locked_published_with(services, network, &locked_fields())
}

fn locked_published_with(
    services: &[(u16, &str, ServiceKind)],
    network: &str,
    fields: &DvbtFields,
) -> Published {
    let mut p = published();
    p.results_mut().set_status(&Status::new(LockState::Locked));
    p.put(fields);
    p.put(&DvbtMultiplexFields {
        network_name: network,
        network_id: 0x3001,
        service_count: services.len() as i64,
        utc: None,
        selected_service: None,
        player_status: PlayerStatus::NoService,
        mux_kbps: 0.0,
    });
    // The table keeps the newest row first: the rows are written in list order.
    for &(id, name, kind) in services {
        let mut row = p.row_mut(&SERVICES, &id.to_string()).unwrap();
        row.set(&SERVICE_ID, i64::from(id));
        row.set(&NAME, name);
        row.set(&SERVICE_KIND, kind);
    }
    p
}

fn locked(services: &[(u16, &str, ServiceKind)], network: &str) -> Results {
    locked_published(services, network).results().clone()
}

/// Drives the scan with `f(channel index, time in channel)` as the results, every 50 ms. Returns the time taken per channel.
fn drive(scan: &mut ChannelScan, f: impl Fn(usize, f64) -> Results) -> Vec<f64> {
    let mut now = 0.0;
    let mut cmd = scan.start(now);
    let mut times = Vec::new();
    let mut ch = 0;
    while let ScanCommand::Tune { .. } = cmd {
        let t0 = now;
        loop {
            now += 0.05;
            if let Some(c) = scan.on_report(now, &f(ch, now - t0)) {
                cmd = c;
                break;
            }
            assert!(now - t0 < 30.0, "the scan never moved on");
        }
        times.push(now - t0);
        ch += 1;
    }
    times
}

#[test]
fn empty_channels_are_skipped_at_once_but_only_once_measured() {
    let mut s = ChannelScan::list(vec![(474e6, None), (482e6, None)], 600e6);
    let t = drive(&mut s, |_, _| empty(Some(1.0)));
    assert!(t.iter().all(|&x| x < 0.3), "{t:?}");
    assert!(s
        .results
        .iter()
        .all(|r| r.note == Some(ScanNote::Empty) && !r.locked));
    // No occupancy yet: the channel is not called empty. After the longest settle it is tried for a lock.
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    let t = drive(&mut s, |_, _| empty(None));
    assert!(t[0] > LOCK_S, "{t:?}");
    assert_eq!(s.results[0].note, Some(ScanNote::NoSignal));
}

#[test]
fn an_occupied_channel_without_dvbt_is_noted() {
    let mut s = ChannelScan::list(vec![(474e6, Some(Bandwidth::B8))], 600e6);
    drive(&mut s, |_, _| {
        let mut p = published();
        p.put(&report_fields(1, Some(12.0)));
        p.results().clone()
    });
    let r = &s.results[0];
    assert!(r.seen && !r.locked);
    assert_eq!(r.note, Some(ScanNote::NoLock));
    assert_eq!(r.occupancy_db, 12.0);
    assert_eq!(r.bandwidth_mhz, 8);
}

#[test]
fn the_service_list_ends_when_named_and_the_network_known() {
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    let t = drive(&mut s, |_, _| {
        locked(
            &[
                (2, "Two", ServiceKind::Tv),
                (1, "One", ServiceKind::Tv),
                (3, "Tunes", ServiceKind::Radio),
            ],
            "Net",
        )
    });
    assert!(t[0] < 1.0, "{t:?}");
    let r = &s.results[0];
    assert!(r.locked);
    assert_eq!(
        mode(r),
        (
            Some(Mode::K8),
            Some(Guard::G8),
            Some(Constellation::Qam64),
            Some(CodeRate::R23)
        )
    );
    assert_eq!(r.network_name, "Net");
    assert_eq!(r.network_id, 0x3001);
    assert_eq!(r.cell_id, 7);
    assert_eq!(r.standard, Decoding::Dvbt);
    assert!((r.rs_good - 0.95).abs() < 1e-6);
    // In the list's order.
    assert_eq!(
        names(&r.services),
        ["Two [TV]", "One [TV]", "Tunes [Radio]"]
    );
    assert_eq!(s.multiplexes().count(), 1);
    assert_eq!(s.services().len(), 3);
}

#[test]
fn unnamed_services_wait_at_most_names_s_and_the_list_at_most_services_s() {
    // Names without a network name: 3 s after the first names.
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    let t = drive(&mut s, |_, _| locked(&[(1, "One", ServiceKind::Tv)], ""));
    assert!(t[0] >= NAMES_S && t[0] < NAMES_S + 1.0, "{t:?}");
    // One service unnamed: "service N", and the wait is bounded too.
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    drive(&mut s, |_, _| {
        locked(
            &[
                (1, "One", ServiceKind::Tv),
                (9, "Service 9", ServiceKind::Tv),
            ],
            "Net",
        )
    });
    assert_eq!(names(&s.results[0].services), ["One [TV]", "service 9"]);
    // No service list at all: 9 s.
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    let t = drive(&mut s, |_, _| locked(&[], ""));
    assert!(t[0] >= SERVICES_S && t[0] < SERVICES_S + 1.0, "{t:?}");
}

#[test]
fn tps_without_packets_stops_waiting_near_the_end_and_the_standard_comes_from_the_receiver() {
    let mut s = ChannelScan::list(vec![(474e6, None)], 600e6);
    let t = drive(&mut s, |_, _| {
        let fields = DvbtFields {
            fec_sync: false,
            ..locked_fields()
        };
        let mut p = locked_published_with(&[], "", &fields);
        p.put(&DvbTerrestrialFields {
            standard: Decoding::Dvbt2,
        });
        p.results().clone()
    });
    // Settle, nearly the whole lock wait, then the service list.
    assert!(t[0] > LOCK_S - 1.0, "{t:?}");
    assert!(s.results[0].locked);
    assert_eq!(s.results[0].standard, Decoding::Dvbt2);
}

#[test]
fn a_plan_scan_carries_labels_widths_progress_and_stop() {
    let mut s = channel_scan(&country_channels("AU"), 600e6);
    assert_eq!(s.progress(), (0, 32));
    let ScanCommand::Tune { hz, bandwidth } = s.start(0.0) else {
        panic!()
    };
    assert_eq!((hz, bandwidth), (177.5e6, Some(Bandwidth::B7)));
    assert_eq!(s.current_hz(), Some(177.5e6));
    s.on_report(0.3, &empty(Some(0.0)));
    assert_eq!(s.progress(), (1, 32));
    assert_eq!(s.results[0].channel, "6");
    assert_eq!(s.stop(), ScanCommand::Done { hz: 600e6 });
    assert!(s.done());
    assert_eq!(s.on_report(1.0, &empty(Some(0.0))), None);
    // Country and range lists.
    // Germany uses Band III for DAB only (BNetzA): its plan is the UHF part, 28 channels.
    assert_eq!(channel_scan(&country_channels("DE"), 0.0).progress().1, 28);
    assert_eq!(ChannelScan::range(474e6, 858e6, 8e6, 0.0).progress().1, 49);
}

// ---------------------------------------------------------------------------------------------------------------------------------
// A whole regional plan, with signals.

const RATE: f64 = 8e6;

fn multiplex(
    services: &[TestService],
    net: &TestNetwork,
    tsid: u16,
    seconds: f64,
    seed: u64,
) -> Vec<Complex32> {
    let n = (seconds * RATE) as usize;
    let mut x = vec![Complex32::default(); n];
    let mut p = GenParams::new(Bandwidth::B7, TpsParams::default());
    let mut b = TestMux::builder(tsid).network(net.clone());
    for s in services {
        b = b.service(s.clone());
    }
    p.hp = Box::new(b.build().expect("a valid test multiplex"));
    let mut g = DvbtGenerator::new(p);
    use sdrkit_core::Generator;
    g.generate(&mut x);
    Awgn::new(1.0, 25.0, 6.66e6, RATE, seed).process(&mut x);
    x
}

fn noise(seconds: f64, seed: u64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (seconds * RATE) as usize];
    Awgn::new(1.0, 25.0, 6.66e6, RATE, seed).process(&mut x);
    x
}

/// The Australian plan (VHF 6 to 12 with 9A, UHF 28 to 51, all 7 MHz): two multiplexes, on 9A and on 30, each with its own network
/// and named services. Noise everywhere else. The scan visits every channel, skips the empty ones, and lists both multiplexes with
/// every service by name in LCN order.
#[test]
fn a_regional_plan_scan_finds_every_multiplex_and_its_named_services() {
    let net = TestNetwork {
        network_id: 0x3201,
        name: "Test Network".into(),
    };
    let mux_a = [
        TestService::tv(0x0201, "Alpha One", 2),
        TestService::tv(0x0202, "Alpha HD", 20),
        TestService::radio(0x0203, "Alpha Radio", 200),
    ];
    let mux_b = [
        TestService::tv(0x0301, "Bravo", 3),
        TestService::tv(0x0302, "Bravo Kids", 30),
    ];
    let a = multiplex(&mux_a, &net, 0x0201, 1.0, 3);
    let b = multiplex(&mux_b, &net, 0x0301, 1.0, 4);
    let quiet = noise(0.25, 5);

    let mut scan = channel_scan(&country_channels("AU"), 529.5e6);
    let cfg = config(&[]);
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&cfg).unwrap();
    let bandwidth_id = rx.schema().id("bandwidth").unwrap();
    rx.configure(&InputInfo::new(RATE, 529.5e6, 529.5e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut now = 0.0;
    let mut cmd = scan.start(now);
    while let ScanCommand::Tune { hz, bandwidth } = cmd {
        // The engine applies the plan's width and tunes.
        let bw = bandwidth.expect("the plan gives the width");
        owner::apply(&mut rx, bandwidth_id, &choice(BandwidthChoice::of(bw))).unwrap();
        rx.retuned(&InputInfo::new(RATE, hz, hz).recording());
        let sig: &[Complex32] = if hz == 205.5e6 {
            &a
        } else if hz == 543.5e6 {
            &b
        } else {
            &quiet
        };
        let mut next = None;
        // A recording of the channel, looped for as long as the scan stays on it.
        'feed: for _ in 0..12 {
            for c in sig.chunks(16384) {
                rx.feed(c, &mut out);
                out.clear();
                now += c.len() as f64 / RATE;
                if let Some(n) = scan.on_report(now, Owner::results(&rx)) {
                    next = Some(n);
                    break 'feed;
                }
            }
        }
        cmd = next.expect("the scan moves on");
    }
    assert_eq!(cmd, ScanCommand::Done { hz: 529.5e6 });
    assert_eq!(scan.results.len(), 32);
    let found: Vec<_> = scan.multiplexes().collect();
    assert_eq!(found.len(), 2, "{:#?}", scan.results);
    let (ra, rb) = (found[0], found[1]);
    assert_eq!((ra.channel.as_str(), ra.frequency_hz), ("9A", 205.5e6));
    assert_eq!((rb.channel.as_str(), rb.frequency_hz), ("30", 543.5e6));
    for r in [ra, rb] {
        assert_eq!(
            mode(r),
            (
                Some(Mode::K2),
                Some(Guard::G8),
                Some(Constellation::Qam16),
                Some(CodeRate::R23)
            )
        );
        assert_eq!(r.network_name, "Test Network");
        assert_eq!(r.network_id, 0x3201);
        assert_eq!(r.bandwidth_mhz, 7);
        assert!(r.snr_db > 18.0, "{r:?}");
        assert!(r.rs_good > 0.9, "{r:?}");
        assert!(r.occupancy_db >= 5.0, "{r:?}");
    }
    assert_eq!(
        names(&ra.services),
        ["Alpha One [TV]", "Alpha HD [TV]", "Alpha Radio [Radio]"]
    );
    assert_eq!(names(&rb.services), ["Bravo [TV]", "Bravo Kids [TV]"]);
    // Nothing of one multiplex leaks into the next channel's result.
    for r in scan.results.iter().filter(|r| !r.locked) {
        assert!(r.services.is_empty() && r.network_name.is_empty(), "{r:?}");
        assert_eq!(r.note, Some(ScanNote::Empty), "{r:?}");
    }
    assert_eq!(scan.services().len(), 5);
}
