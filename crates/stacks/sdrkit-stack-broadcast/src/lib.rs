//! Digital and stereo broadcast radio: DRM (DRM30 and DRM+), HamDRM, HD Radio (NRSC-5), CDR and AM stereo (C-QUAM). Each
//! stack's [`sdrkit_core::StackDescriptor`] is a `pub static`. [`descriptors`] lists them.

#![warn(missing_docs)]

pub mod cdr;
pub mod component;
pub mod cquam;
pub mod datafile;
pub mod drm;
pub mod hamdrm;
pub mod hd;
pub mod qam;
pub mod scan;
pub mod silence;
pub mod typed;
pub mod verify;

use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, LockCondition, LockProfile, LockState, NameSource, SettingSet, StackDescriptor,
    Tuning,
};

/// A lock profile of a broadcast stack: a lock state of at least `lock_state` within `timeout_s`, the name collected for `collect_s`.
const fn lock(
    lock_state: LockState,
    timeout_s: f64,
    collect_s: f64,
    name_from: &'static [NameSource],
) -> LockProfile {
    LockProfile {
        settle_s: 0.2,
        quiet_after_s: Some(1.5),
        signal_snr_db: 4.0,
        timeout_s,
        lock: LockCondition::LockState {
            at_least: lock_state,
        },
        collect_s,
        name_from,
        capture: &["snr_db"],
    }
}

/// AM stereo (C-QUAM).
pub static CQUAM: StackDescriptor = StackDescriptor {
    id: cquam::ID,
    name: "AM stereo (C-QUAM)",
    group: "Broadcast",
    blurb: "Medium-wave AM stereo stations using C-QUAM, with the 25 Hz pilot, stereo meters and separation.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 30e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: cquam::MIN_RATE,
        bandwidth_hz: 20e3,
        tune_offset_hz: cquam::TUNE_OFFSET_HZ,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: cquam::CquamSetting::schema,
    make_receiver: cquam::make_receiver,
    make_generator: Some(cquam::make_generator),
    // The carrier within 0.1 s, the 25 Hz pilot within about 2 s. An AM carrier is already a station.
    lock_profile: lock(LockState::Tracking, 3.0, 0.0, &[]),
    signal_types: &[SignalType::AmBroadcast],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::CQUAM),
    benchmarks: None,
};

/// DRM (DRM30 and DRM+).
pub static DRM: StackDescriptor = StackDescriptor {
    id: drm::ID,
    name: "DRM",
    group: "Digital radio",
    blurb: "Digital Radio Mondiale on short wave (DRM30) and VHF (DRM+): services, text messages, slide shows and Journaline.",
    tuning: Tuning {
        min_hz: 0.1e6,
        max_hz: 240e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: drm::MIN_RATE,
        bandwidth_hz: 20e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: drm::DrmSetting::schema,
    make_receiver: drm::make_receiver,
    make_generator: Some(drm::make_generator),
    // The FAC needs up to 14 frames (5.6 s), the label one super frame more.
    lock_profile: lock(LockState::Locked, 9.0, 3.0, &[NameSource::Summary]),
    signal_types: &[SignalType::Drm],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::DRM),
    benchmarks: Some(benchmarks),
};

/// HamDRM.
pub static HAMDRM: StackDescriptor = StackDescriptor {
    id: hamdrm::ID,
    name: "HamDRM",
    group: "Amateur digital",
    blurb:
        "Digital picture transfer between amateur stations on a DRM-style modem in an SSB channel.",
    tuning: Tuning {
        min_hz: 0.0,
        max_hz: 1e12,
        default_hz: 0.0,
        rate_hz: 48e3,
        min_rate_hz: hamdrm::MIN_RATE,
        bandwidth_hz: 3e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: hamdrm::HamSetting::schema,
    make_receiver: hamdrm::make_receiver,
    make_generator: Some(hamdrm::make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::HamDrm],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::HAMDRM),
    benchmarks: None,
};

/// HD Radio (NRSC-5).
pub static HDRADIO: StackDescriptor = StackDescriptor {
    id: hd::ID,
    name: "HD Radio",
    group: "Digital radio",
    blurb: "HD Radio (NRSC-5) on FM and AM: station information, programs HD1 to HD8, song titles and album art.",
    tuning: Tuning {
        min_hz: 0.52e6,
        max_hz: 108e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: hd::MIN_RATE_AM,
        bandwidth_hz: 400e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: hd::HdSetting::schema,
    make_receiver: hd::make_receiver,
    make_generator: Some(hd::make_generator),
    // Block sync within a fraction of a second, a P1 frame every 1.486 s, the station name from the PIDS.
    lock_profile: lock(
        LockState::Locked,
        7.0,
        8.0,
        &[
            NameSource::Field("call_sign"),
            NameSource::Field("long_name"),
        ],
    ),
    signal_types: &[SignalType::HdRadio],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::HDRADIO),
    benchmarks: None,
};

/// CDR (China Digital Radio).
pub static CDR: StackDescriptor = StackDescriptor {
    id: cdr::ID,
    name: "CDR",
    group: "Digital radio",
    blurb: "China Digital Radio (GY/T 268) in the FM band: the multiplex, its services, the network table and DRA+ audio units.",
    tuning: Tuning {
        min_hz: 87e6,
        max_hz: 108e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: cdr::MIN_RATE,
        bandwidth_hz: 500e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: cdr::CdrSetting::schema,
    make_receiver: cdr::make_receiver,
    make_generator: Some(cdr::make_generator),
    // A sub-frame is 160 ms and carries the system information. The network table comes with the multiplex.
    lock_profile: lock(LockState::Locked, 6.0, 3.0, &[NameSource::Field("network")]),
    signal_types: &[SignalType::Cdr],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::CDR),
    benchmarks: None,
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DRM, &HAMDRM, &HDRADIO, &CDR, &CQUAM]
}

/// The benchmarks of this crate.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    verify::benchmarks()
}
