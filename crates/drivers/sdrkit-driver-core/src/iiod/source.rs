//! The iiod source: the shared base of the network radios running an [`IiodRadio`], plus what the base does not know: the
//! ring of the second receiver, the readings monitor, and the transmitter. The monitor reads the measurements of the board on
//! its own thread and connection, every [`READINGS_EVERY`](super::READINGS_EVERY) seconds, and starts no read while a
//! retune runs. Once a second it also reads the overflow flag of the capture core of the board, when the board offers its
//! registers, and counts each overflow: samples that the board dropped because they were not read in time. [`Source::poll`] takes a copy of the last readings and of the state of the held gain, and never waits on the
//! network.

use super::proto::{IioError, IiodConn, Target};
use super::tx::{IiodTransmitter, TxShared};
use super::{
    connect_ctl, lock, Address, AuxRing, Board, BoardFacts, IiodRadio, Known, IQ_FORMAT,
    READINGS_EVERY, RECEIVERS,
};
use crate::gain::{GainControl, HoldFields};
use crate::net::Abort;
use crate::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use crate::{
    Capabilities, Doing, DriverError, Link, LossReport, Polled, Retuned, RingWriter, Source,
    Started, StreamLayout, Transmitter,
};
use sdrkit_core::net::{show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Results, Schema, SettingSet, Values};
use sdrkit_graph::stream::SampleType;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What the source tells its monitor: the time between two passes, and that a retune runs.
#[derive(Default)]
struct MonitorCtl {
    /// The time between two passes, in ms. 0: the monitor does not read.
    period_ms: AtomicU64,
    retuning: AtomicBool,
}

impl MonitorCtl {
    fn set_period(&self, values: &Values) {
        let ms = (values.get(&READINGS_EVERY).max(0.0) * 1e3).round() as u64;
        self.period_ms.store(ms, Ordering::Release);
    }
}

/// The temperature of the AD936x.
pub const TEMPERATURE: Field<field::F64> = Field::new("temperature_c", "Temperature", "°C");
/// The RSSI of RX1.
pub const RSSI1: Field<field::F64> = Field::new("rssi1_db", "RX1 RSSI", "dB");
/// The RSSI of RX2.
pub const RSSI2: Field<field::F64> = Field::new("rssi2_db", "RX2 RSSI", "dB");
/// The gain of RX1 as the board reports it, read by the monitor.
pub const RX1_GAIN: Field<field::F64> = Field::new("rx1_gain_db", "RX1 gain", "dB");
/// The gain of RX2 as the board reports it.
pub const RX2_GAIN: Field<field::F64> = Field::new("rx2_gain_db", "RX2 gain", "dB");
/// The gain of RX1 that the held gain of the driver set.
pub const RX1_HELD_GAIN: Field<field::F64> = Field::new("rx1_held_gain_db", "RX1 held gain", "dB");
/// What the held gain of RX1 does: searching or holding.
pub const RX1_GAIN_CONTROL: Field<field::Choice<GainControl>> =
    Field::new("rx1_gain_control", "RX1 gain control", "");
/// The gain steps of the held gain of RX1 since the last retune.
pub const RX1_GAIN_STEPS: Field<field::I64> = Field::new("rx1_gain_steps", "RX1 gain steps", "");
/// The gain of RX2 that the held gain of the driver set.
pub const RX2_HELD_GAIN: Field<field::F64> = Field::new("rx2_held_gain_db", "RX2 held gain", "dB");
/// What the held gain of RX2 does.
pub const RX2_GAIN_CONTROL: Field<field::Choice<GainControl>> =
    Field::new("rx2_gain_control", "RX2 gain control", "");
/// The gain steps of the held gain of RX2 since the last retune.
pub const RX2_GAIN_STEPS: Field<field::I64> = Field::new("rx2_gain_steps", "RX2 gain steps", "");
/// The held gain of each receiver; absent while the gain is manual or the AGC of the AD936x runs.
pub const HOLDS: [HoldFields; 2] = [
    HoldFields {
        gain: RX1_HELD_GAIN,
        state: RX1_GAIN_CONTROL,
        steps: RX1_GAIN_STEPS,
    },
    HoldFields {
        gain: RX2_HELD_GAIN,
        state: RX2_GAIN_CONTROL,
        steps: RX2_GAIN_STEPS,
    },
];
/// The overflows of the capture buffer of the board since it was opened: each one dropped samples on the board. Absent on a
/// board that does not offer its registers.
pub const OVERFLOWS: Field<field::I64> = Field::new("rx_overflows", "Capture overflows", "");
/// The calibration mode of the AD936x, as the board provides it.
pub const CALIBRATION: Field<field::Text> = Field::new("calibration", "Calibration", "");
/// The state of the ENSM (the state machine of the AD936x), as the board provides it.
pub const ENSM: Field<field::Text> = Field::new("ensm", "State", "");

/// The readings of an iiod board.
pub const READINGS: &[&dyn Declare] = &[
    &TEMPERATURE,
    &RSSI1,
    &RSSI2,
    &RX1_GAIN,
    &RX2_GAIN,
    &RX1_HELD_GAIN,
    &RX1_GAIN_CONTROL,
    &RX1_GAIN_STEPS,
    &RX2_HELD_GAIN,
    &RX2_GAIN_CONTROL,
    &RX2_GAIN_STEPS,
    &OVERFLOWS,
    &CALIBRATION,
    &ENSM,
    &crate::TX_OUTPUT_DBM,
];

/// The last readings of a board. `None` for a reading the board does not give.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Readings {
    /// The temperature, in °C.
    pub temperature_c: Option<f64>,
    /// The RSSI of each receiver, in dB.
    pub rssi_db: [Option<f64>; 2],
    /// The gain of each receiver, in dB.
    pub gain_db: [Option<f64>; 2],
    /// The calibration mode.
    pub calibration: Option<String>,
    /// The state of the ENSM.
    pub ensm: Option<String>,
}

impl Readings {
    fn write(&self, r: &mut Results) {
        let mut f = r.fields_mut();
        f.opt(&TEMPERATURE, self.temperature_c);
        f.opt(&RSSI1, self.rssi_db[0]);
        f.opt(&RSSI2, self.rssi_db[1]);
        f.opt(&RX1_GAIN, self.gain_db[0]);
        f.opt(&RX2_GAIN, self.gain_db[1]);
        f.opt(&CALIBRATION, self.calibration.as_deref());
        f.opt(&ENSM, self.ensm.as_deref());
    }
}

/// An open iiod board: the receiver (on the shared base), its readings, and its transmitter.
pub struct IiodSource {
    runner: StreamRunner<IiodRadio>,
    at: Address,
    board: Board,
    aux: AuxRing,
    /// The layout and the sample type of the values last taken, which the next stream has.
    layout: StreamLayout,
    sample: SampleType,
    monitor: Option<Monitor>,
    monitor_ctl: Arc<MonitorCtl>,
    tx: Option<Weak<TxShared>>,
}

impl IiodSource {
    /// Opens the board at `address` through `transport` and applies `values`, and keeps what it learns in `known`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Address`] for an address that is not one of a network board, else what the board returns.
    pub fn open(
        transport: Arc<dyn Transport>,
        address: &str,
        values: &Values,
        known: Arc<Mutex<HashMap<String, Known>>>,
    ) -> Result<IiodSource, DriverError> {
        let (host, port) = super::parse_address(address)?;
        let label = format!("ip:{}", show(&host, port));
        let at = Address {
            transport,
            host,
            port,
            label: label.clone(),
        };
        let board: Board = Arc::default();
        let aux: AuxRing = Arc::default();
        let (a2, b, x) = (at.clone(), board.clone(), aux.clone());
        let opener: Opener<IiodRadio> = Arc::new(move |v, abort, notices| {
            IiodRadio::connect(&a2, &known, &b, &x, v, abort, notices)
        });
        let runner = StreamRunner::open(
            RunnerConfig::new(format!("PlutoSDR {label}"), NETWORK_ADVICE),
            opener,
            values,
        )?;
        let layout = values.get(&RECEIVERS).layout();
        let sample = runner.sample_type();
        let monitor_ctl = Arc::<MonitorCtl>::default();
        monitor_ctl.set_period(values);
        Ok(IiodSource {
            runner,
            at,
            board,
            aux,
            layout,
            sample,
            monitor: None,
            monitor_ctl,
            tx: None,
        })
    }

    fn start_monitor(&mut self) {
        if self.monitor.is_none() {
            self.monitor = Some(Monitor::start(
                self.at.clone(),
                self.board.clone(),
                self.monitor_ctl.clone(),
            ));
        }
    }

    fn close_tx(&mut self) {
        if let Some(t) = self.tx.take().and_then(|w| w.upgrade()) {
            t.close();
        }
    }
}

impl Source for IiodSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }

    fn sample_type(&self) -> SampleType {
        self.sample
    }

    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.start_streams(vec![sink])
    }

    fn start_streams(&mut self, mut sinks: Vec<RingWriter>) -> Result<Started, DriverError> {
        let rings = match self.layout {
            StreamLayout::Separate(n) => n,
            _ => 1,
        };
        if sinks.is_empty() || sinks.len() > rings {
            return Err(DriverError::Streams {
                given: sinks.len(),
                has: rings,
            });
        }
        if sinks.len() == 2 {
            if let Some(second) = sinks.pop() {
                *lock(&self.aux) = Some(second);
            }
        }
        let first = sinks.pop().ok_or(DriverError::Streams {
            given: 0,
            has: rings,
        })?;
        let started = self.runner.start(first)?;
        self.start_monitor();
        Ok(started)
    }

    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        self.monitor_ctl.set_period(values);
        self.monitor_ctl.retuning.store(true, Ordering::Release);
        let r = self.runner.apply(values);
        self.monitor_ctl.retuning.store(false, Ordering::Release);
        if r.is_ok() {
            self.layout = values.get(&RECEIVERS).layout();
            self.sample = values.get(&IQ_FORMAT).sample_type();
        }
        r
    }

    fn stop(&mut self) {
        self.close_tx();
        if let Some(m) = self.monitor.take() {
            m.stop();
        }
        self.runner.stop();
        lock(&self.aux).take();
    }

    fn link(&self) -> Link {
        self.runner.link()
    }

    fn poll(&mut self, results: &mut Results) -> Polled {
        let (readings, holds, overflows) = {
            let b = lock(&self.board);
            (b.readings.clone(), b.gain_hold, b.overflows)
        };
        readings.write(results);
        let mut f = results.fields_mut();
        f.opt(
            &OVERFLOWS,
            overflows.map(|n| i64::try_from(n).unwrap_or(i64::MAX)),
        );
        for (fields, status) in HOLDS.iter().zip(holds) {
            fields.write(&mut f, status);
        }
        self.runner.poll(results)
    }

    fn loss(&self) -> Option<LossReport> {
        self.runner.loss()
    }

    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }

    fn layout(&self) -> StreamLayout {
        self.layout
    }

    fn tx_schema(&self) -> Option<Arc<Schema>> {
        let limits = self.capabilities().transmit.as_ref()?;
        Some(crate::tx_schema(super::tx::IiodTxSetting::SETTINGS, limits))
    }

    fn transmitter(&mut self, values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        if self
            .tx
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|t| !t.is_closed())
        {
            return Err(DriverError::NotNow {
                state: Doing::Transmitting,
            });
        }
        let t = IiodTransmitter::open(&self.at, self.board.clone(), values)?;
        self.tx = Some(Arc::downgrade(&t.shared()));
        Ok(Box::new(t))
    }
}

impl Drop for IiodSource {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The readings monitor thread.
struct Monitor {
    stop: Arc<AtomicBool>,
    abort: Arc<Abort>,
    thread: Option<JoinHandle<()>>,
}

impl Monitor {
    fn start(at: Address, board: Board, ctl: Arc<MonitorCtl>) -> Monitor {
        let stop = Arc::new(AtomicBool::new(false));
        let abort = Abort::new();
        let (s, a) = (stop.clone(), abort.clone());
        let thread = std::thread::Builder::new()
            .name("sdrkit-iiod-monitor".into())
            .spawn(move || monitor(&at, &board, &ctl, &s, &a))
            .ok();
        Monitor {
            stop,
            abort,
            thread,
        }
    }

    fn stop(mut self) {
        self.stop.store(true, Ordering::Release);
        self.abort.fire();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Why a pass of the monitor ended without readings.
enum Stopped {
    /// The connection failed.
    Broken,
    /// A retune started: the pass is made again after it.
    Deferred,
}

/// How often the monitor reads the overflow flag of the capture core.
const OVERFLOW_EVERY: Duration = Duration::from_secs(1);
/// The debug attribute through which iiod reads and writes the registers of a device.
const REG_ACCESS: &str = "direct_reg_access";
/// The status register of the capture core of the AD936x (in the core's register space) and its overflow flag, which a write
/// of the flag clears.
const STATUS_REG: &str = "0x80000088";
const OVERFLOW_FLAG: u64 = 1 << 2;

/// The loop of the monitor thread: one pass over the measurements per period, none while the period is 0, and one read of the
/// overflow flag a second; nothing while a retune runs.
fn monitor(at: &Address, board: &Board, ctl: &MonitorCtl, stop: &AtomicBool, abort: &Arc<Abort>) {
    let mut conn: Option<IiodConn> = None;
    let mut last: Option<Instant> = None;
    let mut last_flag: Option<Instant> = None;
    while !stop.load(Ordering::Acquire) {
        let period = Duration::from_millis(ctl.period_ms.load(Ordering::Acquire));
        let due = !period.is_zero() && last.is_none_or(|t| t.elapsed() >= period);
        let facts = lock(board).facts.clone();
        let has_reg = facts
            .as_ref()
            .is_some_and(|f| f.rx.debug_attrs.iter().any(|a| a == REG_ACCESS));
        let flag_due = has_reg && last_flag.is_none_or(|t| t.elapsed() >= OVERFLOW_EVERY);
        if !(due || flag_due) || ctl.retuning.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        if conn.is_none() {
            conn = connect_ctl(at, abort).ok().map(|(c, _, _)| c);
        }
        if flag_due {
            let read = match (conn.as_mut(), facts.as_ref()) {
                (Some(c), Some(f)) => overflowed(c, f, ctl),
                _ => Err(Stopped::Broken),
            };
            // The first read clears what an earlier client left.
            let first = last_flag.is_none();
            match read {
                Ok(o) => {
                    let mut b = lock(board);
                    let n = b.overflows.get_or_insert(0);
                    *n += u64::from(o && !first);
                }
                Err(Stopped::Broken) => {
                    if let Some(mut c) = conn.take() {
                        c.shutdown();
                    }
                    last_flag = Some(Instant::now());
                    continue;
                }
                Err(Stopped::Deferred) if ctl.retuning.load(Ordering::Acquire) => continue,
                Err(Stopped::Deferred) => {}
            }
            last_flag = Some(Instant::now());
        }
        if !due {
            continue;
        }
        let pass = match (conn.as_mut(), facts) {
            (Some(c), Some(f)) => read_all(c, &f, board, ctl),
            _ => Err(Stopped::Broken),
        };
        match pass {
            Ok(r) => {
                lock(board).readings = r;
                last = Some(Instant::now());
            }
            Err(Stopped::Broken) => {
                if let Some(mut c) = conn.take() {
                    c.shutdown();
                }
                last = Some(Instant::now());
            }
            Err(Stopped::Deferred) => {}
        }
    }
    if let Some(mut c) = conn {
        c.shutdown();
    }
}

/// Reads the overflow flag of the capture core, and clears it when it is set. True: the board overflowed since the last read.
fn overflowed(c: &mut IiodConn, f: &BoardFacts, ctl: &MonitorCtl) -> Result<bool, Stopped> {
    let fail = |e: IioError| match e {
        IioError::Io(_) => Stopped::Broken,
        _ => Stopped::Deferred,
    };
    let dev = f.rx.id.as_str();
    if ctl.retuning.load(Ordering::Acquire) {
        return Err(Stopped::Deferred);
    }
    c.write_attr(dev, Target::Debug, REG_ACCESS, STATUS_REG)
        .map_err(fail)?;
    let v = c.read_attr(dev, Target::Debug, REG_ACCESS).map_err(fail)?;
    let v = v.trim();
    let value = match v.strip_prefix("0x") {
        Some(h) => u64::from_str_radix(h, 16).ok(),
        None => v.parse().ok(),
    }
    .ok_or(Stopped::Deferred)?;
    if value & OVERFLOW_FLAG == 0 {
        return Ok(false);
    }
    let clear = format!("{STATUS_REG} 0x{OVERFLOW_FLAG:x}");
    c.write_attr(dev, Target::Debug, REG_ACCESS, &clear)
        .map_err(fail)?;
    Ok(true)
}

/// Reads every measurement that the board lists. It starts no read while a retune runs.
fn read_all(
    c: &mut IiodConn,
    f: &BoardFacts,
    board: &Board,
    ctl: &MonitorCtl,
) -> Result<Readings, Stopped> {
    let mut out = Readings::default();
    let number = |s: &str| {
        s.split_whitespace()
            .next()
            .and_then(|x| x.parse::<f64>().ok())
    };
    // `Ok(None)`: the board refused the read.
    let get = |c: &mut IiodConn, t: Target, attr: &str| -> Result<Option<String>, Stopped> {
        if ctl.retuning.load(Ordering::Acquire) {
            return Err(Stopped::Deferred);
        }
        match c.read_attr(&f.phy, t, attr) {
            Ok(v) => Ok(Some(v)),
            Err(IioError::Errno(_) | IioError::Malformed) => Ok(None),
            Err(IioError::Io(_)) => Err(Stopped::Broken),
        }
    };
    let mut read = || -> Result<(), Stopped> {
        if f.phy_has("temp0", false, "input") {
            if let Some(v) = get(c, Target::Channel(false, "temp0"), "input")? {
                let t = number(&v).map(|m| m / 1000.0);
                // The sensor now and then reads about -40 °C for one sample: the last good value stays then.
                out.temperature_c = match t {
                    Some(t) if (-39.0..150.0).contains(&t) => Some(t),
                    _ => lock(board).readings.temperature_c,
                };
            }
        }
        for (i, ch) in ["voltage0", "voltage1"]
            .iter()
            .enumerate()
            .take(f.receivers)
        {
            if f.phy_has(ch, false, "rssi") {
                if let Some(v) = get(c, Target::Channel(false, ch), "rssi")? {
                    out.rssi_db[i] = number(&v);
                }
            }
            if f.phy_has(ch, false, "hardwaregain") {
                if let Some(v) = get(c, Target::Channel(false, ch), "hardwaregain")? {
                    out.gain_db[i] = number(&v);
                }
            }
        }
        if f.phy_dev.has_attr("calib_mode") {
            out.calibration = get(c, Target::Device, "calib_mode")?;
        }
        if f.phy_dev.has_attr("ensm_mode") {
            if let Some(v) = get(c, Target::Device, "ensm_mode")? {
                lock(board).fdd = Some(v == "fdd");
                out.ensm = Some(v);
            }
        }
        Ok(())
    };
    read().map(|()| out)
}
