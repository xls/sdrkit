//! Helpers shared by the `bits_*` tests: a chain of registered components fed from a [`Source`], run in pieces, with its output
//! items and tags gathered.

#![allow(dead_code)]

#[path = "../set_util/mod.rs"]
mod set_util;
#[allow(unused_imports)]
pub use set_util::{gset, set, typed};

use sdrkit_components::testing::{extend_range, Source};
use sdrkit_core::{Complex32, Output, Results, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Graph, GraphBuilder, Tag, TagKind};

/// A graph `src -> chain[0] -> chain[1] -> ...`, each `(id, registered name)`, with `settings` (`"<id>.<key>"`) applied. The
/// source returns `data` (of `format`'s kind) `piece` items per pass. A setting value may be written as [`typed`] takes it.
pub fn chain(
    format: Format,
    data: Data,
    piece: usize,
    chain: &[(&str, &str)],
    settings: &[(&str, Value)],
) -> Graph {
    let r = sdrkit_components::registry();
    let mut b = GraphBuilder::new().add("src", Source::new(format, data, piece));
    let mut prev = "src".to_string();
    let mut schemas = Vec::new();
    for (id, name) in chain {
        let c = r.make(name).unwrap_or_else(|e| panic!("{name}: {e}"));
        schemas.push((*id, c.info().settings));
        b = b.add_boxed(id, c).connect(&prev, id);
        prev = id.to_string();
    }
    b = b
        .add_boxed("sink", r.make("dsp.null_sink").unwrap())
        .connect(&format!("{prev}.out"), "sink");
    for (k, v) in settings {
        let (id, key) = k.split_once('.').unwrap();
        let schema = &schemas.iter().find(|(i, _)| *i == id).unwrap().1;
        b = b.set(k, typed(schema, key, v));
    }
    b.build().unwrap()
}

/// What one port of a graph produced over a run: its items and its tags (absolute positions).
pub struct Run {
    pub data: Data,
    pub tags: Vec<Tag>,
}

/// Configures `g` and runs `passes` passes, gathering `port`.
pub fn run(g: &mut Graph, passes: usize, port: &str) -> Run {
    let _tap = g.tap(port, 1).unwrap();
    g.configure_input(Format::iq(1.0, 0.0, 0.0)).unwrap();
    let mut data = g.format(port).unwrap().kind.empty_data();
    let mut tags = Vec::new();
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..passes {
        out.clear();
        g.process(&empty, &mut out);
        let b = g.buffer(port).unwrap();
        extend_range(&mut data, &b.data, 0, b.data.len());
        tags.extend(b.tags().iter().cloned());
    }
    Run { data, tags }
}

/// Builds the chain and runs it until the source is empty (plus a few passes).
pub fn run_chain(
    format: Format,
    data: Data,
    piece: usize,
    stages: &[(&str, &str)],
    settings: &[(&str, Value)],
    port: &str,
) -> Run {
    let n = data.len();
    let mut g = chain(format, data, piece, stages, settings);
    run(&mut g, n / piece.max(1) + 3, port)
}

pub fn hard(d: &Data) -> Vec<u8> {
    match d {
        Data::HardBits(v) => v.clone(),
        d => panic!("not hard bits: {d:?}"),
    }
}

pub fn llrs(d: &Data) -> Vec<i8> {
    match d {
        Data::Llr(v) => v.clone(),
        d => panic!("not LLRs: {d:?}"),
    }
}

pub fn packets(d: &Data) -> Vec<Vec<u8>> {
    match d {
        Data::Bytes(v) => v.iter().map(|p| p.bytes().to_vec()).collect(),
        d => panic!("not bytes: {d:?}"),
    }
}

/// The positions and contents of the `FrameStart` tags.
pub fn frame_starts(tags: &[Tag]) -> Vec<(u64, bool, u32)> {
    tags.iter()
        .filter_map(|t| match t.kind {
            TagKind::FrameStart { inverted, errors } => Some((t.at, inverted, errors)),
            _ => None,
        })
        .collect()
}

/// Repeatable pseudo-random bits.
pub fn random_bits(n: usize, seed: u32) -> Vec<u8> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s >> 7) as u8 & 1
        })
        .collect()
}

/// Hard bits as LLRs of magnitude `m` (0 is +m).
pub fn soft(bits: &[u8], m: i8) -> Vec<i8> {
    bits.iter()
        .map(|&b| if b & 1 == 0 { m } else { -m })
        .collect()
}

/// The hard decisions of LLRs.
pub fn decide(l: &[i8]) -> Vec<u8> {
    l.iter().map(|&v| (v < 0) as u8).collect()
}

/// The pieces every cut test feeds: one item at a time, odd sizes, and everything at once.
pub const PIECES: &[usize] = &[1, 3, 7, 64, 1 << 20];

/// A number result of a component.
pub fn result_i64(g: &Graph, id: &str, key: &str) -> i64 {
    let r: &Results = g
        .component_results(id)
        .unwrap_or_else(|| panic!("{id} has no results"));
    let f = r.fields();
    match f.decl().id(key).and_then(|i| f.value(i)) {
        Some(Value::I64(v)) => *v,
        other => panic!("{id} has no number {key}: {other:?}"),
    }
}
