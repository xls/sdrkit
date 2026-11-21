//! Systems, colour combinations and SECAM.

use super::common::*;
use sdrkit_stack_analogtv::gen_video::SecamIdent;
use sdrkit_stack_analogtv::sound_gen::Programme;
use sdrkit_stack_analogtv::systems::{Colour, System, SystemId};
use sdrkit_stack_analogtv::{AtvGenParams, AtvGenerator};

/// System, colour, rate, seconds, name, bars limit.
type Case = (SystemId, Option<Colour>, f64, f64, &'static str, f64);
/// System, colour, rate, seconds, system setting, name, bars limit.
type Forced = (
    SystemId,
    Option<Colour>,
    f64,
    f64,
    &'static str,
    &'static str,
    f64,
);

fn p(system: SystemId, colour: Option<Colour>, rate: f64) -> AtvGenParams {
    AtvGenParams {
        system,
        colour,
        sound: Programme::Continuous,
        ..params(rate)
    }
}

#[test]
fn y2_the_generator_refuses_what_did_not_exist() {
    assert!(!AtvGenerator::new(p(SystemId::A, Some(Colour::Pal), 10e6)).is_valid());
    assert!(!AtvGenerator::new(p(SystemId::L, Some(Colour::Ntsc), 10e6)).is_valid());
    assert!(AtvGenerator::new(p(SystemId::M, Some(Colour::PalM), 10e6)).is_valid());
    assert!(AtvGenerator::new(p(SystemId::G, Some(Colour::Mono), 10e6)).is_valid());
    assert!(System::from_key("x").is_none());
}

fn check(
    id: SystemId,
    colour: Option<Colour>,
    rate: f64,
    seconds: f64,
    settings: &[(&str, &str)],
    name: &str,
    bars: f64,
) {
    let (r, _) = gen_run(p(id, colour, rate), seconds, settings);
    let d = r.last();
    assert_eq!(d.state, 2, "{name}");
    assert!(d.data_valid, "{name}");
    assert_eq!(d.system, name);
    let fh = if d.lines == 525 { 59.94 } else { 50.0 };
    assert!(
        (d.field_hz - fh).abs() < 0.01,
        "{name}: field {}",
        d.field_hz
    );
    assert!(
        d.fields_ok > 20 && d.fields_bad <= 3,
        "{name}: {} / {}",
        d.fields_ok,
        d.fields_bad
    );
    assert!(
        d.carrier_offset_hz.abs() < 1500.0,
        "{name}: offset {}",
        d.carrier_offset_hz
    );
    let pic = r.picture.as_ref().unwrap();
    if d.colour_system == "mono" {
        assert!(grey(pic) < 0.03, "{name}");
        assert!(luma_error(pic) < bars, "{name}: luma {}", luma_error(pic));
    } else {
        let e = bars_error(pic);
        assert!(e < bars, "{name}: bars {e}");
    }
    if colour != Some(Colour::Secam) || !matches!(id, SystemId::M | SystemId::N) {
        assert!(d.sound_present, "{name}: sound");
        let (l, _) = r.tail(0.2);
        assert!(tone(&l, 1000.0) > 0.3, "{name}: tone {}", tone(&l, 1000.0));
    }
}

#[test]
fn y3_forced_systems() {
    let cases: Vec<Forced> = vec![
        (
            SystemId::A,
            Some(Colour::Mono),
            10e6,
            2.0,
            "a",
            "A 405/50 mono",
            0.06,
        ),
        (
            SystemId::C,
            Some(Colour::Mono),
            10e6,
            2.0,
            "c",
            "C 625/50 mono",
            0.06,
        ),
        (
            SystemId::E,
            Some(Colour::Mono),
            20e6,
            1.6,
            "e",
            "E 819/50 mono",
            0.06,
        ),
        (
            SystemId::L,
            Some(Colour::Secam),
            10e6,
            2.5,
            "l",
            "SECAM L 625/50",
            0.12,
        ),
        (
            SystemId::N,
            Some(Colour::NtscN),
            10e6,
            2.5,
            "n",
            "NTSC-N 625/50",
            0.10,
        ),
        (
            SystemId::M,
            Some(Colour::Secam),
            10e6,
            2.5,
            "m",
            "SECAM M 525/59.94",
            0.14,
        ),
        (
            SystemId::H,
            Some(Colour::Pal),
            10e6,
            2.0,
            "h",
            "PAL H 625/50",
            0.09,
        ),
        (
            SystemId::Kp,
            Some(Colour::Secam),
            10e6,
            2.5,
            "kp",
            "SECAM K' 625/50",
            0.12,
        ),
    ];
    std::thread::scope(|s| {
        for (id, c, rate, secs, key, name, bars) in cases {
            s.spawn(move || {
                // SECAM chosen on M takes the 5.5 MHz plan.
                if id == SystemId::M && c == Some(Colour::Secam) {
                    check(
                        id,
                        c,
                        rate,
                        secs,
                        &[("system", key), ("colour", "secam")],
                        name,
                        bars,
                    )
                } else {
                    check(id, c, rate, secs, &[("system", key)], name, bars)
                }
            });
        }
    });
}

#[test]
fn y4_found_without_forcing() {
    let cases: Vec<Case> = vec![
        (
            SystemId::I,
            Some(Colour::Ntsc443),
            10e6,
            2.0,
            "NTSC 4.43 I 625/50",
            0.10,
        ),
        (
            SystemId::A,
            Some(Colour::Mono),
            10e6,
            2.5,
            "A 405/50 mono",
            0.06,
        ),
        (
            SystemId::E,
            Some(Colour::Mono),
            20e6,
            2.0,
            "E 819/50 mono",
            0.06,
        ),
        (
            SystemId::L,
            Some(Colour::Secam),
            10e6,
            2.5,
            "SECAM L 625/50",
            0.12,
        ),
        (
            SystemId::Lp,
            Some(Colour::Secam),
            10e6,
            2.5,
            "SECAM L' 625/50",
            0.12,
        ),
        (
            SystemId::C,
            Some(Colour::Mono),
            10e6,
            2.0,
            "C 625/50 mono",
            0.06,
        ),
    ];
    std::thread::scope(|s| {
        for (id, c, rate, secs, name, bars) in cases {
            s.spawn(move || {
                let settings: &[(&str, &str)] = if id == SystemId::E {
                    &[("system", "e")]
                } else {
                    &[]
                };
                check(id, c, rate, secs, settings, name, bars)
            });
        }
    });
}

#[test]
fn y5_positive_modulation_off_frequency() {
    std::thread::scope(|s| {
        for (id, key, off, name) in [
            (SystemId::L, "l", 50e3, "L 625/50 mono"),
            (SystemId::A, "a", -50e3, "A 405/50 mono"),
        ] {
            s.spawn(move || {
                let q = p(id, Some(Colour::Mono), 10e6);
                let imp = Imp {
                    cnr_db: 40.0,
                    offset_hz: off,
                    ..Default::default()
                };
                let (r, _) = imp_run(q, imp, 2.0, &[("system", key)]);
                let d = r.last();
                assert_eq!(d.state, 2);
                assert_eq!(d.system, name);
                assert!(
                    (d.carrier_offset_hz - off).abs() < 1500.0,
                    "{name}: {}",
                    d.carrier_offset_hz
                );
            });
        }
    });
}

#[test]
fn f1_secam_identification_each_way() {
    std::thread::scope(|s| {
        for (id, how) in [
            (SystemId::DK, SecamIdent::Both),
            (SystemId::DK, SecamIdent::BackPorch),
            (SystemId::DK, SecamIdent::FieldLines),
            (SystemId::L, SecamIdent::FieldLines),
        ] {
            s.spawn(move || {
                let mut q = p(id, Some(Colour::Secam), 10e6);
                q.secam_ident = how;
                let (r, g) = gen_run(q, 2.0, &[]);
                let d = r.last();
                assert_eq!(d.state, 2, "{id:?} {how:?}");
                assert_eq!(d.colour_system, "SECAM", "{id:?} {how:?}");
                assert!(d.colour);
                assert!(d.fields_bad <= 3);
                assert!(
                    (d.chroma_level - 1.0).abs() < 0.25,
                    "{how:?}: chroma {}",
                    d.chroma_level
                );
                assert!(!d.secam_identification.is_empty());
                let pic = r.picture.as_ref().unwrap();
                assert!(
                    bars_error(pic) < 0.07,
                    "{id:?} {how:?}: bars {}",
                    bars_error(pic)
                );
                assert!(
                    luma_difference(pic, g.card()) < 6.0 + 3.0,
                    "luma {}",
                    luma_difference(pic, g.card())
                );
            });
        }
    });
}

#[test]
fn f2_f4_secam_in_noise_and_never_as_pal() {
    std::thread::scope(|s| {
        for (cnr, bars) in [(30.0, 0.08), (20.0, 0.17)] {
            s.spawn(move || {
                let (r, _) = imp_run(
                    p(SystemId::DK, Some(Colour::Secam), 10e6),
                    Imp {
                        cnr_db: cnr,
                        ..Default::default()
                    },
                    2.0,
                    &[],
                );
                let d = r.last();
                assert_eq!(d.state, 2);
                assert_eq!(d.colour_system, "SECAM", "{cnr}");
                assert!(
                    bars_error(r.picture.as_ref().unwrap()) < bars,
                    "{cnr}: {}",
                    bars_error(r.picture.as_ref().unwrap())
                );
            });
        }
        s.spawn(|| {
            let (r, _) = gen_run(p(SystemId::DK, Some(Colour::Pal), 10e6), 1.25, &[]);
            assert_eq!(r.last().colour_system, "PAL");
        });
        s.spawn(|| {
            let (r, _) = gen_run(p(SystemId::DK, Some(Colour::Mono), 10e6), 1.5, &[]);
            assert_eq!(r.last().colour_system, "mono");
        });
    });
}

#[test]
fn n11_colour_systems_at_24_db() {
    let cases = [
        (SystemId::G, Colour::Pal, "PAL"),
        (SystemId::M, Colour::Ntsc, "NTSC"),
        (SystemId::M, Colour::PalM, "PAL-M"),
        (SystemId::N, Colour::PalN, "PAL-N"),
        (SystemId::G, Colour::Secam, "SECAM"),
    ];
    std::thread::scope(|s| {
        for (id, c, name) in cases {
            s.spawn(move || {
                let (r, _) = imp_run(
                    p(id, Some(c), 10e6),
                    Imp {
                        cnr_db: 24.0,
                        ..Default::default()
                    },
                    2.0,
                    &[],
                );
                let d = r.last();
                assert_eq!(d.state, 2, "{name}");
                assert_eq!(d.colour_system, name);
                assert!(d.colour, "{name}");
                assert!(
                    bars_error(r.picture.as_ref().unwrap()) < 0.16,
                    "{name}: {}",
                    bars_error(r.picture.as_ref().unwrap())
                );
            });
        }
    });
}
