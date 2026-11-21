//! The sample path of a SpyServer radio (bytes from the connection to the ring) allocates nothing once warm.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_spyserver::proto::{self, msg, ClientSync, DeviceInfo, Header};
use sdrkit_driver_spyserver::SpyServerDriver;
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

fn message(msg_type: u32, body: &[u8]) -> Vec<u8> {
    let h = Header {
        protocol: proto::PROTOCOL_VERSION,
        msg_type,
        flags: 0,
        stream_type: 1,
        sequence: 0,
        body_size: body.len() as u32,
    };
    let mut v = h.to_bytes().to_vec();
    v.extend_from_slice(body);
    v
}

/// A SpyServer that answers the handshake, then sends 8-bit IQ messages as fast as the link takes them.
fn serve(mut c: Box<dyn Conn>) {
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut b = [0u8; 256];
        while r.read(&mut b).is_ok_and(|n| n > 0) {}
    });
    let sync = ClientSync {
        can_control: true,
        ..Default::default()
    };
    let mut hello = message(msg::DEVICE_INFO, &DeviceInfo::airspy_r2().to_bytes());
    hello.extend(message(msg::CLIENT_SYNC, &sync.to_bytes()));
    if c.write_all(&hello).is_err() {
        return;
    }
    let body: Vec<u8> = (0..1 << 15).map(|i| i as u8).collect();
    let iq = message(msg::UINT8_IQ, &body);
    while c.write_all(&iq).is_ok() {}
}

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

#[test]
fn the_sample_path_does_not_allocate() {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 5555, serve);
    let driver = SpyServerDriver::new(Arc::new(net));
    let kind = DriverKind::new(Box::new(driver.clone()));
    let mut v = Values::defaults(kind.schema());
    v.set(kind.schema().id("rate").unwrap(), &Value::Hz(10e6))
        .unwrap();
    let mut src = driver.open("127.0.0.1:5555", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(300));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("SpyServer on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
