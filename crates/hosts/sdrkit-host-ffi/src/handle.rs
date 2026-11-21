//! Handles: the numbers that the caller holds for an engine, an object of an engine or a graph, the buffers that each
//! handle keeps for the views that its calls return, and the guard that every C function runs in.

use crate::error::{Error, LastError, SDRKIT_E_NO_SUCH_OBJECT, SDRKIT_OK};
use sdrkit_core::{Owner, Value};
use sdrkit_host_engine::{Engine, EngineError, OwnerRef};
use std::ffi::{c_char, CStr};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, MutexGuard};

/// An engine, an object of an engine, or a graph. 0 is never a handle.
pub type Handle = u64;

/// What one engine of the C API holds.
pub(crate) struct Host {
    pub engine: Engine,
    pub last: Mutex<LastError>,
}

/// What a handle stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum What {
    /// The engine. Freeing it frees the engine and every handle of it.
    Engine,
    /// Another object.
    Object(OwnerRef),
    /// A graph that the caller builds.
    Graph,
    /// A host feed. Freeing it closes the feed.
    Feed(sdrkit_host_engine::FeedId),
}

/// One handle: its engine, what it stands for, and the buffers behind the views that its calls return.
pub(crate) struct Entry {
    pub host: Arc<Host>,
    pub what: What,
    pub state: Mutex<State>,
    /// For a session: its host audio input, taken when the handle is made, so that a write takes no lock of the engine.
    pub audio_in: Option<sdrkit_host_engine::AudioInput>,
}

/// The buffers of one handle. A view that a call returns points into them and stays valid until the next call on the handle.
#[derive(Default)]
pub(crate) struct State {
    pub string: String,
    pub bytes: Vec<u8>,
    pub error_names: [String; 4],
    pub value: Value,
    pub objects: Vec<(String, i32)>,
    pub decl: crate::decl::Held,
    pub results: crate::results::View,
    pub events: crate::events::Taken,
    pub command: crate::commands::Called,
    pub typed: crate::typed::Resolved,
    pub graph: crate::graph::Plan,
    pub streams: crate::streams::Buffers,
}

impl Entry {
    /// The object of the handle.
    pub fn owner_ref(&self) -> Result<OwnerRef, Error> {
        match self.what {
            What::Engine => Ok(OwnerRef::Engine),
            What::Object(r) => Ok(r),
            What::Graph | What::Feed(_) => Err(Error::WrongHandle),
        }
    }

    /// Runs `f` on the object of the handle.
    pub fn owner<R>(&self, f: impl FnOnce(&mut dyn Owner) -> R) -> Result<R, Error> {
        let r = self.owner_ref()?;
        Ok(self.host.engine.owner(r, f)?)
    }

    /// The buffers of the handle.
    pub fn state(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }
}

/// The lock of `m`, also after a thread panicked while it held it.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Table {
    /// Each slot: its generation and its entry.
    slots: Vec<(u32, Option<Arc<Entry>>)>,
    free: Vec<u32>,
}

static TABLE: Mutex<Table> = Mutex::new(Table {
    slots: Vec::new(),
    free: Vec::new(),
});

fn split(h: Handle) -> (usize, u32) {
    ((h & 0xFFFF_FFFF) as usize, (h >> 32) as u32)
}

/// Returns a new handle of `entry`.
pub(crate) fn insert(entry: Entry) -> Handle {
    let mut t = lock(&TABLE);
    let entry = Some(Arc::new(entry));
    let index = match t.free.pop() {
        Some(i) => {
            let slot = &mut t.slots[i as usize];
            slot.0 = slot.0.wrapping_add(1).max(1);
            slot.1 = entry;
            i
        }
        None => {
            t.slots.push((1, entry));
            u32::try_from(t.slots.len() - 1).unwrap_or(u32::MAX)
        }
    };
    let generation = t.slots[index as usize].0;
    (u64::from(generation) << 32) | u64::from(index)
}

/// The entry of handle `h`, if it is live.
pub(crate) fn get(h: Handle) -> Option<Arc<Entry>> {
    let (i, generation) = split(h);
    let t = lock(&TABLE);
    match t.slots.get(i) {
        Some((g, Some(e))) if *g == generation => Some(Arc::clone(e)),
        _ => None,
    }
}

/// Frees handle `h`. For the engine handle, frees every handle of that engine too.
pub(crate) fn remove(h: Handle) -> bool {
    let (i, generation) = split(h);
    let mut gone = Vec::new();
    {
        let mut t = lock(&TABLE);
        let host = match t.slots.get(i) {
            Some((g, Some(e))) if *g == generation => {
                (e.what == What::Engine).then(|| Arc::clone(&e.host))
            }
            _ => return false,
        };
        let Table { slots, free } = &mut *t;
        for (k, slot) in slots.iter_mut().enumerate() {
            let hit = match (&slot.1, &host) {
                (Some(e), Some(host)) => Arc::ptr_eq(&e.host, host),
                (Some(_), None) => k == i,
                (None, _) => false,
            };
            if hit {
                gone.push(slot.1.take());
                free.push(u32::try_from(k).unwrap_or(u32::MAX));
            }
        }
    }
    // A feed closes, and the engine stops when the last entry of it goes, outside the lock of the table.
    for e in gone.iter().flatten() {
        if let What::Feed(id) = e.what {
            let _ = e.host.engine.close_feed(id);
        }
    }
    drop(gone);
    true
}

/// Runs the body of a C function on handle `h`. Returns `SDRKIT_E_NO_SUCH_OBJECT` for a dead handle, keeps an error as the
/// last error of the engine, and returns `SDRKIT_E_PANIC` for a panic.
pub(crate) fn run(h: Handle, f: impl FnOnce(&Entry) -> Result<(), Error>) -> i32 {
    let Some(e) = get(h) else {
        return SDRKIT_E_NO_SUCH_OBJECT;
    };
    let r = catch_unwind(AssertUnwindSafe(|| f(&e))).unwrap_or(Err(Error::Panic));
    match r {
        Ok(()) => SDRKIT_OK,
        Err(err) => e.host.fail(&err),
    }
}

/// Runs the body of a C function that takes no handle. Keeps an error as the last error of handle 0, and returns
/// `SDRKIT_E_PANIC` for a panic.
pub(crate) fn run_free(f: impl FnOnce() -> Result<(), Error>) -> i32 {
    match catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(Error::Panic)) {
        Ok(()) => SDRKIT_OK,
        Err(err) => crate::error::fail_process(&err),
    }
}

impl From<EngineError> for Error {
    fn from(e: EngineError) -> Self {
        Error::Engine(e)
    }
}

/// A type that a call writes through an out parameter.
///
/// # Safety
///
/// A type with `SIZED` is `repr(C)`, its first member is `size: u32`, and all bytes 0 are a value of it.
pub(crate) unsafe trait Slot: Copy {
    /// The type is a struct of the C header that begins with its size.
    const SIZED: bool = false;
}

macro_rules! plain_slots {
    ($($t:ty),* $(,)?) => { $(
        // SAFETY: not `SIZED`.
        unsafe impl Slot for $t {}
    )* };
}
plain_slots!(bool, i32, u32, i64, u64, usize, f64);
plain_slots!(crate::Str, crate::Bytes, crate::Variant, crate::Position);

/// Declares structs of the C header that begin with `size: u32`. `Default` is the struct with its size set and every
/// other member 0, as the `*_INIT` initialiser of the header.
macro_rules! sized_structs {
    ($($t:ty),* $(,)?) => { $(
        // SAFETY: the struct is `repr(C)`, begins with `size: u32`, and holds numbers, switches, views, pointers and
        // optional functions only.
        unsafe impl $crate::handle::Slot for $t {
            const SIZED: bool = true;
        }
        impl Default for $t {
            fn default() -> Self {
                $crate::handle::sized_zero()
            }
        }
    )* };
}
pub(crate) use sized_structs;

/// The smallest `size` of a sized struct: the `size` member itself.
const SIZE_MEMBER: usize = std::mem::size_of::<u32>();

/// The sized struct with its size set and every other member 0.
pub(crate) fn sized_zero<T: Slot>() -> T {
    debug_assert!(T::SIZED);
    // SAFETY: all bytes 0 are a value of a sized struct, and it begins with `size: u32`.
    unsafe {
        let mut v = std::mem::zeroed::<T>();
        set_size(&mut v, std::mem::size_of::<T>());
        v
    }
}

/// Writes `size` to the first member of the sized struct `v`.
fn set_size<T: Slot>(v: &mut T, size: usize) {
    // SAFETY: a sized struct begins with `size: u32`.
    unsafe {
        std::ptr::from_mut(v)
            .cast::<u32>()
            .write(u32::try_from(size).unwrap_or(u32::MAX))
    }
}

/// The `size` member of the sized struct at `p`.
///
/// # Errors
///
/// [`Error::BadArgument`] if the size does not hold the `size` member.
///
/// # Safety
///
/// `p` is valid for a read of 4 bytes.
pub(crate) unsafe fn size_at(p: *const u8) -> Result<usize, Error> {
    // SAFETY: the caller's contract. The read needs no alignment.
    let size = unsafe { p.cast::<u32>().read_unaligned() } as usize;
    if size < SIZE_MEMBER {
        return Err(Error::BadArgument);
    }
    Ok(size)
}

/// The sized struct that the caller passes at `p`. The call reads the bytes that both the caller and the library know:
/// the smaller of the `size` member and the size of `T`. The other members are 0, and `size` is the size of `T`.
///
/// # Errors
///
/// [`Error::Null`] for NULL, and [`Error::BadArgument`] if `size` does not hold the `size` member.
///
/// # Safety
///
/// `p` is NULL or valid for a read of the number of bytes in its `size` member.
pub(crate) unsafe fn sized_in<T: Slot>(p: *const T) -> Result<T, Error> {
    if p.is_null() {
        return Err(Error::Null);
    }
    let mut v = sized_zero::<T>();
    // SAFETY: the caller's contract. The copy stays within both structs.
    unsafe {
        let n = size_at(p.cast())?.min(std::mem::size_of::<T>());
        std::ptr::copy_nonoverlapping(p.cast::<u8>(), std::ptr::from_mut(&mut v).cast::<u8>(), n);
    }
    set_size(&mut v, std::mem::size_of::<T>());
    Ok(v)
}

/// The sized struct at `p`, as [`sized_in`] reads it, or `None` for NULL.
///
/// # Errors
///
/// [`Error::BadArgument`] if `size` does not hold the `size` member.
///
/// # Safety
///
/// As [`sized_in`].
pub(crate) unsafe fn optional_in<T: Slot>(p: *const T) -> Result<Option<T>, Error> {
    if p.is_null() {
        return Ok(None);
    }
    // SAFETY: the caller's contract.
    unsafe { sized_in(p).map(Some) }
}

/// An out parameter. For a sized struct, the caller sets `size` before the call, and [`Out::put`] writes the bytes
/// that both the caller and the library know and sets `size` to their number.
pub(crate) struct Out<T> {
    at: NonNull<T>,
    /// The number of bytes to write.
    room: usize,
}

impl<T: Slot> Out<T> {
    /// The out parameter at `p`.
    ///
    /// `p` is NULL, or valid for a write of `T` until [`Out::put`]; for a sized struct, of the bytes in its `size`
    /// member.
    ///
    /// # Errors
    ///
    /// [`Error::Null`] for NULL, and [`Error::BadArgument`] if the `size` of a sized struct does not hold the `size`
    /// member.
    pub unsafe fn new(p: *mut T) -> Result<Self, Error> {
        let at = NonNull::new(p).ok_or(Error::Null)?;
        let own = std::mem::size_of::<T>();
        let room = if T::SIZED {
            // SAFETY: the caller's contract.
            unsafe { size_at(p.cast())?.min(own) }
        } else {
            own
        };
        Ok(Out { at, room })
    }

    /// Writes `v`.
    pub fn put(self, mut v: T) {
        if T::SIZED {
            set_size(&mut v, self.room);
        }
        // SAFETY: `new` was given a pointer valid for a write of `room` bytes. The copy does not read or drop the old
        // bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(
                std::ptr::from_ref(&v).cast::<u8>(),
                self.at.as_ptr().cast::<u8>(),
                self.room,
            );
        }
    }
}

/// An out parameter that the caller can leave NULL.
///
/// # Errors
///
/// [`Error::BadArgument`] for a sized struct with a wrong `size`.
pub(crate) fn optional<T>(out: Result<Out<T>, Error>) -> Result<Option<Out<T>>, Error> {
    match out {
        Ok(o) => Ok(Some(o)),
        Err(Error::Null) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The UTF-8 string at `p`, which ends with a NUL byte.
pub(crate) unsafe fn str_in<'a>(p: *const c_char) -> Result<&'a str, Error> {
    if p.is_null() {
        return Err(Error::Null);
    }
    // SAFETY: the caller supplies a NUL-terminated string that is valid for 'a.
    let c = unsafe { CStr::from_ptr(p) };
    c.to_str().map_err(|_| Error::BadString)
}

/// The `n` items at `p`. NULL is allowed when `n` is 0.
pub(crate) unsafe fn slice_in<'a, T>(p: *const T, n: usize) -> Result<&'a [T], Error> {
    if n == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err(Error::Null);
    }
    // SAFETY: the caller supplies `n` readable, initialised items at a non-NULL `p`. C arrays of `T` are aligned for `T`.
    Ok(unsafe { std::slice::from_raw_parts(p, n) })
}

/// The `n` items at `p` to write. NULL is allowed when `n` is 0.
pub(crate) unsafe fn slice_out<'a, T: Copy>(p: *mut T, n: usize) -> Result<&'a mut [T], Error> {
    if n == 0 {
        return Ok(&mut []);
    }
    if p.is_null() {
        return Err(Error::Null);
    }
    // SAFETY: the caller supplies `n` items of a plain `Copy` type at a non-NULL `p`, which only this call touches.
    Ok(unsafe { std::slice::from_raw_parts_mut(p, n) })
}
