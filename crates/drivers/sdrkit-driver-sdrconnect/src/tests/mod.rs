//! Tests against the fake SDRconnect server of `fake.rs`, on a fake network.

mod fake;

use crate::*;
use fake::{frame, iq_message, push, Cfg, Fake, Mode};
use sdrkit_core::net::{FakeTransport, NetError};
use sdrkit_core::{Complex32, Owner, SettingId, Value};
use sdrkit_driver_core::ws::UpgradeError;
use sdrkit_driver_core::{ring, DriverKind, RingReader, RingStats};
use std::time::Instant;

const MASK: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

fn timing() -> Timing {
    Timing {
        connect: Duration::from_secs(2),
        handshake: Duration::from_millis(800),
        confirm: Duration::from_millis(400),
        stall: Duration::from_millis(1500),
        tick: Duration::from_millis(20),
        poll: Duration::from_millis(200),
        window: Duration::from_millis(300),
        open_attempts: 1,
        fixed_mask: Some(MASK),
        fixed_key: Some(*b"the sample nonce"),
    }
}

fn drv(f: &Fake) -> SdrconnectDriver {
    SdrconnectDriver::with_timing(f.transport(), timing())
}

/// The values of a radio at `centre` and `rate`, then `settings` by key.
fn tune(rate: Hz, centre: Hz, settings: &[(&str, Value)]) -> Values {
    let kind = DriverKind::new(Box::new(SdrconnectDriver::new(Arc::new(
        FakeTransport::new(),
    ))));
    let schema = kind.schema();
    let mut v = Values::defaults(schema);
    v.set(schema.id("centre_hz").unwrap(), &Value::Hz(centre))
        .unwrap();
    v.set(schema.id("rate").unwrap(), &Value::Hz(rate)).unwrap();
    for (k, x) in settings {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    v
}

fn text(t: &str) -> Value {
    Value::Text(t.into())
}

/// The settings that `src` adds once SDRconnect reported its antennas: it polls until they are there.
fn added(src: &mut dyn Source) -> Vec<sdrkit_core::SettingDecl> {
    let mut r = results();
    let end = Instant::now() + Duration::from_secs(5);
    while src.settings().is_empty() && Instant::now() < end {
        src.poll(&mut r);
        std::thread::sleep(Duration::from_millis(10));
    }
    src.settings()
}

/// `v` with the settings that `src` added when it opened, and its antenna set to `name`.
fn with_antenna(src: &mut dyn Source, v: &Values, name: &str) -> Values {
    let mut decls = v.schema().settings().to_vec();
    decls.extend(added(src));
    let schema = Arc::new(sdrkit_core::Schema::new(decls).unwrap());
    let mut out = Values::defaults(&schema);
    for (i, _) in v.schema().settings().iter().enumerate() {
        let id = SettingId(i as u16);
        out.set(id, v.value(id).unwrap()).unwrap();
    }
    let id = schema.id("antenna").unwrap();
    let at = schema.setting(id).unwrap().field.choices.iter().position(|c| c.key == name).unwrap();
    out.set(id, &Value::Choice(at as u16)).unwrap();
    out
}

fn tuner(t: Tuner) -> Value {
    Value::Choice(sdrkit_core::Choice::index(t))
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

fn read_n(r: &mut RingReader, n: usize, secs: f64) -> Vec<Complex32> {
    let mut v = vec![Complex32::default(); n];
    let mut got = 0;
    let t = Instant::now();
    while got < n && t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(20));
        got += r.read_iq(&mut v[got..]);
    }
    v.truncate(got);
    v
}

fn start(src: &mut Box<dyn Source>, capacity: usize) -> (Started, RingReader, Arc<RingStats>) {
    let (w, r) = ring(capacity, src.sample_type());
    let stats = w.stats();
    (src.start(w).unwrap(), r, stats)
}

const PRIMARY: Complex32 = Complex32::new(0.5, -0.5);
const SECONDARY: Complex32 = Complex32::new(-0.25, 0.75);

fn results() -> Results {
    Results::new(DESCRIPTOR.decl().results().clone())
}

/// Log lines and restart requests gathered over several polls.
#[derive(Default)]
struct Notes {
    lines: Vec<String>,
    restart: bool,
}

impl Notes {
    fn pull(&mut self, s: &mut Box<dyn Source>) -> &Self {
        let mut r = results();
        self.restart |= s.poll(&mut r).restart;
        self.lines.extend(r.log_lines().iter().cloned());
        self
    }
    fn has(&self, text: &str) -> bool {
        self.lines.iter().any(|l| l.contains(text))
    }
}

/// The readings of `s` now.
fn readings(s: &mut Box<dyn Source>) -> Results {
    let mut r = results();
    s.poll(&mut r);
    r
}

fn refused(id: SettingId) -> DriverError {
    DriverError::Refused { id }
}

/// The error of this crate inside `e`.
fn own(e: &DriverError) -> Option<SdrconnectError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<SdrconnectError>().copied(),
        _ => None,
    }
}

#[test]
fn the_open_sends_the_tuner_the_questions_and_the_settings_in_order() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(
            &f.addr(),
            &tune(250_000.0, 104.9e6, &[("lna_state", Value::I64(5))]),
        )
        .unwrap();
    assert_eq!(src.sample_type(), SampleType::Cs16);
    let (st, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!((st.rate_hz, st.centre_hz), (250_000.0, 104.9e6));
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    let lines = f.lines(1);
    let mut want: Vec<String> = vec![
        "set_primary_device_enable  true -".into(),
        "set_secondary_device_enable  false -".into(),
    ];
    for p in [
        "api_version",
        "active_device",
        "can_control",
        "started",
        "device_sample_rate",
        "device_center_frequency",
        "lna_state_min",
        "lna_state_max",
        "lna_state",
        "valid_antennas",
        "active_antenna",
        "overload",
    ] {
        want.push(format!("get_property {p}  primary"));
    }
    want.push("set_property device_sample_rate 250000 primary".into());
    want.push("set_property device_center_frequency 104900000 primary".into());
    want.push("set_property lna_state 5 primary".into());
    want.push("iq_stream_enable  true -".into());
    assert_eq!(&lines[..want.len()], &want[..]);
    // Nothing is requested that receiving does not need, and the radio was already started.
    assert!(!lines.iter().any(|l| l.starts_with("device_stream_enable")));
    let caps = src.capabilities().clone();
    // The ranges are those of a radio, not placeholders.
    let placeholders = sdrkit_driver_core::form::check_capabilities(&caps);
    assert!(placeholders.is_empty(), "{placeholders:?}");
    assert_eq!(
        caps.name,
        format!("SDRconnect: RSPduo 1806030000 at {}", f.addr())
    );
    assert_eq!(caps.gain_db, None);
    assert_eq!(caps.max_freq_hz, 2e9);
    src.stop();
    // The close: the IQ stream off, then a close frame.
    assert!(
        wait_for(
            || f.lines(1)
                .last()
                .is_some_and(|l| l == "iq_stream_enable  false -"),
            2.0
        ),
        "{:?}",
        f.lines(1)
    );
}

#[test]
fn the_upgrade_request_and_the_frames_follow_rfc_6455() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    // The fake fails on an unmasked client frame, so samples mean every frame was masked.
    assert_eq!(read_n(&mut r, 2000, 3.0).len(), 2000);
    // A ping is answered and the stream goes on.
    f.send(frame(9, b"ab"));
    assert_eq!(read_n(&mut r, 2000, 3.0).len(), 2000);
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn the_settings_and_what_the_server_reports() {
    let schema = DESCRIPTOR.decl().schema().clone();
    let lna = schema.setting(own_id(SdrconnectSetting::Lna)).unwrap();
    assert_eq!(lna.range.map(|r| (r.min, r.max)), Some((-1.0, 255.0)));
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let r = readings(&mut src);
    let fl = r.fields();
    assert_eq!((fl.get(&LNA_MIN), fl.get(&LNA_MAX)), (Some(0), Some(9)));
    assert_eq!(fl.get(&API_VERSION), Some("1.0.3"));
    src.stop();
}

#[test]
fn the_rspduo_tuners_are_told_apart() {
    for (t, want) in [(Tuner::Primary, PRIMARY), (Tuner::Secondary, SECONDARY)] {
        let f = Fake::start(Cfg::default());
        let mut src = drv(&f)
            .open(&f.addr(), &tune(100_000.0, 7.1e6, &[("tuner", tuner(t))]))
            .unwrap();
        let (_, mut r, _) = start(&mut src, 1 << 16);
        let s = read_n(&mut r, 3000, 3.0);
        assert_eq!(s.len(), 3000, "{t:?}");
        assert!(s.iter().all(|c| *c == want), "{t:?}: {:?}", s[0]);
        assert_eq!(
            *f.st.enabled.lock().unwrap(),
            [t == Tuner::Primary, t == Tuner::Secondary]
        );
        let lines = f.lines(1);
        assert!(lines.contains(&format!(
            "set_property device_center_frequency 7100000 {}",
            t.name()
        )));
        if t == Tuner::Secondary {
            assert!(src.capabilities().name.ends_with(" (secondary tuner)"));
        }
        src.stop();
    }
}

#[test]
fn the_secondary_tuner_needs_api_1_0_2() {
    let mut cfg = Cfg::default();
    for t in cfg.props.iter_mut() {
        t.insert("api_version".into(), "1.0.1".into());
    }
    let f = Fake::start(cfg);
    let e = drv(&f)
        .open(
            &f.addr(),
            &tune(100_000.0, 7.1e6, &[("tuner", tuner(Tuner::Secondary))]),
        )
        .err()
        .unwrap();
    assert_eq!(
        own(&e),
        Some(SdrconnectError::ApiTooOld {
            got: (1, 0, 1),
            need: (1, 0, 2)
        })
    );
    assert!(!e.retryable());
    // The primary tuner works on the same server.
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 7.1e6, &[]))
        .unwrap();
    src.stop();
}

/// SDRconnect keeps its rate without a message (an RSPduo with two tuners runs 2 Msps at most): the stream runs at the
/// rate SDRconnect has, and the refused rate is not asked again.
#[test]
fn a_live_rate_change_that_sdrconnect_does_not_take_keeps_its_rate() {
    let f = Fake::start(Cfg {
        read_only: vec!["device_sample_rate".into()],
        ..Cfg::default()
    });
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (st, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(st.rate_hz, 100_000.0);
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    let asked = "set_property device_sample_rate 200000 primary".to_string();
    for _ in 0..2 {
        match src.apply(&tune(200_000.0, 100e6, &[])).unwrap() {
            Retuned::Live(s) => assert_eq!(s.rate_hz, 100_000.0),
            r => panic!("{r:?}"),
        }
    }
    assert_eq!(f.lines(1).iter().filter(|l| **l == asked).count(), 1);
    let mut notes = Notes::default();
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    assert!(!notes.pull(&mut src).restart);
    src.stop();
}

#[test]
fn a_rate_change_is_sent_live_and_followed() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (st, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(st.rate_hz, 100_000.0);
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    // The stack requests a new rate: sent live, and the stream continues.
    match src.apply(&tune(200_000.0, 100e6, &[])).unwrap() {
        Retuned::Live(s) => assert_eq!(s.rate_hz, 200_000.0),
        r => panic!("{r:?}"),
    }
    assert!(wait_for(
        || f.lines(1)
            .contains(&"set_property device_sample_rate 200000 primary".to_string()),
        2.0
    ));
    let mut notes = Notes::default();
    assert_eq!(read_n(&mut r, 4000, 3.0).len(), 4000);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !notes.pull(&mut src).restart,
        "its own change is no restart"
    );
    // SDRconnect changes the rate by itself: followed, with a restart.
    f.send(push(
        "property_changed",
        "device_sample_rate",
        "500000",
        "primary",
    ));
    assert!(wait_for(|| notes.pull(&mut src).restart, 3.0));
    assert!(notes.has("SDRconnect changed its sample rate to 0.5 Msps"));
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn a_rate_that_sdrconnect_does_not_take_is_noted() {
    let cfg = Cfg {
        rate_answer: Some("125000".into()),
        ..Default::default()
    };
    let f = Fake::start(cfg);
    let mut src = drv(&f)
        .open(&f.addr(), &tune(110_000.0, 100e6, &[]))
        .unwrap();
    let mut notes = Notes::default();
    let (st, _r, _) = start(&mut src, 1 << 16);
    assert_eq!(st.rate_hz, 125_000.0);
    assert!(notes
        .pull(&mut src)
        .has("SDRconnect runs 0.125 Msps, 0.11 Msps were asked"));
    src.stop();
}

#[test]
fn keeping_the_server_rate_sends_no_rate() {
    let f = Fake::start(Cfg::default());
    let server = Value::Choice(sdrkit_core::Choice::index(RateSource::Server));
    let mut src = drv(&f)
        .open(&f.addr(), &tune(2e6, 100e6, &[("rate_source", server)]))
        .unwrap();
    let (st, _r, _) = start(&mut src, 1 << 16);
    assert_eq!(st.rate_hz, 100_000.0);
    assert!(!f
        .lines(1)
        .iter()
        .any(|l| l.contains("device_sample_rate 2000000")));
    let caps = src.capabilities();
    assert_eq!(caps.rates.ranges, vec![(100_000.0, 100_000.0)]);
    src.stop();
}

#[test]
fn a_dropped_connection_is_reconnected_with_the_settings() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 14.2e6, &[]))
        .unwrap();
    let t = with_antenna(&mut *src, &tune(100_000.0, 14.2e6, &[]), "Tuner 1 Hi-Z");
    src.apply(&t).unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    f.drop_client();
    assert!(wait_for(|| f.conns() >= 2, 6.0));
    assert!(wait_for(
        || f.lines(2)
            .iter()
            .any(|l| l.starts_with("iq_stream_enable  true")),
        4.0
    ));
    let l2 = f.lines(2);
    assert!(
        l2.contains(&"set_property device_center_frequency 14200000 primary".to_string()),
        "{l2:?}"
    );
    assert!(
        l2.contains(&"set_property active_antenna Tuner 1 Hi-Z primary".to_string()),
        "{l2:?}"
    );
    let _ = read_n(&mut r, 1 << 17, 0.3);
    assert_eq!(read_n(&mut r, 2000, 3.0).len(), 2000);
    assert!(src.loss().unwrap().link_down > 0);
    src.stop();
}

#[test]
fn malformed_messages_are_counted_and_the_stream_goes_on() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    for t in [
        "not json",
        "[1, 2]",
        r#"{"property":"lna_state","value":"1"}"#,
        r#"{"event_type":"property_changed","property":"lna_state","value":"1","device":"third"}"#,
        r#"{"event_type":"property_changed","property":"lna_state","value":"many"}"#,
        "{“event_type”: “property_changed”}",
    ] {
        f.send_text(t);
    }
    f.send(frame(2, &[2])); // under 2 bytes
    f.send(frame(2, &[2, 0, 1, 2, 3, 4, 5])); // one sample and 2 bytes over
                                              // Ignored: an unknown event, an unused property, audio, spectrum, an unknown type, the IQ of the other tuner.
    f.send_text(r#"{"event_type":"hello","property":"","value":""}"#);
    f.send(push("property_changed", "rds_ps", "TRIPLE M", "primary"));
    f.send(frame(2, &[1, 0, 0, 0, 0, 0]));
    f.send(frame(2, &[3, 0, 7, 7, 7]));
    f.send(frame(2, &[9, 0, 1, 1]));
    f.send(iq_message(5, (1, 1), 10));
    assert!(wait_for(
        || {
            let r = readings(&mut src);
            r.fields().get(&MALFORMED).is_some_and(|n| n >= 7)
                && r.fields().get(&IGNORED).is_some_and(|n| n >= 6)
        },
        3.0
    ));
    assert_eq!(read_n(&mut r, 3000, 3.0).len(), 3000);
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn bad_frames_drop_the_connection_and_it_is_reopened() {
    for raw in [vec![0x81, 0x81, 1, 2, 3, 4, 0x41], vec![0xc1, 0x01, 0x41]] {
        let f = Fake::start(Cfg::default());
        let mut src = drv(&f)
            .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
            .unwrap();
        let (_, mut r, _) = start(&mut src, 1 << 18);
        assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
        f.send(raw);
        let mut notes = Notes::default();
        assert!(wait_for(|| notes.pull(&mut src).has("Malformed"), 3.0));
        assert!(wait_for(|| f.conns() >= 2, 6.0));
        src.stop();
    }
}

#[test]
fn a_message_above_1_mib_drops_the_connection_and_it_is_reopened() {
    // A whole IQ message of 1 MiB + 2 bytes, and a header that declares 2^40 bytes.
    let whole = iq_message(2, (16384, -16384), (1 << 20) / 4);
    let mut declared = vec![0x82, 127];
    declared.extend_from_slice(&(1u64 << 40).to_be_bytes());
    declared.extend_from_slice(&[2, 0, 0, 0]);
    for raw in [whole, declared] {
        let f = Fake::start(Cfg::default());
        let mut src = drv(&f)
            .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
            .unwrap();
        let (_, mut r, _) = start(&mut src, 1 << 18);
        assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
        f.send(raw);
        let mut notes = Notes::default();
        assert!(wait_for(|| notes.pull(&mut src).has("TooLong"), 3.0));
        assert!(wait_for(|| f.conns() >= 2, 6.0));
        src.stop();
    }
}

#[test]
fn a_slow_host_drops_and_counts_and_stays_connected() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f).open(&f.addr(), &tune(2e6, 100e6, &[])).unwrap();
    let (_, _r, stats) = start(&mut src, 4096);
    // Nobody reads the ring: 20 000 samples come every 10 ms.
    assert!(wait_for(|| stats.cpu_lost() > 100_000, 5.0));
    assert!(stats.drop_events() > 0);
    assert_eq!(src.loss().unwrap().radio_lost, 0);
    assert_eq!(f.conns(), 1);
    src.stop();
}

#[test]
fn settings_out_of_range_are_refused() {
    let f = Fake::start(Cfg::default());
    let lna = own_id(SdrconnectSetting::Lna);
    // The fake reports an LNA range of 0 to 9.
    let e = drv(&f)
        .open(
            &f.addr(),
            &tune(100_000.0, 100e6, &[("lna_state", Value::I64(12))]),
        )
        .err()
        .unwrap();
    assert_eq!(e, refused(lna));
    assert!(!e.retryable());
    let d = drv(&f);
    let mut src = d.open(&f.addr(), &tune(100_000.0, 100e6, &[])).unwrap();
    // Once the report is known, a retune with a bad value is refused before anything is sent.
    let e = src
        .apply(&tune(100_000.0, 100e6, &[("lna_state", Value::I64(10))]))
        .err()
        .unwrap();
    assert_eq!(e, refused(lna));
    // The antenna is a choice of the names that SDRconnect listed: no other name can be set.
    let listed: Vec<String> = added(&mut *src)[0].field.choices.iter().map(|c| c.key.to_string()).collect();
    assert!(listed.len() > 1 && listed[0].is_empty(), "{listed:?}");
    src.stop();
    // The schema refuses values of the wrong kind or out of its range.
    let schema = DESCRIPTOR.decl().schema().clone();
    let mut v = Values::defaults(&schema);
    for (k, x) in [
        ("tuner", Value::Choice(2)),
        ("lna_state", Value::I64(-5)),
        ("wss", text("maybe")),
    ] {
        assert!(v.set(schema.id(k).unwrap(), &x).is_err(), "{k}");
    }
    // A pin that is not a SHA-256 digest is refused at the open.
    let e = drv(&f)
        .open(
            &f.addr(),
            &tune(100_000.0, 100e6, &[("tls_pin", text("12"))]),
        )
        .err()
        .unwrap();
    assert_eq!(e, refused(own_id(SdrconnectSetting::TlsPin)));
}

#[test]
fn without_control_nothing_is_set() {
    let mut cfg = Cfg::default();
    cfg.props[0].insert("can_control".into(), "false".into());
    let f = Fake::start(cfg);
    let mut src = drv(&f)
        .open(
            &f.addr(),
            &tune(250_000.0, 104.9e6, &[("lna_state", Value::I64(5))]),
        )
        .unwrap();
    let mut notes = Notes::default();
    let (st, mut r, _) = start(&mut src, 1 << 16);
    assert!(notes
        .pull(&mut src)
        .has("does not give this client control"));
    assert_eq!((st.rate_hz, st.centre_hz), (100_000.0, 100e6));
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    assert!(!f.lines(1).iter().any(|l| l.starts_with("set_property")));
    src.stop();
}

#[test]
fn readings_show_what_sdrconnect_reports() {
    let f = Fake::start(Cfg::default());
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 18);
    let _ = read_n(&mut r, 1000, 3.0);
    // The power and SNR are requested on each poll.
    assert!(wait_for(
        || readings(&mut src).fields().get(&SIGNAL_SNR).is_some(),
        3.0
    ));
    let res = readings(&mut src);
    assert_eq!(res.fields().get(&SIGNAL_POWER), Some(-71.5));
    assert_eq!(res.fields().get(&SIGNAL_SNR), Some(12.25));
    f.send(push("property_changed", "overload", "true", "primary"));
    assert!(wait_for(
        || readings(&mut src).fields().get(&OVERLOAD) == Some(true),
        3.0
    ));
    let res = readings(&mut src);
    assert_eq!(res.fields().get(&LNA_STATE), Some(3));
    assert_eq!(res.fields().get(&API_VERSION), Some("1.0.3"));
    assert!(wait_for(
        || readings(&mut src).fields().get(&MBIT_S).is_some(),
        3.0
    ));
    src.stop();
}

#[test]
fn a_stopped_radio_is_started_and_stopped_again() {
    let mut cfg = Cfg::default();
    for t in cfg.props.iter_mut() {
        t.insert("started".into(), "false".into());
    }
    let f = Fake::start(cfg);
    let mut src = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    src.stop();
    assert!(f
        .lines(1)
        .contains(&"device_stream_enable  true -".to_string()));
    assert!(
        wait_for(
            || f.lines(1)
                .last()
                .is_some_and(|l| l == "device_stream_enable  false -"),
            2.0
        ),
        "{:?}",
        f.lines(1)
    );
}

#[test]
fn servers_that_are_not_sdrconnect_fail_as_they_should() {
    let f = Fake::start(Cfg {
        mode: Mode::Silent,
        ..Default::default()
    });
    let e = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .err()
        .unwrap();
    assert_eq!(own(&e), Some(SdrconnectError::NoProperties));
    let f = Fake::start(Cfg {
        mode: Mode::NotWebSocket,
        ..Default::default()
    });
    let e = drv(&f)
        .open(&f.addr(), &tune(100_000.0, 100e6, &[]))
        .err()
        .unwrap();
    assert_eq!(
        e,
        DriverError::Handshake(UpgradeError::Refused { status: Some(404) })
    );
    assert!(!e.retryable());
    let e = drv(&f)
        .open("127.0.0.1:1", &tune(100_000.0, 100e6, &[]))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Net(NetError::Refused));
    assert!(e.retryable());
    assert_eq!(
        parse_address("pc.local").unwrap(),
        ("pc.local".into(), 5454)
    );
    assert!(parse_address("h:x").is_err());
}

#[test]
fn wss_works_with_the_pinned_certificate_and_not_with_another() {
    use rustls::pki_types::pem::PemObject;
    let der = rustls::pki_types::CertificateDer::from_pem_slice(fake::CERT).unwrap();
    let pin: String = sdrkit_algo_crypto::sha256(&der)
        .iter()
        .map(|b| format!("{b:02x}:"))
        .collect();
    let f = Fake::start(Cfg {
        tls: true,
        ..Default::default()
    });
    let wss = |pin: &str| {
        tune(
            100_000.0,
            100e6,
            &[("wss", Value::Bool(true)), ("tls_pin", text(pin))],
        )
    };
    let mut src = drv(&f)
        .open(&f.addr(), &wss(pin.trim_end_matches(':')))
        .unwrap();
    let (_, mut r, _) = start(&mut src, 1 << 16);
    assert_eq!(read_n(&mut r, 2000, 3.0).len(), 2000);
    src.stop();
    let e = drv(&f)
        .open(&f.addr(), &wss(&"00".repeat(32)))
        .err()
        .unwrap();
    assert_eq!(e, DriverError::Net(NetError::Untrusted));
    assert!(!e.retryable());
    // Without a pin, TLS is the job of the transport. The fake network has none.
    let e = drv(&f).open(&f.addr(), &wss("")).err().unwrap();
    assert_eq!(e, DriverError::Net(NetError::TlsUnavailable));
}

#[test]
fn both_rspduo_tuners_stream_at_once_without_mixing_their_properties() {
    let f = Fake::start(Cfg::default());
    let d = drv(&f);
    let mut a = d.open(&f.addr(), &tune(100_000.0, 7.1e6, &[])).unwrap();
    let mut b = d
        .open(
            &f.addr(),
            &tune(100_000.0, 14.2e6, &[("tuner", tuner(Tuner::Secondary))]),
        )
        .unwrap();
    let (_, mut ra, _) = start(&mut a, 1 << 16);
    let (_, mut rb, _) = start(&mut b, 1 << 16);
    let sa = read_n(&mut ra, 3000, 3.0);
    let sb = read_n(&mut rb, 3000, 3.0);
    assert!(sa.len() == 3000 && sa.iter().all(|c| *c == PRIMARY));
    assert!(sb.len() == 3000 && sb.iter().all(|c| *c == SECONDARY));
    assert_eq!(readings(&mut a).fields().get(&SERVER_CENTRE), Some(7.1e6));
    assert_eq!(readings(&mut b).fields().get(&SERVER_CENTRE), Some(14.2e6));
    assert_eq!(f.conns(), 2);
    a.stop();
    b.stop();
}
