//! DVB-T: the receiver, the test signal generator, the sender of a host's transport stream and the channel scan, for 2K
//! and 8K, every guard interval, constellation, code rate and hierarchy, and 5 to 8 MHz channels. The transport stream goes
//! to the layer of `sdrkit-proto-mpegts`. The stacks are [`DVBT`] (`"dvbt"`), [`DVB_TERRESTRIAL`] (`"dvb_terrestrial"`, DVB-T2 and DVB-T on
//! one frequency) and [`dvbsh::DVBSH`] (`"dvbsh"`, DVB-SH on the same OFDM chain).
//!
//! Standard: ETSI EN 300 744; ETSI EN 302 583 for DVB-SH.

#![warn(missing_docs)]

pub mod bandwidth;
pub mod coding;
pub mod dvbsh;
pub mod equaliser;
pub mod fecstage;
pub mod fft;
pub mod finder;
pub mod generator;
pub mod inner;
pub mod input;
pub mod items;
pub mod level;
pub mod outer;
pub mod params;
pub mod receiver;
pub mod report;
pub mod results;
pub mod scan;
pub mod settings;
pub mod sync;
pub mod tables;
pub mod terrestrial;
pub mod tps;
pub mod tpsdec;
pub mod transmit;
pub mod typed;

pub use generator::{DvbtGenerator, GenParams};
pub use params::{Bandwidth, CodeRate, Constellation, Guard, Hierarchy, Mode, TpsParams};
pub use settings::{DvbtSetting, TerrestrialSetting};
pub use terrestrial::TerrestrialReceiver;

use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockCondition, LockProfile, LockState, NameSource,
    Receiver, SettingSet, SignalType, StackConfig, StackDescriptor, Tuning, Value, Values,
};
use sdrkit_proto_mpegts::source::{NullSource, NumberedSource};
use settings::{
    GenSource, BANDWIDTH, GEN_BANDWIDTH, GEN_CELL_ID, GEN_CONSTELLATION, GEN_GUARD, GEN_HIERARCHY,
    GEN_LENGTH, GEN_MODE, GEN_NAME, GEN_PHASE, GEN_RATE_HP, GEN_RATE_LP, GEN_SERVICE, GEN_SOURCE,
};

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver::receiver(cfg)?))
}

fn make_terrestrial(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(TerrestrialReceiver::new(cfg)?))
}

/// The generator of the stack at `cfg.rate_hz`, from the `gen_*` settings of `cfg`. The width is that of
/// `gen_bandwidth`, else that of `bandwidth`, else 8 MHz.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let v = &cfg.settings;
    let tps = TpsParams {
        mode: v.get(&GEN_MODE),
        guard: v.get(&GEN_GUARD),
        constellation: v.get(&GEN_CONSTELLATION),
        hierarchy: v.get(&GEN_HIERARCHY),
        rate_hp: v.get(&GEN_RATE_HP),
        rate_lp: v.get(&GEN_RATE_LP),
        cell_id: v.get(&GEN_CELL_ID) as u16,
        length: v.get(&GEN_LENGTH).value(),
        dvbh: 0,
    };
    let source = v.get(&GEN_SOURCE);
    // The width of the test signal, else that of the stack, so that the signal locks with the settings of a region.
    let width = v
        .get(&GEN_BANDWIDTH)
        .bandwidth()
        .or_else(|| v.get(&BANDWIDTH).bandwidth())
        .unwrap_or(Bandwidth::B8);
    let mut p = GenParams::new(width, tps);
    p.puncture_phase = v.get(&GEN_PHASE) as usize;
    p.hp = match source {
        GenSource::Pattern => Box::new(NumberedSource::new(generator::PATTERN_HP)),
        GenSource::Null => Box::new(NullSource),
        GenSource::Builtin => generator::test_mux(
            v.get(&GEN_SERVICE) as u16,
            v.get(&GEN_NAME),
            tps.net_bitrate(width, false),
        ),
    };
    if tps.hierarchy.hierarchical() && source == GenSource::Pattern {
        p.lp = Box::new(NumberedSource::new(generator::PATTERN_LP));
    }
    Ok(Box::new(DvbtGenerator::new(p).at_rate(cfg.rate_hz)?))
}

const TUNING: Tuning = Tuning {
    min_hz: 47e6,
    max_hz: 1000e6,
    default_hz: 0.0,
    rate_hz: 64e6 / 7.0,
    min_rate_hz: 4.9375e6,
    bandwidth_hz: 8e6,
    tune_offset_hz: 0.0,
    carrier_at_centre: false,
    not_circular: false,
    radio_filter_hz: Some(8e6),
    no_offset_tuning: true,
    ..Tuning::DEFAULT
};

/// The lock profile of a TV multiplex for the scanner.
const LOCK_PROFILE: LockProfile = LockProfile {
    settle_s: 0.3,
    quiet_after_s: Some(2.0),
    signal_snr_db: 3.0,
    timeout_s: 8.0,
    lock: LockCondition::LockState {
        at_least: LockState::Locked,
    },
    collect_s: 4.0,
    name_from: &[NameSource::Summary],
    capture: &[],
};

/// The DVB-T stack.
pub static DVBT: StackDescriptor = StackDescriptor {
    id: receiver::ID,
    name: "DVB-T",
    group: "TV",
    blurb: "Digital terrestrial TV (DVB-T): channels, programme guide, teletext and subtitles. Transmits a transport stream that the host gives.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..TUNING
    },
    submodes: &[],
    settings: DvbtSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[SignalType::DvbT],
    auto_gain: AutoGain::Hold,
    verify: None,
    benchmarks: Some(benchmarks),
};

/// DVB-T2 and DVB-T on one frequency.
pub static DVB_TERRESTRIAL: StackDescriptor = StackDescriptor {
    id: terrestrial::ID,
    name: "DVB-T2 / DVB-T",
    group: "TV",
    blurb: "Digital terrestrial TV, DVB-T2 or DVB-T, whichever the multiplex is.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..TUNING
    },
    submodes: &[],
    settings: TerrestrialSetting::schema,
    make_receiver: make_terrestrial,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[SignalType::DvbT2, SignalType::DvbT],
    auto_gain: AutoGain::Hold,
    verify: None,
    benchmarks: None,
};

/// The stacks of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DVBT, &DVB_TERRESTRIAL, &dvbsh::DVBSH]
}

/// Adds the DVB-T components and those of the transport stream crate to a graph registry. Each component made here has
/// its own control link.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("dvbt.input", || Box::new(input::DvbtInput::default()));
    r.add("dvbt.finder", || Box::new(finder::DvbtFinder::default()));
    r.add("dvbt.sync", || Box::new(sync::DvbtSync::default()));
    r.add("dvbt.fft", || Box::new(fft::DvbtFft::new()));
    r.add("dvbt.tps", || Box::new(tpsdec::DvbtTps::default()));
    r.add("dvbt.equaliser", || {
        Box::new(equaliser::DvbtEqualiser::new())
    });
    r.add("dvbt.inner", || Box::new(fecstage::DvbtInner::new()));
    r.add("dvbt.outer", || Box::new(fecstage::DvbtOuter::default()));
    r.add("dvbt.fec", || Box::new(fecstage::DvbtFec::default()));
    r.add("dvbt.report", || Box::new(report::DvbtReport::default()));
    r.add("dvbsh.input", || {
        Box::new(dvbsh::rx::ShInput::new(
            items::Link::new(),
            Default::default(),
        ))
    });
    r.add("dvbsh.data", || Box::new(dvbsh::rx::ShData::new(items::Link::new())));
    r.add("dvbsh.tdm_front", || {
        Box::new(dvbsh::tdmrx::TdmFront::new(Default::default()))
    });
    r.add("dvbsh.tdm", || Box::new(dvbsh::tdmrx::TdmDemod::new(Default::default())));
    r.add("dvbsh.decode", || {
        Box::new(dvbsh::rx::ShDecode::new(
            items::Link::new(),
            Default::default(),
        ))
    });
    sdrkit_proto_mpegts::register(r);
}

/// The benchmarks of this crate: the whole receiver on one thread (items: input samples; the CPU share is of one core at
/// the rate of the input), and the sender. The receiver cases are 8K 64-QAM 3/4 in a 7 MHz channel at the rate of an 8 MHz
/// channel (the resampler runs) and at its own rate, 8K 64-QAM 3/4 and 2/3 in 8 MHz, and 2K 16-QAM 2/3 in 8 MHz, each at
/// 30 dB. The sender cases are 2K QPSK 1/2 in 6 MHz and 8K 64-QAM 2/3 in 8 MHz at the natural rate. The Viterbi decoder
/// and Reed-Solomon alone are benchmarked in `sdrkit-algo-fec`.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    use sdrkit_core::Benchmark;
    vec![
        Benchmark {
            name: "dvbt.receiver_8k_64qam_34_7mhz_at_9143k",
            run: |s| bench_receiver(s, &BENCH_7_MHZ, Bandwidth::B8.natural_rate()),
        },
        Benchmark {
            name: "dvbt.receiver_8k_64qam_34_7mhz",
            run: |s| bench_receiver(s, &BENCH_7_MHZ, Bandwidth::B7.natural_rate()),
        },
        Benchmark {
            name: "dvbt.receiver_8k_64qam_34_8mhz",
            run: |s| bench_receiver(s, &BENCH_8_MHZ_34, Bandwidth::B8.natural_rate()),
        },
        Benchmark {
            name: "dvbt.receiver_8k_64qam_23_8mhz",
            run: |s| bench_receiver(s, &BENCH_8_MHZ_23, Bandwidth::B8.natural_rate()),
        },
        Benchmark {
            name: "dvbt.receiver_2k_16qam_23_8mhz",
            run: |s| bench_receiver(s, &BENCH_2K, Bandwidth::B8.natural_rate()),
        },
        Benchmark {
            name: "dvbt.sender_2k_qpsk_12_6mhz",
            run: |s| bench_sender(s, &SEND_2K),
        },
        Benchmark {
            name: "dvbt.sender_8k_64qam_23_8mhz",
            run: |s| bench_sender(s, &SEND_8K),
        },
    ]
}

/// The sender of the 2K benchmark: 6 MHz, 2K, guard 1/8, QPSK 1/2.
const SEND_2K: transmit::TxConfig = transmit::TxConfig {
    bandwidth: Bandwidth::B6,
    mode: Mode::K2,
    guard: Guard::G8,
    constellation: Constellation::Qpsk,
    hierarchy: params::Hierarchy::None,
    rate_hp: CodeRate::R12,
    rate_lp: CodeRate::R12,
    cell_id: 0,
};
/// The sender of the 8K benchmark: 8 MHz, 8K, guard 1/8, 64-QAM 2/3.
const SEND_8K: transmit::TxConfig = transmit::TxConfig {
    bandwidth: Bandwidth::B8,
    mode: Mode::K8,
    constellation: Constellation::Qam64,
    rate_hp: CodeRate::R23,
    rate_lp: CodeRate::R23,
    ..SEND_2K
};

/// The sender of `cfg` at the natural rate of its width for four frames, the queue full of numbered packets (items: output
/// samples; the CPU share is of one core at that rate).
fn bench_sender(seconds: f64, cfg: &transmit::TxConfig) -> sdrkit_core::BenchResult {
    use sdrkit_proto_mpegts::source::PacketSource;
    let rate = cfg.bandwidth.natural_rate();
    let symbols = 4 * 68;
    let samples = symbols * cfg.tps().symbol_samples();
    let packets = (cfg.bit_rate(false) * samples as f64 / rate / 1504.0).ceil() as usize + 8;
    let mut src = NumberedSource::new(generator::PATTERN_HP);
    let mut bytes = Vec::with_capacity(packets * 188);
    let mut p = [0u8; 188];
    for _ in 0..packets.min(sdrkit_proto_mpegts::sendqueue::QUEUE_PACKETS) {
        src.next_packet(&mut p);
        bytes.extend_from_slice(&p);
    }
    let mut out = Vec::new();
    sdrkit_core::bench::measure(seconds, samples, samples as f64 / rate, || {
        let Ok(mut chain) = transmit::Chain::new(cfg, rate) else {
            return;
        };
        let mut hp = sdrkit_proto_mpegts::sendqueue::SendQueue::new(cfg.bit_rate(false));
        let mut lp = sdrkit_proto_mpegts::sendqueue::SendQueue::new(cfg.bit_rate(true));
        let _ = hp.append(&bytes);
        for _ in 0..symbols {
            chain.symbol(&mut hp, &mut lp, &mut out);
        }
    })
}

/// One benchmark signal: the channel width with its setting, the parameters and the OFDM frames.
struct BenchCase {
    bandwidth: Bandwidth,
    choice: settings::BandwidthChoice,
    mode: Mode,
    guard: Guard,
    constellation: Constellation,
    rate: CodeRate,
    frames: usize,
}

const BENCH_7_MHZ: BenchCase = BenchCase {
    bandwidth: Bandwidth::B7,
    choice: settings::BandwidthChoice::B7,
    mode: Mode::K8,
    guard: Guard::G16,
    constellation: Constellation::Qam64,
    rate: CodeRate::R34,
    frames: 24,
};
const BENCH_8_MHZ_34: BenchCase = BenchCase {
    bandwidth: Bandwidth::B8,
    choice: settings::BandwidthChoice::B8,
    guard: Guard::G8,
    ..BENCH_7_MHZ
};
const BENCH_8_MHZ_23: BenchCase = BenchCase {
    rate: CodeRate::R23,
    ..BENCH_8_MHZ_34
};
const BENCH_2K: BenchCase = BenchCase {
    mode: Mode::K2,
    constellation: Constellation::Qam16,
    frames: 96,
    ..BENCH_8_MHZ_23
};

/// The signal of `case` at `rate` with 30 dB of noise over the sample band.
fn bench_signal(case: &BenchCase, rate: f64) -> Vec<sdrkit_core::Complex32> {
    let tps = TpsParams {
        mode: case.mode,
        guard: case.guard,
        constellation: case.constellation,
        rate_hp: case.rate,
        rate_lp: case.rate,
        ..Default::default()
    };
    let mut g = DvbtGenerator::new(GenParams::new(case.bandwidth, tps));
    let mut sig = Vec::new();
    for _ in 0..case.frames * 68 {
        sig.extend_from_slice(g.next_symbol());
    }
    let natural = case.bandwidth.natural_rate();
    if (rate - natural).abs() > 1e-9 * natural {
        let mut at_rate = Vec::new();
        sdrkit_algo_dsp::resample::resample_span(&sig, natural, rate, &mut at_rate);
        sig = at_rate;
    }
    if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::with_power(1e-3, 0x1234_5678) {
        n.process(&mut sig);
    }
    sig
}

fn bench_receiver(seconds: f64, case: &BenchCase, rate: f64) -> sdrkit_core::BenchResult {
    use sdrkit_core::Choice;
    let sig = bench_signal(case, rate);
    let signal_s = sig.len() as f64 / rate;
    let schema = DvbtSetting::schema();
    let mut v = Values::defaults(&schema);
    // The value is a choice of the setting.
    let _ = v.set(
        DvbtSetting::Bandwidth.id(),
        &Value::Choice(case.choice.index()),
    );
    let cfg = StackConfig::new(v);
    sdrkit_core::bench::measure(seconds, sig.len(), signal_s, || {
        let Ok(mut rx) = receiver::receiver(&cfg) else {
            return;
        };
        let input = sdrkit_core::InputInfo::new(rate, 0.0, 0.0).recording();
        if rx.configure(&input).is_err() {
            return;
        }
        let mut out = sdrkit_core::Output::default();
        for c in sig.chunks(16384) {
            rx.feed(c, &mut out);
            out.clear();
        }
        rx.finish(&mut out);
    })
}
