//! The OpenSky identifier: its test shape found consistent with noise and an offset, other shapes and noise not, and the data
//! file.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{Generator, LockState};
use sdrkit_stack_trunk::opensky::gen::{OpenSkyGenerator, Shape};
use sdrkit_stack_trunk::typed::OPENSKY_BLOCKS;
use sdrkit_stack_trunk::OPENSKY;

const DIAL: f64 = 851.0125e6;

/// `seconds` of `shape` at `rate`, the channel `channel_hz` from the centre, with noise of `sigma`.
fn signal(shape: Shape, rate: f64, channel_hz: f64, sigma: f64, seconds: f64) -> Vec<Complex32> {
    let mut g = OpenSkyGenerator::new(rate, shape, channel_hz, 4);
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * seconds) as usize];
    g.generate(&mut x);
    if sigma > 0.0 {
        add_noise(&mut x, sigma, 8);
    }
    x
}

fn verdicts(x: &[Complex32], rate: f64) -> (Vec<String>, f64, Box<dyn sdrkit_core::Receiver>) {
    let mut rx = make(&OPENSKY, &[]);
    configure(rx.as_mut(), rate, DIAL, 0.0);
    let fed = feed(rx.as_mut(), x, 16384);
    let v: Vec<String> = fed
        .events
        .iter()
        .filter(|e| e.kind() == "opensky.verdict")
        .map(|e| field(e, "verdict"))
        .collect();
    let burst = num(results(rx.as_ref()), "burst_ms");
    (v, burst, rx)
}

#[test]
fn the_opensky_shape_is_consistent() {
    let rate = 240e3;
    let (v, burst, rx) = verdicts(&signal(Shape::OpenSky, rate, 0.0, 0.01, 4.0), rate);
    assert!(
        !v.is_empty() && v.iter().all(|v| v == "consistent"),
        "{v:?}"
    );
    assert!((burst - 30.0).abs() < 2.0, "burst {burst} ms");
    assert_eq!(results(rx.as_ref()).status().lock_state, LockState::Locked);
    assert!(!rows(results(rx.as_ref()), &OPENSKY_BLOCKS).is_empty());
    let (v, _, _) = verdicts(&signal(Shape::OpenSky, rate, 900.0, 0.03, 4.0), rate);
    assert!(
        !v.is_empty() && v.iter().all(|v| v == "consistent"),
        "{v:?}"
    );
}

#[test]
fn other_shapes_and_noise_are_not() {
    let rate = 240e3;
    for shape in [Shape::P25Like, Shape::EdacsLike, Shape::Baud3600] {
        let (v, _, rx) = verdicts(&signal(shape, rate, 0.0, 0.01, 4.0), rate);
        assert!(v.iter().all(|v| v != "consistent"), "{shape:?}: {v:?}");
        assert_ne!(results(rx.as_ref()).status().lock_state, LockState::Locked);
    }
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 4.0) as usize];
    add_noise(&mut x, 0.05, 2);
    let (v, _, rx) = verdicts(&x, rate);
    assert!(v.is_empty(), "{v:?}");
    let r = results(rx.as_ref());
    assert!(num(r, "rx_state") <= 1.0);
    assert!(text(r, "note").contains("Not confirmed by a public document"));
}

#[test]
fn the_data_file() {
    let file = crate::ermes::plan_file("opensky.json");
    let bands = &file.groups[0].bands;
    assert_eq!(bands.len(), 1);
    assert_eq!((bands[0].start_hz, bands[0].end_hz), (30e6, 1000e6));
}

#[test]
fn rate_retune_clear_and_saved_state() {
    let rate = 240e3;
    let mut rx = make(&OPENSKY, &[]);
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(48e3, DIAL, DIAL))
        .is_err());
    let (_, _, mut rx) = verdicts(&signal(Shape::OpenSky, rate, 0.0, 0.01, 3.0), rate);
    let n = rows(results(rx.as_ref()), &OPENSKY_BLOCKS).len();
    assert!(n > 0);
    let saved = rx.save_state().expect("a saved state");
    rx.retuned(&sdrkit_core::InputInfo::new(rate, DIAL + 25e3, DIAL + 25e3).recording());
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 24_000], 8192);
    assert_eq!(rows(results(rx.as_ref()), &OPENSKY_BLOCKS).len(), n);
    invoke(rx.as_mut(), "clear");
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 24_000], 8192);
    assert!(rows(results(rx.as_ref()), &OPENSKY_BLOCKS).is_empty());
    let mut back = make(&OPENSKY, &[]);
    configure(back.as_mut(), rate, DIAL, 0.0);
    back.restore_state(&saved).expect("the state is taken");
    feed(back.as_mut(), &vec![Complex32::new(0.0, 0.0); 24_000], 8192);
    assert_eq!(rows(results(back.as_ref()), &OPENSKY_BLOCKS).len(), n);
}
