//! CODAR / HF ocean radar, receive only.
//!
//! ```text
//! iq -> [Hann FFT ~1 ms, floor, burst guard, peaks, tracks] --+--> hfradar.sweep --+--> [radars, events, waterfall, Morse ID] -> results
//!                                                              +--> excess 5 ms ----+
//! ```

pub mod bands;
pub mod detect;
pub mod gen;
pub mod radars;

use crate::receiver::GEN_SECTION;
use gen::RadarSignal;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// The signal of the test signal.
pub const GEN_SIGNAL: Setting<field::Choice<RadarSignal>> =
    Setting::choice("gen_signal", "Signal", RadarSignal::Radars)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the ocean radar stack. The noise of the test signal comes from the standard `gen_*` settings.
    pub enum RadarStackSetting {
        /// Scan the radar bands.
        Scan = radars::SCAN,
        /// Dwell.
        Dwell = radars::DWELL,
        /// Signal of the test signal.
        GenSignal = GEN_SIGNAL,
    }
}

/// The ocean radar receiver graph.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("detect", detect::SweepDetector::default())
        .add("radars", radars::RadarTable::default())
        .input("detect")
        .connect("detect.sweeps", "radars.sweeps")
        .connect("detect.excess", "radars.excess")
        .output("radars.events")
        .output("radars.requests")
        .output("radars.waterfall")
        .clock(0.25)
        .status_from("radars")
}

/// The ocean radar receiver. With `scan` on and a live radio it requests the host to step through the radar bands.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("hfradar"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        240e3,
        "CODAR: the sample rate is too low (240 kHz at least)",
    )
    .route("scan", &["radars.scan"])?
    .route("dwell_s", &["radars.dwell_s"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command(
        "band.tune",
        sdrkit_graph::CommandTarget::of("radars", "band.tune"),
    )
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::RadarGenerator::from_config(cfg)))
}

/// The benchmarks of this stack: its entries of the crate's list.
fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    crate::benchmarks()
        .into_iter()
        .filter(|b| b.name.starts_with("hfradar."))
        .collect()
}

/// CODAR / HF ocean radar.
pub static HFRADAR: StackDescriptor = StackDescriptor {
    id: "hfradar",
    name: "CODAR / HF ocean radar",
    group: "HF utility",
    blurb: "Coastal HF ocean radars (CODAR SeaSonde, WERA): their FMCW sweeps, repetition, band and keyed ID (receive only).",
    tuning: Tuning::anywhere(13.5e6, 2e6, 240e3, 2e6),
    submodes: &[],
    settings: RadarStackSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::HfOceanRadar],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HFRADAR),
    benchmarks: Some(benchmarks),
};
