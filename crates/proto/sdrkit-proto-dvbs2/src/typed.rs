//! The shared interface `dvbs2`: the result set, the tables and the plots of the DVB-S/S2 receiver that [`crate::attach`]
//! adds to the graph of a stack.

use crate::modcod::Modulation;
use crate::sync::Received;
use crate::{Carries, CodeRate, FrameSize, RollOffSource, SignalNote};
use sdrkit_core::interface::{Family, Fields, Hertz, StackShape, Table};
use sdrkit_core::publish::{keys, SeriesDef};
use std::sync::OnceLock;

/// The result fields of the DVB-S/S2 receiver: the carrier search, the frames, the code and the locks. A measurement of
/// the carrier search has no value without a carrier.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbs2Fields {
    /// The standard received.
    #[field(label = "Standard")]
    pub standard: Option<Received>,
    /// A note on the signal for the user.
    #[field(label = "Note")]
    pub signal_note: Option<SignalNote>,
    /// The symbol rate in use. No value before a rate is set or measured.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The symbol rate from the spectrum (width or cyclic line).
    #[field(label = "Symbol rate from the spectrum", unit = "Hz")]
    pub spectrum_rate: Option<Hertz>,
    /// The error of the symbol clock.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: f64,
    /// The symbol rate was set by hand.
    #[field(label = "Manual symbol rate")]
    pub manual_rate: bool,
    /// The plateau of the carrier over the noise floor.
    #[field(label = "Carrier over floor", unit = "dB")]
    pub spectrum_snr_db: Option<f64>,
    /// The misfit of the raised cosine fit.
    #[field(label = "Fit misfit")]
    pub misfit: Option<f64>,
    /// The lower -3 dB edge of the carrier, from the centre.
    #[field(label = "Lower carrier edge", unit = "Hz")]
    pub edge_low: Option<Hertz>,
    /// The upper -3 dB edge of the carrier, from the centre.
    #[field(label = "Upper carrier edge", unit = "Hz")]
    pub edge_high: Option<Hertz>,
    /// The carrier offset from the centre. No value without a carrier.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The spectrum is inverted.
    #[field(label = "Spectrum inverted")]
    pub inverted: bool,
    /// The roll-off.
    #[field(label = "Roll-off")]
    pub roll_off: f64,
    /// Where the roll-off came from.
    #[field(label = "Roll-off from")]
    pub roll_off_source: RollOffSource,
    /// The modulation.
    #[field(label = "Modulation")]
    pub modulation: Option<Modulation>,
    /// The code rate.
    #[field(label = "Code rate")]
    pub code_rate: Option<CodeRate>,
    /// The MODCOD (DVB-S2).
    #[field(label = "MODCOD")]
    pub modcod: Option<i64>,
    /// The FEC frame size (DVB-S2).
    #[field(label = "FEC frame")]
    pub frame: Option<FrameSize>,
    /// Pilots are on.
    #[field(label = "Pilots")]
    pub pilots: bool,
    /// The MODCOD changes between frames.
    #[field(label = "VCM or ACM")]
    pub vcm: bool,
    /// The carrier has one input stream.
    #[field(label = "Single stream")]
    pub single_stream: bool,
    /// The PL scrambling code in use.
    #[field(label = "PL scrambling code")]
    pub pl_scrambling_code: i64,
    /// PLFRAMEs seen.
    #[field(label = "PLFRAMEs")]
    pub plframes: i64,
    /// Dummy PLFRAMEs seen.
    #[field(label = "Dummy frames")]
    pub dummy_frames: i64,
    /// Frames followed but not decoded.
    #[field(label = "Frames not decoded")]
    pub unsupported_frames: i64,
    /// Frames skipped by the MODCOD rule or the frame length rule.
    #[field(label = "Frames of another MODCOD")]
    pub other_modcod_frames: i64,
    /// Frames whose pilots differ from the setting.
    #[field(label = "Frames with other pilots")]
    pub pilot_mismatch: i64,
    /// BBHEADERs refused (CRC-8 or length).
    #[field(label = "BBHEADER failures")]
    pub bbheader_bad: i64,
    /// The net bit rate of the data fields.
    #[field(label = "Net bit rate", unit = "bit/s")]
    pub net_bit_rate: f64,
    /// Es/N0.
    #[field(label = "Es/N0", unit = "dB")]
    pub esn0_db: Option<f64>,
    /// The modulation error ratio.
    #[field(shared = keys::MER_DB)]
    pub mer_db: Option<f64>,
    /// Eb/N0.
    #[field(label = "Eb/N0", unit = "dB")]
    pub ebn0_db: Option<f64>,
    /// Es/N0 above the quasi-error-free level of EN 302 307-1 table 13.
    #[field(label = "Margin to QEF", unit = "dB")]
    pub margin_to_qef_db: Option<f64>,
    /// LDPC iterations, recent mean.
    #[field(label = "LDPC iterations")]
    pub ldpc_iterations: f64,
    /// BCH words good.
    #[field(label = "BCH good")]
    pub bch_ok: i64,
    /// BCH words that failed.
    #[field(label = "BCH failed")]
    pub bch_bad: i64,
    /// Reed-Solomon words with no error (DVB-S).
    #[field(label = "RS words clean")]
    pub rs_clean: i64,
    /// Reed-Solomon words corrected (DVB-S).
    #[field(label = "RS words corrected")]
    pub rs_corrected: i64,
    /// Reed-Solomon words that failed (DVB-S).
    #[field(label = "RS words failed")]
    pub rs_failed: i64,
    /// A carrier is found.
    #[field(label = "Carrier lock")]
    pub carrier_lock: bool,
    /// The symbol timing runs.
    #[field(label = "Timing lock")]
    pub timing_lock: bool,
    /// The carrier phase and the framing hold.
    #[field(label = "Frame lock")]
    pub frame_lock: bool,
    /// The FEC decodes words.
    #[field(label = "FEC lock")]
    pub fec_lock: bool,
    /// The transport stream is valid.
    #[field(label = "TS lock")]
    pub ts_lock: bool,
    /// The level of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The LNB supply selects the other polarisation than the transponder's.
    #[field(label = "LNB on the other polarisation")]
    pub lnb_mismatch: bool,
}

/// A row of the carriers of the survey. The row key is the offset in kHz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbs2CarrierRow {
    /// The offset of the carrier from the centre.
    #[field(label = "Offset", unit = "Hz")]
    pub offset: Hertz,
    /// Its symbol rate.
    #[field(label = "Symbol rate", unit = "Hz")]
    pub symbol_rate: Hertz,
    /// Its roll-off.
    #[field(label = "Roll-off")]
    pub roll_off: f64,
    /// Its plateau over the floor.
    #[field(label = "SNR", unit = "dB")]
    pub snr_db: f64,
    /// Its level against the strongest.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// It is the carrier decoded.
    #[field(label = "Decoded")]
    pub decoded: bool,
}

/// A row of the input streams. The row key is the ISI.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbs2StreamRow {
    /// The input stream identifier.
    #[field(label = "ISI")]
    pub isi: i64,
    /// What the stream carries.
    #[field(label = "Carries")]
    pub carries: Carries,
    /// The user packet length. No value for a stream without user packets.
    #[field(label = "Packet length", unit = "bytes")]
    pub packet_bytes: Option<i64>,
    /// The sync byte of the user packets.
    #[field(label = "Sync byte")]
    pub sync_byte: Option<i64>,
    /// Constant coding and modulation.
    #[field(label = "CCM")]
    pub ccm: bool,
    /// Frames of the stream.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Data field bits of the stream.
    #[field(label = "Bits")]
    pub bits: i64,
}

/// A row of the MODCODs of the input streams. The row key is `<isi>/<modcod>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Dvbs2StreamModcodRow<'a> {
    /// The input stream, a row key of the stream table.
    #[field(label = "Input stream", refers_to = "streams")]
    pub stream: &'a str,
    /// The MODCOD.
    #[field(label = "MODCOD")]
    pub modcod: i64,
    /// The modulation of the MODCOD.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The code rate of the MODCOD.
    #[field(label = "Code rate")]
    pub code_rate: Option<CodeRate>,
    /// The frames of the stream with this MODCOD.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The share of the stream's frames with this MODCOD.
    #[field(label = "Share", unit = "%")]
    pub share_pct: f64,
}

/// The carriers of the survey.
pub static DVBS2_CARRIERS: Table<Dvbs2CarrierRow> = Table::new("carriers", "Carriers", 32);
/// The input streams.
pub static DVBS2_STREAMS: Table<Dvbs2StreamRow> = Table::new("streams", "Input streams", 256);
/// The MODCODs of each input stream.
pub static DVBS2_STREAM_MODCODS: Table<Dvbs2StreamModcodRow<'static>> =
    Table::new("stream_modcods", "MODCODs of the input streams", 1024);

/// The plots of the DVB-S/S2 receiver, in order.
pub const DVBS2_SERIES: [SeriesDef; 3] = [
    SeriesDef {
        key: "constellation",
        label: "Constellation",
        x_unit: "",
        y_unit: "",
    },
    SeriesDef {
        key: "spectrum",
        label: "Spectrum",
        x_unit: "bin",
        y_unit: "dB",
    },
    SeriesDef {
        key: "mer_history",
        label: "MER history",
        x_unit: "frame",
        y_unit: "dB",
    },
];

/// `shape` with the results of the DVB-S/S2 receiver after the items it has: the result set [`Dvbs2Fields`], the
/// tables [`DVBS2_CARRIERS`], [`DVBS2_STREAMS`] and [`DVBS2_STREAM_MODCODS`], and the series [`DVBS2_SERIES`]. A stack
/// whose graph holds the receiver of [`crate::attach`] calls it first and adds its own items after.
pub fn with_dvbs2_receiver(shape: StackShape) -> StackShape {
    let mut s = shape
        .fields::<Dvbs2Fields>()
        .table(&DVBS2_CARRIERS)
        .table(&DVBS2_STREAMS)
        .table(&DVBS2_STREAM_MODCODS);
    for series in DVBS2_SERIES {
        s = s.series(series);
    }
    s
}

/// The shared interface `dvbs2`: the result set and the tables of the DVB-S/S2 receiver, which the stacks of several
/// families show. A family whose stacks list them names it with `Family::uses`.
pub fn dvbs2_interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("dvbs2", 1, 0).stack(
            StackShape::new("dvbs2")
                .fields::<Dvbs2Fields>()
                .table(&DVBS2_CARRIERS)
                .table(&DVBS2_STREAMS)
                .table(&DVBS2_STREAM_MODCODS),
        )
    })
}
