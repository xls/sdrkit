//! What the HD Radio receiver publishes: the shapes of its result set, its table rows and the song event, its tables
//! and the constellation.

use super::l1::ServiceMode;
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, Table};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::TableId;
use std::sync::OnceLock;

sdrkit_core::choice_enum! {
    /// The kind of HD Radio signal.
    pub enum HdBand {
        /// FM hybrid.
        FmHybrid = ("fm_hybrid", "FM hybrid"),
        /// AM hybrid.
        AmHybrid = ("am_hybrid", "AM hybrid"),
        /// AM all-digital.
        AmAllDigital = ("am_all_digital", "AM all-digital"),
    }
}

/// The results of the HD Radio stack. No band or mode before the first frame, no measurement while the receiver
/// searches and no station data before the PIDS carry it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HdradioFields<'a> {
    /// The kind of signal.
    #[field(label = "Band")]
    pub band: Option<HdBand>,
    /// The service mode.
    #[field(label = "Service mode")]
    pub service_mode: Option<ServiceMode>,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The MER of the lower sideband (AM: the primary).
    #[field(label = "MER lower", unit = "dB")]
    pub mer_lower_db: Option<f64>,
    /// The MER of the upper sideband (AM: the secondary).
    #[field(label = "MER upper", unit = "dB")]
    pub mer_upper_db: Option<f64>,
    /// The bit error rate of the P1 channel.
    #[field(shared = keys::BER)]
    pub ber: Option<f64>,
    /// Blocks.
    #[field(label = "Blocks")]
    pub blocks: i64,
    /// Block syncs.
    #[field(label = "Block syncs")]
    pub block_syncs: i64,
    /// P1 frames good.
    #[field(label = "P1 good")]
    pub p1_ok: i64,
    /// P1 frames bad.
    #[field(label = "P1 bad")]
    pub p1_bad: i64,
    /// P3 frames good.
    #[field(label = "P3 good")]
    pub p3_ok: i64,
    /// P3 frames bad.
    #[field(label = "P3 bad")]
    pub p3_bad: i64,
    /// PIDS frames good.
    #[field(label = "PIDS good")]
    pub pids_ok: i64,
    /// PIDS frames bad.
    #[field(label = "PIDS bad")]
    pub pids_bad: i64,
    /// Audio PDUs.
    #[field(label = "Audio PDUs")]
    pub pdus: i64,
    /// AAS packets good.
    #[field(label = "AAS good")]
    pub aas_ok: i64,
    /// AAS packets bad.
    #[field(label = "AAS bad")]
    pub aas_bad: i64,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// What the audio codec needs, or why it cannot play.
    #[field(label = "Codec")]
    pub codec_note: Option<&'a str>,
    /// P1 frames arrive.
    #[field(label = "P1")]
    pub p1_on: bool,
    /// PIDS frames arrive.
    #[field(label = "PIDS")]
    pub pids_on: bool,
    /// Song data seen.
    #[field(label = "PSD")]
    pub psd_on: bool,
    /// Data seen.
    #[field(label = "Data")]
    pub data_on: bool,
    /// The row key of the program to hear in [`PROGRAMS`].
    #[field(label = "Program", refers_to = "programs")]
    pub program: &'a str,
    /// The call sign.
    #[field(label = "Call sign")]
    pub call_sign: Option<&'a str>,
    /// The universal name.
    #[field(label = "Universal name")]
    pub universal_name: Option<&'a str>,
    /// The long name.
    #[field(label = "Long name")]
    pub long_name: Option<&'a str>,
    /// The slogan.
    #[field(label = "Slogan")]
    pub slogan: Option<&'a str>,
    /// The station message.
    #[field(label = "Message")]
    pub message: Option<&'a str>,
    /// The country.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The facility id.
    #[field(label = "Facility id")]
    pub facility_id: Option<i64>,
    /// The location of the station.
    #[field(label = "Location", unit = "deg")]
    pub location: Option<(f64, f64)>,
    /// The altitude of the station.
    #[field(label = "Altitude", unit = "m")]
    pub altitude_m: Option<f64>,
}

/// An audio program. The row key is "HD1" to "HD8".
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HdradioProgramRow<'a> {
    /// The program number, 0 for HD1.
    #[field(label = "Number")]
    pub number: i64,
    /// "HD1" to "HD8".
    #[field(label = "Program")]
    pub label: &'a str,
    /// The name from the SIG.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The program type code.
    #[field(label = "Type")]
    pub program_type: i64,
    /// The name of the program type.
    #[field(label = "Type name")]
    pub type_name: Option<&'a str>,
    /// Restricted access.
    #[field(label = "Restricted")]
    pub restricted: bool,
    /// The codec mode.
    #[field(label = "Codec mode")]
    pub codec_mode: Option<i64>,
    /// The blend control.
    #[field(label = "Blend")]
    pub blend: Option<i64>,
    /// The sound experience code.
    #[field(label = "Sound experience")]
    pub sound_experience: i64,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_bps: f64,
    /// Packets good.
    #[field(label = "Packets good")]
    pub packets_ok: i64,
    /// Packets bad.
    #[field(label = "Packets bad")]
    pub packets_bad: i64,
    /// Listed in the SIS.
    #[field(label = "In SIS")]
    pub in_sis: bool,
    /// Listed in the SIG.
    #[field(label = "In SIG")]
    pub in_sig: bool,
    /// On air.
    #[field(label = "On air")]
    pub broadcasting: bool,
    /// Song data messages.
    #[field(label = "PSD messages")]
    pub psd_count: i64,
    /// The song title.
    #[field(label = "Title")]
    pub title: Option<&'a str>,
    /// The artist.
    #[field(label = "Artist")]
    pub artist: Option<&'a str>,
    /// The album.
    #[field(label = "Album")]
    pub album: Option<&'a str>,
    /// The genre.
    #[field(label = "Genre")]
    pub genre: Option<&'a str>,
    /// The comment.
    #[field(label = "Comment")]
    pub comment: Option<&'a str>,
    /// The unique file identifier.
    #[field(label = "UFID")]
    pub ufid: Option<&'a str>,
    /// The commercial frame, its parts separated by "; ".
    #[field(label = "Commercial")]
    pub commercial: Option<&'a str>,
    /// The LOT number of the album art.
    #[field(label = "Album art")]
    pub art_lot: Option<i64>,
    /// The type of the album art.
    #[field(label = "Album art type")]
    pub art_mime: Option<&'a str>,
    /// The port of the station logo.
    #[field(label = "Logo port")]
    pub logo_port: Option<i64>,
    /// The port of the album art.
    #[field(label = "Art port")]
    pub art_port: Option<i64>,
}

/// A data service component. The row key is its port.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HdradioDataServiceRow<'a> {
    /// The name of the service.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The service number.
    #[field(label = "Service")]
    pub service: i64,
    /// The port.
    #[field(label = "Port")]
    pub port: i64,
    /// The component type.
    #[field(label = "Type")]
    pub kind: i64,
    /// The content type.
    #[field(label = "MIME")]
    pub mime: &'a str,
    /// Packets.
    #[field(label = "Packets")]
    pub packets: i64,
    /// Bytes.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// A file of the LOT transport. The row key is `<port>:<lot>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HdradioFileRow<'a> {
    /// The port.
    #[field(label = "Port")]
    pub port: i64,
    /// The LOT number.
    #[field(label = "LOT")]
    pub lot: i64,
    /// The file name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The content type.
    #[field(label = "MIME")]
    pub mime: &'a str,
    /// The size in bytes.
    #[field(label = "Size")]
    pub size: i64,
    /// Bytes received.
    #[field(label = "Received")]
    pub received: i64,
    /// The file is complete.
    #[field(label = "Complete")]
    pub complete: bool,
    /// The version.
    #[field(label = "Version")]
    pub version: i64,
    /// The expiry date.
    #[field(label = "Expires", unit = "MJD")]
    pub expiry_mjd: i64,
    /// When it last changed, in signal time.
    #[field(label = "Updated", unit = "s")]
    pub updated: Seconds,
}

/// The fields of the song event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HdradioSongEvent<'a> {
    /// The program ("HD1").
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The song title.
    #[field(label = "Title")]
    pub title: &'a str,
    /// The artist.
    #[field(label = "Artist")]
    pub artist: &'a str,
    /// "artist - title", or the one of them that is there.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The audio programs.
pub static PROGRAMS: Table<HdradioProgramRow<'static>> = Table::new("programs", "Programs", 8);
/// The data service components.
pub static DATA_SERVICES: Table<HdradioDataServiceRow<'static>> =
    Table::new("data_services", "Data services", 64);
/// The files of the LOT transport, newest first.
pub static FILES: Table<HdradioFileRow<'static>> = Table::new("files", "Files", 64);

/// The constellation of the last frame.
pub const SERIES_CONSTELLATION: SeriesDef = SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
};

/// The song data of a program changed.
pub static SONG: EventKind<HdradioSongEvent<'static>> = EventKind::new("hdr.psd", "Song");

/// The id of the `programs` table in the results of the stack.
pub fn programs_table() -> TableId {
    static ID: OnceLock<TableId> = OnceLock::new();
    *ID.get_or_init(|| {
        crate::typed::shape(super::ID)
            .results_decl()
            .table_id(PROGRAMS.key)
            .unwrap_or(TableId(0))
    })
}
