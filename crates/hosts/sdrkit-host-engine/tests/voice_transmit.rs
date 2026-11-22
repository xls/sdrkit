//! Push-to-talk through the engine: audio that the host writes to a session of the general stack goes out of the
//! transmitter of its radio as narrow FM with the tone of the audio.

mod common;

use common::*;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Clock, Complex32, Facilities, MemStorage, Random, Value, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts as IiodOpts};
use sdrkit_host_engine::{
    Engine, Input, OwnerRef, RadioId, SessionId, Setup, BURST_ATTENUATION, BURST_SAMPLES,
    BURST_SENT, BURST_TX_POWER, TRANSMIT_ALLOWED,
};
use std::f64::consts::TAU;
use std::sync::Arc;
use std::time::{Duration, Instant};

const T: f64 = 20.0;

fn engine(net: &FakeTransport) -> Engine {
    let mut components = sdrkit_graph::Registry::standard();
    sdrkit_stack_general::register(&mut components);
    // The clock runs: a burst with a stated start goes to the radio when its time comes.
    let t0 = Instant::now();
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::new(move || 1.7e9 + t0.elapsed().as_secs_f64()),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net.clone()),
        stacks: sdrkit_stack_general::descriptors(),
        families: vec![sdrkit_stack_general::typed::family()],
        components,
    })
    .unwrap()
}

fn set(e: &Engine, r: OwnerRef, key: &str, v: Value) {
    e.owner(r, |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &v)
    })
    .unwrap()
    .unwrap();
}

/// Presses or releases push-to-talk of session `s`.
fn ptt(e: &Engine, s: OwnerRef, on: bool) {
    e.owner(s, |o| {
        let id = o.commands().id("ptt").unwrap();
        let mut args = o.commands().def(id).unwrap().new_args();
        let a = args.decl().id("on").unwrap();
        args.set_value(a, &Value::Bool(on)).unwrap();
        sdrkit_core::owner::invoke(o, id, &args)
    })
    .unwrap()
    .unwrap();
}

/// The power of the tone `hz` in `x` at `rate`, by the Goertzel sum.
fn tone_power(x: &[f32], rate: f64, hz: f64) -> f64 {
    let w = TAU * hz / rate;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, v) in x.iter().enumerate() {
        let p = w * i as f64;
        re += f64::from(*v) * p.cos();
        im -= f64::from(*v) * p.sin();
    }
    (re * re + im * im) / (x.len() as f64).powi(2)
}

/// Runs `f` with an engine, a fake board that transmits, the transmit switch on, and a session of the general stack on
/// the board that has asked for host audio at 48 kHz.
fn with_general(f: impl FnOnce(&Engine, &FakeIiod, RadioId, SessionId)) {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        IiodOpts {
            tx_plays: true,
            ..IiodOpts::default()
        },
    );
    let e = engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let r = e
        .open_radio("iiod", &srv.addr(), &defaults(&e, kind))
        .unwrap();
    set(
        &e,
        OwnerRef::Engine,
        TRANSMIT_ALLOWED.key(),
        Value::Bool(true),
    );
    set(&e, OwnerRef::Radio(r), "tx_power", Value::F64(-10.0));
    let d = &sdrkit_stack_general::DESCRIPTOR;
    let values = Values::defaults(&(d.settings)());
    let s = e
        .start_stack(d.id, None, &values, Input::Radio(r), None)
        .unwrap();
    let rate = wait_for(T, || {
        match field(&e, OwnerRef::Session(s), "session.audio_in_rate_hz") {
            Some(Value::Hz(hz)) => Some(hz),
            _ => None,
        }
    })
    .expect("the session asks for host audio");
    assert_eq!(rate, 48e3);
    f(&e, &srv, r, s);
}

/// The field `session.audio_in_wanted` of session `s`.
fn wanted(e: &Engine, s: SessionId) -> bool {
    match field(e, OwnerRef::Session(s), "session.audio_in_wanted") {
        Some(Value::Bool(b)) => b,
        other => panic!("session.audio_in_wanted is {other:?}"),
    }
}

/// Waits until the field `session.audio_in_wanted` of `s` is `want`.
fn wait_wanted(e: &Engine, s: SessionId, want: bool, what: &str) {
    let got = wait_for(T, || (wanted(e, s) == want).then_some(()));
    assert!(got.is_some(), "{what}: audio_in_wanted stays {}", !want);
}

/// True if the field `session.audio_in_wanted` of `s` is true at some time within `seconds`.
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

/// Runs the command `key` of session `s` with the text argument `text`, if given.
fn command(e: &Engine, s: OwnerRef, key: &str, text: Option<&str>) {
    e.owner(s, |o| {
        let id = o.commands().id(key).unwrap();
        let mut args = o.commands().def(id).unwrap().new_args();
        if let Some(t) = text {
            let a = args.decl().id("text").unwrap();
            args.set_value(a, &Value::Text(t.into())).unwrap();
        }
        sdrkit_core::owner::invoke(o, id, &args)
    })
    .unwrap()
    .unwrap_or_else(|e| panic!("{key}: {e}"));
}

#[test]
fn the_session_wants_host_audio_only_while_push_to_talk_runs() {
    with_general(|e, _, _, s| {
        let session = OwnerRef::Session(s);
        assert!(!ever_wanted(e, s, 0.3), "wanted before push-to-talk");
        ptt(e, session, true);
        wait_wanted(e, s, true, "push-to-talk on");
        // It stays true while the speech runs, past the pass that armed it.
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(500) {
            assert!(wanted(e, s), "not wanted while push-to-talk is on");
            std::thread::sleep(Duration::from_millis(5));
        }
        ptt(e, session, false);
        wait_wanted(e, s, false, "push-to-talk off");
        // A release with nothing running asks for no audio for longer than one pass.
        ptt(e, session, false);
        wait_wanted(e, s, false, "a release while idle");
        assert!(!ever_wanted(e, s, 0.3), "wanted after a release while idle");
        // Morse code and the tune carrier send no host audio.
        e.set_text(session, "demod", "cw").unwrap();
        command(e, session, "send", Some("E"));
        assert!(!ever_wanted(e, s, 0.3), "wanted while Morse code is sent");
        command(e, session, "halt", None);
        command(e, session, "tune", None);
        assert!(!ever_wanted(e, s, 0.3), "wanted while the carrier is sent");
        command(e, session, "halt", None);
        e.set_text(session, "demod", "nfm").unwrap();
        // Halt ends speech too.
        ptt(e, session, true);
        wait_wanted(e, s, true, "push-to-talk on again");
        command(e, session, "halt", None);
        wait_wanted(e, s, false, "halt");
        // The transmit switch going off ends speech too.
        ptt(e, session, true);
        wait_wanted(e, s, true, "push-to-talk on a third time");
        set(
            e,
            OwnerRef::Engine,
            TRANSMIT_ALLOWED.key(),
            Value::Bool(false),
        );
        wait_wanted(e, s, false, "the transmit switch off");
    });
}

#[test]
fn host_audio_with_ptt_on_goes_out_as_narrow_fm_with_its_tone() {
    with_general(host_audio_goes_out);
}

fn host_audio_goes_out(e: &Engine, srv: &FakeIiod, r: RadioId, s: SessionId) {
    let session = OwnerRef::Session(s);

    // The host writes a tone of 1 kHz in real time, 10 ms at a time, for 1.5 s with the key down.
    ptt(e, session, true);
    let start = Instant::now();
    let mut n = 0_u64;
    while start.elapsed() < Duration::from_millis(1500) {
        let due = (start.elapsed().as_secs_f64() * 48e3) as u64;
        while n + 480 <= due {
            let piece: Vec<f32> = (n..n + 480)
                .map(|i| (0.25 * (TAU * 1000.0 * i as f64 / 48e3).sin()) as f32)
                .collect();
            e.write_audio(s, &piece, 1, 48e3).unwrap();
            n += 480;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    ptt(e, session, false);

    let mut events = Vec::new();
    let burst = wait_for(T, || {
        e.take_events(session, &mut events).unwrap();
        events
            .iter()
            .find(|ev| ev.is(&sdrkit_host_engine::TRANSMIT))
            .cloned()
    })
    .expect("the transmission ended");
    assert_eq!(
        burst.fields.get(&BURST_SENT),
        Some(true),
        "{:?}",
        field(e, session, "session.problem")
    );
    assert_eq!(number(e, session, "session.tx_failed"), 0.0);
    // The power of the burst is the radio setting; the attenuation is its negative.
    assert_eq!(burst.fields.get(&BURST_TX_POWER), Some(-10.0));
    assert_eq!(burst.fields.get(&BURST_ATTENUATION), Some(10.0));
    // The radio never ran out of samples inside the transmission.
    let gap = srv
        .board
        .tx_gap_in_signal
        .load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(gap, 0, "samples of gap inside the transmission");
    assert_eq!(number(e, session, "session.audio_in_lost"), 0.0);
    let samples = burst.fields.get(&BURST_SAMPLES).unwrap() as f64;
    let tx_rate = number(e, OwnerRef::Radio(r), "rate_hz");
    assert!(tx_rate > 0.0);
    let seconds = samples / tx_rate;
    assert!((1.4..2.0).contains(&seconds), "{seconds} s on the air");

    // What the radio got: the frequency of the carrier follows the tone.
    let raw = srv.board.tx_bytes.lock().unwrap().clone();
    let iq: Vec<Complex32> = raw
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| {
            Complex32::new(
                f32::from(i16::from_le_bytes([b[0], b[1]])),
                f32::from(i16::from_le_bytes([b[2], b[3]])),
            )
        })
        .collect();
    assert!(iq.len() as f64 >= samples);
    // The middle second of the transmission.
    let from = (0.3 * tx_rate) as usize;
    let part = &iq[from..from + (0.8 * tx_rate) as usize];
    let freq: Vec<f32> = part
        .windows(2)
        .map(|w| (w[1] * w[0].conj()).arg())
        .collect();
    let at = |hz: f64| tone_power(&freq, tx_rate, hz);
    let tone = at(1000.0);
    for other in [400.0, 700.0, 1300.0, 2000.0, 3000.0] {
        assert!(
            tone > 100.0 * at(other),
            "1 kHz {tone:e}, {other} Hz {:e}",
            at(other)
        );
    }
    // The peak deviation of the tone is that of narrow FM: some kHz.
    let dev_hz = (2.0 * tone).sqrt() * tx_rate / TAU;
    assert!((500.0..6000.0).contains(&dev_hz), "deviation {dev_hz} Hz");
}
