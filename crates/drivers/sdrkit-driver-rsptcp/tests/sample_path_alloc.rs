//! The sample path of the rsp_tcp driver (server bytes to the ring) allocates nothing once warm, on a fake network.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_rsptcp::{RspTcpDriver, Timing};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

/// Reads the ring for `d` and returns the samples read and the allocations meanwhile in the whole program.
fn drain(r: &mut RingReader, d: Duration) -> (usize, usize) {
    let before = ALLOCS.load(Ordering::Relaxed);
    let t = Instant::now();
    let mut n = 0;
    while t.elapsed() < d {
        n += r.read_with(1 << 16, |_| {});
        std::thread::sleep(Duration::from_millis(1));
    }
    (n, ALLOCS.load(Ordering::Relaxed) - before)
}

/// An rsp_tcp server that greets as an RSP1A with 16-bit samples, then sends bytes as fast as the link takes them.
fn serve(mut c: Box<dyn Conn>) {
    let greeting = [
        b'R', b'S', b'P', b'0', 0, 1, 0, 0, 0, 0, 0, 0x2B, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0, 0, 0, 2,
    ];
    if c.write_all(&greeting).is_err() {
        return;
    }
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut cmd = [0u8; 5];
        while r.read_exact(&mut cmd).is_ok() {}
    });
    // An odd length, so samples split across reads.
    let buf: Vec<u8> = (0..(1 << 16) + 3).map(|i| i as u8).collect();
    while c.write_all(&buf).is_ok() {}
}

#[test]
fn the_sample_path_does_not_allocate() {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 1234, serve);
    let driver = RspTcpDriver::with_timing(
        Arc::new(net),
        Timing {
            window: Duration::from_secs(1000),
            ..Timing::default()
        },
    );
    let kind = DriverKind::new(Box::new(driver.clone()));
    let mut v = Values::defaults(kind.schema());
    v.set(kind.schema().id("rate").unwrap(), &Value::Hz(2e6))
        .unwrap();
    let mut src = driver.open("127.0.0.1:1234", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(300));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("rsp_tcp on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
