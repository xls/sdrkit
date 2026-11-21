//! The interface of the tags family: the shapes of the result sets, the rows of the tables and the events, the tables, the
//! event types and what each stack declares.

use sdrkit_core::interface::{EventKind, Family, Fields, Seconds, StackShape, Table};
use sdrkit_core::publish::keys;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use std::sync::OnceLock;

// ---- every stack ----

sdrkit_core::choice_enum! {
    /// The state of a stack of the family.
    pub enum TagsState {
        /// No energy and no result in the last 2 s.
        NoSignal = ("no_signal", "No signal"),
        /// Energy or a refused result in the last 2 s, no good result in the last 8 s.
        Energy = ("energy", "Energy"),
        /// A good result in the last 8 s.
        Decoding = ("decoding", "Decoding"),
    }
}

/// The status of a stack of the family: the headline, the state, the counts of results and the input level.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TagsStatusFields<'a> {
    /// The headline.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The state.
    #[field(label = "State")]
    pub state: TagsState,
    /// The good results: what passed every check of its format.
    #[field(label = "Good")]
    pub good: i64,
    /// The refused results: what failed a check.
    #[field(label = "Refused")]
    pub bad: i64,
    /// The input level of the last report, relative to full scale.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What the stack reads and what no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- rfid_lf ----

sdrkit_core::choice_enum! {
    /// The bit rate of an EM4100 tag, in periods of the reader carrier.
    pub enum RfidBitRate {
        /// 64 carrier periods a bit.
        Rf64 = ("rf64", "RF/64"),
        /// 32 carrier periods a bit.
        Rf32 = ("rf32", "RF/32"),
    }
}

sdrkit_core::choice_enum! {
    /// The code of an EM4100 tag.
    pub enum RfidCoding {
        /// A change in the middle of every bit.
        Manchester = ("manchester", "Manchester"),
        /// A change at the start of every bit, one more in the middle of a 0.
        Biphase = ("biphase", "Biphase"),
    }
}

sdrkit_core::choice_enum! {
    /// The bit rates that `rfid_lf` reads.
    pub enum RfidBitRates {
        /// RF/64 and RF/32.
        Both = ("both", "Both"),
        /// RF/64 only.
        Rf64 = ("rf64", "RF/64"),
        /// RF/32 only.
        Rf32 = ("rf32", "RF/32"),
    }
}

sdrkit_core::choice_enum! {
    /// The codes that `rfid_lf` reads.
    pub enum RfidCodings {
        /// Manchester and Biphase.
        Both = ("both", "Both"),
        /// Manchester only.
        Manchester = ("manchester", "Manchester"),
        /// Biphase only.
        Biphase = ("biphase", "Biphase"),
    }
}

/// The result fields of `rfid_lf`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidLfFields<'a> {
    /// The level of the reader carrier, relative; no value before the first input.
    #[field(label = "Carrier level (relative)", unit = "dB")]
    pub carrier_db: Option<f64>,
    /// The modulation depth of the load (swing over carrier); no value without load modulation in the last 2 s.
    #[field(label = "Modulation depth", unit = "%")]
    pub depth_pct: Option<f64>,
    /// The different tags seen.
    #[field(label = "Different tags seen")]
    pub tags: i64,
    /// The reads: frames that passed every check.
    #[field(label = "Reads")]
    pub reads: i64,
    /// Frames at the place of the frames of the last read (the same code, the same position in the stream) whose header and
    /// stop bit held and whose row or column parity failed.
    #[field(label = "Frames refused")]
    pub refused: i64,
    /// The bit rate of the last read.
    #[field(label = "Bit rate of the last read")]
    pub bit_rate: Option<RfidBitRate>,
    /// The code of the last read.
    #[field(label = "Code of the last read")]
    pub coding: Option<RfidCoding>,
    /// The last tag: version and number, 10 hex digits.
    #[field(label = "Last tag")]
    pub last_tag: Option<&'a str>,
}

/// A row of the tag table: one tag, read until another tag came or it was not read for the hold time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidLfTagRow<'a> {
    /// The signal time of the last read.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// Version (2 hex digits) and number (8 hex digits).
    #[field(label = "Tag")]
    pub tag: &'a str,
    /// The version or customer code, 0 to 255.
    #[field(label = "Version")]
    pub version: i64,
    /// The number, 0 to 4294967295.
    #[field(label = "Number")]
    pub number: i64,
    /// The upper 16 bits of the number.
    #[field(label = "Number, upper 16 bits")]
    pub number_hi: i64,
    /// The lower 16 bits of the number.
    #[field(label = "Number, lower 16 bits")]
    pub number_lo: i64,
    /// The bit rate.
    #[field(label = "Bit rate")]
    pub bit_rate: RfidBitRate,
    /// The code.
    #[field(label = "Code")]
    pub coding: RfidCoding,
    /// The reads of the row.
    #[field(label = "Reads")]
    pub reads: i64,
    /// The carrier level, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A tag was read for the first time, or again after another tag or after the hold time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidTagEvent<'a> {
    /// The signal time of the read.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// Version (2 hex digits) and number (8 hex digits).
    #[field(label = "Tag")]
    pub tag: &'a str,
    /// The version or customer code, 0 to 255.
    #[field(label = "Version")]
    pub version: i64,
    /// The number, 0 to 4294967295.
    #[field(label = "Number")]
    pub number: i64,
    /// The upper 16 bits of the number.
    #[field(label = "Number, upper 16 bits")]
    pub number_hi: i64,
    /// The lower 16 bits of the number.
    #[field(label = "Number, lower 16 bits")]
    pub number_lo: i64,
    /// The bit rate.
    #[field(label = "Bit rate")]
    pub bit_rate: RfidBitRate,
    /// The code.
    #[field(label = "Code")]
    pub coding: RfidCoding,
    /// The reads of the row.
    #[field(label = "Reads")]
    pub reads: i64,
    /// The carrier level, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The tags read, newest first.
pub static RFID_LF_TAGS: Table<RfidLfTagRow<'static>> =
    Table::new("tags", "Tags", crate::rfid_lf::ROWS_KEPT);
/// A tag was read.
pub static RFID_TAG: EventKind<RfidTagEvent<'static>> = EventKind::new("rfid.tag", "Tag read");

// ---- qi ----

sdrkit_core::choice_enum! {
    /// The type of a Qi packet (Qi version 1.2.3, Table 26).
    pub enum QiMessage {
        /// 01: how well the coils are coupled.
        SignalStrength = ("signal_strength", "Signal strength"),
        /// 02: the power receiver asks to end the power transfer.
        EndPowerTransfer = ("end_power_transfer", "End power transfer"),
        /// 03: the power receiver asks for more (positive) or less (negative) power.
        ControlError = ("control_error", "Control error"),
        /// 04: the power that the power receiver measures, 8 bits.
        ReceivedPower = ("received_power", "Received power"),
        /// 05: the charge of the battery.
        ChargeStatus = ("charge_status", "Charge status"),
        /// 06: the time the power transmitter waits before it changes the power.
        HoldOff = ("hold_off", "Power control hold-off"),
        /// 07: a request of the extended power profile.
        GeneralRequest = ("general_request", "General request"),
        /// 09: renegotiate.
        Renegotiate = ("renegotiate", "Renegotiate"),
        /// 20: a request of the extended power profile.
        SpecificRequest = ("specific_request", "Specific request"),
        /// 22: the state of the foreign object detection.
        FodStatus = ("fod_status", "FOD status"),
        /// 31: the power that the power receiver measures, 24 bits.
        ReceivedPower24 = ("received_power_24", "Received power (24 bits)"),
        /// 51: the configuration of the power receiver.
        Configuration = ("configuration", "Configuration"),
        /// 54 and 55: the wireless power identifier.
        Wpid = ("wpid", "Wireless power ID"),
        /// 71: the identification of the power receiver.
        Identification = ("identification", "Identification"),
        /// 81: the extended identification.
        ExtendedIdentification = ("extended_identification", "Extended identification"),
        /// A header that the standard leaves to the maker.
        Proprietary = ("proprietary", "Proprietary"),
        /// A reserved header.
        Other = ("other", "Other"),
    }
}

/// The result fields of `qi`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QiFields {
    /// The level of the power signal, relative; no value before the first input.
    #[field(label = "Power signal level (relative)", unit = "dB")]
    pub power_signal_db: Option<f64>,
    /// The load modulation (swing over power signal); no value without load modulation in the last 2 s.
    #[field(label = "Load modulation", unit = "%")]
    pub load_modulation_pct: Option<f64>,
    /// The packets that passed every check.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// The packets that failed a parity, a start or stop bit, the checksum, or lost their bit clock after the header.
    #[field(label = "Packets failed")]
    pub packets_failed: i64,
    /// The bit rate that the bit clock follows; no value without load modulation in the last 2 s.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate: Option<f64>,
    /// The type of the last good packet.
    #[field(label = "Last message")]
    pub last_message: Option<QiMessage>,
}

/// A row of the packet table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QiPacketRow<'a> {
    /// The signal time of the end of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The header, 2 hex digits.
    #[field(label = "Header")]
    pub header: &'a str,
    /// The type.
    #[field(label = "Message")]
    pub message: QiMessage,
    /// The message bytes in hex.
    #[field(label = "Bytes")]
    pub bytes: &'a str,
    /// Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
    #[field(label = "Value")]
    pub value: Option<i64>,
    /// The reason of an End Power Transfer packet.
    #[field(label = "End reason")]
    pub end_reason: Option<&'a str>,
    /// The meaning of a Charge Status value.
    #[field(label = "Charge")]
    pub charge: Option<&'a str>,
    /// The power class of a Configuration packet.
    #[field(label = "Power class")]
    pub power_class: Option<i64>,
    /// The maximum power value of a Configuration packet.
    #[field(label = "Maximum power value")]
    pub max_power: Option<i64>,
    /// The version of an Identification packet, major.minor.
    #[field(label = "Version")]
    pub version: Option<&'a str>,
    /// The manufacturer code of an Identification packet, 4 hex digits.
    #[field(label = "Manufacturer")]
    pub manufacturer: Option<&'a str>,
    /// The basic device identifier of an Identification packet, 8 hex digits (31 bits).
    #[field(label = "Device identity")]
    pub device_id: Option<&'a str>,
    /// An Extended Identification packet follows an Identification packet.
    #[field(label = "Extended identification")]
    pub extended: Option<bool>,
    /// The power signal level, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A packet passed every check.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QiPacketEvent<'a> {
    /// The signal time of the end of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The header, 2 hex digits.
    #[field(label = "Header")]
    pub header: &'a str,
    /// The type.
    #[field(label = "Message")]
    pub message: QiMessage,
    /// The message bytes in hex.
    #[field(label = "Bytes")]
    pub bytes: &'a str,
    /// Signal strength, End Power Transfer code, control error (signed), received power, charge status or hold-off (ms).
    #[field(label = "Value")]
    pub value: Option<i64>,
    /// The reason of an End Power Transfer packet.
    #[field(label = "End reason")]
    pub end_reason: Option<&'a str>,
    /// The meaning of a Charge Status value.
    #[field(label = "Charge")]
    pub charge: Option<&'a str>,
    /// The power class of a Configuration packet.
    #[field(label = "Power class")]
    pub power_class: Option<i64>,
    /// The maximum power value of a Configuration packet.
    #[field(label = "Maximum power value")]
    pub max_power: Option<i64>,
    /// The version of an Identification packet, major.minor.
    #[field(label = "Version")]
    pub version: Option<&'a str>,
    /// The manufacturer code of an Identification packet, 4 hex digits.
    #[field(label = "Manufacturer")]
    pub manufacturer: Option<&'a str>,
    /// The basic device identifier of an Identification packet, 8 hex digits (31 bits).
    #[field(label = "Device identity")]
    pub device_id: Option<&'a str>,
    /// An Extended Identification packet follows an Identification packet.
    #[field(label = "Extended identification")]
    pub extended: Option<bool>,
    /// The power signal level, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The packets, newest first.
pub static QI_PACKETS: Table<QiPacketRow<'static>> =
    Table::new("packets", "Packets", crate::qi::ROWS_KEPT);
/// A packet passed every check.
pub static QI_PACKET: EventKind<QiPacketEvent<'static>> = EventKind::new("qi.packet", "Packet");

// ---- rfid_uhf ----

sdrkit_core::choice_enum! {
    /// A reader command of EPC Gen2 (version 2.0.1, Table 6.28).
    pub enum Gen2Command {
        /// Starts an inventory round.
        Query = ("query", "Query"),
        /// The next slot of a round.
        QueryRep = ("query_rep", "QueryRep"),
        /// Changes Q.
        QueryAdjust = ("query_adjust", "QueryAdjust"),
        /// Acknowledges a tag.
        Ack = ("ack", "ACK"),
        /// Returns the tags to arbitrate.
        Nak = ("nak", "NAK"),
        /// Selects a tag population.
        Select = ("select", "Select"),
        /// Asks a new RN16 or a handle.
        ReqRn = ("req_rn", "Req_RN"),
        /// Reads memory.
        Read = ("read", "Read"),
        /// Writes memory.
        Write = ("write", "Write"),
        /// Kills a tag.
        Kill = ("kill", "Kill"),
        /// Locks memory.
        Lock = ("lock", "Lock"),
        /// Opens the secured state.
        Access = ("access", "Access"),
    }
}

/// The result fields of `rfid_uhf`. The timings are those of the last good command; they have no value without a reader
/// carrier in the last 8 s.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidUhfFields {
    /// Tari: the length of a data-0.
    #[field(label = "Tari (data-0)", unit = "us")]
    pub tari_us: Option<f64>,
    /// RTcal: a data-0 and a data-1.
    #[field(label = "RTcal", unit = "us")]
    pub rtcal_us: Option<f64>,
    /// RTcal in Tari.
    #[field(label = "RTcal in Tari")]
    pub rtcal_tari: Option<f64>,
    /// TRcal of the last Query.
    #[field(label = "TRcal", unit = "us")]
    pub trcal_us: Option<f64>,
    /// The backscatter link frequency that the last Query asks with the divide ratio 8: 8 / TRcal.
    #[field(label = "Tag link frequency, DR 8", unit = "kHz")]
    pub blf_dr8_khz: Option<f64>,
    /// The backscatter link frequency with the divide ratio 64/3.
    #[field(label = "Tag link frequency, DR 64/3", unit = "kHz")]
    pub blf_dr643_khz: Option<f64>,
    /// The commands that passed every check.
    #[field(label = "Commands decoded")]
    pub commands_ok: i64,
    /// The frames that failed a timing rule, their length or their CRC.
    #[field(label = "Frames failed")]
    pub commands_failed: i64,
    /// The Query commands decoded.
    #[field(label = "Query")]
    pub count_query: i64,
    /// The QueryRep commands decoded.
    #[field(label = "QueryRep")]
    pub count_query_rep: i64,
    /// The QueryAdjust commands decoded.
    #[field(label = "QueryAdjust")]
    pub count_query_adjust: i64,
    /// The ACK commands decoded.
    #[field(label = "ACK")]
    pub count_ack: i64,
    /// The NAK commands decoded.
    #[field(label = "NAK")]
    pub count_nak: i64,
    /// The Select commands decoded.
    #[field(label = "Select")]
    pub count_select: i64,
    /// The Req_RN commands decoded.
    #[field(label = "Req_RN")]
    pub count_req_rn: i64,
    /// The Read commands decoded.
    #[field(label = "Read")]
    pub count_read: i64,
    /// The Write commands decoded.
    #[field(label = "Write")]
    pub count_write: i64,
    /// The Kill commands decoded.
    #[field(label = "Kill")]
    pub count_kill: i64,
    /// The Lock commands decoded.
    #[field(label = "Lock")]
    pub count_lock: i64,
    /// The Access commands decoded.
    #[field(label = "Access")]
    pub count_access: i64,
    /// A modulated reader carrier was seen in the last 2 s.
    #[field(label = "Reader carrier seen")]
    pub reader_carrier: bool,
}

/// A row of the command table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidUhfCommandRow<'a> {
    /// The signal time of the end of the command.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The command.
    #[field(label = "Command")]
    pub command: Gen2Command,
    /// The session, 0 to 3.
    #[field(label = "Session")]
    pub session: Option<i64>,
    /// Q of a Query.
    #[field(label = "Q")]
    pub q: Option<i64>,
    /// The slots of a Query, 2 to the power Q.
    #[field(label = "Slots")]
    pub slots: Option<i64>,
    /// The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
    #[field(label = "Target")]
    pub target: Option<&'a str>,
    /// The tags a Query addresses: all, ~SL or SL.
    #[field(label = "Sel")]
    pub sel: Option<&'a str>,
    /// The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
    #[field(label = "Tag data coding")]
    pub coding: Option<&'a str>,
    /// A Query asks for a pilot tone (TRext).
    #[field(label = "Pilot tone")]
    pub pilot: Option<bool>,
    /// The divide ratio of a Query: 8 or 64/3.
    #[field(label = "Divide ratio")]
    pub dr: Option<&'a str>,
    /// The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
    #[field(label = "UpDn")]
    pub updn: Option<&'a str>,
    /// The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
    #[field(label = "RN16 or handle")]
    pub rn16: Option<&'a str>,
    /// The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
    #[field(label = "Memory bank")]
    pub bank: Option<&'a str>,
    /// The bit pointer of a Select, or the word pointer of a Read or a Write.
    #[field(label = "Pointer")]
    pub pointer: Option<i64>,
    /// The words that a Read asks.
    #[field(label = "Word count")]
    pub word_count: Option<i64>,
    /// The mask of a Select, as bits (the first 96).
    #[field(label = "Mask")]
    pub mask: Option<&'a str>,
    /// The Truncate bit of a Select.
    #[field(label = "Truncate")]
    pub truncate: Option<bool>,
    /// The action of a Select, 0 to 7.
    #[field(label = "Action")]
    pub action: Option<i64>,
    /// The mask and action bits of a Lock, 5 hex digits.
    #[field(label = "Lock payload")]
    pub lock_payload: Option<&'a str>,
    /// The level of the reader carrier, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A reader command passed every check.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RfidCommandEvent<'a> {
    /// The signal time of the end of the command.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The command.
    #[field(label = "Command")]
    pub command: Gen2Command,
    /// The session, 0 to 3.
    #[field(label = "Session")]
    pub session: Option<i64>,
    /// Q of a Query.
    #[field(label = "Q")]
    pub q: Option<i64>,
    /// The slots of a Query, 2 to the power Q.
    #[field(label = "Slots")]
    pub slots: Option<i64>,
    /// The target: A or B of a Query, S0 to S3, SL or RFU of a Select.
    #[field(label = "Target")]
    pub target: Option<&'a str>,
    /// The tags a Query addresses: all, ~SL or SL.
    #[field(label = "Sel")]
    pub sel: Option<&'a str>,
    /// The coding of the tag reply that a Query asks: FM0, Miller 2, 4 or 8.
    #[field(label = "Tag data coding")]
    pub coding: Option<&'a str>,
    /// A Query asks for a pilot tone (TRext).
    #[field(label = "Pilot tone")]
    pub pilot: Option<bool>,
    /// The divide ratio of a Query: 8 or 64/3.
    #[field(label = "Divide ratio")]
    pub dr: Option<&'a str>,
    /// The change of Q of a QueryAdjust: +1, -1, unchanged or reserved.
    #[field(label = "UpDn")]
    pub updn: Option<&'a str>,
    /// The RN16 of an ACK or a Req_RN, or the handle of an access command, 4 hex digits.
    #[field(label = "RN16 or handle")]
    pub rn16: Option<&'a str>,
    /// The memory bank of a Select (FileType, EPC, TID, File_0) or of a Read or Write (Reserved, EPC, TID, User).
    #[field(label = "Memory bank")]
    pub bank: Option<&'a str>,
    /// The bit pointer of a Select, or the word pointer of a Read or a Write.
    #[field(label = "Pointer")]
    pub pointer: Option<i64>,
    /// The words that a Read asks.
    #[field(label = "Word count")]
    pub word_count: Option<i64>,
    /// The mask of a Select, as bits (the first 96).
    #[field(label = "Mask")]
    pub mask: Option<&'a str>,
    /// The Truncate bit of a Select.
    #[field(label = "Truncate")]
    pub truncate: Option<bool>,
    /// The action of a Select, 0 to 7.
    #[field(label = "Action")]
    pub action: Option<i64>,
    /// The mask and action bits of a Lock, 5 hex digits.
    #[field(label = "Lock payload")]
    pub lock_payload: Option<&'a str>,
    /// The level of the reader carrier, relative.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The reader commands, newest first.
pub static RFID_UHF_COMMANDS: Table<RfidUhfCommandRow<'static>> =
    Table::new("commands", "Commands", crate::rfid_uhf::ROWS_KEPT);
/// A reader command passed every check.
pub static RFID_COMMAND: EventKind<RfidCommandEvent<'static>> =
    EventKind::new("rfid.command", "Reader command");

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let clear = &sdrkit_graph::decoder::CLEAR;
        Family::new("tags", 1, 0)
            .stack(
                StackShape::new(crate::rfid_lf::ID)
                    .fields::<TagsStatusFields>()
                    .fields::<RfidLfFields>()
                    .fields::<DecoderFields>()
                    .table(&RFID_LF_TAGS)
                    .event(&RFID_TAG)
                    .settings(crate::rfid_lf::RfidLfSetting::SETTINGS)
                    .command(clear)
                    .command(&crate::rfid_lf::RESET)
                    .kept(&[
                        "tags",
                        "RfidLfFields.tags",
                        "RfidLfFields.reads",
                        "RfidLfFields.refused",
                        "RfidLfFields.last_tag",
                    ]),
            )
            .stack(
                StackShape::new(crate::qi::ID)
                    .fields::<TagsStatusFields>()
                    .fields::<QiFields>()
                    .fields::<DecoderFields>()
                    .table(&QI_PACKETS)
                    .event(&QI_PACKET)
                    .settings(crate::qi::QiSetting::SETTINGS)
                    .command(clear)
                    .command(&crate::qi::RESET)
                    .kept(&[
                        "packets",
                        "QiFields.packets_ok",
                        "QiFields.packets_failed",
                        "QiFields.last_message",
                    ]),
            )
            .stack(
                StackShape::new(crate::rfid_uhf::ID)
                    .fields::<TagsStatusFields>()
                    .fields::<RfidUhfFields>()
                    .fields::<DecoderFields>()
                    .table(&RFID_UHF_COMMANDS)
                    .event(&RFID_COMMAND)
                    .settings(crate::rfid_uhf::RfidUhfSetting::SETTINGS)
                    .command(clear)
                    .command(&crate::rfid_uhf::RESET)
                    .kept(&["commands"]),
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
        .unwrap_or_else(|| panic!("the tags family does not describe the stack {id}"))
}
