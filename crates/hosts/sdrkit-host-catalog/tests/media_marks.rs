//! The sound and the video that a stack states in its interface description are what its graph gives to the host.

use sdrkit_core::{StackConfig, Values};
use sdrkit_core::media::Picture;
use sdrkit_graph::media::MediaPacket;
use sdrkit_graph::stream::ItemType;
use sdrkit_graph::{Graph, Kind};

/// The stream kinds that the graph of stack `d` gives to the host, with the default settings and with each variant.
/// `None` for a stack whose receive chain is not a graph, or that does not start.
fn host_kinds(d: &sdrkit_core::StackDescriptor) -> Option<Vec<Kind>> {
    let cfg = StackConfig::new(Values::defaults(&(d.settings)()));
    let mut all: Option<Vec<Kind>> = None;
    let configs = std::iter::once(cfg.clone())
        .chain(d.submodes.iter().map(|m| cfg.clone().with_submode(m.id)));
    for c in configs {
        let Ok(mut rx) = (d.make_receiver)(&c) else {
            continue;
        };
        let Some(g) = rx.as_any_mut().and_then(|a| a.downcast_mut::<Graph>()) else {
            continue;
        };
        let list = all.get_or_insert_with(Vec::new);
        for k in g.host_kinds() {
            if !list.contains(&k) {
                list.push(k);
            }
        }
    }
    all
}

#[test]
fn a_stack_states_the_media_that_its_graph_gives() {
    let mut problems = Vec::new();
    let mut without_graph = Vec::new();
    for d in sdrkit_host_catalog::all() {
        let Some(family) = sdrkit_host_catalog::family_of(d.id) else {
            continue;
        };
        let Some(shape) = family.stack_shape(d.id) else {
            continue;
        };
        let media = shape.media();
        let Some(kinds) = host_kinds(d) else {
            // The media of a stack whose receive chain is not a graph are not compared.
            without_graph.push(d.id);
            continue;
        };
        let item = |is: fn(&ItemType) -> bool| kinds.iter().any(|k| matches!(k, Kind::Item(t) if is(t)));
        // Sound reaches the host as samples, or compressed in media packets that the host decodes.
        let samples = kinds.contains(&Kind::Audio);
        let packets = item(|t| t.is::<MediaPacket>());
        let pictures = item(|t| t.is::<Picture>());
        let bytes = kinds.contains(&Kind::Bytes);
        let mut problem = |what: &str| problems.push(format!("{}: {what}", d.id));
        if samples && media.audio.is_none() {
            problem("the graph gives audio to the host, and the description states no sound");
        }
        if media.audio.is_some() && !samples && !packets && !bytes {
            problem("the description states sound, and the graph gives the host neither audio nor media");
        }
        if packets && media.audio.is_none() && !media.video {
            problem("the graph gives media packets to the host, and the description states no sound and no video");
        }
        if pictures && !media.pictures && !media.video {
            problem("the graph gives pictures to the host, and the description states no pictures and no video");
        }
        if media.video && !packets && !pictures && !bytes {
            problem("the description states video, and the graph gives the host neither media nor pictures");
        }
    }
    println!("not compared, no graph: {}", without_graph.join(" "));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
