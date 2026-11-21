//! Compact Position Reporting (CPR), with the field widths and the zone span as parameters ([`CprFormat`]): the airborne, surface,
//! intent and coarse TIS-B formats of 1090 MHz extended squitter and the VDL Mode 4 format. All formats use NZ = 15.
//! [`decode_global`] returns [`Error::CprZoneMismatch`] when the even and odd latitudes have different longitude zone counts NL.
//!
//! Standard: RTCA DO-260B appendix A.1.7 (ICAO Annex 10 Vol. IV, Doc 9871); ICAO Doc 9816 Part 2, 1.4 for VDL Mode 4.

use crate::position::wrap180;
use crate::{Error, Position};
use crate::{Quantity, SyntaxFault};

/// The number of latitude zones per quadrant (NZ) of every CPR format in use.
pub const NZ: u32 = 15;

/// The field widths and zone span of a CPR variant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CprFormat {
    /// Bits of the latitude field.
    pub lat_bits: u32,
    /// Bits of the longitude field.
    pub lon_bits: u32,
    /// Span of the zone set: 360° for airborne-type formats, 90° for the surface format.
    pub span_deg: f64,
}

impl CprFormat {
    /// 1090 MHz extended squitter airborne position (DO-260B A.1.7): 17 bits, 360°.
    pub const AIRBORNE: CprFormat = CprFormat {
        lat_bits: 17,
        lon_bits: 17,
        span_deg: 360.0,
    };
    /// 1090 MHz extended squitter surface position: 17 bits sent of 19, which is 17 bits over 90° zones.
    pub const SURFACE: CprFormat = CprFormat {
        lat_bits: 17,
        lon_bits: 17,
        span_deg: 90.0,
    };
    /// Intent (DO-260B A.1.7.2, Nb = 14).
    pub const INTENT: CprFormat = CprFormat {
        lat_bits: 14,
        lon_bits: 14,
        span_deg: 360.0,
    };
    /// Coarse TIS-B airborne position (DO-260B A.1.7.2, Nb = 12).
    pub const TISB_COARSE: CprFormat = CprFormat {
        lat_bits: 12,
        lon_bits: 12,
        span_deg: 360.0,
    };
    /// VDL Mode 4 synchronisation burst (ICAO Doc 9816): 12-bit latitude, 14-bit longitude.
    pub const VDL4: CprFormat = CprFormat {
        lat_bits: 12,
        lon_bits: 14,
        span_deg: 360.0,
    };

    /// A format with other widths (1 to 30 bits) and a span of 360° or 90°.
    pub fn new(lat_bits: u32, lon_bits: u32, span_deg: f64) -> Result<CprFormat, Error> {
        if !(1..=30).contains(&lat_bits) || !(1..=30).contains(&lon_bits) {
            return Err(Error::OutOfRange(Quantity::CprFieldWidth));
        }
        if span_deg != 360.0 && span_deg != 90.0 {
            return Err(Error::OutOfRange(Quantity::CprZoneSpan));
        }
        Ok(CprFormat {
            lat_bits,
            lon_bits,
            span_deg,
        })
    }

    fn lat_scale(&self) -> f64 {
        (1u64 << self.lat_bits) as f64
    }
    fn lon_scale(&self) -> f64 {
        (1u64 << self.lon_bits) as f64
    }
    fn dlat(&self, odd: bool) -> f64 {
        self.span_deg / (4 * NZ - odd as u32) as f64
    }
    fn check(&self, f: &CprFrame) -> Result<(), Error> {
        if (f.lat as u64) >> self.lat_bits != 0 || (f.lon as u64) >> self.lon_bits != 0 {
            return Err(Error::OutOfRange(Quantity::CprFieldWiderThanFormat));
        }
        Ok(())
    }
}

/// One CPR report: its format flag and the two encoded fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CprFrame {
    /// The odd format (F = 1); even when false.
    pub odd: bool,
    /// Encoded latitude (YZ).
    pub lat: u32,
    /// Encoded longitude (XZ).
    pub lon: u32,
}

/// The transition latitudes of NL from DO-260B Table A-21 (also ICAO Doc 9871 C.2.6.2): NL(lat) = 59 − i while
/// |lat| < `NL_TRANSITIONS[i]`, for NL from 59 down to 3; NL is 2 up to and including 87° and 1 beyond. Held as numbers so the zone
/// changes do not depend on the platform's `acos` and `cos`.
pub const NL_TRANSITIONS: [f64; 57] = [
    10.470_471_30,
    14.828_174_37,
    18.186_263_57,
    21.029_394_93,
    23.545_044_87,
    25.829_247_07,
    27.938_987_10,
    29.911_356_86,
    31.772_097_08,
    33.539_934_36,
    35.228_995_98,
    36.850_251_08,
    38.412_418_92,
    39.922_566_84,
    41.386_518_32,
    42.809_140_12,
    44.194_549_51,
    45.546_267_23,
    46.867_332_52,
    48.160_391_28,
    49.427_764_39,
    50.671_501_66,
    51.893_424_69,
    53.095_161_53,
    54.278_174_72,
    55.443_784_44,
    56.593_187_56,
    57.727_473_54,
    58.847_637_76,
    59.954_592_77,
    61.049_177_74,
    62.132_166_59,
    63.204_274_79,
    64.266_165_23,
    65.318_453_10,
    66.361_710_08,
    67.396_467_74,
    68.423_220_22,
    69.442_426_31,
    70.454_510_75,
    71.459_864_73,
    72.458_845_45,
    73.451_774_42,
    74.438_934_16,
    75.420_562_57,
    76.396_843_91,
    77.367_894_61,
    78.333_740_83,
    79.294_282_25,
    80.249_232_13,
    81.198_013_49,
    82.139_569_81,
    83.071_994_45,
    83.991_735_63,
    84.891_661_91,
    85.755_416_21,
    86.535_369_98,
];

/// The number of longitude zones at a latitude, NL (DO-260B A.1.7.2 d): 59 at the equator down to 1 beyond 87°. NaN and
/// latitudes beyond ±90° give 1.
pub fn nl(lat_deg: f64) -> u32 {
    let a = lat_deg.abs();
    if a.is_nan() || a > 90.0 {
        return 1;
    }
    match NL_TRANSITIONS.iter().position(|&t| a < t) {
        Some(i) => 59 - i as u32,
        None if a <= 87.0 => 2,
        None => 1,
    }
}

/// `MOD(x, y) = x − y·floor(x / y)` as DO-260B defines it (the result has the sign of y).
fn modulo(x: f64, y: f64) -> f64 {
    x - y * (x / y).floor()
}

/// Encodes a position as an even or odd CPR frame (DO-260B A.1.7.3).
///
/// ```
/// use sdrkit_util_geo::cpr::{decode_global, encode, CprFormat};
/// use sdrkit_util_geo::Position;
/// let p = Position::new(52.2572, 3.9194);
/// let e = encode(&CprFormat::AIRBORNE, &p, false).unwrap();
/// let o = encode(&CprFormat::AIRBORNE, &p, true).unwrap();
/// let q = decode_global(&CprFormat::AIRBORNE, &e, &o, true, None).unwrap();
/// assert!((q.lat_deg - p.lat_deg).abs() < 1e-4 && (q.lon_deg - p.lon_deg).abs() < 1e-4);
/// ```
pub fn encode(fmt: &CprFormat, p: &Position, odd: bool) -> Result<CprFrame, Error> {
    p.validate()?;
    let i = odd as u32 as f64;
    let dlat = fmt.dlat(odd);
    let yz = (fmt.lat_scale() * modulo(p.lat_deg, dlat) / dlat + 0.5).floor();
    let rlat = dlat * (yz / fmt.lat_scale() + (p.lat_deg / dlat).floor());
    let n = (nl(rlat) as f64 - i).max(1.0);
    let dlon = fmt.span_deg / n;
    let xz = (fmt.lon_scale() * modulo(p.lon_deg, dlon) / dlon + 0.5).floor();
    Ok(CprFrame {
        odd,
        lat: (yz as i64).rem_euclid(1i64 << fmt.lat_bits) as u32,
        lon: (xz as i64).rem_euclid(1i64 << fmt.lon_bits) as u32,
    })
}

/// Decodes an even and an odd frame of the same aircraft without a prior position (DO-260B A.1.7.7 airborne, A.1.7.8 surface).
///
/// `odd_is_latest` says which frame arrived last; the result is the position at that frame. The surface format (90° span) has
/// four solutions; `near` (a nearby known position: the receiver or a recent fix, within 45° of the target) picks one and is
/// required for it.
/// For airborne formats `near` is not used.
///
/// Returns [`Error::CprZoneMismatch`] when the two latitudes are in different NL zones, as DO-260B requires.
pub fn decode_global(
    fmt: &CprFormat,
    even: &CprFrame,
    odd: &CprFrame,
    odd_is_latest: bool,
    near: Option<&Position>,
) -> Result<Position, Error> {
    fmt.check(even)?;
    fmt.check(odd)?;
    if even.odd || !odd.odd {
        return Err(Error::Syntax(SyntaxFault::FrameOrder));
    }
    let surface = fmt.span_deg < 360.0;
    let near = match (surface, near) {
        (true, None) => return Err(Error::Syntax(SyntaxFault::NoNearbyPosition)),
        (_, Some(r)) => {
            r.validate()?;
            Some(r)
        }
        (false, None) => None,
    };
    let (yz0, yz1) = (
        even.lat as f64 / fmt.lat_scale(),
        odd.lat as f64 / fmt.lat_scale(),
    );
    let (xz0, xz1) = (
        even.lon as f64 / fmt.lon_scale(),
        odd.lon as f64 / fmt.lon_scale(),
    );
    let nz4 = (4 * NZ) as f64;
    let j = ((nz4 - 1.0) * yz0 - nz4 * yz1 + 0.5).floor();
    let mut rlat0 = fmt.dlat(false) * (modulo(j, nz4) + yz0);
    let mut rlat1 = fmt.dlat(true) * (modulo(j, nz4 - 1.0) + yz1);
    match near {
        Some(r) if surface => {
            // The northern solution or the one 90° south of it, whichever is nearer the known position.
            let pick = |north: f64| {
                if (north - r.lat_deg).abs() <= (north - 90.0 - r.lat_deg).abs() {
                    north
                } else {
                    north - 90.0
                }
            };
            rlat0 = pick(rlat0);
            rlat1 = pick(rlat1);
        }
        _ => {
            if rlat0 >= 270.0 {
                rlat0 -= 360.0;
            }
            if rlat1 >= 270.0 {
                rlat1 -= 360.0;
            }
        }
    }
    if rlat0.abs() > 90.0 || rlat1.abs() > 90.0 {
        return Err(Error::OutOfRange(Quantity::DecodedLatitude));
    }
    let nl0 = nl(rlat0);
    if nl0 != nl(rlat1) {
        return Err(Error::CprZoneMismatch);
    }
    let (lat, i, xz) = if odd_is_latest {
        (rlat1, 1.0, xz1)
    } else {
        (rlat0, 0.0, xz0)
    };
    let nlv = nl0 as f64;
    let ni = (nlv - i).max(1.0);
    let m = (xz0 * (nlv - 1.0) - xz1 * nlv + 0.5).floor();
    let base = fmt.span_deg / ni * (modulo(m, ni) + xz);
    let lon = match near {
        Some(r) if surface => {
            // Four solutions 90° apart; take the one nearest the known position.
            let mut best = (f64::MAX, 0.0);
            for k in 0..4 {
                let cand = wrap180(base + 90.0 * k as f64);
                let d = wrap180(cand - r.lon_deg).abs();
                if d < best.0 {
                    best = (d, cand);
                }
            }
            best.1
        }
        _ => wrap180(base),
    };
    Ok(Position::new(lat, lon))
}

/// Decodes one frame against a nearby known position (DO-260B A.1.7.5 and A.1.7.6). The result is right when the true position is
/// within half a zone of that position: 180 NM airborne, 45 NM on the surface.
///
/// ```
/// use sdrkit_util_geo::cpr::{decode_local, CprFormat, CprFrame};
/// use sdrkit_util_geo::Position;
/// // The published even frame (lat 93000, lon 51372) near 52°N 4°E.
/// let f = CprFrame { odd: false, lat: 93_000, lon: 51_372 };
/// let p = decode_local(&CprFormat::AIRBORNE, &f, &Position::new(52.258, 3.918)).unwrap();
/// assert!((p.lat_deg - 52.257_202).abs() < 1e-6 && (p.lon_deg - 3.919_373).abs() < 1e-6);
/// ```
pub fn decode_local(fmt: &CprFormat, frame: &CprFrame, near: &Position) -> Result<Position, Error> {
    fmt.check(frame)?;
    near.validate()?;
    let i = frame.odd as u32 as f64;
    let (yz, xz) = (
        frame.lat as f64 / fmt.lat_scale(),
        frame.lon as f64 / fmt.lon_scale(),
    );
    let dlat = fmt.dlat(frame.odd);
    let j = (near.lat_deg / dlat).floor() + (0.5 + modulo(near.lat_deg, dlat) / dlat - yz).floor();
    let lat = dlat * (j + yz);
    if lat.abs() > 90.0 {
        return Err(Error::OutOfRange(Quantity::DecodedLatitude));
    }
    let dlon = fmt.span_deg / (nl(lat) as f64 - i).max(1.0);
    let m = (near.lon_deg / dlon).floor() + (0.5 + modulo(near.lon_deg, dlon) / dlon - xz).floor();
    Ok(Position::new(lat, wrap180(dlon * (m + xz))))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The worked airborne example of Junzi Sun, "The 1090 Megahertz Riddle" (TU Delft, 2021), chapter 5:
    // even frame 8D40621D58C382D690C8AC2863A7, odd frame 8D40621D58C386435CC412692AD6.
    const EVEN: CprFrame = CprFrame {
        odd: false,
        lat: 93_000,
        lon: 51_372,
    };
    const ODD: CprFrame = CprFrame {
        odd: true,
        lat: 74_158,
        lon: 50_194,
    };

    #[test]
    fn nl_table_matches_the_published_formula() {
        // DO-260B A.1.7.2: NL(lat) = floor(2π / acos(1 − (1 − cos(π/(2·NZ))) / cos²(π·lat/180))).
        let formula = |lat: f64| -> u32 {
            let a = 1.0 - (std::f64::consts::PI / (2.0 * NZ as f64)).cos();
            let b = lat.to_radians().cos().powi(2);
            (std::f64::consts::TAU / (1.0 - a / b).acos()).floor() as u32
        };
        for (i, &t) in NL_TRANSITIONS.iter().enumerate() {
            assert_eq!(formula(t - 1e-7), 59 - i as u32, "below transition {i}");
            assert_eq!(formula(t + 1e-7), 58 - i as u32, "above transition {i}");
            assert_eq!(nl(t - 1e-7), 59 - i as u32);
            assert_eq!(nl(-(t + 1e-7)), 58 - i as u32);
        }
        assert_eq!(nl(0.0), 59);
        assert_eq!(nl(87.0), 2);
        assert_eq!(nl(87.000_001), 1);
        assert_eq!(nl(-90.0), 1);
        assert_eq!(nl(f64::NAN), 1);
    }

    #[test]
    fn published_airborne_pair() {
        let f = CprFormat::AIRBORNE;
        let p = decode_global(&f, &EVEN, &ODD, false, None).unwrap();
        assert!(
            (p.lat_deg - 52.257_202_148_437_5).abs() < 1e-9
                && (p.lon_deg - 3.919_372_558_593_75).abs() < 1e-9,
            "{p:?}"
        );
        let p = decode_global(&f, &EVEN, &ODD, true, None).unwrap();
        assert!(
            (p.lat_deg - 52.265_780_174_126_06).abs() < 1e-9 && (p.lon_deg - 3.938_91).abs() < 1e-4,
            "{p:?}"
        );
        let l = decode_local(&f, &ODD, &Position::new(51.0, 5.0)).unwrap();
        assert!((l.lat_deg - p.lat_deg).abs() < 1e-9 && (l.lon_deg - p.lon_deg).abs() < 1e-9);
        // Encoding the decoded positions returns the transmitted fields.
        assert_eq!(
            encode(
                &f,
                &Position::new(52.257_202_148_437_5, 3.919_372_558_593_75),
                false
            )
            .unwrap(),
            EVEN
        );
    }

    #[test]
    fn published_surface_pair() {
        // "The 1090 Megahertz Riddle", surface position example, receiver near Delft (51.990, 4.375).
        let f = CprFormat::SURFACE;
        let e = CprFrame {
            odd: false,
            lat: 115_609,
            lon: 116_941,
        };
        let o = CprFrame {
            odd: true,
            lat: 39_199,
            lon: 110_269,
        };
        let r = Position::new(51.990, 4.375);
        let p = decode_global(&f, &e, &o, true, Some(&r)).unwrap();
        assert!(
            (p.lat_deg - 52.320_607).abs() < 5e-7 && (p.lon_deg - 4.734_735).abs() < 5e-7,
            "{p:?}"
        );
        let l = decode_local(
            &f,
            &CprFrame {
                odd: true,
                lat: 39_195,
                lon: 110_320,
            },
            &p,
        )
        .unwrap();
        assert!(
            (l.lat_deg - 52.320_561).abs() < 5e-7 && (l.lon_deg - 4.735_735).abs() < 5e-7,
            "{l:?}"
        );
        assert!(decode_global(&f, &e, &o, true, None).is_err());
    }

    #[test]
    fn a_pair_straddling_an_nl_boundary_is_refused() {
        // The aircraft is just south of the NL 59/58 boundary for the even frame and just north of it for the odd frame.
        let f = CprFormat::AIRBORNE;
        let t = NL_TRANSITIONS[0];
        let e = encode(&f, &Position::new(t - 0.001, 10.0), false).unwrap();
        let o = encode(&f, &Position::new(t + 0.001, 10.0), true).unwrap();
        assert_eq!(
            decode_global(&f, &e, &o, true, None),
            Err(Error::CprZoneMismatch)
        );
        // The same check holds for the other widths.
        let v = CprFormat::VDL4;
        let e = encode(&v, &Position::new(t - 0.01, 10.0), false).unwrap();
        let o = encode(&v, &Position::new(t + 0.01, 10.0), true).unwrap();
        assert_eq!(
            decode_global(&v, &e, &o, true, None),
            Err(Error::CprZoneMismatch)
        );
    }

    #[test]
    fn bad_input_is_an_error() {
        let f = CprFormat::AIRBORNE;
        let wide = CprFrame {
            odd: false,
            lat: 1 << 17,
            lon: 0,
        };
        assert!(decode_global(&f, &wide, &ODD, true, None).is_err());
        assert!(decode_global(&f, &ODD, &EVEN, true, None).is_err());
        assert!(decode_local(&f, &EVEN, &Position::new(f64::NAN, 0.0)).is_err());
        assert!(encode(&f, &Position::new(100.0, 0.0), false).is_err());
        assert!(CprFormat::new(0, 17, 360.0).is_err());
        assert!(CprFormat::new(17, 17, 180.0).is_err());
        assert_eq!(CprFormat::new(14, 14, 360.0).unwrap(), CprFormat::INTENT);
    }

    fn rng(s: &mut u64) -> f64 {
        *s ^= *s << 13;
        *s ^= *s >> 7;
        *s ^= *s << 17;
        (*s >> 11) as f64 / (1u64 << 53) as f64
    }

    #[test]
    fn every_format_round_trips() {
        let mut s = 0x9e37_79b9_7f4a_7c15u64;
        for fmt in [
            CprFormat::AIRBORNE,
            CprFormat::INTENT,
            CprFormat::TISB_COARSE,
            CprFormat::VDL4,
            CprFormat::SURFACE,
        ] {
            // The worst-case error is about one quantum of the coarser field.
            let lat_q = fmt.dlat(true) / fmt.lat_scale();
            for _ in 0..4_000 {
                let p = Position::new(-86.0 + 172.0 * rng(&mut s), -180.0 + 360.0 * rng(&mut s));
                let lon_q = fmt.span_deg / fmt.lon_scale();
                let e = encode(&fmt, &p, false).unwrap();
                let o = encode(&fmt, &p, true).unwrap();
                let r = Position::new(
                    p.lat_deg + (rng(&mut s) - 0.5) * 0.2,
                    wrap180(p.lon_deg + (rng(&mut s) - 0.5) * 0.2),
                );
                match decode_global(&fmt, &e, &o, true, Some(&r)) {
                    Ok(q) => {
                        assert!(
                            (q.lat_deg - p.lat_deg).abs() <= lat_q,
                            "{fmt:?} {p:?} {q:?}"
                        );
                        assert!(
                            wrap180(q.lon_deg - p.lon_deg).abs() <= lon_q,
                            "{fmt:?} {p:?} {q:?}"
                        );
                    }
                    // Only refused when the two rounded latitudes straddle an NL boundary.
                    Err(Error::CprZoneMismatch) => {}
                    Err(x) => panic!("{x:?} for {p:?}"),
                }
                let q = decode_local(&fmt, &o, &r).unwrap();
                assert!(
                    (q.lat_deg - p.lat_deg).abs() <= lat_q,
                    "{fmt:?} {p:?} {q:?}"
                );
                assert!(
                    wrap180(q.lon_deg - p.lon_deg).abs() <= lon_q,
                    "{fmt:?} {p:?} {q:?}"
                );
            }
        }
    }
}
