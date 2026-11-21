//! The typed items that flow between the DAB components.

use crate::audio::dabplus::{AudioParams, DabPlusStats};
use crate::data::mot::MotObject;
use crate::data::pad::{Label, PadItem};
use crate::ensemble::Ensemble;
use crate::fic::Fib;
use crate::fig::FigRecord;
use crate::msc::LogicalFrame;
use crate::ofdm::{NullSpectrum, OfdmStats};
use crate::results::AudioCodec;
use sdrkit_core::media::Codec;
use sdrkit_graph::Item;
use std::sync::Arc;

/// The FIC soft bits of one frame (`dab.fic`): the first 9216 values of `soft`.
#[derive(Clone, Debug)]
pub struct FicSoft {
    /// Frame number.
    pub frame: u64,
    /// Start of the frame's null symbol (absolute sample at 2.048 Msps).
    pub start: i64,
    /// Demodulated through a fade.
    pub faded: bool,
    /// The frame's soft bits (FIC then MSC), shared with the CIF items.
    pub soft: Arc<[i8]>,
}
impl Item for FicSoft {
    const KIND: &'static str = "dab.fic";
}

/// One CIF's soft bits (`dab.cif`): 55 296 values of `soft` from `9216 + 55 296 × index`.
#[derive(Clone, Debug)]
pub struct CifSoft {
    /// Frame number.
    pub frame: u64,
    /// Frame start.
    pub start: i64,
    /// CIF 0 … 3 of the frame.
    pub index: u8,
    /// The frame's soft bits.
    pub soft: Arc<[i8]>,
}
impl CifSoft {
    /// The CIF's soft bits.
    pub fn bits(&self) -> &[i8] {
        let a = crate::tables::FIC_BITS + crate::tables::CIF_BITS * self.index as usize;
        &self.soft[a..a + crate::tables::CIF_BITS]
    }
}
impl Item for CifSoft {
    const KIND: &'static str = "dab.cif";
}

/// A FIB (`dab.fib`).
#[derive(Clone, Debug)]
pub struct FibItem {
    /// The FIB.
    pub fib: Fib,
    /// Frame start.
    pub start: i64,
}
impl Item for FibItem {
    const KIND: &'static str = "dab.fib";
}

/// A FIG or the end of a frame's FIC (`dab.fig`).
#[derive(Clone, Debug)]
pub enum FigEvent {
    /// One FIG.
    Fig {
        /// Frame.
        frame: u64,
        /// FIB position.
        pos: u8,
        /// The FIG.
        rec: FigRecord,
    },
    /// All FIBs of a frame were seen.
    FrameDone {
        /// Frame.
        frame: u64,
        /// Frame start.
        start: i64,
        /// Good FIBs in the frame.
        ok: u8,
        /// Bytes used per FIB (−1 for a bad CRC).
        bytes: [i16; 12],
        /// Good and bad FIBs since the reset.
        totals: (u64, u64),
    },
}
impl Item for FigEvent {
    const KIND: &'static str = "dab.fig";
}

/// FIC statistics.
#[derive(Clone, Debug, Default)]
pub struct FicStats {
    /// Good FIBs.
    pub fib_ok: u64,
    /// Bad FIBs.
    pub fib_bad: u64,
    /// Good FIBs in the last frame.
    pub fib_ok_last_frame: u8,
    /// FIC fill in percent.
    pub fic_fill_pct: f64,
    /// Bytes per FIB of the last frame.
    pub fib_bytes: [i16; 12],
    /// Per FIG type and extension: (type, extension, count, bytes).
    pub per_type: Vec<(u8, u8, u64, u64)>,
    /// Distinct FIG records kept.
    pub records: usize,
}

/// The ensemble after a frame's FIC (`dab.ensemble`).
#[derive(Clone, Debug)]
pub struct EnsembleItem {
    /// Frame.
    pub frame: u64,
    /// Frame start.
    pub start: i64,
    /// The CIF count of the frame's first CIF, when its FIC carried FIG 0/0.
    pub cif_count: Option<u16>,
    /// The database (shared; a new Arc when it changed).
    pub db: Arc<Ensemble>,
    /// It changed with this frame.
    pub changed: bool,
    /// The ensemble was replaced by another (EId change).
    pub new_ensemble: bool,
    /// FIC statistics.
    pub fic: Arc<FicStats>,
    /// The ensemble time (seconds since 1970, milliseconds).
    pub utc: (i64, u16),
}
impl Item for EnsembleItem {
    const KIND: &'static str = "dab.ensemble";
}

/// What the sub-channel decoder emits (`dab.frame`).
#[derive(Clone, Debug)]
pub enum MscItem {
    /// A sub-channel decoder (re)started: downstream clears its state for it.
    Start {
        /// Sub-channel.
        subch: u8,
        /// What it carries.
        content: crate::msc::Content,
        /// Bit rate.
        bitrate: u16,
        /// The selected (audio) sub-channel, rather than a data one.
        selected: bool,
    },
    /// A logical frame.
    Frame(LogicalFrame),
}
impl Item for MscItem {
    const KIND: &'static str = "dab.frame";
}

/// Status of the sub-channel decoder.
#[derive(Clone, Debug, Default)]
pub struct MscStatus {
    /// A service is requested.
    pub requested: bool,
    /// The SId of the requested service, once the ensemble names it.
    pub service: Option<u32>,
    /// The sub-channel being decoded.
    pub decoding: Option<u8>,
    /// The selected service is scrambled.
    pub scrambled: bool,
    /// Waiting for the sub-channel to appear in FIG 0/1.
    pub waiting: bool,
    /// Logical frames out (selected sub-channel).
    pub logical_frames: u64,
    /// Erased CIFs put in.
    pub erased_cifs: u64,
    /// The data sub-channel decoded.
    pub data_subchannel: Option<u8>,
}
impl Item for MscStatus {
    const KIND: &'static str = "dab.msc_status";
}

/// A loss in a compressed audio stream: this many output samples to conceal.
#[derive(Clone, Copy, Debug)]
pub struct AudioLoss {
    /// Output samples.
    pub samples: u32,
    /// Signal time.
    pub time: f64,
}

/// Status of an audio decoder.
#[derive(Clone, Debug, Default)]
pub struct AudioStatus {
    /// The codec, once known.
    pub codec: Option<AudioCodec>,
    /// Output rate.
    pub rate: u32,
    /// Output channels.
    pub channels: u8,
    /// SBR and PS signalled.
    pub sbr: bool,
    /// PS signalled.
    pub ps: bool,
    /// MPEG Surround configuration.
    pub surround: u8,
    /// Access units per superframe (DAB+).
    pub units_per_superframe: usize,
    /// Audio being produced.
    pub decoding: bool,
    /// DAB+ counters.
    pub dabplus: DabPlusStats,
    /// MPEG frames out (Layer II).
    pub mp2_frames: u64,
    /// MPEG frames that failed to decode.
    pub mp2_bad: u64,
    /// Frame bytes (Layer II).
    pub frame_bytes: usize,
    /// Bit rate (Layer II header).
    pub bitrate: u16,
    /// Time the first unit came out (signal seconds), 0 before.
    pub first_unit_s: f64,
}
impl Item for AudioStatus {
    const KIND: &'static str = "dab.audio_status";
}

/// PAD of one audio frame, or a reset of the PAD decoder (`dab.pad`).
#[derive(Clone, Debug)]
pub enum PadEvent {
    /// A new service (on this sub-channel): clear label, segments and data groups.
    Reset(u8),
    /// One frame's PAD.
    Pad(PadItem),
}
impl Item for PadEvent {
    const KIND: &'static str = "dab.pad";
}

/// The dynamic label (`dab.label`).
#[derive(Clone, Debug)]
pub struct LabelItem {
    /// The label.
    pub label: Label,
    /// Signal time.
    pub time: f64,
}
impl Item for LabelItem {
    const KIND: &'static str = "dab.label";
}

/// Where a data group came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupSource {
    /// X-PAD of the selected service, with its application type.
    Xpad(u8),
    /// Packet mode: sub-channel and packet address.
    Packet(u8, u16),
    /// A reset of the source's assembly state.
    Reset,
}

/// An MSC data group (`dab.datagroup`).
#[derive(Clone, Debug)]
pub struct DataGroupItem {
    /// Its source.
    pub source: GroupSource,
    /// The bytes.
    pub data: Arc<[u8]>,
    /// Signal time.
    pub time: f64,
}
impl Item for DataGroupItem {
    const KIND: &'static str = "dab.datagroup";
}

/// A completed MOT object (`mot.object`), with what it is.
#[derive(Clone, Debug)]
pub struct MotItem {
    /// Its source.
    pub source: GroupSource,
    /// The object.
    pub object: Arc<MotObject>,
    /// The codec of a slideshow picture.
    pub codec: Option<Codec>,
    /// The picture is the service's logo.
    pub logo: bool,
    /// Sequence number of new pictures of the selected service (0 for other objects).
    pub sequence: u64,
    /// Signal time.
    pub time: f64,
}
impl Item for MotItem {
    const KIND: &'static str = "dab.mot_object";
}

/// Null symbol spectra (`dab.null`).
#[derive(Clone, Debug)]
pub struct NullItem(pub NullSpectrum);
impl Item for NullItem {
    const KIND: &'static str = "dab.null";
}

/// Demodulator status (`dab.ofdm_status`).
#[derive(Clone, Debug)]
pub struct OfdmStatusItem(pub OfdmStats);
impl Item for OfdmStatusItem {
    const KIND: &'static str = "dab.ofdm_status";
}

/// Front-end status (`dab.front_status`).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrontStatus {
    /// Samples zeroed because they were not finite.
    pub bad_samples: u64,
    /// Impulses blanked.
    pub impulses_blanked: u64,
    /// Input level in dBFS (rms).
    pub level_dbfs: f32,
    /// The centring shift applied, Hz.
    pub centring_hz: f64,
    /// Input rate.
    pub input_rate: f64,
}
impl Item for FrontStatus {
    const KIND: &'static str = "dab.front_status";
}

/// Re-exported for the report.
pub type Params = AudioParams;
