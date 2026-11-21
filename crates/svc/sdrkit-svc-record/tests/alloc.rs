//! The three write paths (audio frames, IQ blocks, transport stream packets) allocate nothing per block once warm, on the feeding
//! thread and on the writer thread.

mod support;

use sdrkit_core::storage::{StoreReader, StoreWriter, WriteMode};
use sdrkit_core::{Complex32, Storage, StorageError, StoreEntry, StorePath, Value};
use sdrkit_svc_record::{Feed, RecordService};
use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use support::{call, clock, multiplex, packets, text};

struct Counting;

static COUNT: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call goes to the system allocator unchanged. The counter only reads a flag and adds one.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the layout comes from the caller, as for the system allocator.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was given by `alloc` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNT.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: `p` was given by `alloc` with layout `l`.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// A storage that keeps nothing and counts the bytes written, so the test measures the recorder and not a storage.
#[derive(Default)]
struct Sink {
    bytes: Arc<AtomicU64>,
}

struct SinkWriter {
    bytes: Arc<AtomicU64>,
    pos: u64,
}

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.bytes.fetch_add(buf.len() as u64, Ordering::Relaxed);
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Seek for SinkWriter {
    fn seek(&mut self, to: SeekFrom) -> std::io::Result<u64> {
        if let SeekFrom::Start(p) = to {
            self.pos = p;
        }
        Ok(self.pos)
    }
}

impl StoreWriter for SinkWriter {
    fn set_len(&mut self, _: u64) -> Result<(), StorageError> {
        Ok(())
    }
    fn sync(&mut self) -> Result<(), StorageError> {
        Ok(())
    }
}

impl Storage for Sink {
    fn read(&self, _: &StorePath) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::NotFound)
    }
    fn write(&self, _: &StorePath, _: &[u8]) -> Result<(), StorageError> {
        Ok(())
    }
    fn open_writer(
        &self,
        _: &StorePath,
        _: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError> {
        Ok(Box::new(SinkWriter {
            bytes: self.bytes.clone(),
            pos: 0,
        }))
    }
    fn open_reader(&self, _: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
        Err(StorageError::NotFound)
    }
    fn remove(&self, _: &StorePath) -> Result<(), StorageError> {
        Ok(())
    }
    fn remove_tree(&self, _: &StorePath) -> Result<(), StorageError> {
        Ok(())
    }
    fn rename(&self, _: &StorePath, _: &StorePath) -> Result<(), StorageError> {
        Ok(())
    }
    fn list(&self, _: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
        Err(StorageError::NotFound)
    }
    fn stat(&self, _: &StorePath) -> Result<StoreEntry, StorageError> {
        Err(StorageError::NotFound)
    }
    fn free_space(&self) -> Result<Option<u64>, StorageError> {
        Ok(None)
    }
}

/// Waits until the writer has taken `bytes` in all.
fn wait_bytes(sink: &AtomicU64, bytes: u64) {
    for _ in 0..2000 {
        if sink.load(Ordering::Relaxed) >= bytes {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("timed out at {} of {bytes}", sink.load(Ordering::Relaxed));
}

/// Runs `push` once to warm up, then `rounds` times while counting allocations, and returns the count.
fn count(rounds: usize, mut push: impl FnMut()) -> usize {
    push();
    std::thread::sleep(std::time::Duration::from_millis(20));
    ALLOCS.store(0, Ordering::Relaxed);
    COUNT.store(true, Ordering::Relaxed);
    for _ in 0..rounds {
        push();
    }
    COUNT.store(false, Ordering::Relaxed);
    ALLOCS.load(Ordering::Relaxed)
}

#[test]
fn the_write_paths_allocate_nothing_once_warm() {
    let storage = Arc::new(Sink::default());
    let written = storage.bytes.clone();
    let time = Arc::new(AtomicU64::new(0));
    let mut svc = RecordService::open(storage.clone(), clock(&time)).unwrap();
    for (k, v) in [
        ("reserve_bytes", Value::I64(0)),
        ("queue_bytes", Value::I64(64 << 20)),
    ] {
        let id = sdrkit_core::owner::Owner::schema(&svc).id(k).unwrap();
        sdrkit_core::owner::apply(&mut svc, id, &v).unwrap();
    }

    // Audio: stereo frames, converted to 16-bit as they go into the writer's blocks.
    call(&mut svc, "audio.start", &[("session", text("s1"))]).unwrap();
    let Some(Feed::Audio(mut a)) = svc.take_feeds().pop() else {
        panic!("no audio feed")
    };
    let frames = vec![0.25f32; 2 * 4800];
    let n = count(200, || a.push(&frames, 2, 48_000.0, true));
    wait_bytes(&written, 201 * 4800 * 4 - 65_536);
    assert_eq!(n, 0, "audio");
    drop(a);

    // IQ: 16-bit pairs.
    written.store(0, Ordering::Relaxed);
    call(&mut svc, "iq.start", &[("session", text("s1"))]).unwrap();
    let Some(Feed::Iq(mut q)) = svc.take_feeds().pop() else {
        panic!("no IQ feed")
    };
    q.set_format(2.4e6, 100e6);
    let iq = vec![Complex32::new(0.25, -0.25); 65_536];
    let n = count(100, || q.push(&iq));
    wait_bytes(&written, 101 * 65_536 * 4 - 262_144);
    assert_eq!(n, 0, "iq");
    drop(q);

    // The transport stream: each packet by its shared bytes.
    written.store(0, Ordering::Relaxed);
    call(&mut svc, "ts.start", &[("session", text("s1"))]).unwrap();
    let Some(Feed::Ts(mut t)) = svc.take_feeds().pop() else {
        panic!("no stream feed")
    };
    let all = packets(&multiplex(2.0, 20, 0));
    let n = count(20, || all.iter().for_each(|p| t.push(p)));
    assert_eq!(n, 0, "ts");
    drop(t);
}
