//! The driver on a scripted bus: the stream into the ring, the counter test mode with a loss, unplug and re-plug with
//! the settings restored, the stop time, and no allocation in the sample path once the stream runs.

use sdrkit_core::{SettingSet, Value, Values};
use sdrkit_driver_core::{ring, Link, RadioSetting, RingReader, SourceDriver};
use sdrkit_driver_rtlusb::diag::CounterCheck;
use sdrkit_driver_rtlusb::RtlUsbDriver;
use sdrkit_driver_usb::{
    BulkItem, BulkKind, FakeUsb, Payload, RecordOptions, RecordingUsb, Script, Step, UsbDeviceInfo,
    UsbId,
};
use sdrkit_graph::stream::SampleType;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: the layout is passed on as the caller supplied it.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: `p` came from `System` with this layout.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The replies of an R820T dongle that answer every read of the driver; the probe of the absent tuners stalls.
fn dongle_replies(id: u64) -> String {
    format!(
        "out {id} 40 00 00c8 0610 02 -> !stall\n\
         out {id} 40 00 00c6 0610 00 -> !stall\n\
         in {id} c0 00 0120 000a 1 -> 09\n\
         in {id} c0 00 0034 0600 1 -> 69\n\
         in {id} c0 00 0034 0600 5 -> 69005a0014\n\
         in {id} c0 00 0034 0600 3 -> 69005a\n\
         in {id} c0 00 0034 0600 4 -> 69005a00\n"
    )
}

fn info(id: u64) -> UsbDeviceInfo {
    let mut d = UsbDeviceInfo::new(UsbId(id), 0x0bda, 0x2838);
    d.bus = "1".into();
    d.ports = vec![3];
    d.manufacturer = Some("Realtek".into());
    d.product = Some("RTL2838UHIDIR".into());
    d.serial = Some("0001".into());
    d
}

/// A lenient script for dongle 1 with `items` bulk items.
fn script(items: Vec<BulkItem>) -> Script {
    let text = format!(
        "sdrkit-usb 1\nmode lenient\ndevice 1 0bda:2838 bus=\"1\" ports=3 manufacturer=\"Realtek\" \
         product=\"RTL2838UHIDIR\" serial=\"0001\"\n{}{}",
        dongle_replies(1),
        dongle_replies(2)
    );
    let mut s = Script::parse(&text).unwrap();
    for i in items {
        s.bulk(i);
    }
    s
}

/// `n` bulk items of `len` counter bytes from `start`, ready every `after`.
fn counter_items(dev: u64, start: u8, len: usize, n: u32, after: Duration) -> Vec<BulkItem> {
    (0..n)
        .map(|k| {
            let first = start.wrapping_add((k as usize * len) as u8);
            let data = (0..len).map(|i| first.wrapping_add(i as u8)).collect();
            BulkItem {
                after,
                ..BulkItem::data(UsbId(dev), 0x81, data)
            }
        })
        .collect()
}

fn values(d: &RtlUsbDriver, set: &[(RadioSetting, Value)]) -> Values {
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Centre.id(), &Value::Hz(100e6)).unwrap();
    v.set(RadioSetting::Rate.id(), &Value::Hz(2.4e6)).unwrap();
    for (s, x) in set {
        v.set(s.id(), x).unwrap();
    }
    v
}

fn wait_for(what: &str, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Reads what the ring has into `check`.
fn drain(r: &mut RingReader, check: &mut CounterCheck) {
    while r.read_with(1 << 20, |b| check.feed(b)) > 0 {}
}

#[test]
fn the_counter_stream_arrives_whole_and_a_loss_is_counted() {
    // 40 transfers of counter bytes, then 1000 bytes missing, then 40 more.
    let len = 16_384;
    let mut items = counter_items(1, 0, len, 40, Duration::ZERO);
    let next = (40 * len) as u8;
    items.extend(counter_items(
        1,
        next.wrapping_add(232),
        len,
        40,
        Duration::ZERO,
    ));
    let fake = FakeUsb::new(script(items));
    let rec = RecordingUsb::new(Arc::new(fake.clone()), RecordOptions::default());
    let driver = RtlUsbDriver::new(Arc::new(rec.clone())).with_counter_test(true);
    let mut src = driver.open("", &values(&driver, &[])).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    let total = 80 * len as u64;
    wait_for("the bytes", || driver.bytes_read() == total);
    let mut check = CounterCheck::new();
    drain(&mut r, &mut check);
    src.stop();
    assert_eq!(check.bytes, total);
    assert_eq!((check.breaks, check.lost), (1, 232));
    // The test mode was written: page 0 register 0x19 = 0x03.
    let wrote = rec.script().steps().any(|(_, s)| {
        matches!(s, Step::ControlOut { setup, data, .. } if setup.value == 0x1920 && setup.index == 0x0010 && data == &[0x03])
    });
    assert!(wrote, "the counter test mode was not switched on");
}

#[test]
fn an_unplug_is_away_and_a_replug_restores_every_setting() {
    let len = 48_000;
    let mut items = counter_items(1, 0, len, 20, Duration::from_millis(10));
    items.push(BulkItem {
        kind: BulkKind::Unplug,
        ..BulkItem::data(UsbId(1), 0x81, Vec::new())
    });
    items.extend(counter_items(2, 0, len, 400, Duration::from_millis(10)));
    let fake = FakeUsb::new(script(items));
    let rec = RecordingUsb::new(Arc::new(fake.clone()), RecordOptions::default());
    let driver = RtlUsbDriver::new(Arc::new(rec.clone()));
    let set = [
        (RadioSetting::Agc, Value::Bool(false)),
        (RadioSetting::Gain, Value::F64(29.7)),
    ];
    let mut src = driver.open("", &values(&driver, &set)).unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    wait_for("the unplug", || src.link() == Link::Away);
    fake.plug(info(2));
    wait_for("the re-plug", || src.link() == Link::Up);
    let before = driver.bytes_read();
    wait_for("samples of the new dongle", || driver.bytes_read() > before);
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(150),
        "stop took {:?}",
        t.elapsed()
    );
    // The second dongle got the rate, the tune to 100 MHz and the manual gain of 29.7 dB.
    let script = rec.script();
    let outs: Vec<String> = script
        .steps()
        .filter_map(|(_, s)| match s {
            Step::ControlOut {
                device,
                setup,
                data,
                ..
            } if *device == UsbId(2) => Some(format!(
                "{:04x} {:04x} {}",
                setup.value,
                setup.index,
                data.iter().map(|b| format!("{b:02x}")).collect::<String>()
            )),
            _ => None,
        })
        .collect();
    for want in [
        "9f20 0011 0300",
        "0034 0610 1084bb8031ca5b90",
        "0034 0610 0518",
        "0034 0610 0768",
    ] {
        assert!(
            outs.iter().any(|o| o == want),
            "the new dongle did not get `{want}`"
        );
    }
}

#[test]
fn the_sample_path_does_not_allocate_once_it_runs() {
    // 163 840 bytes every 34 ms: 2.4 MS/s for 3 s.
    let len = 163_840;
    let items = vec![BulkItem {
        after: Duration::from_micros(34_133),
        repeat: 90,
        kind: BulkKind::Data(Payload::Full(vec![0x80; len])),
        ..BulkItem::data(UsbId(1), 0x81, Vec::new())
    }];
    let fake = FakeUsb::new(script(items));
    let driver = RtlUsbDriver::new(Arc::new(fake));
    let mut src = driver.open("", &values(&driver, &[])).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    let mut seen = 0u64;
    let mut read = |r: &mut RingReader| {
        while r.read_with(1 << 20, |b| seen += b.len() as u64) > 0 {}
    };
    let warm = Instant::now() + Duration::from_millis(500);
    while Instant::now() < warm {
        read(&mut r);
        std::thread::sleep(Duration::from_millis(2));
    }
    let before = ALLOCS.load(Ordering::Relaxed);
    let bytes = driver.bytes_read();
    let end = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < end {
        read(&mut r);
        std::thread::sleep(Duration::from_millis(2));
    }
    let allocs = ALLOCS.load(Ordering::Relaxed) - before;
    let moved = driver.bytes_read() - bytes;
    src.stop();
    assert!(moved >= 30 * len as u64, "only {moved} bytes moved");
    assert_eq!(allocs, 0, "allocations while the stream ran");
}

#[test]
fn a_live_retune_marks_a_gap_of_at_least_the_settle_time() {
    // 48 000 bytes every 10 ms: 2.4 MS/s.
    let len = 48_000;
    let items = counter_items(1, 0, len, 300, Duration::from_millis(10));
    let fake = FakeUsb::new(script(items));
    let driver = RtlUsbDriver::new(Arc::new(fake)).with_counter_test(true);
    let mut v = values(&driver, &[]);
    let mut src = driver.open("", &v).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cu8);
    src.start(w).unwrap();
    wait_for("the stream", || driver.bytes_read() >= 10 * len as u64);
    let mut check = CounterCheck::new();
    drain(&mut r, &mut check);
    assert!(r.take_gaps(usize::MAX).is_empty());
    v.set(RadioSetting::Centre.id(), &Value::Hz(101e6)).unwrap();
    src.apply(&v).unwrap();
    let after = driver.bytes_read();
    wait_for("the stream after the retune", || {
        driver.bytes_read() >= after + 10 * len as u64
    });
    let gaps = r.take_gaps(usize::MAX);
    src.stop();
    let lost: u64 = gaps.iter().map(|g| g.lost).sum();
    // At least 5 ms of samples, at most the transfers in flight and 5 ms.
    assert!(!gaps.is_empty(), "no gap after the retune");
    assert!(lost >= 12_000, "only {lost} samples in the gap: {gaps:?}");
    assert!(
        lost <= 16 * 163_840 / 2 + 12_000,
        "{lost} samples in the gap"
    );
}
