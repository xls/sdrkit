//! The cell stacks fed at the pace of a live radio: no call of the feed holds the samples of the radio up for longer than
//! the queue of the engine holds. The tests measure wall time, so they run only when asked for (`--ignored`), alone.
//!
//! A stack keeps up when it drops no sample and the process needs less than [`CPU_MOST`] seconds of processor time for a
//! second of signal. Other work on the machine takes the processor away and makes drops, and it makes the processor time
//! longer too, though less. So a run with drops is made again ([`TRIES`]), and the least processor time of the runs
//! counts: a stack that is too slow drops in every run. When more work is ready to run than the machine has processors
//! ([`machine_is_full`]), drops tell nothing of the stack: the test says so and judges the processor time alone.

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
/// The most processor time of the process for a second of signal while a stack follows cells: the search has one thread,
/// and needs room on it.
const CPU_MOST: f64 = 0.8;
/// The runs made until one shows no drop.
const TRIES: usize = 3;

/// The processor time that the process has used, seconds. None where the system does not tell.
fn cpu_s() -> Option<f64> {
    #[cfg(unix)]
    {
        let mut u = std::mem::MaybeUninit::<libc::rusage>::zeroed();
        // SAFETY: getrusage fills the struct that the pointer names, and the struct lives until the call returns.
        let ok = unsafe { libc::getrusage(libc::RUSAGE_SELF, u.as_mut_ptr()) } == 0;
        // SAFETY: the call above filled the struct, and a zeroed rusage is a valid value too.
        let u = unsafe { u.assume_init() };
        let s = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 * 1e-6;
        ok.then(|| s(u.ru_utime) + s(u.ru_stime))
    }
    #[cfg(not(unix))]
    None
}

/// Whether more work was ready to run in the last minute than the machine has processors. False where the system does
/// not tell.
fn machine_is_full() -> bool {
    #[cfg(unix)]
    {
        let mut load = [0f64; 1];
        // SAFETY: getloadavg writes at most one value into the array of one.
        let n = unsafe { libc::getloadavg(load.as_mut_ptr(), 1) };
        let cores = std::thread::available_parallelism().map_or(1, |c| c.get());
        n == 1 && load[0] > cores as f64
    }
    #[cfg(not(unix))]
    false
}

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
    /// The samples that the stack wanted and did not take, because its search was late (`dropped`), counted since the
    /// stack was built.
    dropped: f64,
    /// The samples that a band search left out between looks, on purpose (`skipped`), counted since the stack was built.
    skipped: f64,
    /// `dropped` when the band search started.
    dropped_before_pass: Option<f64>,
    /// The processor time of the process for a second of signal. None where the system does not tell.
    cpu: Option<f64>,
    /// The retunes the run made.
    retunes: usize,
    /// The steps searched longer for a sync signal.
    longer: usize,
    /// The radio centres of the captures searched, in order, each once.
    searched: Vec<f64>,
    /// Seconds of signal at the end of the first pass of a band search.
    pass_s: Option<f64>,
    /// The radio centre at the end, Hz.
    centre: f64,
    /// The longest signal time for which the host hopped while the stack showed no band search, seconds.
    hop_left_s: f64,
}

/// Feeds `rx` the pieces that `next(centre)` gives (a block each) at the pace of the signal for `seconds`, as a
/// radio and an engine with a queue of [`QUEUE`] samples do. Follows the hops and retunes that the stack asks for when
/// `follow`, each move taking [`MOVE_S`] without samples, and ends after the first pass of a band search.
fn paced(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    centre: f64,
    seconds: f64,
    follow: bool,
    next: impl FnMut(f64) -> Vec<Complex32>,
) -> Paced {
    paced_until(rx, rate, centre, seconds, follow, next, |_| false)
}

/// [`paced`], ending also when `done` holds for the results of the stack.
fn paced_until(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    mut centre: f64,
    seconds: f64,
    follow: bool,
    mut next: impl FnMut(f64) -> Vec<Complex32>,
    mut done: impl FnMut(&sdrkit_core::Results) -> bool,
) -> Paced {
    let mut p = Paced::default();
    let mut out = Output::default();
    let start = Instant::now();
    let cpu0 = cpu_s();
    let (mut lost, mut total) = (0usize, 0usize);
    let mut hop: Option<(Vec<f64>, f64, usize)> = None;
    let mut step_s = 0.0;
    let mut hop_left_s = 0.0;
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
        if p.dropped_before_pass.is_none() && value(r, "search_steps").is_some() {
            p.dropped_before_pass = value(r, "dropped");
        }
        if value(r, "search_passes").unwrap_or(0.0) >= 1.0 {
            p.pass_s = Some(p.fed_s);
            break;
        }
        p.fed_s += piece_s;
        if done(r) {
            total += x.len();
            break;
        }
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
        // A search that ended takes its hop back with the next input.
        if hop.is_some() && value(Owner::results(&*rx), "search_steps").is_none() {
            hop_left_s += piece_s;
            p.hop_left_s = p.hop_left_s.max(hop_left_s);
        } else {
            hop_left_s = 0.0;
        }
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
    p.centre = centre;
    assert!(p.hop_left_s < 0.5, "the host hops after the band search ended: {p:?}");
    p.lost = lost as f64 / total.max(1) as f64;
    p.dropped = value(Owner::results(&*rx), "dropped").unwrap_or(0.0);
    p.skipped = value(Owner::results(&*rx), "skipped").unwrap_or(0.0);
    p.cpu = cpu0.zip(cpu_s()).map(|(a, b)| (b - a) / p.fed_s.max(1e-9));
    p
}

/// Feeds `rx` the pieces of `next` at the pace of the signal for `seconds` on `centre`, again while a run shows drops, at
/// most [`TRIES`] runs. Returns the newest run, with the least processor time of the runs, and whether it showed no drop
/// (true also when the drops cannot be judged, see [`machine_is_full`]).
fn follows(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    centre: f64,
    seconds: f64,
    mut next: impl FnMut(f64) -> Vec<Complex32>,
) -> (Paced, bool) {
    let mut before = value(Owner::results(&*rx), "dropped").unwrap_or(0.0);
    let mut p = Paced::default();
    let mut cpu: Option<f64> = None;
    for _ in 0..TRIES {
        p = paced(rx, rate, centre, seconds, true, &mut next);
        cpu = p.cpu.map(|c| cpu.map_or(c, |least| least.min(c)));
        p.cpu = cpu;
        if p.dropped == before {
            return (p, true);
        }
        eprintln!("a run with {} samples dropped is made again: {p:?}", p.dropped - before);
        before = p.dropped;
    }
    if machine_is_full() {
        eprintln!("not judged by its drops: the machine has more work than processors");
        return (p, true);
    }
    (p, false)
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
        "{} {what}: {:.1} s fed, lost {:.1} %, in the feed {:.1} % of the time, longest feed {:.0} ms, dropped {:.0} samples, \
         skipped between looks {:.0} samples, processor {:.0} % of the signal time, {} retunes",
        desc.id,
        p.fed_s,
        100.0 * p.lost,
        100.0 * p.busy_s / p.fed_s,
        1e3 * p.longest_s,
        p.dropped,
        p.skipped,
        100.0 * p.cpu.unwrap_or(f64::NAN),
        p.retunes
    );
}

/// A 10 MHz LTE carrier at 12 Msps fed live with the default settings: once the cell and its system information are
/// read, every sample is searched (none dropped, none skipped) for 35 s, which holds one more read of the system
/// information.
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
    let mut gen = cycle(x);
    let first = paced(&mut rx, rate, hz, 3.0, true, &mut gen);
    assert_eq!(value(Owner::results(&rx), "cell_count"), Some(1.0), "{first:?}");
    let (p, kept_up) = follows(&mut rx, rate, hz, 35.0, &mut gen);
    report("on a carrier", &CELL, &p);
    assert_eq!(p.lost, 0.0, "{p:?}");
    assert!(kept_up, "{p:?}");
    assert_eq!(p.skipped, 0.0, "{p:?}");
    assert!(p.cpu.is_none_or(|c| c < CPU_MOST), "{p:?}");
    rx.finish(&mut Output::default());
    assert_eq!(cells(&state(&rx)).len(), 1);
    assert!(cells(&state(&rx))[0]["sib1"].is_object(), "{}", cells(&state(&rx))[0]);
}

/// The settings that a host gives `desc` in Australia.
fn australia(desc: &StackDescriptor) -> (Vec<(String, J)>, f64) {
    let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let p = data.preset(desc.id, &au, &sdrkit_presets::Options::default()).unwrap();
    let keys = p
        .settings
        .iter()
        .map(|(k, v)| match v {
            sdrkit_core::Value::Text(t) => (k.clone(), J::from(t.as_str())),
            other => panic!("{k}: {other:?}"),
        })
        .collect();
    (keys, p.frequency_hz)
}

/// Feeds `rx` at the pace of the signal with the Australian settings from `start` until a cell is listed, then until
/// `operators` cells show their operator, then 15 s more. Asserts that the operators show within 5 s of the first cell and
/// that from 2 s after the first cell nothing is dropped. Returns the radio centre at the end.
fn reads_operators_live(
    rx: &mut sdrkit_graph::StackGraph,
    rate: f64,
    start: f64,
    operators: f64,
    mut next: impl FnMut(f64) -> Vec<Complex32>,
) -> f64 {
    let found = paced_until(rx, rate, start, 60.0, true, &mut next, |r| value(r, "cell_count").unwrap_or(0.0) >= 1.0);
    report("until the first cell", &CELL, &found);
    assert!(value(Owner::results(&*rx), "cell_count").unwrap_or(0.0) >= 1.0, "{found:?}");
    let read = paced_until(rx, rate, found.centre, 5.0, true, &mut next, |r| {
        value(r, "identified").unwrap_or(0.0) >= operators
    });
    report("until the operators", &CELL, &read);
    let r = Owner::results(&*rx);
    assert_eq!(value(r, "identified"), Some(operators), "{:?} {read:?}", r.status().summary);
    eprintln!("operators read {:.1} s after the first cell", read.fed_s);
    let settle = paced(rx, rate, read.centre, (2.0 - read.fed_s).max(0.0), true, &mut next);
    let (p, kept_up) = follows(rx, rate, settle.centre, 15.0, &mut next);
    report("on the carrier", &CELL, &p);
    assert!(kept_up, "{p:?}");
    let r = Owner::results(&*rx);
    assert_eq!(text(r, "quiet_reason"), None, "{:?}", r.status().summary);
    p.centre
}

/// The system information messages read of every cell listed, by row key.
fn si_messages(r: &sdrkit_core::Results) -> Vec<(String, i64)> {
    use sdrkit_stack_cell::typed::{CellDetailRow, CELL_DETAILS};
    CELL_DETAILS.rows::<CellDetailRow>(r).map(|(key, row)| (key.to_owned(), row.si_messages)).collect()
}

/// LTE live at 12 Msps with the Australian settings, started on a frequency without a cell: the band search finds two
/// cells of one 10 MHz carrier on the known carrier at 763 MHz and stays there. The operators of both cells show within
/// 5 s of the first cell, and from 2 s after it nothing is dropped. Once their system information is read, a cell is
/// read again only after 30 s: 15 s more add no message.
#[test]
#[ignore]
fn lte_auto_stays_on_a_carrier_and_reads_its_operators_without_drops() {
    use sdrkit_stack_cell::lte::gen::{scene, LteGenOptions, LteGenerator};
    let rate = 12e6;
    let cell_hz = 763e6;
    let mut parts = Vec::new();
    for (k, gain) in [(0u16, 0.0), (1, -6.0)] {
        let mut c = scene(0, None);
        c.pci += 3 * k + 1;
        c.gain_db = gain;
        parts.push(impaired(LteGenerator::new(rate, c, LteGenOptions { seed: 1 + u64::from(k), ..Default::default() }), &[], 1.0));
    }
    let carrier: Vec<Complex32> = (0..parts[0].len()).map(|i| parts[0][i] + parts[1][i]).collect();
    let mut r = sdrkit_algo_sim::rng::Rng::new(4);
    let mut at = 0;
    let next = move |centre: f64| {
        let mut x: Vec<Complex32> = (0..BLOCK).map(|_| r.complex_normal(2.0 * 0.02 * 0.02)).collect();
        if (centre - cell_hz).abs() < 1.0 {
            for v in x.iter_mut() {
                *v += carrier[at];
                at = (at + 1) % carrier.len();
            }
        }
        x
    };
    let (keys, start) = australia(&CELL);
    let keys: Vec<(&str, J)> = keys.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut rx = stack_graph(&CELL, &keys);
    rx.configure(&InputInfo::new(rate, start, start)).unwrap();
    let mut next = next;
    assert_eq!(reads_operators_live(&mut rx, rate, start, 2.0, &mut next), cell_hz);
    let before = si_messages(Owner::results(&rx));
    paced(&mut rx, rate, cell_hz, 15.0, true, &mut next);
    let after = si_messages(Owner::results(&rx));
    eprintln!("system information messages {before:?}, 15 s later {after:?}");
    assert_eq!(before.len(), 2, "{before:?}");
    assert_eq!(before, after);
}

/// The recording of a live 10 MHz band 28 carrier fed live, again and again, with the Australian settings on its centre:
/// its operator 505-02 shows within 5 s of the cell, and from 2 s after the cell nothing is dropped. Skipped when the
/// recording is not on this machine.
#[test]
#[ignore]
fn lte_auto_reads_the_operator_of_a_live_recording() {
    let Some(rec) = crate::search::live_recording("sdrkit_cell_763.000MHz_11.52Msps_20261126-132305") else { return };
    let (rate, hz) = (rec.rate_hz, rec.centre_hz);
    let (keys, _) = australia(&CELL);
    let keys: Vec<(&str, J)> = keys.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut rx = stack_graph(&CELL, &keys);
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    reads_operators_live(&mut rx, rate, hz, 1.0, cycle(rec.samples));
    rx.finish(&mut Output::default());
    let cs = cells(&state(&rx));
    assert_eq!(cs[0]["sib1"]["plmns"][0], "505-02", "{}", cs[0]);
}

/// The recording of a real LTE cell fed live, again and again for 20 s: the cell shows SIB1 to SIB3 and no SIB that it does
/// not show when the recording waits for the search, and once the cell is followed nothing is dropped. Skipped when the
/// recordings are not on this machine.
#[test]
#[ignore]
fn a_live_cell_shows_the_sibs_of_its_recording() {
    let Some(rec) = crate::search::live_recording("sdrkit_cell_763.000MHz_11.52Msps_20261126-132305") else { return };
    let (rate, hz) = (rec.rate_hz, rec.centre_hz);
    let seconds = rec.samples.len() as f64 / rate;
    let sibs = |rx: &sdrkit_graph::StackGraph| -> Vec<i64> {
        let all = cells(&state(rx));
        let c = all.iter().find(|c| c["sib1"].is_object()).expect("a cell with SIB1");
        c["sibs_seen"].as_array().expect("the SIBs seen").iter().filter_map(J::as_i64).collect()
    };
    let mut waits = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    waits.configure(&InputInfo::new(rate, hz, hz).recording()).unwrap();
    for x in rec.samples.chunks(BLOCK) {
        waits.feed(x, &mut Output::default());
    }
    waits.finish(&mut Output::default());
    let mut live = stack_graph(&CELL, &[("band_search", J::from("off"))]);
    live.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let mut gen = cycle(rec.samples);
    let first = paced(&mut live, rate, hz, 8.0, true, &mut gen);
    report("on a recording, first", &CELL, &first);
    let (p, kept_up) = follows(&mut live, rate, hz, 12.0, &mut gen);
    report("on a recording", &CELL, &p);
    live.finish(&mut Output::default());
    let (whole, seen) = (sibs(&waits), sibs(&live));
    eprintln!("a recording of {seconds:.1} s: SIBs {whole:?}, live {seen:?}");
    assert!(kept_up, "{p:?}");
    assert!(seen.iter().all(|n| whole.contains(n)) && [1, 2, 3].iter().all(|n| seen.contains(n)), "{seen:?} {whole:?}");
}

/// One n28 cell, and three cells on three raster points, at 12 Msps fed live with the default settings: once they are
/// listed, every sample is searched (none dropped, none skipped) for 20 s, and the SIB1 of the first cell is read.
#[test]
#[ignore]
fn nr_follows_a_live_cell_at_12_msps_in_real_time() {
    use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
    let rate = 12e6;
    for n in [1u16, 3] {
        let (hz, mut scene_cells) = scene(1);
        // The others on the raster points 2.4 MHz below and above.
        for (k, off) in [(1u16, -2.4e6), (2, 2.4e6)].into_iter().take(usize::from(n) - 1) {
            let mut c = scene_cells[0].clone();
            c.pci += 50 * k;
            c.offset_hz = off;
            c.ssb_hz += off;
            c.sib1 = None;
            scene_cells.push(c);
        }
        let x = impaired(NrGenerator::new(rate, scene_cells, NrGenOptions::default()), &snr(15.0), 1.0);
        let mut rx = stack_graph(&NR, &[]);
        rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
        let mut gen = cycle(x);
        let first = paced(&mut rx, rate, hz, 3.0, true, &mut gen);
        assert_eq!(value(Owner::results(&rx), "cell_count"), Some(f64::from(n)), "{first:?} {:?}", Owner::results(&rx).status().summary);
        let (p, kept_up) = follows(&mut rx, rate, hz, 20.0, &mut gen);
        report(&format!("on {n} cells"), &NR, &p);
        assert_eq!(p.lost, 0.0, "{p:?}");
        assert!(kept_up, "{n} cells: {p:?}");
        assert_eq!(p.skipped, 0.0, "{n} cells: {p:?}");
        assert!(p.cpu.is_none_or(|c| c < CPU_MOST), "{n} cells: {p:?}");
        rx.finish(&mut Output::default());
        let cs = cells(&state(&rx));
        assert_eq!(cs.len(), usize::from(n), "{cs:#?}");
        assert!(cs.iter().any(|c| c["sib1"].is_object()), "{cs:#?}");
    }
}

/// One pass of the band search over the Australian bands at 12 Msps, live on noise, with 0.1 s for each move of the radio:
/// under two minutes for `nr` and under 40 s for `cell`, and every step searched. What the search leaves out between
/// looks is no drop: nothing counts as dropped once the search runs.
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
        assert_eq!(Some(p.dropped), p.dropped_before_pass, "{}: {p:?}", desc.id);
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

/// One second at 12 Msps of the n28 cell of NR scene 1 as a strong cell of a carrier with little traffic shows on the
/// air: the blocks and the SIB1 35 dB over the noise, and short bursts of other transmissions of the carrier, which pass
/// the PSS threshold as the blocks do. Returns the radio centre and the samples.
fn nr_cell_with_bursts() -> (f64, Vec<Complex32>) {
    use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
    let rate = 12e6;
    let (hz, scene_cells) = scene(1);
    let quiet = NrGenOptions { load: false, ..Default::default() };
    let mut x = impaired(NrGenerator::new(rate, scene_cells, quiet), &snr(35.0), 1.0);
    let power = x.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / x.len() as f64;
    let mut r = sdrkit_algo_sim::rng::Rng::new(11);
    // A burst of half a millisecond in every 2.5 ms.
    let (every, len) = ((2.5e-3 * rate) as usize, (0.5e-3 * rate) as usize);
    for (i, v) in x.iter_mut().enumerate() {
        if i % every >= every - len {
            *v += r.complex_normal(BURST_POWER * power);
        }
    }
    (hz, x)
}

/// The power of the bursts of [`nr_cell_with_bursts`] over the mean power of the cell's own signal.
const BURST_POWER: f64 = 4.0;

/// A strong NR cell with bursts of its carrier ([`nr_cell_with_bursts`]) at 12 Msps, fed live with the default settings:
/// each burst shows sync peaks that are no block, and each costs a look. Once the cell is read, nothing is dropped or
/// skipped for 20 s, and the process needs less than half a second of processor time for a second of signal.
#[test]
#[ignore]
fn nr_follows_a_strong_cell_with_bursts_at_12_msps_in_real_time() {
    let rate = 12e6;
    let (hz, x) = nr_cell_with_bursts();
    let mut rx = stack_graph(&NR, &[]);
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let mut gen = cycle(x);
    let first = paced(&mut rx, rate, hz, 3.0, true, &mut gen);
    assert_eq!(value(Owner::results(&rx), "with_sib1"), Some(1.0), "{first:?} {:?}", Owner::results(&rx).status().summary);
    let (p, kept_up) = follows(&mut rx, rate, hz, 20.0, &mut gen);
    report("on a strong cell with bursts", &NR, &p);
    assert_eq!(p.lost, 0.0, "{p:?}");
    assert!(kept_up, "{p:?}");
    assert_eq!(p.skipped, 0.0, "{p:?}");
    assert!(p.cpu.is_none_or(|c| c < 0.5), "{p:?}");
    rx.finish(&mut Output::default());
    let cs = cells(&state(&rx));
    assert!(cs.iter().any(|c| c["pci"] == 222 && c["sib1"].is_object()), "{cs:#?}");
}

/// The recording of a live n5 cell at 12 Msps (PCI 897, 505-01, a strong cell on a carrier with bursts of traffic) fed
/// live, again and again, with the Australian settings on its centre: its operator shows within 5 s, nothing is dropped
/// from the first sample on, and nothing is dropped or skipped for 20 s more, with less than half a second of processor
/// time for a second of signal. Skipped when the recording is not on this machine.
#[test]
#[ignore]
fn nr_auto_follows_the_recording_of_a_live_cell_without_drops() {
    let Some(rec) = crate::search::live_recording("sdrkit_nr_884.450MHz_12Msps_20261127-161307") else { return };
    let (rate, hz) = (rec.rate_hz, rec.centre_hz);
    let (keys, _) = australia(&NR);
    let keys: Vec<(&str, J)> = keys.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let mut rx = stack_graph(&NR, &keys);
    rx.configure(&InputInfo::new(rate, hz, hz)).unwrap();
    let mut gen = cycle(rec.samples);
    let first = paced(&mut rx, rate, hz, 5.0, true, &mut gen);
    report("on a recording, first", &NR, &first);
    assert_eq!(value(Owner::results(&rx), "with_sib1"), Some(1.0), "{first:?} {:?}", Owner::results(&rx).status().summary);
    assert_eq!(first.centre, hz, "{first:?}");
    // The first search is bounded as the later ones are: the input waiting meanwhile fits the helper's backlog.
    assert!(first.dropped == 0.0 || machine_is_full(), "{first:?}");
    let (p, kept_up) = follows(&mut rx, rate, hz, 20.0, &mut gen);
    report("on a recording", &NR, &p);
    assert!(kept_up, "{p:?}");
    assert_eq!((p.skipped, p.retunes), (first.skipped, 0), "{p:?}");
    assert!(p.cpu.is_none_or(|c| c < 0.5), "{p:?}");
    rx.finish(&mut Output::default());
    let cs = cells(&state(&rx));
    let read: Vec<&J> = cs.iter().filter(|c| c["sib1"].is_object()).collect();
    assert_eq!(read.len(), 1, "{cs:#?}");
    assert_eq!((&read[0]["pci"], &read[0]["sib1"]["networks"][0]["plmn"]), (&J::from(897), &J::from("505-01")), "{}", read[0]);
}

/// The n28 cell of NR scene 1 at 12 Msps, 3 MHz over the centre, among the last raster points that a bounded search
/// takes in turn, fed live with the Australian settings in auto: the stack stays on the centre until its raster points
/// are all searched, lists the cell with its operator, and drops nothing from the start.
#[test]
#[ignore]
fn nr_auto_finds_a_cell_on_a_late_raster_point_without_leaving() {
    use sdrkit_stack_cell::nr::gen::{scene, NrGenOptions, NrGenerator};
    let rate = 12e6;
    let (ssb_hz, mut scene_cells) = scene(1);
    let centre = ssb_hz - 3e6;
    scene_cells[0].offset_hz = 3e6;
    let x = impaired(NrGenerator::new(rate, scene_cells, NrGenOptions::default()), &snr(15.0), 1.0);
    let probe = sdrkit_stack_cell::decoder::Capture::new(vec![Complex32::default(); 1000], rate, centre, 0.0);
    let (keys, _) = australia(&NR);
    let keys: Vec<(&str, J)> = keys.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let order: Vec<u16> = keys
        .iter()
        .find(|(k, _)| *k == "band_order")
        .and_then(|(_, v)| v.as_str())
        .map(|s| s.split_whitespace().filter_map(|b| b.parse().ok()).collect())
        .unwrap_or_default();
    let (cands, _) = sdrkit_stack_cell::nr::search::candidates(&probe, &order);
    let at = cands.iter().position(|c| (c.offset_hz - 3e6).abs() < 5e3 && c.scs == 15).expect("the cell's raster point");
    assert!(3 * at >= 2 * cands.len(), "raster point {at} of {}", cands.len());
    let mut rx = stack_graph(&NR, &keys);
    rx.configure(&InputInfo::new(rate, centre, centre)).unwrap();
    let p = paced(&mut rx, rate, centre, 6.0, true, cycle(x));
    report("on a late raster point", &NR, &p);
    assert_eq!((p.centre, p.retunes), (centre, 0), "{p:?}");
    assert!(p.dropped == 0.0 || machine_is_full(), "{p:?}");
    rx.finish(&mut Output::default());
    let cs = cells(&state(&rx));
    assert!(cs.iter().any(|c| c["pci"] == 222 && c["sib1"].is_object()), "{cs:#?}");
}
