//! The interface of the science family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types and what each stack declares.

use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family, Fields, Hertz, Seconds, StackShape, Table, Time,
};
use sdrkit_core::publish::{keys, RasterDef, SeriesDef};
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

// ---- ssdv ----

/// The result fields of SSDV.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SsdvFields {
    /// Packets received.
    #[field(label = "Packets")]
    pub packets_ok: i64,
    /// Packets refused.
    #[field(label = "Packets refused")]
    pub packets_bad: i64,
    /// Packets the RS code corrected.
    #[field(label = "Packets corrected")]
    pub corrected: i64,
    /// Pictures complete.
    #[field(label = "Pictures complete")]
    pub pictures_done: i64,
    /// The speed of the last good packet.
    #[field(label = "Speed", unit = "Bd")]
    pub last_baud: Option<i64>,
    /// The last good packet came as 8N2 bytes.
    #[field(label = "8N2")]
    pub last_n82: Option<bool>,
}

/// An SSDV packet. The row key is the sequence number of the packet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SsdvPacketRow<'a> {
    /// Signal time of a packet.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// Host time of a packet.
    #[field(label = "Host time")]
    pub host_time: Option<Time>,
    /// The call sign.
    #[field(label = "Call sign")]
    pub call: &'a str,
    /// The image id.
    #[field(label = "Image")]
    pub image: i64,
    /// The packet id.
    #[field(label = "Packet")]
    pub packet: i64,
    /// Bytes corrected in a packet.
    #[field(label = "Bytes corrected")]
    pub rs: i64,
    /// The speed of a packet.
    #[field(label = "Speed", unit = "Bd")]
    pub baud: i64,
    /// 8N2 bytes.
    #[field(label = "8N2")]
    pub n82: bool,
    /// Inverted bits.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// A packet without FEC.
    #[field(label = "No FEC")]
    pub no_fec: bool,
    /// The last packet of a picture.
    #[field(label = "Last")]
    pub eoi: bool,
}

/// An SSDV picture. The row key is the call sign and the image id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SsdvPictureRow<'a> {
    /// The call sign.
    #[field(label = "Call sign")]
    pub call: &'a str,
    /// The image id.
    #[field(label = "Image")]
    pub image: i64,
    /// The width of a picture.
    #[field(label = "Width", unit = "px")]
    pub width: i64,
    /// The height of a picture.
    #[field(label = "Height", unit = "px")]
    pub height: i64,
    /// The quality level of a picture.
    #[field(label = "Quality")]
    pub quality: i64,
    /// Packets held of a picture.
    #[field(label = "Packets held")]
    pub held: i64,
    /// Packets missing of a picture.
    #[field(label = "Packets missing")]
    pub missing: i64,
    /// The picture is complete.
    #[field(label = "Complete")]
    pub complete: bool,
    /// The size of the JPEG.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// The SSDV packets, newest first.
pub static SSDV_PACKETS: Table<SsdvPacketRow<'static>> =
    Table::new("packets", "Packets", crate::ssdv::MAX_PACKETS).label(&SsdvPacketRow::CALL);
/// The SSDV pictures, newest first.
pub static SSDV_PICTURES: Table<SsdvPictureRow<'static>> =
    Table::new("pictures", "Pictures", crate::ssdv::MAX_PICTURES).label(&SsdvPictureRow::CALL);

// ---- the event monitors ----

/// An event of a monitor (`schumann`, `whistler`, `jove`, `callisto`, `eme_echo`, `beacon_events`). The row key is the
/// sequence number of the event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScienceEventRow<'a> {
    /// The UTC of an event.
    #[field(label = "Time")]
    pub utc: Time,
    /// The signal time of an event.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The type of an event.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The characteristic frequency of an event.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency_hz: Hertz,
    /// The strength of an event over the background.
    #[field(label = "Peak", unit = "dB")]
    pub peak_db: f64,
    /// The duration of an event.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// Stack-specific value 1.
    #[field(label = "Value 1")]
    pub aux1: f64,
    /// Stack-specific value 2.
    #[field(label = "Value 2")]
    pub aux2: f64,
    /// The note of an event.
    #[field(label = "Note")]
    pub event_note: &'a str,
}

/// An event of a monitor. `text` holds its note.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScienceEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The UTC of an event.
    #[field(label = "Time")]
    pub utc: Time,
    /// The type of an event.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The characteristic frequency of an event.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency_hz: Hertz,
    /// The strength of an event over the background.
    #[field(label = "Peak", unit = "dB")]
    pub peak_db: f64,
    /// The duration of an event.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// Stack-specific value 1.
    #[field(label = "Value 1")]
    pub aux1: f64,
    /// Stack-specific value 2.
    #[field(label = "Value 2")]
    pub aux2: f64,
}

/// The event rows of a monitor, newest first.
pub static SCIENCE_EVENTS: Table<ScienceEventRow<'static>> =
    Table::new("events", "Events", crate::events::MAX_EVENTS);
/// One event of a monitor.
pub static SCIENCE_EVENT: EventKind<ScienceEvent<'static>> =
    EventKind::new("science.event", "Science event");

/// A trend minute of the burst monitors (`jove`, `callisto`). The row key is the minute of the capture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstTrendRow {
    /// The UTC of the end of the minute.
    #[field(label = "Time")]
    pub utc: Time,
    /// The level of a trend minute.
    #[field(label = "Level against the first minute", unit = "dB")]
    pub level_change_db: f64,
    /// Bursts of a trend minute.
    #[field(label = "Bursts per minute")]
    pub bursts_per_min: f64,
}

/// A trend minute of `schumann`. The row key is the minute of the capture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SchumannTrendRow {
    /// The UTC of the end of the minute.
    #[field(label = "Time")]
    pub utc: Time,
    /// The fundamental, Hz.
    #[field(label = "Fundamental", unit = "Hz")]
    pub fundamental_hz: f64,
    /// The height of a resonance over the baseline.
    #[field(label = "Height", unit = "dB")]
    pub height_db: f64,
}

/// A trend minute of `whistler`. The row key is the minute of the capture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WhistlerTrendRow {
    /// The UTC of the end of the minute.
    #[field(label = "Time")]
    pub utc: Time,
    /// Events of a trend minute.
    #[field(label = "Events per minute")]
    pub events_per_min: f64,
    /// Sferics of a trend minute.
    #[field(label = "Sferics per minute")]
    pub sferics_per_min: f64,
}

/// A trend minute of `eme_echo`. The row key is the minute of the capture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EmeEchoTrendRow {
    /// The UTC of the end of the minute.
    #[field(label = "Time")]
    pub utc: Time,
    /// The largest echo level of a trend minute.
    #[field(label = "Largest echo level", unit = "dB")]
    pub echo_level_db: f64,
    /// The mean delay of a trend minute.
    #[field(label = "Mean echo delay", unit = "s")]
    pub echo_delay_s: f64,
}

/// A trend minute of `beacon_events`. The row key is the minute of the capture.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BeaconEventsTrendRow {
    /// The UTC of the end of the minute.
    #[field(label = "Time")]
    pub utc: Time,
    /// The peak S/N of the hit rows of a trend minute.
    #[field(label = "Peak S/N of the hits", unit = "dB")]
    pub peak_snr_db: f64,
    /// The mean width of the hit rows of a trend minute.
    #[field(label = "Mean width of the hits", unit = "Hz")]
    pub mean_width_hz: f64,
}

/// One row a minute of the burst monitors: level change and bursts.
pub static BURST_TREND: Table<BurstTrendRow> =
    Table::new("trend", "Trend per minute", crate::events::TREND_CAP);
/// One row a minute of `schumann`: the fundamental and its height.
pub static SCHUMANN_TREND: Table<SchumannTrendRow> =
    Table::new("trend", "Trend per minute", crate::events::TREND_CAP);
/// One row a minute of `whistler`: events and sferics.
pub static WHISTLER_TREND: Table<WhistlerTrendRow> =
    Table::new("trend", "Trend per minute", crate::events::TREND_CAP);
/// One row a minute of `eme_echo`: the largest echo level and the mean delay of the echoes of pulses in that minute.
pub static EME_ECHO_TREND: Table<EmeEchoTrendRow> =
    Table::new("trend", "Trend per minute", crate::events::TREND_CAP);
/// One row for each minute with hits of `beacon_events`: the peak S/N and the mean width of the hit rows.
pub static BEACON_EVENTS_TREND: Table<BeaconEventsTrendRow> =
    Table::new("trend", "Trend per minute", crate::events::TREND_CAP);

// ---- whistler ----

/// The result fields of `whistler`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WhistlerFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Whistlers found.
    #[field(label = "Whistlers")]
    pub whistlers: i64,
    /// Tweeks found.
    #[field(label = "Tweeks")]
    pub tweeks: i64,
    /// Events in total.
    #[field(label = "Events")]
    pub events_total: i64,
    /// Sferics counted.
    #[field(label = "Sferics")]
    pub sferics: i64,
    /// The dispersion of the last whistler.
    #[field(label = "Last dispersion", unit = "s√Hz")]
    pub dispersion: Option<f64>,
    /// The broadband value of the newest row.
    #[field(label = "Broadband level", unit = "dB")]
    pub broadband_db: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- sferics ----

/// The result fields of `sferics`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SfericsFields<'a> {
    /// Strikes counted.
    #[field(label = "Strikes")]
    pub strikes: i64,
    /// Flashes counted.
    #[field(label = "Flashes")]
    pub flashes: i64,
    /// Impulses of a mains train left out.
    #[field(label = "Mains impulses")]
    pub mains: i64,
    /// Impulses too long to be lightning.
    #[field(label = "Too long")]
    pub too_long: i64,
    /// Strikes in the last minute.
    #[field(label = "Strikes, last minute", unit = "/min")]
    pub per_min_1: f64,
    /// Strikes per minute over 10 minutes.
    #[field(label = "Strikes per minute, 10 minutes", unit = "/min")]
    pub per_min_10: f64,
    /// The floor.
    #[field(label = "Floor", unit = "dBFS")]
    pub floor_db: Option<f64>,
    /// The start level in use.
    #[field(label = "Threshold", unit = "dB")]
    pub threshold_db: f64,
    /// The peak of the newest strike.
    #[field(label = "Last peak", unit = "dB")]
    pub last_peak_db: Option<f64>,
    /// The time since the newest strike.
    #[field(label = "Last strike ago", unit = "s")]
    pub last_ago_s: Option<f64>,
    /// A mains train is being left out.
    #[field(label = "Mains train")]
    pub mains_active: bool,
    /// The storm phase.
    #[field(label = "Storm")]
    pub storm_phase: &'a str,
    /// The change of the median peak.
    #[field(label = "Storm change", unit = "dB")]
    pub storm_db_change: f64,
    /// The activity word of the 10-minute rate.
    #[field(label = "Activity")]
    pub activity: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A strike. The row key is its start, s of signal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SfericsStrikeRow {
    /// The start of a strike.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The start of a strike in UTC.
    #[field(label = "Time")]
    pub utc: Time,
    /// The peak of a strike over the floor.
    #[field(label = "Peak", unit = "dB")]
    pub peak_db: f64,
    /// The width of a strike.
    #[field(label = "Width", unit = "µs")]
    pub width_us: f64,
    /// The flash of a strike.
    #[field(label = "Flash")]
    pub flash: i64,
}

/// The storm phase changed to approaching, overhead or receding.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SfericsStormEvent<'a> {
    /// The storm phase.
    #[field(label = "Storm")]
    pub storm_phase: &'a str,
    /// The change of the median peak.
    #[field(label = "Storm change", unit = "dB")]
    pub storm_db_change: f64,
    /// Strikes per minute in the last 5 minutes.
    #[field(label = "Strikes per minute", unit = "/min")]
    pub per_minute: f64,
}

/// The newest strikes, newest first.
pub static SFERICS_STRIKES: Table<SfericsStrikeRow> =
    Table::new("strikes", "Strikes", crate::sferics::count::TABLE_ROWS);
/// The storm phase changed.
pub static SFERICS_STORM: EventKind<SfericsStormEvent<'static>> =
    EventKind::new("sferics.storm", "Storm phase");
/// The strikes per minute: x the minute (0 the newest), y the strikes.
pub const SFERICS_PER_MINUTE: SeriesDef = SeriesDef {
    key: "per_minute",
    label: "Strikes per minute",
    x_unit: "min",
    y_unit: "strikes",
};

// ---- schumann ----

/// The result fields of `schumann`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SchumannFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The fundamental, Hz.
    #[field(label = "Fundamental", unit = "Hz")]
    pub fundamental_hz: Option<Hertz>,
    /// The height of the fundamental over the baseline.
    #[field(label = "Fundamental height", unit = "dB")]
    pub fundamental_db: Option<f64>,
    /// The Q of the fundamental.
    #[field(label = "Fundamental Q")]
    pub fundamental_q: Option<f64>,
    /// Resonances found.
    #[field(label = "Resonances found")]
    pub peaks_found: i64,
    /// Seconds of averaging.
    #[field(label = "Averaged", unit = "s")]
    pub averaged_s: Seconds,
    /// The level of the last group of the transient band.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// Events in total.
    #[field(label = "Events")]
    pub events_total: i64,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A resonance. The row key is its number, 1 to 5.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SchumannPeakRow {
    /// The resonance of a row of the peak table: 1 to 5.
    #[field(label = "Resonance")]
    pub resonance: i64,
    /// The frequency of a resonance, 0 when none was found.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency_hz: Hertz,
    /// The height of a resonance over the baseline.
    #[field(label = "Height", unit = "dB")]
    pub height_db: f64,
    /// The Q of a resonance.
    #[field(label = "Q")]
    pub q: f64,
}

/// The five lowest resonances.
pub static SCHUMANN_PEAKS: Table<SchumannPeakRow> = Table::new("peaks", "Resonances", 5).sorted(
    &SchumannPeakRow::HEIGHT_DB,
    sdrkit_core::SortOrder::Descending,
);

// ---- jove and callisto ----

/// The result fields of the burst monitors `jove` and `callisto`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Bursts found.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Events in total.
    #[field(label = "Events")]
    pub events_total: i64,
    /// The level of the last row.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// The width of a bin.
    #[field(label = "Bin", unit = "Hz")]
    pub bin_hz: Option<Hertz>,
    /// The time of a row.
    #[field(label = "Row time", unit = "s")]
    pub row_s: Option<Seconds>,
    /// A burst is open.
    #[field(label = "Burst open")]
    pub burst_open: bool,
    /// Files written.
    #[field(label = "Files written")]
    pub files: i64,
    /// The last file written, empty before the first.
    #[field(label = "Last file")]
    pub last_file: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- eme_echo ----

/// The result fields of `eme_echo`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EmeEchoFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Own pulses seen.
    #[field(label = "Own pulses")]
    pub pulses: i64,
    /// Echoes found.
    #[field(label = "Echoes")]
    pub echoes: i64,
    /// The own carrier.
    #[field(label = "Own carrier offset", unit = "Hz")]
    pub carrier_hz: Option<Hertz>,
    /// Transmitting now.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The noise per bin.
    #[field(label = "Noise per bin", unit = "dBFS")]
    pub noise_db: Option<f64>,
    /// The delay of the last echo.
    #[field(label = "Last echo delay", unit = "s")]
    pub last_delay_s: Option<Seconds>,
    /// The Doppler shift of the last echo.
    #[field(label = "Last echo Doppler", unit = "Hz")]
    pub last_doppler_hz: Option<Hertz>,
    /// The level of the last echo.
    #[field(label = "Last echo level", unit = "dB")]
    pub last_level_db: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- beacon_events ----

/// The result fields of `beacon_events`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BeaconEventsFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Openings found.
    #[field(label = "Openings")]
    pub openings: i64,
    /// The noise per bin.
    #[field(label = "Noise per bin", unit = "dBFS")]
    pub noise_db: Option<f64>,
    /// An opening is open.
    #[field(label = "Open")]
    pub open: bool,
    /// The width of the last row.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Option<Hertz>,
    /// The best z of the last row.
    #[field(label = "Scan z")]
    pub z: Option<f64>,
    /// The S/N of the last row.
    #[field(label = "S/N", unit = "dB")]
    pub snr_db: Option<f64>,
    /// The shift of the last row.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<Hertz>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- riometer ----

/// The result fields of `riometer`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RiometerFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The power of the last minute, dB of full scale.
    #[field(label = "Power", unit = "dBFS")]
    pub power_db: Option<f64>,
    /// The local sidereal time.
    #[field(label = "Local sidereal time", unit = "h")]
    pub lst_h: Option<f64>,
    /// The curve at that time.
    #[field(label = "Quiet-day curve here", unit = "dBFS")]
    pub curve_db: Option<f64>,
    /// The absorption.
    #[field(label = "Absorption", unit = "dB")]
    pub absorption_db: Option<f64>,
    /// Bins of the curve with a value.
    #[field(label = "Curve bins filled (of 288)")]
    pub curve_bins: i64,
    /// The curve is ready.
    #[field(label = "Curve ready")]
    pub curve_ready: bool,
    /// The interfered share.
    #[field(label = "Interfered share")]
    pub interfered_share: f64,
    /// An event is open.
    #[field(label = "Absorption event")]
    pub event_on: bool,
    /// Minutes measured.
    #[field(label = "Minutes measured")]
    pub minutes: i64,
    /// The curve as text, as of the last minute measured or the last change of the curve.
    #[field(label = "Quiet-day curve")]
    pub qdc: Option<&'a str>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The start (`phase` `start`, `depth_db` the absorption then) or the end (`end`, the largest absorption, the duration) of an
/// absorption event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RiometerAbsorptionEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Start or end of an event.
    #[field(label = "Phase")]
    pub phase: &'a str,
    /// The depth of an event.
    #[field(label = "Depth", unit = "dB")]
    pub depth_db: f64,
    /// The time from the start of an event.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
}

/// The reply of `export_qdc`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RiometerQdcReply<'a> {
    /// The curve as text.
    #[field(label = "Quiet-day curve")]
    pub qdc: &'a str,
}

/// The arguments of `import_qdc`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct RiometerQdcArgs<'a> {
    /// The curve text.
    #[arg(label = "Curve", help = "qdc1 and 288 values.")]
    pub text: &'a str,
}

/// An absorption event started or ended.
pub static RIOMETER_ABSORPTION: EventKind<RiometerAbsorptionEvent<'static>> =
    EventKind::new("riometer.absorption", "Absorption event");
/// Replies with the curve as text.
pub const RIOMETER_EXPORT_QDC: CommandKind<(), RiometerQdcReply<'static>> =
    CommandKind::new("export_qdc", "Export the quiet-day curve")
        .help("Replies with the curve as text (qdc1 and 288 values).");
/// Takes a curve as text; a bad text is refused and changes nothing.
pub const RIOMETER_IMPORT_QDC: CommandKind<RiometerQdcArgs<'static>> =
    CommandKind::new("import_qdc", "Import a quiet-day curve")
        .help("Takes a curve as text; a bad text is refused and changes nothing.");

// ---- radar_detect ----

/// The result fields of `radar_detect`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RadarDetectFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The rotation period.
    #[field(label = "Rotation", unit = "s")]
    pub rotation_s: Option<f64>,
    /// Rotations per minute.
    #[field(label = "Rotations per minute", unit = "rpm")]
    pub rpm: Option<f64>,
    /// The repetition frequency of the fullest bin.
    #[field(label = "PRF", unit = "Hz")]
    pub prf_hz: Option<Hertz>,
    /// The repetition text.
    #[field(label = "PRF text")]
    pub prf: Option<&'a str>,
    /// The median pulse width.
    #[field(label = "Pulse width", unit = "µs")]
    pub pulse_width_us: Option<f64>,
    /// The beamwidth.
    #[field(label = "Beamwidth", unit = "°")]
    pub beamwidth_deg: Option<f64>,
    /// Crossings found.
    #[field(label = "Crossings")]
    pub crossings: i64,
    /// Pulses found.
    #[field(label = "Pulses")]
    pub pulses: i64,
    /// Rows written.
    #[field(label = "Rows")]
    pub rows_total: i64,
    /// The noise power.
    #[field(label = "Noise", unit = "dBFS")]
    pub noise_dbfs: Option<f64>,
    /// Weather: sweeps in this volume.
    #[field(label = "Sweeps in the volume")]
    pub sweeps: Option<i64>,
    /// Weather: volume starts seen.
    #[field(label = "Volumes")]
    pub volumes: Option<i64>,
    /// Weather: the length of the last whole volume.
    #[field(label = "Last volume", unit = "s")]
    pub last_volume_s: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of `radar_detect`: a rotation (marine) or a sweep (weather). The row key is the sequence number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RadarDetectRow<'a> {
    /// The signal time of a row.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The kind of a row: `rotation` or `sweep`.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The rotation period.
    #[field(label = "Rotation", unit = "s")]
    pub rotation_s: Option<f64>,
    /// Rotations per minute.
    #[field(label = "Rotations per minute", unit = "rpm")]
    pub rpm: Option<f64>,
    /// The repetition text.
    #[field(label = "PRF text")]
    pub prf: &'a str,
    /// The median pulse width.
    #[field(label = "Pulse width", unit = "µs")]
    pub pulse_width_us: Option<f64>,
    /// The beamwidth.
    #[field(label = "Beamwidth", unit = "°")]
    pub beamwidth_deg: Option<f64>,
    /// The peak of the crossing of a row.
    #[field(label = "Peak", unit = "dB")]
    pub peak_db: f64,
    /// The volume of a weather row.
    #[field(label = "Volume")]
    pub volume: Option<i64>,
    /// The sweep of a weather row in its volume.
    #[field(label = "Sweep")]
    pub sweep: Option<i64>,
}

/// One row of `radar_detect`. `text` holds a summary.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RadarDetectRowEvent<'a> {
    /// The text of a decoded message: the summary of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of a row: `rotation` or `sweep`.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The rotation period.
    #[field(label = "Rotation", unit = "s")]
    pub rotation_s: Option<f64>,
    /// Rotations per minute.
    #[field(label = "Rotations per minute", unit = "rpm")]
    pub rpm: Option<f64>,
    /// The repetition text.
    #[field(label = "PRF text")]
    pub prf: &'a str,
    /// The median pulse width.
    #[field(label = "Pulse width", unit = "µs")]
    pub pulse_width_us: Option<f64>,
    /// The beamwidth.
    #[field(label = "Beamwidth", unit = "°")]
    pub beamwidth_deg: Option<f64>,
    /// The peak of the crossing of a row.
    #[field(label = "Peak", unit = "dB")]
    pub peak_db: f64,
    /// The volume of a weather row.
    #[field(label = "Volume")]
    pub volume: Option<i64>,
    /// The sweep of a weather row in its volume.
    #[field(label = "Sweep")]
    pub sweep: Option<i64>,
}

/// The rows of `radar_detect`, newest first.
pub static RADAR_DETECT_ROWS: Table<RadarDetectRow<'static>> =
    Table::new("radar_rows", "Rows", crate::radar::ROWS_KEPT);
/// One row: a rotation (marine) or a sweep (weather).
pub static RADAR_ROW: EventKind<RadarDetectRowEvent<'static>> =
    EventKind::new("radar.row", "Radar row");

// ---- pulsar ----

/// The result fields of `pulsar`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PulsarFields<'a> {
    /// The text of a decoded message: the summary of the stack.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Whether a detection was made.
    #[field(label = "Detected")]
    pub ok: bool,
    /// The best S/N.
    #[field(label = "S/N")]
    pub snr: Option<f64>,
    /// The best period.
    #[field(label = "Period found", unit = "s")]
    pub period_found_s: Option<f64>,
    /// The best DM.
    #[field(label = "DM found", unit = "pc cm⁻³")]
    pub dm_found: Option<f64>,
    /// The pulse phase.
    #[field(label = "Peak phase")]
    pub peak_phase: Option<f64>,
    /// The duty cycle.
    #[field(label = "Duty cycle")]
    pub duty: Option<f64>,
    /// The time observed.
    #[field(label = "Time observed", unit = "s")]
    pub observed_s: Seconds,
    /// A search runs.
    #[field(label = "Searching")]
    pub searching: bool,
    /// Searches made.
    #[field(label = "Searches")]
    pub searches: i64,
    /// Detections made.
    #[field(label = "Detections")]
    pub detections: i64,
    /// Rows held.
    #[field(label = "Rows held")]
    pub rows_held: i64,
    /// The time of a row.
    #[field(label = "Row time", unit = "ms")]
    pub row_ms: Option<f64>,
    /// Channels of the filter bank.
    #[field(label = "Channels")]
    pub bank_channels: Option<i64>,
    /// The area needed.
    #[field(label = "Effective area needed", unit = "m²")]
    pub area_m2: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A search of `pulsar`. The row key is the sequence number of the search.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PulsarSearchRow {
    /// The time observed.
    #[field(label = "Time observed", unit = "s")]
    pub observed_s: Seconds,
    /// Whether a detection was made.
    #[field(label = "Detected")]
    pub ok: bool,
    /// The best S/N.
    #[field(label = "S/N")]
    pub snr: Option<f64>,
    /// The best period.
    #[field(label = "Period found", unit = "s")]
    pub period_found_s: f64,
    /// The best DM.
    #[field(label = "DM found", unit = "pc cm⁻³")]
    pub dm_found: f64,
    /// The pulse phase.
    #[field(label = "Peak phase")]
    pub peak_phase: f64,
    /// The duty cycle.
    #[field(label = "Duty cycle")]
    pub duty: f64,
}

/// A search that detected the pulsar.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PulsarDetectionEvent<'a> {
    /// The text of a decoded message: the summary of the detection.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Whether a detection was made.
    #[field(label = "Detected")]
    pub ok: bool,
    /// The best S/N.
    #[field(label = "S/N")]
    pub snr: f64,
    /// The best period.
    #[field(label = "Period found", unit = "s")]
    pub period_found_s: f64,
    /// The best DM.
    #[field(label = "DM found", unit = "pc cm⁻³")]
    pub dm_found: f64,
    /// The pulse phase.
    #[field(label = "Peak phase")]
    pub peak_phase: f64,
    /// The duty cycle.
    #[field(label = "Duty cycle")]
    pub duty: f64,
    /// The time observed.
    #[field(label = "Time observed", unit = "s")]
    pub observed_s: Seconds,
}

/// One row a search, newest first.
pub static PULSAR_SEARCHES: Table<PulsarSearchRow> =
    Table::new("searches", "Searches", crate::pulsar::MAX_SEARCHES);
/// A search that detected the pulsar.
pub static PULSAR_DETECTION: EventKind<PulsarDetectionEvent<'static>> =
    EventKind::new("pulsar.detection", "Pulsar detection");

// ---- mscatter ----

/// The result fields of `mscatter`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MscatterFields {
    /// Pings counted.
    #[field(label = "Pings")]
    pub pings: i64,
    /// Pings in the last 60 minutes.
    #[field(label = "Pings in the last hour")]
    pub rate: i64,
    /// Broadband bursts refused.
    #[field(label = "Bursts refused")]
    pub rejected: i64,
    /// The beacon's offset from the centre.
    #[field(label = "Beacon offset", unit = "Hz")]
    pub beacon: Option<Hertz>,
    /// The beacon level over the noise in one bin.
    #[field(label = "Beacon SNR", unit = "dB")]
    pub beacon_snr_db: f64,
    /// Noise floor of the channel, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: f64,
    /// The detection level.
    #[field(label = "Detection level", unit = "dB")]
    pub threshold_db: f64,
    /// The level of the ping on the air now.
    #[field(label = "Ping now", unit = "dB")]
    pub ping_now_db: Option<f64>,
}

/// A meteor ping. The row key is the sequence number of the ping.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MscatterPingRow {
    /// The number of a ping.
    #[field(label = "Number")]
    pub id: i64,
    /// UTC of a ping.
    #[field(label = "Time")]
    pub utc: Time,
    /// The length of a ping.
    #[field(label = "Length", unit = "s")]
    pub duration: Seconds,
    /// The peak level over the background.
    #[field(label = "Peak", unit = "dB")]
    pub peak_snr_db: f64,
    /// The Doppler shift at the peak.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Hertz,
    /// The drift of the Doppler shift.
    #[field(label = "Drift", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// Underdense or overdense.
    #[field(label = "Kind")]
    pub kind: crate::mscatter::PingKind,
}

/// A meteor shower. The row key is its name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MscatterShowerRow<'a> {
    /// The name of a shower.
    #[field(label = "Shower")]
    pub name: &'a str,
    /// Days to the peak of a shower (below 0 after it).
    #[field(label = "Peak in", unit = "d")]
    pub peak_in_days: i64,
    /// The shower is active.
    #[field(label = "Active")]
    pub active: bool,
    /// The zenithal hourly rate of a shower.
    #[field(label = "ZHR")]
    pub zhr: i64,
}

/// A meteor ping. `text` holds the ping for people.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MscatterPingEvent<'a> {
    /// The text of a decoded message: the ping for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// UTC of a ping.
    #[field(label = "Time")]
    pub utc: Time,
    /// The length of a ping.
    #[field(label = "Length", unit = "s")]
    pub duration: Seconds,
    /// The peak level over the background.
    #[field(label = "Peak", unit = "dB")]
    pub peak_snr_db: f64,
    /// The Doppler shift at the peak.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Hertz,
    /// Underdense or overdense.
    #[field(label = "Kind")]
    pub kind: crate::mscatter::PingKind,
}

/// The pings, newest first.
pub static MSCATTER_PINGS: Table<MscatterPingRow> =
    Table::new("pings", "Pings", crate::mscatter::MAX_PINGS);
/// The showers active or peaking within 45 days, by name.
pub static MSCATTER_SHOWERS: Table<MscatterShowerRow<'static>> =
    Table::new("showers", "Showers", 64).label(&MscatterShowerRow::NAME);
/// A meteor ping.
pub static MS_PING: EventKind<MscatterPingEvent<'static>> =
    EventKind::new("ms.ping", "Meteor ping");
/// The pings per hour: x the hour, y the pings.
pub const MSCATTER_HOURLY: SeriesDef = SeriesDef {
    key: "hourly",
    label: "Pings per hour",
    x_unit: "h",
    y_unit: "",
};
/// The pings per day: x the day, y the pings.
pub const MSCATTER_DAILY: SeriesDef = SeriesDef {
    key: "daily",
    label: "Pings per day",
    x_unit: "d",
    y_unit: "",
};
/// The spectrum around the beacon: x the bin, y dB.
pub const MSCATTER_AROUND: SeriesDef = SeriesDef {
    key: "beacon_spectrum",
    label: "Around the beacon",
    x_unit: "bin",
    y_unit: "dB",
};

// ---- time ----

/// The result fields of `time`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TimeFields<'a> {
    /// The station decoded.
    #[field(label = "Station")]
    pub station: crate::time::codes::Station,
    /// WWVH (1200 Hz ticks) rather than WWV (1000 Hz ticks).
    #[field(label = "WWVH")]
    pub wwvh: Option<bool>,
    /// The carrier level.
    #[field(label = "Carrier", unit = "dBFS")]
    pub carrier_dbfs: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The keying depth (LF) or the tick level over the background (HF).
    #[field(label = "Keying depth", unit = "%")]
    pub depth_pct: f64,
    /// Where the second's edge falls, after a whole second of signal time.
    #[field(label = "Second edge", unit = "ms")]
    pub tick_ms: f64,
    /// The jitter of the edges.
    #[field(label = "Edge jitter", unit = "ms")]
    pub jitter_ms: f64,
    /// Minutes decoded.
    #[field(label = "Minutes decoded")]
    pub minutes_ok: i64,
    /// Minutes refused.
    #[field(label = "Minutes refused")]
    pub minutes_bad: i64,
    /// Two minutes in a row agree.
    #[field(label = "Confirmed")]
    pub confirmed: bool,
    /// The distance from the station.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: Option<f64>,
    /// The path delay.
    #[field(label = "Path delay", unit = "ms")]
    pub delay_ms: Option<f64>,
    /// How the path delay was found.
    #[field(label = "Path")]
    pub path: Option<crate::time::PathKind>,
    /// The symbols of this minute so far.
    #[field(label = "Symbols")]
    pub symbols: &'a str,
    /// The received UTC now.
    #[field(label = "Received time")]
    pub utc: Option<Time>,
    /// The last minute as the station sends it.
    #[field(label = "Received")]
    pub received: Option<&'a str>,
    /// DUT1 (UT1 − UTC).
    #[field(label = "DUT1", unit = "s")]
    pub dut1_s: Option<f64>,
    /// Summer time is in force.
    #[field(label = "Summer time")]
    pub summer: Option<bool>,
    /// The day of the week sent, by name.
    #[field(label = "Weekday")]
    pub weekday: Option<crate::time::Day>,
    /// The computer clock minus the received time, now.
    #[field(label = "Clock offset", unit = "s")]
    pub clock_offset_s: Option<f64>,
    /// The same, averaged over the last minute.
    #[field(label = "Clock offset (1 min)", unit = "s")]
    pub clock_offset_avg_s: Option<f64>,
}

/// A minute of `time`. The row key is the sequence number of the minute.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TimeMinuteRow<'a> {
    /// The signal time of a minute.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// The host time of a minute.
    #[field(label = "Host time")]
    pub host_time: Option<Time>,
    /// The decoded flag of a minute.
    #[field(label = "Decoded")]
    pub ok: bool,
    /// The text of a minute.
    #[field(label = "Time")]
    pub text: Option<&'a str>,
    /// Why a minute was refused.
    #[field(label = "Refused")]
    pub refusal: Option<crate::time::codes::Refusal>,
    /// The second where the decoder stopped.
    #[field(label = "Second")]
    pub second: Option<i64>,
    /// The symbols of this minute so far.
    #[field(label = "Symbols")]
    pub symbols: &'a str,
}

/// A minute decoded: `text` holds the time as the station sends it, `utc` the start of the minute.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TimeMinuteEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The received UTC now.
    #[field(label = "Received time")]
    pub utc: Time,
    /// The station decoded.
    #[field(label = "Station")]
    pub station: crate::time::codes::Station,
}

/// The minutes, newest first.
pub static TIME_MINUTES: Table<TimeMinuteRow<'static>> =
    Table::new("minutes", "Minutes", crate::time::MAX_MINUTES);
/// A minute decoded.
pub static TIME_MINUTE: EventKind<TimeMinuteEvent<'static>> =
    EventKind::new("time.minute", "Time signal minute");

/// The series of the history of `time`, in the order of the history: x seconds of signal.
pub const TIME_SERIES: [SeriesDef; 4] = [
    SeriesDef {
        key: "level",
        label: "Carrier level",
        x_unit: "s",
        y_unit: "dBFS",
    },
    SeriesDef {
        key: "snr",
        label: "SNR",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "tick",
        label: "Second edge",
        x_unit: "s",
        y_unit: "ms",
    },
    SeriesDef {
        key: "offset",
        label: "Clock offset",
        x_unit: "s",
        y_unit: "s",
    },
];

// ---- eczas ----

/// The result fields of `eczas`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EczasFields {
    /// The received UTC now.
    #[field(label = "Received time")]
    pub utc: Option<Time>,
    /// The hours that the packet adds to UTC for the local time in Poland.
    #[field(label = "Local offset", unit = "h")]
    pub tz_hours: Option<i64>,
    /// A leap second is announced.
    #[field(label = "Leap second announced")]
    pub leap_announce: Option<bool>,
    /// The announced leap second is taken away.
    #[field(label = "Leap second taken away")]
    pub leap_subtract: Option<bool>,
    /// A change of summer time is near.
    #[field(label = "Summer time change soon")]
    pub dst_change_soon: Option<bool>,
    /// The transmitter state.
    #[field(label = "Transmitter")]
    pub tx_state: Option<crate::eczas::TxState>,
    /// The UTC of the last time packet.
    #[field(label = "Last packet")]
    pub last_packet: Option<Time>,
    /// Two packets in a row agree.
    #[field(label = "Confirmed")]
    pub confirmed: bool,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The quality of the last sync.
    #[field(label = "Sync quality")]
    pub sync_quality: f64,
    /// Syncs found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Frames cut.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Packets of the other slots.
    #[field(label = "Other slots")]
    pub other_slots: i64,
    /// Syncs not followed by the sync word.
    #[field(label = "False syncs")]
    pub false_syncs: i64,
    /// The distance from the transmitter.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: Option<f64>,
    /// The path delay.
    #[field(label = "Path delay", unit = "ms")]
    pub delay_ms: Option<f64>,
    /// The computer clock minus the received time, now.
    #[field(label = "Clock offset", unit = "s")]
    pub clock_offset_s: Option<f64>,
    /// The same, averaged over the last 10 packets.
    #[field(label = "Clock offset (10 packets)", unit = "s")]
    pub clock_offset_avg_s: Option<f64>,
}

/// A frame of `eczas`. The row key is the sequence number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EczasFrameRow<'a> {
    /// The signal time of a frame.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The host time of a frame.
    #[field(label = "Host time")]
    pub host_time: Option<Time>,
    /// What a frame is.
    #[field(label = "Kind")]
    pub kind: crate::eczas::FrameKind,
    /// The frame was accepted.
    #[field(label = "Accepted")]
    pub ok: bool,
    /// The text of a frame.
    #[field(label = "Text")]
    pub text: &'a str,
    /// The bytes of a frame.
    #[field(label = "Bytes")]
    pub bytes: Arc<[u8]>,
}

/// A time packet received: `text` holds the time for people, `utc` the time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EczasTimeEvent<'a> {
    /// The text of a frame.
    #[field(label = "Text")]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The received UTC now.
    #[field(label = "Received time")]
    pub utc: Time,
    /// The hours that the packet adds to UTC for the local time in Poland.
    #[field(label = "Local offset", unit = "h")]
    pub tz_hours: i64,
    /// The transmitter state.
    #[field(label = "Transmitter")]
    pub tx_state: crate::eczas::TxState,
}

/// The frame log, newest first.
pub static ECZAS_FRAMES: Table<EczasFrameRow<'static>> =
    Table::new("frames", "Frames", crate::eczas::MAX_FRAMES);
/// A time packet received.
pub static ECZAS_TIME: EventKind<EczasTimeEvent<'static>> =
    EventKind::new("eczas.time", "e-Czas time");

/// The series of `eczas`: the carrier level, the C/N, the clock offsets, the phase and the slicer trace.
pub const ECZAS_SERIES: [SeriesDef; 5] = [
    SeriesDef {
        key: "level",
        label: "Carrier level",
        x_unit: "s",
        y_unit: "dBFS",
    },
    SeriesDef {
        key: "snr",
        label: "C/N in 100 Hz",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "clock_offsets",
        label: "Clock offset",
        x_unit: "s",
        y_unit: "s",
    },
    SeriesDef {
        key: "phase",
        label: "Phase",
        x_unit: "",
        y_unit: "",
    },
    SeriesDef {
        key: "trace",
        label: "Slicer",
        x_unit: "s",
        y_unit: "",
    },
];

// ---- same ----

/// The result fields of `same`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameFields {
    /// The channel.
    #[field(label = "Channel")]
    pub channel: crate::same::WeatherChannel,
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// Messages decoded.
    #[field(label = "Messages")]
    pub messages_ok: i64,
    /// Messages damaged.
    #[field(label = "Damaged messages")]
    pub messages_bad: i64,
    /// An attention tone is on the air now.
    #[field(label = "Alert tone now")]
    pub tone_now: bool,
    /// The country of the location names.
    #[field(label = "Location names")]
    pub country: crate::same::parse::Country,
    /// Counties in the loaded table.
    #[field(label = "Counties loaded")]
    pub counties: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier error.
    #[field(label = "Carrier error", unit = "Hz")]
    pub carrier: Hertz,
}

/// A SAME message. The row key is the count of rows added before it, as six digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameAlertRow<'a> {
    /// The signal time of a row.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The host time of a row.
    #[field(label = "Host time")]
    pub host_time: Option<Time>,
    /// What the row is.
    #[field(label = "Kind")]
    pub kind: crate::same::MessageKind,
    /// The event code.
    #[field(label = "Event")]
    pub event: Option<&'a str>,
    /// The event name.
    #[field(label = "Event name")]
    pub event_name: Option<&'a str>,
    /// The kind of event (warning, watch ...).
    #[field(label = "Event kind")]
    pub event_kind: Option<crate::same::parse::EventKind>,
    /// The originator code.
    #[field(label = "Originator")]
    pub originator: Option<&'a str>,
    /// The originator's name.
    #[field(label = "Originator name")]
    pub originator_name: Option<&'a str>,
    /// The number of locations.
    #[field(label = "Locations")]
    pub locations: i64,
    /// How long the message is valid.
    #[field(label = "Valid for", unit = "s")]
    pub valid_for: Option<Seconds>,
    /// When the message was issued.
    #[field(label = "Issued")]
    pub issued: Option<Time>,
    /// The sender's call sign.
    #[field(label = "Call sign")]
    pub callsign: Option<&'a str>,
    /// The repeats heard.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// The repeats identical to the result.
    #[field(label = "Identical repeats")]
    pub agreeing: i64,
    /// Bits the vote changed.
    #[field(label = "Bits changed")]
    pub bits_changed: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier error.
    #[field(label = "Carrier error", unit = "Hz")]
    pub carrier: Hertz,
    /// The attention tone that followed.
    #[field(label = "Alert tone", unit = "s")]
    pub tone: Option<Seconds>,
    /// The raw text.
    #[field(label = "Text")]
    pub raw: &'a str,
    /// Why the message is damaged.
    #[field(label = "Error")]
    pub error: Option<crate::same::parse::Damage>,
}

/// A location of a SAME message. The row key is `<alert>/<n>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameAlertLocationRow<'a> {
    /// The alert a location belongs to.
    #[field(label = "Alert", refers_to = "alerts")]
    pub alert: &'a str,
    /// A location code (PSSCCC).
    #[field(label = "Code")]
    pub code: &'a str,
    /// The location as people read it.
    #[field(label = "Place")]
    pub place: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// An attention tone. The row key is the sequence number of the tone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameToneRow {
    /// The signal time of a row.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The length of a tone.
    #[field(label = "Length", unit = "s")]
    pub duration: Seconds,
    /// The level of a tone.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A SAME alert. `source` is the sender's call sign, `text` the event and the places, `position` the first place on the map.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameAlertEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: keys::Urgency,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The event code.
    #[field(label = "Event")]
    pub event: &'a str,
    /// The event name.
    #[field(label = "Event name")]
    pub event_name: Option<&'a str>,
    /// The kind of event (warning, watch ...).
    #[field(label = "Event kind")]
    pub event_kind: Option<crate::same::parse::EventKind>,
    /// The originator code.
    #[field(label = "Originator")]
    pub originator: &'a str,
    /// How long the message is valid.
    #[field(label = "Valid for", unit = "s")]
    pub valid_for: Seconds,
    /// The sender's call sign.
    #[field(label = "Call sign")]
    pub callsign: &'a str,
    /// The number of locations.
    #[field(label = "Locations")]
    pub locations: i64,
    /// The raw text.
    #[field(label = "Text")]
    pub raw: &'a str,
}

/// A SAME end of message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SameEndEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The raw text.
    #[field(label = "Text")]
    pub raw: &'a str,
}

/// Messages.
pub static SAME_ALERTS: Table<SameAlertRow<'static>> =
    Table::new("alerts", "Messages", crate::same::MAX_ALERTS).label(&SameAlertRow::CALLSIGN);
/// Locations.
pub static SAME_ALERT_LOCATIONS: Table<SameAlertLocationRow<'static>> = Table::new(
    "alert_locations",
    "Locations",
    crate::same::MAX_ALERTS * crate::same::MAX_LOCATIONS,
);
/// Alert tones.
pub static SAME_TONES: Table<SameToneRow> =
    Table::new("tones", "Alert tones", crate::same::MAX_TONES);
/// SAME alert.
pub static SAME_ALERT: EventKind<SameAlertEvent<'static>> =
    EventKind::new("same.alert", "SAME alert");
/// SAME end of message.
pub static SAME_END: EventKind<SameEndEvent<'static>> =
    EventKind::new("same.end", "SAME end of message");

// ---- sonde ----

/// The result fields of `sonde`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SondeFields<'a> {
    /// The number of sondes in the table.
    #[field(label = "Sondes")]
    pub sondes: i64,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Carrier searches so far.
    #[field(label = "Searches")]
    pub searches: i64,
    /// Channels in use.
    #[field(label = "Channels used")]
    pub channels_used: i64,
    /// Channels allowed.
    #[field(label = "Channels allowed")]
    pub channels_allowed: i64,
    /// The band searched (the input rate).
    #[field(label = "Band searched", unit = "Hz")]
    pub band: Hertz,
    /// The capture is narrower than 2 MHz: only ±0.45 of the rate is searched.
    #[field(label = "Narrow capture")]
    pub narrow: bool,
    /// The row key of the sonde the plots show.
    #[field(label = "Selected sonde", refers_to = "sondes")]
    pub selected: Option<&'a str>,
}

/// A row of the table `sondes`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SondeRow<'a> {
    /// The family.
    #[field(label = "Type")]
    pub r#type: crate::sonde::frame::SondeType,
    /// The subtype.
    #[field(label = "Subtype")]
    pub subtype: Option<&'a str>,
    /// The serial.
    #[field(label = "Serial")]
    pub serial: &'a str,
    /// The receiving channel.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The absolute frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The offset from the centre.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frame counter.
    #[field(label = "Frame")]
    pub frame: Option<i64>,
    /// UTC from the sonde.
    #[field(label = "Sonde time")]
    pub time: Option<Time>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// The highest altitude.
    #[field(label = "Highest", unit = "m")]
    pub max_alt_m: Option<f64>,
    /// The vertical speed.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The satellites used.
    #[field(label = "Satellites")]
    pub sats: Option<i64>,
    /// The temperature.
    #[field(label = "Temperature", unit = "°C")]
    pub temp_c: Option<f64>,
    /// The relative humidity.
    #[field(label = "Humidity", unit = "%")]
    pub humidity_pct: Option<f64>,
    /// The pressure.
    #[field(label = "Pressure", unit = "hPa")]
    pub pressure_hpa: Option<f64>,
    /// The battery.
    #[field(label = "Battery", unit = "V")]
    pub battery_v: Option<f64>,
    /// The burst or kill timer.
    #[field(label = "Kill timer", unit = "s")]
    pub kill: Option<Seconds>,
    /// The calibration parts held.
    #[field(label = "Calibration held")]
    pub cal_held: Option<i64>,
    /// The calibration parts needed.
    #[field(label = "Calibration needed")]
    pub cal_needed: Option<i64>,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Good frames as a share of all frames.
    #[field(label = "Good frames", unit = "%")]
    pub ok_pct: f64,
    /// The predicted landing point.
    #[field(label = "Landing")]
    pub landing: Option<(f64, f64)>,
    /// The time to the landing.
    #[field(label = "Landing in", unit = "s")]
    pub landing_in: Option<Seconds>,
    /// The distance from the station to the landing point.
    #[field(label = "Landing distance", unit = "m")]
    pub landing_distance_m: Option<f64>,
    /// The bearing from the station to the landing point.
    #[field(label = "Landing bearing", unit = "°")]
    pub landing_bearing_deg: Option<f64>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A carrier of the last search. The row key is the place in the list, two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SondeCarrierRow {
    /// The offset from the centre.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// The absolute frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The carrier level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The receiving channel.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
}

/// A sonde frame, every 5 s for each sonde. `source` is the serial, `text` the sonde for people.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SondeFrameEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The family.
    #[field(label = "Type")]
    pub r#type: crate::sonde::frame::SondeType,
    /// The serial.
    #[field(label = "Serial")]
    pub serial: &'a str,
    /// The temperature.
    #[field(label = "Temperature", unit = "°C")]
    pub temp_c: Option<f64>,
    /// The relative humidity.
    #[field(label = "Humidity", unit = "%")]
    pub humidity_pct: Option<f64>,
    /// The vertical speed.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The sonde's burst or kill timer runs.
    #[field(label = "Burst killer")]
    pub burst_killer: Option<bool>,
}

/// The sondes, the most recently heard first.
pub static SONDE_SONDES: Table<SondeRow<'static>> =
    Table::new("sondes", "Radiosondes", crate::sonde::table::MAX_SONDES).label(&SondeRow::SERIAL);
/// The carriers of the last search, strongest first.
pub static SONDE_CARRIERS: Table<SondeCarrierRow> =
    Table::new("carriers", "Carriers", crate::sonde::table::MAX_CARRIERS).sorted(
        &SondeCarrierRow::LEVEL_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// A sonde frame.
pub static SONDE_FRAME: EventKind<SondeFrameEvent<'static>> =
    EventKind::new("sonde.frame", "Radiosonde frame");

/// The row key of a sonde, an argument of `remove` and `select`.
pub const SONDE_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of("sondes");

/// The arguments of `remove` and `select` of `sonde`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct SondeRowArgs<'a> {
    /// The row key of the sonde.
    #[arg(shared = SONDE_ROW)]
    pub row: &'a str,
}

/// Forgets one sonde.
pub const SONDE_REMOVE: CommandKind<SondeRowArgs<'static>> =
    sdrkit_util_objects::commands::remove_kind();
/// Selects the sonde the plots show.
pub const SONDE_SELECT: CommandKind<SondeRowArgs<'static>> =
    CommandKind::new("select", "Select").help("Shows the plots of one sonde.");
/// The altitude over time of the selected sonde.
pub const SONDE_ALT_TIME: SeriesDef = SeriesDef {
    key: "alt_time",
    label: "Altitude",
    x_unit: "s",
    y_unit: "m",
};
/// The temperature by altitude of the selected sonde.
pub const SONDE_TEMP_ALT: SeriesDef = SeriesDef {
    key: "temp_alt",
    label: "Temperature by altitude",
    x_unit: "°C",
    y_unit: "m",
};
/// The track (latitude, longitude) of the selected sonde.
pub const SONDE_TRACK: SeriesDef = SeriesDef {
    key: "track",
    label: "Track",
    x_unit: "°",
    y_unit: "°",
};

// ---- astro ----

/// The result fields of `astro`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AstroFields {
    /// Finished integrations.
    #[field(label = "Integrations")]
    pub finished: i64,
    /// The progress of the current integration.
    #[field(label = "Progress", unit = "%")]
    pub progress_pct: f64,
    /// The width of a bin.
    #[field(label = "Bin width", unit = "Hz")]
    pub bin: Hertz,
    /// The width of a bin in velocity.
    #[field(label = "Bin width", unit = "km/s")]
    pub bin_km_s: f64,
    /// The span of the spectrum.
    #[field(label = "Span", unit = "Hz")]
    pub span: Hertz,
    /// The span in velocity.
    #[field(label = "Span", unit = "km/s")]
    pub span_km_s: f64,
    /// Bins flagged as RFI in the last spectrum.
    #[field(label = "Bins flagged")]
    pub bins_flagged: i64,
    /// The frequency of the peak.
    #[field(label = "Peak", unit = "Hz")]
    pub peak: Option<Hertz>,
    /// The velocity of the peak.
    #[field(label = "Peak velocity", unit = "km/s")]
    pub peak_km_s: Option<f64>,
    /// The excess at the peak.
    #[field(label = "Excess", unit = "%")]
    pub excess_pct: Option<f64>,
    /// The peak over the residual rms.
    #[field(label = "Peak over rms")]
    pub line_snr: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The right ascension of the target now.
    #[field(label = "Target RA", unit = "°")]
    pub target_ra_deg: f64,
    /// The declination of the target now.
    #[field(label = "Target Dec", unit = "°")]
    pub target_dec_deg: f64,
    /// The azimuth of the target, with a station position.
    #[field(label = "Target azimuth", unit = "°")]
    pub target_az_deg: Option<f64>,
    /// The elevation of the target, with a station position.
    #[field(label = "Target elevation", unit = "°")]
    pub target_el_deg: Option<f64>,
    /// The observer's velocity towards the target from the Earth's rotation.
    #[field(label = "Earth rotation", unit = "km/s")]
    pub lsr_rotation_kms: Option<f64>,
    /// The observer's velocity towards the target from the Earth's orbit.
    #[field(label = "Earth orbit", unit = "km/s")]
    pub lsr_orbit_kms: Option<f64>,
    /// The observer's velocity towards the target from the Sun's motion.
    #[field(label = "Sun", unit = "km/s")]
    pub lsr_sun_kms: Option<f64>,
    /// The velocity of the peak towards the target, corrected to the local standard of rest.
    #[field(label = "Peak velocity (LSR)", unit = "km/s")]
    pub vel_lsr_kms: Option<f64>,
    /// The receiver temperature from the Y factor.
    #[field(label = "Receiver temperature", unit = "K")]
    pub trx_k: Option<f64>,
    /// The gain per kelvin from the Y factor.
    #[field(label = "Gain per kelvin")]
    pub gain_per_k: Option<f64>,
    /// A note on the calibration, the switching or the fit.
    #[field(label = "Note")]
    pub note: Option<crate::astro::Note>,
}

/// A bin of the calibrated spectrum of the last finished integration. The row key is the bin, five digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AstroCalibratedRow {
    /// The frequency of a bin.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// The topocentric velocity of a bin.
    #[field(label = "Velocity (topocentric)", unit = "km/s")]
    pub vel_topo_kms: f64,
    /// The LSR velocity of a bin (none without a station position).
    #[field(label = "Velocity (LSR)", unit = "km/s")]
    pub vel_lsr_kms: Option<f64>,
    /// The antenna temperature of a bin: the processed ratio times the system temperature.
    #[field(label = "Antenna temperature", unit = "K")]
    pub ta_k: f64,
    /// The bin was flagged as RFI.
    #[field(label = "Flagged")]
    pub flagged: bool,
}

/// A finished integration of the session. The row key is the sequence number of the integration.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AstroSessionRow {
    /// The signal time at the end of an integration.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The peak velocity of an integration.
    #[field(label = "Peak velocity", unit = "km/s")]
    pub peak_kms: f64,
    /// The excess at the peak.
    #[field(label = "Excess", unit = "%")]
    pub excess_pct: f64,
    /// The peak over the residual rms.
    #[field(label = "Peak over rms")]
    pub line_snr: f64,
}

/// A Gaussian of the last line fit. The row key is the place in the fit.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AstroFitRow {
    /// The centre of a fitted Gaussian.
    #[field(label = "Centre", unit = "km/s")]
    pub centre_kms: f64,
    /// The amplitude of a fitted Gaussian.
    #[field(label = "Amplitude", unit = "K")]
    pub amplitude_k: f64,
    /// The full width at half maximum of a fitted Gaussian.
    #[field(label = "FWHM", unit = "km/s")]
    pub fwhm_kms: f64,
}

/// A row of the table `lv_rows`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AstroMapRow {
    /// The galactic longitude of a row of the map.
    #[field(label = "Longitude", unit = "°")]
    pub longitude: f64,
    /// The terminal velocity at a longitude.
    #[field(label = "Terminal velocity", unit = "km/s")]
    pub terminal_kms: Option<f64>,
}

/// Calibrated spectrum.
pub static ASTRO_CALIBRATED: Table<AstroCalibratedRow> =
    Table::new("calibrated", "Calibrated spectrum", crate::astro::MAX_BINS);
/// Session.
pub static ASTRO_SESSION: Table<AstroSessionRow> =
    Table::new("session", "Session", crate::astro::MAX_SESSION);
/// Line fit.
pub static ASTRO_FIT: Table<AstroFitRow> = Table::new("fit", "Line fit", 8);
/// Map rows.
pub static ASTRO_LV_ROWS: Table<AstroMapRow> =
    Table::new("lv_rows", "Map rows", crate::astro::MAX_MAP_ROWS);

/// The arguments of `capture` of `astro`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct AstroCaptureArgs {
    /// What a capture keeps.
    #[arg(
        label = "What",
        help = "The off-source spectrum, or the power of the hot or the cold load."
    )]
    pub what: crate::astro::CaptureChoice,
}

/// Keeps the next finished integration as the off-source spectrum or a load power.
pub const ASTRO_CAPTURE: CommandKind<AstroCaptureArgs> = CommandKind::new("capture", "Capture").help(
    "Keeps the next finished integration as the off-source spectrum, or its power as the hot or the cold load.",
);
/// The spectrum of the last integration: x km/s, y the ratio.
pub const ASTRO_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Spectrum",
    x_unit: "km/s",
    y_unit: "",
};
/// The total power over time.
pub const ASTRO_TOTAL_POWER: SeriesDef = SeriesDef {
    key: "total_power",
    label: "Total power",
    x_unit: "s",
    y_unit: "dBFS",
};
/// The peak excess of each finished integration over time.
pub const ASTRO_DRIFT: SeriesDef = SeriesDef {
    key: "drift",
    label: "Peak excess by time",
    x_unit: "s",
    y_unit: "",
};
/// The longitude-velocity map: one row per longitude added (table `lv_rows`), one column per km/s from -250.
pub const ASTRO_LV_MAP: RasterDef = RasterDef {
    key: "lv_map",
    label: "Longitude-velocity map",
    x_unit: "km/s",
    y_unit: "°",
    unit: "K",
};

// ---- iono ----

/// The result fields of `iono`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IonoFields<'a> {
    /// The input rate.
    #[field(label = "Sample rate", unit = "Hz")]
    pub rate: Hertz,
    /// The radio frequency of the centre.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Hertz,
    /// The signal time (UTC with a radio, else from the first sample).
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// Sweeps start on whole seconds.
    #[field(label = "Timing lock")]
    pub timing_lock: bool,
    /// The span the radio sees.
    #[field(label = "Span", unit = "Hz")]
    pub span: Hertz,
    /// Sounders heard.
    #[field(label = "Sounders heard")]
    pub sounders_heard: i64,
    /// Sweeps being received.
    #[field(label = "Active sweeps")]
    pub active_sweeps: i64,
    /// The FFT size of the detector.
    #[field(label = "Detector FFT")]
    pub detector_fft: Option<i64>,
    /// The duration of a detector FFT.
    #[field(label = "Detector FFT", unit = "ms")]
    pub detector_ms: Option<f64>,
    /// The bin width of the detector.
    #[field(label = "Detector bin", unit = "Hz")]
    pub detector_bin: Option<Hertz>,
    /// FFTs done.
    #[field(label = "FFTs")]
    pub ffts: Option<i64>,
    /// Chirp peaks not yet tied to a sweep.
    #[field(label = "Peaks not yet tied")]
    pub loose_peaks: Option<i64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The row key of the sounder whose ionogram and history are shown.
    #[field(label = "Sounder shown", refers_to = "sounders")]
    pub selected: Option<&'a str>,
}

/// A sounder heard. The row key is the sounder's id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IonoSounderRow<'a> {
    /// The id of a sounder.
    #[field(label = "Sounder")]
    pub id: i64,
    /// A sweep rate.
    #[field(label = "Rate", unit = "kHz/s")]
    pub rate_khz_s: f64,
    /// The repetition period.
    #[field(label = "Period", unit = "s")]
    pub period: Option<Seconds>,
    /// The start offset in the period.
    #[field(label = "Offset", unit = "s")]
    pub offset_s: Option<f64>,
    /// The start of the last sweep.
    #[field(label = "Last sweep", unit = "s")]
    pub last_start_s: f64,
    /// When the sounder was last heard.
    #[field(label = "Last seen", unit = "s")]
    pub last_seen_s: f64,
    /// The start of the next sweep.
    #[field(label = "Next expected", unit = "s")]
    pub next_s: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The lowest radio frequency heard.
    #[field(label = "Heard from", unit = "Hz")]
    pub heard_low: Hertz,
    /// The highest radio frequency heard.
    #[field(label = "Heard to", unit = "Hz")]
    pub heard_high: Hertz,
    /// Sweeps received.
    #[field(label = "Sweeps")]
    pub sweeps: i64,
    /// Expected sweeps not heard.
    #[field(label = "Missed")]
    pub missed: i64,
    /// A sweep is being received.
    #[field(label = "Active")]
    pub active: bool,
    /// The name of the known sounder it matches.
    #[field(label = "Known sounder")]
    pub known: Option<&'a str>,
    /// How well it matches: 0 no match, 1 the rate (and the distance), 2 the rate and the schedule.
    #[field(label = "Match")]
    pub match_level: i64,
    /// The known sounders of the same rate.
    #[field(label = "Candidates")]
    pub candidates: i64,
    /// The geometry of the latest scaling.
    #[field(label = "Geometry")]
    pub geometry: Option<crate::iono::GeometryChoice>,
    /// The critical frequency of the E layer.
    #[field(label = "foE", unit = "MHz")]
    pub foe_mhz: Option<f64>,
    /// The critical frequency of the F1 layer.
    #[field(label = "foF1", unit = "MHz")]
    pub fof1_mhz: Option<f64>,
    /// The critical frequency of the F2 layer.
    #[field(label = "foF2", unit = "MHz")]
    pub fof2_mhz: Option<f64>,
    /// The virtual height of the E layer.
    #[field(label = "h'E", unit = "km")]
    pub hpe_km: Option<f64>,
    /// The virtual height of the F layer.
    #[field(label = "h'F", unit = "km")]
    pub hpf_km: Option<f64>,
    /// The propagation factor M(3000)F2.
    #[field(label = "M(3000)F2")]
    pub m3000: Option<f64>,
    /// The MUF over 3000 km.
    #[field(label = "MUF(3000)F2", unit = "MHz")]
    pub muf3000_mhz: Option<f64>,
    /// The MUF of the path from an oblique sounder.
    #[field(label = "Path MUF", unit = "MHz")]
    pub path_muf_mhz: Option<f64>,
    /// The distance of an oblique sounder.
    #[field(label = "Distance", unit = "km")]
    pub distance_km: Option<f64>,
}

/// A sweep heard. The row key is the sequence number of the sweep.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IonoTimelineRow {
    /// The start of a sweep.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// The id of a sounder.
    #[field(label = "Sounder")]
    pub id: i64,
    /// A sweep rate.
    #[field(label = "Rate", unit = "kHz/s")]
    pub rate_khz_s: f64,
    /// The lowest frequency of the sweep that crossed the span.
    #[field(label = "Crossed from", unit = "Hz")]
    pub part_low: Hertz,
    /// The highest frequency of the sweep that crossed the span.
    #[field(label = "Crossed to", unit = "Hz")]
    pub part_high: Hertz,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The sweep was expected.
    #[field(label = "Predicted")]
    pub predicted: bool,
}

/// A band and its state, from the selected sounder's usable MUF. The row key is the name of the band.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IonoBandRow<'a> {
    /// A band.
    #[field(label = "Band")]
    pub band: &'a str,
    /// The state of a band.
    #[field(label = "State")]
    pub state: crate::iono::BandStateChoice,
}

/// The sounders heard.
pub static IONO_SOUNDERS: Table<IonoSounderRow<'static>> = Table::new("sounders", "Sounders", 64);
/// The sweeps heard, newest first.
pub static IONO_TIMELINE: Table<IonoTimelineRow> =
    Table::new("timeline", "Timeline", crate::iono::MAX_TIMELINE);
/// The bands open now.
pub static IONO_BANDS: Table<IonoBandRow<'static>> = Table::new("bands", "Bands open now", 32);

/// The row key of a sounder, the argument of `select`.
pub const IONO_SOUNDER_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of("sounders");

/// The arguments of `select` of `iono`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct IonoSelectArgs<'a> {
    /// The row key of the sounder.
    #[arg(shared = IONO_SOUNDER_ROW)]
    pub row: &'a str,
}

/// Shows the ionogram and history of one sounder.
pub const IONO_SELECT: CommandKind<IonoSelectArgs<'static>> =
    CommandKind::new("select", "Show a sounder")
        .help("Shows the ionogram and the history of one sounder.");
/// The coverage of 0 to 30 MHz.
pub const IONO_COVERAGE: SeriesDef = SeriesDef {
    key: "coverage",
    label: "Coverage (1 captured, 2 span now, 3 selected sounder)",
    x_unit: "MHz",
    y_unit: "",
};
/// The scaled values of the selected sounder over time, in the order of the values of a history point.
pub const IONO_HISTORY: [SeriesDef; 9] = [
    SeriesDef {
        key: "history_foe",
        label: "foE",
        x_unit: "s",
        y_unit: "MHz",
    },
    SeriesDef {
        key: "history_fof1",
        label: "foF1",
        x_unit: "s",
        y_unit: "MHz",
    },
    SeriesDef {
        key: "history_fof2",
        label: "foF2",
        x_unit: "s",
        y_unit: "MHz",
    },
    SeriesDef {
        key: "history_hpe",
        label: "h'E",
        x_unit: "s",
        y_unit: "km",
    },
    SeriesDef {
        key: "history_hpf",
        label: "h'F",
        x_unit: "s",
        y_unit: "km",
    },
    SeriesDef {
        key: "history_m3000",
        label: "M(3000)F2",
        x_unit: "s",
        y_unit: "",
    },
    SeriesDef {
        key: "history_muf3000",
        label: "MUF(3000)F2",
        x_unit: "s",
        y_unit: "MHz",
    },
    SeriesDef {
        key: "history_path_muf",
        label: "Path MUF",
        x_unit: "s",
        y_unit: "MHz",
    },
    SeriesDef {
        key: "history_distance",
        label: "Distance",
        x_unit: "s",
        y_unit: "km",
    },
];

// ---- supersid ----

/// The result fields of `supersid`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SupersidFields<'a> {
    /// Transmitters listed.
    #[field(label = "Transmitters listed")]
    pub listed_n: i64,
    /// Transmitters in span.
    #[field(label = "Transmitters in span")]
    pub in_span_n: i64,
    /// Transmitters present.
    #[field(label = "Transmitters present")]
    pub present_n: i64,
    /// Intervals measured.
    #[field(label = "Intervals measured")]
    pub intervals: i64,
    /// The width of a bin of the spectrum.
    #[field(label = "Bin width", unit = "Hz")]
    pub bin_hz: Option<Hertz>,
    /// Events in total.
    #[field(label = "Events")]
    pub events_total: i64,
    /// The transmitter of the last event.
    #[field(label = "Last event transmitter")]
    pub last_event_tx: Option<&'a str>,
    /// The rise of the last event.
    #[field(label = "Last event rise", unit = "dB")]
    pub last_event_rise_db: Option<f64>,
    /// The age of the last event.
    #[field(label = "Last event age", unit = "s")]
    pub last_event_age_s: Option<Seconds>,
    /// The data file written last.
    #[field(label = "Data file")]
    pub log_file: Option<&'a str>,
    /// Why the data file is not written.
    #[field(label = "Data file error")]
    pub log_error: Option<&'a str>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A monitored transmitter, in list order. The row key is its place in the list, two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SupersidTransmitterRow<'a> {
    /// A transmitter's label.
    #[field(label = "Transmitter")]
    pub label: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// In span.
    #[field(label = "In span")]
    pub in_span: bool,
    /// Present.
    #[field(label = "Present")]
    pub present: bool,
    /// The level over the local noise.
    #[field(label = "Level over noise", unit = "dB")]
    pub level_db: Option<f64>,
    /// The absolute level.
    #[field(label = "Absolute level", unit = "dBFS")]
    pub absolute_dbfs: Option<f64>,
    /// The median of the last hour.
    #[field(label = "Baseline", unit = "dB")]
    pub baseline_db: Option<f64>,
    /// The mean of the last 2 minutes less the baseline.
    #[field(label = "Rise", unit = "dB")]
    pub rise_db: Option<f64>,
    /// An event is on.
    #[field(label = "Event")]
    pub event_on: bool,
    /// The peak step of the event that is on.
    #[field(label = "Event peak", unit = "dB")]
    pub event_peak_db: Option<f64>,
    /// The sunlit share of the path.
    #[field(label = "Path sunlit (D region)")]
    pub sunlit: Option<f64>,
    /// The key of the series of minute means.
    #[field(label = "Minute means")]
    pub series: &'a str,
}

/// A row of the table `sid_events`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SupersidEventRow<'a> {
    /// A transmitter's label.
    #[field(label = "Transmitter")]
    pub label: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The UTC of an event's start.
    #[field(label = "Start")]
    pub start_utc: Time,
    /// The signal time of an event's start.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The mean of the last 2 minutes less the baseline.
    #[field(label = "Rise", unit = "dB")]
    pub rise_db: f64,
    /// The peak step of an event.
    #[field(label = "Peak rise", unit = "dB")]
    pub peak_db: f64,
    /// The UTC of an event's end.
    #[field(label = "End")]
    pub end_utc: Option<Time>,
}

/// A terminator mark of the UTC day. The row key is its place in time order, three digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SupersidMarkRow<'a> {
    /// A transmitter's label.
    #[field(label = "Transmitter")]
    pub label: &'a str,
    /// The point of the path of a mark.
    #[field(label = "Point")]
    pub point: &'a str,
    /// The crossing of a mark.
    #[field(label = "Mark")]
    pub mark: &'a str,
    /// The UTC of a mark.
    #[field(label = "Time")]
    pub utc: Time,
}

/// An event `sid.event`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SupersidSidEvent<'a> {
    /// A transmitter's label.
    #[field(label = "Transmitter")]
    pub label: &'a str,
    /// The UTC of an event's start.
    #[field(label = "Start")]
    pub start_utc: Time,
    /// The mean of the last 2 minutes less the baseline.
    #[field(label = "Rise", unit = "dB")]
    pub rise_db: f64,
    /// The peak step of an event.
    #[field(label = "Peak rise", unit = "dB")]
    pub peak_db: f64,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// Transmitters.
pub static SUPERSID_TRANSMITTERS: Table<SupersidTransmitterRow<'static>> = Table::new(
    "transmitters",
    "Transmitters",
    crate::supersid::list::MAX_TRANSMITTERS,
)
.label(&SupersidTransmitterRow::LABEL);
/// Sudden changes.
pub static SUPERSID_EVENTS: Table<SupersidEventRow<'static>> = Table::new(
    "sid_events",
    "Sudden changes",
    crate::supersid::EVENT_LOG_CAP,
)
.label(&SupersidEventRow::LABEL);
/// Terminator marks.
pub static SUPERSID_MARKS: Table<SupersidMarkRow<'static>> = Table::new(
    "marks",
    "Terminator marks",
    crate::supersid::list::MAX_TRANSMITTERS * 12,
)
.label(&SupersidMarkRow::LABEL);
/// Sudden ionospheric disturbance.
pub static SID_EVENT: EventKind<SupersidSidEvent<'static>> =
    EventKind::new("sid.event", "Sudden ionospheric disturbance");

/// The minute means of each list entry: x seconds, y dB. Entry `i` (from 0) has the series `minutes_<i + 1>`, two digits.
pub const SUPERSID_MINUTES: [SeriesDef; 32] = [
    SeriesDef {
        key: "minutes_01",
        label: "Minute means, transmitter 1",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_02",
        label: "Minute means, transmitter 2",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_03",
        label: "Minute means, transmitter 3",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_04",
        label: "Minute means, transmitter 4",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_05",
        label: "Minute means, transmitter 5",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_06",
        label: "Minute means, transmitter 6",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_07",
        label: "Minute means, transmitter 7",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_08",
        label: "Minute means, transmitter 8",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_09",
        label: "Minute means, transmitter 9",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_10",
        label: "Minute means, transmitter 10",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_11",
        label: "Minute means, transmitter 11",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_12",
        label: "Minute means, transmitter 12",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_13",
        label: "Minute means, transmitter 13",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_14",
        label: "Minute means, transmitter 14",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_15",
        label: "Minute means, transmitter 15",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_16",
        label: "Minute means, transmitter 16",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_17",
        label: "Minute means, transmitter 17",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_18",
        label: "Minute means, transmitter 18",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_19",
        label: "Minute means, transmitter 19",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_20",
        label: "Minute means, transmitter 20",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_21",
        label: "Minute means, transmitter 21",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_22",
        label: "Minute means, transmitter 22",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_23",
        label: "Minute means, transmitter 23",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_24",
        label: "Minute means, transmitter 24",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_25",
        label: "Minute means, transmitter 25",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_26",
        label: "Minute means, transmitter 26",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_27",
        label: "Minute means, transmitter 27",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_28",
        label: "Minute means, transmitter 28",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_29",
        label: "Minute means, transmitter 29",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_30",
        label: "Minute means, transmitter 30",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_31",
        label: "Minute means, transmitter 31",
        x_unit: "s",
        y_unit: "dB",
    },
    SeriesDef {
        key: "minutes_32",
        label: "Minute means, transmitter 32",
        x_unit: "s",
        y_unit: "dB",
    },
];

// ---- lightning_tdoa ----

/// The result fields of `lightning_tdoa`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LightningTdoaFields<'a> {
    /// The role in use.
    #[field(label = "Role")]
    pub role: &'a str,
    /// Sferics timed.
    #[field(label = "Sferics timed")]
    pub pulses: i64,
    /// The peak of the newest sferic.
    #[field(label = "Last peak", unit = "dB")]
    pub last_peak_db: Option<f64>,
    /// The floor of the sferic timer.
    #[field(label = "Floor", unit = "dBFS")]
    pub floor_db: Option<f64>,
    /// The best usable time source.
    #[field(label = "Time source")]
    pub time_source: &'a str,
    /// Its 1-sigma now.
    #[field(label = "Clock error", unit = "µs")]
    pub clock_sigma_us: Option<f64>,
    /// The rate error of its sample clock.
    #[field(label = "Sample clock drift", unit = "ppm")]
    pub clock_ppm: Option<f64>,
    /// The 1-sigma of the drift.
    #[field(label = "Drift error", unit = "ppm")]
    pub clock_ppm_sigma: Option<f64>,
    /// A detection now would be fine.
    #[field(label = "Fine timing")]
    pub fine: bool,
    /// The 1-sigma of the GPS clock.
    #[field(label = "GPS clock", unit = "µs")]
    pub sigma_gps_us: Option<f64>,
    /// The 1-sigma of the GNSS clock.
    #[field(label = "GNSS clock", unit = "µs")]
    pub sigma_gnss_us: Option<f64>,
    /// The 1-sigma of the common-view clock.
    #[field(label = "Common-view clock", unit = "µs")]
    pub sigma_cv_us: Option<f64>,
    /// The 1-sigma of the PTP clock.
    #[field(label = "PTP clock", unit = "µs")]
    pub sigma_ptp_us: Option<f64>,
    /// The 1-sigma of the NTP clock.
    #[field(label = "NTP clock", unit = "µs")]
    pub sigma_ntp_us: Option<f64>,
    /// The 1-sigma of the unsynchronised computer clock.
    #[field(label = "Computer clock", unit = "µs")]
    pub sigma_system_us: Option<f64>,
    /// The common-view clock stamps the detections.
    #[field(label = "Common view in use")]
    pub common_view: bool,
    /// The timing transmitter is locked.
    #[field(label = "Timing transmitter locked")]
    pub timing_locked: Option<bool>,
    /// Its SNR.
    #[field(label = "Timing SNR", unit = "dB")]
    pub timing_snr_db: Option<f64>,
    /// The 1-sigma of its boundary.
    #[field(label = "Timing boundary error", unit = "µs")]
    pub timing_sigma_us: Option<f64>,
    /// Its carrier error.
    #[field(label = "Timing carrier error", unit = "Hz")]
    pub timing_carrier_hz: Option<f64>,
    /// The state of the network.
    #[field(label = "Network")]
    pub network: &'a str,
    /// Datagrams sent.
    #[field(label = "Packets sent")]
    pub sent: i64,
    /// Datagrams that could not be sent.
    #[field(label = "Send errors")]
    pub send_errors: i64,
    /// Datagrams accepted by the server.
    #[field(label = "Packets accepted")]
    pub packets_ok: Option<i64>,
    /// Refused by the MAC.
    #[field(label = "Refused (MAC)")]
    pub refused_mac: Option<i64>,
    /// Refused as replays.
    #[field(label = "Replays")]
    pub replays: Option<i64>,
    /// Refused as stale.
    #[field(label = "Stale")]
    pub stale: Option<i64>,
    /// Refused as malformed.
    #[field(label = "Malformed")]
    pub malformed: Option<i64>,
    /// Refused by the rate limit.
    #[field(label = "Rate limited")]
    pub rate_limited: Option<i64>,
    /// Groups of detections tried.
    #[field(label = "Groups")]
    pub groups: Option<i64>,
    /// Fixes made.
    #[field(label = "Fixes")]
    pub fixes: Option<i64>,
    /// Fixes refused by the chi-square test.
    #[field(label = "Rejected")]
    pub rejected: Option<i64>,
    /// Groups without a fix.
    #[field(label = "Unmatched")]
    pub unmatched: Option<i64>,
    /// Groups with too few fine stations.
    #[field(label = "Coarse only")]
    pub coarse_only: Option<i64>,
    /// Impulses too long for lightning.
    #[field(label = "Too long")]
    pub too_long: i64,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A located strike (server). The row key is the sequence number of the strike, eight digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LightningTdoaStrikeRow {
    /// The time of a strike, UTC ns.
    #[field(label = "Time (UTC ns)", unit = "ns")]
    pub utc_ns: i64,
    /// Latitude.
    #[field(label = "Latitude", unit = "°", role = Latitude)]
    pub lat: f64,
    /// Longitude.
    #[field(label = "Longitude", unit = "°", role = Longitude)]
    pub lon: f64,
    /// The semi-major axis of the 95 % ellipse.
    #[field(label = "Ellipse major", unit = "km")]
    pub major_km: f64,
    /// The semi-minor axis.
    #[field(label = "Ellipse minor", unit = "km")]
    pub minor_km: f64,
    /// The azimuth of the major axis.
    #[field(label = "Ellipse azimuth", unit = "°")]
    pub az_deg: f64,
    /// Stations used.
    #[field(label = "Stations")]
    pub stations: i64,
    /// Coarse stations that heard it.
    #[field(label = "Also heard")]
    pub also_heard: i64,
    /// The rms of the residuals.
    #[field(label = "Residual rms", unit = "µs")]
    pub rms_us: f64,
    /// Chi-square.
    #[field(label = "Chi-square")]
    pub chi2: f64,
    /// Degrees of freedom.
    #[field(label = "Degrees of freedom")]
    pub dof: i64,
    /// No degree of freedom checks it.
    #[field(label = "Unchecked")]
    pub unchecked: bool,
    /// A second solution fits as well.
    #[field(label = "Ambiguous")]
    pub ambiguous: bool,
    /// The time is UTC.
    #[field(label = "UTC")]
    pub utc: bool,
}

/// A station of the network (server). The row key is the station id, eight hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LightningTdoaStationRow<'a> {
    /// The station id.
    #[field(label = "Station id")]
    pub id: &'a str,
    /// The station name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// Latitude.
    #[field(label = "Latitude", unit = "°", role = Latitude)]
    pub lat: f64,
    /// Longitude.
    #[field(label = "Longitude", unit = "°", role = Longitude)]
    pub lon: f64,
    /// The best usable time source.
    #[field(label = "Time source")]
    pub time_source: &'a str,
    /// The 1-sigma of the station's clock.
    #[field(label = "Clock error", unit = "µs")]
    pub sigma_us: f64,
    /// The time since the station's last datagram.
    #[field(label = "Age", unit = "s")]
    pub age_s: f64,
    /// Detections received.
    #[field(label = "Detections")]
    pub detections: i64,
    /// Detections used in fixes.
    #[field(label = "Used in fixes")]
    pub used: i64,
    /// A detection now would be fine.
    #[field(label = "Fine timing")]
    pub fine: bool,
    /// The station's clock is UTC.
    #[field(label = "Tied to UTC")]
    pub tied_to_utc: bool,
    /// The common-view correction.
    #[field(label = "Common-view correction", unit = "µs")]
    pub cv_correction_us: Option<f64>,
}

/// A strike located (server).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LightningStrikeEvent {
    /// The time of a strike, UTC ns.
    #[field(label = "Time (UTC ns)", unit = "ns")]
    pub utc_ns: i64,
    /// Latitude.
    #[field(label = "Latitude", unit = "°")]
    pub lat: f64,
    /// Longitude.
    #[field(label = "Longitude", unit = "°")]
    pub lon: f64,
    /// The semi-major axis of the 95 % ellipse.
    #[field(label = "Ellipse major", unit = "km")]
    pub major_km: f64,
    /// The semi-minor axis.
    #[field(label = "Ellipse minor", unit = "km")]
    pub minor_km: f64,
    /// The azimuth of the major axis.
    #[field(label = "Ellipse azimuth", unit = "°")]
    pub az_deg: f64,
    /// Stations used.
    #[field(label = "Stations")]
    pub stations: i64,
    /// Coarse stations that heard it.
    #[field(label = "Also heard")]
    pub also_heard: i64,
    /// The rms of the residuals.
    #[field(label = "Residual rms", unit = "µs")]
    pub rms_us: f64,
    /// Chi-square.
    #[field(label = "Chi-square")]
    pub chi2: f64,
    /// Degrees of freedom.
    #[field(label = "Degrees of freedom")]
    pub dof: i64,
    /// No degree of freedom checks it.
    #[field(label = "Unchecked")]
    pub unchecked: bool,
    /// A second solution fits as well.
    #[field(label = "Ambiguous")]
    pub ambiguous: bool,
    /// The time is UTC.
    #[field(label = "UTC")]
    pub utc: bool,
}

/// The located strikes, newest first (server).
pub static LIGHTNING_TDOA_STRIKES: Table<LightningTdoaStrikeRow> =
    Table::new("strikes", "Strikes", crate::lightning::server::MAX_STRIKES);
/// The stations of the network (server).
pub static LIGHTNING_TDOA_STATIONS: Table<LightningTdoaStationRow<'static>> = Table::new(
    "stations",
    "Stations",
    crate::lightning::server::MAX_STATIONS,
)
.label(&LightningTdoaStationRow::NAME);
/// A strike located (server).
pub static LIGHTNING_STRIKE: EventKind<LightningStrikeEvent> =
    EventKind::new("lightning.strike", "Lightning strike");

/// The arguments of `datagram` of `lightning_tdoa`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct LightningTdoaDatagramArgs {
    /// The bytes of a datagram.
    #[arg(label = "Datagram", help = "One datagram of a station.")]
    pub bytes: Arc<[u8]>,
}

/// The arguments of `time_mark` of `lightning_tdoa`.
#[derive(Clone, Debug, PartialEq, Arguments)]
pub struct LightningTdoaTimeMarkArgs<'a> {
    /// The source of a time mark.
    #[arg(label = "Source", help = "gps or gnss.")]
    pub source: &'a str,
    /// The input sample of a time mark.
    #[arg(label = "Input sample", help = "The absolute input index.")]
    pub sample: i64,
    /// The UTC of a time mark.
    #[arg(
        label = "UTC",
        unit = "ns",
        help = "UTC at that sample, ns since 1970."
    )]
    pub utc_ns: i64,
    /// The 1-sigma of a GNSS mark.
    #[arg(
        label = "1-sigma",
        unit = "ns",
        default = 0.0,
        min = 0.0,
        max = 1e9,
        step = 1.0,
        help = "GNSS: 0 takes the assumed dilution and range residual. GPS marks have their own figures."
    )]
    pub sigma_ns: Option<f64>,
    /// The stream-alignment error of a GNSS mark.
    #[arg(
        label = "Bias",
        unit = "ns",
        default = 0.0,
        min = 0.0,
        max = 1e9,
        step = 1.0,
        help = "GNSS: the error of moving the fix onto this radio's samples."
    )]
    pub bias_ns: Option<f64>,
}

/// Feeds the server one datagram.
pub const LIGHTNING_TDOA_DATAGRAM: CommandKind<LightningTdoaDatagramArgs> =
    CommandKind::new("datagram", "Feed a datagram")
        .help("The server takes one datagram as if it came from the network.");
/// Gives the station a GPS or GNSS time mark.
pub const LIGHTNING_TDOA_TIME_MARK: CommandKind<LightningTdoaTimeMarkArgs<'static>> =
    CommandKind::new("time_mark", "Time mark").help(
        "UTC at an input sample, from the radio's GPS time or a GNSS receiver, as the host reads it.",
    );

// ---- horus ----

/// The present state of the channels of `horus`. The channel bank writes it; a change of frequency clears it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusFields {
    /// Waiting for tones, listening to tones, or decoding (a packet within the last 30 s).
    #[field(label = "State")]
    pub state: crate::horus::State,
    /// Channels running, the dial channel included.
    #[field(label = "Channels")]
    pub channels: i64,
    /// Unique words found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Frames that failed their CRC.
    #[field(label = "Failed")]
    pub failed: i64,
    /// The best SNR in 3 kHz of a packet of the last 60 s.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: Option<f64>,
}

/// What `horus` received: the packet log and the payloads. It stays over a change of frequency.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusLogFields {
    /// Packets decoded, duplicates left out.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Payloads heard within the last 120 s.
    #[field(label = "Active payloads")]
    pub payloads_active: i64,
    /// Lines of the setting `payload_ids` that could not be read.
    #[field(label = "Unreadable ID lines")]
    pub unreadable_ids: i64,
}

/// A row of the table `payloads`. The row key is the payload: its callsign, or "ID n" for an ID that the list does not name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusPayloadRow<'a> {
    /// The callsign, or "ID n".
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The kind of the last packet.
    #[field(label = "Type")]
    pub r#type: crate::horus::Kind,
    /// The payload ID of a binary packet.
    #[field(label = "Payload ID")]
    pub payload_id: Option<i64>,
    /// The sequence number of the last packet.
    #[field(label = "Sequence")]
    pub seq: i64,
    /// The time of the last packet: its time of day on the receive date, UTC.
    #[field(label = "Payload time")]
    pub time: Time,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: f64,
    /// The highest altitude so far.
    #[field(label = "Highest", unit = "m")]
    pub max_alt_m: f64,
    /// The climb rate over the track of the last 30 to 180 s.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The speed of a binary packet.
    #[field(label = "Speed", unit = "km/h", role = Speed)]
    pub speed_kmh: Option<f64>,
    /// The satellites of a binary packet.
    #[field(label = "Satellites")]
    pub sats: Option<i64>,
    /// The temperature of a binary packet.
    #[field(label = "Temperature", unit = "°C")]
    pub temp_c: Option<f64>,
    /// The battery of a binary packet.
    #[field(label = "Battery", unit = "V")]
    pub battery_v: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Packets of the payload.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The middle of the tones of the last packet, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
    /// The frequency of the middle of the tones.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The symbol rate of the last packet.
    #[field(label = "Baud", unit = "Bd")]
    pub baud: f64,
    /// The tone spacing (4FSK) or shift (RTTY) of the last packet.
    #[field(label = "Tone spacing", unit = "Hz")]
    pub spacing: Hertz,
    /// The bits that the Golay code corrected in the last binary packet.
    #[field(label = "Golay corrections")]
    pub corrections: Option<i64>,
    /// The data bits of a character of the last RTTY packet.
    #[field(label = "RTTY data bits")]
    pub rtty_bits: Option<i64>,
    /// Points of the track.
    #[field(label = "Track points")]
    pub track_points: i64,
    /// The last packet as a sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
    /// Wall-clock time when first heard, in whole seconds.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A custom field of the last v2 packet of a payload, or a field after the altitude of its last RTTY sentence. The row key is
/// the payload, "/" and the place of the field from 1.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusCustomFieldRow<'a> {
    /// The payload.
    #[field(label = "Payload", refers_to = "payloads")]
    pub payload: &'a str,
    /// The name of the field.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The value, written by its post-processing.
    #[field(label = "Value")]
    pub value: &'a str,
}

/// A row of the packet log. The row key is the number of the packet, six digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusPacketRow<'a> {
    /// When the packet was received.
    #[field(label = "Received")]
    pub received: Time,
    /// The payload.
    #[field(label = "Payload", refers_to = "payloads")]
    pub payload: &'a str,
    /// The kind of the packet.
    #[field(label = "Type")]
    pub r#type: crate::horus::Kind,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The middle of the tones, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
    /// The channel that received it (0 is the dial).
    #[field(label = "Channel")]
    pub channel: i64,
    /// The packet as a sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
}

/// A row of the table `channels`. The row key is the channel number (0 is the dial channel).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusChannelRow {
    /// The centre of the channel, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
    /// What the channel decodes.
    #[field(label = "Decoding")]
    pub decoding: crate::horus::Mode,
    /// The lowest 4FSK tone, from the channel centre.
    #[field(label = "Tone 1", unit = "Hz")]
    pub tone_1: Option<Hertz>,
    /// The second 4FSK tone.
    #[field(label = "Tone 2", unit = "Hz")]
    pub tone_2: Option<Hertz>,
    /// The third 4FSK tone.
    #[field(label = "Tone 3", unit = "Hz")]
    pub tone_3: Option<Hertz>,
    /// The highest 4FSK tone.
    #[field(label = "Tone 4", unit = "Hz")]
    pub tone_4: Option<Hertz>,
    /// The lower RTTY tone, from the channel centre.
    #[field(label = "RTTY low tone", unit = "Hz")]
    pub rtty_low: Option<Hertz>,
    /// The higher RTTY tone.
    #[field(label = "RTTY high tone", unit = "Hz")]
    pub rtty_high: Option<Hertz>,
    /// The 4FSK tone spacing.
    #[field(label = "Tone spacing", unit = "Hz")]
    pub spacing: Option<Hertz>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Packets decoded.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Unique words found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Frames that failed their CRC.
    #[field(label = "Failed")]
    pub failed: i64,
    /// The span search opened the channel.
    #[field(label = "Opened by the search")]
    pub searched: bool,
}

/// A Horus packet. `source` is the payload, `text` the summary "Horus CALL #seq lat lon alt m".
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusPacketEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// The kind of the packet.
    #[field(label = "Type")]
    pub r#type: crate::horus::Kind,
    /// The sequence number.
    #[field(label = "Sequence")]
    pub seq: i64,
    /// The climb rate.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The speed of a binary packet.
    #[field(label = "Speed", unit = "km/h")]
    pub speed_kmh: Option<f64>,
    /// The satellites of a binary packet.
    #[field(label = "Satellites")]
    pub sats: Option<i64>,
    /// The temperature of a binary packet.
    #[field(label = "Temperature", unit = "°C")]
    pub temp_c: Option<f64>,
    /// The battery of a binary packet.
    #[field(label = "Battery", unit = "V")]
    pub battery_v: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The middle of the tones, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
    /// The channel that received it (0 is the dial).
    #[field(label = "Channel")]
    pub channel: i64,
    /// The packet as a sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
}

/// The span search opened a channel on a signal. `text` is "a signal +x.x kHz from the dial".
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusChannelOpenedEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The centre of the channel, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
}

/// A channel that the span search opened closed: it decoded nothing for 120 s and its signal was gone for 30 s.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HorusChannelClosedEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The centre of the channel, from the dial.
    #[field(label = "Offset from the dial", unit = "Hz")]
    pub offset: Hertz,
}

/// The payloads, the most recently heard first.
pub static HORUS_PAYLOADS: Table<HorusPayloadRow<'static>> =
    Table::new("payloads", "Payloads", crate::horus::payloads::MAX_PAYLOADS);
/// The custom fields of the last v2 packet of each payload, and the fields after the altitude of its last RTTY sentence.
pub static HORUS_CUSTOM_FIELDS: Table<HorusCustomFieldRow<'static>> = Table::new(
    "custom_fields",
    "Custom fields",
    crate::horus::payloads::MAX_CUSTOM_ROWS,
)
.label(&HorusCustomFieldRow::NAME);
/// The packet log, the newest first.
pub static HORUS_PACKETS: Table<HorusPacketRow<'static>> =
    Table::new("packets", "Packets", crate::horus::payloads::MAX_LOG);
/// The channels, the dial channel first.
pub static HORUS_CHANNELS: Table<HorusChannelRow> =
    Table::new("channels", "Channels", crate::horus::bank::MAX_CHANNELS);
/// A Horus packet.
pub static HORUS_PACKET: EventKind<HorusPacketEvent<'static>> =
    EventKind::new("horus.packet", "Horus packet");
/// The span search opened a channel.
pub static HORUS_CHANNEL_OPENED: EventKind<HorusChannelOpenedEvent<'static>> =
    EventKind::new("horus.channel_opened", "Channel opened");
/// A channel of the span search closed.
pub static HORUS_CHANNEL_CLOSED: EventKind<HorusChannelClosedEvent<'static>> =
    EventKind::new("horus.channel_closed", "Channel closed");
/// The spectrum of the dial channel: dB over its median, Hz from the channel centre.
pub const HORUS_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Dial channel spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let objects_clear = &sdrkit_util_objects::commands::CLEAR;
        let decoder_clear = &sdrkit_graph::decoder::CLEAR;
        Family::new("science", 1, 1)
            .added(
                1,
                1,
                &[
                    "sonde.content",
                    "time.content",
                    "eczas.content",
                    "same.content",
                    "mscatter.content",
                    "ssdv.content",
                    "astro.content",
                    "iono.content",
                    "sferics.content",
                    "supersid.content",
                    "schumann.content",
                    "whistler.content",
                    "lightning_tdoa.content",
                    "jove.content",
                    "callisto.content",
                    "riometer.content",
                    "eme_echo.content",
                    "beacon_events.content",
                    "pulsar.content",
                    "radar_detect.content",
                    "horus.content",
                ],
            )
            .content(sdrkit_core::ContentKind::Tracking, &["sonde", "horus"])
            .content(sdrkit_core::ContentKind::Datalink, &["same"])
            .content(sdrkit_core::ContentKind::Pictures, &["ssdv"])
            .content(
                sdrkit_core::ContentKind::Science,
                &[
                    "time",
                    "eczas",
                    "mscatter",
                    "astro",
                    "iono",
                    "sferics",
                    "supersid",
                    "schumann",
                    "whistler",
                    "lightning_tdoa",
                    "jove",
                    "callisto",
                    "riometer",
                    "eme_echo",
                    "beacon_events",
                    "pulsar",
                    "radar_detect",
                ],
            )
            .audio(sdrkit_core::AudioKind::Programme, &["same"])
            .audio(sdrkit_core::AudioKind::Monitor, &["time", "eczas"])
            .pictures(&["ssdv", "iono"])
            .text(&["same"])
            .stack(
                StackShape::new("sonde")
                    .fields::<SondeFields>()
                    .table(&SONDE_SONDES)
                    .table(&SONDE_CARRIERS)
                    .series(SONDE_ALT_TIME)
                    .series(SONDE_TEMP_ALT)
                    .series(SONDE_TRACK)
                    .points("Radiosondes")
                    .event(&SONDE_FRAME)
                    .settings(crate::sonde::SondeSetting::SETTINGS)
                    .command(objects_clear)
                    .command(&SONDE_REMOVE)
                    .command(&SONDE_SELECT),
            )
            .stack(
                TIME_SERIES
                    .into_iter()
                    .fold(
                        StackShape::new("time")
                            .fields::<TimeFields>()
                            .table(&TIME_MINUTES),
                        StackShape::series,
                    )
                    .event(&TIME_MINUTE)
                    .settings(crate::time::TimeSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                ECZAS_SERIES
                    .into_iter()
                    .fold(
                        StackShape::new("eczas")
                            .fields::<EczasFields>()
                            .table(&ECZAS_FRAMES),
                        StackShape::series,
                    )
                    .event(&ECZAS_TIME)
                    .settings(crate::eczas::EczasSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new("same")
                    .fields::<SameFields>()
                    .table(&SAME_ALERTS)
                    .table(&SAME_ALERT_LOCATIONS)
                    .table(&SAME_TONES)
                    .points("Alert areas")
                    .event(&SAME_ALERT)
                    .event(&SAME_END)
                    .settings(crate::same::SameSetting::SETTINGS)
                    .command(objects_clear)
                    .command(&crate::same::CLEAR_COUNTIES),
            )
            .stack(
                StackShape::new("mscatter")
                    .fields::<MscatterFields>()
                    .table(&MSCATTER_PINGS)
                    .table(&MSCATTER_SHOWERS)
                    .series(MSCATTER_HOURLY)
                    .series(MSCATTER_DAILY)
                    .series(MSCATTER_AROUND)
                    .event(&MS_PING)
                    .settings(crate::mscatter::MscatterSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new("ssdv")
                    .fields::<SsdvFields>()
                    .table(&SSDV_PACKETS)
                    .table(&SSDV_PICTURES)
                    .settings(crate::ssdv::SsdvSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new("astro")
                    .fields::<AstroFields>()
                    .table(&ASTRO_CALIBRATED)
                    .table(&ASTRO_SESSION)
                    .table(&ASTRO_FIT)
                    .table(&ASTRO_LV_ROWS)
                    .series(ASTRO_SPECTRUM)
                    .series(ASTRO_TOTAL_POWER)
                    .series(ASTRO_DRIFT)
                    .raster(ASTRO_LV_MAP)
                    .settings(crate::astro::AstroSetting::SETTINGS)
                    .command(objects_clear)
                    .command(&ASTRO_CAPTURE)
                    .command(&crate::astro::FLAG_RFI)
                    .command(&crate::astro::FIT_LINES)
                    .command(&crate::astro::ADD_MAP)
                    .command(&crate::astro::CLEAR_MAP)
                    .command(&crate::astro::RESTART),
            )
            .stack(
                IONO_HISTORY
                    .into_iter()
                    .fold(
                        StackShape::new("iono")
                            .fields::<IonoFields>()
                            .table(&IONO_SOUNDERS)
                            .table(&IONO_TIMELINE)
                            .table(&IONO_BANDS)
                            .series(IONO_COVERAGE),
                        StackShape::series,
                    )
                    .settings(crate::iono::IonoSetting::SETTINGS)
                    .command(objects_clear)
                    .command(&IONO_SELECT)
                    .command(&crate::iono::SELECT_LATEST),
            )
            .stack(
                StackShape::new(crate::sferics::ID)
                    .fields::<SfericsFields>()
                    .fields::<DecoderFields>()
                    .table(&SFERICS_STRIKES)
                    .series(SFERICS_PER_MINUTE)
                    .event(&SFERICS_STORM)
                    .settings(crate::sferics::SfericsSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::sferics::RESET),
            )
            .stack(
                SUPERSID_MINUTES
                    .into_iter()
                    .fold(
                        StackShape::new(crate::supersid::ID)
                            .fields::<SupersidFields>()
                            .fields::<DecoderFields>()
                            .table(&SUPERSID_TRANSMITTERS)
                            .table(&SUPERSID_EVENTS)
                            .table(&SUPERSID_MARKS),
                        StackShape::series,
                    )
                    .event(&SID_EVENT)
                    .settings(crate::supersid::SupersidSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::supersid::RESET),
            )
            .stack(
                StackShape::new(crate::schumann::ID)
                    .fields::<SchumannFields>()
                    .fields::<DecoderFields>()
                    .table(&SCHUMANN_PEAKS)
                    .table(&SCIENCE_EVENTS)
                    .table(&SCHUMANN_TREND)
                    .series(crate::schumann::AVERAGE)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::schumann::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::schumann::SchumannSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::schumann::RESET),
            )
            .stack(
                StackShape::new(crate::whistler::ID)
                    .fields::<WhistlerFields>()
                    .fields::<DecoderFields>()
                    .table(&SCIENCE_EVENTS)
                    .table(&WHISTLER_TREND)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::whistler::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::whistler::WhistlerSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::whistler::RESET),
            )
            .stack(
                StackShape::new(crate::lightning::ID)
                    .fields::<LightningTdoaFields>()
                    .fields::<DecoderFields>()
                    .table(&LIGHTNING_TDOA_STRIKES)
                    .table(&LIGHTNING_TDOA_STATIONS)
                    .points("Lightning strikes")
                    .event(&LIGHTNING_STRIKE)
                    .settings(crate::lightning::TdoaSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::lightning::RESET)
                    .command(&LIGHTNING_TDOA_DATAGRAM)
                    .command(&LIGHTNING_TDOA_TIME_MARK),
            )
            .stack(
                StackShape::new(crate::bursts::JOVE_PROFILE.id)
                    .fields::<BurstFields>()
                    .fields::<DecoderFields>()
                    .table(&SCIENCE_EVENTS)
                    .table(&BURST_TREND)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::bursts::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::bursts::JoveSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::bursts::RESET),
            )
            .stack(
                StackShape::new(crate::bursts::CALLISTO_PROFILE.id)
                    .fields::<BurstFields>()
                    .fields::<DecoderFields>()
                    .table(&SCIENCE_EVENTS)
                    .table(&BURST_TREND)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::bursts::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::bursts::CallistoSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::bursts::RESET),
            )
            .stack(
                StackShape::new(crate::riometer::ID)
                    .fields::<RiometerFields>()
                    .fields::<DecoderFields>()
                    .series(crate::riometer::ABSORPTION_SERIES)
                    .series(crate::riometer::POWER_SERIES)
                    .event(&RIOMETER_ABSORPTION)
                    .settings(crate::riometer::RiometerSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::riometer::RESET)
                    .command(&RIOMETER_EXPORT_QDC)
                    .command(&RIOMETER_IMPORT_QDC),
            )
            .stack(
                StackShape::new(crate::eme::ID)
                    .fields::<EmeEchoFields>()
                    .fields::<DecoderFields>()
                    .table(&SCIENCE_EVENTS)
                    .table(&EME_ECHO_TREND)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::eme::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::eme::EmeSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::eme::RESET),
            )
            .stack(
                StackShape::new(crate::beacon::ID)
                    .fields::<BeaconEventsFields>()
                    .fields::<DecoderFields>()
                    .table(&SCIENCE_EVENTS)
                    .table(&BEACON_EVENTS_TREND)
                    .series(crate::events::HOURS_SERIES)
                    .raster(crate::beacon::SPECTROGRAM)
                    .event(&SCIENCE_EVENT)
                    .settings(crate::beacon::BeaconSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::beacon::RESET),
            )
            .stack(
                StackShape::new(crate::pulsar::ID)
                    .fields::<PulsarFields>()
                    .fields::<DecoderFields>()
                    .table(&PULSAR_SEARCHES)
                    .series(crate::pulsar::PROFILE)
                    .series(crate::pulsar::DM_CURVE)
                    .series(crate::pulsar::PERIOD_CURVE)
                    .event(&PULSAR_DETECTION)
                    .settings(crate::pulsar::PulsarSetting::SETTINGS)
                    .command(decoder_clear)
                    .command(&crate::pulsar::SEARCH_NOW),
            )
            .stack(
                StackShape::new(crate::radar::ID)
                    .fields::<RadarDetectFields>()
                    .fields::<DecoderFields>()
                    .table(&RADAR_DETECT_ROWS)
                    .event(&RADAR_ROW)
                    .settings(crate::radar::RadarSetting::SETTINGS)
                    .command(decoder_clear),
            )
            // The channels and their measurements are of one frequency; the payloads, the custom fields and the packet
            // log are what the stack heard, and stay.
            .stack(
                StackShape::new(crate::horus::ID)
                    .fields::<HorusFields>()
                    .fields::<HorusLogFields>()
                    .table(&HORUS_PAYLOADS)
                    .table(&HORUS_CUSTOM_FIELDS)
                    .table(&HORUS_PACKETS)
                    .table(&HORUS_CHANNELS)
                    .series(HORUS_SPECTRUM)
                    .points("Horus balloons")
                    .event(&HORUS_PACKET)
                    .event(&HORUS_CHANNEL_OPENED)
                    .event(&HORUS_CHANNEL_CLOSED)
                    .settings(crate::horus::HorusSetting::SETTINGS)
                    .command(objects_clear)
                    .kept(&["payloads", "custom_fields", "packets", "HorusLogFields"]),
            )
    })
}

/// The description of stack `id` of the family.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the science family does not describe the stack {id}"))
}
