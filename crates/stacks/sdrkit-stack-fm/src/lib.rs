//! FM broadcast radio with stereo, RDS and RBDS: the receiver graph ([`graph`]), its results ([`results`]), the sender
//! ([`transmit`]) and the test station ([`generator`]) on one multiplex ([`mpx`]), the band scan ([`scan`]) and the band
//! plans ([`bands`]). [`DESCRIPTOR`] describes the stack to the engine. The studio-to-transmitter links of broadcast
//! stations ([`stl`]) run the same receiver.
//! Standard: ITU-R BS.450, IEC 62106, NRSC-4-B, ISO 14819.

#![warn(missing_docs)]

pub mod audio;
pub mod bands;
mod bench;
pub mod channel;
pub mod finder;
pub mod fmdemod;
pub mod frontend;
pub mod generator;
pub mod graph;
pub mod measure;
pub mod mpx;
pub mod pilot;
pub mod pty;
pub mod rds;
pub mod results;
pub mod scan;
pub mod settings;
pub mod stereo;
pub mod stl;
pub mod typed;
pub mod transmit;
pub mod verify;

pub use bench::benchmarks;
pub use generator::{FmGenerator, GenParams};
pub use results::Reading;
pub use settings::{FmSetting, StereoChoice};

use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockCondition, LockProfile, LockState, NameSource,
    Receiver, SettingSet, SignalType, StackConfig, StackDescriptor, Tuning,
};

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(graph::receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(FmGenerator::from_config(cfg)))
}

/// The lowest input rate the receiver accepts.
pub use graph::MIN_RATE_HZ;

/// How a scan of channels judges an FM channel: tracking with 12 dB SNR, the name from the station field.
const LOCK_PROFILE: LockProfile = LockProfile {
    settle_s: 0.1,
    quiet_after_s: Some(0.4),
    signal_snr_db: 10.0,
    timeout_s: 1.0,
    lock: LockCondition::All(&[
        LockCondition::LockState {
            at_least: LockState::Tracking,
        },
        LockCondition::Snr { min_db: 12.0 },
    ]),
    collect_s: 4.0,
    name_from: &[NameSource::Field("station")],
    capture: &["pi", "programme", "text"],
};

/// The stacks of this crate: FM broadcast and STL links.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DESCRIPTOR, &stl::DESCRIPTOR]
}

/// The FM broadcast stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: graph::ID,
    name: "FM radio",
    group: "Broadcast radio",
    blurb: "FM broadcast stations in stereo, with the station name and radio text from RDS.",
    tuning: Tuning {
        min_hz: 65.8e6,
        max_hz: 108e6,
        default_hz: 100e6,
        rate_hz: 4e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 250e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: FmSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[SignalType::FmBroadcast],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::HOOKS),
    benchmarks: Some(benchmarks),
};
