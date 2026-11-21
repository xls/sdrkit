//! The tests of the STL stack: the composite, mono and digital-like test links, noise alone, the deviation reading, the
//! automatic width, the SCA meter, cut invariance and the region data.

mod common;

use common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{LockState, Value};
use sdrkit_stack_fm::stl::settings::GenLink;
use sdrkit_stack_fm::stl::typed::{LinkKind, StlFields};

/// A composite link at `deviation_hz` full deviation, `offset_hz` off the tuned frequency, at C/N 40 dB, through a
/// stack set to the same deviation: (the graph, its sound).
fn composite(deviation_hz: f64, offset_hz: f64, rate: f64, settings: &[(&str, Value)]) -> (sdrkit_graph::StackGraph, Run) {
    let mut p = link(GenLink::Composite, rate);
    (p.fm.deviation_hz, p.fm.offset_hz) = (deviation_hz, offset_hz);
    let mut s = vec![("deviation", Value::Hz(deviation_hz))];
    s.extend(settings.iter().cloned());
    let mut rx = receiver(rate, &s);
    let mut src = Source::new(p, 40.0, 1);
    let r = run(&mut rx, &mut src, rate, 4.0);
    (rx, r)
}

#[test]
fn a_composite_link_reads_pilot_stereo_and_rds() {
    for deviation in [60e3, 75e3] {
        for offset in [0.0, 25e3, -40e3] {
            let (rx, r) = composite(deviation, offset, RATE, &[]);
            let d = reading(&rx);
            let at = format!("±{:.0} kHz, offset {offset}", deviation / 1e3);
            assert_eq!(kind(&rx), LinkKind::Composite, "{at}");
            assert!(d.pilot_locked && d.stereo, "{at}: pilot");
            assert_eq!(d.pi, 0x4A21, "{at}: PI");
            assert_eq!(d.station, "SDRKIT  ", "{at}: PS");
            assert!((d.offset_hz - offset).abs() < 3e3, "{at}: offset {}", d.offset_hz);
            let (left, right) = r.tail(1.0);
            let sep_l = 20.0 * (amp(&left, 1000.0) / amp(&left, 3000.0)).log10();
            let sep_r = 20.0 * (amp(&right, 3000.0) / amp(&right, 1000.0)).log10();
            assert!(sep_l > 30.0 && sep_r > 30.0, "{at}: separation {sep_l:.1} {sep_r:.1} dB");
            let sinad = sinad_db(&left, 1000.0);
            assert!(sinad > 30.0, "{at}: SINAD {sinad:.1} dB");
            let peak = fields(&rx).at(&StlFields::SOUND_PEAK_DBFS).expect("sound");
            assert!((-40.0..3.0).contains(&peak), "{at}: peak {peak:.1} dBFS");
            assert_eq!(Owner::results(&rx).status().lock_state, LockState::Locked, "{at}");
            assert!(link_rows(&rx) >= 1, "{at}");
        }
    }
}

#[test]
fn the_composite_link_at_two_rates_and_a_manual_width() {
    for (rate, settings) in [
        (2e6, vec![]),
        (RATE, vec![("auto_width", Value::Bool(false)), ("width", Value::Hz(400e3))]),
    ] {
        let (rx, r) = composite(75e3, 0.0, rate, &settings);
        assert_eq!(kind(&rx), LinkKind::Composite, "{rate}");
        let (left, _) = r.tail(1.0);
        let sinad = sinad_db(&left, 1000.0);
        assert!(sinad > 30.0, "{rate}: SINAD {sinad:.1} dB");
    }
}

#[test]
fn the_peak_deviation_follows_the_link() {
    let peak = |deviation| {
        let (rx, _) = composite(deviation, 0.0, RATE, &[]);
        fields(&rx)
            .at(&StlFields::PEAK_DEVIATION_HZ)
            .expect("a peak deviation")
    };
    let (p60, p75) = (peak(60e3), peak(75e3));
    assert!(p60 > 20e3, "{p60}");
    let ratio = p75 / p60;
    assert!((1.1..1.4).contains(&ratio), "{p75} / {p60} = {ratio:.3}");
}

#[test]
fn a_mono_link_plays_its_audio_level() {
    // The same tone of half full scale in both channels, at two deviations with the stack set to each.
    for deviation in [75e3, 50e3] {
        let mut p = link(GenLink::Mono, RATE);
        p.fm.deviation_hz = deviation;
        (p.fm.right_hz, p.fm.right_amp) = (1000.0, 0.5);
        let mut rx = receiver(RATE, &[("deviation", Value::Hz(deviation))]);
        let mut src = Source::new(p, 40.0, 2);
        let r = run(&mut rx, &mut src, RATE, 3.0);
        let d = reading(&rx);
        assert_eq!(kind(&rx), LinkKind::Mono);
        assert!(!d.pilot_locked && !d.stereo);
        let (left, right) = r.tail(1.0);
        let (l, rr) = (amp(&left, 1000.0), amp(&right, 1000.0));
        // The FM sound stage plays a fully modulated programme at 0.8 of full scale.
        let want = 0.8 * 0.5;
        let db = 20.0 * (l / want).log10();
        assert!(db.abs() < 1.0, "±{deviation}: {l:.3} against {want}");
        assert!((l - rr).abs() < 0.01 * l);
        assert!(sinad_db(&left, 1000.0) > 30.0);
    }
}

#[test]
fn a_mono_link_with_an_offset_reads_mono() {
    let mut p = link(GenLink::Mono, RATE);
    (p.fm.offset_hz, p.fm.right_hz) = (30e3, 1000.0);
    let mut rx = receiver(RATE, &[]);
    let mut src = Source::new(p, 40.0, 3);
    let r = run(&mut rx, &mut src, RATE, 3.0);
    assert_eq!(kind(&rx), LinkKind::Mono);
    assert!((reading(&rx).offset_hz - 30e3).abs() < 3e3);
    let (left, _) = r.tail(1.0);
    assert!(sinad_db(&left, 1000.0) > 30.0);
}

#[test]
fn a_digital_link_is_detected_and_not_decoded() {
    for over in [10.0, 20.0] {
        let p = link(GenLink::Digital, RATE);
        let width = p.width_hz;
        let mut rx = receiver(RATE, &[]);
        // The noise of the capture `over` dB under the link's density.
        let cnr = over + 10.0 * (200e3 / width).log10();
        let mut src = Source::new(p, cnr, 4);
        let r = run(&mut rx, &mut src, RATE, 3.0);
        assert_eq!(kind(&rx), LinkKind::DigitalUnknown, "{over} dB");
        let f = fields(&rx);
        let w = f.at(&StlFields::OCCUPIED_WIDTH_HZ).expect("a width");
        assert!((width..width + 40e3).contains(&w), "{over} dB: {w}");
        let level = f.at(&StlFields::OVER_FLOOR_DB).expect("a level");
        assert!((level - over).abs() < 3.0, "{over} dB: {level:.1}");
        let st = Owner::results(&rx).status();
        assert!(st.summary.contains("not decoded"), "{}", st.summary);
        assert_eq!(st.lock_state, LockState::Tracking);
        let d = reading(&rx);
        assert!(!d.rds && d.station.is_empty() && !d.carrier);
        let (left, _) = r.tail(1.0);
        assert_eq!(sdrkit_dev_test::measure::rms(&left), 0.0, "no sound");
        assert_eq!(link_rows(&rx), 1);
    }
}

#[test]
fn noise_alone_is_nothing() {
    for level in [-40.0, -20.0, 0.0, 10.0] {
        for seed in 1..=3 {
            let mut rx = receiver(RATE, &[]);
            let power = 10f64.powf(level / 10.0);
            let mut n = sdrkit_algo_sim::noise::Awgn::with_power(power, seed).unwrap();
            run_cut(&mut rx, RATE, 2.0, 100_000, |x| {
                x.fill(Default::default());
                n.process(x);
            });
            assert_eq!(kind(&rx), LinkKind::Nothing, "{level} dB, seed {seed}");
            assert_eq!(link_rows(&rx), 0, "{level} dB, seed {seed}");
            assert_eq!(Owner::results(&rx).status().lock_state, LockState::Searching);
        }
    }
}

#[test]
fn the_automatic_width_narrows_to_the_link_and_an_sca_keeps_it_wide() {
    // A fully modulated link needs 2 × (75 + 60) × 1.35 + 10 = 374.5 kHz: the width stays at 400 kHz.
    let (rx, _) = composite(75e3, 0.0, RATE, &[]);
    assert_eq!(reading(&rx).channel_width_hz, 400e3);
    // Tones at half the level: a peak near 31 kHz and about 200 kHz occupied need about 280 kHz, and the width narrows.
    let mut p = link(GenLink::Composite, RATE);
    (p.fm.left_amp, p.fm.right_amp) = (0.25, 0.25);
    let mut rx = receiver(RATE, &[]);
    let mut src = Source::new(p, 40.0, 5);
    run(&mut rx, &mut src, RATE, 4.0);
    let w = reading(&rx).channel_width_hz;
    let f = fields(&rx);
    let peak = f.at(&StlFields::PEAK_DEVIATION_HZ).unwrap();
    let occupied = f.at(&StlFields::OCCUPIED_WIDTH_HZ).unwrap();
    let need = (2.0 * (peak + 60e3)).max(occupied) * 1.35 + 10e3;
    assert!(
        (250e3..330e3).contains(&w) && (w - need).abs() < 10e3,
        "{w} for a peak of {peak} and {occupied} occupied"
    );
    assert_eq!(fields(&rx).at(&StlFields::WIDTH_AUTO), Some(true));
    assert!(fields(&rx).at(&StlFields::SCA67_SNR_DB).is_none());
    let mut p = link(GenLink::Composite, RATE);
    p.fm.sca = true;
    let mut rx = receiver(RATE, &[]);
    let mut src = Source::new(p, 40.0, 5);
    run(&mut rx, &mut src, RATE, 4.0);
    let f = fields(&rx);
    let (s67, s92) = (f.at(&StlFields::SCA67_SNR_DB), f.at(&StlFields::SCA92_SNR_DB));
    assert!(s67.is_some_and(|x| x > 10.0) && s92.is_some_and(|x| x > 10.0), "{s67:?} {s92:?}");
    assert_eq!(reading(&rx).channel_width_hz, 400e3);
    assert_eq!(kind(&rx), LinkKind::Composite);
}

#[test]
fn any_cut_of_the_input_gives_the_same_results() {
    let results = |piece: usize| {
        let mut p = link(GenLink::Composite, RATE);
        p.fm.offset_hz = 25e3;
        let mut rx = receiver(RATE, &[]);
        let mut src = Source::new(p, 30.0, 6);
        let r = run_cut(&mut rx, RATE, 3.0, piece, |x| src.fill(x));
        let f = fields(&rx);
        (
            reading(&rx).channel_width_hz,
            f.at(&StlFields::PEAK_DEVIATION_HZ),
            f.at(&StlFields::OCCUPIED_WIDTH_HZ),
            r.audio.len(),
        )
    };
    assert_eq!(results(100_000), results(7_777));
}

#[test]
fn the_test_link_locks_with_the_default_settings() {
    let d = &sdrkit_stack_fm::stl::DESCRIPTOR;
    let rate = d.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(d).values().unwrap();
    let mut g = (d.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut rx = receiver(rate, &[]);
    run_cut(&mut rx, rate, 3.0, 50_000, |x| {
        g.generate(x);
    });
    assert_eq!(kind(&rx), LinkKind::Composite);
    assert_eq!(Owner::results(&rx).status().lock_state, LockState::Locked);
}

#[test]
fn the_united_states_plan_starts_the_stack_in_the_aural_stl_band() {
    let data =
        sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap();
    let region = sdrkit_presets::Region::new("US").unwrap();
    let p = data
        .preset("stl", &region, &sdrkit_presets::Options::default())
        .unwrap();
    assert_eq!(p.plan, "stl_us");
    assert_eq!(p.frequency_hz, 944.0125e6);
    assert_eq!(p.width_hz, Some(25e3));
    let last = p.channels.last().unwrap();
    assert!((last.frequency_hz - 951.9875e6).abs() < 1.0);
    assert_eq!(p.channels.len(), 320);
}

