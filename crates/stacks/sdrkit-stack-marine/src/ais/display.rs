//! The AIS display (`ais.stations`): the station table, receptions, counters, alerts, satellite analysis and hopping, published as
//! declared fields, tables, map points and events.

use super::alerts::{Alert, AlertEngine, AlertKind, BadText, Filter};
use super::demod::Layer;
use super::message::{self, StationClass};
use super::receiver::{
    placement, AisHints, AisLayerEvent, ChannelFilter, ChannelLevel, ChannelLimit, Hopper,
    Reception, CHANNELS, REPORT_S,
};
use super::satellite::{self, DopplerTrack, Elements, Rx, Snapshot};
use super::station::{self as st, channel_number, MmsiKey, Station, StationTable};
use crate::report::{Clock, Hearing};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Fields, Hertz, Seconds, Table, Time,
};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Choice, ConfigError, Event, HostRequest, Record, Results, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
    Timed,
};
use sdrkit_util_geo::{Observer, Position};
use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

/// The most sentences kept.
pub const SENTENCE_CAP: usize = 50;
/// The most receptions listed.
pub const RECEPTION_CAP: usize = 500;
/// The most alerts listed.
pub const ALERT_CAP: usize = 50;
/// The most receptions kept for the satellite analysis.
pub const ANALYSIS_CAP: usize = 20_000;
/// The most stations whose track is published.
pub const TRACKS: usize = 25;

// Fields, tables and events.

sdrkit_core::choice_enum! {
    /// A fence crossed.
    pub enum Transition {
        /// The vessel entered the fence.
        Enter = ("enter", "Entered"),
        /// The vessel left the fence.
        Exit = ("exit", "Left"),
    }
}
/// The result fields of the AIS stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisFields {
    /// The reception state of a display.
    #[field(label = "State")]
    pub state: Hearing,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Bursts detected.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Messages found on another frequency candidate or part of a burst.
    #[field(label = "Recovered")]
    pub recovered: i64,
    /// Messages found after cancelling a stronger burst.
    #[field(label = "Cancelled")]
    pub cancelled: i64,
    /// Recovered and cancelled messages together.
    #[field(label = "Collision recoveries")]
    pub collision_recoveries: i64,
    /// Stations in the table.
    #[field(label = "Stations")]
    pub stations: i64,
    /// Stations with a position.
    #[field(label = "With a position")]
    pub with_position: i64,
    /// Stations that sent message 27.
    #[field(label = "Long-range stations")]
    pub long_range_stations: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
    /// Signal time since the start.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
    /// The view filter hides some stations.
    #[field(label = "Some stations hidden")]
    pub filtered: bool,
    /// Names in the name table.
    #[field(label = "Names")]
    pub names: i64,
    /// The start of the next pass of the satellite over the station.
    #[field(label = "Next pass")]
    pub next_pass: Option<Time>,
    /// The highest elevation of the next pass.
    #[field(label = "Next pass elevation", unit = "°")]
    pub next_pass_elevation_deg: Option<f64>,
}

/// A row of the AIS station table. The row key is the MMSI.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisStationRow<'a> {
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The MMSI, nine digits.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The ship type in words, else the class.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The position came from message 27.
    #[field(label = "Long-range position")]
    pub long_range_position: bool,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The navigational status in words.
    #[field(label = "Status")]
    pub status: Option<&'a str>,
    /// The destination.
    #[field(label = "Destination")]
    pub destination: Option<&'a str>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// The stations heard.
pub static STATION_TABLE: Table<AisStationRow<'static>> =
    Table::new("stations", "Stations", st::MAX_STATIONS);

/// A row of the track points of the stations whose track is shown. The row key is the MMSI, `/` and the time of the point.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisTrackPointRow<'a> {
    /// The station a track point belongs to.
    #[field(label = "Station", refers_to = "stations")]
    pub station: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
}

/// The track points.
pub static TRACK_POINTS: Table<AisTrackPointRow<'static>> =
    Table::new("track_points", "Tracks", TRACKS * st::MAX_TRACK);

/// A row of the count of messages by type. The row key is the message type in two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisCounterRow<'a> {
    /// The message type name.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// Messages on AIS 1.
    #[field(label = "A")]
    pub count_a: i64,
    /// Messages on AIS 2.
    #[field(label = "B")]
    pub count_b: i64,
    /// Messages on channel 75.
    #[field(label = "C")]
    pub count_c: i64,
    /// Messages on channel 76.
    #[field(label = "D")]
    pub count_d: i64,
    /// Messages on every channel.
    #[field(label = "Total")]
    pub total: i64,
}

/// The messages by type and channel.
pub static COUNTERS: Table<AisCounterRow<'static>> = Table::new("counters", "Messages by type", 32);

/// A row of the channel table of the AIS stack. The row key is the letter of the channel.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisChannelRow<'a> {
    /// The channel's name.
    #[field(label = "Channel")]
    pub name: &'a str,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Good messages in the last minute.
    #[field(label = "Per minute", unit = "/min")]
    pub messages_per_min: f64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// The Doppler of the channel's bursts now (satellite mode).
    #[field(label = "Doppler", unit = "Hz")]
    pub doppler_hz: Option<Hertz>,
    /// The rate of the Doppler (satellite mode).
    #[field(label = "Doppler rate", unit = "Hz/s")]
    pub doppler_rate_hz_s: Option<f64>,
}

/// The AIS channels.
pub static CHANNEL_TABLE: Table<AisChannelRow<'static>> = Table::new("channels", "Channels", 4);

/// A row of the NMEA sentence table. The row key is the place of the sentence in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisSentenceRow<'a> {
    /// An `!AIVDM` sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
}

/// The NMEA sentences, newest first.
pub static SENTENCES: Table<AisSentenceRow<'static>> =
    Table::new("sentences", "Sentences", SENTENCE_CAP);

/// A row of the reception table. The row key is the place of the reception in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisReceptionRow<'a> {
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// The message type.
    #[field(label = "Type")]
    pub msg_type: i64,
    /// The MMSI, nine digits.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// How a message was found.
    #[field(label = "Found")]
    pub layer: Layer,
}

/// The receptions, newest first.
pub static RECEPTIONS: Table<AisReceptionRow<'static>> =
    Table::new("receptions", "Receptions", RECEPTION_CAP);

/// A row of the alert table. The row key is the place of the alert in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisAlertRow<'a> {
    /// When (wall-clock time, when known).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// What an alert is about.
    #[field(label = "Kind")]
    pub kind: AlertKind,
    /// The fence of an enter or exit.
    #[field(label = "Fence")]
    pub fence: Option<&'a str>,
    /// The MMSI, nine digits.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The distance from the station (CPA) or the fence.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: f64,
    /// The distance at the closest point of approach.
    #[field(label = "CPA", unit = "m")]
    pub cpa_m: Option<f64>,
    /// The time to the closest point of approach.
    #[field(label = "TCPA", unit = "s")]
    pub tcpa: Option<Seconds>,
}

/// The alerts, newest first.
pub static ALERTS: Table<AisAlertRow<'static>> = Table::new("alerts", "Alerts", ALERT_CAP);

/// A row of the hop plan of a receiver that is narrower than the four channels. The row key is the number of the step.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisHopRow {
    /// The centre of a hopping step.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The dwell of a hopping step (none for one step that covers all).
    #[field(label = "Dwell", unit = "s")]
    pub dwell: Option<Seconds>,
}

/// The hop plan.
pub static HOP_PLAN: Table<AisHopRow> = Table::new("hop_plan", "Hopping plan", 4);

/// The row of the selected station, with every value that the station sent. The row key is the MMSI.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisSelectedRow<'a> {
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The MMSI, nine digits.
    #[field(label = "MMSI")]
    pub mmsi: &'a str,
    /// The station class.
    #[field(label = "Class")]
    pub class: &'a str,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The IMO number.
    #[field(label = "IMO")]
    pub imo: Option<i64>,
    /// The ship type in words, else the class.
    #[field(label = "Type")]
    pub r#type: Option<&'a str>,
    /// The destination.
    #[field(label = "Destination")]
    pub destination: Option<&'a str>,
    /// The estimated arrival, "MM-DD HH:MM UTC".
    #[field(label = "ETA")]
    pub eta: Option<&'a str>,
    /// The draught.
    #[field(label = "Draught", unit = "m")]
    pub draught_m: Option<f64>,
    /// The length.
    #[field(label = "Length", unit = "m")]
    pub length_m: Option<f64>,
    /// The beam.
    #[field(label = "Beam", unit = "m")]
    pub beam_m: Option<f64>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The true heading.
    #[field(label = "Heading", unit = "°")]
    pub heading_deg: Option<f64>,
    /// The rate of turn.
    #[field(label = "Rate of turn", unit = "°/min")]
    pub rate_of_turn_deg_min: Option<f64>,
    /// The altitude of a SAR aircraft.
    #[field(label = "Altitude", unit = "m")]
    pub altitude_m: Option<f64>,
    /// The navigational status in words.
    #[field(label = "Status")]
    pub status: Option<&'a str>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The type of the last message.
    #[field(label = "Last message type")]
    pub last_type: i64,
    /// The channel or talk group of a call.
    #[field(shared = keys::CHANNEL)]
    pub channel: &'a str,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// It sent message 27.
    #[field(label = "Long range")]
    pub long_range: bool,
    /// The position latency flag of message 27.
    #[field(label = "Position latency")]
    pub latency: Option<bool>,
    /// The last sentences, one per line.
    #[field(label = "Sentences")]
    pub sentences: &'a str,
}

/// The selected station.
pub static SELECTED: Table<AisSelectedRow<'static>> = Table::new("selected", "Selected station", 1);

/// The fields of the event of a station heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisVesselEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The call sign of a station heard.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The ship and cargo type code.
    #[field(label = "Ship type")]
    pub ship_type: Option<i64>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The navigational status code.
    #[field(label = "Navigational status")]
    pub nav_status: Option<i64>,
    /// The station class.
    #[field(label = "Class")]
    pub class: &'a str,
    /// The destination.
    #[field(label = "Destination")]
    pub destination: &'a str,
}

/// A station was heard.
pub static VESSEL: EventKind<AisVesselEvent<'static>> = EventKind::new("ais.vessel", "Vessel");

/// The fields of the event of an NMEA sentence.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisNmeaEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// An `!AIVDM` sentence.
    #[field(label = "Sentence")]
    pub sentence: &'a str,
}

/// An NMEA sentence of a message.
pub static NMEA: EventKind<AisNmeaEvent<'static>> = EventKind::new("ais.nmea", "AIS sentence");

/// The fields of the event of a closest point of approach alert.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisCpaEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The distance from the station (CPA) or the fence.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: f64,
    /// The distance at the closest point of approach.
    #[field(label = "CPA", unit = "m")]
    pub cpa_m: Option<f64>,
    /// The time to the closest point of approach.
    #[field(label = "TCPA", unit = "s")]
    pub tcpa: Option<Seconds>,
}

/// A station comes closer than the limit.
pub static CPA_ALERT: EventKind<AisCpaEvent<'static>> = EventKind::new("ais.cpa", "CPA alert");

/// The fields of the event of a geofence alert.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AisGeofenceEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The station name, else its MMSI.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The distance from the station (CPA) or the fence.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: f64,
    /// The fence of an enter or exit.
    #[field(label = "Fence")]
    pub fence: &'a str,
    /// A fence crossed: enter or exit.
    #[field(label = "Transition")]
    pub transition: Transition,
}

/// A station entered or left a fence.
pub static GEOFENCE_ALERT: EventKind<AisGeofenceEvent<'static>> =
    EventKind::new("ais.geofence", "Geofence alert");

// The rows of the logs.

/// One sentence of the sentence log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SentenceRow {
    /// When.
    pub at: Stamp,
    /// The sentence.
    pub text: String,
}

impl Shift for SentenceRow {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Shaped for SentenceRow {
    type Row = AisSentenceRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AisSentenceRow {
            sentence: &self.text,
        });
    }
}

/// One reception of the reception log.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceptionRow {
    /// When.
    pub at: Stamp,
    /// The channel letter.
    pub channel: char,
    /// The message type.
    pub msg_type: u8,
    /// The station.
    pub mmsi: u32,
    /// The SNR in 48 kHz, dB.
    pub snr_db: f64,
    /// The carrier, Hz from the channel centre.
    pub offset_hz: f64,
    /// How it was found.
    pub layer: Layer,
}

impl Shaped for ReceptionRow {
    type Row = AisReceptionRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AisReceptionRow {
            time: self.at.wall.map(Time),
            channel: channel_number(self.channel),
            msg_type: i64::from(self.msg_type),
            mmsi: MmsiKey(self.mmsi).digits(&mut [0; 9]),
            snr_db: self.snr_db,
            offset_hz: Hertz(self.offset_hz),
            layer: self.layer,
        });
    }
}

impl Shaped for Alert {
    type Row = AisAlertRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AisAlertRow {
            time: self.at.wall.map(Time),
            kind: self.kind,
            fence: (!self.fence.is_empty()).then_some(self.fence.as_str()),
            mmsi: MmsiKey(self.mmsi).digits(&mut [0; 9]),
            name: &self.name,
            position: (self.lat, self.lon),
            distance_m: self.distance_m,
            cpa_m: self.dcpa_m,
            tcpa: self.tcpa_s.map(Seconds),
        });
    }
}

/// What the display keeps across sessions: the station table and the last sentences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AisTables {
    /// The stations.
    pub stations: StationTable,
    /// The last sentences.
    pub sentences: LogRing<SentenceRow>,
}

impl Default for AisTables {
    fn default() -> Self {
        AisTables {
            stations: StationTable::default(),
            sentences: LogRing::new(SENTENCE_CAP),
        }
    }
}

impl Shift for AisTables {
    fn shift_times(&mut self, by_s: f64) {
        self.stations.shift_times(by_s);
        self.sentences.shift_times(by_s);
    }
}

impl Tables for AisTables {
    const FORMAT: StateFormat = StateFormat::new("marine.ais_stations", 1);
    fn clear(&mut self) {
        self.stations.stations.clear();
        self.sentences.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.stations.stations.adopt(saved.stations.stations);
        self.sentences.adopt(saved.sentences);
    }
}

// The settings and commands.

/// Satellite mode.
pub const SATELLITE: Setting<field::Bool> = Setting::new("satellite", "Satellite mode", "", false)
    .help(
        "Wider filters, Doppler candidates and successive cancellation for reception from orbit.",
    );
/// Hopping.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Hopping", "", false)
    .help("Alternates between AIS 1 + 2 and 75 + 76 when the radio cannot hold all four.");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time base", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.")
    .section("Satellite");
/// The satellite's two-line elements.
pub const SATELLITE_TLE: Setting<field::Text> = Setting::new(
    "satellite_tle",
    "Satellite elements",
    "",
    "",
)
.help("Two-line elements of the satellite (with checksums), for predicted Doppler and passes.")
.section("Satellite");
/// The least elevation of a pass.
pub const MIN_ELEVATION: Setting<field::F64> =
    Setting::new("min_elevation", "Minimum elevation", "°", 0.0)
        .range(0.0, 40.0, 0.0)
        .section("Satellite");
/// Predict the next pass.
pub const ARM_PASSES: Setting<field::Bool> =
    Setting::new("arm_passes", "Arm the passes", "", false)
        .help("Predicts the next pass over the station. The station position is needed.")
        .section("Satellite");
/// CPA alerts.
pub const CPA: Setting<field::Bool> = Setting::new("cpa", "CPA alerts", "", false)
    .help("Alerts for vessels that will pass close to the station. The station position is needed.")
    .section("Alerts");
/// The CPA limit.
pub const CPA_NM: Setting<field::F64> = Setting::new("cpa_nm", "CPA limit", "nm", 0.5)
    .range(0.05, 5.0, 0.0)
    .section("Alerts");
/// The TCPA limit.
pub const TCPA_MIN: Setting<field::F64> = Setting::new("tcpa_min", "TCPA limit", "min", 15.0)
    .range(1.0, 60.0, 0.0)
    .section("Alerts");
/// The time before a vessel can raise a CPA alert again.
pub const CPA_COOLDOWN_MIN: Setting<field::F64> =
    Setting::new("cpa_cooldown_min", "CPA cool-down", "min", 10.0)
        .range(1.0, 120.0, 0.0)
        .section("Alerts");
/// The own ship moves.
pub const OWN_MOVING: Setting<field::Bool> =
    Setting::new("own_moving", "Own ship moving", "", false).section("Alerts");
/// The own speed.
pub const OWN_SOG_KN: Setting<field::F64> = Setting::new("own_sog_kn", "Own speed", "kn", 0.0)
    .range(0.0, 40.0, 0.0)
    .section("Alerts");
/// The own course.
pub const OWN_COG_DEG: Setting<field::F64> = Setting::new("own_cog_deg", "Own course", "°", 0.0)
    .range(0.0, 359.0, 0.0)
    .section("Alerts");
/// The geofences.
pub const GEOFENCES: Setting<field::Text> = Setting::new("geofences", "Geofences", "", "")
    .help("One per line: name;lat,lon,radius_nm or name;poly;lat,lon lat,lon ...")
    .section("Alerts");
/// The view filter.
pub const FILTER: Setting<field::Text> =
    Setting::new("filter", "Filter", "", "").help("classes=63;groups=1023;moving=0;sog=0.5");
/// Send each sentence as an event.
pub const NMEA_OUT: Setting<field::Bool> = Setting::new("nmea_out", "Send AIS as NMEA", "", true);
/// The name table.
pub const NAMES_TEXT: Setting<field::Text> = Setting::new("names", "Name table", "", "")
    .help("Ship and station names by MMSI to import, one record per line.");

sdrkit_core::settings! {
    /// The settings of [`Stations`].
    pub enum StationsSetting {
        /// Which channels the input can hold.
        Channels = super::receiver::LIMIT,
        /// Satellite mode.
        Satellite = SATELLITE,
        /// Hopping.
        Hop = HOP,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
        /// The satellite's elements.
        SatelliteTle = SATELLITE_TLE,
        /// The least elevation of a pass.
        MinElevation = MIN_ELEVATION,
        /// Predict the next pass.
        ArmPasses = ARM_PASSES,
        /// CPA alerts.
        Cpa = CPA,
        /// The CPA limit.
        CpaNm = CPA_NM,
        /// The TCPA limit.
        TcpaMin = TCPA_MIN,
        /// The CPA cool-down.
        CpaCooldownMin = CPA_COOLDOWN_MIN,
        /// The own ship moves.
        OwnMoving = OWN_MOVING,
        /// The own speed.
        OwnSogKn = OWN_SOG_KN,
        /// The own course.
        OwnCogDeg = OWN_COG_DEG,
        /// The geofences.
        Geofences = GEOFENCES,
        /// The view filter.
        Filter = FILTER,
        /// Send each sentence as an event.
        NmeaOut = NMEA_OUT,
        /// The name table.
        Names = NAMES_TEXT,
    }
}

/// Shows one station in full in the `selected` table.
pub const SELECT: CommandKind<AisStationArgs<'static>> =
    CommandKind::new("select", "Select").help("Shows one station in full.");

const STATION_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of("stations");

/// The arguments of the commands `select` and `remove` of the AIS stack.
#[derive(Arguments)]
pub struct AisStationArgs<'a> {
    /// The row key of the station.
    #[arg(shared = STATION_ROW)]
    pub row: &'a str,
}

/// Forgets one station.
pub const REMOVE: CommandKind<AisStationArgs<'static>> = objcmd::remove_kind();

sdrkit_core::commands! {
    /// The commands of [`Stations`].
    pub enum StationsCommand {
        /// Select a station.
        Select = SELECT.command,
        /// Forget every station, reception and alert.
        Clear = objcmd::CLEAR,
        /// Forget one station.
        Remove = REMOVE.command,
    }
}

/// The next pass of the satellite, kept until it ends.
#[derive(Clone, Copy, Debug)]
struct NextPass {
    aos: f64,
    los: f64,
    max_el: f64,
}

/// The station table, alerts, analysis and results (`ais.stations`).
pub struct Stations {
    p: Published,
    clock: Clock,
    host_clock: sdrkit_core::Clock,
    live: bool,
    tables: AisTables,
    geofences: String,
    filter_text: String,
    tle: String,
    names_text: String,
    time_base: f64,
    alerts: AlertEngine,
    alert_log: LogRing<Alert>,
    filter: Filter,
    names: IdNames,
    receptions: LogRing<ReceptionRow>,
    rx_all: LogRing<Rx>,
    good: [u64; 4],
    bad_ch: [u64; 4],
    counts: [[u64; 29]; 4],
    bad: u64,
    recovered: u64,
    cancelled: u64,
    collisions: u64,
    levels: [ChannelLevel; 4],
    limit: ChannelLimit,
    in_view: [bool; 4],
    noise_scale: f64,
    track_keys: Vec<String>,
    key_buf: String,
    per_min: [VecDeque<f64>; 4],
    doppler: [DopplerTrack; 4],
    snr: Option<f64>,
    offset: Option<f64>,
    last_good: f64,
    last_burst: f64,
    last_detected: u64,
    state: Hearing,
    reports: u64,
    vessel_event: BTreeMap<u32, f64>,
    nmea_out: bool,
    satellite: bool,
    sat: Option<Elements>,
    min_elevation: f64,
    arm_passes: bool,
    next_pass: Option<NextPass>,
    next_pass_tried: f64,
    select: Option<u32>,
    pending_events: Vec<Event>,
    pending_requests: Vec<HostRequest>,
    pass_gap_s: f64,
    station: Option<Position>,
    new_logged: usize,
    /// Hopping: on, the plan's state, its clock (report periods counted, so it runs on through the retunes it requests), the
    /// input rate and the radio's centre.
    hop: bool,
    hopper: Option<Hopper>,
    hop_clock: f64,
    hop_asked: Option<f64>,
    input_rate: f64,
    input_centre: f64,
}

impl Default for Stations {
    fn default() -> Self {
        let mut p = Published::from_decl(Arc::new(crate::typed::shape("ais").results_decl()));
        p.results_mut().set_status(&Status {
            lock_state: Hearing::Searching.lock_state(),
            summary: "AIS: searching".into(),
            ..Status::default()
        });
        Stations {
            p,
            clock: Clock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            live: false,
            tables: AisTables::default(),
            geofences: String::new(),
            filter_text: String::new(),
            tle: String::new(),
            names_text: String::new(),
            time_base: 0.0,
            alerts: AlertEngine::new(),
            alert_log: LogRing::new(ALERT_CAP),
            filter: Filter::default(),
            names: IdNames::new(IdKind::Mmsi),
            receptions: LogRing::new(RECEPTION_CAP),
            rx_all: LogRing::new(ANALYSIS_CAP),
            good: [0; 4],
            bad_ch: [0; 4],
            counts: [[0; 29]; 4],
            bad: 0,
            recovered: 0,
            cancelled: 0,
            collisions: 0,
            levels: Default::default(),
            limit: ChannelLimit::All,
            in_view: [false; 4],
            noise_scale: 1.0,
            track_keys: Vec::new(),
            key_buf: String::new(),
            per_min: Default::default(),
            doppler: Default::default(),
            snr: None,
            offset: None,
            last_good: f64::NEG_INFINITY,
            last_burst: f64::NEG_INFINITY,
            last_detected: 0,
            state: Hearing::Searching,
            reports: 0,
            vessel_event: BTreeMap::new(),
            nmea_out: true,
            satellite: false,
            sat: None,
            min_elevation: 0.0,
            arm_passes: false,
            next_pass: None,
            next_pass_tried: f64::NEG_INFINITY,
            select: None,
            pending_events: Vec::new(),
            pending_requests: Vec::new(),
            pass_gap_s: 900.0,
            station: None,
            new_logged: 0,
            hop: false,
            hopper: None,
            hop_clock: 0.0,
            hop_asked: None,
            input_rate: 0.0,
            input_centre: 0.0,
        }
    }
}


/// The class of a station in the vessel event.
fn class_text(c: StationClass) -> &'static str {
    match c {
        StationClass::A => "A",
        StationClass::B => "B",
        StationClass::BaseStation => "base",
        StationClass::AidToNavigation => "aid",
        StationClass::SarAircraft => "SAR",
        StationClass::Other => "other",
    }
}

/// The urgency of a station and the reason for people (ITU-R M.585-9 Annex 1 clause 2: AIS-SART 970xxyyyy, MOB 972xxyyyy,
/// EPIRB-AIS 974xxyyyy; ITU-R M.1371-5 Annex 8 Table 50: navigational status 14 is AIS-SART active).
pub fn urgency(s: &Station) -> (Urgency, Option<&'static str>) {
    match s.mmsi / 1_000_000 {
        970 => (Urgency::Emergency, Some("AIS-SART")),
        972 => (Urgency::Emergency, Some("man overboard")),
        974 => (Urgency::Emergency, Some("EPIRB-AIS")),
        _ if s.nav_status == Some(14) => (Urgency::Emergency, Some("AIS-SART active")),
        _ => (Urgency::None, None),
    }
}

impl Stations {
    /// The station table.
    pub fn table(&self) -> &StationTable {
        &self.tables.stations
    }

    /// The receptions listed (oldest first).
    pub fn receptions(&self) -> impl Iterator<Item = &ReceptionRow> {
        self.receptions.iter()
    }

    /// The satellite analysis of every reception kept.
    pub fn analysis(&self) -> satellite::Analysis {
        let rx: Vec<Rx> = self.rx_all.iter().cloned().collect();
        satellite::analyse(
            &rx,
            self.pass_gap_s,
            self.sat.as_ref(),
            &[],
            self.min_elevation,
        )
    }

    /// The id of the station table.
    fn table_id(&self) -> sdrkit_core::TableId {
        self.p
            .results()
            .decl()
            .table_id(STATION_TABLE.key)
            .expect("the declaration holds the station table")
    }

    fn clear(&mut self) {
        self.tables.clear();
        self.receptions.clear();
        self.rx_all.clear();
        self.alerts.clear();
        self.alert_log.clear();
        self.good = [0; 4];
        self.bad_ch = [0; 4];
        self.counts = [[0; 29]; 4];
        self.bad = 0;
        self.recovered = 0;
        self.cancelled = 0;
        self.collisions = 0;
        self.new_logged = 0;
    }

    fn reception(&mut self, r: &Reception, t: f64) {
        let m = &r.message;
        let at = self.clock.stamp(t);
        let ci = CHANNELS.iter().position(|c| c.0 == r.channel).unwrap_or(0);
        self.good[ci] += 1;
        self.counts[ci][(m.msg_type as usize).min(28)] += 1;
        match r.layer {
            Layer::Direct => {}
            Layer::Recovered => {
                self.recovered += 1;
                self.collisions += 1;
            }
            Layer::Cancelled => {
                self.cancelled += 1;
                self.collisions += 1;
            }
        }
        self.per_min[ci].push_back(t);
        self.doppler[ci].add(t, r.offset_hz);
        self.snr = Some(self.snr.map_or(r.snr_db, |s| s + 0.15 * (r.snr_db - s)));
        self.offset = Some(
            self.offset
                .map_or(r.offset_hz, |s| s + 0.15 * (r.offset_hz - s)),
        );
        self.last_good = t;
        self.last_burst = t;
        let new = self.tables.stations.update(m, at, r.channel, &r.sentences);
        if let Some(s) = self.tables.stations.stations.get_mut(&MmsiKey(m.mmsi)) {
            if s.name.is_empty() {
                if let Some(n) = self.names.name(&format!("{:09}", m.mmsi)) {
                    s.name = n.to_string();
                }
            }
        }
        if new && self.new_logged < 40 {
            self.new_logged += 1;
            self.p
                .results_mut()
                .log(format!("AIS: new station {:09}", m.mmsi));
        }
        let freq = CHANNELS[ci].2;
        for l in &r.sentences {
            if self.nmea_out {
                self.pending_events.push(NMEA.event(
                    t,
                    freq,
                    &AisNmeaEvent {
                        source: MmsiKey(m.mmsi).digits(&mut [0; 9]),
                        family: Family::Vessel,
                        sentence: l,
                    },
                ));
            }
            self.tables.sentences.push(SentenceRow {
                at,
                text: l.clone(),
            });
        }
        self.receptions.push(ReceptionRow {
            at,
            channel: r.channel,
            msg_type: m.msg_type,
            mmsi: m.mmsi,
            snr_db: r.snr_db,
            offset_hz: r.offset_hz,
            layer: r.layer,
        });
        self.rx_all.push(Rx {
            unix: at.wall.unwrap_or(t),
            mmsi: m.mmsi,
            msg_type: m.msg_type,
            snr_db: r.snr_db,
            lat: m.lat,
            lon: m.lon,
        });
        // Alerts and vessel events on a position update.
        if m.position().is_none() {
            return;
        }
        let Some(s) = self.tables.stations.get(m.mmsi) else {
            return;
        };
        for a in self.alerts.check(s, at) {
            self.p.results_mut().log(a.line());
            let mut digits = [0; 9];
            let source = MmsiKey(a.mmsi).digits(&mut digits);
            let e = match a.kind {
                AlertKind::Cpa => CPA_ALERT.event(
                    t,
                    freq,
                    &AisCpaEvent {
                        source,
                        family: Family::Vessel,
                        urgency: Urgency::Urgent,
                        name: &a.name,
                        position: (a.lat, a.lon),
                        distance_m: a.distance_m,
                        cpa_m: a.dcpa_m,
                        tcpa: a.tcpa_s.map(Seconds),
                    },
                ),
                AlertKind::Enter | AlertKind::Exit => GEOFENCE_ALERT.event(
                    t,
                    freq,
                    &AisGeofenceEvent {
                        source,
                        family: Family::Vessel,
                        urgency: Urgency::Urgent,
                        name: &a.name,
                        position: (a.lat, a.lon),
                        distance_m: a.distance_m,
                        fence: &a.fence,
                        transition: if a.kind == AlertKind::Enter {
                            Transition::Enter
                        } else {
                            Transition::Exit
                        },
                    },
                ),
            };
            self.pending_events.push(e);
            self.alert_log.push(a);
        }
        let last = self
            .vessel_event
            .get(&m.mmsi)
            .copied()
            .unwrap_or(f64::NEG_INFINITY);
        if t - last < 5.0 {
            return;
        }
        self.vessel_event.insert(m.mmsi, t);
        if self.vessel_event.len() > 4000 {
            let cut = t - 600.0;
            self.vessel_event.retain(|_, v| *v > cut);
        }
        let (urgency, why) = urgency(s);
        let e = VESSEL.event(
            t,
            freq,
            &AisVesselEvent {
                source: MmsiKey(s.mmsi).digits(&mut [0; 9]),
                family: Family::Vessel,
                urgency,
                text: why,
                name: &s.name,
                callsign: &s.callsign,
                ship_type: s.ship_type.map(i64::from),
                position: s.lat.zip(s.lon),
                speed_kt: s.speed_kn(),
                course_deg: s.cog_deg,
                nav_status: s.nav_status.map(i64::from),
                class: class_text(s.class),
                destination: &s.destination,
            },
        );
        self.pending_events.push(e);
    }

    fn hints(&self) -> AisHints {
        let mut h = AisHints::default();
        if !self.satellite {
            return h;
        }
        let now = self.clock.now();
        let snap = self
            .sat
            .as_ref()
            .zip(self.clock.wall(now))
            .and_then(|(sat, unix)| Snapshot::of(sat, unix));
        for (i, item) in h.0.iter_mut().enumerate() {
            *item = self.doppler[i].hints(now, 30.0, 6);
            let Some(snap) = &snap else {
                continue;
            };
            let mut n = 0;
            for r in self.tables.stations.stations.iter() {
                if n >= 300 {
                    break;
                }
                let s = r.record;
                let Some(p) = s.position() else {
                    continue;
                };
                if now - s.position_time > 900.0 {
                    continue;
                }
                let (el, dop) = snap.look(p, CHANNELS[i].2);
                if el > -1.0 {
                    item.push(dop);
                    n += 1;
                }
            }
        }
        h
    }

    /// The next pass over the station, searched at most once a minute of signal while the satellite and the station are known.
    fn update_pass(&mut self, now: f64) {
        let (Some(sat), Some(p), Some(unix)) = (&self.sat, self.station, self.clock.wall(now))
        else {
            self.next_pass = None;
            return;
        };
        if self.next_pass.is_some_and(|n| n.los > unix) || now - self.next_pass_tried < 60.0 {
            return;
        }
        self.next_pass_tried = now;
        let obs = Observer::new(p);
        self.next_pass = sat
            .sat
            .next_pass(&obs, unix, unix + 86_400.0, self.min_elevation)
            .ok()
            .flatten()
            .map(|q| NextPass {
                aos: q.aos_unix,
                los: q.los_unix,
                max_el: q.max_elevation_deg,
            });
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        for q in &mut self.per_min {
            while q.front().is_some_and(|&t| now - t > 60.0) {
                q.pop_front();
            }
        }
        let detected: u64 = self.levels.iter().map(|l| l.detected).sum();
        if detected > self.last_detected {
            self.last_burst = now;
            self.last_detected = detected;
        }
        let state = if now - self.last_good <= 10.0 {
            Hearing::Decoding
        } else if now - self.last_burst <= 10.0 {
            Hearing::Signal
        } else {
            Hearing::Searching
        };
        if state != self.state {
            self.p.results_mut().log(match state {
                Hearing::Decoding => "AIS: decoding messages",
                Hearing::Signal => "AIS: signal, no good message",
                Hearing::Searching => "AIS: lost the signal",
            });
            self.state = state;
        }
        if self.arm_passes {
            self.update_pass(now);
        }
        let good: u64 = self.good.iter().sum();
        let (n_st, with_pos, lr) = self.tables.stations.counts();
        for (_, s) in self.tables.stations.stations.records_mut() {
            s.hidden = !self.filter.shows(s);
        }
        // The tracks of the 25 stations heard most recently.
        let tracked: Vec<MmsiKey> = self
            .tables
            .stations
            .stations
            .iter_newest_first()
            .filter(|r| r.record.track.len() > 1 && !r.record.hidden)
            .take(TRACKS)
            .map(|r| *r.key)
            .collect();
        for (k, s) in self.tables.stations.stations.records_mut() {
            s.show_track = tracked.contains(k);
        }
        self.publish_tracks();
        let p = &mut self.p;
        p.put(&AisFields {
            state,
            frames_ok: good as i64,
            frames_bad: self.bad as i64,
            bursts: detected as i64,
            recovered: self.recovered as i64,
            cancelled: self.cancelled as i64,
            collision_recoveries: self.collisions as i64,
            stations: n_st as i64,
            with_position: with_pos as i64,
            long_range_stations: lr as i64,
            snr_db: self.snr,
            offset_hz: self.offset.map(Hertz),
            reports: self.reports as i64,
            signal_time: Seconds(now),
            filtered: self.filter.narrowed(),
            names: self.names.len() as i64,
            next_pass: self.next_pass.map(|n| n.aos).map(Time),
            next_pass_elevation_deg: self.next_pass.map(|n| n.max_el),
        });
        shaped::rows_where(
            &self.tables.stations.stations,
            &STATION_TABLE,
            now,
            p,
            |r| !r.record.hidden,
        );
        shaped::points(&self.tables.stations.stations, "", p.results_mut());
        // Messages by type.
        let mut key = String::new();
        for ty in 1..=28usize {
            let row: [u64; 4] = std::array::from_fn(|c| self.counts[c][ty]);
            let total: u64 = row.iter().sum();
            if total == 0 {
                continue;
            }
            key.clear();
            let _ = write!(key, "{ty:02}");
            let counts = AisCounterRow {
                r#type: message::type_name(ty as u8),
                count_a: row[0] as i64,
                count_b: row[1] as i64,
                count_c: row[2] as i64,
                count_d: row[3] as i64,
                total: total as i64,
            };
            COUNTERS.put(p, &key, &counts);
        }
        // The channels.
        for (i, (l, name, f)) in CHANNELS.iter().enumerate() {
            key.clear();
            key.push(*l);
            if !self.in_view[i] && self.good[i] == 0 {
                p.remove_row(&CHANNEL_TABLE, &key);
                continue;
            }
            let lvl = &self.levels[i];
            let span = now.clamp(5.0, 60.0);
            let pm = self.per_min[i].len() as f64 * 60.0 / span;
            let (dop, rate) = if self.satellite {
                (self.doppler[i].at(now), self.doppler[i].rate())
            } else {
                (None, None)
            };
            CHANNEL_TABLE.put(
                p,
                &key,
                &AisChannelRow {
                    name,
                    freq_hz: Hertz(*f),
                    frames_ok: self.good[i] as i64,
                    frames_bad: self.bad_ch[i] as i64,
                    messages_per_min: pm,
                    level_dbfs: (lvl.level > 0.0).then(|| sdrkit_algo_dsp::meter::power_to_db(lvl.level)),
                    noise_dbfs: lvl.floor.map(|f| sdrkit_algo_dsp::meter::power_to_db(f * self.noise_scale)),
                    doppler_hz: dop.map(Hertz),
                    doppler_rate_hz_s: rate,
                },
            );
        }
        shaped::log_rows(&self.tables.sentences, &SENTENCES, now, p);
        shaped::log_rows(&self.receptions, &RECEPTIONS, now, p);
        shaped::log_rows(&self.alert_log, &ALERTS, now, p);
        let steps = self.hopper.as_ref().map_or(&[][..], |h| h.plan());
        for k in 0..HOP_PLAN.cap {
            key.clear();
            let _ = write!(key, "{k}");
            match steps.get(k) {
                Some(&(c, d)) => {
                    HOP_PLAN.put(
                        p,
                        &key,
                        &AisHopRow {
                            centre_hz: Hertz(c),
                            dwell: d.is_finite().then_some(d).map(Seconds),
                        },
                    );
                }
                None => {
                    p.remove_row(&HOP_PLAN, &key);
                }
            }
        }
        let sel = self.select.map(MmsiKey);
        let shown = sel.and_then(|k| {
            let t = &self.tables.stations.stations;
            Some((k, t.get(&k)?, t.seen(&k)?))
        });
        let held = p
            .results()
            .decl()
            .table_id(SELECTED.key)
            .and_then(|id| p.results().table(id))
            .and_then(|t| t.rows_newest_first().next().map(|(k, _)| k.to_owned()));
        if let Some(k) = held.filter(|k| shown.is_none_or(|(s, _, _)| s.to_string() != *k)) {
            p.remove_row(&SELECTED, &k);
        }
        if let Some((k, s, seen)) = shown {
            key.clear();
            let _ = write!(key, "{k}");
            write_selected(s, seen, p, &key);
        }
        // Status.
        let lv = (0..4)
            .filter(|&i| self.in_view[i])
            .map(|i| self.levels[i].level)
            .fold(0.0, f64::max);
        let mut summary = if state == Hearing::Searching {
            "AIS: searching".to_string()
        } else {
            format!(
                "AIS: {}, {n_st} stations, {good} messages ({} bad)",
                if state == Hearing::Decoding {
                    "decoding"
                } else {
                    "signal"
                },
                self.bad
            )
        };
        if self.station.is_none() {
            if self.alerts.settings.cpa {
                summary.push_str("; CPA alerts need the station position");
            }
            if self.arm_passes {
                summary.push_str("; pass prediction needs the station position");
            }
        } else if self.arm_passes && self.sat.is_some() && self.clock.wall(now).is_none() {
            summary.push_str("; pass prediction needs the time of the recording");
        }
        p.results_mut().set_status(&Status {
            lock_state: state.lock_state(),
            snr_db: self.snr.map(|v| v as f32),
            level_dbfs: (lv > 0.0).then(|| sdrkit_algo_dsp::meter::power_to_db(lv) as f32),
            offset_hz: self.offset,
            summary,
        });
    }

    /// Hopping: one report period on. Requests the host to move the radio (a retune to the centre the plan requires) when the
    /// plan says so, and once at the start when the radio is not where the plan begins.
    fn hop_step(&mut self) {
        if !self.hop || self.input_rate <= 0.0 {
            return;
        }
        self.hop_clock += REPORT_S;
        let h = self
            .hopper
            .get_or_insert_with(|| Hopper::new(self.input_rate, self.hop_clock));
        h.tick(self.hop_clock);
        let want = h.current();
        let dwell = h.dwell().unwrap_or(0.0);
        if want != self.hop_asked {
            self.hop_asked = want;
            if let Some(c) = want {
                if (c - self.input_centre).abs() > 1.0 {
                    let reason = if dwell.is_finite() {
                        format!("AIS hopping, {dwell:.0} s here")
                    } else {
                        "AIS hopping".to_string()
                    };
                    self.pending_requests.push(HostRequest::Retune {
                        tuned_hz: c,
                        reason,
                    });
                }
            }
        }
    }

    /// Writes the track points of the stations whose track is shown, and removes the rows of points that left. Each written
    /// row becomes the newest, so the rows of points that left are the oldest.
    fn publish_tracks(&mut self) {
        let mut written = 0;
        for r in self.tables.stations.stations.iter() {
            let s = r.record;
            if !s.show_track {
                continue;
            }
            for pt in s.track.points() {
                self.key_buf.clear();
                let _ = write!(self.key_buf, "{}/{:.3}", r.key, pt.t);
                let point = AisTrackPointRow {
                    station: self.key_buf.get(..9).unwrap_or(&self.key_buf),
                    position: (pt.position.lat_deg, pt.position.lon_deg),
                    time: self.clock.wall(pt.t).map(Time),
                };
                TRACK_POINTS.put(&mut self.p, &self.key_buf, &point);
                written += 1;
            }
        }
        let Some(id) = self.p.results().decl().table_id(TRACK_POINTS.key) else {
            return;
        };
        let Some(table) = self.p.results().table(id) else {
            return;
        };
        if table.len() <= written {
            return;
        }
        self.track_keys.clear();
        self.track_keys.extend(
            table
                .rows_newest_first()
                .skip(written)
                .map(|(k, _)| k.to_owned()),
        );
        for k in &self.track_keys {
            self.p.remove_row(&TRACK_POINTS, k);
        }
    }
}

/// Writes the cells of the selected station.
fn write_selected(s: &Station, seen: &Seen, p: &mut Published, key: &str) {
    let eta = s.eta.map_or(String::new(), |[mo, d, h, mi]| {
        if h < 24 && mi < 60 {
            format!("{mo:02}-{d:02} {h:02}:{mi:02} UTC")
        } else {
            format!("{mo:02}-{d:02}")
        }
    });
    let size = s
        .dim
        .map(|d| (f64::from(d[0] + d[1]), f64::from(d[2] + d[3])));
    let ship = s.ship_type.map_or(String::new(), |t| {
        format!("{} ({t})", message::ship_type_text(t))
    });
    let sentences: Vec<&str> = s.sentences.iter().map(String::as_str).collect();
    fn text(v: &str) -> Option<&str> {
        (!v.is_empty()).then_some(v)
    }
    SELECTED.put(
        p,
        key,
        &AisSelectedRow {
            name: &s.name,
            mmsi: MmsiKey(s.mmsi).digits(&mut [0; 9]),
            class: s.class.name(),
            callsign: text(&s.callsign),
            imo: s.imo.map(i64::from),
            r#type: text(&ship),
            destination: text(&s.destination),
            eta: text(&eta),
            draught_m: s.draught_m,
            length_m: size.map(|v| v.0),
            beam_m: size.map(|v| v.1),
            position: s.lat.zip(s.lon),
            speed_kt: s.speed_kn(),
            course_deg: s.cog_deg,
            heading_deg: s.heading.map(f64::from),
            rate_of_turn_deg_min: s.rot_deg_min,
            altitude_m: s.altitude_m.map(f64::from),
            status: s.status_text(),
            heard: columns::heard(Some(seen)),
            last_type: i64::from(s.last_type),
            channel: channel_number(s.channel),
            last_heard: columns::last_heard(Some(seen)).map(Time),
            long_range: s.long_range,
            latency: s.latency,
            sentences: &sentences.join("\n"),
        },
    );
}

/// The setting error of a text that does not parse.
fn malformed(id: SettingId, e: BadText) -> SettingError {
    SettingError::Malformed { id, at: Some(e.at) }
}

impl Component for Stations {
    fn info(&self) -> ComponentInfo {
        let mut inputs = vec![Port::typed::<AisLayerEvent>("receptions")];
        for i in 0..4 {
            inputs.push(Port::typed::<ChannelLevel>(&format!("level{i}")));
        }
        // The input itself, for its rate and centre only (the hopping plan).
        inputs.push(Port::iq("iq"));
        ComponentInfo::processor(
            "ais.stations",
            inputs,
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AisHints>("hints").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(StationsSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if let Some(f) = inputs.get(5) {
            if f.rate != self.input_rate {
                self.hopper = None;
            }
            self.input_rate = f.rate;
            self.input_centre = f.centre_hz;
            let place = placement(f.rate, f.centre_hz, f.tuned_hz, self.limit);
            self.in_view = std::array::from_fn(|i| place[i / 2].is_some());
        }
        self.noise_scale = 48e3 / ChannelFilter::new(0).with_satellite(self.satellite).enbw();
        // A new input: its item times start again at 0, the clock runs on.
        self.clock.restart(self.live, self.host_clock.now());
        self.levels = Default::default();
        self.last_detected = 0;
        for d in &mut self.doppler {
            d.clear();
        }
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<AisHints>(), 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
        self.station = ctx.station.position;
        self.alerts.settings.own = self.station;
        self.next_pass = None;
        self.next_pass_tried = f64::NEG_INFINITY;
    }

    fn process(&mut self, io: &mut Io) {
        for e in io.items::<AisLayerEvent>(0) {
            let t = self.clock.at(e.time);
            match &e.value {
                AisLayerEvent::Reception(r) => self.reception(r, t),
                AisLayerEvent::Bad { channel } => {
                    self.bad += 1;
                    self.bad_ch[*channel as usize % 4] += 1;
                    self.last_burst = t;
                }
            }
        }
        for i in 0..4 {
            if let Some(l) = io.items::<ChannelLevel>(i + 1).last() {
                self.clock.at(l.value.clock_s);
                self.levels[i].clone_from(&l.value);
            }
        }
        if io.tick() {
            self.hop_step();
            self.report();
            let h = self.hints();
            io.items_out::<AisHints>(1).push(Timed {
                time: self.clock.now(),
                value: h,
            });
        }
        let now = self.clock.now();
        io.messages_out(0).append(&mut self.pending_events);
        for r in self.pending_requests.drain(..) {
            io.items_out::<HostRequest>(2).push(Timed {
                time: now,
                value: r,
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let a = &mut self.alerts.settings;
        match StationsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            StationsSetting::Channels => {
                self.limit = super::receiver::LIMIT.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            StationsSetting::Cpa => a.cpa = CPA.read(id, v)?,
            StationsSetting::CpaNm => a.cpa_nm = CPA_NM.read(id, v)?,
            StationsSetting::TcpaMin => a.tcpa_min = TCPA_MIN.read(id, v)?,
            StationsSetting::CpaCooldownMin => a.cooldown_min = CPA_COOLDOWN_MIN.read(id, v)?,
            StationsSetting::OwnMoving => a.own_moving = OWN_MOVING.read(id, v)?,
            StationsSetting::OwnSogKn => a.own_sog = OWN_SOG_KN.read(id, v)?,
            StationsSetting::OwnCogDeg => a.own_cog = OWN_COG_DEG.read(id, v)?,
            StationsSetting::Geofences => {
                let t = GEOFENCES.read(id, v)?;
                self.alerts.set_fences(t).map_err(|e| malformed(id, e))?;
                self.geofences = t.to_owned();
            }
            StationsSetting::Filter => {
                let t = FILTER.read(id, v)?;
                self.filter = Filter::parse(t).map_err(|e| malformed(id, e))?;
                self.filter_text = t.to_owned();
            }
            StationsSetting::NmeaOut => self.nmea_out = NMEA_OUT.read(id, v)?,
            StationsSetting::Satellite => {
                self.satellite = SATELLITE.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            StationsSetting::SatelliteTle => {
                let t = SATELLITE_TLE.read(id, v)?;
                self.sat = if t.trim().is_empty() {
                    None
                } else {
                    Some(Elements::parse(t).map_err(|_| SettingError::Malformed { id, at: None })?)
                };
                self.tle = t.to_owned();
                self.next_pass = None;
                self.next_pass_tried = f64::NEG_INFINITY;
            }
            StationsSetting::MinElevation => {
                self.min_elevation = MIN_ELEVATION.read(id, v)?;
                self.next_pass = None;
                self.next_pass_tried = f64::NEG_INFINITY;
            }
            StationsSetting::ArmPasses => self.arm_passes = ARM_PASSES.read(id, v)?,
            StationsSetting::Names => {
                let t = NAMES_TEXT.read(id, v)?;
                self.names.import(t);
                self.names_text = t.to_owned();
            }
            StationsSetting::Hop => {
                self.hop = HOP.read(id, v)?;
                self.hopper = None;
                self.hop_asked = None;
            }
            StationsSetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let a = &self.alerts.settings;
        values_of([
            (StationsSetting::Channels, Value::Choice(self.limit.index())),
            (StationsSetting::Satellite, Value::Bool(self.satellite)),
            (StationsSetting::Hop, Value::Bool(self.hop)),
            (StationsSetting::TimeBase, Value::Time(self.time_base)),
            (StationsSetting::SatelliteTle, Value::Text(self.tle.clone())),
            (
                StationsSetting::MinElevation,
                Value::F64(self.min_elevation),
            ),
            (StationsSetting::ArmPasses, Value::Bool(self.arm_passes)),
            (StationsSetting::Cpa, Value::Bool(a.cpa)),
            (StationsSetting::CpaNm, Value::F64(a.cpa_nm)),
            (StationsSetting::TcpaMin, Value::F64(a.tcpa_min)),
            (StationsSetting::CpaCooldownMin, Value::F64(a.cooldown_min)),
            (StationsSetting::OwnMoving, Value::Bool(a.own_moving)),
            (StationsSetting::OwnSogKn, Value::F64(a.own_sog)),
            (StationsSetting::OwnCogDeg, Value::F64(a.own_cog)),
            (
                StationsSetting::Geofences,
                Value::Text(self.geofences.clone()),
            ),
            (
                StationsSetting::Filter,
                Value::Text(self.filter_text.clone()),
            ),
            (StationsSetting::NmeaOut, Value::Bool(self.nmea_out)),
            (StationsSetting::Names, Value::Text(self.names_text.clone())),
        ])
    }

    fn commands(&self) -> &Commands {
        StationsCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match StationsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            StationsCommand::Select => {
                let key = args.arg(&STATION_ROW)?;
                let mmsi = key
                    .parse::<u32>()
                    .ok()
                    .filter(|m| self.tables.stations.get(*m).is_some())
                    .ok_or_else(|| CommandError::NoSuchRow {
                        table: self.table_id(),
                        key: key.to_owned(),
                    })?;
                self.select = Some(mmsi);
            }
            StationsCommand::Clear => {
                self.clear();
            }
            StationsCommand::Remove => {
                let table = self.table_id();
                objcmd::remove(
                    &mut self.tables.stations.stations,
                    table,
                    &mut self.p,
                    args,
                    &STATION_ROW,
                )?;
            }
        }
        self.report();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        let n = self.tables.stations.stations.len();
        self.p
            .results_mut()
            .log(format!("AIS: loaded {n} stations"));
        self.report();
        Ok(())
    }
}
