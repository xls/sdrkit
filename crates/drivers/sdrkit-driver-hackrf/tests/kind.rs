//! The driver as the engine sees it: the kind with its `discover` command, `devices` table and `address` command, an open
//! radio with its readings and transmitter, the release of the device at a stop, and an overrun reported by the device.

mod common;

use common::*;
use sdrkit_core::publish::field;
use sdrkit_core::{owner, Field, Owner, Record};
use sdrkit_driver_core::{DriverKind, SourceDriver};
use sdrkit_driver_hackrf::{
    HackrfDriver, Timing, BOARD, D_BOARD, D_FIRMWARE, FIRMWARE, TUNE_OFFSET,
};
use sdrkit_driver_usb::{
    Availability, BulkIn, BulkItem, FakeUsb, Script, Setup, UsbBus, UsbDevice, UsbDeviceInfo,
    UsbError, UsbId, UsbWatch,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Runs command `key` of `kind` with `set` among its arguments.
fn run(kind: &mut DriverKind, key: &str, set: &[(&'static str, &str)]) -> Option<Record> {
    let id = kind.commands().id(key).unwrap();
    let mut args = Record::new(kind.commands().def(id).unwrap().args.record_decl().clone());
    for (k, v) in set {
        let f: Field<field::Text> = Field::new(k, "", "");
        assert!(args.set(&f, v));
    }
    owner::invoke(kind, id, &args).unwrap()
}

#[test]
fn the_kind_finds_composes_and_opens_and_the_radio_transmits() {
    let api = 0x0110;
    let ident = open_lines(1, api, 4).replace("out 1 40 01 0000 0000 -\n", "");
    let text = format!(
        "sdrkit-usb 1\n{}\n{ident}{}",
        device_line(1),
        open_lines(1, api, 4)
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut kind = DriverKind::new(Box::new(HackrfDriver::with_timing(
        Arc::new(bus.clone()),
        timing(),
    )));
    assert!(run(&mut kind, "discover", &[]).is_none());
    let r = kind.results();
    let t = r.table(r.decl().table_id("devices").unwrap()).unwrap();
    let row = t.row(&format!("hackrf:{SERIAL}")).unwrap();
    assert_eq!(row.get(&D_FIRMWARE), Some("2026.01.3"));
    assert!(row.get(&D_BOARD).unwrap().starts_with("HackRF One r9"));
    for (device, address) in [
        ("#1", "hackrf:#1"),
        ("@1-2.1", "hackrf:@1-2.1"),
        ("", "hackrf:"),
    ] {
        let reply = run(&mut kind, "address", &[("device", device)]).unwrap();
        assert_eq!(
            reply.get(&sdrkit_driver_core::address::ADDRESS),
            Some(address)
        );
    }

    let mut radio = kind.open("hackrf:#0", &example_values()).unwrap();
    radio.poll(0.0, &mut Vec::new());
    let f = radio.results().fields();
    assert!(f.get(&BOARD).unwrap().starts_with("HackRF One r9"));
    assert_eq!(f.get(&FIRMWARE), Some("2026.01.3"));
    assert_eq!(f.get(&TUNE_OFFSET), Some(2e6));
    assert!(radio.schema().id("lna_db").is_ok() && radio.schema().id("tx_amp").is_ok());
    let tx = radio.transmitter().unwrap();
    assert!(!tx.capabilities().full_duplex);
    drop(tx);
    radio.stop();
    bus.check().unwrap();
}

/// A bus that counts the devices it opened that were dropped.
struct Spy {
    inner: FakeUsb,
    released: Arc<AtomicUsize>,
}

struct SpyDevice {
    inner: Box<dyn UsbDevice>,
    released: Arc<AtomicUsize>,
}

impl Drop for SpyDevice {
    fn drop(&mut self) {
        self.released.fetch_add(1, Ordering::SeqCst);
    }
}

impl UsbBus for Spy {
    fn availability(&self) -> Availability {
        self.inner.availability()
    }
    fn devices(&self) -> Result<Vec<UsbDeviceInfo>, UsbError> {
        self.inner.devices()
    }
    fn watch(&self) -> Result<Box<dyn UsbWatch>, UsbError> {
        self.inner.watch()
    }
    fn open(&self, id: UsbId) -> Result<Box<dyn UsbDevice>, UsbError> {
        Ok(Box::new(SpyDevice {
            inner: self.inner.open(id)?,
            released: self.released.clone(),
        }))
    }
}

impl UsbDevice for SpyDevice {
    fn info(&self) -> &UsbDeviceInfo {
        self.inner.info()
    }
    fn claim(&mut self, interface: u8) -> Result<(), UsbError> {
        self.inner.claim(interface)
    }
    fn control_in(&mut self, s: Setup, b: &mut [u8], t: Duration) -> Result<usize, UsbError> {
        self.inner.control_in(s, b, t)
    }
    fn control_out(&mut self, s: Setup, d: &[u8], t: Duration) -> Result<(), UsbError> {
        self.inner.control_out(s, d, t)
    }
    fn bulk_in(&mut self, ep: u8, size: usize, n: usize) -> Result<Box<dyn BulkIn>, UsbError> {
        self.inner.bulk_in(ep, size, n)
    }
    fn bulk_out(&mut self, ep: u8, d: &[u8], t: Duration) -> Result<usize, UsbError> {
        self.inner.bulk_out(ep, d, t)
    }
    fn clear_halt(&mut self, ep: u8) -> Result<(), UsbError> {
        self.inner.clear_halt(ep)
    }
    fn reset(&mut self) -> Result<(), UsbError> {
        self.inner.reset()
    }
}

#[test]
fn a_stop_releases_the_device_while_a_transmitter_still_holds_the_link() {
    let api = 0x0110;
    let text = format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0000 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        rx_start_lines(1, api)
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let released = Arc::new(AtomicUsize::new(0));
    let spy = Spy {
        inner: bus.clone(),
        released: released.clone(),
    };
    let mut src = HackrfDriver::with_timing(Arc::new(spy), timing())
        .open("hackrf:", &example_values())
        .unwrap();
    let (w, _r) = cs8_ring();
    src.start(w).unwrap();
    let tx = src.transmitter(&tx_values(src.as_ref())).unwrap();
    src.stop();
    assert_eq!(released.load(Ordering::SeqCst), 1);
    assert_eq!(tx.state(), sdrkit_driver_core::TxState::Closed);
    drop(tx);
    bus.check().unwrap();
}

#[test]
fn an_overrun_that_the_device_reports_marks_a_gap() {
    // A lenient script: the settings follow without steps; the streaming state grows from 0 to 1 shortfall of
    // 32 768 bytes.
    let mut state0 = [0u8; 40];
    state0[4] = 2;
    let mut state1 = state0;
    state1[16] = 1;
    state1[21] = 0x80;
    let text = format!(
        "sdrkit-usb 1\nmode lenient\n{}\n{}{}in 1 c0 29 0000 0000 40 -> {}\nin 1 c0 29 0000 0000 40 -> {}\n",
        device_line(1),
        open_lines(1, 0x0110, 4),
        rx_start_lines(1, 0x0110),
        hex(&state0),
        hex(&state1)
    );
    let mut script = Script::parse(&text).unwrap();
    script.bulk(BulkItem {
        repeat: 100,
        after: Duration::from_millis(5),
        ..BulkItem::data(UsbId(1), 0x81, block(3, 16_384))
    });
    let bus = FakeUsb::new(script);
    let t = Timing {
        state_poll: Some(Duration::from_millis(10)),
        ..timing()
    };
    let mut src = HackrfDriver::with_timing(Arc::new(bus.clone()), t)
        .open("hackrf:", &example_values())
        .unwrap();
    let (w, mut r) = cs8_ring();
    src.start(w).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let gaps = r.take_gaps(1 << 22);
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(gaps[0].lost, 16_384);
    src.stop();
    bus.check().unwrap();
}
