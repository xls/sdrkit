//! The facilities of a host on a desktop or a phone: a folder on disk as the storage, the system clock, the random source of
//! the operating system and its network.

use sdrkit_core::storage::{StoreReader, StoreWriter};
use sdrkit_core::{
    Clock, Facilities, Random, Storage, StorageError, StoreEntry, StorePath, WriteMode,
};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub use sdrkit_core::net::StdNet;

/// A folder on disk as the host's storage. A [`StorePath`] names a file below the folder.
#[derive(Debug)]
pub struct DirStorage {
    root: PathBuf,
    /// Numbers the temporary files of [`Storage::write`].
    next: AtomicU64,
}

impl DirStorage {
    /// The storage below `root`. The folder is made if it is missing.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the folder cannot be made.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(DirStorage {
            root,
            next: AtomicU64::new(0),
        })
    }

    /// The folder.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, p: &StorePath) -> PathBuf {
        if p.is_root() {
            return self.root.clone();
        }
        let mut out = self.root.clone();
        out.extend(p.as_str().split('/'));
        out
    }

    fn parent_made(&self, p: &StorePath) -> Result<PathBuf, StorageError> {
        let path = self.path(p);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        Ok(path)
    }
}

fn modified(meta: &fs::Metadata) -> Option<f64> {
    let t = meta.modified().ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_secs_f64())
}

fn entry(name: String, meta: &fs::Metadata) -> StoreEntry {
    StoreEntry {
        name,
        size: if meta.is_dir() { 0 } else { meta.len() },
        modified: modified(meta),
        is_dir: meta.is_dir(),
    }
}

struct DiskWriter(File);

impl io::Write for DiskWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl io::Seek for DiskWriter {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl StoreWriter for DiskWriter {
    fn set_len(&mut self, len: u64) -> Result<(), StorageError> {
        Ok(self.0.set_len(len)?)
    }
    fn sync(&mut self) -> Result<(), StorageError> {
        Ok(self.0.sync_data()?)
    }
}

struct DiskReader(File);

impl io::Read for DiskReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl io::Seek for DiskReader {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }
}

impl StoreReader for DiskReader {
    fn size(&mut self) -> Result<u64, StorageError> {
        Ok(self.0.metadata()?.len())
    }
}

impl Storage for DirStorage {
    fn read(&self, path: &StorePath) -> Result<Vec<u8>, StorageError> {
        Ok(fs::read(self.path(path))?)
    }

    /// Writes a temporary file beside the file, then renames it over the file.
    fn write(&self, path: &StorePath, data: &[u8]) -> Result<(), StorageError> {
        let target = self.parent_made(path)?;
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        let mut name = target
            .file_name()
            .map(|s| s.to_os_string())
            .unwrap_or_default();
        name.push(format!(".{}-{n}.part", std::process::id()));
        let temp = target.with_file_name(name);
        let written = fs::write(&temp, data).and_then(|()| fs::rename(&temp, &target));
        if written.is_err() {
            let _ = fs::remove_file(&temp);
        }
        Ok(written?)
    }

    fn open_writer(
        &self,
        path: &StorePath,
        how: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError> {
        let target = self.parent_made(path)?;
        let mut o = OpenOptions::new();
        match how {
            WriteMode::Truncate => o.write(true).create(true).truncate(true),
            WriteMode::Append => o.append(true).create(true),
            WriteMode::Update => o.write(true).create(true).truncate(false),
            WriteMode::CreateNew => o.write(true).create_new(true),
        };
        Ok(Box::new(DiskWriter(o.open(target)?)))
    }

    fn open_reader(&self, path: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
        Ok(Box::new(DiskReader(File::open(self.path(path))?)))
    }

    fn remove(&self, path: &StorePath) -> Result<(), StorageError> {
        Ok(fs::remove_file(self.path(path))?)
    }

    fn remove_tree(&self, path: &StorePath) -> Result<(), StorageError> {
        Ok(fs::remove_dir_all(self.path(path))?)
    }

    fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError> {
        let target = self.parent_made(to)?;
        Ok(fs::rename(self.path(from), target)?)
    }

    fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
        let mut out = Vec::new();
        for e in fs::read_dir(self.path(folder))? {
            let e = e?;
            let meta = e.metadata()?;
            out.push(entry(e.file_name().to_string_lossy().into_owned(), &meta));
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn stat(&self, path: &StorePath) -> Result<StoreEntry, StorageError> {
        let full = self.path(path);
        let meta = fs::metadata(&full)?;
        let name = full
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(entry(name, &meta))
    }

    /// The standard library cannot read the free space of a disk, so this is `None`.
    fn free_space(&self) -> Result<Option<u64>, StorageError> {
        Ok(None)
    }
}

/// The wall clock of the system, in seconds since 1970 (UTC).
pub fn system_clock() -> Clock {
    Clock::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64())
    })
}

/// The random source of the operating system.
///
/// # Panics
///
/// When it is used, if the operating system provides no random bytes.
pub fn system_random() -> Random {
    Random::new(|buf| {
        if let Err(e) = getrandom::getrandom(buf) {
            panic!("the operating system gave no random bytes: {e}");
        }
    })
}

/// The facilities of a host that keeps its files in the folder `root`: the folder, the system clock, the system random source
/// and no codec plug-ins.
///
/// # Errors
///
/// [`StorageError`] if the folder cannot be made.
pub fn system_facilities(root: impl Into<PathBuf>) -> Result<Facilities, StorageError> {
    Ok(Facilities {
        storage: Arc::new(DirStorage::new(root)?),
        clock: system_clock(),
        random: system_random(),
        codecs: Arc::default(),
    })
}

/// The facilities of a host that keeps its files in memory: a [`MemStorage`](sdrkit_core::storage::MemStorage), the system clock, the system random source and
/// no codec plug-ins. The files are lost when the engine is dropped.
pub fn memory_facilities() -> Facilities {
    Facilities {
        storage: Arc::new(sdrkit_core::storage::MemStorage::new()),
        clock: system_clock(),
        random: system_random(),
        codecs: Arc::default(),
    }
}
