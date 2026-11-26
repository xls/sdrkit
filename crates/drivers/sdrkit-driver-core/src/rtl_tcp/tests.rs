//! rtl_tcp tests: the pure rules, and the client against a fake rtl_tcp server on a fake network.

use super::fake::{FakeOpts, FakeRtl};
use super::*;
use crate::{ring, Retuned, RingReader, RingWriter};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Choice, Results};

const M: f64 = 1e6;

fn server(mode: ServerMode) -> (&'static str, Value) {
    ("server", Value::Choice(mode.index()))
}

/// The values of a radio at 100 MHz, a gain of 30 dB, the rate `rate` and the settings `set`.
fn tune(rate: Hz, set: &[(&str, Value)]) -> Values {
    let d = RtlTcpDriver::new(Arc::new(FakeTransport::new()));
    let mut all = vec![
        ("centre_hz", Value::Hz(100e6)),
        ("rate", Value::Hz(rate)),
        ("gain", Value::F64(30.0)),
        ("agc", Value::Bool(false)),
    ];
    all.extend(set.iter().cloned());
    crate::testing::values(&d, &all)
}

fn plan(rate: Hz, mode: ServerMode) -> Result<RatePlan, DriverError> {
    let t = tune(rate, &[server(mode)]);
    plan_rate(&Options::from_values(&t), &t, false)
}

fn refused_rate() -> DriverError {
    DriverError::Refused {
        id: RadioSetting::Rate.id(),
    }
}

#[test]
fn rates_sent_for_an_rtl_sdr() {
    let rtl = ServerMode::Rtl;
    assert_eq!(plan(2.4 * M, rtl).unwrap().rate, 2_400_000);
    assert_eq!(plan(0.25 * M, rtl).unwrap().rate, 250_000);
    assert_eq!(plan(0.5 * M, rtl).unwrap().rate, 900_001);
    assert_eq!(plan(0.2 * M, rtl).unwrap().rate, 900_001);
    assert_eq!(plan(3.0 * M, rtl).unwrap().rate, 2_560_000);
    assert_eq!(plan(0.0, rtl).unwrap().rate, 2_048_000);
    assert_eq!(plan(10.0 * M, rtl).unwrap_err(), refused_rate());
}

#[test]
fn auto_takes_a_fast_request_as_another_radio() {
    let auto = ServerMode::Auto;
    let p = plan(3.0 * M, auto).unwrap();
    assert_eq!((p.rate, p.class), (3_000_000, RateClass::Other));
    assert!(p
        .note
        .unwrap()
        .contains("greets as an RTL-SDR but 3 Msps was asked for"));
    assert_eq!(plan(10.0 * M, auto).unwrap().rate, 10_000_000);
    assert_eq!(plan(2.4 * M, auto).unwrap().class, RateClass::Rtl);
    assert_eq!(plan(30.0 * M, auto).unwrap_err(), refused_rate());
    // The greeting said "other".
    let t = tune(0.1 * M, &[]);
    assert_eq!(
        plan_rate(&Options::from_values(&t), &t, true).unwrap().rate,
        250_000
    );
}

#[test]
fn chosen_any_and_pinned_rates() {
    let p = |t: &Values| plan_rate(&Options::from_values(t), t, false);
    let exact = ("rate_exact", Value::Bool(true));
    let t = tune(3.0 * M, &[server(ServerMode::Rtl), exact.clone()]);
    assert_eq!(p(&t).unwrap().rate, 3_000_000);
    let t = tune(10.0 * M, &[server(ServerMode::Rtl), exact]);
    assert_eq!(p(&t).unwrap().rate, 10_000_000);
    let any = server(ServerMode::Any);
    let t = tune(30.0 * M, std::slice::from_ref(&any));
    assert_eq!(p(&t).unwrap().rate, 30_000_000);
    let t = tune(0.05 * M, std::slice::from_ref(&any));
    assert_eq!(p(&t).unwrap().rate, 100_000);
    let t = tune(60.0 * M, &[any]);
    assert_eq!(p(&t).unwrap_err(), refused_rate());
    let t = tune(
        10.0 * M,
        &[server(ServerMode::Rtl), ("server_rate", Value::Hz(1.0 * M))],
    );
    assert_eq!(p(&t).unwrap().rate, 1_000_000);
}

#[test]
fn greetings() {
    let mut b = *b"RTL0\0\0\0\x05\0\0\0\x1d";
    let g = Greeting::parse(&b).unwrap();
    assert_eq!(
        (g.tuner, g.gains, g.is_other(), g.tuner_name()),
        (5, 29, false, "R820T")
    );
    b[7] = 9;
    assert!(Greeting::parse(&b).unwrap().is_other());
    assert!(Greeting::parse(b"RSP0\0\0\0\x01\0\0\0\0")
        .unwrap()
        .is_other());
    assert_eq!(Greeting::parse(b"\x00TL0\0\0\0\x05\0\0\0\x1d"), None);
    assert_eq!(Greeting::parse(b"RTL0\0\x01\0\x05\0\0\0\x1d"), None);
    assert_eq!(command(cmd::PPM, -5i32 as u32), [5, 0xFF, 0xFF, 0xFF, 0xFB]);
}

#[test]
fn gains_pick_the_nearest_entry_lower_on_a_tie() {
    assert_eq!(nearest_gain(GAINS_R820T, 300), 297);
    assert_eq!(nearest_gain(GAINS_R820T, 0), 0);
    assert_eq!(nearest_gain(GAINS_R820T, 1000), 496);
    assert_eq!(nearest_gain(&[10, 20], 15), 10);
    assert_eq!(nearest_gain(GAINS_E4000, -50), -10);
}

#[test]
fn pinning_rules() {
    assert_eq!(snap_server_rate(1.01 * M), 1.0 * M);
    assert_eq!(snap_server_rate(2.39 * M), 2.4 * M);
    assert_eq!(snap_server_rate(7.123 * M), 7.12 * M);
    let mut d = PinDetector::default();
    let mut total = 0;
    let mut feed = |d: &mut PinDetector, achieved: f64, asked: f64| {
        total += 1000;
        d.reading(achieved, asked, total, None)
    };
    // Two skipped, then three collected: the median decides.
    assert_eq!(feed(&mut d, 5.0 * M, 2.4 * M), None);
    assert_eq!(feed(&mut d, 5.0 * M, 2.4 * M), None);
    assert_eq!(feed(&mut d, 1.0 * M, 2.4 * M), None);
    assert_eq!(feed(&mut d, 0.99 * M, 2.4 * M), None);
    assert_eq!(feed(&mut d, 3.0 * M, 2.4 * M), Some(1.0 * M));
    // Less than asked and not steady, as a slow link gives it: not pinned.
    for x in [1.31, 1.12, 1.47, 1.2, 1.38, 1.05] {
        assert_eq!(feed(&mut d, x * M, 2.4 * M), None);
    }
    // An obeying server, within 4 %.
    for _ in 0..10 {
        assert_eq!(feed(&mut d, 2.45 * M, 2.4 * M), None);
    }
    // A new requested rate starts again with two skips.
    assert_eq!(feed(&mut d, 1.0 * M, 2.0 * M), None);
    assert_eq!(feed(&mut d, 1.0 * M, 2.0 * M), None);
    assert_eq!(feed(&mut d, 1.0 * M, 2.0 * M), None);
    assert_eq!(feed(&mut d, 1.0 * M, 2.0 * M), None);
    assert_eq!(feed(&mut d, 1.0 * M, 2.0 * M), Some(1.0 * M));
}

fn wait_for(mut what: impl FnMut() -> bool, secs: f64) -> bool {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < secs {
        if what() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    what()
}

/// Reads `n` samples as their two bytes.
fn read_n(r: &mut RingReader, n: usize, secs: f64) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 * n);
    let t = Instant::now();
    while v.len() < 2 * n && t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(20));
        let want = n - v.len() / 2;
        r.read_with(want, |b| v.extend_from_slice(b));
    }
    v
}

fn cu8_ring() -> (RingWriter, RingReader) {
    ring(1 << 22, SampleType::Cu8)
}

#[test]
fn opens_streams_and_retunes_live() {
    let srv = FakeRtl::start(FakeOpts::default());
    let mut src = srv
        .driver()
        .open(
            &srv.addr(),
            &tune(
                2.4 * M,
                &[
                    ("ppm", Value::F64(5.0)),
                    ("ds", Value::Choice(DirectSampling::Off.index())),
                ],
            ),
        )
        .unwrap();
    assert!(src.capabilities().name.contains("R820T"));
    // The ranges are those of a radio, not placeholders.
    let placeholders = crate::form::check_capabilities(src.capabilities());
    assert!(placeholders.is_empty(), "{placeholders:?}");
    assert_eq!(src.capabilities().rates.max_hz, 2.56 * M);
    // The usual rates and the gain steps of the tuner (29 for the R820T, 0 to 49.6 dB).
    let caps = src.capabilities();
    assert_eq!(caps.common_rates_hz.len(), 7);
    assert_eq!(
        plan(caps.common_rates_hz[0], ServerMode::Rtl).unwrap().rate,
        900_001
    );
    assert_eq!(caps.gain_steps_db.len(), 29);
    assert_eq!(caps.gain_steps_db.last(), Some(&49.6));
    let (w, mut r) = cu8_ring();
    let s = src.start(w).unwrap();
    assert_eq!((s.rate_hz, s.centre_hz), (2.4 * M, 100e6));
    assert!(wait_for(|| srv.cmds(0).len() >= 7, 2.0));
    assert_eq!(
        srv.cmds(0),
        [
            (cmd::GAIN_MODE, 1),
            (cmd::AGC, 0),
            (cmd::RATE, 2_400_000),
            (cmd::DIRECT, 0),
            (cmd::PPM, 5),
            (cmd::FREQ, 100_000_000),
            (cmd::GAIN, 297)
        ]
    );
    // Samples: the server sends bytes 0, 1, 2, ... in order, and the ring holds them as they came.
    let got = read_n(&mut r, 1000, 3.0);
    assert_eq!(got.len(), 2000);
    for (i, b) in got.iter().enumerate() {
        assert_eq!(*b, i as u8, "byte {i}");
    }
    // Only what changed is sent, and a rate change is live.
    let base = [
        ("ppm", Value::F64(5.0)),
        ("ds", Value::Choice(DirectSampling::Off.index())),
        ("centre_hz", Value::Hz(101e6)),
    ];
    let t = tune(2.4 * M, &base);
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(s) if s.centre_hz == 101e6));
    let t = tune(1.024 * M, &base);
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(s) if s.rate_hz == 1.024 * M));
    let mut with_bias = base.to_vec();
    with_bias.push(("bias_tee", Value::Bool(true)));
    src.apply(&tune(1.024 * M, &with_bias)).unwrap();
    assert!(wait_for(|| srv.cmds(0).len() >= 10, 2.0));
    assert_eq!(
        srv.cmds(0)[7..],
        [
            (cmd::FREQ, 101_000_000),
            (cmd::RATE, 1_024_000),
            (cmd::BIAS, 1)
        ]
    );
    // On close the bias tee is switched off.
    src.stop();
    assert!(wait_for(
        || srv.cmds(0).last() == Some(&(cmd::BIAS, 0)),
        2.0
    ));
    src.stop();
}

#[test]
fn a_server_that_is_not_rtl_tcp_fails_at_once() {
    let srv = FakeRtl::start(FakeOpts {
        greeting: *b"\x00\x01\x02\x03\0\0\0\x05\0\0\0\x1d",
        ..Default::default()
    });
    let t = Instant::now();
    let e = srv.driver().open(&srv.addr(), &tune(2.4 * M, &[])).err();
    assert_eq!(e, Some(DriverError::Protocol(Protocol::NotThisRadio)));
    assert!(t.elapsed() < Duration::from_millis(400));
    assert_eq!(srv.connections(), 1);
}

#[test]
fn a_refused_connection_is_tried_four_times() {
    let net = FakeTransport::new();
    let drv = RtlTcpDriver::new(Arc::new(net.clone()));
    let t = Instant::now();
    let e = drv.open("127.0.0.1:4321", &tune(2.4 * M, &[])).err();
    assert_eq!(e, Some(DriverError::Net(NetError::Refused)));
    let secs = t.elapsed().as_secs_f64();
    assert!((1.4..3.0).contains(&secs), "{secs}");
    assert_eq!(
        drv.open("radio:x", &tune(2.4 * M, &[])).err(),
        Some(DriverError::Address)
    );
}

#[test]
fn another_radio_gets_its_gain_as_asked_and_the_wide_rate_list() {
    let srv = FakeRtl::start(FakeOpts {
        greeting: *b"RSP0\0\0\0\0\0\0\0\0",
        ..Default::default()
    });
    let drv = srv.driver();
    let mut src = drv.open(&srv.addr(), &tune(8.0 * M, &[])).unwrap();
    assert_eq!(src.capabilities().rates.max_hz, 20.0 * M);
    let (w, _r) = cu8_ring();
    src.start(w).unwrap();
    assert!(wait_for(|| srv.cmds(0).len() >= 5, 2.0));
    let c = srv.cmds(0);
    assert!(
        c.contains(&(cmd::RATE, 8_000_000)) && c.contains(&(cmd::GAIN, 300)),
        "{c:?}"
    );
    assert!(wait_for(|| drv.throughput(&srv.addr()).is_some(), 3.0));
    let tp = drv.throughput(&srv.addr()).unwrap();
    assert!((tp.achieved_hz / 8e6 - 1.0).abs() < 0.1, "{tp:?}");
    let mut res = Results::default();
    src.poll(&mut res);
    assert!(res.status().summary.contains("Mbit/s"));
    src.stop();
}

#[test]
fn reconnects_after_a_drop_and_sends_everything_again() {
    let srv = FakeRtl::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(300)),
        ..Default::default()
    });
    let mut src = srv.driver().open(&srv.addr(), &tune(1.0 * M, &[])).unwrap();
    let (w, mut r) = cu8_ring();
    src.start(w).unwrap();
    assert!(
        wait_for(|| srv.cmds(1).len() >= 5, 3.0),
        "no second connection"
    );
    assert_eq!(
        srv.cmds(1)[..5],
        [
            (cmd::GAIN_MODE, 1),
            (cmd::AGC, 0),
            (cmd::RATE, 1_000_000),
            (cmd::PPM, 0),
            (cmd::FREQ, 100_000_000)
        ]
    );
    // Samples go on into the same ring.
    r.skip_all();
    assert_eq!(read_n(&mut r, 10_000, 3.0).len(), 20_000);
    let mut res = Results::default();
    src.poll(&mut res);
    let log: Vec<&String> = res.log_lines().iter().collect();
    assert!(
        log.iter().any(|t| t.contains("the connection is closed")),
        "{log:?}"
    );
    assert!(
        log.iter().any(|t| t.contains("reconnected after")),
        "{log:?}"
    );
    let loss = src.loss().unwrap();
    assert!(
        loss.link_down > 0 && loss.radio_lost >= loss.link_down,
        "{loss:?}"
    );
    src.stop();
}

#[test]
fn a_retune_while_away_is_stored_and_used_by_the_reconnect() {
    let mut srv = FakeRtl::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(200)),
        mute_after_first: true,
        ..Default::default()
    });
    let mut src = srv.driver().open(&srv.addr(), &tune(1.0 * M, &[])).unwrap();
    let (w, _r) = cu8_ring();
    src.start(w).unwrap();
    // After the drop, the server is gone entirely.
    std::thread::sleep(Duration::from_millis(250));
    srv.shutdown();
    std::thread::sleep(Duration::from_millis(300));
    let t = tune(1.0 * M, &[("centre_hz", Value::Hz(433.92e6))]);
    let started = Instant::now();
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(_)));
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "a retune must not wait on an open"
    );
    assert_eq!(src.link(), crate::Link::Away);
    // The server comes back on the same address.
    let srv2 = FakeRtl::start_on(srv.net().clone(), FakeOpts::default());
    assert!(wait_for(|| srv2.cmds(0).len() >= 5, 4.0), "no reconnect");
    assert!(
        srv2.cmds(0).contains(&(cmd::FREQ, 433_920_000)),
        "{:?}",
        srv2.cmds(0)
    );
    src.stop();
}

#[test]
fn a_stop_cuts_a_pending_open_short() {
    let srv = FakeRtl::start(FakeOpts {
        drop_first_after: Some(Duration::from_millis(150)),
        mute_after_first: true,
        ..Default::default()
    });
    let mut src = srv.driver().open(&srv.addr(), &tune(1.0 * M, &[])).unwrap();
    let (w, _r) = ring(1 << 20, SampleType::Cu8);
    src.start(w).unwrap();
    // The reconnect waits (up to 10 s) for a greeting that never comes.
    assert!(wait_for(|| srv.connections() >= 2, 3.0));
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(300),
        "stop took {:?}",
        t.elapsed()
    );
}

#[test]
fn a_server_that_ignores_the_rate_is_pinned() {
    let srv = FakeRtl::start(FakeOpts {
        own_rate: Some(1.0 * M),
        ..Default::default()
    });
    let drv = RtlTcpDriver::with_timing(
        Arc::new(srv.net().clone()),
        Duration::from_millis(200),
        Duration::from_secs(3),
    );
    let mut src = drv.open(&srv.addr(), &tune(2.4 * M, &[])).unwrap();
    let (w, mut r) = cu8_ring();
    src.start(w).unwrap();
    let mut res = Results::default();
    let mut polled = Vec::new();
    let found = wait_for(
        || {
            r.skip_all();
            let p = src.poll(&mut res);
            let hit = p.store == Some((own_id(RtlSetting::ServerRate), Value::Hz(1.0 * M)));
            polled.push(p);
            hit
        },
        4.0,
    );
    assert!(found, "{polled:?}");
    let p = polled.last().unwrap();
    assert!(p.restart);
    assert!(polled.iter().any(|p| p.capabilities), "{polled:?}");
    assert!(res
        .log_lines()
        .iter()
        .any(|t| t.contains("sends 1 Msps whatever it is asked")));
    let rates = &src.capabilities().rates;
    assert_eq!((rates.min_hz, rates.max_hz), (1.0 * M, 1.0 * M));
    // The host stores the rate and restarts: the driver requests exactly that rate from the server.
    src.stop();
    let mut src = drv
        .open(
            &srv.addr(),
            &tune(2.4 * M, &[("server_rate", Value::Hz(1.0 * M))]),
        )
        .unwrap();
    let (w, _r) = cu8_ring();
    assert_eq!(src.start(w).unwrap().rate_hz, 1.0 * M);
    assert!(wait_for(
        || srv.cmds(1).contains(&(cmd::RATE, 1_000_000)),
        2.0
    ));
    src.stop();
}

#[test]
fn an_obeying_server_is_never_pinned() {
    let srv = FakeRtl::start(FakeOpts::default());
    let drv = RtlTcpDriver::with_timing(
        Arc::new(srv.net().clone()),
        Duration::from_millis(150),
        Duration::from_secs(3),
    );
    let mut src = drv.open(&srv.addr(), &tune(1.024 * M, &[])).unwrap();
    let (w, mut r) = cu8_ring();
    src.start(w).unwrap();
    let t = Instant::now();
    let mut res = Results::default();
    let mut stored = false;
    while t.elapsed() < Duration::from_millis(1500) {
        r.skip_all();
        stored |= src.poll(&mut res).store.is_some();
        std::thread::sleep(Duration::from_millis(20));
    }
    src.stop();
    assert!(!stored);
    assert!(drv.throughput(&srv.addr()).is_some());
}
