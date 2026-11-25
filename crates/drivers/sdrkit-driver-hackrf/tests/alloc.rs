//! Once receive runs, the path from the bulk reads to the ring allocates nothing.

mod common;

use common::*;
use sdrkit_driver_usb::{BulkItem, FakeUsb, Script, UsbId};
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
fn the_receive_path_allocates_nothing_after_start() {
    // USB API 0x0102: no clock status and no streaming state requests while it streams.
    let api = 0x0102;
    let text = format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0000 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        rx_start_lines(1, api)
    );
    let mut script = Script::parse(&text).unwrap();
    script.bulk(BulkItem {
        repeat: 2000,
        ..BulkItem::data(UsbId(1), 0x81, block(7, 262_144))
    });
    let bus = FakeUsb::new(script);
    let mut src = open(&bus, &example_values());
    let (w, _r) = sdrkit_driver_core::ring(1 << 25, sdrkit_graph::stream::SampleType::Cs8);
    let stats = w.stats();
    src.start(w).unwrap();
    let wait = |n: u64| {
        let end = Instant::now() + Duration::from_secs(20);
        while stats.written() < n {
            assert!(Instant::now() < end, "{} samples", stats.written());
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    wait(4 << 20);
    let before = ALLOCS.load(Ordering::Relaxed);
    wait(24 << 20);
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    assert_eq!(allocs, 0, "allocations while receive ran");
    src.stop();
    bus.check().unwrap();
}
