//! Analogue video receivers: analogue television ([`ATV`]: PAL, NTSC and SECAM with FM, AM, NICAM-728 and A2 sound and teletext,
//! FM video and NBTV), D2-MAC and D-MAC ([`MAC`]), and analogue FPV video ([`FPV`]). Each stack is a graph of components with
//! a generator of its clean test signal. `atv` and `fpv` also send: the generator fed with the host's pictures and sound
//! ([`transmit`]).
//!
//! Standard: ITU-R BT.470-6, ETSI EN 300 163, ITU-R BS.707, ETS 300 250, ETS 300 706.

#![warn(missing_docs)]
// The DSP loops index several buffers in step. An index is clearer than zipped iterators there.
#![allow(clippy::needless_range_loop)]

pub mod atv;
mod bench;
pub mod card;
pub mod channels;
pub mod fpv;
pub mod gen_atv;
pub mod gen_video;
pub mod mac;
pub mod nicam;
pub mod results;
pub mod settings;
pub mod sound_gen;
pub mod systems;
pub mod teletext;
pub mod transmit;
pub mod typed;
pub mod util;

/// The recorded cost of each stage of the stacks of the family.
pub const COSTS: &str = include_str!("../cost.txt");

pub use bench::benchmarks;
pub use gen_atv::{AtvGenParams, AtvGenerator};

mod verify;

use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, SettingSet, StackConfig,
    StackDescriptor, Tuning,
};
use settings::{
    AtvSetting, FpvSetting, MacSetting, GEN_NBTV_LINES, GEN_NBTV_PATTERN, GEN_NBTV_RATE, GEN_SYSTEM,
};

fn make_atv(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    if cfg.settings.get(&settings::MODULATION) == settings::ModulationChoice::Auto {
        return Ok(Box::new(atv::auto::AutoReceiver::new(cfg)?));
    }
    Ok(Box::new(atv::graph::receiver(cfg)?))
}

fn make_atv_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let sys = v.get(&GEN_SYSTEM);
    if sys.system().is_none() {
        let lines = v.get(&GEN_NBTV_LINES);
        let rate = v.get(&GEN_NBTV_RATE);
        let format = atv::nbtv::NbtvFormat {
            lines: if lines >= 24 { lines as u32 } else { 32 },
            rate: if rate > 0.0 { rate } else { 12.5 },
            vertical: true,
        };
        return Ok(Box::new(atv::nbtv::NbtvGenerator::new(
            format,
            cfg.rate_hz,
            sys == settings::GenSystem::NbtvFm,
            5e3,
            v.get(&GEN_NBTV_PATTERN) as u32,
        )));
    }
    Ok(Box::new(AtvGenerator::new(settings::atv_gen_params(
        cfg.rate_hz,
        v,
    ))))
}

/// The analogue TV stack.
pub static ATV: StackDescriptor = StackDescriptor {
    id: "atv",
    name: "Analog TV",
    group: "Video",
    blurb: "Analogue television: PAL, NTSC and SECAM pictures with FM, AM, NICAM or A2 sound and teletext; also FM video and NBTV. Transmits a picture and the sound that the host gives.",
    tuning: Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 45e6,
        max_hz: 6e9,
        default_hz: 0.0,
        rate_hz: 10e6,
        min_rate_hz: atv::graph::MIN_RATE_HZ,
        bandwidth_hz: 8e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(9e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: AtvSetting::schema,
    make_receiver: make_atv,
    make_generator: Some(make_atv_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::AnalogTv],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::ATV_HOOKS),
    benchmarks: Some(benchmarks),
};

fn make_mac(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(mac::receiver(cfg)?))
}

fn make_mac_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(mac::gen::MacGenerator::new(
        settings::mac_gen_params(cfg.rate_hz, &cfg.settings),
    )))
}

/// The MAC stack.
pub static MAC: StackDescriptor = StackDescriptor {
    id: "mac",
    name: "MAC",
    group: "Video",
    blurb: "D2-MAC and D-MAC satellite television: the picture, the digital sound and the service names.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 50e6,
        max_hz: 6e9,
        default_hz: 0.0,
        rate_hz: 20e6,
        min_rate_hz: mac::MIN_RATE_HZ,
        bandwidth_hz: 27e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(20e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: MacSetting::schema,
    make_receiver: make_mac,
    make_generator: Some(make_mac_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Mac],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MAC_HOOKS),
    benchmarks: None,
};

fn make_fpv(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(fpv::receiver(cfg)?))
}

fn make_fpv_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(fpv::FpvGenerator::new(settings::fpv_gen_params(
        cfg.rate_hz,
        &cfg.settings,
    ))))
}

/// The analogue FPV video stack.
pub static FPV: StackDescriptor = StackDescriptor {
    id: "fpv",
    name: "FPV video (analogue)",
    group: "Video",
    blurb: "Analogue FPV video transmitters on the 5.8 GHz and 1.2 GHz channels: a channel table, the picture and its sound. Transmits a picture and the sound that the host gives.",
    tuning: Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 900e6,
        max_hz: 6e9,
        default_hz: 0.0,
        rate_hz: 20e6,
        min_rate_hz: fpv::MIN_RATE_HZ,
        bandwidth_hz: 18e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(18e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: FpvSetting::schema,
    make_receiver: make_fpv,
    make_generator: Some(make_fpv_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::FmVideo],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::FPV_HOOKS),
    benchmarks: Some(bench::fpv_benchmarks),
};

/// Every stack of the family.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&ATV, &MAC, &FPV]
}

/// Adds the components of this crate (`atv.*`, `mac.*`, `fpv.*`) to a graph registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    atv::graph::register(r);
    mac::register(r);
    fpv::register(r);
    transmit::register(r);
}
