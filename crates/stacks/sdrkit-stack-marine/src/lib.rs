//! The marine stacks: AIS, MF/HF and VHF DSC, NAVTEX, HF weather fax, VDES, marine VHF voice and ATIS on inland waterways.
//! [`descriptors`] lists them.

// DSP code indexes several arrays in step. Plain index loops read more clearly there.
#![allow(clippy::needless_range_loop)]

pub mod ais;
pub mod atis;
mod bench;
pub mod dsc;
pub mod dsp;
pub mod fax;
pub mod fsk;
pub mod hfgen;
pub mod marine;
pub mod navtex;
pub mod report;
pub mod typed;
pub mod vdes;
pub mod verify;
pub mod vhf;
pub mod vhfplan;

pub use bench::benchmarks;

use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, SignalType, StackConfig,
    StackDescriptor, SubMode, Tuning,
};
use sdrkit_graph::{GraphError, Registry, StackGraph};

/// The group every marine stack is listed under.
const GROUP: &str = "Marine";

/// Adds every marine component kind to `r`, so a marine graph can be built from a description.
pub fn register(r: &mut Registry) {
    ais::receiver::register(r);
    marine::register(r);
    vdes::register(r);
    vhf::register(r);
    atis::register(r);
    r.add("marine.frontend", || {
        Box::new(dsp::FixedRateFrontEnd::default())
    });
}

/// The receiver of a stack graph.
fn stack_receiver(built: Result<StackGraph, GraphError>) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}

fn make_ais(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(ais::receiver::stack_graph(cfg))
}

fn make_ais_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(ais::generator::from_config(cfg))
}

/// AIS.
pub static AIS: StackDescriptor = StackDescriptor {
    id: "ais",
    name: "AIS",
    group: GROUP,
    blurb: "Ships, base stations and aids to navigation on AIS 1 and 2 and the long-range channels, on a map and in a table.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 156e6,
        max_hz: 163e6,
        default_hz: 162.0e6,
        rate_hz: 2e6,
        min_rate_hz: ais::receiver::MIN_RATE_HZ,
        bandwidth_hz: 0.1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: ais::receiver::schema,
    make_receiver: make_ais,
    make_generator: Some(make_ais_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ais],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::AIS),
    benchmarks: Some(benchmarks),
};

fn make_marine(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(marine::stack_graph(cfg))
}

fn make_marine_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(hfgen::from_config(cfg))
}

/// The marine HF receiver: NAVTEX, DSC and weather fax.
pub static MARINE: StackDescriptor = StackDescriptor {
    id: "marine",
    name: "Marine (HF)",
    group: GROUP,
    blurb: "NAVTEX warnings, DSC calls on MF/HF and VHF channel 70, and HF weather fax charts.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 0.1e6,
        max_hz: 174e6,
        default_hz: 518e3,
        rate_hz: 2e6,
        min_rate_hz: marine::MIN_RATE_HZ,
        bandwidth_hz: 0.01e6,
        tune_offset_hz: marine::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[
        SubMode {
            id: "navtex",
            name: "NAVTEX",
            default_hz: 518e3,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "dsc",
            name: "DSC",
            default_hz: 2187.5e3,
            ..SubMode::DEFAULT
        },
        SubMode {
            id: "wefax",
            name: "Weather fax",
            default_hz: 7878.1e3,
            ..SubMode::DEFAULT
        },
    ],
    settings: marine::schema,
    make_receiver: make_marine,
    make_generator: Some(make_marine_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Sitor, SignalType::Dsc, SignalType::Wefax],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MARINE),
    benchmarks: None,
};

fn make_vdes(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(vdes::stack_graph(cfg))
}

fn make_vdes_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(vdes::from_config(cfg))
}

/// VDES.
pub static VDES: StackDescriptor = StackDescriptor {
    id: "vdes",
    name: "VDES",
    group: GROUP,
    blurb: "VHF data exchange: ASM frames decoded, VDE-TER bursts measured and classified.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 150e6,
        max_hz: 165e6,
        default_hz: 161.9e6,
        rate_hz: 1e6,
        min_rate_hz: vdes::MIN_RATE_HZ,
        bandwidth_hz: 0.4e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: <vdes::VdesSetting as sdrkit_core::SettingSet>::schema,
    make_receiver: make_vdes,
    make_generator: Some(make_vdes_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Vdes],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::VDES),
    benchmarks: None,
};

fn make_atis(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(atis::stack_graph(cfg))
}

fn make_atis_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(atis::from_config(cfg))
}

/// ATIS on inland waterways.
pub static ATIS: StackDescriptor = StackDescriptor {
    id: "atis",
    name: "ATIS (inland waterways)",
    group: GROUP,
    blurb: "The voice of an inland waterway channel and the ATIS identity each radio sends after it speaks.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 16e3)),
        min_hz: 156.0e6,
        max_hz: 162.1e6,
        default_hz: 156.5e6,
        rate_hz: 1e6,
        min_rate_hz: atis::MIN_RATE_HZ,
        bandwidth_hz: 0.5e6,
        tune_offset_hz: atis::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: <atis::AtisSetting as sdrkit_core::SettingSet>::schema,
    make_receiver: make_atis,
    make_generator: Some(make_atis_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Atis],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::ATIS),
    benchmarks: None,
};

fn make_vhf(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(vhf::stack_graph(cfg))
}

fn make_vhf_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(vhf::from_config(cfg))
}

/// Marine VHF voice.
pub static MARINEVHF: StackDescriptor = StackDescriptor {
    id: "marinevhf",
    name: "Marine VHF",
    group: GROUP,
    blurb: "Voice on every marine VHF channel in the captured span, with a call log and a scan that watches a priority channel.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 16e3)),
        min_hz: 156.0e6,
        max_hz: 162.1e6,
        default_hz: 159.0e6,
        rate_hz: 8e6,
        min_rate_hz: vhf::MIN_RATE_HZ,
        bandwidth_hz: 6e6,
        tune_offset_hz: vhf::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: <vhf::VhfSetting as sdrkit_core::SettingSet>::schema,
    make_receiver: make_vhf,
    make_generator: Some(make_vhf_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::NarrowFmVoice, SignalType::KeyedVoice],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MARINEVHF),
    benchmarks: None,
};

/// Every marine stack.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&AIS, &MARINE, &VDES, &MARINEVHF, &ATIS]
}
