//! The recorded test vector decodes as `tests/vectors/dab/expected.json` says.
#![allow(clippy::needless_range_loop)]

use super::common::*;
use sdrkit_core::{Choice, LockState, StreamKind};
use sdrkit_dev_test::sigmf;
use sdrkit_stack_dab::results::*;
use sdrkit_stack_dab::typed::*;

fn load() -> Option<(Vec<sdrkit_core::Complex32>, serde_json::Value)> {
    let rec = sdrkit_dev_test::read_recording("dab/dab_ensemble_2048k")?;
    let exp = sigmf::read_expected("dab").ok()?;
    Some((rec.samples, exp))
}

#[test]
fn the_vector_decodes_as_expected() {
    let Some((x, exp)) = load() else {
        return;
    };
    let sig = &exp["signal"];
    for sel in 1..=4 {
        let r = run(&x, 2.048e6, sel, 20_480);
        let e = &exp["per_selection"][sel.to_string()];
        // Signal.
        assert_eq!(r.lock_state(), LockState::Locked, "locked");
        assert_eq!(r.at(&SyncFields::SYNC), Some(true));
        let frames = r.count(&SyncFields::FRAMES);
        assert!((23..=25).contains(&frames), "frames {frames}");
        assert!(r.count(&SyncFields::FIB_OK) >= 276);
        assert_eq!(r.count(&SyncFields::FIB_BAD), 0);
        assert_eq!(
            r.count(&SyncFields::FIB_OK_LAST_FRAME),
            sig["fib_ok_last_frame"].as_i64().unwrap()
        );
        let cfo = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
        assert!((1175.0..=1225.0).contains(&cfo), "cfo {cfo}");
        assert!(r.at(&SignalFields::SNR_DB).unwrap_or(0.0) >= 18.0);
        let ppm = r.at(&SignalFields::CLOCK_PPM).unwrap_or(0.0);
        assert!((-5.0..=5.0).contains(&ppm));
        let detection = r
            .at(&SyncFields::DETECTION)
            .map(|d| Detection::CHOICES[usize::from(d.index())].key.to_string());
        assert_eq!(detection.as_deref(), sig["detection"].as_str());
        assert_eq!(r.count(&SyncFields::SYNC_LOSSES), 0);
        assert_eq!(r.count(&SyncFields::ERASED_CIFS), 0);
        assert_eq!(r.count(&SyncFields::IMPULSES_BLANKED), 0);
        // Ensemble.
        let ee = &exp["ensemble"];
        let eid = r.at(&EnsembleFields::EID).map(|e| format!("{e:04X}"));
        assert_eq!(eid.as_deref(), ee["eid"].as_str());
        assert_eq!(r.at(&EnsembleFields::ENSEMBLE_LABEL), ee["label"].as_str());
        assert_eq!(
            r.at(&EnsembleFields::ENSEMBLE_SHORT_LABEL),
            ee["short_label"].as_str()
        );
        let t = r.at(&EnsembleFields::UTC).unwrap() as i64;
        assert!((1_700_000_000..=1_700_000_003).contains(&t), "time {t}");
        let subchannels = r.rows(&SUBCHANNELS);
        assert_eq!(
            subchannels.len(),
            ee["subchannels"].as_array().unwrap().len()
        );
        for s in ee["subchannels"].as_array().unwrap() {
            let g = subchannels
                .iter()
                .find(|(k, _)| k.parse::<i64>().ok() == s["id"].as_i64())
                .unwrap()
                .1;
            assert_eq!(g.at(&SubchannelRow::ID), s["id"].as_i64());
            assert_eq!(g.at(&SubchannelRow::START_CU), s["start_cu"].as_i64());
            assert_eq!(g.at(&SubchannelRow::SIZE_CU), s["size_cu"].as_i64());
            assert_eq!(g.at(&SubchannelRow::PROTECTION), s["protection"].as_str());
            assert_eq!(
                g.at(&SubchannelRow::BITRATE_KBPS),
                s["bitrate_kbps"].as_i64()
            );
        }
        let services = r.rows(&SERVICES);
        for s in ee["services"].as_array().unwrap() {
            let sid = s["sid"].as_str().unwrap();
            let g = services.iter().find(|(k, _)| *k == sid).unwrap().1;
            let kind = g.at(&ServiceRow::KIND).map(|k| {
                ServiceKind::CHOICES[usize::from(k.index())]
                    .label
                    .to_string()
            });
            assert_eq!(kind.as_deref(), s["kind"].as_str(), "{sid}");
            assert_eq!(g.at(&ServiceRow::LABEL), s["label"].as_str(), "{sid}");
            assert_eq!(
                g.at(&ServiceRow::SHORT_LABEL),
                s["short_label"].as_str(),
                "{sid}"
            );
            assert_eq!(
                g.at(&ServiceRow::COMPONENT_TYPE),
                s["ascty"].as_i64(),
                "{sid}"
            );
            assert_eq!(
                g.at(&ServiceRow::SUBCHANNEL),
                s["subchannel"].as_i64(),
                "{sid}"
            );
            assert_eq!(g.at(&ServiceRow::PTY), s["pty"].as_i64(), "{sid}");
            assert_eq!(
                g.at(&ServiceRow::PTY_STATIC),
                s["pty_static"].as_bool(),
                "{sid}"
            );
            assert_eq!(g.at(&ServiceRow::LANGUAGE), s["language"].as_i64(), "{sid}");
        }
        // The selected sub-channel.
        let lf = r.count(&AudioFields::AUDIO_LOGICAL_FRAMES);
        assert!(
            (lf - e["logical_frames"].as_i64().unwrap()).abs() <= 2,
            "sub {sel}: logical frames {lf}"
        );
        if sel < 4 {
            let profile = r.at(&AudioFields::AUDIO_CODEC).map(|c| {
                AudioCodec::CHOICES[usize::from(c.index())]
                    .label
                    .to_string()
            });
            assert_eq!(profile.as_deref(), e["profile"].as_str());
            assert_eq!(r.at(&AudioFields::AUDIO_RATE), e["sample_rate"].as_f64());
            assert_eq!(r.at(&AudioFields::AUDIO_CHANNELS), e["channels"].as_i64());
            assert_eq!(
                r.at(&AudioFields::AUDIO_UNITS_PER_SUPERFRAME),
                e["access_units_per_superframe"].as_i64()
            );
            assert!(r.count(&AudioFields::AUDIO_SUPERFRAMES_OK) >= 15);
            assert_eq!(r.count(&AudioFields::AUDIO_SUPERFRAMES_BAD), 0);
            assert_eq!(r.count(&AudioFields::AUDIO_RS_CORRECTED), 0);
            assert_eq!(r.count(&AudioFields::AUDIO_UNITS_BAD), 0);
            let ok = r.count(&AudioFields::AUDIO_UNITS_OK) as f64;
            let want = e["access_units_ok"].as_f64().unwrap();
            let slack = if sel == 2 { 4.0 } else { 6.0 };
            assert!(
                ok >= 0.85 * want && ok <= want + slack,
                "sub {sel}: {ok} access units"
            );
            let units = r.units(StreamKind::Aac);
            assert_eq!(units.len() as f64, ok);
            assert_eq!(
                units[0].format.fourcc().as_str(),
                e["fourcc"].as_str().unwrap()
            );
            let asc: String = units[0]
                .format
                .config
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(asc, e["audio_specific_config_hex"].as_str().unwrap());
            if let Some(tones) = e["tones"].as_object() {
                if let Some((pcm, rate)) = ffmpeg_decode(&units) {
                    let l = left(&pcm);
                    let rr = right(&pcm);
                    let n = rate as usize;
                    let (lt, rt) = (&l[l.len() - n..], &rr[rr.len() - n..]);
                    let (fl, fr) = (
                        tones["left_hz"].as_f64().unwrap(),
                        tones["right_hz"].as_f64().unwrap(),
                    );
                    let al = tone(lt, rate as f64, fl);
                    let ar = tone(rt, rate as f64, fr);
                    assert!((20.0 * (al / 0.5).log10()).abs() < 0.3, "left {al}");
                    assert!((20.0 * (ar / 0.5).log10()).abs() < 0.3, "right {ar}");
                    assert!(tone(rt, rate as f64, fl) < al * 0.01);
                }
            }
        } else {
            assert_eq!(r.at(&AudioFields::AUDIO_CODEC), Some(AudioCodec::Mp2));
            assert_eq!(r.at(&AudioFields::AUDIO_RATE), Some(48_000.0));
            assert_eq!(r.at(&AudioFields::AUDIO_CHANNELS), Some(2));
            assert_eq!(r.at(&PlayingFields::SELECTED_BITRATE_KBPS), Some(128));
            let frames = r.units(StreamKind::Mp2);
            assert_eq!(frames[0].format.fourcc().as_str(), ".mp2");
            assert_eq!(frames[0].packet.bytes().len(), 384);
            let n = r.count(&AudioFields::AUDIO_MP2_FRAMES);
            assert!((n - 81).abs() <= 2, "mp2 frames {n}");
            // The stack's own Layer II decoder: the tones of the last second.
            let l = left(&r.audio);
            let rr = right(&r.audio);
            assert!(l.len() > 48_000);
            let (lt, rt) = (&l[l.len() - 48_000..], &rr[rr.len() - 48_000..]);
            let al = tone(lt, 48e3, 1500.0);
            let ar = tone(rt, 48e3, 750.0);
            assert!((20.0 * (al / 0.5).log10()).abs() < 1.0, "left {al}");
            assert!((20.0 * (ar / 0.5).log10()).abs() < 1.0, "right {ar}");
            assert!(tone(rt, 48e3, 1500.0) < al * 0.01);
        }
        if sel == 3 {
            assert_eq!(
                r.at(&sdrkit_proto_bcdata::publish::LabelFields::TEXT),
                e["dynamic_label"].as_str()
            );
            let charset = r
                .at(&PadFields::LABEL_CHARSET)
                .map(|c| sdrkit_stack_dab::registry::charset_name(c as u8));
            assert_eq!(charset, e["dynamic_label_charset"].as_str());
            let es = &e["slide"];
            assert_eq!(
                r.at(&sdrkit_proto_bcdata::publish::SlideFields::SLIDE_NAME),
                es["name"].as_str()
            );
            assert_eq!(r.at(&PadFields::SLIDE_WIDTH), es["width"].as_i64());
            assert_eq!(r.at(&PadFields::SLIDE_HEIGHT), es["height"].as_i64());
            assert_eq!(r.at(&PadFields::SLIDE_BYTES), es["bytes"].as_i64());
            assert_eq!(r.at(&PadFields::SLIDE_SEQUENCE), Some(1));
            let pics = r.units(StreamKind::Picture);
            assert_eq!(pics.len(), 1);
            if let Some(p) = sdrkit_dev_test::recording("dab/slide.png") {
                let want = std::fs::read(p).unwrap();
                assert_eq!(pics[0].packet.bytes(), &want[..]);
            }
            assert_eq!(
                pics[0].format.fourcc().as_str(),
                es["fourcc"].as_str().unwrap()
            );
            assert!(r.at(&TimingFields::TIMING_SLIDE).unwrap() <= 2.2);
            assert!(r.at(&TimingFields::TIMING_LABEL).unwrap() <= 1.0);
        } else {
            assert_eq!(
                r.at(&sdrkit_proto_bcdata::publish::LabelFields::TEXT)
                    .unwrap_or(""),
                ""
            );
            assert_eq!(r.at(&PadFields::SLIDE_SEQUENCE), None);
        }
        let lock = r.at(&TimingFields::TIMING_LOCK).unwrap();
        assert!(lock <= 0.35, "lock {lock}");
        assert!(r.at(&TimingFields::TIMING_ALL_LABELS).unwrap() <= 0.5);
        let first = r.at(&TimingFields::TIMING_FIRST_ACCESS_UNIT).unwrap();
        assert!(first <= 0.7, "first unit {first}");
    }
}

#[test]
fn the_counts_do_not_depend_on_the_piece_size() {
    let Some((x, _)) = load() else { return };
    let counts = |piece| {
        let r = run(&x, 2.048e6, 1, piece);
        (
            r.count(&SyncFields::FRAMES),
            r.count(&SyncFields::FIB_OK),
            r.count(&AudioFields::AUDIO_UNITS_OK),
            r.count(&AudioFields::AUDIO_LOGICAL_FRAMES),
        )
    };
    let a = counts(4096);
    assert_eq!(a, counts(65_536));
    assert_eq!(a, counts(20_480));
}

#[test]
fn the_recorded_vector_passes_the_stack_verification() {
    let h = sdrkit_dev_test::Harness::new(&sdrkit_stack_dab::DESCRIPTOR);
    let Some(run) = h
        .run_vector("dab", "dab_ensemble_2048k")
        .expect("the receiver runs")
    else {
        return;
    };
    let dir = sdrkit_dev_test::Harness::vectors("dab");
    let expected = sdrkit_dev_test::sigmf::read_expected("dab").expect("expected.json");
    let check = sdrkit_stack_dab::verify::HOOKS
        .check_vector
        .expect("a vector check");
    let lines = check(&run.vector("dab_ensemble_2048k", &expected, &expected, &dir));
    let bad: Vec<String> = lines
        .iter()
        .filter(|l| l.outcome == sdrkit_core::verify::Outcome::Fail)
        .map(|l| format!("{}: {}", l.key, l.detail))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(lines.len() > 20);
}

#[test]
fn the_test_signal_passes_the_stack_verification() {
    let run = sdrkit_dev_test::Harness::new(&sdrkit_stack_dab::DESCRIPTOR)
        .seconds(5.0)
        .run()
        .expect("the receiver runs");
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 5.0,
        clean: true,
        snr_db: 30.0,
    };
    let cases = (sdrkit_stack_dab::verify::HOOKS.cases)(&o);
    let case = cases.iter().find(|c| c.name.contains("DAB+")).unwrap();
    let lines = (case.check.unwrap())(&run.results);
    let failed =
        |l: &&sdrkit_core::verify::CheckLine| l.outcome == sdrkit_core::verify::Outcome::Fail;
    assert!(!lines.iter().any(|l| failed(&l)), "{lines:#?}");
}
