//! The interface of the wlan family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::ble::adv::DeviceKind;
use crate::ble::table::AddressType;
use crate::ble::BleSetting;
use crate::btclassic::BtSetting;
use crate::espnow::EspnowSetting;
use crate::halow::phy::Preamble;
use crate::halow::table::WidthState;
use crate::halow::HalowSetting;
use crate::tracker::book::{TrackerKind, TrackerState, Trend};
use crate::tracker::layout::{SmartTagState, TagMode};
use crate::tracker::TrackerSetting;
use crate::wfb::WfbSetting;
use crate::wifi::mac::{WifiGeneration, WifiSecurity};
use crate::wifi::plan::Band;
use crate::wifi::{PhyKind, WifiSetting};
use sdrkit_core::command::Command;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::settings::Argument;
use sdrkit_core::{Setting, SettingSet};
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

// What every stack shares.

/// Forgets everything that the stack decoded so far.
pub const CLEAR: &Command = &sdrkit_graph::decoder::CLEAR;

// wfb

/// The result fields of `wfb`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WfbFields<'a> {
    /// Frames of wfb-ng or OpenHD links.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// HT frames with a good FCS.
    #[field(label = "HT frames decoded")]
    pub ht_frames: i64,
    /// HT-mixed PPDUs that the decoder does not take.
    #[field(label = "HT frames not decodable here")]
    pub ht_not_decodable: i64,
    /// HT frames with a bad FCS.
    #[field(label = "Frames with a bad FCS")]
    pub bad_fcs: i64,
    /// Links in the table.
    #[field(label = "Links")]
    pub links_heard: i64,
    /// Frames lost over every link.
    #[field(label = "Frames lost")]
    pub frames_lost: i64,
    /// The SNR of the last frame. No value before the first frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the links of `wfb`. The row key is the system and the link id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WfbLinkRow<'a> {
    /// "wfb-ng" or "OpenHD".
    #[field(label = "Link")]
    pub link: &'a str,
    /// The link id.
    #[field(label = "Id")]
    pub id: &'a str,
    /// Frames heard.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames lost.
    #[field(label = "Frames lost")]
    pub frames_lost: i64,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The SNR of the last frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The type of the last frame.
    #[field(label = "Last type")]
    pub last_type: &'a str,
    /// The detail of the last frame.
    #[field(label = "Last detail")]
    pub last_detail: &'a str,
}

/// A row of the frame log of `wfb`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WfbFrameRow<'a> {
    /// The UTC of the frame, for a live source.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// "wfb-ng" or "OpenHD".
    #[field(label = "Link")]
    pub link: &'a str,
    /// The link id.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The type of the frame.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The detail of the frame.
    #[field(label = "Detail")]
    pub detail: &'a str,
    /// The first payload bytes, hex.
    #[field(label = "Payload")]
    pub hex: &'a str,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// The links of `wfb`, most recently heard first.
pub static WFB_LINKS: Table<WfbLinkRow<'static>> =
    Table::new("links", "Links", crate::wfb::LINK_CAP).label(&WfbLinkRow::ID);
/// The frame log of `wfb`, newest first.
pub static WFB_FRAMES: Table<WfbFrameRow<'static>> =
    Table::new("frames", "Frames", crate::wfb::FRAME_CAP).label(&WfbFrameRow::HEX);

// ble

/// The result fields of `ble`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleFields<'a> {
    /// Devices in the table.
    #[field(label = "Devices", reading = 1)]
    pub devices: i64,
    /// Good packets.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Exact access-address matches whose CRC failed on every channel.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Access-address candidates.
    #[field(label = "Access address hits")]
    pub aa_hits: i64,
    /// The running mean of the eye SNR. No value before the first good packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good packet. No value before it.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The deviation of the last good packet. No value before it.
    #[field(label = "Deviation", unit = "Hz")]
    pub deviation_hz: Option<Hertz>,
    /// The level of the channel. No value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The tuned advertising channel. No value when it is not known, as for a recording.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The channel of the last good packet. No value before it.
    #[field(label = "Last channel")]
    pub last_channel: Option<i64>,
    /// Good packets on channel 37.
    #[field(label = "Packets on 37")]
    pub packets_37: i64,
    /// Good packets on channel 38.
    #[field(label = "Packets on 38")]
    pub packets_38: i64,
    /// Good packets on channel 39.
    #[field(label = "Packets on 39")]
    pub packets_39: i64,
    /// The host steps through the advertising channels.
    #[field(label = "Hopping")]
    pub hopping: bool,
    /// The privacy note.
    #[field(label = "Privacy")]
    pub privacy: &'a str,
    /// Good packets on the secondary advertising channels and the periodic trains (channels 0 to 36).
    #[field(label = "Packets on secondary channels")]
    pub packets_secondary: i64,
    /// Good packets on the LE 2M PHY.
    #[field(label = "Packets on LE 2M")]
    pub packets_2m: i64,
    /// Good packets on the LE Coded PHY.
    #[field(label = "Packets on LE Coded")]
    pub packets_coded: i64,
    /// The BLE channels inside the capture: the primary ones are received all the time, a secondary one while a packet points
    /// to it. 0 when the centre of the input is not known.
    #[field(label = "Channels in the capture")]
    pub channels_in_capture: i64,
    /// Auxiliary packets reached through the AuxPtr that pointed to them.
    #[field(label = "Auxiliary packets followed")]
    pub aux_followed: i64,
    /// AuxPtr fields that point to a channel outside the capture: their data cannot be received here.
    #[field(label = "Pointers outside the capture")]
    pub aux_outside: i64,
    /// Periodic advertising trains whose packets the receivers take.
    #[field(label = "Periodic trains")]
    pub periodic_trains: i64,
    /// Connections whose events the receivers follow now (from a CONNECT_IND or AUX_CONNECT_REQ heard).
    #[field(label = "Connections followed")]
    pub connections_followed: i64,
    /// Good packets of followed connections.
    #[field(label = "Connection packets")]
    pub connection_packets: i64,
}

/// A row of the devices of `ble`. The row key is the address and "r" (random) or "p" (public).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleDeviceRow<'a> {
    /// The address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The kind of the address.
    #[field(label = "Type")]
    pub address_type: AddressType,
    /// The name, if a packet gave one.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The level of the last packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Packets heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The vendor, if known.
    #[field(label = "Vendor")]
    pub vendor: Option<&'a str>,
    /// The beacon, the services and the TX power, if a packet gave them.
    #[field(label = "Beacon / services")]
    pub beacon: Option<&'a str>,
    /// The channels the device was heard on, as "37 38 39".
    #[field(label = "Channels")]
    pub channels: &'a str,
    /// The device takes connections.
    #[field(label = "Connectable")]
    pub connectable: bool,
    /// Another device sent a scan request to it.
    #[field(label = "Scanned")]
    pub scanned: bool,
    /// The first 32 bytes of manufacturer data after the company identifier, hex, if a packet gave them.
    #[field(label = "Manufacturer data")]
    pub manufacturer_data: Option<&'a str>,
    /// The UTC of the first packet.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The PHYs the device was heard on, as "LE 1M, LE Coded".
    #[field(label = "PHYs")]
    pub phys: &'a str,
    /// The device uses extended advertising.
    #[field(label = "Extended")]
    pub extended: bool,
    /// An AuxPtr of the device points to a channel outside the capture: the row holds what the primary packets gave.
    #[field(label = "Data on a channel outside the capture")]
    pub data_outside: bool,
    /// The advertising set id (SID) of the last extended packet, if one had it.
    #[field(label = "Set")]
    pub set_id: Option<i64>,
    /// The interval of the device's periodic advertising train, if a SyncInfo gave one.
    #[field(label = "Periodic interval", unit = "ms")]
    pub periodic_interval_ms: Option<f64>,
    /// The kind of device its Appearance or beacon format declares.
    #[field(label = "Device kind")]
    pub device_kind: DeviceKind,
    /// Its advertising data matches a rule of the tracker detector of `bletracker` (Find My, Tile, Chipolo, SmartTag,
    /// Google Find Hub).
    #[field(label = "Tracker")]
    pub tracker: bool,
}

/// A row of the packet log of `ble`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BlePacketRow<'a> {
    /// The signal time of the packet.
    #[field(label = "Signal time", unit = "s", role = Time)]
    pub signal_s: f64,
    /// The advertising channel of the packet.
    #[field(label = "Ch")]
    pub channel: i64,
    /// The PDU type name.
    #[field(label = "PDU")]
    pub pdu: &'a str,
    /// The address of the sender, if the PDU has one.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// The level of the packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The payload length.
    #[field(label = "Length", unit = "B")]
    pub length: i64,
    /// The one-line summary.
    #[field(label = "Details")]
    pub details: &'a str,
    /// The PHY of the packet ("LE 1M", "LE 2M", "LE Coded S=8" ...).
    #[field(label = "PHY")]
    pub phy: &'a str,
}

/// A row of the good packets of each PDU type of `ble`. The row key is the PDU type name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BlePduTypeRow<'a> {
    /// The PDU type name.
    #[field(label = "PDU")]
    pub pdu: &'a str,
    /// Good packets of the type.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// A row of the level history of the devices of `ble`. The row key is `<device row>/<slot>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleDeviceLevelRow<'a> {
    /// The row of the device.
    #[field(label = "Device", refers_to = "devices")]
    pub device: &'a str,
    /// The number of the level in the history of its device, counted from the first one heard.
    #[field(label = "Sample")]
    pub sample: i64,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A row of the connections of `ble`, followed from their CONNECT_IND or AUX_CONNECT_REQ. The row key is the access address
/// as eight hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleConnectionRow<'a> {
    /// The access address of the connection, eight hex digits.
    #[field(label = "Access address")]
    pub access_address: &'a str,
    /// The central (the initiator of the connection).
    #[field(label = "Central", refers_to = "devices")]
    pub central: &'a str,
    /// The peripheral (the advertiser it connected to).
    #[field(label = "Peripheral", refers_to = "devices")]
    pub peripheral: &'a str,
    /// The connection interval.
    #[field(label = "Interval", unit = "ms")]
    pub interval_ms: f64,
    /// The peripheral latency, events.
    #[field(label = "Latency")]
    pub latency: i64,
    /// The supervision timeout.
    #[field(label = "Timeout", unit = "ms")]
    pub timeout_ms: f64,
    /// The PHY ("LE 1M", "LE 2M", "LE Coded").
    #[field(label = "PHY")]
    pub phy: &'a str,
    /// The channel selection algorithm (1 or 2). No value until a packet tells.
    #[field(label = "Channel selection")]
    pub csa: Option<i64>,
    /// The data channels its channel map uses.
    #[field(label = "Channels used")]
    pub channels_used: i64,
    /// Packets of the connection heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The connection is encrypted: LL_ENC_REQ or LL_START_ENC_REQ was heard. Its payloads are then not read.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The LL control procedures heard, joined with ", " ("LL_VERSION_IND, LL_ENC_REQ").
    #[field(label = "LL control")]
    pub procedures: &'a str,
    /// ATT PDUs read in the clear.
    #[field(label = "ATT PDUs")]
    pub att: i64,
    /// LL_TERMINATE_IND was heard.
    #[field(label = "Ended")]
    pub ended: bool,
    /// The UTC of the CONNECT_IND.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD, role = Time)]
    pub last_heard: Option<Time>,
}

/// A row of the Attribute Protocol log of `ble`: an ATT PDU sent in the clear on a followed connection.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleAttRow<'a> {
    /// The signal time of the packet.
    #[field(label = "Signal time", unit = "s", role = Time)]
    pub signal_s: f64,
    /// The connection.
    #[field(label = "Connection", refers_to = "connections")]
    pub connection: &'a str,
    /// The central sent it (the first packet of its connection event); false for the peripheral.
    #[field(label = "From central")]
    pub from_central: bool,
    /// The ATT opcode name ("ATT_READ_REQ").
    #[field(label = "Opcode")]
    pub opcode: &'a str,
    /// The attribute handle the PDU names. No value when it names none.
    #[field(label = "Handle")]
    pub handle: Option<i64>,
    /// The GATT service that holds the handle, by its assigned name, when a service discovery on the connection was
    /// heard. No value otherwise.
    #[field(label = "Service")]
    pub service: Option<&'a str>,
    /// The bytes after the opcode and the handle as hex, at most 32 bytes; empty when the setting `att_values` is off.
    #[field(label = "Value")]
    pub value: &'a str,
    /// The number of bytes after the opcode and the handle.
    #[field(label = "Length", unit = "B")]
    pub length: i64,
}

/// The fields of a good packet with an address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleDeviceEvent<'a> {
    /// The address.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The name of the device, if known.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The level of the packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The devices of `ble`.
pub static BLE_DEVICES: Table<BleDeviceRow<'static>> =
    Table::new("devices", "Devices", crate::ble::table::DEVICE_CAP)
        .label(&BleDeviceRow::NAME)
        .sorted(&BleDeviceRow::LEVEL_DBFS, sdrkit_core::SortOrder::Descending)
        .role(sdrkit_core::TableRole::Nodes);
/// The packet log of `ble`, newest first.
pub static BLE_PACKETS: Table<BlePacketRow<'static>> =
    Table::new("packets", "Packets", crate::ble::table::LOG_CAP)
        .label(&BlePacketRow::ADDRESS)
        .role(sdrkit_core::TableRole::Log);
/// The good packets of each PDU type of `ble` since the last clear.
pub static BLE_PDU_TYPES: Table<BlePduTypeRow<'static>> = Table::new("pdu_types", "PDU types", 16);
/// The last levels of each device of `ble`, one row a level.
pub static BLE_DEVICE_LEVELS: Table<BleDeviceLevelRow<'static>> = Table::new(
    "device_levels",
    "Device levels",
    crate::ble::table::DEVICE_CAP * crate::ble::table::HISTORY,
)
.label(&BleDeviceLevelRow::DEVICE);
/// The connections of `ble`, most recently heard first.
pub static BLE_CONNECTIONS: Table<BleConnectionRow<'static>> =
    Table::new("connections", "Connections", crate::ble::conn::CONN_CAP)
        .label(&BleConnectionRow::ACCESS_ADDRESS)
        .sorted(&BleConnectionRow::LAST_HEARD, sdrkit_core::SortOrder::Descending);
/// The ATT log of `ble`, newest first.
pub static BLE_ATT: Table<BleAttRow<'static>> =
    Table::new("att", "ATT", crate::ble::conn::ATT_CAP)
        .label(&BleAttRow::OPCODE)
        .role(sdrkit_core::TableRole::Log);
/// A good packet with an address.
pub static BLE_DEVICE: EventKind<BleDeviceEvent<'static>> =
    EventKind::new("ble.device", "Bluetooth LE device");

/// The row key of a device.
pub const BLE_DEVICE_ROW: Argument<field::Text> = objcmd::row_of("devices");

/// The arguments of `remove` of `ble`.
#[derive(Arguments)]
pub struct BleDeviceArgs<'a> {
    /// The row key of the device.
    #[arg(shared = BLE_DEVICE_ROW)]
    pub row: &'a str,
}

/// Forgets one device.
pub const BLE_REMOVE: CommandKind<BleDeviceArgs<'static>> = objcmd::remove_kind();

/// The result fields of the advertising sender of `ble` and `bletracker`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleTxFields {
    /// Whether a send is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The packets of the send on air that went out.
    #[field(label = "Packets sent")]
    pub tx_sent: i64,
    /// The packets still to go: the rest of the send on air and the sends that wait.
    #[field(label = "Packets left")]
    pub tx_left: i64,
}

/// The fields of a change of an advertising send.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleTxEvent<'a> {
    /// The state: started, queued, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, halted, the transmitter is off, the format changed, the frequency changed, or why a send
    /// was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The advertiser address.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The PDU type, the channel, the packets and the advertising data in hex; empty for finished.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An advertising send.
pub static BLE_TX: EventKind<BleTxEvent<'static>> =
    EventKind::new("ble.tx", "Bluetooth LE transmission");

/// The arguments of `send` of `ble`.
#[derive(Arguments)]
pub struct BleSendArgs<'a> {
    /// The advertiser address.
    #[arg(
        label = "Address",
        help = "The advertiser address: 12 hex digits, most significant byte first, with or without colons (C0:FF:EE:12:34:56).",
        refers_to = "devices",
        column = "address"
    )]
    pub address: &'a str,
    /// Public or random.
    #[arg(shared = crate::ble::tx::ADDRESS_TYPE)]
    pub address_type: Option<crate::ble::tx::SendAddress>,
    /// The PDU type.
    #[arg(shared = crate::ble::tx::PDU_TYPE)]
    pub pdu_type: Option<crate::ble::tx::SendPdu>,
    /// The advertising data.
    #[arg(
        label = "Data",
        help = "The advertising data in hex, up to 31 bytes of AD structures. When given, the name, UUID and manufacturer fields stay empty.",
        default = ""
    )]
    pub data: Option<&'a str>,
    /// The complete local name.
    #[arg(
        label = "Name",
        help = "Without data: a complete local name structure of this text.",
        default = ""
    )]
    pub name: Option<&'a str>,
    /// A 16-bit service UUID.
    #[arg(
        label = "Service UUID",
        help = "Without data: a complete list of 16-bit service UUIDs that holds this one, 4 hex digits (180F).",
        default = ""
    )]
    pub uuid16: Option<&'a str>,
    /// The company identifier.
    #[arg(
        label = "Company",
        help = "Without data: manufacturer specific data of this company identifier, 4 hex digits (004C).",
        default = ""
    )]
    pub company: Option<&'a str>,
    /// The manufacturer specific data.
    #[arg(
        label = "Manufacturer data",
        help = "The bytes after the company identifier, in hex. Needs the company.",
        default = ""
    )]
    pub manufacturer_data: Option<&'a str>,
    /// The packets.
    #[arg(
        label = "Packets",
        help = "The packets of one send.",
        default = 1,
        min = 1,
        max = 1000,
        step = 1
    )]
    pub count: Option<i64>,
    /// The interval.
    #[arg(
        label = "Interval",
        unit = "ms",
        help = "From the start of one packet to the start of the next.",
        default = 100,
        min = 20,
        max = 10000,
        step = 1
    )]
    pub interval_ms: Option<i64>,
}

/// Sends advertising packets of the LE 1M PHY on the advertising channel of the tuned frequency.
pub const BLE_SEND: CommandKind<BleSendArgs<'static>> = CommandKind::new(
    "send",
    "Send an advertisement",
)
.help(
    "Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: 2402, 2426 or 2480 MHz. \
     A send during another waits for it. Transmit must be on.",
)
.transmits(sdrkit_core::TxContent::Beacon)
.stopped_by("halt");

/// Ends a send.
pub const BLE_HALT: Command = Command::new("halt", "Halt")
    .help("Drops the packets of the send on air that are still to go, and the sends that wait.");

// bletracker

/// The result fields of `bletracker`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BletrackerFields<'a> {
    /// Trackers that follow the radio now.
    #[field(label = "Following now")]
    pub following_now: i64,
    /// Tracker advertisements taken since the last clear.
    #[field(label = "Tracker packets")]
    pub tracker_packets: i64,
    /// Trackers heard in the last minute.
    #[field(label = "Trackers now", reading = 1)]
    pub trackers_now: i64,
    /// Trackers in the table.
    #[field(label = "Trackers", reading = 2)]
    pub trackers: i64,
    /// How long a tracker must stay before the alert.
    #[field(label = "Alert after", unit = "s")]
    pub alert_after: Seconds,
    /// The averaged level from which a tracker counts as near.
    #[field(label = "Near above", unit = "dBFS")]
    pub near_dbfs: f64,
    /// The signal time of the book.
    #[field(label = "Signal time", unit = "s")]
    pub now_s: f64,
    /// Good packets.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Exact access-address matches whose CRC failed on every channel.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The running mean of the eye SNR. No value before the first good packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good packet. No value before it.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the channel. No value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The tuned advertising channel. No value when it is not known, as for a recording.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// What the detector cannot tell.
    #[field(label = "Limits")]
    pub limits: &'a str,
    /// The privacy note.
    #[field(label = "Privacy")]
    pub privacy: &'a str,
}

/// A row of the trackers of `bletracker`. The row key is the tracker id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BletrackerTrackerRow<'a> {
    /// The label of the tracker: its kind and the last two address bytes.
    #[field(label = "Tracker")]
    pub tracker: &'a str,
    /// The kind, if recognised.
    #[field(label = "Kind")]
    pub kind: Option<TrackerKind>,
    /// What the tracker does now.
    #[field(label = "State")]
    pub state: TrackerState,
    /// How long it stayed with the radio.
    #[field(label = "Stays", unit = "s")]
    pub stays: Seconds,
    /// The share of the time it was near.
    #[field(label = "Near", unit = "%")]
    pub near_pct: f64,
    /// The averaged level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The highest level.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// Whether the level rises or falls.
    #[field(label = "Trend")]
    pub trend: Trend,
    /// The addresses it used.
    #[field(label = "Addresses")]
    pub addresses: i64,
    /// The signal time it was last heard.
    #[field(label = "Last heard", unit = "s")]
    pub last_s: f64,
    /// The UTC of the last sighting, for a live source.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Packets heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// It says it is away from its owner.
    #[field(label = "Separated")]
    pub separated: bool,
    /// The status byte, if the advertisement has one.
    #[field(label = "Status")]
    pub status: Option<i64>,
    /// It follows the radio.
    #[field(label = "Following")]
    pub following: bool,
    /// It is marked as the user's own.
    #[field(label = "Mine")]
    pub mine: bool,
    /// The tracker id.
    #[field(label = "Id")]
    pub id: i64,
    /// The mode that the advertised UUID tells: Tile activated or shipping, SmartTag registered or unregistered, Chipolo
    /// disconnected from its owner. No value for other families.
    #[field(label = "Mode")]
    pub mode: Option<TagMode>,
    /// The SmartTag tag state. No value for the codes 0 and 7, which have no name, and for other families.
    #[field(label = "Tag state")]
    pub tag_state: Option<SmartTagState>,
    /// The SmartTag tag state code, 0 to 7.
    #[field(label = "Tag state code")]
    pub tag_state_code: Option<i64>,
    /// The SmartTag aging counter: 15-minute steps of the tag's clock from 2020-07-02 00:00 UTC.
    #[field(label = "Aging counter")]
    pub aging_counter: Option<i64>,
    /// The SmartTag region ID, 0 to 15.
    #[field(label = "Region")]
    pub region: Option<i64>,
    /// The SmartTag E2E flag: finders must encrypt their location reports.
    #[field(label = "E2E")]
    pub e2e: Option<bool>,
    /// The SmartTag UWB flag.
    #[field(label = "UWB")]
    pub uwb: Option<bool>,
    /// The SmartTag battery code, 0 to 3.
    #[field(label = "Battery code")]
    pub battery: Option<i64>,
    /// The rolling identifier in hex, opaque: the SmartTag privacy ID.
    #[field(label = "Rolling identifier")]
    pub identifier: Option<&'a str>,
    /// The service data value under the family's UUID in hex, as sent (Tile, Chipolo, SmartTag).
    #[field(label = "Service data")]
    pub service_data: Option<&'a str>,
}

/// A row of the alerts of `bletracker`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BletrackerAlertRow<'a> {
    /// The UTC of the alert, for a live source.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time of the alert.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The level of the tracker.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The text of the alert.
    #[field(label = "Alert")]
    pub alert: &'a str,
}

/// A row of the trackers heard of each kind of `bletracker`. The row key is the key of the kind.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BletrackerKindRow {
    /// The kind.
    #[field(label = "Kind")]
    pub kind: TrackerKind,
    /// Trackers of the kind heard in the last minute.
    #[field(label = "In the last minute")]
    pub last_minute: i64,
    /// Trackers of the kind heard in the last 30 minutes.
    #[field(label = "In the last 30 min")]
    pub last_30_min: i64,
}

/// The fields of an unwanted tracking alert.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BletrackerAlertEvent<'a> {
    /// The label of the tracker.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The kind, if recognised.
    #[field(label = "Kind")]
    pub kind: Option<TrackerKind>,
    /// The text of the alert.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// How long it stayed with the radio.
    #[field(label = "Stays", unit = "s")]
    pub stays: Seconds,
    /// The level of the tracker.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Urgent for a tracker away from its owner, else a notice.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The family of the source: an alert.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The trackers of `bletracker`, the ones that follow first.
pub static BLETRACKER_TRACKERS: Table<BletrackerTrackerRow<'static>> =
    Table::new("trackers", "Trackers", crate::tracker::book::RECORD_CAP)
        .label(&BletrackerTrackerRow::IDENTIFIER)
        .sorted(&BletrackerTrackerRow::LEVEL_DBFS, sdrkit_core::SortOrder::Descending)
        .role(sdrkit_core::TableRole::Nodes);
/// The alerts of `bletracker`, newest first.
pub static BLETRACKER_ALERTS: Table<BletrackerAlertRow<'static>> =
    Table::new("alerts", "Alerts", crate::tracker::book::ALERT_CAP);
/// The trackers heard of each kind of `bletracker`.
pub static BLETRACKER_KINDS: Table<BletrackerKindRow> =
    Table::new("tracker_kinds", "Tracker kinds", 5);
/// A tracker stays with the radio.
pub static BLETRACKER_ALERT: EventKind<BletrackerAlertEvent<'static>> =
    EventKind::new("ble.tracker", "Unwanted tracking alert");

/// The row key of a tracker.
pub const BLETRACKER_ROW: Argument<field::Text> = objcmd::row_of("trackers");
/// Yes marks the tracker as the user's own, no takes the mark away.
pub const BLETRACKER_MINE_ARG: Setting<field::Bool> = Setting::new("mine", "Mine", "", true)
    .help("Yes marks the tracker as the user's own, no takes the mark away.");

/// The arguments of `mine` of `bletracker`.
#[derive(Arguments)]
pub struct BletrackerMineArgs<'a> {
    /// The row key of the tracker.
    #[arg(shared = BLETRACKER_ROW)]
    pub row: &'a str,
    /// Mark or unmark.
    #[arg(shared = BLETRACKER_MINE_ARG)]
    pub mine: Option<bool>,
}

/// Marks a tracker as the user's own: it raises no alert.
pub const BLETRACKER_MINE: CommandKind<BletrackerMineArgs<'static>> =
    CommandKind::new("mine", "Mark as mine")
        .help("A tracker marked as the user's own raises no alert.");

/// The arguments of `send` of `bletracker`.
#[derive(Arguments)]
pub struct BletrackerSendArgs<'a> {
    /// The tracker family.
    #[arg(shared = crate::ble::tx::TRACKER)]
    pub tracker: Option<crate::ble::tx::SendTracker>,
    /// The advertiser address.
    #[arg(
        label = "Address",
        help = "The advertiser address: 12 hex digits, most significant byte first, with or without colons (C0:FF:EE:12:34:56)."
    )]
    pub address: &'a str,
    /// Public or random.
    #[arg(shared = crate::ble::tx::ADDRESS_TYPE)]
    pub address_type: Option<crate::ble::tx::SendAddress>,
    /// The PDU type.
    #[arg(shared = crate::ble::tx::PDU_TYPE)]
    pub pdu_type: Option<crate::ble::tx::SendPdu>,
    /// The Find My status byte.
    #[arg(
        label = "Status",
        help = "Find My: the status byte after the type and the length.",
        default = 0,
        min = 0,
        max = 255,
        step = 1
    )]
    pub status: Option<i64>,
    /// The Google frame type.
    #[arg(
        label = "Unwanted-tracking protection",
        help = "Google: frame type 0x41 in place of 0x40.",
        default = false
    )]
    pub protection: Option<bool>,
    /// The rest of the frame.
    #[arg(
        label = "Payload",
        help = "In hex. Find My: the 24 bytes after the status. Google: up to 26 bytes after the frame type.",
        default = ""
    )]
    pub payload: Option<&'a str>,
    /// The packets.
    #[arg(
        label = "Packets",
        help = "The packets of one send.",
        default = 1,
        min = 1,
        max = 1000,
        step = 1
    )]
    pub count: Option<i64>,
    /// The interval.
    #[arg(
        label = "Interval",
        unit = "ms",
        help = "From the start of one packet to the start of the next.",
        default = 100,
        min = 20,
        max = 10000,
        step = 1
    )]
    pub interval_ms: Option<i64>,
    /// The Tile shipping mode.
    #[arg(
        label = "Shipping mode",
        help = "Tile: the UUID list holds 0xFEEC, a Tile not yet activated, and no service data goes out. Else the UUID list holds 0xFEED and the service data follows.",
        default = false
    )]
    pub shipping: Option<bool>,
    /// The service data value of Tile and Chipolo.
    #[arg(
        label = "Service data",
        help = "Tile (activated) and Chipolo: the service data value after the UUID, in hex, 1 to 23 bytes. It goes out as is.",
        default = ""
    )]
    pub service_data: Option<&'a str>,
    /// The SmartTag tag state.
    #[arg(shared = crate::ble::tx::TAG_STATE)]
    pub tag_state: Option<SmartTagState>,
    /// The SmartTag aging counter.
    #[arg(
        label = "Aging counter",
        help = "SmartTag: the tag's clock in 15-minute steps, (UTC seconds - 1593648000) / 900.",
        default = 0,
        min = 0,
        max = 16777215,
        step = 1
    )]
    pub aging_counter: Option<i64>,
    /// The SmartTag privacy ID.
    #[arg(
        label = "Privacy ID",
        help = "SmartTag: the rolling identifier, 8 bytes in hex.",
        default = ""
    )]
    pub privacy_id: Option<&'a str>,
    /// The SmartTag region ID.
    #[arg(
        label = "Region",
        help = "SmartTag: the region ID, 0 to 15.",
        default = 0,
        min = 0,
        max = 15,
        step = 1
    )]
    pub region: Option<i64>,
    /// The SmartTag E2E flag.
    #[arg(
        label = "E2E",
        help = "SmartTag: the E2E flag, finders must encrypt their location reports.",
        default = false
    )]
    pub e2e: Option<bool>,
    /// The SmartTag UWB flag.
    #[arg(label = "UWB", help = "SmartTag: the UWB flag.", default = false)]
    pub uwb: Option<bool>,
    /// The SmartTag battery code.
    #[arg(
        label = "Battery code",
        help = "SmartTag: the battery code, 0 to 3.",
        default = 3,
        min = 0,
        max = 3,
        step = 1
    )]
    pub battery: Option<i64>,
    /// The SmartTag signature.
    #[arg(
        label = "Signature",
        help = "SmartTag: the signature, 4 bytes in hex.",
        default = ""
    )]
    pub signature: Option<&'a str>,
}

/// Sends tracker advertisements in the form that the detector reads.
pub const BLETRACKER_SEND: CommandKind<BletrackerSendArgs<'static>> = CommandKind::new(
    "send",
    "Send a tracker advertisement",
)
.help(
    "Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: Find My manufacturer \
     data of company 0x004C (type 0x12, length 25, the status, the payload); Google service data of UUID 0xFEAA (frame \
     type 0x40 or 0x41, the payload); Tile, a UUID list of 0xFEED and the service data under 0xFEED, or in shipping mode \
     a UUID list of 0xFEEC alone; Chipolo, a UUID list of 0xFE33 and the service data under 0xFE33; SmartTag, the 20-byte \
     service data of UUID 0xFD5A from the tag state, the aging counter, the privacy ID, the region, the E2E and UWB \
     flags, the battery code and the signature. No flags structure goes out. A send during another waits for it. \
     Transmit must be on.",
)
.transmits(sdrkit_core::TxContent::Beacon)
.stopped_by("halt");

// wifi

/// The result fields of `wifi`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiFields<'a> {
    /// Networks in the table.
    #[field(label = "Networks", reading = 1)]
    pub networks: i64,
    /// Frames with a good FCS.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames with a bad FCS.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// DSSS SFDs found.
    #[field(label = "DSSS preambles")]
    pub dsss_sfds: i64,
    /// DSSS PLCP headers with a good CRC.
    #[field(label = "DSSS headers good")]
    pub dsss_headers_ok: i64,
    /// DSSS PLCP headers with a bad CRC.
    #[field(label = "DSSS headers bad")]
    pub dsss_headers_bad: i64,
    /// DSSS headers of a CCK rate, not decoded.
    #[field(label = "CCK headers")]
    pub dsss_cck_headers: i64,
    /// OFDM short preambles found.
    #[field(label = "OFDM preambles")]
    pub ofdm_preambles: i64,
    /// OFDM SIGNAL fields with a good parity.
    #[field(label = "SIGNAL good")]
    pub ofdm_signal_ok: i64,
    /// OFDM SIGNAL fields with a bad parity or rate.
    #[field(label = "SIGNAL bad")]
    pub ofdm_signal_bad: i64,
    /// Management frames.
    #[field(label = "Management frames")]
    pub mgmt_frames: i64,
    /// Control frames.
    #[field(label = "Control frames")]
    pub control_frames: i64,
    /// Data frames.
    #[field(label = "Data frames")]
    pub data_frames: i64,
    /// Beacons.
    #[field(label = "Beacons")]
    pub beacons: i64,
    /// Probe responses.
    #[field(label = "Probe responses")]
    pub probe_responses: i64,
    /// FILS discovery frames.
    #[field(label = "FILS frames")]
    pub fils_frames: i64,
    /// Frames heard again on a second channel at once.
    #[field(label = "Duplicates")]
    pub duplicate_frames: i64,
    /// The band of the tuned frequency, if it is in one.
    #[field(label = "Band")]
    pub band: Option<Band>,
    /// The channel of the tuned frequency, if it is one.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The 20 MHz channels that the stack reads at once.
    #[field(label = "Channels at once")]
    pub sub_channels: i64,
    /// The PHY of the last good frame. No value before it.
    #[field(label = "PHY")]
    pub phy: Option<PhyKind>,
    /// The running mean of the SNR. No value before the first good frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good frame. No value before it.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the input. No value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The host steps through the hop list.
    #[field(label = "Hopping")]
    pub hopping: bool,
    /// The passive note.
    #[field(label = "Passive")]
    pub passive: &'a str,
    /// Client stations in the table.
    #[field(label = "Stations", reading = 2)]
    pub stations: i64,
    /// HT PPDUs whose signal field was read since the last clear.
    #[field(label = "HT PPDUs")]
    pub ppdu_ht: i64,
    /// VHT PPDUs whose signal field was read since the last clear.
    #[field(label = "VHT PPDUs")]
    pub ppdu_vht: i64,
    /// HE SU PPDUs whose signal field was read since the last clear.
    #[field(label = "HE SU PPDUs")]
    pub ppdu_he_su: i64,
    /// HE ER SU PPDUs whose signal field was read since the last clear.
    #[field(label = "HE ER SU PPDUs")]
    pub ppdu_he_er_su: i64,
    /// HE MU PPDUs whose signal field was read since the last clear.
    #[field(label = "HE MU PPDUs")]
    pub ppdu_he_mu: i64,
    /// HE TB PPDUs whose signal field was read since the last clear.
    #[field(label = "HE TB PPDUs")]
    pub ppdu_he_tb: i64,
    /// HE with a bad HE-SIG-A PPDUs whose signal field was read since the last clear.
    #[field(label = "HE with a bad HE-SIG-A PPDUs")]
    pub ppdu_he_bad_sig: i64,
    /// EHT PPDUs whose signal field was read since the last clear.
    #[field(label = "EHT PPDUs")]
    pub ppdu_eht: i64,
    /// HT, VHT, HE and EHT PPDUs whose data was decoded.
    #[field(label = "PPDUs decoded")]
    pub ppdu_decoded: i64,
    /// PPDUs not decoded because of two or more spatial streams.
    #[field(label = "PPDUs not decoded: two or more spatial streams")]
    pub ppdu_not_decoded_streams: i64,
    /// PPDUs not decoded because of wider than 20 MHz.
    #[field(label = "PPDUs not decoded: wider than 20 MHz")]
    pub ppdu_not_decoded_width: i64,
    /// PPDUs not decoded because of LDPC coding.
    #[field(label = "PPDUs not decoded: LDPC coding")]
    pub ppdu_not_decoded_ldpc: i64,
    /// PPDUs not decoded because of STBC.
    #[field(label = "PPDUs not decoded: STBC")]
    pub ppdu_not_decoded_stbc: i64,
    /// PPDUs not decoded because of multi-user.
    #[field(label = "PPDUs not decoded: multi-user")]
    pub ppdu_not_decoded_multi_user: i64,
    /// PPDUs not decoded because of trigger-based.
    #[field(label = "PPDUs not decoded: trigger-based")]
    pub ppdu_not_decoded_trigger_based: i64,
    /// PPDUs not decoded because of HE data.
    #[field(label = "PPDUs not decoded: HE data")]
    pub ppdu_not_decoded_he_data: i64,
    /// PPDUs not decoded because of EHT data.
    #[field(label = "PPDUs not decoded: EHT data")]
    pub ppdu_not_decoded_eht_data: i64,
    /// PPDUs not decoded because of a bad signal field.
    #[field(label = "PPDUs not decoded: a bad signal field")]
    pub ppdu_not_decoded_signal_field: i64,
    /// PPDUs not decoded because of an MCS that is not decoded.
    #[field(label = "PPDUs not decoded: an MCS that is not decoded")]
    pub ppdu_not_decoded_mcs: i64,
    /// PPDUs not decoded because of a length that does not fit.
    #[field(label = "PPDUs not decoded: a length that does not fit")]
    pub ppdu_not_decoded_length: i64,
    /// PPDUs not decoded because of data that did not decode.
    #[field(label = "PPDUs not decoded: data that did not decode")]
    pub ppdu_not_decoded_data: i64,
    /// PPDUs not decoded because of another reason.
    #[field(label = "PPDUs not decoded: another reason")]
    pub ppdu_not_decoded_other: i64,
    /// PPDUs not decoded because of the decoder was busy.
    #[field(label = "PPDUs not decoded: the decoder was busy")]
    pub ppdu_not_decoded_busy: i64,
    /// A-MPDU delimiters with a bad CRC or signature since the last clear.
    #[field(label = "A-MPDU bad delimiters")]
    pub ampdu_bad_delimiters: i64,
    /// Probe requests since the last clear.
    #[field(label = "Probe requests")]
    pub probe_requests: i64,
    /// Authentication frames since the last clear.
    #[field(label = "Authentications")]
    pub authentications: i64,
    /// Association and reassociation frames since the last clear.
    #[field(label = "Associations")]
    pub associations: i64,
    /// Deauthentication and disassociation frames since the last clear.
    #[field(label = "Departures")]
    pub departures: i64,
    /// Action frames since the last clear.
    #[field(label = "Actions")]
    pub actions: i64,
    /// RTS frames since the last clear.
    #[field(label = "RTS")]
    pub rts: i64,
    /// CTS frames since the last clear.
    #[field(label = "CTS")]
    pub cts: i64,
    /// ACK frames since the last clear.
    #[field(label = "ACKs")]
    pub acks: i64,
    /// Block Ack frames and requests since the last clear.
    #[field(label = "Block Acks")]
    pub block_acks: i64,
}

/// A row of the networks of `wifi`. The row key is the BSSID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiNetworkRow<'a> {
    /// The SSID, or "(hidden)".
    #[field(label = "SSID")]
    pub ssid: &'a str,
    /// The BSSID.
    #[field(label = "BSSID")]
    pub bssid: &'a str,
    /// The vendor of the BSSID, or "(local)" for a locally administered one.
    #[field(label = "Vendor")]
    pub vendor: &'a str,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<Band>,
    /// The primary channel. No value when the frames give none and the receiver does not know its own channel.
    #[field(label = "Ch")]
    pub channel: Option<i64>,
    /// The channel the frames were heard on. No value when the receiver does not know it, as for a recording.
    #[field(label = "Heard on")]
    pub heard_on: Option<i64>,
    /// The centre frequency of the whole channel (all its 20 MHz parts). No value when the channel is not known.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Option<Hertz>,
    /// The channel width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The newest amendment the network declares. No value when the band and the elements do not tell.
    #[field(label = "Generation")]
    pub generation: Option<WifiGeneration>,
    /// The security.
    #[field(label = "Security")]
    pub security: WifiSecurity,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD, role = Time)]
    pub last_heard: Option<Time>,
    /// Beacons heard.
    #[field(label = "Beacons")]
    pub beacons: i64,
    /// Probe responses heard.
    #[field(label = "Probe responses")]
    pub probe_responses: i64,
    /// The beacon interval.
    #[field(label = "Interval", unit = "s")]
    pub interval: Seconds,
    /// The pairwise ciphers.
    #[field(label = "Ciphers")]
    pub ciphers: &'a str,
    /// The key management.
    #[field(label = "Key management")]
    pub akm: &'a str,
    /// Protected management frames: "required", "capable" or "".
    #[field(label = "PMF")]
    pub pmf: &'a str,
    /// The capability bits as text.
    #[field(label = "Capability")]
    pub capability: &'a str,
    /// The other elements as text.
    #[field(label = "Details")]
    pub details: &'a str,
    /// Client stations heard with this BSSID.
    #[field(label = "Stations")]
    pub stations: i64,
}

/// A row of the frame log of `wifi`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiBeaconRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel the frame was heard on. No value when the receiver does not know it.
    #[field(label = "Ch")]
    pub channel: Option<i64>,
    /// The BSSID.
    #[field(label = "BSSID")]
    pub bssid: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The length, FCS included.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The summary.
    #[field(label = "Details")]
    pub details: &'a str,
    /// The PPDU format of the frame ("HT", "VHT"); no value for DSSS and non-HT OFDM.
    #[field(label = "Format")]
    pub format: Option<&'a str>,
    /// The MCS of an HT or VHT frame.
    #[field(label = "MCS")]
    pub mcs: Option<i64>,
    /// The frame came in an A-MPDU.
    #[field(label = "A-MPDU")]
    pub aggregated: bool,
}

sdrkit_core::choice_enum! {
    /// How far a client station got with its access point. Standard: IEEE Std 802.11-2020 clause 11.3.
    pub enum StationLink {
        /// No frame ties it to an access point.
        Unknown = ("unknown", "Unknown"),
        /// Probe requests only.
        Probing = ("probing", "Probing"),
        /// Authentication or an association request was seen.
        Joining = ("joining", "Joining"),
        /// Associated: a successful association response, or data to or from the access point.
        Associated = ("associated", "Associated"),
        /// A deauthentication or disassociation was seen last.
        Left = ("left", "Left"),
    }
}

impl From<crate::wifi::stations::Link> for StationLink {
    fn from(l: crate::wifi::stations::Link) -> Self {
        use crate::wifi::stations::Link;
        match l {
            Link::Unknown => StationLink::Unknown,
            Link::Probing => StationLink::Probing,
            Link::Joining => StationLink::Joining,
            Link::Associated => StationLink::Associated,
            Link::Left => StationLink::Left,
        }
    }
}

/// A row of the client stations of `wifi`. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiStationRow<'a> {
    /// The address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The vendor of the address.
    #[field(label = "Vendor")]
    pub vendor: &'a str,
    /// The address is locally administered (randomised).
    #[field(label = "Randomised")]
    pub randomised: bool,
    /// The BSSID of its access point, if a frame tied it to one.
    #[field(label = "BSSID", refers_to = "networks")]
    pub bssid: Option<&'a str>,
    /// How far it got with its access point.
    #[field(label = "Link")]
    pub link: StationLink,
    /// The UTC of the last frame, for a live source.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The level of the last frame it sent.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: Option<f64>,
    /// Frames it sent.
    #[field(label = "Sent")]
    pub sent: i64,
    /// Frames sent to it.
    #[field(label = "Received")]
    pub received: i64,
    /// Probe requests it sent.
    #[field(label = "Probes")]
    pub probes: i64,
    /// Data frames to or from it.
    #[field(label = "Data")]
    pub data: i64,
    /// Null data frames it sent.
    #[field(label = "Null data")]
    pub null_data: i64,
    /// Protected frames to or from it.
    #[field(label = "Protected")]
    pub protected: i64,
    /// The SSIDs it probed for, joined with ", ".
    #[field(label = "SSIDs")]
    pub ssids: &'a str,
    /// The newest amendment its frames declare. No value when none does.
    #[field(label = "Generation")]
    pub generation: Option<WifiGeneration>,
    /// The highest data rate its frames declare.
    #[field(label = "Max rate", unit = "Mbit/s")]
    pub max_rate_mbps: Option<f64>,
    /// The channel of its last frame (no value when not known).
    #[field(label = "Ch")]
    pub channel: Option<i64>,
}

/// A row of the good OFDM frames at each rate of `wifi`. The row key is the rate in Mbit/s.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiRateRow {
    /// The rate.
    #[field(label = "Rate", unit = "Mbit/s")]
    pub rate_mbps: f64,
    /// Good frames at the rate.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// A row of the summary of each band of `wifi`. The row key is the key of the band.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiBandRow {
    /// The band.
    #[field(label = "Band")]
    pub band: Band,
    /// Networks in the band.
    #[field(label = "Networks")]
    pub networks: i64,
    /// Beacons heard in the band.
    #[field(label = "Beacons")]
    pub beacons: i64,
    /// Networks of 802.11ax or 802.11be.
    #[field(label = "ax / be")]
    pub ax_be: i64,
    /// Networks with WPA3 or OWE.
    #[field(label = "WPA3 / OWE")]
    pub wpa3_owe: i64,
    /// Open networks.
    #[field(label = "Open")]
    pub open: i64,
    /// The level of the strongest network.
    #[field(label = "Strongest", unit = "dBFS")]
    pub strongest_dbfs: f64,
}

/// The networks of `wifi`.
pub static WIFI_NETWORKS: Table<WifiNetworkRow<'static>> =
    Table::new("networks", "Networks", crate::wifi::table::NETWORK_CAP)
        .label(&WifiNetworkRow::SSID)
        .sorted(
            &WifiNetworkRow::LEVEL_DBFS,
            sdrkit_core::SortOrder::Descending,
        )
        .role(sdrkit_core::TableRole::Nodes);
/// The client stations of `wifi`, most recently heard first. The networks are the nodes of the stack (one table of that
/// role); a station names its network in `bssid`.
pub static WIFI_STATIONS: Table<WifiStationRow<'static>> =
    Table::new("stations", "Stations", crate::wifi::stations::STATION_CAP)
        .label(&WifiStationRow::ADDRESS)
        .sorted(&WifiStationRow::LAST_HEARD, sdrkit_core::SortOrder::Descending);
/// The frame log of `wifi`, newest first.
pub static WIFI_BEACONS: Table<WifiBeaconRow<'static>> =
    Table::new("beacons", "Beacons", crate::wifi::table::LOG_CAP).label(&WifiBeaconRow::BSSID);
/// The good OFDM frames at each rate of `wifi` since the last clear.
pub static WIFI_OFDM_RATES: Table<WifiRateRow> = Table::new("ofdm_rates", "OFDM frames by rate", 8);
/// The summary of each band of `wifi`.
pub static WIFI_BANDS: Table<WifiBandRow> = Table::new("bands", "Bands", 3);

/// The row key of a network.
pub const NETWORK_ROW: Argument<field::Text> = objcmd::row_of("networks");

/// The arguments of `remove` of `wifi` and `halow`.
#[derive(Arguments)]
pub struct NetworkArgs<'a> {
    /// The row key of the network.
    #[arg(shared = NETWORK_ROW)]
    pub row: &'a str,
}

/// Forgets one network.
pub const NETWORK_REMOVE: CommandKind<NetworkArgs<'static>> = objcmd::remove_kind();

// espnow

/// The result fields of `espnow`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowFields<'a> {
    /// Devices in the table.
    #[field(label = "Devices", reading = 1)]
    pub devices: i64,
    /// Wi-Fi frames with a good FCS.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Wi-Fi frames with a bad FCS.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Management action frames.
    #[field(label = "Action frames")]
    pub action_frames: i64,
    /// ESP-NOW frames.
    #[field(label = "ESP-NOW frames")]
    pub espnow_frames: i64,
    /// ESP-NOW frames that were protected.
    #[field(label = "Encrypted")]
    pub encrypted: i64,
    /// Protected frames that a key opened.
    #[field(label = "Decrypted")]
    pub decrypted: i64,
    /// ESP-NOW frames to the broadcast address.
    #[field(label = "Broadcast")]
    pub broadcast: i64,
    /// An ESP-NOW frame came in the last 5 s.
    #[field(label = "Heard")]
    pub heard: bool,
    /// The channel nearest the radio centre. No value when the centre is not known.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The centre frequency of the channel. No value when the centre is not known.
    #[field(label = "Channel frequency")]
    pub channel_hz: Option<Hertz>,
    /// The offset of the channel's centre from the radio centre. No value when the centre is not known.
    #[field(label = "Channel offset")]
    pub channel_offset_hz: Option<Hertz>,
    /// The level of the last good frame. No value before it.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The SNR of the last good frame. No value before it.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good frame. No value before it.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The input rate.
    #[field(label = "Input rate")]
    pub input_rate_hz: Hertz,
    /// DSSS SFDs found.
    #[field(label = "DSSS preambles")]
    pub dsss_sfds: i64,
    /// DSSS PLCP headers with a good CRC.
    #[field(label = "DSSS headers good")]
    pub dsss_headers_ok: i64,
    /// DSSS PLCP headers with a bad CRC.
    #[field(label = "DSSS headers bad")]
    pub dsss_headers_bad: i64,
    /// DSSS headers of a CCK rate, not decoded.
    #[field(label = "CCK headers")]
    pub dsss_cck_headers: i64,
    /// OFDM short preambles found.
    #[field(label = "OFDM preambles")]
    pub ofdm_preambles: i64,
    /// OFDM SIGNAL fields with a good parity.
    #[field(label = "SIGNAL good")]
    pub ofdm_signal_ok: i64,
    /// OFDM SIGNAL fields with a bad parity or rate.
    #[field(label = "SIGNAL bad")]
    pub ofdm_signal_bad: i64,
    /// The host steps through the hop list.
    #[field(label = "Hopping")]
    pub hopping: bool,
    /// What is decoded, what is only detected, and what no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the devices of `espnow`. The row key is the sender address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowDeviceRow<'a> {
    /// The sender address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The maker of the address, or "(local)" for a locally administered one.
    #[field(label = "Vendor")]
    pub vendor: &'a str,
    /// The address is one of Espressif.
    #[field(label = "Espressif")]
    pub espressif: bool,
    /// ESP-NOW frames heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Protected frames heard.
    #[field(label = "Encrypted")]
    pub encrypted: i64,
    /// Frames to the broadcast address.
    #[field(label = "Broadcasts")]
    pub broadcasts: i64,
    /// Packets per second over the present window of 1 to 10 s; 0 after 10 s unheard.
    #[field(label = "Rate", unit = "1/s")]
    pub rate: f64,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The channel of the last frame. No value when the receiver does not know it.
    #[field(label = "Ch")]
    pub channel: Option<i64>,
    /// The PHY rate of the last frame.
    #[field(label = "PHY rate", unit = "kbit/s")]
    pub phy_rate_kbps: i64,
    /// "broadcast" or the destination of the last frame.
    #[field(label = "Last to")]
    pub last_to: &'a str,
    /// The payload of the last frame as hex; empty when it was encrypted.
    #[field(label = "Last payload")]
    pub last_payload: &'a str,
    /// The ESP-NOW version of the last readable frame. No value before one.
    #[field(label = "Version")]
    pub version: Option<i64>,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD, role = Time)]
    pub last_heard: Option<Time>,
    /// The signal time of the first frame.
    #[field(label = "First heard (signal)", unit = "s")]
    pub first_s: f64,
    /// The signal time of the last frame.
    #[field(label = "Last heard (signal)", unit = "s")]
    pub last_s: f64,
}

/// A row of the frame log of `espnow`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowFrameRow<'a> {
    /// The UTC of the frame. No value for a recording.
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel. No value when the receiver does not know it.
    #[field(label = "Ch")]
    pub channel: Option<i64>,
    /// The PHY rate.
    #[field(label = "PHY rate", unit = "kbit/s")]
    pub phy_rate_kbps: i64,
    /// A DSSS frame with the short preamble.
    #[field(label = "Short preamble")]
    pub short_preamble: bool,
    /// The sender address.
    #[field(label = "From")]
    pub from: &'a str,
    /// "broadcast" or the destination address.
    #[field(label = "To")]
    pub to: &'a str,
    /// The sequence number.
    #[field(label = "Seq")]
    pub seq: i64,
    /// The ESP-NOW version. No value for a frame that stayed encrypted.
    #[field(label = "Version")]
    pub version: Option<i64>,
    /// The frame was protected.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// A key opened the frame.
    #[field(label = "Decrypted")]
    pub decrypted: bool,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The payload as hex; empty when it stayed encrypted.
    #[field(label = "Payload")]
    pub payload: &'a str,
    /// The summary.
    #[field(label = "Details")]
    pub details: &'a str,
}

/// A row of the ESP-NOW frames at each PHY rate of `espnow`. The row key is the key of the rate.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowRateRow<'a> {
    /// The PHY rate group.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// ESP-NOW frames at the rate.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// A row of the ESP-NOW frames on each channel of `espnow`. The row key is the channel number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowChannelRow {
    /// The channel.
    #[field(label = "Ch")]
    pub channel: i64,
    /// ESP-NOW frames on the channel.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// The fields of an ESP-NOW frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowFrameEvent<'a> {
    /// The sender address.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// "broadcast" or the destination address.
    #[field(label = "To")]
    pub to: &'a str,
    /// The summary.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a device heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EspnowDeviceEvent<'a> {
    /// The sender address.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The maker of the address.
    #[field(label = "Vendor")]
    pub vendor: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The devices of `espnow`, most recently heard first.
pub static ESPNOW_DEVICES: Table<EspnowDeviceRow<'static>> =
    Table::new("devices", "Devices", crate::espnow::table::DEVICE_CAP)
        .label(&EspnowDeviceRow::ADDRESS)
        .sorted(&EspnowDeviceRow::LEVEL_DBFS, sdrkit_core::SortOrder::Descending)
        .role(sdrkit_core::TableRole::Nodes);
/// The frame log of `espnow`, newest first.
pub static ESPNOW_FRAMES: Table<EspnowFrameRow<'static>> =
    Table::new("frames", "Frames", crate::espnow::table::LOG_CAP).label(&EspnowFrameRow::FROM);
/// The ESP-NOW frames at each PHY rate of `espnow` since the last clear.
pub static ESPNOW_BY_RATE: Table<EspnowRateRow<'static>> =
    Table::new("by_rate", "Frames by PHY rate", 5);
/// The ESP-NOW frames on each channel of `espnow` since the last clear.
pub static ESPNOW_BY_CHANNEL: Table<EspnowChannelRow> =
    Table::new("by_channel", "Frames by channel", 14);
/// One ESP-NOW frame.
pub static ESPNOW_FRAME: EventKind<EspnowFrameEvent<'static>> =
    EventKind::new("espnow.frame", "ESP-NOW frame");
/// An ESP-NOW device heard for the first time.
pub static ESPNOW_DEVICE: EventKind<EspnowDeviceEvent<'static>> =
    EventKind::new("espnow.device", "New ESP-NOW device");

// halow

/// The result fields of `halow`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowFields<'a> {
    /// Networks in the table.
    #[field(label = "Networks", reading = 1)]
    pub networks: i64,
    /// Frames with a good FCS.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames with a bad FCS.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// PPDUs read from their SIG field only.
    #[field(label = "Only counted")]
    pub undecoded: i64,
    /// PPDUs with several spatial streams.
    #[field(label = "Several streams")]
    pub mimo_frames: i64,
    /// PPDUs with STBC.
    #[field(label = "STBC")]
    pub stbc_frames: i64,
    /// PPDUs with LDPC coding.
    #[field(label = "LDPC")]
    pub ldpc_frames: i64,
    /// A-MPDUs.
    #[field(label = "A-MPDU")]
    pub ampdu_frames: i64,
    /// Null data packets.
    #[field(label = "NDP")]
    pub ndp_frames: i64,
    /// Preambles detected.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// SIG fields read.
    #[field(label = "SIG fields read")]
    pub sig_ok: i64,
    /// SIG fields not readable.
    #[field(label = "SIG fields not readable")]
    pub sig_bad: i64,
    /// Data fields cut short.
    #[field(label = "Data fields cut short")]
    pub data_bad: i64,
    /// Network frames (beacons and probe responses).
    #[field(label = "Network frames")]
    pub beacons_seen: i64,
    /// Frames in the last whole second.
    #[field(label = "Frames per second")]
    pub frames_per_second: i64,
    /// The tuned frequency. No value when it is not known, as for a recording.
    #[field(label = "Tuned", unit = "Hz")]
    pub tuned_hz: Option<Hertz>,
    /// The nearest 1 MHz channel of the plan, if one is within 1 MHz of the tuned frequency.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The widest channel that the rate holds.
    #[field(label = "Widest channel", unit = "Hz")]
    pub max_width_hz: Hertz,
    /// The running mean of the SNR. No value before the first good frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good frame. No value before it.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the input. No value before the first input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The host steps through the channels.
    #[field(label = "Scanning")]
    pub scanning: bool,
    /// The receive and privacy note.
    #[field(label = "Note")]
    pub receive_note: &'a str,
    /// The note on what the data decoder reads.
    #[field(label = "Data")]
    pub data_note: &'a str,
}

/// A row of the networks of `halow`. The row key is the BSSID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowNetworkRow<'a> {
    /// The SSID, "(hidden)", the compressed SSID or "-" when not known.
    #[field(label = "SSID")]
    pub ssid: &'a str,
    /// The BSSID.
    #[field(label = "BSSID")]
    pub bssid: &'a str,
    /// The vendor.
    #[field(label = "Vendor")]
    pub vendor: &'a str,
    /// The channel number.
    #[field(label = "Ch")]
    pub channel: i64,
    /// The frequency of the channel, if known.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// The width of the last PPDU.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The operating width, if a beacon gave it.
    #[field(label = "Operating width", unit = "Hz")]
    pub op_width_hz: Option<Hertz>,
    /// The MCS of the last PPDU.
    #[field(label = "MCS")]
    pub mcs: i64,
    /// The spatial streams of the last PPDU.
    #[field(label = "NSS")]
    pub nss: i64,
    /// The SNR of the last PPDU.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The security.
    #[field(label = "Security")]
    pub security: &'a str,
    /// The beacon interval.
    #[field(label = "Beacon", unit = "s")]
    pub interval: Seconds,
    /// The level of the last PPDU.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Beacons heard.
    #[field(label = "Beacons")]
    pub beacons: i64,
    /// The country.
    #[field(label = "Country")]
    pub country: &'a str,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A row of the frame log of `halow`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub signal_s: f64,
    /// The width of the PPDU.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The MCS.
    #[field(label = "MCS")]
    pub mcs: i64,
    /// The spatial streams.
    #[field(label = "NSS")]
    pub nss: i64,
    /// The name of the preamble, if the SIG field gave it.
    #[field(label = "Preamble")]
    pub preamble: Option<&'a str>,
    /// The SNR of the PPDU.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The kind of the frame.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The BSSID.
    #[field(label = "BSSID")]
    pub bssid: &'a str,
    /// The length.
    #[field(label = "Len")]
    pub length: i64,
    /// The details of the frame.
    #[field(label = "Details")]
    pub details: &'a str,
}

/// A row of the width view of `halow`. The row key is the width in MHz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowWidthRow {
    /// The width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// SIG fields read at the width.
    #[field(label = "SIG fields read")]
    pub sig_ok: i64,
    /// The lowest radio rate for the width.
    #[field(label = "Radio rate needed", unit = "Hz")]
    pub min_rate_hz: Hertz,
    /// The radio rate recommended for the width.
    #[field(label = "Recommended", unit = "Hz")]
    pub rate_hz: Hertz,
    /// Whether the stack reads the width.
    #[field(label = "State")]
    pub state: WidthState,
}

/// A row of the PPDUs of each MCS of `halow`. The row key is the MCS.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowMcsRow {
    /// The MCS.
    #[field(label = "MCS")]
    pub mcs: i64,
    /// PPDUs counted.
    #[field(label = "PPDUs")]
    pub ppdus: i64,
}

/// A row of the PPDUs of each preamble of `halow`. The row key is the key of the preamble.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HalowPreambleRow {
    /// The preamble.
    #[field(label = "Preamble")]
    pub preamble: Preamble,
    /// PPDUs counted.
    #[field(label = "PPDUs")]
    pub ppdus: i64,
}

/// The networks of `halow`.
pub static HALOW_NETWORKS: Table<HalowNetworkRow<'static>> =
    Table::new("networks", "Networks", crate::halow::table::NETWORK_CAP)
        .label(&HalowNetworkRow::SSID)
        .sorted(&HalowNetworkRow::SNR_DB, sdrkit_core::SortOrder::Descending)
        .role(sdrkit_core::TableRole::Nodes);
/// The frame log of `halow`, newest first.
pub static HALOW_FRAMES: Table<HalowFrameRow<'static>> =
    Table::new("frames", "Frames", crate::halow::table::LOG_CAP).label(&HalowFrameRow::BSSID);
/// The width view of `halow`.
pub static HALOW_WIDTHS: Table<HalowWidthRow> = Table::new("widths", "Widths", 5);
/// The PPDUs of each MCS of `halow` since the last clear.
pub static HALOW_BY_MCS: Table<HalowMcsRow> = Table::new("by_mcs", "By MCS", 11);
/// The PPDUs of each preamble of `halow` since the last clear.
pub static HALOW_BY_PREAMBLE: Table<HalowPreambleRow> = Table::new("by_preamble", "By preamble", 3);

// btclassic

/// The result fields of `btclassic`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BtclassicFields<'a> {
    /// Good access codes.
    #[field(label = "Access codes")]
    pub access_codes: i64,
    /// Bursts analysed without an access code.
    #[field(label = "Bursts without an access code")]
    pub bursts_without_code: i64,
    /// GIAC and LIAC ID packets.
    #[field(label = "Inquiry ID packets")]
    pub inquiry_ids: i64,
    /// FHS packets read.
    #[field(label = "Inquiry responses")]
    pub inquiry_responses: i64,
    /// Bursts analysed.
    #[field(label = "Bursts analysed")]
    pub bursts_analysed: i64,
    /// Wideband bursts skipped.
    #[field(label = "Wideband bursts")]
    pub bursts_wideband: i64,
    /// The channels inside the capture.
    #[field(label = "Channels covered")]
    pub channels_covered: i64,
    /// The channel numbers count from a capture centre taken as channel 39 (the radio centre is not known).
    #[field(label = "Channel numbers relative")]
    pub channels_relative: bool,
    /// The carrier offset of the last good packet. No value before it.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub cfo_hz: Option<f64>,
    /// The SNR of the last good packet. No value before it.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The input level. No value before the first input.
    #[field(label = "Input level", unit = "dBFS")]
    pub input_level_dbfs: Option<f64>,
    /// 1 when an access code was found in the last 5 s of signal.
    #[field(label = "State")]
    pub state: i64,
    /// Devices in the table.
    #[field(label = "Devices", reading = 1)]
    pub devices: i64,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the devices of `btclassic`. The row key is the LAP.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BtclassicDeviceRow<'a> {
    /// The LAP as hex text.
    #[field(label = "LAP")]
    pub lap: &'a str,
    /// What the device is.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The address from an FHS packet.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// The class of device as text.
    #[field(label = "Class of device")]
    pub class: Option<&'a str>,
    /// The major class name.
    #[field(label = "Major class")]
    pub major_class: Option<&'a str>,
    /// The class of device value.
    #[field(label = "Class of device value")]
    pub class_raw: Option<i64>,
    /// CLK27..CLK2 from the FHS packet.
    #[field(label = "Clock", unit = "1.25 ms")]
    pub clock: Option<i64>,
    /// The level of the last packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The last levels.
    #[field(label = "Level history", unit = "dBFS")]
    pub level_history: &'a str,
    /// The UTC of the first packet.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD, role = Time)]
    pub last_heard: Option<Time>,
    /// Packets heard.
    #[field(label = "Packets")]
    pub packets: i64,
    /// The channel of the last packet.
    #[field(label = "Last channel", unit = "MHz")]
    pub last_channel_mhz: f64,
    /// The channels heard, as hex: bit k is channel k.
    #[field(label = "Channels heard")]
    pub channels_heard: &'a str,
    /// The kind of device its Class of Device declares (unknown before an FHS packet).
    #[field(label = "Device kind")]
    pub device_kind: DeviceKind,
}

/// A row of the packet log of `btclassic`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BtclassicPacketRow<'a> {
    /// The UTC of the packet, for a live source.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The channel of the packet.
    #[field(label = "Channel", unit = "MHz")]
    pub channel_mhz: f64,
    /// The LAP as hex text.
    #[field(label = "LAP")]
    pub lap: &'a str,
    /// The level of the packet.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Sync-word bits that differed.
    #[field(label = "Sync word errors")]
    pub sync_errors: i64,
    /// What the packet is.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The fields of a LAP heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BtclassicDeviceEvent<'a> {
    /// The LAP as hex text.
    #[field(label = "LAP")]
    pub lap: &'a str,
    /// What the device is.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of an inquiry response.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BtclassicInquiryEvent<'a> {
    /// The address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The class of device as text.
    #[field(label = "Class of device")]
    pub class: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The devices of `btclassic`, most recently heard first.
pub static BTCLASSIC_DEVICES: Table<BtclassicDeviceRow<'static>> =
    Table::new("devices", "Devices", crate::btclassic::DEVICE_CAP)
        .label(&BtclassicDeviceRow::ADDRESS)
        .sorted(&BtclassicDeviceRow::LEVEL_DBFS, sdrkit_core::SortOrder::Descending)
        .role(sdrkit_core::TableRole::Nodes);
/// The packet log of `btclassic`, newest first.
pub static BTCLASSIC_PACKETS: Table<BtclassicPacketRow<'static>> =
    Table::new("packets", "Packets", crate::btclassic::PACKET_CAP);
/// A LAP heard for the first time.
pub static BTCLASSIC_DEVICE_NEW: EventKind<BtclassicDeviceEvent<'static>> =
    EventKind::new("btclassic.device_new", "New Bluetooth device");
/// An inquiry response.
pub static BTCLASSIC_INQUIRY: EventKind<BtclassicInquiryEvent<'static>> =
    EventKind::new("btclassic.inquiry_response", "Bluetooth inquiry response");
/// Forgets the devices, the packet log and the counters of `btclassic`.
pub const BTCLASSIC_CLEAR: Command =
    Command::new("clear_devices", "Clear").help(sdrkit_graph::decoder::CLEAR.help);

/// What version 4.2 added: the connections of `ble` and their ATT log.
const ADDED_4_2: &[&str] = &[
    "shape.BleFields.connections_followed",
    "shape.BleFields.connection_packets",
    "ble.table.connections",
    "ble.table.att",
    "shape.BleConnectionRow",
    "shape.BleConnectionRow.access_address",
    "shape.BleConnectionRow.central",
    "shape.BleConnectionRow.peripheral",
    "shape.BleConnectionRow.interval_ms",
    "shape.BleConnectionRow.latency",
    "shape.BleConnectionRow.timeout_ms",
    "shape.BleConnectionRow.phy",
    "shape.BleConnectionRow.csa",
    "shape.BleConnectionRow.channels_used",
    "shape.BleConnectionRow.packets",
    "shape.BleConnectionRow.encrypted",
    "shape.BleConnectionRow.procedures",
    "shape.BleConnectionRow.att",
    "shape.BleConnectionRow.ended",
    "shape.BleConnectionRow.first_heard",
    "shape.BleConnectionRow.last_heard",
    "shape.BleAttRow",
    "shape.BleAttRow.signal_s",
    "shape.BleAttRow.connection",
    "shape.BleAttRow.from_central",
    "shape.BleAttRow.opcode",
    "shape.BleAttRow.handle",
    "shape.BleAttRow.service",
    "shape.BleAttRow.value",
    "ble.kept.connections",
    "ble.kept.att",
];

/// What version 4.3 added: the length of an ATT value and the setting that keeps the values out of the log.
const ADDED_4_3: &[&str] = &["shape.BleAttRow.length", "ble.setting.att_values"];

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The tables of what was heard and the frame logs stay over a change of frequency, with the counts of their
        // rows. The counters of the receiver and the measurements of the signal start again.
        Described::new("wlan", 4, 3)
            .content(
                sdrkit_core::ContentKind::Networks,
                &[
                    "ble",
                    "bletracker",
                    "wifi",
                    "halow",
                    "btclassic",
                    "wfb",
                    "espnow",
                ],
            )
            .shared_source("tracker", &["bletracker"])
            .stack(
                StackShape::new("ble")
                    .fields::<BleFields>()
                    .decoder("adv")
                    .table(&BLE_DEVICES)
                    .table(&BLE_PACKETS)
                    .table(&BLE_PDU_TYPES)
                    .table(&BLE_DEVICE_LEVELS)
                    .table(&BLE_CONNECTIONS)
                    .table(&BLE_ATT)
                    .event(&BLE_DEVICE)
                    .settings(BleSetting::SETTINGS)
                    .command(CLEAR)
                    .command(&BLE_REMOVE)
                    .fields::<BleTxFields>()
                    .event(&BLE_TX)
                    .command(&BLE_SEND)
                    .command(&BLE_HALT)
                    .kept(&[
                        "devices",
                        "packets",
                        "device_levels",
                        "connections",
                        "att",
                        "BleFields.devices",
                    ]),
            )
            .added(4, 2, ADDED_4_2)
            .added(4, 3, ADDED_4_3)
            .stack(
                StackShape::new("bletracker")
                    .fields::<BletrackerFields>()
                    .decoder("adv")
                    .table(&BLETRACKER_TRACKERS)
                    .table(&BLETRACKER_ALERTS)
                    .table(&BLETRACKER_KINDS)
                    .event(&BLETRACKER_ALERT)
                    .settings(TrackerSetting::SETTINGS)
                    .command(CLEAR)
                    .command(&BLETRACKER_MINE)
                    .fields::<BleTxFields>()
                    .event(&BLE_TX)
                    .command(&BLETRACKER_SEND)
                    .command(&BLE_HALT)
                    .kept(&[
                        "trackers",
                        "alerts",
                        "tracker_kinds",
                        "BletrackerFields.following_now",
                        "BletrackerFields.trackers_now",
                        "BletrackerFields.trackers",
                        "BletrackerFields.now_s",
                    ]),
            )
            .stack(
                StackShape::new("wifi")
                    .fields::<WifiFields>()
                    .decoder("rx")
                    .table(&WIFI_NETWORKS)
                    .table(&WIFI_STATIONS)
                    .table(&WIFI_BEACONS)
                    .table(&WIFI_OFDM_RATES)
                    .table(&WIFI_BANDS)
                    .settings(WifiSetting::SETTINGS)
                    .command(CLEAR)
                    .command(&NETWORK_REMOVE)
                    .kept(&["networks", "beacons", "bands", "WifiFields.networks"]),
            )
            .stack(
                StackShape::new("halow")
                    .fields::<HalowFields>()
                    .decoder("rx")
                    .table(&HALOW_NETWORKS)
                    .table(&HALOW_FRAMES)
                    .table(&HALOW_WIDTHS)
                    .table(&HALOW_BY_MCS)
                    .table(&HALOW_BY_PREAMBLE)
                    .settings(HalowSetting::SETTINGS)
                    .command(CLEAR)
                    .command(&NETWORK_REMOVE)
                    .kept(&["networks", "frames", "HalowFields.networks"]),
            )
            .stack(
                StackShape::new("btclassic")
                    .fields::<BtclassicFields>()
                    .fields::<DecoderFields>()
                    .table(&BTCLASSIC_DEVICES)
                    .table(&BTCLASSIC_PACKETS)
                    .event(&BTCLASSIC_DEVICE_NEW)
                    .event(&BTCLASSIC_INQUIRY)
                    .settings(BtSetting::SETTINGS)
                    .command(&BTCLASSIC_CLEAR)
                    .kept(&["devices", "packets", "BtclassicFields.devices"]),
            )
            .stack(
                StackShape::new("wfb")
                    .fields::<WfbFields>()
                    .fields::<DecoderFields>()
                    .table(&WFB_LINKS)
                    .table(&WFB_FRAMES)
                    .settings(WfbSetting::SETTINGS)
                    .command(CLEAR)
                    .kept(&[
                        "links",
                        "frames",
                        "WfbFields.links_heard",
                        "WfbFields.frames_lost",
                    ]),
            )
            .stack(
                StackShape::new("espnow")
                    .fields::<EspnowFields>()
                    .decoder("rx")
                    .table(&ESPNOW_DEVICES)
                    .table(&ESPNOW_FRAMES)
                    .table(&ESPNOW_BY_RATE)
                    .table(&ESPNOW_BY_CHANNEL)
                    .event(&ESPNOW_FRAME)
                    .event(&ESPNOW_DEVICE)
                    .settings(EspnowSetting::SETTINGS)
                    .command(CLEAR)
                    .kept(&["devices", "frames", "EspnowFields.devices"]),
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
        .unwrap_or_else(|| panic!("the wlan family does not describe the stack {id}"))
}
