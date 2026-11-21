//! Checks that every `bits.*` and `bytes.*` component allocates nothing in steady state: each runs in a small graph, is warmed up
//! so its buffers (and the graph's) reach their size, then runs many more passes under an allocator that counts. (The counting
//! allocator needs `unsafe impl GlobalAlloc`; it is defined in this test only.)

mod bits_util;

use bits_util::{chain, random_bits, soft};
use sdrkit_core::{Complex32, Output, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Graph, Kind, Packet};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::hdlc::hdlc_encode;
use sdrkit_util_bits::line::{Manchester, ManchesterConv, Nrzi};
use sdrkit_util_bits::{pack, BitOrder};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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

const PIECE: usize = 61;
const WARM: usize = 400;
const COUNTED: usize = 1000;
const ITEMS: usize = PIECE * (WARM + COUNTED);

/// Allocations made by `g` over the counted passes, after the warm-up passes.
fn steady_allocs(mut g: Graph) -> usize {
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..WARM {
        out.clear();
        g.process(&empty, &mut out);
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..COUNTED {
        out.clear();
        g.process(&empty, &mut out);
    }
    ALLOCS.load(Ordering::Relaxed) - before
}

fn txt(s: &str) -> Value {
    Value::Text(s.into())
}

fn num(x: f64) -> Value {
    Value::F64(x)
}

/// Frames after the CCSDS sync marker, back to back: `frame` bits each.
fn framed(n: usize, frame: usize) -> Vec<u8> {
    let mut asm = Vec::new();
    pack::unpack(&[0x1A, 0xCF, 0xFC, 0x1D], BitOrder::Msb, &mut asm);
    let mut v = Vec::with_capacity(n + frame + 32);
    let mut k = 0;
    while v.len() < n {
        v.extend(&asm);
        v.extend(random_bits(frame, 100 + k));
        k += 1;
    }
    v.truncate(n);
    v
}

// One test function, so no other test thread allocates while counting.
#[test]
fn no_component_allocates_in_steady_state() {
    // The counter sees allocations.
    let before = ALLOCS.load(Ordering::Relaxed);
    std::hint::black_box(Vec::<u8>::with_capacity(16));
    assert!(ALLOCS.load(Ordering::Relaxed) > before);
    let hb = || Format::hard_bits(9600.0);
    let llr = || Format::llr(9600.0, Some(4.0));
    let bits = random_bits(ITEMS, 1);
    let frames = framed(ITEMS, 256);
    let mut chips = Vec::new();
    Manchester::encode(&bits[..ITEMS / 2], ManchesterConv::Ieee, &mut chips);

    // AX.25-sized frames with flags between them, NRZI-coded.
    let payload: Vec<u8> = (0..60u8).collect();
    let n_frames = ITEMS / (payload.len() * 8 + 40) + 2;
    let all: Vec<&[u8]> = (0..n_frames).map(|_| payload.as_slice()).collect();
    let mut hdlc = Vec::new();
    hdlc_encode(&all, &CRC16_IBM_SDLC, 2, &mut hdlc);
    Nrzi::new(0).encode(&mut hdlc);

    let mut with_crc = b"123456789".to_vec();
    with_crc.extend([0x6E, 0x90]);
    let good: Arc<[u8]> = Arc::from(with_crc);
    let packets: Vec<Packet> = (0..ITEMS).map(|_| Packet::new(good.clone(), 0.0)).collect();

    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, Format, Data, Vec<(&str, &str)>, Vec<(&str, Value)>)> = vec![
        (
            "nrzi",
            hb(),
            Data::HardBits(bits.clone()),
            vec![("c", "bits.nrzi")],
            vec![],
        ),
        (
            "nrzm soft",
            llr(),
            Data::Llr(soft(&bits, 9)),
            vec![("c", "bits.nrzm_llr")],
            vec![],
        ),
        (
            "diff",
            hb(),
            Data::HardBits(bits.clone()),
            vec![("c", "bits.diff")],
            vec![],
        ),
        (
            "manchester",
            hb(),
            Data::HardBits(chips.clone()),
            vec![("c", "bits.manchester")],
            vec![],
        ),
        (
            "manchester soft",
            llr(),
            Data::Llr(soft(&chips, 9)),
            vec![("c", "bits.manchester_llr")],
            vec![],
        ),
        (
            "sync and descramble",
            hb(),
            Data::HardBits(frames.clone()),
            vec![("s", "bits.sync"), ("c", "bits.descramble")],
            vec![("c.sequence", txt("ccsds_randomiser"))],
        ),
        (
            "sync and descramble soft",
            llr(),
            Data::Llr(soft(&frames, 9)),
            vec![("s", "bits.sync_llr"), ("c", "bits.descramble_llr")],
            vec![("c.period", num(100.0))],
        ),
        (
            "self sync",
            hb(),
            Data::HardBits(bits.clone()),
            vec![("c", "bits.self_sync")],
            vec![],
        ),
        (
            "sync and block deinterleave",
            hb(),
            Data::HardBits(frames.clone()),
            vec![("s", "bits.sync"), ("c", "bits.deinterleave")],
            vec![("c.rows", num(16.0)), ("c.cols", num(16.0))],
        ),
        (
            "forney soft",
            llr(),
            Data::Llr(soft(&bits, 9)),
            vec![("c", "bits.deinterleave_llr")],
            vec![("c.kind", txt("forney"))],
        ),
        (
            "sync and pack",
            hb(),
            Data::HardBits(frames.clone()),
            vec![("s", "bits.sync"), ("c", "bits.pack")],
            vec![("c.frame_bits", num(256.0))],
        ),
        (
            "nrzi and hdlc",
            hb(),
            Data::HardBits(hdlc),
            vec![("n", "bits.nrzi"), ("c", "bits.hdlc")],
            vec![],
        ),
        (
            "crc",
            Format::new(Kind::Bytes, 0.0),
            Data::Bytes(packets.clone()),
            vec![("c", "bytes.crc")],
            vec![("c.action", txt("mark"))],
        ),
        (
            "byte descramble",
            Format::new(Kind::Bytes, 0.0),
            Data::Bytes(packets),
            vec![("c", "bytes.descramble")],
            vec![],
        ),
        (
            "hard",
            llr(),
            Data::Llr(soft(&bits, 9)),
            vec![("c", "dsp.hard_decision")],
            vec![],
        ),
        (
            "soft",
            hb(),
            Data::HardBits(bits.clone()),
            vec![("c", "bits.soft")],
            vec![],
        ),
    ];
    for (what, format, data, stages, settings) in cases {
        let g = chain(format, data, PIECE, &stages, &settings);
        let n = steady_allocs(g);
        assert_eq!(n, 0, "{what} allocated {n} times in steady state");
    }
}
