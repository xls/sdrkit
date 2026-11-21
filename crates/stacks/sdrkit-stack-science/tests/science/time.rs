//! Time signals through the receiver: every station clean, with the standard impairments, and noise alone.

use sci::calendar;
use sci::time::codes::Station;
use sci::time::{path_delay, Day};
use sci::typed::{TimeFields, TimeMinuteEvent, TIME_MINUTE};
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::Choice;
use sdrkit_core::{Station as Site, Value};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;
use sdrkit_util_geo::Position;

const RX: Position = Position::new(54.7, 25.3);

fn start() -> f64 {
    calendar::unix(2026, 10, 12, 14, 2, 37).unwrap() as f64
}

/// The station pick that selects `st`.
fn pick(st: Station) -> sdrkit_stack_science::time::StationPick {
    sdrkit_stack_science::time::StationPick::from_key(st.info().id)
        .expect("a station that can be picked")
}

/// Runs a station for `seconds`, with the receiver's clock at the true time of arrival (the start plus the path delay).
fn run(st: Station, seconds: f64, impair: &[CaseSetting]) -> Run {
    let (delay, _, _) = path_delay(st, RX);
    let mut h = Harness::new(&sci::TIME)
        .freq(st.info().freq)
        .rate(64e3)
        .seconds(seconds)
        .piece(4096)
        .station(Site::at(RX))
        .set(&sdrkit_stack_science::time::STATION_SETTING, pick(st))
        .set(&sdrkit_stack_science::time::GEN_START_UTC, start())
        .set(
            &sdrkit_stack_science::clock::CLOCK_MODE,
            sdrkit_stack_science::clock::ClockChoice::Fixed,
        )
        .set(&sdrkit_stack_science::clock::CLOCK_ORIGIN, start() + delay)
        .with_settings(impair);
    if !impair.is_empty() {
        h = h.set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        );
    }
    h.run().unwrap()
}

fn f64_field(r: &Run, key: &str) -> f64 {
    match r.field(key) {
        Some(Value::F64(v) | Value::Time(v)) => *v,
        other => panic!("{key}: {other:?} ({})", r.results.status().summary),
    }
}

fn check(st: Station, impair: &[CaseSetting]) -> Run {
    let r = run(st, 200.0, impair);
    let ok = r.results.fields().at(&TimeFields::MINUTES_OK).unwrap_or(0);
    assert_eq!(r.results.fields().at(&TimeFields::MINUTES_BAD), Some(0), "{st:?}");
    assert!(
        ok >= 2,
        "{st:?}: {} minutes, {}",
        ok,
        r.results.status().summary
    );
    let minutes = r.events.iter().filter(|e| e.is(&TIME_MINUTE)).count();
    assert_eq!(minutes as i64, ok, "{st:?}");
    let truth = start() + 200.0;
    let got = f64_field(&r, "utc");
    assert!((got - truth).abs() < 1.5, "{st:?} {got} {truth}");
    let avg = f64_field(&r, "clock_offset_avg_s");
    let tol = if st.info().lf { 0.0055 } else { 0.0045 };
    assert!(avg.abs() < tol, "{st:?} offset {avg}");
    assert_eq!(r.results.fields().at(&TimeFields::CONFIRMED), Some(true), "{st:?}");
    r
}

#[test]
fn every_station_decodes_clean() {
    for st in [
        Station::Dcf77,
        Station::Msf,
        Station::Wwvb,
        Station::Jjy40,
        Station::Jjy60,
        Station::Bpc,
        Station::Wwv,
        Station::Bpm,
        Station::Hla,
    ] {
        let r = check(st, &[]);
        if st == Station::Dcf77 {
            assert_eq!(r.results.fields().at(&TimeFields::SUMMER), Some(true));
        }
        if st == Station::Wwv {
            assert_eq!(r.results.fields().at(&TimeFields::STATION), Some(Station::Wwv));
            assert_eq!(r.results.fields().at(&TimeFields::WWVH), Some(false));
        }
    }
}

#[test]
fn impaired() {
    check(
        Station::Dcf77,
        &[
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 12.0),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 4.0),
        ],
    );
    check(
        Station::Msf,
        &[
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 12.0),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, -3.0),
        ],
    );
    check(
        Station::Wwvb,
        &[
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 15.0),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 2.0),
        ],
    );
    check(
        Station::Wwv,
        &[
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 20.0),
        ],
    );
    check(
        Station::Bpm,
        &[
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_SNR_DB, 25.0),
            CaseSetting::new(&sdrkit_algo_sim::chain::GEN_OFFSET_HZ, 15.0),
        ],
    );
}

#[test]
fn the_weekday_is_published_as_a_day() {
    // 2026-10-12 is a Monday.
    let r = run(Station::Dcf77, 150.0, &[]);
    assert_eq!(r.results.fields().at(&TimeFields::WEEKDAY), Some(Day::Monday));
}

#[test]
fn noise_only_returns_nothing() {
    for st in [Station::Dcf77, Station::Wwv] {
        let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); (130.0 * 64e3) as usize];
        sdrkit_algo_sim::noise::Awgn::with_power(0.02, 5)
            .unwrap()
            .process(&mut x);
        let r = Harness::new(&sci::TIME)
            .freq(st.info().freq)
            .rate(64e3)
            .set(&sdrkit_stack_science::time::STATION_SETTING, pick(st))
            .feed(&x)
            .unwrap();
        assert_eq!(r.results.fields().at(&TimeFields::MINUTES_OK), Some(0));
        assert!(r.field("utc").is_none_or(|v| *v == Value::None));
    }
}

/// With no station position the receiver still decodes, reports no path delay, and says why.
#[test]
fn no_station_position_means_no_path_delay() {
    let r = Harness::new(&sci::TIME)
        .freq(77.5e3)
        .rate(64e3)
        .seconds(150.0)
        .set(&sdrkit_stack_science::time::GEN_START_UTC, start())
        .run()
        .unwrap();
    assert!(r.field("delay_ms").is_none_or(|v| *v == Value::None));
    assert!(
        r.results.status().summary.contains("no station position"),
        "{}",
        r.results.status().summary
    );
}

#[test]
fn rwm_dut1() {
    use sci::time::codes::EncodeOptions;
    use sci::time::{TimeGenOptions, TimeGenerator};
    use sdrkit_core::Generator;
    let o = TimeGenOptions {
        start_utc: 1_780_272_000.0,
        enc: EncodeOptions {
            dut1: 3,
            dut1_fine: -2,
            ..Default::default()
        },
        ..TimeGenOptions::new(Station::Rwm)
    };
    let mut g = TimeGenerator::new(64e3, o);
    let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); (150.0 * 64e3) as usize];
    g.generate(&mut x);
    let r = Harness::new(&sci::TIME)
        .freq(9.996e6)
        .rate(64e3)
        .set(
            &sdrkit_stack_science::time::STATION_SETTING,
            sdrkit_stack_science::time::StationPick::Rwm,
        )
        .feed(&x)
        .unwrap();
    assert_eq!(f64_field(&r, "dut1_s"), 0.3);
    let e = r.events.iter().find(|e| e.is(&TIME_MINUTE)).unwrap();
    assert!(e.at(&TimeMinuteEvent::TEXT).is_some());
}
