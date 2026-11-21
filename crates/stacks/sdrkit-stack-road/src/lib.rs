//! Road vehicle links: 802.11p V2X frames with SAE J2735 basic safety messages and ETSI cooperative awareness messages, on the
//! 802.11 OFDM receiver of `sdrkit-proto-wlan-phy` at half clock.

#![warn(missing_docs)]

pub mod typed;
pub mod v2x;

use sdrkit_core::publish::Declare;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, GenConfig, InputInfo, LockProfile, Output,
    Results, SettingSet, StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::Registry;

/// The frequency of the V2X test signal, Hz. The session gives the frequency of a real channel.
pub const V2X_TEST_SIGNAL_HZ: f64 = 5.9e9;

/// 802.11p V2X.
pub static V2X: StackDescriptor = StackDescriptor {
    id: v2x::ID,
    name: "802.11p V2X",
    group: "Road",
    blurb: "Road vehicles and roadside units on a 10 MHz 802.11p channel: basic safety messages (SAE J2735) and cooperative \
            awareness messages (ETSI) with their positions on the map; hazard warnings counted.",
    tuning: Tuning::anywhere(V2X_TEST_SIGNAL_HZ, 10e6, v2x::MIN_RATE, 10e6),
    submodes: &[],
    settings: v2x::gen::V2xSetting::schema,
    make_receiver: v2x::make_receiver,
    make_generator: Some(v2x::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&V2X_VERIFY),
    benchmarks: Some(benchmarks),
};

fn v2x_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal", o, 40e3);
    c.seconds = c.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.check = Some(|r: &Results| {
        let n = r.fields().at(&typed::V2xFields::SAFETY_MESSAGES).unwrap_or(0);
        vec![CheckLine::new(
            Declare::key(&typed::V2xFields::SAFETY_MESSAGES),
            n >= 4,
            format!("{n} safety messages (at least 4)"),
        )]
    });
    vec![c]
}

/// The verify case of `v2x`.
pub static V2X_VERIFY: VerifyHooks = VerifyHooks {
    cases: v2x_cases,
    vector_settings: None,
    check_vector: None,
};

/// Every stack of the crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&V2X]
}

/// Adds the crate's components to `r`.
pub fn register(r: &mut Registry) {
    r.add("road.v2x", || {
        Box::new(sdrkit_graph::DecoderComponent::new(
            v2x::V2xDecoder::default(),
        ))
    });
}

fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    rate: f64,
    signal_s: f64,
) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let Some(Ok(mut g)) = desc
        .make_generator
        .map(|make| make(&GenConfig::new(rate, settings.clone())))
    else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    let f = desc.tuning.default_hz;
    let configured =
        rx.configure(&InputInfo::new(rate, f + desc.tuning.tune_offset_hz, f).recording());
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        if configured.is_ok() {
            for c in x.chunks(65_536) {
                rx.feed(c, &mut out);
                out.clear();
            }
        }
    })
}

/// The crate's benchmarks: each receiver on its own test signal at the rate in the name.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "road.v2x_10000k",
        run: |s| bench_receiver(s, &V2X, 10e6, 1.0),
    }]
}
