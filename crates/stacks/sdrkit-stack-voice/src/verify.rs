//! The checks of the test signal that a host runs on the stacks of the crate.

use crate::typed::FleetsyncFields;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

/// The checks of the FleetSync stack.
pub static FLEETSYNC: VerifyHooks = VerifyHooks {
    cases: fleetsync_cases,
    vector_settings: None,
    check_vector: None,
};

/// The test signal at default settings: a plain and a FleetSync II burst on two carriers, each with its unit.
fn fleetsync_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a plain and a FleetSync II burst", o, 0.0);
    c.seconds = o.seconds.max(6.0);
    c.check = Some(fleetsync_check);
    vec![c]
}

/// Both bursts and both units are listed.
fn fleetsync_check(r: &Results) -> Vec<CheckLine> {
    let f = r.fields();
    let bursts = f.at(&FleetsyncFields::BURSTS).unwrap_or(0);
    let units = f.at(&FleetsyncFields::UNITS).unwrap_or(0);
    vec![
        CheckLine::new(
            "bursts",
            bursts >= 2,
            format!("{bursts} bursts (at least 2)"),
        ),
        CheckLine::new("units", units >= 2, format!("{units} units (at least 2)")),
    ]
}
