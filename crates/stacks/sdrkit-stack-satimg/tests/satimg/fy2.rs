//! FY-2 S-VISSR.

use super::common::{assert_declared, feed, impaired, run, Ran};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, InputInfo, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_graph::Graph;
use sdrkit_stack_satimg::instruments::svissr::build;
use sdrkit_stack_satimg::typed::{shape, IMAGES};
use sdrkit_stack_satimg::modes::fy2::{test_line, SvissrComponent, DESCRIPTOR};

fn go(rate: f64, h: Harness) -> Ran {
    let x = h.generate().expect("a signal");
    run::<Strip>(&h, &x, rate, 1687.5e6, 1687.5e6, 60_000, Some("svissr.out"))
}

fn harness(rate: f64, secs: f64) -> Harness {
    Harness::new(&DESCRIPTOR).rate(rate).seconds(secs)
}

/// The IR1 strips that equal the line of their counter in the test picture.
fn lines_exact(strips: &[Strip]) -> usize {
    strips
        .iter()
        .filter(|s| s.channel == 1 && *s.samples == test_line(s.first_row as u16).ir[0][..])
        .count()
}

#[test]
fn clean_signal() {
    let r = go(2.4e6, impaired(harness(2.4e6, 9.0), Some(30.0), 0.0, 0.0));
    assert!(r.num("lines") >= 12.0, "{}", r.num("lines"));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("preamble_ber") < 1e-4 && r.num("data_ber") < 1e-4);
    assert_eq!(r.text("hint"), "");
    let es = r.num("snr_db");
    assert!((25.0..=35.0).contains(&es), "{es}");
    assert!(lines_exact(&r.of::<Strip>()) >= 8);
}

#[test]
fn six_db_preamble_ber() {
    let r = go(2.4e6, impaired(harness(2.4e6, 9.0), Some(6.0), 0.0, 0.0));
    assert!(r.num("lines") >= 12.0, "{}", r.num("lines"));
    // 2 q (1 - q), q = erfc(sqrt(10^0.6)) / 2: about 4e-3 (NRZ-M doubles the channel errors).
    let ber = r.num("preamble_ber");
    assert!(ber > 0.002 && ber < 0.008 * 2.0, "{ber}");
}

#[test]
fn offset_and_clock_error() {
    let r = go(
        3.2e6,
        impaired(harness(3.2e6, 10.0), Some(14.0), 18e3, 50.0),
    );
    assert!(r.num("lines") >= 10.0, "{}", r.num("lines"));
    assert!(
        (r.num("offset_hz") - 18e3).abs() < 400.0,
        "{}",
        r.num("offset_hz")
    );
    assert!(lines_exact(&r.of::<Strip>()) >= 8);
}

#[test]
fn import_of_frames() {
    let quiet = vec![Complex32::default(); 600_000];
    let import = |frames: &[u8]| -> Ran {
        let mut rx = Harness::new(&DESCRIPTOR).receiver().unwrap();
        rx.configure(&InputInfo::new(2.4e6, 1687.5e6, 1687.5e6).recording())
            .unwrap();
        rx.as_any_mut()
            .and_then(|a| a.downcast_mut::<Graph>())
            .and_then(|g| g.component_as_mut::<SvissrComponent>("svissr"))
            .expect("the S-VISSR decoder")
            .import(frames.to_vec());
        feed::<Strip>(rx, &quiet, 60_000, None)
    };
    let frames: Vec<u8> = (0..6u16).flat_map(|c| build(&test_line(c))).collect();
    let r = import(&frames);
    assert_eq!(r.num("lines"), 6.0);
    assert_eq!(r.num("line_counter"), 5.0);
    assert_eq!(r.num("imported"), 6.0);
    // Two scans: the counter falls by more than 100.
    let two: Vec<u8> = [
        (500..506u16)
            .flat_map(|c| build(&test_line(c)))
            .collect::<Vec<u8>>(),
        frames,
    ]
    .concat();
    let r = import(&two);
    assert_eq!(r.num("pictures_started"), 2.0);
    let images = r.results.decl().table_id(IMAGES.key).unwrap();
    assert!(r.results.table(images).unwrap().len() >= 5);
}

#[test]
fn noise_only() {
    let r = go(
        2.4e6,
        harness(2.4e6, 1.0)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Floor,
            )
            .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0),
    );
    assert_eq!(r.num("lines"), 0.0);
    assert_eq!(r.results.status().lock_state, LockState::Searching);
    assert!(!r.text("hint").is_empty());
}

#[test]
fn what_it_publishes_is_declared() {
    let r = go(2.4e6, impaired(harness(2.4e6, 4.0), Some(20.0), 0.0, 0.0));
    assert_declared(&r, &shape("fy2_svissr").results_decl());
}
