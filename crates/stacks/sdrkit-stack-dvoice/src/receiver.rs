//! The receiver of a digital voice stack: the decoder, fed the samples of the radio, and the voice monitor in one graph,
//! wrapped in a stack graph that refuses a rate below the minimum of the stack and routes the settings of the host. A stack
//! that sends adds its transmit branch: the host's audio input (`mic`), the sender (`transmit`), the transmit sink (`tx`),
//! and the mute (`mute`) that silences the audio of the monitor while the sender is keyed.

use crate::decoder::{self, Protocol, VoiceDecoder};
use crate::front::ChainSpec;
use crate::monitor::VoiceMonitor;
use crate::monitor::{self, MonitorSetting};
use sdrkit_components::tx::TxMute;
use sdrkit_core::{DeclareSetting, SettingSet, StackConfig};
use sdrkit_graph::components::{HostIn, TxSink};
use sdrkit_graph::{Catalog, Component, DecoderComponent, GraphBuilder, GraphError, StackGraph};

/// The transmit branch of a stack: the sender, the settings and the commands that the stack routes to it.
pub struct Branch {
    /// The sender: input `in` (the session's samples), input `mic` (audio), output `out` (IQ for `tx.sink`), output
    /// `keyed` (the keyed state, [`sdrkit_components::tx::TxKeyed`]).
    pub sender: Box<dyn Component>,
    /// The settings of the sender.
    pub settings: &'static [&'static dyn DeclareSetting],
    /// The keys of the commands of the sender.
    pub commands: &'static [&'static str],
}

/// The receiver of `proto` behind the chain `spec`, refusing rates below `min_rate_hz`, with the transmit branch `tx` if
/// the stack sends. The voice monitor takes the codec plug-ins from the run context.
///
/// # Errors
///
/// A [`GraphError`] if the graph does not build or a setting of `cfg` is refused.
pub fn build<P: Protocol>(
    proto: P,
    spec: ChainSpec,
    min_rate_hz: f64,
    cfg: &StackConfig,
    tx: Option<Branch>,
) -> Result<StackGraph, GraphError> {
    let params = proto.params();
    let own = proto.own_settings();
    let mut g = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("dec", DecoderComponent::new(VoiceDecoder::new(proto, spec)))
        .add(monitor::ID, VoiceMonitor::new())
        .input("dec")
        .connect("dec.speech", "monitor.speech")
        .connect("dec.call", "monitor.call")
        .clock(0.25)
        .status_from("dec");
    let mut routes: Option<(&[&dyn DeclareSetting], &[&str])> = None;
    let mut audio = "monitor.audio";
    if let Some(b) = tx {
        routes = Some((b.settings, b.commands));
        g = g
            .add("mic", HostIn::default())
            .add_boxed("transmit", b.sender)
            .add("tx", TxSink::default())
            .add("mute", TxMute::default())
            .input("mic.in")
            .input("transmit.in")
            .connect("mic.out", "transmit.mic")
            .connect("transmit.out", "tx.in")
            .connect("monitor.audio", "mute.in")
            .connect("transmit.keyed", "mute.keyed");
        audio = "mute.out";
    }
    let graph = g
        .output(audio)
        .output("dec.call")
        .output("dec.message")
        .output("dec.position")
        .build()?;
    let refusal = format!(
        "{} needs a sample rate of at least {} ksps",
        params.name,
        (min_rate_hz / 1000.0).round()
    );
    let shape = crate::typed::family()
        .stack_shape(params.mode)
        .ok_or_else(|| GraphError::Shape {
            stack: params.mode,
            what: "the family does not describe it".into(),
        })?;
    let mut b = StackGraph::of(shape, graph)
        .min_rate(min_rate_hz, &refusal)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command(
            "hold_current",
            sdrkit_graph::CommandTarget::of("monitor", "hold_current"),
        )
        .command(
            "release",
            sdrkit_graph::CommandTarget::of("monitor", "release"),
        )
        .command("remove", sdrkit_graph::CommandTarget::of("dec", "remove"));
    for s in MonitorSetting::schema().settings() {
        let key = s.field.key.as_ref();
        let to_monitor = format!("{}.{key}", monitor::ID);
        b = if key == decoder::LISTEN.key() || key == decoder::HANG_S.key() {
            let to_decoder = format!("dec.{key}");
            b.route(key, &[&to_decoder, &to_monitor])?
        } else {
            b.route(key, &[&to_monitor])?
        };
    }
    for s in own {
        let key = s.key();
        b = b.route(key, &[&format!("dec.{key}")])?;
    }
    if let Some((settings, commands)) = routes {
        for s in settings {
            let key = s.key();
            b = b.route(key, &[&format!("transmit.{key}")])?;
        }
        for c in commands {
            b = b.command(c, sdrkit_graph::CommandTarget::of("transmit", c));
        }
    }
    b.with_settings(&cfg.settings).build()
}
