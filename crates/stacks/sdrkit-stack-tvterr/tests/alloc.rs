//! The receive paths allocate nothing once warm: each stack's test signal through its stack graph.

use sdrkit_core::{Complex32, Generator, InputInfo, Output, StackConfig, Values};
use sdrkit_stack_tvterr::{atsc, atsc3, dtmb, isdbt};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call goes straight to the system allocator. The count is a thread-local cell.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        // SAFETY: the caller's layout goes to the system allocator unchanged.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `alloc` or `realloc` above with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        // SAFETY: as `dealloc`, and `n` is the caller's new size.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(Cell::get);
    f();
    ALLOCS.with(Cell::get) - before
}

/// `seconds` of a generator's signal.
fn signal<G: Generator>(mut g: G, seconds: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::new(0.0, 0.0); (seconds * g.rate_hz()) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

/// Allocations while the receiver of stack `d` at `rate` takes `x` after `warm_s` seconds of it, in pieces of 16 384 samples.
fn steady_state(
    d: &sdrkit_core::StackDescriptor,
    x: &[Complex32],
    rate: f64,
    warm_s: f64,
) -> usize {
    let mut rx = (d.make_receiver)(&StackConfig::new(Values::defaults(&(d.settings)()))).unwrap();
    rx.configure(&InputInfo::new(rate, 539e6, 539e6).recording())
        .expect("rate");
    let warm = ((warm_s * rate) as usize).min(x.len());
    let mut out = Output::default();
    for c in x[..warm].chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let mut n = 0;
    for c in x[warm..].chunks(16_384) {
        out.clear();
        n += allocations(|| rx.feed(c, &mut out));
    }
    n
}

#[test]
fn the_isdbt_receive_path_allocates_nothing_once_warm() {
    let x = signal(
        isdbt::gen::IsdbtGenerator::new(Default::default()).expect("a generator"),
        4.5,
    );
    let n = steady_state(&isdbt::DESCRIPTOR, &x, 8e6, 3.0);
    assert_eq!(n, 0, "ISDB-T: {n} allocations");
}

/// Seconds of the ATSC 3.0 test signal, and of them the seconds before the count starts. A frame does not hold a whole
/// number of rounds of the service of the test signal, so the most ROUTE objects that one pass completes, and with it
/// the last buffer of the object pool, comes only after some frames.
const ATSC3_S: f64 = 2.5;
const ATSC3_WARM_S: f64 = 2.0;

#[test]
fn the_atsc_receive_path_allocates_nothing_once_warm() {
    let x = signal(
        atsc::gen::AtscGenerator::new(Default::default()).expect("a generator"),
        1.5,
    );
    let n = steady_state(&atsc::DESCRIPTOR, &x, 8e6, 1.0);
    assert_eq!(n, 0, "ATSC: {n} allocations");
}

#[test]
fn the_atsc3_receive_path_allocates_nothing_once_warm() {
    let x = signal(
        atsc3::Atsc3Generator::new(Default::default()).expect("a generator"),
        ATSC3_S,
    );
    let n = steady_state(&atsc3::DESCRIPTOR, &x, 8e6, ATSC3_WARM_S);
    assert_eq!(n, 0, "ATSC 3.0: {n} allocations");
}

/// The delay lines of the convolutional time de-interleaver (724 rows) and the buffers of the hybrid one, across subframes
/// and within one, are made in the first frames.
#[test]
fn the_atsc3_time_de_interleavers_allocate_nothing_once_warm() {
    use atsc3::gen::GenTi;
    for time_interleaver in [
        GenTi::Convolutional {
            depth: 1,
            extended: false,
        },
        GenTi::Hybrid {
            inter_subframe: true,
            ti_blocks: 3,
            cell_interleaver: true,
            spare: 1,
        },
        GenTi::Hybrid {
            inter_subframe: false,
            ti_blocks: 2,
            cell_interleaver: true,
            spare: 3,
        },
    ] {
        let p = atsc3::Atsc3GenParams {
            time_interleaver,
            ..Default::default()
        };
        let x = signal(atsc3::Atsc3Generator::new(p).expect("a generator"), ATSC3_S);
        let n = steady_state(&atsc3::DESCRIPTOR, &x, 8e6, ATSC3_WARM_S);
        assert_eq!(n, 0, "ATSC 3.0, {time_interleaver:?}: {n} allocations");
    }
}

/// Frames of two subframes with two FFT sizes: the symbols of each subframe are made with the first frames.
#[test]
fn the_atsc3_receive_path_of_several_subframes_allocates_nothing_once_warm() {
    use atsc3::gen::{GenPlace, GenSubframe};
    let p = atsc3::Atsc3GenParams {
        symbols: 24,
        more_subframes: vec![GenSubframe {
            fft: atsc3::layout::FftSize::K16,
            guard: 768,
            symbols: 16,
            reduced_carriers: 0,
            pilots: (16, 4),
            pilot_boost: 0,
            frequency_interleaver: true,
            boundary: (true, true),
            plps: vec![GenPlace {
                plp: 0,
                start: 0,
                cells: None,
                subslices: 1,
                interval: 0,
            }],
        }],
        ..Default::default()
    };
    let x = signal(atsc3::Atsc3Generator::new(p).expect("a generator"), ATSC3_S);
    let n = steady_state(&atsc3::DESCRIPTOR, &x, 8e6, ATSC3_WARM_S);
    assert_eq!(n, 0, "ATSC 3.0, two subframes: {n} allocations");
}

#[test]
fn the_dtmb_receive_path_allocates_nothing_once_warm() {
    let x = signal(
        dtmb::gen::DtmbGenerator::new(Default::default()).expect("a generator"),
        1.5,
    );
    let n = steady_state(&dtmb::DESCRIPTOR, &x, 10e6, 1.0);
    assert_eq!(n, 0, "DTMB: {n} allocations");
}
