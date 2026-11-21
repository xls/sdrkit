//! Voice channel stacks: the multi-carrier voice channel receiver, and three single-channel stacks behind one 12 kHz front
//! end.
//!
//! | Id | Stack | Module |
//! |---|---|---|
//! | `voice_channels` | CB, FRS / GMRS, PMR446, amateur FM repeaters, HF voice: every carrier of the capture at once, named by the channel plan that the host loads | [`channels`] |
//! | `hfpacket` | HF packet 300 Bd (AX.25 frames in AFSK on a sideband) | [`hfpacket`] |
//! | `elt121` | ELT 121.5 MHz (detects the swept tone; detection only) | [`elt121`] |
//! | `mdc1200` | MDC-1200: the multi-carrier receiver with no plan and a burst decoder on each carrier | [`mdc`] |
//! | `fleetsync` | FleetSync: the same receiver with a FleetSync burst decoder on each carrier (fleet and unit; other bits raw) | [`fleetsync`] |
//! | `wmic` | Wireless microphone finder: wide FM carriers on a coarse raster, measured and named by a band plan file | [`wmic`] |
//! | `voice_inversion` | Voice inversion scrambler (detects, estimates the carrier, descrambles) | [`inversion`] |
//!
//! [`burstfeed`] feeds a burst decoder on each carrier of the multi-carrier receiver. [`common`] holds the front end, the common result fields and the event list of the three single-channel stacks.
//! [`plans`] reads a channel plan from a plan file and holds what each submode fixes, [`rx`] is the voice channel receiver
//! of one carrier, [`scene`] the test signal of the multi-carrier receiver. The plan files in the repository's `data/voice/` are examples for a
//! host to load. They are not part of the library.

pub mod burstfeed;
pub mod channels;
pub mod common;
pub mod elt121;
pub mod fleetsync;
pub mod hfpacket;
pub mod inversion;
pub mod mdc;
pub mod plans;
pub mod rx;
pub mod scene;
pub mod typed;
pub mod verify;
pub mod wmic;

use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockProfile, Output, Receiver, SettingSet, StackConfig, StackDescriptor, SubMode, Tuning,
    Values,
};
use sdrkit_graph::Registry;

/// The tuning that the three stacks share: 2 Msps, 250 ksps at least, the radio 20 kHz above the dial.
const fn tuning(min_hz: f64, max_hz: f64, default_hz: f64, bandwidth_hz: f64) -> Tuning {
    Tuning {
        min_hz,
        max_hz,
        default_hz,
        rate_hz: 2e6,
        min_rate_hz: common::MIN_RATE_HZ,
        bandwidth_hz,
        tune_offset_hz: common::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    }
}

fn make_hfpacket(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(hfpacket::stack_graph(cfg)?))
}

fn make_hfpacket_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(hfpacket::PacketGenerator::from_config(cfg)))
}

/// HF packet 300 Bd.
pub static HFPACKET: StackDescriptor = StackDescriptor {
    id: hfpacket::ID,
    name: "HF packet 300 Bd",
    group: "HF utility",
    blurb:
        "AX.25 frames of amateur packet radio on HF: 300 Bd AFSK with a 200 Hz shift on a sideband.",
    tuning: tuning(1.5e6, 30e6, 0.0, 4e3),
    submodes: &[],
    settings: hfpacket::PacketSetting::schema,
    make_receiver: make_hfpacket,
    make_generator: Some(make_hfpacket_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: Some(benchmarks),
};

fn make_elt121(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(elt121::stack_graph(cfg)?))
}

fn make_elt121_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(elt121::EltGenerator::from_config(cfg)))
}

/// ELT 121.5 MHz.
pub static ELT121: StackDescriptor = StackDescriptor {
    id: elt121::ID,
    name: "ELT 121.5 MHz",
    group: "Navigation",
    blurb: "Detects the swept tone of an analogue emergency locator transmitter on 121.5 MHz. Detection only: the beacon sends no identity.",
    tuning: Tuning {
        carrier_at_centre: true,
        // ICAO Annex 10 fixes the aeronautical emergency channel at 121.5 MHz worldwide.
        ..tuning(121.0e6, 122.0e6, 121.5e6, 12e3)
    },
    submodes: &[],
    settings: elt121::EltSetting::schema,
    make_receiver: make_elt121,
    make_generator: Some(make_elt121_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

fn make_inversion(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(inversion::stack_graph(cfg)?))
}

fn make_inversion_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(inversion::InversionGenerator::from_config(cfg)?))
}

/// The voice inversion scrambler.
pub static VOICE_INVERSION: StackDescriptor = StackDescriptor {
    id: inversion::ID,
    name: "Voice inversion scrambler",
    group: "Listening",
    blurb: "Detects speech with an inverted spectrum on a narrow-FM channel, estimates the inversion carrier and descrambles it.",
    tuning: tuning(1.5e6, 500e6, 0.0, 12e3),
    submodes: &[],
    settings: inversion::InversionSetting::schema,
    make_receiver: make_inversion,
    make_generator: Some(make_inversion_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

fn make_voice_channels(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(channels::stack_graph(cfg)?))
}

/// The test scene of the `gen_scene` setting. Automatic is the scene of the submode, and the mixed scene when the
/// configuration names no submode.
fn make_voice_channels_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let kind = match s.get(&channels::GEN_SCENE) {
        scene::GenScene::Auto => cfg
            .submode
            .as_deref()
            .filter(|id| !id.is_empty())
            .and_then(|id| channels::service_of(Some(id)))
            .map_or(scene::GenScene::Mixed, scene::GenScene::of),
        kind => kind,
    };
    let scene = scene::demo(
        kind,
        cfg.rate_hz,
        s.get(&channels::GEN_CNR_DB),
        cfg.seed.wrapping_add(1),
    );
    Ok(Box::new(scene::SceneGenerator::new(scene)?))
}

/// Voice channels. The radio is tuned without an offset: the host places the capture centre between channels.
pub static VOICE_CHANNELS: StackDescriptor = StackDescriptor {
    id: channels::ID,
    name: "Voice channels",
    group: "Listening",
    blurb: "Every voice carrier inside the capture at once: CB, FRS / GMRS, PMR446, amateur FM repeaters and HF voice, with tones, a call log and a scan. A plan file that the host loads names the channels.",
    tuning: Tuning {
        min_hz: 2.8e6,
        max_hz: 480e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 48e3,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[
        SubMode {
            id: "cb",
            name: "CB radio",
            default_hz: 0.0,
        },
        SubMode {
            id: "frs",
            name: "FRS / GMRS",
            default_hz: 0.0,
        },
        SubMode {
            id: "pmr446",
            name: "PMR446 analog",
            default_hz: 0.0,
        },
        SubMode {
            id: "ham_repeaters",
            name: "Ham FM repeaters",
            default_hz: 0.0,
        },
        SubMode {
            id: "hfvoice",
            name: "HFGCS / HF voice",
            default_hz: 0.0,
        },
    ],
    settings: channels::VoiceSetting::schema,
    make_receiver: make_voice_channels,
    make_generator: Some(make_voice_channels_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: Some(channel_benchmarks),
};

fn make_mdc1200(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(mdc::stack_graph(cfg)?))
}

fn make_mdc1200_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let scene = scene::mdc_demo(
        cfg.rate_hz,
        cfg.settings.get(&channels::GEN_CNR_DB),
        cfg.seed.wrapping_add(1),
    );
    Ok(Box::new(
        scene::SceneGenerator::new(scene)?.with_total_power(10e3),
    ))
}

/// MDC-1200. The radio is tuned without an offset, as for the voice channels.
pub static MDC1200: StackDescriptor = StackDescriptor {
    id: mdc::ID,
    name: "MDC-1200",
    group: "Trunking",
    blurb: "The 1200 bit/s signalling bursts on analogue FM voice channels: every FM carrier inside the capture is followed, and the unit id of each burst is listed with the calls.",
    tuning: Tuning {
        min_hz: 30e6,
        max_hz: 960e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: mdc::MIN_RATE_HZ,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: mdc::MdcSetting::schema,
    make_receiver: make_mdc1200,
    make_generator: Some(make_mdc1200_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: Some(mdc_benchmarks),
};

fn make_fleetsync(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(fleetsync::stack_graph(cfg)?))
}

fn make_fleetsync_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let scene = fleetsync::demo(cfg.rate_hz, &cfg.settings, cfg.seed.wrapping_add(1));
    Ok(Box::new(
        scene::SceneGenerator::new(scene)?.with_total_power(10e3),
    ))
}

/// FleetSync. The radio is tuned without an offset, as for MDC-1200.
pub static FLEETSYNC: StackDescriptor = StackDescriptor {
    id: fleetsync::ID,
    name: "FleetSync",
    group: "Trunking",
    blurb: "The signalling bursts of FleetSync radios on analogue FM voice channels: every FM carrier inside the capture is followed, and the fleet and unit of each burst are listed with the calls. An identity reader: the other bits are shown raw.",
    tuning: Tuning {
        min_hz: 30e6,
        max_hz: 960e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: fleetsync::MIN_RATE_HZ,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: fleetsync::FleetsyncSetting::schema,
    make_receiver: make_fleetsync,
    make_generator: Some(make_fleetsync_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::FLEETSYNC),
    benchmarks: Some(fleetsync_benchmarks),
};

fn make_wmic(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(wmic::stack_graph(cfg)?))
}

fn make_wmic_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let scene = scene::wide_demo(
        cfg.rate_hz,
        cfg.settings.get(&channels::GEN_CNR_DB),
        cfg.seed.wrapping_add(1),
    );
    Ok(Box::new(
        scene::SceneGenerator::new(scene)?.with_total_power(wmic::CHANNEL_WIDTH_HZ),
    ))
}

/// The wireless microphone finder. The radio is tuned without an offset; the host tunes it through the bands.
pub static WMIC: StackDescriptor = StackDescriptor {
    id: wmic::ID,
    name: "Wireless mic finder",
    group: "Listening",
    blurb: "Wide FM carriers of wireless microphones and in-ear monitors: the width and the deviation of each one, whether it looks like analogue FM, and the audio of one. A plan file that the host loads names the bands.",
    tuning: Tuning {
        min_hz: 470e6,
        max_hz: 960e6,
        default_hz: 0.0,
        rate_hz: 10e6,
        min_rate_hz: wmic::MIN_RATE_HZ,
        bandwidth_hz: 8e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: wmic::WmicSetting::schema,
    make_receiver: make_wmic,
    make_generator: Some(make_wmic_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: Some(wmic_benchmarks),
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &HFPACKET,
        &ELT121,
        &VOICE_INVERSION,
        &VOICE_CHANNELS,
        &MDC1200,
        &FLEETSYNC,
        &WMIC,
    ]
}

/// Adds every component kind of the crate to `r` as `voice.<name>`.
pub fn register(r: &mut Registry) {
    r.add("voice.hfpacket", || Box::new(hfpacket::HfPacket::default()));
    r.add("voice.elt121", || Box::new(elt121::Elt121::default()));
    r.add("voice.inversion", || {
        Box::new(inversion::VoiceInversion::default())
    });
    channels::register(r);
    mdc::register(r);
    fleetsync::register(r);
    wmic::register(r);
}

/// Times the receiver of `desc` on `signal_s` seconds of its test signal at `rate`.
fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)())
        .expect("the settings of every stack take the generator settings");
    let settings = Values::defaults(&schema);
    let mut g = (desc.make_generator.expect("every stack has a generator"))(
        &GenConfig::new(rate, settings.clone()).with_seed(1),
    )
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    let k = g.generate(&mut x);
    x.truncate(k);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    // A stack without a default frequency takes the middle of its range.
    let centre = if desc.tuning.default_hz > 0.0 {
        desc.tuning.default_hz
    } else {
        (desc.tuning.min_hz + desc.tuning.max_hz) / 2.0
    };
    let _ = rx.configure(&InputInfo::new(rate, centre, centre).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65_536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks of the receivers (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "hfpacket.receiver_2m",
            run: |s| bench_receiver(s, &HFPACKET, 2e6, 1.0),
        },
        Benchmark {
            name: "elt121.receiver_2m",
            run: |s| bench_receiver(s, &ELT121, 2e6, 1.0),
        },
        Benchmark {
            name: "voice_inversion.receiver_2m",
            run: |s| bench_receiver(s, &VOICE_INVERSION, 2e6, 1.0),
        },
    ]
}

/// The benchmark of the voice channel receiver on its mixed test signal (items are input samples).
pub fn channel_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "voice_channels.receiver_2m",
        run: |s| bench_receiver(s, &VOICE_CHANNELS, 2e6, 3.0),
    }]
}

/// The benchmark of the MDC-1200 receiver on its test signal (items are input samples).
pub fn mdc_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "mdc1200.receiver_2m",
        run: |s| bench_receiver(s, &MDC1200, 2e6, 3.0),
    }]
}

/// The benchmark of the FleetSync receiver on its test signal (items are input samples).
pub fn fleetsync_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "fleetsync.receiver_2m",
        run: |s| bench_receiver(s, &FLEETSYNC, 2e6, 3.0),
    }]
}

/// The benchmark of the wireless microphone finder on its test signal (items are input samples).
pub fn wmic_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "wmic.receiver_10m",
        run: |s| bench_receiver(s, &WMIC, 10e6, 2.0),
    }]
}
