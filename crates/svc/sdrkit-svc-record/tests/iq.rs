//! IQ recording and the rewind buffer through the service: SigMF and raw files that read back with their format and samples.

mod support;

use sdrkit_core::{Choice, Complex32, Storage, StorePath, Value};
use sdrkit_graph::stream::{Kind, SampleType};
use sdrkit_svc_record::{read_recording, Container, Feed, IqContainer, IqSample};
use support::*;

/// Sample `k` of a ramp that 8-bit samples hold exactly: k mod 256 as a signed byte, over 128.
fn sample(k: u64) -> Complex32 {
    let v = f32::from((k % 256) as u8 as i8) / 128.0;
    Complex32::new(v, -v)
}

fn ramp(from: u64, n: u64) -> Vec<Complex32> {
    (from..from + n).map(sample).collect()
}

#[test]
fn the_running_row_shows_the_size_of_the_open_file() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0))]);
    let (row, feed) = rig.start("iq.start", &[("session", text("s1"))]);
    let Feed::Iq(mut f) = feed else {
        panic!("not IQ")
    };
    f.set_stack("hf");
    f.set_format(48_000.0, 7.1e6);
    let size = |r: &Rig| match rows(&r.svc, "running").first() {
        Some((_, rec)) => cell(rec, "bytes"),
        None => Value::I64(-1),
    };
    for k in 0..30 {
        f.push(&ramp(k * 4800, 4800));
    }
    // Two blocks of 256 KiB are full and written; the file is open.
    rig.tick_until(|r| matches!(size(r), Value::I64(n) if n >= 2 * 262_144));
    assert!(matches!(size(&rig), Value::I64(n) if n < 576_000));
    rig.call("iq.stop", &[("row", text(&row))]).unwrap();
    drop(f);
    rig.settle();
    let data = rig.files("sigmf-data");
    assert_eq!(rig.read(&data[0]).len(), 576_000);
}

#[test]
fn an_iq_recording_with_sigmf_metadata_reads_back() {
    let mut rig = Rig::new(&[("reserve_bytes", Value::I64(0))]);
    let (row, feed) = rig.start("iq.start", &[("session", text("s1"))]);
    let Feed::Iq(mut f) = feed else {
        panic!("not IQ")
    };
    f.set_stack("hf");
    f.set_format(48_000.0, 7.1e6);
    for k in 0..10 {
        f.push(&ramp(k * 4800, 4800));
    }
    rig.call("iq.stop", &[("row", text(&row))]).unwrap();
    f.push(&ramp(0, 10));
    drop(f);
    rig.settle();
    let data = rig.files("sigmf-data");
    assert_eq!(data.len(), 1);
    assert_eq!(rig.files("sigmf-meta").len(), 1);
    assert!(
        data[0].starts_with("recordings/sdrkit_hf_7.100MHz_0.048Msps_20250101-120000"),
        "{data:?}"
    );
    let r = read_recording(&*rig.storage, &StorePath::new(&data[0]).unwrap()).unwrap();
    assert_eq!(r.container, Container::Sigmf);
    assert_eq!(r.format.kind, Kind::Iq);
    assert_eq!((r.format.rate, r.format.centre_hz), (48_000.0, 7.1e6));
    assert_eq!(r.format.sample_type, Some(SampleType::Cs16));
    assert_eq!(r.frames, 48_000);
    // The samples: 16-bit pairs of round(x * 32768).
    let b = rig.read(&data[0]);
    for k in [0usize, 1, 200, 47_999] {
        let re = i16::from_le_bytes([b[4 * k], b[4 * k + 1]]);
        assert_eq!(f32::from(re) / 32768.0, sample(k as u64).re, "{k}");
    }
    let rec = rows(&rig.svc, "recordings");
    assert_eq!(rec.len(), 1);
    assert_eq!(cell(&rec[0].1, "seconds"), Value::Duration(1.0));
}

#[test]
fn a_raw_recording_names_its_rate_and_centre() {
    let mut rig = Rig::new(&[
        ("reserve_bytes", Value::I64(0)),
        ("iq_container", Value::Choice(IqContainer::Raw.index())),
        ("iq_sample", Value::Choice(IqSample::Cs8.index())),
    ]);
    let (_, feed) = rig.start(
        "iq.start",
        &[("session", text("s1")), ("seconds", Value::Duration(0.5))],
    );
    let Feed::Iq(mut f) = feed else {
        panic!("not IQ")
    };
    f.set_format(2.4e6, 100e6);
    for k in 0..20 {
        f.push(&ramp(k * 100_000, 100_000));
    }
    assert!(f.is_ended());
    drop(f);
    rig.settle();
    let files = rig.files("cs8");
    assert_eq!(files.len(), 1);
    let r = read_recording(&*rig.storage, &StorePath::new(&files[0]).unwrap()).unwrap();
    assert_eq!(
        (r.container, r.format.rate, r.format.centre_hz),
        (Container::Raw, 2.4e6, 100e6)
    );
    // The length limit ends it at the first block past 0.5 s.
    assert_eq!(r.frames, 1_200_000);
    let b = rig.read(&files[0]);
    assert_eq!(b[2 * 300] as i8, (300 % 256) as u8 as i8);
}

#[test]
fn a_rewind_save_while_the_buffer_goes_on_holds_the_wanted_span() {
    let mut rig = Rig::new(&[
        ("reserve_bytes", Value::I64(0)),
        ("rewind_bytes", Value::I64(4 << 20)),
    ]);
    let id = sdrkit_core::owner::Owner::schema(&rig.svc)
        .id("rewind")
        .unwrap();
    sdrkit_core::owner::apply(&mut rig.svc, id, &Value::Bool(true)).unwrap();
    let Some(Feed::Rewind(mut f)) = rig.svc.take_feeds().pop() else {
        panic!("no rewind feed")
    };
    f.configure(48_000.0, 7.1e6, "hf");
    // Three seconds, then a save of the last second and the half second that follows.
    for k in 0..30 {
        f.push(&ramp(k * 4800, 4800));
    }
    rig.svc.tick(0.0, &mut rig.events);
    let reply = rig
        .call(
            "rewind.save",
            &[
                ("pre_s", Value::Duration(1.0)),
                ("post_s", Value::Duration(0.5)),
            ],
        )
        .unwrap()
        .unwrap();
    assert!(matches!(cell(&reply, "recording"), Value::Text(_)));
    // The buffer goes on while the save writes.
    for k in 30..60 {
        f.push(&ramp(k * 4800, 4800));
    }
    rig.settle();
    let data = rig.files("sigmf-data");
    assert_eq!(data.len(), 1, "{data:?}");
    let r = read_recording(&*rig.storage, &StorePath::new(&data[0]).unwrap()).unwrap();
    assert_eq!(r.format.sample_type, Some(SampleType::Cs8));
    assert_eq!((r.format.rate, r.format.centre_hz), (48_000.0, 7.1e6));
    assert_eq!(r.frames, 72_000);
    // The first sample is the one a second before the save.
    let b = rig.read(&data[0]);
    let first = 96_000u64;
    for k in [0u64, 1, 71_999] {
        assert_eq!(
            b[2 * k as usize] as i8,
            ((first + k) % 256) as u8 as i8,
            "{k}"
        );
    }
    assert_eq!(rows(&rig.svc, "recordings").len(), 1);
    // The space rules count each file once: the data and its metadata.
    let folder = StorePath::new("recordings").unwrap();
    let on_disk: u64 = rig
        .storage
        .list(&folder)
        .unwrap()
        .iter()
        .map(|e| e.size)
        .sum();
    rig.svc.tick(0.0, &mut rig.events);
    assert_eq!(rig.field("used_bytes"), Value::I64(on_disk as i64));
    assert_eq!(
        on_disk,
        144_000 + rig.read(&data[0].replace("-data", "-meta")).len() as u64
    );
}

#[test]
fn a_retune_empties_the_history_and_ends_a_save() {
    let mut rig = Rig::new(&[
        ("reserve_bytes", Value::I64(0)),
        ("rewind_bytes", Value::I64(4 << 20)),
    ]);
    let id = sdrkit_core::owner::Owner::schema(&rig.svc)
        .id("rewind")
        .unwrap();
    sdrkit_core::owner::apply(&mut rig.svc, id, &Value::Bool(true)).unwrap();
    let Some(Feed::Rewind(mut f)) = rig.svc.take_feeds().pop() else {
        panic!("no rewind feed")
    };
    f.configure(48_000.0, 7.1e6, "hf");
    for k in 0..10 {
        f.push(&ramp(k * 4800, 4800));
    }
    rig.svc.tick(0.0, &mut rig.events);
    rig.call(
        "rewind.save",
        &[
            ("pre_s", Value::Duration(0.5)),
            ("post_s", Value::Duration(10.0)),
        ],
    )
    .unwrap();
    for k in 10..20 {
        f.push(&ramp(k * 4800, 4800));
    }
    f.retuned(7.2e6);
    rig.settle();
    rig.svc.tick(0.0, &mut rig.events);
    assert_eq!(rig.field("rewind_s"), Value::Duration(0.0));
    // The save keeps what it had: half a second before and the second after, not the ten requested.
    let data = rig.files("sigmf-data");
    let r = read_recording(&*rig.storage, &StorePath::new(&data[0]).unwrap()).unwrap();
    assert_eq!(r.frames, 72_000);
}
