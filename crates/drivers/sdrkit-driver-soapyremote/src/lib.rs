//! The SoapyRemote client driver. A SoapyRemote server (`SoapySDRServer --bind`, port 55132) shares the SoapySDR radios of one
//! computer. The driver receives channel 0 of the remote radio and can transmit; [`discovery`] finds servers on the local
//! network.
//!
//! Standard: the SoapyRemote protocol, RPC version 0x0400 (`rpc::VERSION`).

#![warn(missing_docs)]

pub mod discovery;
mod format;
mod log;
mod plan;
mod radio;
pub mod rpc;
pub mod stream;
mod tx;

pub use format::WireChoice;
pub use tx::{SoapyTxSetting, TX_OUTPUT, TX_PPM};

use radio::{Ctx, Options, SoapyRadio};
use rpc::{call, Packer};
use sdrkit_core::net::Transport;
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Results, Setting, SettingDecl, SettingSet, TableDef, Values};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, Doing, DriverDescriptor, DriverError, Link, LossReport, Polled, Retuned,
    RingWriter, Source, SourceDriver, Started, StreamFormat, Transmitter,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The timings of the driver.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    /// The reply timeout of every call not listed below.
    pub call: Duration,
    /// Make (call 1).
    pub make: Duration,
    /// Set up stream (call 300).
    pub setup: Duration,
    /// Each call of the close sequence.
    pub close_call: Duration,
    /// The start reply of the log connection.
    pub log_start: Duration,
    /// The first datagram after activate.
    pub first_data: Duration,
    /// No datagram for this long while active is a loss.
    pub stall: Duration,
    /// The wait slice of the read loops (stop checks, the acknowledgement rule).
    pub slice: Duration,
    /// Connect timeout to a remote host.
    pub connect_remote: Duration,
    /// Connect timeout to a host on this device.
    pub connect_local: Duration,
    /// Throughput windows.
    pub throughput_window: Duration,
    /// Stream set-up attempts at the first start.
    pub stream_attempts: u32,
    /// The gap between those attempts.
    pub retry_gap: Duration,
    /// A transmit write fails after this many slices without an acknowledgement.
    pub tx_waits: u32,
    /// The shortest gap between sensor reads.
    pub sensor_gap: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            call: Duration::from_secs(10),
            make: Duration::from_secs(20),
            setup: Duration::from_secs(10),
            close_call: Duration::from_secs(1),
            log_start: Duration::from_secs(5),
            first_data: Duration::from_secs(3),
            stall: Duration::from_secs(3),
            slice: Duration::from_millis(100),
            connect_remote: Duration::from_secs(8),
            connect_local: Duration::from_secs(10),
            throughput_window: Duration::from_secs(1),
            stream_attempts: 4,
            retry_gap: Duration::from_millis(500),
            tx_waits: 50,
            sensor_gap: Duration::from_secs(1),
        }
    }
}

impl Timing {
    /// The connect timeout for `host`.
    pub fn connect_timeout(&self, host: &str) -> Duration {
        if sdrkit_core::net::is_local(host) {
            self.connect_local
        } else {
            self.connect_remote
        }
    }
}

sdrkit_core::choice_enum! {
    /// The size of the datagrams ([`MTU`]).
    pub enum Mtu {
        /// 1500 bytes.
        Standard = ("1500", "Standard (1500)"),
        /// 9000 bytes.
        Jumbo = ("9000", "Jumbo frames (9000)"),
    }
}

impl Mtu {
    /// The size in bytes.
    pub fn bytes(self) -> usize {
        match self {
            Mtu::Standard => 1500,
            Mtu::Jumbo => 9000,
        }
    }
}

/// The clock error in ppm.
pub const PPM: Setting<field::F64> = Setting::new("ppm", "Frequency correction", "ppm", 0.0)
    .range(-200.0, 200.0, 0.01)
    .help(
        "The clock error in ppm, positive when the clock runs fast (signals show up a little low). The radio's own correction \
         is used if it has one. If not, the frequency asked is corrected.",
    );
/// Which radio of the server to use.
pub const REMOTE: Setting<field::Text> = Setting::new("remote", "Remote driver", "", "")
    .restart()
    .help(
        "Which radio of the server to use: a driver name (rtlsdr) or a list (driver=sdrplay,serial=1). Empty: the first radio \
         of the server. The server must run SoapySDRServer --bind. The protocol has no password: anyone on the network can use \
         the radio.",
    );
/// The format the server sends.
pub const FORMAT: Setting<field::Choice<WireChoice>> =
    Setting::choice("format", "IQ format on the network", WireChoice::Auto)
        .restart()
        .help(
            "What the server sends. Auto is the radio's own format (no conversion on the server). CS8 halves the data rate, \
             with less dynamic range. CF32 doubles it and gains nothing for most radios. 2.4 Msps of CS16 is about 79 Mbit/s \
             with headers, CS8 about 40 Mbit/s.",
        );
/// The size of the datagrams.
pub const MTU: Setting<field::Choice<Mtu>> = Setting::choice("mtu", "Datagram size", Mtu::Standard)
    .restart()
    .help(
        "1500 works on every network. Use jumbo frames only when every switch and both computers are set for them. If not, no \
         samples arrive.",
    );
/// The receive buffer to ask for.
pub const WINDOW: Setting<field::F64> = Setting::new("window", "Receive buffer", "MiB", 4.0)
    .range(0.25, 32.0, 0.25)
    .restart()
    .help("How much the network can have in flight. More helps on Wi-Fi at high rates.");
/// Forward the log of the server.
pub const SERVER_LOG: Setting<field::Bool> =
    Setting::new("serverlog", "Show the server's log", "", true)
        .restart()
        .help("The messages of the server appear in the log, marked server:.");
/// The RF gain state of a radio with an RF gain-reduction stage. -1: the gain sets it.
pub const RF_STATE: Setting<field::I64> = Setting::new("rfgr", "RF gain state", "", -1)
    .range(-1.0, 255.0, 1.0)
    .help(
        "The RF gain stage (LNA state) of a radio with gain reduction stages. 0 is the most gain. Higher states reduce it. -1 \
         lets the gain set both stages. Any other state makes the gain drive the IF stage only. A state the radio does not \
         have is refused.",
    );
/// The antenna input. Empty: the radio's default.
pub const ANTENNA: Setting<field::Text> = Setting::new("antenna", "Antenna", "", "")
    .restart()
    .help("The antenna input of the radio. Empty keeps the radio's default. A name the radio does not list is refused.");
/// The clock source. Empty: as the radio is.
pub const CLOCK_SOURCE: Setting<field::Text> = Setting::new("clocksrc", "Clock source", "", "")
    .restart()
    .help(
        "Where the radio takes its clock from (internal, external, gpsdo). Empty keeps it as it is. A source the radio does \
         not list is refused.",
    );
/// The time source. Empty: as the radio is.
pub const TIME_SOURCE: Setting<field::Text> = Setting::new("timesrc", "Time source (PPS)", "", "")
    .restart()
    .help(
        "Where the radio takes its one pulse per second from. Empty keeps it as it is. A source the radio does not list is \
         refused.",
    );

sdrkit_core::settings! {
    /// The own settings of the SoapyRemote driver.
    pub enum SoapyRemoteSetting {
        /// [`PPM`].
        Ppm = PPM,
        /// [`REMOTE`].
        Remote = REMOTE,
        /// [`FORMAT`].
        Format = FORMAT,
        /// [`MTU`].
        Mtu = MTU,
        /// [`WINDOW`].
        Window = WINDOW,
        /// [`SERVER_LOG`].
        ServerLog = SERVER_LOG,
        /// [`RF_STATE`].
        RfState = RF_STATE,
        /// [`ANTENNA`].
        Antenna = ANTENNA,
        /// [`CLOCK_SOURCE`].
        ClockSource = CLOCK_SOURCE,
        /// [`TIME_SOURCE`].
        TimeSource = TIME_SOURCE,
    }
}

/// The server id.
pub const SERVER_ID: Field<field::Text> = Field::new("server_id", "Server id", "");
/// The RPC version of the server ("4.0").
pub const RPC_VERSION: Field<field::Text> = Field::new("rpc_version", "RPC version", "");
/// The SoapySDR driver of the radio.
pub const DRIVER: Field<field::Text> = Field::new("driver", "Driver", "");
/// The hardware key of the radio.
pub const HARDWARE: Field<field::Text> = Field::new("hardware", "Hardware", "");
/// The serial number of the radio.
pub const SERIAL: Field<field::Text> = Field::new("serial", "Serial number", "");
/// The receive channels of the radio.
pub const RX_CHANNELS: Field<field::I64> = Field::new("rx_channels", "Receive channels", "");
/// The transmit channels of the radio.
pub const TX_CHANNELS: Field<field::I64> = Field::new("tx_channels", "Transmit channels", "");
/// The format on the network.
pub const WIRE_FORMAT: Field<field::Text> = Field::new("wire_format", "Format on the network", "");
/// The native format of the radio.
pub const NATIVE_FORMAT: Field<field::Text> = Field::new("native_format", "Native format", "");
/// The full scale of the native format.
pub const NATIVE_SCALE: Field<field::F64> = Field::new("native_scale", "Native full scale", "");
/// The size of the datagrams.
pub const DATAGRAM: Field<field::I64> = Field::new("mtu_bytes", "Datagram size", "B");
/// The requested receive buffer size.
pub const WINDOW_BYTES: Field<field::I64> = Field::new("window_bytes", "Receive buffer", "B");
/// The window of the stream in packets.
pub const WINDOW_PACKETS: Field<field::I64> = Field::new("window_packets", "Window", "packets");
/// The samples in a full datagram.
pub const PER_DATAGRAM: Field<field::I64> = Field::new("per_datagram", "Samples per datagram", "");
/// The data rate over the last throughput window, headers included.
pub const MBIT_S: Field<field::F64> = Field::new("mbit_s", "Link speed", "Mbit/s");
/// The sample rate that arrived over the last throughput window.
pub const ARRIVING: Field<field::Hz> = Field::new("arriving_hz", "Arriving rate", "Hz");
/// Overflows at the server since the start.
pub const OVERFLOWS: Field<field::I64> = Field::new("overflows", "Overflows at the server", "");
/// Datagrams lost on the network since the start.
pub const LOST_DATAGRAMS: Field<field::I64> = Field::new("lost_datagrams", "Datagrams lost", "");
/// Late or repeated datagrams dropped since the start.
pub const LATE: Field<field::I64> = Field::new("late_datagrams", "Late datagrams", "");
/// Malformed or cut datagrams dropped since the start.
pub const MALFORMED: Field<field::I64> =
    Field::new("malformed_datagrams", "Malformed datagrams", "");
/// Other stream errors of the server since the start.
pub const STREAM_ERRORS: Field<field::I64> = Field::new("stream_errors", "Stream errors", "");
/// Reconnects since the start.
pub const RECONNECTS: Field<field::I64> = Field::new("reconnects", "Reconnects", "");
/// Lines of the server's log dropped by the rate limit.
pub const LOG_DROPPED: Field<field::I64> =
    Field::new("log_dropped", "Server log lines dropped", "");

/// The readings of a SoapyRemote radio.
pub const READINGS: &[&dyn Declare] = &[
    &SERVER_ID,
    &RPC_VERSION,
    &DRIVER,
    &HARDWARE,
    &SERIAL,
    &RX_CHANNELS,
    &TX_CHANNELS,
    &WIRE_FORMAT,
    &NATIVE_FORMAT,
    &NATIVE_SCALE,
    &DATAGRAM,
    &WINDOW_BYTES,
    &WINDOW_PACKETS,
    &PER_DATAGRAM,
    &MBIT_S,
    &ARRIVING,
    &OVERFLOWS,
    &LOST_DATAGRAMS,
    &LATE,
    &MALFORMED,
    &STREAM_ERRORS,
    &RECONNECTS,
    &LOG_DROPPED,
];

/// The name of a row of [`SENSORS`] and [`HARDWARE_INFO`].
pub const ITEM_NAME: Field<field::Text> = Field::new("name", "Name", "");
/// The value of a row of [`SENSORS`] and [`HARDWARE_INFO`], as the radio provides it.
pub const ITEM_VALUE: Field<field::Text> = Field::new("value", "Value", "");

/// The sensors of the radio, one row per sensor (row key: its name).
pub static SENSORS: TableDef = TableDef {
    key: "sensors",
    label: "Sensors",
    columns: &[&ITEM_NAME, &ITEM_VALUE],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The hardware information of the radio, one row per item (row key: its name).
pub static HARDWARE_INFO: TableDef = TableDef {
    key: "hardware_info",
    label: "Hardware information",
    columns: &[&ITEM_NAME, &ITEM_VALUE],
    cap: 64,
    marks: sdrkit_core::RowMarks::NONE,
};

const fn iq(sample: SampleType) -> StreamFormat {
    StreamFormat {
        kind: Kind::Iq,
        sample,
        min_rate_hz: 0.0,
        max_rate_hz: f64::MAX,
        channels: 1,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "soapyremote",
    name: "SoapyRemote server",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &[
        "soapyremote:<host> (port 55132)",
        "soapyremote:<host>:<port>",
    ],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(plan::DEFAULT_PORT)),
    ],
    settings: &[SoapyRemoteSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[
        iq(SampleType::Cs16),
        iq(SampleType::Cs8),
        iq(SampleType::Cu8),
        iq(SampleType::Cf32),
    ],
    accepts: &[iq(SampleType::Cf32)],
    tables: &[&SENSORS, &HARDWARE_INFO],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The errors of a SoapyRemote server that the driver core has no variant for.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SoapyError {
    /// The server refused a call, with the message of its radio driver.
    #[error("the SoapyRemote server refused call {call}: {text}")]
    Exception {
        /// The call number.
        call: u32,
        /// The message of the server.
        text: String,
    },
    /// The Remote driver setting is empty and the server provides no radio.
    #[error("the SoapyRemote server gives no radio")]
    NoRadio,
    /// The server has no radio that matches the Remote driver setting.
    #[error("the SoapyRemote server has no radio that matches the Remote driver setting")]
    NoMatch,
    /// The radio has no receive channel.
    #[error("the radio behind the SoapyRemote server has no receiver")]
    NoReceiver,
    /// The stream port in the reply of call 300 is not a port number.
    #[error("the SoapyRemote server gave a stream port that is not a number")]
    StreamPort,
    /// The server started the stream, but no datagram arrived. A firewall or NAT can drop them.
    #[error(
        "the stream started, but no samples arrived (a firewall or NAT can drop the datagrams)"
    )]
    NoSamples,
    /// A reconnect returns another format on the network than the first open.
    #[error("the radio now gives {now:?}, it gave {was:?} when it opened")]
    FormatChanged {
        /// The sample type of the first open.
        was: SampleType,
        /// The sample type now.
        now: SampleType,
    },
    /// The radio accepted no transmit samples for the time the driver waits.
    #[error("the radio stopped taking samples")]
    TxStalled,
    /// A SoapyRemote transmitter cannot repeat a buffer by itself.
    #[error("a SoapyRemote transmitter cannot repeat a buffer")]
    NoCyclic,
    /// The system does not allow this program to search the local network.
    #[error(
        "this program has no permission to search the local network: add the server by its address"
    )]
    SearchNotAllowed,
    /// No search went out.
    #[error("the network search did not go out")]
    SearchNotSent,
}

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The SoapyRemote driver.
#[derive(Clone)]
pub struct SoapyRemoteDriver {
    transport: Arc<dyn Transport>,
    timing: Timing,
}

impl SoapyRemoteDriver {
    /// A driver that reaches its servers through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with its own timings, for tests and slow links.
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        SoapyRemoteDriver { transport, timing }
    }
}

impl SourceDriver for SoapyRemoteDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        Ok(Box::new(self.open_source(address, values)?))
    }
}

impl SoapyRemoteDriver {
    /// Opens the radio at `address` as its own type.
    fn open_source(&self, address: &str, values: &Values) -> Result<SoapySource, DriverError> {
        let (host, port) = plan::clean_address(address)?;
        let ctx = Arc::new(Ctx::new(
            self.transport.clone(),
            &host,
            port,
            self.timing.clone(),
        ));
        let c = ctx.clone();
        let opener: Opener<SoapyRadio> =
            Arc::new(move |values, abort, notices| radio::open(&c, values, abort, notices));
        let runner = StreamRunner::open(
            RunnerConfig::new(format!("SoapyRemote {}", ctx.label), NETWORK_ADVICE),
            opener,
            values,
        )?;
        Ok(SoapySource {
            runner,
            opts: Options::from_values(values),
            ctx,
            tx: None,
            refreshing: Arc::new(AtomicBool::new(false)),
            starts: 0,
        })
    }
}

/// An open SoapyRemote radio: the runner, the reads of the sensors, and the transmitter.
struct SoapySource {
    runner: StreamRunner<SoapyRadio>,
    opts: Options,
    ctx: Arc<Ctx>,
    tx: Option<Arc<tx::TxShared>>,
    refreshing: Arc<AtomicBool>,
    starts: u32,
}

impl SoapySource {
    /// Reads the sensors again on a helper thread, at most once per [`Timing::sensor_gap`].
    fn refresh_sensors(&self) {
        let due = {
            let s = lock(&self.ctx.sensors);
            !s.0.is_empty()
                && s.1
                    .is_none_or(|t| t.elapsed() >= self.ctx.timing.sensor_gap)
        };
        if !due || self.refreshing.swap(true, Ordering::AcqRel) {
            return;
        }
        let ctx = self.ctx.clone();
        let flag = self.refreshing.clone();
        let spawned = std::thread::Builder::new()
            .name("sdrkit-soapy-sensors".into())
            .spawn(move || {
                let conn = lock(&ctx.ctl).clone();
                if let Some(conn) = conn {
                    let names: Vec<String> =
                        lock(&ctx.sensors).0.iter().map(|s| s.0.clone()).collect();
                    let mut out = Vec::new();
                    for n in names {
                        let mut p = Packer::call(call::READ_SENSOR);
                        p.string(&n);
                        let mut c = lock(&conn);
                        if c.is_broken() {
                            break;
                        }
                        let v = c
                            .call(&p, ctx.timing.call, |u| u.string())
                            .unwrap_or_default();
                        out.push((n, v));
                    }
                    if !out.is_empty() {
                        *lock(&ctx.sensors) = (out, Some(Instant::now()));
                    }
                }
                flag.store(false, Ordering::Release);
            });
        if spawned.is_err() {
            self.refreshing.store(false, Ordering::Release);
        }
    }
}

/// Writes (name, value) rows into the table `def` of `results`.
fn write_items<'a>(
    results: &mut Results,
    def: &TableDef,
    items: impl Iterator<Item = (&'a str, &'a str)>,
) {
    let Some(id) = results.decl().table_id(def.key) else {
        return;
    };
    let Some(mut t) = results.table_mut(id) else {
        return;
    };
    t.clear();
    for (k, v) in items.take(def.cap) {
        let mut row = t.row_mut(k);
        row.set(&ITEM_NAME, k);
        row.set(&ITEM_VALUE, v);
    }
}

impl Source for SoapySource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        // The counts start again at each start. The first start counts from the open, so the server's log at open counts.
        self.starts += 1;
        if self.starts > 1 {
            self.ctx.stats.reset();
        }
        self.runner.start(sink)
    }
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        if Options::from_values(values) != self.opts {
            return Ok(Retuned::NeedsRestart);
        }
        self.runner.apply(values)
    }
    fn stop(&mut self) {
        if let Some(t) = self.tx.take() {
            t.close();
        }
        self.runner.stop();
    }
    fn sample_type(&self) -> SampleType {
        self.runner.sample_type()
    }
    fn link(&self) -> Link {
        self.runner.link()
    }
    fn settings(&self) -> Vec<SettingDecl> {
        self.runner.settings()
    }
    fn poll(&mut self, results: &mut Results) -> Polled {
        self.refresh_sensors();
        self.runner.poll(results)
    }
    fn loss(&self) -> Option<LossReport> {
        self.runner.loss()
    }
    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }
    fn tx_schema(&self) -> Option<Arc<sdrkit_core::Schema>> {
        let info = lock(&self.ctx.info);
        let limits = info.facts.tx.as_ref()?;
        Some(sdrkit_driver_core::tx_schema(
            tx::SoapyTxSetting::SETTINGS,
            limits,
        ))
    }
    fn transmitter(&mut self, values: &Values) -> Result<Box<dyn Transmitter>, DriverError> {
        if self.tx.as_ref().is_some_and(|t| !t.is_closed()) {
            return Err(DriverError::NotNow {
                state: Doing::Running,
            });
        }
        let remote = lock(&self.ctx.remote).clone();
        let (t, shared) = tx::SoapyTransmitter::open(&self.ctx, &remote, values)?;
        self.tx = Some(shared);
        Ok(Box::new(t))
    }
}

impl Drop for SoapySource {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests;
