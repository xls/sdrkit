//! Allocations on the receive path of the `adsb` stack, counted by an allocator that counts only on the thread that enables
//! it, so other tests that run at the same time do not change the count.

use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Receiver, SettingSet, StackConfig, Station, Values,
};
use sdrkit_graph::{Graph, StackGraph};
use sdrkit_stack_adsb::checks::FrameChecks;
use sdrkit_stack_adsb::generator::{AdsbGenerator, AirspaceOptions};
use sdrkit_stack_adsb::settings::AdsbSetting;
use sdrkit_stack_adsb::table::Tracker;
use sdrkit_util_geo::Position;
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

const RATE: f64 = 4e6;
const BLOCK: usize = 16_384;

/// The `adsb` receiver with every decoding pass on, at the centre of the test airspace.
fn receiver() -> StackGraph {
    let mut v = Values::defaults(&AdsbSetting::schema());
    v.update_from_json(&serde_json::json!({"effort": "2"}))
        .unwrap();
    let mut rx = sdrkit_stack_adsb::receiver::receiver(&StackConfig::new(v)).unwrap();
    let c = AirspaceOptions::default().centre;
    rx.set_station(&Station::at(Position::new(c.0, c.1)));
    rx.configure(&InputInfo::new(RATE, 1090e6, 1090e6).recording())
        .unwrap();
    rx
}

/// Good messages, aircraft events and reports so far.
fn counts(g: &Graph) -> (u64, usize, u64) {
    let checks = g.component_as::<FrameChecks>("checks").unwrap();
    let tracker = g.component_as::<Tracker>("tracker").unwrap();
    (
        checks.checker().stats().good,
        tracker.table.events.len(),
        tracker.report_number(),
    )
}

/// The results of a run of the stack graph after its warm-up.
#[derive(Debug, Default)]
struct Tally {
    /// Blocks with no message, no event and no report.
    quiet: usize,
    quiet_allocs: usize,
    /// Blocks with messages but no event and no report, their messages and their allocations.
    message_blocks: usize,
    messages: u64,
    message_allocs: usize,
    /// Blocks with an aircraft event (and no report), their events and their allocations.
    event_blocks: usize,
    events: usize,
    event_allocs: usize,
    /// Blocks with a report (or the copy of a report's results), and their allocations.
    busy: usize,
    busy_allocs: usize,
}

/// Feeds `seconds` of the airspace with `aircraft` aircraft through the stack graph in blocks of [`BLOCK`] samples, and sorts
/// the blocks after the first `warm_s` seconds.
fn run(aircraft: usize, warm_s: f64, seconds: f64) -> Tally {
    let opts = AirspaceOptions {
        aircraft,
        ..Default::default()
    };
    let mut g = AdsbGenerator::airspace(RATE, opts, 7);
    let mut noise = sdrkit_algo_sim::noise::Awgn::with_power(1e-6, 3).unwrap();
    let mut rx = receiver();
    let mut x = vec![Complex32::new(0.0, 0.0); BLOCK];
    let mut out = Output::default();
    let blocks = |s: f64| (s * RATE / BLOCK as f64) as usize;
    // The graph copies the changed results of its components every 1/60 s of signal, so the copy of a report comes a few
    // blocks after the report.
    let merge_s = Graph::MERGE_INTERVAL_S + BLOCK as f64 / RATE;
    let mut busy_until = f64::NEG_INFINITY;
    let mut t = Tally::default();
    for k in 0..blocks(warm_s + seconds) {
        g.generate(&mut x);
        noise.process(&mut x);
        let (m0, _, r0) = counts(rx.graph());
        let n = allocations(|| rx.feed(&x, &mut out));
        let (m1, e1, r1) = counts(rx.graph());
        let now = (k + 1) as f64 * BLOCK as f64 / RATE;
        let events = out.events.len() + e1;
        out.clear();
        if r1 != r0 {
            busy_until = now + merge_s;
        }
        if k < blocks(warm_s) {
            continue;
        }
        if now <= busy_until {
            t.busy += 1;
            t.busy_allocs += n;
        } else if events > 0 {
            t.event_blocks += 1;
            t.events += events;
            t.event_allocs += n;
        } else if m1 != m0 {
            t.message_blocks += 1;
            t.messages += m1 - m0;
            t.message_allocs += n;
        } else {
            t.quiet += 1;
            t.quiet_allocs += n;
        }
    }
    t
}

/// Noise only: once warm, a block without a report allocates nothing.
#[test]
fn noise_allocates_nothing_once_warm() {
    let t = run(0, 2.0, 3.0);
    eprintln!("noise: {t:?}");
    assert!(t.quiet > 100, "only {} quiet blocks", t.quiet);
    assert_eq!(t.quiet_allocs, 0, "{t:?}");
}

/// The test airspace with 60 aircraft: once warm, a block with messages and no report or event allocates nothing. The test
/// prints what a message, an event and a report cost.
#[test]
fn sixty_aircraft_allocate_nothing_per_message_once_warm() {
    let t = run(60, 4.0, 4.0);
    eprintln!(
        "60 aircraft: {} message blocks with {} messages: {} allocations ({:.3} a message); {} quiet blocks: {} allocations; \
         {} blocks with {} events: {} allocations ({:.1} an event); {} blocks with a report: {} allocations ({:.1} a block)",
        t.message_blocks,
        t.messages,
        t.message_allocs,
        t.message_allocs as f64 / t.messages.max(1) as f64,
        t.quiet,
        t.quiet_allocs,
        t.event_blocks,
        t.events,
        t.event_allocs,
        t.event_allocs as f64 / t.events.max(1) as f64,
        t.busy,
        t.busy_allocs,
        t.busy_allocs as f64 / t.busy.max(1) as f64
    );
    assert!(t.messages > 1000, "only {} messages", t.messages);
    assert_eq!(t.quiet_allocs, 0, "{t:?}");
    assert_eq!(t.message_allocs, 0, "{t:?}");
}
