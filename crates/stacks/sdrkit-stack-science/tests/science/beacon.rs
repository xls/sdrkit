//! Beacon openings through the receiver: a steady, a Doppler-spread and a fluttering opening with their classes, noise alone,
//! the default test signal and pieces of any size.

use sci::beacon::gen::{BeaconGenerator, GenParams};
use sci::beacon::DESCRIPTOR;
use sci::typed::{BeaconEventsFields, ScienceEvent, SCIENCE_EVENT, SCIENCE_EVENTS};
use sdrkit_core::{Complex32, Generator, InputInfo, Output};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;

#[derive(Clone, Debug, PartialEq)]
struct Ev {
    t: f64,
    kind: String,
    shift: f64,
    peak: f64,
    dur: f64,
    width: f64,
    flutter: f64,
    class: String,
}

fn events(r: &Run) -> Vec<Ev> {
    r.events
        .iter()
        .filter(|e| e.is(&SCIENCE_EVENT))
        .map(|e| Ev {
            t: e.time,
            kind: e.at(&ScienceEvent::TYPE).unwrap_or("").to_string(),
            shift: e.at(&ScienceEvent::FREQUENCY_HZ).unwrap_or(0.0),
            peak: e.at(&ScienceEvent::PEAK_DB).unwrap_or(0.0),
            dur: e.at(&ScienceEvent::DURATION_S).unwrap_or(0.0),
            width: e.at(&ScienceEvent::AUX1).unwrap_or(0.0),
            flutter: e.at(&ScienceEvent::AUX2).unwrap_or(0.0),
            class: e.at(&ScienceEvent::TEXT).unwrap_or("").to_string(),
        })
        .collect()
}

/// 90 s at 8 ksps: an opening at 20 s for 30 s, at +60 Hz drifting 8 Hz over the run.
fn opening(level_db: f64, spread_hz: f64, flutter_db: f64, open: bool) -> Vec<Complex32> {
    let p = GenParams {
        openings: if open { vec![(20.0, 30.0)] } else { Vec::new() },
        cycle_s: None,
        offset_hz: 60.0,
        drift_hz_s: 8.0 / 90.0,
        level_db,
        spread_hz,
        flutter_db,
        ..GenParams::default()
    };
    let mut g = BeaconGenerator::new(8e3, p, 5);
    let mut x = vec![Complex32::default(); (8e3 * 90.0) as usize];
    g.generate(&mut x);
    x
}

#[test]
fn three_openings_have_their_classes() {
    for (level, spread, flutter, class) in [
        (20.0, 0.0, 0.0, "steady carrier (sporadic-E-like)"),
        (28.0, 150.0, 0.0, "Doppler-spread (aurora-like)"),
        (28.0, 0.0, 6.0, "fluttering carrier (TEP / scatter-like)"),
    ] {
        let r = Harness::new(&DESCRIPTOR).rate(8e3).feed(&opening(level, spread, flutter, true)).unwrap();
        let ev = events(&r);
        assert_eq!(ev.len(), 1, "{class}: {ev:?}");
        let e = &ev[0];
        assert_eq!(e.kind, "opening");
        assert_eq!(e.class, class, "{e:?}");
        assert!((e.t - 20.0).abs() <= 4.0, "{e:?}");
        assert!((e.dur - 30.0).abs() <= 8.0, "{e:?}");
        assert!((e.shift - 64.0).abs() <= 25.0, "{e:?}");
        assert!(e.peak > 10.0, "{e:?}");
        let _ = (e.width, e.flutter);
    }
}

#[test]
fn five_minutes_of_noise_give_no_opening() {
    let p = GenParams {
        openings: Vec::new(),
        cycle_s: None,
        ..GenParams::default()
    };
    let mut g = BeaconGenerator::new(8e3, p, 8);
    let mut rx = Harness::new(&DESCRIPTOR).rate(8e3).receiver().unwrap();
    rx.configure(&InputInfo::new(8e3, 50e6, 50e6).recording()).unwrap();
    let mut x = vec![Complex32::default(); 80_000];
    let mut out = Output::default();
    let mut n = 0;
    for _ in 0..30 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
        n += out.events.iter().filter(|e| e.is(&SCIENCE_EVENT)).count();
    }
    assert_eq!(n, 0);
    assert_eq!(rx.results().fields().at(&BeaconEventsFields::OPENINGS), Some(0));
    let r = rx.results();
    assert_eq!(r.table(r.decl().table_id(SCIENCE_EVENTS.key).unwrap()).map_or(0, |t| t.len()), 0);
}

#[test]
fn the_test_signal_gives_openings_in_75_s() {
    let r = Harness::new(&DESCRIPTOR).rate(250e3).seconds(75.0).run().unwrap();
    let ev = events(&r);
    assert!((1..=2).contains(&ev.len()), "{ev:?}");
    assert!((ev[0].t - 5.0).abs() <= 4.0, "{ev:?}");
    assert_eq!(ev[0].class, "Doppler-spread (aurora-like)");
}

#[test]
fn pieces_of_any_size_give_the_same_openings() {
    let x = opening(20.0, 0.0, 0.0, true);
    let runs: Vec<Vec<Ev>> = [555usize, 8192, 120_000]
        .iter()
        .map(|&n| events(&Harness::new(&DESCRIPTOR).rate(8e3).piece(n).feed(&x).unwrap()))
        .collect();
    assert_eq!(runs[0].len(), 1);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}
