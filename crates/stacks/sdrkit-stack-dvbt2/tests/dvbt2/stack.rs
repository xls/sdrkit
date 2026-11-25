//! The stack through the framework: the generator with its settings, the impairments of the harness, the receiver graph with
//! the transport stream layer, the declared results and events.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::{Setting, SettingError, SettingSet, StackConfig, Value, Values};
use sdrkit_dev_test::Harness;
use sdrkit_proto_mpegts::publish::SERVICE_LIST_EVENT;
use sdrkit_stack_dvbt2::params::{Bandwidth, FecFrame, Fft, Guard, PilotPattern, Profile};
use sdrkit_stack_dvbt2::results::Detection;
use sdrkit_stack_dvbt2::settings::{
    gen_settings, PlpModulation, PlpRate, BANDWIDTH, GEN_BLOCKS, GEN_FEC_FRAME, GEN_FFT, GEN_GUARD,
    GEN_MISO, GEN_MODULATION, GEN_PILOTS, GEN_PROFILE, GEN_RATE,
};
use sdrkit_stack_dvbt2::typed::Dvbt2Fields;
use sdrkit_stack_dvbt2::{values_of_config, Dvbt2Setting, DVBT2};

fn small(h: Harness) -> Harness {
    h.set(&GEN_FFT, Fft::K2)
        .set(&GEN_GUARD, Guard::G1_8)
        .set(&GEN_PILOTS, PilotPattern::Pp3)
        .set(&GEN_FEC_FRAME, FecFrame::Short)
        .set(&GEN_MODULATION, PlpModulation::Qam16)
        .set(&GEN_RATE, PlpRate::R1_2)
        .set(&GEN_BLOCKS, 4)
        .seconds(0.6)
}

#[test]
fn generator_to_receiver_through_the_framework() {
    let run = small(Harness::new(&DVBT2)).run().expect("run");
    let r = run.results.fields();
    assert_eq!(
        r.at(&Dvbt2Fields::DETECTION),
        Some(Detection::Flowing),
        "{:?}",
        run.results.fields()
    );
    assert_eq!(r.at(&Dvbt2Fields::FFT), Some(Fft::K2));
    assert_eq!(r.at(&Dvbt2Fields::PILOT_PATTERN), Some(PilotPattern::Pp3));
    let ok = r.get(&keys::FRAMES_OK).unwrap_or(0);
    let bad = r.get(&keys::FRAMES_BAD).unwrap_or(-1);
    assert!(ok > 10 && bad == 0, "{:?}", run.results.fields());
    // Every event emitted is declared, and the service list of the transport stream layer is one of them.
    let decl = sdrkit_stack_dvbt2::typed::shape("dvbt2").results_decl();
    for e in &run.events {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "{} is not declared",
            e.kind()
        );
    }
    assert!(
        run.events.iter().any(|e| e.is(&SERVICE_LIST_EVENT)),
        "no service list"
    );
}

#[test]
fn another_input_rate_and_noise() {
    let run = small(Harness::new(&DVBT2))
        .rate(10e6)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 18.0)
        .set(&GEN_OFFSET_HZ, 25_000.0)
        .run()
        .expect("run");
    let r = run.results.fields();
    assert_eq!(
        r.at(&Dvbt2Fields::DETECTION),
        Some(Detection::Flowing),
        "{:?}",
        run.results.fields()
    );
    let off = r.get(&keys::OFFSET_HZ).unwrap_or(0.0);
    assert!((off - 25_000.0).abs() < 300.0, "{off}");
}

#[test]
fn a_bad_combination_of_settings_is_refused() {
    let v = Harness::new(&DVBT2)
        .set(&GEN_FFT, Fft::K32)
        .set(&GEN_GUARD, Guard::G1_4)
        .values()
        .expect("values");
    assert!(matches!(
        gen_settings(&v),
        Err(SettingError::Conflict { .. })
    ));
}

#[test]
fn the_test_signal_in_t2_lite_and_in_miso_through_the_framework() {
    for (profile, miso) in [(Profile::Lite, false), (Profile::Base, true), (Profile::Lite, true)] {
        // 2K with guard 1/8 takes PP1 in MISO (tables 59 and I.6).
        let pp = if miso { PilotPattern::Pp1 } else { PilotPattern::Pp3 };
        let run = small(Harness::new(&DVBT2))
            .set(&GEN_PROFILE, profile)
            .set(&GEN_MISO, miso)
            .set(&GEN_PILOTS, pp)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 20.0)
            .run()
            .expect("run");
        let r = run.results.fields();
        assert_eq!(r.at(&Dvbt2Fields::DETECTION), Some(Detection::Flowing), "{r:?}");
        assert_eq!(r.at(&Dvbt2Fields::PROFILE), Some(profile));
        assert_eq!(r.at(&Dvbt2Fields::MISO), Some(miso));
        let ok = r.get(&keys::FRAMES_OK).unwrap_or(0);
        let bad = r.get(&keys::FRAMES_BAD).unwrap_or(-1);
        assert!(ok > 10 && bad == 0, "{profile:?} MISO {miso}: {r:?}");
    }
    // T2-Lite never sends 256-QAM with 2/3 (table I.4).
    let v = Harness::new(&DVBT2)
        .set(&GEN_PROFILE, Profile::Lite)
        .set(&GEN_MODULATION, PlpModulation::Qam256)
        .set(&GEN_RATE, PlpRate::R2_3)
        .values()
        .expect("values");
    assert!(matches!(
        gen_settings(&v),
        Err(SettingError::Conflict { .. })
    ));
}

const OTHER_BANDWIDTH: Setting<field::Text> = Setting::new("bandwidth", "Width", "", "7");
const OTHER_PLP: Setting<field::I64> = Setting::new("plp", "PLP", "", 3).range(-1.0, 255.0, 1.0);

sdrkit_core::settings! {
    enum OtherSetting {
        Bandwidth = OTHER_BANDWIDTH,
        Plp = OTHER_PLP,
    }
}

#[test]
fn settings_of_another_stack_are_taken_by_key() {
    let mut own = Values::defaults(&Dvbt2Setting::schema());
    let seven = Value::Choice(sdrkit_core::Choice::index(Bandwidth::B7));
    own.set(Dvbt2Setting::Bandwidth.id(), &seven)
        .expect("a choice");
    assert_eq!(
        values_of_config(&StackConfig::new(own)).get(&BANDWIDTH),
        Bandwidth::B7
    );
    // A text where a choice is declared keeps the default. A number of the same kind is taken.
    let v = values_of_config(&StackConfig::new(Values::defaults(&OtherSetting::schema())));
    assert_eq!(v.get(&BANDWIDTH), Bandwidth::B8);
    assert_eq!(v.get(&sdrkit_stack_dvbt2::settings::PLP), 3);
}

#[test]
fn a_7_mhz_channel_at_the_8_mhz_rate() {
    let run = small(Harness::new(&DVBT2))
        .set(&BANDWIDTH, Bandwidth::B7)
        .rate(64e6 / 7.0)
        .run()
        .expect("run");
    let r = run.results.fields();
    assert_eq!(
        r.at(&Dvbt2Fields::DETECTION),
        Some(Detection::Flowing),
        "{:?}",
        run.results.fields()
    );
}

/// The rows of table `key` of `r`.
fn rows(r: &sdrkit_core::Results, key: &str) -> usize {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// A change of frequency keeps the guide and forgets the PLPs, the services and every measurement of the multiplex
/// before.
#[test]
fn a_change_of_frequency_keeps_the_guide_and_forgets_the_multiplex() {
    use sdrkit_core::owner::Owner;
    use sdrkit_core::{Complex32, GenConfig, Generator, InputInfo, Output, Receiver};
    assert_eq!(
        sdrkit_stack_dvbt2::typed::shape("dvbt2").kept_keys(),
        Ok(vec![
            "table guide".to_owned(),
            "table guide_ratings".to_owned()
        ])
    );
    let schema = Dvbt2Setting::schema();
    let rate = Bandwidth::B8.rate();
    let mut g = sdrkit_stack_dvbt2::T2Generator::from_config(&GenConfig::new(
        rate,
        Values::defaults(&schema),
    ));
    let mut x = vec![Complex32::default(); (1.5 * rate) as usize];
    g.generate(&mut x);
    let mut rx =
        sdrkit_stack_dvbt2::receiver(&StackConfig::new(Values::defaults(&schema))).expect("builds");
    rx.configure(&InputInfo::new(rate, 474e6, 474e6).recording())
        .expect("rate");
    let mut out = Output::default();
    for c in x.chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    let before = Owner::results(&rx).clone();
    rx.forget();
    let after = Owner::results(&rx);
    for t in ["guide", "guide_ratings"] {
        assert_eq!(rows(after, t), rows(&before, t), "{t}");
    }
    for t in ["plps", "services"] {
        assert!(rows(&before, t) > 0, "{t}");
        assert_eq!(rows(after, t), 0, "{t}");
    }
    let (b, a) = (before.fields(), after.fields());
    assert_eq!(b.at(&Dvbt2Fields::DETECTION), Some(Detection::Flowing));
    assert_eq!(a.at(&Dvbt2Fields::DETECTION), Some(Detection::Searching));
    assert!(b.at(&Dvbt2Fields::FFT).is_some());
    assert_eq!(a.at(&Dvbt2Fields::FFT), None);
    assert!(b.at(&Dvbt2Fields::FRAMES_OK).is_some_and(|n| n > 0));
    assert_eq!(a.at(&Dvbt2Fields::FRAMES_OK), Some(0));
    assert_eq!(a.at(&Dvbt2Fields::OFFSET_HZ), None);
}

/// The stack states the elementary rate of the channel width, and at that rate its graph has no resampler.
#[test]
fn the_stack_states_the_rate_of_the_channel_width() {
    use sdrkit_core::{Choice, InputInfo, Receiver};
    let schema = Dvbt2Setting::schema();
    let mut v = Values::defaults(&schema);
    assert_eq!(DVBT2.rate_hz(&v), 64e6 / 7.0);
    let width = schema.id("bandwidth").unwrap();
    v.set(width, &Value::Choice(Bandwidth::B6.index())).unwrap();
    assert_eq!(DVBT2.rate_hz(&v), 48e6 / 7.0);
    v.set(width, &Value::Choice(Bandwidth::B7.index())).unwrap();
    assert_eq!(DVBT2.rate_hz(&v), 8e6);
    let resamplers = |rate: f64| {
        let mut rx = sdrkit_stack_dvbt2::receiver(&StackConfig::new(v.clone())).unwrap();
        rx.configure(&InputInfo::new(rate, 474e6, 474e6).recording())
            .unwrap();
        let inserted = &rx.graph().report().adapters;
        inserted.iter().filter(|a| a.kind.contains("resampl")).count()
    };
    assert_eq!(resamplers(8e6), 0);
    assert_eq!(resamplers(64e6 / 7.0), 1);
}

sdrkit_core::settings! {
    enum TerrestrialSetting {
        DecodeVideo = DECODE_VIDEO,
    }
}

use sdrkit_proto_mpegts::layer::DECODE_VIDEO;

#[test]
fn decode_video_of_the_terrestrial_stack_reaches_the_demultiplexer() {
    let mut v = Values::defaults(&TerrestrialSetting::schema());
    v.set(TerrestrialSetting::DecodeVideo.id(), &Value::Bool(true))
        .unwrap();
    let rx = sdrkit_stack_dvbt2::receiver(&StackConfig::new(v)).unwrap();
    let demux = rx.graph().component("ts_demux").expect("the demultiplexer");
    let values = demux.values();
    let id = values.schema().id("decode_video").unwrap();
    assert_eq!(values.value(id), Some(&Value::Bool(true)));
}

/// The default test signal carries one service with a video and a sound track, and the stack plays the tone.
#[test]
fn the_test_signal_carries_a_service_with_video_and_sound() {
    use sdrkit_core::{Complex32, GenConfig, Generator, InputInfo, Output, Receiver};
    use sdrkit_proto_mpegts::codec::{EsCodec, EsKind};
    use sdrkit_proto_mpegts::publish::{TsServiceRow, TsTrackRow, SERVICES, TRACKS};
    use sdrkit_stack_dvbt2::generator::{SOUND_PID, VIDEO_PID};
    let schema = Dvbt2Setting::schema();
    let rate = Bandwidth::B8.rate();
    let mut g = sdrkit_stack_dvbt2::T2Generator::from_config(&GenConfig::new(
        rate,
        Values::defaults(&schema),
    ));
    let mut rx =
        sdrkit_stack_dvbt2::receiver(&StackConfig::new(Values::defaults(&schema))).expect("builds");
    let mut codecs = sdrkit_core::codec::CodecSet::default();
    sdrkit_codec_mpa::register(&mut codecs);
    rx.set_facilities(&sdrkit_core::Facilities {
        codecs: std::sync::Arc::new(codecs),
        ..Default::default()
    });
    rx.configure(&InputInfo::new(rate, 474e6, 474e6).recording())
        .expect("rate");
    let mut x = vec![Complex32::default(); (rate * 0.1) as usize];
    let mut out = Output::default();
    let (mut peak, mut pictures) = (0f32, 0);
    for _ in 0..25 {
        g.generate(&mut x);
        for c in x.chunks(16_384) {
            out.clear();
            rx.feed(c, &mut out);
            peak = out.audio().iter().fold(peak, |m, s| m.max(s.abs()));
            pictures += out
                .media
                .iter()
                .filter(|e| e.format.codec == sdrkit_core::media::Codec::Mpeg2Video)
                .count();
        }
    }
    let r = sdrkit_core::owner::Owner::results(&rx);
    let services: Vec<(&str, TsServiceRow)> = SERVICES.rows(r).collect();
    assert_eq!(services.len(), 1);
    assert_eq!(services[0].1.name, "sdrkit DVB-T2");
    let tracks: Vec<(&str, TsTrackRow)> = TRACKS.rows(r).collect();
    let track = |pid: u16| {
        tracks
            .iter()
            .find(|t| t.1.pid == i64::from(pid))
            .map(|t| (t.1.service, t.1.kind, t.1.codec))
    };
    assert_eq!(
        track(VIDEO_PID),
        Some(("1", Some(EsKind::Video), Some(EsCodec::Mpeg2Video)))
    );
    assert_eq!(
        track(SOUND_PID),
        Some(("1", Some(EsKind::Audio), Some(EsCodec::Mp2)))
    );
    // The pictures go to the host, and the tone of a quarter of full scale plays.
    assert!(pictures > 10, "{pictures} pictures");
    assert!((0.2..0.3).contains(&peak), "{peak}");
}
