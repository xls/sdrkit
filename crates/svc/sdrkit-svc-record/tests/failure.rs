//! A storage failure in the middle of a recording ends it with a typed problem and an event, and leaves a file that reads back
//! up to the failure.

mod support;

use sdrkit_core::{Choice, Storage, StorageError, StorePath, Value};
use sdrkit_svc_record::{read_recording, AudioFeed, Feed, Problem};
use support::*;

const RATE: f64 = 8000.0;
/// Frames of stereo 16-bit audio in one block of the writer (64 KiB).
const BLOCK_FRAMES: usize = 16_384;

fn audio(feed: Feed) -> AudioFeed {
    match feed {
        Feed::Audio(f) => *f,
        f => panic!("not an audio feed: {f:?}"),
    }
}

/// Pushes `frames` of stereo audio at 0.25 in pieces of 1000 frames.
fn push(f: &mut AudioFeed, frames: usize) {
    let x = vec![0.25f32; 2000];
    for _ in 0..frames / 1000 {
        f.push(&x, 2, RATE, true);
    }
}

/// The size of `path`, polled until it is at least `min`.
fn wait_size(rig: &Rig, path: &StorePath, min: u64) {
    for _ in 0..2000 {
        if rig.storage.stat(path).is_ok_and(|e| e.size >= min) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("timed out");
}

fn failed_with(rig: &Rig, problem: Problem) -> bool {
    rig.events_of("record.failed").iter().any(|e| {
        let id = e.fields.decl().id("problem").unwrap();
        e.fields.value(id) == Some(&Value::Choice(problem.index()))
    })
}

#[test]
fn a_failed_write_ends_the_recording_and_the_file_reads_back() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0))]);
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    push(&mut f, BLOCK_FRAMES + 1000);
    rig.tick_until(|r| !r.events_of("record.started").is_empty());
    let path = StorePath::new(&rig.files("wav")[0]).unwrap();
    wait_size(&rig, &path, 44 + 65_536);
    // The next storage operation is the writer's second block.
    rig.storage.fail_next(StorageError::Io {
        kind: std::io::ErrorKind::Other,
    });
    push(&mut f, BLOCK_FRAMES);
    // Nothing else touches the storage until the writer has met the failure.
    std::thread::sleep(std::time::Duration::from_millis(300));
    rig.tick_until(|r| failed_with(r, Problem::Io));
    push(&mut f, 1000);
    assert!(f.is_ended());
    drop(f);
    rig.settle();
    assert!(failed_with(&rig, Problem::Io), "{:?}", rig.events);
    let r = read_recording(&*rig.storage, &path).unwrap();
    assert_eq!(r.frames, BLOCK_FRAMES as u64);
    let b = rig.read(path.as_str());
    assert_eq!(b.len(), 44 + 65_536);
    assert_eq!(i16::from_le_bytes([b[44], b[45]]), 8192);
}

#[test]
fn a_full_storage_ends_the_recording_with_no_space() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0))]);
    rig.storage.set_limit(Some(150_000));
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    for _ in 0..2000 {
        push(&mut f, 4000);
        rig.svc.tick(0.0, &mut rig.events);
        if failed_with(&rig, Problem::NoSpace) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    drop(f);
    rig.settle();
    assert!(failed_with(&rig, Problem::NoSpace));
    let path = StorePath::new(&rig.files("wav")[0]).unwrap();
    let r = read_recording(&*rig.storage, &path).unwrap();
    // Two whole blocks fit under the limit, the third does not.
    assert_eq!(r.frames, 2 * BLOCK_FRAMES as u64);
    assert_eq!(rig.read(path.as_str()).len(), 44 + 2 * 65_536);
}

#[test]
fn free_space_below_the_reserve_cuts_the_recording() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(100_000))]);
    rig.storage.set_limit(Some(200_000));
    let (_, feed) = rig.start("audio.start", &[("session", text("s1"))]);
    let mut f = audio(feed);
    for _ in 0..2000 {
        push(&mut f, 4000);
        rig.svc.tick(0.0, &mut rig.events);
        if !rig.events_of("record.cut").is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    push(&mut f, 1000);
    assert!(f.is_ended());
    drop(f);
    rig.settle();
    assert_eq!(rig.events_of("record.cut").len(), 1);
    assert_eq!(rig.events_of("record.ended").len(), 1);
    assert!(rig.storage.free_space().unwrap().unwrap() < 100_000);
}
