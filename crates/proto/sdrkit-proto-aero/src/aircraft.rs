//! The aircraft table: one row per aircraft heard on a data link, with its last position and a trail.
//!
//! A row is keyed by the 24-bit ICAO address when it is known, and by the registration otherwise. Times are signal time in
//! seconds.

use crate::error::{Error, Item, Result};
use crate::trail::TrailMark;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::publish::{Cells, Object, Point};
use sdrkit_util_objects::{ObjectTable, RowRef, Seen, Shift, Spacing, Stamp, Trail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;

/// Trail points kept per aircraft by default.
pub const TRAIL_POINTS: usize = 24;

/// How the table keeps the trail of each row: the most points, and the least time and distance between two points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrailSettings {
    /// The most points per row (at least 1). If a trail is full, its oldest point leaves.
    pub points: usize,
    /// The least spacing between two points.
    pub spacing: Spacing,
}

impl Default for TrailSettings {
    /// [`TRAIL_POINTS`] points, with no least spacing.
    fn default() -> Self {
        TrailSettings {
            points: TRAIL_POINTS,
            spacing: Spacing::default(),
        }
    }
}

impl TrailSettings {
    /// An empty trail with these settings.
    fn trail(&self) -> Trail<TrailMark> {
        Trail::new(self.points, self.spacing)
    }

    /// The points of `old` that these settings keep, newest last.
    fn refit(&self, old: &Trail<TrailMark>) -> Trail<TrailMark> {
        let mut t = self.trail();
        for p in old.points() {
            // The points of a trail are valid, so a push never fails.
            let _ = t.push(p.t, p.position, p.extra);
        }
        t
    }
}
/// The minimum distance, in metres, between two positions from which a track is computed.
pub const MIN_MOVE_M: f64 = 1000.0;

/// Where a position came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// An ADS-C report.
    Adsc,
    /// A position written in a message text.
    Text,
    /// A VDL Mode 2 XID location parameter.
    Vdl2Xid,
    /// A VDL Mode 4 synchronisation burst.
    Vdl4Sync,
    /// An HFDL performance or frequency data HFNPDU.
    Hfdl,
    /// A Mode S extended squitter (ADS-B or TIS-B) position, or a position from another receiver of the same link.
    ModeS,
    /// Another link's own position field.
    Other,
}

/// A position report.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fix {
    /// The position (its `alt_m` is not used).
    pub position: Position,
    /// Altitude, feet, when known.
    pub alt_ft: Option<f64>,
    /// The aircraft was on the ground.
    #[serde(default)]
    pub on_ground: bool,
    /// Where it came from.
    pub source: Source,
}

/// Track, ground speed and vertical rate.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    /// Degrees true.
    pub track_deg: f64,
    /// Knots.
    pub gs_kt: f64,
    /// Feet per minute, when known.
    pub vs_fpm: Option<f64>,
}

/// The key of a row.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    /// The 24-bit ICAO address.
    Address(u32),
    /// The registration, upper case, without dashes, dots or spaces.
    Registration(String),
    /// A 24-bit address that is not an ICAO address: a TIS-B track number, an anonymous or a ground vehicle address
    /// (DO-260B 2.2.3.2.1.5, the address qualifier).
    NonIcao(u32),
    /// An aircraft that a ground station identifies by an aircraft id it assigned, until the address is known (HFDL logon).
    Pending {
        /// The ground station.
        ground_station: u8,
        /// The aircraft id that the ground station provided.
        aircraft_id: u8,
    },
}

/// The row id: an address as six upper-case hex digits (`7C1234`), a registration as it is kept (`VHOQA`), a non-ICAO address
/// as `~` and six hex digits (`~7C1234`), a pending row as `@` and the ground station and aircraft id in two hex digits each
/// (`@0312`).
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Address(a) => write!(f, "{a:06X}"),
            Key::Registration(r) => f.write_str(r),
            Key::NonIcao(a) => write!(f, "~{a:06X}"),
            Key::Pending {
                ground_station,
                aircraft_id,
            } => write!(f, "@{ground_station:02X}{aircraft_id:02X}"),
        }
    }
}

/// One aircraft. `X` holds what one link adds to a row (carrier frequency, logon state ...).
///
/// It is saved with serde. The country is saved for people to read, and on restore it comes again from the address.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self")]
pub struct Aircraft<X = ()> {
    /// The 24-bit address, when known.
    pub icao: Option<u32>,
    /// The 24-bit address that is not an ICAO address, for a row keyed by one ([`Key::NonIcao`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_icao: Option<u32>,
    /// The registration, as heard or from the address.
    pub registration: String,
    /// The State the address is allocated to.
    #[serde(skip_deserializing)]
    pub country: Option<&'static str>,
    /// The flight id.
    pub flight: String,
    /// The label of the last message.
    pub last_label: String,
    /// The first [`LAST_TEXT`] characters of the last message.
    pub last_text: String,
    /// Messages heard.
    pub messages: u64,
    /// The last position, with the signal time it was heard.
    pub fix: Option<(f64, Fix)>,
    /// Positions heard.
    pub positions: u64,
    /// Track, degrees true.
    pub track_deg: Option<f64>,
    /// The signal time of the last track that a message provided.
    #[serde(default)]
    pub track_reported_at: Option<f64>,
    /// Ground speed, knots.
    pub gs_kt: Option<f64>,
    /// Vertical rate, feet per minute.
    pub vs_fpm: Option<f64>,
    /// The predicted route: the next waypoint and the one after (position, altitude in feet).
    pub route: Option<[(Position, f64); 2]>,
    /// The trail. Each point carries its altitude in feet.
    pub trail: Trail<TrailMark>,
    /// The link's own data.
    pub extra: X,
}

/// The most characters kept of the last message.
pub const LAST_TEXT: usize = 200;

impl<X: Serialize> Serialize for Aircraft<X> {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        Aircraft::serialize(self, s)
    }
}

/// Reads what [`Serialize`] wrote. The country comes from the address ([`sdrkit_util_ids::icao::country`]), as for a new row.
impl<'de, X: Deserialize<'de>> Deserialize<'de> for Aircraft<X> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let mut a = Aircraft::deserialize(d)?;
        a.country = a.icao.and_then(sdrkit_util_ids::icao::country);
        Ok(a)
    }
}

/// Moves the time of the last fix and of every trail point, and the link data's own times.
impl<X: Shift> Shift for Aircraft<X> {
    fn shift_times(&mut self, by_s: f64) {
        if let Some((t, _)) = &mut self.fix {
            *t += by_s;
        }
        if let Some(t) = &mut self.track_reported_at {
            *t += by_s;
        }
        self.trail.shift_times(by_s);
        self.extra.shift_times(by_s);
    }
}

/// How a row of a link's table writes its cells and its map point.
///
/// Every `X` that is an [`Object`] gets the standard layout: the columns of [`columns::STANDARD`], then the columns of `X`. A
/// link that has its own columns implements this trait on its link data instead of [`Object`].
pub trait RowLayout: Sized {
    /// Writes the cells of `row` to `out`.
    fn row_write(row: &Aircraft<Self>, seen: Option<&Seen>, now_s: f64, out: &mut impl Cells);

    /// The map point of `row`, or `None` when it has no position.
    fn row_point(row: &Aircraft<Self>) -> Option<Point<'_>>;
}

/// The standard layout. The point is the last fix, labelled with the flight id, else the registration, else the address, and
/// headed along the track.
impl<X: Object> RowLayout for X {
    fn row_write(row: &Aircraft<X>, seen: Option<&Seen>, now_s: f64, out: &mut impl Cells) {
        row.write_standard(seen, out);
        row.extra.write(seen, now_s, out);
    }

    fn row_point(row: &Aircraft<X>) -> Option<Point<'_>> {
        row.standard_point()
    }
}

/// The cells and the point defined by the link data's [`RowLayout`].
impl<X: RowLayout> Object for Aircraft<X> {
    fn write(&self, seen: Option<&Seen>, now_s: f64, out: &mut impl Cells) {
        X::row_write(self, seen, now_s, out);
    }

    fn point(&self) -> Option<Point<'_>> {
        X::row_point(self)
    }
}

/// The columns an [`Aircraft`] row writes, in order. A family declares its table with these, then its link data's columns.
///
/// ```
/// use sdrkit_proto_aero::aircraft::{columns, AircraftTable, Fix, Source};
/// use sdrkit_core::publish::{Declare, Published, TableDef};
/// use sdrkit_util_geo::Position;
/// use sdrkit_util_objects::{publish, Stamp};
///
/// const AIRCRAFT: TableDef = TableDef {
///     key: "aircraft",
///     label: "Aircraft",
///     columns: columns::STANDARD,
///     cap: 100,
/// };
/// let mut t: AircraftTable = AircraftTable::new(100);
/// let a = t.heard(Some(0x7C1234), "VH-OQA", Stamp::at(10.0)).unwrap();
/// let fix = Fix { position: Position::new(-33.0, 151.0), alt_ft: Some(37000.0), on_ground: false, source: Source::Text };
/// a.position(10.0, fix, None).unwrap();
/// let mut p = Published::new(&[]).with_tables(&[&AIRCRAFT]);
/// publish::rows(t.rows(), &AIRCRAFT, 25.0, &mut p);
/// publish::points(t.rows(), "aircraft/", p.results_mut());
/// let r = p.results();
/// let table = r.table(r.decl().table_id("aircraft").unwrap()).unwrap();
/// assert_eq!(table.row("7C1234").unwrap().get(&columns::REGISTRATION), Some("VH-OQA"));
/// assert_eq!(r.points()["aircraft/7C1234"].label, "VH-OQA");
/// ```
pub mod columns {
    use sdrkit_core::publish::{field, Declare, Field};

    /// The 24-bit address, six hex digits.
    pub const ADDRESS: Field<field::Text> = Field::new("address", "Address", "");
    /// The registration.
    pub const REGISTRATION: Field<field::Text> = Field::new("registration", "Registration", "");
    /// The State the address is allocated to.
    pub const COUNTRY: Field<field::Text> = Field::new("country", "Country", "");
    /// The flight id.
    pub const FLIGHT: Field<field::Text> = Field::new("flight", "Flight", "");
    /// The label of the last message.
    pub const LABEL: Field<field::Text> = Field::new("label", "Label", "");
    /// Messages heard.
    pub const MESSAGES: Field<field::I64> = Field::new("messages", "Messages", "");
    /// The last position.
    pub const POSITION: Field<field::Position> = Field::new("position", "Position", "");
    /// The altitude of the last position, feet.
    pub const ALT_FT: Field<field::F64> = Field::new("alt_ft", "Altitude", "ft");
    /// The track, degrees true.
    pub const TRACK_DEG: Field<field::F64> = Field::new("track_deg", "Track", "deg");
    /// The ground speed, knots.
    pub const GS_KT: Field<field::F64> = Field::new("gs_kt", "Ground speed", "kt");
    /// The vertical rate, feet per minute.
    pub const VS_FPM: Field<field::F64> = Field::new("vs_fpm", "Vertical rate", "ft/min");

    /// Every column of the standard layout of an [`Aircraft`](super::Aircraft) row, before the columns of its link data.
    pub const STANDARD: &[&dyn Declare] = &[
        &ADDRESS,
        &REGISTRATION,
        &COUNTRY,
        &FLIGHT,
        &LABEL,
        &MESSAGES,
        &POSITION,
        &ALT_FT,
        &TRACK_DEG,
        &GS_KT,
        &VS_FPM,
        &sdrkit_util_objects::publish::columns::LAST_HEARD,
    ];
}

/// The values of the columns of [`columns::STANDARD`] of one aircraft, in the types of a row shape. A stack with an
/// interface description copies them into the struct of its row, whose fields name the constants of [`columns`] with
/// `#[field(shared = ...)]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Standard<'a> {
    /// The 24-bit address, six hex digits.
    pub address: Option<&'a str>,
    /// The registration.
    pub registration: Option<&'a str>,
    /// The State the address is allocated to.
    pub country: Option<&'a str>,
    /// The flight id.
    pub flight: Option<&'a str>,
    /// The label of the last message.
    pub label: Option<&'a str>,
    /// Messages heard.
    pub messages: i64,
    /// The last position, latitude first.
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position, feet.
    pub alt_ft: Option<f64>,
    /// The track, degrees true.
    pub track_deg: Option<f64>,
    /// The ground speed, knots.
    pub gs_kt: Option<f64>,
    /// The vertical rate, feet per minute.
    pub vs_fpm: Option<f64>,
    /// When the aircraft was last heard.
    pub last_heard: Option<f64>,
}

impl<X> Aircraft<X> {
    /// The values of the columns of [`columns::STANDARD`]. The address text is written into `address`.
    pub fn standard<'a>(&'a self, seen: Option<&Seen>, address: &'a mut [u8; 7]) -> Standard<'a> {
        fn text(s: &str) -> Option<&str> {
            (!s.is_empty()).then_some(s)
        }
        let fix = self.fix.map(|(_, f)| f);
        Standard {
            address: self.address_into(address),
            registration: text(&self.registration),
            country: self.country.and_then(text),
            flight: text(&self.flight),
            label: text(&self.last_label),
            messages: i64::try_from(self.messages).unwrap_or(i64::MAX),
            position: fix.map(|f| (f.position.lat_deg, f.position.lon_deg)),
            alt_ft: fix.and_then(|f| f.alt_ft),
            track_deg: self.track_deg,
            gs_kt: self.gs_kt,
            vs_fpm: self.vs_fpm,
            last_heard: sdrkit_util_objects::publish::columns::last_heard(seen),
        }
    }

    /// The map point of the standard layout: the last fix, labelled with the flight id, else the registration, else
    /// the address, and headed along the track. `None` without a position. A stack with an interface description
    /// gives it to `sdrkit_util_objects::shaped::points_by` for the rows of its aircraft table.
    ///
    /// ```
    /// use sdrkit_proto_aero::aircraft::{Aircraft, AircraftTable, Fix, Source};
    /// use sdrkit_util_geo::Position;
    /// use sdrkit_util_objects::Stamp;
    ///
    /// let mut t: AircraftTable = AircraftTable::new(10);
    /// let a = t.heard(Some(0x7C1234), "VH-OQA", Stamp::at(10.0)).unwrap();
    /// assert!(a.standard_point().is_none());
    /// let fix = Fix { position: Position::new(-33.0, 151.0), alt_ft: None, on_ground: false, source: Source::Text };
    /// a.position(10.0, fix, None).unwrap();
    /// assert_eq!(a.standard_point().unwrap().label, "VH-OQA");
    /// ```
    pub fn standard_point(&self) -> Option<Point<'_>> {
        let (_, fix) = self.fix?;
        let label = if !self.flight.is_empty() {
            self.flight.clone()
        } else if !self.registration.is_empty() {
            self.registration.clone()
        } else {
            self.address_text().unwrap_or_default()
        };
        let p = Point::new(fix.position, label);
        Some(match self.track_deg {
            Some(t) => p.heading(t as f32),
            None => p,
        })
    }

    /// Writes the columns of [`columns::STANDARD`] cell by cell, for a stack without an interface description.
    pub fn write_standard(&self, seen: Option<&Seen>, out: &mut impl Cells) {
        let mut address = [0u8; 7];
        let s = self.standard(seen, &mut address);
        out.opt(&columns::ADDRESS, s.address);
        out.opt(&columns::REGISTRATION, s.registration);
        out.opt(&columns::COUNTRY, s.country);
        out.opt(&columns::FLIGHT, s.flight);
        out.opt(&columns::LABEL, s.label);
        out.set(&columns::MESSAGES, s.messages);
        out.opt(&columns::POSITION, s.position);
        out.opt(&columns::ALT_FT, s.alt_ft);
        out.opt(&columns::TRACK_DEG, s.track_deg);
        out.opt(&columns::GS_KT, s.gs_kt);
        out.opt(&columns::VS_FPM, s.vs_fpm);
        out.opt(
            &sdrkit_util_objects::publish::columns::LAST_HEARD,
            s.last_heard,
        );
    }

    /// The address as the row id shows it: six hex digits, with `~` before a non-ICAO address. `None` without an address.
    pub fn address_text(&self) -> Option<String> {
        match (self.icao, self.non_icao) {
            (Some(a), _) => Some(format!("{a:06X}")),
            (None, Some(a)) => Some(format!("~{a:06X}")),
            (None, None) => None,
        }
    }

    /// The text of [`Aircraft::address_text`], written into `buf`.
    fn address_into<'b>(&self, buf: &'b mut [u8; 7]) -> Option<&'b str> {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let (a, start) = match (self.icao, self.non_icao) {
            (Some(a), _) => (a, 1),
            (None, Some(a)) => (a, 0),
            (None, None) => return None,
        };
        buf[0] = b'~';
        for (i, b) in buf[1..].iter_mut().enumerate() {
            *b = HEX[((a >> (20 - 4 * i)) & 0xF) as usize];
        }
        std::str::from_utf8(&buf[start..]).ok()
    }
}

impl<X: Default + Clone> Aircraft<X> {
    fn new(icao: Option<u32>, trail: &TrailSettings) -> Aircraft<X> {
        Aircraft {
            icao,
            non_icao: None,
            registration: String::new(),
            country: icao.and_then(sdrkit_util_ids::icao::country),
            flight: String::new(),
            last_label: String::new(),
            last_text: String::new(),
            messages: 0,
            fix: None,
            positions: 0,
            track_deg: None,
            track_reported_at: None,
            gs_kt: None,
            vs_fpm: None,
            route: None,
            trail: trail.trail(),
            extra: X::default(),
        }
    }

    /// Notes a message: its flight id (when not empty), label and text.
    pub fn message(&mut self, flight: &str, label: &str, text: &str) {
        self.messages += 1;
        let f = flight.trim();
        if !f.is_empty() && self.flight != f {
            self.flight.clear();
            self.flight.push_str(f);
        }
        if self.last_label != label {
            self.last_label.clear();
            self.last_label.push_str(label);
        }
        self.last_text.clear();
        self.last_text.extend(text.chars().take(LAST_TEXT));
    }

    /// Notes an ADS-C message heard at signal time `t`, when its CRC is good. It takes the position of the first report, the
    /// motion of the earth reference, the flight id and the predicted route. Returns true when the message held a position.
    pub fn adsc(&mut self, t: f64, a: &crate::adsc::Adsc) -> bool {
        use crate::adsc::Group;
        if !a.crc_ok {
            return false;
        }
        for g in a.groups() {
            match g {
                Group::FlightId { .. } => {
                    if let Some(f) = g.flight_id_text().filter(|f| !f.is_empty()) {
                        self.flight.clear();
                        self.flight.push_str(f);
                    }
                }
                Group::PredictedRoute { next, then, .. } => {
                    let p =
                        |w: &crate::adsc::Waypoint| (Position::new(w.lat_deg, w.lon_deg), w.alt_ft);
                    self.route = Some([p(next), p(then)]);
                }
                _ => {}
            }
        }
        let motion = a
            .earth_reference()
            .map(|(track_deg, gs_kt, vs_fpm)| Motion {
                track_deg,
                gs_kt,
                vs_fpm: Some(vs_fpm),
            });
        match (a.position(), a.report()) {
            (Some(position), Some(r)) => {
                let fix = Fix {
                    position,
                    alt_ft: Some(r.alt_ft),
                    on_ground: false,
                    source: Source::Adsc,
                };
                self.position(t, fix, motion).is_ok()
            }
            _ => false,
        }
    }

    /// Records a position heard at signal time `t`, with the motion the message provides (if any). A position that differs from
    /// the last trail point is added to the trail, if the trail spacing allows it. Without a given track, the track is the bearing
    /// from the last trail point when the aircraft moved more than [`MIN_MOVE_M`] and that point is newer than the last reported
    /// track, so a reported track stays until two newer positions replace it. Returns an error for a time that is not finite or a
    /// position out of range.
    pub fn position(&mut self, t: f64, fix: Fix, motion: Option<Motion>) -> Result<()> {
        if !t.is_finite() {
            return Err(Error::BadField(Item::Time));
        }
        let last = self.trail.last().map(|p| (p.t, p.position));
        if last.map(|l| l.1) != Some(fix.position) {
            self.trail
                .push(
                    t,
                    fix.position,
                    TrailMark {
                        alt_ft: fix.alt_ft,
                        on_ground: fix.on_ground,
                    },
                )
                .map_err(|_| Error::BadField(Item::Position))?;
        }
        match motion {
            Some(m) => {
                self.track_deg = Some(m.track_deg);
                self.track_reported_at = Some(t);
                self.gs_kt = Some(m.gs_kt);
                self.vs_fpm = m.vs_fpm.or(self.vs_fpm);
            }
            None => {
                let after_report = |pt: f64| self.track_reported_at.is_none_or(|r| pt > r);
                if let Some((_, p)) = last
                    .filter(|(pt, p)| after_report(*pt) && p.distance_m(&fix.position) > MIN_MOVE_M)
                {
                    self.track_deg = Some(p.initial_bearing_deg(&fix.position));
                }
            }
        }
        self.fix = Some((t, fix));
        self.positions += 1;
        Ok(())
    }

    /// Ground speed, track and vertical rate from the last two trail points, when they are `min_dt` to `max_dt` seconds apart.
    pub fn derived(&self, min_dt: f64, max_dt: f64) -> Option<Motion> {
        let mut it = self.trail.points().rev();
        let (b, a) = (it.next()?, it.next()?);
        let dt = b.t - a.t;
        if !(min_dt..=max_dt).contains(&dt) {
            return None;
        }
        let d_nm = a.position.distance_m(&b.position) / 1852.0;
        Some(Motion {
            track_deg: a.position.initial_bearing_deg(&b.position),
            gs_kt: d_nm / dt * 3600.0,
            vs_fpm: a
                .extra
                .alt_ft
                .zip(b.extra.alt_ft)
                .map(|(x, y)| (y - x) / dt * 60.0),
        })
    }

    /// The position at signal time `t` by dead reckoning from the last fix along the track at the ground speed. `None` without a
    /// fix, a track or a speed.
    pub fn estimate(&self, t: f64) -> Option<Position> {
        let (t0, fix) = self.fix?;
        let d_m = self.gs_kt? * 1852.0 / 3600.0 * (t - t0).max(0.0);
        Some(fix.position.destination(self.track_deg?, d_m))
    }

    fn merge(&mut self, o: &Aircraft<X>) {
        self.messages += o.messages;
        if self.registration.is_empty() {
            self.registration.clone_from(&o.registration);
        }
        if self.flight.is_empty() {
            self.flight.clone_from(&o.flight);
        }
        if self.last_label.is_empty() {
            self.last_label.clone_from(&o.last_label);
            self.last_text.clone_from(&o.last_text);
        }
        if newer(&o.fix, &self.fix) {
            self.fix = o.fix;
            self.track_deg = o.track_deg.or(self.track_deg);
            self.track_reported_at = o.track_reported_at.or(self.track_reported_at);
            self.gs_kt = o.gs_kt.or(self.gs_kt);
            self.vs_fpm = o.vs_fpm.or(self.vs_fpm);
        }
        self.positions = self.positions.max(o.positions);
        if o.trail.len() > self.trail.len() {
            self.trail = o.trail.clone();
        }
        if self.route.is_none() {
            self.route = o.route;
        }
    }
}

/// True when `a` holds a fix heard after `b` (or `b` holds none).
fn newer(a: &Option<(f64, Fix)>, b: &Option<(f64, Fix)>) -> bool {
    match (a, b) {
        (Some(_), None) => true,
        (Some(x), Some(y)) => x.0 > y.0,
        (None, _) => false,
    }
}

/// The registration key: upper case, dashes, dots and spaces removed.
fn reg_key(r: &str) -> String {
    crate::msglog::reg_key(r)
}

/// The most registration-to-address links kept before the list starts again.
pub const ALIASES: usize = 4000;

/// The table. `X` is the link's own data in each row.
///
/// It is saved with serde (the rows and the registration-to-address links). Its signal times move with [`Shift`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "X: Serialize", deserialize = "X: DeserializeOwned"))]
pub struct AircraftTable<X = ()> {
    rows: ObjectTable<Key, Aircraft<X>>,
    alias: HashMap<String, u32>,
    #[serde(default)]
    trail: TrailSettings,
}

impl<X: Default + Clone> AircraftTable<X> {
    /// A table of at most `cap` aircraft, with the default trails ([`TrailSettings::default`]). If it is full, the least recently
    /// heard leaves.
    pub fn new(cap: usize) -> Self {
        AircraftTable {
            rows: ObjectTable::new(cap),
            alias: HashMap::new(),
            trail: TrailSettings::default(),
        }
    }

    /// The same table with trails kept by `trail`.
    pub fn with_trail(mut self, trail: TrailSettings) -> Self {
        self.set_trail(trail);
        self
    }

    /// How the trails are kept.
    pub fn trail(&self) -> TrailSettings {
        self.trail
    }

    /// Keeps trails by `trail` from now on. The trails of the rows keep the newest points that the new settings allow.
    pub fn set_trail(&mut self, trail: TrailSettings) {
        if trail == self.trail {
            return;
        }
        self.trail = trail;
        for (_, a) in self.rows.records_mut() {
            a.trail = trail.refit(&a.trail);
        }
    }

    /// The row of an aircraft heard at `at`, made when new. The key is the address when known. Without one, it is
    /// the address that a message provided earlier with this registration, and else the registration. When a message provides both, the
    /// registration's row folds into the address row. `None` for a time that is not finite, or for no address and no
    /// registration.
    pub fn heard(
        &mut self,
        icao: Option<u32>,
        registration: &str,
        at: Stamp,
    ) -> Option<&mut Aircraft<X>> {
        let reg = registration.trim_start_matches('.').trim();
        let rk = reg_key(reg);
        if let (Some(a), false) = (icao, rk.is_empty()) {
            if self.alias.len() >= ALIASES {
                self.alias.clear();
            }
            self.alias.insert(rk.clone(), a);
        }
        let icao = icao.or_else(|| self.alias.get(&rk).copied());
        let key = match icao {
            Some(a) => Key::Address(a),
            None if !rk.is_empty() => Key::Registration(rk.clone()),
            None => return None,
        };
        let folded = match (icao, rk.is_empty()) {
            (Some(_), false) => self.rows.remove(&Key::Registration(rk)),
            _ => None,
        };
        let up = self
            .rows
            .upsert(&key, at, || Aircraft::new(icao, &self.trail))
            .ok()?;
        let row = up.row;
        if let Some(old) = folded {
            row.merge(&old.record);
        }
        if !reg.is_empty() && row.registration != reg {
            row.registration.clear();
            row.registration.push_str(reg);
        }
        if row.registration.is_empty() {
            if let Some(r) = icao.and_then(sdrkit_util_ids::icao::registration) {
                row.registration.push_str(r.as_str());
            }
        }
        Some(row)
    }

    /// The row of an aircraft heard at `at` by its 24-bit `address` alone, made when new. With `non_icao` set, the
    /// address is not an ICAO address: the row is keyed apart ([`Key::NonIcao`]), has no country and no registration, and never
    /// folds into an ICAO row with the same number. `None` for a time that is not finite or an address above 24 bits.
    pub fn heard_address(
        &mut self,
        address: u32,
        non_icao: bool,
        at: Stamp,
    ) -> Option<&mut Aircraft<X>> {
        if address > 0xFF_FFFF {
            return None;
        }
        if !non_icao {
            return self.heard(Some(address), "", at);
        }
        let trail = &self.trail;
        let up = self
            .rows
            .upsert(&Key::NonIcao(address), at, || {
                let mut a = Aircraft::new(None, trail);
                a.non_icao = Some(address);
                a
            })
            .ok()?;
        Some(up.row)
    }

    /// Returns the row of an aircraft heard at `at` that ground station `ground_station` identifies by `aircraft_id`, before its
    /// address is known. The row is created if it does not exist. Returns `None` for a time that is not finite.
    pub fn heard_pending(
        &mut self,
        ground_station: u8,
        aircraft_id: u8,
        at: Stamp,
    ) -> Option<&mut Aircraft<X>> {
        let trail = &self.trail;
        let key = Key::Pending {
            ground_station,
            aircraft_id,
        };
        let up = self
            .rows
            .upsert(&key, at, || Aircraft::new(None, trail))
            .ok()?;
        Some(up.row)
    }

    /// Sets the address of the aircraft that ground station `ground_station` identifies by `aircraft_id`: its pending row is
    /// merged into the row of `address`, which is heard at `at`. Returns that row, or `None` for a time that is not finite or an
    /// address above 24 bits.
    pub fn resolve_pending(
        &mut self,
        ground_station: u8,
        aircraft_id: u8,
        address: u32,
        at: Stamp,
    ) -> Option<&mut Aircraft<X>> {
        if address > 0xFF_FFFF {
            return None;
        }
        let pending = self.rows.remove(&Key::Pending {
            ground_station,
            aircraft_id,
        });
        let row = self.heard(Some(address), "", at)?;
        if let Some(old) = pending {
            row.merge(&old.record);
        }
        Some(row)
    }

    /// Removes the row of `key`. Returns it, or `None` if the table has no such row.
    pub fn remove(&mut self, key: &Key) -> Option<Aircraft<X>> {
        self.rows.remove(key).map(|e| e.record)
    }

    /// The row of an aircraft by its 24-bit address, ICAO or not ([`AircraftTable::heard_address`]).
    pub fn get_address(&self, address: u32, non_icao: bool) -> Option<&Aircraft<X>> {
        self.rows.get(&address_key(address, non_icao))
    }

    /// The row of an aircraft by its 24-bit address, ICAO or not, to change. This does not count as hearing it.
    pub fn get_address_mut(&mut self, address: u32, non_icao: bool) -> Option<&mut Aircraft<X>> {
        self.rows.get_mut(&address_key(address, non_icao))
    }

    /// The row of an aircraft, by address or registration.
    pub fn get(&self, icao: Option<u32>, registration: &str) -> Option<&Aircraft<X>> {
        let rk = reg_key(registration);
        match icao.or_else(|| self.alias.get(&rk).copied()) {
            Some(a) => self.rows.get(&Key::Address(a)),
            None => self.rows.get(&Key::Registration(rk)),
        }
    }

    /// The row of an aircraft, by address or registration, to change. This does not count as hearing it: the row keeps its
    /// place and its last-heard time.
    pub fn get_mut(&mut self, icao: Option<u32>, registration: &str) -> Option<&mut Aircraft<X>> {
        let rk = reg_key(registration);
        match icao.or_else(|| self.alias.get(&rk).copied()) {
            Some(a) => self.rows.get_mut(&Key::Address(a)),
            None => self.rows.get_mut(&Key::Registration(rk)),
        }
    }

    /// Folds the rows of another table (the same aircraft heard by another receiver window) into this one. Message counts add,
    /// the newest position wins, empty names are filled in and the longer trail is kept.
    pub fn merge_from(&mut self, other: &AircraftTable<X>) {
        for r in other.rows.iter() {
            let at = r.seen.last();
            let trail = &self.trail;
            if let Ok(up) = self
                .rows
                .upsert(r.key, at, || Aircraft::new(r.record.icao, trail))
            {
                if up.new {
                    *up.row = r.record.clone();
                } else {
                    up.row.merge(r.record);
                }
            }
        }
        for (k, v) in &other.alias {
            self.alias.entry(k.clone()).or_insert(*v);
        }
    }

    /// The rows, most recently heard first.
    pub fn newest_first(&self) -> impl Iterator<Item = RowRef<'_, Key, Aircraft<X>>> + '_ {
        self.rows.iter_newest_first()
    }

    /// The rows whose address (hex), registration or flight holds `q` (case ignored), most recently heard first.
    pub fn search<'a>(
        &'a self,
        q: &'a str,
    ) -> impl Iterator<Item = RowRef<'a, Key, Aircraft<X>>> + 'a {
        let q = q.to_uppercase();
        self.rows.iter_newest_first().filter(move |r| {
            let a = r.record;
            a.address_text().is_some_and(|t| t.contains(&q))
                || a.registration.to_uppercase().contains(&q)
                || a.flight.to_uppercase().contains(&q)
        })
    }

    /// Removes the rows not heard within `max_age_s` of signal time `now`. Returns how many left.
    pub fn expire(&mut self, now: f64, max_age_s: f64) -> usize {
        self.rows.expire(now, max_age_s)
    }

    /// The number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Forgets every row.
    pub fn clear(&mut self) {
        self.rows.clear();
        self.alias.clear();
    }

    /// Every row, to change. This does not count as hearing them.
    pub fn records_mut(&mut self) -> impl Iterator<Item = &mut Aircraft<X>> + '_ {
        self.rows.records_mut().map(|(_, a)| a)
    }

    /// The table that [`sdrkit_util_objects`] keeps, for publishing and saved state.
    pub fn rows(&self) -> &ObjectTable<Key, Aircraft<X>> {
        &self.rows
    }

    /// Takes the rows and links of `saved` (a restored state) and keeps this table's cap and trail settings. If `saved` holds
    /// more rows, the least recently heard leave.
    pub fn adopt(&mut self, saved: AircraftTable<X>) {
        self.rows.adopt(saved.rows);
        self.alias = saved.alias;
        if saved.trail != self.trail {
            let trail = self.trail;
            for (_, a) in self.rows.records_mut() {
                a.trail = trail.refit(&a.trail);
            }
        }
    }
}

/// The key of a 24-bit address, ICAO or not.
fn address_key(address: u32, non_icao: bool) -> Key {
    if non_icao {
        Key::NonIcao(address)
    } else {
        Key::Address(address)
    }
}

/// Moves the signal times of every row.
impl<X: Shift> Shift for AircraftTable<X> {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

/// Returns the time of a report as seconds of the UTC day. A text report carries HHMMSS (`sec_of_day`). An ADS-C report carries
/// seconds past the hour, which are placed in the hour of the wall clock, or in the hour before if that would be more than
/// 120 s in the future. Without either, returns the wall clock's time of day.
pub fn report_time(sec_of_day: Option<f64>, sec_past_hour: Option<f64>, wall_unix: f64) -> f64 {
    let wall_day = wall_unix.rem_euclid(86_400.0);
    if let Some(t) = sec_of_day {
        return t;
    }
    if let Some(s) = sec_past_hour {
        let mut t = (wall_day / 3600.0).floor() * 3600.0 + s;
        if t - wall_day > 120.0 {
            t -= 3600.0;
        }
        return t.rem_euclid(86_400.0);
    }
    wall_day
}
