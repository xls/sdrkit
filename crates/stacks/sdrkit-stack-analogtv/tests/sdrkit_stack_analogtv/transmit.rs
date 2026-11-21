//! The senders of `atv` and `fpv`: the stack sends, its own receiver reads the picture and the sound back, and the line and
//! field timing, the colour burst, the sound carrier and the FM swing are measured on the samples against the numbers of the
//! receive specification (`analogtv.md` 10.1 to 10.3, 15.1), written here as numbers so that sender and receiver cannot
//! share a wrong table. The transmit stream is reached as the engine reaches it: the stack's graph with the host streams in
//! its context.

use crate::common::{atv, bars_error, detail, luma_difference, luma_error, pictures, tone, values_of, AtvPicture};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Choice, CommandError, SettingSet, Complex32, InputInfo, Output, Owner, Picture, Receiver, StackConfig, StackDescriptor,
    Value,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_analogtv::card::{Card, Pattern, Planes};
use sdrkit_stack_analogtv::settings::FpvSetting;
use sdrkit_stack_analogtv::transmit::fit_frame;
use sdrkit_stack_analogtv::typed::{FpvFields, TvPixels, TvTxFields};
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, Mutex};

/// The channel centre of the `atv` sender.
const DIAL: f64 = 543e6;
/// The channel of the `fpv` sender: R5.
const FPV_DIAL: f64 = 5806e6;
/// The rate of the `atv` sessions.
const RATE: f64 = 10e6;
/// The rate of the `fpv` sessions.
const FPV_RATE: f64 = 20e6;
/// The rate of the host's audio.
const MIC_RATE: f64 = 48_000.0;
/// The host's tone: 1 kHz at half of full scale.
const TONE: f64 = 0.5;

/// The transmit stream and the host's microphone.
#[derive(Default)]
struct Sink {
    /// Each write: its samples, its end mark.
    writes: Mutex<Vec<(Vec<Complex32>, bool)>>,
    /// The microphone: samples given, samples the host has made so far.
    mic: Mutex<(u64, u64)>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (HostPort::Transmit, HostData::Iq { samples, end, .. }) = (port, data) {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
        }
    }
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        if port != HostPort::Microphone {
            return 0;
        }
        let mut m = self.mic.lock().unwrap();
        let k = out.len().min((m.1 - m.0) as usize);
        for (i, o) in out[..k].iter_mut().enumerate() {
            let n = m.0 + i as u64;
            *o = (TONE * (TAU * 1000.0 * n as f64 / MIC_RATE).sin()) as f32;
        }
        m.0 += k as u64;
        k
    }
}

impl Sink {
    /// The samples written and whether the end mark came.
    fn samples(&self) -> (Vec<Complex32>, bool) {
        let w = self.writes.lock().unwrap();
        let x = w.iter().flat_map(|(s, _)| s.iter().copied()).collect();
        (x, w.last().is_some_and(|(_, e)| *e))
    }
    /// The samples written so far.
    fn written(&self) -> usize {
        self.writes.lock().unwrap().iter().map(|(s, _)| s.len()).sum()
    }
}

/// A station: the stack `desc` with `settings`, transmit on, its transmit stream and microphone.
struct Station {
    rx: Box<dyn Receiver>,
    sink: Arc<Sink>,
    rate: f64,
}

impl Station {
    fn new(desc: &StackDescriptor, rate: f64, dial: f64, settings: &[(&str, &str)]) -> Station {
        let v = values_of(&(desc.settings)(), settings);
        let mut rx = (desc.make_receiver)(&StackConfig::new(v)).expect("the stack builds");
        rx.configure(&InputInfo::new(rate, dial, dial).recording())
            .expect("configure");
        let sink = Arc::new(Sink::default());
        let g = rx
            .as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .expect("the stack is a graph");
        let mut ctx: RunContext = g.context().clone();
        ctx.streams = Some(sink.clone());
        g.set_context(ctx);
        assert!(rx.transmits());
        rx.set_transmit(true);
        Station { rx, sink, rate }
    }

    fn command(&mut self, key: &str) -> Result<(), CommandError> {
        let id = Owner::commands(self.rx.as_ref()).id(key).unwrap();
        let a = Owner::commands(self.rx.as_ref()).def(id).unwrap().new_args();
        owner::invoke(self.rx.as_mut(), id, &a).map(|_| ())
    }

    /// Runs `frame` with an RGB picture.
    fn frame(&mut self, w: u32, h: u32, rgb: &[u8]) -> Result<(), CommandError> {
        let id = Owner::commands(self.rx.as_ref()).id("frame").unwrap();
        let mut a = Owner::commands(self.rx.as_ref()).def(id).unwrap().new_args();
        for (k, v) in [
            ("pixels", Value::Bytes(Arc::from(rgb))),
            ("width", Value::I64(i64::from(w))),
            ("height", Value::I64(i64::from(h))),
            ("pixel_format", Value::Choice(TvPixels::Rgb8.index())),
        ] {
            let i = a.decl().id(k).unwrap();
            a.set_value(i, &v).unwrap();
        }
        owner::invoke(self.rx.as_mut(), id, &a).map(|_| ())
    }

    /// Feeds `seconds` of the session in pieces of `piece` samples; the host makes its audio as the time passes.
    fn run(&mut self, seconds: f64, piece: usize) {
        let zeros = vec![Complex32::default(); piece];
        let mut out = Output::default();
        let n = (seconds * self.rate / piece as f64).round() as usize;
        for _ in 0..n {
            {
                let mut m = self.sink.mic.lock().unwrap();
                m.1 += (piece as f64 / self.rate * MIC_RATE).round() as u64;
            }
            out.clear();
            self.rx.feed(&zeros, &mut out);
        }
    }

    fn fields(&self) -> TvTxFields<'_> {
        TvTxFields::read(Owner::results(self.rx.as_ref()).fields()).expect("the transmit fields")
    }
}

/// The test card of the generator at `w` x `h` as RGB bytes and as the planes the sender makes of them.
fn card_frame(w: usize, h: usize, label: &str, raster: &str) -> (Vec<u8>, Planes) {
    let card = Card::new(w, h, Pattern::Card, "HOST PICTURE", label, raster);
    let mut rgb = Vec::with_capacity(w * h * 3);
    for r in 0..h {
        for c in 0..w {
            for x in card.planes.rgb(c, r) {
                rgb.push((x.clamp(0.0, 1.0) * 255.0).round() as u8);
            }
        }
    }
    let planes = fit_frame(&rgb, w as u32, h as u32, TvPixels::Rgb8, w, h).unwrap();
    (rgb, planes)
}

/// A picture of one colour.
fn solid(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
    rgb.repeat(w * h)
}

/// The mean R, G, B of the middle of a picture (BGRA bytes).
fn middle(p: &Picture) -> [f64; 3] {
    let l = p.layout.as_ref().unwrap();
    let (w, h) = (l.width as usize, l.height as usize);
    let mut s = [0.0; 3];
    let mut n = 0.0;
    for r in h * 2 / 5..h * 3 / 5 {
        for c in w * 2 / 5..w * 3 / 5 {
            let i = (r * w + c) * 4;
            s[0] += f64::from(p.data[i + 2]);
            s[1] += f64::from(p.data[i + 1]);
            s[2] += f64::from(p.data[i]);
            n += 1.0;
        }
    }
    s.map(|x| x / n)
}

/// What a receiver made of a transmission: the pictures with the time they came (seconds of signal fed), the audio, and the
/// last results.
struct Heard {
    pictures: Vec<(f64, AtvPicture)>,
    audio: Vec<f32>,
    detail: crate::common::Detail,
}

/// Feeds `x` to the `atv` receiver at `rate` in pieces of 5 ms.
fn hear_atv(x: &[Complex32], rate: f64, settings: &[(&str, &str)]) -> Heard {
    let mut rx = atv(rate, settings);
    let piece = (0.005 * rate) as usize;
    let mut out = Output::default();
    let mut h = Heard {
        pictures: Vec::new(),
        audio: Vec::new(),
        detail: Default::default(),
    };
    let mut done = 0;
    for c in x.chunks(piece) {
        out.clear();
        rx.feed(c, &mut out);
        done += c.len();
        h.audio.extend_from_slice(out.audio());
        let colour = Owner::results(&rx)
            .fields()
            .at(&sdrkit_stack_analogtv::typed::AtvFields::COLOUR)
            .unwrap_or(false);
        for p in pictures(&out) {
            h.pictures
                .push((done as f64 / rate, AtvPicture { picture: p, colour }));
        }
    }
    h.detail = detail(Owner::results(&rx), done as f64 / rate);
    h
}

/// The amplitude of the 1 kHz tone in the left channel of interleaved stereo audio, over the second quarter of it.
fn tone_left(audio: &[f32]) -> f64 {
    let left: Vec<f32> = audio.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
    let n = left.len();
    tone(&left[n / 4..n / 2], 1000.0)
}

/// One loop of `atv`: the stack sends the card of the generator as a picture of the host with the host's tone, its own
/// receiver reads it. After 1.4 s a blue picture is given; returns the reading, the planes sent, and the delay from that
/// command to the first received picture that is blue.
fn atv_loop(settings: &[(&str, &str)], w: usize, h: usize, label: &str, raster: &str) -> (Heard, Planes, f64) {
    let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, settings);
    let (rgb, planes) = card_frame(w, h, label, raster);
    st.frame(w as u32, h as u32, &rgb).unwrap();
    st.command("start").unwrap();
    assert!(st.fields().tx_sending);
    st.run(1.4, 50_000);
    let at = st.sink.written() as f64 / RATE;
    st.frame(w as u32, h as u32, &solid(w, h, [0, 0, 191])).unwrap();
    st.run(0.6, 50_000);
    st.command("stop").unwrap();
    st.run(0.1, 50_000);
    assert!(!st.fields().tx_sending);
    let (x, ended) = st.sink.samples();
    assert!(ended, "the end mark");
    let clipped = x.iter().filter(|z| z.norm() > 0.8999).count();
    assert!(clipped * 100_000 < x.len(), "{clipped} samples at the bound");
    let heard = hear_atv(&x, RATE, &[]);
    // On a mono system the blue picture is dark grey.
    let mono = heard.detail.system.ends_with("mono");
    let blue = heard
        .pictures
        .iter()
        .find(|(t, p)| {
            let m = middle(&p.picture);
            *t > at && if mono { m.iter().all(|&c| c < 40.0) } else { m[2] > 150.0 && m[0] < 40.0 }
        })
        .map_or(f64::INFINITY, |(t, _)| t - at);
    (heard, planes, blue)
}

/// The last picture before `t` seconds.
fn before(h: &Heard, t: f64) -> &AtvPicture {
    &h.pictures.iter().rev().find(|(at, _)| *at < t).expect("a picture").1
}

/// PAL B/G: the system, colour, the card's bars and luminance, the host's tone at its level, and a picture given by command
/// on the air within two frames.
#[test]
fn atv_tx_pal_bg_loop() {
    let (h, planes, delay) = atv_loop(&[], 768, 576, "PAL B/G", "625/50");
    let d = &h.detail;
    assert_eq!(d.system, "PAL B/G 625/50");
    assert_eq!(d.colour_system, "PAL");
    let p = before(&h, 1.4);
    assert!(p.colour);
    let (bars, luma) = (bars_error(p), luma_difference(p, &planes));
    let t = tone_left(&h.audio);
    eprintln!("PAL B/G: bars error {bars:.3}, luma difference {luma:.2}, tone {t:.3}, new picture after {:.0} ms", delay * 1e3);
    assert!(bars < 0.09, "bars error {bars}");
    assert!(luma < 4.5, "luma difference {luma}");
    assert!((t - TONE).abs() < 0.06, "tone {t}");
    // Two frames of 40 ms, and the 5 ms pieces of the receiver.
    assert!(delay < 0.085, "the new picture after {delay} s");
}

/// A loop case: name, settings, picture size, label and raster of the card, the system the receiver names, the limits of
/// the bars error and of the luma difference.
type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], usize, usize, &'a str, &'a str, &'a str, f64, f64);

/// The other system families: NTSC M, SECAM L (positive, AM sound) and D/K, PAL I, and the 405 lines of A.
#[test]
fn atv_tx_ntsc_secam_and_others_loop() {
    let cases: [Case; 5] = [
            ("NTSC M", &[("tx_system", "m")], 640, 480, "NTSC M", "525/60", "NTSC M 525/59.94", 0.10, 6.0),
            ("SECAM L", &[("tx_system", "l")], 768, 576, "SECAM L", "625/50", "SECAM L 625/50", 0.12, 6.0),
            (
                "SECAM D/K",
                &[("tx_system", "dk"), ("tx_colour", "secam")],
                768,
                576,
                "SECAM D/K",
                "625/50",
                "SECAM D/K 625/50",
                0.12,
                6.0,
            ),
            ("PAL I", &[("tx_system", "i")], 768, 576, "PAL I", "625/50", "PAL I 625/50", 0.10, 4.5),
            ("A", &[("tx_system", "a")], 504, 376, "MONO A", "405/50", "A 405/50 mono", 0.06, 8.0),
        ];
    std::thread::scope(|s| {
        let handles: Vec<_> = cases
            .iter()
            .map(|c| {
                s.spawn(move || {
                    let (name, settings, w, h, label, raster, system, bars_max, luma_max) = *c;
                    let (heard, planes, delay) = atv_loop(settings, w, h, label, raster);
                    let p = before(&heard, 1.4);
                    // A mono picture: the luminance of the bars in place of their colour.
                    let bars = if name == "A" { luma_error(p) } else { bars_error(p) };
                    let luma = luma_difference(p, &planes);
                    let t = tone_left(&heard.audio);
                    eprintln!(
                        "{name}: bars error {bars:.3}, luma difference {luma:.2}, tone {t:.3}, new picture after {:.0} ms",
                        delay * 1e3
                    );
                    assert_eq!(heard.detail.system, system, "{name}");
                    assert!(bars < bars_max, "{name}: bars error {bars}");
                    assert!(luma < luma_max, "{name}: luma difference {luma}");
                    assert!((t - TONE).abs() < 0.08, "{name}: tone {t}");
                    assert!(delay < 0.0901, "{name}: the new picture after {delay} s");
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
}

/// The vision signal of `x`: the carrier at `vision_hz` from the centre mixed to 0 Hz and turned so that the sync tips are
/// real, and its real part (the amplitude of the carrier).
fn vision(x: &[Complex32], rate: f64, vision_hz: f64) -> (Vec<Complex32>, Vec<f64>) {
    let y: Vec<Complex32> = x
        .iter()
        .enumerate()
        .map(|(n, z)| {
            let (s, c) = (-TAU * vision_hz * n as f64 / rate).sin_cos();
            z * Complex32::new(c as f32, s as f32)
        })
        .collect();
    let peak = y.iter().map(|z| z.norm()).fold(0.0, f32::max);
    let tips: Complex32 = y.iter().filter(|z| z.norm() > 0.98 * peak).sum();
    let turn = Complex32::from_polar(1.0, -tips.arg());
    let y: Vec<Complex32> = y.iter().map(|z| z * turn).collect();
    let v = y.iter().map(|z| f64::from(z.re)).collect();
    (y, v)
}

/// The places (fractional samples) where `v` crosses `level` going up and going down.
fn crossings(v: &[f64], level: f64) -> (Vec<f64>, Vec<f64>) {
    let (mut up, mut down) = (Vec::new(), Vec::new());
    for n in 1..v.len() {
        let (a, b) = (v[n - 1] - level, v[n] - level);
        if (a < 0.0) != (b < 0.0) {
            let at = n as f64 - 1.0 + a / (a - b);
            if b >= 0.0 {
                up.push(at);
            } else {
                down.push(at);
            }
        }
    }
    (up, down)
}

/// The timing of one raster, from `analogtv.md` 10.2.
struct Timing {
    vision_hz: f64,
    line_us: f64,
    sync_us: f64,
    eq_us: f64,
    broad_us: f64,
    pulses: usize,
    active_start_us: f64,
    active_us: f64,
    lines: f64,
}

/// A sync pulse: its leading edge (µs) and its width (µs).
type Pulse = (f64, f64);

/// What the timing check found: the line syncs, and the blanking and white levels of the vision signal.
struct Found {
    line: Vec<Pulse>,
    blank: f64,
    white: f64,
}

/// Measures the line and field timing of `v`, the vision signal of a white picture without sound (from [`vision`]), and
/// checks it against `t`.
fn check_timing(v: &[f64], t: &Timing, name: &str) -> Found {
    let us = 1e6 / RATE;
    let tip = v.iter().copied().fold(f64::MIN, f64::max);
    let white = {
        let mut s: Vec<f64> = v.to_vec();
        s.sort_by(f64::total_cmp);
        s[s.len() / 4]
    };
    // Blanking: the middle of the levels between white and the sync tip, which are the porches and the blanked lines.
    let mut between: Vec<f64> = v
        .iter()
        .copied()
        .filter(|&x| x > white + 0.4 * (tip - white) && x < white + 0.9 * (tip - white))
        .collect();
    between.sort_by(f64::total_cmp);
    let blank = between[between.len() / 2];
    let (lead, trail) = crossings(v, 0.5 * (tip + blank));
    let pulses: Vec<Pulse> = lead
        .iter()
        .filter_map(|&a| trail.iter().find(|&&b| b > a).map(|&b| (a * us, (b - a) * us)))
        .collect();
    let kind = |w: f64| -> char {
        if (w - t.sync_us).abs() < 0.5 {
            'l'
        } else if (w - t.eq_us).abs() < 0.5 {
            'e'
        } else if (w - t.broad_us).abs() < 0.5 {
            'b'
        } else {
            '?'
        }
    };
    let kinds: Vec<char> = pulses.iter().map(|p| kind(p.1)).collect();
    let odd: Vec<&Pulse> = pulses.iter().zip(&kinds).filter(|(_, &k)| k == '?').map(|(p, _)| p).take(5).collect();
    assert!(odd.is_empty(), "{name}: pulses of none of the three widths: {odd:?}");
    let of = |k: char| -> Vec<Pulse> { pulses.iter().zip(&kinds).filter(|(_, &c)| c == k).map(|(p, _)| *p).collect() };
    let (line, eq, broad) = (of('l'), of('e'), of('b'));
    let worst = |ps: &[Pulse], want: f64| ps.iter().map(|p| (p.1 - want).abs()).fold(0.0, f64::max);
    eprintln!(
        "{name}: {} line syncs (width {:.4} µs off at worst), {} equalising ({:.4}), {} broad ({:.4})",
        line.len(),
        worst(&line, t.sync_us),
        eq.len(),
        worst(&eq, t.eq_us),
        broad.len(),
        worst(&broad, t.broad_us)
    );
    assert!(worst(&line, t.sync_us) < 0.05, "{name}: line sync width");
    assert!(worst(&eq, t.eq_us) < 0.05, "{name}: equalising pulse width");
    assert!(worst(&broad, t.broad_us) < 0.05, "{name}: broad pulse width");
    // Each field: a run of equalising pulses, one of broad pulses, one of equalising pulses, each of the count of the
    // raster (the runs cut by the ends of the samples left out).
    let mut runs: Vec<(char, usize)> = Vec::new();
    for &k in &kinds {
        match runs.last_mut() {
            Some((c, n)) if *c == k => *n += 1,
            _ => runs.push((k, 1)),
        }
    }
    let inner = &runs[1..runs.len() - 1];
    let fields = inner.iter().filter(|r| r.0 == 'b').count();
    assert!(fields >= 8, "{name}: {fields} fields");
    for w in inner.windows(3).filter(|w| w[1].0 == 'b') {
        assert_eq!((w[0], w[1].1, w[2]), (('e', t.pulses), t.pulses, ('e', t.pulses)), "{name}: field sync");
    }
    // The line period between line syncs one line apart.
    let periods: Vec<f64> = line
        .windows(2)
        .map(|w| w[1].0 - w[0].0)
        .filter(|d| (d - t.line_us).abs() < 1.0)
        .collect();
    let mean = periods.iter().sum::<f64>() / periods.len() as f64;
    let spread = periods.iter().map(|p| (p - t.line_us).abs()).fold(0.0, f64::max);
    eprintln!("{name}: line period {mean:.5} µs ({spread:.4} off at worst) over {} lines", periods.len());
    assert!((mean - t.line_us).abs() < 0.0005, "{name}: line period {mean}");
    // Single periods: the crossings are interpolated linearly between samples 0.1 µs apart.
    assert!(spread < 0.01, "{name}: line period spread {spread}");
    // The first broad pulses of successive fields: half a frame apart.
    let starts: Vec<f64> = broad.windows(2).filter(|w| w[1].0 - w[0].0 > 2.0 * t.line_us).map(|w| w[1].0).collect();
    let half = t.lines * t.line_us / 2.0;
    for w in starts.windows(2) {
        assert!(((w[1] - w[0]) - half).abs() < 0.01, "{name}: fields {} µs apart", w[1] - w[0]);
    }
    // The active picture of the white lines: from the line sync to the half-way fall to white, and to the rise back.
    let (rises, falls) = crossings(v, 0.5 * (blank + white));
    let mut active = Vec::new();
    for p in &line {
        let a = p.0 / us;
        let Some(&f) = falls.iter().find(|&&f| f > a) else { continue };
        let Some(&r) = rises.iter().find(|&&r| r > f) else { continue };
        if (f - a) * us < t.active_start_us + 2.0 && (r - a) * us < t.line_us {
            active.push(((f - a) * us, (r - f) * us));
        }
    }
    let n = active.len() as f64;
    let s0 = active.iter().map(|a| a.0).sum::<f64>() / n;
    let l0 = active.iter().map(|a| a.1).sum::<f64>() / n;
    eprintln!(
        "{name}: active picture from {s0:.3} µs, {l0:.3} µs long, front porch {:.3} µs, over {} lines",
        t.line_us - s0 - l0,
        active.len()
    );
    assert!(active.len() > 400, "{name}: white lines");
    assert!((s0 - t.active_start_us).abs() < 0.1, "{name}: active start {s0}");
    assert!((l0 - t.active_us).abs() < 0.15, "{name}: active length {l0}");
    Found { line, blank, white }
}

/// A transmission of a white picture without sound, at `settings`.
fn white_transmission(settings: &[(&str, &str)], w: usize, h: usize) -> Vec<Complex32> {
    let mut all = vec![("tx_sound", "false")];
    all.extend_from_slice(settings);
    let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, &all);
    st.frame(w as u32, h as u32, &solid(w, h, [255, 255, 255])).unwrap();
    st.command("start").unwrap();
    st.run(0.3, 50_000);
    st.sink.samples().0
}

/// The 625 lines of PAL B/G: the line and field timing, and the colour burst: its start, its cycles, its level, its
/// frequency and the alternation of its phase.
#[test]
fn atv_tx_625_timing_and_pal_burst() {
    let x = white_transmission(&[], 768, 576);
    let t = Timing {
        vision_hz: -2.75e6,
        line_us: 64.0,
        sync_us: 4.7,
        eq_us: 2.35,
        broad_us: 27.3,
        pulses: 5,
        active_start_us: 10.5,
        active_us: 52.0,
        lines: 625.0,
    };
    let (y, v) = vision(&x, RATE, t.vision_hz);
    // Whole fields after the rise.
    let skip = (0.05 * RATE) as usize;
    let (y, v) = (&y[skip..skip + (0.2 * RATE) as usize], &v[skip..skip + (0.2 * RATE) as usize]);
    let found = check_timing(v, &t, "625 lines");
    // The burst: 10 cycles of 4 433 618.75 Hz from 5.6 µs, 3/14 of the span from blanking to white, at +135° and -135° from
    // line to line. The subcarrier's upper sideband alone passes the vestigial-sideband filter: it is half the burst.
    let fsc = 4_433_618.75;
    let taps = sdrkit_algo_dsp::fir::lowpass(RATE, 0.6e6, 2.0e6, 60.0).unwrap();
    let half_taps = (taps.len() - 1) / 2;
    let d: Vec<Complex32> = y
        .iter()
        .enumerate()
        .map(|(n, z)| {
            let (s, c) = (-TAU * fsc * (n + skip) as f64 / RATE).sin_cos();
            z * Complex32::new(c as f32, s as f32)
        })
        .collect();
    let us = RATE / 1e6;
    let lp = |n: usize| -> Complex32 { taps.iter().enumerate().map(|(k, &t)| d[n + half_taps - k] * t).sum() };
    let span = found.blank - found.white;
    let mut bursts = Vec::new();
    for p in found.line.iter().skip(2) {
        let edge = p.0 * us;
        let a = edge.floor() as usize;
        if a + (12.0 * us) as usize + half_taps >= d.len() {
            break;
        }
        let env: Vec<f64> = (0..(10.0 * us) as usize).map(|i| f64::from(lp(a + i).norm())).collect();
        let (m0, m1) = ((6.2 * us) as usize, (7.2 * us) as usize);
        let top = env[m0..m1].iter().sum::<f64>() / (m1 - m0) as f64;
        if top < 0.02 * span {
            continue;
        }
        let (up, down) = crossings(&env, top / 2.0);
        let frac = edge - edge.floor();
        let mid: Complex32 = (m0..m1).map(|i| lp(a + i)).sum();
        bursts.push(((up[0] - frac) / us, (down[0] - up[0]) / us, 2.0 * top / span, p.0, f64::from(mid.arg())));
    }
    let n = bursts.len() as f64;
    let (s, l, a) = (
        bursts.iter().map(|b| b.0).sum::<f64>() / n,
        bursts.iter().map(|b| b.1).sum::<f64>() / n,
        bursts.iter().map(|b| b.2).sum::<f64>() / n,
    );
    eprintln!(
        "PAL burst: from {s:.3} µs, {:.2} cycles, {a:.4} of the span from blanking to white, over {} lines",
        l * fsc / 1e6,
        bursts.len()
    );
    assert!(bursts.len() > 500);
    assert!((s - 5.6).abs() < 0.1, "burst start {s}");
    assert!((l * fsc / 1e6 - 10.0).abs() < 0.5, "burst cycles");
    assert!((a - 3.0 / 14.0).abs() < 0.02, "burst level {a}");
    // Lines one after the other: the phase turns by 90° one way, then back.
    let wrap = |x: f64| (x + 3.0 * PI).rem_euclid(TAU) - PI;
    let mut turns = Vec::new();
    for w in bursts.windows(2) {
        if (w[1].3 - w[0].3 - t.line_us).abs() < 0.5 {
            turns.push(wrap(w[1].4 - w[0].4).to_degrees());
        }
    }
    let worst = turns.iter().map(|d| (d.abs() - 90.0).abs()).fold(0.0, f64::max);
    eprintln!("PAL burst: the phase turns by 90° to {worst:.2}° from line to line, over {} lines", turns.len());
    assert!(worst < 3.0, "PAL alternation off by {worst}°");
    // The frequency: in the longest run of lines one after the other, the phase of every second burst stays put against the
    // subcarrier of the specification.
    let mut runs: Vec<Vec<(f64, f64)>> = vec![Vec::new()];
    for (i, b) in bursts.iter().enumerate() {
        if i > 0 && (b.3 - bursts[i - 1].3 - t.line_us).abs() > 0.5 {
            runs.push(Vec::new());
        }
        runs.last_mut().unwrap().push((b.3, b.4));
    }
    let run = runs.iter().max_by_key(|r| r.len()).unwrap();
    for w in run.windows(3) {
        let (t1, t2) = (wrap(w[1].1 - w[0].1), wrap(w[2].1 - w[1].1));
        assert!(t1.signum() != t2.signum(), "the alternation");
    }
    let same: Vec<(f64, f64)> = run.iter().step_by(2).copied().collect();
    let mut unwrapped = vec![same[0].1];
    for w in same.windows(2) {
        unwrapped.push(unwrapped.last().unwrap() + wrap(w[1].1 - w[0].1));
    }
    let n = same.len() as f64;
    let (mt, mp) = (same.iter().map(|p| p.0).sum::<f64>() / n, unwrapped.iter().sum::<f64>() / n);
    let slope = same.iter().zip(&unwrapped).map(|(p, u)| (p.0 - mt) * (u - mp)).sum::<f64>()
        / same.iter().map(|p| (p.0 - mt).powi(2)).sum::<f64>();
    let df = slope / TAU * 1e6;
    eprintln!("PAL burst: {df:.3} Hz from 4 433 618.75 Hz over {} lines", run.len());
    assert!(run.len() > 200);
    assert!(df.abs() < 5.0, "burst frequency off by {df} Hz");
}

/// The 525 lines of NTSC M: the line and field timing.
#[test]
fn atv_tx_525_timing() {
    let x = white_transmission(&[("tx_system", "m")], 640, 480);
    let (_, v) = vision(&x, RATE, -1.75e6);
    let skip = (0.05 * RATE) as usize;
    check_timing(
        &v[skip..skip + (0.2 * RATE) as usize],
        &Timing {
            vision_hz: -1.75e6,
            line_us: 1e6 / (4.5e6 / 286.0),
            sync_us: 4.7,
            eq_us: 2.3,
            broad_us: 27.1,
            pulses: 6,
            active_start_us: 9.4,
            active_us: 52.656,
            lines: 525.0,
        },
        "525 lines",
    );
}

/// The FM sound carrier of B/G: 5.5 MHz above the vision carrier, 13 dB below it, the host's tone at 0.5 of full scale
/// deviating it by 0.5 x 50 kHz x the pre-emphasis gain of 50 µs at 1 kHz; the peak of the whole signal at most 0.9.
#[test]
fn atv_tx_sound_carrier_and_level() {
    let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, &[]);
    st.command("start").unwrap();
    st.run(0.5, 50_000);
    let (x, _) = st.sink.samples();
    let peak = x.iter().map(|z| z.norm()).fold(0.0, f32::max);
    // The sync tip: the vision signal through a low-pass of 1 MHz, which takes out the sound carrier.
    let (_, v) = vision(&x, RATE, -2.75e6);
    let vlp = sdrkit_algo_dsp::fir::lowpass(RATE, 1e6, 2e6, 60.0).unwrap();
    let tip = (0.1 * RATE) as usize..(0.3 * RATE) as usize;
    let tip = tip
        .step_by(7)
        .map(|n| vlp.iter().enumerate().map(|(k, &t)| f64::from(t) * v[n - k]).sum::<f64>())
        .fold(f64::MIN, f64::max);
    let clipped = x.iter().filter(|z| z.norm() > 0.8999).count() as f64 / x.len() as f64;
    // The sound carrier at +2.75 MHz from the centre, through a low-pass of 150 kHz, at 1 Msps.
    let taps = sdrkit_algo_dsp::fir::lowpass(RATE, 150e3, 600e3, 70.0).unwrap();
    let a = (0.15 * RATE) as usize;
    let s: Vec<Complex32> = (a..x.len() - taps.len())
        .step_by(10)
        .map(|n| {
            taps.iter()
                .enumerate()
                .map(|(k, &t)| {
                    let m = n + k;
                    let (s, c) = (-TAU * 2.75e6 * m as f64 / RATE).sin_cos();
                    x[m] * Complex32::new(c as f32, s as f32) * t
                })
                .sum()
        })
        .collect();
    let fs = RATE / 10.0;
    let f: Vec<f32> = s
        .windows(2)
        .map(|w| (f64::from((w[1] * w[0].conj()).arg()) * fs / TAU) as f32)
        .collect();
    let mean = f.iter().map(|&x| f64::from(x)).sum::<f64>() / f.len() as f64;
    let dev = sdrkit_dev_test::measure::tone_amplitude(&f, fs, 1000.0);
    let level = s.iter().map(|z| f64::from(z.norm())).sum::<f64>() / s.len() as f64;
    let db = 20.0 * (level / tip).log10();
    let want = TONE * 50e3 * (1.0 + (TAU * 1000.0 * 50e-6).powi(2)).sqrt();
    eprintln!(
        "sound carrier {mean:.1} Hz from its place, deviation {dev:.0} Hz (want {want:.0}), {db:.2} dB to vision; peak {peak:.4}, sync tip {tip:.4}, {:.4} % of the samples at the bound",
        clipped * 100.0
    );
    assert!(mean.abs() < 150.0, "carrier offset {mean}");
    assert!((dev / want - 1.0).abs() < 0.06, "deviation {dev}");
    assert!((db + 13.0).abs() < 0.7, "sound level {db} dB");
    assert!(peak <= 0.9 + 1e-6, "peak {peak}");
    assert!(clipped < 1e-5, "{clipped} of the samples at the bound");
}

/// `stop` ends at the end of a field with a fade and the end mark; the results count the fields and the pictures; the
/// samples do not depend on the size of the passes; with transmit off `start` is refused and nothing is sent.
#[test]
fn atv_tx_start_stop_cuts_and_switch() {
    let sent = |piece: usize| {
        let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, &[("tx_sound", "false")]);
        st.command("start").unwrap();
        st.run(0.25, piece);
        st.sink.samples().0
    };
    let (a, b) = (sent(4093), sent(32_768));
    let n = a.len().min(b.len());
    assert!(n > (0.2 * RATE) as usize);
    assert_eq!(a[..n], b[..n], "the same samples for any cut");

    let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, &[("tx_sound", "false")]);
    let (rgb, _) = card_frame(768, 576, "PAL B/G", "625/50");
    st.frame(768, 576, &rgb).unwrap();
    st.command("start").unwrap();
    st.run(0.13, 50_000);
    assert_eq!(st.fields().tx_system, Some("PAL B/G 625/50"));
    st.command("stop").unwrap();
    st.run(0.1, 50_000);
    let (x, ended) = st.sink.samples();
    assert!(ended);
    let field = (RATE / 50.0) as usize;
    // The end mark sits on the last sample, or on one silent sample after it.
    assert!(x.len() % field <= 1, "ends at the end of a field: {} samples", x.len());
    assert!(x[0].norm() < 0.01 && x[x.len() - 1].norm() < 0.01, "the rise and the fall");
    let f = st.fields();
    assert!(!f.tx_sending);
    assert_eq!(f.tx_fields as usize, x.len() / field);
    assert_eq!(f.tx_frames, 1);
    assert!(st.frame(768, 576, &rgb[1..]).is_err(), "a picture of the wrong length");

    let mut st = Station::new(&sdrkit_stack_analogtv::ATV, RATE, DIAL, &[]);
    st.rx.set_transmit(false);
    assert!(matches!(st.command("start"), Err(CommandError::TransmitOff)));
    st.run(0.05, 50_000);
    assert_eq!(st.sink.written(), 0);
}

/// `fpv`: the stack sends the card as a picture of the host on R5 with the host's tone on the 6.5 MHz subcarrier; its own
/// receiver reads the picture, the colour and the tone (25 kHz deviation at full scale, played at 50 kHz full scale); a
/// picture given by command is on the air within two frames. Without sound, the swing of a white picture is measured:
/// the sync tip 8 MHz below the carrier, white 8 MHz above.
#[test]
fn fpv_tx_loop_and_swing() {
    let mut st = Station::new(&sdrkit_stack_analogtv::FPV, FPV_RATE, FPV_DIAL, &[("channels", "R5=5806000000")]);
    let (rgb, planes) = card_frame(768, 576, "PAL B/G", "625/50");
    st.frame(768, 576, &rgb).unwrap();
    st.command("start").unwrap();
    st.run(1.4, 100_000);
    let at = st.sink.written() as f64 / FPV_RATE;
    st.frame(768, 576, &solid(768, 576, [0, 0, 191])).unwrap();
    st.run(0.5, 100_000);
    st.command("stop").unwrap();
    st.run(0.1, 100_000);
    let (x, ended) = st.sink.samples();
    assert!(ended);
    let peak = x.iter().map(|z| z.norm()).fold(0.0, f32::max);
    assert!(peak <= 0.9 + 1e-6, "peak {peak}");

    let v = values_of(&FpvSetting::schema(), &[("channels", "R5=5806000000")]);
    let mut rx = sdrkit_stack_analogtv::fpv::receiver(&StackConfig::new(v)).unwrap();
    rx.configure(&InputInfo::new(FPV_RATE, FPV_DIAL, FPV_DIAL).recording()).unwrap();
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut seen: Vec<(f64, Picture)> = Vec::new();
    let mut done = 0;
    for c in x.chunks((0.005 * FPV_RATE) as usize) {
        out.clear();
        rx.feed(c, &mut out);
        done += c.len();
        audio.extend_from_slice(out.audio());
        for p in pictures(&out) {
            seen.push((done as f64 / FPV_RATE, p));
        }
    }
    let f = Owner::results(&rx).fields();
    assert_eq!(f.at(&FpvFields::LINES), Some(625));
    assert_eq!(f.at(&FpvFields::SYSTEM), Some("PAL B/G 625/50"));
    assert_eq!(f.at(&FpvFields::COLOUR_SYSTEM).map(|c| c.label().to_string()).as_deref(), Some("PAL"));
    assert!(f.at(&FpvFields::SOUND_PRESENT).unwrap_or(false));
    let p = &seen.iter().rev().find(|(t, _)| *t < at).expect("a picture").1;
    let p = AtvPicture { picture: p.clone(), colour: true };
    let (bars, luma) = (bars_error(&p), luma_difference(&p, &planes));
    let n = audio.len();
    let t = tone(&audio[n * 9 / 20..n * 7 / 10], 1000.0);
    let delay = seen
        .iter()
        .find(|(tt, p)| {
            let m = middle(p);
            *tt > at && m[2] > 150.0 && m[0] < 40.0
        })
        .map_or(f64::INFINITY, |(tt, _)| tt - at);
    eprintln!("FPV: bars error {bars:.3}, luma difference {luma:.2}, tone {t:.3}, new picture after {:.0} ms", delay * 1e3);
    assert!(bars < 0.12, "bars error {bars}");
    assert!(luma < 8.0, "luma difference {luma}");
    assert!((t - TONE * 25.0 / 50.0).abs() < 0.04, "tone {t}");
    assert!(delay < 0.085, "the new picture after {delay} s");

    // The swing.
    let mut st = Station::new(&sdrkit_stack_analogtv::FPV, FPV_RATE, FPV_DIAL, &[("tx_sound", "false")]);
    st.frame(768, 576, &solid(768, 576, [255, 255, 255])).unwrap();
    st.command("start").unwrap();
    st.run(0.1, 100_000);
    let (x, _) = st.sink.samples();
    let fi: Vec<f64> = x[(0.05 * FPV_RATE) as usize..]
        .windows(2)
        .map(|w| f64::from((w[1] * w[0].conj()).arg()) * FPV_RATE / TAU)
        .collect();
    // Means over 1 µs.
    let m: Vec<f64> = fi.chunks(20).map(|c| c.iter().sum::<f64>() / c.len() as f64).collect();
    let mut sorted = m.clone();
    sorted.sort_by(f64::total_cmp);
    let (low, high) = (sorted[sorted.len() / 200], sorted[sorted.len() * 3 / 4]);
    eprintln!("FPV swing: sync tip {:.3} MHz, white {:.3} MHz", low / 1e6, high / 1e6);
    assert!((low + 8e6).abs() < 0.1e6, "sync tip {low}");
    assert!((high - 8e6).abs() < 0.1e6, "white {high}");
}
