//! Pulse radar detection through the receiver: the rotation, repetition and beamwidth of marine radars at two speeds and two
//! seeds, a weak main lobe without side lobes, noise alone, the sweeps and volumes of a weather radar, 8-bit samples at the
//! stack's own rate, pieces of any size and the clear command. Captures are fed from the generator piece by piece.

use sci::radar::gen::{GenParams, Kind, RadarGenerator};
use sci::radar::DESCRIPTOR;
use sci::typed::{RadarDetectFields, RadarDetectRowEvent, RADAR_DETECT_ROWS, RADAR_ROW};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, Results};
use sdrkit_dev_test::{Harness, Quantizer};
use sdrkit_stack_science as sci;

/// A row event: time, rotation, PRF text, width, peak.
#[derive(Clone, Debug, PartialEq)]
struct Row {
    t: f64,
    rotation: Option<f64>,
    prf: String,
    width: Option<f64>,
    peak: f64,
}

/// What a capture gave: the row events and the results at the end.
struct Capture {
    rows: Vec<Row>,
    results: Results,
    rx: Box<dyn Receiver>,
}

/// `seconds` of the signal `p` at `rate` for `submode`, fed in pieces of `piece` samples, through an 8-bit quantiser with full
/// scale 4.0 when `quantise`.
fn capture(
    submode: &str,
    rate: f64,
    seconds: f64,
    p: GenParams,
    seed: u64,
    piece: usize,
    quantise: bool,
) -> Capture {
    let mut rx = Harness::new(&DESCRIPTOR)
        .submode(submode)
        .rate(rate)
        .receiver()
        .unwrap();
    let f = DESCRIPTOR.tuning.default_hz;
    rx.configure(&InputInfo::new(rate, f, f).recording())
        .unwrap();
    let mut g = RadarGenerator::new(rate, p, seed);
    let q = Quantizer::new(8, 4.0);
    let mut left = (rate * seconds).round() as usize;
    let mut x = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut rows = Vec::new();
    let mut take = |out: &mut Output| {
        for e in out.events.drain(..).filter(|e| e.is(&RADAR_ROW)) {
            rows.push(Row {
                t: e.time,
                rotation: e.at(&RadarDetectRowEvent::ROTATION_S),
                prf: e.at(&RadarDetectRowEvent::PRF).unwrap_or("").to_string(),
                width: e.at(&RadarDetectRowEvent::PULSE_WIDTH_US),
                peak: e.at(&RadarDetectRowEvent::PEAK_DB).unwrap_or(0.0),
            });
        }
    };
    while left > 0 {
        let n = left.min(piece);
        g.generate(&mut x[..n]);
        if quantise {
            q.process(&mut x[..n]);
        }
        out.clear();
        rx.feed(&x[..n], &mut out);
        take(&mut out);
        left -= n;
    }
    out.clear();
    rx.finish(&mut out);
    take(&mut out);
    Capture {
        rows,
        results: rx.results().clone(),
        rx,
    }
}

fn number(r: &Results, key: &str) -> Option<f64> {
    let id = r.decl().field_id(key)?;
    match r.fields().value(id)? {
        sdrkit_core::Value::F64(v) | sdrkit_core::Value::Hz(v) => Some(*v),
        sdrkit_core::Value::I64(v) => Some(*v as f64),
        _ => None,
    }
}

fn text(r: &Results, key: &str) -> String {
    let Some(id) = r.decl().field_id(key) else {
        return String::new();
    };
    match r.fields().value(id) {
        Some(sdrkit_core::Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

fn table_rows(r: &Results) -> usize {
    r.table(r.decl().table_id(RADAR_DETECT_ROWS.key).unwrap())
        .map_or(0, |t| t.len())
}

#[test]
fn marine_radars_at_24_and_48_rpm_give_their_rotation_prf_and_beamwidth() {
    for rpm in [24.0, 48.0] {
        for seed in [1, 2] {
            let p = GenParams {
                rpm,
                ..GenParams::of(Kind::Marine)
            };
            let period = 60.0 / rpm;
            let c = capture("marine", 2e6, 5.2 * period, p, seed, 65_536, false);
            let r = &c.results;
            let rot = number(r, RadarDetectFields::ROTATION_S.key).unwrap_or(0.0);
            assert!(
                (rot - period).abs() <= 0.02 * period,
                "{rpm} rpm, seed {seed}: {rot} ({})",
                r.status().summary
            );
            assert_eq!(text(r, RadarDetectFields::PRF.key), "1000 Hz", "{rpm} rpm, seed {seed}");
            let bw = number(r, RadarDetectFields::BEAMWIDTH_DEG.key).unwrap_or(0.0);
            assert!((1.2..=5.0).contains(&bw), "{rpm} rpm, seed {seed}: beamwidth {bw}");
            assert!(!c.rows.is_empty() && c.rows[0].prf == "1000 Hz", "{:?}", c.rows);
            let w = number(r, RadarDetectFields::PULSE_WIDTH_US.key).unwrap_or(0.0);
            assert!((0.4..=1.6).contains(&w), "width {w} us");
        }
    }
}

#[test]
fn an_18_db_main_lobe_without_side_lobes_gives_the_rotation() {
    let p = GenParams {
        peaks_db: vec![18.0],
        side_lobe_db: None,
        prf_hz: 1500.0,
        ..GenParams::of(Kind::Marine)
    };
    let c = capture("marine", 2e6, 12.5, p, 3, 65_536, false);
    let rot = number(&c.results, RadarDetectFields::ROTATION_S.key).unwrap_or(0.0);
    assert!((rot - 2.5).abs() <= 0.05, "{rot}: {}", c.results.status().summary);
}

#[test]
fn six_seconds_of_noise_give_no_rotation_and_no_row() {
    let p = GenParams {
        peaks_db: vec![-100.0],
        scale: 0.1,
        ..GenParams::of(Kind::Marine)
    };
    let c = capture("marine", 2e6, 6.0, p, 4, 65_536, false);
    assert_eq!(number(&c.results, RadarDetectFields::ROTATION_S.key), None);
    assert_eq!(number(&c.results, RadarDetectFields::ROWS_TOTAL.key), Some(0.0));
    assert!(c.rows.is_empty());
}

#[test]
fn a_weather_radar_gives_its_rotation_dual_prf_and_volumes() {
    let p = GenParams::of(Kind::Weather);
    let c = capture("weather", 2e6, 8.5 * 6.0, p, 5, 65_536, false);
    let r = &c.results;
    let rot = number(r, RadarDetectFields::ROTATION_S.key).unwrap_or(0.0);
    assert!((rot - 6.0).abs() <= 0.1, "{rot}: {}", r.status().summary);
    let prf = text(r, RadarDetectFields::PRF.key);
    assert!(prf.starts_with("dual "), "{prf}");
    assert!(prf.contains("1200 / 900 Hz"), "{prf}");
    let v = number(r, RadarDetectFields::VOLUMES.key).unwrap_or(-1.0);
    assert!((1.0..=2.0).contains(&v), "{v} volumes");
    // A row for every sweep.
    assert!(c.rows.len() >= 8, "{:?}", c.rows);
}

#[test]
fn eight_bit_samples_at_the_stacks_own_rate() {
    let marine = GenParams {
        rpm: 48.0,
        ..GenParams::of(Kind::Marine)
    };
    let c = capture("marine", 10e6, 3.0, marine, 6, 16_384, true);
    let rot = number(&c.results, RadarDetectFields::ROTATION_S.key).unwrap_or(0.0);
    assert!((rot - 1.25).abs() <= 0.025, "{rot}: {}", c.results.status().summary);
    assert_eq!(text(&c.results, RadarDetectFields::PRF.key), "1000 Hz");
    let c = capture("weather", 2e6, 14.0, GenParams::of(Kind::Weather), 7, 16_384, true);
    let rot = number(&c.results, RadarDetectFields::ROTATION_S.key).unwrap_or(0.0);
    assert!((rot - 6.0).abs() <= 0.1, "{rot}: {}", c.results.status().summary);
    assert!(text(&c.results, RadarDetectFields::PRF.key).starts_with("dual "));
}

#[test]
fn pieces_of_any_size_give_the_same_rows() {
    let p = GenParams::of(Kind::Weather);
    let runs: Vec<Vec<Row>> = [997usize, 16_384, 65_536]
        .iter()
        .map(|&n| capture("weather", 2e6, 14.0, p.clone(), 8, n, false).rows)
        .collect();
    assert!(runs[0].len() >= 3, "{:?}", runs[0]);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}

#[test]
fn clear_empties_the_rows() {
    let p = GenParams {
        rpm: 48.0,
        ..GenParams::of(Kind::Marine)
    };
    let mut c = capture("marine", 2e6, 4.0, p, 9, 65_536, false);
    assert!(table_rows(c.rx.results()) >= 1);
    let id = c.rx.commands().id("clear").unwrap();
    let args = c.rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(c.rx.as_mut(), id, &args).unwrap();
    assert_eq!(table_rows(c.rx.results()), 0);
    assert_eq!(number(c.rx.results(), RadarDetectFields::ROWS_TOTAL.key), Some(0.0));
    assert_eq!(number(c.rx.results(), RadarDetectFields::ROTATION_S.key), None);
}

#[test]
fn the_verify_cases_pass_with_and_without_their_submode() {
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 5.0,
        clean: false,
        snr_db: 20.0,
    };
    for case in (DESCRIPTOR.verify.unwrap().cases)(&o) {
        for submode in [case.submode.clone(), None] {
            let mut h = Harness::new(&DESCRIPTOR)
                .seconds(case.seconds)
                .piece(16_384)
                .with_settings(&case.settings);
            if let Some(s) = &submode {
                h = h.submode(s);
            }
            let r = h.run().unwrap();
            for line in (case.check.unwrap())(&r.results) {
                assert_eq!(
                    line.outcome,
                    sdrkit_core::verify::Outcome::Pass,
                    "{} {submode:?}: {line:?}: {}",
                    case.name,
                    r.results.status().summary
                );
            }
        }
    }
}

#[test]
fn the_generator_builds_at_250_ksps_without_a_submode() {
    let schema = sdrkit_components::sim::with_generator_settings(&(DESCRIPTOR.settings)()).unwrap();
    let cfg = sdrkit_core::GenConfig::new(250e3, sdrkit_core::Values::defaults(&schema)).with_seed(1);
    let mut g = (DESCRIPTOR.make_generator.unwrap())(&cfg).unwrap();
    assert_eq!(g.station_offset_hz(0.0), 0.0);
    assert!(g.signal_info().is_some());
    let mut x = vec![Complex32::default(); 250_000];
    assert_eq!(g.generate(&mut x), x.len());
}
