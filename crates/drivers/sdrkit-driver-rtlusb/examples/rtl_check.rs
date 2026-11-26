//! Checks of RTL2832U dongles on the bench. It only receives and never writes an EEPROM.
//!
//! `cargo run -p sdrkit-driver-rtlusb --example rtl_check -- <command> ...`, where the command is one of:
//!
//! - `select`: lists the dongles through the `discover` command and opens each by every selector form;
//! - `range <selector> <MHz>...`: tunes the dongle to each frequency and prints the PLL lock;
//! - `gains <selector> <MHz>`: the level and the band power at each gain of the tuner;
//! - `dc <selector> <MHz>`: the DC offset calibration of an E4000 and the mean of I and Q before and after;
//! - `drift <selector> <MHz> <seconds>`: the frequency of the station at `<MHz>` every 10 s;
//! - `dual <selector> <selector> <seconds>`: the counter test mode on two dongles at once;
//! - `record <selector> <MHz> <file>`: records the open, one tune and a short stream as a script;
//! - `spike <selector> <MHz>`: the carrier at the centre against the noise floor, with DC removal off and on.

use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_dsp::Complex32;
use sdrkit_core::{CommandSet, Owner, SettingSet, Value, Values};
use sdrkit_driver_core::{
    ring, DriverKind, RadioSetting, RingReader, Source, SourceDriver, RING_CAPACITY,
};
use sdrkit_driver_rtlusb::diag::CounterCheck;
use sdrkit_driver_rtlusb::{
    dongles, open_selected, Gain, Rtl2832, RtlKindCommand, RtlUsbDriver, Selector, DEVICES, D_PATH,
    D_STATE, D_TUNER,
};
use sdrkit_driver_usb::{platform_bus, Entry, RecordOptions, RecordingUsb, Step, UsbBus};
use sdrkit_graph::stream::SampleType;
use std::sync::Arc;
use std::time::{Duration, Instant};

const RATE: f64 = 2.4e6;

fn values(d: &RtlUsbDriver, centre: f64, gain: Option<f64>) -> Values {
    let schema = d.descriptor().decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(RadioSetting::Centre.id(), &Value::Hz(centre))
        .unwrap();
    v.set(RadioSetting::Rate.id(), &Value::Hz(RATE)).unwrap();
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

/// Reads `n` samples from the ring as complex values of full scale 1.
fn take(r: &mut RingReader, n: usize) -> Vec<Complex32> {
    let mut out = Vec::with_capacity(n);
    let end = Instant::now() + Duration::from_secs(10);
    while out.len() < n && Instant::now() < end {
        r.wait(1, Duration::from_millis(20));
        r.read_with(n - out.len(), |b| {
            for iq in b.as_chunks::<2>().0 {
                out.push(Complex32::new(
                    (f32::from(iq[0]) - 127.5) / 127.5,
                    (f32::from(iq[1]) - 127.5) / 127.5,
                ));
            }
        });
    }
    out
}

fn psd(x: &[Complex32]) -> Vec<f32> {
    let mut p = vec![0f32; 4096];
    welch(
        x,
        4096,
        Window::BlackmanHarris4,
        0.5,
        Scaling::Power,
        &mut p,
    )
    .unwrap();
    p
}

fn band_db(p: &[f32], offset: f64, width: f64) -> f64 {
    let n = p.len() as f64;
    let bin = |f: f64| (((f / RATE) + 0.5) * n).round().clamp(0.0, n - 1.0) as usize;
    let s: f64 = p[bin(offset - width / 2.0)..=bin(offset + width / 2.0)]
        .iter()
        .map(|&x| f64::from(x))
        .sum();
    10.0 * s.max(1e-30).log10()
}

fn stats(x: &[Complex32]) -> (f64, f64, f64) {
    let n = x.len() as f64;
    let (mut i, mut q, mut p) = (0.0, 0.0, 0.0);
    for s in x {
        i += f64::from(s.re);
        q += f64::from(s.im);
        p += f64::from(s.norm_sqr());
    }
    (i / n, q / n, (p / n / 2.0).sqrt())
}

fn mhz(s: &str) -> f64 {
    s.parse::<f64>().expect("a frequency in MHz") * 1e6
}

fn select(bus: Arc<dyn UsbBus>) {
    let mut k = DriverKind::new(Box::new(RtlUsbDriver::new(bus.clone())));
    let c = RtlKindCommand::Discover;
    let commands = RtlKindCommand::commands();
    let args = commands
        .check(c.id(), &commands.def(c.id()).unwrap().new_args())
        .unwrap();
    k.invoke(c.id(), &args).unwrap();
    let id = k.results_decl().table_id(DEVICES.key).unwrap();
    for (_, r) in k.results().table(id).unwrap().rows_newest_first() {
        println!(
            "devices row: path {} tuner {} state {}",
            r.get(&D_PATH).unwrap_or_default(),
            r.get(&D_TUNER).unwrap_or_default(),
            r.get(&D_STATE).unwrap_or_default()
        );
    }
    let list = dongles(bus.as_ref()).unwrap();
    let serials: Vec<String> = list
        .iter()
        .map(|d| d.serial.clone().unwrap_or_default())
        .collect();
    println!(
        "{} dongles; the serial numbers are {}",
        list.len(),
        if serials.windows(2).all(|w| w[0] == w[1]) {
            "equal"
        } else {
            "different"
        }
    );
    let mut sels: Vec<(String, String)> = Vec::new();
    for (i, d) in list.iter().enumerate() {
        sels.push((format!("@{}", d.path()), format!("@{}", d.path())));
        sels.push((format!("#{i}"), format!("#{i}")));
        sels.push((format!("serial of {}", d.path()), serials[i].clone()));
    }
    let d = RtlUsbDriver::new(bus.clone());
    for (label, sel) in sels {
        match d.open(&sel, &values(&d, 100e6, None)) {
            Ok(mut src) => {
                println!("{label}: opens {}", src.capabilities().name);
                src.stop();
            }
            Err(e) => println!("{label}: {e}"),
        }
    }
}

fn open_low(bus: &Arc<dyn UsbBus>, sel: &str) -> Rtl2832 {
    let s = Selector::parse(sel).unwrap();
    let (dev, _) = open_selected(bus.as_ref(), &s, &Default::default()).unwrap();
    Rtl2832::open(dev).unwrap()
}

fn range(bus: Arc<dyn UsbBus>, sel: &str, freqs: &[String]) {
    let mut d = open_low(&bus, sel);
    println!("tuner {}", d.tuner().name());
    d.set_sample_rate(RATE as u32).unwrap();
    for f in freqs {
        let hz = mhz(f) as u64;
        match d.set_frequency(hz) {
            Ok(()) => println!("{f} MHz: locked {}", d.locked()),
            Err(e) => println!("{f} MHz: {e}"),
        }
    }
    d.close();
}

fn gains(bus: Arc<dyn UsbBus>, sel: &str, f: f64, tone: Option<f64>) {
    let d = RtlUsbDriver::new(bus);
    let mut list: Vec<Option<f64>> = vec![None];
    {
        let mut src = d.open(sel, &values(&d, f, Some(0.0))).unwrap();
        list.extend(src.capabilities().gain_steps_db.iter().map(|&g| Some(g)));
        src.stop();
    }
    let mut v = values(&d, f, None);
    let mut src = d.open(sel, &v).unwrap();
    let mut r = start(src.as_mut());
    for g in list {
        v.set(RadioSetting::Agc.id(), &Value::Bool(g.is_none()))
            .unwrap();
        if let Some(g) = g {
            v.set(RadioSetting::Gain.id(), &Value::F64(g)).unwrap();
        }
        src.apply(&v).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        r.skip_all();
        let x = take(&mut r, 1 << 19);
        let p = psd(&x);
        let (i, q, rms) = stats(&x);
        println!(
            "gain {:>6}: RMS {rms:.4}, mean I {i:+.4} Q {q:+.4}, tone {:.1} dB, centre {:.1} dB, +-600 kHz {:.1} / {:.1} dB, +300 kHz {:.1} dB",
            g.map_or("auto".to_string(), |g| format!("{g:.1}")),
            tone.map_or(f64::NAN, |t| band_db(&p, t - f, 10e3)),
            band_db(&p, 0.0, 200e3),
            band_db(&p, 600e3, 200e3),
            band_db(&p, -600e3, 200e3),
            band_db(&p, 300e3, 150e3),
        );
    }
    src.stop();
}

/// Streams 2^19 samples at low level from an open dongle and returns them.
fn raw(d: &mut Rtl2832, n: usize) -> Vec<Complex32> {
    d.reset_buffer().unwrap();
    let mut bulk = d.device().bulk_in(0x81, 163_840, 15).unwrap();
    let mut out = Vec::with_capacity(n);
    let mut skip = 4;
    while out.len() < n {
        let b = bulk.next(Duration::from_secs(1)).unwrap();
        if skip > 0 {
            skip -= 1;
            continue;
        }
        for iq in b.as_chunks::<2>().0 {
            out.push(Complex32::new(
                (f32::from(iq[0]) - 127.5) / 127.5,
                (f32::from(iq[1]) - 127.5) / 127.5,
            ));
        }
    }
    out.truncate(n);
    out
}

fn report(label: &str, x: &[Complex32]) {
    let p = psd(x);
    let (i, q, rms) = stats(x);
    println!(
        "{label}: RMS {rms:.4}, mean I {i:+.4} Q {q:+.4}, +200 kHz tone {:.1} dB, centre {:.1} dB, +-300 kHz {:.1} / {:.1} dB, +-600 kHz {:.1} / {:.1} dB",
        band_db(&p, 200e3, 10e3),
        band_db(&p, 0.0, 200e3),
        band_db(&p, 300e3, 150e3),
        band_db(&p, -300e3, 150e3),
        band_db(&p, 600e3, 200e3),
        band_db(&p, -600e3, 200e3),
    );
}

/// An E4000 register: read with the repeater on.
fn e4k_read(d: &mut Rtl2832, reg: u8) -> u8 {
    let c = d.chip_mut();
    c.repeater(true).unwrap();
    let v = c.i2c_read_reg(0xC8, reg).unwrap();
    c.repeater(false).unwrap();
    v
}

fn e4k_write(d: &mut Rtl2832, reg: u8, v: u8) {
    let c = d.chip_mut();
    c.repeater(true).unwrap();
    c.i2c_write(0xC8, &[reg, v]).unwrap();
    c.repeater(false).unwrap();
}

/// Tunes an E4000 dongle, dumps its main registers, streams, then applies each `reg=value` (hex) and streams again.
fn e4k(bus: Arc<dyn UsbBus>, sel: &str, f: f64, pokes: &[String]) {
    let mut d = open_low(&bus, sel);
    d.set_sample_rate(RATE as u32).unwrap();
    d.set_frequency(f as u64).unwrap();
    d.set_gain(Gain::Manual(420)).unwrap();
    let regs: Vec<String> = [
        0x00u8, 0x07, 0x10, 0x11, 0x12, 0x14, 0x15, 0x16, 0x17, 0x1A, 0x20, 0x24, 0x2D,
    ]
    .iter()
    .map(|&r| format!("{r:02x}={:02x}", e4k_read(&mut d, r)))
    .collect();
    println!("registers {}", regs.join(" "));
    let x = raw(&mut d, 1 << 19);
    report("as set", &x);
    for p in pokes {
        let (r, v) = p.split_once('=').unwrap();
        let (r, v) = (
            u8::from_str_radix(r, 16).unwrap(),
            u8::from_str_radix(v, 16).unwrap(),
        );
        if r >= 0xF0 {
            // Demodulator page r - 0xF0, register v >> 8 is not expressible: use page 0 or 1 writes as `f0`/`f1`.
            continue;
        }
        e4k_write(&mut d, r, v);
        let x = raw(&mut d, 1 << 19);
        report(&format!("after {p}"), &x);
    }
    d.close();
}

/// Tunes from `a` to `b` in steps of 2 MHz and prints the strongest 100 kHz band away from the centre at each step.
fn sweep(bus: Arc<dyn UsbBus>, sel: &str, a: f64, b: f64, pokes: &[String]) {
    let mut d = open_low(&bus, sel);
    d.set_sample_rate(RATE as u32).unwrap();
    d.set_gain(Gain::Manual(420)).unwrap();
    for p in pokes {
        let (r, v) = p.split_once('=').unwrap();
        e4k_write(
            &mut d,
            u8::from_str_radix(r, 16).unwrap(),
            u8::from_str_radix(v, 16).unwrap(),
        );
    }
    let mut f = a;
    while f <= b {
        if d.set_frequency(f as u64).is_err() {
            println!("{:.1} MHz: no lock", f / 1e6);
            f += 2e6;
            continue;
        }
        let x = raw(&mut d, 1 << 18);
        let p = psd(&x);
        let mut bands: Vec<(f64, f64)> = (-10..=10)
            .filter(|k| *k != 0)
            .map(|k| {
                (
                    f + f64::from(k) * 100e3,
                    band_db(&p, f64::from(k) * 100e3, 100e3),
                )
            })
            .collect();
        bands.sort_by(|x, y| y.1.total_cmp(&x.1));
        let floor = bands[bands.len() / 2].1;
        println!(
            "{:.1} MHz: floor {floor:.1} dB, strongest {:.1} MHz {:+.1} dB, {:.1} MHz {:+.1} dB",
            f / 1e6,
            bands[0].0 / 1e6,
            bands[0].1 - floor,
            bands[1].0 / 1e6,
            bands[1].1 - floor
        );
        f += 2e6;
    }
    d.close();
}

/// The DC offset of an E4000: the mean of I and Q with the DC estimator of the demodulator on and off, then the DC
/// calibration of the tuner (one run per mixer and IF stage 1 gain), its table, and the mean again.
fn dc(bus: Arc<dyn UsbBus>, sel: &str, f: f64) {
    let mut d = open_low(&bus, sel);
    d.set_sample_rate(RATE as u32).unwrap();
    d.set_frequency(f as u64).unwrap();
    d.set_gain(Gain::Manual(290)).unwrap();
    report("DC estimator on", &raw(&mut d, 1 << 19));
    d.chip_mut().demod_write1(1, 0xB1, 0x19).unwrap();
    report("DC estimator off", &raw(&mut d, 1 << 19));
    let r16 = e4k_read(&mut d, 0x16);
    let r15 = e4k_read(&mut d, 0x15);
    let mut table = Vec::new();
    for (k, (mix, if1)) in [(0u8, 0u8), (0, 1), (1, 0), (1, 1)].into_iter().enumerate() {
        e4k_write(&mut d, 0x15, (r15 & !1) | mix);
        e4k_write(&mut d, 0x16, (r16 & !1) | if1);
        let r2d = e4k_read(&mut d, 0x2D);
        e4k_write(&mut d, 0x2D, r2d | 0x04);
        e4k_write(&mut d, 0x29, 0x01);
        std::thread::sleep(Duration::from_millis(20));
        let (a, b, c, r29) = (
            e4k_read(&mut d, 0x2A),
            e4k_read(&mut d, 0x2B),
            e4k_read(&mut d, 0x2C),
            e4k_read(&mut d, 0x29),
        );
        println!(
            "calibration {k} (mixer {} dB, IF 1 {} dB): R29 {r29:02x}, I offset {} range {}, Q offset {} range {}",
            if mix == 1 { 12 } else { 4 },
            if if1 == 1 { 6 } else { -3 },
            a & 0x3F,
            c & 0x03,
            b & 0x3F,
            (c >> 4) & 0x03
        );
        table.push((
            (a & 0x3F) | ((c & 0x03) << 6),
            (b & 0x3F) | (((c >> 4) & 0x03) << 6),
        ));
    }
    for (k, (i, q)) in table.iter().enumerate() {
        e4k_write(&mut d, 0x60 + k as u8, *i);
        e4k_write(&mut d, 0x50 + k as u8, *q);
    }
    e4k_write(&mut d, 0x15, r15);
    e4k_write(&mut d, 0x16, r16);
    let r2d = e4k_read(&mut d, 0x2D);
    e4k_write(&mut d, 0x2D, r2d | 0x03);
    report("table on, DC estimator off", &raw(&mut d, 1 << 19));
    let (r70, r71) = (e4k_read(&mut d, 0x70), e4k_read(&mut d, 0x71));
    e4k_write(&mut d, 0x70, r70 | 0x03);
    e4k_write(&mut d, 0x71, r71 | 0x03);
    report(
        "table and time-variant correction on",
        &raw(&mut d, 1 << 19),
    );
    e4k_write(&mut d, 0x70, r70);
    e4k_write(&mut d, 0x71, r71);
    e4k_write(&mut d, 0x2D, r2d & !0x03);
    report("table off again", &raw(&mut d, 1 << 19));
    d.chip_mut().demod_write1(1, 0xB1, 0x1B).unwrap();
    d.close();
}

/// The mean frequency of the signal in `x` relative to the centre, in Hz: the mean phase step of the samples after a
/// moving sum of 8 (about 300 kHz wide).
fn mean_offset(x: &[Complex32]) -> f64 {
    let y: Vec<Complex32> = x
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| c.iter().sum())
        .collect();
    let mut acc = 0.0;
    for w in y.windows(2) {
        acc += f64::from((w[1] * w[0].conj()).arg());
    }
    acc / (y.len() - 1) as f64 * (RATE / 8.0) / std::f64::consts::TAU
}

/// Measures the frequency of the station at `f` every 10 s for `secs` seconds.
fn drift(bus: Arc<dyn UsbBus>, sel: &str, f: f64, secs: f64) {
    let d = RtlUsbDriver::new(bus);
    let mut src = d.open(sel, &values(&d, f, None)).unwrap();
    let mut r = start(src.as_mut());
    let t0 = Instant::now();
    let mut first = None;
    let mut last = 0.0;
    while t0.elapsed().as_secs_f64() <= secs {
        r.skip_all();
        let x = take(&mut r, 2 * RATE as usize);
        let hz = mean_offset(&x);
        first.get_or_insert(hz);
        last = hz;
        println!(
            "t {:5.0} s: station at {hz:+.1} Hz from the centre",
            t0.elapsed().as_secs_f64()
        );
        std::thread::sleep(Duration::from_secs(8));
    }
    src.stop();
    let first = first.unwrap_or(0.0);
    println!(
        "start {first:+.1} Hz, end {last:+.1} Hz: a change of {:+.1} Hz, {:+.3} ppm of {:.1} MHz",
        last - first,
        (last - first) / f * 1e6,
        f / 1e6
    );
}

/// The counter test mode on two dongles at once, one thread each.
fn dual(bus: Arc<dyn UsbBus>, a: &str, b: &str, secs: f64) {
    let run = |sel: String| {
        let bus = bus.clone();
        std::thread::spawn(move || {
            let d = RtlUsbDriver::new(bus).with_counter_test(true);
            let mut src = d.open(&sel, &values(&d, 100e6, None)).unwrap();
            let mut r = start(src.as_mut());
            let mut k = CounterCheck::new();
            let t0 = Instant::now();
            while t0.elapsed().as_secs_f64() < secs {
                r.wait(1, Duration::from_millis(20));
                r.read_with(1 << 20, |b| k.feed(b));
            }
            let dropped = r.stats().dropped();
            let el = t0.elapsed().as_secs_f64();
            src.stop();
            println!(
                "{sel}: {} bytes in {el:.3} s, expected {:.0}; {} discontinuities, {} bytes lost; ring dropped {dropped}",
                k.bytes,
                RATE * 2.0 * el,
                k.breaks,
                k.lost
            );
        })
    };
    let (x, y) = (run(a.to_string()), run(b.to_string()));
    x.join().unwrap();
    y.join().unwrap();
}

/// Records the open of the dongle at `f` and a short stream, with the serial numbers replaced by `SERIAL`.
fn record(bus: Arc<dyn UsbBus>, sel: &str, f: f64, path: &str) {
    let rec = RecordingUsb::new(
        bus,
        RecordOptions {
            keep: 16,
            timing: false,
        },
    );
    let d = RtlUsbDriver::new(Arc::new(rec.clone()));
    let mut src = d.open(sel, &values(&d, f, None)).unwrap();
    let mut r = start(src.as_mut());
    let _ = take(&mut r, 480_000);
    src.stop();
    let mut script = rec.script();
    for info in &mut script.devices {
        info.serial = info.serial.as_ref().map(|_| "SERIAL".to_string());
    }
    for e in &mut script.entries {
        if let Entry::Step(Step::Plug { info }) = e {
            info.serial = info.serial.as_ref().map(|_| "SERIAL".to_string());
        }
    }
    script.save(std::path::Path::new(path)).unwrap();
    println!(
        "recorded {} entries, {} bytes read",
        script.entries.len(),
        d.bytes_read()
    );
}

/// Writes `secs` seconds of the unchanged bytes of the dongle at `f` and `rate` into the file `path`, with the gain in
/// dB `opt[0]` (else automatic) and the bandwidth in Hz `opt[1]` (else from the rate).
fn capture(
    bus: Arc<dyn UsbBus>,
    sel: &str,
    f: f64,
    rate: f64,
    secs: f64,
    path: &str,
    opt: &[String],
) {
    let gain: Option<f64> = opt.first().and_then(|g| g.parse().ok());
    let bandwidth: Option<f64> = opt.get(1).and_then(|b| b.parse().ok());
    let d = RtlUsbDriver::new(bus);
    let mut v = values(&d, f, gain);
    if let Some(b) = bandwidth {
        v.set(RadioSetting::Bandwidth.id(), &Value::Hz(b)).unwrap();
    }
    v.set(RadioSetting::Rate.id(), &Value::Hz(rate)).unwrap();
    let mut src = d.open(sel, &v).unwrap();
    let mut r = start(src.as_mut());
    let mut out = Vec::new();
    let t0 = Instant::now();
    while t0.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(20));
        r.read_with(1 << 20, |b| out.extend_from_slice(b));
    }
    src.stop();
    std::fs::write(path, &out).unwrap();
    println!("wrote {} bytes", out.len());
}

/// Writes 2.2 s of an R820T dongle at `f` and `rate` with the manual gain `g` (tenth dB) and the VGA code `vga` into
/// `path`.
fn vga(bus: Arc<dyn UsbBus>, sel: &str, f: f64, rate: u32, g: i32, vga: u8, path: &str) {
    let mut d = open_low(&bus, sel);
    d.set_sample_rate(rate).unwrap();
    d.set_frequency(f as u64).unwrap();
    d.set_gain(Gain::Manual(g)).unwrap();
    let c = d.chip_mut();
    c.repeater(true).unwrap();
    c.i2c_write(0x34, &[0x0C, 0x60 | vga]).unwrap();
    c.repeater(false).unwrap();
    let x = raw(&mut d, (f64::from(rate) * 2.2) as usize);
    let mut out = Vec::with_capacity(x.len() * 2);
    for s in x {
        out.push((s.re * 127.5 + 127.5).round() as u8);
        out.push((s.im * 127.5 + 127.5).round() as u8);
    }
    std::fs::write(path, out).unwrap();
    d.close();
}

/// The power of the bin at the centre against the median bin, in dB, with DC removal off and on.
fn spike(bus: Arc<dyn UsbBus>, sel: &str, f: f64) {
    let d = RtlUsbDriver::new(bus);
    for on in [false, true] {
        let mut v = values(&d, f, Some(30.0));
        let id = v.schema().id("dc_removal").unwrap();
        v.set(id, &Value::Bool(on)).unwrap();
        let mut src = d.open(sel, &v).unwrap();
        let mut r = start(&mut *src);
        let mut buf = vec![Complex32::default(); 1 << 20];
        // Settle: the first 0.4 s are left out.
        let mut got = 0;
        let end = Instant::now() + Duration::from_secs(10);
        let mut skipped = 0;
        while got < buf.len() && Instant::now() < end {
            r.wait(1, Duration::from_millis(20));
            if skipped < (0.4 * RATE) as usize {
                let mut tmp = vec![Complex32::default(); 65_536];
                skipped += r.read_iq(&mut tmp);
                continue;
            }
            got += r.read_iq(&mut buf[got..]);
        }
        src.stop();
        let p = psd(&buf[..got]);
        let mut sorted: Vec<f32> = p.clone();
        sorted.sort_by(f32::total_cmp);
        let floor = f64::from(sorted[sorted.len() / 2]);
        let centre = f64::from(p[p.len() / 2]);
        println!(
            "{f} Hz, DC removal {}: centre bin {:.1} dB above the median bin",
            if on { "on" } else { "off" },
            10.0 * (centre / floor).log10()
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bus = platform_bus();
    match args.first().map(String::as_str) {
        Some("select") => select(bus),
        Some("range") => range(bus, &args[1], &args[2..]),
        Some("gains") => gains(bus, &args[1], mhz(&args[2]), args.get(3).map(|t| mhz(t))),
        Some("sweep") => sweep(bus, &args[1], mhz(&args[2]), mhz(&args[3]), &args[4..]),
        Some("drift") => drift(bus, &args[1], mhz(&args[2]), args[3].parse().unwrap()),
        Some("dual") => dual(bus, &args[1], &args[2], args[3].parse().unwrap()),
        Some("record") => record(bus, &args[1], mhz(&args[2]), &args[3]),
        Some("capture") => capture(
            bus,
            &args[1],
            mhz(&args[2]),
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
            &args[5],
            &args[6..],
        ),
        Some("vga") => vga(
            bus,
            &args[1],
            mhz(&args[2]),
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
            args[5].parse().unwrap(),
            &args[6],
        ),
        Some("dc") => dc(bus, &args[1], mhz(&args[2])),
        Some("spike") => spike(bus, &args[1], mhz(&args[2])),
        Some("e4k") => e4k(bus, &args[1], mhz(&args[2]), &args[3..]),
        _ => println!("usage: see the module documentation"),
    }
}
