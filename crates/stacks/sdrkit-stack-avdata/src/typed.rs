//! The interface of the aviation family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types and what each stack declares.

use crate::vdl3::phy::{BurstKind, Training};
use sdrkit_components::voice::{ptt, PttArgs};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::{Command, SettingSet};
use sdrkit_graph::DecoderFields;
use sdrkit_proto_aero::aircraft::columns as aircraft;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

// --- OFDM structure stacks (ldacs, aeromacs) ---

/// The result fields of an OFDM structure stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OfdmShapeFields<'a> {
    /// The OFDM of the numerology is there.
    #[field(label = "Detected")]
    pub detected: bool,
    /// The symbol duration with its prefix. No value while the numerology of the settings is not valid.
    #[field(label = "Symbol", unit = "us")]
    pub symbol_us: Option<f64>,
    /// The prefix duration. No value while the numerology of the settings is not valid.
    #[field(label = "Prefix", unit = "us")]
    pub prefix_us: Option<f64>,
    /// The sample of the symbol period where the prefix correlation peaks, while detected.
    #[field(label = "Symbol timing", unit = "samples")]
    pub timing: Option<i64>,
    /// The prefix correlation, while detected.
    #[field(label = "Prefix correlation")]
    pub correlation: Option<f64>,
    /// The carrier offset, while detected.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The frame structure in words. No value while it is not known.
    #[field(label = "Frame structure")]
    pub frame: Option<&'a str>,
    /// What the stack decodes.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The summary line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the detection starts. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OfdmShapeDetectionRow<'a> {
    /// The signal time of the start.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The symbol duration with its prefix.
    #[field(label = "Symbol", unit = "us")]
    pub symbol_us: f64,
    /// The prefix duration.
    #[field(label = "Prefix", unit = "us")]
    pub prefix_us: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The prefix correlation.
    #[field(label = "Prefix correlation")]
    pub correlation: f64,
    /// The frame structure in words. No value while it was not known.
    #[field(label = "Frame structure")]
    pub frame: Option<&'a str>,
}

/// The detection starts, newest first.
pub static OFDM_DETECTIONS: Table<OfdmShapeDetectionRow<'static>> =
    Table::new("detections", "Detections", crate::ofdm_shape::DETECTION_CAP);

// --- GBAS VDB ---

/// The result fields of GBAS VDB.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GbasVdbFields<'a> {
    /// Slots of 8 that held a burst in the last frame.
    #[field(label = "Slots busy (of 8)")]
    pub slots_busy: i64,
    /// The line of the symbol rate was seen.
    #[field(label = "Symbol rate 10 500 /s seen")]
    pub symbol_rate_seen: bool,
    /// Bursts counted.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The length of the last burst. No value before a burst.
    #[field(label = "Burst length", unit = "ms")]
    pub length_ms: Option<f64>,
    /// The signal to noise ratio of the last burst. No value before a burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The summary line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the stack decodes.
    #[field(label = "Content")]
    pub content: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the burst log: every 16th burst. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GbasVdbBurstRow {
    /// The signal time of the burst.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The length of the burst.
    #[field(label = "Burst length", unit = "ms")]
    pub length_ms: f64,
    /// Slots busy then.
    #[field(label = "Slots busy (of 8)")]
    pub slots_busy: i64,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The burst log, newest first.
pub static GBAS_VDB_BURSTS: Table<GbasVdbBurstRow> =
    Table::new("bursts", "Burst log", crate::gbas_vdb::LOG_CAP);

// --- VDL Mode 3 ---

/// The result fields of VDL Mode 3.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl3Fields<'a> {
    /// The state: 0 listening, 1 bursts seen, 2 decoding.
    #[field(label = "Decoder state")]
    pub rx_state: i64,
    /// Training sequences found.
    #[field(label = "Training sequences")]
    pub trainings: i64,
    /// Good bursts.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts that failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// M uplink bursts.
    #[field(label = "M uplink bursts")]
    pub m_uplink: i64,
    /// M uplink bursts of the 3T configuration.
    #[field(label = "M uplink (3T) bursts")]
    pub m_uplink_3t: i64,
    /// Handoff check bursts.
    #[field(label = "Handoff check bursts")]
    pub handoff: i64,
    /// Poll response bursts.
    #[field(label = "Poll response bursts")]
    pub poll_response: i64,
    /// M downlink bursts.
    #[field(label = "M downlink bursts")]
    pub m_downlink: i64,
    /// Net entry request bursts.
    #[field(label = "Net entry request bursts")]
    pub net_entry: i64,
    /// V/D data bursts.
    #[field(label = "V/D data bursts")]
    pub vd_data: i64,
    /// V/D voice bursts.
    #[field(label = "V/D voice bursts")]
    pub vd_voice: i64,
    /// The level of the last burst. No value before a burst.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The signal to noise ratio of the last burst. No value before a burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last burst. No value before a burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The summary of the newest burst.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the bursts. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl3BurstRow<'a> {
    /// The signal time of the end of the training.
    #[field(label = "Time", unit = "s", role = Time)]
    pub time_s: f64,
    /// The burst kind.
    #[field(label = "Kind")]
    pub kind: BurstKind,
    /// The training sequence.
    #[field(label = "Training")]
    pub training: Training,
    /// Symbols after the training.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// Every Golay word decoded.
    #[field(label = "Golay good")]
    pub golay_ok: bool,
    /// Bits corrected by Golay.
    #[field(label = "Golay corrected", unit = "bits")]
    pub golay_corrected: i64,
    /// The information words.
    #[field(label = "Information words")]
    pub info: &'a str,
    /// The Reed-Solomon word was good.
    #[field(label = "Reed-Solomon good")]
    pub rs_ok: bool,
    /// Octets corrected by Reed-Solomon.
    #[field(label = "Reed-Solomon corrected", unit = "octets")]
    pub rs_corrected: i64,
    /// The data octets, hex.
    #[field(label = "Data")]
    pub data_hex: &'a str,
    /// The printable text of the data.
    #[field(label = "Text")]
    pub data_text: &'a str,
    /// The level of the burst.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier offset of the burst.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub cfo_hz: f64,
    /// The error vector magnitude of the burst.
    #[field(shared = keys::EVM_PCT)]
    pub evm_pct: f64,
    /// The summary of the burst.
    #[field(label = "Summary", role = Text)]
    pub summary: &'a str,
}

/// The bursts, newest first.
pub static VDL3_BURSTS: Table<Vdl3BurstRow<'static>> =
    Table::new("bursts", "Bursts", crate::vdl3::BURST_CAP).role(sdrkit_core::TableRole::Messages);

// --- ACARS ---

/// The result fields of ACARS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AcarsFields<'a> {
    /// Good blocks.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bad blocks.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Good blocks that needed a repair.
    #[field(label = "Blocks repaired")]
    pub blocks_repaired: i64,
    /// Frame starts (SYN SYN SOH) seen.
    #[field(label = "Frames started")]
    pub frames_started: i64,
    /// Messages with a position.
    #[field(label = "Positions")]
    pub positions: i64,
    /// Channels being decoded now.
    #[field(label = "Channels active")]
    pub channels_active: i64,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// Messages in the list.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Good blocks, percent of all blocks. No value before a block.
    #[field(label = "Good blocks", unit = "%")]
    pub good_pct: Option<f64>,
    /// What is watched: the channel list, or the 25 kHz grid.
    #[field(label = "Watching")]
    pub watching: &'a str,
}

/// A row of the messages. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AcarsMessageRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the carrier.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The mode character.
    #[field(label = "Mode")]
    pub mode: Option<&'a str>,
    /// The registration.
    #[field(shared = aircraft::REGISTRATION, role = Sender)]
    pub registration: Option<&'a str>,
    /// The row of the aircraft in the aircraft table.
    #[field(label = "Aircraft", refers_to = "aircraft")]
    pub aircraft: Option<&'a str>,
    /// The flight id of a downlink.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: Option<&'a str>,
    /// The label.
    #[field(shared = aircraft::LABEL)]
    pub label: Option<&'a str>,
    /// The meaning of the label.
    #[field(label = "Meaning")]
    pub label_meaning: Option<&'a str>,
    /// The block id of the first block.
    #[field(label = "Block id")]
    pub block_id: Option<&'a str>,
    /// The message sequence number of a downlink.
    #[field(label = "Message number")]
    pub msn: Option<&'a str>,
    /// Aircraft to ground.
    #[field(label = "Downlink")]
    pub downlink: bool,
    /// The technical acknowledgement: `NAK`, or the block id acknowledged.
    #[field(label = "Acknowledgement")]
    pub ack: Option<&'a str>,
    /// The printable text.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: Option<&'a str>,
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
    /// Blocks joined.
    #[field(label = "Blocks")]
    pub blocks: i64,
    /// Bits the repair changed.
    #[field(label = "Bits repaired")]
    pub bits_fixed: i64,
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude the message gives.
    #[field(label = "Altitude", unit = "ft", role = Altitude)]
    pub alt_ft: Option<f64>,
}

/// A row of the aircraft. The row key is the registration.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AcarsAircraftRow<'a> {
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
    /// Messages of the aircraft.
    #[field(shared = aircraft::MESSAGES)]
    pub messages: i64,
    /// The last position.
    #[field(shared = aircraft::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(shared = aircraft::ALT_FT)]
    pub alt_ft: Option<f64>,
    /// The track.
    #[field(shared = aircraft::TRACK_DEG)]
    pub track_deg: Option<f64>,
    /// The ground speed.
    #[field(shared = aircraft::GS_KT)]
    pub gs_kt: Option<f64>,
    /// The vertical rate.
    #[field(shared = aircraft::VS_FPM)]
    pub vs_fpm: Option<f64>,
    /// When the aircraft was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The channel of the last message.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the last downlink.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A row of the channels. The row key is the frequency in hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AcarsChannelRow {
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level now. No value without a carrier.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// The signal to noise ratio in 7 kHz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// A decoder is on the channel.
    #[field(label = "Active")]
    pub active: bool,
    /// Good blocks.
    #[field(label = "Good")]
    pub good: i64,
    /// Bad blocks.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The wall-clock time of the last good block, when known.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// An ACARS message. The source is the registration of the aircraft, as the link names it. The link does not name the
/// ground station, so the target has no value.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AcarsMessageEvent<'a> {
    /// The registration.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The receiver. No value: the link does not name it.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio. No value: the stack does not measure it per message.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The flight id of a downlink.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: &'a str,
    /// The label.
    #[field(shared = aircraft::LABEL)]
    pub label: &'a str,
    /// Aircraft to ground.
    #[field(label = "Downlink")]
    pub downlink: bool,
    /// The message sequence number.
    #[field(label = "Message number")]
    pub msn: &'a str,
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude the message gives.
    #[field(label = "Altitude", unit = "ft")]
    pub alt_ft: Option<f64>,
}

/// The messages, newest first.
pub static ACARS_MESSAGES: Table<AcarsMessageRow<'static>> =
    Table::new("messages", "Messages", crate::acars::tables::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&AcarsMessageRow::FLIGHT);
/// The aircraft.
pub static ACARS_AIRCRAFT: Table<AcarsAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::acars::tables::AIRCRAFT_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&AcarsAircraftRow::FLIGHT);
/// The channels.
pub static ACARS_CHANNELS: Table<AcarsChannelRow> =
    Table::new("channels", "Channels", crate::acars::tables::CHANNEL_CAP);
/// Each message.
pub static ACARS_MESSAGE: EventKind<AcarsMessageEvent<'static>> =
    EventKind::new("acars.message", "ACARS message");

// --- VDL Mode 2 ---

/// The result fields of VDL Mode 2.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2Fields {
    /// Good frames.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts that failed.
    #[field(label = "Bad bursts")]
    pub bursts_bad: i64,
    /// Octets the Reed-Solomon code corrected.
    #[field(label = "Octets corrected")]
    pub octets_corrected: i64,
    /// Headers that needed their FEC.
    #[field(label = "Headers corrected")]
    pub headers_fixed: i64,
    /// Frames with a bad FCS.
    #[field(label = "Bad FCS")]
    pub fcs_bad: i64,
    /// Frames with a position.
    #[field(label = "Positions")]
    pub positions: i64,
    /// How far the radio is off the channel grid. No value before the first report of the channels.
    #[field(label = "Tuning error", unit = "Hz")]
    pub tuning_error: Option<Hertz>,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// Ground stations in the table.
    #[field(label = "Ground stations")]
    pub ground_stations: i64,
    /// Frames in the list.
    #[field(label = "Frames")]
    pub messages: i64,
}

/// A row of the frames. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2MessageRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the burst.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The source address, six hex digits.
    #[field(label = "From", role = Sender)]
    pub from: &'a str,
    /// The type of the source address.
    #[field(label = "From type")]
    pub from_type: i64,
    /// The destination address, six hex digits.
    #[field(label = "To", role = Addressee)]
    pub to: &'a str,
    /// The type of the destination address.
    #[field(label = "To type")]
    pub to_type: i64,
    /// I, S or U.
    #[field(label = "Frame kind")]
    pub frame_kind: Option<&'a str>,
    /// The name of the frame.
    #[field(label = "Frame")]
    pub frame: Option<&'a str>,
    /// The C/R bit of the source.
    #[field(label = "Response")]
    pub response: bool,
    /// The deepest layer read.
    #[field(label = "Protocol")]
    pub protocol: Option<&'a str>,
    /// One line.
    #[field(label = "Summary")]
    pub summary: Option<&'a str>,
    /// The registration of an ACARS message.
    #[field(shared = aircraft::REGISTRATION)]
    pub registration: Option<&'a str>,
    /// The flight id of an ACARS message.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: Option<&'a str>,
    /// The label of an ACARS message.
    #[field(shared = aircraft::LABEL)]
    pub label: Option<&'a str>,
    /// The text of an ACARS message.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: Option<&'a str>,
    /// The ACARS check sequence passed.
    #[field(label = "ACARS check good")]
    pub acars_ok: bool,
    /// Blocks joined into the ACARS message.
    #[field(label = "Blocks")]
    pub blocks: Option<i64>,
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
    /// The position the frame gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude the frame gives.
    #[field(label = "Altitude", unit = "ft", role = Altitude)]
    pub alt_ft: Option<f64>,
    /// Every layer, one line each, joined by " | ".
    #[field(label = "Detail")]
    pub detail: Option<&'a str>,
}

/// A row of the aircraft. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2AircraftRow<'a> {
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
    /// Messages of the aircraft.
    #[field(shared = aircraft::MESSAGES)]
    pub messages: i64,
    /// The last position.
    #[field(shared = aircraft::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(shared = aircraft::ALT_FT)]
    pub alt_ft: Option<f64>,
    /// The track.
    #[field(shared = aircraft::TRACK_DEG)]
    pub track_deg: Option<f64>,
    /// The ground speed.
    #[field(shared = aircraft::GS_KT)]
    pub gs_kt: Option<f64>,
    /// The vertical rate.
    #[field(shared = aircraft::VS_FPM)]
    pub vs_fpm: Option<f64>,
    /// When the aircraft was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Frames to or from the aircraft.
    #[field(label = "Frames")]
    pub count: i64,
    /// The channel of the last frame.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The ground station the aircraft last talked to.
    #[field(label = "Ground station", refers_to = "ground_stations")]
    pub ground_station: Option<&'a str>,
    /// The position now, by dead reckoning from the last fix.
    #[field(label = "Position now")]
    pub estimate: Option<(f64, f64)>,
    /// The points of the trail.
    #[field(label = "Trail points")]
    pub trail_points: i64,
}

/// A row of the ground stations. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2StationRow<'a> {
    /// The address, six hex digits.
    #[field(shared = aircraft::ADDRESS)]
    pub address: &'a str,
    /// The kind of the station.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// Frames the station sent.
    #[field(label = "Frames")]
    pub count: i64,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of its last frame.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// When the station was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The airports the station covers.
    #[field(label = "Airports")]
    pub airports: Option<&'a str>,
    /// The position from its GSIF.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A row of the channels heard now. The row key is the frequency in hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2ChannelRow {
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The channel is inside the band of the input.
    #[field(label = "In band")]
    pub in_band: bool,
    /// A good burst in the last 2 s.
    #[field(label = "Active")]
    pub active: bool,
    /// The level of the channel.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Bursts found.
    #[field(label = "Found")]
    pub found: i64,
    /// Good bursts.
    #[field(label = "Good")]
    pub good: i64,
    /// Bad bursts.
    #[field(label = "Bad")]
    pub bad: i64,
    /// Frames.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The offset of the last burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
}

/// A VDL2 frame. The source and the target are the addresses of the sender and the receiver, six hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl2MessageEvent<'a> {
    /// The address of the sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The address of the receiver.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The summary of the frame.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// An aircraft sent the frame.
    #[field(label = "From an aircraft")]
    pub from_air: bool,
    /// The deepest layer read.
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The name of the frame.
    #[field(label = "Frame")]
    pub frame: &'a str,
}

/// The frames, newest first.
pub static VDL2_MESSAGES: Table<Vdl2MessageRow<'static>> =
    Table::new("messages", "Frames", crate::vdl2::tables::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&Vdl2MessageRow::FLIGHT);
/// The aircraft.
pub static VDL2_AIRCRAFT: Table<Vdl2AircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::vdl2::tables::AIRCRAFT_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&Vdl2AircraftRow::FLIGHT);
/// The ground stations.
pub static VDL2_STATIONS: Table<Vdl2StationRow<'static>> = Table::new(
    "ground_stations",
    "Ground stations",
    crate::vdl2::tables::STATION_CAP,
)
.label(&Vdl2StationRow::ADDRESS);
/// The channels heard now.
pub static VDL2_CHANNELS: Table<Vdl2ChannelRow> =
    Table::new("channels", "Channels", crate::vdl2::tables::CHANNEL_CAP);
/// Each frame.
pub static VDL2_MESSAGE: EventKind<Vdl2MessageEvent<'static>> =
    EventKind::new("vdl2.message", "VDL2 frame");

/// The arguments of `remove` of VDL2.
#[derive(Arguments)]
pub struct Vdl2StationArgs<'a> {
    /// The row key of the ground station.
    #[arg(shared = VDL2_STATION_ROW)]
    pub row: &'a str,
}

/// The row key of a ground station of VDL2.
pub const VDL2_STATION_ROW: Argument<field::Text> = objcmd::row_of("ground_stations");
/// Forgets one ground station.
pub const VDL2_REMOVE: CommandKind<Vdl2StationArgs<'static>> = objcmd::remove_kind();

// --- SELCAL ---

/// The result fields of SELCAL.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SelcalFields {
    /// Calls in the log.
    #[field(label = "Calls")]
    pub calls: i64,
    /// Codes in the table.
    #[field(label = "Codes")]
    pub codes: i64,
    /// The strongest tone of the audio now. No value without a tone.
    #[field(label = "Strongest tone", unit = "Hz")]
    pub tone: Option<Hertz>,
}

/// A row of the calls. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SelcalCallRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The code, "AM-CP".
    #[field(label = "Code", role = Addressee)]
    pub code: &'a str,
    /// The code as its row key in the codes.
    #[field(label = "Code row", refers_to = "codes")]
    pub code_row: &'a str,
    /// The four tones, "a b / c d".
    #[field(label = "Tones", unit = "Hz")]
    pub tones: &'a str,
    /// The dial frequency. No value when it is not known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
    /// The level of the tones, dB relative to a full-scale sine.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The aircraft the code belongs to.
    #[field(label = "Aircraft")]
    pub aircraft: Option<&'a str>,
}

/// A row of the codes heard. The row key is the code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SelcalCodeRow<'a> {
    /// The code.
    #[field(label = "Code")]
    pub code: &'a str,
    /// The aircraft the code belongs to.
    #[field(label = "Aircraft")]
    pub aircraft: Option<&'a str>,
    /// How often the code was heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// When the code was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The last dial frequency. No value when it is not known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
}

/// The calls, newest first.
pub static SELCAL_CALLS: Table<SelcalCallRow<'static>> =
    Table::new("calls", "Calls", crate::selcal::tables::CALL_CAP)
        .role(sdrkit_core::TableRole::Calls);
/// The codes heard.
pub static SELCAL_CODES: Table<SelcalCodeRow<'static>> =
    Table::new("codes", "Codes", crate::selcal::tables::CODE_CAP)
        .role(sdrkit_core::TableRole::Nodes);
/// The audio spectrum.
pub const SELCAL_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Audio spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The arguments of `remove` of SELCAL.
#[derive(Arguments)]
pub struct SelcalCodeArgs<'a> {
    /// The row key of the code.
    #[arg(shared = SELCAL_CODE_ROW)]
    pub row: &'a str,
}

/// The row key of a code of SELCAL.
pub const SELCAL_CODE_ROW: Argument<field::Text> = objcmd::row_of("codes");
/// Forgets one code.
pub const SELCAL_REMOVE: CommandKind<SelcalCodeArgs<'static>> = objcmd::remove_kind();

// --- Airband ---

/// The result fields of Airband.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandFields {
    /// The rate of the channel receivers.
    #[field(label = "Channel rate", unit = "Hz")]
    pub channel_rate: Hertz,
    /// How far the radio is off. No value while it is not known.
    #[field(label = "Radio off by", unit = "Hz")]
    pub tuning_error: Option<Hertz>,
    /// The squelch over the noise.
    #[field(label = "Squelch", unit = "dB")]
    pub squelch_db: f64,
    /// How long a channel stays open after its carrier goes.
    #[field(label = "Hang", unit = "s")]
    pub hang: Seconds,
    /// The scan is on.
    #[field(label = "Scan")]
    pub scan: bool,
    /// The channel the scan holds. No value while it holds none.
    #[field(label = "Scan channel", unit = "Hz")]
    pub scan_channel: Option<Hertz>,
    /// Transmissions heard.
    #[field(label = "Transmissions heard")]
    pub transmissions: i64,
}

/// A row of the channels. The row key is the name of the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandChannelRow<'a> {
    /// The name of the channel.
    #[field(label = "Channel")]
    pub name: &'a str,
    /// The label of the channel.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The channel is inside the band of the input.
    #[field(label = "In band")]
    pub in_band: bool,
    /// The carrier over the noise.
    #[field(label = "C/N", unit = "dB")]
    pub cn_db: f64,
    /// The level of the channel.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The squelch of the channel, when it has its own.
    #[field(label = "Squelch", unit = "dB")]
    pub squelch_db: Option<f64>,
    /// The squelch is open.
    #[field(label = "Open")]
    pub open: bool,
    /// The channel plays.
    #[field(label = "Playing")]
    pub playing: bool,
    /// Two stations on the channel at once.
    #[field(label = "Two stations")]
    pub heterodyne: bool,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Transmissions on the channel.
    #[field(label = "Transmissions")]
    pub count: i64,
}

/// A row of the band watch: a carrier on the air that is not in the list. The row key is its name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandSeenRow<'a> {
    /// The name of the channel.
    #[field(label = "Channel")]
    pub name: &'a str,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The channel is on the 8.33 kHz grid.
    #[field(label = "8.33 kHz")]
    pub is833: bool,
    /// The level over the floor.
    #[field(label = "Level", unit = "dB")]
    pub excess_db: f64,
    /// How often the carrier was heard.
    #[field(label = "Heard")]
    pub reports: i64,
    /// The carrier is on now.
    #[field(label = "Active")]
    pub active: bool,
    /// The frequency is approximate.
    #[field(label = "Approximate")]
    pub approximate: bool,
    /// The signal time it was last seen.
    #[field(label = "Last seen", unit = "s")]
    pub last_s: f64,
}

/// A row of the transmissions. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandActivityRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time of the start.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// The name of the channel.
    #[field(label = "Channel")]
    pub name: &'a str,
    /// The label of the channel.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
    /// The carrier over the noise.
    #[field(label = "C/N", unit = "dB")]
    pub cn_db: f64,
    /// The level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// Two stations at once.
    #[field(label = "Two stations")]
    pub heterodyne: bool,
}

/// A transmission, when it ends. Analogue voice carries no identity: the source and the target have no value. The channel
/// is the name of the channel, the text its label.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandActivityEvent<'a> {
    /// The sender. No value: voice carries no identity.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver. No value: voice carries no identity.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The name of the channel.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The label of the channel.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The carrier over the noise.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// What the event is about: voice.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
    /// The carrier over the noise.
    #[field(label = "C/N", unit = "dB")]
    pub cn_db: f64,
}

/// The channels.
pub static AIRBAND_CHANNELS: Table<AirbandChannelRow<'static>> =
    Table::new("channels", "Channels", 256)
        .role(sdrkit_core::TableRole::Groups)
        .label(&AirbandChannelRow::NAME);
/// The band watch.
pub static AIRBAND_SEEN: Table<AirbandSeenRow<'static>> =
    Table::new("seen", "Band watch", 200).label(&AirbandSeenRow::NAME);
/// The transmissions, newest first.
pub static AIRBAND_ACTIVITY: Table<AirbandActivityRow<'static>> =
    Table::new("activity", "Transmissions", crate::airband::tables::LOG)
        .role(sdrkit_core::TableRole::Calls)
        .label(&AirbandActivityRow::NAME);
/// Each transmission.
pub static AIRBAND_TRANSMISSION: EventKind<AirbandActivityEvent<'static>> =
    EventKind::new("airband.activity", "Transmission");

/// The result fields of the Airband transmitter. The measurements have no value while the stack does not transmit.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AirbandTxFields {
    /// Whether the stack transmits now.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the tuned frequency.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// The time on air of the transmission.
    #[field(label = "Time on air", unit = "s")]
    pub tx_time_s: Option<Seconds>,
    /// The peak of the microphone audio over the last report.
    #[field(label = "Microphone peak", unit = "dBFS")]
    pub tx_mic_dbfs: Option<f64>,
    /// The peak modulation depth over the last report.
    #[field(label = "Modulation depth", unit = "%")]
    pub tx_depth: Option<f64>,
}

/// Push-to-talk: speech from the microphone input as AM on the tuned frequency.
pub const PTT: CommandKind<PttArgs> = ptt("With on true, sends the speech of the microphone input as AM with full carrier on the tuned frequency, in the audio band of the channel spacing, until ptt with on false, halt or the time-out.");
/// Ends the transmission at once.
pub const HALT: Command = Command::new("halt", "Halt")
    .help("Ends the transmission at once, with the fall of the carrier.");

/// What version 1.1 added: the transmit branch of Airband.
const ADDED_1_1: &[&str] = &[
    "shape.AirbandTxFields",
    "shape.AirbandTxFields.transmitting",
    "shape.AirbandTxFields.tx_freq_hz",
    "shape.AirbandTxFields.tx_time_s",
    "shape.AirbandTxFields.tx_mic_dbfs",
    "shape.AirbandTxFields.tx_depth",
    "airband.fields.AirbandTxFields",
    "airband.setting.tx_spacing",
    "airband.setting.tx_am_depth",
    "airband.setting.tx_mic_gain",
    "airband.setting.tx_timeout",
    "airband.command.ptt",
    "airband.command.halt",
];

// --- HFDL ---

/// The result fields of HFDL.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlFields<'a> {
    /// Good PDUs: squitters and MPDUs, one a burst.
    #[field(label = "PDUs")]
    pub pdus: i64,
    /// Bursts that failed.
    #[field(label = "Bad bursts")]
    pub bursts_bad: i64,
    /// Squitters.
    #[field(label = "Squitters")]
    pub squitters: i64,
    /// LPDUs.
    #[field(label = "LPDUs")]
    pub lpdus: i64,
    /// ACARS messages.
    #[field(label = "ACARS")]
    pub acars: i64,
    /// Messages with a position.
    #[field(label = "Positions")]
    pub positions: i64,
    /// Good bursts at 300 bit/s.
    #[field(label = "Bursts at 300 bit/s")]
    pub bursts_300: i64,
    /// Good bursts at 600 bit/s.
    #[field(label = "Bursts at 600 bit/s")]
    pub bursts_600: i64,
    /// Good bursts at 1200 bit/s.
    #[field(label = "Bursts at 1200 bit/s")]
    pub bursts_1200: i64,
    /// Good bursts at 1800 bit/s.
    #[field(label = "Bursts at 1800 bit/s")]
    pub bursts_1800: i64,
    /// Bursts lost: the preamble was refused.
    #[field(label = "Preambles refused")]
    pub lost_preamble: i64,
    /// Bursts lost: cut short.
    #[field(label = "Bursts cut short")]
    pub lost_short: i64,
    /// Squitters lost to a bad FCS.
    #[field(label = "Bad squitter FCS")]
    pub lost_squitter_fcs: i64,
    /// Bursts lost to a bad header FCS.
    #[field(label = "Bad header FCS")]
    pub lost_header_fcs: i64,
    /// LPDUs lost to a bad FCS.
    #[field(label = "Bad LPDU FCS")]
    pub lost_lpdu_fcs: i64,
    /// LPDUs of an unknown type.
    #[field(label = "Unknown LPDU type")]
    pub lost_lpdu_type: i64,
    /// What the band hopper does.
    #[field(label = "Band hopper")]
    pub hopper: &'a str,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
}

/// A row of the messages. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlMessageRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The dial frequency of the channel.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// The level of the burst.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The offset of the burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The bit rate of the burst.
    #[field(label = "Rate", unit = "bit/s")]
    pub bps: i64,
    /// The ground station id.
    #[field(label = "Ground station")]
    pub gs_id: i64,
    /// The ground station as its row key.
    #[field(label = "Ground station row", refers_to = "ground_stations")]
    pub ground_station: &'a str,
    /// The aircraft id the ground station gave.
    #[field(label = "Aircraft id")]
    pub ac_id: Option<i64>,
    /// The kind of the PDU.
    #[field(label = "Kind")]
    pub kind: Option<&'a str>,
    /// The kind of the HFNPDU.
    #[field(label = "HFNPDU")]
    pub hfnpdu: Option<&'a str>,
    /// Ground to aircraft.
    #[field(label = "Uplink")]
    pub uplink: bool,
    /// One line.
    #[field(label = "Summary")]
    pub summary: Option<&'a str>,
    /// The registration of an ACARS message.
    #[field(shared = aircraft::REGISTRATION)]
    pub registration: Option<&'a str>,
    /// The flight id.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: Option<&'a str>,
    /// The label of an ACARS message.
    #[field(shared = aircraft::LABEL)]
    pub label: Option<&'a str>,
    /// The block id of an ACARS message.
    #[field(label = "Block id")]
    pub block_id: Option<&'a str>,
    /// The message sequence number of an ACARS message.
    #[field(label = "Message number")]
    pub msn: Option<&'a str>,
    /// The text of an ACARS message.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: Option<&'a str>,
    /// The ACARS check sequence passed.
    #[field(label = "ACARS check good")]
    pub acars_ok: bool,
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
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// Every decoded layer, joined by " | ".
    #[field(label = "Detail")]
    pub detail: Option<&'a str>,
}

/// A row of the aircraft. The row key is the address, or the ground station and the aircraft id while the address is
/// not known.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlAircraftRow<'a> {
    /// The row key.
    #[field(label = "Aircraft")]
    pub key: &'a str,
    /// The 24-bit address, six hex digits.
    #[field(shared = aircraft::ADDRESS)]
    pub address: Option<&'a str>,
    /// The registration.
    #[field(shared = aircraft::REGISTRATION)]
    pub registration: Option<&'a str>,
    /// The flight id.
    #[field(shared = aircraft::FLIGHT)]
    pub flight: Option<&'a str>,
    /// The ground station id.
    #[field(label = "Ground station")]
    pub gs_id: i64,
    /// The ground station as its row key.
    #[field(label = "Ground station row", refers_to = "ground_stations")]
    pub ground_station: &'a str,
    /// Messages of the aircraft.
    #[field(label = "Messages")]
    pub count: i64,
    /// The dial frequency of the last message.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// When the aircraft was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The aircraft is logged on.
    #[field(label = "Logged on")]
    pub logged_on: bool,
    /// The last position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(label = "Altitude", unit = "ft", role = Altitude)]
    pub alt_ft: Option<f64>,
}

/// A row of the ground stations. The row key is the station id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlStationRow<'a> {
    /// The station id.
    #[field(label = "Ground station")]
    pub gs_id: i64,
    /// The name of the station.
    #[field(label = "Name")]
    pub name: &'a str,
    /// Heard directly: a squitter or an uplink.
    #[field(label = "Heard directly")]
    pub heard_direct: bool,
    /// The station is synchronised to UTC.
    #[field(label = "UTC")]
    pub utc_sync: bool,
    /// Squitters.
    #[field(label = "Squitters")]
    pub squitters: i64,
    /// Messages that name the station.
    #[field(label = "Messages")]
    pub count: i64,
    /// The dial frequency it was heard on.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// When the station was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frame index of the last squitter.
    #[field(label = "Frame")]
    pub frame_index: Option<i64>,
    /// The frequencies in use by the last squitter, kHz, separated by spaces.
    #[field(label = "In use, kHz")]
    pub active_khz: Option<&'a str>,
}

/// A row of the channels. The row key is the dial frequency in hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlChannelRow<'a> {
    /// The dial frequency.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// The name of the ground station of the channel.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The ground station as its row key.
    #[field(label = "Ground station row", refers_to = "ground_stations")]
    pub ground_station: Option<&'a str>,
    /// The channel is inside the band of the input.
    #[field(label = "In band")]
    pub in_band: bool,
    /// Preambles found.
    #[field(label = "Found")]
    pub found: i64,
    /// Good bursts.
    #[field(label = "Good")]
    pub good: i64,
    /// Bad bursts.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The signal to noise ratio of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The offset of the last burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The bit rate of the last burst.
    #[field(label = "Rate", unit = "bit/s")]
    pub bps: i64,
}

/// An HFDL message. The source is the sender (the address or registration of the aircraft, or the name of the ground
/// station), the target the receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfdlMessageEvent<'a> {
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The receiver. No value for a squitter.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The summary of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The kind of the PDU.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// Ground to aircraft.
    #[field(label = "Uplink")]
    pub uplink: bool,
    /// The ground station id.
    #[field(label = "Ground station")]
    pub gs_id: i64,
}

/// The messages, newest first.
pub static HFDL_MESSAGES: Table<HfdlMessageRow<'static>> =
    Table::new("messages", "Messages", crate::hfdl::tables::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&HfdlMessageRow::FLIGHT);
/// The aircraft.
pub static HFDL_AIRCRAFT: Table<HfdlAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::hfdl::tables::AIRCRAFT_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&HfdlAircraftRow::FLIGHT);
/// The ground stations.
pub static HFDL_STATIONS: Table<HfdlStationRow<'static>> = Table::new(
    "ground_stations",
    "Ground stations",
    crate::hfdl::tables::STATION_CAP,
)
.label(&HfdlStationRow::NAME);
/// The channels.
pub static HFDL_CHANNELS: Table<HfdlChannelRow<'static>> =
    Table::new("channels", "Channels", crate::hfdl::tables::CHANNEL_CAP)
        .label(&HfdlChannelRow::NAME);
/// Each message.
pub static HFDL_MESSAGE: EventKind<HfdlMessageEvent<'static>> =
    EventKind::new("hfdl.message", "HFDL message");

/// The arguments of `remove` of HFDL.
#[derive(Arguments)]
pub struct HfdlAircraftArgs<'a> {
    /// The row key of the aircraft.
    #[arg(shared = HFDL_AIRCRAFT_ROW)]
    pub row: &'a str,
}

/// The row key of an aircraft of HFDL.
pub const HFDL_AIRCRAFT_ROW: Argument<field::Text> = objcmd::row_of("aircraft");
/// Forgets one aircraft.
pub const HFDL_REMOVE: CommandKind<HfdlAircraftArgs<'static>> = objcmd::remove_kind();

// --- VDL Mode 4 ---

/// The result fields of VDL Mode 4.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4Fields {
    /// Frames with a good FCS.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Bursts that failed on the channels now heard.
    #[field(label = "Bad")]
    pub bad: i64,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// Ground stations in the table.
    #[field(label = "Ground stations")]
    pub ground_stations: i64,
}

/// A row of the messages. The row key is the place in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4MessageRow<'a> {
    /// The wall-clock time. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level of the burst.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The address of the sender, six hex digits.
    #[field(shared = aircraft::ADDRESS, role = Sender)]
    pub address: &'a str,
    /// The kind of the address.
    #[field(label = "Address kind")]
    pub address_kind: &'a str,
    /// The name of the message.
    #[field(label = "Message")]
    pub message: Option<&'a str>,
    /// One line.
    #[field(label = "Summary", role = Text)]
    pub summary: Option<&'a str>,
    /// The text of the DLS.
    #[field(label = "DLS text")]
    pub dls_text: Option<&'a str>,
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude the message gives.
    #[field(label = "Altitude", unit = "ft", role = Altitude)]
    pub alt_ft: Option<f64>,
}

/// A row of the aircraft. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4AircraftRow<'a> {
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
    /// Messages of the aircraft.
    #[field(shared = aircraft::MESSAGES)]
    pub messages: i64,
    /// The last position.
    #[field(shared = aircraft::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(shared = aircraft::ALT_FT)]
    pub alt_ft: Option<f64>,
    /// The track.
    #[field(shared = aircraft::TRACK_DEG)]
    pub track_deg: Option<f64>,
    /// The ground speed.
    #[field(shared = aircraft::GS_KT)]
    pub gs_kt: Option<f64>,
    /// The vertical rate.
    #[field(shared = aircraft::VS_FPM)]
    pub vs_fpm: Option<f64>,
    /// When the aircraft was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Messages heard from it on the link.
    #[field(label = "Messages")]
    pub count: i64,
    /// The channel of the last burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The points of the trail.
    #[field(label = "Trail points")]
    pub trail_points: i64,
}

/// A row of the ground stations. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4StationRow<'a> {
    /// The address, six hex digits.
    #[field(shared = aircraft::ADDRESS)]
    pub address: &'a str,
    /// Messages of the station.
    #[field(label = "Messages")]
    pub count: i64,
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// When the station was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The position the station gave.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A row of the channels heard now. The row key is the frequency in hertz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4ChannelRow {
    /// The channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The channel is inside the band of the input.
    #[field(label = "In band")]
    pub in_band: bool,
    /// Bursts found.
    #[field(label = "Found")]
    pub found: i64,
    /// Good bursts.
    #[field(label = "Good")]
    pub good: i64,
    /// Bad bursts.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The level of the channel.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A VDL4 message. The source is the address of the sender, six hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Vdl4MessageEvent<'a> {
    /// The address of the sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The summary of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The signal to noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What the event is about: an aircraft.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The name of the message.
    #[field(label = "Message")]
    pub message: &'a str,
    /// The position the message gives.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// The messages, newest first.
pub static VDL4_MESSAGES: Table<Vdl4MessageRow<'static>> =
    Table::new("messages", "Messages", crate::vdl4::tables::MESSAGE_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&Vdl4MessageRow::ADDRESS);
/// The aircraft.
pub static VDL4_AIRCRAFT: Table<Vdl4AircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::vdl4::tables::AIRCRAFT_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&Vdl4AircraftRow::FLIGHT);
/// The ground stations.
pub static VDL4_STATIONS: Table<Vdl4StationRow<'static>> = Table::new(
    "ground_stations",
    "Ground stations",
    crate::vdl4::tables::STATION_CAP,
)
.label(&Vdl4StationRow::ADDRESS);
/// The channels heard now.
pub static VDL4_CHANNELS: Table<Vdl4ChannelRow> = Table::new("channels", "Channels", 64);
/// Each message.
pub static VDL4_MESSAGE: EventKind<Vdl4MessageEvent<'static>> =
    EventKind::new("vdl4.message", "VDL4 message");

/// The arguments of `remove` of VDL4.
#[derive(Arguments)]
pub struct Vdl4StationArgs<'a> {
    /// The row key of the ground station.
    #[arg(shared = VDL4_STATION_ROW)]
    pub row: &'a str,
}

/// The row key of a ground station of VDL4.
pub const VDL4_STATION_ROW: Argument<field::Text> = objcmd::row_of("ground_stations");
/// Forgets one ground station.
pub const VDL4_REMOVE: CommandKind<Vdl4StationArgs<'static>> = objcmd::remove_kind();

// --- The description ---

/// A stack whose status component is a decoder: its own sets, then the count of the decoder, its settings and `clear`.
fn decoder_stack(
    s: StackShape,
    settings: &[&'static dyn sdrkit_core::settings::DeclareSetting],
) -> StackShape {
    s.fields::<DecoderFields>()
        .settings(settings)
        .command(&sdrkit_graph::decoder::CLEAR)
}

/// An OFDM structure stack.
fn ofdm_stack(
    id: &'static str,
    settings: &[&'static dyn sdrkit_core::settings::DeclareSetting],
) -> StackShape {
    decoder_stack(
        StackShape::new(id)
            .fields::<OfdmShapeFields>()
            .table(&OFDM_DETECTIONS),
        settings,
    )
    .kept(&["detections"])
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let vdl3 = decoder_stack(
            StackShape::new("vdl3")
                .fields::<Vdl3Fields>()
                .table(&VDL3_BURSTS),
            crate::vdl3::Vdl3Setting::SETTINGS,
        )
        .kept(&[
            "bursts",
            "Vdl3Fields.trainings",
            "Vdl3Fields.frames_ok",
            "Vdl3Fields.frames_bad",
            "Vdl3Fields.m_uplink",
            "Vdl3Fields.m_uplink_3t",
            "Vdl3Fields.handoff",
            "Vdl3Fields.poll_response",
            "Vdl3Fields.m_downlink",
            "Vdl3Fields.net_entry",
            "Vdl3Fields.vd_data",
            "Vdl3Fields.vd_voice",
            "Vdl3Fields.text",
        ]);
        let gbas = decoder_stack(
            StackShape::new("gbas_vdb")
                .fields::<GbasVdbFields>()
                .table(&GBAS_VDB_BURSTS),
            crate::gbas_vdb::gen::GbasSetting::SETTINGS,
        )
        .kept(&["bursts", "GbasVdbFields.bursts"]);
        let acars = StackShape::new("acars")
            .fields::<AcarsFields>()
            .table(&ACARS_MESSAGES)
            .table(&ACARS_AIRCRAFT)
            .table(&ACARS_CHANNELS)
            .points("Aircraft")
            .event(&ACARS_MESSAGE)
            .settings(crate::acars::AcarsSetting::SETTINGS)
            .command(&objcmd::CLEAR)
            .kept(&[
                "messages",
                "aircraft",
                "AcarsFields.aircraft",
                "AcarsFields.messages",
            ]);
        let vdl2 = StackShape::new("vdl2")
            .fields::<Vdl2Fields>()
            .table(&VDL2_MESSAGES)
            .table(&VDL2_AIRCRAFT)
            .table(&VDL2_STATIONS)
            .table(&VDL2_CHANNELS)
            .points("Aircraft and ground stations")
            .event(&VDL2_MESSAGE)
            .settings(crate::vdl2::Vdl2Setting::SETTINGS)
            .command(&objcmd::CLEAR)
            .command(&VDL2_REMOVE)
            .kept(&[
                "messages",
                "aircraft",
                "ground_stations",
                "Vdl2Fields.aircraft",
                "Vdl2Fields.ground_stations",
                "Vdl2Fields.messages",
            ]);
        let selcal = StackShape::new("selcal")
            .fields::<SelcalFields>()
            .table(&SELCAL_CALLS)
            .table(&SELCAL_CODES)
            .series(SELCAL_SPECTRUM)
            .event(sdrkit_proto_tones::components::selcal())
            .settings(crate::selcal::SelcalSetting::SETTINGS)
            .command(&objcmd::CLEAR)
            .command(&SELCAL_REMOVE)
            .kept(&["calls", "codes", "SelcalFields.calls", "SelcalFields.codes"]);
        let airband = StackShape::new("airband")
            .fields::<AirbandFields>()
            .table(&AIRBAND_CHANNELS)
            .table(&AIRBAND_SEEN)
            .table(&AIRBAND_ACTIVITY)
            .event(&AIRBAND_TRANSMISSION)
            .fields::<AirbandTxFields>()
            .settings(crate::airband::AirbandSetting::SETTINGS)
            .command(&objcmd::CLEAR)
            .command(&PTT)
            .command(&HALT)
            .kept(&["seen", "activity", "AirbandFields.transmissions"]);
        let hfdl = StackShape::new("hfdl")
            .fields::<HfdlFields>()
            .table(&HFDL_MESSAGES)
            .table(&HFDL_AIRCRAFT)
            .table(&HFDL_STATIONS)
            .table(&HFDL_CHANNELS)
            .points("Aircraft and ground stations")
            .event(&HFDL_MESSAGE)
            .settings(crate::hfdl::HfdlSetting::SETTINGS)
            .command(&objcmd::CLEAR)
            .command(&HFDL_REMOVE)
            .kept(&[
                "messages",
                "aircraft",
                "ground_stations",
                "HfdlFields.hopper",
                "HfdlFields.aircraft",
            ]);
        let vdl4 = StackShape::new("vdl4")
            .fields::<Vdl4Fields>()
            .table(&VDL4_MESSAGES)
            .table(&VDL4_AIRCRAFT)
            .table(&VDL4_STATIONS)
            .table(&VDL4_CHANNELS)
            .points("Aircraft")
            .event(&VDL4_MESSAGE)
            .settings(crate::vdl4::Vdl4Setting::SETTINGS)
            .command(&objcmd::CLEAR)
            .command(&VDL4_REMOVE)
            .kept(&[
                "messages",
                "aircraft",
                "ground_stations",
                "Vdl4Fields.frames",
                "Vdl4Fields.aircraft",
                "Vdl4Fields.ground_stations",
            ]);
        // The keys of the training sequences differ by a star only, so the starred ones get a name of their own.
        Described::new("avdata", 1, 4)
            .added(
                1,
                2,
                &[
                    "acars.content",
                    "vdl2.content",
                    "vdl4.content",
                    "selcal.content",
                    "airband.content",
                    "hfdl.content",
                    "vdl3.content",
                    "gbas_vdb.content",
                    "ldacs.content",
                    "aeromacs.content",
                ],
            )
            .content(sdrkit_core::ContentKind::Voice, &["airband"])
            .content(
                sdrkit_core::ContentKind::Datalink,
                &["acars", "vdl2", "vdl4", "hfdl", "vdl3", "ldacs", "aeromacs"],
            )
            .content(sdrkit_core::ContentKind::Paging, &["selcal"])
            .content(sdrkit_core::ContentKind::Navigation, &["gbas_vdb"])
            .audio(sdrkit_core::AudioKind::Voice, &["airband"])
            .text(&["acars", "vdl2", "vdl4", "selcal", "hfdl"])
            .option_name("training", "s1*", "s1_star")
            .option_name("training", "s2*", "s2_star")
            .uses(sdrkit_proto_tones::components::interface())
            .shared_source("ofdm_shape", &["ldacs", "aeromacs"])
            .shared_source(
                "report",
                &["acars", "vdl2", "vdl4", "selcal", "airband", "hfdl"],
            )
            .shared_source("messages", &["acars", "vdl2", "vdl4", "hfdl"])
            .shared_source("history", &["acars", "vdl2"])
            .shared_source("d8psk", &["vdl2", "vdl3"])
            .shared_source("filters", &["vdl2", "vdl4", "airband"])
            .shared_source("verify", ALL)
            .shared_source("bench", ALL)
            .stack(acars)
            .stack(vdl2)
            .stack(vdl4)
            .stack(selcal)
            .stack(airband)
            .stack(hfdl)
            .stack(vdl3)
            .stack(gbas)
            .stack(ofdm_stack("ldacs", crate::ldacs::LdacsSetting::SETTINGS))
            .stack(ofdm_stack(
                "aeromacs",
                crate::aeromacs::AeromacsSetting::SETTINGS,
            ))
            .added(1, 1, ADDED_1_1)
    })
}

/// Every stack of the family.
const ALL: &[&str] = &[
    "acars", "vdl2", "vdl4", "selcal", "airband", "hfdl", "vdl3", "gbas_vdb", "ldacs", "aeromacs",
];

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the avdata family does not describe the stack {id}"))
}

/// An empty store of the results of the stack `id`, for its status component.
pub fn results(id: &str) -> Published {
    Published::from_decl(Arc::new(shape(id).results_decl()))
}
