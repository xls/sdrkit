//! Location of a radio source from the times at which several receivers hear it.
//!
//! | Module | Contents |
//! |---|---|
//! | [`propagation`] | WGS-84 distances (Vincenty, with a haversine fallback), destinations, north-east offsets; ground-wave and ionosphere-hop travel times |
//! | [`stats`] | the normal quantile (Abramowitz and Stegun 26.2.23) and the chi-square quantile (Wilson-Hilferty) |
//! | [`clock`] | ranked timing sources, the disciplined sample clock (UTC as a fitted line of the sample time) and a set of clocks, one per source |
//! | [`tdoa`] | the position solver: latitude, longitude and one offset per clock group from arrival times, with ambiguities, outliers, a chi-square test and the 95 % error ellipse |
//! | [`commonview`] | the alignment of receiver clocks on the symbol boundaries of one transmitter that all of them receive |
//! | [`matcher`] | the grouping of detections from several receivers into sets that one source explains, and their positions |
//!
//! Times on the UTC scale are `i64` nanoseconds; durations, sigmas and times relative to one origin are `f64` seconds. Distances are
//! metres and angles degrees. Bad input returns an error, never a panic.

#![warn(missing_docs)]
#![deny(unsafe_code)]

pub mod clock;
pub mod commonview;
mod error;
pub mod matcher;
pub mod propagation;
pub mod stats;
pub mod tdoa;
#[cfg(test)]
mod testkit;

pub use error::LocateError;
pub use propagation::{Propagation, SPEED_OF_LIGHT_M_S};
