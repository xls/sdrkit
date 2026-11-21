//! C-QUAM AM stereo: the recorded vectors against expected.json, and the acceptance cases.

use crate::support::{apply, flag, impair, int, is_none, num, snr, stack_config, text, Set};
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Receiver, Results, SettingSet};
use sdrkit_dev_test::measure;
use sdrkit_stack_broadcast::cquam::{self, CquamGenerator, GenKind, GenOptions};
use serde_json::json;

const DIAL: f64 = 999e3;

struct Run {
    r: Results,
    left: Vec<f32>,
    right: Vec<f32>,
    first_stereo_s: Option<f64>,
    reports: usize,
}

impl Run {
    fn stereo(&self) -> bool {
        flag(&self.r, "stereo")
    }
    fn offset(&self) -> f64 {
        num(&self.r, "offset_hz")
    }
    fn lock_state(&self) -> LockState {
        self.r.status().lock_state
    }
}

fn receiver(settings: &[Set]) -> Box<dyn Receiver> {
    let mut rx = cquam::make_receiver(&stack_config(cquam::CquamSetting::schema(), &[])).unwrap();
    for (k, v) in settings {
        apply(rx.as_mut(), k, v.clone()).unwrap();
    }
    rx
}

fn feed_all(rx: &mut dyn Receiver, x: &[Complex32], rate: f64, piece: usize) -> Run {
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut first_stereo = None;
    let mut fed = 0usize;
    let mut reports = 0;
    let mut last_t = -1.0;
    for c in x.chunks(piece) {
        rx.feed(c, &mut out);
        audio.extend_from_slice(out.audio());
        out.clear();
        fed += c.len();
        let r = rx.results();
        let t = num(r, "signal_time");
        if t.is_finite() && t != last_t {
            reports += 1;
            last_t = t;
        }
        if flag(r, "stereo") && first_stereo.is_none() {
            first_stereo = Some(fed as f64 / rate);
        }
    }
    let (left, right) = measure::deinterleave(&audio);
    Run {
        r: rx.results().clone(),
        left,
        right,
        first_stereo_s: first_stereo,
        reports,
    }
}

/// The clean signal through the framework's impairments (C/N 35 dB in 10 kHz unless `settings` say otherwise).
fn generate(rate: f64, o: GenOptions, seconds: f64, settings: &[Set]) -> Vec<Complex32> {
    let mut all: Vec<Set> = snr(35.0).to_vec();
    all.push(("gen_seed", json!(7)));
    all.extend(settings.iter().cloned());
    impair(Box::new(CquamGenerator::new(rate, o)), seconds, &all)
}

fn run_gen(rate: f64, o: GenOptions, seconds: f64, gen: &[Set], settings: &[Set]) -> Run {
    let centre_off = o.centre_offset_hz;
    let x = generate(rate, o, seconds, gen);
    let mut rx = receiver(settings);
    rx.configure(&InputInfo::new(rate, DIAL - centre_off, DIAL).recording())
        .unwrap();
    feed_all(rx.as_mut(), &x, rate, 65_536)
}

fn opts(rate: f64) -> GenOptions {
    GenOptions {
        centre_offset_hz: cquam::default_centre_offset(rate),
        ..GenOptions::default()
    }
}

fn last(x: &[f32], seconds: f64) -> &[f32] {
    let n = (seconds * 48_000.0) as usize;
    &x[x.len().saturating_sub(n)..]
}

fn amp(x: &[f32], f: f64) -> f64 {
    measure::tone_amplitude(last(x, 2.0), 48_000.0, f)
}

#[test]
fn recorded_stereo_vector_matches_expected() {
    let exp = sdrkit_dev_test::sigmf::read_expected("broadcast").unwrap();
    let e = &exp["cquam_stereo_250k"];
    let Some(rec) = sdrkit_dev_test::read_recording("broadcast/cquam_stereo_250k") else {
        return;
    };
    let mut rx = receiver(&[]);
    rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording())
        .unwrap();
    let r = feed_all(rx.as_mut(), &rec.samples, rec.rate_hz, 25_000);
    let t = &e["tolerance"];
    let range = |k: &str| (t[k][0].as_f64().unwrap(), t[k][1].as_f64().unwrap());
    assert_eq!(r.lock_state(), LockState::Locked);
    assert!(r.stereo() && flag(&r.r, "pilot_seen"));
    let (a, b) = range("pilot_pct");
    let pilot = num(&r.r, "pilot_pct");
    assert!((a..=b).contains(&pilot), "pilot {pilot}");
    let (a, b) = range("carrier_offset_hz");
    assert!((a..=b).contains(&r.offset()), "offset {}", r.offset());
    let l440 = amp(&r.left, 440.0);
    let r660 = amp(&r.right, 660.0);
    let (a, b) = range("left_440_amp");
    assert!((a..=b).contains(&l440), "left 440 {l440}");
    let (a, b) = range("right_660_amp");
    assert!((a..=b).contains(&r660), "right 660 {r660}");
    let sep_l = measure::ratio_db(l440, amp(&r.left, 660.0));
    let sep_r = measure::ratio_db(r660, amp(&r.right, 440.0));
    assert!(sep_l >= 25.0 && sep_r >= 25.0, "separation {sep_l} {sep_r}");
    let (a, b) = range("mod_pct_peak");
    let m = num(&r.r, "mod_peak_pct");
    assert!((a..=b).contains(&m), "modulation {m}");
    assert!(int(&r.r, "pilot_checks_ok") > 5);
    assert!(num(&r.r, "cn_db") >= 25.0, "C/N {}", num(&r.r, "cn_db"));
    assert!(r.first_stereo_s.unwrap() <= 3.0, "{:?}", r.first_stereo_s);
}

#[test]
fn recorded_mono_vector_matches_expected() {
    let exp = sdrkit_dev_test::sigmf::read_expected("broadcast").unwrap();
    let e = &exp["am_mono_250k"];
    let Some(rec) = sdrkit_dev_test::read_recording("broadcast/am_mono_250k") else {
        return;
    };
    let mut rx = receiver(&[]);
    rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording())
        .unwrap();
    let r = feed_all(rx.as_mut(), &rec.samples, rec.rate_hz, 25_000);
    assert_eq!(r.lock_state(), LockState::Tracking);
    assert!(!r.stereo() && !flag(&r.r, "pilot_seen") && r.first_stereo_s.is_none());
    let t = &e["tolerance"]["carrier_offset_hz"];
    assert!(
        (t[0].as_f64().unwrap()..=t[1].as_f64().unwrap()).contains(&r.offset()),
        "{}",
        r.offset()
    );
    assert!(amp(&r.left, 440.0) > 0.10, "{}", amp(&r.left, 440.0));
    let (mut d, mut s) = (0.0f64, 0.0f64);
    for (a, b) in r.left.iter().zip(&r.right) {
        d += ((a - b) as f64).powi(2);
        s += (*a as f64).powi(2);
    }
    assert!(d <= 1e-6 * s.max(1e-30));
}

#[test]
fn aq1_stereo_at_35_db() {
    let r = run_gen(1e6, opts(1e6), 8.0, &[], &[]);
    assert!(r.stereo(), "{:?}", r.r.fields());
    let pilot = num(&r.r, "pilot_pct");
    assert!((3.5..=6.5).contains(&pilot), "{pilot}");
    assert!(r.offset().abs() <= 3.0, "{}", r.offset());
    let l = amp(&r.left, 440.0);
    let rr = amp(&r.right, 660.0);
    assert!(
        (l - 0.35).abs() <= 0.05 && (rr - 0.35).abs() <= 0.05,
        "{l} {rr}"
    );
    assert!(measure::ratio_db(l, amp(&r.left, 660.0)) > 25.0);
    assert!(measure::ratio_db(rr, amp(&r.right, 440.0)) > 25.0);
    assert!(int(&r.r, "pilot_checks_ok") > 5);
    let m = num(&r.r, "mod_peak_pct");
    assert!((40.0..=100.0).contains(&m), "{m}");
    let q = num(&r.r, "lock_quality");
    assert!(q > 0.8 && q <= 1.0);
    assert!(r.r.status().summary.starts_with("C-QUAM stereo"));
}

#[test]
fn aq2_left_silent_keeps_separation() {
    let o = GenOptions {
        left_silent: true,
        ..opts(1e6)
    };
    let r = run_gen(1e6, o, 8.0, &[], &[]);
    assert!(r.stereo(), "{:?}", r.r.fields());
    let right = amp(&r.right, 660.0);
    let leak = amp(&r.left, 660.0);
    assert!(measure::ratio_db(right, leak) > 22.0, "{right} {leak}");
}

#[test]
fn aq3_plain_am_is_mono_with_identical_channels() {
    let o = GenOptions {
        kind: GenKind::PlainAm,
        ..opts(1e6)
    };
    let r = run_gen(1e6, o, 10.0, &[], &[]);
    assert_eq!(r.lock_state(), LockState::Tracking);
    assert!(!r.stereo() && !flag(&r.r, "pilot_seen"));
    assert!(r.left == r.right);
    assert!(amp(&r.left, 440.0) > 0.1);
    assert!(r.r.status().summary.contains("no 25 Hz pilot"));
}

#[test]
fn aq4_mono_forced() {
    let r = run_gen(1e6, opts(1e6), 8.0, &[], &[("stereo", json!("mono"))]);
    assert!(!r.stereo());
    assert!(r.left == r.right);
    assert!(is_none(&r.r, "width_db"));
    assert_eq!(text(&r.r, "stereo_mode"), "mono");
}

#[test]
fn aq5_noise_only_is_silent() {
    let o = GenOptions {
        kind: GenKind::NoiseOnly,
        ..opts(1e6)
    };
    let r = run_gen(1e6, o, 6.0, &snr(20.0), &[]);
    assert_eq!(r.lock_state(), LockState::Searching);
    assert!(!r.stereo());
    let ms = r.left.iter().map(|v| (v * v) as f64).sum::<f64>() / r.left.len() as f64;
    assert!(ms < 1e-4, "{ms}");
    assert_eq!(r.r.status().summary, "AM stereo: no carrier yet");
}

#[test]
fn aq6_tuning_errors_and_station_offsets() {
    for cfo in [-400.0, -150.0, -20.0, 35.0, 180.0, 400.0] {
        let r = run_gen(1e6, opts(1e6), 10.0, &[("gen_offset_hz", json!(cfo))], &[]);
        assert!(r.stereo(), "cfo {cfo}");
        assert!((r.offset() - cfo).abs() <= 3.0, "cfo {cfo}: {}", r.offset());
    }
    // A station off the dial, with the channel offset set to it: the loop pulls in what is left.
    for st in [-250.0, 120.0] {
        let r = run_gen(
            1e6,
            opts(1e6),
            10.0,
            &[("gen_offset_hz", json!(st))],
            &[("channel_offset", json!(st - 30.0))],
        );
        assert!(r.stereo(), "station {st}");
        assert!(
            (r.offset() - 30.0).abs() <= 3.0,
            "station {st}: {}",
            r.offset()
        );
    }
}

#[test]
fn aq7_rates() {
    for rate in [250e3, 500e3, 2e6] {
        let r = run_gen(rate, opts(rate), 8.0, &[], &[]);
        assert!(r.stereo(), "rate {rate}");
        assert_eq!(num(&r.r, "input_rate"), rate);
    }
}

#[test]
fn aq8_weak_signals() {
    let mut weakest = None;
    for c in [35.0, 30.0, 25.0, 20.0, 16.0, 12.0] {
        let r = run_gen(1e6, opts(1e6), 14.0, &snr(c), &[]);
        if c >= 20.0 {
            assert!(r.stereo(), "C/N {c}");
        }
        if c >= 14.0 {
            assert!(r.lock_state() != LockState::Searching, "C/N {c}");
        }
        if r.stereo() {
            weakest = Some(c);
        }
    }
    eprintln!("weakest stereo C/N: {weakest:?} dB");
}

#[test]
fn aq9_plain_am_never_stereo() {
    for (c, cfo) in [(25.0, 0.0), (12.0, 40.0), (8.0, 0.0)] {
        let o = GenOptions {
            kind: GenKind::PlainAm,
            ..opts(1e6)
        };
        let mut g: Vec<Set> = snr(c).to_vec();
        g.push(("gen_offset_hz", json!(cfo)));
        let r = run_gen(1e6, o, 10.0, &g, &[]);
        assert!(r.first_stereo_s.is_none(), "{c} {cfo}");
    }
}

#[test]
fn aq10_flat_fading() {
    // The whole signal (with its noise) swings between 1 and 1 - depth at 0.5 Hz.
    let rate = 1e6;
    for depth in [0.5, 0.8] {
        let o = opts(rate);
        let mut x = generate(rate, o.clone(), 16.0, &snr(30.0));
        for (n, s) in x.iter_mut().enumerate() {
            let t = n as f64 / rate;
            *s *= (1.0 - depth * 0.5 * (1.0 - (std::f64::consts::TAU * 0.5 * t).cos())) as f32;
        }
        let mut rx = receiver(&[]);
        rx.configure(&InputInfo::new(rate, DIAL - o.centre_offset_hz, DIAL).recording())
            .unwrap();
        let r = feed_all(rx.as_mut(), &x, rate, 65_536);
        assert!(r.stereo(), "depth {depth}");
    }
}

#[test]
fn aq11_retune_in_the_middle() {
    let rate = 1e6;
    let o = opts(rate);
    let x = generate(rate, o.clone(), 14.0, &[("gen_seed", json!(3))]);
    let mut rx = receiver(&[]);
    let info = InputInfo::new(rate, DIAL - o.centre_offset_hz, DIAL).recording();
    rx.configure(&info).unwrap();
    let half = x.len() / 2;
    let _ = feed_all(rx.as_mut(), &x[..half], rate, 50_000);
    rx.retuned(&info);
    let r = feed_all(rx.as_mut(), &x[half..], rate, 50_000);
    assert!(r.stereo());
}

#[test]
fn aq12_reports_and_rate_refusal() {
    let r = run_gen(1e6, opts(1e6), 6.0, &snr(28.0), &[]);
    assert!(r.reports > 20, "{}", r.reports);
    assert!(r.stereo());
    assert!((3.5..=6.5).contains(&num(&r.r, "pilot_pct")));
    let mut rx = receiver(&[]);
    assert!(rx.configure(&InputInfo::new(200e3, DIAL, DIAL)).is_err());
}

#[test]
fn cut_independence() {
    let rate = 250e3;
    let x = generate(rate, opts(rate), 3.0, &[("gen_seed", json!(9))]);
    let mut outs = Vec::new();
    for piece in [1_000usize, 25_000, 77_777] {
        let mut rx = receiver(&[]);
        rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
            .unwrap();
        let r = feed_all(rx.as_mut(), &x, rate, piece);
        outs.push(r.left);
    }
    assert_eq!(outs[0], outs[1]);
    assert_eq!(outs[0], outs[2]);
}

#[test]
fn width_and_squelch_settings() {
    let r = run_gen(
        1e6,
        opts(1e6),
        6.0,
        &[],
        &[("width", json!(8000.0)), ("squelch", json!(false))],
    );
    assert_eq!(num(&r.r, "filter_width"), 8000.0);
    assert!(r.stereo());
}

#[test]
fn bad_settings_are_refused_not_replaced() {
    let mut rx = receiver(&[]);
    assert!(apply(rx.as_mut(), "width", json!(20000.0)).is_err());
    assert!(apply(rx.as_mut(), "stereo", json!("quad")).is_err());
    assert!(apply(rx.as_mut(), "squelch", json!("yes please")).is_err());
}

#[test]
fn the_pilot_detector_keeps_its_window_without_moving_samples() {
    // The pilot window is a ring of 1 s at 1 kHz: a long run allocates nothing per sample and keeps its result.
    let r = run_gen(1e6, opts(1e6), 12.0, &[], &[]);
    assert!(
        int(&r.r, "pilot_checks_ok") >= 18,
        "{}",
        int(&r.r, "pilot_checks_ok")
    );
}

/// A change of frequency forgets every measurement of the signal: the stack keeps nothing over it.
#[test]
fn a_change_of_frequency_forgets_the_measurements() {
    let rate = 1e6;
    let o = opts(rate);
    let centre_off = o.centre_offset_hz;
    let x = generate(rate, o, 4.0, &[]);
    let mut rx = receiver(&[]);
    rx.configure(&InputInfo::new(rate, DIAL - centre_off, DIAL).recording())
        .unwrap();
    let r = feed_all(rx.as_mut(), &x, rate, 65_536);
    assert!(r.stereo() && !is_none(&r.r, "cn_db") && int(&r.r, "pilot_checks_ok") > 0);
    rx.forget();
    let r = rx.results();
    for key in [
        "offset_hz",
        "cn_db",
        "carrier_dbfs",
        "pilot_db",
        "pilot_pct",
        "lock_quality",
        "left_dbfs",
        "right_dbfs",
        "width_db",
        "noise_dbfs",
    ] {
        assert!(is_none(r, key), "{key}");
    }
    assert!(!flag(r, "stereo") && !flag(r, "pilot_seen"));
    assert_eq!(int(r, "pilot_checks_ok"), 0);
    assert_eq!(num(r, "input_rate"), rate);
}
