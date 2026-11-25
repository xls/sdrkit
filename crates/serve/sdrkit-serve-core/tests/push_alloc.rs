//! The feeder with 16 clients at 2.4 MS/s, two of which stop reading: a push allocates nothing after the hub is made, while
//! the two lose chunks, are cut and their rings are retired. Its time per block is measured. Allocations are counted on the
//! producer thread only, inside `push`; the writers and the fake network allocate on their own threads.

use sdrkit_core::net::{Conn, FakeTransport, Transport};
use sdrkit_serve_core::{
    Block, Config, ConnOutlet, Context, Event, Hub, Leave, Listen, Parser, Protocol, RefuseAll,
    SampleType, Session,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Counting;

thread_local! {
    static COUNT_HERE: Cell<bool> = const { Cell::new(false) };
}

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

fn note() {
    if COUNT_HERE.try_with(Cell::get).unwrap_or(false) {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        note();
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        note();
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The source's bytes as they are, no greeting, commands ignored.
struct Raw;

struct Ignore;

impl Parser for Ignore {
    fn feed(&mut self, _bytes: &[u8], _cx: &mut Context<'_>) {}
}

impl Protocol for Raw {
    fn name(&self) -> &'static str {
        "raw"
    }
    fn session(&self, conn: &dyn Conn, source: SampleType) -> Session {
        Session {
            greeting: Vec::new(),
            outlet: Some(Box::new(ConnOutlet::new(conn.try_clone().unwrap(), source))),
            parser: Box::new(Ignore),
        }
    }
}

#[test]
fn sixteen_clients_push_without_allocating() {
    const CLIENTS: usize = 16;
    const STALLED: usize = 2;
    const PER_BLOCK: usize = 16_384;
    let rate = 2.4e6;
    let secs = 3.0;
    let net: Arc<dyn Transport> = Arc::new(FakeTransport::new());
    let b = Block {
        format: SampleType::Cu8,
        centre_hz: 100e6,
        rate_hz: rate,
        lost: 0,
    };
    let (hub, mut feeder) = Hub::new(
        Config {
            max_clients: CLIENTS,
            block_bytes: 2 * PER_BLOCK,
            queue_ms: 200,
            drop_after_s: 0.5,
            ..Config::default()
        },
        &b,
    )
    .unwrap();
    let l = hub
        .listen(
            &net,
            &Listen {
                host: "127.0.0.1".into(),
                port: 0,
                allow: Vec::new(),
            },
            Arc::new(Raw),
            Arc::new(RefuseAll),
        )
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..CLIENTS)
        .map(|i| {
            let stalled = i < STALLED;
            let mut c = net
                .connect("127.0.0.1", l.addr.port(), false, Duration::from_secs(2))
                .unwrap();
            let stop = stop.clone();
            std::thread::spawn(move || {
                let _ = c.set_read_timeout(Some(Duration::from_millis(20)));
                let mut buf = vec![0u8; 1 << 16];
                let mut n = 0usize;
                while stalled && !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                while !stop.load(Ordering::Relaxed) {
                    match c.read(&mut buf) {
                        Ok(0) => break,
                        Ok(k) => n += k,
                        Err(_) => {}
                    }
                }
                n
            })
        })
        .collect();
    let t = Instant::now();
    while hub.snapshot().clients.len() < CLIENTS && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(hub.snapshot().clients.len(), CLIENTS);

    let data: Vec<u8> = (0..2 * PER_BLOCK).map(|i| (i % 251) as u8).collect();
    let blocks = (secs * rate / PER_BLOCK as f64) as usize;
    let block_time = PER_BLOCK as f64 / rate;
    let mut times = Vec::with_capacity(blocks);
    let start = Instant::now();
    for k in 0..blocks {
        let due = Duration::from_secs_f64(k as f64 * block_time);
        if let Some(w) = due.checked_sub(start.elapsed()) {
            std::thread::sleep(w);
        }
        COUNT_HERE.with(|c| c.set(true));
        let t0 = Instant::now();
        feeder.push(&data, &b);
        let dt = t0.elapsed();
        COUNT_HERE.with(|c| c.set(false));
        times.push(dt);
    }
    let allocs = ALLOCS.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(300));
    let snap = hub.snapshot();
    let hub_events = hub.take_events();
    stop.store(true, Ordering::Relaxed);
    drop(hub);
    let received: Vec<usize> = readers.into_iter().map(|r| r.join().unwrap()).collect();

    times.sort();
    let mean = times.iter().sum::<Duration>() / times.len() as u32;
    let p99 = times[times.len() * 99 / 100];
    let max = times[times.len() - 1];
    let dropped: u64 = snap.clients.iter().map(|c| c.dropped_samples).sum();
    let cut = hub_events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::ClientLeft {
                    reason: Leave::TooSlow,
                    ..
                }
            )
        })
        .count();
    let mut got = received.clone();
    got.sort();
    println!(
        "producer: {blocks} blocks of {PER_BLOCK} samples ({:.2} ms of signal each), {CLIENTS} clients: mean {mean:?}, \
         p99 {p99:?}, max {max:?} per block ({:.3} % of the block time at the mean); {cut} stalled clients cut; dropped samples of \
         the others {dropped}; bytes per reading client {}..{}",
        block_time * 1e3,
        mean.as_secs_f64() / block_time * 100.0,
        got[STALLED],
        got[CLIENTS - 1],
    );
    assert_eq!(allocs, 0, "push allocated");
    assert_eq!(snap.pool_misses, 0);
    assert_eq!(cut, STALLED);
    assert_eq!((snap.clients.len(), dropped), (CLIENTS - STALLED, 0));
    assert!(got[STALLED..].iter().all(|n| *n == blocks * 2 * PER_BLOCK));
}
