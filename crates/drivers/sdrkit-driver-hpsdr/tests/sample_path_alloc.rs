//! The sample path of the HPSDR driver (radio packets to the ring) allocates nothing once warm, on a fake network.

use sdrkit_core::net::{DatagramOptions, FakeTransport, Transport};
use sdrkit_core::{Value, Values};
use sdrkit_driver_core::{ring, RingReader, SourceDriver};
use sdrkit_driver_hpsdr::{p1, HpsdrDriver, HERMES_LITE2};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Set on the thread of the fake radio: its allocations are the fake network's, not the driver's.
    static EXEMPT: Cell<bool> = const { Cell::new(false) };
}

fn count() {
    if !EXEMPT.with(Cell::get) {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count();
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        count();
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

/// A protocol 1 radio with one receiver. After the start command it stops listening, so that the fake network does not copy
/// the control packets of the driver, and sends EP6 packets at about 384 ksps.
fn radio(net: FakeTransport, quit: Arc<AtomicBool>) {
    EXEMPT.with(|e| e.set(true));
    let mut listen = net
        .bind(&DatagramOptions::at("127.0.0.1:1024".parse().unwrap()))
        .unwrap();
    listen
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut buf = [0u8; 2048];
    let client = loop {
        if quit.load(Ordering::Acquire) {
            return;
        }
        if let Ok((len, from)) = listen.recv_from(&mut buf) {
            if buf[..len] == p1::command(1) {
                break from;
            }
        }
    };
    drop(listen);
    let mut send = net
        .bind(&DatagramOptions::at("127.0.0.1:0".parse().unwrap()))
        .unwrap();
    let mut pkt = [0u8; p1::PACKET];
    pkt[..4].copy_from_slice(&[0xEF, 0xFE, 0x01, 0x06]);
    for f in 0..2 {
        let o = 8 + f * p1::FRAME;
        pkt[o..o + 3].copy_from_slice(&[0x7F; 3]);
        for s in 0..p1::slots(1) {
            let so = o + 8 + 8 * s;
            pkt[so..so + 6].copy_from_slice(&[0xE0, 0, 0, 0x40, 0, 0]);
        }
    }
    let per = p1::per_packet(1) as f64;
    let start = Instant::now();
    let mut seq: u32 = 0;
    while !quit.load(Ordering::Acquire) {
        let due = (start.elapsed().as_secs_f64() * 384_000.0 / per) as u32;
        while seq < due {
            pkt[4..8].copy_from_slice(&seq.to_be_bytes());
            let _ = send.send_to(&pkt, client);
            seq += 1;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_sample_path_does_not_allocate() {
    let net = FakeTransport::new();
    let quit = Arc::new(AtomicBool::new(false));
    let (n2, q2) = (net.clone(), quit.clone());
    let fake = std::thread::spawn(move || radio(n2, q2));
    let transport: Arc<dyn Transport> = Arc::new(net);
    let driver = HpsdrDriver::new(HERMES_LITE2, transport);
    let schema = driver.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("rate").unwrap(), &Value::Hz(384_000.0))
        .unwrap();
    let mut src = driver.open("127.0.0.1:1024", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    // Past the first throughput window, so that its figures exist.
    drain(&mut r, Duration::from_millis(1300));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    quit.store(true, Ordering::Release);
    fake.join().unwrap();
    eprintln!("HPSDR on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
