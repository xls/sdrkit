//! The sample path of an SDR++ server radio (bytes from the connection to the ring) allocates nothing once warm, with and
//! without compression.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_sdrpp::proto::{cmd, pkt};
use sdrkit_driver_sdrpp::SdrppDriver;
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
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

/// The menu of an Airspy source, as an SDR++ server sends it.
const REAL_MENU_HEX: &str = "\
00 00 00 00 80 01 04 16 00 23 23 73 64 72 70 70 5f 73 65 72 76 65 72 5f \
73 72 63 5f 73 65 6c 02 00 00 00 00 04 07 00 41 69 72 73 70 79 00 02 ff \
ff ff ff 00 00 00 00 80 01 04 1f 00 23 23 5f 61 69 72 73 70 79 5f 64 65 \
76 5f 73 65 6c 5f 41 69 72 73 70 79 20 53 6f 75 72 63 65 02 00 00 00 00 \
04 11 00 32 35 38 43 36 32 44 43 32 44 34 33 35 38 38 46 00 02 ff ff ff \
ff 00 80 00 04 1e 00 23 23 5f 61 69 72 73 70 79 5f 73 72 5f 73 65 6c 5f \
41 69 72 73 70 79 20 53 6f 75 72 63 65 02 00 00 00 00 04 0f 00 31 30 2e \
30 4d 48 7a 00 32 2e 35 4d 48 7a 00 02 ff ff ff ff 00 01 00 00 00 00 00 \
81 01 04 23 00 52 65 66 72 65 73 68 23 23 5f 61 69 72 73 70 79 5f 72 65 \
66 72 5f 41 69 72 73 70 79 20 53 6f 75 72 63 65 03 00 00 00 00 03 00 00 \
00 00 00 85 00 00 82 00 02 03 00 00 00 04 25 00 41 69 72 73 70 79 47 61 \
69 6e 4d 6f 64 65 43 6f 6c 75 6d 6e 73 23 23 5f 41 69 72 73 70 79 20 53 \
6f 75 72 63 65 01 00 00 84 01 04 23 00 53 65 6e 73 69 74 69 76 65 23 23 \
5f 61 69 72 73 70 79 5f 67 6d 5f 41 69 72 73 70 79 20 53 6f 75 72 63 65 \
01 00 00 83 00 00 84 01 04 20 00 4c 69 6e 65 61 72 23 23 5f 61 69 72 73 \
70 79 5f 67 6d 5f 41 69 72 73 70 79 20 53 6f 75 72 63 65 01 01 00 83 00 \
00 84 01 04 1e 00 46 72 65 65 23 23 5f 61 69 72 73 70 79 5f 67 6d 5f 41 \
69 72 73 70 79 20 53 6f 75 72 63 65 01 00 00 82 00 02 01 00 00 00 04 28 \
00 45 6e 64 41 69 72 73 70 79 47 61 69 6e 4d 6f 64 65 43 6f 6c 75 6d 6e \
73 23 23 5f 41 69 72 73 70 79 20 53 6f 75 72 63 65 01 00 00 86 00 00 87 \
00 04 04 00 47 61 69 6e 00 00 00 00 88 00 04 20 00 23 23 5f 61 69 72 73 \
70 79 5f 6c 69 6e 5f 67 61 69 6e 5f 41 69 72 73 70 79 20 53 6f 75 72 63 \
65 02 12 00 00 00 02 00 00 00 00 02 15 00 00 00 02 01 00 00 00 02 00 00 \
00 00 00 8b 00 04 1d 00 42 69 61 73 20 54 23 23 5f 61 69 72 73 70 79 5f \
41 69 72 73 70 79 20 53 6f 75 72 63 65 01 00 \
";

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

fn packet(kind: u32, body: &[u8]) -> Vec<u8> {
    let mut p = kind.to_le_bytes().to_vec();
    p.extend(((8 + body.len()) as u32).to_le_bytes());
    p.extend(body);
    p
}

fn command(kind: u32, c: u32, arg: &[u8]) -> Vec<u8> {
    let mut b = c.to_le_bytes().to_vec();
    b.extend(arg);
    packet(kind, &b)
}

/// An SDR++ server that answers the commands of the driver, then sends int16 baseband packets as fast as the link accepts
/// them, compressed when the driver requests it.
fn serve(mut c: Box<dyn Conn>) {
    let menu = hex(REAL_MENU_HEX);
    let (tx, rx) = mpsc::channel::<(u32, Vec<u8>)>();
    let mut r = c.try_clone().unwrap();
    std::thread::spawn(move || loop {
        let mut h = [0u8; 8];
        if r.read_exact(&mut h).is_err() {
            return;
        }
        let size = u32::from_le_bytes([h[4], h[5], h[6], h[7]]) as usize;
        let mut b = vec![0u8; size.saturating_sub(8)];
        if r.read_exact(&mut b).is_err() || b.len() < 4 {
            return;
        }
        let id = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        if tx.send((id, b[4..].to_vec())).is_err() {
            return;
        }
    });
    if c.write_all(&command(
        pkt::COMMAND,
        cmd::SET_SAMPLERATE,
        &2.5e6f64.to_le_bytes(),
    ))
    .is_err()
    {
        return;
    }
    let mut body = Vec::new();
    body.extend(0u16.to_le_bytes());
    body.extend(1u16.to_le_bytes());
    body.extend(1.0f32.to_le_bytes());
    for i in 0..1 << 14 {
        body.extend((i as i16).to_le_bytes());
        body.extend((-(i as i16)).to_le_bytes());
    }
    let plain = packet(pkt::BASEBAND, &body);
    let z =
        ruzstd::encoding::compress_to_vec(&body[..], ruzstd::encoding::CompressionLevel::Fastest);
    let packed = packet(pkt::BASEBAND_COMPRESSED, &z);
    let (mut running, mut compress) = (false, false);
    loop {
        while let Ok((id, arg)) = rx.try_recv() {
            let answer = match id {
                cmd::GET_UI => Some(command(pkt::COMMAND_ACK, id, &menu)),
                cmd::UI_ACTION if arg.first() == Some(&1) => {
                    Some(command(pkt::COMMAND_ACK, id, &menu))
                }
                cmd::SET_FREQUENCY => Some(command(pkt::COMMAND_ACK, id, &[])),
                cmd::START => {
                    running = true;
                    None
                }
                cmd::STOP => {
                    running = false;
                    None
                }
                cmd::SET_COMPRESSION => {
                    compress = arg.first() == Some(&1);
                    None
                }
                _ => None,
            };
            if answer.is_some_and(|a| c.write_all(&a).is_err()) {
                return;
            }
        }
        if running {
            if c.write_all(if compress { &packed } else { &plain })
                .is_err()
            {
                return;
            }
        } else {
            std::thread::sleep(Duration::from_millis(2));
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

fn run(compression: bool) -> (usize, usize) {
    let net = FakeTransport::new();
    net.serve("127.0.0.1", 5259, serve);
    let driver = SdrppDriver::new(Arc::new(net));
    let kind = DriverKind::new(Box::new(driver.clone()));
    let mut v = Values::defaults(kind.schema());
    v.set(kind.schema().id("rate").unwrap(), &Value::Hz(2.5e6))
        .unwrap();
    v.set(
        kind.schema().id("compression").unwrap(),
        &Value::Bool(compression),
    )
    .unwrap();
    let mut src = driver.open("127.0.0.1:5259", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(1000));
    let got = drain(&mut r, Duration::from_millis(500));
    src.stop();
    got
}

#[test]
fn the_sample_path_does_not_allocate() {
    static ONE: AtomicBool = AtomicBool::new(false);
    assert!(!ONE.swap(true, Ordering::Relaxed));
    let (n, allocs) = run(false);
    eprintln!("SDR++ server on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
    let (n, allocs) = run(true);
    eprintln!("SDR++ server with compression: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
