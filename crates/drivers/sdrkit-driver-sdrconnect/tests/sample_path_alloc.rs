//! The sample path of an SDRconnect radio (bytes from the connection to the ring) allocates nothing once warm.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_sdrconnect::SdrconnectDriver;
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

/// A server frame (never masked).
fn frame(op: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | op, 127];
    f.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    f.extend_from_slice(payload);
    f
}

fn push(property: &str, value: &str) -> Vec<u8> {
    let t = format!(
        r#"{{"event_type":"property_changed","property":"{property}","device":"primary","value":"{value}"}}"#
    );
    frame(1, t.as_bytes())
}

/// An SDRconnect server that answers the upgrade, reports its properties without control, then sends IQ as fast as the link
/// takes it.
fn serve(mut c: Box<dyn Conn>) {
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        if c.read(&mut b).ok() != Some(1) {
            return;
        }
        req.push(b[0]);
    }
    let req = String::from_utf8_lossy(&req).into_owned();
    let key = req
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap_or("");
    let answer = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        sdrkit_driver_core::ws::accept_for(key)
    );
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut b = [0u8; 4096];
        while r.read(&mut b).is_ok_and(|n| n > 0) {}
    });
    let mut out = answer.into_bytes();
    for (p, v) in [
        ("api_version", "1.0.3"),
        ("can_control", "false"),
        ("started", "true"),
        ("device_sample_rate", "2000000"),
        ("device_center_frequency", "100000000"),
        ("lna_state_min", "0"),
        ("lna_state_max", "9"),
        ("valid_antennas", "A"),
    ] {
        out.extend(push(p, v));
    }
    if c.write_all(&out).is_err() {
        return;
    }
    let mut p = 2u16.to_le_bytes().to_vec();
    p.extend((0..1 << 15).map(|i| i as u8));
    let iq = frame(2, &p);
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
    net.serve("127.0.0.1", 5454, serve);
    let driver = SdrconnectDriver::new(Arc::new(net));
    let kind = DriverKind::new(Box::new(driver.clone()));
    let mut v = Values::defaults(kind.schema());
    v.set(kind.schema().id("rate").unwrap(), &Value::Hz(2e6))
        .unwrap();
    let mut src = driver.open("127.0.0.1:5454", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(1500));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("SDRconnect on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
