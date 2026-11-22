//! The interface of the satellite communications family: the shapes of the result sets, the table rows and the events,
//! the tables, the event types and what each stack declares.

use crate::modes::argos::downlink::Coding;
use crate::modes::argos::{BurstKind, Link};
use crate::modes::cubesat::demod::Framing;
use crate::modes::goes::body::DataFormat;
use crate::modes::inmarsatc::ChannelState;
use crate::modes::iridium::Direction;
use crate::modes::lorasat::display::CrcState;
use crate::modes::orbcomm::{DecoderState, PacketType};
use crate::modes::psk::display::{Note, ShapeFound};
use crate::modes::psk::rx::OrderChoice;
use crate::modes::qo100::rx::{LockPhase, SignalClass};
use crate::modes::{
    aero, argos, bgan, cubesat, fleet, globalstar, goes, hrdcp, inmarsat, inmarsatc, iridium,
    lorasat, orbcomm, psk, qo100, sarsat, sat_survey,
};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::settings::Argument;
use sdrkit_core::{ContentType, SettingSet};
use sdrkit_graph::DecoderFields;
use sdrkit_proto_aero::aircraft::columns as aircraft;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

/// The result fields of Iridium.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IridiumFields {
    /// Bursts the detector found.
    #[field(label = "Bursts found")]
    pub bursts: i64,
    /// Bursts demodulated.
    #[field(label = "Bursts demodulated")]
    pub demodulated: i64,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Input items dropped because a helper thread did not keep up.
    #[field(shared = keys::DROPPED)]
    pub dropped: i64,
    /// Bursts found twice and counted once.
    #[field(label = "Duplicates")]
    pub duplicates: i64,
    /// Bursts with the downlink unique word.
    #[field(label = "Downlink bursts")]
    pub downlink: i64,
    /// Bursts with the uplink unique word.
    #[field(label = "Uplink bursts")]
    pub uplink: i64,
    /// Bursts found per second (the last report interval).
    #[field(label = "Bursts a second", unit = "Hz")]
    pub burst_rate: Hertz,
    /// Unique words found per second (the last report interval).
    #[field(label = "Unique words a second", unit = "Hz")]
    pub uw_rate: Hertz,
    /// The mean confidence of the bursts in the last interval.
    #[field(label = "Confidence", unit = "%")]
    pub confidence_pct: Option<f64>,
    /// The mean Es/N0 of the bursts in the last interval.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// The detector's noise floor, dB relative to full scale in 1 Hz.
    #[field(label = "Noise density", unit = "dBFS/Hz")]
    pub noise_db_hz: Option<f64>,
}

/// A RAW line. The row key is the sequence number of the line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IridiumRawRow<'a> {
    /// At (signal time).
    #[field(label = "At (signal time)", unit = "s", role = Time)]
    pub at: Seconds,
    /// Frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// Direction.
    #[field(label = "Direction")]
    pub direction: Direction,
    /// Confidence.
    #[field(label = "Confidence", unit = "%")]
    pub confidence_pct: f64,
    /// RAW line.
    #[field(label = "RAW line", role = Text)]
    pub line: &'a str,
}

/// A RAW line of one burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IridiumRawEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// Confidence.
    #[field(label = "Confidence", unit = "%")]
    pub confidence_pct: f64,
}

/// The result fields of Fleet Broadcast.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetBroadcastFields<'a> {
    /// The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
    #[field(label = "State")]
    pub state: i64,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Option<Hertz>,
    /// The line of the carrier search over the floor.
    #[field(label = "Carrier line", unit = "dB")]
    pub line_db: f64,
    /// Sample-clock or symbol-clock error, parts per million.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The eye: 0 clean, towards 1 noisy.
    #[field(label = "Eye")]
    pub eye: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Symbols so far.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// The frames are aligned.
    #[field(label = "Framed")]
    pub framed: bool,
    /// Several columns repeat.
    #[field(label = "Ambiguous")]
    pub ambiguous: bool,
    /// The repeat of the sync channel, frames.
    #[field(label = "Sync repeat", unit = "frames")]
    pub sync_period: Option<i64>,
    /// The agreement of the sync channel with itself one period later.
    #[field(label = "Sync agreement", unit = "%")]
    pub sync_agreement: Option<f64>,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The class letters, channel 0 first.
    #[field(label = "Channels")]
    pub classes: &'a str,
    /// What is not confirmed by a public document.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// One of the 16 channels. The row key is its place, `000` first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetBroadcastChannelRow<'a> {
    /// The channel (or column, when not framed).
    #[field(label = "Channel")]
    pub channel: i64,
    /// The class of a channel.
    #[field(label = "Class")]
    pub class: &'a str,
    /// The share of bit changes.
    #[field(label = "Activity", unit = "%")]
    pub activity_pct: f64,
    /// The share of ones.
    #[field(label = "Ones", unit = "%")]
    pub ones_pct: f64,
    /// The repeat of the sync channel, frames.
    #[field(label = "Sync repeat", unit = "frames")]
    pub sync_period: Option<i64>,
    /// The last 64 bits as hex, oldest first.
    #[field(label = "Last 64 bits")]
    pub last_bits: &'a str,
}

/// A frame of the frame log. The row key is its place, `000` first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetBroadcastFrameRow<'a> {
    /// A frame as 4 hex digits, channel 0 the most significant bit.
    #[field(label = "Frame")]
    pub word: &'a str,
    /// A frame as 16 bits, channel 0 first.
    #[field(label = "Bits")]
    pub bits: &'a str,
}

/// A change of the state of the receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetBroadcastStateEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The state: 0 nothing, 1 carrier, 2 bits, 3 framed.
    #[field(label = "State")]
    pub state: i64,
}

/// The result fields of BGAN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BganFields<'a> {
    /// Carriers found.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// Carriers in frame sync.
    #[field(label = "In frame sync")]
    pub in_sync: i64,
    /// Carriers whose signalling decoded.
    #[field(label = "Signalling decoded")]
    pub decoded: i64,
    /// GMR-2 class carriers.
    #[field(label = "GMR-2 class carriers")]
    pub gmr2: i64,
    /// Networks heard (MCC-MNC).
    #[field(label = "Networks")]
    pub networks: &'a str,
    /// Satellites heard.
    #[field(label = "Satellites")]
    pub satellites_heard: &'a str,
    /// FEC blocks decoded.
    #[field(label = "Blocks good")]
    pub blocks_ok: i64,
    /// FEC blocks that failed.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Connection PDUs counted, never read.
    #[field(label = "Connection PDUs")]
    pub connection_pdus: i64,
    /// PDUs addressed to a terminal counted, never read.
    #[field(label = "Addressed PDUs")]
    pub addressed_pdus: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// What is not confirmed by a public document.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A carrier. The row key is its place, `000` first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BganCarrierRow<'a> {
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Option<Hertz>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Width of a carrier.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// What the carrier is.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The demodulator state.
    #[field(label = "State")]
    pub state: &'a str,
    /// Bearer.
    #[field(label = "Bearer")]
    pub bearer: Option<&'a str>,
    /// Subtype of block 0.
    #[field(label = "Subtype")]
    pub subtype: Option<&'a str>,
    /// Symbol rate found, Hz.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frequency left after the channel filter.
    #[field(label = "CFO", unit = "Hz")]
    pub cfo_hz: Option<Hertz>,
    /// C/N0 from the pilots.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
    /// Frames in sync.
    #[field(label = "Frames")]
    pub frames: i64,
    /// FEC blocks decoded.
    #[field(label = "Blocks good")]
    pub blocks_ok: i64,
    /// FEC blocks that failed.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Share of blocks with PDUs.
    #[field(label = "Load", unit = "%")]
    pub load_pct: f64,
    /// Frame sync without decodable tables.
    #[field(label = "Structure only")]
    pub structure_only: bool,
    /// Spot beam id.
    #[field(label = "Spot beam")]
    pub spot_beam: Option<i64>,
    /// rnc-id.
    #[field(label = "RNC")]
    pub rnc: Option<i64>,
    /// bct-id.
    #[field(label = "BCt id")]
    pub bct_id: Option<i64>,
    /// f-bearer.
    #[field(label = "f-bearer")]
    pub f_bearer: Option<i64>,
    /// Beam type.
    #[field(label = "Beam type")]
    pub beam_type: Option<&'a str>,
    /// MCC-MNC.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// The carrying satellite.
    #[field(label = "Satellite")]
    pub satellite: Option<&'a str>,
    /// Seconds since last heard.
    #[field(label = "Last seen", unit = "s")]
    pub last_seen: Seconds,
}

/// A spot beam. The row key is its place, `000` first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BganBeamRow<'a> {
    /// Spot beam id.
    #[field(label = "Spot beam")]
    pub spot_beam: i64,
    /// Beam type.
    #[field(label = "Beam type")]
    pub beam_type: Option<&'a str>,
    /// The forward frequency of a primary bearer.
    #[field(label = "Primary bearer", unit = "Hz")]
    pub primary_hz: Option<Hertz>,
    /// The FBearerType name of a primary bearer.
    #[field(label = "Bearer type")]
    pub bearer_type: Option<&'a str>,
}

/// A line of the signalling log. The row key is its place, `000` first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BganLogRow<'a> {
    /// Seconds into the capture.
    #[field(label = "At", unit = "s")]
    pub at: Seconds,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A line of decoded signalling.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BganSignallingEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The result fields of the satellite carrier survey.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatSurveyFields<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The method of the submode.
    #[field(label = "Method")]
    pub method: &'a str,
    /// The threshold in force.
    #[field(label = "Threshold in force", unit = "dB")]
    pub threshold_in_force_db: f64,
    /// The bin of the transform.
    #[field(label = "Bin", unit = "Hz")]
    pub bin_hz: Option<f64>,
    /// Element sets read.
    #[field(label = "Element sets")]
    pub catalogue: i64,
    /// Carrier lines shown.
    #[field(label = "Carrier lines")]
    pub lines: Option<i64>,
    /// Channels active now.
    #[field(label = "Channels active")]
    pub channels_active: Option<i64>,
    /// Channels that have been active.
    #[field(label = "Channels seen")]
    pub channels_seen: Option<i64>,
    /// The frame rate searched.
    #[field(label = "Frame rate searched", unit = "Hz")]
    pub frame_rate_hz: Option<f64>,
    /// The frame rate that fitted best.
    #[field(label = "Frame rate measured", unit = "Hz")]
    pub measured_rate_hz: Option<f64>,
    /// The level of the frame fit.
    #[field(label = "Frame level", unit = "dB")]
    pub frame_level_db: Option<f64>,
    /// The frame structure is detected.
    #[field(label = "Detected")]
    pub detected: Option<bool>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A carrier line. The row key is the number of the line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatSurveyCarrierRow<'a> {
    /// The number of a carrier line.
    #[field(label = "Line")]
    pub line: i64,
    /// Offset from the centre of the samples.
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The frequency, MHz.
    #[field(label = "Frequency", unit = "MHz")]
    pub mhz: f64,
    /// The level over the background or floor.
    #[field(label = "S/N", unit = "dB")]
    pub snr_db: f64,
    /// The length of a line.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// The drift of a line.
    #[field(label = "Drift", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The line or channel is active.
    #[field(label = "Active")]
    pub active: bool,
    /// The channel name of a line.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The satellite that the Doppler curve names.
    #[field(label = "Satellite")]
    pub satellite: &'a str,
    /// Its catalogue number.
    #[field(label = "Catalogue number")]
    pub norad: Option<i64>,
    /// The rms of the Doppler match.
    #[field(label = "Match rms", unit = "Hz")]
    pub rms_hz: Option<f64>,
    /// Another satellite fits about as well.
    #[field(label = "Ambiguous")]
    pub ambiguous: bool,
}

/// A channel that has been active. The row key is its grid frequency in whole Hz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatSurveyChannelRow<'a> {
    /// Offset from the centre of the samples.
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The frequency of a channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency_hz: Hertz,
    /// The width of a channel.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: f64,
    /// The level over the background or floor.
    #[field(label = "S/N", unit = "dB")]
    pub snr_db: f64,
    /// The share of time a channel is on.
    #[field(label = "Duty")]
    pub duty: f64,
    /// Times a channel turned active.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The signal time a channel was last on.
    #[field(label = "Last seen", unit = "s")]
    pub last_seen_s: f64,
    /// The line or channel is active.
    #[field(label = "Active")]
    pub active: bool,
    /// The name of a channel.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// A finished carrier line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatSurveyCarrierEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The number of a carrier line.
    #[field(label = "Line")]
    pub line: i64,
    /// Offset from the centre of the samples.
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The frequency, MHz.
    #[field(label = "Frequency", unit = "MHz")]
    pub mhz: f64,
    /// The level over the background or floor.
    #[field(label = "S/N", unit = "dB")]
    pub snr_db: f64,
    /// The length of a line.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// The drift of a line.
    #[field(label = "Drift", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The channel name of a line.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The satellite that the Doppler curve names.
    #[field(label = "Satellite")]
    pub satellite: &'a str,
    /// Its catalogue number.
    #[field(label = "Catalogue number")]
    pub norad: Option<i64>,
    /// The rms of the Doppler match.
    #[field(label = "Match rms", unit = "Hz")]
    pub rms_hz: Option<f64>,
    /// Another satellite fits about as well.
    #[field(label = "Ambiguous")]
    pub ambiguous: bool,
}

/// The RAW lines of Iridium, newest first (only while `raw_output` is on).
pub static IRIDIUM_RAW: Table<IridiumRawRow<'static>> =
    Table::new("raw", "RAW lines", iridium::RAW_CAP).role(sdrkit_core::TableRole::Messages);
/// A RAW line of Iridium.
pub static IRIDIUM_RAW_EVENT: EventKind<IridiumRawEvent<'static>> =
    EventKind::new("iridium.raw", "RAW line");
/// The bursts of Iridium found in the last 2 s: (age, offset from the centre).
pub const IRIDIUM_SCATTER_DETECTED: SeriesDef = SeriesDef {
    key: "scatter_detected",
    label: "Bursts found",
    x_unit: "s",
    y_unit: "Hz",
};
/// The bursts of Iridium with a unique word in the last 2 s: (age, offset from the centre).
pub const IRIDIUM_SCATTER_UW: SeriesDef = SeriesDef {
    key: "scatter_uw",
    label: "Bursts with a unique word",
    x_unit: "s",
    y_unit: "Hz",
};

/// The 16 channels of Fleet Broadcast.
pub static FLEET_BROADCAST_CHANNELS: Table<FleetBroadcastChannelRow<'static>> =
    Table::new("channels", "Channels", fleet::rx::COLUMNS);
/// The last frames of Fleet Broadcast, newest last: the frame log and the bit matrix.
pub static FLEET_BROADCAST_FRAMES: Table<FleetBroadcastFrameRow<'static>> =
    Table::new("frame_log", "Frame log", fleet::rx::FRAME_LOG);
/// A change of the state of the Fleet Broadcast receiver, with its text.
pub static FLEET_BROADCAST_STATE: EventKind<FleetBroadcastStateEvent<'static>> =
    EventKind::new("fleet_broadcast.state", "Fleet Broadcast state");

/// The carriers of BGAN.
pub static BGAN_CARRIERS: Table<BganCarrierRow<'static>> =
    Table::new("carrier_table", "Carriers", 64)
        .label(&BganCarrierRow::SATELLITE)
        .sorted(&BganCarrierRow::SNR_DB, sdrkit_core::SortOrder::Descending);
/// The spot beams of BGAN.
pub static BGAN_BEAMS: Table<BganBeamRow<'static>> = Table::new("beam_table", "Beams", 256);
/// The signalling log of BGAN.
pub static BGAN_LOG: Table<BganLogRow<'static>> = Table::new("log", "Log", bgan::rx::LOG_LINES);
/// A line of BGAN signalling.
pub static BGAN_SIGNALLING: EventKind<BganSignallingEvent<'static>> =
    EventKind::new("bgan.signalling", "BGAN signalling");

/// The carrier lines of the survey: active ones of 2.5 s or more and the finished ones.
pub static SAT_SURVEY_CARRIERS: Table<SatSurveyCarrierRow<'static>> =
    Table::new("carriers", "Carrier lines", sat_survey::CARRIER_ROWS)
        .label(&SatSurveyCarrierRow::SATELLITE)
        .sorted(
            &SatSurveyCarrierRow::SNR_DB,
            sdrkit_core::SortOrder::Descending,
        );
/// The channels of the survey that have been active: the active ones first, then by S/N.
pub static SAT_SURVEY_CHANNELS: Table<SatSurveyChannelRow<'static>> =
    Table::new("channels", "Channels", sat_survey::CHANNEL_ROWS).label(&SatSurveyChannelRow::NAME);
/// A finished carrier line of the survey, with the satellite when the Doppler curve names one.
pub static SAT_SURVEY_CARRIER: EventKind<SatSurveyCarrierEvent<'static>> =
    EventKind::new("survey.carrier", "Carrier line");

/// The result fields of Globalstar.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlobalstarFields {
    /// Channels active now.
    #[field(label = "Active channels")]
    pub active_channels: i64,
    /// Stretches of activity completed since the start (or the last Clear).
    #[field(label = "Activities")]
    pub activities: i64,
    /// The mean Doppler of the active channels.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Option<Hertz>,
}

/// A row of the 13 channels. The row key is the channel number ("07").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlobalstarChannelRow {
    /// A channel number, 1 to 13.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The centre of the channel.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Hertz,
    /// The channel lies inside the input.
    #[field(label = "In view")]
    pub in_view: bool,
    /// The channel is active now.
    #[field(label = "Active")]
    pub active: bool,
    /// The power of the channel.
    #[field(label = "Power", unit = "dBFS")]
    pub power_dbfs: f64,
    /// The noise floor of the channel.
    #[field(label = "Noise floor", unit = "dBFS")]
    pub floor_dbfs: f64,
    /// The signal to noise ratio of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The share of time the channel was on (60 s average).
    #[field(label = "Occupancy", unit = "%")]
    pub occupancy_pct: f64,
    /// The Doppler shift of the channel.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Hertz,
    /// The signal time when the channel was last active.
    #[field(label = "Last active (signal time)", unit = "s")]
    pub last_active: Option<Seconds>,
    /// The stretches of activity on the channel.
    #[field(label = "Events")]
    pub events: i64,
}

/// A stretch of activity on a channel. The row key is the sequence number of the stretch.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlobalstarActivityRow {
    /// The signal time of the start.
    #[field(label = "Start (signal time)", unit = "s")]
    pub start: Seconds,
    /// How long a stretch of activity lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// A channel number, 1 to 13.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The peak SNR.
    #[field(label = "Peak SNR", unit = "dB")]
    pub peak_snr_db: f64,
    /// The Doppler shift at the start.
    #[field(label = "Doppler at the start", unit = "Hz")]
    pub doppler_start: Hertz,
    /// The Doppler shift at the end.
    #[field(label = "Doppler at the end", unit = "Hz")]
    pub doppler_end: Hertz,
    /// The wall-clock time of the start, when the samples are live.
    #[field(label = "Started at")]
    pub wall_start: Option<Time>,
}

/// A stretch of activity on a channel. The source is the channel ("channel 7").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlobalstarActivityEvent<'a> {
    /// The channel ("channel 7").
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The length and the Doppler shift of the stretch.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The peak signal to noise ratio of the stretch.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// A channel number, 1 to 13.
    #[field(label = "Channel")]
    pub channel: i64,
    /// How long a stretch of activity lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
}

/// The 13 channels of Globalstar.
pub static GLOBALSTAR_CHANNELS: Table<GlobalstarChannelRow> =
    Table::new("channels", "Channels", globalstar::CHANNELS);
/// The stretches of activity, newest first.
pub static GLOBALSTAR_ACTIVITY: Table<GlobalstarActivityRow> =
    Table::new("activity", "Activity", globalstar::EVENTS_CAP);
/// A stretch of activity on a Globalstar channel.
pub static GLOBALSTAR_ACTIVITY_EVENT: EventKind<GlobalstarActivityEvent<'static>> =
    EventKind::new("globalstar.activity", "Activity");

/// The series of Doppler points (signal seconds, Hz) of one Globalstar channel inside the plot span.
const fn doppler_track(key: &'static str, label: &'static str) -> SeriesDef {
    SeriesDef {
        key,
        label,
        x_unit: "s",
        y_unit: "Hz",
    }
}

/// The Doppler tracks of the 13 Globalstar channels, channel 1 first.
pub const GLOBALSTAR_DOPPLER: [SeriesDef; globalstar::CHANNELS] = [
    doppler_track("doppler_01", "Doppler track, channel 1"),
    doppler_track("doppler_02", "Doppler track, channel 2"),
    doppler_track("doppler_03", "Doppler track, channel 3"),
    doppler_track("doppler_04", "Doppler track, channel 4"),
    doppler_track("doppler_05", "Doppler track, channel 5"),
    doppler_track("doppler_06", "Doppler track, channel 6"),
    doppler_track("doppler_07", "Doppler track, channel 7"),
    doppler_track("doppler_08", "Doppler track, channel 8"),
    doppler_track("doppler_09", "Doppler track, channel 9"),
    doppler_track("doppler_10", "Doppler track, channel 10"),
    doppler_track("doppler_11", "Doppler track, channel 11"),
    doppler_track("doppler_12", "Doppler track, channel 12"),
    doppler_track("doppler_13", "Doppler track, channel 13"),
];

/// The result fields of GOES DCS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesDcsFields {
    /// The reports read whole.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The reports cut off or lost.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The carriers followed now.
    #[field(label = "Carriers")]
    pub carriers: i64,
}

/// A platform report. The row key is the sequence number of the report.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesDcsReportRow<'a> {
    /// The signal time of a report.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time of a report (live radios only).
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Hertz,
    /// Manchester coded.
    #[field(label = "Manchester")]
    pub manchester: bool,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The flag byte, or -1.
    #[field(label = "Flags")]
    pub flags: i64,
    /// The data format.
    #[field(label = "Format")]
    pub format: DataFormat,
    /// The report ended with an EOT.
    #[field(label = "EOT")]
    pub eot: bool,
    /// Characters whose parity failed.
    #[field(label = "Parity errors")]
    pub parity_errors: i64,
    /// The data of the report.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A platform heard. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesDcsPlatformRow<'a> {
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Hertz,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The data format.
    #[field(label = "Format")]
    pub format: DataFormat,
    /// The data of the report.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// How many times it was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A carrier followed now. The row key is its place in the list ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesDcsCarrierRow {
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Option<Hertz>,
}

/// A platform report. The source is the address, the text the data.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesDcsReportEvent<'a> {
    /// The address of the platform.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The data of the report.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Hertz,
    /// Manchester coded.
    #[field(label = "Manchester")]
    pub manchester: bool,
    /// The flag byte, or -1.
    #[field(label = "Flags")]
    pub flags: i64,
    /// The data format.
    #[field(label = "Format")]
    pub format: DataFormat,
    /// The report ended with an EOT.
    #[field(label = "EOT")]
    pub eot: bool,
    /// Characters whose parity failed.
    #[field(label = "Parity errors")]
    pub parity_errors: i64,
    /// The address bits corrected.
    #[field(shared = keys::BITS_CORRECTED)]
    pub bits_corrected: i64,
}

/// The reports, newest last.
pub static GOES_DCS_REPORTS: Table<GoesDcsReportRow<'static>> =
    Table::new("reports", "Reports", goes::REPORT_CAP).label(&GoesDcsReportRow::ADDRESS);
/// The platforms heard.
pub static GOES_DCS_PLATFORMS: Table<GoesDcsPlatformRow<'static>> =
    Table::new("platforms", "Platforms", goes::PLATFORM_CAP).label(&GoesDcsPlatformRow::ADDRESS);
/// The carriers followed now.
pub static GOES_DCS_CARRIERS: Table<GoesDcsCarrierRow> =
    Table::new("carriers", "Carriers", goes::MAX_SLOTS).sorted(
        &GoesDcsCarrierRow::SNR_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// A platform report of GOES DCS.
pub static GOES_DCS_REPORT: EventKind<GoesDcsReportEvent<'static>> =
    EventKind::new("goes_dcs.report", "Platform report");
/// The band line of GOES DCS.
pub const GOES_DCS_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Band",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The arguments of `remove` of GOES DCS.
#[derive(Arguments)]
pub struct GoesDcsPlatformArgs<'a> {
    /// The row key of the platform.
    #[arg(shared = GOES_DCS_PLATFORM_ROW)]
    pub row: &'a str,
}

/// The row key of a GOES DCS platform.
pub const GOES_DCS_PLATFORM_ROW: Argument<field::Text> = objcmd::row_of("platforms");
/// Forgets one GOES DCS platform.
pub const GOES_DCS_REMOVE: CommandKind<GoesDcsPlatformArgs<'static>> = objcmd::remove_kind();

/// The result fields of Argos.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosFields {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The link the display shows: uplink or downlink.
    #[field(label = "Link")]
    pub link: Link,
    /// The receiver holds lock.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The downlink's carrier line over the noise.
    #[field(label = "Carrier line", unit = "dB")]
    pub carrier_line_db: f64,
    /// Downlink bits received.
    #[field(label = "Downlink bits")]
    pub downlink_bits: i64,
    /// The downlink coding in use.
    #[field(label = "Coding")]
    pub coding: Option<Coding>,
}

/// An uplink burst. The row key is the sequence number of the burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosBurstRow<'a> {
    /// The signal time of the row's moment.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time of the row's moment, when the samples were live.
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The modulation of a burst.
    #[field(label = "Kind")]
    pub kind: BurstKind,
    /// The carrier from the dial.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The unmodulated carrier.
    #[field(label = "Carrier", unit = "ms")]
    pub carrier_ms: f64,
    /// The whole burst.
    #[field(label = "Burst", unit = "ms")]
    pub total_ms: f64,
    /// The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
    #[field(label = "Quality")]
    pub quality: f64,
    /// The platform id (7 hex digits).
    #[field(label = "Platform")]
    pub platform: Option<&'a str>,
    /// The number of user data bits.
    #[field(label = "Data bits")]
    pub data_bits: Option<i64>,
    /// The user data, the first bit the high bit of the first byte.
    #[field(label = "Data", content = ContentType::Binary)]
    pub data: Option<Arc<[u8]>>,
}

/// A platform heard. The row key is the platform id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosPlatformRow<'a> {
    /// The platform id (7 hex digits).
    #[field(label = "Platform")]
    pub platform: &'a str,
    /// The user data, the first bit the high bit of the first byte.
    #[field(label = "Data", content = ContentType::Binary)]
    pub data: Arc<[u8]>,
    /// The drift.
    #[field(shared = keys::DRIFT_HZ_S)]
    pub drift_hz_s: f64,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// How many times it was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A downlink frame with a good FCS. The row key is the sequence number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosFrameRow {
    /// The signal time of the row's moment.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The downlink coding in use.
    #[field(label = "Coding")]
    pub coding: Coding,
    /// The length of a frame without its FCS.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// A frame's bytes.
    #[field(label = "Frame", content = ContentType::Binary)]
    pub frame: Arc<[u8]>,
}

/// A platform message. The source is the platform id, the text the data in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosPttEvent<'a> {
    /// The platform id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The data in hexadecimal.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The platform id (7 hex digits).
    #[field(label = "Platform")]
    pub platform: &'a str,
    /// The number of user data bits.
    #[field(label = "Data bits")]
    pub data_bits: i64,
    /// The carrier from the dial.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The quality of the coherent soft bits (1 without noise, 0.64 for noise alone).
    #[field(label = "Quality")]
    pub quality: f64,
    /// The user data, the first bit the high bit of the first byte.
    #[field(label = "Data", content = ContentType::Binary)]
    pub data: Arc<[u8]>,
}

/// A downlink frame with a good FCS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArgosDownlinkFrameEvent {
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The downlink coding in use.
    #[field(label = "Coding")]
    pub coding: Coding,
    /// The length of a frame without its FCS.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// A frame's bytes.
    #[field(label = "Frame", content = ContentType::Binary)]
    pub frame: Arc<[u8]>,
}

/// The uplink burst log.
pub static ARGOS_BURSTS: Table<ArgosBurstRow<'static>> =
    Table::new("bursts", "Bursts", argos::BURST_CAP).label(&ArgosBurstRow::PLATFORM);
/// The platforms heard.
pub static ARGOS_PLATFORMS: Table<ArgosPlatformRow<'static>> =
    Table::new("platforms", "Platforms", argos::PLATFORM_CAP).label(&ArgosPlatformRow::PLATFORM);
/// The downlink frames with a good FCS.
pub static ARGOS_FRAMES: Table<ArgosFrameRow> =
    Table::new("frames", "Downlink frames", argos::FRAME_CAP);
/// A platform message of Argos.
pub static ARGOS_PTT: EventKind<ArgosPttEvent<'static>> =
    EventKind::new("argos.ptt", "Platform message");
/// A downlink frame of Argos.
pub static ARGOS_DOWNLINK_FRAME: EventKind<ArgosDownlinkFrameEvent> =
    EventKind::new("argos.downlink_frame", "Downlink frame");
/// The Doppler track of the Argos downlink.
pub const ARGOS_DOPPLER: SeriesDef = SeriesDef {
    key: "doppler",
    label: "Downlink Doppler",
    x_unit: "s",
    y_unit: "Hz",
};

/// The arguments of `remove` of Argos.
#[derive(Arguments)]
pub struct ArgosPlatformArgs<'a> {
    /// The row key of the platform.
    #[arg(shared = ARGOS_PLATFORM_ROW)]
    pub row: &'a str,
}

/// The row key of an Argos platform.
pub const ARGOS_PLATFORM_ROW: Argument<field::Text> = objcmd::row_of("platforms");
/// Forgets one Argos platform.
pub const ARGOS_REMOVE: CommandKind<ArgosPlatformArgs<'static>> = objcmd::remove_kind();

/// The result fields of Meteosat HRDCP.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrdcpFields<'a> {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The carriers followed now.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// The channel plan in use.
    #[field(label = "Channel plan")]
    pub plan: &'a str,
    /// What is not confirmed by a public document.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A message of the log. The row key is the sequence number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrdcpReportRow<'a> {
    /// The signal time of a message.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time of a message (live radios only).
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The bits corrected.
    #[field(label = "Bits corrected")]
    pub bits_corrected: i64,
    /// The sequence counter.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// Self-timed or alert.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The 10 health bits.
    #[field(label = "Health")]
    pub health: &'a str,
    /// The compression of the data.
    #[field(label = "Compression")]
    pub compression: &'a str,
    /// The CRC passed.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
    /// Coded frames of the message.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames whose words did not decode.
    #[field(label = "Bad frames")]
    pub frames_bad: i64,
    /// Reed-Solomon bytes corrected.
    #[field(label = "RS corrected")]
    pub rs_corrected: i64,
    /// Platform data bytes.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// ASCII or binary.
    #[field(label = "Format")]
    pub format: &'a str,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Hertz,
    /// Manchester coded.
    #[field(label = "Manchester")]
    pub manchester: bool,
    /// The frequency of the carrier.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The channel number of the plan.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The data of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A platform heard. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrdcpPlatformRow<'a> {
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The channel number of the plan.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// ASCII or binary.
    #[field(label = "Format")]
    pub format: &'a str,
    /// The data of the last message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// How many times it was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A carrier followed now. The row key is its place in the list ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrdcpCarrierRow {
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The channel number of the plan.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier state: 0 looking for the marker, 2 decoding.
    #[field(label = "State")]
    pub state: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Option<Hertz>,
    /// Platform data bytes.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// A message received whole with a good CRC. The source is the address, the text the data.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrdcpReportEvent<'a> {
    /// The address of the platform.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The data of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The address word (8 hex digits).
    #[field(label = "Address")]
    pub address: &'a str,
    /// The channel number of the plan.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The sequence counter.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// Self-timed or alert.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The 10 health bits.
    #[field(label = "Health")]
    pub health: &'a str,
    /// The compression of the data.
    #[field(label = "Compression")]
    pub compression: &'a str,
    /// Platform data bytes.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// Reed-Solomon bytes corrected.
    #[field(label = "RS corrected")]
    pub rs_corrected: i64,
    /// The CRC passed.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
}

/// The message log of Meteosat HRDCP.
pub static HRDCP_REPORTS: Table<HrdcpReportRow<'static>> =
    Table::new("reports", "Reports", hrdcp::REPORT_CAP).label(&HrdcpReportRow::ADDRESS);
/// The platforms heard.
pub static HRDCP_PLATFORMS: Table<HrdcpPlatformRow<'static>> =
    Table::new("platforms", "Platforms", hrdcp::PLATFORM_CAP).label(&HrdcpPlatformRow::ADDRESS);
/// The carriers followed now.
pub static HRDCP_CARRIERS: Table<HrdcpCarrierRow> =
    Table::new("carriers", "Carriers", hrdcp::MAX_FOLLOWERS)
        .sorted(&HrdcpCarrierRow::SNR_DB, sdrkit_core::SortOrder::Descending);
/// A message of Meteosat HRDCP.
pub static HRDCP_REPORT: EventKind<HrdcpReportEvent<'static>> =
    EventKind::new("hrdcp.report", "HRDCP message");
/// The band line of Meteosat HRDCP.
pub const HRDCP_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Band",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The result fields of COSPAS-SARSAT.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SarsatFields {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Bursts heard.
    #[field(label = "Bursts heard")]
    pub bursts_heard: i64,
    /// First-generation finders running.
    #[field(label = "Finders")]
    pub finders: i64,
    /// The width the finders cover.
    #[field(label = "Search span", unit = "Hz")]
    pub span_hz: Hertz,
}

/// A burst of the log. The row key is the sequence number of the burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SarsatBurstRow<'a> {
    /// The signal time of a burst.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time of the burst, when the samples were live.
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The beacon generation, 1 or 2.
    #[field(label = "Generation")]
    pub generation: i64,
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    #[field(label = "Hex ID")]
    pub hex_id: &'a str,
    /// The beacon type (EPIRB, ELT, PLB ...).
    #[field(label = "Type")]
    pub beacon_type: &'a str,
    /// The country of the beacon's MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The protocol in words.
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The identification in words.
    #[field(label = "Identification")]
    pub identification: &'a str,
    /// The position that the beacon sends.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The 121.5 MHz homing flag.
    #[field(label = "121.5 MHz homing")]
    pub homing: bool,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
}

/// A beacon heard, with its last burst. The row key is the hex identity.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SarsatBeaconRow<'a> {
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    #[field(label = "Hex ID")]
    pub hex_id: &'a str,
    /// The beacon generation, 1 or 2.
    #[field(label = "Generation")]
    pub generation: i64,
    /// The beacon type (EPIRB, ELT, PLB ...).
    #[field(label = "Type")]
    pub beacon_type: &'a str,
    /// The country of the beacon's MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The identification in words.
    #[field(label = "Identification")]
    pub identification: &'a str,
    /// The position that the beacon sends.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// Bursts whose soft bits were summed to decode the message.
    #[field(label = "Combined bursts")]
    pub combined: Option<i64>,
    /// A later burst decoded to the same identification before the message was reported.
    #[field(label = "Confirmed by repeat")]
    pub confirmed: Option<bool>,
    /// The distance from the decoded word to the next one that passes, over the mean reliability of a bit.
    #[field(label = "Repair margin")]
    pub repair_margin: Option<f64>,
    /// The reliability of the repaired bits over the mean reliability of a bit.
    #[field(label = "Repair cost")]
    pub repair_cost: Option<f64>,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// How many times it was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A beacon decoded. The source is the hex identity. A distress beacon writes the urgency emergency, a test beacon none.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SarsatBeaconEvent<'a> {
    /// The hex identity.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What the beacon says.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent the beacon is: emergency for a distress beacon.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The position that the beacon sends.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The beacon generation, 1 or 2.
    #[field(label = "Generation")]
    pub generation: i64,
    /// The beacon's hex identity (15 digits, or 23 for the second generation).
    #[field(label = "Hex ID")]
    pub hex_id: &'a str,
    /// The beacon type (EPIRB, ELT, PLB ...).
    #[field(label = "Type")]
    pub beacon_type: &'a str,
    /// The country of the beacon's MID.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The country code (MID).
    #[field(label = "Country code")]
    pub mid: i64,
    /// The protocol in words.
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The identification in words.
    #[field(label = "Identification")]
    pub identification: &'a str,
    /// The 121.5 MHz homing flag.
    #[field(label = "121.5 MHz homing")]
    pub homing: bool,
    /// The self-test or test-protocol flag.
    #[field(label = "Test")]
    pub self_test: bool,
    /// The bits corrected.
    #[field(label = "Bits corrected")]
    pub bits_corrected: i64,
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The message bits as hexadecimal digits (as received after the repair).
    #[field(label = "Message")]
    pub message_hex: &'a str,
}

/// The burst log of COSPAS-SARSAT.
pub static SARSAT_BURSTS: Table<SarsatBurstRow<'static>> =
    Table::new("bursts", "Bursts", sarsat::LOG_CAP).label(&SarsatBurstRow::HEX_ID);
/// The beacons heard.
pub static SARSAT_BEACONS: Table<SarsatBeaconRow<'static>> =
    Table::new("beacons", "Beacons", sarsat::BEACON_CAP).label(&SarsatBeaconRow::HEX_ID);
/// A beacon decoded.
pub static SARSAT_BEACON: EventKind<SarsatBeaconEvent<'static>> =
    EventKind::new("sarsat.beacon", "Beacon decoded");

/// The arguments of `remove` of COSPAS-SARSAT.
#[derive(Arguments)]
pub struct SarsatBeaconArgs<'a> {
    /// The row key of the beacon.
    #[arg(shared = SARSAT_BEACON_ROW)]
    pub row: &'a str,
}

/// The row key of a COSPAS-SARSAT beacon.
pub const SARSAT_BEACON_ROW: Argument<field::Text> = objcmd::row_of("beacons");
/// Forgets one COSPAS-SARSAT beacon.
pub const SARSAT_REMOVE: CommandKind<SarsatBeaconArgs<'static>> = objcmd::remove_kind();

/// The arguments of `preset.apply` of COSPAS-SARSAT.
#[derive(Arguments)]
pub struct SarsatPresetArgs {
    /// The dial preset.
    #[arg(shared = sarsat::PRESET)]
    pub preset: Option<sarsat::Preset>,
}

/// Asks the host to tune the dial to a COSPAS-SARSAT preset.
pub const SARSAT_PRESET_APPLY: CommandKind<SarsatPresetArgs> =
    CommandKind::new("preset.apply", "Apply preset")
        .help("Asks the host to tune the dial to the preset.");

/// The result fields of the LoRa satellite stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatFields<'a> {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The pass planner's name of the satellite overhead.
    #[field(label = "Satellite")]
    pub satellite: &'a str,
    /// The name of the preset that runs.
    #[field(label = "Preset")]
    pub preset: Option<&'a str>,
    /// The number of detectors without Doppler tracking.
    #[field(label = "Demodulators")]
    pub demodulators: i64,
    /// The largest Doppler shift expected.
    #[field(label = "Doppler allowance", unit = "Hz")]
    pub doppler_max_hz: Hertz,
    /// The largest Doppler rate expected.
    #[field(label = "Doppler rate allowance", unit = "Hz/s")]
    pub doppler_rate_max_hz_s: f64,
}

/// A frame of the log. The row key is the sequence number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatFrameRow<'a> {
    /// The wall-clock time a frame was heard (live radios only).
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The spreading factor of a frame.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth of a frame.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw_hz: Hertz,
    /// The coding rate of a frame: the denominator n of 4/n.
    #[field(label = "Coding rate 4/")]
    pub cr: i64,
    /// The CRC of a frame.
    #[field(label = "CRC")]
    pub crc: CrcState,
    /// The sync word of a frame.
    #[field(label = "Sync word")]
    pub sync: i64,
    /// The payload length of a frame.
    #[field(label = "Length")]
    pub bytes: i64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier offset from the dial at the first data symbol.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler_hz: Hertz,
    /// The carrier rate during a frame.
    #[field(label = "Doppler rate", unit = "Hz/s")]
    pub doppler_rate_hz_s: Option<f64>,
    /// The frequency of the frame.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The pass planner's name of the satellite overhead.
    #[field(label = "Satellite")]
    pub satellite: Option<&'a str>,
    /// The payload of a frame.
    #[field(label = "Payload", content = ContentType::Binary)]
    pub payload: Arc<[u8]>,
    /// The payload as printable text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A demodulator running now. The row key is its place in the list ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatDemodulatorRow {
    /// The spreading factor of a frame.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth of a frame.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw_hz: Hertz,
    /// Detector counters: preambles found.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Detector counters: preambles without a sync word or SFD.
    #[field(label = "Sync failed")]
    pub sync_failed: i64,
    /// Detector counters: headers that failed.
    #[field(label = "Header failed")]
    pub header_failed: i64,
    /// Detector counters: frames decoded, good or not.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Detector counters: frames whose CRC failed.
    #[field(label = "CRC failed")]
    pub crc_failed: i64,
}

/// A sync word of a demodulator. The row key is the demodulator and the sync word ("000/18").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatDemodulatorSyncRow<'a> {
    /// The detector a sync word belongs to.
    #[field(label = "Demodulator", refers_to = "demodulators")]
    pub demodulator: &'a str,
    /// The sync word of a frame.
    #[field(label = "Sync word")]
    pub sync: i64,
}

/// The frames of one spreading factor. The row key is the SF ("07").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatSfRow {
    /// The spreading factor of a frame.
    #[field(label = "SF")]
    pub sf: i64,
    /// Detector counters: frames decoded, good or not.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// A frame with a good CRC. The source is the satellite overhead, the text the payload as printable text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraSatFrameEvent<'a> {
    /// The satellite overhead.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The payload as printable text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The spreading factor of a frame.
    #[field(label = "SF")]
    pub sf: i64,
    /// The payload of a frame.
    #[field(label = "Payload", content = ContentType::Binary)]
    pub payload: Arc<[u8]>,
    /// The carrier offset from the dial at the first data symbol.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler_hz: Hertz,
    /// The carrier rate during a frame.
    #[field(label = "Doppler rate", unit = "Hz/s")]
    pub doppler_rate_hz_s: Option<f64>,
}

/// The frame log of the LoRa satellite stack.
pub static LORA_SAT_FRAMES: Table<LoraSatFrameRow<'static>> =
    Table::new("frames", "Frames", lorasat::display::FRAMES_CAP).label(&LoraSatFrameRow::SATELLITE);
/// The demodulators running now.
pub static LORA_SAT_DEMODULATORS: Table<LoraSatDemodulatorRow> =
    Table::new("demodulators", "Demodulators", 64);
/// The sync words of the demodulators.
pub static LORA_SAT_DEMODULATOR_SYNCS: Table<LoraSatDemodulatorSyncRow<'static>> =
    Table::new("demodulator_syncs", "Sync words of the demodulators", 256);
/// The frames by spreading factor.
pub static LORA_SAT_FRAMES_PER_SF: Table<LoraSatSfRow> =
    Table::new("frames_per_sf", "Frames by SF", 16);
/// A LoRa frame from a satellite.
pub static LORA_SAT_FRAME: EventKind<LoraSatFrameEvent<'static>> =
    EventKind::new("lora_sat.frame", "LoRa frame");
/// The Doppler curve of the pass.
pub const LORA_SAT_DOPPLER: SeriesDef = SeriesDef {
    key: "doppler",
    label: "Doppler curve",
    x_unit: "s",
    y_unit: "Hz",
};

/// The result fields of Inmarsat-C.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatcFields<'a> {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The offset of the carrier of the first channel from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The drift.
    #[field(shared = keys::DRIFT_HZ_S)]
    pub drift_hz_s: f64,
    /// The frames found.
    #[field(label = "Frames found")]
    pub frames_found: i64,
    /// The packets good.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// The packets bad.
    #[field(label = "Packets bad")]
    pub packets_bad: i64,
    /// The messages seen.
    #[field(label = "Messages seen")]
    pub message_count: i64,
    /// The es/N0.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// The eb/N0.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: Option<f64>,
    /// The unique word errors.
    #[field(label = "Unique word errors")]
    pub uw_errors: Option<i64>,
    /// The unique word errors (average).
    #[field(label = "Unique word errors (average)")]
    pub uw_errors_avg: Option<f64>,
    /// The symbol error rate.
    #[field(label = "Symbol error rate")]
    pub symbol_error_rate: Option<f64>,
    /// The polarity slips.
    #[field(label = "Polarity slips")]
    pub polarity_slips: i64,
    /// The ocean region code.
    #[field(label = "Ocean region code")]
    pub ocean_region_code: Option<i64>,
    /// The ocean region.
    #[field(label = "Ocean region")]
    pub region: Option<&'a str>,
    /// The station id.
    #[field(label = "Station id")]
    pub station_id: Option<i64>,
    /// The station.
    #[field(label = "Station")]
    pub station_name: Option<&'a str>,
    /// The channel type code.
    #[field(label = "Channel type code")]
    pub channel_type_code: Option<i64>,
    /// The channel type.
    #[field(label = "Channel type")]
    pub channel_type: Option<&'a str>,
    /// The network version.
    #[field(label = "Network version")]
    pub network_version: Option<i64>,
    /// The signalling channel.
    #[field(label = "Signalling channel")]
    pub signalling_channel: Option<i64>,
    /// The random interval.
    #[field(label = "Random interval")]
    pub random_interval: Option<i64>,
    /// The status byte.
    #[field(label = "Status byte")]
    pub status_byte: Option<i64>,
    /// The status.
    #[field(label = "Status")]
    pub status_text: Option<&'a str>,
    /// The services mask.
    #[field(label = "Services mask")]
    pub services_mask: Option<i64>,
    /// The services.
    #[field(label = "Services")]
    pub services_text: Option<&'a str>,
    /// The signalling uplink.
    #[field(label = "Signalling uplink", unit = "Hz")]
    pub signalling_uplink_hz: Option<Hertz>,
    /// The frame number.
    #[field(label = "Frame number")]
    pub frame_number: Option<i64>,
    /// The frame time of day.
    #[field(label = "Frame time of day", unit = "s")]
    pub frame_time: Option<Seconds>,
}

/// An EGC message. The row key is the message number and the service.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatcMessageRow<'a> {
    /// The first heard.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The channel a message came on (0 the main one).
    #[field(label = "Channel", refers_to = "channels")]
    pub channel: i64,
    /// The message number.
    #[field(label = "Number")]
    pub message_number: i64,
    /// The EGC service code.
    #[field(label = "Service code")]
    pub service_code: i64,
    /// The service.
    #[field(label = "Service")]
    pub service: &'a str,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The priority.
    #[field(label = "Priority")]
    pub priority: &'a str,
    /// The presentation.
    #[field(label = "Presentation")]
    pub presentation: &'a str,
    /// The packets of the message heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Every packet of the message was heard.
    #[field(label = "Complete")]
    pub complete: bool,
    /// The heard complete.
    #[field(label = "Heard complete")]
    pub times_heard: i64,
    /// The area of a message.
    #[field(label = "Area")]
    pub area: &'a str,
    /// The subject of a message.
    #[field(label = "Subject")]
    pub subject: &'a str,
    /// The warning.
    #[field(label = "Warning")]
    pub warning_number: &'a str,
    /// The text of the message.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: &'a str,
    /// The text of the message is cut short.
    #[field(label = "Truncated")]
    pub truncated: bool,
}

/// A channel followed now. The row key is the channel index.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatcChannelRow<'a> {
    /// The channel index.
    #[field(label = "Index")]
    pub index: i64,
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// A carrier was found.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// The state.
    #[field(label = "State")]
    pub state: ChannelState,
    /// The eb/N0.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: f64,
    /// The unique word errors (average).
    #[field(label = "Unique word errors (average)")]
    pub uw_errors_avg: f64,
    /// The frame number.
    #[field(label = "Frame number")]
    pub frame_number: i64,
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The messages.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The ocean region.
    #[field(label = "Ocean region")]
    pub region: &'a str,
    /// The station.
    #[field(label = "Station")]
    pub station_name: &'a str,
    /// The channel type.
    #[field(label = "Channel type")]
    pub channel_type: &'a str,
}

/// The packets of one descriptor type. The row key is the descriptor in hexadecimal ("2A").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatcPacketCountRow<'a> {
    /// The descriptor.
    #[field(label = "Descriptor")]
    pub descriptor: i64,
    /// The packet type.
    #[field(label = "Packet type")]
    pub descriptor_name: &'a str,
    /// The packets good.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// The packets bad.
    #[field(label = "Packets bad")]
    pub packets_bad: i64,
}

/// A complete EGC message. The source is the message number and the service, the text the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatcEgcEvent<'a> {
    /// The message number and the service.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent the message is, from its priority.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The message number.
    #[field(label = "Number")]
    pub message_number: i64,
    /// The EGC service code.
    #[field(label = "Service code")]
    pub service_code: i64,
    /// The EGC priority code (0 routine, 1 safety, 2 urgency, 3 distress).
    #[field(label = "Priority code")]
    pub priority_code: i64,
    /// The subject of a message.
    #[field(label = "Subject")]
    pub subject: &'a str,
    /// The area of a message.
    #[field(label = "Area")]
    pub area: &'a str,
}

/// The EGC messages of Inmarsat-C.
pub static INMARSATC_MESSAGES: Table<InmarsatcMessageRow<'static>> =
    Table::new("messages", "EGC messages", inmarsatc::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages);
/// The channels followed now.
pub static INMARSATC_CHANNELS: Table<InmarsatcChannelRow<'static>> =
    Table::new("channels", "Channels", inmarsatc::MAX_CHANNELS);
/// The packets by descriptor type.
pub static INMARSATC_PACKET_COUNTS: Table<InmarsatcPacketCountRow<'static>> =
    Table::new("packet_counts", "Packets by type", 64);
/// A complete EGC message.
pub static INMARSATC_EGC: EventKind<InmarsatcEgcEvent<'static>> =
    EventKind::new("inmarsatc.egc", "EGC message");

/// The result fields of Orbcomm.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommFields {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Messages joined from their parts.
    #[field(label = "Messages joined")]
    pub messages_joined: i64,
    /// Satellites heard.
    #[field(label = "Satellites")]
    pub satellites: i64,
}

/// A satellite heard. The row key is the satellite number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommSatelliteRow {
    /// The satellite.
    #[field(label = "Satellite")]
    pub satellite: i64,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The doppler.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Option<Hertz>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The frames.
    #[field(label = "Frames")]
    pub frames: i64,
    /// How many times it was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The bad packets.
    #[field(label = "Bad packets")]
    pub bad: i64,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The height.
    #[field(label = "Height", unit = "m", role = Altitude)]
    pub height_m: Option<f64>,
    /// The inclination.
    #[field(label = "Inclination", unit = "deg")]
    pub inclination_deg: Option<f64>,
    /// The period.
    #[field(label = "Period", unit = "s")]
    pub period: Option<Seconds>,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A channel that a satellite announces. The row key is `<satellite>/<u or d><n>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommSatelliteChannelRow {
    /// The satellite.
    #[field(label = "Satellite", refers_to = "satellites")]
    pub satellite: i64,
    /// The channel is an uplink (else a downlink).
    #[field(label = "Uplink")]
    pub uplink: bool,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
}

/// A decoder. The row key is the decoder number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommDecoderRow {
    /// The decoder.
    #[field(label = "Decoder")]
    pub decoder: i64,
    /// The state.
    #[field(label = "State")]
    pub state: DecoderState,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The offset.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Option<Hertz>,
    /// The doppler.
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler: Option<Hertz>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// The satellite.
    #[field(label = "Satellite")]
    pub satellite: Option<i64>,
    /// The clock error.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The frames.
    #[field(label = "Frames")]
    pub frames: Option<i64>,
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: Option<i64>,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: Option<i64>,
    /// The sync losses.
    #[field(label = "Sync losses")]
    pub sync_losses: Option<i64>,
}

/// A packet of the log. The row key is the sequence number of the packet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommPacketRow<'a> {
    /// The at (signal time).
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The time.
    #[field(label = "Time")]
    pub wall: Option<Time>,
    /// The satellite.
    #[field(label = "Satellite")]
    pub satellite: Option<i64>,
    /// The decoder.
    #[field(label = "Decoder")]
    pub decoder: i64,
    /// The type.
    #[field(label = "Type")]
    pub r#type: PacketType,
    /// The bytes.
    #[field(label = "Bytes", content = ContentType::Binary)]
    pub bytes: Option<Arc<[u8]>>,
    /// What the packet says.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The packets of one type. The row key is the key of the type.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommPacketTypeRow {
    /// The type.
    #[field(label = "Type")]
    pub r#type: PacketType,
    /// The count.
    #[field(label = "Count")]
    pub count: i64,
}

/// A message joined from its parts. The source is the satellite ("satellite 12").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OrbcommMessageEvent<'a> {
    /// The satellite ("satellite 12").
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The message number of a joined message.
    #[field(label = "Message")]
    pub message_id: i64,
    /// Whether every part of a joined message came.
    #[field(label = "Complete")]
    pub complete: bool,
}

/// The satellites heard.
pub static ORBCOMM_SATELLITES: Table<OrbcommSatelliteRow> =
    Table::new("satellites", "Satellites", orbcomm::SATELLITES_CAP).sorted(
        &OrbcommSatelliteRow::SNR_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// The channels that each satellite announces.
pub static ORBCOMM_SATELLITE_CHANNELS: Table<OrbcommSatelliteChannelRow> = Table::new(
    "satellite_channels",
    "Satellite channels",
    orbcomm::SATELLITES_CAP * 16,
);
/// The decoders.
pub static ORBCOMM_DECODERS: Table<OrbcommDecoderRow> = Table::new("decoders", "Decoders", 2);
/// The packet log, newest first.
pub static ORBCOMM_PACKETS: Table<OrbcommPacketRow<'static>> =
    Table::new("packets", "Packets", orbcomm::LOG_CAP);
/// The packets counted by type.
pub static ORBCOMM_PACKET_TYPES: Table<OrbcommPacketTypeRow> =
    Table::new("packet_types", "Packet types", 9);
/// An Orbcomm message joined from its parts.
pub static ORBCOMM_MESSAGE: EventKind<OrbcommMessageEvent<'static>> =
    EventKind::new("orbcomm.message", "Message");
/// The constellation of the first active decoder (I, Q after the differential detector).
pub const ORBCOMM_CONSTELLATION: SeriesDef = SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
};

/// The arguments of `remove` of Orbcomm.
#[derive(Arguments)]
pub struct OrbcommSatelliteArgs<'a> {
    /// The row key of the satellite.
    #[arg(shared = ORBCOMM_SATELLITE_ROW)]
    pub row: &'a str,
}

/// The row key of an Orbcomm satellite.
pub const ORBCOMM_SATELLITE_ROW: Argument<field::Text> = objcmd::row_of("satellites");
/// Forgets one Orbcomm satellite.
pub const ORBCOMM_REMOVE: CommandKind<OrbcommSatelliteArgs<'static>> = objcmd::remove_kind();

/// The arguments of `preset.apply` of Orbcomm.
#[derive(Arguments)]
pub struct OrbcommChannelArgs {
    /// The channel preset.
    #[arg(shared = orbcomm::CHANNEL)]
    pub channel: Option<orbcomm::Channel>,
}

/// Asks the host to tune the dial to an Orbcomm channel preset.
pub const ORBCOMM_PRESET_APPLY: CommandKind<OrbcommChannelArgs> =
    CommandKind::new("preset.apply", "Tune to channel")
        .help("Asks the host to tune the dial to the channel.");

/// The result fields of the PSK demodulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodFields<'a> {
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The modulation order the demodulator runs.
    #[field(label = "Order")]
    pub order: Option<OrderChoice>,
    /// The carrier loop's lock value (1 on the points).
    #[field(label = "Lock")]
    pub lock: Option<f64>,
    /// The carrier, from the dial.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Option<Hertz>,
    /// The drift.
    #[field(shared = keys::DRIFT_HZ_S)]
    pub drift_hz_s: Option<f64>,
    /// The EVM.
    #[field(shared = keys::EVM_PCT)]
    pub evm_pct: Option<f64>,
    /// Es/N0 from the error vector.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// Eb/N0 from the error vector.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: Option<f64>,
    /// Symbols demodulated since the lock.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// Bits since the start.
    #[field(label = "Bits")]
    pub bits: i64,
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The newest bits, oldest first, as the characters 0 and 1.
    #[field(label = "Bits")]
    pub bit_stream: &'a str,
    /// The index of the first bit of `bit_stream`.
    #[field(label = "First bit")]
    pub bits_first: i64,
    /// The phase rotation the sync words resolved.
    #[field(label = "Rotation", unit = "deg")]
    pub rotation_deg: f64,
    /// Whether the sync words resolved a spectral inversion.
    #[field(label = "Spectrum inverted")]
    pub conjugated: bool,
    /// Whether the sync words resolved an OQPSK rail shift.
    #[field(label = "Rail shift")]
    pub rail_shift: bool,
    /// Whether the phase ambiguity is resolved.
    #[field(label = "Resolved")]
    pub resolved: bool,
    /// The centre of the signal found, from the dial.
    #[field(label = "Signal", unit = "Hz")]
    pub signal_hz: Option<Hertz>,
    /// Its width at 3 dB.
    #[field(label = "Width at 3 dB", unit = "Hz")]
    pub width_3db_hz: Option<Hertz>,
    /// Its width at 10 dB.
    #[field(label = "Width at 10 dB", unit = "Hz")]
    pub width_10db_hz: Option<Hertz>,
    /// Its 99 % occupied bandwidth.
    #[field(label = "Occupied bandwidth", unit = "Hz")]
    pub occupied_hz: Option<Hertz>,
    /// Its in-band SNR.
    #[field(label = "In-band SNR", unit = "dB")]
    pub signal_snr_db: Option<f64>,
    /// The symbol rate the analyser measured, before snapping.
    #[field(label = "Measured rate", unit = "Hz")]
    pub baud_raw_hz: Option<Hertz>,
    /// Whether the rate was snapped to a standard one.
    #[field(label = "Snapped")]
    pub snapped: Option<bool>,
    /// The confidence of the rate, 0 to 1.
    #[field(label = "Rate confidence")]
    pub rate_confidence: Option<f64>,
    /// The confidence of the order, 0 to 1.
    #[field(label = "Order confidence")]
    pub order_confidence: Option<f64>,
    /// The pulse shape.
    #[field(label = "Shape")]
    pub shape: Option<ShapeFound>,
    /// The roll-off of root raised cosine pulses.
    #[field(label = "Roll-off")]
    pub roll_off: Option<f64>,
    /// The envelope line over the median.
    #[field(label = "Envelope line", unit = "dB")]
    pub envelope_db: Option<f64>,
    /// The squared signal's line over the median.
    #[field(label = "x^2 line", unit = "dB")]
    pub line_x2_db: Option<f64>,
    /// The fourth power's line over the median.
    #[field(label = "x^4 line", unit = "dB")]
    pub line_x4_db: Option<f64>,
    /// The eighth power's line over the median.
    #[field(label = "x^8 line", unit = "dB")]
    pub line_x8_db: Option<f64>,
    /// What the analyser said when it found no PSK signal.
    #[field(label = "Note")]
    pub note: Option<Note>,
}

/// A rate candidate of the analyser. The row key is its place ("0").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodRateRow {
    /// A rate candidate.
    #[field(label = "Rate", unit = "Hz")]
    pub rate_hz: Hertz,
    /// A line's strength over the median.
    #[field(label = "Strength", unit = "dB")]
    pub strength_db: f64,
}

/// A trial demodulation of the analyser. The row key is its place ("00").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodTrialRow {
    /// The modulation order the demodulator runs.
    #[field(label = "Order")]
    pub order: OrderChoice,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The error vector of a trial, unit power.
    #[field(label = "EVM")]
    pub evm: f64,
}

/// The bit index after a recent sync word, oldest first. The row key is its place ("00").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodSyncRow {
    /// The bit index after a sync word.
    #[field(label = "Bit")]
    pub bit: i64,
}

/// A line of the log of analyses. The row key is the sequence number of the line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodLogRow<'a> {
    /// The signal time of a line of the log.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A signal was locked.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodLockEvent<'a> {
    /// The order and the symbol rate.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The modulation order the demodulator runs.
    #[field(label = "Order")]
    pub order: Option<OrderChoice>,
}

/// Bits packed into bytes for the host. The text is the bytes in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskDemodBytesEvent<'a> {
    /// The bytes in hexadecimal.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The bytes of a bytes event.
    #[field(label = "Bytes", content = ContentType::Binary)]
    pub bytes: Arc<[u8]>,
}

/// The rate candidates of the analyser.
pub static PSK_DEMOD_RATES: Table<PskDemodRateRow> = Table::new("rates", "Rate candidates", 8);
/// The trial demodulations of the analyser.
pub static PSK_DEMOD_TRIALS: Table<PskDemodTrialRow> = Table::new("trials", "Order trials", 16);
/// The bit indices after the recent sync words, oldest first.
pub static PSK_DEMOD_SYNCS: Table<PskDemodSyncRow> =
    Table::new("syncs", "Sync words at", psk::display::SYNCS_CAP);
/// The log of analyses.
pub static PSK_DEMOD_LOG: Table<PskDemodLogRow<'static>> =
    Table::new("log", "Log", psk::display::LOG_CAP);
/// A signal was locked.
pub static PSK_DEMOD_LOCK: EventKind<PskDemodLockEvent<'static>> =
    EventKind::new("psk_demod.lock", "Signal locked");
/// Bits packed into bytes for the host.
pub static PSK_DEMOD_BYTES: EventKind<PskDemodBytesEvent<'static>> =
    EventKind::new("psk_demod.bytes", "Bits as bytes");
/// The constellation (I, Q).
pub const PSK_DEMOD_CONSTELLATION: SeriesDef = SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
};
/// The eye: the I rail of the channel, 16 samples a trace.
pub const PSK_DEMOD_EYE: SeriesDef = SeriesDef {
    key: "eye",
    label: "Eye of the I rail",
    x_unit: "samples",
    y_unit: "",
};
/// The envelope spectrum (0 to half the channel rate), dB over the median.
pub const PSK_DEMOD_ENVELOPE: SeriesDef = SeriesDef {
    key: "envelope_spectrum",
    label: "Envelope spectrum",
    x_unit: "bins",
    y_unit: "dB",
};
/// The M-th power spectrum (centred), dB over the median.
pub const PSK_DEMOD_POWER: SeriesDef = SeriesDef {
    key: "power_spectrum",
    label: "M-th power spectrum",
    x_unit: "bins",
    y_unit: "dB",
};

/// The result fields of QO-100.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100Fields {
    /// The LNB's LO error (actual minus nominal).
    #[field(label = "LNB error", unit = "Hz")]
    pub lnb_error_hz: Hertz,
    /// The LNB error in parts per million of the nominal LO.
    #[field(label = "LNB error", unit = "ppm")]
    pub lnb_error_ppm: f64,
    /// The drift.
    #[field(shared = keys::DRIFT_HZ_S)]
    pub drift_hz_s: f64,
    /// The phase of the beacon lock.
    #[field(label = "Beacon lock")]
    pub beacon_lock: LockPhase,
    /// Whether the LNB error is applied to the frequencies shown.
    #[field(label = "Correction applied")]
    pub correction_applied: bool,
    /// Fine estimates that found a CW beacon.
    #[field(label = "Measurements")]
    pub measurements: i64,
    /// The locked for.
    #[field(shared = keys::LOCKED_FOR)]
    pub locked_for: Seconds,
    /// The USB dial for listening (RF).
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Hertz,
    /// The conversation followed by the dial.
    #[field(label = "Listening to", refers_to = "conversations")]
    pub listening: Option<i64>,
    /// Whether the wideband transponder is outside the radio's band.
    #[field(label = "Wideband outside the band")]
    pub wideband_outside: bool,
    /// The noise floor of the narrowband spectrum.
    #[field(label = "Narrowband floor", unit = "dBFS")]
    pub narrow_floor_dbfs: Option<f64>,
    /// The level of the strongest signal in the 2.7 kHz above the dial less the level of the PSK beacon, both as the peak
    /// of a spectrum bin. While the stack sends, that signal is the user's own downlink: it must stay below 0 dB. With
    /// nothing on the dial it is the level of the noise peaks there. No value while the beacon is not found.
    #[field(label = "Own signal against the beacon", unit = "dB")]
    pub own_to_beacon_db: Option<f64>,
}

/// The result fields of the voice sender of QO-100. The measurements have no value while the stack does not send.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100TxFields {
    /// Whether the stack transmits now.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The uplink frequency of the dial: the dial less the translation of the transponder.
    #[field(label = "Uplink frequency", unit = "Hz")]
    pub tx_freq_hz: Hertz,
    /// The time on air of the transmission.
    #[field(label = "Time on air", unit = "s")]
    pub tx_time_s: Option<Seconds>,
    /// The peak of the microphone audio over the last report.
    #[field(label = "Microphone peak", unit = "dBFS")]
    pub tx_mic_dbfs: Option<f64>,
    /// The peak of the modulation over the last report, against the peak level.
    #[field(label = "Modulation peak", unit = "dB")]
    pub tx_peak_db: Option<f64>,
    /// The gain of the level control of the speech processor.
    #[field(label = "Level gain", unit = "dB")]
    pub tx_level_db: Option<f64>,
    /// The most gain reduction of the limiter over the last report: 0 when it did not act.
    #[field(label = "Limiter", unit = "dB")]
    pub tx_limit_db: Option<f64>,
}

/// A beacon of the narrowband transponder. The row key is its place ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100BeaconRow<'a> {
    /// The name of a beacon.
    #[field(label = "Name")]
    pub name: &'a str,
    /// A nominal RF.
    #[field(label = "Nominal", unit = "Hz")]
    pub nominal_hz: Hertz,
    /// Whether a beacon was found.
    #[field(label = "Found")]
    pub found: bool,
    /// The RF where a beacon shows with no correction.
    #[field(label = "Apparent", unit = "Hz")]
    pub apparent_hz: Hertz,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A signal of the spectrum now. The row key is its place ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100SignalRow<'a> {
    /// The RF of a signal.
    #[field(label = "RF", unit = "Hz")]
    pub rf_hz: Hertz,
    /// The width of a signal.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The level of a signal.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The tracker's class of a signal.
    #[field(label = "Class")]
    pub class: SignalClass,
    /// The beacon a signal is.
    #[field(label = "Beacon")]
    pub beacon: Option<&'a str>,
    /// The id of a track.
    #[field(label = "Track", refers_to = "tracks")]
    pub track: Option<i64>,
}

/// A signal followed over time. The row key is the track number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100TrackRow {
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The RF of a signal.
    #[field(label = "RF", unit = "Hz")]
    pub rf_hz: Hertz,
    /// The width of a signal.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The level of a signal.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Whether a track or a conversation is active now.
    #[field(label = "Active")]
    pub active: bool,
    /// The share of the last 30 s a track was on.
    #[field(label = "On", unit = "%")]
    pub duty_pct: f64,
    /// The tracker's class of a signal.
    #[field(label = "Class")]
    pub class: SignalClass,
    /// The conversation of a track.
    #[field(label = "Conversation", refers_to = "conversations")]
    pub conversation: Option<i64>,
}

/// A conversation: SSB tracks grouped by frequency and time. The row key is the conversation number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100ConversationRow {
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Whether a track or a conversation is active now.
    #[field(label = "Active")]
    pub active: bool,
    /// The USB dial for listening (RF).
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Hertz,
    /// The lower edge of a conversation.
    #[field(label = "Lower edge", unit = "Hz")]
    pub lower_edge_hz: Hertz,
    /// The level of a signal.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The peak level of a conversation.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The stations estimated in a conversation.
    #[field(label = "Stations")]
    pub stations: i64,
    /// The talk time of a conversation.
    #[field(label = "Talk", unit = "s")]
    pub talk: Seconds,
    /// How long a conversation has lasted.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
}

/// A callsign read in a conversation. The row key is the conversation and the callsign ("3/G4ABC").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100CallsignRow<'a> {
    /// The conversation of a track.
    #[field(label = "Conversation", refers_to = "conversations")]
    pub conversation: i64,
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
}

/// A DATV carrier of the wideband transponder. The row key is its place ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100DatvCarrierRow {
    /// The RF of a signal.
    #[field(label = "RF", unit = "Hz")]
    pub rf_hz: Hertz,
    /// The centre of a DATV carrier in the input.
    #[field(label = "IF", unit = "Hz")]
    pub if_hz: Hertz,
    /// The width of a signal.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Whether a DATV carrier has a flat top.
    #[field(label = "Flat-topped")]
    pub flat_topped: bool,
    /// Whether a DATV carrier is the beacon.
    #[field(label = "Beacon")]
    pub is_beacon: bool,
}

/// The beacons were locked.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100LockEvent<'a> {
    /// The LNB error at the lock.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The LNB's LO error (actual minus nominal).
    #[field(label = "LNB error", unit = "Hz")]
    pub lnb_error_hz: Hertz,
    /// The LNB error in parts per million of the nominal LO.
    #[field(label = "LNB error", unit = "ppm")]
    pub lnb_error_ppm: f64,
}

/// A conversation started. The channel is the conversation number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Qo100ConversationEvent<'a> {
    /// "conversation started".
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The channel.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The conversation of a track.
    #[field(label = "Conversation", refers_to = "conversations")]
    pub conversation: i64,
    /// The USB dial for listening (RF).
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Hertz,
}

/// The beacons of the narrowband transponder.
pub static QO100_BEACONS: Table<Qo100BeaconRow<'static>> =
    Table::new("beacons", "Beacons", 3).label(&Qo100BeaconRow::NAME);
/// The signals of the spectrum now.
pub static QO100_SIGNALS: Table<Qo100SignalRow<'static>> = Table::new("signals", "Signals", 256)
    .sorted(&Qo100SignalRow::SNR_DB, sdrkit_core::SortOrder::Descending);
/// The signals followed over time.
pub static QO100_TRACKS: Table<Qo100TrackRow> =
    Table::new("tracks", "Tracks", qo100::display::TRACKS_CAP);
/// The conversations.
pub static QO100_CONVERSATIONS: Table<Qo100ConversationRow> = Table::new(
    "conversations",
    "Conversations",
    qo100::display::CONVERSATIONS_CAP,
);
/// The callsigns read in the conversations.
pub static QO100_CALLSIGNS: Table<Qo100CallsignRow<'static>> = Table::new(
    "conversation_callsigns",
    "Callsigns heard",
    qo100::display::CONVERSATIONS_CAP * 8,
)
.label(&Qo100CallsignRow::CALLSIGN);
/// The DATV carriers of the wideband transponder.
pub static QO100_DATV: Table<Qo100DatvCarrierRow> = Table::new("datv", "DATV carriers", 32);
/// The beacons of QO-100 were locked.
pub static QO100_LOCK: EventKind<Qo100LockEvent<'static>> =
    EventKind::new("qo100.lock", "Beacon lock");
/// A conversation started on QO-100.
pub static QO100_CONVERSATION: EventKind<Qo100ConversationEvent<'static>> =
    EventKind::new("qo100.conversation", "Conversation started");
/// The narrowband spectrum: x is Hz above 10489.5 MHz (corrected RF), y dBFS.
pub const QO100_NARROWBAND: SeriesDef = SeriesDef {
    key: "narrowband",
    label: "Narrowband spectrum (Hz above 10489.5 MHz)",
    x_unit: "Hz",
    y_unit: "dBFS",
};
/// The wideband spectrum: x is Hz above 10489.5 MHz, y dBFS.
pub const QO100_WIDEBAND: SeriesDef = SeriesDef {
    key: "wideband",
    label: "Wideband spectrum (Hz above 10489.5 MHz)",
    x_unit: "Hz",
    y_unit: "dBFS",
};
/// The LNB error: x is signal time in seconds, y Hz.
pub const QO100_LNB_ERROR: SeriesDef = SeriesDef {
    key: "lnb_error",
    label: "LNB error",
    x_unit: "s",
    y_unit: "Hz",
};

/// The arguments of `speech.text` of QO-100.
#[derive(Arguments)]
pub struct Qo100SpeechArgs<'a> {
    /// The text that the host's speech-to-text heard.
    #[arg(
        label = "Text",
        help = "The text that the host's speech-to-text heard."
    )]
    pub text: &'a str,
}

/// Gives the text that the host's speech-to-text heard. The callsigns in it go to the active conversation.
pub const QO100_SPEECH_TEXT: CommandKind<Qo100SpeechArgs<'static>> =
    CommandKind::new("speech.text", "Speech-to-text")
        .help("Gives the text that the host's speech-to-text heard. Callsigns are read from it.");

/// The arguments of `conversation.listen` of QO-100.
#[derive(Arguments)]
pub struct Qo100ListenArgs<'a> {
    /// The row key of a conversation, or empty for the dial.
    #[arg(
        label = "Conversation",
        refers_to = "conversations",
        help = "The row key of a conversation in the conversations table. Empty: the dial."
    )]
    pub row: &'a str,
}

/// Makes the dial follow a conversation, or the dial alone.
pub const QO100_CONVERSATION_LISTEN: CommandKind<Qo100ListenArgs<'static>> =
    CommandKind::new("conversation.listen", "Listen to conversation")
        .help("Makes the dial follow a conversation. An empty row returns to the dial.");

/// The arguments of `remove` of QO-100.
#[derive(Arguments)]
pub struct Qo100ConversationArgs<'a> {
    /// The row key of the conversation.
    #[arg(shared = QO100_CONVERSATION_ROW)]
    pub row: &'a str,
}

/// The row key of a QO-100 conversation.
pub const QO100_CONVERSATION_ROW: Argument<field::Text> = objcmd::row_of("conversations");
/// Forgets one QO-100 conversation.
pub const QO100_REMOVE: CommandKind<Qo100ConversationArgs<'static>> = objcmd::remove_kind();
/// Push-to-talk: speech from the microphone input in upper sideband on the uplink of the dial.
pub const QO100_PTT: CommandKind<sdrkit_components::voice::PttArgs> = sdrkit_components::voice::ptt(
    "With on true, sends the speech of the microphone input in upper sideband on the uplink of the dial until ptt with on false or the time-out. Transmit must be on.",
);

/// The result fields of the CubeSat telemetry stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatFields<'a> {
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames the Reed-Solomon code repaired.
    #[field(label = "Repaired by RS")]
    pub rs_repaired: i64,
    /// Frames sent to the KISS output.
    #[field(label = "Sent to KISS")]
    pub kiss_sent: i64,
    /// Which parts of the framings received are not confirmed by a public document.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A frame of the log. The row key is the sequence number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatFrameRow<'a> {
    /// The wall-clock time a frame was heard (live radios only).
    #[field(label = "Heard at")]
    pub heard_at: Option<Time>,
    /// The framing of a frame.
    #[field(label = "Framing")]
    pub framing: Framing,
    /// The bit rate of a frame.
    #[field(label = "Baud")]
    pub baud: i64,
    /// Reed-Solomon symbols corrected (none for a framing without Reed-Solomon).
    #[field(label = "RS corrected")]
    pub rs_corrected: Option<i64>,
    /// The satellite of a frame.
    #[field(label = "Satellite")]
    pub satellite: Option<&'a str>,
    /// Its NORAD number.
    #[field(label = "NORAD")]
    pub norad: i64,
    /// The name comes from the dial frequency.
    #[field(label = "Named from the dial")]
    pub guess: bool,
    /// The frequency of the frame.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// AX.25 source.
    #[field(label = "From")]
    pub from: Option<&'a str>,
    /// AX.25 destination.
    #[field(label = "To")]
    pub to: Option<&'a str>,
    /// AX.25 repeaters, separated by commas.
    #[field(label = "Path")]
    pub path: Option<&'a str>,
    /// AX.25 frame type.
    #[field(label = "Type")]
    pub frame_type: Option<&'a str>,
    /// AX.25 PID.
    #[field(label = "PID")]
    pub pid: Option<i64>,
    /// The CSP header in words.
    #[field(label = "CSP header")]
    pub csp: Option<&'a str>,
    /// The frame's data.
    #[field(label = "Data", content = ContentType::Binary)]
    pub data: Arc<[u8]>,
    /// The payload.
    #[field(label = "Payload", content = ContentType::Binary)]
    pub payload: Arc<[u8]>,
    /// The text beacon.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What passed the checks of a frame.
    #[field(label = "Validated")]
    pub validated: Option<&'a str>,
    /// The Lucky-7 kind of a frame.
    #[field(label = "Kind")]
    pub kind: Option<&'a str>,
    /// The sync word was found complemented.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The spacecraft id of a CCSDS header.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The virtual channel of a CCSDS header.
    #[field(label = "Virtual channel")]
    pub vc: Option<i64>,
    /// The NGHam flags.
    #[field(label = "Flags")]
    pub flags: Option<i64>,
}

/// The frames of one framing. The row key is the key of the framing.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatFramingRow {
    /// The framing of a frame.
    #[field(label = "Framing")]
    pub framing: Framing,
    /// Frames by framing.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Sync word matches of a framing.
    #[field(label = "Sync matches")]
    pub sync_hits: Option<i64>,
    /// Candidates of a framing that failed their checks.
    #[field(label = "Failed decodes")]
    pub decode_failed: Option<i64>,
}

/// A carrier found now. The row key is its place ("000").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatCarrierRow {
    /// A carrier's offset from the dial.
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A frame. The source is the satellite, the text the text beacon or nothing.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatFrameEvent<'a> {
    /// The satellite.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The text beacon.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The framing of a frame.
    #[field(label = "Framing")]
    pub framing: Framing,
    /// Its NORAD number.
    #[field(label = "NORAD")]
    pub norad: i64,
    /// The frame's data.
    #[field(label = "Data", content = ContentType::Binary)]
    pub data: Arc<[u8]>,
}

/// A frame in KISS form for a packet program. The source is the satellite.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatKissEvent<'a> {
    /// The satellite.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The KISS frame.
    #[field(label = "KISS frame", content = ContentType::Binary)]
    pub kiss_frame: Arc<[u8]>,
}

/// The frame log of the CubeSat telemetry stack.
pub static CUBESAT_FRAMES: Table<CubesatFrameRow<'static>> =
    Table::new("frames", "Frames", cubesat::display::FRAMES_CAP).label(&CubesatFrameRow::SATELLITE);
/// The frames by framing.
pub static CUBESAT_FRAMES_PER_FRAMING: Table<CubesatFramingRow> =
    Table::new("frames_per_framing", "Frames by framing", 32);
/// The carriers found now.
pub static CUBESAT_CARRIERS: Table<CubesatCarrierRow> = Table::new("carriers", "Carriers", 16)
    .sorted(
        &CubesatCarrierRow::SNR_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// A CubeSat frame.
pub static CUBESAT_FRAME: EventKind<CubesatFrameEvent<'static>> =
    EventKind::new("cubesat.frame", "CubeSat frame");
/// A CubeSat frame in KISS form.
pub static CUBESAT_KISS: EventKind<CubesatKissEvent<'static>> =
    EventKind::new("cubesat.kiss", "KISS frame");

/// The result fields of the AX.25 sender of the CubeSat stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatTxFields {
    /// Whether a transmission is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The frames of the transmission on air.
    #[field(label = "Frames sent")]
    pub tx_sent: i64,
    /// The frames waiting for the next transmission.
    #[field(label = "Frames waiting")]
    pub tx_waiting: i64,
}

/// The fields of a change of an AX.25 transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CubesatTxEvent<'a> {
    /// The state: started, queued, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a frame was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The destination and the digipeaters of the frames, separated by commas.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The text or the hexadecimal bytes of the frame for started, queued and refused; empty for finished.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a satellite.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An AX.25 transmission.
pub static CUBESAT_TX: EventKind<CubesatTxEvent<'static>> =
    EventKind::new("cubesat.tx", "AX.25 transmission");

/// The arguments of the cubesat `send`.
#[derive(Arguments)]
pub struct CubesatSendArgs<'a> {
    /// The destination address.
    #[arg(
        label = "Destination",
        help = "An AX.25 address: 1 to 6 letters or digits and an optional SSID 0 to 15 (CALL-7). Empty sends to CQ.",
        default = "CQ"
    )]
    pub destination: Option<&'a str>,
    /// The digipeaters.
    #[arg(
        label = "Digipeaters",
        help = "Up to 8 AX.25 addresses separated by commas, for example the alias of a satellite digipeater.",
        default = ""
    )]
    pub path: Option<&'a str>,
    /// The information as text.
    #[arg(
        label = "Text",
        help = "The information field as its UTF-8 bytes, at most 256 octets. Give the text or the bytes.",
        default = ""
    )]
    pub text: Option<&'a str>,
    /// The information as bytes.
    #[arg(
        label = "Bytes",
        help = "The information field as pairs of hexadecimal digits (spaces allowed), at most 256 octets.",
        default = ""
    )]
    pub hex: Option<&'a str>,
    /// The protocol identifier.
    #[arg(
        label = "PID",
        help = "The PID octet, 0 to 254: 240 (F0) is no layer 3.",
        default = 240,
        min = 0,
        max = 254,
        step = 1
    )]
    pub pid: Option<i64>,
}

/// Sends an AX.25 UI frame.
pub const CUBESAT_SEND: CommandKind<CubesatSendArgs<'static>> =
    CommandKind::new("send", "Send a frame")
        .help(
            "Builds a UI frame from the station callsign to the destination and queues it. A transmission starts when none \
             is on air; frames sent during a transmission go in the next one. Transmit must be on and the station callsign set.",
        )
        .transmits(sdrkit_core::TxContent::Text)
        .stopped_by("halt");
/// Ends the transmission.
pub const CUBESAT_HALT: sdrkit_core::command::Command = sdrkit_core::command::Command::new(
    "halt", "Halt",
)
.help("Drops the frames waiting and ends the transmission after the current bit with a 5 ms fall.");

/// A row of the channel table of the Aero stacks. The row key is the place of the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroChannelRow {
    /// The id of the channel.
    #[field(label = "Id")]
    pub id: i64,
    /// The offset from the centre of the input.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The bit rate.
    #[field(label = "Rate", unit = "bit/s")]
    pub rate: i64,
    /// How far the channel decodes.
    #[field(label = "State")]
    pub state: aero::ChannelState,
    /// The energy per bit over the noise density.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: f64,
    /// The level of the carrier.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The frames read.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The frames whose unique word was missed.
    #[field(label = "Unique word misses")]
    pub uw_misses: i64,
    /// The signal units with a good check.
    #[field(label = "SUs good")]
    pub sus_good: i64,
    /// The signal units with a bad check.
    #[field(label = "SUs bad")]
    pub sus_bad: i64,
    /// The bit error rate of the channel.
    #[field(shared = keys::BER)]
    pub ber: f64,
}

/// The result fields of the Aero display.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroFields {
    /// The signal units with a good check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The signal units with a bad check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The frames read.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The messages in the log.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The positions heard.
    #[field(label = "Positions")]
    pub positions: i64,
    /// The aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// The channels that decode frames.
    #[field(label = "Channels decoding")]
    pub channels_decoding: i64,
    /// The best Eb/N0 of the channels that decode. Absent while none decodes.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: Option<f64>,
}

/// The count of one SU type. The row key is the type in two hexadecimal digits (FE for SUs that failed their check).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroSuTypeRow<'a> {
    /// The SU type.
    #[field(label = "SU type")]
    pub su_type: i64,
    /// The name of the type.
    #[field(label = "SU name")]
    pub su_name: &'a str,
    /// The SUs of the type.
    #[field(label = "Count")]
    pub count: i64,
}

/// A log-on or log-off. The row key is the number of the entry in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroLogonRow<'a> {
    /// The signal time.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time, when known.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The id of the channel.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The AES id, six hexadecimal digits.
    #[field(label = "AES")]
    pub aes: &'a str,
    /// The GES id, two hexadecimal digits.
    #[field(label = "GES")]
    pub ges: &'a str,
    /// A log-on (else a log-off).
    #[field(label = "Log-on")]
    pub logon: bool,
}

/// An ACARS message. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroMessageRow<'a> {
    /// The signal time.
    #[field(label = "At (signal time)", unit = "s")]
    pub at: Seconds,
    /// The wall-clock time, when known.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The id of the channel.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The bit rate of the channel.
    #[field(label = "Rate", unit = "bit/s")]
    pub rate: i64,
    /// The frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The AES id, six hexadecimal digits.
    #[field(label = "AES", role = Sender)]
    pub aes: &'a str,
    /// The GES id, two hexadecimal digits.
    #[field(label = "GES", role = Addressee)]
    pub ges: &'a str,
    /// The registration.
    #[field(shared = aircraft::REGISTRATION)]
    pub registration: &'a str,
    /// The flight id.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: &'a str,
    /// The ACARS label.
    #[field(shared = aircraft::LABEL)]
    pub label: &'a str,
    /// What the label means.
    #[field(label = "Label meaning")]
    pub label_name: &'a str,
    /// The message sequence number.
    #[field(label = "Message number")]
    pub message_number: &'a str,
    /// The text.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: &'a str,
    /// The block check is good.
    #[field(label = "Check good")]
    pub check_ok: bool,
    /// Every block of the message was heard.
    #[field(label = "Complete")]
    pub complete: bool,
    /// The ADS-C report in words. Empty for a message that is not ADS-C.
    #[field(label = "ADS-C")]
    pub adsc: &'a str,
    /// The position in the message.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude in the message.
    #[field(shared = aircraft::ALT_FT)]
    pub alt_ft: Option<f64>,
    /// The reading of the fixed format.
    #[field(label = "Reading")]
    pub reading: Option<&'a str>,
    /// The kind of the reading.
    #[field(label = "Reading kind")]
    pub reading_kind: Option<&'a str>,
    /// The category of the reading.
    #[field(label = "Reading category")]
    pub reading_category: Option<&'a str>,
    /// The CPDLC envelope: AT1, CR1, CC1 or DR1.
    #[field(label = "CPDLC envelope")]
    pub cpdlc_imi: Option<&'a str>,
    /// The CPDLC ground facility.
    #[field(label = "CPDLC ground facility")]
    pub cpdlc_ground: Option<&'a str>,
    /// The aircraft named in the CPDLC envelope.
    #[field(label = "CPDLC aircraft")]
    pub cpdlc_aircraft: Option<&'a str>,
    /// The CPDLC envelope check passed.
    #[field(label = "CPDLC check good")]
    pub cpdlc_crc_ok: Option<bool>,
    /// The CPDLC message body decoded completely.
    #[field(label = "CPDLC decoded")]
    pub cpdlc_parsed: Option<bool>,
    /// The decoded CPDLC message.
    #[field(label = "CPDLC text")]
    pub cpdlc_text: Option<&'a str>,
}

/// An aircraft heard: the standard aircraft columns, then the frequency last heard on. The row key is the address,
/// else the registration.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroAircraftRow<'a> {
    /// The 24-bit address, six hex digits.
    #[field(shared = aircraft::ADDRESS)]
    pub address: Option<&'a str>,
    /// The registration.
    #[field(shared = aircraft::REGISTRATION)]
    pub registration: Option<&'a str>,
    /// The State the address is allocated to.
    #[field(shared = aircraft::COUNTRY)]
    pub country: Option<&'a str>,
    /// The flight id.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: Option<&'a str>,
    /// The label of the last message.
    #[field(shared = aircraft::LABEL)]
    pub label: Option<&'a str>,
    /// Messages heard.
    #[field(shared = aircraft::MESSAGES)]
    pub messages: i64,
    /// The last position.
    #[field(shared = aircraft::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(shared = aircraft::ALT_FT)]
    pub alt_ft: Option<f64>,
    /// The track, degrees true.
    #[field(shared = aircraft::TRACK_DEG)]
    pub track_deg: Option<f64>,
    /// The ground speed.
    #[field(shared = aircraft::GS_KT)]
    pub gs_kt: Option<f64>,
    /// The vertical rate.
    #[field(shared = aircraft::VS_FPM)]
    pub vs_fpm: Option<f64>,
    /// Wall-clock time when last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frequency last heard on.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
}

/// A system fact (a satellite, the control of the P and R channels). The row key is the key of the fact.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroFactRow<'a> {
    /// The kind of the fact.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The fact in words.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// An ACARS message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroMessageEvent<'a> {
    /// The registration. Absent when the message names none.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The position in the message.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The ACARS label.
    #[field(shared = aircraft::LABEL)]
    pub label: &'a str,
    /// The AES id, six hexadecimal digits.
    #[field(label = "AES")]
    pub aes: &'a str,
    /// The flight id.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: &'a str,
    /// The block check is good.
    #[field(label = "Check good")]
    pub check_ok: bool,
}

/// A log-on or log-off.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AeroLogonEvent<'a> {
    /// The AES id, six hexadecimal digits.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// `log-on` or `log-off`.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The GES id, two hexadecimal digits.
    #[field(label = "GES")]
    pub ges: &'a str,
    /// A log-on (else a log-off).
    #[field(label = "Log-on")]
    pub logon: bool,
}

/// The channels that the Aero receiver found.
pub static AERO_CHANNELS: Table<AeroChannelRow> =
    Table::new("channels", "Channels", aero::MAX_CHANNELS);
/// The SU types heard.
pub static AERO_SU_TYPES: Table<AeroSuTypeRow<'static>> = Table::new("su_types", "SU types", 64);
/// Log-ons and log-offs.
pub static AERO_LOGONS: Table<AeroLogonRow<'static>> =
    Table::new("logons", "Log-ons", aero::LOGON_CAP);
/// The ACARS messages.
pub static AERO_MESSAGES: Table<AeroMessageRow<'static>> =
    Table::new("messages", "Messages", aero::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&AeroMessageRow::FLIGHT);
/// The aircraft heard.
pub static AERO_AIRCRAFT: Table<AeroAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", aero::AIRCRAFT_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&AeroAircraftRow::FLIGHT);
/// The system facts.
pub static AERO_FACTS: Table<AeroFactRow<'static>> = Table::new("facts", "System", aero::FACT_CAP);
/// An ACARS message. The source is the registration.
pub static AERO_MESSAGE: EventKind<AeroMessageEvent<'static>> =
    EventKind::new("aero.message", "ACARS message");
/// A log-on or log-off. The source is the AES id.
pub static AERO_LOGON: EventKind<AeroLogonEvent<'static>> = EventKind::new("aero.logon", "Log-on");

/// The arguments of `remove` of the Aero stack.
#[derive(Arguments)]
pub struct AeroAircraftArgs<'a> {
    /// The row key of the aircraft.
    #[arg(shared = AERO_AIRCRAFT_ROW)]
    pub row: &'a str,
}

/// The row key of an aircraft of the Aero stack.
pub const AERO_AIRCRAFT_ROW: Argument<field::Text> = objcmd::row_of("aircraft");
/// Forgets one aircraft.
pub const AERO_REMOVE: CommandKind<AeroAircraftArgs<'static>> = objcmd::remove_kind();

/// The result fields of the merged Inmarsat display.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatFields<'a> {
    /// The ACARS messages and the Inmarsat-C bulletin frames.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The satellite that the identification names. Absent while it names none.
    #[field(label = "Satellite")]
    pub satellite: Option<&'a str>,
    /// How sure the identification is, 0 to 1.
    #[field(label = "Confidence")]
    pub satellite_confidence: Option<f64>,
    /// Two satellites fit about as well.
    #[field(label = "Ambiguous")]
    pub satellite_ambiguous: bool,
    /// The beam, in words.
    #[field(label = "Beam")]
    pub beam: &'a str,
    /// A note on the identification.
    #[field(label = "Note")]
    pub identification_note: &'a str,
    /// The ACARS messages.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The ACARS messages with a position.
    #[field(label = "Positions")]
    pub positions: i64,
    /// The Inmarsat-C frames with a bulletin board.
    #[field(label = "Inmarsat-C frames")]
    pub egc_frames: i64,
    /// The carriers found.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// The carriers that decode.
    #[field(label = "Carriers decoding")]
    pub decoding: i64,
}

/// A carrier found. The row key is the carrier id in five digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatCarrierRow<'a> {
    /// The id of the carrier.
    #[field(label = "Id")]
    pub id: i64,
    /// The offset from the centre of the input.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The frequency of the carrier.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The width of the carrier.
    #[field(label = "Width", unit = "Hz")]
    pub width: Hertz,
    /// The level over the floor.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The class by the width.
    #[field(label = "Class")]
    pub class: inmarsat::CarrierClass,
    /// What decodes the carrier.
    #[field(label = "Decodes")]
    pub decodes: inmarsat::Decodes,
    /// The bit rate.
    #[field(label = "Rate", unit = "bit/s")]
    pub rate: i64,
    /// How far the carrier decodes: 0 seen, 1 carrier, 2 frames, 3 data.
    #[field(label = "State")]
    pub state: i64,
    /// The energy per bit over the noise density.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: f64,
    /// The frames read.
    #[field(label = "Frames")]
    pub frames: i64,
    /// What the carrier is on the identified satellite.
    #[field(label = "Satellite")]
    pub satellite: &'a str,
    /// The window of the carrier is parked.
    #[field(label = "Parked")]
    pub parked: bool,
    /// The carrier has the focus.
    #[field(label = "Focused")]
    pub focused: bool,
    /// The window that holds the carrier.
    #[field(label = "Window")]
    pub window: i64,
    /// The carrier has the width of a voice channel.
    #[field(label = "Voice candidate")]
    pub voice_candidate: bool,
}

/// A satellite that the identification weighed. The row key is the name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatCandidateRow<'a> {
    /// The name of the satellite.
    #[field(label = "Name")]
    pub name: &'a str,
    /// How sure the identification is of this satellite, 0 to 1.
    #[field(label = "Confidence")]
    pub satellite_confidence: f64,
    /// The score of the decoded content.
    #[field(label = "Content score")]
    pub content: Option<f64>,
    /// The score of the frequencies.
    #[field(label = "Frequency score")]
    pub frequency_score: Option<f64>,
    /// The score of the geometry.
    #[field(label = "Geometry score")]
    pub geometry: Option<f64>,
    /// The elevation from the station.
    #[field(label = "Elevation", unit = "deg")]
    pub elevation_deg: Option<f64>,
    /// The azimuth from the station.
    #[field(label = "Azimuth", unit = "deg")]
    pub azimuth_deg: Option<f64>,
}

/// A reason of a candidate. The row key is `<candidate>/<n>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InmarsatCandidateReasonRow<'a> {
    /// The candidate.
    #[field(label = "Candidate", refers_to = "candidates")]
    pub candidate: &'a str,
    /// The reason in words.
    #[field(label = "Why")]
    pub why: &'a str,
}

/// Every carrier found.
pub static INMARSAT_CARRIERS: Table<InmarsatCarrierRow<'static>> =
    Table::new("carriers", "Carriers", 256)
        .label(&InmarsatCarrierRow::SATELLITE)
        .sorted(
            &InmarsatCarrierRow::SNR_DB,
            sdrkit_core::SortOrder::Descending,
        );
/// The satellites that the identification weighed.
pub static INMARSAT_CANDIDATES: Table<InmarsatCandidateRow<'static>> =
    Table::new("candidates", "Satellite candidates", 8).label(&InmarsatCandidateRow::NAME);
/// The reasons of each candidate.
pub static INMARSAT_CANDIDATE_REASONS: Table<InmarsatCandidateReasonRow<'static>> =
    Table::new("candidate_reasons", "Why", 128);

/// The arguments of `carrier.focus`.
#[derive(Arguments)]
pub struct InmarsatCarrierArgs<'a> {
    /// The row key of a carrier, or empty for none.
    #[arg(
        label = "Carrier",
        help = "The row key of a carrier in the carriers table. Empty: no carrier."
    )]
    pub row: &'a str,
}

/// Keeps the window of one carrier from being parked.
pub const INMARSAT_CARRIER_FOCUS: CommandKind<InmarsatCarrierArgs<'static>> =
    CommandKind::new("carrier.focus", "Focus on carrier")
        .help("Never parks the window of this carrier. An empty row ends the focus.");

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("satcom", 1, 5)
            .added(
                1,
                3,
                &[
                    "inmarsatc.content",
                    "aero.content",
                    "inmarsat.content",
                    "iridium.content",
                    "globalstar.content",
                    "goes_dcs.content",
                    "argos.content",
                    "hrdcp.content",
                    "sarsat.content",
                    "lora_sat.content",
                    "orbcomm.content",
                    "psk_demod.content",
                    "qo100.content",
                    "cubesat.content",
                    "fleet_broadcast.content",
                    "bgan.content",
                    "sat_survey.content",
                ],
            )
            .content(sdrkit_core::ContentKind::Tracking, &["sarsat", "orbcomm"])
            .content(
                sdrkit_core::ContentKind::Datalink,
                &["inmarsatc", "aero", "iridium"],
            )
            .content(
                sdrkit_core::ContentKind::SatelliteLinks,
                &[
                    "argos",
                    "inmarsat",
                    "globalstar",
                    "goes_dcs",
                    "hrdcp",
                    "lora_sat",
                    "qo100",
                    "cubesat",
                    "fleet_broadcast",
                    "bgan",
                    "sat_survey",
                ],
            )
            .content(sdrkit_core::ContentKind::Measurement, &["psk_demod"])
            .text(&["inmarsatc", "aero", "iridium"])
            .stack(
                StackShape::new(inmarsatc::ID)
                    .fields::<InmarsatcFields>()
                    .fields::<DecoderFields>()
                    .decoder("demod")
                    .table(&INMARSATC_MESSAGES)
                    .table(&INMARSATC_CHANNELS)
                    .table(&INMARSATC_PACKET_COUNTS)
                    .event(&INMARSATC_EGC)
                    .settings(inmarsatc::InmcSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    // The log of EGC messages belongs to no single frequency.
                    .kept(&["messages", "InmarsatcFields.message_count"]),
            )
            .stack(
                StackShape::new(aero::ID)
                    .fields::<AeroFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .decoder("su")
                    .table(&AERO_CHANNELS)
                    .table(&AERO_SU_TYPES)
                    .table(&AERO_LOGONS)
                    .table(&AERO_MESSAGES)
                    .table(&AERO_AIRCRAFT)
                    .table(&AERO_FACTS)
                    .points("Aircraft")
                    .event(&AERO_MESSAGE)
                    .event(&AERO_LOGON)
                    .settings(aero::AeroSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&AERO_REMOVE)
                    // The logs, the aircraft and the system facts are built over time, and the counts are their
                    // totals. The channels and their frames are of the present signal.
                    .kept(&[
                        "su_types",
                        "logons",
                        "messages",
                        "aircraft",
                        "facts",
                        "AeroFields.frames_ok",
                        "AeroFields.frames_bad",
                        "AeroFields.messages",
                        "AeroFields.positions",
                        "AeroFields.aircraft",
                    ]),
            )
            .stack(
                // The merged display is the status component and writes the totals of the stack. The Aero display
                // and the Inmarsat-C display show their results under their names, and their counts of good frames,
                // messages and positions are the fields of the stack.
                StackShape::new(inmarsat::ID)
                    .fields::<InmarsatFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .table(&INMARSAT_CARRIERS)
                    .table(&INMARSAT_CANDIDATES)
                    .table(&INMARSAT_CANDIDATE_REASONS)
                    .component("aero_display", |c| {
                        c.fields::<AeroFields>()
                            .shares(&["frames_ok", "messages", "positions"])
                            .dropped()
                            .table(&AERO_CHANNELS)
                            .table(&AERO_SU_TYPES)
                            .table(&AERO_LOGONS)
                            .table(&AERO_MESSAGES)
                            .table(&AERO_AIRCRAFT)
                            .table(&AERO_FACTS)
                    })
                    .component("egc", |c| {
                        c.fields::<InmarsatcFields>()
                            .shares(&["frames_ok"])
                            .dropped()
                            .table(&INMARSATC_MESSAGES)
                            .table(&INMARSATC_CHANNELS)
                            .table(&INMARSATC_PACKET_COUNTS)
                    })
                    .series(inmarsat::SPECTRUM)
                    .event(&AERO_MESSAGE)
                    .event(&AERO_LOGON)
                    .event(&INMARSATC_EGC)
                    .settings(inmarsat::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&INMARSAT_CARRIER_FOCUS)
                    // The logs, the aircraft and the system facts are built over time, and the counts are their
                    // totals. The carriers and the identification of the satellite are of the present signal.
                    .kept(&[
                        "aero_display.su_types",
                        "aero_display.logons",
                        "aero_display.messages",
                        "aero_display.aircraft",
                        "aero_display.facts",
                        "egc.messages",
                        "InmarsatFields.frames_ok",
                        "InmarsatFields.messages",
                        "InmarsatFields.positions",
                        "InmarsatFields.egc_frames",
                        "AeroFields.frames_bad",
                        "AeroFields.aircraft",
                        "InmarsatcFields.message_count",
                    ]),
            )
            .stack(
                StackShape::new(iridium::ID)
                    .fields::<IridiumFields>()
                    .table(&IRIDIUM_RAW)
                    .series(IRIDIUM_SCATTER_DETECTED)
                    .series(IRIDIUM_SCATTER_UW)
                    .event(&IRIDIUM_RAW_EVENT)
                    .settings(iridium::IridiumSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .stack({
                let mut s = StackShape::new(globalstar::ID)
                    .fields::<GlobalstarFields>()
                    .fields::<DecoderFields>()
                    .decoder("channels")
                    .table(&GLOBALSTAR_CHANNELS)
                    .table(&GLOBALSTAR_ACTIVITY);
                for d in GLOBALSTAR_DOPPLER {
                    s = s.series(d);
                }
                s.event(&GLOBALSTAR_ACTIVITY_EVENT)
                    .settings(globalstar::GlobalstarSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
            })
            .stack(
                StackShape::new(goes::ID)
                    .fields::<GoesDcsFields>()
                    .fields::<DecoderFields>()
                    .decoder("rx")
                    .table(&GOES_DCS_REPORTS)
                    .table(&GOES_DCS_PLATFORMS)
                    .table(&GOES_DCS_CARRIERS)
                    .series(GOES_DCS_SPECTRUM)
                    .event(&GOES_DCS_REPORT)
                    .settings(goes::GoesSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&GOES_DCS_REMOVE)
                    // The platforms are a database of stations: the display that holds them stays.
                    .kept(&["reports", "platforms", "carriers", "GoesDcsFields"]),
            )
            .stack(
                StackShape::new(argos::ID)
                    .fields::<ArgosFields>()
                    .fields::<DecoderFields>()
                    .decoder("rx")
                    .table(&ARGOS_BURSTS)
                    .table(&ARGOS_PLATFORMS)
                    .table(&ARGOS_FRAMES)
                    .series(ARGOS_DOPPLER)
                    .event(&ARGOS_PTT)
                    .event(&ARGOS_DOWNLINK_FRAME)
                    .settings(argos::ArgosSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&ARGOS_REMOVE)
                    // The platforms are a database of stations: the display that holds them stays.
                    .kept(&["bursts", "platforms", "frames", "ArgosFields"]),
            )
            .stack(
                StackShape::new(hrdcp::ID)
                    .fields::<HrdcpFields>()
                    .fields::<DecoderFields>()
                    .decoder("rx")
                    .table(&HRDCP_REPORTS)
                    .table(&HRDCP_PLATFORMS)
                    .table(&HRDCP_CARRIERS)
                    .series(HRDCP_SPECTRUM)
                    .event(&HRDCP_REPORT)
                    .settings(hrdcp::HrdcpSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    // The platforms are a database of stations: the display that holds them stays.
                    .kept(&["reports", "platforms", "carriers", "HrdcpFields"]),
            )
            .stack(
                StackShape::new(sarsat::ID)
                    .fields::<SarsatFields>()
                    .fields::<DecoderFields>()
                    .decoder("rx")
                    .table(&SARSAT_BURSTS)
                    .table(&SARSAT_BEACONS)
                    .points("Beacons")
                    .event(&SARSAT_BEACON)
                    .settings(sarsat::SarsatSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&SARSAT_REMOVE)
                    .command(&SARSAT_PRESET_APPLY)
                    // The beacons are a database of stations: the display that holds them stays.
                    .kept(&["bursts", "beacons", "SarsatFields"]),
            )
            .stack(
                StackShape::new(lorasat::ID)
                    .fields::<LoraSatFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .table(&LORA_SAT_FRAMES)
                    .table(&LORA_SAT_DEMODULATORS)
                    .table(&LORA_SAT_DEMODULATOR_SYNCS)
                    .table(&LORA_SAT_FRAMES_PER_SF)
                    .series(LORA_SAT_DOPPLER)
                    .event(&LORA_SAT_FRAME)
                    .settings(lorasat::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .stack(
                StackShape::new(orbcomm::ID)
                    .fields::<OrbcommFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .table(&ORBCOMM_SATELLITES)
                    .table(&ORBCOMM_SATELLITE_CHANNELS)
                    .table(&ORBCOMM_DECODERS)
                    .table(&ORBCOMM_PACKETS)
                    .table(&ORBCOMM_PACKET_TYPES)
                    .series(ORBCOMM_CONSTELLATION)
                    .points("Satellites")
                    .event(&ORBCOMM_MESSAGE)
                    .settings(orbcomm::OrbcommSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&ORBCOMM_PRESET_APPLY)
                    .command(&ORBCOMM_REMOVE)
                    // The satellites are a database of stations: the display that holds them stays.
                    .kept(&[
                        "satellites",
                        "satellite_channels",
                        "decoders",
                        "packets",
                        "packet_types",
                        "OrbcommFields",
                    ]),
            )
            .stack(
                StackShape::new(psk::ID)
                    .fields::<PskDemodFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .decoder("bits")
                    .table(&PSK_DEMOD_RATES)
                    .table(&PSK_DEMOD_TRIALS)
                    .table(&PSK_DEMOD_SYNCS)
                    .table(&PSK_DEMOD_LOG)
                    .series(PSK_DEMOD_CONSTELLATION)
                    .series(PSK_DEMOD_EYE)
                    .series(PSK_DEMOD_ENVELOPE)
                    .series(PSK_DEMOD_POWER)
                    .event(&PSK_DEMOD_LOCK)
                    .event(&PSK_DEMOD_BYTES)
                    .settings(psk::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .stack(
                StackShape::new(qo100::ID)
                    .fields::<Qo100Fields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .table(&QO100_BEACONS)
                    .table(&QO100_SIGNALS)
                    .table(&QO100_TRACKS)
                    .table(&QO100_CONVERSATIONS)
                    .table(&QO100_CALLSIGNS)
                    .table(&QO100_DATV)
                    .series(QO100_NARROWBAND)
                    .series(QO100_WIDEBAND)
                    .series(QO100_LNB_ERROR)
                    .event(&QO100_LOCK)
                    .event(&QO100_CONVERSATION)
                    .settings(qo100::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&QO100_SPEECH_TEXT)
                    .command(&QO100_CONVERSATION_LISTEN)
                    .command(&QO100_REMOVE)
                    .fields::<Qo100TxFields>()
                    .command(&QO100_PTT),
            )
            .stack(
                StackShape::new(cubesat::ID)
                    .fields::<CubesatFields>()
                    .fields::<DecoderFields>()
                    .decoder("receiver")
                    .decoder("naming")
                    .table(&CUBESAT_FRAMES)
                    .table(&CUBESAT_FRAMES_PER_FRAMING)
                    .table(&CUBESAT_CARRIERS)
                    .event(&CUBESAT_FRAME)
                    .event(&CUBESAT_KISS)
                    .settings(cubesat::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .fields::<CubesatTxFields>()
                    .event(&CUBESAT_TX)
                    .command(&CUBESAT_SEND)
                    .command(&CUBESAT_HALT),
            )
            .stack(
                StackShape::new(fleet::ID)
                    .fields::<FleetBroadcastFields>()
                    .fields::<DecoderFields>()
                    .table(&FLEET_BROADCAST_CHANNELS)
                    .table(&FLEET_BROADCAST_FRAMES)
                    .event(&FLEET_BROADCAST_STATE)
                    .settings(fleet::FleetSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .stack(
                StackShape::new(bgan::ID)
                    .fields::<BganFields>()
                    .fields::<DecoderFields>()
                    .table(&BGAN_CARRIERS)
                    .table(&BGAN_BEAMS)
                    .table(&BGAN_LOG)
                    .event(&BGAN_SIGNALLING)
                    .settings(bgan::BganSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .stack(
                StackShape::new(sat_survey::ID)
                    .fields::<SatSurveyFields>()
                    .fields::<DecoderFields>()
                    .table(&SAT_SURVEY_CARRIERS)
                    .table(&SAT_SURVEY_CHANNELS)
                    .series(sat_survey::SPECTRUM)
                    .event(&SAT_SURVEY_CARRIER)
                    .settings(sat_survey::SurveySetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
            .added(1, 1, ADDED_1_1)
            .added(1, 2, ADDED_1_2)
    })
}

/// What version 1.1 added: the AX.25 sender of the stack `cubesat`.
const ADDED_1_1: &[&str] = &[
    "cubesat.fields.CubesatTxFields",
    "cubesat.event.cubesat.tx",
    "cubesat.setting.tx_rate",
    "cubesat.setting.tx_delay",
    "cubesat.setting.tx_level",
    "cubesat.command.send",
    "cubesat.command.halt",
    "shape.CubesatTxFields",
    "shape.CubesatTxFields.tx_sending",
    "shape.CubesatTxFields.tx_sent",
    "shape.CubesatTxFields.tx_waiting",
    "shape.CubesatTxEvent",
    "shape.CubesatTxEvent.state",
    "shape.CubesatTxEvent.reason",
    "shape.CubesatTxEvent.target",
    "shape.CubesatTxEvent.text",
    "shape.CubesatTxEvent.family",
];

/// What version 1.2 added: the voice sender of the stack `qo100` and the level of the user's own downlink against the beacon.
const ADDED_1_2: &[&str] = &[
    "qo100.fields.Qo100TxFields",
    "qo100.setting.tx_level",
    "qo100.setting.tx_timeout",
    "qo100.command.ptt",
    "shape.Qo100Fields.own_to_beacon_db",
    "shape.Qo100TxFields",
    "shape.Qo100TxFields.transmitting",
    "shape.Qo100TxFields.tx_freq_hz",
    "shape.Qo100TxFields.tx_time_s",
    "shape.Qo100TxFields.tx_mic_dbfs",
    "shape.Qo100TxFields.tx_peak_db",
    "shape.Qo100TxFields.tx_level_db",
    "shape.Qo100TxFields.tx_limit_db",
];

/// The description of stack `id` of the family.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the satcom family does not describe the stack {id}"))
}
