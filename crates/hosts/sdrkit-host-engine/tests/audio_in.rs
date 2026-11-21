//! The audio that a host gives to a session ([`Engine::write_audio`]) reaches the host audio input (`host.in`) of the
//! graph of the session, converted to 48 kHz and to the channel count of that input; what the graph does not take in
//! time is counted; the call never waits.
//! A session that cannot transmit does not ask for host audio, also with a host audio input.

mod common;

use common::{field, number, wait_for};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{
    Clock, Complex32, ConfigError, Facilities, MemStorage, Random, StackDescriptor, Value, Values,
};
use sdrkit_graph::components::{HostIn, HOST_IN_RATE};
use sdrkit_graph::{Component, ComponentInfo, Format, GraphBuilder, Io, Port};
use sdrkit_host_engine::{
    Engine, EngineError, FeedId, Input, OwnerRef, Samples, SessionId, Setup,
};
use std::sync::{Arc, Mutex};

const T: f64 = 20.0;

/// The audio that each capture stack got, by the id of its stack.
static CAPTURED: Mutex<Vec<(&'static str, Vec<f32>)>> = Mutex::new(Vec::new());

/// Keeps the audio of its input, which has `1` channels, under the name `0`.
struct Capture(&'static str, u16);

impl Component for Capture {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.capture", vec![Port::audio("in")])
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        assert_eq!(inputs[0], Format::audio(HOST_IN_RATE, self.1));
        Ok(Vec::new())
    }
    fn process(&mut self, io: &mut Io) {
        let a = io.audio(0);
        assert_eq!(a.channels, self.1);
        let mut all = CAPTURED.lock().unwrap();
        match all.iter_mut().find(|(id, _)| *id == self.0) {
            Some((_, got)) => got.extend_from_slice(&a.samples),
            None => all.push((self.0, a.samples.to_vec())),
        }
    }
}

/// True once the capture stack `id` ran: its host audio input has asked for audio.
fn ran(id: &str) -> bool {
    CAPTURED.lock().unwrap().iter().any(|(k, _)| *k == id)
}

/// The audio that the capture stack `id` got so far.
fn captured(id: &str) -> Vec<f32> {
    let all = CAPTURED.lock().unwrap();
    all.iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// A stack whose graph is a host audio input of `channels` channels (1 or 2) and a capture under the name `id`.
fn capture_graph(
    id: &'static str,
    channels: u16,
) -> Result<Box<dyn sdrkit_core::Receiver>, ConfigError> {
    let mic = if channels == 2 {
        HostIn::stereo()
    } else {
        HostIn::default()
    };
    let g = GraphBuilder::new()
        .add("mic", mic)
        .add("cap", Capture(id, channels))
        .input("mic.in")
        .connect("mic.out", "cap.in")
        .build()?;
    Ok(Box::new(g))
}

macro_rules! capture_stack {
    ($name:ident, $id:literal) => {
        capture_stack!($name, $id, 1);
    };
    ($name:ident, $id:literal, $channels:literal) => {
        static $name: StackDescriptor = StackDescriptor {
            id: $id,
            submodes: &[],
            signal_types: &[],
            make_receiver: |_| capture_graph($id, $channels),
            benchmarks: None,
            ..common::METER_DESC
        };
    };
}
capture_stack!(CAP48, "cap48");
capture_stack!(CAP44, "cap44");
capture_stack!(CAP16, "cap16");
capture_stack!(CAPLOSS, "caploss");
capture_stack!(CAPRX, "caprx");
capture_stack!(MONO_HALF, "monohalf");
capture_stack!(ST48, "st48", 2);
capture_stack!(ST44, "st44", 2);
capture_stack!(ST_MONO, "stmono", 2);
capture_stack!(ST_CHANGE, "stchange", 2);
capture_stack!(ST_LOSS, "stloss", 2);
capture_stack!(ST_STALE, "ststale", 2);

/// An engine with the capture stacks and the meter.
fn engine() -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![
            &CAP48,
            &CAP44,
            &CAP16,
            &CAPLOSS,
            &CAPRX,
            &MONO_HALF,
            &ST48,
            &ST44,
            &ST_MONO,
            &ST_CHANGE,
            &ST_LOSS,
            &ST_STALE,
            &common::METER,
        ],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// Starts stack `id` on a feed of samples, and waits until its graph ran and took every block given. The session
/// cannot transmit: it takes host audio, but its fields do not ask for it.
fn start(e: &Engine, id: &str) -> (SessionId, FeedId) {
    let feed = e.open_feed(48e3, 100e6);
    let v = common::defaults(e, e.find(&format!("stack.{id}")).unwrap());
    let s = e
        .start_stack(id, None, &v, Input::Feed(feed), None)
        .unwrap();
    let mut pushed = 0.0;
    let started = wait_for(T, || {
        e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
            .unwrap();
        pushed += 480.0;
        ran(id).then_some(())
    });
    assert!(started.is_some(), "the graph did not run");
    let took = wait_for(T, || {
        (number(e, OwnerRef::Session(s), "session.samples") >= pushed).then_some(())
    });
    assert!(took.is_some(), "the graph did not take the samples");
    (s, feed)
}

#[test]
fn a_session_that_cannot_transmit_does_not_ask_for_host_audio() {
    let e = engine();
    let (s, feed) = start(&e, "caprx");
    e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(matches!(
        field(&e, OwnerRef::Session(s), "session.audio_in_rate_hz"),
        None | Some(Value::None)
    ));
    assert_eq!(
        field(&e, OwnerRef::Session(s), "session.audio_in_wanted"),
        Some(Value::Bool(false))
    );
}

/// Audio that no test value repeats: a sweep with a ramp on it.
fn audio(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f32;
            0.5 * (t * 0.01 + t * t * 1e-6).sin() + t * 1e-6
        })
        .collect()
}

/// Writes the mono audio `sent` at `rate` to a session of stack `id` in one call, and returns what its graph got once
/// it has `expect` samples.
fn through(id: &str, sent: &[f32], rate: f64, expect: usize) -> Vec<f32> {
    through_channels(id, sent, 1, rate, expect)
}

/// As [`through`], for interleaved audio of `channels` channels.
fn through_channels(id: &str, sent: &[f32], channels: u16, rate: f64, expect: usize) -> Vec<f32> {
    let e = engine();
    let (s, feed) = start(&e, id);
    assert!(captured(id).is_empty());
    e.write_audio(s, sent, channels, rate).unwrap();
    let got = wait_for(T, || {
        e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
            .unwrap();
        let got = captured(id);
        (got.len() >= expect).then_some(got)
    })
    .unwrap_or_else(|| panic!("{} of {expect} samples came", captured(id).len()));
    assert_eq!(
        number(&e, OwnerRef::Session(s), "session.audio_in_lost"),
        0.0
    );
    got
}

/// What the shared resampler makes of `sent` at `rate`, in the pieces of the engine.
fn converted(sent: &[f32], rate: f64) -> Vec<f32> {
    let mut r = Resampler::<f32>::new(rate, HOST_IN_RATE).unwrap();
    let mut out = Vec::new();
    for piece in sent.chunks(4096) {
        r.process(piece, &mut out);
    }
    out
}

#[test]
fn audio_at_48_khz_reaches_the_port_as_written() {
    let sent = audio(24_000);
    assert_eq!(through("cap48", &sent, 48e3, sent.len()), sent);
}

#[test]
fn audio_at_44_1_khz_reaches_the_port_converted_to_48_khz() {
    let sent = audio(22_050);
    let want = converted(&sent, 44_100.0);
    // Half a second of 44.1 kHz is about half a second of 48 kHz.
    assert!((want.len() as i64 - 24_000).abs() < 200, "{}", want.len());
    assert_eq!(through("cap44", &sent, 44_100.0, want.len()), want);
}

#[test]
fn audio_at_16_khz_reaches_the_port_converted_to_48_khz() {
    let sent = audio(8000);
    let want = converted(&sent, 16e3);
    assert!((want.len() as i64 - 24_000).abs() < 200, "{}", want.len());
    assert_eq!(through("cap16", &sent, 16e3, want.len()), want);
}

#[test]
fn audio_that_the_graph_does_not_take_is_counted_and_the_call_does_not_wait() {
    let e = engine();
    let (s, feed) = start(&e, "caploss");
    // No samples come: the graph takes nothing. The host writes 10 s of audio.
    let piece = audio(480);
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        e.write_audio(s, &piece, 1, 48e3).unwrap();
    }
    let took = t.elapsed();
    assert!(took.as_secs_f64() < 1.0, "1000 calls took {took:?}");
    // The session keeps the newest 32768 samples.
    let lost = wait_for(T, || {
        let n = number(&e, OwnerRef::Session(s), "session.audio_in_lost");
        (n > 0.0).then_some(n)
    });
    assert_eq!(lost, Some(480_000.0 - 32_768.0));
    let got = wait_for(T, || {
        e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
            .unwrap();
        let got = captured("caploss");
        (got.len() >= 32_768).then_some(got)
    })
    .expect("the kept audio comes");
    // 32768 samples are 68 pieces and 128 samples: the kept audio starts 352 samples into a piece.
    assert_eq!(got.len(), 32_768);
    assert_eq!(got[..128], piece[352..]);
    assert_eq!(got[128..608], piece[..]);
}

#[test]
fn a_rate_of_zero_is_refused_and_a_session_without_an_audio_input_takes_none() {
    let e = engine();
    let feed = e.open_feed(48e3, 100e6);
    let v = common::defaults(&e, e.find("stack.meter").unwrap());
    let s = e
        .start_stack("meter", None, &v, Input::Feed(feed), None)
        .unwrap();
    assert!(e.write_audio(s, &[0.0; 16], 1, 0.0).is_err());
    assert!(e.write_audio(s, &[0.0; 16], 1, f64::NAN).is_err());
    for channels in [0, 3] {
        assert!(matches!(
            e.write_audio(s, &[0.0; 12], channels, 48e3),
            Err(EngineError::AudioChannels { got }) if got == channels
        ));
    }
    e.write_audio(s, &audio(100_000), 1, 48e3).unwrap();
    e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(matches!(
        field(&e, OwnerRef::Session(s), "session.audio_in_rate_hz"),
        None | Some(Value::None)
    ));
    assert_eq!(
        number(&e, OwnerRef::Session(s), "session.audio_in_lost"),
        0.0
    );
}

/// Stereo frames with the audio `left` on the left and silence on the right.
fn left_only(left: &[f32]) -> Vec<f32> {
    left.iter().flat_map(|&l| [l, 0.0]).collect()
}

#[test]
fn stereo_audio_reaches_a_stereo_port_with_left_and_right_apart() {
    let sent = left_only(&audio(12_000));
    assert_eq!(through_channels("st48", &sent, 2, 48e3, sent.len()), sent);
}

#[test]
fn stereo_audio_at_44_1_khz_stays_on_its_side_when_converted() {
    let left = audio(11_025);
    let want = converted(&left, 44_100.0);
    let got = through_channels("st44", &left_only(&left), 2, 44_100.0, 2 * want.len());
    assert_eq!(got.len(), 2 * want.len());
    for (f, w) in got.chunks(2).zip(&want) {
        assert!((f[0] - w).abs() < 1e-5, "left {} for {w}", f[0]);
        assert_eq!(f[1], 0.0);
    }
}

#[test]
fn stereo_audio_reaches_a_mono_port_as_the_mean_of_left_and_right() {
    let (left, right) = (audio(12_000), audio(24_000).split_off(12_000));
    let sent: Vec<f32> = left.iter().zip(&right).flat_map(|(&l, &r)| [l, r]).collect();
    let want: Vec<f32> = left.iter().zip(&right).map(|(l, r)| (l + r) * 0.5).collect();
    assert_eq!(through_channels("monohalf", &sent, 2, 48e3, want.len()), want);
}

#[test]
fn mono_audio_reaches_a_stereo_port_in_both_channels() {
    let sent = audio(12_000);
    let want: Vec<f32> = sent.iter().flat_map(|&s| [s, s]).collect();
    assert_eq!(through("stmono", &sent, 48e3, want.len()), want);
}

#[test]
fn the_channel_count_can_change_between_writes() {
    let e = engine();
    let (s, feed) = start(&e, "stchange");
    let takes = |expect: usize| {
        wait_for(T, || {
            e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
                .unwrap();
            let got = captured("stchange");
            (got.len() >= expect).then_some(got)
        })
        .unwrap_or_else(|| panic!("{} of {expect} samples came", captured("stchange").len()))
    };
    // An odd count of mono samples, then stereo with a signal on the left only, then mono again.
    let mono = audio(4801);
    let stereo = left_only(&audio(4800));
    e.write_audio(s, &mono, 1, 48e3).unwrap();
    takes(2 * mono.len());
    e.write_audio(s, &stereo, 2, 48e3).unwrap();
    takes(2 * mono.len() + stereo.len());
    e.write_audio(s, &mono, 1, 48e3).unwrap();
    let got = takes(4 * mono.len() + stereo.len());
    let twice: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
    assert_eq!(got, [&twice[..], &stereo[..], &twice[..]].concat());
    assert_eq!(
        number(&e, OwnerRef::Session(s), "session.audio_in_lost"),
        0.0
    );
}

#[test]
fn stereo_audio_that_was_written_over_keeps_left_and_right_in_place() {
    let e = engine();
    let (s, feed) = start(&e, "stloss");
    // No samples come: the graph takes nothing. One mono sample puts the stereo audio at an odd place of the store,
    // then the host writes 2.5 s of stereo, more than the session keeps.
    e.write_audio(s, &[0.25], 1, 48e3).unwrap();
    let piece = left_only(&[0.5; 240]);
    for _ in 0..1000 {
        e.write_audio(s, &piece, 2, 48e3).unwrap();
    }
    let got = wait_for(T, || {
        e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
            .unwrap();
        let got = captured("stloss");
        (got.len() >= 32_768).then_some(got)
    })
    .expect("the kept audio comes");
    // The session keeps the newest 32768 samples: 16384 whole frames.
    assert_eq!(got.len(), 32_768);
    assert!(got.chunks(2).all(|f| f == [0.5, 0.0]));
}

#[test]
fn audio_of_the_channel_count_before_that_was_not_taken_is_left_out() {
    let e = engine();
    let (s, feed) = start(&e, "ststale");
    // No samples come between the writes: the mono audio still waits when the stereo audio begins.
    let stereo = left_only(&audio(4800));
    e.write_audio(s, &audio(4801), 1, 48e3).unwrap();
    e.write_audio(s, &stereo, 2, 48e3).unwrap();
    let got = wait_for(T, || {
        e.push(feed, Samples::Iq(&[Complex32::default(); 480]))
            .unwrap();
        let got = captured("ststale");
        (got.len() >= stereo.len()).then_some(got)
    })
    .expect("the stereo audio comes");
    assert_eq!(got, stereo);
    let lost = wait_for(T, || {
        let n = number(&e, OwnerRef::Session(s), "session.audio_in_lost");
        (n > 0.0).then_some(n)
    });
    assert_eq!(lost, Some(4801.0));
}
