//! The sample path of the KiwiSDR driver (WebSocket frames to the ring) allocates nothing once warm, on a fake network.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Clock, Owner, Value, Values};
use sdrkit_driver_core::ws::accept_for;
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_kiwisdr::{KiwiDriver, Timing};
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

/// A server frame header for a binary payload of `n` bytes (126 to 65535).
fn header(n: usize) -> [u8; 4] {
    let n = n as u16;
    [0x82, 126, (n >> 8) as u8, n as u8]
}

fn send_msg(c: &mut Box<dyn Conn>, text: &str) -> std::io::Result<()> {
    let p = format!("MSG {text}");
    c.write_all(&[0x82, p.len() as u8])?;
    c.write_all(p.as_bytes())
}

const SAMPLES: usize = 512;
const FRAMES: usize = 64;

/// A KiwiSDR that answers the handshake, then sends big-endian IQ frames as fast as the link takes them.
fn serve(mut c: Box<dyn Conn>) {
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        if c.read(&mut b).unwrap_or(0) == 0 {
            return;
        }
        req.push(b[0]);
    }
    let text = String::from_utf8_lossy(&req).into_owned();
    let key = text
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap_or("")
        .trim()
        .to_string();
    let answer = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept_for(&key)
    );
    if c.write_all(answer.as_bytes()).is_err() {
        return;
    }
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while r.read(&mut buf).is_ok_and(|n| n > 0) {}
    });
    for m in [
        "version_maj=1 version_min=800",
        "bandwidth=30000000 freq_offset=0.000",
        "sample_rate=12001.135",
    ] {
        if send_msg(&mut c, m).is_err() {
            return;
        }
    }
    let payload = 20 + SAMPLES * 4;
    let one = 4 + payload;
    let mut buf = vec![0u8; one * FRAMES];
    for f in buf.chunks_exact_mut(one) {
        f[..4].copy_from_slice(&header(payload));
        f[4..8].copy_from_slice(&[b'S', b'N', b'D', 0x08]);
        f[12..14].copy_from_slice(&700u16.to_be_bytes());
        for (i, s) in f[24..].as_chunks_mut::<4>().0.iter_mut().enumerate() {
            s.copy_from_slice(&[(i >> 8) as u8, i as u8, 0x80, 0x01]);
        }
    }
    let mut seq: u32 = 0;
    loop {
        for f in buf.chunks_exact_mut(one) {
            f[8..12].copy_from_slice(&seq.to_le_bytes());
            seq = seq.wrapping_add(1);
        }
        if c.write_all(&buf).is_err() {
            return;
        }
    }
}

#[test]
fn the_sample_path_does_not_allocate() {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 8073, serve);
    let driver = KiwiDriver::with_timing(
        Arc::new(net),
        Clock::fixed(1_790_000_000.0),
        Timing {
            keepalive: Duration::from_secs(1000),
            stall: Duration::from_secs(1000),
            ..Timing::default()
        },
    );
    let kind = DriverKind::new(Box::new(driver.clone()));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    v.set(schema.id("centre_hz").unwrap(), &Value::Hz(7e6))
        .unwrap();
    v.set(schema.id("users").unwrap(), &Value::Bool(false))
        .unwrap();
    let mut src = driver.open("127.0.0.1:8073", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(500));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("KiwiSDR on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
