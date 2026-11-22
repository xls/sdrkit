//! The interface of the digital voice family: the shapes of the result sets, the table rows and the events, the tables,
//! the event types and what each stack declares.

use crate::auto::Detected;
use crate::dcr::DcrRowKind;
use crate::dmr::{Activity, Link};
use crate::items::{
    AccessKind, CallDetail, CallKind, EncState, MessageFormat, PositionFormat, StationType,
};
use crate::monitor::Verdict as HeardVerdict;
use crate::monitor::{self, MonitorSetting};
use crate::results::Verdict;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::{Command, SettingSet, TxContent, Value};
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::calls::CallEnd;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

/// The result fields of the automatic detector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AutoFields {
    /// The stack decoded last.
    #[field(label = "Detected stack")]
    pub detected: Option<Detected>,
}

/// A stack that the detector heard. The row key is the stack id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SeenRow {
    /// A stack the detector decoded.
    #[field(label = "Stack")]
    pub stack: Detected,
    /// Blocks decoded.
    #[field(label = "Blocks decoded")]
    pub blocks: i64,
    /// Signal time of the last decoded block, seconds.
    #[field(label = "Last decoded", unit = "s")]
    pub last_s: f64,
}

/// The result fields of DMR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DmrFields<'a> {
    /// Colour code.
    #[field(label = "Colour code")]
    pub colour_code: Option<i64>,
    /// Link.
    #[field(label = "Link")]
    pub link: Option<Link>,
    /// Slots locked.
    #[field(label = "Slots locked")]
    pub slots_locked: i64,
    /// Activity, slot 1.
    #[field(label = "Activity, slot 1")]
    pub cach_activity_1: Option<Activity>,
    /// Activity, slot 2.
    #[field(label = "Activity, slot 2")]
    pub cach_activity_2: Option<Activity>,
    /// Last activity, slot 1.
    #[field(label = "Last activity, slot 1")]
    pub cach_activity_last_1: Option<Activity>,
    /// Last activity, slot 2.
    #[field(label = "Last activity, slot 2")]
    pub cach_activity_last_2: Option<Activity>,
    /// Talker alias, slot 1.
    #[field(label = "Talker alias, slot 1")]
    pub talker_alias_1: &'a str,
    /// Talker alias, slot 2.
    #[field(label = "Talker alias, slot 2")]
    pub talker_alias_2: &'a str,
}

/// The result fields of dPMR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DpmrFields {
    /// Colour code.
    #[field(label = "Colour code")]
    pub colour_code: Option<i64>,
}

/// The result fields of DCR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DcrFields<'a> {
    /// Frames read after a sync.
    #[field(label = "Frames")]
    pub dcr_frames: i64,
    /// SACCH units that passed their CRC.
    #[field(label = "SACCH good")]
    pub sacch_ok: i64,
    /// SACCH units that failed their CRC.
    #[field(label = "SACCH bad")]
    pub sacch_bad: i64,
    /// Frames whose RICH parity failed.
    #[field(label = "RICH parity bad")]
    pub rich_bad: i64,
    /// Call signs read from a PICH.
    #[field(label = "Call signs read")]
    pub pich_ok: i64,
    /// Data blocks that passed their CRC.
    #[field(label = "Data blocks good")]
    pub data_ok: i64,
    /// Data blocks that failed their CRC.
    #[field(label = "Data blocks bad")]
    pub data_bad: i64,
    /// Voice frames of 20 ms in frames whose SACCH decoded.
    #[field(label = "Voice frames")]
    pub voice_frames: i64,
    /// Calls opened.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The bits that the Viterbi decoder repaired, of the coded bits of the blocks that passed their CRC.
    #[field(label = "Bit errors", unit = "%")]
    pub ber_pct: f64,
    /// The user code of the last message that counted, 0 to 511.
    #[field(label = "Last user code")]
    pub last_user_code: Option<i64>,
    /// The last call sign read, 9 digits.
    #[field(label = "Last call sign")]
    pub last_call_sign: Option<&'a str>,
    /// The plan channel of the tuned frequency, or "not on the plan". No value without a plan.
    #[field(label = "Plan channel")]
    pub channel: Option<&'a str>,
    /// The plan channels in the span of a scan.
    #[field(label = "Plan channels in the span")]
    pub scan_channels: i64,
    /// The decoders of scanned channels that run.
    #[field(label = "Decoders running")]
    pub scan_decoders: i64,
    /// The decoders of scanned channels that decode.
    #[field(label = "Decoders decoding")]
    pub scan_decoding: i64,
    /// What the stack decodes of the voice.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the DCR list: a call, a call end, a call sign, a data block or a message. The row key counts the rows.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DcrEventRow<'a> {
    /// Signal time, seconds.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What the row reports.
    #[field(label = "Kind")]
    pub kind: DcrRowKind,
    /// Who: "UC n", a call sign, "TCH1" or "TCH2", after the plan label of a scanned channel.
    #[field(label = "Identity")]
    pub identity: &'a str,
    /// The text.
    #[field(label = "Text")]
    pub text: &'a str,
    /// The plan label of a scanned channel.
    #[field(label = "Channel")]
    pub channel: Option<&'a str>,
}

/// The result fields of D-STAR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DstarFields {
    /// Headers.
    #[field(label = "Headers")]
    pub headers_ok: i64,
    /// Voice frames.
    #[field(label = "Voice frames")]
    pub dstar_voice_frames: i64,
    /// Inverted sync.
    #[field(label = "Inverted sync")]
    pub sync_inverted: bool,
}

/// The result fields of FreeDV.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreedvFields<'a> {
    /// The active waveform: "700D", "700E", "1600", or empty when none is.
    #[field(label = "Submode")]
    pub submode: &'a str,
    /// The sync state of the 700D waveform: "search", "trial" or "synced"; empty when it does not run.
    #[field(label = "700D sync")]
    pub sync_700d: &'a str,
    /// The sync state of the 700E waveform: "search", "trial" or "synced"; empty when it does not run.
    #[field(label = "700E sync")]
    pub sync_700e: &'a str,
    /// The sync state of the 1600 waveform: "search" or "synced"; empty when it does not run.
    #[field(label = "1600 sync")]
    pub sync_1600: &'a str,
    /// The active waveform held sync at 3 dB or more for 1.5 s.
    #[field(label = "Confirmed")]
    pub confirmed: bool,
    /// The frequency estimate of the active OFDM waveform, relative to the 1500 Hz centre.
    #[field(label = "Frequency offset", unit = "Hz")]
    pub freq_offset_hz: f64,
    /// The sample clock offset of the active OFDM waveform.
    #[field(label = "Clock offset", unit = "ppm")]
    pub clock_offset_ppm: f64,
    /// The last timing metric of the active OFDM waveform (0 to about 1).
    #[field(label = "Timing metric")]
    pub timing_metric: f64,
    /// Unique word bit errors of the bits of the unique word, since the call started.
    #[field(label = "Unique word errors", unit = "%")]
    pub uw_ber_pct: f64,
    /// Frames whose LDPC decoder did not satisfy every check, since the call started.
    #[field(label = "LDPC failed")]
    pub ldpc_failed_frames: i64,
    /// Modem frames decoded with sync.
    #[field(label = "Frames")]
    pub freedv_frames: i64,
    /// Codec frames sent to the speech output.
    #[field(label = "Speech frames")]
    pub speech_frames: i64,
    /// The text line in progress, or the last finished line.
    #[field(label = "Callsign")]
    pub callsign: &'a str,
}

/// The result fields of M17.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct M17Fields {
    /// Link setup frames.
    #[field(label = "Link setup frames")]
    pub lsf_ok: i64,
    /// Stream frames.
    #[field(label = "Stream frames")]
    pub stream_frames: i64,
    /// Packets.
    #[field(label = "Packets")]
    pub packets_ok: i64,
}

sdrkit_core::choice_enum! {
    /// What an M17 transmission sends.
    pub enum M17TxKind {
        /// A voice stream from the microphone input (push-to-talk).
        Voice = ("voice", "Voice"),
        /// A packet: SMS text or data.
        Packet = ("packet", "Packet"),
    }
}

/// The result fields of the M17 sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct M17TxFields {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// What the transmission sends, or the last one sent.
    #[field(label = "Sending")]
    pub tx_kind: Option<M17TxKind>,
    /// The stream or packet frames of the transmission, or of the last one, that are written.
    #[field(label = "Frames sent")]
    pub tx_frames: i64,
}

/// The arguments of `ptt`.
#[derive(Arguments)]
pub struct PttArgs {
    /// Push-to-talk pressed (on) or released (off).
    #[arg(
        label = "On",
        help = "True starts a voice transmission from the microphone input; false ends it after the frame on air, with the end sequence of the stack."
    )]
    pub on: bool,
}

/// The arguments of `send`.
#[derive(Arguments)]
pub struct M17SendArgs<'a> {
    /// The text of the SMS packet.
    #[arg(label = "Text", help = "Up to 821 bytes of UTF-8 text.")]
    pub text: &'a str,
}

/// The arguments of `send_data`.
#[derive(Arguments)]
pub struct M17SendDataArgs {
    /// The protocol byte of the packet.
    #[arg(label = "Protocol", help = "0 to 255; 5 is SMS text.")]
    pub protocol: i64,
    /// The body of the packet.
    #[arg(label = "Data", help = "Up to 822 bytes.")]
    pub data: std::sync::Arc<[u8]>,
}

/// Push-to-talk: a voice stream, Codec 2 at 3200 bit/s from the microphone input.
pub const PTT: CommandKind<PttArgs> = CommandKind::new("ptt", "Push to talk")
    .help("With on true, sends a voice stream from the microphone input (Codec 2, 3200 bit/s) from the setting callsign to the setting destination, until ptt with on false, halt or the time-out.")
    .transmits(TxContent::Voice)
    .stopped_by("ptt");
/// Sends an SMS packet.
pub const SEND: CommandKind<M17SendArgs<'static>> = CommandKind::new("send", "Send a text")
    .help("Sends the text as an SMS packet (protocol 5) from the setting callsign to the setting destination.")
    .transmits(TxContent::Text)
    .stopped_by("halt");
/// Sends a data packet.
pub const SEND_DATA: CommandKind<M17SendDataArgs> = CommandKind::new("send_data", "Send data")
    .help("Sends the bytes as a packet with the protocol byte, from the setting callsign to the setting destination.")
    .transmits(TxContent::Frame)
    .stopped_by("halt");
/// Ends every transmission.
pub const HALT: Command = Command::new("halt", "Halt")
    .help("Ends the transmission after the frame or the burst on air, with the end sequence of the stack.");

sdrkit_core::choice_enum! {
    /// What a FreeDV transmission sends.
    pub enum FreedvTxKind {
        /// Voice on the 700D waveform.
        Voice700D = ("700D", "Voice, 700D"),
        /// Voice on the 700E waveform.
        Voice700E = ("700E", "Voice, 700E"),
        /// Voice on the 1600 waveform.
        Voice1600 = ("1600", "Voice, 1600"),
    }
}

/// The result fields of the FreeDV sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreedvTxFields {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency (the suppressed carrier): the listening frequency plus the transmit offset. No value while
    /// the stack does not transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// What the transmission sends, or the last one sent.
    #[field(label = "Sending")]
    pub tx_kind: Option<FreedvTxKind>,
    /// The modem frames of the transmission, or of the last one, that are written.
    #[field(label = "Frames sent")]
    pub tx_frames: i64,
}

/// Push-to-talk of FreeDV: speech from the microphone input, Codec 2 on the waveform of the submode.
pub const FREEDV_PTT: CommandKind<PttArgs> = CommandKind::new("ptt", "Push to talk")
    .help("With on true, sends the microphone input on the waveform of the submode: Codec 2 1300 on 1600 in the submode 1600, else Codec 2 700C on 700D (700E in the submode 700E); with the setting callsign in the text channel, until ptt with on false, halt or the time-out. The submode RADE does not send.")
    .transmits(TxContent::Voice)
    .stopped_by("ptt");

sdrkit_core::choice_enum! {
    /// What a D-STAR transmission sends.
    pub enum DstarTxKind {
        /// A text in the slow data, with the radio header and voice frames of the silence pattern.
        Text = ("text", "Text"),
        /// A voice transmission from the microphone input (push-to-talk).
        Voice = ("voice", "Voice"),
    }
}

/// The result fields of the D-STAR sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DstarTxFields<'a> {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not
    /// transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// What the transmission sends, or the last one sent.
    #[field(label = "Sending")]
    pub tx_kind: Option<DstarTxKind>,
    /// The voice frames of the transmission, or of the last one, that are written.
    #[field(label = "Frames sent")]
    pub tx_frames: i64,
    /// What the sender puts in the voice frames.
    #[field(label = "Transmit note")]
    pub tx_note: &'a str,
}

/// The arguments of the D-STAR `send`.
#[derive(Arguments)]
pub struct DstarSendArgs<'a> {
    /// The text of the slow data.
    #[arg(label = "Text", help = "Up to 20 printable ASCII characters.")]
    pub text: &'a str,
}

/// Sends a text in the slow data of a D-STAR transmission.
pub const DSTAR_SEND: CommandKind<DstarSendArgs<'static>> = CommandKind::new("send", "Send a text")
    .help("Sends the radio header from the setting callsign to the setting your_call, and the text in the slow data of voice frames that carry the silence pattern. The transmission ends after every slow-data block went out once, or at halt.")
    .transmits(TxContent::Text)
    .stopped_by("halt");

/// Push-to-talk of D-STAR: a voice transmission from the microphone input, coded with the D-STAR voice coder.
pub const DSTAR_PTT: CommandKind<PttArgs> = CommandKind::new("ptt", "Push to talk")
    .help("With on true, sends the radio header from the setting callsign to the setting your_call, then the microphone input in voice frames of 20 ms with the header copy in the slow data, until ptt with on false, halt or the time-out. The speech is coded with the D-STAR voice coder.")
    .transmits(TxContent::Voice)
    .stopped_by("ptt");

sdrkit_core::choice_enum! {
    /// What a DMR transmission sends.
    pub enum DataTxKind {
        /// A text message as short data.
        Text = ("text", "Text"),
        /// A voice call from the microphone input (push-to-talk).
        Voice = ("voice", "Voice"),
    }
}

/// The result fields of the DMR sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DmrTxFields<'a> {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the listening frequency plus the transmit offset. No value while the stack does not
    /// transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// What the transmission sends, or the last one sent.
    #[field(label = "Sending")]
    pub tx_kind: Option<DataTxKind>,
    /// The bursts or frames of the transmission, or of the last one, that are written.
    #[field(label = "Frames sent")]
    pub tx_frames: i64,
    /// What the sender sends.
    #[field(label = "Transmit note")]
    pub tx_note: &'a str,
}

/// The arguments of a text `send` of short data.
#[derive(Arguments)]
pub struct DataSendArgs<'a> {
    /// The text of the message.
    #[arg(
        label = "Text",
        help = "The text of the message; its limit depends on the stack."
    )]
    pub text: &'a str,
}

/// Sends a DMR text message as short data in direct mode.
pub const DMR_SEND: CommandKind<DataSendArgs<'static>> = CommandKind::new("send", "Send a text")
    .help("Sends the text as defined short data in direct mode, on the setting slot with the setting colour_code, from the setting radio_id to the setting called_id, or to the setting talkgroup when called_id is 0: up to 752 ISO 8859-1 characters, or 376 other characters as UTF-16LE.")
    .transmits(TxContent::Text)
    .stopped_by("halt");

/// Push-to-talk of DMR: a voice call from the microphone input in direct mode, coded with the half-rate coder.
pub const DMR_PTT: CommandKind<PttArgs> = CommandKind::new("ptt", "Push to talk")
    .help("With on true, sends a voice call from the microphone input in direct mode, on the setting slot with the setting colour_code, from the setting radio_id to the setting called_id, or to the setting talkgroup when called_id is 0, until ptt with on false, halt or the time-out. The speech is coded with the half-rate coder, three 20 ms frames to a burst.")
    .transmits(TxContent::Voice)
    .stopped_by("ptt");

/// The result fields of the voice monitor.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MonitorFields<'a> {
    /// The call that plays now: a row key of the `calls` table.
    #[field(label = "Playing call", refers_to = "calls")]
    pub playing: Option<&'a str>,
    /// The target held.
    #[field(label = "Held target")]
    pub held: Option<&'a str>,
    /// What the codec plug-ins say: a codec that is not installed or was turned off.
    #[field(label = "Codec")]
    pub codec: Option<&'a str>,
}

/// A call the monitor heard. The row key is the call id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HistoryRow<'a> {
    /// The call a row belongs to: a row key of the `calls` table.
    #[field(label = "Call", refers_to = "calls")]
    pub call: &'a str,
    /// Signal time of the start of a call, seconds.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// Who sends.
    #[field(label = "Source")]
    pub src: &'a str,
    /// Who is called.
    #[field(label = "Target")]
    pub dst: &'a str,
    /// Who is called and its name.
    #[field(label = "Label")]
    pub label: &'a str,
    /// Why a call is or is not heard.
    #[field(label = "Verdict")]
    pub verdict: HeardVerdict,
    /// Frames played.
    #[field(label = "Played frames")]
    pub played_frames: i64,
    /// The call is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The call is on air.
    #[field(label = "Active")]
    pub active: bool,
}

/// A source radio or callsign. The row key is the source.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HeardRow<'a> {
    /// Who sends.
    #[field(label = "Source")]
    pub src: &'a str,
    /// Calls heard.
    #[field(label = "Calls")]
    pub calls: i64,
    /// Time on air.
    #[field(label = "Airtime", unit = "s")]
    pub airtime: Seconds,
    /// Signal time of the end of the last call, seconds.
    #[field(label = "Last heard", unit = "s")]
    pub last_s: f64,
    /// The mean bit error rate.
    #[field(label = "Mean bit errors")]
    pub mean_ber: f64,
    /// An emergency call.
    #[field(label = "Emergency")]
    pub emergency: bool,
}

/// The result fields of NXDN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NxdnFields {
    /// RAN.
    #[field(label = "RAN")]
    pub ran: Option<i64>,
    /// Frames.
    #[field(label = "Frames")]
    pub nxdn_frames: i64,
    /// CRC good.
    #[field(label = "CRC good")]
    pub crc_ok: i64,
    /// CRC bad.
    #[field(label = "CRC bad")]
    pub crc_bad: i64,
}

/// The result fields of P25 phase 1.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct P25Fields {
    /// The network access code heard.
    #[field(label = "NAC")]
    pub nac: Option<i64>,
    /// Frames.
    #[field(label = "Frames")]
    pub p25_frames: i64,
}

/// A channel identifier. The row key is the identifier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IdentifierRow {
    /// Identifier.
    #[field(label = "Identifier")]
    pub id: i64,
    /// VHF or UHF.
    #[field(label = "VHF or UHF")]
    pub vhf_uhf: bool,
    /// Base.
    #[field(label = "Base", unit = "Hz")]
    pub base_hz: Hertz,
    /// Spacing.
    #[field(label = "Spacing", unit = "Hz")]
    pub spacing_hz: Hertz,
    /// Transmit offset.
    #[field(label = "Transmit offset", unit = "Hz")]
    pub tx_offset_hz: Hertz,
    /// Bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bandwidth_hz: Hertz,
}

/// An adjacent site. The row key is the system, the RFSS and the site.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AdjacentRow<'a> {
    /// The system id.
    #[field(label = "System")]
    pub system: i64,
    /// The RF subsystem.
    #[field(label = "RFSS")]
    pub rfss: i64,
    /// The site.
    #[field(label = "Site")]
    pub site: i64,
    /// The channel of a grant for people.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The frequency of a grant.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
}

/// The result fields of P25 phase 2.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct P25p2Fields {
    /// Superframe location.
    #[field(label = "Superframe location")]
    pub location: i64,
}

/// The measurements that every digital voice stack publishes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SharedFields {
    /// The channel level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The noise floor.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: f64,
    /// The carrier to noise ratio of the channel.
    #[field(label = "Carrier to noise", unit = "dB")]
    pub cnr_db: Option<f64>,
    /// The signal to noise ratio on the symbols.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The measured deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Hertz,
    /// The symbol clock error.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The bit error rate.
    #[field(shared = keys::BER)]
    pub ber: f64,
    /// The signal is inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// Blocks that passed their error check.
    #[field(label = "Blocks good")]
    pub blocks_ok: i64,
    /// Blocks that failed their error check.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Frames whose identity field was read.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frame syncs found.
    #[field(label = "Frame syncs")]
    pub syncs: i64,
    /// Reports written.
    #[field(label = "Reports")]
    pub reports: i64,
    /// Why the stack decodes or does not.
    #[field(label = "Verdict")]
    pub verdict: Verdict,
}

/// A row of the call log. The row key is the call id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CallRow<'a> {
    /// The id of a call.
    #[field(label = "Id")]
    pub id: i64,
    /// Signal time of the start of a call, seconds.
    #[field(label = "Start", unit = "s", role = Time)]
    pub start_s: f64,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// The time slot, 0 when the stack has none.
    #[field(label = "Slot")]
    pub slot: i64,
    /// What a transmission carries.
    #[field(label = "Kind")]
    pub kind: CallKind,
    /// Who sends.
    #[field(label = "Source", role = Sender)]
    pub src: &'a str,
    /// Who is called.
    #[field(label = "Target", role = Addressee)]
    pub dst: &'a str,
    /// The target is a group.
    #[field(label = "Group")]
    pub group: bool,
    /// An emergency call.
    #[field(label = "Emergency")]
    pub emergency: bool,
    /// Which access code the call carried.
    #[field(label = "Access code")]
    pub access_kind: Option<AccessKind>,
    /// The value of the access code.
    #[field(label = "Access")]
    pub access_code: Option<i64>,
    /// Encrypted or not.
    #[field(label = "Encryption")]
    pub encryption: EncState,
    /// The name of the algorithm.
    #[field(label = "Algorithm")]
    pub algorithm: &'a str,
    /// The algorithm id as sent.
    #[field(label = "Algorithm id")]
    pub alg_id: Option<i64>,
    /// The key id.
    #[field(label = "Key")]
    pub enc_key_id: Option<i64>,
    /// The talker alias.
    #[field(label = "Alias")]
    pub alias: &'a str,
    /// Text carried by a transmission.
    #[field(label = "Text")]
    pub text: &'a str,
    /// Voice frames received.
    #[field(label = "Voice frames")]
    pub voice_frames: i64,
    /// Voice frames passed to the media output.
    #[field(label = "Media frames")]
    pub media_frames: i64,
    /// The bit error rate of a call.
    #[field(label = "Bit errors")]
    pub ber: f64,
    /// The call is on air.
    #[field(label = "Active")]
    pub active: bool,
    /// The call started from an identity inside it.
    #[field(label = "Late entry")]
    pub late_entry: bool,
    /// The call ended by its terminator.
    #[field(label = "End seen")]
    pub end_seen: bool,
    /// The call ended after the hang time.
    #[field(label = "Signal lost")]
    pub signal_lost: bool,
    /// The carrier frequency.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Hertz,
    /// Where a call is in its life.
    #[field(label = "End")]
    pub end: CallEnd,
}

/// A call started, ended, or was a single transmission. The field `end` is `active` when it started.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CallEvent<'a> {
    /// Who sends.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Who is called.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The talkgroup of a group call, else the time slot.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// How urgent it is.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The content is encrypted.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// The id of a call.
    #[field(label = "Id")]
    pub id: i64,
    /// What a transmission carries.
    #[field(label = "Kind")]
    pub kind: CallKind,
    /// The target is a group.
    #[field(label = "Group")]
    pub group: bool,
    /// Where a call is in its life.
    #[field(label = "End")]
    pub end: CallEnd,
    /// How long a call lasted.
    #[field(label = "Duration", unit = "s")]
    pub duration: Option<Seconds>,
}

/// A fact of a call that only some stacks send. The row key is the call id, a slash and the detail key.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CallDetailRow<'a> {
    /// The call a row belongs to: a row key of the `calls` table.
    #[field(label = "Call", refers_to = "calls")]
    pub call: &'a str,
    /// Which fact a detail row holds.
    #[field(label = "Detail")]
    pub detail: CallDetail,
    /// The value of a detail.
    #[field(label = "Value")]
    pub value: &'a Value,
}

/// A text message. The row key counts the messages.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MessageRow<'a> {
    /// Signal time, seconds.
    #[field(label = "Time", unit = "s", role = Time)]
    pub time_s: f64,
    /// Who sends.
    #[field(label = "Source", role = Sender)]
    pub src: &'a str,
    /// Who is called.
    #[field(label = "Target", role = Addressee)]
    pub dst: &'a str,
    /// The target is a group.
    #[field(label = "Group")]
    pub group: bool,
    /// How a message was sent.
    #[field(label = "Format")]
    pub format: MessageFormat,
    /// The DMR short data format code.
    #[field(label = "Data format")]
    pub data_format: Option<i64>,
    /// Text carried by a transmission.
    #[field(label = "Text", role = Text)]
    pub text: &'a str,
    /// The error check was good.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
    /// The time slot, 0 when the stack has none.
    #[field(label = "Slot")]
    pub slot: i64,
    /// An SMS.
    #[field(label = "SMS")]
    pub sms: bool,
}

/// A position. The row key counts the positions.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PositionRow<'a> {
    /// Signal time, seconds.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// Who sends.
    #[field(label = "Source")]
    pub src: &'a str,
    /// The position, latitude first.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The height.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// The speed.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The accuracy of a position.
    #[field(label = "Accuracy", unit = "m")]
    pub accuracy_m: Option<f64>,
    /// How a position was sent.
    #[field(label = "Format")]
    pub format: PositionFormat,
    /// The comment sent with a position.
    #[field(label = "Comment")]
    pub comment: &'a str,
    /// The kind of station that sent a position.
    #[field(label = "Station")]
    pub station_type: Option<StationType>,
    /// The data source code of a position.
    #[field(label = "Data source")]
    pub data_source: Option<i64>,
}

/// The site and system identity of the stack. The row key is the stack id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SiteRow<'a> {
    /// Signal time, seconds.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The identity is complete.
    #[field(label = "Complete")]
    pub valid: bool,
    /// The system id.
    #[field(label = "System")]
    pub system: Option<i64>,
    /// The wide area communications network.
    #[field(label = "WACN")]
    pub wacn: Option<i64>,
    /// The RF subsystem.
    #[field(label = "RFSS")]
    pub rfss: Option<i64>,
    /// The site.
    #[field(label = "Site")]
    pub site: Option<i64>,
    /// The network access code.
    #[field(label = "NAC")]
    pub nac: Option<i64>,
    /// The control channel.
    #[field(label = "Control channel")]
    pub control_channel: Option<&'a str>,
    /// The location id.
    #[field(label = "Location")]
    pub location: Option<i64>,
    /// The radio access number.
    #[field(label = "RAN")]
    pub ran: Option<i64>,
    /// The country code.
    #[field(label = "Country")]
    pub country: Option<i64>,
    /// The network code.
    #[field(label = "Network")]
    pub network: Option<i64>,
    /// The base station.
    #[field(label = "Base station")]
    pub base_station: Option<i64>,
    /// The channel of a site.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The colour code of a cell.
    #[field(label = "Colour code")]
    pub colour_code: Option<i64>,
    /// The location area of a cell.
    #[field(label = "Location area")]
    pub location_area: Option<i64>,
    /// The main carrier number of a cell.
    #[field(label = "Main carrier")]
    pub main_carrier: Option<i64>,
    /// The frequency band of a cell.
    #[field(label = "Band")]
    pub band: Option<i64>,
    /// The main carrier frequency of a cell.
    #[field(label = "Main carrier frequency", unit = "Hz")]
    pub main_carrier_hz: Option<Hertz>,
}

/// A channel grant. The row key counts the grants.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GrantRow<'a> {
    /// Signal time, seconds.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The talkgroup or radio granted.
    #[field(label = "Target")]
    pub target: i64,
    /// The target is a group.
    #[field(label = "Group")]
    pub group: bool,
    /// The source of a grant request.
    #[field(label = "Source")]
    pub source_id: i64,
    /// The channel number of a grant as sent.
    #[field(label = "Channel number")]
    pub channel_number: i64,
    /// The channel of a grant for people.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The frequency of a grant.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// The time slot, 0 when the stack has none.
    #[field(label = "Slot")]
    pub slot: Option<i64>,
    /// An emergency call.
    #[field(label = "Emergency")]
    pub emergency: bool,
    /// The call is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The priority of a grant.
    #[field(label = "Priority")]
    pub priority: i64,
    /// An update of a running call.
    #[field(label = "Update")]
    pub update: bool,
}

/// A talkgroup heard, with its counters. The row key is the talkgroup.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TalkgroupRow {
    /// A talkgroup.
    #[field(label = "Talkgroup")]
    pub talkgroup: i64,
    /// Grants to a talkgroup.
    #[field(label = "Grants")]
    pub grants: i64,
    /// Voice calls to a talkgroup.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The call is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A text message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MessageEvent<'a> {
    /// Who sends.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Who is called.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How a message was sent.
    #[field(label = "Format")]
    pub format: MessageFormat,
    /// The target is a group.
    #[field(label = "Group")]
    pub group: bool,
}

/// A position.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PositionEvent<'a> {
    /// Who sends.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The position, latitude first.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The height.
    #[field(shared = keys::HEIGHT_M)]
    pub height_m: Option<f64>,
    /// The speed.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How a position was sent.
    #[field(label = "Format")]
    pub format: PositionFormat,
}

/// The result fields of TETRA.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TetraFields {
    /// Bursts.
    #[field(label = "Bursts")]
    pub bursts: i64,
}

/// The result fields of TETRA DMO.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TetraDmoFields<'a> {
    /// Synchronisation bursts.
    #[field(label = "Synchronisation bursts")]
    pub dsb: i64,
    /// Normal bursts.
    #[field(label = "Normal bursts")]
    pub dnb: i64,
    /// SCH/F and STCH good.
    #[field(label = "SCH/F and STCH good")]
    pub data_blocks: i64,
    /// SCH/S and SCH/H bad.
    #[field(label = "SCH/S and SCH/H bad")]
    pub sync_blocks_bad: i64,
    /// Traffic bursts.
    #[field(label = "Traffic bursts")]
    pub traffic_bursts: i64,
    /// Traffic refused.
    #[field(label = "Traffic refused")]
    pub traffic_refused: i64,
    /// Communication type.
    #[field(label = "Communication type")]
    pub comm_type: Option<i64>,
    /// Frame.
    #[field(label = "Frame")]
    pub frame: Option<i64>,
    /// Slot.
    #[field(label = "Slot")]
    pub slot: Option<i64>,
    /// Encryption state.
    #[field(label = "Encryption state")]
    pub aie_state: Option<i64>,
    /// Last source.
    #[field(label = "Last source")]
    pub last_source: Option<&'a str>,
    /// Last destination.
    #[field(label = "Last destination")]
    pub last_destination: Option<&'a str>,
    /// MNI.
    #[field(label = "MNI")]
    pub mni: Option<&'a str>,
    /// Note.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The result fields of TETRAPOL.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TetrapolFields {
    /// Voice frames.
    #[field(label = "Voice frames")]
    pub tetrapol_voice_frames: i64,
    /// Signalling frames whose CRC checks.
    #[field(label = "Signalling frames")]
    pub tetrapol_signalling_frames: i64,
}

/// The result fields of System Fusion.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct YsfFields<'a> {
    /// Data type.
    #[field(label = "Data type")]
    pub data_type: &'a str,
}

/// The call log of DMR (at most 100 entries).
pub static CALLS_100: Table<CallRow<'static>> = Table::new("calls", "Calls", 100)
    .role(sdrkit_core::TableRole::Calls)
    .label(&CallRow::ALIAS);
/// The call log of the other stacks (at most 300 entries).
pub static CALLS_300: Table<CallRow<'static>> = Table::new("calls", "Calls", 300)
    .role(sdrkit_core::TableRole::Calls)
    .label(&CallRow::ALIAS);
/// The facts of a call that only some stacks send.
pub static CALL_DETAILS: Table<CallDetailRow<'static>> =
    Table::new("call_details", "Call details", 3000).label(&CallDetailRow::CALL);
/// Text messages.
pub static MESSAGES: Table<MessageRow<'static>> = Table::new("messages", "Messages", 100)
    .role(sdrkit_core::TableRole::Messages)
    .label(&MessageRow::SRC);
/// Positions.
pub static POSITIONS: Table<PositionRow<'static>> =
    Table::new("positions", "Positions", crate::calls::POSITION_CAP).label(&PositionRow::SRC);
/// The site and system identity of the stack.
pub static SITES: Table<SiteRow<'static>> = Table::new("sites", "Sites", 1);
/// Channel grants.
pub static GRANTS: Table<GrantRow<'static>> =
    Table::new("grants", "Grants", crate::calls::GRANT_CAP);
/// Talkgroups heard, with their counters.
pub static TALKGROUPS: Table<TalkgroupRow> =
    Table::new("talkgroups", "Talkgroups", crate::calls::TALKGROUP_CAP)
        .role(sdrkit_core::TableRole::Groups);
/// The stacks that the automatic detector heard.
pub static SEEN: Table<SeenRow> = Table::new("modes_seen", "Modes heard", 16);
/// The channel identifiers of a P25 site.
pub static IDENTIFIERS: Table<IdentifierRow> = Table::new("identifiers", "Channel identifiers", 16);
/// The sites next to a P25 site.
pub static ADJACENT: Table<AdjacentRow<'static>> =
    Table::new("adjacent_sites", "Adjacent sites", 64);
/// The rows of DCR: calls, call ends, call signs, data blocks and messages.
pub static DCR_EVENTS: Table<DcrEventRow<'static>> =
    Table::new("events", "Events", crate::dcr::ROW_CAP).label(&DcrEventRow::IDENTITY);
/// The calls the monitor heard, newest first.
pub static HISTORY: Table<HistoryRow<'static>> =
    Table::new("history", "Heard calls", monitor::HISTORY_CAP).label(&HistoryRow::CALL);
/// One row per source radio or callsign.
pub static HEARD: Table<HeardRow<'static>> =
    Table::new("heard", "Heard stations", monitor::HEARD_CAP)
        .role(sdrkit_core::TableRole::Nodes)
        .label(&HeardRow::SRC);

/// A call started, ended, or was a single transmission.
pub static CALL: EventKind<CallEvent<'static>> = EventKind::new("dvoice.call", "Call");
/// A text message.
pub static MESSAGE: EventKind<MessageEvent<'static>> = EventKind::new("dvoice.message", "Message");
/// A position.
pub static POSITION: EventKind<PositionEvent<'static>> =
    EventKind::new("dvoice.position", "Position");

/// The series of the channel spectrum.
pub const SPECTRUM: &str = "spectrum";
/// The series of the eye.
pub const EYE: &str = "eye";
/// The series of the level histogram.
pub const LEVELS: &str = "levels";
/// The series of the SNR history.
pub const SNR_HISTORY: &str = "snr_history";
/// The series of the bit error history.
pub const BER_HISTORY: &str = "ber_history";

/// The arguments of `remove`.
#[derive(Arguments)]
pub struct TalkgroupArgs<'a> {
    /// The row key of the talkgroup.
    #[arg(shared = TALKGROUP_ROW)]
    pub row: &'a str,
}

/// The row key of a talkgroup.
pub const TALKGROUP_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    objcmd::row_of("talkgroups");
/// Forgets one talkgroup.
pub const REMOVE: CommandKind<TalkgroupArgs<'static>> = objcmd::remove_kind();

/// What every stack of the family declares, with its call log `calls`: the shared measurements, the lists, the
/// pictures and the events. The result sets and tables of the stack and of the monitor follow.
fn common(id: &'static str, calls: &'static Table<CallRow<'static>>) -> StackShape {
    let series = |key, label, x_unit, y_unit| SeriesDef {
        key,
        label,
        x_unit,
        y_unit,
    };
    StackShape::new(id)
        .fields::<SharedFields>()
        .table(calls)
        .table(&CALL_DETAILS)
        .table(&MESSAGES)
        .table(&POSITIONS)
        .table(&SITES)
        .table(&GRANTS)
        .table(&TALKGROUPS)
        .series(series(EYE, "Eye", "", ""))
        .series(series(LEVELS, "Level histogram", "", ""))
        .series(series(SPECTRUM, "Channel spectrum", "Hz", "dB"))
        .series(series(SNR_HISTORY, "SNR history", "", "dB"))
        .series(series(BER_HISTORY, "Bit error history", "", ""))
        .points("Positions")
        .event(&CALL)
        .event(&MESSAGE)
        .event(&POSITION)
}

/// The stack `shape` with what the monitor and the decoder component add: the result sets, the tables, the settings
/// of the monitor and the commands.
fn finish(shape: StackShape) -> StackShape {
    shape
        .fields::<MonitorFields>()
        .fields::<DecoderFields>()
        .table(&HISTORY)
        .table(&HEARD)
        .settings(MonitorSetting::SETTINGS)
        .command(&sdrkit_graph::decoder::CLEAR)
        .command(&monitor::HOLD_CURRENT)
        .command(&monitor::RELEASE)
        .command(&REMOVE)
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let plain = |id| common(id, &CALLS_300);
        Described::new("dvoice", 2, 3)
            .added(2, 2, &["dstar.command.ptt", "dstar.setting.tx_timeout"])
            .added(
                2,
                1,
                &[
                    "dmr.content",
                    "p25.content",
                    "m17.content",
                    "dstar.content",
                    "nxdn.content",
                    "dpmr.content",
                    "ysf.content",
                    "p25p2.content",
                    "tetrapol.content",
                    "tetra.content",
                    "tetra_dmo.content",
                    "freedv.content",
                    "dvoice-auto.content",
                    "dcr.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Voice,
                &[
                    "dmr",
                    "p25",
                    "m17",
                    "dstar",
                    "nxdn",
                    "dpmr",
                    "ysf",
                    "p25p2",
                    "tetrapol",
                    "tetra",
                    "tetra_dmo",
                    "freedv",
                    "dvoice-auto",
                    "dcr",
                ],
            )
            .audio(
                sdrkit_core::AudioKind::Voice,
                &[
                    "dmr",
                    "p25",
                    "m17",
                    "dstar",
                    "nxdn",
                    "dpmr",
                    "ysf",
                    "p25p2",
                    "tetrapol",
                    "tetra",
                    "tetra_dmo",
                    "freedv",
                    "dvoice-auto",
                    "dcr",
                ],
            )
            .stack(
                finish(common("dmr", &CALLS_100).fields::<DmrFields>())
                    .settings(&[&crate::dmr::GEN_CC])
                    .fields::<DmrTxFields>()
                    .settings(crate::dmr_tx::TxSetting::SETTINGS)
                    .command(&DMR_PTT)
                    .command(&DMR_SEND)
                    .command(&HALT),
            )
            .stack(
                finish(
                    plain("p25")
                        .fields::<P25Fields>()
                        .table(&IDENTIFIERS)
                        .table(&ADJACENT),
                )
                .settings(&[&crate::p25::GEN_NAC]),
            )
            .stack(
                finish(plain("m17").fields::<M17Fields>())
                    .fields::<M17TxFields>()
                    .settings(crate::m17_tx::TxSetting::SETTINGS)
                    .command(&PTT)
                    .command(&SEND)
                    .command(&SEND_DATA)
                    .command(&HALT),
            )
            .stack(
                finish(plain("dstar").fields::<DstarFields>())
                    .fields::<DstarTxFields>()
                    .settings(crate::dstar_tx::TxSetting::SETTINGS)
                    .command(&DSTAR_PTT)
                    .command(&DSTAR_SEND)
                    .command(&HALT),
            )
            .stack(finish(plain("nxdn").fields::<NxdnFields>()))
            .stack(finish(plain("dpmr").fields::<DpmrFields>()))
            .stack(finish(plain("ysf").fields::<YsfFields>()).settings(&[&crate::ysf::GEN_VOICE]))
            .stack(finish(plain("p25p2").fields::<P25p2Fields>()))
            .stack(finish(plain("tetrapol").fields::<TetrapolFields>()))
            .stack(finish(plain("tetra").fields::<TetraFields>()))
            .stack(
                finish(plain("tetra_dmo").fields::<TetraDmoFields>())
                    .settings(crate::tetra_dmo::settings::SETTINGS),
            )
            .stack(
                finish(plain("freedv").fields::<FreedvFields>())
                    .settings(&[&crate::freedv_ofdm::GEN_CLIP])
                    .fields::<FreedvTxFields>()
                    .settings(crate::freedv_tx::TxSetting::SETTINGS)
                    .command(&FREEDV_PTT)
                    .command(&HALT),
            )
            .stack(finish(
                // The detector publishes the result sets of the stacks that it can detect. dPMR has the colour code of
                // DMR and no other field.
                plain("dvoice-auto")
                    .fields::<AutoFields>()
                    .fields::<DmrFields>()
                    .fields::<NxdnFields>()
                    .fields::<P25Fields>()
                    .fields::<YsfFields>()
                    .fields::<M17Fields>()
                    .table(&SEEN)
                    .table(&IDENTIFIERS)
                    .table(&ADJACENT),
            ))
            .stack(
                finish(plain("dcr").fields::<DcrFields>().table(&DCR_EVENTS))
                    .settings(crate::dcr::SETTINGS),
            )
    })
}
