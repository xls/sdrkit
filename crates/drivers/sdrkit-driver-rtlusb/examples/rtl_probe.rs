//! Opens the first RTL2832U dongle, prints its tuner and EEPROM strings, receives a station at the frequency given and
//! measures it, then streams the counter of the test mode to count lost bytes. It only receives.
//!
//! `cargo run -p sdrkit-driver-rtlusb --example rtl_probe -- <MHz> [--seconds <s>] [--gain <dB>] [--record <file>]`
//!
//! `--device <selector>` picks the dongle as an address does (`#1`, `@<bus>-<ports>`, a serial number); without
//! it the first. Without `--gain` the tuner sets its gain itself. `--retunes <n> --retune-to <MHz>` retunes n times between the two
//! frequencies while streaming and prints the gap of each retune and the PLL lock. `--counter-seconds <s>` sets the
//! length of the counter runs.
//!
//! With `--record`, the USB exchanges of the measurements (not of the EEPROM read) are saved as a script: bulk payloads
//! cut short with their checksums, the serial number replaced by `SERIAL`.

use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;
use sdrkit_core::{Results, SettingSet, Value, Values};
use sdrkit_driver_core::{ring, RadioSetting, RingReader, Source, SourceDriver, RING_CAPACITY};
use sdrkit_driver_rtlusb::diag::CounterCheck;
use sdrkit_driver_rtlusb::radio::PLL_LOCKED;
use sdrkit_driver_rtlusb::{dongles, Rtl2832, RtlUsbDriver, Selector};
use sdrkit_driver_usb::{platform_bus, Entry, RecordOptions, RecordingUsb, Step, UsbBus};
use sdrkit_graph::stream::SampleType;
use std::sync::Arc;
use std::time::{Duration, Instant};

const RATE: f64 = 2.4e6;
/// Samples kept for the spectrum.
const KEEP: usize = 1 << 20;

/// What a capture measured.
struct Capture {
    bytes: u64,
    secs: f64,
    sum: [f64; 2],
    sum2: [f64; 2],
    kept: Vec<Complex32>,
    counter: Option<CounterCheck>,
}

/// Reads the ring for `secs` seconds; with `counter`, checks the bytes as a counter stream.
fn capture(r: &mut RingReader, secs: f64, counter: bool) -> Capture {
    let mut c = Capture {
        bytes: 0,
        secs: 0.0,
        sum: [0.0; 2],
        sum2: [0.0; 2],
        kept: Vec::with_capacity(KEEP),
        counter: counter.then(CounterCheck::new),
    };
    let start = Instant::now();
    let end = start + Duration::from_secs_f64(secs);
    while Instant::now() < end {
        r.wait(1, Duration::from_millis(20));
        r.read_with(1 << 20, |b| {
            c.bytes += b.len() as u64;
            if let Some(k) = c.counter.as_mut() {
                k.feed(b);
                return;
            }
            for iq in b.as_chunks::<2>().0 {
                let i = (f64::from(iq[0]) - 127.5) / 127.5;
                let q = (f64::from(iq[1]) - 127.5) / 127.5;
                c.sum[0] += i;
                c.sum[1] += q;
                c.sum2[0] += i * i;
                c.sum2[1] += q * q;
                if c.kept.len() < KEEP {
                    c.kept.push(Complex32::new(i as f32, q as f32));
                }
            }
        });
    }
    c.secs = start.elapsed().as_secs_f64();
    c
}

/// The power in dB of the band of `width` Hz at `offset` Hz from the centre of a spectrum `psd` (linear, lowest bin
/// first) at the rate `rate`.
fn band_db(psd: &[f32], rate: f64, offset: f64, width: f64) -> f64 {
    let n = psd.len() as f64;
    let bin = |f: f64| (((f / rate) + 0.5) * n).round().clamp(0.0, n - 1.0) as usize;
    let p: f64 = psd[bin(offset - width / 2.0)..=bin(offset + width / 2.0)]
        .iter()
        .map(|&x| f64::from(x))
        .sum();
    10.0 * p.max(1e-30).log10()
}

fn spectrum(x: &[Complex32]) -> Vec<f32> {
    let mut psd = vec![0f32; 4096];
    welch(
        x,
        4096,
        Window::BlackmanHarris4,
        0.5,
        Scaling::Power,
        &mut psd,
    )
    .unwrap();
    psd
}

fn values(driver: &RtlUsbDriver, centre: f64, rate: f64, gain: Option<f64>) -> Values {
    let schema = driver.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Centre.id(), &Value::Hz(centre))
        .unwrap();
    v.set(RadioSetting::Rate.id(), &Value::Hz(rate)).unwrap();
    v.set(RadioSetting::Agc.id(), &Value::Bool(gain.is_none()))
        .unwrap();
    if let Some(g) = gain {
        v.set(RadioSetting::Gain.id(), &Value::F64(g)).unwrap();
    }
    v
}

fn start(src: &mut dyn Source) -> RingReader {
    let (w, r) = ring(RING_CAPACITY, SampleType::Cu8);
    src.start(w).unwrap();
    r
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(mhz) = args.get(1).and_then(|a| a.parse::<f64>().ok()) else {
        println!("usage: rtl_probe <MHz> [--seconds <s>] [--gain <dB>] [--record <file>]");
        return;
    };
    let opt = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };
    let secs: f64 = opt("--seconds").and_then(|s| s.parse().ok()).unwrap_or(5.0);
    let record = opt("--record");
    let gain: Option<f64> = opt("--gain").and_then(|s| s.parse().ok());
    let retunes: u32 = opt("--retunes").and_then(|s| s.parse().ok()).unwrap_or(0);
    let other: Option<f64> = opt("--retune-to").and_then(|s| s.parse().ok());
    let counter_secs: f64 = opt("--counter-seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(secs);
    let centre = mhz * 1e6;
    let raw = platform_bus();

    // The dongle and its EEPROM.
    let device = opt("--device").unwrap_or_default();
    let sel = Selector::parse(&device).unwrap();
    let list = dongles(raw.as_ref()).unwrap();
    let pick = match &sel {
        Selector::First => list.first(),
        Selector::Index(n) => list.get(*n),
        Selector::Path(p) => list.iter().find(|d| d.path() == *p),
        Selector::Serial(s) => list
            .iter()
            .find(|d| d.serial.as_deref() == Some(s.as_str())),
    };
    let Some(info) = pick else {
        println!("no RTL2832U dongle");
        return;
    };
    println!(
        "dongle {:04x}:{:04x} at USB {}",
        info.vendor_id,
        info.product_id,
        info.path()
    );
    {
        let mut d = Rtl2832::open(raw.open(info.id).unwrap()).unwrap();
        println!("tuner {}", d.tuner().name());
        match d.read_eeprom() {
            Ok(Some(e)) => println!(
                "EEPROM {:04x}:{:04x} manufacturer \"{}\" product \"{}\" serial \"{}\"",
                e.vendor_id, e.product_id, e.manufacturer, e.product, e.serial
            ),
            Ok(None) => println!("EEPROM without the signature"),
            Err(e) => println!("EEPROM not read: {e}"),
        }
        d.close();
    }

    let rec = RecordingUsb::new(
        raw.clone(),
        RecordOptions {
            keep: 16,
            timing: false,
        },
    );
    let bus: Arc<dyn UsbBus> = if record.is_some() {
        Arc::new(rec.clone())
    } else {
        raw
    };

    // The station.
    let driver = RtlUsbDriver::new(bus.clone());
    let mut v = values(&driver, centre, RATE, gain);
    let mut src = driver.open(&device, &v).unwrap();
    let mut r = start(src.as_mut());
    let c = capture(&mut r, secs, false);
    let n = (c.bytes / 2) as f64;
    let expected = RATE * 2.0 * c.secs;
    println!(
        "{mhz} MHz at {} MS/s, {}: {} bytes in {:.3} s, expected {:.0} ({:.2} %)",
        RATE / 1e6,
        gain.map_or("tuner AGC".to_string(), |g| format!("gain {g} dB")),
        c.bytes,
        c.secs,
        expected,
        100.0 * c.bytes as f64 / expected
    );
    let mean = [c.sum[0] / n, c.sum[1] / n];
    let rms = [(c.sum2[0] / n).sqrt(), (c.sum2[1] / n).sqrt()];
    println!(
        "I mean {:+.4} RMS {:.4}, Q mean {:+.4} RMS {:.4} (full scale 1)",
        mean[0], rms[0], mean[1], rms[1]
    );
    let psd = spectrum(&c.kept);
    let centre_db = band_db(&psd, RATE, 0.0, 200e3);
    let up = band_db(&psd, RATE, 600e3, 200e3);
    let down = band_db(&psd, RATE, -600e3, 200e3);
    println!(
        "power in 200 kHz: centre {centre_db:.1} dB, +600 kHz {up:.1} dB, -600 kHz {down:.1} dB: centre is {:.1} dB above the \
         mean of the two",
        centre_db - 10.0 * ((10f64.powf(up / 10.0) + 10f64.powf(down / 10.0)) / 2.0).log10()
    );
    // Tuned 300 kHz low, the station shows at +300 kHz.
    v.set(RadioSetting::Centre.id(), &Value::Hz(centre - 300e3))
        .unwrap();
    src.apply(&v).unwrap();
    r.skip_all();
    let c = capture(&mut r, 1.0, false);
    let psd = spectrum(&c.kept);
    let plus = band_db(&psd, RATE, 300e3, 150e3);
    let minus = band_db(&psd, RATE, -300e3, 150e3);
    println!(
        "tuned 300 kHz low: +300 kHz {plus:.1} dB, -300 kHz {minus:.1} dB: the station is on the {} side",
        if plus > minus { "upper (right)" } else { "lower (mirrored)" }
    );
    if let Some(to) = other {
        let mut results = Results::new(driver.descriptor().decl().results().clone());
        for k in 0..retunes {
            let f = if k % 2 == 0 { to } else { mhz };
            v.set(RadioSetting::Centre.id(), &Value::Hz(f * 1e6))
                .unwrap();
            r.skip_all();
            let began = Instant::now();
            let applied = src.apply(&v);
            let took = began.elapsed().as_secs_f64() * 1e3;
            std::thread::sleep(Duration::from_millis(300));
            let gaps = r.take_gaps(usize::MAX);
            r.skip_all();
            src.poll(&mut results);
            let locked = results.fields().get(&PLL_LOCKED);
            let lost: u64 = gaps.iter().map(|g| g.lost).sum();
            println!(
                "retune {} to {f} MHz: {} in {took:.1} ms, PLL locked {:?}, gap {lost} samples ({:.2} ms) in {} part(s)",
                k + 1,
                if applied.is_ok() { "applied" } else { "refused" },
                locked,
                lost as f64 / RATE * 1e3,
                gaps.len()
            );
        }
    }
    let t = Instant::now();
    src.stop();
    println!("stop took {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    if let Some(l) = src.loss() {
        println!(
            "loss report: {} samples lost by the radio, {} events",
            l.radio_lost, l.radio_events
        );
    }

    // The counter test mode.
    for rate in [2.4e6, 3.2e6] {
        let driver = RtlUsbDriver::new(bus.clone()).with_counter_test(true);
        let mut src = driver
            .open(&device, &values(&driver, centre, rate, gain))
            .unwrap();
        let mut r = start(src.as_mut());
        let c = capture(&mut r, if rate < 3e6 { counter_secs } else { secs }, true);
        let dropped = r.stats().dropped();
        src.stop();
        let k = c.counter.unwrap_or_default();
        println!(
            "counter at {} MS/s: {} bytes in {:.3} s, expected {:.0}; {} discontinuities, {} bytes lost; ring dropped {} \
             samples",
            rate / 1e6,
            k.bytes,
            c.secs,
            rate * 2.0 * c.secs,
            k.breaks,
            k.lost,
            dropped
        );
    }

    if let Some(path) = record {
        let mut script = rec.script();
        for d in &mut script.devices {
            d.serial = d.serial.as_ref().map(|_| "SERIAL".to_string());
        }
        for e in &mut script.entries {
            if let Entry::Step(Step::Plug { info }) = e {
                info.serial = info.serial.as_ref().map(|_| "SERIAL".to_string());
            }
        }
        script.save(std::path::Path::new(&path)).unwrap();
        println!("recorded {} entries to {path}", script.entries.len());
    }
}
