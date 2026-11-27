//! The files of the message store: the journal of one JSON line per message, the older journal that a rotation keeps,
//! and a small side index. An open reads the side index and a bounded tail of the journal; a full load runs on a
//! thread of its own.

use std::collections::BTreeMap;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::sync::{Arc, Mutex};

use sdrkit_core::storage::Storage;
use sdrkit_core::{StorageError, StorePath};
use sdrkit_svc_events::Kinds;
use serde::{Deserialize, Serialize};

use crate::error::StoreError;
use crate::message::Message;

/// The bytes at the end of the journal that an open reads: the newest messages, for the first searches and the throttle.
pub(crate) const TAIL_BYTES: u64 = 256 << 10;
/// The most bytes after the point of the side index that an open reads. A longer stretch needs a full load.
const MAX_GAP: u64 = 1 << 20;
/// The side index is written again after this many bytes of new lines.
pub(crate) const INDEX_EVERY: u64 = 64 << 10;

/// The three files of a store.
#[derive(Clone)]
pub(crate) struct Files {
    /// The journal that takes new lines.
    pub(crate) current: StorePath,
    /// The journal before the last rotation.
    pub(crate) older: StorePath,
    /// The side index.
    pub(crate) index: StorePath,
}

impl Files {
    pub(crate) fn of(path: &StorePath) -> Result<Files, StoreError> {
        let near = |suffix: &str| {
            StorePath::new(&format!("{}{suffix}", path.as_str())).map_err(|e| StoreError::Storage {
                path: path.clone(),
                source: StorageError::InvalidPath(e),
            })
        };
        Ok(Files {
            current: path.clone(),
            older: near(".1")?,
            index: near(".idx")?,
        })
    }
}

/// What the files hold, as the side index keeps it. A row is a line whose message id has not been in an earlier line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Disk {
    /// The bytes of the current journal.
    pub(crate) len: u64,
    /// The id of the next new message.
    pub(crate) next_id: u64,
    /// The rows of the current journal.
    pub(crate) rows: u64,
    /// The rows of the older journal.
    pub(crate) old_rows: u64,
    /// The bytes of the older journal.
    pub(crate) old_bytes: u64,
}

impl Disk {
    pub(crate) fn read(storage: &dyn Storage, files: &Files) -> Option<Disk> {
        serde_json::from_slice(&storage.read(&files.index).ok()?).ok()
    }

    pub(crate) fn write(&self, storage: &dyn Storage, files: &Files) -> Result<(), StoreError> {
        let data = serde_json::to_vec(self)?;
        storage
            .write(&files.index, &data)
            .map_err(|source| StoreError::Storage {
                path: files.index.clone(),
                source,
            })
    }
}

fn size_of(storage: &dyn Storage, path: &StorePath) -> Result<u64, StoreError> {
    match storage.stat(path) {
        Ok(e) => Ok(e.size),
        Err(StorageError::NotFound) => Ok(0),
        Err(source) => Err(StoreError::Storage {
            path: path.clone(),
            source,
        }),
    }
}

/// Bytes `from..to` of a file.
fn read_range(
    storage: &dyn Storage,
    path: &StorePath,
    from: u64,
    to: u64,
) -> Result<Vec<u8>, StoreError> {
    let err = |source| StoreError::Storage {
        path: path.clone(),
        source,
    };
    let mut out = Vec::new();
    if to <= from {
        return Ok(out);
    }
    let mut r = storage.open_reader(path).map_err(err)?;
    r.seek(SeekFrom::Start(from)).map_err(|e| err(e.into()))?;
    r.take(to - from)
        .read_to_end(&mut out)
        .map_err(|e| err(e.into()))?;
    Ok(out)
}

/// Calls `each` with the start offset and the message of every line of `data`, which starts at offset `base` of its
/// file. `None` for a line that does not parse. Empty lines are skipped.
fn each_line(data: &[u8], base: u64, kinds: &Kinds, mut each: impl FnMut(u64, Option<Message>)) {
    let mut at = 0;
    for raw in data.split_inclusive(|b| *b == b'\n') {
        let start = base + at as u64;
        at += raw.len();
        let line = String::from_utf8_lossy(raw);
        let line = line.trim();
        if !line.is_empty() {
            each(start, Message::from_line(line, kinds));
        }
    }
}

/// What an open reads.
pub(crate) struct Opened {
    /// The counts of the files, with the lines after the point of the side index.
    pub(crate) disk: Disk,
    /// The side index was there and fits the journal: `disk.next_id` is exact.
    pub(crate) indexed: bool,
    /// The newest messages, by id, the last line of an id winning.
    pub(crate) tail: BTreeMap<u64, Message>,
    /// The tail covers every line of every file.
    pub(crate) whole: bool,
    /// Lines of the tail that do not parse.
    pub(crate) bad_lines: u64,
    /// The journal ends inside a line: the last write was cut off.
    pub(crate) torn: bool,
}

/// Reads the side index and at most [`TAIL_BYTES`] (or the stretch after the point of the index, up to [`MAX_GAP`]) of
/// the end of the journal. The cost does not depend on the size of the files.
pub(crate) fn open(
    storage: &dyn Storage,
    files: &Files,
    kinds: &Kinds,
) -> Result<Opened, StoreError> {
    let size = size_of(storage, &files.current)?;
    let old_bytes = size_of(storage, &files.older)?;
    let index = Disk::read(storage, files).filter(|d| d.len <= size && size - d.len <= MAX_GAP);
    let tail_from = size.saturating_sub(TAIL_BYTES);
    let from = index.map_or(tail_from, |d| d.len.min(tail_from));
    // One byte before `from` says whether `from` starts a line.
    let data = read_range(storage, &files.current, from.saturating_sub(1), size)?;
    let (data, base) = if from == 0 {
        (&data[..], 0)
    } else if data.first() == Some(&b'\n') {
        (&data[1..], from)
    } else {
        // `from` is inside a line: the tail starts after it.
        let cut = data
            .iter()
            .position(|b| *b == b'\n')
            .map_or(data.len(), |i| i + 1);
        (&data[cut..], from.saturating_sub(1) + cut as u64)
    };
    let mut disk = index.unwrap_or_default();
    disk.len = size;
    disk.old_bytes = old_bytes;
    let mut tail = BTreeMap::new();
    let mut bad_lines = 0;
    let mut top = 0;
    each_line(data, base, kinds, |start, m| match m {
        Some(m) => {
            if let Some(d) = index {
                if start >= d.len && m.id >= d.next_id && !tail.contains_key(&m.id) {
                    disk.rows += 1;
                }
            }
            top = top.max(m.id);
            tail.insert(m.id, m);
        }
        None => bad_lines += 1,
    });
    disk.next_id = disk.next_id.max(top + 1).max(1);
    let whole = base == 0 && old_bytes == 0;
    if whole {
        disk.rows = tail.len() as u64;
        disk.old_rows = 0;
    }
    Ok(Opened {
        disk,
        indexed: index.is_some() || whole,
        tail,
        whole,
        bad_lines,
        torn: data.last().is_some_and(|b| *b != b'\n'),
    })
}

/// The result of a full load.
pub(crate) struct Loaded {
    /// The clear that the load belongs to: a load that a clear overtook is dropped.
    pub(crate) generation: u64,
    /// Every message, by id, the last line of an id winning.
    pub(crate) messages: BTreeMap<u64, Message>,
    /// Lines that do not parse.
    pub(crate) bad_lines: u64,
    /// The rows of the current journal, up to the length that the load read.
    pub(crate) rows: u64,
    /// The rows of the older journal.
    pub(crate) old_rows: u64,
}

/// Reads the older journal and the first `upto` bytes of the current one.
pub(crate) fn load(
    storage: &dyn Storage,
    files: &Files,
    kinds: &Kinds,
    upto: u64,
    generation: u64,
) -> Result<Loaded, StoreError> {
    let mut out = Loaded {
        generation,
        messages: BTreeMap::new(),
        bad_lines: 0,
        rows: 0,
        old_rows: 0,
    };
    let old_size = size_of(storage, &files.older)?;
    for (path, to, older) in [
        (&files.older, old_size, true),
        (&files.current, upto, false),
    ] {
        let data = read_range(storage, path, 0, to)?;
        each_line(&data, 0, kinds, |_, m| match m {
            Some(m) => {
                if out.messages.insert(m.id, m).is_none() {
                    if older {
                        out.old_rows += 1;
                    } else {
                        out.rows += 1;
                    }
                }
            }
            None => out.bad_lines += 1,
        });
    }
    Ok(out)
}

/// A full load on a thread of its own. The store takes the result when it is there.
pub(crate) struct Loading {
    pub(crate) slot: Arc<Mutex<Option<Result<Loaded, StoreError>>>>,
    pub(crate) thread: Option<std::thread::JoinHandle<()>>,
    /// The rows that the store wrote to the current journal before the load started.
    pub(crate) rows_before: u64,
}

impl Loading {
    pub(crate) fn start(
        storage: Arc<dyn Storage>,
        files: Files,
        kinds: Arc<Kinds>,
        upto: u64,
        generation: u64,
        rows_before: u64,
    ) -> Loading {
        let slot = Arc::new(Mutex::new(None));
        let put = Arc::clone(&slot);
        let (s, f, k) = (Arc::clone(&storage), files.clone(), Arc::clone(&kinds));
        let run = move || {
            let r = load(&*s, &f, &k, upto, generation);
            *put.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(r);
        };
        let thread = std::thread::Builder::new()
            .name("sdrkit-store-load".into())
            .spawn(run)
            .ok();
        if thread.is_none() {
            // No thread: the load runs here.
            let r = load(&*storage, &files, &kinds, upto, generation);
            *slot
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(r);
        }
        Loading {
            slot,
            thread,
            rows_before,
        }
    }

    /// The result, if the load has ended. `wait` waits for it.
    pub(crate) fn take(&mut self, wait: bool) -> Option<Result<Loaded, StoreError>> {
        if wait {
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
        self.slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}
