//! One connection to a SpyServer: connect, handshake, configure, and the read loop with its throughput count.

use crate::plan::{self, RatePlan};
use crate::proto::{self, format, msg, setting, ClientSync, DeviceInfo, Header};
use crate::{
    caps_for, ServerInfo, SpyError, SpySetting, Throughput, ARRIVING, DEVICE, DIGITAL_GAIN,
    DROPPED_SAMPLES, FORMAT, GAIN_INDEX, IN_CONTROL, LOW_THROUGHPUT, MBIT_S, SERIAL,
};
use sdrkit_core::net::{Conn, NetError, Transport};
use sdrkit_core::{Hz, Results, SettingSet, Values};
use sdrkit_driver_core::net::{self, Abort};
use sdrkit_driver_core::runner::{Notices, Radio, RadioStream, Sink, StreamCtx};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverError, Protocol, RadioSetting, AGC, CENTRE, GAIN, RATE,
};
use sdrkit_graph::stream::SampleType;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The timings of a driver: 3 s for the connect, each handshake read and a stall, and 1 s throughput windows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// The connect timeout.
    pub connect: Duration,
    /// The timeout of each handshake read.
    pub handshake: Duration,
    /// No complete read for this long ends the stream.
    pub stall: Duration,
    /// The shortest throughput window.
    pub window: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect: Duration::from_secs(3),
            handshake: Duration::from_secs(3),
            stall: Duration::from_secs(3),
            window: Duration::from_secs(1),
        }
    }
}

/// The handshake reads at most this many messages.
pub(crate) const HANDSHAKE_MESSAGES: usize = 64;

/// What the driver keeps per server address.
#[derive(Debug, Default)]
pub(crate) struct Entry {
    pub info: ServerInfo,
    /// The rate in use (0 before the first configure).
    pub rate_hz: Hz,
    pub last: Option<Throughput>,
    /// The digital gain of the last IQ message, in dB.
    pub digital_gain_db: Option<f64>,
    /// Counts the times "can control" changed in a CLIENT_SYNC. A gain sent before a change is forgotten.
    pub control_changes: u64,
}

pub(crate) type Shared = Arc<Mutex<Entry>>;

/// The IQ messages the server dropped on one source, over all its connections: samples filled with zeros and the gaps.
#[derive(Debug, Default)]
pub(crate) struct Dropped {
    samples: AtomicU64,
    gaps: AtomicU64,
}

impl Dropped {
    fn add(&self, samples: u64) {
        self.samples.fetch_add(samples, Ordering::Relaxed);
        self.gaps.fetch_add(1, Ordering::Relaxed);
    }

    /// The samples filled and the number of gaps.
    pub(crate) fn totals(&self) -> (u64, u64) {
        (
            self.samples.load(Ordering::Relaxed),
            self.gaps.load(Ordering::Relaxed),
        )
    }
}

/// The sequence numbers of the IQ messages of a stream. A server that cannot send fast enough drops whole messages; the
/// sequence numbers show how many.
#[derive(Debug, Default)]
struct Sequence {
    /// The sequence number of the last IQ message, and its whole samples.
    last: Option<(u32, usize)>,
    /// Messages of another kind since the last IQ message: they can share the counter.
    others: u32,
}

impl Sequence {
    /// Takes the header of an IQ message and returns the samples to fill before its body: the dropped messages times the
    /// samples of the last IQ message, or 0 when none is missing or the gap is longer than `limit` samples (a restart).
    fn iq(&mut self, seq: u32, samples: usize, limit: f64) -> usize {
        let fill = match self.last {
            Some((last, size)) => {
                let gap = i64::from(seq.wrapping_sub(last).wrapping_sub(1) as i32)
                    - i64::from(self.others);
                let fill = usize::try_from(gap).unwrap_or(0).saturating_mul(size);
                if fill as f64 <= limit {
                    fill
                } else {
                    0
                }
            }
            None => 0,
        };
        self.last = Some((seq, samples));
        self.others = 0;
        fill
    }

    /// Takes the header of any other message.
    fn other(&mut self) {
        self.others = self.others.saturating_add(1);
    }
}

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Where a server is.
pub(crate) struct At<'a> {
    pub transport: &'a Arc<dyn Transport>,
    pub host: &'a str,
    pub port: u16,
    pub label: &'a str,
}

fn closed(e: std::io::Error) -> DriverError {
    match NetError::from(e) {
        NetError::TimedOut => DriverError::Net(NetError::TimedOut),
        _ => DriverError::Net(NetError::Closed),
    }
}

/// Reads the header of one message. Its body of `body_size` bytes follows.
fn read_header(r: &mut dyn Conn) -> Result<Header, DriverError> {
    let mut h = [0u8; proto::HEADER_LEN];
    r.read_exact(&mut h).map_err(closed)?;
    let h = Header::parse(&h);
    if !h.size_ok() {
        return Err(DriverError::Protocol(Protocol::TooLong));
    }
    Ok(h)
}

/// Reads the body of a message into `buf[..size]` (`buf` grows only when it is too small) and returns its size.
fn read_body(r: &mut dyn Conn, h: &Header, buf: &mut Vec<u8>) -> Result<usize, DriverError> {
    let n = h.body_size as usize;
    if buf.len() < n {
        buf.resize(n, 0);
    }
    r.read_exact(&mut buf[..n]).map_err(closed)?;
    Ok(n)
}

/// Reads one message. Its body lands in `buf[..size]`.
fn read_message(r: &mut dyn Conn, buf: &mut Vec<u8>) -> Result<(Header, usize), DriverError> {
    let h = read_header(r)?;
    let n = read_body(r, &h, buf)?;
    Ok((h, n))
}

/// The handshake: HELLO, then up to 64 messages until a DEVICE_INFO and a CLIENT_SYNC have both arrived. The read timeout of
/// `conn` must be set.
pub(crate) fn handshake(conn: &mut dyn Conn) -> Result<(DeviceInfo, ClientSync), DriverError> {
    conn.write_all(&proto::hello(proto::CLIENT_NAME))
        .map_err(closed)?;
    let mut dev = None;
    let mut sync = None;
    let mut body = Vec::with_capacity(64);
    for _ in 0..HANDSHAKE_MESSAGES {
        let Ok((h, n)) = read_message(conn, &mut body) else {
            break;
        };
        let body = &body[..n];
        match h.msg_type {
            msg::DEVICE_INFO => {
                let d = DeviceInfo::parse(body);
                if d.no_radio() {
                    return Err(DriverError::driver(SpyError::NoRadio));
                }
                dev = Some(d);
            }
            msg::CLIENT_SYNC => sync = Some(ClientSync::parse(body)),
            _ => {}
        }
        if let (Some(d), Some(s)) = (dev, sync) {
            return Ok((d, s));
        }
    }
    Err(DriverError::Protocol(if dev.is_none() {
        Protocol::NotThisRadio
    } else {
        Protocol::MissingPart
    }))
}

/// Stores what a handshake found.
fn store(entry: &Shared, dev: DeviceInfo, sync: ClientSync) {
    lock(entry).info = ServerInfo {
        device: dev,
        sync: Some(sync),
    };
}

/// The sample type of the ring for an IQ format of the server.
fn sample_type_of(fmt: u32) -> Result<SampleType, DriverError> {
    match fmt {
        format::UINT8 => Ok(SampleType::Cu8),
        format::INT16 => Ok(SampleType::Cs16),
        format::FLOAT => Ok(SampleType::Cf32),
        f => Err(DriverError::driver(SpyError::WireFormat { format: f })),
    }
}

/// Keeps `fmt` as the format of the radio, or refuses it if the radio was opened with another one.
fn fix_format(fixed: &Mutex<Option<u32>>, fmt: u32) -> Result<(), DriverError> {
    if *lock(fixed).get_or_insert(fmt) == fmt {
        Ok(())
    } else {
        Err(DriverError::Refused {
            id: own_id(SpySetting::Format),
        })
    }
}

/// One connection to a SpyServer.
pub(crate) struct SpyRadio {
    label: String,
    conn: Box<dyn Conn>,
    entry: Shared,
    dropped: Arc<Dropped>,
    sent: HashMap<u32, u32>,
    enabled: bool,
    rate: Hz,
    centre: Hz,
    format: u32,
    sample: SampleType,
    timing: Timing,
    closed: bool,
    /// The `control_changes` of the entry when the gain was last looked at.
    control_seen: u64,
}

impl SpyRadio {
    /// Connects, says hello, reads the server information, stores it and requests IQ only.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open(
        at: &At<'_>,
        entry: Shared,
        fixed: &Mutex<Option<u32>>,
        dropped: Arc<Dropped>,
        timing: Timing,
        values: &Values,
        abort: &Arc<Abort>,
        notices: &Notices,
    ) -> Result<SpyRadio, DriverError> {
        let mut conn = net::connect(at.transport, at.host, at.port, false, timing.connect, abort)?;
        net::recv_buffer(conn.as_mut(), &[16 << 20, 8 << 20, 4 << 20]);
        conn.set_read_timeout(Some(timing.handshake))?;
        let (dev, sync) = match handshake(conn.as_mut()) {
            Ok(v) => v,
            Err(e) => {
                conn.shutdown();
                return Err(e);
            }
        };
        let fmt = plan::iq_format(values.get(&FORMAT), dev.forced_iq_format);
        let sample = match fix_format(fixed, fmt).and_then(|()| sample_type_of(fmt)) {
            Ok(s) => s,
            Err(e) => {
                conn.shutdown();
                return Err(e);
            }
        };
        store(&entry, dev, sync);
        let control_seen = {
            let mut e = lock(&entry);
            e.last = None;
            e.digital_gain_db = None;
            e.control_changes
        };
        notices.log(format!(
            "SpyServer: connected to {} ({}, serial {:08X}, {:.3} Msps, {})",
            at.label,
            dev.device_name(),
            dev.serial,
            f64::from(dev.max_sample_rate) / 1e6,
            if sync.can_control {
                "in control"
            } else {
                "read only: it keeps its own frequency and gain"
            }
        ));
        let mut r = SpyRadio {
            label: at.label.to_string(),
            conn,
            entry,
            dropped,
            sent: HashMap::new(),
            enabled: false,
            rate: 0.0,
            centre: 0.0,
            format: fmt,
            sample,
            timing,
            closed: false,
            control_seen,
        };
        if let Err(e) = r.send(setting::STREAMING_MODE, proto::stream::MODE_IQ_ONLY) {
            r.close();
            return Err(e);
        }
        Ok(r)
    }

    fn send(&mut self, id: u32, value: u32) -> Result<(), DriverError> {
        self.conn
            .write_all(&proto::set_setting(id, value))
            .map_err(closed)?;
        self.sent.insert(id, value);
        Ok(())
    }

    fn send_changed(&mut self, id: u32, value: u32) -> Result<(), DriverError> {
        if self.sent.get(&id) != Some(&value) {
            self.send(id, value)?;
        }
        Ok(())
    }
}

impl Radio for SpyRadio {
    type Stream = SpyStream;

    fn readings(&mut self, results: &mut Results) {
        let e = lock(&self.entry);
        let d = &e.info.device;
        let mut f = results.fields_mut();
        f.set(&DEVICE, d.device_name());
        f.set(&SERIAL, i64::from(d.serial));
        f.opt(&IN_CONTROL, e.info.sync.map(|s| s.can_control));
        f.opt(&GAIN_INDEX, e.info.sync.map(|s| i64::from(s.gain)));
        f.opt(&DIGITAL_GAIN, e.digital_gain_db);
        f.opt(&MBIT_S, e.last.map(|t| t.mbit_s));
        f.opt(&ARRIVING, e.last.map(|t| t.achieved_hz));
        f.opt(&LOW_THROUGHPUT, e.last.map(|t| t.low));
        f.set(
            &DROPPED_SAMPLES,
            i64::try_from(self.dropped.totals().0).unwrap_or(i64::MAX),
        );
    }

    fn check(&self, values: &Values) -> Result<(), DriverError> {
        let forced = lock(&self.entry).info.device.forced_iq_format;
        if plan::iq_format(values.get(&FORMAT), forced) != self.format {
            return Err(DriverError::Refused {
                id: own_id(SpySetting::Format),
            });
        }
        Ok(())
    }

    fn configure(&mut self, values: &Values, _live: bool) -> Result<String, DriverError> {
        self.check(values)?;
        let (info, control_changes) = {
            let e = lock(&self.entry);
            (e.info, e.control_changes)
        };
        if control_changes != self.control_seen {
            // Control passed to or from this client: the gain of the server can be another client's now.
            self.control_seen = control_changes;
            self.sent.remove(&setting::GAIN);
        }
        let dev = info.device;
        let plan: RatePlan = plan::plan_rate(&dev, values.get(&RATE))
            .ok_or(DriverError::driver(SpyError::NoRate))?;
        let centre = values.get(&CENTRE);
        let f = plan::frequency_sent(centre).ok_or(DriverError::Refused {
            id: RadioSetting::Centre.id(),
        })?;
        let gain = if values.get(&AGC) {
            None
        } else {
            Some(
                plan::gain_sent(values.get(&GAIN), dev.max_gain_index).ok_or(
                    DriverError::Refused {
                        id: RadioSetting::Gain.id(),
                    },
                )?,
            )
        };
        // In this order: format, decimation, frequency, gain.
        self.send_changed(setting::IQ_FORMAT, self.format)?;
        self.send_changed(setting::IQ_DECIMATION, plan.decimation)?;
        self.send_changed(setting::IQ_FREQUENCY, f)?;
        if let Some(g) = gain.filter(|_| info.sync.is_some_and(|s| s.can_control)) {
            self.send_changed(setting::GAIN, g)?;
        }
        self.rate = plan.rate_hz;
        self.centre = Hz::from(f);
        {
            lock(&self.entry).rate_hz = plan.rate_hz;
        }
        let mut notes: Vec<String> = Vec::new();
        notes.extend(plan::range_note(centre, info.tuning_range()));
        if let Some(s) = &info.sync {
            notes.extend(plan::read_only_note(&self.label, Hz::from(f), s));
        }
        if notes.is_empty() {
            notes.extend(plan.warning());
        }
        Ok(notes.join("; "))
    }

    fn stream(&mut self, abort: &Arc<Abort>) -> Result<SpyStream, DriverError> {
        if !self.enabled {
            self.send(setting::STREAMING_ENABLED, 1)?;
            self.enabled = true;
        }
        let mut conn = self.conn.try_clone()?;
        abort.register(conn.as_ref());
        conn.set_read_timeout(Some(self.timing.stall))?;
        Ok(SpyStream {
            conn,
            label: self.label.clone(),
            entry: self.entry.clone(),
            dropped: self.dropped.clone(),
            sequence: Sequence::default(),
            body: vec![0u8; 64 << 10],
            window: Window {
                length: self.timing.window,
                ..Window::default()
            },
            format: self.format,
            size: format::bytes_per_sample(self.format),
        })
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        self.conn.shutdown();
    }

    fn rate_hz(&self) -> Hz {
        self.rate
    }

    fn centre_hz(&self) -> Hz {
        self.centre
    }

    fn sample_type(&self) -> SampleType {
        self.sample
    }

    fn capabilities(&self) -> Capabilities {
        caps_for(&self.label, &lock(&self.entry).info)
    }
}

impl Drop for SpyRadio {
    fn drop(&mut self) {
        self.close();
    }
}

/// The read loop of one SpyServer connection. It reads the IQ bodies of the stream's format straight into the ring; `body`
/// takes the other bodies, and IQ that the ring has no room for.
pub(crate) struct SpyStream {
    conn: Box<dyn Conn>,
    label: String,
    entry: Shared,
    dropped: Arc<Dropped>,
    sequence: Sequence,
    body: Vec<u8>,
    window: Window,
    format: u32,
    size: usize,
}

impl SpyStream {
    /// Reads an IQ body of `n` bytes into the sink and returns `n`. A part of a sample at its end is dropped.
    fn receive_iq(&mut self, n: usize, sink: &mut Sink) -> Result<usize, DriverError> {
        sink.receive_exact(n, &mut self.body, |dst| self.conn.read(dst))
            .map_err(closed)?;
        Ok(n)
    }

    /// Fills the IQ messages the server dropped before the one with header `h`: zero samples in their place, counted as
    /// dropped. A gap of more than one second of samples is a restart of the server's stream and is not filled.
    fn fill_dropped(&mut self, h: &Header, sink: &mut Sink) {
        let samples = h.body_size as usize / self.size;
        let rate = lock(&self.entry).rate_hz;
        let n = self.sequence.iq(h.sequence, samples, rate);
        if n == 0 {
            return;
        }
        // The zero of an unsigned 8-bit part is 127.5: 128 is the nearest.
        let zero = if self.format == format::UINT8 {
            0x80
        } else {
            0
        };
        sink.write_with(n, |dst, _| {
            for b in dst {
                b.write(zero);
            }
        });
        self.dropped.add(n as u64);
    }

    fn client_sync(&mut self, n: usize, ctx: &StreamCtx) {
        let s = ClientSync::parse(&self.body[..n]);
        let was = {
            let mut e = lock(&self.entry);
            let was = e.info.sync.map(|o| o.can_control);
            if was.is_some_and(|w| w != s.can_control) {
                e.control_changes += 1;
            }
            e.info.sync = Some(s);
            was
        };
        if was.is_some_and(|w| w != s.can_control) {
            ctx.notices.log(if s.can_control {
                "SpyServer: now in control"
            } else {
                "SpyServer: now read only"
            });
        }
    }

    /// Counts an IQ message into the throughput window. The entry is locked only when the digital gain changes and at the
    /// end of a window.
    fn count(&mut self, flags: u32, bytes: usize, samples: usize, ctx: &StreamCtx) {
        let now = Instant::now();
        let w = &mut self.window;
        if w.flags != Some(flags) {
            w.flags = Some(flags);
            lock(&self.entry).digital_gain_db = Some(f64::from(flags));
        }
        let Some(start) = w.start else {
            w.start = Some(now);
            return;
        };
        w.bytes += bytes as u64;
        w.samples += samples as u64;
        let secs = now.duration_since(start).as_secs_f64();
        if secs < w.length.as_secs_f64() {
            return;
        }
        let mut e = lock(&self.entry);
        if e.rate_hz != w.rate_hz {
            // A new rate: the window starts again.
            (w.rate_hz, w.low_windows) = (e.rate_hz, 0);
            (w.start, w.bytes, w.samples) = (Some(now), 0, 0);
            return;
        }
        let achieved = w.samples as f64 / secs;
        let below = e.rate_hz > 0.0 && achieved < 0.95 * e.rate_hz;
        w.low_windows = if below { w.low_windows + 1 } else { 0 };
        let low = w.low_windows >= 2;
        e.last = Some(Throughput {
            mbit_s: w.bytes as f64 * 8.0 / secs / 1e6,
            achieved_hz: achieved,
            low,
        });
        (w.start, w.bytes, w.samples) = (Some(now), 0, 0);
        if low && !w.low_warned {
            w.low_warned = true;
            ctx.notices.warn(format!(
                "SpyServer {}: {} Msps arrive of {} Msps: the network or the server is the limit (8-bit IQ needs 16 Mbit/s \
                 per Msps)",
                self.label,
                sdrkit_driver_core::fmt_m(achieved),
                sdrkit_driver_core::fmt_m(e.rate_hz),
            ));
        }
        if !below {
            w.low_warned = false;
        }
    }
}

/// The throughput window of a stream.
#[derive(Debug, Default)]
struct Window {
    length: Duration,
    start: Option<Instant>,
    bytes: u64,
    samples: u64,
    rate_hz: Hz,
    flags: Option<u32>,
    low_windows: u32,
    low_warned: bool,
}

impl RadioStream for SpyStream {
    fn run(&mut self, sink: &mut Sink, ctx: &StreamCtx) -> Result<(), DriverError> {
        loop {
            if ctx.stopping() || sink.closed() {
                return Ok(());
            }
            let read = read_header(self.conn.as_mut()).and_then(|h| {
                let n = if proto::iq_format_of(h.msg_type) == Some(self.format) {
                    self.fill_dropped(&h, sink);
                    self.receive_iq(h.body_size as usize, sink)?
                } else {
                    self.sequence.other();
                    read_body(self.conn.as_mut(), &h, &mut self.body)?
                };
                Ok((h, n))
            });
            let (h, n) = match read {
                Ok(v) => v,
                Err(_) if ctx.stopping() => return Ok(()),
                Err(e) => return Err(e),
            };
            if proto::iq_format_of(h.msg_type) == Some(self.format) {
                let whole = n - n % self.size;
                self.count(h.flags, whole, whole / self.size, ctx);
            } else if h.msg_type == msg::CLIENT_SYNC {
                self.client_sync(n, ctx);
            }
            // Anything else (PONG, READ_SETTING, audio, FFT, a second DEVICE_INFO, unknown) was read in full: dropped.
        }
    }
}
