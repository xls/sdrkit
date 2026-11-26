//! The TCI client driver. TCI (Transceiver Control Interface) is served by an SDR program on a computer (ExpertSDR3 for the
//! SunSDR radios, and compatible programs), not by the radio; one WebSocket carries text commands and binary stream packets both
//! ways. The driver receives the IQ stream of one receiver, places its panorama on the tuned frequency and picks the IQ rate.
//! The transmitter sends audio for the modulator of the server; a receive-only server has none.
//!
//! Standard: Expert Electronics TCI Protocol, versions 1.4 to 2.0.

#![warn(missing_docs)]

pub mod proto;
mod radio;
mod tx;

use proto::ServerInfo;
use radio::{Shared, TciRadio, TxShared};
use sdrkit_core::net::{host_port, show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Hz, Results, Setting, SettingSet, Values};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverDescriptor, DriverError, Link, LossReport, Polled, RateLimits,
    Retuned, RingWriter, Source, SourceDriver, Started, StreamFormat, Transmitter,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The frequency range of a server that returned no VFO limits.
pub const DEFAULT_MIN_FREQ: Hz = 10e3;
/// See [`DEFAULT_MIN_FREQ`].
pub const DEFAULT_MAX_FREQ: Hz = 6e9;

/// The receiver whose IQ the driver takes, counted from 0.
pub const RECEIVER: Setting<field::I64> = Setting::new("receiver", "Receiver", "", 0)
    .range(0.0, (proto::MAX_RECEIVERS - 1) as f64, 1.0)
    .section(sdrkit_driver_core::form::OTHER_SECTION)
    .restart()
    .help(
        "The receiver of the server whose IQ to take, counted from 0. A receiver the server does not have is refused.",
    );

sdrkit_core::settings! {
    /// The own settings of the TCI driver.
    pub enum TciSetting {
        /// [`RECEIVER`].
        Receiver = RECEIVER,
    }
}

/// The server program and its TCI version.
pub const SERVER: Field<field::Text> = Field::new("server", "Server", "");
/// The name of the radio.
pub const DEVICE: Field<field::Text> = Field::new("device", "Device", "");
/// The radio cannot transmit.
pub const RECEIVE_ONLY: Field<field::Bool> = Field::new("receive_only", "Receive only", "");
/// The receivers of the server.
pub const RECEIVERS: Field<field::I64> = Field::new("receivers", "Receivers", "");
/// The VFO channels of each receiver.
pub const CHANNELS: Field<field::I64> = Field::new("channels", "Channels per receiver", "");
/// The lowest VFO frequency of the server.
pub const VFO_MIN: Field<field::Hz> = Field::new("vfo_min_hz", "Lowest VFO", "Hz");
/// The highest VFO frequency of the server.
pub const VFO_MAX: Field<field::Hz> = Field::new("vfo_max_hz", "Highest VFO", "Hz");
/// The lowest IF offset of the server.
pub const IF_MIN: Field<field::Hz> = Field::new("if_min_hz", "Lowest IF offset", "Hz");
/// The highest IF offset of the server.
pub const IF_MAX: Field<field::Hz> = Field::new("if_max_hz", "Highest IF offset", "Hz");
/// The receiver in use, counted from 0.
pub const RECEIVER_IN_USE: Field<field::I64> = Field::new("receiver", "Receiver", "");
/// The level the server measures on the receiver in use.
pub const SMETER: Field<field::F64> = Field::new("smeter_dbm", "S-meter (server)", "dBm");
/// The server transmits on the receiver in use.
pub const SERVER_TRX: Field<field::Bool> = Field::new("server_trx", "Server transmits", "");
/// Transmit audio packets sent.
pub const TX_PACKETS: Field<field::I64> = Field::new("tx_packets", "Transmit packets", "");
/// Transmit audio frames sent.
pub const TX_FRAMES: Field<field::I64> = Field::new("tx_frames", "Transmit frames", "");
/// Transmit audio frames that were missing when a packet went out.
pub const TX_UNDERRUN_FRAMES: Field<field::I64> =
    Field::new("tx_underrun_frames", "Transmit underrun frames", "");
/// The IQ rate the server confirmed.
pub const IQ_RATE: Field<field::Hz> = Field::new("iq_rate_hz", "IQ rate", "Hz");
/// The sample type of the last usable packet.
pub const FORMAT: Field<field::Text> = Field::new("format", "Sample type", "");
/// Packets the driver dropped: malformed, or not in the sample type of the ring.
pub const UNUSABLE: Field<field::I64> = Field::new("unusable", "Packets dropped", "");
/// The radio of the server runs.
pub const SERVER_RUNNING: Field<field::Bool> =
    Field::new("server_running", "Server radio runs", "");
/// The panorama centre the server reported, when it differs from the one sent.
pub const PANORAMA: Field<field::Hz> = Field::new("panorama_hz", "Panorama (server)", "Hz");
/// The IQ data rate over the last throughput window.
pub const MBIT_S: Field<field::F64> = Field::new("mbit_s", "Link speed", "Mbit/s");
/// The sample rate that arrived over the last throughput window.
pub const ARRIVING: Field<field::Hz> = Field::new("arriving_hz", "Arriving rate", "Hz");
/// Less than 95 % of the rate arrived for three windows: the network or the server is the limit.
pub const LOW_THROUGHPUT: Field<field::Bool> = Field::new("low_throughput", "Link too slow", "");

/// The readings of a TCI radio.
pub const READINGS: &[&dyn Declare] = &[
    &SERVER,
    &DEVICE,
    &RECEIVE_ONLY,
    &RECEIVERS,
    &CHANNELS,
    &VFO_MIN,
    &VFO_MAX,
    &IF_MIN,
    &IF_MAX,
    &RECEIVER_IN_USE,
    &SMETER,
    &SERVER_TRX,
    &TX_PACKETS,
    &TX_FRAMES,
    &TX_UNDERRUN_FRAMES,
    &IQ_RATE,
    &FORMAT,
    &UNUSABLE,
    &SERVER_RUNNING,
    &PANORAMA,
    &MBIT_S,
    &ARRIVING,
    &LOW_THROUGHPUT,
];

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "tci",
    name: "TCI (network)",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &["tci:<host> (port 50001)", "tci:<host>:<port>"],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(proto::DEFAULT_PORT)),
    ],
    settings: &[TciSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cf32,
        min_rate_hz: proto::IQ_RATES[0] as f64,
        max_rate_hz: proto::IQ_RATES[3] as f64,
        channels: 1,
    }],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The errors of TCI that the driver core has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TciError {
    /// The server says its radio cannot transmit.
    #[error("this TCI server is receive only")]
    ReceiveOnly,
    /// The server does not let this receiver transmit now.
    #[error("the TCI server does not let receiver {receiver} transmit now")]
    ReceiverMayNotTransmit {
        /// The receiver, counted from 0.
        receiver: usize,
    },
    /// The server transmits audio: a repeated IQ buffer is not offered.
    #[error("the TCI server transmits audio: a repeated buffer is not offered")]
    NoCyclic,
}

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The timings of the driver. Tests shorten them. `fixed_mask` and `fixed_key` make client frames repeatable.
#[derive(Clone, Debug)]
pub struct Timing {
    /// The connect timeout (3 s).
    pub connect: Duration,
    /// Each handshake read (3 s).
    pub handshake: Duration,
    /// No message for this long ends the read loop (5 s).
    pub stall: Duration,
    /// The read timeout of the connection, in which reads look for a stop (100 ms).
    pub tick: Duration,
    /// Each read of the drain at the close (100 ms).
    pub drain_step: Duration,
    /// The whole drain at the close (400 ms).
    pub drain_total: Duration,
    /// The throughput window (1 s).
    pub window: Duration,
    /// A fixed mask for every client frame (tests only).
    pub fixed_mask: Option<[u8; 4]>,
    /// A fixed `Sec-WebSocket-Key` (tests only).
    pub fixed_key: Option<[u8; 16]>,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect: Duration::from_secs(3),
            handshake: Duration::from_secs(3),
            stall: Duration::from_secs(5),
            tick: Duration::from_millis(100),
            drain_step: Duration::from_millis(100),
            drain_total: Duration::from_millis(400),
            window: Duration::from_secs(1),
            fixed_mask: None,
            fixed_key: None,
        }
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Throughput {
    /// Binary payload bits per second after the headers, in millions.
    pub mbit_s: f64,
    /// Complex samples per second that arrived.
    pub achieved_hz: Hz,
    /// The achieved rate stayed below 95 % of the rate of the driver for 3 windows.
    pub low: bool,
}

/// Splits `host` or `host:port`. The port is 50001 when the address names none.
fn parse_address(address: &str) -> Result<(String, u16), DriverError> {
    host_port(address, Some(proto::DEFAULT_PORT))
        .map(|(h, p)| (h.to_owned(), p))
        .map_err(|_| DriverError::Address)
}

/// The rates as single-rate ranges, slowest first.
pub(crate) fn rate_limits() -> RateLimits {
    RateLimits {
        min_hz: Hz::from(proto::IQ_RATES[0]),
        max_hz: Hz::from(proto::IQ_RATES[3]),
        ranges: proto::IQ_RATES
            .iter()
            .map(|&r| (Hz::from(r), Hz::from(r)))
            .collect(),
        steady_hz: None,
    }
}

/// The capabilities for a server.
pub(crate) fn caps_for(label: &str, info: &ServerInfo) -> Capabilities {
    let (min, max) = info
        .vfo_limits
        .unwrap_or((DEFAULT_MIN_FREQ, DEFAULT_MAX_FREQ));
    let name = match info.device.as_deref().filter(|d| !d.is_empty()) {
        Some(d) => format!("{d} (TCI) at {label}"),
        None => format!("TCI server at {label}"),
    };
    Capabilities {
        name,
        rates: rate_limits(),
        min_freq_hz: min,
        max_freq_hz: max,
        gain_db: Some((0.0, 0.0, 1.0)),
        bandwidth_hz: None,
        has_agc: false,
        transmit: (!info.is_receive_only()).then(|| tx::limits(info)),
        has_bias_tee: false,
        realtime: true,
        common_rates_hz: Vec::new(),
        common_bandwidths_hz: Vec::new(),
        gain_steps_db: Vec::new(),
    }
}

/// The TCI driver. It keeps, per server address, what the server said and the last throughput.
#[derive(Clone)]
pub struct TciDriver {
    transport: Arc<dyn Transport>,
    entries: Arc<Mutex<HashMap<String, Shared>>>,
    timing: Timing,
}

impl TciDriver {
    /// A driver that reaches its servers through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with other timings (tests, slow links).
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        TciDriver {
            transport,
            entries: Arc::default(),
            timing,
        }
    }

    fn entry(&self, label: &str) -> Shared {
        lock(&self.entries)
            .entry(label.to_string())
            .or_default()
            .clone()
    }
}

impl SourceDriver for TciDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) = parse_address(address)?;
        let label = show(&host, port);
        let entry = self.entry(&label);
        let tx: TxShared = Arc::default();
        let current: tx::Current = Arc::default();
        let at = radio::At {
            transport: self.transport.clone(),
            host,
            port,
            label: label.clone(),
            timing: self.timing.clone(),
        };
        let (e, t, c) = (entry.clone(), tx.clone(), current.clone());
        let opener: Opener<TciRadio> = Arc::new(move |values, abort, notices| {
            let r = TciRadio::connect(&at, e.clone(), t.clone(), values, abort, notices)?;
            *lock(&c) = Some(r.conn.clone());
            Ok(r)
        });
        let runner = StreamRunner::open(
            RunnerConfig::new(format!("TCI {label}"), NETWORK_ADVICE),
            opener,
            values,
        )?;
        Ok(Box::new(TciSource {
            runner,
            entry,
            label,
            tx,
            current,
            receiver: values.get(&RECEIVER),
        }))
    }
}

/// An open TCI radio: the runner, and the transmitter.
struct TciSource {
    runner: StreamRunner<TciRadio>,
    entry: Shared,
    label: String,
    tx: TxShared,
    current: tx::Current,
    receiver: i64,
}

impl Source for TciSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.runner.start(sink)
    }
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        // The receiver takes effect only at open.
        if values.get(&RECEIVER) != self.receiver {
            return Ok(Retuned::NeedsRestart);
        }
        self.runner.apply(values)
    }
    fn stop(&mut self) {
        self.runner.stop();
        lock(&self.current).take();
        let mut t = lock(&self.tx);
        t.gen = None;
        t.queue.clear();
    }
    fn sample_type(&self) -> SampleType {
        self.runner.sample_type()
    }
    fn link(&self) -> Link {
        self.runner.link()
    }
    fn poll(&mut self, results: &mut Results) -> Polled {
        self.runner.poll(results)
    }
    fn loss(&self) -> Option<LossReport> {
        self.runner.loss()
    }
    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }
    fn tx_schema(&self) -> Option<Arc<sdrkit_core::Schema>> {
        let limits = self.runner.capabilities().transmit.as_ref()?;
        Some(sdrkit_driver_core::tx_schema(&[], limits))
    }
    fn transmitter(&mut self, _values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        tx::refusal(&self.entry, None)?;
        Ok(Box::new(tx::TciTransmitter::new(
            self.tx.clone(),
            self.current.clone(),
            self.entry.clone(),
            &self.label,
        )))
    }
}

/// The id of the receiver setting, for refusals.
pub(crate) fn receiver_id() -> sdrkit_core::SettingId {
    own_id(TciSetting::Receiver)
}

#[cfg(test)]
mod tests;
