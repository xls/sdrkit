//! The sample path of an OpenWebRX radio (bytes from the connection to the ring) allocates nothing once warm.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Clock, Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_openwebrx::OpenWebRxDriver;
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

const CONFIG: &str = r#"{"type":"config","value":{"sdr_id":"rtl","profile_id":"20m","center_freq":14100000,"samp_rate":250000,"fft_size":1024,"audio_compression":"adpcm","fft_compression":"adpcm","max_clients":20,"start_offset_freq":74000}}"#;

/// An OpenWebRX server that answers the upgrade, greets, sends its configuration, then sends ADPCM audio with sync headers
/// and a waterfall line now and then, as fast as the link takes it.
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
    out.extend(frame(1, b"CLIENT DE SERVER server=openwebrx version=1.2.2"));
    out.extend(frame(1, CONFIG.as_bytes()));
    if c.write_all(&out).is_err() {
        return;
    }
    let mut audio = vec![0x02];
    audio.extend_from_slice(b"SYNC\x00\x00\x00\x00");
    audio.extend((0..1000).map(|i| i as u8));
    let audio = frame(2, &audio);
    let mut line = vec![0x01];
    line.extend(std::iter::repeat_n(0x11u8, 517));
    let line = frame(2, &line);
    for i in 0u64.. {
        let f = if i % 16 == 0 { &line } else { &audio };
        if c.write_all(f).is_err() {
            return;
        }
    }
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
    net.serve("127.0.0.1", 8073, serve);
    let driver = OpenWebRxDriver::new(Arc::new(net), Clock::fixed(0.0));
    let kind = DriverKind::new(Box::new(driver.clone()));
    let mut v = Values::defaults(kind.schema());
    v.set(
        kind.schema().id("centre_hz").unwrap(),
        &Value::Hz(14_176_700.0),
    )
    .unwrap();
    let mut src = driver.open("127.0.0.1:8073", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(2500));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("OpenWebRX on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
