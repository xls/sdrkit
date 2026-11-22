//! What a front end needs to show a stack without knowing it: the kind of content of the stack, the media that it
//! gives, the column that names a row, the default order of the rows, and the columns that hold a place on the map.
//! A stack states them in its interface description, or they follow from what it declares.

use crate::publish::Declare;
use crate::value::ValueKind;
use serde::Serialize;

crate::choice_enum! {
    /// The kind of content of a stack: what its results are for a person. The set is closed. A front end maps each
    /// kind to a view; the kind names no view.
    pub enum ContentKind {
        /// Broadcast sound with programme data: station name, text, services.
        Broadcast = ("broadcast", "Broadcast radio"),
        /// Television and video, with services and a programme guide where the standard has them.
        Television = ("television", "Television and video"),
        /// Voice calls between stations, with a log of the calls.
        Voice = ("voice", "Voice calls"),
        /// Text that operators type to each other, read and sent as a running text.
        TextChat = ("text_chat", "Keyboard text"),
        /// Short messages of weak-signal protocols, decoded in time slots.
        WeakSignal = ("weak_signal", "Weak-signal messages"),
        /// Messages of a mesh or packet network, with the nodes of the network.
        Mesh = ("mesh", "Mesh messaging"),
        /// Things that report their position: aircraft, ships, balloons, drones, satellites.
        Tracking = ("tracking", "Position tracking"),
        /// Messages of a data link between stations and a network, as a log.
        Datalink = ("datalink", "Datalink messages"),
        /// Pictures and fax pages sent as a signal.
        Pictures = ("pictures", "Pictures and fax"),
        /// Images and data products of weather and earth observation satellites.
        SatelliteImagery = ("satellite_imagery", "Satellite imagery"),
        /// Measurements of a signal or of the band: numbers, limits, plots.
        Measurement = ("measurement", "Measurement and analysis"),
        /// Observations of nature and of propagation: trends and events over a long time.
        Science = ("science", "Science and propagation"),
        /// Navigation aids and beacons: identification, bearing, corrections.
        Navigation = ("navigation", "Navigation aids"),
        /// Cells, access points and links of a network, as lists with their signalling.
        Networks = ("networks", "Networks and cells"),
        /// Paging messages with their addresses.
        Paging = ("paging", "Paging"),
        /// Sensors, meters, tags and other devices that each send their own readings.
        Devices = ("devices", "Sensors and devices"),
        /// Frames and carriers of a utility protocol, as tables and a frame log.
        Utility = ("utility", "Utility decoders"),
        /// Carriers, frames and reports of a satellite link.
        SatelliteLinks = ("satellite_links", "Satellite links"),
        /// A receiver for any signal: demodulated sound and a spectrum.
        General = ("general", "General receiver"),
    }
}

crate::choice_enum! {
    /// The kind of sound that a stack gives.
    pub enum AudioKind {
        /// A broadcast programme: music and speech for listeners.
        Programme = ("programme", "Programme"),
        /// Speech of calls between stations. There is sound only while a call lasts.
        Voice = ("voice", "Voice"),
        /// The demodulated signal, to hear that it is there: tones, data sounds, an identification.
        Monitor = ("monitor", "Monitor"),
    }
}

/// The media that a stack gives, apart from its results.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Media {
    /// The sound, or `None` for a stack without sound.
    pub audio: Option<AudioKind>,
    /// Still pictures.
    pub pictures: bool,
    /// Moving pictures.
    pub video: bool,
    /// Running text for people: decoded characters or whole messages.
    pub text: bool,
}

/// What a column holds, for a front end that places a row on a map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The position of the row, for a table with several columns of the position kind. Without it, the first column of
    /// the position kind is the position.
    Position,
    /// The latitude in degrees north, in a table that has no column of the position kind.
    Latitude,
    /// The longitude in degrees east, in a table that has no column of the position kind.
    Longitude,
    /// The height. The unit of the column says in what.
    Altitude,
    /// The direction of travel, in degrees from north.
    Course,
    /// The speed. The unit of the column says in what.
    Speed,
}

/// The order of the rows of a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SortOrder {
    /// The lowest value first.
    Ascending,
    /// The highest value first.
    Descending,
}

/// What a table states about its rows, by column key. [`TableDef::decl`](crate::TableDef::decl) turns it into
/// [`TableMarks`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowMarks {
    /// The key of the column that names a row for people. Empty for none.
    pub label: &'static str,
    /// The key of the column that orders the rows by default. Empty for none: the newest row is first.
    pub sort: &'static str,
    /// The order along `sort`.
    pub order: SortOrder,
}

impl RowMarks {
    /// Nothing stated.
    pub const NONE: RowMarks = RowMarks {
        label: "",
        sort: "",
        order: SortOrder::Descending,
    };
}

/// The marks of a declared table, each as the number of a column of the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TableMarks {
    /// The column that names a row for people.
    pub label: Option<u16>,
    /// The column that orders the rows by default, and the order. `None`: the newest row is first.
    pub sort: Option<(u16, SortOrder)>,
    /// The column of the position kind that holds the place of a row.
    pub position: Option<u16>,
    /// The column of the latitude, where the table has no position column.
    pub latitude: Option<u16>,
    /// The column of the longitude, where the table has no position column.
    pub longitude: Option<u16>,
    /// The column of the height.
    pub altitude: Option<u16>,
    /// The column of the direction of travel.
    pub course: Option<u16>,
    /// The column of the speed.
    pub speed: Option<u16>,
}

impl TableMarks {
    /// The marks of a table with the columns `columns` that states `rows`. The position column is the column with
    /// [`Role::Position`], else the first column of the position kind. Each other place column is the first column
    /// with its role.
    ///
    /// # Panics
    ///
    /// If `rows` names a column that the table does not have.
    pub fn of(table: &str, columns: &[&dyn Declare], rows: &RowMarks) -> Self {
        let at = |key: &str| -> Option<u16> {
            if key.is_empty() {
                return None;
            }
            let i = columns.iter().position(|c| c.key() == key);
            let Some(i) = i else {
                panic!("the table {table} has no column {key}")
            };
            u16::try_from(i).ok()
        };
        let with = |role: Role| -> Option<u16> {
            let i = columns.iter().position(|c| c.role() == Some(role))?;
            u16::try_from(i).ok()
        };
        let first_position = || -> Option<u16> {
            let i = columns
                .iter()
                .position(|c| c.kind() == ValueKind::Position)?;
            u16::try_from(i).ok()
        };
        let position = with(Role::Position).or_else(first_position);
        TableMarks {
            label: at(rows.label),
            sort: at(rows.sort).map(|c| (c, rows.order)),
            position,
            latitude: with(Role::Latitude).filter(|_| position.is_none()),
            longitude: with(Role::Longitude).filter(|_| position.is_none()),
            altitude: with(Role::Altitude),
            course: with(Role::Course),
            speed: with(Role::Speed),
        }
    }
    /// True if nothing is marked.
    pub fn is_empty(&self) -> bool {
        *self == TableMarks::default()
    }
}
