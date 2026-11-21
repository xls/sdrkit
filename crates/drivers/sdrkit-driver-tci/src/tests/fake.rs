//! A fake TCI server on a fake network. One client at a time.

use crate::proto::{self, Header};
use sdrkit_core::net::{Conn, FakeTransport, Transport};
use sdrkit_driver_core::ws;
use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 50001;

/// The two initialisation messages of a server.
pub const INIT1: &str =
    "protocol:ExpertSDR3,1.9;device:SunSDR2DX;receive_only:false;trx_count:2;vfo_limits:10000,30000000;";
pub const INIT2: &str = "if_limits:-24000,24000;modulations_list:AM,LSB,USB;ready;";

/// A server frame (never masked).
pub fn sframe(opcode: u8, payload: &[u8], fin: bool) -> Vec<u8> {
    let mut v = vec![if fin { 0x80 } else { 0 } | opcode];
    let n = payload.len();
    if n <= 125 {
        v.push(n as u8);
    } else if n <= 0xffff {
        v.push(126);
        v.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        v.push(127);
        v.extend_from_slice(&(n as u64).to_be_bytes());
    }
    v.extend_from_slice(payload);
    v
}

pub fn text(t: &str) -> Vec<u8> {
    sframe(1, t.as_bytes(), true)
}

pub fn binary(b: &[u8]) -> Vec<u8> {
    sframe(2, b, true)
}

/// An IQ packet body: header then `n` complex samples of (i, q) in `format`.
pub fn iq_packet(rx: u32, rate: u32, format: u32, n: usize, i: f32, q: f32) -> Vec<u8> {
    let mut p = vec![0u8; proto::HEADER];
    Header {
        receiver: rx,
        sample_rate: rate,
        format,
        length: (2 * n) as u32,
        kind: proto::kind::IQ,
        channels: 2,
        ..Default::default()
    }
    .write(&mut p);
    let enc = |v: f32, out: &mut Vec<u8>| match format {
        0 => out.extend_from_slice(&((v * 32768.0) as i16).to_le_bytes()),
        1 => out.extend_from_slice(&((v * 8_388_608.0) as i32).to_le_bytes()[..3]),
        2 => out.extend_from_slice(&((v as f64 * 2_147_483_648.0) as i32).to_le_bytes()),
        _ => out.extend_from_slice(&v.to_le_bytes()),
    };
    for _ in 0..n {
        enc(i, &mut p);
        enc(q, &mut p);
    }
    p
}

/// How the fake answers the upgrade.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// 101 with the right accept value, then the initialisation.
    Normal,
    /// An SSH greeting instead of HTTP.
    Plain,
    /// Accepts and never answers.
    Silent,
    /// This header instead of the right one (then the initialisation).
    Answer(String),
}

#[derive(Clone, Debug)]
pub struct Cfg {
    pub mode: Mode,
    /// Raw bytes sent right after the upgrade (default: the two frames of [`INIT1`] and [`INIT2`]).
    pub init: Vec<Vec<u8>>,
    /// Send `stop;` after the initialisation and stream only after `start`.
    pub radio_stopped: bool,
    /// The sample type of streamed packets.
    pub format: u32,
    /// Stream on `iq_start`.
    pub stream: bool,
    /// Answer `iq_samplerate` with this rate instead (and use it in the headers).
    pub rate_answer: Option<u32>,
    /// Echo `dds` back.
    pub echo: bool,
    /// Keep streaming after `iq_stop` and the client's close frame, until a write fails.
    pub ignore_close: bool,
    /// Packet interval.
    pub interval: Duration,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            mode: Mode::Normal,
            init: vec![text(INIT1), text(INIT2)],
            radio_stopped: false,
            format: 3,
            stream: true,
            rate_answer: None,
            echo: true,
            ignore_close: false,
            interval: Duration::from_millis(100),
        }
    }
}

/// One client frame as received.
#[derive(Clone, Debug)]
pub struct Frame {
    pub opcode: u8,
    pub payload: Vec<u8>,
    pub raw: Vec<u8>,
}

#[derive(Default)]
pub struct State {
    pub cfg: Mutex<Cfg>,
    pub conns: AtomicUsize,
    pub requests: Mutex<Vec<String>>,
    pub cmds: Mutex<Vec<String>>,
    pub frames: Mutex<Vec<Frame>>,
    pub inject: Mutex<VecDeque<Vec<u8>>>,
    pub drop_now: AtomicBool,
    pub quit: AtomicBool,
    pub connected: AtomicBool,
    pub packets: AtomicUsize,
    /// Held while a client is served.
    pub one: Mutex<()>,
}

pub struct Fake {
    pub net: FakeTransport,
    pub st: Arc<State>,
}

impl Fake {
    pub fn start(cfg: Cfg) -> Fake {
        let net = FakeTransport::new();
        let st = Arc::new(State {
            cfg: Mutex::new(cfg),
            ..Default::default()
        });
        let s = st.clone();
        net.serve(HOST, PORT, move |sock| {
            let _one = s
                .one
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if s.quit.load(Ordering::Acquire) {
                return;
            }
            s.conns.fetch_add(1, Ordering::AcqRel);
            s.connected.store(true, Ordering::Release);
            serve(sock, &s);
            s.connected.store(false, Ordering::Release);
        });
        Fake { net, st }
    }

    pub fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }

    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }
    pub fn cfg(&self, f: impl FnOnce(&mut Cfg)) {
        f(&mut self.st.cfg.lock().unwrap());
    }
    pub fn cmds(&self) -> Vec<String> {
        self.st.cmds.lock().unwrap().clone()
    }
    pub fn frames(&self) -> Vec<Frame> {
        self.st.frames.lock().unwrap().clone()
    }
    pub fn count(&self, cmd: &str) -> usize {
        self.cmds().iter().filter(|c| *c == cmd).count()
    }
    pub fn conns(&self) -> usize {
        self.st.conns.load(Ordering::Acquire)
    }
    pub fn send(&self, raw: Vec<u8>) {
        self.st.inject.lock().unwrap().push_back(raw);
    }
    pub fn drop_client(&self) {
        self.st.drop_now.store(true, Ordering::Release);
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.st.quit.store(true, Ordering::Release);
        self.net.stop_serving(HOST, PORT);
    }
}

fn read_request(sock: &mut Box<dyn Conn>) -> Option<String> {
    let _ = sock.set_read_timeout(Some(Duration::from_secs(5)));
    let mut h = Vec::new();
    let mut b = [0u8; 1];
    while !h.ends_with(b"\r\n\r\n") {
        match sock.read(&mut b) {
            Ok(1) => h.push(b[0]),
            _ => return None,
        }
    }
    Some(String::from_utf8_lossy(&h).into_owned())
}

fn key_of(req: &str) -> String {
    req.split("\r\n")
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap_or("")
        .to_string()
}

fn hold(sock: &mut Box<dyn Conn>, st: &State) {
    // Keep the connection open, reading whatever comes, until the client goes, a drop or a quit.
    let _ = sock.set_read_timeout(Some(Duration::from_millis(10)));
    let mut b = [0u8; 1024];
    loop {
        if st.quit.load(Ordering::Acquire) || st.drop_now.swap(false, Ordering::AcqRel) {
            return;
        }
        match sock.read(&mut b) {
            Ok(0) => return,
            Ok(_) => {}
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
    }
}

fn serve(mut sock: Box<dyn Conn>, st: &State) {
    let Some(req) = read_request(&mut sock) else {
        return;
    };
    st.requests.lock().unwrap().push(req.clone());
    let cfg = st.cfg.lock().unwrap().clone();
    match &cfg.mode {
        Mode::Silent => return hold(&mut sock, st),
        Mode::Plain => {
            let _ = sock.write_all(b"SSH-2.0-not a websocket\r\n\r\n");
            return hold(&mut sock, st);
        }
        Mode::Normal => {
            let a = format!(
                "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                ws::accept_for(&key_of(&req))
            );
            let _ = sock.write_all(a.as_bytes());
        }
        Mode::Answer(a) => {
            let _ = sock.write_all(a.as_bytes());
        }
    }
    for m in &cfg.init {
        let _ = sock.write_all(m);
    }
    if cfg.radio_stopped {
        let _ = sock.write_all(&text("stop;"));
    }
    let _ = sock.set_read_timeout(Some(Duration::from_millis(5)));
    let mut buf: Vec<u8> = Vec::new();
    let mut rbuf = vec![0u8; 65536];
    let mut streaming: Option<u32> = None;
    let mut running = !cfg.radio_stopped;
    let mut rate: u32 = 48_000;
    let mut last = Instant::now();
    let mut client_closed = false;
    loop {
        if st.quit.load(Ordering::Acquire) {
            return;
        }
        if st.drop_now.swap(false, Ordering::AcqRel) {
            sock.shutdown();
            return;
        }
        let cfg = st.cfg.lock().unwrap().clone();
        if !client_closed {
            match sock.read(&mut rbuf) {
                Ok(0) => {
                    if !cfg.ignore_close {
                        return;
                    }
                    client_closed = true;
                }
                Ok(n) => buf.extend_from_slice(&rbuf[..n]),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => return,
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        // Client frames.
        while let Some(f) = take_frame(&mut buf) {
            st.frames.lock().unwrap().push(f.clone());
            match f.opcode {
                1 => {
                    let t = String::from_utf8_lossy(&f.payload).into_owned();
                    for c in proto::parse(&t) {
                        let rec = if c.args.is_empty() {
                            c.name.clone()
                        } else {
                            format!("{}:{}", c.name, c.args.join(","))
                        };
                        st.cmds.lock().unwrap().push(rec);
                        match c.name.as_str() {
                            "iq_start" => {
                                if cfg.stream {
                                    streaming = c.arg(0).parse().ok();
                                }
                            }
                            "iq_stop" if !cfg.ignore_close => streaming = None,
                            "iq_samplerate" => {
                                let asked: u32 = c.arg(0).parse().unwrap_or(48_000);
                                rate = cfg.rate_answer.unwrap_or(asked);
                                let _ = sock.write_all(&text(&format!("iq_samplerate:{rate};")));
                            }
                            "start" => {
                                running = true;
                                let _ = sock.write_all(&text("start;"));
                            }
                            "dds" if cfg.echo => {
                                let _ = sock.write_all(&text(&format!(
                                    "dds:{},{};",
                                    c.arg(0),
                                    c.arg(1)
                                )));
                            }
                            _ => {}
                        }
                    }
                }
                8 if !cfg.ignore_close => {
                    let _ = sock.write_all(&sframe(8, &f.payload, true));
                    sock.shutdown();
                    return;
                }
                _ => {}
            }
        }
        // Injected bytes.
        let inject: Vec<Vec<u8>> = st.inject.lock().unwrap().drain(..).collect();
        for raw in inject {
            if sock.write_all(&raw).is_err() {
                return;
            }
        }
        // Streaming.
        if let Some(rx) = streaming {
            if running && last.elapsed() >= cfg.interval {
                last = Instant::now();
                let p = iq_packet(rx, rate, cfg.format, 1024, 0.5, -0.25);
                if sock.write_all(&binary(&p)).is_err() {
                    return;
                }
                st.packets.fetch_add(1, Ordering::AcqRel);
            }
        }
    }
}

fn take_frame(buf: &mut Vec<u8>) -> Option<Frame> {
    if buf.len() < 2 {
        return None;
    }
    let opcode = buf[0] & 0x0f;
    assert!(buf[1] & 0x80 != 0, "a client frame must be masked");
    let (len, mut at) = match buf[1] & 0x7f {
        126 if buf.len() >= 4 => (u16::from_be_bytes([buf[2], buf[3]]) as usize, 4),
        127 if buf.len() >= 10 => (
            u64::from_be_bytes(buf[2..10].try_into().unwrap()) as usize,
            10,
        ),
        126 | 127 => return None,
        n => (n as usize, 2),
    };
    if buf.len() < at + 4 + len {
        return None;
    }
    let mask = [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]];
    at += 4;
    let payload: Vec<u8> = buf[at..at + len]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask[i & 3])
        .collect();
    let raw: Vec<u8> = buf.drain(..at + len).collect();
    Some(Frame {
        opcode,
        payload,
        raw,
    })
}
