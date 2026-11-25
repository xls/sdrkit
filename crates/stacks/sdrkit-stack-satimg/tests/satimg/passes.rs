//! The end of a pass on the test signals with `gen_lines` (`gen_rows` for Meteor LRPT): the signal ends after its pass,
//! and the picture of the pass reaches the finished state in the silence after it, before the end of the input.

use super::common::{finished, impaired, pictures_of};
use sdrkit_core::{Complex32, InputInfo, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::components::pass::PASS_GAP;
use sdrkit_stack_satimg::modes;

/// One test pass: the stack, its submode, the rate, the frequency, the setting of the pass length and its value, and
/// the seconds asked of the generator (more than the pass).
struct Case {
    desc: &'static StackDescriptor,
    submode: Option<&'static str>,
    rate: f64,
    hz: f64,
    gen_lines: &'static sdrkit_core::Setting<sdrkit_core::publish::field::I64>,
    lines: i64,
    seconds: f64,
}

/// Runs `c` with a pass gap of 1 s and 1.5 s of silence after the pass. It returns the seconds of the pass.
fn pass_ends_finished(c: &Case) -> f64 {
    let mut h = impaired(
        Harness::new(c.desc).rate(c.rate).seconds(c.seconds),
        Some(30.0),
        0.0,
        0.0,
    )
    .set(c.gen_lines, c.lines)
    .set(&PASS_GAP, 1.0);
    if let Some(s) = c.submode {
        h = h.submode(s);
    }
    let mut x = h.generate().expect("a signal");
    let pass_s = x.len() as f64 / c.rate;
    assert!(
        pass_s < c.seconds - 0.1,
        "{}: the signal did not end ({pass_s} s)",
        c.desc.id
    );
    x.extend(vec![Complex32::default(); (1.5 * c.rate) as usize]);
    let mut rx = h.receiver().expect("a receiver");
    rx.configure(&InputInfo::new(c.rate, c.hz, c.hz).recording())
        .expect("the rate");
    let sent = pictures_of(&mut *rx, &x, 65_536, false);
    assert!(!sent.is_empty(), "{}: no picture", c.desc.id);
    let done = finished(&sent);
    assert_eq!(done.len(), 1, "{}: {sent:#?}", c.desc.id);
    assert_eq!(
        sent.last().map(|s| s.state),
        Some(sdrkit_core::PictureState::Finished),
        "{}",
        c.desc.id
    );
    pass_s
}

#[test]
fn aws() {
    pass_ends_finished(&Case {
        desc: &modes::aws::DESCRIPTOR,
        submode: None,
        rate: 6e6,
        hz: 1707e6,
        gen_lines: &modes::aws::GEN_LINES,
        lines: 3,
        seconds: 6.0,
    });
}

#[test]
fn noaa_dsb() {
    pass_ends_finished(&Case {
        desc: &modes::dsb::DESCRIPTOR,
        submode: None,
        rate: 96e3,
        hz: 137.77e6,
        gen_lines: &modes::dsb::GEN_LINES,
        lines: 1,
        seconds: 30.0,
    });
}

#[test]
fn goes_gvar() {
    pass_ends_finished(&Case {
        desc: &modes::gvar::DESCRIPTOR,
        submode: None,
        rate: 6e6,
        hz: 1685.7e6,
        gen_lines: &modes::gvar::GEN_LINES,
        lines: 2,
        seconds: 6.0,
    });
}

#[test]
fn hrpt() {
    pass_ends_finished(&Case {
        desc: &modes::hrpt::DESCRIPTOR,
        submode: None,
        rate: 8e6,
        hz: 1701.3e6,
        gen_lines: &modes::hrpt::GEN_LINES,
        lines: 12,
        seconds: 6.0,
    });
}

#[test]
fn hrpt_meteor() {
    pass_ends_finished(&Case {
        desc: &modes::hrpt::DESCRIPTOR,
        submode: Some("meteor"),
        rate: 4e6,
        hz: 1700e6,
        gen_lines: &modes::hrpt::GEN_LINES,
        lines: 12,
        seconds: 6.0,
    });
}

#[test]
fn msg_raw() {
    pass_ends_finished(&Case {
        desc: &modes::msg::DESCRIPTOR,
        submode: None,
        rate: 6e6,
        hz: 1686.83e6,
        gen_lines: &modes::msg::GEN_LINES,
        lines: 30,
        seconds: 6.0,
    });
}

#[test]
fn fy2_svissr() {
    pass_ends_finished(&Case {
        desc: &modes::fy2::DESCRIPTOR,
        submode: None,
        rate: 2.4e6,
        hz: 1687.5e6,
        gen_lines: &modes::fy2::GEN_LINES,
        lines: 4,
        seconds: 9.0,
    });
}

#[test]
fn noaa_gac() {
    pass_ends_finished(&Case {
        desc: &modes::gac::DESCRIPTOR,
        submode: None,
        rate: 8e6,
        hz: 1702.5e6,
        gen_lines: &modes::gac::GEN_LINES,
        lines: 4,
        seconds: 6.0,
    });
}

#[test]
fn meteor_lrpt() {
    pass_ends_finished(&Case {
        desc: &modes::meteor::DESCRIPTOR,
        submode: None,
        rate: 250e3,
        hz: 137.9e6,
        gen_lines: &modes::meteor::GEN_ROWS,
        lines: 6,
        seconds: 15.0,
    });
}

#[test]
fn a_new_meteor_pass_starts_at_row_0() {
    use super::common::run;
    use sdrkit_codec_image::strip::Strip;
    // Two passes, each with 1.5 s of silence after it: the on-board times of the second pass start again, earlier than
    // the end of the first.
    let h = impaired(
        Harness::new(&modes::meteor::DESCRIPTOR)
            .rate(250e3)
            .seconds(15.0),
        Some(30.0),
        0.0,
        0.0,
    )
    .set(&modes::meteor::GEN_ROWS, 6)
    .set(&PASS_GAP, 1.0);
    let pass = h.generate().expect("a signal");
    let mut x = pass.clone();
    x.extend(vec![Complex32::default(); 375_000]);
    x.extend(&pass);
    x.extend(vec![Complex32::default(); 375_000]);
    let r = run::<Strip>(&h, &x, 250e3, 137.9e6, 137.9e6, 65_536, Some("msumr.out"));
    let strips = r.of::<Strip>();
    let first = strips.first().map_or(0, |s| s.picture);
    let (one, two): (Vec<_>, Vec<_>) = strips.iter().partition(|s| s.picture == first);
    assert!(
        !one.is_empty() && !two.is_empty(),
        "{} strips",
        strips.len()
    );
    assert!(two.iter().all(|s| s.picture == first + 1));
    let min = |v: &[&Strip]| v.iter().map(|s| s.first_row).min();
    let max = |v: &[&Strip]| v.iter().map(|s| s.first_row).max();
    assert_eq!(min(&one), Some(0));
    assert_eq!(min(&two), Some(0));
    assert_eq!(max(&one), max(&two));
}
