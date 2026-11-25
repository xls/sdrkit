//! A receive-only check of a real HackRF: prints the board and the firmware, receives 5 s at 100 MHz and 8 MS/s (or the
//! rate given, in MS/s), prints the bytes received against the bytes expected and the mean and RMS of the samples, and
//! records the USB exchange to `tests/vectors/usb/hackrf/probe-100mhz-<rate>msps.txt` with the serial number replaced by
//! zeros. It never transmits.
//!
//! ```text
//! cargo run -p sdrkit-driver-hackrf --example hackrf_probe [-- <address> [<rate MS/s>]]
//! ```

use sdrkit_core::{Owner, Value, Values};
use sdrkit_driver_core::{ring, DriverKind, RING_CAPACITY};
use sdrkit_driver_hackrf::{
    is_dfu, is_hackrf, HackrfDriver, BOARD, FIRMWARE, MEASURED_RATE, SERIAL, SHORTFALLS, USB_API,
};
use sdrkit_driver_usb::{platform_bus, Entry, RecordOptions, RecordingUsb, Script, Step};
use sdrkit_graph::stream::SampleType;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CENTRE_HZ: f64 = 100e6;
const SECONDS: f64 = 5.0;
const PLACEHOLDER: &str = "00000000000000000000000000000000";

fn main() {
    let address = std::env::args().nth(1).unwrap_or_else(|| "hackrf:".into());
    let msps: f64 = std::env::args()
        .nth(2)
        .and_then(|r| r.parse().ok())
        .unwrap_or(8.0);
    let rate = msps * 1e6;
    let rec = RecordingUsb::new(
        platform_bus(),
        RecordOptions {
            keep: 64,
            timing: true,
        },
    );
    let kind = DriverKind::new(Box::new(HackrfDriver::new(Arc::new(rec.clone()))));
    let schema = kind.schema().clone();
    let mut v = Values::defaults(&schema);
    for (key, value) in [("centre_hz", CENTRE_HZ), ("rate", rate)] {
        v.set(
            schema.id(key).expect("a setting of every radio"),
            &Value::Hz(value),
        )
        .expect("a valid value");
    }
    let code = match receive(&kind, &address, &v, rate) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("hackrf_probe: {e}");
            1
        }
    };
    save(rec.script(), msps);
    std::process::exit(code);
}

/// Opens the radio at `address`, prints what it is, receives for [`SECONDS`] and prints the figures.
fn receive(kind: &DriverKind, address: &str, v: &Values, rate: f64) -> Result<(), String> {
    let mut radio = kind.open(address, v).map_err(|e| e.to_string())?;
    radio.poll(0.0, &mut Vec::new());
    let f = radio.results().fields();
    println!("board:    {}", f.get(&BOARD).unwrap_or_default());
    println!(
        "firmware: {} (USB API {})",
        f.get(&FIRMWARE).unwrap_or_default(),
        f.get(&USB_API).unwrap_or_default()
    );
    println!("serial:   {}", f.get(&SERIAL).unwrap_or_default());
    let (w, mut r) = ring(RING_CAPACITY, SampleType::Cs8);
    radio.start(w).map_err(|e| e.to_string())?;
    let t = Instant::now();
    let (mut bytes, mut sum_i, mut sum_q, mut power) = (0u64, 0f64, 0f64, 0f64);
    while t.elapsed().as_secs_f64() < SECONDS {
        r.wait(1, Duration::from_millis(50));
        r.read_with(1 << 16, |b| {
            bytes += b.len() as u64;
            for p in b.as_chunks::<2>().0 {
                let (i, q) = (f64::from(p[0] as i8), f64::from(p[1] as i8));
                sum_i += i;
                sum_q += q;
                power += i * i + q * q;
            }
        });
    }
    let elapsed = t.elapsed().as_secs_f64();
    radio.poll(0.0, &mut Vec::new());
    let f = radio.results().fields();
    let measured = f.get(&MEASURED_RATE);
    let shortfalls = f.get(&SHORTFALLS);
    radio.stop();
    let n = (bytes / 2).max(1) as f64;
    let expected = (elapsed * rate * 2.0) as u64;
    println!(
        "bytes:    {bytes} of {expected} expected in {elapsed:.2} s ({:.1} %)",
        bytes as f64 / expected.max(1) as f64 * 100.0
    );
    if let Some(m) = measured {
        println!("device:   samples at {:.4} MS/s", m / 1e6);
    }
    println!(
        "dropped:  {} shortfalls reported by the device",
        shortfalls.unwrap_or(0)
    );
    println!(
        "mean:     I {:+.3}, Q {:+.3} (of full scale 128)",
        sum_i / n,
        sum_q / n
    );
    println!(
        "RMS:      {:.2} ({:.1} dBFS)",
        (power / n).sqrt(),
        10.0 * (power / n / (128.0 * 128.0)).max(1e-12).log10()
    );
    Ok(())
}

/// Writes the recording with the HackRF devices only and every serial number replaced by zeros.
fn save(mut s: Script, msps: f64) {
    let ours = |d: &sdrkit_driver_usb::UsbDeviceInfo| is_hackrf(d) || is_dfu(d);
    let kept: Vec<_> = s.devices.iter().filter(|d| ours(d)).map(|d| d.id).collect();
    s.devices.retain(|d| ours(d));
    s.entries.retain(|e| match e {
        Entry::Step(Step::Plug { info }) => ours(info),
        Entry::Step(Step::Unplug { device }) => kept.contains(device),
        _ => true,
    });
    for d in &mut s.devices {
        d.serial = d.serial.as_ref().map(|_| PLACEHOLDER.to_string());
    }
    for e in &mut s.entries {
        match e {
            Entry::Step(Step::Plug { info }) => {
                info.serial = info.serial.as_ref().map(|_| PLACEHOLDER.to_string());
            }
            Entry::Step(Step::ControlIn {
                setup,
                reply: Ok(data),
                ..
            }) if setup.request == 0x12 && setup.request_type == 0xc0 => {
                for b in data.iter_mut().skip(8) {
                    *b = 0;
                }
            }
            _ => {}
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/usb/hackrf");
    let path = dir.join(format!("probe-100mhz-{msps}msps.txt"));
    match std::fs::create_dir_all(&dir).and_then(|()| s.save(&path)) {
        Ok(()) => println!("recorded: {}", path.display()),
        Err(e) => eprintln!("hackrf_probe: {}: {e}", path.display()),
    }
}
