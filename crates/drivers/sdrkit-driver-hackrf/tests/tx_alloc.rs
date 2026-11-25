//! Once transmit runs, the path from the push to the bulk OUT queue allocates nothing.

mod common;

use common::*;
use sdrkit_driver_hackrf::proto::tx_write_size;
use sdrkit_driver_usb::FakeUsb;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn the_transmit_path_allocates_nothing_after_start() {
    // A lenient script: the settings and the writes need no steps; the TX VGA reply is the one input.
    let text = format!(
        "sdrkit-usb 1\nmode lenient\n{}\n{}in 1 c0 15 0000 0014 1 -> 01\n",
        device_line(1),
        open_lines(1, 0x0110, 4)
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    let (iq, _) = tx_samples(tx_write_size(8e6) / 2);
    let wait = |t: &dyn sdrkit_driver_core::Transmitter, n: u64| {
        let end = Instant::now() + Duration::from_secs(20);
        while t.stats().buffers < n {
            assert!(Instant::now() < end, "{} transfers", t.stats().buffers);
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    for _ in 0..4 {
        assert_eq!(t.push(&iq, Duration::from_secs(1)).unwrap(), iq.len());
    }
    wait(t.as_ref(), 4);
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..60 {
        assert_eq!(t.push(&iq, Duration::from_secs(1)).unwrap(), iq.len());
    }
    wait(t.as_ref(), 64);
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(allocs, 0, "allocations while transmit ran");
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    t.disarm();
    drop(t);
    src.stop();
    bus.check().unwrap();
}
