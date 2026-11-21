//! The tone decoders (`tones`): CTCSS, DCS, DTMF, sequential calls, SELCAL and two-tone pages from `sdrkit-proto-tones`, the
//! strongest tone, an audio spectrum and a gate for the listening audio. Receive only.

pub mod events;
pub mod front;
pub mod gen;
pub mod listen;
pub mod spectrum;

use front::{Demod, AUDIO_INPUT, DEMOD};
use listen::{GATE, GATE_CODE, GATE_INVERTED, GATE_KIND, GATE_TONE, LISTEN};
use sdrkit_core::{ResultsDecl, StackConfig, Value};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, StackGraph};
use sdrkit_proto_tones::components::{
    DtmfComponent, SelcalComponent, SelcallComponent, SubAudioComponent, TwoToneComponent,
};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "tones";

sdrkit_core::settings! {
    /// The settings of the tone stack.
    pub enum ToneSetting {
        /// The demodulation.
        Demod = DEMOD,
        /// The input is audio.
        AudioInput = AUDIO_INPUT,
        /// When the gate opens.
        Gate = GATE,
        /// What it opens on.
        GateKind = GATE_KIND,
        /// The chosen tone.
        GateTone = GATE_TONE,
        /// The chosen code.
        GateCode = GATE_CODE,
        /// The chosen code is inverted.
        GateInverted = GATE_INVERTED,
        /// Give the listening audio.
        Listen = LISTEN,
    }
}

/// The receiver graph. With `ssb_search` the SELCAL decoder also tries tones moved by a sideband receiver that is off tune.
/// The decoders' outputs leave the graph as their events.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(ssb_search: bool, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", front::ToneFront::default())
        .add("sub", SubAudioComponent::default())
        .add("dtmf", DtmfComponent::default())
        .add("selcall", SelcallComponent::default())
        .add("selcal", SelcalComponent::default())
        .add("two_tone", TwoToneComponent::default())
        .add("spectrum", spectrum::SpectrumComponent::default())
        .add("gate", listen::GateComponent::default())
        .add("events", events::EventsComponent::default())
        .set("selcal.ssb_search", Value::Bool(ssb_search))
        .input("front")
        .connect("front.out", "sub")
        .connect("front.out", "dtmf")
        .connect("front.out", "selcall")
        .connect("front.out", "selcal")
        .connect("front.out", "two_tone")
        .connect("front.out", "spectrum")
        .connect("front.out", "gate.in")
        .connect("sub", "gate.sub")
        .connect("front.report", "events.front")
        .connect("spectrum", "events.spectrum")
        .connect("gate.state", "events.gate")
        .connect("sub", "events.sub")
        .connect("dtmf", "events.dtmf")
        .connect("selcall", "events.selcall")
        .connect("selcal", "events.selcal")
        .connect("two_tone", "events.two_tone")
        .output("gate.out")
        .output("sub")
        .output("dtmf")
        .output("selcall")
        .output("selcal")
        .output("two_tone")
        .clock(0.25)
        .status_from("events")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    events::declaration()
}

/// The receiver: 500 ksps at least from a radio, 8 kHz and up as audio.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let audio = cfg.settings.get(&AUDIO_INPUT);
    let demod = cfg.settings.get(&DEMOD);
    let sideband = !audio && matches!(demod, Demod::Auto | Demod::Usb | Demod::Lsb);
    let (min, why) = if audio {
        (8000.0, "the tone decoders need audio at 8 kHz or more")
    } else {
        (
            500e3,
            "the tone decoders need at least 500 ksps from a radio",
        )
    };
    let mut b = StackGraph::of(
        crate::typed::shape(ID),
        graph(sideband, &Catalog::of(cfg)?)?,
    )
    .min_rate(min, why)
    .route(DEMOD.key(), &["front.demod"])?
    .route(AUDIO_INPUT.key(), &["front.audio_input"])?
    .command("clear", sdrkit_graph::CommandTarget::of("events", "clear"));
    for key in [
        GATE.key(),
        GATE_KIND.key(),
        GATE_TONE.key,
        GATE_CODE.key,
        GATE_INVERTED.key(),
        LISTEN.key(),
    ] {
        b = b.route(key, &[&format!("gate.{key}")])?;
    }
    b.with_settings(&cfg.settings).build()
}
