//! ADS-B and Mode S on 1090 MHz, Mode A/C replies, Mode S interrogations on 1030 MHz, and UAT with FIS-B on 978 MHz, with Beast,
//! SBS and ASTERIX feeds. Four stacks: `adsb`, `uat`, `modeac` and `uplink1030` ([`descriptors`]).
//!
//! Standard: ICAO Annex 10 Vol. IV; RTCA DO-260B; ICAO Doc 9871; RTCA DO-282B and DO-358B; EUROCONTROL ASTERIX 021, 048, 062

#![warn(missing_docs)]

pub mod asterix;
pub mod checks;
pub mod codes;
pub mod commb;
pub mod crc;
pub mod demod;
pub mod feeds;
pub mod frontend;
pub mod generator;
pub mod geo;
pub mod message;
pub mod modeac;
pub mod receiver;
pub mod settings;
pub mod table;
pub mod typed;
pub mod uat;
pub mod uplink;
pub mod verify;

pub use verify::register;

use frontend::{HalfSampleInterpolator, PulseDcRemoval};
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockProfile, Output, Receiver, SettingSet, SignalType, StackConfig, StackDescriptor, Tuning,
    Value, Values,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use std::sync::Arc;

/// The Mode A/C graph: interpolation below 4.5 Msps, DC removal, the decoder.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn modeac_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("interp", HalfSampleInterpolator::default())
        .add("dc", PulseDcRemoval::default())
        .add("dec", modeac::ModeAcDecoder::standalone())
        .input("interp")
        .connect("interp.out", "dc")
        .connect("dc", "dec.in")
        .connect("interp.delay", "dec.delay")
        .output("dec")
        .clock(0.25)
        .status_from("dec")
        .catalog(catalog)
        .build()
}

/// The Mode A/C receiver with these settings.
///
/// # Errors
///
/// The errors of [`modeac_graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn modeac_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(typed::shape("modeac"), modeac_graph(&Catalog::of(cfg)?)?)
        .min_rate(2e6, "Mode A/C needs a sample rate of at least 2 Msps")
        .route("expiry_s", &["dec.expiry_s"])?
        .route("min_replies", &["dec.min_replies"])?
        .command("clear", sdrkit_graph::CommandTarget::of("dec", "clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// The 1030 MHz graph: the decoder. The DPSK chips are 0.25 µs, so 8 Msps is the least that resolves them.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn uplink_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("dec", uplink::UplinkDecoder::default())
        .input("dec")
        .output("dec")
        .clock(0.25)
        .status_from("dec")
        .catalog(catalog)
        .build()
}

/// The 1030 MHz interrogation receiver with these settings.
///
/// # Errors
///
/// The errors of [`uplink_graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn uplink_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        typed::shape("uplink1030"),
        uplink_graph(&Catalog::of(cfg)?)?,
    )
    .min_rate(
        8e6,
        "1030 MHz interrogations need a sample rate of at least 8 Msps",
    )
    .route("expiry_s", &["dec.expiry_s"])?
    .command("clear", sdrkit_graph::CommandTarget::of("dec", "clear"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_adsb(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver::receiver(cfg)?))
}
fn make_adsb_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator::AdsbGenerator::from_config(cfg)))
}
fn make_uat(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(uat::receiver(cfg)?))
}
fn make_uat_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(uat::generator::UatGenerator::from_config(cfg)))
}
fn make_modeac(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(modeac_receiver(cfg)?))
}
fn make_modeac_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(modeac::ModeAcGenerator::from_config(cfg)))
}
fn make_uplink(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(uplink_receiver(cfg)?))
}
fn make_uplink_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(uplink::UplinkGenerator::from_config(cfg)))
}

/// ADS-B and Mode S on 1090 MHz.
pub static ADSB: StackDescriptor = StackDescriptor {
    id: "adsb",
    name: "ADS-B / Mode S",
    group: "Aircraft",
    blurb: "Aircraft on 1090 MHz: positions, callsigns, altitudes and speeds from ADS-B and Mode S replies.",
    tuning: Tuning {
        min_hz: 1085e6,
        max_hz: 1095e6,
        default_hz: 1090e6,
        rate_hz: 4e6,
        min_rate_hz: receiver::MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(3.5e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: settings::AdsbSetting::schema,
    make_receiver: make_adsb,
    make_generator: Some(make_adsb_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::AdsbModeS],
    auto_gain: AutoGain::High,
    verify: Some(&verify::ADSB_HOOKS),
    benchmarks: Some(benchmarks),
};

/// UAT and FIS-B on 978 MHz.
pub static UAT: StackDescriptor = StackDescriptor {
    id: "uat",
    name: "UAT 978 MHz",
    group: "Aircraft",
    blurb: "UAT on 978 MHz: aircraft and TIS-B targets, and the FIS-B weather and notices from ground stations.",
    tuning: Tuning {
        min_hz: 960e6,
        max_hz: 990e6,
        default_hz: 978e6,
        rate_hz: 2e6,
        min_rate_hz: uat::MIN_RATE_HZ,
        bandwidth_hz: 1e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.5e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: settings::UatSetting::schema,
    make_receiver: make_uat,
    make_generator: Some(make_uat_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Uat],
    auto_gain: AutoGain::High,
    verify: Some(&verify::UAT_HOOKS),
    benchmarks: None,
};

/// Mode A/C replies on 1090 MHz.
pub static MODEAC: StackDescriptor = StackDescriptor {
    id: "modeac",
    name: "Mode A/C replies",
    group: "Aircraft",
    blurb:
        "Mode A and Mode C transponder replies on 1090 MHz: squawk codes and pressure altitudes.",
    tuning: Tuning {
        min_hz: 1085e6,
        max_hz: 1095e6,
        default_hz: 1090e6,
        rate_hz: 4e6,
        min_rate_hz: 2e6,
        bandwidth_hz: 2e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(3.5e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: settings::ModeAcSetting::schema,
    make_receiver: make_modeac,
    make_generator: Some(make_modeac_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::ModeAcReplies],
    auto_gain: AutoGain::High,
    verify: Some(&verify::MODEAC_HOOKS),
    benchmarks: None,
};

/// Interrogations on 1030 MHz.
pub static UPLINK1030: StackDescriptor = StackDescriptor {
    id: "uplink1030",
    name: "Mode S interrogations 1030 MHz",
    group: "Aircraft",
    blurb: "Radar interrogations on 1030 MHz: Mode A, Mode C, all-calls and Mode S uplink messages with the aircraft they address.",
    tuning: Tuning {
        min_hz: 1025e6,
        max_hz: 1035e6,
        default_hz: 1030e6,
        rate_hz: 10e6,
        min_rate_hz: 8e6,
        bandwidth_hz: 8e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: true,
        radio_filter_hz: Some(8e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: settings::UplinkSetting::schema,
    make_receiver: make_uplink,
    make_generator: Some(make_uplink_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::ModeSInterrogations],
    auto_gain: AutoGain::High,
    verify: Some(&verify::UPLINK_HOOKS),
    benchmarks: None,
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&ADSB, &UAT, &MODEAC, &UPLINK1030]
}

fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let Ok(schema) = sdrkit_components::sim::with_generator_settings(&(desc.settings)()) else {
        return BenchResult::default();
    };
    let mut settings = Values::defaults(&schema);
    if let Ok(id) = schema.id(settings::EFFORT.key()) {
        let all = Value::Choice(sdrkit_core::Choice::index(demod::EffortChoice::All));
        if settings.set(id, &all).is_err() {
            return BenchResult::default();
        }
    }
    let Some(make) = desc.make_generator else {
        return BenchResult::default();
    };
    let Ok(mut g) = make(&GenConfig::new(rate, settings.clone()).with_seed(1)) else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let _ = rx.configure(
        &InputInfo::new(rate, desc.tuning.default_hz, desc.tuning.default_hz).recording(),
    );
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// Benchmarks of the receivers and the generator (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "adsb.receiver_2m",
            run: |s| bench_receiver(s, &ADSB, 2e6, 1.0),
        },
        Benchmark {
            name: "adsb.receiver_4m",
            run: |s| bench_receiver(s, &ADSB, 4e6, 1.0),
        },
        Benchmark {
            name: "adsb.receiver_10m",
            run: |s| bench_receiver(s, &ADSB, 10e6, 0.5),
        },
        Benchmark {
            name: "uat.receiver_2m",
            run: |s| bench_receiver(s, &UAT, 2.083334e6, 1.0),
        },
        Benchmark {
            name: "modeac.receiver_4m",
            run: |s| bench_receiver(s, &MODEAC, 4e6, 0.5),
        },
        Benchmark {
            name: "uplink1030.receiver_10m",
            run: |s| bench_receiver(s, &UPLINK1030, 10e6, 0.25),
        },
        Benchmark {
            name: "adsb.generator_4m",
            run: |s| {
                let mut g = generator::AdsbGenerator::airspace(
                    4e6,
                    generator::AirspaceOptions::default(),
                    1,
                );
                let mut x = vec![Complex32::new(0.0, 0.0); 400_000];
                sdrkit_core::bench::measure(s, x.len(), 0.1, || {
                    g.generate(&mut x);
                })
            },
        },
    ]
}
