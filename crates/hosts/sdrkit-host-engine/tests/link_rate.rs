//! What the link to a network radio carries is a live reading: it chooses a narrower rate where the stack allows one,
//! and it never refuses a stack and never moves a stream that runs.

mod common;

use common::{defaults, field, number};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{Input, OwnerRef};
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
fn a_stack_that_the_link_cannot_carry_starts_all_the_same() {
    let net = FakeTransport::new();
    // 12 MB/s: about 2.5 Msps of 16-bit samples with room to spare; Wi-Fi needs 4 Msps or more.
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
    std::thread::sleep(Duration::from_secs(2));
    e.start_stack("wifi", None, &stack_values("wifi"), Input::Radio(r), None)
        .unwrap();
    // The stack has the rate it needs: the loss shows, and nothing is refused.
    assert!(number(&e, OwnerRef::Radio(r), "rate_hz") >= 4e6);
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}

#[test]
fn on_automatic_a_stream_that_gets_its_rate_leaves_the_next_stack_its_preferred_rate() {
    let net = FakeTransport::new();
    // 100 MB/s: 25 Msps of 16-bit samples. The idle stream runs far below it and gets its rate.
    let srv = FakeIiod::start_at(
        net.clone(),
        "10.0.0.5",
        FakeOpts {
            link_bytes_per_s: Some(100e6),
            ..FakeOpts::default()
        },
    );
    let e = sdrkit_host_catalog::engine_on(Arc::new(net.clone())).unwrap();
    let kind = e.find("driver.iiod").unwrap();
    let r = e.open_radio("iiod", &srv.addr(), &defaults(&e, kind)).unwrap();
    // Long enough for the watch of the idle stream to judge several windows.
    std::thread::sleep(Duration::from_secs(8));
    e.start_stack("wifi", None, &stack_values("wifi"), Input::Radio(r), None)
        .unwrap();
    // Wi-Fi prefers 20 Msps: the rate of the idle stream is no figure of the link, so it gets 20, with no note.
    assert_eq!(number(&e, OwnerRef::Radio(r), "rate_hz"), 20e6);
    let problem = field(&e, OwnerRef::Radio(r), "problem");
    assert!(matches!(problem, None | Some(sdrkit_core::Value::None)), "{problem:?}");
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}

#[test]
fn a_link_that_slows_down_under_a_running_stack_leaves_its_rate() {
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
    let rate = number(&e, OwnerRef::Radio(r), "rate_hz");
    assert!(rate >= 4e6);
    // The link drops to 12 MB/s: about 2.5 Msps, below the 4 that Wi-Fi needs. The stream keeps its rate.
    *srv.board.link_now.lock().unwrap() = Some(12e6);
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(number(&e, OwnerRef::Radio(r), "rate_hz"), rate);
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}
