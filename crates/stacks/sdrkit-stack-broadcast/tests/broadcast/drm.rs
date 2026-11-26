//! DRM: generator -> impairments -> receiver round trips, and the acceptance cases.

use crate::support::{
    cell_num, cell_text, flag, impair, int, invoke, is_none, num, row_ids, snr, stack_config, text,
    Set,
};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, InputInfo, LockState, Output, Results, SettingSet};
use sdrkit_stack_broadcast::drm::{
    self,
    params::{Qam, Robustness},
    results as res,
    tx::{audio_frame_exact, AudioKind, DataConfig, PacketFaults, TxConfig},
    DrmGenerator,
};
use serde_json::json;

const DIAL: f64 = 9.5e6;

#[derive(Debug, Default)]
struct Run {
    r: Results,
    exact: usize,
    bad_frames: usize,
    seq_ok: bool,
    events: Vec<sdrkit_core::Event>,
}

impl Run {
    fn locked(&self) -> bool {
        self.r.status().lock_state == LockState::Locked
    }
    fn n(&self, key: &str) -> f64 {
        num(&self.r, key)
    }
    fn t(&self, key: &str) -> String {
        text(&self.r, key)
    }
    fn service(&self, id: &str, key: &str) -> String {
        cell_text(&self.r, &res::SERVICES, id, key)
    }
}

/// The clean signal through the framework's impairments (30 dB in the channel unless `extra` says otherwise).
fn generate(cfg: TxConfig, rate: f64, seconds: f64, extra: &[Set]) -> Vec<Complex32> {
    let g = DrmGenerator::new(cfg, rate).expect("valid configuration");
    let mut all: Vec<Set> = snr(30.0).to_vec();
    all.push(("gen_seed", json!(5)));
    all.extend(extra.iter().cloned());
    impair(Box::new(g), seconds, &all)
}

fn receive(x: &[Complex32], rate: f64, piece: usize, kind: AudioKind, seed: u64) -> Run {
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    let mut run = Run {
        seq_ok: true,
        ..Default::default()
    };
    let mut last_t = -1.0;
    for c in x.chunks(piece) {
        rx.feed(c, &mut out);
        for m in &out.media {
            if audio_frame_exact(m.packet.bytes(), kind, seed) {
                run.exact += 1;
            } else {
                run.bad_frames += 1;
            }
        }
        run.events.append(&mut out.events);
        out.clear();
        let t = num(rx.results(), "signal_time");
        if t < last_t {
            run.seq_ok = false;
        }
        if t.is_finite() {
            last_t = t;
        }
    }
    run.r.clone_from(rx.results());
    run
}

fn usual(mode: Robustness) -> TxConfig {
    TxConfig::usual(mode)
}

#[test]
fn r1_round_trip_mode_b_30_db() {
    let rate = 192e3;
    let x = generate(usual(Robustness::B), rate, 14.0, &[]);
    let r = receive(&x, rate, 50_000, AudioKind::Melody, 1);
    assert!(r.locked(), "{:#?}", r.r.fields());
    assert_eq!(r.t("robustness_mode"), "B");
    assert_eq!(int(&r.r, "occupancy"), 3);
    assert_eq!(r.t("msc"), "64-QAM");
    assert_eq!(r.t("sdc"), "16-QAM");
    assert!(r.t("interleaver").starts_with("long"));
    assert_eq!(r.service("0", "label"), "Test DRM");
    assert_eq!(r.service("0", "service_id"), "E12345");
    assert!(r.service("0", "language").starts_with("5 English"));
    assert!(r.service("0", "language").contains("eng"));
    assert_eq!(r.service("0", "country"), "ae");
    assert_eq!(r.service("0", "programme"), "10 Pop Music");
    assert_eq!(r.service("0", "codec"), "AAC, 24 kHz, stereo");
    assert_eq!(r.t(res::DrmFields::PLAYING_SERVICE.key), "0");
    assert!(r.t("time").contains("2026-10-06 12:0"), "{}", r.t("time"));
    assert_eq!(r.t("text"), "DRM test signal");
    assert_eq!(int(&r.r, "fac_bad"), 0);
    assert_eq!(int(&r.r, "sdc_bad"), 0);
    assert!(r.exact >= 8, "exact {}", r.exact);
    assert_eq!(r.bad_frames, 0);
    assert_eq!(int(&r.r, "audio_bad"), 0);
    assert!(r.seq_ok);
    assert!((r.n("snr_db") - 30.0).abs() < 4.0, "snr {}", r.n("snr_db"));
    assert!(r.n("offset_hz").abs() < 2.0);
    assert!(r.n("clock_ppm").abs() < 5.0);
    assert!(r.n("quality") > 0.8, "{}", r.n("quality"));
    assert!(r.n("first_audio") < 12.0);
    assert!(
        r.t("protection").starts_with("1 ("),
        "{}",
        r.t("protection")
    );
    assert!(r.events.iter().any(|e| e.is(&res::TEXT_CHANGED)));
}

#[test]
fn r2_every_mode_and_occupancy() {
    for mode in [Robustness::A, Robustness::B, Robustness::C, Robustness::D] {
        for occ in 0..6u8 {
            if mode.range(occ).is_none() {
                continue;
            }
            for q in [Qam::Q64, Qam::Q16] {
                // Keep the run short: the wide occupancies and both constellations of mode B, the usual one elsewhere.
                if mode != Robustness::B && (occ != mode.usual_occupancy() && occ != 5) {
                    continue;
                }
                let mut cfg = usual(mode);
                cfg.occupancy = occ;
                cfg.qam = q;
                cfg.protection = drm::tx::usual_protection(mode, q);
                cfg.short_interleaver = true;
                let x = generate(cfg, 48e3, 7.0, &[]);
                let r = receive(&x, 48e3, 30_000, AudioKind::Melody, 1);
                assert!(r.locked(), "{mode:?} {occ} {q:?}");
                assert_eq!(r.t("robustness_mode"), mode.letter());
                assert_eq!(int(&r.r, "occupancy"), occ as i64);
                assert_eq!(r.t("msc"), q.name());
                assert_eq!(r.service("0", "label"), "Test DRM");
                assert!(r.exact >= 6, "{mode:?} {occ} {q:?}: {}", r.exact);
                assert_eq!(r.bad_frames, 0, "{mode:?} {occ} {q:?}");
                assert_eq!(r.t("text"), "DRM test signal");
            }
        }
    }
}

#[test]
fn r3_drm_plus_with_offsets() {
    for (q, short, cfo) in [(Qam::Q16, false, 1500.0), (Qam::Q4, true, -3000.0)] {
        let mut cfg = usual(Robustness::E);
        cfg.qam = q;
        cfg.protection = drm::tx::usual_protection(Robustness::E, q);
        cfg.short_interleaver = short;
        let x = generate(cfg, 192e3, 5.0, &[("gen_offset_hz", json!(cfo))]);
        let r = receive(&x, 192e3, 40_000, AudioKind::Melody, 1);
        assert!(r.locked(), "{q:?} {:#?}", r.r.fields());
        assert_eq!(r.t("robustness_mode"), "E");
        assert_eq!(int(&r.r, "occupancy"), 0);
        assert_eq!(r.n("bandwidth"), 100e3);
        assert_eq!(r.t("sdc"), "4-QAM");
        assert_eq!(r.service("0", "label"), "Test DRM+");
        assert!(cell_num(&r.r, &res::SERVICES, "0", "bit_rate_bps") > 30e3);
        assert_eq!(r.t("text"), "DRM test signal");
        assert!(r.t("time").contains("2026-10-06"));
        assert_eq!(int(&r.r, "fac_bad"), 0);
        assert_eq!(int(&r.r, "sdc_bad"), 0);
        assert!(int(&r.r, "audio_ok") >= 40, "{}", int(&r.r, "audio_ok"));
        assert_eq!(int(&r.r, "audio_bad"), 0);
        assert!((r.n("offset_hz") - cfo).abs() < 5.0);
        assert!(r.n("first_audio") < 4.0, "{}", r.n("first_audio"));
    }
    assert!(drm::DrmSetting::schema().id("gen_robustness").is_ok());
    assert!(sdrkit_stack_broadcast::DRM.tuning.max_hz >= 230e6);
    assert!(sdrkit_stack_broadcast::DRM.tuning.min_hz <= 0.15e6);
}

#[test]
fn o1_carrier_offsets() {
    for cfo in [-10000.0, -2500.0, -123.4, 7.3, 1000.0, 12000.0] {
        let mut g = snr(28.0).to_vec();
        g.push(("gen_offset_hz", json!(cfo)));
        let x = generate(usual(Robustness::B), 48e3, 9.0, &g);
        let r = receive(&x, 48e3, 20_000, AudioKind::Melody, 1);
        assert!(r.locked(), "cfo {cfo}");
        assert!(
            (r.n("offset_hz") - cfo).abs() < 3.0,
            "cfo {cfo}: {}",
            r.n("offset_hz")
        );
        assert_eq!(r.bad_frames, 0);
        assert!(r.exact > 0);
    }
}

#[test]
fn o2_clock_offsets_through_impairments() {
    for ppm in [-100.0, 30.0, 100.0] {
        let x = generate(
            usual(Robustness::B),
            48e3,
            14.0,
            &[
                ("gen_clock_ppm", json!(ppm)),
                ("gen_offset_hz", json!(25.0)),
            ],
        );
        let r = receive(&x, 48e3, 24_000, AudioKind::Melody, 1);
        assert!(r.locked(), "ppm {ppm}");
        assert_eq!(r.bad_frames, 0, "ppm {ppm}");
        assert!(r.exact > 10, "ppm {ppm}: {}", r.exact);
    }
}

#[test]
fn chain_at_250_ksps_with_noise() {
    let x = generate(usual(Robustness::B), 250e3, 9.0, &snr(25.0));
    let r = receive(&x, 250e3, 65_536, AudioKind::Melody, 1);
    assert!(r.locked());
    assert_eq!(r.bad_frames, 0);
    assert!(r.exact > 0);
}

#[test]
fn not_aac_frames_are_counted_as_damaged() {
    let mut cfg = usual(Robustness::B);
    cfg.audio = AudioKind::NotAac;
    cfg.short_interleaver = true;
    let x = generate(cfg, 48e3, 6.0, &[]);
    let r = receive(&x, 48e3, 48_000, AudioKind::NotAac, 1);
    assert!(r.locked());
    assert!(int(&r.r, "audio_bad") > 0);
    assert_eq!(r.exact, 0);
}

/// A service that the SDC names as xHE-AAC: the multiplex decodes, the service is listed with its label, no frame goes to
/// the host as AAC and none is counted, and the status says that the audio is not decoded.
#[test]
fn xhe_aac_audio_is_named_and_not_decoded() {
    let mut cfg = usual(Robustness::B);
    cfg.xhe = true;
    cfg.short_interleaver = true;
    let x = generate(cfg, 48e3, 8.0, &[]);
    let r = receive(&x, 48e3, 48_000, AudioKind::Melody, 1);
    assert!(r.locked());
    assert!(int(&r.r, "mux_ok") > 5);
    assert_eq!(r.exact + r.bad_frames, 0);
    assert_eq!(int(&r.r, "audio_ok") + int(&r.r, "audio_bad"), 0);
    assert_eq!(r.t("audio_state"), "no_decoder");
    assert!(r.t("audio_info").starts_with("xHE-AAC, 24 kHz stereo"));
    assert!(r.r.status().summary.ends_with(drm::rx::XHE_NOT_DECODED));
    assert!(!r.service("0", "label").is_empty());
}

/// A 10 kHz channel in the middle of 250 ksps with a station 12 dB stronger 80 kHz away (a carrier with noise sidebands
/// 4 kHz wide): the receiver stays on the DRM channel and decodes it.
#[test]
fn a_stronger_am_station_beside_the_channel_is_not_taken() {
    let rate = 250e3;
    let mut x = generate(usual(Robustness::B), rate, 12.0, &[]);
    let p = x.iter().map(|v| v.norm_sqr()).sum::<f32>() / x.len() as f32;
    // The station: a carrier and audio that is noise through a one-pole low-pass of about 2 kHz.
    let amp = (16.0 * p).sqrt();
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let mut audio = 0.0f32;
    for (i, v) in x.iter_mut().enumerate() {
        audio += 0.05 * (rng.normal_f32() - audio);
        let ph = (std::f64::consts::TAU * 80e3 * i as f64 / rate) as f32;
        *v += Complex32::new(ph.cos(), ph.sin()) * amp * (0.7 + 1.5 * audio);
    }
    let r = receive(&x, rate, 25_000, AudioKind::Melody, 1);
    assert!(r.locked());
    assert!(r.n("offset_hz").abs() < 5.0, "{}", r.n("offset_hz"));
    assert!(
        r.exact >= 200 && r.bad_frames == 0,
        "{} {}",
        r.exact,
        r.bad_frames
    );
}

/// The spectrum search of the channel centring: a filled 10 kHz block is found at its middle and ranks first, a stronger
/// carrier with weak sidebands is no block, and a block wider than a DRM channel is left out.
#[test]
fn centring_finds_filled_blocks_only() {
    let bin = 500.0;
    let mut p = vec![1.0f32; 400];
    let at = |hz: f64| (200.0 + hz / bin) as usize;
    for v in &mut p[at(30e3)..at(40e3)] {
        *v = 50.0;
    }
    // A carrier 30 dB over its sidebands.
    for v in &mut p[at(-64e3)..at(-56e3)] {
        *v = 20.0;
    }
    p[at(-60e3)] = 20_000.0;
    // 40 kHz of something else.
    for v in &mut p[at(-40e3)..at(0.0)] {
        *v = 100.0;
    }
    let b = drm::rx::filled_blocks(&p, bin);
    assert_eq!(b.len(), 1, "{b:?}");
    assert!(
        (b[0].centre_hz - 35e3).abs() <= bin && b[0].rank > 40.0,
        "{b:?}"
    );
}

#[test]
fn data_service_slide_show() {
    let mut cfg = usual(Robustness::B);
    cfg.short_interleaver = true;
    cfg.data = Some(DataConfig::default());
    let x = generate(cfg, 48e3, 30.0, &[]);
    let r = receive(&x, 48e3, 48_000, AudioKind::Melody, 1);
    assert!(r.locked());
    assert_eq!(r.service("1", "label"), "Test Data");
    assert_eq!(r.service("1", "kind"), "data");
    assert_eq!(r.service("1", "application"), "MOT slide show");
    assert!(r
        .service("1", "mode_text")
        .starts_with("packet mode, data units, packet id 1"));
    assert_eq!(r.service("1", "decoded"), "yes");
    assert_eq!(r.service("0", "kind"), "audio");
    let good = int(&r.r, "audio_ok");
    let bad = int(&r.r, "audio_bad");
    assert!(good >= 3 * bad.max(1) || bad == 0);
    let s = |k: &str| cell_text(&r.r, &res::SLIDES, "1", k);
    assert_eq!(s("width"), "32");
    assert_eq!(s("height"), "24");
    assert_eq!(s("content_type"), "image/PNG");
    assert!(cell_num(&r.r, &res::SLIDES, "1", "size") > 100.0);
    assert!(s("click_url").starts_with("http://example.org/"));
    let names: std::collections::BTreeSet<String> = r
        .events
        .iter()
        .filter(|e| e.is(&res::OBJECT_RECEIVED))
        .filter_map(|e| e.get(&keys::TEXT).map(str::to_string))
        .collect();
    assert_eq!(names.len(), 3, "{names:?}");
    let triggers: Vec<String> = row_ids(&r.r, "objects")
        .iter()
        .map(|id| cell_text(&r.r, &res::OBJECTS, id, "name"))
        .collect();
    assert_eq!(triggers.len(), 3, "{triggers:?}");
}

#[test]
fn data_service_with_faults_completes() {
    let mut cfg = usual(Robustness::B);
    cfg.short_interleaver = true;
    cfg.data = Some(DataConfig {
        faults: PacketFaults {
            drop_every: Some(17),
            bad_crc_every: Some(23),
            foreign_every: Some(29),
        },
        ..Default::default()
    });
    let x = generate(cfg, 48e3, 40.0, &[]);
    let r = receive(&x, 48e3, 48_000, AudioKind::Melody, 1);
    let c = |k: &str| cell_num(&r.r, &res::PACKET_COUNTERS, "1", k);
    assert!(c("lost") > 0.0, "{:?}", row_ids(&r.r, "packets"));
    assert!(c("bad_crc") > 0.0);
    assert!(c("other_id") > 0.0);
    assert_eq!(c("empty"), 0.0);
    assert!(c("units_dropped") > 0.0);
    let objs: std::collections::BTreeSet<String> = r
        .events
        .iter()
        .filter(|e| e.is(&res::OBJECT_RECEIVED))
        .filter_map(|e| e.get(&keys::TEXT).map(str::to_string))
        .collect();
    assert_eq!(objs.len(), 3, "{objs:?}");
}

#[test]
fn journaline_and_website() {
    for (app, check) in [
        (0x44Au16, "journaline"),
        (0x003, "website"),
        (0x007, "epg"),
        (0x004, "tpeg"),
    ] {
        let mut cfg = usual(Robustness::B);
        cfg.short_interleaver = true;
        cfg.data = Some(DataConfig {
            app_type: app,
            ..Default::default()
        });
        let x = generate(cfg, 48e3, 12.0, &[]);
        let r = receive(&x, 48e3, 48_000, AudioKind::Melody, 1);
        match check {
            "journaline" => {
                assert_eq!(cell_text(&r.r, &res::PAGES, "0", "title"), "Main menu");
                assert!(cell_text(&r.r, &res::PAGES, "0", "content").contains("1 News"));
                assert_eq!(cell_text(&r.r, &res::PAGES, "2", "kind"), "list");
            }
            "website" => assert!(r.events.iter().any(|e| e.is(&res::OBJECT_RECEIVED)
                && e.at(&res::DrmObjectEvent::CONTENT_TYPE) == Some("text/HTML"))),
            "epg" => assert_eq!(r.service("1", "application"), "EPG"),
            _ => assert!(
                cell_num(&r.r, &res::PACKET_COUNTERS, "1", "tpeg_frames") > 0.0,
                "{:?}",
                row_ids(&r.r, "packets")
            ),
        }
    }
}

#[test]
fn cut_independence() {
    let mut cfg = usual(Robustness::B);
    cfg.short_interleaver = true;
    let x = generate(cfg, 48e3, 5.0, &[]);
    let a = receive(&x, 48e3, 1_000, AudioKind::Melody, 1);
    let b = receive(&x, 48e3, 33_333, AudioKind::Melody, 1);
    assert_eq!(a.exact, b.exact);
    assert_eq!(int(&a.r, "audio_ok"), int(&b.r, "audio_ok"));
    assert_eq!(a.n("offset_hz"), b.n("offset_hz"));
}

#[test]
fn retune_keeps_counters_and_locks_again() {
    let mut cfg = usual(Robustness::B);
    cfg.short_interleaver = true;
    let x = generate(cfg, 48e3, 10.0, &[]);
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    let info = InputInfo::new(48e3, DIAL, DIAL).recording();
    rx.configure(&info).unwrap();
    let mut out = Output::default();
    for c in x[..x.len() / 2].chunks(24_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let before = int(rx.results(), "fac_ok");
    rx.retuned(&info);
    for c in x[x.len() / 2..].chunks(24_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert!(int(rx.results(), "fac_ok") > before);
    assert_eq!(rx.results().status().lock_state, LockState::Locked);
}

#[test]
fn service_setting_and_rate_refusal() {
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    assert!(invoke(rx.as_mut(), "service.select", &[("row", json!("0"))]).is_ok());
    assert!(invoke(rx.as_mut(), "service.select", &[("row", json!("9"))]).is_err());
    assert!(rx.configure(&InputInfo::new(24e3, DIAL, DIAL)).is_err());
}

/// O12 (part): a channel off the middle of the sample band at 250 ksps, found by the channel centring (tuned to the middle) and
/// reached by tuning (tuned to the channel). The centring must not move a channel already in the middle of its lane again.
#[test]
fn o12_off_centre_channel_at_250_ksps() {
    let rate = 250e3;
    let off = 81.5e3;
    let x = generate(
        usual(Robustness::B),
        rate,
        16.0,
        &[("gen_offset_hz", json!(off))],
    );
    for tuned in [DIAL, DIAL + off] {
        let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
        rx.configure(&InputInfo::new(rate, DIAL, tuned).recording())
            .unwrap();
        let mut out = Output::default();
        for c in x.chunks(25_000) {
            rx.feed(c, &mut out);
            out.clear();
        }
        let r = rx.results();
        assert_eq!(text(r, "robustness_mode"), "B", "{:?}", r.fields());
        let good = int(r, "audio_ok");
        assert!(
            good >= 200 && int(r, "audio_bad") == 0,
            "tuned {tuned}: {good}"
        );
        assert!(!flag(r, "mirrored"));
    }
}

/// A change of frequency forgets the multiplex: the services, the text, the counts and the measurements.
#[test]
fn a_change_of_frequency_forgets_the_multiplex() {
    let rate = 192e3;
    let x = generate(usual(Robustness::B), rate, 14.0, &[]);
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(50_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    assert!(!row_ids(rx.results(), res::SERVICES.key).is_empty());
    assert_eq!(text(rx.results(), "text"), "DRM test signal");
    let fac_ok = int(rx.results(), "fac_ok");
    rx.forget();
    let r = rx.results();
    assert!(row_ids(r, res::SERVICES.key).is_empty());
    assert!(row_ids(r, res::STREAMS.key).is_empty());
    for key in [
        "robustness_mode",
        "msc",
        "snr_db",
        "offset_hz",
        "quality",
        "time",
        "text",
        "playing_service",
    ] {
        assert!(is_none(r, key), "{key}");
    }
    assert_eq!(int(r, "fac_ok"), 0);
    // The counts start from zero: the same signal again counts as much as the first time, not twice as much.
    for c in x.chunks(50_000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = rx.results();
    let again = int(r, "fac_ok");
    assert!(again > 0 && again <= fac_ok + 2, "{again} after {fac_ok}");
    assert!(!row_ids(r, res::SERVICES.key).is_empty());
}

#[test]
fn the_test_signal_plays_its_melody_through_the_aac_decoder() {
    let rate = 48e3;
    let x = generate(usual(Robustness::B), rate, 10.0, &[]);
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    let mut codecs = sdrkit_core::codec::CodecSet::default();
    sdrkit_codec_aac::register(&mut codecs);
    rx.as_any_mut()
        .and_then(|a| a.downcast_mut::<sdrkit_graph::Graph>())
        .expect("a graph")
        .set_codecs(codecs);
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    let (mut sound, mut fs, mut ch) = (Vec::new(), 0.0, 0);
    for c in x.chunks(48_000) {
        rx.feed(c, &mut out);
        for p in out.audio_runs() {
            sound.extend_from_slice(p.samples);
            fs = p.rate_hz;
            ch = p.channels;
        }
        out.clear();
    }
    assert_eq!((fs, ch), (24e3, 2));
    // At least 3 s of sound, and its last second is loud: the notes of the melody at 0.4 of full scale.
    assert!(sound.len() as f64 > 3.0 * fs * 2.0, "{} samples", sound.len());
    let tail = &sound[sound.len() - 48_000..];
    let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
    assert!(rms > 0.1, "rms {rms}");
    // The left channel crosses zero at the pitch of the notes, 523 to 1047 Hz.
    let left: Vec<f32> = tail.iter().step_by(2).copied().collect();
    let crossings = left.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    assert!((500..=1100).contains(&crossings), "{crossings} Hz");
}

/// The raw 8-bit recording `name` in the folder that the variable `SDRKIT_LIVE_RECORDINGS` names, or None. The recordings
/// of real signals are kept outside the repository; without the variable the test is skipped.
fn live_recording(name: &str) -> Option<Vec<Complex32>> {
    let Ok(folder) = std::env::var("SDRKIT_LIVE_RECORDINGS") else {
        eprintln!("skipped: SDRKIT_LIVE_RECORDINGS is not set");
        return None;
    };
    let p = std::path::PathBuf::from(folder).join(name);
    let Ok(bytes) = std::fs::read(&p) else {
        eprintln!("skipped: {name} is not here");
        return None;
    };
    Some(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| Complex32::new(b[0] as i8 as f32 / 128.0, b[1] as i8 as f32 / 128.0))
            .collect(),
    )
}

/// A recording at 2 Msps with a 10 kHz DRM channel in the middle and a stronger AM station 175 kHz away: the receiver stays
/// on the DRM channel, decodes the FAC and the SDC, lists the service and says that its xHE-AAC audio is not decoded.
#[test]
#[ignore = "needs the recordings of real signals"]
fn live_drm_beside_an_am_station() {
    let Some(x) = live_recording("drm_xheaac_2M.cs8") else {
        return;
    };
    let rate = 2e6;
    let mut rx = drm::make_receiver(&stack_config(drm::DrmSetting::schema(), &[])).unwrap();
    rx.configure(&InputInfo::new(rate, DIAL, DIAL).recording())
        .unwrap();
    let mut out = Output::default();
    let mut media = 0usize;
    for (i, c) in x.chunks(200_000).enumerate() {
        rx.feed(c, &mut out);
        media += out.media.len();
        out.clear();
        if i % 50 == 49 {
            let r = rx.results();
            eprintln!(
                "{:5.1} s  {:?}  mode {}  snr {:.1}  offset {:.1}  fac {}/{}  sdc {}/{}  mux {}/{}  audio {}/{}  {}",
                (i + 1) as f64 * 0.1,
                r.status().lock_state,
                text(r, "robustness_mode"),
                num(r, "snr_db"),
                num(r, "offset_hz"),
                int(r, "fac_ok"),
                int(r, "fac_bad"),
                int(r, "sdc_ok"),
                int(r, "sdc_bad"),
                int(r, "mux_ok"),
                int(r, "mux_bad"),
                int(r, "audio_ok"),
                int(r, "audio_bad"),
                r.status().summary,
            );
        }
    }
    let r = rx.results();
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert!(num(r, "offset_hz").abs() < 500.0);
    let (ok, bad) = (int(r, "fac_ok"), int(r, "fac_bad"));
    assert!(ok >= 100 && bad * 20 <= ok, "FAC {ok} good, {bad} bad");
    assert!(int(r, "sdc_ok") >= 30);
    assert_eq!(row_ids(r, "services").len(), 1);
    assert!(r.status().summary.contains("xHE-AAC audio is not decoded"));
    assert_eq!(media, 0);
}
