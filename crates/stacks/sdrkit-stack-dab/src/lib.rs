//! DAB and DAB+ digital radio: the receiver graph ([`receiver`]), its results ([`results`]), the test ensemble
//! ([`generator`]), the channel table ([`channels`]) and the ensemble scan ([`scan`]). [`DESCRIPTOR`] describes the stack
//! to the engine.
//!
//! Standard: EN 300 401, TS 102 563, TS 101 756.

#![warn(missing_docs)]
// Signal-processing loops index several arrays at once; ranges read better there than zipped iterators.
#![allow(clippy::needless_range_loop)]

pub mod announce;
pub mod audio;
mod bench;
pub mod bits;
pub mod channels;
pub mod components;
pub mod data;
pub mod ensemble;
pub mod fec;
pub mod fic;
pub mod fig;
pub mod follow;
pub mod generator;
pub mod health;
pub mod msc;
pub mod ofdm;
pub mod receiver;
pub mod registry;
pub mod results;
pub mod scan;
pub mod settings;
pub mod tables;
pub mod typed;
pub mod verify;

pub use bench::benchmarks;
pub use components::register;
pub use generator::{DabGenerator, GenParams};
pub use receiver::MIN_RATE_HZ;
pub use settings::DabSetting;

use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockCondition, LockProfile, LockState, NameSource,
    Receiver, SettingSet, SignalType, StackConfig, StackDescriptor, Tuning,
};

/// The DAB receiver ([`receiver::receiver`]).
fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver::receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(DabGenerator::from_config(cfg)))
}

/// How a scan of channels judges a DAB channel: locked, the name from the ensemble label.
const LOCK_PROFILE: LockProfile = LockProfile {
    settle_s: 0.2,
    quiet_after_s: Some(1.5),
    signal_snr_db: 4.0,
    timeout_s: 5.0,
    lock: LockCondition::LockState {
        at_least: LockState::Locked,
    },
    collect_s: 3.0,
    name_from: &[NameSource::Field("ensemble_label")],
    capture: &["snr_db"],
};

/// The DAB and DAB+ stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: receiver::ID,
    name: "DAB / DAB+",
    group: "Broadcast",
    blurb: "Digital radio ensembles: DAB+ and DAB audio with labels, slideshow, programme guide, Journaline and DMB video.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 174e6,
        max_hz: 1_492e6,
        default_hz: 0.0,
        rate_hz: 2.048e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 1.7e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: DabSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[SignalType::Dab],
    auto_gain: AutoGain::Hold,
    verify: Some(&verify::HOOKS),
    benchmarks: Some(benchmarks),
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DESCRIPTOR]
}
