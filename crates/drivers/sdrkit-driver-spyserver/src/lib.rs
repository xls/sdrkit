//! The SpyServer client driver. A SpyServer shares one radio over TCP. The driver requests the IQ stream only, at the slowest
//! decimated rate of the server that is fast enough, in 8-bit or 16-bit. One client controls the tuner and the gain. The others
//! are read only and get a band inside what the controlling client tunes.
//!
//! Standard: Airspy SpyServer protocol description (`spyserver_protocol.h`), version 2.0.1700.

#![warn(missing_docs)]

pub mod plan;
pub mod proto;
mod radio;

pub use radio::Timing;

use radio::SpyRadio;

use proto::{ClientSync, DeviceInfo};
use radio::{Dropped, Shared};
use sdrkit_core::net::{host_port, show, Transport};
use sdrkit_core::publish::field;
use sdrkit_core::{Declare, Field, Hz, Setting, SettingSet, Values};
use sdrkit_driver_core::runner::{Opener, RunnerConfig, RunnerHooks, StreamRunner, NETWORK_ADVICE};
use sdrkit_driver_core::{
    Capabilities, DriverDescriptor, DriverError, RateLimits, Source, SourceDriver, StreamFormat,
};
use sdrkit_graph::stream::{Kind, SampleType};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

sdrkit_core::choice_enum! {
    /// The sample format requested from the server ([`FORMAT`]).
    pub enum WireFormat {
        /// Unsigned 8-bit samples.
        Bits8 = ("8", "8-bit (low bandwidth)"),
        /// Signed 16-bit samples.
        Bits16 = ("16", "16-bit"),
    }
}

/// The sample format requested from the server.
pub const FORMAT: Setting<field::Choice<WireFormat>> =
    sdrkit_driver_core::form::format(WireFormat::Bits8).restart().help(
        "The sample format the server sends. 8-bit needs the least network (2 bytes a sample). 16-bit keeps weak signals next \
         to strong ones apart better, at twice the bandwidth. A server that forces a format sends that one. The format is \
         fixed while the radio is open.",
    );

sdrkit_core::settings! {
    /// The own settings of the SpyServer driver.
    pub enum SpySetting {
        /// [`FORMAT`].
        Format = FORMAT,
    }
}

/// The radio of the server ("Airspy").
pub const DEVICE: Field<field::Text> = Field::new("device", "Device", "");
/// The serial number of the radio of the server.
pub const SERIAL: Field<field::I64> = Field::new("serial", "Serial number", "");
/// This client controls the tuner and the gain.
pub const IN_CONTROL: Field<field::Bool> = Field::new("in_control", "In control", "");
/// The gain index of the server.
pub const GAIN_INDEX: Field<field::I64> = Field::new("gain_index", "Gain index", "");
/// The digital gain the server applied to the last IQ message.
pub const DIGITAL_GAIN: Field<field::F64> = Field::new("digital_gain_db", "Digital gain", "dB");
/// The IQ data rate over the last throughput window.
pub const MBIT_S: Field<field::F64> = Field::new("mbit_s", "Link speed", "Mbit/s");
/// The sample rate that arrived over the last throughput window.
pub const ARRIVING: Field<field::Hz> = Field::new("arriving_hz", "Arriving rate", "Hz");
/// Less than 95 % of the rate arrives for two windows: the network or the server is the limit.
pub const LOW_THROUGHPUT: Field<field::Bool> = Field::new("low_throughput", "Link too slow", "");
/// Samples of IQ messages that the server dropped, filled with zeros in their place.
pub const DROPPED_SAMPLES: Field<field::I64> =
    Field::new("dropped_samples", "Samples dropped by the server", "");

/// The readings of a SpyServer radio.
pub const READINGS: &[&dyn Declare] = &[
    &DEVICE,
    &SERIAL,
    &IN_CONTROL,
    &GAIN_INDEX,
    &DIGITAL_GAIN,
    &MBIT_S,
    &ARRIVING,
    &LOW_THROUGHPUT,
    &DROPPED_SAMPLES,
];

const fn format(sample: SampleType) -> StreamFormat {
    StreamFormat {
        kind: Kind::Iq,
        sample,
        // The rates are those of the device behind the server.
        min_rate_hz: 0.0,
        max_rate_hz: f64::MAX,
        channels: 1,
    }
}

static DESCRIPTOR: DriverDescriptor = DriverDescriptor {
    key: "spyserver",
    name: "SpyServer",
    opens: sdrkit_driver_core::Opens::Many,
    addresses: &["spyserver:<host>:<port> (the port is required)"],
    address: &[
        sdrkit_driver_core::AddressPart::host(true),
        sdrkit_driver_core::AddressPart::port(None),
    ],
    settings: &[SpySetting::SETTINGS],
    commands: &[],
    readings: READINGS,
    produces: &[
        format(SampleType::Cu8),
        format(SampleType::Cs16),
        format(SampleType::Cf32),
    ],
    accepts: &[],
    tables: &[],
    rasters: &[],
    series: &[],
    points: "",
    kind_commands: &[],
    kind_tables: &[],
};

/// The errors of a SpyServer that the driver core has no variant for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SpyError {
    /// The server answered, but no radio is plugged into it.
    #[error("the SpyServer has no radio connected")]
    NoRadio,
    /// The server offers no sample rate.
    #[error("the SpyServer offers no sample rate")]
    NoRate,
    /// The server forces a sample format that the driver cannot give as a native sample type.
    #[error("the SpyServer forces sample format {format}, which the driver cannot take")]
    WireFormat {
        /// The format number of the server.
        format: u32,
    },
}

/// What a server said about itself, kept per address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ServerInfo {
    /// The DEVICE_INFO (an Airspy R2 for a server not seen yet).
    pub device: DeviceInfo,
    /// The last CLIENT_SYNC.
    pub sync: Option<ClientSync>,
}

impl Default for ServerInfo {
    fn default() -> Self {
        ServerInfo {
            device: DeviceInfo::airspy_r2(),
            sync: None,
        }
    }
}

impl ServerInfo {
    fn tuning_range(&self) -> (Hz, Hz) {
        plan::tuning_range(&self.device, self.sync.as_ref())
    }
}

/// One throughput window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Throughput {
    /// IQ body bits per second, in millions (headers left out).
    pub mbit_s: f64,
    /// Complex samples per second that arrived.
    pub achieved_hz: Hz,
    /// The achieved rate stays below 95 % of the rate of the driver.
    pub low: bool,
}

/// The capabilities of a server.
pub(crate) fn caps_for(label: &str, info: &ServerInfo) -> Capabilities {
    let mut rates = plan::offered_rates(&info.device);
    rates.reverse();
    let (lo, hi) = info.tuning_range();
    Capabilities {
        name: format!("SpyServer {label} (network)"),
        rates: RateLimits {
            min_hz: rates.first().copied().unwrap_or(0.0),
            max_hz: rates.last().copied().unwrap_or(0.0),
            ranges: rates.iter().map(|&r| (r, r)).collect(),
            steady_hz: None,
        },
        min_freq_hz: lo,
        max_freq_hz: hi,
        gain_db: Some((0.0, f64::from(info.device.max_gain_index.max(1)), 1.0)),
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

/// Splits `host:port` (an IPv6 host in brackets). The port is required.
///
/// # Errors
///
/// [`DriverError::Address`] without a port, or for text that is not an address.
pub(crate) fn parse_address(address: &str) -> Result<(String, u16), DriverError> {
    match host_port(address, None) {
        Ok((h, p)) if p != 0 => Ok((h.to_owned(), p)),
        _ => Err(DriverError::Address),
    }
}

/// The SpyServer driver. It keeps, per server address, what the server said and the last throughput.
#[derive(Clone)]
pub struct SpyServerDriver {
    transport: Arc<dyn Transport>,
    addrs: Arc<Mutex<HashMap<String, Shared>>>,
    timing: Timing,
}

impl SpyServerDriver {
    /// A driver that reaches its servers through `transport`.
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        Self::with_timing(transport, Timing::default())
    }

    /// A driver with its own timings, for tests and slow links.
    pub fn with_timing(transport: Arc<dyn Transport>, timing: Timing) -> Self {
        SpyServerDriver {
            transport,
            addrs: Arc::default(),
            timing,
        }
    }

    fn entry(&self, label: &str) -> Shared {
        radio::lock(&self.addrs)
            .entry(label.to_string())
            .or_default()
            .clone()
    }
}

impl SourceDriver for SpyServerDriver {
    fn descriptor(&self) -> &'static DriverDescriptor {
        &DESCRIPTOR
    }

    fn open(&self, address: &str, values: &Values) -> Result<Box<dyn Source>, DriverError> {
        let (host, port) = parse_address(address)?;
        let label = show(&host, port);
        let entry = self.entry(&label);
        let (timing, transport) = (self.timing, self.transport.clone());
        let (l, e) = (label.clone(), entry.clone());
        let fixed = Arc::new(Mutex::new(None));
        let dropped = Arc::new(Dropped::default());
        let d = dropped.clone();
        let opener: Opener<SpyRadio> = Arc::new(move |values, abort, notices| {
            let at = radio::At {
                transport: &transport,
                host: &host,
                port,
                label: &l,
            };
            SpyRadio::open(
                &at,
                e.clone(),
                &fixed,
                d.clone(),
                timing,
                values,
                abort,
                notices,
            )
        });
        let mut cfg = RunnerConfig::new(format!("SpyServer {label}"), NETWORK_ADVICE);
        cfg.hooks = Some(Arc::new(SpyHooks { dropped }));
        Ok(Box::new(StreamRunner::open(cfg, opener, values)?))
    }
}

/// The runner hooks of one SpyServer source: the IQ messages the server dropped count as loss of the radio.
#[derive(Debug)]
struct SpyHooks {
    dropped: Arc<Dropped>,
}

impl RunnerHooks for SpyHooks {
    fn extra_loss(&self) -> (u64, u64) {
        self.dropped.totals()
    }
}

#[cfg(test)]
mod tests;
