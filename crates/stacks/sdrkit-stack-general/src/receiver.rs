//! The general receiver: the graph of [`build_graph`] in a stack graph for each channel, and [`GeneralReceiver`], which maps the
//! settings of the stack onto the components, keeps a width and a step per demodulator, and runs up to three extra channels from
//! the same capture. The main channel has the transmit branch (`general.transmit` into `tx.sink`); the extra channels receive
//! only.

use crate::amproc::AmProcessorComponent;
use crate::audio::{AudioFilters, Clip, Gate, Select, VoiceNr};
use crate::blanker::ChannelBlanker;
use crate::channel::{ChannelDecimator, ChannelFilter, ExactResampler};
use crate::demod::DemodComponent;
use crate::noise::NoiseComponent;
use crate::presets::preset;
use crate::report::Report;
use crate::results::declaration;
use crate::settings::{GeneralSetting, GeneralSettings, WIDTH};
use crate::squelch::SquelchComponent;
use crate::toneid::ToneIdComponent;
use crate::transmit::settings::TX_KEYS;
use crate::transmit::Transmitter;
use crate::typed::{
    self, ChannelRow, GeneralFields, PresetApplyArgs, CHANNELS, HALT, PRESETS, PRESET_APPLY, PTT,
    SEND, TUNE,
};
use crate::types::Demod;
use sdrkit_components::dsp::RealToAudio;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, EventDecl,
    HostRequest, InputInfo, Output, Owner, Receiver, Record, Results, ResultsDecl, Schema,
    SettingError, SettingId, SettingSet, StackConfig, StageError, StateBlob, StateError, Station,
    Value, Values,
};
use sdrkit_graph::components::TxSink;
use sdrkit_graph::{
    Catalog, Component, Graph, GraphBuilder, GraphError, Kept, SettingTarget, StackGraph,
};
use sdrkit_proto_tones::components::{DtmfComponent, SubAudioComponent};
use std::sync::Arc;

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 250e3;
/// What the receiver says when the rate is too low.
const RATE_MESSAGE: &str = "The general stack needs a sample rate of at least 250 ksps";
/// The input guard limit: a sample beyond magnitude 2 is brought back to 2.
pub const GUARD_LIMIT: f32 = 2.0;

/// Builds the graph of the main channel, with its transmit branch (not yet configured).
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn build_graph() -> Result<Graph, GraphError> {
    graph_builder(Mic::None).build()
}

/// The microphone input of the transmitter of a channel.
pub enum Mic {
    /// A receive-only channel: no transmit branch.
    Off,
    /// The transmit branch with the audio of the host at its microphone input (`host.in`): silence while the host
    /// gives none.
    None,
    /// The transmit branch with this audio source at its microphone input: a component with an input `in` (the session's
    /// samples, which give the pace) and an audio output `out`.
    Source(Box<dyn Component>),
}

fn graph_builder(mic: Mic) -> GraphBuilder {
    let b = GraphBuilder::new()
        .add("decimate", ChannelDecimator::default())
        .add("blanker", ChannelBlanker::default())
        .add("noise", NoiseComponent::default())
        .add("resample", ExactResampler::default())
        .add("filter", ChannelFilter::default())
        .add("squelch", SquelchComponent::default())
        .add("demod", DemodComponent::default())
        .add("amproc", AmProcessorComponent::default())
        .add("select", Select::default())
        .add("clip", Clip)
        .add("gate", Gate::default())
        .add("afilters", AudioFilters::default())
        .add("voice_nr", VoiceNr::default())
        .add("audio", RealToAudio::default())
        .add("subaudio", SubAudioComponent::default())
        .add("dtmf", DtmfComponent::default())
        .add("toneid", ToneIdComponent::default())
        .add("report", Report::default())
        .input("decimate")
        .connect("decimate", "blanker.in")
        .connect("blanker.out", "resample")
        .connect("blanker.out", "noise")
        .connect("resample", "filter")
        .connect("resample", "amproc.in")
        .connect("filter", "squelch.in")
        .connect("noise", "squelch.noise")
        .connect("filter", "demod.in")
        .connect("demod.out", "select.a")
        .connect("amproc.out", "select.b")
        .connect("select", "clip")
        .connect("clip", "gate.in")
        .connect("squelch.out", "gate.sq")
        .connect("toneid", "gate.tone")
        .connect("gate", "afilters")
        .connect("afilters", "voice_nr.in")
        .connect("voice_nr.out", "audio")
        .connect("clip", "subaudio")
        .connect("clip", "dtmf")
        .connect("clip", "toneid.audio")
        .connect("subaudio", "toneid.in")
        .connect("squelch.out", "report.sq")
        .connect("voice_nr.out", "report.audio")
        .connect("demod.report", "report.demod")
        .connect("amproc.status", "report.am")
        .connect("blanker.report", "report.nb")
        .connect("voice_nr.speech", "report.speech")
        .connect("toneid", "report.tone")
        .output("audio")
        .output("subaudio")
        .output("dtmf")
        .clock(0.1)
        .reconfigure_on_retune()
        .status_from("report");
    let b = match mic {
        Mic::Off => return b,
        Mic::None => b
            .add("mic", sdrkit_graph::components::HostIn::default())
            .input("mic.in")
            .connect("mic.out", "transmit.mic"),
        Mic::Source(c) => b
            .add_boxed("mic", c)
            .input("mic.in")
            .connect("mic.out", "transmit.mic"),
    };
    b.add("transmit", Transmitter::default())
        .add("tx", TxSink::default())
        .input("transmit.in")
        .connect("transmit.out", "tx.in")
        .connect("transmit.keyed", "gate.tx")
}

/// The receiver of one channel: the graph in a stack graph (minimum rate, input guard, station, saved state), with the results
/// of the general receiver declared.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build.
pub fn stack_graph() -> Result<StackGraph, GraphError> {
    stack_graph_with(graph_builder(Mic::None))
}

fn stack_graph_with(builder: GraphBuilder) -> Result<StackGraph, GraphError> {
    StackGraph::builder("general", builder.build()?)
        .min_rate(MIN_RATE_HZ, RATE_MESSAGE)
        .guard_limit(GUARD_LIMIT)
        .declare(&declaration())
        .build()
}

/// What a change of a setting touches, so the change sets only what it must.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Touch {
    All,
    Demod,
    Width,
    Offset,
    Squelch,
    Gain,
    Deemph,
    NfmLowpass,
    Pitch,
    Am,
    Blanker,
    AudioFilters,
    VoiceNr,
    Tone,
    Custom,
    Presets,
    Transmit,
}

fn touch_of(s: GeneralSetting) -> Option<Touch> {
    use GeneralSetting as S;
    Some(match s {
        S::Demod => Touch::Demod,
        S::Width => Touch::Width,
        S::ChannelOffset => Touch::Offset,
        S::Squelch | S::SquelchLevel => Touch::Squelch,
        S::Agc | S::Gain => Touch::Gain,
        S::DeemphNfm | S::DeemphWfm => Touch::Deemph,
        S::NfmLowpass => Touch::NfmLowpass,
        S::CwPitch => Touch::Pitch,
        S::AmAuto | S::AmDetector | S::AmNr | S::AmBandwidth | S::AmEnhance => Touch::Am,
        S::Nb | S::NbThreshold => Touch::Blanker,
        S::AfHighpass | S::AfLowpass | S::AfNotch => Touch::AudioFilters,
        S::VoiceNr | S::VoiceNrStrength => Touch::VoiceNr,
        S::ToneGate => Touch::Tone,
        S::CustomFilter => Touch::Custom,
        S::Presets => Touch::Presets,
        S::TxSplit
        | S::TxDeviation
        | S::TxPreemphasis
        | S::TxCtcss
        | S::TxAmDepth
        | S::TxSsbMethod
        | S::TxSsbBand
        | S::TxCompression
        | S::TxTimeout
        | S::TxCwWpm
        | S::TxCwFarnsworth
        | S::TxTuneLimit => Touch::Transmit,
        _ => return None,
    })
}

/// The component settings that the receiver writes, found once in the graph of a channel.
struct Targets {
    decimate_offset: SettingTarget,
    decimate_wide: SettingTarget,
    blanker_enabled: SettingTarget,
    blanker_threshold: SettingTarget,
    noise_lo: SettingTarget,
    noise_hi: SettingTarget,
    filter_lo: SettingTarget,
    filter_hi: SettingTarget,
    filter_custom: SettingTarget,
    squelch_kind: SettingTarget,
    squelch_level: SettingTarget,
    squelch_ssb: SettingTarget,
    demod_demod: SettingTarget,
    demod_agc: SettingTarget,
    demod_gain: SettingTarget,
    demod_deemph: SettingTarget,
    demod_pitch: SettingTarget,
    demod_nfm_lowpass: SettingTarget,
    am_active: SettingTarget,
    am_auto: SettingTarget,
    am_detector: SettingTarget,
    am_bandwidth: SettingTarget,
    am_nr: SettingTarget,
    am_nb: SettingTarget,
    am_nb_threshold: SettingTarget,
    am_enhance: SettingTarget,
    am_agc: SettingTarget,
    am_gain: SettingTarget,
    am_width: SettingTarget,
    select_b: SettingTarget,
    af: [SettingTarget; 3],
    voice_nr_enabled: SettingTarget,
    voice_nr_strength: SettingTarget,
    subaudio_ctcss: SettingTarget,
    subaudio_dcs: SettingTarget,
    toneid_enabled: SettingTarget,
    toneid_gate: SettingTarget,
    report_demod: SettingTarget,
    report_width: SettingTarget,
    report_offset: SettingTarget,
    report_dial: SettingTarget,
    report_rate: SettingTarget,
    report_presets: SettingTarget,
}

impl Targets {
    fn find(g: &Graph) -> Result<Self, GraphError> {
        let t = |key: &str| {
            g.target(key).map_err(|error| {
                GraphError::Setting {
                    key: key.into(),
                    error,
                }
                .named(Owner::schema(g))
            })
        };
        Ok(Targets {
            decimate_offset: t("decimate.offset")?,
            decimate_wide: t("decimate.wide")?,
            blanker_enabled: t("blanker.enabled")?,
            blanker_threshold: t("blanker.threshold")?,
            noise_lo: t("noise.lo")?,
            noise_hi: t("noise.hi")?,
            filter_lo: t("filter.lo")?,
            filter_hi: t("filter.hi")?,
            filter_custom: t("filter.custom")?,
            squelch_kind: t("squelch.kind")?,
            squelch_level: t("squelch.level")?,
            squelch_ssb: t("squelch.ssb")?,
            demod_demod: t("demod.demod")?,
            demod_agc: t("demod.agc")?,
            demod_gain: t("demod.gain")?,
            demod_deemph: t("demod.deemph")?,
            demod_pitch: t("demod.pitch")?,
            demod_nfm_lowpass: t("demod.nfm_lowpass")?,
            am_active: t("amproc.active")?,
            am_auto: t("amproc.auto")?,
            am_detector: t("amproc.detector")?,
            am_bandwidth: t("amproc.bandwidth")?,
            am_nr: t("amproc.nr")?,
            am_nb: t("amproc.nb")?,
            am_nb_threshold: t("amproc.nb_threshold")?,
            am_enhance: t("amproc.enhance")?,
            am_agc: t("amproc.agc")?,
            am_gain: t("amproc.gain")?,
            am_width: t("amproc.width")?,
            select_b: t("select.b")?,
            af: [
                t("afilters.highpass")?,
                t("afilters.lowpass")?,
                t("afilters.notch")?,
            ],
            voice_nr_enabled: t("voice_nr.enabled")?,
            voice_nr_strength: t("voice_nr.strength")?,
            subaudio_ctcss: t("subaudio.ctcss")?,
            subaudio_dcs: t("subaudio.dcs")?,
            toneid_enabled: t("toneid.enabled")?,
            toneid_gate: t("toneid.gate")?,
            report_demod: t("report.demod")?,
            report_width: t("report.width")?,
            report_offset: t("report.channel_offset")?,
            report_dial: t("report.dial_hz")?,
            report_rate: t("report.input_rate")?,
            report_presets: t("report.presets")?,
        })
    }
}

/// The settings and the commands of the transmitter of the main channel, found once in its graph.
struct TxTargets {
    demod: SettingTarget,
    width: SettingTarget,
    offset: SettingTarget,
    /// The transmit settings, in the order of [`TX_KEYS`].
    settings: Vec<SettingTarget>,
    ptt: CommandId,
    send: CommandId,
    tune: CommandId,
    halt: CommandId,
}

impl TxTargets {
    fn find(g: &Graph) -> Result<Self, GraphError> {
        let t = |key: &str| {
            let key = format!("transmit.{key}");
            g.target(&key)
                .map_err(|error| GraphError::Setting { key, error }.named(Owner::schema(g)))
        };
        let c = |key: &str| {
            let key = format!("transmit.{key}");
            Owner::commands(g)
                .id(&key)
                .map_err(|_| GraphError::UnresolvedCommand {
                    key: key.clone(),
                    target: sdrkit_graph::CommandTarget::of("transmit", &key[9..]),
                })
        };
        Ok(TxTargets {
            demod: t("demod")?,
            width: t("width")?,
            offset: t("channel_offset")?,
            settings: TX_KEYS.iter().map(|k| t(k)).collect::<Result<_, _>>()?,
            ptt: c(PTT.command.key)?,
            send: c(SEND.command.key)?,
            tune: c(TUNE.key)?,
            halt: c(HALT.key)?,
        })
    }
}

/// The component commands that the receiver runs, found once in the graph of a channel.
struct Restarts {
    noise: CommandId,
    squelch: CommandId,
    squelch_noise: CommandId,
    demod: CommandId,
    amproc: CommandId,
    toneid: CommandId,
    report: CommandId,
}

impl Restarts {
    fn find(g: &Graph) -> Result<Self, GraphError> {
        let c = |key: &str| {
            Owner::commands(g)
                .id(key)
                .map_err(|_| GraphError::UnresolvedCommand {
                    key: key.into(),
                    target: key.split_once('.').map_or_else(
                        || sdrkit_graph::CommandTarget::every(key),
                        |(c, k)| sdrkit_graph::CommandTarget::of(c, k),
                    ),
                })
        };
        Ok(Restarts {
            noise: c("noise.restart")?,
            squelch: c("squelch.restart")?,
            squelch_noise: c("squelch.restart_noise")?,
            demod: c("demod.restart")?,
            amproc: c("amproc.restart")?,
            toneid: c("toneid.restart")?,
            report: c("report.restart")?,
        })
    }
}

/// One channel: a graph with its own demodulator, width memory and offset. The shared settings come from the main channel.
struct Chan {
    graph: StackGraph,
    s: GeneralSettings,
    t: Targets,
    tx: Option<TxTargets>,
    restarts: Restarts,
    no_args: Values,
    edges: (f64, f64),
    subaudio_on: Option<bool>,
}

impl Chan {
    /// A channel with settings `s`. The main channel receives `own`: the results the stack writes itself (the extra channels),
    /// and `mic`: its transmit branch.
    fn new(
        s: GeneralSettings,
        catalog: &Arc<Catalog>,
        own: Option<Published>,
        mic: Mic,
    ) -> Result<Self, ConfigError> {
        let transmit = !matches!(mic, Mic::Off);
        let mut builder = graph_builder(mic).catalog(catalog);
        if let Some(own) = own {
            builder = builder.publish(own);
        }
        let graph = stack_graph_with(builder)?;
        let mut c = Chan {
            t: Targets::find(graph.graph())?,
            tx: if transmit {
                Some(TxTargets::find(graph.graph())?)
            } else {
                None
            },
            restarts: Restarts::find(graph.graph())?,
            graph,
            s,
            no_args: Values::defaults(&Arc::new(Schema::of_arguments(&[]))),
            edges: (-6250.0, 6250.0),
            subaudio_on: None,
        };
        c.push(Touch::All)?;
        Ok(c)
    }
    /// Sets a component setting.
    fn set(&mut self, t: SettingTarget, v: Value) -> Result<(), SettingError> {
        self.graph.graph_mut().apply_to(t, &v)
    }
    /// Runs a component command that takes no arguments.
    fn run(&mut self, id: CommandId) -> Result<(), ConfigError> {
        Owner::invoke(self.graph.graph_mut(), id, &self.no_args)
            .map(|_| ())
            .map_err(|e| ConfigError::Stage(StageError::new(e)))
    }
    fn am_values(&mut self) -> Result<(), SettingError> {
        use crate::amproc::AmDetector;
        let s = &self.s;
        let active = s.am_processor();
        let det = match s.am_detector {
            AmDetector::Auto if !s.am_auto => AmDetector::Envelope,
            AmDetector::Envelope if s.am_auto => AmDetector::Auto,
            d => d,
        };
        let blanker = s.nb && !s.demod.is_wide() && !active;
        let values = [
            (self.t.am_auto, Value::Bool(s.am_auto)),
            (self.t.am_detector, Value::Choice(det.index())),
            (self.t.am_bandwidth, Value::Hz(s.am_bandwidth)),
            (self.t.am_nr, Value::Choice(s.am_nr.index())),
            (self.t.am_enhance, Value::Bool(s.am_enhance)),
            (
                self.t.am_width,
                Value::Hz(s.width().clamp(1000.0, 20_000.0)),
            ),
            (self.t.am_active, Value::Bool(active)),
            (self.t.select_b, Value::Bool(active)),
            (self.t.blanker_enabled, Value::Bool(blanker)),
        ];
        for (t, v) in values {
            self.set(t, v)?;
        }
        Ok(())
    }
    fn edges(&mut self) -> Result<(), SettingError> {
        let (lo, hi) = self.s.demod.edges(self.s.width());
        // The filter refuses a low edge at or above its high edge, so the order keeps them apart.
        if lo < self.edges.1 {
            self.set(self.t.filter_lo, Value::Hz(lo))?;
            self.set(self.t.filter_hi, Value::Hz(hi))?;
        } else {
            self.set(self.t.filter_hi, Value::Hz(hi))?;
            self.set(self.t.filter_lo, Value::Hz(lo))?;
        }
        self.edges = (lo, hi);
        self.set(self.t.noise_lo, Value::Hz(lo))?;
        self.set(self.t.noise_hi, Value::Hz(hi))?;
        self.set(self.t.report_width, Value::Hz(self.s.width()))
    }
    fn voice(&mut self) -> Result<(), SettingError> {
        let on = self.s.voice_nr();
        let strength = self.s.mem.voice_nr_strength[self.s.demod.slot()];
        self.set(self.t.voice_nr_enabled, Value::Bool(on))?;
        self.set(self.t.voice_nr_strength, Value::F64(strength))
    }
    fn deemph(&mut self) -> Result<(), SettingError> {
        self.set(self.t.demod_deemph, Value::F64(self.s.deemph_us()))
    }
    /// Pushes the channel and the transmit settings that a change touches into the transmitter.
    fn push_transmit(&mut self, t: Touch) -> Result<(), SettingError> {
        let Some(tx) = &self.tx else {
            return Ok(());
        };
        let all = t == Touch::All;
        let mut values: Vec<(SettingTarget, Value)> = Vec::new();
        if all || t == Touch::Demod {
            values.push((tx.demod, Value::Choice(self.s.demod.index())));
        }
        if all || matches!(t, Touch::Demod | Touch::Width) {
            values.push((tx.width, Value::Hz(self.s.width())));
        }
        if all || t == Touch::Offset {
            values.push((tx.offset, Value::Hz(self.s.channel_offset)));
        }
        if all || t == Touch::Transmit {
            values.extend(tx.settings.iter().copied().zip(self.s.tx.values()));
        }
        for (target, v) in values {
            self.set(target, v)?;
        }
        Ok(())
    }
    /// Runs the transmit command `id` of the main channel with `args`.
    fn transmit(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        Owner::invoke(self.graph.graph_mut(), id, args)
    }
    /// Whether the transmitter of the channel mutes the receive audio.
    fn keyed(&self) -> bool {
        self.graph
            .graph()
            .component_as::<Transmitter>("transmit")
            .is_some_and(Transmitter::keyed)
    }
    /// Pushes the settings that a change touches into the graph.
    fn push(&mut self, t: Touch) -> Result<(), ConfigError> {
        let all = t == Touch::All;
        self.push_transmit(t)?;
        if all || t == Touch::Custom {
            let c = Value::Text(self.s.custom_filter.clone());
            self.set(self.t.filter_custom, c)?;
        }
        if all || t == Touch::Squelch {
            self.set(self.t.squelch_kind, Value::Choice(self.s.squelch.index()))?;
            self.set(self.t.squelch_level, Value::F64(self.s.squelch_level))?;
        }
        if all || t == Touch::Gain {
            let (agc, gain) = (Value::Bool(self.s.agc), Value::F64(self.s.gain));
            self.set(self.t.demod_agc, agc.clone())?;
            self.set(self.t.demod_gain, gain.clone())?;
            self.set(self.t.am_agc, agc)?;
            self.set(self.t.am_gain, gain)?;
        }
        if all || t == Touch::Pitch {
            self.set(self.t.demod_pitch, Value::Hz(self.s.cw_pitch))?;
        }
        if all || t == Touch::Deemph {
            self.deemph()?;
        }
        if all || t == Touch::NfmLowpass {
            let l = Value::Choice(self.s.nfm_lowpass.index());
            self.set(self.t.demod_nfm_lowpass, l)?;
        }
        if all || t == Touch::Blanker {
            self.set(self.t.blanker_threshold, Value::F64(self.s.nb_threshold))?;
            self.set(self.t.am_nb, Value::Bool(self.s.nb))?;
            self.set(self.t.am_nb_threshold, Value::F64(self.s.nb_threshold))?;
            let on = self.s.nb && !self.s.demod.is_wide() && !self.s.am_processor();
            self.set(self.t.blanker_enabled, Value::Bool(on))?;
        }
        if all || t == Touch::Am {
            self.am_values()?;
        }
        if all || t == Touch::AudioFilters {
            for i in 0..3 {
                self.set(self.t.af[i], Value::Hz(self.s.af[i]))?;
            }
        }
        if all || t == Touch::VoiceNr {
            self.voice()?;
        }
        if all || t == Touch::Presets {
            let p = Value::Text(self.s.presets.clone());
            self.set(self.t.report_presets, p)?;
        }
        if all || t == Touch::Tone {
            let g = Value::Text(self.s.tone_gate.clone());
            self.set(self.t.toneid_gate, g)?;
        }
        if all || t == Touch::Demod {
            let d = self.s.demod;
            if !all {
                // A new demodulator starts the noise estimate and the tone identification again.
                self.run(self.restarts.squelch_noise)?;
                self.run(self.restarts.toneid)?;
            }
            self.set(self.t.demod_demod, Value::Choice(d.index()))?;
            self.set(self.t.squelch_ssb, Value::Bool(d.is_ssb()))?;
            self.set(self.t.report_demod, Value::Choice(d.index()))?;
            let nfm = d == Demod::Nfm;
            self.set(self.t.toneid_enabled, Value::Bool(nfm))?;
            // The sub-audio decoders run on NFM only. A change reformats them, so it is made only when the value changes.
            if self.subaudio_on != Some(nfm) {
                self.subaudio_on = Some(nfm);
                self.set(self.t.subaudio_ctcss, Value::Bool(nfm))?;
                self.set(self.t.subaudio_dcs, Value::Bool(nfm))?;
            }
            self.deemph()?;
            self.voice()?;
            self.edges()?;
            self.am_values()?;
            if !all {
                self.run(self.restarts.amproc)?;
            }
            // Last: a change between WFM and a narrow demodulator reformats the whole chain.
            self.set(self.t.decimate_wide, Value::Bool(d.is_wide()))?;
        }
        if t == Touch::Width {
            self.edges()?;
            let w = Value::Hz(self.s.width().clamp(1000.0, 20_000.0));
            self.set(self.t.am_width, w)?;
            self.run(self.restarts.demod)?;
            self.run(self.restarts.squelch_noise)?;
        }
        if all || t == Touch::Offset {
            let off = self.s.channel_offset;
            self.set(self.t.decimate_offset, Value::Hz(off))?;
            self.set(self.t.report_offset, Value::Hz(off))?;
            if !all {
                self.run(self.restarts.noise)?;
                self.run(self.restarts.squelch)?;
                self.run(self.restarts.report)?;
            }
        }
        Ok(())
    }
    fn set_input(&mut self, input: &InputInfo) -> Result<(), SettingError> {
        self.set(self.t.report_dial, Value::Hz(input.tuned_hz))?;
        self.set(self.t.report_rate, Value::Hz(input.rate_hz))
    }
    /// Tells each component to forget the results of the frequency before, except its fields and tables among `kept`
    /// (the keys of the stack: a field key, or `table` and a space before a table key).
    fn forget(&mut self, kept: &[String]) {
        let g = self.graph.graph_mut();
        let ids: Vec<String> = g.order().into_iter().map(str::to_owned).collect();
        for id in &ids {
            let own = g.component_results(id).map_or_else(Kept::default, |r| {
                let d = r.decl();
                let fields: Vec<&str> = d
                    .fields()
                    .fields()
                    .iter()
                    .map(|f| f.key.as_ref())
                    .filter(|k| kept.iter().any(|x| x == k))
                    .collect();
                let tables: Vec<&str> = d
                    .tables()
                    .iter()
                    .map(|t| t.key.as_ref())
                    .filter(|k| kept.iter().any(|x| x.strip_prefix("table ") == Some(k)))
                    .collect();
                Kept::of(&fields, &tables)
            });
            if let Some(c) = g.component_mut(id) {
                c.forget(&own);
            }
        }
        self.graph.refresh_results();
    }
}

/// The id of the argument `row` among the arguments of [`PRESET_APPLY`].
const PRESET_ROW_ID: SettingId = SettingId(0);

sdrkit_core::commands! {
    /// The commands of the general receiver.
    pub enum GeneralCommand {
        /// Applies a built-in preset.
        PresetApply = PRESET_APPLY.command,
        /// Push-to-talk.
        Ptt = PTT.command,
        /// Morse code.
        Send = SEND.command,
        /// The tune carrier.
        Tune = TUNE,
        /// Ends every transmission.
        Halt = HALT,
    }
}

/// The general receiver.
pub struct GeneralReceiver {
    s: GeneralSettings,
    values: Values,
    schema: Arc<Schema>,
    /// The keys of the results that survive a change of frequency, as the stack shows them.
    kept: Vec<String>,
    main: Chan,
    catalog: Arc<Catalog>,
    extras: [Option<Chan>; 3],
    input: Option<InputInfo>,
    station: Station,
    /// The facilities of the host, for the channels that are made later.
    facilities: Option<sdrkit_core::Facilities>,
    scratch: Output,
    requests: Vec<HostRequest>,
}

impl GeneralReceiver {
    /// A receiver with the settings of `cfg`. The `am` submode opens in AM, with the shortwave filter unless a width is given.
    /// The microphone input of its transmitter takes the audio of the host.
    ///
    /// # Errors
    ///
    /// A [`ConfigError`] if a setting of `cfg` is refused or the graph does not build.
    pub fn new(cfg: &StackConfig) -> Result<Self, ConfigError> {
        Self::with_mic(cfg, Mic::None)
    }
    /// As [`GeneralReceiver::new`], with `mic` at the microphone input of the transmitter (an audio source in the graph).
    ///
    /// # Errors
    ///
    /// As [`GeneralReceiver::new`].
    pub fn with_mic(cfg: &StackConfig, mic: Mic) -> Result<Self, ConfigError> {
        let mut s = GeneralSettings::from_values(&cfg.settings)?;
        if cfg.submode.as_deref() == Some("am") {
            s.apply(
                GeneralSetting::Demod.id(),
                &Value::Choice(Demod::Am.index()),
            )?;
            let width_given = cfg
                .settings
                .schema()
                .id(WIDTH.field.key)
                .ok()
                .and_then(|id| cfg.settings.value(id))
                .is_some_and(|v| *v != Value::Hz(0.0));
            if !width_given {
                s.apply(
                    GeneralSetting::Width.id(),
                    &Value::Hz(crate::bands::SW_FILTER_HZ),
                )?;
            }
        }
        let catalog = Catalog::of(cfg)?;
        let own = Published::new(&[]).with_tables(&[CHANNELS.def()]);
        let main = Chan::new(s.clone(), &catalog, Some(own), mic)?;
        let schema = GeneralSetting::schema();
        let kept = typed::shape()
            .kept_keys()
            .map_err(|key| GraphError::Shape {
                stack: typed::ID,
                what: format!("the kept item {key} names nothing of the description"),
            })?;
        if let Some(what) = typed::shape().difference(
            &schema,
            Owner::results_decl(&main.graph),
            GeneralCommand::commands(),
        ) {
            return Err(GraphError::Shape {
                stack: typed::ID,
                what,
            }
            .into());
        }
        let mut r = GeneralReceiver {
            values: s.values(),
            schema,
            kept,
            main,
            catalog,
            s,
            extras: [None, None, None],
            input: None,
            station: Station::default(),
            facilities: None,
            scratch: Output::default(),
            requests: Vec::new(),
        };
        r.sync_extras()?;
        Ok(r)
    }
    /// The settings in force.
    pub fn settings(&self) -> &GeneralSettings {
        &self.s
    }
    fn extra_settings(&self, i: usize) -> GeneralSettings {
        let x = self.s.extra_channels[i];
        let mut e = self.s.clone();
        e.demod = x.demod;
        e.channel_offset = x.offset_hz;
        if x.width_hz > 0.0 {
            let slot = x.demod.slot();
            e.mem.width[slot] = x.demod.clamp_width(x.width_hz, x.demod.default_width());
        }
        e
    }
    fn sync_extras(&mut self) -> Result<(), ConfigError> {
        for i in 0..3 {
            match (self.s.extra_channels[i].on, self.extras[i].is_some()) {
                (true, false) => {
                    let mut c = Chan::new(self.extra_settings(i), &self.catalog, None, Mic::Off)?;
                    c.graph.set_station(&self.station);
                    if let Some(f) = &self.facilities {
                        Receiver::set_facilities(&mut c.graph, f);
                    }
                    if let Some(inp) = self.input {
                        c.set_input(&inp)?;
                        Receiver::configure(&mut c.graph, &inp)?;
                    }
                    self.extras[i] = Some(c);
                    self.summarise_extras();
                }
                (false, true) => {
                    self.extras[i] = None;
                    if let Some(own) = self.main.graph.graph_mut().own_results_mut() {
                        own.remove_row(&CHANNELS, channel_key(i));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    /// Writes a row of the extra channels table for each extra channel that is on.
    fn summarise_extras(&mut self) {
        let Some(own) = self.main.graph.graph_mut().own_results_mut() else {
            return;
        };
        for (i, e) in self.extras.iter().enumerate() {
            let Some(c) = e else { continue };
            let r = Owner::results(&c.graph);
            let f = r.fields();
            CHANNELS.put(
                own,
                channel_key(i),
                &ChannelRow {
                    channel: CHANNEL_NAMES[i],
                    freq_hz: f.at(&GeneralFields::LISTENING_HZ).map(Hertz),
                    demod: f.at(&GeneralFields::DEMOD),
                    squelch_open: f.at(&GeneralFields::SQUELCH_OPEN),
                    summary: r.status().summary.as_str(),
                },
            );
        }
    }
    /// Runs `step` (a feed or the finish) on the main channel and on each extra channel. Only the channel that the user listens
    /// to writes its audio to `out`. The events of every channel go to `out`, with the frequency of their channel.
    fn each_channel(
        &mut self,
        out: &mut Output,
        mut step: impl FnMut(&mut StackGraph, &mut Output),
    ) {
        let listen = self.s.listen_channel;
        let dial = self.input.map_or(0.0, |i| i.tuned_hz);
        let first = out.events.len();
        if listen == 0 || self.extras[listen - 1].is_none() {
            step(&mut self.main.graph, out);
        } else {
            self.scratch.clear();
            step(&mut self.main.graph, &mut self.scratch);
            out.events.append(&mut self.scratch.events);
        }
        name_frequency(&mut out.events[first..], dial + self.s.channel_offset);
        let keyed = self.main.keyed();
        for (i, c) in self.extras.iter_mut().enumerate() {
            if let Some(c) = c {
                let first = out.events.len();
                if listen == i + 1 {
                    let from = out.audio().len();
                    step(&mut c.graph, out);
                    if keyed {
                        // The transmitter of the main channel mutes the channel heard.
                        out.audio_mut()[from..].fill(0.0);
                    }
                } else {
                    self.scratch.clear();
                    step(&mut c.graph, &mut self.scratch);
                    out.events.append(&mut self.scratch.events);
                }
                name_frequency(&mut out.events[first..], dial + c.s.channel_offset);
            }
        }
        if self.extras.iter().any(Option::is_some) {
            self.summarise_extras();
        }
    }
    /// Runs the transmit command that `pick` chooses on the transmitter of the main channel.
    fn transmit(
        &mut self,
        pick: impl Fn(&TxTargets) -> CommandId,
        args: &Values,
    ) -> Result<Option<Record>, CommandError> {
        let id = self
            .main
            .tx
            .as_ref()
            .map(pick)
            .ok_or(CommandError::NoSuchId {
                id: CommandId(u16::MAX),
            })?;
        self.main.transmit(id, args)
    }
    /// Applies a setting of the stack and pushes what it touches into the channels.
    fn apply_setting(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        use GeneralSetting as S;
        let which = S::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        let mut s = self.s.clone();
        s.apply(id, value)?;
        if which == S::ChannelOffset {
            if let Some(inp) = self.input {
                // The channel stays inside 0.45 of the rate on either side.
                let (lo, hi) = s.demod.edges(s.width());
                let min = -0.45 * inp.rate_hz - inp.signal_offset_hz() - lo;
                let max = 0.45 * inp.rate_hz - inp.signal_offset_hz() - hi;
                if min <= max && !(min..=max).contains(&s.channel_offset) {
                    return Err(SettingError::OutOfRange {
                        id,
                        min,
                        max,
                        got: s.channel_offset,
                    });
                }
            }
        }
        self.s = s;
        self.values = self.s.values();
        let config = |e: ConfigError| SettingError::Config {
            id,
            source: Box::new(e),
        };
        match which {
            S::Extra1 | S::Extra2 | S::Extra3 | S::ListenChannel => {
                return self.sync_extras().map_err(config);
            }
            S::Extra1Offset | S::Extra1Demod | S::Extra1Width => self.extras[0] = None,
            S::Extra2Offset | S::Extra2Demod | S::Extra2Width => self.extras[1] = None,
            S::Extra3Offset | S::Extra3Demod | S::Extra3Width => self.extras[2] = None,
            _ => {}
        }
        if matches!(
            which,
            S::Extra1Offset
                | S::Extra1Demod
                | S::Extra1Width
                | S::Extra2Offset
                | S::Extra2Demod
                | S::Extra2Width
                | S::Extra3Offset
                | S::Extra3Demod
                | S::Extra3Width
        ) {
            // A new offset, demodulator or width makes that channel again.
            return self.sync_extras().map_err(config);
        }
        let Some(t) = touch_of(which) else {
            return Ok(());
        };
        self.main.s = self.s.clone();
        self.main.push(t).map_err(config)?;
        if t == Touch::Offset {
            if let Some(inp) = self.input {
                self.main.set_input(&inp)?;
            }
        }
        if !matches!(t, Touch::Demod | Touch::Width | Touch::Offset) {
            for i in 0..3 {
                if self.extras[i].is_some() {
                    let es = self.extra_settings(i);
                    if let Some(c) = self.extras[i].as_mut() {
                        c.s = es;
                        c.push(t).map_err(config)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// The row keys of the extra channels in the `channels` table.
const CHANNEL_KEYS: [&str; 3] = ["1", "2", "3"];
/// The names of the extra channels.
const CHANNEL_NAMES: [&str; 3] = ["Extra 1", "Extra 2", "Extra 3"];

fn channel_key(i: usize) -> &'static str {
    CHANNEL_KEYS[i]
}

/// Returns the events of one channel that name no frequency the listening frequency of the channel.
fn name_frequency(events: &mut [sdrkit_core::Event], listening_hz: f64) {
    for e in events.iter_mut().filter(|e| e.freq_hz == 0.0) {
        e.freq_hz = listening_hz;
    }
}

impl Owner for GeneralReceiver {
    fn schema(&self) -> &Arc<Schema> {
        &self.schema
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.apply_setting(id, value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        Owner::results_decl(&self.main.graph)
    }
    fn results(&self) -> &Results {
        Owner::results(&self.main.graph)
    }
    fn events(&self) -> &[EventDecl] {
        Owner::events(&self.main.graph)
    }
    fn commands(&self) -> &Commands {
        GeneralCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match GeneralCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            GeneralCommand::PresetApply => {
                let PresetApplyArgs { row: key } = PRESET_APPLY.args(args)?;
                let Some(p) = preset(&self.s.preset_list, key).cloned() else {
                    let table = Owner::results_decl(&self.main.graph).table_id(PRESETS.key);
                    return Err(match table {
                        Some(table) => CommandError::NoSuchRow {
                            table,
                            key: key.into(),
                        },
                        None => CommandError::NoSuchKey { arg: PRESET_ROW_ID },
                    });
                };
                let failed = |e: SettingError| CommandError::Failed {
                    source: StageError::new(e),
                };
                self.apply_setting(GeneralSetting::Demod.id(), &Value::Choice(p.demod.index()))
                    .map_err(failed)?;
                self.apply_setting(GeneralSetting::Width.id(), &Value::Hz(p.width_hz))
                    .map_err(failed)?;
                self.apply_setting(
                    GeneralSetting::Squelch.id(),
                    &Value::Choice(p.squelch.index()),
                )
                .map_err(failed)?;
                self.requests.push(HostRequest::retune(p.freq_hz, &p.name));
            }
            GeneralCommand::Ptt => return self.transmit(|t| t.ptt, args),
            GeneralCommand::Send => return self.transmit(|t| t.send, args),
            GeneralCommand::Tune => return self.transmit(|t| t.tune, args),
            GeneralCommand::Halt => return self.transmit(|t| t.halt, args),
        }
        Ok(None)
    }
}

impl Receiver for GeneralReceiver {
    fn configure(&mut self, input: &InputInfo) -> Result<(), ConfigError> {
        self.input = Some(*input);
        self.main.set_input(input)?;
        Receiver::configure(&mut self.main.graph, input)?;
        for c in self.extras.iter_mut().flatten() {
            c.set_input(input)?;
            Receiver::configure(&mut c.graph, input)?;
        }
        Ok(())
    }
    fn feed(&mut self, iq: &[Complex32], out: &mut Output) {
        out.requests.append(&mut self.requests);
        self.each_channel(out, |g, o| g.feed(iq, o));
    }
    fn finish(&mut self, out: &mut Output) {
        out.requests.append(&mut self.requests);
        self.each_channel(out, |g, o| g.finish(o));
    }
    fn retuned(&mut self, input: &InputInfo) {
        self.input = Some(*input);
        // The report takes only values in its range, and the input of a running receiver is one of them.
        let _ = self.main.set_input(input);
        self.main.graph.retuned(input);
        for c in self.extras.iter_mut().flatten() {
            let _ = c.set_input(input);
            c.graph.retuned(input);
        }
    }
    fn set_station(&mut self, station: &Station) {
        self.station = station.clone();
        self.main.graph.set_station(station);
        for c in self.extras.iter_mut().flatten() {
            c.graph.set_station(station);
        }
    }
    fn forget(&mut self) {
        self.main.forget(&self.kept);
        for c in self.extras.iter_mut().flatten() {
            c.forget(&self.kept);
        }
        if self.extras.iter().any(Option::is_some) {
            self.summarise_extras();
        }
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.main.graph.save_state()
    }
    fn transmits(&self) -> bool {
        Receiver::transmits(&self.main.graph)
    }
    fn set_transmit(&mut self, on: bool) {
        Receiver::set_transmit(&mut self.main.graph, on);
    }
    /// Gives the files, the clock, the random source and the codecs of the host to the graph of every channel. The
    /// transmit side states the start of a transmission by this clock.
    fn set_facilities(&mut self, facilities: &sdrkit_core::Facilities) {
        Receiver::set_facilities(&mut self.main.graph, facilities);
        for c in self.extras.iter_mut().flatten() {
            Receiver::set_facilities(&mut c.graph, facilities);
        }
        self.facilities = Some(facilities.clone());
    }
    /// The graph of the main channel, which holds the transmit sink.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Receiver::as_any_mut(&mut self.main.graph)
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.main.graph.restore_state(state)
    }
}
