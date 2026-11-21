//! Satellite identification: content (the system table's longitude, the bulletin board's ocean region), frequency (the decoded P
//! carriers in a satellite's channel list) and geometry (the slot's elevation from the station), each scored 0 to 1 and combined.

use sdrkit_util_geo::{Observer, Position};

/// A candidate satellite.
pub struct Sat {
    pub name: &'static str,
    /// East positive.
    pub lon: f64,
    pub region: Option<u8>,
    /// P 600 / 1200 list and P 10500 list (MHz ranges).
    pub p_list: Option<(f64, f64)>,
    pub p10500: Option<(f64, f64)>,
}

pub const SATELLITES: [Sat; 8] = [
    Sat {
        name: "Alphasat 25E",
        lon: 25.0,
        region: None,
        p_list: Some((1545.115, 1545.130)),
        p10500: Some((1545.315, 1545.3275)),
    },
    Sat {
        name: "Inmarsat-4 F3 98W",
        lon: -98.0,
        region: None,
        p_list: Some((1545.020, 1545.205)),
        p10500: Some((1546.005, 1546.0775)),
    },
    Sat {
        name: "Inmarsat-3 F5 54W (AOR-W)",
        lon: -54.0,
        region: Some(0),
        p_list: Some((1545.025, 1545.040)),
        p10500: Some((1546.055, 1546.070)),
    },
    Sat {
        name: "Inmarsat-6 F1 83.5E",
        lon: 83.5,
        region: None,
        p_list: Some((1545.160, 1545.225)),
        p10500: Some((1546.0425, 1546.1225)),
    },
    Sat {
        name: "Inmarsat-4 F1 143.5E",
        lon: 143.5,
        region: None,
        p_list: None,
        p10500: None,
    },
    Sat {
        name: "Inmarsat 15.5W (AOR-E)",
        lon: -15.5,
        region: Some(1),
        p_list: None,
        p10500: None,
    },
    Sat {
        name: "Inmarsat 64.5E (IOR)",
        lon: 64.5,
        region: Some(3),
        p_list: None,
        p10500: None,
    },
    Sat {
        name: "Inmarsat 178E (POR)",
        lon: 178.0,
        region: Some(2),
        p_list: None,
        p10500: None,
    },
];

/// The nominal slots of the ocean regions.
fn region_slot(r: u8) -> Option<f64> {
    match r {
        0 => Some(-54.0),
        1 => Some(-15.5),
        2 => Some(178.0),
        3 => Some(64.5),
        _ => None,
    }
}

/// What has been heard.
#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub longitudes: Vec<f64>,
    pub region: Option<u8>,
    /// Decoded P carriers: absolute frequency and rate (0 unknown).
    pub p_freqs: Vec<(f64, u32)>,
    pub location: Option<(f64, f64)>,
    pub spot: Option<bool>,
    pub manual: Option<String>,
}

/// One candidate's scores.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Candidate {
    pub name: String,
    pub confidence: f64,
    pub content: Option<f64>,
    pub frequency: Option<f64>,
    pub geometry: Option<f64>,
    pub elevation: Option<f64>,
    pub azimuth: Option<f64>,
    pub words: Vec<String>,
}

/// The result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identification {
    pub name: String,
    pub confidence: f64,
    pub ambiguous: bool,
    pub beam: String,
    pub candidates: Vec<Candidate>,
    pub note: String,
}

/// The radius of the geostationary orbit, m.
pub const GEO_RADIUS_M: f64 = 42_164_170.0;

/// The look angles (azimuth, elevation, degrees) of a geostationary slot (east longitude) from a point on the WGS-84 ellipsoid.
pub fn look_angles(lat: f64, lon: f64, slot: f64) -> (f64, f64) {
    let obs = Observer::new(Position::new(lat, lon));
    // On the equator the WGS-84 radius is the semi-major axis, so the orbit's height above the ellipsoid is the rest.
    let sat = Position {
        alt_m: GEO_RADIUS_M - 6_378_137.0,
        ..Position::new(0.0, slot)
    };
    let l = obs.look(&sat.to_ecef());
    (l.azimuth_deg, l.elevation_deg)
}

fn lon_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

fn in_list(s: &Sat, f: f64, rate: u32) -> Option<bool> {
    let mhz = f / 1e6;
    let tol = 0.004;
    let use10500 = rate >= 10500 || (rate == 0 && mhz > 1545.95);
    let l = if use10500 { s.p10500 } else { s.p_list }?;
    Some(mhz >= l.0 - tol && mhz <= l.1 + tol)
}

/// Identifies the satellite.
pub fn identify(ev: &Evidence) -> Identification {
    let beam = match ev.spot {
        Some(true) => "spot beam",
        Some(false) => "global beam",
        None => "",
    }
    .to_string();
    if let Some(m) = &ev.manual {
        return Identification {
            name: m.clone(),
            confidence: 1.0,
            beam,
            note: "chosen by hand".into(),
            ..Default::default()
        };
    }
    let p_carriers: Vec<&(f64, u32)> = ev
        .p_freqs
        .iter()
        .filter(|p| (1545.0e6..=1546.2e6).contains(&p.0))
        .collect();
    let mut cands: Vec<Candidate> = SATELLITES
        .iter()
        .map(|s| {
            let mut c = Candidate {
                name: s.name.into(),
                ..Default::default()
            };
            // Content.
            let mut content: Option<f64> = None;
            for &l in &ev.longitudes {
                let d = lon_diff(l, s.lon);
                let v = if d <= 1.6 {
                    1.0
                } else if d <= 3.1 {
                    0.6
                } else {
                    0.0
                };
                content = Some(content.map_or(v, |c: f64| c.max(v)));
            }
            if let Some(r) = ev.region {
                let v = match s.region {
                    Some(sr) if sr == r => 1.0,
                    Some(_) => 0.0,
                    None => match region_slot(r) {
                        Some(slot) if lon_diff(slot, s.lon) <= 3.1 => 0.8,
                        _ => 0.0,
                    },
                };
                content = Some(content.map_or(v, |c: f64| c.max(v)));
            }
            c.content = content;
            match content {
                Some(v) if v >= 0.6 => c
                    .words
                    .push("the system table or bulletin board points at this satellite".into()),
                Some(_) => c
                    .words
                    .push("the system table or bulletin board points elsewhere".into()),
                None => c
                    .words
                    .push("no system table or bulletin board decoded yet".into()),
            }
            // Frequency.
            if !p_carriers.is_empty() && (s.p_list.is_some() || s.p10500.is_some()) {
                let hits = p_carriers
                    .iter()
                    .filter(|p| in_list(s, p.0, p.1) == Some(true))
                    .count();
                c.frequency = Some(hits as f64 / p_carriers.len() as f64);
                c.words.push(format!(
                    "{hits} of {} P carriers are in this satellite's channel list",
                    p_carriers.len()
                ));
            }
            // Geometry.
            if let Some((la, lo)) = ev.location {
                let (az, el) = look_angles(la, lo, s.lon);
                c.azimuth = Some(az);
                c.elevation = Some(el);
                c.geometry = Some(if el <= 2.0 {
                    0.0
                } else {
                    (0.2 + (el - 2.0) / 30.0).min(1.0)
                });
                c.words.push(format!(
                    "azimuth {az:.0}, elevation {el:.0} degrees from your location"
                ));
            }
            // Combination.
            let mut wsum = 0.0;
            let mut vsum = 0.0;
            let mut sources = 0;
            let mut strong = 0;
            for (v, w) in [(c.content, 0.5), (c.frequency, 0.3)] {
                if let Some(v) = v {
                    wsum += w;
                    vsum += w * v;
                    sources += 1;
                    if v >= 0.6 {
                        strong += 1;
                    }
                }
            }
            let mut conf = if sources == 0 {
                0.0
            } else {
                (vsum / wsum).min(if sources == 1 { 0.6 } else { 0.85 })
            };
            if let Some(g) = c.geometry {
                if sources > 0 && g > 0.0 {
                    conf += 0.15;
                }
            }
            if sources > 0 {
                conf *= 0.5 + 0.5 * strong as f64 / sources as f64;
            }
            if let Some(el) = c.elevation {
                if el <= 0.0 {
                    conf *= 0.25;
                } else if el < 10.0 {
                    conf *= 0.8;
                }
            }
            c.confidence = conf;
            c
        })
        .collect();
    cands.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
    cands.truncate(6);
    let best = cands.first().cloned().unwrap_or_default();
    let second = cands.get(1).map(|c| c.confidence).unwrap_or(0.0);
    let mut id = Identification {
        beam,
        candidates: cands.clone(),
        ..Default::default()
    };
    if best.confidence >= 0.2 {
        id.ambiguous = second >= 0.2 && best.confidence - second <= 0.15;
        id.name = best.name.clone();
        id.confidence = best.confidence;
        id.note = if id.ambiguous {
            "ambiguous".into()
        } else {
            String::new()
        };
    } else {
        id.note = "satellite not identified yet".into();
        // A system table longitude that matches no slot.
        if let Some(&l) = ev.longitudes.first() {
            if SATELLITES.iter().all(|s| lon_diff(l, s.lon) > 3.1) {
                id.note = format!(
                    "a satellite at {:.1}{} (not in the table)",
                    l.abs(),
                    if l < 0.0 { "W" } else { "E" }
                );
            }
        }
    }
    id
}

/// The satellite name a carrier is labelled with: in the satellite's list and its confidence within 0.15 of the best, or the
/// identification at least 0.5 sure.
pub fn carrier_label(id: &Identification, f: f64, rate: u32) -> String {
    if id.name.is_empty() {
        return String::new();
    }
    if id.confidence >= 0.5 {
        return id.name.clone();
    }
    for c in &id.candidates {
        if id.confidence - c.confidence <= 0.15 {
            if let Some(s) = SATELLITES.iter().find(|s| s.name == c.name) {
                if f > 0.0 && in_list(s, f, rate) == Some(true) {
                    return c.name.clone();
                }
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i4_identification() {
        // A system table naming 25.5E with P carriers in Alphasat's list, from Europe.
        let ev = Evidence {
            longitudes: vec![25.5],
            p_freqs: vec![(1545.12e6, 1200), (1545.125e6, 600)],
            location: Some((48.0, 11.0)),
            ..Default::default()
        };
        let id = identify(&ev);
        assert_eq!(id.name, "Alphasat 25E");
        assert!(id.confidence > 0.8, "{}", id.confidence);
        // The same content from the other side of the Earth: ambiguous or refused.
        let mut far = ev.clone();
        far.location = Some((-40.0, -150.0));
        let id = identify(&far);
        assert!(id.ambiguous || id.confidence < 0.5, "{id:?}");
        // An IOR bulletin board names the 64.5E slot.
        let id = identify(&Evidence {
            region: Some(3),
            ..Default::default()
        });
        assert_eq!(id.name, "Inmarsat 64.5E (IOR)");
        // No evidence.
        assert_eq!(
            identify(&Evidence::default()).note,
            "satellite not identified yet"
        );
        // Look angles: a slot straight south of a point on the equator is overhead.
        let (_, el) = look_angles(0.0, 25.0, 25.0);
        assert!((el - 90.0).abs() < 1e-6);
    }

    /// The look angles from a station on the WGS-84 ellipsoid (geodetic latitude, the local vertical normal to the ellipsoid),
    /// worked out here from the ellipsoid's parameters (NIMA TR8350.2 section 4).
    fn wgs84_look(lat: f64, lon: f64, slot: f64) -> (f64, f64) {
        let a = 6_378_137.0f64;
        let f = 1.0 / 298.257_223_563;
        let e2 = f * (2.0 - f);
        let (la, lo, sl) = (lat.to_radians(), lon.to_radians(), slot.to_radians());
        let n = a / (1.0 - e2 * la.sin().powi(2)).sqrt();
        let o = [
            n * la.cos() * lo.cos(),
            n * la.cos() * lo.sin(),
            n * (1.0 - e2) * la.sin(),
        ];
        let s = [GEO_RADIUS_M * sl.cos(), GEO_RADIUS_M * sl.sin(), 0.0];
        let d = [s[0] - o[0], s[1] - o[1], s[2] - o[2]];
        let e = -lo.sin() * d[0] + lo.cos() * d[1];
        let nn = -la.sin() * lo.cos() * d[0] - la.sin() * lo.sin() * d[1] + la.cos() * d[2];
        let u = la.cos() * lo.cos() * d[0] + la.cos() * lo.sin() * d[1] + la.sin() * d[2];
        (
            e.atan2(nn).to_degrees().rem_euclid(360.0),
            u.atan2((e * e + nn * nn).sqrt()).to_degrees(),
        )
    }

    #[test]
    fn look_angles_use_the_wgs84_ellipsoid() {
        // A spherical Earth puts a station at 60 N about 0.1 degrees off in elevation: the identification uses WGS-84.
        for (lat, lon, slot) in [
            (60.0, 25.0, 25.0),
            (48.0, 11.0, -15.5),
            (-33.9, 151.2, 143.5),
        ] {
            let (az, el) = look_angles(lat, lon, slot);
            let (waz, wel) = wgs84_look(lat, lon, slot);
            assert!((el - wel).abs() < 1e-6, "{lat} {lon}: {el} against {wel}");
            assert!((az - waz).abs() < 1e-6);
        }
    }
}
