//! The interface of the HF data family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::ale::calls::CallType;
use crate::ale::word::WordType;
use crate::ale::AleSetting;
use crate::ale3g::Ale3gSetting;
use crate::arqe::ArqeSetting;
use crate::autospec::AutospecSetting;
use crate::beacons::list::Pattern;
use crate::beacons::prop::BandState;
use crate::beacons::BeaconSetting;
use crate::buoy::rx::LinkFormat;
use crate::buoy::BuoySetting;
use crate::cis::CisSetting;
use crate::coquelet::CoqueletSetting;
use crate::crowd36::Crowd36Setting;
use crate::efr::EfrSetting;
use crate::fsk200::Fsk200Setting;
use crate::fsk_modems::FskModemsSetting;
use crate::gtor::GtorSetting;
use crate::gwfsk::GwfskSetting;
use crate::hfid::features::SignalClass;
use crate::hfid::HfidSetting;
use crate::link11::Link11Setting;
use crate::ms110::Ms110Setting;
use crate::pactor1::link::{FrameType, SeenAs};
use crate::pactor1::Pactor1Setting;
use crate::pactorhf::rx::Modulation as PactorModulation;
use crate::pactorhf::{Pactor2Setting, Pactor3Setting, Pactor4Setting};
use crate::piccolo::PiccoloSetting;
use crate::radar::radars::SweepDirection;
use crate::radar::RadarStackSetting;
use crate::rpr::rx::{RprModulation, RprState};
use crate::rpr::RprSetting;
use crate::s4285::S4285Setting;
use crate::s4481::S4481Setting;
use crate::s4539::S4539Setting;
use crate::util::ClockSource;
use crate::uvb76::Uvb76Setting;
use crate::vlf_msk::VlfMskSetting;
use crate::xpa::XpaSetting;
use sdrkit_core::command::Command;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, EventShape, Family as Described, Fields, Hertz, Seconds,
    StackShape, Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, SeriesDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

// The teleprinter stacks: EFR, G-TOR, ARQ-E, AUTOSPEC, Piccolo and Coquelet.

/// The result fields of a teleprinter stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeleprinterFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The search state: 0 searching, 1 signal found, 2 decoding.
    #[field(label = "State")]
    pub state: i64,
    /// The speed of the signal. Absent while the front has no lock.
    #[field(label = "Speed", unit = "Bd")]
    pub baud: Option<f64>,
    /// The centre of the signal above the dial. Absent while the front has no lock.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Option<f64>,
    /// The shift between the tones. Absent while the front has no lock or the signal has no shift.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<f64>,
    /// What was found beside the speed (cycle, depth, tone length). Empty while not decoding.
    #[field(label = "Detail")]
    pub detail: &'a str,
    /// Words, characters or frames read.
    #[field(label = "Good")]
    pub good: i64,
    /// Words, characters or frames that failed their check.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A lock of the words, characters or frames.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskLockEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The speed of the signal.
    #[field(label = "Speed", unit = "Bd")]
    pub baud: f64,
    /// The shift between the tones.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: f64,
    /// The centre of the signal above the dial.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: f64,
    /// What was found beside the speed.
    #[field(label = "Detail")]
    pub detail: &'a str,
}

/// The sync of the words, characters or frames was lost.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskLostEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Why sync was lost.
    #[field(label = "Reason")]
    pub reason: &'a str,
}

/// New text: a line, 64 characters, or what came before a pause.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TextEvent<'a> {
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A telegram of EFR. The row key is the number of the telegram.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EfrTelegramRow<'a> {
    /// The time a telegram ended.
    #[field(label = "Time")]
    pub time: Time,
    /// The checksum: "good" or "bad".
    #[field(label = "Checksum")]
    pub checksum: &'a str,
    /// The message number (user byte 1).
    #[field(label = "Message")]
    pub message: i64,
    /// Address 1 (user byte 2).
    #[field(label = "Address 1")]
    pub address1: i64,
    /// Address 2 (user byte 3).
    #[field(label = "Address 2")]
    pub address2: i64,
    /// The number of payload bytes.
    #[field(label = "Payload bytes")]
    pub payload_bytes: i64,
    /// The payload as hex.
    #[field(label = "Payload")]
    pub payload: &'a str,
}

/// A telegram of EFR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EfrTelegramEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The checksum: "good" or "bad".
    #[field(label = "Checksum")]
    pub checksum: &'a str,
    /// The message number (user byte 1).
    #[field(label = "Message")]
    pub message: i64,
    /// Address 1 (user byte 2).
    #[field(label = "Address 1")]
    pub address1: i64,
    /// Address 2 (user byte 3).
    #[field(label = "Address 2")]
    pub address2: i64,
    /// The number of payload bytes.
    #[field(label = "Payload bytes")]
    pub payload_bytes: i64,
    /// The payload as hex.
    #[field(label = "Payload")]
    pub payload: &'a str,
}

/// A frame of G-TOR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GtorFrameEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The speed of the frame.
    #[field(label = "Speed", unit = "Bd")]
    pub baud: f64,
    /// The command of a frame: data, changeover, disconnect, connect.
    #[field(label = "Command")]
    pub command: &'a str,
    /// The block number, 0 to 3.
    #[field(label = "Block")]
    pub block: i64,
    /// "plain" or "Golay".
    #[field(label = "Form")]
    pub form: &'a str,
    /// The bits were inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The compression: none, Huffman, swapped Huffman.
    #[field(label = "Compression")]
    pub compression: &'a str,
}

/// The tone pairs of Piccolo were found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PiccoloPairsEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The tone pairs of Coquelet were found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CoqueletPairsEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A lock of the words, characters or frames.
pub static FSK_LOCK: EventKind<FskLockEvent<'static>> = EventKind::new("fsk.lock", "Sync");
/// The sync was lost.
pub static FSK_LOST: EventKind<FskLostEvent<'static>> = EventKind::new("fsk.lost", "Sync lost");
/// New text.
pub static TEXT: EventKind<TextEvent<'static>> = EventKind::new("text", "Text");
/// The telegrams of EFR, newest first.
pub static EFR_TELEGRAMS: Table<EfrTelegramRow<'static>> =
    Table::new("telegrams", "Telegrams", 200);
/// A telegram of EFR.
pub static EFR_TELEGRAM: EventKind<EfrTelegramEvent<'static>> =
    EventKind::new("efr.telegram", "Telegram");
/// A frame of G-TOR.
pub static GTOR_FRAME: EventKind<GtorFrameEvent<'static>> = EventKind::new("gtor.frame", "Frame");
/// The tone pairs of Piccolo were found.
pub static PICCOLO_PAIRS: EventKind<PiccoloPairsEvent<'static>> =
    EventKind::new("piccolo.pairs", "Tone pairs");
/// The tone pairs of Coquelet were found.
pub static COQUELET_PAIRS: EventKind<CoqueletPairsEvent<'static>> =
    EventKind::new("coquelet.pairs", "Tone pairs");

// The serial-tone modems: MIL-STD-188-110, STANAG 4285 and STANAG 4539.

/// The result fields of a serial-tone modem.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SerialFields<'a> {
    /// The parts of the stack that no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The rate and interleaver being received.
    #[field(label = "Rate and interleaver")]
    pub waveform: &'a str,
    /// Messages ended by their end-of-message pattern.
    #[field(label = "Messages")]
    pub messages_total: i64,
    /// Interleaver blocks decoded.
    #[field(label = "Interleaver blocks")]
    pub blocks_total: i64,
    /// D_PDUs with a good header CRC.
    #[field(label = "D_PDUs")]
    pub dpdus_good: i64,
    /// The signal-to-noise ratio. Absent without a measure.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The frequency error. Absent without a measure.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A STANAG 5066 D_PDU found in the bits of a modem. The row key is the number of the D_PDU.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DpduRow<'a> {
    /// The UTC of the D_PDU.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The D_PDU type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The EOT byte.
    #[field(label = "EOT")]
    pub eot: i64,
    /// The destination address (hex).
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The source address (hex).
    #[field(label = "Source")]
    pub source: &'a str,
    /// Header bytes.
    #[field(label = "Header bytes")]
    pub header_bytes: i64,
    /// Payload bytes.
    #[field(label = "Payload bytes")]
    pub payload_bytes: i64,
    /// The data CRC: good, bad or none.
    #[field(label = "Data CRC")]
    pub data_crc: &'a str,
    /// The first 32 payload bytes (hex).
    #[field(label = "Payload")]
    pub payload: &'a str,
}

/// A status event of MIL-STD-188-110: sync, end of message, loss.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ms110Event<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A message of MIL-STD-188-110 with its text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ms110MessageEvent<'a> {
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The bytes of the message.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// A status event of STANAG 4285: sync, end of message, loss.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4285Event<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A message of STANAG 4285 with its text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4285MessageEvent<'a> {
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The bytes of the message.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// A status event of STANAG 4539: sync, end of message, loss.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4539Event<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A message of STANAG 4539 with its text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4539MessageEvent<'a> {
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The bytes of the message.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// The D_PDUs found in the bits of a modem, newest first.
pub static DPDUS: Table<DpduRow<'static>> = Table::new("dpdus", "D_PDUs", 500);
/// A status event of MIL-STD-188-110.
pub static MS110_EVENT: EventKind<Ms110Event<'static>> = EventKind::new("ms110.event", "Event");
/// A message of MIL-STD-188-110.
pub static MS110_MESSAGE: EventKind<Ms110MessageEvent<'static>> =
    EventKind::new("ms110.message", "Message");
/// A status event of STANAG 4285.
pub static S4285_EVENT: EventKind<S4285Event<'static>> = EventKind::new("s4285.event", "Event");
/// A message of STANAG 4285.
pub static S4285_MESSAGE: EventKind<S4285MessageEvent<'static>> =
    EventKind::new("s4285.message", "Message");
/// A status event of STANAG 4539.
pub static S4539_EVENT: EventKind<S4539Event<'static>> = EventKind::new("s4539.event", "Event");
/// A message of STANAG 4539.
pub static S4539_MESSAGE: EventKind<S4539MessageEvent<'static>> =
    EventKind::new("s4539.message", "Message");

// 3G-ALE.

/// The result fields of 3G-ALE.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ale3gFields<'a> {
    /// The parts that no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last lines.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Bursts read.
    #[field(label = "Bursts")]
    pub bursts_total: i64,
    /// PDUs with a good check.
    #[field(label = "PDUs good")]
    pub pdus_good: i64,
    /// The signal-to-noise ratio of the last burst. Absent without a measure.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The frequency error of the last burst. Absent without a measure.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A PDU of 3G-ALE. The row key is the number of the PDU.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ale3gPduRow<'a> {
    /// The UTC of the PDU.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The burst (BW0 to BW4).
    #[field(label = "Burst")]
    pub burst: &'a str,
    /// The PDU type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The check of the PDU passed.
    #[field(label = "Check good")]
    pub crc_good: bool,
    /// The PDU in words.
    #[field(label = "Content")]
    pub words: &'a str,
    /// The signal-to-noise ratio of the burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frequency error of the burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The data bytes of a packet or datagram.
    #[field(label = "Data")]
    pub data: &'a str,
}

/// A PDU of 3G-ALE.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ale3gPduEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The burst (BW0 to BW4).
    #[field(label = "Burst")]
    pub burst: &'a str,
    /// The PDU type.
    #[field(label = "Type")]
    pub r#type: &'a str,
}

/// A complete datagram of 3G-ALE.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Ale3gDatagramEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The PDUs of 3G-ALE, newest first.
pub static ALE3G_PDUS: Table<Ale3gPduRow<'static>> = Table::new("pdus", "PDUs", 1000);
/// A PDU of 3G-ALE.
pub static ALE3G_PDU: EventKind<Ale3gPduEvent<'static>> = EventKind::new("ale3g.pdu", "PDU");
/// A complete datagram of 3G-ALE.
pub static ALE3G_DATAGRAM: EventKind<Ale3gDatagramEvent<'static>> =
    EventKind::new("ale3g.datagram", "Datagram");

// STANAG 4481.

/// The result fields of STANAG 4481.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4481Fields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What is received: one channel, sub-carriers, or nothing.
    #[field(label = "Reception")]
    pub reception: &'a str,
}

/// A channel of STANAG 4481: the single channel or a sub-carrier. The row key is the number of the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4481ChannelRow<'a> {
    /// The number of the channel (0: the single channel).
    #[field(label = "Channel")]
    pub channel: i64,
    /// The mark tone.
    #[field(label = "Mark", unit = "Hz")]
    pub mark_hz: f64,
    /// The space tone.
    #[field(label = "Space", unit = "Hz")]
    pub space_hz: f64,
    /// The channel carries a signal.
    #[field(label = "On")]
    pub on: bool,
    /// The speed of the channel. Absent until it is found.
    #[field(label = "Speed", unit = "Bd")]
    pub speed_bd: Option<f64>,
    /// "start-stop" or "sync".
    #[field(label = "Framing")]
    pub framing: &'a str,
    /// Characters read.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters with a framing error.
    #[field(label = "Framing errors")]
    pub framing_errors: i64,
    /// Bits received.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The signal-to-noise ratio of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The last text of the channel.
    #[field(label = "Text")]
    pub last_text: &'a str,
}

/// An event of STANAG 4481.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct S4481Event<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The channels of STANAG 4481.
pub static S4481_CHANNELS: Table<S4481ChannelRow<'static>> = Table::new("channels", "Channels", 17);
/// An event of STANAG 4481.
pub static S4481_EVENT: EventKind<S4481Event<'static>> = EventKind::new("s4481.event", "Event");

// Link 11.

/// The result fields of Link 11.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Link11Fields<'a> {
    /// The parts that no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last lines.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Messages since the last Clear.
    #[field(label = "Messages")]
    pub messages_total: i64,
    /// The Doppler offset of the last message. Absent without a measure.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A message of Link 11. The row key is the number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Link11MessageRow<'a> {
    /// The start of the message, seconds.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// The frame rate: "fast" or "slow".
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// Preamble frames.
    #[field(label = "Preamble frames")]
    pub preamble_frames: i64,
    /// Data frames.
    #[field(label = "Data frames")]
    pub data_frames: i64,
    /// The duration, seconds.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: f64,
    /// The Doppler offset.
    #[field(label = "Doppler offset", unit = "Hz")]
    pub doppler_hz: f64,
    /// The constellation offset, degrees.
    #[field(label = "Constellation offset", unit = "deg")]
    pub offset_deg: f64,
    /// The signal-to-noise ratio of the message.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// An event of Link 11.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Link11Event<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A message of Link 11.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Link11MessageEvent<'a> {
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Data frames.
    #[field(label = "Data frames")]
    pub data_frames: i64,
    /// The quarter turns of every data frame (15 digits a frame).
    #[field(label = "Phase steps")]
    pub steps: &'a str,
}

/// The messages of Link 11, newest first.
pub static LINK11_MESSAGES: Table<Link11MessageRow<'static>> =
    Table::new("messages", "Messages", 500);
/// An event of Link 11.
pub static LINK11_EVENT: EventKind<Link11Event<'static>> = EventKind::new("link11.event", "Event");
/// A message of Link 11.
pub static LINK11_MESSAGE: EventKind<Link11MessageEvent<'static>> =
    EventKind::new("link11.message", "Message");

// The CIS and HF data identification stack.

/// The result fields of the CIS stack. A field that the submode in use does not measure has no value.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CisHfFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text received, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The search state: 0 searching, 1 signal found, 2 decoding (or identified).
    #[field(label = "State")]
    pub state: i64,
    /// What was found, in one line.
    #[field(label = "Found")]
    pub found: &'a str,
    /// The name of the signal identified, or empty.
    #[field(label = "Identified")]
    pub identified: &'a str,
    /// Characters, words or frames read.
    #[field(label = "Good")]
    pub good: i64,
    /// Characters, words or frames that failed their check.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The frequency error. Absent without a measure.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The signal-to-noise ratio. Absent without a measure.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The centre of the signal above the dial (2-FSK and CIS-3000).
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Option<f64>,
    /// The shift between the tones (2-FSK).
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<f64>,
    /// The eye opening of the 2-FSK demodulators.
    #[field(label = "Eye")]
    pub eye: Option<f64>,
    /// Frames read (CIS-14).
    #[field(label = "Frames")]
    pub frames: Option<i64>,
    /// The state bits of the last frames (CIS-14), raw.
    #[field(label = "State bits")]
    pub state_bits: Option<&'a str>,
    /// The pilot (CIS-12).
    #[field(label = "Pilot", unit = "Hz")]
    pub pilot_hz: Option<f64>,
    /// Carriers seen (CIS-12, OFDM).
    #[field(label = "Carriers seen")]
    pub carriers: Option<i64>,
    /// The symbol rate.
    #[field(label = "Symbol rate", unit = "Bd")]
    pub symbol_rate: Option<f64>,
    /// The modulation found.
    #[field(label = "Modulation")]
    pub modulation: Option<&'a str>,
    /// The moment of the 2nd power.
    #[field(label = "M2")]
    pub m2: Option<f64>,
    /// The moment of the 4th power.
    #[field(label = "M4")]
    pub m4: Option<f64>,
    /// The moment of the 8th power.
    #[field(label = "M8")]
    pub m8: Option<f64>,
    /// Tones seen (CIS-36).
    #[field(label = "Tones seen")]
    pub tones: Option<i64>,
    /// The spacing of the tones or carriers.
    #[field(label = "Spacing", unit = "Hz")]
    pub spacing_hz: Option<f64>,
    /// The lowest tone or band edge.
    #[field(label = "Lowest", unit = "Hz")]
    pub low_hz: Option<f64>,
    /// The highest tone or band edge.
    #[field(label = "Highest", unit = "Hz")]
    pub high_hz: Option<f64>,
    /// The span of the tones (CIS-36).
    #[field(label = "Span", unit = "Hz")]
    pub span_hz: Option<f64>,
    /// Double gaps between tones (CIS-36).
    #[field(label = "Double gaps")]
    pub double_gaps: Option<i64>,
    /// The useful part of an OFDM symbol.
    #[field(label = "Useful part", unit = "ms")]
    pub useful_ms: Option<f64>,
    /// The guard of an OFDM symbol.
    #[field(label = "Guard", unit = "ms")]
    pub guard_ms: Option<f64>,
    /// The centre carrier of the OFDM signal is off.
    #[field(label = "Centre carrier off")]
    pub centre_off: Option<bool>,
    /// A pilot at 3300 Hz is there.
    #[field(label = "Pilot 3300 Hz")]
    pub pilot_3300: Option<bool>,
    /// The spectral line of the 2nd power.
    #[field(label = "Line of the 2nd power", unit = "dB")]
    pub line2_db: Option<f64>,
    /// The spectral line of the 4th power.
    #[field(label = "Line of the 4th power", unit = "dB")]
    pub line4_db: Option<f64>,
    /// The spectral line of the 8th power.
    #[field(label = "Line of the 8th power", unit = "dB")]
    pub line8_db: Option<f64>,
    /// The spectral line at the symbol rate.
    #[field(label = "Symbol-rate line", unit = "dB")]
    pub rate_line_db: Option<f64>,
}

/// A `bee` message of CIS-36-50. The row key is the number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CisHfMessageRow<'a> {
    /// The time the message ended.
    #[field(label = "Time")]
    pub time: Time,
    /// The speed of the message.
    #[field(label = "Speed", unit = "Bd")]
    pub speed: f64,
    /// The start word.
    #[field(label = "Start word")]
    pub start_word: &'a str,
    /// The session key.
    #[field(label = "Session key")]
    pub session_key: &'a str,
    /// Characters read.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that failed the 3:4 check.
    #[field(label = "Failing 3:4")]
    pub failing: i64,
    /// What ended the message.
    #[field(label = "Ended by")]
    pub ended_by: &'a str,
}

/// A carrier of CIS-12. The row key is the number of the carrier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CisHfCarrierRow {
    /// The carrier number (1 to 12).
    #[field(label = "Carrier")]
    pub number: i64,
    /// The carrier frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: f64,
    /// The carrier is on.
    #[field(label = "On")]
    pub on: bool,
}

/// A `bee` message of CIS-36-50.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CisMessageEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The start word.
    #[field(label = "Start word")]
    pub start_word: &'a str,
    /// The session key.
    #[field(label = "Session key")]
    pub session_key: &'a str,
    /// Characters read.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that failed the 3:4 check.
    #[field(label = "Failing 3:4")]
    pub failing: i64,
}

/// A signal was identified.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CisIdentEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The name of the signal identified.
    #[field(label = "Identified")]
    pub identified: &'a str,
}

/// The `bee` messages, newest first.
pub static CIS_HF_MESSAGES: Table<CisHfMessageRow<'static>> =
    Table::new("messages", "Messages", 200);
/// The carriers of CIS-12.
pub static CIS_HF_CARRIERS: Table<CisHfCarrierRow> = Table::new("carriers", "Carriers", 12);
/// A `bee` message.
pub static CIS_MESSAGE: EventKind<CisMessageEvent<'static>> =
    EventKind::new("cis.message", "Message");
/// A signal was identified.
pub static CIS_IDENT: EventKind<CisIdentEvent<'static>> = EventKind::new("cis.ident", "Identified");

// The HF signal identifier.

/// The result fields of the HF signal identifier: the measures of the last analysis.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfidFields<'a> {
    /// A signal was found by the last analysis.
    #[field(label = "Signal")]
    pub signal: bool,
    /// The best candidate.
    #[field(label = "Best estimate")]
    pub best: Option<SignalClass>,
    /// The confidence of the best candidate, 0 to 95 %.
    #[field(label = "Confidence (estimate)", unit = "%")]
    pub best_confidence_pct: Option<f64>,
    /// The analyses run.
    #[field(label = "Analyses")]
    pub analyses: i64,
    /// The lower edge of the occupied band.
    #[field(label = "Band from", unit = "Hz")]
    pub band_low: Option<Hertz>,
    /// The upper edge of the occupied band.
    #[field(label = "Band to", unit = "Hz")]
    pub band_high: Option<Hertz>,
    /// The signal-to-noise ratio in the band. Absent without a signal.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The noise floor.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// The steady carrier line.
    #[field(label = "Carrier line", unit = "Hz")]
    pub carrier_line: Option<Hertz>,
    /// The carrier line over its surroundings.
    #[field(label = "Carrier line level", unit = "dB")]
    pub carrier_line_db: Option<f64>,
    /// The 605 Hz tone over its surroundings.
    #[field(label = "Tone at 605 Hz", unit = "dB")]
    pub tone_605_db: Option<f64>,
    /// A tone at 2915 Hz.
    #[field(label = "Tone at 2915 Hz")]
    pub tone_2915: bool,
    /// The tone comb spacing.
    #[field(label = "Tone spacing", unit = "Hz")]
    pub comb_spacing: Option<Hertz>,
    /// The regularity of the comb, 0 to 100 %.
    #[field(label = "Comb regularity", unit = "%")]
    pub comb_regularity_pct: Option<f64>,
    /// The tones of the comb.
    #[field(label = "Tones")]
    pub comb_tones: Option<i64>,
    /// The two-tone shift.
    #[field(label = "Shift", unit = "Hz")]
    pub shift: Option<Hertz>,
    /// The symbol rate at the clock line.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The clock line over its surroundings.
    #[field(label = "Clock line", unit = "dB")]
    pub clock_line_db: Option<f64>,
    /// The slow envelope modulation.
    #[field(label = "Envelope modulation", unit = "Hz")]
    pub envelope_modulation: Option<Hertz>,
    /// The envelope modulation line over its surroundings.
    #[field(label = "Envelope modulation level", unit = "dB")]
    pub envelope_modulation_db: Option<f64>,
    /// The bursts in the analysed audio (0: continuous).
    #[field(label = "Bursts")]
    pub bursts: Option<i64>,
    /// The part of the time with a burst on.
    #[field(label = "On", unit = "%")]
    pub duty_pct: Option<f64>,
    /// The constellation order at the clock: 2, 4, 8, or 99 for higher or unclear.
    #[field(label = "Constellation order")]
    pub order: Option<i64>,
    /// The repetition period in symbols.
    #[field(label = "Repetition (symbols)")]
    pub repeat_symbols: Option<i64>,
    /// The repetition period.
    #[field(label = "Repetition", unit = "s")]
    pub repeat_period: Option<Seconds>,
    /// The correlation at the repetition period, 0 to 100 %.
    #[field(label = "Repetition correlation", unit = "%")]
    pub repeat_correlation_pct: Option<f64>,
    /// The measured parameters as lines of text.
    #[field(label = "Measured parameters")]
    pub measured: &'a str,
    /// The level of the audio, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The audio rate of the analysis.
    #[field(label = "Audio rate", unit = "Hz")]
    pub audio_rate: Hertz,
}

/// A candidate of the last analysis of the identifier. The row key is the place, "0" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfidCandidateRow<'a> {
    /// The waveform.
    #[field(label = "SignalClass")]
    pub waveform: SignalClass,
    /// The confidence of the estimate, 0 to 95 %.
    #[field(label = "Confidence (estimate)", unit = "%")]
    pub confidence_pct: f64,
    /// What the score is based on.
    #[field(label = "Measured basis")]
    pub basis: &'a str,
    /// What the waveform is.
    #[field(label = "Note")]
    pub note: &'a str,
    /// A public source.
    #[field(label = "Source")]
    pub link: &'a str,
}

/// The candidates of the last analysis, best first.
pub static HFID_CANDIDATES: Table<HfidCandidateRow<'static>> =
    Table::new("candidates", "Candidates", 6);

// ALE.

/// The result fields of ALE.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleFields {
    /// In word sync (the lock state "Sync").
    #[field(label = "Word sync")]
    pub in_sync: bool,
    /// A word was decoded in the last 60 s.
    #[field(label = "Words")]
    pub words_recent: bool,
    /// Words decoded.
    #[field(label = "Words decoded")]
    pub words_ok: i64,
    /// Words missed.
    #[field(label = "Words missed")]
    pub words_missed: i64,
    /// Calls in the event log.
    #[field(label = "Calls")]
    pub calls: i64,
    /// Stations heard.
    #[field(label = "Stations")]
    pub stations: i64,
    /// The frequency correction.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The signal-to-noise ratio of the last call. Absent before a call.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The bit error rate of the last call. Absent before a call.
    #[field(shared = keys::BER)]
    pub ber: Option<f64>,
    /// The level of the audio, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
}

/// A call in the ALE event log. The row key is the sequence number of the call.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleEventRow<'a> {
    /// The signal time of the first word.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC of the first word, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// The dial it was heard on, when known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
    /// The caller, when known.
    #[field(label = "From", refers_to = "stations")]
    pub from: Option<&'a str>,
    /// The TO addresses then the THRU members.
    #[field(label = "To")]
    pub to: &'a str,
    /// The type of the call.
    #[field(label = "Type")]
    pub r#type: CallType,
    /// TIS (else TWS).
    #[field(label = "TIS")]
    pub tis: bool,
    /// The AMD text.
    #[field(label = "AMD")]
    pub amd: &'a str,
    /// The commands.
    #[field(label = "Commands")]
    pub commands: &'a str,
    /// The bit error rate of the call.
    #[field(shared = keys::BER)]
    pub ber: f64,
    /// The mean word SNR in 3 kHz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frequency correction at the end of the call.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Words in the transmission.
    #[field(label = "Words")]
    pub words: i64,
}

/// A caller of ALE. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleStationRow<'a> {
    /// The address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The dial of the last call, when known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
    /// The SNR of the last call.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The bit error rate of the last call.
    #[field(shared = keys::BER)]
    pub ber: f64,
    /// The type of the last call.
    #[field(label = "Last")]
    pub last_type: CallType,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// When last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A decoded ALE word. The row key is the sequence number of the word.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleWordRow<'a> {
    /// The signal time of its first symbol.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// The word type.
    #[field(label = "Type")]
    pub r#type: WordType,
    /// The three characters of the word (non-printing ones as '.').
    #[field(label = "Text")]
    pub text: &'a str,
    /// Vote errors.
    #[field(label = "Vote")]
    pub votes: i64,
    /// Golay corrections.
    #[field(label = "Golay")]
    pub golay: i64,
}

/// A net address learned from the answers. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleNetRow<'a> {
    /// The address.
    #[field(label = "Address")]
    pub address: &'a str,
}

/// An ALE call: `source` is the caller, `target` the called addresses and `text` the call for people.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleCallEvent<'a> {
    /// The type of the call.
    #[field(label = "Type")]
    pub r#type: CallType,
    /// The caller.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The called addresses.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The call for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// TIS (else TWS).
    #[field(label = "TIS")]
    pub tis: bool,
    /// The AMD text.
    #[field(label = "AMD")]
    pub amd: &'a str,
    /// The commands.
    #[field(label = "Commands")]
    pub commands: &'a str,
    /// The bit error rate of the call.
    #[field(shared = keys::BER)]
    pub ber: f64,
    /// The mean word SNR in 3 kHz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frequency correction at the end of the call.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Words in the transmission.
    #[field(label = "Words")]
    pub words: i64,
    /// What the event is about: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The arguments of `remove` of an ALE station.
#[derive(Arguments)]
pub struct AleStationArgs<'a> {
    /// The row key of the station.
    #[arg(shared = ALE_STATION_ROW)]
    pub row: &'a str,
}

/// The calls of ALE, oldest first.
pub static ALE_EVENTS: Table<AleEventRow<'static>> = Table::new("events", "Events", 500);
/// The callers of ALE, the one heard last first.
pub static ALE_STATIONS: Table<AleStationRow<'static>> = Table::new("stations", "Stations", 500);
/// The decoded ALE words.
pub static ALE_WORDS: Table<AleWordRow<'static>> = Table::new("words", "Words", 400);
/// The net addresses learned from the answers.
pub static ALE_LEARNED_NETS: Table<AleNetRow<'static>> =
    Table::new("learned_nets", "Learned nets", 64);
/// An ALE call.
pub static ALE_CALL: EventKind<AleCallEvent<'static>> = EventKind::new("ale.call", "ALE call");
/// The row key of an ALE station.
pub const ALE_STATION_ROW: Argument<field::Text> = objcmd::row_of("stations");
/// Forgets one ALE station.
pub const ALE_REMOVE: CommandKind<AleStationArgs<'static>> = objcmd::remove_kind();

sdrkit_core::choice_enum! {
    /// What the ALE transmitter sends.
    pub enum AleTxKind {
        /// An individual call (or the allcall).
        Call = ("call", "Call"),
        /// A sounding.
        Sounding = ("sounding", "Sounding"),
        /// The answer to a call: the response of the three-way handshake.
        Response = ("response", "Response"),
        /// The acknowledgement of a response.
        Ack = ("ack", "Acknowledgement"),
        /// An AMD text: in a call, or to the linked station.
        Amd = ("amd", "AMD message"),
    }
}

sdrkit_core::choice_enum! {
    /// The state of the three-way handshake (call, response, acknowledgement).
    pub enum AleLinkState {
        /// No handshake.
        Idle = ("idle", "Idle"),
        /// The stack sends a call or waits for its response.
        Calling = ("calling", "Calling"),
        /// The stack answered a call and waits for the acknowledgement.
        Responding = ("responding", "Responding"),
        /// The handshake is complete.
        Linked = ("linked", "Linked"),
        /// The response or the acknowledgement did not come in time.
        TimedOut = ("timed_out", "Timed out"),
        /// A call to the stack was not answered, or a response not acknowledged; the reason says why.
        NotAnswered = ("not_answered", "Not answered"),
    }
}

/// The result fields of the ALE transmitter. The values of a transmission have no value while nothing is sent.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleTxFields<'a> {
    /// Whether a call or a sounding is being sent.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// What is being sent.
    #[field(label = "Transmission")]
    pub tx_kind: Option<AleTxKind>,
    /// The called address of a call.
    #[field(label = "Called address")]
    pub tx_to: Option<&'a str>,
    /// The word being sent, from 1.
    #[field(label = "Word sent")]
    pub tx_word: Option<i64>,
    /// The words of the transmission.
    #[field(label = "Words to send")]
    pub tx_words: Option<i64>,
    /// What the transmitter does not do.
    #[field(label = "Transmit note")]
    pub tx_note: &'a str,
    /// The state of the three-way handshake.
    #[field(label = "Link")]
    pub tx_link: AleLinkState,
    /// The other station of the handshake. Absent while idle.
    #[field(label = "Link station")]
    pub tx_link_station: Option<&'a str>,
    /// Why the link state is what it is; empty when it needs no reason.
    #[field(label = "Link reason")]
    pub tx_link_reason: &'a str,
}

/// The fields of a change of the ALE transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleTxEvent<'a> {
    /// The state: started, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// What is sent.
    #[field(label = "Transmission")]
    pub kind: AleTxKind,
    /// The called address of a call; empty for a sounding.
    #[field(label = "Called address")]
    pub to: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An ALE transmission.
pub static ALE_TX: EventKind<AleTxEvent<'static>> = EventKind::new("ale.tx", "ALE transmission");

/// The fields of a step of the three-way handshake.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleLinkEvent<'a> {
    /// The link state after the step.
    #[field(label = "Link")]
    pub state: AleLinkState,
    /// The other station.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The step: the call sent, the call heard, the response heard, the acknowledgement sent or heard, or why there
    /// was no answer.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A step of the ALE three-way handshake.
pub static ALE_LINK: EventKind<AleLinkEvent<'static>> = EventKind::new("ale.link", "ALE link");

/// The link quality of a station on a channel. The row key is the address and the dial.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleLqaRow<'a> {
    /// The address.
    #[field(label = "Address", refers_to = "stations")]
    pub address: &'a str,
    /// The dial, when known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
    /// The mean vote errors a word of its last call, 0 to 30.
    #[field(label = "LQA BER")]
    pub lqa_ber: i64,
    /// The SINAD of its last call, 0 to 30 dB.
    #[field(label = "SINAD", unit = "dB")]
    pub sinad_db: i64,
    /// The multipath figure of its last call (not measured: 0).
    #[field(label = "Multipath")]
    pub multipath: i64,
    /// The BER that the station reported of this station's signal, in its last call to the own address with an LQA
    /// report.
    #[field(label = "Reported BER")]
    pub reported_ber: Option<i64>,
    /// The SINAD that the station reported of this station's signal, dB.
    #[field(label = "Reported SINAD", unit = "dB")]
    pub reported_sinad_db: Option<i64>,
    /// The multipath that the station reported.
    #[field(label = "Reported multipath")]
    pub reported_multipath: Option<i64>,
    /// Calls heard from it on this channel.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// When last heard on this channel.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// An AMD text heard. The row key is the sequence number of the text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AleMessageRow<'a> {
    /// The signal time of the call's first word.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC of the call's first word, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// The dial it was heard on, when known.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Option<Hertz>,
    /// The caller, when known.
    #[field(label = "From", refers_to = "stations")]
    pub from: Option<&'a str>,
    /// The called addresses.
    #[field(label = "To")]
    pub to: &'a str,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The link quality of the stations heard, per station and channel.
pub static ALE_LQA: Table<AleLqaRow<'static>> = Table::new("lqa", "Link quality", 500);
/// The AMD texts heard, oldest first.
pub static ALE_MESSAGES: Table<AleMessageRow<'static>> = Table::new("messages", "Messages", 200);

/// The arguments of the ALE `amd`.
#[derive(Arguments)]
pub struct AleAmdArgs<'a> {
    /// The called address.
    #[arg(
        label = "Called address",
        help = "1 to 15 characters of A-Z and 0-9. Empty sends to the linked station."
    )]
    pub to: &'a str,
    /// The text.
    #[arg(
        label = "Text",
        help = "1 to 90 characters of ASCII-64 (space to underscore); lower case is sent as upper case."
    )]
    pub text: &'a str,
}

/// Sends an AMD text.
pub const ALE_AMD: CommandKind<AleAmdArgs<'static>> = CommandKind::new("amd", "Send AMD text")
    .help(
        "Sends an AMD text. To the linked station: the called address twice, the text, the own address in TWS words.          To another station: a call with the text before the own address, which starts the three-way handshake.          Transmit must be on and the own address set.",
    )
    .transmits(sdrkit_core::TxContent::Text)
    .stopped_by("stop");

/// The arguments of the ALE `call`.
#[derive(Arguments)]
pub struct AleCallArgs<'a> {
    /// The called address.
    #[arg(
        label = "Called address",
        help = "1 to 15 characters of A-Z and 0-9, or @?@ for the allcall. Empty calls @?@."
    )]
    pub to: &'a str,
}

/// Sends one individual call.
pub const ALE_CALL_TX: CommandKind<AleCallArgs<'static>> = CommandKind::new("call", "Call")
    .help(
        "Sends one individual call on this channel: the called address in TO words the calling cycle repeats times, then \
         the own address in TIS words. Transmit must be on and the own address set.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("stop");
/// Sends one sounding.
pub const ALE_SOUND: Command = Command::new("sound", "Sound")
    .help(
        "Sends one sounding on this channel: the own address in TWS or TIS words, the calling cycle repeats times. \
         Transmit must be on and the own address set.",
    )
    .transmits(sdrkit_core::TxContent::Beacon)
    .stopped_by("stop");
/// Ends the ALE transmission.
pub const ALE_STOP: Command =
    Command::new("stop", "Stop sending").help("Ends the call or the sounding at once.");

// HF ocean radars.

/// The result fields of the HF ocean radar stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfradarFields<'a> {
    /// The radar band the radio is in, if it is in one.
    #[field(label = "Band now")]
    pub band: Option<&'a str>,
    /// Radars shown.
    #[field(label = "Radars")]
    pub radar_count: i64,
    /// Sweeps counted.
    #[field(label = "Sweeps counted")]
    pub sweeps_counted: i64,
    /// Tracks rejected.
    #[field(label = "Tracks rejected")]
    pub tracks_rejected: i64,
    /// Broadband bursts.
    #[field(label = "Broadband bursts")]
    pub broadband_bursts: i64,
    /// The best SNR of the radars heard. Absent without a radar.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The stops of the scan, while it steps.
    #[field(label = "Scan stops")]
    pub scan_stops: Option<i64>,
    /// The time of one waterfall row.
    #[field(label = "Waterfall row", unit = "s")]
    pub waterfall_row: Seconds,
    /// The signal time of the newest waterfall row.
    #[field(label = "Newest waterfall row", unit = "s")]
    pub waterfall_newest: Seconds,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
}

/// A radar. The row key is the radar number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfradarRadarRow<'a> {
    /// The radar number.
    #[field(label = "#")]
    pub number: i64,
    /// The centre of the sweep.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Hertz,
    /// The centre against the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The radar band, if the radar is in one.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The width of the sweep.
    #[field(label = "Width", unit = "Hz")]
    pub width: Hertz,
    /// The sweep rate.
    #[field(label = "Rate", unit = "Hz/s")]
    pub rate_hz_s: f64,
    /// The direction of the sweep.
    #[field(label = "Dir")]
    pub direction: SweepDirection,
    /// The repetition period of the sweeps, when measured.
    #[field(label = "Repeat", unit = "s")]
    pub repeat: Option<Seconds>,
    /// The signal-to-noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Sweeps counted.
    #[field(label = "Sweeps")]
    pub sweeps: i64,
    /// The signal time when first heard.
    #[field(label = "First heard (signal time)", unit = "s")]
    pub first: Seconds,
    /// The signal time when last heard.
    #[field(label = "Last heard (signal time)", unit = "s")]
    pub last: Seconds,
    /// The wall time when first heard, when the clock is known.
    #[field(label = "First heard")]
    pub first_heard: Option<Time>,
    /// The wall time when last heard, in whole seconds, when the clock is known.
    #[field(label = "Last heard")]
    pub last_heard: Option<Time>,
    /// The keyed ID, when read.
    #[field(label = "ID (unverified)")]
    pub id: Option<&'a str>,
    /// The speed of the keyed ID.
    #[field(label = "ID speed", unit = "wpm")]
    pub id_wpm: Option<f64>,
    /// Times the ID was seen.
    #[field(label = "ID seen")]
    pub id_seen: i64,
    /// What the radar may be.
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// A radar heard. The row key is the sequence number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfradarHeardRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The wall time, when the clock is known.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The radar number.
    #[field(label = "#", refers_to = "radars")]
    pub number: i64,
    /// The centre of the sweep.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The radar band, if the radar is in one.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The signal-to-noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A radar heard first, or again after a minute's pause. `source` is the keyed ID when it was read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfradarRadarHeardEvent<'a> {
    /// The radar number.
    #[field(label = "Radar")]
    pub radar: i64,
    /// The radar band, if the radar is in one.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The width of the sweep.
    #[field(label = "Width", unit = "Hz")]
    pub width: Hertz,
    /// The direction of the sweep.
    #[field(label = "Dir")]
    pub direction: SweepDirection,
    /// The keyed ID, when read.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The radars.
pub static HFRADAR_RADARS: Table<HfradarRadarRow<'static>> = Table::new("radars", "Radars", 64);
/// The radars heard, oldest first.
pub static HFRADAR_HEARD: Table<HfradarHeardRow<'static>> = Table::new("heard", "Heard", 300);
/// A radar heard.
pub static HFRADAR_RADAR_HEARD: EventKind<HfradarRadarHeardEvent<'static>> =
    EventKind::new("hfradar.radar_heard", "Radar heard");

/// An oceanographic radar band. The row key is the band name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfradarBandRow<'a> {
    /// The band name ("12 - 14 MHz").
    #[field(label = "Band")]
    pub name: &'a str,
    /// The lower edge of the working range.
    #[field(label = "Low", unit = "Hz")]
    pub low: Hertz,
    /// The upper edge of the working range.
    #[field(label = "High", unit = "Hz")]
    pub high: Hertz,
    /// The middle of the band: where `band.tune` tunes the radio.
    #[field(label = "Middle", unit = "Hz")]
    pub middle: Hertz,
    /// The range class of the radars of the band.
    #[field(label = "Range class")]
    pub range_class: &'a str,
}

/// The oceanographic radar bands, lowest first.
pub static HFRADAR_BANDS: Table<HfradarBandRow<'static>> = Table::new("bands", "Radar bands", 8);

/// The arguments of `band.tune` of the `hfradar` stack.
#[derive(Arguments)]
pub struct HfradarBandArgs<'a> {
    /// The row key of the band ("12 - 14 MHz").
    #[arg(shared = HFRADAR_BAND_ROW)]
    pub row: &'a str,
}

/// The row key of a band of the HF ocean radar stack.
pub const HFRADAR_BAND_ROW: Argument<field::Text> = objcmd::row_of("bands");
/// Asks the host to tune to the middle of a radar band.
pub const HFRADAR_BAND_TUNE: CommandKind<HfradarBandArgs<'static>> =
    CommandKind::new("band.tune", "Tune to a band").help(
        "Asks the host to tune to the middle of an oceanographic radar band. The radio takes in one part of the band at a \
         time (1.8 MHz at 2 Msps, so a whole band at once from about 12 Msps for 12 - 14 MHz): a wide radio sees all radars of \
         a band together, a narrow one needs the scan. Refused while the scan is on.",
    );

// PACTOR-II, -III and -IV.

/// The result fields of the PACTOR-II, -III and -IV stacks: the measures of the last analysis.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PactorhfFields<'a> {
    /// A signal was found by the last analysis.
    #[field(label = "Signal")]
    pub signal: bool,
    /// The variant found: 2, 3 or 4 (0 for nothing).
    #[field(label = "Variant")]
    pub variant: i64,
    /// The signal on the channel is not the variant that the stack receives.
    #[field(label = "Another variant")]
    pub other_variant: bool,
    /// The symbol rate. Absent without a signal.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The clock error. Absent without a signal.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The middle of the outer carriers.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Option<Hertz>,
    /// The centre against its nominal place.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The carrier spacing (0 for one carrier).
    #[field(label = "Spacing", unit = "Hz")]
    pub spacing: Option<Hertz>,
    /// The occupied bandwidth.
    #[field(label = "Occupies about", unit = "Hz")]
    pub bandwidth: Option<Hertz>,
    /// The clock line of the symbol clock.
    #[field(label = "Clock line", unit = "dB")]
    pub clock_line_db: Option<f64>,
    /// The number of carriers.
    #[field(label = "Carriers")]
    pub carrier_count: i64,
    /// The PACTOR-III speed level (0 when unknown or another variant).
    #[field(label = "Speed level")]
    pub speed_level: i64,
    /// The PACTOR-IV constellation estimate: 2, 4, 8, 16 (QAM), 0 unknown.
    #[field(label = "Constellation")]
    pub constellation: i64,
    /// The number of symbols demodulated on each carrier.
    #[field(label = "Symbols")]
    pub symbol_count: i64,
    /// The start of the demodulated stretch, into the analysed audio.
    #[field(label = "Stretch start", unit = "s")]
    pub stretch_start: Option<Seconds>,
    /// The length of the demodulated stretch.
    #[field(label = "Stretch length", unit = "s")]
    pub stretch_length: Option<Seconds>,
    /// A repeating frame structure was found.
    #[field(label = "Frame found")]
    pub frame_found: bool,
    /// The frame period in symbols.
    #[field(label = "Frame repeats (symbols)")]
    pub frame_symbols: Option<i64>,
    /// The frame period.
    #[field(label = "Frame repeats", unit = "s")]
    pub frame_period: Option<Seconds>,
    /// The number of frame copies in the buffer.
    #[field(label = "Copies")]
    pub frame_copies: Option<i64>,
    /// The mean agreement of neighbouring copies.
    #[field(label = "Agreement of copies", unit = "%")]
    pub frame_agreement_pct: Option<f64>,
    /// The first symbol of the constant part.
    #[field(label = "Constant part from symbol")]
    pub constant_start: Option<i64>,
    /// The length of the constant part, in symbols.
    #[field(label = "Constant part (symbols)")]
    pub constant_symbols: Option<i64>,
    /// The constant part as hexadecimal, 2 bits per symbol.
    #[field(label = "Constant part")]
    pub constant_hex: &'a str,
    /// The analysis note (PACTOR-IV).
    #[field(label = "Note")]
    pub note: &'a str,
    /// The newest phase steps of the first carrier, one digit per symbol.
    #[field(label = "Symbols, carrier 1")]
    pub symbols_1: &'a str,
    /// The newest phase steps of the second carrier.
    #[field(label = "Symbols, carrier 2")]
    pub symbols_2: &'a str,
    /// The newest phase steps of the third carrier.
    #[field(label = "Symbols, carrier 3")]
    pub symbols_3: &'a str,
    /// The analyses run.
    #[field(label = "Analyses")]
    pub analyses: i64,
    /// The analyses that found a signal.
    #[field(label = "Analyses with a signal")]
    pub analyses_with_signal: i64,
    /// The audio level of the last second.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
}

/// A carrier of the last analysis. The row key is the carrier's number, "00" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PactorhfCarrierRow {
    /// The carrier frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The clock line of the carrier.
    #[field(label = "Clock line", unit = "dB")]
    pub line_db: f64,
    /// Phase steps per symbol.
    #[field(label = "Phase steps")]
    pub steps: i64,
    /// The modulation, when the phase steps are 2, 4, 8 or 16.
    #[field(label = "Modulation")]
    pub modulation: Option<PactorModulation>,
    /// The concentration of the phase steps, 0 to 100 %.
    #[field(label = "Concentration", unit = "%")]
    pub concentration_pct: f64,
}

/// An event of the PACTOR-II, -III and -IV stacks. The row key is the sequence number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PactorhfEventRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// What happened.
    #[field(label = "Event")]
    pub text: &'a str,
}

/// A new variant or carrier count: `text` describes the signal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PactorhfAnalysisEvent<'a> {
    /// The variant found.
    #[field(label = "Variant")]
    pub variant: i64,
    /// The signal in words.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The carriers of the last analysis, lowest first.
pub static PACTORHF_CARRIERS: Table<PactorhfCarrierRow> = Table::new("carriers", "Carriers", 18);
/// The events, oldest first.
pub static PACTORHF_EVENTS: Table<PactorhfEventRow<'static>> = Table::new("events", "Events", 60);
/// A new variant or carrier count.
pub static PACTORHF_ANALYSIS: EventKind<PactorhfAnalysisEvent<'static>> =
    EventKind::new("pactorhf.analysis", "PACTOR signal");

/// A PACTOR-II, -III or -IV stack `id`. The component holds the event log with the measures of the last analysis, so
/// it keeps its results over a change of frequency.
fn pactorhf(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<PactorhfFields>()
        .table(&PACTORHF_CARRIERS)
        .table(&PACTORHF_EVENTS)
        .series(SeriesDef {
            key: crate::pactorhf::rx::LEVEL_SERIES,
            label: "Level",
            x_unit: "s",
            y_unit: "dB",
        })
        .event(&PACTORHF_ANALYSIS)
        .command(&objcmd::CLEAR)
        .kept(&["events", "PactorhfFields"])
}

// Robust Packet.

/// The result fields of the Robust Packet stack: the comb of the last analysis and the last burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RprFields<'a> {
    /// What the last analysis found: nothing, the comb, or bursts.
    #[field(label = "State")]
    pub state: RprState,
    /// The centre of the comb in the audio. Absent without a comb.
    #[field(label = "Comb centre", unit = "Hz")]
    pub comb_centre: Option<Hertz>,
    /// The comb centre against the audio centre setting.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The mean power inside the comb over the mean power beside it.
    #[field(label = "Comb strength", unit = "dB")]
    pub comb_db: Option<f64>,
    /// The carriers seen: those of the last burst with a clear step pattern, else the comb cells that stand out.
    #[field(label = "Carriers seen")]
    pub carrier_count: i64,
    /// The carrier spacing measured on the last burst.
    #[field(label = "Spacing", unit = "Hz")]
    pub spacing: Option<Hertz>,
    /// The symbol rate of the last burst, at the peak of its clock line.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The clock error of the last burst of more than 150 symbols.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The speed level of the last burst: 1 (D-BPSK) or 2 (D-QPSK); 0 before a burst.
    #[field(label = "Speed level")]
    pub speed_level: i64,
    /// The modulation of the last burst.
    #[field(label = "Modulation")]
    pub modulation: Option<RprModulation>,
    /// The net rate of the speed level before the AX.25 overhead: 200 or 600 bit/s.
    #[field(label = "Net rate", unit = "bit/s")]
    pub net_rate: Option<i64>,
    /// The SNR of the last burst, from the phase error of its steps.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The best SNR of a burst since the last Clear.
    #[field(label = "Best SNR", unit = "dB")]
    pub best_snr_db: Option<f64>,
    /// The length of the last burst.
    #[field(label = "Last burst", unit = "s")]
    pub burst_length: Option<Seconds>,
    /// The newest step indices of carrier 0, one digit per symbol.
    #[field(label = "Steps, carrier 0")]
    pub symbols_1: &'a str,
    /// The newest step indices of carrier 1.
    #[field(label = "Steps, carrier 1")]
    pub symbols_2: &'a str,
    /// The newest step indices of carrier 2.
    #[field(label = "Steps, carrier 2")]
    pub symbols_3: &'a str,
    /// What the stack does not do.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The analyses run.
    #[field(label = "Analyses")]
    pub analyses: i64,
    /// The analyses that found the comb.
    #[field(label = "Analyses with the comb")]
    pub detections: i64,
    /// The bursts demodulated since the last Clear.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The audio level of the last second.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
}

/// A carrier of the last burst. The row key is the carrier number, "0" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RprCarrierRow {
    /// The carrier frequency in the audio: its place in the comb and its measured turn per symbol.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The concentration of its phase steps for the order of the burst, 0 to 100 %.
    #[field(label = "Concentration", unit = "%")]
    pub concentration_pct: f64,
}

/// A burst demodulated. The row key is the sequence number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RprBurstRow<'a> {
    /// The signal time of its start.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC of its start, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// Its length.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
    /// The speed level: 1 or 2.
    #[field(label = "Speed level")]
    pub speed_level: i64,
    /// The modulation.
    #[field(label = "Modulation")]
    pub modulation: RprModulation,
    /// The net rate of the speed level: 200 or 600 bit/s.
    #[field(label = "Net rate", unit = "bit/s")]
    pub net_rate: i64,
    /// The carriers with a clear step pattern.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// The symbols demodulated on each carrier.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// The SNR from the phase error of its steps.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// The comb centre against the audio centre setting.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// The step indices of the first 24 symbol times as hexadecimal: carrier 0 to 7 in turn, 1 bit (D-BPSK) or 2 bits (D-QPSK)
    /// each, most significant first. The bits are still coded and scrambled.
    #[field(label = "First steps")]
    pub first_steps: &'a str,
}

/// An event of the Robust Packet stack. The row key is the sequence number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RprEventRow<'a> {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// What happened.
    #[field(label = "Event")]
    pub text: &'a str,
}

/// A burst demodulated, with its whole stream of phase steps.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RprBurstEvent<'a> {
    /// The speed level: 1 or 2.
    #[field(label = "Speed level")]
    pub speed_level: i64,
    /// The modulation.
    #[field(label = "Modulation")]
    pub modulation: RprModulation,
    /// The carriers with a clear step pattern.
    #[field(label = "Carriers")]
    pub carriers: i64,
    /// The symbols demodulated on each carrier.
    #[field(label = "Symbols")]
    pub symbols: i64,
    /// The length of the burst.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
    /// The SNR from the phase error of its steps.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The comb centre against the audio centre setting.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The step indices, one byte each (0 or 1 for D-BPSK, 0 to 3 for D-QPSK; the step is index x 360 / order degrees after
    /// the constant turn of the carrier): for each symbol time after the first, carrier 0 to 7 in turn.
    #[field(label = "Steps")]
    pub steps: Arc<[u8]>,
    /// The burst in words.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The eight carriers of the last burst.
pub static RPR_CARRIERS: Table<RprCarrierRow> = Table::new("carriers", "Carriers", 8);
/// The bursts demodulated, oldest first.
pub static RPR_BURSTS: Table<RprBurstRow<'static>> = Table::new("bursts", "Bursts", 60);
/// The events, oldest first.
pub static RPR_EVENTS: Table<RprEventRow<'static>> = Table::new("events", "Events", 60);
/// A burst demodulated.
pub static RPR_BURST: EventKind<RprBurstEvent<'static>> =
    EventKind::new("rpr.burst", "Robust Packet burst");

/// The Robust Packet stack. The bursts and the event log stay over a change of frequency, with the measures of the same
/// component.
fn rpr() -> StackShape {
    StackShape::new("rpr")
        .fields::<RprFields>()
        .table(&RPR_CARRIERS)
        .table(&RPR_BURSTS)
        .table(&RPR_EVENTS)
        .series(SeriesDef {
            key: crate::rpr::rx::LEVEL_SERIES,
            label: "Level",
            x_unit: "s",
            y_unit: "dB",
        })
        .event(&RPR_BURST)
        .settings(RprSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&["bursts", "events", "RprFields"])
}

// PACTOR-I.

/// The result fields of PACTOR-I.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Pactor1Fields<'a> {
    /// A tone pair is locked or a frame was read in the last 30 s (the lock state "Signal").
    #[field(label = "Signal")]
    pub signal: bool,
    /// A frame was read in the last 30 s (the lock state "Packets").
    #[field(label = "Packets")]
    pub packets_recent: bool,
    /// Good packets.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// Good packets at 200 baud.
    #[field(label = "At 200 baud")]
    pub packets_fast: i64,
    /// Good packets at 100 baud.
    #[field(label = "At 100 baud")]
    pub packets_slow: i64,
    /// Packets whose CRC failed.
    #[field(label = "Packets failed")]
    pub packets_bad: i64,
    /// Control signals.
    #[field(label = "Control signals")]
    pub control_signals: i64,
    /// Repeated packets.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// Packets read by memory ARQ.
    #[field(label = "Memory ARQ saves")]
    pub memory_arq_saves: i64,
    /// Changeovers (CS3).
    #[field(label = "Changeovers")]
    pub changeovers: i64,
    /// Connect packets.
    #[field(label = "Connect packets")]
    pub connects: i64,
    /// The link is in unproto (FEC) mode, else ARQ.
    #[field(label = "Unproto / FEC")]
    pub fec_mode: bool,
    /// The Huffman table holds all 95 printable characters.
    #[field(label = "Huffman table complete")]
    pub huffman_complete: bool,
    /// Entries in the Huffman table.
    #[field(label = "Huffman entries")]
    pub huffman_entries: i64,
    /// The locked lower tone. Absent without a lock.
    #[field(label = "Lower tone", unit = "Hz")]
    pub pair_low: Option<Hertz>,
    /// The locked upper tone. Absent without a lock.
    #[field(label = "Upper tone", unit = "Hz")]
    pub pair_high: Option<Hertz>,
    /// The centre of the pair against 1500 Hz. Absent without a lock.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR of the tone pair. Absent without a lock.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The audio level of the last second.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The text of both sides.
    #[field(label = "Text")]
    pub text: &'a str,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
}

/// A frame of PACTOR-I. The row key is the frame's number in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Pactor1FrameRow<'a> {
    /// The signal time of the frame's first bit.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The UTC of the frame, when the clock is known.
    #[field(label = "UTC")]
    pub time: Option<Time>,
    /// The kind of frame.
    #[field(label = "Kind")]
    pub kind: FrameType,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The header byte of a packet, 0x55 or 0xAA.
    #[field(label = "Header")]
    pub header: Option<i64>,
    /// The packet number 0 to 3.
    #[field(label = "#")]
    pub number: Option<i64>,
    /// Huffman text.
    #[field(label = "Huffman")]
    pub huffman: bool,
    /// The break-in flag.
    #[field(label = "Break-in")]
    pub break_in: bool,
    /// The QRT flag.
    #[field(label = "QRT")]
    pub qrt: bool,
    /// The CRC passed.
    #[field(label = "CRC")]
    pub crc_ok: bool,
    /// Read by memory ARQ.
    #[field(label = "Memory ARQ")]
    pub memory_arq: bool,
    /// A repeat of the packet before.
    #[field(label = "Repeat")]
    pub repeat: bool,
    /// A repeat with no control signal since the packet before.
    #[field(label = "Unproto / FEC repeat")]
    pub fec_repeat: bool,
    /// The tones were swapped.
    #[field(label = "Swapped")]
    pub inverted: bool,
    /// The sending side, 1 or 2.
    #[field(label = "Side")]
    pub side: i64,
    /// The text of the frame.
    #[field(label = "Text")]
    pub text: &'a str,
    /// The data bytes in hexadecimal.
    #[field(label = "Data")]
    pub data_hex: &'a str,
    /// The called station of a connect packet: the row key of its row of the stations table.
    #[field(label = "Station", refers_to = "stations")]
    pub call: Option<&'a str>,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The lower tone of the frame.
    #[field(label = "Lower tone", unit = "Hz")]
    pub pair_low: Hertz,
}

/// A station of PACTOR-I. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Pactor1StationRow<'a> {
    /// The callsign.
    #[field(label = "Station")]
    pub call: &'a str,
    /// How the station was seen.
    #[field(label = "Seen as")]
    pub seen_as: SeenAs,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// When last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A frame of PACTOR-I.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PactorFrameEvent<'a> {
    /// The kind of frame.
    #[field(label = "Kind")]
    pub kind: FrameType,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The header byte of a packet.
    #[field(label = "Header")]
    pub header: Option<i64>,
    /// The packet number 0 to 3.
    #[field(label = "#")]
    pub number: Option<i64>,
    /// The sender; not read.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The called station of a connect packet.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The text of the frame.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The sending side, 1 or 2.
    #[field(label = "Side")]
    pub side: i64,
    /// The tones were swapped.
    #[field(label = "Swapped")]
    pub inverted: bool,
    /// A repeat of the packet before.
    #[field(label = "Repeat")]
    pub repeat: bool,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// What the event is about: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The arguments of `remove` of a PACTOR-I station.
#[derive(Arguments)]
pub struct Pactor1StationArgs<'a> {
    /// The row key of the station.
    #[arg(shared = PACTOR1_STATION_ROW)]
    pub row: &'a str,
}

/// The frames of PACTOR-I, oldest first.
pub static PACTOR1_FRAMES: Table<Pactor1FrameRow<'static>> = Table::new("frames", "Frames", 300);
/// The stations of PACTOR-I.
pub static PACTOR1_STATIONS: Table<Pactor1StationRow<'static>> =
    Table::new("stations", "Stations", 60);
/// A frame of PACTOR-I.
pub static PACTOR_FRAME: EventKind<PactorFrameEvent<'static>> =
    EventKind::new("pactor.frame", "PACTOR-I frame");
/// The row key of a PACTOR-I station.
pub const PACTOR1_STATION_ROW: Argument<field::Text> = objcmd::row_of("stations");
/// Forgets one PACTOR-I station.
pub const PACTOR1_REMOVE: CommandKind<Pactor1StationArgs<'static>> = objcmd::remove_kind();

// HF beacons.

/// The result fields of the HF beacon monitor.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbeaconsFields<'a> {
    /// The low edge of the span shown (centre - 0.4 x rate).
    #[field(label = "Span from", unit = "Hz")]
    pub span_low: Hertz,
    /// The high edge of the span shown (centre + 0.4 x rate).
    #[field(label = "Span to", unit = "Hz")]
    pub span_high: Hertz,
    /// Beacons of the list in the span.
    #[field(label = "Beacons in the span")]
    pub in_span: i64,
    /// Beacons heard now.
    #[field(label = "Heard now")]
    pub heard_now: i64,
    /// Channels watched.
    #[field(label = "Channels watched")]
    pub channels: i64,
    /// Frames processed.
    #[field(label = "Frames processed")]
    pub frames: i64,
    /// Keying runs started.
    #[field(label = "Runs started")]
    pub runs_started: i64,
    /// Where the times come from.
    #[field(label = "Clock")]
    pub clock: ClockSource,
    /// The clock is good enough for the time slots of the beacons.
    #[field(label = "Clock valid")]
    pub clock_valid: bool,
    /// Beacons in the list.
    #[field(label = "Beacons in the list")]
    pub list_count: i64,
    /// The station position is known.
    #[field(label = "Station position known")]
    pub station_known: bool,
    /// The band to try now, when the model names one.
    #[field(label = "Band to try now")]
    pub best_band: Option<&'a str>,
    /// The stations heard on that band.
    #[field(label = "Stations on it")]
    pub best_band_stations: Option<i64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A beacon of the list. The row key is the place in the list, "0000" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbeaconsBeaconRow<'a> {
    /// The beacon's name.
    #[field(label = "Beacon")]
    pub name: &'a str,
    /// The frequency of the beacon.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The keying pattern.
    #[field(label = "Kind")]
    pub kind: Pattern,
    /// The entry of the list is uncertain.
    #[field(label = "Uncertain")]
    pub uncertain: bool,
    /// The beacon is inside the capture.
    #[field(label = "In the capture")]
    pub in_capture: bool,
    /// The beacon is heard now.
    #[field(label = "Heard")]
    pub heard: bool,
    /// The confidence of the identity.
    #[field(label = "Confidence")]
    pub confidence: i64,
    /// The signal time when last heard.
    #[field(label = "Last heard (signal time)", unit = "s")]
    pub last: Option<Seconds>,
    /// The UTC when last heard, in whole seconds.
    #[field(label = "Last heard UTC")]
    pub last_heard: Option<Time>,
    /// The SNR when last heard.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The best SNR of the last hour.
    #[field(label = "1 h best", unit = "dB")]
    pub best_1h_db: Option<f64>,
    /// The best SNR of the last 24 hours.
    #[field(label = "24 h best", unit = "dB")]
    pub best_24h_db: Option<f64>,
    /// Power steps heard.
    #[field(label = "Power steps")]
    pub steps: i64,
    /// The SNR of each power step.
    #[field(label = "Step SNRs")]
    pub step_snrs: &'a str,
    /// The distance to the station.
    #[field(label = "Distance", unit = "km")]
    pub distance_km: Option<f64>,
    /// The bearing from the station.
    #[field(label = "Bearing", unit = "deg")]
    pub bearing_deg: Option<f64>,
    /// The newest 48 history points in dB, "-" for listened and not heard.
    #[field(label = "Trend")]
    pub trend: &'a str,
    /// History points in 24 hours.
    #[field(label = "Listens")]
    pub listens: i64,
    /// History points heard.
    #[field(label = "Heard")]
    pub heard_count: i64,
    /// A note on the beacon.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The band of the beacon.
    #[field(label = "Band")]
    pub band: &'a str,
    /// The position of the beacon.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A band of the beacon list. The row key is the place, "00" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbeaconsBandRow<'a> {
    /// The band.
    #[field(label = "Band")]
    pub band: &'a str,
    /// Beacons of the band in the list.
    #[field(label = "In the list")]
    pub in_list: i64,
    /// Beacons of the band in the capture.
    #[field(label = "In the capture")]
    pub in_capture: i64,
    /// Beacons of the band heard now.
    #[field(label = "Heard now")]
    pub heard_now: i64,
    /// The farthest beacon heard.
    #[field(label = "Farthest", unit = "km")]
    pub farthest_km: Option<f64>,
}

/// The propagation of a band. The row key is the place, "00" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbeaconsPropagationRow<'a> {
    /// The band.
    #[field(label = "Band")]
    pub band: &'a str,
    /// The state of the band.
    #[field(label = "State")]
    pub state: BandState,
    /// Stations heard.
    #[field(label = "Stations")]
    pub stations: i64,
    /// The best SNR.
    #[field(label = "Best SNR", unit = "dB")]
    pub best_snr_db: Option<f64>,
    /// The farthest station heard.
    #[field(label = "Farthest", unit = "km")]
    pub farthest_km: Option<f64>,
    /// The score of the band.
    #[field(label = "Score")]
    pub score: f64,
}

/// A beacon heard with its identity confirmed: `source` is the beacon's name, `text` the note.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbDetectionEvent<'a> {
    /// The beacon's name.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The confidence of the identity.
    #[field(label = "Confidence")]
    pub confidence: i64,
    /// The note.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position of the beacon, when known.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The beacons of the list.
pub static HFBEACONS_BEACONS: Table<HfbeaconsBeaconRow<'static>> =
    Table::new("beacons", "Beacons", 4096);
/// The bands of the list.
pub static HFBEACONS_BANDS: Table<HfbeaconsBandRow<'static>> =
    Table::new("bands", "Bands open", 16);
/// The propagation of each band.
pub static HFBEACONS_PROPAGATION: Table<HfbeaconsPropagationRow<'static>> =
    Table::new("propagation", "Propagation", 16);
/// A beacon heard.
pub static HFB_DETECTION: EventKind<HfbDetectionEvent<'static>> =
    EventKind::new("hfb.detection", "Beacon heard");

/// The result fields of the own beacon. The values of a sending have no value while nothing is sent.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbTxFields<'a> {
    /// Whether the beacon runs: it sends now or waits for its next sending.
    #[field(label = "Beacon on")]
    pub tx_beacon_on: bool,
    /// Whether a sending is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The share of the sending on air that is sent.
    #[field(label = "Sending progress", unit = "%")]
    pub tx_progress_pct: Option<f64>,
    /// The time to the next sending, while the beacon waits.
    #[field(label = "Next sending in", unit = "s")]
    pub tx_next_s: Option<Seconds>,
    /// The sendings since `start`.
    #[field(label = "Sendings")]
    pub tx_sendings: i64,
    /// The time on air of one sending with the present settings.
    #[field(label = "Sending time on air", unit = "s")]
    pub tx_sending_s: Seconds,
    /// The share of the interval that one sending is on air.
    #[field(label = "Duty of the schedule", unit = "%")]
    pub tx_duty_pct: f64,
    /// What the beacon is: a proposal of the specification, not taken from a standard.
    #[field(label = "Transmit note")]
    pub tx_note: &'a str,
}

/// The fields of a change of the own beacon.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfbTxEvent<'a> {
    /// The state: started (the first sending), sending (a later one), sent (a sending ended), finished (the beacon
    /// ended) or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, or why it was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The text keyed.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A change of the own beacon.
pub static HFB_TX: EventKind<HfbTxEvent<'static>> = EventKind::new("hfb.tx", "Own beacon");

/// Starts the own beacon.
pub const HFB_START: Command = Command::new("start", "Start the beacon")
    .help(
        "Sends the message in Morse at offset_s after the next interval boundary of UTC (at once when that is now), then \
         again each interval, until stop or count sendings. Each sending is one transmission; between sendings the \
         transmitter is free. Transmit must be on and the call sign set.",
    )
    .transmits(sdrkit_core::TxContent::Beacon)
    .stopped_by("stop");
/// Sends the own beacon once.
pub const HFB_SEND: Command = Command::new("send", "Send the beacon once")
    .help("Sends the message in Morse once, at once, outside the schedule. Transmit must be on and the call sign set.")
    .transmits(sdrkit_core::TxContent::Beacon)
    .stopped_by("stop");
/// Stops the own beacon.
pub const HFB_STOP: Command = Command::new("stop", "Stop the beacon")
    .help("Ends the sending on air within 50 ms, with its fall, and ends the schedule.");

// The wave buoys.

/// The result fields of the wave buoy stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WavebuoyFields<'a> {
    /// The link format.
    #[field(label = "Format")]
    pub format: LinkFormat,
    /// The format is forced by the `format` setting.
    #[field(label = "Format forced")]
    pub format_forced: bool,
    /// The carrier finder sees a signal.
    #[field(label = "Signal found")]
    pub signal_found: bool,
    /// The data is valid: 2FSK with the HXV vectors locked, or 4FSK with the symbols locked.
    #[field(label = "Data valid")]
    pub data_valid: bool,
    /// The centre of the signal about the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The absolute frequency of the signal centre.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The listed buoy channel within 6 kHz of the signal.
    #[field(label = "Channel")]
    pub channel: Option<&'a str>,
    /// The peak of the spectrum over the floor.
    #[field(label = "Peak over the floor", unit = "dB")]
    pub peak_db: Option<f64>,
    /// The width holding 90 % of the signal power.
    #[field(label = "Width (90 % of the power)", unit = "Hz")]
    pub width: Option<Hertz>,
    /// The SNR in 100 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The clock error.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The measured distance between the tones.
    #[field(label = "Tone step", unit = "Hz")]
    pub tone_step: Option<Hertz>,
    /// The RMS distance of the symbol means to the nearest tone, in tone steps.
    #[field(label = "Symbol error", unit = "steps")]
    pub symbol_error_steps: Option<f64>,
    /// Symbols demodulated since the last restart.
    #[field(label = "Symbols")]
    pub symbol_count: i64,
    /// The symbols are locked.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The newest 320 bits (higher tone = 1; 4FSK: the tone number as two bits).
    #[field(label = "Bits")]
    pub bits: &'a str,
    /// The HXV vectors are locked.
    #[field(label = "HXV locked")]
    pub hxv_locked: bool,
    /// The tones are the other way round.
    #[field(label = "Inverted")]
    pub hxv_inverted: bool,
    /// Vectors accepted.
    #[field(label = "Vectors")]
    pub vector_count: i64,
    /// Vectors accepted with no bit repaired.
    #[field(label = "Clean vectors")]
    pub clean_count: i64,
    /// Vectors accepted with 1 or 2 bits repaired.
    #[field(label = "Repaired vectors")]
    pub repaired_count: i64,
    /// Vectors lost.
    #[field(label = "Lost vectors")]
    pub lost_count: i64,
    /// Blocks with a system word read.
    #[field(label = "Blocks")]
    pub block_count: i64,
    /// The system word number of the last block.
    #[field(label = "Last block")]
    pub last_block: Option<i64>,
    /// The last vector is a WR-SG vector (two vertical samples, west AAA).
    #[field(label = "WR-SG")]
    pub wrsg: bool,
    /// The heave of the last vector.
    #[field(label = "Heave", unit = "cm")]
    pub heave_cm: Option<i64>,
    /// The north displacement of the last vector.
    #[field(label = "North", unit = "cm")]
    pub north_cm: Option<i64>,
    /// The west displacement of the last vector.
    #[field(label = "West", unit = "cm")]
    pub west_cm: Option<i64>,
    /// System file words received, of 16.
    #[field(label = "System words received")]
    pub system_word_count: i64,
    /// Spectrum points received, of 64.
    #[field(label = "Spectrum points received")]
    pub spectrum_point_count: i64,
    /// The buoy type number Tn (system word 0).
    #[field(label = "Tn")]
    pub tn: Option<i64>,
    /// The GPS module works (system word 0).
    #[field(label = "GPS module ok")]
    pub gps_ok: Option<bool>,
    /// The GPS has a fix (system word 0).
    #[field(label = "GPS fix")]
    pub gps_fix: Option<bool>,
    /// The RMS wave height (system word 1).
    #[field(label = "Hrms", unit = "m")]
    pub hrms_m: Option<f64>,
    /// The significant wave height, 4 Hrms (system word 1).
    #[field(label = "Hs", unit = "m")]
    pub hs_m: Option<f64>,
    /// The zero-crossing frequency (system word 2).
    #[field(label = "fz", unit = "Hz")]
    pub fz: Option<Hertz>,
    /// The zero-crossing period, 1 / fz (system word 2).
    #[field(label = "Tz", unit = "s")]
    pub tz_s: Option<f64>,
    /// The largest spectral density (system word 3).
    #[field(label = "PSDmax", unit = "m^2/Hz")]
    pub psd_max_m2_hz: Option<f64>,
    /// The buoy temperature Tr (system word 4).
    #[field(label = "Tr", unit = "°C")]
    pub tr_c: Option<f64>,
    /// The water temperature Tw (system word 5).
    #[field(label = "Tw (water)", unit = "°C")]
    pub tw_c: Option<f64>,
    /// The operational life left (system word 6).
    #[field(label = "Life left", unit = "weeks")]
    pub life_left_weeks: Option<i64>,
    /// The battery status, 0 to 7 (system word 6).
    #[field(label = "Battery status")]
    pub battery_status: Option<i64>,
    /// The vertical accelerometer offset (system word 7).
    #[field(label = "av0", unit = "m/s^2")]
    pub av0_m_s2: Option<f64>,
    /// The x accelerometer offset (system word 8).
    #[field(label = "ax0", unit = "m/s^2")]
    pub ax0_m_s2: Option<f64>,
    /// The y accelerometer offset (system word 9).
    #[field(label = "ay0", unit = "m/s^2")]
    pub ay0_m_s2: Option<f64>,
    /// The buoy position (system words 10 to 13).
    #[field(label = "Position")]
    pub position: Option<(f64, f64)>,
    /// The orientation (system word 14).
    #[field(label = "Orientation", unit = "°")]
    pub orientation_deg: Option<f64>,
    /// The inclination (system word 15).
    #[field(label = "Inclination", unit = "°")]
    pub inclination_deg: Option<f64>,
    /// The repetition period of the bit stream.
    #[field(label = "Period", unit = "bits")]
    pub period_bits: Option<i64>,
    /// How far the period stands out of random bits (z score).
    #[field(label = "Period z")]
    pub period_z: Option<f64>,
    /// The constant part of a period, in hex.
    #[field(label = "Sync")]
    pub sync: Option<&'a str>,
    /// The first bit of the constant part in the period.
    #[field(label = "Sync start", unit = "bit")]
    pub sync_start_bit: Option<i64>,
    /// The CRC or checksum that fits the frames (an attempt, never a verified Datawell CRC).
    #[field(label = "CRC attempt")]
    pub crc_attempt: Option<&'a str>,
    /// Frames the attempt passed in the last analysis.
    #[field(label = "CRC passes")]
    pub crc_passes: i64,
    /// Frames the CRC attempt accepts.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames the CRC attempt refuses.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
}

/// An entry of the directional spectrum received. The row key is the entry number, "00" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WavebuoySpectrumRow {
    /// The spectrum entry number, 0 to 63.
    #[field(label = "n")]
    pub entry: i64,
    /// The frequency of the entry.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The spectral density, once PSDmax is known.
    #[field(label = "PSD", unit = "m^2/Hz")]
    pub psd_m2_hz: Option<f64>,
    /// The mean wave direction.
    #[field(label = "Direction", unit = "°")]
    pub direction_deg: f64,
    /// The directional spread.
    #[field(label = "Spread", unit = "°")]
    pub spread_deg: f64,
    /// The second-order Fourier coefficient m2 (no unit).
    #[field(label = "m2")]
    pub m2: f64,
    /// The second-order Fourier coefficient n2 (no unit).
    #[field(label = "n2")]
    pub n2: f64,
    /// The check factor K (no unit).
    #[field(label = "K")]
    pub k: f64,
    /// The density relative to PSDmax (no unit).
    #[field(label = "Relative")]
    pub relative_psd: f64,
}

/// A system file word received. The row key is the word number, "00" first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WavebuoySystemWordRow {
    /// The system file word number, 0 to 15.
    #[field(label = "Word")]
    pub word: i64,
    /// The 12-bit value of the word.
    #[field(label = "Value")]
    pub value: i64,
}

/// A frame cut at the sync. The row key is the frame's number in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WavebuoyFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub start: Seconds,
    /// The bits of the frame in hex.
    #[field(label = "Frame")]
    pub hex: &'a str,
    /// The CRC attempt accepts the frame.
    #[field(label = "Accepted")]
    pub accepted: bool,
}

/// An accepted HXV vector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WavebuoyVectorEvent {
    /// The heave of the vector.
    #[field(label = "Heave", unit = "cm")]
    pub heave_cm: i64,
    /// The north displacement of the vector.
    #[field(label = "North", unit = "cm")]
    pub north_cm: i64,
    /// The west displacement of the vector.
    #[field(label = "West", unit = "cm")]
    pub west_cm: i64,
    /// The cyclic data word of the vector.
    #[field(label = "Cyclic word")]
    pub cyclic: i64,
    /// What the event is about: a sensor.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The directional spectrum received.
pub static WAVEBUOY_SPECTRUM: Table<WavebuoySpectrumRow> = Table::new("spectrum", "Spectrum", 64);
/// The system file words received.
pub static WAVEBUOY_SYSTEM_WORDS: Table<WavebuoySystemWordRow> =
    Table::new("system_words", "System words", 16);
/// The frames cut at the sync, oldest first.
pub static WAVEBUOY_FRAMES: Table<WavebuoyFrameRow<'static>> =
    Table::new("frames", "Frames", crate::buoy::results::FRAMES_CAP);
/// An accepted HXV vector.
pub static WAVEBUOY_VECTOR: EventKind<WavebuoyVectorEvent> =
    EventKind::new("wavebuoy.vector", "HXV vector");

// The HF sender stacks: XPA, CROWD36, FSK200, GW FSK, the telephone FSK modems, the UVB-76 and VLF MSK monitors.

/// A row of a number-sender stack: one thing read. The row key is the count of rows before it, as six digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SenderRow<'a> {
    /// The time it was read.
    #[field(label = "Time")]
    pub time: Time,
    /// What it is: start, message start, groups, block sync, break, end, sync, text.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The signal and speed it was read as.
    #[field(label = "Id")]
    pub id: &'a str,
    /// What was read.
    #[field(label = "Text")]
    pub text: &'a str,
    /// The SNR in 3 kHz of the last fit of the tones. Absent without a fit.
    #[field(label = "SNR", unit = "dB")]
    pub snr: Option<f64>,
}

/// The tones of a sender were found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TonesFoundEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The offset of the tones from their nominal place.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// The SNR in 3 kHz.
    #[field(label = "SNR", unit = "dB")]
    pub snr: f64,
}

/// The tones of a sender are gone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TonesLostEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The offset of the tones when they were lost.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// Characters or groups read in the lock.
    #[field(label = "Read")]
    pub read: i64,
}

/// The rows of a number-sender stack, newest first.
pub static SENDER_ROWS: Table<SenderRow<'static>> = Table::new("rows", "Rows", 500);
/// The tones were found.
pub static TONES_FOUND: EventKind<TonesFoundEvent<'static>> =
    EventKind::new("tones.found", "Tones found");
/// The tones are gone.
pub static TONES_LOST: EventKind<TonesLostEvent<'static>> =
    EventKind::new("tones.lost", "Tones lost");

/// The result fields of the CROWD36 stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Crowd36Fields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text read, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    #[field(label = "State")]
    pub state: i64,
    /// The offset of the tones from their nominal place. Absent while the tones are not found.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The sync tones of the last sync, after the correction. Absent before a sync.
    #[field(label = "Sync tones")]
    pub sync_tones: Option<&'a str>,
    /// Characters read.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Text lines read.
    #[field(label = "Good")]
    pub good: i64,
    /// Tones read outside the character tables.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Text lines refused by the check of their form.
    #[field(label = "Refused")]
    pub refused: i64,
}

/// The result fields of the XPA stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct XpaFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The groups read, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    #[field(label = "State")]
    pub state: i64,
    /// The offset of the tones from their nominal place. Absent while the tones are not found.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The speed of the reading that won. Absent before a reading wins.
    #[field(label = "Speed", unit = "Bd")]
    pub speed: Option<f64>,
    /// The sideband of the reading that won: "upper" or "lower (tones reversed)".
    #[field(label = "Sideband")]
    pub sideband: Option<&'a str>,
    /// The numbering of the reading that won: "as found" or "moved by +N tones".
    #[field(label = "Tone numbering")]
    pub numbering: Option<&'a str>,
    /// Groups read.
    #[field(label = "Groups")]
    pub groups: i64,
    /// Lines of groups read.
    #[field(label = "Good")]
    pub good: i64,
    /// Tones read without a meaning.
    #[field(label = "Bad")]
    pub bad: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The result fields of the FSK200 stack. A field that the submode in use does not count has no value.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fsk200Fields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The text and blocks read, the last characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The search state: 0 searching, 1 tones found, 2 decoding.
    #[field(label = "State")]
    pub state: i64,
    /// The centre of the tone pair above the dial. Absent while the tones are not found.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR in 3 kHz of the last fit. Absent while the tones are not found.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The tones from the dial and their sense. Absent while the tones are not found.
    #[field(label = "Tones")]
    pub tones: Option<&'a str>,
    /// The traffic kind of f06: F06 figure groups, ASCII or F06a file.
    #[field(label = "Traffic")]
    pub traffic: Option<&'a str>,
    /// Characters read (f01).
    #[field(label = "Characters")]
    pub characters: Option<i64>,
    /// Characters with a stop unit of space (f01).
    #[field(label = "Framing errors")]
    pub framing_errors: Option<i64>,
    /// Blocks whose CRC holds, and dividers (f06).
    #[field(label = "Good blocks")]
    pub blocks_good: Option<i64>,
    /// Blocks corrected by one bit (f06).
    #[field(label = "Corrected blocks")]
    pub blocks_corrected: Option<i64>,
    /// Blocks with a CRC error (f06).
    #[field(label = "CRC errors")]
    pub crc_errors: Option<i64>,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A block of f06 was read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct F06BlockEvent<'a> {
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The block number. Absent for a divider.
    #[field(label = "Block")]
    pub number: Option<i64>,
    /// Divider, Header, Block or File.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The CRC result.
    #[field(label = "CRC")]
    pub crc: &'a str,
}

/// A block of f06.
pub static F06_BLOCK: EventKind<F06BlockEvent<'static>> = EventKind::new("f06.block", "Block");

/// The result fields of the GW FSK stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GwfskFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The state: 0 nothing heard, 1 bursts heard, 2 bursts read.
    #[field(label = "State")]
    pub state: i64,
    /// The tones of the last burst whose tones were found.
    #[field(label = "Tones")]
    pub tones: Option<&'a str>,
    /// The code of the last shore station heard.
    #[field(label = "Last shore station")]
    pub last_station: Option<&'a str>,
    /// Bursts analysed.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Bursts with tones that were not understood.
    #[field(label = "Not understood")]
    pub not_understood: i64,
    /// Packets read.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Channel markers read.
    #[field(label = "Channel markers")]
    pub markers: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A burst of GW FSK was read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GwPacketEvent<'a> {
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Channel marker, Packet, Position or ACK.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The type of a packet.
    #[field(label = "Type")]
    pub packet_type: Option<i64>,
    /// The subtype of a packet.
    #[field(label = "Subtype")]
    pub subtype: Option<i64>,
}

/// A burst of GW FSK.
pub static GW_PACKET: EventKind<GwPacketEvent<'static>> = EventKind::new("gw.packet", "Packet");

/// The result fields of the telephone FSK modem stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskModemsFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The state: 0 nothing heard, 1 a burst is open, 2 messages read.
    #[field(label = "State")]
    pub state: i64,
    /// The standard of the last message.
    #[field(label = "Standard")]
    pub standard: Option<&'a str>,
    /// The tones, the sideband and the dial error of the last message.
    #[field(label = "Tones")]
    pub tones: Option<&'a str>,
    /// The bytes, framing errors and length of the last message.
    #[field(label = "Last message")]
    pub last_message: Option<&'a str>,
    /// Messages read.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A burst of a telephone FSK modem was read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModemMessageEvent<'a> {
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The standard.
    #[field(label = "Standard")]
    pub standard: &'a str,
    /// The sideband: upper or lower.
    #[field(label = "Sideband")]
    pub sideband: &'a str,
    /// The mark and space tones.
    #[field(label = "Tones")]
    pub tones: &'a str,
    /// The bytes, as hex.
    #[field(label = "Bytes")]
    pub bytes: &'a str,
    /// Framing errors.
    #[field(label = "Framing errors")]
    pub framing_errors: i64,
    /// The length of the burst.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
}

/// A message of a telephone FSK modem.
pub static MODEM_MESSAGE: EventKind<ModemMessageEvent<'static>> =
    EventKind::new("modem.message", "Message");

/// The result fields of the UVB-76 buzzer monitor. The measures of the train are absent while no train is locked.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Uvb76Fields<'a> {
    /// What the stack does not do and what no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The state: 0 no carrier, 1 carrier, 2 buzz train locked.
    #[field(label = "State")]
    pub state: i64,
    /// A carrier is in the channel.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// Buzzes heard.
    #[field(label = "Buzzes")]
    pub buzzes: i64,
    /// The buzz rate, 60 over the median period.
    #[field(label = "Buzz rate", unit = "/min")]
    pub rate_per_min: Option<f64>,
    /// The median buzz length.
    #[field(label = "Buzz length", unit = "s")]
    pub buzz_s: Option<Seconds>,
    /// The median period.
    #[field(label = "Period", unit = "s")]
    pub period_s: Option<Seconds>,
    /// The median distance of the periods from their median.
    #[field(label = "Period spread", unit = "ms")]
    pub spread_ms: Option<f64>,
    /// Train breaks logged.
    #[field(label = "Breaks")]
    pub breaks: i64,
    /// Long transmissions logged.
    #[field(label = "Long transmissions")]
    pub long_transmissions: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A buzz train, a break or a long transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Uvb76Event<'a> {
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Buzz train, Train break or Long transmission.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The value: the rate, the time since the last buzz or the length.
    #[field(label = "Value")]
    pub value: &'a str,
}

/// A buzz train, a break or a long transmission.
pub static UVB76_EVENT: EventKind<Uvb76Event<'static>> = EventKind::new("uvb76.event", "Event");

/// The result fields of the VLF MSK monitor. The measures are absent while no transmitter is on.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VlfMskFields<'a> {
    /// What the stack does not do and what no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The state: 0 off air or too weak, 1 carrier only, 2 MSK.
    #[field(label = "State")]
    pub state: i64,
    /// The class: "MSK 200 bit/s", "MSK about N bit/s", "carrier only" or "off air or too weak".
    #[field(label = "Class")]
    pub signal_class: &'a str,
    /// The carrier from the tuned frequency.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_hz: Option<Hertz>,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Option<f64>,
    /// The spacing of the two lines of the squared signal.
    #[field(label = "Line spacing", unit = "Hz")]
    pub line_spacing_hz: Option<Hertz>,
    /// The level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: Option<f64>,
    /// The listed station near the carrier: "likely NAME".
    #[field(label = "Station hint")]
    pub hint: Option<&'a str>,
    /// Transmitters that came on.
    #[field(label = "Transmitters")]
    pub transmitters: i64,
    /// The level of the channel, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A transmitter came on, went off or changed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VlfTransmitterEvent<'a> {
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// on, off or changed.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub rate: Option<f64>,
    /// The carrier from the tuned frequency.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier: Option<Hertz>,
    /// The level over the noise.
    #[field(label = "Level", unit = "dB")]
    pub level: Option<f64>,
    /// The listed station near the carrier.
    #[field(label = "Station hint")]
    pub hint: Option<&'a str>,
}

/// A transmitter came on, went off or changed.
pub static VLF_TRANSMITTER: EventKind<VlfTransmitterEvent<'static>> =
    EventKind::new("vlf.transmitter", "Transmitter");

/// A serial-tone modem `id`: its fields, the drop count of its decoder, the D_PDUs, its two events `event` and
/// `message`, and the `clear` command. The board holds the text and the D_PDUs received, so it keeps its results over a
/// change of frequency.
fn serial(id: &'static str, event: EventShape, message: EventShape) -> StackShape {
    StackShape::new(id)
        .fields::<SerialFields>()
        .fields::<DecoderFields>()
        .table(&DPDUS)
        .event(event)
        .event(message)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["dpdus", "SerialFields"])
}

/// A teleprinter stack `id`: its fields, the drop count of its decoder, and the events of every teleprinter stack. The
/// stack adds its own tables and events, its settings and the commands.
fn teleprinter(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<TeleprinterFields>()
        .fields::<DecoderFields>()
        .event(&FSK_LOCK)
        .event(&FSK_LOST)
        .event(&TEXT)
}

/// The commands of a teleprinter stack, and what it keeps over a change of frequency: the decoder holds the text that
/// was received, so it keeps its results.
fn teleprinter_end(s: StackShape, kept: &'static [&'static str]) -> StackShape {
    s.command(&sdrkit_graph::decoder::CLEAR)
        .command(&crate::teleprinter::RESET)
        .kept(kept)
}

/// The stacks that share `src/senders.rs`.
const SENDER_STACKS: &[&str] = &[
    "crowd36",
    "xpa",
    "fsk200",
    "gwfsk",
    "fsk_modems",
    "uvb76",
    "vlf_msk",
];

/// A sender stack: the commands `clear` and `reset`, and what it keeps over a change of frequency (its rows and its text).
fn sender(s: StackShape, kept: &'static [&'static str]) -> StackShape {
    s.command(&sdrkit_graph::decoder::CLEAR)
        .command(&crate::teleprinter::RESET)
        .kept(kept)
}

/// What version 1.4 added: the radar bands of the ocean radar stack and `band.tune`.
const ADDED_1_4: &[&str] = &[
    "hfradar.table.bands",
    "shape.HfradarBandRow",
    "shape.HfradarBandRow.name",
    "shape.HfradarBandRow.low",
    "shape.HfradarBandRow.high",
    "shape.HfradarBandRow.middle",
    "shape.HfradarBandRow.range_class",
    "hfradar.kept.bands",
    "hfradar.command.band.tune",
];

/// What version 1.3 added: the scanning call, the three-way handshake, LQA and AMD of ALE; the power steps, the start
/// in the interval, the count and `send` of the own beacon.
const ADDED_1_3: &[&str] = &[
    "shape.AleTxFields.tx_link",
    "shape.AleTxFields.tx_link_station",
    "shape.AleTxFields.tx_link_reason",
    "ale.event.ale.link",
    "shape.AleLinkEvent",
    "shape.AleLinkEvent.state",
    "shape.AleLinkEvent.source",
    "shape.AleLinkEvent.reason",
    "shape.AleLinkEvent.family",
    "ale.table.lqa",
    "shape.AleLqaRow",
    "shape.AleLqaRow.address",
    "shape.AleLqaRow.dial",
    "shape.AleLqaRow.lqa_ber",
    "shape.AleLqaRow.sinad_db",
    "shape.AleLqaRow.multipath",
    "shape.AleLqaRow.reported_ber",
    "shape.AleLqaRow.reported_sinad_db",
    "shape.AleLqaRow.reported_multipath",
    "shape.AleLqaRow.heard",
    "shape.AleLqaRow.last_heard",
    "ale.table.messages",
    "shape.AleMessageRow",
    "shape.AleMessageRow.start",
    "shape.AleMessageRow.time",
    "shape.AleMessageRow.dial",
    "shape.AleMessageRow.from",
    "shape.AleMessageRow.to",
    "shape.AleMessageRow.text",
    "ale.kept.lqa",
    "ale.kept.messages",
    "ale.setting.scan_channels",
    "ale.setting.scan_dwell_s",
    "ale.setting.auto_answer",
    "ale.setting.lqa",
    "ale.setting.answer_wait_s",
    "ale.command.amd",
    "shape.HfbTxFields.tx_duty_pct",
    "hfbeacons.setting.power_steps",
    "hfbeacons.setting.offset_s",
    "hfbeacons.setting.count",
    "hfbeacons.command.send",
];

/// What version 1.2 added: the transmitters of ALE and of the own beacon.
const ADDED_1_2: &[&str] = &[
    "ale.fields.AleTxFields",
    "shape.AleTxFields",
    "shape.AleTxFields.tx_sending",
    "shape.AleTxFields.tx_kind",
    "shape.AleTxFields.tx_to",
    "shape.AleTxFields.tx_word",
    "shape.AleTxFields.tx_words",
    "shape.AleTxFields.tx_note",
    "ale.event.ale.tx",
    "shape.AleTxEvent",
    "shape.AleTxEvent.state",
    "shape.AleTxEvent.reason",
    "shape.AleTxEvent.kind",
    "shape.AleTxEvent.to",
    "shape.AleTxEvent.family",
    "ale.setting.self_address",
    "ale.setting.call_repeats",
    "ale.setting.sounding",
    "ale.command.call",
    "ale.command.sound",
    "ale.command.stop",
    "hfbeacons.fields.HfbTxFields",
    "shape.HfbTxFields",
    "shape.HfbTxFields.tx_beacon_on",
    "shape.HfbTxFields.tx_sending",
    "shape.HfbTxFields.tx_progress_pct",
    "shape.HfbTxFields.tx_next_s",
    "shape.HfbTxFields.tx_sendings",
    "shape.HfbTxFields.tx_sending_s",
    "shape.HfbTxFields.tx_note",
    "hfbeacons.event.hfb.tx",
    "shape.HfbTxEvent",
    "shape.HfbTxEvent.state",
    "shape.HfbTxEvent.reason",
    "shape.HfbTxEvent.text",
    "shape.HfbTxEvent.family",
    "hfbeacons.setting.call",
    "hfbeacons.setting.message",
    "hfbeacons.setting.wpm",
    "hfbeacons.setting.pitch_hz",
    "hfbeacons.setting.long_dash_s",
    "hfbeacons.setting.interval_min",
    "hfbeacons.command.start",
    "hfbeacons.command.stop",
];

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("hfdata", 1, 4)
            .added(1, 4, ADDED_1_4)
            .added(1, 3, ADDED_1_3)
            .added(1, 2, ADDED_1_2)
            .shared_source("senders", SENDER_STACKS)
            .stack(sender(
                StackShape::new("crowd36")
                    .fields::<Crowd36Fields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&TEXT)
                    .event(&TONES_FOUND)
                    .event(&TONES_LOST)
                    .settings(Crowd36Setting::SETTINGS),
                &["rows", "Crowd36Fields.text"],
            ))
            .stack(sender(
                StackShape::new("xpa")
                    .fields::<XpaFields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&TEXT)
                    .event(&TONES_FOUND)
                    .event(&TONES_LOST)
                    .settings(XpaSetting::SETTINGS),
                &["rows", "XpaFields.text"],
            ))
            .stack(sender(
                StackShape::new("fsk200")
                    .fields::<Fsk200Fields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&TEXT)
                    .event(&F06_BLOCK)
                    .event(&TONES_FOUND)
                    .event(&TONES_LOST)
                    .settings(Fsk200Setting::SETTINGS),
                &["rows", "Fsk200Fields.text"],
            ))
            .stack(sender(
                StackShape::new("gwfsk")
                    .fields::<GwfskFields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&GW_PACKET)
                    .settings(GwfskSetting::SETTINGS),
                &["rows"],
            ))
            .stack(sender(
                StackShape::new("fsk_modems")
                    .fields::<FskModemsFields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&MODEM_MESSAGE)
                    .settings(FskModemsSetting::SETTINGS),
                &["rows"],
            ))
            .stack(sender(
                StackShape::new("uvb76")
                    .fields::<Uvb76Fields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&UVB76_EVENT)
                    .settings(Uvb76Setting::SETTINGS),
                &["rows"],
            ))
            .stack(sender(
                StackShape::new("vlf_msk")
                    .fields::<VlfMskFields>()
                    .fields::<DecoderFields>()
                    .table(&SENDER_ROWS)
                    .event(&VLF_TRANSMITTER)
                    .settings(VlfMskSetting::SETTINGS),
                &["rows"],
            ))
            .stack(teleprinter_end(
                teleprinter("efr")
                    .table(&EFR_TELEGRAMS)
                    .event(&EFR_TELEGRAM)
                    .settings(EfrSetting::SETTINGS),
                &["telegrams", "TeleprinterFields"],
            ))
            .stack(teleprinter_end(
                teleprinter("gtor")
                    .event(&GTOR_FRAME)
                    .settings(GtorSetting::SETTINGS),
                &["TeleprinterFields"],
            ))
            .stack(teleprinter_end(
                teleprinter("arqe").settings(ArqeSetting::SETTINGS),
                &["TeleprinterFields"],
            ))
            .stack(teleprinter_end(
                teleprinter("autospec").settings(AutospecSetting::SETTINGS),
                &["TeleprinterFields"],
            ))
            .stack(teleprinter_end(
                teleprinter("piccolo")
                    .event(&PICCOLO_PAIRS)
                    .settings(PiccoloSetting::SETTINGS),
                &["TeleprinterFields"],
            ))
            .stack(teleprinter_end(
                teleprinter("coquelet")
                    .event(&COQUELET_PAIRS)
                    .settings(CoqueletSetting::SETTINGS),
                &["TeleprinterFields"],
            ))
            .stack(
                StackShape::new("ale3g")
                    .fields::<Ale3gFields>()
                    .fields::<DecoderFields>()
                    .table(&ALE3G_PDUS)
                    .event(&ALE3G_PDU)
                    .event(&ALE3G_DATAGRAM)
                    .settings(Ale3gSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["pdus", "Ale3gFields"]),
            )
            .stack(
                StackShape::new("s4481")
                    .fields::<S4481Fields>()
                    .fields::<DecoderFields>()
                    .table(&S4481_CHANNELS)
                    .event(&S4481_EVENT)
                    .settings(S4481Setting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["S4481Fields"]),
            )
            .stack(
                StackShape::new("link11")
                    .fields::<Link11Fields>()
                    .fields::<DecoderFields>()
                    .table(&LINK11_MESSAGES)
                    .event(&LINK11_EVENT)
                    .event(&LINK11_MESSAGE)
                    .settings(Link11Setting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["messages", "Link11Fields"]),
            )
            .stack(
                StackShape::new("cis_hf")
                    .fields::<CisHfFields>()
                    .fields::<DecoderFields>()
                    .table(&CIS_HF_MESSAGES)
                    .table(&CIS_HF_CARRIERS)
                    .event(&TEXT)
                    .event(&CIS_MESSAGE)
                    .event(&CIS_IDENT)
                    .settings(CisSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&crate::teleprinter::RESET)
                    .kept(&["messages", "CisHfFields"]),
            )
            .stack(
                StackShape::new("hfid")
                    .fields::<HfidFields>()
                    .table(&HFID_CANDIDATES)
                    .settings(HfidSetting::SETTINGS),
            )
            .stack(
                // The tables hold the calls, stations and words heard: they stay over a change of frequency, with
                // the measures of the same component.
                StackShape::new("ale")
                    .fields::<AleFields>()
                    .table(&ALE_EVENTS)
                    .table(&ALE_STATIONS)
                    .table(&ALE_WORDS)
                    .table(&ALE_LEARNED_NETS)
                    .table(&ALE_LQA)
                    .table(&ALE_MESSAGES)
                    .fields::<AleTxFields>()
                    .event(&ALE_CALL)
                    .event(&ALE_TX)
                    .event(&ALE_LINK)
                    .settings(AleSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&ALE_REMOVE)
                    .command(&ALE_CALL_TX)
                    .command(&ALE_SOUND)
                    .command(&ALE_STOP)
                    .command(&ALE_AMD)
                    .kept(&[
                        "events",
                        "stations",
                        "words",
                        "learned_nets",
                        "lqa",
                        "messages",
                        "AleFields",
                    ]),
            )
            .stack(
                // The radars and the log of radars heard stay over a change of frequency, with the measures of the
                // same component.
                StackShape::new("hfradar")
                    .fields::<HfradarFields>()
                    .table(&HFRADAR_RADARS)
                    .table(&HFRADAR_HEARD)
                    .series(SeriesDef {
                        key: crate::radar::radars::MARKS_SERIES,
                        label: "Sweep marks",
                        x_unit: "s",
                        y_unit: "Hz",
                    })
                    .series(SeriesDef {
                        key: crate::radar::radars::SNR_SERIES,
                        label: "SNR history",
                        x_unit: "s",
                        y_unit: "dB",
                    })
                    .table(&HFRADAR_BANDS)
                    .event(&HFRADAR_RADAR_HEARD)
                    .settings(RadarStackSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&HFRADAR_BAND_TUNE)
                    .kept(&["radars", "heard", "bands", "HfradarFields"]),
            )
            .stack(pactorhf("pactor2").settings(Pactor2Setting::SETTINGS))
            .stack(pactorhf("pactor3").settings(Pactor3Setting::SETTINGS))
            .stack(pactorhf("pactor4").settings(Pactor4Setting::SETTINGS))
            .stack(
                // The frames, the stations and the text stay over a change of frequency, with the measures of the
                // same component.
                StackShape::new("pactor1")
                    .fields::<Pactor1Fields>()
                    .table(&PACTOR1_FRAMES)
                    .table(&PACTOR1_STATIONS)
                    .series(SeriesDef {
                        key: crate::pactor1::link::LEVEL_SERIES,
                        label: "Level",
                        x_unit: "s",
                        y_unit: "dB",
                    })
                    .event(&PACTOR_FRAME)
                    .settings(Pactor1Setting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&PACTOR1_REMOVE)
                    .kept(&["frames", "stations", "Pactor1Fields"]),
            )
            .stack(
                // The beacon list with its history, the bands and the propagation are a database of what was heard:
                // they stay over a change of frequency, with the measures of the same component.
                StackShape::new("hfbeacons")
                    .fields::<HfbeaconsFields>()
                    .table(&HFBEACONS_BEACONS)
                    .table(&HFBEACONS_BANDS)
                    .table(&HFBEACONS_PROPAGATION)
                    .points("Beacons heard")
                    .fields::<HfbTxFields>()
                    .event(&HFB_DETECTION)
                    .event(&HFB_TX)
                    .settings(BeaconSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&crate::beacons::rx::RESTORE)
                    .command(&HFB_START)
                    .command(&HFB_STOP)
                    .command(&HFB_SEND)
                    .kept(&["beacons", "bands", "propagation", "HfbeaconsFields"]),
            )
            .stack(
                // The measures, the bits, the vectors and the system words describe the buoy on the present
                // frequency: nothing is kept over a change of frequency.
                StackShape::new("wavebuoy")
                    .fields::<WavebuoyFields>()
                    .table(&WAVEBUOY_SPECTRUM)
                    .table(&WAVEBUOY_SYSTEM_WORDS)
                    .table(&WAVEBUOY_FRAMES)
                    .series(SeriesDef {
                        key: crate::buoy::results::SCATTER_SERIES,
                        label: "Symbol means",
                        x_unit: "symbol",
                        y_unit: "Hz",
                    })
                    .series(SeriesDef {
                        key: crate::buoy::results::HEAVE_SERIES,
                        label: "Heave",
                        x_unit: "sample",
                        y_unit: "cm",
                    })
                    .series(SeriesDef {
                        key: crate::buoy::results::NORTH_SERIES,
                        label: "North",
                        x_unit: "sample",
                        y_unit: "cm",
                    })
                    .series(SeriesDef {
                        key: crate::buoy::results::WEST_SERIES,
                        label: "West",
                        x_unit: "sample",
                        y_unit: "cm",
                    })
                    .series(SeriesDef {
                        key: crate::buoy::rx::SPECTRUM_SERIES,
                        label: "Spectrum",
                        x_unit: "column",
                        y_unit: "",
                    })
                    .event(&WAVEBUOY_VECTOR)
                    .settings(BuoySetting::SETTINGS)
                    .command(&crate::buoy::results::CLEAR),
            )
            .stack(
                serial("ms110", (&MS110_EVENT).into(), (&MS110_MESSAGE).into())
                    .settings(Ms110Setting::SETTINGS),
            )
            .stack(
                serial("s4285", (&S4285_EVENT).into(), (&S4285_MESSAGE).into())
                    .settings(S4285Setting::SETTINGS),
            )
            .stack(
                serial("s4539", (&S4539_EVENT).into(), (&S4539_MESSAGE).into())
                    .settings(S4539Setting::SETTINGS),
            )
            .added_in(1, 1, |f| f.stack(rpr()))
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
        .unwrap_or_else(|| panic!("the hfdata family does not describe the stack {id}"))
}
