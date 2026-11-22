//! The `bletracker` stack: tracker recognition, the book on synthetic time, the receiver and the recording.

use super::help::{cnum, command, config, feed, impaired, num, rows, set, snr, text};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::{Choice, InputInfo, Owner, Receiver, SettingSet, Value};
use sdrkit_proto_wlan_phy::ble::ad;
use sdrkit_stack_wlan::ble::gen::{addr, tracker_schedule, BleGenParams, BleGenerator};
use sdrkit_stack_wlan::tracker::book::{
    recognise, Book, Recognised, Sighting, TrackerKind, TrackerState,
};
use sdrkit_stack_wlan::tracker::{self, TrackerSetting};
use sdrkit_stack_wlan::typed::{
    BletrackerAlertEvent as AlertEv, BletrackerTrackerRow as T, BLETRACKER_ALERT,
};

fn findmy(status: u8, len: usize) -> Vec<u8> {
    let mut m = vec![0x12, 0x19, status];
    m.extend(vec![0x55; 22]);
    m.extend([0x02, 0x00]);
    m.truncate(len);
    let mut d = Vec::new();
    ad::push_manufacturer(&mut d, 0x004C, &m).unwrap();
    d
}

fn service(uuid: u16, data: &[u8]) -> Vec<u8> {
    let mut d = Vec::new();
    ad::push_service_data16(&mut d, uuid, data).unwrap();
    d
}

fn uuids(u: &[u16]) -> Vec<u8> {
    let mut d = Vec::new();
    ad::push_uuids16(&mut d, u).unwrap();
    d
}

fn s(a: &str, level: f64, t: f64, what: Recognised) -> Sighting {
    Sighting {
        addr: addr(a, true),
        what,
        level_dbfs: level,
        time_s: t,
        wall: None,
    }
}

/// A packet every 0.5 s while `f(t)` returns a level, housekeeping every 0.25 s.
fn run(book: &mut Book, from: f64, to: f64, a: &str, f: impl Fn(f64) -> Option<f64>) {
    let fm = recognise(&findmy(0x10, 27)).unwrap();
    let mut t = from;
    while t <= to + 1e-9 {
        if (t * 2.0).fract().abs() < 1e-9 {
            if let Some(l) = f(t) {
                book.hear(&s(a, l, t, fm));
            }
        }
        book.housekeeping(t);
        t += 0.25;
    }
}

fn only(b: &Book) -> &sdrkit_stack_wlan::tracker::book::Tracker {
    assert_eq!(b.trackers.len(), 1);
    b.trackers.iter().next().unwrap().record
}

/// The registered SmartTag value printed in the published analysis of the first SmartTag.
const SMARTTAG_EXAMPLE: [u8; 20] = [
    0x15, 0x6B, 0xFA, 0x00, 0xC8, 0x40, 0x62, 0xB2, 0x8F, 0x00, 0xE2, 0x60, 0xC3, 0x00, 0x00, 0x00,
    0xAD, 0x01, 0x8B, 0x47,
];

#[test]
fn the_uuid_tells_the_mode_and_the_service_data_is_kept() {
    use sdrkit_stack_wlan::tracker::layout::{SmartTagState, TagMode};
    let mode = |d: &[u8]| recognise(d).unwrap().detail.mode;
    assert_eq!(mode(&uuids(&[0xFEED])), Some(TagMode::Activated));
    let t = recognise(&uuids(&[0xFEEC])).unwrap();
    assert_eq!(t.kind, TrackerKind::Tile);
    assert_eq!(t.detail.mode, Some(TagMode::Shipping));
    assert_eq!(mode(&uuids(&[0xFD84])), None);
    assert_eq!(mode(&uuids(&[0xFD84, 0xFEEC])), Some(TagMode::Shipping));
    assert_eq!(mode(&uuids(&[0xFE33])), Some(TagMode::Disconnected));
    assert_eq!(mode(&service(0xFD59, &[1, 2])), Some(TagMode::Unregistered));
    assert_eq!(
        recognise(&service(0xFD59, &[1])).unwrap().kind,
        TrackerKind::SmartTag
    );

    // A UUID list first, then the service data of the same family: both parts count.
    let mut d = uuids(&[0xFEED]);
    d.extend(service(0xFEED, &[0x02, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]));
    let t = recognise(&d).unwrap();
    assert_eq!(t.detail.mode, Some(TagMode::Activated));
    assert_eq!(
        t.detail.service_data.unwrap().as_slice(),
        &[0x02, 0x00, 1, 2, 3, 4, 5, 6, 7, 8]
    );
    assert!(t.detail.smarttag.is_none());

    // The printed SmartTag value.
    let t = recognise(&service(0xFD5A, &SMARTTAG_EXAMPLE)).unwrap();
    assert_eq!(t.detail.mode, Some(TagMode::Registered));
    let v = t.detail.smarttag.unwrap();
    assert_eq!(v.state(), Some(SmartTagState::ConnectedOne));
    assert_eq!(
        (v.aging_counter, v.region, v.e2e, v.uwb, v.battery),
        (64_107, 12, false, false, 3)
    );
    assert_eq!(
        v.privacy_id,
        [0xC8, 0x40, 0x62, 0xB2, 0x8F, 0x00, 0xE2, 0x60]
    );
    // Under the unregistered UUID the value is kept, not decoded.
    let t = recognise(&service(0xFD59, &SMARTTAG_EXAMPLE)).unwrap();
    assert!(t.detail.smarttag.is_none() && t.detail.service_data.is_some());

    // The book keeps the hex of the privacy ID and of the service data.
    let mut b = Book::default();
    let t = recognise(&service(0xFD5A, &SMARTTAG_EXAMPLE)).unwrap();
    b.hear(&s("D1:22:33:44:55:66", -50.0, 1.0, t));
    let r = only(&b);
    assert_eq!(r.identifier, "C84062B28F00E260");
    assert_eq!(r.service_data, "156BFA00C84062B28F00E260C3000000AD018B47");
    assert_eq!(r.mode, Some(TagMode::Registered));
}

#[test]
fn recognition_reads_only_the_public_structure() {
    let r = recognise(&findmy(0x24, 27)).unwrap();
    assert_eq!(r.kind, TrackerKind::FindMy);
    assert!(r.separated);
    assert_eq!(r.status, Some(0x24));
    let mut d = Vec::new();
    ad::push(&mut d, ad::FLAGS, &[6]).unwrap();
    d.extend(findmy(0x24, 27));
    assert_eq!(recognise(&d).unwrap().kind, TrackerKind::FindMy);
    assert!(recognise(&findmy(0x24, 12)).is_none());
    let mut nearby = Vec::new();
    ad::push_manufacturer(
        &mut nearby,
        0x004C,
        &[0x10, 0x05, 0x01, 0x18, 0x44, 0x22, 0x11],
    )
    .unwrap();
    assert!(recognise(&nearby).is_none());
    let mut m = vec![0x12, 0x19, 0x24];
    m.extend(vec![0x55; 24]);
    let mut other = Vec::new();
    ad::push_manufacturer(&mut other, 0x0059, &m).unwrap();
    assert!(recognise(&other).is_none());
    let t = recognise(&service(0xFEED, &[1, 2])).unwrap();
    assert_eq!(t.kind, TrackerKind::Tile);
    assert!(!t.separated);
    assert_eq!(
        recognise(&uuids(&[0x180F, 0xFD84])).unwrap().kind,
        TrackerKind::Tile
    );
    assert_eq!(
        recognise(&uuids(&[0xFE33])).unwrap().kind,
        TrackerKind::Chipolo
    );
    assert_eq!(
        recognise(&service(0xFD5A, &[1])).unwrap().kind,
        TrackerKind::SmartTag
    );
    let g = recognise(&service(0xFEAA, &[0x41, 1])).unwrap();
    assert!(g.kind == TrackerKind::Google && g.unwanted_protection);
    let g = recognise(&service(0xFEAA, &[0x40, 1])).unwrap();
    assert!(g.kind == TrackerKind::Google && !g.unwanted_protection);
    assert!(recognise(&service(0xFEAA, &[0x00, 1])).is_none());
    assert!(recognise(&[0x09, 0xFF, 0x4C, 0x00, 0x12]).is_none());
    assert!(recognise(&[]).is_none());
    assert!(TrackerKind::FindMy.full().contains("AirTag"));
}

#[test]
fn near_for_twelve_minutes_alerts_once_after_ten() {
    let mut b = Book::default();
    let fm = recognise(&findmy(0x10, 27)).unwrap();
    let mut alert_at = None;
    let mut t: f64 = 0.0;
    while t <= 720.0 {
        if (t * 2.0).fract().abs() < 1e-9 {
            b.hear(&s("AA:BB:CC:DD:EE:01", -45.0, t, fm));
        }
        b.housekeeping(t);
        if alert_at.is_none() && !b.alerts.is_empty() {
            alert_at = Some(t);
        }
        t += 0.25;
    }
    assert_eq!(b.alerts.len(), 1);
    assert!((600.0..601.0).contains(&alert_at.unwrap()));
    let r = only(&b);
    assert!(r.following && r.alerted);
    assert_eq!(b.following_now(), 1);
    assert!(b
        .alerts
        .iter()
        .next()
        .unwrap()
        .text
        .contains("Unwanted tracking"));
    assert!(r.label().contains("..EE:01") && !r.label().contains("AA:BB"));
    assert_eq!(b.now_by_kind()[0], 1);
    assert_eq!(b.window_by_kind()[0], 1);
}

#[test]
fn short_far_patchy_and_mine_do_not_alert() {
    let a = "AA:BB:CC:DD:EE:01";
    let mut b = Book::default();
    run(&mut b, 0.0, 540.0, a, |_| Some(-45.0));
    assert!(b.alerts.is_empty());
    let mut b = Book::default();
    run(&mut b, 0.0, 3600.0, a, |_| Some(-90.0));
    assert!(b.alerts.is_empty());
    assert_eq!(only(&b).proximity, 0);
    assert!(!only(&b).following);
    let mut b = Book::default();
    run(&mut b, 0.0, 840.0, a, |t| {
        (t <= 60.0 || t >= 780.0).then_some(-45.0)
    });
    assert!(b.alerts.is_empty());
    let mut b = Book::default();
    run(&mut b, 0.0, 30.0, a, |_| Some(-45.0));
    let id = only(&b).id;
    assert!(b.ignore(id, true));
    run(&mut b, 30.25, 900.0, a, |_| Some(-45.0));
    assert!(b.alerts.is_empty());
    assert_eq!(only(&b).state(), TrackerState::Mine);
    assert!(!only(&b).following);
}

#[test]
fn rotation_two_tags_and_kinds() {
    let mut b = Book::default();
    run(&mut b, 0.0, 400.0, "AA:BB:CC:DD:EE:10", |_| Some(-45.0));
    run(&mut b, 400.25, 800.0, "AA:BB:CC:DD:EE:11", |t| {
        (t >= 405.0).then_some(-46.0)
    });
    assert_eq!(only(&b).addresses, 2);
    assert_eq!(b.alerts.len(), 1);
    let mut b = Book::default();
    let fm = recognise(&findmy(0x10, 27)).unwrap();
    let mut t = 0.0;
    while t < 60.0 {
        b.hear(&s("AA:BB:CC:DD:EE:01", -45.0, t, fm));
        b.hear(&s("AA:BB:CC:DD:EE:02", -46.0, t + 0.1, fm));
        b.housekeeping(t + 0.25);
        t += 0.5;
    }
    assert_eq!(b.trackers.len(), 2);
    let mut b = Book::default();
    run(&mut b, 0.0, 100.0, "AA:BB:CC:DD:EE:01", |_| Some(-45.0));
    let tile = recognise(&uuids(&[0xFEED])).unwrap();
    let mut t = 105.0;
    while t < 200.0 {
        b.hear(&s("AA:BB:CC:DD:EE:02", -45.0, t, tile));
        b.housekeeping(t);
        t += 0.5;
    }
    assert_eq!(b.trackers.len(), 2);
}

#[test]
fn trend_forgetting_and_clear() {
    let a = "AA:BB:CC:DD:EE:01";
    let mut b = Book::default();
    run(&mut b, 0.0, 100.0, a, |t| Some(-80.0 + 0.4 * t));
    assert_eq!(only(&b).trend, 1);
    let mut b = Book::default();
    run(&mut b, 0.0, 20.0, a, |_| Some(-45.0));
    b.housekeeping(20.0 + 1800.0 - 1.0);
    assert_eq!(b.trackers.len(), 1);
    b.housekeeping(20.0 + 1800.0 + 1.0);
    assert_eq!(b.trackers.len(), 0);
    run(&mut b, 2000.0, 2010.0, a, |_| Some(-45.0));
    b.clear();
    assert!(b.trackers.is_empty() && b.alerts.is_empty());
}

fn rx(settings: &[(&str, Value)]) -> sdrkit_graph::StackGraph {
    tracker::receiver(&config(TrackerSetting::schema(), settings)).unwrap()
}

#[test]
fn trackers_live_through_the_receiver() {
    let gen = BleGenerator::new(BleGenParams {
        packets: tracker_schedule(),
        ..Default::default()
    });
    let x = impaired(Box::new(gen), &snr(30.0), 1, 4.0);
    let mut r = rx(&[
        ("alert_after_s", Value::F64(1.5)),
        ("near_dbfs", Value::F64(-100.0)),
    ]);
    r.configure(&InputInfo::new(4e6, 0.0, 0.0).recording())
        .unwrap();
    let fed = feed(&mut r, &x, 65536);
    let res = Owner::results(&r);
    let t = rows(res, "trackers");
    assert_eq!(t.len(), 5);
    let mut kinds: Vec<String> = t
        .iter()
        .map(|c| text(c, &T::TRACKER))
        .map(|l| l[..l.find(" ..").unwrap()].to_string())
        .collect();
    kinds.sort();
    assert_eq!(
        kinds,
        vec!["Chipolo", "Find My", "Google", "SmartTag", "Tile"]
    );
    assert!(t
        .iter()
        .all(|c| c.at(&T::FOLLOWING) == Some(true) && text(c, &T::TRACKER).len() < 24));
    assert_eq!(rows(res, "alerts").len(), 5);
    let fm = t
        .iter()
        .find(|c| text(c, &T::TRACKER) == "Find My ..31:5A")
        .unwrap();
    assert_eq!(fm.at(&T::SEPARATED), Some(true));
    assert_eq!(fm.at(&T::STATUS), Some(0x10));
    let ev: Vec<_> = fed
        .events
        .iter()
        .filter(|e| e.is(&BLETRACKER_ALERT))
        .collect();
    assert_eq!(ev.len(), 5);
    for e in &ev {
        assert!(e.freq_hz > 0.0);
        assert!(e.at(&AlertEv::URGENCY).is_some());
        let s = e.at(&AlertEv::TEXT).unwrap();
        assert!(s
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w == "tracker"));
        // No address ever leaves the receiver.
        assert!(!serde_json::to_string(&e.fields).unwrap().contains("E9:2E"));
    }
    assert!(r.save_state().is_none(), "identifiers are never saved");
    // Mark one as mine, clear, and 0.5 s more: fewer than five alerts.
    let id = cnum(t[0], &T::ID) as u32;
    command(&mut r, "mine", &[("row", Value::Text(id.to_string()))]).unwrap();
    command(&mut r, "clear", &[]).unwrap();
    let gen = BleGenerator::new(BleGenParams {
        packets: tracker_schedule(),
        ..Default::default()
    });
    let y = impaired(Box::new(gen), &snr(30.0), 9, 0.5);
    feed(&mut r, &y, 65536);
    assert!(rows(Owner::results(&r), "alerts").len() < 5);
}

#[test]
fn settings_are_strict() {
    let mut r = rx(&[]);
    assert!(set(&mut r, "alert_after_min", Value::F64(0.5)).is_err());
    assert!(set(&mut r, "near_dbfs", Value::F64(-10.0)).is_err());
    assert!(command(&mut r, "mine", &[("row", Value::Text("abc".into()))]).is_err());
    assert!(set(&mut r, "hop", Value::Bool(true)).is_err());
    set(&mut r, "alert_after_min", Value::F64(5.0)).unwrap();
}

#[test]
fn the_tracker_recording_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("wlan/ble_trackers_4m")
        .expect("the recording wlan/ble_trackers_4m");
    let exp = sdrkit_dev_test::sigmf::read_expected("wlan").unwrap();
    let e = &exp["ble_trackers_4m"];
    let mut r = rx(&[]);
    r.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz - 1e6).recording())
        .unwrap();
    feed(&mut r, &rec.samples, 16384);
    let res = Owner::results(&r);
    let ok = num(res, "frames_ok").unwrap();
    assert!((46.0..=52.0).contains(&ok), "{ok}");
    let tp = num(res, "tracker_packets").unwrap();
    assert!((39.0..=44.0).contains(&tp), "{tp}");
    assert_eq!(num(res, "frames_bad"), Some(0.0));
    assert_eq!(num(res, "following_now"), Some(0.0));
    assert!(rows(res, "alerts").is_empty());
    assert_eq!(res.status().summary, e["summary"].as_str().unwrap());
    assert_eq!(num(res, "trackers_now"), Some(5.0));
    let want = e["trackers"].as_array().unwrap();
    let got = rows(res, "trackers");
    assert_eq!(got.len(), want.len());
    for w in want {
        let label = w["label"].as_str().unwrap();
        let t = got
            .iter()
            .find(|c| text(c, &T::TRACKER) == label)
            .unwrap_or_else(|| panic!("{label}"));
        let kind = t.at(&T::KIND).unwrap();
        assert_eq!(
            TrackerKind::CHOICES[usize::from(kind.index())].label,
            w["kind"].as_str().unwrap()
        );
        assert!(
            (cnum(t, &T::LEVEL_DBFS) - w["level_dbfs"].as_f64().unwrap()).abs() <= 1.5,
            "{label}"
        );
        assert!(
            (cnum(t, &T::PACKETS) - w["packets"].as_f64().unwrap()).abs() <= 1.0,
            "{label}"
        );
        assert_eq!(cnum(t, &T::ADDRESSES), w["addresses"].as_f64().unwrap());
        if let Some(s) = w.get("separated") {
            assert_eq!(t.at(&T::SEPARATED), Some(s.as_bool().unwrap()));
        }
    }
}

#[test]
fn every_verify_case_passes_on_the_test_signal() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 1.0,
        clean: false,
        snr_db: 25.0,
    };
    for d in sdrkit_stack_wlan::descriptors() {
        let hooks = d.verify.unwrap();
        for c in (hooks.cases)(&o) {
            let mut h = sdrkit_dev_test::Harness::new(d).seconds(c.seconds + 0.3);
            if let Some(s) = c.snr_db {
                h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, s);
            }
            let run = h.run().unwrap();
            for line in (c.check.unwrap())(&run.results) {
                assert_eq!(
                    line.outcome,
                    sdrkit_core::verify::Outcome::Pass,
                    "{} {}: {:?}",
                    d.id,
                    c.name,
                    line
                );
            }
        }
    }
}
