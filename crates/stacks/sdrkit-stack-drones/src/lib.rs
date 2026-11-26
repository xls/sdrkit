//! Drones and glider tracking: Drone Remote ID on Bluetooth LE and Wi-Fi, DJI DroneID and drone links by their shape, the
//! old FLARM protocol, ADS-L, the OGN tracker, 2.4 GHz radio-control links and MAVLink telemetry on SiK radios. Every stack is
//! receive only and has a test-signal generator.

#![warn(missing_docs)]
// Signal-processing loops index several arrays by one sample position.
#![allow(clippy::needless_range_loop, clippy::too_many_arguments)]

pub mod adsl;
pub mod chips;
pub mod dji;
pub mod family;
pub mod flarm;
pub mod fsklink;
pub mod links;
pub mod mavlink_sik;
pub mod ogn;
pub mod rc_links;
pub mod remote_id;
pub mod typed;

pub use mavlink_sik::MAVLINK_SIK;
pub use rc_links::RC_LINKS;

use sdrkit_core::interface::Column;
use sdrkit_core::publish::{field, Declare};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockProfile, Output, Receiver, Results, SettingSet, StackConfig, StackDescriptor, Tuning,
    Values,
};

/// The heading of the stack list.
pub const GROUP: &str = "Aircraft and drones";

fn make_rid(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(remote_id::receiver(cfg)?))
}
fn make_rid_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        remote_id::generator::RemoteIdGenerator::from_config(cfg),
    ))
}

/// Drone Remote ID.
pub static REMOTE_ID: StackDescriptor = StackDescriptor {
    id: "remote_id",
    name: "Drone Remote ID",
    group: GROUP,
    blurb: "The identification drones broadcast openly on Bluetooth LE and Wi-Fi (ASTM F3411 / EN 4709-002): serial, position, \
            operator.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 2400e6,
        max_hz: 2500e6,
        bandwidth_hz: 20e6,
        no_offset_tuning: true,
        ..Tuning::anywhere(2432e6, 20e6, remote_id::MIN_RATE_HZ, 20e6)
    },
    submodes: &[],
    settings: remote_id::RidSetting::schema,
    make_receiver: make_rid,
    make_generator: Some(make_rid_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::BluetoothLe, SignalType::Wifi],
    auto_gain: AutoGain::Radio,
    verify: Some(&RID_VERIFY),
    benchmarks: Some(benchmarks),
};

fn make_dji(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(dji::receiver(cfg)?))
}
fn make_dji_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(dji::generator::DjiGenerator::from_config(cfg)))
}

/// DJI drones.
pub static DJI_DRONEID: StackDescriptor = StackDescriptor {
    id: "dji_droneid",
    name: "DJI drones",
    group: GROUP,
    blurb: "DJI drones: DroneID packets (serial number, position, home and pilot) and drone video links recognised by their \
            shape. Nothing is decrypted.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 2300e6,
        max_hz: 6000e6,
        bandwidth_hz: 20e6,
        no_offset_tuning: true,
        ..Tuning::anywhere(2440e6, 20e6, dji::MIN_RATE_HZ, 20e6)
    },
    submodes: &[],
    settings: dji::DjiSetting::schema,
    make_receiver: make_dji,
    make_generator: Some(make_dji_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::DjiDroneId],
    auto_gain: AutoGain::Radio,
    verify: Some(&DJI_VERIFY),
    benchmarks: None,
};

fn make_flarm(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(flarm::receiver(cfg)?))
}
fn make_flarm_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(flarm::generator::from_config(cfg)))
}

/// FLARM.
pub static FLARM: StackDescriptor = StackDescriptor {
    id: "flarm",
    name: "FLARM",
    group: GROUP,
    blurb: "Gliders and light aircraft on 868 MHz: the old FLARM protocol's positions, privacy flags respected.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 860e6,
        max_hz: 930e6,
        bandwidth_hz: 0.4e6,
        no_offset_tuning: true,
        ..Tuning::anywhere(868.3e6, 1e6, flarm::MIN_RATE_HZ, 0.4e6)
    },
    submodes: &[],
    settings: flarm::FlarmSetting::schema,
    make_receiver: make_flarm,
    make_generator: Some(make_flarm_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Flarm],
    auto_gain: AutoGain::Radio,
    verify: Some(&FLARM_VERIFY),
    benchmarks: None,
};

fn make_adsl(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(adsl::receiver(cfg)?))
}
fn make_adsl_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(adsl::generator::from_config(cfg)))
}
/// ADS-L.
pub static ADSL: StackDescriptor = StackDescriptor {
    id: "adsl",
    name: "ADS-L",
    group: GROUP,
    blurb: "ADS-L traffic, status and ground uplink of light aircraft on 868.2, 868.4 and 869.525 MHz from one capture, and \
            FLARM from the same capture.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 860e6,
        max_hz: 930e6,
        bandwidth_hz: 1.75e6,
        no_offset_tuning: true,
        ..Tuning::anywhere(adsl::channels::WIDE_CENTRE_HZ, 2e6, adsl::MIN_RATE_HZ, 1.75e6)
    },
    submodes: &[],
    settings: adsl::AdslSetting::schema,
    make_receiver: make_adsl,
    make_generator: Some(make_adsl_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&ADSL_VERIFY),
    benchmarks: None,
};
fn make_ogn(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(ogn::receiver(cfg)?))
}

fn make_ogn_gen(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(ogn::generator::from_config(cfg)))
}

/// OGN tracker.
pub static OGN: StackDescriptor = StackDescriptor {
    id: "ogn",
    name: "OGN tracker",
    group: GROUP,
    blurb: "Position packets of Open Glider Network trackers on the channels of one capture: address, position, altitude, \
            speed, heading, climb and aircraft type. Packets with an encrypted position are counted. Experimental: the sync \
            word, the code table, the whitening and the field layout are not confirmed by a public document.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 860e6,
        max_hz: 930e6,
        bandwidth_hz: 0.6e6,
        no_offset_tuning: true,
        ..Tuning::anywhere(868.3e6, 1e6, ogn::MIN_RATE_HZ, 0.6e6)
    },
    submodes: &[],
    settings: ogn::OgnSetting::schema,
    make_receiver: make_ogn,
    make_generator: Some(make_ogn_gen),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&OGN_VERIFY),
    benchmarks: None,
};

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![
        &REMOTE_ID,
        &DJI_DRONEID,
        &FLARM,
        &ADSL,
        &OGN,
        &RC_LINKS,
        &MAVLINK_SIK,
    ]
}

/// Times the receiver of `desc` on `signal_s` seconds of its own test signal at its default rate.
pub(crate) fn bench_receiver(seconds: f64, desc: &'static StackDescriptor, signal_s: f64) -> BenchResult {
    let settings = Values::defaults(&(desc.settings)());
    let rate = desc.tuning.rate_hz;
    let Ok(mut g) = (desc.make_generator.expect("every stack has a generator"))(&GenConfig::new(rate, settings.clone()).with_seed(1)) else {
        return BenchResult::default();
    };
    let n = (rate * signal_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    let Ok(mut rx) = (desc.make_receiver)(&StackConfig::new(settings)) else {
        return BenchResult::default();
    };
    // A receiver that refuses the rate decodes nothing, and the benchmark then times the refusal.
    let _ = rx.configure(
        &InputInfo::new(rate, desc.tuning.default_hz, desc.tuning.default_hz).recording(),
    );
    let mut out = Output::default();
    sdrkit_core::bench::measure(seconds, n, signal_s, || {
        for c in x.chunks(65536) {
            rx.feed(c, &mut out);
            out.clear();
        }
    })
}

/// The pair trigger of `dji_droneid` alone on 0.2 s of noise at 20 Msps, with one DroneID frequency 4.5 MHz above the centre
/// (as at the default frequency), so the mixer runs too. Items are input samples.
fn bench_trigger(seconds: f64) -> BenchResult {
    let rate = DJI_DRONEID.tuning.rate_hz;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
    let x: Vec<Complex32> = (0..(rate * 0.2) as usize)
        .map(|_| rng.complex_normal(0.01))
        .collect();
    let mut t = dji::trigger::PairTrigger::new(rate, [4.5e6].into_iter());
    let mut hits = Vec::new();
    sdrkit_core::bench::measure(seconds, x.len(), 0.2, || {
        for c in x.chunks(65_536) {
            t.feed(c, &mut hits);
            hits.clear();
        }
    })
}

/// One burst of the pair trigger of `dji_droneid` that is too weak to decode (5 dB below the noise in its 9 MHz): every
/// trial of the burst receiver runs to its last turbo iteration. Items are bursts, so the time of one is 1 / items per second.
fn bench_weak_burst(seconds: f64) -> BenchResult {
    use dji::droneid;
    let burst = droneid::modulate(&dji::generator::packet(0, 0.0).frame(), 0, true);
    let power = burst.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / burst.len() as f64;
    let noise = power * 10f64.powf(0.5) * droneid::FS / 9e6;
    let mut rng = sdrkit_algo_sim::rng::Rng::new(1);
    let margin = 300;
    let mut x: Vec<Complex32> = (0..burst.len() + 2 * margin)
        .map(|_| rng.complex_normal(noise))
        .collect();
    for (o, v) in x[margin..].iter_mut().zip(&burst) {
        *o += *v;
    }
    let zc_at = margin + 2 * (droneid::NFFT + 72);
    let mut rx = droneid::BurstReceiver::default();
    sdrkit_core::bench::measure(seconds, 1, 0.0, || {
        std::hint::black_box(rx.decode_at(&x, zc_at, 0..droneid::NC));
    })
}

/// Benchmarks of the three receivers on their own test signals (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "remote_id.receiver_20m",
            run: |s| bench_receiver(s, &REMOTE_ID, 0.1),
        },
        Benchmark {
            name: "dji_droneid.receiver_20m",
            run: |s| bench_receiver(s, &DJI_DRONEID, 0.2),
        },
        Benchmark {
            name: "dji_droneid.trigger_20m",
            run: bench_trigger,
        },
        Benchmark {
            name: "dji_droneid.weak_burst",
            run: bench_weak_burst,
        },
        Benchmark {
            name: "flarm.receiver_1m",
            run: |s| bench_receiver(s, &FLARM, 1.0),
        },
        Benchmark {
            name: "adsl.receiver_2m",
            run: |s| bench_receiver(s, &ADSL, 2.0),
        },
        Benchmark {
            name: "ogn.receiver_1m",
            run: |s| bench_receiver(s, &OGN, 4.0),
        },
    ]
}

/// A check that the count field `c` of the results is at least `min`.
fn at_least(r: &Results, c: &Column<field::I64>, min: i64) -> CheckLine {
    let v = r.fields().at(c).unwrap_or(0);
    CheckLine::new(c.key(), v >= min, format!("{v}, at least {min} expected"))
}

fn rid_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, BLE and Wi-Fi beacons", o, 0.0);
    c.seconds = o.seconds.max(0.2);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        vec![
            at_least(r, &typed::RemoteIdFields::DRONES, 2),
            at_least(r, &typed::RemoteIdFields::MESSAGES, 20),
        ]
    });
    vec![c]
}

fn dji_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, DroneID every 60 ms", o, 0.0);
    c = c.with(&dji::GEN_PERIOD_S, 0.06);
    c.seconds = o.seconds.max(0.3);
    c.expect_audio = Some(false);
    c.check = Some(|r| vec![at_least(r, &typed::DjiDroneidFields::DRONEID_DECODED, 2)]);
    vec![c]
}

fn flarm_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, six devices", o, 0.0);
    c = c
        .with(&flarm::WALL_CLOCK, false)
        .with(&flarm::START_TIME, 1_800_000_000.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| vec![at_least(r, &typed::FlarmFields::V6_DECODED, 4)]);
    vec![c]
}

fn adsl_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four aircraft", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        vec![
            at_least(r, &typed::AdslFields::AIRCRAFT, 4),
            at_least(r, &typed::AdslFields::TRAFFIC, 8),
        ]
    });
    vec![c]
}
fn ogn_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four trackers", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        vec![
            at_least(r, &typed::OgnFields::TRACKERS, 4),
            at_least(r, &typed::OgnFields::FRAMES_OK, 8),
        ]
    });
    vec![c]
}

fn sik_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, four vehicles", o, 0.0);
    c.seconds = o.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        vec![
            at_least(r, &typed::LinkFields::LINKS_HEARD, 4),
            at_least(r, &typed::LinkFields::FRAMES_OK, 40),
        ]
    });
    vec![c]
}

fn rc_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, seven transmitters", o, 0.0);
    c.seconds = o.seconds.max(2.0);
    c.expect_audio = Some(false);
    c.expect_lock = true;
    c.check = Some(|r| {
        vec![
            at_least(r, &typed::LinkFields::LINKS_HEARD, 6),
            at_least(r, &typed::LinkFields::FRAMES_OK, 40),
        ]
    });
    let mut p = VerifyCase::new("test signal of the submode ppm72, four channels", o, 0.0)
        .with(&rc_links::SUBMODE, rc_links::Submode::Ppm72);
    p.seconds = o.seconds.max(2.0);
    p.snr_db = (!o.clean).then_some(30.0);
    p.expect_audio = Some(false);
    p.check = Some(|r| {
        let f = <typed::RcLinksPpmFields as sdrkit_core::interface::ReadFields>::read(r.fields());
        let n = f.as_ref().and_then(|f| f.channels);
        let good = f.as_ref().map_or(0, |f| f.ppm_frames_ok);
        vec![
            CheckLine::new(typed::RcLinksPpmFields::CHANNELS.key, n == Some(4), format!("{n:?}, 4 sent")),
            CheckLine::new(typed::RcLinksPpmFields::PPM_FRAMES_OK.key, good >= 60, format!("{good}, 100 sent in 2 s")),
        ]
    });
    vec![c, p]
}

/// The verify case of `mavlink_sik`.
pub static MAVLINK_SIK_VERIFY: VerifyHooks = VerifyHooks {
    cases: sik_cases,
    vector_settings: None,
    check_vector: None,
};

/// The verify case of `rc_links`.
pub static RC_LINKS_VERIFY: VerifyHooks = VerifyHooks {
    cases: rc_cases,
    vector_settings: None,
    check_vector: None,
};

/// The verify case of `ogn`.
pub static OGN_VERIFY: VerifyHooks = VerifyHooks {
    cases: ogn_cases,
    vector_settings: None,
    check_vector: None,
};

/// The verify case of `adsl`.
pub static ADSL_VERIFY: VerifyHooks = VerifyHooks {
    cases: adsl_cases,
    vector_settings: None,
    check_vector: None,
};
/// The verify case of `remote_id`.
pub static RID_VERIFY: VerifyHooks = VerifyHooks {
    cases: rid_cases,
    vector_settings: None,
    check_vector: None,
};
/// The verify case of `dji_droneid`.
pub static DJI_VERIFY: VerifyHooks = VerifyHooks {
    cases: dji_cases,
    vector_settings: None,
    check_vector: None,
};
/// The verify case of `flarm`.
pub static FLARM_VERIFY: VerifyHooks = VerifyHooks {
    cases: flarm_cases,
    vector_settings: None,
    check_vector: None,
};
