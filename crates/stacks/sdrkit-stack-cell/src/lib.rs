//! Cell and railway broadcast stacks. The cell stacks read only what every cell broadcasts to all phones: LTE, 5G NR,
//! NB-IoT, GSM and GSM-R, and the 3G cells of UMTS, cdma2000 and TD-SCDMA. The C-V2X stack shows the structure of LTE
//! sidelink transmissions. The railway stacks read the LBJ train broadcasts, the Eurobalise up-link telegrams and the
//! end-of-train telemetry. Nothing addressed to a phone is decoded, and nothing is sent. The generators make the test signals.
//!
//! [`descriptors`] lists the stacks: `cell` (LTE), `nr`, `nbiot`, `gsmr`, `lbj`, `ebicab`, `eot`, `umts`, `cdma2000`,
//! `tdscdma` and `cv2x`.

#![warn(missing_docs)]
// Signal processing over resource grids indexes several arrays with one index; the loops say so plainly.
#![allow(clippy::needless_range_loop)]

pub mod balise;
pub mod bands;
pub mod cdma2000;
pub mod cv2x;
pub mod decoder;
pub mod dsp;
pub mod eot;
pub mod gsm;
pub mod lbj;
pub mod lte;
pub mod nbiot;
pub mod nr;
pub mod railway_cir;
pub mod scan;
pub mod tdscdma;
pub mod towers;
pub mod typed;
pub mod umts;
pub mod verify;

use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, GenConfig, InputInfo, LockProfile, Output, StackConfig,
    StackDescriptor, Tuning, Values,
};

/// The heading of this family in the stack list.
pub const GROUP: &str = "Cellular and railway";

/// LBJ train broadcasts.
pub static LBJ: StackDescriptor = StackDescriptor {
    id: "lbj",
    name: "LBJ",
    group: GROUP,
    blurb: "Chinese railway train broadcasts: train number, speed, kilometre post, locomotive, route and position.",
    tuning: Tuning {
        min_hz: 821.0e6,
        max_hz: 821.5e6,
        default_hz: 0.0,
        rate_hz: 960e3,
        min_rate_hz: 256e3,
        bandwidth_hz: 35e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(150e3),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: lbj::schema,
    make_receiver: lbj::make_receiver,
    make_generator: Some(lbj::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lbj],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LBJ),
    benchmarks: None,
};

/// Eurobalise and EBICab up-link.
pub static EBICAB: StackDescriptor = StackDescriptor {
    id: "ebicab",
    name: "EBICab / Eurobalise",
    group: GROUP,
    blurb: "Eurobalise up-link telegrams between the rails (SUBSET-036), and the bursts of other balise systems.",
    tuning: Tuning {
        min_hz: 1e6,
        max_hz: 30e6,
        // SUBSET-036 fixes the Eurobalise up-link at 4.234 MHz wherever the system is used.
        default_hz: 4.234e6,
        rate_hz: 10e6,
        min_rate_hz: 4e6,
        bandwidth_hz: 1.8e6,
        tune_offset_hz: 1e6,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: balise::schema,
    make_receiver: balise::make_receiver,
    make_generator: Some(balise::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Balise],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::BALISE),
    benchmarks: None,
};

/// GSM and GSM-R base stations.
pub static GSMR: StackDescriptor = StackDescriptor {
    id: "gsmr",
    name: "GSM / GSM-R",
    group: GROUP,
    blurb: "GSM 900 and GSM-R base stations: frequency, BSIC, operator, location area and cell identity from the broadcast channel.",
    tuning: Tuning {
        min_hz: 860e6,
        max_hz: 960e6,
        default_hz: 0.0,
        rate_hz: 5e6,
        min_rate_hz: 1e6,
        bandwidth_hz: 4.8e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(5e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: gsm::schema,
    make_receiver: gsm::make_receiver,
    make_generator: Some(gsm::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Gsm],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::GSM),
    benchmarks: None,
};

/// NB-IoT cells.
pub static NBIOT: StackDescriptor = StackDescriptor {
    id: "nbiot",
    name: "NB-IoT cells",
    group: GROUP,
    blurb: "NB-IoT cells: cell identity, operator, band and level from the synchronisation signals, MIB-NB and SIB1-NB.",
    tuning: Tuning {
        min_hz: 450e6,
        max_hz: 2200e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 1e6,
        bandwidth_hz: 1.75e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: nbiot::schema,
    make_receiver: nbiot::make_receiver,
    make_generator: Some(nbiot::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::NbIot],
    auto_gain: AutoGain::Hold,
    verify: Some(&verify::NBIOT),
    benchmarks: None,
};

/// 5G NR cells.
pub static NR: StackDescriptor = StackDescriptor {
    id: "nr",
    name: "5G NR cells",
    group: GROUP,
    blurb: "5G NR cells: physical cell identity, SS/PBCH beams and the MIB of every SSB on the sync raster, and SIB1 (networks, tracking area, cell identity) where CORESET#0 lies in the capture.",
    tuning: Tuning {
        min_hz: 600e6,
        max_hz: 4200e6,
        default_hz: 0.0,
        rate_hz: 20e6,
        min_rate_hz: 3.84e6,
        bandwidth_hz: 20e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(15e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: nr::schema,
    make_receiver: nr::make_receiver,
    make_generator: Some(nr::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Nr],
    auto_gain: AutoGain::Hold,
    verify: Some(&verify::NR),
    benchmarks: None,
};

/// LTE cells.
pub static CELL: StackDescriptor = StackDescriptor {
    id: "cell",
    name: "LTE cells",
    group: GROUP,
    blurb: "LTE cells: operator, band, cell identity, area code and signal strength from the information every cell broadcasts.",
    tuning: Tuning {
        min_hz: 450e6,
        max_hz: 3800e6,
        default_hz: 0.0,
        rate_hz: 20e6,
        min_rate_hz: 1.92e6,
        bandwidth_hz: 10e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(15e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: lte::schema,
    make_receiver: lte::make_receiver,
    make_generator: Some(lte::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lte],
    auto_gain: AutoGain::Hold,
    verify: Some(&verify::LTE),
    benchmarks: None,
};

/// The frequency of the end-of-train test signal, Hz. The host tunes the session from its data.
pub const EOT_TEST_SIGNAL_HZ: f64 = 450e6;

/// End-of-train telemetry and head-of-train commands.
pub static EOT: StackDescriptor = StackDescriptor {
    id: eot::ID,
    name: "End of train (EOT / HOT)",
    group: GROUP,
    blurb: "End-of-train devices: unit, brake pipe pressure, motion, battery and marker light from 1200 bit/s AFSK on FM, or the head-of-train commands.",
    tuning: Tuning {
        min_hz: 25e6,
        max_hz: 1000e6,
        default_hz: EOT_TEST_SIGNAL_HZ,
        rate_hz: 250e3,
        min_rate_hz: eot::MIN_RATE,
        bandwidth_hz: 12.5e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: eot::schema,
    make_receiver: eot::make_receiver,
    make_generator: Some(eot::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::EOT),
    benchmarks: Some(eot_benchmarks),
};

/// C-V2X sidelink structure.
pub static CV2X: StackDescriptor = StackDescriptor {
    id: cv2x::ID,
    name: "C-V2X (structure)",
    group: GROUP,
    blurb: "Structure only: LTE sidelink (C-V2X) transmissions found by the SC-FDMA prefix and the 1 ms subframe, with the carrier offset and the busy subframes; nothing is decoded.",
    tuning: Tuning {
        min_hz: 5800e6,
        max_hz: 6000e6,
        default_hz: cv2x::TEST_SIGNAL_HZ,
        rate_hz: 15.36e6,
        min_rate_hz: cv2x::MIN_RATE_HZ,
        bandwidth_hz: 10e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: cv2x::schema,
    make_receiver: cv2x::make_receiver,
    make_generator: Some(cv2x::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::CV2X),
    benchmarks: Some(cv2x_benchmarks),
};

/// UMTS cells.
pub static UMTS: StackDescriptor = StackDescriptor {
    id: umts::ID,
    name: "UMTS cells",
    group: GROUP,
    blurb: "UMTS (WCDMA) cells: primary scrambling code, Ec/Io, level and carrier offset from the synchronisation channels and the CPICH, and the operator and cell identity from the broadcast channel.",
    tuning: Tuning {
        min_hz: 700e6,
        max_hz: 2700e6,
        default_hz: umts::TEST_SIGNAL_HZ,
        rate_hz: 7.68e6,
        min_rate_hz: umts::MIN_RATE_HZ,
        bandwidth_hz: 5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(5e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: umts::schema,
    make_receiver: umts::make_receiver,
    make_generator: Some(umts::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::UMTS),
    benchmarks: Some(umts_benchmarks),
};

/// CDMA2000 cells.
pub static CDMA2000: StackDescriptor = StackDescriptor {
    id: cdma2000::ID,
    name: "CDMA2000 cells",
    group: GROUP,
    blurb: "CDMA2000 1x and IS-95 base stations: every pilot by PN offset with Ec/Io, and its Sync Channel Message (SID, NID, System Time, Paging Channel).",
    tuning: Tuning {
        min_hz: 400e6,
        max_hz: 2200e6,
        default_hz: cdma2000::TEST_SIGNAL_HZ,
        rate_hz: 2.4576e6,
        min_rate_hz: cdma2000::MIN_RATE_HZ,
        bandwidth_hz: 1.25e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: cdma2000::schema,
    make_receiver: cdma2000::make_receiver,
    make_generator: Some(cdma2000::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::CDMA2000),
    benchmarks: Some(cdma2000_benchmarks),
};

/// TD-SCDMA cells.
pub static TDSCDMA: StackDescriptor = StackDescriptor {
    id: tdscdma::ID,
    name: "TD-SCDMA cells",
    group: GROUP,
    blurb: "TD-SCDMA cells: SYNC-DL code, cell parameter, frame parity, level and carrier offset from the downlink pilot and the midamble; the broadcast channel is not decoded.",
    tuning: Tuning {
        min_hz: 1880e6,
        max_hz: 2400e6,
        default_hz: tdscdma::TEST_SIGNAL_HZ,
        rate_hz: 2.56e6,
        min_rate_hz: tdscdma::MIN_RATE_HZ,
        bandwidth_hz: 1.6e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: tdscdma::schema,
    make_receiver: tdscdma::make_receiver,
    make_generator: Some(tdscdma::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::TDSCDMA),
    benchmarks: Some(tdscdma_benchmarks),
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&CELL, &NR, &NBIOT, &GSMR, &LBJ, &EBICAB, &EOT, &UMTS, &CDMA2000, &TDSCDMA, &CV2X, &railway_cir::DESCRIPTOR]
}

/// Runs the receiver of `desc` at `rate` on `signal_s` of its own test signal, the channel where the receiver looks.
fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let mut g = (desc.make_generator.expect("the stack has a generator"))(&GenConfig::new(
        rate,
        settings.clone(),
    ))
    .expect("the generator builds");
    let n = (rate * signal_s) as usize;
    let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let mut rx = (desc.make_receiver)(&StackConfig::new(settings)).expect("the receiver builds");
    // A stack without a default frequency takes the middle of its range.
    let f = if desc.tuning.default_hz > 0.0 {
        desc.tuning.default_hz
    } else {
        (desc.tuning.min_hz + desc.tuning.max_hz) / 2.0
    };
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

/// The benchmark of the railway CIR receiver at 960 ksps on the worked KLW frame (items are input samples).
pub fn railway_cir_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "railway_cir.receiver_960k",
        run: |s| bench_receiver(s, &railway_cir::DESCRIPTOR, 960e3, 2.2),
    }]
}

/// The benchmark of the end-of-train receiver at the rate it asks for (items are input samples).
fn eot_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "eot.receiver_250k",
        run: |s| bench_receiver(s, &EOT, 250e3, 1.0),
    }]
}

/// The benchmark of the UMTS receiver at 7.68 Msps.
fn umts_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "umts.receiver_7m68",
        run: |s| bench_receiver(s, &UMTS, 7.68e6, 0.4),
    }]
}

/// The benchmark of the cdma2000 receiver at 2.4576 Msps.
fn cdma2000_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "cdma2000.receiver_2m4576",
        run: |s| bench_receiver(s, &CDMA2000, 2.4576e6, 2.0),
    }]
}

/// The benchmark of the TD-SCDMA receiver at 2.56 Msps.
fn tdscdma_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "tdscdma.receiver_2m56",
        run: |s| bench_receiver(s, &TDSCDMA, 2.56e6, 0.4),
    }]
}

/// The benchmark of the C-V2X detector at 15.36 Msps.
fn cv2x_benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "cv2x.receiver_15m36",
        run: |s| bench_receiver(s, &CV2X, 15.36e6, 0.5),
    }]
}
