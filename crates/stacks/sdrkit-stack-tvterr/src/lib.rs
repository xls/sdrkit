//! Terrestrial TV beyond DVB: ISDB-T, ISDB-TSB, ATSC 1.0 (8-VSB), ATSC 3.0, DTMB, and TV (auto), which finds out which standard a
//! channel carries. Each stack is a graph of components. The transport stream goes to the layer of `sdrkit-proto-mpegts`.
//!
//! Standard: ARIB STD-B31, ABNT NBR 15601, ITU-R BS.1114 Annex 3, ATSC A/53, A/65, A/321, A/322, A/330, A/331, GB 20600-2006,
//! ISO/IEC 13818-1.

#![warn(missing_docs)]
// DSP and table code indexes several arrays with one counter. The index form is the clearer one there.
#![allow(clippy::needless_range_loop)]

pub mod atsc;
pub mod atsc3;
pub mod auto;
pub mod band;
pub mod dtmb;
pub mod isdbt;
pub mod isdbtsb;
pub mod plans;
pub mod scan;
pub mod status;
pub mod typed;
pub mod verify;

use sdrkit_core::{
    Generator, LockCondition, LockProfile, LockState, NameSource, Receiver, ResultsDecl, Schema,
    StackConfig, StackDescriptor, Values,
};
use std::sync::{Arc, OnceLock};

/// How a scan of channels judges a TV multiplex: locked, named from the status summary.
pub const TV_LOCK_PROFILE: LockProfile = LockProfile {
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

/// The schema of a stack without settings of its own.
pub fn no_settings() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA.get_or_init(|| Arc::new(Schema::of(&[]))).clone()
}

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &isdbt::DESCRIPTOR,
        &isdbtsb::DESCRIPTOR,
        &atsc::DESCRIPTOR,
        &atsc3::DESCRIPTOR,
        &dtmb::DESCRIPTOR,
        &auto::DESCRIPTOR,
    ]
}

/// Adds every component of this crate to `r`, so a description can use them. A component that depends on the channel width
/// is made for the stack's default width (6 MHz for ISDB-T, 8 MHz for DTMB).
pub fn register(r: &mut sdrkit_graph::Registry) {
    use atsc::graph as a;
    use atsc3::components as a3;
    use isdbt::graph as i;
    r.add("isdbt.sync", || Box::new(i::SyncComp::default()));
    r.add("isdbt.equalise", || Box::new(i::EqComp::default()));
    r.add("isdbt.deinterleave", || Box::new(i::DeintComp::default()));
    r.add("isdbt.fec", || Box::new(i::FecComp::default()));
    r.add("isdbt.merge", || Box::new(i::MergeComp::default()));
    r.add("isdbt.report", || Box::new(i::Report::default()));
    r.add("atsc.pilot_find", || Box::new(a::PilotFind::default()));
    r.add("atsc.front", || Box::new(a::FrontComp::default()));
    r.add("atsc.pilot_loop", || Box::new(a::PilotLoopComp::default()));
    r.add("atsc.symbol_clock", || Box::new(a::ClockComp::default()));
    r.add("atsc.field_sync", || Box::new(a::FieldComp::default()));
    r.add("atsc.equalise", || Box::new(a::EqComp::default()));
    r.add("atsc.fec", || Box::new(a::FecComp::default()));
    r.add("atsc.report", || Box::new(atsc::Report::default()));
    r.add("atsc3.sync", || Box::new(a3::Sync::default()));
    r.add("atsc3.frame", || Box::new(a3::Frame::default()));
    r.add("atsc3.fec", || Box::new(a3::Fec::default()));
    r.add("atsc3.lls", || Box::new(a3::Lls::default()));
    r.add("atsc3.media", || Box::new(a3::Media::default()));
    r.add("atsc3.report", || {
        Box::new(atsc3::results::Report::default())
    });
    r.add("dtmb.front", || Box::new(dtmb::front::Front::default()));
    r.add("dtmb.search", || Box::new(dtmb::track::Search::default()));
    r.add("dtmb.track", || Box::new(dtmb::track::Track::default()));
    r.add("dtmb.report", || Box::new(dtmb::results::Report::default()));
    r.add("dtmb.body", || Box::new(dtmb::body::Body::default()));
    r.add("dtmb.si", || Box::new(dtmb::si::Si::default()));
    r.add("dtmb.fec", || Box::new(dtmb::fec::Fec::default()));
}

/// The declaration of stack `id`, `None` for an id of another crate.
pub fn declaration(id: &str) -> Option<ResultsDecl> {
    Some(match id {
        isdbt::ID => typed::shape(isdbt::ID).results_decl(),
        isdbtsb::ID => typed::shape(isdbtsb::ID).results_decl(),
        atsc::ID => typed::shape(atsc::ID).results_decl(),
        atsc3::ID => typed::shape(atsc3::ID).results_decl(),
        dtmb::ID => typed::shape(dtmb::ID).results_decl(),
        auto::ID => typed::shape(auto::ID).results_decl(),
        _ => return None,
    })
}

/// The benchmarks of this crate: each receiver on its own test signal.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "tvterr.atsc_receiver_8m",
            run: bench_atsc,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.atsc3_receiver_8m",
            run: bench_atsc3,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.dtmb_receiver_10m",
            run: bench_dtmb,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.dtmb_receiver_8m",
            run: bench_dtmb_8m,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.dtmb_noise_10m",
            run: bench_dtmb_noise,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.isdbt_receiver_8m",
            run: bench_isdbt,
        },
        sdrkit_core::Benchmark {
            name: "tvterr.isdbtsb_receiver_2m",
            run: bench_isdbtsb,
        },
    ]
}

fn bench_stack<G: Generator>(
    seconds: f64,
    mut g: G,
    rate: f64,
    n: usize,
    mut rx: Box<dyn Receiver>,
) -> sdrkit_core::BenchResult {
    let mut x = vec![sdrkit_core::Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let input = sdrkit_core::InputInfo::new(rate, 500e6, 500e6).recording();
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, n, n as f64 / rate, || {
        // The descriptor's own test signal and rate: the configuration is the same on every round.
        if rx.configure(&input).is_err() {
            return;
        }
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

fn config(d: &StackDescriptor) -> StackConfig {
    StackConfig::new(Values::defaults(&(d.settings)()))
}

fn bench_atsc(seconds: f64) -> sdrkit_core::BenchResult {
    let Ok(g) = atsc::gen::AtscGenerator::new(atsc::gen::AtscGenParams::default()) else {
        return sdrkit_core::BenchResult::default();
    };
    let Ok(rx) = (atsc::DESCRIPTOR.make_receiver)(&config(&atsc::DESCRIPTOR)) else {
        return sdrkit_core::BenchResult::default();
    };
    bench_stack(seconds, g, 8e6, 4_000_000, rx)
}

/// The ATSC 3.0 receiver on one second of its test signal at 8 Msps: the bootstrap search, the frames and the FEC blocks of
/// the PLP (QPSK, 8/15, 64800 bits).
fn bench_atsc3(seconds: f64) -> sdrkit_core::BenchResult {
    let Ok(g) = atsc3::Atsc3Generator::new(atsc3::Atsc3GenParams::default()) else {
        return sdrkit_core::BenchResult::default();
    };
    let Ok(rx) = (atsc3::DESCRIPTOR.make_receiver)(&config(&atsc3::DESCRIPTOR)) else {
        return sdrkit_core::BenchResult::default();
    };
    bench_stack(seconds, g, 8e6, 8_000_000, rx)
}

/// The DTMB receiver on half a second of the signal of `g` at `rate`: the search, the tracking and, on a signal, the decoding.
fn bench_dtmb_on<G: Generator>(seconds: f64, g: G, rate: f64) -> sdrkit_core::BenchResult {
    let Ok(rx) = (dtmb::DESCRIPTOR.make_receiver)(&config(&dtmb::DESCRIPTOR)) else {
        return sdrkit_core::BenchResult::default();
    };
    bench_stack(seconds, g, rate, (0.5 * rate) as usize, rx)
}

/// The DTMB test signal (PN945, 64QAM, rate 0.6) at `rate`.
fn bench_dtmb_signal(seconds: f64, rate: f64) -> sdrkit_core::BenchResult {
    let p = dtmb::gen::DtmbGenParams {
        rate_hz: rate,
        ..Default::default()
    };
    match dtmb::gen::DtmbGenerator::new(p) {
        Ok(g) => bench_dtmb_on(seconds, g, rate),
        Err(_) => sdrkit_core::BenchResult::default(),
    }
}

fn bench_dtmb(seconds: f64) -> sdrkit_core::BenchResult {
    bench_dtmb_signal(seconds, 10e6)
}

fn bench_dtmb_8m(seconds: f64) -> sdrkit_core::BenchResult {
    bench_dtmb_signal(seconds, 8e6)
}

/// White noise of power 0.04 at 10 Msps.
struct DtmbNoise(sdrkit_algo_sim::rng::Rng);

impl Generator for DtmbNoise {
    fn rate_hz(&self) -> f64 {
        10e6
    }
    fn generate(&mut self, out: &mut [sdrkit_core::Complex32]) -> usize {
        out.iter_mut()
            .for_each(|v| *v = self.0.complex_normal(0.04));
        out.len()
    }
}

/// Noise alone at 10 Msps: the search never finds a header.
fn bench_dtmb_noise(seconds: f64) -> sdrkit_core::BenchResult {
    bench_dtmb_on(seconds, DtmbNoise(sdrkit_algo_sim::rng::Rng::new(4)), 10e6)
}

fn bench_isdbt(seconds: f64) -> sdrkit_core::BenchResult {
    let Ok(g) = isdbt::gen::IsdbtGenerator::new(Default::default()) else {
        return sdrkit_core::BenchResult::default();
    };
    let Ok(rx) = (isdbt::DESCRIPTOR.make_receiver)(&config(&isdbt::DESCRIPTOR)) else {
        return sdrkit_core::BenchResult::default();
    };
    bench_stack(seconds, g, 8e6, 2_000_000, rx)
}

fn bench_isdbtsb(seconds: f64) -> sdrkit_core::BenchResult {
    let d = &isdbtsb::DESCRIPTOR;
    let settings = Values::defaults(&(d.settings)());
    let Ok(g) = isdbt::gen::IsdbtGenerator::new(isdbtsb::test_params(&settings, 2.048e6, 1)) else {
        return sdrkit_core::BenchResult::default();
    };
    let Ok(rx) = (d.make_receiver)(&config(d)) else {
        return sdrkit_core::BenchResult::default();
    };
    bench_stack(seconds, g, 2.048e6, 1_000_000, rx)
}
