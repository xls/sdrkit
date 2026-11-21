//! GOES DCS: the generator's platforms through the standard impairments into the receiver.

use crate::common::{cell, command, row_count, rows, set, values};
use sdrkit_components::sim::impaired;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, Generator, LockState, Results, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_satcom::modes::goes::{self, body, gen};
use sdrkit_stack_satcom::typed::GoesDcsReportEvent;
use serde_json::json;

const CENTRE: f64 = 401.85e6;

/// The transmitters `txs` at `rate`, through the standard impairments `imp` (`gen_*` keys and values).
fn signal(rate: f64, txs: Vec<gen::Tx>, imp: &[CaseSetting], seconds: f64) -> Vec<Complex32> {
    let mut given = vec![CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SEED, 3)];
    given.extend(imp.iter().cloned());
    let v = values(&goes::DESCRIPTOR, &given);
    let mut g = impaired(Box::new(gen::GoesGenerator::new(rate, CENTRE, txs)), &v).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (seconds * rate) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

fn snr(db: f64) -> Vec<CaseSetting> {
    vec![
        CaseSetting::new(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        ),
        CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, db),
    ]
}

fn receive(rate: f64, x: &[Complex32]) -> Run {
    Harness::new(&goes::DESCRIPTOR)
        .rate(rate)
        .freq(CENTRE)
        .piece(20_000)
        .feed(x)
        .unwrap()
}

fn reports(run: &Run) -> Vec<Event> {
    run.events_of("goes_dcs.report").cloned().collect()
}

fn int(r: &Results, key: &str) -> i64 {
    let id = r.decl().field_id(key).unwrap();
    match r.fields().value(id) {
        Some(Value::I64(v)) => *v,
        other => panic!("{key}: {other:?}"),
    }
}

fn address(a: u32) -> String {
    let w = body::encode_address(a)
        .iter()
        .fold(0u64, |v, &b| (v << 1) | b as u64);
    format!("{w:08X}")
}

#[test]
fn d3_each_rate_and_coding_on_channel_100() {
    let rate = 96e3;
    for (r, man) in [
        (100.0, true),
        (300.0, false),
        (300.0, true),
        (1200.0, false),
        (1200.0, true),
    ] {
        let mut t = gen::Tx::new(100, 0x0A_2B3C, r, man, 0.5);
        t.repeat_s = 1000.0;
        let secs = t.length_s() + 1.5;
        let run = receive(rate, &signal(rate, vec![t], &snr(40.0), secs));
        let ev = reports(&run);
        assert_eq!(ev.len(), 1, "{r} {man}");
        let e = &ev[0];
        assert_eq!(e.get(&keys::TEXT), Some(gen::TEXT), "{r} {man}");
        assert_eq!(e.at(&GoesDcsReportEvent::CHANNEL), Some(100));
        assert_eq!(e.at(&GoesDcsReportEvent::EOT), Some(true));
        assert_eq!(e.at(&GoesDcsReportEvent::PARITY_ERRORS), Some(0));
        assert_eq!(e.at(&GoesDcsReportEvent::BITS_CORRECTED), Some(0));
        assert!(e.get(&keys::SNR_DB).unwrap() > 18.0);
        assert_eq!(e.at(&GoesDcsReportEvent::BIT_RATE_HZ), Some(r));
        assert_eq!(e.at(&GoesDcsReportEvent::MANCHESTER), Some(man));
        assert_eq!(
            e.at(&GoesDcsReportEvent::FLAGS),
            Some(if r > 100.0 { 0x5A } else { -1 })
        );
        assert_eq!(
            e.at(&GoesDcsReportEvent::ADDRESS),
            Some(address(0x0A_2B3C).as_str())
        );
        assert!(
            (e.freq_hz - body::channel_hz(100)).abs() < 100.0,
            "{}",
            e.freq_hz
        );
        let reports = rows(&run.results, "reports");
        let row = reports[0].1;
        assert!(cell(row, "text").starts_with("WL 12.34"), "{row:?}");
        assert_eq!(row_count(&run.results, "platforms"), 1);
        assert_eq!(run.results.status().lock_state, LockState::Locked);
    }
}

#[test]
fn d3_three_platforms_frequency_and_clock_errors() {
    let rate = 250e3;
    let mut txs = gen::scenario(3);
    for (i, t) in txs.iter_mut().enumerate() {
        t.freq_err_hz = [120.0, -200.0, 60.0, 0.0][i];
        t.clock_ppm = [80.0, -80.0, 50.0, 0.0][i];
    }
    let mut imp = snr(35.0);
    imp.push(CaseSetting::new(
        &sdrkit_algo_sim::chain::GEN_CLOCK_PPM,
        20.0,
    ));
    imp.push(CaseSetting::new(
        &sdrkit_algo_sim::chain::GEN_OFFSET_HZ,
        150.0,
    ));
    let run = receive(rate, &signal(rate, txs, &imp, 34.0));
    let ev = reports(&run);
    for (ch, addr) in [(60usize, 0x01_2345u32), (100, 0x0A_2B3C), (140, 0x15_4D5E)] {
        let mine: Vec<&Event> = ev
            .iter()
            .filter(|e| e.at(&GoesDcsReportEvent::ADDRESS) == Some(address(addr).as_str()))
            .collect();
        assert!(mine.len() >= 2, "{ch}: {}", mine.len());
        assert!(mine
            .iter()
            .all(|e| e.at(&GoesDcsReportEvent::CHANNEL) == Some(ch as i64)));
    }
    // The steady carrier produces no platform.
    assert_eq!(row_count(&run.results, "platforms"), 3);
    assert!(ev
        .iter()
        .all(|e| e.get(&keys::TEXT) == Some(gen::TEXT)
            && e.at(&GoesDcsReportEvent::EOT) == Some(true)));
}

/// The bit clock follows a platform clock a few hundred ppm off through a long report.
#[test]
fn long_reports_follow_the_platform_clock() {
    let rate = 96e3;
    for ppm in [-400.0, 400.0] {
        let mut t = gen::Tx::new(100, 0x0A_2B3C, 300.0, false, 0.5);
        t.repeat_s = 1000.0;
        t.data = gen::TEXT.repeat(12).into_bytes();
        t.clock_ppm = ppm;
        let secs = t.length_s() + 1.5;
        let run = receive(rate, &signal(rate, vec![t], &snr(35.0), secs));
        let ev = reports(&run);
        assert_eq!(ev.len(), 1, "{ppm}");
        assert_eq!(
            ev[0].get(&keys::TEXT),
            Some(gen::TEXT.repeat(12).as_str()),
            "{ppm}"
        );
        assert_eq!(
            ev[0].at(&GoesDcsReportEvent::PARITY_ERRORS),
            Some(0),
            "{ppm}"
        );
    }
}

#[test]
fn noise_and_a_steady_carrier_give_nothing() {
    let rate = 96e3;
    let mut s = gen::Tx::new(100, 0, 100.0, false, 0.0);
    s.steady = true;
    let run = receive(rate, &signal(rate, vec![s], &snr(40.0), 8.0));
    assert!(reports(&run).is_empty());
    let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
    let x: Vec<Complex32> = (0..(5.0 * rate) as usize)
        .map(|_| rng.complex_normal(0.005))
        .collect();
    let run = receive(rate, &x);
    assert!(reports(&run).is_empty());
    assert_eq!(row_count(&run.results, "platforms"), 0);
    assert_eq!(run.results.status().lock_state, LockState::Searching);
}

#[test]
fn the_stack_generator_and_its_events() {
    let run = Harness::new(&goes::DESCRIPTOR)
        .rate(96e3)
        .seconds(16.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 30.0)
        .run()
        .unwrap();
    let ev = reports(&run);
    assert_eq!(ev.len(), 3, "{ev:?}");
    for e in &run.events {
        assert!(e.is(&sdrkit_stack_satcom::typed::GOES_DCS_REPORT), "{e:?}");
        assert!(e.freq_hz > 401e6, "{}", e.freq_hz);
    }
    assert_eq!(int(&run.results, "frames_ok"), 3);
    let id = run.results.decl().series_id("spectrum").unwrap();
    assert!(run.results.series(id).unwrap().points.len() == goes::LINE_POINTS);
}

#[test]
fn clear_and_saved_state() {
    let rate = 96e3;
    let mut t = gen::Tx::new(100, 0x0A_2B3C, 1200.0, true, 0.3);
    t.repeat_s = 1000.0;
    let x = signal(rate, vec![t.clone()], &snr(40.0), t.length_s() + 1.2);
    let h = Harness::new(&goes::DESCRIPTOR).rate(rate).freq(CENTRE);
    let mut r = h.receiver().unwrap();
    r.configure(&sdrkit_core::InputInfo::new(rate, CENTRE, CENTRE).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    r.feed(&x, &mut out);
    let state = r.save_state().unwrap();
    command(r.as_mut(), "clear").unwrap();
    r.feed(&x[..24_000], &mut out);
    assert_eq!(row_count(r.results(), "reports"), 0);
    r.restore_state(&state).unwrap();
    r.feed(&x[..24_000], &mut out);
    assert_eq!(row_count(r.results(), "reports"), 1);
    assert!(set(r.as_mut(), "threshold_db", json!(40.0)).is_err());
}
