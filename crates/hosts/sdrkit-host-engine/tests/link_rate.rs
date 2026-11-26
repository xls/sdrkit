//! A network radio runs no stack at a rate that its link cannot carry: a stack whose least rate is above what the link
//! carries is refused with the two numbers, and the radio is left as it was.

mod common;

use common::{defaults, number};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{EngineError, Input, OwnerRef};
use std::sync::Arc;
use std::time::Duration;

fn stack_values(id: &str) -> Values {
    let d = sdrkit_host_catalog::all()
        .into_iter()
        .find(|d| d.id == id)
        .unwrap();
    Values::defaults(&(d.settings)())
}

#[test]
fn a_stack_that_the_link_cannot_carry_is_refused_and_the_radio_is_left_alone() {
    let net = FakeTransport::new();
    // 12 MB/s: about 2.5 Msps of 16-bit samples with room to spare; Wi-Fi needs 20 Msps.
    let srv = FakeIiod::start_at(
        net.clone(),
        "10.0.0.5",
        FakeOpts {
            link_bytes_per_s: Some(12e6),
            ..FakeOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e.open_radio("iiod", &srv.addr(), &defaults(&e, kind)).unwrap();
    // The radio streams at its idle rate; the driver times the link at the start of that stream.
    std::thread::sleep(Duration::from_secs(2));
    let before = srv.board.log().len();
    let refused = e.start_stack("wifi", None, &stack_values("wifi"), Input::Radio(r), None);
    let Err(err @ EngineError::LinkTooSlow { need_hz, carries_hz, .. }) = refused else {
        panic!("{refused:?}");
    };
    // The least rate that Wi-Fi declares.
    assert_eq!(need_hz, 4e6);
    assert!((1.5e6..3.5e6).contains(&carries_hz), "{carries_hz}");
    let text = err.to_string();
    assert!(text.starts_with("Wi-Fi networks needs 4 Msps; the link to this radio carries about "), "{text}");
    // Nothing was written to the radio.
    let after = &srv.board.log()[before..];
    assert!(!after.iter().any(|l| l.contains("sampling_frequency")), "{after:?}");
    // A stack that fits the link starts.
    e.start_stack("fm", None, &stack_values("fm"), Input::Radio(r), None)
        .unwrap();
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}

#[test]
fn a_stack_that_fits_the_link_below_its_wish_runs_narrower() {
    let net = FakeTransport::new();
    // 40 MB/s: about 8.5 Msps with room to spare; Wi-Fi would like 20 and runs at 4 or more.
    let srv = FakeIiod::start_at(
        net.clone(),
        "10.0.0.5",
        FakeOpts {
            link_bytes_per_s: Some(40e6),
            ..FakeOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e.open_radio("iiod", &srv.addr(), &defaults(&e, kind)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    e.start_stack("wifi", None, &stack_values("wifi"), Input::Radio(r), None)
        .unwrap();
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    assert!((4e6..12e6).contains(&rate), "{rate}");
    assert!(!srv.board.has("iio:device1 INPUT voltage0 sampling_frequency=20000000"));
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}

#[test]
fn a_link_that_slows_down_under_a_running_stack_brings_the_radio_to_what_it_carries() {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_at(
        net.clone(),
        "10.0.0.5",
        FakeOpts {
            link_bytes_per_s: Some(40e6),
            ..FakeOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e.open_radio("iiod", &srv.addr(), &defaults(&e, kind)).unwrap();
    std::thread::sleep(Duration::from_secs(2));
    e.start_stack("wifi", None, &stack_values("wifi"), Input::Radio(r), None)
        .unwrap();
    assert!(number(&e, OwnerRef::Radio(r), "rate_hz") >= 4e6);
    // The link drops to 12 MB/s: about 2.5 Msps, below the 4 that Wi-Fi needs.
    *srv.board.link_now.lock().unwrap() = Some(12e6);
    let end = std::time::Instant::now() + Duration::from_secs(12);
    while number(&e, OwnerRef::Radio(r), "rate_hz") >= 4e6 && std::time::Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    assert!((1.5e6..3.5e6).contains(&rate), "{rate}");
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}
