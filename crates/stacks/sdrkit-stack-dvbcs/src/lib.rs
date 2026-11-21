//! Cable and satellite television and the TV satellite beacon decoder. The transport stream of every TV stack goes to the
//! layer of `sdrkit-proto-mpegts` (services, guide, teletext, subtitles and media), and DVB-S2 frames go through the stream
//! adaptation and IP of `sdrkit-proto-ipcast`. The DVB-S and DVB-S2 receiver and the receive and transmit parts that the TV
//! stacks share are in `sdrkit-proto-dvbs2`. The stacks are [`DVBC`] (`"dvbc"`), [`DVBC2`] (`"dvbc2"`), [`DVBS`] (`"dvbs"`,
//! DVB-S and DVB-S2), [`ISDBS`] (`"isdbs"`) and [`SATBEACON`] (`"satbeacon"`).
//!
//! Standard: ETSI EN 300 429, EN 302 769, EN 300 421, EN 302 307-1; ITU-R BO.1408-1.

#![warn(missing_docs)]
// Index loops read more clearly than iterator chains in the signal processing here.
#![allow(clippy::needless_range_loop)]

pub mod beacon;
pub mod dvbc;
pub mod dvbc2;
pub mod dvbs;
pub mod dvbs2_transmit;
pub mod dvbs_transmit;
pub mod isdbs;
pub mod typed;
pub mod verify;

pub use beacon::SATBEACON;
pub use dvbc::DVBC;
pub use dvbc2::DVBC2;
pub use dvbs::DVBS;
pub use isdbs::ISDBS;

use sdrkit_core::{
    Choice, Complex32, InputInfo, LockCondition, LockProfile, LockState, NameSource, Output,
    SettingId, SettingSet, StackConfig, Value, Values,
};

/// The parts of the transport stream layer that the TV stacks of this crate attach: every part.
pub const TS_LAYER: sdrkit_proto_mpegts::layer::TsLayer = sdrkit_proto_mpegts::layer::TsLayer {
    teletext: true,
    subtitles: true,
};

/// The lock profile of a TV multiplex for the scanner.
pub(crate) const TV_LOCK_PROFILE: LockProfile = LockProfile {
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

/// The stacks of this crate.
pub fn descriptors() -> Vec<&'static sdrkit_core::StackDescriptor> {
    vec![&DVBC, &DVBC2, &DVBS, &ISDBS, &SATBEACON]
}

/// Adds the components of this crate, the satellite receiver's, the transport stream layer's and the IP layer's to a graph
/// registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("dvbc.demod", || Box::new(dvbc::DvbcDemod::default()));
    r.add("dvbc.outer", || Box::new(dvbc::DvbcOuter::default()));
    r.add("dvbc.report", || Box::new(dvbc::DvbcReport::default()));
    r.add(
        "dvbc2.receiver",
        || Box::new(dvbc2::comp::C2Comp::default()),
    );
    r.add("dvbs.fec1", || Box::new(dvbs::Dvbs1Fec::default()));
    r.add("isdbs.demod", || Box::new(isdbs::IsdbsDemod::default()));
    r.add("isdbs.decode", || Box::new(isdbs::IsdbsDecode::default()));
    r.add("isdbs.report", || Box::new(isdbs::IsdbsReport::default()));
    r.add("beacon.carrier", || {
        Box::new(beacon::BeaconCarrier::default())
    });
    r.add("beacon.telemetry", || {
        Box::new(beacon::BeaconTelemetry::default())
    });
    r.add(
        "beacon.frames",
        || Box::new(beacon::BeaconFrames::default()),
    );
    sdrkit_proto_dvbs2::register(r);
    sdrkit_proto_mpegts::register(r);
    sdrkit_proto_ipcast::register(r);
}

/// The benchmarks of this crate: the DVB-C receiver on 64-QAM, the DVB-S2 receiver on QPSK 3/5 normal frames (LDPC and BCH
/// included), the beacon carrier stage (items: input samples) and the DVB-S and DVB-S2 senders (items: output samples). The DVB-S2 signal has no noise and is 2 s long, so that the
/// frames decoded outweigh the carrier search at the start of each run.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbc_64qam",
            run: |s| {
                bench_stack(
                    &DVBC,
                    10e6,
                    &[(
                        dvbc::DvbcSetting::GenQam.id(),
                        Value::Choice(dvbc::qam::Qam::Q64.index()),
                    )],
                    0.5,
                    s,
                )
            },
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbs2_qpsk35",
            run: |s| {
                bench_stack(
                    &DVBS,
                    4e6,
                    &[
                        (
                            sdrkit_proto_dvbs2::DvbsSetting::GenRate.id(),
                            Value::Choice(sdrkit_proto_dvbs2::CodeRate::R35.index()),
                        ),
                        (
                            sdrkit_proto_dvbs2::DvbsSetting::GenSymbolRate.id(),
                            Value::Hz(1e6),
                        ),
                    ],
                    2.0,
                    s,
                )
            },
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.beacon_carrier",
            run: |s| bench_stack(&SATBEACON, 250e3, &[], 0.5, s),
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbs_sender_qpsk34_1msym_at_2msps",
            run: |s| dvbs_transmit::bench_sender(s, &dvbs_transmit::BENCH_1M, 2e6),
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbs_sender_qpsk34_2msym_at_4msps",
            run: |s| dvbs_transmit::bench_sender(s, &dvbs_transmit::BENCH_2M, 4e6),
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbs2_sender_qpsk34_1msym_at_2msps",
            run: |s| dvbs2_transmit::bench_sender(s, &dvbs2_transmit::bench_qpsk(), 2e6),
        },
        sdrkit_core::Benchmark {
            name: "dvbcs.dvbs2_sender_16apsk34_1msym_at_2msps",
            run: |s| dvbs2_transmit::bench_sender(s, &dvbs2_transmit::bench_16apsk(), 2e6),
        },
    ]
}

fn bench_stack(
    desc: &'static sdrkit_core::StackDescriptor,
    rate: f64,
    gen: &[(SettingId, Value)],
    signal_s: f64,
    seconds: f64,
) -> sdrkit_core::BenchResult {
    let mut values = Values::defaults(&(desc.settings)());
    for (id, v) in gen {
        // The benchmark values are choices and rates that the settings take.
        let _ = values.set(*id, v);
    }
    let mut sig = vec![Complex32::default(); (rate * signal_s) as usize];
    if let Some(make) = desc.make_generator {
        let Ok(mut g) = make(&sdrkit_core::GenConfig::new(rate, values.clone()).with_seed(1)) else {
            return sdrkit_core::BenchResult::default();
        };
        let n = g.generate(&mut sig);
        sig.truncate(n);
    }
    let signal_s = sig.len() as f64 / rate;
    let cfg = StackConfig::new(values);
    sdrkit_core::bench::measure(seconds, sig.len(), signal_s, || {
        let Ok(mut rx) = (desc.make_receiver)(&cfg) else {
            return;
        };
        if rx
            .configure(&InputInfo::new(rate, 1e9, 1e9).recording())
            .is_err()
        {
            return;
        }
        let mut out = Output::default();
        for c in sig.chunks(16384) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}
