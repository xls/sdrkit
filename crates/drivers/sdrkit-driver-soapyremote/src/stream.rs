//! The stream: the 24-byte datagram header, the flow-control acknowledgements, the reader of the status socket, and the receive
//! loop that writes the samples of each datagram into the ring.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

use crate::plan;
use crate::{SoapyError, Timing};
use core::net::{IpAddr, SocketAddr};
use sdrkit_core::net::{Datagram, NetError};
use sdrkit_core::Hz;
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Notices, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::DriverError;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The stream header length.
pub const HEADER_LEN: usize = 24;
/// The receive buffer of one datagram (any MTU up to 9000 fits).
pub const DATAGRAM_BUF: usize = 65_536;
/// Stream flag: end of burst.
pub const END_BURST: i32 = 2;
/// Stream flag: has time.
pub const HAS_TIME: i32 = 4;
/// Error code: overflow.
pub const OVERFLOW: i32 = -4;
/// Error code: underflow.
pub const UNDERFLOW: i32 = -7;
/// More error messages in a row than this, without data, end the read loop.
pub(crate) const MAX_ERROR_RUN: u32 = 50;

/// One stream message header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DgHeader {
    /// Total length, header included.
    pub bytes: u32,
    /// Data: the sender's counter. Acknowledgement: the next expected. Status: the channel mask.
    pub seq: u32,
    /// Data: elements per channel (negative: an error code). Acknowledgement: the window in packets.
    pub elems: i32,
    /// SoapySDR stream flags.
    pub flags: i32,
    /// Time in ns.
    pub time: i64,
}

impl DgHeader {
    /// The header in the first 24 bytes of `b`, or `None` if `b` is shorter.
    pub fn parse(b: &[u8]) -> Option<DgHeader> {
        let b: &[u8; HEADER_LEN] = b.get(..HEADER_LEN)?.try_into().ok()?;
        let w = |i: usize| [b[i], b[i + 1], b[i + 2], b[i + 3]];
        let mut t = [0u8; 8];
        t.copy_from_slice(&b[16..24]);
        Some(DgHeader {
            bytes: u32::from_be_bytes(w(0)),
            seq: u32::from_be_bytes(w(4)),
            elems: i32::from_be_bytes(w(8)),
            flags: i32::from_be_bytes(w(12)),
            time: i64::from_be_bytes(t),
        })
    }
    /// The 24 bytes.
    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut b = [0u8; HEADER_LEN];
        b[0..4].copy_from_slice(&self.bytes.to_be_bytes());
        b[4..8].copy_from_slice(&self.seq.to_be_bytes());
        b[8..12].copy_from_slice(&self.elems.to_be_bytes());
        b[12..16].copy_from_slice(&self.flags.to_be_bytes());
        b[16..24].copy_from_slice(&self.time.to_be_bytes());
        b
    }
}

/// A flow-control acknowledgement: `next` expected sequence, `window` packets.
pub fn ack(next: u32, window: u32) -> [u8; HEADER_LEN] {
    DgHeader {
        bytes: HEADER_LEN as u32,
        seq: next,
        elems: window as i32,
        flags: 0,
        time: 0,
    }
    .to_bytes()
}

/// An acknowledgement is due after this many packets (window / 8, at least 1).
pub fn ack_trigger(window: u32) -> u32 {
    (window / 8).max(1)
}

/// What a received sequence number means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqEvent {
    /// The expected one.
    InOrder,
    /// Ahead: this many datagrams were lost.
    Lost(u32),
    /// Behind: late or repeated. It is dropped.
    Late,
}

/// Classifies `seq` against the `next` expected sequence (modulo 2^32).
pub fn seq_event(next: u32, seq: u32) -> SeqEvent {
    match seq.wrapping_sub(next) {
        0 => SeqEvent::InOrder,
        d if d < 1 << 31 => SeqEvent::Lost(d),
        _ => SeqEvent::Late,
    }
}

/// The counters of one radio's streams (zeroed at each start), shared by its connections and streams.
#[derive(Debug, Default)]
pub(crate) struct StreamStats {
    /// Overflow messages from the server.
    pub overflows: AtomicU64,
    /// `O` characters in the server's log.
    pub ssi_overflows: AtomicU64,
    /// `S` characters in the server's log.
    pub ssi_sequence: AtomicU64,
    /// Datagrams lost on the network.
    pub lost_datagrams: AtomicU64,
    /// Samples those datagrams carried (estimated).
    pub lost_samples: AtomicU64,
    /// Other error messages (data and status).
    pub errors: AtomicU64,
    /// Late or repeated datagrams dropped.
    pub late: AtomicU64,
    /// Datagrams dropped because they were shorter than their `bytes`.
    pub truncated: AtomicU64,
    /// Malformed datagrams dropped.
    pub malformed: AtomicU64,
    /// Data datagrams received.
    pub datagrams: AtomicU64,
    /// Acknowledgements sent.
    pub acks: AtomicU64,
    /// Opens after the first (reconnects).
    pub reconnects: AtomicU64,
    /// Lines of the server's log dropped by the rate limit.
    pub log_dropped: AtomicU64,
}

impl StreamStats {
    /// Zeroes every counter.
    pub fn reset(&self) {
        for c in [
            &self.overflows,
            &self.ssi_overflows,
            &self.ssi_sequence,
            &self.lost_datagrams,
            &self.lost_samples,
            &self.errors,
            &self.late,
            &self.truncated,
            &self.malformed,
            &self.datagrams,
            &self.acks,
            &self.reconnects,
            &self.log_dropped,
        ] {
            c.store(0, Ordering::Relaxed);
        }
    }
    /// The status line: empty while the stream is clean.
    pub fn status(&self) -> String {
        let g = |c: &AtomicU64| c.load(Ordering::Relaxed);
        plan::status_text(
            g(&self.overflows).max(g(&self.ssi_overflows)),
            g(&self.lost_datagrams),
            g(&self.reconnects),
        )
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Throughput {
    /// Datagram bytes x 8 / seconds, headers included, in millions.
    pub mbit_s: f64,
    /// Elements per second that arrived.
    pub achieved_hz: Hz,
    /// The requested rate.
    pub asked_hz: Hz,
}

/// The throughput of a radio and the requested rate (a change starts a new window).
#[derive(Debug, Default)]
pub(crate) struct Flow {
    /// The last complete window.
    pub last: Mutex<Option<Throughput>>,
    /// The requested rate, as f64 bits.
    pub asked: AtomicU64,
}

impl Flow {
    /// Records the requested rate.
    pub fn set_asked(&self, r: Hz) {
        self.asked.store(r.to_bits(), Ordering::Relaxed);
    }
    /// The requested rate (0 before the first).
    pub fn asked(&self) -> Hz {
        f64::from_bits(self.asked.load(Ordering::Relaxed))
    }
}

/// Passes each header that `server` sent to `socket` and that waits there (status messages, or the acknowledgements a sender
/// gets) to `on_header`. It never waits. A datagram from another address, or one that is not a header, is skipped.
pub(crate) fn read_headers(
    socket: &mut dyn Datagram,
    server: IpAddr,
    mut on_header: impl FnMut(&DgHeader),
) {
    let mut b = [0u8; 64];
    while let Ok(Some((n, from))) = socket.try_recv_from(&mut b) {
        if from.ip() != server {
            continue;
        }
        if let Some(h) = DgHeader::parse(&b[..n]) {
            on_header(&h);
        }
    }
}

/// The status socket is read after this many datagrams, and whenever the data socket is quiet.
const STATUS_EVERY: u32 = 256;

/// Logs at most one line per 10 s, with the count since the last line.
#[derive(Debug, Default)]
struct Every10s {
    at: Option<Instant>,
    count: u64,
}

impl Every10s {
    /// Counts `n` and returns the count to log when a line is due.
    fn count(&mut self, n: u64) -> Option<u64> {
        self.count += n;
        if self
            .at
            .is_some_and(|t| t.elapsed() < Duration::from_secs(10))
        {
            return None;
        }
        self.at = Some(Instant::now());
        Some(std::mem::take(&mut self.count))
    }
}

/// Where a receive stream sends and reads.
pub(crate) struct RxSockets {
    /// The data socket.
    pub data: Box<dyn Datagram>,
    /// The status socket.
    pub status: Box<dyn Datagram>,
    /// The data port of the server.
    pub server: SocketAddr,
}

/// What a receive stream shares with its radio.
pub(crate) struct RxShared {
    /// The counters.
    pub stats: Arc<StreamStats>,
    /// The throughput.
    pub flow: Arc<Flow>,
    /// Where log lines go.
    pub notices: Notices,
    /// `host:port` of the server, for log lines.
    pub label: String,
}

/// The receive side of a stream that is set up, owned by the radio's thread.
pub(crate) struct RxStream {
    data: Box<dyn Datagram>,
    status: Box<dyn Datagram>,
    status_log: Every10s,
    server: SocketAddr,
    bytes: usize,
    shift: u32,
    buf: Vec<u8>,
    next: u32,
    acked: u32,
    window: u32,
    last_elems: u64,
    err_run: u32,
    pending: Option<usize>,
    timing: Timing,
    abort: Arc<Abort>,
    sh: RxShared,
    lost_log: Every10s,
    mtu_warned: bool,
    win_start: Option<Instant>,
    win_bytes: u64,
    win_elems: u64,
    win_asked: Hz,
}

impl RxStream {
    /// A stream on `sockets` for samples of `bytes` bytes each, shifted left by `shift` bits (CS16 only), with a window of
    /// `window` packets.
    ///
    /// # Errors
    ///
    /// [`DriverError`] if the data socket refuses its timeout.
    pub fn new(
        mut sockets: RxSockets,
        bytes: usize,
        shift: u32,
        window: u32,
        timing: Timing,
        abort: Arc<Abort>,
        sh: RxShared,
    ) -> Result<RxStream, DriverError> {
        sockets.data.set_read_timeout(Some(timing.slice))?;
        Ok(RxStream {
            data: sockets.data,
            status: sockets.status,
            status_log: Every10s::default(),
            server: sockets.server,
            bytes,
            shift,
            buf: vec![0u8; DATAGRAM_BUF],
            next: 0,
            acked: 0,
            window: window.max(1),
            last_elems: 0,
            err_run: 0,
            pending: None,
            timing,
            abort,
            sh,
            lost_log: Every10s::default(),
            mtu_warned: false,
            win_start: None,
            win_bytes: 0,
            win_elems: 0,
            win_asked: 0.0,
        })
    }

    /// Sends an acknowledgement for the next expected sequence.
    pub fn send_ack(&mut self) {
        let _ = self.data.send_to(&ack(self.next, self.window), self.server);
        self.acked = self.next;
        self.sh.stats.acks.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts the overflows and errors that the server reported on the status socket since the last call.
    fn read_status(&mut self) {
        let (stats, notices, label) = (&self.sh.stats, &self.sh.notices, &self.sh.label);
        let log = &mut self.status_log;
        read_headers(self.status.as_mut(), self.server.ip(), |h| {
            let code = h.elems;
            if code >= 0 {
                return;
            }
            if code == OVERFLOW {
                stats.overflows.fetch_add(1, Ordering::Relaxed);
                return;
            }
            stats.errors.fetch_add(1, Ordering::Relaxed);
            if let Some(n) = log.count(1) {
                notices.log(format!(
                    "SoapyRemote {label}: the server reports {} ({n} in the last 10 s)",
                    plan::error_name(code)
                ));
            }
        });
    }

    fn stopped(&self, ctx: Option<&StreamCtx>) -> bool {
        self.abort.fired() || ctx.is_some_and(StreamCtx::stopping)
    }

    /// Receives one datagram from the server into the buffer. `Ok(None)`: nothing within a slice.
    fn recv(&mut self) -> Result<Option<usize>, DriverError> {
        match self.data.recv_from(&mut self.buf) {
            Ok((n, from)) if from.ip() == self.server.ip() => Ok(Some(n)),
            Ok(_) => Ok(None),
            Err(e)
                if matches!(
                    e.kind(),
                    ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            // An ICMP "port unreachable" from a server that is not ready yet: keep waiting.
            Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
                std::thread::sleep(self.timing.slice);
                Ok(None)
            }
            Err(e) => Err(DriverError::Net(NetError::from(e))),
        }
    }

    /// Waits up to `timeout` for the first datagram and acknowledges again after each empty wait. The datagram is kept for the
    /// read loop.
    ///
    /// # Errors
    ///
    /// [`SoapyError::NoSamples`] if none arrives, [`NetError::Aborted`] if the stream was stopped, or the error of the socket.
    pub fn wait_first(&mut self, timeout: Duration) -> Result<(), DriverError> {
        let end = Instant::now() + timeout;
        loop {
            if self.stopped(None) {
                return Err(DriverError::Net(NetError::Aborted));
            }
            if let Some(n) = self.recv()? {
                self.pending = Some(n);
                return Ok(());
            }
            if Instant::now() >= end {
                return Err(DriverError::driver(SoapyError::NoSamples));
            }
            self.send_ack();
        }
    }

    /// Handles the datagram of `n` bytes in the buffer.
    fn handle(&mut self, n: usize, sink: &mut Sink) -> Result<(), DriverError> {
        let stats = &self.sh.stats;
        let Some(h) = DgHeader::parse(&self.buf[..n]) else {
            stats.malformed.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        };
        if h.bytes as usize > n {
            stats.truncated.fetch_add(1, Ordering::Relaxed);
            if !self.mtu_warned {
                self.mtu_warned = true;
                self.sh.notices.warn(format!(
                    "SoapyRemote {}: a datagram arrived cut short: the MTU setting may be unachievable on this network: lower it \
                     to 1500",
                    self.sh.label
                ));
            }
            return Ok(());
        }
        if (h.bytes as usize) < HEADER_LEN {
            stats.malformed.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }
        match seq_event(self.next, h.seq) {
            SeqEvent::InOrder => {}
            SeqEvent::Late => {
                stats.late.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            }
            SeqEvent::Lost(d) => {
                let d = u64::from(d);
                stats.lost_datagrams.fetch_add(d, Ordering::Relaxed);
                stats
                    .lost_samples
                    .fetch_add(d * self.last_elems, Ordering::Relaxed);
                if let Some(k) = self.lost_log.count(d) {
                    self.sh.notices.log(format!(
                        "{k} datagrams lost on the network in the last 10 s (use a wired connection, a lower rate, or a \
                         smaller IQ format)"
                    ));
                }
            }
        }
        self.next = h.seq.wrapping_add(1);
        if self.next.wrapping_sub(self.acked) >= ack_trigger(self.window) {
            self.send_ack();
        }
        self.count_bytes(u64::from(h.bytes), h.elems.max(0) as u64);
        let stats = &self.sh.stats;
        if h.elems > 0 {
            let k = h.elems as usize;
            let payload = &self.buf[HEADER_LEN..h.bytes as usize];
            let Some(payload) = payload.get(..k.saturating_mul(self.bytes)) else {
                stats.malformed.fetch_add(1, Ordering::Relaxed);
                return Ok(());
            };
            if self.shift == 0 {
                sink.write(payload);
            } else {
                let shift = self.shift;
                sink.write_with(k, |dst, from| {
                    let src = &payload[from * 4..];
                    let src = src.as_chunks::<2>().0;
                    for (d, s) in dst.as_chunks_mut::<2>().0.iter_mut().zip(src) {
                        let [a, b] = (i16::from_le_bytes(*s) << shift).to_le_bytes();
                        d[0].write(a);
                        d[1].write(b);
                    }
                });
            }
            self.last_elems = k as u64;
            self.err_run = 0;
            stats.datagrams.fetch_add(1, Ordering::Relaxed);
        } else if h.elems < 0 {
            if h.elems == OVERFLOW {
                stats.overflows.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.errors.fetch_add(1, Ordering::Relaxed);
                self.err_run += 1;
                if self.err_run > MAX_ERROR_RUN {
                    return Err(DriverError::Device { code: h.elems });
                }
            }
        }
        Ok(())
    }

    fn count_bytes(&mut self, bytes: u64, elems: u64) {
        let now = Instant::now();
        let asked = self.sh.flow.asked();
        let start = match self.win_start {
            Some(s) if asked == self.win_asked => s,
            _ => {
                (
                    self.win_start,
                    self.win_bytes,
                    self.win_elems,
                    self.win_asked,
                ) = (Some(now), 0, 0, asked);
                return;
            }
        };
        self.win_bytes += bytes;
        self.win_elems += elems;
        let secs = now.duration_since(start).as_secs_f64();
        if secs >= self.timing.throughput_window.as_secs_f64() {
            let t = Throughput {
                mbit_s: self.win_bytes as f64 * 8.0 / secs / 1e6,
                achieved_hz: self.win_elems as f64 / secs,
                asked_hz: asked,
            };
            *crate::lock(&self.sh.flow.last) = Some(t);
            (self.win_start, self.win_bytes, self.win_elems) = (Some(now), 0, 0);
        }
    }
}

impl RadioStream for RxStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        let mut last_rx = Instant::now();
        if let Some(n) = self.pending.take() {
            self.handle(n, sink)?;
        }
        let mut since_status = 0u32;
        loop {
            if self.stopped(Some(ctx)) || sink.closed() {
                return Ok(());
            }
            match self.recv() {
                Ok(Some(n)) => {
                    last_rx = Instant::now();
                    self.handle(n, sink)?;
                    since_status += 1;
                    if since_status >= STATUS_EVERY {
                        since_status = 0;
                        self.read_status();
                    }
                    continue;
                }
                Ok(None) => {}
                Err(_) if self.stopped(Some(ctx)) => return Ok(()),
                Err(e) => return Err(e),
            }
            if last_rx.elapsed() >= self.timing.stall {
                return Err(DriverError::Net(NetError::TimedOut));
            }
            since_status = 0;
            self.read_status();
            // No datagram for a slice: acknowledge again, so a sender whose acknowledgements were lost does not wait for ever.
            self.send_ack();
        }
    }
}
