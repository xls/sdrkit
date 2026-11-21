//! One TCI connection run on the shared driver base: connect and handshake, configure, the read loop and the close.

use crate::proto::{self, kind, Header, ServerInfo, Verdict};
use crate::{caps_for, lock, receiver_id, Throughput, Timing};
use crate::{
    ARRIVING, CHANNELS, DEVICE, FORMAT, IF_MAX, IF_MIN, IQ_RATE, LOW_THROUGHPUT, MBIT_S, PANORAMA,
    RECEIVERS, RECEIVER_IN_USE, RECEIVE_ONLY, SERVER, SERVER_RUNNING, SERVER_TRX, SMETER,
    TX_FRAMES, TX_PACKETS, TX_UNDERRUN_FRAMES, UNUSABLE, VFO_MAX, VFO_MIN,
};
use sdrkit_core::net::{Conn as NetConn, NetError, Transport};
use sdrkit_core::Results;
use sdrkit_core::{Hz, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::ws::{self, Message, ReadError, Reader, Writer};
use sdrkit_driver_core::{Capabilities, DriverError, Protocol, CENTRE, RATE};
use sdrkit_graph::stream::SampleType;
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How many messages the handshake reads at most while waiting for `ready`.
pub(crate) const HANDSHAKE_MESSAGES: usize = 256;
/// The transmit audio queue, in frames (1 s at 48 000 sps).
pub(crate) const TX_QUEUE: usize = 48_000;
/// How long packets at the previous rate are not followed after a rate was sent.
const RATE_GRACE: Duration = Duration::from_secs(1);

static GENERATION: AtomicU64 = AtomicU64::new(1);

// ---- Per address ----

/// What the driver keeps per server address.
#[derive(Debug, Default)]
pub(crate) struct Entry {
    pub info: ServerInfo,
    /// The receiver in use on the last connection.
    pub receiver: Option<usize>,
    /// The IQ rate the server confirmed (by command or packet header).
    pub confirmed_rate: Option<Hz>,
    /// The sample type of the last usable packet.
    pub format: Option<u32>,
    /// The panorama centre the server reported, when it differs from the one sent.
    pub moved: Option<Hz>,
    /// The last throughput window.
    pub last: Option<Throughput>,
    /// Packets dropped: malformed, or not in the sample type of the ring.
    pub unusable: u64,
    /// The rate the server ran when it was last sent a rate: (sent, run).
    pub answer: Option<(u32, Hz)>,
}

pub(crate) type Shared = Arc<Mutex<Entry>>;

// ---- Transmit state, per source ----

#[derive(Debug, Default)]
pub(crate) struct TxLive {
    /// The connection a transmission runs on; None when off.
    pub gen: Option<u64>,
    /// Mono audio frames waiting.
    pub queue: VecDeque<f32>,
    pub packets: u64,
    pub frames: u64,
    pub underruns: u64,
    pub underrun_frames: u64,
    /// The transmission ended because its connection was lost.
    pub lost: bool,
    pub since: Option<Instant>,
}

pub(crate) type TxShared = Arc<Mutex<TxLive>>;

// ---- One connection ----

/// The sending side: each frame goes out whole. After the close frame, every send fails.
#[derive(Debug)]
pub(crate) struct Link(Arc<Writer>);

impl Link {
    pub fn send(&self, opcode: u8, payload: &[u8]) -> Result<(), DriverError> {
        self.0
            .send(opcode, payload)
            .map_err(|_| DriverError::Net(NetError::Closed))
    }
    pub fn text(&self, t: &str) -> Result<(), DriverError> {
        self.send(ws::op::TEXT, t.as_bytes())
    }
}

/// What was sent and what runs on one connection.
#[derive(Debug, Default)]
pub(crate) struct Live {
    /// The driver's rate.
    pub rate: Hz,
    pub sent_rate: Option<u32>,
    /// The previous rate sent and when it changed (packets at it are not followed for a moment).
    grace: Option<(u32, Instant)>,
    pub sent_freq: Option<u64>,
    prev_freq: Option<u64>,
    pub centre: Hz,
    pub iq_started: bool,
    pub start_sent: bool,
    pub iq_seen: bool,
    moved_logged: Option<u64>,
}

/// One connection, shared by the control side, the read loop and a transmitter.
#[derive(Debug)]
pub(crate) struct Conn {
    pub gen: u64,
    pub label: String,
    pub rx: usize,
    pub link: Link,
    pub live: Mutex<Live>,
}

/// Where a server is and how the driver waits on it.
pub(crate) struct At {
    pub transport: Arc<dyn Transport>,
    pub host: String,
    pub port: u16,
    pub label: String,
    pub timing: Timing,
}

/// The upgrade and the wait for `ready`, each read waiting up to `wait`. Returns the reader, the sending side and what the
/// server said.
pub(crate) fn handshake(
    sock: &mut dyn NetConn,
    label: &str,
    timing: &Timing,
    wait: Duration,
    stop: &dyn Fn() -> bool,
) -> Result<(Reader, Link, ServerInfo), DriverError> {
    sock.set_read_timeout(Some(timing.tick))?;
    let up = ws::Upgrade {
        path: "/",
        host: label,
        extra: &[],
        key: timing.fixed_key,
        mask: timing.fixed_mask,
        wait,
        max_message: ws::MAX_MESSAGE,
        frame_wait: wait * 11,
    };
    let link = ws::Link::new(sock.try_clone()?, false)?;
    let (mut reader, writer) = ws::handshake(link, &up, stop)?;
    let link = Link(writer);
    let mut info = ServerInfo::default();
    for _ in 0..HANDSHAKE_MESSAGES {
        match reader.read(wait, stop) {
            Ok(Message::Text(b)) => {
                let text = String::from_utf8_lossy(b);
                for c in proto::parse(&text) {
                    info.apply(&c);
                }
                if info.ready {
                    break;
                }
            }
            Ok(Message::Binary(_)) => {}
            Err(ReadError::Timeout) => break,
            Err(e) => return Err(e.into()),
        }
    }
    if !info.ready && info.program.is_none() {
        link.0.send_close(&[0x03, 0xe8]);
        sock.shutdown();
        return Err(DriverError::Protocol(Protocol::NotThisRadio));
    }
    info.seen = true;
    Ok((reader, link, info))
}

/// Ends a connection once the last commands are out: a close frame (status 1000), then read and drop what arrives for up to
/// `total`, in steps of `step`, then shut the connection.
pub(crate) fn finish(link: &Link, sock: &mut dyn NetConn, step: Duration, total: Duration) {
    if link.0.is_closed() {
        return;
    }
    link.0.send_close(&[0x03, 0xe8]);
    let _ = sock.set_read_timeout(Some(step.max(Duration::from_millis(1))));
    let end = Instant::now() + total;
    let mut sink = [0u8; 16 << 10];
    while Instant::now() < end {
        match sock.read(&mut sink) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) if ws::is_timeout(&e) || e.kind() == ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
    sock.shutdown();
}

/// The radio of one TCI connection, as the base drives it.
pub(crate) struct TciRadio {
    pub(crate) conn: Arc<Conn>,
    sock: Option<Box<dyn NetConn>>,
    reader: Option<Reader>,
    entry: Shared,
    tx: TxShared,
    timing: Timing,
    closed: bool,
}

impl TciRadio {
    /// Connects, runs the handshake and takes the receiver of `values`.
    pub(crate) fn connect(
        at: &At,
        entry: Shared,
        tx: TxShared,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<TciRadio, DriverError> {
        let (label, timing) = (at.label.as_str(), &at.timing);
        let mut sock = net::connect(
            &at.transport,
            &at.host,
            at.port,
            false,
            timing.connect,
            abort,
        )?;
        net::recv_buffer(sock.as_mut(), &[16 << 20, 8 << 20, 4 << 20]);
        let stop = || abort.fired();
        let (reader, link, info) =
            handshake(sock.as_mut(), label, timing, timing.handshake, &stop)?;
        let rx = match usize::try_from(values.get(&crate::RECEIVER)) {
            Ok(r) if r < info.receivers() => r,
            _ => {
                finish(&link, sock.as_mut(), timing.drain_step, timing.drain_total);
                return Err(DriverError::Refused { id: receiver_id() });
            }
        };
        {
            let mut e = lock(&entry);
            e.info = info.clone();
            e.receiver = Some(rx);
            e.moved = None;
            e.format = None;
            e.confirmed_rate = None;
            e.last = None;
        }
        notices.log(format!(
            "TCI: connected to {} at {label} ({} {})",
            info.device
                .as_deref()
                .filter(|d| !d.is_empty())
                .unwrap_or("a server"),
            info.program.as_deref().unwrap_or(""),
            info.version.as_deref().unwrap_or("")
        ));
        let conn = Arc::new(Conn {
            gen: GENERATION.fetch_add(1, Ordering::Relaxed),
            label: label.to_string(),
            rx,
            link,
            live: Mutex::new(Live::default()),
        });
        Ok(TciRadio {
            conn,
            sock: Some(sock),
            reader: Some(reader),
            entry,
            tx,
            timing: timing.clone(),
            closed: false,
        })
    }
}

impl Radio for TciRadio {
    type Stream = TciStream;

    fn readings(&mut self, results: &mut Results) {
        let e = lock(&self.entry);
        let t = lock(&self.tx);
        let i = &e.info;
        let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        let server = i.program.as_ref().map(|p| {
            format!("{p} {}", i.version.as_deref().unwrap_or(""))
                .trim()
                .to_string()
        });
        let mut f = results.fields_mut();
        f.opt(&SERVER, server.as_deref());
        f.opt(&DEVICE, i.device.as_deref());
        f.set(&RECEIVE_ONLY, i.is_receive_only());
        f.set(&RECEIVERS, count(i.receivers() as u64));
        f.opt(&CHANNELS, i.channels_count.map(i64::from));
        f.opt(&VFO_MIN, i.vfo_limits.map(|l| l.0));
        f.opt(&VFO_MAX, i.vfo_limits.map(|l| l.1));
        f.opt(&IF_MIN, i.if_limits.map(|l| l.0));
        f.opt(&IF_MAX, i.if_limits.map(|l| l.1));
        f.opt(&RECEIVER_IN_USE, e.receiver.map(|r| count(r as u64)));
        f.opt(&SMETER, e.receiver.and_then(|r| i.smeter[r]).map(|s| s.0));
        f.opt(&SERVER_TRX, e.receiver.and_then(|r| i.trx[r]));
        f.set(&TX_PACKETS, count(t.packets));
        f.set(&TX_FRAMES, count(t.frames));
        f.set(&TX_UNDERRUN_FRAMES, count(t.underrun_frames));
        f.opt(&IQ_RATE, e.confirmed_rate);
        f.opt(&FORMAT, e.format.map(proto::format::name));
        f.set(&UNUSABLE, count(e.unusable));
        f.opt(&SERVER_RUNNING, i.running);
        f.opt(&PANORAMA, e.moved);
        f.opt(&MBIT_S, e.last.map(|t| t.mbit_s));
        f.opt(&ARRIVING, e.last.map(|t| t.achieved_hz));
        f.opt(&LOW_THROUGHPUT, e.last.map(|t| t.low));
    }

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        let conn = self.conn.clone();
        let rx = conn.rx;
        let (want_rate, centre) = (values.get(&RATE), values.get(&CENTRE));
        let mut l = lock(&conn.live);
        // Rate.
        let chosen = proto::choose_rate(want_rate);
        if l.sent_rate != Some(chosen) {
            conn.link.text(&format!("iq_samplerate:{chosen};"))?;
            if let Some(old) = l.sent_rate {
                l.grace = Some((old, Instant::now()));
            }
            l.sent_rate = Some(chosen);
            let e = lock(&self.entry);
            l.rate = match e.answer {
                Some((sent, ran)) if sent == chosen => ran,
                _ => Hz::from(chosen),
            };
        }
        // Frequency.
        let f = proto::freq_sent(centre);
        let limits = lock(&self.entry).info.vfo_limits;
        let mut note = String::new();
        if let Some((lo, hi)) = limits {
            if (f as Hz) < lo || (f as Hz) > hi {
                note = format!(
                    "this TCI radio is specified for {} to {} MHz; the driver may not reach {} MHz or the signal may be weak or absent",
                    proto::mhz(lo),
                    proto::mhz(hi),
                    proto::mhz(f as Hz)
                );
            }
        }
        if l.sent_freq != Some(f) {
            conn.link.text(&format!("dds:{rx},{f};"))?;
            conn.link.text(&format!("vfo:{rx},0,{f};"))?;
            l.prev_freq = l.sent_freq;
            l.sent_freq = Some(f);
            l.moved_logged = None;
            lock(&self.entry).moved = None;
        }
        l.centre = f as Hz;
        if note.is_empty() && want_rate > 0.0 && l.rate < 0.97 * want_rate {
            note = format!(
                "this radio only offers {:.2} Msps, {:.2} Msps were requested for this channel width",
                l.rate / 1e6,
                want_rate / 1e6
            );
        }
        Ok(note)
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<TciStream, DriverError> {
        let mut reader = self
            .reader
            .take()
            .ok_or(DriverError::Net(NetError::Closed))?;
        reader.frame_wait = self.timing.stall * 11;
        reader.link().register(abort);
        let conn = self.conn.clone();
        {
            let mut l = lock(&conn.live);
            if !l.iq_started {
                conn.link.text(&format!("iq_start:{};", conn.rx))?;
                l.iq_started = true;
            }
        }
        Ok(TciStream {
            reader,
            st: StreamState {
                conn,
                entry: self.entry.clone(),
                tx: self.tx.clone(),
                win: Window {
                    length: self.timing.window,
                    ..Window::default()
                },
                wait: self.timing.stall,
                txbuf: Vec::with_capacity(4096),
                unusable: 0,
                iq_seen: false,
            },
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        let conn = self.conn.clone();
        let rx = conn.rx;
        let transmitting = {
            let mut t = lock(&self.tx);
            if t.gen == Some(conn.gen) {
                t.gen = None;
                t.queue.clear();
                true
            } else {
                false
            }
        };
        if transmitting {
            let _ = conn.link.text(&format!("trx:{rx},false;"));
        }
        if lock(&conn.live).iq_started {
            let _ = conn.link.text(&format!("iq_stop:{rx};"));
        }
        if let Some(mut sock) = self.sock.take() {
            finish(
                &conn.link,
                sock.as_mut(),
                self.timing.drain_step,
                self.timing.drain_total,
            );
        }
        self.reader = None;
    }

    fn rate_hz(&self) -> Hz {
        lock(&self.conn.live).rate
    }

    fn centre_hz(&self) -> Hz {
        lock(&self.conn.live).centre
    }

    fn sample_type(&self) -> SampleType {
        SampleType::Cf32
    }

    fn capabilities(&self) -> Capabilities {
        caps_for(&self.conn.label, &lock(&self.entry).info)
    }
}

impl Drop for TciRadio {
    fn drop(&mut self) {
        self.close();
    }
}

/// The read loop of one TCI connection.
pub(crate) struct TciStream {
    reader: Reader,
    st: StreamState,
}

/// What the read loop keeps besides the reader.
struct StreamState {
    conn: Arc<Conn>,
    entry: Shared,
    tx: TxShared,
    win: Window,
    /// No message for this long ends the read loop.
    wait: Duration,
    txbuf: Vec<u8>,
    unusable: u64,
    iq_seen: bool,
}

/// The throughput window of a stream.
#[derive(Debug, Default)]
struct Window {
    length: Duration,
    start: Option<Instant>,
    bytes: u64,
    samples: u64,
    low: u32,
    format: Option<u32>,
}

impl StreamState {
    /// A stopped radio with no IQ yet gets one `start;` per connection.
    fn check_start(&self, ctx: &StreamCtx) {
        let stopped = lock(&self.entry).info.running == Some(false);
        if !stopped {
            return;
        }
        let mut l = lock(&self.conn.live);
        if !l.iq_seen && !l.start_sent {
            l.start_sent = true;
            if self.conn.link.text("start;").is_ok() {
                ctx.notices
                    .log("TCI: the server's radio is stopped, sending start");
            }
        }
    }

    /// A rate named by the server: followed when more than 0.5 sps from the rate of the driver.
    fn follow_rate(conn: &Conn, entry: &Shared, win: &mut Window, r: Hz, ctx: &StreamCtx) {
        let mut l = lock(&conn.live);
        if let Some((old, at)) = l.grace {
            if at.elapsed() < RATE_GRACE && (r - old as Hz).abs() <= 0.5 {
                return;
            }
        }
        {
            let mut e = lock(entry);
            e.confirmed_rate = Some(r);
            if let Some(sent) = l.sent_rate {
                e.answer = Some((sent, r));
            }
            if (r - l.rate).abs() <= 0.5 {
                return;
            }
        }
        (win.start, win.low) = (None, 0);
        l.rate = r;
        drop(l);
        let m = sdrkit_driver_core::fmt_m(r);
        ctx.notices
            .warn(format!("the TCI server changed its IQ rate to {m} Msps"));
        ctx.notices.restart();
    }

    fn on_text(&mut self, msg: &[u8], ctx: &StreamCtx) {
        let text = String::from_utf8_lossy(msg);
        let rx = self.conn.rx;
        for c in proto::parse(&text) {
            lock(&self.entry).info.apply(&c);
            match c.name.as_str() {
                "iq_samplerate" => {
                    if let Some(r) = c.num(0).filter(|r| *r > 0.0 && *r < 1e8) {
                        Self::follow_rate(&self.conn, &self.entry, &mut self.win, r, ctx);
                    }
                }
                "dds" if c.rx(0) == Some(rx) => {
                    let Some(v) = c.num(1) else { continue };
                    let mut l = lock(&self.conn.live);
                    let near = |s: Option<u64>| s.is_some_and(|s| (s as Hz - v).abs() <= 1.0);
                    if near(l.sent_freq) {
                        lock(&self.entry).moved = None;
                    } else if !near(l.prev_freq) {
                        let vv = v.max(0.0).round() as u64;
                        lock(&self.entry).moved = Some(v);
                        if l.moved_logged != Some(vv) {
                            l.moved_logged = Some(vv);
                            ctx.notices.log(format!(
                                "TCI: the server moved the panorama to {} MHz; sdrkit keeps tuning {} MHz",
                                proto::mhz(v),
                                proto::mhz(l.centre)
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        self.check_start(ctx);
    }

    fn on_binary(&mut self, msg: &[u8], sink: &mut Sink, ctx: &StreamCtx) {
        let rx = self.conn.rx as u32;
        if msg.len() >= proto::HEADER {
            let h = Header::parse(msg);
            if h.kind == kind::TX_CHRONO && h.receiver == rx {
                answer_chrono(&self.conn, &self.tx, &h, &mut self.txbuf);
                return;
            }
        }
        let (h, count) = match proto::check(msg, rx) {
            Verdict::Unusable => {
                self.unusable += 1;
                return;
            }
            Verdict::Skipped => return,
            Verdict::Use { header, n } => (header, n),
        };
        let rate = lock(&self.conn.live).rate;
        if proto::follows_rate(h.sample_rate, rate) {
            Self::follow_rate(
                &self.conn,
                &self.entry,
                &mut self.win,
                Hz::from(h.sample_rate),
                ctx,
            );
        }
        if !self.iq_seen {
            self.iq_seen = true;
            lock(&self.conn.live).iq_seen = true;
            ctx.notices.log(format!(
                "TCI: IQ is arriving ({} Hz, format {})",
                h.sample_rate, h.format
            ));
        }
        if self.win.format != Some(h.format) {
            self.win.format = Some(h.format);
            lock(&self.entry).format = Some(h.format);
            if h.format != proto::format::FLOAT32 {
                ctx.notices.warn(format!(
                    "TCI: the server sends {} IQ: the driver takes float32 only, these packets are dropped",
                    proto::format::name(h.format)
                ));
            }
        }
        if h.format != proto::format::FLOAT32 {
            self.unusable += 1;
            return;
        }
        let data = &msg[proto::HEADER..proto::HEADER + count * 8];
        sink.write(data);
        self.count(data.len(), count, rate);
    }

    /// Counts one usable packet into the throughput window. The entry is locked only at the end of a window.
    fn count(&mut self, bytes: usize, samples: usize, rate: Hz) {
        let now = Instant::now();
        let w = &mut self.win;
        let Some(start) = w.start else {
            (w.start, w.bytes, w.samples) = (Some(now), 0, 0);
            return;
        };
        w.bytes += bytes as u64;
        w.samples += samples as u64;
        let secs = now.duration_since(start).as_secs_f64();
        if secs < w.length.as_secs_f64() {
            return;
        }
        let achieved = w.samples as f64 / secs;
        w.low = if achieved < 0.95 * rate { w.low + 1 } else { 0 };
        let t = Throughput {
            mbit_s: w.bytes as f64 * 8.0 / secs / 1e6,
            achieved_hz: achieved,
            low: w.low >= 3,
        };
        (w.start, w.bytes, w.samples) = (Some(now), 0, 0);
        let mut e = lock(&self.entry);
        e.last = Some(t);
        e.unusable = self.unusable;
        if e.confirmed_rate.is_none() {
            e.confirmed_rate = Some(rate);
        }
    }
}

/// Answers a chrono packet with one transmit audio packet, when a transmission runs on this connection.
fn answer_chrono(conn: &Conn, tx: &TxShared, h: &Header, buf: &mut Vec<u8>) {
    let mut t = lock(tx);
    if t.gen != Some(conn.gen) {
        return;
    }
    let max_values = ((ws::MAX_MESSAGE - proto::HEADER) / 4) as u32;
    let length = h.length.min(max_values);
    let short = proto::tx_audio_packet(buf, conn.rx as u32, length, || t.queue.pop_front());
    t.packets += 1;
    t.frames += (length / 2) as u64;
    if short > 0 {
        t.underruns += 1;
        t.underrun_frames += short as u64;
    }
    drop(t);
    let _ = conn.link.send(ws::op::BINARY, buf);
}

impl RadioStream for TciStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        self.st.check_start(ctx);
        let conn = self.st.conn.clone();
        let stop = || ctx.stopping();
        let wait = self.st.wait;
        let result = loop {
            if ctx.stopping() {
                break Ok(());
            }
            match self.reader.read(wait, &stop) {
                Ok(Message::Text(b)) => self.st.on_text(b, ctx),
                Ok(Message::Binary(b)) => self.st.on_binary(b, sink, ctx),
                Err(ReadError::Stopped) => break Ok(()),
                Err(_) if ctx.stopping() => break Ok(()),
                Err(ReadError::Timeout) => {
                    if !self.st.iq_seen {
                        ctx.notices.log(format!(
                            "TCI: connected to {} but no IQ arrived in {} s (is the radio switched on in ExpertSDR3, and is \
                             the receiver number right?)",
                            conn.label,
                            wait.as_secs_f64()
                        ));
                    }
                    break Err(DriverError::Net(NetError::TimedOut));
                }
                Err(e) => break Err(e.into()),
            }
        };
        if self.st.unusable > 0 {
            lock(&self.st.entry).unusable = self.st.unusable;
            ctx.notices.log(format!(
                "TCI: {} malformed or unusable packets ignored",
                self.st.unusable
            ));
        }
        if result.is_err() && !ctx.stopping() {
            let mut t = lock(&self.st.tx);
            if t.gen == Some(conn.gen) {
                t.gen = None;
                t.lost = true;
                t.queue.clear();
                drop(t);
                let _ = conn.link.text(&format!("trx:{},false;", conn.rx));
            }
        }
        result
    }
}
