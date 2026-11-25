//! A session asks for host audio while its sender keeps it, whatever command started the transmission: the FM
//! broadcast sender and the TV sender after `start`, until `stop`. A sender that takes no host audio never asks. The
//! host of these tests writes audio only while the session asks for it, and the tone of the audio is on the air.
//!
//! The fake radio plays its transmit samples at the sample rate, as a radio does, so a sender must make its samples in
//! real time. The tests run one after the other: no sender of another test takes the cores of the one that runs. On a
//! machine too busy for a sender to make its samples in real time, the engine ends the burst as starved: the tests
//! then check that report and the samples the radio got before the end, not the tone.

mod common;

use common::{defaults, field, number, wait_for};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Complex32, Value, ValueKind, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
use sdrkit_core::Event;
use sdrkit_host_engine::{
    Engine, Input, OwnerRef, RadioId, SessionId, BURST_REFUSED, BURST_SAMPLES, BURST_SENT,
    TRANSMIT, TRANSMIT_ALLOWED,
};
use std::f64::consts::TAU;
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: f64 = 20.0;

/// The tests of this file run one after the other.
static ALONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The field `session.audio_in_wanted` of session `s`.
fn wanted(e: &Engine, s: SessionId) -> bool {
    field(e, OwnerRef::Session(s), "session.audio_in_wanted") == Some(Value::Bool(true))
}

/// True if session `s` asks for host audio at some time within `seconds`.
fn ever_wanted(e: &Engine, s: SessionId, seconds: f64) -> bool {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < seconds {
        if wanted(e, s) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Runs command `key` of session `s` with its default arguments, every yes-or-no argument set to yes and every empty
/// text set to a short text. True if the stack accepted it.
fn invoke(e: &Engine, s: SessionId, key: &str) -> bool {
    e.owner(OwnerRef::Session(s), |o| {
        let id = o.commands().id(key).ok()?;
        let mut args = o.commands().def(id)?.new_args();
        let fields: Vec<(String, ValueKind)> = args
            .decl()
            .fields()
            .iter()
            .map(|f| (f.key.to_string(), f.kind))
            .collect();
        for (k, kind) in fields {
            let a = args.decl().id(&k)?;
            let v = match kind {
                ValueKind::Bool => Value::Bool(true),
                ValueKind::Text => Value::Text("TEST".into()),
                _ => continue,
            };
            let _ = args.set_value(a, &v);
        }
        sdrkit_core::owner::invoke(o, id, &args).ok()
    })
    .unwrap()
    .is_some()
}

/// Runs `f` with an engine of every stack, a fake radio that transmits, the transmit switch on, and a session of
/// stack `stack` on the radio, with the settings a sender needs to start.
fn with_stack(stack: &str, f: impl FnOnce(&Engine, &FakeIiod, RadioId, SessionId)) {
    let _alone = ALONE.lock().unwrap_or_else(|p| p.into_inner());
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        IiodOpts {
            tx_plays: true,
            lo_range: "[100000 1 6000000000]",
            ..IiodOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    e.owner(OwnerRef::Engine, |o| {
        let id = o.schema().id(TRANSMIT_ALLOWED.key()).unwrap();
        sdrkit_core::owner::apply(o, id, &Value::Bool(true))
    })
    .unwrap()
    .unwrap();
    let d = sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == stack)
        .unwrap();
    let values = Values::defaults(&(d.settings)());
    let s = e
        .start_stack(d.id, None, &values, Input::Radio(r), None)
        .unwrap();
    // The graph runs: the session publishes its fields.
    wait_for(T, || {
        field(&e, OwnerRef::Session(s), "session.audio_in_lost")
    })
    .expect("the session runs");
    for (key, v) in [("callsign", "VK2TST"), ("tx_pi", "1234")] {
        let _ = e.owner(OwnerRef::Session(s), |o| {
            let id = o.schema().id(key).ok()?;
            sdrkit_core::owner::apply(o, id, &Value::Text(v.into())).ok()
        });
    }
    f(&e, &srv, r, s);
}

/// The transmit samples that the fake radio got.
fn sent(srv: &FakeIiod) -> Vec<Complex32> {
    let raw = srv.board.tx_bytes.lock().unwrap();
    raw.as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            Complex32::new(
                f32::from(i16::from_le_bytes([b[0], b[1]])),
                f32::from(i16::from_le_bytes([b[2], b[3]])),
            )
        })
        .collect()
}

/// The frequency of the carrier `offset_hz` from the centre of `x` at `rate`, sample by sample at about 200 kHz, in
/// radians per sample, with the rate of the result.
fn carrier_frequency(x: &[Complex32], rate: f64, offset_hz: f64) -> (Vec<f32>, f64) {
    let step = ((rate / 200e3) as usize).max(1);
    let low: Vec<Complex32> = x
        .chunks_exact(step)
        .enumerate()
        .map(|(c, part)| {
            part.iter()
                .enumerate()
                .map(|(k, z)| {
                    let (s, co) = (-TAU * offset_hz * (c * step + k) as f64 / rate).sin_cos();
                    *z * Complex32::new(co as f32, s as f32)
                })
                .sum()
        })
        .collect();
    let f = low
        .windows(2)
        .map(|w| (w[1] * w[0].conj()).arg())
        .collect();
    (f, rate / step as f64)
}

/// The power of the tone `hz` in `x` at `rate`, without the mean of `x`.
fn tone_power(x: &[f32], rate: f64, hz: f64) -> f64 {
    let mean = x.iter().map(|v| f64::from(*v)).sum::<f64>() / x.len() as f64;
    let w = TAU * hz / rate;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let p = w * i as f64;
        re += (f64::from(*v) - mean) * p.cos();
        im -= (f64::from(*v) - mean) * p.sin();
    }
    (re * re + im * im) / (x.len() as f64).powi(2)
}

/// The session asks for no host audio before `start`, asks after it, and stops asking after `stop`. A host that
/// writes a tone of 1 kHz in `channels` channels (the tone on the left only for stereo) while the session asks puts
/// the tone on the carrier `offset_hz` from the centre of the transmission. Returns the seconds from `start` to the
/// first time the session asks.
fn start_asks_and_the_tone_goes_out(
    e: &Engine,
    srv: &FakeIiod,
    r: RadioId,
    s: SessionId,
    channels: u16,
    offset_hz: f64,
) -> f64 {
    assert!(!ever_wanted(e, s, 0.3), "wanted before start");
    let begin = Instant::now();
    assert!(invoke(e, s, "start"), "start refused");
    wait_for(T, || wanted(e, s).then_some(())).expect("wanted after start");
    let delay = begin.elapsed().as_secs_f64();

    // The host writes in real time, 10 ms at a time, only while the session asks.
    let start = Instant::now();
    let mut n = 0_u64;
    while start.elapsed() < Duration::from_millis(1200) {
        assert!(wanted(e, s), "not wanted while the transmission runs");
        let due = (start.elapsed().as_secs_f64() * 48e3) as u64;
        while n + 480 <= due {
            let mut piece = Vec::with_capacity(480 * usize::from(channels));
            for i in n..n + 480 {
                piece.push((0.5 * (TAU * 1000.0 * i as f64 / 48e3).sin()) as f32);
                if channels == 2 {
                    piece.push(0.0);
                }
            }
            e.write_audio(s, &piece, channels, 48e3).unwrap();
            n += 480;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(invoke(e, s, "stop"), "stop refused");
    wait_for(T, || (!wanted(e, s)).then_some(())).expect("not wanted after stop");
    assert!(!ever_wanted(e, s, 0.3), "wanted again after stop");
    assert_eq!(number(e, OwnerRef::Session(s), "session.audio_in_lost"), 0.0);

    // The transmission is one burst, which ends at `stop`, or before if the sender wrote it too slowly.
    let mut events = Vec::new();
    let burst = wait_for(T, || {
        e.take_events(OwnerRef::Session(s), &mut events).unwrap();
        events.iter().find(|ev| ev.is(&TRANSMIT)).cloned()
    })
    .expect("the transmission ended");
    let rate = number(e, OwnerRef::Radio(r), "rate_hz");
    if burst.fields.get(&BURST_SENT) == Some(false) {
        the_starved_burst_is_reported(e, srv, s, &burst, rate);
        return delay;
    }

    // The radio got samples of the transmission. A sender of a wide signal on a loaded machine makes fewer than the
    // time on the air asks for.
    let iq = wait_for(T, || {
        let iq = sent(srv);
        (iq.len() as f64 > 0.35 * rate).then_some(iq)
    })
    .unwrap_or_else(|| panic!("the radio got {} samples at {rate} Hz", sent(srv).len()));
    // The tone is on the carrier in some tenth of a second of them: the audio starts after the session asked.
    let piece = (0.1 * rate) as usize;
    let mut best = 0.0_f64;
    for part in iq.chunks_exact(piece) {
        let (freq, fs) = carrier_frequency(part, rate, offset_hz);
        let tone = tone_power(&freq, fs, 1000.0);
        let other = [400.0, 700.0, 1300.0, 2300.0]
            .iter()
            .map(|hz| tone_power(&freq, fs, *hz))
            .fold(f64::MIN_POSITIVE, f64::max);
        best = best.max(tone / other);
    }
    assert!(best > 30.0, "1 kHz is {best:.1} times the other tones");
    delay
}

/// Checks the report of `burst`, which failed, of session `s` at `rate`: the only failure allowed is a sender too slow
/// for real time. The burst counts as failed, its reason gives the time on the air that its samples fill, and the
/// radio got no signal after those samples.
fn the_starved_burst_is_reported(
    e: &Engine,
    srv: &FakeIiod,
    s: SessionId,
    burst: &Event,
    rate: f64,
) {
    let why = burst.fields.get(&BURST_REFUSED).unwrap_or_default();
    let after_s: f64 = why
        .strip_prefix("the stack wrote the burst too slowly: the radio ran out of samples ")
        .and_then(|rest| rest.strip_suffix(" s into the burst, which was ended"))
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("the burst failed: {why}"));
    println!("starved after {after_s} s: the sender was too slow for real time");
    assert_eq!(number(e, OwnerRef::Session(s), "session.tx_failed"), 1.0);
    assert_eq!(number(e, OwnerRef::Session(s), "session.tx_sent"), 0.0);
    let samples = burst.fields.get(&BURST_SAMPLES).unwrap();
    assert!(samples > 0, "a burst on the air without samples");
    assert!(
        (after_s - samples as f64 / rate).abs() <= 5e-4,
        "{samples} samples at {rate} Hz, ran out after {after_s} s"
    );
    // The transmitter drops what it still holds when it goes off the air: the radio may get none of the samples.
    let signal = sent(srv).iter().filter(|z| z.norm_sqr() > 0.0).count();
    assert!(
        signal as u64 <= samples.unsigned_abs(),
        "the radio got {signal} samples of signal, the stack wrote {samples}"
    );
}

#[test]
fn the_fm_sender_asks_for_host_audio_from_start_to_stop_and_sends_it() {
    with_stack("fm", |e, srv, r, s| {
        let delay = start_asks_and_the_tone_goes_out(e, srv, r, s, 2, 0.0);
        println!("fm: asks {:.0} ms after start", delay * 1e3);
    });
}

#[test]
fn the_tv_sender_asks_for_host_audio_from_start_to_stop_and_sends_it() {
    with_stack("atv", |e, srv, r, s| {
        // The sound carrier of B/G is 5.5 MHz above the vision carrier, which is 2.75 MHz below the centre.
        let delay = start_asks_and_the_tone_goes_out(e, srv, r, s, 1, 2.75e6);
        println!("atv: asks {:.0} ms after start", delay * 1e3);
    });
}

#[test]
fn a_sender_without_host_audio_never_asks() {
    with_stack("dvbt", |e, _, _, s| {
        assert!(!ever_wanted(e, s, 0.3), "wanted before start");
        let started = invoke(e, s, "start");
        println!("dvbt: start accepted: {started}");
        assert!(!ever_wanted(e, s, 0.8), "wanted while dvbt sends");
        invoke(e, s, "stop");
        assert!(!ever_wanted(e, s, 0.3), "wanted after stop");
    });
}
