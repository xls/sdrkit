//! ISM sensors (`ism`): every burst in the input band is found, channelised, cut into pulse trains and offered to every device
//! protocol. The Sigfox chain runs beside it on a Sigfox band. Each reading leaves as an `ookdev.reading` event. In the submode
//! `pulse_analyser` the pulse analyser ([`analyser`]) reads the trains in place of the device protocols.

pub mod analyser;
pub mod checks;
pub mod detect;
pub mod devices;
pub mod encode;
pub mod gen;
pub mod protocols;
pub mod pulsegen;
pub mod send;

use crate::family::{at_least_value, min_rate_message, REPORT_S};
use crate::typed::IsmFields;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::AutoGain;
use sdrkit_core::ConfigError;
use sdrkit_core::SignalType;
use sdrkit_core::{
    GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet, StackConfig, StackDescriptor,
    Tuning,
};
use sdrkit_graph::Catalog;
use sdrkit_graph::{Graph, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_ookdev::ProtocolList;
use std::sync::{Arc, OnceLock};

/// The lowest sample rate.
pub const MIN_RATE_HZ: f64 = 1e6;
/// The lowest sample rate of the submode `pulse_analyser`.
pub const MIN_RATE_PULSE_HZ: f64 = 100e3;

/// The protocol list of the stack: the device protocols of [`protocols::PROTOCOLS`].
pub fn protocol_list() -> Arc<ProtocolList> {
    static LIST: OnceLock<Arc<ProtocolList>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut l = ProtocolList::new();
        l.add_table(&protocols::PROTOCOLS);
        Arc::new(l)
    })
    .clone()
}

/// Builds the `ism` graph.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`].
pub fn build_graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .catalog(catalog)
        .add("burstdetect", detect::BurstDetect::default())
        .add("channelise", detect::Channelise::default())
        .add("pulses", detect::Pulses::with_protocols(protocol_list()))
        .add("decode", devices::Decode::new(protocol_list()))
        .add("analyser", analyser::Analyser::default())
        .add("devices", devices::Devices::default())
        .add("sfx_front", crate::sigfox::Front::new(true))
        .add("sfx_finder", crate::sigfox::Finder::default())
        .add("sfx_demod", crate::sigfox::Demod::default())
        .add("sender", crate::tx::FrameSender::<send::IsmFrames>::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("burstdetect")
        .input("sfx_front")
        .input("sender.in")
        .connect("sender.out", "tx.in")
        .connect("burstdetect", "channelise")
        .connect("channelise", "pulses")
        .connect("pulses", "decode")
        .connect("pulses", "analyser")
        .connect("analyser", "devices.frames")
        .connect("decode.readings", "devices.readings")
        .connect("decode.unknown", "devices.unknown")
        .connect("sfx_front.out", "sfx_finder")
        .connect("sfx_front.state", "devices.sigfox_state")
        .connect("sfx_finder", "sfx_demod")
        .connect("sfx_demod", "devices.sigfox")
        .output("devices.events")
        .output("decode.readings")
        .output("sender.events")
        .clock(REPORT_S)
        .status_from("devices")
        .build()
}

sdrkit_core::choice_enum! {
    /// The scene of the test signal.
    pub enum GenScene {
        /// The 433.92 MHz sensors.
        Sensors433 = ("0", "433.92 MHz sensors"),
        /// The 868.3 MHz sensors.
        Sensors868 = ("1", "868.3 MHz sensors"),
        /// Z-Wave.
        Zwave = ("2", "Z-Wave"),
    }
}

/// The scene of the test signal.
pub const GEN_SCENE: Setting<field::Choice<GenScene>> =
    Setting::choice("gen_scene", "Test signal scene", GenScene::Sensors433)
        .section("Test signal")
        .advanced();
/// A strong carrier in the test signal.
pub const GEN_CARRIER: Setting<field::Bool> =
    Setting::new("gen_carrier", "Strong continuous carrier", "", false)
        .help("A carrier 20 dB above the devices, 300 kHz above the centre.")
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `ism` stack.
    pub enum IsmSetting {
        /// The detection threshold.
        Threshold = detect::THRESHOLD,
        /// The scene of the test signal.
        GenScene = GEN_SCENE,
        /// A strong carrier in the test signal.
        GenCarrier = GEN_CARRIER,
        /// The device decoders or the pulse analyser.
        Submode = analyser::SUBMODE,
        /// The longest gap inside a frame of the pulse analyser.
        FrameGapMs = analyser::FRAME_GAP_MS,
        /// The fewest pulses of a frame of the pulse analyser.
        MinPulses = analyser::MIN_PULSES,
        /// The split ratio of the clusters of the pulse analyser.
        ClusterRatio = analyser::CLUSTER_RATIO,
        /// The line code of the test signal of the pulse analyser.
        GenPattern = pulsegen::GEN_PATTERN,
        /// The bits of the test signal of the pulse analyser.
        GenBits = pulsegen::GEN_BITS,
        /// The unit of the test signal of the pulse analyser.
        GenTUs = pulsegen::GEN_T_US,
        /// The PPM pulse of the test signal.
        GenPulseUs = pulsegen::GEN_PULSE_US,
        /// The PPM gap of a 0 of the test signal.
        GenGap0Us = pulsegen::GEN_GAP0_US,
        /// The PPM gap of a 1 of the test signal.
        GenGap1Us = pulsegen::GEN_GAP1_US,
        /// The timing jitter of the test signal.
        GenJitter = pulsegen::GEN_JITTER,
        /// The frames of a group of the test signal.
        GenFrames = pulsegen::GEN_FRAMES,
        /// The gap after each frame of the test signal.
        GenFrameGapS = pulsegen::GEN_FRAME_GAP_S,
        /// The level sent.
        TxLevel = crate::tx::TX_LEVEL,
    }
}

/// The `ism` receiver. Its lowest rate is 1 Msps, or 100 ksps for a recording or a generator when it is made in the submode
/// `pulse_analyser` (a radio is tuned 250 kHz off the signal, so it needs 1 Msps in either submode).
///
/// # Errors
///
/// The errors of [`build_graph`] and of [`StackGraphBuilder::build`](sdrkit_graph::StackGraphBuilder::build), among them a
/// refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let (min, name) = if submode(&cfg.settings) == analyser::Submode::PulseAnalyser {
        (MIN_RATE_PULSE_HZ, "The pulse analyser")
    } else {
        (MIN_RATE_HZ, "ISM sensors")
    };
    StackGraph::of(crate::typed::shape("ism"), build_graph(&Catalog::of(cfg)?)?)
        .min_rate(min, &min_rate_message(name, min))
        .min_rate_live(MIN_RATE_HZ, &min_rate_message("ISM sensors", MIN_RATE_HZ))
        .route(detect::THRESHOLD.key(), &["burstdetect.threshold"])?
        .route(
            analyser::SUBMODE.key(),
            &[
                "burstdetect.submode",
                "pulses.submode",
                "decode.submode",
                "analyser.submode",
                "devices.submode",
            ],
        )?
        .route(analyser::FRAME_GAP_MS.key(), &["analyser.frame_gap_ms"])?
        .route(analyser::MIN_PULSES.key(), &["analyser.min_pulses"])?
        .route(analyser::CLUSTER_RATIO.key(), &["analyser.cluster_ratio"])?
        .route(crate::tx::TX_LEVEL.key(), &["sender.tx_level"])?
        .command("clear", sdrkit_graph::CommandTarget::of("devices", "clear"))
        .command(
            "remove",
            sdrkit_graph::CommandTarget::of("devices", "remove"),
        )
        .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
        .command("halt", sdrkit_graph::CommandTarget::of("sender", "halt"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The generator settings of the stack.
pub fn gen_params(cfg: &GenConfig) -> gen::GenParams {
    let s = &cfg.settings;
    gen::GenParams {
        scene: match s.get(&GEN_SCENE) {
            GenScene::Sensors433 => 0,
            GenScene::Sensors868 => 1,
            GenScene::Zwave => 2,
        },
        carrier: s.get(&GEN_CARRIER),
    }
}

/// The submode of the settings `v` (the device decoders when `v` has none).
pub fn submode(v: &sdrkit_core::Values) -> analyser::Submode {
    v.schema()
        .id(analyser::SUBMODE.key())
        .ok()
        .and_then(|i| v.value(i).and_then(|x| analyser::SUBMODE.read(i, x).ok()))
        .unwrap_or(analyser::Submode::Devices)
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    if submode(&cfg.settings) == analyser::Submode::PulseAnalyser {
        let g =
            pulsegen::generator(cfg.rate_hz, &cfg.settings, cfg.seed).ok_or(ConfigError::NoRate)?;
        return Ok(Box::new(g));
    }
    let p = gen_params(cfg);
    let g =
        gen::generator(cfg.rate_hz, &p, gen::scene_slots(p.scene)).ok_or(ConfigError::NoRate)?;
    Ok(Box::new(g))
}

/// ISM sensors.
pub static ISM: StackDescriptor = StackDescriptor {
    id: "ism",
    name: "ISM sensors",
    group: "Sensors, 433 / 868 MHz",
    blurb: "Weather stations, thermometers, tyre pressure sensors, door and motion sensors, remotes and meters on 315 to 915 MHz.",
    tuning: Tuning {
        min_hz: 300e6,
        max_hz: 930e6,
        tune_offset_hz: 250e3,
        no_offset_tuning: false,
        ..Tuning::anywhere(433.92e6, 2e6, MIN_RATE_HZ, 1.6e6)
    },
    submodes: &[],
    settings: IsmSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::NarrowDigital],
    auto_gain: AutoGain::Radio,
    verify: Some(&VERIFY),
    benchmarks: Some(crate::benchmarks),
};

fn cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut a = VerifyCase::new("test signal, 433.92 MHz sensors", o, 0.0);
    a.seconds = o.seconds.max(6.0);
    a.expect_audio = Some(false);
    a.check = Some(|r| {
        let f = IsmFields::read(r.fields());
        vec![
            at_least_value("devices", f.as_ref().map_or(0, |f| f.devices), 12),
            at_least_value(
                "packets_decoded",
                f.as_ref().map_or(0, |f| f.packets_decoded),
                12,
            ),
        ]
    });
    let mut b = VerifyCase::new("test signal, 868.3 MHz sensors", o, 0.0)
        .with(&GEN_SCENE, GenScene::Sensors868);
    b.seconds = o.seconds.max(5.0);
    b.expect_audio = Some(false);
    b.check = Some(|r| {
        let f = IsmFields::read(r.fields());
        vec![at_least_value(
            "devices",
            f.as_ref().map_or(0, |f| f.devices),
            4,
        )]
    });
    let mut c = VerifyCase::new(
        "test signal of the submode pulse_analyser, PWM frames",
        o,
        0.0,
    )
    .with(&analyser::SUBMODE, analyser::Submode::PulseAnalyser);
    c.seconds = o.seconds.max(2.5);
    c.snr_db = (!o.clean).then_some(25.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let f = crate::typed::IsmPulseFields::read(r.fields());
        let good = f.as_ref().map_or(0, |f| f.frames_good);
        // The default bits 101100101001110010100110 of the generator.
        let hex = f.as_ref().and_then(|f| f.bits_hex.map(str::to_string));
        vec![
            at_least_value("frames_good", good, 6),
            sdrkit_core::verify::CheckLine::new(
                "bits_hex",
                hex.as_deref() == Some("B29CA6"),
                format!("{hex:?}, B29CA6 sent"),
            ),
        ]
    });
    vec![a, b, c]
}

/// The test-signal cases of `ism`.
pub static VERIFY: VerifyHooks = VerifyHooks {
    cases,
    vector_settings: None,
    check_vector: None,
};

/// The benchmark of the submode `pulse_analyser`: 1.5 s of its PWM test signal at 250 ksps.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![sdrkit_core::Benchmark {
        name: "ism.pulse_analyser_250k",
        run: bench_pulse_analyser,
    }]
}

fn bench_pulse_analyser(seconds: f64) -> sdrkit_core::bench::BenchResult {
    use sdrkit_core::Choice;
    let rate = 250e3;
    let mut v = sdrkit_core::Values::defaults(&IsmSetting::schema());
    if let Ok(id) = v.schema().id(analyser::SUBMODE.key()) {
        let _ = v.set(
            id,
            &sdrkit_core::Value::Choice(analyser::Submode::PulseAnalyser.index()),
        );
    }
    let mut x = vec![sdrkit_core::Complex32::default(); (rate * 1.5) as usize];
    if let Some(mut g) = pulsegen::generator(rate, &v, 1) {
        g.generate(&mut x);
    }
    let Ok(mut rx) = receiver(&StackConfig::new(v)) else {
        return sdrkit_core::bench::BenchResult::default();
    };
    let dial = ISM.tuning.default_hz;
    let _ = rx.configure(&sdrkit_core::InputInfo::new(rate, dial, dial).recording());
    let mut out = sdrkit_core::Output::default();
    sdrkit_core::bench::measure(seconds, x.len(), 0.5, || {
        out.clear();
        rx.feed(&x, &mut out);
    })
}
