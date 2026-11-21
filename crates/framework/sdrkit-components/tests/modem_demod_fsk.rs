//! `modem.fsk_demod`: 2-FSK on IQ and AFSK audio by the tone correlators, 4-level FSK (C4FM dibits) by the discriminator, each
//! made by `sdrkit-algo-sim`'s modulators and decoded back to the exact bits; cuts; strict settings.

use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_sim::modulate::{Afsk, Cpm};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_components::modem::demod::{FskDemod, FskEnergies};
use sdrkit_components::testing::{run_iq, run_passes, Source};
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Component, Format, Graph, Kind, Timed};

#[path = "modem_demod_util.rs"]
mod util;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};
use util::{contains, hard, random_bits, run_items};

fn iq_graph(settings: &[(&str, Value)]) -> Graph {
    let mut b = Tb::new()
        .add("fsk", FskDemod::default())
        .add("sink", NullSink::default())
        .input("fsk")
        .connect("fsk.out", "sink");
    for (k, v) in settings {
        b = b.set(&format!("fsk.{k}"), v.clone());
    }
    b.build().unwrap()
}

/// 2-FSK, 1200 baud, tones at -600 and +600 Hz (h = 1) at 12 kHz, in light noise. Level +1 (the upper tone, tone 1) is bit 1.
fn fsk2_signal(bits: &[u8]) -> Vec<Complex32> {
    let mut m = Cpm::cpfsk(1.0, 10.0).unwrap();
    let a: Vec<f32> = bits.iter().map(|&b| 2.0 * b as f32 - 1.0).collect();
    let mut x = Vec::new();
    m.push(&a, &mut x);
    m.flush(&mut x);
    Awgn::with_power(0.05, 3).unwrap().process(&mut x);
    x
}

#[test]
fn two_fsk_on_iq_by_the_correlators_returns_the_bits() {
    let bits = random_bits(2000, 1);
    let x = fsk2_signal(&bits);
    let set = [
        ("shift_hz", Value::F64(1200.0)),
        ("baud", Value::F64(1200.0)),
    ];
    let mut g = iq_graph(&set);
    let f = Format::iq(12_000.0, 0.0, 0.0);
    let got = hard(&run_iq(&mut g, f.clone(), &x, 4096, "fsk.out"));
    assert!(contains(&got, &bits, 100, 10, 120), "bits not found");
    assert_eq!(g.format("fsk.out").unwrap().llr_scale, Some(4.0));
    assert_eq!(g.format("fsk.out").unwrap().rate, 1200.0);
    // Any cut produces the same soft bits and the same tone energies.
    let mut a = iq_graph(&set);
    let mut b = iq_graph(&set);
    let la = run_iq(&mut a, f.clone(), &x, 1, "fsk.out");
    let lb = run_iq(&mut b, f.clone(), &x, 777, "fsk.out");
    assert!(matches!((&la, &lb), (Data::Llr(p), Data::Llr(q)) if p == q && p.len() > 1900));
    let mut a = iq_graph(&set);
    let ea = run_items::<FskEnergies>(&mut a, f.clone(), &x, 333, "fsk.energies");
    let mut b = iq_graph(&set);
    let eb = run_items::<FskEnergies>(&mut b, f, &x, 4096, "fsk.energies");
    let (ea, eb) = (energies(&ea), energies(&eb));
    assert_eq!(ea, eb);
    assert_eq!(ea.len(), hard(&la).len());
    assert!(ea.iter().all(|e| e.len() == 2));
}

fn energies(d: &[Timed<FskEnergies>]) -> Vec<Vec<f32>> {
    d.iter().map(|t| t.value.energies.to_vec()).collect()
}

#[test]
fn afsk_audio_by_the_correlators_returns_the_bits() {
    // Bell 202: 1200 baud, mark (bit 1) 1200 Hz, space 2200 Hz, at 48 kHz. Map binary: the first tone listed (1200) is bit 1.
    let bits = random_bits(1500, 2);
    let mut m = Afsk::new(48_000.0, 1200.0, 1200.0, 2200.0).unwrap();
    let mut audio = Vec::new();
    m.push(&bits, &mut audio);
    m.flush(&mut audio);
    for piece in [4096usize, 1000] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(
                    Format::new(Kind::Real, 48_000.0),
                    Data::Real(audio.clone()),
                    piece,
                ),
            )
            .add("fsk", FskDemod::default())
            .add("sink", NullSink::default())
            .connect("src", "fsk")
            .connect("fsk.out", "sink")
            .set("fsk.tones_hz", txt("1200, 2200"))
            .set("fsk.map", txt("binary"))
            .set("fsk.baud", Value::F64(1200.0))
            .build()
            .unwrap();
        let got = hard(&run_passes(&mut g, audio.len() / piece + 2, "fsk.out"));
        assert!(
            contains(&got, &bits, 100, 10, 120),
            "piece {piece}: bits not found"
        );
    }
}

#[test]
fn four_level_fsk_by_the_discriminator_returns_the_c4fm_dibits() {
    // 4800 symbols/s, levels -3, -1, +1, +3 at 600 Hz per unit level (outer deviation 1800 Hz), rectangular pulse, at 48 kHz.
    let bits = random_bits(4000, 3);
    // C4FM (TIA-102.BAAA-A table 3): 01 -> +3, 00 -> +1, 10 -> -1, 11 -> -3.
    let level = |d: u8| match d {
        0b01 => 3.0,
        0b00 => 1.0,
        0b10 => -1.0,
        _ => -3.0f32,
    };
    let a: Vec<f32> = bits.chunks(2).map(|p| level(p[0] << 1 | p[1])).collect();
    let mut m = Cpm::with_deviation(Shape::Rect, 1.0, 600.0, 4800.0, 48_000.0).unwrap();
    let mut x = Vec::new();
    m.push(&a, &mut x);
    m.flush(&mut x);
    Awgn::with_power(0.01, 5).unwrap().process(&mut x);
    let set = [
        ("detector", Value::Text("discriminator".into())),
        ("levels", Value::F64(4.0)),
        ("map", Value::Text("c4fm".into())),
        ("deviation_hz", Value::F64(1800.0)),
        ("baud", Value::F64(4800.0)),
    ];
    let f = Format::iq(48_000.0, 0.0, 0.0);
    let mut g = iq_graph(&set);
    let got = hard(&run_iq(&mut g, f.clone(), &x, 4096, "fsk.out"));
    assert!(contains(&got, &bits, 400, 20, 440), "dibits not found");
    // The levels output carries one level per symbol; any cut produces the same.
    let mut a1 = iq_graph(&set);
    let la = run_iq(&mut a1, f.clone(), &x, 999, "fsk.levels");
    let mut b1 = iq_graph(&set);
    let lb = run_iq(&mut b1, f, &x, 4096, "fsk.levels");
    assert!(
        matches!((&la, &lb), (Data::Real(p), Data::Real(q)) if p == q && p.len() * 2 == got.len())
    );
}

#[test]
fn settings_are_strict() {
    let mut d = FskDemod::default();
    assert_eq!(
        set(&mut d, "levels", &Value::F64(4.0)),
        Ok(Applied::Reformat)
    );
    assert!(set(&mut d, "levels", &Value::F64(3.0)).is_err());
    assert!(set(&mut d, "levels", &Value::F64(2.5)).is_err());
    assert!(set(&mut d, "detector", &Value::Text("pll".into())).is_err());
    assert!(set(&mut d, "map", &Value::Text("dibit".into())).is_err());
    assert!(set(&mut d, "tones_hz", &Value::Text("1200, loud".into())).is_err());
    assert!(set(&mut d, "baud", &Value::F64(0.0)).is_err());
    assert!(set(&mut d, "baud", &Value::Text("fast".into())).is_err());
    assert!(set(&mut d, "llr_scale", &Value::F64(500.0)).is_err());
    assert!(set(&mut d, "colour", &Value::Bool(true)).is_err());
    let correlator = Value::Text("correlator".into());
    assert!(set(&mut FskDemod::on_real(), "detector", &correlator).is_err());
    assert!(FskDemod::on_real()
        .values()
        .to_json()
        .get("detector")
        .is_none());
    // Settings that do not fit together are refused when the graph configures.
    let bad = |set: &[(&str, Value)]| {
        let mut g = iq_graph(set);
        g.configure_input(Format::iq(12_000.0, 0.0, 0.0)).is_err()
    };
    assert!(bad(&[(
        "tones_hz",
        Value::Text("100, 200, 300, 400".into())
    )]));
    assert!(bad(&[("map", Value::Text("c4fm".into()))]));
    assert!(bad(&[("baud", Value::F64(10_000.0))]));
    assert!(bad(&[("tones_hz", Value::Text("100, 7000".into()))]));
    assert!(!bad(&[]));
    let tones = d.info().settings.id("tones_hz").unwrap();
    assert!(d.values().value(tones).is_some());
}
