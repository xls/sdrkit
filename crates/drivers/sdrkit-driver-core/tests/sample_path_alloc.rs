//! The sample path of a driver (device bytes to the ring) allocates nothing once warm: the test signal, and an rtl_tcp radio
//! on a fake network. Both run in one test, so no other test allocates while one counts.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Complex32, Generator, Hz, Owner, Value, Values};
use sdrkit_driver_core::rtl_tcp::RtlTcpDriver;
use sdrkit_driver_core::synth::SynthDriver;
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_graph::stream::SampleType;
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

/// A tone at a tenth of the rate.
struct Tone(u64);

impl Generator for Tone {
    fn rate_hz(&self) -> Hz {
        1e6
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            *o = Complex32::from_polar(0.5, self.0 as f32 * 0.628);
            self.0 += 1;
        }
        out.len()
    }
}

fn values(driver: impl SourceDriver + 'static, set: &[(&str, Value)]) -> Values {
    let kind = DriverKind::new(Box::new(driver));
    let mut v = Values::defaults(kind.schema());
    for (key, value) in set {
        v.set(kind.schema().id(key).unwrap(), value).unwrap();
    }
    v
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

fn synth() -> (usize, usize) {
    let driver = SynthDriver::new(Arc::new(|_: &str, _: &Values, _| {
        Some(Box::new(Tone(0)) as Box<dyn Generator>)
    }));
    let set = [
        ("rate", Value::Hz(1e6)),
        ("benchmark", Value::Bool(true)),
        ("quantize", Value::Bool(true)),
    ];
    let v = values(driver.clone(), &set);
    let mut src = driver.open("", &v).unwrap();
    let (w, mut r) = ring(1 << 20, SampleType::Cf32);
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(200));
    let got = drain(&mut r, Duration::from_millis(400));
    src.stop();
    got
}

/// An rtl_tcp server that greets, then sends bytes as fast as the link takes them.
fn serve(mut c: Box<dyn Conn>) {
    if c.write_all(b"RTL0\0\0\0\x05\0\0\0\x1d").is_err() {
        return;
    }
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || {
        let mut cmd = [0u8; 5];
        while r.read_exact(&mut cmd).is_ok() {}
    });
    let buf: Vec<u8> = (0..1 << 16).map(|i| i as u8).collect();
    while c.write_all(&buf).is_ok() {}
}

fn rtl_tcp() -> (usize, usize) {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 1234, serve);
    let driver = RtlTcpDriver::with_timing(
        Arc::new(net),
        Duration::from_secs(1000),
        Duration::from_secs(3),
    );
    let v = values(driver.clone(), &[("rate", Value::Hz(2.4e6))]);
    let mut src = driver.open("127.0.0.1:1234", &v).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(300));
    let got = drain(&mut r, Duration::from_millis(500));
    src.stop();
    got
}

#[test]
fn the_sample_paths_do_not_allocate() {
    let (n, allocs) = synth();
    eprintln!("test signal: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
    let (n, allocs) = rtl_tcp();
    eprintln!("rtl_tcp on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
