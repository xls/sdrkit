//! The interface of the GNSS family: the shapes of the result sets, the table rows and the events, the tables, the event
//! types and what each stack declares.

use crate::monitor::{EventLevel, InterferenceStatus};
use crate::navic::NavicSetting;
use crate::results::{EphemerisState, FrequencySource, OrbitSource};
use crate::settings::{GnssSetting, SbasSetting};
use crate::system::{SbasService, System};
use crate::track::ChannelState;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_graph::{DecoderFields, GuardFields};
use std::sync::OnceLock;

/// The result fields of the receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GnssFields<'a> {
    /// The centre frequency of the input.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The sample rate of the input.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate: Hertz,
    /// The system that the search works on now.
    #[field(label = "Search system")]
    pub search_system: Option<System>,
    /// The PRN that the search works on now.
    #[field(label = "Search PRN")]
    pub search_prn: Option<i64>,
    /// The part of the search round done.
    #[field(label = "Search progress", unit = "%")]
    pub search_progress_pct: Option<f64>,
    /// The number of the search round, from 1.
    #[field(label = "Search round")]
    pub search_round: Option<i64>,
    /// The centre of the search window (the radio's frequency error at L1).
    #[field(label = "Search centre", unit = "Hz")]
    pub search_centre: Option<Hertz>,
    /// The half width of the search window.
    #[field(label = "Search half width", unit = "Hz")]
    pub search_half_width: Option<Hertz>,
    /// The integration of the search.
    #[field(label = "Search integration", unit = "ms")]
    pub search_integration_ms: Option<f64>,
    /// The radio's frequency error at L1, measured by the fix.
    #[field(label = "Frequency error", unit = "Hz")]
    pub frequency_error: Option<Hertz>,
    /// The same error in parts per million of 1575.42 MHz.
    #[field(label = "Frequency error", unit = "ppm")]
    pub frequency_error_ppm: Option<f64>,
    /// Where the frequency error comes from.
    #[field(label = "Frequency error source")]
    pub frequency_source: FrequencySource,
    /// Signal time of the first lock.
    #[field(label = "First lock", unit = "s")]
    pub first_lock: Option<Seconds>,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// SBAS messages with a good CRC.
    #[field(label = "SBAS messages")]
    pub sbas_messages: i64,
    /// GLONASS time minus GPS time.
    #[field(label = "GLONASS tau GPS", unit = "s")]
    pub glonass_tau_gps_s: Option<f64>,
    /// GLONASS time minus UTC(SU).
    #[field(label = "GLONASS tau c", unit = "s")]
    pub glonass_tau_c_s: Option<f64>,
    /// BeiDou time minus GPS time, less 14 s.
    #[field(label = "BDT - GPS (less 14 s)", unit = "s")]
    pub bdt_gps_s: Option<f64>,
    /// GPS satellites with an almanac entry.
    #[field(label = "Almanac")]
    pub almanac_satellites: i64,
    /// Whether the ionosphere model comes from the satellites.
    #[field(label = "Ionosphere from the satellites")]
    pub iono_broadcast: bool,
    /// GPS time minus UTC: broadcast, else from the station's leap-second table.
    #[field(label = "Leap seconds", unit = "s")]
    pub leap_seconds: Option<i64>,
    /// Whether the satellites broadcast the leap seconds.
    #[field(label = "Leap seconds from the satellites")]
    pub leap_seconds_broadcast: bool,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Input samples clipped.
    #[field(label = "Clipped", unit = "%")]
    pub clipped_pct: f64,
    /// Input DC, in phase (full scale 1).
    #[field(label = "DC I")]
    pub dc_i: f64,
    /// Input DC, quadrature (full scale 1).
    #[field(label = "DC Q")]
    pub dc_q: f64,
    /// Signal time fed.
    #[field(label = "Signal time", unit = "s")]
    pub signal: Seconds,
    /// What the receiver waits for while it has no fix, for people, one item a line.
    #[field(label = "Progress")]
    pub progress: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// Horizontal error, 1 sigma.
    #[field(label = "Horizontal error", unit = "m")]
    pub error_h_m: Option<f64>,
    /// Vertical error, 1 sigma.
    #[field(label = "Vertical error", unit = "m")]
    pub error_v_m: Option<f64>,
    /// Horizontal dilution of precision.
    #[field(label = "HDOP")]
    pub hdop: Option<f64>,
    /// Vertical dilution of precision.
    #[field(label = "VDOP")]
    pub vdop: Option<f64>,
    /// Position dilution of precision.
    #[field(label = "PDOP")]
    pub pdop: Option<f64>,
    /// Satellites in the fix.
    #[field(label = "Satellites used")]
    pub satellites_used: Option<i64>,
    /// Satellites tracked.
    #[field(label = "Satellites tracked")]
    pub satellites_tracked: i64,
    /// UTC of the fix.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// GPS week of the fix.
    #[field(label = "GPS week")]
    pub gps_week: Option<i64>,
    /// GPS seconds of the week of the fix.
    #[field(label = "GPS time of week", unit = "s")]
    pub gps_tow_s: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The receiver's clock bias.
    #[field(label = "Clock bias", unit = "m")]
    pub clock_bias_m: Option<f64>,
    /// Signal time of the first fix.
    #[field(label = "First fix", unit = "s")]
    pub first_fix: Option<Seconds>,
}

/// The result fields of the interference monitor.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InterferenceFields<'a> {
    /// The monitor's verdict.
    #[field(label = "Interference")]
    pub interference: InterferenceStatus,
    /// A tone that sweeps through the band.
    #[field(label = "Swept")]
    pub swept: bool,
    /// Pulses.
    #[field(label = "Pulsed")]
    pub pulsed: bool,
    /// A narrow line that the receiver does not know.
    #[field(label = "Narrowband")]
    pub narrowband: bool,
    /// A raised noise floor.
    #[field(label = "Wideband")]
    pub wideband: bool,
    /// Why the monitor says so, for people.
    #[field(label = "Reason")]
    pub interference_reason: &'a str,
    /// The noise floor, dBFS in 1 Hz.
    #[field(label = "Noise floor", unit = "dBFS/Hz")]
    pub noise_floor_dbfs: f64,
    /// The band power.
    #[field(label = "Band power", unit = "dBFS")]
    pub band_power_dbfs: f64,
    /// Spectral flatness, 0 to 1.
    #[field(label = "Flatness")]
    pub flatness: f64,
    /// Whether spoofing is suspected.
    #[field(label = "Spoofing")]
    pub spoofing: bool,
    /// Why spoofing is suspected, for people.
    #[field(label = "Spoofing reason")]
    pub spoofing_reason: Option<&'a str>,
}

/// A satellite system. The row key is the key of the system.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GnssSystemRow {
    /// A satellite system.
    #[field(label = "System")]
    pub system: System,
    /// This system is selected for reception.
    #[field(label = "Wanted")]
    pub wanted: bool,
    /// The receiver has a band for this system at the present tune.
    #[field(label = "Active")]
    pub active: bool,
    /// The receiver has searched or tracks this system.
    #[field(label = "Searched")]
    pub searched: bool,
}

/// A satellite system in the fix. The row key is the key of the system.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GnssFixSystemRow {
    /// A satellite system.
    #[field(label = "System")]
    pub system: System,
    /// Satellites of a system in the fix.
    #[field(label = "Satellites")]
    pub satellites: i64,
    /// The receiver clock of a system in the fix.
    #[field(label = "Clock", unit = "m")]
    pub clock_m: f64,
}

/// A satellite: what the channels, the navigation data, the sky and the orbits say of it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GnssSatelliteRow {
    /// A satellite system.
    #[field(label = "System")]
    pub system: Option<System>,
    /// A PRN or slot number.
    #[field(label = "PRN")]
    pub prn: Option<i64>,
    /// The frequency channel of a GLONASS satellite.
    #[field(label = "Channel")]
    pub glo_channel: Option<i64>,
    /// A channel tracks the satellite.
    #[field(label = "Tracked")]
    pub tracked: bool,
    /// The state of the satellite's channel.
    #[field(label = "State")]
    pub state: Option<ChannelState>,
    /// Carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
    /// Doppler at the signal's carrier, with the radio's error.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Option<Hertz>,
    /// Code phase.
    #[field(label = "Code", unit = "chips")]
    pub code_chips: Option<f64>,
    /// Azimuth, degrees from true north.
    #[field(label = "Az", unit = "°")]
    pub azimuth_deg: Option<f64>,
    /// Elevation.
    #[field(label = "El", unit = "°")]
    pub elevation_deg: Option<f64>,
    /// Time since lock was gained, seconds.
    #[field(shared = keys::LOCKED_FOR)]
    pub locked_for: Option<Seconds>,
    /// Frames that passed their check.
    #[field(label = "Frames good")]
    pub frames_good: Option<i64>,
    /// Frames received.
    #[field(label = "Frames")]
    pub frames: Option<i64>,
    /// The residual of the satellite in the fix.
    #[field(label = "Residual", unit = "m")]
    pub residual_m: Option<f64>,
    /// The satellite says it is healthy.
    #[field(label = "Healthy")]
    pub healthy: Option<bool>,
    /// The satellite is in the fix.
    #[field(label = "Used")]
    pub used: bool,
    /// What the receiver holds of the ephemeris.
    #[field(label = "Ephemeris")]
    pub ephemeris: Option<EphemerisState>,
    /// Ephemeris parts collected.
    #[field(label = "Parts")]
    pub parts: Option<i64>,
    /// Ephemeris parts needed.
    #[field(label = "Parts needed")]
    pub parts_of: Option<i64>,
    /// About how long until the ephemeris is complete.
    #[field(label = "Time left", unit = "s")]
    pub seconds_left: Option<Seconds>,
    /// The issue of data (IODE, IODnav, AODE or the GLONASS tb index).
    #[field(label = "IOD")]
    pub iod: Option<i64>,
    /// The age of the data from its epoch (negative: ahead).
    #[field(label = "Data age", unit = "s")]
    pub data_age: Option<Seconds>,
    /// The week of the data.
    #[field(label = "Week")]
    pub week: Option<i64>,
    /// The time of week of the last unit.
    #[field(label = "TOW", unit = "s")]
    pub tow_s: Option<f64>,
    /// The satellite clock af0.
    #[field(label = "af0", unit = "s")]
    pub af0_s: Option<f64>,
    /// The point below the satellite.
    #[field(label = "Sub-point", unit = "°")]
    pub sub_point: Option<(f64, f64)>,
    /// The height of the satellite.
    #[field(label = "Height", unit = "km")]
    pub height_km: Option<f64>,
    /// Where the satellite's position comes from.
    #[field(label = "Orbit from")]
    pub orbit: Option<OrbitSource>,
}

/// A line of the interference spectrum.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InterferenceLineRow {
    /// The offset of a line from the centre.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// A level above the floor.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// A line that the receiver has identified (a satellite or the radio's own).
    #[field(label = "Known")]
    pub known: bool,
}

/// An interference event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InterferenceEventRow<'a> {
    /// Signal time of an event.
    #[field(label = "Signal time", unit = "s")]
    pub at: Seconds,
    /// The level of an event.
    #[field(label = "Level")]
    pub level: EventLevel,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A fix.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GnssFixEvent {
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The height of the object that the event is about, metres above mean sea level.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: f64,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// Satellites in the fix.
    #[field(label = "Satellites used")]
    pub satellites_used: i64,
}

/// The result fields of NavIC.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NavicFields<'a> {
    /// 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
    #[field(label = "State")]
    pub state: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The PRN being searched.
    #[field(label = "Searching PRN")]
    pub searching_prn: i64,
    /// What is not confirmed by a public document.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A NavIC satellite.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NavicSatelliteRow {
    /// The PRN.
    #[field(label = "PRN")]
    pub prn: i64,
    /// 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
    #[field(label = "State")]
    pub state: i64,
    /// C/N0.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: f64,
    /// Doppler.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler_hz: Hertz,
    /// Subframes good.
    #[field(label = "Subframes good")]
    pub subframes_ok: i64,
    /// Subframes bad.
    #[field(label = "Subframes bad")]
    pub subframes_bad: i64,
    /// The last subframe ID.
    #[field(label = "Subframe")]
    pub subframe_id: Option<i64>,
    /// The time of week count.
    #[field(label = "TOWC")]
    pub towc: Option<i64>,
    /// The week number.
    #[field(label = "Week")]
    pub wn: Option<i64>,
    /// The alert flag.
    #[field(label = "Alert")]
    pub alert: Option<bool>,
}

/// A NavIC subframe.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NavicSubframeEvent {
    /// The PRN.
    #[field(label = "PRN")]
    pub prn: i64,
    /// The last subframe ID.
    #[field(label = "Subframe")]
    pub subframe_id: i64,
    /// The time of week count.
    #[field(label = "TOWC")]
    pub towc: i64,
    /// The week number.
    #[field(label = "Week")]
    pub wn: Option<i64>,
    /// The message ID.
    #[field(label = "Message ID")]
    pub message_id: Option<i64>,
    /// The PRN ID.
    #[field(label = "PRN ID")]
    pub prn_id: Option<i64>,
    /// The alert flag.
    #[field(label = "Alert")]
    pub alert: bool,
    /// The autonav flag.
    #[field(label = "Autonav")]
    pub autonav: bool,
}

/// The result fields of the SBAS content decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasFields<'a> {
    /// A PRN mask and fast corrections are held.
    #[field(label = "Corrections held")]
    pub data_valid: bool,
    /// The SBAS messages with a good CRC.
    #[field(label = "SBAS messages")]
    pub sbas_messages: i64,
    /// The messages missed.
    #[field(label = "Messages missed")]
    pub messages_missed: i64,
    /// Help for people while nothing is heard.
    #[field(label = "Help")]
    pub help: Option<&'a str>,
    /// The PRN whose corrections the tables show.
    #[field(label = "Selected PRN")]
    pub selected: Option<i64>,
    /// GEO navigation of the selected PRN: the issue of data.
    #[field(label = "GEO IODN")]
    pub geo_iodn: Option<i64>,
    /// GEO navigation: the time of applicability.
    #[field(label = "GEO t0", unit = "s")]
    pub geo_t0_s: Option<f64>,
    /// GEO navigation: the accuracy index.
    #[field(label = "GEO URA")]
    pub geo_ura: Option<i64>,
    /// GEO navigation: the distance from the centre of the earth.
    #[field(label = "GEO radius", unit = "m")]
    pub geo_radius_m: Option<f64>,
    /// GEO navigation: the longitude.
    #[field(label = "GEO longitude", unit = "°")]
    pub geo_longitude_deg: Option<f64>,
    /// Network time of the selected PRN: the week.
    #[field(label = "Network week")]
    pub sbas_week: Option<i64>,
    /// Network time: the time of week.
    #[field(label = "Network TOW", unit = "s")]
    pub sbas_tow_s: Option<i64>,
    /// Network time: the leap seconds.
    #[field(label = "Leap seconds", unit = "s")]
    pub sbas_leap_s: Option<i64>,
    /// Network time: the leap seconds after the next change.
    #[field(label = "Next leap seconds", unit = "s")]
    pub sbas_leap_next_s: Option<i64>,
    /// Network time: the UTC offset A0.
    #[field(label = "A0", unit = "s")]
    pub utc_a0_s: Option<f64>,
    /// Network time: the UTC drift A1.
    #[field(label = "A1", unit = "s/s")]
    pub utc_a1: Option<f64>,
    /// Network time: the identifier of the UTC standard.
    #[field(label = "UTC identifier")]
    pub utc_id: Option<i64>,
}

/// A row of the SBAS satellite table. The row key is the PRN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasSatelliteRow {
    /// The PRN.
    #[field(label = "PRN")]
    pub prn: i64,
    /// The service of the PRN.
    #[field(label = "Service")]
    pub service: Option<SbasService>,
    /// The state of the channel of the satellite.
    #[field(label = "State")]
    pub state: Option<ChannelState>,
    /// The carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: f64,
    /// The Doppler at the carrier, with the error of the radio.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Hertz,
    /// The messages received.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The messages missed.
    #[field(label = "Messages missed")]
    pub messages_missed: i64,
}

/// A message type that an SBAS satellite sent. The row key is the PRN and the type, as `123/9`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasSatelliteTypeRow {
    /// The SBAS satellite.
    #[field(label = "PRN", refers_to = "sbas_satellites")]
    pub prn: i64,
    /// The message type.
    #[field(label = "Type")]
    pub message_type: i64,
}

/// The count of one message type. The row key is the type.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasTypeRow<'a> {
    /// The message type.
    #[field(label = "Type")]
    pub message_type: i64,
    /// The name of the type.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The messages of the type.
    #[field(label = "Count")]
    pub count: i64,
}

/// A row of the message log. The row key is the number of the message since the start.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasMessageRow<'a> {
    /// The signal time of the message.
    #[field(label = "Signal time", unit = "s")]
    pub at: Seconds,
    /// The SBAS satellite.
    #[field(label = "PRN", refers_to = "sbas_satellites")]
    pub prn: i64,
    /// The message type.
    #[field(label = "Type")]
    pub message_type: i64,
    /// The content in words.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A fast correction of the selected PRN. The row key is the slot.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasFastCorrectionRow {
    /// The slot of the PRN mask.
    #[field(label = "Slot")]
    pub slot: i64,
    /// The system of the corrected satellite.
    #[field(label = "System")]
    pub system: Option<System>,
    /// The PRN of the corrected satellite.
    #[field(label = "PRN")]
    pub prn: Option<i64>,
    /// The fast correction.
    #[field(label = "Correction", unit = "m")]
    pub correction_m: Option<f64>,
    /// The UDRE indicator.
    #[field(label = "UDREI")]
    pub udrei: i64,
    /// The UDRE as one sigma. No value for "not monitored" and "do not use".
    #[field(label = "UDRE sigma", unit = "m")]
    pub udre_sigma_m: Option<f64>,
    /// The issue of data of the fast corrections.
    #[field(label = "IODF")]
    pub iodf: i64,
    /// The issue of data of the PRN mask.
    #[field(label = "IODP")]
    pub iodp: i64,
    /// The correction belongs to the PRN mask held.
    #[field(label = "Mask held")]
    pub iodp_current: bool,
    /// The degradation factor.
    #[field(label = "Degradation", unit = "m/s²")]
    pub degradation: Option<f64>,
}

/// An ionosphere band of the selected PRN. The row key is the band.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbasIonosphereRow {
    /// The band.
    #[field(label = "Band")]
    pub band: i64,
    /// The issue of data of the ionosphere.
    #[field(label = "IODI")]
    pub iodi: i64,
    /// The grid points.
    #[field(label = "Points")]
    pub points: i64,
    /// The blocks seen.
    #[field(label = "Blocks seen")]
    pub blocks_seen: i64,
    /// The blocks needed.
    #[field(label = "Blocks needed")]
    pub blocks_needed: i64,
    /// The delays held.
    #[field(label = "Values")]
    pub values: i64,
    /// The points not monitored.
    #[field(label = "Not monitored")]
    pub not_monitored: i64,
    /// The smallest vertical delay.
    #[field(label = "Min", unit = "m")]
    pub min_m: f64,
    /// The mean vertical delay.
    #[field(label = "Mean", unit = "m")]
    pub mean_m: f64,
    /// The largest vertical delay.
    #[field(label = "Max", unit = "m")]
    pub max_m: f64,
    /// The worst GIVE indicator.
    #[field(label = "Worst GIVEI")]
    pub worst_givei: i64,
}

/// The SBAS satellites.
pub static SBAS_SATELLITES: Table<SbasSatelliteRow> =
    Table::new("sbas_satellites", "SBAS satellites", 39);
/// The message types that each SBAS satellite sent.
pub static SBAS_SATELLITE_TYPES: Table<SbasSatelliteTypeRow> =
    Table::new("sbas_satellite_types", "Types by satellite", 39 * 64);
/// The SBAS messages by type.
pub static SBAS_TYPES: Table<SbasTypeRow<'static>> = Table::new("types", "Types", 64);
/// The newest SBAS messages.
pub static SBAS_MESSAGES: Table<SbasMessageRow<'static>> = Table::new("messages", "Messages", 100);
/// The fast corrections of the selected PRN.
pub static SBAS_FAST_CORRECTIONS: Table<SbasFastCorrectionRow> =
    Table::new("fast_corrections", "Fast corrections", 51);
/// The ionosphere bands of the selected PRN.
pub static SBAS_IONOSPHERE: Table<SbasIonosphereRow> = Table::new("ionosphere", "Ionosphere", 11);

/// The row key of a row of the SBAS satellite table: the PRN.
pub const SBAS_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of("sbas_satellites");

/// The arguments of `select` of the `sbas` stack.
#[derive(Arguments)]
pub struct SbasSelectArgs<'a> {
    /// The row key of the SBAS satellite: its PRN.
    #[arg(shared = SBAS_ROW)]
    pub row: &'a str,
}

/// Shows the corrections of one SBAS satellite.
pub const SBAS_SELECT: CommandKind<SbasSelectArgs<'static>> =
    CommandKind::new("select", "Select").help("Shows the corrections of this SBAS satellite.");

/// An NMEA 0183 sentence.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NmeaEvent<'a> {
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The sentence.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The satellite systems.
pub static SYSTEMS: Table<GnssSystemRow> = Table::new("systems", "Systems", 6);
/// The satellite systems in the fix.
pub static FIX_SYSTEMS: Table<GnssFixSystemRow> =
    Table::new("fix_systems", "Systems in the fix", 6);
/// The satellites.
pub static SATELLITES: Table<GnssSatelliteRow> = Table::new("satellites", "Satellites", 160);
/// The lines of the interference spectrum.
pub static INTERFERENCE_LINES: Table<InterferenceLineRow> =
    Table::new("interference_lines", "Interference lines", 16);
/// The interference events.
pub static INTERFERENCE_EVENTS: Table<InterferenceEventRow<'static>> =
    Table::new("interference_events", "Interference events", 200);
/// A fix.
pub static FIX: EventKind<GnssFixEvent> = EventKind::new("gnss.fix", "Fix");
/// An NMEA 0183 sentence.
pub static NMEA: EventKind<NmeaEvent<'static>> = EventKind::new("text.nmea", "NMEA sentence");

/// The NavIC satellites.
pub static NAVIC_SATELLITES: Table<NavicSatelliteRow> =
    Table::new("satellites", "Satellites", 14);
/// A NavIC subframe.
pub static NAVIC_SUBFRAME: EventKind<NavicSubframeEvent> =
    EventKind::new("navic.subframe", "NavIC subframe");

/// The series of the receiver: key, label, unit of x and unit of y.
pub const RECEIVER_SERIES: [(&str, &str, &str, &str); 5] = [
    ("acquisition", "Search correlation", "", ""),
    ("scatter", "Prompt scatter", "", ""),
    ("level", "Input level", "s", "dBFS"),
    ("best_cn0", "Best C/N0", "s", "dB-Hz"),
    ("clock_offset", "Clock offset", "s", "ms"),
];
/// The series of the interference monitor: key, label, unit of x and unit of y.
pub const MONITOR_SERIES: [(&str, &str, &str, &str); 2] = [
    ("spectrum", "Band spectrum", "bin", "dB"),
    ("history", "Noise floor", "s", "dBFS/Hz"),
];

/// `shape` with the series `series`.
fn with_series(mut shape: StackShape, series: &[(&'static str, &'static str, &'static str, &'static str)]) -> StackShape {
    for &(key, label, x_unit, y_unit) in series {
        shape = shape.series(SeriesDef {
            key,
            label,
            x_unit,
            y_unit,
        });
    }
    shape
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The receiver forgets its channels and searches over a change of frequency by its own `reset`. The interference
        // monitor measures the band continuously and has no `clear`.
        let gnss = StackShape::new("gnss")
            .fields::<GnssFields>()
            .fields::<InterferenceFields>()
            .receiver_fields::<GuardFields>()
            .table(&SYSTEMS)
            .table(&FIX_SYSTEMS)
            .table(&SATELLITES)
            .table(&INTERFERENCE_LINES)
            .table(&INTERFERENCE_EVENTS);
        let gnss = with_series(with_series(gnss, &RECEIVER_SERIES), &MONITOR_SERIES)
            .points("Satellites")
            .event(&FIX)
            .event(&NMEA)
            .settings(GnssSetting::SETTINGS)
            .command(&crate::components::RESET);
        // The SBAS content decoder is the status component. The receiver `rx` shows its results under its name, and
        // its count of SBAS messages is the field `sbas_messages` of the stack, which the content decoder writes too.
        // The message log and the counts of the messages are kept over a change of frequency.
        let sbas = StackShape::new("sbas")
            .fields::<SbasFields>()
            .fields::<InterferenceFields>()
            .receiver_fields::<GuardFields>()
            .table(&SBAS_SATELLITES)
            .table(&SBAS_SATELLITE_TYPES)
            .table(&SBAS_TYPES)
            .table(&SBAS_MESSAGES)
            .table(&SBAS_FAST_CORRECTIONS)
            .table(&SBAS_IONOSPHERE)
            .table(&INTERFERENCE_LINES)
            .table(&INTERFERENCE_EVENTS);
        let sbas = with_series(sbas, &MONITOR_SERIES)
            .component("rx", |mut c| {
                c = c
                    .fields::<GnssFields>()
                    .shares(&["sbas_messages"])
                    .table(&SYSTEMS)
                    .table(&FIX_SYSTEMS)
                    .table(&SATELLITES);
                for (key, label, x_unit, y_unit) in RECEIVER_SERIES {
                    c = c.series(SeriesDef {
                        key,
                        label,
                        x_unit,
                        y_unit,
                    });
                }
                c
            })
            .settings(SbasSetting::SETTINGS)
            .command(&crate::components::RESET)
            .command(&SBAS_SELECT)
            .command(&crate::components::CLEAR)
            .kept(&[
                "messages",
                "types",
                "sbas_satellite_types",
                "SbasFields.sbas_messages",
                "SbasFields.messages_missed",
            ]);
        Described::new("gnss", 1, 0).stack(gnss).stack(sbas).stack(
            StackShape::new("navic")
                .fields::<NavicFields>()
                .fields::<DecoderFields>()
                .table(&NAVIC_SATELLITES)
                .event(&NAVIC_SUBFRAME)
                .settings(NavicSetting::SETTINGS)
                .command(&sdrkit_graph::decoder::CLEAR),
        )
    })
}

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the gnss family does not describe the stack {id}"))
}
