//! The AIS view filter, CPA alerts and geofence alerts.

use super::message::{type_group, StationClass};
use super::station::Station;
use sdrkit_util_geo::position::{
    closest_approach, polygon_contains, wrap180, Fence as Area, Track,
};
use sdrkit_util_geo::{Position, KNOT_M_S, NAUTICAL_MILE_M};
use sdrkit_util_objects::{Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A text setting that does not parse: the byte position of the part that fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadText {
    /// The byte position.
    pub at: usize,
}

/// Why a fence line is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FenceFault {
    /// The line has no name.
    NoName,
    /// A circle is not `lat,lon,radius_nm`.
    NotACircle,
    /// A latitude or longitude does not parse or is out of range.
    BadPosition,
    /// The radius is not above 0 and below 5000 nm.
    BadRadius,
    /// A polygon point is not `lat,lon`.
    BadPolygonPoint,
    /// A polygon has fewer than 3 points.
    FewCorners,
    /// The line is neither `name;lat,lon,radius_nm` nor `name;poly;lat,lon lat,lon ...`.
    NotAFence,
}

/// The view filter: classes shown, ship-type groups shown, moving only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    /// The classes shown, one bit per [`StationClass::bit`].
    pub classes: u32,
    /// The ship-type groups shown, one bit per [`type_group`].
    pub groups: u32,
    /// Only stations moving at `sog` knots or more.
    pub moving: bool,
    /// The least speed of a moving station, knots.
    pub sog: f64,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            classes: 63,
            groups: 1023,
            moving: false,
            sog: 0.5,
        }
    }
}

impl Filter {
    /// Parses "classes=63;groups=1023;moving=0;sog=0.5" (any of the parts, in any order, empty text is the default).
    ///
    /// # Errors
    ///
    /// [`BadText`] with the byte position of a part that is not `key=value`, has an unknown key or a bad number.
    pub fn parse(text: &str) -> Result<Filter, BadText> {
        let mut f = Filter::default();
        let mut at = 0;
        for raw in text.split(';') {
            let bad = BadText { at };
            at += raw.len() + 1;
            let part = raw.trim();
            if part.is_empty() {
                continue;
            }
            let (k, v) = part.split_once('=').ok_or(bad)?;
            let (k, v) = (k.trim(), v.trim());
            match k {
                "classes" => f.classes = v.parse().map_err(|_| bad)?,
                "groups" => f.groups = v.parse().map_err(|_| bad)?,
                "moving" => f.moving = v.parse::<u32>().map_err(|_| bad)? != 0,
                "sog" => {
                    f.sog = v
                        .parse::<f64>()
                        .ok()
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .ok_or(bad)?
                }
                _ => return Err(bad),
            }
        }
        Ok(f)
    }

    /// The text form.
    pub fn text(&self) -> String {
        format!(
            "classes={};groups={};moving={};sog={}",
            self.classes,
            self.groups,
            u8::from(self.moving),
            self.sog
        )
    }

    /// Whether any part is narrowed ("some stations are hidden").
    pub fn narrowed(&self) -> bool {
        self.classes & 63 != 63 || self.groups & 1023 != 1023 || self.moving
    }

    /// Whether a station is shown.
    pub fn shows(&self, s: &Station) -> bool {
        if self.classes & s.class.bit() == 0 {
            return false;
        }
        if matches!(
            s.class,
            StationClass::A | StationClass::B | StationClass::Other
        ) && self.groups & type_group(s.ship_type) == 0
        {
            return false;
        }
        if self.moving {
            match s.sog_kn {
                Some(v) if v >= self.sog && v < 102.3 => {}
                _ => return false,
            }
        }
        true
    }
}

/// The closest point of approach of a vessel to the own position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cpa {
    /// The distance now, m.
    pub distance_m: f64,
    /// The distance at the closest point, m.
    pub dcpa_m: f64,
    /// Seconds to the closest point (0 when they move apart or keep their distance).
    pub tcpa_s: f64,
    /// The vessel's course and speed are known.
    pub predictable: bool,
}

/// The track of a vessel: speed and course when both are known and the speed is not "not available" (102.3 knots).
fn track(p: Position, sog_kn: Option<f64>, cog_deg: Option<f64>) -> (Track, bool) {
    match (sog_kn, cog_deg) {
        (Some(s), Some(c)) if s < 102.3 => (
            Track {
                position: p,
                speed_m_s: s * KNOT_M_S,
                course_deg: c,
            },
            true,
        ),
        _ => (
            Track {
                position: p,
                speed_m_s: 0.0,
                course_deg: 0.0,
            },
            false,
        ),
    }
}

/// The CPA of a vessel at `p` against the own ship at `own` moving at `own_sog_kn` on `own_cog_deg`, by
/// `sdrkit_util_geo::position::closest_approach`.
pub fn cpa(
    own: Position,
    own_sog_kn: f64,
    own_cog_deg: f64,
    p: Position,
    sog_kn: Option<f64>,
    cog_deg: Option<f64>,
) -> Cpa {
    let (own, _) = track(own, Some(own_sog_kn), Some(own_cog_deg));
    let (other, predictable) = track(p, sog_kn, cog_deg);
    let a = closest_approach(&own, &other);
    Cpa {
        distance_m: own.position.distance_m(&p),
        dcpa_m: a.distance_m,
        tcpa_s: a.time_s,
        predictable,
    }
}

/// A named fence.
#[derive(Clone, Debug, PartialEq)]
pub struct Fence {
    /// Its name.
    pub name: String,
    /// The area.
    pub area: Area,
}

impl Fence {
    /// Whether a point is inside. A polygon can cross the 180° meridian: its corners are taken within half a turn of the point.
    pub fn contains(&self, p: &Position, scratch: &mut Vec<Position>) -> bool {
        match &self.area {
            Area::Polygon { corners } => {
                scratch.clear();
                scratch.extend(
                    corners.iter().map(|c| {
                        Position::new(c.lat_deg, p.lon_deg + wrap180(c.lon_deg - p.lon_deg))
                    }),
                );
                polygon_contains(scratch, p)
            }
            a => a.contains(p),
        }
    }

    /// The distance to the fence's centre (circle) or first corner (polygon), m.
    pub fn distance_m(&self, p: &Position) -> f64 {
        match &self.area {
            Area::Circle { centre, .. } => centre.distance_m(p),
            Area::Polygon { corners } => corners.first().map_or(0.0, |c| c.distance_m(p)),
        }
    }
}

/// The most fences.
pub const MAX_FENCES: usize = 32;

fn latlon(s: &str) -> Option<Position> {
    let (a, b) = s.split_once(',')?;
    let (a, b): (f64, f64) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (a.abs() <= 90.0 && b.abs() <= 180.0).then_some(Position::new(a, b))
}

/// Parses the fence lines: `name;lat,lon,radius_nm` or `name;poly;lat,lon lat,lon ...`, `#` starting a comment line. Returns the
/// fences (at most [`MAX_FENCES`]) and the bad lines (line number, byte position of the line, fault).
pub fn parse_fences(text: &str) -> (Vec<Fence>, Vec<(usize, usize, FenceFault)>) {
    let mut fences = Vec::new();
    let mut errors = Vec::new();
    let mut start = 0;
    for (i, line) in text.split('\n').enumerate() {
        let at = start;
        start += line.len() + 1;
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let n = i + 1;
        let parts: Vec<&str> = l.split(';').map(str::trim).collect();
        if parts[0].is_empty() {
            errors.push((n, at, FenceFault::NoName));
            continue;
        }
        if parts.len() == 2 {
            let f: Vec<&str> = parts[1].split(',').map(str::trim).collect();
            if f.len() != 3 {
                errors.push((n, at, FenceFault::NotACircle));
                continue;
            }
            let (Ok(a), Ok(b)) = (f[0].parse::<f64>(), f[1].parse::<f64>()) else {
                errors.push((n, at, FenceFault::BadPosition));
                continue;
            };
            if a.abs() > 90.0 || b.abs() > 180.0 {
                errors.push((n, at, FenceFault::BadPosition));
                continue;
            }
            match f[2].parse::<f64>() {
                Ok(r) if r > 0.0 && r < 5000.0 => fences.push(Fence {
                    name: parts[0].into(),
                    area: Area::Circle {
                        centre: Position::new(a, b),
                        radius_m: r * NAUTICAL_MILE_M,
                    },
                }),
                _ => errors.push((n, at, FenceFault::BadRadius)),
            }
        } else if parts.len() == 3 && parts[1].eq_ignore_ascii_case("poly") {
            let pts: Option<Vec<Position>> = parts[2].split_whitespace().map(latlon).collect();
            match pts {
                None => errors.push((n, at, FenceFault::BadPolygonPoint)),
                Some(p) if p.len() < 3 => errors.push((n, at, FenceFault::FewCorners)),
                Some(corners) => fences.push(Fence {
                    name: parts[0].into(),
                    area: Area::Polygon { corners },
                }),
            }
        } else {
            errors.push((n, at, FenceFault::NotAFence));
        }
    }
    fences.truncate(MAX_FENCES);
    (fences, errors)
}

sdrkit_core::choice_enum! {
    /// What an alert is about.
    #[derive(Default, Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum AlertKind {
        /// A vessel will pass close.
        #[default]
        Cpa = ("cpa", "Close approach"),
        /// A vessel entered a fence.
        Enter = ("enter", "Entered a fence"),
        /// A vessel left a fence.
        Exit = ("exit", "Left a fence"),
    }
}

impl AlertKind {
    /// `cpa`, `enter` or `exit`.
    pub fn name(self) -> &'static str {
        match self {
            AlertKind::Cpa => "cpa",
            AlertKind::Enter => "enter",
            AlertKind::Exit => "exit",
        }
    }
}

/// One alert.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    /// When.
    pub at: Stamp,
    /// What.
    pub kind: AlertKind,
    /// The fence, for an enter or exit.
    pub fence: String,
    /// The vessel.
    pub mmsi: u32,
    /// Its name.
    pub name: String,
    /// Its latitude, degrees.
    pub lat: f64,
    /// Its longitude, degrees.
    pub lon: f64,
    /// Its distance from the own position (CPA) or the fence (enter, exit), m.
    pub distance_m: f64,
    /// The distance at the closest point, m (CPA).
    pub dcpa_m: Option<f64>,
    /// Seconds to the closest point (CPA).
    pub tcpa_s: Option<f64>,
}

impl Shift for Alert {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

impl Alert {
    /// The log line of the alert.
    pub fn line(&self) -> String {
        let who = if self.name.is_empty() {
            format!("{:09}", self.mmsi)
        } else {
            format!("{:09} {}", self.mmsi, self.name)
        };
        match self.kind {
            AlertKind::Cpa => format!(
                "AIS alert: {who} will pass {:.2} nm in {:.0} min",
                self.dcpa_m.unwrap_or(0.0) / NAUTICAL_MILE_M,
                self.tcpa_s.unwrap_or(0.0) / 60.0
            ),
            AlertKind::Enter => format!("AIS alert: {who} entered {}", self.fence),
            AlertKind::Exit => format!("AIS alert: {who} left {}", self.fence),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct VesselMemory {
    inside: BTreeMap<String, bool>,
    cpa_active: bool,
    cpa_last: Option<f64>,
}

/// Settings of the alert engine.
#[derive(Clone, Debug, PartialEq)]
pub struct AlertSettings {
    /// CPA alerts on.
    pub cpa: bool,
    /// The CPA limit, nautical miles.
    pub cpa_nm: f64,
    /// The TCPA limit, minutes.
    pub tcpa_min: f64,
    /// The least time between two CPA alerts of one vessel, minutes.
    pub cooldown_min: f64,
    /// The own position (the station's).
    pub own: Option<Position>,
    /// The own ship moves at `own_sog` knots on `own_cog` degrees.
    pub own_moving: bool,
    /// The own speed, knots.
    pub own_sog: f64,
    /// The own course, degrees.
    pub own_cog: f64,
}

impl Default for AlertSettings {
    fn default() -> Self {
        AlertSettings {
            cpa: false,
            cpa_nm: 0.5,
            tcpa_min: 15.0,
            cooldown_min: 10.0,
            own: None,
            own_moving: false,
            own_sog: 0.0,
            own_cog: 0.0,
        }
    }
}

/// The alert engine: CPA and geofence transitions, with a bounded memory per vessel.
#[derive(Clone, Debug, Default)]
pub struct AlertEngine {
    /// The settings.
    pub settings: AlertSettings,
    /// The fences.
    pub fences: Vec<Fence>,
    memory: BTreeMap<u32, VesselMemory>,
    scratch: Vec<Position>,
}

impl AlertEngine {
    /// An engine with no fences.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the fence lines (the memory per vessel is kept).
    ///
    /// # Errors
    ///
    /// [`BadText`] with the byte position of the first bad line. The fences do not change.
    pub fn set_fences(&mut self, text: &str) -> Result<(), BadText> {
        let (f, e) = parse_fences(text);
        if let Some(&(_, at, _)) = e.first() {
            return Err(BadText { at });
        }
        self.fences = f;
        Ok(())
    }

    /// Checks a station after a position update at `at`. Returns the new alerts.
    pub fn check(&mut self, s: &Station, at: Stamp) -> Vec<Alert> {
        let mut out = Vec::new();
        let Some(p) = s.position() else {
            return out;
        };
        let now = at.t;
        if now - s.position_time > 180.0 {
            return out;
        }
        let first = !self.memory.contains_key(&s.mmsi);
        let mem = self.memory.entry(s.mmsi).or_default();
        let alert = |kind, fence: &str, distance_m, dcpa_m, tcpa_s| Alert {
            at,
            kind,
            fence: fence.into(),
            mmsi: s.mmsi,
            name: s.name.clone(),
            lat: p.lat_deg,
            lon: p.lon_deg,
            distance_m,
            dcpa_m,
            tcpa_s,
        };
        for f in &self.fences {
            let inside = f.contains(&p, &mut self.scratch);
            let was = mem.inside.insert(f.name.clone(), inside);
            if let (false, Some(w)) = (first, was) {
                if w != inside {
                    let kind = if inside {
                        AlertKind::Enter
                    } else {
                        AlertKind::Exit
                    };
                    out.push(alert(kind, &f.name, f.distance_m(&p), None, None));
                }
            }
        }
        let st = &self.settings;
        if let (true, Some(own)) = (st.cpa, st.own) {
            if !matches!(
                s.class,
                StationClass::BaseStation | StationClass::AidToNavigation
            ) {
                let (os, oc) = if st.own_moving {
                    (st.own_sog, st.own_cog)
                } else {
                    (0.0, 0.0)
                };
                let c = cpa(own, os, oc, p, s.sog_kn, s.cog_deg);
                let limit = st.cpa_nm * NAUTICAL_MILE_M;
                let close = c.dcpa_m < limit
                    && ((c.predictable && c.tcpa_s <= st.tcpa_min * 60.0) || c.distance_m < limit);
                // Cleared once the predicted pass is well clear of the limit.
                if mem.cpa_active && c.dcpa_m > 1.25 * limit {
                    mem.cpa_active = false;
                }
                let cooled = mem
                    .cpa_last
                    .is_none_or(|t| now - t >= st.cooldown_min * 60.0);
                if close && !mem.cpa_active && cooled {
                    mem.cpa_active = true;
                    mem.cpa_last = Some(now);
                    out.push(alert(
                        AlertKind::Cpa,
                        "",
                        c.distance_m,
                        Some(c.dcpa_m),
                        Some(c.tcpa_s),
                    ));
                }
            }
        }
        // Bounded memory: forget vessels inside no fence and with no CPA active.
        if self.memory.len() > 4000 {
            self.memory
                .retain(|_, m| m.cpa_active || m.inside.values().any(|v| *v));
        }
        out
    }

    /// Forgets the memory of every vessel.
    pub fn clear(&mut self) {
        self.memory.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NM: f64 = NAUTICAL_MILE_M;

    #[test]
    fn cpa_geometry() {
        let own = Position::new(50.0, -1.0);
        let at = |lat: f64, lon: f64| Position::new(lat, lon);
        // Head-on: a vessel 2 nm north steaming south at 10 kn: TCPA 12 min, DCPA 0.
        let c = cpa(
            own,
            0.0,
            0.0,
            at(50.0 + 2.0 / 60.0, -1.0),
            Some(10.0),
            Some(180.0),
        );
        assert!(
            c.dcpa_m < 0.01 * NM && (c.tcpa_s - 720.0).abs() < 6.0,
            "{c:?}"
        );
        // Diverging: the closest point is now.
        let c = cpa(
            own,
            0.0,
            0.0,
            at(50.0 + 2.0 / 60.0, -1.0),
            Some(10.0),
            Some(0.0),
        );
        assert!(c.tcpa_s == 0.0 && (c.dcpa_m - c.distance_m).abs() < 1e-6);
        // Crossing ahead 1 nm east of us, northbound from 1 nm south: CPA 1 nm.
        let c = cpa(
            own,
            0.0,
            0.0,
            at(
                50.0 - 1.0 / 60.0,
                -1.0 + 1.0 / 60.0 / (50f64).to_radians().cos(),
            ),
            Some(12.0),
            Some(0.0),
        );
        assert!((c.dcpa_m - NM).abs() < 0.01 * NM && c.tcpa_s > 0.0, "{c:?}");
        // Parallel at the same speed: no relative motion.
        let c = cpa(own, 10.0, 90.0, at(50.01, -1.0), Some(10.0), Some(90.0));
        assert_eq!(c.tcpa_s, 0.0);
        // Stationary target: unknown course.
        let c = cpa(own, 0.0, 0.0, at(50.001, -1.0), None, None);
        assert!(!c.predictable && c.tcpa_s == 0.0);
    }

    #[test]
    fn fences_and_filter() {
        let (f, e) = parse_fences("# x\nharbour;50.8,-1.1,2\ndateline;poly;10,179 10,-179 -10,-179 -10,179\n;1,2,3\nb;1,2\nc;95,0,1\nd;1,2,0\ne;poly;1,2 3\nf;poly;1,2 3,4\ng;a;b;c\n");
        assert_eq!(f.len(), 2);
        let reasons: Vec<FenceFault> = e.iter().map(|x| x.2).collect();
        assert_eq!(
            reasons,
            vec![
                FenceFault::NoName,
                FenceFault::NotACircle,
                FenceFault::BadPosition,
                FenceFault::BadRadius,
                FenceFault::BadPolygonPoint,
                FenceFault::FewCorners,
                FenceFault::NotAFence,
            ]
        );
        let mut s = Vec::new();
        let inside = |lat, lon, s: &mut Vec<Position>| f[1].contains(&Position::new(lat, lon), s);
        assert!(
            inside(0.0, 180.0, &mut s)
                && inside(0.0, -179.5, &mut s)
                && !inside(0.0, 170.0, &mut s)
        );
        let mut eng = AlertEngine::new();
        assert_eq!(eng.set_fences("a;1,2,3\nx;1,2"), Err(BadText { at: 8 }));
        eng.set_fences("harbour;50.8,-1.1,2").unwrap();
        let mut st = Station::new(232_000_001);
        (st.lat, st.lon) = (Some(50.8), Some(-1.1));
        // The first position inside does not enter.
        assert!(eng.check(&st, Stamp::at(0.0)).is_empty());
        st.lat = Some(51.0);
        assert_eq!(eng.check(&st, Stamp::at(1.0))[0].kind, AlertKind::Exit);
        st.lat = Some(50.8);
        assert_eq!(eng.check(&st, Stamp::at(2.0))[0].kind, AlertKind::Enter);
        let fl = Filter::parse("classes=1;moving=1;sog=2").unwrap();
        assert_eq!(fl.text(), "classes=1;groups=1023;moving=1;sog=2");
        assert_eq!(Filter::parse(&fl.text()), Ok(fl.clone()));
        assert!(fl.narrowed());
        assert_eq!(Filter::parse("moving=0;groups=bad"), Err(BadText { at: 9 }));
        assert!(Filter::parse("zz=1").is_err());
        assert_eq!(Filter::parse(""), Ok(Filter::default()));
    }

    #[test]
    fn cpa_clear_margin_and_cooldown() {
        let mut eng = AlertEngine::new();
        eng.settings.cpa = true;
        eng.settings.own = Some(Position::new(50.0, -1.0));
        let mut s = Station::new(232_000_002);
        s.class = StationClass::A;
        (s.lat, s.lon) = (Some(50.0 + 2.0 / 60.0), Some(-1.0));
        (s.sog_kn, s.cog_deg) = (Some(10.0), Some(180.0));
        assert_eq!(eng.check(&s, Stamp::at(0.0)).len(), 1);
        assert!(eng.check(&s, Stamp::at(10.0)).is_empty());
        // Cleared (moving away), then closing again within the cool-down: nothing. After it: again.
        s.cog_deg = Some(0.0);
        s.position_time = 20.0;
        eng.check(&s, Stamp::at(20.0));
        s.cog_deg = Some(180.0);
        s.position_time = 30.0;
        assert!(eng.check(&s, Stamp::at(30.0)).is_empty());
        s.position_time = 700.0;
        assert_eq!(eng.check(&s, Stamp::at(700.0)).len(), 1);
    }
}
