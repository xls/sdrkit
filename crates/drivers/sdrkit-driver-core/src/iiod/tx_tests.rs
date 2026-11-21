//! Transmit tests, against the fake iiod server only: every attribute written, every sample sent, nothing unless armed.

use super::fake::{FakeIiod, FakeOpts};
use super::samples::{mask_of, tx_layout};
use super::tests::{tune, wait_for, with_text, WantDriver, WantSource};
use super::*;
use crate::{ring, Transmitter, TxState};
use sdrkit_core::{Complex32, Results, Value};
use std::collections::BTreeMap;
use std::time::Instant;

const M: f64 = 1e6;
const OUT0: &str = "iio:device1 OUTPUT voltage0";
const OUT1: &str = "iio:device1 OUTPUT voltage1";
const MUTE: &str = "hardwaregain=-89.75";

/// The writes of a transmit attenuator in `log` that are not the largest attenuation.
fn unmutes(log: &[String]) -> Vec<String> {
    log.iter()
        .filter(|l| l.contains(" hardwaregain=") && !l.ends_with(MUTE))
        .cloned()
        .collect()
}

/// The gains of the two transmit attenuators of the fake board, in dB.
fn gains(srv: &FakeIiod) -> (f64, f64) {
    (srv.board.tx_gain(0), srv.board.tx_gain(1))
}

/// The target state of a transmitter in a test. [`TxWant::v`] makes the transmit values of the iiod kind.
#[derive(Clone, Debug)]
pub(super) struct TxWant {
    pub centre_hz: f64,
    pub rate_hz: f64,
    pub bandwidth_hz: f64,
    /// The attenuation: the opposite of the setting `tx_power`.
    pub attenuation_db: f64,
    pub settings: BTreeMap<String, String>,
}

impl Default for TxWant {
    fn default() -> Self {
        TxWant {
            centre_hz: 100e6,
            rate_hz: 0.0,
            bandwidth_hz: 0.0,
            attenuation_db: tx::MAX_ATTENUATION,
            settings: BTreeMap::new(),
        }
    }
}

impl TxWant {
    /// The transmit values of the iiod kind, with the power range and the filter width of the AD936x and any carrier.
    pub fn v(&self) -> Values {
        let schema = crate::tx_schema(
            tx::IiodTxSetting::SETTINGS,
            &crate::TxLimits {
                bandwidth_hz: Some(super::FILTER_WIDTH),
                power_db: (-tx::MAX_ATTENUATION, 0.0, 0.25),
                ..crate::TxLimits::default()
            },
        );
        let mut v = Values::defaults(&schema);
        let mut set = |s: crate::TxSetting, x: Value| v.set(s.id(), &x).unwrap();
        set(crate::TxSetting::Frequency, Value::Hz(self.centre_hz));
        set(crate::TxSetting::Rate, Value::Hz(self.rate_hz));
        set(crate::TxSetting::Bandwidth, Value::Hz(self.bandwidth_hz));
        set(crate::TxSetting::Power, Value::F64(-self.attenuation_db));
        with_text(v, &self.settings)
    }
}

/// Test helpers that call a transmitter with a `TxWant`.
trait WantTx {
    fn configure_w(&mut self, t: &TxWant) -> Result<(), DriverError>;
    fn configure_output_w(&mut self, k: usize, t: &TxWant) -> Result<(), DriverError>;
}

impl WantTx for Box<dyn Transmitter> {
    fn configure_w(&mut self, t: &TxWant) -> Result<(), DriverError> {
        self.configure(&t.v())
    }
    fn configure_output_w(&mut self, k: usize, t: &TxWant) -> Result<(), DriverError> {
        self.configure_output(k, &t.v())
    }
}

fn shared(part: tx::SharedPart) -> Option<tx::TxError> {
    Some(tx::TxError::Shared(part))
}

/// The transmit error that `e` carries, if any.
fn tx_err(e: &DriverError) -> Option<tx::TxError> {
    match e {
        DriverError::Driver(s) => s.downcast_ref::<tx::TxError>().copied(),
        _ => None,
    }
}

fn wrap(e: tx::TxError) -> Option<tx::TxError> {
    Some(e)
}

fn refused(s: tx::IiodTxSetting) -> DriverError {
    DriverError::Refused {
        id: SettingId(crate::TxSetting::SETTINGS.len() as u16 + s.id().0),
    }
}

fn tx_tune(settings: &[(&str, &str)]) -> TxWant {
    let mut t = TxWant {
        centre_hz: 433.92e6,
        rate_hz: 2.4 * M,
        attenuation_db: 10.0,
        ..Default::default()
    };
    for (k, v) in settings {
        t.settings.insert(k.to_string(), v.to_string());
    }
    t
}

fn open(o: FakeOpts) -> (FakeIiod, Box<dyn Source>) {
    let srv = FakeIiod::start(o);
    let src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.4 * M, &[]))
        .unwrap();
    (srv, src)
}

/// What the fake server got by WRITEBUF, as samples of TX1 (or TX2).
fn sent(srv: &FakeIiod, two: bool, tx2: bool) -> Vec<Complex32> {
    let n = if two { 4 } else { 2 };
    let mut ch = String::new();
    for i in 0..n {
        ch += &format!("<channel id=\"voltage{i}\" type=\"output\"><scan-element index=\"{i}\" format=\"le:S16/16&gt;&gt;0\" /></channel>");
    }
    let x = format!("<context name=\"n\"><device id=\"iio:device3\" name=\"cf-ad9361-dds-core-lpc\">{ch}</device></context>");
    let dds = context::Context::parse(&x).unwrap().devices.remove(0);
    let pair = if tx2 { (2, 3) } else { (0, 1) };
    let l = tx_layout(&dds, &mask_of(&[pair.0, pair.1], n), pair).unwrap();
    l.unpack(&srv.board.tx_bytes.lock().unwrap())
}

/// The zeros that go out before the first sample of a burst at `rate`, in samples.
fn lead(rate: f64) -> usize {
    let buf = buffer_samples(rate);
    tx::lead_in_buffers(Duration::from_secs_f64(buf as f64 / rate)) * buf
}

/// What follows the zeros at the start of a burst at `rate` in `got`. Panics unless `got` starts with exactly those.
fn after_lead(got: &[Complex32], rate: f64) -> &[Complex32] {
    let n = lead(rate);
    assert!(got.len() >= n, "{} of {n} samples of zeros", got.len());
    let zeros = got[..n].iter().all(|c| *c == Complex32::default());
    assert!(zeros, "a burst starts with {n} samples of zeros");
    &got[n..]
}

fn ramp(n: usize) -> Vec<Complex32> {
    (0..n)
        .map(|i| {
            let x = (i % 2000) as f32 / 2000.0 - 0.5;
            Complex32::new(x, -x / 2.0)
        })
        .collect()
}

#[test]
fn nothing_is_sent_unless_armed() {
    let (srv, mut src) = open(FakeOpts::default());
    let before = srv.board.tx_log();
    assert!(before.is_empty(), "{before:?}");
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    // Opening mutes the output and writes nothing else.
    let muted = [format!("{OUT0} {MUTE}")];
    assert_eq!(srv.board.tx_log(), muted);
    assert_eq!(tx.state(), TxState::Disarmed);
    assert!(!tx.state().transmitting());
    assert_eq!(tx.capabilities().outputs, ["tx1"]);
    let limits = &tx.capabilities().limits;
    assert_eq!(limits.power_db, (-89.75, 0.0, 0.25));
    assert_eq!(limits.common_rates_hz.first(), Some(&2.1e6));
    // The output estimate comes from the data sheet: 8 dBm at 800 MHz, 7.5 dBm at 2.4 GHz, none below 800 MHz.
    assert_eq!(limits.output_dbm(800e6, -10.0), Some(-2.0));
    assert_eq!(limits.output_dbm(1.6e9, 0.0), Some(7.75));
    assert_eq!(limits.output_dbm(433.92e6, 0.0), None);
    assert_eq!((limits.min_freq_hz, limits.max_freq_hz), (46875001.0, 6e9));
    // The radio declared the same limits when it opened, before the transmitter.
    assert_eq!(src.capabilities().transmit.as_ref(), Some(limits));
    // Configure, push, flush, cyclic: refused or stored, and nothing reaches the radio.
    tx.configure_w(&tx_tune(&[("txport", "B")])).unwrap();
    assert!(tx.push(&ramp(1000), Duration::ZERO).is_err());
    assert!(tx.flush().is_err());
    assert!(tx.play_cyclic(&ramp(100)).is_err());
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(srv.board.tx_log(), muted);
    assert!(srv.board.tx_bytes.lock().unwrap().is_empty());
    // A second transmitter while one is open: refused.
    assert!(src.transmitter_w(&tx_tune(&[])).is_err());
    drop(tx);
    assert_eq!(
        srv.board.tx_log(),
        muted,
        "dropping a disarmed transmitter touches nothing"
    );
    let tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    drop(tx);
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

/// The latency that the transmitter reports is the time that the zeros in front of a burst play, off the air and on.
#[test]
fn the_start_latency_is_the_time_of_the_zeros_in_front_of_a_burst() {
    let (_srv, mut src) = open(FakeOpts::default());
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    let zeros = Duration::from_secs_f64(lead(2.4 * M) as f64 / (2.4 * M));
    let close = |d: Duration| d.abs_diff(zeros) < Duration::from_micros(1);
    assert!(close(tx.start_latency()), "{:?}", tx.start_latency());
    tx.arm().unwrap();
    assert!(close(tx.start_latency()), "{:?}", tx.start_latency());
    tx.disarm();
    src.stop();
}

#[test]
fn arm_writes_in_order_and_every_sample_is_sent() {
    let (srv, mut src) = open(FakeOpts::default());
    let mut tx = src.transmitter_w(&tx_tune(&[("txport", "B")])).unwrap();
    tx.arm().unwrap();
    assert_eq!(tx.state(), TxState::Armed);
    assert!(tx.stats().transmitting_since.is_some());
    let buf = buffer_samples(2.4 * M);
    assert_eq!(buf, 65_536);
    assert_eq!(tx.capabilities().buffer_samples, buf);
    // The first line is the mute of open, the second the mute of arm.
    let expect: Vec<String> = [
        "iio:device1 OUTPUT voltage0 hardwaregain=-89.75",
        "iio:device1 OUTPUT voltage0 hardwaregain=-89.75",
        "iio:device3 OUTPUT altvoltage0 raw=0",
        "iio:device3 OUTPUT altvoltage1 raw=0",
        "iio:device3 OUTPUT altvoltage2 raw=0",
        "iio:device3 OUTPUT altvoltage3 raw=0",
        "iio:device1 OUTPUT voltage0 sampling_frequency=2400000",
        "iio:device1 OUTPUT voltage0 rf_port_select=B",
        "iio:device1 OUTPUT voltage0 rf_bandwidth=2280000",
        "iio:device1 OUTPUT altvoltage1 frequency=433920000",
        "iio:device1 OUTPUT altvoltage1 powerdown=0",
        "SET iio:device3 BUFFERS_COUNT 12",
        "OPEN iio:device3 65536 00000003",
        "iio:device1 OUTPUT voltage0 hardwaregain=-10.00",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(srv.board.tx_log(), expect);
    // Two and a half buffers, then the end of the burst: three buffers go out, the last padded with zeros.
    let iq = ramp(buf * 5 / 2);
    let mut done = 0;
    let t = Instant::now();
    while done < iq.len() && t.elapsed() < Duration::from_secs(5) {
        done += tx.push(&iq[done..], Duration::from_millis(100)).unwrap();
    }
    assert_eq!(done, iq.len());
    tx.flush().unwrap();
    assert!(
        wait_for(|| tx.stats().buffers == 3, 3.0),
        "{:?}",
        tx.stats()
    );
    let s = tx.stats();
    assert_eq!(
        (s.sent, s.padded, s.queued),
        (3 * buf as u64, (buf / 2) as u64, 0)
    );
    // On the wire: the zeros of the start of the burst, then every sample.
    assert_eq!(lead(2.4 * M), 10 * buf);
    let all = sent(&srv, false, false);
    let got = after_lead(&all, 2.4 * M);
    assert_eq!(got.len(), 3 * buf);
    for (i, (a, b)) in got.iter().zip(&iq).enumerate() {
        assert!((a - b).norm() < 1e-4, "sample {i}: {a} vs {b}");
    }
    assert!(got[iq.len()..].iter().all(|c| *c == Complex32::default()));
    // Clipped beyond full scale. A burst after a flush starts with zeros again.
    let n0 = all.len();
    tx.push(&[Complex32::new(3.0, -3.0)], Duration::ZERO)
        .unwrap();
    tx.flush().unwrap();
    assert!(wait_for(|| tx.stats().buffers == 4, 3.0));
    let all = sent(&srv, false, false);
    let second = after_lead(&all[n0..], 2.4 * M);
    assert_eq!(second.len(), buf);
    assert_eq!(second[0], Complex32::new(1.0, -1.0));
    // Disarm: muted first, then the buffer closed, then the LO powered down.
    let n = srv.board.tx_log().len();
    tx.disarm();
    assert_eq!(
        srv.board.tx_log()[n..],
        [
            format!("{OUT0} hardwaregain=-89.75"),
            "CLOSE iio:device3".to_string(),
            "iio:device1 OUTPUT altvoltage1 powerdown=1".to_string(),
        ]
    );
    assert_eq!(tx.state(), TxState::Disarmed);
    assert!(tx.stats().transmitting_since.is_none());
    assert!(tx.push(&iq, Duration::ZERO).is_err());
    let bytes = srv.board.tx_bytes.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(srv.board.tx_bytes.lock().unwrap().len(), bytes);
    tx.disarm();
    tx.close();
    assert_eq!(tx.state(), TxState::Closed);
    assert!(tx.arm().is_err());
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn live_changes_while_transmitting() {
    let (srv, mut src) = open(FakeOpts::default());
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    tx.arm().unwrap();
    let n = srv.board.tx_log().len();
    let mut t = tx_tune(&[]);
    t.centre_hz = 434.0e6;
    t.attenuation_db = 20.1; // to the 0.25 dB step
    tx.configure_w(&t).unwrap();
    // The schema of the transmitter holds the power to the range of the board.
    let power = crate::TxSetting::Power.id();
    assert!(matches!(
        tx.capabilities().settings.check(power, &Value::F64(-200.0)),
        Err(sdrkit_core::SettingError::OutOfRange { .. })
    ));
    t.attenuation_db = 89.75;
    tx.configure_w(&t).unwrap();
    assert_eq!(
        srv.board.tx_log()[n..],
        [
            "iio:device1 OUTPUT altvoltage1 frequency=434000000".to_string(),
            format!("{OUT0} hardwaregain=-20.00"),
            format!("{OUT0} hardwaregain=-89.75"),
        ]
    );
    t.rate_hz = 4.0 * M;
    assert!(tx.configure_w(&t).is_err(), "a new rate needs a disarm");
    t.rate_hz = 2.4 * M;
    t.settings.insert("txoutput".into(), "tx2".into());
    assert!(tx.configure_w(&t).is_err(), "one transmitter");
    tx.configure_w(&TxWant {
        centre_hz: 20e6,
        ..tx_tune(&[])
    })
    .unwrap();
    let mut r = Results::default();
    tx.poll(&mut r);
    assert!(
        r.log_lines().iter().any(|l| l.contains("specified for")),
        "{:?}",
        r.log_lines()
    );
    src.stop();
    assert_eq!(
        tx.state(),
        TxState::Closed,
        "stopping the source closes the transmitter"
    );
    assert!(srv.board.has(&format!("{OUT0} hardwaregain=-89.75")));
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn a_cyclic_tone() {
    let (srv, mut src) = open(FakeOpts::default());
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    tx.arm().unwrap();
    let tone: Vec<Complex32> = (0..1000)
        .map(|i| Complex32::from_polar(0.5, std::f32::consts::TAU * i as f32 / 100.0))
        .collect();
    tx.play_cyclic(&tone).unwrap();
    assert_eq!(tx.state(), TxState::Cyclic);
    assert!(tx.state().transmitting());
    let log = srv.board.tx_log();
    let i = log
        .iter()
        .position(|l| l == "OPEN iio:device3 1000 00000003 CYCLIC")
        .unwrap();
    assert_eq!(
        log[i - 1],
        "CLOSE iio:device3",
        "the streaming buffer closed first"
    );
    assert_eq!(log[i + 1], "WRITEBUF iio:device3 4000");
    let got = sent(&srv, false, false);
    assert_eq!(got.len(), 1000);
    assert!(got.iter().zip(&tone).all(|(a, b)| (a - b).norm() < 1e-4));
    assert!(tx.push(&tone, Duration::ZERO).is_err());
    let n = srv.board.tx_log().len();
    tx.disarm();
    assert_eq!(
        srv.board.tx_log()[n..],
        [
            format!("{OUT0} hardwaregain=-89.75"),
            "CLOSE iio:device3".to_string(),
            "iio:device1 OUTPUT altvoltage1 powerdown=1".to_string(),
        ]
    );
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

/// A pause inside a burst that outlasts what the radio holds at 2.1 Msps: the zeros of the start and one buffer.
const PAUSE_MS: u64 = 600;

/// The samples of time the radio is dry when a burst at 2.1 Msps pauses for [`PAUSE_MS`] after its first buffer: the pause
/// less the zeros of the start and that buffer, which the radio still plays. The upper end leaves 60 ms for the test thread.
fn dry_2m1() -> std::ops::Range<u64> {
    let held = (lead(2.1 * M) + 65_536) as f64;
    let dry = (PAUSE_MS as f64 / 1e3 * 2.1 * M - held) as u64;
    dry - 20_000..dry + 126_000
}

#[test]
fn underruns_are_counted_in_a_burst_not_after_it() {
    let (_srv, mut src) = open(FakeOpts::default());
    let mut t = tx_tune(&[]);
    t.rate_hz = 2.1 * M; // 65536 samples: 31 ms
    let mut tx = src.transmitter_w(&t).unwrap();
    tx.arm().unwrap();
    let buf = 65_536;
    tx.push(&ramp(buf), Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 1, 2.0));
    assert_eq!(
        tx.stats().underruns,
        0,
        "the zeros of the start are no underrun"
    );
    std::thread::sleep(Duration::from_millis(PAUSE_MS));
    tx.push(&ramp(buf), Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 2, 2.0));
    let s = tx.stats();
    assert_eq!(s.underruns, 1, "{s:?}");
    assert!(dry_2m1().contains(&s.underrun_samples), "{s:?}");
    // After a flush, the pause is idle time.
    tx.flush().unwrap();
    std::thread::sleep(Duration::from_millis(150));
    tx.push(&ramp(buf), Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 3, 2.0));
    assert_eq!(tx.stats().underruns, 1);
    src.stop();
}

#[test]
fn tx2_on_a_two_transmitter_board() {
    let (srv, mut src) = open(FakeOpts {
        two_receivers: true,
        ..Default::default()
    });
    let mut tx = src.transmitter_w(&tx_tune(&[("txoutput", "tx2")])).unwrap();
    assert_eq!(tx.capabilities().outputs, ["tx1", "tx2"]);
    tx.arm().unwrap();
    let log = srv.board.tx_log();
    assert!(
        log.contains(&"OPEN iio:device3 65536 0000000c".to_string()),
        "{log:?}"
    );
    assert!(
        log.contains(&"iio:device1 OUTPUT voltage1 hardwaregain=-10.00".to_string()),
        "{log:?}"
    );
    assert!(log.contains(&"iio:device1 OUTPUT voltage1 rf_bandwidth=2280000".to_string()));
    assert!(!log
        .iter()
        .any(|l| l.starts_with("OPEN") && !l.ends_with("0000000c")));
    // 8 DDS tones off on a 2T board.
    assert_eq!(log.iter().filter(|l| l.ends_with(" raw=0")).count(), 8);
    let iq = ramp(65_536);
    tx.push(&iq, Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 1, 2.0));
    let all = sent(&srv, true, true);
    assert!(close_to(after_lead(&all, 2.4 * M), &iq));
    tx.disarm();
    assert!(srv
        .board
        .has("iio:device1 OUTPUT voltage1 hardwaregain=-89.75"));
    src.stop();
}

#[test]
fn transmit_and_receive_share_the_board() {
    // Full duplex: TX takes the receiver's rate and does not write it.
    let (srv, mut src) = open(FakeOpts::default());
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let mut t = tx_tune(&[]);
    t.rate_hz = 0.0;
    let mut tx = src.transmitter_w(&t).unwrap();
    tx.arm().unwrap();
    assert!(!srv
        .board
        .tx_log()
        .iter()
        .any(|l| l.contains("sampling_frequency")));
    // The receiver cannot take the FIR while TX is on air.
    let e = src
        .retune(&tune(2.048 * M, &[("lowratefir", "1")]))
        .err()
        .unwrap();
    assert_eq!(
        e,
        DriverError::NotNow {
            state: crate::Doing::Transmitting
        }
    );
    tx.disarm();
    // A different rate while RX streams: refused.
    t.rate_hz = 4.0 * M;
    tx.configure_w(&t).unwrap();
    let e = tx.arm().err().unwrap();
    assert_eq!(
        tx_err(&e),
        wrap(tx::TxError::RateOfReceiver {
            rx_rate_hz: 2.4 * M
        })
    );
    assert_eq!(tx.state(), TxState::Disarmed);
    drop(tx);
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));

    // TDD firmware: no TX while RX streams.
    let (srv, mut src) = open(FakeOpts {
        ensm: "tdd",
        ..Default::default()
    });
    let (w, _r) = ring(1 << 22, SampleType::Cs16);
    src.start(w).unwrap();
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    let e = tx.arm().err().unwrap();
    assert_eq!(tx_err(&e), wrap(tx::TxError::NeedsFdd));
    assert!(srv.board.tx_bytes.lock().unwrap().is_empty());
    let log = srv.board.tx_log();
    assert!(unmutes(&log).is_empty() && !log.iter().any(|l| l.starts_with("OPEN")));
    drop(tx);
    src.stop();

    // Another client transmitting: refused.
    let (srv, mut src) = open(FakeOpts::default());
    srv.board
        .attrs
        .lock()
        .unwrap()
        .insert("iio:device3 BUFFER enable".into(), "1".into());
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    let e = tx.arm().err().unwrap();
    assert_eq!(tx_err(&e), wrap(tx::TxError::OtherClient));
    assert!(srv.board.tx_log().is_empty());
    drop(tx);
    src.stop();

    // The receiver on the low-rate FIR: refused.
    let srv = FakeIiod::start(FakeOpts::default());
    let mut src = IiodDriver::new(srv.transport())
        .open_w(&srv.addr(), &tune(2.048 * M, &[("lowratefir", "1")]))
        .unwrap();
    let fir_writes = |srv: &FakeIiod| {
        srv.board
            .log()
            .iter()
            .filter(|l| l.ends_with("filter_fir_en=0"))
            .count()
    };
    let before = fir_writes(&srv);
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    assert_eq!(
        tx_err(&tx.arm().unwrap_err()),
        wrap(tx::TxError::ReceiverHoldsClock)
    );
    assert_eq!(fir_writes(&srv), before, "the receiver keeps its FIR");
    drop(tx);
    src.stop();
}

// ---- Two outputs ----

fn two_board() -> (FakeIiod, Box<dyn Source>) {
    open(FakeOpts {
        two_receivers: true,
        ..Default::default()
    })
}

/// What the fake server got by WRITEBUF with both outputs in the buffer: (TX1, TX2).
fn sent_both(srv: &FakeIiod) -> (Vec<Complex32>, Vec<Complex32>) {
    let mut ch = String::new();
    for i in 0..4 {
        ch += &format!("<channel id=\"voltage{i}\" type=\"output\"><scan-element index=\"{i}\" format=\"le:S16/16&gt;&gt;0\" /></channel>");
    }
    let x = format!("<context name=\"n\"><device id=\"iio:device3\" name=\"cf-ad9361-dds-core-lpc\">{ch}</device></context>");
    let dds = context::Context::parse(&x).unwrap().devices.remove(0);
    let mask = mask_of(&[0, 1, 2, 3], 4);
    let raw = srv.board.tx_bytes.lock().unwrap().clone();
    let a = tx_layout(&dds, &mask, (0, 1)).unwrap().unpack(&raw);
    let b = tx_layout(&dds, &mask, (2, 3)).unwrap().unpack(&raw);
    (a, b)
}

fn close_to(got: &[Complex32], want: &[Complex32]) -> bool {
    got.len() >= want.len() && got.iter().zip(want).all(|(a, b)| (a - b).norm() < 1e-4)
}

#[test]
fn both_outputs_at_once_each_armed_and_disarmed_alone() {
    let (srv, mut src) = two_board();
    let mut tx = src
        .transmitter_w(&tx_tune(&[("transmitters", "both")]))
        .unwrap();
    assert_eq!(tx.layout(), crate::StreamLayout::Separate(2));
    assert_eq!(tx.capabilities().limits.rates.max_hz, MAX_RATE_2R2T);
    assert_eq!(
        (tx.state_output(0), tx.state_output(1), tx.state_output(2)),
        (TxState::Disarmed, TxState::Disarmed, TxState::Closed)
    );
    let muted = [format!("{OUT0} {MUTE}"), format!("{OUT1} {MUTE}")];
    assert_eq!(srv.board.tx_log(), muted, "opening only mutes");
    let mut t2 = tx_tune(&[("transmitters", "both")]);
    t2.attenuation_db = 20.0;
    tx.configure_output_w(1, &t2).unwrap();
    assert_eq!(
        srv.board.tx_log(),
        muted,
        "configuring a disarmed output writes nothing"
    );
    // TX1 on air: TX2 muted before the buffer opens with all four channels, TX1 unmuted last.
    tx.arm_output(0).unwrap();
    let log = srv.board.tx_log();
    let at = |s: &str| {
        log.iter()
            .position(|l| l == s)
            .unwrap_or_else(|| panic!("{s}: {log:?}"))
    };
    assert!(
        at("iio:device1 OUTPUT voltage1 hardwaregain=-89.75")
            < at("OPEN iio:device3 65536 0000000f")
    );
    assert_eq!(log.last().unwrap(), &format!("{OUT0} hardwaregain=-10.00"));
    assert_eq!(tx.state_output(1), TxState::Disarmed);
    // TX2 joins: only its attenuation is written.
    let n = srv.board.tx_log().len();
    tx.arm_output(1).unwrap();
    assert_eq!(
        srv.board.tx_log()[n..],
        ["iio:device1 OUTPUT voltage1 hardwaregain=-20.00".to_string()]
    );
    let buf = 65_536;
    let a = ramp(buf);
    let b: Vec<Complex32> = a
        .iter()
        .map(|c| c * Complex32::new(0.0, 1.0) * 0.5)
        .collect();
    assert_eq!(tx.push_output(1, &b, Duration::from_secs(1)).unwrap(), buf);
    assert_eq!(tx.push_output(0, &a, Duration::from_secs(1)).unwrap(), buf);
    assert!(wait_for(|| tx.stats_output(0).buffers == 1, 3.0));
    // The count of the driver rises before the server has kept the bytes: four channels of two bytes a sample.
    let all = 8 * (lead(2.4 * M) + buf);
    assert!(wait_for(
        || srv.board.tx_bytes.lock().unwrap().len() >= all,
        3.0
    ));
    let (ga, gb) = sent_both(&srv);
    assert!(close_to(after_lead(&ga, 2.4 * M), &a) && close_to(after_lead(&gb, 2.4 * M), &b));
    assert_eq!(
        (tx.stats_output(0).sent, tx.stats_output(1).sent),
        (buf as u64, buf as u64)
    );
    // TX1 off air alone: muted, the buffer stays for TX2, the LO stays up.
    let n = srv.board.tx_log().len();
    tx.disarm_output(0);
    assert_eq!(
        srv.board.tx_log()[n..],
        [format!("{OUT0} hardwaregain=-89.75")]
    );
    assert_eq!(
        (tx.state_output(0), tx.state_output(1)),
        (TxState::Disarmed, TxState::Armed)
    );
    assert!(tx.push_output(0, &a, Duration::ZERO).is_err());
    // TX2 goes on alone: TX1's channels carry zeros.
    let before = sent_both(&srv).0.len();
    tx.push_output(1, &b, Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats_output(1).buffers == 2, 3.0));
    let (ga, gb) = sent_both(&srv);
    assert!(ga[before..].iter().all(|c| *c == Complex32::default()));
    assert!(close_to(&gb[before..], &b));
    // The last output off air: every attenuator of the board muted, the buffer closed, the LO powered down.
    let n = srv.board.tx_log().len();
    tx.disarm_output(1);
    assert_eq!(
        srv.board.tx_log()[n..],
        [
            format!("{OUT0} {MUTE}"),
            "iio:device1 OUTPUT voltage1 hardwaregain=-89.75".to_string(),
            "CLOSE iio:device3".to_string(),
            "iio:device1 OUTPUT altvoltage1 powerdown=1".to_string(),
        ]
    );
    src.stop();
    assert_eq!(tx.state_output(1), TxState::Closed);
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn the_outputs_share_one_lo_one_clock_and_one_filter() {
    let (_srv, mut src) = two_board();
    let mut tx = src
        .transmitter_w(&tx_tune(&[("transmitters", "both")]))
        .unwrap();
    tx.arm_output(0).unwrap();
    // A different carrier on TX2 while TX1 is on air: refused, at arm and live.
    let mut t2 = tx_tune(&[("transmitters", "both")]);
    t2.centre_hz = 434.5e6;
    tx.configure_output_w(1, &t2).unwrap();
    let e = tx.arm_output(1).unwrap_err();
    assert_eq!(tx_err(&e), shared(tx::SharedPart::Lo));
    assert_eq!(tx.state_output(1), TxState::Disarmed);
    t2.centre_hz = 433.92e6;
    tx.configure_output_w(1, &t2).unwrap();
    tx.arm_output(1).unwrap();
    t2.centre_hz = 434.5e6;
    assert_eq!(
        tx_err(&tx.configure_output_w(1, &t2).unwrap_err()),
        shared(tx::SharedPart::Lo)
    );
    // Another rate or filter: refused.
    let mut t1 = tx_tune(&[("transmitters", "both")]);
    t1.bandwidth_hz = 1e6;
    assert_eq!(
        tx_err(&tx.configure_output_w(0, &t1).unwrap_err()),
        shared(tx::SharedPart::Filter)
    );
    tx.disarm_output(1);
    t2 = tx_tune(&[("transmitters", "both")]);
    t2.rate_hz = 4.0 * M;
    tx.configure_output_w(1, &t2).unwrap();
    assert_eq!(
        tx_err(&tx.arm_output(1).unwrap_err()),
        shared(tx::SharedPart::Clock)
    );
    // With TX1 alone on air, its carrier moves freely.
    t1 = tx_tune(&[("transmitters", "both")]);
    t1.centre_hz = 435e6;
    tx.configure_output_w(0, &t1).unwrap();
    src.stop();
}

#[test]
fn two_outputs_run_at_most_30_72_msps() {
    let (srv, mut src) = two_board();
    let mut t = tx_tune(&[("transmitters", "both")]);
    t.rate_hz = 40.0 * M;
    let e = src.transmitter_w(&t).err().unwrap();
    assert_eq!(
        tx_err(&e),
        wrap(tx::TxError::TwoOutputs { max_hz: 30.72 * M })
    );
    t.rate_hz = 30.72 * M;
    let mut tx = src.transmitter_w(&t).unwrap();
    t.rate_hz = 61.44 * M;
    assert_eq!(
        tx_err(&tx.configure_output_w(1, &t).unwrap_err()),
        wrap(tx::TxError::TwoOutputs { max_hz: 30.72 * M })
    );
    // One output on the same board takes any rate the radio takes.
    drop(tx);
    let mut one = tx_tune(&[]);
    one.rate_hz = 61.44 * M;
    let tx = src.transmitter_w(&one).unwrap();
    assert_eq!(tx.capabilities().limits.rates.max_hz, MAX_RATE);
    assert!(unmutes(&srv.board.tx_log()).is_empty());
    assert!(!srv.board.tx_log().iter().any(|l| l.starts_with("OPEN")));
    src.stop();
}

#[test]
fn a_pluto_with_one_transmitter_refuses_a_second_output() {
    let (srv, mut src) = open(FakeOpts::default());
    for setting in ["both", "interleaved"] {
        let e = src
            .transmitter_w(&tx_tune(&[("transmitters", setting)]))
            .err()
            .unwrap();
        assert_eq!(e, refused(tx::IiodTxSetting::Transmitters));
    }
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    assert_eq!(tx.layout(), crate::StreamLayout::Single);
    assert_eq!(
        tx.arm_output(1).unwrap_err(),
        DriverError::Streams { given: 2, has: 1 }
    );
    assert!(tx.push_output(1, &ramp(10), Duration::ZERO).is_err());
    assert_eq!(tx.state_output(1), TxState::Closed);
    // The layout is fixed from open.
    assert_eq!(
        tx.configure_w(&tx_tune(&[("transmitters", "both")]))
            .unwrap_err(),
        refused(tx::IiodTxSetting::Transmitters)
    );
    assert_eq!(srv.board.tx_log(), [format!("{OUT0} {MUTE}")]);
    src.stop();
}

#[test]
fn underruns_are_counted_per_output() {
    let (_srv, mut src) = two_board();
    let mut t = tx_tune(&[("transmitters", "both")]);
    t.rate_hz = 2.1 * M; // 65536 samples: 31 ms
    let mut tx = src.transmitter_w(&t).unwrap();
    tx.configure_output_w(1, &t).unwrap();
    tx.arm_output(0).unwrap();
    tx.arm_output(1).unwrap();
    let buf = 65_536;
    // TX1 in a burst with a gap; TX2 idle: only TX1 counts it.
    tx.push_output(0, &ramp(buf), Duration::from_secs(1))
        .unwrap();
    assert!(wait_for(|| tx.stats_output(0).buffers == 1, 2.0));
    std::thread::sleep(Duration::from_millis(PAUSE_MS));
    tx.push_output(0, &ramp(buf), Duration::from_secs(1))
        .unwrap();
    assert!(wait_for(|| tx.stats_output(0).buffers == 2, 2.0));
    let (s1, s2) = (tx.stats_output(0), tx.stats_output(1));
    assert_eq!((s1.underruns, s2.underruns), (1, 0), "{s1:?} {s2:?}");
    assert!(dry_2m1().contains(&s1.underrun_samples), "{s1:?}");
    assert_eq!(s2.sent, 0);
    // TX2 joins the burst with half a buffer and stalls: the transfer waits, then sends its half and counts the rest for TX2.
    tx.push_output(1, &ramp(buf / 2), Duration::from_secs(1))
        .unwrap();
    tx.push_output(0, &ramp(buf), Duration::from_secs(1))
        .unwrap();
    assert!(wait_for(|| tx.stats_output(0).buffers == 3, 2.0));
    let s2 = tx.stats_output(1);
    assert_eq!(
        (s2.underruns, s2.underrun_samples),
        (1, (buf / 2) as u64),
        "{s2:?}"
    );
    assert_eq!(tx.stats_output(0).underruns, s1.underruns);
    // After a flush, the pause is idle time for both.
    tx.flush_output(0).unwrap();
    tx.flush_output(1).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    tx.push_output(0, &ramp(buf), Duration::from_secs(1))
        .unwrap();
    assert!(wait_for(|| tx.stats_output(0).buffers == 4, 2.0));
    assert_eq!(
        (tx.stats_output(0).underruns, tx.stats_output(1).underruns),
        (1, 1)
    );
    src.stop();
}

#[test]
fn interleaved_outputs_take_one_stream_on_tx1() {
    let (srv, mut src) = two_board();
    let mut tx = src
        .transmitter_w(&tx_tune(&[("transmitters", "interleaved")]))
        .unwrap();
    assert_eq!(tx.layout(), crate::StreamLayout::Interleaved(2));
    tx.arm_output(0).unwrap();
    tx.arm_output(1).unwrap();
    let buf = 65_536;
    let a = ramp(buf);
    let b: Vec<Complex32> = a.iter().map(|c| -c).collect();
    let mixed: Vec<Complex32> = a.iter().zip(&b).flat_map(|(x, y)| [*x, *y]).collect();
    assert_eq!(
        tx_err(&tx.push_output(1, &b, Duration::ZERO).unwrap_err()),
        wrap(tx::TxError::Interleaved)
    );
    assert_eq!(
        tx.push_output(0, &mixed, Duration::from_secs(1)).unwrap(),
        2 * buf
    );
    assert!(wait_for(|| tx.stats_output(0).buffers == 1, 3.0));
    let (ga, gb) = sent_both(&srv);
    assert!(close_to(after_lead(&ga, 2.4 * M), &a) && close_to(after_lead(&gb, 2.4 * M), &b));
    // TX2 disarmed: its half of the stream is dropped, its channels carry zeros.
    tx.disarm_output(1);
    let before = sent_both(&srv).0.len();
    tx.push_output(0, &mixed, Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats_output(0).buffers == 2, 3.0));
    let (ga, gb) = sent_both(&srv);
    assert!(close_to(&ga[before..], &a));
    assert!(gb[before..].iter().all(|c| *c == Complex32::default()));
    src.stop();
}

#[test]
fn a_cyclic_buffer_repeats_every_output_of_the_board() {
    let (srv, mut src) = two_board();
    let mut tx = src
        .transmitter_w(&tx_tune(&[("transmitters", "both")]))
        .unwrap();
    tx.arm_output(0).unwrap();
    tx.arm_output(1).unwrap();
    let tone = ramp(1000);
    // TX2 streams: a cyclic buffer on TX1 is refused.
    assert_eq!(
        tx.play_cyclic_output(0, &tone).unwrap_err(),
        DriverError::NotNow {
            state: crate::Doing::Transmitting
        }
    );
    tx.disarm_output(1);
    tx.play_cyclic_output(0, &tone).unwrap();
    tx.arm_output(1).unwrap();
    assert_eq!(
        tx.push_output(1, &tone, Duration::ZERO).unwrap_err(),
        DriverError::NotNow {
            state: crate::Doing::Cyclic
        }
    );
    assert_eq!(
        tx_err(&tx.play_cyclic_output(1, &ramp(500)).unwrap_err()),
        wrap(tx::TxError::CyclicLength)
    );
    srv.board.tx_bytes.lock().unwrap().clear();
    let back: Vec<Complex32> = tone.iter().map(|c| c * 0.5).collect();
    tx.play_cyclic_output(1, &back).unwrap();
    assert_eq!(
        (tx.state_output(0), tx.state_output(1)),
        (TxState::Cyclic, TxState::Cyclic)
    );
    assert!(srv.board.has("OPEN iio:device3 1000 0000000f CYCLIC"));
    let (ga, gb) = sent_both(&srv);
    assert!(close_to(&ga, &tone) && close_to(&gb, &back));
    // TX1 off: the buffer is made again with zeros for it.
    srv.board.tx_bytes.lock().unwrap().clear();
    tx.disarm_output(0);
    let (ga, gb) = sent_both(&srv);
    assert!(ga.iter().all(|c| *c == Complex32::default()) && close_to(&gb, &back));
    tx.disarm_output(1);
    assert!(srv.board.has("iio:device1 OUTPUT altvoltage1 powerdown=1"));
    src.stop();
    assert!(wait_for(|| srv.open_connections() == 0, 2.0));
}

#[test]
fn a_fir_left_on_is_switched_off_before_transmit() {
    // An earlier program left the FIR on: the transmitter switches it off and reads it back.
    let (srv, mut src) = open(FakeOpts {
        tx_fir_left_on: Some("1"),
        ..Default::default()
    });
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    for s in [
        "iio:device1 OUTPUT voltage0 filter_fir_en=0",
        "iio:device1 INPUT out voltage_filter_fir_en=0",
    ] {
        assert!(srv.board.has(s), "{s}: {:?}", srv.board.log());
    }
    assert!(srv.board.tx_bytes.lock().unwrap().is_empty());
    assert!(unmutes(&srv.board.tx_log()).is_empty());
    tx.arm().unwrap();
    tx.disarm();
    drop(tx);
    src.stop();

    // The FIR does not switch off: the transmitter is refused.
    let (srv, mut src) = open(FakeOpts {
        tx_fir_left_on: Some("1"),
        tx_fir_stuck: true,
        ..Default::default()
    });
    let e = src.transmitter_w(&tx_tune(&[])).err().unwrap();
    assert_eq!(tx_err(&e), wrap(tx::TxError::FirStaysOn));
    assert!(!srv
        .board
        .log()
        .iter()
        .any(|l| l.contains("OUTPUT voltage0 hardwaregain") || l.starts_with("OPEN iio:device3")));
    // A rate that needs the FIR: refused at arm.
    let (_srv, mut src) = open(FakeOpts::default());
    let mut t = tx_tune(&[]);
    t.rate_hz = 1.0 * M;
    let mut tx = src.transmitter_w(&t).unwrap();
    assert_eq!(
        tx_err(&tx.arm().unwrap_err()),
        wrap(tx::TxError::BelowConverter {
            min_hz: 25e6 / 12.0
        })
    );
    drop(tx);
    src.stop();
}

#[test]
fn an_open_radio_lists_its_transmit_settings_and_opens_its_transmitter_with_them() {
    use sdrkit_core::Owner;
    let srv = FakeIiod::start(FakeOpts::default());
    let kind = crate::testing::kind(IiodDriver::new(srv.transport()));
    let v = crate::testing::values(&IiodDriver::new(srv.transport()), &[]);
    let mut radio = kind.open(&srv.addr(), &v).unwrap();
    let id = radio.schema().id(crate::TX_POWER.key()).unwrap();
    assert!(radio.schema().id(tx::TRANSMITTERS.key()).is_ok());
    // A low but usable power until the host sets one, with the usual values inside the range of the board.
    let decl = radio.schema().setting(id).unwrap();
    assert_eq!(decl.default, Value::F64(-10.0));
    assert_eq!(decl.range.map(|r| (r.min, r.max)), Some((-89.75, 0.0)));
    assert_eq!(
        radio.schema().common(id, radio.values()),
        &[-30.0, -20.0, -10.0, -6.0, -3.0, 0.0]
    );
    // The estimate of the output follows the carrier and the power: absent below 800 MHz, from the table above.
    let estimate = |radio: &crate::OpenRadio| {
        radio
            .results()
            .fields()
            .get(&crate::TX_OUTPUT_DBM)
    };
    assert_eq!(estimate(&radio), None);
    let carrier = radio.schema().id(crate::TX_FREQUENCY.key()).unwrap();
    radio.apply(carrier, &Value::Hz(2.4e9)).unwrap();
    assert_eq!(estimate(&radio), Some(7.5 - 10.0));
    let seq = radio.tx_seq();
    radio.apply(id, &Value::F64(-30.0)).unwrap();
    assert_eq!(radio.tx_seq(), seq + 1);
    assert_eq!(estimate(&radio), Some(7.5 - 30.0));
    let tx_values = radio.tx_values().unwrap();
    assert_eq!(tx_values.get(&crate::TX_POWER), -30.0);
    // The rates and filter widths that people use, inside the limits of the board.
    let rate = radio.schema().id(crate::RATE.key()).unwrap();
    let rates = radio.schema().common(rate, None);
    assert!(rates.contains(&2.1e6) && rates.contains(&20e6), "{rates:?}");
    let width = radio.schema().id(crate::BANDWIDTH.key()).unwrap();
    assert_eq!(radio.schema().common(width, None).first(), Some(&0.2e6));
    let mut t = radio.transmitter().unwrap();
    assert_eq!(t.state(), TxState::Disarmed);
    t.close();
}

// ---- One stream, and the attenuators of the whole board ----

/// A board with two outputs whose attenuators the firmware left at 10 dB. `crossed`: the scan channels of each output
/// are played through the attenuator of the other.
fn two_board_paired(crossed: bool) -> (FakeIiod, Box<dyn Source>) {
    let (srv, src) = open(FakeOpts {
        two_receivers: true,
        tx_crossed: crossed,
        ..Default::default()
    });
    assert_eq!(gains(&srv), (-10.0, -10.0), "as the firmware starts");
    (srv, src)
}

/// Sends one buffer of `iq` and waits until the board has played `transfers` transfers with a signal in all.
fn play(tx: &mut Box<dyn Transmitter>, srv: &FakeIiod, iq: &[Complex32], transfers: usize) {
    assert_eq!(tx.push(iq, Duration::from_secs(1)).unwrap(), iq.len());
    assert!(
        wait_for(
            || srv.board.tx_played.lock().unwrap().len() >= transfers,
            3.0
        ),
        "{:?}",
        srv.board.tx_played.lock().unwrap()
    );
}

#[test]
fn one_stream_is_in_one_pair_and_its_attenuation_in_every_attenuator() {
    for (key, chan) in [("", 0), ("tx1", 0), ("tx2", 1)] {
        for crossed in [false, true] {
            let what = format!("output {key:?}, crossed {crossed}");
            let (srv, mut src) = two_board_paired(crossed);
            let mut t = if key.is_empty() {
                tx_tune(&[])
            } else {
                tx_tune(&[("txoutput", key)])
            };
            t.attenuation_db = 30.0;
            let mut tx = src.transmitter_w(&t).unwrap();
            assert_eq!(tx.layout(), crate::StreamLayout::Single);
            assert_eq!(tx.capabilities().limits.rates.max_hz, MAX_RATE);
            // Open: every attenuator of the board muted, nothing else written.
            let muted = [format!("{OUT0} {MUTE}"), format!("{OUT1} {MUTE}")];
            assert_eq!(srv.board.tx_log(), muted, "{what}");
            assert_eq!(gains(&srv), (-89.75, -89.75), "after open");
            let n = srv.board.tx_log().len();
            tx.arm().unwrap();
            let own = format!("iio:device1 OUTPUT voltage{chan}");
            let mask = if chan == 0 { "00000003" } else { "0000000c" };
            let mut expect = muted.to_vec();
            expect.extend((0..8).map(|i| format!("iio:device3 OUTPUT altvoltage{i} raw=0")));
            expect.extend([
                "iio:device1 OUTPUT voltage0 sampling_frequency=2400000".to_string(),
                format!("{own} rf_bandwidth=2280000"),
                "iio:device1 OUTPUT altvoltage1 frequency=433920000".to_string(),
                "iio:device1 OUTPUT altvoltage1 powerdown=0".to_string(),
                "SET iio:device3 BUFFERS_COUNT 12".to_string(),
                format!("OPEN iio:device3 65536 {mask}"),
                format!("{OUT0} hardwaregain=-30.00"),
                format!("{OUT1} hardwaregain=-30.00"),
            ]);
            assert_eq!(srv.board.tx_log()[n..], expect, "{what}");
            assert_eq!(gains(&srv), (-30.0, -30.0));
            // The samples are in one pair, four bytes a sample, and the attenuator they pass has the set attenuation.
            let iq = ramp(65_536);
            play(&mut tx, &srv, &iq, 1);
            assert!(srv.board.has("WRITEBUF iio:device3 262144"), "{what}");
            assert!(close_to(
                after_lead(&sent(&srv, true, chan == 1), 2.4 * M),
                &iq
            ));
            let played = srv.board.tx_played.lock().unwrap().clone();
            let through = if crossed { 1 - chan } else { chan };
            assert!(
                played
                    .iter()
                    .all(|p| (p.pair, p.attenuator, p.gain_db) == (chan, through, -30.0)),
                "{what}: {played:?}"
            );
            // A live change reaches every attenuator, and the played level follows it.
            let n = srv.board.tx_log().len();
            t.attenuation_db = 55.0;
            tx.configure_w(&t).unwrap();
            assert_eq!(
                srv.board.tx_log()[n..],
                [
                    format!("{OUT0} hardwaregain=-55.00"),
                    format!("{OUT1} hardwaregain=-55.00")
                ]
            );
            play(&mut tx, &srv, &iq, 2);
            let played = srv.board.tx_played.lock().unwrap().clone();
            assert!(played[1..].iter().all(|p| p.gain_db == -55.0), "{played:?}");
            // The end of the burst: every attenuator muted first, then the buffer closed, then the LO powered down.
            let n = srv.board.tx_log().len();
            tx.disarm();
            assert_eq!(
                srv.board.tx_log()[n..],
                [
                    format!("{OUT0} {MUTE}"),
                    format!("{OUT1} {MUTE}"),
                    "CLOSE iio:device3".to_string(),
                    "iio:device1 OUTPUT altvoltage1 powerdown=1".to_string(),
                ]
            );
            assert_eq!(gains(&srv), (-89.75, -89.75), "after the burst");
            // A second burst, then close while on air.
            tx.arm().unwrap();
            assert_eq!(gains(&srv), (-55.0, -55.0));
            tx.close();
            assert_eq!(tx.state(), TxState::Closed);
            assert_eq!(gains(&srv), (-89.75, -89.75), "after close");
            src.stop();
            assert!(wait_for(|| srv.open_connections() == 0, 2.0));
        }
    }
}

#[test]
fn a_cyclic_buffer_of_one_stream_is_in_one_pair() {
    let (srv, mut src) = two_board_paired(true);
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    tx.arm().unwrap();
    let tone = ramp(1000);
    tx.play_cyclic(&tone).unwrap();
    assert!(srv.board.has("OPEN iio:device3 1000 00000003 CYCLIC"));
    assert!(close_to(&sent(&srv, true, false), &tone));
    assert_eq!(gains(&srv), (-10.0, -10.0));
    tx.disarm();
    assert_eq!(gains(&srv), (-89.75, -89.75));
    src.stop();
}

#[test]
fn a_board_with_one_output_takes_tx1_only() {
    let (srv, mut src) = open(FakeOpts::default());
    let mut t = tx_tune(&[]);
    t.attenuation_db = 30.0;
    let mut tx = src.transmitter_w(&t).unwrap();
    let n = srv.board.tx_log().len();
    tx.arm().unwrap();
    let log = srv.board.tx_log()[n..].to_vec();
    assert_eq!(unmutes(&log), [format!("{OUT0} hardwaregain=-30.00")]);
    assert!(!log.iter().any(|l| l.contains("voltage1 hardwaregain")));
    play(&mut tx, &srv, &ramp(65_536), 1);
    let played = srv.board.tx_played.lock().unwrap().clone();
    assert!(
        played
            .iter()
            .all(|p| (p.pair, p.attenuator, p.gain_db) == (0, 0, -30.0)),
        "{played:?}"
    );
    tx.disarm();
    assert_eq!(srv.board.tx_gain(0), -89.75);
    tx.close();
    t.settings.insert("txoutput".into(), "tx2".into());
    assert_eq!(
        src.transmitter_w(&t).err().unwrap(),
        refused(tx::IiodTxSetting::Output)
    );
    src.stop();
}

#[test]
fn one_stream_needs_four_bytes_a_sample_on_a_board_with_two_outputs() {
    // The board takes the bytes of one pair at 2.1 Msps and no more.
    let slow = || {
        open(FakeOpts {
            two_receivers: true,
            tx_max_bytes_per_s: Some(2.1 * M * 4.0),
            ..Default::default()
        })
    };
    let gap = |srv: &FakeIiod| srv.board.tx_gap.load(std::sync::atomic::Ordering::Relaxed);
    let (srv, mut src) = slow();
    let mut t = tx_tune(&[]);
    t.rate_hz = 2.1 * M;
    let mut tx = src.transmitter_w(&t).unwrap();
    tx.arm().unwrap();
    for n in 1..=3 {
        play(&mut tx, &srv, &ramp(65_536), n);
    }
    let bufs: Vec<String> = srv
        .board
        .tx_log()
        .into_iter()
        .filter(|l| l.starts_with("WRITEBUF"))
        .collect();
    assert!(bufs.iter().all(|l| l == "WRITEBUF iio:device3 262144"));
    assert_eq!(gap(&srv), 0, "the board never ran dry");
    src.stop();
    // Two outputs in the buffer need twice the bytes: this board runs dry.
    let (srv, mut src) = slow();
    t.settings.insert("transmitters".into(), "both".into());
    let mut tx = src.transmitter_w(&t).unwrap();
    tx.arm().unwrap();
    play(&mut tx, &srv, &ramp(65_536), 1);
    assert!(wait_for(|| gap(&srv) > 0, 2.0));
    src.stop();
}

#[test]
fn opening_beside_another_client_on_air_leaves_its_outputs() {
    let (srv, mut src) = two_board_paired(false);
    srv.board
        .attrs
        .lock()
        .unwrap()
        .insert("iio:device3 BUFFER enable".into(), "1".into());
    let tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    assert!(srv.board.tx_log().is_empty());
    assert_eq!(gains(&srv), (-10.0, -10.0));
    drop(tx);
    assert!(srv.board.tx_log().is_empty());
    src.stop();
}

#[test]
fn a_refused_setting_is_named_by_label_and_key() {
    let (_srv, mut src) = open(FakeOpts::default());
    let tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    let schema = &tx.capabilities().settings;
    assert_eq!(
        refused(tx::IiodTxSetting::Output).text(schema),
        "the device refused setting \"Output\" (txoutput)"
    );
    assert!(schema
        .setting(schema.id(tx::TX_OUTPUT.key()).unwrap())
        .is_some_and(|d| d.advanced));
    drop(tx);
    src.stop();
}

#[test]
fn slow_transfers_at_the_start_of_a_burst_fall_in_its_zeros() {
    use std::sync::atomic::Ordering::Relaxed;
    // After a pause, each of the first three transfers takes 1.5 times the time its buffer plays: the converter runs dry
    // before each of the next three.
    let (srv, mut src) = open(FakeOpts {
        tx_plays: true,
        tx_slow_start: Some((3, 1.5)),
        ..Default::default()
    });
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    tx.arm().unwrap();
    let buf = 65_536;
    let iq: Vec<Complex32> = ramp(3 * buf)
        .iter()
        .map(|c| c + Complex32::new(0.4, 0.0))
        .collect();
    for burst in 1..=2u64 {
        let mut done = 0;
        while done < iq.len() {
            done += tx.push(&iq[done..], Duration::from_millis(100)).unwrap();
        }
        tx.flush().unwrap();
        assert!(tx.drain(Duration::from_secs(3)), "burst {burst}");
        let dry = srv.board.tx_gap.load(Relaxed);
        assert!(
            dry > burst * 3 * (buf as u64 / 4),
            "burst {burst}: the converter ran dry for {dry} samples of time"
        );
        // All of the dry time is before a buffer of zeros: none between two buffers of the signal.
        assert_eq!(srv.board.tx_gap_in_signal.load(Relaxed), 0);
        let s = tx.stats();
        assert_eq!((s.underruns, s.underrun_samples), (0, 0), "{s:?}");
        assert_eq!((s.sent, s.buffers), (burst * 3 * buf as u64, burst * 3));
        // The pause that makes the transfers of the next burst slow.
        std::thread::sleep(Duration::from_millis(150));
    }
    // Every sample is on the wire, after the zeros of each burst.
    let all = sent(&srv, false, false);
    let first = after_lead(&all, 2.4 * M);
    assert!(close_to(first, &iq));
    assert!(close_to(after_lead(&first[iq.len()..], 2.4 * M), &iq));
    tx.disarm();
    assert_eq!(srv.board.tx_cut.load(Relaxed), 0);
    src.stop();
}

#[test]
fn the_fake_counts_dry_time_between_buffers_of_a_signal() {
    use std::sync::atomic::Ordering::Relaxed;
    // The measure of the test above is not blind: a pause inside a burst is dry time in the signal.
    let (srv, mut src) = open(FakeOpts {
        tx_plays: true,
        ..Default::default()
    });
    let mut tx = src.transmitter_w(&tx_tune(&[])).unwrap();
    tx.arm().unwrap();
    let iq: Vec<Complex32> = ramp(65_536)
        .iter()
        .map(|c| c + Complex32::new(0.4, 0.0))
        .collect();
    tx.push(&iq, Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 1, 2.0));
    std::thread::sleep(Duration::from_millis(500));
    tx.push(&iq, Duration::from_secs(1)).unwrap();
    assert!(wait_for(|| tx.stats().buffers == 2, 2.0));
    assert!(wait_for(
        || srv.board.tx_gap_in_signal.load(Relaxed) > 0,
        2.0
    ));
    assert_eq!(tx.stats().underruns, 1);
    src.stop();
}
