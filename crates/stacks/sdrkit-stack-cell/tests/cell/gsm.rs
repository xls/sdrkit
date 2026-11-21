//! GSM-R: the recording against its expected results, test signal round trips, and the published results.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::{Complex32, InputInfo, LockState, Owner};
use sdrkit_dev_test::Harness;
use sdrkit_stack_cell::gsm::gen::{scene as gen_scene, GsmGenerator};
use sdrkit_stack_cell::gsm::si::type_name;
use sdrkit_stack_cell::gsm::{GsmScene, GEN_SCENE};
use sdrkit_stack_cell::GSMR;
use serde_json::Value as J;

/// The radio centre of the test scenes.
const CENTRE_HZ: f64 = 922.8e6;

#[test]
fn recording() {
    let Some((rec, exp)) = vector("gsmr_1cell_1000k") else {
        return;
    };
    assert_eq!(exp["mode"], GSMR.id);
    assert_eq!(exp["center_hz"].as_f64(), Some(rec.centre_hz));
    let input = InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording();
    let mut rx = stack_graph(&GSMR, &[]);
    let s = run_graph(&mut rx, input, &rec.samples, &[]);
    let cs = cells(&s);
    let want = exp["cells"].as_array().expect("expected cells");
    assert_eq!(want.len(), 1);
    assert_eq!(
        cs.len(),
        want.len() + exp["other_cells"].as_u64().unwrap() as usize,
        "{cs:?}"
    );
    let c = &cs[0];
    let w = &want[0];
    for k in [
        "arfcn",
        "frequency_hz",
        "railway_band",
        "bsic",
        "ncc",
        "bcc",
        "lai",
        "si3",
        "si1_cell_allocation",
        "si2_neighbours",
        "ncc_permitted",
        "si_failed",
    ] {
        assert_eq!(&c[k], &w[k], "{k}");
    }
    let types: Vec<J> = c["type_codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| J::from(type_name(t.as_u64().unwrap() as u8)))
        .collect();
    assert_eq!(J::from(types), w["types_seen"]);
    let offset = w["carrier_offset_hz"].as_f64().unwrap();
    let tol = w["carrier_offset_tol"].as_f64().unwrap();
    assert!(
        near(&c["carrier_offset_hz"], offset, tol),
        "{}",
        c["carrier_offset_hz"]
    );
    assert!(c["si_messages"].as_u64().unwrap() >= w["si_messages_min"].as_u64().unwrap());
    assert!(c["snr_db"].as_f64().unwrap() >= w["snr_db_min"].as_f64().unwrap());
    let blocks = exp["blocks_searched"].as_i64().unwrap();
    assert_eq!(value(rx.results(), "captures"), Some(blocks as f64));
    // The same table whatever the cuts of the input.
    let mut rx2 = stack_graph(&GSMR, &[]);
    let s2 = run_graph(
        &mut rx2,
        input,
        &rec.samples,
        &random_pieces(5, rec.samples.len()),
    );
    assert_same(&s, &s2);
}

/// The scene `scene` at `rate` for `seconds`, with noise for `snr_db` and the whole signal moved by `offset_hz`.
fn gen(rate: f64, scene: GsmScene, offset_hz: f64, snr_db: f64, seconds: f64) -> Vec<Complex32> {
    Harness::new(&GSMR)
        .rate(rate)
        .freq(CENTRE_HZ)
        .seconds(seconds)
        .seed(3)
        .set(&GEN_SCENE, scene)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .generate()
        .expect("the test signal")
}

/// The cells found in `x`, most recently seen first.
fn found(rate: f64, x: &[Complex32]) -> Vec<J> {
    let mut rx = stack_graph(&GSMR, &[]);
    let input = InputInfo::new(rate, CENTRE_HZ, CENTRE_HZ).recording();
    cells(&run_graph(&mut rx, input, x, &[]))
}

#[test]
fn two_cells() {
    let x = gen(5e6, GsmScene::TwoCells, 8e3, 20.0, 2.3);
    let cs = found(5e6, &x);
    for (arfcn, bsic, cell) in [(960, 10, 12001), (968, 13, 12002)] {
        let c = cs
            .iter()
            .find(|c| c["arfcn"] == arfcn)
            .unwrap_or_else(|| panic!("ARFCN {arfcn}: {cs:?}"));
        assert_eq!(c["bsic"], bsic);
        assert_eq!(c["lai"]["lac"], 4711);
        assert_eq!(c["si3"]["cell_id"], cell);
        assert_eq!(
            c["si1_cell_allocation"],
            serde_json::json!([arfcn - 2, arfcn, arfcn + 2])
        );
        assert!(
            near(&c["carrier_offset_hz"], 8000.0, 400.0),
            "{}",
            c["carrier_offset_hz"]
        );
        assert_eq!(c["seen"]["count"], 1);
    }
    assert_eq!(cs.len(), 2);
}

#[test]
fn band_edge() {
    let x = gen(5e6, GsmScene::BandEdge, -8e3, 12.0, 2.3);
    let cs = found(5e6, &x);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["arfcn"], 955);
    assert_eq!(cs[0]["si3"]["cell_id"], 777);
}

#[test]
fn three_cells() {
    // A 2.4 Msps radio sees only ARFCN 964.
    let x = gen(2.4e6, GsmScene::ThreeCells, 0.0, 25.0, 2.3);
    let cs = found(2.4e6, &x);
    assert_eq!(cs.len(), 1, "{cs:?}");
    assert_eq!(cs[0]["arfcn"], 964);
    assert_eq!(cs[0]["si3"]["cell_id"], 20002);
    // All three at 5 Msps: the two strong ones identified, nothing wrong.
    let x = gen(5e6, GsmScene::ThreeCells, 0.0, 25.0, 2.3);
    let cs = found(5e6, &x);
    for (a, id) in [(957, 20001), (964, 20002)] {
        let c = cs.iter().find(|c| c["arfcn"] == a).expect("cell");
        assert_eq!(c["si3"]["cell_id"], id);
    }
    for c in &cs {
        assert!([957, 964, 971].contains(&c["arfcn"].as_i64().unwrap()));
    }
}

#[test]
fn noise_only() {
    let mut n = sdrkit_algo_sim::rng::Rng::new(1);
    let x: Vec<Complex32> = (0..(1e6 * 2.3) as usize)
        .map(|_| n.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    assert!(found(1e6, &x).is_empty());
}

/// Without the radio centre, a cell is keyed by its BSIC and its offset on the 200 kHz raster, and has no ARFCN.
#[test]
fn unknown_centre() {
    let x = gen(5e6, GsmScene::BandEdge, 0.0, 30.0, 2.3);
    let mut rx = stack_graph(&GSMR, &[]);
    let s = run_graph(&mut rx, InputInfo::new(5e6, 0.0, 0.0).recording(), &x, &[]);
    let cs = cells(&s);
    assert_eq!(cs.len(), 1, "{cs:?}");
    // ARFCN 955 (921.2 MHz) lies 1.6 MHz below the radio centre: raster step -8.
    assert_eq!(cs[0]["arfcn"], -1);
    assert_eq!(cs[0]["key"]["channel"]["raster"], -8);
    assert!(
        near(&cs[0]["frequency_hz"], -1.6e6, 1e3),
        "{}",
        cs[0]["frequency_hz"]
    );
    assert_eq!(cs[0]["si3"]["cell_id"], 777);
}

/// The generator makes the clean signal at unit mean power, and states its bandwidth and symbol rate.
#[test]
fn generator_is_clean_at_unit_power() {
    use sdrkit_core::Generator;
    for scene in 0..3 {
        let mut g = GsmGenerator::new(5e6, CENTRE_HZ, gen_scene(scene));
        let mut x = vec![Complex32::default(); 5_000_000];
        assert_eq!(g.generate(&mut x), x.len());
        // After the quiet lead of the latest carrier.
        let p = x[50_000..].iter().map(|v| v.norm_sqr() as f64).sum::<f64>()
            / (x.len() - 50_000) as f64;
        assert!((p - 1.0).abs() < 0.02, "scene {scene}: {p}");
        let info = g.signal_info().expect("signal info");
        assert_eq!(info.bandwidth_hz, 200e3 * gen_scene(scene).len() as f64);
        assert_eq!(info.symbol_rate_hz, Some(1625e3 / 6.0));
    }
}

/// The receiver publishes the cells and the carriers of the test signal, with its counts.
#[test]
fn published_results() {
    let run = Harness::new(&GSMR).freq(CENTRE_HZ).seconds(2.3).run().expect("a run");
    let r = &run.results;
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(
        r.status().summary,
        "2 cells, 2 identified, 2 in the railway band"
    );
    assert_eq!(table_len(r, "cells"), 2);
    assert!(table_len(r, "carriers") >= 2);
    assert_eq!(value(r, "cell_count"), Some(2.0));
    assert_eq!(value(r, "railway_cells"), Some(2.0));
}
