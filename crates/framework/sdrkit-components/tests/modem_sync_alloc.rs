//! Every `modem.*` synchronisation component allocates nothing in steady state: one graph holding all of them (the classic chain
//! from matched filter to slicer, with the equaliser and SNR meter beside it, and the frequency search, squelch and burst detector
//! on the input) is warmed up, then run for 1,000 more calls under an allocator that counts. (The counting allocator needs
//! `unsafe impl GlobalAlloc`; it is defined in this test only.)

mod set_util;
use set_util::{result, result_f64, Tb};

use sdrkit_algo_sim::modulate::Linear;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::sync::points::ConstellationName;
use sdrkit_components::modem::sync::{
    BurstDetect, CarrierSyncComponent, Equaliser, FreqSearch, MatchedFilter, Slicer, SnrMeter,
    SquelchComponent, SymbolSyncComponent,
};
use sdrkit_components::registry;
use sdrkit_core::{Complex32, Output, Value};
use sdrkit_graph::Format;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

const RATE: f64 = 48_000.0;
const PIECE: usize = 4096;

/// RRC QPSK at 4 samples per symbol, 300 Hz off, with noise, and a strong burst at the start of every piece.
fn signal() -> Vec<Complex32> {
    let mut rng = Rng::new(1);
    let s = std::f32::consts::FRAC_1_SQRT_2;
    let syms: Vec<Complex32> = (0..16 * PIECE / 4)
        .map(|_| {
            Complex32::new(
                if rng.bit() == 0 { s } else { -s },
                if rng.bit() == 0 { s } else { -s },
            )
        })
        .collect();
    let mut tx = Linear::rrc(0.35, 16.0, 4.0).unwrap();
    let mut x = Vec::new();
    tx.push(&syms, &mut x);
    x.truncate(16 * PIECE);
    FrequencyOffset::new(RATE, 300.0, 0.0)
        .unwrap()
        .process(&mut x);
    for (k, v) in x.iter_mut().enumerate() {
        if k % PIECE < 600 {
            *v *= 30.0;
        }
    }
    Awgn::with_power(0.05, 2).unwrap().process(&mut x);
    x
}

// One test function, so no other test thread allocates while counting.
#[test]
fn the_components_allocate_nothing_in_steady_state() {
    let mut g = Tb::new()
        .add("mf", MatchedFilter::rrc(0.35, 4.0, 2).unwrap())
        .add("ss", SymbolSyncComponent::new(2.0).unwrap())
        .add("cs", CarrierSyncComponent::costas(4, 0.02))
        .add("sl", Slicer::new(ConstellationName::Qpsk))
        .add("eq", Equaliser::default())
        .add("snr", SnrMeter::default())
        .add("fs", FreqSearch::default())
        .add("sq", SquelchComponent::default())
        .add("bd", BurstDetect::default())
        .input("mf")
        .input("fs")
        .input("sq")
        .input("bd")
        .connect("mf", "ss")
        .connect("ss.out", "cs")
        .connect("cs", "sl")
        .connect("cs", "eq")
        .connect("cs", "snr")
        .set("cs.fll", Value::F64(0.005))
        .set("fs.fft", Value::F64(1024.0))
        .set("sq.threshold_db", Value::F64(-10.0));
    // A component with no consumer does not run: end each branch in a sink.
    for (i, from) in ["sl.out", "eq.out", "fs.out", "sq.out", "bd.out"]
        .into_iter()
        .enumerate()
    {
        let sink = registry().make("dsp.null_sink").unwrap();
        g = g
            .add_boxed(&format!("sink{i}"), sink)
            .connect(from, &format!("sink{i}"));
    }
    let mut g = g.build().unwrap();
    g.configure_input(Format::iq(RATE, 0.0, 0.0)).unwrap();
    let x = signal();
    let mut out = Output::default();
    let pieces: Vec<&[Complex32]> = x.chunks(PIECE).collect();
    let mut k = 0;
    let mut run = |n: usize, g: &mut sdrkit_graph::Graph, out: &mut Output| {
        for _ in 0..n {
            out.clear();
            g.process(pieces[k % pieces.len()], out);
            k += 1;
        }
    };
    run(300, &mut g, &mut out);
    let before = ALLOCS.load(Ordering::Relaxed);
    run(1000, &mut g, &mut out);
    let n = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(n, 0, "{n} allocations in 1000 calls");
    // The run did the work: bursts were found and the chain locked.
    let bursts = result_f64(&g, "bd", "bursts");
    assert!(bursts >= 1000.0, "{bursts} bursts");
    assert_eq!(result(&g, "cs", "locked"), Value::Bool(true));
}
