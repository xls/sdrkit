//! `sdrkit_session_write_audio` allocates nothing on its thread when it succeeds, so a real-time audio thread can call it.
//! A counting allocator counts the allocations of the calling thread only; the threads of the engine allocate as they
//! like meanwhile. The audio is ten times what the session keeps, so the lost count shows that it arrived.

use sdrkit::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::ffi::CString;
use std::sync::atomic::{AtomicU64, Ordering};

struct Counting;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// The allocations of this thread are counted.
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: every call goes to the system allocator; the count is a side effect.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller's contract.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count();
        // SAFETY: the caller's contract.
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        // SAFETY: the caller's contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn count() {
    if WATCHED.try_with(Cell::get).unwrap_or(false) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn c(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

/// The whole number in the result field `key` of `session`, from a new snapshot.
fn integer(session: Handle, key: &str) -> i64 {
    let mut id = 0;
    // SAFETY: a NUL-terminated key and valid out pointers; a zeroed variant is a valid out value.
    unsafe {
        assert_eq!(sdrkit_field_find(session, c(key).as_ptr(), &mut id), SDRKIT_OK);
        let mut info = SnapshotInfo::default();
        assert_eq!(sdrkit_snapshot_read(session, 0, &mut info), SDRKIT_OK);
        let mut v: Variant = std::mem::zeroed();
        assert_eq!(sdrkit_snapshot_field(session, id, &mut v), SDRKIT_OK);
        v.integer
    }
}

#[test]
fn writing_host_audio_allocates_nothing() {
    let (mut e, mut radio, mut session) = (0, 0, 0);
    let audio: Vec<f32> = (0..480).map(|i| (i as f32 * 0.01).sin()).collect();
    // SAFETY: NUL-terminated strings, no options, valid out pointers, `audio` holds 480 floats.
    unsafe {
        assert_eq!(sdrkit_engine_new(std::ptr::null(), &mut e), SDRKIT_OK);
        assert_eq!(
            sdrkit_radio_open(e, c("synth:general").as_ptr(), std::ptr::null(), &mut radio),
            SDRKIT_OK
        );
        let input = sdrkit_input_radio(radio);
        assert_eq!(
            sdrkit_stack_start(e, c("general").as_ptr(), std::ptr::null(), &input, &mut session),
            SDRKIT_OK
        );
        // The graph has run, so its host audio input takes audio.
        let t = std::time::Instant::now();
        while integer(session, "session.samples") == 0 {
            assert!(t.elapsed().as_secs() < 20, "the session does not run");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // Ten seconds of audio in pieces of 10 ms, at two rates, as mono and as stereo.
        WATCHED.with(|w| w.set(true));
        for n in 0..1000 {
            let rate = if n % 2 == 0 { 48_000.0 } else { 44_100.0 };
            let channels = if n % 4 < 2 { 1 } else { 2 };
            let frames = 480 / u32::from(channels);
            assert_eq!(
                sdrkit_session_write_audio(session, audio.as_ptr(), frames, channels, rate),
                SDRKIT_OK
            );
        }
        WATCHED.with(|w| w.set(false));
        // The audio reached the session: more than it keeps was written, and the rest is counted as lost.
        let t = std::time::Instant::now();
        while integer(session, "session.audio_in_lost") == 0 {
            assert!(t.elapsed().as_secs() < 20, "no host audio reached the session");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(sdrkit_session_stop(session), SDRKIT_OK);
        assert_eq!(sdrkit_free(e), SDRKIT_OK);
    }
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
