//! The WebSocket client that the network drivers share: the opening handshake, masked client frames, and a reader that joins
//! fragments, answers pings and closes, and limits the message size. It runs over a connection of the host's transport,
//! plain (`ws://`) or with TLS (`wss://`).
//!
//! Standard: RFC 6455 sections 4, 5 and 7.

#[cfg(test)]
mod tests;
#[cfg(feature = "tls")]
pub mod tls;

use crate::{DriverError, Protocol};
use sdrkit_core::net::{Conn, NetError};
use std::hash::{BuildHasher, Hasher};
use std::io::{self, ErrorKind, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The GUID that the accept value adds to the key (RFC 6455 section 4.2.2).
pub const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// The longest handshake answer header that is read.
pub const MAX_HEADER: usize = 8192;
/// The default limit on one message (the sum of its fragments).
pub const MAX_MESSAGE: usize = 1 << 20;
/// The close status for a normal closure (RFC 6455 section 7.4.1).
pub const NORMAL_CLOSURE: u16 = 1000;

/// The frame opcodes (RFC 6455 section 5.2).
pub mod op {
    /// A continuation frame.
    pub const CONT: u8 = 0x0;
    /// A text frame.
    pub const TEXT: u8 = 0x1;
    /// A binary frame.
    pub const BINARY: u8 = 0x2;
    /// A close frame.
    pub const CLOSE: u8 = 0x8;
    /// A ping frame.
    pub const PING: u8 = 0x9;
    /// A pong frame.
    pub const PONG: u8 = 0xA;
}

// ---- Keys, masks and the accept value ----

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A seed for the key and mask generator: the standard library's random hash keys, a counter and the clock.
fn seed() -> u64 {
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.finish() | 1
}

/// Makes masking keys: a fixed one (for tests), or a new one for each frame from a small xorshift generator. RFC 6455 section
/// 10.3 requires that a script in a web page cannot predict the keys. These clients run no scripts, so a non-cryptographic
/// generator is enough.
#[derive(Clone, Debug)]
pub struct Masker {
    fixed: Option<[u8; 4]>,
    state: u64,
}

impl Masker {
    /// A generator. `fixed`: always use this key (tests only).
    pub fn new(fixed: Option<[u8; 4]>) -> Masker {
        Masker {
            fixed,
            state: seed(),
        }
    }

    /// The next 64 bits of the generator.
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// The masking key for the next frame.
    pub fn next_mask(&mut self) -> [u8; 4] {
        match self.fixed {
            Some(k) => k,
            None => (self.next_u64() as u32).to_le_bytes(),
        }
    }
}

/// 16 random bytes for `Sec-WebSocket-Key` (RFC 6455 section 4.1, item 7).
pub fn random_key() -> [u8; 16] {
    let mut m = Masker::new(None);
    let mut k = [0u8; 16];
    k[..8].copy_from_slice(&m.next_u64().to_le_bytes());
    k[8..].copy_from_slice(&m.next_u64().to_le_bytes());
    k
}

/// The text form of a key: base64 of its 16 bytes, 24 characters.
pub fn key_text(key: &[u8; 16]) -> String {
    let mut s = String::with_capacity(24);
    sdrkit_core::base64::encode_into(key, sdrkit_core::base64::Alphabet::Standard, true, &mut s);
    s
}

/// The `Sec-WebSocket-Accept` value that a server must send for the key `key` (its text form). RFC 6455 section 4.2.2 item 5.4
/// defines it as base64 of the SHA-1 of the key and [`GUID`].
///
/// ```
/// // The example of RFC 6455 section 1.3.
/// assert_eq!(sdrkit_driver_core::ws::accept_for("dGhlIHNhbXBsZSBub25jZQ=="), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
/// ```
pub fn accept_for(key: &str) -> String {
    let mut v = Vec::with_capacity(key.len() + GUID.len());
    v.extend_from_slice(key.as_bytes());
    v.extend_from_slice(GUID.as_bytes());
    let mut s = String::with_capacity(28);
    sdrkit_core::base64::encode_into(
        &sdrkit_algo_crypto::sha1(&v),
        sdrkit_core::base64::Alphabet::Standard,
        true,
        &mut s,
    );
    s
}

// ---- The opening handshake ----

/// The upgrade request (RFC 6455 section 4.1): `GET path`, then `Host`, `Upgrade`, `Connection`, the key and the version, then
/// `extra` headers in order, each line ended by CR LF, and an empty line.
pub fn upgrade_request(path: &str, host: &str, key: &str, extra: &[(&str, &str)]) -> String {
    let mut r = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n"
    );
    for (name, value) in extra {
        r.push_str(name);
        r.push_str(": ");
        r.push_str(value);
        r.push_str("\r\n");
    }
    r.push_str("\r\n");
    r
}

/// Why the opening handshake failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UpgradeError {
    /// No byte came before the connection ended or the wait passed. Trying again can help.
    #[error("no answer to the WebSocket handshake")]
    NoAnswer,
    /// The connection ended, or went quiet, inside the answer header. Trying again can help.
    #[error("the connection dropped during the WebSocket handshake")]
    Dropped,
    /// The answer is not an HTTP header, or it has no empty line in [`MAX_HEADER`] bytes.
    #[error("not a WebSocket server: the answer is not an HTTP header")]
    NotHttp,
    /// The status is not 101.
    #[error("the WebSocket handshake was refused with status {status:?}")]
    Refused {
        /// The HTTP status, if the first line has one.
        status: Option<u16>,
    },
    /// The answer has no `Sec-WebSocket-Accept` header.
    #[error("the WebSocket handshake answer has no Sec-WebSocket-Accept")]
    NoAccept,
    /// The `Sec-WebSocket-Accept` value is not the one for the key sent.
    #[error("the WebSocket handshake answer does not match the key")]
    Mismatch,
    /// The answer names an extension or a subprotocol. The client requests none, so it must refuse them.
    ///
    /// Standard: RFC 6455 section 4.1, client steps 5 and 6.
    #[error(
        "the WebSocket handshake answer names an extension or subprotocol that was not asked for"
    )]
    Unasked,
    /// A stop was requested.
    #[error("the connection was stopped")]
    Stopped,
}

impl UpgradeError {
    /// Trying again later can help (the server did not answer yet, or the stop came first).
    pub fn retry(&self) -> bool {
        matches!(
            self,
            UpgradeError::NoAnswer | UpgradeError::Dropped | UpgradeError::Stopped
        )
    }
}

/// Reads an answer header (up to, without, the empty line) against the key sent (its text form). The status must be 101 and
/// the accept value must match. An extension or a subprotocol in the answer is refused.
pub fn check_answer(header: &[u8], key: &str) -> Result<(), UpgradeError> {
    let text = String::from_utf8_lossy(header);
    let first = text.split("\r\n").next().unwrap_or("");
    if !first.starts_with("HTTP/") {
        return Err(UpgradeError::NotHttp);
    }
    let status = first.split(' ').nth(1);
    if status != Some("101") {
        return Err(UpgradeError::Refused {
            status: status.and_then(|s| s.parse().ok()),
        });
    }
    let want = accept_for(key);
    let mut accept: Option<bool> = None;
    let mut unasked = false;
    for line in text.split("\r\n").skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim_matches([' ', '\t']));
        if name.eq_ignore_ascii_case("sec-websocket-accept") {
            accept.get_or_insert(value == want);
        } else if (name.eq_ignore_ascii_case("sec-websocket-extensions")
            || name.eq_ignore_ascii_case("sec-websocket-protocol"))
            && !value.is_empty()
        {
            unasked = true;
        }
    }
    match accept {
        None => Err(UpgradeError::NoAccept),
        Some(false) => Err(UpgradeError::Mismatch),
        Some(true) if unasked => Err(UpgradeError::Unasked),
        Some(true) => Ok(()),
    }
}

// ---- Client frames ----

/// Appends one client frame to `out`: FIN set, the payload masked with `mask` (RFC 6455 section 5.2 and 5.3). A client frame
/// is always masked.
pub fn encode_frame(out: &mut Vec<u8>, opcode: u8, payload: &[u8], mask: [u8; 4]) {
    out.push(0x80 | (opcode & 0x0F));
    let n = payload.len();
    if n < 126 {
        out.push(0x80 | n as u8);
    } else if n <= 0xFFFF {
        out.push(0x80 | 126);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else {
        out.push(0x80 | 127);
        out.extend_from_slice(&(n as u64).to_be_bytes());
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i & 3]));
}

/// The error is a receive timeout (the read timeout of the connection passed).
pub fn is_timeout(e: &io::Error) -> bool {
    sdrkit_core::net::timed_out(e)
}

// ---- The connection ----

fn lock<T: ?Sized>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// One connection that a [`Reader`] and a [`Writer`] share. Reads and writes have their own handles of the connection, so on
/// a plain connection a write never waits for a read.
pub struct Link {
    rx: Mutex<Box<dyn Conn>>,
    tx: Mutex<Box<dyn Conn>>,
    ctl: Mutex<Box<dyn Conn>>,
    secure: bool,
}

impl std::fmt::Debug for Link {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Link").field("tls", &self.secure).finish()
    }
}

impl Link {
    /// A link on `conn`. `secure` says that the transport opened it with TLS (`wss://`). Over TLS the handles share one
    /// session, so reads and writes take turns.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the connection cannot give a second handle.
    pub fn new(conn: Box<dyn Conn>, secure: bool) -> Result<Link, NetError> {
        Ok(Link {
            rx: Mutex::new(conn.try_clone()?),
            tx: Mutex::new(conn.try_clone()?),
            ctl: Mutex::new(conn),
            secure,
        })
    }

    /// The connection uses TLS.
    pub fn secure(&self) -> bool {
        self.secure
    }

    /// Sets how long a read can wait. Keep it short (100 ms): the reader looks at a stop between reads.
    ///
    /// # Errors
    ///
    /// [`NetError`] if the system refuses the timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), NetError> {
        lock(&self.ctl).set_read_timeout(timeout)
    }

    /// Registers the connection with `abort`, which shuts it down when it fires.
    pub fn register(&self, abort: &crate::net::Abort) {
        abort.register(lock(&self.ctl).as_ref());
    }

    /// Reads what is there, up to `buf.len()` bytes. 0 is the end of the stream. A receive timeout is an error that
    /// [`is_timeout`] accepts.
    pub fn read_some(&self, buf: &mut [u8]) -> io::Result<usize> {
        lock(&self.rx).read(buf)
    }

    /// Writes all of `bytes`.
    pub fn write_all(&self, bytes: &[u8]) -> io::Result<()> {
        let mut tx = lock(&self.tx);
        tx.write_all(bytes).and_then(|()| tx.flush())
    }

    /// Shuts the connection down both ways. This wakes a read that waits.
    pub fn shutdown(&self) {
        lock(&self.ctl).shutdown();
    }
}

/// The sending side of a connection. Every frame goes out whole under one lock, so threads can share it. Each frame gets a new
/// masking key.
pub struct Writer {
    link: Arc<Link>,
    mask: Mutex<(Masker, Vec<u8>)>,
    closed: AtomicBool,
    /// Frame bytes written.
    pub bytes_out: AtomicU64,
}

impl std::fmt::Debug for Writer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writer")
            .field("link", &self.link)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl Writer {
    /// A writer on `link`. `fixed_mask`: always use this masking key (tests only).
    pub fn new(link: Arc<Link>, fixed_mask: Option<[u8; 4]>) -> Writer {
        Writer {
            link,
            mask: Mutex::new((Masker::new(fixed_mask), Vec::with_capacity(512))),
            closed: AtomicBool::new(false),
            bytes_out: AtomicU64::new(0),
        }
    }

    /// Sends one frame. After a close frame went out, every send fails with `NotConnected`.
    pub fn send(&self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        if self.is_closed() {
            return Err(ErrorKind::NotConnected.into());
        }
        self.send_frame(opcode, payload)
    }

    fn send_frame(&self, opcode: u8, payload: &[u8]) -> io::Result<()> {
        // The frame buffer is cleared before use, so a guard left by a thread that panicked holds nothing to keep.
        let mut g = self
            .mask
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (masker, frame) = &mut *g;
        frame.clear();
        let mask = masker.next_mask();
        encode_frame(frame, opcode, payload, mask);
        self.link.write_all(frame)?;
        self.bytes_out
            .fetch_add(frame.len() as u64, Ordering::Relaxed);
        Ok(())
    }

    /// Sends a text message.
    pub fn text(&self, t: &str) -> io::Result<()> {
        self.send(op::TEXT, t.as_bytes())
    }

    /// Sends a binary message.
    pub fn binary(&self, b: &[u8]) -> io::Result<()> {
        self.send(op::BINARY, b)
    }

    /// Sends a close frame with `payload` (a status code and a reason, or nothing), once. Later calls and sends do nothing and
    /// fail. RFC 6455 section 5.5.1 allows no data frame after a close frame.
    pub fn send_close(&self, payload: &[u8]) {
        if !self.closed.swap(true, Ordering::AcqRel) {
            let _ = self.send_frame(op::CLOSE, payload);
        }
    }

    /// Sends a close frame with [`NORMAL_CLOSURE`] (if none went out yet), then shuts the socket down both ways. It is safe to
    /// call twice.
    pub fn close(&self) {
        self.send_close(&NORMAL_CLOSURE.to_be_bytes());
        self.link.shutdown();
    }

    /// A close frame went out.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// The connection underneath.
    pub fn link(&self) -> &Arc<Link> {
        &self.link
    }
}

// ---- Server frames ----

/// One whole message from the server. It borrows the reader until the next read.
#[derive(Debug, PartialEq, Eq)]
pub enum Message<'a> {
    /// A text message. The reader does not look at its UTF-8 (RFC 6455 section 8.1 leaves that to the receiver).
    Text(&'a [u8]),
    /// A binary message.
    Binary(&'a [u8]),
}

impl<'a> Message<'a> {
    /// The payload.
    pub fn bytes(&self) -> &'a [u8] {
        match *self {
            Message::Text(b) | Message::Binary(b) => b,
        }
    }
}

/// How a server frame broke the rules of RFC 6455 section 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameFault {
    /// A server frame is masked.
    Masked,
    /// A reserved bit is set.
    Reserved,
    /// The opcode is not one of RFC 6455.
    UnknownOpcode,
    /// A control frame is fragmented.
    FragmentedControl,
    /// A control frame is over 125 bytes.
    ControlTooLong,
    /// A continuation frame comes with nothing to continue.
    NothingToContinue,
    /// A new message starts inside a fragmented one.
    NewInsideFragmented,
}

/// Why a read returned no message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// [`Reader::read`] only: no frame began within the wait.
    #[error("no message within the wait")]
    Timeout,
    /// A frame began but no byte of it came within [`Reader::frame_wait`].
    #[error("a frame cut short: no more of it came")]
    Stalled,
    /// The connection ended: the end of the stream, a socket error, or the server's close frame (which was answered).
    #[error("the connection closed")]
    Closed,
    /// A frame broke RFC 6455.
    #[error("a bad WebSocket frame: {0:?}")]
    Bad(FrameFault),
    /// A message is longer than the limit of the reader.
    #[error("a WebSocket message over the size limit")]
    TooBig,
    /// [`Reader::read`] only: a stop was requested.
    #[error("the connection was stopped")]
    Stopped,
}

impl From<UpgradeError> for DriverError {
    fn from(e: UpgradeError) -> Self {
        match e {
            UpgradeError::NoAnswer => DriverError::Net(NetError::TimedOut),
            UpgradeError::Dropped => DriverError::Net(NetError::Closed),
            UpgradeError::Stopped => DriverError::Net(NetError::Aborted),
            e @ (UpgradeError::NotHttp
            | UpgradeError::Refused { .. }
            | UpgradeError::NoAccept
            | UpgradeError::Mismatch
            | UpgradeError::Unasked) => DriverError::Handshake(e),
        }
    }
}

impl From<ReadError> for DriverError {
    fn from(e: ReadError) -> Self {
        match e {
            ReadError::Timeout | ReadError::Stalled => DriverError::Net(NetError::TimedOut),
            ReadError::Closed => DriverError::Net(NetError::Closed),
            ReadError::Stopped => DriverError::Net(NetError::Aborted),
            ReadError::Bad(_) => DriverError::Protocol(Protocol::Malformed),
            ReadError::TooBig => DriverError::Protocol(Protocol::TooLong),
        }
    }
}

/// Why reading the answer header did not finish.
#[derive(Clone, Debug, PartialEq, Eq)]
enum HeadError {
    /// A read timed out with the header not complete yet.
    Pending,
    /// The stream ended. `true` when some bytes came.
    Eof(bool),
    /// Not an HTTP answer, or no empty line in [`MAX_HEADER`] bytes.
    NotHttp,
}

/// A frame found in the buffer: its opcode, and where its payload is.
#[derive(Clone, Copy)]
enum Found {
    /// The payload is in the read buffer at `at`.
    Direct { op: u8, at: usize, len: usize },
    /// The payload is the joined fragments.
    Joined { op: u8 },
}

/// Reads server frames into one buffer and returns whole messages. An unfragmented message is a slice of that buffer, so it
/// is not copied. Fragments are joined in a second buffer. Both buffers grow to the largest message seen (at most the limit)
/// and are kept, so reading does not allocate once they have grown.
///
/// The reader waits in ticks of the socket's read timeout, so the caller can look at a stop between ticks.
pub struct Reader {
    link: Arc<Link>,
    writer: Arc<Writer>,
    buf: Vec<u8>,
    start: usize,
    end: usize,
    consume: usize,
    joined: Vec<u8>,
    joining: Option<u8>,
    max: usize,
    last_byte: Instant,
    /// How long a frame that has begun can go without a byte before the read fails with [`ReadError::Stalled`].
    pub frame_wait: Duration,
    /// Every byte read, the handshake answer included.
    pub bytes_in: u64,
    /// Payload bytes of the frames read (data and control frames).
    pub payload_in: u64,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("link", &self.link)
            .field("buffered", &(self.end - self.start))
            .field("max", &self.max)
            .finish()
    }
}

impl Reader {
    /// A reader on `link` with the message limit `max_message`. It answers control frames through `writer`. A frame that has
    /// begun can wait `frame_wait` between bytes.
    pub fn new(
        link: Arc<Link>,
        writer: Arc<Writer>,
        max_message: usize,
        frame_wait: Duration,
    ) -> Reader {
        Reader {
            link,
            writer,
            buf: vec![0u8; 64 << 10],
            start: 0,
            end: 0,
            consume: 0,
            joined: Vec::new(),
            joining: None,
            max: max_message,
            last_byte: Instant::now(),
            frame_wait,
            bytes_in: 0,
            payload_in: 0,
        }
    }

    /// The connection (to set its read timeout, or to register it with an abort handle).
    pub fn link(&self) -> &Arc<Link> {
        &self.link
    }

    /// The writer that answers control frames.
    pub fn writer(&self) -> &Arc<Writer> {
        &self.writer
    }

    /// A frame or a fragmented message has begun and is not complete.
    pub fn partial(&self) -> bool {
        self.end > self.start + self.consume || self.joining.is_some()
    }

    /// Reads from the link into the buffer, keeping room for `need` bytes from `start`. `Ok(false)`: the read timed out.
    fn fill(&mut self, need: usize) -> Result<bool, ReadError> {
        if self.start + need > self.buf.len() {
            self.buf.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }
        if need > self.buf.len() {
            let n = need.next_power_of_two().max(self.buf.len());
            self.buf.resize(n, 0);
        }
        match self.link.read_some(&mut self.buf[self.end..]) {
            Ok(0) => Err(ReadError::Closed),
            Ok(n) => {
                self.end += n;
                self.bytes_in += n as u64;
                self.last_byte = Instant::now();
                Ok(true)
            }
            Err(e) if is_timeout(&e) || e.kind() == ErrorKind::Interrupted => Ok(false),
            Err(_) => Err(ReadError::Closed),
        }
    }

    /// Reads the answer header up to the empty line. The bytes after it stay in the buffer for the frames.
    fn read_head(&mut self) -> Result<Vec<u8>, HeadError> {
        loop {
            let have = &self.buf[self.start..self.end];
            if !b"HTTP/".starts_with(&have[..have.len().min(5)]) {
                return Err(HeadError::NotHttp);
            }
            if let Some(p) = have.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = have[..p].to_vec();
                self.start += p + 4;
                return Ok(head);
            }
            if have.len() > MAX_HEADER {
                return Err(HeadError::NotHttp);
            }
            let some = !have.is_empty();
            match self.fill(MAX_HEADER + 4) {
                Ok(true) => {}
                Ok(false) => return Err(HeadError::Pending),
                Err(_) => return Err(HeadError::Eof(some)),
            }
        }
    }

    /// Bytes needed from `start` to complete the frame there (the size limit was already applied).
    fn need(&self) -> usize {
        let b = &self.buf[self.start..self.end];
        if b.len() < 2 {
            return 14;
        }
        let (hl, len) = match b[1] & 0x7F {
            126 if b.len() >= 4 => (4, u16::from_be_bytes([b[2], b[3]]) as usize),
            127 if b.len() >= 10 => {
                let mut l = [0u8; 8];
                l.copy_from_slice(&b[2..10]);
                (10, u64::from_be_bytes(l).min(self.max as u64) as usize)
            }
            126 | 127 => return 14,
            n => (2, n as usize),
        };
        hl + len
    }

    /// Parses the frames that are complete at `start`. Data fragments are joined and used up here. Control frames are
    /// answered here.
    fn parse(&mut self) -> Result<Option<Found>, ReadError> {
        loop {
            let b = &self.buf[self.start..self.end];
            if b.len() < 2 {
                return Ok(None);
            }
            let (b0, b1) = (b[0], b[1]);
            if b1 & 0x80 != 0 {
                return Err(ReadError::Bad(FrameFault::Masked));
            }
            if b0 & 0x70 != 0 {
                return Err(ReadError::Bad(FrameFault::Reserved));
            }
            let fin = b0 & 0x80 != 0;
            let opc = b0 & 0x0F;
            let (hl, len) = match b1 & 0x7F {
                126 if b.len() < 4 => return Ok(None),
                126 => (4, u16::from_be_bytes([b[2], b[3]]) as u64),
                127 if b.len() < 10 => return Ok(None),
                127 => {
                    let mut l = [0u8; 8];
                    l.copy_from_slice(&b[2..10]);
                    (10, u64::from_be_bytes(l))
                }
                n => (2, n as u64),
            };
            let control = opc >= 8;
            if control {
                if !matches!(opc, op::CLOSE | op::PING | op::PONG) {
                    return Err(ReadError::Bad(FrameFault::UnknownOpcode));
                }
                if !fin {
                    return Err(ReadError::Bad(FrameFault::FragmentedControl));
                }
                if len > 125 {
                    return Err(ReadError::Bad(FrameFault::ControlTooLong));
                }
            } else {
                match (opc, self.joining) {
                    (op::CONT, None) => return Err(ReadError::Bad(FrameFault::NothingToContinue)),
                    (op::TEXT | op::BINARY, Some(_)) => {
                        return Err(ReadError::Bad(FrameFault::NewInsideFragmented))
                    }
                    (op::CONT | op::TEXT | op::BINARY, _) => {}
                    _ => return Err(ReadError::Bad(FrameFault::UnknownOpcode)),
                }
                let so_far = if opc == op::CONT {
                    self.joined.len() as u64
                } else {
                    0
                };
                if len + so_far > self.max as u64 {
                    return Err(ReadError::TooBig);
                }
            }
            let len = len as usize;
            if b.len() < hl + len {
                return Ok(None);
            }
            let at = self.start + hl;
            self.payload_in += len as u64;
            if control {
                self.start += hl + len;
                let mut p = [0u8; 125];
                p[..len].copy_from_slice(&self.buf[at..at + len]);
                match opc {
                    op::PING => {
                        let _ = self.writer.send(op::PONG, &p[..len]);
                    }
                    op::CLOSE => {
                        // Echo the status code (RFC 6455 section 5.5.1), then the connection is over.
                        self.writer.send_close(&p[..len.min(2)]);
                        return Err(ReadError::Closed);
                    }
                    _ => {}
                }
                continue;
            }
            match self.joining {
                None if fin => {
                    self.consume = hl + len;
                    return Ok(Some(Found::Direct { op: opc, at, len }));
                }
                None => {
                    self.joined.clear();
                    self.joined.extend_from_slice(&self.buf[at..at + len]);
                    self.joining = Some(opc);
                    self.start += hl + len;
                }
                Some(first) => {
                    self.joined.extend_from_slice(&self.buf[at..at + len]);
                    self.start += hl + len;
                    if fin {
                        self.joining = None;
                        return Ok(Some(Found::Joined { op: first }));
                    }
                }
            }
        }
    }

    /// One tick: a whole message, or `None` when a read timed out first.
    fn poll(&mut self) -> Result<Option<Found>, ReadError> {
        self.start += std::mem::take(&mut self.consume);
        loop {
            if let Some(f) = self.parse()? {
                return Ok(Some(f));
            }
            let need = self.need();
            if !self.fill(need)? {
                if self.partial() && self.last_byte.elapsed() >= self.frame_wait {
                    return Err(ReadError::Stalled);
                }
                return Ok(None);
            }
        }
    }

    fn message(&self, f: Found) -> Message<'_> {
        let (op, p) = match f {
            Found::Direct { op, at, len } => (op, &self.buf[at..at + len]),
            Found::Joined { op } => (op, &self.joined[..]),
        };
        if op == op::TEXT {
            Message::Text(p)
        } else {
            Message::Binary(p)
        }
    }

    /// The next whole message, or `Ok(None)` when one read tick (the socket's read timeout) passed without one. Control frames
    /// are handled on the way: a ping is answered with a pong, a pong is dropped, and a close is answered and ends the
    /// connection with [`ReadError::Closed`].
    pub fn poll_message(&mut self) -> Result<Option<Message<'_>>, ReadError> {
        Ok(self.poll()?.map(|f| self.message(f)))
    }

    /// The next whole message, waiting tick by tick. It fails with [`ReadError::Timeout`] when no byte came for `wait` and no
    /// frame has begun, and with [`ReadError::Stopped`] when `stop` says so between ticks.
    pub fn read(
        &mut self,
        wait: Duration,
        stop: &dyn Fn() -> bool,
    ) -> Result<Message<'_>, ReadError> {
        let begun = Instant::now();
        loop {
            if stop() {
                return Err(ReadError::Stopped);
            }
            if let Some(f) = self.poll()? {
                return Ok(self.message(f));
            }
            let quiet = begun.max(self.last_byte).elapsed();
            if !self.partial() && quiet >= wait {
                return Err(ReadError::Timeout);
            }
        }
    }
}

// ---- The whole opening ----

/// What the opening handshake sends and how long it waits.
#[derive(Clone, Debug)]
pub struct Upgrade<'a> {
    /// The request path ("/").
    pub path: &'a str,
    /// The `Host` header: `host:port`, an IPv6 host in brackets.
    pub host: &'a str,
    /// More request headers, in order.
    pub extra: &'a [(&'a str, &'a str)],
    /// A fixed key (tests only). `None` makes a random one.
    pub key: Option<[u8; 16]>,
    /// A fixed masking key (tests only).
    pub mask: Option<[u8; 4]>,
    /// How long the answer can go without a byte. Reads wait in ticks of the socket's read timeout.
    pub wait: Duration,
    /// The message limit of the reader.
    pub max_message: usize,
    /// How long a begun frame can wait between bytes ([`Reader::frame_wait`]).
    pub frame_wait: Duration,
}

impl Default for Upgrade<'_> {
    fn default() -> Self {
        Upgrade {
            path: "/",
            host: "",
            extra: &[],
            key: None,
            mask: None,
            wait: Duration::from_secs(3),
            max_message: MAX_MESSAGE,
            frame_wait: Duration::from_secs(3),
        }
    }
}

/// Runs the opening handshake on `link` (RFC 6455 section 4.1): sends the upgrade request, reads the answer and makes sure that it is right.
/// `stop` is checked between read ticks. Returns the reader, which keeps any frame bytes that came after the answer, and the
/// writer.
pub fn handshake(
    link: Link,
    up: &Upgrade<'_>,
    stop: &dyn Fn() -> bool,
) -> Result<(Reader, Arc<Writer>), UpgradeError> {
    let link = Arc::new(link);
    let key = key_text(&up.key.unwrap_or_else(random_key));
    let req = upgrade_request(up.path, up.host, &key, up.extra);
    if link.write_all(req.as_bytes()).is_err() {
        return Err(UpgradeError::NoAnswer);
    }
    let writer = Arc::new(Writer::new(link.clone(), up.mask));
    let mut reader = Reader::new(link, writer.clone(), up.max_message, up.frame_wait);
    let begun = Instant::now();
    let head = loop {
        if stop() {
            return Err(UpgradeError::Stopped);
        }
        let some = reader.end > reader.start;
        match reader.read_head() {
            Ok(h) => break h,
            Err(HeadError::Pending) => {
                if begun.max(reader.last_byte).elapsed() >= up.wait {
                    return Err(if some {
                        UpgradeError::Dropped
                    } else {
                        UpgradeError::NoAnswer
                    });
                }
            }
            Err(HeadError::Eof(false)) => return Err(UpgradeError::NoAnswer),
            Err(HeadError::Eof(true)) => return Err(UpgradeError::Dropped),
            Err(HeadError::NotHttp) => return Err(UpgradeError::NotHttp),
        }
    };
    check_answer(&head, &key)?;
    Ok((reader, writer))
}
