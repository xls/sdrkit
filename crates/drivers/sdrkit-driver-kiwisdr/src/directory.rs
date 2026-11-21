//! The public receiver list: a parser for a listing that the host fetched, and sorting by distance.

use crate::info::Loose;
use crate::proto::{parse_address, parse_gps};
use serde::de::IgnoredAny;
use serde::Deserialize;

/// The most entries kept.
pub const MAX_ENTRIES: usize = 5000;
/// The deepest nesting read.
pub const MAX_DEPTH: usize = 16;
/// The most characters looked at.
pub const STEP_BUDGET: usize = 8_000_000;
/// The mean radius of the earth, km.
///
/// Standard: IUGG mean radius R1 of the GRS 80 ellipsoid (Moritz, Geodetic Reference System 1980).
pub const EARTH_KM: f64 = 6371.0;

/// One public receiver.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    /// Its name.
    pub name: String,
    /// The host (IPv6 without brackets).
    pub host: String,
    /// The port.
    pub port: u16,
    /// Its location text.
    pub location: String,
    /// The antenna text.
    pub antenna: String,
    /// The hardware text.
    pub sdr_hw: String,
    /// Latitude and longitude, when known.
    pub gps: Option<(f64, f64)>,
    /// Listeners now.
    pub users: u32,
    /// Channels.
    pub users_max: u32,
    /// The 0 to 30 MHz SNR figure.
    pub snr: Option<f64>,
    /// The covered band, Hz.
    pub bands: Option<(f64, f64)>,
    /// Marked offline.
    pub offline: bool,
    /// Its id.
    pub id: String,
}

impl Entry {
    /// Every channel is taken.
    pub fn full(&self) -> bool {
        self.users_max > 0 && self.users >= self.users_max
    }
    /// The address, `host:port`.
    pub fn address(&self) -> String {
        sdrkit_core::net::show(&self.host, self.port)
    }
}

/// False if the JSON text nests deeper than [`MAX_DEPTH`] within the step budget.
fn depth_ok(s: &[u8]) -> bool {
    let (mut depth, mut in_str, mut esc) = (0usize, false, false);
    for &b in s.iter().take(STEP_BUDGET) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > MAX_DEPTH {
                    return false;
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// One element of a listing.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Item {
    Entry(Box<Raw>),
    Other(IgnoredAny),
}

/// The fields of a listing entry that the parser reads.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Raw {
    url: Option<Loose>,
    name: Option<Loose>,
    loc: Option<Loose>,
    antenna: Option<Loose>,
    sdr_hw: Option<Loose>,
    gps: Option<Loose>,
    users: Option<Loose>,
    users_max: Option<Loose>,
    snr: Option<Loose>,
    bands: Option<Loose>,
    offline: Option<Loose>,
    id: Option<Loose>,
}

fn text(v: Option<Loose>) -> String {
    v.map(Loose::text).unwrap_or_default()
}

fn count(v: Option<&Loose>) -> u32 {
    v.and_then(Loose::number)
        .filter(|v| (0.0..=999.0).contains(v))
        .map_or(0, |v| v as u32)
}

fn entry(o: Raw) -> Option<Entry> {
    let a = parse_address(&text(o.url)).ok()?;
    let gps = parse_gps(&text(o.gps)).filter(|&(a, b)| !(a == 0.0 && b == 0.0));
    let users_max = count(o.users_max.as_ref());
    let users = count(o.users.as_ref());
    let snr = o
        .snr
        .as_ref()
        .and_then(Loose::number)
        .filter(|v| *v > 0.0 && *v < 100.0);
    let bands = text(o.bands).split_once('-').and_then(|(a, b)| {
        let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
        (a >= 0.0 && b > a).then_some((a, b))
    });
    let offline = match o.offline {
        Some(Loose::Bool(b)) => b,
        Some(Loose::Text(t)) => matches!(t.to_ascii_lowercase().as_str(), "yes" | "true" | "1"),
        Some(Loose::Number(n)) => n == 1.0,
        _ => false,
    };
    Some(Entry {
        name: text(o.name),
        host: a.host,
        port: a.port,
        location: text(o.loc),
        antenna: text(o.antenna),
        sdr_hw: text(o.sdr_hw),
        gps,
        users,
        users_max,
        snr,
        bands,
        offline,
        id: text(o.id),
    })
}

/// Reads a listing: a JSON array of objects, or the same array inside a JavaScript file (parsing starts at the first `[`). A bad
/// entry is dropped. Text that is not a listing returns an empty list.
pub fn parse_list(input: &str) -> Vec<Entry> {
    let Some(start) = input.find('[') else {
        return Vec::new();
    };
    let s = &input[start..];
    if !depth_ok(s.as_bytes()) {
        return Vec::new();
    }
    let s = if s.len() > STEP_BUDGET {
        let mut end = STEP_BUDGET;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    } else {
        s
    };
    let Some(Ok(items)) = serde_json::Deserializer::from_str(s)
        .into_iter::<Vec<Item>>()
        .next()
    else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|v| match v {
            Item::Entry(r) => entry(*r),
            Item::Other(_) => None,
        })
        .take(MAX_ENTRIES)
        .collect()
}

/// The great-circle distance in km.
pub fn distance_km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, lo1, la2, lo2) = (
        a.0.to_radians(),
        a.1.to_radians(),
        b.0.to_radians(),
        b.1.to_radians(),
    );
    let h = ((la2 - la1) / 2.0).sin().powi(2)
        + la1.cos() * la2.cos() * ((lo2 - lo1) / 2.0).sin().powi(2);
    2.0 * EARTH_KM * h.sqrt().min(1.0).asin()
}

/// Sorts by distance from `from`. Receivers without a position go last, in their order.
pub fn sort_by_distance(list: &mut [Entry], from: (f64, f64)) {
    list.sort_by(|a, b| match (a.gps, b.gps) {
        (Some(x), Some(y)) => distance_km(x, from).total_cmp(&distance_km(y, from)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = r#"var kiwisdr_com = [
        {"name": "London", "url": "http://kiwi.example.org:8074", "users": "1", "users_max": "4", "gps": "(51.5, -0.12)",
         "snr": "32,28", "extra": {"nested": [1, 2, {"x": 3}]}},
        {"name": "no url"},
        {"name": "full", "url": "http://10.1.2.3", "users": 4, "users_max": 4, "offline": "yes"},
        {"name": "bad", "url": "http://a b:80"},
        {"name": "v6", "url": "http://[2001:db8::1]:8073/", "bands": "100000-30000000"}
    ];"#;

    #[test]
    fn the_listing_is_read_and_sorted() {
        let mut l = parse_list(LIST);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].port, 8074);
        assert_eq!(l[0].snr, Some(32.0));
        assert!(!l[0].full());
        assert_eq!((l[1].port, l[1].full(), l[1].offline), (8073, true, true));
        assert_eq!(l[2].host, "2001:db8::1");
        assert_eq!(l[2].bands.unwrap().0, 100_000.0);
        sort_by_distance(&mut l, (55.0, 24.0));
        assert_eq!(l[0].name, "London");
        let d = distance_km(l[0].gps.unwrap(), (55.0, 24.0));
        assert!((d - 1700.0).abs() < 150.0, "{d}");
        assert_eq!(l[2].gps, None);
    }

    #[test]
    fn garbage_returns_nothing_quickly() {
        for g in ["", "not json", "[", "[{", "[1,2]"] {
            assert!(parse_list(g).is_empty(), "{g}");
        }
        let t = std::time::Instant::now();
        assert!(parse_list(&"[".repeat(200_000)).is_empty());
        assert!(t.elapsed() < std::time::Duration::from_secs(1));
    }
}
