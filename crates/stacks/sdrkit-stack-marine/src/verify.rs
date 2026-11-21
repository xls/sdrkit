//! What the family registers beside its descriptors: one verification case per stack (its test signal, which must fill the stack's
//! main table), and the benchmarks.

use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

pub(crate) static AIS: VerifyHooks = VerifyHooks {
    cases: ais_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static MARINE: VerifyHooks = VerifyHooks {
    cases: marine_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static VDES: VerifyHooks = VerifyHooks {
    cases: vdes_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static MARINEVHF: VerifyHooks = VerifyHooks {
    cases: vhf_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static ATIS: VerifyHooks = VerifyHooks {
    cases: atis_cases,
    vector_settings: None,
    check_vector: None,
};

/// The number of rows of table `name`.
fn rows(r: &Results, name: &str) -> usize {
    r.decl()
        .table_id(name)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// A check that table `name` holds at least one row.
fn has_rows(r: &Results, name: &str, what: &str) -> Vec<CheckLine> {
    let n = rows(r, name);
    vec![CheckLine::new(name, n > 0, format!("{n} {what}"))]
}

/// The port demo: ships heard on AIS 1 and 2.
fn ais_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(6.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| has_rows(r, "stations", "stations listed"));
    vec![c]
}

/// The NAVTEX demo messages and the DSC test calls.
fn marine_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut navtex = VerifyCase::new("NAVTEX test signal", o, 0.0);
    navtex.submode = Some("navtex".into());
    navtex.seconds = navtex.seconds.max(40.0);
    navtex.check = Some(|r| has_rows(r, "navtex", "NAVTEX messages listed"));
    let mut dsc = VerifyCase::new("DSC test signal", o, 0.0);
    dsc.submode = Some("dsc".into());
    dsc.freq_hz = Some(2187.5e3);
    dsc.seconds = dsc.seconds.max(30.0);
    dsc.check = Some(|r| has_rows(r, "dsc", "DSC calls listed"));
    vec![navtex, dsc]
}

/// An ASM frame and VDE-TER bursts.
fn vdes_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| has_rows(r, "asm", "ASM frames listed"));
    vec![c]
}

/// Voice calls on several channels.
fn vhf_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(8.0);
    c.check = Some(|r| has_rows(r, "calls", "calls logged"));
    vec![c]
}

/// A spoken transmission with its ATIS identity.
fn atis_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(8.0);
    c.check = Some(|r| has_rows(r, "stations", "ATIS stations listed"));
    vec![c]
}
