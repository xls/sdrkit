//! Science and time stacks: radiosondes, time signals, e-Czas, NOAA SAME, meteor scatter, SSDV pictures, radio astronomy and
//! ionospheric sounders. Each stack is a graph of components with typed ports, declares what it publishes, and has a test
//! signal.
//!
//! | Id | Stack | Module |
//! |---|---|---|
//! | `sonde` | Radiosondes (RS41, RS92, DFM, M10, M20, LMS6) | [`sonde`] |
//! | `time` | Time signals (DCF77, MSF, WWVB, JJY, BPC, WWV, BPM, HLA, RWM) | [`time`] |
//! | `eczas` | e-Czas, 225 kHz Poland | [`eczas`] |
//! | `same` | NOAA Weather Radio SAME | [`same`] |
//! | `mscatter` | Meteor scatter | [`mscatter`] |
//! | `ssdv` | SSDV pictures | [`ssdv`] |
//! | `astro` | Radio astronomy | [`astro`] |
//! | `iono` | Ionospheric sounders | [`iono`] |
//! | `schumann` | Schumann resonances | [`schumann`] |
//! | `whistler` | Whistlers and tweeks | [`whistler`] |
//! | `jove` | Radio Jove | [`bursts`] |
//! | `callisto` | Solar burst spectrogram | [`bursts`] |
//! | `riometer` | Riometer | [`riometer`] |
//! | `eme_echo` | Moon echo test | [`eme`] |
//! | `beacon_events` | Beacon openings | [`beacon`] |
//! | `pulsar` | Pulsar folding | [`pulsar`] |
//! | `radar_detect` | Pulse radar (detect) | [`radar`] |
//! | `horus` | Horus balloons (Horus Binary v1, v2 and RTTY) | [`horus`] |

pub mod astro;
pub mod beacon;
pub mod bench;
pub mod bursts;
pub mod calendar;
pub mod clock;
pub mod eczas;
pub mod eme;
pub mod events;
pub mod family;
pub mod horus;
pub mod iono;
pub mod lightning;
pub mod mscatter;
pub mod pulsar;
pub mod radar;
pub mod riometer;
pub mod same;
pub mod schumann;
pub mod sferics;
pub mod sonde;
pub mod ssdv;
pub mod supersid;
pub mod time;
pub mod typed;
pub mod whistler;

use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, LockProfile, Receiver, SettingSet, StackDescriptor, Tuning,
};
use sdrkit_graph::{GraphError, StackGraph};

/// Radiosondes.
pub static SONDE: StackDescriptor = StackDescriptor {
    id: "sonde",
    name: "Radiosonde",
    group: "Science, time and utility",
    blurb: "Weather balloons: RS41, RS92, DFM, M10, M20 and LMS6 positions, heights, sensors and landing predictions.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 380e6,
        max_hz: 1700e6,
        default_hz: 403e6,
        rate_hz: 8e6,
        min_rate_hz: 250e3,
        bandwidth_hz: 6e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(7e6),
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: sonde::SondeSetting::schema,
    make_receiver: sonde::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(sonde::make_generator),
    signal_types: &[SignalType::Radiosonde],
    auto_gain: AutoGain::Radio,
    verify: Some(&sonde::VERIFY),
    benchmarks: Some(bench::benchmarks),
};

/// NOAA Weather Radio SAME.
pub static SAME: StackDescriptor = StackDescriptor {
    id: "same",
    name: "NOAA Weather Radio SAME",
    group: "Science, time and utility",
    blurb:
        "Emergency and weather alerts (SAME headers) on the US and Canadian weather radio channels.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Nfm, 12.5e3)),
        min_hz: 162.400e6,
        max_hz: 162.550e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 64e3,
        bandwidth_hz: 25e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: same::SameSetting::schema,
    make_receiver: same::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(same::make_generator),
    signal_types: &[SignalType::Same],
    auto_gain: AutoGain::Radio,
    verify: Some(&same::VERIFY),
    benchmarks: None,
};

/// e-Czas.
pub static ECZAS: StackDescriptor = StackDescriptor {
    id: "eczas",
    name: "e-Czas (225 kHz, Poland)",
    group: "Science, time and utility",
    blurb: "The official Polish time from the phase keying of the 225 kHz long-wave carrier.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 0.2e6,
        max_hz: 0.25e6,
        default_hz: 225e3,
        rate_hz: 256e3,
        min_rate_hz: 8e3,
        bandwidth_hz: 10e3,
        tune_offset_hz: 12e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: eczas::EczasSetting::schema,
    make_receiver: eczas::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(eczas::make_generator),
    signal_types: &[SignalType::TimeSignal],
    auto_gain: AutoGain::Radio,
    verify: Some(&eczas::VERIFY),
    benchmarks: None,
};

/// Meteor scatter.
pub static MSCATTER: StackDescriptor = StackDescriptor {
    id: "mscatter",
    name: "Meteor scatter",
    group: "Science, time and utility",
    blurb:
        "Counts and measures meteor echoes of a CW beacon such as the GRAVES radar on 143.050 MHz.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 30e6,
        max_hz: 300e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: 24e3,
        bandwidth_hz: 8e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: mscatter::MscatterSetting::schema,
    make_receiver: mscatter::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(mscatter::make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&mscatter::VERIFY),
    benchmarks: None,
};

/// Time signals.
pub static TIME: StackDescriptor = StackDescriptor {
    id: "time",
    name: "Time signals",
    group: "Science, time and utility",
    blurb: "The time and date from DCF77, MSF, WWVB, JJY, BPC, WWV/WWVH, BPM, HLA and RWM, compared with your clock.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Am, 6e3)),
        min_hz: 0.03e6,
        max_hz: 30e6,
        // 77.5 kHz: the DCF77 carrier (PTB), the station of the test signal; the automatic station choice needs it.
        default_hz: 77.5e3,
        rate_hz: 256e3,
        min_rate_hz: 8e3,
        bandwidth_hz: 10e3,
        tune_offset_hz: 12e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: time::TimeSetting::schema,
    make_receiver: time::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(time::make_generator),
    signal_types: &[SignalType::TimeSignal],
    auto_gain: AutoGain::Radio,
    verify: Some(&time::VERIFY),
    benchmarks: None,
};

/// SSDV pictures.
pub static SSDV: StackDescriptor = StackDescriptor {
    id: "ssdv",
    name: "SSDV pictures",
    group: "Science, time and utility",
    blurb: "Pictures from high-altitude balloons and small satellites sent as SSDV packets.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 30e6,
        max_hz: 1000e6,
        default_hz: 434.650e6,
        rate_hz: 2e6,
        min_rate_hz: 250e3,
        bandwidth_hz: 50e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: ssdv::SsdvSetting::schema,
    make_receiver: ssdv::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(ssdv::make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&ssdv::VERIFY),
    benchmarks: None,
};

/// Radio astronomy.
pub static ASTRO: StackDescriptor = StackDescriptor {
    id: "astro",
    name: "Radio astronomy",
    group: "Science, time and utility",
    blurb: "The hydrogen line (and OH lines): long integrations, velocities, calibration, line fits and maps of the Milky Way.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 10e6,
        max_hz: 6000e6,
        default_hz: 1_420_405_751.0,
        rate_hz: 2.5e6,
        min_rate_hz: 250e3,
        bandwidth_hz: 2.5e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: astro::AstroSetting::schema,
    make_receiver: astro::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(astro::make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&astro::VERIFY),
    benchmarks: None,
};

/// Ionospheric sounders.
pub static IONO: StackDescriptor = StackDescriptor {
    id: "iono",
    name: "Ionospheric sounders",
    group: "Science, time and utility",
    blurb: "Chirp sounders: finds them, builds their ionograms and scales them (foE, foF2, MUF, path MUF) to tell which bands are open.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.1e6,
        max_hz: 40e6,
        default_hz: 10e6,
        rate_hz: 20e6,
        min_rate_hz: 250e3,
        bandwidth_hz: 18e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: iono::IonoSetting::schema,
    make_receiver: iono::make_receiver,
    lock_profile: LockProfile::DEFAULT,
    make_generator: Some(iono::make_generator),
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&iono::VERIFY),
    benchmarks: None,
};

/// Every stack of the crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &SONDE, &TIME, &ECZAS, &SAME, &MSCATTER, &SSDV, &ASTRO, &IONO,
        // VLF and lightning monitors.
        &sferics::DESCRIPTOR,
        &supersid::DESCRIPTOR,
        &schumann::DESCRIPTOR,
        &whistler::DESCRIPTOR,
        &lightning::DESCRIPTOR,
        // Radio science monitors.
        &bursts::JOVE,
        &bursts::CALLISTO,
        &riometer::DESCRIPTOR,
        &eme::DESCRIPTOR,
        &beacon::DESCRIPTOR,
        &pulsar::DESCRIPTOR,
        &radar::DESCRIPTOR,
        // Balloon telemetry.
        &horus::DESCRIPTOR,
    ]
}

/// Adds every component of the crate to `r`, each under the name its `info()` returns.
pub fn register(r: &mut sdrkit_graph::Registry) {
    r.add("sonde.search", || {
        Box::new(sonde::search::CarrierSearch::default())
    });
    r.add("sonde.bank", || {
        Box::new(sonde::channel::ChannelBank::default())
    });
    r.add("sonde.table", || {
        Box::new(sonde::table::SondeTable::default())
    });
    r.add("time.front", || Box::new(time::Front::default()));
    r.add("time.seconds", || Box::new(time::Seconds::default()));
    r.add("time.minutes", || Box::new(time::Minutes::default()));
    r.add("eczas.demod", || Box::new(eczas::Demod::default()));
    r.add("eczas.slicer", || Box::new(eczas::Slicer::default()));
    r.add("eczas.keeper", || Box::new(eczas::Keeper::default()));
    r.add("same.fm", || Box::new(same::FmAudio::default()));
    r.add("same.fm_narrow", || Box::new(same::FmAudio::narrow()));
    r.add("same.afsk", || Box::new(same::AfskReader::default()));
    r.add("same.tone", || Box::new(same::ToneDetector::default()));
    r.add("same.alerts", || Box::new(same::Alerts::default()));
    r.add("same.listen", || Box::new(same::Listen::default()));
    r.add("mscatter.detector", || {
        Box::new(mscatter::Detector::default())
    });
    r.add("mscatter.counts", || {
        Box::new(mscatter::PingTable::default())
    });
    r.add("ssdv.fm", || Box::new(ssdv::FmFront::default()));
    r.add("ssdv.slicers", || Box::new(ssdv::Slicers::default()));
    r.add("ssdv.pictures", || Box::new(ssdv::Pictures::default()));
    r.add(
        "astro.integrator",
        || Box::new(astro::Integrator::default()),
    );
    r.add("astro.processor", || Box::new(astro::Processor::default()));
    iono::register(r);
    // VLF and lightning monitors.
    sferics::register(r);
    supersid::register(r);
    schumann::register(r);
    whistler::register(r);
    lightning::register(r);
    // Radio science monitors.
    bursts::register(r);
    riometer::register(r);
    eme::register(r);
    beacon::register(r);
    pulsar::register(r);
    radar::register(r);
    // Balloon telemetry.
    horus::register(r);
}

/// The receiver of a stack graph.
fn stack_receiver(built: Result<StackGraph, GraphError>) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(built?))
}
