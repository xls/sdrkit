//! The sound options (1 Msps, C/N 50, mono, no RDS, no pre-emphasis unless stated).

use super::common::*;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Complex32, Value};
use sdrkit_stack_fm::GenParams;

const RATE: f64 = 1e6;

fn mono(left_hz: f64, left_amp: f64, right_hz: f64, right_amp: f64) -> Sig {
    sig(
        GenParams {
            rate_hz: RATE,
            stereo: false,
            rds: false,
            preemphasis_s: 0.0,
            left_hz,
            left_amp,
            right_hz,
            right_amp,
            gain: 0.3,
            ..Default::default()
        },
        50.0,
    )
}

fn left_amp_at(settings: &[(&str, &str)], p: Sig, hz: f64) -> f64 {
    let mut rx = receiver(RATE, 100e6, settings);
    let run = run(&mut rx, p, 2.0, |_| {});
    let (l, _) = run.tail(1.0);
    amp(&l, hz)
}

#[test]
fn de_emphasis() {
    let p = mono(10_000.0, 0.3, 10_000.0, 0.3);
    let on = left_amp_at(&[("deemphasis", "50")], p.clone(), 10_000.0);
    let off = left_amp_at(&[("deemphasis", "off")], p, 10_000.0);
    let db = 20.0 * (off / on).log10();
    assert!(db > 6.0, "off is only {db:.1} dB louder");
    assert!(off > 0.05, "{off}");
}

#[test]
fn audio_bandwidth() {
    let p = mono(1000.0, 0.3, 10_000.0, 0.3);
    let base = [("deemphasis", "off")];
    let full10 = left_amp_at(&base, p.clone(), 10_000.0);
    let full1 = left_amp_at(&base, p.clone(), 1000.0);
    let cut10 = left_amp_at(
        &[("deemphasis", "off"), ("audio_bandwidth", "5000")],
        p.clone(),
        10_000.0,
    );
    let cut1 = left_amp_at(
        &[("deemphasis", "off"), ("audio_bandwidth", "5000")],
        p,
        1000.0,
    );
    assert!(
        20.0 * (full10 / cut10).log10() > 20.0,
        "10 kHz {full10} -> {cut10}"
    );
    assert!(
        (20.0 * (full1 / cut1).log10()).abs() < 1.0,
        "1 kHz {full1} -> {cut1}"
    );
}

#[test]
fn squelch() {
    for (squelch, quiet) in [("15", true), ("0", false)] {
        let mut rx = receiver(RATE, 100e6, &[("squelch", squelch)]);
        let mut noise = noise_sigma(0.1, 3);
        let run = run_with(&mut rx, RATE, 2.0, |buf, n| {
            buf.resize(n, Complex32::default());
            noise.process(buf);
        });
        let (l, _) = run.tail(1.5);
        let rms = sdrkit_dev_test::measure::rms(&l);
        if quiet {
            assert!(rms < 1e-4, "squelched noise RMS {rms}");
            assert!(run.last.squelched);
        } else {
            assert!(rms > 1e-3, "open noise RMS {rms}");
            assert!(!run.last.squelched);
        }
    }
    let mut rx = receiver(RATE, 100e6, &[("squelch", "15")]);
    let run = run(&mut rx, mono(1000.0, 0.5, 1000.0, 0.5), 2.0, |_| {});
    let (l, _) = run.tail(1.0);
    assert!(sdrkit_dev_test::measure::rms(&l) > 0.05);
    assert!(!run.last.squelched);
}

fn weak_pilot_signal(pilot_pct: f64) -> Sig {
    sig(
        GenParams {
            rate_hz: RATE,
            rds: false,
            left_amp: 0.4,
            right_amp: 0.0,
            pilot_pct,
            gain: 0.3,
            ..Default::default()
        },
        50.0,
    )
}

fn separation(choice: &str, p: Sig) -> (f64, bool) {
    let mut rx = receiver(RATE, 100e6, &[("stereo", choice)]);
    let run = run(&mut rx, p, 3.0, |_| {});
    let (l, r) = run.tail(1.5);
    (
        20.0 * (amp(&l, 1000.0) / amp(&r, 1000.0)).log10(),
        run.last.stereo,
    )
}

#[test]
fn auto_and_mono() {
    let (sep, stereo) = separation("auto", weak_pilot_signal(9.0));
    assert!(stereo && sep > 20.0, "auto: stereo {stereo}, {sep:.1} dB");
    let (sep, stereo) = separation("mono", weak_pilot_signal(9.0));
    assert!(
        !stereo && sep.abs() < 1.0,
        "mono: stereo {stereo}, {sep:.1} dB"
    );
}

#[test]
fn forced_stereo_with_a_weak_pilot() {
    let (auto, _) = separation("auto", weak_pilot_signal(1.0));
    let (forced, stereo) = separation("stereo", weak_pilot_signal(1.0));
    assert!(stereo, "forced stereo reports stereo");
    assert!(
        forced > auto + 6.0,
        "forced {forced:.1} dB against auto {auto:.1} dB"
    );
}

#[test]
fn tone_controls_change_the_sound_and_zero_is_the_identity() {
    let p = mono(100.0, 0.2, 10_000.0, 0.2);
    let flat = left_amp_at(&[("deemphasis", "off")], p.clone(), 100.0);
    let zero = left_amp_at(
        &[("deemphasis", "off"), ("bass", "0"), ("treble", "0")],
        p.clone(),
        100.0,
    );
    assert_eq!(flat, zero);
    let boosted = left_amp_at(&[("deemphasis", "off"), ("bass", "6")], p.clone(), 100.0);
    assert!(
        (20.0 * (boosted / flat).log10() - 6.0).abs() < 1.0,
        "bass +6: {flat} -> {boosted}"
    );
    let t0 = left_amp_at(&[("deemphasis", "off")], p.clone(), 10_000.0);
    let warm = left_amp_at(&[("deemphasis", "off"), ("warm", "1")], p, 10_000.0);
    assert!(
        (20.0 * (warm / t0).log10() + 3.0).abs() < 1.0,
        "warm: {t0} -> {warm}"
    );
}

#[test]
fn a_setting_changed_while_running_takes_effect() {
    let mut rx = receiver(RATE, 100e6, &[("deemphasis", "off")]);
    let p = mono(10_000.0, 0.3, 10_000.0, 0.3);
    let before = run(&mut rx, p.clone(), 1.0, |_| {});
    set(&mut rx, "audio_bandwidth", "5000").unwrap();
    let after = run(&mut rx, p, 1.0, |_| {});
    let (a, _) = before.tail(0.5);
    let (b, _) = after.tail(0.5);
    assert!(amp(&a, 10e3) / amp(&b, 10e3) > 10.0);
    set(&mut rx, "stereo", "mono").unwrap();
    let stereo = rx.schema().id("stereo").unwrap();
    assert!(owner::apply(&mut rx, stereo, &Value::Choice(9)).is_err());
    assert!(rx.schema().id("no_such_key").is_err());
}
