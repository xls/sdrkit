//! `modem.gmsk_demod`: GMSK made by `sdrkit-algo-sim` (`Cpm::gmsk`) decoded back to the exact bits by the discriminator receiver and by
//! MLSE; cuts; strict settings.

use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_components::modem::demod::GmskDemod;
use sdrkit_components::testing::run_iq;
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph};

#[path = "modem_demod_util.rs"]
mod util;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};
use util::{contains, hard, random_bits};

/// GMSK of `bt` at 4 samples per symbol (9600 baud at 38.4 kHz), a small frequency offset, light noise. Bit 0 is level +1.
fn gmsk(bits: &[u8], bt: f64, seed: u64) -> Vec<Complex32> {
    let mut m = Cpm::gmsk(bt, 4.0).unwrap();
    let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
    let mut x = Vec::new();
    m.push(&a, &mut x);
    m.flush(&mut x);
    let mut ph = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(0.002);
    for v in &mut x {
        *v *= ph.next_phasor();
    }
    Awgn::with_power(0.02, seed).unwrap().process(&mut x);
    x
}

fn graph(method: &str, bt: f64) -> Graph {
    Tb::new()
        .add("gmsk", GmskDemod::default())
        .add("sink", NullSink::default())
        .input("gmsk")
        .connect("gmsk.out", "sink")
        .set("gmsk.baud", Value::F64(9600.0))
        .set("gmsk.bt", bt)
        .set("gmsk.method", method)
        .build()
        .unwrap()
}

fn llrs(d: &Data) -> Vec<i8> {
    match d {
        Data::Llr(v) => v.clone(),
        d => panic!("{d:?}"),
    }
}

const RATE: f64 = 38_400.0;

#[test]
fn the_discriminator_receiver_returns_the_bits() {
    let bits = random_bits(5000, 11);
    let x = gmsk(&bits, 0.5, 1);
    let f = Format::iq(RATE, 0.0, 0.0);
    let mut g = graph("discriminator", 0.5);
    let soft = run_iq(&mut g, f.clone(), &x, 4096, "gmsk.out");
    assert!(
        contains(&hard(&soft), &bits, 500, 20, 520),
        "bits not found"
    );
    let mut g = graph("discriminator", 0.5);
    let bits_out = run_iq(&mut g, f.clone(), &x, 1000, "gmsk.bits");
    // The hard bits are the signs of the exact soft values: the same as the LLRs' except where an LLR rounded to 0 (erased).
    let (l, b) = (llrs(&soft), hard(&bits_out));
    assert_eq!(l.len(), b.len());
    assert!(l
        .iter()
        .zip(&b)
        .all(|(&l, &b)| l == 0 || (l < 0) as u8 == b));
    let mut g = graph("discriminator", 0.5);
    assert_eq!(llrs(&run_iq(&mut g, f, &x, 1, "gmsk.out")), llrs(&soft));
}

#[test]
fn mlse_over_the_levels_returns_the_bits_at_bt_0_3() {
    let bits = random_bits(5000, 12);
    let x = gmsk(&bits, 0.3, 2);
    let f = Format::iq(RATE, 0.0, 0.0);
    let mut g = graph("mlse", 0.3);
    let soft = run_iq(&mut g, f.clone(), &x, 4096, "gmsk.out");
    let got = hard(&soft);
    assert!(contains(&got, &bits, 500, 120, 520), "bits not found");
    // Any cut produces the same soft bits.
    let mut g = graph("mlse", 0.3);
    assert_eq!(llrs(&run_iq(&mut g, f, &x, 333, "gmsk.out")), llrs(&soft));
    assert!(matches!(soft, Data::Llr(ref v) if v.iter().any(|&l| l.abs() > 20)));
}

#[test]
fn settings_are_strict() {
    let mut d = GmskDemod::default();
    assert_eq!(set(&mut d, "bt", &Value::F64(0.3)), Ok(Applied::Reformat));
    assert!(set(&mut d, "bt", &Value::F64(0.0)).is_err());
    assert!(set(&mut d, "method", &Value::Text("viterbi".into())).is_err());
    assert!(set(&mut d, "baud", &Value::Text("fast".into())).is_err());
    assert!(set(&mut d, "invert", &Value::Text("maybe".into())).is_err());
    assert!(set(&mut d, "llr_scale", &Value::F64(-1.0)).is_err());
    assert!(set(&mut d, "colour", &Value::Bool(true)).is_err());
    let mut g = graph("discriminator", 0.5);
    // One sample per symbol is too few.
    assert!(g.configure_input(Format::iq(9600.0, 0.0, 0.0)).is_err());
    assert!(g.configure_input(Format::iq(RATE, 0.0, 0.0)).is_ok());
}
