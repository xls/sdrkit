//! The driver against the fake server on a fake network, plus the pure rules of the driver.

use super::fake::{self, Fake, LIN_ID, SR_ID};
use super::{hex, REAL_MENU_HEX};
use crate::proto::cmd;
use crate::*;
use sdrkit_core::net::{FakeTransport, NetError};
use sdrkit_core::{Complex32, Owner, Value};
use sdrkit_driver_core::{ring, DriverKind, Protocol, RingReader};
use std::time::{Duration, Instant};

const M: f64 = 1e6;

/// The values of a radio at `centre` and `rate`. `gain`: a fixed gain, or `None` for the gain control of the server. Then
/// `settings` by key.
fn tune(rate: f64, centre: f64, gain: Option<f64>, settings: &[(&str, Value)]) -> Values {
    let kind = DriverKind::new(Box::new(SdrppDriver::new(Arc::new(FakeTransport::new()))));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    let mut set = |k: &str, x: &Value| v.set(schema.id(k).unwrap(), x).unwrap();
    set("centre_hz", &Value::Hz(centre));
    set("rate", &Value::Hz(rate));
    set("agc", &Value::Bool(gain.is_none()));
    if let Some(g) = gain {
        set("gain", &Value::F64(g));
    }
    for (k, x) in settings {
        set(k, x);
    }
    v
}

fn wire(t: WireType) -> Value {
    Value::Choice(sdrkit_core::Choice::index(t))
}

fn drv(f: &Fake) -> SdrppDriver {
    SdrppDriver::new(f.transport())
}

fn wait_until(secs: f64, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    f()
}

/// Reads `n` samples (waiting up to `secs`).
fn read(r: &mut RingReader, n: usize, secs: f64) -> Vec<Complex32> {
    let mut out = vec![Complex32::default(); n];
    let mut got = 0;
    let end = Instant::now() + Duration::from_secs_f64(secs);
    while got < n && Instant::now() < end {
        r.wait(1, Duration::from_millis(20));
        got += r.read_iq(&mut out[got..]);
    }
    out.truncate(got);
    out
}

fn all_near(s: &[Complex32], i: f32, q: f32, tol: f32) -> bool {
    !s.is_empty()
        && s.iter()
            .all(|c| (c.re - i).abs() < tol && (c.im - q).abs() < tol)
}

fn live(r: Result<Retuned, DriverError>) -> Started {
    match r.expect("retune") {
        Retuned::Live(s) => s,
        Retuned::NeedsRestart => panic!("the driver does every change live"),
    }
}

/// Opens and starts against a fake, giving the source and the reader of the ring.
fn started(drv: &SdrppDriver, f: &Fake, t: &Values) -> (Box<dyn Source>, RingReader, Started) {
    let mut src = drv.open(&f.addr(), t).expect("open");
    let (w, r) = ring(1 << 22, src.sample_type());
    let s = src.start(w).expect("start");
    (src, r, s)
}

fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

/// The log lines since the last poll.
fn logs(src: &mut Box<dyn Source>) -> Vec<String> {
    let mut r = results();
    src.poll(&mut r);
    r.log_lines().iter().cloned().collect()
}

/// The readings of `src` now.
fn readings(src: &mut Box<dyn Source>) -> Results {
    let mut r = results();
    src.poll(&mut r);
    r
}

// ---- Pure rules ----

#[test]
fn addresses_are_normalised() {
    assert_eq!(
        parse_address(" ip:10.0.0.2 "),
        Ok(("10.0.0.2".into(), 5259))
    );
    assert_eq!(parse_address("tcp://host:99"), Ok(("host".into(), 99)));
    assert_eq!(parse_address("[::1]:5259"), Ok(("::1".into(), 5259)));
    let e = parse_address("host:x").unwrap_err();
    assert!(!e.retryable());
    assert_eq!(e, DriverError::Address);
    assert!(parse_address(":5259").is_err());
}

#[test]
fn the_rate_entry_is_the_slowest_fast_enough() {
    let r = [10e6, 2.5e6];
    assert_eq!(pick_rate(&r, 2.2 * M), 1);
    assert_eq!(pick_rate(&r, 8.0 * M), 0);
    assert_eq!(pick_rate(&r, 12.0 * M), 0);
    assert_eq!(pick_rate(&r, 0.0), 1);
    assert_eq!(pick_rate(&r, 2.5 * M * 1.00005), 1);
}

#[test]
fn the_frequency_is_corrected_for_the_ppm() {
    assert_eq!(corrected_frequency(1e9, 10.0), 1e9 / (1.0 + 10.0 * 1e-6));
    assert_eq!(
        corrected_frequency(1e9, 10.0).to_le_bytes().to_vec(),
        hex("c4 cc 0c 78 51 cd cd 41")
    );
    assert_eq!(corrected_frequency(1e9, 5000.0), 1e9);
    assert_eq!(corrected_frequency(-5.0, 0.0), 0.0);
}

#[test]
fn the_driver_settings() {
    let schema = DESCRIPTOR.decl().schema().clone();
    let own = |s: SdrppSetting| schema.setting(sdrkit_driver_core::own_id(s)).unwrap();
    assert_eq!(
        own(SdrppSetting::SampleFormat).field.key.as_ref(),
        "format"
    );
    assert!(own(SdrppSetting::Compression).restart);
    assert!(!own(SdrppSetting::SampleFormat).restart && !own(SdrppSetting::Ppm).restart);
    let ppm = own(SdrppSetting::Ppm).range.unwrap();
    assert_eq!((ppm.min, ppm.max), (-200.0, 200.0));
    assert_eq!(DESCRIPTOR.key, "sdrpp");
    assert_eq!(DESCRIPTOR.name, "SDR++ server");
    let o = Options::from_values(&tune(
        0.0,
        0.0,
        None,
        &[
            ("format", wire(WireType::F32)),
            ("compression", Value::Bool(true)),
            ("ppm", Value::F64(1.5)),
        ],
    ));
    assert_eq!(
        (o.sample_type, o.compression, o.server_gain, o.ppm),
        (proto::SampleType::F32, true, false, 1.5)
    );
}

#[test]
fn window_loss_is_not_counted_twice() {
    let base = |radio_lost, link_down, events| sdrkit_driver_core::LossReport {
        rate_hz: 10e6,
        radio_lost,
        radio_events: events,
        link_down,
        since_radio_loss: Some(Duration::from_secs(5)),
        ..Default::default()
    };
    // The detector already found more than the windows: its count stands.
    let r = merge_loss(
        base(6_000_000, 0, 3),
        5_000_000,
        2,
        Some(Duration::from_secs(1)),
    );
    assert_eq!((r.radio_lost, r.radio_events), (6_000_000, 3));
    assert_eq!(r.since_radio_loss, Some(Duration::from_secs(5)));
    // The windows found more: their count replaces the detector's, the link-down gap stays apart.
    let r = merge_loss(
        base(3_000_000, 1_000_000, 2),
        4_000_000,
        4,
        Some(Duration::from_secs(1)),
    );
    assert_eq!(
        (r.radio_lost, r.radio_events, r.link_down),
        (5_000_000, 4, 1_000_000)
    );
    assert_eq!(r.since_radio_loss, Some(Duration::from_secs(1)));
    // Only a gap and no window loss.
    let r = merge_loss(base(2_000_000, 2_000_000, 1), 0, 0, None);
    assert_eq!(r.radio_lost, 2_000_000);
}

// ---- Scenarios ----

#[test]
fn an_open_reads_the_server() {
    let f = Fake::start();
    let d = drv(&f);
    let mut src = d
        .open(&f.addr(), &tune(2.2 * M, 100e6, None, &[]))
        .expect("open");
    let c = src.capabilities().clone();
    // The ranges are those of a radio, not placeholders.
    let placeholders = sdrkit_driver_core::form::check_capabilities(&c);
    assert!(placeholders.is_empty(), "{placeholders:?}");
    assert!(c.name.contains("Airspy"), "{}", c.name);
    assert_eq!(c.rates.ranges, [(2.5e6, 2.5e6), (10e6, 10e6)]);
    assert_eq!(c.rates.max_hz, 10e6);
    assert_eq!(c.gain_db.map(|g| (g.0, g.1)), Some((0.0, 21.0)));
    let res = readings(&mut src);
    assert_eq!(res.fields().get(&SOURCE), Some("Airspy"));
    // The transcript: the rate of the server, then GET_UI and the answer.
    let st = f.state();
    assert_eq!(st.received[0], hex("00 00 00 00 0c 00 00 00 00 00 00 00"));
    assert_eq!(
        st.sent[0],
        hex("00 00 00 00 14 00 00 00 80 00 00 00 00 00 00 00 d0 12 63 41")
    );
    let mut ack = hex("01 00 00 00");
    ack.extend(((8 + 4 + 591) as u32).to_le_bytes());
    ack.extend(hex("00 00 00 00"));
    ack.extend(hex(REAL_MENU_HEX));
    assert_eq!(st.sent[1], ack, "the menu of the fake is the real one");
    drop(st);
    src.stop();
    assert!(f.wait(2.0, |s| !s.client));
}

/// Start, busy, a live retune that needs a stop, a gain-only retune, ppm, stop.
#[test]
fn start_retune_and_stop() {
    let f = Fake::start();
    let d = drv(&f);
    let (mut src, mut r, s) = started(&d, &f, &tune(2.2 * M, 100e6, Some(10.0), &[]));
    assert_eq!(s.rate_hz, 2.5e6);
    assert!(f.wait(3.0, |s| s.has("get_ui")
        && s.log.iter().filter(|l| *l == "get_ui").count() >= 2));
    {
        let st = f.state();
        let sr = st.pos(&format!("action {SR_ID}=1")).expect("rate action");
        let start = st.pos("start").expect("start");
        assert!(sr < start, "{:?}", st.log);
        assert!(st.has(&format!("action {LIN_ID}=10")));
        assert!(st.has("freq 100000000"));
        assert!(st.has("compression 0"));
        assert_eq!(
            (
                st.rate_entry,
                st.lin_gain,
                st.sample_type,
                st.compression,
                st.running
            ),
            (1, 10, 1, false, true)
        );
        // The transcript after the greeting, byte for byte.
        let expect = [
            hex("00 00 00 00 0c 00 00 00 00 00 00 00"),
            hex("00 00 00 00 0d 00 00 00 07 00 00 00 00"),
            proto::command_bytes(
                cmd::UI_ACTION,
                &proto::ui_action_arg(SR_ID, proto::ActionValue::Int(1), true),
            ),
            proto::command_bytes(
                cmd::UI_ACTION,
                &proto::ui_action_arg(LIN_ID, proto::ActionValue::Int(10), true),
            ),
            hex("00 00 00 00 0d 00 00 00 06 00 00 00 01"),
            hex("00 00 00 00 14 00 00 00 04 00 00 00 00 00 00 00 84 d7 97 41"),
            hex("00 00 00 00 0c 00 00 00 02 00 00 00"),
            hex("00 00 00 00 0c 00 00 00 00 00 00 00"),
        ];
        assert_eq!(st.received, expect);
        // The server's answers: rate, menu, rate 2.5 then the redrawn menu, the redrawn menu, the frequency's ACK.
        assert_eq!(
            st.sent[2],
            hex("00 00 00 00 14 00 00 00 80 00 00 00 00 00 00 00 d0 12 43 41")
        );
        assert_eq!(st.sent[5], hex("01 00 00 00 0c 00 00 00 04 00 00 00"));
    }
    let got = read(&mut r, 20_000, 3.0);
    assert!(all_near(&got, 0.5, -0.25, 1e-4), "{:?}", got.first());
    // The stopped-only controls were learnt; the Refresh button is not a control.
    let stopped_only: Vec<String> = lock(&d.mem(&f.addr()))
        .stopped_only
        .iter()
        .cloned()
        .collect();
    for id in [
        "##sdrpp_server_src_sel",
        "##_airspy_dev_sel_Airspy Source",
        SR_ID,
    ] {
        assert!(
            stopped_only.iter().any(|x| x == id),
            "{id} in {stopped_only:?}"
        );
    }
    assert_eq!(stopped_only.len(), 3);

    // Busy.
    let e = d
        .open(&f.addr(), &tune(2.2 * M, 100e6, None, &[]))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Busy);

    // A rate change, more gain and 8-bit samples: held stopped for the rate list.
    f.state().log.clear();
    let s = live(src.apply(&tune(
        8.0 * M,
        100e6,
        Some(21.0),
        &[("format", wire(WireType::I8))],
    )));
    assert_eq!(s.rate_hz, 10e6);
    assert!(f.wait(3.0, |s| s.has("start")), "{:?}", f.state().log);
    {
        let st = f.state();
        let stop = st.pos("stop").expect("stop");
        let sr = st.pos(&format!("action {SR_ID}=0")).expect("rate");
        let start = st.pos("start").expect("start");
        assert!(stop < sr && sr < start, "{:?}", st.log);
        assert!(
            st.has(&format!("action {LIN_ID}=21")) && st.has("type 0"),
            "{:?}",
            st.log
        );
        assert_eq!(st.log.iter().filter(|l| *l == "stop").count(), 1);
    }
    r.skip_all();
    let got = read(&mut r, 20_000, 3.0);
    assert!(all_near(&got, 0.5, -0.25, 1e-4), "{:?}", got.first());

    // Gain only: no stop.
    f.state().log.clear();
    live(src.apply(&tune(
        8.0 * M,
        100e6,
        Some(5.0),
        &[("format", wire(WireType::I8))],
    )));
    assert!(f.wait(3.0, |s| s.has(&format!("action {LIN_ID}=5"))));
    assert!(!f.state().has("stop"));

    // The frequency correction.
    live(src.apply(&tune(
        8.0 * M,
        1e9,
        Some(5.0),
        &[
            ("format", wire(WireType::I8)),
            ("ppm", Value::F64(10.0)),
        ],
    )));
    assert!(
        f.wait(3.0, |s| s.has("freq 999990000")),
        "{:?}",
        f.state().log
    );
    assert_eq!(f.state().freq_bytes, hex("c4 cc 0c 78 51 cd cd 41"));

    // The stop sends STOP before the socket goes, and closes it.
    f.state().log.clear();
    src.stop();
    assert!(
        f.wait(3.0, |s| s.has("stop") && !s.client),
        "{:?}",
        f.state().log
    );
    assert!(!f.state().running);
    src.stop();
}

/// A slow link: no sample taken with the old gain reaches the ring after the retune returns.
#[test]
fn a_flush_drops_samples_taken_before_the_change() {
    let f = Fake::with(|s| {
        s.i_follows_gain = true;
    });
    let d = drv(&f);
    let (mut src, mut r, _) = started(&d, &f, &tune(2.2 * M, 100e6, Some(10.0), &[]));
    let got = read(&mut r, 10_000, 3.0);
    assert!(
        all_near(&got, 10.0 / 32.0, -0.25, 1e-3),
        "{:?}",
        got.first()
    );
    f.state().cmd_delay = Duration::from_millis(300);
    std::thread::sleep(Duration::from_millis(50));
    let t = Instant::now();
    live(src.apply(&tune(2.2 * M, 100e6, Some(20.0), &[])));
    assert!(
        t.elapsed() < Duration::from_millis(150),
        "{:?}",
        t.elapsed()
    );
    r.skip_all();
    let got = read(&mut r, 400_000, 5.0);
    assert!(got.len() > 100_000, "{}", got.len());
    assert!(
        all_near(&got, 20.0 / 32.0, -0.25, 1e-3),
        "{:?}",
        got.iter().find(|c| (c.re - 0.625).abs() > 1e-3)
    );
    assert_eq!(f.state().lin_gain, 20);
    src.stop();
}

fn drain(mut r: RingReader) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = vec![Complex32::default(); 1 << 16];
        while !r.writer_gone() {
            r.wait(1, Duration::from_millis(20));
            r.read_iq(&mut buf);
        }
    })
}

/// Throughput: a link slower than the stream shows as radio-side loss.
#[test]
fn a_slow_link_counts_as_radio_side_loss() {
    let f = Fake::start();
    let d = drv(&f);
    // At 2.5 Msps, so a loaded test machine keeps up with the stream.
    let (mut src, r, s) = started(&d, &f, &tune(2.2 * M, 100e6, None, &[]));
    assert_eq!(s.rate_hz, 2.5e6);
    let t = drain(r);
    std::thread::sleep(Duration::from_millis(2500));
    let lost = src.loss().unwrap().radio_lost;
    assert!(lost < 3_000_000, "{lost}");
    src.stop();
    t.join().unwrap();

    let f = Fake::with(|s| s.throttle = 0.33);
    let d = drv(&f);
    let (mut src, r, _) = started(&d, &f, &tune(8.0 * M, 100e6, None, &[]));
    let t = drain(r);
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_millis(4500));
    let rep = src.loss().unwrap();
    assert!(rep.radio_lost > 10_000_000, "{rep:?}");
    // Never more than the server could have sent since the start: the same samples are not counted twice.
    assert!(
        (rep.radio_lost as f64) < 10e6 * t0.elapsed().as_secs_f64(),
        "{rep:?}"
    );
    let res = readings(&mut src);
    let (arriving, mbit) = (
        res.fields().get(&ARRIVING).unwrap(),
        res.fields().get(&MBIT_S).unwrap(),
    );
    assert!(arriving < 5e6 && mbit > 0.0, "{arriving} {mbit}");
    assert_eq!(res.fields().get(&SERVER_RATE), Some(10e6));
    let l: Vec<String> = res.log_lines().iter().cloned().collect();
    assert!(l.iter().any(|x| x.contains("carries only")), "{l:?}");
    src.stop();
    t.join().unwrap();
}

/// Reconnect after the server drops the connection.
#[test]
fn the_driver_reconnects() {
    let f = Fake::start();
    let d = drv(&f);
    let (mut src, mut r, _) = started(
        &d,
        &f,
        &tune(
            2.2 * M,
            100e6,
            Some(10.0),
            &[("format", wire(WireType::I8))],
        ),
    );
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    {
        let mut st = f.state();
        st.log.clear();
        st.drop_now = true;
    }
    assert!(
        f.wait(10.0, |s| s.accepts >= 2
            && s.has("type 0")
            && s.has("start")),
        "{:?}",
        f.state().log
    );
    r.skip_all();
    let got = read(&mut r, 20_000, 5.0);
    assert!(all_near(&got, 0.5, -0.25, 1e-2), "{}", got.len());
    assert!(wait_until(3.0, || src.loss().unwrap().link_down > 0));
    let l = logs(&mut src);
    assert!(
        l.iter().any(|x| x.contains("the connection is closed")),
        "{l:?}"
    );
    assert!(l.iter().any(|x| x.contains("reconnected after")), "{l:?}");
    src.stop();
    assert!(f.wait(3.0, |s| !s.client));
}

/// Nothing listening, not an SDR++ server, no source module.
#[test]
fn failures_at_connect() {
    let t = tune(2.2 * M, 100e6, None, &[]);
    let d = SdrppDriver::new(Arc::new(FakeTransport::new()));
    let e = d.open("127.0.0.1:5259", &t).err().unwrap();
    assert_eq!(e, DriverError::Net(NetError::Refused));
    assert!(e.retryable());

    let d = SdrppDriver::new(Arc::new(fake::not_sdrpp()));
    let e = d.open("127.0.0.1:5259", &t).err().unwrap();
    assert_eq!(e, DriverError::Protocol(Protocol::NotThisRadio));
    assert!(!e.retryable());

    let f = Fake::with(|s| s.no_source = true);
    let t0 = Instant::now();
    let e = drv(&f).open(&f.addr(), &t).err().expect("refused");
    assert!(
        matches!(&e, DriverError::Driver(x) if x.downcast_ref::<SdrppError>() == Some(&SdrppError::NoSource)),
        "{e:?}"
    );
    assert!(!e.retryable());
    assert!(
        t0.elapsed() < Duration::from_secs(2),
        "a final error is not retried"
    );
}

/// A server that accepts and stays silent: no answer, a retry.
#[test]
fn a_silent_server_has_no_answer() {
    let net = FakeTransport::new();
    net.serve(fake::HOST, fake::PORT, |s| {
        std::thread::sleep(Duration::from_millis(1500));
        drop(s);
    });
    let d = SdrppDriver::with_timing(
        Arc::new(net),
        Timing {
            connect: Duration::from_millis(400),
            greeting: Duration::from_millis(400),
            answer_open: Duration::from_millis(400),
            ..Timing::default()
        },
    );
    let mut src_err = None;
    let t0 = Instant::now();
    if let Err(e) = d.open("127.0.0.1:5259", &tune(2.2 * M, 100e6, None, &[])) {
        src_err = Some(e);
    }
    let e = src_err.expect("no answer");
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
    assert!(e.retryable());
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
}

/// The connect race: the driver waits for the rate before asking for the menu.
#[test]
fn the_connect_race_is_avoided() {
    let f = Fake::with(|s| s.race = true);
    let d = drv(&f);
    for _ in 0..3 {
        let mut src = d
            .open(&f.addr(), &tune(2.2 * M, 100e6, None, &[]))
            .expect("open");
        src.stop();
        assert!(f.wait(2.0, |s| !s.client));
    }
    let (mut src, mut r, _) = started(&d, &f, &tune(2.2 * M, 100e6, Some(10.0), &[]));
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    assert_eq!(f.state().broken, 0);
    src.stop();
}

/// An ERROR packet keeps the connection and shows in the status.
#[test]
fn an_error_packet_is_shown() {
    let f = Fake::with(|s| s.unknown = vec![cmd::SET_COMPRESSION]);
    let d = drv(&f);
    let (mut src, mut r, _) = started(&d, &f, &tune(2.2 * M, 100e6, None, &[]));
    let mut l = Vec::new();
    assert!(wait_until(3.0, || {
        let r = readings(&mut src);
        l.extend(r.log_lines().iter().cloned());
        r.fields().get(&SERVER_ERROR) == Some("invalid command")
    }));
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    l.extend(logs(&mut src));
    assert!(
        l.iter()
            .any(|x| x.contains("reports an error: invalid command")),
        "{l:?}"
    );
    src.stop();
}

/// Compressed samples, and a corrupt frame dropped with one log line.
#[test]
fn compressed_samples() {
    let f = Fake::start();
    let d = drv(&f);
    let (mut src, mut r, _) = started(
        &d,
        &f,
        &tune(2.2 * M, 100e6, None, &[("compression", Value::Bool(true))]),
    );
    assert!(f.state().compression);
    let got = read(&mut r, 20_000, 3.0);
    assert!(all_near(&got, 0.5, -0.25, 1e-4), "{}", got.len());
    f.state().corrupt_frames = 3;
    assert!(f.wait(3.0, |s| s.corrupt_frames == 0));
    r.skip_all();
    let got = read(&mut r, 20_000, 3.0);
    assert!(all_near(&got, 0.5, -0.25, 1e-4), "{}", got.len());
    let l = logs(&mut src);
    assert_eq!(
        l.iter().filter(|x| x.contains("cannot be decoded")).count(),
        1,
        "{l:?}"
    );
    src.stop();
}

/// A rate change made on the server requires a restart.
#[test]
fn an_unprompted_rate_change_restarts() {
    let f = Fake::start();
    let d = drv(&f);
    let (mut src, mut r, s) = started(&d, &f, &tune(2.2 * M, 100e6, None, &[]));
    assert_eq!(s.rate_hz, 2.5e6);
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    logs(&mut src);
    f.state().push_rate = Some(10e6);
    let mut res = results();
    assert!(wait_until(3.0, || src.poll(&mut res).restart));
    assert!(res
        .log_lines()
        .iter()
        .any(|x| x.contains("changed its rate to 10 Msps")));
    src.stop();
}

/// A server whose radio sends nothing after START: a stall ends the connection and the driver reconnects.
#[test]
fn a_stall_reconnects() {
    let f = Fake::start();
    let d = SdrppDriver::with_timing(
        f.transport(),
        Timing {
            stall: Duration::from_millis(600),
            ..Timing::default()
        },
    );
    let (mut src, mut r, _) = started(&d, &f, &tune(2.2 * M, 100e6, None, &[]));
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    f.state().throttle = 0.0;
    assert!(f.wait(6.0, |s| s.accepts >= 2), "{:?}", f.state().log);
    let l = logs(&mut src);
    assert!(l.iter().any(|x| x.contains("sends no samples")), "{l:?}");
    src.stop();
}

/// After a reconnect, a retune goes to the new connection.
#[test]
fn a_retune_after_a_reconnect_reaches_the_server() {
    let f = Fake::start();
    let d = drv(&f);
    let (mut src, mut r, _) = started(&d, &f, &tune(2.2 * M, 100e6, Some(10.0), &[]));
    assert!(!read(&mut r, 1000, 3.0).is_empty());
    f.state().drop_now = true;
    assert!(f.wait(3.0, |s| s.accepts >= 2));
    assert!(f.wait(5.0, |s| s.running));
    live(src.apply(&tune(2.2 * M, 101e6, Some(10.0), &[])));
    assert!(
        f.wait(3.0, |s| s.freq.round() == 101e6),
        "{:?}",
        f.state().log
    );
    src.stop();
}

#[test]
fn server_controls_are_settings_of_the_open_radio() {
    let f = Fake::start();
    let kind = DriverKind::new(Box::new(drv(&f)));
    let mut radio = kind
        .open(&f.addr(), &tune(2.5 * M, 100e6, Some(10.0), &[]))
        .expect("open");
    let schema = radio.schema().clone();
    let gm = schema.id("server.##_airspy_gm_Airspy Source").unwrap();
    let bias = schema.id("server.Bias T##_airspy_Airspy Source").unwrap();
    assert_eq!(gm, DESCRIPTOR.run_time_id(2));
    assert!(
        schema.id(&format!("server.{SR_ID}")).is_err(),
        "RATE drives the rate list"
    );
    assert!(
        schema.id(&format!("server.{LIN_ID}")).is_err(),
        "GAIN drives the gain sliders"
    );
    radio.apply(gm, &Value::Choice(2)).unwrap();
    radio.apply(bias, &Value::Bool(true)).unwrap();
    assert!(
        f.wait(3.0, |s| (s.gain_mode, s.bias) == (2, true)),
        "{:?}",
        f.state()
    );
    radio.stop();
}
