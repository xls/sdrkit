//! The status of `cell` and `nr` when nothing shows, and the band search of `nr`.

use crate::common::*;
use sdrkit_core::{Complex32, HostRequest, InputInfo, Output, Owner, Receiver};
use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
use sdrkit_stack_cell::{CELL, NR};
use serde_json::Value as J;

/// Complex noise of standard deviation `sigma` a part, `seconds` long at `rate`.
fn noise(rate: f64, seconds: f64, sigma: f64, seed: u64) -> Vec<Complex32> {
    let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..(rate * seconds) as usize).map(|_| r.complex_normal(2.0 * sigma * sigma)).collect()
}

/// The status summary of a fresh stack of `desc` (band search off) over `x` at `rate`, tuned to `hz`.
fn summary(desc: &sdrkit_core::StackDescriptor, rate: f64, hz: f64, x: &[Complex32]) -> String {
    let mut rx = stack_graph(desc, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(rate, hz, hz).recording(), x, &[]);
    Owner::results(&rx).status().summary.clone()
}

#[test]
fn lte_on_noise_says_there_is_no_carrier() {
    let s = summary(&CELL, 2.56e6, 778e6, &noise(2.56e6, 0.6, 0.05, 1));
    assert!(s.contains("no LTE carrier on this frequency"), "{s}");
}

#[test]
fn a_signal_in_the_lowest_steps_of_the_converter_is_too_weak() {
    // An 8-bit radio whose samples move by one or two steps of 1/128 only.
    let x: Vec<Complex32> = noise(2.56e6, 0.6, 0.006, 2)
        .into_iter()
        .map(|v| Complex32::new((v.re * 128.0).round() / 128.0, (v.im * 128.0).round() / 128.0))
        .collect();
    let s = summary(&CELL, 2.56e6, 778e6, &x);
    assert!(s.contains("too weak at the radio"), "{s}");
    let s = summary(&NR, 7.68e6, 3550e6, &noise(7.68e6, 0.2, 0.05, 3));
    assert!(!s.contains("too weak at the radio"), "{s}");
}

#[test]
fn nr_says_when_the_capture_is_too_narrow_for_its_blocks() {
    let s = summary(&NR, 4e6, 3550e6, &noise(4e6, 0.3, 0.05, 4));
    assert!(s.contains("too narrow for a 30 kHz SSB"), "{s}");
    let s = summary(&NR, 20e6, 3550e6, &noise(20e6, 0.1, 0.05, 5));
    assert!(s.contains("no SSB on the"), "{s}");
    assert!(!s.contains("too narrow"), "{s}");
}

/// Tuned at the edge of n28 with nothing there, `nr` hops over the sync raster of n28, finds the cell of scene 1 and stays
/// on it after the pass. The test plays the host.
#[test]
fn nr_band_search_finds_and_stays() {
    let rate = 7.68e6;
    let (cell_hz, cells) = scene(1);
    let mut rx = stack_graph(&NR, &[("search_dwell_s", J::from(0.1))]);
    let mut centre = 760.0e6;
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    let mut out = Output::default();
    let piece = (0.02 * rate) as usize;
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut stayed = None;
    let (mut fed, mut step) = (0.0, 0.0);
    let gen_at = |c: f64| {
        let v = gen_values(&[("gen_offset_hz", J::from(cell_hz - c))]);
        sdrkit_components::sim::impaired(Box::new(NrGenerator::new(rate, cells.clone(), NrGenOptions::default())), &v)
            .expect("the offset")
    };
    let mut g = gen_at(centre);
    let mut r = sdrkit_algo_sim::rng::Rng::new(9);
    let mut buf = vec![Complex32::default(); piece];
    while stayed.is_none() {
        assert!(fed < 40.0, "no stay after {fed} s");
        g.generate(&mut buf);
        let near = (cell_hz - centre).abs() < rate / 2.0 - 2.2e6;
        for v in buf.iter_mut() {
            if !near {
                *v = Complex32::default();
            }
            *v += r.complex_normal(2.0 * 0.01 * 0.01);
        }
        rx.feed(&buf, &mut out);
        fed += 0.02;
        step += 0.02;
        let mut next = None;
        for q in out.requests.drain(..) {
            match q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, .. } => {
                    hop = None;
                    stayed = Some(tuned_hz);
                    next = Some(tuned_hz);
                }
                other => panic!("unexpected request {other:?}"),
            }
        }
        out.clear();
        if next.is_none() {
            if let Some((ch, dwell, at)) = hop.as_mut() {
                if step >= *dwell {
                    *at = (*at + 1) % ch.len();
                    next = Some(ch[*at]);
                }
            }
        }
        if let Some(n) = next {
            centre = n;
            step = 0.0;
            g = gen_at(centre);
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    let pcis: Vec<i64> = cells_of(&rx);
    assert_eq!(pcis, [222]);
    assert!((stayed.unwrap() - cell_hz).abs() < 10e3, "{stayed:?} for {cell_hz}");
}

fn cells_of(rx: &sdrkit_graph::StackGraph) -> Vec<i64> {
    cells(&state(rx)).iter().filter_map(|c| c["pci"].as_i64()).collect()
}
