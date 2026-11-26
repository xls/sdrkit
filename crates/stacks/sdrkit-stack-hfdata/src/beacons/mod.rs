//! HF beacons: the propagation monitor. No decoder: it watches the frequencies of an editable beacon list
//! in a wide HF capture, distinguishes each beacon from a random carrier by its keying and schedule, measures its SNR and keeps
//! its history.
//!
//! ```text
//! iq (wide) -> [stage 1 filter bank] -> [stage 2, channel meter] -> hfb.excess --+--> [runs] -> hfb.run --+--> [keying judges] -> hfb.detection -> [stats] -> results
//!                                                                                  +-------------------------+
//! iq (wide) -> [HF channel front end at the beacon - 700 Hz] -> [sideband audio] -> [listen] -> audio
//! iq -> [own beacon transmitter: start, send, stop] -> [tx sink]
//! ```

pub mod gen;
pub mod list;
pub mod prop;
pub mod rx;
pub mod stats;
pub mod tx;

use crate::frontend::HfChannelFrontEnd;
use crate::listen::Listen;
use crate::listen::LISTEN;
use crate::receiver::GEN_SECTION;
use crate::sideband::Demod;
use crate::sideband::SidebandAudio;
use gen::BeaconSignal;
use sdrkit_core::publish::field;
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Setting, SettingSet,
    StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};

/// Listen to the beacon on this frequency: the channel goes 700 Hz below it. 0 is off.
pub const LISTEN_KHZ: Setting<field::F64> =
    Setting::new("listen_khz", "Listen to the beacon on", "kHz", 0.0).range(0.0, 1e6, 0.1);
/// The signal of the test signal.
pub const GEN_SIGNAL: Setting<field::Choice<BeaconSignal>> =
    Setting::choice("gen_signal", "Signal", BeaconSignal::Beacons)
        .section(GEN_SECTION)
        .advanced();
/// The centre of the test signal. 0 is 3.65 MHz.
pub const GEN_CENTRE: Setting<field::Hz> =
    Setting::new("gen_centre_hz", "Centre (0 = 3.65 MHz)", "Hz", 0.0)
        .range(0.0, 1e9, 1.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the HF beacon stack. The noise of the test signal comes from the standard `gen_*` settings.
    pub enum BeaconSetting {
        /// Frequency error.
        Ppm = rx::PPM,
        /// Step through the bands.
        Hop = rx::HOP,
        /// Dwell.
        Dwell = rx::DWELL,
        /// NCDXF/IARU.
        WatchNcdxf = rx::WATCH_NCDXF,
        /// Long Dashes.
        WatchDashes = rx::WATCH_DASHES,
        /// Single letters.
        WatchLetters = rx::WATCH_LETTERS,
        /// Your carriers.
        WatchCarriers = rx::WATCH_CARRIERS,
        /// Beacon list.
        List = rx::LIST,
        /// Clock offset.
        ClockOffset = rx::CLOCK_OFFSET,
        /// Recording start.
        StartUtc = rx::START_UTC,
        /// Listen.
        Listen = LISTEN,
        /// Listen to the beacon on.
        ListenKhz = LISTEN_KHZ,
        /// Call sign of the own beacon.
        Call = tx::CALL,
        /// Message of the own beacon.
        Message = tx::MESSAGE,
        /// Speed of the own beacon.
        Wpm = tx::WPM,
        /// Tone of the own beacon.
        Pitch = tx::PITCH_HZ,
        /// Long dash of the own beacon.
        LongDash = tx::LONG_DASH_S,
        /// Interval of the own beacon.
        Interval = tx::INTERVAL_MIN,
        /// Power steps.
        PowerSteps = tx::POWER_STEPS,
        /// Start in the interval.
        Offset = tx::OFFSET_S,
        /// Number of sendings.
        Count = tx::COUNT,
        /// Signal of the test signal.
        GenSignal = GEN_SIGNAL,
        /// Centre of the test signal.
        GenCentre = GEN_CENTRE,
    }
}

/// The HF beacon receiver graph.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn graph() -> Result<Graph, GraphError> {
    builder().build()
}

fn builder() -> GraphBuilder {
    GraphBuilder::new()
        .add("bank", rx::BeaconBank::default())
        .add("meter", rx::BeaconMeter::default())
        .add("runs", rx::RunDetector::default())
        .add("judges", rx::KeyingJudges::default())
        .add("stats", stats::BeaconStats::default())
        .add("front", HfChannelFrontEnd::default())
        .add("sb", SidebandAudio::new(Demod::Usb))
        .add("listen", Listen::default())
        .add("sender", tx::BeaconTransmitter::default())
        .add("tx", sdrkit_graph::components::TxSink::default())
        .input("bank")
        .input("stats.input")
        .input("front")
        .input("sender.in")
        .connect("bank.bins", "meter.bins")
        .connect("meter", "runs.excess")
        .connect("meter", "judges.excess")
        .connect("runs", "judges.runs")
        .connect("judges", "stats.detections")
        .connect("bank.plan", "meter.plan")
        .connect("bank.plan", "runs.plan")
        .connect("bank.plan", "judges.plan")
        .connect("bank.plan", "stats.plan")
        .connect("front", "sb")
        .connect("sb.audio", "listen")
        .connect("sender.out", "tx.in")
        .output("stats")
        .output("listen")
        .output("bank.requests")
        .output("sender.events")
        .clock(0.25)
        .status_from("stats")
        .reconfigure_on_retune()
}

/// The HF beacon receiver and the own beacon. With `hop` on and a live radio narrower than the watched list it requests the
/// host to step through the bands; a step ends a running own beacon. The settings of the own beacon and `start` and `stop`
/// go to its transmitter.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a component refuses a setting of `cfg`.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(
        crate::typed::shape("hfbeacons"),
        builder().catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        24e3,
        "HF beacons: the sample rate is too low (24 kHz at least)",
    )
    .min_rate_live(
        250e3,
        "HF beacons: the sample rate of a live radio is too low (250 kHz at least)",
    )
    .route("ppm", &["bank.ppm"])?
    .route(
        "clock_offset_s",
        &["judges.clock_offset_s", "stats.clock_offset_s"],
    )?
    .route("start_utc_s", &["judges.start_utc_s"])?
    .route("hop", &["bank.hop"])?
    .route("dwell_s", &["bank.dwell_s"])?
    .route("watch_ncdxf", &["bank.watch_ncdxf"])?
    .route("watch_dashes", &["bank.watch_dashes"])?
    .route("watch_letters", &["bank.watch_letters"])?
    .route("watch_carriers", &["bank.watch_carriers"])?
    .route("list_json", &["bank.list_json"])?
    .route("listen", &["listen.listen"])?
    .route("listen_khz", &["front.beacon_khz"])?
    .route("call", &["sender.call"])?
    .route("message", &["sender.message"])?
    .route("wpm", &["sender.wpm"])?
    .route("pitch_hz", &["sender.pitch_hz"])?
    .route("long_dash_s", &["sender.long_dash_s"])?
    .route("interval_min", &["sender.interval_min"])?
    .route("power_steps", &["sender.power_steps"])?
    .route("offset_s", &["sender.offset_s"])?
    .route("count", &["sender.count"])?
    .command("clear", sdrkit_graph::CommandTarget::every("clear"))
    .command(
        "list.restore",
        sdrkit_graph::CommandTarget::of("bank", "list.restore"),
    )
    .command("start", sdrkit_graph::CommandTarget::of("sender", "start"))
    .command("stop", sdrkit_graph::CommandTarget::of("sender", "stop"))
    .command("send", sdrkit_graph::CommandTarget::of("sender", "send"))
    .with_settings(&cfg.settings)
    .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::BeaconGenerator::from_config(cfg)))
}

/// HF beacons.
pub static HFBEACONS: StackDescriptor = StackDescriptor {
    id: "hfbeacons",
    name: "HF beacons",
    group: "HF utility",
    blurb: "Propagation monitor: the NCDXF/IARU beacons, the Long Dashes and the single-letter beacons heard in a wide HF capture.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        no_offset_tuning: true,
        ..Tuning::anywhere(14.5e6, 10e6, 250e3, 8e6)
    },
    submodes: &[],
    settings: BeaconSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::CwMorse],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::HFBEACONS),
    benchmarks: None,
};
