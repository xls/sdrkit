//! Transport stream playback paced by the PCR, with pause and seek, of a file or of the time-shift buffer. Stream time is
//! the seconds of PCR since the first PCR of the clock PID (the first PID that carries one).
//!
//! Standard: ISO/IEC 13818-1, 2.4.3.5 (program_clock_reference).

use sdrkit_core::storage::StoreReader;
use sdrkit_core::{Storage, StorageError, StorePath};
use sdrkit_proto_mpegts::packet::{self, SYNC, TS_SIZE};
use sdrkit_proto_mpegts::pcr::{Pacer, PCR_WRAP_S};
use std::io::{Read, Seek, SeekFrom};

/// Bytes a store holds, read at absolute positions.
pub(crate) trait TsStore: Send {
    /// The first readable position.
    fn start(&mut self) -> u64;
    /// The end of what is readable (it grows for a buffer being written).
    fn end(&mut self) -> u64;
    /// Reads at `pos` into `buf` and returns the bytes read (0 at the end). An error means that `pos` is outside the data held.
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> std::io::Result<usize>;
    /// The raw PCR (seconds) that is time zero, if the store has it (a time-shift buffer that indexed it).
    fn first_pcr(&mut self) -> Option<(u16, f64)> {
        None
    }
    /// A position at or before stream time `t`, when the store keeps an index.
    fn position_of(&mut self, _t: f64) -> Option<u64> {
        None
    }
    /// The stream times of its first and last indexed PCR, when it keeps an index.
    fn span(&mut self) -> Option<(f64, f64)> {
        None
    }
}

/// A `.ts` file (188-byte packets) in storage.
pub(crate) struct FileStore {
    reader: Box<dyn StoreReader>,
    len: u64,
}

impl FileStore {
    /// Opens `path` on `storage`.
    pub(crate) fn open(storage: &dyn Storage, path: &StorePath) -> Result<FileStore, StorageError> {
        let mut reader = storage.open_reader(path)?;
        let len = reader.size()?;
        Ok(FileStore { reader, len })
    }
}

impl TsStore for FileStore {
    fn start(&mut self) -> u64 {
        0
    }
    fn end(&mut self) -> u64 {
        if let Ok(n) = self.reader.size() {
            self.len = n;
        }
        self.len
    }
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.seek(SeekFrom::Start(pos))?;
        let mut n = 0;
        while n < buf.len() {
            match self.reader.read(&mut buf[n..])? {
                0 => break,
                k => n += k,
            }
        }
        Ok(n)
    }
}

/// What [`TsPlayer::read`] did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ReadState {
    /// Packets were added. Call again soon.
    More,
    /// Nothing is due before this wall time (seconds).
    WaitUntil(f64),
    /// Paused.
    Paused,
    /// At the end of what is held. A buffer that grows can have more later.
    End,
}

/// The player's figures.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PlayStatus {
    /// Stream time of the last PCR passed (seconds).
    pub position_s: f64,
    /// Stream time of the end, when known.
    pub duration_s: Option<f64>,
    /// Paused.
    pub paused: bool,
    /// Reached the end.
    pub ended: bool,
    /// Times the reading position fell outside the data held and restarted at the oldest data.
    pub overruns: u64,
    /// Bytes skipped to find packet sync.
    pub resyncs: u64,
}

/// Read-ahead size.
const AHEAD: usize = 512 * TS_SIZE;

/// Plays a store paced by its PCR.
pub(crate) struct TsPlayer<S: TsStore> {
    store: S,
    origin: u64,
    clock: Option<(u16, f64)>,
    pos: u64,
    buf: Vec<u8>,
    buf_at: u64,
    /// A packet waiting for its time, with its PCR's stream time.
    held: Option<([u8; TS_SIZE], f64)>,
    pacer: Pacer,
    status: PlayStatus,
}

impl<S: TsStore> TsPlayer<S> {
    /// A player at the start of the store. Finds the packet alignment and the clock PID (the first PID carrying a PCR).
    pub fn new(mut store: S) -> std::io::Result<TsPlayer<S>> {
        let start = store.start();
        let mut head = vec![0u8; 64 * TS_SIZE];
        let n = store.read_at(start, &mut head)?;
        head.truncate(n);
        let origin = start + align(&head).unwrap_or(0) as u64;
        let mut p = TsPlayer {
            store,
            origin,
            clock: None,
            pos: origin,
            buf: Vec::new(),
            buf_at: 0,
            held: None,
            pacer: Pacer::new(),
            status: PlayStatus::default(),
        };
        p.clock = p
            .store
            .first_pcr()
            .or_else(|| p.scan_pcr(origin, 64 << 20, None));
        Ok(p)
    }

    /// The store.
    pub fn store(&mut self) -> &mut S {
        &mut self.store
    }

    fn rel(&self, pcr_s: f64) -> f64 {
        match self.clock {
            Some((_, first)) => (pcr_s - first).rem_euclid(PCR_WRAP_S),
            None => 0.0,
        }
    }

    /// Packet boundary at or before `pos`.
    fn floor(&self, pos: u64) -> u64 {
        if pos <= self.origin {
            return self.origin;
        }
        self.origin + (pos - self.origin) / TS_SIZE as u64 * TS_SIZE as u64
    }

    /// The first PCR (pid, raw seconds) of `pid` (or of any PID) from `from`, in at most `limit` bytes, with its position.
    fn scan_pcr_at(&mut self, from: u64, limit: u64, pid: Option<u16>) -> Option<(u16, f64, u64)> {
        let mut pos = self.floor(from);
        let end = from.saturating_add(limit);
        let mut chunk = vec![0u8; AHEAD];
        while pos < end {
            let n = self.store.read_at(pos, &mut chunk).ok()?;
            if n < TS_SIZE {
                return None;
            }
            for (k, p) in chunk[..n].as_chunks::<TS_SIZE>().0.iter().enumerate() {
                if p[0] != SYNC {
                    continue;
                }
                let Some(parsed) = packet::parse(p) else {
                    continue;
                };
                let Some(pcr) = parsed.adaptation.and_then(|a| a.pcr) else {
                    continue;
                };
                let id = parsed.header.pid;
                if pid.is_none_or(|w| w == id) {
                    return Some((id, packet::pcr_seconds(pcr), pos + (k * TS_SIZE) as u64));
                }
            }
            pos += (n / TS_SIZE * TS_SIZE) as u64;
        }
        None
    }

    fn scan_pcr(&mut self, from: u64, limit: u64, pid: Option<u16>) -> Option<(u16, f64)> {
        self.scan_pcr_at(from, limit, pid).map(|(p, t, _)| (p, t))
    }

    /// Stream time of the last PCR in the store (looking back from the end), when there is one.
    pub fn duration(&mut self) -> Option<f64> {
        if let Some((_, b)) = self.store.span() {
            return Some(b);
        }
        let (pid, _) = self.clock?;
        let end = self.store.end();
        let start = self.store.start().max(self.origin);
        let mut window: u64 = 1 << 20;
        loop {
            let from = end.saturating_sub(window).max(start);
            let mut at = from;
            let mut last = None;
            while let Some((_, t, p)) = self.scan_pcr_at(at, end - at, Some(pid)) {
                last = Some(t);
                at = p + TS_SIZE as u64;
            }
            if let Some(t) = last {
                let d = self.rel(t);
                self.status.duration_s = Some(d);
                return Some(d);
            }
            if from == start {
                return None;
            }
            window *= 4;
        }
    }

    /// Moves to stream time `t` (seconds): to the last PCR at or before it, found from the store's index or by a binary search
    /// on the file. Playing on from there is paced again from the next call.
    pub fn seek(&mut self, t: f64) {
        let t = t.max(0.0);
        let target = match self.store.position_of(t) {
            Some(p) => p,
            None => self.search(t),
        };
        self.jump(target);
        self.status.position_s = t;
    }

    fn search(&mut self, t: f64) -> u64 {
        let Some((pid, _)) = self.clock else {
            return self.origin;
        };
        let mut lo = self.store.start().max(self.origin);
        let mut hi = self.store.end();
        while hi - lo > 64 * TS_SIZE as u64 {
            let mid = self.floor(lo + (hi - lo) / 2);
            match self.scan_pcr_at(mid, hi - mid, Some(pid)) {
                Some((_, pt, at)) if self.rel(pt) <= t => lo = at.max(mid),
                Some(_) => hi = mid,
                None => hi = mid,
            }
        }
        // The last PCR at or before t, from lo.
        let mut best = lo;
        let mut at = lo;
        while let Some((_, pt, p)) =
            self.scan_pcr_at(at, hi.saturating_sub(at) + 64 * TS_SIZE as u64, Some(pid))
        {
            if self.rel(pt) > t {
                break;
            }
            best = p;
            at = p + TS_SIZE as u64;
        }
        best
    }

    fn jump(&mut self, pos: u64) {
        self.pos = self.floor(pos);
        self.buf.clear();
        self.held = None;
        self.pacer = Pacer::new();
        self.status.ended = false;
    }

    /// Pauses. Packets stop and the position stays.
    pub fn pause(&mut self) {
        self.status.paused = true;
    }

    /// Plays on from where it paused, paced again from now.
    pub fn resume(&mut self) {
        if self.status.paused {
            self.status.paused = false;
            self.pacer = Pacer::new();
        }
    }

    /// The figures.
    pub fn status(&self) -> PlayStatus {
        self.status.clone()
    }

    fn fill(&mut self) -> bool {
        let start = self.store.start();
        if self.pos < start {
            self.status.overruns += 1;
            let p = self.floor(start + TS_SIZE as u64 - 1);
            self.jump(p.max(start));
        }
        if self.buf_at <= self.pos && self.pos < self.buf_at + self.buf.len() as u64 {
            return true;
        }
        self.buf.resize(AHEAD, 0);
        match self.store.read_at(self.pos, &mut self.buf) {
            Ok(n) => {
                self.buf.truncate(n / TS_SIZE * TS_SIZE);
                self.buf_at = self.pos;
                !self.buf.is_empty()
            }
            Err(_) => {
                // No longer held: from the oldest data.
                self.status.overruns += 1;
                let s = self.store.start();
                self.jump(self.floor(s + TS_SIZE as u64 - 1).max(s));
                self.buf.clear();
                false
            }
        }
    }

    /// Adds the packets due at wall time `now` (seconds, any monotonic clock) to `out`, at most `max`.
    pub fn read(&mut self, now: f64, out: &mut Vec<[u8; TS_SIZE]>, max: usize) -> ReadState {
        if self.status.paused {
            return ReadState::Paused;
        }
        let mut added = 0;
        while added < max {
            let p = match self.held.take() {
                Some((p, t)) => {
                    self.status.position_s = t;
                    p
                }
                None => {
                    if !self.fill() {
                        self.status.ended = true;
                        return if added > 0 {
                            ReadState::More
                        } else {
                            ReadState::End
                        };
                    }
                    let off = (self.pos - self.buf_at) as usize;
                    let mut p = [0u8; TS_SIZE];
                    p.copy_from_slice(&self.buf[off..off + TS_SIZE]);
                    self.pos += TS_SIZE as u64;
                    if p[0] != SYNC {
                        // Out of step: find the next sync.
                        self.status.resyncs += 1;
                        let rest = &self.buf[off + 1..];
                        let k = align(rest).unwrap_or(rest.len());
                        self.origin = self.buf_at + (off + 1 + k) as u64;
                        self.pos = self.origin;
                        continue;
                    }
                    if let Some(parsed) = packet::parse(&p) {
                        if self.clock.is_none() {
                            // A store that was empty when the player was made.
                            self.clock = self.store.first_pcr().or_else(|| {
                                let pcr = parsed.adaptation.and_then(|a| a.pcr)?;
                                Some((parsed.header.pid, packet::pcr_seconds(pcr)))
                            });
                        }
                        if let (Some((cpid, _)), Some(pcr)) =
                            (self.clock, parsed.adaptation.and_then(|a| a.pcr))
                        {
                            if parsed.header.pid == cpid {
                                let t = self.rel(packet::pcr_seconds(pcr));
                                let wait = self.pacer.pcr(cpid, t, now);
                                if wait > 0.0 {
                                    self.held = Some((p, t));
                                    return ReadState::WaitUntil(now + wait);
                                }
                                self.status.position_s = t;
                            }
                        }
                    }
                    p
                }
            };
            out.push(p);
            added += 1;
        }
        ReadState::More
    }
}

/// The offset of the first byte where three packets in a row start with the sync byte.
fn align(b: &[u8]) -> Option<usize> {
    (0..TS_SIZE.min(b.len())).find(|&k| {
        (0..3).all(|j| {
            b.get(k + j * TS_SIZE)
                .map_or(j > 0 && k + j * TS_SIZE >= b.len(), |&x| x == SYNC)
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alignment() {
        let mut b = vec![0u8; 5];
        for _ in 0..4 {
            let mut p = [0u8; TS_SIZE];
            p[0] = SYNC;
            b.extend_from_slice(&p);
        }
        assert_eq!(align(&b), Some(5));
        assert_eq!(align(&[1, 2, 3]), None);
    }
}
