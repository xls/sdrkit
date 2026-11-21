//! The sender of the FM stack: the worked RDS examples of the specification bit for bit, the stack's own receiver on the
//! sent signal (RDS content, the text A/B change, the clock group, stereo separation and distortion), the peak deviation
//! under full-scale and overdriven audio, the pilot and the RDS spectrum measured from the samples, and the refusals. The
//! transmit stream is reached as the engine reaches it: the graph, with the host streams in its context. The PI codes are
//! artificial (0x1234, and 0x4A21 of the specification's examples).

use super::common::*;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    owner, Clock, CommandError, Complex32, ConfigError, InputInfo, Output, Owner, Receiver,
    StackConfig, Values,
};
use sdrkit_dev_test::measure;
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Component, ComponentInfo, Format, Graph, Io, Port, StackGraph};
use sdrkit_stack_fm::rds::block::BlockLayer;
use sdrkit_stack_fm::rds::encode::{clock_group, group_words, RdsContent, RdsEncoder};
use sdrkit_stack_fm::transmit::FmTxError;
use sdrkit_stack_fm::typed::FmTxFields;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

/// The carrier frequency of the tests.
const FREQ: f64 = 100e6;
/// 2026-11-19 12:34:00 UTC, the time of the specification's clock example.
const CLOCK_EXAMPLE: f64 = 1_795_091_640.0;

/// The transmit stream: each write's samples and its end mark.
#[derive(Default)]
struct Sink {
    writes: Mutex<Vec<(Vec<Complex32>, bool)>>,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (HostPort::Transmit, HostData::Iq { samples, end, .. }) = (port, data) {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
        }
    }
}

impl Sink {
    /// Every sample sent, and whether the last write carried the end mark.
    fn sent(&self) -> (Vec<Complex32>, bool) {
        let w = self.writes.lock().unwrap();
        let x = w.iter().flat_map(|(s, _)| s.iter().copied()).collect();
        (x, w.last().is_some_and(|(_, e)| *e))
    }
}

/// Stereo audio at 48 kHz from a buffer, as many frames as the session's samples bring time.
struct TestAudio {
    rate: f64,
    owed: f64,
    at: usize,
    frames: Vec<[f32; 2]>,
}

impl Component for TestAudio {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.audio", vec![Port::iq("in")], vec![Port::audio("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        Ok(vec![Format::audio(48_000.0, 2)])
    }
    fn process(&mut self, io: &mut Io) {
        self.owed += io.input_len(0) as f64 * 48_000.0 / self.rate;
        let m = self.owed.floor();
        self.owed -= m;
        let out = &mut io.audio_out(0).samples;
        for _ in 0..m as usize {
            out.extend(self.frames.get(self.at).copied().unwrap_or([0.0; 2]));
            self.at += 1;
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// `seconds` of 48 kHz stereo from `f(t)`.
fn audio(seconds: f64, f: impl Fn(f64) -> [f32; 2]) -> Vec<[f32; 2]> {
    (0..(seconds * 48e3) as usize)
        .map(|i| f(i as f64 / 48e3))
        .collect()
}

/// A sending stack at `rate` on [`FREQ`] with `settings` and the audio `frames`, the transmitter on, the clock at
/// `clock`, and its stream.
fn station(
    rate: f64,
    settings: &[(&str, &str)],
    frames: Vec<[f32; 2]>,
    clock: f64,
) -> (StackGraph, Arc<Sink>) {
    let a = TestAudio {
        rate: 0.0,
        owed: 0.0,
        at: 0,
        frames,
    };
    let cfg = StackConfig::new(values(settings));
    let mut rx = sdrkit_stack_fm::graph::receiver_with_audio(&cfg, Box::new(a)).expect("builds");
    rx.set_transmit(true);
    rx.configure(&InputInfo::new(rate, FREQ, FREQ).recording())
        .unwrap();
    let sink = Arc::new(Sink::default());
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    let mut ctx = g.context().clone();
    ctx.streams = Some(sink.clone());
    ctx.clock = Clock::fixed(clock);
    g.set_context(ctx);
    assert!(rx.transmits());
    (rx, sink)
}

/// Invokes the command `key` of `rx`.
fn command(rx: &mut StackGraph, key: &str) -> Result<(), CommandError> {
    let id = Owner::commands(rx).id(key).unwrap();
    let a = Owner::commands(rx).def(id).unwrap().new_args();
    owner::invoke(rx, id, &a).map(|_| ())
}

/// Feeds `seconds` of silence in pieces of 0.1 s.
fn idle(rx: &mut StackGraph, rate: f64, seconds: f64) {
    let zeros = vec![Complex32::default(); (rate / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

fn fields(rx: &StackGraph) -> FmTxFields {
    FmTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// One transmission of `seconds` at `rate` with `settings` and `frames`: the samples sent and the largest peak deviation
/// the sender reported.
fn send(rate: f64, settings: &[(&str, &str)], frames: Vec<[f32; 2]>, seconds: f64) -> (Vec<Complex32>, f64) {
    let (mut rx, sink) = station(rate, settings, frames, CLOCK_EXAMPLE);
    command(&mut rx, "start").expect("starts");
    let mut reported = 0.0f64;
    for _ in 0..(seconds * 10.0).round() as usize {
        idle(&mut rx, rate, 0.1);
        let f = fields(&rx);
        assert!(f.transmitting);
        reported = reported.max(f.tx_peak_hz.map_or(0.0, |h| h.0));
    }
    command(&mut rx, "stop").unwrap();
    idle(&mut rx, rate, 0.2);
    assert!(!fields(&rx).transmitting);
    let (x, ended) = sink.sent();
    assert!(ended, "the transmission has its end mark");
    (x, reported)
}

/// The instantaneous frequency of `x` from its phase steps, Hz.
fn frequency(x: &[Complex32], rate: f64) -> Vec<f32> {
    x.windows(2)
        .map(|w| ((w[1] * w[0].conj()).arg() as f64 * rate / TAU) as f32)
        .collect()
}

/// The phase and amplitude of the tone at `hz` in the real signal `x`.
fn phasor(x: &[f32], rate: f64, hz: f64) -> (f64, f64) {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let p = TAU * hz * i as f64 / rate;
        re += f64::from(*v) * p.cos();
        im -= f64::from(*v) * p.sin();
    }
    let n = x.len() as f64;
    (im.atan2(re), 2.0 * re.hypot(im) / n)
}

#[test]
fn the_worked_examples_match_bit_for_bit() {
    let content = RdsContent {
        pi: 0x4A21,
        ps: "SDRKIT".into(),
        rt: "Test text".into(),
        pty: 10,
        tp: true,
        music: true,
        stereo: true,
        ..Default::default()
    };
    let mut e = RdsEncoder::new(content.clone());
    let g: Vec<[u16; 4]> = (0..5).map(|_| e.next_group(None)).collect();
    // The blocks and the 26-bit words as the specification lists them (1.8, worked examples 1 and 2).
    let examples: [([u16; 4], [u32; 4]); 3] = [
        (
            [0x4A21, 0x0548, 0xCDCD, 0x5344],
            [0x1288784, 0x0152100, 0x33736B3, 0x14D1256],
        ),
        (
            [0x4A21, 0x054F, 0xCDCD, 0x2020],
            [0x1288784, 0x0153C96, 0x33736B3, 0x08080DC],
        ),
        (
            [0x4A21, 0x2540, 0x5465, 0x7374],
            [0x1288784, 0x09501AC, 0x15197E0, 0x1CDD081],
        ),
    ];
    for (got, (blocks, words)) in [g[0], g[3], g[4]].iter().zip(examples) {
        assert_eq!(*got, blocks);
        assert_eq!(group_words(*got), words);
    }
    // Worked example 3: group 4A for 2026-11-19 12:34 UTC, offset 0, made as the minute changes.
    let clock = clock_group(0x4A21, (1 << 10) | (10 << 5), CLOCK_EXAMPLE, 0);
    assert_eq!(clock, [0x4A21, 0x4541, 0xDF66, 0xC880]);
    assert_eq!(
        group_words(clock),
        [0x1288784, 0x115055D, 0x37D9B61, 0x3220013]
    );
    let mut e = RdsEncoder::new(RdsContent {
        clock_offset: Some(0),
        ..content.clone()
    });
    assert_eq!(e.next_group(Some(CLOCK_EXAMPLE - 0.05))[1] >> 12, 0);
    assert_eq!(e.next_group(Some(CLOCK_EXAMPLE + 0.04)), clock);
    // Block A as bits, and as sent after differential coding from a previous bit of 0.
    let mut e = RdsEncoder::new(content);
    let sent: String = (0..26)
        .map(|_| if e.next_bit(None) { '1' } else { '0' })
        .collect();
    assert_eq!(format!("{:026b}", 0x1288784), "01001010001000011110000100");
    assert_eq!(sent, "01110011110000010100000111");
    // The receiver's block layer takes every block of the examples with no correction.
    let mut layer = BlockLayer::new();
    let mut groups = Vec::new();
    for _ in 0..3 {
        for (_, words) in examples {
            for w in words {
                for b in (0..26).rev() {
                    groups.extend(layer.push(w >> b & 1 == 1, 1.0));
                }
            }
        }
    }
    let clean: Vec<_> = groups
        .iter()
        .filter(|g| g.blocks.iter().all(|b| b.is_some_and(|b| !b.corrected)))
        .map(|g| g.blocks.map(|b| b.unwrap().data))
        .collect();
    assert!(clean.len() >= 6, "{} groups", clean.len());
    for g in &clean {
        assert!(examples.iter().any(|(b, _)| b == g), "{g:04X?}");
    }
    assert_eq!(layer.stats().corrected, 0);
}

#[test]
fn the_stack_hears_what_it_sends() {
    let rate = 2e6;
    // The clock is 0.5 s before a minute starts: the clock group comes once it starts.
    let (mut tx, sink) = station(
        rate,
        &[
            ("tx_pi", "1234"),
            ("tx_ps", "TEST FM"),
            ("tx_rt", "Test text"),
            ("tx_pty", "10"),
            ("tx_tp", "1"),
            ("tx_ta", "0"),
            ("tx_music", "1"),
            ("tx_ct", "1"),
            ("tx_ct_offset", "4"),
        ],
        audio(7.0, |t| [(0.5 * (TAU * 1000.0 * t).sin()) as f32, 0.0]),
        CLOCK_EXAMPLE - 0.5,
    );
    command(&mut tx, "start").expect("starts");
    idle(&mut tx, rate, 4.0);
    // A new text: the A/B flag turns over and the receiver shows it within two text cycles.
    set(&mut tx, "tx_rt", "New text").unwrap();
    idle(&mut tx, rate, 3.0);
    assert!(fields(&tx).tx_groups > 70);
    command(&mut tx, "stop").unwrap();
    idle(&mut tx, rate, 0.2);
    let (x, ended) = sink.sent();
    assert!(ended);
    let mut rx = receiver(rate, FREQ, &[("rds_variant", "rds")]);
    let mut at = 0;
    let run = run_with(&mut rx, rate, 7.0, |buf, n| {
        let e = (at + n).min(x.len());
        buf.extend_from_slice(&x[at..e]);
        buf.resize(n, Complex32::default());
        at = e;
    });
    let early = &run.reports[39];
    assert_eq!(early.station, "TEST FM ");
    assert_eq!(early.text, "Test text");
    assert_eq!(early.pi, 0x1234);
    assert_eq!(early.pty, Some(10));
    assert!(early.traffic && !early.ta && early.music);
    assert!(early.stereo && early.pilot_locked, "the stereo pilot");
    assert!((early.pilot_pct - 9.0).abs() < 0.5, "pilot {}", early.pilot_pct);
    // The clock group: the minute that started, and 4 half hours of local offset.
    assert_eq!(early.clock, Some(CLOCK_EXAMPLE));
    assert_eq!(early.clock_offset_s, Some(7200.0));
    // A text cycle of "New text" is 3 groups of text in 15 groups, 1.3 s: two cycles after the change.
    let changed = run
        .first(|r| r.text == "New text")
        .expect("the new text is shown");
    assert!(changed > 4.0 && changed <= 4.0 + 2.0 * 1.32 + 0.2, "{changed}");
    // The sound of 2 to 3.5 s: 1 kHz on the left only.
    let (l, r) = run.tail(5.0);
    let (l, r) = (&l[..72_000], &r[..72_000]);
    let a = amp(l, 1000.0);
    let b = amp(r, 1000.0);
    let separation = 20.0 * (a / b).log10();
    // The audio part is what the pilot (0.09) and RDS (2/75) leave; the receiver gives 0.8 for 0.9 of deviation.
    let want = 0.5 * 0.8 / 0.9 * (1.0 - 0.09 - 2.0 / 75.0);
    let level_db = 20.0 * (a / want).log10();
    let rest = (measure::rms(l).powi(2) - a * a / 2.0).max(0.0).sqrt();
    let thd_n = rest / (a / 2f64.sqrt()) * 100.0;
    eprintln!("level {level_db:.2} dB, separation {separation:.1} dB, THD+N {thd_n:.3} %");
    assert!(level_db.abs() < 0.5, "level {level_db} dB");
    assert!(separation > 30.0, "separation {separation} dB");
    assert!(thd_n < 1.0, "THD+N {thd_n} %");
}

#[test]
fn the_deviation_never_passes_the_set_peak() {
    let rate = 1e6;
    let mut rng = Rng::new(3);
    let mut noise = |gain: f32| {
        let mut x = audio(2.0, |_| [0.0; 2]);
        for f in &mut x {
            *f = [
                gain * (2.0 * rng.uniform_f32() - 1.0),
                gain * (2.0 * rng.uniform_f32() - 1.0),
            ];
        }
        x
    };
    let square = audio(2.0, |t| {
        let s = if (TAU * 3000.0 * t).sin() >= 0.0 { 4.0 } else { -4.0 };
        [s, -s]
    });
    let cases = [
        ("full-scale noise", noise(1.0), "1"),
        ("noise 12 dB over full scale", noise(4.0), "1"),
        ("square wave 12 dB over full scale", square, "1"),
        ("noise 12 dB over, limiter off", noise(4.0), "0"),
    ];
    for (what, frames, limiter) in cases {
        let (x, reported) = send(
            rate,
            &[("tx_pi", "1234"), ("tx_ps", "TEST FM"), ("tx_limiter", limiter)],
            frames,
            2.0,
        );
        let f = frequency(&x, rate);
        let peak = f.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        eprintln!("{what}: peak {peak:.0} Hz, reported {reported:.0} Hz");
        assert!(peak <= 75_000.5, "{what}: {peak} Hz");
        assert!(reported <= 75_000.5, "{what}: reported {reported} Hz");
        if what.contains("over") {
            // The audio part peaks with the difference subcarrier, where the pilot is at 71 % of its peak: about 73 kHz.
            assert!(peak > 70_000.0, "{what}: the peak {peak} Hz is not near the budget");
        }
        // The magnitude never passes 0.9 of full scale.
        assert!(x.iter().all(|z| z.norm() <= 0.9 + 1e-6));
    }
}

#[test]
fn the_pilot_and_the_rds_spectrum_are_measured_from_the_samples() {
    let rate = 1e6;
    let (x, _) = send(
        rate,
        &[("tx_pi", "1234"), ("tx_ps", "TEST FM"), ("tx_rt", "Test text")],
        Vec::new(),
        2.2,
    );
    let mpx = frequency(&x[(0.1 * rate) as usize..(2.1 * rate) as usize], rate);
    // The pilot: its level from one second, its frequency from the phase it gains from one second to the next.
    let half = mpx.len() / 2;
    let (p0, a0) = phasor(&mpx[..half], rate, 19_000.0);
    let (p1, _) = phasor(&mpx[half..], rate, 19_000.0);
    let drift = (p1 - p0 + TAU * 19_000.0 * half as f64 / rate).rem_euclid(TAU);
    let off_hz = (if drift > std::f64::consts::PI { drift - TAU } else { drift }) / TAU;
    let level = a0 / 75_000.0 * 100.0;
    eprintln!("pilot {level:.3} %, {off_hz:+.4} Hz off 19 kHz");
    assert!((level - 9.0).abs() < 0.3, "pilot level {level} %");
    assert!(off_hz.abs() < 2.0, "pilot {off_hz} Hz off");
    // RDS: the power more than 2.4 kHz from 57 kHz against the power within 2.4 kHz.
    let n = 16_384;
    let z: Vec<Complex32> = mpx.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    let mut p = vec![0f32; n];
    welch(&z, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
    let bin = rate / n as f64;
    let band = |lo: f64, hi: f64| -> f64 {
        p.iter()
            .enumerate()
            .map(|(i, v)| ((i as f64 - (n / 2) as f64) * bin, f64::from(*v)))
            .filter(|(f, _)| *f >= lo && *f < hi)
            .map(|(_, v)| v)
            .sum()
    };
    let inside = band(54_600.0, 59_400.0);
    let outside = band(48_000.0, 54_600.0) + band(59_400.0, 66_000.0);
    let ratio = 10.0 * (inside / outside).log10();
    eprintln!("RDS outside ±2.4 kHz: {ratio:.1} dB down");
    assert!(ratio >= 40.0, "{ratio} dB");
}

#[test]
fn the_transmission_needs_a_pi_code_and_the_transmitter() {
    let (mut rx, sink) = station(1e6, &[("tx_ps", "TEST FM")], Vec::new(), CLOCK_EXAMPLE);
    let why = match command(&mut rx, "start") {
        Err(CommandError::Failed { source }) => source.downcast_ref::<FmTxError>().cloned(),
        other => panic!("{other:?}"),
    };
    assert_eq!(why, Some(FmTxError::NoPi));
    assert_eq!(why.unwrap().setting(), Some("tx_pi"));
    idle(&mut rx, 1e6, 0.3);
    assert!(sink.sent().0.is_empty(), "nothing on air");
    // With the PI code it starts; with the transmitter off it does not.
    let (mut rx, _) = station(1e6, &[("tx_pi", "1234")], Vec::new(), CLOCK_EXAMPLE);
    command(&mut rx, "start").expect("starts");
    rx.set_transmit(false);
    assert!(matches!(command(&mut rx, "start"), Err(CommandError::TransmitOff)));
}

/// A host with audio at 48 kHz for the host audio input of the graph, and the transmit stream. `frames` holds stereo
/// frames; a host that is not `stereo` has only their left channel, as mono audio.
#[derive(Default)]
struct AudioHost {
    sink: Sink,
    stereo: bool,
    frames: Mutex<Vec<[f32; 2]>>,
}

impl HostStreams for AudioHost {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        self.sink.write(port, data);
    }
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        assert_eq!(port, HostPort::Microphone);
        let mut frames = self.frames.lock().unwrap();
        let n = frames.len().min(out.len());
        for (o, f) in out.iter_mut().zip(frames.drain(..n)) {
            *o = if self.stereo { (f[0] + f[1]) / 2.0 } else { f[0] };
        }
        n
    }
    fn read_channels(&self, port: HostPort, channels: u16, out: &mut [f32]) -> usize {
        if !(self.stereo && channels == 2) {
            // The mono audio in every channel, as a host without stereo audio gives it.
            let c = usize::from(channels);
            let mut mono = vec![0.0; out.len() / c];
            let n = self.read(port, &mut mono);
            for (o, s) in out.chunks_mut(c).zip(&mono[..n]) {
                o.fill(*s);
            }
            return n * c;
        }
        let mut frames = self.frames.lock().unwrap();
        let n = frames.len().min(out.len() / 2);
        for (o, f) in out.chunks_mut(2).zip(frames.drain(..n)) {
            o.copy_from_slice(&f);
        }
        2 * n
    }
}

/// The power of the multiplex of a 2 s transmission of the FM stack with its own host audio input, in the band of the
/// difference signal that a 1 kHz tone makes (36.5 to 39.5 kHz), Hz squared. The host gives a 1 kHz tone on the left
/// alone, as stereo audio or as mono audio.
fn difference_band_power(stereo: bool) -> f64 {
    let rate = 1e6;
    let cfg = StackConfig::new(values(&[("tx_pi", "1234")]));
    let mut rx = sdrkit_stack_fm::graph::receiver(&cfg).expect("builds");
    rx.set_transmit(true);
    rx.configure(&InputInfo::new(rate, FREQ, FREQ).recording())
        .unwrap();
    let host = Arc::new(AudioHost {
        stereo,
        ..AudioHost::default()
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    let mut ctx = g.context().clone();
    ctx.streams = Some(host.clone());
    ctx.clock = Clock::fixed(CLOCK_EXAMPLE);
    g.set_context(ctx);
    command(&mut rx, "start").expect("starts");
    let tone = audio(2.2, |t| [(0.5 * (TAU * 1000.0 * t).sin()) as f32, 0.0]);
    for piece in tone.chunks(4800) {
        host.frames.lock().unwrap().extend_from_slice(piece);
        idle(&mut rx, rate, 0.1);
    }
    command(&mut rx, "stop").unwrap();
    idle(&mut rx, rate, 0.2);
    let (x, ended) = host.sink.sent();
    assert!(ended, "the transmission has its end mark");
    let mpx = frequency(&x[(0.3 * rate) as usize..(2.1 * rate) as usize], rate);
    let n = 16_384;
    let z: Vec<Complex32> = mpx.iter().map(|&v| Complex32::new(v, 0.0)).collect();
    let mut p = vec![0f32; n];
    welch(&z, n, Window::BlackmanHarris4, 0.5, Scaling::Power, &mut p).unwrap();
    let bin = rate / n as f64;
    p.iter()
        .enumerate()
        .map(|(i, v)| ((i as f64 - (n / 2) as f64) * bin, f64::from(*v)))
        .filter(|(f, _)| (36_500.0..39_500.0).contains(f))
        .map(|(_, v)| v)
        .sum()
}

#[test]
fn stereo_audio_of_the_host_is_sent_as_stereo() {
    let stereo = difference_band_power(true);
    let mono = difference_band_power(false);
    let db = 10.0 * (stereo / mono).log10();
    eprintln!("difference band: stereo {stereo:.3e}, mono {mono:.3e} Hz^2, {db:.1} dB apart");
    // Half of the 0.5 tone is the difference signal: two sidebands of about 8 kHz deviation each.
    assert!(stereo > 1e7, "difference band with stereo audio: {stereo} Hz^2");
    assert!(db > 40.0, "stereo audio is {db} dB above mono audio in the difference band");
}
