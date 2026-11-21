//! The interface of the cable and satellite television family: the shapes of the result sets, the table rows and the
//! events, the tables, the event types and what each stack declares.
//!
//! The TV stacks (`dvbc`, `dvbc2`, `dvbs`, `isdbs`) show the transport stream layer of `sdrkit-proto-mpegts` and the
//! components of `sdrkit-proto-ipcast` through the shared interfaces of these crates.
//!
//! The stack `dvbs` shows the results of the receiver of `sdrkit-proto-dvbs2` through its shared interface `dvbs2`.

use crate::beacon::frames::{Confidence, SyncWord, ViewKind};
use crate::beacon::{BeaconSetting, BeaconState, Coding};
use crate::dvbc::demod::{Note as DvbcNote, Stage};
use crate::dvbc::qam::Qam;
use crate::dvbc::{Annex, DvbcSetting};
use crate::dvbc2::comp::{HeaderKind, PayloadType, PlpType, SliceKind};
use crate::dvbc2::rx::State as C2State;
use crate::dvbc2::settings::Dvbc2Setting;
use crate::dvbc2::signal::Guard;
use crate::dvbc2::tables::{CodeRate as C2CodeRate, Qam as C2Qam};
use crate::isdbs::tx::McChoice;
use crate::isdbs::{IsdbsSetting, RxState};
use crate::TS_LAYER;
use sdrkit_core::interface::{
    CommandKind, EventKind, Family, Fields, Hertz, Seconds, StackShape, Table,
};
use sdrkit_core::publish::{keys, SeriesDef};
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{Command, SettingSet, TxContent};
use sdrkit_proto_dvbs2::settings::DvbsSetting;
use sdrkit_proto_dvbs2::typed::{dvbs2_interface, with_dvbs2_receiver};
use sdrkit_proto_ipcast::components as ipcast;
use sdrkit_proto_mpegts::layer;
use sdrkit_proto_mpegts::sendqueue::TxAppendArgs;
use std::sync::{Arc, OnceLock};

// The DVB-S2 receiver.
// DVB-C.

/// The result fields of `dvbc`: the carrier, the demodulator, the outer decoder and the locks. A measurement of the
/// carrier has no value before a carrier is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbcFields {
    /// What the demodulator is doing.
    #[field(label = "Stage")]
    pub stage: Stage,
    /// The reason the demodulator does not lock, when known.
    #[field(label = "Note")]
    pub note: Option<DvbcNote>,
    /// The QAM order in use.
    #[field(label = "QAM")]
    pub qam: Option<Qam>,
    /// The symbol rate in use.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The symbol rate measured from the spectrum.
    #[field(label = "Measured symbol rate", unit = "Hz")]
    pub measured_rate: Option<Hertz>,
    /// The manual symbol rate is in use.
    #[field(label = "Manual symbol rate")]
    pub manual_rate: bool,
    /// The -3 dB width of the carrier.
    #[field(label = "Width (-3 dB)", unit = "Hz")]
    pub width: Option<Hertz>,
    /// The annex of J.83.
    #[field(label = "Annex")]
    pub annex: Annex,
    /// The roll-off of the annex.
    #[field(label = "Roll-off")]
    pub roll_off: f64,
    /// The spectrum is inverted.
    #[field(label = "Spectrum inverted")]
    pub inverted: bool,
    /// The error of the symbol clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The carrier offset from the centre.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The modulation error ratio.
    #[field(shared = keys::MER_DB)]
    pub mer_db: Option<f64>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The fraction of bytes Reed-Solomon changed.
    #[field(label = "Byte errors before RS")]
    pub pre_rs: f64,
    /// Reed-Solomon words with no error.
    #[field(label = "RS words clean")]
    pub rs_clean: i64,
    /// Reed-Solomon words corrected.
    #[field(label = "RS words corrected")]
    pub rs_corrected: i64,
    /// Reed-Solomon words that failed.
    #[field(label = "RS words failed")]
    pub rs_failed: i64,
    /// Byte re-alignments without a loss of lock.
    #[field(label = "Byte re-alignments")]
    pub realigned: i64,
    /// The net bit rate. No value while the receiver is not locked.
    #[field(label = "Net bit rate", unit = "bit/s")]
    pub net_bitrate_bps: Option<f64>,
    /// The carrier is found in the spectrum.
    #[field(label = "Spectrum lock")]
    pub spectrum_lock: bool,
    /// The symbol timing holds.
    #[field(label = "Timing lock")]
    pub timing_lock: bool,
    /// The equaliser has converged.
    #[field(label = "Equaliser lock")]
    pub equaliser_lock: bool,
    /// The carrier loop holds.
    #[field(label = "Carrier lock")]
    pub carrier_lock: bool,
    /// The sync bytes are found.
    #[field(label = "Sync lock")]
    pub sync_lock: bool,
    /// The transport stream is valid.
    #[field(label = "TS lock")]
    pub ts_lock: bool,
    /// Acquisitions started again.
    #[field(label = "Searches")]
    pub searches: i64,
    /// Frames good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames bad.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
}

// DVB-C2.

/// The result fields of the receiver of `dvbc2`: the signal, the L1 signalling and the FEC. A value of the L1 has
/// no value before the L1 is read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbc2Fields<'a> {
    /// What the receiver does now.
    #[field(label = "State")]
    pub state: C2State,
    /// The guard interval.
    #[field(label = "Guard interval")]
    pub guard: Option<Guard>,
    /// The carrier offset from the centre. No value while the receiver searches.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the sample clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The carrier at the centre of the receive window.
    #[field(label = "Window centre carrier")]
    pub window_carrier: Option<i64>,
    /// The frequency of the centre of the receive window.
    #[field(label = "Window centre", unit = "Hz")]
    pub window_hz: Option<Hertz>,
    /// Es/N0.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// The modulation error ratio.
    #[field(shared = keys::MER_DB)]
    pub mer_db: Option<f64>,
    /// The network id of the L1.
    #[field(label = "Network id")]
    pub network_id: Option<i64>,
    /// The C2 system id of the L1.
    #[field(label = "C2 system id")]
    pub system_id: Option<i64>,
    /// The start frequency of the C2 system.
    #[field(label = "Start frequency", unit = "Hz")]
    pub start_frequency_hz: Option<Hertz>,
    /// The width of the C2 system.
    #[field(label = "System width", unit = "Hz")]
    pub bandwidth_hz: Option<Hertz>,
    /// The change counter of the L1.
    #[field(label = "L1 change counter")]
    pub change_counter: Option<i64>,
    /// The early warning flag of the L1.
    #[field(label = "Early warning")]
    pub ews: Option<bool>,
    /// The C2 version of the L1.
    #[field(label = "C2 version")]
    pub c2_version: Option<i64>,
    /// Reserved tones are in use.
    #[field(label = "Reserved tones")]
    pub reserved_tones: Option<bool>,
    /// FEC blocks good.
    #[field(label = "FEC blocks good")]
    pub blocks_ok: i64,
    /// FEC blocks that failed.
    #[field(label = "FEC blocks bad")]
    pub blocks_bad: i64,
    /// L1 blocks that failed.
    #[field(label = "L1 failures")]
    pub l1_bad: i64,
    /// Losses of the frame sync.
    #[field(label = "Sync losses")]
    pub sync_losses: i64,
    /// The bit error ratio before the LDPC decoder.
    #[field(label = "BER before LDPC")]
    pub pre_ber: Option<f64>,
    /// LDPC iterations, recent mean.
    #[field(label = "LDPC iterations")]
    pub mean_iterations: Option<f64>,
    /// C2 frames received.
    #[field(label = "C2 frames")]
    pub c2_frames: i64,
    /// The data slice decoded.
    #[field(label = "Data slice")]
    pub chosen_slice: Option<i64>,
    /// The PLP decoded.
    #[field(label = "PLP")]
    pub chosen_plp: Option<i64>,
    /// The common PLP of the PLP decoded.
    #[field(label = "Common PLP")]
    pub common_plp: Option<i64>,
    /// The data slices that carry the PLP, when more than one.
    #[field(label = "Bundled slices")]
    pub bundled_slices: Option<i64>,
    /// The frequency to tune to for the chosen data slice, when it is outside the window.
    #[field(label = "Tune to", unit = "Hz")]
    pub retune_hz: Option<Hertz>,
    /// The data slice that the decoding needs and that reaches out of the receive window.
    #[field(label = "Slice out of the window")]
    pub outside_slice: Option<i64>,
    /// How far that data slice reaches beyond the edge of the receive window.
    #[field(label = "Out of the window by", unit = "Hz")]
    pub outside_hz: Option<Hertz>,
    /// Packets of a generic stream.
    #[field(label = "Generic stream packets")]
    pub generic_packets: i64,
    /// Bytes of a generic stream.
    #[field(label = "Generic stream bytes")]
    pub generic_bytes: i64,
    /// A note on what the receiver decodes.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The counts of the stream adaptation and the IP layer that `dvbc2` shows under its own keys.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbc2StreamFields {
    /// Transport packets of the chosen PLP.
    #[field(label = "TS packets")]
    pub ts_packets: i64,
    /// GSE PDUs.
    #[field(label = "GSE PDUs")]
    pub pdus: i64,
    /// IP packets.
    #[field(label = "IP packets")]
    pub ip_packets: i64,
}

/// A row of the data slices of the L1. The row key is DSLICE_ID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbc2SliceRow {
    /// DSLICE_ID.
    #[field(label = "Slice")]
    pub slice_id: i64,
    /// DSLICE_TYPE.
    #[field(label = "Type")]
    pub slice_type: SliceKind,
    /// The FECFrame header of a type 2 slice.
    #[field(label = "Header")]
    pub header_type: HeaderKind,
    /// The tuning position of the slice.
    #[field(label = "Tuning position", unit = "Hz")]
    pub tune_hz: Hertz,
    /// The first carrier of the slice.
    #[field(label = "Start", unit = "Hz")]
    pub start_hz: Option<Hertz>,
    /// The last carrier of the slice.
    #[field(label = "End", unit = "Hz")]
    pub end_hz: Option<Hertz>,
    /// The depth of the time interleaver.
    #[field(label = "Time interleaving", unit = "symbols")]
    pub ti_depth: i64,
    /// The PLPs of the slice.
    #[field(label = "PLPs")]
    pub plp_count: i64,
    /// The slice is inside the receive window.
    #[field(label = "In the window")]
    pub in_window: bool,
    /// The width of the slice.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Option<Hertz>,
}

/// The data slices of the L1.
pub static DVBC2_SLICES: Table<Dvbc2SliceRow> = Table::new("slices", "Data slices", 256);

/// A row of the PLPs of the L1. The row key is `<slice>/<PLP_ID>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbc2PlpRow {
    /// The data slice of the PLP.
    #[field(label = "Slice")]
    pub plp_slice: i64,
    /// PLP_ID.
    #[field(label = "PLP")]
    pub plp_id: i64,
    /// PLP_TYPE.
    #[field(label = "Type")]
    pub plp_type: PlpType,
    /// PLP_PAYLOAD_TYPE.
    #[field(label = "Payload")]
    pub payload: PayloadType,
    /// The length of the FEC frame.
    #[field(label = "FEC frame", unit = "bits")]
    pub fec_frame_bits: Option<i64>,
    /// The constellation.
    #[field(label = "Constellation")]
    pub modulation: Option<C2Qam>,
    /// The code rate.
    #[field(label = "Code rate")]
    pub code_rate: Option<C2CodeRate>,
    /// The transport stream id. No value for a PLP with PSI reprocessing.
    #[field(label = "TS id")]
    pub ts_id: Option<i64>,
    /// The original network id. No value for a PLP with PSI reprocessing.
    #[field(label = "ON id")]
    pub on_id: Option<i64>,
    /// It is the PLP decoded.
    #[field(label = "Decoded")]
    pub chosen: bool,
    /// PLP_GROUP_ID. No value for a normal data PLP.
    #[field(label = "Group")]
    pub group: Option<i64>,
    /// PLP_BUNDLED: the PLP is spread over several data slices.
    #[field(label = "Bundled")]
    pub bundled: bool,
}

/// The PLPs of the L1.
pub static DVBC2_PLPS: Table<Dvbc2PlpRow> = Table::new("plps", "PLPs", 1024);

/// A row of the notches of the L1. The row key is the number of the notch.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbc2NotchRow {
    /// The first carrier of the notch.
    #[field(label = "Start", unit = "Hz")]
    pub notch_start_hz: Option<Hertz>,
    /// The last carrier of the notch.
    #[field(label = "End", unit = "Hz")]
    pub notch_end_hz: Option<Hertz>,
}

/// The notches of the L1.
pub static DVBC2_NOTCHES: Table<Dvbc2NotchRow> = Table::new("notches", "Notches", 16);

// ISDB-S.

/// The result fields of `isdbs`: the demodulator, the TMCC and the outer decoder. The offset has no value before the
/// frame sync is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbsFields {
    /// What the receiver does now.
    #[field(label = "State")]
    pub state: RxState,
    /// The frame sync words are found.
    #[field(label = "Frame sync")]
    pub frame_sync: bool,
    /// The carrier offset from the centre.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Es/N0.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// Frames received.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Super-frames received.
    #[field(label = "Super-frames")]
    pub superframes: i64,
    /// Losses of the frame sync.
    #[field(label = "Sync losses")]
    pub sync_losses: i64,
    /// TMCC words good.
    #[field(label = "TMCC good")]
    pub tmcc_good: i64,
    /// TMCC words that failed.
    #[field(label = "TMCC bad")]
    pub tmcc_bad: i64,
    /// The change counter of the TMCC.
    #[field(label = "TMCC change counter")]
    pub change_counter: Option<i64>,
    /// Reed-Solomon words with no error.
    #[field(label = "RS words clean")]
    pub rs_clean: i64,
    /// Reed-Solomon words corrected.
    #[field(label = "RS words corrected")]
    pub rs_corrected: i64,
    /// Reed-Solomon words that failed.
    #[field(label = "RS words failed")]
    pub rs_failed: i64,
    /// The stream decoded.
    #[field(label = "Stream decoded")]
    pub stream: Option<i64>,
    /// The emergency warning flag of the TMCC.
    #[field(label = "Emergency warning")]
    pub ews: bool,
    /// The transport stream is valid.
    #[field(label = "TS lock")]
    pub ts_lock: bool,
}

/// A row of the streams of the transponder. The row key is the stream number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbsStreamRow {
    /// The stream number in the TMCC.
    #[field(label = "Stream")]
    pub stream: i64,
    /// The transport stream id.
    #[field(label = "TS id")]
    pub ts_id: i64,
    /// The slots of the stream.
    #[field(label = "Slots")]
    pub slots: i64,
    /// The modulation and code. No value for a stream with mixed modulation.
    #[field(label = "Modulation")]
    pub modulation: Option<McChoice>,
    /// The slots of the stream differ in modulation.
    #[field(label = "Mixed modulation")]
    pub mixed: bool,
    /// The bit rate of the stream.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bitrate_bps: f64,
    /// It is the stream decoded.
    #[field(label = "Decoded")]
    pub decoded: bool,
}

/// The streams of the transponder, by stream number.
pub static ISDBS_STREAMS: Table<IsdbsStreamRow> = Table::new("streams", "Streams", 8);

// The TV stacks.

/// `shape` with the counts of the transport stream monitor, for a TV stack. Call it after the result sets of the
/// stack.
fn with_monitor(shape: StackShape) -> StackShape {
    sdrkit_proto_mpegts::monitor::describe(shape, "monitor")
}

/// `shape` with the stream adaptation and the IP layer of `sdrkit-proto-ipcast`, for a TV stack that takes BBFRAMEs.
fn with_bbframes(shape: StackShape) -> StackShape {
    let shape = ipcast::describe::<ipcast::BbFrameFields>(shape, "bbframe");
    ipcast::describe::<ipcast::IpFields>(shape, "ip")
}

/// `shape` with the transport stream layer, the settings and the `clear` command of a TV stack. The guide holds the
/// events of every multiplex that named them, so it is kept over a change of frequency; the services, the tracks,
/// the teletext pages and every count belong to the multiplex of the frequency before.
fn tv(
    shape: StackShape,
    series: &[SeriesDef],
    settings: &[&'static dyn DeclareSetting],
) -> StackShape {
    let mut s = layer::describe(shape, &TS_LAYER);
    for series in series {
        s = s.series(*series);
    }
    s.settings(settings)
        .command(&sdrkit_proto_mpegts::monitor::CLEAR)
        .kept(&["guide", "guide_ratings"])
}

// The DVB-S and DVB-S2 sender.

/// The result fields of the sender `transmit` of `dvbs`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbsTxFields {
    /// Whether a transmission is under way.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the centre of the carrier sent. No value while the stack does not transmit.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// The bit rate of the transport stream that the settings carry: the rate that the host feeds at most.
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
}

/// Starts sending.
pub const TX_START: Command = Command::new("start", "Start sending")
    .help(
        "Sends a DVB-S signal (EN 300 421) or a DVB-S2 signal (EN 302 307-1), as tx_standard says, with the tx_* \
         settings, until stop. The packets of append fill it; a slot with no packet carries a null packet. Transmit \
         must be on.",
    )
    .transmits(TxContent::File)
    .stopped_by("stop");
/// Stops sending.
pub const TX_STOP: Command = Command::new("stop", "Stop sending").help(
    "Ends the transmission after the group of eight packets on air (DVB-S) or the PLFRAME on air (DVB-S2) with a \
     fall of 5 ms. The packets that wait are dropped.",
);
/// Adds packets to the stream.
pub const TX_APPEND: CommandKind<TxAppendArgs> = CommandKind::new("append", "Add packets").help(
    "Adds transport packets to the queue of the stream sent. All or none: refused when the queue has no room, so the \
     host feeds at the useful bit rate.",
);

/// The declarations of `dvbc2` added in 1.3: the width of a data slice, the group and bundling of a PLP, and the data slice
/// out of the receive window.
const ADDED_1_3: &[&str] = &[
    "shape.Dvbc2Fields.outside_slice",
    "shape.Dvbc2Fields.outside_hz",
    "shape.Dvbc2SliceRow.width_hz",
    "shape.Dvbc2PlpRow.group",
    "shape.Dvbc2PlpRow.bundled",
];

/// The settings of the DVB-S2 form of the sender of `dvbs`, added in 1.4.
const ADDED_1_4: &[&str] = &[
    "dvbs.setting.tx_standard",
    "dvbs.setting.tx_modcod",
    "dvbs.setting.tx_frame",
    "dvbs.setting.tx_pilots",
    "dvbs.setting.tx_rolloff",
    "dvbs.setting.tx_pl_scrambling",
];

/// The declarations of the sender of `dvbs`, added in 1.2.
const ADDED_1_2: &[&str] = &[
    "dvbs.fields.DvbsTxFields",
    "shape.DvbsTxFields",
    "shape.DvbsTxFields.transmitting",
    "shape.DvbsTxFields.tx_freq_hz",
    "shape.DvbsTxFields.tx_bitrate_bps",
    "shape.DvbsTxFields.tx_packets",
    "shape.DvbsTxFields.tx_nulls",
    "shape.DvbsTxFields.tx_queue",
    "shape.DvbsTxFields.tx_queue_s",
    "shape.DvbsTxFields.tx_pcr_restamped",
    "dvbs.setting.tx_symbol_rate",
    "dvbs.setting.tx_rate",
    "dvbs.setting.tx_inversion",
    "dvbs.command.start",
    "dvbs.command.stop",
    "dvbs.command.append",
];

// The TV satellite beacon decoder.

/// The result fields of `satbeacon`: the carrier, the subcarrier, the bit clock and the frame learner. A measurement of
/// the subcarrier has no value without a subcarrier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconFields {
    /// What the decoder does now.
    #[field(label = "State")]
    pub state: BeaconState,
    /// The signal time decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The work rate of the carrier loop.
    #[field(label = "Work rate", unit = "Hz")]
    pub work_rate: Hertz,
    /// The offset of the carrier from the centre.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The RF frequency of the carrier.
    #[field(label = "Carrier frequency", unit = "Hz")]
    pub rf: Option<Hertz>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: f64,
    /// The drift of the carrier.
    #[field(label = "Drift", unit = "Hz/min")]
    pub drift_hz_min: f64,
    /// The noise density.
    #[field(label = "Noise density", unit = "dBFS/Hz")]
    pub noise_density_dbfs: f64,
    /// The carrier loop is locked.
    #[field(label = "Carrier locked")]
    pub carrier_locked: bool,
    /// The rms of the phase modulation.
    #[field(label = "Phase modulation (rms)", unit = "degrees")]
    pub pm_rms_deg: f64,
    /// The subcarrier frequency.
    #[field(label = "Subcarrier", unit = "Hz")]
    pub subcarrier: Option<Hertz>,
    /// The humps of the subcarrier spectrum.
    #[field(label = "Subcarrier humps")]
    pub tones: Option<i64>,
    /// Their spacing.
    #[field(label = "Hump spacing", unit = "Hz")]
    pub tone_spacing: Option<Hertz>,
    /// The subcarrier over the noise of the phase modulation.
    #[field(label = "Subcarrier SNR", unit = "dB")]
    pub subcarrier_snr_db: Option<f64>,
    /// The bit rate. No value before the bit clock is found.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_bps: Option<f64>,
    /// The line code.
    #[field(label = "Coding")]
    pub coding: Option<Coding>,
    /// The bit clock is locked.
    #[field(label = "Bit clock locked")]
    pub clock_locked: bool,
    /// The frame length. No value before the frames are found.
    #[field(label = "Frame length", unit = "bits")]
    pub frame_bits: Option<i64>,
    /// The agreement of the frames with the template.
    #[field(label = "Frame agreement")]
    pub frame_agreement: f64,
    /// Bits that do not change from frame to frame.
    #[field(label = "Fixed bits")]
    pub fixed_bits: i64,
    /// The sync word.
    #[field(label = "Sync word")]
    pub sync_word: Option<SyncWord>,
    /// The byte of the frame counter.
    #[field(label = "Counter byte")]
    pub counter_byte: Option<i64>,
    /// The modulo of the frame counter.
    #[field(label = "Counter modulo")]
    pub counter_modulo: Option<i64>,
    /// Bits received.
    #[field(label = "Bits")]
    pub bits: i64,
    /// Frames good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames bad.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The template of a frame.
    #[field(label = "Template")]
    pub template: Arc<[u8]>,
    /// The mask of the fixed bits, one bit for each bit of the frame.
    #[field(label = "Fixed mask")]
    pub fixed_mask: Arc<[u8]>,
    /// The spacing of a second carrier above the beacon.
    #[field(label = "Second carrier spacing", unit = "Hz")]
    pub pilot_spacing: Option<Hertz>,
    /// The lines of the spectrum.
    #[field(label = "Lines")]
    pub lines: i64,
    /// The signal time when the first frames were found.
    #[field(label = "First frames found at", unit = "s")]
    pub first_frames: Option<Seconds>,
}

/// A row of the narrow lines of the spectrum, strongest first. The row key is the place.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconLineRow {
    /// The offset of the line from the centre.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// Its level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// Its height over the noise.
    #[field(label = "Over noise", unit = "dB")]
    pub over_noise_db: f64,
}

/// A row of the frames received. The row key is the frame number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconFrameRow {
    /// The frame number.
    #[field(label = "Frame")]
    pub frame: i64,
    /// The frame counter.
    #[field(label = "Counter")]
    pub counter: Option<i64>,
    /// The errors against the template.
    #[field(label = "Errors")]
    pub errors: i64,
    /// The frame is good.
    #[field(label = "Good")]
    pub good: bool,
    /// The bytes of the frame.
    #[field(label = "Bytes")]
    pub bytes: Arc<[u8]>,
}

/// A row of the decoded view. The row key is the name and the bytes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconDecodedRow<'a> {
    /// The name of the field.
    #[field(label = "Name")]
    pub name: &'a str,
    /// What the line is.
    #[field(label = "Kind")]
    pub kind: ViewKind,
    /// Its value, for people.
    #[field(label = "Value")]
    pub value: &'a str,
    /// Its bytes in the frame, for people.
    #[field(label = "Bytes")]
    pub bytes: &'a str,
    /// How sure the reading is.
    #[field(label = "Confidence")]
    pub confidence: Confidence,
    /// Why, for people.
    #[field(label = "Reason")]
    pub reason: &'a str,
}

/// The fields of a telemetry frame, when it is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconFrameEvent {
    /// The frame number.
    #[field(label = "Frame")]
    pub frame: i64,
    /// The frame counter.
    #[field(label = "Counter")]
    pub counter: Option<i64>,
    /// The errors against the template.
    #[field(label = "Errors")]
    pub errors: i64,
    /// The frame is good.
    #[field(label = "Good")]
    pub good: bool,
    /// The bytes of the frame.
    #[field(label = "Bytes")]
    pub bytes: Arc<[u8]>,
}

/// The fields of the carrier measurements of one second of signal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SatbeaconCarrierEvent {
    /// The signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The offset of the carrier from the centre.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: f64,
    /// The drift of the carrier.
    #[field(label = "Drift", unit = "Hz/min")]
    pub drift_hz_min: f64,
    /// The bit rate. No value before the bit clock is found.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_bps: Option<f64>,
    /// The frame length. No value before the frames are found.
    #[field(label = "Frame length", unit = "bits")]
    pub frame_bits: Option<i64>,
}

/// The narrow lines of the spectrum.
pub static SATBEACON_LINES: Table<SatbeaconLineRow> = Table::new("lines", "Lines", 6);
/// The frames received.
pub static SATBEACON_FRAMES: Table<SatbeaconFrameRow> = Table::new("frames", "Frames", 48);
/// The decoded view.
pub static SATBEACON_DECODED: Table<SatbeaconDecodedRow<'static>> =
    Table::new("decoded", "Decoded", 256);

/// A telemetry frame was found.
pub static SATBEACON_FRAME: EventKind<SatbeaconFrameEvent> =
    EventKind::new("beacon.frame", "Telemetry frame");
/// The carrier measurements, once a second of signal.
pub static SATBEACON_CARRIER: EventKind<SatbeaconCarrierEvent> =
    EventKind::new("beacon.carrier", "Beacon carrier");

/// The plots of `satbeacon`, in order.
const SATBEACON_SERIES: [SeriesDef; 3] = [
    SeriesDef {
        key: "rf_spectrum",
        label: "RF spectrum",
        x_unit: "bin",
        y_unit: "dB",
    },
    SeriesDef {
        key: "pm_spectrum",
        label: "Phase modulation spectrum",
        x_unit: "bin",
        y_unit: "dB",
    },
    SeriesDef {
        key: "column_stability",
        label: "Column stability",
        x_unit: "byte",
        y_unit: "",
    },
];

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The frames are numbered by the learner, which starts again on a change of frequency; the frames, the decoded
        // view and every measurement belong to the carrier of the frequency before, so nothing is kept.
        let mut satbeacon = StackShape::new("satbeacon")
            .fields::<SatbeaconFields>()
            .table(&SATBEACON_LINES)
            .table(&SATBEACON_FRAMES)
            .table(&SATBEACON_DECODED);
        for series in SATBEACON_SERIES {
            satbeacon = satbeacon.series(series);
        }
        let satbeacon = satbeacon
            .event(&SATBEACON_FRAME)
            .event(&SATBEACON_CARRIER)
            .settings(BeaconSetting::SETTINGS)
            .command(&crate::beacon::CLEAR);
        let dvbc = with_monitor(StackShape::new("dvbc").fields::<DvbcFields>());
        let dvbs = with_bbframes(with_monitor(with_dvbs2_receiver(StackShape::new("dvbs"))));
        let isdbs = with_monitor(
            StackShape::new("isdbs")
                .fields::<IsdbsFields>()
                .table(&ISDBS_STREAMS),
        );
        let dvbc2 = StackShape::new("dvbc2")
            .fields::<Dvbc2Fields<'static>>()
            .fields::<Dvbc2StreamFields>()
            .table(&DVBC2_SLICES)
            .table(&DVBC2_PLPS)
            .table(&DVBC2_NOTCHES);
        let dvbc2 = with_bbframes(with_monitor(dvbc2));
        Family::new("dvbcs", 1, 4)
            .shared_source("beacon", &["satbeacon"])
            .stack(satbeacon)
            .uses(sdrkit_proto_mpegts::publish::interface())
            .uses(ipcast::interface())
            .uses(dvbs2_interface())
            .stack(tv(dvbc, &crate::dvbc::SERIES, DvbcSetting::SETTINGS))
            .stack(tv(dvbc2, &[], Dvbc2Setting::SETTINGS))
            .stack(
                tv(dvbs, &[], DvbsSetting::SETTINGS)
                    .fields::<DvbsTxFields>()
                    .command(&TX_START)
                    .command(&TX_STOP)
                    .command(&TX_APPEND),
            )
            .stack(tv(isdbs, &crate::isdbs::SERIES, IsdbsSetting::SETTINGS))
            .added(
                1,
                1,
                &[
                    "dvbc.setting.decode_video",
                    "dvbc2.setting.decode_video",
                    "dvbs.setting.decode_video",
                    "isdbs.setting.decode_video",
                ],
            )
            .added(1, 2, ADDED_1_2)
            .added(1, 3, ADDED_1_3)
            .added(1, 4, ADDED_1_4)
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
        .unwrap_or_else(|| panic!("the dvbcs family does not describe the stack {id}"))
}
