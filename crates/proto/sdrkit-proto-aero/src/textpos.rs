//! Positions written as text in ACARS messages.
//!
//! Three forms are read. The airline formats behind them are not public standards.
//!
//! | Form | Example | Where |
//! |---|---|---|
//! | [`Form::PosReport`] | `POSN43312W123174,EASON,215754,370` | `POS` or `/PS` anywhere in the text: latitude DDMMm, longitude DDDMMm (m is tenths of a minute), then an optional waypoint, the time HHMMSS and the flight level |
//! | [`Form::Thousandths`] | `POSN25312W055214,,125000,350` | the same in label 20, where the digits are thousandths of a degree |
//! | [`Form::Decimal`] | `N 44.203,W 86.546,31965` | decimal degrees with hemisphere letters, after `POS` or at the start of labels 12, 16 and 20, then the altitude in feet (`GRD` for the ground) |
//!
//! Every position passes the same test ([`sane`]). A latitude over 90 degrees is refused. A longitude is wrapped into -180 to 180
//! degrees. The point 0, 0 is refused because broken reports give it. The readers do not allocate.
//!
//! ```
//! use sdrkit_proto_aero::textpos::{read, Form};
//! let p = read("H1", "POSN43312W123174,EASON,215754,370").unwrap();
//! assert!((p.position.lat_deg - 43.52).abs() < 1e-9);
//! assert_eq!((p.alt_ft, p.sec_of_day, p.form), (Some(37000.0), Some(79_074), Form::PosReport));
//! ```

use sdrkit_util_geo::position::wrap180;
use sdrkit_util_geo::Position;

/// The form a position was written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// `POS` or `/PS`, degrees and tenths of minutes.
    PosReport,
    /// `POS` in label 20, thousandths of a degree.
    Thousandths,
    /// Decimal degrees with hemisphere letters.
    Decimal,
}

/// A position read from a text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextPosition {
    /// The position. Its `alt_m` is 0 (see `alt_ft`).
    pub position: Position,
    /// The altitude in feet, when the text provides it.
    pub alt_ft: Option<f64>,
    /// The time of the report as seconds of the UTC day, when the text provides it.
    pub sec_of_day: Option<u32>,
    /// The form.
    pub form: Form,
}

/// The position test of every reader. It returns `None` for a latitude over 90 degrees, a longitude over 360 degrees, a value
/// that is not finite, and exactly 0, 0. Otherwise it returns the position with its longitude wrapped into -180 to 180 degrees.
pub fn sane(lat_deg: f64, lon_deg: f64) -> Option<Position> {
    if !lat_deg.is_finite() || !lon_deg.is_finite() || lat_deg.abs() > 90.0 || lon_deg.abs() > 360.0
    {
        return None;
    }
    let lon = wrap180(lon_deg);
    if lat_deg == 0.0 && lon == 0.0 {
        return None;
    }
    Some(Position::new(lat_deg, lon))
}

fn digits(s: &[u8], n: usize) -> Option<u32> {
    let d = s.get(..n)?;
    d.iter()
        .all(u8::is_ascii_digit)
        .then(|| d.iter().fold(0, |a, &c| a * 10 + (c - b'0') as u32))
}

fn hemisphere(c: Option<&u8>, pos: u8, neg: u8) -> Option<f64> {
    match c {
        Some(&c) if c == pos => Some(1.0),
        Some(&c) if c == neg => Some(-1.0),
        _ => None,
    }
}

/// `N43312W123174` (or with a space before the longitude): returns latitude, longitude and the bytes used. The digits are
/// degrees and tenths of minutes (DDMMm, DDDMMm), or thousandths of a degree with `thousandths`. `None` when the form does not
/// match or a minute value is 60 or more.
pub fn packed(s: &[u8], thousandths: bool) -> Option<(f64, f64, usize)> {
    let ns = hemisphere(s.first(), b'N', b'S')?;
    let la = digits(&s[1..], 5)?;
    let mut i = 6;
    if s.get(i) == Some(&b' ') {
        i += 1;
    }
    let ew = hemisphere(s.get(i), b'E', b'W')?;
    let lo = digits(s.get(i + 1..)?, 6)?;
    let (lat, lon) = if thousandths {
        (la as f64 / 1000.0, lo as f64 / 1000.0)
    } else {
        if la % 1000 >= 600 || lo % 1000 >= 600 {
            return None;
        }
        (
            (la / 1000) as f64 + (la % 1000) as f64 / 600.0,
            (lo / 1000) as f64 + (lo % 1000) as f64 / 600.0,
        )
    };
    Some((ns * lat, ew * lon, i + 7))
}

/// Digits, a point and digits at the start of `s`, after any spaces.
fn decimal(s: &[u8]) -> Option<(f64, usize)> {
    let skip = s.iter().take_while(|&&c| c == b' ').count();
    let t = &s[skip..];
    let i = t.iter().take_while(|c| c.is_ascii_digit()).count();
    if i == 0 || t.get(i) != Some(&b'.') {
        return None;
    }
    let j = i + 1 + t[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
    if j == i + 1 {
        return None;
    }
    let v = std::str::from_utf8(&t[..j]).ok()?.parse().ok()?;
    Some((v, skip + j))
}

/// `N 44.203,W 86.546`, `N 28.177/W 96.055`, `N33.123 W097.456`: returns latitude, longitude and the bytes used.
pub fn decimal_pair(s: &[u8]) -> Option<(f64, f64, usize)> {
    let ns = hemisphere(s.first(), b'N', b'S')?;
    let (la, n) = decimal(&s[1..])?;
    let mut p = 1 + n;
    while matches!(s.get(p), Some(b' ' | b',' | b'/')) {
        p += 1;
    }
    let ew = hemisphere(s.get(p), b'E', b'W')?;
    let (lo, n) = decimal(&s[p + 1..])?;
    Some((ns * la, ew * lo, p + 1 + n))
}

fn hhmmss(f: &str) -> Option<u32> {
    let b = f.as_bytes();
    if b.len() != 6 {
        return None;
    }
    let (h, m, s) = (digits(b, 2)?, digits(&b[2..], 2)?, digits(&b[4..], 2)?);
    (h < 24 && m < 60 && s < 60).then_some(h * 3600 + m * 60 + s)
}

/// The fields after a packed position: an optional waypoint, the time, the flight level of at most three digits.
fn report_fields(rest: &str) -> (Option<u32>, Option<f64>) {
    let mut f = rest
        .strip_prefix(',')
        .unwrap_or(rest)
        .split(',')
        .map(str::trim);
    let (a, b) = (f.next(), f.next());
    let (time, fl) = match (a.and_then(hhmmss), b.and_then(hhmmss)) {
        (Some(t), _) => (Some(t), b),
        (None, Some(t)) => (Some(t), f.next()),
        _ => (None, None),
    };
    let alt = fl
        .filter(|x| !x.is_empty() && x.len() <= 3 && x.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|x| x.parse::<f64>().ok())
        .map(|x| x * 100.0);
    (time, alt)
}

/// The altitude field after a decimal pair: feet, or `GRD` for the ground.
fn altitude_field(rest: &str) -> Option<f64> {
    let f = rest.strip_prefix(',')?.split(',').next()?.trim();
    if f == "GRD" {
        Some(0.0)
    } else {
        f.parse().ok().filter(|v: &f64| v.is_finite())
    }
}

/// The first position in a downlink text of `label`, or `None`.
pub fn read(label: &str, text: &str) -> Option<TextPosition> {
    let thousandths = label == "20";
    if matches!(label, "12" | "16" | "20") {
        let t = text.trim_start();
        if let Some((lat, lon, n)) = decimal_pair(t.as_bytes()) {
            return Some(TextPosition {
                position: sane(lat, lon)?,
                alt_ft: altitude_field(&t[n..]),
                sec_of_day: None,
                form: Form::Decimal,
            });
        }
    }
    let b = text.as_bytes();
    let mut from = 0;
    while from + 3 <= b.len() {
        let at = from
            + b[from..]
                .windows(3)
                .position(|w| w == b"POS" || w == b"/PS")?;
        from = at + 1;
        let s = &text[at + 3..];
        if let Some((lat, lon, n)) = packed(s.as_bytes(), thousandths) {
            let rest = &s[n..];
            if !(rest.is_empty() || rest.starts_with(',')) {
                continue;
            }
            let (sec_of_day, alt_ft) = report_fields(rest);
            return Some(TextPosition {
                position: sane(lat, lon)?,
                alt_ft,
                sec_of_day,
                form: if thousandths {
                    Form::Thousandths
                } else {
                    Form::PosReport
                },
            });
        }
        let st = s.trim_start();
        if let Some((lat, lon, n)) = decimal_pair(st.as_bytes()) {
            return Some(TextPosition {
                position: sane(lat, lon)?,
                alt_ft: altitude_field(&st[n..]),
                sec_of_day: None,
                form: Form::Decimal,
            });
        }
    }
    None
}
