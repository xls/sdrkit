//! The results that each component of the `gnss` and `sbas` stacks declares, from the shapes of the description.

use crate::typed::{
    GnssFields, InterferenceFields, SbasFields, FIX_SYSTEMS, INTERFERENCE_EVENTS,
    INTERFERENCE_LINES, MONITOR_SERIES, RECEIVER_SERIES, SATELLITES, SBAS_FAST_CORRECTIONS,
    SBAS_IONOSPHERE, SBAS_MESSAGES, SBAS_SATELLITES, SBAS_SATELLITE_TYPES, SBAS_TYPES, SYSTEMS,
};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::ResultsDecl;

sdrkit_core::choice_enum! {
    /// Where the frequency error of the radio comes from.
    pub enum FrequencySource {
        /// Measured by the fix.
        Measured = ("measured", "Measured by the fix"),
        /// Near the search centre until the fix.
        SearchCentre = ("search_centre", "About the search centre"),
        /// Not known yet.
        Unknown = ("unknown", "Not known yet"),
    }
}

sdrkit_core::choice_enum! {
    /// What the receiver holds of a satellite's ephemeris.
    pub enum EphemerisState {
        /// The ephemeris is held.
        Held = ("held", "Held"),
        /// Parts of it are collected.
        Collecting = ("collecting", "Collecting"),
        /// No part of it is collected yet.
        Waiting = ("waiting", "Waiting for the frame"),
        /// The system sends none that the receiver uses (SBAS).
        NotUsed = ("not_used", "Not used"),
    }
}

sdrkit_core::choice_enum! {
    /// Where a satellite's position comes from.
    pub enum OrbitSource {
        /// The broadcast ephemeris.
        Ephemeris = ("ephemeris", "Broadcast ephemeris"),
        /// The almanac.
        Almanac = ("almanac", "Almanac"),
    }
}

// Declarations.

/// `d` with the series `series`: key, label, unit of x and unit of y.
fn with_series(mut d: ResultsDecl, series: &[(&str, &str, &str, &str)]) -> ResultsDecl {
    for (key, label, x, y) in series {
        d = d.series(key, label, x, y);
    }
    d
}

/// The results of the receiver component.
pub fn receiver_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(GnssFields::SHAPE.fields)
            .with_table(SYSTEMS.def())
            .with_table(FIX_SYSTEMS.def())
            .with_table(SATELLITES.def()),
        &RECEIVER_SERIES,
    )
    .points("Satellites")
}

/// The results of the interference monitor.
pub fn monitor_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(InterferenceFields::SHAPE.fields)
            .with_table(INTERFERENCE_LINES.def())
            .with_table(INTERFERENCE_EVENTS.def()),
        &MONITOR_SERIES,
    )
}

/// The results of the SBAS content decoder.
pub fn sbas_content_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(SbasFields::SHAPE.fields)
        .with_table(SBAS_SATELLITES.def())
        .with_table(SBAS_SATELLITE_TYPES.def())
        .with_table(SBAS_TYPES.def())
        .with_table(SBAS_MESSAGES.def())
        .with_table(SBAS_FAST_CORRECTIONS.def())
        .with_table(SBAS_IONOSPHERE.def())
}
