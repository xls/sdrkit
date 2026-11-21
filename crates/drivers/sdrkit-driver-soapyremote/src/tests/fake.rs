//! A fake SoapyRemote server on a fake network at 127.0.0.1:55132. It answers calls from a script, records every request byte
//! for byte, runs receive streams at a set rate within the client's window, receives transmit streams, and forwards scripted
//! log lines.

use crate::rpc::{self, call, Kwargs, Packer, Range, Unpacker};
use crate::stream::{DgHeader, HEADER_LEN};
use core::net::SocketAddr;
use sdrkit_core::net::{Conn, Datagram, DatagramOptions, FakeTransport, Transport};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The port the fake serves on.
pub const PORT: u16 = 55132;

/// A reply override: (call, request payload after the call value) to a reply payload. `None`: use the table.
pub type Handler = Arc<dyn Fn(u32, &[u8]) -> Option<Vec<u8>> + Send + Sync>;

/// One request as received.
#[derive(Clone, Debug)]
pub struct Req {
    /// Which connection (counted from 0).
    pub conn: usize,
    /// It is a log connection.
    pub log: bool,
    /// The call number.
    pub call: u32,
    /// The complete packet.
    pub bytes: Vec<u8>,
}

/// How the receive stream behaves.
#[derive(Clone, Debug)]
pub struct StreamScript {
    /// Data datagrams per second.
    pub rate: f64,
    /// Elements per data datagram.
    pub elems: usize,
    /// Bytes per element.
    pub bpe: usize,
    /// Explicit datagrams sent first (their sequence fields as given).
    pub custom: Vec<Vec<u8>>,
    /// Sequence jumps: after sending this sequence, the next one is `.1`.
    pub jumps: Vec<(u32, u32)>,
    /// Stop sending after this many datagrams (per stream).
    pub stop_after: Option<u64>,
    /// Never send.
    pub never_send: bool,
    /// Ignore the first acknowledgement.
    pub ignore_first_ack: bool,
    /// Status messages (error codes) sent after the first datagram.
    pub status: Vec<i32>,
    /// Ignore the window (send whatever the acknowledgements say).
    pub ignore_window: bool,
    /// Transmit streams: never acknowledge.
    pub tx_never_ack: bool,
    /// Transmit streams: status messages sent after the first datagram.
    pub tx_status: Vec<i32>,
}

impl Default for StreamScript {
    fn default() -> Self {
        StreamScript {
            rate: 2000.0,
            elems: 714,
            bpe: 2,
            custom: Vec::new(),
            jumps: Vec::new(),
            stop_after: None,
            never_send: false,
            ignore_first_ack: false,
            status: Vec::new(),
            ignore_window: false,
            tx_never_ack: false,
            tx_status: Vec::new(),
        }
    }
}

/// The server's behaviour.
#[derive(Clone)]
pub struct Script {
    /// Reply payloads by call.
    pub replies: HashMap<u32, Vec<u8>>,
    /// Overrides, consulted first.
    pub handler: Option<Handler>,
    /// Delays before replying, by call.
    pub delay: HashMap<u32, Duration>,
    /// Calls never answered.
    pub silent: HashSet<u32>,
    /// Close each control connection after its N-th request (1-based), without replying.
    pub close_after: Option<usize>,
    /// Header word of replies.
    pub header: [u8; 4],
    /// Trailer word of replies.
    pub trailer: [u8; 4],
    /// Write replies one byte at a time.
    pub split: bool,
    /// The version word of replies.
    pub version: u32,
    /// The stream.
    pub stream: StreamScript,
    /// Log lines sent after call 21.
    pub log: Vec<(i8, String)>,
    /// Never answer call 21.
    pub log_silent: bool,
    /// Make fails this many times with this text, then succeeds.
    pub make_fail: Option<(usize, String)>,
}

pub fn f64_payload(v: f64) -> Vec<u8> {
    let mut p = Packer::default();
    p.f64(v);
    p.buf
}

/// Reply payload helpers.
pub fn string(s: &str) -> Vec<u8> {
    let mut p = Packer::default();
    p.string(s);
    p.buf
}
/// A STRING_LIST payload.
pub fn strings(v: &[&str]) -> Vec<u8> {
    let mut p = Packer::default();
    p.string_list(v);
    p.buf
}
/// A BOOL payload.
pub fn boolean(b: bool) -> Vec<u8> {
    let mut p = Packer::default();
    p.bool(b);
    p.buf
}
/// An INT32 payload.
pub fn int(v: i32) -> Vec<u8> {
    let mut p = Packer::default();
    p.i32(v);
    p.buf
}
/// A RANGE payload.
pub fn range(min: f64, max: f64, step: f64) -> Vec<u8> {
    let mut p = Packer::default();
    p.range(Range::new(min, max, step), rpc::VERSION);
    p.buf
}
/// A RANGE_LIST payload.
pub fn ranges(v: &[(f64, f64, f64)]) -> Vec<u8> {
    let mut p = Packer::default();
    let r: Vec<Range> = v.iter().map(|&(a, b, c)| Range::new(a, b, c)).collect();
    p.range_list(&r, rpc::VERSION);
    p.buf
}
/// An EXCEPTION payload.
pub fn exception(t: &str) -> Vec<u8> {
    let mut p = Packer::default();
    p.exception(t);
    p.buf
}
/// The payload of a complete packet.
pub fn payload_of(packet: &[u8]) -> Vec<u8> {
    packet[12..packet.len() - 4].to_vec()
}

impl Script {
    /// An RTL-SDR.
    pub fn rtl() -> Script {
        let mut r = HashMap::new();
        let h = super::hex;
        r.insert(
            call::SERVER_ID,
            string("2f9a3c10-1b2c-4d5e-8f90-a1b2c3d4e5f6"),
        );
        r.insert(call::DRIVER_KEY, payload_of(&h("53 52 50 43 00 00 04 00 00 00 00 1c 06 02 00 00 00 06 52 54 4c 53 44 52 43 50 52 53")));
        r.insert(call::HARDWARE_KEY, string("Generic RTL2832U OEM"));
        let mut kw = Kwargs::new();
        kw.insert("tuner".into(), "Rafael Micro R820T".into());
        let mut p = Packer::default();
        p.kwargs(&kw);
        r.insert(call::HARDWARE_INFO, p.buf);
        r.insert(call::NATIVE_FORMAT, payload_of(&h(super::NATIVE)));
        r.insert(call::STREAM_FORMATS, payload_of(&h(super::FORMATS)));
        r.insert(call::SAMPLE_RATE_RANGE, payload_of(&h(super::RATES)));
        r.insert(call::FREQUENCY_RANGE, ranges(&[(24e6, 1.766e9, 0.0)]));
        r.insert(call::GAIN_RANGE, payload_of(&h(super::GAIN)));
        r.insert(call::LIST_GAINS, strings(&["TUNER"]));
        r.insert(call::GAIN_RANGE_ELEMENT, range(0.0, 49.6, 0.0));
        r.insert(call::GET_GAIN_ELEMENT, f64_payload(29.7));
        r.insert(call::HAS_GAIN_MODE, boolean(true));
        r.insert(call::HAS_FREQ_CORRECTION, boolean(true));
        r.insert(call::LIST_ANTENNAS, strings(&["RX"]));
        r.insert(call::SETTING_INFO, payload_of(&h(super::SETTING_INFO)));
        r.insert(call::READ_SETTING, string("0"));
        r.insert(call::BANDWIDTH_RANGE, ranges(&[]));
        r.insert(call::LIST_SENSORS, strings(&[]));
        Script {
            replies: r,
            handler: None,
            delay: HashMap::new(),
            silent: HashSet::new(),
            close_after: None,
            header: rpc::HEADER,
            trailer: rpc::TRAILER,
            split: false,
            version: rpc::VERSION,
            stream: StreamScript::default(),
            log: Vec::new(),
            log_silent: false,
            make_fail: None,
        }
    }
}

/// A receive stream the fake runs.
#[derive(Default)]
pub struct StreamState {
    /// Acknowledgements received (in order).
    pub acks: Mutex<Vec<(Instant, [u8; HEADER_LEN])>>,
    /// Data datagrams sent.
    pub sent: AtomicUsize,
    /// Activated.
    pub active: AtomicBool,
    /// Closed.
    pub closed: AtomicBool,
    /// Datagrams received (transmit streams): headers and payloads.
    pub received: Mutex<Vec<(DgHeader, Vec<u8>)>>,
    /// The direction.
    pub dir: AtomicI32,
    /// The requested format.
    pub format: Mutex<String>,
    /// The stream arguments.
    pub args: Mutex<Kwargs>,
    /// The ports the client sent (data, status).
    pub ports: Mutex<(u16, u16)>,
}

/// What the fake shares with its tests.
#[derive(Default)]
pub struct Shared {
    /// Every request.
    pub requests: Mutex<Vec<Req>>,
    /// Connections accepted.
    pub conns: AtomicUsize,
    /// Connections open now.
    pub open: AtomicUsize,
    /// Streams set up.
    pub streams: Mutex<Vec<Arc<StreamState>>>,
    /// The sample rate set (call 900).
    pub rate: Mutex<f64>,
    /// Makes received.
    pub makes: AtomicUsize,
    /// Status socket target of the latest stream.
    pub status_to: Mutex<Option<u16>>,
}

/// The fake server.
pub struct Fake {
    /// The fake network it serves on.
    pub net: FakeTransport,
    /// Its port.
    pub port: u16,
    /// Shared state.
    pub sh: Arc<Shared>,
    /// The script (may be changed while it runs).
    pub script: Arc<Mutex<Script>>,
    stop: Arc<AtomicBool>,
}

impl Fake {
    /// Starts a fake with `script` on a fake network of its own.
    pub fn start(script: Script) -> Fake {
        let net = FakeTransport::new();
        let sh = Arc::new(Shared {
            rate: Mutex::new(2.4e6),
            ..Default::default()
        });
        let script = Arc::new(Mutex::new(script));
        let stop = Arc::new(AtomicBool::new(false));
        let (sh2, sc2, st2, n2) = (sh.clone(), script.clone(), stop.clone(), net.clone());
        net.serve("127.0.0.1", PORT, move |s| {
            let id = sh2.conns.fetch_add(1, Ordering::SeqCst);
            sh2.open.fetch_add(1, Ordering::SeqCst);
            conn(s, id, &sh2, &sc2, &st2, &n2);
            sh2.open.fetch_sub(1, Ordering::SeqCst);
        });
        Fake {
            net,
            port: PORT,
            sh,
            script,
            stop,
        }
    }

    /// The network of the fake, for a driver.
    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    /// `127.0.0.1:port`.
    pub fn addr(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    /// Requests of control connections (not log connections).
    pub fn control(&self) -> Vec<Req> {
        self.sh
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| !r.log)
            .cloned()
            .collect()
    }

    /// Calls of control connections, in order.
    pub fn calls(&self) -> Vec<u32> {
        self.control().iter().map(|r| r.call).collect()
    }

    /// Requests from index `from` on.
    pub fn since(&self, from: usize) -> Vec<Req> {
        self.control().into_iter().skip(from).collect()
    }

    /// The latest stream.
    pub fn stream(&self) -> Option<Arc<StreamState>> {
        self.sh.streams.lock().unwrap().last().cloned()
    }

    /// Waits until `f` holds, up to `d`.
    pub fn wait(&self, d: Duration, f: impl Fn(&Fake) -> bool) -> bool {
        let end = Instant::now() + d;
        while Instant::now() < end {
            if f(self) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        f(self)
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.net.stop_serving("127.0.0.1", PORT);
        for s in self.sh.streams.lock().unwrap().iter() {
            s.closed.store(true, Ordering::Release);
        }
    }
}

fn read_req(s: &mut dyn Conn) -> Option<(u32, Vec<u8>, Vec<u8>)> {
    let mut h = [0u8; 12];
    s.read_exact(&mut h).ok()?;
    let len = u32::from_be_bytes([h[8], h[9], h[10], h[11]]) as usize;
    if &h[..4] != b"SRPC" || !(17..1 << 24).contains(&len) {
        return None;
    }
    let mut rest = vec![0u8; len - 12];
    s.read_exact(&mut rest).ok()?;
    let mut bytes = h.to_vec();
    bytes.extend_from_slice(&rest);
    let payload = rest[..rest.len() - 4].to_vec();
    let mut u = Unpacker::new(&payload, rpc::VERSION);
    let n = u.call().ok()?;
    let args = payload[payload.len() - u.remaining()..].to_vec();
    Some((n, bytes, args))
}

fn send(s: &mut dyn Conn, sc: &Script, payload: &[u8]) -> bool {
    let mut pkt = rpc::packet(payload, sc.version);
    pkt[..4].copy_from_slice(&sc.header);
    let n = pkt.len();
    pkt[n - 4..].copy_from_slice(&sc.trailer);
    if sc.split {
        for b in pkt {
            if s.write_all(&[b]).is_err() {
                return false;
            }
            let _ = s.flush();
        }
        true
    } else {
        s.write_all(&pkt).is_ok()
    }
}

fn conn(
    mut s: Box<dyn Conn>,
    id: usize,
    sh: &Arc<Shared>,
    script: &Arc<Mutex<Script>>,
    stop: &Arc<AtomicBool>,
    net: &FakeTransport,
) {
    let s = s.as_mut();
    let mut count = 0;
    let mut is_log = false;
    loop {
        let Some((n, bytes, args)) = read_req(s) else {
            return;
        };
        count += 1;
        if n == call::START_LOG && count == 1 {
            is_log = true;
        }
        sh.requests.lock().unwrap().push(Req {
            conn: id,
            log: is_log,
            call: n,
            bytes,
        });
        let sc = script.lock().unwrap().clone();
        if is_log {
            match n {
                call::START_LOG => {
                    if sc.log_silent {
                        continue;
                    }
                    send(s, &sc, &[rpc::tag::VOID]);
                    for (lvl, msg) in &sc.log {
                        let mut p = Packer::default();
                        p.char(*lvl).string(msg);
                        if !send(s, &sc, &p.buf) {
                            return;
                        }
                    }
                }
                call::HANGUP => {
                    send(s, &sc, &[rpc::tag::VOID]);
                    s.shutdown();
                    return;
                }
                _ => {
                    send(s, &sc, &[rpc::tag::VOID]);
                }
            }
            continue;
        }
        if sc.close_after.is_some_and(|k| count >= k) {
            s.shutdown();
            return;
        }
        if sc.silent.contains(&n) {
            // Never answer: wait until the client stops waiting.
            let mut b = [0u8; 1];
            let _ = s.set_read_timeout(None);
            while !stop.load(Ordering::Acquire) {
                match s.read(&mut b) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
            }
            return;
        }
        if let Some(d) = sc.delay.get(&n) {
            std::thread::sleep(*d);
        }
        let reply = answer(n, &args, sh, script, &sc, net);
        if !send(s, &sc, &reply) {
            return;
        }
        if n == call::HANGUP {
            s.shutdown();
            return;
        }
    }
}

fn answer(
    n: u32,
    args: &[u8],
    sh: &Arc<Shared>,
    script: &Arc<Mutex<Script>>,
    sc: &Script,
    net: &FakeTransport,
) -> Vec<u8> {
    if let Some(h) = &sc.handler {
        if let Some(r) = h(n, args) {
            return r;
        }
    }
    let void = vec![rpc::tag::VOID];
    match n {
        call::MAKE => {
            sh.makes.fetch_add(1, Ordering::SeqCst);
            let mut g = script.lock().unwrap();
            if let Some((k, text)) = g.make_fail.clone() {
                if k > 0 {
                    g.make_fail = Some((k - 1, text.clone()));
                    return exception(&text);
                }
            }
            void
        }
        call::NUM_CHANNELS => {
            let dir = args.get(1).copied().unwrap_or(1);
            if let Some(r) = sc.replies.get(&n) {
                if dir == 1 {
                    return r.clone();
                }
            }
            int(if dir == 1 { 1 } else { 0 })
        }
        call::SET_SAMPLE_RATE => {
            let mut u = Unpacker::new(args, rpc::VERSION);
            let _ = u.char();
            let _ = u.i32();
            if let Ok(r) = u.f64() {
                *sh.rate.lock().unwrap() = r;
            }
            void
        }
        call::GET_SAMPLE_RATE => f64_payload(*sh.rate.lock().unwrap()),
        call::SETUP_STREAM => setup(args, sh, script, net),
        call::ACTIVATE_STREAM | call::DEACTIVATE_STREAM => {
            let mut u = Unpacker::new(args, rpc::VERSION);
            let id = u.i32().unwrap_or(0) as usize;
            if let Some(st) = sh.streams.lock().unwrap().get(id) {
                st.active
                    .store(n == call::ACTIVATE_STREAM, Ordering::Release);
            }
            sc.replies.get(&n).cloned().unwrap_or_else(|| int(0))
        }
        call::CLOSE_STREAM => {
            let mut u = Unpacker::new(args, rpc::VERSION);
            let id = u.i32().unwrap_or(0) as usize;
            if let Some(st) = sh.streams.lock().unwrap().get(id) {
                st.closed.store(true, Ordering::Release);
            }
            void
        }
        _ => match sc.replies.get(&n) {
            Some(r) => r.clone(),
            None if is_setter(n) => void,
            None => exception(&format!("call {n} is not supported")),
        },
    }
}

fn is_setter(n: u32) -> bool {
    matches!(
        n,
        call::UNMAKE
            | call::HANGUP
            | call::SET_ANTENNA
            | call::SET_FREQ_CORRECTION
            | call::SET_GAIN_MODE
            | call::SET_GAIN
            | call::SET_GAIN_ELEMENT
            | call::SET_FREQUENCY
            | call::SET_BANDWIDTH
            | call::SET_CLOCK_SOURCE
            | call::SET_TIME_SOURCE
            | call::WRITE_SETTING
            | call::STOP_LOG
    )
}

fn setup(
    args: &[u8],
    sh: &Arc<Shared>,
    script: &Arc<Mutex<Script>>,
    net: &FakeTransport,
) -> Vec<u8> {
    let mut u = Unpacker::new(args, rpc::VERSION);
    let dir = u.char().unwrap_or(1);
    let format = u.string().unwrap_or_default();
    let _ = u.size_list();
    let kw = u.kwargs().unwrap_or_default();
    let dport: u16 = u.string().ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let sport: u16 = u.string().ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let sock = net.bind(&DatagramOptions::at(local(0))).unwrap();
    let port = sock.local_addr().port();
    let st = Arc::new(StreamState::default());
    st.dir.store(dir as i32, Ordering::Relaxed);
    *st.format.lock().unwrap() = format;
    *st.args.lock().unwrap() = kw;
    *st.ports.lock().unwrap() = (dport, sport);
    *sh.status_to.lock().unwrap() = Some(sport);
    let id = {
        let mut v = sh.streams.lock().unwrap();
        v.push(st.clone());
        v.len() - 1
    };
    let script = script.clone();
    let status = net.bind(&DatagramOptions::at(local(0))).unwrap();
    let ends = Ends {
        sock,
        status,
        data_to: local(dport),
        status_to: local(sport),
    };
    if dir == 1 {
        std::thread::spawn(move || rx_stream(ends, st, script));
    } else {
        std::thread::spawn(move || tx_stream(ends, st, script));
    }
    let mut p = Packer::default();
    p.i32(id as i32).string(&port.to_string());
    p.buf
}

/// 127.0.0.1 at `port`.
fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// The sockets of a fake stream and where it sends.
struct Ends {
    sock: Box<dyn Datagram>,
    status: Box<dyn Datagram>,
    data_to: SocketAddr,
    status_to: SocketAddr,
}

fn data(seq: u32, elems: usize, bpe: usize, n: u64) -> Vec<u8> {
    let h = DgHeader {
        bytes: (HEADER_LEN + elems * bpe) as u32,
        seq,
        elems: elems as i32,
        flags: 0,
        time: 0,
    };
    let mut v = h.to_bytes().to_vec();
    v.extend((0..elems * bpe).map(|i| (i as u64 * 7 + n) as u8));
    v
}

fn rx_stream(mut e: Ends, st: Arc<StreamState>, script: Arc<Mutex<Script>>) {
    let _ = e.sock.set_read_timeout(Some(Duration::from_micros(100)));
    let mut acked: Option<(u32, u32)> = None;
    let mut first_ignored = false;
    let mut seq: u32 = 0;
    let mut sent: u64 = 0;
    let mut custom_i = 0;
    let mut status_done = false;
    let start = Instant::now();
    let mut b = [0u8; 64];
    loop {
        if st.closed.load(Ordering::Acquire) {
            return;
        }
        let sc = script.lock().unwrap().stream.clone();
        while let Ok((n, _)) = e.sock.recv_from(&mut b) {
            if n >= HEADER_LEN {
                let mut a = [0u8; HEADER_LEN];
                a.copy_from_slice(&b[..HEADER_LEN]);
                st.acks.lock().unwrap().push((Instant::now(), a));
                if sc.ignore_first_ack && !first_ignored {
                    first_ignored = true;
                    continue;
                }
                let h = DgHeader::parse(&a).unwrap();
                acked = Some((h.seq, h.elems.max(1) as u32));
            }
        }
        let Some((ack_seq, window)) = acked else {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        };
        if !st.active.load(Ordering::Acquire)
            || sc.never_send
            || sc.stop_after.is_some_and(|k| sent >= k)
        {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        if !sc.ignore_window && seq.wrapping_sub(ack_seq) >= window && custom_i >= sc.custom.len() {
            std::thread::sleep(Duration::from_micros(200));
            continue;
        }
        let due = start + Duration::from_secs_f64(sent as f64 / sc.rate);
        if Instant::now() < due {
            std::thread::sleep(due - Instant::now());
        }
        let pkt = if custom_i < sc.custom.len() {
            custom_i += 1;
            let p = sc.custom[custom_i - 1].clone();
            if let Some(h) = DgHeader::parse(&p) {
                seq = h.seq.wrapping_add(1);
            }
            p
        } else {
            let p = data(seq, sc.elems, sc.bpe, sent);
            let cur = seq;
            seq = sc
                .jumps
                .iter()
                .find(|j| j.0 == cur)
                .map(|j| j.1)
                .unwrap_or(cur.wrapping_add(1));
            p
        };
        let _ = e.sock.send_to(&pkt, e.data_to);
        sent += 1;
        st.sent.fetch_add(1, Ordering::Relaxed);
        if !status_done && !sc.status.is_empty() {
            status_done = true;
            for code in &sc.status {
                let h = DgHeader {
                    bytes: HEADER_LEN as u32,
                    seq: 1,
                    elems: *code,
                    flags: 0,
                    time: 0,
                };
                let _ = e.status.send_to(&h.to_bytes(), e.status_to);
            }
        }
    }
}

fn tx_stream(mut e: Ends, st: Arc<StreamState>, script: Arc<Mutex<Script>>) {
    let _ = e.sock.set_read_timeout(Some(Duration::from_millis(5)));
    let window = 64u32;
    let mut next: u32;
    let mut acked = 0u32;
    let mut started = false;
    let mut status_done = false;
    let mut b = vec![0u8; 65536];
    loop {
        if st.closed.load(Ordering::Acquire) {
            return;
        }
        let sc = script.lock().unwrap().stream.clone();
        if !started && st.active.load(Ordering::Acquire) {
            started = true;
            if !sc.tx_never_ack {
                let _ = e.sock.send_to(&crate::stream::ack(0, window), e.data_to);
            }
        }
        if let Ok((n, _)) = e.sock.recv_from(&mut b) {
            if let Some(h) = DgHeader::parse(&b[..n]) {
                st.received
                    .lock()
                    .unwrap()
                    .push((h, b[HEADER_LEN..n].to_vec()));
                next = h.seq.wrapping_add(1);
                if !status_done && !sc.tx_status.is_empty() {
                    status_done = true;
                    for code in &sc.tx_status {
                        let s = DgHeader {
                            bytes: HEADER_LEN as u32,
                            seq: 1,
                            elems: *code,
                            flags: 0,
                            time: 0,
                        };
                        let _ = e.status.send_to(&s.to_bytes(), e.status_to);
                    }
                }
                if !sc.tx_never_ack && next.wrapping_sub(acked) >= window / 8 {
                    acked = next;
                    let _ = e.sock.send_to(&crate::stream::ack(next, window), e.data_to);
                }
            }
        }
    }
}
