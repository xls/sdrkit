//! The transport stream layer of a TV receiver. [`attach`] adds the demultiplexer and what follows it after the transport
//! stream output of a stack's own decoder, so every stack has the same components, outputs, tables, events and commands after
//! its transport stream.

use crate::components::{
    DvbSubtitleFields, PsiSi, PsiSiFields, SubtitleComponent, TeletextComponent,
    TeletextStreamFields, TsDemuxComponent, TsDemuxFields, TsMedia, PIDS, SECTIONS,
};
use crate::publish::{
    dvb_subtitle, epg_event, service_list, teletext_page, ts_network, ts_time, SiTables,
    TeletextTables, EPG_EVENT, GUIDE, GUIDE_RATINGS, SERVICES, SERVICE_LIST_EVENT,
    SERVICE_SELECT_KIND, SUBTITLE_EVENT, TELETEXT_PAGE_EVENT, TELETEXT_PAGE_TABLE, TELETEXT_SELECT,
    TELETEXT_SUBPAGE_TABLE, TRACKS, TRACK_SELECT_KIND, TS_NETWORK_EVENT, TS_TIME_EVENT,
};
use sdrkit_core::interface::StackShape;
use sdrkit_core::publish::field;
use sdrkit_core::{ResultsDecl, Setting};
use sdrkit_graph::GraphBuilder;

/// The id of the demultiplexer in the graph.
pub const DEMUX: &str = "ts_demux";
/// The id of the service information component.
pub const SI: &str = "psi_si";
/// The id of the teletext component.
pub const TELETEXT: &str = "teletext";
/// The id of the DVB subtitle component.
pub const SUBTITLE: &str = "dvb_subtitle";
/// The id of the component whose output `ts_media.out` returns the whole transport stream as media packets of
/// [`Codec::MpegTs`](sdrkit_core::media::Codec::MpegTs).
pub const TS_MEDIA: &str = "ts_media";
/// The port of the service that plays, as a transport stream, read inside the receiver with `Graph::buffer` or a tap.
pub const SERVICE_TS: &str = "ts_demux.service_ts";

/// The stack setting that decodes MPEG-2 video in the demultiplexer. A stack lists it in its settings and routes it
/// with [`DECODE_VIDEO_ROUTE`].
pub const DECODE_VIDEO: Setting<field::Bool> = Setting::new("decode_video", "Decode video", "", false).help(
    "MPEG-2 video leaves as frames of raw pixels (I420) in display order, for a host that has no decoder of \
     MPEG-2 video. Off: the coded pictures. H.264 and HEVC always leave as coded pictures.",
);
/// The route of [`DECODE_VIDEO`] to the demultiplexer.
pub const DECODE_VIDEO_ROUTE: (&str, &[&str]) = ("decode_video", &["ts_demux.decode_video"]);

/// Which optional parts [`attach`] adds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TsLayer {
    /// Teletext pages.
    pub teletext: bool,
    /// DVB subtitles.
    pub subtitles: bool,
}

impl TsLayer {
    /// Video, audio, the services and the guide only.
    pub fn basic() -> TsLayer {
        TsLayer::default()
    }

    /// Every part.
    pub fn all() -> TsLayer {
        TsLayer {
            teletext: true,
            subtitles: true,
        }
    }
}

/// Adds the transport stream layer after `from`, an output that carries a transport stream (for example `"fec.out"`). The
/// layer adds the command aliases `service.select`, `track.select` and, with teletext, `teletext.select`. A graph with
/// command aliases shows only them, so a stack that adds the layer adds its own commands as aliases too.
pub fn attach(b: GraphBuilder, from: &str, parts: &TsLayer) -> GraphBuilder {
    let alias = |c: &sdrkit_core::command::Command, comp: &str| {
        (c.key, sdrkit_graph::CommandTarget::of(comp, c.key))
    };
    let mut b = b
        .add(DEMUX, TsDemuxComponent::default())
        .add(SI, PsiSi::default())
        .add(TS_MEDIA, TsMedia::default())
        .connect(from, "ts_demux.in")
        .connect(from, "ts_media.in")
        .connect("ts_demux.sections", "psi_si.sections")
        .connect("ts_demux.pids", "psi_si.pids")
        .output("ts_demux.video")
        .output("ts_demux.audio")
        .output("psi_si.services")
        .output("psi_si.events")
        .output("ts_media.out");
    for (name, target) in [
        alias(&SERVICE_SELECT_KIND.command, DEMUX),
        alias(&TRACK_SELECT_KIND.command, DEMUX),
    ] {
        b = b.command_alias(name, target);
    }
    if parts.teletext {
        let (name, target) = alias(&TELETEXT_SELECT.command, TELETEXT);
        b = b
            .add(TELETEXT, TeletextComponent::default())
            .connect("ts_demux.teletext", "teletext.in")
            .output("teletext.pages")
            .command_alias(name, target);
    }
    if parts.subtitles {
        b = b
            .add(SUBTITLE, SubtitleComponent::default())
            .connect("ts_demux.subtitle", "dvb_subtitle.in")
            .output("dvb_subtitle.displays");
    }
    b
}

/// `decl` with the tables and the event kinds of the layer that `parts` describe, for a stack's results declaration: the
/// tables keep their own names in the stack's results.
pub fn declare(decl: ResultsDecl, parts: &TsLayer) -> ResultsDecl {
    let mut d = decl;
    for t in SiTables::SERVICE_TABLES
        .into_iter()
        .chain(SiTables::GUIDE_TABLES)
    {
        d = d.with_table(t);
    }
    for e in [
        &SERVICE_LIST_EVENT,
        &EPG_EVENT,
        &TS_TIME_EVENT,
        &TS_NETWORK_EVENT,
    ] {
        d = d.event(e);
    }
    if parts.teletext {
        for t in TeletextTables::TABLES {
            d = d.with_table(t);
        }
        d = d.event(&TELETEXT_PAGE_EVENT);
    }
    if parts.subtitles {
        d = d.event(&SUBTITLE_EVENT);
    }
    d
}

/// `shape` with what the layer of [`attach`] adds to the results of a stack, for the description of the stack: the tables
/// `services`, `tracks`, `guide` and `guide_ratings`, the result sets and the tables of the components, and the event
/// types, with the teletext and subtitle items that `parts` names. The declarations are those of [`declare`].
///
/// List the result sets and the tables of the stack first, and the components that run before the layer. A field of a
/// component whose key a result set of the stack declares is the field of the stack
/// ([`ComponentShape::shares_declared`](sdrkit_core::interface::ComponentShape::shares_declared)); every other field
/// has the key `<component>.<key>`. The family names [`interface`](crate::publish::interface) with `Family::uses`.
///
/// The commands of the layer ([`SERVICE_SELECT_KIND`](crate::publish::SERVICE_SELECT_KIND),
/// [`TRACK_SELECT_KIND`](crate::publish::TRACK_SELECT_KIND), [`TELETEXT_SELECT`]) are not added: a stack lists the ones
/// that its receiver gives a target.
#[must_use]
pub fn describe(shape: StackShape, parts: &TsLayer) -> StackShape {
    let mut s = shape
        .table(&SERVICES)
        .table(&TRACKS)
        .table(&GUIDE)
        .table(&GUIDE_RATINGS);
    if parts.teletext {
        s = s.table(&TELETEXT_PAGE_TABLE).table(&TELETEXT_SUBPAGE_TABLE);
    }
    s = s
        .component(DEMUX, |c| {
            c.fields::<TsDemuxFields>()
                .shares_declared()
                .writes_table(&SERVICES)
                .writes_table(&TRACKS)
                .table(&PIDS)
        })
        .component(SI, |c| {
            c.fields::<PsiSiFields<'static>>()
                .shares_declared()
                .writes_table(&GUIDE)
                .writes_table(&GUIDE_RATINGS)
                .table(&SECTIONS)
        })
        .event(service_list())
        .event(epg_event())
        .event(ts_time())
        .event(ts_network());
    if parts.teletext {
        s = s
            .component(TELETEXT, |c| {
                c.fields::<TeletextStreamFields<'static>>()
                    .shares_declared()
                    .writes_table(&TELETEXT_PAGE_TABLE)
                    .writes_table(&TELETEXT_SUBPAGE_TABLE)
            })
            .event(teletext_page());
    }
    if parts.subtitles {
        s = s
            .component(SUBTITLE, |c| {
                c.fields::<DvbSubtitleFields>().shares_declared()
            })
            .event(dvb_subtitle());
    }
    s
}
