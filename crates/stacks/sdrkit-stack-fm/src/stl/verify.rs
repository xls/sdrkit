//! The verify cases of the STL stack: a composite link with RDS 25 kHz off the tuned frequency, a mono link, and a
//! digital-like link that is detected and not decoded.

use super::settings::{GenLink, GEN_LINK};
use super::typed::{LinkKind, StlFields};
use crate::results::Reading;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::Results;

pub(crate) static HOOKS: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let seconds = o.seconds.max(4.0);
    let mut composite = VerifyCase::new("test link, composite stereo with RDS", o, 25e3);
    composite.seconds = seconds;
    composite.expect_audio = Some(true);
    composite.check = Some(check_composite);
    let mut mono = VerifyCase::new("test link, mono", o, 1000.0).with(&GEN_LINK, GenLink::Mono);
    mono.seconds = seconds;
    mono.expect_audio = Some(true);
    mono.check = Some(check_mono);
    let mut digital =
        VerifyCase::new("test link, digital-like", o, 1000.0).with(&GEN_LINK, GenLink::Digital);
    digital.seconds = seconds;
    digital.expect_lock = false;
    digital.expect_audio = Some(false);
    digital.check = Some(check_digital);
    vec![composite, mono, digital]
}

fn link(r: &Results) -> LinkKind {
    r.fields().at(&StlFields::LINK).unwrap_or_default()
}

fn kind_line(r: &Results, want: LinkKind) -> CheckLine {
    let k = link(r);
    CheckLine::new("link", k == want, format!("{k:?}, sent {want:?}"))
}

fn check_composite(r: &Results) -> Vec<CheckLine> {
    let d = Reading::from_results(r);
    let st = d.station.trim_end();
    vec![
        kind_line(r, LinkKind::Composite),
        CheckLine::new("pilot", d.pilot_locked, format!("{:.1} %", d.pilot_pct)),
        CheckLine::new("station", st == "SDRKIT", format!("{st:?}, sent \"SDRKIT\"")),
        CheckLine::new("pi", d.pi == 0x4A21, format!("{:04X}, sent 4A21", d.pi)),
        CheckLine::new(
            "offset",
            (d.offset_hz - 25e3).abs() < 3e3,
            format!("{:.0} Hz, sent 25000 Hz", d.offset_hz),
        ),
    ]
}

fn check_mono(r: &Results) -> Vec<CheckLine> {
    let d = Reading::from_results(r);
    let rms = r.fields().at(&StlFields::SOUND_RMS_DBFS);
    vec![
        kind_line(r, LinkKind::Mono),
        CheckLine::new("no pilot", !d.pilot_locked, format!("{:.1} %", d.pilot_pct)),
        CheckLine::new(
            "sound",
            rms.is_some_and(|x| (-30.0..0.0).contains(&x)),
            format!("RMS {rms:?} dBFS"),
        ),
    ]
}

fn check_digital(r: &Results) -> Vec<CheckLine> {
    let width = r.fields().at(&StlFields::OCCUPIED_WIDTH_HZ);
    let summary = &r.status().summary;
    vec![
        kind_line(r, LinkKind::DigitalUnknown),
        CheckLine::new(
            "width",
            width.is_some_and(|w| (200e3..280e3).contains(&w)),
            format!("{width:?} Hz, sent 220 kHz"),
        ),
        CheckLine::new(
            "not decoded",
            summary.contains("not decoded"),
            summary.clone(),
        ),
    ]
}
