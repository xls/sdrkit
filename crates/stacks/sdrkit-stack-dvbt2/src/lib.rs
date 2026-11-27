//! DVB-T2: the receiver, the test signal generator, the sender of a host's transport stream and the parameter tables. The
//! receiver takes T2-base and T2-Lite; the transport stream of the chosen PLP goes to the layer of `sdrkit-proto-mpegts`.
//! The sender ([`transmit`]) sends one data PLP in T2-base. The stack is [`DVBT2`] (`"dvbt2"`).
//!
//! Standard: ETSI EN 302 755 V1.4.1.

#![warn(missing_docs)]

pub mod bicm;
pub mod error;
pub mod fec;
pub mod frame;
pub mod generator;
pub mod graph;
pub mod interleave;
pub mod l1;
pub mod p1;
pub mod params;
pub mod pilots;
pub mod results;
pub mod rx;
pub mod settings;
pub mod tables;
pub mod transmit;
pub mod tx;
pub mod typed;

pub use generator::T2Generator;
pub use rx::{RxSettings, RxStatus, T2Receiver};
pub use settings::Dvbt2Setting;

use graph::{FecComp, RxComp};
use params::Bandwidth;
use results::TS_LAYER;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    SettingId, SettingSet, SignalType, StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_core::owner::Owner;
use sdrkit_graph::components::TxSink;
use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, GraphError, StackGraph, Stage};
use transmit::{Dvbt2Transmit, TxSetting, COMMANDS};
use sdrkit_proto_ipcast::components::{BbFrameAdapt, SystemChoice};
use sdrkit_proto_mpegts::layer;
use settings::{BANDWIDTH, PLP};

/// The stack id.
pub const ID: &str = "dvbt2";
/// The recorded cost of the stages, per variant: the FFT size, the constellation and the code rate of the PLP.
const COSTS: &str = include_str!("../cost.txt");
/// The variant whose cost the graph declares before it knows the multiplex: the heaviest one.
pub const COST_VARIANT: &str = "32k-256qam-2/3";

/// The values of `cfg` on the schema of this stack. A setting of `cfg` that has the key of one of these settings and a value
/// that it takes is kept: a choice is matched by its key. Every other setting keeps its default. This lets the
/// `dvb_terrestrial` stack, whose schema differs, make this receiver.
pub fn values_of_config(cfg: &StackConfig) -> Values {
    let schema = Dvbt2Setting::schema();
    let mut out = Values::defaults(&schema);
    let given = &cfg.settings;
    for (i, d) in given.schema().settings().iter().enumerate() {
        let Ok(id) = schema.id(&d.field.key) else {
            continue;
        };
        let Some(v) = given.value(SettingId(i as u16)) else {
            continue;
        };
        let mine = match v {
            Value::Choice(c) => {
                let key = d.field.choices.get(usize::from(*c)).map(|c| &c.key);
                let to = schema.setting(id).and_then(|s| {
                    key.and_then(|k| s.field.choices.iter().position(|c| c.key == *k))
                });
                match to.and_then(|t| u16::try_from(t).ok()) {
                    Some(t) => Value::Choice(t),
                    None => continue,
                }
            }
            v => v.clone(),
        };
        // A value that this schema refuses keeps the default.
        let _ = out.set(id, &mine);
    }
    out
}

/// The receiver graph: `dvbt2.receiver` and the mode adaptation `ipcast.bbframe`, which puts the common PLP back into the
/// transport stream of the data PLP, with the transport stream layer after them.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph(set: RxSettings) -> Result<sdrkit_graph::Graph, GraphError> {
    builder(set).build()
}

/// The DVB-T2 graph. The cells of whole Interleaving Frames cross into the stage `fec`, whole BBFRAMEs into the
/// stage `ts`.
fn builder(set: RxSettings) -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("rx", RxComp::new(set))
        .add("fec", FecComp::new(set.max_iter))
        .add("ts", BbFrameAdapt::default())
        .input("rx.in")
        .connect("rx.cells", "fec.cells")
        .connect("rx.state", "fec.state")
        .connect("fec.bbframes", "ts.in")
        .connect("fec.plps", "ts.plps")
        .set("ts.system", Value::Choice(SystemChoice::DvbT2.index()))
        .status_from("fec");
    layer::attach(b, "ts.ts", &TS_LAYER)
        .first_stage("ofdm")
        .stage("rx", "fec", Stage::new("fec"))
        .stage("fec", "ts", Stage::new("ts").ahead_bytes(1 << 20))
        .costs(&sdrkit_core::CostTable::parse(COSTS), ID, COST_VARIANT)
}

/// The DVB-T2 stack with the settings of `cfg` (see [`values_of_config`]): the receiver, and the sender (`transmit`) on the
/// session's samples into the transmit sink (`tx`).
///
/// # Errors
///
/// [`GraphError::NamedSetting`] if a component refuses a value of `cfg`, or another [`GraphError`] if the graph does not build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build(cfg, true)
}

/// The graph of the stack with the values of `cfg`; with `transmit`, with the sender and its settings and commands.
fn build(cfg: &StackConfig, transmit: bool) -> Result<StackGraph, GraphError> {
    let values = values_of_config(cfg);
    let bandwidth = values.get(&BANDWIDTH);
    let set = RxSettings {
        bandwidth,
        // -1 (and only -1, by the range of the setting) takes the first data PLP.
        plp: u8::try_from(values.get(&PLP)).ok(),
        ..RxSettings::default()
    };
    let mut g = builder(set).catalog(&Catalog::of(cfg)?);
    if transmit {
        g = g
            .add("transmit", Dvbt2Transmit::default())
            .add("tx", TxSink::default())
            .input("transmit.in")
            .connect("transmit.out", "tx.in");
    }
    let g = g.build()?;
    let layer_commands: Vec<String> = Owner::commands(&g)
        .defs()
        .iter()
        .map(|d| d.key.to_string())
        .filter(|k| !COMMANDS.contains(&k.as_str()))
        .collect();
    let shape = if transmit {
        typed::shape(ID)
    } else {
        typed::receiver_shape()
    };
    // The occupied band (extended carriers) is 7.77 MHz of an 8 MHz channel.
    let mut b = StackGraph::of(shape, g)
        .min_rate(bandwidth.hz() * 0.98, "")
        .route("bandwidth", &["rx.bandwidth"])?
        .route("plp", &["rx.plp"])?
        .route(layer::DECODE_VIDEO_ROUTE.0, layer::DECODE_VIDEO_ROUTE.1)?;
    for s in TxSetting::schema().settings() {
        let key = &s.field.key;
        // Without the sender its settings have no target.
        b = if transmit {
            b.route(key, &[&format!("transmit.{key}")])?
        } else {
            b.route(key, &[])?
        };
    }
    if transmit {
        // Once the stack names a command it names all: the commands of the layer first, then those of the sender.
        for key in layer_commands {
            b = b.command(&key, CommandTarget::every(&key));
        }
        for c in COMMANDS {
            b = b.command(c, CommandTarget::of("transmit", c));
        }
    }
    b.with_settings(&values).build()
}

/// Makes the DVB-T2 receiver without the sender. Its signature fits the DVB-T2 factory of the `dvb_terrestrial` stack.
///
/// # Errors
///
/// [`ConfigError`] if the graph does not build or a component refuses a value of `cfg`.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg, false)?))
}

/// Makes the DVB-T2 stack: the receiver and the sender.
fn make_stack(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The generator of the stack, from its settings.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(T2Generator::from_config(cfg)))
}

/// Whether `x` (at the elementary period of a channel width) holds a DVB-T2 P1 symbol: a quick test that distinguishes
/// DVB-T2 from DVB-T, which has none. A T2-frame lasts at most 250 ms, so pass at least that much signal.
pub fn p1_present(x: &[Complex32]) -> bool {
    let mut d = p1::P1Detector::new(0.4, 60);
    let mut found = false;
    d.process(x, |f| found |= f.info.is_some_and(|i| i.is_t2()));
    found
}

const TUNING: Tuning = Tuning {
    min_hz: 47e6,
    max_hz: 1000e6,
    default_hz: 0.0,
    rate_hz: 64e6 / 7.0,
    min_rate_hz: 7.84e6,
    bandwidth_hz: 8e6,
    tune_offset_hz: 0.0,
    carrier_at_centre: false,
    not_circular: false,
    radio_filter_hz: Some(8e6),
    no_offset_tuning: true,
    ..Tuning::DEFAULT
};

/// The DVB-T2 stack.
pub static DVBT2: StackDescriptor = StackDescriptor {
    id: ID,
    name: "DVB-T2",
    group: "TV",
    blurb: "Digital terrestrial TV (DVB-T2 and T2-Lite): PLPs, channels, programme guide and subtitles. Transmits a transport stream that the host gives.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..TUNING
    },
    submodes: &[],
    settings: Dvbt2Setting::schema,
    make_receiver: make_stack,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::DvbT2],
    auto_gain: AutoGain::Hold,
    verify: None,
    benchmarks: Some(benchmarks),
};

/// The stacks of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DVBT2]
}

/// Adds the DVB-T2 component and those of the IP over broadcast and transport stream crates to a graph registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("dvbt2.receiver", || Box::new(RxComp::default()));
    r.add("dvbt2.fec", || Box::new(FecComp::default()));
    r.add(transmit::ID, || Box::new(Dvbt2Transmit::default()));
    sdrkit_proto_ipcast::register(r);
    sdrkit_proto_mpegts::register(r);
}

/// The benchmarks of this crate: the receiver on 8K 1/8 PP3 64-QAM 2/3 with 64 800-bit frames (items: input samples), and
/// the sender with its defaults in 8 MHz and with 2K QPSK 1/2 in 1.7 MHz, each at its elementary rate (items: output
/// samples).
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::Benchmark;
    vec![
        Benchmark {
            name: "dvbt2.receiver_8k_64qam_23",
            run: bench_receiver,
        },
        Benchmark {
            name: "dvbt2.sender_8k_64qam_23_8mhz",
            run: |s| transmit::bench_sender(s, &transmit::bench_default(), 2).0,
        },
        Benchmark {
            name: "dvbt2.sender_2k_qpsk_12_1700khz",
            run: |s| transmit::bench_sender(s, &transmit::bench_narrow(), 4).0,
        },
    ]
}

fn bench_receiver(seconds: f64) -> sdrkit_core::BenchResult {
    use params::{CodeRate, FecFrame, Fft, Guard, Modulation, PilotPattern, Profile};
    let g = settings::GenSettings {
        bandwidth: Bandwidth::B8,
        fft: Fft::K8,
        guard: Guard::G1_8,
        pp: PilotPattern::Pp3,
        extended: false,
        coding: bicm::Coding {
            frame: FecFrame::Normal,
            rate: CodeRate::R2_3,
            modulation: Modulation::Qam64,
            rotated: true,
        },
        profile: Profile::Base,
        miso: false,
        blocks: 4,
        pattern: true,
    };
    let src = Box::new(sdrkit_proto_mpegts::source::NumberedSource::new(
        generator::PATTERN,
    ));
    let Ok(mut tx) = tx::Transmitter::new(generator::tx_config(&g, src)) else {
        return sdrkit_core::BenchResult::default();
    };
    let mut sig = Vec::new();
    for _ in 0..4 {
        if let Ok(f) = tx.frame() {
            sig.extend(f);
        }
    }
    let rate = Bandwidth::B8.rate();
    let signal_s = sig.len() as f64 / rate;
    sdrkit_core::bench::measure(seconds, sig.len(), signal_s, || {
        let mut rx = T2Receiver::new(RxSettings::default());
        for c in sig.chunks(16_384) {
            rx.feed(c, &mut |_| {});
        }
    })
}
