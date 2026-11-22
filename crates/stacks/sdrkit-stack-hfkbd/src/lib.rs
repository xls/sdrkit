//! HF keyboard, teleprinter, Morse, ARQ and picture stacks: RTTY, PSK31 and its variants, CW, Olivia, Contestia, MFSK
//! (with pictures), DominoEX, THOR, MT63, FSQ, IFKP, Hellschreiber, SSTV, SITOR-B, ARDOP, ROS (identifier) and WINMOR
//! (detect only).
//!
//! Every stack is a graph of the family front end ([`front::HfFront`]) and the stack's decoder, made into a receiver by
//! [`decoder::receiver`]. [`descriptors`] lists the stacks.

#![warn(missing_docs)]
// Signal processing here walks several arrays by one index (tones, carriers, bits). Indexing says that plainly.
#![allow(clippy::needless_range_loop)]
// Tables of codes are tuples in maps: names for each part do not make them clearer.
#![allow(clippy::type_complexity)]

pub mod ardop;
pub mod auto;
pub mod cp16;
pub mod cw;
pub mod decoder;
pub mod domino;
pub mod fec;
pub mod finder;
pub mod front;
pub mod fsq;
pub mod hell;
pub mod kinds;
pub mod mfsk;
pub mod mt63;
pub mod olivia;
pub mod picture;
pub mod psk;
pub mod qrss;
pub mod ros;
pub mod rtty;
pub mod sender;
pub mod sitor;
pub mod sstv;
pub mod throb;
pub mod typed;
pub mod util;
pub mod verify;
pub mod winmor;

use decoder::{receiver, InputPort, RxOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, LockProfile, Receiver, Schema, StackConfig, StackDescriptor, SubMode,
    Tuning,
};
use sdrkit_graph::Registry;
use std::sync::{Arc, OnceLock};

/// The heading of this family in the stack list.
pub const GROUP: &str = "HF and utility";

/// The tuning every stack of the family shares: any frequency (the region data gives the usual dials), 2 Msps requested, a
/// 1.75 MHz analogue filter, the radio 20 kHz above the dial, at least 12 ksps.
pub const fn tuning(default_mhz: f64, width_hz: f64) -> Tuning {
    Tuning {
        tune_offset_hz: 20e3,
        radio_filter_hz: Some(1.75e6),
        ..Tuning::anywhere(default_mhz * 1e6, 2e6, front::MIN_RATE, width_hz)
    }
}

const fn opts(id: &'static str, input: InputPort, demod: bool) -> RxOptions {
    RxOptions {
        id,
        input,
        demod,
        sideband_only: false,
        listen: true,
        span: false,
        pictures: false,
        outputs: &[],
        sender: None,
    }
}

/// Makes the settings function and the receiver function of a stack. The settings come from the description.
macro_rules! stack_fns {
    ($schema:ident, $make:ident, $opt:ident, $dec:expr) => {
        fn $schema() -> Arc<Schema> {
            static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
            typed::schema(&CELL, $opt.id)
        }
        fn $make(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
            receiver($dec, $opt, cfg)
        }
    };
}

pub(crate) const RTTY_RX: RxOptions = RxOptions {
    sender: Some(rtty::sender),
    ..opts("rtty", InputPort::CAudio, true)
};
stack_fns!(rtty_schema, make_rtty, RTTY_RX, rtty::RttyDecoder::new());

/// RTTY.
pub static RTTY: StackDescriptor = StackDescriptor {
    id: "rtty",
    name: "RTTY",
    group: GROUP,
    blurb: "Radioteletype: ITA2 text sent by two-tone frequency shift keying.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: rtty_schema,
    make_receiver: make_rtty,
    make_generator: Some(rtty::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Rtty],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const SITOR_RX: RxOptions = opts("sitor", InputPort::Wide, false);
stack_fns!(
    sitor_schema,
    make_sitor,
    SITOR_RX,
    sitor::SitorDecoder::new()
);

/// SITOR-B (and the physical layer of NAVTEX), and SITOR-A (ARQ) links read by a monitor.
pub static SITOR: StackDescriptor = StackDescriptor {
    id: "sitor",
    name: "SITOR",
    group: GROUP,
    blurb: "Maritime teleprinter: SITOR-B broadcasts (as used by NAVTEX), four-of-seven code with time diversity, and SITOR-A (AMTOR) ARQ links read by a monitor.",
    // 518 kHz: the international NAVTEX frequency (ITU-R M.540, IMO NAVTEX Manual).
    tuning: tuning(0.518, 10000.0),
    submodes: &[],
    settings: sitor_schema,
    make_receiver: make_sitor,
    make_generator: Some(sitor::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Sitor],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const PSK_RX: RxOptions = RxOptions {
    sender: Some(psk::sender),
    ..opts("psk", InputPort::Wide, false)
};
stack_fns!(psk_schema, make_psk, PSK_RX, psk::PskDecoder::new());

/// PSK31 and its variants.
pub static PSK: StackDescriptor = StackDescriptor {
    id: "psk",
    name: "PSK31",
    group: GROUP,
    blurb: "Keyboard chat by phase shift keying: every PSK31, PSK63, PSK125, PSK250 and QPSK signal in the passband at once.",
    tuning: tuning(0.0, 4000.0),
    submodes: &[],
    settings: psk_schema,
    make_receiver: make_psk,
    make_generator: Some(psk::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Psk31],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const CW_RX: RxOptions = RxOptions {
    listen: false,
    span: true,
    ..opts("cw", InputPort::Wide, false)
};
stack_fns!(cw_schema, make_cw, CW_RX, cw::CwDecoder::new());

/// CW (Morse) skimmer.
pub static CW: StackDescriptor = StackDescriptor {
    id: "cw",
    name: "CW",
    group: GROUP,
    blurb: "Morse code: every keyed carrier in a span of 14 to 192 kHz decoded at once, with the speed of each.",
    tuning: tuning(0.0, 14000.0),
    submodes: &[],
    settings: cw_schema,
    make_receiver: make_cw,
    make_generator: Some(cw::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::CwMorse],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const MFSK_RX: RxOptions = RxOptions {
    sender: Some(mfsk::sender),
    pictures: true,
    ..opts("mfsk", InputPort::CAudio, true)
};
stack_fns!(mfsk_schema, make_mfsk, MFSK_RX, mfsk::MfskDecoder::new());

/// MFSK (MFSK4 to MFSK128L) with Shortwave Radiogram pictures.
pub static MFSK: StackDescriptor = StackDescriptor {
    id: "mfsk",
    name: "MFSK",
    group: GROUP,
    blurb: "Multi-tone keyboard mode with error correction (MFSK16 and its family), and the pictures sent with it.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: mfsk_schema,
    make_receiver: make_mfsk,
    make_generator: Some(mfsk::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Mfsk],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const DOMINOEX_RX: RxOptions = RxOptions {
    sender: Some(domino::domino_sender),
    ..opts("dominoex", InputPort::CAudio, true)
};
pub(crate) const THOR_RX: RxOptions = RxOptions {
    sender: Some(domino::thor_sender),
    ..opts("thor", InputPort::CAudio, true)
};
stack_fns!(
    dominoex_schema,
    make_dominoex,
    DOMINOEX_RX,
    domino::IfkDecoder::new(domino::Family::DominoEx)
);
stack_fns!(
    thor_schema,
    make_thor,
    THOR_RX,
    domino::IfkDecoder::new(domino::Family::Thor)
);

/// DominoEX.
pub static DOMINOEX: StackDescriptor = StackDescriptor {
    id: "dominoex",
    name: "DominoEX",
    group: GROUP,
    blurb: "Incremental frequency keying with 18 tones: tolerant of tuning and drift, no error correction.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: dominoex_schema,
    make_receiver: make_dominoex,
    make_generator: Some(domino::domino_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::DominoEx],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

/// THOR.
pub static THOR: StackDescriptor = StackDescriptor {
    id: "thor",
    name: "THOR",
    group: GROUP,
    blurb: "DominoEX's incremental frequency keying with convolutional error correction and interleaving.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: thor_schema,
    make_receiver: make_thor,
    make_generator: Some(domino::thor_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Thor],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const OLIVIA_RX: RxOptions = RxOptions {
    sender: Some(olivia::olivia_sender),
    ..opts("olivia", InputPort::CAudio, true)
};
pub(crate) const CONTESTIA_RX: RxOptions = RxOptions {
    sender: Some(olivia::contestia_sender),
    ..opts("contestia", InputPort::CAudio, true)
};
stack_fns!(
    olivia_schema,
    make_olivia,
    OLIVIA_RX,
    olivia::OliviaDecoder::new(olivia::Kind::Olivia)
);
stack_fns!(
    contestia_schema,
    make_contestia,
    CONTESTIA_RX,
    olivia::OliviaDecoder::new(olivia::Kind::Contestia)
);

/// Olivia.
pub static OLIVIA: StackDescriptor = StackDescriptor {
    id: "olivia",
    name: "Olivia",
    group: GROUP,
    blurb: "Multi-tone frequency shift keying with Walsh-Hadamard block coding and interleaving: copies text below the noise.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: olivia_schema,
    make_receiver: make_olivia,
    make_generator: Some(olivia::olivia_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Olivia],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

/// Contestia.
pub static CONTESTIA: StackDescriptor = StackDescriptor {
    id: "contestia",
    name: "Contestia",
    group: GROUP,
    blurb: "A faster variant of Olivia with 6-bit upper-case characters.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: contestia_schema,
    make_receiver: make_contestia,
    make_generator: Some(olivia::contestia_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Contestia],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const MT63_RX: RxOptions = RxOptions {
    sender: Some(mt63::sender),
    ..opts("mt63", InputPort::CAudio, false)
};
stack_fns!(mt63_schema, make_mt63, MT63_RX, mt63::Mt63Decoder::new());

/// MT63.
pub static MT63: StackDescriptor = StackDescriptor {
    id: "mt63",
    name: "MT63",
    group: GROUP,
    blurb: "64 phase-keyed carriers with Walsh-Hadamard coding and long interleaving: keyboard text through fading and noise.",
    tuning: tuning(0.0, 4000.0),
    submodes: &[],
    settings: mt63_schema,
    make_receiver: make_mt63,
    make_generator: Some(mt63::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Mt63],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const FSQ_RX: RxOptions = RxOptions {
    outputs: &["dec.messages"],
    sender: Some(fsq::fsq_sender),
    ..opts("fsq", InputPort::CAudio, false)
};
pub(crate) const IFKP_RX: RxOptions = RxOptions {
    sender: Some(fsq::ifkp_sender),
    ..opts("ifkp", InputPort::CAudio, false)
};
stack_fns!(
    fsq_schema,
    make_fsq,
    FSQ_RX,
    fsq::FsqDecoder::new(fsq::Kind::Fsq)
);
stack_fns!(
    ifkp_schema,
    make_ifkp,
    IFKP_RX,
    fsq::FsqDecoder::new(fsq::Kind::Ifkp)
);

/// FSQ.
pub static FSQ: StackDescriptor = StackDescriptor {
    id: "fsq",
    name: "FSQ",
    group: GROUP,
    blurb:
        "Fast Simple QSO: 33-tone incremental keying with addressed messages and automatic replies.",
    tuning: tuning(0.0, 4000.0),
    submodes: &[],
    settings: fsq_schema,
    make_receiver: make_fsq,
    make_generator: Some(fsq::fsq_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Fsq],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

/// IFKP.
pub static IFKP: StackDescriptor = StackDescriptor {
    id: "ifkp",
    name: "IFKP",
    group: GROUP,
    blurb:
        "Incremental frequency keying with 33 tones, a slower relative of FSQ for keyboard chat.",
    tuning: tuning(0.0, 4000.0),
    submodes: &[],
    settings: ifkp_schema,
    make_receiver: make_ifkp,
    make_generator: Some(fsq::ifkp_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ifkp],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: None,
};

pub(crate) const HELL_RX: RxOptions = RxOptions {
    sender: Some(hell::sender),
    pictures: true,
    ..opts("hell", InputPort::CAudio, true)
};
stack_fns!(hell_schema, make_hell, HELL_RX, hell::HellDecoder::new());

/// Hellschreiber.
pub static HELL: StackDescriptor = StackDescriptor {
    id: "hell",
    name: "Hellschreiber",
    group: GROUP,
    blurb: "Text sent as pictures of letters, column by column: the screen draws what it hears.",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: hell_schema,
    make_receiver: make_hell,
    make_generator: Some(hell::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hellschreiber],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::COLUMNS),
    benchmarks: None,
};

pub(crate) const SSTV_RX: RxOptions = RxOptions {
    pictures: true,
    ..opts("sstv", InputPort::CAudio, true)
};
fn sstv_schema() -> Arc<Schema> {
    static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
    typed::schema(&CELL, SSTV_RX.id)
}
fn make_sstv(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(sstv::graph(cfg)?))
}

/// SSTV.
pub static SSTV: StackDescriptor = StackDescriptor {
    id: "sstv",
    name: "SSTV",
    group: GROUP,
    blurb: "Slow-scan television: colour pictures sent line by line as tones (Robot, Martin, Scottie, PD, Wraase).",
    tuning: tuning(0.0, 3000.0),
    submodes: &[],
    settings: sstv_schema,
    make_receiver: make_sstv,
    make_generator: Some(sstv::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Sstv],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::PICTURE),
    benchmarks: None,
};

pub(crate) const ARDOP_RX: RxOptions = opts("ardop", InputPort::CAudio, true);
fn ardop_schema() -> Arc<Schema> {
    static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
    typed::schema(&CELL, ARDOP_RX.id)
}
fn make_ardop(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(ardop::tx::graph(cfg)?))
}

/// ARDOP.
pub static ARDOP: StackDescriptor = StackDescriptor {
    id: "ardop",
    name: "ARDOP",
    group: GROUP,
    blurb: "Lists the ARDOP frames and stations heard: frame types, IDs, connect requests, and 4FSK, PSK and QAM data. Sends FEC text, the ID frame and Ping, and makes ARQ links.",
    tuning: tuning(0.0, 4000.0),
    submodes: &[],
    settings: ardop_schema,
    make_receiver: make_ardop,
    make_generator: Some(ardop::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ardop],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::FRAMES),
    benchmarks: None,
};

pub(crate) const ROS_RX: RxOptions = RxOptions {
    sideband_only: true,
    ..opts("ros", InputPort::CAudio, true)
};
stack_fns!(ros_schema, make_ros, ROS_RX, ros::RosDecoder::new());

const ROS_SUBMODES: &[SubMode] = &[
    SubMode {
        id: "ros16",
        name: "ROS 16 baud",
        default_hz: 0.0,
    },
    SubMode {
        id: "ros8",
        name: "ROS 8 baud",
        default_hz: 0.0,
    },
    SubMode {
        id: "ros4",
        name: "ROS 4 baud",
        default_hz: 0.0,
    },
    SubMode {
        id: "ros1",
        name: "ROS 1 baud (older versions)",
        default_hz: 0.0,
    },
    SubMode {
        id: "rosmf",
        name: "ROS MF (MF1 / MF2 / MF7)",
        default_hz: 0.0,
    },
    SubMode {
        id: "roseme",
        name: "ROS EME (VHF)",
        default_hz: 0.0,
    },
    SubMode {
        id: "ros500",
        name: "ROS HF 500 Hz",
        default_hz: 0.0,
    },
];

/// ROS (identifier only).
pub static ROS: StackDescriptor = StackDescriptor {
    id: "ros",
    name: "ROS (HF)",
    group: GROUP,
    blurb: "Identifies ROS transmissions by their publicly reported tones and rate. The text itself is not public and is not read.",
    tuning: tuning(0.0, 4000.0),
    submodes: ROS_SUBMODES,
    settings: ros_schema,
    make_receiver: make_ros,
    make_generator: Some(ros::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ros],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::DETECTIONS),
    benchmarks: None,
};

pub(crate) const THROB_RX: RxOptions = opts("throb", InputPort::CAudio, true);
stack_fns!(
    throb_schema,
    make_throb,
    THROB_RX,
    throb::ThrobDecoder::new()
);

/// The dial of the test signals of Throb and QRSS. It is not a calling frequency: the host gives the frequency.
pub const SLOW_TEST_SIGNAL_MHZ: f64 = 14.0;

/// Throb and ThrobX.
pub static THROB: StackDescriptor = StackDescriptor {
    id: "throb",
    name: "Throb",
    group: GROUP,
    blurb: "Throb and ThrobX: one or two tones at a time out of 9 or 11, each pair a character; the variant is found from the signal.",
    tuning: tuning(SLOW_TEST_SIGNAL_MHZ, 3000.0),
    submodes: &[],
    settings: throb_schema,
    make_receiver: make_throb,
    make_generator: Some(throb::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TEXT),
    benchmarks: Some(throb_benchmarks),
};

pub(crate) const QRSS_RX: RxOptions = RxOptions {
    pictures: true,
    ..opts("qrss", InputPort::CAudio, true)
};
fn qrss_schema() -> Arc<Schema> {
    static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
    typed::schema(&CELL, "qrss")
}
/// The settings of the QRSS stack.
pub(crate) fn qrss_schema_of() -> Arc<Schema> {
    qrss_schema()
}
fn make_qrss(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    receiver(qrss::QrssDecoder::new(), QRSS_RX, &qrss::with_submode(cfg)?)
}

/// QRSS and DFCW.
pub static QRSS: StackDescriptor = StackDescriptor {
    id: "qrss",
    name: "QRSS / DFCW",
    group: GROUP,
    blurb: "Very slow Morse: a narrow spectrogram of the steady tone as picture rows, and the text read from its keying.",
    tuning: tuning(SLOW_TEST_SIGNAL_MHZ, 3000.0),
    submodes: &[
        SubMode {
            id: "qrss",
            name: "QRSS",
            default_hz: SLOW_TEST_SIGNAL_MHZ * 1e6,
        },
        SubMode {
            id: "dfcw",
            name: "DFCW",
            default_hz: SLOW_TEST_SIGNAL_MHZ * 1e6,
        },
    ],
    settings: qrss_schema,
    make_receiver: make_qrss,
    make_generator: Some(qrss::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::QRSS),
    benchmarks: Some(qrss_benchmarks),
};

fn qrss_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "qrss.receiver_48k",
        run: |s| bench_receiver(s, &QRSS, 60.0),
    }]
}

fn make_cp16(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(cp16::graph(cfg)?))
}

/// The dial of the test signal of CP16. It is not an exercise frequency: the host gives the frequency.
pub const CP16_TEST_SIGNAL_MHZ: f64 = 7.0;

/// CP16.
pub static CP16: StackDescriptor = StackDescriptor {
    id: "cp16",
    name: "CP16",
    group: GROUP,
    blurb: "16 carriers keyed by 16 x 16 dot characters: a picture of the dots and the characters read against a font. Receive and transmit.",
    tuning: tuning(CP16_TEST_SIGNAL_MHZ, 4000.0),
    submodes: &[],
    settings: cp16::schema,
    make_receiver: make_cp16,
    make_generator: Some(cp16::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::CP16),
    benchmarks: Some(cp16_benchmarks),
};

fn cp16_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "cp16.receiver_48k",
        run: |s| bench_receiver(s, &CP16, 30.0),
    }]
}

/// Runs the receiver of `desc` over `signal_s` seconds of its own test signal at 48 ksps (items are input samples).
fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    signal_s: f64,
) -> sdrkit_core::BenchResult {
    use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Values};
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

pub(crate) const WINMOR_RX: RxOptions = opts("winmor", winmor::INPUT, false);
stack_fns!(
    winmor_schema,
    make_winmor,
    WINMOR_RX,
    winmor::WinmorDecoder::new()
);

/// The dial of the test signal of WINMOR. It is not an operating frequency: the host gives the frequency.
pub const WINMOR_TEST_SIGNAL_MHZ: f64 = 7.0;

/// WINMOR (detect only).
pub static WINMOR: StackDescriptor = StackDescriptor {
    id: "winmor",
    name: "WINMOR (detect only)",
    group: GROUP,
    blurb: "Detect only: the 1500 Hz alternating-phase pilot of the retired WINMOR modem, the tuning error and the four raw frame-type tones.",
    tuning: tuning(WINMOR_TEST_SIGNAL_MHZ, 3000.0),
    submodes: &[],
    settings: winmor_schema,
    make_receiver: make_winmor,
    make_generator: Some(winmor::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::PILOTS),
    benchmarks: Some(winmor_benchmarks),
};

fn winmor_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "winmor.receiver_48k",
        run: |s| bench_receiver(s, &WINMOR, 30.0),
    }]
}

fn throb_benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "throb.receiver_48k",
        run: |s| bench_receiver(s, &THROB, 30.0),
    }]
}

/// Every stack of this family.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &RTTY, &PSK, &CW, &OLIVIA, &CONTESTIA, &MFSK, &DOMINOEX, &THOR, &HELL, &MT63, &FSQ, &IFKP,
        &SSTV, &SITOR, &ARDOP, &ROS, &THROB, &QRSS, &CP16, &WINMOR,
    ]
}

/// Adds the family's components to a registry: the front end and every decoder, under `hfkbd.<name>`.
pub fn register(r: &mut Registry) {
    use sdrkit_graph::DecoderComponent;
    r.add("hfkbd.front", || Box::new(front::HfFront::new()));
    r.add("hfkbd.ros", || {
        Box::new(DecoderComponent::new(ros::RosDecoder::new()))
    });
    r.add("hfkbd.rtty", || {
        Box::new(DecoderComponent::new(rtty::RttyDecoder::new()))
    });
    r.add("hfkbd.ardop", || {
        Box::new(DecoderComponent::new(ardop::ArdopDecoder::new()))
    });
    r.add("hfkbd.cw", || {
        Box::new(DecoderComponent::new(cw::CwDecoder::new()))
    });
    r.add("hfkbd.morse", || {
        Box::new(DecoderComponent::new(cw::MorseDecoder::new()))
    });
    r.add("hfkbd.dominoex", || {
        Box::new(DecoderComponent::new(domino::IfkDecoder::new(
            domino::Family::DominoEx,
        )))
    });
    r.add("hfkbd.thor", || {
        Box::new(DecoderComponent::new(domino::IfkDecoder::new(
            domino::Family::Thor,
        )))
    });
    r.add("hfkbd.olivia", || {
        Box::new(DecoderComponent::new(olivia::OliviaDecoder::new(
            olivia::Kind::Olivia,
        )))
    });
    r.add("hfkbd.contestia", || {
        Box::new(DecoderComponent::new(olivia::OliviaDecoder::new(
            olivia::Kind::Contestia,
        )))
    });
    r.add("hfkbd.fsq", || {
        Box::new(DecoderComponent::new(fsq::FsqDecoder::new(fsq::Kind::Fsq)))
    });
    r.add("hfkbd.ifkp", || {
        Box::new(DecoderComponent::new(fsq::FsqDecoder::new(fsq::Kind::Ifkp)))
    });
    r.add("hfkbd.hell", || {
        Box::new(DecoderComponent::new(hell::HellDecoder::new()))
    });
    r.add("hfkbd.mfsk", || {
        Box::new(DecoderComponent::new(mfsk::MfskDecoder::new()))
    });
    r.add("hfkbd.mt63", || {
        Box::new(DecoderComponent::new(mt63::Mt63Decoder::new()))
    });
    r.add("hfkbd.psk", || {
        Box::new(DecoderComponent::new(psk::PskDecoder::new()))
    });
    r.add("hfkbd.sstv", || {
        Box::new(DecoderComponent::new(sstv::SstvDecoder::new()))
    });
    r.add("hfkbd.sstvtx", || Box::new(sstv::SstvSender::default()));
    r.add("hfkbd.ardoptx", || {
        Box::new(ardop::tx::ArdopSender::default())
    });
    r.add("hfkbd.throb", || {
        Box::new(DecoderComponent::new(throb::ThrobDecoder::new()))
    });
    r.add("hfkbd.qrss", || {
        Box::new(DecoderComponent::new(qrss::QrssDecoder::new()))
    });
    r.add("hfkbd.cp16", || {
        Box::new(DecoderComponent::new(cp16::Cp16Decoder::new()))
    });
    r.add("hfkbd.cp16tx", || Box::new(cp16::Cp16Sender::default()));
    r.add("hfkbd.rttytx", rtty::sender);
    r.add("hfkbd.psktx", psk::sender);
    r.add("hfkbd.dominoextx", domino::domino_sender);
    r.add("hfkbd.mfsktx", mfsk::sender);
    r.add("hfkbd.oliviatx", olivia::olivia_sender);
    r.add("hfkbd.contestiatx", olivia::contestia_sender);
    r.add("hfkbd.mt63tx", mt63::sender);
    r.add("hfkbd.thortx", domino::thor_sender);
    r.add("hfkbd.helltx", hell::sender);
    r.add("hfkbd.fsqtx", fsq::fsq_sender);
    r.add("hfkbd.ifkptx", fsq::ifkp_sender);
    r.add("hfkbd.winmor", || {
        Box::new(DecoderComponent::new(winmor::WinmorDecoder::new()))
    });
    r.add("hfkbd.sitor", || {
        Box::new(DecoderComponent::new(sitor::SitorDecoder::new()))
    });
}
