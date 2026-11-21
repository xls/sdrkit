//! `fec.quantize`: exact values against the plain quantizer, fixed and adaptive scales, the scale in the format, any cut of the
//! stream, and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::Quantizer;
use sdrkit_components::fec::Quantize;
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Applied, Format, Kind};

fn soft(n: usize) -> Vec<f32> {
    let mut rng = Rng(11);
    (0..n)
        .map(|i| if i % 3 == 0 { -1.0 } else { 1.0 } * 2.5 + rng.gauss())
        .collect()
}

fn quantize(x: &[f32], per_pass: usize, settings: &[(&str, Value)]) -> (Vec<i8>, Format) {
    let mut b = Tb::new()
        .add(
            "src",
            Source::new(
                Format::new(Kind::Real, 9600.0),
                Data::Real(x.to_vec()),
                per_pass,
            ),
        )
        .add("q", Quantize::default())
        .connect("src", "q");
    for (k, v) in settings {
        b = b.set(&format!("q.{k}"), v.clone());
    }
    let mut g = b.build().unwrap();
    let got = run(&mut g, x.len() / per_pass + 2, &["q.out"]);
    (llrs(&got[0].data), g.format("q.out").unwrap().clone())
}

#[test]
fn fixed_scale_returns_round_of_value_times_scale_and_states_it() {
    let x = soft(1000);
    let (out, fmt) = quantize(
        &x,
        100,
        &[("scale", Value::F64(6.0)), ("limit", Value::F64(40.0))],
    );
    assert_eq!(fmt.kind, Kind::Llr);
    assert_eq!(fmt.llr_scale, Some(6.0));
    assert_eq!(fmt.rate, 9600.0);
    let want: Vec<i8> = x
        .iter()
        .map(|v| (v * 6.0).round().clamp(-40.0, 40.0) as i8)
        .collect();
    assert_eq!(out, want);
    // Signs (hard decisions) are kept.
    assert!(x
        .iter()
        .zip(&out)
        .all(|(v, &o)| o == 0 || (o < 0) == (*v < 0.0)));
}

#[test]
fn adaptive_scale_matches_the_plain_quantizer_window_by_window_for_any_cut() {
    let x = soft(4096 + 100);
    let q = Quantizer {
        target: 24.0,
        ..Quantizer::default()
    };
    let mut want = vec![0i8; 4096];
    for (xs, os) in x[..4096].chunks(512).zip(want.chunks_mut(512)) {
        q.quantize_auto(xs, os).unwrap();
    }
    let settings = [("target", Value::F64(24.0)), ("window", Value::F64(512.0))];
    for per_pass in [1usize, 7, 512, 1000, 5000] {
        let (out, fmt) = quantize(&x, per_pass, &settings);
        assert_eq!(fmt.llr_scale, None);
        // The last 100 values wait for the rest of their window.
        assert_eq!(out, want, "pieces of {per_pass}");
    }
}

#[test]
fn negate_and_values_that_are_not_numbers() {
    let mut x = soft(64);
    x[5] = f32::NAN;
    x[9] = f32::INFINITY;
    let (out, _) = quantize(
        &x,
        64,
        &[("scale", Value::F64(10.0)), ("negate", Value::Bool(true))],
    );
    assert_eq!(out[5], 0);
    assert_eq!(out[9], -127);
    assert_eq!(out[0], -((x[0] * 10.0).round() as i8));
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Real, 4.0), Data::Real(x), 64),
        )
        .add("q", Quantize::default())
        .connect("src", "q")
        .set("q.scale", Value::F64(10.0))
        .build()
        .unwrap();
    run(&mut g, 2, &["q.out"]);
    assert_eq!(field(&g, "q", "bad_samples"), 1.0);
    assert_eq!(field(&g, "q", "llr_scale"), 10.0);
}

#[test]
fn settings_are_strict() {
    let mut q = Quantize::default();
    assert!(set(&mut q, "scale", &Value::F64(-1.0)).is_err());
    assert!(set(&mut q, "scale", &Value::Text("big".into())).is_err());
    assert!(set(&mut q, "limit", &Value::F64(128.0)).is_err());
    assert!(set(&mut q, "limit", &Value::F64(12.5)).is_err());
    assert!(set(&mut q, "target", &Value::F64(0.0)).is_err());
    assert!(set(&mut q, "window", &Value::F64(8.0)).is_err());
    assert!(set(&mut q, "negate", &Value::Text("maybe".into())).is_err());
    assert!(set(&mut q, "colour", &Value::Bool(true)).is_err());
    assert_eq!(
        set(&mut q, "scale", &Value::F64(4.0)),
        Ok(Applied::Reformat)
    );
    assert_eq!(set(&mut q, "limit", &Value::F64(31.0)), Ok(Applied::Live));
    let g = Tb::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Real, 4.0), Data::Real(vec![]), 1),
        )
        .add("q", Quantize::default())
        .connect("src", "q")
        .set("q.negate", txt("maybe"))
        .build();
    assert!(g.is_err());
}
