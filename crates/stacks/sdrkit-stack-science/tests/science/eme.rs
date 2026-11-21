//! The moon echo test through the receiver: echoes of four pulses with their delay, Doppler shift and spread, nothing without
//! a transmission, the default test signal, pieces of any size and the clear command.

use sci::eme::gen::{EmeGenerator, GenParams};
use sci::eme::DESCRIPTOR;
use sci::typed::{EmeEchoFields, ScienceEvent, SCIENCE_EVENT};
use sdrkit_core::{Complex32, Generator};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;

#[derive(Clone, Debug, PartialEq)]
struct Ev {
    t: f64,
    kind: String,
    doppler: f64,
    level: f64,
    delay: f64,
    width: f64,
}

fn events(r: &Run) -> Vec<Ev> {
    r.events
        .iter()
        .filter(|e| e.is(&SCIENCE_EVENT))
        .map(|e| Ev {
            t: e.time,
            kind: e.at(&ScienceEvent::TYPE).unwrap_or("").to_string(),
            doppler: e.at(&ScienceEvent::FREQUENCY_HZ).unwrap_or(0.0),
            level: e.at(&ScienceEvent::PEAK_DB).unwrap_or(0.0),
            delay: e.at(&ScienceEvent::AUX1).unwrap_or(0.0),
            width: e.at(&ScienceEvent::AUX2).unwrap_or(0.0),
        })
        .collect()
}

/// 56 s at 8 ksps: pulses of 1.5 s at 10, 20, 30 and 40 s, the carrier at +15 Hz drifting 10 Hz over the run; the echo 2.55 s
/// later, +120 Hz, 4 Hz of spread, `echo_db` over the noise in 12 Hz.
fn four_pulses(echo_db: f64, pulses: bool) -> Vec<Complex32> {
    let p = GenParams {
        pulses: if pulses { vec![10.0, 20.0, 30.0, 40.0] } else { Vec::new() },
        cycle_s: None,
        carrier_hz: 15.0,
        carrier_drift_hz_s: 10.0 / 56.0,
        echo_db,
        ..GenParams::default()
    };
    let mut g = EmeGenerator::new(8e3, p, 21);
    let mut x = vec![Complex32::default(); (8e3 * 56.0) as usize];
    g.generate(&mut x);
    x
}

#[test]
fn echoes_of_12_db_give_delay_doppler_and_spread() {
    let r = Harness::new(&DESCRIPTOR).rate(8e3).feed(&four_pulses(12.0, true)).unwrap();
    let ev = events(&r);
    assert!(ev.len() >= 3 && ev.len() <= 4, "{ev:?}");
    let mean_err = ev.iter().map(|e| (e.delay - 2.55).abs()).sum::<f64>() / ev.len() as f64;
    assert!(mean_err < 0.08, "{mean_err}: {ev:?}");
    for e in &ev {
        assert_eq!(e.kind, "echo");
        assert!((e.doppler - 120.0).abs() <= 12.0, "{e:?}");
        assert!((3.0..=40.0).contains(&e.width), "{e:?}");
        assert!(e.level > 0.0, "{e:?}");
    }
    assert_eq!(r.results.fields().at(&EmeEchoFields::PULSES), Some(4));
}

#[test]
fn no_transmission_gives_no_event() {
    let r = Harness::new(&DESCRIPTOR).rate(8e3).feed(&four_pulses(-60.0, false)).unwrap();
    assert!(events(&r).is_empty(), "{:?}", events(&r));
    assert_eq!(r.results.fields().at(&EmeEchoFields::PULSES), Some(0));
}

#[test]
fn the_test_signal_gives_echoes_at_250_ksps() {
    let r = Harness::new(&DESCRIPTOR).rate(250e3).seconds(20.0).run().unwrap();
    let ev = events(&r);
    assert_eq!(ev.len(), 2, "{ev:?}");
    for e in &ev {
        assert!((e.delay - 2.55).abs() < 0.1, "{e:?}");
        assert!((e.doppler - 120.0).abs() <= 12.0, "{e:?}");
    }
    assert_eq!(r.results.fields().at(&EmeEchoFields::ECHOES), Some(2));
}

#[test]
fn pieces_of_any_size_give_the_same_echoes() {
    let x = four_pulses(12.0, true);
    let runs: Vec<Vec<Ev>> = [333usize, 4096, 100_000]
        .iter()
        .map(|&n| events(&Harness::new(&DESCRIPTOR).rate(8e3).piece(n).feed(&x).unwrap()))
        .collect();
    assert!(runs[0].len() >= 3);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}
