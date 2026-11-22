//! What the DRM receiver publishes: the shapes of its result set, its table rows and its events, its tables and its
//! series.

use super::params::Robustness;
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, Table};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;

sdrkit_core::choice_enum! {
    /// What the receiver does with the audio of the service playing.
    pub enum AudioState {
        /// The service has no audio stream yet.
        NoAudioStream = ("no_audio_stream", "No audio stream"),
        /// The audio goes to the host's decoder.
        NoDecoder = ("no_decoder", "Decoded by the host"),
        /// The audio is decoded.
        Decoding = ("decoding", "Decoding"),
    }
}

/// The results of the DRM stack. No FAC values before the first FAC, no protection before the multiplex
/// description, no time before SDC type 8, no measurement before the first frame and no first audio before it came.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmFields<'a> {
    /// The robustness mode, A to E.
    #[field(label = "Robustness mode")]
    pub robustness_mode: Option<Robustness>,
    /// The spectrum occupancy code.
    #[field(label = "Occupancy")]
    pub occupancy: Option<i64>,
    /// The channel bandwidth.
    #[field(label = "Bandwidth", unit = "Hz")]
    pub bandwidth: Option<Hertz>,
    /// The interleaver as text.
    #[field(label = "Interleaver")]
    pub interleaver: Option<&'a str>,
    /// The MSC mapping, with the hierarchical flag.
    #[field(label = "MSC mapping")]
    pub msc: Option<&'a str>,
    /// The SDC mapping.
    #[field(label = "SDC mapping")]
    pub sdc: Option<&'a str>,
    /// The protection levels and their code rates.
    #[field(label = "Protection")]
    pub protection: Option<&'a str>,
    /// The protection level of part B.
    #[field(label = "Protection B")]
    pub protection_b: Option<i64>,
    /// The time of SDC type 8 as text.
    #[field(label = "Time")]
    pub time: Option<&'a str>,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The sample clock error.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The delay of the strongest path from the symbol start.
    #[field(label = "Strongest path", unit = "ms")]
    pub delay_ms: Option<f64>,
    /// The guard correlation at the symbol timing.
    #[field(label = "Guard correlation")]
    pub guard_correlation: Option<f64>,
    /// The quality, 0 to 1.
    #[field(label = "Quality")]
    pub quality: Option<f64>,
    /// FAC blocks good.
    #[field(label = "FAC good")]
    pub fac_ok: i64,
    /// FAC blocks bad.
    #[field(label = "FAC bad")]
    pub fac_bad: i64,
    /// SDC blocks good.
    #[field(label = "SDC good")]
    pub sdc_ok: i64,
    /// SDC blocks bad.
    #[field(label = "SDC bad")]
    pub sdc_bad: i64,
    /// Multiplex frames good.
    #[field(label = "Multiplex frames good")]
    pub mux_ok: i64,
    /// Multiplex frames bad.
    #[field(label = "Multiplex frames bad")]
    pub mux_bad: i64,
    /// Audio frames good.
    #[field(label = "Audio frames good")]
    pub audio_ok: i64,
    /// Audio frames bad.
    #[field(label = "Audio frames bad")]
    pub audio_bad: i64,
    /// Fades held.
    #[field(label = "Fades held")]
    pub fades: i64,
    /// Re-synchronisations.
    #[field(label = "Re-synchronisations")]
    pub resyncs: i64,
    /// The audio state.
    #[field(label = "Audio")]
    pub audio_state: AudioState,
    /// What plays, or why nothing can.
    #[field(label = "Audio")]
    pub audio_info: &'a str,
    /// Text segments good.
    #[field(label = "Text segments good")]
    pub text_ok: i64,
    /// Text segments bad.
    #[field(label = "Text segments bad")]
    pub text_bad: i64,
    /// Seconds of signal.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// When the first audio frame came.
    #[field(label = "First audio", unit = "s")]
    pub first_audio: Option<Seconds>,
    /// Holding through a fade.
    #[field(label = "Holding through a fade")]
    pub holding: bool,
    /// The input is mirrored (I and Q swapped).
    #[field(label = "Mirrored")]
    pub mirrored: bool,
}

/// A service. The row key is its short id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmServiceRow<'a> {
    /// The short id.
    #[field(label = "Short id")]
    pub short_id: i64,
    /// The 24-bit service id, six hex digits.
    #[field(label = "Service id")]
    pub service_id: &'a str,
    /// "audio" or "data".
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The label, once the SDC gave it.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The language.
    #[field(label = "Language")]
    pub language: &'a str,
    /// The country, once the SDC gave it.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The programme type or application id.
    #[field(label = "Programme")]
    pub programme: &'a str,
    /// Conditional access is flagged.
    #[field(label = "Conditional access")]
    pub ca: bool,
    /// The audio coding as text, once the SDC gave it.
    #[field(label = "Codec")]
    pub codec: Option<&'a str>,
    /// The core sampling rate, once the SDC gave it.
    #[field(label = "Core rate", unit = "Hz")]
    pub core_rate: Option<Hertz>,
    /// The audio mode code.
    #[field(label = "Audio mode")]
    pub audio_mode: i64,
    /// SBR.
    #[field(label = "SBR")]
    pub sbr: bool,
    /// Text messages carried.
    #[field(label = "Text message")]
    pub text: bool,
    /// The stream id.
    #[field(label = "Stream")]
    pub stream: i64,
    /// The net bit rate of the stream.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_bps: f64,
    /// The application name of a data service.
    #[field(label = "Application")]
    pub application: Option<&'a str>,
    /// The mode text of a data service.
    #[field(label = "Mode")]
    pub mode_text: Option<&'a str>,
    /// Whether the service is decoded.
    #[field(label = "Decoded")]
    pub decoded: bool,
}

/// The bytes per logical frame of a stream. The row key is the stream number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmStreamRow {
    /// The bytes of part A.
    #[field(label = "Part A", unit = "bytes")]
    pub part_a: i64,
    /// The bytes of part B.
    #[field(label = "Part B", unit = "bytes")]
    pub part_b: i64,
}

/// A MOT object of a data service. The row key is `<short id>:<transport id>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmObjectRow<'a> {
    /// The short id of the service.
    #[field(label = "Short id")]
    pub short_id: i64,
    /// The MOT transport id.
    #[field(label = "Transport id")]
    pub transport_id: i64,
    /// The name, or the transport id for an object without one.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The content type.
    #[field(label = "Type")]
    pub content_type: &'a str,
    /// The size in bytes.
    #[field(label = "Size")]
    pub size: i64,
    /// Times received.
    #[field(label = "Received")]
    pub received: i64,
    /// The order of arrival.
    #[field(label = "Order")]
    pub order: i64,
    /// What the object is, where the stack can say.
    #[field(label = "Note")]
    pub note: Option<&'a str>,
}

/// The latest slide of a service. The row key is the short id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmSlideRow<'a> {
    /// The short id of the service.
    #[field(label = "Short id")]
    pub short_id: i64,
    /// The order number of the slide.
    #[field(label = "Order")]
    pub order: i64,
    /// The name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The category title.
    #[field(label = "Category")]
    pub category: Option<&'a str>,
    /// The category id.
    #[field(label = "Category id")]
    pub category_id: Option<i64>,
    /// The slide id.
    #[field(label = "Slide id")]
    pub slide_id: Option<i64>,
    /// The click-through URL.
    #[field(label = "Click URL")]
    pub click_url: Option<&'a str>,
    /// The alternative location URL.
    #[field(label = "Alternative URL")]
    pub alt_url: Option<&'a str>,
    /// The trigger time as text.
    #[field(label = "Trigger")]
    pub trigger: Option<&'a str>,
    /// The content type.
    #[field(label = "Type")]
    pub content_type: &'a str,
    /// The size in bytes.
    #[field(label = "Size")]
    pub size: i64,
    /// The width of the picture.
    #[field(label = "Width")]
    pub width: i64,
    /// The height of the picture.
    #[field(label = "Height")]
    pub height: i64,
}

/// A Journaline object. The row key is its id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmPageRow<'a> {
    /// The object id.
    #[field(label = "Id")]
    pub id: i64,
    /// The object type.
    #[field(label = "Type")]
    pub kind: &'a str,
    /// The title.
    #[field(label = "Title")]
    pub title: &'a str,
    /// The content: links (id and text), body or list items, one per line.
    #[field(label = "Content")]
    pub content: &'a str,
}

/// The packet counters of a packet-mode service. The row key is its short id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmPacketRow {
    /// The short id of the service.
    #[field(label = "Short id")]
    pub short_id: i64,
    /// Packets.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Packets with a bad CRC.
    #[field(label = "Bad CRC")]
    pub bad_crc: i64,
    /// Packets lost.
    #[field(label = "Lost")]
    pub lost: i64,
    /// Continuation packets without their first.
    #[field(label = "Orphans")]
    pub orphans: i64,
    /// Padding packets.
    #[field(label = "Empty")]
    pub empty: i64,
    /// Packets of another packet id.
    #[field(label = "Other packet id")]
    pub other_id: i64,
    /// Data units complete.
    #[field(label = "Data units good")]
    pub units_ok: i64,
    /// Data units dropped.
    #[field(label = "Data units dropped")]
    pub units_dropped: i64,
    /// Bytes of the data units.
    #[field(label = "Unit bytes")]
    pub unit_bytes: i64,
    /// TPEG frames counted.
    #[field(label = "TPEG frames")]
    pub tpeg_frames: i64,
}

/// A note on what the receiver cannot decode. The row key is its order.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmNoteRow<'a> {
    /// The note.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The services.
pub static SERVICES: Table<DrmServiceRow<'static>> =
    Table::new("services", "Services", 4).label(&DrmServiceRow::LABEL);
/// The bytes per logical frame of each stream.
pub static STREAMS: Table<DrmStreamRow> = Table::new("streams", "Streams", 4);
/// The objects of every data service.
pub static OBJECTS: Table<DrmObjectRow<'static>> =
    Table::new("objects", "Objects", 96).label(&DrmObjectRow::NAME);
/// The latest slide of each service.
pub static SLIDES: Table<DrmSlideRow<'static>> =
    Table::new("slides", "Slides", 4).label(&DrmSlideRow::NAME);
/// The Journaline objects.
pub static PAGES: Table<DrmPageRow<'static>> =
    Table::new("journaline", "Journaline", 512).label(&DrmPageRow::TITLE);
/// The packet counters of each packet-mode service.
pub static PACKET_COUNTERS: Table<DrmPacketRow> = Table::new("packets", "Packets", 4);
/// The notes on what the receiver cannot decode.
pub static NOTES: Table<DrmNoteRow<'static>> = Table::new("notes", "Notes", 16);

/// The fields of the event of a new text message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmTextEvent<'a> {
    /// The text message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a received MOT object.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DrmObjectEvent<'a> {
    /// The MOT transport id.
    #[field(label = "Transport id")]
    pub transport_id: i64,
    /// The name of the object.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The content type.
    #[field(label = "Type")]
    pub content_type: &'a str,
    /// The size in bytes.
    #[field(label = "Size")]
    pub size: i64,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The text message of the service playing changed.
pub static TEXT_CHANGED: EventKind<DrmTextEvent<'static>> =
    EventKind::new("drm.text", "Text message");
/// A MOT object of a data service was received.
pub static OBJECT_RECEIVED: EventKind<DrmObjectEvent<'static>> =
    EventKind::new("drm.object", "Object received");

/// The channel magnitude in dB over the occupied carriers.
pub const SERIES_CHANNEL: SeriesDef = SeriesDef {
    key: "channel",
    label: "Channel",
    x_unit: "carrier",
    y_unit: "dB",
};
/// The impulse response in dB, 256 points, ms.
pub const SERIES_IMPULSE: SeriesDef = SeriesDef {
    key: "impulse_response",
    label: "Impulse response",
    x_unit: "ms",
    y_unit: "dB",
};
/// The FAC constellation.
pub const SERIES_FAC: SeriesDef = SeriesDef {
    key: "fac_constellation",
    label: "FAC",
    x_unit: "",
    y_unit: "",
};
/// The SDC constellation.
pub const SERIES_SDC: SeriesDef = SeriesDef {
    key: "sdc_constellation",
    label: "SDC",
    x_unit: "",
    y_unit: "",
};
/// The MSC constellation.
pub const SERIES_MSC: SeriesDef = SeriesDef {
    key: "msc_constellation",
    label: "MSC",
    x_unit: "",
    y_unit: "",
};

/// The id of the `services` table in the results of the stack.
pub fn services_table() -> sdrkit_core::TableId {
    static ID: std::sync::OnceLock<sdrkit_core::TableId> = std::sync::OnceLock::new();
    *ID.get_or_init(|| {
        crate::typed::shape(super::ID)
            .results_decl()
            .table_id(SERVICES.key)
            .unwrap_or(sdrkit_core::TableId(0))
    })
}
