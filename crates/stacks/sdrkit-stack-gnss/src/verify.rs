//! What the crate registers beside its descriptors: one test-signal case for each stack, with checks of what the receiver
//! found.

use crate::typed::GnssFields;
use crate::settings::{GEN_BEIDOU, GEN_GALILEO, GEN_GLONASS, GEN_QZSS, GEN_SBAS};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::{field, Declare};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

/// The verify case of `gnss`.
pub static GNSS_VERIFY: VerifyHooks = VerifyHooks {
    cases: gnss_cases,
    vector_settings: None,
    check_vector: None,
};

/// The verify case of `sbas`.
pub static SBAS_VERIFY: VerifyHooks = VerifyHooks {
    cases: sbas_cases,
    vector_settings: None,
    check_vector: None,
};

fn at_least(r: &Results, c: &Column<field::I64>, min: f64) -> CheckLine {
    let v = r.fields().at(c).map_or(0.0, |x| x as f64);
    CheckLine::new(c.key(), v >= min, format!("{v}, at least {min} expected"))
}

/// GPS only: after 12 s, at least four satellites tracked.
fn gnss_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, GPS fix", o, 1500.0)
        .with(&GEN_QZSS, false)
        .with(&GEN_SBAS, false)
        .with(&GEN_GALILEO, false)
        .with(&GEN_BEIDOU, false)
        .with(&GEN_GLONASS, false);
    c.seconds = o.seconds.max(12.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| vec![at_least(r, &GnssFields::SATELLITES_TRACKED, 4.0)]);
    vec![c]
}

/// The three SBAS satellites of the test signal: after 20 s, all three tracked and their messages decoded.
fn sbas_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, three SBAS satellites", o, 1500.0);
    c.seconds = o.seconds.max(20.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let summary = r.status().summary.as_str();
        vec![
            at_least(r, &GnssFields::SBAS_MESSAGES, 30.0),
            CheckLine::new(
                "summary",
                summary.starts_with("SBAS: 3 satellites tracked"),
                summary.to_string(),
            ),
        ]
    });
    vec![c]
}
