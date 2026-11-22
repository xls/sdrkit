//! The interface of the voice family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::channels::VoiceSetting;
use crate::common::Stage;
use crate::elt121::EltSetting;
use crate::fleetsync::FleetsyncSetting;
use crate::hfpacket::PacketSetting;
use crate::inversion::InversionSetting;
use crate::mdc::MdcSetting;
use crate::plans::Service;
use crate::rx::Demod;
use crate::wmic::WmicSetting;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys;
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_proto_channels::ScanState;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

/// The result fields that every single-channel stack shares.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DetectorFields {
    /// How far the detector is.
    #[field(label = "State")]
    pub state: Stage,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The quality measure of the stack.
    #[field(label = "Quality", unit = "dB")]
    pub quality_db: Option<f64>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: Seconds,
    /// The rate of the input.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Hertz,
    /// The count of good results.
    #[field(label = "Good")]
    pub good: i64,
    /// The count of bad results.
    #[field(label = "Bad")]
    pub bad: i64,
}

/// An event of a single-channel stack. The row key is the number of the event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DetectorEventRow<'a> {
    /// The wall-clock time of an event.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: Seconds,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The result fields of HF packet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfpacketFields<'a> {
    /// The tuning correction.
    #[field(label = "Tone offset", unit = "Hz")]
    pub tone_offset_hz: Hertz,
    /// Different sources heard.
    #[field(label = "Stations")]
    pub stations: i64,
    /// The newest frame in the monitor form.
    #[field(label = "Last frame")]
    pub last_frame: Option<&'a str>,
}

/// A frame with a good FCS. The row key is the number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfpacketFrameRow<'a> {
    /// When a call started.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The source of a frame.
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The digipeater path of a frame.
    #[field(label = "Path")]
    pub path: &'a str,
    /// The frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The protocol identifier.
    #[field(label = "PID")]
    pub pid: Option<i64>,
    /// The information field as text.
    #[field(label = "Information")]
    pub info: &'a str,
    /// The tuning correction.
    #[field(label = "Tone offset", unit = "Hz")]
    pub tone_offset_hz: Hertz,
}

/// A frame with a good FCS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfpacketFrameEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The source of a frame.
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The digipeater path of a frame.
    #[field(label = "Path")]
    pub path: &'a str,
    /// The frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The information field as text.
    #[field(label = "Information")]
    pub info: &'a str,
    /// The tuning correction.
    #[field(label = "Tone offset", unit = "Hz")]
    pub tone_offset_hz: Hertz,
}

/// The result fields of the 121.5 MHz ELT detector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Elt121Fields {
    /// The periods of the sweeps confirm the signal.
    #[field(label = "Confirmed")]
    pub confirmed: bool,
    /// Sweeps a second.
    #[field(label = "Sweep rate", unit = "Hz")]
    pub sweep_rate_hz: Option<f64>,
    /// The first frequency of a sweep.
    #[field(label = "Sweep from", unit = "Hz")]
    pub sweep_top_hz: Option<Hertz>,
    /// The lowest frequency of a sweep.
    #[field(label = "Sweep to", unit = "Hz")]
    pub sweep_bottom_hz: Option<Hertz>,
    /// The modulation of the tone.
    #[field(label = "Modulation", unit = "%")]
    pub depth_pct: Option<f64>,
}

/// An ELT tone found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Elt121FoundEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Sweeps a second.
    #[field(label = "Sweep rate", unit = "Hz")]
    pub sweep_rate_hz: f64,
    /// The first frequency of a sweep.
    #[field(label = "Sweep from", unit = "Hz")]
    pub sweep_top_hz: Hertz,
    /// The lowest frequency of a sweep.
    #[field(label = "Sweep to", unit = "Hz")]
    pub sweep_bottom_hz: Hertz,
}

/// An ELT tone ended.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Elt121EndedEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The result fields of the voice inversion detector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VoiceInversionFields {
    /// Inverted speech is there.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The inversion carrier found.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Option<Hertz>,
    /// The fit of the spectrum as heard.
    #[field(label = "Fit as heard")]
    pub fit_heard: Option<f64>,
    /// The best fit of the flipped spectrum.
    #[field(label = "Fit inverted")]
    pub fit_inverted: Option<f64>,
}

/// Inverted speech found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VoiceInversionFoundEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The inversion carrier found.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Hertz,
}

/// Speech that is not inverted.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VoiceInversionClearEvent<'a> {
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The result fields of the multi-carrier receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VoiceChannelsFields<'a> {
    /// The service.
    #[field(label = "Service")]
    pub submode: Option<Service>,
    /// The name of the plan in use. Absent without a plan.
    #[field(label = "Channel plan")]
    pub plan: Option<&'a str>,
    /// The channels of the plan in use.
    #[field(label = "Plan channels")]
    pub plan_channels: i64,
    /// The raster step in use.
    #[field(label = "Raster step", unit = "Hz")]
    pub raster_step_hz: Hertz,
    /// Carriers followed now.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// Channels with a carrier now.
    #[field(label = "Active channels")]
    pub active_channels: i64,
    /// Channels with an open squelch.
    #[field(label = "Open channels")]
    pub open_channels: i64,
    /// The channel that plays.
    #[field(label = "Playing")]
    pub playing: Option<&'a str>,
    /// The frequency that plays.
    #[field(label = "Playing frequency", unit = "Hz")]
    pub playing_hz: Option<Hertz>,
    /// The scan is on.
    #[field(label = "Scan")]
    pub scan: bool,
    /// What the player does.
    #[field(label = "Scan state")]
    pub scan_state: ScanState,
    /// The scan steps the radio.
    #[field(label = "Scan steps the radio")]
    pub single_tuner: bool,
    /// Where the stepping scan requires the radio.
    #[field(label = "Radio on", unit = "Hz")]
    pub tune_hz: Option<Hertz>,
    /// The capture holds more than one channel.
    #[field(label = "Whole span")]
    pub multi_channel: bool,
    /// Calls since the start.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Hertz,
    /// The centre of the capture.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: Seconds,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
}

/// A call in the call log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CallRow<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// When a call started.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// The channel key.
    #[field(label = "Key")]
    pub key: i64,
    /// The number of the plan.
    #[field(label = "Number")]
    pub number: i64,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The signal time a call started.
    #[field(label = "Start", unit = "s")]
    pub start_s: Seconds,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// The highest level.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The mean level of a call.
    #[field(label = "Mean", unit = "dBFS")]
    pub mean_dbfs: f64,
    /// The demodulator of a call.
    #[field(label = "Demodulator")]
    pub demod: &'a str,
    /// The tone or code heard.
    #[field(label = "Tone")]
    pub tone: &'a str,
    /// The squelch is open.
    #[field(label = "Open")]
    pub open: bool,
}

/// A channel of the plan or a carrier heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ChannelRow<'a> {
    /// The channel key.
    #[field(label = "Key")]
    pub key: i64,
    /// The number of the plan.
    #[field(label = "Number")]
    pub number: i64,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The use of the channel.
    #[field(label = "Use")]
    pub name: &'a str,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The repeater input.
    #[field(label = "Repeater input", unit = "Hz")]
    pub input_hz: Option<Hertz>,
    /// A carrier is there.
    #[field(label = "Active")]
    pub active: bool,
    /// The squelch is open.
    #[field(label = "Open")]
    pub open: bool,
    /// The channel plays.
    #[field(label = "Playing")]
    pub playing: bool,
    /// The channel is in the scan list.
    #[field(label = "In scan")]
    pub in_scan: bool,
    /// The schedule says the channel broadcasts now.
    #[field(label = "On the air")]
    pub on_air: bool,
    /// The channel level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: Option<f64>,
    /// The demodulator in use.
    #[field(label = "Demodulator")]
    pub demod: Demod,
    /// The tone or code heard.
    #[field(label = "Tone")]
    pub tone: &'a str,
    /// The other reading of a DCS word.
    #[field(label = "Tone alias")]
    pub tone_alias: &'a str,
    /// The last tone or code heard on the channel.
    #[field(label = "Last tone")]
    pub last_tone: &'a str,
    /// Calls on the channel.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The time the channel was open in all.
    #[field(label = "Time open", unit = "s")]
    pub total_s: Seconds,
    /// The time since the channel was last open.
    #[field(label = "Last heard", unit = "s")]
    pub last_heard_s: Option<Seconds>,
    /// When the channel was last open.
    #[field(label = "Last heard at")]
    pub last_heard_utc: Option<Time>,
    /// The highest level.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
}

/// A call that ended.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VoiceChannelsCallEvent<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// The channel key.
    #[field(label = "Key")]
    pub key: i64,
    /// The number of the plan.
    #[field(label = "Number")]
    pub number: i64,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The signal time a call started.
    #[field(label = "Start", unit = "s")]
    pub start_s: Seconds,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// The highest level.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The mean level of a call.
    #[field(label = "Mean", unit = "dBFS")]
    pub mean_dbfs: f64,
    /// The demodulator of a call.
    #[field(label = "Demodulator")]
    pub demod: &'a str,
    /// The tone or code heard.
    #[field(label = "Tone")]
    pub tone: &'a str,
}

/// The result fields of the MDC-1200 burst decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mdc1200Fields {
    /// Bursts so far.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Blocks whose check value was wrong.
    #[field(label = "Check failures")]
    pub check_failures: i64,
}

/// A burst in the burst log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mdc1200BurstRow<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// When a call started.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The unit id, 4 hex digits.
    #[field(label = "Unit")]
    pub unit_id: &'a str,
    /// The operation code, 2 hex digits.
    #[field(label = "Operation")]
    pub op: &'a str,
    /// The argument, 2 hex digits.
    #[field(label = "Argument")]
    pub arg: &'a str,
    /// A double packet.
    #[field(label = "Double")]
    pub double: bool,
    /// The four bytes of the second block of a double packet, 8 hex digits.
    #[field(label = "Extra bytes")]
    pub extra: &'a str,
    /// The meaning of the packet.
    #[field(label = "Meaning")]
    pub text: &'a str,
}

/// A burst that was decoded.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mdc1200BurstEvent<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// When a call started.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The unit id, 4 hex digits.
    #[field(label = "Unit")]
    pub unit_id: &'a str,
    /// The operation code, 2 hex digits.
    #[field(label = "Operation")]
    pub op: &'a str,
    /// The argument, 2 hex digits.
    #[field(label = "Argument")]
    pub arg: &'a str,
    /// A double packet.
    #[field(label = "Double")]
    pub double: bool,
    /// The four bytes of the second block of a double packet, 8 hex digits.
    #[field(label = "Extra bytes")]
    pub extra: &'a str,
    /// The meaning of the packet.
    #[field(label = "Meaning")]
    pub text: &'a str,
}

/// A call that ended.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mdc1200CallEvent<'a> {
    /// The number of a call.
    #[field(label = "#")]
    pub serial: i64,
    /// The channel key.
    #[field(label = "Key")]
    pub key: i64,
    /// The number of the plan.
    #[field(label = "Number")]
    pub number: i64,
    /// The short label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The signal time a call started.
    #[field(label = "Start", unit = "s")]
    pub start_s: Seconds,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Seconds,
    /// The highest level.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The mean level of a call.
    #[field(label = "Mean", unit = "dBFS")]
    pub mean_dbfs: f64,
    /// The demodulator of a call.
    #[field(label = "Demodulator")]
    pub demod: &'a str,
    /// The tone or code heard.
    #[field(label = "Tone")]
    pub tone: &'a str,
}

/// The result fields of the FleetSync burst decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetsyncFields {
    /// Bursts so far.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Frames found by their sync word whose check failed.
    #[field(label = "Check failures")]
    pub check_failures: i64,
    /// Fleet and unit pairs heard.
    #[field(label = "Units")]
    pub units: i64,
}

/// A FleetSync burst: a row of the burst log and the fields of its event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetsyncBurstRow<'a> {
    /// The number of the burst.
    #[field(label = "#")]
    pub serial: i64,
    /// When it was decoded.
    #[field(label = "Time")]
    pub time_utc: Option<Time>,
    /// The frequency of the carrier.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The frequency in MHz, 4 decimals.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The fleet number.
    #[field(label = "Fleet")]
    pub fleet: i64,
    /// The unit number.
    #[field(label = "Unit")]
    pub unit: i64,
    /// The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
    #[field(label = "Valid identity")]
    pub in_range: bool,
    /// "FleetSync" or "FleetSync II".
    #[field(label = "Type")]
    pub kind: &'a str,
    /// The signalling rate.
    #[field(label = "Rate", unit = "bit/s")]
    pub bit_rate: i64,
    /// Word 1, 8 hex digits.
    #[field(label = "Word 1")]
    pub word1: &'a str,
    /// Word 2, 8 hex digits.
    #[field(label = "Word 2")]
    pub word2: &'a str,
    /// Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits.
    #[field(label = "Raw fields")]
    pub raw: &'a str,
    /// "good", or for FleetSync II the bits that the code repaired.
    #[field(label = "Check")]
    pub check: &'a str,
    /// The burst in words.
    #[field(label = "Text")]
    pub text: &'a str,
}

/// A fleet and unit heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FleetsyncUnitRow<'a> {
    /// The fleet number.
    #[field(label = "Fleet")]
    pub fleet: i64,
    /// The unit number.
    #[field(label = "Unit")]
    pub unit: i64,
    /// The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
    #[field(label = "Valid identity")]
    pub in_range: bool,
    /// Bursts heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The carrier of its last burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The type of its last burst.
    #[field(label = "Type")]
    pub kind: &'a str,
}

/// The result fields of the wireless microphone finder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmicFields<'a> {
    /// Candidates now.
    #[field(label = "Candidates")]
    pub candidates: i64,
    /// The name of the band plan.
    #[field(label = "Band plan")]
    pub plan: Option<&'a str>,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Hertz,
    /// The centre of the capture.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: Seconds,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
}

/// A carrier that can be a wireless microphone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmicCandidateRow<'a> {
    /// The carrier id of the finder.
    #[field(label = "Id")]
    pub id: i64,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The occupied width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The channel level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The signal-to-noise ratio.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// The peak deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// It looks like analogue FM.
    #[field(label = "FM")]
    pub fm: bool,
    /// Above the threshold now.
    #[field(label = "Active")]
    pub active: bool,
    /// This candidate plays.
    #[field(label = "Listening")]
    pub listening: bool,
    /// The time since it was first reported.
    #[field(label = "Age", unit = "s")]
    pub age_s: Seconds,
    /// The band of the plan that holds it.
    #[field(label = "Band")]
    pub band: &'a str,
}

/// A new candidate carrier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WmicCandidateEvent<'a> {
    /// The carrier id of the finder.
    #[field(label = "Id")]
    pub id: i64,
    /// Where to listen.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// The occupied width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The channel level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The signal-to-noise ratio.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// The peak deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// It looks like analogue FM.
    #[field(label = "FM")]
    pub fm: bool,
    /// The band of the plan that holds it.
    #[field(label = "Band")]
    pub band: &'a str,
}

/// A station heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfpacketStationEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The station.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
}

/// The count of the input guard of a single-channel stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GuardFields {
    /// Input samples replaced because they were not finite or far beyond full scale.
    #[field(shared = keys::BAD_SAMPLES)]
    pub bad_samples: i64,
}

/// The arguments of `listen`.
#[derive(Arguments)]
pub struct ListenArgs {
    /// The id of a candidate, as the `candidates` table shows it.
    #[arg(label = "Candidate", help = "The id of a candidate.")]
    pub id: i64,
}

/// Plays a candidate, or stops it when it plays.
pub const WMIC_LISTEN: CommandKind<ListenArgs> =
    CommandKind::new("listen", "Listen").help("Plays a candidate. The candidate that plays stops.");

/// The events of a single-channel stack, newest first.
pub static DETECTOR_EVENTS: Table<DetectorEventRow<'static>> =
    Table::new("events", "Events", crate::common::EVENT_CAP);
/// The frames of HF packet, newest first.
pub static HFPACKET_FRAMES: Table<HfpacketFrameRow<'static>> =
    Table::new("frames", "Frames", crate::hfpacket::FRAME_CAP).label(&HfpacketFrameRow::SOURCE);
/// A frame with a good FCS.
pub static HFPACKET_FRAME: EventKind<HfpacketFrameEvent<'static>> =
    EventKind::new("hfpacket.frame", "Frame");
/// A station heard for the first time.
pub static HFPACKET_STATION: EventKind<HfpacketStationEvent<'static>> =
    EventKind::new("hfpacket.station", "Station");
/// An ELT tone found.
pub static ELT121_FOUND: EventKind<Elt121FoundEvent<'static>> =
    EventKind::new("elt121.found", "ELT tone found");
/// An ELT tone ended.
pub static ELT121_ENDED: EventKind<Elt121EndedEvent<'static>> =
    EventKind::new("elt121.ended", "ELT tone ended");
/// Inverted speech found.
pub static VOICE_INVERSION_FOUND: EventKind<VoiceInversionFoundEvent<'static>> =
    EventKind::new("voice_inversion.found", "Inverted speech found");
/// Speech that is not inverted.
pub static VOICE_INVERSION_CLEAR: EventKind<VoiceInversionClearEvent<'static>> =
    EventKind::new("voice_inversion.clear", "Speech, not inverted");
/// The call log, newest first.
pub static CALLS: Table<CallRow<'static>> =
    Table::new("calls", "Calls", crate::channels::CALL_CAP).label(&CallRow::LABEL);
/// The channels.
pub static CHANNELS: Table<ChannelRow<'static>> =
    Table::new("channels", "Channels", 512).label(&ChannelRow::NAME);
/// A call that ended.
pub static VOICE_CHANNELS_CALL: EventKind<VoiceChannelsCallEvent<'static>> =
    EventKind::new("voice_channels.call", "Call");
/// The bursts of MDC-1200, newest first.
pub static MDC1200_BURSTS: Table<Mdc1200BurstRow<'static>> =
    Table::new("bursts", "Bursts", crate::mdc::BURST_CAP).label(&Mdc1200BurstRow::LABEL);
/// A call of MDC-1200 that ended.
pub static MDC1200_CALL: EventKind<Mdc1200CallEvent<'static>> =
    EventKind::new("mdc1200.call", "Call");
/// A burst that was decoded.
pub static MDC1200_BURST: EventKind<Mdc1200BurstEvent<'static>> =
    EventKind::new("mdc1200.burst", "Burst");
/// The FleetSync bursts, newest first.
pub static FLEETSYNC_BURSTS: Table<FleetsyncBurstRow<'static>> =
    Table::new("bursts", "Bursts", crate::fleetsync::BURST_CAP).label(&FleetsyncBurstRow::LABEL);
/// The fleet and unit pairs heard.
pub static FLEETSYNC_UNITS: Table<FleetsyncUnitRow<'static>> =
    Table::new("units", "Units", crate::fleetsync::UNIT_CAP);
/// A call of the FleetSync stack that ended.
pub static FLEETSYNC_CALL: EventKind<VoiceChannelsCallEvent<'static>> =
    EventKind::new("fleetsync.call", "Call");
/// A FleetSync burst that was decoded.
pub static FLEETSYNC_BURST: EventKind<FleetsyncBurstRow<'static>> =
    EventKind::new("fleetsync.burst", "Burst");
/// The candidates of the wireless microphone finder.
pub static WMIC_CANDIDATES: Table<WmicCandidateRow<'static>> =
    Table::new("candidates", "Candidates", 64).sorted(
        &WmicCandidateRow::SNR_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// A new candidate carrier.
pub static WMIC_CANDIDATE: EventKind<WmicCandidateEvent<'static>> =
    EventKind::new("wmic.candidate", "Candidate");

/// A single-channel stack `id` with its own result set `F`: the shared fields first, then `F`, the event list, and the
/// count of the input guard.
fn detector<F: Fields>(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<DetectorFields>()
        .fields::<F>()
        .component("guard", |c| c.fields::<GuardFields>())
        .table(&DETECTOR_EVENTS)
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        use sdrkit_proto_tones::components as tones;
        // The event lists, the frame list, the call logs and the lists of channels are what the stacks heard: they
        // stay over a change of frequency, with the counts of frames and stations and the last frame of HF packet. The
        // level, the quality, the state and the measures of a detector belong to the signal of one frequency and are
        // cleared. The candidates of the microphone finder are carriers of the present capture and are cleared.
        Described::new("voice", 1, 2)
            .added(
                1,
                2,
                &[
                    "hfpacket.content",
                    "elt121.content",
                    "voice_inversion.content",
                    "voice_channels.content",
                    "mdc1200.content",
                    "wmic.content",
                    "fleetsync.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Voice,
                &["voice_channels", "mdc1200", "fleetsync"],
            )
            .content(
                sdrkit_core::ContentKind::Measurement,
                &["voice_inversion", "wmic"],
            )
            .content(sdrkit_core::ContentKind::Navigation, &["elt121"])
            .content(sdrkit_core::ContentKind::Utility, &["hfpacket"])
            .audio(
                sdrkit_core::AudioKind::Voice,
                &["voice_inversion", "voice_channels", "mdc1200", "fleetsync"],
            )
            .audio(sdrkit_core::AudioKind::Monitor, &["wmic"])
            .uses(tones::interface())
            .stack(
                detector::<HfpacketFields>("hfpacket")
                    .table(&HFPACKET_FRAMES)
                    .event(&HFPACKET_FRAME)
                    .event(&HFPACKET_STATION)
                    .settings(PacketSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&[
                        "events",
                        "frames",
                        "DetectorFields.good",
                        "DetectorFields.bad",
                        "HfpacketFields.stations",
                        "HfpacketFields.last_frame",
                    ]),
            )
            .stack(
                detector::<Elt121Fields>("elt121")
                    .event(&ELT121_FOUND)
                    .event(&ELT121_ENDED)
                    .settings(EltSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["events"]),
            )
            .stack(
                detector::<VoiceInversionFields>("voice_inversion")
                    .series(SeriesDef {
                        key: crate::inversion::SPECTRUM_SERIES,
                        label: "Long-term spectrum",
                        x_unit: "Hz",
                        y_unit: "dB",
                    })
                    .event(&VOICE_INVERSION_FOUND)
                    .event(&VOICE_INVERSION_CLEAR)
                    .settings(InversionSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["events"]),
            )
            .stack(
                StackShape::new("voice_channels")
                    .fields::<VoiceChannelsFields>()
                    .table(&CALLS)
                    .table(&CHANNELS)
                    .event(&VOICE_CHANNELS_CALL)
                    .event(tones::subaudio())
                    .settings(VoiceSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .kept(&["calls", "channels", "VoiceChannelsFields"]),
            )
            .stack(
                StackShape::new("mdc1200")
                    .fields::<VoiceChannelsFields>()
                    .fields::<Mdc1200Fields>()
                    .table(&CALLS)
                    .table(&CHANNELS)
                    .table(&MDC1200_BURSTS)
                    .event(&MDC1200_CALL)
                    .event(&MDC1200_BURST)
                    .event(tones::subaudio())
                    .settings(MdcSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .kept(&[
                        "calls",
                        "channels",
                        "bursts",
                        "VoiceChannelsFields",
                        "Mdc1200Fields",
                    ]),
            )
            .stack(
                StackShape::new("wmic")
                    .fields::<WmicFields>()
                    .table(&WMIC_CANDIDATES)
                    .event(&WMIC_CANDIDATE)
                    .settings(WmicSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&WMIC_LISTEN),
            )
            // Version 1.1 added the FleetSync stack.
            .added_in(1, 1, |f| {
                f.stack(
                    StackShape::new("fleetsync")
                        .fields::<VoiceChannelsFields>()
                        .fields::<FleetsyncFields>()
                        .table(&CALLS)
                        .table(&CHANNELS)
                        .table(&FLEETSYNC_BURSTS)
                        .table(&FLEETSYNC_UNITS)
                        .event(&FLEETSYNC_CALL)
                        .event(&FLEETSYNC_BURST)
                        .event(tones::subaudio())
                        .settings(FleetsyncSetting::SETTINGS)
                        .command(&objcmd::CLEAR)
                        .kept(&[
                            "calls",
                            "channels",
                            "bursts",
                            "units",
                            "VoiceChannelsFields",
                            "FleetsyncFields",
                        ]),
                )
            })
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
        .unwrap_or_else(|| panic!("the voice family does not describe the stack {id}"))
}
