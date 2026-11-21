//! Helpers shared by the `modem_demod_*` tests (included by them with `#[path]`): gathering a typed output, which
//! `sdrkit_components::testing` does not copy, and finding a run of bits.

#![allow(dead_code)]

use sdrkit_core::{Complex32, Output};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Graph, Item, Timed};

#[path = "set_util/mod.rs"]
pub mod set_util;

/// The items of typed output `port` held in `d` (after one pass).
pub fn items_of<T: Item>(d: &Data) -> Vec<Timed<T>> {
    match d {
        Data::Items(Some(b)) => b
            .as_any()
            .downcast_ref::<Vec<Timed<T>>>()
            .expect("the item type")
            .clone(),
        Data::Items(None) => Vec::new(),
        d => panic!("not a typed output: {d:?}"),
    }
}

/// Runs `x` through `g` (whose input is `Iq` in `input`) in pieces of `piece` samples and gathers typed output `port`.
pub fn run_items<T: Item>(
    g: &mut Graph,
    input: Format,
    x: &[Complex32],
    piece: usize,
    port: &str,
) -> Vec<Timed<T>> {
    let _tap = g.tap(port, 1).expect("the port");
    g.configure_input(input).expect("the graph configures");
    let mut all = Vec::new();
    let mut out = Output::default();
    for p in x.chunks(piece.max(1)) {
        out.clear();
        g.process(p, &mut out);
        all.extend(items_of::<T>(&g.buffer(port).expect("port").data));
    }
    all
}

/// Configures `g` and runs `passes` passes with no input (for a graph fed by a source), gathering typed output `port`.
pub fn run_items_passes<T: Item>(g: &mut Graph, passes: usize, port: &str) -> Vec<Timed<T>> {
    let _tap = g.tap(port, 1).expect("the port");
    g.configure_input(Format::iq(1.0, 0.0, 0.0))
        .expect("the graph configures");
    let mut all = Vec::new();
    let mut out = Output::default();
    let empty: [Complex32; 0] = [];
    for _ in 0..passes {
        out.clear();
        g.process(&empty, &mut out);
        all.extend(items_of::<T>(&g.buffer(port).expect("port").data));
    }
    all
}

/// True when `want[skip..want.len() - tail]` appears in `got` starting within its first `lead` positions.
pub fn contains(got: &[u8], want: &[u8], skip: usize, tail: usize, lead: usize) -> bool {
    let w = &want[skip..want.len() - tail];
    (0..lead).any(|o| got.len() >= o + w.len() && &got[o..o + w.len()] == w)
}

/// The hard decisions of an `Llr` or `HardBits` output.
pub fn hard(d: &Data) -> Vec<u8> {
    match d {
        Data::Llr(v) => v.iter().map(|&l| (l < 0) as u8).collect(),
        Data::HardBits(v) => v.clone(),
        d => panic!("not bits: {d:?}"),
    }
}

/// Random bits from `sdrkit-algo-sim`'s generator.
pub fn random_bits(n: usize, seed: u64) -> Vec<u8> {
    let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..n).map(|_| r.bit()).collect()
}
