//! Satellite AIS: Doppler tracking per channel, the satellite from its two-line elements (by `sdrkit_util_geo::orbit`), and the analysis
//! of receptions into passes, vessels and months, with CSV exports.

use sdrkit_util_text::csv::push_record;

/// Appends one CSV record with the line end of RFC 4180 (CRLF).
fn push_row<'a>(out: &mut String, cells: impl IntoIterator<Item = &'a str>) {
    push_record(out, cells, ',');
    out.push_str("\r\n");
}
use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
use sdrkit_util_geo::orbit::{doppler_hz, Satellite};
use sdrkit_util_geo::time::{iso8601, Civil};
use sdrkit_util_geo::{Observer, Position};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The most burst frequencies a Doppler track keeps.
const TRACK_POINTS: usize = 40;

/// Doppler tracking of one channel: recent burst frequencies and a straight-line fit (3 points or more, the median before that).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DopplerTrack {
    points: VecDeque<(f64, f64)>,
}

impl DopplerTrack {
    /// Adds a burst's frequency `hz` at signal time `t`.
    pub fn add(&mut self, t: f64, hz: f64) {
        if self.points.len() >= TRACK_POINTS {
            self.points.pop_front();
        }
        self.points.push_back((t, hz));
    }

    /// The rate, Hz/s, when 3 points or more.
    pub fn rate(&self) -> Option<f64> {
        self.fit().map(|f| f.1)
    }

    /// The least-squares line `hz = a + b t` (centred sums), from 3 points.
    fn fit(&self) -> Option<(f64, f64)> {
        let n = self.points.len();
        if n < 3 {
            return None;
        }
        let mt = self.points.iter().map(|p| p.0).sum::<f64>() / n as f64;
        let mf = self.points.iter().map(|p| p.1).sum::<f64>() / n as f64;
        let (mut sxy, mut sxx) = (0.0, 0.0);
        for (t, f) in &self.points {
            sxy += (t - mt) * (f - mf);
            sxx += (t - mt) * (t - mt);
        }
        if sxx <= 0.0 {
            return Some((mf, 0.0));
        }
        let b = sxy / sxx;
        Some((mf - b * mt, b))
    }

    /// The estimate at signal time `t`.
    pub fn at(&self, t: f64) -> Option<f64> {
        if let Some((a, b)) = self.fit() {
            return Some(a + b * t);
        }
        // The median of one or two points: the upper one.
        self.points.iter().map(|p| p.1).reduce(f64::max)
    }

    /// The recent frequencies (the last `window` seconds before `now`), clustered to 300 Hz, the most common first, at most `max`.
    pub fn hints(&self, now: f64, window: f64, max: usize) -> Vec<f64> {
        let mut clusters: Vec<(f64, usize)> = Vec::new();
        for &(t, f) in &self.points {
            if now - t > window {
                continue;
            }
            match clusters.iter_mut().find(|c| (c.0 - f).abs() < 300.0) {
                Some(c) => c.1 += 1,
                None => clusters.push((f, 1)),
            }
        }
        clusters.sort_by_key(|c| std::cmp::Reverse(c.1));
        clusters.into_iter().take(max).map(|c| c.0).collect()
    }

    /// Forgets every point.
    pub fn clear(&mut self) {
        self.points.clear();
    }
}

/// The satellite of the analysis: its name and its orbit (SGP4 from two-line elements).
#[derive(Clone, Debug)]
pub struct Elements {
    /// The name line, or empty.
    pub name: String,
    /// The orbit.
    pub sat: Satellite,
}

impl Elements {
    /// Parses a two-line element set (an optional name line, line 1, line 2) with its checksums.
    ///
    /// # Errors
    ///
    /// The geometry crate's error if the text is not a valid element set.
    pub fn parse(text: &str) -> Result<Elements, sdrkit_util_geo::Error> {
        let tle = Tle::parse(text.trim(), Checksum::Require)?;
        let sat = Satellite::from_tle(&tle)?;
        Ok(Elements {
            name: tle.name.as_str().to_string(),
            sat,
        })
    }

    /// Elevation, degrees, seen from a ground point at `unix`. `None` when the orbit cannot be propagated.
    pub fn elevation_deg(&self, unix: f64, at: &Observer) -> Option<f64> {
        self.sat.look(at, unix).ok().map(|l| l.elevation_deg)
    }
}

/// The satellite seen from many ground points at one time: its Earth-fixed position and velocity, computed once.
#[derive(Clone, Copy, Debug)]
pub struct Snapshot {
    pos: sdrkit_util_geo::Ecef,
    vel: [f64; 3],
}

impl Snapshot {
    /// The satellite at `unix`. `None` when the orbit cannot be propagated.
    pub fn of(e: &Elements, unix: f64) -> Option<Snapshot> {
        let (pos, vel) = e.sat.ecef_at(unix).ok()?;
        Some(Snapshot { pos, vel })
    }

    /// The elevation (degrees) and the Doppler at `carrier_hz` (Hz) seen from `p`.
    pub fn look(&self, p: Position, carrier_hz: f64) -> (f64, f64) {
        let l = Observer::new(p).look_moving(&self.pos, &self.vel);
        (l.elevation_deg, doppler_hz(carrier_hz, l.range_rate_m_s))
    }
}

/// One reception as the analysis sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rx {
    /// When, seconds since 1970 (UTC).
    pub unix: f64,
    /// The station.
    pub mmsi: u32,
    /// The message type.
    pub msg_type: u8,
    /// The SNR, dB.
    pub snr_db: f64,
    /// The latitude, degrees.
    pub lat: Option<f64>,
    /// The longitude, degrees.
    pub lon: Option<f64>,
}

/// One pass.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pass {
    /// Its number, from 1.
    pub index: usize,
    /// The satellite's name.
    pub satellite: String,
    /// Its first reception (or predicted AOS), seconds since 1970.
    pub start: f64,
    /// Its last reception (or predicted LOS).
    pub end: f64,
    /// It comes from the predicted passes.
    pub predicted: bool,
    /// Receptions in it.
    pub detections: usize,
    /// Messages 27 in it.
    pub msg27: usize,
    /// The vessels heard.
    pub vessels: Vec<u32>,
    /// The vessels that sent message 27.
    pub vessels_msg27: usize,
    /// Known vessels above the minimum elevation at the middle of the pass.
    pub expected: usize,
    /// Of those, the ones heard.
    pub expected_heard: usize,
    /// The highest elevation of a reception, degrees.
    pub max_elevation: Option<f64>,
    /// The mean SNR, dB.
    pub mean_snr: f64,
    /// Receptions by message type.
    pub types: BTreeMap<u8, usize>,
}

/// The numbers of one vessel.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VesselStats {
    /// First heard, seconds since 1970.
    pub first: f64,
    /// Last heard.
    pub last: f64,
    /// Messages.
    pub messages: usize,
    /// Messages 27.
    pub msg27: usize,
    /// Messages 1 to 3, 18 and 19.
    pub common: usize,
    /// Passes it was heard in.
    pub passes: usize,
    /// The last pass it was heard in.
    pub last_pass: usize,
    /// Its latitude, degrees.
    pub lat: Option<f64>,
    /// Its longitude, degrees.
    pub lon: Option<f64>,
    /// The SNR of its last reception, dB.
    pub last_snr: f64,
}

/// The analysis over all receptions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The passes.
    pub passes: Vec<Pass>,
    /// The vessels.
    pub vessels: BTreeMap<u32, VesselStats>,
    /// Receptions.
    pub receptions: usize,
    /// Vessels that sent message 27 ("new type").
    pub new_type: usize,
    /// Their share of all vessels.
    pub new_type_ratio: f64,
    /// The share of new-type vessels also heard on messages 1 to 3, 18 or 19.
    pub new_type_also_common: f64,
    /// Receptions by message type.
    pub types: BTreeMap<u8, usize>,
    /// (elevation bin start, mean SNR, count).
    pub snr_by_elevation: Vec<(f64, f64, usize)>,
    /// (YYYY-MM, passes, vessels, new-type vessels, detection probability).
    pub months: Vec<(String, usize, usize, usize, f64)>,
}

/// Analyses receptions: a silence of `gap_s` ends a pass, or the predicted passes (AOS, LOS) collect the receptions inside them.
/// With the satellite, receptions are binned by elevation and each pass counts the known vessels in view at its middle.
pub fn analyse(
    rx: &[Rx],
    gap_s: f64,
    sat: Option<&Elements>,
    predicted: &[(f64, f64)],
    min_el: f64,
) -> Analysis {
    let mut a = Analysis::default();
    let mut sorted: Vec<&Rx> = rx.iter().collect();
    sorted.sort_by(|x, y| x.unix.total_cmp(&y.unix));
    a.receptions = sorted.len();
    // Group into passes.
    let mut groups: Vec<(f64, f64, bool, Vec<&Rx>)> = Vec::new();
    if !predicted.is_empty() {
        for &(aos, los) in predicted {
            let inside = sorted
                .iter()
                .copied()
                .filter(|r| r.unix >= aos && r.unix <= los)
                .collect();
            groups.push((aos, los, true, inside));
        }
    } else {
        for r in &sorted {
            match groups.last_mut() {
                Some(g) if r.unix - g.1 <= gap_s => {
                    g.1 = r.unix;
                    g.3.push(r);
                }
                _ => groups.push((r.unix, r.unix, false, vec![r])),
            }
        }
    }
    let name = sat.map_or(String::new(), |s| s.name.clone());
    // Known ships with positions, for the footprint.
    let mut known: BTreeMap<u32, Observer> = BTreeMap::new();
    for r in &sorted {
        if let (Some(la), Some(lo)) = (r.lat, r.lon) {
            known.insert(r.mmsi, Observer::new(Position::new(la, lo)));
        }
    }
    for (i, (s, e, pred, rs)) in groups.iter().enumerate() {
        let mut p = Pass {
            index: i + 1,
            satellite: name.clone(),
            start: *s,
            end: *e,
            predicted: *pred,
            detections: rs.len(),
            ..Default::default()
        };
        let mut vs = BTreeSet::new();
        let mut v27 = BTreeSet::new();
        let mut snr = 0.0;
        for r in rs {
            vs.insert(r.mmsi);
            if r.msg_type == 27 {
                p.msg27 += 1;
                v27.insert(r.mmsi);
            }
            *p.types.entry(r.msg_type).or_default() += 1;
            snr += r.snr_db;
            let v = a.vessels.entry(r.mmsi).or_insert_with(|| VesselStats {
                first: r.unix,
                ..Default::default()
            });
            v.last = r.unix;
            v.messages += 1;
            if r.msg_type == 27 {
                v.msg27 += 1;
            }
            if matches!(r.msg_type, 1..=3 | 18 | 19) {
                v.common += 1;
            }
            if v.last_pass != i + 1 {
                v.passes += 1;
                v.last_pass = i + 1;
            }
            if r.lat.is_some() {
                v.lat = r.lat;
                v.lon = r.lon;
            }
            v.last_snr = r.snr_db;
            let el = match (sat, r.lat, r.lon) {
                (Some(sat), Some(la), Some(lo)) => {
                    sat.elevation_deg(r.unix, &Observer::new(Position::new(la, lo)))
                }
                _ => None,
            };
            if let Some(el) = el {
                p.max_elevation = Some(p.max_elevation.map_or(el, |m: f64| m.max(el)));
                let bin = (el / 10.0).floor().clamp(0.0, 8.0) * 10.0;
                match a.snr_by_elevation.iter_mut().find(|b| b.0 == bin) {
                    Some(b) => {
                        b.1 += r.snr_db;
                        b.2 += 1;
                    }
                    None => a.snr_by_elevation.push((bin, r.snr_db, 1)),
                }
            }
        }
        p.mean_snr = if rs.is_empty() {
            0.0
        } else {
            snr / rs.len() as f64
        };
        p.vessels_msg27 = v27.len();
        if let Some(sat) = sat {
            let mid = (p.start + p.end) / 2.0;
            for (m, obs) in &known {
                if sat.elevation_deg(mid, obs).is_some_and(|el| el >= min_el) {
                    p.expected += 1;
                    if vs.contains(m) {
                        p.expected_heard += 1;
                    }
                }
            }
        }
        p.vessels = vs.into_iter().collect();
        a.passes.push(p);
    }
    for r in &sorted {
        *a.types.entry(r.msg_type).or_default() += 1;
    }
    for b in &mut a.snr_by_elevation {
        b.1 /= b.2 as f64;
    }
    a.snr_by_elevation.sort_by(|x, y| x.0.total_cmp(&y.0));
    a.new_type = a.vessels.values().filter(|v| v.msg27 > 0).count();
    let nv = a.vessels.len().max(1);
    a.new_type_ratio = a.new_type as f64 / nv as f64;
    a.new_type_also_common = if a.new_type == 0 {
        0.0
    } else {
        a.vessels
            .values()
            .filter(|v| v.msg27 > 0 && v.common > 0)
            .count() as f64
            / a.new_type as f64
    };
    // Months.
    type Month = (usize, BTreeSet<u32>, BTreeSet<u32>, usize, usize);
    let mut months: BTreeMap<String, Month> = BTreeMap::new();
    for p in &a.passes {
        let key = Civil::from_unix_f64(p.start).map_or_else(
            |_| "-".to_string(),
            |c| format!("{:04}-{:02}", c.year, c.month),
        );
        let e = months.entry(key).or_default();
        e.0 += 1;
        for v in &p.vessels {
            e.1.insert(*v);
            if a.vessels.get(v).is_some_and(|s| s.msg27 > 0) {
                e.2.insert(*v);
            }
        }
        e.3 += p.expected_heard;
        e.4 += p.expected;
    }
    for (k, (np, vs, n27, heard, exp)) in months {
        let prob = if exp > 0 {
            heard as f64 / exp as f64
        } else if vs.is_empty() || np < 2 {
            0.0
        } else {
            // From the passes: the share of vessels heard in more than one pass.
            let multi = vs
                .iter()
                .filter(|v| a.vessels.get(v).is_some_and(|s| s.passes > 1))
                .count();
            multi as f64 / vs.len() as f64
        };
        a.months.push((k, np, vs.len(), n27.len(), prob));
    }
    a
}

/// A time as ISO 8601 UTC text to the second, or empty when out of range.
fn iso(t: f64) -> String {
    iso8601(t, 0).map_or_else(|_| String::new(), |s| s.as_str().to_string())
}

/// An optional number with `d` decimals, or empty.
fn opt(v: Option<f64>, d: usize) -> String {
    v.map_or_else(String::new, |x| format!("{x:.d$}"))
}

impl Analysis {
    /// The passes as CSV (RFC 4180).
    pub fn passes_csv(&self) -> String {
        let mut s = String::new();
        push_row(
            &mut s,
            [
                "index",
                "satellite",
                "start",
                "end",
                "predicted",
                "detections",
                "msg27",
                "vessels",
                "vessels_msg27",
                "expected",
                "expected_heard",
                "max_elevation",
                "mean_snr",
            ],
        );
        for p in &self.passes {
            let cells = [
                p.index.to_string(),
                p.satellite.clone(),
                iso(p.start),
                iso(p.end),
                p.predicted.to_string(),
                p.detections.to_string(),
                p.msg27.to_string(),
                p.vessels.len().to_string(),
                p.vessels_msg27.to_string(),
                p.expected.to_string(),
                p.expected_heard.to_string(),
                opt(p.max_elevation, 1),
                format!("{:.1}", p.mean_snr),
            ];
            push_row(&mut s, cells.iter().map(String::as_str));
        }
        s
    }

    /// The vessels as CSV (RFC 4180).
    pub fn vessels_csv(&self) -> String {
        let mut s = String::new();
        push_row(
            &mut s,
            [
                "mmsi", "first", "last", "messages", "msg27", "common", "passes", "lat", "lon",
                "last_snr",
            ],
        );
        for (m, v) in &self.vessels {
            let cells = [
                format!("{m:09}"),
                iso(v.first),
                iso(v.last),
                v.messages.to_string(),
                v.msg27.to_string(),
                v.common.to_string(),
                v.passes.to_string(),
                opt(v.lat, 5),
                opt(v.lon, 5),
                format!("{:.1}", v.last_snr),
            ];
            push_row(&mut s, cells.iter().map(String::as_str));
        }
        s
    }

    /// The months as CSV (RFC 4180).
    pub fn months_csv(&self) -> String {
        let mut s = String::new();
        push_row(
            &mut s,
            [
                "month",
                "passes",
                "vessels",
                "new_type",
                "detection_probability",
            ],
        );
        for m in &self.months {
            let cells = [
                m.0.clone(),
                m.1.to_string(),
                m.2.to_string(),
                m.3.to_string(),
                format!("{:.3}", m.4),
            ];
            push_row(&mut s, cells.iter().map(String::as_str));
        }
        s
    }
}

/// Receptions as CSV (RFC 4180).
pub fn receptions_csv(rx: &[Rx]) -> String {
    let mut s = String::new();
    push_row(&mut s, ["unix", "mmsi", "type", "snr_db", "lat", "lon"]);
    for r in rx {
        let cells = [
            format!("{:.3}", r.unix),
            format!("{:09}", r.mmsi),
            r.msg_type.to_string(),
            format!("{:.1}", r.snr_db),
            opt(r.lat, 5),
            opt(r.lon, 5),
        ];
        push_row(&mut s, cells.iter().map(String::as_str));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_geo::orbit::max_doppler_hz;

    const ISS: &str = "ISS (ZARYA)
1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927
2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";

    #[test]
    fn the_doppler_grid_covers_a_low_orbit() {
        // From 500 km at 162 MHz the largest Doppler is about 3.8 kHz: inside the decoder's +-4.5 kHz grid.
        let d = max_doppler_hz(162e6, 500e3);
        assert!((3600.0..4200.0).contains(&d), "{d}");
        let e = Elements::parse(ISS).unwrap();
        assert_eq!(e.name, "ISS (ZARYA)");
        let t = 1_222_000_000.0;
        let snap = Snapshot::of(&e, t).unwrap();
        let (_, dop) = snap.look(Position::new(51.5, -0.1), 162e6);
        assert!(dop.abs() < 5000.0, "{dop}");
        assert!(Elements::parse("1 25544U\n2 25544").is_err());
    }

    #[test]
    fn doppler_tracking() {
        let mut d = DopplerTrack::default();
        for k in 0..10 {
            let t = k as f64 * 2.0;
            d.add(t, 1000.0 + 30.0 * t);
        }
        assert!((d.rate().unwrap() - 30.0).abs() < 0.01);
        assert!((d.at(25.0).unwrap() - 1750.0).abs() < 0.5);
    }

    #[test]
    fn passes_and_vessels() {
        let r = |t: f64, m: u32, ty: u8| Rx {
            unix: 1.7e9 + t,
            mmsi: m,
            msg_type: ty,
            snr_db: 10.0,
            lat: None,
            lon: None,
        };
        let rx = vec![
            r(0.0, 1, 1),
            r(10.0, 2, 27),
            r(20.0, 3, 1),
            r(30.0, 1, 1),
            r(5000.0, 4, 27),
            r(5010.0, 1, 1),
        ];
        let a = analyse(&rx, 900.0, None, &[], 0.0);
        assert_eq!(a.passes.len(), 2);
        assert_eq!(
            (
                a.passes[0].detections,
                a.passes[0].vessels.len(),
                a.passes[0].vessels_msg27
            ),
            (4, 3, 1)
        );
        assert_eq!(
            (
                a.passes[1].detections,
                a.passes[1].vessels.len(),
                a.passes[1].msg27
            ),
            (2, 2, 1)
        );
        assert_eq!((a.vessels.len(), a.new_type), (4, 2));
        assert_eq!(a.passes_csv().lines().count(), 3);
        assert_eq!(a.months[0].0, "2023-11");
        assert!(a.passes_csv().contains("2023-11-14T22:13:20Z"));
    }
}
