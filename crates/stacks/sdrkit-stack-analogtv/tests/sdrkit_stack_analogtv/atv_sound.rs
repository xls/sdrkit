//! The sound of `atv`: FM and AM carriers, NICAM-728 and A2.

use super::common::*;
use sdrkit_stack_analogtv::sound_gen::{Programme, SecondSound};
use sdrkit_stack_analogtv::systems::{Colour, SystemId};
use sdrkit_stack_analogtv::AtvGenParams;

fn p(system: SystemId, sound: Programme, second: SecondSound) -> AtvGenParams {
    AtvGenParams {
        system,
        sound,
        second,
        ..params(10e6)
    }
}

/// The tone against everything else, dB.
fn tone_to_rest(x: &[f32], f: f64) -> f64 {
    let a = tone(x, f);
    let total = sdrkit_dev_test::measure::rms(x).powi(2);
    let t = a * a / 2.0;
    10.0 * (t / (total - t).max(1e-12)).log10()
}

#[test]
fn a1_fm_tones_on_every_spacing() {
    std::thread::scope(|s| {
        for (id, spacing, nominal, tau) in [
            (SystemId::G, 5.5, 50.0, 50e-6),
            (SystemId::I, 6.0, 50.0, 50e-6),
            (SystemId::DK, 6.5, 50.0, 50e-6),
            (SystemId::M, 4.5, 25.0, 75e-6),
        ] {
            s.spawn(move || {
                let (r, _) = gen_run(p(id, Programme::Continuous, SecondSound::None), 1.25, &[]);
                let d = r.last();
                assert_eq!(d.state, 2, "{id:?}");
                assert!(d.sound_present);
                assert!((d.sound_spacing_mhz - spacing).abs() < 0.01);
                assert!(
                    ((d.sound_hz - d.vision_hz) - spacing * 1e6).abs() < 3000.0,
                    "{id:?}: {}",
                    d.sound_hz - d.vision_hz
                );
                let (l, rr) = r.tail(0.2);
                let a = tone(&l, 1000.0);
                assert!((a - 0.5).abs() < 0.04, "{id:?}: {a}");
                assert!((tone(&rr, 1000.0) - a).abs() < 0.01);
                assert!(
                    tone_to_rest(&l, 1000.0) > 38.0,
                    "{id:?}: {}",
                    tone_to_rest(&l, 1000.0)
                );
                let want =
                    0.5 * nominal * (1.0 + (std::f64::consts::TAU * 1000.0 * tau).powi(2)).sqrt();
                assert!(
                    (d.sound_deviation_khz - want).abs() < 0.1 * nominal,
                    "{id:?}: dev {} want {want}",
                    d.sound_deviation_khz
                );
                assert!(
                    (d.sound_level_dbfs + 9.0).abs() < 1.5,
                    "{id:?}: level {}",
                    d.sound_level_dbfs
                );
                assert_eq!(d.sound_system, "FM");
            });
        }
    });
}

#[test]
fn a2_a3_melody_and_gaps() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let (r, _) = gen_run(
                p(SystemId::G, Programme::Melody, SecondSound::None),
                2.2,
                &[],
            );
            let left: Vec<f32> = r.audio.chunks(2).map(|c| c[0]).collect();
            // Notes of 0.35 s: 261.63, 261.63, 392, 392, 440, 440, 392, rest, ...; 0.2 s from the middle of three of them.
            for (t0, f) in [(0.77, 392.0), (1.47, 440.0), (1.82, 440.0)] {
                let a = (t0 * 48_000.0) as usize;
                let x = &left[a..a + 9_600];
                let amp = tone(x, f);
                assert!((amp - 0.5).abs() < 0.05, "{f} at {t0}: {amp}");
            }
        });
        s.spawn(|| {
            let (r, _) = gen_run(
                p(SystemId::G, Programme::ToneGaps, SecondSound::None),
                1.5,
                &[],
            );
            let left: Vec<f32> = r.audio.chunks(2).map(|c| c[0]).collect();
            // The receiver's audio delay is well under 0.05 s; the tone runs to 1.3 s, the gap from 1.3 to 1.5 s.
            let on =
                sdrkit_dev_test::measure::rms(&left[(0.9 * 48e3) as usize..(1.2 * 48e3) as usize]);
            let off = sdrkit_dev_test::measure::rms(
                &left[(1.38 * 48e3) as usize..(1.48 * 48e3) as usize],
            );
            assert!((on - 0.354).abs() < 0.04, "on {on}");
            assert!(off < 0.02, "off {off}");
        });
    });
}

#[test]
fn a4_unmodulated_and_no_carrier() {
    std::thread::scope(|s| {
        s.spawn(|| {
            let (r, _) = gen_run(
                p(SystemId::G, Programme::Unmodulated, SecondSound::None),
                1.25,
                &[],
            );
            let d = r.last();
            assert!(d.sound_present);
            let (l, _) = r.tail(0.2);
            assert!(sdrkit_dev_test::measure::rms(&l) < 0.01);
        });
        s.spawn(|| {
            let (r, _) = gen_run(
                p(SystemId::G, Programme::NoCarrier, SecondSound::None),
                1.5,
                &[],
            );
            let d = r.last();
            assert_eq!(d.state, 2);
            assert_eq!(d.system, "PAL B/G 625/50");
            assert!(!d.sound_present);
            let (l, _) = r.tail(0.2);
            assert!(l.iter().all(|x| x.abs() < 0.01));
        });
    });
}

#[test]
fn a8_nicam_stereo_and_dual() {
    std::thread::scope(|s| {
        for (id, second, dual) in [
            (SystemId::G, SecondSound::NicamStereo, "first"),
            (SystemId::I, SecondSound::NicamStereo, "first"),
            (SystemId::G, SecondSound::NicamDual, "first"),
            (SystemId::G, SecondSound::NicamDual, "second"),
        ] {
            s.spawn(move || {
                let (r, _) = gen_run(
                    p(id, Programme::Continuous, second),
                    1.5,
                    &[("dual_sound", dual)],
                );
                let d = r.last();
                assert_eq!(d.state, 2);
                let n = d.nicam.clone().unwrap();
                assert!(n.locked && n.valid, "{id:?} {second:?}: {n:?}");
                assert_eq!(n.parity_errors, 0);
                let (l, rr) = r.tail(0.2);
                if second == SecondSound::NicamStereo {
                    assert_eq!(n.mode, "stereo");
                    assert_eq!(d.sound_system, "NICAM stereo");
                    let (a, b) = (tone(&l, 1000.0), tone(&rr, 1500.0));
                    assert!(
                        a > 0.05 && b > 0.05 && (20.0 * (a / b).log10()).abs() < 3.0,
                        "{a} {b}"
                    );
                    assert!(tone(&l, 1500.0) < a / 31.6 && tone(&rr, 1000.0) < b / 31.6);
                } else {
                    assert_eq!(n.mode, "dual mono");
                    let (want, other) = if dual == "first" {
                        (1000.0, 1500.0)
                    } else {
                        (1500.0, 1000.0)
                    };
                    let name = if dual == "first" {
                        "NICAM dual mono, channel 1"
                    } else {
                        "NICAM dual mono, channel 2"
                    };
                    assert_eq!(d.sound_system, name);
                    for x in [&l, &rr] {
                        assert!(tone(x, want) > 31.6 * tone(x, other), "{dual}");
                    }
                }
            });
        }
    });
}

#[test]
fn a9_a2_stereo_dual_mono() {
    std::thread::scope(|s| {
        for (second, mode) in [
            (SecondSound::A2Stereo, "stereo"),
            (SecondSound::A2Dual, "dual"),
            (SecondSound::A2Mono, "mono"),
        ] {
            s.spawn(move || {
                let (r, _) = gen_run(p(SystemId::G, Programme::Continuous, second), 2.5, &[]);
                let d = r.last();
                assert_eq!(d.state, 2);
                let a2 = d.a2.clone().unwrap();
                assert!(a2.present, "{mode}: {a2:?}");
                assert_eq!(a2.mode, mode);
                let (l, rr) = r.tail(0.2);
                match mode {
                    "stereo" => {
                        assert_eq!(d.sound_system, "A2 stereo");
                        let (a, b) = (tone(&l, 1000.0), tone(&rr, 1500.0));
                        assert!(a > 0.03 && b > 0.03, "{a} {b}");
                        assert!(
                            tone(&l, 1500.0) < a / 17.8 && tone(&rr, 1000.0) < b / 17.8,
                            "crosstalk"
                        );
                    }
                    "dual" => {
                        assert_eq!(d.sound_system, "A2 dual, sound A");
                        assert!(tone(&l, 1000.0) > 17.8 * tone(&l, 1500.0));
                        assert!(tone(&rr, 1000.0) > 17.8 * tone(&rr, 1500.0));
                    }
                    _ => {
                        assert_eq!(d.sound_system, "FM mono (A2 pilot)");
                        let (a, b) = (
                            sdrkit_dev_test::measure::rms(&l),
                            sdrkit_dev_test::measure::rms(&rr),
                        );
                        assert!((20.0 * (a / b).log10()).abs() < 1.0);
                    }
                }
            });
        }
    });
}

#[test]
fn carrier_only_ignores_nicam() {
    let (r, _) = gen_run(
        p(SystemId::G, Programme::Continuous, SecondSound::NicamStereo),
        1.25,
        &[("sound_system", "carrier")],
    );
    let d = r.last();
    assert_eq!(d.sound_system, "FM");
    let (l, rr) = r.tail(0.2);
    assert!((tone(&l, 1000.0) - 0.5).abs() < 0.04 && (tone(&rr, 1000.0) - 0.5).abs() < 0.04);
}

#[test]
fn am_sound_on_the_positive_systems() {
    let (r, _) = gen_run(
        AtvGenParams {
            colour: Some(Colour::Mono),
            ..p(SystemId::L, Programme::Continuous, SecondSound::None)
        },
        1.5,
        &[],
    );
    let d = r.last();
    assert_eq!(d.state, 2);
    assert_eq!(d.sound_system, "AM");
    assert!(d.sound_present);
    let (l, _) = r.tail(0.2);
    assert!(
        (tone(&l, 1000.0) - 0.5).abs() < 0.06,
        "{}",
        tone(&l, 1000.0)
    );
}
