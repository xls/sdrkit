//! The aircraft table: what every accepted message updates, CPR position decoding, squawk confirmation, emergencies, Comm-B,
//! derived wind and temperature, trails, range statistics and the GPS interference indicator.
//!
//! [`Table`] is the plain type. [`Tracker`] (`adsb.tracker`, `modes.frame` in, `adsb.feed` and `adsb.aircraft` out) is the component.

use crate::checks::{Accepted, CheckStats, Msg};
use crate::codes::{squawk_text, Callsign};
use crate::commb::{self, Bds};
use crate::feeds::{FeedItem, RemoteState, SbsFields, SourceStatus, TargetReport};
use crate::geo::{bearing_deg, distance_nm};
use crate::message::{
    emergency_name, es, flight_status, header, Es, Frame, Header, SpeedKind, NON_ICAO,
};
use crate::settings::{DROP_S, EXPIRY_S, GPS_LAYER};
use crate::typed::{
    self, series, AircraftChange, AircraftRow, AircraftSeen, FixChoice, GpsCellRow, MessageRow,
    SourceRow, TrackerFields, AIRCRAFT, AIRCRAFT_SEEN, BY_DF, GPS_CELLS, MESSAGES, SOURCES,
};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields, Hertz, Time};
use sdrkit_core::publish::{keys, Published};
use sdrkit_core::results::Series;
use sdrkit_core::{
    Clock, ConfigError, EventDef, LockState, Record, Results, SettingError, SettingId, SettingSet,
    StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom,
    RunContext, Timed,
};
use sdrkit_proto_aero::air::{declination_deg, sat_from_tas_mach, wind_triangle};
use sdrkit_proto_aero::aircraft::{AircraftTable, Fix, Key, Source, TrailSettings};
use sdrkit_util_geo::cpr::{self, CprFormat, CprFrame};
use sdrkit_util_geo::NAUTICAL_MILE_M;
use sdrkit_util_geo::{Position as GeoPosition, Text};
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Spacing, Stamp, Tables};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

/// How a position was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosKind {
    /// From an even/odd pair.
    Global,
    /// Local, from a confirmed position.
    Local,
    /// Local, from the receiver position (or a chain of local decodes that started there).
    LocalFromReceiver,
}

impl PosKind {
    fn confirmed(self) -> bool {
        self != PosKind::LocalFromReceiver
    }
}

/// A position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    /// Degrees.
    pub lat: f64,
    /// Degrees.
    pub lon: f64,
    /// Signal time it was taken.
    pub time: f64,
    /// How.
    pub kind: PosKind,
}

impl Shift for Position {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// The last 5,0 report: roll, track, ground speed, track rate, TAS, and when.
pub type B50Held = (
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    Option<f64>,
    f64,
);

/// One aircraft: a row of the shared aircraft table with the Mode S data of [`ModeS`].
///
/// The shared row keeps the address, the callsign (`flight`), the last position with its barometric altitude (`fix`), the ground
/// speed (`gs_kt`), the track (`track_deg`), the vertical rate (`vs_fpm`) and the trail.
pub type Aircraft = sdrkit_proto_aero::aircraft::Aircraft<ModeS>;

/// What Mode S and ADS-B add to a row of the shared aircraft table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ModeS {
    /// Mean pulse power (linear).
    pub level_lin: Option<f32>,
    /// Heard through TIS-B.
    pub tisb: bool,
    /// On the ground.
    pub on_ground: Option<bool>,
    /// Alert.
    pub alert: bool,
    /// SPI.
    pub spi: bool,
    /// Emitter category.
    pub category: Option<(char, u8)>,
    /// Barometric altitude and when.
    pub baro_alt: Option<(i32, f64)>,
    /// Altitude from an extended squitter and when.
    pub trusted_alt: Option<(i32, f64)>,
    /// GNSS height.
    pub gnss_alt: Option<i32>,
    /// The kind of the newest speed. For a ground speed, the value is the row's `gs_kt`. Otherwise it is `airspeed_kt`.
    pub speed_kind: Option<SpeedKind>,
    /// The newest airspeed (IAS or TAS), knots.
    pub airspeed_kt: Option<f64>,
    /// Whether the newest heading is the track. For a track, the value is the row's `track_deg`. Otherwise it is `heading_deg`.
    pub heading_is_track: Option<bool>,
    /// The newest magnetic heading, degrees.
    pub heading_deg: Option<f64>,
    /// GNSS minus barometric.
    pub gnss_diff: Option<i32>,
    /// Selected altitude.
    pub sel_alt: Option<i32>,
    /// Selected heading.
    pub sel_heading: Option<f64>,
    /// Baro setting.
    pub baro_setting: Option<f64>,
    /// ADS-B version.
    pub version: Option<u8>,
    /// NACp.
    pub nacp: Option<u8>,
    /// SIL.
    pub sil: Option<u8>,
    /// NIC from the position type code.
    pub nic: Option<u8>,
    /// Squawk.
    pub squawk: Option<u16>,
    /// A DF5/21 squawk change waiting for its confirmation.
    pub pending_squawk: Option<(u16, f64)>,
    /// TC 28 emergency state.
    pub tc28_emergency: u8,
    /// How the row's position was found.
    pub pos_kind: Option<PosKind>,
    /// Stored airborne CPR frames (even, odd) with their times.
    pub cpr: [Option<(u32, u32, f64)>; 2],
    /// Stored surface CPR frames.
    pub cpr_surface: [Option<(u32, u32, f64)>; 2],
    /// Jump check: positions dropped in a row.
    pub jumps: u8,
    /// Roll (5,0).
    pub roll: Option<f64>,
    /// IAS (6,0).
    pub ias: Option<f64>,
    /// Mach (6,0).
    pub mach: Option<f64>,
    /// TAS (5,0).
    pub tas: Option<f64>,
    /// Last ADS-B velocity.
    pub adsb_velocity_time: f64,
    /// Last 5,0: (roll, track, gs, rate, tas, time).
    pub b50: Option<B50Held>,
    /// Last 6,0: (heading, mach, time).
    pub b60: Option<(Option<f64>, Option<f64>, f64)>,
    /// Derived wind vector (east, north, kt) and time.
    pub wind_derived: Option<(f64, f64, f64)>,
    /// Derived SAT and time.
    pub sat_derived: Option<(f64, f64)>,
    /// Reported wind (speed, from) and time.
    pub wind_reported: Option<(f64, f64, f64)>,
    /// Reported SAT and time.
    pub sat_reported: Option<(f64, f64)>,
    /// Reported pressure.
    pub pressure: Option<f64>,
    /// Reported humidity.
    pub humidity: Option<f64>,
    /// Turbulence, wind shear, microburst, icing, wake vortex.
    pub hazards: [Option<u8>; 5],
    /// ACAS RA: (ARA, RAC, terminated, threat, time).
    pub advisory: Option<(u16, u8, bool, Option<u32>, f64)>,
    /// Subnetwork version and ACAS RA capability (1,0).
    pub capability: Option<(u8, bool)>,
    /// GICB mask (1,7).
    pub gicb: Option<u32>,
    /// The source of the last update (0 this receiver).
    pub source: u16,
    /// Track number.
    pub track_no: u16,
    /// Last update event.
    pub last_event: f64,
    /// Distance (m) and bearing from the station, at the last report.
    pub range: Option<(f64, f64)>,
    /// Sent other data-link messages (Comm-B).
    pub datalink: bool,
    /// The texts of the row, made again at each report only when their source changed.
    #[serde(skip)]
    pub texts: RowTexts,
}

/// The text cells of a row, kept so that a report that changes nothing writes nothing new.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowTexts {
    /// The packed trail ([`sdrkit_proto_aero::trail::encode`]).
    pub trail: Vec<u8>,
    /// The trail points and the newest point time that `trail` holds.
    trail_of: (usize, f64),
    /// The hazards with their levels.
    pub hazards: String,
    hazards_of: [Option<u8>; 5],
    /// The resolution advisory in words.
    pub advisory: String,
    advisory_of: Option<(u16, u8, bool)>,
    /// The GICB registers.
    pub gicb: String,
    gicb_of: Option<u32>,
}

impl ModeS {
    /// The emergency shown.
    pub fn emergency(&self) -> u8 {
        if self.tc28_emergency != 0 {
            return self.tc28_emergency;
        }
        match self.squawk {
            Some(0o7500) => 5,
            Some(0o7600) => 4,
            Some(0o7700) => 1,
            _ => 0,
        }
    }
    /// Level in dBFS.
    pub fn level_dbfs(&self) -> Option<f32> {
        self.level_lin.map(|l| 10.0 * l.max(1e-12).log10())
    }
    /// The CPR frames held at signal time `now` without a position.
    pub fn cpr_held(&self, now: f64) -> Option<CprHeld> {
        let e = self.cpr[0].is_some_and(|f| now - f.2 <= 15.0);
        let o = self.cpr[1].is_some_and(|f| now - f.2 <= 15.0);
        match (e, o) {
            (true, true) => Some(CprHeld::Pair),
            (true, false) => Some(CprHeld::Even),
            (false, true) => Some(CprHeld::Odd),
            _ => None,
        }
    }
}

/// The airborne CPR frames that wait for a position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CprHeld {
    /// An even and an odd frame.
    Pair,
    /// An even frame.
    Even,
    /// An odd frame.
    Odd,
}

impl Shift for ModeS {
    fn shift_times(&mut self, by_s: f64) {
        self.adsb_velocity_time += by_s;
        self.last_event += by_s;
        for t in [&mut self.baro_alt, &mut self.trusted_alt]
            .into_iter()
            .flatten()
        {
            t.1 += by_s;
        }
        for c in self
            .cpr
            .iter_mut()
            .chain(self.cpr_surface.iter_mut())
            .flatten()
        {
            c.2 += by_s;
        }
        if let Some(p) = self.pending_squawk.as_mut() {
            p.1 += by_s;
        }
        for t in [&mut self.wind_derived, &mut self.wind_reported]
            .into_iter()
            .flatten()
        {
            t.2 += by_s;
        }
        for t in [&mut self.sat_derived, &mut self.sat_reported]
            .into_iter()
            .flatten()
        {
            t.1 += by_s;
        }
        if let Some(x) = self.advisory.as_mut() {
            x.4 += by_s;
        }
        if let Some(x) = self.b50.as_mut() {
            x.5 += by_s;
        }
        if let Some(x) = self.b60.as_mut() {
            x.2 += by_s;
        }
    }
}

/// The ADS-B view of a row of the shared table: the 24-bit key, the callsign, the position, the speed and the heading.
pub trait ModeSRow {
    /// The key: the 24-bit address, with bit 24 ([`NON_ICAO`]) set for a non-ICAO address.
    fn key(&self) -> u32;
    /// The callsign, when one was heard.
    fn callsign(&self) -> Option<&str>;
    /// Sets the callsign.
    fn set_callsign(&mut self, c: &str);
    /// The last position.
    fn pos(&self) -> Option<Position>;
    /// The newest speed in knots, with its kind.
    fn speed(&self) -> Option<(f64, SpeedKind)>;
    /// Sets the newest speed. A ground speed goes to the row's `gs_kt`.
    fn set_speed(&mut self, kt: f64, kind: SpeedKind);
    /// The newest heading in degrees, and whether it is the track.
    fn heading(&self) -> Option<(f64, bool)>;
    /// Sets the newest heading. A track goes to the row's `track_deg`.
    fn set_heading(&mut self, deg: f64, is_track: bool);
}

impl ModeSRow for Aircraft {
    fn key(&self) -> u32 {
        match (self.icao, self.non_icao) {
            (Some(a), _) => a,
            (None, Some(a)) => a | NON_ICAO,
            (None, None) => 0,
        }
    }
    fn callsign(&self) -> Option<&str> {
        (!self.flight.is_empty()).then_some(self.flight.as_str())
    }
    fn set_callsign(&mut self, c: &str) {
        if self.flight != c {
            self.flight.clear();
            self.flight.push_str(c);
        }
    }
    fn pos(&self) -> Option<Position> {
        let (time, fix) = self.fix?;
        Some(Position {
            lat: fix.position.lat_deg,
            lon: fix.position.lon_deg,
            time,
            kind: self.extra.pos_kind.unwrap_or(PosKind::Global),
        })
    }
    fn speed(&self) -> Option<(f64, SpeedKind)> {
        match self.extra.speed_kind? {
            SpeedKind::Ground => self.gs_kt.map(|v| (v, SpeedKind::Ground)),
            k => self.extra.airspeed_kt.map(|v| (v, k)),
        }
    }
    fn set_speed(&mut self, kt: f64, kind: SpeedKind) {
        match kind {
            SpeedKind::Ground => self.gs_kt = Some(kt),
            _ => self.extra.airspeed_kt = Some(kt),
        }
        self.extra.speed_kind = Some(kind);
    }
    fn heading(&self) -> Option<(f64, bool)> {
        match self.extra.heading_is_track? {
            true => self.track_deg.map(|v| (v, true)),
            false => self.extra.heading_deg.map(|v| (v, false)),
        }
    }
    fn set_heading(&mut self, deg: f64, is_track: bool) {
        if is_track {
            self.track_deg = Some(deg);
        } else {
            self.extra.heading_deg = Some(deg);
        }
        self.extra.heading_is_track = Some(is_track);
    }
}

/// The key of the shared table for a 24-bit key with the [`NON_ICAO`] bit.
fn row_key(k: u32) -> Key {
    if k & NON_ICAO != 0 {
        Key::NonIcao(k & 0xFF_FFFF)
    } else {
        Key::Address(k & 0xFF_FFFF)
    }
}

/// Range statistics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Coverage {
    /// Farthest range per 10 degree sector, NM: `sectors[0]` for all heights, `sectors[1 + band]` for each band.
    pub sectors: Vec<Vec<f64>>,
    /// Range histogram, 50 bins of 10 NM.
    pub histogram: Vec<u64>,
    /// Positions counted.
    pub positions: u64,
    /// Maximum range, NM.
    pub max_range_nm: f64,
    /// Highest altitude seen.
    pub max_alt_ft: Option<i32>,
    /// Every 10 s of signal: (time, messages per second, aircraft).
    pub series: VecDeque<(f64, f64, usize)>,
}

impl Default for Coverage {
    fn default() -> Self {
        Coverage {
            sectors: vec![vec![0.0; 36]; 6],
            histogram: vec![0; 50],
            positions: 0,
            max_range_nm: 0.0,
            max_alt_ft: None,
            series: VecDeque::new(),
        }
    }
}

/// The altitude band of an altitude (None for unknown).
pub fn band(alt_ft: Option<i32>, on_ground: bool) -> Option<usize> {
    if on_ground {
        return Some(0);
    }
    let a = alt_ft?;
    Some(if a < 10_000 {
        0
    } else if a < 20_000 {
        1
    } else if a < 30_000 {
        2
    } else if a < 40_000 {
        3
    } else {
        4
    })
}

impl Coverage {
    /// Counts a position at a range and bearing.
    pub fn add(&mut self, range_nm: f64, bearing: f64, alt_ft: Option<i32>, on_ground: bool) {
        let s = ((bearing.rem_euclid(360.0)) / 10.0).floor() as usize % 36;
        if range_nm > self.sectors[0][s] {
            self.sectors[0][s] = range_nm;
        }
        if let Some(b) = band(alt_ft, on_ground) {
            if range_nm > self.sectors[1 + b][s] {
                self.sectors[1 + b][s] = range_nm;
            }
        }
        let h = ((range_nm / 10.0).floor() as usize).min(49);
        self.histogram[h] += 1;
        self.positions += 1;
        self.max_range_nm = self.max_range_nm.max(range_nm);
    }
}

impl Shift for Coverage {
    fn shift_times(&mut self, by_s: f64) {
        for s in &mut self.series {
            s.0 += by_s;
        }
    }
}

/// A message in the message list. Its text is made when the list is published.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MessageLine {
    /// The bytes of the frame (the first `bits / 8` are used).
    pub bytes: [u8; 14],
    /// 56 or 112.
    pub bits: u8,
    /// The key: the 24-bit address with the [`NON_ICAO`] bit.
    pub key: u32,
    /// The altitude that the Comm-B registers were read with.
    pub commb_alt_ft: Option<i32>,
    /// Bits repaired.
    pub repaired: u8,
    /// Signal time.
    pub time: f64,
    /// Level.
    pub level_dbfs: f32,
}

impl MessageLine {
    /// The frame.
    pub fn frame(&self) -> Frame {
        Frame {
            bytes: self.bytes,
            bits: self.bits,
        }
    }
    /// The one-line description of the message.
    pub fn text(&self) -> String {
        let f = self.frame();
        let h = Header {
            key: self.key,
            ..header(&f, f.remainder())
        };
        let e = matches!(f.df(), 17..=19).then(|| es(&f)).flatten();
        let b = matches!(f.df(), 20 | 21)
            .then(|| commb::infer(f.me(), self.commb_alt_ft))
            .flatten();
        describe(&f, &h, e.as_ref(), b.as_ref())
    }
}

impl Shift for MessageLine {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// An aircraft event: an aircraft heard for the first time, or a later state, at most one every 2 s.
#[derive(Clone, Debug, PartialEq)]
pub struct AdsbEvent {
    /// New or update.
    pub change: AircraftChange,
    /// The 24-bit key with the [`NON_ICAO`] bit.
    pub key: u32,
    /// The callsign.
    pub callsign: Option<String>,
    /// The registration.
    pub registration: Option<String>,
    /// The emitter category: set letter and code.
    pub category: Option<(char, u8)>,
    /// Messages heard.
    pub messages: u64,
    /// The emergency state.
    pub emergency: u8,
    /// On the ground.
    pub on_ground: Option<bool>,
    /// The squawk.
    pub squawk: Option<u16>,
    /// Barometric altitude, feet.
    pub altitude_ft: Option<i32>,
    /// Speed, knots.
    pub speed_kt: Option<f64>,
    /// Track or heading, degrees.
    pub heading_deg: Option<f64>,
    /// Vertical rate, feet per minute.
    pub vertical_rate_fpm: Option<i32>,
    /// Latitude and longitude, degrees.
    pub position: Option<(f64, f64)>,
    /// Distance from the station, km.
    pub distance_km: Option<f64>,
}

impl AdsbEvent {
    /// The event of an aircraft heard for the first time, before its state is known.
    fn new_aircraft(key: u32) -> AdsbEvent {
        AdsbEvent {
            change: AircraftChange::New,
            key,
            callsign: None,
            registration: None,
            category: None,
            messages: 0,
            emergency: 0,
            on_ground: None,
            squawk: None,
            altitude_ft: None,
            speed_kt: None,
            heading_deg: None,
            vertical_rate_fpm: None,
            position: None,
            distance_km: None,
        }
    }
}

/// The reason for an emergency urgency: squawk 7500, 7600 or 7700, or an emergency state.
pub fn emergency_reason(squawk: Option<u16>, emergency: u8) -> Option<&'static str> {
    match squawk {
        Some(0o7500) => Some("squawk 7500: unlawful interference"),
        Some(0o7600) => Some("squawk 7600: radio failure"),
        Some(0o7700) => Some("squawk 7700: emergency"),
        _ => (emergency != 0).then(|| emergency_name(emergency)),
    }
}

impl Item for AdsbEvent {
    const KIND: &'static str = "adsb.aircraft";
    fn event_def() -> Option<&'static EventDef> {
        Some(AIRCRAFT_SEEN.def())
    }
    fn write_fields(&self, r: &mut Record) {
        let reason = emergency_reason(self.squawk, self.emergency);
        AircraftSeen {
            event: self.change,
            source: &hex_key_text(self.key),
            callsign: self.callsign.as_deref(),
            registration: self.registration.as_deref(),
            category: self
                .category
                .map(|(set, code)| category_text(set, code))
                .as_deref(),
            squawk: self.squawk.map(squawk_text).as_deref(),
            emergency: i64::from(self.emergency),
            on_ground: self.on_ground,
            altitude_ft: self.altitude_ft.map(i64::from),
            speed_kt: self.speed_kt,
            course_deg: self.heading_deg,
            vertical_rate_fpm: self.vertical_rate_fpm.map(i64::from),
            position: self.position,
            distance_km: self.distance_km,
            messages: self.messages as i64,
            urgency: if reason.is_some() {
                keys::Urgency::Emergency
            } else {
                keys::Urgency::None
            },
            text: reason,
            family: keys::Family::Aircraft,
        }
        .write(r);
    }
}

/// The text of a 24-bit key: six hex digits, with `~` in front for a non-ICAO address.
fn hex_key_text(k: u32) -> Text<8> {
    let mut t = Text::new();
    let tilde = if k & NON_ICAO != 0 { "~" } else { "" };
    // At most seven characters: they fit.
    let _ = write!(t, "{tilde}{:06X}", k & 0xFF_FFFF);
    t
}

/// The text of an emitter category: the set letter and the code ("A3").
fn category_text(set: char, code: u8) -> Text<8> {
    let mut t = Text::new();
    // A letter and a digit fit.
    let _ = write!(t, "{set}{}", code % 10);
    t
}

/// The table's settings.
#[derive(Clone, Debug)]
pub struct TableSettings {
    /// Expiry time: an aircraft is hidden after this silence, s.
    pub expiry_s: f64,
    /// Drop time (None: the expiry time; a very long time for "never"), s.
    pub drop_s: Option<f64>,
    /// The GPS interference layer is computed.
    pub gps_layer: bool,
}

impl Default for TableSettings {
    fn default() -> Self {
        TableSettings {
            expiry_s: 60.0,
            drop_s: None,
            gps_layer: false,
        }
    }
}

struct Effect {
    pos_now: Option<(f64, f64)>,
    bds: Option<Bds>,
    commb_alt_ft: Option<i32>,
    es: Option<Es>,
    header: Header,
}

#[derive(Clone, Debug, Default)]
struct GpsCell {
    seen: HashMap<u32, (f64, bool)>,
}

/// What the table hears and keeps across a restart: the aircraft, the range statistics and the message list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Held {
    /// The aircraft.
    pub aircraft: AircraftTable<ModeS>,
    /// Range statistics.
    pub coverage: Coverage,
    /// The last good messages of this receiver, oldest first.
    pub messages: LogRing<MessageLine>,
}

impl Default for Held {
    fn default() -> Self {
        Held {
            aircraft: AircraftTable::new(crate::checks::MAX_AIRCRAFT).with_trail(TrailSettings {
                points: TRAIL_POINTS,
                spacing: Spacing::seconds(5.0),
            }),
            coverage: Coverage::default(),
            messages: LogRing::new(MESSAGE_LIST),
        }
    }
}

impl Shift for Held {
    fn shift_times(&mut self, by_s: f64) {
        self.aircraft.shift_times(by_s);
        self.coverage.shift_times(by_s);
        self.messages.shift_times(by_s);
    }
}

impl Tables for Held {
    const FORMAT: StateFormat = StateFormat::new("adsb.aircraft", 2);
    fn clear(&mut self) {
        self.aircraft.clear();
        self.coverage = Coverage::default();
        self.messages.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.aircraft.adopt(saved.aircraft);
        self.coverage = saved.coverage;
        self.messages.adopt(saved.messages);
    }
}

/// The aircraft table.
pub struct Table {
    /// Settings.
    pub settings: TableSettings,
    /// The station's position, from the run context.
    pub station: Option<(f64, f64)>,
    /// What is heard and kept.
    pub held: Held,
    /// The newest time seen.
    pub now: f64,
    next_track: u16,
    last_new_log: f64,
    /// Log lines waiting for the results.
    pub log: Vec<String>,
    /// Events waiting.
    pub events: Vec<AdsbEvent>,
    /// Feed items waiting.
    pub feed: Vec<FeedItem>,
    /// The wall time of the messages being taken, seconds since 1970, when the input is live.
    pub wall: Option<f64>,
    /// The host's wall clock, from the run context.
    pub clock: Clock,
    gps: HashMap<(i32, i32), GpsCell>,
    last_pos_for_gps: HashMap<u32, (f64, f64, f64)>,
}

impl Default for Table {
    fn default() -> Self {
        Table {
            settings: TableSettings::default(),
            station: None,
            held: Held::default(),
            now: 0.0,
            next_track: 1,
            last_new_log: f64::MIN,
            log: Vec::new(),
            events: Vec::new(),
            feed: Vec::new(),
            wall: None,
            clock: Clock::fixed(0.0),
            gps: HashMap::new(),
            last_pos_for_gps: HashMap::new(),
        }
    }
}

/// The most aircraft a report lists.
pub const REPORT_AIRCRAFT: usize = 128;
/// The most messages in the message list.
pub const MESSAGE_LIST: usize = 64;
/// The most trail points per aircraft.
pub const TRAIL_POINTS: usize = 80;

fn hex_key(k: u32) -> String {
    hex_key_text(k).as_str().to_owned()
}

/// The one-line description of a message.
pub fn describe(f: &Frame, h: &Header, e: Option<&Es>, bds: Option<&Bds>) -> String {
    let df = f.df();
    let alt = |a: Option<i32>| a.map(|a| format!(", altitude {a} ft")).unwrap_or_default();
    match df {
        0 | 16 => format!("Air-air reply{}", alt(h.alt_ft)),
        4 => match h.alt_ft {
            Some(a) => format!("Altitude reply {a} ft"),
            None => "Altitude reply".into(),
        },
        5 => format!(
            "Identity reply {}",
            h.squawk.map(squawk_text).unwrap_or_default()
        ),
        11 => "All-call reply".into(),
        17..=19 => {
            let p = match df {
                18 => "TIS-B ",
                19 => "Military ",
                _ => "",
            };
            let body = match e {
                Some(Es::Identification { callsign, .. }) => format!("Identification {callsign}"),
                Some(Es::Surface { .. }) => "Surface position".into(),
                Some(Es::Airborne { gnss: true, .. }) => "Airborne position (GNSS height)".into(),
                Some(Es::Airborne { alt_ft, .. }) => match alt_ft {
                    Some(a) => format!("Airborne position {a} ft"),
                    None => "Airborne position".into(),
                },
                Some(Es::Velocity {
                    speed_kt,
                    speed_kind,
                    ..
                }) => {
                    let k = match speed_kind {
                        SpeedKind::Ground => "ground",
                        SpeedKind::Ias => "IAS",
                        SpeedKind::Tas => "TAS",
                    };
                    match speed_kt {
                        Some(s) => format!("Velocity {s:.0} kt {k}"),
                        None => format!("Velocity {k}"),
                    }
                }
                Some(Es::NoPosition { alt_ft: Some(a) }) => format!("Altitude {a} ft, no position"),
                Some(Es::NoPosition { alt_ft: None }) => "No position information".into(),
                Some(Es::Status { emergency, .. }) if *emergency != 0 => {
                    format!("Status: {}", emergency_name(*emergency).to_lowercase())
                }
                Some(Es::Status { .. }) => "Aircraft status".into(),
                Some(Es::TargetState { .. }) => "Target state".into(),
                Some(Es::OpStatus { .. }) => "Operational status".into(),
                Some(Es::Other { tc }) => format!("Extended squitter, type {tc}"),
                None => format!("Extended squitter, type {}", f.field(33, 37)),
            };
            format!("{p}{body}")
        }
        20 | 21 => {
            let mut s = if df == 20 {
                match h.alt_ft {
                    Some(a) => format!("Comm-B altitude {a} ft"),
                    None => "Comm-B altitude".into(),
                }
            } else {
                format!(
                    "Comm-B identity {}",
                    h.squawk.map(squawk_text).unwrap_or_default()
                )
            };
            if let Some(b) = bds {
                s.push_str(&format!(", BDS {} {}", b.id(), commb::describe(b)));
            }
            s
        }
        24 => "Extended length message".into(),
        _ => format!("DF{df}"),
    }
}

impl Table {
    fn keep_s(&self) -> f64 {
        self.settings.drop_s.unwrap_or(self.settings.expiry_s)
    }

    /// Empties the table, the message list and the statistics (keeps the settings and the station).
    pub fn clear(&mut self) {
        self.held.clear();
        self.gps.clear();
        self.last_pos_for_gps.clear();
    }

    fn get_or_create(
        &mut self,
        key: u32,
        time: f64,
        create: bool,
        level: Option<f32>,
        local: bool,
    ) -> Option<&mut Aircraft> {
        let (address, non_icao) = (key & 0xFF_FFFF, key & NON_ICAO != 0);
        let existed = self.held.aircraft.get_address(address, non_icao).is_some();
        if !existed {
            if !create {
                return None;
            }
            if local {
                if time - self.last_new_log >= 2.0 {
                    self.last_new_log = time;
                    self.log.push(format!(
                        "new aircraft {}, {:.0} dBFS",
                        hex_key(key),
                        level.unwrap_or(-120.0)
                    ));
                }
                self.events.push(AdsbEvent::new_aircraft(key));
            }
        }
        let track_no = self.next_track;
        let at = match self.wall {
            Some(w) => Stamp::at(time).with_wall(w),
            None => Stamp::at(time),
        };
        let row = self.held.aircraft.heard_address(address, non_icao, at)?;
        if !existed {
            row.extra.track_no = track_no;
            row.extra.last_event = f64::MIN;
            self.next_track = self.next_track.wrapping_add(1).max(1);
        }
        Some(row)
    }

    /// Takes one accepted frame. Returns the description (for the message list) when it changed the table.
    pub fn ingest(&mut self, a: &Accepted) -> bool {
        let time = if a.source == 0 { a.time } else { self.now };
        self.now = self.now.max(time);
        let f = &a.frame;
        let h0 = header(f, f.remainder());
        let h = Header { key: a.key, ..h0 };
        let local = a.source == 0;
        let rx = self.station;
        let before = self
            .held
            .aircraft
            .get_address(a.key & 0xFF_FFFF, a.key & NON_ICAO != 0);
        let existed = before.is_some();
        let squawk_before = before.and_then(|x| x.extra.squawk);
        let Some(ac) = self.get_or_create(a.key, time, a.trusted, a.level_dbfs, local) else {
            return false;
        };
        ac.extra.source = a.source;
        if let Some(l) = a.level_dbfs {
            let lin = 10f32.powf(l / 10.0);
            ac.extra.level_lin = Some(ac.extra.level_lin.map_or(lin, |m| m + 0.2 * (lin - m)));
        }
        let df = f.df();
        if df == 18 {
            ac.extra.tisb = true;
        }
        let mut ground_set = false;
        let mut alert_from_status = false;
        // Header fields.
        match df {
            0 | 16 => {
                ac.extra.on_ground = Some(h.code == 1);
                ground_set = true;
                if let Some(alt) = h.alt_ft {
                    ac.extra.baro_alt = Some((alt, time));
                }
            }
            4 | 5 | 20 | 21 => {
                let (g, alert, spi) = flight_status(h.code);
                if let Some(g) = g {
                    ac.extra.on_ground = Some(g);
                }
                ground_set = g.is_some() || matches!(h.code, 4 | 5);
                ac.extra.alert = alert;
                ac.extra.spi = spi;
                alert_from_status = true;
                if let Some(alt) = h.alt_ft {
                    ac.extra.baro_alt = Some((alt, time));
                }
                if let Some(sq) = h.squawk {
                    match ac.extra.squawk {
                        None => ac.extra.squawk = Some(sq),
                        Some(cur) if cur == sq => ac.extra.pending_squawk = None,
                        Some(_) => match ac.extra.pending_squawk {
                            Some((p, t)) if p == sq && time - t <= 60.0 => {
                                ac.extra.squawk = Some(sq);
                                ac.extra.pending_squawk = None;
                            }
                            _ => ac.extra.pending_squawk = Some((sq, time)),
                        },
                    }
                }
            }
            _ => {}
        }
        let _ = alert_from_status;
        let mut eff = Effect {
            pos_now: None,
            bds: None,
            commb_alt_ft: None,
            es: None,
            header: h.clone(),
        };
        // Extended squitter.
        if let Some(e) = es(f) {
            match &e {
                Es::Identification {
                    set,
                    code,
                    callsign,
                } => {
                    ac.extra.category = (*code != 0).then_some((*set, *code));
                    if !callsign.is_empty() {
                        ac.set_callsign(callsign);
                    }
                }
                Es::Surface {
                    speed_kt,
                    track_deg,
                    cpr,
                    ..
                } => {
                    ac.extra.on_ground = Some(true);
                    ground_set = true;
                    if let Some(s) = speed_kt {
                        ac.set_speed(*s, SpeedKind::Ground);
                    }
                    if let Some(t) = track_deg {
                        ac.set_heading(*t, true);
                    }
                    eff.pos_now = surface_position(ac, *cpr, time, rx);
                }
                Es::Airborne {
                    ss,
                    alt_ft,
                    gnss,
                    nic,
                    cpr,
                    ..
                } => {
                    ac.extra.on_ground = Some(false);
                    ground_set = true;
                    ac.extra.alert = *ss == 1 || *ss == 2;
                    ac.extra.spi = *ss == 3;
                    if *gnss {
                        ac.extra.gnss_alt = *alt_ft;
                    } else if let Some(alt) = alt_ft {
                        ac.extra.baro_alt = Some((*alt, time));
                        ac.extra.trusted_alt = Some((*alt, time));
                    }
                    if nic.is_some() {
                        ac.extra.nic = *nic;
                    }
                    eff.pos_now = airborne_position(ac, *cpr, time, rx);
                }
                Es::NoPosition { alt_ft: Some(alt) } => {
                    ac.extra.baro_alt = Some((*alt, time));
                    ac.extra.trusted_alt = Some((*alt, time));
                }
                Es::Velocity {
                    speed_kt,
                    speed_kind,
                    heading_deg,
                    is_track,
                    vrate_fpm,
                    gnss_diff_ft,
                    ..
                } => {
                    if let Some(s) = speed_kt {
                        ac.set_speed(*s, *speed_kind);
                    }
                    if let Some(hd) = heading_deg {
                        ac.set_heading(*hd, *is_track);
                    }
                    if vrate_fpm.is_some() {
                        ac.vs_fpm = vrate_fpm.map(f64::from);
                    }
                    if gnss_diff_ft.is_some() {
                        ac.extra.gnss_diff = *gnss_diff_ft;
                    }
                    ac.extra.adsb_velocity_time = time;
                }
                Es::Status { emergency, squawk } => {
                    ac.extra.tc28_emergency = *emergency;
                    if let Some(s) = squawk {
                        ac.extra.squawk = Some(*s);
                        ac.extra.pending_squawk = None;
                    }
                }
                Es::TargetState {
                    sel_alt_ft,
                    baro_hpa,
                    sel_heading_deg,
                    nacp,
                    ..
                } => {
                    if sel_alt_ft.is_some() {
                        ac.extra.sel_alt = *sel_alt_ft;
                    }
                    if baro_hpa.is_some() {
                        ac.extra.baro_setting = *baro_hpa;
                    }
                    if sel_heading_deg.is_some() {
                        ac.extra.sel_heading = *sel_heading_deg;
                    }
                    // NACp is taken from the operational status only.
                    let _ = nacp;
                }
                Es::OpStatus {
                    version, nacp, sil, ..
                } => {
                    ac.extra.version = Some(*version);
                    if nacp.is_some() {
                        ac.extra.nacp = *nacp;
                    }
                    if sil.is_some() {
                        ac.extra.sil = *sil;
                    }
                }
                _ => {}
            }
            eff.es = Some(e);
        }
        if !ground_set && matches!(df, 11 | 17) {
            match h.code {
                4 => ac.extra.on_ground = Some(true),
                5 => ac.extra.on_ground = Some(false),
                _ => {}
            }
        }
        // Comm-B.
        if df == 20 || df == 21 {
            let alt = if df == 20 {
                h.alt_ft
            } else {
                ac.extra
                    .baro_alt
                    .filter(|(_, t)| time - t <= 30.0)
                    .map(|(a, _)| a)
            };
            eff.commb_alt_ft = alt;
            if let Some(b) = commb::infer(f.me(), alt) {
                ac.extra.datalink = true;
                apply_bds(ac, &b, time, rx);
                eff.bds = Some(b);
            }
        }
        // GPS interference bookkeeping.
        let key = a.key;
        let (pos_now, alt_now, ground_now) = (
            eff.pos_now,
            ac.extra.baro_alt.map(|x| x.0),
            ac.extra.on_ground == Some(true),
        );
        let (nacp, nic, airborne) = (
            ac.extra.nacp,
            ac.extra.nic,
            ac.extra.on_ground != Some(true),
        );
        let squawk_after = ac.extra.squawk;
        let snapshot_pos = ac.pos();
        // Range statistics.
        if let (Some((lat, lon)), Some(p)) = (pos_now, snapshot_pos) {
            if let Some(ma) = alt_now {
                self.held.coverage.max_alt_ft =
                    Some(self.held.coverage.max_alt_ft.map_or(ma, |m| m.max(ma)));
            }
            if let (Some((rl, ro)), true) = (rx, p.kind.confirmed()) {
                let d = distance_nm(rl, ro, lat, lon);
                let b = bearing_deg(rl, ro, lat, lon);
                self.held.coverage.add(d, b, alt_now, ground_now);
            }
        }
        // Only global or confirmed positions count: a first, unconfirmed local decode that the next global fix corrects is not a jump.
        let confirmed_now = snapshot_pos.is_some_and(|p| p.kind.confirmed());
        if self.settings.gps_layer && airborne && confirmed_now {
            if let Some((lat, lon)) = pos_now {
                let jump = self
                    .last_pos_for_gps
                    .get(&key)
                    .is_some_and(|&(pl, po, pt)| {
                        let dt = time - pt;
                        dt > 0.0
                            && dt <= 300.0
                            && distance_nm(pl, po, lat, lon) / (dt / 3600.0) > 2000.0
                    });
                self.last_pos_for_gps.insert(key, (lat, lon, time));
                let reports = nacp.is_some() || nic.is_some();
                let degraded = nacp.is_some_and(|n| n < 7) || nic.is_some_and(|n| n < 6) || jump;
                if reports || jump {
                    let cell = ((lat / 0.5).floor() as i32, (lon / 0.5).floor() as i32);
                    let c = self.gps.entry(cell).or_default();
                    let e = c.seen.entry(key).or_insert((time, false));
                    e.0 = time;
                    e.1 |= degraded;
                }
            }
        }
        // Events (this receiver only).
        if local {
            let seen = self
                .held
                .aircraft
                .rows()
                .seen(&row_key(key))
                .copied()
                .unwrap_or_default();
            let Some(ac) = self
                .held
                .aircraft
                .get_address_mut(key & 0xFF_FFFF, key & NON_ICAO != 0)
            else {
                return true;
            };
            let changed_squawk = existed && squawk_before != squawk_after;
            if !existed || changed_squawk || time - ac.extra.last_event >= 2.0 {
                ac.extra.last_event = time;
                let e = event_of(ac, &seen, rx, AircraftChange::Update);
                if existed {
                    self.events.push(e);
                } else if let Some(last) = self.events.last_mut() {
                    // Replace the bare "new" event by a full one, and add the update.
                    *last = AdsbEvent {
                        change: AircraftChange::New,
                        ..e.clone()
                    };
                    self.events.push(e);
                }
            }
            // Message list and feeds.
            let level = a.level_dbfs.unwrap_or(-120.0);
            self.held.messages.push(MessageLine {
                bytes: f.bytes,
                bits: f.bits,
                key,
                commb_alt_ft: eff.commb_alt_ft,
                repaired: a.repaired,
                time,
                level_dbfs: level,
            });
            let Some(row) = self.held.aircraft.rows().row(&row_key(key)) else {
                return true;
            };
            let sbs = sbs_fields(f, &eff, row.record);
            let target = Some(self.target_report(row.record, row.seen, df, &eff, Some(level)));
            self.feed.push(FeedItem::Message {
                frame: *f,
                time,
                level_dbfs: level,
                sbs,
                target,
            });
        }
        true
    }

    /// Takes an aircraft state from another receiver (SBS, ASTERIX).
    pub fn ingest_state(&mut self, s: &RemoteState, source: u16) {
        let time = self.now;
        let rx = self.station;
        let Some(ac) = self.get_or_create(s.key, time, true, None, false) else {
            return;
        };
        ac.extra.source = source;
        if let Some(c) = &s.callsign {
            ac.set_callsign(c);
        }
        if let Some(a) = s.alt_ft {
            ac.extra.baro_alt = Some((a, time));
        }
        if let Some(v) = s.speed_kt {
            ac.set_speed(v, SpeedKind::Ground);
        }
        if let Some(t) = s.track_deg {
            ac.set_heading(t, true);
        }
        if s.vrate_fpm.is_some() {
            ac.vs_fpm = s.vrate_fpm.map(f64::from);
        }
        if s.squawk.is_some() {
            ac.extra.squawk = s.squawk;
        }
        if s.on_ground.is_some() {
            ac.extra.on_ground = s.on_ground;
        }
        if let Some((lat, lon)) = s.pos {
            place(ac, lat, lon, time, PosKind::Global);
            let (alt, g) = (
                ac.extra.baro_alt.map(|x| x.0),
                ac.extra.on_ground == Some(true),
            );
            if let Some((rl, ro)) = rx {
                self.held.coverage.add(
                    distance_nm(rl, ro, lat, lon),
                    bearing_deg(rl, ro, lat, lon),
                    alt,
                    g,
                );
            }
        }
    }

    /// The ASTERIX view of an aircraft.
    fn target_report(
        &self,
        ac: &Aircraft,
        seen: &Seen,
        df: u8,
        eff_or_none: &Effect,
        level: Option<f32>,
    ) -> TargetReport {
        let rx = self.station;
        let tod = self.clock.now().rem_euclid(86400.0);
        let mb = eff_or_none.bds.as_ref().map(|b| {
            let id = b.id();
            let (x, y) = (
                id.as_bytes()[0] - b'0',
                u8::from_str_radix(&id[2..], 16).unwrap_or(0),
            );
            (0u64, x, y)
        });
        target_of(ac, seen, rx, tod, df, mb, level)
    }

    /// Expires silent aircraft.
    pub fn expire(&mut self, now: f64) {
        let keep = self.keep_s();
        self.held.aircraft.expire(now, keep);
        for c in self.gps.values_mut() {
            c.seen.retain(|_, v| now - v.0 <= 3600.0);
        }
        self.gps.retain(|_, c| !c.seen.is_empty());
        self.last_pos_for_gps.retain(|_, v| now - v.2 <= 300.0);
    }

    /// The GPS interference cells: (lat, lon of the cell's south-west corner, aircraft, level 0 to 3).
    pub fn gps_cells(&self) -> Vec<(f64, f64, usize, u8)> {
        let mut v: Vec<(f64, f64, usize, u8)> = self
            .gps
            .iter()
            .filter(|(_, c)| c.seen.len() >= 3)
            .map(|(&(a, o), c)| {
                let n = c.seen.len();
                let d = c.seen.values().filter(|x| x.1).count() as f64 / n as f64;
                let level = if d < 0.1 {
                    0
                } else if d < 0.3 {
                    1
                } else if d < 0.6 {
                    2
                } else {
                    3
                };
                (a as f64 * 0.5, o as f64 * 0.5, n, level)
            })
            .collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        v
    }

    /// The aircraft, most recently heard first.
    pub fn recent(
        &self,
    ) -> impl Iterator<Item = sdrkit_util_objects::RowRef<'_, Key, Aircraft>> + '_ {
        self.held.aircraft.newest_first()
    }
}

/// Makes (`lat`, `lon`) the position of `ac` at signal time `time`, with the barometric altitude, and adds it to the trail. The
/// track decoded from messages is kept: a position never replaces it.
fn place(ac: &mut Aircraft, lat: f64, lon: f64, time: f64, kind: PosKind) {
    let fix = Fix {
        position: GeoPosition::new(lat, lon),
        alt_ft: ac.extra.baro_alt.map(|(a, _)| f64::from(a)),
        on_ground: false,
        source: Source::ModeS,
    };
    let track = ac.track_deg;
    if ac.position(time, fix, None).is_ok() {
        ac.extra.pos_kind = Some(kind);
    }
    if track.is_some() {
        ac.track_deg = track;
    }
}

fn set_position(
    ac: &mut Aircraft,
    lat: f64,
    lon: f64,
    time: f64,
    kind: PosKind,
) -> Option<(f64, f64)> {
    // Jump check.
    if let Some(p) = ac.pos() {
        if p.kind.confirmed() && time - p.time < 60.0 {
            let limit = 1800.0 * (time - p.time).max(0.0) / 3600.0 + 2.0;
            if distance_nm(p.lat, p.lon, lat, lon) > limit {
                ac.extra.jumps += 1;
                if ac.extra.jumps < 3 {
                    return None;
                }
            }
        }
    }
    ac.extra.jumps = 0;
    place(ac, lat, lon, time, kind);
    Some((lat, lon))
}

fn airborne_position(
    ac: &mut Aircraft,
    f: CprFrame,
    time: f64,
    rx: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let i = f.odd as usize;
    ac.extra.cpr[i] = Some((f.lat, f.lon, time));
    if ac.pos().is_none() {
        if let Some((rl, ro)) = rx {
            if let Some((la, lo)) = cpr_local(&f, false, rl, ro) {
                if distance_nm(rl, ro, la, lo) <= 180.0 {
                    return set_position(ac, la, lo, time, PosKind::LocalFromReceiver);
                }
            }
        }
    }
    if let Some((ol, oo, ot)) = ac.extra.cpr[1 - i] {
        if (time - ot).abs() <= 15.0 {
            let other = CprFrame {
                odd: !f.odd,
                lat: ol,
                lon: oo,
            };
            let (e, o) = if f.odd { (other, f) } else { (f, other) };
            if let Ok(p) = cpr::decode_global(&CprFormat::AIRBORNE, &e, &o, f.odd, None) {
                let (la, lo) = (p.lat_deg, p.lon_deg);
                return set_position(ac, la, lo, time, PosKind::Global);
            }
        }
    }
    if let Some(p) = ac.pos() {
        if time - p.time <= 30.0 {
            if let Some((la, lo)) = cpr_local(&f, false, p.lat, p.lon) {
                let kind = if p.kind.confirmed() {
                    PosKind::Local
                } else {
                    PosKind::LocalFromReceiver
                };
                return set_position(ac, la, lo, time, kind);
            }
        }
    }
    None
}

fn surface_position(
    ac: &mut Aircraft,
    f: CprFrame,
    time: f64,
    rx: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let i = f.odd as usize;
    ac.extra.cpr_surface[i] = Some((f.lat, f.lon, time));
    let (rl, ro) = match (ac.pos(), rx) {
        (Some(p), _) => (p.lat, p.lon),
        (None, Some(r)) => r,
        (None, None) => return None,
    };
    let kind = match ac.pos() {
        None => PosKind::LocalFromReceiver,
        Some(p) if !p.kind.confirmed() => PosKind::LocalFromReceiver,
        Some(_) => PosKind::Local,
    };
    if let Some((ol, oo, ot)) = ac.extra.cpr_surface[1 - i] {
        if (time - ot).abs() <= 25.0 {
            let other = CprFrame {
                odd: !f.odd,
                lat: ol,
                lon: oo,
            };
            let (e, o) = if f.odd { (other, f) } else { (f, other) };
            let near = GeoPosition::new(rl, ro);
            if let Ok(p) = cpr::decode_global(&CprFormat::SURFACE, &e, &o, f.odd, Some(&near)) {
                let (la, lo) = (p.lat_deg, p.lon_deg);
                return set_position(ac, la, lo, time, kind);
            }
        }
    }
    let (la, lo) = cpr_local(&f, true, rl, ro)?;
    set_position(ac, la, lo, time, kind)
}

/// Decodes one CPR frame against a nearby known position, airborne or surface format.
fn cpr_local(f: &CprFrame, surface: bool, lat: f64, lon: f64) -> Option<(f64, f64)> {
    let fmt = if surface {
        CprFormat::SURFACE
    } else {
        CprFormat::AIRBORNE
    };
    let p = cpr::decode_local(&fmt, f, &GeoPosition::new(lat, lon)).ok()?;
    Some((p.lat_deg, p.lon_deg))
}

fn apply_bds(ac: &mut Aircraft, b: &Bds, time: f64, rx: Option<(f64, f64)>) {
    match b {
        Bds::B20 { callsign } => ac.set_callsign(callsign),
        Bds::B40 {
            mcp_ft,
            fms_ft,
            baro_hpa,
        } => {
            if let Some(a) = mcp_ft.or(*fms_ft) {
                ac.extra.sel_alt = Some(a);
            }
            if baro_hpa.is_some() {
                ac.extra.baro_setting = *baro_hpa;
            }
        }
        Bds::B50 {
            roll_deg,
            track_deg,
            gs_kt,
            track_rate_dps,
            tas_kt,
        } => {
            if roll_deg.is_some() {
                ac.extra.roll = *roll_deg;
            }
            if tas_kt.is_some() {
                ac.extra.tas = *tas_kt;
            }
            if time - ac.extra.adsb_velocity_time > 10.0 || ac.extra.adsb_velocity_time == 0.0 {
                if let Some(g) = gs_kt {
                    ac.set_speed(*g, SpeedKind::Ground);
                }
                if let Some(t) = track_deg {
                    ac.set_heading(*t, true);
                }
            }
            ac.extra.b50 = Some((
                *roll_deg,
                *track_deg,
                *gs_kt,
                *track_rate_dps,
                *tas_kt,
                time,
            ));
            derive_weather(ac, rx);
        }
        Bds::B60 {
            heading_deg,
            ias_kt,
            mach,
            baro_rate_fpm,
            ..
        } => {
            if ias_kt.is_some() {
                ac.extra.ias = *ias_kt;
            }
            if mach.is_some() {
                ac.extra.mach = *mach;
            }
            if ac.vs_fpm.is_none() {
                ac.vs_fpm = baro_rate_fpm.map(f64::from);
            }
            ac.extra.b60 = Some((*heading_deg, *mach, time));
            derive_weather(ac, rx);
        }
        Bds::B44 {
            wind,
            sat_c,
            pressure_hpa,
            turbulence,
            humidity_pct,
            ..
        } => {
            if let Some((s, d)) = wind {
                ac.extra.wind_reported = Some((*s, *d, time));
            }
            ac.extra.sat_reported = Some((*sat_c, time));
            if pressure_hpa.is_some() {
                ac.extra.pressure = *pressure_hpa;
            }
            if humidity_pct.is_some() {
                ac.extra.humidity = *humidity_pct;
            }
            if turbulence.is_some() {
                ac.extra.hazards[0] = *turbulence;
            }
        }
        Bds::B45 {
            hazards,
            sat_c,
            pressure_hpa,
            ..
        } => {
            for (d, s) in ac.extra.hazards.iter_mut().zip(hazards) {
                if s.is_some() {
                    *d = *s;
                }
            }
            ac.extra.sat_reported = Some((*sat_c, time));
            if pressure_hpa.is_some() {
                ac.extra.pressure = *pressure_hpa;
            }
        }
        Bds::B30 {
            ara,
            rac,
            terminated,
            threat,
            ..
        } => ac.extra.advisory = Some((*ara, *rac, *terminated, *threat, time)),
        Bds::B10 {
            subnetwork_version,
            acas_ra,
        } => ac.extra.capability = Some((*subnetwork_version, *acas_ra)),
        Bds::B17 { mask } => ac.extra.gicb = Some(*mask),
    }
}

fn derive_weather(ac: &mut Aircraft, rx: Option<(f64, f64)>) {
    let (Some(b5), Some(b6)) = (ac.extra.b50, ac.extra.b60) else {
        return;
    };
    let (roll, track, gs, rate, tas, t5) = b5;
    let (hdg, mach, t6) = b6;
    if (t5 - t6).abs() > 4.0 {
        return;
    }
    let now = t5.max(t6);
    if let (Some(tas), Some(m)) = (tas, mach) {
        if tas >= 150.0 && m >= 0.4 {
            let t = sat_from_tas_mach(tas, m).unwrap_or(f64::NAN);
            if (-80.0..=50.0).contains(&t) {
                ac.extra.sat_derived = Some(match ac.extra.sat_derived {
                    Some((old, ot)) if now - ot <= 60.0 => (old + 0.3 * (t - old), now),
                    _ => (t, now),
                });
            }
        }
    }
    if roll.is_some_and(|r| r.abs() > 5.0) || rate.is_some_and(|r| r.abs() > 0.5) {
        return;
    }
    let (Some(track), Some(gs), Some(tas), Some(hdg)) = (track, gs, tas, hdg) else {
        return;
    };
    if tas < 100.0 {
        return;
    }
    let at = ac.pos().map(|p| (p.lat, p.lon)).or(rx);
    let Some((la, lo)) = at else { return };
    let Ok(decl) = declination_deg(la, lo) else {
        return;
    };
    let Ok(w) = wind_triangle(track, gs, hdg + decl, tas) else {
        return;
    };
    let (from, speed) = (w.from_deg, w.speed_kt);
    if speed > 250.0 {
        return;
    }
    let (we, wn) = (
        -speed * from.to_radians().sin(),
        -speed * from.to_radians().cos(),
    );
    ac.extra.wind_derived = Some(match ac.extra.wind_derived {
        Some((oe, on, ot)) if now - ot <= 60.0 => (oe + 0.3 * (we - oe), on + 0.3 * (wn - on), now),
        _ => (we, wn, now),
    });
}

fn event_of(
    ac: &Aircraft,
    seen: &Seen,
    rx: Option<(f64, f64)>,
    change: AircraftChange,
) -> AdsbEvent {
    AdsbEvent {
        change,
        key: ac.key(),
        callsign: ac.callsign().map(str::to_string),
        registration: (!ac.registration.is_empty()).then(|| ac.registration.clone()),
        category: ac.extra.category,
        messages: seen.count,
        emergency: ac.extra.emergency(),
        on_ground: ac.extra.on_ground,
        squawk: ac.extra.squawk,
        altitude_ft: ac.extra.baro_alt.map(|x| x.0),
        speed_kt: ac.speed().map(|x| x.0),
        heading_deg: ac.heading().map(|x| x.0),
        vertical_rate_fpm: ac.vs_fpm.map(|v| v.round() as i32),
        position: ac.pos().map(|p| (p.lat, p.lon)),
        distance_km: match (rx, ac.pos()) {
            (Some((rl, ro)), Some(p)) => {
                Some(distance_nm(rl, ro, p.lat, p.lon) * (NAUTICAL_MILE_M / 1000.0))
            }
            _ => None,
        },
    }
}

fn target_of(
    ac: &Aircraft,
    seen: &Seen,
    rx: Option<(f64, f64)>,
    tod: f64,
    df: u8,
    mb: Option<(u64, u8, u8)>,
    level: Option<f32>,
) -> TargetReport {
    let pos = ac.pos().map(|p| (p.lat, p.lon));
    let polar = match (rx, pos) {
        (Some((rl, ro)), Some((la, lo))) => {
            Some((distance_nm(rl, ro, la, lo), bearing_deg(rl, ro, la, lo)))
        }
        _ => None,
    };
    let ground = match (ac.speed(), ac.heading()) {
        (Some((s, SpeedKind::Ground)), Some((t, true))) => Some((s, t)),
        _ => None,
    };
    TargetReport {
        key: ac.key(),
        time: seen.last_s,
        tod_s: tod,
        callsign: ac.callsign().and_then(|c| Callsign::try_from_str(c).ok()),
        squawk: ac.extra.squawk,
        alt_ft: ac.extra.baro_alt.map(|x| x.0),
        gnss_ft: ac.extra.gnss_alt,
        pos,
        polar,
        ground,
        heading_deg: ac.heading().filter(|h| !h.1).map(|h| h.0),
        ias_kt: ac.extra.ias,
        tas_kt: ac.extra.tas,
        mach: ac.extra.mach,
        vrate: ac.vs_fpm.map(|v| (v.round() as i32, true)),
        on_ground: ac.extra.on_ground == Some(true),
        category: ac.extra.category,
        sel_alt_ft: ac.extra.sel_alt,
        emergency: ac.extra.emergency(),
        spi: ac.extra.spi,
        version: ac.extra.version,
        nacp: ac.extra.nacp,
        sil: ac.extra.sil,
        nic: ac.extra.nic,
        track_no: ac.extra.track_no,
        df,
        mb,
        tisb: ac.extra.tisb,
        level_dbfs: level,
    }
}

fn sbs_fields(f: &Frame, eff: &Effect, ac: &Aircraft) -> Option<SbsFields> {
    let df = f.df();
    let tc = eff.header.tc;
    let t = match df {
        17..=19 => match tc? {
            1..=4 => 1,
            5..=8 => 2,
            9..=18 | 20..=22 => 3,
            19 => 4,
            _ => return None,
        },
        4 | 20 => 5,
        5 | 21 => 6,
        0 | 16 => 7,
        11 => 8,
        _ => return None,
    };
    let mut s = SbsFields {
        msg_type: t,
        key: ac.key(),
        on_ground: ac.extra.on_ground == Some(true),
        ..Default::default()
    };
    match (t, &eff.es) {
        (1, Some(Es::Identification { callsign, .. })) => s.callsign = Some(*callsign),
        (
            2,
            Some(Es::Surface {
                speed_kt,
                track_deg,
                ..
            }),
        ) => {
            s.gs_kt = speed_kt.map(|v| v.round());
            s.track_deg = track_deg.map(|v| v.round());
            s.pos = eff.pos_now;
        }
        (3, Some(Es::Airborne { alt_ft, gnss, .. })) => {
            s.alt_ft = if *gnss { None } else { *alt_ft };
            s.pos = eff.pos_now;
        }
        (
            4,
            Some(Es::Velocity {
                speed_kt,
                speed_kind: SpeedKind::Ground,
                heading_deg,
                vrate_fpm,
                ..
            }),
        ) => {
            s.gs_kt = speed_kt.map(|v| v.round());
            s.track_deg = heading_deg.map(|v| v.round());
            s.vrate_fpm = *vrate_fpm;
        }
        (4, Some(Es::Velocity { vrate_fpm, .. })) => s.vrate_fpm = *vrate_fpm,
        _ => {}
    }
    if matches!(t, 5 | 7) {
        s.alt_ft = eff.header.alt_ft;
    }
    if t == 6 {
        s.squawk = eff.header.squawk;
    }
    if matches!(t, 5 | 6) {
        let (_, alert, spi) = flight_status(eff.header.code);
        let em = matches!(ac.extra.squawk, Some(0o7500 | 0o7600 | 0o7700));
        s.flags = Some((alert, em, spi));
    }
    Some(s)
}

// ---------------------------------------------------------------- the report

/// The names of the five hazards of BDS 4,4 and 4,5, in their order in [`ModeS::hazards`].
const HAZARD_NAMES: [&str; 5] = [
    "turbulence",
    "wind shear",
    "microburst",
    "icing",
    "wake vortex",
];

impl RowTexts {
    /// Makes the texts of `row` again where their source changed.
    fn refresh(row: &mut Aircraft) {
        let t = &mut row.extra.texts;
        let newest = row.trail.points().last().map_or(f64::MIN, |p| p.t);
        let trail_of = (row.trail.points().count(), newest);
        if t.trail_of != trail_of {
            t.trail_of = trail_of;
            sdrkit_proto_aero::trail::encode(row.trail.points(), &mut t.trail);
        }
        let a = &mut row.extra;
        let t = &mut a.texts;
        if t.hazards_of != a.hazards
            || t.hazards.is_empty() && a.hazards.iter().any(Option::is_some)
        {
            t.hazards_of = a.hazards;
            t.hazards.clear();
            for (h, n) in a.hazards.iter().zip(HAZARD_NAMES) {
                if let Some(l) = h {
                    if !t.hazards.is_empty() {
                        t.hazards.push_str(", ");
                    }
                    let _ = write!(t.hazards, "{n} {l}");
                }
            }
        }
        let adv = a.advisory.map(|x| (x.0, x.1, x.2));
        if t.advisory_of != adv {
            t.advisory_of = adv;
            t.advisory.clear();
            if let Some((ara, rac, terminated)) = adv {
                t.advisory
                    .push_str(&commb::advisory_text(ara, rac, terminated));
            }
        }
        if t.gicb_of != a.gicb {
            t.gicb_of = a.gicb;
            t.gicb.clear();
            if let Some(g) = a.gicb {
                for r in commb::gicb_registers(g) {
                    if !t.gicb.is_empty() {
                        t.gicb.push(' ');
                    }
                    t.gicb.push_str(r);
                }
            }
        }
    }
}

/// `t`, or `None` when it is empty.
fn text(t: &str) -> Option<&str> {
    (!t.is_empty()).then_some(t)
}

/// The wall time of signal time `t` of a row last heard as `seen` says, rounded down to the second.
fn wall_of(seen: Option<&Seen>, t: f64) -> Option<f64> {
    let s = seen?;
    s.last_wall.map(|w| (w - (s.last_s - t).max(0.0)).floor())
}

/// The ADS-B layout: the cells of [`AIRCRAFT`] from the shared row and its Mode S data. Distance and bearing are those of the
/// last report.
impl ModeS {
    fn row_write(
        row: &Aircraft,
        seen: Option<&Seen>,
        now: f64,
        out: RowOut<'_, '_, AircraftRow<'static>>,
    ) {
        let a = &row.extra;
        let fresh = |t: f64, max: f64| now - t <= max;
        let sat_rep = a.sat_reported.filter(|x| fresh(x.1, 300.0));
        let sat_der = a
            .sat_derived
            .filter(|x| fresh(x.1, 300.0) && a.sat_reported.is_none_or(|r| now - r.1 > 60.0));
        let adv = a.advisory.filter(|x| fresh(x.4, 60.0));
        let wind_d = a
            .wind_derived
            .filter(|x| fresh(x.2, 300.0))
            .map(|(we, wn, _)| {
                let from = (-we).atan2(-wn).to_degrees().rem_euclid(360.0);
                (from, we.hypot(wn))
            });
        let wind_r = a
            .wind_reported
            .filter(|x| fresh(x.2, 300.0))
            .map(|(s, d, _)| (d, s));
        let fix = match row.pos() {
            Some(p) => Some(match p.kind {
                PosKind::Global => FixChoice::Global,
                PosKind::Local => FixChoice::Local,
                PosKind::LocalFromReceiver => FixChoice::LocalFromReceiver,
            }),
            None => a.cpr_held(now).map(|h| match h {
                CprHeld::Pair => FixChoice::CprPair,
                CprHeld::Even => FixChoice::CprEven,
                CprHeld::Odd => FixChoice::CprOdd,
            }),
        };
        out.put(&AircraftRow {
            icao: &hex_key_text(row.key()),
            callsign: text(&row.flight),
            registration: text(&row.registration),
            country: row.country.and_then(text),
            category: a.category.map(|(s, c)| category_text(s, c)).as_deref(),
            squawk: a.squawk.map(squawk_text).as_deref(),
            emergency: i64::from(a.emergency()),
            emergency_name: (a.emergency() != 0).then(|| emergency_name(a.emergency())),
            on_ground: a.on_ground,
            alert: a.alert,
            spi: a.spi,
            position: row.pos().map(|p| (p.lat, p.lon)),
            fix,
            position_time: row.pos().and_then(|p| wall_of(seen, p.time)).map(Time),
            alt_ft: a.baro_alt.map(|x| i64::from(x.0)),
            gnss_alt_ft: a.gnss_alt.map(i64::from),
            speed_kt: row.speed().map(|x| x.0),
            speed_kind: row.speed().map(|x| x.1),
            heading_deg: row.heading().map(|x| x.0),
            heading_is_track: row.heading().map(|x| x.1),
            vertical_rate_fpm: row.vs_fpm.map(|v| v.round() as i64),
            gnss_minus_baro_ft: a.gnss_diff.map(i64::from),
            selected_alt_ft: a.sel_alt.map(i64::from),
            selected_heading_deg: a.sel_heading,
            baro_setting_hpa: a.baro_setting,
            adsb_version: a.version.map(i64::from),
            nacp: a.nacp.map(i64::from),
            sil: a.sil.map(i64::from),
            nic: a.nic.map(i64::from),
            ias_kt: a.ias,
            mach: a.mach,
            roll_deg: a.roll,
            wind_derived_from_deg: wind_d.map(|w| w.0),
            wind_derived_kt: wind_d.map(|w| w.1),
            wind_reported_from_deg: wind_r.map(|w| w.0),
            wind_reported_kt: wind_r.map(|w| w.1),
            sat_derived_c: sat_der.map(|x| x.0),
            sat_reported_c: sat_rep.map(|x| x.0),
            pressure_hpa: a.pressure,
            humidity_pct: a.humidity,
            hazards: text(&a.texts.hazards),
            advisory: adv.and(text(&a.texts.advisory)),
            advisory_threat: adv
                .and_then(|x| x.3)
                .map(|t| hex_key_text(t & 0xFF_FFFF))
                .as_deref(),
            advisory_time: adv.and_then(|x| wall_of(seen, x.4)).map(Time),
            subnetwork_version: a.capability.map(|c| i64::from(c.0)),
            acas_ra_capable: a.capability.map(|c| c.1),
            gicb: text(&a.texts.gicb),
            datalink: a.datalink,
            tisb: a.tisb,
            level_dbfs: a.level_dbfs().map(f64::from),
            distance_m: a.range.map(|r| r.0),
            bearing_deg: a.range.map(|r| r.1),
            source: &number_key(usize::from(a.source)),
            trail: a.texts.trail.as_slice().into(),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }

    fn row_point(row: &Aircraft) -> Option<Point<'_>> {
        let p = row.pos()?;
        let label = match row.callsign() {
            Some(c) => std::borrow::Cow::Borrowed(c),
            None => std::borrow::Cow::Owned(hex_key(row.key())),
        };
        let pt = Point::new(GeoPosition::new(p.lat, p.lon), label);
        Some(match row.heading() {
            Some(h) => pt.heading(h.0 as f32),
            None => pt,
        })
    }
}

impl Shaped for MessageLine {
    type Row = MessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let f = self.frame();

        let row = MessageRow {
            time_s: self.time,
            hex: &hex_text(f.data()),
            icao: &hex_key_text(self.key),
            df: i64::from(f.df()),
            fixed_bits: i64::from(self.repaired),
            level_dbfs: f64::from(self.level_dbfs),
            message: &self.text(),
        };
        out.put(&row);
    }
}

/// `data` (at most 14 bytes) as upper-case hex digits.
fn hex_text(data: &[u8]) -> Text<28> {
    let mut t = Text::new();
    for b in data.iter().take(14) {
        // Two digits a byte for at most 14 bytes fit.
        let _ = write!(t, "{b:02X}");
    }
    t
}

/// Resets the range statistics.
pub const RESET_STATS: Command =
    Command::new("reset_stats", "Reset statistics").help("Starts the range statistics again.");

sdrkit_core::settings! {
    /// The settings of [`Tracker`].
    pub enum TrackerSetting {
        /// Expiry time.
        ExpiryS = EXPIRY_S,
        /// Drop time.
        DropS = DROP_S,
        /// GPS interference layer.
        GpsLayer = GPS_LAYER,
    }
}

const AIRCRAFT_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of(AIRCRAFT.def().key);
const REMOVE: sdrkit_core::command::Command =
    sdrkit_util_objects::commands::remove_command(&[&AIRCRAFT_ROW]);

sdrkit_core::commands! {
    /// The commands of [`Tracker`].
    pub enum TrackerCommand {
        /// Forgets the aircraft, the messages and the statistics.
        Clear = sdrkit_util_objects::commands::CLEAR,
        /// Starts the range statistics again.
        ResetStats = RESET_STATS,
        /// Forgets one aircraft.
        Remove = REMOVE,
    }
}

/// The tracker component: the table, the reports and the results.
pub struct Tracker {
    /// The table.
    pub table: Table,
    published: Published,
    series: [sdrkit_core::SeriesId; 3],
    drop_s: f64,
    live: bool,
    report: u64,
    windows: [u64; 4],
    win_i: usize,
    good_at_last: u64,
    state: u8,
    last_good: f64,
    level_mean: Option<f32>,
    snr_mean: Option<f32>,
    stats: CheckStats,
    sources: Vec<SourceStatus>,
    last_series: f64,
    summary: String,
    points: Vec<[f32; 2]>,
    tracks: Vec<TargetReport>,
}

impl Default for Tracker {
    fn default() -> Self {
        let decl = Arc::new(typed::tracker_declaration());
        let id = |k| decl.series_id(k).expect("the tracker declares its series");
        let series = [
            id(series::COVERAGE_RANGE.key),
            id(series::COVERAGE_HISTOGRAM.key),
            id(series::COVERAGE_RATE.key),
        ];
        let mut published = Published::from_decl(decl);
        published
            .results_mut()
            .set_status(&Status::new(LockState::Searching));
        Tracker {
            table: Table::default(),
            published,
            series,
            drop_s: DROP_S.default,
            live: false,
            report: 0,
            windows: [0; 4],
            win_i: 0,
            good_at_last: 0,
            state: 0,
            last_good: f64::MIN,
            level_mean: None,
            snr_mean: None,
            stats: CheckStats::default(),
            sources: Vec::new(),
            last_series: f64::MIN,
            summary: String::new(),
            points: Vec::new(),
            tracks: Vec::new(),
        }
    }
}

impl Tracker {
    /// The report number.
    pub fn report_number(&self) -> u64 {
        self.report
    }
    /// The state (0 searching, 1 pulses, 2 receiving).
    pub fn state(&self) -> u8 {
        self.state
    }
    /// Messages per second (the last four windows).
    pub fn messages_per_second(&self) -> f64 {
        self.windows.iter().sum::<u64>() as f64
    }

    /// Takes one message of the checks.
    pub fn take(&mut self, m: &Msg) {
        match m {
            Msg::Frame(a) => {
                if self.table.ingest(a) && a.source == 0 {
                    self.last_good = a.time;
                    if let Some(l) = a.level_dbfs {
                        self.level_mean = Some(self.level_mean.map_or(l, |m| m + 0.05 * (l - m)));
                    }
                    if let Some(s) = a.snr_db {
                        self.snr_mean = Some(self.snr_mean.map_or(s, |m| m + 0.05 * (s - m)));
                    }
                }
            }
            Msg::State { state, source } => self.table.ingest_state(state, *source),
            Msg::Sources(s) => self.sources.clone_from(s),
            Msg::Stats(s) => {
                self.stats.clone_from(s);
                self.report_at(s.demod.time);
            }
        }
    }

    /// Makes a report at signal time `now` (every 0.25 s).
    pub fn report_at(&mut self, now: f64) {
        self.table.now = self.table.now.max(now);
        let now = self.table.now;
        self.report += 1;
        let good = self.stats.good;
        let new = good.saturating_sub(self.good_at_last);
        self.good_at_last = good;
        self.windows[self.win_i] = new;
        self.win_i = (self.win_i + 1) % 4;
        self.table.expire(now);
        let old = self.state;
        self.state = if now - self.last_good <= 5.0 {
            2
        } else if self
            .stats
            .last_strong_failure
            .is_some_and(|t| now - t <= 10.0)
        {
            1
        } else {
            0
        };
        if old != 2 && self.state == 2 {
            self.table.log.push("ADS-B messages are coming in".into());
        } else if old == 2 && self.state != 2 {
            self.table.log.push("no ADS-B messages for 5 s".into());
        }
        if let Some(l) = self.stats.demod.log {
            self.table.log.push(l.to_string());
        }
        if now - self.last_series >= 10.0 {
            self.last_series = now;
            let count = self.table.held.aircraft.len();
            let s = &mut self.table.held.coverage.series;
            if s.len() == 2000 {
                s.pop_front();
            }
            s.push_back((now, self.windows.iter().sum::<u64>() as f64, count));
        }
        self.publish(now);
        // CAT062 tracks.
        let rx = self.table.station;
        let tod = self.table.clock.now().rem_euclid(86400.0);
        let mut tracks = std::mem::take(&mut self.tracks);
        tracks.clear();
        tracks.extend(
            self.table
                .recent()
                .take(REPORT_AIRCRAFT)
                .filter(|r| r.record.extra.source == 0)
                .map(|r| target_of(r.record, r.seen, rx, tod, 0, None, None)),
        );
        self.table.feed.push(FeedItem::Tracks(Arc::new(tracks)));
    }

    /// A station position near the aircraft heard when the station given is more than 500 NM from all of them.
    fn position_hint(&self) -> Option<(f64, f64)> {
        let (rl, ro) = self.table.station?;
        let pos: Vec<(f64, f64)> = self
            .table
            .recent()
            .filter_map(|r| r.record.pos().map(|p| (p.lat, p.lon)))
            .collect();
        if pos.len() < 3 {
            return None;
        }
        let nearest = pos
            .iter()
            .map(|p| distance_nm(rl, ro, p.0, p.1))
            .fold(f64::MAX, f64::min);
        if nearest <= 500.0 {
            return None;
        }
        let mut la: Vec<f64> = pos.iter().map(|p| p.0).collect();
        let mut lo: Vec<f64> = pos.iter().map(|p| p.1).collect();
        la.sort_by(f64::total_cmp);
        lo.sort_by(f64::total_cmp);
        Some((la[la.len() / 2], lo[lo.len() / 2]))
    }

    fn publish(&mut self, now: f64) {
        let rx = self.table.station;
        for a in self.table.held.aircraft.records_mut() {
            a.extra.range = match (rx, a.pos()) {
                (Some((rl, ro)), Some(p)) => Some((
                    distance_nm(rl, ro, p.lat, p.lon) * NAUTICAL_MILE_M,
                    bearing_deg(rl, ro, p.lat, p.lon),
                )),
                _ => None,
            };
            RowTexts::refresh(a);
        }
        let count = self.table.held.aircraft.len();
        let with_pos = self
            .table
            .recent()
            .filter(|r| r.record.fix.is_some())
            .count();
        let hint = self.position_hint();
        let mps = self.messages_per_second();
        let snr = if self.state == 0 {
            0.0
        } else {
            self.snr_mean.unwrap_or(0.0)
        };
        let noise = self.stats.demod.noise_dbfs;
        self.summary.clear();
        let _ = write!(
            self.summary,
            "{} {:.1} msg/s, {} aircraft ({} with position), SNR {:.1} dB, noise {:.1} dBFS, {} good, {} failed",
            ["searching", "pulses", "receiving"][self.state as usize],
            mps,
            count,
            with_pos,
            snr,
            noise,
            self.stats.good,
            self.stats.failed
        );
        let lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][self.state as usize];
        let status_snr = (self.state > 0).then_some(snr);
        let level = self.level_mean;
        let p = &mut self.published;
        {
            let summary = &self.summary;
            let r = p.results_mut();
            r.update_status(|s: &mut Status| {
                let changed = s.lock_state != lock_state
                    || s.snr_db != status_snr
                    || s.level_dbfs != level
                    || s.offset_hz.is_some()
                    || s.summary != *summary;
                if changed {
                    s.lock_state = lock_state;
                    s.snr_db = status_snr;
                    s.level_dbfs = level;
                    s.offset_hz = None;
                    s.summary.clone_from(summary);
                }
                changed
            });
            for l in self.table.log.drain(..) {
                r.log(l);
            }
        }
        let cov = &self.table.held.coverage;
        p.put(&TrackerFields {
            aircraft_count: count as i64,
            with_position: with_pos as i64,
            message_rate: Hertz(mps),
            level_dbfs: self.level_mean.map(f64::from),
            noise_dbfs: f64::from(noise),
            snr_db: (self.state > 0).then_some(f64::from(snr)),
            max_range_m: (cov.max_range_nm > 0.0).then_some(cov.max_range_nm * NAUTICAL_MILE_M),
            report: self.report as i64,
            time_s: now,
            frames_ok: self.stats.good as i64,
            frames_bad: self.stats.failed as i64,
            repaired: self.stats.repaired as i64,
            loud: self.stats.loud as i64,
            candidates: self.stats.demod.candidates as i64,
            effort: i64::from(self.stats.demod.effort),
            position_hint: hint,
            coverage_positions: cov.positions as i64,
            coverage_max_alt_ft: cov.max_alt_ft.map(i64::from),
        });
        sdrkit_util_objects::shaped::rows_by(
            self.table.held.aircraft.rows(),
            &AIRCRAFT,
            p,
            |_| true,
            |r, out| ModeS::row_write(r.record, Some(r.seen), now, out),
        );
        sdrkit_util_objects::shaped::row_points_by(
            self.table.held.aircraft.rows(),
            AIRCRAFT.def(),
            "",
            p.results_mut(),
            ModeS::row_point,
        );
        sdrkit_util_objects::shaped::log_rows(&self.table.held.messages, &MESSAGES, now, p);
        // This receiver first, then the incoming feeds: row keys are their numbers, the same numbers the aircraft's source
        // column holds.
        let mut counts = [0u64; 33];
        for r in self.table.recent() {
            if let Some(c) = counts.get_mut(usize::from(r.record.extra.source)) {
                *c += 1;
            }
        }
        let own = SourceStatus {
            connected: true,
            messages: self.stats.good,
            ..Default::default()
        };
        let n = self.sources.len() + 1;
        for (i, s) in std::iter::once(&own)
            .chain(&self.sources)
            .enumerate()
            .take(SOURCES.cap)
        {
            SOURCES.put(
                p,
                &number_key(i),
                &SourceRow {
                    name: if i == 0 { "this radio" } else { &s.name },
                    format: s.format,
                    connected: s.connected,
                    messages: s.messages as i64,
                    aircraft: counts.get(i).copied().unwrap_or(0) as i64,
                    error: s.error.map(|e| e.to_string()).as_deref(),
                },
            );
        }
        for i in n..SOURCES.cap {
            if !p.remove_row(&SOURCES, &number_key(i)) {
                break;
            }
        }
        typed::count_rows(p, &BY_DF, &self.stats.by_df);
        p.clear_rows(&GPS_CELLS);
        if self.table.settings.gps_layer {
            for (lat, lon, n, level) in self.table.gps_cells() {
                let mut key = String::new();
                let _ = write!(key, "{lat:.2},{lon:.2}");
                let row = GpsCellRow {
                    corner: (lat, lon),
                    aircraft: n as i64,
                    level: i64::from(level),
                };
                GPS_CELLS.put(p, &key, &row);
            }
        }
        let points = &mut self.points;
        let r = p.results_mut();
        points.clear();
        points.extend(
            cov.sectors[0]
                .iter()
                .enumerate()
                .map(|(i, nm)| [(i * 10) as f32 + 5.0, (*nm * NAUTICAL_MILE_M) as f32]),
        );
        put_series(r, self.series[0], true, points);
        points.clear();
        points.extend(cov.histogram.iter().enumerate().map(|(i, n)| {
            [
                ((i * 10) as f64 + 5.0) as f32 * NAUTICAL_MILE_M as f32,
                *n as f32,
            ]
        }));
        put_series(r, self.series[1], false, points);
        points.clear();
        points.extend(
            cov.series
                .iter()
                .map(|(t, mps, _)| [*t as f32, *mps as f32]),
        );
        put_series(r, self.series[2], true, points);
    }

    fn clear(&mut self) {
        self.table.clear();
        self.stats = CheckStats::default();
        self.good_at_last = 0;
        self.windows = [0; 4];
        self.level_mean = None;
        self.snr_mean = None;
        self.publish(self.table.now);
    }
}

/// The row key of a source: its number in decimal digits.
fn number_key(i: usize) -> Text<20> {
    let mut t = Text::new();
    // A `usize` has at most 20 digits.
    let _ = write!(t, "{i}");
    t
}

/// Makes series `id` of `r` hold `points`, drawn as a line or as bars. It writes only when something changed.
fn put_series(r: &mut Results, id: sdrkit_core::SeriesId, line: bool, points: &[[f32; 2]]) {
    r.update_series(id, |s: &mut Series| {
        if s.line == line && s.points == points {
            return false;
        }
        s.line = line;
        s.points.clear();
        s.points.extend_from_slice(points);
        true
    });
}

impl Component for Tracker {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "adsb.tracker",
            vec![Port::typed::<Msg>("in")],
            vec![
                Port::typed::<FeedItem>("feed").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<AdsbEvent>("events").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(TrackerSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<FeedItem>(), 0.0),
            Format::new(Kind::typed::<AdsbEvent>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.table.station = ctx.station.position.map(|p| (p.lat_deg, p.lon_deg));
        self.table.clock = ctx.clock.clone();
        self.live = ctx.live;
    }
    fn process(&mut self, io: &mut Io) {
        let items = io.items::<Msg>(0);
        if self.live && !items.is_empty() {
            self.table.wall = Some(self.table.clock.now());
        }
        for m in items {
            self.take(&m.value);
        }
        let now = self.table.now;
        let fo = io.items_out::<FeedItem>(0);
        for f in self.table.feed.drain(..) {
            fo.push(Timed {
                time: now,
                value: f,
            });
        }
        let eo = io.items_out::<AdsbEvent>(1);
        for e in self.table.events.drain(..) {
            eo.push(Timed {
                time: now,
                value: e,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = &mut self.table.settings;
        match TrackerSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TrackerSetting::ExpiryS => s.expiry_s = EXPIRY_S.read(id, v)?,
            TrackerSetting::DropS => {
                self.drop_s = DROP_S.read(id, v)?;
                s.drop_s = Some(crate::checks::keep_s(self.drop_s));
            }
            TrackerSetting::GpsLayer => s.gps_layer = GPS_LAYER.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let s = &self.table.settings;
        values_of([
            (TrackerSetting::ExpiryS, Value::F64(s.expiry_s)),
            (TrackerSetting::DropS, Value::F64(self.drop_s)),
            (TrackerSetting::GpsLayer, Value::Bool(s.gps_layer)),
        ])
    }
    fn commands(&self) -> &Commands {
        TrackerCommand::commands()
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TrackerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TrackerCommand::Clear => self.clear(),
            TrackerCommand::ResetStats => self.table.held.coverage = Coverage::default(),
            TrackerCommand::Remove => {
                let row = args.arg(&AIRCRAFT_ROW)?;
                let aircraft = &mut self.table.held.aircraft;
                let key = sdrkit_util_objects::rows::key_of_row(aircraft.rows(), row)
                    .ok_or_else(|| self.published.no_such_row(&AIRCRAFT, row))?;
                aircraft.remove(&key);
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.table.held.save_state(self.table.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.table.held.restore_state(state, self.table.now)
    }
}
