//! Position reports: the uncompressed and compressed forms (APRS 1.0.1 chapters 8 and 9), position ambiguity, the data
//! extensions (chapter 7), the altitude in a comment, the DAO extension of APRS 1.2 (a datum and extra digits) and the base-91
//! comment telemetry of APRS 1.2.

use super::telemetry::Telemetry;
use super::weather::Weather;
use super::{text, Symbol};
use crate::Param;
use sdrkit_util_geo::Position;

/// A data extension: seven bytes after the symbol code (chapter 7).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Extension {
    /// `PHGphgd`: power, antenna height above average terrain, gain and directivity.
    Phg(Station),
    /// `DFSshgd`: an omni direction-finding report. The power digit is the received S-units (0 to 9).
    Dfs(Station),
    /// `RNGrrrr`: the station's radio range, in miles.
    Range {
        /// Range, statute miles.
        miles: f64,
    },
    /// `Tyy/Cxx`: an area object's shape and colour (chapter 11, "area objects"). The two `T` digits are read as
    /// one two-digit number.
    Area {
        /// The two digits after `T`: the area type.
        kind: u8,
        /// The two digits after `C`: the colour.
        colour: u8,
    },
}

/// The PHG and DFS values (chapter 7).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Station {
    /// PHG: power in watts (the digit squared). DFS: the signal strength in S-units (the digit).
    pub power: f64,
    /// Antenna height above average terrain, feet (10 times 2 to the power of the digit).
    pub height_ft: f64,
    /// Antenna gain, dB.
    pub gain_db: u8,
    /// Directivity, degrees (0 is omnidirectional).
    pub directivity_deg: u16,
}

/// A direction-finding bearing after the course and speed: `/BRG/NRQ` (chapter 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bearing {
    /// Bearing, degrees.
    pub bearing_deg: u16,
    /// Number of hits (N), range (R) and quality (Q), as the three digits.
    pub nrq: [u8; 3],
}

/// The position part of a position report, an object or an item.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Located {
    /// Latitude and longitude. `alt_m` is 0, the altitude, if sent, is in `altitude_m`.
    pub position: Position,
    /// The map symbol.
    pub symbol: Symbol,
    /// Position ambiguity, 0 to 4 (chapter 8). If it is above 0, `position` is the centre of the box the sender left blank.
    pub ambiguity: u8,
    /// The size of that box, degrees of latitude (0.01 minute when there is no ambiguity).
    pub resolution_deg: f64,
    /// The position was in the compressed form.
    pub compressed: bool,
    /// The compression type byte of a compressed position, less 33 (chapter 9, table "compression type").
    pub compression_type: Option<u8>,
    /// Course, degrees (1 to 360). The value 0 means "not known".
    pub course_deg: Option<u16>,
    /// Speed, knots.
    pub speed_kn: Option<f64>,
    /// Altitude, metres.
    pub altitude_m: Option<f64>,
    /// A data extension.
    pub extension: Option<Extension>,
    /// A direction-finding bearing.
    pub bearing: Option<Bearing>,
    /// The datum character of a DAO extension (`W` or `w` is WGS-84).
    pub datum: Option<char>,
    /// Weather values, when the symbol is the weather station `_`.
    pub weather: Option<Weather>,
    /// Base-91 telemetry from the comment.
    pub telemetry: Option<Telemetry>,
    /// The comment, without the parts read above.
    pub comment: String,
}

/// The box size in minutes for each ambiguity level.
const BOX_MIN: [f64; 5] = [0.01, 0.1, 1.0, 10.0, 60.0];

fn digit_or_blank(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b' ' => Some(0),
        _ => None,
    }
}

/// Reads `DDMM.hhN/DDDMM.hhW$` (19 bytes, chapter 8). Blanks give the ambiguity.
pub(crate) fn uncompressed(b: &[u8]) -> Option<Located> {
    if b.len() < 19 || b[4] != b'.' || b[14] != b'.' {
        return None;
    }
    let ns = b[7].to_ascii_uppercase();
    let ew = b[17].to_ascii_uppercase();
    if !matches!(ns, b'N' | b'S') || !matches!(ew, b'E' | b'W') {
        return None;
    }
    // The ambiguity counts the blanks from the right of the latitude: hundredths, tenths, minute units, minute tens.
    let mut ambiguity = 0u8;
    for &i in &[6usize, 5, 3, 2] {
        if b[i] != b' ' {
            break;
        }
        ambiguity += 1;
    }
    let lat_digits = [b[0], b[1], b[2], b[3], b[5], b[6]];
    let blanks = lat_digits.iter().filter(|&&c| c == b' ').count();
    if blanks != ambiguity as usize {
        return None;
    }
    let mut v = [0u8; 6];
    for (o, &c) in v.iter_mut().zip(&lat_digits) {
        *o = digit_or_blank(c)?;
    }
    let lon_digits = [b[9], b[10], b[11], b[12], b[13], b[15], b[16]];
    let mut w = [0u8; 7];
    for (o, &c) in w.iter_mut().zip(&lon_digits) {
        *o = digit_or_blank(c)?;
    }
    let lat_deg = (v[0] * 10 + v[1]) as f64;
    let lat_min = (v[2] * 10 + v[3]) as f64 + (v[4] * 10 + v[5]) as f64 / 100.0;
    let lon_deg = (w[0] as u32 * 100 + w[1] as u32 * 10 + w[2] as u32) as f64;
    let lon_min = (w[3] * 10 + w[4]) as f64 + (w[5] * 10 + w[6]) as f64 / 100.0;
    if lat_deg > 90.0 || lat_min >= 60.0 || lon_deg > 180.0 || lon_min >= 60.0 {
        return None;
    }
    // The centre of the box the blanks leave. The longitude takes the latitude's ambiguity (chapter 8).
    let step = BOX_MIN[ambiguity as usize];
    let half = if ambiguity > 0 { step / 2.0 } else { 0.0 };
    let mut lat = lat_deg + (lat_min + half) / 60.0;
    let lon_box = if ambiguity > 0 {
        (lon_min / step).floor() * step
    } else {
        lon_min
    };
    let mut lon = lon_deg + (lon_box + half) / 60.0;
    if lat > 90.0 {
        lat = 90.0;
    }
    if ns == b'S' {
        lat = -lat;
    }
    if ew == b'W' {
        lon = -lon;
    }
    Some(Located {
        position: Position {
            lat_deg: lat,
            lon_deg: lon,
            alt_m: 0.0,
        },
        symbol: Symbol {
            table: b[8] as char,
            code: b[18] as char,
        },
        ambiguity,
        resolution_deg: BOX_MIN[ambiguity as usize] / 60.0,
        ..Default::default()
    })
}

/// The value of base-91 characters (`!` is 0, `{` is 90).
pub(crate) fn base91(s: &[u8]) -> Option<u32> {
    let mut v = 0u32;
    for &c in s {
        if !(33..=123).contains(&c) {
            return None;
        }
        v = v * 91 + (c - 33) as u32;
    }
    Some(v)
}

/// Reads a compressed position `/YYYYXXXX$csT` (13 bytes, chapter 9).
pub(crate) fn compressed(b: &[u8]) -> Option<Located> {
    if b.len() < 13 {
        return None;
    }
    let table = b[0];
    // The table is `/`, `\`, an overlay letter A-Z, or a-j (an overlay digit 0-9 in the compressed form).
    if !(table == b'/'
        || table == b'\\'
        || table.is_ascii_uppercase()
        || (b'a'..=b'j').contains(&table))
    {
        return None;
    }
    let y = base91(&b[1..5])? as f64;
    let x = base91(&b[5..9])? as f64;
    let lat = 90.0 - y / 380_926.0;
    let lon = -180.0 + x / 190_463.0;
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let mut l = Located {
        position: Position {
            lat_deg: lat,
            lon_deg: lon,
            alt_m: 0.0,
        },
        symbol: Symbol {
            // An overlay digit is sent as a-j.
            table: if (b'a'..=b'j').contains(&table) {
                (table - b'a' + b'0') as char
            } else {
                table as char
            },
            code: b[9] as char,
        },
        compressed: true,
        resolution_deg: 1.0 / 380_926.0,
        ..Default::default()
    };
    let (c, s, t) = (b[10], b[11], b[12]);
    if c != b' ' {
        let tt = t.wrapping_sub(33);
        l.compression_type = Some(tt);
        if (tt >> 3) & 3 == 2 {
            // The NMEA source was GGA: cs is the altitude, 1.002^cs feet.
            let cs = base91(&[c, s])?;
            l.altitude_m = Some(1.002f64.powi(cs as i32) * 0.3048);
        } else if (b'!'..=b'z').contains(&c) {
            l.course_deg = Some((c - 33) as u16 * 4);
            l.speed_kn = Some(1.08f64.powi(s.checked_sub(33)? as i32) - 1.0);
        } else if c == b'{' {
            l.extension = Some(Extension::Range {
                miles: 2.0 * 1.08f64.powi(s.checked_sub(33)? as i32),
            });
        }
    }
    Some(l)
}

fn three_digits(b: &[u8]) -> Option<u16> {
    if b.len() == 3 && b.iter().all(u8::is_ascii_digit) {
        Some(b.iter().fold(0u16, |a, &c| a * 10 + (c - b'0') as u16))
    } else {
        None
    }
}

fn station(b: &[u8]) -> Option<Station> {
    // b holds the four characters p h g d.
    let p = b[0].checked_sub(b'0').filter(|v| *v <= 9)?;
    let h = b[1].checked_sub(b'0').filter(|v| *v < 64)?;
    let g = b[2].checked_sub(b'0').filter(|v| *v <= 9)?;
    let d = b[3].checked_sub(b'0').filter(|v| *v <= 8)?;
    Some(Station {
        power: p as f64,
        height_ft: 10.0 * 2f64.powi(h as i32),
        gain_db: g,
        directivity_deg: d as u16 * 45,
    })
}

/// Reads the data after the symbol code of `l`: an extension, weather (symbol `_`), the comment and what it holds.
pub(crate) fn after(rest: &[u8], l: &mut Located) {
    let mut rest = rest;
    let weather_symbol = l.symbol.code == '_';
    let digit = |x: u8| x.checked_sub(b'0').filter(|v| *v <= 9);
    if rest.len() >= 7 && rest[0] == b'T' && rest[3] == b'/' && rest[4] == b'C' {
        if let (Some(k1), Some(k2), Some(c1), Some(c2)) = (
            digit(rest[1]),
            digit(rest[2]),
            digit(rest[5]),
            digit(rest[6]),
        ) {
            l.extension = Some(Extension::Area {
                kind: k1 * 10 + k2,
                colour: c1 * 10 + c2,
            });
            rest = &rest[7..];
        }
    } else if !l.compressed && rest.len() >= 7 && rest[3] == b'/' {
        let c = &rest[..3];
        let s = &rest[4..7];
        let blank = |x: &[u8]| x.iter().all(|&v| v == b'.' || v == b' ');
        if weather_symbol && (three_digits(c).is_some() || blank(c)) {
            // Wind direction and speed in the course and speed places (chapter 12).
            let mut w = Weather {
                wind_dir_deg: three_digits(c),
                wind_mph: three_digits(s).map(f64::from),
                ..Default::default()
            };
            let tail = w.read_fields(&rest[7..], false);
            l.weather = Some(w);
            l.comment = text(tail);
            return;
        }
        if let (Some(cv), Some(sv)) = (three_digits(c), three_digits(s)) {
            if cv <= 360 {
                l.course_deg = Some(cv);
                l.speed_kn = Some(sv as f64);
                rest = &rest[7..];
                // A DF bearing: /BRG/NRQ.
                if rest.len() >= 8 && rest[0] == b'/' && rest[4] == b'/' {
                    if let (Some(brg), true) = (
                        three_digits(&rest[1..4]),
                        rest[5..8].iter().all(u8::is_ascii_digit),
                    ) {
                        l.bearing = Some(Bearing {
                            bearing_deg: brg,
                            nrq: [rest[5] - b'0', rest[6] - b'0', rest[7] - b'0'],
                        });
                        rest = &rest[8..];
                    }
                }
            }
        } else if blank(c) && blank(s) {
            rest = &rest[7..];
        }
    } else if rest.len() >= 7 && (rest.starts_with(b"PHG") || rest.starts_with(b"DFS")) {
        if let Some(st) = station(&rest[3..7]) {
            l.extension = Some(if rest[0] == b'P' {
                Extension::Phg(Station {
                    power: st.power * st.power,
                    ..st
                })
            } else {
                Extension::Dfs(st)
            });
            rest = &rest[7..];
        }
    } else if rest.len() >= 7
        && rest.starts_with(b"RNG")
        && rest[3..7].iter().all(u8::is_ascii_digit)
    {
        let v = rest[3..7]
            .iter()
            .fold(0u32, |a, &c| a * 10 + (c - b'0') as u32);
        l.extension = Some(Extension::Range { miles: v as f64 });
        rest = &rest[7..];
    }
    if weather_symbol
        && rest
            .first()
            .is_some_and(|c| matches!(c, b'c' | b'g' | b't' | b's' | b'r'))
    {
        let mut w = Weather::default();
        rest = w.read_fields(rest, false);
        l.weather = Some(w);
    }
    let mut comment = text(rest);
    if let Some(a) = take_altitude(&mut comment) {
        l.altitude_m = Some(a);
    }
    if let Some((datum, dlat, dlon)) = take_dao(&mut comment) {
        l.datum = Some(datum);
        if !l.compressed && l.ambiguity == 0 {
            let s = |v: f64| if v < 0.0 { -1.0 } else { 1.0 };
            l.position.lat_deg += s(l.position.lat_deg) * dlat / 60.0;
            l.position.lon_deg += s(l.position.lon_deg) * dlon / 60.0;
            l.resolution_deg = if datum.is_ascii_lowercase() {
                0.0001 / 60.0
            } else {
                0.001 / 60.0
            };
        }
    }
    if let Some(t) = take_comment_telemetry(&mut comment) {
        l.telemetry = Some(t);
    }
    l.comment = comment;
}

/// Takes `/A=dddddd` (feet, six digits, or a minus sign and five) out of a comment. Returns metres.
pub(crate) fn take_altitude(comment: &mut String) -> Option<f64> {
    let i = comment.find("/A=")?;
    let v = comment.get(i + 3..i + 9)?;
    let b = v.as_bytes();
    let ok = (b[0] == b'-' || b[0].is_ascii_digit()) && b[1..].iter().all(u8::is_ascii_digit);
    if !ok {
        return None;
    }
    let ft: f64 = v.parse().ok()?;
    comment.replace_range(i..i + 9, "");
    Some(ft * 0.3048)
}

/// Takes the DAO extension `!DAO!` (APRS 1.2, "Datum and extra position resolution") out of a comment. Returns the datum
/// character and the extra latitude and longitude, in minutes. An upper-case datum sends one more decimal digit of the minutes as
/// two digits. A lower-case datum sends two more as base-91 characters (value times 1.1, so 0 to 99 ten-thousandths).
pub(crate) fn take_dao(comment: &mut String) -> Option<(char, f64, f64)> {
    let b = comment.as_bytes();
    let i = (0..b.len().saturating_sub(4))
        .rev()
        .find(|&i| b[i] == b'!' && b[i + 4] == b'!')?;
    let (d, a, o) = (b[i + 1], b[i + 2], b[i + 3]);
    let r = if d.is_ascii_uppercase() {
        let f = |c: u8| match c {
            b'0'..=b'9' => Some((c - b'0') as f64 * 0.001),
            b' ' => Some(0.0),
            _ => None,
        };
        (d as char, f(a)?, f(o)?)
    } else if d.is_ascii_lowercase() {
        let f = |c: u8| match c {
            b'!'..=b'{' => Some(((c - 33) as f64 * 1.1).round() * 0.0001),
            b' ' => Some(0.0),
            _ => None,
        };
        (d as char, f(a)?, f(o)?)
    } else {
        return None;
    };
    comment.replace_range(i..i + 5, "");
    Some(r)
}

/// Takes base-91 comment telemetry `|ssaa...|` (APRS 1.2, "base-91 telemetry in comments") out of a comment: a sequence number,
/// 1 to 5 analog values and an optional digital byte, two base-91 characters each.
pub(crate) fn take_comment_telemetry(comment: &mut String) -> Option<Telemetry> {
    let b = comment.as_bytes();
    let start = b.iter().position(|&c| c == b'|')?;
    let len = b[start + 1..].iter().position(|&c| c == b'|')?;
    let body = &b[start + 1..start + 1 + len];
    if !(4..=14).contains(&len) || len % 2 != 0 {
        return None;
    }
    let mut vals = [0u32; 7];
    for (k, pair) in body.chunks(2).enumerate() {
        if !pair.iter().all(|c| (33..=123).contains(c)) {
            return None;
        }
        vals[k] = base91(pair)?;
    }
    let n = len / 2;
    let mut t = Telemetry {
        sequence: vals[0].to_string(),
        ..Default::default()
    };
    let analog = (n - 1).min(5);
    for k in 0..analog {
        t.analog[k] = Some(vals[1 + k] as f64);
    }
    if n == 7 {
        t.digital = Some((vals[6] & 0xFF) as u8);
    }
    comment.replace_range(start..start + len + 2, "");
    Some(t)
}

/// Appends an uncompressed position `DDMM.hhN/DDDMM.hhW$` (chapter 8). The minutes are rounded to hundredths, with the carry
/// (59.999' becomes the next degree). `ambiguity` (0 to 4) blanks that many digits from the right of both minutes fields.
pub fn encode_uncompressed(
    position: &Position,
    symbol: Symbol,
    ambiguity: u8,
    out: &mut String,
) -> crate::Result<()> {
    use sdrkit_util_geo::coord::{format_ddmm, Axis};
    if ambiguity > 4 {
        return Err(crate::Error::BadParameter(Param::Ambiguity));
    }
    let bad = |_| crate::Error::BadParameter(Param::Position);
    let (lat, ns) = format_ddmm(position.lat_deg, Axis::Latitude, 2).map_err(bad)?;
    let (lon, ew) = format_ddmm(position.lon_deg, Axis::Longitude, 2).map_err(bad)?;
    let blank = |s: &str, out: &mut String| {
        // The digit places blanked, from the right: hundredths, tenths, minute units, minute tens.
        let n = s.len();
        let places = [n - 1, n - 2, n - 4, n - 5];
        for (i, c) in s.chars().enumerate() {
            let hidden = places[..ambiguity as usize].contains(&i);
            out.push(if hidden { ' ' } else { c });
        }
    };
    blank(lat.as_str(), out);
    out.push(ns);
    out.push(symbol.table);
    blank(lon.as_str(), out);
    out.push(ew);
    out.push(symbol.code);
    Ok(())
}

/// Appends a compressed position `/YYYYXXXX$csT` (chapter 9). The overlay digits 0 to 9 are sent as `a` to `j`. With
/// `course_speed` (degrees, knots) the `cs` bytes carry them. Otherwise they are blanks. `compression_type` is the `T` byte
/// less 33 (0 to 63). The grid values are truncated, as the chapter's examples show.
pub fn encode_compressed(
    position: &Position,
    symbol: Symbol,
    course_speed: Option<(u16, f64)>,
    compression_type: u8,
    out: &mut String,
) -> crate::Result<()> {
    let (lat, lon) = (position.lat_deg, position.lon_deg);
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) || compression_type > 63 {
        return Err(crate::Error::BadParameter(Param::CompressedPosition));
    }
    let digits = |v: f64, out: &mut String| {
        // A small allowance keeps an exact value from falling one step below itself.
        let mut n = ((v + 1e-6).floor() as u64).min(91u64.pow(4) - 1);
        let mut d = [0u8; 4];
        for k in (0..4).rev() {
            d[k] = (n % 91) as u8 + 33;
            n /= 91;
        }
        out.extend(d.iter().map(|&c| c as char));
    };
    out.push(match symbol.table {
        c @ '0'..='9' => (c as u8 - b'0' + b'a') as char,
        c => c,
    });
    digits(380_926.0 * (90.0 - lat), out);
    digits(190_463.0 * (180.0 + lon), out);
    out.push(symbol.code);
    match course_speed {
        Some((c, s)) => {
            if c > 360 || !(0.0..=1e4).contains(&s) {
                return Err(crate::Error::BadParameter(Param::CourseOrSpeed));
            }
            out.push(((c % 360) / 4 + 33) as u8 as char);
            let k = ((s + 1.0).ln() / 1.08f64.ln()).round().clamp(0.0, 89.0) as u8;
            out.push((k + 33) as char);
        }
        None => out.push_str("  "),
    }
    out.push((compression_type + 33) as char);
    Ok(())
}
