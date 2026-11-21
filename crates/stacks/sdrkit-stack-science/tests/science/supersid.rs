//! SuperSID: the judge on long level series, the receiver on IQ at 12 and 250 ksps, the span rule, pieces of any size,
//! clear and reset, and the data file in memory.

use sci::supersid::gen::{Carrier, SidGenerator};
use sci::supersid::judge::{Judge, SidEvent};
use sci::supersid::list;
use sci::supersid::log::{LogFormat, Logged, SidLog, Site, CONTACT};
use sci::supersid::meter::{self, Meter, PRESENT_DB};
use sci::supersid::{self as sid, TRANSMITTERS};
use sci::typed::{SupersidSidEvent, SupersidTransmitterRow, SID_EVENT};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::{
    owner, Complex32, Facilities, Generator, InputInfo, MemStorage, Output, Receiver, Results,
    Storage, StorePath,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;
use std::sync::Arc;

const TX_CSV: &str = include_str!("../../../../../data/vlf/supersid_transmitters.csv");

/// The starts and ends of the events of a judge fed `levels` (one per 5 s from 0).
fn judge_all(levels: impl Iterator<Item = f64>) -> (Vec<SidEvent>, Vec<SidEvent>, Judge) {
    let mut j = Judge::new(5.0, 1.5);
    let (mut starts, mut ends) = (Vec::new(), Vec::new());
    for (i, l) in levels.enumerate() {
        let v = j.push(i as f64 * 5.0, l);
        starts.extend(v.started);
        ends.extend(v.ended);
    }
    (starts, ends, j)
}

#[test]
fn three_hours_of_levels_give_one_event_on_the_stepped_transmitter() {
    let n = 3 * 720;
    let mut rng = Rng::new(7);
    let a = (0..n).map(|i| {
        let t = i as f64 * 5.0;
        let step = if t >= 7200.0 {
            3.0 * (-(t - 7200.0) / 1500.0).exp()
        } else {
            0.0
        };
        25.0 + rng.range(-0.15, 0.15) + step
    });
    let (starts, ends, j) = judge_all(a.collect::<Vec<_>>().into_iter());
    assert_eq!(starts.len(), 1, "{starts:?}");
    let s = starts[0].start_s;
    assert!((7200.0..=7500.0).contains(&s), "{s}");
    assert_eq!(ends.len(), 1);
    assert!(ends[0].end_s.unwrap() <= 10_800.0);
    assert!(j.active().is_none());
    assert!(j.minutes().len() >= 170, "{}", j.minutes().len());

    let (b, _, _) = judge_all((0..n).map(|_| 18.0));
    assert!(b.is_empty());
    let c = (0..n).map(|i| 14.0 + 0.5 * (i as f64 * 5.0 / 3000.0).sin());
    let (c, _, _) = judge_all(c);
    assert!(c.is_empty());
}

#[test]
fn a_slow_fall_of_4_db_gives_no_event() {
    let fall = (0..2 * 720).map(|i| {
        let t = i as f64 * 5.0;
        20.0 - 4.0 * ((t - 1800.0) / 5400.0).clamp(0.0, 1.0)
    });
    let (s, _, _) = judge_all(fall);
    assert!(s.is_empty(), "{s:?}");
}

#[test]
fn level_noise_gives_no_event_and_a_weak_transmitter_is_never_present() {
    let mut rng = Rng::new(11);
    let noisy: Vec<f64> = (0..3 * 720).map(|_| 22.0 + rng.range(-0.5, 0.5)).collect();
    let (s, _, _) = judge_all(noisy.into_iter());
    assert!(s.is_empty(), "{s:?}");
    let weak: Vec<f64> = (0..3 * 720).map(|_| 2.0 + rng.range(-3.0, 3.0)).collect();
    assert!(weak.iter().all(|l| *l < PRESENT_DB));
    let (s, _, _) = judge_all(weak.into_iter());
    assert!(s.is_empty());
}

/// The number in column `f` of row `row` of the transmitter table.
fn cell(r: &Results, row: &str, f: &Column<field::F64>) -> Option<f64> {
    let t = r.table(r.decl().table_id("transmitters")?)?;
    t.row(row)?.at(f)
}

/// The yes or no in column `f` of row `row` of the transmitter table.
fn flag(r: &Results, row: &str, f: &Column<field::Bool>) -> Option<bool> {
    let t = r.table(r.decl().table_id("transmitters")?)?;
    t.row(row)?.at(f)
}

#[test]
fn iq_at_12_ksps_finds_the_step_through_the_receiver() {
    let rate = 12e3;
    let h = Harness::new(&sid::DESCRIPTOR)
        .rate(rate)
        .freq(22e3)
        .set(&TRANSMITTERS, "NAA,24000;NPM,21400;NLK,24800");
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, 22e3, 22e3).recording())
        .unwrap();
    let carriers = vec![
        Carrier {
            offset_hz: 2000.0,
            amplitude: 10.0,
            step: Some((18.0 * 60.0, 2f64.sqrt())),
        },
        Carrier {
            offset_hz: -600.0,
            amplitude: 10f64.powf(12.0 / 20.0),
            step: None,
        },
    ];
    let mut g = SidGenerator::new(rate, carriers, 1.0, 3);
    let mut x = vec![Complex32::default(); rate as usize];
    let mut out = Output::default();
    let mut events = Vec::new();
    for _ in 0..25 * 60 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
        events.append(&mut out.events);
    }
    out.clear();
    rx.finish(&mut out);
    events.append(&mut out.events);
    let r = rx.results();
    assert_eq!(flag(r, "01", &SupersidTransmitterRow::PRESENT), Some(true));
    assert_eq!(flag(r, "02", &SupersidTransmitterRow::PRESENT), Some(true));
    assert_eq!(flag(r, "03", &SupersidTransmitterRow::IN_SPAN), Some(true));
    assert_eq!(flag(r, "03", &SupersidTransmitterRow::PRESENT), Some(false));
    let level = &SupersidTransmitterRow::LEVEL_DB;
    let d = cell(r, "01", level).unwrap() - cell(r, "02", level).unwrap();
    assert!((d - 11.0).abs() <= 1.5, "{d}");
    let sids: Vec<_> = events.iter().filter(|e| e.is(&SID_EVENT)).collect();
    assert!(!sids.is_empty());
    assert!(sids.iter().all(|e| e.at(&SupersidSidEvent::LABEL) == Some("NAA")));
}

#[test]
fn the_span_holds_some_transmitters_at_12_ksps_and_none_at_2_4_msps() {
    let text = list::setting_text_from_csv(TX_CSV).unwrap();
    let l = list::parse(&text).unwrap();
    assert_eq!(l.len(), 17);
    let n = l
        .iter()
        .filter(|t| meter::in_span(t.frequency_hz - 22e3, 12e3))
        .count();
    assert!((6..17).contains(&n), "{n}");
    let m = Meter::plain(2.4e6, 5.0);
    assert_eq!(m.fft_size(), None);
    assert!(l.iter().all(|t| !meter::in_span(t.frequency_hz - 22e3, 2.4e6)));
    let spectrum = meter::Spectrum {
        rate: 2.4e6,
        ..Default::default()
    };
    assert!(!spectrum.level(1000.0).in_span);
}

fn levels_of(r: &Results) -> Vec<f64> {
    (1..=4)
        .map(|i| cell(r, &format!("{i:02}"), &SupersidTransmitterRow::LEVEL_DB).unwrap_or(f64::NAN))
        .collect()
}

#[test]
fn pieces_of_any_size_give_the_same_levels() {
    let h = Harness::new(&sid::DESCRIPTOR)
        .seconds(11.0)
        .set(&TRANSMITTERS, sid::test_transmitters(sid::TEST_DIAL_HZ).as_str());
    let x = h.generate().unwrap();
    let a = h.clone().piece(997).feed(&x).unwrap();
    let b = h.clone().piece(65_536).feed(&x).unwrap();
    let (la, lb) = (levels_of(&a.results), levels_of(&b.results));
    for (u, v) in la.iter().zip(&lb) {
        assert!(u.is_finite() && (u - v).abs() < 1e-3, "{la:?} {lb:?}");
        assert!(*u > 10.0, "{la:?}");
    }
}

fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    owner::invoke(rx, id, &args).unwrap();
}

fn rows(r: &Results, table: &str) -> usize {
    r.table(r.decl().table_id(table).unwrap())
        .map_or(0, |t| t.len())
}

#[test]
fn clear_and_reset_keep_the_settings() {
    // 18 minutes of four strong carriers, the first 3 dB up from 16 minutes: an event at about 17 minutes.
    let rate = 12e3;
    let h = Harness::new(&sid::DESCRIPTOR)
        .rate(rate)
        .set(&TRANSMITTERS, sid::test_transmitters(sid::TEST_DIAL_HZ).as_str());
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, sid::TEST_DIAL_HZ, sid::TEST_DIAL_HZ).recording())
        .unwrap();
    let carriers = sci::supersid::gen::TEST_CARRIERS
        .iter()
        .enumerate()
        .map(|(i, &(offset_hz, _))| Carrier {
            offset_hz,
            amplitude: 1.0,
            step: (i == 0).then_some((16.0 * 60.0, 2f64.sqrt())),
        })
        .collect();
    let mut g = SidGenerator::new(rate, carriers, 1.0, 5);
    let mut x = vec![Complex32::default(); rate as usize];
    let mut out = Output::default();
    for _ in 0..18 * 60 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    let get = |rx: &dyn Receiver, k: &str| sci::family::field(rx.results(), k);
    assert!(get(rx.as_ref(), "events_total") >= 1.0);
    assert_eq!(rows(rx.results(), "sid_events") as f64, get(rx.as_ref(), "events_total"));
    assert!(cell(rx.results(), "01", &SupersidTransmitterRow::BASELINE_DB).is_some());
    command(rx.as_mut(), "clear");
    out.clear();
    rx.feed(&x[..1000], &mut out);
    assert_eq!(get(rx.as_ref(), "events_total"), 0.0);
    assert_eq!(rows(rx.results(), "sid_events"), 0);
    assert_eq!(get(rx.as_ref(), "listed_n"), 4.0);
    assert!(cell(rx.results(), "01", &SupersidTransmitterRow::BASELINE_DB).is_some());
    command(rx.as_mut(), "reset");
    for _ in 0..12 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    assert_eq!(get(rx.as_ref(), "listed_n"), 4.0);
    assert_eq!(get(rx.as_ref(), "present_n"), 4.0);
    assert!(cell(rx.results(), "01", &SupersidTransmitterRow::BASELINE_DB).is_none());
    assert_eq!(get(rx.as_ref(), "events_total"), 0.0);
}

fn site() -> Site {
    Site {
        site: "Home".into(),
        monitor_id: "M1".into(),
        longitude: Some(151.2),
        latitude: Some(-33.86),
        utc_offset_s: 36_000,
        time_zone: "Australia/Sydney".into(),
        interval_s: 5.0,
    }
}

fn logged() -> Vec<Logged> {
    vec![
        Logged {
            label: "NWC".into(),
            frequency_hz: 19_800.0,
        },
        Logged {
            label: "NPM".into(),
            frequency_hz: 21_400.0,
        },
    ]
}

fn text(s: &MemStorage, path: &str) -> String {
    String::from_utf8(s.read(&StorePath::new(path).unwrap()).unwrap()).unwrap()
}

const HEADER_TOP: [&str; 12] = [
    "# Site = Home",
    "",
    "# Longitude = 151.2",
    "# Latitude = -33.86",
    "#",
    "#",
    "# UTC_Offset = +10:00",
    "# TimeZone = Australia/Sydney",
    "# UTC_StartTime = 2026-01-01 00:00:00",
    "# LogInterval = 5",
    "# LogType = raw",
    "# MonitorID = M1",
];

fn check_header(lines: &[&str]) {
    for (i, want) in HEADER_TOP.iter().enumerate() {
        if i == 1 {
            assert_eq!(lines[1], format!("# Contact = {CONTACT}"));
        } else {
            assert_eq!(lines[i], *want, "line {i}");
        }
    }
}

#[test]
fn sid_format_writes_one_file_per_transmitter() {
    let s = MemStorage::new();
    let folder = StorePath::new("supersid").unwrap();
    let mut log = SidLog::new(LogFormat::Sid, folder, site(), logged());
    // 2026-01-01 13:00:00 and 13:00:05 UTC.
    let t = 1_767_225_600.0 + 13.0 * 3600.0;
    log.write(&s, t, &[Some(0.25), None]).unwrap();
    log.write(&s, t + 5.0, &[Some(1e-6), Some(0.5)]).unwrap();
    let a = text(&s, "supersid/Home_NWC_2026-01-01.csv");
    let lines: Vec<&str> = a.lines().collect();
    check_header(&lines);
    assert_eq!(lines[12], "# StationID = NWC");
    assert_eq!(lines[13], "# Frequency = 19800");
    assert_eq!(lines[14], "2026-01-01 13:00:00, 0.250000000000000");
    assert_eq!(lines[15], "2026-01-01 13:00:05, 0.000001000000000");
    assert_eq!(lines.len(), 16);
    let b = text(&s, "supersid/Home_NPM_2026-01-01.csv");
    let lines: Vec<&str> = b.lines().collect();
    assert_eq!(lines[12], "# StationID = NPM");
    assert_eq!(lines[14], "2026-01-01 13:00:00, 0.000000000000000");
    assert_eq!(lines[15], "2026-01-01 13:00:05, 0.500000000000000");
}

#[test]
fn supersid_format_started_during_the_day_is_padded_from_midnight() {
    let s = MemStorage::new();
    let folder = StorePath::new("supersid").unwrap();
    let mut log = SidLog::new(LogFormat::Supersid, folder.clone(), site(), logged());
    let day = 1_767_225_600.0;
    // The first reading at 00:00:15: rows 0 to 2 are padding, row 3 the reading.
    log.write(&s, day + 15.0, &[Some(0.25), Some(0.5)]).unwrap();
    log.write(&s, day + 20.0, &[None, Some(0.125)]).unwrap();
    let a = text(&s, "supersid/Home_2026-01-01.csv");
    let lines: Vec<&str> = a.lines().collect();
    check_header(&lines);
    assert_eq!(lines[12], "# Stations = NWC, NPM");
    assert_eq!(lines[13], "# Frequencies = 19800, 21400");
    let zero = "0.000000000000000, 0.000000000000000";
    assert_eq!(&lines[14..17], &[zero, zero, zero]);
    assert_eq!(lines[17], "0.250000000000000, 0.500000000000000");
    assert_eq!(lines[18], "0.000000000000000, 0.125000000000000");
    assert_eq!(lines.len(), 19);

    // A second run of the same day continues the file at the present row.
    let mut again = SidLog::new(LogFormat::Supersid, folder.clone(), site(), logged());
    again.write(&s, day + 35.0, &[Some(1.0), Some(1.0)]).unwrap();
    let a = text(&s, "supersid/Home_2026-01-01.csv");
    let lines: Vec<&str> = a.lines().collect();
    assert_eq!(lines.len(), 22);
    assert_eq!(lines[19], zero);
    assert_eq!(lines[20], zero);
    assert_eq!(lines[21], "1.000000000000000, 1.000000000000000");

    // Other stations: a new file beside it.
    let mut other = SidLog::new(LogFormat::Supersid, folder, site(), logged()[..1].to_vec());
    other.write(&s, day + 40.0, &[Some(1.0)]).unwrap();
    let b = text(&s, "supersid/Home_2026-01-01_2.csv");
    assert_eq!(b.lines().nth(12), Some("# Stations = NWC"));
    assert_eq!(b.lines().count(), 14 + 9);

    // Another log interval: a new file beside them.
    let mut slow_site = site();
    slow_site.interval_s = 10.0;
    let mut slow = SidLog::new(LogFormat::Supersid, StorePath::new("supersid").unwrap(), slow_site, logged());
    slow.write(&s, day + 40.0, &[Some(1.0), Some(1.0)]).unwrap();
    let d = text(&s, "supersid/Home_2026-01-01_3.csv");
    assert_eq!(d.lines().nth(9), Some("# LogInterval = 10"));
    assert_eq!(d.lines().count(), 14 + 5);

    // The next UTC day opens a new file at row 0.
    log.write(&s, day + 86_400.0, &[Some(0.5), Some(0.5)]).unwrap();
    let c = text(&s, "supersid/Home_2026-01-02.csv");
    assert_eq!(c.lines().nth(8), Some("# UTC_StartTime = 2026-01-02 00:00:00"));
    assert_eq!(c.lines().nth(14), Some("0.500000000000000, 0.500000000000000"));
}

#[test]
fn the_receiver_writes_the_file_through_the_host_storage() {
    let storage = Arc::new(MemStorage::new());
    let h = Harness::new(&sid::DESCRIPTOR)
        .rate(48e3)
        .set(&TRANSMITTERS, sid::test_transmitters(sid::TEST_DIAL_HZ).as_str())
        .set(&sid::LOG_FORMAT, LogFormat::Supersid)
        .set(&sid::START_UTC, 1_767_225_600.0);
    let mut rx = h.receiver().unwrap();
    rx.set_facilities(&Facilities {
        storage: storage.clone(),
        ..Facilities::default()
    });
    rx.configure(&InputInfo::new(48e3, sid::TEST_DIAL_HZ, sid::TEST_DIAL_HZ).recording())
        .unwrap();
    let mut g = SidGenerator::test_signal(48e3, false, 1);
    let mut x = vec![Complex32::default(); 48_000];
    let mut out = Output::default();
    for _ in 0..11 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    let f = text(&storage, "supersid/supersid_2026-01-01.csv");
    let lines: Vec<&str> = f.lines().collect();
    assert_eq!(lines[12], "# Stations = A, B, C, D");
    // Two intervals of 5 s: rows 0 and 1, every value a reading.
    assert_eq!(lines.len(), 16, "{f}");
    for l in &lines[14..] {
        let v: Vec<f64> = l.split(", ").map(|c| c.parse().unwrap()).collect();
        assert_eq!(v.len(), 4);
        assert!(v.iter().all(|p| *p > 0.0), "{l}");
        // The first carrier, amplitude 0.5: a power near 0.25 of full scale.
        assert!((v[0] / 0.25 - 1.0).abs() < 0.1, "{l}");
    }
}

#[test]
fn the_verify_case_passes_and_the_event_is_declared() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 5.0,
        clean: false,
        snr_db: 20.0,
    };
    for case in (sid::VERIFY.cases)(&o) {
        let r = Harness::new(&sid::DESCRIPTOR)
            .seconds(case.seconds)
            .piece(16_384)
            .with_settings(&case.settings)
            .run()
            .unwrap();
        for line in (case.check.unwrap())(&r.results) {
            assert_eq!(line.outcome, sdrkit_core::verify::Outcome::Pass, "{line:?}");
        }
        assert_eq!(
            r.results.status().lock_state,
            sdrkit_core::LockState::Tracking
        );
    }
    assert!(sid::declaration()
        .events()
        .iter()
        .any(|d| d.kind == SID_EVENT.kind));
}

#[test]
fn the_mix_front_end_puts_each_carrier_on_its_transmitter() {
    // 48 ksps centred on 22 kHz, the dial at 23 kHz: the carrier at 24 kHz is 1 kHz above the dial.
    let rate = 48e3;
    let h = Harness::new(&sid::DESCRIPTOR)
        .rate(rate)
        .set(&TRANSMITTERS, "A,24000;B,25000");
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, 22e3, 23e3).recording())
        .unwrap();
    let carriers = vec![Carrier {
        offset_hz: 2000.0,
        amplitude: 0.5,
        step: None,
    }];
    let mut g = SidGenerator::new(rate, carriers, 1.0, 2);
    let mut x = vec![Complex32::default(); 48_000];
    let mut out = Output::default();
    for _ in 0..6 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    assert_eq!(flag(rx.results(), "01", &SupersidTransmitterRow::PRESENT), Some(true));
    assert_eq!(flag(rx.results(), "02", &SupersidTransmitterRow::PRESENT), Some(false));
}

#[test]
fn marks_and_the_sunlit_share_come_from_the_station_and_the_list() {
    // 2026-03-20 12:00 UTC; the receiver at Berlin, a transmitter in north-west England.
    let rate = 48e3;
    let station = sdrkit_core::Station {
        position: Some(sdrkit_util_geo::Position::new(52.5, 13.4)),
        ..sdrkit_core::Station::default()
    };
    let h = Harness::new(&sid::DESCRIPTOR)
        .rate(rate)
        .station(station)
        .set(&TRANSMITTERS, "A,24000,54.91,-3.28;B,24800")
        .set(&sid::START_UTC, 1_773_964_800.0 + 12.0 * 3600.0);
    let x = SidGenerator::test_signal(rate, false, 1);
    let mut g = x;
    let mut buf = vec![Complex32::default(); 48_000 * 6];
    g.generate(&mut buf);
    let r = h.feed(&buf).unwrap();
    let marks = r.results.table(r.results.decl().table_id("marks").unwrap()).unwrap();
    assert_eq!(marks.len(), 12);
    assert_eq!(cell(&r.results, "01", &SupersidTransmitterRow::SUNLIT), Some(1.0));
    assert_eq!(cell(&r.results, "02", &SupersidTransmitterRow::SUNLIT), None);
}
