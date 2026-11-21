//! The AIS station table: one record per MMSI, at most 300, updated from every accepted message, with its track.

use super::display::AisStationRow;
use super::message::{nav_status_text, ship_type_text, AisMessage, StationClass};
use sdrkit_core::interface::Time;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::publish::{columns, Point};
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{ObjectTable, Seen, Shift, Spacing, Stamp, Trail};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;

/// The most stations kept.
pub const MAX_STATIONS: usize = 300;
/// The most track points per station.
pub const MAX_TRACK: usize = 100;
/// A position becomes a track point when it is this far from the last one, metres.
pub const TRACK_SPACING_M: f64 = 20.0;
/// The most sentences kept per station.
pub const MAX_SENTENCES: usize = 4;

/// An MMSI as a table key, written with its nine digits.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MmsiKey(pub u32);

impl MmsiKey {
    /// The nine digits, written into `buf`.
    pub fn digits(self, buf: &mut [u8; 9]) -> &str {
        let mut n = self.0;
        for b in buf.iter_mut().rev() {
            *b = b'0' + (n % 10) as u8;
            n /= 10;
        }
        std::str::from_utf8(buf).unwrap_or_default()
    }
}

impl fmt::Display for MmsiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:09}", self.0)
    }
}

/// One station.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Station {
    /// Its MMSI.
    pub mmsi: u32,
    /// The kind of station.
    pub class: StationClass,
    /// The type of its last message.
    pub last_type: u8,
    /// The channel letter of its last message.
    pub channel: char,
    /// Its name.
    pub name: String,
    /// Its call sign.
    pub callsign: String,
    /// Its IMO number.
    pub imo: Option<u32>,
    /// The ship and cargo type code.
    pub ship_type: Option<u8>,
    /// Where it is bound.
    pub destination: String,
    /// The estimated arrival: month, day, hour, minute.
    pub eta: Option<[u8; 4]>,
    /// The draught, m.
    pub draught_m: Option<f64>,
    /// The distances from the position's reference point to bow, stern, port and starboard, m (M.1371 Annex 8 Figure 42).
    pub dim: Option<[u16; 4]>,
    /// The latitude, degrees.
    pub lat: Option<f64>,
    /// The longitude, degrees.
    pub lon: Option<f64>,
    /// The position came from message 27 (1/10 minute).
    pub long_range_position: bool,
    /// The speed over the ground, knots.
    pub sog_kn: Option<f64>,
    /// The course over the ground, degrees.
    pub cog_deg: Option<f64>,
    /// The true heading, degrees.
    pub heading: Option<u16>,
    /// The rate of turn, degrees per minute.
    pub rot_deg_min: Option<f64>,
    /// The navigational status code.
    pub nav_status: Option<u8>,
    /// The altitude of a SAR aircraft, m.
    pub altitude_m: Option<u16>,
    /// The aid to navigation type code.
    pub aid_type: Option<u8>,
    /// A virtual aid to navigation.
    pub virtual_aid: Option<bool>,
    /// An aid off its position.
    pub off_position: Option<bool>,
    /// Sent message 27 ("new-type vessel").
    pub long_range: bool,
    /// Messages 27 heard.
    pub msg27_count: u64,
    /// The position latency flag of message 27.
    pub latency: Option<bool>,
    /// Where it has been.
    pub track: Trail,
    /// Its last sentences.
    pub sentences: VecDeque<String>,
    /// The signal time of its last position, seconds.
    pub position_time: f64,
    /// Left out of the table and the map by the view filter (set before each report).
    #[serde(skip)]
    pub hidden: bool,
    /// Its track is published (set before each report).
    #[serde(skip)]
    pub show_track: bool,
}

impl Station {
    /// A station with nothing known but its MMSI.
    pub fn new(mmsi: u32) -> Self {
        Station {
            mmsi,
            class: StationClass::Other,
            last_type: 0,
            channel: '-',
            name: String::new(),
            callsign: String::new(),
            imo: None,
            ship_type: None,
            destination: String::new(),
            eta: None,
            draught_m: None,
            dim: None,
            lat: None,
            lon: None,
            long_range_position: false,
            sog_kn: None,
            cog_deg: None,
            heading: None,
            rot_deg_min: None,
            nav_status: None,
            altitude_m: None,
            aid_type: None,
            virtual_aid: None,
            off_position: None,
            long_range: false,
            msg27_count: 0,
            latency: None,
            track: Trail::new(MAX_TRACK, Spacing::metres(TRACK_SPACING_M)),
            sentences: VecDeque::with_capacity(MAX_SENTENCES + 1),
            position_time: 0.0,
            hidden: false,
            show_track: false,
        }
    }

    /// The name, else the MMSI written into `buf`.
    pub fn label<'a>(&'a self, buf: &'a mut [u8; 9]) -> &'a str {
        if self.name.is_empty() {
            MmsiKey(self.mmsi).digits(buf)
        } else {
            &self.name
        }
    }

    /// The ship type in words, else the class.
    pub fn type_text(&self) -> &'static str {
        match self.ship_type {
            Some(t) if t > 0 => ship_type_text(t),
            _ => self.class.name(),
        }
    }

    /// The position, when known.
    pub fn position(&self) -> Option<Position> {
        Some(Position::new(self.lat?, self.lon?))
    }

    /// The heading drawn on the map: the true heading, else the course when it moves faster than half a knot.
    pub fn map_heading(&self) -> Option<f32> {
        self.heading
            .map(f32::from)
            .or(match (self.sog_kn, self.cog_deg) {
                (Some(v), Some(c)) if v > 0.5 => Some(c as f32),
                _ => None,
            })
    }

    /// The navigational status in words.
    pub fn status_text(&self) -> Option<&'static str> {
        self.nav_status.map(nav_status_text)
    }
}

impl Shift for Station {
    fn shift_times(&mut self, by_s: f64) {
        self.track.shift_times(by_s);
        self.position_time += by_s;
    }
}

/// The channel number shown for a channel letter.
pub fn channel_number(c: char) -> &'static str {
    match c {
        'A' => "1",
        'B' => "2",
        'C' => "75",
        'D' => "76",
        _ => "-",
    }
}

impl Station {
    /// The speed over the ground, when it is a speed (ITU-R M.1371-5 Annex 8 Table 50: 102.3 kn means "not available").
    pub fn speed_kn(&self) -> Option<f64> {
        self.sog_kn.filter(|v| *v < 102.3)
    }
}

impl Shaped for Station {
    type Row = AisStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let (mut name, mut digits) = ([0u8; 9], [0u8; 9]);
        out.put(&AisStationRow {
            name: self.label(&mut name),
            mmsi: MmsiKey(self.mmsi).digits(&mut digits),
            callsign: (!self.callsign.is_empty()).then_some(self.callsign.as_str()),
            r#type: self.type_text(),
            speed_kt: self.speed_kn(),
            course_deg: self.cog_deg,
            position: self.lat.zip(self.lon),
            long_range_position: self.long_range_position,
            channel: channel_number(self.channel),
            status: self.status_text(),
            destination: (!self.destination.is_empty()).then_some(self.destination.as_str()),
            last_heard: columns::last_heard(seen).map(Time),
            heard: columns::heard(seen),
        });
    }

    fn point(&self) -> Option<Point<'_>> {
        if self.hidden {
            return None;
        }
        let label: std::borrow::Cow<'_, str> = if self.name.is_empty() {
            MmsiKey(self.mmsi).to_string().into()
        } else {
            self.name.as_str().into()
        };
        let p = Point::new(self.position()?, label);
        Some(match self.map_heading() {
            Some(h) => p.heading(h),
            None => p,
        })
    }
}

/// The table of stations, by MMSI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationTable {
    /// The stations.
    pub stations: ObjectTable<MmsiKey, Station>,
}

impl Default for StationTable {
    fn default() -> Self {
        StationTable {
            stations: ObjectTable::new(MAX_STATIONS),
        }
    }
}

impl Shift for StationTable {
    fn shift_times(&mut self, by_s: f64) {
        self.stations.shift_times(by_s);
    }
}

impl StationTable {
    /// The station of `mmsi`.
    pub fn get(&self, mmsi: u32) -> Option<&Station> {
        self.stations.get(&MmsiKey(mmsi))
    }

    /// Updates from an accepted message heard at `at` (signal time `at.t`) on `channel`, with its `sentences`. Returns whether the
    /// station is new. A time that is not finite changes nothing.
    pub fn update(
        &mut self,
        m: &AisMessage,
        at: Stamp,
        channel: char,
        sentences: &[String],
    ) -> bool {
        let Ok(up) = self
            .stations
            .upsert(&MmsiKey(m.mmsi), at, || Station::new(m.mmsi))
        else {
            return false;
        };
        let new = up.new;
        let s = up.row;
        s.last_type = m.msg_type;
        s.channel = channel;
        if m.class != StationClass::Other {
            s.class = m.class;
        } else if new {
            s.class = StationClass::of(m.msg_type, m.mmsi);
        }
        if matches!(m.msg_type, 1 | 2 | 3 | 9 | 18 | 19 | 27) {
            s.sog_kn = m.sog_kn;
            s.cog_deg = m.cog_deg;
            if m.msg_type != 27 {
                s.heading = m.heading;
            }
        }
        if matches!(m.msg_type, 1..=3) {
            s.rot_deg_min = m.rot_deg_min;
            s.nav_status = m.nav_status;
        }
        if m.msg_type == 27 {
            s.nav_status = m.nav_status;
            s.long_range = true;
            s.msg27_count += 1;
            s.latency = m.latency;
        }
        if m.msg_type == 9 {
            s.altitude_m = m.altitude_m;
        }
        if let Some((lat, lon)) = m.position() {
            if !matches!(m.msg_type, 17 | 22 | 23) {
                s.lat = Some(lat);
                s.lon = Some(lon);
                s.long_range_position = m.msg_type == 27;
                s.position_time = at.t;
                // A position out of range is left out of the track.
                let _ = s.track.push(at.t, Position::new(lat, lon), ());
            }
        }
        let keep = |dst: &mut String, v: &Option<String>| {
            if let Some(v) = v {
                if !v.is_empty() {
                    dst.clone_from(v);
                }
            }
        };
        if matches!(m.msg_type, 5 | 19 | 21 | 24) {
            keep(&mut s.name, &m.name);
        }
        if matches!(m.msg_type, 5 | 24) {
            keep(&mut s.callsign, &m.callsign);
        }
        if let Some(i) = m.imo.filter(|i| *i > 0) {
            s.imo = Some(i);
        }
        if let Some(t) = m.ship_type {
            if t > 0 || s.ship_type.is_none() {
                s.ship_type = Some(t);
            }
        }
        if m.msg_type == 5 {
            s.destination = m.destination.clone().unwrap_or_default();
            s.eta = m.eta;
            s.draught_m = m.draught_m;
        }
        if let Some(d) = m.dim.filter(|d| d.iter().any(|&v| v > 0)) {
            s.dim = Some(d);
        }
        if m.msg_type == 21 {
            s.aid_type = m.aid_type;
            s.virtual_aid = m.virtual_aid;
            if m.second.is_some_and(|v| v < 60) {
                s.off_position = m.off_position;
            }
        }
        for l in sentences {
            s.sentences.push_back(l.clone());
            while s.sentences.len() > MAX_SENTENCES {
                s.sentences.pop_front();
            }
        }
        new
    }

    /// The number of stations, those with a position, and those that sent message 27.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut with_pos = 0;
        let mut lr = 0;
        for r in self.stations.iter() {
            with_pos += usize::from(r.record.lat.is_some());
            lr += usize::from(r.record.long_range);
        }
        (self.stations.len(), with_pos, lr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ais::message::decode;
    use sdrkit_util_geo::nmea::{self, aivdm};

    fn msg(line: &str) -> AisMessage {
        let mut a = aivdm::Assembler::new();
        let s = nmea::parse(line).unwrap();
        let v = aivdm::Vdm::from_sentence(&s).unwrap();
        let bits = a.push(&v).unwrap().unwrap().bits.to_vec();
        decode(&bits).unwrap()
    }

    #[test]
    fn a_station_keeps_its_track_and_the_table_its_cap() {
        let mut t = StationTable::default();
        let m = msg("!AIVDM,1,1,,A,15RTgt0PAso;90TKcjM8h6g208CQ,0*4A");
        assert!(t.update(&m, Stamp::at(1.0), 'A', &["a".into()]));
        assert!(!t.update(&m, Stamp::at(2.0), 'B', &["b".into()]));
        let s = t.get(371_798_000).unwrap();
        // The same place twice: one track point.
        assert_eq!((s.track.len(), s.channel, s.sentences.len()), (1, 'B', 2));
        assert_eq!(t.stations.seen(&MmsiKey(371_798_000)).unwrap().count, 2);
        assert_eq!(MmsiKey(3_669_702).to_string(), "003669702");
        assert_eq!(MmsiKey(3_669_702).digits(&mut [0; 9]), "003669702");
        for k in 0..MAX_STATIONS as u32 + 5 {
            let mut m2 = m.clone();
            m2.mmsi = 244_000_000 + k;
            t.update(&m2, Stamp::at(3.0 + f64::from(k)), 'A', &[]);
        }
        assert_eq!(t.stations.len(), MAX_STATIONS);
        assert!(
            t.get(371_798_000).is_none(),
            "the least recently heard leaves first"
        );
    }
}
