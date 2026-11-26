//! Satellite navigation: the `gnss` stack (GPS L1 C/A, QZSS, SBAS, Galileo E1-B, BeiDou B1I and GLONASS L1OF: position,
//! velocity and time, the sky, interference and spoofing watch, NMEA and RINEX), the `sbas` stack (the messages of the SBAS
//! satellites, decoded and shown, never applied) and the `navic` stack (NavIC L5 SPS subframes, no position).
//!
//! Standard: IS-GPS-200, IS-QZSS-PNT, Galileo OS SIS ICD, BDS SIS ICD B1I, GLONASS ICD, RTCA DO-229, RINEX 3.04, NMEA 0183.

// DSP loops index several arrays in step; the index reads more plainly than zipped iterators there. Constructors that take a
// hit, a band position and the loop hints are clearer with their arguments named than wrapped in a struct.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

pub mod acq;
pub mod bds;
pub mod codes;
pub mod components;
pub mod core;
pub mod fec;
pub mod fixsolve;
pub mod frontend;
pub mod gen;
pub mod geo;
pub mod glo;
pub mod inav;
pub mod lnav;
pub mod monitor;
pub mod msg;
pub mod navic;
pub mod navstore;
pub mod orbit;
pub mod outputs;
pub mod pvt;
pub mod results;
pub mod sbas;
pub mod settings;
pub mod system;
pub mod track;
pub mod typed;
pub mod verify;

use components::{
    GnssCoreComponent, MonitorComponent, NmeaComponent, RinexComponent, SbasContentComponent,
};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, LockProfile,
    Receiver, SettingSet, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// Adds every component of the crate to `r`, each under the name its `info()` returns.
pub fn register(r: &mut sdrkit_graph::Registry) {
    components::register(r);
    navic::register(r);
}

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 2.046e6;

/// Builds the `gnss` graph:
///
/// ```text
/// IQ --+--> rx (gnss.receiver) --measurement, fix--> interference (gnss.interference)
///      |                       --fix--> nmea (gnss.nmea) --> events
///      |                       --measurement, ephemeris--> rinex (gnss.rinex) --> RINEX text
///      +--> interference
/// ```
///
/// # Errors
///
/// [`GraphError`] if the graph cannot be built.
pub fn gnss_graph() -> Result<Graph, GraphError> {
    gnss_builder().build()
}

fn gnss_builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("rx", GnssCoreComponent::new(false))
        .add("interference", MonitorComponent::default())
        .add("nmea", NmeaComponent)
        .add("rinex", RinexComponent::default())
        .input("rx.in")
        .input("interference.in")
        .connect("rx.measurement", "interference.measurement")
        .connect("rx.fix", "interference.fix")
        .connect("rx.fix", "nmea")
        .connect("rx.measurement", "rinex.measurement")
        .connect("rx.ephemeris", "rinex.ephemeris")
        .output("rx.events")
        .output("nmea")
        .output("rinex")
        .clock(0.25)
        .status_from("rx")
}

/// Builds the `sbas` graph: the receiver of SBAS alone, its messages into the content decoder.
///
/// # Errors
///
/// [`GraphError`] if the graph cannot be built.
pub fn sbas_graph() -> Result<Graph, GraphError> {
    sbas_builder().build()
}

fn sbas_builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("rx", GnssCoreComponent::new(true))
        .add("sbas", SbasContentComponent::default())
        .add("interference", MonitorComponent::default())
        .input("rx.in")
        .input("interference.in")
        .connect("rx.measurement", "interference.measurement")
        .connect("rx.fix", "interference.fix")
        .connect("rx.sbas", "sbas.messages")
        .connect("rx.channels", "sbas.channels")
        .output("rx.events")
        .clock(0.25)
        .status_from("sbas")
}

/// The receiver of the stack that `shape` describes: its graph, the minimum rate, and every receiver setting routed to `rx`.
fn receiver(
    shape: &sdrkit_core::interface::StackShape,
    graph: GraphBuilder,
    cfg: &StackConfig,
) -> Result<StackGraph, GraphError> {
    let schema = shape.schema();
    let mut b = StackGraph::of(shape, graph.catalog(&Catalog::of(cfg)?).build()?)
        .min_rate(MIN_RATE_HZ, "")
        .on_retune(sdrkit_graph::CommandTarget::of("rx", "reset"))?
        .command("reset", sdrkit_graph::CommandTarget::of("rx", "reset"));
    for s in schema.settings() {
        let key = s.field.key.as_ref();
        if key.starts_with("gen_") {
            continue;
        }
        // The host tunes the radio, so the tune reaches no component.
        if key == settings::TUNE.key() {
            b = b.route(key, &[])?;
        } else {
            b = b.route(key, &[&format!("rx.{key}")])?;
        }
    }
    b.with_settings(&cfg.settings).build()
}

/// The `gnss` receiver with these settings.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn gnss_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver(typed::shape(GNSS.id), gnss_builder(), cfg)
}

/// The `sbas` receiver with these settings.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn sbas_receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let shape = typed::shape(SBAS.id);
    let schema = shape.schema();
    let mut b = StackGraph::of(shape, sbas_builder().catalog(&Catalog::of(cfg)?).build()?)
        .min_rate(MIN_RATE_HZ, "")
        .on_retune(sdrkit_graph::CommandTarget::of("rx", "reset"))?
        .command("reset", sdrkit_graph::CommandTarget::of("rx", "reset"))
        .command("select", sdrkit_graph::CommandTarget::of("sbas", "select"))
        .command("clear", sdrkit_graph::CommandTarget::of("sbas", "clear"));
    for s in schema.settings() {
        let key = s.field.key.as_ref();
        b = b.route(key, &[&format!("rx.{key}")])?;
    }
    b.with_settings(&cfg.settings).build()
}

fn make_gnss(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(gnss_receiver(cfg)?))
}
fn make_sbas(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(sbas_receiver(cfg)?))
}

/// The generator of the `gnss` stack at the requested rate.
pub fn make_gnss_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let o = settings::gen_options(&cfg.settings, cfg.rate_hz, cfg.seed.max(1), false, None);
    Ok(Box::new(gen::GnssGenerator::new(o)))
}
/// The generator of the `sbas` stack: PRN 123, 127 and 128 with structured content.
pub fn make_sbas_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let o = settings::gen_options(&cfg.settings, cfg.rate_hz, cfg.seed.max(1), true, None);
    Ok(Box::new(gen::GnssGenerator::new(o)))
}

const TUNING: Tuning = Tuning {
    min_hz: 1550e6,
    max_hz: 1610e6,
    default_hz: 1575.42e6,
    rate_hz: 4e6,
    min_rate_hz: MIN_RATE_HZ,
    bandwidth_hz: 2.046e6,
    tune_offset_hz: 0.0,
    carrier_at_centre: false,
    not_circular: false,
    radio_filter_hz: Some(2.5e6),
    no_offset_tuning: true,
    ..Tuning::DEFAULT
};

/// GNSS: GPS, QZSS, SBAS, Galileo, BeiDou and GLONASS.
pub static GNSS: StackDescriptor = StackDescriptor {
    id: "gnss",
    name: "GNSS",
    group: "Satellite navigation",
    blurb: "GPS, Galileo, BeiDou, GLONASS, QZSS and SBAS: satellites tracked, position and time, the sky, interference and spoofing watch. Needs an active antenna (bias-tee on).",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Fixed),
        ..TUNING
    },
    submodes: &[],
    settings: settings::GnssSetting::schema,
    make_receiver: make_gnss,
    make_generator: Some(make_gnss_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Gnss],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::GNSS_VERIFY),
    benchmarks: Some(benchmarks),
};

/// SBAS: the messages of the augmentation satellites. The corrections are shown, never applied.
pub static SBAS: StackDescriptor = StackDescriptor {
    id: "sbas",
    name: "SBAS",
    group: "Satellite navigation",
    blurb: "WAAS, EGNOS, MSAS, GAGAN and other SBAS satellites: PRN masks, fast and long-term corrections, ionosphere, GEO navigation and time. Shown, never applied.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..TUNING
    },
    submodes: &[],
    settings: settings::SbasSetting::schema,
    make_receiver: make_sbas,
    make_generator: Some(make_sbas_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Sbas],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SBAS_VERIFY),
    benchmarks: None,
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&GNSS, &SBAS, &navic::DESCRIPTOR]
}

/// The receiver core on `signal_s` seconds of the test signal at `rate`, fed again and again: GPS, or with `sbas` the `sbas`
/// stack on the three SBAS satellites.
fn bench_receiver(seconds: f64, rate: f64, signal_s: f64, sbas: bool) -> BenchResult {
    let systems = if sbas {
        system::System::Sbas.bit()
    } else {
        system::System::Gps.bit()
    };
    let mut g = gen::GnssGenerator::new(gen::GenOptions {
        rate_hz: rate,
        systems,
        sbas_structured: sbas,
        ..Default::default()
    });
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let mut rx = core::GnssCore::new(core::CoreConfig {
        systems,
        sbas_mode: sbas,
        ..Default::default()
    });
    if rx.configure(rate, 1575.42e6, false).is_err() {
        return sdrkit_core::bench::measure(seconds, 0, signal_s, || {});
    }
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c);
        }
    })
}

/// Benchmarks of the receiver (input samples) and the generator.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "gnss.receiver_4m",
            run: |s| bench_receiver(s, 4e6, 1.0, false),
        },
        Benchmark {
            name: "gnss.sbas_receiver_4m",
            run: |s| bench_receiver(s, 4e6, 1.0, true),
        },
        Benchmark {
            name: "navic.receiver_4m",
            run: navic::bench_receiver,
        },
        Benchmark {
            name: "gnss.generator_4m",
            run: |s| {
                let mut g = gen::GnssGenerator::new(gen::GenOptions::default());
                let mut x = vec![Complex32::new(0.0, 0.0); 400_000];
                sdrkit_core::bench::measure(s, x.len(), 0.1, || {
                    g.generate(&mut x);
                })
            },
        },
    ]
}
