//! Radio navigation aids: NDB, VOR and Doppler VOR, ILS localizer, glide path and marker beacons, DME, Loran-C and eLoran chains, and DGPS
//! beacons. Each stack is a graph of components on the shared crates, with declared results, verify cases and a generator of
//! the clean test signal.
//! Standard: ICAO Annex 10 Volume I, ITU-R M.589, ITU-R M.823, RTCM 10402.3, IS-GPS-200.

#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]

pub mod carrier;
pub mod dgps;
pub mod dme;
pub mod fields;
pub mod fold;
pub mod ident;
pub mod ils;
pub mod keyer;
pub mod loran;
pub mod marker;
pub mod morse;
pub mod ndb;
pub mod pairing;
pub mod typed;
pub mod verify;
pub mod vor;

use sdrkit_core::bench::measure;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, GenConfig, InputInfo, LockProfile, Output,
    SettingError, SettingSet, StackConfig, StackDescriptor, SubMode, Tuning, Values,
};
use sdrkit_graph::GraphError;

/// The heading in the stack list.
pub const GROUP: &str = "Navigation";

/// NDB.
pub static NDB: StackDescriptor = StackDescriptor {
    id: ndb::ID,
    name: "NDB",
    group: GROUP,
    blurb: "Non-directional beacons: the carrier, the Morse identification, the audio, and every beacon in the capture.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Am, 3e3)),
        min_hz: 150e3,
        max_hz: 1800e3,
        default_hz: 344e3,
        rate_hz: 250e3,
        min_rate_hz: ndb::MIN_RATE_HZ,
        bandwidth_hz: ndb::CHANNEL_WIDTH_HZ,
        carrier_at_centre: true,
        ..Tuning::anywhere(344e3, 250e3, ndb::MIN_RATE_HZ, ndb::CHANNEL_WIDTH_HZ)
    },
    submodes: &[],
    settings: ndb::NdbSetting::schema,
    make_receiver: ndb::receiver,
    make_generator: Some(ndb::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ndb],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::NDB_HOOKS),
    benchmarks: Some(benchmarks),
};

/// VOR.
pub static VOR: StackDescriptor = StackDescriptor {
    id: vor::ID,
    name: "VOR",
    group: GROUP,
    blurb: "The radial from a VOR or Doppler VOR, the course deviation for a selected course, and the station's identification.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Am, 10e3)),
        min_hz: 108e6,
        max_hz: 118e6,
        carrier_at_centre: true,
        ..Tuning::anywhere(113.1e6, 250e3, vor::MIN_RATE_HZ, vor::CHANNEL_WIDTH_HZ)
    },
    submodes: &[],
    settings: vor::VorSetting::schema,
    make_receiver: vor::receiver,
    make_generator: Some(vor::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Vor],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::VOR_HOOKS),
    benchmarks: None,
};

/// ILS.
pub static ILS: StackDescriptor = StackDescriptor {
    id: ils::ID,
    name: "ILS",
    group: GROUP,
    blurb: "Localizer, glide path and marker beacons: deviation from the course and the path, the flag, the identification, paired frequencies, and the marker passed over.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: ils::MARKER_RANGE_HZ.0,
        max_hz: 336e6,
        carrier_at_centre: true,
        ..Tuning::anywhere(110.3e6, 250e3, ils::MIN_RATE_HZ, ils::CHANNEL_WIDTH_HZ)
    },
    submodes: &[
        SubMode { id: "localizer", name: "Localizer", default_hz: 110.3e6, ..SubMode::DEFAULT },
        SubMode { id: "glideslope", name: "Glide path", default_hz: 335.0e6, ..SubMode::DEFAULT },
        SubMode { id: "marker", name: "Marker beacon", default_hz: marker::CARRIER_HZ, ..SubMode::DEFAULT },
    ],
    settings: ils::IlsSetting::schema,
    make_receiver: ils::receiver,
    make_generator: Some(ils::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Ils],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::ILS_HOOKS),
    benchmarks: None,
};

/// DME.
pub static DME: StackDescriptor = StackDescriptor {
    id: dme::ID,
    name: "DME",
    group: GROUP,
    blurb: "Pulse pairs of a DME beacon or of interrogating aircraft: the channel, X or Y, the pair rate and the identification.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 960e6,
        max_hz: 1215e6,
        not_circular: true,
        radio_filter_hz: Some(1.5e6),
        ..Tuning::anywhere(978e6, dme::RATE_HZ, dme::MIN_RATE_HZ, 1e6)
    },
    submodes: &[],
    settings: dme::DmeSetting::schema,
    make_receiver: dme::receiver,
    make_generator: Some(dme::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dme],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::DME_HOOKS),
    benchmarks: None,
};

/// Loran-C and eLoran.
pub static LORAN: StackDescriptor = StackDescriptor {
    id: loran::ID,
    name: "Loran-C / eLoran",
    group: GROUP,
    blurb: "A Loran chain on 100 kHz: its GRI, the master and secondaries, their time differences, ECD and signal to noise.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 90e3,
        max_hz: 110e3,
        carrier_at_centre: true,
        no_offset_tuning: true,
        ..Tuning::anywhere(100e3, 250e3, loran::MIN_RATE_HZ, 20e3)
    },
    submodes: &[],
    settings: loran::LoranSetting::schema,
    make_receiver: loran::receiver,
    make_generator: Some(loran::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::LoranC],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::LORAN_HOOKS),
    benchmarks: None,
};

/// DGPS beacons.
pub static DGPS: StackDescriptor = StackDescriptor {
    id: dgps::ID,
    name: "DGPS beacon",
    group: GROUP,
    blurb: "Differential GPS beacons: RTCM corrections, the base station's position, the beacon almanac and text messages.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 283.5e3,
        max_hz: 325e3,
        ..Tuning::anywhere(304e3, 250e3, dgps::MIN_RATE_HZ, dgps::CHANNEL_WIDTH_HZ)
    },
    submodes: &[],
    settings: dgps::DgpsSetting::schema,
    make_receiver: dgps::receiver,
    make_generator: Some(dgps::generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Dgps],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::DGPS_HOOKS),
    benchmarks: None,
};

/// Every navigation-aid stack.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&NDB, &VOR, &ILS, &DME, &LORAN, &DGPS]
}

/// Adds every component of this crate to `r`, each under the name its `info()` returns (`avnav.<name>`).
pub fn register(r: &mut sdrkit_graph::Registry) {
    use crate::{carrier, keyer, morse};
    r.add("avnav.am_carrier", || {
        Box::new(carrier::AmCarrier::default())
    });
    r.add("avnav.bfo", || Box::new(carrier::Bfo::default()));
    r.add("avnav.tone_keyer", || Box::new(keyer::ToneKeyer::default()));
    r.add("avnav.key_select", || Box::new(keyer::KeySelect::default()));
    r.add("avnav.morse_decoder", || {
        Box::new(morse::MorseDecoder::default())
    });
    r.add("avnav.carrier_keyer", || {
        Box::new(ndb::CarrierKeyer::default())
    });
    r.add("avnav.beacon_scan", || Box::new(ndb::BeaconScan::default()));
    r.add("avnav.word_guard", || Box::new(ndb::WordGuard::default()));
    r.add("avnav.ident_folder", || {
        Box::new(fold::IdentFolder::default())
    });
    r.add("avnav.ndb_report", || Box::new(ndb::NdbReport::default()));
    r.add("avnav.vor_bearing", || Box::new(vor::VorBearing::default()));
    r.add("avnav.vor_report", || Box::new(vor::VorReport::default()));
    r.add("avnav.ils_ddm", || Box::new(ils::IlsDdm::default()));
    r.add("avnav.ils_report", || Box::new(ils::IlsReport::default()));
    r.add("avnav.marker", || Box::new(marker::MarkerTones::default()));
    r.add("avnav.pulse_detect", || {
        Box::new(dme::PulseDetect::default())
    });
    r.add("avnav.pulse_pairs", || Box::new(dme::PulsePairs::default()));
    r.add("avnav.dme_ident", || Box::new(dme::DmeIdent::default()));
    r.add("avnav.dme_report", || Box::new(dme::DmeReport::default()));
    r.add("avnav.loran_tracker", || {
        Box::new(loran::LoranTracker::default())
    });
    r.add("avnav.loran_report", || {
        Box::new(loran::LoranReport::default())
    });
    r.add("avnav.rtcm_framer", || {
        Box::new(dgps::rtcm::RtcmFramer::default())
    });
    r.add(
        "avnav.rtcm_select",
        || Box::new(dgps::RtcmSelect::default()),
    );
    r.add(
        "avnav.dgps_report",
        || Box::new(dgps::DgpsReport::default()),
    );
}

/// The standard components and this crate's.
pub fn registry() -> sdrkit_graph::Registry {
    let mut r = sdrkit_components::registry();
    register(&mut r);
    r
}

/// A component made by a constructor that checks its arguments against its schema, or the refusal as a [`GraphError`] that
/// names the component.
fn checked<C>(component: &str, made: Result<C, SettingError>) -> Result<C, GraphError> {
    made.map_err(|error| GraphError::Setting {
        key: component.into(),
        error,
    })
}

fn bench_stack(
    d: &StackDescriptor,
    rate: f64,
    tuned: f64,
    signal_s: f64,
    seconds: f64,
) -> BenchResult {
    bench_submode(d, None, rate, tuned, signal_s, seconds)
}

/// The benchmark of `d` with `submode` (none: the stack's own choice) on its test signal.
fn bench_submode(
    d: &StackDescriptor,
    submode: Option<&str>,
    rate: f64,
    tuned: f64,
    signal_s: f64,
    seconds: f64,
) -> BenchResult {
    let settings = Values::defaults(&(d.settings)());
    let Some(make) = d.make_generator else {
        return BenchResult::default();
    };
    let submode = submode.map(str::to_owned);
    let Ok(mut g) = make(
        &GenConfig::new(rate, settings.clone())
            .with_seed(1)
            .with_submode_opt(submode.clone()),
    ) else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let Ok(mut rx) = (d.make_receiver)(&StackConfig::new(settings).with_submode_opt(submode)) else {
        return BenchResult::default();
    };
    if rx
        .configure(&InputInfo::new(rate, tuned, tuned).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    measure(seconds, x.len(), signal_s, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// Benchmarks of the receivers at their input rates (`items_per_second` counts input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "avnav.ndb_250k",
            run: |s| bench_stack(&NDB, 250e3, 344e3, 1.0, s),
        },
        Benchmark {
            name: "avnav.vor_250k",
            run: |s| bench_stack(&VOR, 250e3, 113.1e6, 1.0, s),
        },
        Benchmark {
            name: "avnav.ils_250k",
            run: |s| bench_stack(&ILS, 250e3, 110.3e6, 1.0, s),
        },
        Benchmark {
            name: "avnav.ils_marker_250k",
            run: |s| bench_submode(&ILS, Some("marker"), 250e3, marker::CARRIER_HZ, 1.0, s),
        },
        Benchmark {
            name: "avnav.dme_2m",
            run: |s| bench_stack(&DME, 2e6, 978e6, 0.5, s),
        },
        Benchmark {
            name: "avnav.loran_250k",
            run: |s| bench_stack(&LORAN, 250e3, 100e3, 1.0, s),
        },
        Benchmark {
            name: "avnav.dgps_250k",
            run: |s| bench_stack(&DGPS, 250e3, 304e3, 1.0, s),
        },
    ]
}
