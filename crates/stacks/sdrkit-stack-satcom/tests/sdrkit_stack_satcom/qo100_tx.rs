//! The voice sender of the qo100 stack: the sent signal is upper sideband with the carrier and the other sideband
//! suppressed, its occupied bandwidth is at most 2.7 kHz, the uplink frequency is the dial less the translation of the transponder, the
//! level follows `tx_level`, `ptt` is refused with the transmit setting off, and `ptt` off ends with the fall and the end mark.

use crate::common::command_with;
use sdrkit_algo_dsp::spectrum::{welch, Scaling};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, StackConfig, Value, Values};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_stack_satcom::modes::qo100::{self, tx};
use sdrkit_stack_satcom::typed::Qo100TxFields;
use std::f64::consts::TAU;
use std::sync::{Arc, Mutex};

/// The session rate.
const RATE: f64 = 2e6;
/// The centre of the radio: the narrowband transponder through a 9750 MHz LNB.
const CENTRE: f64 = 740.1e6;

/// The audio of the host: `shape(t)` at 48 kHz, as much as the test lets through.
struct Mic {
    shape: fn(f64) -> f32,
    n: u64,
    owed: usize,
}

/// The host: the microphone, and each write to the transmit stream with its centre and its end mark.
struct Host {
    mic: Mutex<Mic>,
    writes: Mutex<Vec<(Vec<Complex32>, f64, bool)>>,
}

impl HostStreams for Host {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                centre_hz,
                end,
                ..
            },
        ) = (port, data)
        {
            self.writes
                .lock()
                .unwrap()
                .push((samples.to_vec(), centre_hz, end));
        }
    }
    fn read(&self, port: HostPort, out: &mut [f32]) -> usize {
        if port != HostPort::Microphone {
            return 0;
        }
        let mut m = self.mic.lock().unwrap();
        let k = m.owed.min(out.len());
        for o in &mut out[..k] {
            *o = (m.shape)(m.n as f64 / 48_000.0);
            m.n += 1;
        }
        m.owed -= k;
        k
    }
}

/// One transmission: its samples, its centre and whether its end mark came.
struct Burst {
    samples: Vec<Complex32>,
    centre: f64,
    ended: bool,
}

impl Host {
    fn bursts(&self) -> Vec<Burst> {
        let mut out: Vec<Burst> = Vec::new();
        let mut open = false;
        for (s, c, end) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push(Burst {
                    samples: Vec::new(),
                    centre: *c,
                    ended: false,
                });
            }
            let b = out.last_mut().unwrap();
            b.samples.extend_from_slice(s);
            b.ended = *end;
            open = !end;
        }
        out
    }
}

fn tone_1k(t: f64) -> f32 {
    (0.25 * (TAU * 1000.0 * t).sin()) as f32
}

/// Speech-like audio that fills the audio band and beyond: tones from 100 Hz to 5 kHz at fixed phases.
fn wide(t: f64) -> f32 {
    let s: f64 = (1..=50)
        .map(|k| (TAU * 100.0 * f64::from(k) * t + 0.7 * f64::from(k * k)).sin())
        .sum();
    (0.05 * s) as f32
}

/// The stack with `set`, the microphone fed with `shape`, the engine's transmit setting as `transmit`, and its host.
fn station(
    set: &[(&str, Value)],
    shape: fn(f64) -> f32,
    transmit: bool,
) -> (Box<dyn Receiver>, Arc<Host>) {
    let schema = (qo100::DESCRIPTOR.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in set {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    let mut rx = (qo100::DESCRIPTOR.make_receiver)(&StackConfig::new(s)).unwrap();
    rx.configure(&InputInfo::new(RATE, CENTRE, CENTRE).recording())
        .unwrap();
    let host = Arc::new(Host {
        mic: Mutex::new(Mic {
            shape,
            n: 0,
            owed: 0,
        }),
        writes: Mutex::new(Vec::new()),
    });
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(host.clone());
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(transmit);
    (rx, host)
}

fn ptt(rx: &mut dyn Receiver, on: bool) -> Result<(), sdrkit_core::CommandError> {
    command_with(rx, "ptt", &[("on", Value::Bool(on))])
}

/// Feeds `seconds` of silence in pieces of 0.1 s, with the audio of the host for each piece.
fn idle(rx: &mut dyn Receiver, host: &Host, seconds: f64) {
    let zeros = vec![Complex32::default(); (RATE / 10.0) as usize];
    let mut out = Output::default();
    for _ in 0..(seconds * 10.0).round() as usize {
        host.mic.lock().unwrap().owed += 4800;
        out.clear();
        rx.feed(&zeros, &mut out);
    }
}

fn fields(rx: &dyn Receiver) -> Qo100TxFields {
    Qo100TxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// One transmission of `seconds` of `shape` with the settings `set`.
fn transmission(set: &[(&str, Value)], shape: fn(f64) -> f32, seconds: f64) -> Burst {
    let (mut rx, host) = station(set, shape, true);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, seconds);
    assert!(fields(rx.as_ref()).transmitting);
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.5);
    assert!(!fields(rx.as_ref()).transmitting);
    let mut bursts = host.bursts();
    assert_eq!(bursts.len(), 1);
    bursts.pop().unwrap()
}

/// The amplitude of the complex tone at `hz` in `x` at [`RATE`], under a Hann window.
fn tone(x: &[Complex32], hz: f64) -> f64 {
    let n = x.len() as f64;
    let (mut re, mut im, mut sum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, z) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (TAU * i as f64 / n).cos();
        let (s, c) = (TAU * hz * i as f64 / RATE).sin_cos();
        re += w * (f64::from(z.re) * c + f64::from(z.im) * s);
        im += w * (f64::from(z.im) * c - f64::from(z.re) * s);
        sum += w;
    }
    re.hypot(im) / sum
}

#[test]
fn the_voice_is_upper_sideband_with_the_carrier_suppressed() {
    let b = transmission(&[], tone_1k, 2.0);
    assert!(b.ended);
    // The second after the level control settled.
    let x = &b.samples[(0.9 * RATE) as usize..(1.9 * RATE) as usize];
    let upper = tone(x, 1000.0);
    let lower = 20.0 * (tone(x, -1000.0) / upper).log10();
    let carrier = 20.0 * (tone(x, 0.0) / upper).log10();
    eprintln!(
        "qo100 voice: upper {upper:.4}, lower sideband {lower:.1} dB, carrier {carrier:.1} dB"
    );
    assert!(upper > 0.05, "{upper}");
    assert!(lower < -40.0, "the lower sideband is {lower} dB");
    assert!(carrier < -40.0, "the carrier is {carrier} dB");
}

#[test]
fn the_occupied_bandwidth_is_at_most_2700_hz_above_the_uplink_dial() {
    let b = transmission(&[], wide, 3.0);
    let n = 65_536;
    let mut p = vec![0.0f32; n];
    welch(
        &b.samples[(0.5 * RATE) as usize..(2.9 * RATE) as usize],
        n,
        Window::BlackmanHarris4,
        0.5,
        Scaling::Power,
        &mut p,
    )
    .unwrap();
    let bin = RATE / n as f64;
    let hz = |i: usize| (i as f64 - (n / 2) as f64) * bin;
    let total: f64 = p.iter().map(|v| f64::from(*v)).sum();
    // The band that holds 99 % of the power: 0.5 % cut from each end.
    let mut acc = 0.0;
    let mut lo = 0;
    while acc + f64::from(p[lo]) < 0.005 * total {
        acc += f64::from(p[lo]);
        lo += 1;
    }
    let mut acc = 0.0;
    let mut hi = n - 1;
    while acc + f64::from(p[hi]) < 0.005 * total {
        acc += f64::from(p[hi]);
        hi -= 1;
    }
    let (lo, hi) = (hz(lo), hz(hi));
    let above: f64 = p
        .iter()
        .enumerate()
        .filter(|(i, _)| hz(*i) > 0.0 && hz(*i) < 2700.0)
        .map(|(_, v)| f64::from(*v))
        .sum();
    eprintln!(
        "qo100 voice: 99 % of the power from {lo:.0} to {hi:.0} Hz ({:.0} Hz), {:.2} % within 0 to 2700 Hz",
        hi - lo,
        100.0 * above / total
    );
    assert!(hi - lo <= 2700.0, "{lo} to {hi}");
    // All of it is above the dial, and the audio above 2700 Hz is in the skirt of the sideband filter.
    assert!(lo > 0.0 && hi < 3100.0, "{lo} to {hi}");
    assert!(above > 0.95 * total);
}

#[test]
fn the_uplink_is_the_dial_less_the_translation() {
    assert_eq!(tx::TRANSLATION_HZ, 8089.5e6);
    for (dial, uplink) in [
        (10_489.650e6, 2400.150e6),
        (10_489.550e6, 2400.050e6),
        (10_489.800e6, 2400.300e6),
    ] {
        let b = transmission(&[("dial_hz", Value::Hz(dial))], tone_1k, 0.3);
        assert!((b.centre - uplink).abs() < 0.5, "{dial}: {}", b.centre);
        assert!((tx::uplink_hz(dial) - uplink).abs() < 0.5);
    }
    // A new dial moves the uplink of a running stack, and the receive side keeps the same dial.
    let (mut rx, host) = station(&[], tone_1k, true);
    assert!((fields(rx.as_ref()).tx_freq_hz.0 - 2400.1e6).abs() < 0.5);
    crate::common::set(rx.as_mut(), "dial_hz", serde_json::json!(10_489.7e6)).unwrap();
    idle(rx.as_mut(), &host, 0.5);
    assert!((fields(rx.as_ref()).tx_freq_hz.0 - 2400.2e6).abs() < 0.5);
    assert!(
        (crate::common::number(
            Owner::results(rx.as_ref())
                .fields()
                .value(
                    Owner::results(rx.as_ref())
                        .decl()
                        .field_id("dial_hz")
                        .unwrap()
                )
                .unwrap()
        )
        .unwrap()
            - 10_489.7e6)
            .abs()
            < 0.5
    );
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 0.2);
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.3);
    assert!((host.bursts()[0].centre - 2400.2e6).abs() < 0.5);
}

#[test]
fn the_peak_follows_tx_level() {
    let peak = |b: &Burst| b.samples.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
    let low = peak(&transmission(&[("tx_level", Value::F64(0.2))], wide, 1.5));
    let high = peak(&transmission(&[("tx_level", Value::F64(0.8))], wide, 1.5));
    eprintln!("qo100 voice: peak {low:.4} at tx_level 0.2, {high:.4} at 0.8");
    assert!(low <= 0.2 * 1.02 && low > 0.1, "{low}");
    assert!(high <= 0.8 * 1.02, "{high}");
    assert!((high / low - 4.0).abs() < 0.01, "{}", high / low);
}

#[test]
fn ptt_is_refused_with_the_transmit_setting_off() {
    let (mut rx, host) = station(&[], tone_1k, false);
    let e = ptt(rx.as_mut(), true).unwrap_err();
    assert!(e.to_string().contains("transmitter is off"), "{e}");
    idle(rx.as_mut(), &host, 0.3);
    assert!(!fields(rx.as_ref()).transmitting);
    assert!(host.bursts().is_empty());
    // The setting going off during a transmission ends it at once, with the end mark.
    let (mut rx, host) = station(&[], tone_1k, true);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 0.3);
    rx.set_transmit(false);
    idle(rx.as_mut(), &host, 0.2);
    let b = host.bursts();
    assert!(b.len() == 1 && b[0].ended);
    assert!(!fields(rx.as_ref()).transmitting);
}

#[test]
fn ptt_off_ends_with_the_fall_and_the_end_mark() {
    let (mut rx, host) = station(&[], tone_1k, true);
    ptt(rx.as_mut(), true).unwrap();
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 1.0);
    let f = fields(rx.as_ref());
    assert!(f.transmitting && f.tx_time_s.is_some() && f.tx_mic_dbfs.is_some());
    ptt(rx.as_mut(), false).unwrap();
    idle(rx.as_mut(), &host, 0.3);
    let f = fields(rx.as_ref());
    assert!(!f.transmitting && f.tx_time_s.is_none());
    let bursts = host.bursts();
    assert_eq!(bursts.len(), 1);
    let b = &bursts[0];
    assert!(b.ended);
    // One second on air, the 5 ms fall and at most two blocks of 5 ms that were under way.
    let seconds = b.samples.len() as f64 / RATE;
    assert!((1.0..1.03).contains(&seconds), "{seconds}");
    let mean = |from_ms: f64, to_ms: f64| {
        let n = b.samples.len();
        let a = n - (from_ms * 1e-3 * RATE) as usize;
        let z = n - (to_ms * 1e-3 * RATE) as usize;
        b.samples[a..z]
            .iter()
            .map(|z| f64::from(z.norm()))
            .sum::<f64>()
            / (z - a) as f64
    };
    let (before, falling, last) = (mean(30.0, 20.0), mean(6.0, 4.0), mean(1.0, 0.0));
    eprintln!("qo100 voice: fall {before:.4} -> {falling:.4} -> {last:.5}");
    assert!(before > 0.05 && falling < before && last < 0.05 * before);
    // The time-out ends a transmission in the same way.
    let (mut rx, host) = station(&[("tx_timeout", Value::Duration(1.0))], tone_1k, true);
    ptt(rx.as_mut(), true).unwrap();
    idle(rx.as_mut(), &host, 1.5);
    assert!(!fields(rx.as_ref()).transmitting);
    let b = host.bursts();
    assert!(b.len() == 1 && b[0].ended);
    assert!((b[0].samples.len() as f64 / RATE - 1.0).abs() < 0.03);
}
