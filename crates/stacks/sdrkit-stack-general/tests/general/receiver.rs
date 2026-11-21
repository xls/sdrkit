//! The acceptance cases of the receiver, cut independence, settings and configuration.

use crate::common::*;
use sdrkit_core::{
    Complex32, InputInfo, LockState, Output, Owner, Receiver, SettingSet, StackConfig, Value,
};
use sdrkit_stack_general::typed::GeneralFields;
use sdrkit_stack_general::{GeneralReceiver, GeneralSetting};

const RATE: f64 = 2e6;
const DIAL: f64 = -250e3;

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

#[test]
fn filter_edges_and_default_frequency() {
    let mut rx = receiver(RATE, DIAL, &[("demod", "usb")]);
    let mut g = generator(RATE, "usb", &[]);
    let r = run_gen(&mut rx, &mut g, 0.2, 20_000);
    assert_eq!(
        (r.last().filter_lo_hz, r.last().filter_hi_hz),
        (300.0, 3000.0)
    );
    let mut rx = receiver(RATE, DIAL, &[("demod", "lsb")]);
    let r = run_gen(&mut rx, &mut g, 0.2, 20_000);
    assert_eq!(
        (r.last().filter_lo_hz, r.last().filter_hi_hz),
        (-3000.0, -300.0)
    );
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let r = run_gen(&mut rx, &mut g, 0.2, 20_000);
    assert_eq!(
        (r.last().filter_lo_hz, r.last().filter_hi_hz),
        (-6250.0, 6250.0)
    );
    let mut rx = receiver(RATE, DIAL, &[("demod", "cw")]);
    // A width beyond the schema is refused. One inside it but beyond CW's range is brought into the range.
    assert!(set(&mut rx, "width", "1e6").is_err());
    set(&mut rx, "width", "220000.0").unwrap();
    let r = run_gen(&mut rx, &mut g, 0.2, 20_000);
    assert_eq!(r.last().width_hz, 3000.0);
    assert_eq!(r.last().filter_lo_abs_hz, 145.5e6 - 1500.0);
}

#[test]
fn each_demodulator_on_its_own_signal() {
    for (d, f, want) in [
        ("nfm", 1000.0, 0.25),
        ("wfm", 1000.0, 0.25),
        ("am", 1000.0, 0.25),
        ("usb", 1000.0, 0.5),
        ("lsb", 1000.0, 0.5),
        ("cw", 700.0, 0.5),
    ] {
        let mut rx = receiver(RATE, DIAL, &[("demod", d), ("deemph_wfm", "off")]);
        let mut g = generator(RATE, d, &[("gen_snr_db", "30")]);
        let r = run_gen(&mut rx, &mut g, 1.5, 20_000);
        assert!(r.audio.len() > 60_000, "{d}: {} samples", r.audio.len());
        let t = r.tail(0.75);
        let a = tone(t, f);
        assert!((db(a) - db(want)).abs() < 1.0, "{d}: tone {a}");
        assert!(
            tone(t, 1.37 * f) < 0.1 * a,
            "{d}: a tone where none was sent"
        );
        assert!(
            rms(t) * 2f64.sqrt() < 1.3 * a,
            "{d}: distortion, rms {}",
            rms(t)
        );
        let l = r.last();
        assert!(
            l.squelch_open && l.noise_valid && l.snr_db > 15.0,
            "{d}: {}",
            l.summary
        );
        assert_eq!(l.state, 2);
    }
}

#[test]
fn the_voice_test_signal_plays_in_fm_and_am() {
    for d in ["nfm", "am"] {
        let mut rx = receiver(RATE, DIAL, &[("demod", d)]);
        let mut g = generator(RATE, d, &[("gen_snr_db", "30"), ("gen_audio", "voice")]);
        let r = run_gen(&mut rx, &mut g, 1.5, 20_000);
        let t = r.tail(0.75);
        // Noise of the voice band, not the tone: no line at 1 kHz.
        assert!(rms(t) > 0.02, "{d}: rms {}", rms(t));
        assert!(tone(t, 1000.0) < 0.3 * rms(t), "{d}: a tone at 1 kHz");
        let l = r.last();
        assert!(l.squelch_open && l.snr_db > 15.0, "{d}: {}", l.summary);
        assert_eq!(l.state, 2);
    }
}

#[test]
fn a_station_off_the_dial() {
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm"), ("channel_offset", "25000")]);
    let mut g = generator(
        RATE,
        "nfm",
        &[("gen_snr_db", "30"), ("gen_offset_hz", "25000")],
    );
    let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
    let a = tone(r.tail(0.5), 1000.0);
    assert!((db(a) - db(0.25)).abs() < 1.0, "{a}");
    assert!(r.last().squelch_open);
    assert_eq!(r.last().channel_offset_hz, 25_000.0);
    assert_eq!(r.last().listening_hz, 145.525e6);
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let mut g = generator(
        RATE,
        "nfm",
        &[("gen_snr_db", "30"), ("gen_offset_hz", "25000")],
    );
    let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
    assert!(!r.last().squelch_open);
    assert!(rms(r.tail(0.5)) < 1e-4);
}

#[test]
fn noise_only_keeps_auto_closed() {
    for d in ["nfm", "wfm", "am", "usb", "lsb", "cw"] {
        let mut rx = receiver(RATE, DIAL, &[("demod", d)]);
        let mut g = generator(RATE, d, &[("gen_snr_db", "30"), ("gen_noise_only", "1")]);
        let r = run_gen(&mut rx, &mut g, 1.5, 20_000);
        let l = r.last();
        assert!(!l.squelch_open, "{d}");
        assert!(l.blocks_open < 5, "{d}: {} open blocks", l.blocks_open);
        assert!(rms(r.tail(1.0)) < 1e-4, "{d}");
    }
}

#[test]
fn level_squelch_and_a_weak_station() {
    let lvl = [
        ("demod", "nfm"),
        ("squelch", "level"),
        ("squelch_level", "-30"),
    ];
    let mut rx = receiver(RATE, DIAL, &lvl);
    let mut g = generator(
        RATE,
        "nfm",
        &[("gen_snr_db", "30"), ("gen_noise_only", "1")],
    );
    let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
    assert!(!r.last().squelch_open && rms(r.tail(0.5)) < 1e-4);
    assert!(
        (r.last().level_dbfs + 40.0).abs() < 2.0,
        "noise about -40 dBFS: {}",
        r.last().level_dbfs
    );
    let mut rx = receiver(RATE, DIAL, &lvl);
    let mut g = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
    let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
    assert!(r.last().squelch_open && tone(r.tail(0.5), 1000.0) > 0.2);
    assert_eq!(r.last().squelch_threshold_dbfs, -30.0);
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm"), ("squelch", "off")]);
    let mut g = generator(
        RATE,
        "nfm",
        &[("gen_snr_db", "30"), ("gen_noise_only", "1")],
    );
    let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
    assert!(r.last().squelch_open && rms(r.tail(0.5)) > 0.01);
    assert_eq!(r.last().squelch_threshold_dbfs, -200.0);
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let mut g = generator(RATE, "nfm", &[("gen_snr_db", "8")]);
    let r = run_gen(&mut rx, &mut g, 1.5, 20_000);
    assert!(
        r.last().squelch_open,
        "a weak station opens Auto: {}",
        r.last().summary
    );
}

#[test]
fn wfm_de_emphasis() {
    let run10k = |deemph: &str, f: &str| {
        let mut rx = receiver(RATE, DIAL, &[("demod", "wfm"), ("deemph_wfm", deemph)]);
        let mut g = generator(RATE, "wfm", &[("gen_snr_db", "40"), ("gen_tone", f)]);
        let r = run_gen(&mut rx, &mut g, 1.0, 20_000);
        tone(r.tail(0.5), f.parse().unwrap())
    };
    let off = run10k("off", "10000");
    let on = run10k("50", "10000");
    let rel = db(on) - db(off);
    assert!((-11.5..=-8.5).contains(&rel), "{rel}");
    let low = run10k("50", "300");
    assert!((db(low) - db(0.25)).abs() < 1.0, "{low}");
}

#[test]
fn audio_low_pass() {
    let p = |lp: &str| {
        let mut rx = receiver(
            RATE,
            DIAL,
            &[("demod", "nfm"), ("squelch", "off"), ("af_lowpass", lp)],
        );
        let mut g = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
        let r = run_gen(&mut rx, &mut g, 3.0, 20_000);
        rms(r.tail(2.0)).powi(2)
    };
    assert!(p("100") < 0.5 * p("0"));
}

/// The NFM audio low-pass is a setting, 3 kHz by default. FM noise above the threshold grows as the cube of the audio
/// bandwidth: 4 kHz has (4/3)^3, 3.7 dB, more noise than 3 kHz. A tone at 2.7 kHz keeps its level.
#[test]
fn nfm_audio_low_pass_is_a_setting_of_3_khz() {
    let run = |settings: &[(&str, &str)], gen: &[(&str, &str)]| {
        let mut s = vec![("demod", "nfm"), ("squelch", "off")];
        s.extend_from_slice(settings);
        let mut rx = receiver(RATE, DIAL, &s);
        let mut g = generator(RATE, "nfm", gen);
        run_gen(&mut rx, &mut g, 3.0, 20_000)
    };
    // A bare carrier at 20 dB in 12.5 kHz: the audio is the noise of the discriminator.
    let carrier = [("gen_snr_db", "20"), ("gen_mod", "0")];
    let noise = |settings: &[(&str, &str)]| rms(run(settings, &carrier).tail(2.0)).powi(2);
    let n3 = noise(&[("nfm_lowpass", "3khz")]);
    let n4 = noise(&[("nfm_lowpass", "4khz")]);
    let gain_db = 10.0 * (n4 / n3).log10();
    let theory_db = 30.0 * (4.0f64 / 3.0).log10();
    assert!(
        (gain_db - theory_db).abs() < 0.25,
        "{gain_db:.2} dB, not {theory_db:.2}"
    );
    assert_eq!(noise(&[]), n3, "the default is 3 kHz");
    let tone = |lp: &str| {
        let gen = [("gen_snr_db", "40"), ("gen_tone", "2700")];
        tone(run(&[("nfm_lowpass", lp)], &gen).tail(2.0), 2700.0)
    };
    let kept_db = db(tone("3khz") / tone("4khz"));
    assert!(
        kept_db.abs() < 0.2,
        "the 2.7 kHz tone changed by {kept_db:.2} dB"
    );
}

#[test]
fn voice_noise_remover() {
    let go = |settings: &[(&str, &str)]| {
        let mut s = vec![("demod", "nfm")];
        s.extend_from_slice(settings);
        let mut rx = receiver(RATE, DIAL, &s);
        let mut g = generator(RATE, "nfm", &[("gen_snr_db", "12")]);
        run_gen(&mut rx, &mut g, 1.5, 20_000).audio
    };
    let plain = go(&[]);
    let off = go(&[("voice_nr", "0")]);
    assert_eq!(plain, off, "off is bit-identical");
    let on = go(&[("voice_nr", "1"), ("voice_nr_strength", "80")]);
    assert_eq!(on.len(), plain.len());
    assert!(on.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
    assert_ne!(on, plain);
}

fn audio_with_cuts(settings: &[(&str, &str)], demod: &str, sizes: &[usize]) -> (Vec<f32>, String) {
    let mut rx = receiver(RATE, DIAL, settings);
    let mut g = generator(RATE, demod, &[("gen_snr_db", "20")]);
    let total = 1_000_000;
    let mut x = vec![Complex32::default(); total];
    g.generate(&mut x);
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut i = 0;
    let mut k = 0;
    while i < total {
        let n = sizes[k % sizes.len()].min(total - i);
        out.clear();
        rx.feed(&x[i..i + n], &mut out);
        audio.extend(first_channel(&out));
        i += n;
        k += 1;
    }
    let d = Detail::of(Owner::results(&rx));
    (
        audio,
        format!(
            "{} {} {} {}",
            d.blocks_closed, d.level_dbfs, d.noise_dbfs, d.blocks_open
        ),
    )
}

#[test]
fn any_cut_returns_the_same_result() {
    for (demod, s) in [
        ("nfm", vec![("demod", "nfm"), ("nb", "1")]),
        (
            "am",
            vec![("demod", "am"), ("am_auto", "1"), ("am_detector", "auto")],
        ),
        ("usb", vec![("demod", "usb")]),
        ("wfm", vec![("demod", "wfm")]),
    ] {
        let a = audio_with_cuts(&s, demod, &[200_000]);
        let b = audio_with_cuts(&s, demod, &[7777, 1, 30_001, 512]);
        let c = audio_with_cuts(&s, demod, &[1_000_000]);
        assert!(
            a.0 == b.0 && a.0 == c.0,
            "{demod}: audio differs with the cut"
        );
        assert_eq!(a.1, b.1, "{demod}");
        assert_eq!(a.1, c.1, "{demod}");
    }
}

#[test]
fn settings_take_effect() {
    // A step and the channel heard.
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    set(&mut rx, "step", "25000").unwrap();
    assert_eq!(rx.settings().step(), 25_000.0);
    set(&mut rx, "listen_channel", "1.0").unwrap();
    assert_eq!(rx.settings().listen_channel, 1);
    assert!(set(&mut rx, "listen_channel", "4.0").is_err());
    // CW pitch.
    let mut rx = receiver(RATE, DIAL, &[("demod", "cw")]);
    let mut g = generator(RATE, "cw", &[("gen_snr_db", "30")]);
    run_gen(&mut rx, &mut g, 0.5, 20_000);
    set(&mut rx, "cw_pitch", "900.0").unwrap();
    let r = run_gen(&mut rx, &mut g, 0.5, 20_000);
    assert!(tone(r.tail(0.25), 900.0) > 0.4);
    // Manual gain with AGC off: 20 dB more gain, 20 dB more audio.
    let level = |gain: &str| {
        let mut rx = receiver(
            RATE,
            DIAL,
            &[("demod", "usb"), ("agc", "0"), ("gain", gain)],
        );
        let mut g = generator(
            RATE,
            "usb",
            &[("gen_snr_db", "30"), ("gen_level_dbfs", "-50.457")],
        );
        tone(run_gen(&mut rx, &mut g, 0.6, 20_000).tail(0.3), 1000.0)
    };
    let (a, b) = (level("20"), level("40"));
    assert!((db(b) - db(a) - 20.0).abs() < 0.5, "{a} {b}");
    // A narrower width shows in the report and the filter edges.
    let mut rx = receiver(RATE, DIAL, &[("demod", "am")]);
    set(&mut rx, "width", "6000.0").unwrap();
    let r = run_gen(&mut rx, &mut generator(RATE, "am", &[]), 0.2, 20_000);
    assert_eq!(
        (r.last().filter_lo_hz, r.last().filter_hi_hz),
        (-3000.0, 3000.0)
    );
    // A demodulator change keeps the other demodulator's width.
    set(&mut rx, "demod", "nfm").unwrap();
    set(&mut rx, "demod", "am").unwrap();
    let r = run_gen(&mut rx, &mut generator(RATE, "am", &[]), 0.2, 20_000);
    assert_eq!(r.last().width_hz, 6000.0);
    // Switching to WFM and back reformats the chain and still decodes.
    set(&mut rx, "demod", "wfm").unwrap();
    let r = run_gen(
        &mut rx,
        &mut generator(RATE, "wfm", &[("gen_snr_db", "30")]),
        0.6,
        20_000,
    );
    assert_eq!(r.last().demod_rate_hz, 240e3);
    assert!(tone(r.tail(0.2), 1000.0) > 0.2);
    set(&mut rx, "demod", "nfm").unwrap();
    let r = run_gen(
        &mut rx,
        &mut generator(RATE, "nfm", &[("gen_snr_db", "30")]),
        0.6,
        20_000,
    );
    assert!(tone(r.tail(0.2), 1000.0) > 0.2);
    // Unknown keys and bad values are refused.
    assert!(set(&mut rx, "volume", "1.0").is_err());
    assert!(
        sdrkit_core::owner::apply(&mut rx, GeneralSetting::Demod.id(), &Value::Choice(99)).is_err()
    );
}

#[test]
fn a_channel_move_restarts_the_measures() {
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let mut g = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
    let r = run_gen(&mut rx, &mut g, 0.495, 495_000);
    assert_eq!(r.last().state, 2);
    // The move just before a report: no squelch block since, so the report says state 0.
    set(&mut rx, "channel_offset", "1000.0").unwrap();
    let r = run_gen(&mut rx, &mut g, 0.005, 10_000);
    assert_eq!(r.last().state, 0, "{}", r.last().summary);
    assert!(r.last().summary.ends_with("no signal yet"));
    assert_eq!(r.last().level_dbfs, -200.0);
    // The noise estimate starts again: the squelch stays shut until it is back (about 33 ms), then opens.
    let r = run_gen(&mut rx, &mut g, 0.5, 20_000);
    assert_eq!(r.last().state, 2);
    assert_eq!(r.last().channel_offset_hz, 1000.0);
}

#[test]
fn a_retune_starts_again() {
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let mut g = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
    let before = run_gen(&mut rx, &mut g, 0.5, 20_000);
    let blocks = before.last().blocks_open;
    rx.retuned(&InputInfo::new(RATE, 145.75e6, 145.5e6).recording());
    // Filters, squelch and noise estimate start again: closed (silent) until the first noise estimate.
    let r = run_gen(&mut rx, &mut g, 0.02, 20_000);
    assert!(rms(&r.audio) < 1e-4, "silent right after the retune");
    let r = run_gen(&mut rx, &mut g, 0.5, 20_000);
    assert!(r.last().squelch_open);
    assert!(r.last().blocks_open > blocks, "the counters are kept");
}

/// A change of frequency by the host forgets the level, the squelch and the tone of the frequency before, and keeps
/// the presets and the width presets.
#[test]
fn a_change_of_frequency_forgets_the_signal_and_keeps_the_presets() {
    use sdrkit_stack_general::typed::{PRESETS, WIDTH_PRESETS};
    let text = crate::common::presets_text();
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm"), ("presets", &text)]);
    let mut g = generator(RATE, "nfm", &[("gen_ctcss", "88.5"), ("gen_snr_db", "30")]);
    let r = run_gen(&mut rx, &mut g, 2.0, 20_000);
    assert!(r.last().squelch_open);
    let f = Owner::results(&rx).fields();
    assert!(f.at(&GeneralFields::LEVEL_DBFS).is_some());
    assert!(f.at(&GeneralFields::TONE).is_some(), "the tone is found");
    let gate = f.at(&GeneralFields::TONE_GATE_OPEN);
    let widths = WIDTH_PRESETS
        .rows::<sdrkit_stack_general::typed::WidthPresetRow>(Owner::results(&rx))
        .count();
    assert!(widths > 0);
    rx.retuned(&InputInfo::new(RATE, 145.75e6, 145.5e6).recording());
    rx.forget();
    let r = Owner::results(&rx);
    let f = r.fields();
    assert_eq!(
        f.at(&GeneralFields::LEVEL_DBFS),
        None,
        "the level is forgotten"
    );
    assert_eq!(f.at(&GeneralFields::SNR_DB), None);
    assert_eq!(f.at(&GeneralFields::SQUELCH_OPEN), Some(false));
    assert_eq!(f.at(&GeneralFields::TONE), None, "the tone is forgotten");
    assert_eq!(f.at(&GeneralFields::CTCSS_HZ), None);
    assert_eq!(
        PRESETS
            .rows::<sdrkit_stack_general::typed::PresetRow>(r)
            .count(),
        crate::common::presets().len(),
        "the presets are kept"
    );
    assert_eq!(
        WIDTH_PRESETS
            .rows::<sdrkit_stack_general::typed::WidthPresetRow>(r)
            .count(),
        widths,
        "the width presets are kept"
    );
    // The signal is found again on the new frequency.
    run_gen(&mut rx, &mut g, 2.0, 20_000);
    let f = Owner::results(&rx).fields();
    assert!(f.at(&GeneralFields::LEVEL_DBFS).is_some());
    assert!(
        f.at(&GeneralFields::TONE).is_some(),
        "the tone is found again"
    );
    assert_eq!(f.at(&GeneralFields::TONE_GATE_OPEN), gate);
}

#[test]
fn rates_below_250_ksps_are_refused() {
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&[]))).unwrap();
    let e = rx
        .configure(&InputInfo::new(200e3, 145.5e6, 145.5e6))
        .unwrap_err();
    assert!(
        matches!(e, sdrkit_core::ConfigError::RateTooLow { .. }),
        "{e}"
    );
    assert_eq!(
        Owner::results(&rx).status().summary,
        "The general stack needs a sample rate of at least 250 ksps"
    );
    assert!(rx
        .configure(&InputInfo::new(250e3, 145.55e6, 145.5e6))
        .is_ok());
}

#[test]
fn the_am_submode_opens_in_am() {
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&[])).with_submode("am")).unwrap();
    rx.configure(&InputInfo::new(RATE, 1e6, 999e3)).unwrap();
    assert_eq!(rx.settings().demod.id(), "am");
    assert_eq!(rx.settings().width(), 9000.0);
}

#[test]
fn reports_come_every_tenth_of_a_second() {
    let mut rx = receiver(RATE, DIAL, &[("demod", "nfm")]);
    let mut g = generator(RATE, "nfm", &[("gen_snr_db", "30")]);
    let r = run_gen(&mut rx, &mut g, 1.0, 200_000);
    let blocks: Vec<u64> = r
        .reports
        .iter()
        .map(|d| d.blocks_open + d.blocks_closed)
        .collect();
    assert_eq!(blocks, (1..=10).map(|k| 10 * k).collect::<Vec<_>>());
    let r = Owner::results(&rx);
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!(r.status().summary.starts_with("NFM 12.5 kHz"));
    assert_eq!(r.fields().at(&GeneralFields::LISTENING_HZ), Some(145.5e6));
    assert!(r.status().snr_db.unwrap() > 20.0);
}
