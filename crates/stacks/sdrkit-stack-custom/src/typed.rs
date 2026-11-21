//! The interface of the custom family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::{arq_packet, file_transfer, hf_chat, picture_burst, qam_video, slotted_voice};
use sdrkit_core::interface::{EventKind, Family, Fields, Hertz, StackShape, Table, Time};
use sdrkit_core::publish::keys;
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use std::sync::OnceLock;

// What every stack shares.

/// The fields of a transmission that started, finished or was refused.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxEvent<'a> {
    /// What the transmission did: `started`, `finished` or `refused`.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why the transmission was refused or ended early; empty for none.
    #[field(label = "Reason")]
    pub reason: &'a str,
}

// slotted_voice

/// The result fields of `slotted_voice`. A value of the signal has no value while no receiver is locked.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlottedVoiceFields<'a> {
    /// A receiver is locked.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The frame version received, 3 or 4.
    #[field(label = "Frame version")]
    pub frame_version: Option<i64>,
    /// The voice bit rate.
    #[field(label = "Voice bit rate", unit = "kbit/s")]
    pub voice_kbps: Option<f64>,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Words or slots that decoded, of the last receiver that was locked.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: Option<i64>,
    /// Words or slots that did not decode, of the last receiver that was locked.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: Option<i64>,
    /// The station identity of the sweep, while a receiver reads it.
    #[field(label = "Sweep identity")]
    pub sweep_id: Option<&'a str>,
    /// The sweep level relative to the voice.
    #[field(label = "Sweep level", unit = "dB")]
    pub sweep_db: Option<f64>,
    /// The callsign of the newest beacon.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the stations heard in beacons. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlottedVoiceStationRow<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The grid locator.
    #[field(shared = keys::LOCATOR)]
    pub locator: &'a str,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The beacon power value.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: f64,
    /// The link quality value.
    #[field(label = "Link quality")]
    pub link: i64,
    /// The beacon text, or the text of the last full beacon of the station.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// When the station was last heard.
    #[field(label = "Last heard")]
    pub last_heard: Time,
}

/// The fields of a station beacon.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlottedVoiceStationEvent<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The grid locator.
    #[field(shared = keys::LOCATOR)]
    pub locator: &'a str,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The beacon power value.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: f64,
    /// The beacon text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The stations heard in beacons.
pub static SLOTTED_VOICE_STATIONS: Table<SlottedVoiceStationRow<'static>> =
    Table::new("stations", "Stations heard", 200);
/// A beacon of a station.
pub static SLOTTED_VOICE_STATION: EventKind<SlottedVoiceStationEvent<'static>> =
    EventKind::new("voice.station", "Station beacon");
/// A voice transmission started, finished or was refused.
pub static SLOTTED_VOICE_TX: EventKind<TxEvent<'static>> =
    EventKind::new("voice.tx", "Voice transmission");

// qam_video

/// The result fields of `qam_video`. A value of the last frame has no value before the first frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QamVideoFields<'a> {
    /// A frame header passed in the last 2 s.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The profile of the last frame.
    #[field(label = "Profile")]
    pub profile: Option<&'a str>,
    /// The Es/N0 of the last frame.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// The carrier offset of the last frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Frames with all four blocks good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames with a failed block.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Blocks that failed.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Groups restored by the erasure code.
    #[field(label = "Groups restored")]
    pub groups_restored: i64,
    /// Groups lost.
    #[field(label = "Groups lost")]
    pub groups_lost: i64,
    /// Packets received.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The callsign of the station identification.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The symbol rate of the listen profile.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the streams heard. The row key is the packet type and the stream id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QamVideoStreamRow<'a> {
    /// The stream id.
    #[field(label = "Stream")]
    pub stream: &'a str,
    /// The packet type: video, audio, text, picture, data or other.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// Packets of the stream.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// The streams of the signal.
pub static QAM_VIDEO_STREAMS: Table<QamVideoStreamRow<'static>> =
    Table::new("streams", "Streams", 64);
/// A transmission of the video link started, finished or was refused.
pub static QAM_VIDEO_TX: EventKind<TxEvent<'static>> =
    EventKind::new("video.tx", "Video link transmission");

// picture_burst

/// The result fields of `picture_burst`. A value of the last burst has no value before the first decoded burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PictureBurstFields<'a> {
    /// The header of the last burst decoded.
    #[field(label = "Header good")]
    pub header_ok: Option<bool>,
    /// The profile of the last burst.
    #[field(label = "Profile")]
    pub profile: Option<i64>,
    /// Layers of the last burst decoded.
    #[field(label = "Layers decoded")]
    pub layers: Option<i64>,
    /// Layers of the last burst sent.
    #[field(label = "Layers sent")]
    pub layers_sent: Option<i64>,
    /// The SNR of the last burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The clock error of the last burst, if measured.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The callsign of the last burst.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// Bursts seen.
    #[field(label = "Bursts seen")]
    pub bursts: i64,
    /// Bursts decoded.
    #[field(label = "Bursts decoded")]
    pub bursts_decoded: i64,
    /// A burst is being received, or one was decoded.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the pictures received. The row key is the number of the decoded burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PictureBurstPictureRow<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The picture size, "width x height".
    #[field(label = "Size")]
    pub size: &'a str,
    /// Layers decoded.
    #[field(label = "Layers decoded")]
    pub layers: i64,
    /// When the burst was decoded.
    #[field(label = "Time")]
    pub time: Time,
}

/// The fields of a decoded burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PictureBurstEvent<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The picture size, "width x height".
    #[field(label = "Size")]
    pub size: &'a str,
    /// Layers decoded.
    #[field(label = "Layers decoded")]
    pub layers: i64,
}

/// The pictures received.
pub static PICTURE_BURST_PICTURES: Table<PictureBurstPictureRow<'static>> =
    Table::new("pictures", "Pictures", 100);
/// A burst was decoded.
pub static PICTURE_BURST_PICTURE: EventKind<PictureBurstEvent<'static>> =
    EventKind::new("picture.burst", "Picture received");
/// A picture transmission started, finished or was refused.
pub static PICTURE_BURST_TX: EventKind<TxEvent<'static>> =
    EventKind::new("picture.tx", "Picture transmission");

// arq_packet

/// The result fields of `arq_packet`. A value of the last burst has no value before the first burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArqPacketFields<'a> {
    /// Blocks whose CRC passed.
    #[field(label = "Blocks good")]
    pub blocks_good: i64,
    /// Blocks whose CRC failed.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// The profile of the last burst.
    #[field(label = "Profile")]
    pub profile: Option<i64>,
    /// The Es/N0 of the last burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The callsign of the last burst.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// Bursts whose header was read.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Messages listed.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Messages heard again within 120 s.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// Acknowledgement bursts heard.
    #[field(label = "Acknowledgements heard")]
    pub acks_heard: i64,
    /// A burst header was read in the last 5 s.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The sequence number of the last burst.
    #[field(label = "Sequence")]
    pub sequence: Option<i64>,
}

/// A row of the messages received. The row key is a running number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArqPacketMessageRow<'a> {
    /// The sending station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The sequence number of the last burst of the message.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// The message length.
    #[field(label = "Length", unit = "bytes")]
    pub length: i64,
    /// The message as text.
    #[field(label = "Text")]
    pub text: &'a str,
    /// When the message was completed.
    #[field(label = "Time")]
    pub time: Time,
}

/// The fields of a completed message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArqPacketMessageEvent<'a> {
    /// The sending station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The sequence number of the last burst of the message.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// The message length.
    #[field(label = "Length", unit = "bytes")]
    pub length: i64,
    /// The message as text.
    #[field(label = "Text")]
    pub text: &'a str,
}

/// The fields of the acknowledgement that the receiver would send for a data burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArqPacketAckOutEvent<'a> {
    /// The sending station of the burst.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The sequence number of the burst.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// Blocks of the burst.
    #[field(label = "Blocks")]
    pub blocks: i64,
    /// Blocks acknowledged.
    #[field(label = "Blocks acknowledged")]
    pub blocks_acked: i64,
    /// The Es/N0 of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The fields of an acknowledgement burst heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArqPacketAckInEvent<'a> {
    /// The acknowledging station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The sequence number that it acknowledges.
    #[field(label = "Sequence")]
    pub sequence: i64,
    /// Blocks acknowledged.
    #[field(label = "Blocks acknowledged")]
    pub blocks_acked: i64,
    /// The Es/N0 that it reports.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The messages received.
pub static ARQ_PACKET_MESSAGES: Table<ArqPacketMessageRow<'static>> =
    Table::new("messages", "Messages", 200);
/// A message was completed.
pub static ARQ_PACKET_MESSAGE: EventKind<ArqPacketMessageEvent<'static>> =
    EventKind::new("packet.message", "Message received");
/// The acknowledgement that the receiver would send for a data burst.
pub static ARQ_PACKET_ACK_OUT: EventKind<ArqPacketAckOutEvent<'static>> =
    EventKind::new("packet.ack_out", "Acknowledgement to send");
/// An acknowledgement burst was heard.
pub static ARQ_PACKET_ACK_IN: EventKind<ArqPacketAckInEvent<'static>> =
    EventKind::new("packet.ack_in", "Acknowledgement heard");
/// A packet transmission started, finished or was refused.
pub static ARQ_PACKET_TX: EventKind<TxEvent<'static>> =
    EventKind::new("packet.tx", "Packet transmission");

// file_transfer

/// The result fields of `file_transfer`. A value of the last frame has no value before the first frame, and a value of
/// the transfer none while no transfer is announced.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FileTransferFields<'a> {
    /// The session id of the active transfer.
    #[field(label = "Session")]
    pub session: Option<i64>,
    /// Blocks whose CRC passed.
    #[field(label = "Blocks good")]
    pub blocks_good: i64,
    /// Blocks whose CRC failed.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Frames whose header was read.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Bytes of the active transfer received.
    #[field(label = "Progress", unit = "%")]
    pub progress: Option<f64>,
    /// The last file passed its hash.
    #[field(label = "Hash good")]
    pub hash_ok: Option<bool>,
    /// The profile of the last frame.
    #[field(label = "Profile")]
    pub profile: Option<i64>,
    /// The width of the last frame.
    #[field(label = "Width")]
    pub width: Option<&'a str>,
    /// The Es/N0 of the last frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The callsign of the last frame.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// Files received.
    #[field(label = "Files")]
    pub files: i64,
    /// Bytes held from an earlier session when the last transfer was announced.
    #[field(label = "Bytes held", unit = "bytes")]
    pub held_bytes: i64,
    /// Acknowledgement frames heard.
    #[field(label = "Acknowledgements heard")]
    pub acks_heard: i64,
    /// A frame header was read in the last 5 s.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the files heard. The row key is the session id and the start of the hash.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FileTransferFileRow<'a> {
    /// The file name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The size.
    #[field(label = "Size", unit = "bytes")]
    pub size: i64,
    /// The hash, hex.
    #[field(label = "Hash")]
    pub hash: &'a str,
    /// The state: receiving, verified, or the hash failed.
    #[field(label = "State")]
    pub state: &'a str,
    /// The sending station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// When the state was reached.
    #[field(label = "Time")]
    pub time: Time,
}

/// The fields of a file that passed every check and was saved.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FileTransferReceivedEvent<'a> {
    /// The file name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The size.
    #[field(label = "Size", unit = "bytes")]
    pub size: i64,
    /// The hash, hex.
    #[field(label = "Hash")]
    pub hash: &'a str,
    /// Where the file was saved; empty if it was not saved.
    #[field(label = "Path")]
    pub path: &'a str,
    /// The sending station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
}

/// The fields of an announced transfer.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FileTransferOpenEvent<'a> {
    /// The session id.
    #[field(label = "Session")]
    pub session: i64,
    /// The file name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The size.
    #[field(label = "Size", unit = "bytes")]
    pub size: i64,
    /// Bytes held from an earlier session.
    #[field(label = "Bytes held", unit = "bytes")]
    pub held_bytes: i64,
    /// The sending station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
}

/// The files heard.
pub static FILE_TRANSFER_FILES: Table<FileTransferFileRow<'static>> =
    Table::new("files", "Files", 200);
/// A file passed every check and was saved.
pub static FILE_TRANSFER_RECEIVED: EventKind<FileTransferReceivedEvent<'static>> =
    EventKind::new("file.received", "File received");
/// A transfer was announced.
pub static FILE_TRANSFER_OPEN: EventKind<FileTransferOpenEvent<'static>> =
    EventKind::new("file.open", "Transfer announced");
/// A file transmission started, finished or was refused.
pub static FILE_TRANSFER_TX: EventKind<TxEvent<'static>> =
    EventKind::new("file.tx", "File transmission");

// hf_chat

/// The result fields of `hf_chat`. A value of the last burst has no value before the first burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatFields<'a> {
    /// The profile of the last burst.
    #[field(label = "Profile")]
    pub profile: Option<i64>,
    /// The SNR of the last burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The clock error of the last burst against the slot grid.
    #[field(label = "Clock error", unit = "s")]
    pub clock_error_s: Option<f64>,
    /// The sending station of the last burst; empty if the burst names none.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// Bursts decoded.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Blocks whose code word and CRC held.
    #[field(label = "Blocks good")]
    pub blocks_good: i64,
    /// Blocks that did not decode.
    #[field(label = "Blocks bad")]
    pub blocks_bad: i64,
    /// Messages completed.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Beacons heard.
    #[field(label = "Beacons")]
    pub beacons: i64,
    /// Acknowledgements heard.
    #[field(label = "Acknowledgements heard")]
    pub acks_heard: i64,
    /// Voice blocks heard.
    #[field(label = "Voice blocks")]
    pub voice_blocks: i64,
    /// A burst was decoded in the last 30 s.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The note on the source of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The message id of the last burst; 0 if it carries no message part.
    #[field(label = "Message id")]
    pub message_id: Option<i64>,
}

/// A row of the messages received. The row key is a running number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatMessageRow<'a> {
    /// The sending station; empty for a data message.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The recipient; empty for a data message.
    #[field(label = "To")]
    pub to: &'a str,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// When the message was completed, UTC.
    #[field(label = "Time")]
    pub time: Time,
}

/// A row of the beacons received. The row key is a running number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatBeaconRow<'a> {
    /// The station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The locator.
    #[field(shared = keys::LOCATOR)]
    pub locator: &'a str,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The power.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: f64,
    /// The text of a full beacon; empty for a short one.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// When the beacon was completed, UTC.
    #[field(label = "Time")]
    pub time: Time,
}

/// The fields of a completed chat or data message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatMessageEvent<'a> {
    /// The sending station; empty for a data message.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The recipient; empty for a data message.
    #[field(label = "To")]
    pub to: &'a str,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The fields of a completed beacon.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatBeaconEvent<'a> {
    /// The station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The locator.
    #[field(shared = keys::LOCATOR)]
    pub locator: &'a str,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The power.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: f64,
    /// The text of a full beacon; empty for a short one.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The fields of an acknowledgement heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatAckEvent<'a> {
    /// The acknowledging station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The message id that it acknowledges.
    #[field(label = "Message id")]
    pub message_id: i64,
    /// Blocks acknowledged.
    #[field(label = "Blocks acknowledged")]
    pub blocks_acked: i64,
    /// The SNR that it reports.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The profile that it asks for.
    #[field(label = "Profile")]
    pub profile: i64,
}

/// The fields of a voice block heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfChatVoiceEvent<'a> {
    /// The calling station.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The call number.
    #[field(label = "Call")]
    pub call: i64,
    /// The profile of the burst.
    #[field(label = "Profile")]
    pub profile: i64,
}

/// The messages received.
pub static HF_CHAT_MESSAGES: Table<HfChatMessageRow<'static>> =
    Table::new("messages", "Messages", 200);
/// The beacons received.
pub static HF_CHAT_BEACONS: Table<HfChatBeaconRow<'static>> = Table::new("beacons", "Beacons", 200);
/// A chat or data message was completed.
pub static HF_CHAT_MESSAGE: EventKind<HfChatMessageEvent<'static>> =
    EventKind::new("chat.message", "Message received");
/// A beacon was completed.
pub static HF_CHAT_BEACON: EventKind<HfChatBeaconEvent<'static>> =
    EventKind::new("chat.beacon", "Beacon received");
/// An acknowledgement was heard.
pub static HF_CHAT_ACK: EventKind<HfChatAckEvent<'static>> =
    EventKind::new("chat.ack", "Acknowledgement heard");
/// A voice block was heard.
pub static HF_CHAT_VOICE: EventKind<HfChatVoiceEvent<'static>> =
    EventKind::new("chat.voice", "Voice heard");
/// A chat transmission started, finished or was refused.
pub static HF_CHAT_TX: EventKind<TxEvent<'static>> = EventKind::new("chat.tx", "Chat transmission");

/// The settings of `file_transfer`: the receiver settings, then the transmitter settings without the callsign that both
/// share.
pub fn file_transfer_settings() -> Vec<&'static dyn DeclareSetting> {
    use file_transfer::stack::{RxSetting, TxSetting, CALLSIGN};
    let mut all: Vec<&'static dyn DeclareSetting> = RxSetting::SETTINGS.to_vec();
    all.extend(
        TxSetting::SETTINGS
            .iter()
            .copied()
            .filter(|s| s.key() != CALLSIGN.key()),
    );
    all
}

/// The commands of every stack: `clear` of the receiver, `send` and `stop` of the sender.
fn commands(
    s: StackShape,
    send: &'static sdrkit_core::command::Command,
    stop: &'static sdrkit_core::command::Command,
) -> StackShape {
    s.command(&sdrkit_graph::decoder::CLEAR)
        .command(send)
        .command(stop)
}

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // What a stack heard from stations stays over a change of frequency: the station list, the logs of messages,
        // pictures and files, and the counts of these logs. The streams of `qam_video` belong to the signal on air.
        // 1.2: each `send` names `stop` as the command that ends its transmission.
        Family::new("custom", 1, 2)
            .stack(commands(
                StackShape::new("slotted_voice")
                    .fields::<SlottedVoiceFields>()
                    .fields::<DecoderFields>()
                    .table(&SLOTTED_VOICE_STATIONS)
                    .event(&SLOTTED_VOICE_STATION)
                    .event(&SLOTTED_VOICE_TX)
                    .settings(slotted_voice::stack::RxSetting::SETTINGS)
                    .settings(slotted_voice::stack::TxSetting::SETTINGS)
                    .settings(slotted_voice::stack::GenSetting::SETTINGS)
                    .kept(&["stations"]),
                &slotted_voice::stack::SEND,
                &slotted_voice::stack::STOP,
            ))
            .stack(commands(
                StackShape::new("qam_video")
                    .fields::<QamVideoFields>()
                    .fields::<DecoderFields>()
                    .table(&QAM_VIDEO_STREAMS)
                    .event(&QAM_VIDEO_TX)
                    .settings(qam_video::stack::RxSetting::SETTINGS)
                    .settings(qam_video::stack::TxSetting::SETTINGS),
                &qam_video::stack::SEND,
                &qam_video::stack::STOP,
            ))
            .stack(commands(
                StackShape::new("picture_burst")
                    .fields::<PictureBurstFields>()
                    .fields::<DecoderFields>()
                    .table(&PICTURE_BURST_PICTURES)
                    .event(&PICTURE_BURST_PICTURE)
                    .event(&PICTURE_BURST_TX)
                    .settings(picture_burst::stack::TxSetting::SETTINGS)
                    .kept(&["pictures", "PictureBurstFields.bursts_decoded"]),
                &picture_burst::stack::SEND,
                &picture_burst::stack::STOP,
            ))
            .stack(commands(
                StackShape::new("arq_packet")
                    .fields::<ArqPacketFields>()
                    .fields::<DecoderFields>()
                    .table(&ARQ_PACKET_MESSAGES)
                    .event(&ARQ_PACKET_MESSAGE)
                    .event(&ARQ_PACKET_ACK_OUT)
                    .event(&ARQ_PACKET_ACK_IN)
                    .event(&ARQ_PACKET_TX)
                    .settings(arq_packet::stack::TxSetting::SETTINGS)
                    .kept(&["messages", "ArqPacketFields.messages"]),
                &arq_packet::stack::SEND,
                &arq_packet::stack::STOP,
            ))
            .stack(commands(
                StackShape::new("file_transfer")
                    .fields::<FileTransferFields>()
                    .fields::<DecoderFields>()
                    .table(&FILE_TRANSFER_FILES)
                    .event(&FILE_TRANSFER_RECEIVED)
                    .event(&FILE_TRANSFER_OPEN)
                    .event(&FILE_TRANSFER_TX)
                    .settings(&file_transfer_settings())
                    .kept(&["files", "FileTransferFields.files"]),
                &file_transfer::stack::SEND,
                &file_transfer::stack::STOP,
            ))
            .stack(commands(
                StackShape::new("hf_chat")
                    .fields::<HfChatFields>()
                    .fields::<DecoderFields>()
                    .table(&HF_CHAT_MESSAGES)
                    .table(&HF_CHAT_BEACONS)
                    .event(&HF_CHAT_MESSAGE)
                    .event(&HF_CHAT_BEACON)
                    .event(&HF_CHAT_ACK)
                    .event(&HF_CHAT_VOICE)
                    .event(&HF_CHAT_TX)
                    .settings(hf_chat::stack::RxSetting::SETTINGS)
                    .settings(hf_chat::stack::TxSetting::SETTINGS)
                    .kept(&[
                        "messages",
                        "beacons",
                        "HfChatFields.messages",
                        "HfChatFields.beacons",
                    ]),
                &hf_chat::stack::SEND,
                &hf_chat::stack::STOP,
            ))
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
        .unwrap_or_else(|| panic!("the custom family does not describe the stack {id}"))
}
