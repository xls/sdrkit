//! Files and folders by relative path, provided by the host. [`MemStorage`] keeps everything in memory.

use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex, MutexGuard};

/// Why a [`StorePath`] was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    /// The path is empty.
    #[error("the path is empty")]
    Empty,
    /// The path starts with `/`.
    #[error("the path starts at a root")]
    Root,
    /// Two `/` follow each other, or the path ends with `/`.
    #[error("the path has an empty segment")]
    EmptySegment,
    /// A segment is `.` or `..`.
    #[error("the path has a . or .. segment")]
    Dot,
    /// The path holds a `\`, a `:` or a NUL character, at this byte offset.
    #[error("the path has a character that is not allowed at byte {at}")]
    Char {
        /// The byte offset.
        at: usize,
    },
}

/// A relative path in a [`Storage`]: segments joined by `/`, with no root, no empty segment, no `.` or `..`, and no `\`, `:`
/// or NUL character. The empty path is the root folder, made only by [`StorePath::root`].
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct StorePath(String);

impl StorePath {
    /// The path `path`.
    ///
    /// # Errors
    ///
    /// [`PathError`] for the first rule that `path` breaks.
    pub fn new(path: &str) -> Result<Self, PathError> {
        if path.is_empty() {
            return Err(PathError::Empty);
        }
        if path.starts_with('/') {
            return Err(PathError::Root);
        }
        if let Some(at) = path.find(['\\', ':', '\0']) {
            return Err(PathError::Char { at });
        }
        for seg in path.split('/') {
            match seg {
                "" => return Err(PathError::EmptySegment),
                "." | ".." => return Err(PathError::Dot),
                _ => {}
            }
        }
        Ok(StorePath(path.to_owned()))
    }

    /// The root folder.
    pub fn root() -> Self {
        StorePath(String::new())
    }

    /// True for the root folder.
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// The path as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This path with `rest` (one or more segments) added.
    ///
    /// # Errors
    ///
    /// [`PathError`] if `rest` is not a valid path.
    pub fn join(&self, rest: &str) -> Result<Self, PathError> {
        let rest = StorePath::new(rest)?;
        if self.is_root() {
            return Ok(rest);
        }
        Ok(StorePath(format!("{}/{}", self.0, rest.0)))
    }

    /// The folder that holds this path. `None` for the root.
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        Some(match self.0.rfind('/') {
            Some(i) => StorePath(self.0[..i].to_owned()),
            None => StorePath::root(),
        })
    }

    /// The last segment. Empty for the root.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or("")
    }

    /// The part of the last segment after its last `.`. `None` if it has no `.` after its first character.
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        name.rfind('.').filter(|&i| i > 0).map(|i| &name[i + 1..])
    }

    /// This path with the extension of its last segment replaced by `ext`, or removed if `ext` is empty.
    ///
    /// # Errors
    ///
    /// [`PathError`] if the result is not a valid path.
    pub fn with_extension(&self, ext: &str) -> Result<Self, PathError> {
        let name = self.file_name();
        let stem_len = name.rfind('.').filter(|&i| i > 0).unwrap_or(name.len());
        let base = &self.0[..self.0.len() - name.len() + stem_len];
        if ext.is_empty() {
            StorePath::new(base)
        } else {
            StorePath::new(&format!("{base}.{ext}"))
        }
    }

    /// True if this path is `folder` or lies inside it.
    pub fn starts_with(&self, folder: &StorePath) -> bool {
        folder.is_root()
            || self.0 == folder.0
            || (self.0.starts_with(&folder.0)
                && self.0.as_bytes().get(folder.0.len()) == Some(&b'/'))
    }
}

impl std::fmt::Display for StorePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for StorePath {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            return Ok(StorePath::root());
        }
        StorePath::new(&s).map_err(serde::de::Error::custom)
    }
}

/// Why a storage operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    /// The file or folder does not exist.
    #[error("not found")]
    NotFound,
    /// The file already exists.
    #[error("already exists")]
    AlreadyExists,
    /// The storage is full.
    #[error("no space left")]
    NoSpace,
    /// The host does not allow the operation.
    #[error("access denied")]
    Denied,
    /// The path is not valid.
    #[error("invalid path: {0}")]
    InvalidPath(PathError),
    /// Another input or output error.
    #[error("input or output error: {kind}")]
    Io {
        /// The kind of the error.
        kind: io::ErrorKind,
    },
}

impl From<PathError> for StorageError {
    fn from(e: PathError) -> Self {
        StorageError::InvalidPath(e)
    }
}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        match e.kind() {
            io::ErrorKind::NotFound => StorageError::NotFound,
            io::ErrorKind::AlreadyExists => StorageError::AlreadyExists,
            io::ErrorKind::StorageFull => StorageError::NoSpace,
            io::ErrorKind::PermissionDenied => StorageError::Denied,
            kind => e
                .get_ref()
                .and_then(|inner| inner.downcast_ref::<StorageError>())
                .copied()
                .unwrap_or(StorageError::Io { kind }),
        }
    }
}

impl From<StorageError> for io::Error {
    fn from(e: StorageError) -> Self {
        let kind = match e {
            StorageError::NotFound => io::ErrorKind::NotFound,
            StorageError::AlreadyExists => io::ErrorKind::AlreadyExists,
            StorageError::NoSpace => io::ErrorKind::StorageFull,
            StorageError::Denied => io::ErrorKind::PermissionDenied,
            StorageError::InvalidPath(_) => io::ErrorKind::InvalidInput,
            StorageError::Io { kind } => kind,
        };
        io::Error::new(kind, e)
    }
}

/// One entry of a folder, or the state of one path.
#[derive(Clone, Debug, PartialEq)]
pub struct StoreEntry {
    /// The last segment of the path.
    pub name: String,
    /// The length in bytes. 0 for a folder.
    pub size: u64,
    /// The time of the last change, seconds since 1970 (UTC). `None` if the host does not know it.
    pub modified: Option<f64>,
    /// True for a folder.
    pub is_dir: bool,
}

/// How [`Storage::open_writer`] opens a file. Every write mode makes missing parent folders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteMode {
    /// Make the file, or empty it if it exists.
    Truncate,
    /// Make the file if it is missing. Every write goes to the end.
    Append,
    /// Make the file if it is missing and keep its bytes. Writes go to the position, which starts at 0.
    Update,
    /// Make the file. If it exists, fail with [`StorageError::AlreadyExists`].
    CreateNew,
}

/// A file open for writing.
pub trait StoreWriter: Write + Seek + Send {
    /// Cuts or extends the file to `len` bytes. The position does not change.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the storage refuses.
    fn set_len(&mut self, len: u64) -> Result<(), StorageError>;
    /// Makes sure that every write so far is durable.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the storage refuses.
    fn sync(&mut self) -> Result<(), StorageError>;
}

/// A file open for reading, at any position.
pub trait StoreReader: Read + Seek + Send {
    /// The length of the file in bytes.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the storage refuses.
    fn size(&mut self) -> Result<u64, StorageError>;
}

/// Files and folders by [`StorePath`], provided by the host. Writes make missing parent folders.
pub trait Storage: Send + Sync {
    /// The whole file.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn read(&self, path: &StorePath) -> Result<Vec<u8>, StorageError>;
    /// Replaces the file with `data` as one step: a reader sees the old bytes or the new bytes, never a part.
    ///
    /// # Errors
    ///
    /// [`StorageError`]. On an error the old file stays.
    fn write(&self, path: &StorePath, data: &[u8]) -> Result<(), StorageError>;
    /// Opens a file for writing in `mode`.
    ///
    /// # Errors
    ///
    /// [`StorageError`].
    fn open_writer(
        &self,
        path: &StorePath,
        mode: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError>;
    /// Opens a file for reading.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn open_reader(&self, path: &StorePath) -> Result<Box<dyn StoreReader>, StorageError>;
    /// Removes a file.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn remove(&self, path: &StorePath) -> Result<(), StorageError>;
    /// Removes a folder and everything in it.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn remove_tree(&self, path: &StorePath) -> Result<(), StorageError>;
    /// Moves a file to `to`, and replaces a file that is there.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError>;
    /// The entries of a folder, sorted by name.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError>;
    /// The state of one file or folder.
    ///
    /// # Errors
    ///
    /// [`StorageError`], for example [`StorageError::NotFound`].
    fn stat(&self, path: &StorePath) -> Result<StoreEntry, StorageError>;
    /// The bytes free for new data. `None` if the host does not know.
    ///
    /// # Errors
    ///
    /// [`StorageError`].
    fn free_space(&self) -> Result<Option<u64>, StorageError>;
}

#[derive(Debug, Default)]
struct MemFile {
    data: Vec<u8>,
    modified: f64,
}

type FileRef = Arc<Mutex<MemFile>>;

#[derive(Debug, Default)]
struct MemInner {
    files: BTreeMap<StorePath, FileRef>,
    limit: Option<u64>,
    fail_next: Option<StorageError>,
    now: f64,
}

impl MemInner {
    fn used(&self) -> u64 {
        self.files.values().map(|f| lock(f).data.len() as u64).sum()
    }
    fn take_failure(&mut self) -> Result<(), StorageError> {
        self.fail_next.take().map_or(Ok(()), Err)
    }
    /// Makes sure that `grow` more bytes fit.
    fn room(&self, grow: u64) -> Result<(), StorageError> {
        match self.limit {
            Some(limit) if self.used().saturating_add(grow) > limit => Err(StorageError::NoSpace),
            _ => Ok(()),
        }
    }
    fn is_dir(&self, path: &StorePath) -> bool {
        self.files.keys().any(|p| p != path && p.starts_with(path))
    }
    fn file(&self, path: &StorePath) -> Result<FileRef, StorageError> {
        if path.is_root() {
            return Err(IS_A_DIRECTORY);
        }
        self.files.get(path).cloned().ok_or(StorageError::NotFound)
    }
}

const IS_A_DIRECTORY: StorageError = StorageError::Io {
    kind: io::ErrorKind::IsADirectory,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A [`Storage`] in memory, for tests. It can have a size limit, and it can make the next operation fail.
#[derive(Clone, Debug, Default)]
pub struct MemStorage {
    inner: Arc<Mutex<MemInner>>,
}

impl MemStorage {
    /// An empty storage with no size limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the most bytes that all files together can hold. A write past it fails with [`StorageError::NoSpace`].
    pub fn set_limit(&self, bytes: Option<u64>) {
        lock(&self.inner).limit = bytes;
    }

    /// Makes the next operation (of the storage, a writer or a reader) fail with `e`.
    pub fn fail_next(&self, e: StorageError) {
        lock(&self.inner).fail_next = Some(e);
    }

    /// Sets the time that changes from now on record, seconds since 1970 (UTC). It starts at 0.
    pub fn set_time(&self, unix_s: f64) {
        lock(&self.inner).now = unix_s;
    }

    fn inner(&self) -> Result<MutexGuard<'_, MemInner>, StorageError> {
        let mut g = lock(&self.inner);
        g.take_failure()?;
        Ok(g)
    }
}

impl Storage for MemStorage {
    fn read(&self, path: &StorePath) -> Result<Vec<u8>, StorageError> {
        let g = self.inner()?;
        let f = g.file(path)?;
        let data = lock(&f).data.clone();
        Ok(data)
    }

    fn write(&self, path: &StorePath, data: &[u8]) -> Result<(), StorageError> {
        let mut g = self.inner()?;
        if path.is_root() || g.is_dir(path) {
            return Err(IS_A_DIRECTORY);
        }
        let old = g.files.get(path).map_or(0, |f| lock(f).data.len() as u64);
        g.room((data.len() as u64).saturating_sub(old))?;
        let modified = g.now;
        g.files.insert(
            path.clone(),
            Arc::new(Mutex::new(MemFile {
                data: data.to_vec(),
                modified,
            })),
        );
        Ok(())
    }

    fn open_writer(
        &self,
        path: &StorePath,
        mode: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError> {
        let mut g = self.inner()?;
        if path.is_root() || g.is_dir(path) {
            return Err(IS_A_DIRECTORY);
        }
        let now = g.now;
        let file = match (g.files.get(path).cloned(), mode) {
            (Some(_), WriteMode::CreateNew) => return Err(StorageError::AlreadyExists),
            (Some(f), WriteMode::Truncate) => {
                {
                    let mut m = lock(&f);
                    m.data.clear();
                    m.modified = now;
                }
                f
            }
            (Some(f), _) => f,
            (None, _) => {
                let f: FileRef = Arc::new(Mutex::new(MemFile {
                    data: Vec::new(),
                    modified: now,
                }));
                g.files.insert(path.clone(), Arc::clone(&f));
                f
            }
        };
        Ok(Box::new(MemWriter {
            store: Arc::clone(&self.inner),
            file,
            pos: 0,
            append: mode == WriteMode::Append,
        }))
    }

    fn open_reader(&self, path: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
        let g = self.inner()?;
        let file = g.file(path)?;
        Ok(Box::new(MemReader {
            store: Arc::clone(&self.inner),
            file,
            pos: 0,
        }))
    }

    fn remove(&self, path: &StorePath) -> Result<(), StorageError> {
        let mut g = self.inner()?;
        g.file(path)?;
        g.files.remove(path);
        Ok(())
    }

    fn remove_tree(&self, path: &StorePath) -> Result<(), StorageError> {
        let mut g = self.inner()?;
        if !g.is_dir(path) {
            return Err(StorageError::NotFound);
        }
        g.files.retain(|p, _| !p.starts_with(path));
        Ok(())
    }

    fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError> {
        let mut g = self.inner()?;
        let f = g.file(from)?;
        if to.is_root() || g.is_dir(to) {
            return Err(IS_A_DIRECTORY);
        }
        g.files.remove(from);
        g.files.insert(to.clone(), f);
        Ok(())
    }

    fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
        let g = self.inner()?;
        if !folder.is_root() && !g.is_dir(folder) {
            return Err(if g.files.contains_key(folder) {
                StorageError::Io {
                    kind: io::ErrorKind::NotADirectory,
                }
            } else {
                StorageError::NotFound
            });
        }
        let skip = if folder.is_root() {
            0
        } else {
            folder.as_str().len() + 1
        };
        let mut out: BTreeMap<String, StoreEntry> = BTreeMap::new();
        for (p, f) in g
            .files
            .iter()
            .filter(|(p, _)| p.starts_with(folder) && *p != folder)
        {
            let rest = &p.as_str()[skip..];
            let entry = match rest.split_once('/') {
                Some((dir, _)) => StoreEntry {
                    name: dir.to_owned(),
                    size: 0,
                    modified: None,
                    is_dir: true,
                },
                None => {
                    let m = lock(f);
                    StoreEntry {
                        name: rest.to_owned(),
                        size: m.data.len() as u64,
                        modified: Some(m.modified),
                        is_dir: false,
                    }
                }
            };
            out.entry(entry.name.clone()).or_insert(entry);
        }
        Ok(out.into_values().collect())
    }

    fn stat(&self, path: &StorePath) -> Result<StoreEntry, StorageError> {
        let g = self.inner()?;
        if path.is_root() || g.is_dir(path) {
            return Ok(StoreEntry {
                name: path.file_name().to_owned(),
                size: 0,
                modified: None,
                is_dir: true,
            });
        }
        let f = g.file(path)?;
        let m = lock(&f);
        Ok(StoreEntry {
            name: path.file_name().to_owned(),
            size: m.data.len() as u64,
            modified: Some(m.modified),
            is_dir: false,
        })
    }

    fn free_space(&self) -> Result<Option<u64>, StorageError> {
        let g = self.inner()?;
        Ok(g.limit.map(|l| l.saturating_sub(g.used())))
    }
}

struct MemWriter {
    store: Arc<Mutex<MemInner>>,
    file: FileRef,
    pos: u64,
    append: bool,
}

impl Write for MemWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut g = lock(&self.store);
        g.take_failure()?;
        let len = lock(&self.file).data.len() as u64;
        let at = if self.append { len } else { self.pos };
        let end = at + buf.len() as u64;
        g.room(end.saturating_sub(len))?;
        let now = g.now;
        let mut f = lock(&self.file);
        let (at, end) = (at as usize, end as usize);
        if f.data.len() < end {
            f.data.resize(end, 0);
        }
        f.data[at..end].copy_from_slice(buf);
        f.modified = now;
        self.pos = end as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        lock(&self.store).take_failure()?;
        Ok(())
    }
}

impl Seek for MemWriter {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        lock(&self.store).take_failure()?;
        let len = lock(&self.file).data.len() as u64;
        self.pos = seek_to(self.pos, len, to)?;
        Ok(self.pos)
    }
}

impl StoreWriter for MemWriter {
    fn set_len(&mut self, len: u64) -> Result<(), StorageError> {
        let mut g = lock(&self.store);
        g.take_failure()?;
        let old = lock(&self.file).data.len() as u64;
        g.room(len.saturating_sub(old))?;
        let now = g.now;
        let mut f = lock(&self.file);
        f.data.resize(len as usize, 0);
        f.modified = now;
        Ok(())
    }
    fn sync(&mut self) -> Result<(), StorageError> {
        lock(&self.store).take_failure()
    }
}

struct MemReader {
    store: Arc<Mutex<MemInner>>,
    file: FileRef,
    pos: u64,
}

impl Read for MemReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        lock(&self.store).take_failure()?;
        let f = lock(&self.file);
        let at = (self.pos as usize).min(f.data.len());
        let n = buf.len().min(f.data.len() - at);
        buf[..n].copy_from_slice(&f.data[at..at + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for MemReader {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        lock(&self.store).take_failure()?;
        let len = lock(&self.file).data.len() as u64;
        self.pos = seek_to(self.pos, len, to)?;
        Ok(self.pos)
    }
}

impl StoreReader for MemReader {
    fn size(&mut self) -> Result<u64, StorageError> {
        lock(&self.store).take_failure()?;
        Ok(lock(&self.file).data.len() as u64)
    }
}

fn seek_to(pos: u64, len: u64, to: SeekFrom) -> io::Result<u64> {
    let target = match to {
        SeekFrom::Start(p) => Some(p),
        SeekFrom::End(d) => len.checked_add_signed(d),
        SeekFrom::Current(d) => pos.checked_add_signed(d),
    };
    target.ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> StorePath {
        StorePath::new(s).unwrap()
    }

    #[test]
    fn a_path_refuses_parents_roots_empty_segments_and_backslashes() {
        assert_eq!(StorePath::new("a/../b"), Err(PathError::Dot));
        assert_eq!(StorePath::new("/etc/passwd"), Err(PathError::Root));
        assert_eq!(StorePath::new("a//b"), Err(PathError::EmptySegment));
        assert_eq!(StorePath::new("a/"), Err(PathError::EmptySegment));
        assert_eq!(StorePath::new(""), Err(PathError::Empty));
        assert_eq!(StorePath::new("a\\b"), Err(PathError::Char { at: 1 }));
        assert_eq!(StorePath::new("C:x"), Err(PathError::Char { at: 1 }));
        let r = p("rec/x.cs8");
        assert_eq!(
            r.with_extension("sigmf-meta").unwrap(),
            p("rec/x.sigmf-meta")
        );
        assert_eq!(r.parent(), Some(p("rec")));
        assert_eq!(r.extension(), Some("cs8"));
        assert_eq!(StorePath::root().join("a/b").unwrap(), p("a/b"));
        assert!(!p("recx/a").starts_with(&p("rec")));
    }

    #[test]
    fn write_replaces_and_append_and_seek_work() {
        let s = MemStorage::new();
        let f = p("logs/a.txt");
        s.write(&f, b"old").unwrap();
        s.write(&f, b"new!").unwrap();
        assert_eq!(s.read(&f).unwrap(), b"new!");

        let mut w = s.open_writer(&f, WriteMode::Append).unwrap();
        w.write_all(b"12").unwrap();
        w.seek(SeekFrom::Start(0)).unwrap();
        w.write_all(b"3").unwrap();
        assert_eq!(s.read(&f).unwrap(), b"new!123");

        let mut u = s.open_writer(&f, WriteMode::Update).unwrap();
        u.seek(SeekFrom::End(-2)).unwrap();
        u.write_all(b"xy").unwrap();
        u.set_len(3).unwrap();
        assert_eq!(s.read(&f).unwrap(), b"new");
        assert!(matches!(
            s.open_writer(&f, WriteMode::CreateNew),
            Err(StorageError::AlreadyExists)
        ));

        let mut r = s.open_reader(&f).unwrap();
        r.seek(SeekFrom::Start(1)).unwrap();
        let mut b = [0u8; 2];
        r.read_exact(&mut b).unwrap();
        assert_eq!(&b, b"ew");
        assert_eq!(r.size(), Ok(3));
    }

    #[test]
    fn rename_list_and_remove_tree() {
        let s = MemStorage::new();
        s.set_time(100.0);
        s.write(&p("a/one"), b"1").unwrap();
        s.write(&p("a/sub/two"), b"22").unwrap();
        s.rename(&p("a/one"), &p("a/three")).unwrap();
        assert_eq!(s.read(&p("a/one")), Err(StorageError::NotFound));
        let names: Vec<(String, bool)> = s
            .list(&p("a"))
            .unwrap()
            .into_iter()
            .map(|e| (e.name, e.is_dir))
            .collect();
        assert_eq!(names, [("sub".into(), true), ("three".into(), false)]);
        assert_eq!(s.stat(&p("a/three")).unwrap().modified, Some(100.0));
        s.remove_tree(&p("a")).unwrap();
        assert_eq!(s.list(&StorePath::root()).unwrap(), []);
        assert_eq!(s.remove(&p("a/three")), Err(StorageError::NotFound));
    }

    #[test]
    fn a_limit_returns_no_space_and_keeps_the_old_file() {
        let s = MemStorage::new();
        s.set_limit(Some(8));
        let f = p("f");
        s.write(&f, b"1234").unwrap();
        assert_eq!(s.free_space(), Ok(Some(4)));
        assert_eq!(s.write(&f, b"123456789"), Err(StorageError::NoSpace));
        assert_eq!(s.read(&f).unwrap(), b"1234");
        let mut w = s.open_writer(&p("g"), WriteMode::Truncate).unwrap();
        let e = w.write_all(b"12345").unwrap_err();
        assert_eq!(StorageError::from(e), StorageError::NoSpace);
    }

    #[test]
    fn an_injected_failure_hits_the_next_operation_only() {
        let s = MemStorage::new();
        let f = p("f");
        s.write(&f, b"1").unwrap();
        s.fail_next(StorageError::Denied);
        assert_eq!(s.read(&f), Err(StorageError::Denied));
        assert_eq!(s.read(&f).unwrap(), b"1");
        let mut w = s.open_writer(&f, WriteMode::Append).unwrap();
        s.fail_next(StorageError::Io {
            kind: io::ErrorKind::Interrupted,
        });
        let e = w.write(b"2").unwrap_err();
        assert_eq!(
            StorageError::from(e),
            StorageError::Io {
                kind: io::ErrorKind::Interrupted
            }
        );
        let storage: Arc<dyn Storage> = Arc::new(s);
        assert_eq!(storage.read(&f).unwrap(), b"1");
    }
}
