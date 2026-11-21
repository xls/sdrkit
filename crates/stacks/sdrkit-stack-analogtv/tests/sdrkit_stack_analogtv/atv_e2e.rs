//! `atv` end to end and its controls: generator, impairments, receiver.

use super::common::*;
use sdrkit_core::{Complex32, InputInfo, Output, Owner, Receiver, SettingSet};
use sdrkit_stack_analogtv::sound_gen::Programme;
use sdrkit_stack_analogtv::systems::{Colour, SystemId};
use sdrkit_stack_analogtv::AtvGenParams;

fn sys(system: SystemId, colour: Option<Colour>) -> AtvGenParams {
    AtvGenParams {
        system,
        colour,
        ..params(10e6)
    }
}

#[test]
fn e1_pal_bg_locks_and_decodes_everything() {
    let p = sys(SystemId::G, None);
    let (r, g) = imp_run(p, Imp::cnr(45.0), 2.5, &[]);
    let d = r.last();
    assert_eq!(d.state, 2);
    assert!(d.data_valid);
    assert_eq!(d.system, "PAL B/G 625/50");
    assert_eq!(d.colour_system, "PAL");
    assert!(d.colour);
    assert_eq!(d.lines, 625);
    assert!(r.first(|d| d.state == 2).unwrap() < 1.0, "lock time");
    assert!(d.field_count > 80, "fields {}", d.field_count);
    assert!(d.pictures > 40, "pictures {}", d.pictures);
    assert!(d.fields_bad <= 2, "bad fields {}", d.fields_bad);
    assert!(d.line_error_ppm.abs() < 6.0, "ppm {}", d.line_error_ppm);
    assert!((d.field_hz - 50.0).abs() < 0.005);
    assert!(
        (d.vision_hz + 2.75e6).abs() < 300.0,
        "vision {}",
        d.vision_hz
    );
    assert!(d.carrier_offset_hz.abs() < 300.0);
    assert_eq!(d.sound_spacing_mhz, 5.5);
    assert!((d.sound_hz - 2.75e6).abs() < 1500.0, "sound {}", d.sound_hz);
    assert!(
        (d.sync_depth_pct - 26.25).abs() < 1.0,
        "depth {}",
        d.sync_depth_pct
    );
    assert!(
        (d.burst_level - 1.0).abs() < 0.12,
        "burst {}",
        d.burst_level
    );
    assert!(d.synchronous_detector);
    assert!(d.video_snr_db > 30.0, "snr {}", d.video_snr_db);
    assert!(
        (d.carrier_to_noise_db - 45.0).abs() < 1.5,
        "C/N {}",
        d.carrier_to_noise_db
    );
    let p = r.picture.as_ref().unwrap();
    let l = p.picture.layout.as_ref().unwrap();
    assert_eq!((l.width, l.height), (768, 576));
    assert!(p.colour);
    assert!(bars_error(p) < 0.09, "bars {}", bars_error(p));
    assert!(
        luma_difference(p, g.card()) < 4.5 + 3.0,
        "luma {}",
        luma_difference(p, g.card())
    );
    // The sound: 1 kHz with gaps; the last 0.2 s is inside a tone stretch (2.3 to 2.5 s: tone from 1.5 to 2.8 s).
    let (left, right) = r.tail(0.15);
    let a = tone(&left, 1000.0);
    assert!((a - 0.5).abs() < 0.06, "tone {a}");
    assert!((tone(&right, 1000.0) - a).abs() < 0.01);
    assert!(d.sound_present);
}

#[test]
fn e2_systems_lock_with_their_names() {
    let cases = [
        (SystemId::B, None, "PAL B/G 625/50", 5.5, -2.25e6),
        (SystemId::I, None, "PAL I 625/50", 6.0, -2.75e6),
        (
            SystemId::DK,
            Some(Colour::Pal),
            "PAL D/K 625/50",
            6.5,
            -2.75e6,
        ),
        (SystemId::M, None, "NTSC M 525/59.94", 4.5, -1.75e6),
        (
            SystemId::M,
            Some(Colour::PalM),
            "PAL-M 525/59.94",
            4.5,
            -1.75e6,
        ),
        (SystemId::N, None, "PAL-N 625/50", 4.5, -1.75e6),
    ];
    std::thread::scope(|s| {
        for (id, col, name, spacing, vision) in cases {
            s.spawn(move || {
                let mut p = sys(id, col);
                p.sound = Programme::Continuous;
                let (r, _) = gen_run(p, 2.0, &[]);
                let d = r.last();
                assert_eq!(d.state, 2, "{name}");
                assert_eq!(d.system, name);
                assert!(d.colour, "{name} colour");
                assert!(
                    (d.sound_spacing_mhz - spacing).abs() < 0.01,
                    "{name} spacing {}",
                    d.sound_spacing_mhz
                );
                assert!(
                    (d.vision_hz - vision).abs() < 400.0,
                    "{name} vision {}",
                    d.vision_hz
                );
                assert!(
                    d.line_error_ppm.abs() < 8.0,
                    "{name} ppm {}",
                    d.line_error_ppm
                );
                assert!(d.fields_bad <= 3, "{name} bad {}", d.fields_bad);
                assert!(d.sound_present, "{name} sound");
                let nominal = if spacing == 4.5 { 25.0 } else { 50.0 };
                assert!(
                    d.sound_deviation_khz > 0.4 * nominal,
                    "{name} deviation {}",
                    d.sound_deviation_khz
                );
                let p = r.picture.as_ref().unwrap();
                assert!(bars_error(p) < 0.10, "{name} bars {}", bars_error(p));
            });
        }
    });
}

#[test]
fn e3_mono_is_grey() {
    let (r, _) = gen_run(sys(SystemId::G, Some(Colour::Mono)), 1.5, &[]);
    let d = r.last();
    assert_eq!(d.state, 2);
    assert!(!d.colour);
    assert_eq!(d.colour_system, "mono");
    let p = r.picture.as_ref().unwrap();
    assert!(grey(p) < 0.02, "grey {}", grey(p));
    assert!(luma_error(p) < 0.08, "luma {}", luma_error(p));
}

#[test]
fn e4_pal_at_8_msps_is_monochrome() {
    let (r, _) = gen_run(
        AtvGenParams {
            rate_hz: 8e6,
            ..params(8e6)
        },
        1.5,
        &[],
    );
    let d = r.last();
    assert_eq!(d.state, 2);
    assert!(!d.colour);
}

#[test]
fn e5_noise_and_a_bare_carrier_give_nothing() {
    for carrier in [false, true] {
        let rate = 10e6;
        let mut rx = atv(rate, &[]);
        let mut n = noise(0.1 / 2f64.sqrt(), 3);
        let mut ph = 0.0f64;
        let r = run(&mut rx, rate, 1.5, 50_000, |b| {
            b.fill(Complex32::default());
            n.process(b);
            for x in b.iter_mut() {
                if carrier {
                    *x += Complex32::from_polar(0.2, ph as f32);
                    ph = (ph + std::f64::consts::TAU * 1e6 / rate) % std::f64::consts::TAU;
                }
            }
        });
        assert!(
            r.reports.iter().all(|d| d.state == 0 && !d.data_valid),
            "carrier {carrier}"
        );
        assert_eq!(r.pictures, 0);
    }
}

#[test]
fn e6_low_rates_are_refused() {
    for rate in [2e6, 4e6, 6e6] {
        let values = values_of(&sdrkit_stack_analogtv::settings::AtvSetting::schema(), &[]);
        let mut rx =
            sdrkit_stack_analogtv::atv::graph::receiver(&sdrkit_core::StackConfig::new(values))
                .unwrap();
        assert!(rx.configure(&InputInfo::new(rate, 600e6, 600e6)).is_err());
        assert!(Owner::results(&rx)
            .status()
            .summary
            .contains("at least 8 Msps"));
        let mut out = Output::default();
        rx.feed(&[Complex32::default(); 1000], &mut out);
    }
}

#[test]
fn e7_rates() {
    std::thread::scope(|s| {
        for rate in [8e6, 12.5e6, 16e6, 20e6] {
            s.spawn(move || {
                let mut p = params(rate);
                p.sound = Programme::Continuous;
                let (r, _) = gen_run(p, 1.5, &[]);
                let d = r.last();
                assert_eq!(d.state, 2, "{rate}");
                let p = r.picture.as_ref().unwrap();
                if rate < 9.5e6 {
                    assert!(!d.colour);
                } else {
                    assert!(d.colour, "{rate} colour");
                    assert!(bars_error(p) < 0.06, "{rate} bars {}", bars_error(p));
                }
            });
        }
    });
}

#[test]
fn n1_noise() {
    std::thread::scope(|s| {
        for (cnr, bars) in [(30.0, 0.08), (22.0, 0.15)] {
            s.spawn(move || {
                let mut p = params(10e6);
                p.sound = Programme::Continuous;
                let (r, _) = imp_run(
                    p,
                    Imp {
                        cnr_db: cnr,
                        ..Default::default()
                    },
                    1.75,
                    &[],
                );
                let d = r.last();
                assert_eq!(d.state, 2, "{cnr}");
                assert!(d.colour, "{cnr} colour");
                assert!(
                    (d.carrier_to_noise_db - cnr).abs() < 1.5,
                    "{cnr}: C/N {}",
                    d.carrier_to_noise_db
                );
                assert!(
                    (d.video_snr_db - (cnr - 7.2)).abs() < 2.0,
                    "{cnr}: video SNR {}",
                    d.video_snr_db
                );
                let e = bars_error(r.picture.as_ref().unwrap());
                assert!(e < bars, "{cnr}: bars {e}");
            });
        }
    });
}

#[test]
fn n5_offsets() {
    std::thread::scope(|s| {
        for off in [-600e3, -12e3, 150e3, 1.2e6] {
            s.spawn(move || {
                let mut p = params(10e6);
                p.sound = Programme::Continuous;
                let (r, _) = imp_run(
                    p,
                    Imp {
                        cnr_db: 40.0,
                        offset_hz: off,
                        ..Default::default()
                    },
                    1.75,
                    &[],
                );
                let d = r.last();
                assert_eq!(d.state, 2, "{off}");
                assert!(d.colour, "{off}");
                assert!(
                    (d.carrier_offset_hz - off).abs() < 400.0,
                    "{off}: offset {}",
                    d.carrier_offset_hz
                );
                assert!(bars_error(r.picture.as_ref().unwrap()) < 0.06, "{off}");
                let (l, _) = r.tail(0.2);
                assert!(
                    (tone(&l, 1000.0) - 0.5).abs() < 0.05,
                    "{off}: tone {}",
                    tone(&l, 1000.0)
                );
            });
        }
    });
}

#[test]
fn n6_channel_width_sets_the_offset_reading() {
    for (w, want) in [("7", 100e3), ("8", 600e3)] {
        let p = sys(SystemId::B, None);
        let imp = Imp {
            cnr_db: 40.0,
            offset_hz: 100e3,
            ..Default::default()
        };
        let (r, _) = imp_run(p, imp, 1.25, &[("channel_width", w)]);
        let d = r.last();
        assert_eq!(d.state, 2);
        assert!(
            (d.carrier_offset_hz - want).abs() < 400.0,
            "width {w}: {}",
            d.carrier_offset_hz
        );
    }
}

#[test]
fn n7_clock_error() {
    std::thread::scope(|s| {
        for ppm in [-100.0, 40.0] {
            s.spawn(move || {
                let imp = Imp {
                    cnr_db: 40.0,
                    clock_ppm: ppm,
                    ..Default::default()
                };
                let (r, _) = imp_run(params(10e6), imp, 1.75, &[]);
                let d = r.last();
                assert_eq!(d.state, 2);
                assert!(d.colour);
                assert!(
                    (d.line_error_ppm + ppm).abs() < 8.0,
                    "{ppm}: {}",
                    d.line_error_ppm
                );
                assert!((d.field_hz - 50.0 * (1.0 - ppm * 1e-6)).abs() < 0.01);
                assert!(bars_error(r.picture.as_ref().unwrap()) < 0.06);
            });
        }
    });
}

#[test]
fn n9_n10_levels_quantisation_and_echo() {
    std::thread::scope(|s| {
        s.spawn(|| {
            // The peak carrier 6 dB below the full scale of an 8-bit converter.
            let (r, _) = imp_run(
                params(10e6),
                Imp {
                    bits8_headroom_db: Some(6.0),
                    ..Default::default()
                },
                1.5,
                &[],
            );
            assert_eq!(r.last().state, 2);
            assert!(bars_error(r.picture.as_ref().unwrap()) < 0.09);
        });
        s.spawn(|| {
            // S6: a ghost 1.5 µs late, 25 dB down.
            let (r, _) = imp_run(
                params(10e6),
                Imp {
                    echo: Some((1.5e-6, -25.0)),
                    ..Default::default()
                },
                1.5,
                &[],
            );
            assert_eq!(r.last().state, 2);
            let e = bars_error(r.picture.as_ref().unwrap());
            assert!(
                e < 0.07,
                "ghost: bars {e} {:?}",
                bars(r.picture.as_ref().unwrap())
            );
        });
    });
}

#[test]
fn s10_envelope_detector() {
    let (r, _) = gen_run(params(10e6), 1.5, &[("detector", "envelope")]);
    let d = r.last();
    assert_eq!(d.state, 2);
    assert!(!d.synchronous_detector);
    assert_eq!(d.detector, "envelope");
    assert!(d.colour);
    assert!(bars_error(r.picture.as_ref().unwrap()) < 0.22);
}

#[test]
fn s11_bob_returns_a_picture_a_field() {
    let (r, _) = gen_run(params(10e6), 2.0, &[("deinterlace", "bob")]);
    assert!(r.pictures > 80, "pictures {}", r.pictures);
}

#[test]
fn s12_colour_off_and_forced_mono() {
    for s in [[("colour_on", "false")], [("colour", "mono")]] {
        let (r, _) = gen_run(params(10e6), 1.25, &s);
        let d = r.last();
        assert_eq!(d.state, 2);
        let p = r.picture.as_ref().unwrap();
        assert!(grey(p) < 0.02, "{s:?}: {}", grey(p));
        if s[0].0 == "colour" {
            assert_eq!(d.colour_system, "mono");
        }
    }
}

#[test]
fn s13_snow() {
    let rate = 10e6;
    // Without the option: no snow.
    let mut rx = atv(rate, &[]);
    let mut n = noise(0.07, 5);
    let r = run(&mut rx, rate, 0.5, 50_000, |b| {
        n.process({
            b.fill(Complex32::default());
            b
        })
    });
    assert_eq!(r.snow, 0);
    // With it: noise, then a signal.
    let mut rx = atv(rate, &[("snow", "true")]);
    let mut g = impaired(
        Box::new(sdrkit_stack_analogtv::AtvGenerator::new(params(rate))),
        Imp::cnr(40.0),
    );
    let mut k = 0usize;
    let r = run(&mut rx, rate, 2.0, 50_000, |b| {
        b.fill(Complex32::default());
        if k >= 5_000_000 {
            g.generate(b);
        }
        n.process(b);
        k += b.len();
    });
    assert!(r.snow >= 10, "snow {}", r.snow);
    assert!(r.pictures >= 20, "pictures {}", r.pictures);
    let first_real = r.reports.iter().position(|d| d.pictures > 0).unwrap();
    let snow_then = r.reports[first_real].snow_pictures;
    assert!(
        r.snow <= snow_then + 1,
        "snow after lock: {} then {snow_then}",
        r.snow
    );
}

#[test]
fn s13_snow_pictures_look_like_noise() {
    let rate = 10e6;
    let mut rx = atv(rate, &[("snow", "true")]);
    let mut n = noise(0.07, 5);
    let mut out = Output::default();
    let mut buf = vec![Complex32::default(); 50_000];
    let mut snow = Vec::new();
    for _ in 0..100 {
        buf.fill(Complex32::default());
        n.process(&mut buf);
        out.clear();
        rx.feed(&buf, &mut out);
        snow.extend(pictures(&out));
    }
    assert!(snow.len() >= 8, "{}", snow.len());
    let f = Owner::results(&rx).fields();
    assert_eq!(
        f.at(&sdrkit_stack_analogtv::typed::AtvFields::PICTURES),
        Some(0)
    );
    assert!(f.at(&sdrkit_stack_analogtv::typed::AtvFields::SNOW_PICTURES) >= Some(8));
    let p = snow.last().unwrap();
    let l = p.layout.as_ref().unwrap();
    assert_eq!((l.width, l.height), (768, 576));
    let g: Vec<f64> = p.data.chunks(4).map(|c| c[1] as f64).collect();
    let mean = g.iter().sum::<f64>() / g.len() as f64;
    let sd = (g.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / g.len() as f64).sqrt();
    assert!(
        (100.0..160.0).contains(&mean) && sd > 20.0,
        "mean {mean} sd {sd}"
    );
}

#[test]
fn s14_tuned_to_the_vision_carrier() {
    // The sound must be inside the input: B/G at 20 Msps, M at 10 Msps.
    std::thread::scope(|s| {
        for (id, rate, centre) in [(SystemId::G, 20e6, 2.75e6), (SystemId::M, 10e6, 1.75e6)] {
            s.spawn(move || {
                let mut p = sys(id, None);
                p.rate_hz = rate;
                p.centre_offset_hz = centre;
                let (r, _) = gen_run(p, 1.5, &[]);
                let d = r.last();
                assert_eq!(d.state, 2, "{id:?}");
                assert!(d.vision_hz.abs() < 2000.0, "{id:?}: vision {}", d.vision_hz);
                assert!(r.reports.iter().any(|_| true));
            });
        }
    });
}

#[test]
fn c1_the_cuts_do_not_matter() {
    let mut pics = Vec::new();
    for piece in [333usize, 4096, 65_536] {
        let rate = 10e6;
        let mut g = impaired(
            Box::new(sdrkit_stack_analogtv::AtvGenerator::new(params(rate))),
            Imp::cnr(40.0),
        );
        let mut rx = atv(rate, &[]);
        let r = run(&mut rx, rate, 1.0, piece, |b| {
            g.generate(b);
        });
        let d = r.last();
        assert_eq!(d.state, 2, "piece {piece}");
        assert_eq!(d.system, "PAL B/G 625/50");
        assert!(
            bars_error(r.picture.as_ref().unwrap()) < 0.06,
            "piece {piece}"
        );
        pics.push((d, r.picture.unwrap(), r.pictures));
    }
    // Every component produces the same result for any piece size.
    for w in pics.windows(2) {
        assert_eq!(w[0].0.line_count, w[1].0.line_count);
        assert_eq!(w[0].0.fields_ok, w[1].0.fields_ok);
        assert_eq!(w[0].2, w[1].2);
        assert_eq!(w[0].1.picture.data, w[1].1.picture.data);
    }
}

#[test]
fn c2_reset_mid_stream() {
    let rate = 10e6;
    let imp = Imp {
        cnr_db: 40.0,
        bits8_headroom_db: Some(2.0),
        ..Default::default()
    };
    let mut g = impaired(
        Box::new(sdrkit_stack_analogtv::AtvGenerator::new(params(rate))),
        imp,
    );
    let mut rx = atv(rate, &[]);
    let mut src = |b: &mut [Complex32]| {
        g.generate(b);
    };
    let a = run(&mut rx, rate, 1.0, 50_000, &mut src);
    assert_eq!(a.last().state, 2);
    rx.retuned(&InputInfo::new(rate, 600e6, 600e6).recording());
    // Down after the reset: the store is replaced, nothing is locked.
    let mut out = Output::default();
    rx.feed(&[Complex32::default(); 1000], &mut out);
    assert_eq!(
        detail(Owner::results(&rx), 0.0).state,
        0,
        "down after the reset"
    );
    let b = run(&mut rx, rate, 1.25, 50_000, &mut src);
    assert_eq!(b.last().state, 2, "locked again");
}
