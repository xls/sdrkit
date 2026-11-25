//! The recording of the real dongle (`examples/rtl_probe.rs` on a NESDR SMArt v5-C) replayed through the scripted
//! bus: the same calls give the same control transfers in the same order, and every bulk byte arrives.

use sdrkit_core::{SettingSet, Value, Values};
use sdrkit_driver_core::{ring, RadioSetting, SourceDriver};
use sdrkit_driver_rtlusb::RtlUsbDriver;
use sdrkit_driver_usb::{BulkItem, BulkKind, Entry, FakeUsb, Payload, Script};
use sdrkit_graph::stream::SampleType;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CENTRE: f64 = 104.9e6;

fn values(d: &RtlUsbDriver, centre: f64, rate: f64) -> Values {
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Centre.id(), &Value::Hz(centre))
        .unwrap();
    v.set(RadioSetting::Rate.id(), &Value::Hz(rate)).unwrap();
    v.set(RadioSetting::Agc.id(), &Value::Bool(true)).unwrap();
    v
}

/// Waits until the bytes that `d` read stop growing for 100 ms.
fn settle(d: &RtlUsbDriver) {
    let mut last = (d.bytes_read(), Instant::now());
    let end = Instant::now() + Duration::from_secs(2);
    while Instant::now() < end && last.1.elapsed() < Duration::from_millis(100) {
        std::thread::sleep(Duration::from_millis(5));
        let n = d.bytes_read();
        if n != last.0 {
            last = (n, Instant::now());
        }
    }
}

/// The script with every cut bulk item longer than `size` split into items of at most `size` bytes, so that no item
/// is read in parts: the scripted bus cannot read a cut payload from an offset past its head.
fn split_cut_items(script: Script, size: usize) -> Script {
    let mut out = Script {
        entries: Vec::new(),
        ..script.clone()
    };
    for e in script.entries {
        match e {
            Entry::Bulk(
                b @ BulkItem {
                    kind: BulkKind::Data(Payload::Cut { .. }),
                    ..
                },
            ) => {
                let BulkKind::Data(Payload::Cut { len, crc, head }) = &b.kind else {
                    unreachable!()
                };
                let mut at = 0;
                while at < *len {
                    let n = size.min(len - at);
                    out.entries.push(Entry::Bulk(BulkItem {
                        kind: BulkKind::Data(Payload::Cut {
                            len: n,
                            crc: *crc,
                            head: if at == 0 { head.clone() } else { Vec::new() },
                        }),
                        ..b.clone()
                    }));
                    at += n;
                }
            }
            e => out.entries.push(e),
        }
    }
    out
}

#[test]
fn the_recording_of_the_dongle_replays() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/usb/rtl/smart_v5c_open_fm.usbscript");
    let script = Script::load(&path).unwrap();
    let recorded: u64 = script
        .entries
        .iter()
        .map(|e| match e {
            Entry::Bulk(b) => match &b.kind {
                BulkKind::Data(p) => p.len() as u64 * u64::from(b.repeat.max(1)),
                _ => 0,
            },
            Entry::Step(_) => 0,
        })
        .sum();
    let bus = Arc::new(FakeUsb::new(split_cut_items(script, 163_840)));

    // The station, then 300 kHz lower.
    let d = RtlUsbDriver::new(bus.clone());
    let mut v = values(&d, CENTRE, 2.4e6);
    let mut src = d.open("", &v).unwrap();
    assert!(
        src.capabilities().name.contains("R820T"),
        "{}",
        src.capabilities().name
    );
    let (w, _r) = ring(1 << 25, SampleType::Cu8);
    src.start(w).unwrap();
    settle(&d);
    v.set(RadioSetting::Centre.id(), &Value::Hz(CENTRE - 300e3))
        .unwrap();
    src.apply(&v).unwrap();
    src.stop();
    let mut total = d.bytes_read();

    // The counter test mode at two rates.
    for rate in [2.4e6, 3.2e6] {
        let d = RtlUsbDriver::new(bus.clone()).with_counter_test(true);
        let mut src = d.open("", &values(&d, CENTRE, rate)).unwrap();
        let (w, _r) = ring(1 << 22, SampleType::Cu8);
        src.start(w).unwrap();
        settle(&d);
        src.stop();
        total += d.bytes_read();
    }
    if let Err(m) = bus.check() {
        panic!("{m}");
    }
    assert_eq!(total, recorded);
}

/// Replays a recording of `examples/rtl_check.rs record`: the open of the dongle at `selector` at 104.9 MHz, a short
/// stream and the stop. Returns the tuner in the name of the radio.
fn replay_open(file: &str, selector: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/vectors/usb/rtl")
        .join(file);
    let script = Script::load(&path).unwrap();
    let recorded: u64 = script
        .entries
        .iter()
        .map(|e| match e {
            Entry::Bulk(b) => match &b.kind {
                BulkKind::Data(p) => p.len() as u64,
                _ => 0,
            },
            Entry::Step(_) => 0,
        })
        .sum();
    let bus = Arc::new(FakeUsb::new(script));
    let d = RtlUsbDriver::new(bus.clone());
    let mut src = d.open(selector, &values(&d, CENTRE, 2.4e6)).unwrap();
    let name = src.capabilities().name.clone();
    let (w, _r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    settle(&d);
    src.stop();
    if let Err(m) = bus.check() {
        panic!("{m}");
    }
    assert_eq!(d.bytes_read(), recorded);
    name
}

#[test]
fn the_recording_of_the_e4000_dongle_replays() {
    let name = replay_open("smartee_xtr_e4000_open.usbscript", "@00-1.4");
    assert!(name.contains("E4000"), "{name}");
}

#[test]
fn the_recording_of_the_nano_dongle_replays() {
    let name = replay_open("nano3_r820t_open.usbscript", "@00-1.1");
    assert!(name.contains("R820T"), "{name}");
}
