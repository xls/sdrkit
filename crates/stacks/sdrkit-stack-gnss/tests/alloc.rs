//! Allocations on the receive path, counted by an allocator that counts only on the thread that enables it, so other tests that
//! run at the same time do not change the count.

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver, StackConfig, Value, Values};
use sdrkit_stack_gnss::components::GnssCoreComponent;
use sdrkit_stack_gnss::core::{CoreConfig, GnssCore, PIECE};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator};
use sdrkit_stack_gnss::monitor::Monitor;
use sdrkit_stack_gnss::system::System;
use std::alloc::{GlobalAlloc, Layout, System as Heap};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn note() {
    // `try_with` fails only while the thread ends. Nothing is counted then.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call goes to the system allocator with the same arguments. `note` only touches thread-local cells.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the caller keeps the contract of `GlobalAlloc::alloc`.
        unsafe { Heap.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` came from `Heap` with layout `l`.
        unsafe { Heap.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// The allocations that `f` makes on this thread.
fn allocations(f: impl FnOnce()) -> usize {
    ALLOCS.with(|n| n.set(0));
    COUNTING.with(|on| on.set(true));
    f();
    COUNTING.with(|on| on.set(false));
    ALLOCS.with(|n| n.get())
}

/// The generator and the noise of its sky.
fn sky(o: GenOptions) -> (GnssGenerator, Awgn) {
    let g = GnssGenerator::new(o);
    let level = NoiseLevel::Cn0DbHz(g.sky_cn0_dbhz());
    let noise = Awgn::complex(level, 1.0, g.opt.rate_hz, 7).unwrap();
    (g, noise)
}

/// Once its buffers have their size, a piece of the monitor allocates nothing.
#[test]
fn monitor_piece_allocates_nothing() {
    let fs = 4e6;
    let mut m = Monitor::new(fs);
    let x: Vec<Complex32> = (0..16_384)
        .map(|i| {
            Complex32::new(
                ((i * 7) % 13) as f32 / 13.0 - 0.5,
                ((i * 5) % 11) as f32 / 11.0 - 0.5,
            )
        })
        .collect();
    m.piece(&x, (0.0, 0.0));
    let n = allocations(|| {
        m.piece(&x, (0.0, 0.0));
    });
    assert_eq!(n, 0, "{n} allocations in a piece");
}

/// The pieces of `seconds` of the GPS (or, with `sbas`, the SBAS) test signal after a warm-up of `warm_s`: how many produced no
/// output (no report, measurement, frame, search result or log line), how many of those allocated, and their allocations.
fn quiet_piece_allocations(sbas: bool, warm_s: f64, seconds: f64) -> (usize, usize, usize) {
    let rate = 4e6;
    let systems = if sbas {
        System::Sbas.bit()
    } else {
        System::Gps.bit()
    };
    let (mut g, mut noise) = sky(GenOptions {
        rate_hz: rate,
        systems,
        sbas_structured: sbas,
        ..Default::default()
    });
    let mut rx = GnssCore::new(CoreConfig {
        systems,
        sbas_mode: sbas,
        ..Default::default()
    });
    rx.configure(rate, 1575.42e6, false).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); PIECE];
    let pieces = |s: f64| (s * rate / PIECE as f64) as usize;
    let (mut quiet, mut bad, mut allocs) = (0, 0, 0);
    for k in 0..pieces(warm_s + seconds) {
        g.generate(&mut x);
        noise.process(&mut x);
        let log = rx.log.back().map(|l| l.as_ptr());
        let n = allocations(|| rx.feed(&x));
        let gave = rx.report_due
            || rx.log.back().map(|l| l.as_ptr()) != log
            || !rx.events.is_empty()
            || !rx.out_hits.is_empty()
            || !rx.out_frames.is_empty()
            || !rx.out_eph.is_empty()
            || !rx.out_meas.is_empty()
            || !rx.out_fix.is_empty()
            || !rx.out_sbas.is_empty()
            || !rx.out_channels.is_empty();
        rx.report_due = false;
        rx.events.clear();
        rx.out_hits.clear();
        rx.out_frames.clear();
        rx.out_eph.clear();
        rx.out_meas.clear();
        rx.out_fix.clear();
        rx.out_sbas.clear();
        rx.out_channels.clear();
        if k >= pieces(warm_s) && !gave {
            quiet += 1;
            if n > 0 {
                bad += 1;
                allocs += n;
            }
        }
    }
    (quiet, bad, allocs)
}

/// Tracking and searching allocate nothing: once the channels run, a piece that produces no output allocates nothing.
#[test]
fn core_quiet_pieces_allocate_nothing() {
    for sbas in [false, true] {
        let (quiet, bad, allocs) = quiet_piece_allocations(sbas, 8.0, 4.0);
        assert!(quiet > 100, "only {quiet} quiet pieces");
        assert_eq!(
            bad, 0,
            "sbas {sbas}: {bad} of {quiet} quiet pieces made {allocs} allocations"
        );
    }
}

/// The `gnss` stack graph with one GPS satellite locked: once warm, a block of 16 384 samples that holds no report, no
/// measurement, no navigation frame and no copy of their results allocates nothing. The test prints the
/// allocations of the blocks that do.
#[test]
fn stack_graph_tracking_one_satellite_allocates_nothing_once_warm() {
    let rate = 4e6;
    let (mut g, mut noise) = sky(GenOptions {
        rate_hz: rate,
        systems: System::Gps.bit(),
        max_sats: Some(1),
        ..Default::default()
    });
    let desc = &sdrkit_stack_gnss::GNSS;
    let schema = (desc.settings)();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("known_week").unwrap(), &Value::I64(2400))
        .unwrap();
    for key in [
        "sys_qzss",
        "sys_sbas",
        "sys_galileo",
        "sys_beidou",
        "sys_glonass",
    ] {
        v.set(schema.id(key).unwrap(), &Value::Bool(false)).unwrap();
    }
    let mut rx = sdrkit_stack_gnss::gnss_receiver(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(rate, 1575.42e6, 1575.42e6).recording())
        .unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); PIECE];
    let mut out = Output::default();
    let pieces = |s: f64| (s * rate / PIECE as f64) as usize;
    let core = |rx: &sdrkit_graph::StackGraph| {
        let c = rx.graph().component_as::<GnssCoreComponent>("rx").unwrap();
        let frames = c.core.frame_counts();
        let units: f64 = c.core.nav.sats.values().filter_map(|n| n.last_tow).sum();
        let news = c.core.log.len() as f64 + (frames.0 + frames.1) as f64 + units;
        (c.core.last_report().state, news, c.core.signal_s())
    };
    let (mut quiet, mut quiet_allocs, mut busy, mut busy_allocs) = (0, 0, 0, 0);
    // The graph copies the changed results of its components every 1/60 s of signal, so the copy of a change comes a few
    // blocks after the change.
    let merge_s = sdrkit_graph::Graph::MERGE_INTERVAL_S + PIECE as f64 / rate;
    let mut changed_at = f64::NEG_INFINITY;
    for k in 0..pieces(16.0) {
        g.generate(&mut x);
        noise.process(&mut x);
        let (_, log, t0) = core(&rx);
        let n = allocations(|| rx.feed(&x, &mut out));
        let (state, log_after, t1) = core(&rx);
        let gave = !out.events.is_empty()
            || log_after != log
            || (t1 / 0.25).floor() != (t0 / 0.25).floor()
            || (t1 - 0.08).floor() != (t0 - 0.08).floor()
            || t1.floor() != t0.floor();
        if gave {
            changed_at = t1;
        }
        let gave = gave || t1 - changed_at <= merge_s;
        out.clear();
        if k < pieces(10.0) {
            continue;
        }
        assert!(state >= 1, "no satellite locked after 10 s");
        if gave {
            busy += 1;
            busy_allocs += n;
        } else {
            quiet += 1;
            quiet_allocs += n;
        }
    }
    eprintln!(
        "{quiet} quiet blocks: {quiet_allocs} allocations; {busy} blocks with a report, a measurement or an event: \
         {busy_allocs} allocations, {:.1} a block",
        busy_allocs as f64 / busy.max(1) as f64
    );
    assert!(quiet > 50, "only {quiet} quiet blocks");
    assert_eq!(
        quiet_allocs, 0,
        "{quiet_allocs} allocations in {quiet} quiet blocks"
    );
}
