//! Eurobalise: the recording against its expected results, and test signal round trips.

use crate::common::*;
use sdrkit_algo_sim::chain::GEN_INVERTED;
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_EBN0_DB, GEN_NOISE, GEN_OFFSET_HZ,
};
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver};
use sdrkit_dev_test::Harness;
use sdrkit_stack_cell::balise::gen::{BaliseScene, GEN_BIT_RATE_PCT, GEN_RANDOM_BITS, GEN_SCENE};
use sdrkit_stack_cell::EBICAB;
use serde_json::Value as J;

fn input(rate: f64) -> InputInfo {
    InputInfo::new(rate, 5.234e6, 4.234e6).recording()
}

/// What the telegram decoder holds: `telegrams`, `balises`, `other_bursts`, `log` and `counters`.
fn held(s: &J) -> &J {
    data(s, "telegrams")
}

/// The counters of a saved state.
fn counters(s: &J) -> &J {
    &held(s)["counters"]
}

/// The balise records of a saved state.
fn balises(s: &J) -> Vec<J> {
    held(s)["balises"]["rows"]
        .as_array()
        .expect("a table of balises")
        .iter()
        .map(|r| r["record"].clone())
        .collect()
}

#[test]
fn recording() {
    let Some((rec, exp)) = vector("ebicab_4000k") else {
        return;
    };
    let mut rx = stack_graph(&EBICAB, &[]);
    let s = run_graph(&mut rx, input(rec.rate_hz), &rec.samples, &[]);
    let c = counters(&s);
    assert!(
        c["telegrams"].as_u64().unwrap() >= exp["telegrams_min"].as_u64().unwrap(),
        "{c}"
    );
    assert!(c["long"].as_u64().unwrap() >= exp["long_min"].as_u64().unwrap());
    assert!(c["short"].as_u64().unwrap() >= exp["short_min"].as_u64().unwrap());
    assert_eq!(c["bursts"], exp["bursts"]);
    assert_eq!(c["bursts_without_telegram"], exp["bursts_without_telegram"]);
    assert_eq!(c["inverted"], exp["inverted"]);
    let tol = |k: &str| exp[k].as_f64().unwrap();
    assert!(
        near(&c["bit_rate_hz"], tol("bit_rate"), tol("bit_rate_tol")),
        "{}",
        c["bit_rate_hz"]
    );
    assert!(
        near(
            &c["carrier_offset_hz"],
            tol("carrier_offset_hz"),
            tol("carrier_offset_tol")
        ),
        "{}",
        c["carrier_offset_hz"]
    );
    let bal = balises(&s);
    assert_eq!(bal.len(), exp["balises"].as_array().unwrap().len());
    for w in exp["balises"].as_array().unwrap() {
        let b = bal
            .iter()
            .find(|b| b["nid_bg"] == w["nid_bg"] && b["n_pig"] == w["n_pig"])
            .expect("balise");
        for (k, v) in w.as_object().unwrap() {
            assert_eq!(&b[k], v, "{k}");
        }
    }
    let rows = entries(&held(&s)["telegrams"]);
    assert_eq!(rows.len() as u64, exp["bursts"].as_u64().unwrap());
    for r in &rows {
        let w = exp["telegram_rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["nid_bg"] == r["nid_bg"] && w["n_pig"] == r["n_pig"])
            .expect("row");
        for k in ["length", "q_updown", "m_mcount", "q_link", "first_packet"] {
            assert_eq!(r[k], w[k], "{k}");
        }
        assert!(r["user_bits_hex"]
            .as_str()
            .unwrap()
            .starts_with(w["user_bits_hex_prefix"].as_str().unwrap()));
        let copies = r["copies"].as_i64().unwrap();
        let want = w["copies_per_burst"].as_i64().unwrap();
        assert!(
            (copies - want).abs() <= exp["copies_tol"].as_i64().unwrap(),
            "copies {copies}"
        );
    }
    // The same telegrams whatever the cuts of the input.
    let mut rx2 = stack_graph(&EBICAB, &[]);
    let s2 = run_graph(
        &mut rx2,
        input(rec.rate_hz),
        &rec.samples,
        &random_pieces(3, rec.samples.len()),
    );
    assert_same(&s, &s2);
}

/// The test signal of `h` at `rate` for `seconds`, received with the up-link 1 MHz under the radio centre.
fn case(h: Harness, rate: f64, seconds: f64) -> J {
    case_at(h, rate, seconds, 4.234e6)
}

/// As [`case`], with the dial at `dial_hz`.
fn case_at(h: Harness, rate: f64, seconds: f64, dial_hz: f64) -> J {
    let x = h
        .rate(rate)
        .freq(5.234e6)
        .seconds(seconds)
        .generate()
        .expect("the test signal");
    let mut rx = stack_graph(&EBICAB, &[]);
    run_graph(
        &mut rx,
        InputInfo::new(rate, 5.234e6, dial_hz).recording(),
        &x,
        &[],
    )
}

#[test]
fn clean_long_and_short() {
    let s = case(Harness::new(&EBICAB), 10e6, 0.5);
    let c = counters(&s);
    assert!(
        c["long"].as_u64().unwrap() > 0 && c["short"].as_u64().unwrap() > 0,
        "{c}"
    );
    assert!(near(&c["bit_rate_hz"], 564480.0, 8000.0));
    assert_eq!(balises(&s).len(), 3);
}

#[test]
fn impairments() {
    let base = || {
        Harness::new(&EBICAB)
            .set(&GEN_NOISE, NoiseKind::EbN0)
            .set(&GEN_EBN0_DB, 30.0)
    };
    let cases: Vec<(&str, f64, Harness)> = vec![
        ("offset +40k", 10e6, base().set(&GEN_OFFSET_HZ, 40e3)),
        ("offset -40k", 10e6, base().set(&GEN_OFFSET_HZ, -40e3)),
        ("bit rate +2%", 10e6, base().set(&GEN_BIT_RATE_PCT, 2.0)),
        ("bit rate -2%", 10e6, base().set(&GEN_BIT_RATE_PCT, -2.0)),
        ("clock +1000 ppm", 10e6, base().set(&GEN_CLOCK_PPM, 1000.0)),
        ("drift", 10e6, base().set(&GEN_DRIFT_HZ_S, 100e3)),
        ("Eb/N0 12", 10e6, base().set(&GEN_EBN0_DB, 12.0)),
        ("4 Msps", 4e6, base()),
        ("6 Msps", 6e6, base()),
        ("20 Msps", 20e6, base()),
    ];
    for (name, rate, h) in cases {
        let s = case(h, rate, 0.15);
        let c = counters(&s);
        assert!(c["telegrams"].as_u64().unwrap() >= 3, "{name}: {c}");
        assert_eq!(c["inverted"], 0, "{name}");
    }
    // An inverted spectrum puts the up-link 1 MHz over the centre and swaps the two tones, so every telegram reads inverted.
    let s = case_at(base().set(&GEN_INVERTED, true), 10e6, 0.15, 6.234e6);
    let c = counters(&s);
    assert!(c["telegrams"].as_u64().unwrap() >= 3, "inverted: {c}");
    assert_eq!(c["inverted"], c["telegrams"]);
}

#[test]
fn random_bursts_are_other_bursts() {
    let s = case(
        Harness::new(&EBICAB).set(&GEN_SCENE, BaliseScene::Random),
        10e6,
        0.1,
    );
    assert_eq!(counters(&s)["telegrams"], 0);
    assert!(entries(&held(&s)["other_bursts"]).len() >= 3);
    let s = case(
        Harness::new(&EBICAB)
            .set(&GEN_SCENE, BaliseScene::Random)
            .set(&GEN_RANDOM_BITS, 255),
        10e6,
        0.1,
    );
    let o = &entries(&held(&s)["other_bursts"])[0];
    assert!(o["hint"].as_str().unwrap().contains("EBICab 900"), "{o}");
}

#[test]
fn noise_alone() {
    let mut n = sdrkit_algo_sim::rng::Rng::new(4);
    let x: Vec<Complex32> = (0..1_000_000)
        .map(|_| n.complex_normal(2.0 * 0.05 * 0.05))
        .collect();
    let mut rx = stack_graph(&EBICAB, &[]);
    let s = run_graph(&mut rx, input(10e6), &x, &[]);
    assert_eq!(counters(&s)["telegrams"], 0);
    assert_eq!(counters(&s)["bursts"], 0);
}

#[test]
fn rate_below_minimum_is_refused() {
    let mut rx = stack_graph(&EBICAB, &[]);
    assert!(rx.configure(&input(2e6)).is_err());
    assert_eq!(
        Owner::results(&rx).status().summary,
        "Balise: the sample rate is too low (4 Msps at least)"
    );
}
