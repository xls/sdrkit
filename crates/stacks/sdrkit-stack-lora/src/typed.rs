//! The interface of the LoRa family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types and what each stack declares.

use crate::commands::{
    AdvertScope, WantAck, CHANNEL, CHANNEL_ROW, CONTACT_NAME, EMOJI, HOP_LIMIT, INDEX, KEY, MC_KEY,
    MC_TO, MT_TO, PRECISION, REPLY_ID, SCOPE, WANT_ACK, WHAT,
};
use crate::components::elrs::FrameKind;
use crate::components::fanet::{FrameKind as FanetFrameKind, StationKind};
use crate::mesh::mt_client::MtRequest;
use crate::mesh::types::{DeliveryState, Protocol, TxKind};
use crate::proto::reticulum::PacketType;
use crate::results::{CrcState, Modulation};
use sdrkit_core::command::{Command, CommandSet};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::settings::Argument;
use sdrkit_core::{SettingSet, TxContent};
use sdrkit_graph::GuardFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // Version 3.2 names rows and orders tables.
        Described::new("lora", 3, 2)
            .added(
                3,
                1,
                &[
                    "mesh.content",
                    "meshtastic.content",
                    "meshcore.content",
                    "lorawan.content",
                    "loraaprs.content",
                    "meshcom.content",
                    "lorawanwide.content",
                    "loraraw.content",
                    "aprs.content",
                    "fanet.content",
                    "reticulum.content",
                    "elrs.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Mesh,
                &[
                    "mesh",
                    "meshtastic",
                    "meshcore",
                    "loraaprs",
                    "meshcom",
                    "aprs",
                    "reticulum",
                ],
            )
            .content(sdrkit_core::ContentKind::Tracking, &["fanet"])
            .content(
                sdrkit_core::ContentKind::Devices,
                &["lorawan", "lorawanwide", "loraraw", "elrs"],
            )
            .audio(sdrkit_core::AudioKind::Monitor, &["aprs"])
            .text(&[
                "mesh",
                "meshtastic",
                "meshcore",
                "loraaprs",
                "meshcom",
                "aprs",
                "reticulum",
            ])
            .shared_source("results", ALL_STACKS)
            .shared_source("commands", ALL_STACKS)
            .shared_source(
                "modulate",
                &[
                    "mesh",
                    "meshtastic",
                    "meshcore",
                    "lorawan",
                    "loraaprs",
                    "meshcom",
                    "loraraw",
                    "aprs",
                ],
            )
            .shared_source("raw", &["loraraw"])
            .shared_source("aprs", &["aprs", "loraaprs", "meshcom"])
            .shared_source("mesh", MESH_STACKS)
            .shared_source("lorawan", &["lorawan", "lorawanwide"])
            .stack(mesh_stack("mesh"))
            .stack(mesh_stack("meshtastic"))
            .stack(mesh_stack("meshcore"))
            .stack(mesh_stack("lorawan"))
            .stack(mesh_stack("loraaprs"))
            .stack(mesh_stack("meshcom"))
            .stack(lorawanwide())
            .stack(loraraw())
            .stack(aprs())
            .stack(fanet())
            .stack(reticulum())
            .stack(elrs())
    })
}

/// The stacks whose code shares the files of results and commands.
const ALL_STACKS: &[&str] = &[
    "mesh",
    "meshtastic",
    "meshcore",
    "lorawan",
    "loraaprs",
    "meshcom",
    "lorawanwide",
    "loraraw",
    "aprs",
];

/// The stacks of the mesh family and of the code it shares with `lorawanwide`.
const MESH_STACKS: &[&str] = &[
    "mesh",
    "meshtastic",
    "meshcore",
    "lorawan",
    "loraaprs",
    "meshcom",
    "lorawanwide",
];

/// The `reset` command of the LoRa bank, which the stacks with a bank name as their own.
fn bank_reset() -> &'static Command {
    use sdrkit_proto_lora_phy::components::LoraBankCommand;
    LoraBankCommand::COMMANDS
        .iter()
        .copied()
        .find(|c| c.key == "reset")
        .expect("the LoRa bank has a reset command")
}

// The events of several stacks.

/// The fields of one LoRa or FSK frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LoraPacketEvent<'a> {
    /// The protocol of the frame.
    #[field(label = "Net")]
    pub net: Protocol,
    /// How the frame was modulated.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The spreading factor, none for FSK.
    #[field(label = "Spreading factor")]
    pub sf: Option<i64>,
    /// The payload.
    #[field(label = "Payload")]
    pub payload: Arc<[u8]>,
    /// The CRC is right, or the frame has none.
    #[field(label = "CRC passed")]
    pub crc_ok: bool,
    /// The sender, if the protocol names one.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The addressee, if the protocol names one.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// LoRaWAN: the device address, eight hex digits.
    #[field(label = "DevAddr")]
    pub devaddr: Option<&'a str>,
    /// LoRaWAN: the IQ is inverted (a downlink).
    #[field(label = "IQ inverted")]
    pub iq_inverted: Option<bool>,
    /// The frame has a CRC.
    #[field(label = "CRC present")]
    pub crc: Option<bool>,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Option<Hertz>,
    /// The coding rate denominator.
    #[field(label = "Coding rate")]
    pub cr: Option<i64>,
    /// The header is implicit.
    #[field(label = "Implicit header")]
    pub implicit: Option<bool>,
    /// The sync word.
    #[field(label = "Sync word")]
    pub sync: Option<i64>,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a frame that the stack requests the host to send.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxRequestEvent<'a> {
    /// A number for the reply.
    #[field(label = "Id")]
    pub id: i64,
    /// What the frame is.
    #[field(label = "What")]
    pub what: TxKind,
    /// The payload.
    #[field(label = "Payload")]
    pub payload: Arc<[u8]>,
    /// The frequency to send on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The earliest send time, signal seconds.
    #[field(label = "Not before", unit = "s")]
    pub not_before_s: f64,
    /// The time on air of a LoRa frame.
    #[field(label = "Time on air", unit = "s")]
    pub air_time: Option<Seconds>,
    /// The own message that the frame carries.
    #[field(label = "Message")]
    pub message_key: Option<i64>,
    /// The LoRa setting in words.
    #[field(label = "LoRa setting")]
    pub setting: Option<&'a str>,
    /// The packet speed of an AX.25 frame.
    #[field(label = "Packet speed", unit = "bit/s")]
    pub afsk_baud: Option<i64>,
}

/// One LoRa or FSK frame.
pub static LORA_PACKET: EventKind<LoraPacketEvent<'static>> =
    EventKind::new("lora.packet", "LoRa packet");
/// A frame to send.
pub static TX_REQUEST: EventKind<TxRequestEvent<'static>> =
    EventKind::new("tx.request", "Frame to send");

/// The counts of the LoRa modulator.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModulateFields {
    /// Bursts written to the output.
    #[field(label = "Bursts written")]
    pub tx_bursts: i64,
    /// Samples written to the output.
    #[field(label = "Samples written")]
    pub tx_samples: i64,
    /// Frames that could not be modulated: a bad LoRa setting, a payload that does not fit, or a carrier outside the
    /// band.
    #[field(label = "Frames dropped")]
    pub tx_dropped: i64,
}

// LoRa (raw).

/// The result fields of the raw LoRa stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RawFields<'a> {
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset of the last frame, while frames come.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Frames with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frame preambles seen.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Headers that failed their checksum.
    #[field(label = "Headers failed")]
    pub header_failed: i64,
    /// What is searched.
    #[field(label = "Searching")]
    pub searching: &'a str,
    /// The dial.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// Frames sent.
    #[field(label = "Sent")]
    pub tx_sent: i64,
    /// Frames at SF5.
    #[field(label = "SF5 frames")]
    pub sf5: i64,
    /// Frames at SF6.
    #[field(label = "SF6 frames")]
    pub sf6: i64,
    /// Frames at SF7.
    #[field(label = "SF7 frames")]
    pub sf7: i64,
    /// Frames at SF8.
    #[field(label = "SF8 frames")]
    pub sf8: i64,
    /// Frames at SF9.
    #[field(label = "SF9 frames")]
    pub sf9: i64,
    /// Frames at SF10.
    #[field(label = "SF10 frames")]
    pub sf10: i64,
    /// Frames at SF11.
    #[field(label = "SF11 frames")]
    pub sf11: i64,
    /// Frames at SF12.
    #[field(label = "SF12 frames")]
    pub sf12: i64,
    /// Frames at 125 kHz.
    #[field(label = "125 kHz frames")]
    pub bw125: i64,
    /// Frames at 250 kHz.
    #[field(label = "250 kHz frames")]
    pub bw250: i64,
    /// Frames at 500 kHz.
    #[field(label = "500 kHz frames")]
    pub bw500: i64,
    /// The signal to noise ratio of the last frame, while frames come.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
}

/// A row of the raw frame log. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RawFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The spreading factor.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Hertz,
    /// The coding rate denominator.
    #[field(label = "CR")]
    pub cr: i64,
    /// The payload length.
    #[field(label = "Length", unit = "B")]
    pub len: i64,
    /// What the CRC says.
    #[field(label = "CRC")]
    pub crc: CrcState,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level of the frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier offset.
    #[field(label = "Offset", unit = "Hz")]
    pub cfo: Hertz,
    /// The payload.
    #[field(label = "Payload")]
    pub payload: Arc<[u8]>,
    /// The payload as text for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A row of the raw decoders. The row key is the tag of the detector.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RawDecoderRow<'a> {
    /// The spreading factor.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Hertz,
    /// The sync words searched, in words.
    #[field(label = "Sync")]
    pub syncs: &'a str,
    /// The channel lies inside the captured band.
    #[field(label = "In band")]
    pub in_band: bool,
    /// Preambles.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Frames decoded.
    #[field(label = "Frames")]
    pub frames: i64,
    /// CRC errors.
    #[field(label = "CRC errors")]
    pub crc_errors: i64,
    /// Headers failed.
    #[field(label = "Header failed")]
    pub header_failed: i64,
    /// Sync words failed.
    #[field(label = "Sync failed")]
    pub sync_failed: i64,
}

/// The raw frame log, newest last.
pub static RAW_FRAMES: Table<RawFrameRow<'static>> =
    Table::new("frames", "Frames", crate::components::raw::LOG_CAP);
/// The raw decoders.
pub static RAW_DECODERS: Table<RawDecoderRow<'static>> = Table::new("decoders", "Decoders", 100);

/// The arguments of `send` of the raw stack.
#[derive(Arguments)]
pub struct RawSendArgs {
    /// The payload of the frame.
    #[arg(label = "Payload", help = "1 to 255 bytes.")]
    pub payload: Arc<[u8]>,
}

/// Sends a LoRa frame.
pub const RAW_SEND: CommandKind<RawSendArgs> = CommandKind::new("send", "Send a frame")
    .help("Sends the bytes with the LoRa setting of the stack. Transmit must be on.")
    .transmits(TxContent::Frame);

/// The raw LoRa stack.
fn loraraw() -> StackShape {
    StackShape::new("loraraw")
        .fields::<RawFields>()
        .receiver_fields::<GuardFields>()
        .component("modulate", |c| c.fields::<ModulateFields>())
        .table(&RAW_FRAMES)
        .table(&RAW_DECODERS)
        .event(&LORA_PACKET)
        .event(&TX_REQUEST)
        .settings(crate::settings::RawSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(bank_reset())
        .command(&RAW_SEND)
        .kept(&[
            "frames",
            "RawFields.frames_ok",
            "RawFields.frames_bad",
            "RawFields.preambles",
            "RawFields.header_failed",
            "RawFields.tx_sent",
            "RawFields.sf5",
            "RawFields.sf6",
            "RawFields.sf7",
            "RawFields.sf8",
            "RawFields.sf9",
            "RawFields.sf10",
            "RawFields.sf11",
            "RawFields.sf12",
            "RawFields.bw125",
            "RawFields.bw250",
            "RawFields.bw500",
        ])
}

/// The shape of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack `id`.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the lora family does not describe the stack {id}"))
}

/// An empty store of the results of the stack `id`, for its status component.
pub fn results(id: &str) -> sdrkit_core::publish::Published {
    sdrkit_core::publish::Published::from_decl(std::sync::Arc::new(shape(id).results_decl()))
}

// ExpressLRS.

/// The result fields of the ExpressLRS stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ElrsFields<'a> {
    /// Frames whose CRC fits a link.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose CRC fits no link.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Preambles whose sync word followed.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// The binding phrase of the user's own link is set.
    #[field(label = "Binding phrase set")]
    pub phrase_set: bool,
    /// The hop channels inside the capture.
    #[field(label = "Hop channels inside the capture")]
    pub hop_channels_inside: i64,
    /// The hop channels of the plan.
    #[field(label = "Hop channels")]
    pub hop_channels: i64,
    /// The signal to noise ratio of the last frame, while frames come.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The links in the table.
    #[field(label = "Links")]
    pub links_heard: i64,
    /// What the decoder cannot confirm, and what it needs.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the frame log. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ElrsFrameRow<'a> {
    /// The UTC of the frame, when known.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The link of the frame.
    #[field(label = "Link", refers_to = "links")]
    pub link: &'a str,
    /// What the frame is.
    #[field(label = "Type")]
    pub kind: FrameKind,
    /// The frame in one line.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The hop channel plus the carrier offset.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The index of the hop channel.
    #[field(label = "Hop channel")]
    pub hop_channel: i64,
    /// The spreading factor.
    #[field(label = "SF")]
    pub sf: i64,
    /// The length of the frame.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The IQ was inverted.
    #[field(label = "Inverted IQ")]
    pub inverted: bool,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The frame bytes as hex.
    #[field(label = "Hex")]
    pub hex: &'a str,
}

/// A row of the links. The row key and `source` are the link, 4 hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ElrsLinkRow<'a> {
    /// The link.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What the last frame of the link was.
    #[field(label = "Type")]
    pub kind: FrameKind,
    /// The last frame of the link in one line.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The frames of the link.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The sync frames of the link.
    #[field(label = "Sync frames")]
    pub sync_frames: i64,
    /// The rate of the last frame.
    #[field(label = "Rate")]
    pub rate: &'a str,
    /// The hop index of the last sync frame, once one came.
    #[field(label = "Hop index")]
    pub hop_index: Option<i64>,
    /// The hop channels on which the link was heard, in rising order.
    #[field(label = "Heard on hop channels")]
    pub heard_on: &'a str,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The fields of a link heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ElrsLinkEvent<'a> {
    /// The link, 4 hex digits.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The frame log, newest last.
pub static ELRS_FRAMES: Table<ElrsFrameRow<'static>> =
    Table::new("frames", "Frames", crate::components::elrs::FRAME_CAP).label(&ElrsFrameRow::LINK);
/// The links.
pub static ELRS_LINKS: Table<ElrsLinkRow<'static>> =
    Table::new("links", "Links", crate::components::elrs::LINK_CAP).label(&ElrsLinkRow::SOURCE);
/// A link heard for the first time.
pub static ELRS_LINK: EventKind<ElrsLinkEvent<'static>> =
    EventKind::new("elrs.link", "ExpressLRS link");

/// The ExpressLRS stack.
fn elrs() -> StackShape {
    StackShape::new("elrs")
        .fields::<ElrsFields>()
        .receiver_fields::<GuardFields>()
        .table(&ELRS_FRAMES)
        .table(&ELRS_LINKS)
        .event(&ELRS_LINK)
        .settings(crate::components::elrs::ElrsSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&[
            "frames",
            "links",
            "ElrsFields.frames_ok",
            "ElrsFields.frames_bad",
            "ElrsFields.preambles",
            "ElrsFields.links_heard",
        ])
}

// FANET.

/// The result fields of the FANET stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetFields<'a> {
    /// Frames with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The signal to noise ratio of the last frame, while frames come.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last frame, while frames come.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The channel of the plan.
    #[field(label = "Channel")]
    pub listening: &'a str,
    /// The stations in the table.
    #[field(label = "Stations")]
    pub stations_heard: i64,
}

/// A row of the FANET frame log. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What the frame is.
    #[field(label = "Kind")]
    pub kind: FanetFrameKind,
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The addressee of a unicast frame, else empty.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The frame in one line.
    #[field(label = "Decoded")]
    pub decoded: &'a str,
    /// The position that the frame gives, if it gives one.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The level of the frame.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

/// A row of the FANET stations. The row key and `source` are the station id "MM:DDDD".
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetStationRow<'a> {
    /// The station id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The name that the station sent.
    #[field(label = "Name")]
    pub name: &'a str,
    /// What the station is.
    #[field(label = "Kind")]
    pub kind: StationKind,
    /// The aircraft type, from a tracking frame.
    #[field(label = "Aircraft type")]
    pub aircraft: Option<&'a str>,
    /// The last position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude, from a tracking frame.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
    /// The speed, from a tracking frame.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The climb rate, from a tracking frame.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The heading, from a tracking frame.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The last service frame in one line.
    #[field(label = "Weather")]
    pub weather: &'a str,
    /// The frames of the station.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the first frame.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A track point of a FANET station. The row key is the station id and the signal time of the point.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetTrackRow<'a> {
    /// The station that the point belongs to.
    #[field(label = "Station", refers_to = "stations")]
    pub station: &'a str,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The signal time of the point.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
}

/// The fields of a station heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetStationEvent<'a> {
    /// The station id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetMessageEvent<'a> {
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The addressee of a unicast message, else empty.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a ground tracker that requests help.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FanetDistressEvent<'a> {
    /// The station id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The state of the tracker in words.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position of the tracker, if the frame gives one.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// How urgent the request is.
    #[field(shared = keys::URGENCY)]
    pub urgency: keys::Urgency,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The FANET frame log, newest last.
pub static FANET_FRAMES: Table<FanetFrameRow<'static>> =
    Table::new("frames", "Frames", crate::components::fanet::FRAME_CAP)
        .label(&FanetFrameRow::SOURCE);
/// The FANET stations.
pub static FANET_STATIONS: Table<FanetStationRow<'static>> = Table::new(
    "stations",
    "Stations",
    crate::components::fanet::STATION_CAP,
)
.label(&FanetStationRow::NAME);
/// The track points of the FANET stations.
pub static FANET_TRACKS: Table<FanetTrackRow<'static>> =
    Table::new("station_tracks", "Tracks", 4000).label(&FanetTrackRow::STATION);
/// A FANET station heard for the first time.
pub static FANET_STATION: EventKind<FanetStationEvent<'static>> =
    EventKind::new("fanet.station", "FANET station");
/// A FANET message.
pub static FANET_MESSAGE: EventKind<FanetMessageEvent<'static>> =
    EventKind::new("fanet.message", "FANET message");
/// A FANET ground tracker that requests help.
pub static FANET_DISTRESS: EventKind<FanetDistressEvent<'static>> =
    EventKind::new("fanet.distress", "FANET distress");

/// The FANET stack.
fn fanet() -> StackShape {
    StackShape::new("fanet")
        .fields::<FanetFields>()
        .receiver_fields::<GuardFields>()
        .table(&FANET_FRAMES)
        .table(&FANET_STATIONS)
        .table(&FANET_TRACKS)
        .points("Stations")
        .event(&FANET_STATION)
        .event(&FANET_MESSAGE)
        .event(&FANET_DISTRESS)
        .settings(crate::components::fanet::FanetSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&[
            "frames",
            "stations",
            "station_tracks",
            "FanetFields.frames_ok",
            "FanetFields.frames_bad",
            "FanetFields.stations_heard",
        ])
}

// Reticulum.

/// The result fields of the Reticulum stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RnsFields<'a> {
    /// Frames that gave a packet.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that gave no packet.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Preambles whose sync word followed.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// The signal to noise ratio of the last frame, while frames come.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset of the last frame, while frames come.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The LoRa settings that the plan searches.
    #[field(label = "Searching")]
    pub searching: &'a str,
    /// The destinations in the table.
    #[field(label = "Destinations")]
    pub destinations_heard: i64,
}

/// A row of the Reticulum packet log. The row key is the place of the packet in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RnsPacketRow<'a> {
    /// The UTC of the packet, when known.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of the packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The destination hash as hex.
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The type of the packet.
    #[field(label = "Type")]
    pub r#type: PacketType,
    /// The packet in one line.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The channel plus the carrier offset.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
    /// The first bytes of the packet.
    #[field(label = "First bytes")]
    pub hex: Arc<[u8]>,
    /// The LoRa setting of the frame.
    #[field(label = "LoRa setting")]
    pub setting: &'a str,
}

/// A row of the Reticulum destinations. The row key is the destination hash as hex.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RnsDestinationRow<'a> {
    /// The destination hash as hex.
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The type of the last packet.
    #[field(label = "Type")]
    pub r#type: PacketType,
    /// The last packet in one line.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The packets to the destination.
    #[field(label = "Packets")]
    pub packets: Option<i64>,
    /// The identity that an announce gave, as hex.
    #[field(label = "Identity")]
    pub identity: &'a str,
    /// The text that an announce gave.
    #[field(label = "Announced text")]
    pub app_text: &'a str,
    /// The UTC of the first packet.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The fields of a destination heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RnsDestinationEvent<'a> {
    /// The destination hash as hex.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The line of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of an announce.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RnsAnnounceEvent<'a> {
    /// The destination hash as hex.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The identity as hex.
    #[field(label = "Identity")]
    pub identity: &'a str,
    /// The announced text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The Reticulum packet log, newest last.
pub static RNS_PACKETS: Table<RnsPacketRow<'static>> = Table::new(
    "packets",
    "Packets",
    crate::components::reticulum::PACKET_CAP,
)
.label(&RnsPacketRow::DESTINATION);
/// The Reticulum destinations.
pub static RNS_DESTINATIONS: Table<RnsDestinationRow<'static>> = Table::new(
    "destinations",
    "Destinations",
    crate::components::reticulum::DESTINATION_CAP,
)
.label(&RnsDestinationRow::DESTINATION);
/// A Reticulum destination heard for the first time.
pub static RNS_DESTINATION: EventKind<RnsDestinationEvent<'static>> =
    EventKind::new("reticulum.destination", "Reticulum destination");
/// A Reticulum announce.
pub static RNS_ANNOUNCE: EventKind<RnsAnnounceEvent<'static>> =
    EventKind::new("reticulum.announce", "Reticulum announce");

/// The Reticulum stack.
fn reticulum() -> StackShape {
    StackShape::new("reticulum")
        .fields::<RnsFields>()
        .receiver_fields::<GuardFields>()
        .table(&RNS_PACKETS)
        .table(&RNS_DESTINATIONS)
        .event(&RNS_DESTINATION)
        .event(&RNS_ANNOUNCE)
        .settings(crate::components::reticulum::RnsSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .kept(&[
            "packets",
            "destinations",
            "RnsFields.frames_ok",
            "RnsFields.frames_bad",
            "RnsFields.preambles",
            "RnsFields.destinations_heard",
        ])
}

// APRS (AFSK).

/// The result fields of the APRS stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AprsFields {
    /// Frames with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frames at 1200 bit/s.
    #[field(label = "1200 bit/s frames")]
    pub frames_1200: i64,
    /// Frames at 9600 bit/s.
    #[field(label = "9600 bit/s frames")]
    pub frames_9600: i64,
    /// APRS frames, without the digipeated copies.
    #[field(label = "APRS frames")]
    pub aprs_frames: i64,
    /// Digipeated copies.
    #[field(label = "Digipeated copies")]
    pub copies: i64,
    /// Frames that are not AX.25.
    #[field(label = "Not AX.25")]
    pub not_ax25: i64,
    /// Stations, objects and items in the table.
    #[field(label = "Stations")]
    pub stations: i64,
    /// A carrier is present.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// The signal to noise ratio, once the demodulator reports.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, once the demodulator reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// A row of the APRS frame log. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AprsFrameRow<'a> {
    /// The signal time of the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The destination.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The digipeater path.
    #[field(label = "Path")]
    pub path: &'a str,
    /// The APRS kind or the AX.25 frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The frame in one line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The bit rate.
    #[field(label = "Speed", unit = "bit/s")]
    pub bit_rate: Hertz,
}

/// A row of the APRS stations. The row key is the callsign with its SSID, or the name of an object or item.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AprsStationRow<'a> {
    /// The callsign with its SSID, or the name of an object or item.
    #[field(label = "Call")]
    pub call: &'a str,
    /// The map symbol.
    #[field(label = "Symbol")]
    pub symbol: &'a str,
    /// The last position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frames of the station.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The course.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The speed.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
    /// The comment or status.
    #[field(label = "Comment")]
    pub comment: &'a str,
    /// The station that sent an object, an item or a third-party packet.
    #[field(label = "Via")]
    pub via: &'a str,
}

/// The fields of one AX.25 frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AprsPacketEvent<'a> {
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The destination.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The digipeater path.
    #[field(label = "Path")]
    pub path: &'a str,
    /// The information field as received.
    #[field(label = "Information")]
    pub info: &'a str,
    /// The APRS kind or the AX.25 frame type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The frame in one line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position that the frame gives, if it gives one.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The frame in TNC2 text.
    #[field(label = "Packet (TNC2)")]
    pub tnc2: &'a str,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The APRS frame log, newest last.
pub static APRS_FRAMES: Table<AprsFrameRow<'static>> =
    Table::new("frames", "Frames", crate::aprs::station::FRAMES_CAP).label(&AprsFrameRow::SOURCE);
/// The APRS stations, objects and items.
pub static APRS_STATIONS: Table<AprsStationRow<'static>> =
    Table::new("stations", "Stations", crate::aprs::station::STATIONS_CAP)
        .label(&AprsStationRow::CALL);
/// One AX.25 frame.
pub static APRS_PACKET: EventKind<AprsPacketEvent<'static>> =
    EventKind::new("aprs.packet", "APRS packet");

/// The row key of a row of the APRS stations.
pub const APRS_STATION_ROW: Argument<field::Text> = objcmd::row_of("stations");

/// The arguments of `remove` of the APRS stack.
#[derive(Arguments)]
pub struct AprsRemoveArgs<'a> {
    /// The row key of the station.
    #[arg(shared = APRS_STATION_ROW)]
    pub row: &'a str,
}

/// Forgets one station.
pub const APRS_REMOVE: CommandKind<AprsRemoveArgs<'static>> = objcmd::remove_kind();

/// The arguments of `aprs.send`.
#[derive(Arguments)]
pub struct AprsSendArgs<'a> {
    /// The addressee.
    #[arg(label = "To", help = "A callsign with its SSID.")]
    pub to: &'a str,
    /// The text of the message.
    #[arg(label = "Text")]
    pub text: &'a str,
}

/// The reply of `aprs.send`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AprsSendReply {
    /// The message number: the column `packet_id` of its row in the messages. The column `state` of the row says whether
    /// the addressee acknowledged it.
    #[field(label = "Message number")]
    pub packet_id: i64,
}

/// Sends an APRS message.
pub const APRS_SEND: CommandKind<AprsSendArgs<'static>, AprsSendReply> =
    CommandKind::new("aprs.send", "Send an APRS message")
        .help("Sends one numbered message under the callsign of the station. It goes again after 30, 60, 120 and 240 s until the addressee answers, within the duty limit; after the fifth transmission without an answer it fails. The column `state` of its row turns to delivered when the addressee acknowledges it. A numbered message to the station is acknowledged once per number in 30 s.")
        .transmits(TxContent::Text);

/// The APRS stack.
fn aprs() -> StackShape {
    StackShape::new("aprs")
        .fields::<AprsFields>()
        .fields::<HamFields>()
        .receiver_fields::<GuardFields>()
        .component("modulate", |c| c.fields::<ModulateFields>())
        .table(&APRS_FRAMES)
        .table(&APRS_STATIONS)
        .table(&MESH_MESSAGES)
        .points("Stations")
        .event(&APRS_PACKET)
        .event(&TX_REQUEST)
        .settings(crate::settings::AprsSetting::SETTINGS)
        .command(&APRS_SEND)
        .command(&crate::commands::APRS_BEACON)
        .command(&objcmd::CLEAR)
        .command(&APRS_REMOVE)
        .kept(&[
            "frames",
            "stations",
            "messages",
            "AprsFields.frames_ok",
            "AprsFields.frames_bad",
            "AprsFields.frames_1200",
            "AprsFields.frames_9600",
            "AprsFields.aprs_frames",
            "AprsFields.copies",
            "AprsFields.not_ax25",
            "AprsFields.stations",
        ])
}

// The mesh receiver: mesh, meshtastic, meshcore, lorawan, loraaprs and meshcom.

/// The result fields of the mesh decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshFields<'a> {
    /// Frames with a good CRC since the last reset.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames whose CRC failed since the last reset.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frame preambles seen.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Headers that failed their checksum.
    #[field(label = "Headers failed")]
    pub header_failed: i64,
    /// Meshtastic nodes.
    #[field(label = "Meshtastic nodes")]
    pub mt_nodes: i64,
    /// MeshCore nodes.
    #[field(label = "MeshCore nodes")]
    pub mc_nodes: i64,
    /// Messages in the table.
    #[field(label = "Messages")]
    pub messages: i64,
    /// APRS stations.
    #[field(label = "APRS stations")]
    pub stations: i64,
    /// LoRaWAN devices.
    #[field(label = "LoRaWAN devices")]
    pub devices: i64,
    /// The band plan.
    #[field(label = "Band plan")]
    pub plan: &'a str,
    /// What is searched.
    #[field(label = "Searching")]
    pub searching: &'a str,
    /// The dial.
    #[field(label = "Dial", unit = "Hz")]
    pub dial: Hertz,
    /// The protocols searched, in words.
    #[field(label = "Protocols")]
    pub protocols: &'a str,
    /// The custom LoRa setting is searched.
    #[field(label = "Custom setting")]
    pub custom: bool,
    /// The signal to noise ratio of the last good frame, while frames come.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good frame, while frames come.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// The own Meshtastic node, once the Meshtastic client reports.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshMtFields<'a> {
    /// The own node id.
    #[field(label = "Meshtastic node")]
    pub mt_node_id: Option<&'a str>,
    /// The own public key, base64.
    #[field(label = "Meshtastic public key")]
    pub mt_public_key: Option<&'a str>,
    /// The own long name.
    #[field(label = "Long name")]
    pub mt_long_name: Option<&'a str>,
    /// The own short name.
    #[field(label = "Short name")]
    pub mt_short_name: Option<&'a str>,
    /// Own packets waiting for an acknowledgement.
    #[field(label = "Waiting for an ack")]
    pub mt_in_flight: Option<i64>,
    /// Packets of other nodes sent again.
    #[field(label = "Rebroadcasts")]
    pub mt_rebroadcasts: Option<i64>,
    /// Warnings about the settings of the own node.
    #[field(label = "Warnings")]
    pub mt_warnings: Option<&'a str>,
    /// The channel frequency that the client sends on, while the session listens to a channel of the network.
    #[field(label = "Meshtastic frequency", unit = "Hz")]
    pub mt_freq: Option<Hertz>,
}

/// The own MeshCore node, once the MeshCore client reports.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshMcFields<'a> {
    /// The own public key, hex.
    #[field(label = "MeshCore public key")]
    pub mc_public_key: Option<&'a str>,
    /// The own name.
    #[field(label = "MeshCore name")]
    pub mc_name: Option<&'a str>,
    /// The contacts.
    #[field(label = "Contacts")]
    pub mc_contacts: Option<i64>,
    /// The channel frequency that the client sends on, while the session listens to a channel of the network.
    #[field(label = "MeshCore frequency", unit = "Hz")]
    pub mc_freq: Option<Hertz>,
}

/// The own APRS station, once the amateur client reports.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HamFields<'a> {
    /// The source call of own APRS frames: the callsign of the station with the SSID. Empty without a callsign.
    #[field(label = "APRS call")]
    pub aprs_call: Option<&'a str>,
    /// The frequency that the client sends on, while the session listens to a channel of its link.
    #[field(label = "APRS frequency", unit = "Hz")]
    pub aprs_freq: Option<Hertz>,
}

/// The state of the transmit scheduler.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshTxFields {
    /// Frames waiting to be sent.
    #[field(label = "Waiting to send")]
    pub tx_pending: i64,
    /// Frames sent.
    #[field(label = "Sent")]
    pub tx_sent: i64,
}

/// A row of the packet list. The row key is the number of the packet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshPacketRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The packet type, as the protocol names it.
    #[field(label = "Type")]
    pub r#type: Option<&'a str>,
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The addressee.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The payload length.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The content, decoded or raw.
    #[field(label = "Content")]
    pub content: Option<&'a str>,
    /// The hops taken.
    #[field(label = "Hops")]
    pub hops: Option<i64>,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The spreading factor.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Hertz,
    /// More about the packet.
    #[field(label = "Detail")]
    pub summary: Option<&'a str>,
}

/// A row of the node list. The row key is `<net key>/<id>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshNodeRow<'a> {
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The node name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The short name.
    #[field(label = "Short")]
    pub short: Option<&'a str>,
    /// The node id.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The hardware model.
    #[field(label = "Hardware")]
    pub hw: Option<&'a str>,
    /// The role.
    #[field(label = "Role")]
    pub role: Option<&'a str>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The packets of the node.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The signal to noise ratio of the last packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The hops of the last packet.
    #[field(label = "Hops")]
    pub hops: Option<i64>,
    /// The battery charge; above 100 the node is powered from outside.
    #[field(label = "Battery", unit = "%")]
    pub battery_pct: Option<f64>,
    /// The battery voltage.
    #[field(label = "Voltage", unit = "V")]
    pub voltage_v: Option<f64>,
    /// The last position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A row of the messages. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshMessageRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The channel.
    #[field(shared = keys::CHANNEL)]
    pub channel: Option<&'a str>,
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The name of the sender.
    #[field(label = "Sender")]
    pub sender: Option<&'a str>,
    /// The addressee.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The hops taken.
    #[field(label = "Hops")]
    pub hops: Option<i64>,
    /// The delivery state of an own message.
    #[field(label = "State")]
    pub state: Option<DeliveryState>,
    /// Why the delivery failed.
    #[field(label = "Failure")]
    pub failure: Option<&'a str>,
    /// The distinct relays heard for an own broadcast.
    #[field(label = "Relays")]
    pub relays: Option<i64>,
    /// The packet id: the reply of `mt.send` for an own Meshtastic text, the message number of an APRS message (the reply of
    /// `aprs.send` for an own one). None for a protocol without packet ids.
    #[field(label = "Packet id")]
    pub packet_id: Option<i64>,
    /// The UTC when the text was received or sent, if the clock of the session is known.
    #[field(label = "Time (UTC)")]
    pub utc: Option<Time>,
    /// The signal to noise ratio of the frame of a received text.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the frame of a received text, relative to the full scale of the input (not an absolute power).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The text comes from a copy with a good CRC. False for a damaged copy that was shown after the wait without a good
    /// copy; a later good copy replaces the text and sets it. None for an own message.
    #[field(label = "CRC passed")]
    pub crc_ok: Option<bool>,
    /// The copies of the packet heard while the message is remembered (10 minutes), damaged ones included. None for an own
    /// message.
    #[field(label = "Copies")]
    pub copies: Option<i64>,
}

/// A row of the telemetry and position readings. The row key is the number of the reading.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshReadingRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The node of the reading.
    #[field(label = "Node")]
    pub node: &'a str,
    /// The battery charge.
    #[field(label = "Battery", unit = "%")]
    pub battery_pct: Option<f64>,
    /// The battery voltage.
    #[field(label = "Voltage", unit = "V")]
    pub voltage_v: Option<f64>,
    /// The channel use.
    #[field(label = "Channel use", unit = "%")]
    pub ch_util_pct: Option<f64>,
    /// The own air time.
    #[field(label = "Air time", unit = "%")]
    pub air_tx_pct: Option<f64>,
    /// The temperature.
    #[field(label = "Temperature", unit = "°C")]
    pub temp_c: Option<f64>,
    /// The relative humidity.
    #[field(label = "Humidity", unit = "%")]
    pub humidity_pct: Option<f64>,
    /// The barometric pressure.
    #[field(label = "Pressure", unit = "hPa")]
    pub pressure_hpa: Option<f64>,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
}

/// A row of the APRS stations of LoRa APRS and MeshCom. The row key is `<net key>/<call>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshStationRow<'a> {
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The call or the object name.
    #[field(label = "Call")]
    pub call: &'a str,
    /// The map symbol.
    #[field(label = "Symbol")]
    pub symbol: Option<&'a str>,
    /// The last position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The type of the last packet.
    #[field(label = "Type")]
    pub r#type: Option<&'a str>,
    /// The UTC of the last packet.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The packets of the station.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The signal to noise ratio of the last packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The relay path.
    #[field(label = "Path")]
    pub path: Option<&'a str>,
    /// The comment.
    #[field(label = "Comment")]
    pub comment: Option<&'a str>,
    /// The battery charge.
    #[field(label = "Battery", unit = "%")]
    pub battery_pct: Option<f64>,
}

/// A row of the LoRaWAN devices. The row key is the device id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshDeviceRow<'a> {
    /// The DevAddr or the DevEUI.
    #[field(label = "Id")]
    pub id: &'a str,
    /// The network operator.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// The maker, or the JoinEUI.
    #[field(label = "Vendor / JoinEUI")]
    pub vendor: Option<&'a str>,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frames of the device.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The level of the last frame.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The signal to noise ratio of the last frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The last frame counter.
    #[field(label = "FCnt")]
    pub fcnt: Option<i64>,
    /// The frames that the counter missed.
    #[field(label = "Missed")]
    pub missed: i64,
    /// The spreading factor of the last frame, none for FSK.
    #[field(label = "SF")]
    pub sf: Option<i64>,
    /// The frequency of the last frame.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// Uplinks.
    #[field(label = "Uplinks")]
    pub uplinks: i64,
    /// Downlinks.
    #[field(label = "Downlinks")]
    pub downlinks: i64,
    /// Join requests.
    #[field(label = "Joins")]
    pub joins: i64,
    /// The type of the last frame.
    #[field(label = "Type")]
    pub r#type: Option<&'a str>,
    /// The last FPort.
    #[field(label = "Port")]
    pub fport: Option<i64>,
    /// The last MAC commands or status.
    #[field(label = "MAC / status")]
    pub mac: Option<&'a str>,
}

/// A row of the packet counts by protocol and type. The row key is `<net key>/<type>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshTypeRow<'a> {
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The packet type.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The packets of the type.
    #[field(label = "Count")]
    pub count: i64,
}

/// A row of the decoders: one searched LoRa setting. The row key is the entry number of the plan.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshDecoderRow<'a> {
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The preset.
    #[field(label = "Preset")]
    pub preset: &'a str,
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// The channel offset from the dial.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// The spreading factor.
    #[field(label = "SF")]
    pub sf: i64,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Hertz,
    /// The coding rate denominator: 5 is 4/5, 8 is 4/8.
    #[field(label = "CR")]
    pub cr: i64,
    /// The sync word.
    #[field(label = "Sync")]
    pub sync: i64,
    /// The preamble length.
    #[field(label = "Preamble")]
    pub preamble: i64,
    /// Low data rate optimisation.
    #[field(label = "LDRO")]
    pub ldro: bool,
    /// The channel lies inside the captured band.
    #[field(label = "In band")]
    pub in_band: bool,
    /// Frames decoded.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames lost to errors.
    #[field(label = "Errors")]
    pub errors: i64,
    /// The signal to noise ratio of the last good frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last good frame.
    #[field(label = "Last offset", unit = "Hz")]
    pub cfo: Option<Hertz>,
    /// The signal time of the last frame.
    #[field(label = "Last frame", unit = "s")]
    pub last_s: Option<f64>,
}

/// A row of the MeshCore contacts. The row key is the hex of the public key.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshContactRow<'a> {
    /// The name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The first 12 hex digits of the public key.
    #[field(label = "Key")]
    pub key: &'a str,
    /// The UTC of the last packet, when the clock is known.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The signal to noise ratio of the last packet.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The learned path, or "flood".
    #[field(label = "Path")]
    pub path: &'a str,
}

/// A row of the channels with keys. The row key is `<net key>/<name>`. The key itself is never shown.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshChannelRow<'a> {
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The name of the channel.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The channel holds a key.
    #[field(label = "Key set")]
    pub key_set: bool,
}

/// The fields of a text message, sent or received.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshTextEvent<'a> {
    /// The sender.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The addressee.
    #[field(shared = keys::TARGET)]
    pub target: &'a str,
    /// The channel.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The name of the sender, if known.
    #[field(label = "Sender")]
    pub sender: Option<&'a str>,
    /// The hops taken, if known.
    #[field(label = "Hops")]
    pub hops: Option<i64>,
    /// The packet id.
    #[field(label = "Packet id")]
    pub packet_id: i64,
    /// The message that the text replies to.
    #[field(label = "Reply to")]
    pub reply_id: i64,
    /// A reaction, as a Unicode code point.
    #[field(label = "Reaction")]
    pub emoji: i64,
    /// Sent by the own node.
    #[field(label = "Own")]
    pub own: bool,
    /// The delivery state of an own message.
    #[field(label = "State")]
    pub state: Option<DeliveryState>,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The signal to noise ratio of the frame of a received text.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the frame of a received text, relative to the full scale of the input (not an absolute power).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The text comes from a copy with a good CRC. False for a damaged copy shown after the wait; a later good copy sends
    /// the text again with true. None for an own message.
    #[field(label = "CRC passed")]
    pub crc_ok: Option<bool>,
}

/// A row of the Meshtastic frequency slots of the primary preset in the slot band. The row key is the slot number with
/// four digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshSlotRow {
    /// The slot number, from 1, as the setting `mt_channel_num` counts.
    #[field(label = "Slot")]
    pub slot: i64,
    /// The centre of the slot plus the setting `mt_freq_offset`.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The slot that the stack listens on: that of `mt_channel_num`, else of the hash of `mt_channel_name`; with
    /// `mt_override_freq` the slot whose frequency it is, and no row for a frequency between slots.
    #[field(label = "Current")]
    pub current: bool,
    /// The slot of the default channel name of the primary preset.
    #[field(label = "Preset default")]
    pub preset_default: bool,
}

/// The fields of a node heard for the first time or with news.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeshNodeEvent<'a> {
    /// The node id.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The node name, if known.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The position, if known.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The protocol.
    #[field(label = "Net")]
    pub net: Protocol,
    /// The family of the event.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The packet list, newest last.
pub static MESH_PACKETS: Table<MeshPacketRow<'static>> =
    Table::new("packets", "Packets", crate::mesh::store::PACKETS_CAP).label(&MeshPacketRow::SOURCE);
/// The node list.
pub static MESH_NODES: Table<MeshNodeRow<'static>> =
    Table::new("nodes", "Nodes", crate::mesh::store::NODES_CAP).label(&MeshNodeRow::NAME);
/// The messages, newest last.
pub static MESH_MESSAGES: Table<MeshMessageRow<'static>> =
    Table::new("messages", "Messages", crate::mesh::store::MESSAGES_CAP)
        .label(&MeshMessageRow::SENDER);
/// The telemetry and position readings, newest last.
pub static MESH_READINGS: Table<MeshReadingRow<'static>> =
    Table::new("readings", "Readings", crate::mesh::store::READINGS_SHOWN)
        .label(&MeshReadingRow::NODE);
/// The APRS stations of LoRa APRS and MeshCom.
pub static MESH_STATIONS: Table<MeshStationRow<'static>> =
    Table::new("stations", "Stations", crate::mesh::store::STATIONS_CAP)
        .label(&MeshStationRow::CALL);
/// The LoRaWAN devices.
pub static MESH_DEVICES: Table<MeshDeviceRow<'static>> =
    Table::new("devices", "Devices", crate::mesh::store::DEVICES_CAP).label(&MeshDeviceRow::ID);
/// The packet counts by protocol and type.
pub static MESH_TYPES: Table<MeshTypeRow<'static>> =
    Table::new("types", "Packet types", crate::mesh::store::TYPES_CAP).label(&MeshTypeRow::TYPE);
/// The decoders.
pub static MESH_DECODERS: Table<MeshDecoderRow<'static>> = Table::new("decoders", "Decoders", 400)
    .label(&MeshDecoderRow::PRESET)
    .sorted(&MeshDecoderRow::FREQ, sdrkit_core::SortOrder::Ascending);
/// The MeshCore contacts.
pub static MESH_CONTACTS: Table<MeshContactRow<'static>> =
    Table::new("contacts", "Contacts", crate::mesh::mc_client::CONTACTS_CAP)
        .label(&MeshContactRow::NAME);
/// The channels with keys that the user supplied.
pub static MESH_CHANNELS: Table<MeshChannelRow<'static>> = Table::new(
    "channels",
    "Channels",
    crate::components::mesh::CHANNELS_CAP,
)
.label(&MeshChannelRow::CHANNEL);
/// The Meshtastic frequency slots of the primary preset.
pub static MESH_SLOTS: Table<MeshSlotRow> =
    Table::new("slots", "Meshtastic slots", crate::mesh::plan::SLOTS_CAP)
        .label(&MeshSlotRow::SLOT)
        .sorted(&MeshSlotRow::SLOT, sdrkit_core::SortOrder::Ascending);
/// A text message, sent or received.
pub static MESH_TEXT: EventKind<MeshTextEvent<'static>> =
    EventKind::new("mesh.text", "Mesh text message");
/// A node heard for the first time or with news.
pub static MESH_NODE: EventKind<MeshNodeEvent<'static>> =
    EventKind::new("mesh.node", "Mesh node heard");

/// The row key of a row of the node list.
pub const MESH_NODE_ROW: Argument<field::Text> = objcmd::row_of("nodes");

/// The arguments of `remove` of the mesh stacks.
#[derive(Arguments)]
pub struct MeshRemoveArgs<'a> {
    /// The row key of the node.
    #[arg(shared = MESH_NODE_ROW)]
    pub row: &'a str,
}

/// Forgets one node.
pub const MESH_REMOVE: CommandKind<MeshRemoveArgs<'static>> = objcmd::remove_kind();

/// The arguments of `mt.send`.
#[derive(Arguments)]
pub struct MtSendArgs<'a> {
    /// The text to send.
    #[arg(label = "Text")]
    pub text: &'a str,
    /// The channel of the text.
    #[arg(shared = CHANNEL)]
    pub channel: Option<&'a str>,
    /// The addressee.
    #[arg(shared = MT_TO)]
    pub to: Option<&'a str>,
    /// The packet id that the text replies to.
    #[arg(shared = REPLY_ID)]
    pub reply_id: Option<i64>,
    /// A reaction.
    #[arg(shared = EMOJI)]
    pub emoji: Option<i64>,
    /// Whether the text asks for an acknowledgement.
    #[arg(shared = WANT_ACK)]
    pub want_ack: Option<WantAck>,
    /// The hop limit of the text.
    #[arg(shared = HOP_LIMIT)]
    pub hop_limit: Option<i64>,
}

/// The reply of `mt.send`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MtSendReply {
    /// The packet id of the text: the column `packet_id` of its row in the messages and the field `packet_id` of its
    /// `mesh.text` events. The column `state` of the row says whether it was acknowledged.
    #[field(label = "Packet id")]
    pub packet_id: i64,
}

/// The arguments of `mc.send`.
#[derive(Arguments)]
pub struct McSendArgs<'a> {
    /// The text to send.
    #[arg(label = "Text")]
    pub text: &'a str,
    /// The channel of the text.
    #[arg(shared = CHANNEL)]
    pub channel: Option<&'a str>,
    /// The addressee of a direct text.
    #[arg(shared = MC_TO)]
    pub to: Option<&'a str>,
}

/// The arguments of `mt.channel.add`.
#[derive(Arguments)]
pub struct MtChannelAddArgs<'a> {
    /// The name of the channel.
    #[arg(label = "Name")]
    pub name: &'a str,
    /// The key of the channel.
    #[arg(shared = KEY)]
    pub key: Option<&'a str>,
}

/// The arguments of `mc.channel.add`.
#[derive(Arguments)]
pub struct McChannelAddArgs<'a> {
    /// The name of the channel.
    #[arg(label = "Name")]
    pub name: &'a str,
    /// The secret of the channel.
    #[arg(shared = MC_KEY)]
    pub key: Option<&'a str>,
}

/// The arguments of `channel.remove`.
#[derive(Arguments)]
pub struct ChannelRemoveArgs<'a> {
    /// The row key of the channel.
    #[arg(shared = CHANNEL_ROW)]
    pub row: &'a str,
}

/// The arguments of `mc.advert`.
#[derive(Arguments)]
pub struct McAdvertArgs {
    /// Where the advert goes.
    #[arg(shared = SCOPE)]
    pub scope: Option<AdvertScope>,
}

/// The arguments of `mc.contact.add`.
#[derive(Arguments)]
pub struct McContactAddArgs<'a> {
    /// The public key of the contact.
    #[arg(label = "Public key", help = "32 bytes in hex or base64.")]
    pub key: &'a str,
    /// The name of the contact.
    #[arg(shared = CONTACT_NAME)]
    pub name: Option<&'a str>,
}

/// The arguments of `mt.request`.
#[derive(Arguments)]
pub struct MtRequestArgs<'a> {
    /// The node that the request goes to.
    #[arg(label = "Node", help = "A node id \"!xxxxxxxx\".")]
    pub node: &'a str,
    /// What the request is for.
    #[arg(shared = WHAT)]
    pub what: Option<MtRequest>,
}

/// The arguments of `mt.channel`.
#[derive(Arguments)]
pub struct MtChannelArgs<'a> {
    /// The slot: 0 is the primary channel.
    #[arg(shared = INDEX)]
    pub index: Option<i64>,
    /// The name of the channel.
    #[arg(label = "Name")]
    pub name: &'a str,
    /// The key of the channel.
    #[arg(shared = KEY)]
    pub key: Option<&'a str>,
    /// The position precision.
    #[arg(shared = PRECISION)]
    pub precision: Option<i64>,
}

/// The arguments of `mt.identity` and `mc.identity`: a private key or an identity seed.
#[derive(Arguments)]
pub struct IdentityArgs<'a> {
    /// The key in base64 or hex.
    #[arg(
        label = "Key",
        help = "32 bytes in base64 or hex. The key is kept in the saved state and never shown."
    )]
    pub key: &'a str,
}

/// The help of the commands that add a channel.
const CHANNEL_ADD_HELP: &str =
    "The session reads the texts of the channel and can send on it. The key is kept and never shown.";

/// Adds a Meshtastic channel: the session reads its texts and can send on it.
pub const MT_CHANNEL_ADD: CommandKind<MtChannelAddArgs<'static>> =
    CommandKind::new("mt.channel.add", "Add a Meshtastic channel").help(CHANNEL_ADD_HELP);
/// Adds a MeshCore channel: the session reads its texts and can send on it.
pub const MC_CHANNEL_ADD: CommandKind<McChannelAddArgs<'static>> =
    CommandKind::new("mc.channel.add", "Add a MeshCore channel").help(CHANNEL_ADD_HELP);
/// Removes a channel key that the user added.
pub const CHANNEL_REMOVE: CommandKind<ChannelRemoveArgs<'static>> =
    CommandKind::new("channel.remove", "Remove a channel");
/// Adds a MeshCore contact by its public key.
pub const CONTACT_ADD: CommandKind<McContactAddArgs<'static>> =
    CommandKind::new("mc.contact.add", "Add a MeshCore contact");
/// Sets an own Meshtastic channel.
pub const MT_CHANNEL: CommandKind<MtChannelArgs<'static>> =
    CommandKind::new("mt.channel", "Set a Meshtastic channel slot").help("Sets the key and the position precision of one of the 8 channel slots. Slot 0 is the primary channel: its name comes from the setting `mt_channel_name`.");
/// Sets the own Meshtastic private key.
pub const MT_IDENTITY: CommandKind<IdentityArgs<'static>> =
    CommandKind::new("mt.identity", "Set the Meshtastic key");
/// Sends a Meshtastic text.
pub const MT_SEND: CommandKind<MtSendArgs<'static>, MtSendReply> =
    CommandKind::new("mt.send", "Send a Meshtastic text")
        .help("To a node (\"!xxxxxxxx\") or to the channel (\"^all\"). The reply holds the packet id of the text; the column `state` of its row in the messages says whether it was delivered.")
        .transmits(TxContent::Text);
/// Sends a request to a Meshtastic node.
pub const MT_REQUEST: CommandKind<MtRequestArgs<'static>> =
    CommandKind::new("mt.request", "Ask a Meshtastic node").transmits(TxContent::Frame);
/// Sets the own MeshCore identity seed.
pub const MC_IDENTITY: CommandKind<IdentityArgs<'static>> =
    CommandKind::new("mc.identity", "Set the MeshCore identity");
/// Sends a MeshCore text.
pub const MC_SEND: CommandKind<McSendArgs<'static>> =
    CommandKind::new("mc.send", "Send a MeshCore text").transmits(TxContent::Text);
/// Sends the own MeshCore advert.
pub const MC_ADVERT: CommandKind<McAdvertArgs> =
    CommandKind::new("mc.advert", "Send a MeshCore advert").transmits(TxContent::Beacon);

/// A stack of the mesh family. The stacks of one protocol have the commands of their client only.
fn mesh_stack(id: &'static str) -> StackShape {
    let (mt, mc, aprs) = (
        matches!(id, "mesh" | "meshtastic"),
        matches!(id, "mesh" | "meshcore"),
        matches!(id, "mesh" | "loraaprs"),
    );
    let mut s = StackShape::new(id)
        .fields::<MeshFields>()
        .fields::<MeshMtFields>()
        .fields::<MeshMcFields>()
        .fields::<HamFields>()
        .fields::<MeshTxFields>()
        .receiver_fields::<GuardFields>()
        .component("modulate", |c| c.fields::<ModulateFields>())
        .table(&MESH_PACKETS)
        .table(&MESH_NODES)
        .table(&MESH_MESSAGES)
        .table(&MESH_READINGS)
        .table(&MESH_STATIONS)
        .table(&MESH_DEVICES)
        .table(&MESH_TYPES)
        .table(&MESH_DECODERS)
        .table(&MESH_CONTACTS)
        .table(&MESH_CHANNELS)
        .table(&MESH_SLOTS)
        .points("Nodes and stations")
        .event(&LORA_PACKET)
        .event(&MESH_TEXT)
        .event(&MESH_NODE)
        .event(&TX_REQUEST)
        .settings(crate::settings::mesh_settings(id));
    if mt {
        s = s.command(&MT_SEND);
    }
    if mc {
        s = s.command(&MC_SEND);
    }
    if aprs {
        s = s.command(&APRS_SEND);
    }
    if mt {
        s = s.command(&MT_CHANNEL_ADD);
    }
    if mc {
        s = s.command(&MC_CHANNEL_ADD);
    }
    if mt || mc {
        s = s.command(&CHANNEL_REMOVE);
    }
    if mc {
        s = s.command(&MC_ADVERT).command(&CONTACT_ADD);
    }
    if aprs {
        s = s.command(&crate::commands::APRS_BEACON);
    }
    if mt {
        s = s.command(&MT_REQUEST);
    }
    s = s
        .command(&objcmd::CLEAR)
        .command(&MESH_REMOVE)
        .command(bank_reset());
    if mt {
        s = s.command(&MT_CHANNEL).command(&MT_IDENTITY);
    }
    if mc {
        s = s.command(&MC_IDENTITY);
    }
    s.kept(&[
        "packets",
        "nodes",
        "messages",
        "readings",
        "stations",
        "devices",
        "types",
        "contacts",
        "channels",
        "slots",
        "MeshFields.mt_nodes",
        "MeshFields.mc_nodes",
        "MeshFields.messages",
        "MeshFields.stations",
        "MeshFields.devices",
        "MeshFields.preambles",
        "MeshFields.header_failed",
        "MeshMtFields.mt_node_id",
        "MeshMtFields.mt_public_key",
        "MeshMtFields.mt_long_name",
        "MeshMtFields.mt_short_name",
        "MeshMtFields.mt_in_flight",
        "MeshMtFields.mt_rebroadcasts",
        "MeshMtFields.mt_warnings",
        "MeshMcFields.mc_public_key",
        "MeshMcFields.mc_name",
        "MeshMcFields.mc_contacts",
        "MeshTxFields",
    ])
}

// LoRaWAN (all channels).

/// The result fields of the LoRaWAN stack of every channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WideFields<'a> {
    /// The level of the input, once the bank reports.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Frames with a good CRC.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that are not LoRaWAN or whose CRC failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frame preambles seen.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Headers that failed their checksum.
    #[field(label = "Headers failed")]
    pub header_failed: i64,
    /// FSK sync words found.
    #[field(label = "FSK syncs")]
    pub fsk_syncs: i64,
    /// FSK frames.
    #[field(label = "FSK frames")]
    pub fsk_frames: i64,
    /// FSK frames whose CRC failed.
    #[field(label = "FSK CRC errors")]
    pub fsk_crc_errors: i64,
    /// LoRa demodulators running.
    #[field(label = "Demodulators")]
    pub demodulators: i64,
    /// Channels in the plan.
    #[field(label = "Channels in the plan")]
    pub channels_listed: i64,
    /// Channels decoded.
    #[field(label = "Channels decoded")]
    pub channels_decoded: i64,
    /// What is decoded, in words.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The plan in words: its count of channels and their span.
    #[field(label = "Plan")]
    pub plan: &'a str,
    /// LoRaWAN devices.
    #[field(label = "LoRaWAN devices")]
    pub devices: i64,
}

/// A row of the LoRaWAN frame log. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WideFrameRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The frame type.
    #[field(label = "Type")]
    pub r#type: Option<&'a str>,
    /// The DevAddr or the DevEUI.
    #[field(label = "DevAddr / DevEUI")]
    pub device: Option<&'a str>,
    /// The network operator.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// The frame counter.
    #[field(label = "FCnt")]
    pub fcnt: Option<i64>,
    /// The FPort.
    #[field(label = "Port")]
    pub fport: Option<i64>,
    /// The encrypted payload length of a LoRaWAN frame.
    #[field(label = "Payload", unit = "B")]
    pub payload_bytes: Option<i64>,
    /// The FCtrl flags.
    #[field(label = "FCtrl")]
    pub fctrl: Option<&'a str>,
    /// The MAC commands.
    #[field(label = "MAC commands")]
    pub mac_commands: Option<&'a str>,
    /// The message integrity code, hex.
    #[field(label = "MIC")]
    pub mic: Option<&'a str>,
    /// How the frame was modulated.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The spreading factor, none for the FSK channel.
    #[field(label = "SF")]
    pub sf: Option<i64>,
    /// The signal to noise ratio of the frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The frequency of the frame.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// More about the frame.
    #[field(label = "Detail")]
    pub summary: &'a str,
}

/// A row of the LoRaWAN channels. The row key is the place of the channel in the plan.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WideChannelRow {
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// The bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bw: Hertz,
    /// The least spreading factor searched.
    #[field(label = "Least SF")]
    pub sf_min: i64,
    /// The greatest spreading factor searched.
    #[field(label = "Greatest SF")]
    pub sf_max: i64,
    /// The channel lies inside the captured band.
    #[field(label = "In band")]
    pub in_band: bool,
    /// Preambles.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Frames decoded.
    #[field(label = "Frames")]
    pub frames: i64,
    /// CRC errors.
    #[field(label = "CRC errors")]
    pub crc_errors: i64,
}

/// The LoRaWAN frame log, newest last.
pub static WIDE_FRAMES: Table<WideFrameRow<'static>> =
    Table::new("frames", "Frames", crate::components::lorawan::WIDE_LOG_CAP)
        .label(&WideFrameRow::DEVICE);
/// The LoRaWAN channels.
pub static WIDE_CHANNELS: Table<WideChannelRow> = Table::new("channels", "Channels", 200)
    .sorted(&WideChannelRow::FREQ, sdrkit_core::SortOrder::Ascending);

/// The LoRaWAN stack of every channel.
fn lorawanwide() -> StackShape {
    StackShape::new("lorawanwide")
        .fields::<WideFields>()
        .receiver_fields::<GuardFields>()
        .table(&WIDE_FRAMES)
        .table(&WIDE_CHANNELS)
        .table(&MESH_DEVICES)
        .table(&MESH_TYPES)
        .event(&LORA_PACKET)
        .settings(crate::settings::WideSetting::SETTINGS)
        .command(&objcmd::CLEAR)
        .command(bank_reset())
        .kept(&[
            "frames",
            "devices",
            "types",
            "WideFields.frames_ok",
            "WideFields.frames_bad",
            "WideFields.preambles",
            "WideFields.header_failed",
            "WideFields.devices",
        ])
}
