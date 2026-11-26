//! Studio-to-transmitter links (`stl`) of broadcast stations, receive only. An analogue link is a wide FM carrier with the
//! stereo multiplex of a broadcast transmitter (ITU-R BS.450: pilot 19 kHz, difference signal on 38 kHz, RDS on 57 kHz
//! per IEC 62106) or with programme audio; the stack plays its sound and reads its RDS with the components of the FM
//! stack. A digital link is detected and measured (occupied width, level over the floor, flatness) and not decoded: no
//! public document describes its frames.
//!
//! The graph: the link finder ([`link`]) steers the FM channel (centre and width); the FM demodulator, multiplex
//! spectrum, stereo decoder, de-emphasis and RDS of [`crate::graph`] follow; the SCA meter ([`sca`]) and the sound stage
//! ([`sound`]) are the STL's own; the display ([`display`]) decides the kind of the link.

pub mod display;
pub mod generator;
pub mod link;
pub mod sca;
pub mod settings;
pub mod sound;
pub mod typed;
pub mod verify;

use crate::graph::{
    ChannelComponent, DeemphasisComponent, DemodComponent, RdsComponent, SpectrumComponent,
    StereoComponent, REPORT_S,
};
use generator::{StlGenerator, StlParams};
use sdrkit_core::bench::measure;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockCondition, LockProfile, LockState, NameSource, Output, Receiver, SettingSet, SignalType,
    StackConfig, StackDescriptor, Tuning, Values,
};
use sdrkit_graph::{Catalog, CommandTarget, GraphBuilder, GraphError, Registry, StackGraph};
use settings::StlSetting;

/// The stack id.
pub const ID: &str = "stl";
/// The lowest input rate: that of the FM components.
pub const MIN_RATE_HZ: f64 = crate::graph::MIN_RATE_HZ;

/// Adds the STL components (kinds `stl.*`) to `r`.
pub fn register(r: &mut Registry) {
    r.add("stl.link", || Box::new(link::LinkComponent::default()));
    r.add("stl.sca", || Box::new(sca::ScaComponent::default()));
    r.add("stl.sound", || Box::new(sound::SoundComponent::default()));
    r.add("stl.display", || Box::new(display::StlDisplay::default()));
}

/// The component that each stack setting goes to.
fn component_of(s: StlSetting) -> Option<&'static str> {
    use StlSetting as S;
    Some(match s {
        S::Stereo => "stereo",
        S::Deemphasis => "deemph",
        S::Deviation => "sound",
        S::AutoWidth | S::Width => "link",
        S::RdsVariant | S::RdsCorrection | S::RdsVoting | S::RdsCharset => "rds",
        S::GenLink
        | S::GenDeviation
        | S::GenSca
        | S::GenWidth
        | S::GenLeftHz
        | S::GenLeftAmp
        | S::GenRightHz
        | S::GenRightAmp
        | S::GenRds
        | S::GenPs
        | S::GenRt
        | S::GenPi
        | S::GenPty
        | S::GenPreemphasis => return None,
    })
}

/// The STL graph at its defaults.
pub fn builder() -> GraphBuilder {
    let mut b = GraphBuilder::new()
        .add("link", link::LinkComponent::default())
        .add("channel", ChannelComponent::default())
        .add("demod", DemodComponent::default())
        .add("spectrum", SpectrumComponent::default())
        .add("stereo", StereoComponent::default())
        .add("deemph", DeemphasisComponent::default())
        .add("sound", sound::SoundComponent::default())
        .add("sca", sca::ScaComponent::default())
        .add("rds", RdsComponent::default())
        .add("display", display::StlDisplay::default())
        .input("link.in")
        .input("channel.in")
        .connect("link.steer", "channel.steer")
        .connect("channel.out", "demod.in")
        .connect("channel.setup", "demod.setup")
        .connect("demod.meas", "spectrum.in")
        .connect("demod.report", "spectrum.demod")
        .connect("demod.sound", "stereo.in")
        .connect("spectrum.report", "stereo.snr")
        .connect("demod.report", "stereo.demod")
        .connect("stereo.out", "deemph.in")
        .connect("deemph.out", "sound.in")
        .connect("demod.report", "sound.demod")
        .connect("demod.meas", "sca.in")
        .connect("demod.meas", "rds.in")
        .connect("stereo.carrier", "rds.carrier")
        .connect("channel.report", "display.channel")
        .connect("demod.report", "display.demod")
        .connect("spectrum.report", "display.spectrum")
        .connect("stereo.report", "display.stereo")
        .connect("rds.report", "display.rds")
        .connect("link.report", "display.link")
        .connect("sca.report", "display.sca")
        .connect("sound.report", "display.sound")
        .feedback("spectrum.report", "demod.snr")
        .feedback("demod.report", "link.demod")
        .feedback("sca.report", "link.sca")
        .output("sound.out")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune()
        .command_alias("clear", CommandTarget::every("clear"));
    for s in StlSetting::SETTINGS {
        let key = s.key();
        let Some(c) = StlSetting::schema()
            .id(key)
            .ok()
            .and_then(StlSetting::from_id)
            .and_then(component_of)
        else {
            continue;
        };
        b = b.alias(key, &format!("{c}.{key}"));
    }
    b
}

/// The receiver of the STL stack: its graph as the engine runs it, with the stack's settings applied.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        typed::stack_shape(),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(MIN_RATE_HZ, "STL needs a sample rate of at least 500 ksps")
    .command("clear", CommandTarget::every("clear"))
    .on_retune(CommandTarget::every("clear"))?
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(StlGenerator::from_config(cfg)))
}

/// How a scan of channels judges a link: a carrier tracked, or a signal that is not FM measured; the name from RDS.
const LOCK_PROFILE: LockProfile = LockProfile {
    settle_s: 0.5,
    quiet_after_s: Some(1.0),
    signal_snr_db: 10.0,
    timeout_s: 2.0,
    lock: LockCondition::LockState {
        at_least: LockState::Tracking,
    },
    collect_s: 4.0,
    name_from: &[NameSource::Field("station")],
    capture: &["link", "pi", "text"],
};

/// The STL graph on the composite test link at 30 dB C/N in 200 kHz, at `rate`.
fn bench_receiver(seconds: f64, rate: f64) -> BenchResult {
    let mut g = StlGenerator::new(StlParams::new(settings::GenLink::Composite, rate));
    let mut x = vec![Complex32::default(); rate as usize];
    g.generate(&mut x);
    let level = sdrkit_algo_sim::noise::NoiseLevel::SnrDb {
        snr_db: 30.0,
        bandwidth_hz: 200e3,
    };
    if let Ok(mut n) = sdrkit_algo_sim::noise::Awgn::complex(level, 1.0, rate, 1) {
        n.process(&mut x);
    }
    let cfg = StackConfig::new(Values::defaults(&StlSetting::schema()));
    let Ok(mut rx) = receiver(&cfg) else {
        return BenchResult::default();
    };
    if rx
        .configure(&InputInfo::new(rate, 947.5e6, 947.5e6).recording())
        .is_err()
    {
        return BenchResult::default();
    }
    let mut out = Output::default();
    measure(seconds, x.len(), 1.0, || {
        for b in x.chunks(65_536) {
            out.clear();
            rx.feed(b, &mut out);
        }
    })
}

/// The benchmarks of the STL stack.
pub fn benchmarks() -> Vec<Benchmark> {
    vec![Benchmark {
        name: "stl.receiver_1000k",
        run: |s| bench_receiver(s, 1e6),
    }]
}

/// The STL stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "STL link",
    group: "Broadcast radio",
    blurb: "Studio-to-transmitter links of broadcast stations: the sound and RDS of an analogue FM link; a digital link detected and measured, not decoded.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 100e6,
        max_hz: 6000e6,
        default_hz: 947.5e6,
        rate_hz: 1e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 280e3,
        tune_offset_hz: 0.0,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: StlSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[
        SignalType::FmBroadcast,
        SignalType::WideFm,
        SignalType::WideDigital,
    ],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::HOOKS),
    benchmarks: Some(benchmarks),
};

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Owner;

    #[test]
    fn the_description_builds_and_every_setting_reaches_a_component() {
        let g = builder().build().expect("builds");
        let d = g.describe();
        assert_eq!(d.components.len(), 10);
        assert_eq!(d.feedback.len(), 3);
        let s = g.schema();
        for key in StlSetting::SETTINGS
            .iter()
            .map(|s| s.key())
            .filter(|k| !k.starts_with("gen_"))
        {
            assert!(s.id(key).is_ok(), "{key}");
        }
    }

    #[test]
    fn the_benchmark_runs() {
        for b in benchmarks() {
            let r = (b.run)(0.0);
            assert!(r.items_per_second > 0.0, "{}", b.name);
        }
    }
}
