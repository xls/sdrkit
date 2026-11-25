//! Transmit on the scripted bus: worked example 6.2 byte for byte with the end of a transmission in each of the three
//! firmware cases, the order of the samples across the queue of transfers, a disarm in the middle of a burst, the
//! turnaround receive-transmit-receive with its gap mark, and underruns counted by the device and by the host.

mod common;

use common::*;
use sdrkit_driver_core::{Source, TxState};
use sdrkit_driver_hackrf::proto::tx_write_size;
use sdrkit_driver_usb::{BulkItem, FakeUsb, Script, UsbId};
use std::fmt::Write;
use std::time::Duration;

const BLOCK: usize = 16_384;

/// The `bout` line of `data`.
fn bout(data: &[u8]) -> String {
    format!("bout 1 02 {}\n", payload(data))
}

/// Opens a device with USB API `api`, transmits `samples` samples at 433.92 MHz and ends the transmission: flush,
/// drain, disarm. `after_open` follows the open in the script; `end` is the expected end after the data writes.
fn transmit(api: u16, buffer: Option<&str>, end: &str) {
    let w = tx_write_size(8e6);
    let n = (w + 3000) / 2;
    let (iq, bytes) = tx_samples(n);
    let mut text = format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0002 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        tx_prepare_lines(1, api, buffer)
    );
    text.push_str(&bout(&bytes[..w]));
    text.push_str(&bout(&padded(&bytes[w..])));
    text.push_str(end);
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    assert!(!t.capabilities().full_duplex);
    assert_eq!(t.state(), TxState::Disarmed);
    t.arm().unwrap();
    assert_eq!(t.push(&iq, Duration::from_secs(1)).unwrap(), n);
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    t.disarm();
    assert_eq!(t.state(), TxState::Disarmed);
    let s = t.stats();
    assert_eq!(s.buffers, 2);
    assert_eq!(s.queued, 0);
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn worked_example_6_2_on_release_firmware_flushes_32_kib() {
    let end = format!("{}out 1 40 01 0000 0000 -\n", bout(&[0u8; 32_768]));
    transmit(0x0110, None, &end);
}

#[test]
fn worked_example_6_2_with_the_buffer_size_read_flushes_that_size() {
    let end = format!("{}out 1 40 01 0000 0000 -\n", bout(&[0u8; 65_536]));
    transmit(0x0113, Some("00000100"), &end);
}

#[test]
fn worked_example_6_2_where_the_firmware_drains_by_itself_sends_no_flush() {
    transmit(0x0113, Some("!stall"), "out 1 40 01 0000 0000 -\n");
}

#[test]
fn receive_transmit_receive_marks_the_gap_in_receive() {
    let api = 0x0110;
    let w = tx_write_size(8e6);
    let n = (2 * w + 10_000) / 2;
    let (iq, bytes) = tx_samples(n);
    let mut text = format!(
        "sdrkit-usb 1\n{}\n{}{}{}out 1 40 01 0002 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        rx_start_lines(1, api),
        tx_prepare_lines(1, api, None),
    );
    text.push_str(&bout(&bytes[..w]));
    text.push_str(&bout(&bytes[w..2 * w]));
    text.push_str(&bout(&padded(&bytes[2 * w..])));
    text.push_str(&bout(&[0u8; 32_768]));
    let _ = write!(
        text,
        "out 1 40 01 0000 0000 -\n{}out 1 40 01 0000 0000 -\n",
        rx_start_lines(1, api)
    );
    let mut script = Script::parse(&text).unwrap();
    let before: Vec<u8> = (0..4).flat_map(|i| block(i, BLOCK)).collect();
    let after: Vec<u8> = (10..14).flat_map(|i| block(i, BLOCK)).collect();
    for b in before.chunks(BLOCK) {
        script.bulk(BulkItem::data(UsbId(1), 0x81, b.to_vec()));
    }
    for (i, b) in after.chunks(BLOCK).enumerate() {
        let mut item = BulkItem::data(UsbId(1), 0x81, b.to_vec());
        if i == 0 {
            item.after = Duration::from_millis(300);
        }
        script.bulk(item);
    }
    let bus = FakeUsb::new(script);
    let mut src = open(&bus, &example_values());
    let (wr, mut r) = cs8_ring();
    src.start(wr).unwrap();
    assert_eq!(read_bytes(&mut r, before.len()), before);
    assert!(r.take_gaps(1 << 20).is_empty());

    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    assert_eq!(t.push(&iq, Duration::from_secs(1)).unwrap(), n);
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    std::thread::sleep(Duration::from_millis(20));
    t.disarm();

    // Receive starts again; the first block after it follows a gap of the time away.
    wait_for("receive again", || r.available() >= after.len() / 2);
    let gaps = r.take_gaps(1 << 20);
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    assert_eq!(gaps[0].at, 0);
    // At least the 20 ms of the pause at 8 MS/s.
    assert!(gaps[0].lost >= 160_000, "{gaps:?}");
    assert_eq!(read_bytes(&mut r, after.len()), after);
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn underruns_reported_by_the_device_are_counted() {
    // A lenient script: the writes and settings follow without steps; the streaming state grows from 0 to 2
    // shortfalls of 16 384 bytes.
    let mut state0 = [0u8; 40];
    state0[4] = 4;
    let mut state2 = state0;
    state2[16] = 2;
    state2[21] = 0x40;
    let text = format!(
        "sdrkit-usb 1\nmode lenient\n{}\n{}in 1 c0 15 0000 0014 1 -> 01\n\
         in 1 c0 29 0000 0000 40 -> {}\nin 1 c0 29 0000 0000 40 -> {}\n",
        device_line(1),
        open_lines(1, 0x0110, 4),
        hex(&state0),
        hex(&state2)
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let timing = sdrkit_driver_hackrf::Timing {
        state_poll: Some(Duration::from_millis(10)),
        ..timing()
    };
    let mut src = sdrkit_driver_core::SourceDriver::open(
        &sdrkit_driver_hackrf::HackrfDriver::with_timing(std::sync::Arc::new(bus.clone()), timing),
        "hackrf:",
        &example_values(),
    )
    .unwrap();
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    let (iq, _) = tx_samples(tx_write_size(8e6) / 2);
    t.push(&iq, Duration::from_secs(1)).unwrap();
    wait_for("two underruns", || t.stats().underruns == 2);
    std::thread::sleep(Duration::from_millis(50));
    let s = t.stats();
    assert_eq!(s.underruns, 2);
    assert_eq!(s.underrun_samples, 2 * 8_192);
    t.disarm();
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn a_host_that_delivers_late_is_counted_as_an_underrun() {
    // USB API 0x0104 has no streaming state: the driver counts what the device must have filled with zeros.
    let api = 0x0104;
    let w = tx_write_size(8e6);
    let (iq, bytes) = tx_samples(w);
    let mut text = format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0002 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        tx_prepare_lines(1, api, None)
    );
    text.push_str(&bout(&bytes[..w]));
    text.push_str(&bout(&bytes[w..]));
    text.push_str(&bout(&[0u8; 32_768]));
    text.push_str("out 1 40 01 0000 0000 -\n");
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    let half = w / 2;
    t.push(&iq[..half], Duration::from_secs(1)).unwrap();
    wait_for("the first write", || t.stats().buffers == 1);
    assert_eq!(t.stats().underruns, 0);
    // The first write holds 10 ms of samples; the next comes 80 ms later.
    std::thread::sleep(Duration::from_millis(80));
    t.push(&iq[half..], Duration::from_secs(1)).unwrap();
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    let s = t.stats();
    assert_eq!(s.underruns, 1);
    // About 70 ms at 8 MS/s.
    assert!((400_000..2_000_000).contains(&s.underrun_samples), "{s:?}");
    t.disarm();
    drop(t);
    src.stop();
    bus.check().unwrap();
}

/// `n` samples with their bytes, in which no two transfers are the same: I steps by one each sample, Q each 241
/// samples.
fn distinct_samples(n: usize) -> (Vec<sdrkit_core::Complex32>, Vec<u8>) {
    let mut iq = Vec::with_capacity(n);
    let mut bytes = Vec::with_capacity(2 * n);
    for k in 0..n {
        let i = (k % 256) as u8 as i8;
        let q = ((k / 241) % 256) as u8 as i8;
        iq.push(sdrkit_core::Complex32::new(
            f32::from(i) / 128.0,
            f32::from(q) / 128.0,
        ));
        bytes.push(i as u8);
        bytes.push(q as u8);
    }
    (iq, bytes)
}

/// The script of an open device with USB API `api` that arms for transmit; the writes follow.
fn armed_script(api: u16) -> String {
    format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0002 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        tx_prepare_lines(1, api, None)
    )
}

/// The `bout` lines of `bytes` in transfers of `w`, the last padded to whole blocks.
fn bouts(bytes: &[u8], w: usize) -> String {
    bytes
        .chunks(w)
        .map(|t| {
            if t.len() == w {
                bout(t)
            } else {
                bout(&padded(t))
            }
        })
        .collect()
}

#[test]
fn samples_keep_their_order_across_the_queue() {
    // More than the ring and the transfers in flight hold, pushed in pieces larger than the ring that do not line up
    // with the transfers: each push fills the ring at once, so transfers go out back to back.
    let w = tx_write_size(8e6);
    let total = 7 * w + 5000;
    let (iq, bytes) = distinct_samples(total / 2);
    let mut text = armed_script(0x0110);
    text.push_str(&bouts(&bytes, w));
    text.push_str(&bout(&[0u8; 32_768]));
    text.push_str("out 1 40 01 0000 0000 -\n");
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    for piece in iq.chunks(5 * w / 4 + 1) {
        assert_eq!(t.push(piece, Duration::from_secs(2)).unwrap(), piece.len());
    }
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    t.disarm();
    let s = t.stats();
    assert_eq!(s.buffers, 8);
    assert_eq!(s.sent, (7 * w + BLOCK) as u64 / 2);
    assert_eq!(s.padded, (BLOCK - 5000 + 32_768) as u64 / 2);
    assert_eq!(s.queued, 0);
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn flush_zeros_larger_than_a_transfer_go_out_over_the_whole_pool() {
    // A reported buffer of 1 MiB at 8 MS/s: six full transfers of zeros and one of 65 536 bytes, more than the
    // transfers in flight hold.
    let api = 0x0113;
    let w = tx_write_size(8e6);
    let (iq, bytes) = distinct_samples(w / 2);
    let mut text = format!(
        "sdrkit-usb 1\n{}\n{}{}out 1 40 01 0002 0000 -\n",
        device_line(1),
        open_lines(1, api, 4),
        tx_prepare_lines(1, api, Some("00001000"))
    );
    text.push_str(&bout(&bytes));
    let zeros = vec![0u8; w];
    for _ in 0..6 {
        text.push_str(&bout(&zeros));
    }
    text.push_str(&bout(&[0u8; 65_536]));
    text.push_str("out 1 40 01 0000 0000 -\n");
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    assert_eq!(t.push(&iq, Duration::from_secs(1)).unwrap(), iq.len());
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    t.disarm();
    let s = t.stats();
    assert_eq!(s.buffers, 1);
    assert_eq!(s.padded, 524_288);
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn a_disarm_in_the_middle_of_a_burst_stops_the_writes_before_mode_off() {
    // Two whole transfers go out; the part of a third waits for more samples when the disarm comes. Mode off follows
    // the second write at once, with no tail and no flush zeros. A second burst opens the queue again.
    let w = tx_write_size(8e6);
    let (iq, bytes) = distinct_samples((2 * w + 6000) / 2);
    let mut text = armed_script(0x0110);
    text.push_str(&bouts(&bytes[..2 * w], w));
    text.push_str("out 1 40 01 0000 0000 -\n");
    let (iq2, bytes2) = distinct_samples(1000);
    let _ = write!(
        text,
        "{}out 1 40 01 0002 0000 -\n{}{}out 1 40 01 0000 0000 -\n",
        tx_prepare_lines(1, 0x0110, None),
        bout(&padded(&bytes2)),
        bout(&[0u8; 32_768])
    );
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    assert_eq!(t.push(&iq, Duration::from_secs(2)).unwrap(), iq.len());
    wait_for("two writes", || t.stats().buffers == 2);
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(t.stats().buffers, 2);
    assert_eq!(t.stats().queued, 3000);
    t.disarm();
    assert_eq!(t.state(), TxState::Disarmed);
    assert_eq!(t.stats().queued, 0);

    t.arm().unwrap();
    assert_eq!(t.push(&iq2, Duration::from_secs(1)).unwrap(), 1000);
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    t.disarm();
    assert_eq!(t.stats().buffers, 3);
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn a_late_host_after_a_full_queue_counts_each_underrun() {
    // USB API 0x0104: the host counts. Three transfers (30 ms) go out at once; the fourth comes 80 ms later and the
    // fifth 80 ms after that: two underruns.
    let api = 0x0104;
    let w = tx_write_size(8e6);
    let (iq, bytes) = distinct_samples(5 * w / 2);
    let mut text = armed_script(api);
    text.push_str(&bouts(&bytes, w));
    text.push_str(&bout(&[0u8; 32_768]));
    text.push_str("out 1 40 01 0000 0000 -\n");
    let bus = FakeUsb::from_text(&text).unwrap();
    let mut src = open(&bus, &example_values());
    let mut t = src.transmitter(&tx_values(src.as_ref())).unwrap();
    t.arm().unwrap();
    let half = w / 2;
    t.push(&iq[..3 * half], Duration::from_secs(1)).unwrap();
    wait_for("three writes", || t.stats().buffers == 3);
    assert_eq!(t.stats().underruns, 0);
    std::thread::sleep(Duration::from_millis(80));
    t.push(&iq[3 * half..4 * half], Duration::from_secs(1))
        .unwrap();
    wait_for("four writes", || t.stats().buffers == 4);
    assert_eq!(t.stats().underruns, 1);
    std::thread::sleep(Duration::from_millis(80));
    t.push(&iq[4 * half..], Duration::from_secs(1)).unwrap();
    t.flush().unwrap();
    assert!(t.drain(Duration::from_secs(2)));
    let s = t.stats();
    assert_eq!(s.underruns, 2);
    // About 50 ms and 70 ms at 8 MS/s.
    assert!((600_000..3_000_000).contains(&s.underrun_samples), "{s:?}");
    t.disarm();
    drop(t);
    src.stop();
    bus.check().unwrap();
}

#[test]
fn the_transmit_limits_follow_the_band_table() {
    let bus = FakeUsb::from_text(&format!(
        "sdrkit-usb 1\nmode lenient\n{}\n{}",
        device_line(1),
        open_lines(1, 0x0110, 4)
    ))
    .unwrap();
    let src: Box<dyn Source> = open(&bus, &example_values());
    let l = src.capabilities().transmit.clone().unwrap();
    assert_eq!((l.min_freq_hz, l.max_freq_hz), (1e6, 6e9));
    assert_eq!(l.power_db, (-47.0, 0.0, 1.0));
    assert_eq!(l.output_dbm(2.4e9, 0.0), Some(14.0));
    assert_eq!(l.output_dbm(5e9, 0.0), Some(-5.0));
    assert_eq!(l.output_dbm(5.5e6, -3.0), Some(7.0));
    let schema = src.tx_schema().unwrap();
    assert!(schema.id("tx_amp").is_ok());
}
