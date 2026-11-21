//! The verification cases of the family: the test signal of each stack must give its text, picture, frames or identifications,
//! and each recording must contain the text that `expected.json` names.

use crate::typed::{
    ArdopFields, Cp16Fields, HellFields, KeyboardFields, PictureFields, QrssFields, RosFields,
    WinmorFields,
};
use sdrkit_core::interface::Column;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VectorRun, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

pub(crate) static TEXT: VerifyHooks = VerifyHooks {
    cases: text_cases,
    vector_settings: None,
    check_vector: Some(check_text_vector),
};
pub(crate) static PICTURE: VerifyHooks = VerifyHooks {
    cases: picture_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static FRAMES: VerifyHooks = VerifyHooks {
    cases: frame_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static DETECTIONS: VerifyHooks = VerifyHooks {
    cases: detection_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static PILOTS: VerifyHooks = VerifyHooks {
    cases: pilot_cases,
    vector_settings: None,
    check_vector: None,
};
pub(crate) static COLUMNS: VerifyHooks = VerifyHooks {
    cases: column_cases,
    vector_settings: None,
    check_vector: None,
};

/// A count field of the results, 0 when it has no value.
fn count(r: &Results, c: &Column<field::I64>) -> i64 {
    r.fields().at(c).unwrap_or(0)
}

fn case(o: &VerifyOptions, seconds: f64) -> VerifyCase {
    let mut c = VerifyCase::new("test signal", o, 0.0);
    c.seconds = c.seconds.max(seconds);
    c
}

/// WINMOR pilots are reported.
fn pilot_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 6.0);
    c.check = Some(|r| {
        let n = count(r, &WinmorFields::PILOTS);
        vec![CheckLine::new("pilots", n > 0, format!("{n} pilots"))]
    });
    vec![c]
}

/// The test text comes out.
fn text_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 30.0);
    c.check = Some(|r| {
        let n = count(r, &KeyboardFields::CHARACTERS);
        vec![CheckLine::new(
            "characters",
            n > 0,
            format!("{n} characters"),
        )]
    });
    vec![c]
}

/// A picture is received.
fn picture_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 60.0);
    c.check = Some(|r| {
        let n = count(r, &PictureFields::PICTURE_ROWS);
        vec![CheckLine::new("picture_rows", n > 0, format!("{n} rows"))]
    });
    vec![c]
}

/// ARDOP frames are read.
fn frame_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 20.0);
    c.check = Some(|r| {
        let n = count(r, &ArdopFields::FRAMES_OK);
        vec![CheckLine::new("frames_ok", n > 0, format!("{n} frames"))]
    });
    vec![c]
}

/// ROS transmissions are identified.
fn detection_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 30.0);
    c.check = Some(|r| {
        let n = count(r, &RosFields::DETECTIONS);
        vec![CheckLine::new(
            "detections",
            n > 0,
            format!("{n} detections"),
        )]
    });
    vec![c]
}

/// Hell columns are drawn.
fn column_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 20.0);
    c.check = Some(|r| {
        let n = count(r, &HellFields::COLUMNS);
        vec![CheckLine::new("columns", n > 0, format!("{n} columns"))]
    });
    vec![c]
}

/// A recording's text must contain `expect.contains` of its section.
fn check_text_vector(v: &VectorRun) -> Vec<CheckLine> {
    let Some(want) = v.section["expect"]["contains"].as_str() else {
        return Vec::new();
    };
    let text = v.results.fields().at(&KeyboardFields::TEXT).unwrap_or("");
    vec![CheckLine::new(
        "expect.contains",
        text.contains(want),
        format!("{:?} in the text", want),
    )]
}

/// QRSS: the steady tone is found (the text of 3 s elements takes minutes).
pub(crate) static QRSS: VerifyHooks = VerifyHooks {
    cases: qrss_cases,
    vector_settings: None,
    check_vector: None,
};

fn qrss_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 45.0);
    c.check = Some(|r| {
        let found = r.fields().at(&QrssFields::LOCKED).unwrap_or(false);
        vec![CheckLine::new(
            "locked",
            found,
            format!("tone found: {found}"),
        )]
    });
    vec![c]
}

/// CP16: the carriers and the rows are found.
pub(crate) static CP16: VerifyHooks = VerifyHooks {
    cases: cp16_cases,
    vector_settings: None,
    check_vector: None,
};

fn cp16_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = case(o, 30.0);
    c.check = Some(|r| {
        let state = r.fields().at(&Cp16Fields::STATE).unwrap_or(0);
        vec![CheckLine::new(
            "state",
            state == 2,
            format!("state {state}"),
        )]
    });
    vec![c]
}
