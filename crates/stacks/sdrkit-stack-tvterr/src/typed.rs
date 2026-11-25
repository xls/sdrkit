//! The interface of the terrestrial television family: the shapes of the result sets, the table rows and the events,
//! the tables, the event types and what each stack declares.
//!
//! The stacks with a transport stream (`atsc`, `dtmb`) show the layer of `sdrkit-proto-mpegts` through its shared
//! interface; `atsc3` shows the components of `sdrkit-proto-ipcast`.

use crate::atsc3::components::ComponentState;
use crate::atsc3::delivery::ComponentKind;
use crate::atsc3::frame::NotDecoded;
use crate::atsc3::results::{Delivery, ServiceAction, ServiceCategory, SystemBandwidth};
use crate::auto::detect::{Standard, Verdict};
use crate::auto::AutoSetting;
use crate::dtmb::pn::Header;
use crate::dtmb::profile::{Carriers, CodeRate as DtmbCodeRate, Interleaver, Mapping};
use crate::dtmb::DtmbStackSetting;
use crate::isdbt::results::{
    CodeRate as IsdbtCodeRate, LayerName as IsdbtLayerName, Modulation as IsdbtModulation,
};
use crate::isdbt::{Guard as IsdbtGuard, IsdbtStackSetting};
use crate::isdbtsb::IsdbtsbSetting;
use crate::status::{Quality, StepColour};
use sdrkit_core::command::Command;
use sdrkit_core::interface::{EventKind, Family, Fields, Hertz, Seconds, StackShape, Table};
use sdrkit_core::publish::keys::{self, Family as SourceFamily};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_graph::components::DcRemovalFields;
use sdrkit_graph::DecoderFields;
use sdrkit_proto_ipcast::components as ipcast;
use sdrkit_proto_mpegts::layer::{self, TsLayer};
use sdrkit_proto_mpegts::publish::{SERVICE_SELECT_KIND, TRACK_SELECT_KIND};
use std::sync::OnceLock;

// ISDB-T and ISDB-TSB.

/// The result fields of `isdbt` and `isdbtsb`: the detection, the transmission parameters, the signal and the TMCC.
/// A parameter of the mode has no value before the mode is found. A value of the TMCC has no value while no TMCC is
/// decoded.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbtFields {
    /// Detection level: 0 nothing, 1 a periodic cyclic prefix, 2 TMCC decoded, 3 transport stream flowing.
    #[field(label = "Detection level")]
    pub detection: i64,
    /// The channel width in use.
    #[field(label = "Channel width", unit = "Hz")]
    pub width: Hertz,
    /// The width setting is automatic.
    #[field(label = "Width found automatically")]
    pub width_auto: bool,
    /// The IFFT sampling rate of the width.
    #[field(label = "IFFT rate", unit = "Hz")]
    pub ifft_rate: Hertz,
    /// Mode 1, 2 or 3.
    #[field(label = "Mode")]
    pub mode: Option<i64>,
    /// The guard interval.
    #[field(label = "Guard interval")]
    pub guard: Option<IsdbtGuard>,
    /// The FFT size.
    #[field(label = "FFT size")]
    pub fft_size: Option<i64>,
    /// The carriers in the band.
    #[field(label = "Carriers")]
    pub carriers: Option<i64>,
    /// The carrier spacing.
    #[field(label = "Carrier spacing", unit = "Hz")]
    pub carrier_spacing: Option<Hertz>,
    /// The frame length (204 symbols).
    #[field(label = "Frame length", unit = "s")]
    pub frame_length: Option<Seconds>,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the sample clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The SNR of the equalised cells.
    #[field(shared = keys::SNR_DB, reading = 2)]
    pub snr_db: Option<f64>,
    /// Reed-Solomon words clean or corrected.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Reed-Solomon words that failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The integer carrier shift that the TMCC search found.
    #[field(label = "Integer carrier shift")]
    pub carrier_shift: Option<i64>,
    /// The index of the symbol in the frame. No value while no TMCC is decoded.
    #[field(label = "Symbol in the frame")]
    pub frame_symbol: Option<i64>,
    /// The time since the last good TMCC.
    #[field(label = "Since the last TMCC", unit = "s")]
    pub since_tmcc: Option<Seconds>,
    /// I and Q are swapped.
    #[field(label = "I and Q swapped")]
    pub mirrored: bool,
    /// The system identification of the TMCC (0 ISDB-T, 1 ISDB-TSB).
    #[field(label = "System identification")]
    pub system_id: Option<i64>,
    /// The parameter switching countdown of the TMCC (15: no switch announced).
    #[field(label = "Parameter switching countdown")]
    pub switch_countdown: Option<i64>,
    /// The emergency alarm start flag of the TMCC.
    #[field(label = "Emergency alarm flag")]
    pub emergency_alarm: Option<bool>,
    /// The partial reception flag of the TMCC.
    #[field(label = "Partial reception (one-seg)")]
    pub partial_reception: Option<bool>,
    /// The share of the Reed-Solomon words that are clean or corrected.
    #[field(label = "Packets ok", unit = "%")]
    pub packets_ok_pct: f64,
    /// The net bit rate of every layer together.
    #[field(label = "Net bit rate", unit = "bit/s")]
    pub net_rate_bit_s: f64,
    /// The signal quality score.
    #[field(label = "Signal quality", unit = "%", reading = 1)]
    pub quality_pct: f64,
    /// The signal quality class.
    #[field(label = "Signal quality")]
    pub quality: Quality,
    /// The mode step: the mode and the guard are found.
    #[field(label = "Mode step")]
    pub step_mode: StepColour,
    /// The TMCC step.
    #[field(label = "TMCC step")]
    pub step_tmcc: StepColour,
    /// The layers step: a layer is synchronised.
    #[field(label = "Layers step")]
    pub step_layers: StepColour,
    /// The transport stream step.
    #[field(label = "TS step")]
    pub step_ts: StepColour,
}

/// The result fields that a narrow-band receiver (`isdbtsb`) adds.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbtNarrowFields<'a> {
    /// The segment count that the receiver uses (1 or 3).
    #[field(label = "Segments in use")]
    pub band_segments: i64,
    /// What the receiver decodes and which of its values no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the layer table of `isdbt` and `isdbtsb`, one for each layer in use. The row key is `A`, `B` or `C`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbtLayerRow {
    /// The layer.
    #[field(label = "Layer")]
    pub layer: IsdbtLayerName,
    /// The layer is the partial-reception segment (one-seg).
    #[field(label = "Partial reception")]
    pub partial: bool,
    /// The segments of the layer.
    #[field(label = "Segments")]
    pub segments: i64,
    /// The carrier modulation. No value for a code that the standard reserves.
    #[field(label = "Modulation")]
    pub modulation: Option<IsdbtModulation>,
    /// The inner code rate. No value for a code that the standard reserves.
    #[field(label = "Code rate")]
    pub code_rate: Option<IsdbtCodeRate>,
    /// The time interleaving length I.
    #[field(label = "Interleaving")]
    pub interleave_length: i64,
    /// The net bit rate of the layer.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub rate_bit_s: f64,
    /// The packets delivered.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Reed-Solomon words clean.
    #[field(label = "RS clean")]
    pub rs_clean: i64,
    /// Reed-Solomon words corrected.
    #[field(label = "RS corrected")]
    pub rs_corrected: i64,
    /// Reed-Solomon words that failed.
    #[field(label = "RS failed")]
    pub rs_failed: i64,
    /// Reed-Solomon has synchronised the layer.
    #[field(label = "RS locked")]
    pub rs_locked: bool,
    /// The Viterbi margin (1: no noise).
    #[field(label = "Viterbi margin")]
    pub viterbi_margin: f64,
}

/// The layers in use.
pub static ISDBT_LAYERS: Table<IsdbtLayerRow> = Table::new("layers", "Layers", 3);

/// The fields of the event `isdbt.tmcc`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IsdbtTmccEvent<'a> {
    /// The system identification (0 ISDB-T, 1 ISDB-TSB).
    #[field(label = "System identification")]
    pub system_id: i64,
    /// The parameter switching countdown (15: no switch announced).
    #[field(label = "Parameter switching countdown")]
    pub switch_countdown: i64,
    /// The emergency alarm start flag.
    #[field(label = "Emergency alarm flag")]
    pub emergency_alarm: bool,
    /// The partial reception flag.
    #[field(label = "Partial reception (one-seg)")]
    pub partial_reception: bool,
    /// The layers in use ("A 1 QPSK 2/3, B 12 64QAM 3/4").
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// An emergency when the emergency alarm start flag is set.
    #[field(shared = keys::URGENCY)]
    pub urgency: keys::Urgency,
    /// The family of the source: a broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: SourceFamily,
}

/// The TMCC changed.
pub static ISDBT_TMCC: EventKind<IsdbtTmccEvent<'static>> = EventKind::new("isdbt.tmcc", "TMCC");

/// The plots of `isdbt` and `isdbtsb`.
pub const ISDBT_SERIES: [SeriesDef; 5] = [
    SeriesDef {
        key: "constellation",
        label: "Equalised data cells",
        x_unit: "I",
        y_unit: "Q",
    },
    SeriesDef {
        key: "pilots",
        label: "Scattered pilots",
        x_unit: "I",
        y_unit: "Q",
    },
    SeriesDef {
        key: "channel_db",
        label: "Channel magnitude",
        x_unit: "carrier",
        y_unit: "dB",
    },
    SeriesDef {
        key: "channel_phase",
        label: "Channel phase",
        x_unit: "carrier",
        y_unit: "rad",
    },
    SeriesDef {
        key: "impulse",
        label: "Impulse response",
        x_unit: "samples",
        y_unit: "dB",
    },
];

/// `shape` with what the report of an ISDB-T receiver and the DC removal before it add after the result sets: the
/// layer table, the plots and the TMCC event, then the transport stream layer.
fn isdbt(shape: StackShape) -> StackShape {
    let shape = shape
        .component("dc", |c| c.fields::<DcRemovalFields>())
        .table(&ISDBT_LAYERS);
    let shape = ISDBT_SERIES.into_iter().fold(shape, StackShape::series);
    tv(shape.event(&ISDBT_TMCC))
}

// ATSC 1.0.

/// The result fields of `atsc`: the detection, the locks, the signal and the counts of the decoder. A measurement of
/// a stage has no value before the stage locks.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtscFields {
    /// Detection level: 0 nothing, 1 pilot, 2 segment sync, 3 field sync, 4 transport stream flowing.
    #[field(label = "Detection level")]
    pub detection: i64,
    /// The pilot loop is locked.
    #[field(label = "Pilot locked")]
    pub pilot_locked: bool,
    /// The segment sync and the symbol clock are locked.
    #[field(label = "Segment sync")]
    pub segment_sync: bool,
    /// Fields are framed.
    #[field(label = "Field sync")]
    pub field_sync: bool,
    /// The equaliser is trained (SNR on the known symbols at least 9 dB).
    #[field(label = "Equaliser trained")]
    pub equaliser_trained: bool,
    /// The transport stream flows.
    #[field(label = "Transport stream flowing")]
    pub ts_flowing: bool,
    /// The carrier offset. No value before the pilot locks.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock. No value before the segment sync.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The SNR on the known symbols. No value before the field sync.
    #[field(shared = keys::SNR_DB, reading = 2)]
    pub snr_db: Option<f64>,
    /// Reed-Solomon words good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Reed-Solomon words that failed.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The quality of the segment sync correlation, 0 to 1.
    #[field(label = "Segment sync correlation")]
    pub segment_correlation: f64,
    /// The field parity, 1 or 2. No value without the field sync.
    #[field(label = "Field")]
    pub field_parity: Option<i64>,
    /// The SNR of the data symbols against the decoded trellis path. No value before a field decodes.
    #[field(label = "Data symbols SNR", unit = "dB")]
    pub data_snr_db: Option<f64>,
    /// The pilot level over the data level. No value before the pilot locks.
    #[field(label = "Pilot over data", unit = "dB")]
    pub pilot_db: Option<f64>,
    /// Fields decoded.
    #[field(label = "Fields decoded")]
    pub fields: i64,
    /// Reed-Solomon words clean.
    #[field(label = "RS clean")]
    pub rs_clean: i64,
    /// Reed-Solomon words corrected.
    #[field(label = "RS corrected")]
    pub rs_corrected: i64,
    /// Reed-Solomon words that failed.
    #[field(label = "RS failed")]
    pub rs_failed: i64,
    /// Packets delivered.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Field syncs expected but not found.
    #[field(label = "Field-sync misses")]
    pub field_sync_misses: i64,
    /// Times the field sync was lost.
    #[field(label = "Lock losses")]
    pub lock_losses: i64,
    /// The signal quality score.
    #[field(label = "Signal quality", unit = "%", reading = 1)]
    pub quality_pct: f64,
    /// The signal quality class.
    #[field(label = "Signal quality")]
    pub quality: Quality,
    /// The pilot step.
    #[field(label = "Pilot step")]
    pub step_pilot: StepColour,
    /// The segment sync step.
    #[field(label = "Segment step")]
    pub step_segment: StepColour,
    /// The field sync step.
    #[field(label = "Field step")]
    pub step_field: StepColour,
    /// The equaliser step.
    #[field(label = "Equaliser step")]
    pub step_equaliser: StepColour,
    /// The trellis step.
    #[field(label = "Trellis step")]
    pub step_trellis: StepColour,
    /// The Reed-Solomon step.
    #[field(label = "RS step")]
    pub step_rs: StepColour,
    /// The transport stream step.
    #[field(label = "TS step")]
    pub step_ts: StepColour,
}

// ATSC 3.0.

/// The result fields of `atsc3`: the bootstrap, the frames, the service list and the media. A value of the bootstrap
/// has no value before a bootstrap is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Atsc3Fields {
    /// A bootstrap was seen recently.
    #[field(label = "Bootstrap seen")]
    pub bootstrap_seen: bool,
    /// Bootstraps found.
    #[field(label = "Bootstraps found")]
    pub bootstraps: i64,
    /// Frames taken.
    #[field(label = "Frames")]
    pub frames_taken: i64,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The carrier offset of the last bootstrap.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The minor version of the bootstrap.
    #[field(label = "Minor version")]
    pub minor_version: Option<i64>,
    /// The system bandwidth that the bootstrap signals.
    #[field(label = "Channel")]
    pub system_bandwidth: Option<SystemBandwidth>,
    /// The preamble structure, 0 to 159.
    #[field(label = "Preamble structure")]
    pub preamble_structure: Option<i64>,
    /// The minimum time to the next frame.
    #[field(label = "Minimum time to the next frame", unit = "s")]
    pub min_time_next: Option<Seconds>,
    /// The sample rate after the bootstrap.
    #[field(label = "Post-bootstrap sample rate", unit = "Hz")]
    pub post_rate: Option<Hertz>,
    /// The emergency alert wake-up.
    #[field(label = "Emergency alert wake-up")]
    pub ea_wake_up: Option<bool>,
    /// The time between the last two bootstraps.
    #[field(label = "Frame length", unit = "s")]
    pub frame_length: Option<Seconds>,
    /// Service list tables read.
    #[field(label = "Service list tables")]
    pub lls_tables: i64,
    /// Signed multi-tables or signed signalling packages arrived in the last 10 s.
    #[field(label = "Signalling signed")]
    pub signed_signalling: bool,
    /// TAI minus UTC from the system time table.
    #[field(label = "Leap seconds", unit = "s")]
    pub leap_seconds: Option<i64>,
    /// The service chosen with `service.select`. No value for the automatic choice.
    #[field(label = "Service chosen")]
    pub service: Option<i64>,
    /// The service received: the chosen one, or the first visible linear TV service delivered by ROUTE.
    #[field(label = "Service received", reading = 1)]
    pub service_received: Option<i64>,
    /// ROUTE objects taken.
    #[field(label = "ROUTE objects")]
    pub route_objects: i64,
    /// Access units given out as media packets, video and audio.
    #[field(label = "Media samples")]
    pub media_samples: i64,
    /// The bootstrap step.
    #[field(label = "Bootstrap step")]
    pub step_bootstrap: StepColour,
    /// The L1 step.
    #[field(label = "L1 step")]
    pub step_l1: StepColour,
    /// The service step: green while a component of the received service is given.
    #[field(label = "Service step")]
    pub step_service: StepColour,
    /// Video access units given out as media packets.
    #[field(label = "Video packets")]
    pub video_packets: i64,
    /// Audio access units given out as media packets.
    #[field(label = "Sound packets")]
    pub audio_packets: i64,
}

/// The result fields of `atsc3` about the frames: what the L1 signalling says, the PLP that is decoded and its FEC blocks.
/// A value of the signalling has no value before L1-Basic is read, a value of the PLP none before a FEC block is decoded.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Atsc3FrameFields {
    /// Frames that were not decoded.
    #[field(label = "Frames not decoded")]
    pub frames_failed: i64,
    /// Why the last frame was not decoded. No value after a decoded frame.
    #[field(label = "Not decoded")]
    pub not_decoded: Option<NotDecoded>,
    /// The FFT size of the Preamble and the first subframe, points.
    #[field(label = "FFT size")]
    pub fft_size: Option<i64>,
    /// The guard interval of the Preamble and the first subframe.
    #[field(label = "Guard interval", unit = "samples")]
    pub guard_samples: Option<i64>,
    /// The L1-Basic mode that the bootstrap signals, 1 to 5.
    #[field(label = "L1-Basic mode")]
    pub l1_basic_mode: Option<i64>,
    /// The L1-Detail mode, 1 to 7.
    #[field(label = "L1-Detail mode")]
    pub l1_detail_mode: Option<i64>,
    /// The Preamble symbols of the frame.
    #[field(label = "Preamble symbols")]
    pub preamble_symbols: Option<i64>,
    /// The subframes of the frame. The first one is decoded.
    #[field(label = "Subframes")]
    pub subframes: Option<i64>,
    /// The data and subframe boundary symbols of the first subframe.
    #[field(label = "Symbols of the first subframe")]
    pub subframe_symbols: Option<i64>,
    /// The spacing of the carriers that bear scattered pilots in the first subframe.
    #[field(label = "Scattered pilot carrier spacing")]
    pub pilot_dx: Option<i64>,
    /// The symbols of one scattered pilot sequence in the first subframe.
    #[field(label = "Scattered pilot symbol period")]
    pub pilot_dy: Option<i64>,
    /// The PLPs of the frame: the different PLP ids in all its subframes.
    #[field(label = "PLPs")]
    pub plps: Option<i64>,
    /// The id of the PLP that is decoded.
    #[field(label = "PLP decoded")]
    pub plp_id: Option<i64>,
    /// The constellation points of the PLP that is decoded: 4, 16, 64 or 256.
    #[field(label = "PLP constellation points")]
    pub plp_points: Option<i64>,
    /// The code rate of the PLP that is decoded, in fifteenths.
    #[field(label = "PLP code rate", unit = "/15")]
    pub plp_code_rate: Option<i64>,
    /// The LDPC code word of the PLP that is decoded: 16200 or 64800 bits.
    #[field(label = "PLP LDPC code word", unit = "bit")]
    pub plp_ldpc_bits: Option<i64>,
    /// FEC blocks of the PLP that ended in the last frame.
    #[field(label = "FEC blocks in the last frame")]
    pub fec_blocks: i64,
    /// Those of them that decoded.
    #[field(label = "FEC blocks decoded in the last frame")]
    pub fec_blocks_good: i64,
    /// Baseband packets given out.
    #[field(label = "Baseband packets")]
    pub baseband_packets: i64,
    /// FEC blocks whose baseband packet was not good.
    #[field(label = "Baseband packets bad")]
    pub baseband_packets_bad: i64,
    /// Bits that the outer BCH code corrected after the LDPC decoder.
    #[field(label = "BCH corrections")]
    pub outer_corrected: i64,
    /// The mean LDPC iterations of the FEC blocks of the last frame.
    #[field(label = "LDPC iterations")]
    pub ldpc_iterations: Option<f64>,
    /// The MER of the cells of the decoded PLP against the nearest constellation points.
    #[field(label = "MER (PLP cells)", unit = "dB", reading = 2)]
    pub mer_db: Option<f64>,
    /// The signal to noise ratio that the pilots of the first subframe give.
    #[field(label = "Pilot SNR", unit = "dB")]
    pub pilot_snr_db: Option<f64>,
    /// The PLP step: green when more than 99 % of the baseband packets are good, amber above 50 %, red below.
    #[field(label = "PLP step")]
    pub step_plp: StepColour,
}

/// A row of the PLPs of the last frame: one row for each PLP of each subframe. The row key is the subframe number, a
/// point and the PLP id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Atsc3PlpRow {
    /// The number of the subframe in the frame, from 0.
    #[field(label = "Subframe")]
    pub subframe: i64,
    /// The PLP id.
    #[field(label = "Id")]
    pub id: i64,
    /// The layer: 0 core, above 0 enhanced.
    #[field(label = "Layer")]
    pub layer: i64,
    /// The constellation points, also of a coding that the stack does not decode. No value for a reserved code.
    #[field(label = "Constellation points")]
    pub points: Option<i64>,
    /// The code rate in fifteenths.
    #[field(label = "Code rate", unit = "/15")]
    pub code_rate: Option<i64>,
    /// The LDPC code word, bits.
    #[field(label = "LDPC code word", unit = "bit")]
    pub ldpc_bits: Option<i64>,
    /// The data cells of the PLP in the subframe.
    #[field(label = "Cells")]
    pub cells: i64,
    /// The time interleaver: 0 none, 1 convolutional, 2 hybrid.
    #[field(label = "Time interleaver mode")]
    pub time_interleaver: i64,
    /// The PLP is the one that the stack decodes.
    #[field(label = "Decoded")]
    pub decoded: bool,
    /// The channel bonding format (table 9.25): 0 plain, 1 SNR averaging, 2 and 3 reserved. No value when the PLP is
    /// not bonded with another RF channel.
    #[field(label = "Channel bonding")]
    pub bonding: Option<i64>,
    /// The BSID of the RF channel that the PLP is bonded with, as L1-Detail lists it.
    #[field(label = "Bonded channel BSID")]
    pub bonded_bsid: Option<i64>,
}

/// A row of the services of the service list. The row key is the service id; `service.select` takes it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Atsc3ServiceRow<'a> {
    /// The major channel number.
    #[field(label = "Major channel")]
    pub major: i64,
    /// The minor channel number.
    #[field(label = "Minor channel")]
    pub minor: i64,
    /// The short name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The service is hidden.
    #[field(label = "Hidden")]
    pub hidden: bool,
    /// The service id.
    #[field(label = "Id")]
    pub id: i64,
    /// The category of the service.
    #[field(label = "Category")]
    pub category: ServiceCategory,
    /// How the signalling is delivered.
    #[field(label = "Delivery")]
    pub delivery: Delivery,
    /// What the receiver does with the service.
    #[field(label = "Action")]
    pub action: ServiceAction,
    /// Service layer signalling objects received on TSI 0 of the ROUTE session of the service.
    #[field(label = "Signalling objects")]
    pub sls_objects: i64,
    /// Signalling documents (USBD, S-TSID, MPD and others) read from those objects.
    #[field(label = "Signalling documents")]
    pub sls_documents: i64,
    /// Those objects that were signed packages. The signature is not verified.
    #[field(label = "Signed signalling packages")]
    pub sls_signed_packages: i64,
    /// The service list says that a component needed to present the service is protected, for example encrypted.
    #[field(label = "Protected")]
    pub protected: bool,
}

/// A row of the media components seen, in the order they appeared. The row key is the destination address and port of
/// the ROUTE session and the transport session id of the component: `239.255.10.2:5002/1`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Atsc3ComponentRow<'a> {
    /// The transport session id of the component in its ROUTE session.
    #[field(label = "ROUTE session")]
    pub tsi: i64,
    /// The sample entry type of the component: the four-character code of its codec.
    #[field(label = "FourCC")]
    pub fourcc: &'a str,
    /// The destination address and port of the ROUTE session.
    #[field(label = "Destination")]
    pub destination: &'a str,
    /// The service whose session description names the component. No value when none that was read names it.
    #[field(label = "Service")]
    pub service: Option<i64>,
    /// What the component carries.
    #[field(label = "Kind")]
    pub kind: ComponentKind,
    /// What the stack does with the component.
    #[field(label = "State")]
    pub state: ComponentState,
}

/// The services of the service list.
pub static ATSC3_SERVICES: Table<Atsc3ServiceRow<'static>> =
    Table::new("services", "Services", 64).label(&Atsc3ServiceRow::NAME);
/// The media components seen.
pub static ATSC3_COMPONENTS: Table<Atsc3ComponentRow<'static>> =
    Table::new("components", "Media components", 64);
/// The PLPs of every subframe of the last frame.
pub static ATSC3_PLPS: Table<Atsc3PlpRow> = Table::new("plps", "PLPs", 64);

// DTMB.

/// The result fields of `dtmb`: the frame headers, the system information, the decoder and the signal. A value of
/// the header has no value while no header is tracked, and a value of the profile none before the system information
/// is decoded.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DtmbFields<'a> {
    /// Detection level: 0 nothing, 1 frame headers tracked, 2 system information decoded, 3 transport stream locked.
    #[field(label = "Detection level")]
    pub detection: i64,
    /// The header grid holds.
    #[field(label = "Frames tracked")]
    pub tracking: bool,
    /// The frame header.
    #[field(label = "Frame header")]
    pub header: Option<Header>,
    /// The phase of the header changes from frame to frame.
    #[field(label = "PN phase rotates")]
    pub pn_phase_rotates: Option<bool>,
    /// The channel width.
    #[field(label = "Channel width", unit = "Hz")]
    pub width: Hertz,
    /// The length of a signal frame (header and body).
    #[field(label = "Signal frame", unit = "s")]
    pub frame_length: Option<Seconds>,
    /// C/N from the fit of the header.
    #[field(label = "C/N (PN header)", unit = "dB")]
    pub cn_db: Option<f64>,
    /// The time between the first and the last significant path.
    #[field(label = "Echo span", unit = "s")]
    pub echo_span: Option<Seconds>,
    /// Frames with a usable header.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Frames erased: their header window was exact silence (a zero-filled gap of the source).
    #[field(label = "Frames erased")]
    pub frames_erased: i64,
    /// The share of recent frames without a usable header.
    #[field(label = "Recent frames without a usable header", unit = "%")]
    pub no_header_pct: f64,
    /// The input rate is enough for the width.
    #[field(label = "Input rate ok")]
    pub rate_ok: bool,
    /// The header sync step.
    #[field(label = "PN sync step")]
    pub step_pn_sync: StepColour,
    /// The system information index, 3 to 24.
    #[field(label = "System information index")]
    pub si_index: Option<i64>,
    /// The normalised score of the leading system information word (1 is perfect). No value while no header is
    /// tracked.
    #[field(label = "System information correlation")]
    pub si_corr: Option<f64>,
    /// The mapping.
    #[field(label = "Modulation")]
    pub mapping: Option<Mapping>,
    /// The LDPC rate.
    #[field(label = "LDPC code rate")]
    pub code_rate: Option<DtmbCodeRate>,
    /// The time interleaver.
    #[field(label = "Time interleaver")]
    pub interleaver: Option<Interleaver>,
    /// The carrier form.
    #[field(label = "Carriers")]
    pub carriers: Option<Carriers>,
    /// The MER of the equalised data symbols.
    #[field(label = "MER (data symbols)", unit = "dB", reading = 1)]
    pub mer_db: Option<f64>,
    /// LDPC code words that decoded.
    #[field(label = "LDPC code words good")]
    pub ldpc_ok: i64,
    /// LDPC code words that did not.
    #[field(label = "LDPC code words bad")]
    pub ldpc_bad: i64,
    /// The mean LDPC iterations of recent code words.
    #[field(label = "LDPC iterations")]
    pub ldpc_iterations: f64,
    /// BCH blocks with one bit corrected.
    #[field(label = "BCH corrections")]
    pub bch_corrected: i64,
    /// Code words dropped for lack of time.
    #[field(label = "Code words dropped")]
    pub codewords_dropped: i64,
    /// Code words not tried during a long run of failures.
    #[field(label = "Code words skipped")]
    pub codewords_skipped: i64,
    /// Transport packets delivered.
    #[field(label = "Transport packets")]
    pub packets: i64,
    /// A good packet came out within the last 0.7 s.
    #[field(label = "Transport stream lock")]
    pub ts_lock: bool,
    /// The net bit rate of the signalled profile. No value before the profile and the header are known.
    #[field(label = "Payload rate", unit = "bit/s")]
    pub payload_bps: Option<f64>,
    /// What the stack decodes and which of its tables no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The carrier offset. No value while no header is tracked.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock. No value while no header is tracked.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR: the MER once the profile is known and it lies between 8 dB and the C/N, else the C/N.
    #[field(shared = keys::SNR_DB, reading = 2)]
    pub snr_db: Option<f64>,
}

/// The channel impulse response of `dtmb`.
pub const DTMB_CIR: SeriesDef = SeriesDef {
    key: "cir",
    label: "Channel impulse response",
    x_unit: "symbol",
    y_unit: "dB",
};

// TV (auto).

/// The result fields of `tv-auto`: the line shown, the capture and the last detection. A value of the detection has no
/// value before the first detection.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AutoFields<'a> {
    /// The line shown ("Listening to the channel...").
    #[field(label = "TV (auto)")]
    pub line: &'a str,
    /// The progress of the capture.
    #[field(label = "Capture", unit = "%")]
    pub progress_pct: f64,
    /// Captures looked at.
    #[field(label = "Tries")]
    pub tries: i64,
    /// The id of the stack requested, when one is found.
    #[field(label = "Stack")]
    pub switch_to: Option<&'a str>,
    /// The standard of the last detection.
    #[field(label = "Standard", reading = 1)]
    pub standard: Option<Standard>,
    /// The headline of the last detection.
    #[field(label = "Found")]
    pub headline: Option<&'a str>,
    /// The verdict of the last detection.
    #[field(label = "Verdict", reading = 2)]
    pub verdict: Option<Verdict>,
    /// The confidence of the last detection.
    #[field(label = "Confidence", unit = "%", reading = 3)]
    pub confidence_pct: Option<f64>,
    /// The channel width found.
    #[field(label = "Channel width", unit = "Hz", reading = 4)]
    pub width: Option<Hertz>,
    /// The centre of the channel from the centre of the capture.
    #[field(label = "Channel centre offset", unit = "Hz")]
    pub centre_offset: Option<Hertz>,
    /// The detector stopped: the command `try_again` starts it again.
    #[field(label = "Try again offered")]
    pub try_again_offered: bool,
}

/// The fields of a standard found. The text is the headline.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AutoFoundEvent<'a> {
    /// The id of the stack that receives the standard found.
    #[field(label = "Stack")]
    pub stack: &'a str,
    /// The standard found.
    #[field(label = "Standard")]
    pub standard: Standard,
    /// The headline.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The channel width found.
    #[field(label = "Channel width", unit = "Hz")]
    pub width: Hertz,
    /// The detector is sure.
    #[field(label = "Sure")]
    pub sure: bool,
    /// The family of the source: a receiver.
    #[field(shared = keys::FAMILY)]
    pub family: SourceFamily,
}

/// A standard was found.
pub static AUTO_FOUND: EventKind<AutoFoundEvent<'static>> =
    EventKind::new("tv-auto.found", "TV standard found");

/// Starts the detection again after it stopped.
pub const TRY_AGAIN: Command =
    Command::new("try_again", "Clear").help("Forgets everything decoded so far.");

// The stacks.

/// `shape`, which lists the result sets of the stack, with the transport stream layer, the select commands and the
/// guide kept. The guide holds the events of every multiplex that named them, so it is kept over a change of
/// frequency; the services, the tracks and every count belong to the multiplex of the frequency before.
fn tv(shape: StackShape) -> StackShape {
    layer::describe(shape, &TsLayer::basic())
        .command(&SERVICE_SELECT_KIND)
        .command(&TRACK_SELECT_KIND)
        .kept(&["guide", "guide_ratings"])
}

/// What version 1.2 added: the frame fields of `atsc3` (L1 signalling, the decoded PLP and its FEC blocks) and its table
/// of PLPs.
const ADDED_1_2: &[&str] = &[
    "shape.Atsc3FrameFields",
    "shape.Atsc3FrameFields.frames_failed",
    "shape.Atsc3FrameFields.not_decoded",
    "shape.Atsc3FrameFields.fft_size",
    "shape.Atsc3FrameFields.guard_samples",
    "shape.Atsc3FrameFields.l1_basic_mode",
    "shape.Atsc3FrameFields.l1_detail_mode",
    "shape.Atsc3FrameFields.preamble_symbols",
    "shape.Atsc3FrameFields.subframes",
    "shape.Atsc3FrameFields.subframe_symbols",
    "shape.Atsc3FrameFields.pilot_dx",
    "shape.Atsc3FrameFields.pilot_dy",
    "shape.Atsc3FrameFields.plps",
    "shape.Atsc3FrameFields.plp_id",
    "shape.Atsc3FrameFields.plp_points",
    "shape.Atsc3FrameFields.plp_code_rate",
    "shape.Atsc3FrameFields.plp_ldpc_bits",
    "shape.Atsc3FrameFields.fec_blocks",
    "shape.Atsc3FrameFields.fec_blocks_good",
    "shape.Atsc3FrameFields.baseband_packets",
    "shape.Atsc3FrameFields.baseband_packets_bad",
    "shape.Atsc3FrameFields.outer_corrected",
    "shape.Atsc3FrameFields.ldpc_iterations",
    "shape.Atsc3FrameFields.mer_db",
    "shape.Atsc3FrameFields.pilot_snr_db",
    "shape.Atsc3FrameFields.step_plp",
    "atsc3.fields.Atsc3FrameFields",
    "shape.Atsc3PlpRow",
    "shape.Atsc3PlpRow.id",
    "shape.Atsc3PlpRow.layer",
    "shape.Atsc3PlpRow.points",
    "shape.Atsc3PlpRow.code_rate",
    "shape.Atsc3PlpRow.ldpc_bits",
    "shape.Atsc3PlpRow.cells",
    "shape.Atsc3PlpRow.time_interleaver",
    "shape.Atsc3PlpRow.decoded",
    "atsc3.table.plps",
];

/// What version 1.8 added: the sound and the video of `atsc3` as media packets, the packet counts, the protected mark of
/// a service and what the stack does with each component.
const ADDED_1_8: &[&str] = &[
    "shape.Atsc3Fields.video_packets",
    "shape.Atsc3Fields.audio_packets",
    "shape.Atsc3ServiceRow.protected",
    "shape.Atsc3ComponentRow.destination",
    "shape.Atsc3ComponentRow.service",
    "shape.Atsc3ComponentRow.kind",
    "shape.Atsc3ComponentRow.state",
];

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let atsc = tv(StackShape::new(crate::atsc::ID)
            .fields::<AtscFields>()
            .settings(crate::atsc::AtscSetting::SETTINGS));
        // The services of the service list belong to the multiplex of the frequency before: nothing is kept.
        let atsc3 = StackShape::new(crate::atsc3::ID)
            .fields::<Atsc3Fields>()
            .fields::<Atsc3FrameFields>()
            .table(&ATSC3_SERVICES)
            .table(&ATSC3_COMPONENTS)
            .table(&ATSC3_PLPS)
            .settings(crate::atsc3::Atsc3Setting::SETTINGS);
        let atsc3 = ipcast::describe::<ipcast::AlpFields>(atsc3, "alp");
        let atsc3 = ipcast::describe::<ipcast::IpFields>(atsc3, "ip");
        let atsc3 =
            ipcast::describe::<ipcast::RouteFields>(atsc3, "route").command(&SERVICE_SELECT_KIND);
        let dtmb = tv(StackShape::new(crate::dtmb::ID)
            .fields::<DtmbFields<'static>>()
            .series(DTMB_CIR)
            .settings(DtmbStackSetting::SETTINGS));
        // A detection belongs to the channel of the frequency before: nothing is kept.
        let auto = StackShape::new(crate::auto::ID)
            .fields::<AutoFields<'static>>()
            .fields::<DecoderFields>()
            .event(&AUTO_FOUND)
            .settings(AutoSetting::SETTINGS)
            .command(&TRY_AGAIN);
        let isdbt_full = isdbt(StackShape::new(crate::isdbt::ID).fields::<IsdbtFields>())
            .settings(IsdbtStackSetting::SETTINGS);
        let isdbtsb = isdbt(
            StackShape::new(crate::isdbtsb::ID)
                .fields::<IsdbtFields>()
                .fields::<IsdbtNarrowFields<'static>>(),
        )
        .settings(IsdbtsbSetting::SETTINGS);
        Family::new("tvterr", 1, 8)
            .added(1, 8, ADDED_1_8)
            .added(
                1,
                6,
                &[
                    "isdbt.content",
                    "isdbtsb.content",
                    "atsc.content",
                    "atsc3.content",
                    "dtmb.content",
                    "tv-auto.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Television,
                &["isdbt", "isdbtsb", "atsc", "atsc3", "dtmb"],
            )
            .content(sdrkit_core::ContentKind::Measurement, &["tv-auto"])
            .audio(
                sdrkit_core::AudioKind::Programme,
                &["isdbt", "isdbtsb", "atsc", "atsc3", "dtmb"],
            )
            .video(&["isdbt", "isdbtsb", "atsc", "atsc3", "dtmb"])
            .uses(sdrkit_proto_mpegts::publish::interface())
            .uses(ipcast::interface())
            .uses(sdrkit_graph::components::interface())
            .shared_source("auto", &[crate::auto::ID])
            .stack(isdbt_full)
            .stack(isdbtsb)
            .stack(atsc)
            .stack(atsc3)
            .stack(dtmb)
            .stack(auto)
            .added(
                1,
                5,
                &["shape.Atsc3PlpRow.bonding", "shape.Atsc3PlpRow.bonded_bsid"],
            )
            .added(1, 4, &["shape.Atsc3PlpRow.subframe"])
            .added(1, 3, &["atsc3.setting.plp"])
            .added(1, 2, ADDED_1_2)
            .added(
                1,
                1,
                &[
                    "isdbt.setting.decode_video",
                    "isdbtsb.setting.decode_video",
                    "atsc.setting.decode_video",
                    "dtmb.setting.decode_video",
                    "tv-auto.setting.decode_video",
                ],
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
        .unwrap_or_else(|| panic!("the tvterr family does not describe the stack {id}"))
}

/// An empty store of the results of the stack `id`, for its status component.
pub fn results(id: &str) -> sdrkit_core::publish::Published {
    sdrkit_core::publish::Published::from_decl(std::sync::Arc::new(shape(id).results_decl()))
}
