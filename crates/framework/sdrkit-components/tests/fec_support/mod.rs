//! Helpers shared by the `fec_*` tests: a source that also places `FrameStart` tags, a runner that gathers several ports, a
//! repeatable generator, and conversions between bits, LLRs and packets.
#![allow(dead_code)]

use sdrkit_components::fec::FrameStatus;
use sdrkit_components::testing::extend_range;
use sdrkit_core::{Complex32, ConfigError, Output, Values};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Graph, Io, Packet, Port, Tag, TagKind, Timed,
};
use std::sync::Arc;

#[path = "../set_util/mod.rs"]
pub mod set_util;
#[allow(unused_imports)]
pub use set_util::{finish, gset, result_f64, set, tv, txt, typed, Tb};

/// A generator that emits `data` `per_pass` items at a time and puts a `FrameStart` tag on each item listed in `tags`.
pub struct TaggedSource {
    format: Format,
    data: Data,
    tags: Vec<u64>,
    pos: usize,
    per_pass: usize,
}

impl TaggedSource {
    pub fn new(format: Format, data: Data, tags: &[u64], per_pass: usize) -> Self {
        TaggedSource {
            format,
            data,
            tags: tags.to_vec(),
            pos: 0,
            per_pass: per_pass.max(1),
        }
    }
}

impl Component for TaggedSource {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator(
            "test.tagged_source",
            vec![Port::new("out", self.format.kind)],
        )
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.pos = 0;
        Ok(vec![self.format.clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let end = (self.pos + self.per_pass).min(self.data.len());
        if end > self.pos {
            let out = io.output(0);
            extend_range(&mut out.data, &self.data, self.pos, end);
            for &t in &self.tags {
                if (self.pos as u64..end as u64).contains(&t) {
                    out.tag_here(
                        t as usize - self.pos,
                        TagKind::FrameStart {
                            inverted: false,
                            errors: 0,
                        },
                    );
                }
            }
            self.pos = end;
        }
    }
}

/// What a run gathered from one port.
pub struct Gathered {
    pub data: Option<Data>,
    pub tags: Vec<Tag>,
    pub status: Vec<FrameStatus>,
}

/// Configures `g`, runs `passes` passes and gathers each of `ports` (items, tags; for a `status` port its items).
pub fn run(g: &mut Graph, passes: usize, ports: &[&str]) -> Vec<Gathered> {
    let _taps: Vec<_> = ports.iter().map(|p| g.tap(p, 1).unwrap()).collect();
    g.configure_input(Format::iq(1.0, 0.0, 0.0))
        .expect("the graph configures");
    let mut got: Vec<Gathered> = ports
        .iter()
        .map(|_| Gathered {
            data: None,
            tags: Vec::new(),
            status: Vec::new(),
        })
        .collect();
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..passes {
        out.clear();
        g.process(&empty, &mut out);
        for (p, gth) in ports.iter().zip(got.iter_mut()) {
            let b = g.buffer(p).unwrap_or_else(|| panic!("no port {p}"));
            gth.tags.extend(b.tags().iter().cloned());
            match &b.data {
                Data::Items(c) => {
                    if let Some(c) = c {
                        if let Some(v) = c.as_any().downcast_ref::<Vec<Timed<FrameStatus>>>() {
                            gth.status.extend(v.iter().map(|t| t.value));
                        }
                    }
                }
                d => {
                    let all = gth
                        .data
                        .get_or_insert_with(|| g.format(p).expect("port").kind.empty_data());
                    extend_range(all, d, 0, d.len());
                }
            }
        }
    }
    got
}

pub fn hard_bits(d: &Option<Data>) -> Vec<u8> {
    match d {
        Some(Data::HardBits(b)) => b.clone(),
        None => Vec::new(),
        Some(d) => panic!("not hard bits: {d:?}"),
    }
}

pub fn llrs(d: &Option<Data>) -> Vec<i8> {
    match d {
        Some(Data::Llr(b)) => b.clone(),
        None => Vec::new(),
        Some(d) => panic!("not LLRs: {d:?}"),
    }
}

pub fn packets(d: &Option<Data>) -> Vec<Vec<u8>> {
    match d {
        Some(Data::Bytes(b)) => b.iter().map(|p| p.bytes().to_vec()).collect(),
        None => Vec::new(),
        Some(d) => panic!("not bytes: {d:?}"),
    }
}

/// Byte packets as a `Bytes` stream, 0.3 s apart (so results, updated a few times a second of signal, follow every packet).
pub fn bytes_data(packets: &[Vec<u8>]) -> Data {
    Data::Bytes(
        packets
            .iter()
            .enumerate()
            .map(|(i, p)| Packet::new(Arc::from(p.clone()), i as f64 * 0.3))
            .collect(),
    )
}

/// Hard bits to full-confidence LLRs of magnitude `m` (positive means 0).
pub fn llr_of(bits: &[u8], m: i8) -> Vec<i8> {
    bits.iter().map(|&b| if b == 0 { m } else { -m }).collect()
}

/// Bits packed most significant first, the last byte padded with zeros.
pub fn pack(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| {
            c.iter()
                .enumerate()
                .fold(0u8, |a, (i, &b)| a | (b << (7 - i)))
        })
        .collect()
}

/// Bits from text of 0 and 1 (other characters skipped).
pub fn bits_of(s: &str) -> Vec<u8> {
    s.bytes()
        .filter(|b| *b == b'0' || *b == b'1')
        .map(|b| b - b'0')
        .collect()
}

/// A small repeatable generator.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn bits(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.next() >> 33) as u8 & 1).collect()
    }
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.next() >> 24) as u8).collect()
    }
    /// About Gaussian, standard deviation 1.
    pub fn gauss(&mut self) -> f32 {
        let s: f32 = (0..12)
            .map(|_| (self.next() >> 40) as f32 / (1u64 << 24) as f32)
            .sum();
        s - 6.0
    }
    /// `count` distinct positions below `n`.
    pub fn positions(&mut self, n: usize, count: usize) -> Vec<usize> {
        let mut v: Vec<usize> = Vec::with_capacity(count);
        while v.len() < count {
            let p = self.below(n);
            if !v.contains(&p) {
                v.push(p);
            }
        }
        v
    }
}

/// A results field of component `id`, as a number.
pub fn field(g: &Graph, id: &str, key: &str) -> f64 {
    result_f64(g, id, key)
}
