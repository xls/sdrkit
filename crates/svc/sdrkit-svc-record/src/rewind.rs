//! The rewind buffer: the last seconds of raw samples kept in memory as signed 8-bit pairs, saved on request as "the last N
//! seconds, then M more" to a SigMF recording. An optional second history keeps the centre of the band for longer: the
//! average of every `long_factor` samples.
//!
//! Standard: SigMF specification v1.0.0 (datatype `ci8`, annotations).

use crate::error::RecordError;
use crate::format::{iq_name, iso_utc, stamp, IqSample};
use crate::sigmf;
use crate::spool;
use sdrkit_core::{Complex32, Hz, Storage, StorageError, StorePath};
use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::Arc;

/// Chunks queued for a save beyond those of its window.
const SAVE_ROOM: usize = 8;

/// How much the buffer keeps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RewindSpec {
    /// Memory for the full-rate history, in bytes (two bytes a sample).
    pub budget_bytes: usize,
    /// Samples a chunk holds.
    pub chunk_samples: usize,
    /// Average every this many samples into the centre history. 0 or 1 turns it off.
    pub long_factor: u32,
    /// Memory for the centre history, in bytes.
    pub long_budget_bytes: usize,
}

/// A save that the control side requests.
#[derive(Clone, Debug)]
pub(crate) struct SaveOrder {
    pub id: u64,
    pub folder: StorePath,
    pub prefix: String,
    pub pre_s: f64,
    pub post_s: f64,
    pub long: bool,
    pub reason: String,
    pub utc_offset_min: i32,
    pub now_unix: f64,
}

/// What a save reports.
#[derive(Debug)]
pub(crate) enum SaveNote {
    /// The data file of save `id` is open.
    Opened { id: u64, path: StorePath },
    /// Save `id` is complete. `gaps` counts the samples missing inside the file.
    Done {
        id: u64,
        path: StorePath,
        long: Option<StorePath>,
        samples: u64,
        gaps: u64,
    },
    /// Save `id` failed on `path`. Its data file keeps what came before.
    Failed {
        id: u64,
        path: StorePath,
        error: StorageError,
    },
}

/// The figures of the buffer, shared without a lock.
#[derive(Debug, Default)]
pub(crate) struct RewindLive {
    pub stop: AtomicBool,
    pub held: AtomicU64,
    pub capacity: AtomicU64,
    pub rate_bits: AtomicU64,
}

impl RewindLive {
    /// The sample rate, 0 before the first format.
    pub(crate) fn rate(&self) -> f64 {
        f64::from_bits(self.rate_bits.load(Ordering::Relaxed))
    }
}

/// The control side of the buffer: it sends save orders and reads the notes of the saves.
pub(crate) struct RewindControl {
    pub orders: Sender<SaveOrder>,
    pub notes: Receiver<SaveNote>,
    pub live: Arc<RewindLive>,
}

/// What a save's writer receives.
enum Piece {
    /// Samples `from..to` of a chunk (0 the full rate, 1 the centre history).
    Data {
        stream: u8,
        chunk: Arc<Chunk>,
        from: usize,
        to: usize,
    },
    /// The end.
    End,
}

struct ActiveSave {
    tx: SyncSender<Piece>,
    next: u64,
    end: u64,
}

/// The feed of the rewind buffer. The engine passes it the samples of the radio. [`RewindFeed::push`] never waits on storage and
/// allocates nothing, except a new chunk while a save still holds the one that leaves the history.
pub struct RewindFeed {
    spec: RewindSpec,
    storage: Arc<dyn Storage>,
    rate: Hz,
    centre: Hz,
    stack_id: String,
    ring: Option<Ring>,
    long: Option<Ring>,
    acc: Complex32,
    acc_n: u32,
    saves: Vec<ActiveSave>,
    orders: Receiver<SaveOrder>,
    notes: Sender<SaveNote>,
    live: Arc<RewindLive>,
}

impl std::fmt::Debug for RewindFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RewindFeed")
            .field("rate", &self.rate)
            .field("centre", &self.centre)
            .finish_non_exhaustive()
    }
}

/// Makes a rewind buffer on `storage`. It holds nothing until [`RewindFeed::configure`].
pub(crate) fn start(spec: RewindSpec, storage: Arc<dyn Storage>) -> (RewindFeed, RewindControl) {
    let (otx, orx) = mpsc::channel();
    let (ntx, nrx) = mpsc::channel();
    let live = Arc::new(RewindLive::default());
    (
        RewindFeed {
            spec,
            storage,
            rate: 0.0,
            centre: 0.0,
            stack_id: String::new(),
            ring: None,
            long: None,
            acc: Complex32::new(0.0, 0.0),
            acc_n: 0,
            saves: Vec::new(),
            orders: orx,
            notes: ntx,
            live: live.clone(),
        },
        RewindControl {
            orders: otx,
            notes: nrx,
            live,
        },
    )
}

fn q8(v: f32) -> u8 {
    (v * 128.0).round().clamp(-128.0, 127.0) as i8 as u8
}

impl RewindFeed {
    /// True once the control side has turned the buffer off. The engine drops it then.
    pub fn is_ended(&self) -> bool {
        self.live.stop.load(Ordering::Relaxed)
    }

    /// Sets the rate, the centre and the stack id (named in saves), and takes the memory of the histories. An earlier history
    /// is emptied and saves in progress end.
    pub fn configure(&mut self, rate_hz: Hz, centre_hz: Hz, stack_id: &str) {
        self.end_saves();
        self.rate = rate_hz;
        self.centre = centre_hz;
        stack_id.clone_into(&mut self.stack_id);
        let ring = Ring::new(self.spec.budget_bytes, self.spec.chunk_samples);
        self.live
            .capacity
            .store((ring.cap * ring.chunk) as u64, Ordering::Relaxed);
        self.ring = Some(ring);
        self.long = (self.spec.long_factor > 1).then(|| {
            Ring::new(
                self.spec.long_budget_bytes,
                self.spec.chunk_samples.min(4096),
            )
        });
        self.acc = Complex32::new(0.0, 0.0);
        self.acc_n = 0;
        self.live
            .rate_bits
            .store(rate_hz.to_bits(), Ordering::Relaxed);
        self.live.held.store(0, Ordering::Relaxed);
    }

    /// The radio moved: the history is emptied and saves in progress end with what they have.
    pub fn retuned(&mut self, centre_hz: Hz) {
        self.end_saves();
        self.centre = centre_hz;
        if let Some(r) = &mut self.ring {
            r.clear();
        }
        if let Some(r) = &mut self.long {
            r.clear();
        }
        self.acc = Complex32::new(0.0, 0.0);
        self.acc_n = 0;
        self.live.held.store(0, Ordering::Relaxed);
    }

    /// Adds samples, and starts the saves that the control side requested.
    pub fn push(&mut self, iq: &[Complex32]) {
        while let Ok(order) = self.orders.try_recv() {
            self.save(&order);
        }
        let Some(ring) = &mut self.ring else { return };
        let saves = &mut self.saves;
        ring.push_iq(iq, |done| deliver(saves, 0, done));
        self.live.held.store(ring.held(), Ordering::Relaxed);
        if let Some(long) = &mut self.long {
            let factor = self.spec.long_factor;
            for &s in iq {
                self.acc += s;
                self.acc_n += 1;
                if self.acc_n == factor {
                    let a = self.acc / factor as f32;
                    self.acc = Complex32::new(0.0, 0.0);
                    self.acc_n = 0;
                    long.push(q8(a.re), q8(a.im));
                }
            }
        }
    }

    /// Ends every save in progress with what it has, after it gets the chunk being filled.
    fn end_saves(&mut self) {
        if let Some(r) = &mut self.ring {
            if r.cur.len > 0 && !self.saves.is_empty() {
                let done = r.complete();
                deliver(&mut self.saves, 0, &done);
            }
        }
        for s in self.saves.drain(..) {
            let _ = s.tx.send(Piece::End);
        }
    }

    /// Starts save `order`: the last `pre_s` seconds (as many as are held) and the next `post_s`. The chunks are shared with
    /// the save's own thread, which writes them, so the buffer goes on.
    fn save(&mut self, order: &SaveOrder) {
        let fail = |error| SaveNote::Failed {
            id: order.id,
            path: order.folder.clone(),
            error,
        };
        let Some(ring) = &self.ring else {
            let _ = self.notes.send(fail(StorageError::NotFound));
            return;
        };
        let rate = self.rate;
        let now = ring.next;
        let want = (order.pre_s.max(0.0) * rate).round() as u64;
        let start = now.saturating_sub(want).max(ring.oldest());
        let end = now + (order.post_s.max(0.0) * rate).round() as u64;
        let long_rate = self
            .long
            .as_ref()
            .filter(|_| order.long)
            .map(|_| rate / f64::from(self.spec.long_factor));
        let post_chunks = ((end - now) as usize).div_ceil(ring.chunk) + 1;
        let room = ring.full.len()
            + self.long.as_ref().map_or(0, |l| l.full.len())
            + post_chunks
            + SAVE_ROOM;
        let (tx, rx) = mpsc::sync_channel(room);
        let job = SaveJob {
            id: order.id,
            folder: order.folder.clone(),
            name: iq_name(
                &order.prefix,
                &self.stack_id,
                self.centre,
                rate,
                &stamp(
                    order.now_unix - (now - start) as f64 / rate,
                    order.utc_offset_min,
                ),
            ),
            long_name: long_rate.map(|lr| {
                let held = self.long.as_ref().map_or(0, Ring::held);
                iq_name(
                    &format!("{}-centre", order.prefix),
                    &self.stack_id,
                    self.centre,
                    lr,
                    &stamp(order.now_unix - held as f64 / lr, order.utc_offset_min),
                )
            }),
            stack_id: self.stack_id.clone(),
            centre: self.centre,
            rate,
            start_unix: order.now_unix - (now - start) as f64 / rate,
            trigger_at: now - start,
            first: start,
            reason: order.reason.clone(),
            long_rate,
        };
        let storage = self.storage.clone();
        let notes = self.notes.clone();
        let spawned = std::thread::Builder::new()
            .name("sdrkit-rewind-save".into())
            .spawn(move || write_save(&*storage, job, &rx, &notes));
        if let Err(e) = spawned {
            let _ = self.notes.send(fail(StorageError::Io { kind: e.kind() }));
            return;
        }
        for c in &ring.full {
            let from = start.max(c.first);
            let to = now.min(c.end());
            if to > from {
                let _ = tx.send(Piece::Data {
                    stream: 0,
                    chunk: c.clone(),
                    from: (from - c.first) as usize,
                    to: (to - c.first) as usize,
                });
            }
        }
        if long_rate.is_some() {
            if let Some(l) = &mut self.long {
                if l.cur.len > 0 {
                    l.complete();
                }
                for c in &l.full {
                    let _ = tx.send(Piece::Data {
                        stream: 1,
                        chunk: c.clone(),
                        from: 0,
                        to: c.len,
                    });
                }
            }
        }
        let next = start.max(ring.cur.first);
        if next >= end && ring.cur.first >= end {
            let _ = tx.send(Piece::End);
        } else {
            self.saves.push(ActiveSave { tx, next, end });
        }
    }
}

impl Drop for RewindFeed {
    fn drop(&mut self) {
        self.end_saves();
    }
}

/// Sends the part of a completed chunk that the saves in their post-roll want. A send fails only when the save's thread has
/// ended, and then the piece has nowhere to go.
fn deliver(saves: &mut Vec<ActiveSave>, stream: u8, done: &Arc<Chunk>) {
    saves.retain_mut(|s| {
        let from = s.next.max(done.first);
        let to = s.end.min(done.end());
        if to > from {
            let _ = s.tx.send(Piece::Data {
                stream,
                chunk: done.clone(),
                from: (from - done.first) as usize,
                to: (to - done.first) as usize,
            });
            s.next = to;
        }
        if s.next >= s.end {
            let _ = s.tx.send(Piece::End);
            return false;
        }
        true
    });
}

struct SaveJob {
    id: u64,
    folder: StorePath,
    name: String,
    long_name: Option<String>,
    stack_id: String,
    centre: Hz,
    rate: Hz,
    start_unix: f64,
    trigger_at: u64,
    first: u64,
    reason: String,
    long_rate: Option<Hz>,
}

/// One data file of a save.
struct Out {
    path: StorePath,
    w: Box<dyn sdrkit_core::storage::StoreWriter>,
    samples: u64,
}

fn open_out(
    storage: &dyn Storage,
    folder: &StorePath,
    name: &str,
) -> Result<Out, (StorePath, StorageError)> {
    let path = folder
        .join(&format!("{name}.sigmf-data"))
        .map_err(|e| (folder.clone(), StorageError::InvalidPath(e)))?;
    let (path, w) = spool::create(storage, &path)?;
    Ok(Out {
        path,
        w,
        samples: 0,
    })
}

/// Syncs `o` and writes its metadata.
fn finish_out(
    storage: &dyn Storage,
    o: &mut Out,
    meta: &sigmf::Meta,
) -> Result<(), (StorePath, StorageError)> {
    o.w.sync().map_err(|e| (o.path.clone(), e))?;
    let m = o
        .path
        .with_extension("sigmf-meta")
        .map_err(|e| (o.path.clone(), StorageError::InvalidPath(e)))?;
    storage.write(&m, &sigmf::write(meta)).map_err(|e| (m, e))
}

/// The thread of one save: it writes the shared chunks as they arrive, then the metadata.
fn write_save(storage: &dyn Storage, job: SaveJob, rx: &Receiver<Piece>, notes: &Sender<SaveNote>) {
    let failed = |(path, error): (StorePath, StorageError)| {
        let _ = notes.send(SaveNote::Failed {
            id: job.id,
            path,
            error,
        });
    };
    let mut main = match open_out(storage, &job.folder, &job.name) {
        Ok(o) => o,
        Err(e) => return failed(e),
    };
    let _ = notes.send(SaveNote::Opened {
        id: job.id,
        path: main.path.clone(),
    });
    let mut long: Option<Out> = None;
    let mut gaps = 0u64;
    let mut expect = job.first;
    let mut error: Option<(StorePath, StorageError)> = None;
    while let Ok(piece) = rx.recv() {
        match piece {
            Piece::Data {
                stream,
                chunk,
                from,
                to,
            } => {
                if error.is_some() {
                    continue;
                }
                let out = if stream == 0 {
                    let first = chunk.first + from as u64;
                    gaps += first.saturating_sub(expect);
                    expect = chunk.first + to as u64;
                    &mut main
                } else {
                    if long.is_none() {
                        let name = job.long_name.as_deref().unwrap_or("rewind-centre");
                        match open_out(storage, &job.folder, name) {
                            Ok(o) => long = Some(o),
                            Err(e) => {
                                error = Some(e);
                                continue;
                            }
                        }
                    }
                    let Some(l) = long.as_mut() else { continue };
                    l
                };
                if let Err(e) = out.w.write_all(&chunk.data[2 * from..2 * to]) {
                    error = Some((out.path.clone(), e.into()));
                    continue;
                }
                out.samples += (to - from) as u64;
            }
            Piece::End => break,
        }
    }
    let post = main.samples.saturating_sub(job.trigger_at);
    let at = job.trigger_at.min(main.samples);
    let meta = |rate: Hz, start: f64, annotation| sigmf::Meta {
        datatype: IqSample::Cs8.sigmf_datatype().into(),
        rate_hz: rate,
        centre_hz: job.centre,
        datetime: iso_utc(start),
        description: job.stack_id.clone(),
        annotation,
        gaps: Vec::new(),
    };
    let r = finish_out(
        storage,
        &mut main,
        &meta(
            job.rate,
            job.start_unix,
            Some((at, post, job.reason.clone())),
        ),
    );
    if let Some(l) = &mut long {
        let lr = job.long_rate.unwrap_or(job.rate);
        let start = job.start_unix - l.samples as f64 / lr;
        if let Err(e) = finish_out(storage, l, &meta(lr, start, None)) {
            error.get_or_insert(e);
        }
    }
    if let Err(e) = r {
        error.get_or_insert(e);
    }
    match error {
        Some(e) => failed(e),
        None => {
            let _ = notes.send(SaveNote::Done {
                id: job.id,
                path: main.path,
                long: long.map(|l| l.path),
                samples: main.samples,
                gaps,
            });
        }
    }
}

/// Starts save `order` from the control side. The feed picks it up at its next push.
pub(crate) fn order(control: &RewindControl, order: SaveOrder) -> Result<(), RecordError> {
    if control.live.rate() <= 0.0 || control.live.held.load(Ordering::Relaxed) == 0 {
        return Err(RecordError::NothingHeld);
    }
    control
        .orders
        .send(order)
        .map_err(|_| RecordError::FeedGone)
}

/// A chunk of samples in `ci8`.
#[derive(Debug)]
struct Chunk {
    /// The absolute index of its first sample.
    first: u64,
    /// Samples in it.
    len: usize,
    data: Box<[u8]>,
}

impl Chunk {
    fn new(samples: usize) -> Arc<Chunk> {
        Arc::new(Chunk {
            first: 0,
            len: 0,
            data: vec![0u8; samples * 2].into_boxed_slice(),
        })
    }
    fn end(&self) -> u64 {
        self.first + self.len as u64
    }
}

/// One history: full chunks oldest first, the chunk being filled, and spare chunks.
struct Ring {
    chunk: usize,
    cap: usize,
    full: VecDeque<Arc<Chunk>>,
    cur: Arc<Chunk>,
    free: Vec<Arc<Chunk>>,
    next: u64,
    allocated: usize,
}

impl Ring {
    fn new(budget_bytes: usize, chunk: usize) -> Ring {
        let chunk = chunk.max(16);
        let cap = (budget_bytes / (chunk * 2)).max(2);
        let free: Vec<_> = (0..cap).map(|_| Chunk::new(chunk)).collect();
        Ring {
            chunk,
            cap,
            full: VecDeque::with_capacity(cap + 1),
            cur: Chunk::new(chunk),
            free,
            next: 0,
            allocated: cap + 1,
        }
    }

    fn oldest(&self) -> u64 {
        self.full.front().map_or(self.cur.first, |c| c.first)
    }

    fn held(&self) -> u64 {
        self.next - self.oldest()
    }

    /// A chunk to fill next: a spare, the oldest full one (dropped and replaced when a writer still holds it), or a new one while
    /// the history grows.
    fn take(&mut self) -> Arc<Chunk> {
        if let Some(c) = self.free.pop() {
            return c;
        }
        if self.full.len() >= self.cap {
            if let Some(mut old) = self.full.pop_front() {
                if Arc::get_mut(&mut old).is_some() {
                    return old;
                }
                self.allocated -= 1; // the writer frees it
            }
        }
        self.allocated += 1;
        Chunk::new(self.chunk)
    }

    /// Appends samples, calling `done` with each chunk they complete.
    fn push_iq(&mut self, iq: &[Complex32], mut done: impl FnMut(&Arc<Chunk>)) {
        let mut rest = iq;
        while !rest.is_empty() {
            let next = self.next;
            let c = Arc::get_mut(&mut self.cur).expect("the chunk being filled is never shared");
            if c.len == 0 {
                c.first = next;
            }
            let n = (self.chunk - c.len).min(rest.len());
            let dst = &mut c.data[2 * c.len..2 * (c.len + n)];
            for (d, s) in dst.as_chunks_mut::<2>().0.iter_mut().zip(&rest[..n]) {
                d[0] = q8(s.re);
                d[1] = q8(s.im);
            }
            c.len += n;
            self.next += n as u64;
            rest = &rest[n..];
            if c.len == self.chunk {
                let d = self.complete();
                done(&d);
            }
        }
    }

    /// Appends one sample. Returns the chunk that it completed, if any.
    #[inline]
    fn push(&mut self, re: u8, im: u8) -> Option<Arc<Chunk>> {
        let c = Arc::get_mut(&mut self.cur).expect("the chunk being filled is never shared");
        if c.len == 0 {
            c.first = self.next;
        }
        c.data[2 * c.len] = re;
        c.data[2 * c.len + 1] = im;
        c.len += 1;
        self.next += 1;
        (c.len == self.chunk).then(|| self.complete())
    }

    /// Moves the chunk being filled into the history (also when it is not full) and returns it.
    fn complete(&mut self) -> Arc<Chunk> {
        let mut fresh = self.take();
        let f = Arc::get_mut(&mut fresh).expect("a taken chunk is not shared");
        f.len = 0;
        f.first = self.next;
        let done = std::mem::replace(&mut self.cur, fresh);
        self.full.push_back(done.clone());
        while self.full.len() > self.cap {
            if let Some(mut old) = self.full.pop_front() {
                if Arc::get_mut(&mut old).is_some() {
                    self.free.push(old);
                } else {
                    self.allocated -= 1;
                }
            }
        }
        done
    }

    /// Empties the history (after a retune).
    fn clear(&mut self) {
        while let Some(mut c) = self.full.pop_front() {
            if Arc::get_mut(&mut c).is_some() {
                self.free.push(c);
            } else {
                self.allocated -= 1;
            }
        }
        let c = Arc::get_mut(&mut self.cur).expect("the chunk being filled is never shared");
        c.len = 0;
        c.first = self.next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_holds_its_budget_and_recycles() {
        let mut r = Ring::new(64 * 2 * 4, 64);
        assert_eq!(r.cap, 4);
        for i in 0..64 * 10 {
            r.push(i as u8, 0);
        }
        assert_eq!(r.full.len(), 4);
        assert_eq!(r.held(), 64 * 4);
        assert_eq!(r.allocated, 5);
        assert_eq!(r.oldest(), 64 * 6);
        assert_eq!(r.full[0].data[0], (64 * 6) as u8);
        // A chunk held elsewhere is replaced, not overwritten.
        let held = r.full[0].clone();
        for i in 0..64 * 4 {
            r.push(i as u8, 1);
        }
        assert_eq!(held.first, 64 * 6);
        assert_eq!(held.data[0], (64 * 6) as u8);
        drop(held);
        assert!(r.allocated <= 6);
        r.clear();
        assert_eq!(r.held(), 0);
    }

    #[test]
    fn samples_quantise_to_8_bits() {
        assert_eq!(q8(1.0), 127);
        assert_eq!(q8(-1.0), 0x80);
    }
}
