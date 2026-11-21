//! What a person does with a mesh stack, with the plain values that the data folder has for a country: add a channel without a key, send a text on it,
//! and get a sentence that names the argument when the channel does not exist.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::Value;
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{Engine, OwnerRef, SessionId, BURST_SENT, TRANSMIT};
use sdrkit_presets::{Data, Options, Region};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A session of `stack` with the values of Australia on a fake radio that plays its transmit samples at the sample rate.
fn session(stack: &str) -> (FakeIiod, Engine, SessionId) {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        FakeOpts {
            tx_plays: true,
            ..FakeOpts::default()
        },
    );
    let sdr = sdrkit_host_catalog::engine_on(Arc::new(net)).unwrap();
    let data = Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    sdr.set(OwnerRef::Engine, "transmit_allowed", true).unwrap();
    let radio = sdr.open(&format!("iiod:{}", srv.addr())).unwrap();
    let au = Region::new("AU").unwrap();
    let mesh = sdrkit_presets::start(&data, &sdr, radio, stack, &au, &Options::default()).unwrap();
    (srv, sdr, mesh)
}

/// The frequencies of the bursts the session sends until `count` have gone out. Each burst must be sent whole.
fn bursts(sdr: &Engine, mesh: SessionId, count: usize) -> Vec<f64> {
    let (mut events, mut out) = (Vec::new(), Vec::new());
    let t0 = Instant::now();
    while out.len() < count && t0.elapsed() < Duration::from_secs(40) {
        sdr.take_events(mesh.into(), &mut events).unwrap();
        for e in events.drain(..).filter(|e| e.is(&TRANSMIT)) {
            assert_eq!(e.fields.get(&BURST_SENT), Some(true));
            out.push(e.freq_hz);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(out.len(), count, "{:?}", sdr.problem(mesh));
    out
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

#[test]
fn meshtastic_adds_a_channel_without_a_key_and_sends_on_it() {
    let (srv, sdr, mesh) = session("meshtastic");
    sdr.call(mesh, "mt.channel.add", &[("name", text("Friends"))])
        .unwrap();
    sdr.call(
        mesh,
        "mt.send",
        &[("text", text("hello")), ("channel", text("friends"))],
    )
    .unwrap();
    // The node information and the text, on the Meshtastic frequency of Australia.
    assert_eq!(bursts(&sdr, mesh, 2), [919.875e6, 919.875e6]);
    assert_eq!(srv.board.tx_cut.load(Ordering::Relaxed), 0);
    assert_eq!(srv.board.tx_gap.load(Ordering::Relaxed), 0);
    assert_eq!(sdr.problem(mesh).unwrap(), None);
}

#[test]
fn meshcore_adds_channels_without_a_key_and_sends_an_advert_and_a_text() {
    let (srv, sdr, mesh) = session("meshcore");
    for name in ["#test", "club", "Public"] {
        sdr.call(mesh, "mc.channel.add", &[("name", text(name))])
            .unwrap();
    }
    sdr.call(mesh, "mc.advert", &[]).unwrap();
    // The plain name is the hashtag channel of that name.
    sdr.call(
        mesh,
        "mc.send",
        &[("text", text("hello")), ("channel", text("club"))],
    )
    .unwrap();
    // The advert and the text, on the MeshCore frequency of Australia.
    assert_eq!(bursts(&sdr, mesh, 2), [915.8e6, 915.8e6]);
    assert_eq!(srv.board.tx_cut.load(Ordering::Relaxed), 0);
    assert_eq!(srv.board.tx_gap.load(Ordering::Relaxed), 0);
    assert_eq!(sdr.problem(mesh).unwrap(), None);
}

#[test]
fn a_text_for_a_channel_that_does_not_exist_is_refused_with_the_name_of_the_argument() {
    for (stack, send) in [("meshtastic", "mt.send"), ("meshcore", "mc.send")] {
        let (_srv, sdr, mesh) = session(stack);
        let e = sdr
            .call(
                mesh,
                send,
                &[("text", text("hello")), ("channel", text("nowhere"))],
            )
            .unwrap_err()
            .to_string();
        assert!(e.contains("\"Channel\" (channel)"), "{e}");
    }
}

#[test]
fn a_stack_of_one_network_has_no_command_of_the_other() {
    let (_srv, sdr, mesh) = session("meshtastic");
    assert!(sdr.call(mesh, "mc.send", &[("text", text("x"))]).is_err());
    let (_srv, sdr, mesh) = session("meshcore");
    assert!(sdr.call(mesh, "mt.send", &[("text", text("x"))]).is_err());
}

#[test]
fn a_key_that_is_not_one_is_refused_with_the_form_it_must_have() {
    let (_srv, sdr, mesh) = session("meshcore");
    let e = sdr
        .call(mesh, "mc.identity", &[("key", text("1337"))])
        .unwrap_err()
        .to_string();
    assert!(e.contains("\"Key\" (key)") && e.contains("32 bytes"), "{e}");
}
