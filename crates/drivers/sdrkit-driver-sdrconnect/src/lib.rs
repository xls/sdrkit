//! The SDRconnect client driver. SDRconnect is the receiver program of SDRplay; its WebSocket API carries JSON properties and
//! binary streams on one connection. The driver receives the IQ stream of one tuner (the primary or the secondary tuner of an
//! RSPduo in dual-tuner mode) and can set the centre frequency, the sample rate, the LNA state and the antenna.
//!
//! Standard: SDRconnect WebSocket API 1.0.3 (SDRplay).

#![warn(missing_docs)]

pub mod proto;
mod radio;

use proto::Tuner;
use radio::SdrRadio;
use sdrkit_core::net::{host_port, show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::value::ChoiceDecl;
use sdrkit_core::{
    Declare, Field, FieldDecl, Hz, Results, Setting, SettingDecl, SettingId, SettingSet, Value, Values,
};
use std::borrow::Cow;
use sdrkit_driver_core::form;
use sdrkit_driver_core::runner::{Opener, RunnerConfig, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    own_id, Capabilities, DriverDescriptor, DriverError, Link, LossReport, Polled, RateLimits,
    Retuned, RingWriter, Source, SourceDriver, Started, StreamFormat,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The largest WebSocket message accepted (1 MiB, the limit of SDRconnect's stream). A longer message or a declared length
/// above it ends the connection, which is opened again.
pub const MAX_MESSAGE: usize = 1 << 20;
/// The highest centre frequency.
pub const MAX_FREQ: Hz = 2e9;

sdrkit_core::choice_enum! {
    /// Who sets the sample rate ([`SAMPLE_RATE`]).
    pub enum RateSource {
        /// The driver requests the stack's rate from SDRconnect.
        Stack = ("stack", "As the stack asks"),
        /// The driver keeps the rate SDRconnect runs.
        Server = ("server", "Keep the rate of SDRconnect"),
    }
}

/// WebSocket over TLS.
pub const WSS: Setting<field::Bool> = Setting::new("wss", "Secure WebSocket (wss)", "", false)
    .section(form::LINK_SECTION)
    .restart()
    .help(
        "A WebSocket over TLS. SDRconnect Headless 1.0.10 and later offers it. SDRconnect with its window serves plain \
         WebSockets only.",
    );
/// The SHA-256 digest of the certificate of the server, for wss with a self-signed certificate.
pub const TLS_PIN: Setting<field::Text> =
    Setting::new("tls_pin", "Server certificate (SHA-256)", "", "")
        .section(form::LINK_SECTION)
        .restart()
        .help(
            "For wss with a self-signed certificate: the SHA-256 digest of the certificate of the server, 64 hexadecimal \
             digits. Empty uses the trust of the system.",
        );
/// The tuner whose IQ the driver takes.
pub const TUNER: Setting<field::Choice<Tuner>> = Setting::choice("tuner", "Tuner", Tuner::Primary)
    .section(form::ANTENNA_SECTION)
    .restart()
    .help(
        "For an RSPduo in dual-tuner mode in SDRconnect: the tuner whose IQ to take. Other radios have the primary tuner \
         only. The secondary tuner needs SDRconnect 1.0.8 or later.",
    );
/// Who sets the sample rate.
pub const SAMPLE_RATE: Setting<field::Choice<RateSource>> =
    Setting::choice("rate_source", "Rate set by", RateSource::Stack)
        .section(form::RATE_SECTION)
        .help(
        "As the stack asks: sdrkit sets the sample rate of SDRconnect (SDRconnect 1.0.7 and later). Keep: sdrkit uses the rate \
         SDRconnect runs, and resamples.",
    );
/// The LNA state. -1 leaves the state as it is.
pub const LNA: Setting<field::I64> = form::lna_state(form::STAGE_FOLLOWS, 255.0)
    .help(
        "The API of SDRconnect has one gain control, the LNA state. The IF gain and the AGC stay as SDRconnect has them. A \
         state outside the range SDRconnect reports for the radio and its band is refused. -1 leaves the state as it is.",
    );
/// The antenna setting of a radio whose antennas SDRconnect listed: Keep (the antenna SDRconnect has), then the names in
/// the list. The radio adds it when it opens.
pub(crate) fn antenna_setting(list: &[String]) -> SettingDecl {
    let named = list.iter().map(|a| ChoiceDecl {
        key: Cow::Owned(a.clone()),
        label: Cow::Owned(a.clone()),
    });
    let choices: Vec<ChoiceDecl> = std::iter::once(ChoiceDecl::new("", "Keep SDRconnect's"))
        .chain(named)
        .collect();
    let f = form::ANTENNA_FORM;
    SettingDecl::new(FieldDecl::with_choices(f.key, f.label, choices), Value::Choice(0))
        .with_section(f.section)
        .with_help("One of the antennas SDRconnect lists for the radio. Keep leaves the antenna SDRconnect has.")
}

/// The antenna name that `v` chooses, and the id of the antenna setting. `None` for Keep or a radio without the setting.
fn antenna_of(v: &Values) -> (Option<String>, Option<SettingId>) {
    let Ok(id) = v.schema().id(form::ANTENNA_FORM.key) else {
        return (None, None);
    };
    let name = match (v.schema().setting(id), v.value(id)) {
        (Some(d), Some(Value::Choice(i))) => d.field.choices.get(usize::from(*i)).map(|c| c.key.to_string()),
        _ => None,
    };
    (name.filter(|a| !a.is_empty()), Some(id))
}

sdrkit_core::settings! {
    /// The own settings of the SDRconnect driver.
    pub enum SdrconnectSetting {
        /// [`LNA`].
        Lna = LNA,
        /// [`SAMPLE_RATE`].
        SampleRate = SAMPLE_RATE,
        /// [`TUNER`].
        Tuner = TUNER,
        /// [`WSS`].
        Wss = WSS,
        /// [`TLS_PIN`].
        TlsPin = TLS_PIN,
    }
}

/// The signal power SDRconnect reports.
pub const SIGNAL_POWER: Field<field::F64> = Field::new("signal_power_db", "Signal power", "dB");
/// The signal-to-noise ratio SDRconnect reports.
pub const SIGNAL_SNR: Field<field::F64> = Field::new("signal_snr_db", "Signal SNR", "dB");
/// The radio is overloaded.
pub const OVERLOAD: Field<field::Bool> = Field::new("overload", "Overload", "");
/// The radio streams in SDRconnect.
pub const RADIO_STARTED: Field<field::Bool> = Field::new("radio_started", "Radio streaming", "");
/// This client can control the radio.
pub const CAN_CONTROL: Field<field::Bool> = Field::new("can_control", "Control", "");
/// The LNA state SDRconnect has.
pub const LNA_STATE: Field<field::I64> = Field::new("lna_state", "LNA state", "");
/// The lowest LNA state SDRconnect reports for the radio and its band.
pub const LNA_MIN: Field<field::I64> = Field::new("lna_min", "Lowest LNA state", "");
/// The highest LNA state SDRconnect reports for the radio and its band.
pub const LNA_MAX: Field<field::I64> = Field::new("lna_max", "Highest LNA state", "");
/// The antenna in use.
pub const ANTENNA_IN_USE: Field<field::Text> = Field::new("antenna", "Antenna", "");
/// The name of the radio.
pub const DEVICE: Field<field::Text> = Field::new("device", "Radio", "");
/// The version of the WebSocket API.
pub const API_VERSION: Field<field::Text> = Field::new("api_version", "API version", "");
/// The centre frequency SDRconnect has.
pub const SERVER_CENTRE: Field<field::Hz> =
    Field::new("server_centre_hz", "Centre in SDRconnect", "Hz");
/// The sample rate SDRconnect runs.
pub const SERVER_RATE: Field<field::Hz> = Field::new("server_rate_hz", "Rate in SDRconnect", "Hz");
/// The IQ data rate over the last throughput window.
pub const MBIT_S: Field<field::F64> = Field::new("mbit_s", "Link speed", "Mbit/s");
/// Messages the driver does not use.
pub const IGNORED: Field<field::I64> = Field::new("ignored", "Messages ignored", "");
/// Messages that do not follow the API.
pub const MALFORMED: Field<field::I64> = Field::new("malformed", "Messages malformed", "");

/// The readings of an SDRconnect radio.
pub const READINGS: &[&dyn Declare] = &[
    &SIGNAL_POWER,
    &SIGNAL_SNR,
    &OVERLOAD,
    &RADIO_STARTED,
    &CAN_CONTROL,
    &LNA_STATE,
    &LNA_MIN,
    &LNA_MAX,
    &ANTENNA_IN_USE,
    &DEVICE,
    &API_VERSION,
    &SERVER_CENTRE,
    &SERVER_RATE,
    &MBIT_S,
    &IGNORED,
    &MALFORMED,
];

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "sdrconnect",
    name: "SDRplay radio (SDRconnect)",
    opens: sdrkit_driver_core::Opens::Once,
    addresses: &["sdrconnect:<host> (port 5454)", "sdrconnect:<host>:<port>"],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(Some(proto::DEFAULT_PORT)),
    ],
    settings: &[SdrconnectSetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[StreamFormat {
        kind: Kind::Iq,
        sample: SampleType::Cs16,
        min_rate_hz: 62_500.0,
        max_rate_hz: 10e6,
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

/// The errors of SDRconnect that the driver core has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SdrconnectError {
    /// SDRconnect sent no property after the connect: no radio is open in it.
    #[error("SDRconnect sent no property (is a radio open in SDRconnect?)")]
    NoProperties,
    /// SDRconnect did not report its sample rate.
    #[error("SDRconnect did not report its sample rate")]
    NoRate,
    /// The WebSocket API of the server is older than a part of the API the settings need.
    #[error("SDRconnect has WebSocket API {got:?}, the settings need {need:?}")]
    ApiTooOld {
        /// The version of the server.
        got: (u32, u32, u32),
        /// The version needed.
        need: (u32, u32, u32),
    },
}

/// What a server reported about itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Report {
    /// The API version (`api_version`, SDRconnect 1.0.9 and later).
    pub api_version: Option<String>,
    /// The display name of the radio (`active_device`).
    pub device: Option<String>,
    /// The antenna names (`valid_antennas`).
    pub antennas: Option<Vec<String>>,
    /// The LNA range (`lna_state_min`, `lna_state_max`).
    pub lna_range: Option<(u32, u32)>,
}

impl Report {
    /// The API version as numbers.
    pub fn version(&self) -> Option<(u32, u32, u32)> {
        self.api_version.as_deref().and_then(proto::parse_version)
    }
}

/// The settings of the driver, read from the values of a radio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// WebSocket over TLS.
    pub wss: bool,
    /// The pinned certificate digest. `None`: the trust of the system.
    pub pin: Option<[u8; 32]>,
    /// The tuner.
    pub tuner: Tuner,
    /// Who sets the sample rate.
    pub rate: RateSource,
    /// The LNA state to set. `None` leaves it.
    pub lna: Option<u32>,
    /// The antenna to set. `None` leaves it.
    pub antenna: Option<String>,
    /// The id of the antenna setting, for a radio that has one.
    pub antenna_id: Option<SettingId>,
}

impl Options {
    /// Reads the settings from `v`.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for a pin that is not 64 hexadecimal digits.
    pub fn from_values(v: &Values) -> Result<Options, DriverError> {
        let pin = match v.get(&TLS_PIN).trim() {
            "" => None,
            t => Some(
                sdrkit_driver_core::ws::tls::parse_pin(t).ok_or(DriverError::Refused {
                    id: own_id(SdrconnectSetting::TlsPin),
                })?,
            ),
        };
        let lna = u32::try_from(v.get(&LNA)).ok();
        let (antenna, antenna_id) = antenna_of(v);
        Ok(Options {
            wss: v.get(&WSS),
            pin,
            tuner: v.get(&TUNER),
            rate: v.get(&SAMPLE_RATE),
            lna,
            antenna,
            antenna_id,
        })
    }

    /// Makes sure that the LNA state and the antenna are ones the server reported.
    ///
    /// # Errors
    ///
    /// [`DriverError::Refused`] for an LNA state outside the reported range or an antenna the server does not list.
    pub fn check(&self, report: &Report) -> Result<(), DriverError> {
        if let (Some(n), Some((lo, hi))) = (self.lna, report.lna_range) {
            if n < lo || n > hi {
                return Err(DriverError::Refused {
                    id: own_id(SdrconnectSetting::Lna),
                });
            }
        }
        if let (Some(a), Some(list)) = (&self.antenna, &report.antennas) {
            if let (false, Some(id)) = (list.contains(a), self.antenna_id) {
                return Err(DriverError::Refused { id });
            }
        }
        Ok(())
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Throughput {
    /// IQ payload bits per second, in millions.
    pub mbit_s: f64,
}

/// The values of SDRconnect for the tuner in use, as it last reported them.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Props {
    /// The centre frequency.
    pub centre: Option<Hz>,
    /// The sample rate.
    pub rate: Option<Hz>,
    /// The LNA state.
    pub lna: Option<u32>,
    /// The antenna in use.
    pub antenna: Option<String>,
    /// The radio streams.
    pub started: Option<bool>,
    /// This client can control the hardware.
    pub can_control: Option<bool>,
    /// The radio is overloaded.
    pub overload: Option<bool>,
    /// The signal power, dB.
    pub power: Option<f64>,
    /// The signal SNR, dB.
    pub snr: Option<f64>,
}

/// What the driver keeps per server address and tuner.
#[derive(Debug, Default)]
pub(crate) struct Entry {
    pub report: Option<Report>,
    pub props: Props,
    pub last: Option<Throughput>,
    pub ignored: u64,
    pub malformed: u64,
}

pub(crate) type Shared = Arc<Mutex<Entry>>;

/// Locks `m`, also after a panic of another holder.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The capabilities of a server, from what it reported.
pub(crate) fn caps_for(label: &str, e: &Entry, opts: &Options) -> Capabilities {
    let mut name = match e.report.as_ref().and_then(|r| r.device.as_deref()) {
        Some(d) if !d.is_empty() => format!("SDRconnect: {d} at {label}"),
        _ => format!("SDRconnect at {label}"),
    };
    if opts.tuner == Tuner::Secondary {
        name.push_str(" (secondary tuner)");
    }
    let rates = match (opts.rate, e.props.rate) {
        (RateSource::Server, Some(r)) => RateLimits {
            min_hz: r,
            max_hz: r,
            ranges: vec![(r, r)],
            steady_hz: None,
        },
        _ => RateLimits::default(),
    };
    Capabilities {
        name,
        rates,
        min_freq_hz: 0.0,
        max_freq_hz: MAX_FREQ,
        gain_db: None,
        bandwidth_hz: None,
        has_agc: false,
        transmit: None,
        has_bias_tee: false,
        realtime: true,
        common_rates_hz: Vec::new(),
        common_bandwidths_hz: Vec::new(),
        gain_steps_db: Vec::new(),
    }
}

/// Timings, for tests and slow links.
#[derive(Clone, Debug)]
pub struct Timing {
    /// The TCP connect wait (5 s).
    pub connect: Duration,
    /// Each wait of the TLS and WebSocket handshakes, and of the property answers at open (3 s).
    pub handshake: Duration,
    /// The wait for answers to the settings sent at open (1 s).
    pub confirm: Duration,
    /// No message for this long ends the read loop (5 s).
    pub stall: Duration,
    /// The read timeout of the connection, in which reads look for a stop (100 ms).
    pub tick: Duration,
    /// How often the signal readings are requested (1 s).
    pub poll: Duration,
    /// The throughput window (1 s).
    pub window: Duration,
    /// Open attempts at start.
    pub open_attempts: u32,
    /// A fixed masking key for every client frame (tests only).
    pub fixed_mask: Option<[u8; 4]>,
    /// A fixed `Sec-WebSocket-Key` (tests only).
    pub fixed_key: Option<[u8; 16]>,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            connect: Duration::from_secs(5),
            handshake: Duration::from_secs(3),
            confirm: Duration::from_secs(1),
            stall: Duration::from_secs(5),
            tick: Duration::from_millis(100),
            poll: Duration::from_secs(1),
            window: Duration::from_secs(1),
            open_attempts: 4,
            fixed_mask: None,
            fixed_key: None,
        }
    }
}

/// Splits `host` or `host:port` (an IPv6 host in brackets). The port is 5454 when the address names none.
fn parse_address(address: &str) -> Result<(String, u16), DriverError> {
    host_port(address, Some(proto::DEFAULT_PORT))
        .map(|(h, p)| (h.to_owned(), p))
        .map_err(|_| DriverError::Address)
}

/// The SDRconnect driver. It keeps, per server address and tuner, what the server reported.
#[derive(Clone)]
pub struct SdrconnectDriver {
    transport: Arc<dyn Transport>,
    addrs: Arc<Mutex<HashMap<String, Shared>>>,
    timing: Timing,
}

impl SdrconnectDriver {
    /// A driver that reaches its servers through `transport`. For wss, the transport must give TLS connections.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with its own timings.
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        SdrconnectDriver {
            transport,
            addrs: Arc::default(),
            timing,
        }
    }

    /// The entry of one tuner of one server. Two tuners of one RSPduo report different properties, so each has its own.
    fn entry(&self, label: &str, tuner: Tuner) -> Shared {
        lock(&self.addrs)
            .entry(format!("{label} {}", tuner.name()))
            .or_default()
            .clone()
    }
}

impl SourceDriver for SdrconnectDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) = parse_address(address)?;
        let label = show(&host, port);
        let opts = Options::from_values(values)?;
        let entry = self.entry(&label, opts.tuner);
        if let Some(r) = &lock(&entry).report {
            opts.check(r)?;
        }
        let at = radio::At {
            transport: self.transport.clone(),
            host,
            port,
            label: label.clone(),
            timing: self.timing.clone(),
        };
        let e = entry.clone();
        let opener: Opener<SdrRadio> = Arc::new(move |values, abort, notices| {
            SdrRadio::connect(&at, e.clone(), values, abort, notices)
        });
        let mut cfg = RunnerConfig::new(format!("SDRconnect {label}"), NETWORK_ADVICE);
        cfg.open_attempts = self.timing.open_attempts;
        let runner = StreamRunner::open(cfg, opener, values)?;
        Ok(Box::new(SdrconnectSource { runner, opts }))
    }
}

/// An open SDRconnect radio: the shared base, and the settings that take effect only at open.
struct SdrconnectSource {
    runner: StreamRunner<SdrRadio>,
    opts: Options,
}

impl Source for SdrconnectSource {
    fn capabilities(&self) -> &Capabilities {
        self.runner.capabilities()
    }
    fn start(&mut self, sink: RingWriter) -> Result<Started, DriverError> {
        self.runner.start(sink)
    }
    fn apply(&mut self, values: &Values) -> Result<Retuned, DriverError> {
        let o = Options::from_values(values)?;
        if (o.tuner, o.wss, o.pin) != (self.opts.tuner, self.opts.wss, self.opts.pin) {
            return Ok(Retuned::NeedsRestart);
        }
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
    fn loss(&self) -> Option<LossReport> {
        self.runner.loss()
    }
    fn reconnect(&mut self) -> Result<(), DriverError> {
        self.runner.reconnect()
    }
}

#[cfg(test)]
mod tests;
