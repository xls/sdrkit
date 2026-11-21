//! FY-3 AHRPT and MPT.

use super::common::{assert_declared, impaired, run, set, Ran};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{LockState, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::modes::fy3::{test_value, AHRPT, MPT};

fn harness(d: &'static StackDescriptor, rate: f64, secs: f64) -> Harness {
    Harness::new(d).rate(rate).seconds(secs)
}

fn go(d: &'static StackDescriptor, h: Harness, rate: f64) -> Ran {
    let x = h.generate().expect("a signal");
    let f = d.tuning.default_hz;
    run::<Strip>(&h, &x, rate, f, f, 65_536, Some("instruments.out"))
}

/// The rows of channel `ch` of `instrument` that equal a row of the test picture, with the samples kept as the top 8 of their
/// bits (`shift`).
fn exact_rows(strips: &[Strip], instrument: &str, ch: u16, w: usize, shift: u32) -> usize {
    let mut n = 0;
    for s in strips
        .iter()
        .filter(|s| s.instrument == instrument && s.channel == ch && s.width as usize == w)
    {
        for row in s.samples.chunks(w) {
            if (0..400).any(|gy| {
                (0..w).all(|x| row[x] == (test_value(ch as usize, gy, x, w) >> shift) as u8)
            }) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn fy3c_virr_rows_are_exact() {
    let r = go(
        &AHRPT,
        impaired(
            harness(&AHRPT, 8e6, 1.5).with_settings(&[sdrkit_stack_satimg::verify::entry(
                &sdrkit_stack_satimg::modes::fy3::AHRPT_PRESET,
                "fy3c",
            )]),
            Some(30.0),
            0.0,
            0.0,
        ),
        8e6,
    );
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.text("satellite"), "FY-3C");
    assert!(r.num("frames_bad") <= 3.0 + r.num("frames_ok") / 200.0);
    assert!(r.num("picture_rows") >= 6.0, "{}", r.num("picture_rows"));
    // VIRR samples are 10 bits and the picture keeps the top 8: test values >> 6 >> 2.
    assert!(exact_rows(&r.of::<Strip>(), "VIRR", 1, 2048, 8) >= 6);
}

#[test]
fn rails_found_under_q_negated_and_exchanged() {
    for (q, s) in [(true, false), (false, true), (true, true)] {
        let r = go(
            &AHRPT,
            impaired(
                harness(&AHRPT, 8e6, 1.2)
                    .with_settings(&[sdrkit_stack_satimg::verify::entry(
                        &sdrkit_stack_satimg::modes::fy3::AHRPT_PRESET,
                        "fy3c",
                    )])
                    .set(&sdrkit_stack_satimg::modes::fy3::GEN_QNEG, q)
                    .set(&sdrkit_stack_satimg::modes::fy3::GEN_SWAP, s),
                Some(15.0),
                12e3,
                -30.0,
            ),
            8e6,
        );
        assert_eq!(
            r.results.status().lock_state,
            LockState::Locked,
            "q {q} swap {s}"
        );
        assert!(r.num("picture_rows") >= 4.0);
    }
}

#[test]
fn automatic_measures_or_tries_the_rate() {
    let r = go(
        &AHRPT,
        impaired(
            harness(&AHRPT, 4e6, 4.0)
                .set(&sdrkit_stack_satimg::modes::fy3::GEN_SYMBOL_RATE_AHRPT, 2.8)
                .set(&sdrkit_stack_satimg::modes::fy3::GEN_SATELLITE, 49),
            Some(15.0),
            12e3,
            0.0,
        ),
        4e6,
    );
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.text("satellite"), "FY-3A");
    assert_eq!(r.num("symbol_rate_hz"), 2.8e6);
}

#[test]
fn mpt_mersi_scans() {
    let r = go(
        &MPT,
        impaired(
            harness(&MPT, 12e6, 5.5)
                .set(&sdrkit_stack_satimg::modes::fy3::GEN_SYMBOL_RATE_MPT, 6.0)
                .set(&sdrkit_stack_satimg::modes::fy3::SYMBOL_RATE, 6.0)
                .set(&sdrkit_stack_satimg::modes::fy3::ROLL_OFF, 0.5),
            Some(15.0),
            20e3,
            0.0,
        ),
        12e6,
    );
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("frames_bad") <= 3.0);
    // The 1 km channel 6: 12 bits shown as the top 8.
    assert!(exact_rows(&r.of::<Strip>(), "MERSI", 6, 2048, 8) >= 8);
}

#[test]
fn fy3d_mwri() {
    let r = go(
        &AHRPT,
        impaired(
            harness(&AHRPT, 8e6, 1.5)
                .with_settings(&[sdrkit_stack_satimg::verify::entry(
                    &sdrkit_stack_satimg::modes::fy3::AHRPT_PRESET,
                    "fy3c",
                )])
                .set(&sdrkit_stack_satimg::modes::fy3::GEN_SATELLITE, 52),
            Some(20.0),
            0.0,
            0.0,
        ),
        8e6,
    );
    assert_eq!(r.text("satellite"), "FY-3D");
    assert!(exact_rows(&r.of::<Strip>(), "MWRI", 3, 266, 8) >= 20);
}

#[test]
fn noise_returns_a_hint() {
    let r = go(
        &AHRPT,
        harness(&AHRPT, 8e6, 1.0)
            .with_settings(&[sdrkit_stack_satimg::verify::entry(
                &sdrkit_stack_satimg::modes::fy3::AHRPT_PRESET,
                "fy3c",
            )])
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Floor,
            )
            .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0),
        8e6,
    );
    assert_eq!(r.num("frames_ok"), 0.0);
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert!(!r.text("hint").is_empty());
}

#[test]
fn what_it_publishes_is_declared() {
    let r = go(
        &AHRPT,
        impaired(
            harness(&AHRPT, 8e6, 1.0).with_settings(&[sdrkit_stack_satimg::verify::entry(
                &sdrkit_stack_satimg::modes::fy3::AHRPT_PRESET,
                "fy3c",
            )]),
            Some(20.0),
            0.0,
            0.0,
        ),
        8e6,
    );
    assert_declared(&r, &sdrkit_stack_satimg::typed::shape(AHRPT.id).results_decl());
    let mut rx = r.rx;
    assert!(set(&mut *rx, "roll_off", "1.5").is_err());
    assert!(set(&mut *rx, "preset", "mpt_c").is_err());
}
