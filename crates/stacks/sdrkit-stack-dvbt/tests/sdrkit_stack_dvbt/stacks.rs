//! The stacks as the engine sees them: descriptors, a round trip through the descriptor's generator and receiver with
//! impairments, the DVB-T2/DVB-T stack, the channel scan over generated channels, and the component registry.

use crate::common::choice;
use sdrkit_core::owner::Owner;
use sdrkit_core::SettingSet;
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, HostRequest, InputInfo, LockState, Output, Receiver, Schema,
    StackConfig, Value, Values,
};
use sdrkit_dev_test::{Awgn, FrequencyOffset};
use sdrkit_proto_mpegts::source::FileSource;
use sdrkit_stack_dvbt::bandwidth::WidthSource;
use sdrkit_stack_dvbt::generator::{DvbtGenerator, GenParams};
use sdrkit_stack_dvbt::params::*;
use sdrkit_stack_dvbt::results::Decoding;
use sdrkit_stack_dvbt::scan::{ChannelScan, ScanCommand};
use sdrkit_stack_dvbt::settings::{BandwidthChoice, GenSource, Standard};
use sdrkit_stack_dvbt::typed::DvbTerrestrialFields;
use sdrkit_stack_dvbt::typed::DvbtFields as F;
use std::sync::Arc;

/// The values of `schema` with these settings, by key.
fn stored(schema: &Arc<Schema>, settings: &[(&str, Value)]) -> Values {
    let mut v = Values::defaults(schema);
    for (k, x) in settings {
        v.set(schema.id(k).unwrap(), x).unwrap();
    }
    v
}

#[test]
fn descriptors_are_stable_and_complete() {
    let d = sdrkit_stack_dvbt::descriptors();
    let ids: Vec<&str> = d.iter().map(|x| x.id).collect();
    assert_eq!(ids, vec!["dvbt", "dvb_terrestrial", "dvbsh"]);
    for m in &d {
        assert!(m.id.chars().all(|c| c.is_ascii_lowercase() || c == '_'));
        assert_eq!(m.group, "TV");
        assert!(m.make_generator.is_some());
        assert!(m.tuning.min_rate_hz < m.tuning.rate_hz);
    }
    for m in &d[..2] {
        let s = (m.settings)();
        assert!(s.id("bandwidth").is_ok() && s.id("stream").is_ok());
    }
    let s = (sdrkit_stack_dvbt::dvbsh::DVBSH.settings)();
    assert!(s.id("width").is_ok() && s.id("low_priority").is_ok());
    let s = (sdrkit_stack_dvbt::DVB_TERRESTRIAL.settings)();
    let id = s.id("standard").unwrap();
    assert_eq!(s.setting(id).unwrap().default, choice(Standard::Auto));
    assert!(!sdrkit_stack_dvbt::benchmarks().is_empty());
}

#[test]
fn a_round_trip_through_the_descriptor() {
    let m = &sdrkit_stack_dvbt::DVBT;
    let schema = (m.settings)();
    let values = stored(
        &schema,
        &[
            ("gen_mode", choice(Mode::K8)),
            ("gen_guard", choice(Guard::G4)),
            ("gen_constellation", choice(Constellation::Qam16)),
            ("gen_rate_hp", choice(CodeRate::R12)),
            ("gen_bandwidth", choice(Bandwidth::B7)),
            ("gen_source", choice(GenSource::Pattern)),
            ("bandwidth", choice(BandwidthChoice::Auto)),
        ],
    );
    let mut g = (m.make_generator.unwrap())(&GenConfig::new(8e6, values.clone())).unwrap();
    assert_eq!(g.rate_hz(), 8e6);
    let mut x = vec![Complex32::default(); 8 * 68 * 10240];
    g.generate(&mut x);
    FrequencyOffset::new(8e6, 21_000.0).process(&mut x);
    Awgn::new(1.0, 28.0, 6.66e6, 8e6, 4).process(&mut x);
    let mut rx = (m.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(8e6, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(16384) {
        rx.feed(c, &mut out);
        out.clear();
    }
    let r = rx.results();
    let f = r.fields();
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(f.at(&F::FEC_SYNC), Some(true));
    assert_eq!(f.at(&F::CHANNEL_WIDTH), Some(7e6));
    assert_eq!(f.at(&F::FFT), Some(Mode::K8));
    assert_eq!(f.at(&F::CODE_RATE_HP), Some(CodeRate::R12));
    let off = f.get(&sdrkit_core::publish::keys::OFFSET_HZ).unwrap();
    assert!((off - 21_000.0).abs() < 60.0);
    assert!(f.at(&F::PACKETS).unwrap() > 1000);
    assert_eq!(f.at(&F::RS_FAILED), Some(0));
    assert!(
        r.decl().field_id("state").is_none(),
        "the lock_state says the state"
    );
}

/// The receiver of the descriptor, fixed to `rx_width`, on 40 frames of the descriptor's test signal of `width` at the
/// rate the stack declares.
fn test_signal_results(width: Bandwidth, rx_width: BandwidthChoice) -> sdrkit_core::Results {
    let m = &sdrkit_stack_dvbt::DVBT;
    let rate = m.tuning.rate_hz;
    let values = stored(
        &(m.settings)(),
        &[
            ("gen_bandwidth", choice(BandwidthChoice::of(width))),
            ("bandwidth", choice(rx_width)),
        ],
    );
    let mut g = (m.make_generator.unwrap())(&GenConfig::new(rate, values.clone())).unwrap();
    assert_eq!(g.rate_hz(), rate);
    // 40 frames of the default 2K 1/8 symbols, in the time of the width.
    let n = (40.0 * 68.0 * 2304.0 * rate / width.natural_rate()) as usize;
    let mut x = vec![Complex32::default(); n];
    assert_eq!(g.generate(&mut x), n);
    let mut rx = (m.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(16384) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.results().clone()
}

#[test]
fn the_test_signal_takes_its_width_from_the_settings_of_a_country() {
    let m = &sdrkit_stack_dvbt::DVBT;
    let rate = m.tuning.rate_hz;
    let made = |code: Option<&str>| {
        let width = code.map(|c| {
            crate::common::country_settings(c)
                .into_iter()
                .find(|(k, _)| k == "bandwidth")
                .unwrap()
                .1
        });
        // The country sets the width of the stack, as a host does; the test signal follows it. (This test set
        // `gen_bandwidth` before the generator read `bandwidth`.)
        let set: Vec<(&str, sdrkit_core::Value)> =
            width.map(|w| ("bandwidth", w)).into_iter().collect();
        let cfg = GenConfig::new(rate, stored(&(m.settings)(), &set));
        (m.make_generator.unwrap())(&cfg)
            .unwrap()
            .signal_info()
            .map(|i| (i.bandwidth_hz / 1e6).round())
    };
    // Australia has channels of 7 MHz, Germany of 8 MHz. Auto is 8 MHz.
    let (au, de, none) = (made(Some("AU")), made(Some("DE")), made(None));
    assert!(au < none, "{au:?} {none:?}");
    assert_eq!(de, none);
}

#[test]
fn the_test_signal_of_each_width_decodes_at_the_declared_rate() {
    for width in Bandwidth::ALL {
        let r = test_signal_results(width, BandwidthChoice::of(width));
        let f = r.fields();
        assert_eq!(r.status().lock_state, LockState::Locked, "{width:?}");
        assert_eq!(f.at(&F::FEC_SYNC), Some(true), "{width:?}");
        assert_eq!(
            f.at(&F::CHANNEL_WIDTH),
            Some(width.mhz() as f64 * 1e6),
            "{width:?}"
        );
        assert!(f.at(&F::PACKETS).unwrap() > 1000, "{width:?}");
        assert_eq!(f.at(&F::RS_FAILED), Some(0), "{width:?}");
    }
}

/// The stack of the descriptor and its test signal with only the stack setting `bandwidth` set to `width`, with the
/// Layer II decoder as the host's codec, on about a second of the signal at the rate the stack declares: the results,
/// the audio media packets, and the decoded sound with its rate and channels.
fn stack_width_run(
    width: Bandwidth,
) -> (sdrkit_core::Results, usize, Vec<f32>, Option<(f64, u16)>) {
    let m = &sdrkit_stack_dvbt::DVBT;
    let rate = m.tuning.rate_hz;
    let values = stored(
        &(m.settings)(),
        &[("bandwidth", choice(BandwidthChoice::of(width)))],
    );
    let mut g = (m.make_generator.unwrap())(&GenConfig::new(rate, values.clone())).unwrap();
    let n = (60.0 * 68.0 * 2304.0 * rate / width.natural_rate()) as usize;
    let mut x = vec![Complex32::default(); n];
    assert_eq!(g.generate(&mut x), n);
    let mut rx = (m.make_receiver)(&StackConfig::new(values)).unwrap();
    let mut codecs = sdrkit_core::codec::CodecSet::default();
    sdrkit_codec_mpa::register(&mut codecs);
    rx.set_facilities(&sdrkit_core::Facilities {
        codecs: Arc::new(codecs),
        ..Default::default()
    });
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    let mut out = Output::default();
    let (mut frames, mut sound, mut format) = (0, Vec::new(), None);
    for c in x.chunks(16384) {
        rx.feed(c, &mut out);
        frames += out
            .media
            .iter()
            .filter(|e| e.format.codec == sdrkit_core::media::Codec::Mp2)
            .count();
        sound.extend_from_slice(out.audio());
        format = format.or(out.audio_format());
        out.clear();
    }
    (rx.results().clone(), frames, sound, format)
}

/// With the width that a region gives the stack, and the test signal at its defaults, the stack locks at 6, 7 and 8 MHz,
/// and the sound of the service that plays comes out: Layer II frames, and through the host's decoder a tone of 48 kHz
/// stereo at a quarter of full scale.
#[test]
fn the_test_signal_follows_the_width_of_the_stack_and_its_service_has_sound() {
    use sdrkit_stack_dvbt::typed::DvbtMultiplexFields as M;
    for width in [Bandwidth::B6, Bandwidth::B7, Bandwidth::B8] {
        let (r, frames, sound, format) = stack_width_run(width);
        let f = r.fields();
        assert_eq!(r.status().lock_state, LockState::Locked, "{width:?}");
        assert_eq!(
            f.at(&F::CHANNEL_WIDTH),
            Some(width.mhz() as f64 * 1e6),
            "{width:?}"
        );
        assert_eq!(f.at(&F::RS_FAILED), Some(0), "{width:?}");
        assert_eq!(f.at(&M::SELECTED_SERVICE), Some(1), "{width:?}");
        assert!(frames > 10, "{width:?}: {frames} frames");
        assert_eq!(format, Some((48_000.0, 2)), "{width:?}");
        // The first frame of a decoder rises from silence; the tone is after it.
        let peak = sound[2 * 1152 * 2..]
            .iter()
            .fold(0f32, |m, s| m.max(s.abs()));
        assert!((0.2..0.3).contains(&peak), "{width:?}: peak {peak}");
    }
}

#[test]
fn a_test_signal_of_another_width_does_not_lock() {
    let r = test_signal_results(Bandwidth::B7, BandwidthChoice::B8);
    assert_ne!(r.status().lock_state, LockState::Locked);
}

#[test]
fn rate_too_low_is_refused() {
    let cfg = StackConfig::new(stored(
        &sdrkit_stack_dvbt::DvbtSetting::schema(),
        &[("bandwidth", choice(BandwidthChoice::B8))],
    ));
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&cfg).unwrap();
    let e = rx.configure(&InputInfo::new(6e6, 0.0, 0.0)).unwrap_err();
    assert!(
        matches!(e, ConfigError::RateTooLow { need, got } if (need - 7.9e6).abs() < 1.0 && got == 6e6),
        "{e:?}"
    );
}

/// The settings of two countries of the data folder, in both receiving stacks: at 226.5 MHz, Australian channel 12, the
/// receiver uses the 7 MHz of the `bandwidth` setting and stays; the German list has no channel there, so the receiver
/// uses 8 MHz and asks to tune to the lowest channel, 474 MHz.
#[test]
fn the_settings_of_a_country_set_the_width_and_the_channel() {
    let rate = 64e6 / 7.0;
    for m in [
        &sdrkit_stack_dvbt::DVBT,
        &sdrkit_stack_dvbt::DVB_TERRESTRIAL,
    ] {
        for (code, width, retune) in [("AU", 7e6, None), ("DE", 8e6, Some(474e6))] {
            let settings = crate::common::country_settings(code);
            let settings: Vec<(&str, Value)> = settings
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            let values = stored(&(m.settings)(), &settings);
            let mut rx = (m.make_receiver)(&StackConfig::new(values)).unwrap();
            rx.configure(&InputInfo::new(rate, 226.5e6, 226.5e6))
                .unwrap();
            let mut out = Output::default();
            rx.feed(&vec![Complex32::default(); 16384], &mut out);
            // An empty feed merges the results.
            rx.feed(&[], &mut out);
            let f = rx.results().fields();
            assert_eq!(f.at(&F::CHANNEL_WIDTH), Some(width), "{} {code}", m.id);
            assert_eq!(
                f.at(&F::BANDWIDTH_FROM),
                Some(WidthSource::Setting),
                "{} {code}",
                m.id
            );
            let asked: Vec<f64> = out
                .requests
                .iter()
                .filter_map(|r| match r {
                    HostRequest::Retune { tuned_hz, .. } => Some(*tuned_hz),
                    _ => None,
                })
                .collect();
            assert_eq!(
                asked,
                retune.into_iter().collect::<Vec<_>>(),
                "{} {code}",
                m.id
            );
        }
    }
}

#[test]
fn the_terrestrial_stack_decodes_dvbt_without_a_t2_receiver() {
    let m = &sdrkit_stack_dvbt::DVB_TERRESTRIAL;
    let values = stored(
        &(m.settings)(),
        &[("bandwidth", choice(BandwidthChoice::B7))],
    );
    let mut rx = (m.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(8e6, 0.0, 0.0).recording())
        .unwrap();
    let mut g = DvbtGenerator::new(GenParams::new(Bandwidth::B7, TpsParams::default()));
    let mut out = Output::default();
    for _ in 0..30 * 68 {
        let s = g.next_symbol().to_vec();
        rx.feed(&s, &mut out);
        out.clear();
    }
    let r = rx.results();
    let f = r.fields();
    assert_eq!(f.at(&DvbTerrestrialFields::STANDARD), Some(Decoding::Dvbt));
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(f.at(&F::FEC_SYNC), Some(true));
    assert!(r.log_lines().iter().any(|l| l.contains("DVB-T only")));
    assert_eq!(f.at(&F::CHANNEL_WIDTH), Some(7e6));
    assert!(f.at(&F::FFT).is_some());
    // A change of frequency reaches the DVB-T receiver and its results.
    rx.forget();
    assert_eq!(rx.results().fields().at(&F::FFT), None);
}

/// A channel of the scan: noise, or DVB-T carrying the test multiplex.
fn channel(ts: Option<&[u8]>, seconds: f64) -> Vec<Complex32> {
    let rate = 8e6;
    let n = (seconds * rate) as usize;
    let mut x = vec![Complex32::default(); n];
    if let Some(ts) = ts {
        let mut p = GenParams::new(Bandwidth::B7, TpsParams::default());
        p.hp = Box::new(FileSource::new(ts));
        let mut g = DvbtGenerator::new(p);
        use sdrkit_core::Generator;
        g.generate(&mut x);
    }
    Awgn::new(1.0, 25.0, 6.66e6, rate, 9).process(&mut x);
    x
}

#[test]
fn a_scan_finds_the_occupied_channel_and_its_services() {
    let Some(ts) = sdrkit_dev_test::recording("dvbt/mux_two_services.ts") else {
        return;
    };
    let ts = std::fs::read(ts).unwrap();
    let freqs = [522.5e6, 529.5e6];
    let mut scan = ChannelScan::list(
        freqs.iter().map(|&f| (f, Some(Bandwidth::B7))).collect(),
        600e6,
    );
    let cfg = StackConfig::new(stored(
        &sdrkit_stack_dvbt::DvbtSetting::schema(),
        &[("bandwidth", choice(BandwidthChoice::B7))],
    ));
    let mut rx = sdrkit_stack_dvbt::receiver::receiver(&cfg).unwrap();
    rx.configure(&InputInfo::new(8e6, 600e6, 600e6).recording())
        .unwrap();
    let mut out = Output::default();
    let mut now = 0.0;
    let mut cmd = scan.start(now);
    let mut guard = 0;
    while let ScanCommand::Tune { hz, .. } = cmd {
        rx.retuned(&InputInfo::new(8e6, hz, hz).recording());
        let sig = channel((hz == 529.5e6).then_some(&ts[..]), 1.2);
        let mut next = None;
        for c in sig.chunks(16384) {
            rx.feed(c, &mut out);
            out.clear();
            now += c.len() as f64 / 8e6;
            if let Some(n) = scan.on_report(now, Owner::results(&rx)) {
                next = Some(n);
                break;
            }
        }
        cmd = next.unwrap_or_else(|| scan.stop());
        guard += 1;
        assert!(guard < 10);
    }
    let found = &scan.results;
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(!found[0].locked, "{:?}", found[0]);
    assert!(found[1].locked, "{:?}", found[1]);
    assert_eq!(
        (
            found[1].mode,
            found[1].guard,
            found[1].constellation,
            found[1].code_rate
        ),
        (
            Some(Mode::K2),
            Some(Guard::G8),
            Some(Constellation::Qam16),
            Some(CodeRate::R23)
        )
    );
    assert_eq!(found[1].network_name, "Test Network");
    assert!(
        found[1]
            .services
            .iter()
            .any(|s| s.name == "Test One"
                && s.kind == Some(sdrkit_proto_mpegts::si::ServiceKind::Tv)),
        "{:?}",
        found[1].services
    );
}

#[test]
fn the_components_are_registered() {
    let mut r = sdrkit_graph::Registry::new();
    sdrkit_stack_dvbt::register(&mut r);
    let names: Vec<&str> = r.names().collect();
    for n in [
        "dvbt.input",
        "dvbt.finder",
        "dvbt.sync",
        "dvbt.fft",
        "dvbt.tps",
        "dvbt.equaliser",
        "dvbt.inner",
        "dvbt.outer",
        "dvbt.fec",
        "dvbt.report",
        "mpegts.ts_input",
        "mpegts.ts_demux",
        "mpegts.psi_si",
        "mpegts.pes_video",
        "mpegts.pes_audio",
        "mpegts.teletext",
        "mpegts.dvb_subtitle",
        "mpegts.ts_filter",
    ] {
        assert!(names.contains(&n), "{n}");
    }
    // A DVB-T front end assembled from a description.
    let desc = r#"{"components":[{"id":"front","type":"dvbt.input","settings":{"bandwidth":"7"}},{"id":"finder","type":"dvbt.finder"},{"id":"sync","type":"dvbt.sync"},{"id":"fft","type":"dvbt.fft"},{"id":"tps","type":"dvbt.tps"},{"id":"eq","type":"dvbt.equaliser"},{"id":"inner","type":"dvbt.inner"},{"id":"outer","type":"dvbt.outer"}],
        "edges":[["input","front.in"],["front.out","finder.in"],["finder.out","sync.in"],["sync.out","fft.in"],["fft.out","tps.in"],["tps.out","eq.in"],["eq.out","inner.in"],["inner.out","outer.in"]]}"#;
    let g = sdrkit_graph::Graph::from_json(desc, &r);
    assert!(g.is_ok(), "{:?}", g.err());
}

/// The generator declares its signal (occupied band, cell rate, net bit rate) and makes the mean power it declares.
#[test]
fn the_generator_declares_its_signal() {
    use sdrkit_core::Generator;
    for mode in [Mode::K2, Mode::K8] {
        let tps = TpsParams {
            mode,
            constellation: Constellation::Qam64,
            ..Default::default()
        };
        let mut g = DvbtGenerator::new(GenParams::new(Bandwidth::B8, tps));
        let info = g.signal_info().expect("declared");
        assert!(
            (info.bandwidth_hz - 7.61e6).abs() < 0.01e6,
            "{}",
            info.bandwidth_hz
        );
        let mut x = vec![Complex32::default(); 400_000];
        g.generate(&mut x);
        let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
        eprintln!("{mode:?}: mean power {p}");
        assert!((p / info.power - 1.0).abs() < 0.01, "mean power {p}");
    }
}
