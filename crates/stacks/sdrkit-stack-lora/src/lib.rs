//! LoRa, mesh texting and APRS: the `mesh` stack and its protocol stacks, `lorawanwide`, `loraraw` and `aprs`. Each receiver is
//! a graph of the components in [`components`], and each stack has a generator of its clean test signal ([`gen`]). Encrypted
//! content is read only with keys the user holds, and the published default keys.

#![warn(missing_docs)]

pub mod aprs;
pub mod commands;
pub mod components;
pub mod gen;
pub mod mesh;
pub mod proto;
pub mod receivers;
pub mod results;
pub mod settings;
pub mod typed;
pub mod verify;

use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockProfile, Output, Receiver, SettingSet, SignalType, StackConfig, StackDescriptor, SubMode,
    Tuning, Values,
};
use sdrkit_graph::{GraphError, StackGraph};
use settings::{AprsSetting, MeshSetting, RawSetting, WideSetting};

fn stack_receiver(built: Result<StackGraph, GraphError>) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}

/// The tuning of the mesh stacks. The host sets the frequency: the channels of a network are its settings.
const fn mesh_tuning() -> Tuning {
    Tuning {
        min_hz: 400e6,
        max_hz: 2500e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 1e6,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: 300e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    }
}

fn mesh_generator(stack: &str, cfg: &GenConfig) -> Box<dyn Generator> {
    let mut c = cfg.clone();
    if c.submode.is_none() {
        c.submode = Some(stack.into());
    }
    gen::from_config("mesh", &c)
}

fn make_mesh(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("mesh", cfg))
}
fn gen_mesh(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(gen::from_config("mesh", cfg))
}
fn make_meshtastic(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("meshtastic", cfg))
}
fn gen_meshtastic(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(mesh_generator("meshtastic", cfg))
}
fn make_meshcore(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("meshcore", cfg))
}
fn gen_meshcore(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(mesh_generator("meshcore", cfg))
}
fn make_lorawan(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("lorawan", cfg))
}
fn gen_lorawan(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(mesh_generator("lorawan", cfg))
}
fn make_loraaprs(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("loraaprs", cfg))
}
fn gen_loraaprs(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(mesh_generator("loraaprs", cfg))
}
fn make_meshcom(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::mesh_receiver("meshcom", cfg))
}
fn gen_meshcom(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(mesh_generator("meshcom", cfg))
}

/// The protocol stacks that `mesh` can run as.
pub const MESH_SUBMODES: &[SubMode] = &[
    SubMode {
        id: "meshtastic",
        name: "Meshtastic",
        default_hz: 0.0,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "meshcore",
        name: "MeshCore",
        default_hz: 0.0,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "lorawan",
        name: "LoRaWAN",
        default_hz: 0.0,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "loraaprs",
        name: "LoRa APRS",
        default_hz: 0.0,
        ..SubMode::DEFAULT
    },
    SubMode {
        id: "meshcom",
        name: "MeshCom",
        default_hz: 0.0,
        ..SubMode::DEFAULT
    },
];

/// Mesh (LoRa): one stack for every protocol of the family.
pub static MESH: StackDescriptor = StackDescriptor {
    id: "mesh",
    name: "Mesh (LoRa)",
    group: "IoT and mesh",
    blurb: "Meshtastic, MeshCore, LoRa APRS, MeshCom and LoRaWAN frames on one band plan, with nodes, messages and maps.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: MESH_SUBMODES,
    settings: MeshSetting::schema,
    make_receiver: make_mesh,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_mesh),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MESH),
    benchmarks: Some(sdrkit_proto_lora_phy::benchmarks),
};

/// Meshtastic: a texting client with channels, direct messages, acknowledgements, the node list and positions.
pub static MESHTASTIC: StackDescriptor = StackDescriptor {
    id: "meshtastic",
    name: "Meshtastic",
    group: "IoT and mesh",
    blurb: "A Meshtastic texting client: channels with keys, direct messages, acknowledgements, the node list and positions.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: &[],
    settings: settings::MeshtasticSetting::schema,
    make_receiver: make_meshtastic,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_meshtastic),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MESHTASTIC),
    benchmarks: None,
};

/// MeshCore: a texting client with adverts, contacts, direct messages and channels.
pub static MESHCORE: StackDescriptor = StackDescriptor {
    id: "meshcore",
    name: "MeshCore",
    group: "IoT and mesh",
    blurb: "A MeshCore texting client: adverts, contacts, direct messages with acknowledgements and channels.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: &[],
    settings: settings::MeshcoreSetting::schema,
    make_receiver: make_meshcore,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_meshcore),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MESHCORE),
    benchmarks: None,
};

/// LoRaWAN on the channels of one plan.
pub static LORAWAN: StackDescriptor = StackDescriptor {
    id: "lorawan",
    name: "LoRaWAN",
    group: "IoT and mesh",
    blurb: "LoRaWAN frame headers and devices on the channels of one plan.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: &[],
    settings: settings::LorawanSetting::schema,
    make_receiver: make_lorawan,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_lorawan),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LORAWAN),
    benchmarks: None,
};

/// LoRa APRS on 70 cm.
pub static LORAAPRS: StackDescriptor = StackDescriptor {
    id: "loraaprs",
    name: "LoRa APRS",
    group: "Amateur",
    blurb: "APRS stations, positions and messages over LoRa on 70 cm.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: &[],
    settings: settings::LoraAprsSetting::schema,
    make_receiver: make_loraaprs,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_loraaprs),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LORAAPRS),
    benchmarks: None,
};

/// MeshCom 4 on 70 cm.
pub static MESHCOM: StackDescriptor = StackDescriptor {
    id: "meshcom",
    name: "MeshCom",
    group: "Amateur",
    blurb: "MeshCom 4 amateur mesh texts and positions on 70 cm.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..mesh_tuning()
    },
    submodes: &[],
    settings: settings::MeshcomSetting::schema,
    make_receiver: make_meshcom,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_meshcom),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MESHCOM),
    benchmarks: None,
};

fn make_wide(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::wide_receiver(cfg))
}
fn gen_wide(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(gen::from_config("lorawanwide", cfg))
}

/// LoRaWAN (all channels).
pub static LORAWANWIDE: StackDescriptor = StackDescriptor {
    id: "lorawanwide",
    name: "LoRaWAN (all channels)",
    group: "IoT and mesh",
    blurb: "Every channel of a LoRaWAN plan at once, uplinks and downlinks, and the FSK channel.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 137e6,
        max_hz: 2500e6,
        default_hz: 0.0,
        rate_hz: 4e6,
        min_rate_hz: 1e6,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: 100e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(3.5e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: WideSetting::schema,
    make_receiver: make_wide,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_wide),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LORAWANWIDE),
    benchmarks: None,
};

fn make_raw(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::raw_receiver(cfg))
}
fn gen_raw(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(gen::from_config("loraraw", cfg))
}

/// LoRa (raw).
pub static LORARAW: StackDescriptor = StackDescriptor {
    id: "loraraw",
    name: "LoRa (raw)",
    group: "IoT and mesh",
    blurb: "Plain point-to-point LoRa frames as bytes at every spreading factor and bandwidth; receive and transmit.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 137e6,
        max_hz: 2500e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 1e6,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: 300e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: RawSetting::schema,
    make_receiver: make_raw,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_raw),
    signal_types: &[SignalType::Lora],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LORARAW),
    benchmarks: None,
};

fn make_aprs(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::aprs_receiver(cfg))
}
fn gen_aprs(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(gen::from_config("aprs", cfg))
}

/// APRS (AFSK).
pub static APRS: StackDescriptor = StackDescriptor {
    id: "aprs",
    name: "APRS (AFSK)",
    group: "Amateur",
    blurb: "AX.25 packet radio and APRS at 1200 bit/s AFSK and 9600 bit/s G3RUH: stations, positions, weather and messages.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 12.5e3)),
        min_hz: 25e6,
        max_hz: 1000e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 240e3,
        bandwidth_hz: 25e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: AprsSetting::schema,
    make_receiver: make_aprs,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_aprs),
    signal_types: &[SignalType::PacketAfsk1200],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::APRS),
    benchmarks: None,
};

fn make_fanet(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::fanet_receiver(cfg))
}
fn gen_fanet(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(components::fanet::from_config(cfg)))
}

/// FANET.
pub static FANET: StackDescriptor = StackDescriptor {
    id: "fanet",
    name: "FANET",
    group: "IoT and mesh",
    blurb: "Paraglider, hang glider and glider tracking on one LoRa channel: positions, names, messages, weather stations and thermals.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 860e6,
        max_hz: 930e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 500e3,
        bandwidth_hz: 0.5e6,
        tune_offset_hz: components::fanet::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: components::fanet::FanetSetting::schema,
    make_receiver: make_fanet,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_fanet),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::FANET),
    benchmarks: Some(benchmarks),
};

fn make_reticulum(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::reticulum_receiver(cfg))
}
fn gen_reticulum(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(components::reticulum::from_config(cfg)))
}

/// Reticulum on RNode radios.
pub static RETICULUM: StackDescriptor = StackDescriptor {
    id: "reticulum",
    name: "Reticulum (RNode)",
    group: "IoT and mesh",
    blurb: "Reticulum packets on RNode LoRa radios: packet type, destination and hops, and announces with the signature checked. Data stays encrypted.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 137e6,
        max_hz: 2500e6,
        default_hz: 0.0,
        rate_hz: 1e6,
        min_rate_hz: 250e3,
        bandwidth_hz: 0.25e6,
        tune_offset_hz: components::reticulum::TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: components::reticulum::RnsSetting::schema,
    make_receiver: make_reticulum,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_reticulum),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::RETICULUM),
    benchmarks: None,
};

fn make_elrs(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    stack_receiver(receivers::elrs_receiver(cfg))
}
fn gen_elrs(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(components::elrs::from_config(cfg)))
}

/// ExpressLRS.
pub static ELRS: StackDescriptor = StackDescriptor {
    id: "elrs",
    name: "ExpressLRS",
    group: "IoT and mesh",
    blurb: "ExpressLRS radio-control links on the hop channels of one capture: link id, hop index and nonce from sync frames, frame counts for each link, and the stick values of your own link from its binding phrase. Experimental: the frame layout, the CRC start value, the rate rows and the hop plans are not confirmed by a public document.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 137e6,
        max_hz: 2500e6,
        default_hz: 0.0,
        rate_hz: 3e6,
        min_rate_hz: components::elrs::MIN_RATE_HZ,
        bandwidth_hz: 2.3e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(2.6e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: components::elrs::ElrsSetting::schema,
    make_receiver: make_elrs,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(gen_elrs),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::ELRS),
    benchmarks: None,
};

/// Runs the receiver of `desc` over `signal_s` seconds of its own test signal at the rate of the stack (items are input
/// samples).
fn bench_receiver(seconds: f64, desc: &'static StackDescriptor, signal_s: f64) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let rate = desc.tuning.rate_hz;
    let Some(Ok(mut g)) = desc
        .make_generator
        .map(|make| make(&GenConfig::new(rate, settings.clone()).with_seed(1)))
    else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    // A stack without a default frequency takes the middle of its range.
    let dial = if desc.tuning.default_hz > 0.0 {
        desc.tuning.default_hz
    } else {
        (desc.tuning.min_hz + desc.tuning.max_hz) / 2.0
    };
    if rx
        .configure(&InputInfo::new(rate, dial + desc.tuning.tune_offset_hz, dial).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The benchmarks of the receivers of single services: each receiver on its own test signal.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "fanet.receiver_2m",
            run: |s| bench_receiver(s, &FANET, 2.0),
        },
        Benchmark {
            name: "reticulum.receiver_1m",
            run: |s| bench_receiver(s, &RETICULUM, 4.0),
        },
        Benchmark {
            name: "elrs.receiver_3m",
            run: |s| bench_receiver(s, &ELRS, 2.0),
        },
    ]
}

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &MESH,
        &MESHTASTIC,
        &MESHCORE,
        &LORAWAN,
        &LORAAPRS,
        &MESHCOM,
        &LORAWANWIDE,
        &LORARAW,
        &APRS,
        &FANET,
        &RETICULUM,
        &ELRS,
    ]
}

/// Adds the family's component types to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    components::register(r);
}
