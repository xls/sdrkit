//! The `atv` graphs (AM broadcast, FM video and NBTV), the `atv` receiver over them, and the registry of their component types.

use super::a2::A2Decoder;
use super::colour::ColourLock;
use super::fmvideo::FmVideoDisc;
use super::front::{Mirror, SpikeRemoval};
use super::nbtv::{NbtvDecoder, NbtvFormat, NbtvRadio};
use super::nicam_rx::NicamDecoder;
use super::picture::PictureDecoder;
use super::search::CarrierSearch;
use super::selector::SoundSelector;
use super::snow::Snow;
use super::sound::SoundChannel;
use super::status::AtvStatus;
use super::supervisor::Supervisor;
use super::sync::VideoSync;
use super::teletext::TeletextDecoder;
use super::vision::VisionChannel;
use super::{Modulation, TICK_S};
use crate::settings::{
    nbtv_format, BLACK_SETUP, CHANNELS, CHANNEL_WIDTH, CHROMA_DELAY, COLOUR, COLOUR_ON,
    DEINTERLACE, DETECTOR, DUAL_SOUND, MODULATION, NBTV_DEVIATION, NBTV_LINES, NBTV_RATE,
    NBTV_VERTICAL, NOISE_REDUCTION, SATURATION, SNOW, SOUND_SYSTEM, SYSTEM, TINT,
};
use crate::transmit::{AtvTransmit, COMMANDS as TX_COMMANDS};
use sdrkit_core::{SettingSet, StackConfig, Values};
use sdrkit_graph::{Catalog, Graph, GraphBuilder, GraphError, Registry, StackGraph, Stage};
use std::sync::Arc;

/// The lowest rate of the AM broadcast receiver.
pub const MIN_RATE_HZ: f64 = 8e6;
/// The lowest rate of the FM video receiver: below it the colour filters of the picture decoder pass the Nyquist rate.
pub const MIN_RATE_FM_HZ: f64 = 8e6;
/// The stage of the picture decoder and of the components that read its counts.
pub const PICTURE_STAGE: &str = "picture";

/// Joins the colour lock (`colour`) and the picture decoder (`picture`) to the sync, with the picture decoder at the
/// start of the stage [`PICTURE_STAGE`]. The colour lock stays before the boundary: the supervisor reads its report and
/// feeds its control back to it. Nothing goes back from the picture decoder.
pub fn picture_chain(b: GraphBuilder) -> GraphBuilder {
    b.connect("sync.video", "colour.video")
        .connect("sync.base", "colour.base")
        .connect("sync.lines", "colour.lines")
        .connect("sync.report", "colour.sync")
        .connect("sync.video", "picture.video")
        .connect("sync.base", "picture.base")
        .connect("colour.jobs", "picture.jobs")
        .connect("colour.report", "picture.colour")
        .stage("sync", "picture", Stage::new(PICTURE_STAGE))
        .stage("colour", "picture", Stage::new(PICTURE_STAGE))
        .connect("colour.report", "supervisor.picture")
        .feedback("supervisor.tune", "colour.control")
}

/// What a graph is built with besides its settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Build {
    /// The modulation.
    pub modulation: Modulation,
    /// The NBTV format.
    pub nbtv: NbtvFormat,
    /// NBTV FM deviation, Hz.
    pub nbtv_deviation: f64,
}

impl Default for Build {
    fn default() -> Self {
        Build {
            modulation: Modulation::Am,
            nbtv: NbtvFormat::default(),
            nbtv_deviation: 5e3,
        }
    }
}

fn am() -> GraphBuilder {
    let mut b = GraphBuilder::new()
        .add("spike", SpikeRemoval::default())
        .add("search", CarrierSearch::default())
        .add("mirror", Mirror::default())
        .add("vision", VisionChannel::default())
        .add("sync", VideoSync::default())
        .add("colour", ColourLock::default())
        .add("picture", PictureDecoder::default())
        .add("teletext", TeletextDecoder::default())
        .add("snow", Snow::default())
        .add("sound", SoundChannel::default())
        .add("nicam", NicamDecoder::default())
        .add("a2", A2Decoder::default())
        .add("selector", SoundSelector::default())
        .add("supervisor", Supervisor::new(Modulation::Am))
        .add("status", AtvStatus::new(Modulation::Am))
        .input("spike.in")
        .connect("spike.out", "search.in")
        .input("search.raw")
        .connect("spike.out", "mirror.in")
        .connect("mirror.out", "vision.in")
        .connect("mirror.out", "sound.in")
        .connect("mirror.out", "nicam.in")
        .connect("mirror.out", "a2.in")
        .connect("vision.out", "sync.in")
        .connect("sync.video", "teletext.video")
        .connect("sync.lines", "teletext.lines")
        .input("snow.in")
        .connect("snow.out", "picture.snow")
        .connect("sound.out", "selector.carrier")
        .connect("nicam.out", "selector.nicam")
        .connect("a2.out", "selector.a2")
        .connect("sound.report", "selector.sound")
        .connect("nicam.report", "selector.nicam_report")
        .connect("a2.report", "selector.a2_report")
        .connect("search.found", "supervisor.found")
        .connect("search.monitor", "supervisor.monitor")
        .connect("vision.report", "supervisor.vision")
        .connect("sync.report", "supervisor.sync")
        .connect("sound.report", "supervisor.sound")
        .connect("nicam.report", "supervisor.nicam")
        .connect("a2.report", "supervisor.a2")
        .connect("selector.report", "supervisor.selector")
        .connect("supervisor.status", "status.in")
        .connect("picture.report", "status.picture");
    b = picture_chain(b);
    for c in [
        "spike", "search", "mirror", "vision", "sync", "teletext", "snow", "sound", "nicam", "a2",
        "selector",
    ] {
        b = b.feedback("supervisor.tune", &format!("{c}.control"));
    }
    b.output("supervisor.events")
        .output("supervisor.requests")
        .output("picture.out")
        .output("teletext.pages")
}

fn fm() -> GraphBuilder {
    let mut b = GraphBuilder::new()
        .add("spike", SpikeRemoval::default())
        .add("search", {
            let mut c = CarrierSearch::default();
            c.monitor_only = true;
            c.whole = true;
            c
        })
        .add("fmvideo", FmVideoDisc::default())
        .add("sync", {
            let mut c = VideoSync::default();
            c.fm = true;
            c
        })
        .add("colour", ColourLock::default())
        .add("picture", {
            let mut c = PictureDecoder::default();
            c.fm = true;
            c
        })
        .add("teletext", TeletextDecoder::default())
        .add("snow", {
            let mut c = Snow::default();
            c.fm = true;
            c
        })
        .add("supervisor", Supervisor::new(Modulation::FmVideo))
        .add("status", AtvStatus::new(Modulation::FmVideo))
        .input("spike.in")
        .connect("spike.out", "search.in")
        .input("search.raw")
        .connect("spike.out", "fmvideo.in")
        .connect("fmvideo.out", "sync.in")
        .input("snow.in")
        .connect("snow.out", "picture.snow")
        .connect("sync.video", "teletext.video")
        .connect("sync.lines", "teletext.lines")
        .input("supervisor.iq")
        .connect("search.monitor", "supervisor.monitor")
        .connect("sync.report", "supervisor.sync")
        .connect("supervisor.status", "status.in")
        .connect("picture.report", "status.picture");
    b = picture_chain(b);
    for c in ["spike", "search", "sync", "teletext", "snow"] {
        b = b.feedback("supervisor.tune", &format!("{c}.control"));
    }
    b.output("supervisor.events")
        .output("supervisor.requests")
        .output("picture.out")
        .output("teletext.pages")
}

fn nbtv(build: &Build) -> GraphBuilder {
    let fmod = build.modulation == Modulation::NbtvFm;
    GraphBuilder::new()
        .add("radio", {
            let mut c = NbtvRadio::default();
            c.fm = fmod;
            c.format = build.nbtv;
            c.deviation = build.nbtv_deviation;
            c
        })
        .add("nbtv", {
            let mut c = NbtvDecoder::default();
            c.format = build.nbtv;
            c
        })
        .add("supervisor", Supervisor::new(build.modulation))
        .add("status", AtvStatus::new(build.modulation))
        // NBTV has no teletext. The decoder gets no lines, so the stack has its teletext results, without values,
        // and its `teletext.select` command in every build.
        .add("teletext", TeletextDecoder::default())
        .input("radio.in")
        .connect("radio.out", "nbtv.in")
        .connect("radio.level", "nbtv.level")
        .connect("radio.out", "teletext.video")
        .input("supervisor.iq")
        .connect("nbtv.report", "supervisor.nbtv")
        .connect("supervisor.status", "status.in")
        .feedback("supervisor.tune", "nbtv.control")
        .feedback("supervisor.tune", "teletext.control")
        .output("supervisor.events")
        .output("supervisor.requests")
        .output("nbtv.out")
}

/// The graph builder for a build, with the transmit branch ([`crate::transmit::branch`]) in every build.
pub fn builder(build: &Build) -> GraphBuilder {
    let costs = sdrkit_core::CostTable::parse(crate::COSTS);
    let (b, audio) = match build.modulation {
        // The recorded cost is that of the broadcast build.
        Modulation::Am => (am().costs(&costs, "atv", "-"), Some("selector.out")),
        Modulation::FmVideo => (fm(), None),
        Modulation::NbtvAm | Modulation::NbtvFm => (nbtv(build), None),
    };
    crate::transmit::branch(b, AtvTransmit::default(), audio)
        .status_from("status")
        .clock(TICK_S)
        .reconfigure_on_retune()
}

/// The graph for a build.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(build: &Build, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    builder(build).catalog(catalog).build()
}

/// Returns the build that a set of `atv` settings values selects.
pub fn build_of(v: &Values) -> Build {
    Build {
        modulation: v.get(&MODULATION).first(),
        nbtv: nbtv_format(v),
        nbtv_deviation: v.get(&NBTV_DEVIATION) * 1e3,
    }
}

/// The `atv` receiver: the stack graph of the modulation that `cfg` chooses (AM for automatic), with its settings
/// applied. A change of modulation or NBTV format needs a new receiver (those settings say `restart`).
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or `cfg` holds a value that a component refuses.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    receiver_of(cfg, build_of(&cfg.settings).modulation)
}

/// As [`receiver`], with the graph of `modulation`.
///
/// # Errors
///
/// As [`receiver`].
pub fn receiver_of(cfg: &StackConfig, modulation: Modulation) -> Result<StackGraph, GraphError> {
    let build = Build {
        modulation,
        ..build_of(&cfg.settings)
    };
    let mut b = StackGraph::of(
        crate::typed::shape("atv"),
        graph(&build, &Catalog::of(cfg)?)?,
    )
    .command(
        "channel.tune",
        sdrkit_graph::CommandTarget::of("supervisor", "channel.tune"),
    );
    for s in [NBTV_LINES.key(), NBTV_RATE.key(), NBTV_DEVIATION.key()] {
        b = b.route(s, &[])?;
    }
    b = b
        .route(MODULATION.key(), &[])?
        .route(NBTV_VERTICAL.key(), &[])?
        .route(DETECTOR.key(), &["supervisor.detector"])?
        .route(CHANNEL_WIDTH.key(), &["supervisor.channel_width"])?
        .route(CHANNELS.key(), &["supervisor.channels", "status.channels"])?;
    for s in crate::settings::AtvTxSetting::schema().settings() {
        let k = &s.field.key;
        b = b.route(k, &[&format!("transmit.{k}")])?;
    }
    for c in TX_COMMANDS {
        b = b.command(c, sdrkit_graph::CommandTarget::of("transmit", c));
    }
    let picture = [
        SATURATION.key(),
        TINT.key(),
        DEINTERLACE.key(),
        NOISE_REDUCTION.key(),
        BLACK_SETUP.key(),
        CHROMA_DELAY.key(),
    ];
    b = match build.modulation {
        Modulation::Am => {
            b = b
                .min_rate(
                    MIN_RATE_HZ,
                    "Analog TV needs a sample rate of at least 8 Msps",
                )
                .route(SYSTEM.key(), &["supervisor.system", "vision.system"])?
                .route(COLOUR.key(), &["colour.colour", "supervisor.colour"])?
                .route(COLOUR_ON.key(), &["colour.colour_on"])?
                .route(
                    SOUND_SYSTEM.key(),
                    &["selector.sound_system", "supervisor.sound_system"],
                )?
                .route(
                    DUAL_SOUND.key(),
                    &["selector.dual_sound", "nicam.dual_sound", "a2.dual_sound"],
                )?
                .route(SNOW.key(), &["snow.snow"])?
                .command(
                    "teletext.select",
                    sdrkit_graph::CommandTarget::of("teletext", "teletext.select"),
                );
            for k in picture {
                b = b.route(k, &[&format!("picture.{k}")])?;
            }
            b
        }
        Modulation::FmVideo => {
            b = b
                .min_rate(
                    MIN_RATE_FM_HZ,
                    "FM video needs a sample rate of at least 8 Msps",
                )
                .route(SYSTEM.key(), &["supervisor.system"])?
                .route(COLOUR.key(), &["colour.colour", "supervisor.colour"])?
                .route(COLOUR_ON.key(), &["colour.colour_on"])?
                .route(SOUND_SYSTEM.key(), &["supervisor.sound_system"])?
                .route(DUAL_SOUND.key(), &[])?
                .route(SNOW.key(), &["snow.snow"])?
                .command(
                    "teletext.select",
                    sdrkit_graph::CommandTarget::of("teletext", "teletext.select"),
                );
            for k in picture {
                b = b.route(k, &[&format!("picture.{k}")])?;
            }
            b
        }
        Modulation::NbtvAm | Modulation::NbtvFm => {
            b = b
                .route(SYSTEM.key(), &["supervisor.system"])?
                .route(COLOUR.key(), &["supervisor.colour"])?
                .route(COLOUR_ON.key(), &[])?
                .route(SOUND_SYSTEM.key(), &["supervisor.sound_system"])?
                .route(DUAL_SOUND.key(), &[])?
                .route(SNOW.key(), &[])?
                .command(
                    "teletext.select",
                    sdrkit_graph::CommandTarget::of("teletext", "teletext.select"),
                );
            for k in picture {
                b = b.route(k, &[])?;
            }
            b
        }
    };
    b.with_settings(&cfg.settings).build()
}

/// The standard components and the `atv.*` ones, so an `atv` graph can be built from its description.
pub fn registry() -> Registry {
    let mut r = sdrkit_components::registry();
    register(&mut r);
    r
}

/// Adds the `atv.*` components to `r`.
pub fn register(r: &mut Registry) {
    r.add("atv.spike", || Box::new(SpikeRemoval::default()));
    r.add("atv.search", || Box::new(CarrierSearch::default()));
    r.add("atv.mirror", || Box::new(Mirror::default()));
    r.add("atv.vision", || Box::new(VisionChannel::default()));
    r.add("atv.sync", || Box::new(VideoSync::default()));
    r.add("atv.colour", || Box::new(ColourLock::default()));
    r.add("atv.picture", || Box::new(PictureDecoder::default()));
    r.add("atv.status", || Box::new(AtvStatus::new(Modulation::Am)));
    r.add("atv.snow", || Box::new(Snow::default()));
    r.add("atv.sound", || Box::new(SoundChannel::default()));
    r.add("atv.nicam", || Box::new(NicamDecoder::default()));
    r.add("atv.a2", || Box::new(A2Decoder::default()));
    r.add("atv.selector", || Box::new(SoundSelector::default()));
    r.add("atv.supervisor", || {
        Box::new(Supervisor::new(Modulation::Am))
    });
    r.add("atv.fm_video", || Box::new(FmVideoDisc::default()));
    r.add("atv.nbtv_radio", || Box::new(NbtvRadio::default()));
    r.add("atv.nbtv", || Box::new(NbtvDecoder::default()));
    r.add("atv.teletext", || Box::new(TeletextDecoder::default()));
}
