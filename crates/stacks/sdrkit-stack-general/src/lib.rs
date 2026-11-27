//! The general receiver (AM with synchronous detection, NFM, WFM mono, USB, LSB and CW) with its transmit branch (push-to-talk
//! speech, Morse code, a tune carrier), the listening chain that every audio stack shares, the tone identification and the band
//! tables. [`DESCRIPTOR`] is the stack, and [`register`] adds its components.

pub mod amproc;
pub mod audio;
pub mod bands;
pub mod blanker;
pub mod channel;
pub mod demod;
pub mod generator;
pub mod listen;
pub mod noise;
pub mod presets;
pub mod receiver;
pub mod report;
pub mod results;
pub mod settings;
pub mod smoothing;
pub mod squelch;
pub mod toneid;
pub mod transmit;
pub mod typed;
pub mod types;
pub mod verify;

pub use generator::{GenParams, GeneralGenerator};
pub use receiver::{GeneralCommand, GeneralReceiver, Mic};
pub use settings::{GeneralSetting, GeneralSettings};
pub use types::Demod;

use sdrkit_core::bench::measure;
use sdrkit_core::{
    AutoGain, BenchResult, Benchmark, Complex32, ConfigError, GenConfig, Generator, InputInfo,
    LockCondition, LockProfile, LockState, Output, Receiver, SettingSet, SignalType, StackConfig,
    StackDescriptor, SubMode, Tuning, Value, Values,
};

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(GeneralReceiver::new(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(GeneralGenerator::from_config(cfg)))
}

/// How a scan of channels judges a channel of the general receiver: the squelch opens, and the tone is kept.
const LOCK_PROFILE: LockProfile = LockProfile {
    settle_s: 0.1,
    quiet_after_s: Some(0.3),
    signal_snr_db: 6.0,
    timeout_s: 1.0,
    lock: LockCondition::LockState {
        at_least: LockState::Locked,
    },
    collect_s: 0.5,
    name_from: &[],
    capture: &["tone"],
};

/// The general receiver.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: "general",
    name: "General",
    group: "Listening",
    blurb: "AM (with synchronous detection), narrow and wide FM, USB, LSB and CW, with squelch, noise reduction and presets; transmits speech, Morse code and a tune carrier.",
    tuning: Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Free),
        min_hz: 0.1e6,
        max_hz: 6000e6,
        default_hz: 0.0,
        rate_hz: 2e6,
        min_rate_hz: receiver::MIN_RATE_HZ,
        bandwidth_hz: 1.6e6,
        tune_offset_hz: 250e3,
        carrier_at_centre: true,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[
        SubMode { id: "general", name: "General", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "am", name: "AM broadcast", default_hz: 0.0, ..SubMode::DEFAULT },
        SubMode { id: "shortwave", name: "Shortwave", default_hz: 0.0, ..SubMode::DEFAULT },
    ],
    settings: GeneralSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LOCK_PROFILE,
    signal_types: &[
        SignalType::AmVoice,
        SignalType::AmBroadcast,
        SignalType::NarrowFmVoice,
        SignalType::KeyedVoice,
        SignalType::WideFm,
        SignalType::SsbUpper,
        SignalType::SsbLower,
        SignalType::CwMorse,
    ],
    auto_gain: AutoGain::Radio,
    verify: Some(&verify::HOOKS),
    benchmarks: Some(benchmarks),
};

/// Adds every component of this crate to `r` under its kind name, so a graph description can use them. The receiver graph also
/// uses `tones.subaudio` and `tones.dtmf` of `sdrkit-proto-tones` and the standard DSP components, which this adds too.
pub fn register(r: &mut sdrkit_graph::Registry) {
    sdrkit_proto_tones::components::register(r);
    sdrkit_components::dsp::register(r);
    r.add("general.decimate", || {
        Box::new(channel::ChannelDecimator::default())
    });
    r.add("general.blanker", || {
        Box::new(blanker::ChannelBlanker::default())
    });
    r.add("general.noise", || {
        Box::new(noise::NoiseComponent::default())
    });
    r.add("general.resample", || {
        Box::new(channel::ExactResampler::default())
    });
    r.add("general.filter", || {
        Box::new(channel::ChannelFilter::default())
    });
    r.add("general.squelch", || {
        Box::new(squelch::SquelchComponent::default())
    });
    r.add("general.demod", || {
        Box::new(demod::DemodComponent::default())
    });
    r.add("general.amproc", || {
        Box::new(amproc::AmProcessorComponent::default())
    });
    r.add("general.select", || Box::new(audio::Select::default()));
    r.add("general.clip", || Box::new(audio::Clip));
    r.add("general.gate", || Box::new(audio::Gate::default()));
    r.add("general.afilters", || {
        Box::new(audio::AudioFilters::default())
    });
    r.add("general.voice_nr", || Box::new(audio::VoiceNr::default()));
    r.add("general.toneid", || {
        Box::new(toneid::ToneIdComponent::default())
    });
    r.add("general.report", || Box::new(report::Report::default()));
    r.add("general.listening", || {
        Box::new(listen::ListeningComponent::default())
    });
    r.add("general.transmit", || {
        Box::new(transmit::Transmitter::default())
    });
}

/// Every stack of this crate.
pub fn descriptors() -> Vec<&'static StackDescriptor> {
    vec![&DESCRIPTOR]
}

/// The level of a unit-power signal whose carrier is 0.3 of full scale, dBFS.
pub const CARRIER_0_3_DBFS: f64 = -10.457_574_905_606_752;

/// The settings of the stack and of the generator, with `demod` and `extra` set.
fn bench_values(demod: Demod, extra: &[(GeneralSetting, Value)]) -> Values {
    use sdrkit_core::Choice;
    let schema = sdrkit_components::sim::with_generator_settings(&GeneralSetting::schema())
        .expect("the settings of the stack and of the generator make one schema");
    let mut v = Values::defaults(&schema);
    let mut set = |key: &str, value: Value| {
        let id = schema.id(key).expect("a setting of the schema");
        v.set(id, &value).expect("a value that the schema takes");
    };
    set("demod", Value::Choice(demod.index()));
    for (s, x) in extra {
        let key = GeneralSetting::schema().settings()[s.id().index()]
            .field
            .key
            .clone();
        set(&key, x.clone());
    }
    set(
        "gen_noise",
        Value::Choice(sdrkit_components::sim::NoiseKind::Snr.index()),
    );
    set("gen_snr_db", Value::F64(25.0));
    set("gen_level_dbfs", Value::F64(CARRIER_0_3_DBFS));
    v
}

fn bench_receiver(
    demod: Demod,
    rate: f64,
    extra: &[(GeneralSetting, Value)],
    seconds: f64,
) -> BenchResult {
    let v = bench_values(demod, extra);
    let clean = GeneralGenerator::new(GenParams::from_values(rate, &v));
    let mut g =
        sdrkit_components::sim::impaired(Box::new(clean), &v).expect("valid test signal settings");
    let n = (rate * 0.5) as usize;
    let mut x = vec![Complex32::default(); n];
    g.generate(&mut x);
    let mut rx =
        GeneralReceiver::new(&StackConfig::new(v)).expect("the receiver takes the settings");
    rx.configure(&InputInfo::new(rate, 145.75e6, 145.5e6))
        .expect("rate");
    let mut out = Output::default();
    measure(seconds, n, n as f64 / rate, || {
        out.clear();
        for c in x.chunks(20_000) {
            rx.feed(c, &mut out);
        }
    })
}

/// Push-to-talk in NFM at `rate` with the microphone input open: the receive chain and the transmit branch together.
fn bench_transmit(rate: f64, seconds: f64) -> BenchResult {
    use sdrkit_core::CommandSet;
    let v = bench_values(Demod::Nfm, &[]);
    let mut rx =
        GeneralReceiver::new(&StackConfig::new(v)).expect("the receiver takes the settings");
    rx.configure(&InputInfo::new(rate, 145.75e6, 145.5e6))
        .expect("rate");
    rx.set_transmit(true);
    let args = sdrkit_core::Owner::commands(&rx)
        .def(GeneralCommand::Ptt.id())
        .expect("the ptt command")
        .new_args();
    let on = args.decl().id("on").expect("the argument on");
    let mut args = args;
    args.set_value(on, &Value::Bool(true)).expect("a flag");
    sdrkit_core::owner::invoke(&mut rx, GeneralCommand::Ptt.id(), &args).expect("ptt starts");
    let n = (rate * 0.5) as usize;
    let x = vec![Complex32::default(); n];
    let mut out = Output::default();
    measure(seconds, n, n as f64 / rate, || {
        out.clear();
        for c in x.chunks(20_000) {
            rx.feed(c, &mut out);
        }
    })
}

fn bench_listening(seconds: f64) -> BenchResult {
    use listen::{AgcSpeed, ChainSettings, ChainSquelch, EqPreset, ListeningChain, ModeKind};
    let mut s = ChainSettings::defaults_for(ModeKind::Nfm);
    s.cleanup = ChainSettings::cleanup_preset(ModeKind::Nfm);
    s.cleanup.notch = true;
    s.control.on = true;
    s.control.auto_notch = true;
    s.control.agc = AgcSpeed::Fast;
    s.control.squelch = ChainSquelch::Snr;
    s.control.eq = EqPreset::Voice;
    let mut c = ListeningChain::new(48e3, s);
    let n = 48_000;
    let x: Vec<[f32; 2]> = (0..n)
        .map(|i| {
            let v =
                ((i as f32) * 0.13).sin() * 0.2 + ((i * 7919 % 1000) as f32 / 1000.0 - 0.5) * 0.05;
            [v, v]
        })
        .collect();
    let mut buf = x.clone();
    measure(seconds, n, 1.0, || {
        buf.copy_from_slice(&x);
        for c2 in buf.chunks_mut(480) {
            c.process(c2);
        }
    })
}

/// The benchmarks of this crate: the receiver at 2 Msps (NFM, WFM, USB), AM with Auto quality, NFM with push-to-talk, and the
/// listening chain with every part on (items are input samples).
pub fn benchmarks() -> Vec<Benchmark> {
    vec![
        Benchmark {
            name: "general.nfm_2000k",
            run: |s| bench_receiver(Demod::Nfm, 2e6, &[], s),
        },
        Benchmark {
            name: "general.wfm_2000k",
            run: |s| bench_receiver(Demod::Wfm, 2e6, &[], s),
        },
        Benchmark {
            name: "general.usb_2000k",
            run: |s| bench_receiver(Demod::Usb, 2e6, &[], s),
        },
        Benchmark {
            name: "general.am_auto_2000k",
            run: |s| {
                bench_receiver(
                    Demod::Am,
                    2e6,
                    &[(GeneralSetting::AmAuto, Value::Bool(true))],
                    s,
                )
            },
        },
        Benchmark {
            name: "general.nfm_ptt_2000k",
            run: |s| bench_transmit(2e6, s),
        },
        Benchmark {
            name: "general.listening_48k",
            run: bench_listening,
        },
    ]
}
