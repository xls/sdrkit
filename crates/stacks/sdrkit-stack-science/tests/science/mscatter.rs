//! Meteor scatter through the receiver: placed pings found and measured, and noise alone.

use sci::mscatter::{GenPing, MsGenerator, PingKind};
use sci::typed::{MscatterFields, MscatterPingEvent, MS_PING};
use sdrkit_core::{Complex32, Generator};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

#[test]
fn placed_pings_are_found_and_measured() {
    let pings = vec![
        GenPing {
            start: 8.0,
            duration: 0.4,
            doppler: 120.0,
            peak_dbc: -25.0,
            overdense: false,
        },
        GenPing {
            start: 14.0,
            duration: 2.0,
            doppler: -80.0,
            peak_dbc: -22.0,
            overdense: true,
        },
    ];
    let mut g = MsGenerator::new(48e3, 420.0, pings, 0.0, 1);
    let mut x = vec![Complex32::new(0.0, 0.0); 48_000 * 22];
    g.generate(&mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(1.0 / 10f64.powf(4.0) / 100.0 * 48e3, 3)
        .unwrap()
        .process(&mut x);
    let r = Harness::new(&sci::MSCATTER).rate(48e3).feed(&x).unwrap();
    assert_eq!(
        r.results.fields().at(&MscatterFields::PINGS),
        Some(2),
        "{}",
        r.results.status().summary
    );
    let ev: Vec<_> = r.events.iter().filter(|e| e.is(&MS_PING)).collect();
    assert_eq!(ev.len(), 2);
    let d0 = ev[0].at(&MscatterPingEvent::DOPPLER).unwrap();
    let d1 = ev[1].at(&MscatterPingEvent::DOPPLER).unwrap();
    assert!(
        (d0 - 120.0).abs() < 15.0 && (d1 + 80.0).abs() < 15.0,
        "{d0} {d1}"
    );
    assert_eq!(ev[1].at(&MscatterPingEvent::KIND), Some(PingKind::Overdense));
    assert!(ev[0].at(&MscatterPingEvent::TEXT).is_some());
}

#[test]
fn the_test_signal_returns_pings_through_the_harness() {
    let r = Harness::new(&sci::MSCATTER)
        .rate(48e3)
        .seconds(30.0)
        .set(
            &sdrkit_algo_sim::chain::GEN_NOISE,
            sdrkit_algo_sim::chain::NoiseKind::Snr,
        )
        .set(&sdrkit_algo_sim::chain::GEN_SNR_DB, 40.0)
        .run()
        .unwrap();
    let n = r.results.fields().at(&MscatterFields::PINGS).unwrap();
    assert!(n >= 2, "{}", r.results.status().summary);
}

#[test]
fn noise_alone_returns_no_ping() {
    let mut x = vec![Complex32::new(0.0, 0.0); 48_000 * 15];
    sdrkit_algo_sim::noise::Awgn::with_power(0.01, 4)
        .unwrap()
        .process(&mut x);
    let r = Harness::new(&sci::MSCATTER).rate(48e3).feed(&x).unwrap();
    assert_eq!(r.results.fields().at(&MscatterFields::PINGS), Some(0));
}
