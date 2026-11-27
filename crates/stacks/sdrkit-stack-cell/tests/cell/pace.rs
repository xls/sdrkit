//! The cell stacks fed at the pace of a live radio: no call of the feed holds the samples of the radio up for longer than
//! the queue of the engine holds. The tests measure wall time, so they run only when asked for (`--ignored`), alone.

use crate::common::*;
use sdrkit_core::{Complex32, HostRequest, InputInfo, Output, Owner, Receiver, StackDescriptor};
use sdrkit_stack_cell::{CELL, NR};
use serde_json::Value as J;
use std::time::{Duration, Instant};

/// The samples of one block of the engine.
const BLOCK: usize = 16_384;
/// The samples that the queue of an engine session holds: 128 blocks.
const QUEUE: usize = 128 * BLOCK;
/// The time a network radio takes to move, seconds: no samples reach the stack meanwhile.
const MOVE_S: f64 = 0.1;

/// What a run at the pace of the radio measured.
#[derive(Debug, Default)]
struct Paced {
    /// Seconds of signal fed.
    fed_s: f64,
    /// The share of the samples that a queue of [`QUEUE`] samples lost while a feed held it up.
    lost: f64,
    /// The longest call of the feed, seconds.
    longest_s: f64,
    /// The time in the feed, seconds.
    busy_s: f64,
    /// The samples that the search left out while it searched (`dropped`).
    dropped: f64,
    /// The retunes the run made.
    retunes: usize,
    /// The steps searched longer for a sync signal.
    longer: usize,
    /// The radio centres of the captures searched, in order, each once.
    searched: Vec<f64>,
    /// Seconds of signal at the end of the first pass of a band search.
    pass_s: Option<f64>,
}

/// Feeds `rx` the pieces that `next(centre)` gives (a block each) at the pace of the signal for `seconds`, as a
/// radio and an engine with a queue of [`QUEUE`] samples do. Follows the hops and retunes that the stack asks for when
/// `follow`, each move taking [`MOVE_S`] without samples, and ends after the first pass of a band search.
fn paced(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    mut centre: f64,
    seconds: f64,
    follow: bool,
    mut next: impl FnMut(f64) -> Vec<Complex32>,
) -> Paced {
    let mut p = Paced::default();
    let mut out = Output::default();
    let start = Instant::now();
    let (mut lost, mut total) = (0usize, 0usize);
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut step_s = 0.0;
    while p.fed_s < seconds {
        let x = next(centre);
        let piece_s = x.len() as f64 / rate;
        // The radio writes the piece at its time; what the queue cannot hold is lost.
        let due = start + Duration::from_secs_f64(p.fed_s + piece_s);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        } else {
            let behind = Instant::now().duration_since(due).as_secs_f64();
            if (behind * rate) as usize > QUEUE {
                lost += x.len();
                p.fed_s += piece_s;
                total += x.len();
                continue;
            }
        }
        let t0 = Instant::now();
        rx.feed(&x, &mut out);
        p.longest_s = p.longest_s.max(t0.elapsed().as_secs_f64());
        p.busy_s += t0.elapsed().as_secs_f64();
        let r = Owner::results(&*rx);
        if let Some(c) = value(r, "capture_centre").filter(|c| !p.searched.iter().any(|s| (s - c).abs() < 1.0)) {
            p.searched.push(c);
        }
        if value(r, "search_passes").unwrap_or(0.0) >= 1.0 {
            p.pass_s = Some(p.fed_s);
            break;
        }
        p.fed_s += piece_s;
        step_s += piece_s;
        total += x.len();
        let mut to = None;
        for q in out.requests.drain(..) {
            match q {
                HostRequest::Hop { channels, dwell_s, .. } => {
                    to = Some(channels[0]);
                    hop = Some((channels, dwell_s, 0));
                }
                HostRequest::Retune { tuned_hz, reason } => {
                    p.longer += usize::from(reason.contains("searched longer"));
                    hop = None;
                    to = Some(tuned_hz);
                }
                _ => {}
            }
        }
        out.clear();
        if !follow {
            continue;
        }
        if to.is_none() {
            if let Some((ch, dwell, at)) = hop.as_mut() {
                if step_s >= *dwell {
                    *at = (*at + 1) % ch.len();
                    to = Some(ch[*at]);
                }
            }
        }
        if let Some(c) = to {
            if c != centre {
                p.fed_s += MOVE_S;
            }
            centre = c;
            step_s = 0.0;
            p.retunes += 1;
            let t0 = Instant::now();
            rx.retuned(&InputInfo::new(rate, centre, centre));
            p.longest_s = p.longest_s.max(t0.elapsed().as_secs_f64());
        }
    }
    p.lost = lost as f64 / total.max(1) as f64;
    p.dropped = value(Owner::results(&*rx), "dropped").unwrap_or(0.0);
    p
}

/// Blocks of `x`, again and again.
fn cycle(x: Vec<Complex32>) -> impl FnMut(f64) -> Vec<Complex32> {
    let n = BLOCK;
    let mut at = 0;
    move |_| {
        let mut v = Vec::with_capacity(n);
        while v.len() < n {
            let take = (n - v.len()).min(x.len() - at);
            v.extend_from_slice(&x[at..at + take]);
            at = (at + take) % x.len();
        }
        v
    }
}

/// Blocks of noise.
fn noise_pieces() -> impl FnMut(f64) -> Vec<Complex32> {
    let mut r = sdrkit_algo_sim::rng::Rng::new(3);
    let x: Vec<Complex32> = (0..64 * BLOCK + 77).map(|_| r.complex_normal(2.0 * 0.02 * 0.02)).collect();
    cycle(x)
}

fn report(what: &str, desc: &StackDescriptor, p: &Paced) {
    eprintln!(
        "{} {what}: {:.1} s fed, lost {:.1} %, in the feed {:.1} % of the time, longest feed {:.0} ms, search left out {:.0} \
         samples, {} retunes",
        desc.id,
        p.fed_s,
        100.0 * p.lost,
        100.0 * p.busy_s / p.fed_s,
        1e3 * p.longest_s,
        p.dropped,
        p.retunes
    );
}

/// A 10 MHz LTE carrier at 12 Msps fed live for 20 s: the cell, its system information, and nothing lost.
#[test]
#[ignore]
fn lte_keeps_up_with_a_live_carrier_at_12_msps() {
    let rate = 12e6;
    let c = sdrkit_stack_cell::lte::gen::scene(0, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, c, Default::default());
    let x = impaired(g, &snr(25.0), 1.0);
    let mut rx = stack_graph(&CELL, &[]);
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let p = paced(&mut rx, rate, hz, 20.0, true, cycle(x));
    report("on a carrier", &CELL, &p);
    assert_eq!(p.lost, 0.0, "{p:?}");
    rx.finish(&mut Output::default());
    assert_eq!(cells(&state(&rx)).len(), 1);
    assert!(cells(&state(&rx))[0]["sib1"].is_object(), "{}", cells(&state(&rx))[0]);
}

/// One pass of the band search over the Australian bands at 12 Msps, live on noise, with 0.1 s for each move of the radio:
/// under two minutes for `nr` and under 40 s for `cell`, and every step searched.
#[test]
#[ignore]
fn an_australian_pass_at_12_msps_searches_every_step() {
    let rate = 12e6;
    for (desc, steps_of) in [
        (&NR, sdrkit_stack_cell::scan::nr_steps as fn(u16, f64, &[(f64, f64)], &[f64]) -> Vec<f64>),
        (&CELL, sdrkit_stack_cell::scan::lte_steps),
    ] {
        let order = band_row(desc.id, "band_order", "AU");
        let mut steps: Vec<f64> = Vec::new();
        for b in order.split(' ') {
            let taken = steps.clone();
            for c in steps_of(b.parse().unwrap(), rate, &[], &taken) {
                if !steps.iter().any(|x| (x - c).abs() < 1.0) {
                    steps.push(c);
                }
            }
        }
        let mut rx = stack_graph(desc, &[("band_order", J::from(order.as_str())), ("band_search", J::from("survey"))]);
        let start = 884.45e6;
        rx.configure(&InputInfo::new(rate, start, start)).unwrap();
        let p = paced(&mut rx, rate, start, 400.0, true, noise_pieces());
        report("pass", desc, &p);
        let missed: Vec<f64> =
            steps.iter().copied().filter(|c| !p.searched.iter().any(|s| (s - c).abs() < 1.0)).collect();
        eprintln!(
            "{} pass: {} steps in {:?} s, {} not searched, {} searched longer",
            desc.id,
            steps.len(),
            p.pass_s,
            missed.len(),
            p.longer
        );
        assert_eq!(p.lost, 0.0, "{}: {p:?}", desc.id);
        assert!(missed.is_empty(), "{}: {missed:?}", desc.id);
        let most_s = if desc.id == "nr" { 120.0 } else { 40.0 };
        assert!(p.pass_s.is_some_and(|t| t < most_s), "{p:?}");
    }
}

/// A full cells table costs the input thread nothing more: 40 LTE cells listed, then 10 s of live noise at 12 Msps.
#[test]
#[ignore]
fn a_full_table_keeps_the_input_thread_free() {
    let rate = 12e6;
    let c = sdrkit_stack_cell::lte::gen::scene(0, None);
    let hz = sdrkit_stack_cell::bands::earfcn_to_mhz(c.earfcn) * 1e6;
    let mut rx = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    rx.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    for pci in 1..=40 {
        let g = sdrkit_stack_cell::lte::gen::LteGenerator::new(rate, sdrkit_stack_cell::lte::gen::scene(0, Some(pci)), Default::default());
        rx.feed(&impaired(g, &snr(25.0), 0.25), &mut Output::default());
    }
    let listed = cells(&state(&rx)).len();
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let p = paced(&mut rx, rate, hz, 10.0, false, noise_pieces());
    report(&format!("with {listed} cells"), &CELL, &p);
    assert!(listed >= 30, "{listed}");
    assert_eq!(p.lost, 0.0, "{p:?}");
    assert!(p.busy_s < 0.2 * p.fed_s, "{p:?}");
}

/// A survey of n28 at 12 Msps fed live, with the n28 cell of NR scene 1 on the step that holds it and noise elsewhere, and a
/// first look of 40 ms, often too short to read the SIB1, so that the step of the cell is searched longer after the hop
/// has moved on: the cell is listed once with its SIB1, every step is searched in one pass, and the stack asks for few
/// hops.
#[test]
#[ignore]
fn a_cell_found_live_is_searched_longer_and_the_pass_goes_on() {
    use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
    let rate = 12e6;
    let (cell_hz, scene_cells) = scene(1);
    let steps = sdrkit_stack_cell::scan::nr_steps(28, rate, &[], &[]);
    let mut gen: Option<(f64, Box<dyn sdrkit_core::Generator>)> = None;
    let mut r = sdrkit_algo_sim::rng::Rng::new(5);
    let next = move |centre: f64| {
        if gen.as_ref().is_none_or(|(c, _)| *c != centre) {
            let v = gen_values(&[("gen_offset_hz", J::from(cell_hz - centre))]);
            let g = Box::new(NrGenerator::new(rate, scene_cells.clone(), NrGenOptions::default()));
            gen = Some((centre, sdrkit_components::sim::impaired(g, &v).expect("the offset")));
        }
        let mut x = vec![Complex32::default(); BLOCK];
        if let Some((_, g)) = gen.as_mut() {
            g.generate(&mut x);
        }
        let near = (cell_hz - centre).abs() < rate / 2.0 - 2.2e6;
        for v in x.iter_mut() {
            if !near {
                *v = Complex32::default();
            }
            *v += r.complex_normal(2.0 * 0.01 * 0.01);
        }
        x
    };
    let mut rx = stack_graph(
        &NR,
        &[("band_search", J::from("survey")), ("search_bands", J::from("28")), ("search_look_s", J::from(0.04))],
    );
    let start = steps[0];
    rx.configure(&InputInfo::new(rate, start, start)).unwrap();
    let p = paced(&mut rx, rate, start, 60.0, true, next);
    report("n28 survey", &NR, &p);
    rx.finish(&mut Output::default());
    let missed: Vec<f64> = steps.iter().copied().filter(|c| !p.searched.iter().any(|s| (s - c).abs() < 1.0)).collect();
    eprintln!("nr n28 pass: {} steps in {:?} s, {} searched longer, {} retunes", steps.len(), p.pass_s, p.longer, p.retunes);
    assert!(missed.is_empty(), "{missed:?}");
    assert!(p.pass_s.is_some(), "{p:?}");
    let cs = cells(&state(&rx));
    let pcis: Vec<i64> = cs.iter().filter_map(|c| c["pci"].as_i64()).collect();
    assert_eq!(pcis, [222], "{cs:#?}");
    assert!(cs[0]["sib1"].is_object(), "{}", cs[0]);
    // Each step once, and for each step searched longer the stay and the hop again from the next step.
    assert!(p.retunes <= steps.len() + 2 * p.longer + 2, "{p:?}");
}
