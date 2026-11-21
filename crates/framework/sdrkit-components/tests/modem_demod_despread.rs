//! `modem.despread`: BPSK spread by the IEEE 802.11 Barker-11 code (`dsss::BARKER_11`) and by the GPS C/A code of PRN 7
//! (`sdrkit_util_bits::seq::gps_ca`), at an unknown code phase, acquired and despread back to the exact symbols, at a fixed code phase
//! and with the delay-locked loop; cuts; strict settings.

use sdrkit_algo_modem::dsss::{chips_from_bits, BARKER_11};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_components::modem::demod::Despread;
use sdrkit_components::testing::run_iq;
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph};

#[path = "modem_demod_util.rs"]
mod util;
use util::random_bits;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};

/// `bits` (0 is +1) spread by `code`, `periods` code periods per bit, at `spc` samples per chip, after `lead` samples of noise.
fn spread(
    bits: &[u8],
    code: &[f32],
    periods: usize,
    spc: usize,
    lead: usize,
    noise: f64,
) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); lead];
    for &b in bits {
        let s = 1.0 - 2.0 * b as f32;
        for _ in 0..periods {
            for &c in code {
                for _ in 0..spc {
                    x.push(Complex32::new(s * c, 0.0));
                }
            }
        }
    }
    Awgn::with_power(noise, 4).unwrap().process(&mut x);
    x
}

fn graph(set: &[(&str, Value)]) -> Graph {
    let mut b = Tb::new()
        .add("ds", Despread::default())
        .add("sink", NullSink::default())
        .input("ds")
        .connect("ds", "sink");
    for (k, v) in set {
        b = b.set(&format!("ds.{k}"), v.clone());
    }
    b.build().unwrap()
}

fn symbols(d: &Data) -> Vec<Complex32> {
    match d {
        Data::Symbols(v) => v.clone(),
        d => panic!("{d:?}"),
    }
}

#[test]
fn barker_11_is_acquired_and_despread_to_the_symbols() {
    let code: Vec<f32> = BARKER_11.iter().map(|&c| c as f32).collect();
    let bits = random_bits(400, 21);
    // 802.11b at 1 Mbit/s: 11 Mchip/s at 2 samples per chip; the code starts 7 samples in.
    let x = spread(&bits, &code, 1, 2, 7, 0.5);
    let set = [
        ("chip_rate_hz", Value::F64(11e6)),
        ("acquire_periods", Value::F64(8.0)),
    ];
    let f = Format::iq(22e6, 0.0, 0.0);
    let mut first: Option<Vec<Complex32>> = None;
    for piece in [4096usize, 5, 1] {
        let mut g = graph(&set);
        let s = symbols(&run_iq(&mut g, f.clone(), &x, piece, "ds.out"));
        // Every symbol, from the first, is found (the last one is complete only when the next sample arrives).
        assert!(s.len() >= bits.len() - 1, "{}", s.len());
        for (k, (z, &b)) in s.iter().zip(&bits).enumerate() {
            assert_eq!((z.re < 0.0) as u8, b, "symbol {k}: {z}");
            assert!((z.re.abs() - 1.0).abs() < 0.4, "symbol {k}: {z}");
        }
        match &first {
            None => first = Some(s),
            Some(r) => assert_eq!(&s, r, "piece {piece}"),
        }
    }
    // One symbol per code period: 1 MHz.
    let mut g = graph(&set);
    g.configure_input(f).unwrap();
    assert_eq!(g.format("ds.out").unwrap().rate, 1e6);
}

#[test]
fn gps_ca_is_tracked_by_the_delay_locked_loop() {
    let mut bits1023 = [0u8; 1023];
    sdrkit_util_bits::seq::gps_ca(7, &mut bits1023).unwrap();
    let mut code = Vec::new();
    chips_from_bits(&bits1023, &mut code);
    // 50 bit/s navigation data: 20 code periods per bit, 1.023 Mchip/s at 2 samples per chip, code phase 1000.5 chips in.
    let bits = random_bits(12, 22);
    let x = spread(&bits, &code, 20, 2, 2001, 4.0);
    let set = [
        ("code", Value::Text("gps_ca".into())),
        ("prn", Value::F64(7.0)),
        ("chip_rate_hz", Value::F64(1.023e6)),
        ("periods_per_symbol", Value::F64(20.0)),
        ("acquire_periods", Value::F64(4.0)),
        ("tracking", Value::Text("dll".into())),
    ];
    let f = Format::iq(2.046e6, 0.0, 0.0);
    let mut g = graph(&set);
    let s = symbols(&run_iq(&mut g, f.clone(), &x, 65_536, "ds.out"));
    assert_eq!(g.format("ds.out").unwrap().rate, 50.0);
    // The code starts 2001 samples in: the first period counted is the first one sent, so symbols line up with the bits.
    assert!(s.len() >= 11, "{}", s.len());
    let got: Vec<u8> = s.iter().map(|z| (z.re < 0.0) as u8).collect();
    assert_eq!(&got[..11], &bits[..11]);
    let mut g2 = graph(&set);
    assert_eq!(symbols(&run_iq(&mut g2, f, &x, 10_000, "ds.out")), s);
}

#[test]
fn settings_are_strict() {
    let mut d = Despread::default();
    assert_eq!(
        set(&mut d, "code", &Value::Text("gps_ca".into())),
        Ok(Applied::Reformat)
    );
    assert!(set(&mut d, "code", &Value::Text("kasami".into())).is_err());
    assert!(set(&mut d, "prn", &Value::F64(33.0)).is_err());
    assert!(set(&mut d, "chips", &Value::Text("+-x".into())).is_err());
    assert!(set(&mut d, "chips", &Value::Text("+--+".into())).is_ok());
    assert!(set(&mut d, "tracking", &Value::Text("pll".into())).is_err());
    assert!(set(&mut d, "periods_per_symbol", &Value::F64(0.0)).is_err());
    assert!(set(&mut d, "colour", &Value::Bool(true)).is_err());
    // A custom code with no chips, or fewer than one sample per chip, is refused when the graph configures.
    let mut g = graph(&[("code", Value::Text("custom".into()))]);
    assert!(g.configure_input(Format::iq(1e6, 0.0, 0.0)).is_err());
    let mut g = graph(&[("chip_rate_hz", Value::F64(2e6))]);
    assert!(g.configure_input(Format::iq(1e6, 0.0, 0.0)).is_err());
}
