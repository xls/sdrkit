//! The interface of the weather satellite family: the shapes of the result sets, the table rows and the events, the
//! tables, the event types, the commands and what each stack declares.
//!
//! The DVB-S2 stacks `fy4_lrit`, `fy4a_hrit`, `goes_grb` and `sat_filecast`, and `elektro_l`, `hrpt` and `eodb` are
//! not described yet. `eodb` writes through the shapes of this module.

use crate::items::ProductKind;
use crate::modes::apt::AptSetting;
use crate::modes::aws::AwsSetting;
use crate::modes::dsb::DsbSetting;
use crate::modes::fy2::Fy2Setting;
use crate::modes::fy3::{AhrptSetting, MptSetting, Product};
use crate::modes::fy4::{Fy4HritSetting, Fy4LritSetting};
use crate::modes::gac::GacSetting;
use crate::modes::goes_cda::CdaSetting;
use crate::modes::gvar::GvarSetting;
use crate::modes::hrpt::{Variant, VariantChoice};
use crate::modes::meteor::{MeteorModulation, MeteorSetting, ModulationChoice};
use crate::modes::msg::MsgSetting;
use crate::modes::rdas::RdasSetting;
use crate::modes::sband::SbandSetting;
use crate::modes::xrit::{Gk2aSetting, GoesSetting, RsBasis};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_proto_ipcast::components::{self as ipcast, BbFrameFields};
use std::sync::OnceLock;

/// The fields of the link layers that every coded stack publishes: the front end, the demodulator, the code, the frames and the packets. A field whose report has not come has no value.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LinkFields {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: Option<i64>,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: Option<i64>,
    /// The bit error rate before the code.
    #[field(shared = keys::BER)]
    pub ber: Option<f64>,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
    #[field(label = "Phase ambiguity")]
    pub phase_ambiguity: Option<i64>,
    /// The code synchroniser holds its lock.
    #[field(label = "Code locked")]
    pub code_locked: Option<bool>,
    /// NRZ-M was found before the code.
    #[field(label = "NRZ-M")]
    pub nrzm: Option<bool>,
    /// Wrong bits of the last coded marker compared (of 52).
    #[field(label = "Marker errors")]
    pub marker_errors: Option<i64>,
    /// Reed-Solomon symbols corrected.
    #[field(label = "Bytes corrected")]
    pub bytes_corrected: Option<i64>,
    /// Frame markers found.
    #[field(label = "Markers")]
    pub markers: Option<i64>,
    /// The spacecraft of the last frame.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The virtual channel of the last frame.
    #[field(label = "Virtual channel")]
    pub vcid: Option<i64>,
    /// The virtual channel frame count of the last frame.
    #[field(label = "Frame count")]
    pub frame_count: Option<i64>,
    /// Space packets given out.
    #[field(label = "Packets")]
    pub packets: Option<i64>,
    /// Space packets lost.
    #[field(label = "Packets lost")]
    pub packets_lost: Option<i64>,
}

/// The fields of Meteor-M LRPT beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeteorLrptFields<'a> {
    /// The running modulation.
    #[field(label = "Modulation")]
    pub modulation: MeteorModulation,
    /// The modulation setting: automatic or fixed.
    #[field(label = "Modulation setting")]
    pub modulation_setting: ModulationChoice,
    /// The deinterleaver of the 80k link holds the markers.
    #[field(label = "Interleaver markers")]
    pub interleaver_locked: bool,
    /// The on-board time of the last image packet, seconds of the day (Moscow time).
    #[field(label = "On board", unit = "s")]
    pub onboard_time: Option<Seconds>,
    /// The quality factor of the last image packet.
    #[field(label = "Quality")]
    pub quality: Option<i64>,
    /// Rows of 8 lines in the picture.
    #[field(label = "Picture rows")]
    pub picture_rows: i64,
    /// Lines of 1568 pixels in the picture.
    #[field(label = "Picture lines")]
    pub picture_lines: i64,
    /// The row time learnt from the on-board times.
    #[field(label = "Row time", unit = "s")]
    pub row_time: Option<Seconds>,
    /// The signal time at which frames first decoded.
    #[field(label = "First frames", unit = "s")]
    pub first_decoding: Option<Seconds>,
    /// The channels the automatic colour view shows ("1,2,3").
    #[field(label = "Colour channels")]
    pub colour_channels: Option<&'a str>,
}

/// The fields of an xRIT stack (GOES HRIT, GK-2A LRIT and HRIT, Elektro-L) beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct XritFields<'a> {
    /// The name of the running link ("HRIT", "LRIT", "HRIT 665.4 ksym/s", "GGAK 5 ksym/s").
    #[field(label = "Link")]
    pub link: &'a str,
    /// The carrier is locked (also while frames decode).
    #[field(label = "Carrier locked")]
    pub carrier_locked: Option<bool>,
    /// The symbol rate the timing loop measures.
    #[field(label = "Symbol rate measured", unit = "Hz")]
    pub symbol_rate_measured_hz: Option<Hertz>,
    /// The second code output arrives inverted (as CCSDS sends it).
    #[field(label = "Second output inverted")]
    pub g2_inverted: Option<bool>,
    /// The basis of the Reed-Solomon symbols.
    #[field(label = "Reed-Solomon basis")]
    pub rs_basis: Option<RsBasis>,
    /// Bytes corrected over the last second, percent.
    #[field(label = "Byte errors", unit = "%")]
    pub byte_errors_pct: Option<f64>,
    /// Transfer frames (fill frames not counted).
    #[field(label = "Transfer frames")]
    pub transfer_frames: Option<i64>,
    /// Fill frames.
    #[field(label = "Fill frames")]
    pub fill_frames: Option<i64>,
    /// Gaps of the frame counts.
    #[field(label = "Frame count gaps")]
    pub frame_gaps: Option<i64>,
    /// Packets with a wrong CRC.
    #[field(label = "CRC errors")]
    pub crc_errors: Option<i64>,
    /// Files.
    #[field(label = "Files")]
    pub files: Option<i64>,
    /// Files with errors.
    #[field(label = "Files with errors")]
    pub file_errors: Option<i64>,
    /// Files whose length differs from their transport header.
    #[field(label = "Files of a wrong length")]
    pub length_errors: Option<i64>,
    /// Products kept.
    #[field(label = "Products")]
    pub products: Option<i64>,
    /// GK-2A: keys loaded.
    #[field(label = "Keys loaded")]
    pub keys: Option<i64>,
    /// GK-2A: segments waiting for a key.
    #[field(label = "Segments locked")]
    pub segments_locked: Option<i64>,
    /// What the user can do (empty when all is well).
    #[field(label = "Hint")]
    pub hint: &'a str,
    /// The signal time at which frames first decoded.
    #[field(label = "First frames", unit = "s")]
    pub first_decoding: Option<Seconds>,
    /// CADUs imported from a file.
    #[field(label = "CADUs imported")]
    pub cadus_imported: Option<i64>,
    /// GGAK frames.
    #[field(label = "GGAK frames")]
    pub ggak_frames: Option<i64>,
    /// GGAK frames with a good CRC.
    #[field(label = "GGAK good")]
    pub ggak_good: Option<i64>,
    /// GGAK frames with a bad CRC.
    #[field(label = "GGAK bad")]
    pub ggak_bad: Option<i64>,
    /// The marker of the last GGAK frame.
    #[field(label = "GGAK marker")]
    pub ggak_marker: Option<i64>,
    /// |B| of the last plausible GGAK field.
    #[field(label = "GGAK field")]
    pub ggak_field: Option<f64>,
}

/// The fields of FY-2 S-VISSR beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fy2SvissrFields<'a> {
    /// Lines decoded.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Sync windows found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Frames refused.
    #[field(label = "Refused frames")]
    pub rejected: i64,
    /// Frames with under 2 % preamble errors.
    #[field(label = "Usable frames")]
    pub usable: i64,
    /// The preamble bit error ratio of the last frame.
    #[field(label = "Preamble BER")]
    pub preamble_ber: f64,
    /// The estimate of the data bit error ratio (the set bits of the spare area).
    #[field(label = "Data BER")]
    pub data_ber: f64,
    /// The last line counter.
    #[field(label = "Line counter")]
    pub line_counter: Option<i64>,
    /// Pictures started.
    #[field(label = "Pictures started")]
    pub pictures_started: i64,
    /// Frames imported.
    #[field(label = "Imported frames")]
    pub imported: i64,
    /// What to look at when nothing decodes.
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// The fields of FY-3 AHRPT and MPT beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fy3Fields<'a> {
    /// The product: AHRPT or MPT.
    #[field(label = "Product")]
    pub product: Product,
    /// The preset setting (its choice key, "auto" for automatic).
    #[field(label = "Preset")]
    pub preset: &'a str,
    /// The satellite heard (from the spacecraft id).
    #[field(label = "Satellite")]
    pub satellite: Option<&'a str>,
    /// The symbol rate measured from the signal.
    #[field(label = "Measured symbol rate", unit = "Hz")]
    pub measured_symbol_rate_hz: Option<Hertz>,
    /// Both rails have their phase.
    #[field(label = "Rails locked")]
    pub rails_locked: Option<bool>,
    /// The lock quality of the I rail, 0 to 1.
    #[field(label = "I rail quality")]
    pub rail_i_quality: Option<f64>,
    /// The lock quality of the Q rail, 0 to 1.
    #[field(label = "Q rail quality")]
    pub rail_q_quality: Option<f64>,
    /// The differential hypothesis locked (0 to 7).
    #[field(label = "Differential hypothesis")]
    pub differential_hypothesis: Option<i64>,
    /// The rows of the tallest picture.
    #[field(label = "Picture rows")]
    pub picture_rows: i64,
    /// What to look at when nothing decodes.
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// The fields of NOAA APT.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoaaAptFields<'a> {
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The line lock holds.
    #[field(label = "Line lock")]
    pub locked: bool,
    /// Lines decoded.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Lines taken at the expected place because their sync was not found.
    #[field(label = "Lines without sync")]
    pub lines_missed: i64,
    /// The mean subcarrier amplitude (full deviation is 1).
    #[field(label = "Subcarrier level")]
    pub subcarrier_level: f64,
    /// The correlation of the last line sync with its pattern.
    #[field(label = "Sync correlation")]
    pub sync_quality: f64,
    /// Words a second.
    #[field(label = "Word rate", unit = "Hz")]
    pub word_rate_hz: Hertz,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of NOAA DSB.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoaaDsbFields<'a> {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// The spacecraft of the last frame.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// Sync words found.
    #[field(label = "Sync words")]
    pub sync_hits: i64,
    /// TIP minor frames.
    #[field(label = "TIP minor frames")]
    pub tip_frames: i64,
    /// HIRS lines.
    #[field(label = "HIRS lines")]
    pub lines: i64,
    /// The minor frame counter of the last frame, 0 to 319.
    #[field(label = "Minor frame")]
    pub minor_frame: Option<i64>,
    /// Which chip starts a bit: 0 or 1.
    #[field(label = "Chip pairing")]
    pub chip_pairing: Option<i64>,
    /// The data arrive inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// HIRS elements without odd parity.
    #[field(label = "HIRS parity errors")]
    pub hirs_parity_errors: i64,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of GOES GVAR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesGvarFields<'a> {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// The spacecraft of the last frame.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// Synchronisation codes found.
    #[field(label = "Sync codes")]
    pub sync_hits: i64,
    /// Idle blocks.
    #[field(label = "Idle blocks")]
    pub idle_blocks: i64,
    /// Blocks whose data field failed its CRC.
    #[field(label = "Data CRC errors")]
    pub data_crc_errors: i64,
    /// The id of the last block.
    #[field(label = "Block")]
    pub block_id: Option<i64>,
    /// The block count of the last block.
    #[field(label = "Block count")]
    pub block_count: Option<i64>,
    /// The scan count of the last imager block.
    #[field(label = "Scan")]
    pub scan: i64,
    /// Imager frames whose start was seen.
    #[field(label = "Imager frames")]
    pub frame_starts: i64,
    /// Rows of the visible picture.
    #[field(label = "Visible rows")]
    pub visible_rows: i64,
    /// Rows of the infrared picture.
    #[field(label = "Infrared rows")]
    pub infrared_rows: i64,
    /// Detector records left out.
    #[field(label = "Records refused")]
    pub records_refused: i64,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of AWS beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AwsFields<'a> {
    /// Scans of the radiometer.
    #[field(label = "Scans")]
    pub lines: i64,
    /// Science packets.
    #[field(label = "Science packets")]
    pub science_packets: i64,
    /// Packets whose error control field was wrong.
    #[field(label = "Packet CRC errors")]
    pub packet_crc_errors: i64,
    /// Scan records that were not 7599 bytes long.
    #[field(label = "Records of another length")]
    pub odd_records: i64,
    /// Idle frames.
    #[field(label = "Idle frames")]
    pub idle_frames: i64,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of NOAA GAC.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoaaGacFields<'a> {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// A confirmed frame lock is held.
    #[field(label = "Frame lock")]
    pub frame_lock: bool,
    /// The spacecraft of the last frame.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The frame code of the identification word: 0 in a GAC frame.
    #[field(label = "Identification code")]
    pub id_code: Option<i64>,
    /// Sync words found.
    #[field(label = "Sync words")]
    pub sync_hits: i64,
    /// TIP minor frames.
    #[field(label = "TIP minor frames")]
    pub tip_frames: i64,
    /// AVHRR lines.
    #[field(label = "AVHRR lines")]
    pub lines: i64,
    /// HIRS lines.
    #[field(label = "HIRS lines")]
    pub hirs_lines: i64,
    /// The data arrive inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of the raw Meteosat downlink beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MsgRawFields<'a> {
    /// The first ten bits of the last frame, read as one number.
    #[field(label = "Satellite number")]
    pub satellite_number: Option<i64>,
    /// Lines of part 0 stored.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Rows of single channels written to the SEVIRI picture.
    #[field(label = "Channel rows")]
    pub channel_rows: i64,
    /// Rows of the HRV picture.
    #[field(label = "HRV rows")]
    pub hrv_rows: i64,
    /// SEVIRI packets taken.
    #[field(label = "SEVIRI packets")]
    pub seviri_packets: i64,
    /// SEVIRI packets not placed: too short, or of a satellite number without clock numbers.
    #[field(label = "Packets not placed")]
    pub packets_refused: i64,
    /// Discs finished.
    #[field(label = "Discs finished")]
    pub discs: i64,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of every stack that lists frames: the link, the frame lock and the note.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FrameTableFields<'a> {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// A confirmed frame lock is held.
    #[field(label = "Frame lock")]
    pub frame_lock: bool,
    /// Sync words found.
    #[field(label = "Sync words")]
    pub sync_hits: i64,
    /// Reed-Solomon symbols corrected.
    #[field(label = "Bytes corrected")]
    pub bytes_corrected: i64,
    /// The spacecraft of a frame header.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The virtual channel of a frame header.
    #[field(label = "Virtual channel")]
    pub vcid: Option<i64>,
    /// The QPSK stream in use, 0 to 3.
    #[field(label = "Bit stream")]
    pub stream: Option<i64>,
    /// The data arrive inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of the S-band CCSDS stack beside those of the frame table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SbandCcsdsFields<'a> {
    /// The name of the link.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The carrier is locked (also while frames decode).
    #[field(label = "Carrier locked")]
    pub carrier_locked: Option<bool>,
    /// The code synchroniser holds its lock.
    #[field(label = "Code locked")]
    pub code_locked: Option<bool>,
    /// The phase ambiguity the code synchroniser undid: 0 to 7, plus 8 when the pairing was half a symbol late.
    #[field(label = "Phase ambiguity")]
    pub phase_ambiguity: Option<i64>,
    /// What the user can do (empty when all is well).
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// The fields of RDAS beside those of the frame table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RdasFields<'a> {
    /// The carrier is locked (also while frames decode).
    #[field(label = "Carrier locked")]
    pub carrier_locked: bool,
    /// MSU-GS frames found.
    #[field(label = "MSU-GS frames")]
    pub packets: i64,
    /// Infrared lines.
    #[field(label = "Infrared lines")]
    pub lines_ir: i64,
    /// Reduced visible lines.
    #[field(label = "Visible lines (reduced)")]
    pub lines_visible: i64,
    /// Input samples let go by the gate.
    #[field(label = "Samples let go by")]
    pub gate_skipped: Option<i64>,
    /// The share of the input let go by the gate, percent.
    #[field(label = "Input let go by", unit = "%")]
    pub gate_skipped_pct: Option<f64>,
    /// The height of the last gate line over the mean.
    #[field(label = "Gate line over mean")]
    pub gate_line: Option<f64>,
    /// What the user can do (empty when all is well).
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// The fields of EODB beside those of the frame table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EodbFields<'a> {
    /// The carrier is locked (also while frames decode).
    #[field(label = "Carrier locked")]
    pub carrier_locked: bool,
    /// Packets.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Space packets lost.
    #[field(label = "Packets lost")]
    pub packets_lost: i64,
    /// Space packets that went into a picture.
    #[field(label = "Packets or frames in pictures")]
    pub instrument_packets: i64,
    /// Picture lines.
    #[field(label = "Lines")]
    pub lines: i64,
    /// The satellite of the last good frame.
    #[field(label = "Satellite")]
    pub satellite: &'a str,
    /// The code hypothesis that holds the lock.
    #[field(label = "Locked hypothesis")]
    pub hypothesis: &'a str,
    /// The input rate is below the least rate of the link.
    #[field(label = "Rate too low")]
    pub rate_too_low: bool,
    /// CADUs of frame files taken.
    #[field(label = "CADUs from files")]
    pub imported: i64,
    /// Input samples let go by the gate.
    #[field(label = "Samples let go by")]
    pub gate_skipped: Option<i64>,
    /// The share of the input let go by the gate, percent.
    #[field(label = "Input let go by", unit = "%")]
    pub gate_skipped_pct: Option<f64>,
    /// The height of the last gate line over the mean.
    #[field(label = "Gate line over mean")]
    pub gate_line: Option<f64>,
    /// What the user can do (empty when all is well).
    #[field(label = "Hint")]
    pub hint: &'a str,
}

/// A finished image. The row key names the image. The picture itself leaves on the picture stream.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ImageRow<'a> {
    /// The product, or the instrument, that an image shows.
    #[field(label = "Product")]
    pub product: &'a str,
    /// The channel that an image shows, when the product names one.
    #[field(label = "Channel")]
    pub channel: Option<&'a str>,
    /// The signal time at which the image started.
    #[field(label = "Time", unit = "s")]
    pub time: Option<Seconds>,
    /// The image width in pixels.
    #[field(label = "Width", unit = "px")]
    pub width: i64,
    /// The image height in pixels (rows received for a picture that grows).
    #[field(label = "Height", unit = "px")]
    pub height: Option<i64>,
}

/// The packets of one channel of Meteor-M LRPT. The row key is the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeteorLrptChannelRow<'a> {
    /// The name of an image channel or the telemetry.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The packets of a channel.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// The frames of one virtual channel. The row key is the virtual channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct VirtualChannelRow {
    /// The virtual channel of a frame header.
    #[field(label = "Virtual channel")]
    pub vcid: i64,
    /// The good frames of a virtual channel.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// A product of an xRIT stack. The row key is the product id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ProductRow<'a> {
    /// The name of a product in the `products` table.
    #[field(label = "Product")]
    pub product: &'a str,
    /// The kind of a product in the `products` table.
    #[field(label = "Kind")]
    pub kind: ProductKind,
    /// How much of a product arrived: rows or segments for images, a size for others, "..." while it arrives, "errors".
    #[field(label = "Done")]
    pub done: &'a str,
}

/// A frame of a stack that lists frames. The row key is the place of the frame in the stream.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FrameRow<'a> {
    /// Signal time of a frame.
    #[field(label = "Time", unit = "s")]
    pub time: Seconds,
    /// The spacecraft of a frame header.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The virtual channel of a frame header.
    #[field(label = "Virtual channel")]
    pub vcid: Option<i64>,
    /// Bytes of a frame with its sync word.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// The check of a frame: good, check failed, or none.
    #[field(label = "Check")]
    pub check: &'a str,
    /// The first 24 bytes of a frame after decoding.
    #[field(label = "First bytes")]
    pub hex: &'a str,
}

/// An instrument of FY-3. The row key names the instrument.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fy3InstrumentRow<'a> {
    /// An instrument.
    #[field(label = "Instrument")]
    pub instrument: &'a str,
    /// The frames of an instrument.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The lines or scans of an instrument.
    #[field(label = "Lines or scans")]
    pub rows: i64,
}

/// A block of GOES GVAR. The row key is the place of the block.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GoesGvarBlockRow {
    /// The block count of the last block.
    #[field(label = "Block count")]
    pub block_count: i64,
    /// The id of the last block.
    #[field(label = "Block")]
    pub block_id: i64,
    /// The word size of a block.
    #[field(label = "Word size", unit = "bit")]
    pub word_size: i64,
    /// The word count of a block.
    #[field(label = "Word count")]
    pub word_count: i64,
    /// A header copy of a block had a good CRC.
    #[field(label = "Header CRC good")]
    pub header_ok: bool,
    /// The data field of a block had a good CRC.
    #[field(label = "Data CRC good")]
    pub data_ok: bool,
}

/// The frames of one virtual channel of AWS. The row key is the virtual channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AwsVirtualChannelRow {
    /// A virtual channel.
    #[field(label = "Virtual channel")]
    pub vcid: i64,
    /// The frames of a virtual channel.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// The packets of one application process of AWS. The row key is the APID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AwsApidRow {
    /// An application process.
    #[field(label = "APID")]
    pub apid: i64,
    /// The packets of an application process.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// A product that changed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ProductEvent<'a> {
    /// Where the product comes from: its name.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text: the name of a product, the text of a text product.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a satellite.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The product id.
    #[field(label = "Product")]
    pub id: i64,
    /// The kind of a product.
    #[field(label = "Kind")]
    pub kind: ProductKind,
    /// The name of a product.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The virtual channel of a product.
    #[field(label = "Virtual channel")]
    pub vcid: i64,
    /// The APID of a product.
    #[field(label = "APID")]
    pub apid: i64,
    /// Rows received.
    #[field(label = "Rows")]
    pub rows: i64,
    /// The picture width.
    #[field(label = "Width")]
    pub width: i64,
    /// The picture height.
    #[field(label = "Height")]
    pub height: i64,
    /// Segments received.
    #[field(label = "Segments")]
    pub segments: i64,
    /// Segments in all.
    #[field(label = "Segments in all")]
    pub segments_total: i64,
    /// Every segment arrived.
    #[field(label = "Complete")]
    pub complete: bool,
    /// A file of the product had errors.
    #[field(label = "Errors")]
    pub errors: bool,
    /// Grows with every change of the product.
    #[field(label = "Version")]
    pub version: i64,
    /// A note ("picture not decoded: ...").
    #[field(label = "Note")]
    pub note: Option<&'a str>,
}

/// A text product.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TextEvent<'a> {
    /// Where the product comes from: its name.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text: the name of a product, the text of a text product.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a satellite.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The name of a product.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The file name inside a ZIP archive.
    #[field(label = "File in the archive")]
    pub inner_name: Option<&'a str>,
    /// The product id.
    #[field(label = "Product")]
    pub id: i64,
}

/// A GGAK telemetry frame of Elektro-L.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GgakFrameEvent {
    /// The family of the source: a satellite.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The marker of a GGAK frame.
    #[field(label = "Marker")]
    pub marker: i64,
    /// The sub-frame of a GGAK frame.
    #[field(label = "Sub-frame")]
    pub subframe: i64,
    /// The time stamp of a GGAK frame.
    #[field(label = "Time stamp")]
    pub timestamp: i64,
    /// The CRC of a GGAK frame is right.
    #[field(label = "CRC good")]
    pub crc_ok: bool,
    /// The polarity of a GGAK frame was inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// Bx of the first triple of a GGAK frame.
    #[field(label = "Bx")]
    pub bx: i64,
    /// By of the first triple of a GGAK frame.
    #[field(label = "By")]
    pub by: i64,
    /// Bz of the first triple of a GGAK frame.
    #[field(label = "Bz")]
    pub bz: i64,
}

pub use sdrkit_codec_image::component::PictureFields;

/// The note of the Arktika-M submode of Elektro-L: what no public document confirms.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArktikaFields<'a> {
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The finished images, newest first.
pub static IMAGES: Table<ImageRow<'static>> = Table::new("images", "Images", 80);
/// The packets of each channel of Meteor-M LRPT.
pub static METEOR_LRPT_CHANNELS: Table<MeteorLrptChannelRow<'static>> =
    Table::new("channels", "Channels", 16);
/// The frames of each virtual channel.
pub static VIRTUAL_CHANNELS: Table<VirtualChannelRow> =
    Table::new("virtual_channels", "Virtual channels", 64);
/// The products of an xRIT stack.
pub static PRODUCTS: Table<ProductRow<'static>> = Table::new("products", "Products", 80);
/// The newest frames of a stack that lists frames.
pub static FRAMES: Table<FrameRow<'static>> =
    Table::new("frames", "Frames", crate::components::table::TABLE_ROWS).label(&FrameRow::HEX);
/// The instruments of FY-3.
pub static FY3_INSTRUMENTS: Table<Fy3InstrumentRow<'static>> =
    Table::new("instruments", "Instruments", 32);
/// The newest blocks of GOES GVAR.
pub static GOES_GVAR_BLOCKS: Table<GoesGvarBlockRow> = Table::new("blocks", "Blocks", 64);
/// The frames of each virtual channel of AWS.
pub static AWS_VIRTUAL_CHANNELS: Table<AwsVirtualChannelRow> =
    Table::new("virtual_channels", "Virtual channels", 8);
/// The packets of each application process of AWS.
pub static AWS_APIDS: Table<AwsApidRow> = Table::new("apids", "Application processes", 32);

/// A product that changed.
pub static PRODUCT_EVENT: EventKind<ProductEvent<'static>> =
    EventKind::new("satimg.product", "Product");
/// A text product.
pub static TEXT_EVENT: EventKind<TextEvent<'static>> =
    EventKind::new("satimg.text", "Text product");
/// A GGAK telemetry frame.
pub static GGAK_EVENT: EventKind<GgakFrameEvent> =
    EventKind::new("satimg.ggak_frame", "GGAK frame");

/// The series `esn0`: Es/N0 over the last 30 s.
pub const ESN0_SERIES: SeriesDef = SeriesDef {
    key: "esn0",
    label: "Es/N0",
    x_unit: "s",
    y_unit: "dB",
};
/// The series `byte_errors`: byte errors in percent over the last 30 s.
pub const BYTE_ERRORS_SERIES: SeriesDef = SeriesDef {
    key: "byte_errors",
    label: "Byte errors",
    x_unit: "s",
    y_unit: "%",
};
/// The series `frames_per_s`: good frames a second over the last 30 s.
pub const FRAMES_PER_S_SERIES: SeriesDef = SeriesDef {
    key: "frames_per_s",
    label: "Frames per second",
    x_unit: "s",
    y_unit: "",
};
/// The series `constellation`: the newest symbols.
pub const CONSTELLATION_SERIES: SeriesDef = SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
};

/// The arguments of a command that reads a file.
#[derive(Arguments)]
pub struct FileArgs<'a> {
    /// A file in the host's storage.
    #[arg(label = "File", help = "A file in the host's storage.")]
    pub path: &'a str,
}

/// The arguments of `add_keys`.
#[derive(Arguments)]
pub struct KeysArgs<'a> {
    /// Lines "index:hexkey".
    #[arg(
        label = "Keys",
        help = "Lines \"index:hexkey\": the key index in hexadecimal, then 16 hexadecimal digits."
    )]
    pub keys: &'a str,
}

/// The arguments of `product` of EODB.
#[derive(Arguments)]
pub struct EodbProductArgs<'a> {
    /// The key of a product of the link's instrument.
    #[arg(
        label = "Product",
        help = "The key of a product of the link's instrument."
    )]
    pub product: &'a str,
    /// The width limit of the product, 0 for full size.
    #[arg(
        label = "Width limit",
        unit = "px",
        default = 0,
        min = 0,
        max = 100_000.0,
        step = 1,
        help = "0 for full size."
    )]
    pub width: Option<i64>,
}

/// Decodes a file of CADUs: the frame synchroniser of a link takes them at the next input.
pub const OPEN_CADU: CommandKind<FileArgs<'static>> = CommandKind::new("open_cadu", "Open CADU")
    .help("Decodes a file of 1024-byte CADUs, as sent or derandomised, at the next input.");
/// Decodes a file of derandomised CADUs: the CADU import takes them at the next input.
pub const IMPORT_CADU: CommandKind<FileArgs<'static>> = CommandKind::new("open_cadu", "Open CADU")
    .help("Decodes a file of 1024-byte CADUs at the next input.");
/// Reads a KMA key file.
pub const IMPORT_KEYS: CommandKind<FileArgs<'static>> =
    CommandKind::new("import_keys", "Import keys").help("Reads a KMA key file.");
/// Adds keys typed as text.
pub const ADD_KEYS: CommandKind<KeysArgs<'static>> =
    CommandKind::new("add_keys", "Add keys").help("Adds keys typed as text.");
/// Decodes a file of clear S-VISSR frames.
pub const OPEN_SVISSR: CommandKind<FileArgs<'static>> =
    CommandKind::new("open_svissr", "Open .svissr")
        .help("Decodes a file of clear 44 356-byte frames at the next input.");
/// Decodes a frame file of EODB.
pub const OPEN_FRAMES: CommandKind<FileArgs<'static>> =
    CommandKind::new("open_frames", "Open frame file").help(
        "Decodes a file of CADUs of the link's length, each starting with the marker 1ACFFC1D.",
    );
/// Makes a colour product of EODB.
pub const MAKE_PRODUCT: CommandKind<EodbProductArgs<'static>> =
    CommandKind::new("product", "Make product")
        .help("Makes a colour product of the channel pictures.");

/// The `clear` command of the picture component of FY-2: new pictures.
pub const NEW_PICTURES: sdrkit_core::command::Command =
    sdrkit_core::command::Command::new("clear", "New picture")
        .help("Forgets every picture and starts new ones.");

/// The same stack with the series that every coded stack publishes.
fn coded(s: StackShape) -> StackShape {
    s.series(ESN0_SERIES)
        .series(BYTE_ERRORS_SERIES)
        .series(FRAMES_PER_S_SERIES)
        .series(CONSTELLATION_SERIES)
}

/// A stack with the link fields, the fields `F` of the stack, the pictures and the images: the images are a log of what
/// was received and stay over a change of frequency.
fn pictures<F: Fields>(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<LinkFields>()
        .fields::<F>()
        .fields::<PictureFields>()
}

/// An xRIT stack: the products and the images are written by the product store, and stay over a change of frequency.
fn xrit(id: &'static str) -> StackShape {
    coded(
        StackShape::new(id)
            .fields::<LinkFields>()
            .fields::<XritFields>()
            .table(&VIRTUAL_CHANNELS)
            .table(&PRODUCTS)
            .table(&IMAGES)
            .component("products", |c| {
                c.writes_table(&PRODUCTS).writes_table(&IMAGES)
            }),
    )
    .event(&PRODUCT_EVENT)
    .event(&TEXT_EVENT)
    .command(&OPEN_CADU)
    .command(&crate::modes::xrit::GGAK_CLEAR)
    .kept(&["products", "images"])
}

/// `shape` with the results of the DVB-S2 receiver that `sdrkit_proto_dvbs2::attach` adds to the graph: the
/// result set, the tables and the series of the receiver as items of the stack, written by its report component.
fn dvbs2(shape: StackShape) -> StackShape {
    use sdrkit_proto_dvbs2::typed as s2;
    s2::with_dvbs2_receiver(shape).component(sdrkit_proto_dvbs2::S2_REPORT, |c| {
        c.fields::<s2::Dvbs2Fields>()
            .shares_declared()
            .writes_table(&s2::DVBS2_CARRIERS)
            .writes_table(&s2::DVBS2_STREAMS)
            .writes_table(&s2::DVBS2_STREAM_MODCODS)
    })
}

/// The same stack with the frame table, the table of virtual channels and the series: the frames are of the present signal,
/// so nothing stays over a change of frequency.
fn frame_table(s: StackShape) -> StackShape {
    coded(s.table(&FRAMES).table(&VIRTUAL_CHANNELS))
}

// The stacks over a DVB-S2 carrier.

/// The counts of the transfer frames in the data fields of a DVB-S2 carrier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CaduFields<'a> {
    /// CADUs found.
    #[field(label = "CADUs")]
    pub cadus: i64,
    /// The spacecraft id of the frames. No value before a frame is read.
    #[field(label = "Spacecraft")]
    pub spacecraft: Option<i64>,
    /// The virtual channel of the last frame.
    #[field(label = "Virtual channel")]
    pub vcid: Option<i64>,
    /// The frames of each virtual channel, for people.
    #[field(label = "Frames per virtual channel")]
    pub frames_per_vc: &'a str,
    /// Space packets.
    #[field(label = "Packets")]
    pub space_packets: i64,
    /// Space packets lost.
    #[field(label = "Packets lost")]
    pub packets_lost: i64,
    /// Breaks in the stream of data fields.
    #[field(label = "Breaks in the stream")]
    pub field_breaks: i64,
}

/// The result fields of the file layer of `gk2a_uhrit`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UhritFields<'a> {
    /// Frames with a bad CRC-16.
    #[field(label = "Frames with a bad CRC")]
    pub frames_crc_bad: i64,
    /// Packets with a bad CRC-32.
    #[field(label = "Packets with a bad CRC")]
    pub packets_crc_bad: i64,
    /// Files.
    #[field(label = "Files")]
    pub files: i64,
    /// The last file name.
    #[field(label = "Last file")]
    pub last_file: &'a str,
    /// Products kept by the file layer. No value before the file layer reports.
    #[field(label = "Products")]
    pub products: Option<i64>,
    /// What the user can do.
    #[field(label = "Hint")]
    pub hint: &'a str,
    /// A note on the state of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the packets of each APID of `gk2a_uhrit`. The row key is the APID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UhritApidRow {
    /// The APID.
    #[field(label = "APID")]
    pub apid: i64,
    /// The packets of the APID.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// A row of the frames of each virtual channel of `gk2a_uhrit`. The row key is the virtual channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UhritFrameRow {
    /// The virtual channel.
    #[field(label = "Virtual channel")]
    pub vcid: i64,
    /// The frames of the virtual channel.
    #[field(label = "Frames")]
    pub frames: i64,
}

/// The result fields of `fy4_lrit` and `fy4a_hrit` beside those of the DVB-S2 receiver. A count of a layer that has
/// not reported has no value.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fy4Fields<'a> {
    /// The name of the running link.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The carrier of the link: the standard, the MODCOD and the symbol rate.
    #[field(label = "Signal")]
    pub signal: &'a str,
    /// Transport packets.
    #[field(label = "Transport packets")]
    pub ts_packets: i64,
    /// Transport errors.
    #[field(label = "Transport errors")]
    pub ts_errors: i64,
    /// CADUs found in the transport stream or imported.
    #[field(label = "CADUs")]
    pub cadus: i64,
    /// CADUs imported from a file.
    #[field(label = "CADUs imported")]
    pub cadus_imported: i64,
    /// Transfer frames (fill frames not counted).
    #[field(label = "Transfer frames")]
    pub transfer_frames: Option<i64>,
    /// Fill frames.
    #[field(label = "Fill frames")]
    pub fill_frames: Option<i64>,
    /// Gaps of the frame counts.
    #[field(label = "Frame count gaps")]
    pub frame_gaps: Option<i64>,
    /// Space packets.
    #[field(label = "Packets")]
    pub packets: Option<i64>,
    /// Files.
    #[field(label = "Files")]
    pub files: Option<i64>,
    /// Files with errors.
    #[field(label = "Files with errors")]
    pub file_errors: Option<i64>,
    /// Encrypted files.
    #[field(label = "Encrypted files")]
    pub encrypted_files: Option<i64>,
    /// Image segments.
    #[field(label = "Image segments")]
    pub image_segments: Option<i64>,
    /// Pictures complete.
    #[field(label = "Pictures complete")]
    pub pictures_complete: Option<i64>,
    /// Products kept.
    #[field(label = "Products")]
    pub products: Option<i64>,
    /// What the user can do (empty when all is well).
    #[field(label = "Hint")]
    pub hint: &'a str,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
}

/// The result fields of `goes_grb` beside those of the DVB-S2 receiver and the CADUs.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GrbFields<'a> {
    /// Packets with a bad CRC.
    #[field(label = "Packets with a bad CRC")]
    pub packets_crc_bad: i64,
    /// Payloads.
    #[field(label = "Payloads")]
    pub payloads: i64,
    /// Picture blocks.
    #[field(label = "Picture blocks")]
    pub picture_blocks: i64,
    /// JPEG 2000 failures.
    #[field(label = "JPEG 2000 failures")]
    pub j2k_failures: i64,
    /// Files.
    #[field(label = "Files")]
    pub files: i64,
    /// Rows of the picture shown.
    #[field(label = "Lines of the picture")]
    pub picture_lines: i64,
    /// The product shown.
    #[field(label = "Product shown")]
    pub shown_product: &'a str,
    /// What the user can do.
    #[field(label = "Hint")]
    pub hint: &'a str,
    /// A note on the state of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A picture product of `goes_grb`. The row key is the APID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GrbProductRow<'a> {
    /// The APID.
    #[field(label = "APID")]
    pub apid: i64,
    /// The name of the product of the APID.
    #[field(label = "Product")]
    pub name: &'a str,
    /// The signal time of the last update.
    #[field(label = "Last updated", unit = "s")]
    pub updated: Seconds,
}

/// A row of the packets of each APID of `goes_grb`. The row key is the APID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GrbApidRow<'a> {
    /// The APID.
    #[field(label = "APID")]
    pub apid: i64,
    /// The name of the product of the APID.
    #[field(label = "Product")]
    pub name: &'a str,
    /// The packets of the APID.
    #[field(label = "Packets")]
    pub packets: i64,
}

/// The picture products of `goes_grb`, by APID.
pub static GRB_PRODUCTS: Table<GrbProductRow<'static>> =
    Table::new("grb_products", "Products", 256).label(&GrbProductRow::NAME);
/// The packets of the 32 most frequent APIDs of `goes_grb`.
pub static GRB_PACKETS_BY_APID: Table<GrbApidRow<'static>> =
    Table::new("packets_by_apid", "Packets by APID", 32).label(&GrbApidRow::NAME);

/// The GLM lightning results of `goes_grb`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlmFields {
    /// Complete GLM payloads (APIDs 300 to 303).
    #[field(label = "GLM payloads")]
    pub glm_payloads: i64,
    /// GLM payloads dropped because a packet of them was lost or damaged.
    #[field(label = "GLM payloads dropped")]
    pub glm_dropped: i64,
    /// Packets missing from the sequence counts of the GLM APIDs.
    #[field(label = "GLM packets lost")]
    pub glm_packets_lost: i64,
    /// Compressed GLM payloads, saved as files and not decoded.
    #[field(label = "GLM payloads not decoded")]
    pub glm_not_decoded: i64,
    /// Flash and event units whose length does not match their count.
    #[field(label = "GLM units of a wrong length")]
    pub glm_bad_length: i64,
    /// Group units whose length does not match their count.
    #[field(label = "GLM group units of a wrong length")]
    pub glm_group_bad_length: i64,
    /// Flashes decoded.
    #[field(label = "Flashes")]
    pub flash_count: i64,
    /// Groups decoded.
    #[field(label = "Groups")]
    pub group_count: i64,
    /// Events counted.
    #[field(label = "Events")]
    pub event_count: i64,
    /// Flash and group records with an impossible value.
    #[field(label = "GLM records rejected")]
    pub glm_rejected: i64,
    /// The time of the first event of the newest flash (UTC).
    #[field(label = "Newest flash")]
    pub newest_flash: Option<Time>,
    /// Flashes in the 60 s of flash time up to the newest flash.
    #[field(label = "Flash rate", unit = "1/min")]
    pub flash_rate: f64,
}

/// A lightning flash of `goes_grb`. The row key is the product time (microseconds since J2000) and the flash id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlmFlashRow {
    /// The product-unique flash id.
    #[field(label = "Flash")]
    pub flash: i64,
    /// The time of the first event (UTC).
    #[field(label = "Time")]
    pub time: Time,
    /// From the first to the last event.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// The energy-weighted centroid, latitude first.
    #[field(label = "Position", unit = "°")]
    pub position: (f64, f64),
    /// The area covered.
    #[field(label = "Area", unit = "km²")]
    pub area_km2: Option<f64>,
    /// The radiant energy.
    #[field(label = "Energy", unit = "fJ")]
    pub energy_fj: Option<f64>,
    /// The data quality.
    #[field(label = "Quality")]
    pub quality: Option<crate::modes::glm::GlmQuality>,
    /// Groups received with this flash as parent.
    #[field(label = "Groups")]
    pub groups: i64,
    /// Events received in those groups.
    #[field(label = "Events")]
    pub events: i64,
}

/// A lightning group of `goes_grb`. The row key is the product time (microseconds since J2000) and the group id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlmGroupRow {
    /// The product-unique group id.
    #[field(label = "Group")]
    pub group: i64,
    /// The mean time of the events (UTC).
    #[field(label = "Time")]
    pub time: Time,
    /// The energy-weighted centroid, latitude first.
    #[field(label = "Position", unit = "°")]
    pub position: (f64, f64),
    /// The area covered.
    #[field(label = "Area", unit = "km²")]
    pub area_km2: Option<f64>,
    /// The radiant energy.
    #[field(label = "Energy", unit = "fJ")]
    pub energy_fj: Option<f64>,
    /// The parent flash id.
    #[field(label = "Flash")]
    pub flash: i64,
    /// The data quality.
    #[field(label = "Quality")]
    pub quality: Option<crate::modes::glm::GlmQuality>,
    /// Events received with this group as parent.
    #[field(label = "Events")]
    pub events: i64,
}

/// The newest lightning flashes of `goes_grb`.
pub static GLM_FLASHES: Table<GlmFlashRow> = Table::new("flashes", "Lightning flashes", 64);
/// The newest lightning groups of `goes_grb`.
pub static GLM_GROUPS: Table<GlmGroupRow> = Table::new("groups", "Lightning groups", 64);

/// The result fields of `sat_filecast` beside those of the DVB-S2 receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FilecastFields<'a> {
    /// The PID read.
    #[field(label = "PID")]
    pub pid: i64,
    /// Transport packets on the PID.
    #[field(label = "Packets on the PID")]
    pub pid_packets: i64,
    /// IP datagrams.
    #[field(label = "Datagrams")]
    pub datagrams: i64,
    /// Datagrams that are not UDP.
    #[field(label = "Not UDP")]
    pub not_udp: i64,
    /// File-broadcast packets.
    #[field(label = "File-broadcast packets")]
    pub broadcast_packets: i64,
    /// File-broadcast packets refused.
    #[field(label = "Bad packets")]
    pub bad_packets: i64,
    /// Complete files.
    #[field(label = "Files complete")]
    pub files_complete: i64,
    /// Incomplete files.
    #[field(label = "Files incomplete")]
    pub files_incomplete: i64,
    /// Files whose bzip2 data failed.
    #[field(label = "Files failed")]
    pub files_failed: i64,
    /// Files dropped (the queue was full).
    #[field(label = "Files dropped")]
    pub files_dropped: i64,
    /// Bytes of files.
    #[field(label = "Bytes")]
    pub bytes: i64,
    /// Files open.
    #[field(label = "Open files")]
    pub open_files: i64,
    /// The last error.
    #[field(label = "Last error")]
    pub last_error: &'a str,
    /// Pictures finished.
    #[field(label = "Pictures finished")]
    pub pictures_finished: i64,
    /// What the user can do.
    #[field(label = "Hint")]
    pub hint: &'a str,
    /// A note on the state of the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A file of `sat_filecast`. The row key is the number of the file.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RecentFileRow<'a> {
    /// The name of the file.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The size of the file.
    #[field(label = "Size", unit = "bytes")]
    pub size: i64,
    /// The parts of the file.
    #[field(label = "Parts")]
    pub parts: i64,
    /// The parts received.
    #[field(label = "Parts received")]
    pub received: i64,
    /// Every part arrived.
    #[field(label = "Complete")]
    pub complete: bool,
}

/// A picture of `sat_filecast`. The row key is the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FilecastPictureRow<'a> {
    /// The channel.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The time stamp of the picture.
    #[field(label = "Time stamp")]
    pub stamp: &'a str,
    /// The width of the picture.
    #[field(label = "Width", unit = "px")]
    pub width: i64,
    /// The height of the picture.
    #[field(label = "Height", unit = "px")]
    pub height: i64,
    /// The segments received.
    #[field(label = "Segments")]
    pub segments: i64,
    /// The segments in all.
    #[field(label = "Total")]
    pub total: i64,
}

/// The newest files of `sat_filecast`.
pub static RECENT_FILES: Table<RecentFileRow<'static>> = Table::new(
    "recent_files",
    "Recent files",
    crate::modes::filecast::RECENT,
)
.label(&RecentFileRow::NAME);
/// The pictures of `sat_filecast`, one row per channel.
pub static FILECAST_PICTURES: Table<FilecastPictureRow<'static>> =
    Table::new("pictures", "Pictures", 64);

/// The fields of `hrpt` beside the link layers.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HrptFields {
    /// The running variant.
    #[field(label = "Satellite")]
    pub variant: Variant,
    /// The variant setting: automatic or fixed.
    #[field(label = "Satellite setting")]
    pub variant_setting: VariantChoice,
    /// The code synchroniser undid a mirrored transform (the second output inverted). No value for NOAA.
    #[field(label = "Second output inverted")]
    pub phase_ambiguity_mirrored: Option<bool>,
    /// NOAA: the polarity plus 2 times the chip phase of the framer. No value for the other satellites.
    #[field(label = "NOAA rotation")]
    pub noaa_rotation: Option<i64>,
    /// AVHRR packets of APID 103.
    #[field(label = "APID 103 packets")]
    pub packets_apid_103: i64,
    /// AVHRR packets of APID 104.
    #[field(label = "APID 104 packets")]
    pub packets_apid_104: i64,
    /// Other packets.
    #[field(label = "Other packets")]
    pub other_packets: i64,
    /// NOAA minor frames.
    #[field(label = "NOAA minor frames")]
    pub noaa_frames: i64,
    /// AVHRR lines decoded.
    #[field(label = "AVHRR lines")]
    pub avhrr_lines: i64,
    /// Lines of 2048 pixels in the picture.
    #[field(label = "Picture lines")]
    pub picture_lines: i64,
    /// The on-board time of the last line, seconds of the day.
    #[field(label = "On board", unit = "s")]
    pub onboard_time: Option<Seconds>,
    /// HIRS lines given out (NOAA).
    #[field(label = "HIRS lines")]
    pub hirs_lines: i64,
    /// HIRS elements put into a line.
    #[field(label = "HIRS elements")]
    pub hirs_valid: i64,
    /// HIRS elements without odd parity.
    #[field(label = "HIRS parity errors")]
    pub hirs_parity_errors: i64,
}

/// The fields of the Meteor submode of `hrpt`: the link, the transport frames and the MSU-MR frames.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MeteorHrptFields<'a> {
    /// The Es/N0 of the demodulator, dB.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The level of the input, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The carrier offset: the energy location, the search and the loop.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The error of the symbol clock, ppm.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The nominal symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// Transport frames decoded.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Transport frames that failed their check: the link has no check, so 0.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// The seconds of signal decoded.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The input rate.
    #[field(label = "Input rate", unit = "Hz")]
    pub input_rate_hz: Option<Hertz>,
    /// The carrier lock metric, 0 to 1.
    #[field(label = "Carrier lock")]
    pub lock_metric: Option<f64>,
    /// A confirmed frame lock is held.
    #[field(label = "Frame lock")]
    pub frame_lock: bool,
    /// Sync words of transport frames found.
    #[field(label = "Sync words")]
    pub sync_hits: i64,
    /// MSU-MR frames.
    #[field(label = "MSU-MR frames")]
    pub packets: i64,
    /// Sync words of MSU-MR frames found.
    #[field(label = "MSU-MR sync words")]
    pub line_sync_hits: i64,
    /// MSU-MR lines in the picture.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Which chip starts a bit: 0 or 1. No value before the pairing is found.
    #[field(label = "Chip pairing")]
    pub chip_pairing: Option<i64>,
    /// The data arrive inverted.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The packets of each APID of `gk2a_uhrit`.
pub static UHRIT_PACKETS_BY_APID: Table<UhritApidRow> =
    Table::new("packets_by_apid", "Packets by APID", 32);
/// The frames of each virtual channel of `gk2a_uhrit` and `goes_grb`.
pub static UHRIT_FRAMES: Table<UhritFrameRow> = Table::new("frames", "Frames", 64);

/// The description of `elektro_l`: an xRIT stack with the GGAK frames.
fn elektro() -> StackShape {
    xrit("elektro_l")
        .event(&GGAK_EVENT)
        .settings(crate::modes::xrit::ElektroSetting::SETTINGS)
}

/// The description of `elektro_l` in the Arktika-M submode: the stack and the note of what no public document confirms.
/// The family holds one description for each stack, the one of the first submode, so this one is not part of it.
pub fn arktika_shape() -> &'static StackShape {
    static SHAPE: OnceLock<StackShape> = OnceLock::new();
    SHAPE.get_or_init(|| elektro().fields::<ArktikaFields>())
}

/// `shape` of `hrpt` with the series, the settings and the command of a picture stack: the images stay over a change
/// of frequency.
fn hrpt_stack(
    shape: StackShape,
    settings: &'static [&'static dyn sdrkit_core::DeclareSetting],
) -> StackShape {
    coded(shape)
        .settings(settings)
        .command(&crate::components::instruments::CLEAR)
        .kept(&["images"])
}

/// The description of `hrpt` in the Meteor submode, which has its own result sets and no HIRS view. The family holds
/// one description for each stack, the one of the first submode, so this one is not part of it.
pub fn meteor_hrpt_shape() -> &'static StackShape {
    static SHAPE: OnceLock<StackShape> = OnceLock::new();
    SHAPE.get_or_init(|| {
        hrpt_stack(
            StackShape::new("hrpt")
                .fields::<MeteorHrptFields>()
                .fields::<PictureFields>()
                .table(&IMAGES),
            crate::modes::hrpt_meteor::settings(),
        )
    })
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let clear_table = &crate::components::table::CLEAR;
        let new_picture = &crate::components::instruments::CLEAR;
        // The products and the pictures of the file layer are a store of what was received: they stay
        // over a change of frequency. The counts and the tables of the frames are of the present carrier.
        let uhrit = dvbs2(StackShape::new("gk2a_uhrit"))
            .fields::<CaduFields>()
            .fields::<UhritFields>()
            .table(&UHRIT_PACKETS_BY_APID)
            .table(&UHRIT_FRAMES)
            .table(&PRODUCTS)
            .table(&IMAGES)
            .component("products", |c| {
                c.writes_table(&PRODUCTS).writes_table(&IMAGES)
            })
            .event(&PRODUCT_EVENT)
            .event(&TEXT_EVENT)
            .settings(crate::modes::uhrit::UhritSetting::SETTINGS)
            .command(&IMPORT_KEYS)
            .command(&ADD_KEYS)
            .command(&crate::components::xrit::CLEAR)
            .kept(&["products", "images"]);
        // The products and the pictures of the FY-4 store stay over a change of frequency, as in the other xRIT
        // stacks.
        let fy4 =
            |id: &'static str, settings: &'static [&'static dyn sdrkit_core::DeclareSetting]| {
                ipcast::describe::<BbFrameFields>(
                    dvbs2(StackShape::new(id)).fields::<Fy4Fields>(),
                    sdrkit_proto_dvbs2::S2_BBFRAME,
                )
                .table(&PRODUCTS)
                .table(&IMAGES)
                .component("products", |c| {
                    c.writes_table(&PRODUCTS).writes_table(&IMAGES)
                })
                .event(&PRODUCT_EVENT)
                .event(&TEXT_EVENT)
                .settings(settings)
                .command(&IMPORT_CADU)
                .command(&crate::components::xrit::CLEAR)
                .kept(&["products", "images"])
            };
        // The list of picture products and the lightning tables are a log of what was received and stay over a change of
        // frequency. The counts and the tables of the frames and the packets are of the present carrier.
        let grb = dvbs2(StackShape::new("goes_grb"))
            .fields::<CaduFields>()
            .fields::<GrbFields>()
            .fields::<GlmFields>()
            .table(&GRB_PRODUCTS)
            .table(&GRB_PACKETS_BY_APID)
            .table(&UHRIT_FRAMES)
            .table(&GLM_FLASHES)
            .table(&GLM_GROUPS)
            .settings(crate::modes::grb::GrbSetting::SETTINGS)
            .command(&crate::modes::grb::CLEAR)
            .kept(&["grb_products", "flashes", "groups"]);
        // The files received and the pictures are a log of what was received and stay over a change of frequency.
        let filecast = ipcast::describe::<BbFrameFields>(
            dvbs2(StackShape::new("sat_filecast")).fields::<FilecastFields>(),
            sdrkit_proto_dvbs2::S2_BBFRAME,
        )
        .table(&RECENT_FILES)
        .table(&FILECAST_PICTURES)
        .settings(crate::modes::filecast::FilecastSetting::SETTINGS)
        .command(&crate::modes::filecast::CLEAR)
        .kept(&["recent_files", "pictures"]);
        Described::new("satimg", 1, 2)
            .added(
                1,
                2,
                &[
                    "meteor_lrpt.content",
                    "goes_hrit.content",
                    "gk2a_lrit.content",
                    "gk2a_hrit.content",
                    "fy2_svissr.content",
                    "fy3_ahrpt.content",
                    "fy3_mpt.content",
                    "noaa_apt.content",
                    "noaa_dsb.content",
                    "goes_gvar.content",
                    "aws.content",
                    "noaa_gac.content",
                    "msg_raw.content",
                    "goes_cda.content",
                    "goes_mdl.content",
                    "gk2a_cdas.content",
                    "sband_ccsds.content",
                    "rdas.content",
                    "gk2a_uhrit.content",
                    "fy4_lrit.content",
                    "fy4a_hrit.content",
                    "goes_grb.content",
                    "sat_filecast.content",
                    "elektro_l.content",
                    "hrpt.content",
                    "eodb.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::SatelliteImagery,
                &[
                    "meteor_lrpt",
                    "goes_hrit",
                    "gk2a_lrit",
                    "gk2a_hrit",
                    "fy2_svissr",
                    "fy3_ahrpt",
                    "fy3_mpt",
                    "noaa_apt",
                    "noaa_dsb",
                    "goes_gvar",
                    "aws",
                    "noaa_gac",
                    "msg_raw",
                    "rdas",
                    "gk2a_uhrit",
                    "fy4_lrit",
                    "fy4a_hrit",
                    "goes_grb",
                    "sat_filecast",
                    "elektro_l",
                    "hrpt",
                    "eodb",
                ],
            )
            .content(
                sdrkit_core::ContentKind::SatelliteLinks,
                &["goes_cda", "goes_mdl", "gk2a_cdas", "sband_ccsds"],
            )
            .pictures(&[
                "meteor_lrpt",
                "fy2_svissr",
                "fy3_ahrpt",
                "fy3_mpt",
                "noaa_apt",
                "noaa_dsb",
                "goes_gvar",
                "aws",
                "noaa_gac",
                "msg_raw",
                "rdas",
                "goes_grb",
                "sat_filecast",
                "hrpt",
                "eodb",
                "goes_hrit",
                "gk2a_lrit",
                "gk2a_hrit",
                "elektro_l",
                "fy4_lrit",
                "fy4a_hrit",
                "gk2a_uhrit",
            ])
            .added(1, 1, ADDED_1_1)
            .uses(sdrkit_codec_image::component::interface())
            // The labels of these options begin with a bracket, so they give no name.
            .option_name("rs", "255_223", "code_255_223")
            .option_name("rs", "255_239", "code_255_239")
            .shared_source("meteor", &["meteor_lrpt"])
            .shared_source("fy2", &["fy2_svissr"])
            .shared_source("fy3", &["fy3_ahrpt", "fy3_mpt"])
            .shared_source("apt", &["noaa_apt"])
            .shared_source("dsb", &["noaa_dsb"])
            .shared_source("gvar", &["goes_gvar"])
            .shared_source("gac", &["noaa_gac"])
            .shared_source("msg", &["msg_raw"])
            .shared_source("sband", &["sband_ccsds"])
            .stack(
                coded(
                    pictures::<MeteorLrptFields>("meteor_lrpt")
                        .table(&METEOR_LRPT_CHANNELS)
                        .table(&IMAGES),
                )
                .settings(MeteorSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(xrit("goes_hrit").settings(GoesSetting::SETTINGS))
            .stack(
                xrit("gk2a_lrit")
                    .settings(Gk2aSetting::SETTINGS)
                    .command(&IMPORT_KEYS)
                    .command(&ADD_KEYS),
            )
            .stack(
                xrit("gk2a_hrit")
                    .settings(Gk2aSetting::SETTINGS)
                    .command(&IMPORT_KEYS)
                    .command(&ADD_KEYS),
            )
            .stack(
                coded(pictures::<Fy2SvissrFields>("fy2_svissr").table(&IMAGES))
                    .settings(Fy2Setting::SETTINGS)
                    .command(&OPEN_SVISSR)
                    .command(&NEW_PICTURES)
                    .kept(&["images"]),
            )
            .stack(
                coded(
                    pictures::<Fy3Fields>("fy3_ahrpt")
                        .table(&FY3_INSTRUMENTS)
                        .table(&IMAGES),
                )
                .settings(AhrptSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                coded(
                    pictures::<Fy3Fields>("fy3_mpt")
                        .table(&FY3_INSTRUMENTS)
                        .table(&IMAGES),
                )
                .settings(MptSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                StackShape::new("noaa_apt")
                    .fields::<NoaaAptFields>()
                    .fields::<PictureFields>()
                    .table(&IMAGES)
                    .settings(AptSetting::SETTINGS)
                    .command(new_picture)
                    .kept(&["images"]),
            )
            .stack(
                coded(
                    StackShape::new("noaa_dsb")
                        .fields::<NoaaDsbFields>()
                        .fields::<PictureFields>()
                        .table(&IMAGES),
                )
                .settings(DsbSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                coded(
                    StackShape::new("goes_gvar")
                        .fields::<GoesGvarFields>()
                        .fields::<PictureFields>()
                        .table(&GOES_GVAR_BLOCKS)
                        .table(&IMAGES),
                )
                .settings(GvarSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                coded(
                    pictures::<AwsFields>("aws")
                        .table(&AWS_VIRTUAL_CHANNELS)
                        .table(&AWS_APIDS)
                        .table(&IMAGES),
                )
                .settings(AwsSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                coded(
                    StackShape::new("noaa_gac")
                        .fields::<NoaaGacFields>()
                        .fields::<PictureFields>()
                        .table(&IMAGES),
                )
                .settings(GacSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                coded(
                    pictures::<MsgRawFields>("msg_raw")
                        .table(&VIRTUAL_CHANNELS)
                        .table(&IMAGES),
                )
                .settings(MsgSetting::SETTINGS)
                .command(new_picture)
                .kept(&["images"]),
            )
            .stack(
                frame_table(StackShape::new("goes_cda").fields::<FrameTableFields>())
                    .settings(CdaSetting::SETTINGS)
                    .command(clear_table),
            )
            .stack(
                frame_table(StackShape::new("goes_mdl").fields::<FrameTableFields>())
                    .command(clear_table),
            )
            .stack(
                frame_table(StackShape::new("gk2a_cdas").fields::<FrameTableFields>())
                    .command(clear_table),
            )
            .stack(
                frame_table(
                    StackShape::new("sband_ccsds")
                        .fields::<FrameTableFields>()
                        .fields::<SbandCcsdsFields>(),
                )
                .settings(SbandSetting::SETTINGS)
                .command(clear_table),
            )
            .stack(
                frame_table(
                    StackShape::new("rdas")
                        .fields::<FrameTableFields>()
                        .fields::<RdasFields>()
                        .fields::<PictureFields>(),
                )
                .settings(RdasSetting::SETTINGS)
                .command(clear_table),
            )
            .uses(sdrkit_proto_dvbs2::typed::dvbs2_interface())
            .stack(uhrit)
            .uses(ipcast::interface())
            .stack(fy4("fy4_lrit", Fy4LritSetting::SETTINGS))
            .stack(fy4("fy4a_hrit", Fy4HritSetting::SETTINGS))
            .stack(grb)
            .stack(filecast)
            .stack(elektro())
            .stack(hrpt_stack(
                pictures::<HrptFields>("hrpt")
                    .table(&VIRTUAL_CHANNELS)
                    .table(&IMAGES),
                crate::modes::hrpt::HrptSetting::SETTINGS,
            ))
            .stack(
                frame_table(
                    StackShape::new("eodb")
                        .fields::<FrameTableFields>()
                        .fields::<EodbFields>()
                        .fields::<PictureFields>(),
                )
                .settings(crate::modes::eodb::EodbSetting::SETTINGS)
                .command(clear_table)
                .command(&OPEN_FRAMES)
                .command(&MAKE_PRODUCT),
            )
    })
}

/// What version 1.1 added: the GLM lightning results of `goes_grb`.
const ADDED_1_1: &[&str] = &[
    "goes_grb.fields.GlmFields",
    "goes_grb.table.flashes",
    "goes_grb.table.groups",
    "goes_grb.kept.flashes",
    "goes_grb.kept.groups",
    "shape.GlmFields",
    "shape.GlmFields.glm_payloads",
    "shape.GlmFields.glm_dropped",
    "shape.GlmFields.glm_packets_lost",
    "shape.GlmFields.glm_not_decoded",
    "shape.GlmFields.glm_bad_length",
    "shape.GlmFields.glm_group_bad_length",
    "shape.GlmFields.flash_count",
    "shape.GlmFields.group_count",
    "shape.GlmFields.event_count",
    "shape.GlmFields.glm_rejected",
    "shape.GlmFields.newest_flash",
    "shape.GlmFields.flash_rate",
    "shape.GlmFlashRow",
    "shape.GlmFlashRow.flash",
    "shape.GlmFlashRow.time",
    "shape.GlmFlashRow.duration",
    "shape.GlmFlashRow.position",
    "shape.GlmFlashRow.area_km2",
    "shape.GlmFlashRow.energy_fj",
    "shape.GlmFlashRow.quality",
    "shape.GlmFlashRow.groups",
    "shape.GlmFlashRow.events",
    "shape.GlmGroupRow",
    "shape.GlmGroupRow.group",
    "shape.GlmGroupRow.time",
    "shape.GlmGroupRow.position",
    "shape.GlmGroupRow.area_km2",
    "shape.GlmGroupRow.energy_fj",
    "shape.GlmGroupRow.flash",
    "shape.GlmGroupRow.quality",
    "shape.GlmGroupRow.events",
];

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the satimg family does not describe the stack {id}"))
}
