//! Allocations in steady state: the receiver allocates nothing per symbol, per FEC block or per T2-frame, alone and through the
//! stack graph with the transport stream layer.

use super::common::*;
use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Receiver, SettingSet, StackConfig, Values,
};
use sdrkit_stack_dvbt2::params::*;
use sdrkit_stack_dvbt2::rx::{RxSettings, T2Receiver};
use sdrkit_proto_mpegts::source::{NumberedSource, TestMux, TestSound};
use sdrkit_stack_dvbt2::generator::{tx_config, PATTERN, SOUND_PID};
use sdrkit_stack_dvbt2::settings::gen_settings;
use sdrkit_stack_dvbt2::tx::Transmitter;
use sdrkit_stack_dvbt2::{receiver, Dvbt2Setting, T2Generator};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call goes straight to the system allocator; the count is a thread-local cell.
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

#[test]
fn no_allocation_per_symbol_or_block() {
    // 1K frames are long (many symbols and FEC blocks per frame): the count must not grow with them.
    let cfg = single(
        Fft::K1,
        Guard::G1_8,
        PilotPattern::Pp2,
        coding(FecFrame::Short, CodeRate::R1_2, Modulation::Qpsk, false),
        6,
    );
    let mut tx = Transmitter::new(cfg).unwrap();
    let x = signal(&mut tx, 12, 2000);
    let frame = tx.frame_len();
    let symbols = tx.shape().l_f();
    let mut rx = T2Receiver::new(RxSettings::default());
    let mut sink = |_: sdrkit_stack_dvbt2::rx::BbFrameOut<'_>| {};
    let warm = 2000 + 6 * frame;
    for c in x[..warm].chunks(4096) {
        rx.feed(c, &mut sink);
    }
    let before = ALLOCS.with(|c| c.get());
    for c in x[warm..warm + 4 * frame].chunks(4096) {
        rx.feed(c, &mut sink);
    }
    let n = ALLOCS.with(|c| c.get()) - before;
    assert!(rx.status().frames >= 9, "{}", rx.status().frames);
    assert_eq!(n, 0, "allocations in 4 frames of {symbols} symbols");
}

#[test]
fn the_stack_graph_allocates_nothing_once_warm() {
    let schema = Dvbt2Setting::schema();
    let rate = Bandwidth::B8.rate();
    // The test multiplex with its tables, data and sound, without the video: the MPEG video path of the shared transport
    // stream layer still allocates for each access unit.
    let gen = gen_settings(&Values::defaults(&schema)).expect("valid settings");
    let pattern = Transmitter::new(tx_config(&gen, Box::new(NumberedSource::new(PATTERN))))
        .expect("a transmitter");
    let net = pattern.plp_bits_per_frame() as f64 * pattern.rate() / pattern.frame_len() as f64;
    let mux = TestMux::dvbt_builder(1, "sdrkit DVB-T2")
        .sound(TestSound {
            pid: SOUND_PID,
            mux_bit_rate: net,
        })
        .build()
        .expect("a multiplex");
    let tx = Transmitter::new(tx_config(&gen, Box::new(mux))).expect("a transmitter");
    let mut g = T2Generator::new(Some(tx), rate);
    let warm_s = 1.0;
    let mut x = vec![Complex32::default(); ((warm_s + 0.5) * rate) as usize];
    g.generate(&mut x);
    let mut rx = receiver(&StackConfig::new(Values::defaults(&schema))).expect("builds");
    // A worker pool without helpers: the pool allocates for each call that it shares out.
    let pool = sdrkit_graph::Pool::new(1);
    pool.set_workers(0);
    rx.set_context(sdrkit_graph::RunContext {
        pool,
        ..Default::default()
    });
    rx.configure(&InputInfo::new(rate, 474e6, 474e6).recording())
        .expect("rate");
    let mut out = Output::default();
    let warm = (warm_s * rate) as usize;
    for c in x[..warm].chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let before = ALLOCS.with(|c| c.get());
    for c in x[warm..].chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let n = ALLOCS.with(|c| c.get()) - before;
    let ok = sdrkit_core::owner::Owner::results(&rx)
        .fields()
        .get(&sdrkit_core::publish::keys::FRAMES_OK);
    assert!(ok.is_some_and(|n| n > 10), "{ok:?}");
    assert_eq!(n, 0, "allocations once warm");
}
