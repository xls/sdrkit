//! A fake OpenWebRX server on a fake network. The test thread writes the server side of each connection by hand, so every
//! byte of a transcript is in the test.

use sdrkit_core::net::{Conn, FakeTransport, Transport};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HOST: &str = "127.0.0.1";
pub const PORT: u16 = 8073;
pub const ANSWER_101: &str = "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
pub const CONFIG_20M: &str = r#"{"type":"config","value":{"sdr_id":"rtl","profile_id":"20m","center_freq":14100000,"samp_rate":250000,"fft_size":4096,"audio_compression":"adpcm","fft_compression":"adpcm","max_clients":20,"start_mod":"usb","start_offset_freq":74000,"waterfall_levels":{"min":-88,"max":-20},"tuning_step":10}}"#;
pub const PROFILES: &str = r#"{"type":"profiles","value":[{"id":"rtl|20m","name":"RTL 20m"},{"id":"rtl|40m","name":"RTL 40m"}]}"#;

/// A server frame (never masked).
pub fn sframe(op: u8, p: &[u8]) -> Vec<u8> {
    let mut v = vec![0x80 | op];
    let n = p.len();
    if n < 126 {
        v.push(n as u8);
    } else if n <= 0xFFFF {
        v.push(126);
        v.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        v.push(127);
        v.extend_from_slice(&(n as u64).to_be_bytes());
    }
    v.extend_from_slice(p);
    v
}

/// A client frame as the fake server read it.
#[derive(Clone, Debug)]
pub struct CFrame {
    pub op: u8,
    pub payload: Vec<u8>,
    pub raw: Vec<u8>,
}

/// The fields of a client message that the tests look at.
#[derive(Debug, serde::Deserialize)]
pub struct Sent {
    #[serde(rename = "type")]
    pub ty: String,
    pub params: Option<Params>,
}

/// The `dspcontrol` parameters that the tests look at.
#[derive(Debug, serde::Deserialize)]
pub struct Params {
    pub offset_freq: Option<i64>,
    pub low_cut: Option<i64>,
    pub dmr_filter: Option<i64>,
    #[serde(rename = "mod")]
    pub modulation: Option<String>,
}

impl CFrame {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.payload).to_string()
    }
    pub fn sent(&self) -> Sent {
        serde_json::from_slice(&self.payload).unwrap()
    }
    pub fn params(&self) -> Params {
        self.sent().params.unwrap()
    }
}

pub fn read_cframe(r: &mut impl Read) -> Option<CFrame> {
    let mut h = [0u8; 2];
    r.read_exact(&mut h).ok()?;
    let mut raw = h.to_vec();
    assert!(h[1] & 0x80 != 0, "client frames are masked");
    let len = match h[1] & 0x7F {
        126 => {
            let mut b = [0u8; 2];
            r.read_exact(&mut b).ok()?;
            raw.extend_from_slice(&b);
            u16::from_be_bytes(b) as usize
        }
        127 => {
            let mut b = [0u8; 8];
            r.read_exact(&mut b).ok()?;
            raw.extend_from_slice(&b);
            u64::from_be_bytes(b) as usize
        }
        n => n as usize,
    };
    let mut mask = [0u8; 4];
    r.read_exact(&mut mask).ok()?;
    raw.extend_from_slice(&mask);
    let mut p = vec![0u8; len];
    r.read_exact(&mut p).ok()?;
    raw.extend_from_slice(&p);
    for (i, b) in p.iter_mut().enumerate() {
        *b ^= mask[i & 3];
    }
    Some(CFrame {
        op: h[0] & 0x0F,
        payload: p,
        raw,
    })
}

pub fn read_head(r: &mut impl Read) -> Option<String> {
    let mut v = Vec::new();
    let mut b = [0u8; 1];
    while !v.ends_with(b"\r\n\r\n") {
        if r.read(&mut b).ok()? == 0 {
            return None;
        }
        v.push(b[0]);
    }
    Some(String::from_utf8_lossy(&v).to_string())
}

/// A listening address on a fake network. Each connection waits in a queue until the test takes it.
pub struct Fake {
    pub net: FakeTransport,
    pub port: u16,
    conns: Mutex<Receiver<(Box<dyn Conn>, Instant)>>,
}

impl Fake {
    /// A fake on its own network at `127.0.0.1:8073`.
    pub fn new() -> Fake {
        Fake::on(&FakeTransport::new(), PORT)
    }

    /// A fake at `127.0.0.1:port` of `net`.
    pub fn on(net: &FakeTransport, port: u16) -> Fake {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        net.serve(HOST, port, move |c| {
            let _ = tx.lock().unwrap().send((c, Instant::now()));
        });
        Fake {
            net: net.clone(),
            port,
            conns: Mutex::new(rx),
        }
    }

    pub fn addr(&self) -> String {
        format!("{HOST}:{}", self.port)
    }

    pub fn transport(&self) -> Arc<dyn Transport> {
        Arc::new(self.net.clone())
    }

    pub fn try_accept(&self, d: Duration) -> Option<Remote> {
        let (c, at) = self.conns.lock().unwrap().recv_timeout(d).ok()?;
        Some(Remote::new(c, at))
    }

    pub fn accept(&self) -> Remote {
        self.try_accept(Duration::from_secs(10))
            .expect("the client connects")
    }
}

/// One connection, server side. A thread reads the client frames into a channel. The test writes.
pub struct Remote {
    w: Box<dyn Conn>,
    pub req: String,
    frames: Receiver<CFrame>,
    pub at: Instant,
}

impl Remote {
    fn new(mut s: Box<dyn Conn>, at: Instant) -> Remote {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let req = read_head(&mut s).unwrap_or_default();
        s.set_read_timeout(None).unwrap();
        let mut r = s.try_clone().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            while let Some(f) = read_cframe(&mut r) {
                if tx.send(f).is_err() {
                    break;
                }
            }
        });
        Remote {
            w: s,
            req,
            frames: rx,
            at,
        }
    }

    pub fn send(&mut self, b: &[u8]) {
        let _ = self.w.write_all(b);
    }
    pub fn text(&mut self, t: &str) {
        self.send(&sframe(1, t.as_bytes()));
    }
    pub fn bin(&mut self, b: &[u8]) {
        self.send(&sframe(2, b));
    }
    pub fn upgrade(&mut self) {
        self.send(ANSWER_101.as_bytes());
    }
    pub fn next(&self, d: Duration) -> Option<CFrame> {
        self.frames.recv_timeout(d).ok()
    }
    /// The next frame that is not a keep-alive ping.
    pub fn data(&self) -> CFrame {
        loop {
            let f = self.next(Duration::from_secs(5)).expect("a client frame");
            if f.op != 0x9 {
                return f;
            }
        }
    }
    /// No frame but pings and pongs for `d`.
    pub fn quiet(&self, d: Duration) -> bool {
        let end = Instant::now() + d;
        while Instant::now() < end {
            match self.frames.recv_timeout(end - Instant::now()) {
                Ok(f) if f.op == 0x9 || f.op == 0xA => {}
                Ok(f) => panic!("unexpected client frame {:?}", f.text()),
                Err(_) => return true,
            }
        }
        true
    }
    /// The client closed the connection within `d`.
    pub fn closed(&self, d: Duration) -> bool {
        let end = Instant::now() + d;
        loop {
            match self
                .frames
                .recv_timeout(end.saturating_duration_since(Instant::now()))
            {
                Ok(_) => {}
                Err(RecvTimeoutError::Disconnected) => return true,
                Err(RecvTimeoutError::Timeout) => return false,
            }
        }
    }
    pub fn greeting(&self) -> (CFrame, CFrame) {
        (self.data(), self.data())
    }
    pub fn hello(&mut self, v: &str) {
        self.text(&format!("CLIENT DE SERVER server=openwebrx version={v}"));
    }
    /// The upgrade, the greeting, the hello, the 20 m configuration and the profile list.
    pub fn standard(&mut self) {
        self.upgrade();
        self.greeting();
        self.hello("1.2.2");
        self.text(CONFIG_20M);
        self.text(PROFILES);
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        self.w.shutdown();
    }
}

pub const CERT: &[u8] =
    include_bytes!("../../../sdrkit-driver-core/src/ws/testdata/test-server.pem");
const KEY: &[u8] =
    include_bytes!("../../../sdrkit-driver-core/src/ws/testdata/test-server-key.pem");
pub const CA: &[u8] = include_bytes!("../../../sdrkit-driver-core/src/ws/testdata/test-ca.pem");

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

/// A TLS server at `127.0.0.1:port` of `net` that runs the standard script on one connection and returns the request, the
/// texts it read and the opcode of the close of the client.
pub fn tls_server(net: &FakeTransport, port: u16) -> Receiver<Option<Vec<String>>> {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    net.serve(HOST, port, move |mut sock| {
        let run = || -> Option<Vec<String>> {
            sock.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
            let c = rustls::ServerConnection::new(tls_config()).ok()?;
            let mut s = rustls::StreamOwned::new(c, sock);
            let req = read_head(&mut s)?;
            s.write_all(ANSWER_101.as_bytes()).ok()?;
            let mut got = vec![req];
            for _ in 0..2 {
                got.push(read_cframe(&mut s)?.text());
            }
            s.write_all(&sframe(
                1,
                b"CLIENT DE SERVER server=openwebrx version=1.2.2",
            ))
            .ok()?;
            s.write_all(&sframe(1, CONFIG_20M.as_bytes())).ok()?;
            s.flush().ok()?;
            for _ in 0..2 {
                got.push(read_cframe(&mut s)?.text());
            }
            got.push(format!("{}", read_cframe(&mut s)?.op));
            Some(got)
        };
        let _ = tx.lock().unwrap().send(run());
    });
    rx
}
