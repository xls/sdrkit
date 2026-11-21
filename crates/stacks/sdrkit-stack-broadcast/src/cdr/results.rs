//! What the CDR receiver publishes: the shapes of its result set and table rows, its tables, its series and the
//! `resync` command.

use sdrkit_core::interface::{Fields, Hertz, Table};
use sdrkit_core::publish::{keys, SeriesDef};
use sdrkit_core::Command;

/// The results of the CDR stack. No transmission mode, spectrum or LDPC rate before the system information, no
/// offset or SNR before the first sub-frame, no level before the first report of the decoder, no iterations before the
/// first code word and no network before the network information table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CdrFields<'a> {
    /// Whether the GY/T 268.1 tables are loaded ("loaded"), or what is missing.
    #[field(label = "GY/T 268.1 tables")]
    pub tables: &'a str,
    /// The transmission mode (1 to 3).
    #[field(label = "Transmission mode")]
    pub tx_mode: Option<i64>,
    /// The spectrum mode.
    #[field(label = "Spectrum mode")]
    pub spectrum_mode: Option<i64>,
    /// The spectrum mode in words.
    #[field(label = "Spectrum")]
    pub spectrum: Option<&'a str>,
    /// The LDPC rate of the service data ("3/4").
    #[field(label = "LDPC rate")]
    pub ldpc_rate: Option<&'a str>,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR (Es/N0).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Sub-frames received.
    #[field(label = "Sub-frames")]
    pub subframes: i64,
    /// System information good.
    #[field(label = "SI good")]
    pub si_ok: i64,
    /// System information bad.
    #[field(label = "SI bad")]
    pub si_bad: i64,
    /// LDPC code words good.
    #[field(label = "Code words good")]
    pub cw_ok: i64,
    /// LDPC code words bad.
    #[field(label = "Code words bad")]
    pub cw_bad: i64,
    /// Mean LDPC iterations.
    #[field(label = "LDPC iterations")]
    pub iterations: Option<f64>,
    /// Control multiplex frames good.
    #[field(label = "Control frames good")]
    pub control_ok: i64,
    /// Control multiplex frames bad.
    #[field(label = "Control frames bad")]
    pub control_bad: i64,
    /// Service multiplex frames good.
    #[field(label = "Service frames good")]
    pub service_ok: i64,
    /// Service multiplex frames bad.
    #[field(label = "Service frames bad")]
    pub service_bad: i64,
    /// The network name.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// The country of the network.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The network id.
    #[field(label = "Network id")]
    pub network_id: Option<i64>,
    /// The row key of the service playing in [`SERVICES`].
    #[field(label = "Service playing", refers_to = "services")]
    pub selected_service: Option<&'a str>,
    /// DRA+ units of the service listened to.
    #[field(label = "DRA+ units")]
    pub units_in: i64,
}

/// A service of the multiplex. The row key is the service id in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CdrServiceRow<'a> {
    /// The service id.
    #[field(label = "Service id")]
    pub sid: i64,
    /// Audio units seen.
    #[field(label = "Audio")]
    pub audio: bool,
    /// Data units seen.
    #[field(label = "Data")]
    pub data: bool,
    /// The sample rate of the first audio stream.
    #[field(label = "Sample rate", unit = "Hz")]
    pub sample_rate: Option<i64>,
    /// Channels of the first audio stream.
    #[field(label = "Channels")]
    pub channels: Option<i64>,
    /// The language of the first audio stream.
    #[field(label = "Language")]
    pub language: Option<&'a str>,
    /// The audio algorithm code of the first stream.
    #[field(label = "Algorithm")]
    pub algorithm: Option<i64>,
    /// The share of the multiplex.
    #[field(label = "Bit rate", unit = "kbit/s")]
    pub kbps: f64,
    /// Audio units in the last sub-frame.
    #[field(label = "Units")]
    pub units: i64,
    /// The last data unit, when it is printable text.
    #[field(label = "Text")]
    pub text: Option<&'a str>,
    /// Sub-frame headers good.
    #[field(label = "Good")]
    pub good: i64,
    /// Sub-frame headers or units bad.
    #[field(label = "Bad")]
    pub bad: i64,
}

/// A data unit type of a service. The row key is `<service>/<type>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CdrDataTypeRow<'a> {
    /// The row key of the service.
    #[field(label = "Service", refers_to = "services")]
    pub service: &'a str,
    /// The data unit type.
    #[field(label = "Data type")]
    pub data_type: i64,
}

/// A frequency of the network information table. The row key is its place in the table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CdrFrequencyRow {
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
}

/// The services of the multiplex.
pub static SERVICES: Table<CdrServiceRow<'static>> = Table::new("services", "Services", 32);
/// The data unit types of each service.
pub static SERVICE_DATA_TYPES: Table<CdrDataTypeRow<'static>> =
    Table::new("service_data_types", "Data types", 256);
/// The frequencies of the network information table.
pub static NETWORK_FREQUENCIES: Table<CdrFrequencyRow> =
    Table::new("network_frequencies", "Network frequencies", 64);

/// The SI cells of the last sub-frame.
pub const SERIES_SI: SeriesDef = SeriesDef {
    key: "si_constellation",
    label: "SI constellation",
    x_unit: "",
    y_unit: "",
};
/// The MSDS cells of the last logical frame.
pub const SERIES_MSC: SeriesDef = SeriesDef {
    key: "msc_constellation",
    label: "MSDS constellation",
    x_unit: "",
    y_unit: "",
};
/// The channel magnitude of the last sub-frame: (frequency kHz, dB).
pub const SERIES_CHANNEL: SeriesDef = SeriesDef {
    key: "channel",
    label: "Channel",
    x_unit: "kHz",
    y_unit: "dB",
};

/// Starts the synchronisation again: the `clear` command of the OFDM stage under its own key.
pub const RESYNC: Command =
    Command::new("resync", "Clear").help("Forgets the signal and starts again.");
