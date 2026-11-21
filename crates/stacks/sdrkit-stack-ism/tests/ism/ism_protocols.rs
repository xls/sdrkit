//! `ism` acceptance tests 1, 2 and 11: every test device's train decodes to exactly its report, single bit errors never give
//! another reading, a repeat protocol needs its copies, and the line-code guess. Also the pulse detector the stack runs on each
//! burst.

use sdrkit_core::Complex32;
use sdrkit_proto_ookdev::guess::{Guesser, LineCode};
use sdrkit_proto_ookdev::{Modulation, Pulse, Reading, Train};
use sdrkit_stack_ism::ism::detect::{IsmBurst, Pulses};
use sdrkit_stack_ism::ism::gen::{device_train, train_samples, TestDevice};
use sdrkit_stack_ism::ism::protocol_list;

fn decode(t: &Train) -> Vec<Reading> {
    let mut out = Vec::new();
    protocol_list().decode(t, &mut out).unwrap();
    out
}

#[test]
fn every_protocol_clean() {
    for d in TestDevice::ALL {
        for k in 0..4 {
            let r = decode(&device_train(d, k));
            assert_eq!(r.len(), 1, "{d:?} k={k}: {r:#?}");
            assert_eq!(r[0].model, d.model(), "{d:?}");
            assert_eq!(r[0].id, d.id(), "{d:?}");
        }
    }
}

/// The marks and spaces of `t` as bits at `bit` samples each (1 for a mark).
fn levels(t: &Train, bit: f32) -> Vec<u8> {
    let mut v = Vec::new();
    for (i, p) in t.pulses.iter().enumerate() {
        v.extend(std::iter::repeat_n(1u8, (p.mark / bit).round() as usize));
        if i + 1 < t.pulses.len() || t.modulation == Modulation::Fsk {
            v.extend(std::iter::repeat_n(0u8, (p.space / bit).round() as usize));
        }
    }
    v
}

/// A train of `levels` at `bit` samples each, starting at the first mark.
fn of_levels(m: Modulation, dev: f64, levels: &[u8], bit: f32) -> Train {
    let mut t = Train::new(m, 1e6);
    t.deviation_hz = dev;
    sdrkit_proto_ookdev::encode::nrz(levels, bit, &mut t);
    t
}

/// The shortest run of the train, the width one level stands for.
fn unit(t: &Train) -> f32 {
    t.pulses
        .iter()
        .flat_map(|p| [p.mark, p.space])
        .filter(|&x| x > 0.0)
        .fold(f32::MAX, f32::min)
}

/// Flipping any single level of a checked device's train never produces a reading of another device or another value: it
/// produces nothing, or the reading of the unchanged frame (a flip outside the checked bits).
#[test]
fn a_flipped_level_never_returns_a_different_reading() {
    for d in TestDevice::ALL.into_iter().filter(|d| d.checked()) {
        let t = device_train(d, 1);
        let good = decode(&t);
        let bit = unit(&t);
        let lv = levels(&t, bit);
        let step = (lv.len() / 97).max(1);
        for i in (0..lv.len()).step_by(step) {
            let mut l = lv.clone();
            l[i] ^= 1;
            for r in decode(&of_levels(t.modulation, t.deviation_hz, &l, bit)) {
                assert!(
                    good.iter().any(|g| g.same_as(&r)),
                    "{d:?}, level {i}: {r:#?}"
                );
            }
        }
    }
}

/// A repeat protocol with a single copy of its frame produces no reading.
#[test]
fn a_repeat_protocol_needs_its_copies() {
    // Nexus sends 12 copies and needs 3.
    let t = device_train(TestDevice::Nexus, 0);
    let mut one = t.clone();
    one.pulses.truncate(37);
    assert!(decode(&one).is_empty(), "one Nexus copy");
    // EV1527 sends 6 copies and needs 2.
    let t = device_train(TestDevice::Ev1527, 0);
    let mut one = t.clone();
    one.pulses.truncate(25);
    assert!(decode(&one).is_empty(), "one EV1527 copy");
}

#[test]
fn line_code_guess() {
    let mut g = Guesser::default();
    let x = g.guess(&device_train(TestDevice::Ev1527, 0));
    assert_eq!(x.code, LineCode::Pwm);
    assert!(
        (x.short_us - 464.0).abs() < 5.0 && (x.long_us - 1404.0).abs() < 10.0,
        "{x:?}"
    );
    let x = g.guess(&device_train(TestDevice::Nexus, 0));
    assert_eq!(x.code, LineCode::Ppm, "{x:?}");
    assert!(
        (x.short_us - 1000.0).abs() < 10.0 && (x.long_us - 2000.0).abs() < 20.0,
        "{x:?}"
    );
    let x = g.guess(&device_train(TestDevice::F007th, 0));
    assert_eq!(x.code, LineCode::Manchester, "{x:?}");
    assert!((x.short_us - 500.0).abs() < 10.0, "{x:?}");
}

/// The trains `ism.pulses` makes from a burst of `x` at 500 ksps, with 2 ms of quiet before it at noise `sigma`.
fn trains(x: &[Complex32], sigma: f32) -> Vec<Train> {
    let rate = 500e3;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let lead = 1000;
    let mut s = vec![Complex32::default(); lead];
    s.extend_from_slice(x);
    s.extend(vec![Complex32::default(); 2000]);
    for v in &mut s {
        *v += rng.complex_normal((sigma * sigma) as f64);
    }
    let b = IsmBurst {
        samples: s,
        rate,
        offset_hz: 0.0,
        start: 0,
        input_rate: rate,
        lead,
    };
    let mut out = Vec::new();
    Pulses::default().run(&b, |t| out.push(t));
    out
}

/// The last space of an OOK train is the train's own, and slicers read it as the end.
#[test]
fn an_ook_train_ends_with_its_own_last_space() {
    let x = train_samples(&device_train(TestDevice::Ev1527, 0), 500e3, 0.0);
    let t = trains(&x, 0.01);
    assert_eq!(t.len(), 1);
    let last = t[0].pulses.last().unwrap();
    assert!(t[0].us(last.space) < 100_000.0, "{}", t[0].us(last.space));
    assert_eq!(t[0].space_at(t[0].pulses.len() - 1), f32::INFINITY);
    assert_eq!(decode(&t[0]).len(), 1);
}

/// A strong and a weak 500 µs pulse measure the same (the 20 µs average of the narrow OOK channel lengthens both alike).
#[test]
fn strong_and_weak_pulses_measure_the_same() {
    let mut pt = Train::new(Modulation::Ook, 1e6);
    for _ in 0..8 {
        pt.pulses.push(Pulse::new(500.0, 1000.0));
    }
    let clean = train_samples(&pt, 500e3, 0.0);
    let mut width = Vec::new();
    for gain in [1.0f32, 0.05] {
        let x: Vec<Complex32> = clean.iter().map(|&v| v * gain).collect();
        let t = trains(&x, 0.005);
        assert_eq!(t.len(), 1);
        let w = t[0].pulses[2..6]
            .iter()
            .map(|p| t[0].us(p.mark))
            .sum::<f32>()
            / 4.0;
        width.push(w);
    }
    assert!(
        (width[0] - 500.0).abs() < 40.0 && (width[1] - 500.0).abs() < 40.0,
        "{width:?}"
    );
    assert!((width[0] - width[1]).abs() < 12.0, "{width:?}");
}

/// A carrier with no tones produces no FSK train.
#[test]
fn a_plain_carrier_is_not_fsk() {
    let x = vec![Complex32::new(0.5, 0.0); 5000];
    for t in trains(&x, 0.01) {
        assert_ne!(t.modulation, Modulation::Fsk, "{t:?}");
    }
    // The same length of real FSK is FSK.
    let f = train_samples(&device_train(TestDevice::Bresser6, 0), 500e3, 0.0);
    let t = trains(&f, 0.01);
    assert!(
        t.iter()
            .any(|t| t.modulation == Modulation::Fsk && t.deviation_hz > 20e3),
        "{t:?}"
    );
}
