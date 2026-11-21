//! `run --tx-audio`: the feeder gives the WAV audio to a session of the general stack on a fake board that transmits,
//! only while push-to-talk asks for it, and the speech goes on the air in full.

use sdrkit_cli::txaudio::{Feeder, TxAudio};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Facilities, MemStorage, Value, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{
    Engine, Input, OwnerRef, SessionId, Setup, AUDIO_IN_LOST, AUDIO_IN_WANTED, BURST_SAMPLES,
    BURST_SENT, TRANSMIT, TRANSMIT_ALLOWED,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn engine(net: &FakeTransport) -> Arc<Engine> {
    let facilities = Facilities {
        storage: Arc::new(MemStorage::new()),
        clock: sdrkit_host_engine::system_clock(),
        random: sdrkit_host_engine::system_random(),
        codecs: Arc::default(),
    };
    let components = sdrkit_host_catalog::registry(&facilities);
    Arc::new(
        Engine::new(Setup {
            facilities,
            transport: Arc::new(net.clone()),
            stacks: sdrkit_host_catalog::all(),
            families: sdrkit_host_catalog::families(),
            components,
        })
        .expect("an engine"),
    )
}

/// Waits up to 20 s for `f` to give something.
fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(20) {
        if let Some(v) = f() {
            return Some(v);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

fn ptt(e: &Engine, s: SessionId, on: bool) {
    e.owner(OwnerRef::Session(s), |o| {
        let id = o.commands().id("ptt").unwrap();
        let mut args = o.commands().def(id).unwrap().new_args();
        let a = args.decl().id("on").unwrap();
        args.set_value(a, &Value::Bool(on)).unwrap();
        sdrkit_core::owner::invoke(o, id, &args)
    })
    .unwrap()
    .unwrap();
}

/// A fake board that transmits, an engine on it with the transmit switch at `allowed`, and a session of the general
/// stack on the board.
fn general_on_board(allowed: bool) -> (FakeIiod, Arc<Engine>, SessionId) {
    let net = FakeTransport::new();
    let board = FakeIiod::start_on(
        net.clone(),
        FakeOpts {
            tx_plays: true,
            ..FakeOpts::default()
        },
    );
    let e = engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let defaults = e.owner(kind, |o| Values::defaults(o.schema())).unwrap();
    let r = e.open_radio("iiod", &board.addr(), &defaults).unwrap();
    e.set_text(
        OwnerRef::Engine,
        TRANSMIT_ALLOWED.key(),
        if allowed { "true" } else { "false" },
    )
    .unwrap();
    let d = sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == "general")
        .unwrap();
    let s = e
        .start_stack(
            d.id,
            None,
            &Values::defaults(&(d.settings)()),
            Input::Radio(r),
            None,
        )
        .unwrap();
    (board, e, s)
}

/// Two seconds of a tone at 16 kHz.
fn tone() -> Arc<TxAudio> {
    Arc::new(TxAudio {
        samples: (0..32_000)
            .map(|i| (0.25 * (std::f64::consts::TAU * 800.0 * f64::from(i) / 16e3).sin()) as f32)
            .collect(),
        rate_hz: 16e3,
    })
}

#[test]
fn with_the_switch_off_push_to_talk_is_refused_and_nothing_is_sent() {
    let (board, e, s) = general_on_board(false);
    let feeder = Feeder::start(&e, s, tone()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let refused = e
        .owner(OwnerRef::Session(s), |o| {
            let id = o.commands().id("ptt").unwrap();
            let mut args = o.commands().def(id).unwrap().new_args();
            let a = args.decl().id("on").unwrap();
            args.set_value(a, &Value::Bool(true)).unwrap();
            sdrkit_core::owner::invoke(o, id, &args)
        })
        .unwrap();
    assert!(
        matches!(refused, Err(sdrkit_core::CommandError::TransmitOff)),
        "{refused:?}"
    );
    std::thread::sleep(Duration::from_millis(500));
    let wanted = e
        .owner(OwnerRef::Session(s), |o| {
            o.results().fields().get(&AUDIO_IN_WANTED)
        })
        .unwrap();
    drop(feeder);
    assert_eq!(wanted, Some(false));
    assert!(
        board.board.tx_bytes.lock().unwrap().is_empty(),
        "the board got samples"
    );
}

#[test]
fn the_wav_file_goes_out_while_push_to_talk_is_on() {
    let (board, e, s) = general_on_board(true);
    // The tone is longer than push-to-talk is on.
    let feeder = Feeder::start(&e, s, tone()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    ptt(&e, s, true);
    // The level of the microphone input of the transmitter shows that the audio came.
    let mic = wait_for(|| {
        let db = e
            .owner(OwnerRef::Session(s), |o| {
                let res = o.results();
                let id = res.decl().field_id("tx_mic_dbfs")?;
                match res.fields().value(id)? {
                    Value::F64(db) => Some(*db),
                    _ => None,
                }
            })
            .ok()
            .flatten()?;
        (db > -20.0).then_some(db)
    });
    std::thread::sleep(Duration::from_millis(500));
    ptt(&e, s, false);
    assert!(mic.is_some(), "the microphone input stays quiet");
    let mut events = Vec::new();
    let burst = wait_for(|| {
        e.take_events(OwnerRef::Session(s), &mut events).unwrap();
        events.iter().find(|ev| ev.is(&TRANSMIT)).cloned()
    })
    .expect("the transmission ended");
    drop(feeder);
    assert_eq!(burst.fields.get(&BURST_SENT), Some(true));
    assert!(burst.fields.get(&BURST_SAMPLES).unwrap_or(0) > 0);
    let lost = e
        .owner(OwnerRef::Session(s), |o| {
            o.results().fields().get(&AUDIO_IN_LOST)
        })
        .unwrap();
    assert_eq!(lost, Some(0), "host audio lost");
    assert_eq!(
        board
            .board
            .tx_gap_in_signal
            .load(std::sync::atomic::Ordering::Relaxed),
        0,
        "samples of gap inside the transmission"
    );
}
