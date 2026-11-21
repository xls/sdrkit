//! A fake SDRconnect WebSocket server on a fake network that follows the API: the envelope, push and response messages, the
//! client events, the property table per tuner, the binary types, and the tuner filter.

use sdrkit_core::net::{Conn, FakeTransport, Transport};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 5454;

/// One client message, as the driver sends it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Msg {
    pub event_type: String,
    pub property: String,
    pub value: String,
    pub device: Option<String>,
}

/// How the fake answers.
#[derive(Clone, Debug)]
pub enum Mode {
    /// As SDRconnect.
    Normal,
    /// The WebSocket handshake, then nothing.
    Silent,
    /// An HTTP 404 instead of the upgrade.
    NotWebSocket,
}

/// The fake's set-up. `props[0]` is the primary tuner's property table, `props[1]` the secondary's.
#[derive(Clone, Debug)]
pub struct Cfg {
    pub mode: Mode,
    pub props: [BTreeMap<String, String>; 2],
    /// Properties that a `set_property` does not change.
    pub read_only: Vec<String>,
    /// A `set_property device_sample_rate` makes the rate this one instead.
    pub rate_answer: Option<String>,
    /// The constant IQ sample of each tuner.
    pub sample: [(i16, i16); 2],
    /// How often IQ goes out.
    pub period: Duration,
    /// TLS with the test certificate.
    pub tls: bool,
}

pub fn table(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

impl Default for Cfg {
    fn default() -> Self {
        let base = |centre: &str, ant: &str| {
            table(&[
                ("api_version", "1.0.3"),
                ("active_device", "RSPduo 1806030000"),
                ("can_control", "true"),
                ("started", "true"),
                ("device_sample_rate", "100000"),
                ("device_center_frequency", centre),
                ("lna_state_min", "0"),
                ("lna_state_max", "9"),
                ("lna_state", "3"),
                (
                    "valid_antennas",
                    "Tuner 1 50 ohm,Tuner 1 Hi-Z,Tuner 2 50 ohm",
                ),
                ("active_antenna", ant),
                ("overload", "false"),
                ("signal_power", "-71.5"),
                ("signal_snr", "12.25"),
            ])
        };
        Cfg {
            mode: Mode::Normal,
            props: [
                base("100000000", "Tuner 1 50 ohm"),
                base("50000000", "Tuner 2 50 ohm"),
            ],
            read_only: [
                "lna_state_min",
                "lna_state_max",
                "valid_antennas",
                "started",
                "can_control",
                "overload",
                "signal_power",
                "signal_snr",
                "active_device",
                "api_version",
            ]
            .map(String::from)
            .to_vec(),
            rate_answer: None,
            sample: [(16384, -16384), (-8192, 24576)],
            period: Duration::from_millis(10),
            tls: false,
        }
    }
}

/// What the fake saw and holds.
#[derive(Default)]
pub struct St {
    pub cfg: Mutex<Option<Cfg>>,
    /// Every client text message: (connection number, the message).
    pub got: Mutex<Vec<(usize, Msg)>>,
    pub conns: AtomicUsize,
    /// Raw bytes to send on the current connection.
    pub inject: Mutex<Vec<Vec<u8>>>,
    pub drop_now: AtomicBool,
    pub iq_sent: AtomicU64,
    pub stop: AtomicBool,
    /// The current connection's tuner filter (primary, secondary).
    pub enabled: Mutex<[bool; 2]>,
}

pub struct Fake {
    pub net: FakeTransport,
    pub st: Arc<St>,
}

impl Fake {
    pub fn start(cfg: Cfg) -> Fake {
        let net = FakeTransport::new();
        let st = Arc::new(St::default());
        *st.cfg.lock().unwrap() = Some(cfg);
        let s = st.clone();
        net.serve(HOST, PORT, move |conn| {
            let n = s.conns.fetch_add(1, Ordering::SeqCst) + 1;
            serve(conn, n, &s);
        });
        Fake { net, st }
    }

    pub fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }

    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    pub fn conns(&self) -> usize {
        self.st.conns.load(Ordering::SeqCst)
    }

    /// The client messages of connection `conn` (1 is the first).
    pub fn got(&self, conn: usize) -> Vec<Msg> {
        self.st
            .got
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, _)| *c == conn)
            .map(|(_, j)| j.clone())
            .collect()
    }

    /// The client messages of every connection as "event_type property value device" lines.
    pub fn lines(&self, conn: usize) -> Vec<String> {
        self.got(conn)
            .iter()
            .map(|m| {
                format!(
                    "{} {} {} {}",
                    m.event_type,
                    m.property,
                    m.value,
                    m.device.as_deref().unwrap_or("-")
                )
            })
            .collect()
    }

    pub fn send(&self, raw: Vec<u8>) {
        self.st.inject.lock().unwrap().push(raw);
    }

    pub fn send_text(&self, t: &str) {
        self.send(frame(1, t.as_bytes()));
    }

    pub fn drop_client(&self) {
        self.st.drop_now.store(true, Ordering::Release);
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        self.st.stop.store(true, Ordering::Release);
        self.net.stop_serving(HOST, PORT);
    }
}

/// A server frame (never masked).
pub fn frame(op: u8, payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0x80 | op];
    let n = payload.len();
    if n < 126 {
        f.push(n as u8);
    } else if n <= 0xffff {
        f.push(126);
        f.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        f.push(127);
        f.extend_from_slice(&(n as u64).to_be_bytes());
    }
    f.extend_from_slice(payload);
    f
}

/// A binary IQ message of type `ty` with `n` copies of `s`.
pub fn iq_message(ty: u16, s: (i16, i16), n: usize) -> Vec<u8> {
    let mut p = ty.to_le_bytes().to_vec();
    for _ in 0..n {
        p.extend_from_slice(&s.0.to_le_bytes());
        p.extend_from_slice(&s.1.to_le_bytes());
    }
    frame(2, &p)
}

/// A JSON text frame from SDRconnect.
pub fn push(event: &str, property: &str, value: &str, device: &str) -> Vec<u8> {
    #[derive(serde::Serialize)]
    struct Push<'a> {
        event_type: &'a str,
        property: &'a str,
        device: &'a str,
        value: &'a str,
    }
    let j = Push {
        event_type: event,
        property,
        device,
        value,
    };
    frame(1, serde_json::to_string(&j).unwrap().as_bytes())
}

enum Io {
    Plain(Box<dyn Conn>),
    Tls(Box<rustls::StreamOwned<rustls::ServerConnection, Box<dyn Conn>>>),
}

impl Io {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Io::Plain(s) => s.read(b),
            Io::Tls(s) => s.read(b),
        }
    }
    fn write(&mut self, b: &[u8]) -> std::io::Result<()> {
        match self {
            Io::Plain(s) => s.write_all(b),
            Io::Tls(s) => s.write_all(b).and_then(|_| s.flush()),
        }
    }
}

pub const CERT: &[u8] =
    include_bytes!("../../../sdrkit-driver-core/src/ws/testdata/test-server.pem");
const KEY: &[u8] =
    include_bytes!("../../../sdrkit-driver-core/src/ws/testdata/test-server-key.pem");

fn tls_config() -> Arc<rustls::ServerConfig> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let certs = vec![CertificateDer::from_pem_slice(CERT).unwrap()];
    let key = PrivateKeyDer::from_pem_slice(KEY).unwrap();
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap(),
    )
}

fn serve(mut sock: Box<dyn Conn>, conn: usize, st: &St) {
    let cfg = st.cfg.lock().unwrap().clone().unwrap();
    let _ = sock.set_read_timeout(Some(Duration::from_secs(3)));
    let mut shut = sock.try_clone().unwrap();
    let mut io = if cfg.tls {
        let c = rustls::ServerConnection::new(tls_config()).unwrap();
        Io::Tls(Box::new(rustls::StreamOwned::new(c, sock)))
    } else {
        Io::Plain(sock)
    };
    // The upgrade.
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        match io.read(&mut b) {
            Ok(1) => req.push(b[0]),
            _ => return,
        }
    }
    let req = String::from_utf8_lossy(&req).into_owned();
    if matches!(cfg.mode, Mode::NotWebSocket) {
        let _ = io.write(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }
    let key = req
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap_or("")
        .to_string();
    let answer = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        sdrkit_driver_core::ws::accept_for(&key)
    );
    if io.write(answer.as_bytes()).is_err() {
        return;
    }
    let _ = shut.set_read_timeout(Some(Duration::from_millis(2)));
    let mut props = cfg.props.clone();
    // The example library's defaults: primary on, secondary off.
    let mut enabled = [true, false];
    *st.enabled.lock().unwrap() = enabled;
    let mut iq = false;
    let mut buf: Vec<u8> = Vec::new();
    let mut rd = [0u8; 16 << 10];
    let mut next_iq = Instant::now();
    let silent = matches!(cfg.mode, Mode::Silent);
    loop {
        if st.stop.load(Ordering::Acquire) || st.drop_now.swap(false, Ordering::AcqRel) {
            break;
        }
        match io.read(&mut rd) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&rd[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
        let mut out: Vec<Vec<u8>> = std::mem::take(&mut *st.inject.lock().unwrap());
        while let Some((op, payload, used)) = client_frame(&buf) {
            buf.drain(..used);
            match op {
                8 => {
                    let _ = io.write(&frame(8, &payload[..payload.len().min(2)]));
                    shut.shutdown();
                    return;
                }
                1 if !silent => {
                    let Ok(j) = serde_json::from_slice::<Msg>(&payload) else {
                        continue;
                    };
                    st.got.lock().unwrap().push((conn, j.clone()));
                    handle(&j, &cfg, &mut props, &mut enabled, &mut iq, &mut out);
                    *st.enabled.lock().unwrap() = enabled;
                }
                1 => {
                    if let Ok(j) = serde_json::from_slice::<Msg>(&payload) {
                        st.got.lock().unwrap().push((conn, j));
                    }
                }
                _ => {}
            }
        }
        if iq && Instant::now() >= next_iq {
            next_iq += cfg.period;
            let started = props[0]
                .get(crate::proto::prop::STARTED)
                .map(String::as_str)
                == Some("true");
            for (t, on) in enabled.iter().enumerate() {
                let rate: f64 = props[t]
                    .get(crate::proto::prop::RATE)
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(0.0);
                let n = (rate * cfg.period.as_secs_f64()).round() as usize;
                if *on && started && n > 0 {
                    out.push(iq_message([2, 5][t], cfg.sample[t], n));
                    st.iq_sent.fetch_add(n as u64, Ordering::Relaxed);
                }
            }
        }
        for o in out {
            if io.write(&o).is_err() {
                return;
            }
        }
    }
    shut.shutdown();
}

/// One masked client frame at the start of `buf`: (opcode, unmasked payload, bytes used).
fn client_frame(buf: &[u8]) -> Option<(u8, Vec<u8>, usize)> {
    if buf.len() < 2 {
        return None;
    }
    assert!(buf[1] & 0x80 != 0, "a client frame must be masked");
    let (hl, len) = match buf[1] & 0x7f {
        126 if buf.len() >= 4 => (4, u16::from_be_bytes([buf[2], buf[3]]) as usize),
        127 if buf.len() >= 10 => (
            10,
            u64::from_be_bytes(buf[2..10].try_into().unwrap()) as usize,
        ),
        126 | 127 => return None,
        n => (2, n as usize),
    };
    if buf.len() < hl + 4 + len {
        return None;
    }
    let mask = &buf[hl..hl + 4];
    let payload = buf[hl + 4..hl + 4 + len]
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask[i & 3])
        .collect();
    Some((buf[0] & 0x0f, payload, hl + 4 + len))
}

fn handle(
    j: &Msg,
    cfg: &Cfg,
    props: &mut [BTreeMap<String, String>; 2],
    enabled: &mut [bool; 2],
    iq: &mut bool,
    out: &mut Vec<Vec<u8>>,
) {
    let (ev, p, v) = (j.event_type.clone(), j.property.clone(), j.value.clone());
    let d = if j
        .device
        .as_deref()
        .unwrap_or("")
        .eq_ignore_ascii_case("secondary")
    {
        1
    } else {
        0
    };
    let name = ["primary", "secondary"][d];
    match ev.as_str() {
        "set_primary_device_enable" => enabled[0] = v == "true",
        "set_secondary_device_enable" => enabled[1] = v == "true",
        "iq_stream_enable" => *iq = v == "true",
        "device_stream_enable" => {
            for t in props.iter_mut() {
                t.insert("started".into(), v.clone());
            }
            for t in 0..2 {
                if enabled[t] {
                    out.push(push(
                        "property_changed",
                        "started",
                        &v,
                        ["primary", "secondary"][t],
                    ));
                }
            }
        }
        "get_property" => {
            if let (true, Some(val)) = (enabled[d], props[d].get(&p)) {
                out.push(push("get_property_response", &p, val, name));
            }
        }
        "set_property" => {
            if cfg.read_only.contains(&p) {
                return;
            }
            let val = match (&cfg.rate_answer, p.as_str()) {
                (Some(r), "device_sample_rate") => r.clone(),
                _ => v,
            };
            props[d].insert(p.clone(), val.clone());
            if enabled[d] {
                out.push(push("property_changed", &p, &val, name));
            }
        }
        _ => {}
    }
}
