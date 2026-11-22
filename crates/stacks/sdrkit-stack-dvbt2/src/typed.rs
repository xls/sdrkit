//! The interface of the DVB-T2 family: the shapes of the result sets and the PLP rows, the PLP table, the commands of the
//! sender and what the stack declares.
//!
//! The stack shows the mode adaptation `ts` of `sdrkit-proto-ipcast` and the transport stream layer of
//! `sdrkit-proto-mpegts` through the shared interfaces of these crates.

use crate::params::{CodeRate, Fft, Guard, Modulation, PilotPattern, Profile};
use crate::results::{Detection, PayloadType, PlpType, TimeIl, TS_LAYER};
use crate::rx::Problem;
use crate::settings::Dvbt2Setting;
use sdrkit_core::interface::{CommandKind, Family, Fields, Hertz, Seconds, StackShape, Table};
use sdrkit_core::publish::keys;
use sdrkit_core::{Command, SettingSet, TxContent};
use sdrkit_proto_ipcast::components::{self as ipcast, BbFrameFields};
use sdrkit_proto_mpegts::layer;
use sdrkit_proto_mpegts::publish::{SERVICE_SELECT_KIND, TELETEXT_SELECT, TRACK_SELECT_KIND};
use sdrkit_proto_mpegts::sendqueue::TxAppendArgs;
use std::sync::OnceLock;

/// The result fields of `dvbt2`, which the receiver `rx` writes. A value of the frame shape has no value before the
/// P2 symbols are found, and a value of the L1 signalling has no value before the L1-pre is decoded.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbt2Fields {
    /// Detection: searching, P1 found, L1 decoded, or packets flowing.
    #[field(label = "Detection")]
    pub detection: Detection,
    /// The FFT size.
    #[field(label = "FFT size")]
    pub fft: Option<Fft>,
    /// The guard interval.
    #[field(label = "Guard interval")]
    pub guard: Option<Guard>,
    /// The scattered pilot pattern.
    #[field(label = "Pilot pattern")]
    pub pilot_pattern: Option<PilotPattern>,
    /// Extended carrier mode.
    #[field(label = "Extended carriers")]
    pub extended_carriers: Option<bool>,
    /// MISO.
    #[field(label = "MISO")]
    pub miso: Option<bool>,
    /// The profile.
    #[field(label = "Profile")]
    pub profile: Option<Profile>,
    /// The L1-post constellation.
    #[field(label = "L1-post constellation")]
    pub l1_modulation: Option<Modulation>,
    /// CELL_ID.
    #[field(label = "Cell id")]
    pub cell_id: Option<i64>,
    /// NETWORK_ID.
    #[field(label = "Network id")]
    pub network_id: Option<i64>,
    /// T2_SYSTEM_ID.
    #[field(label = "T2 system id")]
    pub t2_system_id: Option<i64>,
    /// NUM_T2_FRAMES.
    #[field(label = "T2-frames per super-frame")]
    pub frames_per_superframe: Option<i64>,
    /// NUM_DATA_SYMBOLS.
    #[field(label = "Data symbols per frame")]
    pub data_symbols: Option<i64>,
    /// The T2-frame length.
    #[field(label = "Frame length", unit = "s")]
    pub frame_length: Option<Seconds>,
    /// T2-frames decoded.
    #[field(label = "T2-frames")]
    pub t2_frames: i64,
    /// L1 signalling failures.
    #[field(label = "L1 failures")]
    pub l1_errors: i64,
    /// The data PLP decoded.
    #[field(label = "PLP")]
    pub plp: Option<i64>,
    /// Its common PLP.
    #[field(label = "Common PLP")]
    pub common_plp: Option<i64>,
    /// Mean LDPC iterations per FEC block.
    #[field(label = "LDPC iterations")]
    pub ldpc_iterations: f64,
    /// The net bit rate of the decoded data PLP.
    #[field(label = "Net bit rate", unit = "bit/s")]
    pub net_rate_bit_s: Option<f64>,
    /// A configuration that the receiver cannot follow.
    #[field(label = "Problem")]
    pub problem: Option<Problem>,
    /// The carrier offset from the centre. No value while the receiver searches.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// FEC blocks good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// FEC blocks that failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Bits that the BCH decoder corrected.
    #[field(shared = keys::BITS_CORRECTED)]
    pub bits_corrected: i64,
}

/// A row of the PLPs of the multiplex. The row key is PLP_ID in decimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbt2PlpRow {
    /// PLP_ID.
    #[field(label = "PLP")]
    pub plp_id: i64,
    /// PLP_TYPE.
    #[field(label = "Type")]
    pub plp_type: PlpType,
    /// PLP_PAYLOAD_TYPE.
    #[field(label = "Payload")]
    pub payload: PayloadType,
    /// The constellation. No value for a reserved code.
    #[field(label = "Constellation")]
    pub modulation: Option<Modulation>,
    /// The code rate. No value for a code that the profile does not use.
    #[field(label = "Code rate")]
    pub code_rate: Option<CodeRate>,
    /// The FEC frame length. No value for a reserved code.
    #[field(label = "FEC frame", unit = "bits")]
    pub fec_frame_bits: Option<i64>,
    /// Rotated constellation.
    #[field(label = "Rotated")]
    pub rotated: bool,
    /// PLP_NUM_BLOCKS of this frame.
    #[field(label = "FEC blocks")]
    pub fec_blocks: i64,
    /// PLP_GROUP_ID.
    #[field(label = "Group")]
    pub group_id: i64,
    /// How the PLP is time interleaved.
    #[field(label = "Time interleaving")]
    pub time_interleaving: TimeIl,
    /// TIME_IL_LENGTH: TI-blocks per frame, or T2-frames per TI-block.
    #[field(label = "Time interleaving length")]
    pub time_il_length: i64,
}

/// The PLPs of the multiplex.
pub static PLPS: Table<Dvbt2PlpRow> = Table::new("plps", "PLPs", 256);

/// The result fields of the sender `transmit` of `dvbt2`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbt2TxFields<'a> {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the centre of the channel sent. No value while the stack does not transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// The bit rate of the transport stream that the settings carry: the rate that the host feeds at most. 0 while the
    /// settings cannot be sent.
    #[field(label = "Useful bit rate", unit = "bit/s")]
    pub tx_bitrate_bps: f64,
    /// The packets of the host sent in this transmission, or in the last one.
    #[field(label = "Packets sent")]
    pub tx_packets: i64,
    /// The null packets put in slots that found no packet of the host.
    #[field(label = "Null packets inserted")]
    pub tx_nulls: i64,
    /// The packets that wait to be sent.
    #[field(label = "Packets waiting")]
    pub tx_queue: i64,
    /// The time that the waiting packets take at the useful bit rate.
    #[field(label = "Queue length", unit = "s")]
    pub tx_queue_s: Seconds,
    /// The PCRs rewritten to the time of their slot.
    #[field(label = "PCRs rewritten")]
    pub tx_pcr_restamped: i64,
    /// The length of a T2-frame. No value while the settings cannot be sent.
    #[field(label = "Frame length", unit = "s")]
    pub tx_frame_s: Option<Seconds>,
    /// The FEC blocks of a T2-frame. No value while the settings cannot be sent.
    #[field(label = "FEC blocks per frame")]
    pub tx_fec_blocks: Option<i64>,
    /// Why the settings cannot be sent. No value while they can.
    #[field(label = "Settings refused")]
    pub tx_problem: Option<&'a str>,
}

/// Starts sending.
pub const TX_START: Command = Command::new("start", "Start sending")
    .help(
        "Sends a DVB-T2 signal with the tx_* settings from the start of a T2-frame, until stop. The packets of append \
         fill it; a slot with no packet carries a null packet. Transmit must be on.",
    )
    .transmits(TxContent::File)
    .stopped_by("stop");
/// Stops sending.
pub const TX_STOP: Command = Command::new("stop", "Stop sending").help(
    "Ends the transmission after the T2-frame on air with a fall of 5 ms. The packets that wait are dropped.",
);
/// Adds packets to the stream.
pub const TX_APPEND: CommandKind<TxAppendArgs> = CommandKind::new("append", "Add packets").help(
    "Adds transport packets to the queue of the stream. All or none: refused when the queue has no room, so the host \
     feeds at the useful bit rate.",
);

/// The declarations of the sender of `dvbt2`, added in 1.2.
const ADDED_1_2: &[&str] = &[
    "dvbt2.fields.Dvbt2TxFields",
    "shape.Dvbt2TxFields",
    "shape.Dvbt2TxFields.transmitting",
    "shape.Dvbt2TxFields.tx_freq_hz",
    "shape.Dvbt2TxFields.tx_bitrate_bps",
    "shape.Dvbt2TxFields.tx_packets",
    "shape.Dvbt2TxFields.tx_nulls",
    "shape.Dvbt2TxFields.tx_queue",
    "shape.Dvbt2TxFields.tx_queue_s",
    "shape.Dvbt2TxFields.tx_pcr_restamped",
    "shape.Dvbt2TxFields.tx_frame_s",
    "shape.Dvbt2TxFields.tx_fec_blocks",
    "shape.Dvbt2TxFields.tx_problem",
    "dvbt2.setting.tx_bandwidth",
    "dvbt2.setting.tx_fft",
    "dvbt2.setting.tx_guard",
    "dvbt2.setting.tx_pilots",
    "dvbt2.setting.tx_extended",
    "dvbt2.setting.tx_modulation",
    "dvbt2.setting.tx_rate",
    "dvbt2.setting.tx_rotation",
    "dvbt2.setting.tx_fec_frame",
    "dvbt2.setting.tx_ti_blocks",
    "dvbt2.setting.tx_frame_ms",
    "dvbt2.setting.tx_l1_mod",
    "dvbt2.setting.tx_cell_id",
    "dvbt2.setting.tx_network_id",
    "dvbt2.setting.tx_system_id",
    "dvbt2.command.start",
    "dvbt2.command.stop",
    "dvbt2.command.append",
];

/// The settings of the sender for T2-Lite and MISO, added in 1.3.
const ADDED_1_3: &[&str] = &["dvbt2.setting.tx_profile", "dvbt2.setting.tx_miso"];

/// The description of the receiver graph of `dvbt2`, which the stack without its sender runs.
pub fn receiver_shape() -> &'static StackShape {
    static SHAPE: OnceLock<StackShape> = OnceLock::new();
    SHAPE.get_or_init(|| {
        let s = StackShape::new(crate::ID)
            .fields::<Dvbt2Fields>()
            .table(&PLPS);
        let s = ipcast::describe::<BbFrameFields>(s, "ts");
        // The guide holds the events of every multiplex that named them, so it is kept over a change of frequency; the
        // PLPs, the services, the tracks and every measurement belong to the multiplex of the frequency before.
        layer::describe(s, &TS_LAYER)
            .settings(Dvbt2Setting::SETTINGS)
            .command(&SERVICE_SELECT_KIND)
            .command(&TELETEXT_SELECT)
            .command(&TRACK_SELECT_KIND)
            .kept(&["guide", "guide_ratings"])
    })
}

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let dvbt2 = receiver_shape()
            .clone()
            .fields::<Dvbt2TxFields<'static>>()
            .command(&TX_START)
            .command(&TX_STOP)
            .command(&TX_APPEND);
        Family::new("dvbt2", 1, 3)
            .uses(sdrkit_proto_mpegts::publish::interface())
            .uses(ipcast::interface())
            .shared_source("graph", &["dvbt2"])
            .shared_source("results", &["dvbt2"])
            .stack(dvbt2)
            .added(1, 1, &["dvbt2.setting.decode_video"])
            .added(1, 2, ADDED_1_2)
            .added(1, 3, ADDED_1_3)
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
        .unwrap_or_else(|| panic!("the dvbt2 family does not describe the stack {id}"))
}
