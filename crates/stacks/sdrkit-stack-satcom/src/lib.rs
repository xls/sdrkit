//! Satellite communications: Inmarsat-C, Inmarsat Aero, Inmarsat (all carriers), Iridium, Orbcomm, Globalstar,
//! COSPAS-SARSAT 406 MHz, GOES DCS, Meteosat HRDCP, Argos, CubeSat telemetry, Fleet Broadcast, LoRa satellites, QO-100 and a PSK demodulator.
//! The CubeSat stack sends AX.25 UI frames for amateur satellite uplinks and the QO-100 stack sends SSB voice on the
//! narrowband uplink; every other stack receives only.
//! Each stack is a graph of components on the shared crates, with declared results and a generator of the clean test signal.

#![allow(clippy::needless_range_loop)] // DSP loops index several buffers at once

pub mod display;
pub mod modes;
pub mod typed;

use sdrkit_core::{ConfigError, Receiver, StackDescriptor};
use sdrkit_graph::{GraphError, StackGraph};

pub use modes::{benchmarks, register};

/// The recorded cost of each stage of the stacks that the test `cost` measures, written by that test.
pub const COSTS: &str = include_str!("../cost.txt");

/// Every stack of the family.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    modes::all()
}

/// A lower error that has no `ConfigError` variant of its own, as a stage error.
pub(crate) fn stage_error(e: impl std::error::Error + Send + Sync + 'static) -> ConfigError {
    ConfigError::Stage(sdrkit_core::StageError::new(e))
}

/// The receiver of `built`.
pub(crate) fn stack_receiver(
    built: Result<StackGraph, GraphError>,
) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}

/// The number in field `key` of `r`. `None` when the field is absent or not a number.
pub(crate) fn field_opt(r: &sdrkit_core::Results, key: &str) -> Option<f64> {
    use sdrkit_core::Value;
    let v = r.decl().field_id(key).and_then(|id| r.fields().value(id))?;
    match v {
        Value::I64(x) => Some(*x as f64),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

/// The number in field `key` of `r`, or 0 when it is absent or not a number.
pub(crate) fn field(r: &sdrkit_core::Results, key: &str) -> f64 {
    field_opt(r, key).unwrap_or(0.0)
}

/// Times the receiver of `desc` (in `submode`) on `signal_s` seconds of its own test signal at its rate.
pub(crate) fn bench_receiver(
    seconds: f64,
    desc: &'static StackDescriptor,
    submode: Option<&str>,
    signal_s: f64,
) -> sdrkit_core::BenchResult {
    use sdrkit_core::{BenchResult, Complex32, GenConfig, InputInfo, Output, StackConfig, Values};
    let settings = Values::defaults(&(desc.settings)());
    let rate = desc.tuning.rate_hz;
    let gc = GenConfig::new(rate, settings.clone())
        .with_seed(1)
        .with_submode_opt(submode.map(String::from));
    let Some(Ok(mut g)) = desc.make_generator.map(|make| make(&gc)) else {
        return BenchResult::default();
    };
    let mut x = vec![Complex32::default(); (rate * signal_s) as usize];
    g.generate(&mut x);
    let cfg = StackConfig::new(settings).with_submode_opt(submode.map(String::from));
    let Ok(mut rx) = (desc.make_receiver)(&cfg) else {
        return BenchResult::default();
    };
    // The station sits where the generator puts it: the centre is that far below the dial.
    let dial = desc.tuning.default_hz;
    let centre = dial - g.station_offset_hz(desc.tuning.tune_offset_hz);
    if rx
        .configure(&InputInfo::new(rate, centre, dial).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}
