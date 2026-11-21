//! A Meshtastic client works with the plain values that the data folder has for a country: a
//! Meshtastic text is sent whole on the channel of that country.

use sdrkit_core::net::FakeTransport;
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{OwnerRef, BURST_SENT, TRANSMIT};
use sdrkit_presets::{Data, Options, Region};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A client that starts the stack with the preset of a country sends a text, on a fake radio that plays its transmit
/// samples at the sample rate.
#[test]
fn the_steps_of_the_client_send_a_text_on_the_channel_of_the_country() {
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
    let mesh =
        sdrkit_presets::start(&data, &sdr, radio, "meshtastic", &au, &Options::default()).unwrap();
    sdr.call(mesh, "mt.send", &[("text", "hello".into())])
        .unwrap();

    let (mut events, mut bursts) = (Vec::new(), Vec::new());
    let t0 = Instant::now();
    // The node information and the text.
    while bursts.len() < 2 && t0.elapsed() < Duration::from_secs(30) {
        sdr.take_events(mesh.into(), &mut events).unwrap();
        bursts.extend(events.drain(..).filter(|e| e.is(&TRANSMIT)));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(bursts.len(), 2, "{:?}", sdr.problem(mesh));
    for b in &bursts {
        assert_eq!(b.fields.get(&BURST_SENT), Some(true));
        assert_eq!(b.freq_hz, 919.875e6);
    }
    assert_eq!(srv.board.tx_cut.load(Ordering::Relaxed), 0);
    assert_eq!(srv.board.tx_gap.load(Ordering::Relaxed), 0);
    assert_eq!(sdr.problem(mesh).unwrap(), None);
}

/// A client without the data folder: the stack starts with its defaults on the
/// frequency that the host gives, and sends there.
#[test]
fn a_client_without_the_data_folder_sends_on_the_frequency_it_starts_on() {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        FakeOpts {
            tx_plays: true,
            ..FakeOpts::default()
        },
    );
    let sdr = sdrkit_host_catalog::engine_on(Arc::new(net)).unwrap();
    sdr.set(OwnerRef::Engine, "transmit_allowed", true).unwrap();
    let radio = sdr.open(&format!("iiod:{}", srv.addr())).unwrap();
    let mesh = sdr.start("meshtastic", radio, Some(919.875e6)).unwrap();
    sdr.call(mesh, "mt.send", &[("text", "hello".into())])
        .unwrap();
    let (mut events, mut bursts) = (Vec::new(), Vec::new());
    let t0 = Instant::now();
    while bursts.len() < 2 && t0.elapsed() < Duration::from_secs(30) {
        sdr.take_events(mesh.into(), &mut events).unwrap();
        bursts.extend(events.drain(..).filter(|e| e.is(&TRANSMIT)));
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(bursts.len(), 2, "{:?}", sdr.problem(mesh));
    for b in &bursts {
        assert_eq!(b.fields.get(&BURST_SENT), Some(true));
        assert_eq!(b.freq_hz, 919.875e6);
    }
    assert_eq!(sdr.problem(mesh).unwrap(), None);
}
