//! The hub: the [`Feeder`] copies each block of the source into a pooled, reference-counted chunk and queues the chunk on one
//! bounded single-producer, single-consumer ring per client. A full ring drops the chunk for that client only and counts it.
//! The feeder never waits and never allocates after [`Hub::new`]; the one lock it touches, the list of new clients, it
//! takes with `try_lock` and only when a client waits. The [`Hub`] handle opens listeners, kicks clients and reads the
//! statistics.

use crate::client::{self, ClientEntry, ClientHandle, ClientShared};
use crate::control::Control;
use crate::format::supported;
use crate::listen::{self, Listen, ListenerEntry, ListenerInfo};
use crate::protocol::Protocol;
use crate::slice::Slice;
use crate::stats::{Event, Snapshot};
use crate::ServeError;
use rtrb::{Consumer, Producer, RingBuffer};
use sdrkit_core::net::Transport;
use sdrkit_driver_core::sample::size;
use sdrkit_graph::stream::SampleType;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::Thread;
use std::time::{Duration, Instant};

/// The most events kept for [`Hub::take_events`]; older ones are dropped.
const EVENTS_KEPT: usize = 1024;

/// How a hub queues and serves.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The time of signal that each client's ring holds, ms, at the rate and format given to [`Hub::new`].
    pub queue_ms: u32,
    /// A client whose loss runs on for this long, s, is disconnected. A loss run starts at the first dropped chunk. It ends
    /// when a chunk is queued on a ring at most half full, when a chunk is queued `queue_ms` after the last drop, or when
    /// the writer finds the ring empty.
    pub drop_after_s: f64,
    /// The most clients at once, over all listeners.
    pub max_clients: usize,
    /// The capacity of one chunk, bytes (at least 64). A larger block is split into several chunks. Set it to the usual block
    /// of the source.
    pub block_bytes: usize,
    /// How long one write to a client can wait before the writer checks for a stop, a kick or the loss limit.
    pub write_timeout: Duration,
    /// The samples a client's transmit ring holds, ms at the client's rate.
    pub tx_queue_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            queue_ms: 500,
            drop_after_s: 5.0,
            max_clients: 8,
            block_bytes: 64 << 10,
            write_timeout: Duration::from_millis(100),
            tx_queue_ms: 200,
        }
    }
}

/// A block of the source as the feeder gets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Block {
    /// The sample type of the bytes: Cu8, Cs8, Cs16 or Cf32.
    pub format: SampleType,
    /// The centre frequency of the capture, Hz.
    pub centre_hz: f64,
    /// The sample rate of the capture, Hz.
    pub rate_hz: f64,
    /// Samples the source lost just before this block.
    pub lost: u64,
}

/// What came between a chunk and the chunk before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    /// The chunk follows the one before without a break.
    None,
    /// The source lost this many samples before the chunk.
    Lost(u64),
    /// The format, centre or rate changed at the chunk.
    Retuned,
}

/// One block of samples, shared by every client that queued it.
#[derive(Debug)]
pub(crate) struct Chunk {
    /// Counts the chunks of the hub from 1; a writer sees its own drops as a jump.
    pub seq: u64,
    /// The index of the first sample in the source, lost samples counted.
    pub index: u64,
    /// The time of the first sample from the start of the hub, ns.
    pub time_ns: i64,
    pub format: SampleType,
    pub capture: Slice,
    pub gap: Gap,
    pub bytes: Vec<u8>,
}

/// A client as the feeder sees it.
pub(crate) struct Slot {
    pub tx: Producer<Arc<Chunk>>,
    pub shared: Arc<ClientShared>,
    pub writer: Thread,
}

/// The state the hub, the feeder and the threads share.
pub(crate) struct Inner {
    pub config: Config,
    /// The capacity of each client's ring, chunks.
    pub ring_chunks: usize,
    pub epoch: Instant,
    pub stop: AtomicBool,
    format: AtomicU8,
    centre_bits: AtomicU64,
    rate_bits: AtomicU64,
    /// New clients for the feeder. The listener prunes clients that left before the feeder took them.
    pub pending: Mutex<Vec<Slot>>,
    /// The length of `pending`, so that the feeder tries its lock only when a client waits.
    pub pending_len: AtomicUsize,
    pub retired: Mutex<Consumer<Slot>>,
    pub clients: Mutex<Vec<ClientEntry>>,
    pub listeners: Mutex<Vec<ListenerEntry>>,
    events: Mutex<VecDeque<Event>>,
    pub next_id: AtomicU64,
    pub blocks: AtomicU64,
    pub samples: AtomicU64,
    pub pool_misses: AtomicU64,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Inner {
    /// Nanoseconds since the hub was made, at least 1 (0 means "no time").
    pub fn now_ns(&self) -> u64 {
        u64::try_from(self.epoch.elapsed().as_nanos())
            .unwrap_or(u64::MAX)
            .max(1)
    }

    /// The format and the centre and rate of the capture now.
    pub fn capture(&self) -> (SampleType, Slice) {
        let f = crate::format::from_index(self.format.load(Ordering::Relaxed))
            .unwrap_or(SampleType::Cu8);
        let s = Slice {
            centre_hz: f64::from_bits(self.centre_bits.load(Ordering::Relaxed)),
            rate_hz: f64::from_bits(self.rate_bits.load(Ordering::Relaxed)),
        };
        (f, s)
    }

    fn set_capture(&self, format: SampleType, centre_hz: f64, rate_hz: f64) {
        self.format
            .store(crate::format::index(format), Ordering::Relaxed);
        self.centre_bits
            .store(centre_hz.to_bits(), Ordering::Relaxed);
        self.rate_bits.store(rate_hz.to_bits(), Ordering::Relaxed);
    }

    pub fn event(&self, e: Event) {
        let mut q = lock(&self.events);
        if q.len() >= EVENTS_KEPT {
            q.pop_front();
        }
        q.push_back(e);
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

/// The owner's handle on a hub: listeners, clients, statistics. Dropping it stops every listener and client and joins
/// their threads.
pub struct Hub {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Hub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hub")
            .field("ring_chunks", &self.inner.ring_chunks)
            .finish()
    }
}

impl Hub {
    /// A hub for a source that starts as `first`, and the feeder that its producer keeps. Each client's ring holds
    /// `queue_ms` of signal at the rate and format of `first`. The pool of chunks, made here, has room for every client's
    /// full ring, so one slow client never takes chunks from the others.
    ///
    /// # Errors
    ///
    /// [`ServeError::Format`] if the format of `first` is not Cu8, Cs8, Cs16 or Cf32.
    pub fn new(mut config: Config, first: &Block) -> Result<(Hub, Feeder), ServeError> {
        if !supported(first.format) {
            return Err(ServeError::Format(first.format));
        }
        config.block_bytes = config.block_bytes.max(64);
        config.max_clients = config.max_clients.max(1);
        let per_s = first.rate_hz.max(1.0) * size(first.format) as f64;
        let ring_chunks =
            ((f64::from(config.queue_ms) / 1000.0 * per_s / config.block_bytes as f64).ceil()
                as usize)
                .max(2);
        let n = config.max_clients;
        let (retire_tx, retire_rx) = RingBuffer::new(n);
        let pool_len = n * (ring_chunks + 2) + 2;
        let pool = (0..pool_len)
            .map(|_| {
                Arc::new(Chunk {
                    seq: 0,
                    index: 0,
                    time_ns: 0,
                    format: first.format,
                    capture: Slice {
                        centre_hz: 0.0,
                        rate_hz: 0.0,
                    },
                    gap: Gap::None,
                    bytes: Vec::with_capacity(config.block_bytes),
                })
            })
            .collect();
        let inner = Arc::new(Inner {
            config,
            ring_chunks,
            epoch: Instant::now(),
            stop: AtomicBool::new(false),
            format: AtomicU8::new(0),
            centre_bits: AtomicU64::new(0),
            rate_bits: AtomicU64::new(0),
            pending: Mutex::new(Vec::new()),
            pending_len: AtomicUsize::new(0),
            retired: Mutex::new(retire_rx),
            clients: Mutex::new(Vec::new()),
            listeners: Mutex::new(Vec::new()),
            events: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            blocks: AtomicU64::new(0),
            samples: AtomicU64::new(0),
            pool_misses: AtomicU64::new(0),
        });
        inner.set_capture(first.format, first.centre_hz, first.rate_hz);
        let feeder = Feeder {
            inner: inner.clone(),
            pool,
            cursor: 0,
            slots: Vec::with_capacity(n),
            retire: retire_tx,
            seq: 0,
            index: 0,
            secs: 0.0,
            last: (first.format, first.centre_hz, first.rate_hz),
        };
        Ok((Hub { inner }, feeder))
    }

    /// The capacity of each client's ring, chunks.
    pub fn ring_chunks(&self) -> usize {
        self.inner.ring_chunks
    }

    /// The format and the centre and rate of the capture, as of the last block.
    pub fn capture(&self) -> (SampleType, Slice) {
        self.inner.capture()
    }

    /// Listens as `listen` says, for clients of `protocol` whose requests go to `control`. One thread takes the connections.
    ///
    /// # Errors
    ///
    /// [`ServeError::Net`] if the transport cannot listen there, [`ServeError::Thread`] if the thread cannot start.
    pub fn listen(
        &self,
        transport: &Arc<dyn Transport>,
        listen: &Listen,
        protocol: Arc<dyn Protocol>,
        control: Arc<dyn Control>,
    ) -> Result<ListenerInfo, ServeError> {
        listen::start(&self.inner, transport, listen, protocol, control)
    }

    /// Stops the listener `id` and joins its thread. Its clients stay. False if there is no such listener.
    pub fn unlisten(&self, id: u64) -> bool {
        listen::stop(&self.inner, id)
    }

    /// Disconnects client `id`. False if there is no such client.
    pub fn kick(&self, id: u64) -> bool {
        client::kick(&self.inner, id)
    }

    /// A handle on client `id`, to read or set its slice.
    pub fn client(&self, id: u64) -> Option<ClientHandle> {
        client::handle(&self.inner, id)
    }

    /// The counters of the hub, its listeners and its clients now. It locks only the lists of listeners and clients, never
    /// anything the feeder uses.
    pub fn snapshot(&self) -> Snapshot {
        listen::housekeep(&self.inner);
        crate::stats::snapshot(&self.inner)
    }

    /// The events since the last call, oldest first.
    pub fn take_events(&self) -> Vec<Event> {
        lock(&self.inner.events).drain(..).collect()
    }

    /// Stops every listener and client and joins their threads. Safe to call twice.
    pub fn shutdown(&mut self) {
        self.inner.stop.store(true, Ordering::Release);
        listen::stop_all(&self.inner);
        client::stop_all(&self.inner);
        listen::housekeep(&self.inner);
    }
}

impl Drop for Hub {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The producer's side of a hub. Keep it on the thread that delivers the blocks.
pub struct Feeder {
    inner: Arc<Inner>,
    pool: Vec<Arc<Chunk>>,
    cursor: usize,
    slots: Vec<Slot>,
    retire: Producer<Slot>,
    seq: u64,
    /// The source's sample counter, lost samples counted.
    index: u64,
    /// The time of the next sample, s.
    secs: f64,
    last: (SampleType, f64, f64),
}

impl std::fmt::Debug for Feeder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Feeder")
            .field("clients", &self.slots.len())
            .field("pool", &self.pool.len())
            .finish()
    }
}

impl Feeder {
    /// Queues `bytes`, samples of `block.format`, for every client. A trailing part of a sample is ignored. A change of format,
    /// centre or rate, or a loss of the source, marks the first chunk as a gap. It never waits or allocates. A block of
    /// a format the hub does not carry is counted as a pool miss and dropped.
    pub fn push(&mut self, bytes: &[u8], block: &Block) {
        self.admit();
        if !supported(block.format) {
            self.inner.pool_misses.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let now = (block.format, block.centre_hz, block.rate_hz);
        let mut gap = if now != self.last {
            self.last = now;
            self.inner
                .set_capture(block.format, block.centre_hz, block.rate_hz);
            Gap::Retuned
        } else if block.lost > 0 {
            Gap::Lost(block.lost)
        } else {
            Gap::None
        };
        let ss = size(block.format);
        let whole = &bytes[..bytes.len() / ss * ss];
        let per_sample = 1.0 / block.rate_hz.max(1.0);
        self.index += block.lost;
        self.secs += block.lost as f64 * per_sample;
        self.inner.blocks.fetch_add(1, Ordering::Relaxed);
        self.inner
            .samples
            .fetch_add((whole.len() / ss) as u64, Ordering::Relaxed);
        let piece = (self.inner.config.block_bytes / ss).max(1) * ss;
        let capture = Slice {
            centre_hz: block.centre_hz,
            rate_hz: block.rate_hz,
        };
        for part in whole.chunks(piece) {
            let n = (part.len() / ss) as u64;
            self.seq += 1;
            let (index, time_ns) = (self.index, (self.secs * 1e9) as i64);
            self.index += n;
            self.secs += n as f64 * per_sample;
            let Some(i) = self.free_chunk() else {
                self.inner.pool_misses.fetch_add(1, Ordering::Relaxed);
                for s in &self.slots {
                    s.shared.dropped(n, &self.inner);
                }
                continue;
            };
            if let Some(c) = Arc::get_mut(&mut self.pool[i]) {
                c.seq = self.seq;
                c.index = index;
                c.time_ns = time_ns;
                c.format = block.format;
                c.capture = capture;
                c.gap = gap;
                c.bytes.clear();
                c.bytes.extend_from_slice(part);
            }
            gap = Gap::None;
            let chunk = &self.pool[i];
            for s in &mut self.slots {
                offer(s, chunk, n, &self.inner);
            }
        }
    }

    /// The clients the feeder queues for now.
    pub fn clients(&self) -> usize {
        self.slots.len()
    }

    /// Moves clients whose writer has gone to the retire ring, and takes waiting clients while there is room.
    fn admit(&mut self) {
        let mut i = 0;
        while i < self.slots.len() {
            if self.slots[i].tx.is_abandoned() && !self.retire.is_full() {
                let s = self.slots.swap_remove(i);
                let _ = self.retire.push(s);
            } else {
                i += 1;
            }
        }
        if self.inner.pending_len.load(Ordering::Acquire) == 0
            || self.slots.len() == self.slots.capacity()
        {
            return;
        }
        // The listener holds this lock only to add or prune a client; the feeder never waits for it.
        let Ok(mut p) = self.inner.pending.try_lock() else {
            return;
        };
        while self.slots.len() < self.slots.capacity() {
            match p.pop() {
                Some(s) => self.slots.push(s),
                None => break,
            }
        }
        self.inner.pending_len.store(p.len(), Ordering::Release);
    }

    /// A chunk that no client holds, from the cursor on.
    fn free_chunk(&mut self) -> Option<usize> {
        let n = self.pool.len();
        for k in 0..n {
            let i = (self.cursor + k) % n;
            if Arc::get_mut(&mut self.pool[i]).is_some() {
                self.cursor = (i + 1) % n;
                return Some(i);
            }
        }
        None
    }
}

/// Queues `chunk` of `n` samples on the ring of `s`, or counts it as dropped when the ring is full.
fn offer(s: &mut Slot, chunk: &Arc<Chunk>, n: u64, inner: &Inner) {
    if s.tx.is_abandoned() {
        return;
    }
    match s.tx.push(chunk.clone()) {
        Ok(()) => {
            let cap = s.tx.buffer().capacity();
            let fill = cap - s.tx.slots();
            s.shared.queued(fill, inner);
            s.writer.unpark();
        }
        Err(_) => s.shared.dropped(n, inner),
    }
}
