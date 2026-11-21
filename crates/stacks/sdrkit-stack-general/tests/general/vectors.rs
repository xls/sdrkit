//! The recorded test vectors of the general receiver decode as their expected values say.

use crate::common::*;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, StackConfig};
use sdrkit_dev_test::sigmf;
use sdrkit_stack_general::listen::{ChainSettings, ChainSquelch, ListeningChain};
use sdrkit_stack_general::GeneralReceiver;

struct Decoded {
    timeline: String,
    run: Run,
}

fn decode(name: &str, extra: &[(&str, &str)]) -> Option<Decoded> {
    let exp = sigmf::read_expected("general").unwrap();
    let e = &exp["recordings"][name];
    let rec = sdrkit_dev_test::read_recording(&format!("general/{name}"))?;
    let tuned = e["tuned_hz"].as_f64().unwrap();
    let demod = e["demod"].as_str().unwrap();
    let mut s = vec![
        ("demod", demod),
        ("am_auto", "0"),
        ("am_detector", "envelope"),
        ("deemph_nfm", "off"),
    ];
    s.extend_from_slice(extra);
    let mut rx = GeneralReceiver::new(&StackConfig::new(values(&s))).unwrap();
    assert_eq!(rec.centre_hz, tuned + 50e3);
    rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, tuned).recording())
        .unwrap();
    let mut r = Run::default();
    let mut pos = 0;
    let x = &rec.samples;
    run(
        &mut rx,
        x.len() as f64 / rec.rate_hz,
        rec.rate_hz,
        25_000,
        |b: &mut [Complex32]| {
            b.copy_from_slice(&x[pos..pos + b.len()]);
            pos += b.len();
        },
        &mut r,
    );
    let timeline: String = r
        .reports
        .iter()
        .map(|d| if d.state == 2 { 'O' } else { '.' })
        .collect();
    Some(Decoded { timeline, run: r })
}

fn check(name: &str) -> Option<Decoded> {
    let exp = sigmf::read_expected("general").unwrap();
    let e = &exp["recordings"][name];
    let d = decode(name, &[])?;
    let want = e["squelch_timeline"].as_str().unwrap();
    let closed = want.chars().take_while(|&c| c == '.').count();
    // "Closed in the first N reports, open from N+2 on."
    assert!(
        d.timeline[..closed].chars().all(|c| c == '.'),
        "{name}: {}",
        d.timeline
    );
    assert!(
        d.timeline[closed + 1..].chars().all(|c| c == 'O'),
        "{name}: {}",
        d.timeline
    );
    let l = d.run.last();
    let f = &e["final"];
    assert_eq!(l.state, 2);
    assert!(
        (l.level_dbfs - f["level_dbfs"].as_f64().unwrap()).abs() <= 1.0,
        "{name}: level {}",
        l.level_dbfs
    );
    let noise_tol = e["tolerance"]["noise_dbfs"].as_f64().unwrap_or(3.0);
    if let Some(n) = f["noise_dbfs"].as_f64() {
        assert!(
            (l.noise_dbfs - n).abs() <= noise_tol,
            "{name}: noise {}",
            l.noise_dbfs
        );
    }
    let min_snr: f64 = e["tolerance"]["snr_db"]
        .as_str()
        .unwrap()
        .trim_start_matches("at least ")
        .parse()
        .unwrap();
    assert!(l.snr_db >= min_snr, "{name}: snr {}", l.snr_db);
    assert!((l.squelch_threshold_dbfs - (l.noise_dbfs + 6.0)).abs() < 0.1);
    let (filter_lo, filter_hi) = (
        e["filter_hz"][0].as_f64().unwrap(),
        e["filter_hz"][1].as_f64().unwrap(),
    );
    assert_eq!((l.filter_lo_hz, l.filter_hi_hz), (filter_lo, filter_hi));
    let a = &e["audio"];
    let tol = &e["tolerance"]["tone_amp"];
    let amp = tone(d.run.tail(0.5), a["tone_hz"].as_f64().unwrap());
    assert!(
        amp >= tol[0].as_f64().unwrap() && amp <= tol[1].as_f64().unwrap(),
        "{name}: tone {amp}"
    );
    // Silent while closed.
    let closed_samples = (closed as f64 * 0.1 * 48_000.0) as usize;
    assert!(
        rms(&d.run.audio[..closed_samples]) < 1e-4,
        "{name}: not silent while closed"
    );
    Some(d)
}

#[test]
fn am_recording() {
    check("general_am_250k");
}

#[test]
fn usb_recording() {
    check("general_usb_250k");
}

#[test]
fn cw_recording() {
    check("general_cw_250k");
}

#[test]
fn nfm_ctcss_recording() {
    let Some(d) = check("general_nfm_ctcss_250k") else {
        return;
    };
    let l = d.run.last();
    assert!(l.freq_offset_hz.abs() <= 50.0, "{}", l.freq_offset_hz);
    let c = tone(d.run.tail(0.5), 88.5);
    assert!((0.045..=0.07).contains(&c), "CTCSS in the audio {c}");
    assert_eq!(l.ctcss_hz, Some(88.5), "detected tone");
    use sdrkit_proto_tones::components::{SubAudioChange, CHANGE, SUBAUDIO_EVENT, TONE_HZ};
    let locked = d
        .run
        .events
        .iter()
        .find(|e| e.is(&SUBAUDIO_EVENT) && e.get(&CHANGE) == Some(SubAudioChange::Locked));
    let locked = locked.expect("a CTCSS lock event");
    assert_eq!(locked.get(&TONE_HZ), Some(88.5));
    assert!(locked.freq_hz > 0.0, "the event names its frequency");
}

fn tone_squelch(tone_hz: f32) -> Option<(String, Vec<f32>)> {
    let d = decode("general_nfm_ctcss_250k", &[])?;
    let mut s = ChainSettings::default();
    s.control.on = true;
    s.control.squelch = ChainSquelch::Ctcss;
    s.control.ctcss_hz = tone_hz;
    s.control.squelch_hang_ms = 250.0;
    let mut chain = ListeningChain::new(48e3, s);
    let mut out = Vec::new();
    let mut timeline = String::new();
    for block in d.run.audio.chunks(4800) {
        let mut st: Vec<[f32; 2]> = block.iter().map(|&v| [v, v]).collect();
        chain.process(&mut st);
        timeline.push(if chain.control().squelch_open() {
            'O'
        } else {
            '.'
        });
        out.extend(st.iter().map(|f| f[0]));
    }
    Some((timeline, out))
}

#[test]
fn tone_squelch_on_the_nfm_recording() {
    let Some((t, out)) = tone_squelch(88.5) else {
        return;
    };
    let first_open = t.find('O').expect("opens");
    // The delay from the first sound to the opening, measured in 10 ms steps.
    let d = decode("general_nfm_ctcss_250k", &[]).unwrap();
    let sound = d.run.audio.iter().position(|v| v.abs() > 0.01).unwrap();
    let mut s = ChainSettings::default();
    s.control.on = true;
    s.control.squelch = ChainSquelch::Ctcss;
    s.control.ctcss_hz = 88.5;
    let mut chain = ListeningChain::new(48e3, s);
    let mut opened = None;
    for (k, block) in d.run.audio.chunks(480).enumerate() {
        let mut st: Vec<[f32; 2]> = block.iter().map(|&v| [v, v]).collect();
        chain.process(&mut st);
        if opened.is_none() && chain.control().squelch_open() {
            opened = Some((k + 1) * 480);
        }
    }
    let delay = (opened.unwrap() - sound) as f64 / 48e3;
    eprintln!("tone squelch 88.5 Hz opens {delay:.2} s after the sound starts; per 0.1 s: {t}");
    assert!(delay < 0.5, "{delay}");
    assert!(first_open < 9, "opens within 0.9 s of audio: {t}");
    assert!(t[first_open..].chars().all(|c| c == 'O'), "stays open: {t}");
    let tail = &out[out.len() - 24_000..];
    let k = tone(tail, 1000.0);
    assert!((k - 0.2495).abs() < 0.01, "1 kHz passes: {k}");
    let c = tone(tail, 88.5);
    assert!(
        20.0 * (0.0588 / c).log10() >= 15.0,
        "88.5 Hz taken out: {c}"
    );
    let (t, out) = tone_squelch(100.0).unwrap();
    assert!(!t.contains('O'), "never opens on 100 Hz: {t}");
    assert!(rms(&out[out.len() - 24_000..]) < 1e-4);
}

/// The verification hook of the stack (`verify`) passes on every recording, run as a host runs it: the settings of the hook,
/// pieces of 0.1 s, the results as JSON after each piece.
#[test]
fn the_verify_hook_passes_on_every_recording() {
    use sdrkit_core::verify::{Outcome, VectorRun};
    let exp = sigmf::read_expected("general").unwrap();
    let hooks = sdrkit_stack_general::DESCRIPTOR
        .verify
        .expect("verify hooks");
    let dir = sdrkit_dev_test::sigmf::vectors_dir().join("general");
    for name in [
        "general_am_250k",
        "general_nfm_ctcss_250k",
        "general_usb_250k",
        "general_cw_250k",
    ] {
        let section = &exp["recordings"][name];
        let Some(rec) = sdrkit_dev_test::read_recording(&format!("general/{name}")) else {
            return;
        };
        let case = (hooks.vector_settings.unwrap())(name, section, &exp);
        let mut v = values(&[]);
        CaseSetting::apply_all(&case, &mut v).unwrap();
        let mut rx = GeneralReceiver::new(&StackConfig::new(v)).unwrap();
        let tuned = section["tuned_hz"].as_f64().unwrap();
        rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, tuned).recording())
            .unwrap();
        let (mut audio, mut events, mut reports) = (Vec::new(), Vec::new(), Vec::new());
        let mut out = sdrkit_core::Output::default();
        let piece = (rec.rate_hz / 10.0) as usize;
        for (k, p) in rec.samples.chunks(piece).enumerate() {
            out.clear();
            rx.feed(p, &mut out);
            audio.extend_from_slice(out.audio());
            events.append(&mut out.events);
            reports.push(((k + 1) as f64 * 0.1, Owner::results(&rx).clone()));
        }
        let results = Owner::results(&rx).clone();
        let run = VectorRun {
            name,
            section,
            expected: &exp,
            results: &results,
            reports: &reports,
            audio: &audio,
            audio_rate_hz: 48_000.0,
            audio_channels: 1,
            events: &events,
            seconds: rec.samples.len() as f64 / rec.rate_hz,
            dir: &dir,
        };
        let lines = (hooks.check_vector.unwrap())(&run);
        assert!(lines.len() >= 8, "{name}: {} checks", lines.len());
        for line in lines {
            assert!(
                line.outcome != Outcome::Fail,
                "{name}: {} {}",
                line.key,
                line.detail
            );
        }
    }
}
