//! The scripted bus: control round trips, the report of a mismatch, timeouts, unplug and plug, and a recording that
//! replays to the same exchanges.

use sdrkit_driver_usb::{
    BulkKind, Entry, FakeUsb, Payload, RecordOptions, RecordingUsb, Script, Setup, UsbBus,
    UsbError, UsbEvent, UsbId,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_millis(100);

const CONTROL: &str = r#"sdrkit-usb 1
mode strict
device 1 0bda:2838 bus="1" ports=2.1 product="Dongle" serial="0001"
open 1
claim 1 0
in 1 c0 00 0020 0010 1 -> 0a
out 1 40 00 0021 0011 0102
out 1 40 00 0022 0011 03 -> !stall
in 1 c0 05 0000 0000 4 -> 01020304
"#;

#[test]
fn control_transfers_follow_the_script() {
    let bus = FakeUsb::from_text(CONTROL).unwrap();
    let list = bus.devices().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].serial.as_deref(), Some("0001"));
    assert_eq!(list[0].path(), "1-2.1");
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut b = [0u8; 1];
    assert_eq!(d.control_in(Setup::vendor(0, 0x20, 0x10), &mut b, T), Ok(1));
    assert_eq!(b, [0x0a]);
    d.control_out(Setup::vendor(0, 0x21, 0x11), &[1, 2], T)
        .unwrap();
    assert_eq!(
        d.control_out(Setup::vendor(0, 0x22, 0x11), &[3], T),
        Err(UsbError::Stall)
    );
    let mut b = [0u8; 4];
    assert_eq!(d.control_in(Setup::vendor(5, 0, 0), &mut b, T), Ok(4));
    assert_eq!(b, [1, 2, 3, 4]);
    bus.check().unwrap();
}

#[test]
fn the_first_mismatch_is_reported() {
    let bus = FakeUsb::from_text(CONTROL).unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut b = [0u8; 1];
    d.control_in(Setup::vendor(0, 0x20, 0x10), &mut b, T)
        .unwrap();
    let e = d
        .control_out(Setup::vendor(0, 0x21, 0x11), &[1, 9], T)
        .unwrap_err();
    let UsbError::Script(text) = e else {
        panic!("not a script error: {e:?}")
    };
    assert_eq!(
        text,
        "step 4: expected `out 1 40 00 0021 0011 0102`, got `out 1 40 00 0021 0011 0109`"
    );
    // A strict script stays failed, and the report keeps the first mismatch.
    assert!(matches!(
        d.control_out(Setup::vendor(0, 0x21, 0x11), &[1, 2], T),
        Err(UsbError::Script(_))
    ));
    let m = bus.check().unwrap_err();
    assert_eq!(m.step, Some(4));
    assert_eq!(m.to_string(), text);
}

#[test]
fn a_step_no_call_took_is_reported() {
    let bus = FakeUsb::from_text(CONTROL).unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let m = bus.check().unwrap_err();
    assert_eq!(
        m.to_string(),
        "step 3: `in 1 c0 00 0020 0010 1` was not called"
    );
}

#[test]
fn a_lenient_script_repeats_and_skips() {
    let text = CONTROL.replace("mode strict", "mode lenient");
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    // No claim; a write with no step succeeds.
    d.control_out(Setup::vendor(7, 0, 0), &[0], T).unwrap();
    let mut b = [0u8; 4];
    assert_eq!(d.control_in(Setup::vendor(5, 0, 0), &mut b, T), Ok(4));
    // The same read again repeats the reply of its step.
    b = [0; 4];
    assert_eq!(d.control_in(Setup::vendor(5, 0, 0), &mut b, T), Ok(4));
    assert_eq!(b, [1, 2, 3, 4]);
    // A read that matches no step fails and is the reported mismatch.
    let e = d.control_in(Setup::vendor(6, 0, 0), &mut b, T).unwrap_err();
    assert_eq!(
        e,
        UsbError::Script("no step is left for `in 1 c0 06 0000 0000 4`".into())
    );
    assert!(bus.check().is_err());
}

const STREAM: &str = r#"sdrkit-usb 1
device 1 1d50:6089 bus="2" ports=1
open 1
claim 1 0
bulk 1 81 00010203
bulk 1 81 after=200000us 04050607
bulk 1 81 !stall
halt 1 81
bulk 1 81 rep=2 0809
bulk 1 81 !unplug
"#;

#[test]
fn bulk_data_keeps_its_pacing_and_errors() {
    let bus = FakeUsb::from_text(STREAM).unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut s = d.bulk_in(0x81, 2, 4).unwrap();
    assert_eq!(s.next(T).unwrap(), &[0, 1]);
    assert_eq!(s.next(T).unwrap(), &[2, 3]);
    let t = Instant::now();
    assert_eq!(s.next(Duration::from_millis(30)), Err(UsbError::Timeout));
    assert_eq!(s.next(Duration::from_secs(2)).unwrap(), &[4, 5]);
    assert!(
        t.elapsed() >= Duration::from_millis(190),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(s.next(T).unwrap(), &[6, 7]);
    assert_eq!(s.next(T), Err(UsbError::Stall));
    s.clear_halt().unwrap();
    assert_eq!(s.next(T).unwrap(), &[8, 9]);
    assert_eq!(s.next(T).unwrap(), &[8, 9]);
    assert_eq!(s.next(T), Err(UsbError::Unplugged));
    assert_eq!(
        d.control_out(Setup::vendor(0, 0, 0), &[], T),
        Err(UsbError::Unplugged)
    );
    assert!(bus.devices().unwrap().is_empty());
    bus.check().unwrap();
}

#[test]
fn an_unplug_ends_a_blocked_read_and_a_plug_arrives() {
    // After its one item the stream has nothing more, so only the unplug can end the read.
    let text = "sdrkit-usb 1\ndevice 1 1d50:6089 bus=\"2\" ports=1\nopen 1\nclaim 1 0\nbulk 1 81 00010203\n";
    let bus = FakeUsb::from_text(text).unwrap();
    let mut watch = bus.watch().unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut s = d.bulk_in(0x81, 4, 4).unwrap();
    assert_eq!(s.next(T).unwrap(), &[0, 1, 2, 3]);
    let reader = std::thread::spawn(move || {
        let t = Instant::now();
        let r = s.next(Duration::from_secs(5)).map(<[u8]>::to_vec);
        (r, t.elapsed())
    });
    std::thread::sleep(Duration::from_millis(50));
    bus.unplug(UsbId(1));
    let (r, waited) = reader.join().unwrap();
    assert_eq!(r, Err(UsbError::Unplugged));
    assert!(waited < Duration::from_secs(1), "{waited:?}");
    assert_eq!(watch.next(T).unwrap(), Some(UsbEvent::Left(UsbId(1))));
    assert!(matches!(bus.open(UsbId(1)), Err(UsbError::NoDevice)));

    let mut again = sdrkit_driver_usb::UsbDeviceInfo::new(UsbId(2), 0x1d50, 0x6089);
    again.bus = "2".into();
    bus.plug(again.clone());
    assert_eq!(watch.next(T).unwrap(), Some(UsbEvent::Arrived(again)));
    assert_eq!(watch.next(Duration::from_millis(10)).unwrap(), None);
}

#[test]
fn scripted_unplug_and_plug_take_effect_in_order() {
    let text = r#"sdrkit-usb 1
device 1 0bda:2838 bus="1" ports=3
open 1
in 1 c0 00 0000 0000 1 -> 01
unplug 1
plug 2 0bda:2838 bus="1" ports=3
open 2
"#;
    let bus = FakeUsb::from_text(text).unwrap();
    let mut watch = bus.watch().unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    let mut b = [0u8; 1];
    d.control_in(Setup::vendor(0, 0, 0), &mut b, T).unwrap();
    assert_eq!(watch.next(T).unwrap(), Some(UsbEvent::Left(UsbId(1))));
    assert!(matches!(
        watch.next(T).unwrap(),
        Some(UsbEvent::Arrived(i)) if i.id == UsbId(2)
    ));
    assert_eq!(
        d.control_in(Setup::vendor(0, 0, 0), &mut b, T),
        Err(UsbError::Unplugged)
    );
    bus.open(UsbId(2)).unwrap();
    bus.check().unwrap();
}

const SESSION: &str = r#"sdrkit-usb 1
mode strict
device 1 0bda:2838 bus="1" ports=2.1 manufacturer="Maker" product="Dongle" serial="0001"
open 1
claim 1 0
in 1 c0 00 0020 0010 2 -> 0a0b
out 1 40 00 0021 0011 0102
bout 1 02 0a0b0c -> 2
bulk 1 81 00010203
bulk 1 81 0405
bulk 1 81 !stall
halt 1 81
bulk 1 81 060708090a0b
"#;

/// Runs one session against `bus` and returns everything the device gave.
fn session(bus: &dyn UsbBus) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let list = bus.devices().unwrap();
    let mut d = bus.open(list[0].id).unwrap();
    d.claim(0).unwrap();
    let mut b = [0u8; 2];
    let n = d
        .control_in(Setup::vendor(0, 0x20, 0x10), &mut b, T)
        .unwrap();
    out.push(b[..n].to_vec());
    d.control_out(Setup::vendor(0, 0x21, 0x11), &[1, 2], T)
        .unwrap();
    out.push(vec![d.bulk_out(0x02, &[10, 11, 12], T).unwrap() as u8]);
    let mut s = d.bulk_in(0x81, 8, 4).unwrap();
    out.push(s.next(T).unwrap().to_vec());
    out.push(s.next(T).unwrap().to_vec());
    assert_eq!(s.next(T), Err(UsbError::Stall));
    s.clear_halt().unwrap();
    out.push(s.next(T).unwrap().to_vec());
    assert_eq!(s.next(Duration::from_millis(5)), Err(UsbError::Timeout));
    out
}

#[test]
fn a_recording_replays_to_the_same_exchanges() {
    let original = Script::parse(SESSION).unwrap();
    assert_eq!(original.to_text(), SESSION);
    let fake = FakeUsb::new(original.clone());
    let rec = RecordingUsb::new(Arc::new(fake.clone()), RecordOptions::default());
    let first = session(&rec);
    fake.check().unwrap();
    let recorded = rec.script();
    assert_eq!(recorded.to_text(), SESSION);

    let replay = FakeUsb::from_text(&recorded.to_text()).unwrap();
    assert_eq!(session(&replay), first);
    replay.check().unwrap();
}

#[test]
fn long_payloads_are_recorded_cut_with_their_crc() {
    let fake = FakeUsb::from_text(SESSION).unwrap();
    let rec = RecordingUsb::new(
        Arc::new(fake.clone()),
        RecordOptions {
            keep: 2,
            timing: false,
        },
    );
    let first = session(&rec);
    let recorded = rec.script();
    let cut: Vec<&Payload> = recorded
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::Bulk(b) => match &b.kind {
                BulkKind::Data(p @ Payload::Cut { .. }) => Some(p),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(cut.len(), 2);
    assert!(cut[0].matches(&[0, 1, 2, 3]));
    assert!(recorded.to_text().contains("bout 1 02 cut:3:"));

    // A cut payload replays as its head and zero bytes, at its full length; a cut bulk OUT matches by its CRC.
    let replay = FakeUsb::new(Script::parse(&recorded.to_text()).unwrap());
    let again = session(&replay);
    assert_eq!(again[2], vec![0, 1, 0, 0]);
    assert_eq!(again[3], first[3]);
    assert_eq!(again[4], vec![6, 7, 0, 0, 0, 0]);
    replay.check().unwrap();
}

const WRITES: &str = r#"sdrkit-usb 1
mode strict
device 1 1d50:6089 bus="1" ports=2 release=0102 speed=high
open 1
claim 1 0
bout 1 02 01020304
bout 1 02 0506 -> 1
bout 1 02 0708090a -> !stall
bout 1 02 0b0c0d0e
"#;

/// Fills the next buffer of `w` with `data` and submits it.
fn send(w: &mut dyn sdrkit_driver_usb::BulkOut, data: &[u8]) {
    let b = w.buffer().expect("a free buffer");
    b[..data.len()].copy_from_slice(data);
    w.submit(data.len()).unwrap();
}

#[test]
fn a_queued_writer_reports_each_transfer_in_order() {
    let bus = FakeUsb::from_text(WRITES).unwrap();
    let info = &bus.devices().unwrap()[0];
    assert_eq!(info.release, 0x0102);
    assert_eq!(info.speed, sdrkit_driver_usb::Speed::High);
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut w = d.bulk_out_queue(0x02, 4, 2).unwrap();
    assert_eq!(w.transfer_size(), 4);
    assert_eq!(w.completed(T), None);
    assert_eq!(
        w.submit(4),
        Err(UsbError::Other(
            "no buffer was taken before the submit".into()
        ))
    );
    send(&mut *w, &[1, 2, 3, 4]);
    send(&mut *w, &[5, 6]);
    // Both buffers are in flight until a result is taken.
    assert_eq!(w.in_flight(), 2);
    assert!(w.buffer().is_none());
    assert_eq!(w.completed(T), Some(Ok(4)));
    send(&mut *w, &[7, 8, 9, 10]);
    assert_eq!(w.completed(T), Some(Ok(1)));
    assert_eq!(w.completed(T), Some(Err(UsbError::Stall)));
    send(&mut *w, &[11, 12, 13, 14]);
    assert_eq!(w.flush(T), Ok(4));
    assert_eq!(w.in_flight(), 0);
    bus.check().unwrap();
}

#[test]
fn a_queued_writer_stops_at_a_mismatch_and_cancels() {
    let bus = FakeUsb::from_text(WRITES).unwrap();
    let mut d = bus.open(UsbId(1)).unwrap();
    d.claim(0).unwrap();
    let mut w = d.bulk_out_queue(0x02, 4, 3).unwrap();
    send(&mut *w, &[1, 2, 3, 4]);
    send(&mut *w, &[5, 6]);
    w.cancel();
    assert_eq!(w.in_flight(), 0);
    let b = w.buffer().unwrap();
    b.copy_from_slice(&[9, 9, 9, 9]);
    let e = w.submit(4).unwrap_err();
    assert_eq!(
        e,
        UsbError::Script("step 5: expected `bout 1 02 0708090a`, got `bout 1 02 09090909`".into())
    );
}

#[test]
fn a_recorded_queued_writer_replays() {
    let fake = FakeUsb::from_text(WRITES).unwrap();
    let rec = RecordingUsb::new(Arc::new(fake.clone()), RecordOptions::default());
    let run = |bus: &dyn UsbBus| {
        let list = bus.devices().unwrap();
        let mut d = bus.open(list[0].id).unwrap();
        d.claim(0).unwrap();
        let mut w = d.bulk_out_queue(0x02, 4, 4).unwrap();
        send(&mut *w, &[1, 2, 3, 4]);
        send(&mut *w, &[5, 6]);
        send(&mut *w, &[7, 8, 9, 10]);
        send(&mut *w, &[11, 12, 13, 14]);
        (0..4).map(|_| w.completed(T).unwrap()).collect::<Vec<_>>()
    };
    let first = run(&rec);
    assert_eq!(first, vec![Ok(4), Ok(1), Err(UsbError::Stall), Ok(4)]);
    fake.check().unwrap();
    let text = rec.script().to_text();
    assert_eq!(text, WRITES);
    let replay = FakeUsb::from_text(&text).unwrap();
    assert_eq!(run(&replay), first);
    replay.check().unwrap();
}
