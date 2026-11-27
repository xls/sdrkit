//! Airband: several AM voice channels of the user's list from one capture, each squelched and demodulated, the radio's tuning
//! error measured from the carriers, a band watch of carriers not in the list, a mixer with mute, solo, priority and scan, a
//! transmission log, and push-to-talk AM voice on the tuned channel ([`transmit`]).
//! Standard: ICAO Annex 10 Volume III Part II and Volume V.

pub mod bank;
pub mod channel;
pub mod generator;
pub mod mixer;
pub mod names;
pub mod tables;
pub mod transmit;
pub mod watch;

use crate::verify;
use channel::ChannelStatus;
use sdrkit_components::dsp::AudioResampler;
use sdrkit_components::tx::TxMute;
use sdrkit_core::publish::field;
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results, Setting,
    SettingSet, SignalType, StackConfig, StackDescriptor, Tuning,
};
use sdrkit_graph::components::{HostIn, TxSink};
use sdrkit_graph::{
    Catalog, Component, Graph, GraphBuilder, GraphError, Item, Registry, StackGraph,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use generator::{AirbandGenerator, GenParams};

/// The stack id.
pub const ID: &str = "airband";
/// Below this the receiver refuses.
pub const MIN_RATE_HZ: f64 = 250e3;
/// The radio is tuned this far above the dial (25 kHz x 13/6).
pub const TUNE_OFFSET_HZ: f64 = 25e3 * 13.0 / 6.0;

/// A transmission (an opening of a channel).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Activity {
    /// Channel index in the list.
    pub channel: usize,
    /// Its name.
    pub name: String,
    /// Its label.
    pub label: String,
    /// Hz.
    pub freq_hz: f64,
    /// Signal time of the start.
    pub start_s: f64,
    /// Length (0 while on).
    pub dur_s: f64,
    /// Strongest C/N.
    pub cn_db: f32,
    /// Channel power, dBFS.
    pub level_db: f32,
    /// Two stations.
    pub heterodyne: bool,
}

impl Item for Activity {
    const KIND: &'static str = "airband.activity";
}

/// What the mixer needs to know of a channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gate {
    /// Hz.
    pub freq_hz: f64,
    /// The squelch is open.
    pub open: bool,
    /// Muted.
    pub mute: bool,
    /// Solo.
    pub solo: bool,
    /// Priority.
    pub priority: bool,
}

/// The 8 kHz audio of every channel in band for one piece, with their gates. The slices are shared and reused by the bank.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelAudio {
    /// Samples per channel.
    pub len: usize,
    /// The audio, channel after channel, `len` samples each.
    pub audio: Arc<[f32]>,
    /// One gate per channel.
    pub gates: Arc<[Gate]>,
}

impl ChannelAudio {
    /// The audio of channel `c`.
    pub fn channel(&self, c: usize) -> &[f32] {
        self.audio
            .get(c * self.len..(c + 1) * self.len)
            .unwrap_or(&[])
    }
}

impl Item for ChannelAudio {
    const KIND: &'static str = "airband.audio";
}

/// The bank at a report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BankState {
    /// Signal time.
    pub time_s: f64,
    /// Input rate.
    pub rate_hz: f64,
    /// Centre.
    pub centre_hz: f64,
    /// Channel rate.
    pub channel_rate_hz: f64,
    /// Tuning error.
    pub tune_error_hz: f64,
    /// Measured.
    pub known: bool,
    /// Global squelch.
    pub squelch_db: f64,
    /// Hang.
    pub hang_s: f64,
    /// Every listed channel.
    pub channels: Vec<ChannelStatus>,
    /// Power of the whole input.
    pub level_db: f32,
    /// New log lines.
    pub log: Vec<String>,
    /// Open transmissions: (channel, start, best C/N).
    pub open_since: Vec<(usize, f64, f32)>,
}

impl Item for BankState {
    const KIND: &'static str = "airband.bank_state";
}

/// A carrier on the air that is not in the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Seen {
    /// Channel frequency, Hz.
    pub freq_hz: f64,
    /// 8.33.
    pub is833: bool,
    /// Name.
    pub name: String,
    /// Excess over the noise in one bin at its strongest, dB.
    pub excess_db: f32,
    /// Reports present (0.5 s each).
    pub reports: u64,
    /// Last present, signal time.
    pub last_s: f64,
    /// Active now.
    pub active: bool,
    /// The tuning error was guessed from the raster.
    pub approximate: bool,
}

/// The band watch list at a report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SeenList {
    /// Most active first.
    pub entries: Vec<Seen>,
}

impl Item for SeenList {
    const KIND: &'static str = "airband.seen";
}

/// What the mixer plays.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanState {
    /// Scanning.
    pub scan: bool,
    /// The frequency (Hz, rounded) the scan stopped on, -1 while scanning.
    pub channel: i64,
    /// The frequencies (Hz, rounded) playing.
    pub playing: Arc<[i64]>,
}

impl Item for ScanState {
    const KIND: &'static str = "airband.scan";
}

/// The channel list.
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Channels", "", "")
    .help("The channel list (v3: records). Empty uses the test layout around the dial.")
    .restart();
/// The squelch.
pub const SQUELCH_DB: Setting<field::F64> = Setting::new("squelch_db", "Squelch", "dB", 6.0)
    .range(-5.0, 30.0, 0.5)
    .help("Carrier-to-noise to open a channel.");
/// The hang time.
pub const HANG_S: Setting<field::Duration> = Setting::new("hang_s", "Hang", "s", 0.5)
    .range(0.0, 3.0, 0.1)
    .help("How long a channel stays open after its carrier goes.");
/// Scan the open channels.
pub const SCAN: Setting<field::Bool> =
    Setting::new("scan", "Scan", "", false).help("Plays one open channel at a time, in turn.");
/// Stay on the channel the scan stopped on.
pub const SCAN_HOLD: Setting<field::Bool> =
    Setting::new("scan_hold", "Hold", "", false).help("Stays on the channel the scan stopped on.");
/// Take the steady noise out of the audio.
pub const NOISE_REMOVER: Setting<field::Bool> =
    Setting::new("noise_remover", "Noise remover", "", false)
        .help("Takes the steady noise out of the audio.");
/// How much of the noise remover's output is heard.
pub const NOISE_REMOVER_STRENGTH: Setting<field::F64> =
    Setting::new("noise_remover_strength", "Strength", "%", 80.0)
        .range(0.0, 100.0, 1.0)
        .help("How much of the noise remover's output is heard.");
/// Show only the open channels.
pub const ONLY_ACTIVE: Setting<field::Bool> = Setting::new("only_active", "Only active", "", false)
    .help("Shows only the open channels in the channel table.");
/// Play the audio.
pub const LISTEN: Setting<field::Bool> =
    Setting::new("listen", "Listen", "", true).help("Plays the audio.");

sdrkit_core::settings! {
    /// The settings of the Airband stack.
    pub enum AirbandSetting {
        /// The channel list.
        Channels = CHANNELS,
        /// The squelch.
        SquelchDb = SQUELCH_DB,
        /// The hang time.
        HangS = HANG_S,
        /// Scan the open channels.
        Scan = SCAN,
        /// Stay on the channel the scan stopped on.
        ScanHold = SCAN_HOLD,
        /// Take the steady noise out of the audio.
        NoiseRemover = NOISE_REMOVER,
        /// How much of the noise remover's output is heard.
        NoiseRemoverStrength = NOISE_REMOVER_STRENGTH,
        /// Show only the open channels.
        OnlyActive = ONLY_ACTIVE,
        /// Play the audio.
        Listen = LISTEN,
        /// The channel spacing of the transmitter.
        TxSpacing = transmit::TX_SPACING,
        /// The peak modulation depth of the transmitter.
        TxAmDepth = transmit::TX_AM_DEPTH,
        /// The microphone gain of the transmitter.
        TxMicGain = transmit::TX_MIC_GAIN,
        /// The time-out of push-to-talk.
        TxTimeout = transmit::TX_TIMEOUT,
    }
}

/// The Airband graph: the receiver, and the transmit branch from the host's audio input (`host.in`) into `tx.sink`. The mute
/// (`tx.mute`) silences the audio of the receiver while the sender is keyed.
///
/// # Errors
///
/// The errors of [`graph_with_mic`].
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    graph_with_mic(catalog, Box::new(HostIn::default()))
}

/// As [`graph`], with `mic` in place of the host's audio input: a component with input `in` (the session's samples) and
/// audio output `out`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`], and [`GraphError::NamedSetting`] if the audio resampler refuses its rate.
pub fn graph_with_mic(
    catalog: &Arc<Catalog>,
    mic: Box<dyn Component>,
) -> Result<Graph, GraphError> {
    let resample = GraphError::checked("resample.rate", AudioResampler::new(48e3))?;
    GraphBuilder::new()
        .add("bank", bank::Bank::default())
        .add("watch", watch::Watch::default())
        .add("mixer", mixer::Mixer::default())
        .add("denoise", mixer::NoiseRemover::default())
        .add("resample", resample)
        .add("tables", tables::Tables::default())
        .add_boxed("mic", mic)
        .add("transmit", transmit::AirbandTransmit::default())
        .add("tx", TxSink::default())
        .add("mute", TxMute::default())
        .input("bank")
        .input("watch.in")
        .input("mic.in")
        .input("transmit.in")
        .connect("bank.audio", "mixer")
        .connect("mixer.out", "denoise")
        .connect("mixer.scan", "tables.scan")
        .connect("denoise", "resample")
        .connect("bank.state", "tables.state")
        .connect("bank.activity", "tables.activity")
        .connect("watch.seen", "tables.seen")
        .connect("mic.out", "transmit.mic")
        .connect("transmit.out", "tx.in")
        .connect("resample", "mute.in")
        .connect("transmit.keyed", "mute.keyed")
        .feedback("bank.state", "watch.state")
        .output("mute.out")
        .output("tables.out")
        .clock(0.25)
        .reconfigure_on_retune()
        .status_from("tables")
        .catalog(catalog)
        .build()
}

/// The Airband receiver with these settings.
///
/// # Errors
///
/// The errors of [`graph`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver_with_mic(cfg, Box::new(HostIn::default()))
}

/// As [`receiver`], with `mic` in place of the host's audio input (see [`graph_with_mic`]).
///
/// # Errors
///
/// The errors of [`graph_with_mic`] and of [`sdrkit_graph::StackGraphBuilder::build`].
pub fn receiver_with_mic(
    cfg: &StackConfig,
    mic: Box<dyn Component>,
) -> Result<StackGraph, GraphError> {
    let tx = |key: &str| format!("transmit.{key}");
    let of = sdrkit_graph::CommandTarget::of;
    StackGraph::of(
        crate::typed::shape(ID),
        graph_with_mic(&Catalog::of(cfg)?, mic)?,
    )
    .min_rate(
        MIN_RATE_HZ,
        "Airband needs a sample rate of at least 250 kHz",
    )
    .default_centre(118.7e6 + TUNE_OFFSET_HZ)
    .guard_limit(2.0)
    .route(CHANNELS.key(), &["bank.channels"])?
    .route(SQUELCH_DB.key(), &["bank.squelch_db"])?
    .route(HANG_S.key(), &["bank.hang_s"])?
    .route(SCAN.key(), &["mixer.scan"])?
    .route(SCAN_HOLD.key(), &["mixer.scan_hold"])?
    .route(LISTEN.key(), &["mixer.listen"])?
    .route(NOISE_REMOVER.key(), &["denoise.on"])?
    .route(NOISE_REMOVER_STRENGTH.key(), &["denoise.strength"])?
    .route(ONLY_ACTIVE.key(), &["tables.only_active"])?
    .route(
        transmit::TX_SPACING.key(),
        &[&tx(transmit::TX_SPACING.key())],
    )?
    .route(
        transmit::TX_AM_DEPTH.key(),
        &[&tx(transmit::TX_AM_DEPTH.key())],
    )?
    .route(
        transmit::TX_MIC_GAIN.key(),
        &[&tx(transmit::TX_MIC_GAIN.key())],
    )?
    .route(
        transmit::TX_TIMEOUT.key(),
        &[&tx(transmit::TX_TIMEOUT.key())],
    )?
    .command("clear", of("tables", "clear"))
    .command(
        crate::typed::PTT.command.key,
        of("transmit", crate::typed::PTT.command.key),
    )
    .command(
        crate::typed::HALT.key,
        of("transmit", crate::typed::HALT.key),
    )
    .with_settings(&cfg.settings)
    .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("avdata.airband_bank", || Box::new(bank::Bank::default()));
    r.add("avdata.airband_watch", || Box::new(watch::Watch::default()));
    r.add("avdata.airband_mixer", || Box::new(mixer::Mixer::default()));
    r.add("avdata.airband_noise_remover", || {
        Box::new(mixer::NoiseRemover::default())
    });
    r.add("avdata.airband_tables", || {
        Box::new(tables::Tables::default())
    });
    r.add("avdata.airband_transmit", || {
        Box::new(transmit::AirbandTransmit::default())
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(AirbandGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, five channels", o, 0.0);
    c.seconds = o.seconds.max(8.0);
    c.expect_audio = Some(true);
    c.check = Some(check_activity);
    vec![c]
}

fn check_activity(r: &Results) -> Vec<CheckLine> {
    vec![verify::at_least(r, "transmissions", 2.0, "transmissions")]
}

/// The Airband stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "Airband",
    group: "Aviation",
    blurb: "Aircraft voice on VHF AM: several channels at once, with squelch, scanning, a transmission log and a band watch, and push-to-talk AM on the tuned channel.",
    tuning: Tuning {
        host_audio_channels: 1,
        kind: Some(sdrkit_core::TuningKind::Channels),
        demod: Some(sdrkit_core::PlainDemod::new(sdrkit_core::DemodMode::Am, 8.33e3)),
        min_hz: 108e6,
        max_hz: 140e6,
        default_hz: 118.7e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE_HZ,
        bandwidth_hz: 1.6e6,
        tune_offset_hz: TUNE_OFFSET_HZ,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: AirbandSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::AmVoice],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};
