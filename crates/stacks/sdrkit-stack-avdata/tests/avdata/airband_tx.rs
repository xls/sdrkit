//! The transmit branch of Airband: a tone and speech-like audio through the stack's own receiver, the side bands and the
//! distortion of the modulation, the depth under full-scale and overdriven audio, the occupied bandwidth for both spacings,
//! the rise and the fall of the carrier, no speech lost at key down, the stated start, the time-out, and nothing on air with
//! the transmitter off. The transmit stream is reached as the engine reaches it: the graph, with the host streams in its
//! context.

use crate::common::{self, rows, text};
use sdrkit_algo_dsp::bandwidth::power_share_edges;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_algo_sim::voice::Voice;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Clock, Complex32, ConfigError, InputInfo, Output, Owner, Receiver, StackConfig, Value,
    Values,
};
use sdrkit_dev_test::measure;
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Component, ComponentInfo, Format, Graph, Io, Port, StackGraph};
use sdrkit_stack_avdata::airband::names::{self, list_to_text, Channel};
use sdrkit_stack_avdata::airband::transmit::{MAX_DEPTH, PEAK, RISE_S};
use sdrkit_stack_avdata::airband::{self, TUNE_OFFSET_HZ};
use sdrkit_stack_avdata::typed::{AirbandTxFields, AIRBAND_CHANNELS};
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The session rate of the sender and of the receiver.
const RATE: f64 = 250e3;
/// The clock of a host that times bursts.
const NOW: f64 = 1_800_000_000.0;
/// The time before push-to-talk (s).
const BEFORE: f64 = 0.2;

/// One write to the transmit stream: its samples, its centre, its end mark and its stated start.
type Write = (Vec<Complex32>, f64, bool, Option<f64>);

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
                centre_hz,
                end,
                start,
                ..
            },
        ) = (port, data)
        {
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), centre_hz, end, start));
        }
    }
    fn transmit_lead(&self) -> Duration {
        self.lead
    }
}

/// One transmission: its samples, its centre, whether its end mark came, and its stated starts (place, UTC).
struct Burst {
    samples: Vec<Complex32>,
    centre: f64,
    ended: bool,
    starts: Vec<(usize, f64)>,
}

impl Sink {
    fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, c, end, start) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push(Burst {
                    samples: Vec::new(),
                    centre: *c,
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

/// The microphone: 48 kHz audio from a buffer, as many samples as the session's samples bring time.
struct TestMic {
    rate: f64,
    owed: f64,
    at: usize,
    audio: Vec<f32>,
}

impl Component for TestMic {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.mic", vec![Port::iq("in")], vec![Port::audio("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![Format::audio(48_000.0, 1)])
    }
    fn process(&mut self, io: &mut Io) {
        self.owed += io.input_len(0) as f64 * 48_000.0 / self.rate;
        let m = self.owed.floor();
        self.owed -= m;
        let out = &mut io.audio_out(0).samples;
        for _ in 0..m as usize {
            out.push(self.audio.get(self.at).copied().unwrap_or(0.0));
            self.at += 1;
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// 48 kHz audio of `seconds`: `f(t)` from [`BEFORE`] on, silence before.
fn audio(seconds: f64, f: impl Fn(f64) -> f32) -> Vec<f32> {
    (0..(seconds * 48e3) as usize)
        .map(|i| {
            let t = i as f64 / 48e3;
            if t >= BEFORE {
                f(t - BEFORE)
            } else {
                0.0
            }
        })
        .collect()
}

/// A tone of `hz` at amplitude `amp`.
fn tone(hz: f64, amp: f64) -> impl Fn(f64) -> f32 {
    move |t| (amp * (TAU * hz * t).sin()) as f32
}

/// The shared speech-like source at RMS `level` for `seconds`, in syllables with pauses or steady.
fn voice(level: f64, seconds: f64, syllables: bool) -> Vec<f32> {
    let v = Voice::new(48e3, level, Rng::new(7)).unwrap();
    let mut v = if syllables { v } else { v.steady() };
    let mut x = vec![0f32; (seconds * 48e3) as usize];
    v.fill(&mut x);
    let mut a = vec![0f32; (BEFORE * 48e3) as usize];
    a.extend(x);
    a
}

/// The channel "118.035" (8.33 kHz) and its frequency.
fn channel() -> (Channel, f64) {
    let (f, is833) = names::parse("118.035").unwrap();
    (
        Channel {
            freq_hz: f,
            is833,
            ..Default::default()
        },
        f,
    )
}

/// A sending stack tuned to `freq` with `settings`, the microphone playing `mic`, the transmitter on, and its stream. With
/// `lead`, a live radio whose host times bursts.
fn station(
    freq: f64,
    settings: &[(&str, &str)],
    mic: Vec<f32>,
    lead: Option<Duration>,
) -> (StackGraph, Arc<Sink>) {
    let m = TestMic {
        rate: 0.0,
        owed: 0.0,
        at: 0,
        audio: mic,
    };
    let cfg = StackConfig::new(common::values(&airband::DESCRIPTOR, settings));
    let mut rx = airband::receiver_with_mic(&cfg, Box::new(m)).expect("the stack builds");
    rx.set_transmit(true);
    let input = InputInfo::new(RATE, freq + TUNE_OFFSET_HZ, freq);
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
        .expect("a graph");
    let mut ctx = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
    assert!(rx.transmits());
    (rx, sink)
}

fn ptt(rx: &mut StackGraph, on: bool) -> Result<(), sdrkit_core::CommandError> {
    let id = Owner::commands(rx).id("ptt").unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    let i = a.decl().id("on").unwrap();
    a.set_value(i, &Value::Bool(on)).unwrap();
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
fn idle(rx: &mut StackGraph, seconds: f64) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

fn fields(rx: &StackGraph) -> AirbandTxFields {
    AirbandTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// One transmission of `seconds` on `freq` with `settings` and `mic`; the transmission, the stack and its reports of the
/// depth while sending (%).
fn talk(
    freq: f64,
    settings: &[(&str, &str)],
    mic: Vec<f32>,
    seconds: f64,
) -> (Burst, StackGraph, Vec<f64>) {
    let (mut rx, sink) = station(freq, settings, mic, None);
    idle(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    let mut depth = Vec::new();
    for _ in 0..(seconds * 10.0).round() as usize {
        idle(&mut rx, 0.1);
        depth.extend(fields(&rx).tx_depth);
    }
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, 0.3);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "one transmission");
    let b = b.remove(0);
    assert!(b.ended, "the transmission has its end mark");
    assert!(
        (b.centre - freq).abs() < 1e-3,
        "the carrier is on the tuned frequency"
    );
    (b, rx, depth)
}

/// The samples of `b` between `from` and `to` seconds after its start.
fn part(b: &Burst, from: f64, to: f64) -> &[Complex32] {
    let a = ((from * RATE) as usize).min(b.samples.len());
    let e = ((to * RATE) as usize).min(b.samples.len());
    &b.samples[a..e]
}

/// The amplitude of the complex tone at `hz` in `x`.
fn line(x: &[Complex32], hz: f64) -> f64 {
    let mut s = (0.0, 0.0);
    for (i, z) in x.iter().enumerate() {
        let p = -TAU * hz * i as f64 / RATE;
        let (re, im) = (f64::from(z.re), f64::from(z.im));
        s.0 += re * p.cos() - im * p.sin();
        s.1 += re * p.sin() + im * p.cos();
    }
    s.0.hypot(s.1) / x.len() as f64
}

/// The carrier `C = PEAK / (1 + m)` for the depth `m`.
fn carrier(m: f64) -> f64 {
    f64::from(PEAK) / (1.0 + m)
}

/// The largest depth on the positive and on the negative side over `x` with carrier `c`: `max|z|/c - 1` and `1 - min re/c`.
fn depths(x: &[Complex32], c: f64) -> (f64, f64) {
    let hi = x.iter().map(|z| f64::from(z.norm())).fold(0.0, f64::max);
    let lo = x.iter().map(|z| f64::from(z.re)).fold(f64::MAX, f64::min);
    (hi / c - 1.0, 1.0 - lo / c)
}

/// The total harmonic distortion of the envelope `|z| / c - 1` of `x` against its fundamental at `hz`.
fn thd(x: &[Complex32], c: f64, hz: f64) -> f64 {
    let e: Vec<f32> = x
        .iter()
        .map(|z| (f64::from(z.norm()) / c - 1.0) as f32)
        .collect();
    let f = measure::tone_amplitude(&e, RATE, hz);
    let h: f64 = (2..=5)
        .map(|k| measure::tone_amplitude(&e, RATE, hz * f64::from(k)).powi(2))
        .sum();
    h.sqrt() / f
}

/// The 99 % occupied bandwidth of `x` (ITU-R SM.328), Hz, and the strongest bin at or beyond `from` Hz against the carrier,
/// dB.
fn occupied(x: &[Complex32], from: f64) -> (f64, f64) {
    let n = 16_384;
    let mut p = vec![0f32; n];
    welch(x, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
    let p: Vec<f64> = p.iter().map(|v| f64::from(*v)).collect();
    let bin = RATE / n as f64;
    let (a, b) = power_share_edges(&p, 0, n - 1, 0.99).unwrap();
    let peak = p.iter().copied().fold(0.0, f64::max);
    let far = p
        .iter()
        .enumerate()
        .filter(|(i, _)| ((*i as f64 - (n / 2) as f64) * bin).abs() >= from)
        .map(|(_, v)| *v)
        .fold(0.0, f64::max);
    ((b - a + 1) as f64 * bin, 10.0 * (far / peak).log10())
}

/// The 48 kHz audio and the channel rows of a receiving stack on `freq` with `list`, fed `b` moved to the tuning offset of
/// the stack, with noise for 0.5 s before and 1.5 s after, at a C/N of `cn_db` in 6.8 kHz.
fn receive(b: &Burst, list: &[Channel], cn_db: f64) -> (Vec<f32>, Vec<common::Row>) {
    let freq = b.centre;
    let lt = list_to_text(list);
    let mut rx = common::receiver(
        &airband::DESCRIPTOR,
        RATE,
        freq + TUNE_OFFSET_HZ,
        freq,
        &[("channels", &lt)],
    );
    let mut x = vec![Complex32::default(); (0.5 * RATE) as usize];
    x.extend_from_slice(&b.samples);
    x.resize(x.len() + (1.5 * RATE) as usize, Complex32::default());
    common::offset(&mut x, RATE, -TUNE_OFFSET_HZ);
    let c = carrier(0.85);
    common::noise(&mut x, c * c, cn_db, 6.8e3, RATE, 11);
    let mut out = Output::default();
    let mut audio = Vec::new();
    for piece in x.chunks((RATE / 10.0) as usize) {
        out.clear();
        rx.feed(piece, &mut out);
        audio.extend(
            out.audio()
                .iter()
                .step_by(out.audio_format().map_or(1, |f| usize::from(f.1.max(1)))),
        );
    }
    (audio, rows(rx.results(), &AIRBAND_CHANNELS))
}

/// The 1 kHz tone of `a` (48 kHz) over `from..to` seconds: its amplitude and the SINAD (tone over the rest, dB).
fn sinad(a: &[f32], from: f64, to: f64) -> (f64, f64) {
    let piece = &a[(from * 48e3) as usize..(to * 48e3) as usize];
    let amp = measure::tone_amplitude(piece, 48e3, 1000.0);
    let p = measure::rms(piece).powi(2);
    let sig = amp * amp / 2.0;
    (amp, 10.0 * (sig / (p - sig).max(1e-12)).log10())
}

#[test]
fn a_tone_comes_back_from_the_receiver_with_its_side_bands_at_the_depth() {
    // A full-scale tone has the RMS that the leveller aims at, so its peaks are at full scale.
    let (ch, f) = channel();
    let (b, _, depth) = talk(f, &[], audio(2.5, tone(1000.0, 1.0)), 2.0);
    let steady = part(&b, 0.5, 1.9);
    let c = line(steady, 0.0);
    let sb = 20.0 * ((line(steady, 1000.0) + line(steady, -1000.0)) / 2.0 / c).log10();
    let d = thd(steady, c, 1000.0);
    eprintln!(
        "carrier {c:.4}, side bands {sb:.2} dB, distortion {:.3} %, depth reports {depth:?}",
        100.0 * d
    );
    assert!(
        (c - carrier(0.85)).abs() < 0.005,
        "the carrier is 0.9 / 1.85"
    );
    assert!(
        (sb + 7.43).abs() < 0.3,
        "each side band is 0.85 / 2 of the carrier: {sb}"
    );
    assert!(d < 0.01, "the modulation holds the tone: {d}");
    assert!(
        depth.iter().skip(2).all(|d| (84.0..=85.0).contains(d)),
        "{depth:?}"
    );

    let (a, chans) = receive(&b, &[ch], 30.0);
    let row = chans
        .iter()
        .find(|r| text(r, "name") == "118.035")
        .expect("the channel row");
    let off = common::num(row, "offset_hz");
    assert!(common::int(row, "count") == 1, "one transmission heard");
    assert!(
        off.abs() < 50.0,
        "the carrier offset is within 50 Hz: {off}"
    );
    let (amp, s) = sinad(&a, 1.0, 2.2);
    eprintln!("received tone {amp:.3}, SINAD {s:.1} dB, offset {off:.1} Hz");
    assert!(amp > 0.05, "the receiver plays the tone: {amp}");
    assert!(s > 25.0, "the received tone is clean: {s}");
}

#[test]
fn speech_like_audio_comes_back_from_the_receiver() {
    let (ch, f) = channel();
    let (b, _, depth) = talk(f, &[], voice(0.3, 3.0, false), 2.0);
    let peak = depth.iter().copied().fold(0.0, f64::max);
    assert!(
        peak <= 85.0 && peak > 80.0,
        "the peaks reach the depth: {depth:?}"
    );
    let (a, chans) = receive(&b, &[ch], 30.0);
    let row = chans
        .iter()
        .find(|r| text(r, "name") == "118.035")
        .expect("the channel row");
    assert!(common::int(row, "count") == 1, "one transmission heard");
    let on = measure::rms(&a[(1.0 * 48e3) as usize..(2.2 * 48e3) as usize]);
    let off = measure::rms(&a[(3.0 * 48e3) as usize..]);
    eprintln!("received speech RMS {on:.3}, after {off:.4}");
    assert!(on > 0.05, "the receiver plays the speech: {on}");
    assert!(
        off < 0.1 * on,
        "the channel closes after the transmission: {off}"
    );
}

#[test]
fn the_depth_never_passes_100_percent_with_full_scale_and_overdriven_audio() {
    let (_, f) = channel();
    let gained = |x: Vec<f32>, amp: f64| x.iter().map(|v| v * amp as f32).collect::<Vec<f32>>();
    for spacing in ["8.33", "25"] {
        for depth in ["85", "100"] {
            let m = (depth.parse::<f64>().unwrap() / 100.0).min(MAX_DEPTH);
            for (what, amp) in [("full scale", 1.0), ("20 dB over", 10.0)] {
                for (src, mic) in [
                    ("tone", audio(2.0, tone(1000.0, amp))),
                    ("steady speech", gained(voice(0.3, 2.0, false), amp)),
                    ("speech", gained(voice(0.3, 2.0, true), amp)),
                ] {
                    let settings = [("tx_am_depth", depth), ("tx_spacing", spacing)];
                    let (b, _, reports) = talk(f, &settings, mic, 1.5);
                    let (up, down) = depths(part(&b, 0.05, 1.45), carrier(m));
                    eprintln!(
                        "{spacing} kHz, {depth} %, {what}, {src}: depth up {:.2} %, down {:.2} %",
                        100.0 * up,
                        100.0 * down
                    );
                    // The peaks between the 48 kHz samples rise by up to 4 % of the depth.
                    let most = (m * 1.04).min(1.0);
                    assert!(
                        up <= most && down <= most,
                        "{depth} {what} {src}: {up} {down}"
                    );
                    assert!(
                        reports.iter().all(|d| *d <= 100.0 * m + 1e-6),
                        "{reports:?}"
                    );
                    let top = b.samples.iter().map(|z| z.norm()).fold(0.0, f32::max);
                    assert!(top <= 1.0, "full scale is the bound: {top}");
                }
            }
        }
    }
}

#[test]
fn high_tones_at_full_depth_never_pass_100_percent() {
    // Tones whose peaks fall between the 48 kHz samples: a whole number of samples per period (16 to 11) and 4.5 kHz, at
    // full scale and 20 dB over, long enough for the level control to settle.
    let (_, f) = channel();
    for spacing in ["8.33", "25"] {
        for hz in [
            3000.0,
            48e3 / 14.0,
            48e3 / 13.0,
            4000.0,
            48e3 / 11.0,
            4500.0,
        ] {
            for amp in [1.0, 10.0] {
                for phase in [0.0, 0.37] {
                    let mic = audio(4.0, move |t| (amp * (TAU * hz * t + phase).sin()) as f32);
                    let settings = [("tx_am_depth", "100"), ("tx_spacing", spacing)];
                    let (b, _, _) = talk(f, &settings, mic, 3.5);
                    let (up, down) = depths(part(&b, 0.05, 3.45), carrier(MAX_DEPTH));
                    eprintln!(
                        "{spacing} kHz, {hz:.1} Hz, amplitude {amp}, phase {phase}: depth up {:.2} %, down {:.2} %",
                        100.0 * up,
                        100.0 * down
                    );
                    assert!(
                        up <= 1.0 && down <= 1.0,
                        "{spacing} {hz} {amp}: {up} {down}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_occupied_bandwidth_stays_inside_the_channel_for_both_spacings() {
    let (_, f) = channel();
    for (spacing, width) in [("8.33", 25e3 / 3.0), ("25", 25e3)] {
        for (what, syllables, gain) in [
            ("steady speech", false, 1.0f32),
            ("steady speech 20 dB over", false, 10.0),
            ("speech", true, 1.0),
            ("speech 20 dB over", true, 10.0),
        ] {
            let mic: Vec<f32> = voice(0.3, 3.0, syllables)
                .iter()
                .map(|x| x * gain)
                .collect();
            let (b, _, _) = talk(f, &[("tx_spacing", spacing)], mic, 2.5);
            let (bw, far) = occupied(part(&b, 0.3, 2.4), width / 2.0);
            eprintln!("{spacing} kHz, {what}: 99 % bandwidth {bw:.0} Hz, strongest beyond the channel edge {far:.1} dB");
            assert!(bw < width, "{spacing} {what}: {bw}");
            assert!(far < -60.0, "{spacing} {what}: {far}");
        }
    }
}

#[test]
fn the_carrier_rises_and_falls_with_a_ramp() {
    let (_, f) = channel();
    let (b, _, _) = talk(f, &[], vec![], 0.5);
    let c = carrier(0.85);
    let mag: Vec<f64> = b.samples.iter().map(|z| f64::from(z.norm())).collect();
    let first = |level: f64| mag.iter().position(|m| *m >= level * c).unwrap();
    let last = |level: f64| mag.iter().rposition(|m| *m >= level * c).unwrap();
    let rise = (first(0.9) - first(0.1)) as f64 / RATE;
    let fall = (last(0.1) - last(0.9)) as f64 / RATE;
    eprintln!("rise {:.2} ms, fall {:.2} ms", 1e3 * rise, 1e3 * fall);
    let edge = 0.59 * RISE_S;
    for t in [rise, fall] {
        assert!(
            (t - edge).abs() < 0.2e-3,
            "a raised cosine of 5 ms: 10 to 90 % in {t}"
        );
    }
    let tail = &mag[last(0.1) + 1..];
    assert!(tail.iter().all(|m| *m < 0.1 * c), "nothing after the fall");
    assert!(*mag.last().unwrap() < 1e-3 * c, "the carrier ends at zero");
}

#[test]
fn no_speech_is_lost_at_key_down_and_the_start_is_stated() {
    let (_, f) = channel();
    let lead = Duration::from_millis(300);
    // A 1 kHz tone of 100 ms from the instant of key down, then silence.
    let mic = audio(1.0, |t| if t < 0.1 { tone(1000.0, 0.5)(t) } else { 0.0 });
    let (mut rx, sink) = station(f, &[], mic, Some(lead));
    idle(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 0.5);
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, 0.3);
    let b = sink.bursts().remove(0);
    assert_eq!(b.starts.len(), 1, "one stated start");
    let (at, utc) = b.starts[0];
    assert_eq!(
        at, 0,
        "the start is on the first sample of the transmission"
    );
    let early = NOW + lead.as_secs_f64();
    assert!(utc >= early && utc <= early + 0.2 + 1e-6, "{}", utc - NOW);
    let c = carrier(0.85);
    let up = b
        .samples
        .iter()
        .position(|z| f64::from(z.norm()) >= 0.999 * c)
        .unwrap();
    // The carrier is up from `up` until the fall 0.5 s later; the speech is the envelope off the carrier in between.
    let modulated: Vec<usize> = b.samples[..up + (0.4 * RATE) as usize]
        .iter()
        .enumerate()
        .skip(up)
        .filter(|(_, z)| (f64::from(z.norm()) / c - 1.0).abs() > 0.3)
        .map(|(i, _)| i)
        .collect();
    let (s, e) = (modulated[0], *modulated.last().unwrap());
    let length = (e - s) as f64 / RATE;
    eprintln!(
        "carrier up at {:.2} ms, speech from {:.2} ms for {:.2} ms",
        1e3 * up as f64 / RATE,
        1e3 * s as f64 / RATE,
        1e3 * length
    );
    assert!(s >= up, "the speech starts once the carrier is up");
    assert!(
        (length - 0.1).abs() < 2e-3,
        "the whole 100 ms of speech is sent: {length}"
    );
}

#[test]
fn the_time_out_and_halt_end_the_transmission_with_its_end_mark() {
    let (_, f) = channel();
    let (mut rx, sink) = station(f, &[("tx_timeout", "1")], vec![], None);
    idle(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 1.5);
    assert!(!fields(&rx).transmitting, "the time-out ended it");
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 0.3);
    assert!(fields(&rx).transmitting);
    let id = Owner::commands(&rx).id("halt").unwrap();
    let args = Owner::commands(&rx).def(id).unwrap().new_args();
    owner::invoke(&mut rx, id, &args).unwrap();
    idle(&mut rx, 0.2);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    assert!(b.iter().all(|b| b.ended), "both have their end mark");
    let first = b[0].samples.len() as f64 / RATE;
    assert!(
        (first - 1.0).abs() < 0.03,
        "the time-out after 1 s: {first}"
    );
}

#[test]
fn nothing_goes_on_air_with_the_transmitter_off() {
    let (_, f) = channel();
    let (mut rx, sink) = station(f, &[], audio(1.0, tone(1000.0, 0.5)), None);
    rx.set_transmit(false);
    idle(&mut rx, BEFORE);
    assert!(ptt(&mut rx, true).is_err(), "push-to-talk is refused");
    idle(&mut rx, 0.5);
    assert!(sink.bursts().is_empty());
    assert!(!fields(&rx).transmitting);
}

/// FNV-1a over the transmissions of `sink` joined across passes: per transmission its length, the bits of every sample,
/// its end mark, and its stated starts (place, UTC to 1 µs).
fn digest(sink: &Sink) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for b in v.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    for b in sink.bursts() {
        put(b.samples.len() as u64);
        for z in &b.samples {
            put(u64::from(z.re.to_bits()) << 32 | u64::from(z.im.to_bits()));
        }
        put(u64::from(b.ended));
        for (at, utc) in &b.starts {
            put(*at as u64);
            put((utc * 1e6).round() as u64);
        }
    }
    h
}

/// The samples of the sender for fixed audio: speech on a live radio with a stated start, two transmissions in a row,
/// and overdriven audio at full depth for 25 kHz that the time-out and `halt` end.
#[test]
fn the_samples_of_the_sender_keep_their_digest() {
    let (_, f) = channel();
    let mut got = Vec::new();
    let (mut rx, sink) = station(
        f,
        &[],
        voice(0.3, 2.5, true),
        Some(Duration::from_millis(300)),
    );
    idle(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 1.0);
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, 0.2);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 0.6);
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, 0.3);
    assert!(sink.bursts().iter().all(|b| b.ended && b.starts.len() == 1));
    assert_eq!(sink.bursts().len(), 2);
    got.push(digest(&sink));
    let loud: Vec<f32> = voice(0.3, 3.0, false).iter().map(|x| x * 10.0).collect();
    let settings = [
        ("tx_am_depth", "100"),
        ("tx_spacing", "25"),
        ("tx_timeout", "1"),
    ];
    let (mut rx, sink) = station(f, &settings, loud, None);
    idle(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 1.3);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, 0.4);
    let id = Owner::commands(&rx).id("halt").unwrap();
    let args = Owner::commands(&rx).def(id).unwrap().new_args();
    owner::invoke(&mut rx, id, &args).unwrap();
    idle(&mut rx, 0.2);
    assert!(sink.bursts().iter().all(|b| b.ended));
    assert_eq!(sink.bursts().len(), 2);
    got.push(digest(&sink));
    eprintln!("airband transmit digests {got:#018x?}");
    assert_eq!(got, [0xca989a3d07fc0fd6, 0x57596692ba346e9e]);
}

/// FNV-1a over the words `v`.
fn fnv(v: impl IntoIterator<Item = u64>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for w in v {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The bits of the samples `x`.
fn bits(x: &[Complex32]) -> impl Iterator<Item = u64> + '_ {
    x.iter()
        .map(|z| u64::from(z.re.to_bits()) << 32 | u64::from(z.im.to_bits()))
}

/// The words of the result fields `f`.
fn field_words(f: &AirbandTxFields) -> [u64; 5] {
    let opt = |v: Option<f64>| v.map_or(u64::MAX, f64::to_bits);
    [
        u64::from(f.transmitting),
        opt(f.tx_freq_hz.map(|h| h.0)),
        opt(f.tx_time_s.map(|s| s.0)),
        opt(f.tx_mic_dbfs),
        opt(f.tx_depth),
    ]
}

/// The output of the sender for a 1 kHz tone, speech-like audio from a fixed seed and silence on a live radio: per input
/// the hash of the key down (the first 20 ms), the steady state, the key up (the last 20 ms with the tail of the rate
/// change), and the result fields every 50 ms with the answers to push-to-talk (again while on, and in the fall).
#[test]
fn the_sender_output_is_pinned_for_tone_speech_and_silence() {
    let (_, f) = channel();
    let inputs = [
        ("tone", audio(1.5, tone(1000.0, 0.5))),
        ("speech", voice(0.3, 1.5, true)),
        ("silence", vec![0.0; (1.5 * 48e3) as usize]),
    ];
    let step = |rx: &mut StackGraph, words: &mut Vec<u64>, seconds: f64| {
        let zeros = vec![Complex32::default(); (RATE / 20.0) as usize];
        let mut out = Output::default();
        for _ in 0..(seconds * 20.0).round() as usize {
            out.clear();
            rx.feed(&zeros, &mut out);
            words.extend(field_words(&fields(rx)));
        }
    };
    let answer = |r: Result<(), sdrkit_core::CommandError>| match r {
        Ok(()) => 0,
        Err(e) => fnv(e.to_string().bytes().map(u64::from)),
    };
    let mut got = Vec::new();
    for (name, mic) in inputs {
        let (mut rx, sink) = station(f, &[], mic, Some(Duration::from_millis(300)));
        let mut words = Vec::new();
        step(&mut rx, &mut words, BEFORE);
        words.push(answer(ptt(&mut rx, true)));
        step(&mut rx, &mut words, 0.5);
        words.push(answer(ptt(&mut rx, true)));
        step(&mut rx, &mut words, 0.5);
        words.push(answer(ptt(&mut rx, false)));
        words.push(answer(ptt(&mut rx, true)));
        step(&mut rx, &mut words, 0.3);
        let b = sink.bursts();
        assert_eq!(b.len(), 1, "{name}: one transmission");
        assert!(b[0].ended && b[0].starts.len() == 1, "{name}");
        let s = &b[0].samples;
        let edge = (0.02 * RATE) as usize;
        let mut down: Vec<u64> = bits(&s[..edge]).collect();
        down.extend(b[0].starts.iter().flat_map(|(at, utc)| [*at as u64, utc.to_bits()]));
        got.push([
            fnv(down),
            fnv(bits(&s[edge..s.len() - edge])),
            fnv(bits(&s[s.len() - edge..]).chain([s.len() as u64])),
            fnv(words),
        ]);
    }
    eprintln!("airband transmit pins {got:#018x?}");
    assert_eq!(
        got,
        [
            [
                0x77841823d2326695,
                0xee118ad7cd8eb0d5,
                0x809212442b18d191,
                0x6f82f03f82f077a4
            ],
            [
                0x1e8c3d32a592cb94,
                0x1ba43f8bc8f60359,
                0x1df403b98f324ac5,
                0x9762c3cbf1f89c6f
            ],
            [
                0xac0736553ebac049,
                0x65a349360f9c23d0,
                0x1df403b98f324ac5,
                0xbf94968ee37fc3ad
            ],
        ]
    );
}

/// The output of the sender when the settings change while keyed: the transmission on air keeps its settings, the next
/// one takes the new depth, gain, spacing and time-out. The hash covers both transmissions and the result fields every
/// 50 ms.
#[test]
fn the_settings_changed_while_keyed_are_pinned() {
    let (_, f) = channel();
    let (mut rx, sink) = station(f, &[], audio(2.5, tone(1000.0, 0.3)), None);
    let mut words = Vec::new();
    let mut step = |rx: &mut StackGraph, seconds: f64| {
        let zeros = vec![Complex32::default(); (RATE / 20.0) as usize];
        let mut out = Output::default();
        for _ in 0..(seconds * 20.0).round() as usize {
            out.clear();
            rx.feed(&zeros, &mut out);
            words.extend(field_words(&fields(rx)));
        }
    };
    step(&mut rx, BEFORE);
    ptt(&mut rx, true).unwrap();
    step(&mut rx, 0.3);
    common::set(&mut rx, "tx_am_depth", &Value::F64(50.0)).unwrap();
    common::set(&mut rx, "tx_mic_gain", &Value::F64(6.0)).unwrap();
    common::set(&mut rx, "tx_spacing", &Value::Choice(1)).unwrap();
    common::set(&mut rx, "tx_timeout", &Value::Duration(1.0)).unwrap();
    step(&mut rx, 0.3);
    ptt(&mut rx, false).unwrap();
    step(&mut rx, 0.2);
    ptt(&mut rx, true).unwrap();
    step(&mut rx, 1.3);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    assert!(b.iter().all(|b| b.ended));
    let mut all = Vec::new();
    for b in &b {
        all.push(b.samples.len() as u64);
        all.extend(bits(&b.samples));
    }
    let got = [fnv(all), fnv(words)];
    eprintln!("airband transmit setting pins {got:#018x?}");
    assert_eq!(got, [0xb941043b4f97c183, 0x1cfc589219de9aa6]);
}

/// While push-to-talk is on, the stack plays no receive audio; after key up and the hold it plays the channel again.
#[test]
fn the_receive_audio_is_muted_while_keyed_and_comes_back_after_key_up() {
    let (ch, f) = channel();
    let lt = list_to_text(&[ch]);
    let (mut rx, sink) = station(f, &[("channels", &lt)], vec![], None);
    // A station on the channel with a 1 kHz tone at 50 % for 4 s, at the tuning offset of the stack.
    let c = carrier(0.85);
    let mut x: Vec<Complex32> = (0..(4.0 * RATE) as usize)
        .map(|i| {
            let t = i as f64 / RATE;
            Complex32::new((c * (1.0 + 0.5 * (TAU * 1000.0 * t).sin())) as f32, 0.0)
        })
        .collect();
    common::offset(&mut x, RATE, -TUNE_OFFSET_HZ);
    let mut out = Output::default();
    let mut rms = Vec::new();
    for (k, piece) in x.chunks((RATE / 10.0) as usize).enumerate() {
        if k == 15 {
            ptt(&mut rx, true).unwrap();
        }
        if k == 25 {
            ptt(&mut rx, false).unwrap();
        }
        out.clear();
        rx.feed(piece, &mut out);
        let step = out.audio_format().map_or(1, |f| usize::from(f.1.max(1)));
        let a: Vec<f32> = out.audio().iter().step_by(step).copied().collect();
        assert!(!a.is_empty(), "audio in each piece");
        rms.push(measure::rms(&a));
    }
    eprintln!("receive audio RMS by 0.1 s: {rms:.3?}");
    assert_eq!(sink.bursts().len(), 1);
    let before = rms[10..15].iter().copied().fold(f64::MAX, f64::min);
    assert!(before > 0.05, "the channel plays before key down: {rms:?}");
    assert!(
        rms[16..25].iter().all(|r| *r == 0.0),
        "no receive audio while keyed: {rms:?}"
    );
    assert!(
        rms[28..].iter().all(|r| *r > 0.5 * before),
        "the channel plays again after key up: {rms:?}"
    );
}

/// The mean depth `|(|z| / c) - 1|` of `x` with carrier `c`.
fn mean_depth(x: &[Complex32], c: f64) -> f64 {
    x.iter()
        .map(|z| (f64::from(z.norm()) / c - 1.0).abs())
        .sum::<f64>()
        / x.len() as f64
}

/// Speech at its usual level and 20 dB under, steady and in syllables: the mean depth over the transmission and after 2 s
/// stay at or over their bounds while the peaks stay at the depth. The leveller starts each transmission at 0 dB and rises
/// with 1 s, so quiet speech has its full gain only after about 2 s.
#[test]
fn the_leveller_raises_the_mean_depth_of_speech() {
    let (_, f) = channel();
    for (what, level, syllables, least, least_settled) in [
        ("steady speech", 0.3, false, 0.23, 0.235),
        ("speech", 0.3, true, 0.095, 0.105),
        ("steady speech 20 dB under", 0.03, false, 0.05, 0.058),
        ("speech 20 dB under", 0.03, true, 0.016, 0.02),
    ] {
        let (b, _, _) = talk(f, &[], voice(level, 4.0, syllables), 3.5);
        let x = part(&b, 0.3, 3.4);
        let (up, down) = depths(x, carrier(0.85));
        let mean = mean_depth(x, carrier(0.85));
        let settled = mean_depth(part(&b, 2.0, 3.4), carrier(0.85));
        eprintln!(
            "{what}: mean depth {:.1} %, after 2 s {:.1} %, peak up {:.2} %, down {:.2} %",
            100.0 * mean,
            100.0 * settled,
            100.0 * up,
            100.0 * down
        );
        assert!(mean >= least, "{what}: {mean}");
        assert!(settled >= least_settled, "{what}: {settled}");
        assert!(
            up <= 0.85 * 1.04 && down <= 0.85 * 1.04,
            "{what}: {up} {down}"
        );
    }
}

/// A step to full scale and to 20 dB over, from silence and from speech 20 dB under that has brought the leveller to its
/// +12 dB: the depth never passes 100 %, nor the set depth with the rise between the samples.
#[test]
fn the_depth_never_passes_100_percent_at_a_step_to_full_scale() {
    let (_, f) = channel();
    for spacing in ["8.33", "25"] {
        for depth in ["85", "100"] {
            let m = (depth.parse::<f64>().unwrap() / 100.0).min(MAX_DEPTH);
            for (before, quiet) in [("silence", 0.0f32), ("speech 20 dB under", 0.1)] {
                for amp in [1.0f32, 10.0] {
                    for (src, loud) in [
                        ("tone", audio(1.5, tone(1000.0, 1.0))),
                        ("speech", voice(1.0, 1.5, true)),
                    ] {
                        let mut mic: Vec<f32> =
                            voice(0.3, 3.0, false).iter().map(|x| x * quiet).collect();
                        mic.extend(loud.iter().map(|x| x * amp));
                        let settings = [("tx_am_depth", depth), ("tx_spacing", spacing)];
                        let (b, _, reports) = talk(f, &settings, mic, 4.0);
                        let (up, down) = depths(part(&b, 0.05, 3.95), carrier(m));
                        eprintln!(
                            "{spacing} kHz, {depth} %, {before} to {src} x{amp}: depth up {:.2} %, down {:.2} %",
                            100.0 * up,
                            100.0 * down
                        );
                        let most = (m * 1.04).min(1.0);
                        assert!(
                            up <= most && down <= most,
                            "{before} {src} {amp}: {up} {down}"
                        );
                        assert!(
                            reports.iter().all(|d| *d <= 100.0 * m + 1e-6),
                            "{reports:?}"
                        );
                        let top = b.samples.iter().map(|z| z.norm()).fold(0.0, f32::max);
                        assert!(top <= 1.0, "full scale is the bound: {top}");
                    }
                }
            }
        }
    }
}
