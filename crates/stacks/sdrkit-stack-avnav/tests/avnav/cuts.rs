//! Any cut of the stream produces the same results: each stack is fed the same signal in pieces of different
//! sizes.

use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_avnav::ils::GEN_MARKER_STOP;
use sdrkit_stack_avnav::{DGPS, DME, ILS, LORAN, NDB, VOR};

fn feed(rx: &mut dyn Receiver, x: &[Complex32], sizes: &[usize]) -> Vec<(String, String)> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    let mut i = 0;
    let mut k = 0;
    while i < x.len() {
        let n = sizes[k % sizes.len()].min(x.len() - i);
        out.clear();
        rx.feed(&x[i..i + n], &mut out);
        ev.extend(out.events.iter().map(|e| {
            (
                e.kind().to_string(),
                serde_json::to_string(&e.fields).unwrap(),
            )
        }));
        i += n;
        k += 1;
    }
    ev
}

fn same_whatever_the_cuts(d: &'static StackDescriptor, rate: f64, freq: f64, seconds: f64) {
    let h = crate::common::noisy(d, 70.0)
        .rate(rate)
        .freq(freq)
        .seconds(seconds)
        .seed(3);
    same_for(&h, d, rate, freq);
}

/// The results and events of `h` at `rate` and `freq` are the same for every cut.
fn same_for(h: &Harness, d: &StackDescriptor, rate: f64, freq: f64) {
    let x = h.generate().unwrap();
    let mut results = Vec::new();
    for sizes in [
        &[65_536usize][..],
        &[1, 7, 999, 4096, 13][..],
        &[333_333][..],
    ] {
        let mut rx = h.receiver().unwrap();
        rx.configure(&InputInfo::new(rate, freq, freq).recording())
            .unwrap();
        let ev = feed(rx.as_mut(), &x, sizes);
        results.push((ev, serde_json::to_value(rx.results()).unwrap()));
    }
    assert!(
        !results[0].1["fields"].as_object().unwrap().is_empty(),
        "{}",
        d.id
    );
    for r in &results[1..] {
        assert_eq!(r.0, results[0].0, "{}: events differ", d.id);
        assert_eq!(r.1, results[0].1, "{}: results differ", d.id);
    }
}

#[test]
fn ndb_cuts() {
    same_whatever_the_cuts(&NDB, 64e3, 344e3, 6.0);
}

#[test]
fn vor_cuts() {
    same_whatever_the_cuts(&VOR, 96e3, 113.1e6, 5.0);
}

#[test]
fn ils_cuts() {
    same_whatever_the_cuts(&ILS, 64e3, 110.3e6, 4.0);
}

#[test]
fn ils_marker_cuts() {
    let h = crate::common::noisy(&ILS, 70.0)
        .submode("marker")
        .rate(48e3)
        .freq(75e6)
        .seconds(5.0)
        .seed(3)
        .set(&GEN_MARKER_STOP, 3.0);
    same_for(&h, &ILS, 48e3, 75e6);
}

#[test]
fn dme_cuts() {
    same_whatever_the_cuts(&DME, 2e6, 978e6, 2.0);
}

#[test]
fn loran_cuts() {
    same_whatever_the_cuts(&LORAN, 100e3, 100e3, 3.0);
}

#[test]
fn dgps_cuts() {
    same_whatever_the_cuts(&DGPS, 16e3, 304e3, 6.0);
}
