//! The SDR++ server client. SDR++ in its server mode streams the baseband of one radio to one client over TCP (port 5259). The
//! protocol carries the frequency, the sample format, compression and start and stop; the sample rate, the gain and every other
//! radio control are in the menu of the server, which it sends as a draw list. The driver operates the sample-rate list and the
//! gain sliders of that menu.
//!
//! Standard: the SDR++ server protocol (`server_protocol.h`).

#![warn(missing_docs)]

mod conn;
pub mod menu;
pub mod proto;
mod radio;

pub use radio::{corrected_frequency, pick_rate};

use conn::{Shortfall, Throughput};
use menu::Menu;
use proto::DEFAULT_PORT;
use radio::SdrppRadio;
use sdrkit_core::net::{host_port, show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Results, Setting, SettingDecl, SettingSet, Values};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, DriverDescriptor, DriverError, Link, LossReport, Polled, Retuned, RingWriter,
    Source, SourceDriver, Started, StreamFormat,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// What every log line starts with.
pub(crate) const PREFIX: &str = "SDR++ server: ";

sdrkit_core::choice_enum! {
    /// The sample format the server sends ([`SAMPLE_FORMAT`]).
    pub enum WireType {
        /// Signed 8-bit I and Q.
        I8 = ("i8", "8-bit"),
        /// Signed 16-bit I and Q.
        I16 = ("i16", "16-bit"),
        /// 32-bit float I and Q.
        F32 = ("f32", "32-bit float"),
    }
}

impl WireType {
    /// The sample type on the wire.
    pub fn wire(self) -> proto::SampleType {
        match self {
            WireType::I8 => proto::SampleType::I8,
            WireType::I16 => proto::SampleType::I16,
            WireType::F32 => proto::SampleType::F32,
        }
    }
}

/// The sample format the server sends.
pub const SAMPLE_FORMAT: Setting<field::Choice<WireType>> =
    Setting::choice("sample_type", "Sample format", WireType::I16).help(
        "What the server sends. 16-bit keeps weak signals beside strong ones. 8-bit needs half the network (2 bytes per \
         sample) and lets a slow link carry a higher rate. Float is exact but takes 8 bytes per sample (at 10 Msps: 160, 320 \
         and 640 Mbit/s).",
    );
/// The server compresses each packet.
pub const COMPRESSION: Setting<field::Bool> = Setting::new("compression", "Compression", "", false)
    .restart()
    .help(
        "The server compresses each packet (zstd). Less network, more CPU on both ends. Most useful with 8-bit or 16-bit \
         samples.",
    );
/// The driver never moves the gain sliders of the server.
pub const SERVER_GAIN: Setting<field::Bool> =
    Setting::new("server_gain", "Leave the gain to the server", "", false).help(
        "The driver never moves the gain sliders of the server. The gain stays as set on the server (or by its AGC).",
    );
/// The frequency correction.
pub const PPM: Setting<field::F64> = Setting::new("ppm", "Frequency correction", "ppm", 0.0)
    .range(-200.0, 200.0, 0.1)
    .help(
        "The clock error of the radio in ppm, positive when its clock runs fast (signals show a little low). The driver asks \
         the server for the corrected frequency. The sample rate is not corrected.",
    );

sdrkit_core::settings! {
    /// The own settings of the SDR++ server driver.
    pub enum SdrppSetting {
        /// [`SAMPLE_FORMAT`].
        SampleFormat = SAMPLE_FORMAT,
        /// [`COMPRESSION`].
        Compression = COMPRESSION,
        /// [`SERVER_GAIN`].
        ServerGain = SERVER_GAIN,
        /// [`PPM`].
        Ppm = PPM,
    }
}

/// The source module the server runs.
pub const SOURCE: Field<field::Text> = Field::new("source", "Source", "");
/// The rate the server announced.
pub const SERVER_RATE: Field<field::Hz> = Field::new("server_rate_hz", "Rate of the server", "Hz");
/// The data rate over the last throughput window.
pub const MBIT_S: Field<field::F64> = Field::new("mbit_s", "Link speed", "Mbit/s");
/// The sample rate that arrived over the last throughput window.
pub const ARRIVING: Field<field::Hz> = Field::new("arriving_hz", "Arriving rate", "Hz");
/// The last error the server reported, as it worded it.
pub const SERVER_ERROR: Field<field::Text> = Field::new("server_error", "Server error", "");
/// Another client holds the server.
pub const BUSY: Field<field::Bool> = Field::new("busy", "Another client", "");

/// The readings of an SDR++ server radio.
pub const READINGS: &[&dyn Declare] = &[
    &SOURCE,
    &SERVER_RATE,
    &MBIT_S,
    &ARRIVING,
    &SERVER_ERROR,
    &BUSY,
];

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "sdrpp",
    name: "SDR++ server",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &["sdrpp:<host> (port 5259)", "sdrpp:<host>:<port>"],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(DEFAULT_PORT)),
    ],
    settings: &[SdrppSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cf32,
        // The rates are those of the device behind the server.
        min_rate_hz: 0.0,
        max_rate_hz: f64::MAX,
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

/// The errors of an SDR++ server that the driver core has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SdrppError {
    /// The server has no source module loaded.
    #[error("the SDR++ server has no source module loaded")]
    NoSource,
}

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What is kept per server address.
#[derive(Debug, Default)]
pub(crate) struct Memory {
    pub rate: Option<f64>,
    pub menu: Option<Menu>,
    pub stopped_only: BTreeSet<String>,
    pub throughput: Option<Throughput>,
    pub last_error: Option<String>,
    pub busy: bool,
    pub connected: bool,
}

pub(crate) type Mem = Arc<Mutex<Memory>>;

/// The timings of the driver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// The TCP connect.
    pub connect: Duration,
    /// The wait for the rate of the server after connecting.
    pub greeting: Duration,
    /// Without any packet, an answer is given up during open.
    pub answer_open: Duration,
    /// Without any packet, an answer is given up afterwards.
    pub answer: Duration,
    /// No packet for this long while the stream is expected ends the connection.
    pub stall: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect: Duration::from_secs(3),
            greeting: Duration::from_secs(3),
            answer_open: Duration::from_secs(3),
            answer: Duration::from_secs(4),
            stall: Duration::from_secs(3),
        }
    }
}

/// The own settings of the driver, read from the values of a radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Options {
    pub sample_type: proto::SampleType,
    pub compression: bool,
    pub server_gain: bool,
    pub ppm: f64,
}

impl Options {
    pub fn from_values(v: &Values) -> Options {
        Options {
            sample_type: v.get(&SAMPLE_FORMAT).wire(),
            compression: v.get(&COMPRESSION),
            server_gain: v.get(&SERVER_GAIN),
            ppm: v.get(&PPM),
        }
    }
}

/// Reads an address: spaces, `ip:` and `tcp://` removed, then `host:port` with the default port 5259.
///
/// # Errors
///
/// [`DriverError::Address`] for text that is not an address.
pub fn parse_address(address: &str) -> Result<(String, u16), DriverError> {
    let mut a = address.trim();
    for p in ["tcp://", "ip:"] {
        if a.len() >= p.len() && a[..p.len()].eq_ignore_ascii_case(p) {
            a = a[p.len()..].trim();
        }
    }
    host_port(a, Some(DEFAULT_PORT))
        .map(|(h, p)| (h.to_owned(), p))
        .map_err(|_| DriverError::Address)
}

/// The SDR++ server driver. It keeps, per server address, the rate, the menu and the controls the server takes only while
/// stopped.
#[derive(Clone)]
pub struct SdrppDriver {
    transport: Arc<dyn Transport>,
    servers: Arc<Mutex<HashMap<String, Mem>>>,
    timing: Timing,
}

impl SdrppDriver {
    /// A driver that reaches its servers through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with other timings (tests, slow links).
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        SdrppDriver {
            transport,
            servers: Arc::default(),
            timing,
        }
    }

    fn mem(&self, label: &str) -> Mem {
        lock(&self.servers)
            .entry(label.to_string())
            .or_default()
            .clone()
    }
}

impl SourceDriver for SdrppDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) = parse_address(address)?;
        let label = show(&host, port);
        let mem = self.mem(&label);
        let shortfall = Arc::new(Shortfall::default());
        let at = radio::At {
            transport: self.transport.clone(),
            host,
            port,
            label: label.clone(),
            timing: self.timing,
        };
        let (m2, s2) = (mem.clone(), shortfall.clone());
        let opener: Opener<SdrppRadio> = Arc::new(move |values, abort, notices| {
            SdrppRadio::open(&at, m2.clone(), s2.clone(), values, abort, notices)
        });
        let runner = StreamRunner::open(
            RunnerConfig::new(format!("SDR++ server {label}"), NETWORK_ADVICE),
            opener,
            values,
        )?;
        Ok(Box::new(SdrppSource { runner, shortfall }))
    }
}

/// An open SDR++ server: the shared runner, and the radio-side loss the throughput windows found.
struct SdrppSource {
    runner: StreamRunner<SdrppRadio>,
    shortfall: Arc<Shortfall>,
}

impl Source for SdrppSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.runner.start(sink)
    }
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        self.runner.apply(values)
    }
    fn stop(&mut self) {
        self.runner.stop();
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
    fn settings(&self) -> Vec<SettingDecl> {
        self.runner.settings()
    }
    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }
    fn loss(&self) -> Option<LossReport> {
        let r = self.runner.loss()?;
        let since = lock(&self.shortfall.last).map(|t| t.elapsed());
        Some(merge_loss(
            r,
            self.shortfall.samples.load(Ordering::Relaxed),
            self.shortfall.events.load(Ordering::Relaxed),
            since,
        ))
    }
}

/// Adds the radio-side loss of the throughput windows to the figures of the base without counting the same samples twice.
///
/// The base's detector and the windows both see a link slower than the stream: each finds the samples that never came. So the
/// two counts are two estimates of one loss, and the larger one is kept. Link-down gaps are counted only by the base, apart.
pub(crate) fn merge_loss(
    mut r: LossReport,
    window_samples: u64,
    window_events: u64,
    since_window: Option<Duration>,
) -> LossReport {
    let detected = r.radio_lost.saturating_sub(r.link_down);
    if window_samples > detected {
        r.radio_lost = r.link_down + window_samples;
        r.radio_events = r.radio_events.max(window_events);
        if let Some(since) = since_window {
            r.since_radio_loss = Some(r.since_radio_loss.map_or(since, |x| x.min(since)));
        }
    }
    r
}

#[cfg(test)]
mod tests;
