//! The interface of the trunking family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types and what each stack declares.

use crate::edacs::calls::{FollowState, Polarity};
use crate::follow::{FollowAction, FollowReason};
use crate::ltr::code::WordKind;
use crate::mpt::msg::MptType;
use crate::mpt::rx::ChannelUse;
use crate::multitone::analysis::{BurstKind, TxKind};
use crate::opensky::Verdict;
use crate::pager::tables::{PageType, PagerProtocol};
use crate::tones::events::ToneKind;
use crate::tones::front::Demod;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::calls::CallEnd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

/// A follow decision. `source` is the caller, `target` the talkgroup or unit, `channel` the channel number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FollowEvent<'a> {
    /// The action of a follow decision.
    #[field(label = "Action")]
    pub action: FollowAction,
    /// The reason of a skip or a release.
    #[field(label = "Reason")]
    pub reason: Option<FollowReason>,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of MPT 1327.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mpt1327Fields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// Codewords good.
    #[field(label = "Codewords good")]
    pub codewords_good: i64,
    /// Codewords repaired.
    #[field(label = "Codewords repaired")]
    pub codewords_repaired: i64,
    /// Codewords bad.
    #[field(label = "Codewords bad")]
    pub codewords_bad: i64,
    /// System codewords.
    #[field(label = "System codewords")]
    pub ccscs: i64,
    /// The messages read.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The system identity of the control channel.
    #[field(label = "System identity")]
    pub system_identity: Option<i64>,
    /// Sync check off.
    #[field(label = "Sync check off")]
    pub sync_check_off: bool,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// An MPT 1327 message. The row key is the sequence number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mpt1327MessageRow<'a> {
    /// The signal time of a message, s.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The type of a message.
    #[field(label = "Type")]
    pub r#type: MptType,
    /// The calling address.
    #[field(label = "Caller")]
    pub caller: &'a str,
    /// The called address.
    #[field(label = "Called")]
    pub called: &'a str,
    /// A channel number.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// What a message means, for people.
    #[field(label = "Meaning")]
    pub meaning: &'a str,
}

/// An MPT 1327 channel. The row key is the channel number, five digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mpt1327ChannelRow<'a> {
    /// A channel number.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The frequency of a channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// What a channel is used for.
    #[field(label = "Use")]
    pub r#use: ChannelUse,
    /// The messages that named a channel.
    #[field(label = "Seen")]
    pub seen: i64,
    /// The last called address on a traffic channel.
    #[field(label = "Last called")]
    pub last_called: Option<&'a str>,
    /// The last calling address on a traffic channel.
    #[field(label = "Last caller")]
    pub last_caller: Option<&'a str>,
}

/// An address codeword: `source` is the caller, `target` the called address, `channel` the channel named, `text` the meaning.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mpt1327MessageEvent<'a> {
    /// The type of a message.
    #[field(label = "Type")]
    pub r#type: MptType,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of LTR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LtrFields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The good words.
    #[field(label = "Good words")]
    pub words_good: i64,
    /// Bad words.
    #[field(label = "Bad words")]
    pub words_bad: i64,
    /// Syncs.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// The AREA bit.
    #[field(label = "Area")]
    pub area: Option<i64>,
    /// Repeaters seen.
    #[field(label = "Repeaters seen")]
    pub repeaters: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Data swing.
    #[field(label = "Data swing", unit = "Hz")]
    pub swing_hz: Hertz,
    /// Words heard.
    #[field(label = "Words heard")]
    pub data_valid: bool,
}

/// A good LTR word. The row key is the sequence number of the word.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LtrWordRow<'a> {
    /// The talkgroup, "{AREA}-{HOME}-{ID}".
    #[field(label = "Talkgroup")]
    pub talkgroup: &'a str,
    /// The AREA bit.
    #[field(label = "Area")]
    pub area: i64,
    /// The GO-TO repeater.
    #[field(label = "Go-to")]
    pub goto: i64,
    /// The HOME repeater.
    #[field(label = "Home")]
    pub home: i64,
    /// The ID.
    #[field(label = "Id")]
    pub id: i64,
    /// The FREE repeater.
    #[field(label = "Free")]
    pub free: i64,
    /// The 7 check bits as received.
    #[field(label = "Check")]
    pub check: i64,
    /// What the ID says.
    #[field(label = "Kind")]
    pub kind: WordKind,
    /// The signal time of a word, s.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
}

/// How often a repeater was named in each field. The row key is the repeater.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LtrRepeaterRow {
    /// A repeater number.
    #[field(label = "Repeater")]
    pub repeater: i64,
    /// Times a repeater was named as go-to.
    #[field(label = "As go-to")]
    pub as_goto: i64,
    /// Times named as home.
    #[field(label = "As home")]
    pub as_home: i64,
    /// Times named as free.
    #[field(label = "As free")]
    pub as_free: i64,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A good LTR word: `target` is the talkgroup.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LtrWordEvent<'a> {
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The AREA bit.
    #[field(label = "Area")]
    pub area: i64,
    /// The GO-TO repeater.
    #[field(label = "Go-to")]
    pub goto: i64,
    /// The HOME repeater.
    #[field(label = "Home")]
    pub home: i64,
    /// The ID.
    #[field(label = "Id")]
    pub id: i64,
    /// The FREE repeater.
    #[field(label = "Free")]
    pub free: i64,
    /// What the ID says.
    #[field(label = "Kind")]
    pub kind: WordKind,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of EDACS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsFields {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The data polarity.
    #[field(label = "Data polarity")]
    pub polarity: Polarity,
    /// The site.
    #[field(label = "Site")]
    pub site: Option<i64>,
    /// The area.
    #[field(label = "Area")]
    pub area: Option<i64>,
    /// The control channel's LCN.
    #[field(label = "Control channel LCN")]
    pub control_lcn: Option<i64>,
    /// The highest LCN seen in a message.
    #[field(label = "Highest LCN")]
    pub highest_lcn: Option<i64>,
    /// Words.
    #[field(label = "Words")]
    pub words: i64,
    /// Words of a known type.
    #[field(label = "Words of a known type")]
    pub words_known: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Data swing.
    #[field(label = "Data swing", unit = "Hz")]
    pub swing_hz: Hertz,
    /// The LCN followed now.
    #[field(label = "Followed LCN")]
    pub following_lcn: Option<i64>,
    /// What the follower does now.
    #[field(label = "Following")]
    pub following: FollowState,
}

/// An EDACS call. `source` is the calling unit, `target` the group or the called unit, `channel` the LCN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsCallRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// A group call.
    #[field(label = "Group call")]
    pub group: bool,
    /// The start of a call, signal time.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// The length of a call.
    #[field(label = "Length", unit = "s")]
    pub duration: Seconds,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// A digital call (not decoded).
    #[field(label = "Digital")]
    pub digital: bool,
    /// A call that no message announced.
    #[field(label = "Carrier only")]
    pub carrier_only: bool,
    /// Where a call is in its life.
    #[field(label = "End")]
    pub end: CallEnd,
    /// The call is followed now.
    #[field(label = "Followed")]
    pub followed: bool,
}

/// An EDACS talkgroup. The row key is the group id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsTalkgroupRow {
    /// A group.
    #[field(label = "Group")]
    pub gid: i64,
    /// Calls.
    #[field(label = "Calls")]
    pub calls: i64,
    /// Last unit.
    #[field(label = "Last unit")]
    pub last_unit: Option<i64>,
    /// Last LCN.
    #[field(label = "Last LCN")]
    pub last_lcn: i64,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// On air.
    #[field(label = "On air")]
    pub active: bool,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// An EDACS channel. The row key is the LCN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsChannelRow {
    /// A voice channel number.
    #[field(label = "LCN")]
    pub lcn: i64,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// Level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// Floor.
    #[field(label = "Floor", unit = "dB")]
    pub floor_db: f64,
    /// In the capture.
    #[field(label = "In the capture")]
    pub in_capture: bool,
    /// Carrier.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// Control channel.
    #[field(label = "Control channel")]
    pub control: bool,
}

/// An EDACS frame. The row key is the sequence number of the frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsFrameRow<'a> {
    /// Message A.
    #[field(label = "Message A")]
    pub a: i64,
    /// Message B.
    #[field(label = "Message B")]
    pub b: i64,
    /// Copy errors of A.
    #[field(label = "Copy errors of A")]
    pub err_a: i64,
    /// Copy errors of B.
    #[field(label = "Copy errors of B")]
    pub err_b: i64,
    /// Time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// Read as.
    #[field(label = "Read as")]
    pub text: &'a str,
}

/// A message type and how often it was heard. The row key is the type.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsWordRow {
    /// Word.
    #[field(label = "Word")]
    pub word: i64,
    /// Count.
    #[field(label = "Count")]
    pub count: i64,
}

/// An EDACS call that started or ended.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EdacsCallEvent<'a> {
    /// Where a call is in its life.
    #[field(label = "End")]
    pub end: CallEnd,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// A group call.
    #[field(label = "Group call")]
    pub group: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// A call that no message announced.
    #[field(label = "Carrier only")]
    pub carrier_only: bool,
    /// The length of a call.
    #[field(label = "Length", unit = "s")]
    pub duration: Seconds,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of the Multitone detector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneFields {
    /// Carrier.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// The transmissions measured.
    #[field(label = "Transmissions")]
    pub transmissions: i64,
    /// Bursts.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Valid BCH words.
    #[field(label = "Valid BCH words")]
    pub valid_words: i64,
    /// Other words.
    #[field(label = "Other words")]
    pub other_words: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A transmission. The row key is its number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneTransmissionRow {
    /// Time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// Duration.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// SNR.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// Deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Hertz,
    /// Kind.
    #[field(label = "Kind")]
    pub kind: TxKind,
    /// Period.
    #[field(label = "Period", unit = "ms")]
    pub period_ms: Option<f64>,
    /// Burst.
    #[field(label = "Burst")]
    pub burst: Option<i64>,
}

/// A tone of a transmission. The row key is the transmission, `/` and the place of the tone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneToneRow<'a> {
    /// The transmission a row belongs to: a row key of [`TX_TABLE`].
    #[field(label = "Transmission", refers_to = "transmissions")]
    pub transmission: &'a str,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// Length.
    #[field(label = "Length", unit = "s")]
    pub seconds: Seconds,
    /// Start in the transmission.
    #[field(label = "Start in the transmission", unit = "s")]
    pub start: Seconds,
}

/// A spectral line of a transmission. The row key is the transmission, `/` and the place of the line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneLineRow<'a> {
    /// The transmission a row belongs to: a row key of [`TX_TABLE`].
    #[field(label = "Transmission", refers_to = "transmissions")]
    pub transmission: &'a str,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// Above the median.
    #[field(label = "Above the median", unit = "dB")]
    pub db: f64,
}

/// A tone heard and how often. The row key is the tone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneHeardRow {
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub hz: Hertz,
    /// Count.
    #[field(label = "Count")]
    pub count: i64,
    /// Length.
    #[field(label = "Length", unit = "s")]
    pub seconds: Seconds,
    /// Last.
    #[field(label = "Last", unit = "s")]
    pub last_s: f64,
}

/// An FSK burst. The row key is its number, eight digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneBurstRow<'a> {
    /// Burst.
    #[field(label = "Burst")]
    pub burst: i64,
    /// Kind.
    #[field(label = "Kind")]
    pub kind: BurstKind,
    /// Baud.
    #[field(label = "Baud")]
    pub baud: f64,
    /// Bits.
    #[field(label = "Bits")]
    pub bits: i64,
    /// Valid BCH words.
    #[field(label = "Valid BCH words")]
    pub bch_valid: i64,
    /// Words.
    #[field(label = "Words")]
    pub bch_total: i64,
    /// Verdict.
    #[field(label = "Verdict")]
    pub verdict: &'a str,
}

/// A finished transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneTransmissionEvent<'a> {
    /// Kind.
    #[field(label = "Kind")]
    pub kind: TxKind,
    /// Duration.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// SNR.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// Deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Hertz,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A finished FSK burst.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultitoneBurstEvent<'a> {
    /// Kind.
    #[field(label = "Kind")]
    pub kind: BurstKind,
    /// Baud.
    #[field(label = "Baud")]
    pub baud: f64,
    /// Bits.
    #[field(label = "Bits")]
    pub bits: i64,
    /// Valid BCH words.
    #[field(label = "Valid BCH words")]
    pub bch_valid: i64,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of Motorola Type II.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2Fields<'a> {
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The state: 0 no input, 1 listening, 2 a good word in the last 5 s.
    #[field(label = "Decoder state")]
    pub rx_state: i64,
    /// Good words.
    #[field(label = "Good words")]
    pub good_words: i64,
    /// Bad words.
    #[field(label = "Bad words")]
    pub bad_words: i64,
    /// Information bits repaired.
    #[field(label = "Bits repaired")]
    pub bits_repaired: i64,
    /// Messages.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Messages understood.
    #[field(label = "Messages known")]
    pub messages_known: i64,
    /// A system ID.
    #[field(label = "System")]
    pub system: Option<&'a str>,
    /// The site ID.
    #[field(label = "Site")]
    pub site_id: Option<i64>,
    /// Feature bits, as text.
    #[field(label = "Features")]
    pub features: Option<&'a str>,
    /// A control channel frequency.
    #[field(label = "Control channel", unit = "Hz")]
    pub control_channel_hz: Option<Hertz>,
    /// Type II or Type I.
    #[field(label = "System type")]
    pub system_type: Option<&'a str>,
    /// SmartZone or SmartNet.
    #[field(label = "Network")]
    pub network: &'a str,
    /// Inverted polarity.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The data swing.
    #[field(label = "Data swing", unit = "Hz")]
    pub swing_hz: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The plan in use.
    #[field(label = "Channel plan")]
    pub plan: Option<&'a str>,
    /// A note.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A Type II message. The row key is the sequence number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2MessageRow<'a> {
    /// Signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// An address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// A group word.
    #[field(label = "Group")]
    pub group: bool,
    /// A command.
    #[field(label = "Command")]
    pub command: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A Type II talkgroup. The row key is the talkgroup.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2TalkgroupRow<'a> {
    /// A talkgroup.
    #[field(label = "Talkgroup")]
    pub talkgroup: &'a str,
    /// Grants counted.
    #[field(label = "Calls")]
    pub calls: i64,
    /// The last calling unit.
    #[field(label = "Last source")]
    pub last_source: Option<i64>,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// The last time.
    #[field(label = "Last", unit = "s")]
    pub last_s: f64,
    /// Active.
    #[field(label = "Active")]
    pub active: bool,
    /// Digital.
    #[field(label = "Digital")]
    pub digital: bool,
    /// Encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// Patched.
    #[field(label = "Patched")]
    pub patched: bool,
}

/// A Type II site. The row key is the system and the site.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2SiteRow<'a> {
    /// A system ID.
    #[field(label = "System")]
    pub system: &'a str,
    /// A site.
    #[field(label = "Site")]
    pub site: i64,
    /// A band code.
    #[field(label = "Band")]
    pub band: i64,
    /// Feature bits, as text.
    #[field(label = "Features")]
    pub features: &'a str,
    /// A control channel frequency.
    #[field(label = "Control channel", unit = "Hz")]
    pub control_channel_hz: Option<Hertz>,
    /// An adjacent site.
    #[field(label = "Adjacent")]
    pub adjacent: bool,
}

/// A member of a patch. The row key is the talkgroup and the member.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2PatchRow<'a> {
    /// A talkgroup.
    #[field(label = "Talkgroup")]
    pub talkgroup: &'a str,
    /// A patch member.
    #[field(label = "Member")]
    pub member: &'a str,
    /// The last time.
    #[field(label = "Last", unit = "s")]
    pub last_s: f64,
}

/// A Type II call. The row key is the talkgroup.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2CallRow<'a> {
    /// A talkgroup.
    #[field(label = "Talkgroup")]
    pub talkgroup: &'a str,
    /// The source of a call.
    #[field(label = "Source")]
    pub source_unit: i64,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// Digital.
    #[field(label = "Digital")]
    pub digital: bool,
    /// Emergency.
    #[field(label = "Emergency")]
    pub emergency: bool,
    /// Encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The start of a call.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// The length of a call.
    #[field(label = "Duration", unit = "s")]
    pub duration_s: f64,
    /// The state of a call.
    #[field(label = "State")]
    pub state: &'a str,
    /// A note.
    #[field(label = "Note")]
    pub note: Option<&'a str>,
}

/// A Type II message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MotorolaType2MessageEvent<'a> {
    /// An address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// A group word.
    #[field(label = "Group")]
    pub group: bool,
    /// A command.
    #[field(label = "Command")]
    pub command: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A voice grant: `target` is the talkgroup, `source` the caller, `channel` the channel number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GrantEvent<'a> {
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// A group word.
    #[field(label = "Group")]
    pub group: bool,
    /// Emergency.
    #[field(label = "Emergency")]
    pub emergency: bool,
    /// Encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// Digital.
    #[field(label = "Digital")]
    pub digital: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of the OpenSky identifier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OpenskyFields<'a> {
    /// The state: 0 no input, 1 analysing, 2 a result.
    #[field(label = "Decoder state")]
    pub rx_state: i64,
    /// Blocks analysed.
    #[field(label = "Blocks analysed")]
    pub blocks: i64,
    /// Symbol rate found, Hz.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The strength of the symbol rate line.
    #[field(label = "Line strength", unit = "dB")]
    pub line_db: Option<f64>,
    /// The levels.
    #[field(label = "Levels")]
    pub levels: Option<i64>,
    /// The deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<f64>,
    /// The burst period.
    #[field(label = "Burst period", unit = "ms")]
    pub burst_ms: Option<f64>,
    /// The duty.
    #[field(label = "Duty")]
    pub duty: Option<f64>,
    /// The occupied width.
    #[field(label = "Occupied width", unit = "Hz")]
    pub width_hz: Option<f64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: Option<Verdict>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The measures of one block. The row key is the sequence number of the block.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OpenskyBlockRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// Symbol rate found, Hz.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The levels.
    #[field(label = "Levels")]
    pub levels: i64,
    /// The occupied width.
    #[field(label = "Occupied width", unit = "Hz")]
    pub width_hz: f64,
    /// The burst period.
    #[field(label = "Burst period", unit = "ms")]
    pub burst_ms: f64,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: Verdict,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The verdict of one block.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OpenskyVerdictEvent<'a> {
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: Verdict,
    /// Symbol rate found, Hz.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The levels.
    #[field(label = "Levels")]
    pub levels: i64,
    /// The deviation.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: f64,
    /// The burst period.
    #[field(label = "Burst period", unit = "ms")]
    pub burst_ms: f64,
    /// The occupied width.
    #[field(label = "Occupied width", unit = "Hz")]
    pub width_hz: f64,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A follow decision of a trunking stack.
pub static TRUNK_FOLLOW: EventKind<FollowEvent<'static>> =
    EventKind::new("trunk.follow", "Follow decision");
/// A voice grant.
pub static TRUNK_GRANT: EventKind<GrantEvent<'static>> =
    EventKind::new("trunk.grant", "Voice grant");

/// The MPT 1327 messages, oldest first.
pub static MPT1327_MESSAGES: Table<Mpt1327MessageRow<'static>> =
    Table::new("messages", "Messages", 200);
/// The MPT 1327 channel table, by channel number.
pub static MPT1327_CHANNELS: Table<Mpt1327ChannelRow<'static>> =
    Table::new("channels", "Channels", 2048);
/// An MPT 1327 address codeword.
pub static MPT1327_MESSAGE: EventKind<Mpt1327MessageEvent<'static>> =
    EventKind::new("mpt1327.message", "MPT 1327 message");

/// The good LTR words, oldest first.
pub static LTR_WORDS: Table<LtrWordRow<'static>> =
    Table::new("words", "Words", 400).label(&LtrWordRow::TALKGROUP);
/// The LTR repeaters.
pub static LTR_REPEATERS: Table<LtrRepeaterRow> = Table::new("repeaters", "Repeaters", 20);
/// A good LTR word.
pub static LTR_WORD: EventKind<LtrWordEvent<'static>> = EventKind::new("ltr.word", "LTR word");

/// The EDACS calls, oldest first.
pub static EDACS_CALLS: Table<EdacsCallRow<'static>> =
    Table::new("calls", "Calls", 200).label(&EdacsCallRow::SOURCE);
/// The EDACS talkgroups.
pub static EDACS_TALKGROUPS: Table<EdacsTalkgroupRow> = Table::new("talkgroups", "Talkgroups", 200);
/// The EDACS channels.
pub static EDACS_CHANNELS: Table<EdacsChannelRow> = Table::new("channels", "Channels", 64);
/// The EDACS frames, oldest first.
pub static EDACS_FRAMES: Table<EdacsFrameRow<'static>> = Table::new("frames", "Frames", 400);
/// The EDACS message types heard.
pub static EDACS_WORDS: Table<EdacsWordRow> = Table::new("words", "Words", 24);
/// An EDACS call.
pub static EDACS_CALL: EventKind<EdacsCallEvent<'static>> =
    EventKind::new("edacs.call", "EDACS call");

/// The Multitone transmissions.
pub static MULTITONE_TRANSMISSIONS: Table<MultitoneTransmissionRow> =
    Table::new("transmissions", "Transmissions", 200);
/// The tones of the Multitone transmissions.
pub static MULTITONE_TONES: Table<MultitoneToneRow<'static>> =
    Table::new("tx_tones", "Tones of the transmissions", 2000);
/// The spectral lines of the Multitone transmissions.
pub static MULTITONE_LINES: Table<MultitoneLineRow<'static>> =
    Table::new("tx_lines", "Lines of the transmissions", 2000);
/// The tones heard.
pub static MULTITONE_HEARD: Table<MultitoneHeardRow> = Table::new("tones_heard", "Tones heard", 24);
/// The FSK bursts.
pub static MULTITONE_BURSTS: Table<MultitoneBurstRow<'static>> =
    Table::new("bursts", "Bursts", 100);
/// A Multitone transmission.
pub static MULTITONE_TRANSMISSION: EventKind<MultitoneTransmissionEvent<'static>> =
    EventKind::new("multitone.transmission", "Multitone transmission");
/// A Multitone FSK burst.
pub static MULTITONE_BURST: EventKind<MultitoneBurstEvent<'static>> =
    EventKind::new("multitone.burst", "Multitone FSK burst");

/// The Type II messages, oldest first.
pub static MOTOROLA_TYPE2_MESSAGES: Table<MotorolaType2MessageRow<'static>> =
    Table::new("messages", "Messages", crate::type2::tables::MESSAGE_CAP)
        .label(&MotorolaType2MessageRow::ADDRESS);
/// The Type II talkgroups.
pub static MOTOROLA_TYPE2_TALKGROUPS: Table<MotorolaType2TalkgroupRow<'static>> = Table::new(
    "talkgroups",
    "Talkgroups",
    crate::type2::tables::TALKGROUP_CAP,
)
.label(&MotorolaType2TalkgroupRow::TALKGROUP);
/// The Type II sites.
pub static MOTOROLA_TYPE2_SITES: Table<MotorolaType2SiteRow<'static>> =
    Table::new("sites", "Sites", 64);
/// The Type II patches.
pub static MOTOROLA_TYPE2_PATCHES: Table<MotorolaType2PatchRow<'static>> =
    Table::new("patches", "Patches", 256).label(&MotorolaType2PatchRow::TALKGROUP);
/// The Type II calls.
pub static MOTOROLA_TYPE2_CALLS: Table<MotorolaType2CallRow<'static>> =
    Table::new("calls", "Calls", crate::type2::tables::CALL_CAP)
        .label(&MotorolaType2CallRow::TALKGROUP);
/// A Type II message.
pub static MOTOROLA_TYPE2_MESSAGE: EventKind<MotorolaType2MessageEvent<'static>> =
    EventKind::new("type2.message", "Type II message");

/// The OpenSky blocks, oldest first.
pub static OPENSKY_BLOCKS: Table<OpenskyBlockRow<'static>> =
    Table::new("blocks", "Blocks", crate::opensky::ROW_CAP);
/// The OpenSky verdict of a block.
pub static OPENSKY_VERDICT: EventKind<OpenskyVerdictEvent<'static>> =
    EventKind::new("opensky.verdict", "OpenSky verdict");

/// The counts and the carrier of the ERMES receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ErmesFields<'a> {
    /// The good batches.
    #[field(label = "Batches")]
    pub batches: i64,
    /// The batches dropped.
    #[field(label = "Bad batches")]
    pub bad_batches: i64,
    /// The initial addresses.
    #[field(label = "Addresses")]
    pub addresses: i64,
    /// The messages.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The code words repaired.
    #[field(label = "Code words repaired")]
    pub words_repaired: i64,
    /// The times that the preamble and the sync word were found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// The network of the last good batch.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// A carrier is present.
    #[field(label = "Carrier")]
    pub carrier: Option<bool>,
    /// The signal to noise ratio of the carrier.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the ERMES table: the initial addresses of a batch, or a message. The row key is the number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ErmesEntryRow<'a> {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What the row is: the addresses of a batch, or a message.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The batch letter.
    #[field(label = "Batch")]
    pub batch: &'a str,
    /// The local address of a message.
    #[field(label = "Address")]
    pub address: Option<i64>,
    /// The category of a message.
    #[field(label = "Category")]
    pub category: Option<&'a str>,
    /// The addresses, or the text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A good ERMES batch.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ErmesBatchEvent<'a> {
    /// The batch letter.
    #[field(label = "Batch")]
    pub batch: &'a str,
    /// The network.
    #[field(label = "Network")]
    pub network: &'a str,
    /// The initial addresses, decimal.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An ERMES message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ErmesMessageEvent<'a> {
    /// The local address.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The category.
    #[field(label = "Category")]
    pub category: Option<&'a str>,
    /// The text of the message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The ERMES rows: the addresses of each batch and its messages, newest last.
pub static ERMES_ROWS: Table<ErmesEntryRow<'static>> =
    Table::new("rows", "Batches and messages", crate::ermes::ROW_CAP);
/// A good ERMES batch: its network and initial addresses.
pub static ERMES_BATCH: EventKind<ErmesBatchEvent<'static>> =
    EventKind::new("ermes.batch", "ERMES batch");
/// One ERMES message.
pub static ERMES_MESSAGE: EventKind<ErmesMessageEvent<'static>> =
    EventKind::new("ermes.message", "ERMES message");

/// The result fields of the tone decoders.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TonesFields<'a> {
    /// The audio level.
    #[field(label = "Audio level", unit = "dB")]
    pub audio_db: f64,
    /// The signal to noise ratio of the carrier. No value for audio input.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The offset of the carrier from the tuned frequency. No value for audio input.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// A carrier was seen in the last 5 s. No value for audio input.
    #[field(label = "Carrier seen in 5 s")]
    pub carrier_recent: Option<bool>,
    /// The CTCSS tone that is locked now.
    #[field(label = "CTCSS tone", unit = "Hz")]
    pub ctcss_hz: Option<Hertz>,
    /// The DCS code that is locked now.
    #[field(label = "DCS code")]
    pub dcs: Option<&'a str>,
    /// The other reading of the waveform of the DCS code.
    #[field(label = "DCS alias")]
    pub dcs_alias: Option<&'a str>,
    /// The strongest tone of the audio spectrum.
    #[field(label = "Strongest tone", unit = "Hz")]
    pub strongest_hz: Option<Hertz>,
    /// The DTMF keys of the string that goes on now.
    #[field(label = "DTMF keys")]
    pub dtmf: &'a str,
    /// The listening gate is open.
    #[field(label = "Gate open")]
    pub gate_open: bool,
    /// The sample rate of the input.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Hertz,
    /// The input is audio.
    #[field(label = "Audio input")]
    pub audio_input: bool,
    /// The demodulation in use.
    #[field(label = "Demodulation")]
    pub demod: Demod,
    /// Every decision of the decoders.
    #[field(label = "Detections")]
    pub detections: i64,
    /// The CTCSS tones heard.
    #[field(label = "CTCSS tones")]
    pub ctcss_count: i64,
    /// The DCS codes heard.
    #[field(label = "DCS codes")]
    pub dcs_count: i64,
    /// The DTMF strings heard.
    #[field(label = "DTMF strings")]
    pub dtmf_count: i64,
    /// The sequential calls heard.
    #[field(label = "Sequential calls")]
    pub selcall_count: i64,
    /// The SELCAL calls heard.
    #[field(label = "SELCAL calls")]
    pub selcal_count: i64,
    /// The two-tone pages heard.
    #[field(label = "Two-tone pages")]
    pub two_tone_count: i64,
}

/// A row of the list of tones heard. The row key is the sequence number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TonesHeardRow<'a> {
    /// The signal time of the start.
    #[field(label = "Start", unit = "s")]
    pub start_s: f64,
    /// What the row holds.
    #[field(label = "Type")]
    pub kind: ToneKind,
    /// What was heard ("123.0 Hz", "D023N", "159#", "12345", "AM-CP", "682.5 + 1000.0 Hz").
    #[field(label = "Heard")]
    pub text: &'a str,
    /// The length so far.
    #[field(label = "Length", unit = "s")]
    pub length: Seconds,
    /// The details.
    #[field(label = "Details")]
    pub detail: &'a str,
    /// The level, relative to a full-scale sine.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// It still goes on.
    #[field(label = "Active")]
    pub active: bool,
}

/// What the tone decoders heard, oldest first.
pub static TONES_HEARD: Table<TonesHeardRow<'static>> =
    Table::new("events", "Tones heard", crate::tones::events::ROWS);
/// The audio spectrum of the tone stack.
pub const TONES_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Audio spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The result fields of the pager stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerFields<'a> {
    /// A decoder receives a transmission.
    #[field(label = "Receiving")]
    pub receiving: bool,
    /// A carrier is present.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// A decoder holds sync.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The protocol and speed of the last sync ("POCSAG 1200").
    #[field(label = "Last speed")]
    pub last_speed: &'a str,
    /// The offset of the carrier from the tuned frequency at the last sync.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The signal to noise ratio of the carrier, while a carrier is present.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The pages read.
    #[field(label = "Pages")]
    pub pages: i64,
    /// The code words valid as received or repaired.
    #[field(label = "Code words good")]
    pub codewords_good: i64,
    /// The code words repaired.
    #[field(label = "Code words repaired")]
    pub codewords_repaired: i64,
    /// The code words lost.
    #[field(label = "Code words lost")]
    pub codewords_lost: i64,
    /// The pages that the acceptance rules refused (address ranges, vector values, message checks).
    #[field(label = "Pages refused")]
    pub pages_refused: i64,
}

/// A row of the page list. The row key is the sequence number of the page.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerPageRow<'a> {
    /// The signal time of the page.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The protocol and speed ("POCSAG 1200", "FLEX 3200/4", "Selcall ZVEI1").
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The RIC or capcode (0 for a sequential call).
    #[field(label = "Address")]
    pub address: i64,
    /// The POCSAG function code.
    #[field(label = "Function")]
    pub function: Option<i64>,
    /// What the page carries.
    #[field(label = "Type")]
    pub r#type: PageType,
    /// The message.
    #[field(label = "Message")]
    pub message: &'a str,
    /// A word was lost or the end is not sure.
    #[field(label = "Damaged")]
    pub damaged: bool,
    /// The bits that the repairs changed.
    #[field(label = "Bits repaired")]
    pub bits_repaired: i64,
    /// The copies voted into the page.
    #[field(label = "Copies")]
    pub copies: i64,
    /// The frequency of the carrier, when it is known.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
}

/// A row of the statistics of each speed. The row key is the speed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerSpeedRow<'a> {
    /// The protocol and speed.
    #[field(label = "Speed")]
    pub speed: &'a str,
    /// The code words valid as received.
    #[field(label = "Good")]
    pub good: i64,
    /// The code words repaired.
    #[field(label = "Repaired")]
    pub repaired: i64,
    /// The code words lost.
    #[field(label = "Lost")]
    pub lost: i64,
    /// The pages read.
    #[field(label = "Pages")]
    pub pages: i64,
    /// The batches (POCSAG) or frames (FLEX).
    #[field(label = "Batches or frames")]
    pub transmissions: i64,
    /// The pages that the acceptance rules refused.
    #[field(label = "Pages refused")]
    pub refused: i64,
}

/// A row of the addresses heard. The row key is the protocol and the address ("pocsag/1234567").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerHeardRow<'a> {
    /// The RIC or capcode.
    #[field(label = "Address")]
    pub address: i64,
    /// The paging protocol.
    #[field(label = "Protocol")]
    pub protocol: PagerProtocol,
    /// The pages of the address.
    #[field(label = "Pages")]
    pub pages: i64,
    /// The protocol and speed of the last page.
    #[field(label = "Speed")]
    pub speed: &'a str,
    /// The last text (a tone-only page does not erase it).
    #[field(label = "Last text")]
    pub last_text: Option<&'a str>,
    /// When it was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A POCSAG page: `source` is the RIC, `text` the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PocsagPageEvent<'a> {
    /// The RIC.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The function code.
    #[field(label = "Function")]
    pub function: Option<i64>,
    /// The bit rate.
    #[field(label = "Rate", unit = "bit/s")]
    pub baud: Option<i64>,
    /// What the page carries.
    #[field(label = "Type")]
    pub r#type: PageType,
    /// A word was lost or the end is not sure.
    #[field(label = "Damaged")]
    pub damaged: bool,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A FLEX page: `source` is the capcode, `text` the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FlexPageEvent<'a> {
    /// The capcode.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The FLEX speed ("FLEX 3200/4").
    #[field(label = "Speed")]
    pub speed: &'a str,
    /// What the page carries.
    #[field(label = "Type")]
    pub r#type: PageType,
    /// A word was lost or the end is not sure.
    #[field(label = "Damaged")]
    pub damaged: bool,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The page list, oldest first.
pub static PAGER_PAGES: Table<PagerPageRow<'static>> =
    Table::new("pages", "Pages", crate::pager::tables::PAGES_KEPT);
/// The statistics of each speed.
pub static PAGER_SPEEDS: Table<PagerSpeedRow<'static>> = Table::new("speeds", "Speeds", 16);
/// The addresses heard, the most recent first.
pub static PAGER_HEARD: Table<PagerHeardRow<'static>> =
    Table::new("heard", "Addresses heard", 1000);
/// A POCSAG page.
pub static POCSAG_PAGE: EventKind<PocsagPageEvent<'static>> =
    EventKind::new("pocsag.page", "POCSAG page");
/// A FLEX page.
pub static FLEX_PAGE: EventKind<FlexPageEvent<'static>> = EventKind::new("flex.page", "FLEX page");

/// The result fields of the POCSAG sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerTxFields {
    /// Whether a transmission is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The pages of the transmission on air.
    #[field(label = "Pages sent")]
    pub tx_sent: i64,
    /// The pages waiting for the next transmission.
    #[field(label = "Pages waiting")]
    pub tx_waiting: i64,
}

/// The fields of a change of a POCSAG transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PagerTxEvent<'a> {
    /// The state: started, queued, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, halted, the transmitter is off, the format changed, or why a page was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The addresses of the pages, separated by commas.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The text of the page for started, queued and refused; empty for finished.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a pager.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A POCSAG transmission.
pub static PAGER_TX: EventKind<PagerTxEvent<'static>> =
    EventKind::new("pager.tx", "POCSAG transmission");

/// The arguments of the pager `send`.
#[derive(Arguments)]
pub struct PagerSendArgs<'a> {
    /// The address (RIC).
    #[arg(
        label = "Address (RIC)",
        help = "0 to 2097151: the low 3 bits select the frame, the other 18 go in the address word."
    )]
    pub address: i64,
    /// The function bits.
    #[arg(
        label = "Function",
        help = "0 to 3. By convention 0 is numeric and 3 alphanumeric; many networks ignore it.",
        default = 3,
        min = 0,
        max = 3,
        step = 1
    )]
    pub function: Option<i64>,
    /// What the page carries.
    #[arg(shared = crate::pager::tx::PAGE_TYPE)]
    pub page_type: Option<crate::pager::tx::SendType>,
    /// The message.
    #[arg(
        label = "Text",
        help = "Numeric: 0 to 9, * U space - ) ( [ ]. Alphanumeric: 7-bit ASCII. Empty sends a tone-only page.",
        default = ""
    )]
    pub text: Option<&'a str>,
}

/// Sends a POCSAG page.
pub const PAGER_SEND: CommandKind<PagerSendArgs<'static>> =
    CommandKind::new("send", "Send a page")
        .help(
            "Checks the page and queues it. A transmission starts when none is on air; pages sent during a transmission go in \
             the next one. POCSAG only: with the setting protocol on FLEX it is refused. Transmit must be on.",
        )
        .transmits(sdrkit_core::TxContent::Text)
        .stopped_by("halt");
/// Ends the transmission.
pub const PAGER_HALT: sdrkit_core::command::Command = sdrkit_core::command::Command::new(
    "halt", "Halt",
)
.help("Drops the pages waiting and ends the transmission after the current bit with a 1 ms fall.");

/// The counts and the measurements of the GSC receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GscFields<'a> {
    /// The pages of every kind.
    #[field(label = "Pages")]
    pub pages: i64,
    /// The tone-only pages.
    #[field(label = "Tone pages")]
    pub tone_pages: i64,
    /// The numeric pages.
    #[field(label = "Numeric pages")]
    pub numeric_pages: i64,
    /// The alphanumeric pages.
    #[field(label = "Alpha pages")]
    pub alpha_pages: i64,
    /// The tone-and-voice pages.
    #[field(label = "Voice pages")]
    pub voice_pages: i64,
    /// The preamble words seen (at most one a second).
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// The receiver holds block lock.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The preamble index remembered (0 to 9).
    #[field(label = "Preamble")]
    pub preamble: Option<i64>,
    /// How the call is sent: "single call" or "batch" (known after the start code).
    #[field(label = "Call")]
    pub call: Option<&'a str>,
    /// The spectrum is inverted (from the last start code).
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// The deviation measured from the levels.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// The offset of the carrier from the tuned frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the sender's bit clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The quality of the last block read while locked: the mean of the soft bits squared over their variance.
    #[field(label = "Block quality", unit = "dB")]
    pub quality_db: Option<f64>,
    /// A carrier is present.
    #[field(label = "Carrier")]
    pub carrier: Option<bool>,
    /// The signal to noise ratio of the carrier.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the GSC table: a page or the end of a voice page. The row key is the number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GscPageRow<'a> {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What the row is: "Tone page", "Numeric page", "Alpha page", "Voice page" or "Voice end".
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The pager code: six digits, or N and five digits when no preamble was seen.
    #[field(label = "Pager code")]
    pub code: &'a str,
    /// The address function, 1 to 4.
    #[field(label = "Function")]
    pub function: i64,
    /// Which words of the address are inverted.
    #[field(label = "Words")]
    pub words: &'a str,
    /// The preamble index (0 to 9).
    #[field(label = "Preamble")]
    pub preamble: Option<i64>,
    /// How the call was sent: "single call", "batch" or "no preamble seen".
    #[field(label = "Call")]
    pub call: &'a str,
    /// The text of a data page, or "tone and voice".
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The data blocks of a data page.
    #[field(label = "Blocks")]
    pub blocks: Option<i64>,
    /// A note: more blocks were announced, the voice is not decoded, how a voice page ended.
    #[field(label = "Note")]
    pub note: Option<&'a str>,
    /// The bits corrected in the two address words.
    #[field(label = "Bits corrected")]
    pub bits_corrected: i64,
    /// The activation word of a voice page, or the word that ended it.
    #[field(label = "Word")]
    pub word: Option<i64>,
    /// The length of the voice.
    #[field(label = "Voice length", unit = "s")]
    pub voice_s: Option<f64>,
}

/// A GSC page: `target` is the pager code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GscPageEvent<'a> {
    /// The pager code.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The kind of page.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The address function, 1 to 4.
    #[field(label = "Function")]
    pub function: i64,
    /// The text of a data page.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The GSC pages and the ends of voice pages, newest last.
pub static GSC_PAGES: Table<GscPageRow<'static>> =
    Table::new("pages", "Pages", crate::gsc::ROW_CAP).label(&GscPageRow::CALL);
/// One GSC page.
pub static GSC_PAGE: EventKind<GscPageEvent<'static>> = EventKind::new("gsc.page", "GSC page");

/// The counts, the link and the measurements of the SCADA receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaFields<'a> {
    /// A chain of the modem holds lock.
    #[field(shared = keys::LOCKED)]
    pub locked: bool,
    /// The link of the last good frame: the modem, the framing, the tones or deviation and the carrier offset.
    #[field(label = "Link")]
    pub link: Option<&'a str>,
    /// The messages kept.
    #[field(label = "Messages")]
    pub messages: i64,
    /// The Modbus messages (RTU and ASCII).
    #[field(label = "Modbus messages")]
    pub modbus: i64,
    /// The DNP3 messages.
    #[field(label = "DNP3 messages")]
    pub dnp3: i64,
    /// The HDLC frames that hold neither protocol.
    #[field(label = "Unknown frames")]
    pub unknown: i64,
    /// The frames whose CRC, LRC or block CRC failed.
    #[field(label = "CRC failures")]
    pub crc_failures: i64,
    /// The DNP3 Secure Authentication messages (shown raw).
    #[field(label = "Encrypted")]
    pub encrypted: i64,
    /// The locked bursts that gave no frame (vendor radio framings), kept as raw bits.
    #[field(label = "Not recognised")]
    pub not_recognised: i64,
    /// The devices heard.
    #[field(label = "Devices")]
    pub devices: i64,
    /// The carrier offset measured on the link of the last good frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The deviation measured on that link (the FM deviation on audio FSK, the outer one on 4-FSK).
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// The level of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms, and what is shown raw.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the SCADA messages. The row key is the number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaMessageRow<'a> {
    /// The message number.
    #[field(label = "Id")]
    pub id: i64,
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The wall-clock time, when the host gives a clock.
    #[field(label = "Wall clock")]
    pub wall_clock_utc: Option<Time>,
    /// The link: the modem and the framing ("Bell 202 AFSK 1200 Bd, 8E1").
    #[field(label = "Link")]
    pub link: &'a str,
    /// The protocol: "Modbus RTU", "Modbus ASCII", "DNP3" or "unknown".
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The sender ("master", "slave 17", "outstation 1").
    #[field(label = "From")]
    pub from: &'a str,
    /// The receiver.
    #[field(label = "To")]
    pub to: &'a str,
    /// The function.
    #[field(label = "Function")]
    pub function: &'a str,
    /// The summary: addresses, quantities, values, objects, indications.
    #[field(label = "Summary")]
    pub summary: &'a str,
    /// The check: "good", "failed", or empty when there is none.
    #[field(label = "CRC")]
    pub crc: &'a str,
    /// The direction: "request", "reply" or "unsolicited".
    #[field(label = "Direction")]
    pub direction: Option<&'a str>,
    /// The id of the paired message.
    #[field(label = "Pair")]
    pub pair_id: Option<i64>,
    /// The time from the request to this reply.
    #[field(label = "Response time", unit = "ms")]
    pub response_ms: Option<f64>,
    /// DNP3 Secure Authentication: shown raw.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The bytes as hex, or the bits of a burst not recognised.
    #[field(label = "Raw")]
    pub raw_hex: &'a str,
    /// -20 log10 of the eye error of the symbols.
    #[field(label = "Quality", unit = "dB")]
    pub quality_db: f64,
}

/// A row of the SCADA devices. The row key is the protocol and the address ("modbus/17", "dnp3/1").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaDeviceRow<'a> {
    /// "Modbus" or "DNP3".
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The address.
    #[field(label = "Address")]
    pub address: i64,
    /// The role: "slave", "master" or "outstation".
    #[field(label = "Role")]
    pub role: &'a str,
    /// When it was first heard, wall clock.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// When it was last heard, wall clock.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// When it was first heard, signal time.
    #[field(label = "First", unit = "s")]
    pub first_s: f64,
    /// When it was last heard, signal time.
    #[field(label = "Last", unit = "s")]
    pub last_s: f64,
    /// The last poll, signal time.
    #[field(label = "Last poll", unit = "s")]
    pub last_poll_s: Option<f64>,
    /// The polls sent or received.
    #[field(label = "Polls")]
    pub polls: i64,
    /// The replies sent or received.
    #[field(label = "Replies")]
    pub replies: i64,
    /// The polls that had no reply.
    #[field(label = "Unanswered")]
    pub unanswered: i64,
    /// The exception replies.
    #[field(label = "Exceptions")]
    pub exceptions: i64,
    /// The unsolicited responses.
    #[field(label = "Unsolicited")]
    pub unsolicited: i64,
    /// The frames whose check failed.
    #[field(label = "CRC failures")]
    pub crc_failures: i64,
    /// The mean response time.
    #[field(label = "Mean response", unit = "ms")]
    pub mean_response_ms: Option<f64>,
    /// The last internal indications of a DNP3 outstation.
    #[field(label = "Last IIN")]
    pub last_iin: Option<&'a str>,
    /// The addresses it talked with.
    #[field(label = "Peers")]
    pub peers: &'a str,
    /// The registers or points seen so far ("40108 = 555", "AI 0 = 1000").
    #[field(label = "Points")]
    pub points: &'a str,
}

/// A SCADA message: `source` is the sender, `target` the receiver, `text` the summary.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaMessageEvent<'a> {
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The receiver.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The protocol.
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The function.
    #[field(label = "Function")]
    pub function: &'a str,
    /// The summary.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The check: "good", "failed", or empty.
    #[field(label = "CRC")]
    pub crc: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A new SCADA device.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaDeviceEvent<'a> {
    /// "Modbus" or "DNP3".
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The address.
    #[field(label = "Address")]
    pub address: i64,
    /// The role.
    #[field(label = "Role")]
    pub role: &'a str,
    /// The kind of thing that the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The reply of `export_csv`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScadaCsvReply<'a> {
    /// The messages as CSV text: the header line, then one line a message.
    #[field(label = "CSV")]
    pub csv: &'a str,
}

/// The SCADA messages, newest last.
pub static SCADA_MESSAGES: Table<ScadaMessageRow<'static>> =
    Table::new("messages", "Messages", crate::scada::MESSAGE_CAP).label(&ScadaMessageRow::FROM);
/// The SCADA devices.
pub static SCADA_DEVICES: Table<ScadaDeviceRow<'static>> =
    Table::new("devices", "Devices", crate::scada::DEVICE_CAP);
/// One SCADA message.
pub static SCADA_MESSAGE: EventKind<ScadaMessageEvent<'static>> =
    EventKind::new("scada.message", "SCADA message");
/// A device heard for the first time.
pub static SCADA_DEVICE: EventKind<ScadaDeviceEvent<'static>> =
    EventKind::new("scada.device", "SCADA device");
/// Replies with the messages as CSV text.
pub const SCADA_EXPORT_CSV: CommandKind<(), ScadaCsvReply<'static>> =
    CommandKind::new("export_csv", "Export the messages as CSV")
        .help("Replies with the messages as CSV text: id, time, wall clock, link, protocol, from, to, function, summary, CRC, direction, pair, response time, encrypted, raw hex.");

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let objects_clear = &sdrkit_util_objects::commands::CLEAR;
        let decoder_clear = &sdrkit_graph::decoder::CLEAR;
        use sdrkit_proto_tones::components as tones;
        Described::new("trunk", 1, 3)
            .added(
                1,
                3,
                &[
                    "mpt1327.content",
                    "ltr.content",
                    "edacs.content",
                    "tones.content",
                    "pager.content",
                    "ermes.content",
                    "multitone.content",
                    "motorola_type2.content",
                    "opensky.content",
                    "gsc.content",
                    "scada.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Voice,
                &["mpt1327", "ltr", "edacs", "motorola_type2", "opensky"],
            )
            .content(sdrkit_core::ContentKind::Measurement, &["tones"])
            .content(
                sdrkit_core::ContentKind::Paging,
                &["pager", "ermes", "multitone", "gsc"],
            )
            .content(sdrkit_core::ContentKind::Devices, &["scada"])
            .audio(sdrkit_core::AudioKind::Monitor, &["tones"])
            .text(&["pager", "ermes", "multitone", "gsc"])
            // The tone events of the `tones` and `pager` stacks are described in their protocol crate.
            .uses(sdrkit_proto_tones::components::interface())
            .stack(
                StackShape::new(crate::mpt::ID)
                    .fields::<Mpt1327Fields>()
                    .table(&MPT1327_MESSAGES)
                    .table(&MPT1327_CHANNELS)
                    .event(&MPT1327_MESSAGE)
                    .event(&TRUNK_FOLLOW)
                    .settings(crate::mpt::MptSetting::SETTINGS),
            )
            .stack(
                StackShape::new(crate::ltr::ID)
                    .fields::<LtrFields>()
                    .table(&LTR_WORDS)
                    .table(&LTR_REPEATERS)
                    .event(&LTR_WORD)
                    .event(&TRUNK_FOLLOW)
                    .settings(crate::ltr::LtrSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new(crate::edacs::ID)
                    .fields::<EdacsFields>()
                    .table(&EDACS_CALLS)
                    .table(&EDACS_TALKGROUPS)
                    .table(&EDACS_CHANNELS)
                    .table(&EDACS_FRAMES)
                    .table(&EDACS_WORDS)
                    .event(&EDACS_CALL)
                    .event(&TRUNK_FOLLOW)
                    .settings(crate::edacs::EdacsSetting::SETTINGS)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new(crate::tones::ID)
                    .fields::<TonesFields>()
                    .table(&TONES_HEARD)
                    .series(TONES_SPECTRUM)
                    .event(tones::subaudio())
                    .event(tones::dtmf())
                    .event(tones::selcall())
                    .event(tones::selcal())
                    .event(tones::two_tone())
                    .settings(crate::tones::ToneSetting::SETTINGS)
                    .command(objects_clear)
                    // A scan across frequencies builds the list of tones heard: the component that holds it stays.
                    .kept(&["events", "TonesFields"]),
            )
            .stack(
                StackShape::new(crate::pager::ID)
                    .fields::<PagerFields>()
                    .table(&PAGER_PAGES)
                    .table(&PAGER_SPEEDS)
                    .table(&PAGER_HEARD)
                    .event(&POCSAG_PAGE)
                    .event(&FLEX_PAGE)
                    .event(tones::selcall())
                    .settings(crate::pager::PagerSetting::SETTINGS)
                    .command(objects_clear)
                    .fields::<PagerTxFields>()
                    .event(&PAGER_TX)
                    .command(&PAGER_SEND)
                    .command(&PAGER_HALT)
                    // The pages carry their frequency and the addresses heard are a database of stations: a scan
                    // across frequencies builds them, and the component that holds them stays.
                    .kept(&["pages", "speeds", "heard", "PagerFields"]),
            )
            .stack(
                StackShape::new(crate::ermes::ID)
                    .fields::<ErmesFields>()
                    .fields::<DecoderFields>()
                    .decoder("modem")
                    .table(&ERMES_ROWS)
                    .event(&ERMES_BATCH)
                    .event(&ERMES_MESSAGE)
                    .settings(crate::ermes::gen::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::multitone::ID)
                    .fields::<MultitoneFields>()
                    .table(&MULTITONE_TRANSMISSIONS)
                    .table(&MULTITONE_TONES)
                    .table(&MULTITONE_LINES)
                    .table(&MULTITONE_HEARD)
                    .table(&MULTITONE_BURSTS)
                    .event(&MULTITONE_TRANSMISSION)
                    .event(&MULTITONE_BURST)
                    .command(objects_clear),
            )
            .stack(
                StackShape::new(crate::type2::ID)
                    .fields::<MotorolaType2Fields>()
                    .fields::<DecoderFields>()
                    .table(&MOTOROLA_TYPE2_MESSAGES)
                    .table(&MOTOROLA_TYPE2_TALKGROUPS)
                    .table(&MOTOROLA_TYPE2_SITES)
                    .table(&MOTOROLA_TYPE2_PATCHES)
                    .table(&MOTOROLA_TYPE2_CALLS)
                    .event(&MOTOROLA_TYPE2_MESSAGE)
                    .event(&TRUNK_GRANT)
                    .event(&TRUNK_FOLLOW)
                    .settings(&[
                        &crate::type2::tables::PLAN,
                        &crate::follow::FOLLOW,
                        &crate::type2::tables::FOLLOW_PRIVATE,
                        &crate::follow::FOLLOW_GROUPS,
                        &crate::follow::FOLLOW_MAX,
                    ])
                    .settings(crate::type2::gen::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::opensky::ID)
                    .fields::<OpenskyFields>()
                    .fields::<DecoderFields>()
                    .table(&OPENSKY_BLOCKS)
                    .event(&OPENSKY_VERDICT)
                    .settings(crate::opensky::gen::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::gsc::ID)
                    .fields::<GscFields>()
                    .fields::<DecoderFields>()
                    .decoder("modem")
                    .table(&GSC_PAGES)
                    .event(&GSC_PAGE)
                    .settings(crate::gsc::gen::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::scada::ID)
                    .fields::<ScadaFields>()
                    .fields::<DecoderFields>()
                    .decoder("modem")
                    .table(&SCADA_MESSAGES)
                    .table(&SCADA_DEVICES)
                    .event(&SCADA_MESSAGE)
                    .event(&SCADA_DEVICE)
                    .settings(crate::scada::gen::SETTINGS)
                    .command(decoder_clear)
                    .command(&SCADA_EXPORT_CSV),
            )
            .added(1, 2, ADDED_1_2)
            .added(1, 1, ADDED_1_1)
    })
}

/// What version 1.2 added: the POCSAG sender of the stack `pager`.
const ADDED_1_2: &[&str] = &[
    "pager.fields.PagerTxFields",
    "pager.event.pager.tx",
    "pager.setting.protocol",
    "pager.setting.pocsag_rate",
    "pager.setting.deviation",
    "pager.setting.polarity",
    "pager.setting.tx_level",
    "pager.command.send",
    "pager.command.halt",
    "shape.PagerTxFields",
    "shape.PagerTxFields.tx_sending",
    "shape.PagerTxFields.tx_sent",
    "shape.PagerTxFields.tx_waiting",
    "shape.PagerTxEvent",
    "shape.PagerTxEvent.state",
    "shape.PagerTxEvent.reason",
    "shape.PagerTxEvent.target",
    "shape.PagerTxEvent.text",
    "shape.PagerTxEvent.family",
];

/// What version 1.1 added: the stacks `gsc` and `scada`.
const ADDED_1_1: &[&str] = &[
    "gsc",
    "gsc.fields.GscFields",
    "gsc.fields.DecoderFields",
    "gsc.component.modem.dropped",
    "gsc.table.pages",
    "gsc.event.gsc.page",
    "gsc.setting.gen_pages",
    "gsc.setting.gen_batch",
    "gsc.setting.gen_no_preamble",
    "gsc.setting.gen_deviation_hz",
    "gsc.command.clear",
    "shape.GscFields",
    "shape.GscFields.pages",
    "shape.GscFields.tone_pages",
    "shape.GscFields.numeric_pages",
    "shape.GscFields.alpha_pages",
    "shape.GscFields.voice_pages",
    "shape.GscFields.preambles",
    "shape.GscFields.locked",
    "shape.GscFields.preamble",
    "shape.GscFields.call",
    "shape.GscFields.inverted",
    "shape.GscFields.deviation_hz",
    "shape.GscFields.offset_hz",
    "shape.GscFields.clock_ppm",
    "shape.GscFields.quality_db",
    "shape.GscFields.carrier",
    "shape.GscFields.snr_db",
    "shape.GscFields.level_dbfs",
    "shape.GscFields.note",
    "shape.GscPageRow",
    "shape.GscPageRow.time_s",
    "shape.GscPageRow.kind",
    "shape.GscPageRow.code",
    "shape.GscPageRow.function",
    "shape.GscPageRow.words",
    "shape.GscPageRow.preamble",
    "shape.GscPageRow.call",
    "shape.GscPageRow.text",
    "shape.GscPageRow.blocks",
    "shape.GscPageRow.note",
    "shape.GscPageRow.bits_corrected",
    "shape.GscPageRow.word",
    "shape.GscPageRow.voice_s",
    "shape.GscPageEvent",
    "shape.GscPageEvent.target",
    "shape.GscPageEvent.kind",
    "shape.GscPageEvent.function",
    "shape.GscPageEvent.text",
    "shape.GscPageEvent.family",
    "scada",
    "scada.fields.ScadaFields",
    "scada.fields.DecoderFields",
    "scada.component.modem.dropped",
    "scada.table.messages",
    "scada.table.devices",
    "scada.event.scada.message",
    "scada.event.scada.device",
    "scada.setting.gen_link",
    "scada.setting.gen_framing",
    "scada.setting.gen_traffic",
    "scada.command.clear",
    "scada.command.export_csv",
    "shape.ScadaFields",
    "shape.ScadaFields.locked",
    "shape.ScadaFields.link",
    "shape.ScadaFields.messages",
    "shape.ScadaFields.modbus",
    "shape.ScadaFields.dnp3",
    "shape.ScadaFields.unknown",
    "shape.ScadaFields.crc_failures",
    "shape.ScadaFields.encrypted",
    "shape.ScadaFields.not_recognised",
    "shape.ScadaFields.devices",
    "shape.ScadaFields.offset_hz",
    "shape.ScadaFields.deviation_hz",
    "shape.ScadaFields.level_dbfs",
    "shape.ScadaFields.note",
    "shape.ScadaMessageRow",
    "shape.ScadaMessageRow.id",
    "shape.ScadaMessageRow.time_s",
    "shape.ScadaMessageRow.wall_clock_utc",
    "shape.ScadaMessageRow.link",
    "shape.ScadaMessageRow.protocol",
    "shape.ScadaMessageRow.from",
    "shape.ScadaMessageRow.to",
    "shape.ScadaMessageRow.function",
    "shape.ScadaMessageRow.summary",
    "shape.ScadaMessageRow.crc",
    "shape.ScadaMessageRow.direction",
    "shape.ScadaMessageRow.pair_id",
    "shape.ScadaMessageRow.response_ms",
    "shape.ScadaMessageRow.encrypted",
    "shape.ScadaMessageRow.raw_hex",
    "shape.ScadaMessageRow.quality_db",
    "shape.ScadaDeviceRow",
    "shape.ScadaDeviceRow.protocol",
    "shape.ScadaDeviceRow.address",
    "shape.ScadaDeviceRow.role",
    "shape.ScadaDeviceRow.first_heard",
    "shape.ScadaDeviceRow.last_heard",
    "shape.ScadaDeviceRow.first_s",
    "shape.ScadaDeviceRow.last_s",
    "shape.ScadaDeviceRow.last_poll_s",
    "shape.ScadaDeviceRow.polls",
    "shape.ScadaDeviceRow.replies",
    "shape.ScadaDeviceRow.unanswered",
    "shape.ScadaDeviceRow.exceptions",
    "shape.ScadaDeviceRow.unsolicited",
    "shape.ScadaDeviceRow.crc_failures",
    "shape.ScadaDeviceRow.mean_response_ms",
    "shape.ScadaDeviceRow.last_iin",
    "shape.ScadaDeviceRow.peers",
    "shape.ScadaDeviceRow.points",
    "shape.ScadaMessageEvent",
    "shape.ScadaMessageEvent.source",
    "shape.ScadaMessageEvent.target",
    "shape.ScadaMessageEvent.protocol",
    "shape.ScadaMessageEvent.function",
    "shape.ScadaMessageEvent.text",
    "shape.ScadaMessageEvent.crc",
    "shape.ScadaMessageEvent.family",
    "shape.ScadaDeviceEvent",
    "shape.ScadaDeviceEvent.protocol",
    "shape.ScadaDeviceEvent.address",
    "shape.ScadaDeviceEvent.role",
    "shape.ScadaDeviceEvent.family",
    "shape.ScadaCsvReply",
    "shape.ScadaCsvReply.csv",
];

/// The description of stack `id` of the family.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the trunk family does not describe the stack {id}"))
}
