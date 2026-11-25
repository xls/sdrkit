//! Receive on the scripted bus: worked example 6.1 byte for byte, the samples unchanged, the stop order and time, the
//! version rules, and an unplug and re-plug.

mod common;

use common::*;
use sdrkit_core::{SettingSet, Value};
use sdrkit_driver_core::{DriverError, Link, RadioSetting, SourceDriver};
use sdrkit_driver_hackrf::{HackrfDriver, HackrfSetting};
use sdrkit_driver_usb::{BulkItem, FakeUsb, Script, UsbBus, UsbId};
use std::sync::Arc;
use std::time::{Duration, Instant};

const BLOCK: usize = 16_384;

/// The script of worked example 6.1 on firmware with USB API `api`, then mode off at the stop.
fn example_script(api: u16) -> String {
    format!(
        "sdrkit-usb 1\nmode strict\n{}\n{}{}out 1 40 01 0000 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        rx_start_lines(1, api)
    )
}

#[test]
fn worked_example_6_1_receive_at_433_92_mhz() {
    let mut script = Script::parse(&example_script(0x0110)).unwrap();
    let data: Vec<u8> = (0..8).flat_map(|i| block(i, BLOCK)).collect();
    for b in data.chunks(BLOCK) {
        script.bulk(BulkItem::data(UsbId(1), 0x81, b.to_vec()));
    }
    let bus = FakeUsb::new(script);
    let mut src = open(&bus, &example_values());
    let (w, mut r) = cs8_ring();
    let started = src.start(w).unwrap();
    assert_eq!(started.rate_hz, 8e6);
    assert_eq!(started.centre_hz, 433.92e6);
    // The bulk buffers reach the ring unchanged, as signed 8-bit I/Q.
    assert_eq!(read_bytes(&mut r, data.len()), data);
    let t = Instant::now();
    src.stop();
    let took = t.elapsed();
    assert!(took < Duration::from_millis(150), "stop took {took:?}");
    assert_eq!(src.link(), Link::Stopped);
    bus.check().unwrap();
}

#[test]
fn firmware_below_0x0106_gets_no_clock_status_request_and_no_clock_output() {
    let text = example_script(0x0105);
    assert!(!text.contains(" c0 2c ") && !text.contains(" c0 2d "));
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let (w, _r) = cs8_ring();
    src.start(w).unwrap();
    src.stop();
    bus.check().unwrap();

    // The clock output needs USB API 0x0103: on firmware 0x0102 it is refused and nothing more is sent.
    let text = format!(
        "sdrkit-usb 1\n{}\n{}",
        device_line(1),
        open_lines(1, 0x0102, 4).replace("out 1 40 01 0000 0000 -\n", "")
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut v = example_values();
    v.set(
        sdrkit_driver_core::own_id(HackrfSetting::ClockOut),
        &Value::Bool(true),
    )
    .unwrap();
    let e = HackrfDriver::with_timing(Arc::new(bus.clone()), timing())
        .open("hackrf:", &v)
        .err();
    assert_eq!(
        e,
        Some(DriverError::Refused {
            id: sdrkit_driver_core::own_id(HackrfSetting::ClockOut)
        })
    );
    bus.check().unwrap();
}

#[test]
fn antenna_power_on_a_board_without_it_is_refused() {
    let text = format!(
        "sdrkit-usb 1\n{}\n{}",
        device_line(1),
        open_lines(1, 0x0110, 3).replace("out 1 40 01 0000 0000 -\n", "")
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut v = example_values();
    v.set(RadioSetting::BiasTee.id(), &Value::Bool(true))
        .unwrap();
    let e = HackrfDriver::with_timing(Arc::new(bus.clone()), timing())
        .open("hackrf:", &v)
        .err();
    assert_eq!(
        e,
        Some(DriverError::Refused {
            id: RadioSetting::BiasTee.id()
        })
    );
    bus.check().unwrap();
}

#[test]
fn antenna_power_is_sent_again_at_every_start() {
    let api = 0x0110;
    let start = rx_start_lines(1, api).replace("out 1 40 17 0000 0000", "out 1 40 17 0001 0000");
    let mut v = example_values();
    v.set(RadioSetting::BiasTee.id(), &Value::Bool(true))
        .unwrap();
    // A start after a stop opens the device again: the open, then the same start with antenna power.
    let again = format!(
        "sdrkit-usb 1\n{}\n{}{start}out 1 40 01 0000 0000 -\n{}{start}out 1 40 01 0000 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        open_lines(1, api, 4),
    );
    let bus = FakeUsb::from_text(&again).unwrap();
    let mut src = HackrfDriver::with_timing(Arc::new(bus.clone()), timing())
        .open("hackrf:", &v)
        .unwrap();
    for _ in 0..2 {
        let (w, _r) = cs8_ring();
        src.start(w).unwrap();
        src.stop();
    }
    bus.check().unwrap();
}

#[test]
fn an_unplug_reports_away_and_a_replug_restores_the_settings() {
    let api = 0x0110;
    let text = format!(
        "sdrkit-usb 1\n{}\n{}{}{}{}out 2 40 01 0000 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        rx_start_lines(1, api),
        open_lines(2, api, 4),
        rx_start_lines(2, api),
    );
    let mut script = Script::parse(&text).unwrap();
    let first = block(1, BLOCK);
    let second = block(2, BLOCK);
    script.bulk(BulkItem::data(UsbId(1), 0x81, first.clone()));
    script.bulk(BulkItem {
        kind: sdrkit_driver_usb::BulkKind::Unplug,
        ..BulkItem::data(UsbId(1), 0x81, Vec::new())
    });
    script.bulk(BulkItem::data(UsbId(2), 0x81, second.clone()));
    let bus = FakeUsb::new(script);
    let mut src = open(&bus, &example_values());
    let (w, mut r) = cs8_ring();
    src.start(w).unwrap();
    assert_eq!(read_bytes(&mut r, BLOCK), first);
    wait_for("the link to go away", || src.link() == Link::Away);
    assert!(bus.devices().unwrap().is_empty());
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(src.link(), Link::Away);
    bus.plug(device_info(2));
    // The same device under a new id opens with the same settings and streams on.
    assert_eq!(read_bytes(&mut r, BLOCK), second);
    assert_eq!(src.link(), Link::Up);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn an_address_selects_its_device_and_a_dfu_device_is_not_opened() {
    let bus = FakeUsb::from_text("sdrkit-usb 1\ndevice 7 1fc9:000c bus=\"1\" ports=4\n").unwrap();
    let driver = HackrfDriver::with_timing(Arc::new(bus.clone()), timing());
    let e = driver.open("hackrf:", &example_values()).err().unwrap();
    assert!(e.to_string().contains("DFU"), "{e}");
    let rows = driver.devices(true);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].state, "dfu");
    assert_eq!(rows[0].name, "HackRF in DFU mode, not usable");
    assert!(matches!(
        driver.open("hackrf:#0", &example_values()),
        Err(DriverError::Unavailable {
            why: sdrkit_driver_core::Unavailable::NotFound
        })
    ));
    bus.check().unwrap();
}

#[test]
fn the_devices_table_reads_board_and_firmware() {
    let text = format!(
        "sdrkit-usb 1\n{}\n{}",
        device_line(1),
        open_lines(1, 0x0110, 4).replace("out 1 40 01 0000 0000 -\n", "")
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let driver = HackrfDriver::with_timing(Arc::new(bus.clone()), timing());
    let rows = driver.devices(true);
    assert_eq!(rows.len(), 1);
    let r = &rows[0];
    assert_eq!(r.address, format!("hackrf:{SERIAL}"));
    assert_eq!(
        r.board.as_deref(),
        Some("HackRF One r9 (r9, built by Great Scott Gadgets)")
    );
    assert_eq!(r.firmware.as_deref(), Some("2026.01.3"));
    assert_eq!(r.serial.as_deref(), Some(SERIAL));
    assert_eq!(r.path, "1-2");
    assert_eq!(r.state, "free");
    bus.check().unwrap();
    let d = driver.descriptor();
    assert_eq!(d.kind_tables[0].key, "devices");
    assert_eq!(d.kind_commands[0].key, "discover");
}
