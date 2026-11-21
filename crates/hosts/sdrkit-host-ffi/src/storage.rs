//! Storage of the caller: a table of callbacks through which the engine reads and writes its files.

use crate::engine::make_engine;
use crate::error::{
    Error, SDRKIT_E_ALREADY_EXISTS, SDRKIT_E_DENIED, SDRKIT_E_IO, SDRKIT_E_NOT_FOUND,
    SDRKIT_E_NO_SPACE, SDRKIT_OK,
};
use crate::handle::{run_free, sized_in, Handle, Out};
use sdrkit_core::storage::{StoreReader, StoreWriter};
use sdrkit_core::{Facilities, Storage, StorageError, StoreEntry, StorePath, WriteMode};
use std::ffi::{c_char, c_void, CStr, CString};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::Arc;

/// Opens the file for reading. The callback returns `SDRKIT_E_NOT_FOUND` if the file does not exist.
pub const SDRKIT_OPEN_READ: i32 = 0;
/// Opens the file for writing. The callback makes the file, or empties it if it exists.
pub const SDRKIT_OPEN_TRUNCATE: i32 = 1;
/// Opens the file for writing at the end. The callback makes the file if it does not exist, and keeps its bytes.
pub const SDRKIT_OPEN_APPEND: i32 = 2;
/// Opens the file for writing at any offset. The callback makes the file if it does not exist, and keeps its bytes.
pub const SDRKIT_OPEN_UPDATE: i32 = 3;
/// Makes the file and opens it for writing. The callback returns `SDRKIT_E_ALREADY_EXISTS` if the file exists.
pub const SDRKIT_OPEN_CREATE_NEW: i32 = 4;

/// One entry of a folder, or the state of one path.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HostEntry {
    /// The number of bytes of the struct. The side that makes the struct sets it to the sizeof of its struct: the
    /// callback for an entry that it adds, the engine for the entry that `stat` fills.
    pub size: u32,
    /// The last segment of the path, a NUL-terminated string. The engine copies it before the callback returns.
    pub name: *const c_char,
    /// The length in bytes. 0 for a folder.
    pub len: u64,
    /// The time of the last change in seconds since 1970 (UTC), NaN if not known.
    pub modified: f64,
    /// True for a folder.
    pub is_dir: bool,
}

/// Opens the file at `path` as `open_as` (an `SDRKIT_OPEN_*` constant) and writes a number for it to `file`. Opening for
/// writing makes the missing folders of the path.
pub type OpenFn = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        path: *const c_char,
        open_as: i32,
        file: *mut u64,
    ) -> i32,
>;
/// Reads at most `capacity` bytes at `offset` of `file` into `buffer`, and writes the number of bytes read to `got`: 0 at
/// the end of the file.
pub type ReadFn = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        file: u64,
        offset: u64,
        buffer: *mut u8,
        capacity: usize,
        got: *mut usize,
    ) -> i32,
>;
/// Writes all `size` bytes of `data` at `offset` of `file`.
pub type WriteFn = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        file: u64,
        offset: u64,
        data: *const u8,
        size: usize,
    ) -> i32,
>;
/// Writes the length of `file` to `size`.
pub type LenFn =
    Option<unsafe extern "C" fn(context: *mut c_void, file: u64, size: *mut u64) -> i32>;
/// Cuts or extends `file` to `size` bytes.
pub type SetLenFn = Option<unsafe extern "C" fn(context: *mut c_void, file: u64, size: u64) -> i32>;
/// Makes every write to `file` durable.
pub type SyncFn = Option<unsafe extern "C" fn(context: *mut c_void, file: u64) -> i32>;
/// Closes `file`.
pub type CloseFn = Option<unsafe extern "C" fn(context: *mut c_void, file: u64)>;
/// Removes the file at `path`, or, if `tree` is true, the folder at `path` and everything in it.
pub type RemoveFn =
    Option<unsafe extern "C" fn(context: *mut c_void, path: *const c_char, tree: bool) -> i32>;
/// Moves the file at `from` to `to`. If a file exists at `to`, the callback replaces it.
pub type RenameFn = Option<
    unsafe extern "C" fn(context: *mut c_void, from: *const c_char, to: *const c_char) -> i32,
>;
/// Adds one entry of a folder to the list of the engine. The entry has its `size` set: use `SDRKIT_STORE_ENTRY_INIT`.
/// The engine does not add an entry with a `size` below 4.
pub type AddEntryFn = Option<unsafe extern "C" fn(sink: *mut c_void, entry: *const HostEntry)>;
/// Calls `add` with `sink` once for each entry of the folder at `folder`.
pub type ListFn = Option<
    unsafe extern "C" fn(
        context: *mut c_void,
        folder: *const c_char,
        sink: *mut c_void,
        add: AddEntryFn,
    ) -> i32,
>;
/// Fills `entry` with the state of the file or folder at `path`. The engine sets `size`, sets `modified` to
/// NaN and the other members to 0 before the call. The callback writes the members that lie within `size`.
pub type StatFn = Option<
    unsafe extern "C" fn(context: *mut c_void, path: *const c_char, entry: *mut HostEntry) -> i32,
>;
/// Writes the number of free bytes to `bytes`, and writes false to `known` if the number is not known.
pub type FreeSpaceFn =
    Option<unsafe extern "C" fn(context: *mut c_void, bytes: *mut u64, known: *mut bool) -> i32>;
/// Called once when the engine stops using the storage.
pub type ReleaseFn = Option<unsafe extern "C" fn(context: *mut c_void)>;

/// The storage callbacks of the caller. Paths are relative, use `/` between segments, and never hold `..`. Each callback
/// returns `SDRKIT_OK`, or `SDRKIT_E_NOT_FOUND`, `SDRKIT_E_ALREADY_EXISTS`, `SDRKIT_E_NO_SPACE`, `SDRKIT_E_DENIED` or
/// `SDRKIT_E_IO`. The engine calls them from any of its threads. A callback must not call a function of this library.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct HostStorage {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct.
    pub size: u32,
    /// The first argument of every callback.
    pub context: *mut c_void,
    /// Opens a file.
    pub open: OpenFn,
    /// Reads from a file.
    pub read: ReadFn,
    /// Writes to a file.
    pub write: WriteFn,
    /// The length of a file.
    pub len: LenFn,
    /// Cuts or extends a file.
    pub set_len: SetLenFn,
    /// Makes the writes to a file durable.
    pub sync: SyncFn,
    /// Closes a file.
    pub close: CloseFn,
    /// Removes a file or a folder.
    pub remove: RemoveFn,
    /// Moves a file.
    pub rename: RenameFn,
    /// Lists a folder.
    pub list: ListFn,
    /// The state of a path.
    pub stat: StatFn,
    /// The free space.
    pub free_space: FreeSpaceFn,
    /// Called once when the engine stops using the storage. Can be NULL.
    pub release: ReleaseFn,
}

/// The callbacks, all present but `release`.
struct Fns(HostStorage);

/// Calls callback `$f` of `$fns` with the caller's context first. Each callback is present:
/// `sdrkit_engine_new_with_storage` refuses a table without one.
macro_rules! call {
    ($fns:expr, $f:ident ( $($a:expr),* )) => {
        match $fns.0.$f {
            // SAFETY: the caller's callback with its own context and arguments that follow its contract.
            Some(f) => unsafe { f($fns.0.context, $($a),*) },
            None => SDRKIT_E_IO,
        }
    };
}

// SAFETY: the contract of `HostStorage` says that the callbacks and the context work from any thread.
unsafe impl Send for Fns {}
// SAFETY: as for `Send`.
unsafe impl Sync for Fns {}

impl Drop for Fns {
    fn drop(&mut self) {
        if let Some(release) = self.0.release {
            // SAFETY: the host's callback, called once with its own context.
            unsafe { release(self.0.context) }
        }
    }
}

fn status(code: i32) -> Result<(), StorageError> {
    match code {
        SDRKIT_OK => Ok(()),
        SDRKIT_E_NOT_FOUND => Err(StorageError::NotFound),
        SDRKIT_E_ALREADY_EXISTS => Err(StorageError::AlreadyExists),
        SDRKIT_E_NO_SPACE => Err(StorageError::NoSpace),
        SDRKIT_E_DENIED => Err(StorageError::Denied),
        _ => Err(StorageError::Io {
            kind: io::ErrorKind::Other,
        }),
    }
}

fn c_path(p: &StorePath) -> Result<CString, StorageError> {
    CString::new(p.as_str()).map_err(|_| StorageError::Io {
        kind: io::ErrorKind::InvalidInput,
    })
}

/// Storage through the host's callbacks.
struct CStorage(Arc<Fns>);

/// A file that the host opened.
struct CFile {
    fns: Arc<Fns>,
    file: u64,
    pos: u64,
    append: bool,
}

impl CFile {
    fn size(&self) -> Result<u64, StorageError> {
        let mut n = 0;
        status(call!(self.fns, len(self.file, &mut n)))?;
        Ok(n)
    }
}

impl Drop for CFile {
    fn drop(&mut self) {
        if let Some(close) = self.fns.0.close {
            // SAFETY: the host's callback, once for a file it opened.
            unsafe { close(self.fns.0.context, self.file) }
        }
    }
}

impl Read for CFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut got = 0;
        let c = call!(
            self.fns,
            read(self.file, self.pos, buf.as_mut_ptr(), buf.len(), &mut got)
        );
        status(c)?;
        let got = got.min(buf.len());
        self.pos += got as u64;
        Ok(got)
    }
}

impl Write for CFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.append {
            self.pos = self.size()?;
        }
        let c = call!(
            self.fns,
            write(self.file, self.pos, buf.as_ptr(), buf.len())
        );
        status(c)?;
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for CFile {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let base = match to {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            SeekFrom::End(d) => (self.size()?, d),
            SeekFrom::Current(d) => (self.pos, d),
        };
        self.pos = base
            .0
            .checked_add_signed(base.1)
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        Ok(self.pos)
    }
}

impl StoreWriter for CFile {
    fn set_len(&mut self, len: u64) -> Result<(), StorageError> {
        status(call!(self.fns, set_len(self.file, len)))
    }

    fn sync(&mut self) -> Result<(), StorageError> {
        status(call!(self.fns, sync(self.file)))
    }
}

impl StoreReader for CFile {
    fn size(&mut self) -> Result<u64, StorageError> {
        CFile::size(self)
    }
}

/// Copies an entry from the caller. `name` is the name if the entry has none.
unsafe fn entry_of(e: &HostEntry, name: &str) -> StoreEntry {
    let name = if e.name.is_null() {
        name.to_owned()
    } else {
        // SAFETY: the caller's contract.
        unsafe { CStr::from_ptr(e.name) }
            .to_string_lossy()
            .into_owned()
    };
    StoreEntry {
        name,
        size: e.len,
        modified: (!e.modified.is_nan()).then_some(e.modified),
        is_dir: e.is_dir,
    }
}

unsafe extern "C" fn add_entry(sink: *mut c_void, entry: *const HostEntry) {
    // SAFETY: `sink` is the `Vec` that `list` passed, and the host supplies a valid entry or NULL.
    let (list, e) = unsafe { (&mut *sink.cast::<Vec<StoreEntry>>(), sized_in(entry)) };
    if let Ok(e) = e {
        // SAFETY: the host supplies a NUL-terminated name or NULL.
        list.push(unsafe { entry_of(&e, "") });
    }
}

impl CStorage {
    fn open(&self, path: &StorePath, open_as: i32) -> Result<CFile, StorageError> {
        let p = c_path(path)?;
        let mut file = 0;
        status(call!(self.0, open(p.as_ptr(), open_as, &mut file)))?;
        Ok(CFile {
            fns: Arc::clone(&self.0),
            file,
            pos: 0,
            append: open_as == SDRKIT_OPEN_APPEND,
        })
    }
}

impl Storage for CStorage {
    fn read(&self, path: &StorePath) -> Result<Vec<u8>, StorageError> {
        let mut f = self.open(path, SDRKIT_OPEN_READ)?;
        let mut out = Vec::with_capacity(usize::try_from(f.size()?).unwrap_or(0));
        f.read_to_end(&mut out)?;
        Ok(out)
    }

    fn write(&self, path: &StorePath, data: &[u8]) -> Result<(), StorageError> {
        let part = StorePath::new(&format!("{}.part", path.as_str()))?;
        {
            let mut f = self.open(&part, SDRKIT_OPEN_TRUNCATE)?;
            f.write_all(data)?;
            StoreWriter::sync(&mut f)?;
        }
        self.rename(&part, path)
    }

    fn open_writer(
        &self,
        path: &StorePath,
        how: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError> {
        let open_as = match how {
            WriteMode::Truncate => SDRKIT_OPEN_TRUNCATE,
            WriteMode::Append => SDRKIT_OPEN_APPEND,
            WriteMode::Update => SDRKIT_OPEN_UPDATE,
            WriteMode::CreateNew => SDRKIT_OPEN_CREATE_NEW,
        };
        Ok(Box::new(self.open(path, open_as)?))
    }

    fn open_reader(&self, path: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
        Ok(Box::new(self.open(path, SDRKIT_OPEN_READ)?))
    }

    fn remove(&self, path: &StorePath) -> Result<(), StorageError> {
        let p = c_path(path)?;
        status(call!(self.0, remove(p.as_ptr(), false)))
    }

    fn remove_tree(&self, path: &StorePath) -> Result<(), StorageError> {
        let p = c_path(path)?;
        status(call!(self.0, remove(p.as_ptr(), true)))
    }

    fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError> {
        let (f, t) = (c_path(from)?, c_path(to)?);
        status(call!(self.0, rename(f.as_ptr(), t.as_ptr())))
    }

    fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
        let p = c_path(folder)?;
        let mut out: Vec<StoreEntry> = Vec::new();
        let sink: *mut Vec<StoreEntry> = &mut out;
        status(call!(
            self.0,
            list(p.as_ptr(), sink.cast(), Some(add_entry))
        ))?;
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    fn stat(&self, path: &StorePath) -> Result<StoreEntry, StorageError> {
        let p = c_path(path)?;
        let mut e = HostEntry {
            modified: f64::NAN,
            ..HostEntry::default()
        };
        status(call!(self.0, stat(p.as_ptr(), &mut e)))?;
        // SAFETY: the host supplies a NUL-terminated name or NULL.
        Ok(unsafe { entry_of(&e, path.file_name()) })
    }

    fn free_space(&self) -> Result<Option<u64>, StorageError> {
        let (mut bytes, mut known) = (0, false);
        status(call!(self.0, free_space(&mut bytes, &mut known)))?;
        Ok(known.then_some(bytes))
    }
}

/// Makes an engine that keeps its files in the storage of the callbacks in `storage`, and writes its handle to `engine`. The
/// engine copies the table. Returns `SDRKIT_E_NULL_POINTER` if a callback other than `release` is NULL.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_engine_new_with_storage(
    storage: *const HostStorage,
    engine: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (table, out) = unsafe { (sized_in(storage), Out::new(engine)) };
    run_free(|| {
        let out = out?;
        let t = table?;
        let complete = t.open.is_some()
            && t.read.is_some()
            && t.write.is_some()
            && t.len.is_some()
            && t.set_len.is_some()
            && t.sync.is_some()
            && t.close.is_some()
            && t.remove.is_some()
            && t.rename.is_some()
            && t.list.is_some()
            && t.stat.is_some()
            && t.free_space.is_some();
        if !complete {
            return Err(Error::Null);
        }
        let fns = Fns(t);
        let facilities = Facilities {
            storage: Arc::new(CStorage(Arc::new(fns))),
            clock: sdrkit_host_engine::system_clock(),
            random: sdrkit_host_engine::system_random(),
            codecs: Arc::default(),
        };
        out.put(make_engine(facilities)?);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::MemStorage;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// The host side of the test: storage in memory behind the callbacks.
    #[derive(Default)]
    struct Mem {
        store: MemStorage,
        files: Mutex<HashMap<u64, StorePath>>,
        next: AtomicUsize,
        released: AtomicUsize,
        calls: AtomicUsize,
    }

    fn code(r: Result<(), StorageError>) -> i32 {
        match r {
            Ok(()) => 0,
            Err(e) => crate::error::storage(e),
        }
    }

    ///
    /// `c` is the `Mem` of the test.
    unsafe fn mem<'a>(c: *mut c_void) -> &'a Mem {
        // SAFETY: the test passes a `Mem` that outlives the engine.
        let m = unsafe { &*c.cast::<Mem>() };
        m.calls.fetch_add(1, Ordering::Relaxed);
        m
    }

    ///
    /// `p` is a NUL-terminated string.
    unsafe fn path(p: *const c_char) -> StorePath {
        // SAFETY: the engine passes NUL-terminated paths.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("");
        StorePath::new(s).unwrap_or_else(|_| StorePath::root())
    }

    fn file(m: &Mem, f: u64) -> Option<StorePath> {
        crate::handle::lock(&m.files).get(&f).cloned()
    }

    unsafe extern "C" fn open(
        c: *mut c_void,
        p: *const c_char,
        open_as: i32,
        out: *mut u64,
    ) -> i32 {
        // SAFETY: the engine's arguments.
        let (m, p) = unsafe { (mem(c), path(p)) };
        let exists = m.store.stat(&p).is_ok();
        let r = match open_as {
            0 if !exists => Err(StorageError::NotFound),
            1 => m.store.write(&p, &[]),
            4 if exists => Err(StorageError::AlreadyExists),
            2..=4 if !exists => m.store.write(&p, &[]),
            0..=4 => Ok(()),
            _ => Err(StorageError::Denied),
        };
        if r.is_ok() {
            let f = m.next.fetch_add(1, Ordering::Relaxed) as u64;
            crate::handle::lock(&m.files).insert(f, p);
            // SAFETY: the engine's out pointer.
            unsafe { *out = f };
        }
        code(r)
    }

    unsafe extern "C" fn read(
        c: *mut c_void,
        f: u64,
        offset: u64,
        buf: *mut u8,
        cap: usize,
        got: *mut usize,
    ) -> i32 {
        // SAFETY: the engine's arguments.
        let m = unsafe { mem(c) };
        let Some(p) = file(m, f) else { return 45 };
        let data = m.store.read(&p).unwrap_or_default();
        let at = (offset as usize).min(data.len());
        let n = cap.min(data.len() - at);
        // SAFETY: the engine supplies `cap` writable bytes and a valid `got`.
        unsafe {
            std::ptr::copy_nonoverlapping(data[at..].as_ptr(), buf, n);
            *got = n;
        }
        0
    }

    unsafe extern "C" fn write(c: *mut c_void, f: u64, offset: u64, d: *const u8, n: usize) -> i32 {
        // SAFETY: the engine's arguments.
        let m = unsafe { mem(c) };
        let Some(p) = file(m, f) else { return 45 };
        let mut data = m.store.read(&p).unwrap_or_default();
        let at = offset as usize;
        if data.len() < at + n {
            data.resize(at + n, 0);
        }
        // SAFETY: the engine supplies `n` readable bytes.
        data[at..at + n].copy_from_slice(unsafe { std::slice::from_raw_parts(d, n) });
        code(m.store.write(&p, &data))
    }

    unsafe extern "C" fn size(c: *mut c_void, f: u64, out: *mut u64) -> i32 {
        // SAFETY: the engine's arguments.
        let m = unsafe { mem(c) };
        let Some(p) = file(m, f) else { return 45 };
        // SAFETY: the engine's out pointer.
        unsafe { *out = m.store.read(&p).map_or(0, |d| d.len() as u64) };
        0
    }

    unsafe extern "C" fn set_len(c: *mut c_void, f: u64, n: u64) -> i32 {
        // SAFETY: the engine's arguments.
        let m = unsafe { mem(c) };
        let Some(p) = file(m, f) else { return 45 };
        let mut data = m.store.read(&p).unwrap_or_default();
        data.resize(n as usize, 0);
        code(m.store.write(&p, &data))
    }

    unsafe extern "C" fn sync(_: *mut c_void, _: u64) -> i32 {
        0
    }

    unsafe extern "C" fn close(c: *mut c_void, f: u64) {
        // SAFETY: the engine's arguments.
        let m = unsafe { mem(c) };
        crate::handle::lock(&m.files).remove(&f);
    }

    unsafe extern "C" fn remove(c: *mut c_void, p: *const c_char, tree: bool) -> i32 {
        // SAFETY: the engine's arguments.
        let (m, p) = unsafe { (mem(c), path(p)) };
        code(if tree {
            m.store.remove_tree(&p)
        } else {
            m.store.remove(&p)
        })
    }

    unsafe extern "C" fn rename(c: *mut c_void, from: *const c_char, to: *const c_char) -> i32 {
        // SAFETY: the engine's arguments.
        let (m, f, t) = unsafe { (mem(c), path(from), path(to)) };
        code(m.store.rename(&f, &t))
    }

    unsafe extern "C" fn list(
        c: *mut c_void,
        folder: *const c_char,
        sink: *mut c_void,
        add: AddEntryFn,
    ) -> i32 {
        // SAFETY: the engine's arguments.
        let (m, p) = unsafe { (mem(c), path(folder)) };
        match m.store.list(&p) {
            Ok(entries) => {
                for e in entries {
                    let name = CString::new(e.name).unwrap_or_default();
                    let entry = HostEntry {
                        name: name.as_ptr(),
                        len: e.size,
                        modified: e.modified.unwrap_or(f64::NAN),
                        is_dir: e.is_dir,
                        ..HostEntry::default()
                    };
                    if let Some(add) = add {
                        // SAFETY: the engine's callback and sink.
                        unsafe { add(sink, &entry) };
                    }
                }
                0
            }
            Err(e) => code(Err(e)),
        }
    }

    unsafe extern "C" fn stat(c: *mut c_void, p: *const c_char, out: *mut HostEntry) -> i32 {
        // SAFETY: the engine's arguments.
        let (m, p) = unsafe { (mem(c), path(p)) };
        match m.store.stat(&p) {
            Ok(e) => {
                // SAFETY: the engine's out pointer. The name is left NULL.
                unsafe {
                    (*out).len = e.size;
                    (*out).is_dir = e.is_dir;
                }
                0
            }
            Err(e) => code(Err(e)),
        }
    }

    unsafe extern "C" fn free_space(_: *mut c_void, bytes: *mut u64, known: *mut bool) -> i32 {
        // SAFETY: the engine's out pointers.
        unsafe {
            *bytes = 1 << 30;
            *known = true;
        }
        0
    }

    unsafe extern "C" fn release(c: *mut c_void) {
        // SAFETY: the engine's argument.
        unsafe { &*c.cast::<Mem>() }
            .released
            .fetch_add(1, Ordering::Relaxed);
    }

    fn table(m: &Mem) -> HostStorage {
        HostStorage {
            context: std::ptr::from_ref(m).cast_mut().cast(),
            open: Some(open),
            read: Some(read),
            write: Some(write),
            len: Some(size),
            set_len: Some(set_len),
            sync: Some(sync),
            close: Some(close),
            remove: Some(remove),
            rename: Some(rename),
            list: Some(list),
            stat: Some(stat),
            free_space: Some(free_space),
            release: Some(release),
            ..HostStorage::default()
        }
    }

    fn p(s: &str) -> StorePath {
        StorePath::new(s).expect("a path")
    }

    #[test]
    fn files_go_through_the_hosts_callbacks() {
        let m = Mem::default();
        let s = CStorage(Arc::new(Fns(table(&m))));
        s.write(&p("a/b.txt"), b"hello").expect("write");
        assert_eq!(s.read(&p("a/b.txt")).expect("read"), b"hello");
        assert!(
            s.stat(&p("a/b.txt.part")).is_err(),
            "the part file was renamed"
        );
        let mut w = s
            .open_writer(&p("a/b.txt"), WriteMode::Append)
            .expect("open");
        w.write_all(b" world").expect("append");
        w.seek(SeekFrom::Start(0)).expect("seek");
        w.write_all(b"!").expect("append at the end");
        drop(w);
        assert_eq!(s.read(&p("a/b.txt")).expect("read"), b"hello world!");
        let mut r = s.open_reader(&p("a/b.txt")).expect("reader");
        r.seek(SeekFrom::End(-6)).expect("seek");
        let mut tail = String::new();
        r.read_to_string(&mut tail).expect("read");
        assert_eq!((tail.as_str(), r.size().expect("size")), ("world!", 12));
        drop(r);
        let list = s.list(&p("a")).expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "b.txt");
        assert_eq!(s.stat(&p("a/b.txt")).expect("stat").name, "b.txt");
        assert_eq!(s.free_space().expect("free"), Some(1 << 30));
        assert_eq!(s.read(&p("none")), Err(StorageError::NotFound));
        s.remove_tree(&p("a")).expect("remove");
        assert!(
            crate::handle::lock(&m.files).is_empty(),
            "every file was closed"
        );
    }

    #[test]
    fn an_engine_on_the_hosts_storage_releases_it_when_freed() {
        let m = Mem::default();
        let t = table(&m);
        let mut e = 0;
        // SAFETY: a complete table whose context outlives the engine, and a valid out pointer.
        assert_eq!(unsafe { sdrkit_engine_new_with_storage(&t, &mut e) }, 0);
        assert_eq!(crate::sdrkit_free(e), 0);
        assert_eq!(m.released.load(Ordering::Relaxed), 1);
        let partial = HostStorage { stat: None, ..t };
        // SAFETY: as above.
        let code = unsafe { sdrkit_engine_new_with_storage(&partial, &mut e) };
        assert_eq!(code, crate::error::SDRKIT_E_NULL_POINTER);
    }
}

crate::handle::sized_structs!(HostEntry, HostStorage);
