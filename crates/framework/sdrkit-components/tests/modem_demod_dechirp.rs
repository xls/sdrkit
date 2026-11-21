//! `modem.dechirp`: chirp symbols built from the plain crate's own base chirp (`Dechirp::base_chirp`, symbol `s` is the up-chirp
//! started `s` chips in) decoded back to the exact symbol values, at one and at two samples per chip; cuts; strict settings.

use sdrkit_algo_modem::chirp::Dechirp;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_components::modem::demod::{ChirpSymbol, DechirpDemod};
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::{components::NullSink, Applied, Format, Graph};

#[path = "modem_demod_util.rs"]
mod util;
use util::run_items;
use util::set_util::{set, Tb};

/// Symbols of spreading factor `sf` at `os` samples per chip, in noise.
fn chirps(sf: u8, os: usize, symbols: &[usize]) -> Vec<Complex32> {
    let d = Dechirp::new(sf, 1).unwrap();
    let n = d.chips();
    let mut x = Vec::new();
    for &s in symbols {
        for k in 0..n {
            for _ in 0..os {
                x.push(d.base_chirp((k + s) % n));
            }
        }
    }
    Awgn::with_power(0.5, 9).unwrap().process(&mut x);
    x
}

fn graph(sf: u8, bw: f64) -> Graph {
    Tb::new()
        .add("dechirp", DechirpDemod::default())
        .add("sink", NullSink::default())
        .input("dechirp")
        .connect("dechirp", "sink")
        .set("dechirp.sf", Value::I64(sf as i64))
        .set("dechirp.bw_hz", Value::Hz(bw))
        .build()
        .unwrap()
}

#[test]
fn chirp_symbols_decode_to_their_values() {
    let symbols: Vec<usize> = (0..40).map(|k| (k * 37 + 5) % 128).collect();
    for (os, piece) in [(1usize, 4096usize), (2, 100), (2, 1)] {
        let x = chirps(7, os, &symbols);
        let mut g = graph(7, 125_000.0);
        let f = Format::iq(125_000.0 * os as f64, 0.0, 0.0);
        let out = run_items::<ChirpSymbol>(&mut g, f, &x, piece, "dechirp.out");
        let got: Vec<usize> = out.iter().map(|t| t.value.bin as usize).collect();
        assert_eq!(got, symbols, "os {os} piece {piece}");
        let s = &out[3].value;
        assert_eq!(s.symbol, 3);
        assert_eq!(s.bins.len(), 128);
        assert!(s.power > 20.0 * s.noise);
        assert!((out[3].time - 3.0 * 128.0 / 125_000.0).abs() < 1e-9);
    }
}

#[test]
fn a_retune_restarts_the_symbol_count() {
    let symbols = [10usize, 20, 30, 40];
    let x = chirps(7, 1, &symbols);
    let mut g = graph(7, 125_000.0);
    g.configure_input(Format::iq(125_000.0, 0.0, 0.0)).unwrap();
    let mut out = sdrkit_core::Output::default();
    // Half a symbol, then a retune: the half symbol is dropped and the count starts again.
    g.process(&x[..64], &mut out);
    sdrkit_core::Receiver::retuned(
        &mut g,
        &sdrkit_core::InputInfo {
            rate_hz: 125_000.0,
            centre_hz: 1e6,
            tuned_hz: 1e6,
            live: false,
        },
    );
    g.process(&x[128..], &mut out);
    let items = util::items_of::<ChirpSymbol>(&g.buffer("dechirp.out").unwrap().data);
    let got: Vec<(u64, u32)> = items
        .iter()
        .map(|t| (t.value.symbol, t.value.bin))
        .collect();
    assert_eq!(got, [(0, 20), (1, 30), (2, 40)]);
}

#[test]
fn settings_are_strict() {
    let mut d = DechirpDemod::default();
    assert_eq!(set(&mut d, "sf", &Value::F64(9.0)), Ok(Applied::Reformat));
    assert!(set(&mut d, "sf", &Value::F64(13.0)).is_err());
    assert!(set(&mut d, "sf", &Value::F64(7.5)).is_err());
    assert!(set(&mut d, "bw_hz", &Value::F64(-1.0)).is_err());
    assert!(set(&mut d, "direction", &Value::Text("sideways".into())).is_err());
    assert!(set(&mut d, "colour", &Value::Bool(true)).is_err());
    // 300 kHz is not a whole multiple of 125 kHz.
    let mut g = graph(7, 125_000.0);
    assert!(g.configure_input(Format::iq(300_000.0, 0.0, 0.0)).is_err());
}
