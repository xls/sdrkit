//! What a front end needs to show a stack without knowing it: the kind of content of the stack, the media that it
//! gives, the column that names a row, the default order of the rows, the columns that hold a place on the map, the
//! tables that hold messages, calls, nodes, groups or a log with the columns of a message and of its delivery, and the
//! settings that choose what a picture shows. A stack states them in its interface description, or they follow from
//! what it declares. The key readings of a stack and the table that gives the values of a command argument are marks
//! of a field ([`FieldDecl::reading`](crate::FieldDecl::reading), [`RefersTo`](crate::RefersTo)).

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

/// What a column holds, for a front end that places a row on a map or shows a row as a message or a call.
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
    /// Who sent the message or made the call: a call sign, an address, a node, a unit.
    Sender,
    /// To whom the message or the call goes: a call sign, an address, a group, a pager.
    Addressee,
    /// The text of the message for people.
    Text,
    /// When the row was received: for a table with several columns of the time kind, or for a column of seconds of
    /// signal (unit `s`) in a table without a column of the time kind. Without it, the first column of the time kind
    /// of a table with a role is the time.
    Time,
    /// Whether this station sent the message of the row: a column of the bool kind, true for an own message. Without
    /// it, in a table with a column of the role [`Role::Delivery`], a row with a delivery state is an own message.
    Own,
    /// How far the delivery of an own message is: a column of the choice kind (queued, sent, acknowledged, failed).
    /// A received message has no value.
    Delivery,
    /// Why the delivery of an own message failed, as text for people. A row has a value only after a failure.
    Failure,
}

/// What the rows of a table are, for a front end that shows them in a view made for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TableRole {
    /// Each row is a message, newest first: the main log of messages of the stack.
    Messages,
    /// Each row is a call or a transmission: who, to whom, when.
    Calls,
    /// Each row is a station, a node or a unit that was heard.
    Nodes,
    /// Each row is a group that stations talk in: a talkgroup, a channel, a net.
    Groups,
    /// Each row is a frame, a packet or a report that arrived: rows come over time, and the newest matter most. A
    /// front end shows the table as a running log. A stack can have several logs.
    Log,
}

impl TableRole {
    /// The roles, in the order of their numbers in the C interface (from 1).
    pub const ALL: [TableRole; 5] = [
        TableRole::Messages,
        TableRole::Calls,
        TableRole::Nodes,
        TableRole::Groups,
        TableRole::Log,
    ];
    /// The name of the role in the registry record.
    pub const fn key(self) -> &'static str {
        match self {
            TableRole::Messages => "messages",
            TableRole::Calls => "calls",
            TableRole::Nodes => "nodes",
            TableRole::Groups => "groups",
            TableRole::Log => "log",
        }
    }
    /// True for a role that at most one table of an owner has. A stack can have several logs.
    pub const fn single(self) -> bool {
        !matches!(self, TableRole::Log)
    }
}

/// What a setting chooses, for a front end that puts its control next to what it acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingRole {
    /// The setting chooses whose picture the stack shows: an instrument, a product, a satellite channel group.
    PictureSource,
    /// The setting chooses the view of the picture shown: a channel, a composite, a projection.
    PictureView,
}

impl SettingRole {
    /// The roles, in the order of their numbers in the C interface (from 1).
    pub const ALL: [SettingRole; 2] = [SettingRole::PictureSource, SettingRole::PictureView];
    /// The name of the role in the registry record.
    pub const fn key(self) -> &'static str {
        match self {
            SettingRole::PictureSource => "picture_source",
            SettingRole::PictureView => "picture_view",
        }
    }
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
    /// What the rows are. `None` for a table that states nothing.
    pub role: Option<TableRole>,
}

impl RowMarks {
    /// Nothing stated.
    pub const NONE: RowMarks = RowMarks {
        label: "",
        sort: "",
        order: SortOrder::Descending,
        role: None,
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
    /// What the rows are.
    pub role: Option<TableRole>,
    /// The column of the sender of a message or a call.
    pub sender: Option<u16>,
    /// The column of the addressee of a message or a call.
    pub addressee: Option<u16>,
    /// The column of the text of a message.
    pub text: Option<u16>,
    /// The column of the time of a row. Only a table with a role or a column with [`Role::Time`] has it.
    pub time: Option<u16>,
    /// The column that says whether this station sent the message of a row.
    pub own: Option<u16>,
    /// The column of the delivery state of an own message.
    pub delivery: Option<u16>,
    /// The column of the reason of a failed delivery.
    pub failure: Option<u16>,
}

impl TableMarks {
    /// The marks of a table with the columns `columns` that states `rows`. The position column is the column with
    /// [`Role::Position`], else the first column of the position kind. Each other place column is the first column
    /// with its role, and so are the sender, the addressee, the text and the columns of a delivery. The time column is the column with
    /// [`Role::Time`], else, in a table with a role, the first column of the time kind.
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
        let first_time = || -> Option<u16> {
            rows.role?;
            let i = columns.iter().position(|c| c.kind() == ValueKind::Time)?;
            u16::try_from(i).ok()
        };
        TableMarks {
            label: at(rows.label),
            sort: at(rows.sort).map(|c| (c, rows.order)),
            position,
            latitude: with(Role::Latitude).filter(|_| position.is_none()),
            longitude: with(Role::Longitude).filter(|_| position.is_none()),
            altitude: with(Role::Altitude),
            course: with(Role::Course),
            speed: with(Role::Speed),
            role: rows.role,
            sender: with(Role::Sender),
            addressee: with(Role::Addressee),
            text: with(Role::Text),
            time: with(Role::Time).or_else(first_time),
            own: with(Role::Own),
            delivery: with(Role::Delivery),
            failure: with(Role::Failure),
        }
    }
    /// True if nothing is marked.
    pub fn is_empty(&self) -> bool {
        *self == TableMarks::default()
    }
}
