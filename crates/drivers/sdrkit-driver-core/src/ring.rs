//! The ring between a radio's thread and the host: one writer, one reader, no locks. It holds native samples of one
//! [`SampleType`], as little-endian bytes.
//!
//! A live radio never waits: if the ring is full, it drops the newest samples and counts them. A file or a generator writes with
//! the blocking writes, which wait for room, so nothing is lost and the reader sets the pace.
//!
//! Each place where samples are missing from the stream (dropped by a full ring, or lost before the ring and reported with
//! [`RingWriter::mark_gap`]) is kept as a [`Gap`], so the reader can mark a discontinuity at the sample where it lies.

use crate::sample;
use sdrkit_core::Complex32;
use sdrkit_graph::stream::SampleType;
use std::io;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The most gaps the ring keeps for a reader that does not take them; older ones are forgotten.
const MAX_GAPS: usize = 256;

/// A place in the stream where samples are missing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gap {
    /// The samples from the next read to the first sample after the gap.
    pub at: usize,
    /// The samples missing there.
    pub lost: u64,
    /// The driver left the samples out on purpose (a retune that flushes the device, the time a half-duplex radio
    /// transmits): the reader marks the place, and nothing counts them as lost.
    pub deliberate: bool,
}

impl Gap {
    /// A gap at `at` where `lost` samples were lost.
    pub fn lost_at(at: usize, lost: u64) -> Gap {
        Gap {
            at,
            lost,
            deliberate: false,
        }
    }
}

/// The ring size the engine uses between a radio and the host: 2^24 samples.
pub const RING_CAPACITY: usize = 1 << 24;

/// Counters shared by both ends, in samples.
#[derive(Debug)]
pub struct RingStats {
    written: AtomicU64,
    read: AtomicU64,
    dropped: AtomicU64,
    drop_events: AtomicU64,
    skipped: AtomicU64,
    skip_events: AtomicU64,
    /// Nanoseconds after `epoch` of the last drop or skip, plus one (0: none yet).
    last_loss_ns: AtomicU64,
    epoch: Instant,
    closed: AtomicBool,
    capacity: usize,
    /// The gaps not yet taken: (samples written before the gap, samples missing, made on purpose).
    gaps: Mutex<VecDeque<(u64, u64, bool)>>,
    gap_count: AtomicU64,
    /// The reader removes the DC offset ([`crate::dc`]).
    dc_on: AtomicBool,
    /// Moves on at each restart of the DC estimate.
    dc_epoch: AtomicU64,
    /// The samples written before the last restart of the DC estimate.
    dc_from: AtomicU64,
    /// The retunes that the source began.
    moves_begun: AtomicU64,
    /// The number of the last retune whose first sample is known, and the samples written before that sample.
    moved: Mutex<(u64, u64)>,
}

/// The retunes of the source of a ring ([`RingStats::moves`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moves {
    /// The retunes that the source began.
    pub begun: u64,
    /// The number of the last retune whose first sample is known. Lower than `begun` while the radio settles.
    pub found: u64,
    /// The place in the ring ([`RingReader::position`]) of the first sample of retune `found`. It can lie ahead of the
    /// samples written.
    pub at: u64,
}

impl RingStats {
    fn new(capacity: usize) -> Self {
        RingStats {
            written: AtomicU64::new(0),
            read: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            drop_events: AtomicU64::new(0),
            skipped: AtomicU64::new(0),
            skip_events: AtomicU64::new(0),
            last_loss_ns: AtomicU64::new(0),
            epoch: Instant::now(),
            closed: AtomicBool::new(false),
            capacity,
            gaps: Mutex::new(VecDeque::with_capacity(MAX_GAPS)),
            gap_count: AtomicU64::new(0),
            dc_on: AtomicBool::new(false),
            dc_epoch: AtomicU64::new(0),
            dc_from: AtomicU64::new(0),
            moves_begun: AtomicU64::new(0),
            moved: Mutex::new((0, 0)),
        }
    }
    /// The source begins a retune of a radio that streams. Returns the number of the retune. The source says where the
    /// retune takes effect with [`RingStats::end_move`]; a source that calls neither has no samples of the frequency
    /// before in flight.
    pub fn begin_move(&self) -> u64 {
        self.moves_begun.fetch_add(1, Ordering::AcqRel) + 1
    }
    /// The first sample of retune `seq`, and of every retune before it, is the one after `at` samples written.
    pub fn end_move(&self, seq: u64, at: u64) {
        let mut m = self.moved.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if seq > m.0 {
            *m = (seq, at);
        }
    }
    /// The retunes of the source.
    pub fn moves(&self) -> Moves {
        // `begun` is read first: a retune that it counts is never taken as found by a number read later.
        let begun = self.moves_begun.load(Ordering::Acquire);
        let (found, at) = *self.moved.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        Moves { begun, found: found.min(begun), at }
    }
    /// Makes [`RingReader::read_iq`] remove the DC offset of the samples, or stop doing so.
    pub fn set_dc_removal(&self, on: bool) {
        self.dc_on.store(on, Ordering::Release);
    }
    /// Starts the DC estimate of the reader again from the next sample written: the offset changed (a retune, a gain).
    pub fn restart_dc(&self) {
        self.dc_from.store(self.written(), Ordering::Release);
        self.dc_epoch.fetch_add(1, Ordering::AcqRel);
    }
    /// Notes `lost` samples missing after the samples written so far, left out on purpose if `deliberate`. A gap at the
    /// place of the last one, of the same kind, adds to it.
    fn gap(&self, lost: u64, deliberate: bool) {
        let at = self.written();
        let mut g = self.gaps.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        match g.back_mut() {
            Some(last) if last.0 == at && last.2 == deliberate => last.1 += lost,
            _ => {
                if g.len() >= MAX_GAPS {
                    g.pop_front();
                }
                g.push_back((at, lost, deliberate));
            }
        }
        self.gap_count.store(g.len() as u64, Ordering::Release);
    }
    fn mark_loss(&self) {
        let ns = u64::try_from(self.epoch.elapsed().as_nanos()).unwrap_or(u64::MAX - 1);
        self.last_loss_ns
            .store(ns.saturating_add(1), Ordering::Relaxed);
    }
    /// Writes that lost samples because the ring was full.
    pub fn drop_events(&self) -> u64 {
        self.drop_events.load(Ordering::Relaxed)
    }
    /// Samples the reader skipped to catch up with a live radio ([`RingReader::skip_backlog`]).
    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }
    /// How many times the reader skipped ahead.
    pub fn skip_events(&self) -> u64 {
        self.skip_events.load(Ordering::Relaxed)
    }
    /// Everything lost on the reader's side: dropped plus skipped samples.
    pub fn cpu_lost(&self) -> u64 {
        self.dropped() + self.skipped()
    }
    /// How long ago the reader's side last lost samples, if it ever did.
    pub fn since_last_loss(&self) -> Option<Duration> {
        let v = self.last_loss_ns.load(Ordering::Relaxed);
        (v > 0).then(|| {
            self.epoch
                .elapsed()
                .saturating_sub(Duration::from_nanos(v - 1))
        })
    }
    /// Samples the radio wrote into the ring.
    pub fn written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }
    /// Samples lost because the reader fell behind.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
    /// The part of the ring that holds samples, from 0 to 1.
    pub fn fill(&self) -> f64 {
        let held = self
            .written()
            .saturating_sub(self.read.load(Ordering::Relaxed) + self.skipped());
        held as f64 / self.capacity.max(1) as f64
    }
    /// The reader has requested a stop.
    pub fn closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

/// Makes a ring that holds `capacity` samples of type `sample`.
pub fn ring(capacity: usize, sample: SampleType) -> (RingWriter, RingReader) {
    let capacity = capacity.max(1);
    let (mut p, c) = rtrb::RingBuffer::new(capacity * sample::size(sample));
    let stats = Arc::new(RingStats::new(capacity));
    // The room of a new ring is one slice that starts at the first byte of its memory.
    let base = p
        .write_chunk_uninit(capacity * sample::size(sample))
        .map_or(0, |mut chunk| chunk.as_mut_slices().0.as_ptr() as usize);
    (
        RingWriter {
            p,
            sample,
            stats: stats.clone(),
            base,
            init: 0,
            part: [0; PART],
            part_len: 0,
        },
        RingReader {
            c,
            sample,
            stats,
            dc: crate::dc::DcBlock::default(),
            dc_seen: 0,
            dc_at: None,
        },
    )
}

/// The size of the largest sample in bytes.
const PART: usize = 8;

/// A [`RingWriter::receive`] reads straight into the ring only if the ring has this many bytes of room in one piece (or as
/// many as the read may take, if that is less). Below it the read goes to the spare buffer, so the reads stay large.
const DIRECT_MIN: usize = 4096;

/// What one [`RingWriter::receive`] took in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Received {
    /// Bytes the read returned. 0: the read returned 0.
    pub bytes: usize,
    /// Whole samples offered to the ring, written or dropped.
    pub samples: usize,
    /// The read filled what it was given: more bytes may have waited behind them.
    pub full: bool,
}

/// The radio's end of the ring.
pub struct RingWriter {
    p: rtrb::Producer<u8>,
    sample: SampleType,
    stats: Arc<RingStats>,
    /// The address of the first byte of the ring's memory.
    base: usize,
    /// The bytes of the ring's memory before this offset have been written at least once.
    init: usize,
    /// The first `part_len` bytes of a sample that the last receive ended in.
    part: [u8; PART],
    part_len: usize,
}

impl RingWriter {
    /// The type of the samples.
    pub fn sample_type(&self) -> SampleType {
        self.sample
    }

    /// For a live radio: writes the whole samples of `bytes` that fit, drops the rest, and returns how many were written.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let size = sample::size(self.sample);
        self.write_with(bytes.len() / size, |dst, from| {
            let src = &bytes[from * size..from * size + dst.len()];
            // SAFETY: `dst` and `src` have the same length, are both valid for it, and do not overlap (one is in the ring,
            // the other is the caller's slice).
            unsafe {
                std::ptr::copy_nonoverlapping(
                    src.as_ptr(),
                    dst.as_mut_ptr().cast::<u8>(),
                    dst.len(),
                );
            }
        })
    }

    /// For a live radio of [`SampleType::Cf32`]: writes the samples of `iq` that fit, as [`RingWriter::write`] does.
    pub fn write_iq(&mut self, iq: &[Complex32]) -> usize {
        debug_assert_eq!(
            self.sample,
            SampleType::Cf32,
            "complex floats go to a cf32 ring"
        );
        self.write_with(iq.len(), |dst, from| {
            for (d, c) in dst.as_chunks_mut::<8>().0.iter_mut().zip(&iq[from..]) {
                let b = [c.re.to_le_bytes(), c.im.to_le_bytes()];
                for (slot, v) in d.iter_mut().zip(b.iter().flatten()) {
                    slot.write(*v);
                }
            }
        })
    }

    /// Offers `n` samples that `fill(dst, from)` writes straight into the ring. `fill` must write every byte of `dst`, with
    /// the samples from sample `from` on. It runs once, or twice when the room wraps around the end of the ring, and only for
    /// what fits. The rest are dropped and counted. It returns how many samples were written.
    pub fn write_with(
        &mut self,
        n: usize,
        mut fill: impl FnMut(&mut [MaybeUninit<u8>], usize),
    ) -> usize {
        let size = sample::size(self.sample);
        let k = n.min(self.p.slots() / size);
        if k > 0 {
            if let Ok(mut chunk) = self.p.write_chunk_uninit(k * size) {
                let (a, b) = chunk.as_mut_slices();
                let la = a.len() / size;
                fill(a, 0);
                if !b.is_empty() {
                    fill(b, la);
                }
                // SAFETY: `fill` writes every byte of both slices, which together are the bytes of the chunk.
                unsafe { chunk.commit_all() };
            }
        }
        self.stats.written.fetch_add(k as u64, Ordering::Relaxed);
        let lost = (n - k) as u64;
        if lost > 0 {
            self.stats.dropped.fetch_add(lost, Ordering::Relaxed);
            self.stats.drop_events.fetch_add(1, Ordering::Relaxed);
            self.stats.mark_loss();
            self.stats.gap(lost, false);
        }
        k
    }

    /// Notes that `lost` samples are missing after the samples written so far: the radio or its link lost them before the
    /// ring. The reader finds the place with [`RingReader::take_gaps`].
    pub fn mark_gap(&mut self, lost: u64) {
        if lost > 0 {
            self.stats.gap(lost, false);
        }
    }

    /// Notes that the driver left out `samples` on purpose after the samples written so far (a retune that flushes the
    /// device, the time a half-duplex radio transmits). The reader finds the place with [`RingReader::take_gaps`], marked
    /// [`Gap::deliberate`].
    pub fn mark_deliberate_gap(&mut self, samples: u64) {
        if samples > 0 {
            self.stats.gap(samples, true);
        }
    }

    /// For a live radio whose bytes are native samples: `read(dst)` reads up to `max` bytes (at least 1) from the device and
    /// returns how many, as [`std::io::Read::read`] does. It is called once. `dst` lies in the ring when the ring has enough
    /// room in one piece, so the samples are not copied; else it lies in `spare` (longer than one sample), and what does not
    /// fit is dropped and counted. A read may end inside a sample: the part is kept and goes before the bytes of the next
    /// receive.
    ///
    /// # Errors
    /// The error of `read`. Nothing is written then, and a kept part stays.
    pub(crate) fn receive(
        &mut self,
        max: usize,
        spare: &mut [u8],
        read: impl FnOnce(&mut [u8]) -> io::Result<usize>,
    ) -> io::Result<Received> {
        let size = sample::size(self.sample);
        let c = self.part_len;
        let room = self.p.slots() / size * size;
        if let Ok(mut chunk) = self.p.write_chunk_uninit(room) {
            let (a, _) = chunk.as_mut_slices();
            let want = max.min(a.len().saturating_sub(c));
            if want > 0 && want >= max.min(DIRECT_MIN) {
                let off = a.as_ptr() as usize - self.base;
                let end = off + c + want;
                if end > self.init {
                    for b in &mut a[self.init.saturating_sub(off)..c + want] {
                        b.write(0);
                    }
                    self.init = end;
                }
                // SAFETY: the bytes are in the chunk, which nothing else uses until it is committed or dropped. They are
                // initialized: those before `init` were written before (the ring's memory is written from its first byte
                // on, so all before `off` were), the rest just now.
                let dst = unsafe {
                    std::slice::from_raw_parts_mut(a.as_mut_ptr().cast::<u8>(), c + want)
                };
                let n = read(&mut dst[c..])?.min(want);
                let total = c + n;
                let whole = total / size * size;
                if whole == 0 {
                    self.part[c..total].copy_from_slice(&dst[c..total]);
                    self.part_len = total;
                } else {
                    dst[..c].copy_from_slice(&self.part[..c]);
                    self.part_len = total - whole;
                    self.part[..total - whole].copy_from_slice(&dst[whole..total]);
                    // SAFETY: the first `whole` bytes of the chunk are the kept part and bytes that `read` returned.
                    unsafe { chunk.commit(whole) };
                    self.stats
                        .written
                        .fetch_add((whole / size) as u64, Ordering::Relaxed);
                }
                return Ok(Received {
                    bytes: n,
                    samples: whole / size,
                    full: n == want,
                });
            }
        }
        let want = max.min(spare.len().saturating_sub(c));
        let n = read(&mut spare[c..c + want])?.min(want);
        spare[..c].copy_from_slice(&self.part[..c]);
        let total = c + n;
        let whole = total / size * size;
        self.write(&spare[..whole]);
        self.part_len = total - whole;
        self.part[..total - whole].copy_from_slice(&spare[whole..total]);
        Ok(Received {
            bytes: n,
            samples: whole / size,
            full: n == want,
        })
    }

    /// Drops the part of a sample that the last receive ended in (at the end of a message).
    pub(crate) fn discard_part(&mut self) {
        self.part_len = 0;
    }

    /// How many samples fit now.
    pub fn room(&self) -> usize {
        self.p.slots() / sample::size(self.sample)
    }

    /// The size of the ring in samples.
    pub fn capacity(&self) -> usize {
        self.stats.capacity
    }

    /// For a file or a generator: waits for room until every whole sample of `bytes` is written. It returns false if the reader
    /// was dropped.
    pub fn write_blocking(&mut self, mut bytes: &[u8]) -> bool {
        let size = sample::size(self.sample);
        while bytes.len() >= size {
            if !self.wait_for_room() {
                return false;
            }
            let n = self.write(&bytes[..(bytes.len() / size).min(self.room()) * size]);
            bytes = &bytes[n * size..];
        }
        true
    }

    /// For a generator of [`SampleType::Cf32`]: waits for room until every sample of `iq` is written. It returns false if the
    /// reader was dropped.
    pub fn write_iq_blocking(&mut self, mut iq: &[Complex32]) -> bool {
        while !iq.is_empty() {
            if !self.wait_for_room() {
                return false;
            }
            let n = self.write_iq(&iq[..iq.len().min(self.room())]);
            iq = &iq[n..];
        }
        true
    }

    fn wait_for_room(&self) -> bool {
        loop {
            if self.closed() {
                return false;
            }
            if self.room() > 0 {
                return true;
            }
            std::thread::sleep(Duration::from_micros(500));
        }
    }

    /// The reader was dropped or has requested a stop.
    pub fn closed(&self) -> bool {
        self.p.is_abandoned() || self.stats.closed()
    }

    /// The counters both ends share.
    pub fn stats(&self) -> Arc<RingStats> {
        self.stats.clone()
    }
}

/// The host's end of the ring.
pub struct RingReader {
    c: rtrb::Consumer<u8>,
    sample: SampleType,
    stats: Arc<RingStats>,
    dc: crate::dc::DcBlock,
    /// The restart of the DC estimate last seen, and the sample where it takes effect while that is not read yet.
    dc_seen: u64,
    dc_at: Option<u64>,
}

impl RingReader {
    /// The type of the samples.
    pub fn sample_type(&self) -> SampleType {
        self.sample
    }

    /// Samples waiting to be read.
    pub fn available(&self) -> usize {
        self.c.slots() / sample::size(self.sample)
    }

    /// Reads up to `max` samples without copying them: `f` gets the bytes where they lie in the ring (once, or twice when
    /// they wrap around its end), and they are released when it returns. It returns how many samples were read.
    pub fn read_with(&mut self, max: usize, mut f: impl FnMut(&[u8])) -> usize {
        let size = sample::size(self.sample);
        let n = max.min(self.available());
        if n == 0 {
            return 0;
        }
        if let Ok(chunk) = self.c.read_chunk(n * size) {
            let (a, b) = chunk.as_slices();
            if !a.is_empty() {
                f(a);
            }
            if !b.is_empty() {
                f(b);
            }
            chunk.commit_all();
        }
        self.stats.read.fetch_add(n as u64, Ordering::Relaxed);
        n
    }

    /// Reads up to `out.len()` samples as complex samples ([`sample::to_iq`]) and returns how many. With DC removal on
    /// ([`RingStats::set_dc_removal`]) the offset is taken out ([`crate::dc`]), its estimate starting again at the sample of
    /// each [`RingStats::restart_dc`].
    pub fn read_iq(&mut self, out: &mut [Complex32]) -> usize {
        let t = self.sample;
        let start = self.position();
        let mut at = 0;
        let n = self.read_with(out.len(), |b| at += sample::to_iq(t, b, &mut out[at..]));
        if self.stats.dc_on.load(Ordering::Acquire) {
            let e = self.stats.dc_epoch.load(Ordering::Acquire);
            if e != self.dc_seen {
                self.dc_seen = e;
                self.dc_at = Some(self.stats.dc_from.load(Ordering::Acquire));
            }
            let got = &mut out[..n];
            match self.dc_at {
                Some(from) if from < start + n as u64 => {
                    let cut = usize::try_from(from.saturating_sub(start)).unwrap_or(0).min(n);
                    let (before, after) = got.split_at_mut(cut);
                    self.dc.process(before);
                    self.dc.restart();
                    self.dc.process(after);
                    self.dc_at = None;
                }
                _ => self.dc.process(got),
            }
        }
        n
    }

    /// Waits up to `timeout` for at least `min` samples, or for the writer to go. It returns what is waiting.
    pub fn wait(&self, min: usize, timeout: Duration) -> usize {
        let deadline = Instant::now() + timeout;
        loop {
            let n = self.available();
            if n >= min || self.c.is_abandoned() || Instant::now() >= deadline {
                return n;
            }
            std::thread::sleep(Duration::from_micros(500));
        }
    }

    /// Drops everything waiting, so the host starts again on new samples (after a retune). It returns how many were dropped.
    pub fn skip_all(&mut self) -> usize {
        self.read_with(usize::MAX, |_| {})
    }

    /// Drops up to `n` waiting samples and returns how many were dropped.
    pub fn skip(&mut self, n: usize) -> usize {
        self.read_with(n, |_| {})
    }

    /// The place of the next read in the stream: the samples read or skipped so far. A place from
    /// [`RingStats::written`] is behind every sample written after it.
    pub fn position(&self) -> u64 {
        self.stats.read.load(Ordering::Relaxed) + self.stats.skipped()
    }

    /// Tells the writer to stop: a waiting blocking write returns false.
    pub fn close(&self) {
        self.stats.closed.store(true, Ordering::Release);
    }

    /// The writer has gone: the radio stopped or failed.
    pub fn writer_gone(&self) -> bool {
        self.c.is_abandoned()
    }

    /// For a live radio, before each read: if more than half the ring waits, it jumps to the newest 1/16 of the ring in one
    /// step and counts the skipped samples as loss on the reader's side. It returns how many samples it skipped.
    pub fn skip_backlog(&mut self) -> usize {
        let cap = self.stats.capacity;
        let n = self.available();
        if n <= cap / 2 {
            return 0;
        }
        let skip = n - cap / 16;
        if let Ok(chunk) = self.c.read_chunk(skip * sample::size(self.sample)) {
            chunk.commit_all();
        }
        self.stats.skipped.fetch_add(skip as u64, Ordering::Relaxed);
        self.stats.skip_events.fetch_add(1, Ordering::Relaxed);
        self.stats.mark_loss();
        skip
    }

    /// The counters both ends share.
    pub fn stats(&self) -> Arc<RingStats> {
        self.stats.clone()
    }

    /// Takes the gaps that lie within the next `n` samples, in order. A gap that a skip passed over is forgotten. It takes no
    /// lock while there is no gap.
    pub fn take_gaps(&mut self, n: usize) -> Vec<Gap> {
        if self.stats.gap_count.load(Ordering::Acquire) == 0 {
            return Vec::new();
        }
        let pos = self.stats.read.load(Ordering::Relaxed) + self.stats.skipped();
        let mut g = self
            .stats
            .gaps
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut out = Vec::new();
        while let Some(&(at, lost, deliberate)) = g.front() {
            if at < pos {
                g.pop_front();
                continue;
            }
            let rel = usize::try_from(at - pos).unwrap_or(usize::MAX);
            if rel > n {
                break;
            }
            out.push(Gap {
                at: rel,
                lost,
                deliberate,
            });
            g.pop_front();
        }
        self.stats.gap_count.store(g.len() as u64, Ordering::Release);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_ring_and_a_marked_loss_leave_gaps_where_the_samples_are_missing() {
        let (mut w, mut r) = ring(8, SampleType::Cs8);
        assert_eq!(w.write(&[1; 12]), 6);
        w.mark_gap(100);
        // 4 more samples, 2 fit: 2 dropped after the 8th sample.
        assert_eq!(w.write(&[2; 8]), 2);
        assert_eq!(r.stats().dropped(), 2);
        assert_eq!(r.take_gaps(5), [], "no gap within the first 5 samples");
        assert_eq!(r.read_with(3, |_| {}), 3);
        assert_eq!(
            r.take_gaps(8),
            [Gap::lost_at(3, 100), Gap::lost_at(5, 2)]
        );
        assert_eq!(r.take_gaps(8), []);
        // A gap that a skip passes over is forgotten.
        r.skip_all();
        w.mark_gap(7);
        w.write(&[3; 4]);
        r.skip_all();
        assert_eq!(r.take_gaps(8), []);
    }

    #[test]
    fn a_gap_made_on_purpose_keeps_its_kind_beside_a_loss_at_the_same_place() {
        let (mut w, mut r) = ring(64, SampleType::Cs8);
        assert_eq!(w.write(&[1; 8]), 4);
        w.mark_deliberate_gap(10);
        w.mark_deliberate_gap(5);
        w.mark_gap(3);
        assert_eq!(w.write(&[1; 8]), 4);
        let gaps = r.take_gaps(8);
        assert_eq!(
            gaps,
            [
                Gap {
                    deliberate: true,
                    ..Gap::lost_at(4, 15)
                },
                Gap::lost_at(4, 3)
            ]
        );
    }

    fn s(v: f32) -> Complex32 {
        Complex32::new(v, -v)
    }

    #[test]
    fn a_live_writer_drops_what_does_not_fit_and_counts_it() {
        let (mut w, mut r) = ring(4, SampleType::Cf32);
        let block: Vec<_> = (0..6).map(|i| s(i as f32)).collect();
        assert_eq!(w.write_iq(&block), 4);
        let stats = r.stats();
        assert_eq!((stats.written(), stats.dropped()), (4, 2));
        let mut out = [Complex32::default(); 8];
        assert_eq!(r.read_iq(&mut out), 4);
        assert_eq!(out[3], s(3.0));
    }

    #[test]
    fn a_paced_writer_waits_and_loses_nothing() {
        let (mut w, mut r) = ring(64, SampleType::Cf32);
        let t = std::thread::spawn(move || {
            let block: Vec<_> = (0..1000).map(|i| s(i as f32)).collect();
            w.write_iq_blocking(&block)
        });
        let mut got = Vec::new();
        let mut buf = [Complex32::default(); 16];
        while got.len() < 1000 {
            r.wait(1, Duration::from_millis(100));
            let n = r.read_iq(&mut buf);
            got.extend_from_slice(&buf[..n]);
        }
        assert!(t.join().unwrap());
        assert_eq!(got.len(), 1000);
        assert!(got.iter().enumerate().all(|(i, v)| *v == s(i as f32)));
        assert_eq!(r.stats().dropped(), 0);
    }

    #[test]
    fn a_large_backlog_is_skipped_in_one_jump_and_counted() {
        let (mut w, mut r) = ring(1024, SampleType::Cf32);
        w.write_iq(&vec![s(0.0); 400]);
        assert_eq!(r.skip_backlog(), 0);
        w.write_iq(&(0..400).map(|i| s(i as f32)).collect::<Vec<_>>());
        assert_eq!(r.skip_backlog(), 800 - 64);
        assert_eq!(r.available(), 64);
        let mut out = [Complex32::default(); 1];
        r.read_iq(&mut out);
        assert_eq!(out[0], s(336.0));
        let st = r.stats();
        assert_eq!(
            (st.skipped(), st.skip_events(), st.cpu_lost()),
            (736, 1, 736)
        );
        assert!(st.since_last_loss().is_some());
    }

    #[test]
    fn read_with_hands_over_the_ring_slices_across_the_wrap() {
        let (mut w, mut r) = ring(8, SampleType::Cf32);
        w.write_iq(&(0..6).map(|i| s(i as f32)).collect::<Vec<_>>());
        let mut out = [Complex32::default(); 6];
        assert_eq!(r.read_iq(&mut out), 6);
        w.write_iq(&(6..12).map(|i| s(i as f32)).collect::<Vec<_>>());
        let mut pieces = Vec::new();
        assert_eq!(r.read_with(100, |x| pieces.push(x.to_vec())), 6);
        assert_eq!(pieces.len(), 2, "the samples wrap around the end");
        let bytes: Vec<u8> = pieces.concat();
        let mut all = [Complex32::default(); 6];
        assert_eq!(sample::to_iq(SampleType::Cf32, &bytes, &mut all), 6);
        assert!(all.iter().enumerate().all(|(i, v)| *v == s((i + 6) as f32)));
        assert_eq!(r.available(), 0);
        assert_eq!(r.read_with(10, |_| panic!("nothing waits")), 0);
    }

    #[test]
    fn closing_releases_a_blocked_writer() {
        let (mut w, r) = ring(8, SampleType::Cf32);
        let t = std::thread::spawn(move || w.write_iq_blocking(&vec![s(1.0); 100]));
        std::thread::sleep(Duration::from_millis(20));
        r.close();
        assert!(!t.join().unwrap());
    }
}

#[cfg(test)]
mod native {
    use super::*;

    #[test]
    fn native_bytes_go_through_whole_and_the_fill_level_follows() {
        let (mut w, mut r) = ring(4, SampleType::Cu8);
        assert_eq!(w.write(&[1, 2, 3, 4, 5]), 2, "a part sample is not written");
        assert!((r.stats().fill() - 0.5).abs() < 1e-9);
        assert_eq!(w.write(&[6, 7, 8, 9, 10, 11]), 2);
        assert_eq!(r.stats().dropped(), 1);
        let mut got = Vec::new();
        assert_eq!(r.read_with(10, |b| got.extend_from_slice(b)), 4);
        assert_eq!(got, [1, 2, 3, 4, 6, 7, 8, 9]);
        assert_eq!(r.stats().fill(), 0.0);
    }

    /// A receive that takes the bytes of `src`.
    fn take(w: &mut RingWriter, spare: &mut [u8], src: &[u8]) -> Received {
        w.receive(src.len(), spare, |dst| {
            dst[..src.len()].copy_from_slice(src);
            Ok(src.len())
        })
        .unwrap()
    }

    #[test]
    fn a_receive_reads_into_the_memory_the_reader_gets() {
        let (mut w, mut r) = ring(8, SampleType::Cs16);
        let mut spare = [0u8; 64];
        let spare_at = spare.as_ptr() as usize..spare.as_ptr() as usize + spare.len();
        let mut at = 0;
        let got = w
            .receive(12, &mut spare, |dst| {
                assert_eq!(dst.len(), 12);
                at = dst.as_ptr() as usize;
                dst[..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
                Ok(8)
            })
            .unwrap();
        assert_eq!((got.bytes, got.samples), (8, 2));
        assert!(!spare_at.contains(&at));
        let mut seen = (0, Vec::new());
        assert_eq!(
            r.read_with(8, |b| seen = (b.as_ptr() as usize, b.to_vec())),
            2
        );
        assert_eq!(seen, (at, vec![1, 2, 3, 4, 5, 6, 7, 8]));
        assert_eq!(r.stats().written(), 2);
    }

    #[test]
    fn a_receive_keeps_a_part_sample_for_the_next_one() {
        let (mut w, mut r) = ring(4, SampleType::Cs16);
        let mut spare = [0u8; 64];
        // 16 bytes of ring. Three samples go through, so the room wraps: 4 bytes at the end, 12 at the start.
        assert_eq!(take(&mut w, &mut spare, &[0; 12]).samples, 3);
        assert_eq!(r.skip_all(), 3);
        let got = take(&mut w, &mut spare, &[1, 2, 3]);
        assert_eq!((got.bytes, got.samples), (3, 0));
        assert_eq!(r.available(), 0);
        // A failed read writes nothing and keeps the part.
        let e = w.receive(4, &mut spare, |_| Err(io::ErrorKind::TimedOut.into()));
        assert_eq!(e.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(r.available(), 0);
        // One byte of room is left in the piece at the end after the part.
        let got = take(&mut w, &mut spare, &[4]);
        assert_eq!((got.bytes, got.samples), (1, 1));
        let got = take(&mut w, &mut spare, &[5, 6, 7, 8, 9, 10]);
        assert_eq!((got.bytes, got.samples), (6, 1));
        let got = take(&mut w, &mut spare, &[11, 12, 13]);
        assert_eq!((got.bytes, got.samples), (3, 1));
        let mut all = Vec::new();
        assert_eq!(r.read_with(8, |b| all.extend_from_slice(b)), 3);
        assert_eq!(all, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        w.discard_part();
        assert_eq!(take(&mut w, &mut spare, &[20, 21, 22, 23]).samples, 1);
        all.clear();
        r.read_with(8, |b| all.extend_from_slice(b));
        assert_eq!(all, [20, 21, 22, 23]);
        assert_eq!(r.stats().dropped(), 0);
    }

    #[test]
    fn receives_after_a_plain_write_go_around_the_ring() {
        let (mut w, mut r) = ring(8, SampleType::Cu8);
        let mut spare = [0u8; 64];
        assert_eq!(w.write(&[1, 2, 3, 4, 5, 6]), 3);
        let mut all = Vec::new();
        for i in 0..10u8 {
            let src = [10 * i + 7, 10 * i + 8, 10 * i + 9, 10 * i + 10];
            assert_eq!(take(&mut w, &mut spare, &src).samples, 2);
            r.read_with(2, |b| all.extend_from_slice(b));
        }
        r.read_with(8, |b| all.extend_from_slice(b));
        let mut sent = vec![1, 2, 3, 4, 5, 6];
        sent.extend((0..10u8).flat_map(|i| [10 * i + 7, 10 * i + 8, 10 * i + 9, 10 * i + 10]));
        assert_eq!(all, sent);
        assert_eq!(r.stats().dropped(), 0);
    }

    #[test]
    fn a_receive_into_a_full_ring_reads_to_the_spare_buffer_and_counts_the_loss() {
        let (mut w, mut r) = ring(2, SampleType::Cu8);
        let mut spare = [0u8; 64];
        assert_eq!(take(&mut w, &mut spare, &[1, 2, 3, 4]).samples, 2);
        let spare_at = spare.as_ptr() as usize;
        let got = w
            .receive(64, &mut spare, |dst| {
                assert_eq!(dst.as_ptr() as usize, spare_at);
                dst[..5].copy_from_slice(&[5, 6, 7, 8, 9]);
                Ok(5)
            })
            .unwrap();
        assert_eq!((got.bytes, got.samples), (5, 2));
        assert_eq!((r.stats().dropped(), r.stats().drop_events()), (2, 1));
        // The reader makes room for one sample: the kept byte and one more.
        let mut all = Vec::new();
        r.read_with(1, |b| all.extend_from_slice(b));
        let got = take(&mut w, &mut spare, &[10]);
        assert_eq!((got.bytes, got.samples), (1, 1));
        r.read_with(8, |b| all.extend_from_slice(b));
        assert_eq!(all, [1, 2, 3, 4, 9, 10]);
    }
}
