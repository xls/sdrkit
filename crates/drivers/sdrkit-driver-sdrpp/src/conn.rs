//! One connection to an SDR++ server: one reader for every packet, the answer counts, the flush and discard rules, the throughput
//! windows.
//!
//! While the stream runs, its thread is the only reader, and a caller waiting for an answer watches the counts. When no stream
//! runs, the waiter reads the connection itself, and samples are dropped.

use crate::proto::{self, cmd, pkt, Baseband, Header, SampleType, HEADER_LEN, MAX_PACKET};
use crate::{lock, menu::Menu, Mem, PREFIX};
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};
use sdrkit_core::net::{Conn as NetConn, NetError};
use sdrkit_driver_core::net::Abort;
use sdrkit_driver_core::runner::{Notices, Sink};
use sdrkit_driver_core::{DriverError, Protocol};
use std::io::{ErrorKind, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How long one read waits before the reader looks around (a stop, a stall, a waiter's deadline).
pub(crate) const READ_STEP: Duration = Duration::from_millis(20);
/// The throughput window.
pub(crate) const WINDOW: Duration = Duration::from_secs(2);
/// Below this share of the rate, a window's shortfall is counted as radio-side loss.
const SHORT_SHARE: f64 = 0.9;
/// Sample packets dropped after a flush's answer (the server's own pipeline).
const FLUSH_EXTRA: u32 = 2;

/// Why a connection failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fail {
    /// A DISCONNECT came: another client has the server.
    Busy,
    /// An invalid header after a good packet.
    Garbled,
    /// An invalid header as the first packet.
    NotSdrpp,
    /// The connection closed or a send failed.
    Closed,
    /// Nothing answered in time.
    NoAnswer,
}

/// Radio-side loss found by the throughput windows, kept for the life of an open source (across reconnects).
#[derive(Debug, Default)]
pub(crate) struct Shortfall {
    pub samples: AtomicU64,
    pub events: AtomicU64,
    pub last: Mutex<Option<Instant>>,
}

/// One throughput window's result, published per server.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Throughput {
    /// Received, megabits per second (sample packets' payload as received).
    pub mbit_s: f64,
    /// Samples received per second.
    pub samples_per_s: f64,
    /// The server's rate.
    pub rate_hz: f64,
    /// The sample type in use.
    pub sample_type: SampleType,
    /// The samples this window lacked, counted as radio-side loss (0 when the window does not count).
    pub shortfall: u64,
}

#[derive(Debug)]
struct Window {
    start: Instant,
    samples: u64,
    bytes: u64,
    purposeful: bool,
}

impl Window {
    fn new() -> Self {
        Window {
            start: Instant::now(),
            samples: 0,
            bytes: 0,
            purposeful: false,
        }
    }
}

/// The reader's state, shared with the waiters.
#[derive(Debug)]
pub(crate) struct State {
    /// Answers counted per client command.
    pub acks: [u64; cmd::CLIENT_COUNT],
    /// Answered commands sent, per client command.
    pub sent: [u64; cmd::CLIENT_COUNT],
    /// The server's rate, once announced.
    pub rate: Option<f64>,
    /// How many SET_SAMPLERATE came.
    pub rate_seq: u64,
    /// The menu as last read.
    pub menu: Option<Menu>,
    pub fail: Option<Fail>,
    /// A DISCONNECT came.
    pub busy: bool,
    /// Good packets so far.
    pub good: u64,
    pub last_packet: Instant,
    /// Every sample packet is dropped (the stream is held stopped).
    pub discard: bool,
    /// Samples are expected (a stall counts).
    pub expect: bool,
    /// The driver is changing the server's state: a rate notice is its own.
    pub configuring: bool,
    flush: Option<(usize, u64)>,
    flush_extra: u32,
    window: Window,
    said_short: bool,
    said_menu: bool,
    said_zstd: bool,
    pub sample_type: SampleType,
}

struct Reader {
    sock: Box<dyn NetConn>,
    hdr: [u8; HEADER_LEN],
    hdr_have: usize,
    header: Option<Header>,
    body: Vec<u8>,
    body_have: usize,
    zdec: FrameDecoder,
    zout: Vec<u8>,
}

enum ReadErr {
    Invalid,
    Closed,
}

impl Reader {
    /// Reads until a whole packet is in, the read step passes (None), or the connection fails. Partial progress is kept.
    fn read_one(&mut self) -> Result<Option<Header>, ReadErr> {
        loop {
            let (buf, want) = match self.header {
                None => {
                    if self.hdr_have == HEADER_LEN {
                        let h = Header::parse(&self.hdr);
                        if !h.valid() {
                            return Err(ReadErr::Invalid);
                        }
                        let n = h.body_len();
                        if self.body.len() < n {
                            self.body.resize(n, 0);
                        }
                        self.header = Some(h);
                        self.body_have = 0;
                        continue;
                    }
                    (&mut self.hdr[self.hdr_have..], HEADER_LEN - self.hdr_have)
                }
                Some(h) => {
                    let n = h.body_len();
                    if self.body_have == n {
                        self.header = None;
                        self.hdr_have = 0;
                        return Ok(Some(h));
                    }
                    (&mut self.body[self.body_have..n], n - self.body_have)
                }
            };
            debug_assert!(want > 0);
            match self.sock.read(buf) {
                Ok(0) => return Err(ReadErr::Closed),
                Ok(k) => {
                    if self.header.is_none() {
                        self.hdr_have += k;
                    } else {
                        self.body_have += k;
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    return Ok(None)
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Err(ReadErr::Closed),
            }
        }
    }
}

/// Inflates one zstd frame into `out`, at most 32 MiB.
fn inflate(dec: &mut FrameDecoder, src: &[u8], out: &mut Vec<u8>) -> Result<(), ()> {
    out.clear();
    let mut input = src;
    dec.reset(&mut input).map_err(|_| ())?;
    loop {
        if !dec.is_finished() {
            dec.decode_blocks(&mut input, BlockDecodingStrategy::UptoBytes(1 << 20))
                .map_err(|_| ())?;
        }
        let can = dec.can_collect();
        if out.len() + can > MAX_PACKET as usize {
            return Err(());
        }
        if can > 0 {
            let at = out.len();
            out.resize(at + can, 0);
            let n = dec.read(&mut out[at..]).map_err(|_| ())?;
            out.truncate(at + n);
        }
        if dec.is_finished() && dec.can_collect() == 0 {
            break;
        }
    }
    if let (Some(want), Some(got)) = (dec.get_checksum_from_data(), dec.get_calculated_checksum()) {
        if want != got {
            return Err(());
        }
    }
    Ok(())
}

/// One connection.
pub(crate) struct Conn {
    pub label: String,
    ctl: Mutex<Box<dyn NetConn>>,
    write: Mutex<Box<dyn NetConn>>,
    reader: Mutex<Reader>,
    st: Mutex<State>,
    cv: Condvar,
    stream_active: AtomicBool,
    pub mem: Mem,
    pub notices: Notices,
    shortfall: Arc<Shortfall>,
    closed: AtomicBool,
}

impl Conn {
    pub fn new(
        mut sock: Box<dyn NetConn>,
        label: &str,
        mem: Mem,
        notices: Notices,
        shortfall: Arc<Shortfall>,
    ) -> Result<Arc<Conn>, NetError> {
        let mut rsock = sock.try_clone()?;
        rsock.set_read_timeout(Some(READ_STEP))?;
        sock.set_write_timeout(Some(Duration::from_secs(5)))?;
        let ctl = sock.try_clone()?;
        Ok(Arc::new(Conn {
            label: label.into(),
            ctl: Mutex::new(ctl),
            write: Mutex::new(sock),
            reader: Mutex::new(Reader {
                sock: rsock,
                hdr: [0; HEADER_LEN],
                hdr_have: 0,
                header: None,
                body: Vec::new(),
                body_have: 0,
                zdec: FrameDecoder::new(),
                zout: Vec::new(),
            }),
            st: Mutex::new(State {
                acks: [0; cmd::CLIENT_COUNT],
                sent: [0; cmd::CLIENT_COUNT],
                rate: None,
                rate_seq: 0,
                menu: None,
                fail: None,
                busy: false,
                good: 0,
                last_packet: Instant::now(),
                discard: false,
                expect: false,
                configuring: false,
                flush: None,
                flush_extra: 0,
                window: Window::new(),
                said_short: false,
                said_menu: false,
                said_zstd: false,
                sample_type: SampleType::I16,
            }),
            cv: Condvar::new(),
            stream_active: AtomicBool::new(false),
            mem,
            notices,
            shortfall,
            closed: AtomicBool::new(false),
        }))
    }

    pub fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.st)
    }

    fn log(&self, line: String) {
        self.notices.log(format!("{PREFIX}{line}"));
    }

    fn set_fail(&self, f: Fail) -> Fail {
        let mut st = self.state();
        if st.fail.is_none() {
            st.fail = Some(f);
        }
        let f = st.fail.unwrap_or(f);
        drop(st);
        self.cv.notify_all();
        f
    }

    /// The error a failure stands for.
    pub fn error(&self, f: Fail) -> DriverError {
        match f {
            Fail::Busy => DriverError::Busy,
            Fail::Garbled => DriverError::Protocol(Protocol::Malformed),
            Fail::NotSdrpp => DriverError::Protocol(Protocol::NotThisRadio),
            Fail::Closed => DriverError::Net(NetError::Closed),
            Fail::NoAnswer => DriverError::Net(NetError::TimedOut),
        }
    }

    /// The failure so far, mapped for a command that went wrong: busy if a DISCONNECT came, else dropped.
    pub fn dropped(&self) -> DriverError {
        let st = self.state();
        let f = if st.busy { Fail::Busy } else { Fail::Closed };
        drop(st);
        self.error(f)
    }

    /// Sends one command. `answered`: the server answers it, count it as sent.
    pub fn send(&self, c: u32, arg: &[u8]) -> Result<(), DriverError> {
        if self.state().fail.is_some() {
            return Err(self.dropped());
        }
        let p = proto::command_bytes(c, arg);
        let mut w = lock(&self.write);
        if w.write_all(&p).is_err() {
            drop(w);
            self.set_fail(Fail::Closed);
            return Err(self.dropped());
        }
        drop(w);
        let answered = c == cmd::GET_UI
            || (c == cmd::UI_ACTION && arg.first() == Some(&1))
            || c == cmd::SET_FREQUENCY;
        if answered {
            self.state().sent[c as usize] += 1;
        }
        Ok(())
    }

    /// Sends an answered command and returns the answer count to wait for.
    pub fn send_answered(&self, c: u32, arg: &[u8]) -> Result<u64, DriverError> {
        self.send(c, arg)?;
        Ok(self.state().sent[c as usize])
    }

    /// Marks "drop samples until the answer to this command", then sends it without waiting.
    pub fn send_flushed(&self, c: u32, arg: &[u8]) -> Result<(), DriverError> {
        {
            let mut st = self.state();
            let target = st.sent[c as usize] + 1;
            st.flush = Some((c as usize, target));
            st.window.purposeful = true;
        }
        self.send(c, arg)
    }

    /// Waits until `done` holds: stops waiting after `idle` without any packet, after `total`, or when the connection fails.
    pub fn wait(
        &self,
        idle: Duration,
        total: Duration,
        mut done: impl FnMut(&State) -> bool,
    ) -> Result<(), Fail> {
        let start = Instant::now();
        loop {
            {
                let st = self.state();
                if done(&st) {
                    return Ok(());
                }
                if let Some(f) = st.fail {
                    return Err(f);
                }
                let quiet = st.last_packet.max(start).elapsed();
                if quiet >= idle || start.elapsed() >= total {
                    return Err(Fail::NoAnswer);
                }
                if self.stream_active.load(Ordering::Acquire) {
                    drop(
                        self.cv
                            .wait_timeout(st, READ_STEP)
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                    continue;
                }
            }
            match self.reader.try_lock() {
                Ok(mut r) => {
                    let _ = self.pump(&mut r, None);
                }
                Err(_) => {
                    let st = self.state();
                    drop(
                        self.cv
                            .wait_timeout(st, READ_STEP)
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                }
            }
        }
    }

    /// Waits for the answer count of `c` to reach `target`.
    pub fn wait_answer(&self, c: u32, target: u64, idle: Duration) -> Result<(), DriverError> {
        self.wait(idle, Duration::from_secs(30), |st| {
            st.acks[c as usize] >= target
        })
        .map_err(|f| {
            if f == Fail::NoAnswer {
                self.error(f)
            } else {
                self.dropped()
            }
        })
    }

    /// Reads and handles one packet, if one comes within the read step. Ok(true): a packet was handled.
    fn pump(&self, r: &mut Reader, sink: Option<&mut Sink>) -> Result<bool, Fail> {
        if let Some(f) = self.state().fail {
            return Err(f);
        }
        match r.read_one() {
            Ok(None) => Ok(false),
            Err(ReadErr::Invalid) => {
                let f = if self.state().good > 0 {
                    Fail::Garbled
                } else {
                    Fail::NotSdrpp
                };
                Err(self.set_fail(f))
            }
            Err(ReadErr::Closed) => Err(self.set_fail(Fail::Closed)),
            Ok(Some(h)) => {
                let n = h.body_len();
                let Reader {
                    body, zdec, zout, ..
                } = r;
                let res = self.handle(h, &body[..n], zdec, zout, sink);
                self.cv.notify_all();
                res.map(|_| true)
            }
        }
    }

    fn handle(
        &self,
        h: Header,
        body: &[u8],
        zdec: &mut FrameDecoder,
        zout: &mut Vec<u8>,
        sink: Option<&mut Sink>,
    ) -> Result<(), Fail> {
        let u32_at = |o: usize| {
            u32::from_le_bytes(
                body[o..o + 4]
                    .try_into()
                    .expect("the range has the array's length"),
            )
        };
        let mut st = self.state();
        st.good += 1;
        st.last_packet = Instant::now();
        match h.kind {
            pkt::COMMAND if body.len() >= 4 => match u32_at(0) {
                cmd::SET_SAMPLERATE if body.len() >= 12 => {
                    let r = f64::from_le_bytes(
                        body[4..12]
                            .try_into()
                            .expect("the range has the array's length"),
                    );
                    let prev = st.rate.replace(r);
                    st.rate_seq += 1;
                    let foreign =
                        st.expect && !st.configuring && prev.is_some_and(|p| (p - r).abs() >= 0.5);
                    drop(st);
                    lock(&self.mem).rate = Some(r);
                    if foreign {
                        let m = sdrkit_driver_core::fmt_m(r);
                        self.notices.warn(format!(
                            "{PREFIX}{} changed its rate to {m} Msps",
                            self.label
                        ));
                        self.notices.restart();
                    }
                }
                cmd::DISCONNECT => {
                    st.busy = true;
                    drop(st);
                    lock(&self.mem).busy = true;
                    return Err(self.set_fail(Fail::Busy));
                }
                _ => {}
            },
            pkt::COMMAND_ACK if body.len() >= 4 => {
                let c = u32_at(0) as usize;
                if c < cmd::CLIENT_COUNT {
                    if c == cmd::GET_UI as usize || c == cmd::UI_ACTION as usize {
                        match Menu::parse(&body[4..]) {
                            Ok(m) => {
                                lock(&self.mem).menu = Some(m.clone());
                                st.menu = Some(m);
                            }
                            Err(_) => {
                                if !std::mem::replace(&mut st.said_menu, true) {
                                    self.log(format!(
                                        "{} sent a menu that cannot be read",
                                        self.label
                                    ));
                                }
                            }
                        }
                    }
                    st.acks[c] += 1;
                    if let Some((fc, target)) = st.flush {
                        if fc == c && st.acks[c] >= target {
                            st.flush = None;
                            st.flush_extra = FLUSH_EXTRA;
                        }
                    }
                }
            }
            pkt::BASEBAND | pkt::BASEBAND_COMPRESSED => {
                let drop_it = if st.discard || st.flush.is_some() {
                    true
                } else if st.flush_extra > 0 {
                    st.flush_extra -= 1;
                    true
                } else {
                    false
                };
                if drop_it {
                    st.window.purposeful = true;
                }
                let Some(sink) = sink else { return Ok(()) };
                if drop_it {
                    return Ok(());
                }
                drop(st);
                let data: &[u8] = if h.kind == pkt::BASEBAND_COMPRESSED {
                    if inflate(zdec, body, zout).is_err() {
                        let mut st = self.state();
                        if !std::mem::replace(&mut st.said_zstd, true) {
                            drop(st);
                            self.log(format!(
                                "{} sent a compressed packet that cannot be decoded: dropped",
                                self.label
                            ));
                        }
                        return Ok(());
                    }
                    zout
                } else {
                    body
                };
                let n = match Baseband::parse(data) {
                    // Float samples are the ring's own type. Integer samples carry the scale of their packet, so they are
                    // scaled into floats.
                    Some(b) if b.kind == SampleType::F32 => sink.write(b.data),
                    Some(b) => {
                        let n = b.len();
                        sink.write_with(n, |dst, from| b.fill(dst, from));
                        n
                    }
                    None => 0,
                };
                self.count(n as u64, body.len() as u64);
            }
            pkt::ERROR if !body.is_empty() => {
                let t = proto::error_text(body[0]);
                drop(st);
                self.log(format!(
                    "the SDR++ server at {} reports an error: {t}",
                    self.label
                ));
                lock(&self.mem).last_error = Some(t);
            }
            _ => {}
        }
        Ok(())
    }

    /// Counts a sample packet into the throughput window; at the end of a window, publishes it.
    fn count(&self, samples: u64, bytes: u64) {
        let mut st = self.state();
        st.window.samples += samples;
        st.window.bytes += bytes;
        let secs = st.window.start.elapsed().as_secs_f64();
        if secs < WINDOW.as_secs_f64() {
            return;
        }
        let w = std::mem::replace(&mut st.window, Window::new());
        let rate = st.rate.unwrap_or(0.0);
        let counts = !w.purposeful && st.expect && rate > 0.0;
        let want = rate * secs;
        let shortfall = if counts && (w.samples as f64) < SHORT_SHARE * want {
            (want - w.samples as f64).round() as u64
        } else {
            0
        };
        let first = shortfall > 0 && !std::mem::replace(&mut st.said_short, true);
        let t = Throughput {
            mbit_s: w.bytes as f64 * 8.0 / secs / 1e6,
            samples_per_s: w.samples as f64 / secs,
            rate_hz: rate,
            sample_type: st.sample_type,
            shortfall,
        };
        drop(st);
        lock(&self.mem).throughput = Some(t);
        if shortfall > 0 {
            self.shortfall
                .samples
                .fetch_add(shortfall, Ordering::Relaxed);
            self.shortfall.events.fetch_add(1, Ordering::Relaxed);
            *lock(&self.shortfall.last) = Some(Instant::now());
        }
        if first {
            self.log(format!(
                "the network from {} carries only {} of the {} Msps the server sends: the rest is lost. Use a lower sample rate, \
                 8-bit samples or compression, or a faster link",
                self.label,
                sdrkit_driver_core::fmt_m(t.samples_per_s),
                sdrkit_driver_core::fmt_m(rate)
            ));
        }
    }

    /// The stream is held stopped or released.
    pub fn hold(&self, on: bool) {
        let mut st = self.state();
        st.discard = on;
        st.expect = !on;
        st.window.purposeful = true;
        st.last_packet = Instant::now();
    }

    /// Samples are expected from now on (after START), or not (after STOP).
    pub fn expect(&self, on: bool) {
        let mut st = self.state();
        st.expect = on;
        st.last_packet = Instant::now();
        st.window = Window::new();
        if !on {
            st.window.purposeful = true;
        }
    }

    /// The read loop: reads every packet and writes samples into `sink` until a stop, a stall or a failure.
    pub fn run(
        &self,
        sink: &mut Sink,
        stopping: impl Fn() -> bool,
        stall: Duration,
    ) -> Result<(), DriverError> {
        let mut r = lock(&self.reader);
        self.stream_active.store(true, Ordering::Release);
        let res = loop {
            if stopping() || self.closed.load(Ordering::Acquire) {
                break Ok(());
            }
            match self.pump(&mut r, Some(sink)) {
                Ok(_) => {}
                Err(f) => {
                    if f == Fail::Busy {
                        self.log(format!(
                            "{}: another client took the server over",
                            self.label
                        ));
                    }
                    break Err(self.error(f));
                }
            }
            let stalled = {
                let st = self.state();
                st.expect && st.last_packet.elapsed() >= stall
            };
            if stalled {
                self.log(format!(
                    "{} sends no samples (is its radio connected?)",
                    self.label
                ));
                break Err(DriverError::Net(NetError::TimedOut));
            }
        };
        self.stream_active.store(false, Ordering::Release);
        self.cv.notify_all();
        res
    }

    /// Registers the connection with `abort`, so a stop ends a blocked read.
    pub fn register(&self, abort: &Abort) {
        abort.register(lock(&self.ctl).as_ref());
    }

    /// Ends the connection: STOP first when streaming, then the connection goes.
    pub fn close(&self, streaming: bool) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        if streaming {
            let p = proto::command_bytes(cmd::STOP, &[]);
            let _ = lock(&self.write).write_all(&p);
        }
        lock(&self.ctl).shutdown();
        self.set_fail(Fail::Closed);
    }
}
