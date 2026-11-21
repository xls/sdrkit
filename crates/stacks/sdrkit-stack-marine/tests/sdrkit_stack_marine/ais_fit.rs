//! The AIS burst fit keeps its buffers: decoding a collision allocates no waveform of the burst's length.

use sdrkit_core::Complex32;
use sdrkit_stack_marine::ais::demod::{DecodeOptions, Decoder, Detector, RATE};
use sdrkit_stack_marine::ais::{encode, gmsk, line};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// The system allocator, with a count of the large blocks this thread requests while counting is on.
pub struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static LIMIT: Cell<usize> = const { Cell::new(usize::MAX) };
    static LARGE: Cell<usize> = const { Cell::new(0) };
}

fn note(size: usize) {
    // `try_with` so an allocation during thread teardown does not panic.
    let _ = ON.try_with(|on| {
        if on.get() && size >= LIMIT.with(Cell::get) {
            LARGE.with(|n| n.set(n.get() + 1));
        }
    });
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

/// Counts the blocks of at least `limit` bytes that `f` allocates on this thread.
fn large_blocks<T>(limit: usize, f: impl FnOnce() -> T) -> (T, usize) {
    LIMIT.with(|l| l.set(limit));
    LARGE.with(|n| n.set(0));
    ON.with(|on| on.set(true));
    let v = f();
    ON.with(|on| on.set(false));
    (v, LARGE.with(Cell::get))
}

fn position(mmsi: u32) -> Vec<u8> {
    encode::position_a(&encode::PositionA {
        msg_type: 1,
        mmsi,
        status: 0,
        rot_raw: 0,
        sog_kn: Some(5.0),
        lat: Some(50.0),
        lon: Some(-1.0),
        cog_deg: Some(10.0),
        heading: Some(10),
        second: 3,
    })
}

/// A strong burst and a weaker one 6 ms later and 600 Hz away on one channel at 48 kHz, with a little noise, through the coast
/// channel filter of the receiver.
fn collision() -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (0.3 * RATE) as usize];
    // The detector learns the floor in the first 60 ms.
    for (mmsi, start, amp, off, ph) in [
        (244_100_002u32, 0.1, 1.0, 0.0, 0.0),
        (244_200_002, 0.106, 0.5, 600.0, 1.0),
    ] {
        let b = gmsk::Burst {
            levels: line::burst_levels(&position(mmsi), 8),
            start_s: start,
            amplitude: amp,
            offset_hz: off,
            phase: ph,
            clock_ppm: 0.0,
            ramp_bits: 1.0,
        };
        b.add_to(&mut x, 0.0, RATE);
    }
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    for s in &mut x {
        *s += Complex32::new(rng.normal() as f32 * 0.01, rng.normal() as f32 * 0.01);
    }
    let taps = sdrkit_algo_dsp::fir::lowpass(RATE, 7.5e3, 15e3, 60.0).unwrap();
    let mut y = vec![Complex32::default(); x.len()];
    sdrkit_algo_dsp::fir::centred_convolve(&x, &taps, &mut y).expect("equal lengths");
    y
}

/// The remodulation and fit buffers live in the decoder, so a collision decode (fit, edge trials, cancellation, a second layer)
/// allocates no block as large as the burst's waveform.
#[test]
fn ais_fit_reuses_its_buffers() {
    // The counter sees a block of the requested size.
    assert_eq!(large_blocks(4096, || vec![0u8; 4096]).1, 1);
    let x = collision();
    let mut det = Detector::new(0);
    let mut cuts = Vec::new();
    det.process(&x, |c| cuts.push(c));
    assert_eq!(cuts.len(), 1, "one cut holds both bursts");
    let cut = &cuts[0];
    let opts = DecodeOptions {
        collision: true,
        noise_scale: 1.0,
        ..Default::default()
    };
    let mut dec = Decoder::default();
    let wave_bytes = cut.samples.len() * std::mem::size_of::<Complex32>();
    let (r, large) = large_blocks(wave_bytes, || dec.decode(cut, &opts));
    assert_eq!(r.frames.len(), 2, "both bursts decode");
    assert_eq!(
        large, 0,
        "blocks of a waveform's size allocated in one decode"
    );
}
