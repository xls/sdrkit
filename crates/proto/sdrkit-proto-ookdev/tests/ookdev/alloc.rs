//! Steady state allocates nothing: the detector, the slicers, the row decoders, the protocol list and the guess, after a
//! warm-up pass, over 1,000 more calls each. The count is per thread, so tests running beside it do not disturb it.

use crate::common::{bits_of, ook_iq};
use sdrkit_proto_ookdev::guess::Guesser;
use sdrkit_proto_ookdev::{
    encode, BitRows, Check, Coding, Ctx, Decoding, DetectorConfig, Modulation, ProtocolDef,
    ProtocolList, PulseDetector, Reading, Slicer, Timing, Train,
};
use sdrkit_util_bits::line::ManchesterConv;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Runs `f` once to warm up, then 1,000 times, and returns the allocations of the 1,000.
fn count(mut f: impl FnMut()) -> usize {
    f();
    let before = ALLOCS.with(|c| c.get());
    for _ in 0..1000 {
        f();
    }
    ALLOCS.with(|c| c.get()) - before
}

fn nothing(_: &BitRows, _: &mut Ctx, _: &mut Vec<Reading>) {}

static QUIET: ProtocolDef = ProtocolDef {
    name: "Quiet",
    modulation: Modulation::Ook,
    timing: Timing::new(Coding::Pwm, 464.0, 1404.0, 1800.0),
    despike_us: 10.0,
    decode: nothing,
};

#[test]
fn steady_state_allocates_nothing() {
    let pairs: Vec<(f32, f32)> = (0..24)
        .map(|k| {
            if k % 3 == 0 {
                (1404.0, 464.0)
            } else {
                (464.0, 1404.0)
            }
        })
        .collect();
    let mut x = ook_iq(&pairs, 1e6, 0.3, 2000.0, 1e-6, 5);
    x.extend(std::iter::repeat_n(
        sdrkit_core::Complex32::default(),
        12_000,
    ));
    let mut d = PulseDetector::new(DetectorConfig::new(1e6)).unwrap();
    let mut trains = 0usize;
    assert_eq!(
        count(|| d.process(&x, |t| trains += t.pulses.len())),
        0,
        "detector"
    );
    assert!(trains > 0);

    let train = Train::from_us(Modulation::Ook, 1e6, &pairs);
    let mut s = Slicer::new();
    let t = Timing::new(Coding::Pwm, 464.0, 1404.0, 1800.0);
    let mut bits = 0usize;
    assert_eq!(
        count(|| s.slice(&t, &train, |r| bits += r.len(0)).unwrap()),
        0,
        "slicer"
    );

    let mut nrz = Train::new(Modulation::Fsk, 1e6);
    encode::nrz(
        &bits_of(&[0xAA, 0xAA, 0xAA, 0x2D, 0xD4, 0x12]),
        52.0,
        &mut nrz,
    );
    let tn = Timing::new(Coding::Nrz, 52.0, 52.0, 150.0);
    assert_eq!(
        count(|| s.slice(&tn, &nrz, |r| bits += r.len(0)).unwrap()),
        0,
        "nrz"
    );

    let rows = BitRows::parse("1 00 10 11 00 10 10 01 10 01 10 01 01 10").unwrap();
    let mut out = BitRows::default();
    assert_eq!(
        count(|| {
            out.clear();
            rows.decode_manchester(0, 1, 100, ManchesterConv::Ieee, &mut out);
            rows.decode_fm0(0, 0, 100, &mut out);
        }),
        0,
        "row decoders"
    );

    let mut list = ProtocolList::new();
    list.add_static(&QUIET);
    let mut work = Decoding::new();
    let mut found: Vec<Reading> = Vec::new();
    assert_eq!(
        count(|| {
            work.run(&list, &train, &mut found).unwrap();
        }),
        0,
        "list"
    );

    let mut g = Guesser::default();
    assert_eq!(
        count(|| {
            g.guess(&train);
        }),
        0,
        "guess"
    );

    let _ = Check::Crc;
}
