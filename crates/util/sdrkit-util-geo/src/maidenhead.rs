//! Maidenhead locators (the IARU grid squares used by amateur radio), both directions, at 2, 4, 6, 8 or 10 characters.
//!
//! A locator is pairs of characters, longitude first within each pair. The functions take and return latitude first.
//!
//! | Pair | Characters | Divides the cell above into | Cell size (lon × lat) |
//! |---|---|---|---|
//! | field | `A`–`R` | 18 × 18 | 20° × 10° |
//! | square | `0`–`9` | 10 × 10 | 2° × 1° |
//! | subsquare | `a`–`x` | 24 × 24 | 5' × 2.5' |
//! | extended square | `0`–`9` | 10 × 10 | 30" × 15" |
//! | extended subsquare | `a`–`x` | 24 × 24 | 1.25" × 0.625" |
//!
//! Decoding ignores case; encoding writes fields in upper case and subsquares in lower case (`JO62qm`). Longitude 180° and
//! latitude 90° fall in the last cell.
//!
//! Standard: IARU Region 1 VHF conference (Brighton, 1980); IARU Region 1 VHF Managers Handbook for 8 characters.

use crate::error::finite;
use crate::{Error, Position, Text};
use crate::{Quantity, SyntaxFault};

/// A decoded locator: the cell it names.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Square {
    /// Latitude of the south edge, degrees.
    pub south_deg: f64,
    /// Longitude of the west edge, degrees.
    pub west_deg: f64,
    /// Height of the cell, degrees of latitude.
    pub lat_size_deg: f64,
    /// Width of the cell, degrees of longitude.
    pub lon_size_deg: f64,
    /// Number of characters in the locator (2, 4, 6, 8 or 10).
    pub chars: u8,
}

impl Square {
    /// The centre of the cell.
    pub fn centre(&self) -> Position {
        Position::new(
            self.south_deg + self.lat_size_deg / 2.0,
            self.west_deg + self.lon_size_deg / 2.0,
        )
    }

    /// Whether a position lies inside the cell (south and west edges included).
    pub fn contains(&self, p: &Position) -> bool {
        p.lat_deg >= self.south_deg
            && p.lat_deg < self.south_deg + self.lat_size_deg
            && p.lon_deg >= self.west_deg
            && p.lon_deg < self.west_deg + self.lon_size_deg
    }
}

/// The base of each pair: letters A–R, digits, letters A–X, digits, letters A–X.
const BASE: [u32; 5] = [18, 10, 24, 10, 24];

/// Decodes a locator of 2, 4, 6, 8 or 10 characters. Case is ignored; surrounding spaces are not allowed.
///
/// ```
/// use sdrkit_util_geo::maidenhead;
/// let sq = maidenhead::decode("FN31pr").unwrap();
/// let c = sq.centre();
/// assert!((c.lat_deg - 41.729).abs() < 0.001 && (c.lon_deg + 72.708).abs() < 0.001);
/// assert!(maidenhead::decode("FN3").is_err());
/// assert!(maidenhead::decode("SN31").is_err()); // fields stop at R
/// ```
pub fn decode(locator: &str) -> Result<Square, Error> {
    let b = locator.as_bytes();
    if b.is_empty() || !b.len().is_multiple_of(2) || b.len() > 10 {
        return Err(Error::Syntax(SyntaxFault::LocatorLength));
    }
    let (mut west, mut south) = (-180.0, -90.0);
    let (mut w, mut h) = (360.0, 180.0);
    for (pair, chunk) in b.chunks(2).enumerate() {
        let base = BASE[pair];
        w /= base as f64;
        h /= base as f64;
        let digit = |c: u8, at: usize| -> Result<u32, Error> {
            let v = if base == 10 {
                c.wrapping_sub(b'0') as u32
            } else {
                c.to_ascii_uppercase().wrapping_sub(b'A') as u32
            };
            if v < base {
                Ok(v)
            } else {
                Err(Error::BadChar { at })
            }
        };
        let x = digit(chunk[0], pair * 2)?;
        let y = digit(chunk[1], pair * 2 + 1)?;
        west += x as f64 * w;
        south += y as f64 * h;
    }
    Ok(Square {
        south_deg: south,
        west_deg: west,
        lat_size_deg: h,
        lon_size_deg: w,
        chars: b.len() as u8,
    })
}

/// The locator of `chars` characters (2, 4, 6, 8 or 10) of the cell that contains `p`.
///
/// ```
/// use sdrkit_util_geo::{maidenhead, Position};
/// let w1aw = Position::new(41.714_775, -72.727_260);
/// assert_eq!(maidenhead::encode(&w1aw, 6).unwrap().as_str(), "FN31pr");
/// assert_eq!(maidenhead::encode(&w1aw, 10).unwrap().as_str(), "FN31pr21rn");
/// ```
pub fn encode(p: &Position, chars: usize) -> Result<Text<10>, Error> {
    finite(&[p.lat_deg, p.lon_deg])?;
    if p.lat_deg.abs() > 90.0 {
        return Err(Error::OutOfRange(Quantity::Latitude));
    }
    if p.lon_deg.abs() > 180.0 {
        return Err(Error::OutOfRange(Quantity::Longitude));
    }
    if !matches!(chars, 2 | 4 | 6 | 8 | 10) {
        return Err(Error::OutOfRange(Quantity::LocatorLength));
    }
    // Work in units of the smallest cell so every step is an exact integer division (no floating-point drift between pairs):
    // 18·10·24·10·24 = 1 036 800 cells of longitude, the same of latitude.
    const CELLS: f64 = 1_036_800.0;
    let x =
        (((p.lon_deg + 180.0) / 360.0 * CELLS).floor() as i64).clamp(0, CELLS as i64 - 1) as u32;
    let y = (((p.lat_deg + 90.0) / 180.0 * CELLS).floor() as i64).clamp(0, CELLS as i64 - 1) as u32;
    let mut out = Text::new();
    let mut div = CELLS as u32;
    for (pair, &base) in BASE.iter().enumerate().take(chars / 2) {
        div /= base;
        let (dx, dy) = ((x / div) % base, (y / div) % base);
        let ch = |d: u32| -> char {
            match pair {
                0 => (b'A' + d as u8) as char,
                1 | 3 => (b'0' + d as u8) as char,
                _ => (b'a' + d as u8) as char,
            }
        };
        out.push(ch(dx))?;
        out.push(ch(dy))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_locators() {
        // ARRL headquarters, W1AW, Newington CT, is FN31pr.
        let w1aw = Position::new(41.714_775, -72.727_260);
        assert_eq!(encode(&w1aw, 6).unwrap(), "FN31pr");
        assert_eq!(encode(&w1aw, 4).unwrap(), "FN31");
        assert_eq!(encode(&w1aw, 2).unwrap(), "FN");
        // Berlin is JO62qm.
        assert_eq!(encode(&Position::new(52.52, 13.405), 6).unwrap(), "JO62qm");
        // The corners of the grid.
        assert_eq!(
            encode(&Position::new(-90.0, -180.0), 10).unwrap(),
            "AA00aa00aa"
        );
        assert_eq!(
            encode(&Position::new(90.0, 180.0), 10).unwrap(),
            "RR99xx99xx"
        );
        // Greenwich, just east of the meridian on the equator side: IO91 / JO01.
        assert_eq!(encode(&Position::new(51.4779, 0.0), 4).unwrap(), "JO01");
        assert_eq!(encode(&Position::new(51.4779, -0.0001), 4).unwrap(), "IO91");
    }

    #[test]
    fn decoding_returns_the_cell() {
        let s = decode("JO62qm").unwrap();
        assert_eq!(s.chars, 6);
        assert!((s.west_deg - (12.0 + 16.0 * 5.0 / 60.0)).abs() < 1e-12);
        assert!((s.south_deg - (52.0 + 12.0 * 2.5 / 60.0)).abs() < 1e-12);
        assert!(
            (s.lon_size_deg - 5.0 / 60.0).abs() < 1e-15
                && (s.lat_size_deg - 2.5 / 60.0).abs() < 1e-15
        );
        let s10 = decode("fn31PR21RN").unwrap();
        assert!((s10.lon_size_deg - 1.25 / 3600.0).abs() < 1e-15);
        assert!(s10.contains(&Position::new(41.714_775, -72.727_260)));
        assert_eq!(decode("AA").unwrap().centre(), Position::new(-85.0, -170.0));
    }

    #[test]
    fn bad_locators_are_errors() {
        assert!(decode("").is_err());
        assert!(decode("J").is_err());
        assert!(decode("JO62qm1").is_err());
        assert!(decode("JO62qm12ab3").is_err());
        assert_eq!(decode("SA00"), Err(Error::BadChar { at: 0 }));
        assert_eq!(decode("JOA2"), Err(Error::BadChar { at: 2 }));
        assert_eq!(decode("JO62qy"), Err(Error::BadChar { at: 5 }));
        assert_eq!(decode("JO62qm1a"), Err(Error::BadChar { at: 7 }));
        assert_eq!(
            decode("JO62qm12a\u{e9}"),
            Err(Error::Syntax(SyntaxFault::LocatorLength))
        );
        assert!(encode(&Position::new(0.0, 0.0), 3).is_err());
        assert!(encode(&Position::new(91.0, 0.0), 6).is_err());
        assert!(encode(&Position::new(f64::NAN, 0.0), 6).is_err());
    }

    #[test]
    fn every_length_round_trips() {
        let mut s = 0x1234_5678_9abc_def0u64;
        for _ in 0..2_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let lat = -90.0 + 180.0 * ((s >> 11) as f64 / (1u64 << 53) as f64);
            let lon = -180.0 + 360.0 * (((s >> 3) & 0xffff_ffff) as f64 / 4_294_967_296.0);
            let p = Position::new(lat, lon);
            for n in [2, 4, 6, 8, 10] {
                let l = encode(&p, n).unwrap();
                let sq = decode(&l).unwrap();
                assert!(sq.contains(&p), "{p:?} {l}");
                assert_eq!(encode(&sq.centre(), n).unwrap(), l);
            }
        }
    }
}
