//! The transmit branch: every mode through the stack's own receive chain, the occupied bandwidth of each mode, the limiter, the
//! time-out, `halt`, the end mark of every transmission, the muted receive audio, and nothing on air with the transmitter off.
//! The transmit stream is reached as the engine reaches it: the receiver's graph as `Any`, with the host streams in its context.

use crate::common::{first_channel, values};
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Clock, Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Component, ComponentInfo, Format, Graph, Io, Port, RunContext};
use sdrkit_stack_general::typed::TransmitFields;
use sdrkit_stack_general::{GeneralReceiver, Mic};
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The dial of the transmitting stack.
const DIAL: f64 = 145.5e6;

/// The clock of a host that times bursts.
const NOW: f64 = 1_800_000_000.0;

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

/// One transmission: its samples, its centre, whether its end mark came, and the stated starts in it (the place and
/// the UTC).
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
    fn is_empty(&self) -> bool {
        self.writes.lock().unwrap().is_empty()
    }
}

/// Test audio at 48 kHz, as many samples as the session's samples bring time: `shape(t)` at `t` seconds.
struct TestAudio {
    rate: f64,
    owed: f64,
    n: u64,
    shape: fn(f64) -> f32,
}

impl Component for TestAudio {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.audio", vec![Port::iq("in")], vec![Port::audio("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, sdrkit_core::ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![Format::audio(48_000.0, 1)])
    }
    fn process(&mut self, io: &mut Io) {
        self.owed += io.input_len(0) as f64 * 48_000.0 / self.rate;
        let m = self.owed.floor();
        self.owed -= m;
        let out = &mut io.audio_out(0).samples;
        for _ in 0..m as usize {
            out.push((self.shape)(self.n as f64 / 48_000.0));
            self.n += 1;
        }
    }
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
}

fn tone_1k(t: f64) -> f32 {
    (0.25 * (TAU * 1000.0 * t).sin()) as f32
}

/// Speech-like audio: four tones at random phases, their sum loud enough that the limiter acts.
fn speechlike(t: f64) -> f32 {
    let s: f64 = [(410.0, 0.3), (930.0, 1.7), (1730.0, 4.1), (2610.0, 5.3)]
        .iter()
        .map(|(f, p)| (TAU * f * t + p).sin())
        .sum();
    (0.4 * s) as f32
}

/// Overdriven audio: quiet for 2 s, then a square wave far beyond full scale.
fn overdriven(t: f64) -> f32 {
    if t < 2.0 {
        (0.01 * (TAU * 800.0 * t).sin()) as f32
    } else if (t * 600.0).fract() < 0.5 {
        3.0
    } else {
        -3.0
    }
}

/// A transmitting stack at `rate` with `settings`, the microphone fed with `shape`, the transmitter on, and its transmit stream.
fn station(
    rate: f64,
    settings: &[(&str, &str)],
    shape: fn(f64) -> f32,
) -> (GeneralReceiver, Arc<Sink>) {
    station_on(rate, settings, shape, None)
}

/// As [`station`]; with `lead`, on a live radio whose host times bursts with that lead, at the clock [`NOW`].
fn station_on(
    rate: f64,
    settings: &[(&str, &str)],
    shape: fn(f64) -> f32,
    lead: Option<Duration>,
) -> (GeneralReceiver, Arc<Sink>) {
    let mic = TestAudio {
        rate: 0.0,
        owed: 0.0,
        n: 0,
        shape,
    };
    let mut rx = GeneralReceiver::with_mic(
        &StackConfig::new(values(settings)),
        Mic::Source(Box::new(mic)),
    )
    .expect("the stack takes the settings");
    let input = InputInfo::new(rate, DIAL + 50e3, DIAL);
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
        .expect("the main channel is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(NOW);
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(true);
    (rx, sink)
}

/// Runs command `key` with the arguments `args`.
fn call(
    rx: &mut GeneralReceiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), sdrkit_core::CommandError> {
    let id = Owner::commands(rx).id(key).unwrap();
    let mut a = Owner::commands(rx).def(id).unwrap().new_args();
    for (k, v) in args {
        let i = a.decl().id(k).unwrap();
        a.set_value(i, v).unwrap();
    }
    owner::invoke(rx, id, &a).map(|_| ())
}

fn ptt(rx: &mut GeneralReceiver, on: bool) -> Result<(), sdrkit_core::CommandError> {
    call(rx, "ptt", &[("on", Value::Bool(on))])
}

/// Feeds `seconds` of silence in pieces of 0.1 s; returns the audio of the stack.
fn idle(rx: &mut GeneralReceiver, rate: f64, seconds: f64) -> Vec<f32> {
    let piece = (rate / 10.0) as usize;
    let zeros = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut audio = Vec::new();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
        audio.extend(first_channel(&out));
    }
    audio
}

fn fields(rx: &GeneralReceiver) -> TransmitFields {
    TransmitFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// The audio of the stack's own receive chain, set as `settings`, on `burst` at `rate`.
fn receive(burst: &Burst, rate: f64, settings: &[(&str, &str)]) -> Vec<f32> {
    let mut s: Vec<(&str, &str)> = vec![
        ("am_auto", "0"),
        ("am_detector", "envelope"),
        ("squelch", "off"),
    ];
    s.extend_from_slice(settings);
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&s))).unwrap();
    rx.configure(&InputInfo::new(rate, burst.centre, burst.centre).recording())
        .unwrap();
    let mut out = Output::default();
    let mut audio = Vec::new();
    for c in burst.samples.chunks((rate / 10.0) as usize) {
        out.clear();
        rx.feed(c, &mut out);
        audio.extend(first_channel(&out));
    }
    audio
}

/// As [`receive`], and the receive chain after the burst.
fn receive_with(
    burst: &Burst,
    rate: f64,
    settings: &[(&str, &str)],
) -> (Vec<f32>, GeneralReceiver) {
    let mut s: Vec<(&str, &str)> = vec![
        ("am_auto", "0"),
        ("am_detector", "envelope"),
        ("squelch", "off"),
    ];
    s.extend_from_slice(settings);
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&s))).unwrap();
    rx.configure(&InputInfo::new(rate, burst.centre, burst.centre).recording())
        .unwrap();
    let mut out = Output::default();
    let mut audio = Vec::new();
    for c in burst.samples.chunks((rate / 10.0) as usize) {
        out.clear();
        rx.feed(c, &mut out);
        audio.extend(first_channel(&out));
    }
    (audio, rx)
}

/// The frequency of the strongest tone of `x` (48 kHz) between `lo` and `hi`, to 0.1 Hz, and its amplitude.
fn strongest(x: &[f32], lo: f64, hi: f64) -> (f64, f64) {
    let mut best = (0.0, 0.0);
    let mut f = lo;
    while f <= hi {
        let a = crate::common::tone(x, f);
        if a > best.1 {
            best = (f, a);
        }
        f += 0.5;
    }
    let (c, _) = best;
    let mut f = c - 0.5;
    while f <= c + 0.5 {
        let a = crate::common::tone(x, f);
        if a > best.1 {
            best = (f, a);
        }
        f += 0.1;
    }
    best
}

/// The power spectrum of `x` at `rate` (linear, sum = mean power), `n` points, lowest frequency first, and the bin width.
fn spectrum(x: &[Complex32], rate: f64, n: usize) -> (Vec<f64>, f64) {
    let mut p = vec![0.0f32; n];
    welch(x, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
    (p.iter().map(|v| f64::from(*v)).collect(), rate / n as f64)
}

/// The most power in one bin at `|f| >= from` against the most power in any bin, dB.
fn beyond_db(p: &[f64], bin: f64, from: f64) -> f64 {
    let n = p.len();
    let peak = p.iter().copied().fold(0.0, f64::max);
    let far = p
        .iter()
        .enumerate()
        .filter(|(i, _)| ((*i as f64 - (n / 2) as f64) * bin).abs() >= from)
        .map(|(_, v)| *v)
        .fold(0.0, f64::max);
    10.0 * (far / peak).log10()
}

/// The power within `f0 +- half` against the most power in any bin, dB.
fn band_db(p: &[f64], bin: f64, f0: f64, half: f64) -> f64 {
    let n = p.len();
    let peak = p.iter().copied().fold(0.0, f64::max);
    let inside = p
        .iter()
        .enumerate()
        .filter(|(i, _)| ((*i as f64 - (n / 2) as f64) * bin - f0).abs() <= half)
        .map(|(_, v)| *v)
        .fold(0.0, f64::max);
    10.0 * (inside / peak).log10()
}

/// The instantaneous frequency of `x` at `rate`, Hz, where the carrier is up (between the rise and the fall).
fn frequency(x: &[Complex32], rate: f64) -> Vec<f64> {
    x.windows(2)
        .filter(|w| w[0].norm() > 0.89 && w[1].norm() > 0.89)
        .map(|w| {
            let d = w[1] * w[0].conj();
            f64::from(d.im).atan2(f64::from(d.re)) * rate / TAU
        })
        .collect()
}

const RATE: f64 = 250e3;

/// One voice transmission of `seconds` in `demod` with `settings` and `shape`: the transmission, the stack afterwards.
fn talk(
    demod: &str,
    settings: &[(&str, &str)],
    shape: fn(f64) -> f32,
    seconds: f64,
) -> (Burst, GeneralReceiver) {
    let mut s = vec![("demod", demod)];
    s.extend_from_slice(settings);
    let (mut rx, sink) = station(RATE, &s, shape);
    idle(&mut rx, RATE, 0.2);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, RATE, seconds);
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, RATE, 0.3);
    let mut b = sink.bursts();
    assert_eq!(b.len(), 1, "{demod}: one transmission");
    let b = b.remove(0);
    assert!(b.ended, "{demod}: the transmission has its end mark");
    (b, rx)
}

#[test]
fn nfm_round_trip_keeps_the_tone_and_scales_with_the_deviation() {
    let mut levels = Vec::new();
    for dev in ["5000", "2500"] {
        let (b, _) = talk("nfm", &[("tx_deviation", dev)], tone_1k, 4.0);
        assert!(
            (b.centre - DIAL).abs() < 1e-3,
            "centred on the transmit frequency"
        );
        let audio = receive(&b, RATE, &[("demod", "nfm")]);
        let tail = &audio[audio.len() - 72_000..audio.len() - 24_000];
        let (f, a) = strongest(tail, 950.0, 1050.0);
        assert!((f - 1000.0).abs() <= 0.5, "{dev}: {f} Hz");
        let purity = a / (2f64.sqrt() * crate::common::rms(tail));
        assert!(purity > 0.95, "{dev}: tone share {purity}");
        levels.push(a);
        // The deviation of the tone after the level control: its RMS target is 0.3 of the 88 % that speech may use.
        let fq = frequency(&b.samples[b.samples.len() / 2..], RATE);
        let peak = fq.iter().copied().fold(0.0f64, |m, v| m.max(v.abs()));
        let want = 0.3 * 2f64.sqrt() * dev.parse::<f64>().unwrap();
        assert!(
            (peak / want - 1.0).abs() < 0.1,
            "{dev}: peak deviation {peak} Hz, want {want}"
        );
    }
    let ratio_db = 20.0 * (levels[0] / levels[1]).log10();
    assert!(
        (ratio_db - 6.02).abs() < 0.5,
        "5 kHz against 2.5 kHz: {ratio_db} dB"
    );
}

#[test]
fn am_round_trip_keeps_the_tone_and_scales_with_the_depth() {
    let mut levels = Vec::new();
    for depth in ["90", "45"] {
        let (b, _) = talk("am", &[("tx_am_depth", depth)], tone_1k, 4.0);
        let audio = receive(&b, RATE, &[("demod", "am")]);
        let tail = &audio[audio.len() - 72_000..audio.len() - 24_000];
        let (f, a) = strongest(tail, 950.0, 1050.0);
        assert!((f - 1000.0).abs() <= 0.5, "{depth}: {f} Hz");
        levels.push(a);
        // The modulation depth of the tone on the samples: the level control holds it at 0.3 RMS of the limit.
        let env: Vec<f32> = b.samples[b.samples.len() / 2..b.samples.len() - 50_000]
            .iter()
            .map(|z| z.norm())
            .collect();
        let (hi, lo) = env
            .iter()
            .fold((0.0f32, f32::MAX), |(h, l), &v| (h.max(v), l.min(v)));
        let m = f64::from((hi - lo) / (hi + lo));
        let want = 0.3 * 2f64.sqrt() * depth.parse::<f64>().unwrap() / 100.0;
        assert!(
            (m / want - 1.0).abs() < 0.1,
            "{depth}: depth {m}, want {want}"
        );
        assert!(hi <= 0.9 + 1e-3, "{depth}: peak {hi}");
    }
    let ratio_db = 20.0 * (levels[0] / levels[1]).log10();
    assert!(
        (ratio_db - 6.02).abs() < 0.5,
        "90 % against 45 %: {ratio_db} dB"
    );
}

#[test]
fn ssb_round_trip_keeps_the_tone_on_its_side_with_both_methods() {
    for (demod, sign) in [("usb", 1.0), ("lsb", -1.0)] {
        for method in ["filter", "phasing"] {
            let (b, _) = talk(demod, &[("tx_ssb_method", method)], tone_1k, 3.0);
            let audio = receive(&b, RATE, &[("demod", demod)]);
            let tail = &audio[audio.len() - 60_000..audio.len() - 24_000];
            let (f, a) = strongest(tail, 950.0, 1050.0);
            assert!((f - 1000.0).abs() <= 0.5, "{demod} {method}: {f} Hz");
            let purity = a / (2f64.sqrt() * crate::common::rms(tail));
            assert!(purity > 0.95, "{demod} {method}: tone share {purity}");
            // On the samples: the tone on its side, the other side and the carrier at least 40 dB down.
            let x = &b.samples[b.samples.len() / 3..b.samples.len() * 2 / 3];
            let (p, bin) = spectrum(x, RATE, 1 << 15);
            let tone = band_db(&p, bin, sign * 1000.0, 10.0);
            let image = band_db(&p, bin, -sign * 1000.0, 10.0);
            let carrier = band_db(&p, bin, 0.0, 10.0);
            assert!(tone > -0.1, "{demod} {method}: the tone is the peak");
            assert!(image < -40.0, "{demod} {method}: other side {image} dB");
            assert!(carrier < -40.0, "{demod} {method}: carrier {carrier} dB");
            eprintln!("{demod} {method}: other side {image:.1} dB, carrier {carrier:.1} dB");
        }
    }
}

#[test]
fn speech_stays_in_its_band_in_every_voice_mode() {
    // NFM within Carson's rule 2 (5000 + 3000) Hz; SSB with the other side and the carrier at least 40 dB down.
    let (b, _) = talk("nfm", &[], speechlike, 4.0);
    let x = &b.samples[RATE as usize..b.samples.len() - RATE as usize / 2];
    let (p, bin) = spectrum(x, RATE, 1 << 14);
    let n = p.len();
    let (lo, hi) = sdrkit_algo_dsp::bandwidth::power_share_edges(&p, 0, n - 1, 0.99).unwrap();
    let obw = (hi - lo + 1) as f64 * bin;
    eprintln!("NFM speech: 99 % bandwidth {obw:.0} Hz");
    assert!(obw <= 16_000.0, "NFM 99 % bandwidth {obw} Hz");
    let peak_dev = frequency(x, RATE)
        .iter()
        .copied()
        .fold(0.0f64, |m, v| m.max(v.abs()));
    assert!(peak_dev <= 5000.0 * 1.02, "peak deviation {peak_dev} Hz");
    for (demod, sign) in [("usb", 1.0), ("lsb", -1.0)] {
        for comp in ["0", "10"] {
            let (b, _) = talk(demod, &[("tx_compression", comp)], speechlike, 3.0);
            let x = &b.samples[RATE as usize..b.samples.len() - RATE as usize / 2];
            let (p, bin) = spectrum(x, RATE, 1 << 14);
            let image = band_db(&p, bin, -sign * 1500.0, 1200.0);
            let carrier = band_db(&p, bin, 0.0, 20.0);
            eprintln!(
                "{demod} compression {comp}: other side {image:.1} dB, carrier {carrier:.1} dB"
            );
            assert!(
                image < -40.0 && carrier < -40.0,
                "{demod} {comp}: {image} {carrier}"
            );
        }
    }
}

#[test]
fn overdriven_audio_never_passes_full_scale() {
    for (demod, extra) in [
        ("nfm", ("tx_ctcss", "88.5")),
        ("am", ("tx_am_depth", "100")),
        ("usb", ("tx_compression", "20")),
        ("lsb", ("tx_ssb_method", "phasing")),
    ] {
        let (b, rx) = talk(demod, &[extra], overdriven, 4.0);
        let peak = b.samples.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
        // The limiters hold 0.9; the guard at full scale never has to act.
        assert!(peak <= 0.93, "{demod}: peak {peak}");
        let f = fields(&rx);
        assert!(!f.transmitting);
        if demod == "nfm" {
            // The tone and the speech together stay within the deviation.
            let dev = frequency(&b.samples, RATE)
                .iter()
                .copied()
                .fold(0.0f64, |m, v| m.max(v.abs()));
            assert!(dev <= 5000.0 * 1.02, "deviation {dev} Hz");
        }
        if demod == "am" {
            // No overmodulation: the carrier with its sidebands never turns negative.
            let lo = b.samples.iter().map(|z| z.re).fold(f32::MAX, f32::min);
            assert!(lo >= -1e-3, "the envelope crosses zero: {lo}");
        }
    }
}

#[test]
fn the_limiter_reports_its_action_while_talking() {
    let (mut rx, _sink) = station(RATE, &[("demod", "usb")], overdriven);
    ptt(&mut rx, true).unwrap();
    // The audio jumps from -40 dBFS to +9.5 dBFS at 2 s: the limiter holds the peaks while the level control comes down.
    idle(&mut rx, RATE, 2.1);
    let f = fields(&rx);
    assert!(f.transmitting);
    assert_eq!(f.tx_freq_hz.map(|h| h.0), Some(DIAL));
    assert!(
        f.tx_mic_dbfs.is_some_and(|d| d > 9.0),
        "{:?}",
        f.tx_mic_dbfs
    );
    assert!(
        f.tx_limit_db.is_some_and(|d| d < -1.0),
        "{:?}",
        f.tx_limit_db
    );
    assert!(f.tx_time_s.is_some_and(|s| (s.0 - 2.1).abs() < 0.2));
}

#[test]
fn cw_keys_text_with_soft_edges_at_the_dial() {
    let (mut rx, sink) = station(RATE, &[("demod", "cw"), ("tx_split", "600")], tone_1k);
    // PTT is for speech, and a WFM or voice mode does not key Morse.
    assert!(ptt(&mut rx, true).is_err());
    call(
        &mut rx,
        "send",
        &[("text", Value::Text("PARIS PARIS PARIS PARIS".into()))],
    )
    .unwrap();
    idle(&mut rx, RATE, 14.0);
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    let b = &b[0];
    assert!(b.ended);
    assert!(
        (b.centre - (DIAL + 600.0)).abs() < 1e-3,
        "the transmit offset moves the carrier"
    );
    // Four times PARIS at 20 WPM: 4 x 43 dots and three word gaps of 7, of 60 ms.
    let env: Vec<f32> = b.samples.iter().map(|z| z.norm()).collect();
    let on: Vec<usize> = env
        .iter()
        .enumerate()
        .filter(|(_, v)| **v >= 0.45)
        .map(|(i, _)| i)
        .collect();
    let span = (on.last().unwrap() - on[0]) as f64 / RATE;
    let want = (4.0 * 43.0 + 3.0 * 7.0) * 0.06;
    assert!(
        (span - want).abs() < 0.005,
        "keyed for {span} s, want {want}"
    );
    // Key clicks: at 500 Hz from the carrier the spectrum is at least 60 dB down.
    let (p, bin) = spectrum(&b.samples, RATE, 1 << 15);
    let far = beyond_db(&p, bin, 500.0);
    eprintln!("CW at 20 WPM: {far:.1} dB at 500 Hz and beyond");
    assert!(far < -60.0, "{far} dB at 500 Hz");
    // The receive chain hears the carrier at its pitch.
    let audio = receive(b, RATE, &[("demod", "cw")]);
    let (f, _) = strongest(&audio, 600.0, 800.0);
    assert!((f - 700.0).abs() <= 1.0, "pitch {f} Hz");
}

#[test]
fn farnsworth_stretches_only_the_gaps() {
    let keyed = |settings: &[(&str, &str)]| {
        let (mut rx, sink) = station(RATE, settings, tone_1k);
        call(
            &mut rx,
            "send",
            &[("text", Value::Text("PARIS PARIS".into()))],
        )
        .unwrap();
        idle(&mut rx, RATE, 12.0);
        let b = sink.bursts();
        let env: Vec<f32> = b[0].samples.iter().map(|z| z.norm()).collect();
        let on: Vec<usize> = (0..env.len()).filter(|&i| env[i] >= 0.45).collect();
        (on.last().unwrap() - on[0]) as f64 / RATE
    };
    let plain = keyed(&[("demod", "cw"), ("tx_cw_wpm", "20")]);
    let slow = keyed(&[
        ("demod", "cw"),
        ("tx_cw_wpm", "20"),
        ("tx_cw_farnsworth", "10"),
    ]);
    // 2 x 31 element dots at 60 ms in both; the 2 x 12 + 7 gap dots stretch to the 10 WPM overall speed.
    let gaps = |wpm: f64| (60.0 / wpm - 31.0 * 0.06) / 19.0;
    let want = 62.0 * 0.06 + 31.0 * gaps(10.0);
    assert!((plain - (93.0 * 0.06)).abs() < 0.005, "{plain}");
    assert!((slow - want).abs() < 0.01, "{slow} s, want {want}");
}

#[test]
fn tune_ends_at_its_time_and_halt_ends_it_at_once() {
    let (mut rx, sink) = station(RATE, &[("demod", "usb"), ("tx_tune_limit", "2")], tone_1k);
    call(&mut rx, "tune", &[]).unwrap();
    idle(&mut rx, RATE, 3.0);
    call(&mut rx, "tune", &[]).unwrap();
    idle(&mut rx, RATE, 0.5);
    call(&mut rx, "halt", &[]).unwrap();
    idle(&mut rx, RATE, 0.5);
    let b = sink.bursts();
    assert_eq!(b.len(), 2);
    for x in &b {
        assert!(x.ended, "every carrier ends with its end mark");
        // A steady carrier at the dial: magnitude 0.9 between its ramps.
        let mid = x.samples[x.samples.len() / 2];
        assert!((mid.norm() - 0.9).abs() < 1e-3 && mid.im.abs() < 1e-3);
    }
    let len = |x: &Burst| x.samples.len() as f64 / RATE;
    assert!(
        (len(&b[0]) - 2.0).abs() < 0.02,
        "tune time: {} s",
        len(&b[0])
    );
    assert!((len(&b[1]) - 0.5).abs() < 0.12, "halt: {} s", len(&b[1]));
}

#[test]
fn ptt_ends_at_the_time_out_and_the_receive_audio_comes_back() {
    let (mut rx, sink) = station(RATE, &[("demod", "nfm"), ("tx_timeout", "2")], tone_1k);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, RATE, 0.2);
    assert!(fields(&rx).transmitting);
    // PTT again while talking changes nothing; another transmission is refused.
    ptt(&mut rx, true).unwrap();
    assert!(call(&mut rx, "tune", &[]).is_err());
    idle(&mut rx, RATE, 3.0);
    assert!(!fields(&rx).transmitting, "the time-out ended it");
    let b = sink.bursts();
    assert_eq!(b.len(), 1);
    assert!(b[0].ended);
    let len = b[0].samples.len() as f64 / RATE;
    assert!((len - 2.0).abs() < 0.02, "on air {len} s");
    // PTT off after the time-out sends nothing.
    ptt(&mut rx, false).unwrap();
    idle(&mut rx, RATE, 0.5);
    assert_eq!(sink.bursts().len(), 1);
}

#[test]
fn the_receive_audio_is_muted_while_transmitting() {
    let mut gen = crate::common::generator(
        RATE,
        "nfm",
        &[("gen_snr_db", "40"), ("gen_tune_offset", "50000")],
    );
    let (mut rx, _sink) = station(RATE, &[("demod", "nfm")], tone_1k);
    let piece = (RATE / 10.0) as usize;
    let mut buf = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let mut level = |rx: &mut GeneralReceiver, seconds: f64| {
        let mut a = Vec::new();
        for _ in 0..(seconds * 10.0) as usize {
            gen.generate(&mut buf);
            out.clear();
            rx.feed(&buf, &mut out);
            a.extend(first_channel(&out));
        }
        crate::common::rms(&a[a.len() / 2..])
    };
    let before = level(&mut rx, 1.0);
    ptt(&mut rx, true).unwrap();
    let during = level(&mut rx, 1.0);
    ptt(&mut rx, false).unwrap();
    level(&mut rx, 0.5);
    let after = level(&mut rx, 1.0);
    assert!(before > 0.05, "{before}");
    assert!(during < 1e-4, "muted: {during}");
    assert!(
        (after / before - 1.0).abs() < 0.1,
        "back: {after} against {before}"
    );
}

#[test]
fn every_way_out_ends_with_the_end_mark() {
    // The transmitter switched off, a new format, a change of mode, and the end of the input.
    let (mut rx, sink) = station(RATE, &[("demod", "am")], tone_1k);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, RATE, 0.5);
    rx.set_transmit(false);
    idle(&mut rx, RATE, 0.2);
    rx.set_transmit(true);
    idle(&mut rx, RATE, 0.5);
    assert_eq!(
        sink.bursts().len(),
        1,
        "the switch coming back starts nothing"
    );
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, RATE, 0.5);
    rx.configure(&InputInfo::new(RATE, DIAL + 50e3 + 1e3, DIAL + 1e3).recording())
        .unwrap();
    idle(&mut rx, RATE, 0.2);
    call(&mut rx, "tune", &[]).unwrap();
    idle(&mut rx, RATE, 0.5);
    crate::common::set(&mut rx, "demod", "usb").unwrap();
    idle(&mut rx, RATE, 0.2);
    ptt(&mut rx, true).unwrap();
    idle(&mut rx, RATE, 0.3);
    let mut out = Output::default();
    rx.finish(&mut out);
    let b = sink.bursts();
    assert_eq!(b.len(), 4);
    assert!(
        b.iter().all(|x| x.ended),
        "{:?}",
        b.iter().map(|x| x.ended).collect::<Vec<_>>()
    );
    assert!(
        (b[2].centre - (DIAL + 1e3)).abs() < 1e-3,
        "after the retune the carrier follows the dial"
    );
    assert!(!fields(&rx).transmitting);
}

#[test]
fn nothing_goes_on_air_with_the_transmitter_off() {
    let (mut rx, sink) = station(RATE, &[("demod", "nfm")], tone_1k);
    rx.set_transmit(false);
    for (key, args) in [("ptt", vec![("on", Value::Bool(true))]), ("tune", vec![])] {
        assert!(call(&mut rx, key, &args).is_err(), "{key} is refused");
    }
    crate::common::set(&mut rx, "demod", "cw").unwrap();
    assert!(call(&mut rx, "send", &[("text", Value::Text("CQ".into()))]).is_err());
    idle(&mut rx, RATE, 1.0);
    assert!(sink.is_empty());
    assert!(!fields(&rx).transmitting);
    // The commands declare what they send and what stops them, so the engine refuses them while its switch is off.
    let defs = Owner::commands(&rx);
    for (key, stop) in [("ptt", "ptt"), ("send", "halt"), ("tune", "halt")] {
        let d = defs.def(defs.id(key).unwrap()).unwrap();
        assert!(d.transmits.is_some(), "{key}");
        assert_eq!(d.stopped_by.as_deref(), Some(stop), "{key}");
    }
    assert!(defs
        .def(defs.id("halt").unwrap())
        .unwrap()
        .transmits
        .is_none());
}

#[test]
fn wfm_and_a_wrong_mode_are_refused() {
    let (mut rx, sink) = station(RATE, &[("demod", "wfm")], tone_1k);
    assert!(ptt(&mut rx, true).is_err());
    assert!(call(&mut rx, "tune", &[]).is_err());
    crate::common::set(&mut rx, "demod", "usb").unwrap();
    assert!(call(&mut rx, "send", &[("text", Value::Text("CQ".into()))]).is_err());
    idle(&mut rx, RATE, 0.5);
    assert!(sink.is_empty());
}

#[test]
fn the_transmitter_keeps_the_pace_at_two_megasamples_and_its_cost() {
    let rate = 2e6;
    let (mut rx, sink) = station(rate, &[("demod", "nfm")], speechlike);
    let piece = 20_011;
    let zeros = vec![Complex32::default(); piece];
    let mut out = Output::default();
    let passes = (3.0 * rate / piece as f64) as usize;
    let mut timed = |rx: &mut GeneralReceiver| {
        let start = std::time::Instant::now();
        for _ in 0..passes {
            out.clear();
            rx.feed(&zeros, &mut out);
        }
        start.elapsed().as_secs_f64() / (passes * piece) as f64 * rate
    };
    let receive = timed(&mut rx);
    ptt(&mut rx, true).unwrap();
    let both = timed(&mut rx);
    let sent: usize = sink.writes.lock().unwrap().iter().map(|w| w.0.len()).sum();
    assert_eq!(sent, passes * piece, "as many samples out as in");
    eprintln!(
        "at 2 Msps: receive {:.1} %, transmit {:.1} % of a core (this build)",
        100.0 * receive,
        100.0 * (both - receive)
    );
}

fn two_tones(t: f64) -> f32 {
    (0.1 * (TAU * 500.0 * t).sin() + 0.1 * (TAU * 2500.0 * t).sin()) as f32
}

#[test]
fn nfm_pre_emphasis_lifts_the_treble_that_de_emphasis_takes_back() {
    // The level at 2500 Hz against 500 Hz in the audio of the receive chain, dB.
    let tilt = |pre: &str, de: &str| {
        let (b, _) = talk("nfm", &[("tx_preemphasis", pre)], two_tones, 3.0);
        let audio = receive(&b, RATE, &[("demod", "nfm"), ("deemph_nfm", de)]);
        let tail = &audio[audio.len() - 60_000..audio.len() - 24_000];
        20.0 * (crate::common::tone(tail, 2500.0) / crate::common::tone(tail, 500.0)).log10()
    };
    let flat = tilt("off", "off");
    let both = tilt("750", "750");
    let lifted = tilt("750", "off");
    eprintln!("2500 against 500 Hz: {flat:.1} dB flat, {both:.1} dB with both, {lifted:.1} dB with pre-emphasis only");
    assert!((both - flat).abs() < 1.0, "{both} against {flat}");
    // A 750 us pre-emphasis lifts 2500 Hz against 500 Hz by |1 + j w2 tau| / |1 + j w1 tau|: 13.2 dB.
    let lift = |f: f64| (1.0 + (TAU * f * 750e-6).powi(2)).sqrt();
    let want = 20.0 * (lift(2500.0) / lift(500.0)).log10();
    assert!(
        (lifted - flat - want).abs() < 1.0,
        "{lifted} against {flat}, want {want} more"
    );
}

#[test]
fn the_ctcss_tone_is_heard_by_the_receive_chain() {
    use sdrkit_stack_general::typed::GeneralFields;
    let (b, _) = talk("nfm", &[("tx_ctcss", "88.5")], tone_1k, 4.0);
    let (_, rx) = receive_with(&b, RATE, &[("demod", "nfm")]);
    let f = GeneralFields::read(Owner::results(&rx).fields()).expect("the fields");
    let got = f.ctcss_hz.map(|h| h.0);
    assert_eq!(got, Some(88.5), "{:?}", f.ctcss_measured_hz);
    let (b, _) = talk("nfm", &[], tone_1k, 4.0);
    let (_, rx) = receive_with(&b, RATE, &[("demod", "nfm")]);
    let f = GeneralFields::read(Owner::results(&rx).fields()).expect("the fields");
    assert_eq!(f.ctcss_hz, None, "no tone set, none heard");
}

/// On a live radio whose host times bursts, push-to-talk states its start once, on the first sample of the
/// transmission, the lead and at most two passes after the clock; the samples are those of a transmission without a
/// stated start, so no audio is cut.
#[test]
fn push_to_talk_on_a_live_radio_states_its_start_on_its_first_sample() {
    let lead = Duration::from_millis(300);
    let run = |lead: Option<Duration>| {
        let (mut rx, sink) = station_on(RATE, &[("demod", "nfm")], tone_1k, lead);
        idle(&mut rx, RATE, 0.2);
        ptt(&mut rx, true).unwrap();
        idle(&mut rx, RATE, 1.0);
        ptt(&mut rx, false).unwrap();
        idle(&mut rx, RATE, 0.3);
        let mut b = sink.bursts();
        assert_eq!(b.len(), 1, "one transmission");
        b.remove(0)
    };
    let timed = run(Some(lead));
    let plain = run(None);
    assert!(plain.starts.is_empty(), "no start without a lead");
    assert_eq!(timed.starts.len(), 1, "one start");
    let (at, utc) = timed.starts[0];
    assert_eq!(at, 0, "the start is on the first sample");
    let earliest = NOW + lead.as_secs_f64();
    assert!(
        utc >= earliest && utc <= earliest + 2.0 * 0.1 + 1e-6,
        "the start is the lead and at most two passes after the clock: {}",
        utc - NOW
    );
    assert!(timed.ended);
    assert_eq!(timed.samples, plain.samples, "the same transmission");
}

/// FNV-1a over the bits of `words`.
fn fnv(words: impl IntoIterator<Item = u32>) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for w in words {
        for b in w.to_le_bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The samples and meters of the speech processor of each voice stack for fixed speech: 3 s of speech, 1 s 20 dB over,
/// 1 s of silence and 3 s 20 dB under, so that the level control and the limiters work down, hold and up.
#[test]
fn the_speech_processor_keeps_its_digests() {
    use sdrkit_algo_sim::voice::Voice;
    use sdrkit_stack_general::transmit::settings::{Channel, SsbMethod, TxSettings};
    use sdrkit_stack_general::transmit::voice::VoiceChain;
    use sdrkit_stack_general::Demod;
    let speech = |level: f64, seconds: f64| {
        let mut v = Voice::new(48e3, level, sdrkit_algo_sim::rng::Rng::new(7)).unwrap();
        let mut x = vec![0f32; (seconds * 48e3) as usize];
        v.fill(&mut x);
        x
    };
    let mut mic = speech(0.1, 3.0);
    mic.extend(speech(1.0, 1.0));
    mic.resize(mic.len() + 48_000, 0.0);
    mic.extend(speech(0.01, 3.0));
    let ch = |demod: Demod| Channel {
        demod,
        width: demod.default_width(),
        offset: 0.0,
    };
    let cases = [
        (ch(Demod::Nfm), TxSettings::default()),
        (
            ch(Demod::Nfm),
            TxSettings {
                ctcss: 88.5,
                ..TxSettings::default()
            },
        ),
        (ch(Demod::Am), TxSettings::default()),
        (
            ch(Demod::Usb),
            TxSettings {
                compression: 10.0,
                ..TxSettings::default()
            },
        ),
        (
            ch(Demod::Lsb),
            TxSettings {
                ssb_method: SsbMethod::Phasing,
                compression: 0.0,
                ..TxSettings::default()
            },
        ),
    ];
    let mut got = Vec::new();
    for (c, s) in cases {
        let mut chain = VoiceChain::new(&c, &s).expect("a voice stack");
        let mut out = vec![Complex32::default(); 480];
        let mut words = Vec::new();
        for piece in mic.chunks(480) {
            chain.process(piece, &mut out[..piece.len()]);
            for z in &out[..piece.len()] {
                words.extend([z.re.to_bits(), z.im.to_bits()]);
            }
            let m = chain.take_meters();
            words.extend([m.peak.to_bits(), m.level_db.to_bits(), m.limit_db.to_bits()]);
        }
        got.push(fnv(words));
    }
    eprintln!("speech processor digests {got:#018x?}");
    assert_eq!(
        got,
        [
            0x082942ea4011e7e3,
            0x84ffe9758e935af5,
            0xdeaded84fd73ef88,
            0xbf05a05fb8863b1f,
            0x42a2fb26006db965,
        ]
    );
}

/// The samples at the transmit stream of one push-to-talk transmission for fixed audio in USB, LSB, AM and NFM: the rise,
/// 1.5 s of speech-like audio, the fall and the end mark, at a session rate that is not the rate of the speech processor.
#[test]
fn the_transmission_keeps_its_digests() {
    let mut got = Vec::new();
    for demod in ["usb", "lsb", "am", "nfm"] {
        let (mut rx, sink) = station(250e3, &[("demod", demod)], speechlike);
        ptt(&mut rx, true).unwrap();
        idle(&mut rx, 250e3, 1.5);
        ptt(&mut rx, false).unwrap();
        idle(&mut rx, 250e3, 0.5);
        let bursts = sink.bursts();
        assert_eq!(bursts.len(), 1, "{demod}");
        assert!(bursts[0].ended, "{demod}");
        got.push(fnv(bursts[0]
            .samples
            .iter()
            .flat_map(|z| [z.re.to_bits(), z.im.to_bits()])));
    }
    eprintln!("transmission digests {got:#018x?}");
    assert_eq!(
        got,
        [
            0x0ed2fd277b76b68f,
            0xae46adfe32c1130f,
            0x22ba0ac63ebaf55f,
            0xaeef33b41924bfa5,
        ]
    );
}
