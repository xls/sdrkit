//! A recording of a real HackRF One (firmware 2026.01.3, USB API 1.10) receiving at 100 MHz and 8 MS/s, replayed: the
//! driver sends the same requests in the same order, and the recorded blocks reach the ring.

mod common;

use common::*;
use sdrkit_core::Value;
use sdrkit_driver_usb::{BulkKind, Entry, FakeUsb, Payload, Script, Step};
use std::time::Duration;

/// The blocks of the recording that the replay gives.
const BLOCKS: usize = 24;

#[test]
fn a_recorded_session_of_a_hackrf_one_replays() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/usb/hackrf/probe-100mhz-8msps.txt"
    );
    let mut s = Script::load(std::path::Path::new(path)).unwrap();
    // The streaming state is read on a timer; the replay runs without it and at once.
    s.entries.retain(
        |e| !matches!(e, Entry::Step(Step::ControlIn { setup, .. }) if setup.request == 0x29),
    );
    let mut kept = 0;
    let mut heads = Vec::new();
    s.entries.retain_mut(|e| match e {
        Entry::Bulk(b) => {
            kept += 1;
            b.after = Duration::ZERO;
            if let BulkKind::Data(p) = &b.kind {
                let mut whole = vec![0u8; p.len()];
                p.copy_to(0, &mut whole);
                heads.push(whole);
            }
            kept <= BLOCKS
        }
        _ => true,
    });
    heads.truncate(BLOCKS);
    assert_eq!(heads.len(), BLOCKS);
    assert!(matches!(
        s.entries.iter().find_map(|e| match e {
            Entry::Bulk(b) => Some(&b.kind),
            _ => None,
        }),
        Some(BulkKind::Data(Payload::Cut { len: 262_144, .. }))
    ));
    let bus = FakeUsb::new(s);
    let v = values(&[("centre_hz", Value::Hz(100e6)), ("rate", Value::Hz(8e6))]);
    let mut src = open(&bus, &v);
    let (w, mut r) = sdrkit_driver_core::ring(1 << 23, sdrkit_graph::stream::SampleType::Cs8);
    src.start(w).unwrap();
    let want: Vec<u8> = heads.concat();
    assert_eq!(read_bytes(&mut r, want.len()), want);
    src.stop();
    bus.check().unwrap();
}
