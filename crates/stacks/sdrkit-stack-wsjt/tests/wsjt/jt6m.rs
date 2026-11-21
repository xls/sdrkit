//! `jt6m`: pings at several offsets and drifts, the pass marks in noise, a ping found in a long recording fed in pieces, the
//! fold of a long weak signal and of two signals, and nothing from noise or carriers.

use crate::common::noise;
use crate::scatter::*;
use sdrkit_core::Receiver;
use sdrkit_stack_wsjt::scatter::jt6m::{self, read_window};
use sdrkit_stack_wsjt::JT6M;

const MSG: &str = "K1JT W9XYZ 26";

fn amp_sigma(snr: f64, sigma: f64) -> f32 {
    (2.0 * 10f64.powf(snr / 10.0) * sigma * sigma * 2500.0 / 4000.0).sqrt() as f32
}

fn folded(rx: &dyn Receiver) -> (String, i64) {
    let f = rx.results().fields();
    (
        f.at(&sdrkit_stack_wsjt::typed::Jt6mFields::FOLDED)
            .unwrap_or("")
            .to_owned(),
        f.at(&sdrkit_stack_wsjt::typed::Jt6mFields::FOLD_PERIOD)
            .unwrap_or(0),
    )
}

fn cyclic(text: &str, want: &str) -> bool {
    text.repeat(2).contains(want)
}

#[test]
fn pings_at_each_offset_and_drift() {
    for (df, drift) in [
        (0.0, 0.0),
        (-120.0, 0.0),
        (85.0, 0.0),
        (180.0, 0.0),
        (-60.0, 4.0),
        (40.0, -6.0),
    ] {
        let x = jt6m::audio(MSG, 8000.0, 4.0, df, drift, amp_at(15.0, 8000.0));
        let a = in_noise(&x, 8.0, 1.0, 3);
        let mut rx = receiver(&JT6M, &[]);
        let ev = feed_audio(rx.as_mut(), &a, 4000);
        let d = decodes(&ev);
        assert!(!d.is_empty(), "{df} {drift}: no ping");
        let t = text(d[0]);
        assert!(
            t.contains("K1JT") && t.contains("W9XYZ"),
            "{df} {drift}: {t}"
        );
        let hz = d[0]
            .at(&sdrkit_stack_wsjt::typed::Jt6mDecodeEvent::AUDIO_HZ)
            .unwrap_or(f64::NAN);
        assert!(
            (hz - (df + 2.0 * drift)).abs() <= 10.0,
            "{df} {drift}: {hz}"
        );
        let (f, p) = folded(rx.as_ref());
        assert_eq!(p, 14, "{df} {drift}: {f}");
        assert!(cyclic(&f, "K1JT W9XYZ 26 "), "{df} {drift}: {f}");
    }
}

/// 3 s of signal at +25 Hz in noise of `snr` dB, 10 seeds: how many give a ping with both calls.
fn passes(snr: f64) -> usize {
    let x = jt6m::audio(MSG, 8000.0, 3.0, 25.0, 0.0, amp_at(snr, 8000.0));
    (0..10)
        .filter(|&seed| {
            let a = in_noise(&x, 7.0, 1.5, 200 + seed);
            let mut rx = receiver(&JT6M, &[]);
            let ev = feed_audio(rx.as_mut(), &a, 4000);
            decodes(&ev)
                .iter()
                .any(|e| text(e).contains("K1JT") && text(e).contains("W9XYZ"))
        })
        .count()
}

#[test]
fn pass_marks_in_noise() {
    assert_eq!(passes(10.0), 10);
    assert_eq!(passes(4.0), 10);
    let p0 = passes(0.0);
    for snr in [-3.0, -6.0, -10.0] {
        eprintln!("JT6M at {snr} dB: {} of 10", passes(snr));
    }
    assert!(p0 >= 8, "0 dB: {p0} of 10");
}

#[test]
fn a_ping_in_a_long_recording_fed_in_pieces() {
    for (k, start) in [5.0, 7.3, 10.6].into_iter().enumerate() {
        let x = jt6m::audio(MSG, 8000.0, 1.0, -50.0, 0.0, amp_sigma(15.0, 0.10));
        let mut a = noise(20 * 8000, 0.10, 30 + k as u64);
        put(&mut a, &x, (start * 8000.0) as usize);
        let mut rx = receiver(&JT6M, &[]);
        let ev = feed_audio(rx.as_mut(), &a, 1500);
        let d = decodes(&ev);
        assert!((1..=2).contains(&d.len()), "{start}: {} pings", d.len());
        assert!(
            d.iter().any(|e| text(e).contains("W9XYZ")),
            "{start}: {}",
            text(d[0])
        );
        let t0 = d[0].time;
        let row_t = rx
            .results()
            .table(rx.results().decl().table_id("decodes").unwrap())
            .unwrap()
            .rows_newest_first()
            .last()
            .and_then(|(_, r)| r.at(&sdrkit_stack_wsjt::typed::Jt6mDecodeRow::TIME_S))
            .unwrap();
        assert!((row_t - start).abs() < 0.2, "{start}: {row_t} ({t0})");
        let hz = d[0]
            .at(&sdrkit_stack_wsjt::typed::Jt6mDecodeEvent::AUDIO_HZ)
            .unwrap();
        assert!((hz + 50.0).abs() <= 10.0, "{start}: {hz}");
    }
}

#[test]
fn a_long_weak_signal_folds() {
    let x = jt6m::audio(
        "SP2XYZ LY2AB KO24",
        8000.0,
        28.0,
        60.0,
        0.0,
        amp_at(-6.0, 8000.0),
    );
    let a = in_noise(&x, 30.0, 1.0, 40);
    let mut rx = receiver(&JT6M, &[]);
    feed_audio(rx.as_mut(), &a, 4000);
    let (f, p) = folded(rx.as_ref());
    assert_eq!(p, 18, "{f}");
    assert!(cyclic(&f, "LY2AB"), "{f}");
}

#[test]
fn the_stronger_of_two_signals_folds() {
    let a1 = jt6m::audio("AAA1 BBB2", 8000.0, 12.0, 0.0, 0.0, amp_at(10.0, 8000.0));
    let a2 = jt6m::audio("CCC3 DDD4", 8000.0, 12.0, 150.0, 0.0, amp_at(0.0, 8000.0));
    let mut a = in_noise(&a1, 14.0, 1.0, 41);
    put(&mut a, &a2, ((1.0 + 0.37) * 8000.0) as usize);
    let mut rx = receiver(&JT6M, &[]);
    feed_audio(rx.as_mut(), &a, 4000);
    let (f, p) = folded(rx.as_ref());
    assert_eq!(p, 10, "{f}");
    assert!(cyclic(&f, "AAA1 BBB2"), "{f}");
}

#[test]
fn noise_and_carriers_give_nothing() {
    let mut rx = receiver(&JT6M, &[]);
    assert!(decodes(&feed_audio(rx.as_mut(), &noise(60 * 8000, SIGMA, 50), 4000)).is_empty());
    for f in [jt6m::sync_hz(), 1500.0] {
        let c: Vec<f32> = (0..20 * 8000)
            .map(|i| {
                amp_at(20.0, 8000.0) * (std::f64::consts::TAU * f * i as f64 / 8000.0).cos() as f32
            })
            .collect();
        let a = in_noise(&c, 22.0, 1.0, 51);
        let mut rx = receiver(&JT6M, &[]);
        let d = decodes(&feed_audio(rx.as_mut(), &a, 4000)).len();
        assert_eq!(d, 0, "carrier at {f}");
    }
}

#[test]
fn a_window_reads_its_frames() {
    let x = jt6m::audio(MSG, 8000.0, 6.0, 0.0, 0.0, 0.3);
    let w = read_window(&x, 8000.0, 200.0).expect("sync");
    assert!(w.df_hz.abs() < 6.0, "{}", w.df_hz);
    assert!(w.frames.iter().filter(|f| f.clear).count() >= 40);
    assert_eq!(w.fold.as_ref().map(|f| f.1), Some(14));
}

#[test]
fn the_test_signal_gives_its_pings() {
    let mut g = (JT6M.make_generator.unwrap())(&sdrkit_core::GenConfig::new(
        48_000.0,
        sdrkit_core::Values::defaults(&(JT6M.settings)()),
    ))
    .unwrap();
    let mut iq = vec![sdrkit_core::Complex32::default(); 24 * 48_000];
    g.generate(&mut iq);
    let mut rx = receiver(&JT6M, &[]);
    rx.configure(&sdrkit_core::InputInfo::new(48_000.0, 50e6 + 20e3, 50e6).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    let mut ev = Vec::new();
    for c in iq.chunks(4800) {
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        out.clear();
    }
    rx.finish(&mut out);
    ev.append(&mut out.events);
    let t: Vec<&str> = decodes(&ev).iter().map(|e| text(e)).collect();
    assert_eq!(t.len(), 3, "{t:?}");
    assert!(t.iter().any(|x| x.contains("K0TST")), "{t:?}");
}

/// 300 runs of 6 s of noise give no ping.
#[test]
fn noise_gives_no_false_ping() {
    let pinged = (0..300u64)
        .filter(|seed| {
            let x = noise(6 * 8000, SIGMA, 9000 + seed);
            let mut rx = receiver(&JT6M, &[]);
            !decodes(&feed_audio(rx.as_mut(), &x, 4000)).is_empty()
        })
        .count();
    assert_eq!(pinged, 0);
}

/// A ping of 1.5 s at -12 dB is found in 56 of 60 runs: a ping that starts within it (or 0.3 s before) and lies within
/// 10 Hz of its offset.
#[test]
fn a_weak_short_ping_is_found() {
    let found = (0..60u64)
        .filter(|seed| {
            let x = jt6m::audio(MSG, 8000.0, 1.5, 25.0, 0.0, amp_at(-12.0, 8000.0));
            let a = in_noise(&x, 6.0, 2.0, 7000 + seed);
            let mut rx = receiver(&JT6M, &[]);
            let ev = feed_audio(rx.as_mut(), &a, 4000);
            decodes(&ev).iter().any(|e| {
                let hz = e
                    .at(&sdrkit_stack_wsjt::typed::Jt6mDecodeEvent::AUDIO_HZ)
                    .unwrap_or(f64::NAN);
                (1.7..3.5).contains(&e.time) && (hz - 25.0).abs() <= 10.0
            })
        })
        .count();
    assert!(found >= 56, "{found} of 60");
}
