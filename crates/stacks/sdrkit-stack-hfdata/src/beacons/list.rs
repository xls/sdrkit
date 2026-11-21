//! The beacon list: the built-in 170 entries, the editable JSON file, the NCDXF/IARU schedule and the
//! band plan for hopping.

use serde::{Deserialize, Serialize};

sdrkit_core::choice_enum! {
    /// What keying a beacon has.
    #[derive(Serialize, Deserialize, PartialOrd, Ord)]
    pub enum Pattern {
        /// The NCDXF/IARU beacon network: callsign and four power steps in a 3-minute cycle.
        Ncdxf = ("ncdxf", "NCDXF/IARU"),
        /// A single-letter beacon.
        Letter = ("letter", "Single letter"),
        /// A Long Dashes beacon.
        Dashes = ("dashes", "Long Dashes"),
        /// A plain carrier.
        Carrier = ("carrier", "Carrier"),
    }
}

impl Pattern {
    /// The key of the list file.
    pub fn name(self) -> &'static str {
        match self {
            Pattern::Ncdxf => "ncdxf",
            Pattern::Letter => "letter",
            Pattern::Dashes => "dashes",
            Pattern::Carrier => "carrier",
        }
    }
}

/// Why a text is not a beacon list. `index` counts the beacons of the list from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListError {
    /// The text ends inside a value, at byte `at`.
    Unterminated {
        /// The byte where the text ran out.
        at: usize,
    },
    /// The text is not a JSON object of the list form (a value of the wrong kind, or not JSON).
    NotAnObject,
    /// The object has no `beacons` array.
    NoBeacons,
    /// A beacon has no frequency from 0 to 1 GHz.
    NoFrequency {
        /// Which beacon.
        index: usize,
    },
    /// A letter beacon's letter is not one of D P S C A F K M.
    BadLetter {
        /// Which beacon.
        index: usize,
    },
    /// An NCDXF beacon lacks `ncdxf_index` 0 to 17 or `ncdxf_band` 0 to 4.
    BadNcdxfSlot {
        /// Which beacon.
        index: usize,
    },
    /// A position is out of range.
    BadPosition {
        /// Which beacon.
        index: usize,
    },
}

impl ListError {
    /// The byte where the text went wrong, when known.
    pub fn at(&self) -> Option<usize> {
        match self {
            ListError::Unterminated { at } => Some(*at),
            _ => None,
        }
    }
}

/// One beacon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Beacon {
    pub name: String,
    pub khz: f64,
    pub pattern: Pattern,
    pub letter: Option<char>,
    pub ncdxf_index: Option<u8>,
    pub ncdxf_band: Option<u8>,
    pub dash_index: u8,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub mode: String,
    pub schedule: String,
    pub source: String,
    pub uncertain: bool,
    pub search_hz: f64,
}

/// The NCDXF/IARU beacons in sequence order: callsign, place, Maidenhead locator.
pub const NCDXF: [(&str, &str, &str); 18] = [
    ("4U1UN", "United Nations", "FN30as"),
    ("VE8AT", "Inuvik, Canada", "CP38"),
    ("W6WX", "California, USA", "CM87"),
    ("KH6RS", "Hawaii, USA", "BL10"),
    ("ZL6B", "New Zealand", "RE78"),
    ("VK6RBP", "Western Australia", "OF87"),
    ("JA2IGY", "Japan", "PM84"),
    ("RR9O", "Novosibirsk, Russia", "NO14"),
    ("VR2B", "Hong Kong", "OL72"),
    ("4S7B", "Sri Lanka", "MJ96"),
    ("ZS6DN", "South Africa", "KG44"),
    ("5Z4B", "Kenya", "KI88"),
    ("4X6TU", "Israel", "KM72"),
    ("OH2B", "Finland", "KP20"),
    ("CS3B", "Madeira", "IM12"),
    ("LU4AA", "Argentina", "GF05"),
    ("OA4B", "Peru", "FH17"),
    ("YV5B", "Venezuela", "FJ69"),
];

/// The NCDXF band frequencies, kHz.
pub const NCDXF_KHZ: [f64; 5] = [14100.0, 18110.0, 21150.0, 24930.0, 28200.0];

/// The Long Dashes: kHz and place in the 27 s cycle.
pub const DASHES: [(f64, u8); 8] = [
    (29996.0, 0),
    (27928.0, 2),
    (21899.0, 3),
    (16174.0, 4),
    (12494.0, 5),
    (11999.0, 6),
    (8999.0, 7),
    (6496.0, 8),
];

/// The single-letter markers and their base cities.
pub const LETTERS: [(char, &str, f64, f64); 8] = [
    ('D', "Sevastopol", 44.62, 33.52),
    ('P', "Kaliningrad", 54.71, 20.51),
    ('S', "Severomorsk", 69.07, 33.42),
    ('C', "Moscow", 55.75, 37.62),
    ('A', "Astrakhan", 46.35, 48.04),
    ('F', "Vladivostok", 43.12, 131.89),
    ('K', "Petropavlovsk-Kamchatsky", 53.04, 158.65),
    ('M', "Magadan", 59.56, 150.81),
];

/// The first marker of each single-letter cluster, kHz.
pub const CLUSTERS: [f64; 9] = [
    3594.7, 4557.7, 5153.7, 7508.7, 8494.7, 10871.7, 13527.7, 16331.7, 20047.7,
];

/// The centre of a Maidenhead locator of 2 to 10 characters, as (latitude, longitude) in degrees.
///
/// # Errors
///
/// The error of `sdrkit_util_geo::maidenhead::decode` for a locator that is not one (a bad character, a field letter past R, an
/// odd length).
pub fn locator_centre(loc: &str) -> Result<(f64, f64), sdrkit_util_geo::Error> {
    let c = sdrkit_util_geo::maidenhead::decode(loc)?.centre();
    Ok((c.lat_deg, c.lon_deg))
}

fn default_search(p: Pattern) -> f64 {
    match p {
        Pattern::Letter => 50.0,
        Pattern::Dashes => 1000.0,
        _ => 150.0,
    }
}

/// The built-in list (170 entries).
pub fn builtin() -> Vec<Beacon> {
    let mut v = Vec::new();
    for (bi, &khz) in NCDXF_KHZ.iter().enumerate() {
        for (k, &(call, place, loc)) in NCDXF.iter().enumerate() {
            let (lat, lon) = match locator_centre(loc) {
                Ok((a, o)) => (Some(a), Some(o)),
                Err(_) => (None, None),
            };
            let slot = (k + bi) % 18;
            v.push(Beacon {
                name: call.into(),
                khz,
                pattern: Pattern::Ncdxf,
                letter: None,
                ncdxf_index: Some(k as u8),
                ncdxf_band: Some(bi as u8),
                dash_index: 0,
                lat,
                lon,
                mode: "CW + 4 dashes (100 W, 10 W, 1 W, 0.1 W)".into(),
                schedule: format!(
                    "10 s slot {slot} of 18 in each 3 minute cycle (UTC), 00:00 start"
                ),
                source: format!("NCDXF/IARU International Beacon Project ({place}, {loc})"),
                uncertain: false,
                search_hz: 150.0,
            });
        }
    }
    for &(khz, place) in &DASHES {
        v.push(Beacon {
            name: format!("Long Dashes {khz:.0}"),
            khz,
            pattern: Pattern::Dashes,
            letter: None,
            ncdxf_index: None,
            ncdxf_band: None,
            dash_index: place,
            lat: Some(-25.0),
            lon: Some(134.0),
            mode: "USB OOK, 2.9 s dash with a 0.2 s multitone start".into(),
            schedule: "27 s cycle, descending from 29996 kHz; on air only some hours a day".into(),
            source: "sigidwiki.com/wiki/Long_Dashes".into(),
            uncertain: true,
            search_hz: 1000.0,
        });
    }
    for &c in &CLUSTERS {
        for (i, &(l, city, lat, lon)) in LETTERS.iter().enumerate() {
            let mut khz = c + 0.1 * i as f64;
            let mut uncertain = false;
            if c == 7508.7 && matches!(l, 'F' | 'K' | 'M') {
                khz = 7039.2 + 0.1 * (i as f64 - 5.0);
                uncertain = true;
            }
            let khz = (khz * 10.0).round() / 10.0;
            v.push(Beacon {
                name: format!("{l} {city} {khz:.1}"),
                khz,
                pattern: Pattern::Letter,
                letter: Some(l),
                ncdxf_index: None,
                ncdxf_band: None,
                dash_index: 0,
                lat: Some(lat),
                lon: Some(lon),
                mode: format!("CW letter {l}"),
                schedule: if uncertain {
                    "not all channels are active at a given time (possibly 7509.2 - 7509.4)".into()
                } else {
                    "not all channels are active at a given time".into()
                },
                source: "priyom.org, \"Russian single-letter beacons\"".into(),
                uncertain,
                search_hz: 50.0,
            });
        }
    }
    v
}

/// Reads a beacon list in the format of the list file (JSON).
///
/// # Errors
///
/// A [`ListError`] that says why the text is not a beacon list.
pub fn parse(text: &str) -> Result<Vec<Beacon>, ListError> {
    let file: ListIn = serde_json::from_str(text).map_err(|e| {
        if e.is_eof() {
            ListError::Unterminated { at: text.len() }
        } else {
            ListError::NotAnObject
        }
    })?;
    let mut v = Vec::new();
    for (index, o) in file
        .beacons
        .ok_or(ListError::NoBeacons)?
        .into_iter()
        .enumerate()
    {
        let khz = o.khz.or_else(|| o.mhz.map(|m| m * 1000.0));
        let Some(khz) = khz.filter(|k| (0.0..=1e6).contains(k)) else {
            return Err(ListError::NoFrequency { index });
        };
        let pattern = match o.pattern.as_deref() {
            Some("ncdxf") => Pattern::Ncdxf,
            Some("letter") => Pattern::Letter,
            Some("dashes") => Pattern::Dashes,
            _ => Pattern::Carrier,
        };
        let mut letter = None;
        if pattern == Pattern::Letter {
            let l = o
                .letter
                .as_deref()
                .and_then(|x| x.chars().next())
                .map(|c| c.to_ascii_uppercase());
            match l {
                Some(c) if "DPSCAFKM".contains(c) => letter = Some(c),
                _ => return Err(ListError::BadLetter { index }),
            }
        }
        let (mut ni, mut nb) = (None, None);
        if pattern == Pattern::Ncdxf {
            let i = o
                .ncdxf_index
                .filter(|x| (0.0..=17.0).contains(x) && x.fract() == 0.0);
            let b = o
                .ncdxf_band
                .filter(|x| (0.0..=4.0).contains(x) && x.fract() == 0.0);
            match (i, b) {
                (Some(i), Some(b)) => {
                    ni = Some(i as u8);
                    nb = Some(b as u8);
                }
                _ => return Err(ListError::BadNcdxfSlot { index }),
            }
        }
        let (lat, lon) = (o.lat, o.lon);
        if lat.is_some_and(|x| !(-90.0..=90.0).contains(&x))
            || lon.is_some_and(|x| !(-180.0..=360.0).contains(&x))
        {
            return Err(ListError::BadPosition { index });
        }
        v.push(Beacon {
            name: o.name.unwrap_or_else(|| format!("{khz} kHz")),
            khz,
            pattern,
            letter,
            ncdxf_index: ni,
            ncdxf_band: nb,
            dash_index: o.dash_index.unwrap_or(0.0).clamp(0.0, 8.0) as u8,
            lat,
            lon: lon.map(|x| if x > 180.0 { x - 360.0 } else { x }),
            mode: o.mode.unwrap_or_default(),
            schedule: o.schedule.unwrap_or_default(),
            source: o.source.unwrap_or_default(),
            uncertain: o.uncertain.unwrap_or(false),
            search_hz: o
                .search_hz
                .filter(|x| *x > 0.0)
                .unwrap_or(default_search(pattern)),
        });
    }
    Ok(v)
}

/// The list file as it is read: every entry and field can be missing.
#[derive(Deserialize)]
struct ListIn {
    beacons: Option<Vec<EntryIn>>,
}

/// One beacon as it is read.
#[derive(Deserialize)]
struct EntryIn {
    name: Option<String>,
    khz: Option<f64>,
    mhz: Option<f64>,
    pattern: Option<String>,
    letter: Option<String>,
    ncdxf_index: Option<f64>,
    ncdxf_band: Option<f64>,
    dash_index: Option<f64>,
    lat: Option<f64>,
    lon: Option<f64>,
    mode: Option<String>,
    schedule: Option<String>,
    source: Option<String>,
    uncertain: Option<bool>,
    search_hz: Option<f64>,
}

/// One beacon as the list file writes it.
#[derive(Serialize)]
struct Entry<'a> {
    name: &'a str,
    khz: f64,
    pattern: &'static str,
    search_hz: f64,
    uncertain: bool,
    mode: &'a str,
    schedule: &'a str,
    source: &'a str,
    dash_index: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    letter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ncdxf_index: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ncdxf_band: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lat: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lon: Option<f64>,
}

/// The list file.
#[derive(Serialize)]
struct ListFile<'a> {
    version: u32,
    beacons: Vec<Entry<'a>>,
}

/// Writes a beacon list in the format of the list file (reading it back returns the same list).
pub fn write(list: &[Beacon]) -> String {
    let beacons = list
        .iter()
        .map(|b| {
            let ncdxf = b.ncdxf_index.zip(b.ncdxf_band);
            Entry {
                name: &b.name,
                khz: b.khz,
                pattern: b.pattern.name(),
                search_hz: b.search_hz,
                uncertain: b.uncertain,
                mode: &b.mode,
                schedule: &b.schedule,
                source: &b.source,
                dash_index: b.dash_index,
                letter: b.letter.map(|l| l.to_string()),
                ncdxf_index: ncdxf.map(|x| x.0),
                ncdxf_band: ncdxf.map(|x| x.1),
                lat: b.lat,
                lon: b.lon,
            }
        })
        .collect();
    serde_json::to_string(&ListFile {
        version: 1,
        beacons,
    })
    .unwrap_or_default()
}

/// The NCDXF beacon on band `b` at second `t` of UTC (0 .. 17).
pub fn ncdxf_on(band: u8, t: f64) -> u8 {
    let slot = ((t.rem_euclid(180.0)) / 10.0).floor() as i64;
    ((slot - band as i64).rem_euclid(18)) as u8
}

/// The next start (UTC seconds) of beacon `k` on band `b` at or after `t`.
pub fn ncdxf_next(k: u8, band: u8, t: f64) -> f64 {
    let slot = ((k as u32 + band as u32) % 18) as f64;
    let cycle = (t / 180.0).floor();
    let mut s = cycle * 180.0 + 10.0 * slot;
    while s < t - 1e-9 {
        s += 180.0;
    }
    s
}

/// The band rows: name and kHz range.
pub const BAND_ROWS: &[(&str, f64, f64)] = &[
    ("10 m", 28000.0, 29700.0),
    ("12 m", 24890.0, 24990.0),
    ("15 m", 21000.0, 21450.0),
    ("17 m", 18068.0, 18168.0),
    ("20 m", 14000.0, 14350.0),
];

/// The band row of a frequency (kHz).
pub fn band_row(khz: f64) -> &'static str {
    BAND_ROWS
        .iter()
        .find(|b| khz >= b.1 && khz <= b.2)
        .map(|b| b.0)
        .unwrap_or(if khz < 10_000.0 {
            "other below 10 MHz"
        } else {
            "other above 10 MHz"
        })
}

/// The radio centres (Hz) that cover every beacon (kHz) with a capture of `rate`: fewest first, in frequency order; no beacon within
/// 20 kHz of a centre, all within 0.4 x rate - 3 kHz.
pub fn band_plan(beacons_khz: &[f64], rate: f64) -> Vec<f64> {
    let mut f: Vec<f64> = beacons_khz.iter().map(|k| k * 1e3).collect();
    f.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    f.dedup();
    let span = 0.8 * rate - 8e3;
    let reach = 0.4 * rate - 3e3;
    let all = f.clone();
    let ok = |c: f64, g: &[f64]| {
        c >= 100e3
            && g.iter().all(|&x| (x - c).abs() <= reach)
            && all.iter().all(|&x| (x - c).abs() >= 20e3)
    };
    let mut plan = Vec::new();
    let mut i = 0;
    while i < f.len() {
        let mut j = i;
        while j + 1 < f.len() && f[j + 1] - f[i] <= span {
            j += 1;
        }
        // Shrink the group until a centre fits.
        loop {
            let g = &f[i..=j];
            let mid = (g[0] + g[g.len() - 1]) / 2.0;
            let mut found = None;
            if ok(mid, g) {
                found = Some(mid);
            } else {
                let mut d = 25e3;
                while d <= reach {
                    for c in [mid + d, mid - d] {
                        if ok(c, g) {
                            found = Some(c);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    d += 5e3;
                }
            }
            if let Some(c) = found {
                plan.push(c);
                i = j + 1;
                break;
            }
            if j == i {
                // A lone beacon that cannot be covered (too low): skip it.
                i += 1;
                break;
            }
            j -= 1;
        }
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b10_list() {
        let l = builtin();
        assert_eq!(l.len(), 170);
        assert_eq!(l.iter().filter(|b| b.pattern == Pattern::Ncdxf).count(), 90);
        assert_eq!(
            l.iter().filter(|b| b.pattern == Pattern::Letter).count(),
            72
        );
        assert_eq!(l.iter().filter(|b| b.pattern == Pattern::Dashes).count(), 8);
        assert_eq!(l.iter().filter(|b| b.uncertain).count(), 11);
        let mut calls: Vec<&str> = l
            .iter()
            .filter(|b| b.pattern == Pattern::Ncdxf)
            .map(|b| b.name.as_str())
            .collect();
        calls.sort();
        calls.dedup();
        assert_eq!(calls.len(), 18);
        let u = l.iter().find(|b| b.name == "4U1UN").unwrap();
        assert!(
            (40.0..=41.5).contains(&u.lat.unwrap()) && (-75.0..=-73.0).contains(&u.lon.unwrap())
        );
        let v = l.iter().find(|b| b.name == "VK6RBP").unwrap();
        assert!(
            (-33.0..=-31.0).contains(&v.lat.unwrap()) && (116.0..=118.0).contains(&v.lon.unwrap())
        );
        let c = l.iter().find(|b| b.letter == Some('C')).unwrap();
        assert!((c.lat.unwrap() - 55.75).abs() < 1e-9);
        assert_eq!(
            l.iter().find(|b| b.khz == 3594.7).unwrap().name,
            "D Sevastopol 3594.7"
        );
        // JSON round trip.
        assert_eq!(parse(&write(&l)).unwrap(), l);
        // A hand-written list.
        let h = parse(r#"{"version":1,"beacons":[{"khz":10000,"pattern":"carrier","search_hz":150},{"khz":4557.9,"pattern":"letter","letter":"s","search_hz":30}]}"#).unwrap();
        assert_eq!(h[0].pattern, Pattern::Carrier);
        assert_eq!(h[1].name, "4557.9 kHz");
        assert_eq!(h[1].letter, Some('S'));
        assert_eq!(h[1].search_hz, 30.0);
        assert_eq!(parse("[1]").unwrap_err(), ListError::NotAnObject);
        assert_eq!(parse("{}").unwrap_err(), ListError::NoBeacons);
        assert_eq!(
            parse(r#"{"beacons":[{"name":"X"}]}"#).unwrap_err(),
            ListError::NoFrequency { index: 0 }
        );
        assert_eq!(
            parse(r#"{"beacons":[{"khz":1,"pattern":"ncdxf","ncdxf_index":20,"ncdxf_band":0}]}"#)
                .unwrap_err(),
            ListError::BadNcdxfSlot { index: 0 }
        );
        assert_eq!(
            parse(r#"{"beacons":[{"khz":1,"pattern":"letter","letter":"Q"}]}"#).unwrap_err(),
            ListError::BadLetter { index: 0 }
        );
        assert_eq!(
            parse(r#"{"beacons":[{"khz":1,"lat":95}]}"#).unwrap_err(),
            ListError::BadPosition { index: 0 }
        );
        let text = r#"{"beacons":[{"name":"ab"#;
        assert_eq!(
            parse(text).unwrap_err(),
            ListError::Unterminated { at: text.len() }
        );
        assert_eq!(band_row(14100.0), "20 m");
        assert_eq!(band_row(3594.7), "other below 10 MHz");
        assert_eq!(band_row(11999.0), "other above 10 MHz");
    }

    #[test]
    fn b11_schedule_and_plan() {
        assert_eq!(ncdxf_on(0, 0.5), 0);
        assert_eq!(ncdxf_on(0, 10.5), 1);
        assert_eq!(ncdxf_on(1, 10.5), 0);
        assert_eq!(ncdxf_on(4, 40.5), 0);
        assert_eq!(ncdxf_on(0, 179.9), 17);
        assert_eq!(ncdxf_on(0, 180.1), 0);
        assert_eq!(ncdxf_next(3, 2, 7.0), 50.0);
        assert_eq!(ncdxf_next(3, 2, 55.0), 230.0);
        for b in 0..5u8 {
            let mut seen: Vec<u8> = (0..18)
                .map(|s| ncdxf_on(b, 10.0 * s as f64 + 5.0))
                .collect();
            seen.sort();
            seen.dedup();
            assert_eq!(seen.len(), 18);
            if b < 4 {
                for s in 0..18 {
                    let t = 10.0 * s as f64 + 5.0;
                    assert_eq!(ncdxf_on(b + 1, t + 10.0), ncdxf_on(b, t));
                }
            }
        }
        let all: Vec<f64> = builtin().iter().map(|b| b.khz).collect();
        for &rate in &[2e6, 10e6, 20e6] {
            let plan = band_plan(&all, rate);
            for &k in &all {
                let f = k * 1e3;
                assert!(
                    plan.iter().any(|&c| (f - c).abs() <= 0.4 * rate),
                    "{rate} {k}"
                );
            }
            for &c in &plan {
                assert!(
                    all.iter().all(|&k| (k * 1e3 - c).abs() >= 19e3),
                    "{rate} centre {c}"
                );
            }
            if rate == 20e6 {
                assert!(plan.len() <= 3, "{plan:?}");
            }
        }
        let dashes: Vec<f64> = DASHES.iter().map(|d| d.0).collect();
        assert!(band_plan(&dashes, 10e6).len() <= 4);
    }
}
