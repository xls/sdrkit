//! The shared WebSocket reader and writer allocate nothing in steady state: after a warm-up, 1,000 rounds are counted.

use sdrkit_core::net::{FakeTransport, Transport};
use sdrkit_driver_core::ws::{Link, Reader, Writer, MAX_MESSAGE};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        System.realloc(p, l, n)
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn reading_and_writing_do_not_allocate() {
    let net = FakeTransport::new();
    let mut l = net.listen("127.0.0.1", 0).unwrap();
    let mut c = net
        .connect(
            "127.0.0.1",
            l.local_addr().port(),
            false,
            Duration::from_secs(1),
        )
        .unwrap();
    let mut s = l.accept().unwrap().unwrap();
    c.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let link = Arc::new(Link::new(c, false).unwrap());
    let w = Arc::new(Writer::new(link.clone(), None));
    let mut r = Reader::new(link, w.clone(), MAX_MESSAGE, Duration::from_secs(1));
    // One server batch: a binary message of 4 KiB, a ping, and a text message in two fragments.
    let mut batch = vec![0x82, 0x7e, 0x10, 0x00];
    batch.extend(vec![5u8; 4096]);
    batch.extend([0x89, 0x01, b'p', 0x01, 0x02, b'a', b'b', 0x80, 0x01, b'c']);
    let mut sink = vec![0u8; 1 << 16];
    let mut round = |r: &mut Reader| {
        s.write_all(&batch).unwrap();
        let mut got = 0;
        while got < 2 {
            if let Some(m) = r.poll_message().unwrap() {
                got += 1;
                assert!(!m.bytes().is_empty());
            }
        }
        w.text("SET keepalive").unwrap();
        // The pong (7 bytes) and the text frame (19 bytes) from the client.
        let mut n = 0;
        while n < 7 + 19 {
            n += s.read(&mut sink[n..]).unwrap();
        }
    };
    round(&mut r);
    let before = ALLOCS.load(Ordering::Relaxed);
    for _ in 0..1000 {
        round(&mut r);
    }
    assert_eq!(ALLOCS.load(Ordering::Relaxed) - before, 0);
}
