//! The interface of the four stacks: the shape of each table row, result set and event as a struct, the tables and event
//! kinds, and the description of the family ([`family`]). A component fills a struct to publish; a consumer reads one.

use sdrkit_core::interface::{EventKind, Family, Fields, Hertz, StackShape, Table, Time};
use sdrkit_core::publish::{keys, Published, TableDef};
use sdrkit_core::{ResultsDecl, SettingSet};
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

sdrkit_core::choice_enum! {
    /// How a position was found, or which CPR frames wait for a partner.
    pub enum FixChoice {
        /// A global decode of an even and an odd frame.
        Global = ("global", "Global"),
        /// A local decode near the last position.
        Local = ("local", "Local"),
        /// A local decode near the receiver.
        LocalFromReceiver = ("local_from_receiver", "Local from the receiver"),
        /// An even and an odd frame wait for a fix.
        CprPair = ("cpr_pair", "CPR pair waiting"),
        /// An even frame waits for a partner.
        CprEven = ("cpr_even", "CPR even waiting"),
        /// An odd frame waits for a partner.
        CprOdd = ("cpr_odd", "CPR odd waiting"),
    }
}

sdrkit_core::choice_enum! {
    /// Whether an aircraft event is the first of its aircraft.
    pub enum AircraftChange {
        /// The first event of the aircraft.
        New = ("new", "New"),
        /// A later event.
        Update = ("update", "Update"),
    }
}

/// A row of the ADS-B aircraft table: the tracker's state for one aircraft. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AircraftRow<'a> {
    /// The 24-bit address, six hex digits (`~` in front for a non-ICAO address).
    #[field(label = "ICAO")]
    pub icao: &'a str,
    /// The flight identification.
    #[field(label = "Callsign")]
    pub callsign: Option<&'a str>,
    /// The registration derived from the address.
    #[field(label = "Registration")]
    pub registration: Option<&'a str>,
    /// The State the address is allocated to.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The emitter category, set letter and code ("A3").
    #[field(label = "Category")]
    pub category: Option<&'a str>,
    /// The identity code, four octal digits.
    #[field(label = "Squawk")]
    pub squawk: Option<&'a str>,
    /// The emergency state (0 none).
    #[field(label = "Emergency code")]
    pub emergency: i64,
    /// The emergency state in words.
    #[field(label = "Emergency")]
    pub emergency_name: Option<&'a str>,
    /// On the ground.
    #[field(label = "On the ground")]
    pub on_ground: Option<bool>,
    /// The alert bit of the flight status.
    #[field(label = "Alert")]
    pub alert: bool,
    /// The special position identification (ident) bit.
    #[field(label = "Ident")]
    pub spi: bool,
    /// The last position.
    #[field(label = "Position", unit = "°")]
    pub position: Option<(f64, f64)>,
    /// How the position was found, or which CPR frames wait for a partner.
    #[field(label = "Position fix")]
    pub fix: Option<FixChoice>,
    /// When the position was heard, wall time.
    #[field(label = "Position heard")]
    pub position_time: Option<Time>,
    /// The barometric altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub alt_ft: Option<i64>,
    /// The GNSS height.
    #[field(label = "GNSS altitude", unit = "ft")]
    pub gnss_alt_ft: Option<i64>,
    /// The speed.
    #[field(label = "Speed", unit = "kt")]
    pub speed_kt: Option<f64>,
    /// What the speed is.
    #[field(label = "Speed kind")]
    pub speed_kind: Option<crate::message::SpeedKind>,
    /// The track or heading.
    #[field(label = "Heading", unit = "°")]
    pub heading_deg: Option<f64>,
    /// Whether the heading is the track over the ground.
    #[field(label = "Heading is track")]
    pub heading_is_track: Option<bool>,
    /// The vertical rate.
    #[field(label = "Vertical rate", unit = "ft/min")]
    pub vertical_rate_fpm: Option<i64>,
    /// GNSS height minus barometric altitude.
    #[field(label = "GNSS minus barometric", unit = "ft")]
    pub gnss_minus_baro_ft: Option<i64>,
    /// The selected altitude.
    #[field(label = "Selected altitude", unit = "ft")]
    pub selected_alt_ft: Option<i64>,
    /// The selected heading.
    #[field(label = "Selected heading", unit = "°")]
    pub selected_heading_deg: Option<f64>,
    /// The barometric pressure setting.
    #[field(label = "Barometric setting", unit = "hPa")]
    pub baro_setting_hpa: Option<f64>,
    /// The ADS-B version number.
    #[field(label = "ADS-B version")]
    pub adsb_version: Option<i64>,
    /// The navigation accuracy category for position.
    #[field(label = "NACp")]
    pub nacp: Option<i64>,
    /// The source integrity level.
    #[field(label = "SIL")]
    pub sil: Option<i64>,
    /// The navigation integrity category.
    #[field(label = "NIC")]
    pub nic: Option<i64>,
    /// The indicated airspeed.
    #[field(label = "Indicated airspeed", unit = "kt")]
    pub ias_kt: Option<f64>,
    /// The Mach number.
    #[field(label = "Mach")]
    pub mach: Option<f64>,
    /// The roll angle.
    #[field(label = "Roll", unit = "°")]
    pub roll_deg: Option<f64>,
    /// Where the derived wind blows from.
    #[field(label = "Wind (derived) from", unit = "°")]
    pub wind_derived_from_deg: Option<f64>,
    /// The derived wind speed.
    #[field(label = "Wind (derived)", unit = "kt")]
    pub wind_derived_kt: Option<f64>,
    /// Where the reported wind blows from.
    #[field(label = "Wind (reported) from", unit = "°")]
    pub wind_reported_from_deg: Option<f64>,
    /// The reported wind speed.
    #[field(label = "Wind (reported)", unit = "kt")]
    pub wind_reported_kt: Option<f64>,
    /// The static air temperature from TAS and Mach.
    #[field(label = "Air temperature (derived)", unit = "°C")]
    pub sat_derived_c: Option<f64>,
    /// The static air temperature reported.
    #[field(label = "Air temperature (reported)", unit = "°C")]
    pub sat_reported_c: Option<f64>,
    /// The static pressure reported.
    #[field(label = "Static pressure", unit = "hPa")]
    pub pressure_hpa: Option<f64>,
    /// The humidity reported.
    #[field(label = "Humidity", unit = "%")]
    pub humidity_pct: Option<f64>,
    /// The hazards reported, with their levels.
    #[field(label = "Hazards")]
    pub hazards: Option<&'a str>,
    /// The ACAS resolution advisory in words.
    #[field(label = "Resolution advisory")]
    pub advisory: Option<&'a str>,
    /// The address of the threat of the advisory.
    #[field(label = "Advisory threat")]
    pub advisory_threat: Option<&'a str>,
    /// When the advisory was heard, wall time.
    #[field(label = "Advisory heard")]
    pub advisory_time: Option<Time>,
    /// The Mode S subnetwork version.
    #[field(label = "Subnetwork version")]
    pub subnetwork_version: Option<i64>,
    /// ACAS generates resolution advisories.
    #[field(label = "ACAS RA capable")]
    pub acas_ra_capable: Option<bool>,
    /// The GICB registers the transponder holds.
    #[field(label = "GICB registers")]
    pub gicb: Option<&'a str>,
    /// Comm-B messages were heard.
    #[field(label = "Data link")]
    pub datalink: bool,
    /// Heard through TIS-B.
    #[field(label = "TIS-B")]
    pub tisb: bool,
    /// The mean pulse level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: Option<f64>,
    /// The distance from the station.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: Option<f64>,
    /// The bearing from the station.
    #[field(label = "Bearing", unit = "°")]
    pub bearing_deg: Option<f64>,
    /// Where the last update came from: the row key of the sources table ("0" this receiver, else the number of the feed).
    #[field(label = "Source", refers_to = "sources")]
    pub source: &'a str,
    /// The trail, packed.
    #[field(shared = sdrkit_proto_aero::trail::TRAIL)]
    pub trail: Arc<[u8]>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when last heard, in whole seconds.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The ADS-B aircraft table, most recently heard first.
pub static AIRCRAFT: Table<AircraftRow<'static>> = Table::new("aircraft", "Aircraft", 160);

/// A row of the message monitor: one good message of this receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MessageRow<'a> {
    /// Signal time of the preamble.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The frame in hex.
    #[field(label = "Hex")]
    pub hex: &'a str,
    /// The address.
    #[field(label = "ICAO")]
    pub icao: &'a str,
    /// The downlink format.
    #[field(label = "DF")]
    pub df: i64,
    /// Bits the repair flipped.
    #[field(label = "Bits fixed")]
    pub fixed_bits: i64,
    /// Level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The description in words.
    #[field(label = "Message")]
    pub message: &'a str,
}

/// The last 64 good messages of this receiver, newest first.
pub static MESSAGES: Table<MessageRow<'static>> = Table::new("messages", "Messages", 64);

/// A receiver that the table takes messages from. The row key is the number of the source: "0" for this receiver, then
/// the number of each feed in.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SourceRow<'a> {
    /// The name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The format, known after the first bytes.
    #[field(label = "Format")]
    pub format: Option<crate::feeds::FeedFormat>,
    /// Connected now.
    #[field(label = "Connected")]
    pub connected: bool,
    /// Messages taken.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Aircraft whose last update came from it.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// The last error.
    #[field(label = "Error")]
    pub error: Option<&'a str>,
}

/// The receivers the table takes messages from: this one first, then the feeds in.
pub static SOURCES: Table<SourceRow<'static>> = Table::new("sources", "Sources", 32);

/// A count of messages of one format. The row key is the format number in two digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CountRow {
    /// A downlink or uplink format number.
    #[field(label = "Format")]
    pub format_number: i64,
    /// How many.
    #[field(label = "Count")]
    pub count: i64,
}

/// Good messages by downlink format.
pub static BY_DF: Table<CountRow> = Table::new("by_df", "Messages by format", 32);

/// A GPS interference cell of half a degree with at least three aircraft.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GpsCellRow {
    /// The south-west corner of the cell.
    #[field(label = "South-west corner", unit = "°")]
    pub corner: (f64, f64),
    /// Aircraft in the cell.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// The level, 0 to 3.
    #[field(label = "Level")]
    pub level: i64,
}

/// The GPS interference cells.
pub static GPS_CELLS: Table<GpsCellRow> = Table::new("gps_cells", "GPS interference", 4096);

/// The result fields of the tracker.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TrackerFields {
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft_count: i64,
    /// Aircraft with a position.
    #[field(label = "With position")]
    pub with_position: i64,
    /// Good messages per second over the last second.
    #[field(label = "Messages per second", unit = "Hz")]
    pub message_rate: Hertz,
    /// The mean level of the messages.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The noise floor.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: f64,
    /// The mean signal to noise ratio of the messages.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The farthest position counted.
    #[field(label = "Max range", unit = "m")]
    pub max_range_m: Option<f64>,
    /// Reports made.
    #[field(label = "Report")]
    pub report: i64,
    /// The signal time of the report, seconds from the start of the input.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
    /// Messages that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Messages that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Messages repaired.
    #[field(label = "Repaired messages")]
    pub repaired: i64,
    /// Messages at least 3 dB below full scale.
    #[field(label = "Loud messages")]
    pub loud: i64,
    /// Preamble candidates.
    #[field(label = "Candidates")]
    pub candidates: i64,
    /// The demodulator effort in use.
    #[field(label = "Demodulator effort")]
    pub effort: i64,
    /// A station position that fits the aircraft heard, when the one given is far from all of them.
    #[field(label = "Position hint", unit = "°")]
    pub position_hint: Option<(f64, f64)>,
    /// Positions counted in the range statistics.
    #[field(label = "Positions counted")]
    pub coverage_positions: i64,
    /// The highest altitude counted.
    #[field(label = "Highest altitude", unit = "ft")]
    pub coverage_max_alt_ft: Option<i64>,
}

/// The result fields of the feeds out.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FeedFields {
    /// Messages sent to the feeds.
    #[field(label = "Messages fed")]
    pub feed_messages: i64,
}

/// A feed out: a server or an outbound target. The row key is the name.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FeedRow<'a> {
    /// The server ("beast", "sbs", "asterix") or the outbound target ("format@host:port").
    #[field(label = "Feed")]
    pub name: &'a str,
    /// The TCP port of a server or a target.
    #[field(label = "Port")]
    pub port: Option<i64>,
    /// Clients connected to a server.
    #[field(label = "Clients")]
    pub clients: Option<i64>,
    /// Messages dropped because a client or target was slow.
    #[field(label = "Dropped")]
    pub dropped: Option<i64>,
    /// A target is connected.
    #[field(label = "Connected")]
    pub connected: Option<bool>,
    /// Why a server did not start.
    #[field(label = "Error")]
    pub error: Option<&'a str>,
}

/// The feeds out: the servers and the outbound targets.
pub static FEEDS: Table<FeedRow<'static>> = Table::new("feeds", "Feeds", 64);

/// The series of the range statistics.
pub mod series {
    use sdrkit_core::publish::SeriesDef;

    /// The farthest range by bearing.
    pub const COVERAGE_RANGE: SeriesDef = SeriesDef {
        key: "coverage_range",
        label: "Farthest range by bearing",
        x_unit: "°",
        y_unit: "m",
    };
    /// The positions by range.
    pub const COVERAGE_HISTOGRAM: SeriesDef = SeriesDef {
        key: "coverage_histogram",
        label: "Positions by range",
        x_unit: "m",
        y_unit: "",
    };
    /// The message rate over time.
    pub const COVERAGE_RATE: SeriesDef = SeriesDef {
        key: "coverage_rate",
        label: "Messages per second",
        x_unit: "s",
        y_unit: "Hz",
    };
}

/// The fields of the event of an aircraft seen or updated. The address is the source. The urgency is an emergency for squawk
/// 7500, 7600 or 7700 and for an emergency state.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AircraftSeen<'a> {
    /// The first event of an aircraft, or a later one.
    #[field(label = "Event")]
    pub event: AircraftChange,
    /// The address.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The flight identification.
    #[field(label = "Callsign")]
    pub callsign: Option<&'a str>,
    /// The registration derived from the address.
    #[field(label = "Registration")]
    pub registration: Option<&'a str>,
    /// The emitter category, set letter and code ("A3").
    #[field(label = "Category")]
    pub category: Option<&'a str>,
    /// The identity code, four octal digits.
    #[field(label = "Squawk")]
    pub squawk: Option<&'a str>,
    /// The emergency state (0 none).
    #[field(label = "Emergency code")]
    pub emergency: i64,
    /// On the ground.
    #[field(label = "On the ground")]
    pub on_ground: Option<bool>,
    /// The barometric altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub altitude_ft: Option<i64>,
    /// The speed over the ground.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The vertical rate.
    #[field(label = "Vertical rate", unit = "ft/min")]
    pub vertical_rate_fpm: Option<i64>,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The distance from the station.
    #[field(label = "Distance", unit = "km")]
    pub distance_km: Option<f64>,
    /// Messages heard.
    #[field(label = "Messages")]
    pub messages: i64,
    /// How urgent the event is.
    #[field(shared = keys::URGENCY)]
    pub urgency: keys::Urgency,
    /// Why the event is urgent.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// An aircraft seen or updated.
pub static AIRCRAFT_SEEN: EventKind<AircraftSeen<'static>> =
    EventKind::new("adsb.aircraft", "Aircraft seen or updated");

/// The tracker's results: its fields, tables, series and map points.
pub fn tracker_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(TrackerFields::SHAPE.fields)
        .with_table(&AIRCRAFT)
        .with_table(&MESSAGES)
        .with_table(&SOURCES)
        .with_table(&BY_DF)
        .with_table(&GPS_CELLS)
        .with_series(&series::COVERAGE_RANGE)
        .with_series(&series::COVERAGE_HISTOGRAM)
        .with_series(&series::COVERAGE_RATE)
        .points("Aircraft")
}

/// Writes the rows of `table` from `counts`: one row for each count that is not 0, keyed by its index in two digits.
pub fn count_rows(p: &mut Published, table: &Table<CountRow>, counts: &[u64]) {
    for (i, &n) in counts.iter().enumerate() {
        let mut key = sdrkit_util_geo::Text::<20>::new();
        // A `usize` has at most 20 digits.
        let _ = std::fmt::Write::write_fmt(&mut key, format_args!("{i:02}"));
        let k = key.as_str();
        if n == 0 {
            p.remove_row(table, k);
        } else {
            let row = CountRow {
                format_number: i as i64,
                count: n as i64,
            };
            table.put(p, k, &row);
        }
    }
}

/// A row of the UAT aircraft table. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UatAircraftRow<'a> {
    /// The 24-bit address, six hex digits.
    #[field(label = "ICAO")]
    pub icao: &'a str,
    /// The address qualifier.
    #[field(label = "Address qualifier")]
    pub address_qualifier: i64,
    /// The flight identification.
    #[field(label = "Callsign")]
    pub callsign: Option<&'a str>,
    /// The registration derived from the address.
    #[field(label = "Registration")]
    pub registration: Option<&'a str>,
    /// The State the address is allocated to.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The identity code, four octal digits.
    #[field(label = "Squawk")]
    pub squawk: Option<&'a str>,
    /// The last position.
    #[field(label = "Position", unit = "°")]
    pub position: Option<(f64, f64)>,
    /// The barometric altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub alt_ft: Option<i64>,
    /// The geometric altitude.
    #[field(label = "Geometric altitude", unit = "ft")]
    pub geo_alt_ft: Option<i64>,
    /// The speed.
    #[field(label = "Speed", unit = "kt")]
    pub speed_kt: Option<f64>,
    /// The track or heading.
    #[field(label = "Heading", unit = "°")]
    pub heading_deg: Option<f64>,
    /// The vertical rate.
    #[field(label = "Vertical rate", unit = "ft/min")]
    pub vertical_rate_fpm: Option<i64>,
    /// On the ground.
    #[field(label = "On the ground")]
    pub on_ground: bool,
    /// Heard through TIS-B.
    #[field(label = "TIS-B")]
    pub tisb: bool,
    /// The navigation integrity category.
    #[field(label = "NIC")]
    pub nic: i64,
    /// The UAT emitter category.
    #[field(label = "Category")]
    pub category: Option<i64>,
    /// The emergency state (0 none).
    #[field(label = "Emergency code")]
    pub emergency: i64,
    /// The level of the last message.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// The distance from the station.
    #[field(label = "Distance", unit = "m")]
    pub distance_m: Option<f64>,
    /// The bearing from the station.
    #[field(label = "Bearing", unit = "°")]
    pub bearing_deg: Option<f64>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when last heard, in whole seconds.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The UAT aircraft table, most recently heard first.
pub static UAT_AIRCRAFT: Table<UatAircraftRow<'static>> = Table::new("aircraft", "Aircraft", 128);

sdrkit_core::choice_enum! {
    /// What a TWGO record is.
    pub enum RecordKind {
        /// A text record.
        Text = ("text", "Text"),
        /// A graphic record.
        Graphic = ("graphic", "Graphic"),
    }
}

sdrkit_core::choice_enum! {
    /// The geometry of a graphic TWGO record.
    pub enum GeometryKind {
        /// A closed path.
        Polygon = ("polygon", "Polygon"),
        /// An open path.
        Polyline = ("polyline", "Polyline"),
        /// A circle.
        Circle = ("circle", "Circle"),
        /// A point.
        Point = ("point", "Point"),
        /// Another overlay geometry.
        Other = ("other", "Other"),
    }
}

/// A FIS-B product held. The row key is the number of the product in six digits.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FisbProductRow<'a> {
    /// Signal time the product was received.
    #[field(label = "Received", unit = "s")]
    pub time_s: f64,
    /// The product id.
    #[field(label = "Product")]
    pub product_id: i64,
    /// The product name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The product's own time ("MM-DD HH:MM:SS", the parts sent).
    #[field(label = "Product time")]
    pub stamp: &'a str,
    /// The text.
    #[field(label = "Text")]
    pub text: &'a str,
    /// TWGO records in the product.
    #[field(label = "Records")]
    pub records: i64,
    /// Grid blocks in the product.
    #[field(label = "Blocks")]
    pub blocks: i64,
}

/// The FIS-B products held, newest first.
pub static FISB_PRODUCTS: Table<FisbProductRow<'static>> =
    Table::new("fisb_products", "FIS-B products", 64);

/// A text or graphic record (TWGO) of a product held: a NOTAM, AIRMET, SIGMET, G-AIRMET, CWA, SUA or TFR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FisbRecordRow<'a> {
    /// The product the record belongs to: the row key of the products table.
    #[field(label = "Product", refers_to = "fisb_products")]
    pub product: &'a str,
    /// Signal time the product was received.
    #[field(label = "Received", unit = "s")]
    pub time_s: f64,
    /// The product id.
    #[field(label = "Product")]
    pub product_id: i64,
    /// The product name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The report number of the record.
    #[field(label = "Report")]
    pub report: i64,
    /// What the record is.
    #[field(label = "Record")]
    pub kind: RecordKind,
    /// The record is active (text records).
    #[field(label = "Active")]
    pub active: Option<bool>,
    /// The label of a graphic record.
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The overlay record identifier.
    #[field(label = "Overlay")]
    pub overlay: Option<i64>,
    /// The start of the validity ("MM-DD HH:MM", the parts sent).
    #[field(label = "Start")]
    pub start: Option<&'a str>,
    /// The end of the validity.
    #[field(label = "End")]
    pub end: Option<&'a str>,
    /// The geometry of a graphic record.
    #[field(label = "Geometry")]
    pub geometry: Option<GeometryKind>,
    /// The first vertex, the centre or the point.
    #[field(label = "Position", unit = "°")]
    pub position: Option<(f64, f64)>,
    /// The number of vertices of a path. The vertices are rows of the vertices table.
    #[field(label = "Vertices")]
    pub vertices: Option<i64>,
    /// The radius of a circle.
    #[field(label = "Radius", unit = "m")]
    pub radius_m: Option<f64>,
    /// The lowest altitude.
    #[field(label = "Bottom", unit = "ft")]
    pub alt_bottom_ft: Option<i64>,
    /// The highest altitude.
    #[field(label = "Top", unit = "ft")]
    pub alt_top_ft: Option<i64>,
    /// The text of a text record.
    #[field(label = "Text")]
    pub text: Option<&'a str>,
}

/// The text and graphic records of the products held.
pub static FISB_RECORDS: Table<FisbRecordRow<'static>> =
    Table::new("fisb_records", "FIS-B records", 512);

/// A block of a gridded product held (NEXRAD, icing, cloud tops, turbulence, lightning).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FisbBlockRow<'a> {
    /// The product the block belongs to: the row key of the products table.
    #[field(label = "Product", refers_to = "fisb_products")]
    pub product: &'a str,
    /// Signal time the product was received.
    #[field(label = "Received", unit = "s")]
    pub time_s: f64,
    /// The product id.
    #[field(label = "Product")]
    pub product_id: i64,
    /// The product name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The block number.
    #[field(label = "Block")]
    pub block: i64,
    /// The block is in the southern hemisphere.
    #[field(label = "South")]
    pub south: bool,
    /// The scale factor (0 high, 1 medium, 2 low resolution).
    #[field(label = "Scale")]
    pub scale: i64,
    /// The north-west corner of the block.
    #[field(label = "North-west corner", unit = "°")]
    pub corner: (f64, f64),
    /// The height of the block.
    #[field(label = "Latitude size", unit = "°")]
    pub lat_size_deg: f64,
    /// The width of the block.
    #[field(label = "Longitude size", unit = "°")]
    pub lon_size_deg: f64,
    /// The highest bin value.
    #[field(label = "Highest bin")]
    pub max_bin: i64,
    /// The 128 bins, one hex digit each, rows from the north. An empty block has none.
    #[field(label = "Bins")]
    pub bins: Option<&'a str>,
}

/// The blocks of the gridded products held.
pub static FISB_BLOCKS: Table<FisbBlockRow<'static>> =
    Table::new("fisb_blocks", "FIS-B grid blocks", 2048);

/// A vertex of the path of a graphic record held.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FisbVertexRow<'a> {
    /// The record the vertex belongs to: the row key of the records table.
    #[field(label = "Record", refers_to = "fisb_records")]
    pub record: &'a str,
    /// The place of the vertex in its path, from 0.
    #[field(label = "Index")]
    pub index: i64,
    /// The position.
    #[field(label = "Position", unit = "°")]
    pub position: (f64, f64),
    /// The altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub alt_ft: i64,
}

/// The vertices of the paths of the graphic records held.
pub static FISB_VERTICES: Table<FisbVertexRow<'static>> =
    Table::new("fisb_vertices", "FIS-B vertices", 8192);

sdrkit_core::choice_enum! {
    /// What a UAT message is.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum UatMessageKind {
        /// An ADS-B message.
        Adsb = ("adsb", "ADS-B"),
        /// A TIS-B message.
        Tisb = ("tisb", "TIS-B"),
        /// A FIS-B product.
        Fisb = ("fisb", "FIS-B"),
        /// Another UAT message.
        Uat = ("uat", "UAT"),
    }
}

/// A line of the UAT message log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UatLogRow<'a> {
    /// Signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What the message is.
    #[field(label = "Kind")]
    pub kind: UatMessageKind,
    /// The address.
    #[field(label = "Address")]
    pub address: Option<&'a str>,
    /// Level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// What it said.
    #[field(label = "Summary")]
    pub summary: &'a str,
}

/// The last 64 UAT messages, newest first.
pub static UAT_LOG: Table<UatLogRow<'static>> = Table::new("log", "Log", 64);

/// The result fields of the UAT tracker.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UatFields {
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft_count: i64,
    /// Aircraft with a position.
    #[field(label = "With position")]
    pub with_position: i64,
    /// Good messages per second over the last second.
    #[field(label = "Messages per second", unit = "Hz")]
    pub message_rate: Hertz,
    /// FIS-B products held.
    #[field(label = "FIS-B products")]
    pub fisb_products: i64,
    /// TIS-B messages among the last 64.
    #[field(label = "TIS-B targets")]
    pub tisb_recent: i64,
    /// FIS-B files waiting for segments.
    #[field(label = "FIS-B files waiting")]
    pub fisb_pending: i64,
    /// Frames that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The noise floor.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: f64,
    /// Reports made.
    #[field(label = "Report")]
    pub report: i64,
    /// The signal time of the report, seconds from the start of the input.
    #[field(label = "Signal time", unit = "s")]
    pub time_s: f64,
}

/// The fields of the event of a decoded UAT message. The address is the source. The urgency is an emergency for squawk 7500,
/// 7600 or 7700 and for an emergency status.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UatMessage<'a> {
    /// What the message said.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the message is.
    #[field(label = "Kind")]
    pub kind: UatMessageKind,
    /// The address.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The position decoded from the message.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// How urgent the event is.
    #[field(shared = keys::URGENCY)]
    pub urgency: keys::Urgency,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// A decoded UAT message.
pub static UAT_MESSAGE: EventKind<UatMessage<'static>> =
    EventKind::new("uat.message", "UAT message");

/// A Mode A/C code heard at least the set number of times. The row key is the code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModeAcRow<'a> {
    /// The 13 code bits in hex.
    #[field(label = "Code")]
    pub code: &'a str,
    /// The code read as a squawk.
    #[field(label = "Squawk")]
    pub squawk: &'a str,
    /// The code read as a Mode C altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub alt_ft: Option<i64>,
    /// Replies heard.
    #[field(label = "Replies")]
    pub replies: Option<i64>,
    /// An SPI pulse was heard.
    #[field(label = "Ident")]
    pub spi: bool,
    /// Level of the last reply.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: f64,
    /// Wall-clock time when last heard, in whole seconds.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The Mode A/C codes heard.
pub static MODEAC_CODES: Table<ModeAcRow<'static>> = Table::new("modeac", "Mode A/C", 256);

/// The result fields of the Mode A/C decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModeAcFields {
    /// Replies decoded.
    #[field(label = "Mode A/C replies")]
    pub modeac_replies: i64,
    /// Codes shown.
    #[field(label = "Mode A/C codes")]
    pub modeac_codes: i64,
}

/// The fields of the event of a Mode A/C reply.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModeAcReplyEvent<'a> {
    /// The 13 code bits in hex.
    #[field(label = "Code")]
    pub code: &'a str,
    /// The code read as a squawk.
    #[field(label = "Squawk")]
    pub squawk: &'a str,
    /// The code read as a Mode C altitude.
    #[field(label = "Altitude", unit = "ft")]
    pub altitude_ft: Option<i64>,
    /// An SPI pulse was heard.
    #[field(label = "Ident")]
    pub spi: bool,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// A Mode A/C reply.
pub static MODEAC_REPLY: EventKind<ModeAcReplyEvent<'static>> =
    EventKind::new("modeac.reply", "Mode A/C reply");

/// An aircraft that the interrogations address. The row key is the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AddressedRow<'a> {
    /// The interrogated address.
    #[field(label = "ICAO")]
    pub icao: &'a str,
    /// Interrogations.
    #[field(label = "Interrogations")]
    pub interrogations: Option<i64>,
    /// Uplink formats with their counts ("UF4x3 UF5x1").
    #[field(label = "Formats")]
    pub formats: &'a str,
    /// The fields of the last interrogation.
    #[field(label = "Last")]
    pub last: &'a str,
    /// Wall-clock time when last heard, in whole seconds.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The aircraft the interrogations address.
pub static ADDRESSED: Table<AddressedRow<'static>> =
    Table::new("addressed", "Addressed aircraft", 256);

/// An interrogator heard in all-calls. The row key is its code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InterrogatorRow<'a> {
    /// The interrogator code of an all-call.
    #[field(label = "Code")]
    pub code: &'a str,
    /// All-calls.
    #[field(label = "All-calls")]
    pub all_calls: Option<i64>,
    /// Wall-clock time when last heard, in whole seconds.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The interrogators heard in all-calls.
pub static INTERROGATORS: Table<InterrogatorRow<'static>> =
    Table::new("interrogators", "Interrogators", 64);

/// Interrogations by uplink format.
pub static BY_UF: Table<CountRow> = Table::new("by_uf", "Interrogations by format", 32);

/// The result fields of the interrogation decoder.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UplinkFields {
    /// Interrogations of all kinds.
    #[field(label = "Interrogations")]
    pub interrogations: i64,
    /// Mode A interrogations.
    #[field(label = "Mode A")]
    pub mode_a: i64,
    /// Mode C interrogations.
    #[field(label = "Mode C")]
    pub mode_c: i64,
    /// Mode S interrogations.
    #[field(label = "Mode S")]
    pub mode_s: i64,
    /// Mode A/C/S all-calls.
    #[field(label = "Mode A/C/S all-call")]
    pub all_call_a: i64,
    /// Mode C/S all-calls.
    #[field(label = "Mode C/S all-call")]
    pub all_call_c: i64,
    /// Mode A/C-only all-calls.
    #[field(label = "Mode A/C-only all-call")]
    pub ac_only_all_call: i64,
}

/// The fields of the event of an interrogation. The interrogated address is the target once it is confirmed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InterrogationEvent<'a> {
    /// What the interrogation is.
    #[field(label = "Kind")]
    pub kind: crate::uplink::InterrogationKind,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The P2 (side-lobe suppression) pulse was seen.
    #[field(label = "P2 pulse")]
    pub p2: bool,
    /// The uplink format of a Mode S interrogation.
    #[field(label = "Uplink format")]
    pub uf: Option<i64>,
    /// The Mode S message in hex.
    #[field(label = "Message")]
    pub hex: Option<&'a str>,
    /// The interrogated address, once it is confirmed (see `confirmed`).
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// Mode S to one aircraft: the address from the parity is in the table of addressed aircraft, so it is the target.
    /// False when it is not yet: a damaged message also gives an address, so the target has no value and the address is
    /// only in the message. No value for Mode A/C, all-calls and broadcasts.
    #[field(label = "Address confirmed")]
    pub confirmed: Option<bool>,
    /// The fields of a Mode S interrogation.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// What the event is about.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// An interrogation.
pub static INTERROGATION: EventKind<InterrogationEvent<'static>> =
    EventKind::new("uplink.interrogation", "Interrogation");

/// The tables of the UAT tracker.
pub fn uat_tables() -> [&'static TableDef; 6] {
    [
        &UAT_AIRCRAFT,
        &FISB_PRODUCTS,
        &FISB_RECORDS,
        &FISB_BLOCKS,
        &FISB_VERTICES,
        &UAT_LOG,
    ]
}

/// The tables of the interrogation decoder.
pub fn uplink_tables() -> [&'static TableDef; 3] {
    [&ADDRESSED, &INTERROGATORS, &BY_UF]
}

/// The interface of the family: what each of the four stacks declares.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(describe)
}

fn describe() -> Family {
    let clear = &sdrkit_util_objects::commands::CLEAR;
    Family::new("adsb", 1, 1)
        .added(1, 1, &["shape.InterrogationEvent.confirmed"])
        .stack(
            StackShape::new("adsb")
                .fields::<TrackerFields>()
                .fields::<FeedFields>()
                .fields::<ModeAcFields>()
                .table(&AIRCRAFT)
                .table(&MESSAGES)
                .table(&SOURCES)
                .table(&BY_DF)
                .table(&GPS_CELLS)
                .table(&FEEDS)
                .table(&MODEAC_CODES)
                .series(series::COVERAGE_RANGE)
                .series(series::COVERAGE_HISTOGRAM)
                .series(series::COVERAGE_RATE)
                .points("Aircraft")
                .event(&AIRCRAFT_SEEN)
                .settings(crate::settings::AdsbSetting::SETTINGS)
                .command(clear)
                .command(&crate::table::RESET_STATS),
        )
        .stack(
            StackShape::new("uat")
                .fields::<UatFields>()
                .table(&UAT_AIRCRAFT)
                .table(&FISB_PRODUCTS)
                .table(&FISB_RECORDS)
                .table(&FISB_BLOCKS)
                .table(&FISB_VERTICES)
                .table(&UAT_LOG)
                .points("Aircraft")
                .event(&UAT_MESSAGE)
                .settings(crate::settings::UatSetting::SETTINGS)
                .command(clear),
        )
        .stack(
            StackShape::new("modeac")
                .fields::<ModeAcFields>()
                .table(&MODEAC_CODES)
                .event(&MODEAC_REPLY)
                .settings(crate::settings::ModeAcSetting::SETTINGS)
                .command(clear),
        )
        .stack(
            StackShape::new("uplink1030")
                .fields::<UplinkFields>()
                .table(&ADDRESSED)
                .table(&INTERROGATORS)
                .table(&BY_UF)
                .event(&INTERROGATION)
                .settings(crate::settings::UplinkSetting::SETTINGS)
                .command(clear),
        )
}

/// The interface description of stack `id` of the family.
///
/// # Panics
///
/// If the family has no stack `id`.
pub fn shape(id: &str) -> &'static StackShape {
    match family().stack_shape(id) {
        Some(s) => s,
        None => panic!("the family has no stack {id}"),
    }
}
