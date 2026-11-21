//! `modem.msk_demod`: MSK made by `sdrkit-algo-sim` (`Cpm::msk`) with a carrier offset of 35 % of the bit rate, decoded back to the
//! exact bits with the offset tracker and not without it; cuts; strict settings.

use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_components::modem::demod::MskDemod;
use sdrkit_components::testing::run_iq;
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph};

#[path = "modem_demod_util.rs"]
mod util;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};
use util::{contains, hard, random_bits};

const RATE: f64 = 9_600.0;
const BAUD: f64 = 1_200.0;

/// MSK at 8 samples per bit, `offset` of the bit rate above 0 Hz, light noise. Bit 0 is the higher frequency.
fn msk(bits: &[u8], offset: f64, seed: u64) -> Vec<Complex32> {
    let mut m = Cpm::msk(RATE / BAUD).unwrap();
    let a: Vec<f32> = bits.iter().map(|&b| 1.0 - 2.0 * b as f32).collect();
    let mut x = Vec::new();
    m.push(&a, &mut x);
    m.flush(&mut x);
    let mut ph = sdrkit_algo_dsp::loopfilter::PhaseAcc::new(offset * BAUD / RATE);
    for v in &mut x {
        *v *= ph.next_phasor();
    }
    Awgn::with_power(0.02, seed).unwrap().process(&mut x);
    x
}

fn graph(tau_s: f64) -> Graph {
    Tb::new()
        .add("msk", MskDemod::default())
        .add("sink", NullSink::default())
        .input("msk")
        .connect("msk.out", "sink")
        .set("msk.baud", BAUD)
        .set("msk.offset_tau_s", tau_s)
        .build()
        .unwrap()
}

fn llrs(d: &Data) -> Vec<i8> {
    match d {
        Data::Llr(v) => v.clone(),
        d => panic!("{d:?}"),
    }
}

#[test]
fn the_offset_tracker_follows_a_third_of_the_bit_rate() {
    let bits = random_bits(5000, 21);
    let x = msk(&bits, 0.35, 3);
    let f = Format::iq(RATE, 0.0, 0.0);
    // 0.1 s is 120 bits: the tracker settles well inside the first 1500 bits.
    let mut g = graph(0.1);
    let soft = run_iq(&mut g, f.clone(), &x, 4096, "msk.out");
    assert!(
        contains(&hard(&soft), &bits, 1500, 20, 1520),
        "bits not found with the tracker"
    );
    // Any cut produces the same soft bits.
    let mut g = graph(0.1);
    assert_eq!(
        llrs(&run_iq(&mut g, f.clone(), &x, 333, "msk.out")),
        llrs(&soft)
    );
    // Without the tracker the receiver follows an eighth of the bit rate only.
    let mut g = graph(0.0);
    let plain = run_iq(&mut g, f, &x, 4096, "msk.out");
    assert!(!contains(&hard(&plain), &bits, 1500, 20, 1520));
}

#[test]
fn settings_are_strict() {
    let mut d = MskDemod::default();
    assert_eq!(
        set(&mut d, "baud", &Value::F64(100.0)),
        Ok(Applied::Reformat)
    );
    assert!(set(&mut d, "baud", &Value::F64(0.0)).is_err());
    assert!(set(&mut d, "offset_tau_s", &Value::F64(-1.0)).is_err());
    assert!(set(&mut d, "clock_bandwidth", &Value::F64(1.0)).is_err());
    assert!(set(&mut d, "invert", &Value::Text("maybe".into())).is_err());
    assert!(set(&mut d, "llr_scale", &Value::F64(-1.0)).is_err());
    assert!(set(&mut d, "colour", &Value::Bool(true)).is_err());
    let mut g = graph(0.0);
    // One sample per bit is too few.
    assert!(g.configure_input(Format::iq(BAUD, 0.0, 0.0)).is_err());
    assert!(g.configure_input(Format::iq(RATE, 0.0, 0.0)).is_ok());
}
