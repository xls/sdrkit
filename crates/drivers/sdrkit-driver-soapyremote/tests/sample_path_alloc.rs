//! The sample path of a SoapyRemote radio (UDP datagrams or TCP messages to the ring) allocates nothing once warm, on a fake
//! network.

use core::net::SocketAddr;
use sdrkit_core::net::{Conn, DatagramOptions, FakeTransport, Transport};
use sdrkit_core::Choice;
use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RingReader, SourceDriver};
use sdrkit_driver_soapyremote::rpc::{self, call, Packer, Range, Unpacker};
use sdrkit_driver_soapyremote::stream::{DgHeader, HEADER_LEN};
use sdrkit_driver_soapyremote::{SoapyRemoteDriver, StreamProt};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    /// Set on the threads of the fake server: their allocations are the fake network's, not the driver's.
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

/// The port the server provides for its data stream. No socket is bound there, so the acknowledgements of the driver go
/// nowhere and the fake network copies nothing for them.
const NOWHERE: u16 = 7;
/// CS16 samples per datagram at an MTU of 1500.
const PER_DATAGRAM: usize = 357;

/// Sends CS16 datagrams to `127.0.0.1:port` at about 700 ksps, until the driver's socket is gone for a while.
fn stream(net: FakeTransport, port: u16) {
    EXEMPT.with(|e| e.set(true));
    let mut s = net
        .bind(&DatagramOptions::at(SocketAddr::from(([127, 0, 0, 1], 0))))
        .unwrap();
    let to = SocketAddr::from(([127, 0, 0, 1], port));
    send_paced(|d| {
        let _ = s.send_to(d, to);
    });
}

/// Sends the same CS16 messages on the TCP data connection `c`, and reads the driver's acknowledgements on a thread of its
/// own so they cost the driver nothing.
fn stream_tcp(mut c: Box<dyn Conn>) {
    EXEMPT.with(|e| e.set(true));
    let mut acks = c.try_clone().unwrap();
    std::thread::spawn(move || {
        EXEMPT.with(|e| e.set(true));
        let mut b = [0u8; 4096];
        while matches!(std::io::Read::read(&mut acks, &mut b), Ok(n) if n > 0) {}
    });
    send_paced(|d| {
        let _ = c.write_all(d);
    });
}

/// Calls `send` with 4000 CS16 messages of [`PER_DATAGRAM`] samples, one every 500 us.
fn send_paced(mut send: impl FnMut(&[u8])) {
    let mut d = vec![0u8; HEADER_LEN + PER_DATAGRAM * 4];
    for (i, b) in d[HEADER_LEN..].iter_mut().enumerate() {
        *b = i as u8;
    }
    let start = Instant::now();
    for seq in 0u32..4000 {
        let h = DgHeader {
            bytes: d.len() as u32,
            seq,
            elems: PER_DATAGRAM as i32,
            flags: 0,
            time: 0,
        };
        d[..HEADER_LEN].copy_from_slice(&h.to_bytes());
        send(&d);
        let due = start + Duration::from_micros(500 * u64::from(seq + 1));
        if let Some(w) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(w);
        }
    }
}

fn reply(n: u32, args: &[u8], net: &FakeTransport) -> Vec<u8> {
    let mut p = Packer::default();
    match n {
        call::SERVER_ID => {
            p.string("fake");
        }
        call::NUM_CHANNELS => {
            p.i32(1);
        }
        call::NATIVE_FORMAT => {
            p.string("CS16").f64(32768.0);
        }
        call::STREAM_FORMATS => {
            p.string_list(&["CS16"]);
        }
        call::SAMPLE_RATE_RANGE => {
            p.range_list(&[Range::new(1e6, 10e6, 0.0)], rpc::VERSION);
        }
        call::FREQUENCY_RANGE => {
            p.range_list(&[Range::new(24e6, 1.7e9, 0.0)], rpc::VERSION);
        }
        call::GET_SAMPLE_RATE => {
            p.f64(2.4e6);
        }
        call::ACTIVATE_STREAM | call::DEACTIVATE_STREAM => {
            p.i32(0);
        }
        call::SETUP_STREAM => {
            let mut u = Unpacker::new(args, rpc::VERSION);
            let _ = (u.char(), u.string(), u.size_list(), u.kwargs());
            let port: u16 = u.string().unwrap().parse().unwrap();
            let n2 = net.clone();
            std::thread::spawn(move || stream(n2, port));
            p.i32(0).string(&NOWHERE.to_string());
        }
        call::MAKE
        | call::UNMAKE
        | call::HANGUP
        | call::SET_SAMPLE_RATE
        | call::SET_FREQUENCY
        | call::SET_GAIN
        | call::SET_GAIN_MODE
        | call::SET_BANDWIDTH
        | call::SET_FREQ_CORRECTION => {
            p.void();
        }
        _ => {
            p.exception("not supported");
        }
    }
    p.buf
}

/// The set-up of a TCP stream: it listens, sends the port as the early reply, takes the data and the status connection, and
/// streams on the data connection. It returns the usual reply.
fn setup_tcp(c: &mut dyn Conn, net: &FakeTransport) -> Vec<u8> {
    let mut l = net.listen("127.0.0.1", 0).unwrap();
    let mut early = Packer::default();
    early.string(&l.local_addr().port().to_string());
    c.write_all(&rpc::packet(&early.buf, rpc::VERSION)).unwrap();
    let mut got = Vec::new();
    while got.len() < 2 {
        match l.accept().unwrap() {
            Some(x) => got.push(x),
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    let status = got.pop().unwrap();
    let data = got.pop().unwrap();
    std::thread::spawn(move || {
        let _keep = status;
        stream_tcp(data);
    });
    let mut p = Packer::default();
    p.i32(0).string("0");
    p.buf
}

/// A SoapyRemote server with one CS16 radio.
fn serve(mut c: Box<dyn Conn>, net: &FakeTransport) {
    EXEMPT.with(|e| e.set(true));
    while let Ok(pkt) = rpc::read_packet(c.as_mut(), Duration::from_secs(30)) {
        let mut u = Unpacker::new(&pkt.payload, pkt.version);
        let Ok(n) = u.call() else { return };
        let args = &pkt.payload[pkt.payload.len() - u.remaining()..];
        let tcp = n == call::SETUP_STREAM && {
            let mut a = Unpacker::new(args, rpc::VERSION);
            let _ = (a.char(), a.string(), a.size_list());
            a.kwargs()
                .is_ok_and(|kw| kw.get("remote:prot").is_some_and(|p| p == "tcp"))
        };
        if tcp {
            let r = setup_tcp(c.as_mut(), net);
            if c.write_all(&rpc::packet(&r, rpc::VERSION)).is_err() {
                return;
            }
            continue;
        }
        let r = reply(n, args, net);
        if c.write_all(&rpc::packet(&r, rpc::VERSION)).is_err() || n == call::HANGUP {
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

/// UDP, then TCP. One test, so that no other test allocates while one measures.
#[test]
fn the_sample_path_does_not_allocate() {
    sample_path(StreamProt::Udp);
    sample_path(StreamProt::Tcp);
}

fn sample_path(prot: StreamProt) {
    let net = FakeTransport::new();
    let n2 = net.clone();
    net.serve("127.0.0.1", 55132, move |c| serve(c, &n2));
    let driver = SoapyRemoteDriver::new(Arc::new(net));
    let kind = DriverKind::new(Box::new(driver.clone()));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    for (key, value) in [
        ("rate", Value::Hz(2.4e6)),
        ("agc", Value::Bool(false)),
        ("serverlog", Value::Bool(false)),
        ("transport", Value::Choice(prot.index())),
    ] {
        v.set(schema.id(key).unwrap(), &value).unwrap();
    }
    let mut src = driver.open("127.0.0.1:55132", &v).unwrap();
    let (w, mut r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    drain(&mut r, Duration::from_millis(300));
    let (n, allocs) = drain(&mut r, Duration::from_millis(500));
    src.stop();
    eprintln!("SoapyRemote {prot:?} on a fake network: {n} samples, {allocs} allocations");
    assert!(n > 100_000, "{n}");
    assert_eq!(allocs, 0);
}
