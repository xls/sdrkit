//! Addresses, the connection and the WebSocket upgrade of an OpenWebRX server. The frames go through the shared WebSocket
//! client of `sdrkit_driver_core::ws`. This module adds the request headers, redirects, the port fallback and the answer to
//! each HTTP status.
//!
//! Standard: RFC 6455 section 4.1 (the opening handshake), RFC 7617 (Basic authentication).

use crate::{OwrxError, Timing};
use sdrkit_core::base64::{self, Alphabet};
use sdrkit_core::net::{self as core_net, NetError, Transport};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::ws::{self, Link, Reader, Writer};
use sdrkit_driver_core::DriverError;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The port of an OpenWebRX server that an address without a port names.
pub const DEFAULT_PORT: u16 = 8073;

/// Where the WebSocket is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// wss.
    pub secure: bool,
    /// The host, without brackets.
    pub host: String,
    /// Ports to try in turn (80 then 8073 for an `http://` link without a port).
    pub ports: Vec<u16>,
    /// The WebSocket path.
    pub path: String,
}

fn join_ws(base: &str) -> String {
    let b = base.trim_end_matches('/');
    let b = b.trim_start_matches('/');
    if b.is_empty() {
        "/ws/".into()
    } else {
        format!("/{b}/ws/")
    }
}

/// Parses an address: `host`, `host:port`, `[v6]:port`, or an `http`, `https`, `ws` or `wss` link with a path.
///
/// # Errors
///
/// [`DriverError::Address`] for text that is not an address.
pub fn parse_address(address: &str) -> Result<Target, DriverError> {
    let a = address.trim();
    let bad = || DriverError::Address;
    let lower = a.to_ascii_lowercase();
    let (scheme, rest) = ["http://", "https://", "ws://", "wss://"]
        .iter()
        .find(|p| lower.starts_with(*p))
        .map(|p| (Some(p.trim_end_matches("://")), &a[p.len()..]))
        .unwrap_or((None, a));
    let rest = rest.split(['#', '?']).next().unwrap_or("");
    let (auth, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (host, port) = if let Some(r) = auth.strip_prefix('[') {
        let end = r.find(']').ok_or_else(bad)?;
        let tail = &r[end + 1..];
        let port = match tail.strip_prefix(':') {
            Some(p) => Some(p),
            None if tail.is_empty() => None,
            None => return Err(bad()),
        };
        (r[..end].to_string(), port)
    } else if auth.matches(':').count() > 1 {
        (auth.to_string(), None)
    } else if let Some((h, p)) = auth.rsplit_once(':') {
        (h.to_string(), Some(p))
    } else {
        (auth.to_string(), None)
    };
    if host.is_empty() {
        return Err(bad());
    }
    let port = match port {
        None => None,
        Some(p) => match p.parse::<u32>() {
            Ok(n) if (1..=65535).contains(&n) => Some(n as u16),
            _ => return Err(bad()),
        },
    };
    let secure = matches!(scheme, Some("https" | "wss"));
    let ports = match (port, scheme) {
        (Some(p), _) => vec![p],
        (None, None) => vec![DEFAULT_PORT],
        (None, Some("https" | "wss")) => vec![443],
        (None, _) => vec![80, DEFAULT_PORT],
    };
    let path = match scheme {
        Some("ws" | "wss") if path.ends_with("/ws") || path.ends_with("/ws/") => path.to_string(),
        _ => join_ws(path),
    };
    Ok(Target {
        secure,
        host,
        ports,
        path,
    })
}

impl Target {
    /// The first port.
    pub fn port(&self) -> u16 {
        self.ports[0]
    }
    /// `host:port` for messages.
    pub fn label_at(&self, port: u16) -> String {
        core_net::show(&self.host, port)
    }
    /// The key that what the driver learns is kept under: the scheme, the host in lower case, the port and the path.
    pub fn key(&self) -> String {
        format!(
            "{}://{}{}",
            if self.secure { "wss" } else { "ws" },
            core_net::show(&self.host.to_ascii_lowercase(), self.port()),
            self.path
        )
    }
    /// The `Host` header: the port left out when it is the scheme's default.
    pub fn host_header(&self, port: u16) -> String {
        let h = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if port == if self.secure { 443 } else { 80 } {
            h
        } else {
            format!("{h}:{port}")
        }
    }
    /// A redirect target (`Location`), absolute or a path on the same server.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] for a location that is not an address.
    pub fn redirect(&self, port: u16, location: &str) -> Result<Target, DriverError> {
        let l = location.trim();
        if l.starts_with('/') {
            let mut t = self.clone();
            t.ports = vec![port];
            let p = l.split(['#', '?']).next().unwrap_or("");
            t.path = if p.ends_with("/ws") || p.ends_with("/ws/") {
                p.to_string()
            } else {
                join_ws(p)
            };
            Ok(t)
        } else {
            parse_address(l)
        }
    }
}

// ---- The connection ----

/// The largest message accepted.
pub const MAX_MESSAGE: usize = 4 << 20;
/// The largest answer header of the upgrade.
pub const MAX_HEADER: usize = ws::MAX_HEADER;
/// The requested receive buffer size.
const RECV_BUFFER: usize = 1 << 20;

/// One WebSocket connection: the shared writer, the time of the last send (for the keep-alive), and whether the server
/// ended the connection.
pub struct Conn {
    writer: Arc<Writer>,
    last_sent: Mutex<Instant>,
    /// The connection ended from the server side (its close frame, or the end of the stream).
    pub peer_closed: AtomicBool,
}

impl Conn {
    /// A connection that sends through `writer`.
    pub fn new(writer: Arc<Writer>) -> Conn {
        Conn {
            writer,
            last_sent: Mutex::new(Instant::now()),
            peer_closed: AtomicBool::new(false),
        }
    }

    /// Sends one frame. After a close frame went out, every send fails with `NotConnected`.
    pub fn send(&self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        self.writer.send(opcode, payload)?;
        *crate::lock(&self.last_sent) = Instant::now();
        Ok(())
    }

    /// Sends a text message.
    pub fn send_text(&self, text: &str) -> io::Result<()> {
        self.send(ws::op::TEXT, text.as_bytes())
    }

    /// Sends a close frame with code 1000, once.
    pub fn send_close(&self) {
        self.writer.send_close(&ws::NORMAL_CLOSURE.to_be_bytes());
    }

    /// A close frame went out (sent by the client, or the answer to the close of the server).
    pub fn close_sent(&self) -> bool {
        self.writer.is_closed()
    }

    /// How long since the client last sent a frame.
    pub fn idle(&self) -> Duration {
        crate::lock(&self.last_sent).elapsed()
    }

    /// Shuts the connection down both ways. It is safe to call twice.
    pub fn shutdown(&self) {
        self.writer.link().shutdown();
    }

    /// Registers the connection with `abort`.
    pub fn register(&self, abort: &Abort) {
        self.writer.link().register(abort);
    }

    /// Sets the read timeout (the tick of the radio thread).
    pub fn set_tick(&self, d: Duration) {
        let _ = self.writer.link().set_read_timeout(Some(d));
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// How an open failed: the error, and whether the backoff wait starts (HTTP 429 or 503).
#[derive(Debug, PartialEq)]
pub struct OpenFail {
    /// The error.
    pub error: DriverError,
    /// The server is busy: wait the backoff before the next connect.
    pub back_off: bool,
}

impl From<DriverError> for OpenFail {
    fn from(error: DriverError) -> Self {
        OpenFail {
            error,
            back_off: false,
        }
    }
}

impl From<OwrxError> for OpenFail {
    fn from(e: OwrxError) -> Self {
        DriverError::driver(e).into()
    }
}

impl From<NetError> for OpenFail {
    fn from(e: NetError) -> Self {
        DriverError::Net(e).into()
    }
}

/// What the upgrade needs beside the target.
pub struct WsOptions<'a> {
    /// The transport.
    pub transport: &'a Arc<dyn Transport>,
    /// The HTTP user (empty: no login).
    pub user: &'a str,
    /// The HTTP password.
    pub password: &'a str,
    /// Timings and test keys.
    pub timing: &'a Timing,
}

fn connect(
    target: &Target,
    port: u16,
    o: &WsOptions,
    abort: &Arc<Abort>,
) -> Result<Box<dyn core_net::Conn>, NetError> {
    let host = &target.host;
    let t = o.timing;
    let timeout = if core_net::is_local(host) {
        t.connect_local
    } else {
        t.connect_remote
    };
    let mut tries = 0;
    loop {
        tries += 1;
        match net::connect(o.transport, host, port, target.secure, timeout, abort) {
            Err(NetError::TimedOut) if tries == 1 && !abort.fired() => continue,
            r => return r,
        }
    }
}

/// Connects, upgrades, and returns the connection, the reader that holds any bytes after the answer, and the target in use
/// (after redirects and the port fallback) with its port first.
///
/// # Errors
///
/// [`OpenFail`] with the reason.
pub fn open(
    target: &Target,
    o: &WsOptions,
    abort: &Arc<Abort>,
) -> Result<(Arc<Conn>, Reader, Target), OpenFail> {
    let mut target = target.clone();
    let mut redirects = 0;
    'redirect: loop {
        let ports = target.ports.clone();
        let mut last = OpenFail::from(NetError::Refused);
        for (i, &port) in ports.iter().enumerate() {
            let more = i + 1 < ports.len();
            match upgrade_once(&target, port, o, abort) {
                Ok(Up::Done(c, r)) => {
                    let mut t = target.clone();
                    t.ports = vec![port];
                    return Ok((c, r, t));
                }
                Ok(Up::Redirect(loc)) => {
                    redirects += 1;
                    if redirects > 2 {
                        return Err(OwrxError::TooManyRedirects.into());
                    }
                    target = target.redirect(port, &loc)?;
                    continue 'redirect;
                }
                Ok(Up::TryNext(e)) if more => last = e,
                Ok(Up::TryNext(e)) => return Err(e),
                Err(e) => {
                    if more && e.error.retryable() && !e.back_off && !abort.fired() {
                        last = e;
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        return Err(last);
    }
}

enum Up {
    Done(Arc<Conn>, Reader),
    Redirect(String),
    /// A 404 on a port with another to try.
    TryNext(OpenFail),
}

/// An HTTP answer header.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpAnswer {
    /// The status code.
    pub status: u16,
    /// Header lines (name, value), the names as sent.
    pub headers: Vec<(String, String)>,
}

impl HttpAnswer {
    /// The value of the header `name`. Names compare without case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Parses an answer header (without the empty line). `None` when it is not an HTTP/1.x answer.
pub fn parse_http(head: &[u8]) -> Option<HttpAnswer> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let first = lines.next()?;
    let mut parts = first.splitn(3, ' ');
    if !parts.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let status: u16 = parts.next()?.trim().parse().ok()?;
    let headers = lines
        .filter_map(|l| {
            let (n, v) = l.split_once(':')?;
            Some((n.trim().to_string(), v.trim().to_string()))
        })
        .collect();
    Some(HttpAnswer { status, headers })
}

/// Why reading the answer header of the upgrade failed.
#[derive(Debug)]
enum HeadFail {
    /// No answer, or the connection failed: trying again can help.
    NoAnswer,
    /// Some bytes came but they are not a whole HTTP header.
    NotWeb,
    /// A stop was requested.
    Stopped,
}

/// Reads the answer of the upgrade up to, without, the empty line. It reads one byte at a time, so the frames after the
/// header stay in the connection for the shared reader.
fn read_head(link: &Link, wait: Duration, abort: &Abort) -> Result<Vec<u8>, HeadFail> {
    let mut head = Vec::with_capacity(512);
    let begun = Instant::now();
    let mut b = [0u8; 1];
    loop {
        if abort.fired() {
            return Err(HeadFail::Stopped);
        }
        match link.read_some(&mut b) {
            Ok(0) if head.is_empty() => return Err(HeadFail::NoAnswer),
            Ok(0) => return Err(HeadFail::NotWeb),
            Ok(_) => {
                head.push(b[0]);
                if !b"HTTP/".starts_with(&head[..head.len().min(5)]) {
                    return Err(HeadFail::NotWeb);
                }
                if head.ends_with(b"\r\n\r\n") {
                    head.truncate(head.len() - 4);
                    return Ok(head);
                }
                if head.len() > MAX_HEADER {
                    return Err(HeadFail::NotWeb);
                }
            }
            Err(e) if ws::is_timeout(&e) || e.kind() == io::ErrorKind::Interrupted => {
                if begun.elapsed() >= wait {
                    return Err(HeadFail::NoAnswer);
                }
            }
            Err(_) => return Err(HeadFail::NoAnswer),
        }
    }
}

fn upgrade_once(
    target: &Target,
    port: u16,
    o: &WsOptions,
    abort: &Arc<Abort>,
) -> Result<Up, OpenFail> {
    let t = o.timing;
    let mut sock = connect(target, port, o, abort)?;
    net::recv_buffer(sock.as_mut(), &[RECV_BUFFER]);
    let _ = sock.set_write_timeout(Some(Duration::from_secs(5)));
    let link = Link::new(sock, target.secure)?;
    link.set_read_timeout(Some(t.tick))?;
    link.register(abort);
    let key = ws::key_text(&t.fixed_key.unwrap_or_else(ws::random_key));
    let host_header = target.host_header(port);
    let origin = format!(
        "{}://{host_header}",
        if target.secure { "https" } else { "http" }
    );
    let mut auth = String::new();
    if !o.user.is_empty() {
        auth.push_str("Basic ");
        base64::encode_into(
            format!("{}:{}", o.user, o.password).as_bytes(),
            Alphabet::Standard,
            true,
            &mut auth,
        );
    }
    let mut extra = vec![("Origin", origin.as_str()), ("User-Agent", "sdrkit")];
    if !auth.is_empty() {
        extra.push(("Authorization", auth.as_str()));
    }
    let req = ws::upgrade_request(&target.path, &host_header, &key, &extra);
    link.write_all(req.as_bytes()).map_err(NetError::from)?;
    let no_answer = || OpenFail::from(NetError::TimedOut);
    let not_web = || {
        OpenFail::from(DriverError::Protocol(
            sdrkit_driver_core::Protocol::NotThisRadio,
        ))
    };
    let head = read_head(&link, t.upgrade, abort).map_err(|e| match e {
        HeadFail::NoAnswer => no_answer(),
        HeadFail::NotWeb => not_web(),
        HeadFail::Stopped => NetError::Aborted.into(),
    })?;
    let ans = parse_http(&head).ok_or_else(not_web)?;
    match ans.status {
        101 => {
            ws::check_answer(&head, &key).map_err(DriverError::from)?;
            let link = Arc::new(link);
            let writer = Arc::new(Writer::new(link.clone(), t.fixed_mask));
            let mut reader = Reader::new(link, writer.clone(), MAX_MESSAGE, t.stall * 2);
            // The throughput counts every byte received, the answer header too.
            reader.bytes_in = head.len() as u64 + 4;
            Ok(Up::Done(Arc::new(Conn::new(writer)), reader))
        }
        301 | 302 | 307 | 308 => match ans.header("location") {
            Some(l) => Ok(Up::Redirect(l.to_string())),
            None => Err(OwrxError::NoWebSocket { status: ans.status }.into()),
        },
        404 => Ok(Up::TryNext(OwrxError::NoWebSocket { status: 404 }.into())),
        s @ (200 | 400 | 426) => Err(OwrxError::NoWebSocket { status: s }.into()),
        401 | 407 => Err(if o.user.is_empty() {
            OwrxError::LoginNeeded
        } else {
            OwrxError::LoginRefused
        }
        .into()),
        429 | 503 => Err(OpenFail {
            error: DriverError::Busy,
            back_off: true,
        }),
        500..=599 => Err(DriverError::Busy.into()),
        s => Err(OwrxError::Http { status: s }.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(a: &str) -> Target {
        parse_address(a).unwrap()
    }

    #[test]
    fn http_answers_parse() {
        let a = parse_http(b"HTTP/1.1 302 Found\r\nLocation: http://h:5/sdr/\r\nX:  y ").unwrap();
        assert_eq!(a.status, 302);
        assert_eq!(a.header("location"), Some("http://h:5/sdr/"));
        assert_eq!(a.header("x"), Some("y"));
        assert_eq!(parse_http(b"HTTP/1.1 404 Not Found").unwrap().status, 404);
        assert!(parse_http(b"SSH-2.0-OpenSSH_9.0").is_none());
        assert!(parse_http(b"HTTP/2 101").is_none());
    }

    /// A link to a fake server on port 1 of a fake network that writes `answer` and keeps the connection.
    fn link_to(answer: &'static [u8]) -> Link {
        use std::io::Write;
        let fake = core_net::FakeTransport::new();
        fake.serve("127.0.0.1", 1, move |mut c| {
            let _ = c.write_all(answer);
            std::thread::sleep(Duration::from_secs(3));
        });
        let net: Arc<dyn Transport> = Arc::new(fake);
        let conn = net
            .connect("127.0.0.1", 1, false, Duration::from_secs(1))
            .unwrap();
        let link = Link::new(conn, false).unwrap();
        link.set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        link
    }

    /// The header and the first frame come in one write. The header reader must leave the frame for the shared reader.
    #[test]
    fn the_header_reader_leaves_the_frames() {
        let link =
            link_to(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\n\r\n\x81\x02hi");
        let head = read_head(&link, Duration::from_secs(2), &Abort::new()).unwrap();
        assert_eq!(parse_http(&head).unwrap().status, 101);
        let link = Arc::new(link);
        let w = Arc::new(Writer::new(link.clone(), Some([0; 4])));
        let mut r = Reader::new(link, w, MAX_MESSAGE, Duration::from_secs(1));
        let m = r.read(Duration::from_secs(2), &|| false).unwrap();
        assert_eq!(m, ws::Message::Text(b"hi"));
    }

    #[test]
    fn the_header_reader_refuses_a_non_http_answer() {
        let link = link_to(b"SSH-2.0-OpenSSH_9.0\r\n");
        let r = read_head(&link, Duration::from_secs(2), &Abort::new());
        assert!(matches!(r, Err(HeadFail::NotWeb)));
    }

    #[test]
    fn address_forms() {
        assert_eq!(
            t("radio.local"),
            Target {
                secure: false,
                host: "radio.local".into(),
                ports: vec![8073],
                path: "/ws/".into()
            }
        );
        assert_eq!(t(" 10.0.0.2:8080 ").ports, vec![8080]);
        assert_eq!(t("[::1]:8073").host, "::1");
        let h = t("http://rx.example.org/sdr");
        assert_eq!(
            (h.ports.clone(), h.path.as_str()),
            (vec![80, 8073], "/sdr/ws/")
        );
        assert_eq!(t("http://rx.example.org/sdr/").path, "/sdr/ws/");
        assert_eq!(t("http://rx.example.org/#freq=1?x").path, "/ws/");
        let s = t("https://rx.example.org/");
        assert!(s.secure);
        assert_eq!(s.ports, vec![443]);
        assert_eq!(t("wss://h:9000/x/ws").path, "/x/ws");
        assert_eq!(t("ws://h:9000/x").path, "/x/ws/");
        assert_eq!(t("ws://h:9000/ws/").path, "/ws/");
        assert!(parse_address("h:0").is_err());
        assert!(parse_address("http://:80/").is_err());
        assert_eq!(parse_address("h:99999").unwrap_err(), DriverError::Address);
        assert_eq!(
            t("HTTP://RX.Example.org:8073/").key(),
            "ws://rx.example.org:8073/ws/"
        );
        assert_eq!(t("127.0.0.1:8073").host_header(8073), "127.0.0.1:8073");
        assert_eq!(t("https://h/").host_header(443), "h");
        assert_eq!(t("[::1]:80").host_header(80), "[::1]");
        let r = t("127.0.0.1:1")
            .redirect(1, "http://127.0.0.1:5/sdr/")
            .unwrap();
        assert_eq!((r.ports, r.path.as_str()), (vec![5], "/sdr/ws/"));
        let r = t("127.0.0.1:1").redirect(1, "/other/").unwrap();
        assert_eq!((r.ports, r.path.as_str()), (vec![1], "/other/ws/"));
        assert!(t("https://x/").redirect(443, "https://y/").unwrap().secure);
    }
}
