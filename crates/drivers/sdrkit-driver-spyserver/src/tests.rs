//! The driver against a fake SpyServer on a fake network. The byte strings are transcripts of the protocol.

use super::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Choice, Complex32, Owner, Results, Value};
use sdrkit_driver_core::{own_id, RadioSetting};
use sdrkit_driver_core::{ring, DriverKind, Protocol, Retuned, RingReader, Started};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// The bytes 200 and 50 of a Cu8 ring as I and Q.
const U8_I: f32 = 72.5 / 128.0;
const U8_Q: f32 = -77.5 / 128.0;

const HOST: &str = "127.0.0.1";
const PORT: u16 = 5555;

const M: f64 = 1e6;

fn hex(s: &str) -> Vec<u8> {
    s.split_whitespace()
        .map(|b| u8::from_str_radix(b, 16).unwrap())
        .collect()
}

/// DEVICE_INFO of an Airspy R2.
const DEVICE_R2: &str = "a4 06 00 02  00 00 00 00  00 00 00 00  00 00 00 00  30 00 00 00
    01 00 00 00  cd ab 34 12  80 96 98 00  00 12 7a 00
    09 00 00 00  01 00 00 00  15 00 00 00  00 36 6e 01
    00 d2 49 6b  0c 00 00 00  00 00 00 00  00 00 00 00";

/// the Airspy HF+ body.
const BODY_HF: &str = "02 00 00 00  07 00 00 00  00 b8 0b 00  c0 27 09 00
    06 00 00 00  03 00 00 00  03 00 00 00  a0 86 01 00
    c0 05 d9 01  10 00 00 00  01 00 00 00  00 00 00 00";

/// CLIENT_SYNC, in control.
const SYNC: &str = "a4 06 00 02  01 00 00 00  00 00 00 00  01 00 00 00  24 00 00 00
    01 00 00 00  08 00 00 00  00 e1 f5 05  00 e1 f5 05
    00 e1 f5 05  c0 9e e6 05  40 23 05 06  c0 9e e6 05
    40 23 05 06";

const HELLO: &str = "00 00 00 00  0a 00 00 00  a4 06 00 02  73 64 72 6b 69 74";

fn sync_read_only() -> Vec<u8> {
    let mut v = hex(SYNC);
    v[20] = 0;
    v
}

fn message(msg_type: u32, stream_type: u32, seq: u32, body: &[u8]) -> Vec<u8> {
    let h = proto::Header {
        protocol: proto::PROTOCOL_VERSION,
        msg_type,
        flags: 0,
        stream_type,
        sequence: seq,
        body_size: body.len() as u32,
    };
    let mut v = h.to_bytes().to_vec();
    v.extend_from_slice(body);
    v
}

fn device_msg(d: &DeviceInfo) -> Vec<u8> {
    message(0, 0, 0, &d.to_bytes())
}

fn sync_msg(s: &ClientSync) -> Vec<u8> {
    message(1, 0, 1, &s.to_bytes())
}

// ---- The fake server ----

#[derive(Clone, PartialEq)]
enum Behave {
    Normal,
    /// Accepts, then says nothing.
    Silent,
    /// Closes right after HELLO (a full server).
    CloseAfterHello,
    /// Sends these bytes at once, then says nothing.
    Raw(Vec<u8>),
    /// Answers HELLO with DEVICE_INFO only.
    DeviceInfoOnly,
}

#[derive(Clone)]
struct FakeOpts {
    behave: Behave,
    /// How connections after the first behave.
    behave_later: Arc<Mutex<Option<Behave>>>,
    device: Vec<u8>,
    sync: Vec<u8>,
    /// Sends 1024 pairs every 2 ms while streaming.
    periodic: bool,
    /// Bytes sent once, on the first connection, when streaming is turned on.
    script: Vec<u8>,
    /// The first connection is dropped this long after streaming starts.
    drop_first_after: Option<Duration>,
    /// The first connection goes quiet (but stays open) this long after streaming starts.
    quiet_first_after: Option<Duration>,
}

impl Default for FakeOpts {
    fn default() -> Self {
        FakeOpts {
            behave: Behave::Normal,
            behave_later: Arc::default(),
            device: hex(DEVICE_R2),
            sync: hex(SYNC),
            periodic: true,
            script: Vec::new(),
            drop_first_after: None,
            quiet_first_after: None,
        }
    }
}

#[derive(Default)]
struct Conn {
    raw: Mutex<Vec<u8>>,
    settings: Mutex<Vec<(u32, u32)>>,
    hello: AtomicBool,
    streaming: AtomicBool,
    format: AtomicU32,
    closed: AtomicBool,
    at: Mutex<Option<Instant>>,
}

struct Fake {
    net: FakeTransport,
    conns: Arc<Mutex<Vec<Arc<Conn>>>>,
    outbox: Arc<Mutex<Vec<u8>>>,
    stop: Arc<AtomicBool>,
}

impl Fake {
    fn start(opts: FakeOpts) -> Fake {
        let net = FakeTransport::new();
        let conns: Arc<Mutex<Vec<Arc<Conn>>>> = Default::default();
        let outbox: Arc<Mutex<Vec<u8>>> = Default::default();
        let stop = Arc::new(AtomicBool::new(false));
        let n = AtomicUsize::new(0);
        let (c2, s2, o2) = (conns.clone(), stop.clone(), outbox.clone());
        net.serve(HOST, PORT, move |s| {
            let first = n.fetch_add(1, Ordering::Relaxed) == 0;
            let conn = Arc::new(Conn::default());
            *conn.at.lock().unwrap() = Some(Instant::now());
            c2.lock().unwrap().push(conn.clone());
            serve(s, conn, opts.clone(), first, s2.clone(), o2.clone());
        });
        Fake {
            net,
            conns,
            outbox,
            stop,
        }
    }

    fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }

    fn driver(&self) -> SpyServerDriver {
        SpyServerDriver::new(Arc::new(self.net.clone()))
    }

    fn driver_with(&self, timing: Timing) -> SpyServerDriver {
        SpyServerDriver::with_timing(Arc::new(self.net.clone()), timing)
    }

    fn conn(&self, i: usize) -> Option<Arc<Conn>> {
        self.conns.lock().unwrap().get(i).cloned()
    }

    fn count(&self) -> usize {
        self.conns.lock().unwrap().len()
    }

    fn raw(&self, i: usize) -> Vec<u8> {
        self.conn(i)
            .map(|c| c.raw.lock().unwrap().clone())
            .unwrap_or_default()
    }

    fn settings(&self, i: usize) -> Vec<(u32, u32)> {
        self.conn(i)
            .map(|c| c.settings.lock().unwrap().clone())
            .unwrap_or_default()
    }

    fn send(&self, bytes: &[u8]) {
        self.outbox.lock().unwrap().extend_from_slice(bytes);
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.net.stop_serving(HOST, PORT);
    }
}

fn reader(mut r: Box<dyn sdrkit_core::net::Conn>, conn: Arc<Conn>) {
    let mut b = [0u8; 4096];
    let mut pending: Vec<u8> = Vec::new();
    loop {
        match r.read(&mut b) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                conn.raw.lock().unwrap().extend_from_slice(&b[..n]);
                pending.extend_from_slice(&b[..n]);
                // While a whole command is buffered, take it.
                while pending.len() >= 8 {
                    let w = |i: usize| {
                        u32::from_le_bytes([
                            pending[i],
                            pending[i + 1],
                            pending[i + 2],
                            pending[i + 3],
                        ])
                    };
                    let (t, size) = (w(0), w(4) as usize);
                    if pending.len() < 8 + size {
                        break;
                    }
                    match t {
                        0 => conn.hello.store(true, Ordering::Release),
                        2 if size >= 8 => {
                            let (id, v) = (w(8), w(12));
                            conn.settings.lock().unwrap().push((id, v));
                            match id {
                                1 => conn.streaming.store(v == 1, Ordering::Release),
                                100 => conn.format.store(v, Ordering::Release),
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                    pending.drain(..8 + size);
                }
            }
        }
    }
    conn.closed.store(true, Ordering::Release);
}

fn iq_message(format: u32, seq: u32) -> Vec<u8> {
    let (t, body): (u32, Vec<u8>) = if format == proto::format::INT16 {
        (
            101,
            (0..1024)
                .flat_map(|_| {
                    let mut p = 16384i16.to_le_bytes().to_vec();
                    p.extend_from_slice(&(-8192i16).to_le_bytes());
                    p
                })
                .collect(),
        )
    } else {
        (100, (0..1024).flat_map(|_| [200u8, 50]).collect())
    };
    message(t, 1, seq, &body)
}

fn serve(
    mut s: Box<dyn sdrkit_core::net::Conn>,
    conn: Arc<Conn>,
    o: FakeOpts,
    first: bool,
    stop: Arc<AtomicBool>,
    outbox: Arc<Mutex<Vec<u8>>>,
) {
    let behave = if first {
        o.behave.clone()
    } else {
        o.behave_later
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(o.behave.clone())
    };
    {
        let r = s.try_clone().unwrap();
        let c = conn.clone();
        std::thread::spawn(move || reader(r, c));
    }
    let done = |c: &Conn| stop.load(Ordering::Relaxed) || c.closed.load(Ordering::Acquire);
    if let Behave::Raw(b) = &behave {
        let _ = s.write_all(b);
    }
    while !conn.hello.load(Ordering::Acquire) {
        if done(&conn) {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    match behave {
        Behave::Silent | Behave::Raw(_) => {
            while !done(&conn) {
                std::thread::sleep(Duration::from_millis(5));
            }
            return;
        }
        Behave::CloseAfterHello => {
            s.shutdown();
            return;
        }
        Behave::DeviceInfoOnly => {
            let _ = s.write_all(&o.device);
            while !done(&conn) {
                std::thread::sleep(Duration::from_millis(5));
            }
            return;
        }
        Behave::Normal => {}
    }
    let _ = s.write_all(&o.device);
    let _ = s.write_all(&o.sync);
    let mut seq = 2;
    let mut script_sent = false;
    let mut streaming_since: Option<Instant> = None;
    loop {
        if done(&conn) {
            s.shutdown();
            return;
        }
        let out = std::mem::take(&mut *outbox.lock().unwrap());
        if !out.is_empty() && s.write_all(&out).is_err() {
            return;
        }
        if conn.streaming.load(Ordering::Acquire) {
            let since = *streaming_since.get_or_insert_with(Instant::now);
            if first && o.drop_first_after.is_some_and(|d| since.elapsed() >= d) {
                s.shutdown();
                return;
            }
            let quiet = first && o.quiet_first_after.is_some_and(|d| since.elapsed() >= d);
            if first && !script_sent {
                script_sent = true;
                if s.write_all(&o.script).is_err() {
                    return;
                }
            }
            if o.periodic && !quiet {
                let m = iq_message(conn.format.load(Ordering::Acquire), seq);
                seq += 1;
                if s.write_all(&m).is_err() {
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

// ---- Helpers ----

/// The values of a radio at `centre` and `rate`. `gain`: a fixed gain index, or `None` for the gain control of the server.
fn tune(centre: Hz, rate: Hz, gain: Option<f64>) -> Values {
    let kind = DriverKind::new(Box::new(SpyServerDriver::new(Arc::new(
        FakeTransport::new(),
    ))));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    let mut set = |key: &str, value: Value| v.set(schema.id(key).unwrap(), &value).unwrap();
    set("centre_hz", Value::Hz(centre));
    set("rate", Value::Hz(rate));
    set("agc", Value::Bool(gain.is_none()));
    if let Some(g) = gain {
        set("gain", Value::F64(g));
    }
    v
}

fn with_format(mut v: Values, f: WireFormat) -> Values {
    v.set(own_id(SpySetting::Format), &Value::Choice(f.index()))
        .unwrap();
    v
}

fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

/// The log lines since the last poll.
fn notes(src: &mut Box<dyn Source>) -> Vec<String> {
    let mut r = results();
    src.poll(&mut r);
    r.log_lines().iter().cloned().collect()
}

fn wait_for(mut what: impl FnMut() -> bool, secs: f64) -> bool {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < secs {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    what()
}

/// Polls until a log line holds `text`, within `secs`.
fn wait_log(src: &mut Box<dyn Source>, secs: f64, text: &str) -> bool {
    wait_for(|| notes(src).iter().any(|l| l.contains(text)), secs)
}

fn read_n(r: &mut RingReader, n: usize, secs: f64) -> Vec<Complex32> {
    let mut v = vec![Complex32::default(); n];
    let mut got = 0;
    let t = Instant::now();
    while got < n && t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(10));
        got += r.read_iq(&mut v[got..]);
    }
    v.truncate(got);
    v
}

fn near(c: Complex32, re: f32, im: f32, tol: f32) -> bool {
    (c.re - re).abs() <= tol && (c.im - im).abs() <= tol
}

fn cmd(id: u32, v: u32) -> Vec<u8> {
    proto::set_setting(id, v).to_vec()
}

fn start(src: &mut Box<dyn Source>, capacity: usize) -> (Started, RingReader) {
    let (w, r) = ring(capacity, src.sample_type());
    (src.start(w).unwrap(), r)
}

fn live(r: Result<Retuned, DriverError>) -> Started {
    match r.unwrap() {
        Retuned::Live(s) => s,
        other => panic!("{other:?}"),
    }
}

/// The error of this crate inside `e`.
fn spy(e: &DriverError) -> Option<SpyError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<SpyError>().copied(),
        _ => None,
    }
}

fn refused(id: sdrkit_core::SettingId) -> DriverError {
    DriverError::Refused { id }
}

#[test]
fn opens_in_control_with_the_exact_bytes_and_streams() {
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let caps = src.capabilities().clone();
    // The ranges are those of a radio, not placeholders.
    let placeholders = sdrkit_driver_core::form::check_capabilities(&caps);
    assert!(placeholders.is_empty(), "{placeholders:?}");
    assert_eq!(caps.name, format!("SpyServer {} (network)", srv.addr()));
    assert_eq!(caps.rates.ranges.len(), 9);
    assert_eq!((caps.rates.min_hz, caps.rates.max_hz), (39_062.5, 10e6));
    assert_eq!((caps.min_freq_hz, caps.max_freq_hz), (24e6, 1800e6));
    assert_eq!(caps.gain_db, Some((0.0, 21.0, 1.0)));
    assert!(caps.transmit.is_none() && !caps.has_bias_tee && caps.realtime);
    assert_eq!(src.sample_type(), SampleType::Cu8);
    let first = notes(&mut src);
    let (s, mut r) = start(&mut src, 1 << 22);
    assert_eq!((s.rate_hz, s.centre_hz), (2_500_000.0, 100e6));
    let mut expected = hex(HELLO);
    for (id, v) in [
        (0, 1),
        (100, 1),
        (102, 2),
        (101, 100_000_000),
        (2, 10),
        (1, 1),
    ] {
        expected.extend(cmd(id, v));
    }
    assert!(wait_for(|| srv.raw(0).len() >= expected.len(), 2.0));
    assert_eq!(srv.raw(0), expected);
    assert_eq!(
        &srv.raw(0)[18..],
        hex("02 00 00 00 08 00 00 00  00 00 00 00  01 00 00 00
             02 00 00 00 08 00 00 00  64 00 00 00  01 00 00 00
             02 00 00 00 08 00 00 00  66 00 00 00  02 00 00 00
             02 00 00 00 08 00 00 00  65 00 00 00  00 e1 f5 05
             02 00 00 00 08 00 00 00  02 00 00 00  0a 00 00 00
             02 00 00 00 08 00 00 00  01 00 00 00  01 00 00 00")
    );
    let got = read_n(&mut r, 5000, 3.0);
    assert_eq!(got.len(), 5000);
    assert!(got.iter().all(|&c| near(c, U8_I, U8_Q, 1e-5)));
    assert!(first.iter().any(|l| l
        == &format!(
            "SpyServer: connected to {} (Airspy, serial 1234ABCD, 10.000 Msps, in control)",
            srv.addr()
        )));
    let mut res = results();
    src.poll(&mut res);
    let f = res.fields();
    assert_eq!(f.get(&DEVICE), Some("Airspy"));
    assert_eq!(f.get(&SERIAL), Some(0x1234ABCD));
    assert_eq!(f.get(&IN_CONTROL), Some(true));
    assert_eq!(f.get(&GAIN_INDEX), Some(8));
    assert_eq!(f.get(&DIGITAL_GAIN), Some(0.0));
    src.stop();
}

#[test]
fn retunes_live_and_sends_only_what_changed() {
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, _r) = start(&mut src, 1 << 22);
    assert!(wait_for(|| srv.settings(0).len() >= 6, 2.0));
    let base = srv.raw(0).len();
    // The format is fixed while the radio is open.
    let t16 = with_format(tune(100e6, 2.2 * M, Some(10.0)), WireFormat::Bits16);
    assert_eq!(src.apply(&t16), Err(refused(own_id(SpySetting::Format))));
    let t = tune(433.92e6, 10.0 * M, Some(21.0));
    let s = live(src.apply(&t));
    assert_eq!((s.rate_hz, s.centre_hz), (10e6, 433.92e6));
    let expected = hex("02 00 00 00 08 00 00 00  66 00 00 00  00 00 00 00
         02 00 00 00 08 00 00 00  65 00 00 00  00 18 dd 19
         02 00 00 00 08 00 00 00  02 00 00 00  15 00 00 00");
    assert!(wait_for(|| srv.raw(0).len() >= base + expected.len(), 2.0));
    assert_eq!(&srv.raw(0)[base..], expected);
    // Nothing changed: nothing sent.
    notes(&mut src);
    src.apply(&t).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(srv.raw(0).len(), base + expected.len());
    // A gain above the table and a frequency above 32-bit Hz are refused.
    let g = tune(433.92e6, 10.0 * M, Some(30.0));
    assert_eq!(src.apply(&g), Err(refused(RadioSetting::Gain.id())));
    let f = tune(5e9, 10.0 * M, Some(21.0));
    assert_eq!(src.apply(&f), Err(refused(RadioSetting::Centre.id())));
    // 3 GHz: one command, with the range warning.
    let s = live(src.apply(&tune(3e9, 10.0 * M, Some(21.0))));
    assert_eq!(s.centre_hz, 3e9);
    assert!(notes(&mut src).iter().any(|l| l.starts_with(
        "this SpyServer radio is specified for 24 to 1800 MHz; the driver may not reach 3000 MHz"
    )),);
    let more = hex("02 00 00 00 08 00 00 00  65 00 00 00  00 5e d0 b2");
    assert!(wait_for(
        || srv.raw(0).len() >= base + expected.len() + 16,
        2.0
    ));
    assert_eq!(&srv.raw(0)[base + expected.len()..], more);
    // No STREAMING_MODE or STREAMING_ENABLED again.
    let s = srv.settings(0);
    assert_eq!(s.iter().filter(|(id, _)| *id == 0 || *id == 1).count(), 2);
    src.stop();
}

#[test]
fn iq_bodies_go_to_the_ring_as_they_come_and_unused_ones_are_dropped() {
    let mut script = hex(
        "a4 06 00 02  64 00 00 00  01 00 00 00  07 00 00 00  04 00 00 00   c8 32 c8 32
         a4 06 00 02  64 00 06 00  01 00 00 00  08 00 00 00  02 00 00 00   c8 32
         a4 06 00 02  65 00 00 00  01 00 00 00  09 00 00 00  04 00 00 00   00 40 00 e0
         a4 06 00 02  66 00 00 00  01 00 00 00  0a 00 00 00  06 00 00 00   00 00 40 00 00 e0
         a4 06 00 02  67 00 00 00  01 00 00 00  0b 00 00 00  08 00 00 00   00 00 00 3f 00 00 80 be
         a4 06 00 02  64 00 00 00  01 00 00 00  0c 00 00 00  03 00 00 00   c8 32 c8
         a4 06 00 02  2d 01 00 00  04 00 00 00  0d 00 00 00  04 00 00 00   01 02 03 04
         a4 06 00 02  02 00 00 00  00 00 00 00  0e 00 00 00  00 00 00 00
         a4 06 00 02  64 00 00 00  01 00 00 00  0f 00 00 00  00 00 10 00",
    );
    script.extend((0..1 << 19).flat_map(|_| [0xc8u8, 0x32]));
    script.extend(hex(
        "a4 06 00 02  64 00 00 00  01 00 00 00  10 00 00 00  01 00 10 00",
    ));
    let srv = Fake::start(FakeOpts {
        periodic: false,
        script,
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, None))
        .unwrap();
    let (_, mut r) = start(&mut src, 1 << 22);
    // 8-bit messages only: the 16-bit, 24-bit and float ones do not fit the ring and are dropped.
    let total = 2 + 1 + 1 + 524_288;
    let got = read_n(&mut r, total + 1, 5.0);
    assert_eq!(got.len(), total);
    assert!(got.iter().all(|&c| near(c, U8_I, U8_Q, 1e-6)));
    // The 1 048 577-byte body ends the loop as a lost connection. The base reconnects.
    assert!(wait_log(&mut src, 3.0, "TooLong"));
    assert!(wait_for(
        || srv.count() >= 2 && srv.raw(1).starts_with(&hex(HELLO)),
        3.0
    ));
    src.stop();
}

/// An 8-bit IQ message of `n` samples with the bytes 200 and 50.
fn u8_iq(seq: u32, n: usize) -> Vec<u8> {
    let body: Vec<u8> = (0..n).flat_map(|_| [200u8, 50]).collect();
    message(proto::msg::UINT8_IQ, 1, seq, &body)
}

#[test]
fn dropped_iq_messages_are_filled_with_zeros_and_counted() {
    const N: usize = 16_384;
    // 10, 14: three messages dropped. 15, a PONG at 16, 17: none dropped. 17, 218: 200 dropped, more than a second at
    // 2.5 Msps, a restart that is not filled.
    let mut script = u8_iq(10, N);
    script.extend(u8_iq(14, N));
    script.extend(u8_iq(15, N));
    script.extend(message(proto::msg::PONG, 0, 16, &[]));
    script.extend(u8_iq(17, N));
    script.extend(u8_iq(218, N));
    let srv = Fake::start(FakeOpts {
        periodic: false,
        script,
        ..Default::default()
    });
    // A stall long enough that no reconnect adds its own loss while the test runs.
    let timing = Timing {
        stall: Duration::from_secs(60),
        ..Timing::default()
    };
    let mut src = srv
        .driver_with(timing)
        .open(&srv.addr(), &tune(100e6, 2.2 * M, None))
        .unwrap();
    let (s, mut r) = start(&mut src, 1 << 22);
    assert_eq!(s.rate_hz, 2.5e6);
    let total = 5 * N + 3 * N;
    let got = read_n(&mut r, total, 2.0);
    assert_eq!(got.len(), total);
    assert!(read_n(&mut r, 1, 0.2).is_empty());
    let zero = 0.5 / 128.0;
    for (i, c) in got.iter().enumerate() {
        let filled = (N..4 * N).contains(&i);
        if filled {
            assert!(near(*c, zero, zero, 1e-6), "{i}: {c}");
        } else {
            assert!(near(*c, U8_I, U8_Q, 1e-6), "{i}: {c}");
        }
    }
    let loss = src.loss().unwrap();
    assert_eq!((loss.radio_lost, loss.radio_events), (49_152, 1));
    let mut res = results();
    src.poll(&mut res);
    assert_eq!(res.fields().get(&DROPPED_SAMPLES), Some(49_152));
    src.stop();
}

#[test]
fn a_read_only_client_never_sends_gain_until_it_gets_control() {
    let srv = Fake::start(FakeOpts {
        sync: sync_read_only(),
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(200e6, 1.0 * M, Some(10.0)))
        .unwrap();
    let (s, _r) = start(&mut src, 1 << 22);
    assert_eq!(s.rate_hz, 1.25e6);
    let note = format!(
        "the SpyServer at {} is read only (another client controls it): it is tuned to 100.000 MHz and gives 99.000 to 101.000 MHz",
        srv.addr()
    );
    assert!(notes(&mut src).contains(&note));
    assert!(wait_for(|| srv.settings(0).len() >= 5, 2.0));
    assert_eq!(
        srv.settings(0),
        [(0, 1), (100, 1), (102, 3), (101, 200_000_000), (1, 1)]
    );
    assert_eq!(
        &srv.raw(0)[18 + 48..18 + 64],
        hex("02 00 00 00 08 00 00 00  65 00 00 00  00 c2 eb 0b")
    );
    live(src.apply(&tune(99.5e6, 1.0 * M, Some(10.0))));
    assert!(!notes(&mut src).iter().any(|l| l.contains("read only")));
    assert!(wait_for(|| srv.settings(0).len() >= 6, 2.0));
    assert_eq!(srv.settings(0)[5], (101, 99_500_000));
    assert_eq!(
        &srv.raw(0)[srv.raw(0).len() - 16..],
        hex("02 00 00 00 08 00 00 00  65 00 00 00  e0 3f ee 05")
    );
    assert!(srv.settings(0).iter().all(|(id, _)| *id != 2));
    // Control passes to this client.
    srv.send(&hex(SYNC));
    assert!(wait_log(&mut src, 2.0, "SpyServer: now in control"));
    src.apply(&tune(99.5e6, 1.0 * M, Some(10.0))).unwrap();
    assert!(wait_for(|| srv.settings(0).len() >= 7, 2.0));
    assert_eq!(srv.settings(0)[6], (2, 10));
    srv.send(&sync_read_only());
    assert!(wait_log(&mut src, 2.0, "SpyServer: now read only"));
    // Read only again: a retune sends no GAIN.
    src.apply(&tune(99.6e6, 1.0 * M, Some(10.0))).unwrap();
    assert!(wait_for(|| srv.settings(0).len() >= 8, 2.0));
    assert_eq!(srv.settings(0)[7], (101, 99_600_000));
    // Control comes back: the same gain goes out again, since another client can have changed it meanwhile.
    srv.send(&hex(SYNC));
    assert!(wait_log(&mut src, 2.0, "SpyServer: now in control"));
    src.apply(&tune(99.6e6, 1.0 * M, Some(10.0))).unwrap();
    assert!(wait_for(|| srv.settings(0).len() >= 9, 2.0));
    assert_eq!(srv.settings(0)[8], (2, 10));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(srv.settings(0).len(), 9);
    src.stop();
}

#[test]
fn reconnects_after_a_drop_and_sends_everything_again() {
    let srv = Fake::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(300)),
        ..Default::default()
    });
    let t = with_format(tune(100e6, 2.2 * M, Some(10.0)), WireFormat::Bits16);
    let mut src = srv.driver().open(&srv.addr(), &t).unwrap();
    assert_eq!(src.sample_type(), SampleType::Cs16);
    let (_, mut r) = start(&mut src, 1 << 22);
    let lost = format!("SpyServer {}: the connection is closed", srv.addr());
    assert!(wait_log(&mut src, 3.0, &lost));
    let t0 = Instant::now();
    assert!(wait_for(|| srv.count() >= 2, 2.0));
    assert!(t0.elapsed() < Duration::from_secs(2));
    assert!(wait_for(|| srv.settings(1).len() >= 6, 2.0));
    let mut expected = hex(HELLO);
    for (id, v) in [
        (0, 1),
        (100, 2),
        (102, 2),
        (101, 100_000_000),
        (2, 10),
        (1, 1),
    ] {
        expected.extend(cmd(id, v));
    }
    assert_eq!(srv.raw(1), expected);
    let mut line = None;
    assert!(wait_for(
        || {
            line = notes(&mut src)
                .into_iter()
                .find(|l| l.contains("reconnected after"));
            line.is_some()
        },
        2.0
    ));
    let line = line.unwrap();
    assert!(
        line.contains(" s, about ") && line.ends_with(" samples lost"),
        "{line}"
    );
    let loss = src.loss().unwrap();
    assert!(
        loss.link_down > 0 && loss.radio_lost >= loss.link_down,
        "{loss:?}"
    );
    // The samples go on into the same ring.
    r.skip_all();
    let got = read_n(&mut r, 2000, 3.0);
    assert_eq!(got.len(), 2000);
    assert!(got.iter().all(|&c| near(c, 0.5, -0.25, 1e-5)));
    src.stop();
}

#[test]
fn a_format_change_while_away_is_refused_at_the_reconnect() {
    let later = Arc::new(Mutex::new(Some(Behave::Silent)));
    let srv = Fake::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(200)),
        behave_later: later.clone(),
        ..Default::default()
    });
    let mut src = srv
        .driver_with(Timing {
            handshake: Duration::from_millis(300),
            ..Timing::default()
        })
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, mut r) = start(&mut src, 1 << 22);
    assert!(wait_log(&mut src, 3.0, "the connection is closed"));
    let t = with_format(tune(100e6, 2.2 * M, Some(10.0)), WireFormat::Bits16);
    live(src.apply(&t));
    *later.lock().unwrap() = None;
    assert!(wait_log(
        &mut src,
        5.0,
        "reconnect failed: the device refused setting"
    ));
    r.skip_all();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.skip_all(), 0);
    src.stop();
}

fn open_err(driver: &SpyServerDriver, addr: &str) -> DriverError {
    driver
        .open(addr, &tune(100e6, 2.2 * M, Some(10.0)))
        .err()
        .expect("the open fails")
}

#[test]
fn nothing_listening_is_a_refused_connection_and_a_retry() {
    let net = FakeTransport::new();
    let d = SpyServerDriver::new(Arc::new(net));
    let e = open_err(&d, "127.0.0.1:5555");
    assert_eq!(e, DriverError::Net(sdrkit_core::NetError::Refused));
    assert!(e.retryable());
}

#[test]
fn a_silent_server_is_not_a_spyserver_after_3_s() {
    let srv = Fake::start(FakeOpts {
        behave: Behave::Silent,
        ..Default::default()
    });
    let t = Instant::now();
    let e = open_err(&srv.driver(), &srv.addr());
    let took = t.elapsed();
    assert_eq!(e, DriverError::Protocol(Protocol::NotThisRadio));
    assert!(!e.retryable());
    assert!(
        took >= Duration::from_millis(2900) && took < Duration::from_millis(4000),
        "{took:?}"
    );
    assert_eq!(srv.count(), 1);
}

#[test]
fn handshake_failures_say_what_is_missing() {
    let quick = Timing {
        handshake: Duration::from_millis(400),
        ..Timing::default()
    };
    let not = DriverError::Protocol(Protocol::NotThisRadio);
    let srv = Fake::start(FakeOpts {
        behave: Behave::CloseAfterHello,
        ..Default::default()
    });
    assert_eq!(open_err(&srv.driver_with(quick), &srv.addr()), not);
    let srv = Fake::start(FakeOpts {
        behave: Behave::Raw(hex("52 54 4c 30 00 00 00 05 00 00 00 1d")),
        ..Default::default()
    });
    assert_eq!(open_err(&srv.driver_with(quick), &srv.addr()), not);
    // A body over 1 MiB in the handshake.
    let srv = Fake::start(FakeOpts {
        behave: Behave::Raw(hex(
            "a4 06 00 02  00 00 00 00  00 00 00 00  00 00 00 00  01 00 10 00",
        )),
        ..Default::default()
    });
    assert_eq!(open_err(&srv.driver_with(quick), &srv.addr()), not);
    let srv = Fake::start(FakeOpts {
        behave: Behave::DeviceInfoOnly,
        ..Default::default()
    });
    assert_eq!(
        open_err(&srv.driver_with(quick), &srv.addr()),
        DriverError::Protocol(Protocol::MissingPart)
    );
    // 64 messages of something else.
    let pongs: Vec<u8> = (0..64).flat_map(|i| message(2, 0, i, &[])).collect();
    let srv = Fake::start(FakeOpts {
        behave: Behave::Raw(pongs),
        ..Default::default()
    });
    let t = Instant::now();
    assert_eq!(open_err(&srv.driver(), &srv.addr()), not);
    assert!(
        t.elapsed() < Duration::from_secs(1),
        "64 messages end the handshake at once"
    );
    let no_radio = Some(SpyError::NoRadio);
    let mut d = DeviceInfo::airspy_r2();
    d.device_type = 0;
    let srv = Fake::start(FakeOpts {
        device: device_msg(&d),
        ..Default::default()
    });
    let e = open_err(&srv.driver_with(quick), &srv.addr());
    assert_eq!(spy(&e), no_radio);
    assert!(!e.retryable());
    let mut d = DeviceInfo::airspy_r2();
    d.max_sample_rate = 0;
    let srv = Fake::start(FakeOpts {
        device: device_msg(&d),
        ..Default::default()
    });
    assert_eq!(
        spy(&open_err(&srv.driver_with(quick), &srv.addr())),
        no_radio
    );
}

#[test]
fn a_bad_address_is_final_and_connects_nowhere() {
    let net = FakeTransport::new();
    let d = SpyServerDriver::new(Arc::new(net.clone()));
    let e = open_err(&d, "host");
    assert_eq!(e, DriverError::Address);
    assert!(!e.retryable());
    assert_eq!(net.connects(), 0);
    for a in ["host:0", "host:70000", ":5555", "[::1]", "::1", "h:x"] {
        assert!(parse_address(a).is_err(), "{a}");
    }
    assert_eq!(parse_address("[::1]:5555").unwrap(), ("::1".into(), 5555));
    assert_eq!(
        parse_address("10.0.0.2:5555").unwrap(),
        ("10.0.0.2".into(), 5555)
    );
}

#[test]
fn a_stall_ends_the_loop_after_3_s_and_the_base_reconnects() {
    let srv = Fake::start(FakeOpts {
        quiet_first_after: Some(Duration::from_millis(200)),
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, mut r) = start(&mut src, 1 << 22);
    // The last sample arrives, then nothing.
    let mut last = Instant::now();
    while srv.count() < 2 && last.elapsed() < Duration::from_secs(1) {
        if r.wait(1, Duration::from_millis(5)) > 0 {
            r.skip_all();
            last = Instant::now();
        }
    }
    assert!(wait_log(&mut src, 5.0, "timed out"));
    let took = last.elapsed();
    assert!(
        took >= Duration::from_millis(2500) && took <= Duration::from_millis(3600),
        "{took:?}"
    );
    assert!(wait_for(|| srv.count() >= 2, 2.0));
    src.stop();
}

#[test]
fn a_stop_while_reading_returns_at_once_and_closes_the_connection() {
    let srv = Fake::start(FakeOpts {
        periodic: false,
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, _r) = start(&mut src, 1 << 20);
    std::thread::sleep(Duration::from_millis(200));
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(200),
        "{:?}",
        t.elapsed()
    );
    assert!(wait_for(
        || srv.conn(0).unwrap().closed.load(Ordering::Acquire),
        1.0
    ));
    src.stop();
}

#[test]
fn a_stop_during_a_reconnect_returns_at_once() {
    let srv = Fake::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(100)),
        behave_later: Arc::new(Mutex::new(Some(Behave::Silent))),
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, _r) = start(&mut src, 1 << 20);
    // The reconnect now waits in its handshake.
    assert!(wait_for(
        || srv.conn(1).is_some_and(|c| c.hello.load(Ordering::Acquire)),
        3.0
    ));
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(200),
        "{:?}",
        t.elapsed()
    );
    assert!(wait_for(
        || srv
            .conns
            .lock()
            .unwrap()
            .iter()
            .all(|c| c.closed.load(Ordering::Acquire)),
        1.0
    ));
}

#[test]
fn close_twice_and_close_before_start_are_fine() {
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    src.stop();
    src.stop();
    assert!(wait_for(
        || srv.conn(0).unwrap().closed.load(Ordering::Acquire),
        1.0
    ));
    // Nothing was ever enabled.
    assert!(srv.settings(0).iter().all(|(id, _)| *id != 1));
}

#[test]
fn capabilities_come_from_the_server() {
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let c = src.capabilities().clone();
    assert_eq!(c.rates.ranges.len(), 9);
    assert_eq!(c.rates.ranges[0], (39_062.5, 39_062.5));
    assert_eq!(c.rates.ranges[8], (10e6, 10e6));
    assert_eq!((c.min_freq_hz, c.max_freq_hz), (24e6, 1800e6));
    assert_eq!(c.gain_db, Some((0.0, 21.0, 1.0)));
    src.stop();
    // The HF+ server.
    let mut d = hex(DEVICE_R2)[..20].to_vec();
    d.extend(hex(BODY_HF));
    // Its CLIENT_SYNC returns an IQ band inside the HF+ range (10 to 11 MHz), so nothing widens.
    let hf_sync = ClientSync {
        can_control: true,
        device_center: 10_500_000,
        iq_center: 10_500_000,
        min_iq_center: 10_000_000,
        max_iq_center: 11_000_000,
        ..Default::default()
    };
    let hf = Fake::start(FakeOpts {
        device: d,
        sync: sync_msg(&hf_sync),
        ..Default::default()
    });
    let mut src = hf
        .driver()
        .open(&hf.addr(), &tune(10e6, 0.1 * M, Some(1.0)))
        .unwrap();
    let mut res = results();
    src.poll(&mut res);
    assert_eq!(res.fields().get(&DEVICE), Some("Airspy HF+"));
    let c = src.capabilities();
    assert_eq!(
        c.rates.ranges.iter().map(|r| r.0).collect::<Vec<_>>(),
        [24_000.0, 48_000.0, 96_000.0, 192_000.0, 384_000.0]
    );
    assert_eq!((c.rates.min_hz, c.rates.max_hz), (24_000.0, 384_000.0));
    assert_eq!((c.min_freq_hz, c.max_freq_hz), (0.1e6, 31e6));
    assert_eq!(c.gain_db, Some((0.0, 3.0, 1.0)));
    src.stop();
}

#[test]
fn client_sync_widens_the_tuning_range_and_reports_the_change() {
    let mut s = ClientSync::parse(&hex(SYNC)[20..]);
    s.min_iq_center = 24_000_000;
    s.max_iq_center = 1_900_000_000;
    let mut sync = sync_msg(&s);
    assert_eq!(&sync[20 + 24..20 + 28], hex("00 b3 3f 71"));
    sync[12] = 1;
    let srv = Fake::start(FakeOpts {
        sync,
        ..Default::default()
    });
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(1850e6, 2.2 * M, Some(10.0)))
        .unwrap();
    assert_eq!(src.capabilities().max_freq_hz, 1900e6);
    notes(&mut src);
    // Inside the widened range: no range note.
    let (_, _r) = start(&mut src, 1 << 20);
    assert!(!notes(&mut src).iter().any(|l| l.contains("specified for")));
    // A later CLIENT_SYNC narrows nothing below DEVICE_INFO, and the change is reported.
    srv.send(&hex(SYNC));
    let mut res = results();
    assert!(wait_for(|| src.poll(&mut res).capabilities, 2.0));
    assert_eq!(src.capabilities().max_freq_hz, 1800e6);
    src.stop();
}

#[test]
fn the_format_setting_and_a_forced_format() {
    let schema = DESCRIPTOR.decl().schema().clone();
    let id = own_id(SpySetting::Format);
    let f = schema.setting(id).unwrap();
    assert_eq!(
        (f.field.key.as_ref(), f.field.label.as_ref()),
        ("format", "Sample format")
    );
    assert_eq!(Values::defaults(&schema).get(&FORMAT), WireFormat::Bits8);
    // A value that is not a choice is refused by the schema, never replaced by the default.
    assert!(Values::defaults(&schema)
        .set(id, &Value::Choice(2))
        .is_err());
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 2.2 * M, None))
        .unwrap();
    assert!(wait_for(|| srv.settings(0).len() >= 4, 2.0));
    assert_eq!(srv.settings(0)[1], (100, 1));
    // No gain requested: no GAIN sent.
    assert!(srv.settings(0).iter().all(|(id, _)| *id != 2));
    src.stop();
    // A server that forces float gets float whatever is chosen.
    let mut d = DeviceInfo::airspy_r2();
    d.forced_iq_format = 4;
    let srv = Fake::start(FakeOpts {
        device: device_msg(&d),
        ..Default::default()
    });
    let t = with_format(tune(100e6, 2.2 * M, None), WireFormat::Bits16);
    let mut src = srv.driver().open(&srv.addr(), &t).unwrap();
    assert_eq!(src.sample_type(), SampleType::Cf32);
    assert!(wait_for(|| srv.settings(0).len() >= 4, 2.0));
    assert_eq!(srv.settings(0)[1], (100, 4));
    src.stop();
    // A server that forces 24-bit is refused: no native sample type holds it.
    d.forced_iq_format = 3;
    let srv = Fake::start(FakeOpts {
        device: device_msg(&d),
        ..Default::default()
    });
    assert_eq!(
        spy(&open_err(&srv.driver(), &srv.addr())),
        Some(SpyError::WireFormat { format: 3 })
    );
}

#[test]
fn a_rate_too_high_warns_and_runs_the_fastest() {
    let srv = Fake::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(&srv.addr(), &tune(100e6, 20.0 * M, Some(10.0)))
        .unwrap();
    let warning =
        "this radio only offers 10.00 Msps, 20.00 Msps were requested for this channel width";
    assert!(notes(&mut src).iter().any(|l| l == warning));
    let (s, _r) = start(&mut src, 1 << 20);
    assert_eq!(s.rate_hz, 10e6);
    // A range note replaces the rate warning.
    live(src.apply(&tune(3e9, 20.0 * M, Some(10.0))));
    let n = notes(&mut src);
    assert!(n
        .iter()
        .any(|l| l.starts_with("this SpyServer radio is specified for")));
    assert!(!n.iter().any(|l| l.contains("only offers")));
    src.stop();
}

#[test]
fn throughput_is_measured_and_a_slow_link_is_flagged() {
    // The fake sends about 0.5 Msps. 2.5 Msps is requested.
    let srv = Fake::start(FakeOpts::default());
    let drv = srv.driver_with(Timing {
        window: Duration::from_millis(250),
        ..Timing::default()
    });
    let mut src = drv
        .open(&srv.addr(), &tune(100e6, 2.2 * M, Some(10.0)))
        .unwrap();
    let (_, mut r) = start(&mut src, 1 << 22);
    let mut res = results();
    assert!(wait_for(
        || {
            r.skip_all();
            src.poll(&mut res);
            res.fields().get(&LOW_THROUGHPUT) == Some(true)
        },
        4.0
    ));
    let f = res.fields();
    let (mbit, arriving) = (f.get(&MBIT_S).unwrap(), f.get(&ARRIVING).unwrap());
    assert!(
        mbit > 1.0 && arriving > 50_000.0 && arriving < 2.0e6,
        "{mbit} {arriving}"
    );
    assert!(res
        .log_lines()
        .iter()
        .any(|l| l.contains("the network or the server is the limit")));
    src.stop();
}

#[test]
fn the_descriptor() {
    let d = SpyServerDriver::new(Arc::new(FakeTransport::new()));
    let k = d.descriptor();
    assert_eq!((k.key, k.name), ("spyserver", "SpyServer"));
    let samples: Vec<_> = k.produces.iter().map(|f| f.sample).collect();
    assert_eq!(
        samples,
        [SampleType::Cu8, SampleType::Cs16, SampleType::Cf32]
    );
}
