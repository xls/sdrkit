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
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    run_graph(&mut rx, InputInfo::new(2.56e6, 778e6, 778e6).recording(), &x, &[]);
    let r = Owner::results(&rx);
    assert!(r.status().summary.contains("too weak at the radio"), "{}", r.status().summary);
    let id = r.decl().field_id("quiet_reason").expect("the field");
    let why = match r.fields().value(id) {
        Some(sdrkit_core::Value::Text(t)) => t.clone(),
        other => panic!("{other:?}"),
    };
    assert!(why.contains("too weak at the radio"), "{why}");
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

/// Tuned at the edge of n28 with nothing there and told to search, `nr` hops over the sync raster of n28, finds the cell of scene 1 and stays
/// on it after the pass. The test plays the host.
#[test]
fn nr_band_search_finds_and_stays() {
    let rate = 7.68e6;
    let (cell_hz, cells) = scene(1);
    let mut rx = stack_graph(&NR, &[("search_dwell_s", J::from(0.1))]);
    let mut centre = 760.0e6;
    rx.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    command(&mut rx, "search");
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
    // At the cell the status says where the search stayed.
    let x = impaired(NrGenerator::new(rate, cells.clone(), NrGenOptions::default()), &snr(15.0), 0.2);
    rx.feed(&x, &mut out);
    let s = Owner::results(&rx).status().summary.clone();
    assert!(s.starts_with("Staying on PCI 222"), "{s}");
    assert!((stayed.unwrap() - cell_hz).abs() < 10e3, "{stayed:?} for {cell_hz}");
}

fn cells_of(rx: &sdrkit_graph::StackGraph) -> Vec<i64> {
    cells(&state(rx)).iter().filter_map(|c| c["pci"].as_i64()).collect()
}

/// On live samples a stack whose tuned frequency shows no cell for a second starts the band search by itself: a hop over the
/// steps of the band of the tuned frequency. A recording never starts one.
#[test]
fn a_quiet_live_frequency_starts_the_search() {
    for live in [true, false] {
        let rate = 2.56e6;
        let mut rx = stack_graph(&CELL, &[]);
        let input = InputInfo::new(rate, 778e6, 778e6);
        rx.configure(&if live { input } else { input.recording() }).unwrap();
        let mut out = Output::default();
        let mut hops = Vec::new();
        for p in noise(rate, 2.5, 0.05, 7).chunks(65536) {
            // Live samples arrive at about their rate; the helper thread searches as they come.
            if live {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            rx.feed(p, &mut out);
            hops.append(&mut out.requests);
            out.clear();
        }
        rx.finish(&mut out);
        hops.append(&mut out.requests);
        let steps = sdrkit_stack_cell::scan::lte_steps(28, rate);
        if live {
            assert!(
                hops.iter().any(|h| matches!(h, HostRequest::Hop { channels, .. } if *channels == steps)),
                "{hops:?}"
            );
        } else {
            assert!(hops.is_empty(), "{hops:?}");
        }
    }
}

/// Plays the host of `rx` at `rate` for `seconds` of noise from `centre`: follows each hop with its dwell and each retune.
/// Returns the requests seen and the frequency where the radio ended.
fn host_on_noise(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    mut centre: f64,
    seconds: f64,
    follow: bool,
) -> (Vec<HostRequest>, f64) {
    let mut out = Output::default();
    let piece = (0.05 * rate) as usize;
    let mut r = sdrkit_algo_sim::rng::Rng::new(11);
    let mut buf = vec![Complex32::default(); piece];
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut seen = Vec::new();
    let (mut fed, mut step) = (0.0, 0.0);
    while fed < seconds {
        buf.iter_mut().for_each(|v| *v = r.complex_normal(2.0 * 0.05 * 0.05));
        rx.feed(&buf, &mut out);
        fed += 0.05;
        step += 0.05;
        let mut next = None;
        for q in out.requests.drain(..) {
            match &q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    next = Some(channels[0]);
                    hop = Some((channels.clone(), *dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, .. } => {
                    hop = None;
                    next = Some(*tuned_hz);
                }
                _ => {}
            }
            seen.push(q);
        }
        out.clear();
        if !follow {
            continue;
        }
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
            rx.retuned(&InputInfo::new(rate, centre, centre).recording());
        }
    }
    (seen, centre)
}

/// A cell in the tables from before the search is not one to stay on: a pass that finds nothing goes on searching.
#[test]
fn a_pass_without_a_cell_does_not_stay_on_an_old_one() {
    let rate = 7.68e6;
    let c = sdrkit_stack_cell::lte::gen::scene(0, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default());
    let x = impaired(g, &snr(25.0), 0.3);
    let mut rx = stack_graph(&CELL, &[("search_bands", J::from("20")), ("search_dwell_s", J::from(0.2))]);
    rx.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    rx.feed(&x, &mut Output::default());
    assert_eq!(cells_of(&rx), [123], "the old cell");
    rx.retuned(&InputInfo::new(rate, 1000e6, 1000e6).recording());
    // The retune reaches the search with the next samples; a search asked for before them would end at it.
    rx.feed(&noise(rate, 0.05, 0.05, 12), &mut Output::default());
    command(&mut rx, "search");
    let steps = sdrkit_stack_cell::scan::lte_steps(20, rate).len();
    let (seen, _) = host_on_noise(&mut rx, rate, 1000e6, 2.4 * steps as f64, true);
    assert!(seen.iter().any(|q| matches!(q, HostRequest::Hop { .. })), "{seen:?}");
    assert!(!seen.iter().any(|q| matches!(q, HostRequest::Retune { .. })), "no stay: {seen:?}");
    let r = sdrkit_core::Owner::results(&rx);
    assert!(
        value(r, "search_passes").unwrap_or(0.0) >= 1.0,
        "{} {:?} {seen:?}",
        r.status().summary,
        value(r, "search_passes")
    );
}

/// A host that does not move the radio ends the search after a few dwells, and the status says why.
#[test]
fn a_search_the_radio_does_not_follow_ends() {
    let rate = 7.68e6;
    let mut rx = stack_graph(&CELL, &[("search_bands", J::from("20")), ("search_dwell_s", J::from(0.2))]);
    rx.configure(&InputInfo::new(rate, 800e6, 800e6).recording()).unwrap();
    command(&mut rx, "search");
    let (seen, _) = host_on_noise(&mut rx, rate, 800e6, 4.0, false);
    assert!(seen.iter().any(|q| matches!(q, HostRequest::Hop { .. })), "{seen:?}");
    let s = Owner::results(&rx).status().summary.clone();
    assert!(s.contains("did not move"), "{s}");
    assert!(!s.contains("Searching"), "{s}");
}
