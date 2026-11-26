//! The writer thread of a recorder. The feeding side fills blocks from a fixed pool and queues them, and the writer thread
//! writes them through `Storage` and returns the buffers. When no block is free, the feeding side either leaves data out
//! and counts it ([`Spool::block`]) or waits for the writer ([`Spool::block_waiting`]).

use crate::error::RecordError;
use crate::flac::FlacWriter;
use crate::wav;
use sdrkit_core::storage::{StoreWriter, WriteMode};
use sdrkit_core::{Storage, StorageError, StorePath};
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Room in the queue for control operations beside the data blocks.
const CONTROL_ROOM: usize = 64;
/// The most numbered names tried for a file whose name is taken.
const MAX_SUFFIX: u32 = 9999;

/// A block of data that the writer thread writes.
pub(crate) trait Block: Send + 'static {
    /// True if the block holds nothing.
    fn is_empty(&self) -> bool;
    /// Writes the block to `sink`.
    fn write_to(&self, sink: &mut Sink) -> std::io::Result<()>;
    /// Empties the block and keeps its memory.
    fn clear(&mut self);
}

impl Block for Vec<u8> {
    fn is_empty(&self) -> bool {
        Vec::is_empty(self)
    }
    fn write_to(&self, sink: &mut Sink) -> std::io::Result<()> {
        sink.write(self)
    }
    fn clear(&mut self) {
        Vec::clear(self);
    }
}

/// How a file encodes the bytes it gets.
pub(crate) enum Encoding {
    /// The bytes as they come.
    Plain,
    /// 16-bit little-endian samples under a WAV header that is finished on close.
    Wav { rate: u32, channels: u16 },
    /// 16-bit little-endian mono samples, encoded as FLAC with these Vorbis comments.
    Flac { rate: u32, tags: Vec<String> },
}

/// An open file of the writer thread.
pub(crate) struct Sink {
    kind: SinkKind,
    bytes: u64,
}

enum SinkKind {
    Plain(Box<dyn StoreWriter>),
    Wav {
        w: Box<dyn StoreWriter>,
        rate: u32,
        channels: u16,
    },
    Flac(FlacWriter<Box<dyn StoreWriter>>),
}

impl Sink {
    fn open(mut w: Box<dyn StoreWriter>, encoding: Encoding) -> std::io::Result<Sink> {
        let kind = match encoding {
            Encoding::Plain => SinkKind::Plain(w),
            Encoding::Wav { rate, channels } => {
                w.write_all(&wav::header(rate, channels, 0))?;
                SinkKind::Wav { w, rate, channels }
            }
            Encoding::Flac { rate, tags } => SinkKind::Flac(FlacWriter::new(w, rate, &tags)?),
        };
        Ok(Sink { kind, bytes: 0 })
    }

    /// Writes `bytes` of data.
    pub(crate) fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match &mut self.kind {
            SinkKind::Plain(w) | SinkKind::Wav { w, .. } => w.write_all(bytes)?,
            SinkKind::Flac(f) => f.push_le(bytes)?,
        }
        self.bytes += bytes.len() as u64;
        Ok(())
    }

    /// Finishes the headers and syncs the file.
    fn finish(self) -> Result<(), StorageError> {
        match self.kind {
            SinkKind::Plain(mut w) => w.sync(),
            SinkKind::Wav {
                mut w,
                rate,
                channels,
            } => {
                let data = u32::try_from(self.bytes).unwrap_or(u32::MAX - 36);
                w.seek(SeekFrom::Start(0))?;
                w.write_all(&wav::header(rate, channels, data))?;
                w.sync()
            }
            SinkKind::Flac(f) => f.finish()?.sync(),
        }
    }
}

/// A request from the feeding side to the writer thread.
pub(crate) enum Op<B> {
    /// Opens `path` in `slot` (with `_2`, `_3` ... before the extension if the name is taken), and writes the bytes of `meta`
    /// beside it, under the extension that `meta` names.
    Open {
        slot: usize,
        path: StorePath,
        encoding: Encoding,
        meta: Option<(&'static str, Vec<u8>)>,
    },
    /// Writes a block to the file of `slot`.
    Data { slot: usize, block: B },
    /// Writes `meta` again beside the file of `slot`, under the extension `ext`.
    Meta {
        slot: usize,
        ext: &'static str,
        meta: Vec<u8>,
    },
    /// Closes the file of `slot`, and removes it if `keep` is false. A kept file adds `log` to its log.
    Close {
        slot: usize,
        keep: bool,
        log: Option<LogLine>,
    },
    /// Closes every file (kept) and ends the thread.
    Finish,
}

/// A line of a log of kept files: `line`, a comma and the file name.
pub(crate) struct LogLine {
    /// The log file.
    pub path: StorePath,
    /// The first line of a new log.
    pub header: &'static str,
    /// The line without the file name.
    pub line: String,
}

/// What the writer thread reports.
#[derive(Debug)]
pub(crate) enum Note {
    /// The file of `slot` is open at `path`.
    Opened { slot: usize, path: StorePath },
    /// The file of `slot` is closed with `bytes` of data. `kept` is false if it was removed.
    Closed {
        slot: usize,
        path: StorePath,
        bytes: u64,
        kept: bool,
    },
    /// A write failed. The file holds what came before, with its header finished.
    Failed {
        path: StorePath,
        error: StorageError,
    },
    /// The thread has ended.
    Done,
}

/// The feeding side of a spool.
pub(crate) struct Spool<B: Block> {
    ops: SyncSender<Op<B>>,
    free: Receiver<B>,
    filling: Vec<Option<B>>,
    gone: bool,
}

/// The control side of a spool: the notes and the thread.
pub(crate) struct Writer<B: Block> {
    ops: SyncSender<Op<B>>,
    notes: Receiver<Note>,
    thread: Option<JoinHandle<()>>,
    done: bool,
    written: Arc<AtomicU64>,
}

/// Starts a writer thread on `storage` with `slots` files and `buffers` blocks that `make` returns.
pub(crate) fn start<B: Block>(
    storage: Arc<dyn Storage>,
    slots: usize,
    buffers: usize,
    make: impl Fn() -> B,
) -> Result<(Spool<B>, Writer<B>), RecordError> {
    let buffers = buffers.max(2);
    let (ops_tx, ops_rx) = mpsc::sync_channel::<Op<B>>(buffers + CONTROL_ROOM);
    let (free_tx, free_rx) = mpsc::sync_channel::<B>(buffers);
    let (note_tx, note_rx) = mpsc::channel::<Note>();
    let written = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&written);
    for _ in 0..buffers {
        // The channel holds exactly `buffers` blocks.
        let _ = free_tx.try_send(make());
    }
    let thread = std::thread::Builder::new()
        .name("sdrkit-record".into())
        .spawn(move || {
            let mut w = Worker {
                storage,
                files: (0..slots).map(|_| None).collect(),
                notes: note_tx,
                written: counter,
            };
            while let Ok(op) = ops_rx.recv() {
                match op {
                    Op::Data { slot, mut block } => {
                        w.data(slot, &block);
                        block.clear();
                        // The channel holds every block, so it always has room for this one.
                        let _ = free_tx.try_send(block);
                    }
                    Op::Finish => break,
                    op => w.control(op),
                }
            }
            for slot in 0..w.files.len() {
                w.close(slot, true);
            }
            let _ = w.notes.send(Note::Done);
        })
        .map_err(|e| RecordError::Thread { kind: e.kind() })?;
    Ok((
        Spool {
            ops: ops_tx.clone(),
            free: free_rx,
            filling: (0..slots).map(|_| None).collect(),
            gone: false,
        },
        Writer {
            ops: ops_tx,
            notes: note_rx,
            thread: Some(thread),
            done: false,
            written,
        },
    ))
}

impl<B: Block> Spool<B> {
    /// The block being filled for `slot`, or `None` when no block is free (the writer is behind) or the writer has ended.
    pub(crate) fn block(&mut self, slot: usize) -> Option<&mut B> {
        if self.gone {
            return None;
        }
        if self.filling[slot].is_none() {
            match self.free.try_recv() {
                Ok(b) => self.filling[slot] = Some(b),
                Err(TryRecvError::Empty) => return None,
                Err(TryRecvError::Disconnected) => {
                    self.gone = true;
                    return None;
                }
            }
        }
        self.filling[slot].as_mut()
    }

    /// The block being filled for `slot`. When no block is free it waits until the writer returns one. `None` only when the
    /// writer has ended.
    pub(crate) fn block_waiting(&mut self, slot: usize) -> Option<&mut B> {
        if self.gone {
            return None;
        }
        if self.filling[slot].is_none() {
            match self.free.recv() {
                Ok(b) => self.filling[slot] = Some(b),
                Err(_) => {
                    self.gone = true;
                    return None;
                }
            }
        }
        self.filling[slot].as_mut()
    }

    /// Hands the block of `slot` to the writer, if it holds data.
    pub(crate) fn send(&mut self, slot: usize) {
        if let Some(block) = self.filling[slot].take_if(|b| !b.is_empty()) {
            self.op(Op::Data { slot, block });
        }
    }

    /// Hands every block that holds data to the writer.
    pub(crate) fn send_all(&mut self) {
        for slot in 0..self.filling.len() {
            self.send(slot);
        }
    }

    /// Queues `op`. It waits only when the queue holds more than its room of control operations.
    pub(crate) fn op(&mut self, op: Op<B>) {
        if self.gone {
            return;
        }
        let r = match self.ops.try_send(op) {
            Err(TrySendError::Full(op)) => self.ops.send(op).map_err(|_| ()),
            Err(TrySendError::Disconnected(_)) => Err(()),
            Ok(()) => Ok(()),
        };
        self.gone = r.is_err();
    }
}

impl<B: Block> Writer<B> {
    /// The next note, if one waits.
    pub(crate) fn note(&mut self) -> Option<Note> {
        let n = self.notes.try_recv().ok()?;
        if matches!(n, Note::Done) {
            self.done = true;
            if let Some(t) = self.thread.take() {
                // The thread sent its last note and returns at once.
                let _ = t.join();
            }
        }
        Some(n)
    }

    /// The bytes of data in the files of the writer, open or closed, that were not removed. It grows with each block
    /// written.
    pub(crate) fn written(&self) -> u64 {
        self.written.load(Ordering::Relaxed)
    }

    /// Asks the thread to close every file and end. The notes say when it has.
    pub(crate) fn finish(&mut self) {
        if !self.done {
            let _ = self.ops.send(Op::Finish);
        }
    }
}

impl<B: Block> Drop for Writer<B> {
    fn drop(&mut self) {
        self.finish();
    }
}

struct OpenFile {
    path: StorePath,
    sink: Option<Sink>,
    /// The bytes of the file counted in `Worker::written`.
    counted: u64,
}

struct Worker {
    storage: Arc<dyn Storage>,
    files: Vec<Option<OpenFile>>,
    notes: mpsc::Sender<Note>,
    /// The bytes of data in the files of this writer that were not removed.
    written: Arc<AtomicU64>,
}

impl Worker {
    fn note(&self, n: Note) {
        let _ = self.notes.send(n);
    }

    fn data<B: Block>(&mut self, slot: usize, block: &B) {
        let Some(Some(f)) = self.files.get_mut(slot) else {
            return;
        };
        let Some(sink) = f.sink.as_mut() else {
            return;
        };
        let result = block.write_to(sink);
        self.written
            .fetch_add(sink.bytes - f.counted, Ordering::Relaxed);
        f.counted = sink.bytes;
        if let Err(e) = result {
            let path = f.path.clone();
            if let Some(sink) = f.sink.take() {
                // The file keeps what came before. A second failure here adds nothing to the first.
                let _ = sink.finish();
            }
            self.note(Note::Failed {
                path,
                error: e.into(),
            });
        }
    }

    fn control<B>(&mut self, op: Op<B>) {
        match op {
            Op::Open {
                slot,
                path,
                encoding,
                meta,
            } => {
                self.close(slot, true);
                match self.open(&path, encoding, meta) {
                    Ok(f) => {
                        self.note(Note::Opened {
                            slot,
                            path: f.path.clone(),
                        });
                        self.files[slot] = Some(f);
                    }
                    Err((path, error)) => self.note(Note::Failed { path, error }),
                }
            }
            Op::Close { slot, keep, log } => {
                let kept = self.close(slot, keep);
                if let (Some(log), Some(file)) = (log, kept) {
                    if let Err(error) = self.line(&log, file.file_name()) {
                        self.note(Note::Failed {
                            path: log.path,
                            error,
                        });
                    }
                }
            }
            Op::Meta { slot, ext, meta } => {
                let Some(Some(f)) = self.files.get(slot) else {
                    return;
                };
                let written = f
                    .path
                    .with_extension(ext)
                    .map_err(StorageError::InvalidPath)
                    .and_then(|m| self.storage.write(&m, &meta));
                if let Err(error) = written {
                    let path = f.path.clone();
                    self.note(Note::Failed { path, error });
                }
            }
            Op::Data { .. } | Op::Finish => {}
        }
    }

    fn open(
        &self,
        path: &StorePath,
        encoding: Encoding,
        meta: Option<(&'static str, Vec<u8>)>,
    ) -> Result<OpenFile, (StorePath, StorageError)> {
        let (path, w) = self.create(path)?;
        let sink = Sink::open(w, encoding).map_err(|e| (path.clone(), e.into()))?;
        if let Some((ext, meta)) = meta {
            let m = path
                .with_extension(ext)
                .map_err(|e| (path.clone(), StorageError::InvalidPath(e)))?;
            self.storage.write(&m, &meta).map_err(|e| (m, e))?;
        }
        Ok(OpenFile {
            path,
            sink: Some(sink),
            counted: 0,
        })
    }

    fn create(
        &self,
        path: &StorePath,
    ) -> Result<(StorePath, Box<dyn StoreWriter>), (StorePath, StorageError)> {
        create(&*self.storage, path)
    }

    /// Closes the file of `slot`. Returns its path if it is kept.
    fn close(&mut self, slot: usize, keep: bool) -> Option<StorePath> {
        let f = self.files.get_mut(slot).and_then(Option::take)?;
        let bytes = f.sink.as_ref().map_or(0, |s| s.bytes);
        if let Some(sink) = f.sink {
            if let Err(error) = sink.finish() {
                self.note(Note::Failed {
                    path: f.path.clone(),
                    error,
                });
            }
        }
        let mut kept = keep;
        if !keep {
            if let Err(error) = self.storage.remove(&f.path) {
                kept = true;
                self.note(Note::Failed {
                    path: f.path.clone(),
                    error,
                });
            }
        }
        if !kept {
            self.written.fetch_sub(f.counted, Ordering::Relaxed);
        }
        self.note(Note::Closed {
            slot,
            path: f.path.clone(),
            bytes,
            kept,
        });
        kept.then_some(f.path)
    }

    fn line(&self, log: &LogLine, file: &str) -> Result<(), StorageError> {
        let new = matches!(self.storage.stat(&log.path), Err(StorageError::NotFound));
        let mut w = self.storage.open_writer(&log.path, WriteMode::Append)?;
        if new {
            w.write_all(log.header.as_bytes())?;
            w.write_all(b"\n")?;
        }
        w.write_all(log.line.as_bytes())?;
        w.write_all(b",")?;
        w.write_all(file.as_bytes())?;
        w.write_all(b"\n")?;
        w.sync()
    }
}

/// Creates `path` on `storage`, or the first free name with a number before the extension. The error names the file that
/// failed.
pub(crate) fn create(
    storage: &dyn Storage,
    path: &StorePath,
) -> Result<(StorePath, Box<dyn StoreWriter>), (StorePath, StorageError)> {
    let mut candidate = path.clone();
    for n in 2..=MAX_SUFFIX + 1 {
        match storage.open_writer(&candidate, WriteMode::CreateNew) {
            Ok(w) => return Ok((candidate, w)),
            Err(StorageError::AlreadyExists) if n <= MAX_SUFFIX => {
                candidate = numbered(path, n).map_err(|e| (path.clone(), e))?;
            }
            Err(e) => return Err((candidate, e)),
        }
    }
    Err((path.clone(), StorageError::AlreadyExists))
}

/// `path` with `_<n>` before its extension.
fn numbered(path: &StorePath, n: u32) -> Result<StorePath, StorageError> {
    let name = path.file_name();
    let (stem, ext) = match path.extension() {
        Some(ext) => (&name[..name.len() - ext.len() - 1], Some(ext)),
        None => (name, None),
    };
    let new = match ext {
        Some(ext) => format!("{stem}_{n}.{ext}"),
        None => format!("{stem}_{n}"),
    };
    let r = match path.parent() {
        Some(dir) => dir.join(&new),
        None => StorePath::new(&new),
    };
    r.map_err(StorageError::InvalidPath)
}

/// The figures that a feed shares with the control side, without a lock.
#[derive(Debug, Default)]
pub(crate) struct Live {
    /// Set when the control side requests the end of the feed.
    pub stop: std::sync::atomic::AtomicBool,
    /// The feed has ended (it was dropped, or it stopped itself).
    pub ended: std::sync::atomic::AtomicBool,
    /// Frames, samples or packets passed to the writer.
    pub frames: std::sync::atomic::AtomicU64,
    /// Frames, samples or packets left out because no block was free.
    pub dropped: std::sync::atomic::AtomicU64,
    /// The places where samples were missing before they reached the recorder (lost at the radio or before the
    /// session).
    pub gaps: std::sync::atomic::AtomicU64,
    /// The samples missing at those places.
    pub gap_samples: std::sync::atomic::AtomicU64,
    /// The sample rate of an IQ recording, as the bits of an `f64`.
    pub rate_bits: std::sync::atomic::AtomicU64,
    /// A transmission is open (gated audio).
    pub gate_open: std::sync::atomic::AtomicBool,
}

impl Live {
    pub(crate) fn stopping(&self) -> bool {
        self.stop.load(std::sync::atomic::Ordering::Relaxed)
    }
    pub(crate) fn add_frames(&self, n: u64) {
        self.frames
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }
    pub(crate) fn add_dropped(&self, n: u64) {
        self.dropped
            .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
    }
}
