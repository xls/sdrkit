//! Schumann resonances: the peaks of the test signal at 200 sps with a frequency error, a drift and a raised background, ELF
//! transients, 48 ksps through the receiver, the trend and event rows in the CSV form, pieces of any size, clear and reset.

use sci::events::{trend_csv, EventRow};
use sci::typed::{
    ScienceEventRow, SchumannFields, SchumannTrendRow, SCHUMANN_TREND, SCIENCE_EVENT, SCIENCE_EVENTS,
};
use sci::schumann::gen::{GenParams, SchumannGenerator, GEN_TRANSIENTS};
use sci::schumann::{
    Monitor, DESCRIPTOR, THRESHOLD,
};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, Value};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

/// The real part of `seconds` of the test signal `p` at `rate`.
fn signal(rate: f64, seconds: f64, p: GenParams) -> Vec<f32> {
    let mut g = SchumannGenerator::new(rate, p, 3);
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x.iter().map(|z| z.re).collect()
}

/// A monitor with its transients and trend rows.
type Fed = (Monitor, Vec<EventRow>, Vec<(u64, [f64; 2])>);

/// A monitor at `rate` fed `x` in pieces of `piece`, with its transients and trend rows.
fn monitor(rate: f64, x: &[f32], piece: usize) -> Fed {
    let mut m = Monitor::new(rate, 0.0, 0.0).unwrap();
    let mut found = Vec::new();
    let mut trend = Vec::new();
    for c in x.chunks(piece) {
        m.process(c);
        found.extend(m.take_found());
        trend.extend(m.take_trend());
    }
    (m, found, trend)
}

fn fundamental(m: &Monitor) -> f64 {
    m.resonances()[0].map_or(0.0, |r| r.frequency_hz)
}

#[test]
fn the_test_signal_of_420_s_at_200_sps() {
    let (m, found, trend) = monitor(200.0, &signal(200.0, 420.0, GenParams::default()), 4096);
    let res = m.resonances();
    let n = res.iter().flatten().count();
    assert!(n >= 4, "{res:?}");
    assert!((fundamental(&m) - 7.83).abs() <= 0.8, "{res:?}");
    let second = res[1].map_or(0.0, |r| r.frequency_hz);
    assert!((12.5..=15.7).contains(&second), "{res:?}");
    assert!(found.is_empty(), "{found:?}");
    assert!(trend.len() >= 5, "{trend:?}");
    assert!(res[0].unwrap().q > 1.0, "{res:?}");
}

#[test]
fn a_frequency_error_and_a_drift() {
    let p = GenParams {
        fundamental_hz: 7.83 + 0.4,
        drift_hz_per_s: 0.2 / 420.0,
        ..GenParams::default()
    };
    let (m, ..) = monitor(200.0, &signal(200.0, 420.0, p), 4096);
    assert!((fundamental(&m) - 8.3).abs() <= 0.9, "{:?}", m.resonances());
}

#[test]
fn a_raised_background() {
    for db in [0.0, 6.0] {
        let p = GenParams {
            background_db: db,
            ..GenParams::default()
        };
        let (m, ..) = monitor(200.0, &signal(200.0, 420.0, p), 4096);
        let f = fundamental(&m);
        assert!((6.5..=9.2).contains(&f), "{db} dB: {:?}", m.resonances());
    }
}

#[test]
fn transients_every_60_s_from_70_s() {
    let p = GenParams {
        transients: 2.0,
        ..GenParams::default()
    };
    let (_, found, _) = monitor(200.0, &signal(200.0, 340.0, p), 4096);
    let hit = [70.0, 130.0, 190.0, 250.0, 310.0]
        .iter()
        .filter(|&&t| {
            found
                .iter()
                .any(|e| e.kind == "transient" && (e.signal_time_s - t).abs() <= 3.0)
        })
        .count();
    assert!(hit >= 4, "{found:?}");
    assert!(found.len() <= 5, "{found:?}");
    for e in &found {
        assert!((8.0..=16.0).contains(&e.frequency_hz), "{e:?}");
    }
}

#[test]
fn the_test_signal_of_150_s_at_48_ksps_through_the_receiver() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(48e3)
        .seconds(150.0)
        .piece(65_536)
        .run()
        .unwrap();
    let f = r.results.fields().at(&SchumannFields::FUNDAMENTAL_HZ).unwrap_or(0.0);
    assert!((f - 7.83).abs() <= 0.9, "{}", r.results.status().summary);
    assert!(r.results.fields().at(&SchumannFields::PEAKS_FOUND).unwrap_or(0) >= 4);
}

#[test]
fn pieces_of_any_size_give_the_same_output() {
    let p = GenParams {
        transients: 2.0,
        ..GenParams::default()
    };
    let x = signal(400.0, 140.0, p);
    let (a, fa, ta) = monitor(400.0, &x, 1);
    let (b, fb, tb) = monitor(400.0, &x, 777);
    let (c, fc, tc) = monitor(400.0, &x, 40_000);
    assert!(!fa.is_empty() && !ta.is_empty());
    assert_eq!((fa.clone(), ta.clone()), (fb, tb));
    assert_eq!((fa, ta), (fc, tc));
    assert_eq!(a.resonances(), b.resonances());
    assert_eq!(a.resonances(), c.resonances());
    assert!(a.resonances()[0].is_some());
}

/// A receiver fed `x` at 400 sps with `start_utc` and the transients of the test signal set.
fn fed(x: &[f32], start_utc: f64) -> Box<dyn Receiver> {
    let h = Harness::new(&DESCRIPTOR)
        .rate(400.0)
        .set(&sci::mscatter::START_UTC, start_utc);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(400.0, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    let z: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    for c in z.chunks(1000) {
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

fn table_len(rx: &dyn Receiver, key: &str) -> usize {
    let r = rx.results();
    r.table(r.decl().table_id(key).unwrap())
        .map_or(0, |t| t.len())
}

#[test]
fn trend_and_event_rows_in_the_csv_form() {
    let p = GenParams {
        transients: 2.0,
        ..GenParams::default()
    };
    let rx = fed(&signal(400.0, 125.0, p), 1_700_000_000.0);
    let r = rx.results();
    // The trend: row 1 at the start + 60 s.
    let t = r.table(r.decl().table_id(SCHUMANN_TREND.key).unwrap()).unwrap();
    let mut rows: Vec<(f64, [f64; 2])> = t
        .rows_newest_first()
        .map(|(_, row)| {
            (
                row.at(&SchumannTrendRow::UTC).unwrap(),
                [
                    row.at(&SchumannTrendRow::FUNDAMENTAL_HZ).unwrap(),
                    row.at(&SchumannTrendRow::HEIGHT_DB).unwrap(),
                ],
            )
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].0, 1_700_000_060.0);
    let csv = trend_csv(
        1_700_000_000.0,
        ["fundamental_hz", "height_db"],
        &rows.iter().map(|r| r.1).collect::<Vec<_>>(),
    );
    assert!(
        csv.lines()
            .nth(1)
            .unwrap()
            .starts_with("2023-11-14T22:14:20Z,"),
        "{csv}"
    );
    // The event rows: the UTC of the capture start plus the whole seconds of the event time.
    let e = r.table(r.decl().table_id(SCIENCE_EVENTS.key).unwrap()).unwrap();
    assert!(!e.is_empty());
    for (_, row) in e.rows_newest_first() {
        let s = row.at(&ScienceEventRow::SIGNAL_TIME_S).unwrap();
        assert_eq!(row.at(&ScienceEventRow::UTC).unwrap(), 1_700_000_000.0 + s.floor());
    }
}

#[test]
fn clear_empties_the_tables_and_reset_keeps_the_settings() {
    let p = GenParams {
        transients: 2.0,
        ..GenParams::default()
    };
    let x = signal(400.0, 135.0, p);
    let mut rx = fed(&x, 0.0);
    assert!(rx.results().fields().at(&SchumannFields::EVENTS_TOTAL).unwrap() >= 1);
    assert!(table_len(rx.as_ref(), SCIENCE_EVENTS.key) >= 1);
    assert!(table_len(rx.as_ref(), SCHUMANN_TREND.key) >= 2);
    command(rx.as_mut(), "clear");
    assert_eq!(rx.results().fields().at(&SchumannFields::EVENTS_TOTAL), Some(0));
    assert_eq!(table_len(rx.as_ref(), SCIENCE_EVENTS.key), 0);
    assert_eq!(table_len(rx.as_ref(), SCHUMANN_TREND.key), 0);
    // The average stays: the resonances are still there.
    assert!(rx.results().fields().at(&SchumannFields::FUNDAMENTAL_HZ).is_some());

    let id = rx.schema().id(THRESHOLD.key()).unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), id, &Value::F64(10.0)).unwrap();
    assert!(sdrkit_core::owner::apply(rx.as_mut(), id, &Value::F64(0.5)).is_err());
    command(rx.as_mut(), "reset");
    assert_eq!(rx.values().unwrap().get(&THRESHOLD), 10.0);
    // The average is learned again.
    assert_eq!(rx.results().fields().at(&SchumannFields::FUNDAMENTAL_HZ), None);
}

#[test]
fn the_test_signal_through_the_harness_has_transients_when_asked() {
    let r = Harness::new(&DESCRIPTOR)
        .rate(400.0)
        .seconds(100.0)
        .set(&GEN_TRANSIENTS, 2.0)
        .run()
        .unwrap();
    let ev: Vec<_> = r.events.iter().filter(|e| e.is(&SCIENCE_EVENT)).collect();
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert!((ev[0].time - 70.0).abs() <= 3.0, "{ev:?}");
}
