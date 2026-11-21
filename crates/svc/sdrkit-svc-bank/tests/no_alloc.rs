//! The path of the bank's components that runs for each piece of input allocates nothing once warm.

use sdrkit_core::Complex32;
use sdrkit_graph::stream::Frames;
use sdrkit_graph::{Buffer, Component, Data, Format, Io, Kind};
use sdrkit_svc_bank::{CarrierScan, ChannelOut, Channelize};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call goes to the system allocator with the same arguments. Only a counter is added.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the caller's contract is passed on unchanged.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract is passed on unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the caller's contract is passed on unchanged.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn clear(outs: &mut [Buffer]) {
    for b in outs {
        match &mut b.data {
            Data::Iq(v) => v.clear(),
            Data::Messages(v) => v.clear(),
            _ => {}
        }
    }
}

#[test]
fn the_channelizer_and_the_carrier_scan_allocate_nothing_once_warm() {
    let rate = 2.4e6;
    let centre = 145e6;
    let chans: Vec<ChannelOut> = [144.6e6, 145.1e6, 145.7e6]
        .iter()
        .map(|&f| ChannelOut {
            freq_hz: f,
            bandwidth_hz: 12.5e3,
            rate_hz: 25e3,
        })
        .collect();
    let mut ch = Channelize::new(&chans, 4096);
    let input = Format::iq(rate, centre, centre);
    let mut formats = vec![input.clone()];
    formats.extend(ch.configure(&[input]).unwrap());
    let x: Vec<Complex32> = (0..24_000)
        .map(|i| {
            let p = std::f64::consts::TAU * 101e3 * i as f64 / rate;
            Complex32::new(p.cos() as f32, p.sin() as f32) * 0.1
        })
        .collect();
    let mut inputs = [Buffer::new(&Kind::Iq)];
    inputs[0].data = Data::Iq(x);
    let mut outs: Vec<Buffer> = (0..3).map(|_| Buffer::new(&Kind::Iq)).collect();
    let run = |ch: &mut Channelize, outs: &mut [Buffer]| {
        clear(outs);
        let mut io = Io::standalone(&inputs, &formats, &[0], &[0, 1, 2], outs);
        ch.process(&mut io);
    };
    for _ in 0..4 {
        run(&mut ch, &mut outs);
    }
    assert_eq!(ch.passes(), 1);

    let mut scan = CarrierScan::watching(&[(145.1e6, 12.5e3), (144.6e6, 12.5e3)]);
    let spec_in = Format {
        bandwidth_hz: rate,
        frame_len: 4096,
        ..Format::iq(50.0, centre, centre).with_kind(Kind::Spectrum)
    };
    let mut sformats = vec![spec_in.clone()];
    sformats.extend(scan.configure(&[spec_in]).unwrap());
    let mut frames = [Buffer::new(&Kind::Spectrum)];
    frames[0].data = Data::Spectrum(Frames {
        bins: 4096,
        values: vec![-80.0; 4096 * 2],
    });
    let mut msgs = vec![Buffer::new(&Kind::Messages)];
    let step = |scan: &mut CarrierScan, msgs: &mut [Buffer]| {
        clear(msgs);
        let mut io = Io::standalone(&frames, &sformats, &[0], &[0], msgs);
        scan.process(&mut io);
    };
    for _ in 0..4 {
        step(&mut scan, &mut msgs);
    }

    COUNTING.store(true, Ordering::Relaxed);
    for _ in 0..20 {
        run(&mut ch, &mut outs);
        step(&mut scan, &mut msgs);
    }
    COUNTING.store(false, Ordering::Relaxed);
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
