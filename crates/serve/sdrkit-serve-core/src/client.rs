//! One client: the counters it shares with the feeder and the snapshot, a writer thread that takes chunks from its ring,
//! converts them to the format of the client's outlet (or to its slice) and sends them there, and a reader thread that runs
//! the protocol's parser on the bytes the client sends.

use crate::control::{Answer, Control, Request};
use crate::format::{self, convert};
use crate::hub::{lock, Chunk, Gap, Inner, Slot};
use crate::outlet::{write_patient, FrameInfo, Outlet, OutletError};
use crate::protocol::{Context, Parser, Protocol};
use crate::slice::{Slice, SliceError, Slicer};
use crate::stats::{Event, Leave};
use rtrb::{Consumer, RingBuffer};
use sdrkit_core::net::{timed_out, Conn};
use sdrkit_driver_core::sample::size;
use sdrkit_graph::stream::SampleType;
use std::io::{ErrorKind, Read};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{JoinHandle, Thread};
use std::time::{Duration, SystemTime};

/// How long a reader waits for bytes before it checks for a stop.
const READ_POLL: Duration = Duration::from_millis(100);
/// How long an idle writer sleeps when the feeder does not wake it.
const WRITE_POLL: Duration = Duration::from_millis(20);
/// The format index of "no outlet".
const NO_WIRE: u8 = 255;

type OutletSlot = Mutex<Option<Option<Box<dyn Outlet>>>>;

/// The counters and flags of one client.
pub(crate) struct ClientShared {
    pub id: u64,
    pub listener: u64,
    pub peer: Option<SocketAddr>,
    pub protocol: &'static str,
    /// The format of the present outlet ([`format::index`]), [`NO_WIRE`] without one.
    pub wire: AtomicU8,
    pub since: SystemTime,
    pub capacity: usize,
    pub bytes: AtomicU64,
    pub samples: AtomicU64,
    pub dropped_samples: AtomicU64,
    pub dropped_chunks: AtomicU64,
    pub drop_events: AtomicU64,
    pub gaps: AtomicU64,
    pub requests: AtomicU64,
    pub refused: AtomicU64,
    pub fill: AtomicUsize,
    /// When the present loss run started (ns of the hub clock), 0 when there is none.
    pub loss_since: AtomicU64,
    /// When the last chunk was dropped (ns of the hub clock).
    pub last_drop: AtomicU64,
    pub kick: AtomicBool,
    pub hangup: AtomicBool,
    pub closed: AtomicBool,
    pub slice: Mutex<Option<Slice>>,
    /// The slice keeps the centre of the capture: the client has only a rate of its own.
    pub follow_centre: AtomicBool,
    pub slice_changed: AtomicBool,
    /// The client asked for a setting or set up a stream. Until then it is a probe: it gets no samples and is not
    /// counted.
    pub engaged: AtomicBool,
    /// The last request of each kind that was not carried out as asked, with its answer.
    pub not_done: Mutex<Vec<(Request, Answer)>>,
    /// A new outlet for the writer (`Some(None)` removes the outlet), and its flag.
    outlet: OutletSlot,
    outlet_changed: AtomicBool,
    /// The writer thread, to wake it.
    writer: Mutex<Option<Thread>>,
}

impl std::fmt::Debug for ClientShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientShared")
            .field("id", &self.id)
            .finish()
    }
}

impl ClientShared {
    /// Counts a chunk of `n` samples that the ring had no room for, and starts a loss run if none runs.
    pub fn dropped(&self, n: u64, inner: &Inner) {
        self.dropped_samples.fetch_add(n, Ordering::Relaxed);
        self.dropped_chunks.fetch_add(1, Ordering::Relaxed);
        let now = inner.now_ns();
        self.last_drop.store(now, Ordering::Relaxed);
        if self.loss_since.load(Ordering::Relaxed) == 0 {
            self.loss_since.store(now, Ordering::Relaxed);
            self.drop_events.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Notes the fill of the ring after a chunk was queued. A ring at most half full, or no drop for `queue_ms`, ends a
    /// loss run.
    pub fn queued(&self, fill: usize, inner: &Inner) {
        self.fill.store(fill, Ordering::Relaxed);
        if self.loss_since.load(Ordering::Relaxed) == 0 {
            return;
        }
        let quiet = u64::from(inner.config.queue_ms) * 1_000_000;
        let last = self.last_drop.load(Ordering::Relaxed);
        if 2 * fill <= self.capacity || inner.now_ns().saturating_sub(last) >= quiet {
            self.end_loss();
        }
    }

    fn end_loss(&self) {
        if self.loss_since.load(Ordering::Relaxed) != 0 {
            self.loss_since.store(0, Ordering::Relaxed);
        }
    }

    fn wake(&self) {
        if let Some(t) = lock(&self.writer).as_ref() {
            t.unpark();
        }
    }

    pub fn wire(&self) -> Option<SampleType> {
        format::from_index(self.wire.load(Ordering::Relaxed))
    }
}

/// A client as the [`Control`], its protocol and the owner see it: who it is, its slice of the capture, and its outlet.
#[derive(Clone)]
pub struct ClientHandle {
    shared: Arc<ClientShared>,
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ClientHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientHandle")
            .field("id", &self.shared.id)
            .field("peer", &self.shared.peer)
            .finish()
    }
}

impl ClientHandle {
    /// The client's id, unique in the hub.
    pub fn id(&self) -> u64 {
        self.shared.id
    }
    /// The id of the listener that took the client.
    pub fn listener(&self) -> u64 {
        self.shared.listener
    }
    /// The client's address, if the transport gives it.
    pub fn peer(&self) -> Option<SocketAddr> {
        self.shared.peer
    }
    /// The centre and rate of the capture now.
    pub fn capture(&self) -> Slice {
        self.inner.capture().1
    }
    /// The sample type of the source now.
    pub fn source_format(&self) -> SampleType {
        self.inner.capture().0
    }
    /// The client's slice of its own centre, or `None` when it gets the capture centre (the whole capture, or a lower
    /// rate of its own, see [`ClientHandle::rate_hz`]).
    pub fn slice(&self) -> Option<Slice> {
        if self.shared.follow_centre.load(Ordering::Acquire) {
            return None;
        }
        *lock(&self.shared.slice)
    }
    /// The client's own rate, from its slice or from [`ClientHandle::set_rate`]; `None` when it gets the capture rate.
    pub fn rate_hz(&self) -> Option<f64> {
        lock(&self.shared.slice).map(|s| s.rate_hz)
    }
    /// Gives the client a rate of its own (`Some`) around the centre of the capture, which it follows when the capture is
    /// tuned; `None` gives it the whole capture again. The writer takes it from its next chunk on.
    ///
    /// # Errors
    ///
    /// [`SliceError`] if the rate is not positive, is above the capture rate or has no resampler; the client then keeps
    /// what it had.
    pub fn set_rate(&self, rate_hz: Option<f64>) -> Result<(), SliceError> {
        let slice = match rate_hz {
            Some(r) => {
                let s = Slice {
                    centre_hz: self.capture().centre_hz,
                    rate_hz: r,
                };
                s.check(&self.capture())?;
                Some(s)
            }
            None => None,
        };
        *lock(&self.shared.slice) = slice;
        self.shared
            .follow_centre
            .store(slice.is_some(), Ordering::Release);
        self.shared.slice_changed.store(true, Ordering::Release);
        Ok(())
    }
    /// True once the client asked for a setting or set up a stream: it is counted and gets samples.
    pub fn engaged(&self) -> bool {
        self.shared.engaged.load(Ordering::Acquire)
    }
    /// Counts the client from now on, and tells the owner that it joined.
    pub(crate) fn engage(&self) {
        if !self.shared.engaged.swap(true, Ordering::AcqRel) {
            self.inner.event(Event::ClientJoined {
                client: self.shared.id,
                listener: self.shared.listener,
                peer: self.shared.peer,
            });
        }
    }
    /// Gives the client its own centre and rate inside the capture (`Some`), or the whole capture again (`None`). The writer
    /// takes it from its next chunk on. A slice that falls outside the capture after a retune gives zero samples until it
    /// fits again.
    ///
    /// # Errors
    ///
    /// [`SliceError`] if the slice does not fit the capture now or its resampler cannot be built; the slice then stays as
    /// it was.
    pub fn set_slice(&self, slice: Option<Slice>) -> Result<(), SliceError> {
        if let Some(s) = slice {
            s.check(&self.capture())?;
        }
        *lock(&self.shared.slice) = slice;
        self.shared.follow_centre.store(false, Ordering::Release);
        self.shared.slice_changed.store(true, Ordering::Release);
        Ok(())
    }
    /// Sends the client's stream to `outlet` from the next chunk on; `None` stops the stream. The writer drops the old
    /// outlet.
    pub fn set_outlet(&self, outlet: Option<Box<dyn Outlet>>) {
        if outlet.is_some() {
            self.engage();
        }
        *lock(&self.shared.outlet) = Some(outlet);
        self.shared.outlet_changed.store(true, Ordering::Release);
        self.shared.wake();
    }
    /// True once the client must stop: the hub stops, the client was kicked or hung up, or its writer ended.
    pub fn stopping(&self) -> bool {
        self.inner.stop.load(Ordering::Acquire)
            || self.shared.kick.load(Ordering::Acquire)
            || self.shared.hangup.load(Ordering::Acquire)
            || self.shared.closed.load(Ordering::Acquire)
    }
    pub(crate) fn hang_up(&self) {
        self.shared.hangup.store(true, Ordering::Release);
        self.shared.wake();
    }
    pub(crate) fn count_request(&self, refused: bool) {
        self.engage();
        self.shared.requests.fetch_add(1, Ordering::Relaxed);
        if refused {
            self.shared.refused.fetch_add(1, Ordering::Relaxed);
        }
    }
    /// Keeps `request` with its `answer` when the request was not carried out as asked: the last one of each kind.
    pub(crate) fn note_answer(&self, request: &Request, answer: &Answer) {
        let done = match answer {
            Answer::Accepted => true,
            Answer::Refused => false,
            Answer::Adjusted(got) => got == request,
        };
        let mut kept = lock(&self.shared.not_done);
        let same = |r: &Request| std::mem::discriminant(r) == std::mem::discriminant(request);
        kept.retain(|(r, _)| !same(r));
        if !done {
            kept.push((*request, *answer));
        }
    }
}

/// A client in the hub's list: its counters, a handle on its connection to shut it down, and its threads.
pub(crate) struct ClientEntry {
    pub shared: Arc<ClientShared>,
    pub conn: Box<dyn Conn>,
    pub writer: Option<JoinHandle<()>>,
    pub reader: Option<JoinHandle<()>>,
}

impl ClientEntry {
    pub fn finished(&self) -> bool {
        self.writer.as_ref().is_none_or(JoinHandle::is_finished)
            && self.reader.as_ref().is_none_or(JoinHandle::is_finished)
    }
    pub fn join(&mut self) {
        for h in [self.writer.take(), self.reader.take()]
            .into_iter()
            .flatten()
        {
            let _ = h.join();
        }
    }
}

/// Starts the threads of a new client on `conn` and puts its ring in the hub's list of clients waiting for the feeder. It
/// returns the entry for the hub's list of clients.
pub(crate) fn start(
    inner: &Arc<Inner>,
    conn: Box<dyn Conn>,
    listener: u64,
    protocol: &Arc<dyn Protocol>,
    control: &Arc<dyn Control>,
) -> std::io::Result<ClientEntry> {
    let (source, _) = inner.capture();
    let mut conn = conn;
    let _ = conn.set_write_timeout(Some(inner.config.write_timeout));
    let session = protocol.session(conn.as_ref(), source);
    let shared = Arc::new(ClientShared {
        id: inner.next_id(),
        listener,
        peer: conn.peer_addr(),
        protocol: protocol.name(),
        wire: AtomicU8::new(
            session
                .outlet
                .as_ref()
                .map_or(NO_WIRE, |o| format::index(o.format())),
        ),
        since: SystemTime::now(),
        capacity: inner.ring_chunks,
        bytes: AtomicU64::new(0),
        samples: AtomicU64::new(0),
        dropped_samples: AtomicU64::new(0),
        dropped_chunks: AtomicU64::new(0),
        drop_events: AtomicU64::new(0),
        gaps: AtomicU64::new(0),
        requests: AtomicU64::new(0),
        refused: AtomicU64::new(0),
        fill: AtomicUsize::new(0),
        loss_since: AtomicU64::new(0),
        last_drop: AtomicU64::new(0),
        kick: AtomicBool::new(false),
        hangup: AtomicBool::new(false),
        closed: AtomicBool::new(false),
        slice: Mutex::new(None),
        follow_centre: AtomicBool::new(false),
        slice_changed: AtomicBool::new(false),
        engaged: AtomicBool::new(false),
        not_done: Mutex::new(Vec::new()),
        outlet: Mutex::new(None),
        outlet_changed: AtomicBool::new(false),
        writer: Mutex::new(None),
    });
    let read_conn = conn.try_clone().map_err(std::io::Error::from)?;
    let keep_conn = conn.try_clone().map_err(std::io::Error::from)?;
    let (tx, rx) = RingBuffer::new(inner.ring_chunks);
    let id = shared.id;
    let w = Writer {
        inner: inner.clone(),
        shared: shared.clone(),
        conn,
        rx,
        outlet: session.outlet,
    };
    let greeting = session.greeting;
    let writer = std::thread::Builder::new()
        .name(format!("sdrkit-serve-w{id}"))
        .spawn(move || w.run(&greeting))?;
    let wake_writer = writer.thread().clone();
    *lock(&shared.writer) = Some(wake_writer.clone());
    let handle = ClientHandle {
        shared: shared.clone(),
        inner: inner.clone(),
    };
    // A protocol that streams from the connect on (rtl_tcp) has its client at once.
    if shared.wire.load(Ordering::Relaxed) != NO_WIRE {
        handle.engage();
    }
    let (parser, control, queue_ms) = (session.parser, control.clone(), inner.config.tx_queue_ms);
    let reader = std::thread::Builder::new()
        .name(format!("sdrkit-serve-r{id}"))
        .spawn(move || read_loop(&handle, read_conn, parser, control.as_ref(), queue_ms));
    let mut entry = ClientEntry {
        shared: shared.clone(),
        conn: keep_conn,
        writer: Some(writer),
        reader: None,
    };
    match reader {
        Ok(r) => entry.reader = Some(r),
        Err(e) => {
            shared.kick.store(true, Ordering::Release);
            entry.conn.shutdown();
            entry.join();
            return Err(e);
        }
    }
    let slot = Slot {
        tx,
        shared,
        writer: wake_writer,
    };
    let mut p = lock(&inner.pending);
    p.retain(|s| !s.tx.is_abandoned());
    p.push(slot);
    inner.pending_len.store(p.len(), Ordering::Release);
    Ok(entry)
}

struct Writer {
    inner: Arc<Inner>,
    shared: Arc<ClientShared>,
    conn: Box<dyn Conn>,
    rx: Consumer<Arc<Chunk>>,
    outlet: Option<Box<dyn Outlet>>,
}

impl Writer {
    fn run(mut self, greeting: &[u8]) {
        let reason = self.serve(greeting);
        while self.rx.pop().is_ok() {}
        self.outlet = None;
        self.conn.shutdown();
        self.shared.closed.store(true, Ordering::Release);
        if !self.shared.engaged.load(Ordering::Acquire) {
            return;
        }
        self.inner.event(Event::ClientLeft {
            client: self.shared.id,
            listener: self.shared.listener,
            peer: self.shared.peer,
            reason,
        });
    }

    /// Sends the greeting, then each chunk of the ring through the present outlet, until the client must stop.
    fn serve(&mut self, greeting: &[u8]) -> Leave {
        let (inner, s) = (self.inner.clone(), self.shared.clone());
        let reason = || must_stop(&inner, &s);
        let mut stop = || reason().is_some();
        if write_patient(self.conn.as_mut(), greeting, &mut stop).is_err() {
            return reason().unwrap_or(Leave::Closed);
        }
        let mut next_seq: Option<u64> = None;
        let mut next_index: Option<u64> = None;
        let mut slicer: Option<Slicer> = None;
        let mut want = *lock(&s.slice);
        let mut follow = s.follow_centre.load(Ordering::Acquire);
        let mut buf = Vec::new();
        let mut scratch = Vec::new();
        loop {
            if let Some(r) = reason() {
                return r;
            }
            if s.outlet_changed.swap(false, Ordering::Acquire) {
                if let Some(o) = lock(&s.outlet).take() {
                    self.outlet = o;
                    let w = self
                        .outlet
                        .as_ref()
                        .map_or(NO_WIRE, |o| format::index(o.format()));
                    s.wire.store(w, Ordering::Relaxed);
                }
            }
            let chunk = match self.rx.pop() {
                Ok(c) => c,
                Err(_) => {
                    s.end_loss();
                    std::thread::park_timeout(WRITE_POLL);
                    continue;
                }
            };
            s.fill.store(self.rx.slots(), Ordering::Relaxed);
            let lost = next_index.map_or(0, |i| chunk.index.saturating_sub(i));
            let gap = match chunk.gap {
                Gap::Retuned => Gap::Retuned,
                _ if lost > 0 => Gap::Lost(lost),
                _ if next_seq.is_some_and(|n| n != chunk.seq) => Gap::Lost(0),
                g => g,
            };
            if gap != Gap::None {
                s.gaps.fetch_add(1, Ordering::Relaxed);
            }
            next_seq = Some(chunk.seq + 1);
            next_index = Some(chunk.index + (chunk.bytes.len() / size(chunk.format)) as u64);
            let Some(outlet) = self.outlet.as_mut() else {
                continue;
            };
            let wire = outlet.format();
            if s.slice_changed.swap(false, Ordering::Acquire) {
                want = *lock(&s.slice);
                follow = s.follow_centre.load(Ordering::Acquire);
            }
            if let (Some(w), true) = (want.as_mut(), follow) {
                w.centre_hz = chunk.capture.centre_hz;
            }
            slicer = match (want, slicer.take()) {
                (None, _) => None,
                (Some(w), Some(mut sl)) => {
                    sl.update(w, chunk.capture);
                    Some(sl)
                }
                (Some(w), None) => Some(Slicer::new(w, chunk.capture)),
            };
            let (bytes, rate, centre): (&[u8], f64, f64) = if let Some(sl) = slicer.as_mut() {
                buf.clear();
                sl.process(chunk.format, &chunk.bytes, wire, &mut buf);
                let w = want.unwrap_or(chunk.capture);
                (&buf, w.rate_hz, w.centre_hz)
            } else if chunk.format == wire {
                (&chunk.bytes, chunk.capture.rate_hz, chunk.capture.centre_hz)
            } else {
                buf.clear();
                convert(chunk.format, &chunk.bytes, wire, &mut buf, &mut scratch);
                (&buf, chunk.capture.rate_hz, chunk.capture.centre_hz)
            };
            let info = FrameInfo {
                seq: chunk.seq,
                first_sample: chunk.index,
                time_ns: chunk.time_ns,
                rate_hz: rate,
                centre_hz: centre,
                gap,
            };
            match outlet.send(bytes, &info, &mut stop) {
                Ok(()) => {}
                Err(OutletError::Stopped) => return reason().unwrap_or(Leave::Closed),
                Err(OutletError::Closed) => return reason().unwrap_or(Leave::Closed),
            }
            s.bytes.fetch_add(bytes.len() as u64, Ordering::Relaxed);
            s.samples
                .fetch_add((bytes.len() / size(wire)) as u64, Ordering::Relaxed);
        }
    }
}

/// Why the writer of `s` must stop now, if it must.
fn must_stop(inner: &Inner, s: &ClientShared) -> Option<Leave> {
    if inner.stop.load(Ordering::Acquire) {
        return Some(Leave::Shutdown);
    }
    if s.kick.load(Ordering::Acquire) {
        return Some(Leave::Kicked);
    }
    if s.hangup.load(Ordering::Acquire) {
        return Some(Leave::Closed);
    }
    let since = s.loss_since.load(Ordering::Relaxed);
    let limit = (inner.config.drop_after_s.max(0.0) * 1e9) as u64;
    if since != 0 && inner.now_ns().saturating_sub(since) >= limit {
        return Some(Leave::TooSlow);
    }
    None
}

/// Reads the client's bytes until it hangs up or the client stops, and runs the parser on them.
fn read_loop(
    handle: &ClientHandle,
    mut conn: Box<dyn Conn>,
    mut parser: Box<dyn Parser>,
    control: &dyn Control,
    tx_queue_ms: u32,
) {
    let _ = conn.set_read_timeout(Some(READ_POLL));
    let mut reply = conn.try_clone().ok();
    let mut buf = [0u8; 4096];
    loop {
        if handle.stopping() {
            break;
        }
        let n = match conn.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if timed_out(&e) || e.kind() == ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        let Some(out) = reply.as_mut() else { break };
        let mut cx = Context {
            handle,
            control,
            conn: out.as_mut(),
            tx_queue_ms,
        };
        parser.feed(&buf[..n], &mut cx);
    }
    if let Some(out) = reply.as_mut() {
        let mut cx = Context {
            handle,
            control,
            conn: out.as_mut(),
            tx_queue_ms,
        };
        parser.closed(&mut cx);
    }
    handle.hang_up();
}

/// Disconnects client `id`.
pub(crate) fn kick(inner: &Inner, id: u64) -> bool {
    let clients = lock(&inner.clients);
    let Some(c) = clients.iter().find(|c| c.shared.id == id) else {
        return false;
    };
    c.shared.kick.store(true, Ordering::Release);
    c.conn.shutdown();
    c.shared.wake();
    true
}

/// A handle on client `id`.
pub(crate) fn handle(inner: &Arc<Inner>, id: u64) -> Option<ClientHandle> {
    lock(&inner.clients)
        .iter()
        .find(|c| c.shared.id == id && !c.shared.closed.load(Ordering::Acquire))
        .map(|c| ClientHandle {
            shared: c.shared.clone(),
            inner: inner.clone(),
        })
}

/// Stops every client and joins its threads. The hub's stop flag is set.
pub(crate) fn stop_all(inner: &Inner) {
    let mut all: Vec<ClientEntry> = lock(&inner.clients).drain(..).collect();
    for c in &all {
        c.conn.shutdown();
        c.shared.wake();
    }
    for c in &mut all {
        c.join();
    }
}
