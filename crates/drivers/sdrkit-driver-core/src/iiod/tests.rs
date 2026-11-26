//! Tests of iiod receive: the rules, the driver against the fake iiod server, and ignored tests against a real board.

use super::fake::{value, FakeIiod, FakeOpts};
use super::*;
use crate::gain::GainControl;
use crate::{ring, Retuned, RingReader};
use sdrkit_core::{Complex32, Results, Value, ValueKind};
use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::time::Instant;

const M: f64 = 1e6;
const RX: &str = "iio:device1 INPUT voltage0";

#[test]
fn buffer_sizes() {
    assert_eq!(buffer_samples(2.1 * M), 65_536);
    assert_eq!(buffer_samples(4.0 * M), 131_072);
    assert_eq!(buffer_samples(10.0 * M), 262_144);
    assert_eq!(buffer_samples(20.0 * M), 524_288);
    assert_eq!(buffer_samples(61.44 * M), 1_048_576);
}

#[test]
fn rates_bandwidths_and_addresses() {
    let t = |r: f64, chosen: bool| {
        Want {
            rate_hz: r,
            rate_chosen: chosen,
            ..Default::default()
        }
        .v()
    };
    assert_eq!(rate_wanted(&t(1.0 * M, false), false), 2.1 * M);
    assert_eq!(rate_wanted(&t(1.0 * M, false), true), 1.0 * M);
    assert_eq!(rate_wanted(&t(0.3 * M, false), true), 2.1 * M);
    assert_eq!(rate_wanted(&t(0.0, false), false), 2.5 * M);
    assert_eq!(rate_wanted(&t(80.0 * M, false), false), 61.44 * M);
    assert_eq!(rate_wanted(&t(1.0 * M, true), false), 1.0 * M);
    assert_eq!(fir_decimation(2.048 * M), Some(2));
    assert_eq!(fir_decimation(1.05 * M), Some(2));
    assert_eq!(fir_decimation(1.0 * M), Some(4));
    assert_eq!(fir_decimation(0.521 * M), Some(4));
    assert_eq!(fir_decimation(0.5 * M), None);
    assert_eq!(fir_decimation(2.1 * M), None);
    assert_eq!(rf_bandwidth(0.0, 2.4 * M), 2_280_000.0);
    assert_eq!(rf_bandwidth(0.0, 0.1 * M), 200_000.0);
    assert_eq!(rf_bandwidth(1.536e6, 2.4 * M), 1_536_000.0);
    assert_eq!(
        parse_address("ip:192.168.2.1"),
        Ok(("192.168.2.1".into(), 30431))
    );
    assert_eq!(
        parse_address("pluto.local:1234"),
        Ok(("pluto.local".into(), 1234))
    );
    assert_eq!(parse_address("usb:1.2.5"), Err(DriverError::Address));
    assert_eq!(
        (
            default_max_gain(100e6),
            default_max_gain(2e9),
            default_max_gain(5e9)
        ),
        (73.0, 71.0, 62.0)
    );
    assert_eq!(network_note(20.0 * M, 1, 80e6), None);
    let n = network_note(20.0 * M, 2, 160e6).unwrap();
    assert!(
        n.contains("1280 Mbit/s") && n.contains("about 14.625 Msps with both receivers"),
        "{n}"
    );
}

#[test]
fn every_board_setting_is_declared_and_a_board_refuses_what_it_lacks() {
    let keys: Vec<&str> = IiodSetting::SETTINGS.iter().map(|s| s.key()).collect();
    assert_eq!(
        keys,
        [
            "ppm",
            "lowratefir",
            "gainmode",
            "antenna",
            "receivers",
            "gainmode2",
            "gain2",
            "rxport",
            "quadtrack",
            "rfdctrack",
            "bbdctrack",
            "kbuffers",
            "bufsize",
            "protocol",
            "link",
            "iqformat",
            "readings_s"
        ]
    );
    let choices = |key: &str| -> Vec<String> {
        let s = DESCRIPTOR.decl().schema().clone();
        let id = s.id(key).unwrap();
        s.setting(id)
            .unwrap()
            .field
            .choices
            .iter()
            .map(|c| c.key.to_string())
            .collect()
    };
    assert_eq!(choices("rxport")[..3], ["auto", "A_BALANCED", "B_BALANCED"]);
    assert_eq!(
        choices("gainmode"),
        ["manual", "slow_attack", "fast_attack", "hybrid"]
    );
    // A board without Tezuka firmware refuses CS8.
    let srv = FakeIiod::start(FakeOpts::default());
    let e = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("iqformat", "cs8")]))
        .err();
    assert_eq!(
        e,
        Some(DriverError::Refused {
            id: own_id(IiodSetting::IqFormat)
        })
    );
}

// ---- Test builders: the target state as plain numbers, the driver settings as text ----

/// The target state of a radio in a test. [`Want::v`] makes the values of the iiod kind.
#[derive(Clone, Debug)]
pub(super) struct Want {
    pub centre_hz: f64,
    pub rate_hz: f64,
    pub bandwidth_hz: f64,
    pub gain_db: Option<f64>,
    pub rate_chosen: bool,
    pub settings: BTreeMap<String, String>,
}

impl Default for Want {
    fn default() -> Self {
        Want {
            centre_hz: 100e6,
            rate_hz: 2.4e6,
            bandwidth_hz: 0.0,
            gain_db: None,
            rate_chosen: false,
            settings: BTreeMap::new(),
        }
    }
}

/// `settings` (key and text) set into `v`. A choice is given by its key.
pub(super) fn with_text(mut v: Values, settings: &BTreeMap<String, String>) -> Values {
    let schema = v.schema().clone();
    for (k, t) in settings {
        let id = schema.id(k).unwrap_or_else(|_| panic!("no setting {k}"));
        let decl = schema.setting(id).unwrap();
        let value = match decl.field.kind {
            ValueKind::Bool => Value::Bool(matches!(t.as_str(), "1" | "true" | "on" | "yes")),
            ValueKind::F64 => Value::F64(t.parse().unwrap()),
            ValueKind::Hz => Value::Hz(t.parse().unwrap()),
            ValueKind::I64 => Value::I64(t.parse().unwrap()),
            ValueKind::Choice => Value::Choice(
                decl.field
                    .choices
                    .iter()
                    .position(|c| c.key == t.as_str())
                    .unwrap_or_else(|| panic!("{k} has no choice {t}")) as u16,
            ),
            _ => Value::Text(t.clone()),
        };
        v.set(id, &value)
            .unwrap_or_else(|e| panic!("{k} = {t}: {e:?}"));
    }
    v
}

impl Want {
    /// The values of a radio of the iiod kind.
    pub fn v(&self) -> Values {
        let schema = DESCRIPTOR.decl().schema().clone();
        let mut v = Values::defaults(&schema);
        let mut set = |s: crate::RadioSetting, x: Value| v.set(s.id(), &x).unwrap();
        set(crate::RadioSetting::Centre, Value::Hz(self.centre_hz));
        set(crate::RadioSetting::Rate, Value::Hz(self.rate_hz));
        set(crate::RadioSetting::Bandwidth, Value::Hz(self.bandwidth_hz));
        set(
            crate::RadioSetting::Agc,
            Value::Bool(self.gain_db.is_none()),
        );
        set(
            crate::RadioSetting::Gain,
            Value::F64(self.gain_db.unwrap_or(30.0)),
        );
        set(
            crate::RadioSetting::RateExact,
            Value::Bool(self.rate_chosen),
        );
        with_text(v, &self.settings)
    }
}

/// What a source told the host, for the tests: every log line, and the status line.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Notice {
    Log(String),
    Warning(String),
}

/// Test helpers that call a source with a `Want`.
pub(super) trait WantSource {
    fn retune(&mut self, w: &Want) -> Result<Retuned, DriverError>;
    fn transmitter_w(
        &mut self,
        t: &super::tx_tests::TxWant,
    ) -> Result<Box<dyn crate::Transmitter>, DriverError>;
    fn take_notices(&mut self) -> Vec<Notice>;
    fn readings(&mut self) -> Results;
}

impl WantSource for Box<dyn Source> {
    fn retune(&mut self, w: &Want) -> Result<Retuned, DriverError> {
        self.apply(&w.v())
    }
    fn transmitter_w(
        &mut self,
        t: &super::tx_tests::TxWant,
    ) -> Result<Box<dyn crate::Transmitter>, DriverError> {
        self.transmitter(&t.v())
    }
    fn take_notices(&mut self) -> Vec<Notice> {
        let mut r = crate::testing::results(&DESCRIPTOR);
        self.poll(&mut r);
        let mut out: Vec<Notice> = r.log_lines().iter().cloned().map(Notice::Log).collect();
        let s = r.status().summary.clone();
        if !s.is_empty() {
            out.push(Notice::Warning(s));
        }
        for line in r.log_lines() {
            out.push(Notice::Warning(line.clone()));
        }
        out
    }
    fn readings(&mut self) -> Results {
        let mut r = crate::testing::results(&DESCRIPTOR);
        self.poll(&mut r);
        r
    }
}

/// Test helper that opens the driver with a `Want`.
pub(super) trait WantDriver {
    fn open_w(&self, address: &str, w: &Want) -> Result<Box<dyn Source>, DriverError>;
}

impl WantDriver for IiodDriver {
    fn open_w(&self, address: &str, w: &Want) -> Result<Box<dyn Source>, DriverError> {
        self.open(address, &w.v())
    }
}

/// A driver on the network of the operating system, for the ignored tests against a real board.
fn real_driver() -> IiodDriver {
    IiodDriver::new(Arc::new(sdrkit_core::net::StdNet))
}

pub(super) fn tune(rate: f64, settings: &[(&str, &str)]) -> Want {
    let mut t = Want {
        centre_hz: 100e6,
        rate_hz: rate,
        gain_db: Some(30.0),
        ..Default::default()
    };
    for (k, v) in settings {
        t.settings.insert(k.to_string(), v.to_string());
    }
    t
}

pub(super) fn wait_for(mut what: impl FnMut() -> bool, secs: f64) -> bool {
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

fn expect(ch: usize, k: usize) -> f32 {
    value(ch, (k % 1000) as i16) as f32 / 2048.0
}

/// The writes at open, in order, for one receiver on `ch` at 2.4 Msps, 100 MHz, 30 dB.
fn open_writes(ch: &str) -> Vec<String> {
    let rx = format!("iio:device1 INPUT {ch}");
    vec![
        format!("{rx} sampling_frequency=2400000"),
        format!("{rx} rf_bandwidth=2280000"),
        format!("{rx} quadrature_tracking_en=1"),
        format!("{rx} rf_dc_offset_tracking_en=1"),
        format!("{rx} bb_dc_offset_tracking_en=1"),
        "iio:device1 OUTPUT altvoltage0 frequency=100000000".to_string(),
        format!("{rx} gain_control_mode=manual"),
        format!("{rx} hardwaregain=30"),
    ]
}

#[test]
fn opens_configures_in_order_and_streams() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    assert_eq!(src.capabilities().name, "Fake AD9361 board");
    assert!(src.capabilities().transmit.is_some());
    assert_eq!(src.layout(), StreamLayout::Single);
    assert_eq!(
        (
            src.capabilities().min_freq_hz,
            src.capabilities().max_freq_hz
        ),
        (70e6, 6e9)
    );
    assert_eq!(srv.board.log(), open_writes("voltage0"));
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    let s = src.start(w).unwrap();
    assert_eq!((s.rate_hz, s.centre_hz), (2.4 * M, 100e6));
    let got = read_n(&mut r, 1000, 3.0);
    assert_eq!(got.len(), 1000);
    for (i, c) in got.iter().enumerate() {
        assert_eq!(*c, Complex32::new(expect(0, i), expect(1, i)), "sample {i}");
    }
    let log = srv.board.log();
    assert!(
        log.contains(&"SET iio:device4 BUFFERS_COUNT 8".to_string()),
        "{log:?}"
    );
    assert!(
        log.contains(&"OPEN iio:device4 65536 00000003".to_string()),
        "{log:?}"
    );
    assert!(!log.contains(&"BINARY".to_string()));
    src.stop();
    src.stop();
}

#[test]
fn nothing_is_left_open_after_stop() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    // Control, stream and monitor.
    assert!(
        wait_for(|| srv.open_connections() == 3, 3.0),
        "{}",
        srv.open_connections()
    );
    let t = Instant::now();
    src.stop();
    assert!(
        t.elapsed() < Duration::from_secs(2),
        "stop took {:?}",
        t.elapsed()
    );
    assert!(
        wait_for(|| srv.open_connections() == 0, 2.0),
        "{} left open",
        srv.open_connections()
    );
    // A source dropped without stop closes too.
    let src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    assert_eq!(srv.open_connections(), 1);
    drop(src);
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn live_retunes_and_a_rate_change_restarts_the_buffer() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert_eq!(read_n(&mut r, 1000, 3.0).len(), 1000);
    let n = srv.board.log().len();
    let mut t = tune(2.4 * M, &[]);
    t.centre_hz = 433.92e6;
    // A gain above the 73 dB of the board is refused, not clamped.
    t.gain_db = Some(80.0);
    assert_eq!(
        src.retune(&t).err(),
        Some(DriverError::Refused {
            id: crate::RadioSetting::Gain.id()
        })
    );
    t.gain_db = Some(73.0);
    assert!(matches!(src.retune(&t).unwrap(), Retuned::Live(s) if s.centre_hz == 433.92e6));
    assert_eq!(
        srv.board.log()[n..],
        [
            "iio:device1 OUTPUT altvoltage0 frequency=433920000".to_string(),
            format!("{RX} hardwaregain=73")
        ]
    );
    // The radio's AGC: written once, then the gain is left alone.
    t.settings.insert("gainmode".into(), "slow_attack".into());
    src.retune(&t).unwrap();
    t.gain_db = Some(10.0);
    src.retune(&t).unwrap();
    // A tracking switch and the port change live.
    t.settings.insert("bbdctrack".into(), "0".into());
    t.settings.insert("rxport".into(), "B_BALANCED".into());
    src.retune(&t).unwrap();
    assert_eq!(
        srv.board.log()[n + 2..],
        [
            format!("{RX} gain_control_mode=slow_attack"),
            format!("{RX} bb_dc_offset_tracking_en=0"),
            format!("{RX} rf_port_select=B_BALANCED"),
        ]
    );
    // A new rate: the stream stops, the radio is configured, a new (bigger) buffer opens.
    t.rate_hz = 10.0 * M;
    let Retuned::Live(s) = src.retune(&t).unwrap() else {
        panic!()
    };
    assert_eq!(s.rate_hz, 10.0 * M);
    assert!(
        wait_for(|| srv.board.has("OPEN iio:device4 262144 00000003"), 3.0),
        "{:?}",
        srv.board.log()
    );
    assert!(srv.board.has(&format!("{RX} sampling_frequency=10000000")));
    r.skip_all();
    assert_eq!(read_n(&mut r, 300_000, 3.0).len(), 300_000);
    assert_eq!(*srv.board.streams.lock().unwrap(), 2);
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn rx2_needs_two_receivers() {
    let one = FakeIiod::start(FakeOpts::default());
    for s in [
        ("antenna", "rx2"),
        ("receivers", "both"),
        ("receivers", "interleaved"),
    ] {
        let e = IiodDriver::new(one.transport())
            .open_w(&one.addr(), &tune(2.4 * M, &[s]))
            .err()
            .unwrap();
        assert!(
            matches!(e, DriverError::Refused { id } if id == own_id(IiodSetting::Antenna)
                || id == own_id(IiodSetting::Receivers)),
            "{e:?}"
        );
    }
    let two = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(two.transport())
        .open_w(&two.addr(), &tune(2.4 * M, &[("antenna", "rx2")]))
        .unwrap();
    assert_eq!(two.board.log(), open_writes("voltage1"));
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let got = read_n(&mut r, 100, 3.0);
    assert!(
        two.board.has("OPEN iio:device4 65536 0000000c"),
        "{:?}",
        two.board.log()
    );
    assert_eq!(got[7], Complex32::new(expect(2, 7), expect(3, 7)));
    src.stop();
}

/// Polls `radio` and tells whether it raised [`crate::RESTART`].
fn restart_raised(radio: &mut crate::OpenRadio) -> bool {
    let mut events = Vec::new();
    radio.poll(0.0, &mut events);
    events.iter().any(|e| e.is(&crate::RESTART))
}

#[test]
fn a_new_antenna_takes_effect_at_the_restart_it_requests() {
    use sdrkit_core::Owner;
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let kind = crate::testing::kind(IiodDriver::new(srv.transport()));
    let mut radio = kind.open(&srv.addr(), &tune(2.4 * M, &[]).v()).unwrap();
    let antenna = radio.schema().id(ANTENNA.key()).unwrap();
    let (w, mut r) = ring(1 << 22, radio.sample_type());
    radio.start(w).unwrap();
    assert_eq!(
        read_n(&mut r, 100, 3.0)[7],
        Complex32::new(expect(0, 7), expect(1, 7))
    );
    assert!(!restart_raised(&mut radio));
    // RX2, then RX1 again: each change requests one restart, and the new stream reads the chosen receiver, with its gain,
    // at the same tuning.
    for (choice, chan, pair, mask) in [
        (1, "voltage1", 2, "0000000c"),
        (0, "voltage0", 0, "00000003"),
    ] {
        let n = srv.board.log().len();
        radio.apply(antenna, &Value::Choice(choice)).unwrap();
        assert_eq!(
            radio.values().unwrap().value(antenna),
            Some(&Value::Choice(choice))
        );
        assert!(restart_raised(&mut radio));
        assert!(!restart_raised(&mut radio));
        radio.stop();
        let (w, mut r) = ring(1 << 22, radio.sample_type());
        let s = radio.start(w).unwrap();
        assert_eq!((s.rate_hz, s.centre_hz), (2.4 * M, 100e6));
        let got = read_n(&mut r, 100, 3.0);
        assert_eq!(got[7], Complex32::new(expect(pair, 7), expect(pair + 1, 7)));
        let log = srv.board.log()[n..].to_vec();
        for line in [
            format!("iio:device1 INPUT {chan} gain_control_mode=manual"),
            format!("iio:device1 INPUT {chan} hardwaregain=30"),
            format!("OPEN iio:device4 65536 {mask}"),
        ] {
            assert!(log.contains(&line), "{line} not in {log:?}");
        }
    }
    // The value in use again: no restart.
    radio.apply(antenna, &Value::Choice(0)).unwrap();
    assert!(!restart_raised(&mut radio));
    radio.stop();
}

#[test]
fn a_new_iq_format_or_receiver_count_gives_the_next_stream_its_sample_type_and_layout() {
    use sdrkit_core::Owner;
    let srv = FakeIiod::start(FakeOpts {
        fw: "v0.1-tezuka-r2",
        two_receivers: true,
        ..Default::default()
    });
    let kind = crate::testing::kind(IiodDriver::new(srv.transport()));
    let mut radio = kind.open(&srv.addr(), &tune(2.4 * M, &[]).v()).unwrap();
    let (w, _r) = ring(1 << 22, radio.sample_type());
    radio.start(w).unwrap();
    // CS8: the next stream reads 8-bit samples into a ring of that type.
    let format = radio.schema().id(IQ_FORMAT.key()).unwrap();
    radio.apply(format, &Value::Choice(1)).unwrap();
    assert!(restart_raised(&mut radio));
    assert_eq!(radio.sample_type(), SampleType::Cs8);
    radio.stop();
    let (w, mut r) = ring(1 << 22, SampleType::Cs8);
    radio.start(w).unwrap();
    assert_eq!(
        read_n(&mut r, 100, 3.0)[5],
        Complex32::new(5.0 / 128.0, -5.0 / 128.0)
    );
    assert!(srv.board.has("OPEN iio:device4 65536 00000001"));
    // Back to CS16, then both receivers as two streams.
    radio.apply(format, &Value::Choice(0)).unwrap();
    let receivers = radio.schema().id(RECEIVERS.key()).unwrap();
    radio.apply(receivers, &Value::Choice(1)).unwrap();
    assert!(restart_raised(&mut radio));
    assert_eq!(radio.sample_type(), SampleType::Cs16);
    assert_eq!(radio.layout(), StreamLayout::Separate(2));
    radio.stop();
    let (w1, mut r1) = ring(1 << 22, SampleType::Cs16);
    let (w2, mut r2) = ring(1 << 22, SampleType::Cs16);
    radio.start_streams(vec![w1, w2]).unwrap();
    assert_eq!(
        read_n(&mut r1, 100, 3.0)[7],
        Complex32::new(expect(0, 7), expect(1, 7))
    );
    assert_eq!(
        read_n(&mut r2, 100, 3.0)[7],
        Complex32::new(expect(2, 7), expect(3, 7))
    );
    radio.stop();
}

#[test]
fn a_board_with_one_receiver_refuses_rx2_at_the_set_and_keeps_streaming() {
    use sdrkit_core::Owner;
    let srv = FakeIiod::start(FakeOpts::default());
    let kind = crate::testing::kind(IiodDriver::new(srv.transport()));
    let mut radio = kind.open(&srv.addr(), &tune(2.4 * M, &[]).v()).unwrap();
    let (w, mut r) = ring(1 << 22, radio.sample_type());
    radio.start(w).unwrap();
    assert_eq!(read_n(&mut r, 100, 3.0).len(), 100);
    for key in [ANTENNA.key(), RECEIVERS.key()] {
        let id = radio.schema().id(key).unwrap();
        assert_eq!(
            radio.apply(id, &Value::Choice(1)),
            Err(sdrkit_core::SettingError::NotPermitted { id })
        );
        assert_eq!(radio.values().unwrap().value(id), Some(&Value::Choice(0)));
        assert!(!restart_raised(&mut radio));
    }
    // Still RX1, on the first stream: I below 1000 counts and Q = -I.
    r.skip_all();
    let got = read_n(&mut r, 100, 3.0);
    assert_eq!(got.len(), 100);
    assert!(
        got.iter().all(|c| c.im == -c.re && c.re * 2048.0 < 1000.0),
        "{:?}",
        &got[..4]
    );
    assert_eq!(*srv.board.streams.lock().unwrap(), 1);
    // A restart opens with the values in use, not the refused ones.
    radio.stop();
    let (w, mut r) = ring(1 << 22, radio.sample_type());
    radio.start(w).unwrap();
    assert_eq!(
        read_n(&mut r, 100, 3.0)[7],
        Complex32::new(expect(0, 7), expect(1, 7))
    );
    radio.stop();
}

#[test]
fn both_receivers_as_two_streams_with_their_own_gains() {
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let t = tune(
        2.4 * M,
        &[
            ("receivers", "both"),
            ("gainmode2", "manual"),
            ("gain2", "12"),
        ],
    );
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .unwrap();
    assert_eq!(src.layout(), StreamLayout::Separate(2));
    let log = srv.board.log();
    for l in [
        "iio:device1 INPUT voltage0 gain_control_mode=manual",
        "iio:device1 INPUT voltage0 hardwaregain=30",
        "iio:device1 INPUT voltage1 gain_control_mode=manual",
        "iio:device1 INPUT voltage1 hardwaregain=12",
    ] {
        assert!(log.contains(&l.to_string()), "{l} missing from {log:?}");
    }
    let (w1, mut r1) = ring(1 << 22, SampleType::Cs16);
    let (w2, mut r2) = ring(1 << 22, SampleType::Cs16);
    assert!(src.start_streams(vec![]).is_err());
    let three = vec![
        ring(16, SampleType::Cs16).0,
        ring(16, SampleType::Cs16).0,
        ring(16, SampleType::Cs16).0,
    ];
    assert!(src.start_streams(three).is_err());
    let s = src.start_streams(vec![w1, w2]).unwrap();
    assert_eq!(s.rate_hz, 2.4 * M);
    let a = read_n(&mut r1, 2000, 3.0);
    let b = read_n(&mut r2, 2000, 3.0);
    assert!(
        srv.board.has("OPEN iio:device4 65536 0000000f"),
        "{:?}",
        srv.board.log()
    );
    assert_eq!((a.len(), b.len()), (2000, 2000));
    for i in 0..2000 {
        assert_eq!(a[i], Complex32::new(expect(0, i), expect(1, i)), "RX1 {i}");
        assert_eq!(b[i], Complex32::new(expect(2, i), expect(3, i)), "RX2 {i}");
    }
    // RX2 follows RX1's gain when set to "same".
    let mut t2 = t.clone();
    t2.settings.insert("gainmode2".into(), "same".into());
    t2.gain_db = Some(40.0);
    let n = srv.board.log().len();
    src.retune(&t2).unwrap();
    assert_eq!(
        srv.board.log()[n..],
        [
            "iio:device1 INPUT voltage0 hardwaregain=40".to_string(),
            "iio:device1 INPUT voltage1 hardwaregain=40".to_string()
        ]
    );
    assert_eq!((r1.stats().cpu_lost(), r2.stats().cpu_lost()), (0, 0));
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn a_two_receiver_board_runs_at_most_30_72_msps() {
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(61.44 * M, &[]))
        .unwrap();
    assert_eq!(src.capabilities().rates.max_hz, 30.72 * M);
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    let s = src.start(w).unwrap();
    assert_eq!(s.rate_hz, 30.72 * M);
    let n = src.take_notices();
    assert!(
        n.iter()
            .any(|n| matches!(n, Notice::Warning(t) if t.contains("at most 30.72 Msps"))),
        "{n:?}"
    );
    src.stop();
}

#[test]
fn both_receivers_interleaved_in_one_ring() {
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("receivers", "interleaved")]))
        .unwrap();
    assert_eq!(src.layout(), StreamLayout::Interleaved(2));
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    let s = src.start(w).unwrap();
    assert_eq!(s.rate_hz, 4.8 * M, "the ring's rate");
    let got = read_n(&mut r, 2000, 3.0);
    for i in 0..1000 {
        assert_eq!(got[2 * i], Complex32::new(expect(0, i), expect(1, i)));
        assert_eq!(got[2 * i + 1], Complex32::new(expect(2, i), expect(3, i)));
    }
    src.stop();
}

#[test]
fn a_widened_buffer_is_picked_apart() {
    // Another client requested all four channels: the server sends them all to everyone.
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        widen: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let got = read_n(&mut r, 1000, 3.0);
    for (i, c) in got.iter().enumerate() {
        assert_eq!(*c, Complex32::new(expect(0, i), expect(1, i)), "sample {i}");
    }
    assert!(src
        .take_notices()
        .iter()
        .any(|n| matches!(n, Notice::Log(t) if t.contains("widened"))));
    src.stop();
}

#[test]
fn the_binary_protocol_is_used_when_offered() {
    let srv = FakeIiod::start(FakeOpts {
        binary: true,
        version: "1.0.abcdef",
        two_receivers: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(
            &srv.addr(),
            &tune(2.4 * M, &[("receivers", "both"), ("kbuffers", "4")]),
        )
        .unwrap();
    let (w1, mut r1) = ring(1 << 22, SampleType::Cs16);
    let (w2, mut r2) = ring(1 << 22, SampleType::Cs16);
    src.start_streams(vec![w1, w2]).unwrap();
    let a = read_n(&mut r1, 140_000, 3.0);
    let b = read_n(&mut r2, 140_000, 3.0);
    assert_eq!((a.len(), b.len()), (140_000, 140_000));
    for i in (0..140_000).step_by(997) {
        assert_eq!(a[i], Complex32::new(expect(0, i), expect(1, i)), "RX1 {i}");
        assert_eq!(b[i], Complex32::new(expect(2, i), expect(3, i)), "RX2 {i}");
    }
    let log = srv.board.log();
    // cf-ad9361-lpc is the 4th device of the context (number 3); four blocks of 65536 x 8 bytes.
    for l in [
        "BINARY",
        "B OPEN dev3 mask f",
        "B BLOCK 0 524288",
        "B BLOCK 3 524288",
        "B OP 19",
    ] {
        assert!(log.contains(&l.to_string()), "{l} missing from {log:?}");
    }
    assert!(!log.iter().any(|l| l.starts_with("OPEN")), "{log:?}");
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
    // "Text only" does not ask.
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("protocol", "text")]))
        .unwrap();
    let n = srv.board.log().iter().filter(|l| *l == "BINARY").count();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    assert_eq!(srv.board.log().iter().filter(|l| *l == "BINARY").count(), n);
    src.stop();
}

#[test]
fn a_dropped_binary_stream_reconnects() {
    let srv = FakeIiod::start(FakeOpts {
        binary: true,
        drop_stream_after: Some(3),
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(
        wait_for(|| *srv.board.streams.lock().unwrap() >= 2, 4.0),
        "{:?}",
        srv.board.log()
    );
    r.skip_all();
    assert!(!read_n(&mut r, 1000, 2.0).is_empty());
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn a_refused_frequency_is_final_with_the_ad9364_hint() {
    let srv = FakeIiod::start(FakeOpts {
        lo_range: "[325000000 1 3800000000]",
        ..Default::default()
    });
    let mut t = tune(2.4 * M, &[]);
    t.centre_hz = 100e6;
    let start = Instant::now();
    let e = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .err()
        .unwrap();
    assert!(
        start.elapsed() < Duration::from_millis(400),
        "a final error is not retried"
    );
    assert_eq!(
        e,
        DriverError::Refused {
            id: crate::RadioSetting::Centre.id()
        }
    );
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn the_low_rate_fir_and_the_clock_correction() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(
            &srv.addr(),
            &tune(2.048 * M, &[("lowratefir", "1"), ("ppm", "10")]),
        )
        .unwrap();
    assert_eq!(
        srv.board.log()[..5],
        [
            format!("{RX} sampling_frequency=4096000"),
            "iio:device1 filter_fir_config=<fir RX 3 GAIN 0 DEC 2>".to_string(),
            format!("{RX} filter_fir_en=1"),
            format!("{RX} sampling_frequency=2048000"),
            format!("{RX} rf_bandwidth=1945600"),
        ]
    );
    assert!(srv.board.has("iio:device1 xo_correction=40000400"));
    // The clock corrects the LO too, so the LO is set to the plain frequency.
    assert!(srv
        .board
        .has("iio:device1 OUTPUT altvoltage0 frequency=100000000"));
    assert_eq!(src.capabilities().rates.min_hz, FIR_MIN_RATE);
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, 2.048 * M);
    src.stop();
    // At close: the rate without the FIR, the FIR off, the clock back.
    let log = srv.board.log();
    let writes: Vec<_> = log
        .iter()
        .filter(|l| !l.starts_with("SET") && !l.starts_with("OPEN") && !l.starts_with("CLOSE"))
        .cloned()
        .collect();
    let tail = &writes[writes.len() - 3..];
    assert_eq!(
        tail,
        [
            format!("{RX} sampling_frequency=4096000"),
            format!("{RX} filter_fir_en=0"),
            "iio:device1 xo_correction=40000000".to_string()
        ]
    );
}

#[test]
fn the_low_rate_fir_down_to_half_a_megasample() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(0.6 * M, &[("lowratefir", "1")]))
        .unwrap();
    assert_eq!(
        srv.board.log()[..4],
        [
            format!("{RX} sampling_frequency=2400000"),
            "iio:device1 filter_fir_config=<fir RX 3 GAIN 0 DEC 4>".to_string(),
            format!("{RX} filter_fir_en=1"),
            format!("{RX} sampling_frequency=600000"),
        ]
    );
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, 0.6 * M);
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    src.stop();
    // A board that switches the receive filter on only together with the transmit filter (seen on an ANTSDR E310).
    let srv = FakeIiod::start(FakeOpts {
        fir_needs_both: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(0.6 * M, &[("lowratefir", "1")]))
        .unwrap();
    assert_eq!(
        srv.board.log()[..5],
        [
            format!("{RX} sampling_frequency=2400000"),
            "iio:device1 filter_fir_config=<fir RX 3 GAIN 0 DEC 4>".to_string(),
            format!("{RX} filter_fir_en=1"),
            "iio:device1 INPUT out voltage_filter_fir_en=1".to_string(),
            format!("{RX} sampling_frequency=600000"),
        ]
    );
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, 0.6 * M);
    src.stop();
    assert!(srv
        .board
        .has("iio:device1 INPUT out voltage_filter_fir_en=0"));
    // A board that refuses the filter runs at 2.1 Msps.
    let srv = FakeIiod::start(FakeOpts {
        refuse_fir: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(0.6 * M, &[("lowratefir", "1")]))
        .unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, 2.1 * M);
    assert!(srv.board.has(&format!("{RX} filter_fir_en=0")));
    src.stop();
}

/// A rate below the converters' lowest that the user chose, with the low-rate FIR setting off: the FIR is switched on for it
/// (0.5 and 1 Msps were refused with errno 22 before). A board that takes the receive filter alone but refuses the low rate gets
/// both filters, and the rate.
#[test]
fn a_chosen_low_rate_switches_the_fir_on_by_itself() {
    let srv = FakeIiod::start(FakeOpts {
        low_rate_needs_both: true,
        ..Default::default()
    });
    let mut t = tune(1.0 * M, &[]);
    t.rate_chosen = true;
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .unwrap();
    assert_eq!(
        srv.board.log()[..6],
        [
            format!("{RX} sampling_frequency=4000000"),
            "iio:device1 filter_fir_config=<fir RX 3 GAIN 0 DEC 4>".to_string(),
            format!("{RX} filter_fir_en=1"),
            format!("{RX} sampling_frequency=1000000"),
            "iio:device1 INPUT out voltage_filter_fir_en=1".to_string(),
            format!("{RX} sampling_frequency=1000000"),
        ]
    );
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, 1.0 * M);
    src.stop();
    assert!(srv
        .board
        .has("iio:device1 INPUT out voltage_filter_fir_en=0"));
    // Below 0.521 Msps no setting of the AD936x returns the rate: the nearest it has.
    let srv = FakeIiod::start(FakeOpts::default());
    let mut t = tune(0.5 * M, &[]);
    t.rate_chosen = true;
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    assert_eq!(src.start(w).unwrap().rate_hz, FIR_MIN_RATE.ceil());
    src.stop();
}

/// Automatic gain is the held gain of the driver: manual gain on the AD936x, a start gain written at once (a manual mode with
/// no gain written would keep the gain that the last program left). The AGC of the AD936x is an explicit choice.
#[test]
fn automatic_gain_is_the_held_gain_of_the_driver() {
    assert_eq!(gain_plan(GainMode::Manual, true, 30.0), GainPlan::Held);
    assert_eq!(
        gain_plan(GainMode::Manual, false, 30.0),
        GainPlan::Manual(30.0)
    );
    assert_eq!(
        gain_plan(GainMode::FastAttack, true, 30.0),
        GainPlan::Chip(GainMode::FastAttack)
    );
    assert_eq!(held_start_gain(100e6, (-1.0, 73.0)), 43.0);
    assert_eq!(held_start_gain(2.4e9, (0.0, 71.0)), 41.0);
    assert_eq!(held_start_gain(5.8e9, (0.0, 62.0)), 32.0);
    let srv = FakeIiod::start(FakeOpts::default());
    let mut t = tune(2.4 * M, &[]);
    t.gain_db = None;
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .unwrap();
    assert!(srv.board.has(&format!("{RX} gain_control_mode=manual")));
    assert!(srv.board.has(&format!("{RX} hardwaregain=43")));
    assert_eq!(held(&mut src), Some((43.0, GainControl::Searching, 0)));
    // The AGC of the AD936x, chosen: no held gain, nothing published.
    t.settings.insert("gainmode".into(), "slow_attack".into());
    src.retune(&t).unwrap();
    assert!(srv
        .board
        .has(&format!("{RX} gain_control_mode=slow_attack")));
    assert_eq!(held(&mut src), None);
    assert_eq!(src.readings().fields().get(&source::RX1_GAIN_STEPS), None);
}

/// The held gain of RX1 as the source publishes it: gain, state, steps.
fn held(src: &mut Box<dyn Source>) -> Option<(f64, GainControl, i64)> {
    let r = src.readings();
    let f = r.fields();
    Some((
        f.get(&source::RX1_HELD_GAIN)?,
        f.get(&source::RX1_GAIN_CONTROL)?,
        f.get(&source::RX1_GAIN_STEPS)?,
    ))
}

const HELD_RATE: f64 = 2.4e6;
/// The mean level of the fake at 0 dB gain that puts the peak at the target of the held gain at `gain` dB (the noise of the
/// fake peaks 13 dB above its mean).
fn level_for(gain: f64) -> f64 {
    crate::gain::RULES.target_peak_dbfs - 13.0 - gain
}

/// A board that streams noise at `level` dBFS at 0 dB gain as fast as the link carries it, opened with the gain on automatic
/// and started.
fn held_board(level: f64) -> (FakeIiod, Box<dyn Source>, RingReader) {
    let srv = FakeIiod::start(FakeOpts {
        fast: true,
        ..Default::default()
    });
    *srv.board.level_dbfs.lock().unwrap() = Some(level);
    let mut t = tune(HELD_RATE, &[("readings_s", "0")]);
    t.gain_db = None;
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &t)
        .unwrap();
    let (w, r) = ring(1 << 20, SampleType::Cs16);
    src.start(w).unwrap();
    (srv, src, r)
}

/// The seconds of signal the board has sent.
fn signal_s(srv: &FakeIiod) -> f64 {
    srv.board.rx_samples.load(Ordering::Relaxed) as f64 / HELD_RATE
}

/// Waits (up to 20 s of wall time) until the held gain holds with more than `after` steps, and returns the seconds of
/// signal sent when it did.
fn wait_hold(srv: &FakeIiod, src: &mut Box<dyn Source>, after: i64) -> f64 {
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(20) {
        if let Some((_, GainControl::Holding, steps)) = held(src) {
            if steps > after {
                return signal_s(srv);
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("no hold: {:?}", held(src));
}

/// The gain the board holds now, in dB.
fn board_gain(srv: &FakeIiod) -> f64 {
    srv.board.attrs.lock().unwrap()[&format!("{RX} hardwaregain")]
        .parse()
        .unwrap()
}

#[test]
fn a_held_gain_reaches_its_target_from_30_db_too_high_and_from_30_db_too_low() {
    // The start gain is 43 dB: the signal is 30 dB above the target there (clipped), or 30 dB below it.
    for (err, ideal) in [(30.0, 13.0), (-30.0, 73.0)] {
        let (srv, mut src, _r) = held_board(level_for(43.0 - err));
        let s = wait_hold(&srv, &mut src, 0);
        let (gain, _, steps) = held(&mut src).unwrap();
        assert_eq!(gain, board_gain(&srv));
        assert!((gain - ideal).abs() <= 3.0, "{err} dB off: held {gain} dB");
        assert!(s < 3.0, "{err} dB off: {s:.2} s of signal to hold");
        assert!(steps <= 4, "{err} dB off: {steps} steps");
        let mode = format!("{RX} gain_control_mode=");
        assert!(srv
            .board
            .log()
            .iter()
            .all(|l| !l.starts_with(&mode) || l.ends_with("=manual")));
        src.stop();
    }
}

#[test]
fn a_held_gain_makes_no_step_on_a_steady_signal_and_follows_a_lasting_change() {
    let (srv, mut src, _r) = held_board(level_for(35.0));
    wait_hold(&srv, &mut src, 0);
    let (gain, _, steps) = held(&mut src).unwrap();
    let writes = srv.board.log().len();
    // 10 s of a steady signal with a peak-to-mean ratio of 13 dB: no step.
    let from = signal_s(&srv);
    assert!(wait_for(|| signal_s(&srv) > from + 10.0, 30.0));
    assert_eq!(held(&mut src), Some((gain, GainControl::Holding, steps)));
    assert_eq!(srv.board.log().len(), writes, "{:?}", &srv.board.log()[writes..]);
    // 20 dB stronger: the gain follows, 20 dB lower within 3 dB.
    *srv.board.level_dbfs.lock().unwrap() = Some(level_for(35.0) + 20.0);
    let from = signal_s(&srv);
    let at = wait_hold(&srv, &mut src, steps);
    let (now, _, _) = held(&mut src).unwrap();
    assert!((now - (gain - 20.0)).abs() <= 3.0, "{gain} dB, then {now} dB");
    assert!(at - from < 4.0, "{:.2} s of signal to follow", at - from);
    src.stop();
}

#[test]
fn a_held_gain_does_not_step_during_a_retune_and_starts_again_after_it() {
    let (srv, mut src, _r) = held_board(level_for(35.0));
    wait_hold(&srv, &mut src, 0);
    // Past the 2 s that a held gain waits after a step, so only the retune can keep it from stepping.
    let held_at = signal_s(&srv);
    assert!(wait_for(|| signal_s(&srv) > held_at + 2.5, 10.0));
    let n = srv.board.log().len();
    // Each write of the retune takes 300 ms; 100 ms into it the signal is 25 dB stronger, which the control would answer after
    // 0.5 s of signal.
    srv.board.write_delay_ms.store(300, Ordering::Relaxed);
    let board = srv.board.clone();
    let louder = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        *board.level_dbfs.lock().unwrap() = Some(level_for(35.0) + 25.0);
    });
    let mut t = tune(HELD_RATE, &[("readings_s", "0")]);
    t.gain_db = None;
    t.centre_hz = 101e6;
    let from = signal_s(&srv);
    src.retune(&t).unwrap();
    let during = signal_s(&srv) - from;
    srv.board.write_delay_ms.store(0, Ordering::Relaxed);
    louder.join().unwrap();
    assert!(during > 1.0, "{during:.2} s of signal during the retune");
    let log = srv.board.log();
    assert_eq!(
        log[n..n + 2],
        [
            "iio:device1 OUTPUT altvoltage0 frequency=101000000".to_string(),
            format!("{RX} hardwaregain=43"),
        ],
        "the retune writes first, then the start gain"
    );
    // The control starts again from 43 dB and finds the gain for the stronger signal.
    wait_hold(&srv, &mut src, 0);
    let (gain, _, steps) = held(&mut src).unwrap();
    assert!((gain - 10.0).abs() <= 3.0, "held {gain} dB");
    assert!(steps <= 4, "{steps} steps since the retune");
    src.stop();
}

/// Waits up to `secs` until `n` samples were read from `r`, and returns the gaps it found on the way, at their place in all
/// that was read.
fn read_gaps(r: &mut RingReader, n: usize, secs: f64) -> Vec<crate::Gap> {
    let (mut got, mut gaps) = (0, Vec::new());
    let t = Instant::now();
    while got < n && t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(20));
        let avail = r.available();
        gaps.extend(r.take_gaps(avail).into_iter().map(|g| crate::Gap {
            at: g.at + got,
            ..g
        }));
        got += r.read_with(avail, |_| {});
    }
    gaps
}

#[test]
fn a_withheld_buffer_counts_as_lost_and_leaves_a_gap_in_the_ring() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("readings_s", "0")]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(read_gaps(&mut r, 200_000, 3.0).is_empty());
    srv.board.fail_readbufs.store(2, Ordering::Relaxed);
    let gaps = read_gaps(&mut r, 600_000, 5.0);
    let withheld = srv.board.rx_withheld.load(Ordering::Relaxed);
    assert_eq!(withheld, 2 * 65_536);
    assert_eq!(gaps.iter().map(|g| g.lost).sum::<u64>(), withheld, "{gaps:?}");
    assert_eq!(src.loss().unwrap().radio_lost, withheld);
    src.stop();
}

#[test]
fn a_full_ring_counts_what_it_drops_and_leaves_one_gap() {
    let srv = FakeIiod::start(FakeOpts {
        fast: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("readings_s", "0")]))
        .unwrap();
    let cap = 1 << 18;
    let (w, mut r) = ring(cap, SampleType::Cs16);
    src.start(w).unwrap();
    let stats = r.stats();
    assert!(wait_for(|| stats.dropped() > 1_000_000, 10.0));
    src.stop();
    let dropped = stats.dropped();
    assert_eq!(stats.written(), cap as u64);
    assert_eq!(
        r.take_gaps(cap),
        [crate::Gap::lost_at(cap, dropped)]
    );
}

#[test]
fn an_overflow_of_the_board_is_counted() {
    let srv = FakeIiod::start(FakeOpts {
        overflow_reg: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("readings_s", "0")]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let overflows = |src: &mut Box<dyn Source>| src.readings().fields().get(&source::OVERFLOWS);
    assert!(wait_for(|| overflows(&mut src) == Some(0), 30.0));
    // Past the settling time of the loss detector.
    read_gaps(&mut r, usize::MAX, 4.5);
    // The next answer is a second late: the eight kernel buffers hold 218 ms, so the board drops the rest.
    srv.board.stall_ms.store(1000, Ordering::Relaxed);
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(60) && overflows(&mut src) != Some(1) {
        read_gaps(&mut r, usize::MAX, 0.05);
    }
    assert_eq!(overflows(&mut src), Some(1));
    let dropped = srv.board.rx_overflowed.load(Ordering::Relaxed);
    assert!(dropped > 1_500_000, "{dropped}");
    read_gaps(&mut r, usize::MAX, 2.0);
    assert_eq!(overflows(&mut src), Some(1), "the flag was cleared");
    // The loss detector finds the samples from the timing once it has settled (4 s after the start).
    let lost = src.loss().unwrap().radio_lost as f64;
    assert!((lost / dropped as f64 - 1.0).abs() < 0.01, "{lost} of {dropped}");
    src.stop();
}

#[test]
fn tezuka_firmware_streams_cs8() {
    let srv = FakeIiod::start(FakeOpts {
        fw: "v0.1-tezuka-r2",
        ..Default::default()
    });
    let d = IiodDriver::new(srv.transport());
    let mut src = d
        .open_w(&srv.addr(), &tune(2.4 * M, &[("iqformat", "cs8")]))
        .unwrap();
    assert_eq!(src.sample_type(), SampleType::Cs8);
    let (w, mut r) = ring(1 << 22, SampleType::Cs8);
    src.start(w).unwrap();
    let got = read_n(&mut r, 100, 3.0);
    assert!(
        srv.board.has("OPEN iio:device4 65536 00000001"),
        "{:?}",
        srv.board.log()
    );
    assert_eq!(got[5], Complex32::new(5.0 / 128.0, -5.0 / 128.0));
    src.stop();
}

#[test]
fn a_lower_rate_than_asked_is_reported() {
    let srv = FakeIiod::start(FakeOpts {
        rate_cap: Some(20e6),
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(30.0 * M, &[]))
        .unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    let s = src.start(w).unwrap();
    assert_eq!(s.rate_hz, 20.0 * M);
    let n = src.take_notices();
    assert!(
        n.iter().any(|n| matches!(n, Notice::Warning(t)
            if t.contains("this radio only offers 20 Msps, 30 Msps were requested for this channel width"))),
        "{n:?}"
    );
    src.stop();
}

#[test]
fn a_buffer_shared_with_another_client_takes_its_size() {
    let srv = FakeIiod::start(FakeOpts {
        binary: true,
        ..Default::default()
    });
    {
        let mut a = srv.board.attrs.lock().unwrap();
        a.insert("iio:device4 BUFFER enable".into(), "1".into());
        a.insert("iio:device4 BUFFER length".into(), "262144".into());
    }
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    // The text protocol (the shared buffer), at the other client's size.
    assert!(
        srv.board.has("OPEN iio:device4 262144 00000003"),
        "{:?}",
        srv.board.log()
    );
    assert!(!srv.board.has("BINARY"));
    let notices = src.take_notices();
    assert!(notices
        .iter()
        .any(|n| matches!(n, Notice::Log(t) if t.contains("another client is streaming"))));
    assert!(notices
        .iter()
        .any(|n| matches!(n, Notice::Warning(t) if t.contains("shared with it"))));
    src.stop();
}

#[test]
fn a_dropped_stream_reconnects() {
    let srv = FakeIiod::start(FakeOpts {
        drop_stream_after: Some(3),
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(
        wait_for(|| *srv.board.streams.lock().unwrap() >= 2, 4.0),
        "no reconnect: {:?}",
        srv.board.log()
    );
    r.skip_all();
    assert!(!read_n(&mut r, 1000, 2.0).is_empty());
    let notices = src.take_notices();
    assert!(
        notices
            .iter()
            .any(|n| matches!(n, Notice::Log(t) if t.contains("reconnected after"))),
        "{notices:?}"
    );
    src.stop();
    assert!(
        wait_for(|| srv.open_connections() == 0, 2.0),
        "{}",
        srv.open_connections()
    );
}

#[test]
fn readings_come_from_the_monitor() {
    let srv = FakeIiod::start(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("receivers", "interleaved")]))
        .unwrap();
    assert_eq!(
        src.readings().fields().get(&source::TEMPERATURE),
        None,
        "nothing before the stream starts"
    );
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(wait_for(
        || src.readings().fields().get(&source::TEMPERATURE).is_some(),
        3.0
    ));
    let r = src.readings();
    let f = r.fields();
    assert_eq!(f.get(&source::TEMPERATURE), Some(41.25));
    assert_eq!(f.get(&source::RSSI1), Some(85.25));
    assert_eq!(f.get(&source::RSSI2), Some(85.25));
    assert_eq!(
        f.get(&source::RX2_GAIN),
        Some(30.0),
        "the gain RX2 was given, read back"
    );
    assert_eq!(f.get(&source::CALIBRATION), Some("auto"));
    assert_eq!(f.get(&source::ENSM), Some("fdd"));
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

/// What the sample stream of a paced board delivered in `secs` while every attribute read lasted `delay_ms`.
struct Delivery {
    /// The longest time between two deliveries, in ms.
    max_gap_ms: f64,
    samples: u64,
    lost: u64,
    slow_reads: usize,
}

fn delivery(delay_ms: u64, settings: &[(&str, &str)], secs: f64) -> Delivery {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, settings))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    srv.board.read_delay_ms.store(delay_ms, Ordering::Relaxed);
    r.skip_all();
    let dropped = r.stats().dropped();
    let t = Instant::now();
    let (mut last, mut max_gap, mut n) = (Instant::now(), 0.0f64, 0u64);
    while t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(5));
        let got = r.read_with(usize::MAX, |_| {}) as u64;
        if got > 0 {
            max_gap = max_gap.max(last.elapsed().as_secs_f64() * 1e3);
            last = Instant::now();
            n += got;
        }
    }
    let lost = r.stats().dropped() - dropped + src.loss().map_or(0, |l| l.radio_lost);
    let slow_reads = srv.board.slow_reads.load(Ordering::Relaxed);
    srv.board.read_delay_ms.store(0, Ordering::Relaxed);
    src.stop();
    Delivery {
        max_gap_ms: max_gap,
        samples: n,
        lost,
        slow_reads,
    }
}

#[test]
fn slow_readings_do_not_disturb_the_sample_stream() {
    let quiet = delivery(0, &[("readings_s", "0")], 3.0);
    let busy = delivery(300, &[("readings_s", "0.5")], 3.0);
    eprintln!(
        "monitor off: gap {:.1} ms, {} samples, {} lost; 300 ms reads ({}): gap {:.1} ms, {} samples, {} lost",
        quiet.max_gap_ms, quiet.samples, quiet.lost, busy.slow_reads, busy.max_gap_ms, busy.samples, busy.lost
    );
    assert_eq!(quiet.slow_reads, 0, "a period of 0 reads nothing");
    assert!(busy.slow_reads >= 3, "the monitor read {} times", busy.slow_reads);
    assert_eq!((quiet.lost, busy.lost), (0, 0));
    assert!(busy.max_gap_ms < quiet.max_gap_ms + 30.0, "{:.1} ms", busy.max_gap_ms);
    let ratio = busy.samples as f64 / quiet.samples as f64;
    assert!((0.97..1.03).contains(&ratio), "{ratio}");
}

#[test]
fn the_monitor_keeps_its_period_and_starts_no_read_during_a_retune() {
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[("readings_s", "0.5")]))
        .unwrap();
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(wait_for(
        || src.readings().fields().get(&source::TEMPERATURE).is_some(),
        3.0
    ));
    // Each read takes 20 ms and a pass is due every 0.5 s, and the retune takes a second or more: without the rule
    // a pass of five reads would start during it.
    srv.board.read_delay_ms.store(20, Ordering::Relaxed);
    assert!(wait_for(|| srv.board.slow_reads.load(Ordering::Relaxed) >= 5, 3.0));
    srv.board.write_delay_ms.store(1000, Ordering::Relaxed);
    let before = srv.board.slow_reads.load(Ordering::Relaxed);
    let mut t = tune(2.4 * M, &[("readings_s", "0.5")]);
    t.centre_hz = 101e6;
    let started = Instant::now();
    src.retune(&t).unwrap();
    let during = srv.board.slow_reads.load(Ordering::Relaxed) - before;
    srv.board.write_delay_ms.store(0, Ordering::Relaxed);
    assert!(started.elapsed() >= Duration::from_millis(900), "{:?}", started.elapsed());
    // The monitor adds at most the read that was in flight when the retune started.
    assert!(during <= 1, "{during} reads during the retune");
    assert!(
        wait_for(|| srv.board.slow_reads.load(Ordering::Relaxed) >= before + 5, 3.0),
        "the passes go on after the retune"
    );
    // A slower period: 5 reads per pass, so no more than one pass in the next second.
    srv.board.read_delay_ms.store(1, Ordering::Relaxed);
    let mut slow = t.clone();
    slow.settings.insert("readings_s".into(), "60".into());
    src.retune(&slow).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let at = srv.board.slow_reads.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(1200));
    assert_eq!(srv.board.slow_reads.load(Ordering::Relaxed), at, "the period is 60 s");
    src.stop();
}

#[test]
fn a_board_that_is_off_is_retried_then_reported() {
    let net = sdrkit_core::net::FakeTransport::new();
    let e = IiodDriver::new(Arc::new(net))
        .open_w("ip:127.0.0.1:4242", &tune(2.4 * M, &[]))
        .err()
        .unwrap();
    assert!(e.retryable());
    assert_eq!(e, DriverError::Net(sdrkit_core::NetError::Refused));
}

/// How many Msps the driver carries from a server that answers as fast as it can, into the ring, with a consumer draining it.
fn measure(binary: bool, settings: &[(&str, &str)], two: bool, secs: f64) -> f64 {
    let srv = FakeIiod::start(FakeOpts {
        fast: true,
        binary,
        two_receivers: two,
        version: if binary { "1.0.x" } else { "0.26.v0.26" },
        ..Default::default()
    });
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(61.44 * M, settings))
        .unwrap();
    let (w, mut r) = ring(crate::RING_CAPACITY, SampleType::Cs16);
    src.start(w).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    r.skip_all();
    let dropped = r.stats().dropped();
    let t = Instant::now();
    let mut n = 0u64;
    while t.elapsed().as_secs_f64() < secs {
        r.wait(1, Duration::from_millis(5));
        n += r.read_with(usize::MAX, |_| {}) as u64;
    }
    let msps = n as f64 / t.elapsed().as_secs_f64() / 1e6;
    let lost = r.stats().dropped() - dropped;
    src.stop();
    assert_eq!(lost, 0, "the ring dropped samples");
    msps
}

#[test]
fn carries_samples_from_a_fast_server_without_ring_loss() {
    // Test builds are slow; the release figure is printed by `throughput` below.
    let msps = measure(false, &[], false, 0.5);
    assert!(msps > 4.0, "{msps:.1} Msps");
}

/// cargo test --release -p sdrkit-driver-core throughput -- --ignored --nocapture
#[test]
#[ignore]
fn throughput() {
    for (name, binary, s, two) in [
        ("text, 1 RX", false, vec![], false),
        ("binary, 1 RX", true, vec![], false),
        (
            "text, 2 RX interleaved",
            false,
            vec![("receivers", "interleaved")],
            true,
        ),
        (
            "binary, 2 RX interleaved",
            true,
            vec![("receivers", "interleaved")],
            true,
        ),
        (
            "text, 2 RX as two rings (first ring read)",
            false,
            vec![("receivers", "both")],
            true,
        ),
    ] {
        let msps = measure(binary, &s, two, 3.0);
        println!("{name}: {msps:.1} Msps into the ring");
    }
}

// ---- Against a real board (receive only). IIOD_URI=ip:192.168.2.1 cargo test ... -- --ignored --nocapture --test-threads 1 ----

fn real_uri() -> String {
    std::env::var("IIOD_URI").unwrap_or_else(|_| "ip:192.168.2.1".into())
}

/// Streams for `secs` and returns (samples per ring, wall seconds, ring losses, radio-side loss).
type RealRun = (Vec<u64>, f64, u64, u64, Box<dyn Source>);

fn real_stream(t: &Want, rings: usize, secs: f64) -> RealRun {
    try_real_stream(t, rings, secs).unwrap()
}

fn try_real_stream(t: &Want, rings: usize, secs: f64) -> Result<RealRun, DriverError> {
    let mut src = real_driver().open_w(&real_uri(), t)?;
    let mut readers = Vec::new();
    let mut writers = Vec::new();
    for _ in 0..rings {
        let (w, r) = ring(crate::RING_CAPACITY, SampleType::Cs16);
        writers.push(w);
        readers.push(r);
    }
    src.start_streams(writers)?;
    std::thread::sleep(Duration::from_millis(700));
    for r in &mut readers {
        r.skip_all();
    }
    let lost0 = src.loss().map(|l| l.radio_lost).unwrap_or(0);
    let start = Instant::now();
    let mut got = vec![0u64; rings];
    while start.elapsed().as_secs_f64() < secs {
        for (i, r) in readers.iter_mut().enumerate() {
            got[i] += r.read_with(usize::MAX, |_| {}) as u64;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    let secs = start.elapsed().as_secs_f64();
    let dropped: u64 = readers.iter().map(|r| r.stats().dropped()).sum();
    let radio = src
        .loss()
        .map(|l| l.radio_lost)
        .unwrap_or(0)
        .saturating_sub(lost0);
    Ok((got, secs, dropped, radio, src))
}

#[test]
#[ignore]
fn real_board_rates_and_receivers() {
    let mut t = tune(2.4 * M, &[]);
    t.centre_hz = 100e6;
    for (name, s, rings) in [
        ("RX1", vec![], 1),
        ("RX2", vec![("antenna", "rx2")], 1),
        ("both, two rings", vec![("receivers", "both")], 2),
        ("both, interleaved", vec![("receivers", "interleaved")], 1),
    ] {
        let mut t = t.clone();
        for (k, v) in &s {
            t.settings.insert(k.to_string(), v.to_string());
        }
        let (got, secs, dropped, radio, mut src) = real_stream(&t, rings, 2.0);
        println!(
            "{name}: {:?} samples/s, ring drops {dropped}, radio loss {radio}; {:?}",
            got.iter()
                .map(|g| (*g as f64 / secs) as u64)
                .collect::<Vec<_>>(),
            src.readings()
        );
        src.stop();
    }
}

#[test]
#[ignore]
fn real_board_sustainable_rates() {
    for proto in ["auto"] {
        for both in [false, true] {
            for rate in [2.4, 5.0, 10.0, 15.36, 20.0, 25.0, 30.72, 40.0, 61.44] {
                let mut s = vec![("protocol", proto)];
                if both {
                    s.push(("receivers", "interleaved"));
                }
                let mut t = tune(rate * M, &s);
                t.centre_hz = 100e6;
                let (got, secs, dropped, radio, mut src) = match try_real_stream(&t, 1, 3.0) {
                    Ok(r) => r,
                    Err(e) => {
                        println!(
                            "{proto:>4} {} {rate:>6} Msps: {e}",
                            if both { "2RX" } else { "1RX" }
                        );
                        continue;
                    }
                };
                let per = got[0] as f64 / secs / if both { 2.0 } else { 1.0 } / 1e6;
                let notes = src.take_notices();
                let binary = notes
                    .iter()
                    .any(|n| matches!(n, Notice::Log(t) if t.contains("binary protocol")));
                println!(
                    "{proto:>4} {} {rate:>6} Msps: delivered {per:.3} Msps per RX ({:.1}%), ring drops {dropped}, radio loss {radio}{}",
                    if both { "2RX" } else { "1RX" },
                    100.0 * per / rate,
                    if binary { ", binary" } else { "" }
                );
                src.stop();
            }
        }
    }
}

/// Leaves the bench board in a sane receive state: 2.4 Msps, 100 MHz, slow attack.
#[test]
#[ignore]
fn real_board_leave_sane() {
    let mut t = tune(2.4 * M, &[("gainmode", "slow_attack")]);
    t.centre_hz = 100e6;
    let mut src = real_driver().open_w(&real_uri(), &t).unwrap();
    src.stop();
}

#[test]
#[ignore]
fn real_board_buffer_settings() {
    for (kb, bs) in [
        ("8", "auto"),
        ("32", "auto"),
        ("8", "2097152"),
        ("4", "131072"),
        ("16", "1048576"),
    ] {
        let mut t = tune(20.0 * M, &[("kbuffers", kb), ("bufsize", bs)]);
        t.centre_hz = 100e6;
        let (got, secs, _, _, mut src) = real_stream(&t, 1, 3.0);
        println!(
            "20 Msps, {kb} buffers of {bs}: {:.3} Msps",
            got[0] as f64 / secs / 1e6
        );
        src.stop();
    }
}

fn real_rate(src: &mut dyn Source, r: &mut RingReader, secs: f64) -> f64 {
    std::thread::sleep(Duration::from_millis(500));
    r.skip_all();
    let t = Instant::now();
    let mut n = 0u64;
    while t.elapsed().as_secs_f64() < secs {
        n += r.read_with(usize::MAX, |_| {}) as u64;
        std::thread::sleep(Duration::from_millis(2));
    }
    let _ = src;
    n as f64 / t.elapsed().as_secs_f64() / 1e6
}

#[test]
#[ignore]
fn real_board_fir_ports_restart_and_sharing() {
    let notes = |src: &mut Box<dyn Source>| {
        src.take_notices()
            .into_iter()
            .map(|n| format!("{n:?}"))
            .filter(|n| !n.contains("Fake"))
            .collect::<Vec<_>>()
    };
    for rate in [2.048, 1.0, 0.6] {
        let mut t = tune(rate * M, &[("lowratefir", "1"), ("rxport", "A_BALANCED")]);
        t.centre_hz = 100e6;
        let mut src = real_driver().open_w(&real_uri(), &t).unwrap();
        let (w, mut r) = ring(crate::RING_CAPACITY, SampleType::Cs16);
        let s = src.start(w).unwrap();
        let got = real_rate(&mut *src, &mut r, 2.0);
        println!(
            "FIR {rate} Msps: started at {} Msps, delivered {got:.3}; {:?}",
            s.rate_hz / 1e6,
            notes(&mut src)
        );
        src.stop();
    }
    // A rate change while streaming: the buffer restarts, our own old buffer is not taken for another client's.
    let mut t = tune(2.4 * M, &[]);
    t.centre_hz = 100e6;
    let mut src = real_driver().open_w(&real_uri(), &t).unwrap();
    let (w, mut r) = ring(crate::RING_CAPACITY, SampleType::Cs16);
    src.start(w).unwrap();
    println!("2.4: {:.3}", real_rate(&mut *src, &mut r, 1.0));
    for rate in [5.0, 2.4, 10.0] {
        t.rate_hz = rate * M;
        let res = src.retune(&t);
        println!(
            "retune to {rate}: {res:?}: {:.3} Msps; {:?}",
            real_rate(&mut *src, &mut r, 1.5),
            notes(&mut src)
        );
    }
    t.rate_hz = 2.4 * M;
    src.retune(&t).unwrap();
    // A second client on the same board while the first streams.
    let mut b = real_driver().open_w(&real_uri(), &t).unwrap();
    let (wb, mut rb) = ring(crate::RING_CAPACITY, SampleType::Cs16);
    b.start(wb).unwrap();
    let (ga, gb) = (
        real_rate(&mut *src, &mut r, 2.0),
        real_rate(&mut *b, &mut rb, 2.0),
    );
    println!(
        "sharing: first {ga:.3} Msps, second {gb:.3} Msps; second's notices {:?}",
        notes(&mut b)
    );
    b.stop();
    println!(
        "first after the second left: {:.3}",
        real_rate(&mut *src, &mut r, 1.0)
    );
    src.stop();
}

#[test]
#[ignore]
fn real_board_read_only_diagnostics() {
    let (host, port) = parse_address(&real_uri()).unwrap();
    let at = Address {
        transport: Arc::new(sdrkit_core::net::StdNet),
        host,
        port,
        label: "diag".into(),
    };
    let (mut c, ctx, v) = connect_ctl(&at, &Abort::new()).unwrap();
    println!("server {v:?}");
    let phy = ctx.device(PHY).unwrap().clone();
    for ch in ["voltage0", "voltage1"] {
        let c0 = phy.channel(ch, false).unwrap();
        println!("{ch} attrs: {:?}", c0.attrs);
        for a in [
            "rf_port_select",
            "rf_port_select_available",
            "filter_fir_en",
            "sampling_frequency",
            "gain_control_mode",
        ] {
            println!(
                "  {ch} {a} = {:?}",
                c.read_attr(&phy.id, Target::Channel(false, ch), a)
                    .map_err(|e| e.to_string())
            );
        }
    }
    println!("phy attrs: {:?}", phy.attrs);
    for a in [
        "filter_fir_config",
        "rx_path_rates",
        "ensm_mode",
        "calib_mode",
    ] {
        println!(
            "{a} = {:?}",
            c.read_attr(&phy.id, Target::Device, a)
                .map_err(|e| e.to_string())
        );
    }
    println!("context attrs: {:?}", ctx.attrs);
    c.shutdown();
}

/// A board across a slow network link (`bytes_per_s` from the board to the client), on a fake network.
fn slow_board(bytes_per_s: Option<f64>) -> FakeIiod {
    FakeIiod::start_at(
        sdrkit_core::net::FakeTransport::new(),
        "10.0.0.5",
        FakeOpts {
            link_bytes_per_s: bytes_per_s,
            ..Default::default()
        },
    )
}

/// The positions of the log entries equal to `what`.
fn places(log: &[String], what: &str) -> Vec<usize> {
    log.iter()
        .enumerate()
        .filter(|(_, l)| *l == what)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn a_stop_in_the_middle_of_a_slow_refill_reads_it_out_and_closes_the_buffer() {
    // 2.4 Msps needs 9.6 MB/s; the link carries 2 MB/s, so every answer is still arriving when the stop comes.
    let srv = slow_board(Some(2e6));
    let drv = IiodDriver::new(srv.transport());
    let mut src = drv.open_w(&srv.addr(), &tune(2.4 * M, &[])).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    std::thread::sleep(Duration::from_millis(300));
    src.stop();
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
    assert!(srv.board.has("CLOSE iio:device4"), "{:?}", srv.board.log());
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
    assert_eq!(srv.board.rx_holders.load(Ordering::SeqCst), 0);
    // A second session gets samples.
    let mut src = drv.open_w(&srv.addr(), &tune(2.4 * M, &[])).unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(read_n(&mut r, 50_000, 5.0).len() == 50_000);
    src.stop();
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
}

#[test]
fn a_stall_in_the_middle_of_a_refill_ends_that_connection_without_a_word() {
    let srv = slow_board(None);
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    let closes = places(&srv.board.log(), "CLOSE iio:device4").len();
    let opened = *srv.board.streams.lock().unwrap();
    // Longer than the read timeout of the stream: the answer is cut in its middle.
    srv.board.stall_mid_ms.store(6000, Ordering::Relaxed);
    assert!(
        wait_for(|| *srv.board.streams.lock().unwrap() > opened, 12.0),
        "no new stream: {:?}",
        srv.board.log()
    );
    r.skip_all();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    let log = srv.board.log();
    // Nothing was written on the cut connection: no CLOSE, no command in the middle of its answer.
    assert_eq!(places(&log, "CLOSE iio:device4").len(), closes, "{log:?}");
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 3.0));
}

#[test]
fn a_garbled_answer_ends_the_stream_and_nothing_more_is_written_on_it() {
    let srv = slow_board(None);
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    // After the probe at the start, one request is on its way at a time: none is sent ahead of the garbled answer.
    std::thread::sleep(Duration::from_millis(1500));
    let opened = *srv.board.streams.lock().unwrap();
    srv.board.garble_next.store(true, Ordering::SeqCst);
    assert!(
        wait_for(|| *srv.board.streams.lock().unwrap() > opened, 6.0),
        "no new stream: {:?}",
        srv.board.log()
    );
    r.skip_all();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    assert_eq!(srv.board.after_garble.load(Ordering::SeqCst), 0);
    src.stop();
}

#[test]
fn a_rate_change_on_a_slow_link_closes_the_old_buffer_before_the_new_one_opens() {
    let srv = slow_board(Some(4e6));
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    let opened = *srv.board.streams.lock().unwrap();
    src.retune(&tune(3.2 * M, &[])).unwrap();
    assert!(wait_for(|| *srv.board.streams.lock().unwrap() > opened, 5.0));
    let log = srv.board.log();
    let opens: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("OPEN iio:device4"))
        .map(|(i, _)| i)
        .collect();
    let closes = places(&log, "CLOSE iio:device4");
    // The old stream's buffer was closed after it opened and before the new one opened.
    let (old, new) = (opens[opens.len() - 2], opens[opens.len() - 1]);
    assert!(closes.iter().any(|c| (old..new).contains(c)), "{log:?}");
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
    src.stop();
}

#[test]
fn a_slow_link_is_measured_reported_and_capped() {
    // 2.4 Msps needs 9.6 MB/s; the link carries 4 MB/s.
    let srv = slow_board(Some(4e6));
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    // Timed at open, before any stream: 0.85 of 4 MB/s at 4 bytes a sample.
    let at_open = src.capabilities().rates.steady_hz.expect("timed at open");
    assert!((0.7 * M..0.95 * M).contains(&at_open), "{at_open}");
    assert!(srv.board.has("CLOSE iio:device4"), "{:?}", srv.board.log());
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let mut seen = Vec::new();
    let warned = wait_for(
        || {
            seen.extend(src.take_notices());
            seen.iter()
                .any(|n| matches!(n, Notice::Warning(t) if t.contains("the link delivers 0.")))
        },
        8.0,
    );
    assert!(warned, "{seen:?}");
    // The state stays in the status line, poll after poll.
    std::thread::sleep(Duration::from_millis(300));
    let r = src.readings();
    assert!(r.status().summary.contains("the link delivers 0."), "{}", r.status().summary);
    let cap = src.capabilities().rates.steady_hz.expect("a measured link");
    // 0.85 of 4 MB/s at 4 bytes a sample.
    assert!((0.7 * M..0.95 * M).contains(&cap), "{cap}");
    src.stop();
}

#[test]
fn a_fast_link_is_timed_at_the_start_of_the_stream() {
    let srv = slow_board(Some(40e6));
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let (w, mut r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    assert!(!read_n(&mut r, 1000, 3.0).is_empty());
    let found = wait_for(
        || {
            src.take_notices();
            src.capabilities().rates.steady_hz.is_some()
        },
        3.0,
    );
    assert!(found);
    let cap = src.capabilities().rates.steady_hz.unwrap();
    // About 0.85 of 40 MB/s at 4 bytes a sample: well above the 2.4 Msps the stream runs.
    assert!((5.0 * M..10.0 * M).contains(&cap), "{cap}");
    src.stop();
}

#[test]
fn a_very_slow_link_is_timed_at_open_and_the_timing_ends_with_close() {
    // 0.3 MB/s: two buffers take longer than the second the timing sends requests for.
    let srv = slow_board(Some(0.3e6));
    let src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    let cap = src.capabilities().rates.steady_hz.expect("timed at open");
    assert!(cap < 0.1 * M, "{cap}");
    assert!(srv.board.has("CLOSE iio:device4"), "{:?}", srv.board.log());
    assert!(srv.board.violations().is_empty(), "{:?}", srv.board.violations());
    assert_eq!(srv.board.rx_holders.load(Ordering::SeqCst), 0);
}
