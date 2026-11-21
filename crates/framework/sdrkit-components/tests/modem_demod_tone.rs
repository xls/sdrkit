//! `modem.tone_detect`: two tones together (the DTMF pair of key 5, 770 and 1336 Hz, ITU-T Q.23) and then one tone, in noise,
//! found with their start and stop times; cuts; a named plan from `sdrkit-util-ids` with its labels; strict settings.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_components::modem::demod::{ToneDetect, ToneEvent};
use sdrkit_components::testing::Source;
use sdrkit_core::Value;
use sdrkit_graph::stream::Data;
use sdrkit_graph::{components::NullSink, Applied, Format, Graph, Kind};

#[path = "modem_demod_util.rs"]
mod util;
use util::run_items_passes;
#[allow(unused_imports)]
use util::set_util::{set, txt, Tb};

const RATE: f64 = 8000.0;

/// 0.1 s quiet, 0.2 s of 770 + 1336 Hz, 0.1 s quiet, 0.2 s of 1000 Hz, 0.1 s quiet; amplitude 0.3 each, noise 0.01.
fn audio() -> Vec<f32> {
    let mut rng = Rng::new(3);
    let tone = |f: f64, n: usize| (std::f64::consts::TAU * f * n as f64 / RATE).sin() as f32 * 0.3;
    (0..5600)
        .map(|n| {
            let t = n as f64 / RATE;
            let s = if (0.1..0.3).contains(&t) {
                tone(770.0, n) + tone(1336.0, n)
            } else if (0.4..0.6).contains(&t) {
                tone(1000.0, n)
            } else {
                0.0
            };
            s + 0.01 * rng.normal_f32()
        })
        .collect()
}

fn graph(x: &[f32], per_pass: usize, set: &[(&str, Value)]) -> Graph {
    let mut b = Tb::new()
        .add(
            "src",
            Source::new(
                Format::new(Kind::Real, RATE),
                Data::Real(x.to_vec()),
                per_pass,
            ),
        )
        .add("tones", ToneDetect::default())
        .add("sink", NullSink::default())
        .connect("src", "tones")
        .connect("tones", "sink")
        .set("tones.tones_hz", txt("770, 1336, 1000"))
        .set("tones.block_s", Value::F64(0.01));
    for (k, v) in set {
        b = b.set(&format!("tones.{k}"), v.clone());
    }
    b.build().unwrap()
}

#[test]
fn tones_start_and_stop_at_their_times() {
    let x = audio();
    let mut first: Option<Vec<(u32, bool, f64)>> = None;
    for per_pass in [5600usize, 77, 1] {
        let mut g = graph(&x, per_pass, &[]);
        let ev = run_items_passes::<ToneEvent>(&mut g, 5600 / per_pass + 2, "tones.out");
        let got: Vec<(u32, bool, f64)> = ev
            .iter()
            .map(|t| (t.value.tone, t.value.on, t.time))
            .collect();
        // Blocks of 10 ms: each change is seen in the block where it happens or the next.
        let want = [
            (0, true, 0.1),
            (1, true, 0.1),
            (0, false, 0.3),
            (1, false, 0.3),
            (2, true, 0.4),
            (2, false, 0.6),
        ];
        assert_eq!(got.len(), want.len(), "{got:?}");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!((g.0, g.1), (w.0, w.1), "{got:?}");
            assert!((g.2 - w.2).abs() <= 0.0101, "{got:?}");
        }
        assert_eq!(ev[0].value.hz, 770.0);
        // 0.3 amplitude: about -10.5 dB relative to a full-scale sine.
        assert!(
            (ev[0].value.level_db + 10.5).abs() < 1.0,
            "{}",
            ev[0].value.level_db
        );
        match &first {
            None => first = Some(got),
            Some(f) => assert_eq!(&got, f, "per pass {per_pass}"),
        }
    }
}

#[test]
fn settings_are_strict() {
    let mut t = ToneDetect::default();
    assert_eq!(
        set(&mut t, "plan", &Value::Text("list".into())),
        Ok(Applied::Reformat)
    );
    assert_eq!(
        set(&mut t, "plan", &Value::Text("dtmf".into())),
        Ok(Applied::Reformat)
    );
    assert_eq!(
        set(&mut t, "plan", &Value::Text("selcall_zvei1".into())),
        Ok(Applied::Reformat)
    );
    // DCS is a bit stream, not a tone plan.
    assert!(set(&mut t, "plan", &Value::Text("dcs".into())).is_err());
    assert!(set(&mut t, "plan", &Value::Text("loud".into())).is_err());
    assert!(set(&mut t, "tones_hz", &Value::Text("770, loud".into())).is_err());
    assert!(set(&mut t, "block_s", &Value::F64(0.0)).is_err());
    assert!(set(&mut t, "threshold_db", &Value::F64(3.0)).is_err());
    assert!(set(&mut t, "min_blocks", &Value::F64(0.0)).is_err());
    assert!(set(&mut t, "colour", &Value::Bool(true)).is_err());
    let x = vec![0f32; 10];
    // A tone above half the rate, or no tones at all, is refused when the graph configures.
    let mut g = graph(&x, 10, &[("tones_hz", Value::Text("5000".into()))]);
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
    let mut g = graph(&x, 10, &[("tones_hz", Value::Text("".into()))]);
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
}

#[test]
fn a_named_plan_takes_its_tones_and_labels_from_the_tables() {
    // The DTMF plan: rows 697 770 852 941, then columns 1209 1336 1477 1633; key 5 is 770 + 1336 Hz.
    let x = audio();
    // 40 ms blocks: the 1000 Hz tone, 59 Hz from 941 Hz, would show in 941's bin with 10 ms blocks (100 Hz resolution).
    let mut g = graph(
        &x,
        5600,
        &[
            ("plan", Value::Text("dtmf".into())),
            ("block_s", Value::F64(0.04)),
        ],
    );
    let ev = run_items_passes::<ToneEvent>(&mut g, 3, "tones.out");
    let got: Vec<(u32, &str, bool)> = ev
        .iter()
        .map(|t| (t.value.tone, t.value.label, t.value.on))
        .collect();
    assert_eq!(
        got,
        [
            (1, "770", true),
            (5, "1336", true),
            (1, "770", false),
            (5, "1336", false)
        ]
    );
    assert_eq!(ev[1].value.hz, 1336.0);
}
