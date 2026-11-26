//! Trunking, paging and tone signalling: MPT 1327, LTR, EDACS and Motorola Type II control channels, the tone decoders, POCSAG and FLEX pagers,
//! ERMES and GSC paging, a Multitone detector, an OpenSky identifier and SCADA telemetry (Modbus and DNP3). Every receiver is a graph of components in a [`StackGraph`].

#![warn(missing_docs)]

pub mod edacs;
pub mod ermes;
pub mod fmtx;
pub mod follow;
pub mod front;
pub mod grant;
pub mod gsc;
pub mod ltr;
pub mod mpt;
pub mod multitone;
pub mod opensky;
pub mod pager;
pub mod scada;
pub mod tones;
pub mod type2;
pub mod typed;
pub mod util;
pub mod verify;

use num_complex::Complex32;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, ConfigError, GenConfig, Generator, InputInfo, LockProfile,
    LockState, Output, Receiver, Results, SettingSet, SignalType, StackConfig, StackDescriptor,
    Tuning, Values,
};
use sdrkit_graph::{
    Component, ComponentInfo, Format, GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use std::fmt::Write as _;

/// The format of a typed stream that keeps the frequencies of `from`.
pub(crate) fn typed_format<T: Item>(from: &Format) -> Format {
    Format {
        rate: 0.0,
        frame_len: 0,
        media: None,
        ..from.with_kind(Kind::typed::<T>())
    }
}

/// A typed output port with no sample rate.
pub(crate) fn typed_port<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// Writes the status of `r`: the lock state, the level and the summary that `summary` returns, formatted into `scratch`. Nothing is
/// allocated once `scratch` and the summary have their size, and nothing changes when the status is the same.
pub(crate) fn set_status(
    r: &mut Results,
    lock_state: LockState,
    level_dbfs: Option<f32>,
    scratch: &mut String,
    summary: std::fmt::Arguments<'_>,
) {
    scratch.clear();
    // Writing into a `String` cannot fail.
    let _ = scratch.write_fmt(summary);
    r.update_status(|s| {
        let changed =
            s.lock_state != lock_state || s.level_dbfs != level_dbfs || s.summary != *scratch;
        if changed {
            s.lock_state = lock_state;
            s.level_dbfs = level_dbfs;
            s.summary.clone_from(scratch);
        }
        changed
    });
}

/// The receiver of `built`.
pub(crate) fn boxed(
    built: Result<StackGraph, GraphError>,
) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}

/// `trunk.real`: the real part of the input (audio given as samples, Q = 0).
#[derive(Default)]
pub struct RealPart;

impl Component for RealPart {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "trunk.real",
            vec![Port::iq("in")],
            vec![Port::real("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Real)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.real_out(0);
        out.extend(x.iter().map(|z| z.re));
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

/// `trunk.level`: the level of a real signal every 0.2 s, as [`front::Level`] items.
#[derive(Default)]
pub struct RealLevel {
    rate: f64,
    acc: f64,
    n: usize,
}

impl Component for RealLevel {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "trunk.level",
            vec![Port::real("in")],
            vec![typed_port::<front::Level>("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate;
        self.acc = 0.0;
        self.n = 0;
        Ok(vec![typed_format::<front::Level>(&inputs[0])])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let first = io.input(0).first();
        let every = (self.rate * 0.2).max(1.0) as usize;
        let (rate, mut acc, mut n) = (self.rate, self.acc, self.n);
        let out = io.items_out::<front::Level>(0);
        for (i, &v) in x.iter().enumerate() {
            acc += f64::from(v * v);
            n += 1;
            if n >= every {
                let p = acc / n as f64;
                out.push(Timed {
                    time: (first + i as u64) as f64 / rate,
                    value: front::Level {
                        dbfs: if p > 0.0 {
                            (10.0 * p.log10()) as f32
                        } else {
                            -200.0
                        },
                    },
                });
                acc = 0.0;
                n = 0;
            }
        }
        self.acc = acc;
        self.n = n;
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
}

#[allow(clippy::too_many_arguments)]
const fn tuning(
    min_mhz: f64,
    max_mhz: f64,
    default_hz: f64,
    rate_hz: f64,
    min_rate_hz: f64,
    bandwidth_hz: f64,
    tune_offset_hz: f64,
    radio_filter_hz: Option<f64>,
) -> Tuning {
    Tuning {
        min_hz: min_mhz * 1e6,
        max_hz: max_mhz * 1e6,
        default_hz,
        rate_hz,
        min_rate_hz,
        bandwidth_hz,
        tune_offset_hz,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    }
}

fn make_mpt(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(mpt::receiver(cfg))
}
fn make_mpt_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(mpt::gen::MptGenerator::from_config(cfg)))
}

/// MPT 1327 trunking.
pub static MPT1327: StackDescriptor = StackDescriptor {
    id: mpt::ID,
    name: "MPT1327 trunking",
    group: "Trunking",
    blurb: "MPT 1327 control channels: calls, acknowledgements and broadcasts, the system identity and the channel table.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(30.0, 520.0, 170e6, 240e3, 48e3, 12.5e3, 50e3, None)
    },
    submodes: &[],
    settings: mpt::MptSetting::schema,
    make_receiver: make_mpt,
    make_generator: Some(make_mpt_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Mpt1327],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MPT1327),
    benchmarks: Some(benchmarks),
};

fn make_ltr(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(ltr::receiver(cfg))
}
fn make_ltr_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ltr::gen::LtrGenerator::from_config(cfg)))
}

/// LTR trunking.
pub static LTR: StackDescriptor = StackDescriptor {
    id: ltr::ID,
    name: "LTR trunking",
    group: "Trunking",
    blurb: "LTR repeaters: the sub-audible 300 bit/s words with talkgroups, go-to and free repeaters, and the repeater table.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(29.7, 960.0, 460e6, 480e3, 240e3, 25e3, 50e3, Some(150e3))
    },
    submodes: &[],
    settings: ltr::LtrSetting::schema,
    make_receiver: make_ltr,
    make_generator: Some(make_ltr_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ltr],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LTR),
    benchmarks: None,
};

fn make_edacs(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(edacs::receiver(cfg))
}
fn make_edacs_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(edacs::gen::EdacsGenerator::from_config(cfg)))
}

/// EDACS trunking.
pub static EDACS: StackDescriptor = StackDescriptor {
    id: edacs::ID,
    name: "EDACS trunking",
    group: "Trunking",
    blurb: "EDACS control channels: calls, talkgroups and the site, a power scan of the voice channels, and following calls.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(29.7, 960.0, 856e6, 480e3, 307.2e3, 25e3, 100e3, Some(300e3))
    },
    submodes: &[],
    settings: edacs::EdacsSetting::schema,
    make_receiver: make_edacs,
    make_generator: Some(make_edacs_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Edacs],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::EDACS),
    benchmarks: None,
};

fn make_tones(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(tones::receiver(cfg))
}
fn make_tones_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(tones::gen::TonesGenerator::from_config(cfg)))
}

/// The tone decoders.
pub static TONES: StackDescriptor = StackDescriptor {
    id: tones::ID,
    name: "Tone decoders (CTCSS, DCS, DTMF, selcall)",
    group: "Tones and selective calling",
    blurb: "CTCSS tones, DCS codes, DTMF keys, sequential calls, SELCAL and two-tone pages, with a gate that opens for a chosen tone.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 12.5e3)),
        ..tuning(1.5, 1300.0, 145.5e6, 2e6, 500e3, 25e3, 50e3, Some(1.75e6))
    },
    submodes: &[],
    settings: tones::ToneSetting::schema,
    make_receiver: make_tones,
    make_generator: Some(make_tones_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::NarrowFmVoice, SignalType::KeyedVoice],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TONES),
    benchmarks: None,
};

fn make_pager(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(pager::receiver(cfg))
}
fn make_pager_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(pager::gen::PagerGenerator::from_config(cfg)))
}

/// Pagers.
pub static PAGER: StackDescriptor = StackDescriptor {
    id: pager::ID,
    name: "Pagers",
    group: "Paging",
    blurb: "POCSAG and FLEX pagers on one channel: addresses, numeric and text messages, with repeated copies combined.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(25.0, 1000.0, 466.075e6, 2e6, 1e6, 25e3, 50e3, Some(1.75e6))
    },
    submodes: &[],
    settings: pager::PagerSetting::schema,
    make_receiver: make_pager,
    make_generator: Some(make_pager_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Pocsag],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::PAGER),
    benchmarks: None,
};

fn make_multitone(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    boxed(multitone::receiver(cfg))
}
fn make_multitone_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(multitone::MultitoneGenerator::from_config(cfg)))
}

/// Multitone paging.
pub static MULTITONE: StackDescriptor = StackDescriptor {
    id: multitone::ID,
    name: "Multitone Paging",
    group: "Paging",
    blurb: "A detector of Multitone paging transmissions: carrier, deviation, tones and FSK measurements.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 12.5e3)),
        ..tuning(25.0, 480.0, 446.475e6, 2e6, 250e3, 25e3, 50e3, Some(1.75e6))
    },
    submodes: &[],
    settings: multitone::schema,
    make_receiver: make_multitone,
    make_generator: Some(make_multitone_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::MultitonePaging],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MULTITONE),
    benchmarks: None,
};

/// The frequency of the ERMES test signal, Hz. The session gives the frequency of a real channel.
pub const ERMES_TEST_SIGNAL_HZ: f64 = 170e6;

/// ERMES paging.
pub static ERMES: StackDescriptor = StackDescriptor {
    id: ermes::ID,
    name: "ERMES paging",
    group: "Paging",
    blurb: "ERMES pagers: 4-level FSK at 3125 symbols/s, the network identity, the initial addresses and the numeric and text messages of each batch.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(25.0, 1000.0, ERMES_TEST_SIGNAL_HZ, 250e3, ermes::MIN_RATE, 25e3, 50e3, None)
    },
    submodes: &[],
    settings: ermes::schema,
    make_receiver: ermes::make_receiver,
    make_generator: Some(ermes::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::ERMES),
    benchmarks: None,
};

/// The frequency of the Type II test signal, Hz. The session gives the frequency of a real control channel.
pub const TYPE2_TEST_SIGNAL_HZ: f64 = 850e6;

/// Motorola Type II trunking.
pub static MOTOROLA_TYPE2: StackDescriptor = StackDescriptor {
    id: type2::ID,
    name: "Motorola Type II trunking",
    group: "Trunking",
    blurb: "Motorola Type II control channels: talkgroups, units, sites, patches and voice grants, with a channel plan file from the host and following calls.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(29.7, 960.0, TYPE2_TEST_SIGNAL_HZ, 240e3, type2::MIN_RATE, 25e3, 100e3, None)
    },
    submodes: &[],
    settings: type2::schema,
    make_receiver: type2::make_receiver,
    make_generator: Some(type2::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::MOTOROLA_TYPE2),
    benchmarks: None,
};

/// The frequency of the OpenSky test signal, Hz. The session gives the frequency of a real channel.
pub const OPENSKY_TEST_SIGNAL_HZ: f64 = 850e6;

/// The OpenSky identifier.
pub static OPENSKY: StackDescriptor = StackDescriptor {
    id: opensky::ID,
    name: "OpenSky identifier",
    group: "Trunking",
    blurb: "Says whether an FM channel carries the OpenSky pattern (9600 baud 4-level FSK in TDMA bursts) from its symbol rate, levels, width and bursts; nothing is decoded.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(30.0, 1000.0, OPENSKY_TEST_SIGNAL_HZ, 240e3, opensky::MIN_RATE, 25e3, 50e3, None)
    },
    submodes: &[],
    settings: opensky::schema,
    make_receiver: opensky::make_receiver,
    make_generator: Some(opensky::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::OPENSKY),
    benchmarks: None,
};

/// The frequency of the GSC test signal, Hz. The session gives the frequency of a real channel.
pub const GSC_TEST_SIGNAL_HZ: f64 = 160e6;

/// Golay sequential code paging.
pub static GSC: StackDescriptor = StackDescriptor {
    id: gsc::ID,
    name: "GSC paging",
    group: "Paging",
    blurb: "Golay sequential code pagers: tone, numeric, alphanumeric and tone-and-voice pages, single calls and batches, with the pager code and its address function.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(26.0, 1000.0, GSC_TEST_SIGNAL_HZ, 250e3, gsc::MIN_RATE, 12.5e3, 50e3, None)
    },
    submodes: &[],
    settings: gsc::schema,
    make_receiver: gsc::make_receiver,
    make_generator: Some(gsc::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::GSC),
    benchmarks: None,
};

/// The frequency of the SCADA test signal, Hz. The session gives the frequency of a real channel.
pub const SCADA_TEST_SIGNAL_HZ: f64 = 450e6;

/// SCADA radio telemetry.
pub static SCADA: StackDescriptor = StackDescriptor {
    id: scada::ID,
    name: "SCADA telemetry",
    group: "Data and IoT",
    blurb: "Modbus RTU, Modbus ASCII and DNP3 over narrowband FSK radio links: the modem found by itself, the frames, polls paired with replies, and the devices with their registers and points.",
    tuning: sdrkit_core::Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        ..tuning(26.0, 1000.0, SCADA_TEST_SIGNAL_HZ, 250e3, scada::MIN_RATE, 40e3, 50e3, None)
    },
    submodes: &[],
    settings: scada::schema,
    make_receiver: scada::make_receiver,
    make_generator: Some(scada::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SCADA),
    benchmarks: None,
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &MPT1327,
        &LTR,
        &EDACS,
        &TONES,
        &PAGER,
        &MULTITONE,
        &ERMES,
        &MOTOROLA_TYPE2,
        &OPENSKY,
        &GSC,
        &SCADA,
    ]
}

fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let mut g = (desc.make_generator.expect("every stack has a generator"))(&GenConfig::new(
        rate,
        settings.clone(),
    ))
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    let configured = rx.configure(
        &InputInfo::new(rate, desc.tuning.default_hz, desc.tuning.default_hz).recording(),
    );
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        if configured.is_ok() {
            for c in x.chunks(65536) {
                rx.feed(c, &mut out);
                out.clear();
            }
        }
    })
}

/// Runs the receiver of `desc` at `rate` on `signal_s` of its own test signal, the channel where a radio tuned with the
/// stack's offset sees it.
fn bench_offset(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let mut g = (desc.make_generator.expect("every stack has a generator"))(&GenConfig::new(
        rate,
        settings.clone(),
    ))
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    let f = desc.tuning.default_hz;
    let configured =
        rx.configure(&InputInfo::new(rate, f + desc.tuning.tune_offset_hz, f).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        if configured.is_ok() {
            for c in x.chunks(65536) {
                rx.feed(c, &mut out);
                out.clear();
            }
        }
    })
}

/// Benchmarks of the receivers at the rates they ask for (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "mpt1327.receiver_240k",
            run: |s| bench_receiver(s, &MPT1327, 240e3, 2.0),
        },
        Benchmark {
            name: "ltr.receiver_480k",
            run: |s| bench_receiver(s, &LTR, 480e3, 1.0),
        },
        Benchmark {
            name: "edacs.receiver_480k",
            run: |s| bench_receiver(s, &EDACS, 480e3, 1.0),
        },
        Benchmark {
            name: "tones.receiver_2m",
            run: |s| bench_receiver(s, &TONES, 2e6, 0.5),
        },
        Benchmark {
            name: "pager.receiver_2m",
            run: |s| bench_receiver(s, &PAGER, 2e6, 0.5),
        },
        Benchmark {
            name: "multitone.receiver_2m",
            run: |s| bench_receiver(s, &MULTITONE, 2e6, 0.5),
        },
        Benchmark {
            name: "ermes.receiver_250k",
            run: |s| bench_offset(s, &ERMES, 250e3, 1.0),
        },
        Benchmark {
            name: "motorola_type2.receiver_240k",
            run: |s| bench_offset(s, &MOTOROLA_TYPE2, 240e3, 1.0),
        },
        Benchmark {
            name: "opensky.receiver_240k",
            run: |s| bench_offset(s, &OPENSKY, 240e3, 2.0),
        },
        Benchmark {
            name: "gsc.receiver_250k",
            run: |s| bench_offset(s, &GSC, 250e3, 4.0),
        },
        Benchmark {
            name: "scada.receiver_250k",
            run: |s| bench_offset(s, &SCADA, 250e3, 2.0),
        },
    ]
}
