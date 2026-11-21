//! Helpers for testing components in a graph: a [`Source`] that feeds any stream kind in pieces, and [`run_passes`] that runs a
//! graph and gathers what one port produced.

use sdrkit_core::Values;
use sdrkit_core::{Complex32, ConfigError, Output};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Graph, Io, Port};

/// A generator component that emits the items of `data` (of `format`'s kind), `per_pass` at a time, then nothing.
///
/// ```
/// use sdrkit_components::testing::{run_passes, Source};
/// use sdrkit_graph::{stream::Data, Format, GraphBuilder};
///
/// let mut g = GraphBuilder::new()
///     .add("src", Source::new(Format::hard_bits(1200.0), Data::HardBits(vec![1, 0, 1, 1]), 3))
///     .add("sink", sdrkit_graph::components::NullSink::default())
///     .connect("src", "sink")
///     .build()
///     .unwrap();
/// let out = run_passes(&mut g, 2, "src.out");
/// assert!(matches!(out, Data::HardBits(ref b) if b == &[1, 0, 1, 1]));
/// ```
pub struct Source {
    format: Format,
    data: Data,
    pos: usize,
    per_pass: usize,
}

impl Source {
    /// Emits `data` in pieces of `per_pass` items. Supported kinds: `Iq`, `Symbols`, `Real`, `Audio`, `HardBits`, `Llr`, `Bytes` and
    /// `Messages`.
    pub fn new(format: Format, data: Data, per_pass: usize) -> Source {
        Source {
            format,
            data,
            pos: 0,
            per_pass: per_pass.max(1),
        }
    }
}

/// Appends `src[a..b]` to `dst` (both of the same kind). Panics on a kind this helper does not support.
pub fn extend_range(dst: &mut Data, src: &Data, a: usize, b: usize) {
    match (dst, src) {
        (Data::Iq(d), Data::Iq(s)) | (Data::Symbols(d), Data::Symbols(s)) => {
            d.extend_from_slice(&s[a..b])
        }
        (Data::Real(d), Data::Real(s)) => d.extend_from_slice(&s[a..b]),
        (Data::Audio(d), Data::Audio(s)) => {
            let ch = usize::from(s.channels);
            d.channels = s.channels;
            d.samples.extend_from_slice(&s.samples[a * ch..b * ch]);
        }
        (Data::HardBits(d), Data::HardBits(s)) => d.extend_from_slice(&s[a..b]),
        (Data::Llr(d), Data::Llr(s)) => d.extend_from_slice(&s[a..b]),
        (Data::Bytes(d), Data::Bytes(s)) => d.extend_from_slice(&s[a..b]),
        (Data::Messages(d), Data::Messages(s)) => d.extend_from_slice(&s[a..b]),
        (d, s) => panic!("cannot copy {s:?} into {d:?}"),
    }
}

impl Component for Source {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("test.source", vec![Port::new("out", self.format.kind)])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.pos = 0;
        Ok(vec![self.format.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let n = self.data.len();
        let end = (self.pos + self.per_pass).min(n);
        if end > self.pos {
            extend_range(&mut io.output(0).data, &self.data, self.pos, end);
            self.pos = end;
        }
    }
}

/// Configures `g` (for an `Iq` input it does not read), runs `passes` passes with no input samples, and returns everything output
/// `port` produced, in order.
pub fn run_passes(g: &mut Graph, passes: usize, port: &str) -> Data {
    g.configure_input(Format::iq(1.0, 0.0, 0.0))
        .expect("the graph configures");
    let mut all: Option<Data> = None;
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..passes {
        out.clear();
        g.process(&empty, &mut out);
        let b = g.buffer(port).unwrap_or_else(|| panic!("no port {port}"));
        let d = all.get_or_insert_with(|| g.format(port).expect("port").kind.empty_data());
        extend_range(d, &b.data, 0, b.data.len());
    }
    all.unwrap_or_else(|| g.format(port).expect("port").kind.empty_data())
}

/// Runs `x` through `g` (whose input is `Iq`) in pieces of `piece` samples and returns everything output `port` produced.
pub fn run_iq(g: &mut Graph, input: Format, x: &[Complex32], piece: usize, port: &str) -> Data {
    g.configure_input(input).expect("the graph configures");
    let mut all = g.format(port).expect("port").kind.empty_data();
    let mut out = Output::default();
    for p in x.chunks(piece.max(1)) {
        out.clear();
        g.process(p, &mut out);
        let b = g.buffer(port).unwrap_or_else(|| panic!("no port {port}"));
        extend_range(&mut all, &b.data, 0, b.data.len());
    }
    all
}
