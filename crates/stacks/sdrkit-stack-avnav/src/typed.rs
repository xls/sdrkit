//! The interface of the navigation-aid family: the shapes of the result sets, the table rows and the events, the tables,
//! the event types and what each stack declares.

use crate::dgps::StationHealth;
use crate::dme::Heard;
use crate::ils::{IlsCommand, IlsPart};
use crate::marker::{MarkerKind, MarkerPattern, PassageKind};
use crate::morse::IdentTracker;
use crate::pairing::DmeMode;
use crate::vor::ToFrom;
use sdrkit_core::interface::{
    EventKind, Family as Described, Fields, Hertz, Seconds, Shape, StackShape, Table,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{Declare, Published, TableDef};
use sdrkit_core::SettingSet;
use std::sync::OnceLock;

// What several stacks share.

/// The tuned frequency.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TunedFields {
    /// The tuned frequency.
    #[field(label = "Tuned", unit = "Hz")]
    pub tuned_hz: Hertz,
}

/// The identification of the station.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IdentFields<'a> {
    /// The confirmed identification. No value before one is confirmed.
    #[field(label = "Identification", reading = 1)]
    pub ident: Option<&'a str>,
    /// The last identification heard. No value before a word is heard.
    #[field(label = "Last heard")]
    pub last_ident: Option<&'a str>,
    /// The Morse speed of the last identification. No value before a word is heard.
    #[field(label = "Morse speed", unit = "wpm")]
    pub ident_wpm: Option<f64>,
}

/// The carrier of the station.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CarrierFields {
    /// A carrier stands clear of the noise.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// The frequency of the carrier. No value before the first measurement.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Option<Hertz>,
    /// The carrier to noise density. No value before the first measurement.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
    /// The offset of the carrier from the tuned frequency. No value before the first measurement.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the carrier. No value before the first measurement.
    #[field(shared = keys::LEVEL_DBFS, reading = 4)]
    pub level_dbfs: Option<f64>,
}

/// The identification tone that the tone keyer listens to.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KeyerFields {
    /// The tone: the setting, or the tone found. No value before the first input.
    #[field(label = "Identification tone", unit = "Hz")]
    pub id_tone_hz: Option<Hertz>,
    /// The modulation depth of the tone. No value before the first input.
    #[field(label = "Identification depth", unit = "%")]
    pub id_depth_pct: Option<f64>,
}

/// A row of the identifications heard. The row key is the word.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IdentRow<'a> {
    /// The word.
    #[field(label = "Identification")]
    pub ident: &'a str,
    /// How often it was heard.
    #[field(label = "Heard")]
    pub heard: i64,
}

/// Every word heard as an identification, with how often.
pub static IDENTS: Table<IdentRow<'static>> =
    Table::new("idents", "Identifications heard", IdentTracker::CAP).label(&IdentRow::IDENT);

/// The fields of a decoded identification word: a spot of the station that sends it. The source, the text and the
/// callsign hold the word.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IdentWordEvent<'a> {
    /// The word.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The word.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: other.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The word.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// No value: a beacon sends no locator.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// No value: the decoder does not measure it.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// No value: the event carries the frequency.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Option<Hertz>,
    /// The emission: "CW".
    #[field(shared = keys::SPOT_MODE)]
    pub mode: &'a str,
    /// The Morse speed.
    #[field(label = "Morse speed", unit = "wpm")]
    pub wpm: f64,
}

/// A decoded identification word.
pub static IDENT_WORD: EventKind<IdentWordEvent<'static>> =
    EventKind::new("morse.word", "Identification word");

// --- NDB ---

/// The identification that the average of the repeats reads. No value while the average reads nothing.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NdbFields<'a> {
    /// The period of the identification.
    #[field(label = "Identification period", unit = "s", reading = 3)]
    pub ident_period_s: Option<f64>,
    /// The robust z-score of the autocorrelation at that period.
    #[field(label = "Period z-score")]
    pub ident_period_z: Option<f64>,
    /// The swing of the average of the repeats over its noise.
    #[field(label = "Average quality")]
    pub ident_fold_quality: Option<f64>,
    /// The word read from the average.
    #[field(label = "Read from the average", reading = 2)]
    pub ident_fold_text: Option<&'a str>,
}

/// A row of the band scan. The row key is the frequency in whole hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NdbScanRow {
    /// The frequency of the carrier.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The level of the carrier.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: f64,
}

/// The carriers of the last band scan.
pub static NDB_SCAN: Table<NdbScanRow> = Table::new("scan", "Beacons in the input", 64)
    .sorted(&NdbScanRow::CN0_DBHZ, sdrkit_core::SortOrder::Descending);

// --- VOR ---

/// The result fields of VOR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VorFields<'a> {
    /// The radial. No value without a valid bearing.
    #[field(label = "Radial", unit = "°", reading = 2)]
    pub radial_deg: Option<f64>,
    /// The bearing to the station: the radial plus 180°. No value without a valid bearing.
    #[field(label = "Bearing to", unit = "°", reading = 3)]
    pub bearing_to_deg: Option<f64>,
    /// The selected course.
    #[field(label = "Course", unit = "°")]
    pub obs_deg: f64,
    /// TO, FROM, or OFF (the flag: no valid bearing).
    #[field(label = "To or from")]
    pub to_from: ToFrom,
    /// The course deviation, positive to fly right. No value without a valid bearing.
    #[field(label = "Course deviation", unit = "°")]
    pub cdi_deg: Option<f64>,
    /// The course deviation in dots. No value without a valid bearing.
    #[field(label = "Course deviation", unit = "dots")]
    pub cdi_dots: Option<f64>,
    /// The depth of the 30 Hz AM. No value before the first measurement.
    #[field(label = "30 Hz AM", unit = "%")]
    pub am30_pct: Option<f64>,
    /// The depth of the subcarrier. No value before the first measurement.
    #[field(label = "Subcarrier", unit = "%")]
    pub subcarrier_pct: Option<f64>,
    /// The peak deviation of the subcarrier. No value before the first measurement.
    #[field(label = "Subcarrier deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// How steady the bearing is, 0 (random) to 1. No value before the first measurement.
    #[field(label = "Steadiness", reading = 5)]
    pub steadiness: Option<f64>,
    /// Measurements made.
    #[field(label = "Measurements")]
    pub fixes: i64,
    /// The tuned frequency.
    #[field(label = "Tuned", unit = "Hz")]
    pub tuned_hz: Hertz,
    /// The DME channel paired with the frequency ("17X"). No value for a frequency without one.
    #[field(label = "DME channel")]
    pub dme_channel: Option<&'a str>,
}

// --- ILS ---

/// The result fields of ILS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IlsFields<'a> {
    /// Localizer or glide path.
    #[field(label = "Part")]
    pub part: IlsPart,
    /// The difference in depth of modulation (90 Hz less 150 Hz). No value while the flag is out.
    #[field(label = "DDM", reading = 2)]
    pub ddm: Option<f64>,
    /// The sum in depth of modulation. No value before the first measurement.
    #[field(label = "SDM", unit = "%")]
    pub sdm_pct: Option<f64>,
    /// The depth of the 90 Hz tone. No value before the first measurement.
    #[field(label = "90 Hz", unit = "%")]
    pub m90_pct: Option<f64>,
    /// The depth of the 150 Hz tone. No value before the first measurement.
    #[field(label = "150 Hz", unit = "%")]
    pub m150_pct: Option<f64>,
    /// The needle, positive to fly right or up, limited to 150 µA either way. No value while the flag is out.
    #[field(label = "Needle", unit = "µA")]
    pub needle_ua: Option<f64>,
    /// The needle in dots (75 µA a dot), limited to 2.5 either way. No value while the flag is out.
    #[field(label = "Needle", unit = "dots")]
    pub needle_dots: Option<f64>,
    /// What the needle says.
    #[field(label = "Command", reading = 3)]
    pub command: IlsCommand,
    /// The flag is out: no carrier, an SDM far from its nominal value, or the marker part, which has no needle.
    #[field(label = "Flag")]
    pub flag: bool,
    /// The paired localizer or glide path frequency. No value for a frequency without one.
    #[field(label = "Paired", unit = "Hz")]
    pub paired_hz: Option<Hertz>,
    /// The tuned frequency.
    #[field(label = "Tuned", unit = "Hz")]
    pub tuned_hz: Hertz,
    /// The DME channel paired with the localizer ("17X"). No value for a frequency without one.
    #[field(label = "DME channel")]
    pub dme_channel: Option<&'a str>,
}

/// The marker beacon (the part `marker`): the passage under way, or the last one, and the live indication.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MarkerFields<'a> {
    /// The marker of the passage, named by its tone. None before the first passage of three elements and outside the
    /// marker part.
    #[field(label = "Marker", reading = 5)]
    pub marker: MarkerKind,
    /// The tone of the passage, measured. No value without a passage.
    #[field(label = "Marker tone", unit = "Hz")]
    pub marker_tone_hz: Option<Hertz>,
    /// The modulation depth of the tone. No value without a passage.
    #[field(label = "Marker depth", unit = "%")]
    pub marker_depth_pct: Option<f64>,
    /// The keying as text ("2.0 dashes per second (0.30 s long)"). No value without a passage.
    #[field(label = "Keying")]
    pub keying: Option<&'a str>,
    /// Elements a second, from the first onset to the last. No value without a passage.
    #[field(label = "Keying rate", unit = "1/s")]
    pub keying_rate: Option<f64>,
    /// Dot-dash pairs a minute. No value without mixed keying.
    #[field(label = "Dot-dash pairs", unit = "1/min")]
    pub pairs_per_min: Option<f64>,
    /// The mean dot length. No value without dots.
    #[field(label = "Dot", unit = "s")]
    pub dot_s: Option<f64>,
    /// The mean dash length. No value without dashes.
    #[field(label = "Dash", unit = "s")]
    pub dash_s: Option<f64>,
    /// The keying fits the standard (ICAO Annex 10 Volume I, 3.1.7.5). No value without a passage.
    #[field(label = "Keying fits")]
    pub keying_fits: Option<bool>,
    /// The length of the passage. No value without a passage.
    #[field(label = "Passage", unit = "s")]
    pub passage_s: Option<f64>,
    /// Passages finished, those of fewer than three elements not counted.
    #[field(label = "Passages")]
    pub passages: i64,
    /// The tone with the greatest held depth keys the last period.
    #[field(label = "Keyed now")]
    pub keyed_now: bool,
    /// The held depth of that tone. No value outside the marker part.
    #[field(label = "Held depth", unit = "%")]
    pub held_depth_pct: Option<f64>,
    /// The pattern of the last on-lengths of that tone.
    #[field(label = "Pattern heard")]
    pub pattern: MarkerPattern,
}

/// A row of the marker passages. The row key is the number of the passage.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MarkerPassageRow<'a> {
    /// A marker, or a marker tone whose keying does not fit the standard.
    #[field(label = "Kind")]
    pub kind: PassageKind,
    /// The marker of the tone.
    #[field(label = "Marker")]
    pub marker: MarkerKind,
    /// The tone, the keying, the depth and the length ("400 Hz, 2.0 dashes per second (0.30 s long), depth 95 %, 7.8 s").
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The carrier level at the end of the passage. No value before the first carrier measurement.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The signal time of the end of the passage.
    #[field(label = "Time", unit = "s", role = Time)]
    pub time_s: f64,
}

/// The finished marker passages, a log.
pub static MARKER_PASSAGES: Table<MarkerPassageRow<'static>> =
    Table::new("passages", "Marker passages", 64)
        .label(&MarkerPassageRow::MARKER)
        .role(sdrkit_core::TableRole::Log);

/// The fields of a finished marker passage.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MarkerEvent<'a> {
    /// The marker ("Outer marker").
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The tone, the keying, the depth and the length.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: other.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// A marker, or a marker tone whose keying does not fit the standard.
    #[field(label = "Kind")]
    pub kind: PassageKind,
    /// The marker of the tone.
    #[field(label = "Marker")]
    pub marker: MarkerKind,
    /// The tone, measured.
    #[field(label = "Marker tone", unit = "Hz")]
    pub tone_hz: Hertz,
    /// The modulation depth of the tone.
    #[field(label = "Marker depth", unit = "%")]
    pub depth_pct: f64,
    /// The length of the passage.
    #[field(label = "Passage", unit = "s")]
    pub duration_s: f64,
}

/// A finished marker passage.
pub static MARKER_EVENT: EventKind<MarkerEvent<'static>> =
    EventKind::new("ils.marker", "Marker passage");

// --- DME ---

/// The result fields of DME, over the last second of pairs.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DmeFields<'a> {
    /// What is heard.
    #[field(label = "Heard")]
    pub heard: Heard,
    /// X or Y, from the pulse spacing. No value with too few pairs.
    #[field(label = "Channel mode")]
    pub channel_mode: Option<DmeMode>,
    /// The channel from the tuned frequency and the mode ("17X"). No value with too few pairs.
    #[field(shared = keys::CHANNEL, reading = 3)]
    pub channel: Option<&'a str>,
    /// The paired VOR or localizer frequency. No value without a channel that has one.
    #[field(label = "Paired VHF", unit = "Hz")]
    pub paired_vhf_hz: Option<Hertz>,
    /// Pairs a second.
    #[field(label = "Pairs a second", unit = "1/s", reading = 2)]
    pub pairs_per_s: f64,
    /// The mean pair spacing. No value with too few pairs.
    #[field(label = "Pair spacing", unit = "us")]
    pub spacing_us: Option<f64>,
    /// The mean pulse width at half amplitude. No value with too few pairs.
    #[field(label = "Pulse width", unit = "us")]
    pub width_us: Option<f64>,
    /// Pairs seen in all.
    #[field(label = "Pairs")]
    pub pairs: i64,
    /// The mean level of the pulses. No value with too few pairs.
    #[field(shared = keys::LEVEL_DBFS, reading = 4)]
    pub level_dbfs: Option<f64>,
}

// --- Loran ---

/// The result fields of Loran. A field of the chain has no value before the first measurement.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoranFields<'a> {
    /// The GRI in use, in tens of microseconds.
    #[field(label = "GRI", reading = 2)]
    pub gri: Option<i64>,
    /// The chain of that GRI. No value for a GRI that the `chains` setting does not name.
    #[field(label = "Chain", reading = 1)]
    pub chain: Option<&'a str>,
    /// The master is found: its correlation SNR is at least 10 dB.
    #[field(label = "Master", reading = 3)]
    pub master: bool,
    /// The correlation SNR of the master.
    #[field(label = "Master SNR", unit = "dB", reading = 4)]
    pub master_snr_db: Option<f64>,
    /// Measurements made.
    #[field(label = "Measurements")]
    pub fixes: i64,
}

/// A row of the stations of the chain. The row key is the letter of the station.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoranStationRow<'a> {
    /// `M` for the master, then the letters of the secondaries.
    #[field(label = "Station")]
    pub station: &'a str,
    /// The time difference from the master.
    #[field(label = "TD", unit = "us")]
    pub td_us: f64,
    /// The envelope-to-cycle difference relative to the master.
    #[field(label = "ECD", unit = "us")]
    pub ecd_us: f64,
    /// The signal to noise ratio of the correlation.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level relative to the master.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The master and the secondaries of the last measurement.
pub static LORAN_STATIONS: Table<LoranStationRow<'static>> =
    Table::new("stations", "Stations", 16).label(&LoranStationRow::STATION);

// --- DGPS ---

/// The result fields of DGPS. A field of the station has no value before the first frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsFields<'a> {
    /// The base station id.
    #[field(label = "Station", reading = 1)]
    pub station_id: Option<i64>,
    /// The health of the station.
    #[field(label = "Health", reading = 2)]
    pub health: Option<StationHealth>,
    /// The last modified Z-count: the time into the hour.
    #[field(label = "Z-count", unit = "s")]
    pub zcount: Option<Seconds>,
    /// The bit rate decoded. No value before the first report of the link.
    #[field(shared = keys::SYMBOL_RATE_HZ, reading = 4)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The framer of that bit rate holds the frame sync.
    #[field(shared = keys::LOCKED, reading = 3)]
    pub locked: bool,
    /// Words with good parity.
    #[field(label = "Good words")]
    pub good_words: i64,
    /// Words with bad parity.
    #[field(label = "Bad words")]
    pub bad_words: i64,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK, reading = 5)]
    pub frames_ok: i64,
    /// The position of the base station. No value before a message of type 3.
    #[field(label = "Base station")]
    pub base_station: Option<(f64, f64)>,
    /// The height of the base station above the ellipsoid. No value before a message of type 3.
    #[field(label = "Base station height", unit = "m")]
    pub base_height_m: Option<f64>,
    /// The last text message. No value before a message of type 16.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The satellites flagged unhealthy, by PRN. No value before a message of type 5.
    #[field(label = "Unhealthy satellites")]
    pub unhealthy: Option<&'a str>,
    /// The corrections held.
    #[field(label = "Corrections")]
    pub corrections: i64,
}

/// The levels of the channel, from its power meter.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsMeterFields {
    /// The mean level. No value before the first tenth of a second.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The peak level. No value before the first tenth of a second.
    #[field(label = "Peak level", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
}

/// The results of one MSK demodulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsMskFields {
    /// The error of the symbol clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The carrier offset that the demodulator removes.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A row of the frames by message type. The row key is the type in two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsMessageRow {
    /// The message type.
    #[field(label = "Type")]
    pub msg_type: i64,
    /// The frames of the type.
    #[field(label = "Count")]
    pub count: i64,
}

/// A row of the pseudorange corrections. The row key is the PRN in two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsCorrectionRow {
    /// The satellite.
    #[field(label = "PRN")]
    pub prn: i64,
    /// The pseudorange correction.
    #[field(label = "Range correction", unit = "m")]
    pub prc_m: f64,
    /// The range-rate correction.
    #[field(label = "Range-rate correction", unit = "m/s")]
    pub rrc_m_s: f64,
    /// The issue of data.
    #[field(label = "IOD")]
    pub iod: i64,
    /// The user differential range error.
    #[field(label = "UDRE")]
    pub udre: i64,
}

/// A row of the beacon almanac. The row key is the station id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsAlmanacRow {
    /// The station id.
    #[field(label = "Station")]
    pub station_id: i64,
    /// The frequency of the beacon.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The bit rate of the beacon.
    #[field(label = "Bit rate", unit = "Hz")]
    pub bit_rate_hz: Hertz,
    /// The position of the beacon.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The range of the beacon.
    #[field(label = "Range", unit = "km")]
    pub range_km: f64,
    /// The health of the beacon, 0 (healthy) to 3.
    #[field(label = "Health")]
    pub health: i64,
}

/// The row of the station table: the base station of the frames received. The row key is the station id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DgpsStationRow {
    /// The station id.
    #[field(label = "Station")]
    pub station_id: i64,
    /// The health of the station.
    #[field(label = "Health")]
    pub health: Option<StationHealth>,
    /// The position of the station. No value before a message of type 3.
    #[field(label = "Position")]
    pub position: Option<(f64, f64)>,
    /// The height of the station above the ellipsoid. No value before a message of type 3.
    #[field(label = "Height", unit = "m", role = Altitude)]
    pub height_m: Option<f64>,
}

/// The base station of the frames received: one row. The map point of the station shows this row.
pub static DGPS_STATION: Table<DgpsStationRow> =
    Table::new("station", "Station", 1).label(&DgpsStationRow::STATION_ID);
/// The frames by message type.
pub static DGPS_MESSAGES: Table<DgpsMessageRow> =
    Table::new("messages", "Messages", 64).label(&DgpsMessageRow::MSG_TYPE);
/// The latest pseudorange corrections.
pub static DGPS_CORRECTIONS: Table<DgpsCorrectionRow> =
    Table::new("corrections", "Corrections", 32)
        .label(&DgpsCorrectionRow::PRN)
        .sorted(&DgpsCorrectionRow::PRN, sdrkit_core::SortOrder::Ascending);
/// The beacon almanac.
pub static DGPS_ALMANAC: Table<DgpsAlmanacRow> =
    Table::new("almanac", "Beacon almanac", 64).label(&DgpsAlmanacRow::STATION_ID);

/// The fields of an RTCM frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RtcmFrameEvent<'a> {
    /// The station id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// A short text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: other.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The message type.
    #[field(label = "Type")]
    pub msg_type: i64,
    /// The station id.
    #[field(label = "Station")]
    pub station_id: i64,
    /// The modified Z-count, seconds into the hour.
    #[field(label = "Z-count", unit = "s")]
    pub zcount_s: f64,
    /// The sequence number.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// The health of the station.
    #[field(label = "Health")]
    pub health: StationHealth,
}

/// An RTCM frame.
pub static RTCM_FRAME: EventKind<RtcmFrameEvent<'static>> =
    EventKind::new("rtcm.frame", "RTCM frame");

/// The names of the MSK demodulators of DGPS in the graph, one for each bit rate.
pub const DGPS_DEMODS: [&str; 3] = ["msk50", "msk100", "msk200"];

// --- The description ---

/// A stack that reads a Morse identification: its own sets, the identification and the table of the words heard, the
/// word event, the settings and `clear`.
fn ident_stack(
    s: StackShape,
    settings: &[&'static dyn sdrkit_core::settings::DeclareSetting],
) -> StackShape {
    s.table(&IDENTS)
        .event(&IDENT_WORD)
        .settings(settings)
        .command(&sdrkit_graph::decoder::CLEAR)
}

/// The fields of the tone keyer `keyer`, which the stack shows under their own keys.
fn with_keyer(s: StackShape) -> StackShape {
    s.fields::<KeyerFields>().component("keyer", |c| {
        c.fields::<KeyerFields>()
            .shares(&["id_tone_hz", "id_depth_pct"])
    })
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The band scan covers the whole input, not the tuned beacon: it is kept over a change of frequency.
        let ndb = ident_stack(
            with_keyer(
                StackShape::new("ndb")
                    .fields::<TunedFields>()
                    .fields::<IdentFields>()
                    .fields::<CarrierFields>()
                    .fields::<NdbFields>(),
            ),
            crate::ndb::NdbSetting::SETTINGS,
        )
        .table(&NDB_SCAN)
        .kept(&["scan"]);
        let vor = ident_stack(
            with_keyer(
                StackShape::new("vor")
                    .fields::<VorFields>()
                    .fields::<IdentFields>()
                    .fields::<CarrierFields>(),
            ),
            crate::vor::VorSetting::SETTINGS,
        );
        let ils = ident_stack(
            with_keyer(
                StackShape::new("ils")
                    .fields::<IlsFields>()
                    .fields::<IdentFields>()
                    .fields::<CarrierFields>()
                    .fields::<MarkerFields>(),
            ),
            crate::ils::IlsSetting::SETTINGS,
        )
        .table(&MARKER_PASSAGES)
        .event(&MARKER_EVENT);
        let dme = ident_stack(
            StackShape::new("dme")
                .fields::<DmeFields>()
                .fields::<TunedFields>()
                .fields::<IdentFields>(),
            crate::dme::DmeSetting::SETTINGS,
        );
        let loran = StackShape::new("loran")
            .fields::<LoranFields>()
            .table(&LORAN_STATIONS)
            .settings(crate::loran::LoranSetting::SETTINGS)
            .command(&sdrkit_graph::decoder::CLEAR);
        // The almanac lists every beacon, the message counts and the text are a log of what was received: they are
        // kept over a change of frequency. The station, its corrections and the link are dropped.
        let mut dgps = StackShape::new("dgps")
            .fields::<DgpsFields>()
            .fields::<DgpsMeterFields>()
            .component("level", |c| {
                c.fields::<DgpsMeterFields>()
                    .shares(&["level_dbfs", "peak_dbfs"])
            });
        for name in DGPS_DEMODS {
            dgps = dgps.component(name, |c| c.fields::<DgpsMskFields>());
        }
        let dgps = dgps
            .table(&DGPS_STATION)
            .table(&DGPS_MESSAGES)
            .table(&DGPS_CORRECTIONS)
            .table(&DGPS_ALMANAC)
            .points("Base station")
            .event(&RTCM_FRAME)
            .settings(crate::dgps::DgpsSetting::SETTINGS)
            .command(&sdrkit_graph::decoder::CLEAR)
            .kept(&[
                "almanac",
                "messages",
                "DgpsFields.frames_ok",
                "DgpsFields.text",
            ]);
        Described::new("avnav", 2, 0)
            .content(
                sdrkit_core::ContentKind::Navigation,
                &["ndb", "vor", "ils", "dme", "loran", "dgps"],
            )
            .audio(sdrkit_core::AudioKind::Monitor, &["ndb", "vor", "ils"])
            .shared_source("fields", ALL)
            .shared_source("carrier", &["ndb", "vor", "ils"])
            .shared_source("keyer", &["ndb", "vor", "ils"])
            .shared_source("morse", &["ndb", "vor", "ils", "dme"])
            .shared_source("fold", &["ndb"])
            .shared_source("ident", &["ndb", "vor", "ils", "dme"])
            .shared_source("pairing", &["vor", "ils", "dme"])
            .shared_source("verify", ALL)
            .stack(ndb)
            .stack(vor)
            .stack(ils)
            .stack(dme)
            .stack(loran)
            .stack(dgps)
    })
}

/// Every stack of the family.
const ALL: &[&str] = &["ndb", "vor", "ils", "dme", "loran", "dgps"];

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the avnav family does not describe the stack {id}"))
}

/// An empty store of the result sets `sets` and the tables `tables`, for the component that writes them.
pub fn store(sets: &[Shape], tables: &[&TableDef]) -> Published {
    let fields: Vec<&dyn Declare> = sets.iter().flat_map(|s| s.fields.iter().copied()).collect();
    Published::new(&fields).with_tables(tables)
}
