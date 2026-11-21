//! The trail of an aircraft: what each point carries beside its position ([`TrailMark`]), and one packed layout of a trail
//! for a table cell ([`encode`], [`decode`], [`TRAIL`]).

use crate::error::{Error, Result};
use sdrkit_core::publish::{field, Field};
use sdrkit_core::ContentType;
use sdrkit_util_geo::Position;
use sdrkit_util_objects::TrailPoint;
use serde::{Deserialize, Serialize};

/// What a trail point carries beside its position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrailMark {
    /// The barometric altitude, feet, when known.
    pub alt_ft: Option<f64>,
    /// The aircraft was on the ground.
    #[serde(default)]
    pub on_ground: bool,
}

/// The packed trail of a row ([`encode`]).
pub const TRAIL: Field<field::Bytes> =
    Field::new("trail", "Trail", "").content(ContentType::Binary);

/// The tag of a full point.
const FULL: [u8; 2] = [0x80, 0x00];
/// Units of latitude in half a circle, and of longitude in a whole circle.
const LAT_UNITS: f64 = 8_388_608.0;
const LON_UNITS: f64 = 16_777_216.0;

/// Writes `points` into `out` in the packed layout, after clearing it.
///
/// The first point, and a point too far from the one before, is the two bytes 0x80 0x00 and then 8 bytes: latitude and
/// longitude as 24-bit fractions of half a circle and a whole circle, then the altitude in 100 ft units in 14 bits, the
/// "altitude known" bit and the on-ground bit. Every other point is 5 bytes: the latitude and longitude steps in those units,
/// 16 bits each, then the altitude step in 6 bits and the two bits. Numbers are big-endian and two's complement.
pub fn encode<'a>(points: impl IntoIterator<Item = &'a TrailPoint<TrailMark>>, out: &mut Vec<u8>) {
    out.clear();
    let mut prev: Option<(i32, i32, i32)> = None;
    for p in points {
        let la = (p.position.lat_deg / 180.0 * LAT_UNITS).round() as i32;
        let lo = (p.position.lon_deg / 360.0 * LON_UNITS).round() as i32;
        let al = p
            .extra
            .alt_ft
            .map_or(0, |a| ((a / 100.0).trunc() as i32).clamp(-3000, 3000));
        let flags = (i32::from(p.extra.alt_ft.is_some()) << 1) | i32::from(p.extra.on_ground);
        let step = prev
            .map(|(a, b, c)| (la - a, lo - b, al - c))
            .filter(|d| d.0.abs() < 32767 && d.1.abs() < 32767 && d.2.abs() <= 31);
        match step {
            Some((dla, dlo, dal)) => out.extend_from_slice(&[
                (dla >> 8) as u8,
                dla as u8,
                (dlo >> 8) as u8,
                dlo as u8,
                (((dal & 0x3F) << 2) | flags) as u8,
            ]),
            None => {
                let a = ((al & 0x3FFF) << 2) | flags;
                let (la24, lo24) = (la & 0xFF_FFFF, lo & 0xFF_FFFF);
                out.extend_from_slice(&FULL);
                out.extend_from_slice(&[
                    (la24 >> 16) as u8,
                    (la24 >> 8) as u8,
                    la24 as u8,
                    (lo24 >> 16) as u8,
                    (lo24 >> 8) as u8,
                    lo24 as u8,
                    (a >> 8) as u8,
                    a as u8,
                ]);
            }
        }
        prev = Some((la, lo, al));
    }
}

/// Reads a trail that [`encode`] wrote into `out`, after clearing it. The altitude comes back in whole 100 ft.
///
/// # Errors
///
/// [`Error::Short`] if the bytes end inside a point, or the first point is not a full point.
pub fn decode(bytes: &[u8], out: &mut Vec<(Position, TrailMark)>) -> Result<()> {
    out.clear();
    let sx = |v: i32, b: u32| if v >= 1 << (b - 1) { v - (1 << b) } else { v };
    let (mut la, mut lo, mut al) = (0i32, 0i32, 0i32);
    let mut rest = bytes;
    while !rest.is_empty() {
        let flags;
        if rest.starts_with(&FULL) {
            let c = rest.get(2..10).ok_or(Error::Short)?;
            la = sx(
                i32::from(c[0]) << 16 | i32::from(c[1]) << 8 | i32::from(c[2]),
                24,
            );
            lo = sx(
                i32::from(c[3]) << 16 | i32::from(c[4]) << 8 | i32::from(c[5]),
                24,
            );
            let a = i32::from(c[6]) << 8 | i32::from(c[7]);
            al = sx(a >> 2, 14);
            flags = a & 3;
            rest = &rest[10..];
        } else {
            if out.is_empty() {
                return Err(Error::Short);
            }
            let c = rest.get(..5).ok_or(Error::Short)?;
            la += sx(i32::from(c[0]) << 8 | i32::from(c[1]), 16);
            lo += sx(i32::from(c[2]) << 8 | i32::from(c[3]), 16);
            al += sx(i32::from(c[4] >> 2), 6);
            flags = i32::from(c[4] & 3);
            rest = &rest[5..];
        }
        out.push((
            Position::new(
                f64::from(la) * 180.0 / LAT_UNITS,
                f64::from(lo) * 360.0 / LON_UNITS,
            ),
            TrailMark {
                alt_ft: (flags & 2 != 0).then_some(f64::from(al) * 100.0),
                on_ground: flags & 1 != 0,
            },
        ));
    }
    Ok(())
}
