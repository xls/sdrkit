//! The interface of the wlan family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::ble::table::AddressType;
use crate::ble::BleSetting;
use crate::btclassic::BtSetting;
use crate::espnow::EspnowSetting;
use crate::halow::phy::Preamble;
use crate::halow::table::WidthState;
use crate::halow::HalowSetting;
use crate::tracker::book::{TrackerKind, TrackerState, Trend};
use crate::tracker::TrackerSetting;
use crate::wfb::WfbSetting;
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
    Table::new("links", "Links", crate::wfb::LINK_CAP);
/// The frame log of `wfb`, newest first.
pub static WFB_FRAMES: Table<WfbFrameRow<'static>> =
    Table::new("frames", "Frames", crate::wfb::FRAME_CAP);

// ble

/// The result fields of `ble`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BleFields<'a> {
    /// Devices in the table.
    #[field(label = "Devices")]
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
}

/// A row of the packet log of `ble`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BlePacketRow<'a> {
    /// The signal time of the packet.
    #[field(label = "Signal time", unit = "s")]
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
    #[field(label = "Length", unit = "bytes")]
    pub length: i64,
    /// The one-line summary.
    #[field(label = "Details")]
    pub details: &'a str,
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
    Table::new("devices", "Devices", crate::ble::table::DEVICE_CAP);
/// The packet log of `ble`, newest first.
pub static BLE_PACKETS: Table<BlePacketRow<'static>> =
    Table::new("packets", "Packets", crate::ble::table::LOG_CAP);
/// The good packets of each PDU type of `ble` since the last clear.
pub static BLE_PDU_TYPES: Table<BlePduTypeRow<'static>> = Table::new("pdu_types", "PDU types", 16);
/// The last levels of each device of `ble`, one row a level.
pub static BLE_DEVICE_LEVELS: Table<BleDeviceLevelRow<'static>> = Table::new(
    "device_levels",
    "Device levels",
    crate::ble::table::DEVICE_CAP * crate::ble::table::HISTORY,
);
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
    #[arg(shared = crate::ble::tx::ADDRESS)]
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
    #[arg(shared = crate::ble::tx::COUNT)]
    pub count: Option<i64>,
    /// The interval.
    #[arg(shared = crate::ble::tx::INTERVAL_MS)]
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
    #[field(label = "Trackers now")]
    pub trackers_now: i64,
    /// Trackers in the table.
    #[field(label = "Trackers")]
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
    Table::new("trackers", "Trackers", crate::tracker::book::RECORD_CAP);
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
    #[arg(shared = crate::ble::tx::ADDRESS)]
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
    #[arg(shared = crate::ble::tx::COUNT)]
    pub count: Option<i64>,
    /// The interval.
    #[arg(shared = crate::ble::tx::INTERVAL_MS)]
    pub interval_ms: Option<i64>,
}

/// Sends tracker advertisements in the form that the detector reads.
pub const BLETRACKER_SEND: CommandKind<BletrackerSendArgs<'static>> = CommandKind::new(
    "send",
    "Send a tracker advertisement",
)
.help(
    "Sends count packets, interval ms apart, on the advertising channel of the tuned frequency: Find My manufacturer \
     data of company 0x004C (type 0x12, length 25, the status, the payload) or Google service data of UUID 0xFEAA (frame \
     type 0x40 or 0x41, the payload). A send during another waits for it. Transmit must be on.",
)
.transmits(sdrkit_core::TxContent::Beacon)
.stopped_by("halt");

// wifi

/// The result fields of `wifi`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiFields<'a> {
    /// Networks in the table.
    #[field(label = "Networks")]
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
    /// The primary channel.
    #[field(label = "Ch")]
    pub channel: i64,
    /// The channel the frames were heard on.
    #[field(label = "Heard on")]
    pub heard_on: i64,
    /// The channel width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The newest standard ("ax").
    #[field(label = "Std")]
    pub standard: &'a str,
    /// The security.
    #[field(label = "Security")]
    pub security: &'a str,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
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
}

/// A row of the frame log of `wifi`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WifiBeaconRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Signal time", unit = "s")]
    pub signal_s: f64,
    /// The channel the frame was heard on.
    #[field(label = "Ch")]
    pub channel: i64,
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
    Table::new("networks", "Networks", crate::wifi::table::NETWORK_CAP);
/// The frame log of `wifi`, newest first.
pub static WIFI_BEACONS: Table<WifiBeaconRow<'static>> =
    Table::new("beacons", "Beacons", crate::wifi::table::LOG_CAP);
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
    #[field(label = "Devices")]
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
    /// The channel of the last frame (0: not known).
    #[field(label = "Ch")]
    pub channel: i64,
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
    #[field(shared = columns::LAST_HEARD)]
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
    /// The channel (0: not known).
    #[field(label = "Ch")]
    pub channel: i64,
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
    Table::new("devices", "Devices", crate::espnow::table::DEVICE_CAP);
/// The frame log of `espnow`, newest first.
pub static ESPNOW_FRAMES: Table<EspnowFrameRow<'static>> =
    Table::new("frames", "Frames", crate::espnow::table::LOG_CAP);
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
    #[field(label = "Networks")]
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
    Table::new("networks", "Networks", crate::halow::table::NETWORK_CAP);
/// The frame log of `halow`, newest first.
pub static HALOW_FRAMES: Table<HalowFrameRow<'static>> =
    Table::new("frames", "Frames", crate::halow::table::LOG_CAP);
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
    #[field(label = "Devices")]
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
    #[field(shared = columns::LAST_HEARD)]
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
    Table::new("devices", "Devices", crate::btclassic::DEVICE_CAP);
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

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The tables of what was heard and the frame logs stay over a change of frequency, with the counts of their
        // rows. The counters of the receiver and the measurements of the signal start again.
        Described::new("wlan", 1, 2)
            .shared_source("tracker", &["bletracker"])
            .stack(
                StackShape::new("ble")
                    .fields::<BleFields>()
                    .decoder("adv")
                    .table(&BLE_DEVICES)
                    .table(&BLE_PACKETS)
                    .table(&BLE_PDU_TYPES)
                    .table(&BLE_DEVICE_LEVELS)
                    .event(&BLE_DEVICE)
                    .settings(BleSetting::SETTINGS)
                    .command(CLEAR)
                    .command(&BLE_REMOVE)
                    .fields::<BleTxFields>()
                    .event(&BLE_TX)
                    .command(&BLE_SEND)
                    .command(&BLE_HALT)
                    .kept(&["devices", "packets", "device_levels", "BleFields.devices"]),
            )
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
            .added_in(1, 1, |f| {
                f.stack(
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
            .added(1, 2, ADDED_1_2)
    })
}

/// What version 1.2 added: the advertising senders of the stacks `ble` and `bletracker`.
const ADDED_1_2: &[&str] = &[
    "ble.fields.BleTxFields",
    "ble.event.ble.tx",
    "ble.setting.tx_level",
    "ble.command.send",
    "ble.command.halt",
    "bletracker.fields.BleTxFields",
    "bletracker.event.ble.tx",
    "bletracker.setting.tx_level",
    "bletracker.command.send",
    "bletracker.command.halt",
    "shape.BleTxFields",
    "shape.BleTxFields.tx_sending",
    "shape.BleTxFields.tx_sent",
    "shape.BleTxFields.tx_left",
    "shape.BleTxEvent",
    "shape.BleTxEvent.state",
    "shape.BleTxEvent.reason",
    "shape.BleTxEvent.target",
    "shape.BleTxEvent.text",
    "shape.BleTxEvent.family",
];

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
