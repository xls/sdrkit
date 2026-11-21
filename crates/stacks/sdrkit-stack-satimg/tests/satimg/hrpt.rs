//! HRPT and AHRPT.

use super::common::{assert_declared, impaired, run, set, Ran};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Choice, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::modes::hrpt::{test_line, Variant, DESCRIPTOR, GEN_VARIANT};
use sdrkit_stack_satimg::typed::{VirtualChannelRow, VIRTUAL_CHANNELS};

/// Runs the impaired signal of `h`, with the strips of `avhrr.out` (or `noaa.out` for a NOAA signal).
fn go(h: Harness) -> Ran {
    let x = h.generate().expect("a signal");
    let noaa = h.values().expect("the settings").get(&GEN_VARIANT) == Variant::Noaa;
    let tap = if noaa { "noaa.out" } else { "avhrr.out" };
    run::<Strip>(&h, &x, 8e6, 1701.3e6, 1701.3e6, 65_536, Some(tap))
}

fn harness(secs: f64) -> Harness {
    Harness::new(&DESCRIPTOR).rate(8e6).seconds(secs)
}

/// The strips of channel 2 that equal a line of the test picture (any of the 60), and all the strips of channel 2.
fn exact_lines(strips: &[Strip]) -> (usize, usize) {
    let want: Vec<Vec<u8>> = (0..60)
        .map(|l| {
            test_line(l)[2048..4096]
                .iter()
                .map(|&v| (v >> 2) as u8)
                .collect()
        })
        .collect();
    let ch2: Vec<&Strip> = strips.iter().filter(|s| s.channel == 2).collect();
    let exact = ch2
        .iter()
        .filter(|s| want.iter().any(|w| *s.samples == w[..]))
        .count();
    (exact, ch2.len())
}

#[test]
fn metop_lines_are_exact() {
    let r = go(impaired(
        harness(0.7).set(
            &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
            sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
        ),
        Some(14.0),
        30e3,
        40.0,
    ));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(
        (r.num("offset_hz") - 30e3).abs() < 3000.0,
        "{}",
        r.num("offset_hz")
    );
    assert_eq!(r.text("variant"), "metop");
    let amb = r.num("phase_ambiguity");
    assert!((4.0..=7.0).contains(&amb), "{amb}");
    assert_eq!(r.num("phase_ambiguity_mirrored"), 1.0);
    let lines = r.num("picture_lines") as usize;
    assert!(lines >= 3, "{lines}");
    let (exact, all) = exact_lines(&r.of::<Strip>());
    assert!(all >= lines);
    assert_eq!(exact, all);
}

#[test]
fn metop_with_a_doppler_ramp_and_at_the_threshold() {
    let r = go(impaired(
        harness(1.0)
            .set(
                &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
                sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
            )
            .set(&sdrkit_algo_sim::chain::GEN_DRIFT_HZ_S, -400.0),
        Some(12.0),
        10e3,
        0.0,
    ));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    let r = go(impaired(
        harness(1.5).set(
            &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
            sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
        ),
        Some(2.5),
        30e3,
        0.0,
    ));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("picture_lines") >= 5.0, "{}", r.num("picture_lines"));
}

#[test]
fn fy3_frames_on_the_hrpt_chain() {
    let r = go(impaired(
        harness(0.6)
            .set(
                &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
                sdrkit_stack_satimg::modes::hrpt::VariantChoice::Fy3,
            )
            .set(
                &sdrkit_stack_satimg::modes::hrpt::GEN_VARIANT,
                sdrkit_stack_satimg::modes::hrpt::Variant::Fy3,
            ),
        Some(12.0),
        -20e3,
        0.0,
    ));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    let table = r.results.decl().table_id(VIRTUAL_CHANNELS.key).unwrap();
    let vc5 = r
        .results
        .table(table)
        .and_then(|t| t.row("5"))
        .and_then(|row| row.at(&VirtualChannelRow::FRAMES))
        .unwrap_or(0);
    assert!(vc5 > 20, "{vc5}");
    assert_eq!(r.num("picture_lines"), 0.0);
    assert!(r.num("frames_bad") < 3.0);
}

#[test]
fn noaa_lines_in_both_polarities() {
    for (pol, off) in [(false, 25e3), (true, -40e3)] {
        let r = go(impaired(
            harness(1.2)
                .set(
                    &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
                    sdrkit_stack_satimg::modes::hrpt::VariantChoice::Noaa,
                )
                .set(
                    &sdrkit_stack_satimg::modes::hrpt::GEN_VARIANT,
                    sdrkit_stack_satimg::modes::hrpt::Variant::Noaa,
                )
                .set(&sdrkit_stack_satimg::modes::hrpt::GEN_POLARITY, pol),
            Some(14.0),
            off,
            0.0,
        ));
        assert_eq!(r.results.status().lock_state, LockState::Locked, "{pol}");
        assert!(exact_lines(&r.of::<Strip>()).0 >= 4, "{pol}");
    }
}

#[test]
fn automatic_on_metop() {
    let r = go(impaired(harness(1.0), Some(14.0), 5e3, 0.0));
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.text("variant"), "metop");
    assert_eq!(r.text("variant_setting"), "auto");
}

#[test]
fn noise_for_each_variant() {
    for v in [
        sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
        sdrkit_stack_satimg::modes::hrpt::VariantChoice::Fy3,
        sdrkit_stack_satimg::modes::hrpt::VariantChoice::Noaa,
    ] {
        let r = go(harness(0.5)
            .set(&sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE, v)
            .set(&sdrkit_algo_sim::chain::GEN_LEVEL_DBFS, -150.0)
            .set(
                &sdrkit_algo_sim::chain::GEN_NOISE,
                sdrkit_algo_sim::chain::NoiseKind::Floor,
            )
            .set(&sdrkit_algo_sim::chain::GEN_NOISE_FLOOR_DBFS, -20.0));
        assert_eq!(
            r.num("frames_ok") + r.num("noaa_frames"),
            0.0,
            "{}",
            v.key()
        );
        assert_eq!(r.num("picture_lines"), 0.0, "{}", v.key());
        assert_ne!(
            r.results.status().lock_state,
            LockState::Locked,
            "{}",
            v.key()
        );
    }
}

#[test]
fn what_it_publishes_is_declared() {
    let r = go(impaired(
        harness(0.6).set(
            &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
            sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
        ),
        Some(14.0),
        0.0,
        0.0,
    ));
    assert_declared(&r, &sdrkit_stack_satimg::modes::hrpt::declaration());
    // A bad setting is refused.
    let mut rx = r.rx;
    assert!(set(&mut *rx, "variant", "goes").is_err());
    assert!(set(&mut *rx, "frequency_preset", "1700.000").is_err());
}

/// NOAA HRPT gives the HIRS picture from the TIP minor frames: at least one line from 40 HRPT lines, no parity error, and the
/// pixels of the test signal (its 13-bit words, 4096 plus the value, shifted right by 5).
#[test]
fn noaa_gives_the_hirs_picture() {
    use sdrkit_stack_satimg::instruments::tip::{HIRS_CHANNELS, HIRS_WIDTH};
    use sdrkit_stack_satimg::modes::hrpt::{
        hirs_test_value, VariantChoice, GEN_VARIANT, VARIANT_CHOICE,
    };
    let h = Harness::new(&DESCRIPTOR)
        .rate(4e6)
        .seconds(7.0)
        .set(&VARIANT_CHOICE, VariantChoice::Noaa)
        .set(&GEN_VARIANT, Variant::Noaa);
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, 4e6, 1701.3e6, 1701.3e6, 65_536, Some("hirs.out"));
    assert!(r.num("hirs_lines") >= 1.0, "{}", r.num("hirs_lines"));
    assert_eq!(r.num("hirs_parity_errors"), 0.0);
    // The first line starts where the frame lock started: its columns before that are empty. The columns that came hold the
    // test values.
    let strips = r.of::<Strip>();
    let row = strips
        .iter()
        .map(|s| s.first_row)
        .max()
        .expect("a HIRS line");
    let line: Vec<&Strip> = strips.iter().filter(|s| s.first_row == row).collect();
    assert_eq!(line.len(), 20);
    let came: Vec<usize> = (0..HIRS_WIDTH)
        .filter(|&e| line.iter().any(|s| s.samples[e] != 0))
        .collect();
    assert!(came.len() >= 40, "{} columns", came.len());
    let fits = |y: u32| {
        HIRS_CHANNELS.iter().enumerate().all(|(w, &c)| {
            let s = line
                .iter()
                .find(|s| s.channel == u16::from(c))
                .expect("every channel");
            came.iter().all(|&e| {
                u16::from(s.samples[e]) == (4096 + hirs_test_value(w as u16, e as u32, y)) >> 5
            })
        })
    };
    assert!((0..10).any(fits));
}

/// MetOp frames give no HIRS picture.
#[test]
fn metop_gives_no_hirs_picture() {
    let r = go(harness(0.7).set(
        &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
        sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
    ));
    assert_eq!(r.num("hirs_lines"), 0.0);
    assert_eq!(r.num("hirs_valid"), 0.0);
}

/// A change of frequency keeps the images and forgets the frames of each virtual channel.
#[test]
fn a_change_of_frequency_keeps_the_images() {
    use sdrkit_core::Owner;
    let mut r = go(impaired(
        harness(0.7).set(
            &sdrkit_stack_satimg::modes::hrpt::VARIANT_CHOICE,
            sdrkit_stack_satimg::modes::hrpt::VariantChoice::Metop,
        ),
        Some(14.0),
        30e3,
        40.0,
    ));
    let rows = |rx: &dyn sdrkit_core::Receiver, key: &str| {
        let r = Owner::results(rx);
        r.decl()
            .table_id(key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.len())
    };
    let images = rows(r.rx.as_ref(), "images");
    assert!(images > 0);
    assert!(rows(r.rx.as_ref(), "virtual_channels") > 0);
    r.rx.forget();
    assert_eq!(rows(r.rx.as_ref(), "images"), images);
    assert_eq!(rows(r.rx.as_ref(), "virtual_channels"), 0);
}
