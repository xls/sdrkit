//! Script builders and helpers for the tests of the HackRF driver on the scripted bus.

#![allow(dead_code)]

use sdrkit_core::{Value, Values};
use sdrkit_driver_core::{ring, RingReader, Source, SourceDriver};
use sdrkit_driver_hackrf::{HackrfDriver, Timing};
use sdrkit_driver_usb::{FakeUsb, Payload, UsbDeviceInfo, UsbId};
use sdrkit_graph::stream::SampleType;
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The serial number of the test device, as the USB string and as the reply of the part id and serial request.
pub const SERIAL: &str = "0000000000000000a06063c8234e925f";

/// Bytes as hexadecimal digits.
pub fn hex(b: &[u8]) -> String {
    b.iter().fold(String::new(), |mut s, x| {
        let _ = write!(s, "{x:02x}");
        s
    })
}

/// A payload as script text: whole up to 32 bytes, else cut with its CRC.
pub fn payload(b: &[u8]) -> String {
    match Payload::of(b, 32) {
        Payload::Full(d) => hex(&d),
        Payload::Cut { len, crc, head } => format!("cut:{len}:{crc:08x}:{}", hex(&head)),
    }
}

/// The `device` line of device `id`.
pub fn device_line(id: u64) -> String {
    format!(
        "device {id} 1d50:6089 bus=\"1\" ports=2 manufacturer=\"Great Scott Gadgets\" product=\"HackRF One\" serial=\"{SERIAL}\""
    )
}

/// The device as the bus lists it after a re-plug with id `id`.
pub fn device_info(id: u64) -> UsbDeviceInfo {
    let mut d = UsbDeviceInfo::new(UsbId(id), 0x1d50, 0x6089);
    d.bus = "1".into();
    d.ports = vec![2];
    d.manufacturer = Some("Great Scott Gadgets".into());
    d.product = Some("HackRF One".into());
    d.serial = Some(SERIAL.into());
    d
}

/// The open, claim and identification of device `dev` with USB API `api` and board id `board`, then mode off.
pub fn open_lines(dev: u64, api: u16, board: u8) -> String {
    let [lo, hi] = api.to_le_bytes();
    let desc = [
        0x12, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x40, 0x50, 0x1d, 0x89, 0x60, lo, hi, 0x01, 0x02,
        0x04, 0x01,
    ];
    let mut ps = Vec::new();
    for w in [0xa000_cb3cu32, 0x0058_4f53, 0, 0, 0xa060_63c8, 0x234e_925f] {
        ps.extend_from_slice(&w.to_le_bytes());
    }
    let mut s = format!(
        "open {dev}\nclaim {dev} 0\nin {dev} 80 06 0100 0000 18 -> {}\nin {dev} c0 12 0000 0000 24 -> {}\n",
        hex(&desc),
        hex(&ps)
    );
    if api >= 0x0106 {
        let _ = writeln!(s, "in {dev} c0 2d 0000 0000 1 -> 84");
        let _ = writeln!(s, "in {dev} c0 2e 0000 0000 4 -> 00000008");
    }
    // Worked example 6.1, steps 1 to 3.
    let _ = writeln!(s, "in {dev} c0 0e 0000 0000 1 -> {board:02x}");
    let _ = writeln!(s, "in {dev} c0 0f 0000 0000 255 -> {}", hex(b"2026.01.3"));
    let _ = writeln!(s, "out {dev} 40 01 0000 0000 -");
    s
}

/// The start of receive at 433.92 MHz, 8 MS/s, LNA 16 dB, VGA 20 dB, amplifier off, antenna power off: the clock
/// input status from USB API 0x0106 on, then worked example 6.1, steps 4 to 11.
pub fn rx_start_lines(dev: u64, api: u16) -> String {
    let mut s = String::new();
    if api >= 0x0106 {
        let _ = writeln!(s, "in {dev} c0 2c 0000 0000 1 -> 00");
    }
    let _ = write!(
        s,
        "out {dev} 40 06 0000 0000 00127a0001000000\n\
         out {dev} 40 07 8d80 005b -\n\
         out {dev} 40 10 0000 0000 b1010000c0090e00\n\
         out {dev} 40 11 0000 0000 -\n\
         in {dev} c0 13 0000 0010 1 -> 01\n\
         in {dev} c0 14 0000 0014 1 -> 01\n\
         out {dev} 40 17 0000 0000 -\n\
         out {dev} 40 01 0001 0000 -\n"
    );
    s
}

/// The transmit settings at 433.92 MHz, 8 MS/s, TX VGA 20 dB, amplifier off: worked example 6.2, steps 3 to 10.
/// `buffer` is the reply to the buffer size request (USB API 0x0112 or later).
pub fn tx_prepare_lines(dev: u64, api: u16, buffer: Option<&str>) -> String {
    let mut s = format!(
        "out {dev} 40 01 0000 0000 -\n\
         out {dev} 40 06 0000 0000 00127a0001000000\n\
         out {dev} 40 07 8d80 005b -\n\
         out {dev} 40 10 0000 0000 b1010000c0090e00\n\
         out {dev} 40 11 0000 0000 -\n"
    );
    if api >= 0x0112 {
        let _ = writeln!(
            s,
            "in {dev} c0 3d 0000 0000 4 -> {}",
            buffer.unwrap_or("00000100")
        );
    }
    let _ = writeln!(s, "in {dev} c0 15 0000 0014 1 -> 01");
    let _ = writeln!(s, "out {dev} 40 17 0000 0000 -");
    s
}

/// The test timings: no state polling, a watchdog that does not end a test, a quick reopen.
pub fn timing() -> Timing {
    Timing {
        watchdog: Duration::from_secs(30),
        state_poll: None,
        reconnect_gap: Duration::from_millis(20),
        ..Timing::default()
    }
}

/// The values of a radio: the defaults of the kind, then `set` by key.
pub fn values(set: &[(&str, Value)]) -> Values {
    let d = HackrfDriver::new(Arc::new(sdrkit_driver_usb::NoUsb));
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    for (k, x) in set {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    v
}

/// The receive values of worked example 6.1.
pub fn example_values() -> Values {
    values(&[
        ("centre_hz", Value::Hz(433.92e6)),
        ("rate", Value::Hz(8e6)),
        ("lna_db", Value::I64(16)),
        ("vga_db", Value::I64(20)),
    ])
}

/// The transmit values of worked example 6.2 for `src`.
pub fn tx_values(src: &dyn Source) -> Values {
    let schema = src.tx_schema().unwrap();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("tx_frequency").unwrap(), &Value::Hz(433.92e6))
        .unwrap();
    v.set(schema.id("tx_power").unwrap(), &Value::F64(-27.0))
        .unwrap();
    v
}

/// Opens the first HackRF of `bus` with `v`.
pub fn open(bus: &FakeUsb, v: &Values) -> Box<dyn Source> {
    HackrfDriver::with_timing(Arc::new(bus.clone()), timing())
        .open("hackrf:", v)
        .unwrap()
}

/// A ring for the samples of a HackRF.
pub fn cs8_ring() -> (sdrkit_driver_core::RingWriter, RingReader) {
    ring(1 << 22, SampleType::Cs8)
}

/// Waits until `f` holds, at most 5 s.
pub fn wait_for(what: &str, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Reads `n` bytes of samples from `r`, waiting at most 5 s.
pub fn read_bytes(r: &mut RingReader, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n);
    let end = Instant::now() + Duration::from_secs(5);
    while out.len() < n {
        assert!(
            Instant::now() < end,
            "only {} of {n} bytes arrived",
            out.len()
        );
        let want = (n - out.len()) / 2;
        if r.read_with(want, |b| out.extend_from_slice(b)) == 0 {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    out
}

/// A block of bulk data that does not repeat within it, from seed `seed`.
pub fn block(seed: u32, len: usize) -> Vec<u8> {
    let mut x = seed.wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 24) as u8
        })
        .collect()
}

/// `n` transmit samples and their bytes on the wire: I runs through -100 to 99, Q is its negative.
pub fn tx_samples(n: usize) -> (Vec<sdrkit_core::Complex32>, Vec<u8>) {
    let mut iq = Vec::with_capacity(n);
    let mut bytes = Vec::with_capacity(2 * n);
    for k in 0..n {
        let i = (k % 200) as i32 - 100;
        iq.push(sdrkit_core::Complex32::new(
            i as f32 / 128.0,
            -i as f32 / 128.0,
        ));
        bytes.push(i as i8 as u8);
        bytes.push((-i) as i8 as u8);
    }
    (iq, bytes)
}

/// `data` padded with zeros to whole blocks of 16 384 bytes.
pub fn padded(data: &[u8]) -> Vec<u8> {
    let mut v = data.to_vec();
    v.resize(data.len().div_ceil(16_384) * 16_384, 0);
    v
}
