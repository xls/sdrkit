//! TV time-shift: the live transport stream written to a rolling buffer of segment files, indexed by PCR, played back with
//! pause, seek and a return to live.
//!
//! Standard: ISO/IEC 13818-1, 2.4.3.5 (program_clock_reference).

use crate::error::RecordError;
use crate::tsplay::{PlayStatus, ReadState, TsPlayer, TsStore};
use sdrkit_core::storage::{StoreReader, StoreWriter, WriteMode};
use sdrkit_core::{Storage, StorageError, StorePath};
use sdrkit_proto_mpegts::packet::{self, TS_SIZE};
use sdrkit_proto_mpegts::pcr::PCR_WRAP_S;
use sdrkit_proto_mpegts::stream::TsPacket;
use std::collections::VecDeque;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};

/// Packets in one block of the writer.
const BLOCK_PACKETS: usize = 512;
/// Index points kept: one every quarter second.
const POINT_S: f64 = 0.25;

/// The limits of the buffer.
#[derive(Clone, Debug)]
pub(crate) struct ShiftSpec {
    pub folder: StorePath,
    pub budget_bytes: u64,
    pub segment_bytes: u64,
    pub queue_bytes: u64,
}

#[derive(Default)]
struct Index {
    /// The first byte still held (the start of the oldest segment).
    start: u64,
    /// Bytes written.
    written: u64,
    /// (stream time, byte position), oldest first.
    points: VecDeque<(f64, u64)>,
    clock: Option<(u16, f64)>,
    error: Option<StorageError>,
}

struct Shared {
    index: Mutex<Index>,
    stop: AtomicBool,
    dropped: AtomicU64,
}

fn lock(m: &Mutex<Index>) -> MutexGuard<'_, Index> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The feed of the time-shift buffer. The engine passes it the packets of the stream. [`ShiftFeed::push`] keeps each packet
/// by its shared bytes, never waits on storage and allocates nothing.
pub struct ShiftFeed {
    session: String,
    tx: SyncSender<Vec<TsPacket>>,
    free: Receiver<Vec<TsPacket>>,
    block: Option<Vec<TsPacket>>,
    shared: Arc<Shared>,
    ended: bool,
}

impl std::fmt::Debug for ShiftFeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShiftFeed")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

/// The control side of the time-shift buffer: the player and the figures.
pub(crate) struct TimeShift {
    spec: ShiftSpec,
    storage: Arc<dyn Storage>,
    shared: Arc<Shared>,
    player: Option<TsPlayer<SegmentStore>>,
    wait_until: f64,
}

/// What the buffer shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ShiftStatus {
    pub play: PlayStatus,
    pub start_s: f64,
    pub live_s: f64,
    pub behind_s: f64,
    pub bytes: u64,
    pub dropped: u64,
    pub error: Option<StorageError>,
}

fn segment_path(folder: &StorePath, n: u64) -> Result<StorePath, StorageError> {
    folder
        .join(&format!("segment-{n}.ts"))
        .map_err(StorageError::InvalidPath)
}

/// Starts a buffer of `session` in `spec.folder` on `storage`. Segments left from an earlier run are removed.
pub(crate) fn start(
    spec: ShiftSpec,
    session: &str,
    storage: Arc<dyn Storage>,
) -> Result<(ShiftFeed, TimeShift), RecordError> {
    match storage.remove_tree(&spec.folder) {
        Ok(()) | Err(StorageError::NotFound) => {}
        Err(e) => return Err(RecordError::storage(&spec.folder, e)),
    }
    let shared = Arc::new(Shared {
        index: Mutex::new(Index::default()),
        stop: AtomicBool::new(false),
        dropped: AtomicU64::new(0),
    });
    let buffers = (spec.queue_bytes / (BLOCK_PACKETS * TS_SIZE) as u64).max(2) as usize;
    let (tx, rx) = mpsc::sync_channel::<Vec<TsPacket>>(buffers);
    let (ftx, frx) = mpsc::sync_channel::<Vec<TsPacket>>(buffers);
    for _ in 0..buffers {
        let _ = ftx.try_send(Vec::with_capacity(BLOCK_PACKETS));
    }
    let (s2, sh2, st2) = (spec.clone(), shared.clone(), storage.clone());
    std::thread::Builder::new()
        .name("sdrkit-timeshift".into())
        .spawn(move || write_segments(&s2, &*st2, &rx, &ftx, &sh2))
        .map_err(|e| RecordError::Thread { kind: e.kind() })?;
    Ok((
        ShiftFeed {
            session: session.into(),
            tx,
            free: frx,
            block: None,
            shared: shared.clone(),
            ended: false,
        },
        TimeShift {
            spec,
            storage,
            shared,
            player: None,
            wait_until: 0.0,
        },
    ))
}

fn segment_bytes(spec: &ShiftSpec) -> u64 {
    (spec.segment_bytes / TS_SIZE as u64).max(1) * TS_SIZE as u64
}

impl ShiftFeed {
    /// The session whose stream this buffers.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// True once the buffer has ended. The engine drops an ended feed.
    pub fn is_ended(&self) -> bool {
        self.ended
    }

    /// Adds one packet.
    pub fn push(&mut self, p: &TsPacket) {
        if self.ended {
            return;
        }
        if self.shared.stop.load(Ordering::Relaxed) {
            self.ended = true;
            return;
        }
        if self.block.is_none() {
            match self.free.try_recv() {
                Ok(b) => self.block = Some(b),
                Err(TryRecvError::Empty) => {
                    self.shared.dropped.fetch_add(1, Ordering::Relaxed);
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    self.ended = true;
                    return;
                }
            }
        }
        if let Some(b) = &mut self.block {
            b.push(p.clone());
            if b.len() == BLOCK_PACKETS {
                self.flush();
            }
        }
    }

    /// Hands the gathered packets to the writer now, so a quiet stream does not wait for a full block.
    pub fn flush(&mut self) {
        if let Some(b) = self.block.take_if(|b| !b.is_empty()) {
            self.ended |= self.tx.try_send(b).is_err();
        }
    }
}

impl Drop for ShiftFeed {
    fn drop(&mut self) {
        self.flush();
    }
}

impl TimeShift {
    /// The player, made at the first call once the buffer holds an indexed point. It starts live.
    fn player(&mut self) -> Option<&mut TsPlayer<SegmentStore>> {
        if self.player.is_none() && lock(&self.shared.index).points.back().is_some() {
            let store = SegmentStore {
                folder: self.spec.folder.clone(),
                storage: self.storage.clone(),
                seg: segment_bytes(&self.spec),
                shared: self.shared.clone(),
                open: None,
            };
            if let Ok(mut p) = TsPlayer::new(store) {
                if let Some((_, live)) = p.store().span() {
                    p.seek(live);
                }
                self.player = Some(p);
            }
        }
        self.player.as_mut()
    }

    /// Back to live: to the newest indexed point.
    pub(crate) fn go_live(&mut self) {
        self.wait_until = 0.0;
        if let Some(p) = self.player() {
            if let Some((_, live)) = p.store().span() {
                p.seek(live);
            }
            p.resume();
        }
    }

    /// To `behind_s` seconds behind live, limited to what is held.
    pub(crate) fn seek_behind(&mut self, behind_s: f64) {
        self.wait_until = 0.0;
        if let Some(p) = self.player() {
            if let Some((a, b)) = p.store().span() {
                p.seek((b - behind_s).clamp(a, b));
            }
        }
    }

    /// Pauses or resumes playback.
    pub(crate) fn set_paused(&mut self, paused: bool) {
        self.wait_until = 0.0;
        if let Some(p) = self.player() {
            if paused {
                p.pause();
            } else {
                p.resume();
            }
        }
    }

    /// The packets due at wall time `now`, at most `max`, appended to `out`. Before the time that the last call named, it
    /// returns nothing.
    pub(crate) fn read(&mut self, now: f64, out: &mut Vec<[u8; TS_SIZE]>, max: usize) -> ReadState {
        if now < self.wait_until {
            return ReadState::WaitUntil(self.wait_until);
        }
        let state = match self.player() {
            Some(p) => p.read(now, out, max),
            None => ReadState::End,
        };
        self.wait_until = match state {
            ReadState::WaitUntil(t) => t,
            ReadState::More | ReadState::Paused | ReadState::End => 0.0,
        };
        state
    }

    /// The figures.
    pub(crate) fn status(&mut self) -> ShiftStatus {
        let (play, span) = match self.player() {
            Some(p) => (p.status(), p.store().span()),
            None => (PlayStatus::default(), None),
        };
        let (start_s, live_s) = span.unwrap_or((0.0, 0.0));
        let ix = lock(&self.shared.index);
        ShiftStatus {
            behind_s: (live_s - play.position_s).max(0.0),
            play,
            start_s,
            live_s,
            bytes: ix.written - ix.start,
            dropped: self.shared.dropped.load(Ordering::Relaxed),
            error: ix.error,
        }
    }

    /// Ends the buffer and removes its segments.
    pub(crate) fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.player = None;
        // The segments go. A failure leaves them for the next start, which removes them.
        let _ = self.storage.remove_tree(&self.spec.folder);
    }
}

impl Drop for TimeShift {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The writer thread: it writes the blocks to segments, drops the oldest segment past the budget, and indexes the PCR of the
/// clock PID.
fn write_segments(
    spec: &ShiftSpec,
    storage: &dyn Storage,
    rx: &Receiver<Vec<TsPacket>>,
    free: &SyncSender<Vec<TsPacket>>,
    sh: &Shared,
) {
    let seg = segment_bytes(spec);
    let keep = (spec.budget_bytes / seg).max(2);
    let mut n = 0u64;
    let mut first_seg = 0u64;
    let mut out: Option<Box<dyn StoreWriter>> = None;
    let mut in_seg = 0u64;
    let mut total = 0u64;
    let mut last_raw: Option<f64> = None;
    let mut wraps = 0.0f64;
    let mut last_point = f64::NEG_INFINITY;
    let mut new_points: Vec<(f64, u64)> = Vec::with_capacity(BLOCK_PACKETS);
    let mut clock: Option<(u16, f64)> = None;
    let fail = |e: StorageError| lock(&sh.index).error = Some(e);
    while let Ok(mut block) = rx.recv() {
        if sh.stop.load(Ordering::Relaxed) {
            break;
        }
        new_points.clear();
        for p in &block {
            let p = p.bytes();
            if out.is_none() {
                match segment_path(&spec.folder, n)
                    .and_then(|path| storage.open_writer(&path, WriteMode::Truncate))
                {
                    Ok(w) => out = Some(w),
                    Err(e) => {
                        fail(e);
                        break;
                    }
                }
            }
            if let Some(pcr) = packet::parse(p).and_then(|x| {
                x.adaptation
                    .and_then(|a| a.pcr)
                    .map(|pcr| (x.header.pid, pcr))
            }) {
                let (pid, pcr) = pcr;
                let raw = packet::pcr_seconds(pcr);
                let (cpid, first) = *clock.get_or_insert((pid, raw));
                if pid == cpid {
                    if last_raw.is_some_and(|l| raw + PCR_WRAP_S / 2.0 < l) {
                        wraps += PCR_WRAP_S;
                    }
                    last_raw = Some(raw);
                    let t = raw + wraps - first;
                    if t - last_point >= POINT_S || t < last_point {
                        last_point = t;
                        new_points.push((t, total));
                    }
                }
            }
            if let Some(w) = &mut out {
                if let Err(e) = w.write_all(p) {
                    fail(e.into());
                }
            }
            in_seg += TS_SIZE as u64;
            total += TS_SIZE as u64;
            if in_seg >= seg {
                out = None;
                n += 1;
                in_seg = 0;
                while n + 1 - first_seg > keep {
                    // A segment that does not go now is overwritten when its number comes back.
                    if let Ok(path) = segment_path(&spec.folder, first_seg) {
                        let _ = storage.remove(&path);
                    }
                    first_seg += 1;
                }
            }
        }
        block.clear();
        let _ = free.try_send(block);
        let mut ix = lock(&sh.index);
        ix.written = total;
        ix.start = first_seg * seg;
        ix.clock = clock;
        ix.points.extend(new_points.iter().copied());
        while ix.points.front().is_some_and(|p| p.1 < ix.start) {
            ix.points.pop_front();
        }
    }
}

/// The time-shift buffer as a [`TsStore`].
pub(crate) struct SegmentStore {
    folder: StorePath,
    storage: Arc<dyn Storage>,
    seg: u64,
    shared: Arc<Shared>,
    open: Option<(u64, Box<dyn StoreReader>)>,
}

impl TsStore for SegmentStore {
    fn start(&mut self) -> u64 {
        lock(&self.shared.index).start
    }
    fn end(&mut self) -> u64 {
        lock(&self.shared.index).written
    }
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        let (start, end) = {
            let ix = lock(&self.shared.index);
            (ix.start, ix.written)
        };
        if pos < start {
            return Err(StorageError::NotFound.into());
        }
        let seg = self.seg;
        let mut done = 0;
        let mut at = pos;
        while done < buf.len() && at < end {
            let n = at / seg;
            if self.open.as_ref().is_none_or(|o| o.0 != n) {
                let path = segment_path(&self.folder, n)?;
                self.open = Some((n, self.storage.open_reader(&path)?));
            }
            let Some((_, f)) = self.open.as_mut() else {
                break;
            };
            let off = at - n * seg;
            let want = (buf.len() - done)
                .min((seg - off) as usize)
                .min((end - at) as usize);
            f.seek(SeekFrom::Start(off))?;
            let k = f.read(&mut buf[done..done + want])?;
            if k == 0 {
                break;
            }
            done += k;
            at += k as u64;
        }
        Ok(done)
    }
    fn first_pcr(&mut self) -> Option<(u16, f64)> {
        lock(&self.shared.index).clock
    }
    fn position_of(&mut self, t: f64) -> Option<u64> {
        let ix = lock(&self.shared.index);
        let front = *ix.points.front()?;
        let i = ix.points.partition_point(|p| p.0 <= t);
        Some(if i == 0 { front.1 } else { ix.points[i - 1].1 })
    }
    fn span(&mut self) -> Option<(f64, f64)> {
        let ix = lock(&self.shared.index);
        Some((ix.points.front()?.0, ix.points.back()?.0))
    }
}
