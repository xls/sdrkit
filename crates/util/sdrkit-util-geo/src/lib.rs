//! Positions on the Earth, time scales, satellite orbits and NMEA 0183, in one place for every sdrkit crate.
//!
//! This crate has no dependencies beyond `std` (and `serde`, behind the `serde` feature). Bad input returns an [`Error`], never a
//! panic.
//!
//! # Conventions
//!
//! - **Latitude first, always.** Every function and type takes latitude before longitude. North and east are positive.
//! - **Units are in the name** when the type does not carry them: `lat_deg`, `alt_m`, `range_rate_m_s`. Distances are metres,
//!   speeds metres per second, angles degrees in the public API.
//! - **One Earth model per kind of sum.** Spherical formulas (haversine distance, bearing, destination, cross-track) use the IUGG
//!   mean radius [`EARTH_MEAN_RADIUS_M`] = 6 371 008.8 m. Accurate distances use the WGS-84 ellipsoid ([`Ellipsoid::WGS84`])
//!   with Vincenty's formulae. ECEF, ENU and look angles use WGS-84.
//! - **Time is Unix seconds on the UTC scale** (`f64`, or `i64` seconds with nanoseconds where precision matters). Leap seconds
//!   come from one table, [`time::LeapSeconds`], which a host can replace with a newer IERS list.
//!
//! # Modules
//!
//! | Module | Holds |
//! |---|---|
//! | [`position`] | [`Position`], great-circle distance, bearings, destination, cross-track, closest point of approach, fences |
//! | [`ellipsoid`] | ellipsoids, ECEF and geodetic, ENU, azimuth/elevation/range look angles, Vincenty distance and destination |
//! | [`maidenhead`] | Maidenhead locators of 2 to 10 characters, both directions |
//! | [`coord`] | coordinate text (decimal, DMS, degrees and decimal minutes as NMEA writes them), fixed-point angles |
//! | [`cpr`] | compact position reporting (ADS-B airborne and surface, intent, TIS-B, VDL Mode 4 and other widths) |
//! | [`orbit`] | TLE parsing, SGP4/SDP4, two-body orbits, GMST, TEME to ECEF, Doppler, pass prediction |
//! | [`time`] | civil dates, MJD, Julian day, leap seconds, TAI, GPS, Galileo, BeiDou and GLONASS time, ISO 8601 |
//! | [`nmea`] | NMEA 0183 sentences (build, parse, checksum, tag blocks), GGA/RMC/GLL, AIVDM armouring and reassembly |
//! | [`text`] | [`Text`], a fixed-capacity string the formatters return so they do not allocate |
//!
//! # Example
//!
//! ```
//! use sdrkit_util_geo::{maidenhead, Position};
//!
//! let berlin = Position::new(52.5200, 13.4050);
//! let paris = Position::new(48.8566, 2.3522);
//! let d = berlin.distance_m(&paris);
//! assert!((d - 877_500.0).abs() < 1_000.0);
//! assert_eq!(maidenhead::encode(&berlin, 6).unwrap().as_str(), "JO62qm");
//! ```

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod coord;
pub mod cpr;
pub mod ellipsoid;
mod error;
pub mod maidenhead;
pub mod nmea;
pub mod orbit;
pub mod position;
pub mod text;
pub mod time;

pub use ellipsoid::{Ecef, Ellipsoid, Enu, Look, Observer};
pub use error::{Error, Quantity, SyntaxFault};
pub use position::{Position, EARTH_MEAN_RADIUS_M};
pub use text::Text;

/// The speed of light in vacuum, m/s (exact, SI definition).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;
/// One international nautical mile, m (exact).
pub const NAUTICAL_MILE_M: f64 = 1_852.0;
/// One knot, m/s (exact: one nautical mile per hour).
pub const KNOT_M_S: f64 = 1_852.0 / 3_600.0;
/// One international foot, m (exact).
pub const FOOT_M: f64 = 0.3048;
