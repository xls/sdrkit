//! The interface of the DAB family: the shapes of the result sets, the table rows and the events, the tables and event
//! kinds, and the description of the family ([`family`]).
//!
//! The report component writes every result set and table of the stack except the playout counts. The radio text, the
//! slide shown, the text tags, the guide, the slide categories and the logos are the shapes of the shared interface
//! of `sdrkit-proto-bcdata`, which the broadcast data publisher writes. The playout component of that crate writes the
//! playout counts.

use crate::health::{Light, Quality};
use crate::results::{AudioCodec, Detection, JournalineKind, Playing, ServiceKind};
use crate::settings::DabSetting;
use sdrkit_core::interface::{
    EventKind, Family as Described, Fields, Hertz, Seconds, StackShape, Table, Time,
};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::SettingSet;
use sdrkit_proto_bcdata::component::PlayoutFields;
use sdrkit_proto_bcdata::publish as bc;
use sdrkit_proto_mpegts::publish::SERVICE_SELECT_KIND;
use std::sync::OnceLock;

// The result sets.

/// The measurements of the signal. No SNR before the lock, and no offset and clock error while the receiver searches.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SignalFields {
    /// The SNR measure.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset, without the centring shift.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The sample clock error.
    #[field(shared = keys::CLOCK_PPM)]
    pub clock_ppm: Option<f64>,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The ensemble. No value for an item that the ensemble has not signalled yet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EnsembleFields<'a> {
    /// The ensemble label.
    #[field(label = "Ensemble")]
    pub ensemble_label: Option<&'a str>,
    /// The ensemble short label.
    #[field(label = "Ensemble short label")]
    pub ensemble_short_label: Option<&'a str>,
    /// The ensemble identifier (EId).
    #[field(label = "EId")]
    pub eid: Option<i64>,
    /// The extended country code (FIG 0/9).
    #[field(label = "Extended country code")]
    pub ecc: Option<i64>,
    /// The country id (top 4 bits of the EId).
    #[field(label = "Country id")]
    pub country: Option<i64>,
    /// The ensemble time.
    #[field(label = "Ensemble time")]
    pub utc: Option<Time>,
    /// The local time offset (FIG 0/9).
    #[field(label = "Local time offset", unit = "s")]
    pub local_time_offset: Option<Seconds>,
    /// Every label and the service organisation are received.
    #[field(label = "Ensemble complete")]
    pub ensemble_valid: bool,
    /// Services in the ensemble.
    #[field(label = "Services")]
    pub service_count: i64,
}

/// The synchronisation, the FIC and the front end.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SyncFields {
    /// The last synchronisation symbol (PRS) was found.
    #[field(label = "Frame sync found")]
    pub sync: bool,
    /// The PRS correlation peak over its median. No value while the receiver searches.
    #[field(label = "Sync symbol peak")]
    pub prs_peak: Option<f64>,
    /// Frames demodulated.
    #[field(label = "Frames")]
    pub frames: i64,
    /// FIBs with a good CRC.
    #[field(label = "FIBs good")]
    pub fib_ok: i64,
    /// FIBs with a bad CRC.
    #[field(label = "FIBs bad")]
    pub fib_bad: i64,
    /// Good FIBs in the last frame.
    #[field(label = "FIBs good in the last frame")]
    pub fib_ok_last_frame: i64,
    /// The share of the FIC that carries FIGs.
    #[field(label = "FIC fill", unit = "%")]
    pub fic_fill_pct: f64,
    /// The quiet time of the null symbol.
    #[field(label = "Quiet time", unit = "s")]
    pub quiet: Seconds,
    /// Sync losses.
    #[field(label = "Sync losses")]
    pub sync_losses: i64,
    /// Relocks.
    #[field(label = "Relocks")]
    pub relocks: i64,
    /// CIFs erased after lost samples.
    #[field(label = "Erased CIFs")]
    pub erased_cifs: i64,
    /// Impulses blanked.
    #[field(label = "Impulses blanked")]
    pub impulses_blanked: i64,
    /// The spectrum was mirrored.
    #[field(label = "Spectrum mirrored")]
    pub mirrored: bool,
    /// Signal time.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The detection in use. No value while the receiver searches.
    #[field(label = "Detection")]
    pub detection: Option<Detection>,
}

/// The service playing. No value while no service plays.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PlayingFields<'a> {
    /// The row key of the service playing in [`SERVICES`].
    #[field(label = "Service playing", refers_to = "services")]
    pub selected_service: Option<&'a str>,
    /// The sub-channel playing.
    #[field(label = "Sub-channel playing")]
    pub selected_subchannel: Option<i64>,
    /// The label of the service playing.
    #[field(label = "Service playing")]
    pub selected_label: Option<&'a str>,
    /// The kind of the service playing.
    #[field(label = "Kind playing")]
    pub selected_kind: Option<ServiceKind>,
    /// The bit rate playing.
    #[field(label = "Bit rate playing", unit = "kbit/s")]
    pub selected_bitrate_kbps: Option<i64>,
    /// The protection playing ("EEP 3-A", "UEP 12").
    #[field(label = "Protection playing")]
    pub selected_protection: Option<&'a str>,
    /// What the audio of the service playing does.
    #[field(label = "Audio")]
    pub playing: Playing,
}

/// The audio of the service playing. No codec, rate, channels, SBR or PS before the audio configuration is known, and no
/// concealed share before the first access unit.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AudioFields {
    /// The audio codec.
    #[field(label = "Audio codec")]
    pub audio_codec: Option<AudioCodec>,
    /// The audio sample rate.
    #[field(label = "Audio sample rate", unit = "Hz")]
    pub audio_rate: Option<Hertz>,
    /// Audio channels.
    #[field(label = "Audio channels")]
    pub audio_channels: Option<i64>,
    /// SBR in use.
    #[field(label = "SBR")]
    pub audio_sbr: Option<bool>,
    /// Parametric stereo in use.
    #[field(label = "Parametric stereo")]
    pub audio_ps: Option<bool>,
    /// Access units in a DAB+ superframe.
    #[field(label = "Access units per superframe")]
    pub audio_units_per_superframe: Option<i64>,
    /// Logical frames of the sub-channel playing.
    #[field(label = "Logical frames")]
    pub audio_logical_frames: i64,
    /// Good access units.
    #[field(label = "Access units good")]
    pub audio_units_ok: i64,
    /// Bad access units.
    #[field(label = "Access units bad")]
    pub audio_units_bad: i64,
    /// Good superframes.
    #[field(label = "Superframes good")]
    pub audio_superframes_ok: i64,
    /// Bad superframes.
    #[field(label = "Superframes bad")]
    pub audio_superframes_bad: i64,
    /// Reed-Solomon bytes corrected.
    #[field(label = "RS bytes corrected")]
    pub audio_rs_corrected: i64,
    /// Reed-Solomon words beyond repair.
    #[field(label = "RS words beyond repair")]
    pub audio_rs_uncorrectable: i64,
    /// Layer II frames.
    #[field(label = "Layer II frames")]
    pub audio_mp2_frames: i64,
    /// Bad Layer II frames.
    #[field(label = "Layer II frames bad")]
    pub audio_mp2_bad: i64,
    /// The share of access units concealed.
    #[field(label = "Concealed", unit = "%")]
    pub audio_concealed_pct: Option<f64>,
}

/// The dynamic label and the slide of the service playing. No value before the first label or slide.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PadFields {
    /// The character set code of the dynamic label (TS 101 756 table 1).
    #[field(label = "Label character set")]
    pub label_charset: Option<i64>,
    /// The slide number: a new number is a new picture.
    #[field(label = "Slide number")]
    pub slide_sequence: Option<i64>,
    /// The slide width.
    #[field(label = "Slide width", unit = "px")]
    pub slide_width: Option<i64>,
    /// The slide height.
    #[field(label = "Slide height", unit = "px")]
    pub slide_height: Option<i64>,
    /// The slide size.
    #[field(label = "Slide size", unit = "B")]
    pub slide_bytes: Option<i64>,
}

/// When things were first reported, after the start. No value before it happened.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TimingFields {
    /// Lock.
    #[field(label = "Locked at", unit = "s")]
    pub timing_lock: Option<Seconds>,
    /// The first good FIB.
    #[field(label = "First good FIB at", unit = "s")]
    pub timing_first_fib: Option<Seconds>,
    /// The ensemble complete.
    #[field(label = "Ensemble complete at", unit = "s")]
    pub timing_ensemble_valid: Option<Seconds>,
    /// Every label known.
    #[field(label = "Every label at", unit = "s")]
    pub timing_all_labels: Option<Seconds>,
    /// The first audio of the service playing.
    #[field(label = "First audio at", unit = "s")]
    pub timing_first_access_unit: Option<Seconds>,
    /// The first label.
    #[field(label = "First label at", unit = "s")]
    pub timing_label: Option<Seconds>,
    /// The first slide.
    #[field(label = "First slide at", unit = "s")]
    pub timing_slide: Option<Seconds>,
}

/// The announcement that applies to the service playing. No value while none applies.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AnnouncementFields<'a> {
    /// The cluster of the announcement.
    #[field(label = "Announcement cluster")]
    pub announcement_cluster: Option<i64>,
    /// The type of the announcement (TS 101 756 table 14: 0 alarm, 1 road traffic ...).
    #[field(label = "Announcement type")]
    pub announcement_type: Option<i64>,
    /// The name of that type.
    #[field(label = "Announcement")]
    pub announcement_name: Option<&'a str>,
    /// The sub-channel of the announcement.
    #[field(label = "Announcement sub-channel")]
    pub announcement_subchannel: Option<i64>,
}

/// The DMB video sub-channel. No value before the DMB decoder reports, and no size before a sequence parameter set.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DmbFields {
    /// DMB transport stream packets.
    #[field(label = "DMB packets")]
    pub dmb_ts_packets: Option<i64>,
    /// DMB Reed-Solomon words corrected.
    #[field(label = "DMB RS words corrected")]
    pub dmb_rs_corrected: Option<i64>,
    /// DMB Reed-Solomon words beyond repair.
    #[field(label = "DMB RS words failed")]
    pub dmb_rs_failed: Option<i64>,
    /// DMB video access units.
    #[field(label = "DMB video units")]
    pub dmb_video_units: Option<i64>,
    /// DMB audio access units.
    #[field(label = "DMB audio units")]
    pub dmb_audio_units: Option<i64>,
    /// The width of the DMB video, from its sequence parameter set.
    #[field(label = "DMB video width", unit = "px")]
    pub dmb_video_width: Option<i64>,
    /// The height of the DMB video.
    #[field(label = "DMB video height", unit = "px")]
    pub dmb_video_height: Option<i64>,
}

/// The counts of the data decoders. No value before a decoder reports.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DataFields {
    /// FIG records kept for the analyser.
    #[field(label = "FIG records")]
    pub fic_records: i64,
    /// Guide objects decoded.
    #[field(label = "Guide objects")]
    pub spi_objects: Option<i64>,
    /// Journaline objects received.
    #[field(label = "Journaline objects received")]
    pub journaline_received: Option<i64>,
    /// Null symbols analysed for transmitter identification.
    #[field(label = "Null symbols analysed")]
    pub tii_analysed: Option<i64>,
}

/// The health indicators and the signal quality.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HealthFields {
    /// The receiver is locked.
    #[field(label = "Sync")]
    pub health_sync: Light,
    /// The synchronisation symbol peak.
    #[field(label = "Sync symbol")]
    pub health_prs: Light,
    /// The FIBs of the last frame.
    #[field(label = "FIC")]
    pub health_fic: Light,
    /// The ensemble.
    #[field(label = "Ensemble")]
    pub health_ensemble: Light,
    /// The sub-channel playing.
    #[field(label = "Sub-channel")]
    pub health_msc: Light,
    /// The audio.
    #[field(label = "Audio")]
    pub health_audio: Light,
    /// The concealment.
    #[field(label = "Concealment")]
    pub health_concealed: Light,
    /// The signal quality, 0 to 1: (SNR − 4 dB) / 18 dB. No value before the lock.
    #[field(label = "Signal quality")]
    pub quality: Option<f64>,
    /// The signal quality in words. No value before the lock.
    #[field(label = "Signal quality")]
    pub quality_text: Option<Quality>,
}

// The tables.

/// A service of the ensemble. The row key is the SId in hexadecimal: 4 digits, 8 for a data service.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ServiceRow<'a> {
    /// The service identifier (SId).
    #[field(label = "SId")]
    pub sid: i64,
    /// The label.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The short label.
    #[field(label = "Short label")]
    pub short_label: Option<&'a str>,
    /// What the service carries.
    #[field(label = "Type")]
    pub kind: ServiceKind,
    /// The sub-channel of the primary component.
    #[field(label = "Sub-channel", refers_to = "subchannels")]
    pub subchannel: Option<i64>,
    /// The bit rate of that sub-channel.
    #[field(label = "Bit rate", unit = "kbit/s")]
    pub bitrate_kbps: Option<i64>,
    /// The protection of that sub-channel ("EEP 3-A", "UEP 12").
    #[field(label = "Protection")]
    pub protection: Option<&'a str>,
    /// The ASCTy of the audio component, or the DSCTy of a data component.
    #[field(label = "Component type")]
    pub component_type: Option<i64>,
    /// The programme type code.
    #[field(label = "Programme type code")]
    pub pty: Option<i64>,
    /// The name of the programme type.
    #[field(label = "Programme type")]
    pub pty_name: Option<&'a str>,
    /// The programme type is static.
    #[field(label = "Programme type static")]
    pub pty_static: bool,
    /// The language code (TS 101 756 tables 9 and 10).
    #[field(label = "Language code")]
    pub language: Option<i64>,
    /// The name of the language.
    #[field(label = "Language")]
    pub language_name: Option<&'a str>,
    /// Scrambled (conditional access).
    #[field(label = "Scrambled")]
    pub scrambled: bool,
    /// The announcement support flags (ASu).
    #[field(label = "Announcement types")]
    pub announcement_support: i64,
    /// The dynamic label of the service, while it plays.
    #[field(label = "Now playing")]
    pub now_playing: Option<&'a str>,
    /// The service plays.
    #[field(label = "Playing")]
    pub playing: bool,
}

/// A sub-channel. The row key is its id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SubchannelRow<'a> {
    /// The sub-channel id.
    #[field(label = "Id")]
    pub id: i64,
    /// The first capacity unit.
    #[field(label = "Start", unit = "CU")]
    pub start_cu: i64,
    /// The size in capacity units.
    #[field(label = "Size", unit = "CU")]
    pub size_cu: i64,
    /// The protection ("EEP 3-A", "UEP 12").
    #[field(label = "Protection")]
    pub protection: Option<&'a str>,
    /// The bit rate.
    #[field(label = "Bit rate", unit = "kbit/s")]
    pub bitrate_kbps: i64,
}

/// A service component. The row key is `<service>/<n>`; n counts from 0 in the order of FIG 0/2 and FIG 0/8.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ServiceComponentRow<'a> {
    /// The row key of the service.
    #[field(label = "Service", refers_to = "services")]
    pub service: &'a str,
    /// The transport mechanism (TMId: 0 audio stream, 1 data stream, 3 packet data).
    #[field(label = "Transport")]
    pub tmid: i64,
    /// The ASCTy of an audio component, or the DSCTy of a data component.
    #[field(label = "Component type")]
    pub component_type: i64,
    /// The sub-channel.
    #[field(label = "Sub-channel", refers_to = "subchannels")]
    pub subchannel: Option<i64>,
    /// The packet address.
    #[field(label = "Packet address")]
    pub packet_address: Option<i64>,
    /// The service component identifier within the service (SCIdS).
    #[field(label = "SCIdS")]
    pub scids: Option<i64>,
    /// The primary component.
    #[field(label = "Primary")]
    pub primary: bool,
    /// Scrambled (conditional access).
    #[field(label = "Scrambled")]
    pub scrambled: bool,
    /// The component label.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
}

/// A user application of a service (FIG 0/13). The row key is `<service>/<SCIdS>/<type>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UserApplicationRow<'a> {
    /// The row key of the service.
    #[field(label = "Service", refers_to = "services")]
    pub service: &'a str,
    /// The service component identifier within the service (SCIdS).
    #[field(label = "SCIdS")]
    pub scids: i64,
    /// The user application type (TS 101 756 table 16).
    #[field(label = "Application type")]
    pub app_type: i64,
    /// The name of the type.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// An announcement cluster of a service (FIG 0/18). The row key is `<service>/<cluster>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ServiceClusterRow<'a> {
    /// The row key of the service.
    #[field(label = "Service", refers_to = "services")]
    pub service: &'a str,
    /// The announcement cluster.
    #[field(label = "Cluster")]
    pub cluster: i64,
}

/// A transmitter of the database of FIG 0/22. The row key is `<main>.<sub>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TiiEntryRow {
    /// The transmitter main identifier.
    #[field(label = "Main id")]
    pub main: i64,
    /// The transmitter sub-identifier.
    #[field(label = "Sub id")]
    pub sub: i64,
    /// The position.
    #[field(label = "Position")]
    pub position: (f64, f64),
    /// The delay.
    #[field(label = "Delay", unit = "us")]
    pub delay_us: i64,
}

/// A frequency where the service playing can also be received. The row key is the frequency in Hz.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FollowingRow<'a> {
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// The channel name.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// The ensemble identifier.
    #[field(label = "EId")]
    pub eid: i64,
}

/// An announcement running. The row key is its cluster.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AnnouncementRow {
    /// The announcement cluster.
    #[field(label = "Cluster")]
    pub cluster: i64,
    /// The announcement switching flags (ASw).
    #[field(label = "Types")]
    pub asw: i64,
    /// The sub-channel of the announcement.
    #[field(label = "Sub-channel", refers_to = "subchannels")]
    pub subchannel: i64,
}

/// A transmitter identified. The row key is `<main>.<sub>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TransmitterRow {
    /// The transmitter main identifier.
    #[field(label = "Main id")]
    pub main: i64,
    /// The transmitter sub-identifier.
    #[field(label = "Sub id")]
    pub sub: i64,
    /// The level relative to the strongest transmitter.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// The position, from the transmitter database.
    #[field(label = "Position")]
    pub position: Option<(f64, f64)>,
    /// The delay, from the transmitter database.
    #[field(label = "Delay", unit = "us")]
    pub delay_us: Option<i64>,
}

/// A Journaline object. The row key is its id in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct JournalineRow<'a> {
    /// The object id.
    #[field(label = "Id")]
    pub id: i64,
    /// The kind of the object.
    #[field(label = "Kind")]
    pub kind: JournalineKind,
    /// The title.
    #[field(label = "Title")]
    pub title: &'a str,
    /// The body text.
    #[field(label = "Body")]
    pub body: Option<&'a str>,
}

/// A menu entry of a Journaline object. The row key is `<object>/<n>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct JournalineLinkRow<'a> {
    /// The row key of the object.
    #[field(label = "Object", refers_to = "journaline")]
    pub object: &'a str,
    /// The object the entry leads to.
    #[field(label = "Leads to")]
    pub target: i64,
    /// The text of the entry.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A list item of a Journaline object. The row key is `<object>/<n>`. Columns are separated by a tab.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct JournalineItemRow<'a> {
    /// The row key of the object.
    #[field(label = "Object", refers_to = "journaline")]
    pub object: &'a str,
    /// The text of the item.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The statistics of a FIG type. The row key is the FIG type ("0/2").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FigTypeRow<'a> {
    /// The FIG type.
    #[field(label = "FIG")]
    pub fig: &'a str,
    /// FIGs received.
    #[field(label = "Count")]
    pub count: i64,
    /// Bytes received.
    #[field(label = "Bytes")]
    pub bytes: i64,
}

/// The services of the ensemble.
pub static SERVICES: Table<ServiceRow<'static>> =
    Table::new("services", "Services", 256).label(&ServiceRow::LABEL);
/// The sub-channels.
pub static SUBCHANNELS: Table<SubchannelRow<'static>> =
    Table::new("subchannels", "Sub-channels", 64);
/// The service components.
pub static SERVICE_COMPONENTS: Table<ServiceComponentRow<'static>> =
    Table::new("service_components", "Service components", 512).label(&ServiceComponentRow::LABEL);
/// The user applications of the services.
pub static USER_APPLICATIONS: Table<UserApplicationRow<'static>> =
    Table::new("user_applications", "User applications", 512).label(&UserApplicationRow::NAME);
/// The announcement clusters of the services.
pub static SERVICE_CLUSTERS: Table<ServiceClusterRow<'static>> =
    Table::new("service_clusters", "Announcement clusters", 512);
/// The transmitter database of FIG 0/22.
pub static TII_DATABASE: Table<TiiEntryRow> =
    Table::new("tii_database", "Transmitter database", 256);
/// Where the service playing can also be received.
pub static FOLLOWING: Table<FollowingRow<'static>> =
    Table::new("following", "Service following", 64);
/// The announcements running.
pub static ANNOUNCEMENTS: Table<AnnouncementRow> = Table::new("announcements", "Announcements", 32);
/// The transmitters identified.
pub static TRANSMITTERS: Table<TransmitterRow> = Table::new("transmitters", "Transmitters", 64);
/// The Journaline objects.
pub static JOURNALINE: Table<JournalineRow<'static>> =
    Table::new("journaline", "Journaline", 512).label(&JournalineRow::TITLE);
/// The menu entries of the Journaline objects.
pub static JOURNALINE_LINKS: Table<JournalineLinkRow<'static>> =
    Table::new("journaline_links", "Journaline menu entries", 4096);
/// The list items of the Journaline objects.
pub static JOURNALINE_ITEMS: Table<JournalineItemRow<'static>> =
    Table::new("journaline_items", "Journaline list items", 4096);
/// The FIG statistics.
pub static FIG_TYPES: Table<FigTypeRow<'static>> = Table::new("fig_types", "FIG types", 256);

// The series.

/// The constellation of the last symbol.
pub const CONSTELLATION: SeriesDef = SeriesDef {
    key: "constellation",
    label: "Constellation",
    x_unit: "",
    y_unit: "",
};
/// The channel impulse response.
pub const IMPULSE_RESPONSE: SeriesDef = SeriesDef {
    key: "impulse_response",
    label: "Impulse response",
    x_unit: "sample",
    y_unit: "",
};

// The events.

/// The fields of the event of a complete ensemble.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EnsembleEvent<'a> {
    /// The EId in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The ensemble label.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a changed service list.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ServicesEvent<'a> {
    /// The EId in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The number of services.
    #[field(label = "Services")]
    pub count: i64,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a started announcement.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AnnouncementStartedEvent<'a> {
    /// The EId in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The announcement cluster.
    #[field(label = "Cluster")]
    pub cluster: i64,
    /// The announcement switching flags (ASw).
    #[field(label = "Types")]
    pub asw: i64,
    /// The sub-channel of the announcement.
    #[field(label = "Sub-channel", refers_to = "subchannels")]
    pub subchannel: i64,
    /// The name of the first type of the flags.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Urgent for an alarm announcement (type 0), else a notice.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of an ended announcement.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AnnouncementEndedEvent<'a> {
    /// The EId in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The announcement cluster.
    #[field(label = "Cluster")]
    pub cluster: i64,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a lost lock.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SyncLostEvent {
    /// The family of the sender: the receiver.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a new dynamic label.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LabelEvent<'a> {
    /// The SId of the service playing, in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The label.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of the event of a new slide.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlideEvent<'a> {
    /// The SId of the service playing, in hexadecimal.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The slide number.
    #[field(label = "Slide number")]
    pub slide_sequence: i64,
    /// The name of the slide.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The ensemble became complete.
pub static ENSEMBLE_FOUND: EventKind<EnsembleEvent<'static>> =
    EventKind::new("dab.ensemble", "Ensemble complete");
/// The service list changed.
pub static SERVICES_CHANGED: EventKind<ServicesEvent<'static>> =
    EventKind::new("dab.services", "Service list changed");
/// An announcement started.
pub static ANNOUNCEMENT_STARTED: EventKind<AnnouncementStartedEvent<'static>> =
    EventKind::new("dab.announcement_started", "Announcement started");
/// An announcement ended.
pub static ANNOUNCEMENT_ENDED: EventKind<AnnouncementEndedEvent<'static>> =
    EventKind::new("dab.announcement_ended", "Announcement ended");
/// The receiver lost its lock.
pub static SYNC_LOST: EventKind<SyncLostEvent> = EventKind::new("dab.sync_lost", "Sync lost");
/// A new dynamic label of the service playing.
pub static LABEL_CHANGED: EventKind<LabelEvent<'static>> =
    EventKind::new("dab.label", "Dynamic label");
/// A new slide of the service playing.
pub static SLIDE_SHOWN: EventKind<SlideEvent<'static>> = EventKind::new("dab.slide", "Slide");

/// What a change of frequency keeps: the guide and the logos that it names, which describe services of several
/// ensembles, and the count of guide objects.
pub const KEPT: &[&str] = &[
    "guide_services",
    "guide_programmes",
    "guide_groups",
    "logos",
    "DataFields.spi_objects",
];

/// The interface of the family: what the DAB stack declares.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("dab", 2, 0)
            .content(sdrkit_core::ContentKind::Broadcast, &["dab"])
            .audio(sdrkit_core::AudioKind::Programme, &["dab"])
            .pictures(&["dab"])
            .uses(sdrkit_proto_mpegts::publish::interface())
            .uses(bc::interface())
            .shared_source("components", &["dab"])
            .shared_source("results", &["dab"])
            .shared_source("scan", &["dab"])
            .shared_source("verify", &["dab"])
            .stack(
                StackShape::new(crate::receiver::ID)
                    .fields::<SignalFields>()
                    .fields::<EnsembleFields>()
                    .fields::<SyncFields>()
                    .fields::<PlayingFields>()
                    .fields::<AudioFields>()
                    .fields::<PadFields>()
                    .fields::<TimingFields>()
                    .fields::<AnnouncementFields>()
                    .fields::<DmbFields>()
                    .fields::<DataFields>()
                    .fields::<HealthFields>()
                    .fields::<bc::LabelFields>()
                    .fields::<bc::SlideFields>()
                    .fields::<PlayoutFields>()
                    .component("playout", |c| c.fields::<PlayoutFields>().shares_declared())
                    .table(&SERVICES)
                    .table(&SUBCHANNELS)
                    .table(&SERVICE_COMPONENTS)
                    .table(&USER_APPLICATIONS)
                    .table(&SERVICE_CLUSTERS)
                    .table(&TII_DATABASE)
                    .table(&FOLLOWING)
                    .table(&ANNOUNCEMENTS)
                    .table(&TRANSMITTERS)
                    .table(&JOURNALINE)
                    .table(&JOURNALINE_LINKS)
                    .table(&JOURNALINE_ITEMS)
                    .table(&FIG_TYPES)
                    .table(&bc::LABEL_TAG_TABLE)
                    .table(&bc::GUIDE_SERVICE_TABLE)
                    .table(&bc::GUIDE_PROGRAMME_TABLE)
                    .table(&bc::GUIDE_GROUP_TABLE)
                    .table(&bc::SLIDE_CATEGORY_TABLE)
                    .table(&bc::LOGO_TABLE)
                    .series(CONSTELLATION)
                    .series(IMPULSE_RESPONSE)
                    .event(&ENSEMBLE_FOUND)
                    .event(&SERVICES_CHANGED)
                    .event(&ANNOUNCEMENT_STARTED)
                    .event(&ANNOUNCEMENT_ENDED)
                    .event(&SYNC_LOST)
                    .event(&LABEL_CHANGED)
                    .event(&SLIDE_SHOWN)
                    .settings(DabSetting::SETTINGS)
                    .command(&SERVICE_SELECT_KIND)
                    .kept(KEPT),
            )
    })
}

/// The description of the DAB stack.
///
/// # Panics
///
/// If the family does not hold the stack.
pub fn shape() -> &'static StackShape {
    family()
        .stack_shape(crate::receiver::ID)
        .expect("the family holds the DAB stack")
}
