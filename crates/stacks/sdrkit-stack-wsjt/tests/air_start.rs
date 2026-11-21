//! An FT8 call through the engine and the transmit path of a radio: the first sample goes on air at the start of its
//! slot. The radio is the fake iiod server, which plays its transmit buffers at the sample rate. The
//! stack anchors its slots to the system clock, so the test runs in real time and waits up to one cycle for its slot.

use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Facilities, MemStorage, Random, Value, Values};
use sdrkit_driver_core::iiod::fake::{FakeIiod, FakeOpts};
use sdrkit_host_engine::{Engine, Input, OwnerRef, Setup, TRANSMIT_ALLOWED};
use sdrkit_stack_wsjt::{Mode, Sub};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// The largest start error accepted, in seconds: the stack states the start of its burst, and the engine hands the
/// burst to the radio early by the latency of the transmitter.
const TOLERANCE_S: f64 = 0.020;

/// An engine with the weak-signal stacks on `net`, on the system clock, with a station and the transmit switch on.
fn wsjt_engine(net: &FakeTransport) -> Engine {
    let mut stacks = sdrkit_stack_wsjt::descriptors();
    stacks.retain(|d| d.id == "ft8");
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(net.clone()),
        stacks,
        families: vec![sdrkit_stack_wsjt::typed::family()],
        components: sdrkit_stack_wsjt::components::registry(),
    })
    .unwrap();
    for (key, value) in [
        ("callsign", Value::Text("K1ABC".into())),
        ("locator", Value::Text("FN42".into())),
        (TRANSMIT_ALLOWED.key(), Value::Bool(true)),
    ] {
        e.owner(OwnerRef::Engine, |o| {
            let id = o.schema().id(key).unwrap();
            o.apply(id, &value)
        })
        .unwrap()
        .unwrap();
    }
    e
}

/// Runs command `key` of owner `r` without arguments.
fn call(e: &Engine, r: OwnerRef, key: &str) {
    e.owner(r, |o| {
        let id = o.commands().id(key).unwrap();
        let args = o.commands().def(id).unwrap().new_args();
        sdrkit_core::owner::invoke(o, id, &args).map(|_| ())
    })
    .unwrap()
    .unwrap();
}

/// The UTC of instant `t`, from one reading of both clocks.
fn utc_of(t: Instant) -> f64 {
    let (now, sys) = (Instant::now(), SystemTime::now());
    let sys = sys.duration_since(UNIX_EPOCH).unwrap().as_secs_f64();
    if t >= now {
        sys + (t - now).as_secs_f64()
    } else {
        sys - (now - t).as_secs_f64()
    }
}

/// The index of the first transmit sample (I and Q of 16 bits) at or after `from` that is not zero.
fn first_signal(bytes: &[u8], from: usize) -> Option<usize> {
    (from..bytes.len() / 4).find(|&i| bytes[4 * i..4 * i + 4] != [0; 4])
}

#[test]
fn an_ft8_call_goes_on_air_within_the_tolerance_of_its_slot() {
    let net = FakeTransport::new();
    let srv = FakeIiod::start_on(
        net.clone(),
        FakeOpts {
            tx_plays: true,
            ..FakeOpts::default()
        },
    );
    let e = wsjt_engine(&net);
    let kind = e.find("driver.iiod").unwrap();
    let defaults = e.owner(kind, |o| Values::defaults(o.schema())).unwrap();
    let r = e.open_radio("iiod", &srv.addr(), &defaults).unwrap();
    e.owner(OwnerRef::Radio(r), |o| {
        let id = o.schema().id("tx_power").unwrap();
        sdrkit_core::owner::apply(o, id, &Value::F64(-10.0))
    })
    .unwrap()
    .unwrap();
    let d = &sdrkit_stack_wsjt::FT8;
    let values = Values::defaults(&(d.settings)());
    // The 2 m calling dial: no tuning limit stands in the way.
    let s = e
        .start_stack(d.id, None, &values, Input::Radio(r), Some(144.174e6))
        .unwrap();
    let session = OwnerRef::Session(s);
    std::thread::sleep(Duration::from_secs(1));
    call(&e, session, "cq");

    // Wait up to two cycles for the first sample of the call.
    let board = &srv.board;
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut scanned = 0;
    let first = loop {
        let found = {
            let bytes = board.tx_bytes.lock().unwrap();
            let f = first_signal(&bytes, scanned);
            scanned = bytes.len() / 4;
            f
        };
        if let Some(k) = found {
            break Some((k, Instant::now()));
        }
        if Instant::now() > deadline {
            break None;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let (k, arrived) = first.expect("no transmit sample in two cycles");
    let rate: f64 = board
        .attrs
        .lock()
        .unwrap()
        .get("iio:device1 OUTPUT voltage0 sampling_frequency")
        .and_then(|r| r.parse().ok())
        .expect("a transmit rate");
    // On air at the end of the played transfers less the samples after the first. A reading between the arrival of a
    // transfer and its time of play is early by that transfer, so the latest reading of 1 s is the one kept.
    let gap_before = board.tx_gap_in_signal.load(Ordering::Relaxed);
    let mut on_air: Option<Instant> = None;
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until {
        let end = board.tx_play.lock().unwrap().0;
        let n = board.tx_bytes.lock().unwrap().len() / 4;
        if let Some(end) = end {
            let t = end.checked_sub(Duration::from_secs_f64((n - k) as f64 / rate));
            on_air = on_air.max(t);
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    assert_eq!(
        board.tx_gap_in_signal.load(Ordering::Relaxed),
        gap_before,
        "the radio ran dry in the burst"
    );
    call(&e, session, "halt");
    e.stop_session(s).unwrap();

    let utc = utc_of(on_air.expect("a time of play"));
    let t0 = Sub::new(Mode::Ft8).t0();
    let slot = ((utc - t0) / 15.0).round() * 15.0;
    let error = utc - (slot + t0);
    let queued = utc - utc_of(arrived);
    eprintln!("FT8 on air {error:+.3} s from its slot start at {rate} Hz, {queued:.3} s after it reached the radio");
    assert!(error.abs() <= TOLERANCE_S, "start error {error:+.3} s");
}
