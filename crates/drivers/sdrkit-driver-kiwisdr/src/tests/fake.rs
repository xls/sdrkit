//! A fake KiwiSDR on a fake network. It answers the WebSocket upgrade, plays the handshake of the chosen behaviour, streams IQ
//! or waterfall frames, and records every text frame it receives, per connection.

use sdrkit_core::net::{Conn, FakeTransport};
use sdrkit_driver_core::ws::accept_for;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// The address the fake serves on.
pub const HOST: &str = "127.0.0.1";
/// The port the fake serves on.
pub const PORT: u16 = 8073;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Busy,
    BadPw,
    Down,
    Redirect,
    Limit,
    /// Answers the upgrade with an HTTP 404.
    Plain,
    /// Accepts the connection and never answers.
    Silent,
    /// Answers the upgrade, never sends `sample_rate`.
    NoRate,
    /// Answers the upgrade with a wrong accept value.
    WrongAccept,
}

/// A change that the test requests on the running IQ connection.
#[derive(Clone, Debug, PartialEq)]
pub enum Switch {
    Drop,
    Skip5,
    Junk,
    Oversize,
    RestartSeq,
    Stats,
    Audio,
    Split,
    InactivityClose,
    Msg(String),
    Raw(Vec<u8>),
    Silence,
}

#[derive(Default)]
pub struct Shared {
    pub conns: AtomicUsize,
    pub open: AtomicUsize,
    log: Mutex<Vec<(usize, String, String)>>,
    paths: Mutex<Vec<String>>,
    switches: Mutex<Vec<Switch>>,
    mode: Mutex<Option<Mode>>,
    rate: Mutex<String>,
    pub unmasked: AtomicUsize,
    pub accepted_at: Mutex<Vec<Instant>>,
}

pub struct Fake {
    net: FakeTransport,
    pub sh: Arc<Shared>,
}

pub fn server_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0x80 | opcode];
    let n = payload.len();
    if n < 126 {
        v.push(n as u8);
    } else if n < 65536 {
        v.push(126);
        v.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        v.push(127);
        v.extend_from_slice(&(n as u64).to_be_bytes());
    }
    v.extend_from_slice(payload);
    v
}

pub fn msg(s: &str) -> Vec<u8> {
    server_frame(2, format!("MSG {s}").as_bytes())
}

/// An IQ frame with GPS time, big-endian samples.
pub fn iq_payload(flags: u8, seq: u32, smeter: u16, samples: &[(i16, i16)]) -> Vec<u8> {
    let mut v = b"SND".to_vec();
    v.push(flags);
    v.extend_from_slice(&seq.to_le_bytes());
    v.extend_from_slice(&smeter.to_be_bytes());
    v.extend_from_slice(&[1, 0]);
    v.extend_from_slice(&123_456u32.to_le_bytes());
    v.extend_from_slice(&500_000_000u32.to_le_bytes());
    for &(i, q) in samples {
        v.extend_from_slice(&i.to_be_bytes());
        v.extend_from_slice(&q.to_be_bytes());
    }
    v
}

pub const USERS: &str = "user_cb=%5B%7B%22i%22%3A0%2C%22n%22%3A%22sdrkit%22%2C%22f%22%3A7074000%7D%2C%7B%22i%22%3A1%7D%5D";

/// The samples of every streamed IQ frame.
pub const FRAME_SAMPLE: (i16, i16) = (16384, -8192);

impl Fake {
    pub fn new(mode: Mode) -> Fake {
        Self::with_rate(mode, "12001.135")
    }

    pub fn with_rate(mode: Mode, rate: &str) -> Fake {
        let net = FakeTransport::new();
        let sh = Arc::new(Shared::default());
        *sh.mode.lock().unwrap() = Some(mode);
        *sh.rate.lock().unwrap() = rate.into();
        let s = sh.clone();
        net.serve(HOST, PORT, move |c| {
            let id = s.conns.fetch_add(1, Ordering::SeqCst) + 1;
            s.accepted_at.lock().unwrap().push(Instant::now());
            s.open.fetch_add(1, Ordering::SeqCst);
            let _ = serve(c, id, &s);
            s.open.fetch_sub(1, Ordering::SeqCst);
        });
        Fake { net, sh }
    }

    pub fn net(&self) -> FakeTransport {
        self.net.clone()
    }
    pub fn addr(&self) -> String {
        format!("{HOST}:{PORT}")
    }
    pub fn conns(&self) -> usize {
        self.sh.conns.load(Ordering::SeqCst)
    }
    pub fn open(&self) -> usize {
        self.sh.open.load(Ordering::SeqCst)
    }
    pub fn switch(&self, s: Switch) {
        self.sh.switches.lock().unwrap().push(s);
    }
    pub fn set_rate(&self, r: &str) {
        *self.sh.rate.lock().unwrap() = r.into();
    }
    /// The text frames of connection `id` on `kind` ("SND" or "W/F").
    pub fn lines(&self, id: usize, kind: &str) -> Vec<String> {
        self.sh
            .log
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, k, _)| *c == id && k == kind)
            .map(|(_, _, t)| t.clone())
            .collect()
    }
    pub fn all_lines(&self) -> Vec<String> {
        self.sh
            .log
            .lock()
            .unwrap()
            .iter()
            .map(|(_, _, t)| t.clone())
            .collect()
    }
    pub fn paths(&self) -> Vec<String> {
        self.sh.paths.lock().unwrap().clone()
    }
}

fn read_exact(s: &mut Box<dyn Conn>, b: &mut [u8]) -> bool {
    s.read_exact(b).is_ok()
}

/// Reads client frames, unmasks them and passes them on. It ends at the end of the stream or a close frame.
fn client_reader(mut s: Box<dyn Conn>, tx: mpsc::Sender<String>, sh: Arc<Shared>) {
    loop {
        let mut h = [0u8; 2];
        if !read_exact(&mut s, &mut h) {
            return;
        }
        let opcode = h[0] & 15;
        if h[1] & 0x80 == 0 {
            sh.unmasked.fetch_add(1, Ordering::SeqCst);
        }
        let mut len = (h[1] & 0x7f) as usize;
        if len == 126 {
            let mut e = [0u8; 2];
            if !read_exact(&mut s, &mut e) {
                return;
            }
            len = u16::from_be_bytes(e) as usize;
        } else if len == 127 {
            let mut e = [0u8; 8];
            if !read_exact(&mut s, &mut e) {
                return;
            }
            len = u64::from_be_bytes(e) as usize;
        }
        let mut mask = [0u8; 4];
        if h[1] & 0x80 != 0 && !read_exact(&mut s, &mut mask) {
            return;
        }
        let mut p = vec![0u8; len];
        if !read_exact(&mut s, &mut p) {
            return;
        }
        for (i, b) in p.iter_mut().enumerate() {
            *b ^= mask[i & 3];
        }
        match opcode {
            1 => {
                if tx.send(String::from_utf8_lossy(&p).into_owned()).is_err() {
                    return;
                }
            }
            8 => {
                let _ = tx.send(format!("CLOSE {p:02x?}"));
                return;
            }
            _ => {}
        }
    }
}

fn read_request(c: &mut Box<dyn Conn>) -> Option<(String, String)> {
    let mut req = Vec::new();
    let mut b = [0u8; 1];
    while !req.ends_with(b"\r\n\r\n") {
        if c.read(&mut b).ok()? == 0 {
            return None;
        }
        req.push(b[0]);
    }
    let text = String::from_utf8_lossy(&req).into_owned();
    let path = text.lines().next()?.split(' ').nth(1)?.to_string();
    let key = text
        .lines()
        .find_map(|l| l.strip_prefix("Sec-WebSocket-Key: "))
        .unwrap_or("")
        .trim()
        .to_string();
    Some((path, key))
}

fn serve(mut c: Box<dyn Conn>, id: usize, sh: &Arc<Shared>) -> std::io::Result<()> {
    let mode = sh.mode.lock().unwrap().unwrap();
    if mode == Mode::Silent {
        std::thread::sleep(Duration::from_secs(4));
        return Ok(());
    }
    let Some((path, key)) = read_request(&mut c) else {
        return Ok(());
    };
    sh.paths.lock().unwrap().push(path.clone());
    if mode == Mode::Plain {
        c.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n")?;
        return Ok(());
    }
    let accept = if mode == Mode::WrongAccept {
        "AAAA".to_string()
    } else {
        accept_for(&key)
    };
    c.write_all(format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").as_bytes())?;
    let wf = path.ends_with("/W/F");
    let kind = if wf { "W/F" } else { "SND" };
    let (tx, rx) = mpsc::channel();
    let r = c.try_clone().map_err(std::io::Error::from)?;
    let s2 = sh.clone();
    std::thread::spawn(move || client_reader(r, tx, s2));
    let mut streaming = false;
    let mut seq: u32 = 100;
    let mut next = Instant::now();
    let (mut stats, mut audio, mut silent) = (false, false, false);
    let frame_samples = vec![FRAME_SAMPLE; 512];
    let mut wf_on = false;
    let mut wf_zoom = 0u32;
    let mut wf_seq = 0u32;
    loop {
        match rx.recv_timeout(Duration::from_millis(3)) {
            Ok(line) => {
                sh.log.lock().unwrap().push((id, kind.into(), line.clone()));
                if line.starts_with("CLOSE") {
                    let _ = c.write_all(&server_frame(8, &[0x03, 0xe8]));
                    c.shutdown();
                    return Ok(());
                }
                if line.starts_with("SET auth") {
                    match mode {
                        Mode::Busy => c.write_all(&msg("too_busy=4"))?,
                        Mode::BadPw => c.write_all(&msg("badp=1"))?,
                        Mode::Down => c.write_all(&msg("down=1"))?,
                        Mode::Redirect => {
                            c.write_all(&msg("redirect=http%3A%2F%2Fkiwi2.example.org%3A8074%2F"))?
                        }
                        Mode::Limit => {
                            c.write_all(&msg("ip_limit=60"))?;
                            c.shutdown();
                            return Ok(());
                        }
                        Mode::NoRate => c.write_all(&msg("version_maj=1 version_min=800"))?,
                        _ if wf => c.write_all(&msg("wf_setup"))?,
                        _ => {
                            let rate = sh.rate.lock().unwrap().clone();
                            for m in [
                                "version_maj=1 version_min=800".to_string(),
                                "bandwidth=30000000 freq_offset=0.000".into(),
                                "rx_chans=4 chan_no_pwd=0".into(),
                                "badp=0".into(),
                                "load_cfg=%7B%22rx_name%22%3A%22Fake%20Kiwi%22%2C%22rx_gps%22%3A%22(51.5%2C%20-0.12)%22%7D".into(),
                                "inactivity_timeout=30".into(),
                                "audio_rate=12000".into(),
                                format!("sample_rate={rate}"),
                            ] {
                                c.write_all(&msg(&m))?;
                            }
                        }
                    }
                } else if line.starts_with("SET mod=") {
                    streaming = true;
                } else if line == "SET GET_USERS" {
                    c.write_all(&msg(USERS))?;
                } else if let Some(z) = line.strip_prefix("SET zoom=") {
                    wf_zoom = z.split(' ').next().unwrap().parse().unwrap();
                } else if line == "SET wf_comp=0" {
                    wf_on = true;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !wf {
            let sw: Vec<Switch> = std::mem::take(&mut *sh.switches.lock().unwrap());
            for s in sw {
                match s {
                    Switch::Drop => {
                        c.shutdown();
                        return Ok(());
                    }
                    Switch::Skip5 => seq = seq.wrapping_add(5),
                    Switch::RestartSeq => seq = 0,
                    Switch::Stats => stats = true,
                    Switch::Audio => audio = true,
                    Switch::Silence => silent = true,
                    Switch::Msg(m) => c.write_all(&msg(&m))?,
                    Switch::Raw(b) => c.write_all(&b)?,
                    Switch::Junk => {
                        let full = iq_payload(0x08, 1, 700, &[(1, 1); 4]);
                        let mut f18 = full.clone();
                        f18[3] = 0x18;
                        let mut f00 = full.clone();
                        f00[3] = 0x00;
                        let nos = iq_payload(0x08, 1, 700, &[]);
                        let big = iq_payload(0x08, 1, 700, &vec![(1, 1); 9000]);
                        let mut wfz = b"W/F".to_vec();
                        wfz.extend_from_slice(&[0u8; 18]);
                        for p in [
                            b"SN".to_vec(),
                            full[..15].to_vec(),
                            f18,
                            f00,
                            nos,
                            big,
                            wfz,
                            b"MSG no_equals_token a=%zz b=%41 foo".to_vec(),
                        ] {
                            c.write_all(&server_frame(2, &p))?;
                        }
                        c.write_all(&[0x8a, 0x04, 0x70, 0x6f, 0x6e, 0x67])?;
                    }
                    Switch::Oversize => {
                        let mut h = vec![0x82, 127];
                        h.extend_from_slice(&((1u64 << 20) + 1).to_be_bytes());
                        let _ = c.write_all(&h);
                        let _ = c.write_all(&vec![0u8; (1 << 20) + 1]);
                    }
                    Switch::Split => {
                        let f = server_frame(2, &iq_payload(0x08, seq, 700, &frame_samples));
                        seq = seq.wrapping_add(1);
                        c.write_all(&f[..700])?;
                        std::thread::sleep(Duration::from_millis(1400));
                        c.write_all(&f[700..])?;
                    }
                    Switch::InactivityClose => {
                        c.write_all(&msg("inactivity_timeout=30"))?;
                        let _ = c.write_all(&server_frame(8, &[0x03, 0xe8]));
                        c.shutdown();
                        return Ok(());
                    }
                }
            }
        }
        let now = Instant::now();
        if wf && wf_on && now >= next {
            next = now + Duration::from_millis(100);
            let mut p = b"W/F\0".to_vec();
            p.extend_from_slice(&0u32.to_le_bytes());
            p.extend_from_slice(&wf_zoom.to_le_bytes());
            p.extend_from_slice(&wf_seq.to_le_bytes());
            p.extend((0..1024).map(|i| (i % 256) as u8));
            wf_seq += 1;
            c.write_all(&server_frame(2, &p))?;
        }
        if !wf && streaming && !silent && now >= next {
            next += Duration::from_millis(30);
            if next < now {
                next = now + Duration::from_millis(30);
            }
            if stats {
                c.write_all(&msg("stats=1"))?;
            } else if audio {
                let mut p = iq_payload(0x00, seq, 700, &[]);
                p.truncate(10);
                p.extend_from_slice(&[0u8; 1024]);
                c.write_all(&server_frame(2, &p))?;
            } else {
                c.write_all(&server_frame(
                    2,
                    &iq_payload(0x08, seq, 700, &frame_samples),
                ))?;
            }
            seq = seq.wrapping_add(1);
        }
    }
}
