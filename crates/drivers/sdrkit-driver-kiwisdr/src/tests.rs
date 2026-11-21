//! Driver tests against a fake KiwiSDR on a fake network.

mod fake;

use super::*;
use crate::proto::url_decode;
use fake::{iq_payload, msg, server_frame, Fake, Mode, Switch, FRAME_SAMPLE};
use sdrkit_core::SettingSet;
use sdrkit_core::{Choice, CommandError, CommandSet, NetError, Owner, Record, SettingId, Value};
use sdrkit_driver_core::ws::UpgradeError;
use sdrkit_driver_core::{
    ring, DriverKind, Polled, RadioSetting, Retuned, RingReader, Started, CENTRE,
};
use std::time::{Duration, Instant};

// ---- Helpers ----

fn timing() -> Timing {
    Timing {
        first_pause: Duration::from_millis(80),
        max_pause: Duration::from_millis(400),
        stall: Duration::from_secs(3),
        reconnect_gap: Duration::from_millis(20),
        handshake: Duration::from_secs(3),
        ..Timing::default()
    }
}

fn driver(fake: &Fake) -> KiwiDriver {
    KiwiDriver::with_timing(
        Arc::new(fake.net()),
        Clock::fixed(1_790_000_000.0),
        timing(),
    )
}

/// The values of a radio at `centre` with the settings `set`.
fn tune(centre: f64, set: &[(&str, Value)]) -> Values {
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = Values::defaults(&schema);
    v.set(schema.id("centre_hz").unwrap(), &Value::Hz(centre))
        .unwrap();
    for (key, value) in set {
        v.set(schema.id(key).unwrap(), value).unwrap();
    }
    v
}

fn text(s: &str) -> Value {
    Value::Text(s.into())
}

fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

fn wait(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + timeout;
    while Instant::now() < end {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

fn kiwi(e: &DriverError) -> Option<KiwiError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<KiwiError>().cloned(),
        _ => None,
    }
}

/// Runs the test of the connection of the kind of `drv` on `address`, and returns the row of the receiver.
fn probe(drv: &KiwiDriver, address: &str, password: &str) -> Result<Record, DriverError> {
    let mut kind = DriverKind::new(Box::new(drv.clone()));
    let commands = KiwiKindCommand::commands();
    let id = KiwiKindCommand::Probe.id();
    let mut given = commands.def(id).unwrap().new_args();
    given.set(&ADDRESS.field, address);
    given.set(&TEST_PASSWORD.field, password);
    let args = commands.check(id, &given).unwrap();
    if let Err(e) = kind.invoke(id, &args) {
        let CommandError::Failed { source } = e else {
            panic!("{e}")
        };
        return Err(source.downcast_ref::<DriverError>().unwrap().clone());
    }
    let t = kind.results_decl().table_id(RECEIVERS.key).unwrap();
    let row = kind
        .results()
        .table(t)
        .unwrap()
        .rows_newest_first()
        .next()
        .unwrap()
        .1
        .clone();
    Ok(row)
}

fn refused(s: KiwiSetting) -> DriverError {
    DriverError::Refused {
        id: sdrkit_driver_core::own_id(s),
    }
}

struct Running {
    src: Box<dyn Source>,
    rx: RingReader,
    started: Started,
    results: Results,
}

impl Running {
    /// Polls the radio and returns every log line kept so far.
    fn logs(&mut self) -> String {
        self.poll();
        self.results
            .log_lines()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn poll(&mut self) -> Polled {
        self.src.poll(&mut self.results)
    }
    fn status(&mut self) -> String {
        self.poll();
        self.results.status().summary.clone()
    }
    fn get<K: sdrkit_core::record::Kind>(&mut self, f: &Field<K>) -> Option<K::Out<'_>> {
        self.poll();
        self.results.fields().get(f)
    }
    fn count(&mut self, f: &Field<field::I64>) -> i64 {
        self.get(f).unwrap_or(0)
    }
    /// Reads up to `n` samples, as (I, Q) pairs.
    fn read(&mut self, n: usize, timeout: Duration) -> Vec<(i16, i16)> {
        let mut out = Vec::with_capacity(n);
        let end = Instant::now() + timeout;
        while out.len() < n && Instant::now() < end {
            self.rx.wait(1, Duration::from_millis(20));
            let want = n - out.len();
            self.rx.read_with(want, |b| {
                out.extend(b.as_chunks::<4>().0.iter().map(|c| {
                    (
                        i16::from_le_bytes([c[0], c[1]]),
                        i16::from_le_bytes([c[2], c[3]]),
                    )
                }));
            });
        }
        out
    }
    fn drain(&mut self) -> usize {
        self.rx.skip_all()
    }
}

fn start(drv: &KiwiDriver, fake: &Fake, v: &Values) -> Running {
    let mut src = drv.open(&fake.addr(), v).expect("open");
    let (w, rx) = ring(1 << 20, src.sample_type());
    let started = src.start(w).expect("start");
    Running {
        src,
        rx,
        started,
        results: Results::new(DESCRIPTOR.decl().results().clone()),
    }
}

fn filtered(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter(|l| l != "SET keepalive" && l != "SET GET_USERS")
        .collect()
}

// ---- The handshake ----

#[test]
fn the_handshake_is_exactly_the_transcript() {
    // The sample_rate message of the server, byte for byte.
    assert_eq!(
        msg("sample_rate=12001.135"),
        [
            0x82, 0x19, 0x4d, 0x53, 0x47, 0x20, 0x73, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x5f, 0x72,
            0x61, 0x74, 0x65, 0x3d, 0x31, 0x32, 0x30, 0x30, 0x31, 0x2e, 0x31, 0x33, 0x35
        ]
    );
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7_074_000.0, &[]));
    assert_eq!(r.started.rate_hz, 12_001.135);
    assert_eq!(r.started.centre_hz, 7_074_000.0);
    assert!(wait(Duration::from_secs(2), || fake
        .lines(1, "SND")
        .iter()
        .any(|l| l == "SET GET_USERS")));
    let want = [
        "SET auth t=kiwi p=#",
        "SET AR OK in=12000 out=44100",
        "SET ident_user=sdrkit",
        "SET compression=0",
        "SET squelch=0 max=0",
        "SET genattn=0",
        "SET gen=0 mix=-1",
        "SET mod=iq low_cut=-5000 high_cut=5000 freq=7074.000",
        "SET agc=1 hang=0 thresh=-100 slope=6 decay=1000 manGain=50",
        "SET keepalive",
        "SET GET_USERS",
    ];
    let got = fake.lines(1, "SND");
    assert_eq!(&got[..want.len()], &want[..]);
    // The time stamp of the path comes from the host clock.
    assert_eq!(fake.paths()[0], "/kiwi/1790000000/SND");
    // Every client frame is masked.
    assert_eq!(
        fake.sh.unmasked.load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    // The samples reach the ring as the receiver sent them.
    let s = r.read(5_000, Duration::from_secs(3));
    assert_eq!(s.len(), 5_000);
    assert!(s.iter().all(|v| *v == FRAME_SAMPLE));
    assert_eq!(r.get(&VERSION).map(str::to_owned).as_deref(), Some("1.800"));
    assert_eq!(r.get(&SESSION_LIMIT), Some(30));
    assert_eq!(
        r.get(&RX_NAME).map(str::to_owned).as_deref(),
        Some("Fake Kiwi")
    );
    assert_eq!(r.get(&RX_POSITION), Some((51.5, -0.12)));
    assert_eq!(r.get(&RSSI), Some(-57.0));
    assert_eq!(r.get(&GPS), Some(true));
    assert!(r.get(&GPS_TIME).is_some());
    assert_eq!((r.get(&LOWEST), r.get(&HIGHEST)), (Some(10e3), Some(30e6)));
    // The listener list is a table.
    assert!(wait(Duration::from_secs(2), || {
        r.poll();
        let id = r.results.decl().table_id("listeners").unwrap();
        r.results.table(id).is_some_and(|t| t.len() == 2)
    }));
    let id = r.results.decl().table_id("listeners").unwrap();
    let t = r.results.table(id).unwrap();
    let row = t.row("0").unwrap();
    assert_eq!(row.get(&L_NAME), Some("sdrkit"));
    assert_eq!(row.get(&L_FREQ), Some(7_074_000.0));
    assert_eq!(t.row("1").unwrap().get(&L_NAME), Some(""));
    let logs = r.logs();
    assert!(logs.contains("KiwiSDR: connected to 127.0.0.1"), "{logs}");
    assert!(logs.contains("version 1.800"));
    let st = r.status();
    assert!(
        st.starts_with("KiwiSDR 1.800, 12.0 ksps IQ, S9+16 (-57 dBm), GPS time"),
        "{st}"
    );
    let caps = r.src.capabilities().clone();
    assert_eq!(caps.name, format!("KiwiSDR {}", fake.addr()));
    assert_eq!(caps.max_freq_hz, 30e6);
    assert_eq!(caps.rates.ranges, vec![(12_001.135, 12_001.135)]);
    assert!(caps.has_agc && caps.transmit.is_none() && caps.realtime);
    assert_eq!(caps.gain_db, Some((0.0, 120.0, 1.0)));
    r.src.stop();
}

#[test]
fn passwords_and_names_are_url_encoded_and_never_shown() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let t = tune(
        7_094_000.0,
        &[
            ("password", text("s3cret pass")),
            ("tlimit_password", text("tl")),
            ("user_name", text("Ann B")),
            ("location", text("Oslo, NO")),
        ],
    );
    let mut r = start(&drv, &fake, &t);
    assert!(wait(Duration::from_secs(2), || fake.lines(1, "SND").len() >= 4));
    let got = fake.lines(1, "SND");
    assert_eq!(got[0], "SET auth t=kiwi p=s3cret%20pass ipl=tl");
    assert_eq!(got[2], "SET ident_user=Ann%20B");
    assert_eq!(got[3], "SET geoloc=Oslo%2C%20NO");
    r.read(2_000, Duration::from_secs(2));
    let mut all = r.logs();
    all.push_str(&r.status());
    all.push_str(&r.src.capabilities().name);
    all.push_str(&format!("{:?}", r.results));
    r.src.stop();
    all.push_str(&r.logs());
    assert!(!all.contains("s3cret"), "{all}");
    // A refusal with a password set does not show it either.
    let bad = Fake::new(Mode::BadPw);
    let e = driver(&bad).open(&bad.addr(), &t).err().unwrap();
    assert_eq!(kiwi(&e), Some(KiwiError::Password { set: true }));
    assert!(e.to_string().contains("wrong password") && !e.to_string().contains("s3cret"));
}

#[test]
fn a_live_change_sends_only_what_changed() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7_074_000.0, &[]));
    assert!(wait(Duration::from_secs(2), || fake.lines(1, "SND").len() >= 11));
    let before = filtered(fake.lines(1, "SND")).len();
    let t1 = tune(
        14_074_000.0,
        &[
            ("agc", Value::Bool(false)),
            ("gain", Value::F64(40.0)),
            ("passband", choice(Passband::P3000)),
        ],
    );
    match r.src.apply(&t1).unwrap() {
        Retuned::Live(s) => assert_eq!(s.centre_hz, 14_074_000.0),
        other => panic!("{other:?}"),
    }
    assert!(wait(Duration::from_secs(1), || filtered(
        fake.lines(1, "SND")
    )
    .len()
        >= before + 2));
    assert_eq!(
        &filtered(fake.lines(1, "SND"))[before..],
        [
            "SET mod=iq low_cut=-3000 high_cut=3000 freq=14074.000",
            "SET agc=0 hang=0 thresh=-100 slope=6 decay=1000 manGain=40"
        ]
    );
    r.src.apply(&t1).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(filtered(fake.lines(1, "SND")).len(), before + 2);
    // A new centre moves the receiver, and the stream centre with it.
    let t2 = tune(14_080_000.0, &[]);
    let Retuned::Live(s) = r.src.apply(&t2).unwrap() else {
        panic!()
    };
    assert_eq!(s.centre_hz, 14_080_000.0);
    assert!(wait(Duration::from_secs(1), || fake
        .lines(1, "SND")
        .iter()
        .any(|l| l.ends_with("freq=14080.000"))));
    // A dial outside the range of the receiver is refused.
    assert_eq!(
        r.src.apply(&tune(100e6, &[])),
        Err(DriverError::Refused {
            id: RadioSetting::Centre.id()
        })
    );
    // A setting of the next connect requires a restart.
    assert_eq!(
        r.src.apply(&tune(7e6, &[("user_name", text("Bo"))])),
        Ok(Retuned::NeedsRestart)
    );
    r.src.stop();
}

#[test]
fn swap_iq_takes_effect_live() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(
        &drv,
        &fake,
        &tune(7_094_000.0, &[("swap_iq", Value::Bool(true))]),
    );
    let s = r.read(3_000, Duration::from_secs(3));
    assert_eq!(s[2_000], (FRAME_SAMPLE.1, FRAME_SAMPLE.0));
    r.src.apply(&tune(7_094_000.0, &[])).unwrap();
    r.drain();
    std::thread::sleep(Duration::from_millis(200));
    r.drain();
    let s = r.read(3_000, Duration::from_secs(2));
    assert_eq!(s[2_000], FRAME_SAMPLE);
    r.src.stop();
}

// ---- Refusals at the first open ----

#[test]
fn refusals_at_the_first_open_are_final_and_make_one_connection() {
    for (mode, want) in [
        (Mode::Busy, Some(KiwiError::AllBusy { channels: Some(4) })),
        (Mode::BadPw, Some(KiwiError::Password { set: false })),
        (Mode::Down, Some(KiwiError::Down)),
        (Mode::Plain, None),
        (
            Mode::Redirect,
            Some(KiwiError::Redirect {
                to: "kiwi2.example.org:8074".into(),
            }),
        ),
        (Mode::Limit, Some(KiwiError::TimeLimit)),
        (Mode::WrongAccept, None),
    ] {
        let fake = Fake::new(mode);
        let drv = driver(&fake);
        let e = drv
            .open(&fake.addr(), &tune(7e6, &[]))
            .err()
            .expect("refused");
        assert!(!e.retryable(), "{mode:?}: {e}");
        match &want {
            Some(k) => assert_eq!(kiwi(&e).as_ref(), Some(k), "{mode:?}"),
            None => {
                let fault = if mode == Mode::Plain {
                    UpgradeError::Refused { status: Some(404) }
                } else {
                    UpgradeError::Mismatch
                };
                assert_eq!(e, DriverError::Handshake(fault), "{mode:?}");
            }
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(fake.conns(), 1, "{mode:?}");
        let p = probe(&drv, &fake.addr(), "").unwrap_err();
        assert_eq!(kiwi(&p), want, "probe {mode:?}");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(fake.conns(), 2, "probe {mode:?}");
    }
}

#[test]
fn the_probe_reads_the_version_and_streams_nothing() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let row = probe(&drv, &fake.addr(), "").unwrap();
    assert_eq!(
        (
            row.get(&R_ADDRESS),
            row.get(&R_VERSION),
            row.get(&R_CHANNELS)
        ),
        (Some(fake.addr().as_str()), Some("1.800"), Some(4))
    );
    assert!(wait(Duration::from_secs(1), || fake.open() == 0));
    assert_eq!(fake.conns(), 1);
    let lines = fake.lines(1, "SND");
    assert!(!lines.iter().any(|l| l.starts_with("SET mod")), "{lines:?}");
    assert!(
        lines.last().unwrap().starts_with("CLOSE [03, e8]"),
        "{lines:?}"
    );
}

#[test]
fn wrong_rates_silent_and_slow_servers_are_refused_with_their_reason() {
    for r in [48_000.0, 6_000.0] {
        let fake = Fake::with_rate(Mode::Normal, &r.to_string());
        let e = driver(&fake)
            .open(&fake.addr(), &tune(7e6, &[]))
            .err()
            .unwrap();
        assert_eq!(kiwi(&e), Some(KiwiError::NotKiwiRate { reported: Some(r) }));
        assert!(!e.retryable());
    }
    let fake = Fake::new(Mode::Silent);
    let t = Instant::now();
    let e = driver(&fake)
        .open(&fake.addr(), &tune(7e6, &[]))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
    assert!(t.elapsed() < Duration::from_secs(4));
    let fake = Fake::new(Mode::NoRate);
    let e = driver(&fake)
        .open(&fake.addr(), &tune(7e6, &[]))
        .err()
        .unwrap();
    assert_eq!(
        kiwi(&e),
        Some(KiwiError::Incomplete {
            awaited: Awaited::SampleRate
        })
    );
    assert!(e.to_string().contains("(no sample_rate)"), "{e}");
    // Nothing listens.
    let drv = driver(&fake);
    let e = drv.open("127.0.0.1:8074", &tune(7e6, &[])).err().unwrap();
    assert_eq!(e, DriverError::Net(NetError::Refused));
    // A host that does not answer.
    let quiet = Fake::new(Mode::Normal);
    quiet.net().silence("127.0.0.1");
    let drv = KiwiDriver::with_timing(
        Arc::new(quiet.net()),
        Clock::fixed(0.0),
        Timing {
            connect: Duration::from_millis(200),
            ..timing()
        },
    );
    let e = drv.open(&quiet.addr(), &tune(7e6, &[])).err().unwrap();
    assert_eq!(e, DriverError::Net(NetError::TimedOut));
    // Bad and secure addresses never touch the network.
    let n = quiet.net().connects();
    assert_eq!(
        drv.open("host:0", &tune(7e6, &[])).err(),
        Some(DriverError::Address)
    );
    let e = drv.open("https://x", &tune(7e6, &[])).err().unwrap();
    assert_eq!(kiwi(&e), Some(KiwiError::SecureAddress));
    assert_eq!(quiet.net().connects(), n);
}

#[test]
fn a_wide_channel_server_runs_at_its_own_rate() {
    let fake = Fake::with_rate(Mode::Normal, "20250");
    let drv = driver(&fake);
    let r = start(
        &drv,
        &fake,
        &tune(7e6, &[("passband", choice(Passband::Max))]),
    );
    assert_eq!(r.started.rate_hz, 20_250.0);
    assert!(wait(Duration::from_secs(1), || fake
        .lines(1, "SND")
        .iter()
        .any(|l| l.contains("low_cut=-10000 high_cut=10000"))));
    assert_eq!(
        r.src.capabilities().rates.ranges,
        vec![(20_250.0, 20_250.0)]
    );
    // A passband wider than the rate allows is refused.
    let narrow = Fake::with_rate(Mode::Normal, "9000");
    let e = driver(&narrow)
        .open(
            &narrow.addr(),
            &tune(7e6, &[("passband", choice(Passband::P5900))]),
        )
        .err()
        .unwrap();
    assert_eq!(e, refused(KiwiSetting::Passband));
}

#[test]
fn an_exact_rate_must_be_the_rate_of_the_receiver() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    let exact = |rate: f64| {
        tune(
            7e6,
            &[("rate", Value::Hz(rate)), ("rate_exact", Value::Bool(true))],
        )
    };
    assert_eq!(
        r.src.apply(&exact(48_000.0)),
        Err(DriverError::Refused {
            id: RadioSetting::Rate.id()
        })
    );
    assert!(r.src.apply(&exact(12_001.135)).is_ok());
    // A rate that is not exact is a wish: the graph changes the rate.
    assert!(r
        .src
        .apply(&tune(7e6, &[("rate", Value::Hz(48_000.0))]))
        .is_ok());
    assert_eq!(
        r.src.apply(&tune(7e6, &[("bias_tee", Value::Bool(true))])),
        Err(DriverError::Refused {
            id: RadioSetting::BiasTee.id()
        })
    );
    r.src.stop();
}

// ---- Junk and sequence numbers ----

#[test]
fn junk_does_not_break_the_stream() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(3_000, Duration::from_secs(2));
    fake.switch(Switch::Junk);
    std::thread::sleep(Duration::from_millis(300));
    let s = r.read(10_000, Duration::from_secs(3));
    assert_eq!(s.len(), 10_000);
    assert_eq!(fake.conns(), 1);
    let bad = r.count(&BAD);
    r.src.stop();
    assert!(bad >= 4, "{bad}");
    assert!(r.logs().contains("malformed frames ignored"));
}

#[test]
fn a_sequence_gap_is_filled_and_a_restart_is_not() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    let frames0 = r.count(&FRAMES);
    fake.switch(Switch::Skip5);
    assert!(wait(Duration::from_secs(2), || r.count(&MISSED) == 5));
    // The gap is counted before the frame itself: wait for both.
    assert!(wait(Duration::from_secs(1), || r.count(&FRAMES) > frames0));
    assert_eq!(r.count(&GAPS), 1);
    fake.switch(Switch::RestartSeq);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!((r.count(&MISSED), r.count(&GAPS)), (5, 1));
    // A repeat of the last frame is dropped and counted bad.
    let fr = iq_payload(0x08, 7, 700, &[(1, 1); 16]);
    fake.switch(Switch::Raw(
        [server_frame(2, &fr), server_frame(2, &fr)].concat(),
    ));
    std::thread::sleep(Duration::from_millis(300));
    let bad = r.count(&BAD);
    r.src.stop();
    assert!(bad >= 1);
}

#[test]
fn a_sequence_gap_feeds_exactly_the_missing_samples() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Silence);
    std::thread::sleep(Duration::from_millis(150));
    r.drain();
    let (missed, gaps) = (r.count(&MISSED), r.count(&GAPS));
    let n = 16;
    let fr = |seq: u32| server_frame(2, &iq_payload(0x08, seq, 700, &vec![(100, 200); n]));
    // A step backwards (a new start), the next frame, then a gap of 5 frames.
    let base = 0xF000_0000u32;
    fake.switch(Switch::Raw([fr(base), fr(base + 1), fr(base + 7)].concat()));
    let want = n + n + 5 * n + n;
    let s = r.read(want, Duration::from_secs(2));
    assert_eq!(s.len(), want);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(r.drain(), 0, "nothing more than the frames and the fill");
    assert_eq!(r.count(&MISSED) - missed, 5);
    assert_eq!(r.count(&GAPS) - gaps, 1);
    assert!(s[..2 * n].iter().all(|v| *v == (100, 200)));
    assert!(s[2 * n..7 * n].iter().all(|v| *v == (0, 0)));
    assert!(s[7 * n..].iter().all(|v| *v == (100, 200)));
    r.src.stop();
}

#[test]
fn the_iq_frame_replayed_sets_overflow_and_the_meter() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Silence);
    std::thread::sleep(Duration::from_millis(100));
    let mut raw = vec![0x82, 0x20];
    raw.extend_from_slice(&[
        0x53, 0x4e, 0x44, 0x0a, 0xfe, 0xff, 0xff, 0xff, 0x04, 0xf6, 0x01, 0x00, 0x40, 0xe2, 0x01,
        0x00, 0x00, 0x65, 0xcd, 0x1d, 0xff, 0xfe, 0x01, 0x2c, 0x40, 0x00, 0xe0, 0x00, 0x7f, 0xff,
        0x80, 0x00,
    ]);
    fake.switch(Switch::Raw(raw));
    assert!(wait(Duration::from_secs(1), || r.get(&OVERLOAD) == Some(true)));
    assert!(r.status().contains("ADC overload"));
    assert!(wait(Duration::from_secs(2), || r.get(&RSSI) == Some(0.0)));
    r.src.stop();
}

// ---- Keep-alive ----

#[test]
fn keepalives_come_once_a_second() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    let count = || {
        fake.lines(1, "SND")
            .iter()
            .filter(|l| *l == "SET keepalive")
            .count()
    };
    assert!(wait(Duration::from_secs(1), || count() > 0));
    let n0 = count();
    std::thread::sleep(Duration::from_millis(2200));
    let n = count() - n0;
    assert!((2..=4).contains(&n), "{n}");
    r.src.stop();
}

// ---- Stall, slow frames, oversize, time limit, drop ----

#[test]
fn messages_without_iq_stall_and_reconnect() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Stats);
    assert!(
        wait(Duration::from_secs(6), || r
            .logs()
            .contains("messages but no IQ")),
        "{}",
        r.logs()
    );
    assert!(wait(Duration::from_secs(3), || fake.conns() == 2));
    r.drain();
    assert_eq!(r.read(3_000, Duration::from_secs(3)).len(), 3_000);
    r.src.stop();
}

#[test]
fn audio_without_iq_stalls_with_its_own_reason() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Audio);
    assert!(
        wait(Duration::from_secs(6), || r
            .logs()
            .contains("audio frames but no IQ")),
        "{}",
        r.logs()
    );
    assert!(wait(Duration::from_secs(3), || fake.conns() == 2));
    r.src.stop();
}

#[test]
fn compressed_frames_without_iq_stall_with_the_audio_reason() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Silence);
    // Well inside the stall time, so something arrives in it.
    std::thread::sleep(Duration::from_millis(1500));
    let mut f = iq_payload(0x18, 1, 700, &[(1, 1); 64]);
    f[3] = 0x18;
    fake.switch(Switch::Raw(server_frame(2, &f)));
    assert!(
        wait(Duration::from_secs(6), || r
            .logs()
            .contains("audio frames but no IQ")),
        "{}",
        r.logs()
    );
    r.src.stop();
}

#[test]
fn a_silent_stream_stalls() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Silence);
    assert!(
        wait(Duration::from_secs(6), || r
            .logs()
            .contains("no IQ from the KiwiSDR for 3 s: no data at all")),
        "{}",
        r.logs()
    );
    r.src.stop();
}

#[test]
fn a_frame_split_across_a_long_gap_is_waited_for() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Split);
    std::thread::sleep(Duration::from_millis(1800));
    r.drain();
    assert_eq!(r.read(3_000, Duration::from_secs(2)).len(), 3_000);
    assert_eq!(fake.conns(), 1);
    assert!(
        !r.logs().contains("KiwiSDR 127.0.0.1:8073:"),
        "{}",
        r.logs()
    );
    r.src.stop();
}

#[test]
fn an_oversize_message_is_bad_data_and_the_reconnect_sets_up_again() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7_094_000.0, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Oversize);
    let too_long = DriverError::Protocol(sdrkit_driver_core::Protocol::TooLong).to_string();
    assert!(
        wait(Duration::from_secs(3), || r.logs().contains(&too_long)),
        "{}",
        r.logs()
    );
    assert!(wait(Duration::from_secs(3), || fake.lines(2, "SND").len() >= 9));
    let first = filtered(fake.lines(1, "SND"));
    let second = filtered(fake.lines(2, "SND"));
    assert_eq!(&first[..9], &second[..9]);
    assert!(
        wait(Duration::from_secs(2), || r
            .logs()
            .contains("reconnected after")),
        "{}",
        r.logs()
    );
    r.src.stop();
}

#[test]
fn a_time_limit_close_waits_the_hold_pause() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    let t0 = Instant::now();
    fake.switch(Switch::InactivityClose);
    assert!(wait(Duration::from_secs(3), || fake.conns() == 2));
    let gap = fake.sh.accepted_at.lock().unwrap()[1].duration_since(t0);
    assert!(gap > Duration::from_millis(300), "{gap:?}");
    assert!(
        r.logs()
            .contains("time limit of the KiwiSDR for one listener ran out"),
        "{}",
        r.logs()
    );
    r.src.stop();
}

#[test]
fn a_drop_reconnects_after_the_first_pause() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    let t0 = Instant::now();
    fake.switch(Switch::Drop);
    assert!(wait(Duration::from_secs(2), || fake.conns() == 2));
    let gap = fake.sh.accepted_at.lock().unwrap()[1].duration_since(t0);
    assert!(gap < Duration::from_millis(400), "{gap:?}");
    r.drain();
    assert_eq!(r.read(3_000, Duration::from_secs(3)).len(), 3_000);
    assert!(
        wait(Duration::from_secs(1), || r
            .logs()
            .contains("KiwiSDR: reconnected after")),
        "{}",
        r.logs()
    );
    assert!(r.src.loss().unwrap().link_down > 0);
    // A change while connected again is live.
    assert!(matches!(
        r.src.apply(&tune(7.1e6, &[])).unwrap(),
        Retuned::Live(_)
    ));
    assert!(wait(Duration::from_secs(1), || fake
        .lines(2, "SND")
        .iter()
        .any(|l| l.ends_with("freq=7100.000"))));
    r.src.stop();
}

#[test]
fn a_password_refusal_mid_stream_stops_reconnecting() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::Drop);
    assert!(wait(Duration::from_secs(2), || fake.conns() == 2));
    assert!(wait(Duration::from_secs(2), || fake
        .lines(2, "SND")
        .iter()
        .any(|l| l.starts_with("SET mod"))));
    std::thread::sleep(Duration::from_millis(200));
    fake.switch(Switch::Msg("badp=1".into()));
    std::thread::sleep(Duration::from_millis(2000));
    assert_eq!(fake.conns(), 2);
    let st = r.status();
    assert!(st.contains("needs a password"), "{st}");
    r.src.stop();
}

#[test]
fn a_stop_during_a_pause_is_quick_and_leaves_nothing_open() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    fake.switch(Switch::InactivityClose);
    assert!(wait(Duration::from_secs(2), || r
        .status()
        .contains("time limit")));
    let t = Instant::now();
    r.src.stop();
    assert!(
        t.elapsed() < Duration::from_millis(500),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(fake.conns(), 1);
    assert!(wait(Duration::from_secs(1), || fake.open() == 0));
    let st = r.status();
    assert!(st.contains("not connected"), "{st}");
}

#[test]
fn stop_closes_every_connection_with_a_close_frame() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let v = tune(
        7_074_000.0,
        &[("waterfall", Value::Bool(true)), ("wf_zoom", Value::I64(2))],
    );
    let mut r = start(&drv, &fake, &v);
    r.read(2_000, Duration::from_secs(2));
    assert!(wait(Duration::from_secs(2), || fake.open() == 2
        && !fake.lines(2, "W/F").is_empty()));
    r.src.stop();
    r.src.stop();
    assert!(wait(Duration::from_secs(1), || fake.open() == 0));
    let snd = fake.lines(1, "SND");
    assert!(snd.last().unwrap().starts_with("CLOSE [03, e8]"), "{snd:?}");
    assert!(r.status().contains("not connected"));
}

// ---- The waterfall ----

#[test]
fn the_waterfall_shares_the_session_and_follows_the_dial() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let wf = |centre: f64, zoom: i64| {
        tune(
            centre,
            &[
                ("waterfall", Value::Bool(true)),
                ("wf_zoom", Value::I64(zoom)),
            ],
        )
    };
    let mut r = start(&drv, &fake, &wf(7_074_000.0, 2));
    let raster = r.results.decl().raster_id("waterfall").unwrap();
    assert!(wait(Duration::from_secs(2), || {
        r.poll();
        r.results.raster(raster).is_some_and(|x| x.height > 0)
    }));
    let paths = fake.paths();
    let ts = |p: &str| p.split('/').nth(2).unwrap().to_string();
    assert_eq!(paths.len(), 2);
    assert!(paths[1].ends_with("/W/F"));
    assert_eq!(ts(&paths[0]), ts(&paths[1]));
    let lines = fake.lines(2, "W/F");
    assert_eq!(
        &lines[..6],
        [
            "SET auth t=kiwi p=#",
            "SET zoom=2 cf=7074.000",
            "SET maxdb=-10 mindb=-110",
            "SET wf_speed=1",
            "SET wf_comp=0",
            "SET keepalive"
        ]
    );
    let x = r.results.raster(raster).unwrap();
    assert_eq!(x.width, 1024);
    let last = x.row(x.height - 1).unwrap();
    assert_eq!((last[0], last[255]), (-255.0, 0.0));
    assert_eq!(x.x.step * 1024.0, 7.5e6);
    // A change of frequency moves the zoom line.
    r.src.apply(&wf(7_100_000.0, 2)).unwrap();
    assert!(wait(Duration::from_secs(1), || fake
        .lines(2, "W/F")
        .iter()
        .any(|l| l == "SET zoom=2 cf=7100.000")));
    // Zoom 0 is centred on the covered range.
    r.src.apply(&wf(7_100_000.0, 0)).unwrap();
    assert!(wait(Duration::from_secs(1), || fake
        .lines(2, "W/F")
        .iter()
        .any(|l| l == "SET zoom=0 cf=15000.000")));
    // The receiver stream never sees the commands of the waterfall.
    assert!(!fake
        .lines(1, "SND")
        .iter()
        .any(|l| l.starts_with("SET zoom")));
    r.src.stop();
    // Keep-alives also go on the waterfall stream.
    assert!(fake.lines(2, "W/F").iter().any(|l| l == "SET keepalive"));
}

#[test]
fn a_new_server_rate_after_a_reconnect_asks_for_a_restart() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[]));
    r.read(2_000, Duration::from_secs(2));
    // A tiny change keeps the old figure.
    fake.set_rate("12001.200");
    fake.switch(Switch::Drop);
    assert!(wait(Duration::from_secs(2), || fake.conns() == 2));
    std::thread::sleep(Duration::from_millis(200));
    assert!(!r.poll().restart);
    fake.set_rate("12100");
    fake.switch(Switch::Drop);
    assert!(wait(Duration::from_secs(3), || fake.conns() == 3));
    let mut restart = false;
    assert!(wait(Duration::from_secs(1), || {
        restart |= r.poll().restart;
        restart
    }));
    assert!(r.logs().contains("now reports an IQ rate of 12100 Hz"));
    assert_eq!(r.src.capabilities().rates.ranges[0].0, 12_100.0);
    r.src.stop();
}

#[test]
fn the_users_setting_is_live() {
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    let mut r = start(&drv, &fake, &tune(7e6, &[("users", Value::Bool(false))]));
    std::thread::sleep(Duration::from_millis(500));
    assert!(!fake.lines(1, "SND").iter().any(|l| l == "SET GET_USERS"));
    r.src.apply(&tune(7e6, &[])).unwrap();
    assert!(wait(Duration::from_secs(2), || fake
        .lines(1, "SND")
        .iter()
        .any(|l| l == "SET GET_USERS")));
    r.src.stop();
    assert!(fake.all_lines().iter().all(|l| !l.contains("camp")));
}

#[test]
fn the_schema_declares_every_setting() {
    let kind = DriverKind::new(Box::new(driver(&Fake::new(Mode::Normal))));
    let schema = kind.schema();
    let own: Vec<_> = schema.settings()[RadioSetting::SETTINGS.len()..]
        .iter()
        .map(|x| x.field.key.as_ref())
        .collect();
    assert_eq!(
        own,
        [
            "password",
            "tlimit_password",
            "user_name",
            "location",
            "passband",
            "swap_iq",
            "users",
            "waterfall",
            "wf_zoom"
        ]
    );
    let restart: Vec<_> = schema
        .settings()
        .iter()
        .filter(|x| x.restart)
        .map(|x| x.field.key.as_ref())
        .collect();
    assert_eq!(
        restart,
        [
            "password",
            "tlimit_password",
            "user_name",
            "location",
            "waterfall"
        ]
    );
    assert_eq!(kind.descriptor().key, "kiwisdr");
    assert_eq!(kind.descriptor().produces[0].sample, SampleType::Cs16);
    assert!(kind.decl().results().table_id("listeners").is_some());
    assert!(kind.decl().results().raster_id("waterfall").is_some());
    let v = Values::defaults(schema);
    assert_eq!(v.get(&USER_NAME), "sdrkit");
    // A zoom out of range never reaches the driver.
    let id: SettingId = schema.id("wf_zoom").unwrap();
    assert!(schema.check(id, &Value::I64(15)).is_err());
    // A name or location that is too long is refused.
    let fake = Fake::new(Mode::Normal);
    let drv = driver(&fake);
    for (key, n, s) in [
        ("user_name", 33, KiwiSetting::UserName),
        ("location", 65, KiwiSetting::Location),
    ] {
        let e = drv
            .open(&fake.addr(), &tune(7e6, &[(key, text(&"x".repeat(n)))]))
            .err()
            .unwrap();
        assert_eq!(e, refused(s));
    }
    assert_eq!(fake.conns(), 0);
    assert_eq!(url_decode(b"%41"), "A");
}

#[test]
fn an_open_radio_publishes_the_readings_as_an_owner() {
    let fake = Fake::new(Mode::Normal);
    let kind = DriverKind::new(Box::new(driver(&fake)));
    let mut radio = kind.open(&fake.addr(), &tune(7_074_000.0, &[])).unwrap();
    let (w, rx) = ring(1 << 16, radio.sample_type());
    radio.start(w).unwrap();
    assert!(wait(Duration::from_secs(2), || rx
        .wait(1, Duration::from_millis(10))
        > 0));
    let mut events = Vec::new();
    assert!(wait(Duration::from_secs(2), || {
        radio.poll(0.0, &mut events);
        radio.results().fields().get(&VERSION) == Some("1.800")
    }));
    assert_eq!(
        radio.results().fields().get(&FRAMES).map(|n| n > 0),
        Some(true)
    );
    // The frequency is narrowed to the covered range.
    let id = RadioSetting::Centre.id();
    assert!(Owner::apply(&mut radio, id, &Value::Hz(31e6)).is_err());
    assert_eq!(radio.values().unwrap().get(&CENTRE), 7_074_000.0);
    radio.stop();
}

#[test]
fn the_status_line_reads_the_meters() {
    let info = ServerInfo {
        connected: true,
        version_maj: Some(1),
        version_min: Some(800),
        sample_rate: Some(12_001.135),
        ..ServerInfo::default()
    };
    let m = info::Meters::default();
    m.set_rssi(-57.0, -57.0);
    m.gps_valid
        .store(true, std::sync::atomic::Ordering::Relaxed);
    m.set_link(0.41, 1.0);
    assert_eq!(
        status_line(&info, &m),
        "KiwiSDR 1.800, 12.0 ksps IQ, S9+16 (-57 dBm), GPS time, 0.41 Mbit/s"
    );
}
