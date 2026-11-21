//! The SSTV transmitter: a picture of each family of modes sent by the stack and read back by its own receiver, the line
//! layouts, VIS codes and times on air checked on the samples against the numbers of the specification, the continuous phase,
//! `stop`, the stated start of a burst, and nothing sent while transmit is off. The transmit stream is reached as the engine
//! reaches it: the stack's graph with the host streams in its context.

use crate::common::{field_num, field_text, pictures};
use hf::front::Demod;
use hf::sstv::{fit_picture, test_picture, Fit, SstvSignal, SstvWriter, MODES};
use hf::typed::{SstvPixels, SstvTxFields};
use sdrkit_codec_image::Raster;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Choice, Clock, Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value, Values,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_hfkbd as hf;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack: 14.230 MHz, upper sideband.
const DIAL: f64 = 14.23e6;
/// The rate of the session.
const RATE: f64 = 12_000.0;
/// The clock of a host that times bursts.
const NOW: f64 = 1_800_000_000.0;

/// The VIS codes of the specification's table, by mode.
const VIS: [(&str, u8); 12] = [
    ("Robot 36", 8),
    ("Robot 72", 12),
    ("Martin M1", 44),
    ("Martin M2", 40),
    ("Scottie S1", 60),
    ("Scottie S2", 56),
    ("Scottie DX", 76),
    ("PD50", 93),
    ("PD90", 99),
    ("PD120", 95),
    ("PD180", 96),
    ("Wraase SC2-180", 55),
];

/// The seconds of the lines of each mode, the column "Lines (s)" of the specification's table of durations.
const LINES_S: [(&str, f64); 12] = [
    ("Robot 36", 36.000),
    ("Robot 72", 72.000),
    ("Martin M1", 114.290),
    ("Martin M2", 58.060),
    ("Scottie S1", 109.624),
    ("Scottie S2", 71.089),
    ("Scottie DX", 268.877),
    ("PD50", 49.684),
    ("PD90", 89.989),
    ("PD120", 126.103),
    ("PD180", 187.052),
    ("Wraase SC2-180", 182.022),
];

/// One write to the transmit stream: its samples, its end mark and its stated start.
type Write = (Vec<Complex32>, bool, Option<f64>);

/// The transmit stream: each write, and the lead that the host asks for.
#[derive(Default)]
struct Sink {
    writes: Mutex<Vec<Write>>,
    lead: Duration,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), end, start));
        }
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

/// One burst: its samples, whether its end mark came, and the stated starts in it (the place and the UTC).
struct Burst {
    samples: Vec<Complex32>,
    ended: bool,
    starts: Vec<(usize, f64)>,
}

impl Sink {
    fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, end, start) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push(Burst {
                    samples: Vec::new(),
                    ended: false,
                    starts: Vec::new(),
                });
            }
            let b = out.last_mut().unwrap();
            if let Some(utc) = start {
                b.starts.push((b.samples.len(), *utc));
            }
            b.samples.extend_from_slice(s);
            b.ended = *end;
            open = !end;
        }
        out
    }
}

/// The index of mode `name` in [`MODES`].
fn index(name: &str) -> u16 {
    MODES.iter().position(|m| m.name == name).unwrap() as u16
}

/// The stack set to send in `mode` with `fit`, transmit on, and its transmit stream. With `lead`, on a live radio whose host
/// times bursts with that lead, at the clock [`NOW`].
fn station(mode: &str, fit: Fit, lead: Option<Duration>) -> (Box<dyn Receiver>, Arc<Sink>) {
    station_with(mode, fit, lead, &[])
}

/// [`station`] with the settings `set` (key and value) too.
fn station_with(
    mode: &str,
    fit: Fit,
    lead: Option<Duration>,
    set: &[(&str, Value)],
) -> (Box<dyn Receiver>, Arc<Sink>) {
    let schema = (hf::SSTV.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in set {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    s.set(schema.id("tx_mode").unwrap(), &Value::Choice(index(mode)))
        .unwrap();
    s.set(schema.id("fit").unwrap(), &Value::Choice(fit.index()))
        .unwrap();
    let mut rx = (hf::SSTV.make_receiver)(&StackConfig::new(s)).unwrap();
    let input = InputInfo::new(RATE, DIAL, DIAL);
    rx.configure(&if lead.is_some() {
        input
    } else {
        input.recording()
    })
    .unwrap();
    let sink = Arc::new(Sink {
        lead: lead.unwrap_or_default(),
        ..Sink::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

/// Runs `send` with the picture `rgb` of `w` x `h` pixels.
fn send(
    rx: &mut dyn Receiver,
    w: u32,
    h: u32,
    rgb: &[u8],
) -> Result<(), sdrkit_core::CommandError> {
    let id = Owner::commands(rx).id("send").unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    for (k, v) in [
        ("pixels", Value::Bytes(Arc::from(rgb))),
        ("width", Value::I64(i64::from(w))),
        ("height", Value::I64(i64::from(h))),
        ("pixel_format", Value::Choice(SstvPixels::Rgb8.index())),
    ] {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, &v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

fn stop(rx: &mut dyn Receiver) {
    let id = Owner::commands(rx).id("stop").unwrap();
    let a = Owner::commands(rx).def(id).unwrap().new_args();
    owner::invoke(rx, id, &a).unwrap();
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
fn idle(rx: &mut dyn Receiver, seconds: f64) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

fn fields(rx: &dyn Receiver) -> SstvTxFields<'_> {
    SstvTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// The frequency of each phase step of `x` at `rate`, Hz: element n is the step from sample n to n + 1.
fn steps(x: &[Complex32], rate: f64) -> Vec<f64> {
    x.windows(2)
        .map(|p| f64::from((p[1] * p[0].conj()).arg()) * rate / TAU)
        .collect()
}

/// The picture that the stack's receiver reads from `x` (complex at [`RATE`], the dial at 0 Hz), with 0.5 s of silence before
/// and 2 s after: the raster, the mode name and the VIS code.
fn receive(x: &[Complex32]) -> (Raster, String, f64) {
    receive_with(x, Demod::Usb)
}

/// [`receive`] with the demodulator `demod`.
fn receive_with(x: &[Complex32], demod: Demod) -> (Raster, String, f64) {
    let schema = (hf::SSTV.settings)();
    let mut s = Values::defaults(&schema);
    s.set(schema.id("demod").unwrap(), &Value::Choice(demod.index()))
        .unwrap();
    let mut rx = (hf::SSTV.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(RATE, 0.0, 0.0).recording())
        .unwrap();
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .unwrap();
    let mut tap = g.tap("dec.pictures", 1 << 16).unwrap();
    let mut y = vec![Complex32::default(); (0.5 * RATE) as usize];
    y.extend_from_slice(x);
    y.extend(std::iter::repeat_n(
        Complex32::default(),
        (2.0 * RATE) as usize,
    ));
    let mut out = Output::default();
    for c in y.chunks(800) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let res = Owner::results(rx.as_ref());
    let (mode, vis) = (field_text(res, "mode"), field_num(res, "vis"));
    let p = pictures(&mut tap).pop().expect("a picture");
    (p, mode, vis)
}

/// The mean absolute error per colour of a received picture against the RGB rows sent.
fn mean_error(p: &Raster, sent: &[u8]) -> f64 {
    let (w, h) = (p.width() as usize, p.height() as usize);
    let mut err = 0.0;
    for y in 0..h {
        let row = p.row(y as u32).unwrap();
        for x in 0..w {
            let i = (y * w + x) * 3;
            for (k, c) in [2usize, 1, 0].into_iter().enumerate() {
                err += (f64::from(row[4 * x + c]) - f64::from(sent[i + k])).abs();
            }
        }
    }
    err / (w * h * 3) as f64
}

/// Checks that every phase step of `x` lies between the lowest and the highest tone (1100 and 2300 Hz) within 1 Hz, and
/// that the level is 0.5 of full scale.
fn check_phase(x: &[Complex32], rate: f64) {
    for (n, f) in steps(x, rate).into_iter().enumerate() {
        assert!(
            (1099.0..=2301.0).contains(&f),
            "a phase step of {f} Hz at sample {n}"
        );
    }
    for (n, z) in x.iter().enumerate() {
        assert!(
            (z.norm() - 0.5).abs() < 1e-3,
            "level {} at sample {n}",
            z.norm()
        );
    }
}

/// The nominal time on air of `mode`, ms: the VIS header (910 ms), the Scottie start pulse (9 ms), and the lines of the
/// specification's table of durations.
fn nominal_ms(mode: &str) -> f64 {
    let lines = LINES_S.iter().find(|r| r.0 == mode).unwrap().1 * 1000.0;
    let pulse = if mode.starts_with("Scottie") {
        9.0
    } else {
        0.0
    };
    910.0 + pulse + lines
}

/// The test picture at 400 x 300 (a shape that none of the modes has), sent by the stack in `mode` and read back by its own
/// receiver. Checks the VIS code, the mode, the time on air, the phase and the end mark; returns the mean error against the
/// picture as fitted to the mode.
fn round_trip(mode: &str) -> f64 {
    let (w, h) = (400, 300);
    let pic = test_picture(w, h);
    let (mut rx, sink) = station(mode, Fit::Bars, None);
    send(rx.as_mut(), w, h, &pic).expect("the picture is sent");
    assert!(fields(rx.as_ref()).tx_sending);
    idle(rx.as_mut(), nominal_ms(mode) / 1000.0 + 0.5);
    assert!(!fields(rx.as_ref()).tx_sending);
    let bursts = sink.bursts();
    assert_eq!(bursts.len(), 1, "{mode}: one burst");
    let b = &bursts[0];
    assert!(b.ended, "{mode}: the end mark");
    let ms = b.samples.len() as f64 / RATE * 1000.0;
    assert!(
        (ms - nominal_ms(mode)).abs() < 1.0,
        "{mode}: {ms} ms on air, nominal {}",
        nominal_ms(mode)
    );
    check_phase(&b.samples, RATE);
    let (p, name, vis) = receive(&b.samples);
    let code = VIS.iter().find(|r| r.0 == mode).unwrap().1;
    assert_eq!(name, mode);
    assert_eq!(vis, f64::from(code), "{mode}: VIS");
    let m = hf::sstv::mode(mode).unwrap();
    let sent = fit_picture(&pic, w, h, SstvPixels::Rgb8, &m, Fit::Bars).unwrap();
    let e = mean_error(&p, &sent);
    eprintln!("{mode}: mean error {e:.2} levels");
    e
}

#[test]
fn sstv_tx_martin_round_trip() {
    let e = round_trip("Martin M2");
    assert!(e < 12.0, "{e}");
}

#[test]
fn sstv_tx_robot_round_trip() {
    let e = round_trip("Robot 36");
    assert!(e < 12.0, "{e}");
}

#[test]
fn sstv_tx_pd_round_trip() {
    let e = round_trip("PD50");
    assert!(e < 12.0, "{e}");
}

/// Scottie with the start pulse (through the stack) and without it (the same tone plan, the pulse left out).
#[test]
fn sstv_tx_scottie_round_trip_with_and_without_the_start_pulse() {
    let e = round_trip("Scottie S2");
    assert!(e < 12.0, "{e}");
    let m = hf::sstv::mode("Scottie S2").unwrap();
    let pic = test_picture(m.w, m.h);
    let mut w = SstvWriter::new(
        SstvSignal::new(m, pic.clone()).with_start_pulse(false),
        RATE,
        0.5,
    );
    let mut x =
        vec![Complex32::default(); ((nominal_ms("Scottie S2") / 1000.0 + 1.0) * RATE) as usize];
    let n = w.fill(&mut x);
    x.truncate(n);
    assert!(((n as f64 / RATE * 1000.0) - (nominal_ms("Scottie S2") - 9.0)).abs() < 1.0);
    let (p, name, vis) = receive(&x);
    assert_eq!((name.as_str(), vis), ("Scottie S2", 56.0));
    let e = mean_error(&p, &pic);
    eprintln!("Scottie S2 without the start pulse: mean error {e:.2} levels");
    assert!(e < 12.0, "{e}");
}

/// The mean step frequency of `f` (the phase steps of a signal at `rate`) over `a` to `b` ms.
fn mean_hz(f: &[f64], rate: f64, a: f64, b: f64) -> f64 {
    let (i, j) = (
        (a * rate / 1000.0).ceil() as usize,
        (b * rate / 1000.0).floor() as usize,
    );
    f[i..j].iter().sum::<f64>() / (j - i) as f64
}

/// The layout of line `l` of `mode` as the specification's table states it, for a white picture: (start ms, end ms, Hz).
/// Scans of R, G, B and Y are white (2300 Hz); the colour differences of white are 128 (1901.57 Hz).
fn spec_layout(mode: &str, l: u32) -> Vec<(f64, f64, f64)> {
    let (white, grey) = (2300.0, 1500.0 + 800.0 * 128.0 / 255.0);
    let scans = |start: f64, s: f64, gap: f64, hz: &[f64]| {
        let mut v = Vec::new();
        let mut t = start;
        for &f in hz {
            v.push((t, t + s, f));
            if gap > 0.0 {
                v.push((t + s, t + s + gap, 1500.0));
            }
            t += s + gap;
        }
        v
    };
    match mode {
        "Robot 36" => {
            let sep = if l.is_multiple_of(2) { 1500.0 } else { 2300.0 };
            vec![
                (0.0, 9.0, 1200.0),
                (9.0, 12.0, 1500.0),
                (12.0, 100.0, white),
                (100.0, 104.5, sep),
                (104.5, 106.0, 1900.0),
                (106.0, 150.0, grey),
            ]
        }
        "Robot 72" => vec![
            (0.0, 9.0, 1200.0),
            (9.0, 12.0, 1500.0),
            (12.0, 150.0, white),
            (150.0, 154.5, 1500.0),
            (154.5, 156.0, 1900.0),
            (156.0, 225.0, grey),
            (225.0, 229.5, 2300.0),
            (229.5, 231.0, 1900.0),
            (231.0, 300.0, grey),
        ],
        "Martin M1" | "Martin M2" => {
            let s = if mode == "Martin M1" { 146.432 } else { 73.216 };
            let mut v = vec![(0.0, 4.862, 1200.0), (4.862, 5.434, 1500.0)];
            v.extend(scans(5.434, s, 0.572, &[white; 3]));
            v
        }
        "Scottie S1" | "Scottie S2" | "Scottie DX" => {
            let s = match mode {
                "Scottie S1" => 138.240,
                "Scottie S2" => 88.064,
                _ => 345.600,
            };
            vec![
                (0.0, 1.5, 1500.0),
                (1.5, s + 1.5, white),
                (s + 1.5, s + 3.0, 1500.0),
                (s + 3.0, 2.0 * s + 3.0, white),
                (2.0 * s + 3.0, 2.0 * s + 12.0, 1200.0),
                (2.0 * s + 12.0, 2.0 * s + 13.5, 1500.0),
                (2.0 * s + 13.5, 3.0 * s + 13.5, white),
            ]
        }
        "PD50" | "PD90" | "PD120" | "PD180" => {
            let s = match mode {
                "PD50" => 91.520,
                "PD90" => 170.240,
                "PD120" => 121.600,
                _ => 183.040,
            };
            let mut v = vec![(0.0, 20.0, 1200.0), (20.0, 22.08, 1500.0)];
            v.extend(scans(22.08, s, 0.0, &[white, grey, grey, white]));
            v
        }
        _ => {
            let mut v = vec![(0.0, 5.5225, 1200.0), (5.5225, 6.0225, 1500.0)];
            v.extend(scans(6.0225, 235.0, 0.0, &[white; 3]));
            v
        }
    }
}

/// Every mode, a white picture at 48 kHz: the VIS code and its parity from the tones of the samples, and in the first two
/// lines each part of the line at its tone and each change of tone at the time of the specification, to a sample. The line
/// period and the start times of Martin M1 and Scottie S1 are the specification's numbers.
#[test]
fn sstv_tx_layouts_follow_the_specification() {
    let rate = 48_000.0;
    for (name, code) in VIS {
        let m = hf::sstv::mode(name).unwrap();
        let white = vec![255u8; (m.w * m.h * 3) as usize];
        let layout0 = spec_layout(name, 0);
        let period = layout0.last().unwrap().1;
        let header = if name.starts_with("Scottie") {
            919.0
        } else {
            910.0
        };
        let mut w = SstvWriter::new(SstvSignal::new(m, white), rate, 0.5);
        let mut x =
            vec![Complex32::default(); ((header + 2.0 * period + 1.0) * rate / 1000.0) as usize];
        w.fill(&mut x);
        let f = steps(&x, rate);
        // The VIS header: leaders, break, start bit, seven data bits least significant first, parity, stop bit.
        for (a, b, hz) in [
            (0.0, 300.0, 1900.0),
            (300.0, 310.0, 1200.0),
            (310.0, 610.0, 1900.0),
            (610.0, 640.0, 1200.0),
        ] {
            assert!(
                (mean_hz(&f, rate, a + 1.0, b - 1.0) - hz).abs() < 0.5,
                "{name}: header at {a} ms"
            );
        }
        let bit = |i: usize| mean_hz(&f, rate, 641.0 + 30.0 * i as f64, 669.0 + 30.0 * i as f64);
        let mut got = 0u8;
        for i in 0..7 {
            let hz = bit(i);
            assert!(
                (hz - 1100.0).abs() < 0.5 || (hz - 1300.0).abs() < 0.5,
                "{name}: bit {i} at {hz} Hz"
            );
            if hz < 1200.0 {
                got |= 1 << i;
            }
        }
        assert_eq!(got, code, "{name}: VIS code");
        let parity = bit(7) < 1200.0;
        assert_eq!(parity, code.count_ones() % 2 == 1, "{name}: parity");
        assert!(
            (mean_hz(&f, rate, 881.0, 909.0) - 1200.0).abs() < 0.5,
            "{name}: stop bit"
        );
        if name.starts_with("Scottie") {
            assert!(
                (mean_hz(&f, rate, 911.0, 918.0) - 1200.0).abs() < 0.5,
                "{name}: start pulse"
            );
        }
        for l in 0..2 {
            let t0 = header + f64::from(l) * period;
            let layout = spec_layout(name, l);
            for (k, &(a, b, hz)) in layout.iter().enumerate() {
                let edge = 0.05;
                let got = mean_hz(&f, rate, t0 + a + edge, t0 + b - edge);
                assert!(
                    (got - hz).abs() < 0.5,
                    "{name} line {l} part {k} ({a} to {b} ms): {got} Hz, not {hz}"
                );
                // The change of tone into this part, where the tone changes by more than 100 Hz.
                let before = if k == 0 { None } else { Some(layout[k - 1].2) };
                if let Some(p) = before.filter(|p| (p - hz).abs() > 100.0) {
                    let at = (t0 + a) * rate / 1000.0;
                    let lo = (at - 5.0) as usize;
                    let n = (lo..lo + 10)
                        .find(|&n| (f[n] - hz).abs() < (f[n] - p).abs())
                        .unwrap();
                    assert!(
                        (n as f64 - at).abs() <= 1.0,
                        "{name} line {l}: the tone changes at sample {n}, not {at}"
                    );
                }
            }
        }
        // The line period and the start times that the specification lists.
        assert!(
            (period - m.line_ms).abs() < 1e-9,
            "{name}: line period {period}"
        );
        if name == "Martin M1" {
            let starts: Vec<f64> = layout0
                .iter()
                .filter(|p| p.2 == 2300.0)
                .map(|p| p.0)
                .collect();
            assert_eq!(starts, [5.434, 152.438, 299.442]);
            assert!((period - 446.446).abs() < 1e-9);
        }
        if name == "Scottie S1" {
            let at = |hz: f64, k: usize| layout0.iter().filter(|p| p.2 == hz).nth(k).unwrap().0;
            assert!((at(2300.0, 0) - 1.5).abs() < 1e-9);
            assert!((at(2300.0, 1) - 141.24).abs() < 1e-9);
            assert!((at(1200.0, 0) - 279.48).abs() < 1e-9);
            assert!((at(2300.0, 2) - 289.98).abs() < 1e-9);
            assert!((period - 428.22).abs() < 1e-9);
        }
    }
}

/// Every mode at 8 kHz: the time on air of a picture is the nominal one within 1 ms.
#[test]
fn sstv_tx_every_time_on_air_is_nominal() {
    let rate = 8000.0;
    for (name, _) in LINES_S {
        let m = hf::sstv::mode(name).unwrap();
        let mut w = SstvWriter::new(SstvSignal::new(m, test_picture(m.w, m.h)), rate, 0.5);
        let mut x = vec![Complex32::default(); (rate * 10.0) as usize];
        let mut n = 0;
        loop {
            let k = w.fill(&mut x);
            n += k;
            if k < x.len() {
                break;
            }
        }
        assert!(w.finished());
        let ms = n as f64 / rate * 1000.0;
        assert!(
            (ms - nominal_ms(name)).abs() < 1.0,
            "{name}: {ms} ms, nominal {}",
            nominal_ms(name)
        );
    }
}

/// `stop` ends the picture at once: the next pass writes one silent sample with the end mark, and nothing follows.
#[test]
fn sstv_tx_stop_ends_the_picture_at_once() {
    let m = hf::sstv::mode("Robot 36").unwrap();
    let (mut rx, sink) = station("Robot 36", Fit::Bars, None);
    send(rx.as_mut(), m.w, m.h, &test_picture(m.w, m.h)).unwrap();
    idle(rx.as_mut(), 2.0);
    let f = fields(rx.as_ref());
    assert!(f.tx_sending);
    assert_eq!(f.tx_lines, Some(240));
    assert!(f.tx_line.unwrap() > 5, "{:?}", f.tx_line);
    let before: usize = sink.bursts().iter().map(|b| b.samples.len()).sum();
    stop(rx.as_mut());
    assert!(!fields(rx.as_ref()).tx_sending);
    idle(rx.as_mut(), 1.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    assert_eq!(b[0].samples.len(), before + 1);
    assert_eq!(b[0].samples[before], Complex32::default());
    // A new picture can be sent after the stop.
    send(rx.as_mut(), m.w, m.h, &test_picture(m.w, m.h)).unwrap();
}

/// On a live radio whose host gives a transmit lead, the first sample states its time on air: the clock, the lead and two
/// passes of 0.1 s. Without a lead nothing is stated.
#[test]
fn sstv_tx_states_the_burst_start() {
    let m = hf::sstv::mode("Robot 36").unwrap();
    let pic = test_picture(m.w, m.h);
    let (mut rx, sink) = station("Robot 36", Fit::Bars, Some(Duration::from_millis(300)));
    send(rx.as_mut(), m.w, m.h, &pic).unwrap();
    idle(rx.as_mut(), 0.5);
    let b = sink.bursts();
    assert_eq!(b[0].starts.len(), 1);
    let (at, utc) = b[0].starts[0];
    assert_eq!(at, 0);
    assert!((utc - (NOW + 0.3 + 0.2)).abs() < 1e-6, "{}", utc - NOW);
    let (mut rx, sink) = station("Robot 36", Fit::Bars, None);
    send(rx.as_mut(), m.w, m.h, &pic).unwrap();
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts()[0].starts.is_empty());
}

/// With transmit off, or with pixels that do not match the size, `send` is refused and nothing is written.
#[test]
fn sstv_tx_refuses_without_transmit_and_a_wrong_picture() {
    let m = hf::sstv::mode("Robot 36").unwrap();
    let pic = test_picture(m.w, m.h);
    let (mut rx, sink) = station("Robot 36", Fit::Bars, None);
    rx.set_transmit(false);
    assert!(send(rx.as_mut(), m.w, m.h, &pic).is_err());
    rx.set_transmit(true);
    assert!(send(rx.as_mut(), m.w + 1, m.h, &pic).is_err());
    assert!(send(rx.as_mut(), 0, 0, &[]).is_err());
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
    // Transmit going off during a picture ends it.
    send(rx.as_mut(), m.w, m.h, &pic).unwrap();
    idle(rx.as_mut(), 0.5);
    rx.set_transmit(false);
    idle(rx.as_mut(), 0.5);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
}

/// The fit rules on a 2:1 picture into Martin M1 (320 x 256): bars keep the whole picture in 320 x 160 with 48 black rows
/// above and below; crop keeps the middle 400 x 320 of the picture; stretch fills the frame.
#[test]
fn sstv_tx_fit_rules() {
    let m = hf::sstv::mode("Martin M1").unwrap();
    let (w, h) = (640u32, 320u32);
    // Left half red, right half blue.
    let mut pic = Vec::new();
    for _ in 0..h {
        for x in 0..w {
            pic.extend_from_slice(if x < w / 2 {
                &[255, 0, 0]
            } else {
                &[0, 0, 255]
            });
        }
    }
    let px = |rgb: &[u8], x: u32, y: u32| {
        let i = ((y * m.w + x) * 3) as usize;
        [rgb[i], rgb[i + 1], rgb[i + 2]]
    };
    let bars = fit_picture(&pic, w, h, SstvPixels::Rgb8, &m, Fit::Bars).unwrap();
    assert_eq!(px(&bars, 10, 47), [0, 0, 0]);
    assert_eq!(px(&bars, 10, 48), [255, 0, 0]);
    assert_eq!(px(&bars, 310, 207), [0, 0, 255]);
    assert_eq!(px(&bars, 310, 208), [0, 0, 0]);
    let crop = fit_picture(&pic, w, h, SstvPixels::Rgb8, &m, Fit::Crop).unwrap();
    assert_eq!(px(&crop, 0, 0), [255, 0, 0]);
    assert_eq!(px(&crop, 319, 255), [0, 0, 255]);
    let stretch = fit_picture(&pic, w, h, SstvPixels::Rgb8, &m, Fit::Stretch).unwrap();
    assert_eq!(px(&stretch, 0, 0), [255, 0, 0]);
    assert_eq!(px(&stretch, 0, 255), [255, 0, 0]);
    // A small grey picture is enlarged by the nearest pixel.
    let grey = fit_picture(&[10, 200], 2, 1, SstvPixels::Grey8, &m, Fit::Stretch).unwrap();
    assert_eq!(
        (px(&grey, 0, 0), px(&grey, 159, 255), px(&grey, 160, 0)),
        ([10; 3], [10; 3], [200; 3])
    );
}

/// The test picture in Robot 36 with `demod` on both sides: the burst, and the mean error of the picture read back.
fn round_trip_on(demod: Demod) -> (Vec<Complex32>, f64) {
    let (w, h) = (400, 300);
    let pic = test_picture(w, h);
    let set = [("demod", Value::Choice(demod.index()))];
    let (mut rx, sink) = station_with("Robot 36", Fit::Bars, None, &set);
    send(rx.as_mut(), w, h, &pic).expect("the picture is sent");
    idle(rx.as_mut(), nominal_ms("Robot 36") / 1000.0 + 0.5);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let x = b.remove(0).samples;
    let (p, name, vis) = receive_with(&x, demod);
    assert_eq!((name.as_str(), vis), ("Robot 36", 8.0), "{demod:?}");
    let m = hf::sstv::mode("Robot 36").unwrap();
    let sent = fit_picture(&pic, w, h, SstvPixels::Rgb8, &m, Fit::Bars).unwrap();
    (x, mean_error(&p, &sent))
}

/// LSB: every phase step is the mirror of a tone (-2300 to -1100 Hz) at the level, and the receiver on LSB reads the
/// picture. NFM: the carrier at the dial with a constant level, every phase step within the peak deviation; the receiver on
/// NFM reads the picture; the 99 % power bandwidth is under Carson's 2 (2500 + 2300) Hz. AM refuses `send`.
#[test]
fn sstv_tx_lower_sideband_and_fm() {
    let (x, e) = round_trip_on(Demod::Lsb);
    eprintln!("LSB: mean error {e:.2} levels");
    assert!(e < 12.0, "LSB: {e}");
    for (n, f) in steps(&x, RATE).into_iter().enumerate() {
        assert!((-2301.0..=-1099.0).contains(&f), "LSB: {f} Hz at {n}");
    }
    let (x, e) = round_trip_on(Demod::Nfm);
    eprintln!("NFM: mean error {e:.2} levels");
    assert!(e < 12.0, "NFM: {e}");
    for (n, z) in x.iter().enumerate() {
        assert!(
            (z.norm() - 0.5).abs() < 1e-3,
            "NFM: level {} at {n}",
            z.norm()
        );
    }
    for f in steps(&x, RATE) {
        assert!(f.abs() <= hf::sstv::FM_DEVIATION_HZ + 1.0, "NFM: {f} Hz");
    }
    let (lo, hi) = crate::tx_common::occupied(&x);
    eprintln!(
        "NFM: 99 % power from {lo:.0} to {hi:.0} Hz, {:.0} Hz",
        hi - lo
    );
    assert!(hi - lo < 2.0 * (2500.0 + 2300.0), "{lo} to {hi}");
    let set = [("demod", Value::Choice(Demod::Am.index()))];
    let (mut rx, sink) = station_with("Robot 36", Fit::Bars, None, &set);
    assert!(send(rx.as_mut(), 4, 4, &[0u8; 48]).is_err());
    idle(rx.as_mut(), 0.5);
    assert!(sink.bursts().is_empty());
}

/// Repeat with a pause of 1 s: the picture goes again as a new burst 1 s after the end of the last, each with its end mark;
/// the time to the next picture is shown in the pause; `stop` in the pause ends the repeat. With repeat off a picture goes
/// once.
#[test]
fn sstv_tx_repeat() {
    let set = [
        ("repeat", Value::Bool(true)),
        ("pause_s", Value::Duration(1.0)),
    ];
    let (mut rx, sink) = station_with("Robot 36", Fit::Stretch, None, &set);
    send(rx.as_mut(), 4, 4, &[200u8; 48]).unwrap();
    let mut edges = Vec::new();
    let mut was = true;
    let mut shown = None;
    for k in 1..=800 {
        idle(rx.as_mut(), 0.1);
        let f = fields(rx.as_ref());
        if f.tx_sending != was {
            edges.push(k as f64 * 0.1);
            was = f.tx_sending;
        }
        if !f.tx_sending && shown.is_none() {
            shown = f.tx_next_s.map(|s| s.0);
        }
        if edges.len() == 3 {
            break;
        }
    }
    // The end of the first picture, the start of the second, the end of the second.
    assert_eq!(edges.len(), 3, "{edges:?}");
    let pause = edges[1] - edges[0];
    assert!((pause - 1.0).abs() <= 0.15, "pause {pause} s");
    let on = edges[2] - edges[1];
    assert!(
        (on - nominal_ms("Robot 36") / 1000.0).abs() <= 0.15,
        "{on} s"
    );
    assert!(shown.is_some_and(|s| s > 0.8 && s <= 1.0), "{shown:?}");
    idle(rx.as_mut(), 0.3);
    stop(rx.as_mut());
    idle(rx.as_mut(), 3.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    assert!(b.iter().all(|b| b.ended));
    assert!(fields(rx.as_ref()).tx_next_s.is_none());
    // Off: once.
    let (mut rx, sink) = station("Robot 36", Fit::Stretch, None);
    send(rx.as_mut(), 4, 4, &[200u8; 48]).unwrap();
    idle(rx.as_mut(), 42.0);
    assert_eq!(sink.bursts().len(), 1);
}
