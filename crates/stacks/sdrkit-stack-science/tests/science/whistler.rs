//! Whistlers and tweeks through the receiver: placed whistlers found with their dispersion, tweeks at their cut-off, noise and
//! sferics alone, 48 ksps input, pieces of any size, and the clear and reset commands.

use sci::typed::{ScienceEvent, ScienceEventRow, WhistlerFields, SCIENCE_EVENT, SCIENCE_EVENTS};
use sci::whistler::gen::{GenEvent, GenParams, WhistlerGenerator};
use sci::whistler::{DESCRIPTOR, THRESHOLD};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;

/// An event of a run: time, type, frequency and the first stack value (D of a whistler, the mode of a tweek).
#[derive(Clone, Debug, PartialEq)]
struct Ev {
    t: f64,
    kind: String,
    f: f64,
    aux1: f64,
}

fn events(r: &Run) -> Vec<Ev> {
    r.events
        .iter()
        .filter(|e| e.is(&SCIENCE_EVENT))
        .map(|e| Ev {
            t: e.time,
            kind: e.at(&ScienceEvent::TYPE).unwrap_or("").to_string(),
            f: e.at(&ScienceEvent::FREQUENCY_HZ).unwrap_or(0.0),
            aux1: e.at(&ScienceEvent::AUX1).unwrap_or(0.0),
        })
        .collect()
}

fn signal(rate: f64, seconds: f64, p: GenParams) -> Vec<Complex32> {
    let mut g = WhistlerGenerator::new(rate, p, 7);
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

fn placed(dispersion: f64, events: &[(f64, bool)]) -> GenParams {
    GenParams {
        dispersion,
        events: events
            .iter()
            .map(|&(t, whistler)| GenEvent { t, whistler })
            .collect(),
        cycle_s: None,
        ..GenParams::default()
    }
}

fn field(r: &Run, f: &sdrkit_core::interface::Column<sdrkit_core::publish::field::I64>) -> i64 {
    r.results.fields().at(f).unwrap_or(-1)
}

#[test]
fn whistlers_and_tweeks_of_45_s_at_24_ksps() {
    let p = placed(
        60.0,
        &[
            (8.0, true),
            (24.0, true),
            (5.0, false),
            (16.0, false),
            (33.0, false),
            (40.0, false),
        ],
    );
    let r = Harness::new(&DESCRIPTOR)
        .rate(24e3)
        .feed(&signal(24e3, 45.0, p))
        .unwrap();
    let ev = events(&r);
    let at4k = 60.0 / 4000f64.sqrt();
    for t0 in [8.0, 24.0] {
        let w = ev
            .iter()
            .find(|e| e.kind == "whistler" && (e.t - (t0 + at4k)).abs() <= 0.2);
        let w = w.unwrap_or_else(|| panic!("no whistler near {t0} s: {ev:?}"));
        assert!((w.aux1 - 60.0).abs() <= 12.0, "{w:?}");
    }
    let tweeks = [5.0, 16.0, 33.0, 40.0]
        .iter()
        .filter(|&&t0| {
            ev.iter().any(|e| {
                e.kind == "tweek" && (e.t - t0).abs() <= 0.08 && (1650.0..=2150.0).contains(&e.f)
            })
        })
        .count();
    assert!(tweeks >= 3, "{tweeks} tweeks: {ev:?}");
    assert!(ev.len() <= 7, "{ev:?}");
    assert_eq!(field(&r, &WhistlerFields::EVENTS_TOTAL), ev.len() as i64);
    assert!(field(&r, &WhistlerFields::SFERICS) >= 20, "{}", field(&r, &WhistlerFields::SFERICS));
}

#[test]
fn whistlers_of_35_and_110_s_sqrt_hz_are_measured() {
    for d in [35.0, 110.0] {
        let x = signal(24e3, 12.0, placed(d, &[(6.0, true)]));
        let r = Harness::new(&DESCRIPTOR).rate(24e3).feed(&x).unwrap();
        let ev = events(&r);
        let w: Vec<&Ev> = ev.iter().filter(|e| e.kind == "whistler").collect();
        assert_eq!(w.len(), 1, "D {d}: {ev:?}");
        assert!((w[0].aux1 - d).abs() <= 0.2 * d, "D {d}: {:?}", w[0]);
        assert!(
            (w[0].t - (6.0 + d / 4000f64.sqrt())).abs() <= 0.2,
            "D {d}: {:?}",
            w[0]
        );
    }
}

#[test]
fn noise_and_sferics_alone_give_no_event() {
    let x = signal(24e3, 30.0, placed(60.0, &[]));
    let r = Harness::new(&DESCRIPTOR).rate(24e3).feed(&x).unwrap();
    assert!(events(&r).is_empty(), "{:?}", events(&r));
    assert_eq!(field(&r, &WhistlerFields::WHISTLERS), 0);
    assert_eq!(field(&r, &WhistlerFields::TWEEKS), 0);
    assert!(field(&r, &WhistlerFields::SFERICS) >= 20, "{}", field(&r, &WhistlerFields::SFERICS));
}

#[test]
fn a_whistler_and_a_tweek_at_48_ksps() {
    let x = signal(48e3, 18.0, placed(60.0, &[(7.0, true), (15.0, false)]));
    let r = Harness::new(&DESCRIPTOR).rate(48e3).feed(&x).unwrap();
    let ev = events(&r);
    assert!(ev.iter().any(|e| e.kind == "whistler"), "{ev:?}");
    assert!(ev.iter().any(|e| e.kind == "tweek"), "{ev:?}");
}

#[test]
fn the_test_signal_gives_a_whistler_and_a_tweek_in_8_s() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(48e3)
        .seconds(8.0)
        .run()
        .unwrap();
    assert!(field(&r, &WhistlerFields::WHISTLERS) >= 1, "{}", r.results.status().summary);
    assert!(field(&r, &WhistlerFields::TWEEKS) >= 1, "{}", r.results.status().summary);
}

#[test]
fn pieces_of_any_size_give_the_same_events() {
    let x = signal(24e3, 12.0, GenParams::default());
    let runs: Vec<Vec<Ev>> = [97usize, 1000, 16_384]
        .iter()
        .map(|&n| {
            events(
                &Harness::new(&DESCRIPTOR)
                    .rate(24e3)
                    .piece(n)
                    .feed(&x)
                    .unwrap(),
            )
        })
        .collect();
    assert!(runs[0].len() >= 2, "{:?}", runs[0]);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}

/// A receiver fed `x` at 48 ksps with `start_utc` set.
fn fed(x: &[Complex32], start_utc: f64) -> Box<dyn Receiver> {
    let h = Harness::new(&DESCRIPTOR)
        .rate(48e3)
        .set(&sci::mscatter::START_UTC, start_utc);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(48e3, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(4096) {
        out.clear();
        rx.feed(c, &mut out);
    }
    rx
}

fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx, id, &args).unwrap();
}

fn rows(rx: &dyn Receiver) -> usize {
    let r = rx.results();
    r.table(r.decl().table_id(SCIENCE_EVENTS.key).unwrap())
        .map_or(0, |t| t.len())
}

#[test]
fn the_events_table_carries_the_utc_of_the_capture() {
    let x = signal(48e3, 8.0, GenParams::default());
    let rx = fed(&x, 1_700_000_000.0);
    let r = rx.results();
    let t = r.table(r.decl().table_id(SCIENCE_EVENTS.key).unwrap()).unwrap();
    assert!(t.len() >= 2);
    for (_, row) in t.rows_newest_first() {
        let utc = row.at(&ScienceEventRow::UTC).unwrap();
        let s = row.at(&ScienceEventRow::SIGNAL_TIME_S).unwrap();
        assert_eq!(utc, 1_700_000_000.0 + s.floor());
    }
}

#[test]
fn clear_empties_the_tables_and_reset_keeps_the_settings() {
    let x = signal(48e3, 8.0, GenParams::default());
    let mut rx = fed(&x, 0.0);
    assert!(
        rx.results().fields().at(&WhistlerFields::EVENTS_TOTAL).unwrap() >= 2,
        "{:?}",
        rx.results().status()
    );
    assert!(rows(rx.as_ref()) >= 2);
    command(rx.as_mut(), "clear");
    assert_eq!(rx.results().fields().at(&WhistlerFields::EVENTS_TOTAL), Some(0));
    assert_eq!(rows(rx.as_ref()), 0);

    let id = rx.schema().id(THRESHOLD.key()).unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), id, &Value::F64(7.0)).unwrap();
    // Between 0 and 1 dB nothing is allowed.
    assert!(sdrkit_core::owner::apply(rx.as_mut(), id, &Value::F64(0.5)).is_err());
    command(rx.as_mut(), "reset");
    assert_eq!(rx.values().unwrap().get(&THRESHOLD), 7.0);
    assert_eq!(rx.results().fields().at(&WhistlerFields::EVENTS_TOTAL), Some(0));
    // After a reset the floor is learned again: the next events come from the new samples only.
    let mut out = Output::default();
    for c in x.chunks(4096) {
        out.clear();
        rx.feed(c, &mut out);
    }
    assert!(rx.results().fields().at(&WhistlerFields::EVENTS_TOTAL).unwrap() >= 1);
}

#[test]
fn the_trend_counts_the_events_and_sferics_of_each_minute() {
    let x = signal(24e3, 66.0, GenParams::default());
    let mut d = sci::whistler::Detector::new(24e3, 0.0, 0.0).unwrap();
    let reals: Vec<f32> = x.iter().map(|z| z.re).collect();
    let mut found = Vec::new();
    let mut trend = Vec::new();
    for c in reals.chunks(5000) {
        d.process(c);
        found.extend(d.take_found());
        trend.extend(d.take_trend());
    }
    // Minute 0 is written 5 s after its end, when no detection can still fall into it.
    assert_eq!(trend.len(), 1, "{trend:?}");
    let in_minute = found.iter().filter(|f| f.row.signal_time_s < 60.0).count();
    assert_eq!(trend[0].0, 0);
    assert_eq!(trend[0].1[0], in_minute as f64);
    // Every 20 s: two whistlers and three tweeks.
    assert!(in_minute >= 12, "{found:?}");
    assert!(trend[0].1[1] >= 40.0, "{trend:?}");
}
