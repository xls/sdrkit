//! The recorded vectors of `tests/vectors/science` against `expected.json`. The recordings come from an independent generator.

use sdrkit_core::{Event, InputInfo, Output, Owner, Receiver, Results, StackConfig};
use sdrkit_dev_test::{sigmf, Harness};
use sdrkit_graph::{GraphError, StackGraph};
use sdrkit_stack_science as sci;

/// Runs a recording through a receiver tuned `tuned_off` Hz from its centre, with the settings `extra`.
fn run(
    make: fn(&StackConfig) -> Result<StackGraph, GraphError>,
    desc: &'static sdrkit_core::StackDescriptor,
    name: &str,
    extra: fn(Harness) -> Harness,
    tuned_off: f64,
) -> (StackGraph, Vec<Event>) {
    let rec = sdrkit_dev_test::read_recording(&format!("science/{name}"))
        .expect("the recording is in the recordings folder");
    let h = extra(Harness::new(desc));
    let mut rx = make(&StackConfig::new(h.values().unwrap())).unwrap();
    rx.configure(
        &InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz + tuned_off).recording(),
    )
    .unwrap();
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in rec.samples.chunks(4096) {
        rx.feed(c, &mut out);
        events.append(&mut out.events);
        out.clear();
    }
    rx.refresh_results();
    (rx, events)
}

/// The results of a receiver.
fn results(rx: &StackGraph) -> &Results {
    Owner::results(rx)
}

/// Every event is one that the declaration of the stack names.
fn declared(decl: &sdrkit_core::ResultsDecl, events: &[Event]) {
    for e in events {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "{}",
            e.kind()
        );
    }
}

#[test]
fn sonde_rs41_250k() {
    let exp = sigmf::read_expected("science").unwrap();
    let e = &exp["sonde_rs41_250k"]["sondes"][0];
    let (rx, events) = run(
        sci::sonde::receiver,
        &sci::SONDE,
        "sonde_rs41_250k",
        |h| h,
        0.0,
    );
    let t = rx
        .graph()
        .component_as::<sci::sonde::table::SondeTable>("table")
        .unwrap();
    let r = t.store().sondes.iter_newest_first().next().unwrap();
    let s = r.record;
    assert_eq!(s.serial, e["serial"].as_str().unwrap());
    assert_eq!(s.kind, sci::sonde::frame::SondeType::Rs41);
    assert_eq!(s.subtype, e["subtype"].as_str().unwrap());
    assert!(s.good >= 5);
    assert!(s.bad <= 1);
    assert!((s.lat.unwrap() - 25.2).abs() < 0.0005);
    assert!((s.lon.unwrap() - 55.3605).abs() < 0.001);
    assert!((s.alt.unwrap() - 12025.0).abs() < 30.0);
    assert!((s.vel_v.unwrap() - 5.0).abs() < 0.1);
    assert!((s.vel_h.unwrap() - 10.0).abs() < 0.1);
    assert!((s.heading.unwrap() - 90.0).abs() < 1.0);
    assert_eq!(s.sats, Some(9));
    assert!((s.utc.unwrap() - 1780272005.0).abs() <= 1.0);
    assert!((s.battery_v.unwrap() - 2.9).abs() < 0.05);
    assert!((s.frame.unwrap() - 950).abs() <= 1);
    assert!(s.temp_c.is_none());
    let n = s.cal.unwrap().0;
    assert!(n == 5 || n == 6);
    assert!((s.freq_hz.unwrap() - 403e6).abs() < 800.0);
    assert_eq!(
        results(&rx).status().lock_state,
        sdrkit_core::LockState::Locked
    );
    declared(&sci::sonde::table::declaration(), &events);
}

#[test]
fn same_two_alerts_64k() {
    let exp = sigmf::read_expected("science").unwrap();
    let e = &exp["same_two_alerts_64k"];
    let (rx, events) = run(
        sci::same::receiver,
        &sci::SAME,
        "same_two_alerts_64k",
        |h| h,
        -16000.0,
    );
    use sci::same::MessageKind;
    use sci::typed::{SameAlertRow, SameFields, SAME_ALERT, SAME_ALERTS, SAME_TONES};
    let r = results(&rx);
    let table = |key: &str| r.table(r.decl().table_id(key).unwrap()).unwrap();
    // The rows newest first, so the oldest message last.
    let mut alerts: Vec<&sdrkit_core::Record> = table(SAME_ALERTS.key)
        .rows_newest_first()
        .map(|(_, row)| row)
        .collect();
    alerts.reverse();
    let msgs = e["messages"].as_array().unwrap();
    assert_eq!(alerts.len(), msgs.len());
    for (a, m) in alerts.iter().zip(msgs) {
        assert_eq!(a.at(&SameAlertRow::RAW), m["raw"].as_str());
        assert_ne!(a.at(&SameAlertRow::KIND), Some(MessageKind::Damaged));
        if m["eom"] == false {
            assert!(a.at(&SameAlertRow::REPEATS).unwrap() >= 2);
            assert!(a.at(&SameAlertRow::AGREEING).unwrap() >= 2);
            assert_eq!(a.at(&SameAlertRow::EVENT_NAME), m["event_name"].as_str());
            assert_eq!(
                a.at(&SameAlertRow::VALID_FOR),
                Some(m["purge_min"].as_f64().unwrap() * 60.0)
            );
            assert_eq!(a.at(&SameAlertRow::CALLSIGN), m["callsign"].as_str());
        }
    }
    let t = alerts[0].at(&SameAlertRow::TONE).unwrap();
    assert!((1.0..=2.1).contains(&t), "{t}");
    assert!(alerts[2].at(&SameAlertRow::TONE).is_none());
    assert_eq!(r.fields().at(&SameFields::MESSAGES_OK), Some(4));
    assert_eq!(r.fields().at(&SameFields::MESSAGES_BAD), Some(0));
    assert_eq!(table(SAME_TONES.key).len(), 1);
    let ce = r.status().offset_hz.unwrap();
    assert!((200.0..=400.0).contains(&ce), "{ce}");
    let heard = events.iter().filter(|e| e.is(&SAME_ALERT));
    assert_eq!(heard.count(), 2);
    declared(&sci::same::declaration(), &events);
}

#[test]
fn eczas_225k_48k() {
    let exp = sigmf::read_expected("science").unwrap();
    let e = &exp["eczas_225k_48k"];
    let (rx, _) = run(
        sci::eczas::receiver,
        &sci::ECZAS,
        "eczas_225k_48k",
        |h| {
            h.set(
                &sdrkit_stack_science::clock::CLOCK_MODE,
                sdrkit_stack_science::clock::ClockChoice::None,
            )
        },
        -12000.0,
    );
    let k = rx
        .graph()
        .component_as::<sci::eczas::Keeper>("keeper")
        .unwrap();
    let log: Vec<&sci::eczas::LogRow> = k.store().frames.iter().collect();
    let tp: Vec<&&sci::eczas::LogRow> = log
        .iter()
        .filter(|r| r.kind == sci::eczas::FrameKind::Time)
        .collect();
    let want = e["time_packets"].as_array().unwrap();
    assert!(tp.len() >= 3);
    for (g, w) in tp.iter().zip(want) {
        assert!(
            g.text
                .starts_with(&sci::calendar::iso(w["utc"].as_i64().unwrap())),
            "{} {}",
            g.text,
            w["utc"]
        );
        let hex: Vec<String> = g.bytes.iter().map(|b| format!("{b:02X}")).collect();
        assert_eq!(hex.join(" "), w["bytes"].as_str().unwrap());
        assert!(
            (g.signal_time - w["signal_time_s"].as_f64().unwrap()).abs() < 0.03,
            "{} {}",
            g.signal_time,
            w["signal_time_s"]
        );
    }
    use sci::eczas::FrameKind::{Bad, Disagrees, Other};
    assert_eq!(
        log.iter()
            .filter(|r| r.kind == Bad || r.kind == Disagrees)
            .count(),
        0
    );
    assert!(log.iter().filter(|r| r.kind == Other).count() >= 5);
    let r = results(&rx);
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    assert_eq!(r.fields().at(&sci::typed::EczasFields::CONFIRMED), Some(true));
    let off = r.status().offset_hz.unwrap();
    assert!((17.0..=23.0).contains(&off), "{off}");
    let snr = r.status().snr_db.unwrap();
    assert!((27.0..=33.0).contains(&snr), "{snr}");
    assert!(r.fields().at(&sci::typed::EczasFields::SYNCS).unwrap() >= 8);
}

#[test]
fn mscatter_beacon_24k() {
    let exp = sigmf::read_expected("science").unwrap();
    let e = &exp["mscatter_beacon_24k"];
    let (rx, _) = run(
        sci::mscatter::receiver,
        &sci::MSCATTER,
        "mscatter_beacon_24k",
        |h| h.set(&sdrkit_stack_science::mscatter::START_UTC, 1_780_272_000.0),
        0.0,
    );
    let t = rx
        .graph()
        .component_as::<sci::mscatter::PingTable>("counts")
        .unwrap();
    let pings: Vec<&sci::mscatter::Ping> = t.store().pings.iter().collect();
    let r = results(&rx);
    let b = r.status().offset_hz.unwrap();
    assert!((412.0..=428.0).contains(&b));
    let snr = r.status().snr_db.unwrap();
    assert!((48.8..=55.8).contains(&snr), "{snr}");
    let want = e["pings"].as_array().unwrap();
    assert_eq!(pings.len(), want.len());
    for (g, w) in pings.iter().zip(want) {
        let ds = g.start_s - w["start_s"].as_f64().unwrap();
        assert!((-0.35..=0.25).contains(&ds), "start {ds}");
        assert!((g.doppler_hz - w["doppler_hz"].as_f64().unwrap()).abs() <= 9.0);
        let dp = g.peak_snr_db - w["peak_snr_db"].as_f64().unwrap();
        assert!((-7.0..=4.0).contains(&dp), "peak {dp}");
        let wd = w["duration_s"].as_f64().unwrap();
        assert!(
            g.duration_s >= 0.6 * wd && g.duration_s <= 1.8 * wd + 0.4,
            "dur {} {wd}",
            g.duration_s
        );
        assert_eq!(g.overdense, w["kind"] == "overdense");
        assert!((g.utc - (1780272000.0 + w["start_s"].as_f64().unwrap())).abs() <= 1.0);
        assert!((1..=32).contains(&g.profile.len()));
    }
    assert_eq!(r.fields().at(&sci::typed::MscatterFields::REJECTED), Some(0));
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
}
