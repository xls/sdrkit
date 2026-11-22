//! The weak-signal digital stacks: FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144 and JS8, each with its decoder,
//! encoder, test signal, settings and results, and the combined stack `wsjt` that runs every enabled decoder on the same audio.
//! FST4, FST4W and MSK144 decode once a host loads their code tables ([`proto::codes`]); Q65 has its code built in
//! ([`proto::q65code`]).

#![warn(missing_docs)]

pub mod bands;
mod bench;
pub mod components;
pub mod encode;
pub mod front;
pub mod generator;
pub mod mode;
pub mod proto;
pub mod qso;
pub mod receiver;
pub mod runner;
pub mod rx;
pub mod scatter;
pub mod settings;
pub mod slots;
pub mod table;
pub mod typed;
pub mod verify;

pub use bench::{benchmarks, test_slot};
pub use mode::{Mode, Sub};

use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Schema, StackConfig,
    StackDescriptor, SubMode, Tuning,
};
use std::sync::Arc;

const GROUP: &str = "Weak-signal digital";

/// The tuning of the family: any frequency (the region data gives the usual dials), 2 Msps requested, a 1.75 MHz
/// analogue filter, the radio 20 kHz above the dial.
const fn tuning(default_hz: f64) -> Tuning {
    Tuning {
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(default_hz, 2e6, front::MIN_RATE, 3500.0)
    }
}

macro_rules! stack_fns {
    ($rx:ident, $gen:ident, $schema:ident, $m:expr) => {
        fn $rx(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
            Ok(Box::new(receiver::receiver($m, cfg)?))
        }
        fn $gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
            Ok(generator::WsjtGenerator::from_config(
                $m.unwrap_or(Mode::Ft8),
                cfg,
            ))
        }
        fn $schema() -> Arc<Schema> {
            settings::schema_for($m)
        }
    };
}

stack_fns!(rx_wsjt, gen_wsjt, schema_wsjt, None);
stack_fns!(rx_ft8, gen_ft8, schema_ft8, Some(Mode::Ft8));
stack_fns!(rx_ft4, gen_ft4, schema_ft4, Some(Mode::Ft4));
stack_fns!(rx_ft2, gen_ft2, schema_ft2, Some(Mode::Ft2));
stack_fns!(rx_wspr, gen_wspr, schema_wspr, Some(Mode::Wspr));
stack_fns!(rx_jt65, gen_jt65, schema_jt65, Some(Mode::Jt65));
stack_fns!(rx_jt9, gen_jt9, schema_jt9, Some(Mode::Jt9));
stack_fns!(rx_jt4, gen_jt4, schema_jt4, Some(Mode::Jt4));
stack_fns!(rx_q65, gen_q65, schema_q65, Some(Mode::Q65));
stack_fns!(rx_fst4, gen_fst4, schema_fst4, Some(Mode::Fst4));
stack_fns!(rx_fst4w, gen_fst4w, schema_fst4w, Some(Mode::Fst4w));
stack_fns!(rx_msk144, gen_msk144, schema_msk144, Some(Mode::Msk144));
stack_fns!(rx_js8, gen_js8, schema_js8, Some(Mode::Js8));

macro_rules! descriptor {
    ($name:ident, $id:expr, $title:expr, $blurb:expr, $hz:expr, $subs:expr, $rx:ident, $gen:ident, $schema:ident, $verify:expr, $bench:expr, $types:expr) => {
        #[doc = concat!("The ", $title, " stack.")]
        pub static $name: StackDescriptor = StackDescriptor {
            id: $id,
            name: $title,
            group: GROUP,
            blurb: $blurb,
            tuning: tuning($hz),
            submodes: $subs,
            settings: $schema,
            make_receiver: $rx,
            make_generator: Some($gen),
            lock_profile: LockProfile::DEFAULT,
            signal_types: $types,
            auto_gain: AutoGain::Radio,
            verify: Some($verify),
            benchmarks: $bench,
        };
    };
}

descriptor!(WSJT, "wsjt", "Weak-signal (all enabled)", "Every enabled weak-signal decoder on the same audio: FT8, FT4, FT2, WSPR, JT65, JT9, JT4, Q65, FST4, FST4W, MSK144 and JS8.", 0.0, &[], rx_wsjt, gen_wsjt, schema_wsjt, &verify::COMBINED, Some(benchmarks), &[SignalType::Ft8, SignalType::Ft4, SignalType::Ft2, SignalType::Wspr, SignalType::Jt65, SignalType::Jt9, SignalType::Jt4, SignalType::Q65, SignalType::Fst4, SignalType::Fst4w, SignalType::Msk144, SignalType::Js8]);
descriptor!(
    FT8,
    "ft8",
    "FT8",
    "The 15-second weak-signal mode of HF and 6 m contacts.",
    0.0,
    &[],
    rx_ft8,
    gen_ft8,
    schema_ft8,
    &verify::FAST,
    None,
    &[SignalType::Ft8]
);
descriptor!(
    FT4,
    "ft4",
    "FT4",
    "The 7.5-second contest mode.",
    0.0,
    &[],
    rx_ft4,
    gen_ft4,
    schema_ft4,
    &verify::FAST,
    None,
    &[SignalType::Ft4]
);
descriptor!(
    FT2,
    "ft2",
    "FT2",
    "FT4 at twice the speed, in 3.75-second slots.",
    0.0,
    &[],
    rx_ft2,
    gen_ft2,
    schema_ft2,
    &verify::FAST,
    None,
    &[SignalType::Ft2]
);
descriptor!(
    WSPR,
    "wspr",
    "WSPR",
    "Beacons of call, locator and power in 2-minute slots.",
    0.0,
    &[],
    rx_wspr,
    gen_wspr,
    schema_wspr,
    &verify::SLOW,
    None,
    &[SignalType::Wspr]
);
descriptor!(
    JT65,
    "jt65",
    "JT65",
    "One-minute contacts with Reed-Solomon coding (HF and EME).",
    0.0,
    &[
        SubMode {
            id: "jt65a",
            name: "JT65A",
            default_hz: 0.0
        },
        SubMode {
            id: "jt65b",
            name: "JT65B",
            default_hz: 0.0
        },
        SubMode {
            id: "jt65c",
            name: "JT65C",
            default_hz: 0.0
        },
    ],
    rx_jt65,
    gen_jt65,
    schema_jt65,
    &verify::SLOW,
    None,
    &[SignalType::Jt65]
);
descriptor!(
    JT9,
    "jt9",
    "JT9",
    "One-minute contacts in 16 Hz.",
    0.0,
    &[],
    rx_jt9,
    gen_jt9,
    schema_jt9,
    &verify::SLOW,
    None,
    &[SignalType::Jt9]
);
descriptor!(
    JT4,
    "jt4",
    "JT4",
    "One-minute contacts for EME and microwave weak signals.",
    0.0,
    &[
        SubMode {
            id: "jt4a",
            name: "JT4A",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4b",
            name: "JT4B",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4c",
            name: "JT4C",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4d",
            name: "JT4D",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4e",
            name: "JT4E",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4f",
            name: "JT4F",
            default_hz: 0.0
        },
        SubMode {
            id: "jt4g",
            name: "JT4G",
            default_hz: 0.0
        },
    ],
    rx_jt4,
    gen_jt4,
    schema_jt4,
    &verify::SLOW,
    None,
    &[SignalType::Jt4]
);
descriptor!(
    Q65,
    "q65",
    "Q65",
    "Weak-signal contacts by ionoscatter, troposcatter and EME.",
    0.0,
    &[
        SubMode { id: "q65-60a", name: "Q65-60A", default_hz: 0.0 },
        SubMode { id: "q65-30a", name: "Q65-30A", default_hz: 0.0 },
        SubMode { id: "q65-120e", name: "Q65-120E", default_hz: 0.0 },
    ],
    rx_q65,
    gen_q65,
    schema_q65, &verify::SLOW, None, &[SignalType::Q65]);
descriptor!(
    FST4,
    "fst4",
    "FST4",
    "Slow weak-signal contacts on LF to HF, 15 to 1800-second periods (decodes once its code table is loaded).",
    0.0,
    &[],
    rx_fst4,
    gen_fst4,
    schema_fst4, &verify::REFUSED, None, &[SignalType::Fst4]);
descriptor!(
    FST4W,
    "fst4w",
    "FST4W",
    "WSPR-style beacons in FST4, 15 to 1800-second periods (decodes once its code table is loaded).",
    0.0,
    &[],
    rx_fst4w,
    gen_fst4w,
    schema_fst4w, &verify::REFUSED, None, &[SignalType::Fst4w]);
descriptor!(
    MSK144,
    "msk144",
    "MSK144",
    "Meteor scatter: 72 ms frames at 2000 bit/s (decodes once its code table is loaded).",
    0.0,
    &[],
    rx_msk144,
    gen_msk144,
    schema_msk144,
    &verify::REFUSED,
    None,
    &[SignalType::Msk144]
);
descriptor!(
    JS8,
    "js8",
    "JS8",
    "Keyboard chat in FT8-style frames: heartbeats, directed messages and free text.",
    0.0,
    &[
        SubMode {
            id: "js8normal",
            name: "JS8 Normal",
            default_hz: 0.0
        },
        SubMode {
            id: "js8fast",
            name: "JS8 Fast",
            default_hz: 0.0
        },
        SubMode {
            id: "js8turbo",
            name: "JS8 Turbo",
            default_hz: 0.0
        },
    ],
    rx_js8,
    gen_js8,
    schema_js8,
    &verify::FAST,
    None,
    &[SignalType::Js8]
);

/// The dial of the test signals of FSK441, JT6M and ISCAT. It is not a calling frequency: the host gives the frequency.
pub const SCATTER_TEST_SIGNAL_HZ: f64 = 144.0e6;

/// Runs the receiver of `desc` over `signal_s` seconds of its own test signal at 48 ksps (items are input samples).
fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    signal_s: f64,
) -> sdrkit_core::BenchResult {
    use sdrkit_core::{Complex32, InputInfo, Output, Values};
    let settings = Values::defaults(&(desc.settings)());
    let rate = 48_000.0;
    let Some(Ok(mut g)) = desc
        .make_generator
        .map(|make| make(&GenConfig::new(rate, settings.clone()).with_seed(1)))
    else {
        return sdrkit_core::BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return sdrkit_core::BenchResult::default();
    };
    let dial = desc.tuning.default_hz;
    if rx
        .configure(&InputInfo::new(rate, dial + desc.tuning.tune_offset_hz, dial).recording())
        .is_err()
    {
        return sdrkit_core::BenchResult::default();
    }
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(4800) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

fn fsk441_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "fsk441.receiver_48k",
        run: |s| bench_receiver(s, &FSK441, 24.0),
    }]
}

fn jt6m_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "jt6m.receiver_48k",
        run: |s| bench_receiver(s, &JT6M, 24.0),
    }]
}

fn gen_jt6m(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    scatter::jt6m::generator(cfg)
}

/// The dial of the test signal of JT6M. It is not a calling frequency: the host gives the frequency.
pub const JT6M_TEST_SIGNAL_HZ: f64 = 50.0e6;

/// JT6M.
pub static JT6M: StackDescriptor = StackDescriptor {
    id: "jt6m",
    name: "JT6M",
    group: GROUP,
    blurb: "6 m scatter: 44-FSK at 21.5 baud with a sync tone every third symbol; pings read, a long signal folded into its message.",
    tuning: tuning(JT6M_TEST_SIGNAL_HZ),
    submodes: &[],
    settings: scatter::jt6m::schema,
    make_receiver: scatter::jt6m::make_receiver,
    make_generator: Some(gen_jt6m),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SCATTER),
    benchmarks: Some(jt6m_benchmarks),
};

fn iscat_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "iscat.receiver_48k",
        run: |s| bench_receiver(s, &ISCAT, 24.0),
    }]
}

fn gen_iscat(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    scatter::iscat::generator(cfg)
}

/// ISCAT.
pub static ISCAT: StackDescriptor = StackDescriptor {
    id: "iscat",
    name: "ISCAT",
    group: GROUP,
    blurb: "Scatter: 42-FSK in 24-symbol frames with Costas sync and length symbols; the message folded over all frames.",
    tuning: tuning(SCATTER_TEST_SIGNAL_HZ),
    submodes: &[
        SubMode {
            id: "auto",
            name: "ISCAT (A or B)",
            default_hz: SCATTER_TEST_SIGNAL_HZ,
        },
        SubMode {
            id: "a",
            name: "ISCAT-A",
            default_hz: SCATTER_TEST_SIGNAL_HZ,
        },
        SubMode {
            id: "b",
            name: "ISCAT-B",
            default_hz: SCATTER_TEST_SIGNAL_HZ,
        },
    ],
    settings: scatter::iscat::schema,
    make_receiver: scatter::iscat::make_receiver,
    make_generator: Some(gen_iscat),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SCATTER),
    benchmarks: Some(iscat_benchmarks),
};

fn pi4_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "pi4.receiver_48k",
        run: |s| bench_receiver(s, &PI4, 60.0),
    }]
}

fn gen_pi4(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    scatter::pi4::generator(cfg)
}

/// The dial of the test signal of PI4. It is not the frequency of a beacon: the host gives the frequency.
pub const PI4_TEST_SIGNAL_HZ: f64 = 1300.0e6;

/// PI4.
pub static PI4: StackDescriptor = StackDescriptor {
    id: "pi4",
    name: "PI4",
    group: GROUP,
    blurb: "Beacons: 4-FSK, 146 symbols of 1/6 s with a K = 32 code, 8 characters each minute before the CW identification.",
    tuning: tuning(PI4_TEST_SIGNAL_HZ),
    submodes: &[],
    settings: scatter::pi4::schema,
    make_receiver: scatter::pi4::make_receiver,
    make_generator: Some(gen_pi4),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SCATTER),
    benchmarks: Some(pi4_benchmarks),
};

fn gen_fsk441(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    scatter::fsk441::generator(cfg)
}

/// FSK441.
pub static FSK441: StackDescriptor = StackDescriptor {
    id: "fsk441",
    name: "FSK441",
    group: GROUP,
    blurb: "Meteor scatter pings: 4-FSK at 441 baud, three tones a character, read burst by burst.",
    tuning: tuning(SCATTER_TEST_SIGNAL_HZ),
    submodes: &[],
    settings: scatter::fsk441::schema,
    make_receiver: scatter::fsk441::make_receiver,
    make_generator: Some(gen_fsk441),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::SCATTER),
    benchmarks: Some(fsk441_benchmarks),
};

/// Every stack of the family.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &WSJT, &FT8, &FT4, &FT2, &WSPR, &JT65, &JT9, &JT4, &Q65, &FST4, &FST4W, &MSK144, &JS8,
        &FSK441, &JT6M, &ISCAT, &PI4,
    ]
}
