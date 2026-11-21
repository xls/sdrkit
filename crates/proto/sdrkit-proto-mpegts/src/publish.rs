//! The declarations of what a host reads of a transport stream: the tables of services, tracks, guide events and teletext
//! pages, the events of the graph outputs, and the commands that act on a row.
//!
//! The tables refer to each other by row key. A row of `services` has the service_id as its key. A row of `tracks` has the key
//! `<service>/<pid>`, or `<service>/<pid>/<n>` for entry `n` of a subtitle or teletext stream. A row of `guide` has the key
//! `<onid>/<tsid>/<service_id>/<event_id>`, and a row of `guide_ratings` the key `<guide row>/<country>`. A row of
//! `teletext_pages` has the page number as its key, and a row of `teletext_subpages` the key `<page>/<subcode>`.

use crate::codec::{EsCodec, EsKind, EsPackaging};
use crate::epg::EpgEvent;
use crate::si::{list_key, CaState, Service, ServiceKind, SiDb, StreamEntry};
use crate::subtitle::SubtitleDisplay;
use crate::teletext::{TeletextDecoder, TeletextPage};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventShape, Family, Fields, Hertz, Seconds, StackShape, Table, Time,
};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::{EventDef, Record};
use sdrkit_graph::Item;
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

// Columns shared by several tables.

/// The row key of a service in `services`.
pub const SERVICE: Field<field::Text> =
    Field::new("service", "Service", "").refers_to_table("services");
/// service_id.
pub const SERVICE_ID: Field<field::I64> = Field::new("service_id", "Service id", "");
/// original_network_id.
pub const ONID: Field<field::I64> = Field::new("onid", "Original network id", "");
/// transport_stream_id.
pub const TSID: Field<field::I64> = Field::new("tsid", "Transport stream id", "");
/// A name.
pub const NAME: Field<field::Text> = Field::new("name", "Name", "");
/// Scrambled.
pub const SCRAMBLED: Field<field::Bool> = Field::new("scrambled", "Scrambled", "");
/// A PID.
pub const PID: Field<field::I64> = Field::new("pid", "PID", "");
/// An ISO 639-2 language code.
pub const LANG: Field<field::Text> = Field::new("lang", "Language", "");
/// running_status.
pub const RUNNING: Field<field::I64> = Field::new("running_status", "Running status", "");
/// A teletext page number (100 to 899).
pub const PAGE: Field<field::I64> = Field::new("page", "Page", "");

// The services.

/// service_type.
pub const SERVICE_TYPE: Field<field::I64> = Field::new("service_type", "Service type", "");
/// What the service is.
pub const SERVICE_KIND: Field<field::Choice<ServiceKind>> = Field::new("kind", "Kind", "");
/// The provider.
pub const PROVIDER: Field<field::Text> = Field::new("provider", "Provider", "");
/// The logical channel number (0 if none).
pub const LCN: Field<field::I64> = Field::new("lcn", "Channel number", "");
/// Shown in a channel list.
pub const VISIBLE: Field<field::Bool> = Field::new("visible", "Visible", "");
/// Whether it is scrambled.
pub const CA_STATE: Field<field::Choice<CaState>> = Field::new("ca_state", "Access", "");
/// The PMT PID.
pub const PMT_PID: Field<field::I64> = Field::new("pmt_pid", "PMT PID", "");
/// The PCR PID.
pub const PCR_PID: Field<field::I64> = Field::new("pcr_pid", "PCR PID", "");
/// Listed in the PAT.
pub const IN_PAT: Field<field::Bool> = Field::new("in_pat", "In the PAT", "");
/// EIT_schedule_flag.
pub const EIT_SCHEDULE: Field<field::Bool> = Field::new("eit_schedule", "Schedule guide", "");
/// EIT_present_following_flag.
pub const EIT_PF: Field<field::Bool> =
    Field::new("eit_present_following", "Now and next guide", "");
/// The present event's title.
pub const NOW_TITLE: Field<field::Text> = Field::new("now_title", "Now", "");
/// The present event's text.
pub const NOW_TEXT: Field<field::Text> = Field::new("now_text", "Now text", "");
/// The present event's start.
pub const NOW_START: Field<field::Time> = Field::new("now_start", "Now start", "");
/// The present event's length.
pub const NOW_DURATION: Field<field::Duration> = Field::new("now_duration", "Now length", "s");
/// The following event's title.
pub const NEXT_TITLE: Field<field::Text> = Field::new("next_title", "Next", "");
/// The following event's text.
pub const NEXT_TEXT: Field<field::Text> = Field::new("next_text", "Next text", "");
/// The following event's start.
pub const NEXT_START: Field<field::Time> = Field::new("next_start", "Next start", "");
/// The following event's length.
pub const NEXT_DURATION: Field<field::Duration> = Field::new("next_duration", "Next length", "s");

/// A service of the multiplex. The row key is the service_id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsServiceRow<'a> {
    /// service_id.
    #[field(shared = SERVICE_ID)]
    pub service_id: i64,
    /// The name.
    #[field(shared = NAME)]
    pub name: &'a str,
    /// The provider.
    #[field(shared = PROVIDER)]
    pub provider: &'a str,
    /// service_type.
    #[field(shared = SERVICE_TYPE)]
    pub service_type: i64,
    /// What the service is.
    #[field(shared = SERVICE_KIND)]
    pub kind: ServiceKind,
    /// The logical channel number (0 if none).
    #[field(shared = LCN)]
    pub lcn: i64,
    /// Shown in a channel list.
    #[field(shared = VISIBLE)]
    pub visible: bool,
    /// Scrambled.
    #[field(shared = SCRAMBLED)]
    pub scrambled: bool,
    /// Whether it is scrambled.
    #[field(shared = CA_STATE)]
    pub ca_state: CaState,
    /// The PMT PID.
    #[field(shared = PMT_PID)]
    pub pmt_pid: i64,
    /// The PCR PID.
    #[field(shared = PCR_PID)]
    pub pcr_pid: i64,
    /// running_status.
    #[field(shared = RUNNING)]
    pub running_status: i64,
    /// Listed in the PAT.
    #[field(shared = IN_PAT)]
    pub in_pat: bool,
    /// EIT_schedule_flag.
    #[field(shared = EIT_SCHEDULE)]
    pub eit_schedule: bool,
    /// EIT_present_following_flag.
    #[field(shared = EIT_PF)]
    pub eit_present_following: bool,
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
    /// The title of the present event.
    #[field(shared = NOW_TITLE)]
    pub now_title: Option<&'a str>,
    /// The text of the present event.
    #[field(shared = NOW_TEXT)]
    pub now_text: Option<&'a str>,
    /// The start of the present event.
    #[field(shared = NOW_START)]
    pub now_start: Option<Time>,
    /// The length of the present event.
    #[field(shared = NOW_DURATION)]
    pub now_duration: Option<Seconds>,
    /// The title of the following event.
    #[field(shared = NEXT_TITLE)]
    pub next_title: Option<&'a str>,
    /// The text of the following event.
    #[field(shared = NEXT_TEXT)]
    pub next_text: Option<&'a str>,
    /// The start of the following event.
    #[field(shared = NEXT_START)]
    pub next_start: Option<Time>,
    /// The length of the following event.
    #[field(shared = NEXT_DURATION)]
    pub next_duration: Option<Seconds>,
}

/// The services of the multiplex. Row key: the service_id.
pub static SERVICES: Table<TsServiceRow<'static>> = Table::new("services", "Services", 1000);

// The tracks.

/// stream_type.
pub const STREAM_TYPE: Field<field::I64> = Field::new("stream_type", "Stream type", "");
/// What the track is.
pub const TRACK_KIND: Field<field::Choice<EsKind>> = Field::new("kind", "Kind", "");
/// The codec.
pub const CODEC: Field<field::Choice<EsCodec>> = Field::new("codec", "Codec", "");
/// The packaging.
pub const PACKAGING: Field<field::Choice<EsPackaging>> = Field::new("packaging", "Packaging", "");
/// The four-character code.
pub const FOURCC: Field<field::Text> = Field::new("fourcc", "FourCC", "");
/// audio_type of the language entry.
pub const AUDIO_TYPE: Field<field::I64> = Field::new("audio_type", "Audio type", "");
/// An audio description for people who cannot see.
pub const AUDIO_DESCRIPTION: Field<field::Bool> =
    Field::new("audio_description", "Audio description", "");
/// Subtitles for the hard of hearing.
pub const HARD_OF_HEARING: Field<field::Bool> =
    Field::new("hard_of_hearing", "Hard of hearing", "");
/// component_tag.
pub const COMPONENT_TAG: Field<field::I64> = Field::new("component_tag", "Component tag", "");
/// The bit rate of the PID.
pub const KBPS: Field<field::F64> = Field::new("kbps", "Bit rate", "kbit/s");
/// The profile.
pub const PROFILE: Field<field::Text> = Field::new("profile", "Profile", "");
/// The level.
pub const LEVEL: Field<field::Text> = Field::new("level", "Level", "");
/// The picture width.
pub const WIDTH: Field<field::I64> = Field::new("width", "Width", "px");
/// The picture height.
pub const HEIGHT: Field<field::I64> = Field::new("height", "Height", "px");
/// Frames per second.
pub const FRAME_RATE: Field<field::F64> = Field::new("frame_rate", "Frame rate", "Hz");
/// The display aspect.
pub const ASPECT: Field<field::Text> = Field::new("aspect", "Aspect", "");
/// Interlaced.
pub const INTERLACED: Field<field::Bool> = Field::new("interlaced", "Interlaced", "");
/// The audio sample rate.
pub const SAMPLE_RATE: Field<field::Hz> = Field::new("sample_rate", "Sample rate", "Hz");
/// The audio channels.
pub const CHANNELS: Field<field::I64> = Field::new("channels", "Channels", "");
/// The channel layout.
pub const LAYOUT: Field<field::Text> = Field::new("layout", "Layout", "");
/// The bit rate that the frame header provides.
pub const BITRATE_KBPS: Field<field::I64> = Field::new("bitrate_kbps", "Coded bit rate", "kbit/s");
/// The AAC object type or the MPEG audio layer.
pub const OBJECT_TYPE: Field<field::Text> = Field::new("object_type", "Object type", "");
/// subtitling_type, or teletext_type.
pub const ENTRY_TYPE: Field<field::I64> = Field::new("entry_type", "Entry type", "");
/// composition_page_id.
pub const COMPOSITION_PAGE: Field<field::I64> =
    Field::new("composition_page", "Composition page", "");
/// ancillary_page_id.
pub const ANCILLARY_PAGE: Field<field::I64> = Field::new("ancillary_page", "Ancillary page", "");
/// teletext_magazine_number.
pub const MAGAZINE: Field<field::I64> = Field::new("magazine", "Magazine", "");
/// teletext_page_number.
pub const TELETEXT_PAGE: Field<field::I64> = Field::new("teletext_page", "Teletext page", "");

/// A track of a service: an elementary stream, or one entry of a subtitle or teletext stream. The row key is
/// `<service>/<pid>` or `<service>/<pid>/<n>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsTrackRow<'a> {
    /// The row key of the service in `services`.
    #[field(shared = SERVICE)]
    pub service: &'a str,
    /// The PID.
    #[field(shared = PID)]
    pub pid: i64,
    /// stream_type.
    #[field(shared = STREAM_TYPE)]
    pub stream_type: i64,
    /// What the track is.
    #[field(shared = TRACK_KIND)]
    pub kind: Option<EsKind>,
    /// The codec.
    #[field(shared = CODEC)]
    pub codec: Option<EsCodec>,
    /// The packaging.
    #[field(shared = PACKAGING)]
    pub packaging: Option<EsPackaging>,
    /// The four-character code.
    #[field(shared = FOURCC)]
    pub fourcc: Option<&'a str>,
    /// The ISO 639-2 language code.
    #[field(shared = LANG)]
    pub lang: &'a str,
    /// audio_type of the language entry.
    #[field(shared = AUDIO_TYPE)]
    pub audio_type: i64,
    /// An audio description for people who cannot see.
    #[field(shared = AUDIO_DESCRIPTION)]
    pub audio_description: bool,
    /// Subtitles for the hard of hearing.
    #[field(shared = HARD_OF_HEARING)]
    pub hard_of_hearing: bool,
    /// component_tag.
    #[field(shared = COMPONENT_TAG)]
    pub component_tag: Option<i64>,
    /// The bit rate of the PID.
    #[field(shared = KBPS)]
    pub kbps: f64,
    /// Scrambled.
    #[field(shared = SCRAMBLED)]
    pub scrambled: bool,
    /// The profile.
    #[field(shared = PROFILE)]
    pub profile: Option<&'a str>,
    /// The level.
    #[field(shared = LEVEL)]
    pub level: Option<&'a str>,
    /// The picture width. No value for a track that is not video.
    #[field(shared = WIDTH)]
    pub width: Option<i64>,
    /// The picture height. No value for a track that is not video.
    #[field(shared = HEIGHT)]
    pub height: Option<i64>,
    /// Frames per second.
    #[field(shared = FRAME_RATE)]
    pub frame_rate: Option<f64>,
    /// The display aspect.
    #[field(shared = ASPECT)]
    pub aspect: Option<&'a str>,
    /// Interlaced.
    #[field(shared = INTERLACED)]
    pub interlaced: Option<bool>,
    /// The audio sample rate. No value for a track that is not audio.
    #[field(shared = SAMPLE_RATE)]
    pub sample_rate: Option<Hertz>,
    /// The audio channels. No value for a track that is not audio.
    #[field(shared = CHANNELS)]
    pub channels: Option<i64>,
    /// The channel layout.
    #[field(shared = LAYOUT)]
    pub layout: Option<&'a str>,
    /// The bit rate that the frame header provides.
    #[field(shared = BITRATE_KBPS)]
    pub bitrate_kbps: Option<i64>,
    /// The AAC object type or the MPEG audio layer.
    #[field(shared = OBJECT_TYPE)]
    pub object_type: Option<&'a str>,
    /// subtitling_type, or teletext_type.
    #[field(shared = ENTRY_TYPE)]
    pub entry_type: Option<i64>,
    /// composition_page_id.
    #[field(shared = COMPOSITION_PAGE)]
    pub composition_page: Option<i64>,
    /// ancillary_page_id.
    #[field(shared = ANCILLARY_PAGE)]
    pub ancillary_page: Option<i64>,
    /// teletext_magazine_number.
    #[field(shared = MAGAZINE)]
    pub magazine: Option<i64>,
    /// teletext_page_number.
    #[field(shared = TELETEXT_PAGE)]
    pub teletext_page: Option<i64>,
}

/// The tracks of the services: a row per elementary stream, and a row per entry of a subtitle or teletext stream. Row key:
/// `<service>/<pid>` or `<service>/<pid>/<n>`.
pub static TRACKS: Table<TsTrackRow<'static>> = Table::new("tracks", "Tracks", 8192);

// The guide.

/// event_id.
pub const EVENT_ID: Field<field::I64> = Field::new("event_id", "Event id", "");
/// The start.
pub const START: Field<field::Time> = Field::new("start", "Start", "");
/// The length.
pub const DURATION: Field<field::Duration> = Field::new("duration", "Length", "s");
/// The title.
pub const TITLE: Field<field::Text> = Field::new("title", "Title", "");
/// The short text.
pub const TEXT: Field<field::Text> = Field::new("text", "Text", "");
/// The extended text.
pub const EXTENDED: Field<field::Text> = Field::new("extended", "Extended text", "");
/// The first level-1 content nibble.
pub const GENRE: Field<field::I64> = Field::new("genre", "Genre", "");
/// The content bytes (up to 8).
pub const CONTENT: Field<field::Bytes> = Field::new("content", "Content", "");
/// The highest minimum age over all countries (0 if none).
pub const MIN_AGE: Field<field::I64> = Field::new("min_age", "Minimum age", "");
/// The programme CRID.
pub const CRID_PROGRAMME: Field<field::Text> = Field::new("crid_programme", "Programme CRID", "");
/// The series CRID.
pub const CRID_SERIES: Field<field::Text> = Field::new("crid_series", "Series CRID", "");
/// free_CA_mode.
pub const FREE_CA: Field<field::Bool> = Field::new("free_ca", "Scrambled when sent", "");
/// From an EIT of another transport stream.
pub const OTHER_TS: Field<field::Bool> = Field::new("other_ts", "Other multiplex", "");
/// From a schedule table.
pub const SCHEDULE: Field<field::Bool> = Field::new("schedule", "Schedule", "");

/// An event of the programme guide. The row key is `<onid>/<tsid>/<service_id>/<event_id>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsGuideRow<'a> {
    /// The row key of the service in `services`.
    #[field(shared = SERVICE)]
    pub service: &'a str,
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
    /// service_id.
    #[field(shared = SERVICE_ID)]
    pub service_id: i64,
    /// event_id.
    #[field(shared = EVENT_ID)]
    pub event_id: i64,
    /// The start.
    #[field(shared = START)]
    pub start: Time,
    /// The length.
    #[field(shared = DURATION)]
    pub duration: Seconds,
    /// running_status.
    #[field(shared = RUNNING)]
    pub running_status: i64,
    /// The title.
    #[field(shared = TITLE)]
    pub title: &'a str,
    /// The short text.
    #[field(shared = TEXT)]
    pub text: &'a str,
    /// The extended text.
    #[field(shared = EXTENDED)]
    pub extended: &'a str,
    /// The ISO 639-2 language code.
    #[field(shared = LANG)]
    pub lang: &'a str,
    /// The first level-1 content nibble. No value for an event without one.
    #[field(shared = GENRE)]
    pub genre: Option<i64>,
    /// The content bytes (up to 8).
    #[field(shared = CONTENT)]
    pub content: Arc<[u8]>,
    /// The highest minimum age over all countries (0 if none).
    #[field(shared = MIN_AGE)]
    pub min_age: i64,
    /// The programme CRID.
    #[field(shared = CRID_PROGRAMME)]
    pub crid_programme: &'a str,
    /// The series CRID.
    #[field(shared = CRID_SERIES)]
    pub crid_series: &'a str,
    /// free_CA_mode.
    #[field(shared = FREE_CA)]
    pub free_ca: bool,
    /// From an EIT of another transport stream.
    #[field(shared = OTHER_TS)]
    pub other_ts: bool,
    /// From a schedule table.
    #[field(shared = SCHEDULE)]
    pub schedule: bool,
}

/// The programme guide. Row key: `<onid>/<tsid>/<service_id>/<event_id>`.
pub static GUIDE: Table<TsGuideRow<'static>> = Table::new("guide", "Guide", 20_000);

/// The row key of a guide event in `guide`.
pub const EVENT: Field<field::Text> = Field::new("event", "Event", "").refers_to_table("guide");
/// An ISO 3166 country code.
pub const COUNTRY: Field<field::Text> = Field::new("country", "Country", "");
/// The rating (minimum age minus 3).
pub const RATING: Field<field::I64> = Field::new("rating", "Rating", "");

/// The parental rating of a guide event in one country. The row key is `<guide row>/<country>`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsGuideRatingRow<'a> {
    /// The row key of the guide event in `guide`.
    #[field(shared = EVENT)]
    pub event: &'a str,
    /// The ISO 3166 country code.
    #[field(shared = COUNTRY)]
    pub country: &'a str,
    /// The rating (minimum age minus 3).
    #[field(shared = RATING)]
    pub rating: i64,
}

/// The parental ratings of the guide events, one row per country. Row key: `<guide row>/<country>`.
pub static GUIDE_RATINGS: Table<TsGuideRatingRow<'static>> =
    Table::new("guide_ratings", "Guide ratings", 40_000);

// Teletext.

/// The number of subpages held.
pub const SUBPAGES: Field<field::I64> = Field::new("subpages", "Subpages", "");
/// A subcode, in nibble form.
pub const SUBCODE: Field<field::I64> = Field::new("subcode", "Subcode", "");
/// The version, raised on each change.
pub const VERSION: Field<field::I64> = Field::new("version", "Version", "");
/// The row key of a page in `teletext_pages`.
pub const PAGE_ROW: Field<field::Text> =
    Field::new("page_row", "Page", "").refers_to_table("teletext_pages");

/// A teletext page held. The row key is the page number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeletextPageRow {
    /// The page number.
    #[field(shared = PAGE)]
    pub page: i64,
    /// The number of subpages held.
    #[field(shared = SUBPAGES)]
    pub subpages: i64,
}

/// A subpage of a teletext page. The row key is `<page>/<subcode>`, the subcode as four hex digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeletextSubpageRow<'a> {
    /// The row key of the page in `teletext_pages`.
    #[field(shared = PAGE_ROW)]
    pub page_row: &'a str,
    /// The subcode, in nibble form.
    #[field(shared = SUBCODE)]
    pub subcode: i64,
    /// The version, raised on each change.
    #[field(shared = VERSION)]
    pub version: i64,
}

/// The teletext pages held.
pub static TELETEXT_PAGE_TABLE: Table<TeletextPageRow> =
    Table::new("teletext_pages", "Teletext pages", 800);
/// The subpages of the teletext pages.
pub static TELETEXT_SUBPAGE_TABLE: Table<TeletextSubpageRow<'static>> =
    Table::new("teletext_subpages", "Teletext subpages", 8192);

// Commands.

/// The row key of a row of [`SERVICES`].
pub const SERVICE_ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "").refers_to_table("services"))
        .help("The row key, as the table shows it.");
/// The row key of a row of [`TRACKS`].
pub const TRACK_ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "").refers_to_table("tracks"))
        .help("The row key, as the table shows it.");
/// The row key of a row of [`TELETEXT_PAGE_TABLE`].
pub const TELETEXT_ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "").refers_to_table("teletext_pages"))
        .help("The row key, as the table shows it.");

/// The arguments of `service.select`.
#[derive(Arguments)]
pub struct ServiceSelectArgs<'a> {
    /// The row key of the service in `services`. Empty: the first service that can play.
    #[arg(shared = SERVICE_ROW)]
    pub row: &'a str,
}

/// Plays the service whose `services` row key is [`SERVICE_ROW`]. An empty row key plays the first service in list order that has
/// a stream it can play (for a radio stack, the first service with audio), and follows the list when it changes.
pub const SERVICE_SELECT_KIND: CommandKind<ServiceSelectArgs<'static>> =
    CommandKind::new("service.select", "Select a service").help(
        "Plays a service of the multiplex. An empty row plays the first service that can play.",
    );

/// The arguments of `track.select`.
#[derive(Arguments)]
pub struct TrackSelectArgs<'a> {
    /// The row key of the track in `tracks`.
    #[arg(shared = TRACK_ROW)]
    pub row: &'a str,
}

/// Plays the audio track, or shows the subtitle track, whose `tracks` row key is [`TRACK_ROW`].
pub const TRACK_SELECT_KIND: CommandKind<TrackSelectArgs<'static>> =
    CommandKind::new("track.select", "Select a track")
        .help("Plays an audio track or shows a subtitle track.");

/// The arguments of `teletext.select`.
#[derive(Arguments)]
pub struct TeletextSelectArgs<'a> {
    /// The row key of the page in `teletext_pages`.
    #[arg(shared = TELETEXT_ROW)]
    pub row: &'a str,
}

/// Shows the teletext page whose `teletext_pages` row key is [`TELETEXT_ROW`].
pub const TELETEXT_SELECT: CommandKind<TeletextSelectArgs<'static>> =
    CommandKind::new("teletext.select", "Select a teletext page").help("Shows a teletext page.");

// Items and the events they become.

/// The service list as a typed item: a snapshot.
#[derive(Clone, Debug, Default)]
pub struct ServiceList {
    /// transport_stream_id.
    pub tsid: u16,
    /// original_network_id.
    pub onid: u16,
    /// network_id.
    pub network_id: u16,
    /// The network name.
    pub network_name: String,
    /// The services in list order.
    pub services: Vec<Service>,
}

impl ServiceList {
    /// The service with this id.
    pub fn service(&self, service_id: u16) -> Option<&Service> {
        self.services.iter().find(|s| s.service_id == service_id)
    }
}

/// The network id.
pub const NETWORK_ID: Field<field::I64> = Field::new("network_id", "Network id", "");
/// The network name.
pub const NETWORK_NAME: Field<field::Text> = Field::new("network_name", "Network", "");
/// The number of services.
pub const SERVICE_COUNT: Field<field::I64> = Field::new("service_count", "Services", "");

/// The fields of a new service list.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsServiceListEvent<'a> {
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// network_id.
    #[field(shared = NETWORK_ID)]
    pub network_id: i64,
    /// The network name.
    #[field(shared = NETWORK_NAME)]
    pub network_name: &'a str,
    /// The number of services.
    #[field(shared = SERVICE_COUNT)]
    pub service_count: i64,
}

/// The event of a new service list. The rows are in the `services` table.
pub static SERVICE_LIST_EVENT: EventDef = EventDef::new(
    "service.list",
    "Service list",
    <TsServiceListEvent<'static> as Fields>::SHAPE.fields,
);

/// The event type `service.list` with its shape, for the description of a stack.
pub fn service_list() -> EventShape {
    EventShape::of::<TsServiceListEvent<'static>>(&SERVICE_LIST_EVENT)
}

impl Item for ServiceList {
    const KIND: &'static str = "service.list";
    fn event_def() -> Option<&'static EventDef> {
        Some(&SERVICE_LIST_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        TsServiceListEvent {
            tsid: i64::from(self.tsid),
            onid: i64::from(self.onid),
            network_id: i64::from(self.network_id),
            network_name: &self.network_name,
            service_count: self.services.len() as i64,
        }
        .write(r);
    }
}

/// The stream's time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TsTime {
    /// UTC seconds.
    pub utc: i64,
    /// The local time offset in force, minutes.
    pub offset_min: Option<i32>,
}

/// The UTC of the stream.
pub const UTC: Field<field::Time> = Field::new("utc", "UTC", "");
/// The local time offset in force.
pub const LOCAL_OFFSET_MIN: Field<field::I64> =
    Field::new("local_offset_min", "Local offset", "min");

/// The fields of a new stream time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsTimeEvent {
    /// The UTC of the stream.
    #[field(shared = UTC)]
    pub utc: Time,
    /// The local time offset in force. No value before the stream gave one.
    #[field(shared = LOCAL_OFFSET_MIN)]
    pub local_offset_min: Option<i64>,
}

/// The event of a new stream time.
pub static TS_TIME_EVENT: EventDef = EventDef::new(
    "ts.time",
    "Stream time",
    <TsTimeEvent as Fields>::SHAPE.fields,
);

/// The event type `ts.time` with its shape, for the description of a stack.
pub fn ts_time() -> EventShape {
    EventShape::of::<TsTimeEvent>(&TS_TIME_EVENT)
}

impl Item for TsTime {
    const KIND: &'static str = "ts.time";
    fn event_def() -> Option<&'static EventDef> {
        Some(&TS_TIME_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        TsTimeEvent {
            utc: Time(self.utc as f64),
            local_offset_min: self.offset_min.map(i64::from),
        }
        .write(r);
    }
}

/// The network.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TsNetwork {
    /// network_id.
    pub network_id: u16,
    /// The name.
    pub name: String,
    /// original_network_id.
    pub onid: u16,
    /// transport_stream_id.
    pub tsid: u16,
}

/// The fields of a new network.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsNetworkEvent<'a> {
    /// network_id.
    #[field(shared = NETWORK_ID)]
    pub network_id: i64,
    /// The network name.
    #[field(shared = NETWORK_NAME)]
    pub network_name: &'a str,
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
}

/// The event of a new network.
pub static TS_NETWORK_EVENT: EventDef = EventDef::new(
    "ts.network",
    "Network",
    <TsNetworkEvent<'static> as Fields>::SHAPE.fields,
);

/// The event type `ts.network` with its shape, for the description of a stack.
pub fn ts_network() -> EventShape {
    EventShape::of::<TsNetworkEvent<'static>>(&TS_NETWORK_EVENT)
}

impl Item for TsNetwork {
    const KIND: &'static str = "ts.network";
    fn event_def() -> Option<&'static EventDef> {
        Some(&TS_NETWORK_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        TsNetworkEvent {
            network_id: i64::from(self.network_id),
            network_name: &self.name,
            onid: i64::from(self.onid),
            tsid: i64::from(self.tsid),
        }
        .write(r);
    }
}

/// The fields of a new or changed guide event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TsGuideEvent<'a> {
    /// original_network_id.
    #[field(shared = ONID)]
    pub onid: i64,
    /// transport_stream_id.
    #[field(shared = TSID)]
    pub tsid: i64,
    /// service_id.
    #[field(shared = SERVICE_ID)]
    pub service_id: i64,
    /// event_id.
    #[field(shared = EVENT_ID)]
    pub event_id: i64,
    /// The start.
    #[field(shared = START)]
    pub start: Time,
    /// The length.
    #[field(shared = DURATION)]
    pub duration: Seconds,
    /// The title.
    #[field(shared = TITLE)]
    pub title: &'a str,
}

/// The event of a new or changed guide event. The rows are in the `guide` table.
pub static EPG_EVENT: EventDef = EventDef::new(
    "epg.event",
    "Guide event",
    <TsGuideEvent<'static> as Fields>::SHAPE.fields,
);

/// The event type `epg.event` with its shape, for the description of a stack.
pub fn epg_event() -> EventShape {
    EventShape::of::<TsGuideEvent<'static>>(&EPG_EVENT)
}

impl Item for EpgEvent {
    const KIND: &'static str = "epg.event";
    fn event_def() -> Option<&'static EventDef> {
        Some(&EPG_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        let (onid, tsid, sid) = self.service;
        TsGuideEvent {
            onid: i64::from(onid),
            tsid: i64::from(tsid),
            service_id: i64::from(sid),
            event_id: i64::from(self.event_id),
            start: Time(self.start as f64),
            duration: Seconds(self.duration as f64),
            title: &self.title,
        }
        .write(r);
    }
}

/// The text of a page, its 25 rows joined with line feeds.
pub const PAGE_TEXT: Field<field::Text> = Field::new("text", "Text", "");

/// A teletext page that arrived.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeletextPageEvent<'a> {
    /// The page number.
    #[field(shared = PAGE)]
    pub page: i64,
    /// The subcode, in nibble form.
    #[field(shared = SUBCODE)]
    pub subcode: i64,
    /// The version, raised on each change.
    #[field(shared = VERSION)]
    pub version: i64,
    /// The text of the page, its 25 rows joined with line feeds.
    #[field(shared = PAGE_TEXT)]
    pub text: &'a str,
}

/// The event of a teletext page that arrived. The page list is in the `teletext_pages` table.
pub static TELETEXT_PAGE_EVENT: EventDef = EventDef::new(
    "teletext.page",
    "Teletext page",
    <TeletextPageEvent<'static> as Fields>::SHAPE.fields,
);

/// The event type `teletext.page` with its shape, for the description of a stack.
pub fn teletext_page() -> EventShape {
    EventShape::of::<TeletextPageEvent<'static>>(&TELETEXT_PAGE_EVENT)
}

/// The interface of this crate, which stacks of several families show: the tables, the result sets, the event types
/// and the commands of the transport stream layer and of teletext. A family whose stacks list one of them names this
/// interface with `Family::uses`. [`crate::layer::describe`] and [`crate::monitor::describe`] add the items of the
/// layer and of the monitor to the description of a stack.
pub fn interface() -> &'static Family {
    use crate::components::{PesAudioFields, PesVideoFields, TeletextStreamFields};
    use crate::layer::{self, TsLayer};
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        let teletext = StackShape::new("mpegts")
            .table(&TELETEXT_PAGE_TABLE)
            .table(&TELETEXT_SUBPAGE_TABLE)
            .event(teletext_page());
        let parts = TsLayer {
            teletext: false,
            subtitles: true,
        };
        let stack = layer::describe(crate::monitor::describe(teletext, "monitor"), &parts)
            .component(layer::TELETEXT, |c| {
                c.fields::<TeletextStreamFields<'static>>()
                    .writes_table(&TELETEXT_PAGE_TABLE)
                    .writes_table(&TELETEXT_SUBPAGE_TABLE)
            })
            .component("pes_video", |c| c.fields::<PesVideoFields>())
            .component("pes_audio", |c| c.fields::<PesAudioFields>())
            .command(&SERVICE_SELECT_KIND)
            .command(&TRACK_SELECT_KIND)
            .command(&TELETEXT_SELECT);
        Family::shared("mpegts", 1, 1).stack(stack).added(
            1,
            1,
            &["shape.TsDemuxFields.audio_decoded"],
        )
    })
}

impl Item for TeletextPage {
    const KIND: &'static str = "teletext.page";
    fn event_def() -> Option<&'static EventDef> {
        Some(&TELETEXT_PAGE_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        TeletextPageEvent {
            page: i64::from(self.page),
            subcode: i64::from(self.subcode),
            version: self.version as i64,
            text: &self.text().join("\n"),
        }
        .write(r);
    }
}

/// The presentation time stamp, 90 kHz.
pub const PTS: Field<field::I64> = Field::new("pts", "Presentation time", "");
/// How long the display shows at most.
pub const TIMEOUT: Field<field::Duration> = Field::new("timeout_s", "Time out", "s");
/// The display sequence number.
pub const SEQ: Field<field::I64> = Field::new("seq", "Sequence", "");
/// The number of regions (0 clears the screen).
pub const REGION_COUNT: Field<field::I64> = Field::new("region_count", "Regions", "");

/// The fields of a subtitle display.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DvbSubtitleEvent {
    /// The presentation time stamp, 90 kHz. No value for a display without one.
    #[field(shared = PTS)]
    pub pts: Option<i64>,
    /// How long the display shows at most.
    #[field(shared = TIMEOUT)]
    pub timeout_s: Seconds,
    /// The width of the display.
    #[field(shared = WIDTH)]
    pub width: i64,
    /// The height of the display.
    #[field(shared = HEIGHT)]
    pub height: i64,
    /// The display sequence number.
    #[field(shared = SEQ)]
    pub seq: i64,
    /// The number of regions (0 clears the screen).
    #[field(shared = REGION_COUNT)]
    pub region_count: i64,
}

/// The event of a subtitle display. The pictures are in the item.
pub static SUBTITLE_EVENT: EventDef = EventDef::new(
    "dvb.subtitle",
    "Subtitle",
    <DvbSubtitleEvent as Fields>::SHAPE.fields,
);

/// The event type `dvb.subtitle` with its shape, for the description of a stack.
pub fn dvb_subtitle() -> EventShape {
    EventShape::of::<DvbSubtitleEvent>(&SUBTITLE_EVENT)
}

impl Item for SubtitleDisplay {
    const KIND: &'static str = "dvb.subtitle";
    fn event_def() -> Option<&'static EventDef> {
        Some(&SUBTITLE_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        DvbSubtitleEvent {
            pts: self.pts,
            timeout_s: Seconds(f64::from(self.timeout_s)),
            width: i64::from(self.width),
            height: i64::from(self.height),
            seq: self.seq as i64,
            region_count: self.regions.len() as i64,
        }
        .write(r);
    }
}

// Writing the tables.

/// Writes the `services`, `tracks`, `guide` and `guide_ratings` tables from the service information. Only cells that changed
/// are written, and the rows of what left are removed. This is the one way to publish them. The `mpegts.ts_demux` component
/// writes the services and tracks, and `mpegts.psi_si` writes the guide. A stack that keeps its own [`SiDb`] calls
/// [`SiTables::write_services`] and [`SiTables::write_guide`] after each change.
#[derive(Debug, Default)]
pub struct SiTables {
    /// The row keys written by this call. Only the first `kept` are used, the rest keep their memory for the next call.
    keep: Vec<String>,
    kept: usize,
    /// The services in list order: (list key, service id).
    order: Vec<((u16, u16), u16)>,
    /// The service id as text.
    sid: String,
    /// The content bytes of guide events written before, each run of bytes once.
    contents: Vec<Arc<[u8]>>,
}

/// The most runs of content bytes that [`SiTables`] remembers.
const CONTENTS: usize = 256;

impl SiTables {
    /// The tables of [`SiTables::write_services`].
    pub const SERVICE_TABLES: [&'static TableDef; 2] = [SERVICES.def(), TRACKS.def()];
    /// The tables of [`SiTables::write_guide`].
    pub const GUIDE_TABLES: [&'static TableDef; 2] = [GUIDE.def(), GUIDE_RATINGS.def()];

    /// Writes the services and tracks of `db` into `p`, which must declare [`SiTables::SERVICE_TABLES`].
    pub fn write_services(&mut self, db: &SiDb, p: &mut Published) {
        self.order.clear();
        self.order
            .extend(db.known().map(|s| (list_key(s), s.service_id)));
        self.order.sort_unstable();
        let order = std::mem::take(&mut self.order);
        let services = || order.iter().filter_map(|&(_, sid)| db.service(sid));
        self.kept = 0;
        for s in services() {
            self.service(s, p);
        }
        retain(p, &SERVICES, &self.keep[..self.kept]);
        self.kept = 0;
        for s in services() {
            for e in &s.streams {
                self.tracks(s.service_id, e, p);
            }
        }
        retain(p, &TRACKS, &self.keep[..self.kept]);
        self.kept = 0;
        self.order = order;
    }

    /// Writes the guide of `db` into `p`, which must declare [`SiTables::GUIDE_TABLES`]. `changed` are the guide events that
    /// came or changed since the last call. The other rows are written again only when the rows and the events of `db` differ
    /// in number.
    pub fn write_guide(&mut self, db: &SiDb, changed: &[EpgEvent], p: &mut Published) {
        let mut ratings = Vec::new();
        for ev in changed {
            self.guide(ev, p, &mut ratings);
        }
        if table_len(p, &GUIDE) != db.epg.len() {
            // Events came that were not reported, or events ended: bring the rows to the guide.
            self.kept = 0;
            ratings.clear();
            let full = table_len(p, &GUIDE) < db.epg.len();
            for ev in db.epg.iter() {
                if full {
                    self.guide(ev, p, &mut ratings);
                } else {
                    let (onid, tsid, sid) = ev.service;
                    self.row_key(format_args!("{onid}/{tsid}/{sid}/{}", ev.event_id));
                }
            }
            let kept = &self.keep[..self.kept];
            retain(p, &GUIDE, kept);
            let keep: HashSet<&str> = kept.iter().map(String::as_str).collect();
            retain_by(p, &GUIDE_RATINGS, |k| {
                k.rsplit_once('/')
                    .is_some_and(|(event, _)| keep.contains(event))
            });
        }
        self.kept = 0;
    }

    /// Writes the next row key and returns it.
    fn row_key(&mut self, args: std::fmt::Arguments<'_>) -> &str {
        if self.kept == self.keep.len() {
            self.keep.push(String::new());
        }
        let key = &mut self.keep[self.kept];
        self.kept += 1;
        key.clear();
        // Writing to a String does not fail.
        let _ = key.write_fmt(args);
        key
    }

    fn service(&mut self, s: &Service, p: &mut Published) {
        let key = self.row_key(format_args!("{}", s.service_id));
        let (now, next) = (s.now.as_ref(), s.next.as_ref());
        let row = TsServiceRow {
            service_id: i64::from(s.service_id),
            name: &s.name,
            provider: &s.provider,
            service_type: i64::from(s.service_type),
            kind: s.kind,
            lcn: i64::from(s.lcn),
            visible: s.visible,
            scrambled: s.scrambled,
            ca_state: s.ca_state,
            pmt_pid: i64::from(s.pmt_pid),
            pcr_pid: i64::from(s.pcr_pid),
            running_status: i64::from(s.running_status),
            in_pat: s.in_pat,
            eit_schedule: s.eit_schedule,
            eit_present_following: s.eit_pf,
            onid: i64::from(s.onid),
            tsid: i64::from(s.tsid),
            now_title: now.map(|e| e.title.as_str()),
            now_text: now.map(|e| e.text.as_str()),
            now_start: now.map(|e| Time(e.start as f64)),
            now_duration: now.map(|e| Seconds(e.duration as f64)),
            next_title: next.map(|e| e.title.as_str()),
            next_text: next.map(|e| e.text.as_str()),
            next_start: next.map(|e| Time(e.start as f64)),
            next_duration: next.map(|e| Seconds(e.duration as f64)),
        };
        SERVICES.put(p, key, &row);
    }

    fn tracks(&mut self, sid: u16, e: &StreamEntry, p: &mut Published) {
        let entries = e.subtitles.len().max(e.teletext.len());
        let mut service = std::mem::take(&mut self.sid);
        service.clear();
        let _ = write!(service, "{sid}");
        for n in 0..entries.max(1) {
            let key = if entries == 0 {
                self.row_key(format_args!("{sid}/{}", e.pid))
            } else {
                self.row_key(format_args!("{sid}/{}/{n}", e.pid))
            };
            let fourcc = e.fourcc();
            let (sub, tt) = (e.subtitles.get(n), e.teletext.get(n));
            let pr = &e.props;
            let video = e.kind == Some(EsKind::Video);
            let audio = e.kind == Some(EsKind::Audio);
            let row = TsTrackRow {
                service: &service,
                pid: i64::from(e.pid),
                stream_type: i64::from(e.stream_type),
                kind: e.kind,
                codec: e.codec,
                packaging: e.packaging,
                fourcc: fourcc.as_ref().map(|f| f.as_str()),
                lang: match (sub, tt) {
                    (Some(x), _) => x.lang.as_str(),
                    (None, Some(x)) => x.lang.as_str(),
                    (None, None) => e.lang.as_str(),
                },
                audio_type: i64::from(e.audio_type),
                audio_description: e.audio_type == 3
                    || e.supplementary.as_ref().is_some_and(|x| x.editorial == 1),
                hard_of_hearing: sub.is_some_and(|x| (0x20..=0x24).contains(&x.kind)),
                component_tag: e.component_tag.map(i64::from),
                kbps: (e.kbps * 10.0).round() / 10.0,
                scrambled: e.scrambled,
                profile: pr.profile.as_deref(),
                level: pr.level.as_deref(),
                width: video.then_some(i64::from(pr.width)),
                height: video.then_some(i64::from(pr.height)),
                frame_rate: pr
                    .frame_rate
                    .map(|(n, d)| f64::from(n) / f64::from(d.max(1))),
                aspect: filled(&pr.aspect),
                interlaced: pr.interlaced,
                sample_rate: audio.then_some(Hertz(f64::from(pr.sample_rate))),
                channels: audio.then_some(i64::from(pr.channels)),
                layout: filled(&pr.layout),
                bitrate_kbps: (pr.bitrate_kbps > 0).then_some(i64::from(pr.bitrate_kbps)),
                object_type: filled(&pr.object_type),
                entry_type: sub
                    .map(|x| i64::from(x.kind))
                    .or(tt.map(|x| i64::from(x.kind))),
                composition_page: sub.map(|x| i64::from(x.composition_page)),
                ancillary_page: sub.map(|x| i64::from(x.ancillary_page)),
                magazine: tt.map(|x| i64::from(x.magazine)),
                teletext_page: tt.map(|x| i64::from(x.page)),
            };
            TRACKS.put(p, key, &row);
        }
        self.sid = service;
    }

    /// The bytes `content` as shared bytes. Bytes written before are not copied again.
    fn content(&mut self, content: &[u8]) -> Arc<[u8]> {
        if let Some(c) = self.contents.iter().find(|c| ***c == *content) {
            return Arc::clone(c);
        }
        let c: Arc<[u8]> = Arc::from(content);
        if self.contents.len() < CONTENTS {
            self.contents.push(Arc::clone(&c));
        }
        c
    }

    fn guide(&mut self, ev: &EpgEvent, p: &mut Published, ratings: &mut Vec<String>) {
        let (onid, tsid, sid) = ev.service;
        let event = self
            .row_key(format_args!("{onid}/{tsid}/{sid}/{}", ev.event_id))
            .to_owned();
        let service = sid.to_string();
        let row = TsGuideRow {
            service: &service,
            onid: i64::from(onid),
            tsid: i64::from(tsid),
            service_id: i64::from(sid),
            event_id: i64::from(ev.event_id),
            start: Time(ev.start as f64),
            duration: Seconds(ev.duration as f64),
            running_status: i64::from(ev.running),
            title: &ev.title,
            text: &ev.text,
            extended: &ev.extended,
            lang: &ev.lang,
            genre: (ev.genre >= 0).then_some(i64::from(ev.genre)),
            content: self.content(&ev.content),
            min_age: i64::from(ev.min_age),
            crid_programme: &ev.crid_programme,
            crid_series: &ev.crid_series,
            free_ca: ev.free_ca,
            other_ts: ev.other_ts,
            schedule: ev.schedule,
        };
        GUIDE.put(p, &event, &row);
        for (country, rating) in &ev.ratings {
            let key = format!("{event}/{country}");
            let row = TsGuideRatingRow {
                event: &event,
                country,
                rating: i64::from(*rating),
            };
            GUIDE_RATINGS.put(p, &key, &row);
            ratings.push(key);
        }
    }
}

/// Writes the `teletext_pages` and `teletext_subpages` tables from a teletext decoder. Only cells that changed are written, and
/// the rows of pages that left are removed.
#[derive(Debug, Default)]
pub struct TeletextTables {
    keep: Vec<String>,
    subkeep: Vec<String>,
}

impl TeletextTables {
    /// The tables written.
    pub const TABLES: [&'static TableDef; 2] =
        [TELETEXT_PAGE_TABLE.def(), TELETEXT_SUBPAGE_TABLE.def()];

    /// Writes the tables of `d` into `p`, which must declare [`TeletextTables::TABLES`].
    pub fn write(&mut self, d: &TeletextDecoder, p: &mut Published) {
        self.keep.clear();
        self.subkeep.clear();
        for page in d.pages() {
            let key = page.to_string();
            let subcodes = d.subcodes(page);
            let row = TeletextPageRow {
                page: i64::from(page),
                subpages: subcodes.len() as i64,
            };
            TELETEXT_PAGE_TABLE.put(p, &key, &row);
            for sub in subcodes {
                let sk = format!("{page}/{sub:04X}");
                let version = d.page(page, Some(sub)).map_or(0, |pg| pg.version as i64);
                let row = TeletextSubpageRow {
                    page_row: &key,
                    subcode: i64::from(sub),
                    version,
                };
                TELETEXT_SUBPAGE_TABLE.put(p, &sk, &row);
                self.subkeep.push(sk);
            }
            self.keep.push(key);
        }
        retain(p, &TELETEXT_PAGE_TABLE, &self.keep);
        retain(p, &TELETEXT_SUBPAGE_TABLE, &self.subkeep);
    }
}

/// `s`, or no value for an empty text.
fn filled(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// The number of rows of table `t`.
fn table_len(p: &Published, t: &TableDef) -> usize {
    p.results()
        .decl()
        .table_id(t.key)
        .and_then(|id| p.results().table(id))
        .map_or(0, |t| t.len())
}

/// Removes the rows of table `t` whose keys are not in `keep`, which holds each key once.
fn retain(p: &mut Published, t: &TableDef, keep: &[String]) {
    if table_len(p, t) == keep.len() {
        return;
    }
    let keep: HashSet<&str> = keep.iter().map(String::as_str).collect();
    retain_by(p, t, |k| keep.contains(k));
}

/// Removes the rows of table `t` whose keys `keep` refuses.
fn retain_by(p: &mut Published, t: &TableDef, keep: impl Fn(&str) -> bool) {
    let Some(id) = p.results().decl().table_id(t.key) else {
        return;
    };
    let Some(table) = p.results().table(id) else {
        return;
    };
    let gone: Vec<String> = table
        .rows_newest_first()
        .filter(|(k, _)| !keep(k))
        .map(|(k, _)| k.to_owned())
        .collect();
    if let Some(mut table) = p.results_mut().table_mut(id) {
        for k in &gone {
            table.remove(k);
        }
    }
}
