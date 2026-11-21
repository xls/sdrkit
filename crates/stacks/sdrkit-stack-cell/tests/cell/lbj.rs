//! LBJ: the recording against its expected results, and test signal round trips.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_INVERTED, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::{Complex32, Generator, InputInfo, Owner, Receiver};
use sdrkit_dev_test::Harness;
use sdrkit_graph::StackGraph;
use sdrkit_stack_cell::lbj::gen::GEN_DEVIATION_HZ;
use sdrkit_stack_cell::lbj::gen::{basic_text, LbjGenOptions, LbjGenerator, TxPage};
use sdrkit_stack_cell::lbj::message::DETAIL_TEST_TEXT;
use sdrkit_stack_cell::LBJ;
use serde_json::Value as J;

const CENTRE: f64 = 821.2875e6;

fn input(rate: f64) -> InputInfo {
    InputInfo::new(rate, CENTRE, CENTRE - 50e3).recording()
}

/// What the message decoder holds: `messages`, `log` and `counters`.
fn held(s: &J) -> &J {
    data(s, "messages")
}

/// The messages of a saved state, newest first.
fn messages(s: &J) -> Vec<J> {
    let mut m = entries(&held(s)["messages"]);
    m.reverse();
    m
}

#[test]
fn recording() {
    let Some((rec, exp)) = vector("lbj_256k") else {
        return;
    };
    let mut rx = stack_graph(&LBJ, &[("locomotives", J::String(locomotives_text()))]);
    let s = run_graph(&mut rx, input(rec.rate_hz), &rec.samples, &[]);
    let msgs = messages(&s);
    let want = exp["messages_newest_first"].as_array().unwrap();
    assert_eq!(msgs.len(), want.len(), "{s:#}");
    for (m, w) in msgs.iter().zip(want) {
        for (k, v) in w.as_object().unwrap() {
            match k.as_str() {
                "t" => assert!(
                    near(&m["t"], v.as_f64().unwrap(), w["t_tol"].as_f64().unwrap()),
                    "t {}",
                    m["t"]
                ),
                "t_tol" => {}
                "position" => {
                    let tol = v["tol"].as_f64().unwrap();
                    assert_eq!(m["position"]["valid"], v["valid"]);
                    assert!(near(
                        &m["position"]["latitude"],
                        v["latitude"].as_f64().unwrap(),
                        tol
                    ));
                    assert!(near(
                        &m["position"]["longitude"],
                        v["longitude"].as_f64().unwrap(),
                        tol
                    ));
                }
                _ => assert_eq!(&m[k], v, "{k} of {m:#}"),
            }
        }
    }
    for (k, v) in exp["counters"].as_object().unwrap() {
        assert_eq!(&held(&s)["counters"][k], v, "{k}");
    }
    let offset = value(rx.results(), "offset_hz").expect("an offset");
    let (want_off, tol) = (
        exp["carrier_offset_hz"].as_f64().unwrap(),
        exp["carrier_offset_tol"].as_f64().unwrap(),
    );
    assert!((offset - want_off).abs() <= tol, "{offset}");
    // The same messages whatever the cuts of the input.
    let mut rx2 = stack_graph(&LBJ, &[("locomotives", J::String(locomotives_text()))]);
    let s2 = run_graph(
        &mut rx2,
        input(rec.rate_hz),
        &rec.samples,
        &random_pieces(7, rec.samples.len()),
    );
    assert_same(&s, &s2);
}

/// The trains of a saved state, oldest first: train, speed, kilometre post and direction.
fn trains(s: &J) -> Vec<(String, String, String, String)> {
    let text = |m: &J, k: &str| m[k].as_str().unwrap_or("").to_string();
    entries(&held(s)["messages"])
        .iter()
        .map(|m| {
            (
                text(m, "train"),
                text(m, "speed_kmh"),
                text(m, "km_post"),
                text(m, "direction"),
            )
        })
        .collect()
}

fn demo_expected() -> Vec<(String, String, String, String)> {
    [
        ("19533", "67", "38.2", "down"),
        ("10482", "112", "1204.6", "up"),
        ("23017", "0", "5.0", "down"),
        ("41206", "86", "311.9", "up"),
    ]
    .iter()
    .map(|t| (t.0.into(), t.1.into(), t.2.into(), t.3.into()))
    .collect()
}

/// The demo loop at `rate` through the standard impairments of `h` (four bursts in 6 s), received with the dial `dial_error` Hz
/// off.
fn demo_case(h: Harness, rate: f64, dial_error: f64) -> Vec<(String, String, String, String)> {
    let x = h
        .rate(rate)
        .freq(CENTRE)
        .seconds(6.0)
        .generate()
        .expect("the test signal");
    let mut rx: StackGraph = stack_graph(&LBJ, &[]);
    let s = run_graph(
        &mut rx,
        InputInfo::new(rate, CENTRE, CENTRE - 50e3 + dial_error).recording(),
        &x,
        &[],
    );
    trains(&s)
}

/// The demo loop with noise for `snr_db` in 25 kHz and the carrier moved by `offset_hz`.
fn noisy(snr_db: f64, offset_hz: f64, seed: u64) -> Harness {
    Harness::new(&LBJ)
        .seed(seed)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .set(&GEN_OFFSET_HZ, offset_hz)
}

#[test]
fn demo_loop_clean_and_noisy() {
    assert_eq!(demo_case(Harness::new(&LBJ), 960e3, 0.0), demo_expected());
    assert_eq!(
        demo_case(noisy(15.0, 1200.0, 3), 960e3, 0.0),
        demo_expected()
    );
    // Noise alone: nothing.
    let mut n = sdrkit_algo_sim::rng::Rng::new(9);
    let x: Vec<Complex32> = (0..960_000 * 3)
        .map(|_| n.complex_normal(2.0 * 0.1 * 0.1))
        .collect();
    let mut rx = stack_graph(&LBJ, &[]);
    let s = run_graph(&mut rx, input(960e3), &x, &[]);
    assert!(messages(&s).is_empty());
}

#[test]
fn impairments() {
    for snr in [4.0, 8.0, 20.0, 50.0] {
        assert_eq!(
            demo_case(noisy(snr, 0.0, snr as u64), 960e3, 0.0),
            demo_expected(),
            "SNR {snr}"
        );
    }
    for off in [-3500.0, -2000.0, -700.0, 700.0, 2000.0, 3500.0] {
        // At 3.5 kHz off, the outer tone (7.7 kHz) is on the skirt of the 6 kHz channel filter, so these cases get 13 dB.
        let snr = if f64::abs(off) > 3000.0 { 13.0 } else { 10.0 };
        assert_eq!(
            demo_case(noisy(snr, off, 1), 960e3, 0.0),
            demo_expected(),
            "offset {off}"
        );
    }
    for err in [-2000.0, 2000.0] {
        assert_eq!(
            demo_case(Harness::new(&LBJ), 960e3, err),
            demo_expected(),
            "dial error {err}"
        );
    }
    for dev in [3200.0, 3700.0, 4800.0, 5000.0] {
        let h = Harness::new(&LBJ)
            .set(&GEN_DEVIATION_HZ, dev)
            .set(&GEN_OFFSET_HZ, 1000.0);
        assert_eq!(demo_case(h, 960e3, 0.0), demo_expected(), "deviation {dev}");
    }
    let inverted = Harness::new(&LBJ).set(&GEN_INVERTED, true);
    assert_eq!(demo_case(inverted, 960e3, 0.0), demo_expected());
    for rate in [1.92e6, 2.4e6] {
        assert_eq!(
            demo_case(Harness::new(&LBJ), rate, 0.0),
            demo_expected(),
            "rate {rate}"
        );
    }
    let worst = noisy(6.0, 2500.0, 11).set(&GEN_DEVIATION_HZ, 3500.0);
    assert_eq!(demo_case(worst, 960e3, 500.0), demo_expected());
}

#[test]
fn detail_message_pairs() {
    let burst = vec![
        TxPage {
            ric: 1234000,
            function: 3,
            text: basic_text("123", 23, 123),
        },
        TxPage {
            ric: 1234002,
            function: 3,
            text: DETAIL_TEST_TEXT.into(),
        },
    ];
    let opt = LbjGenOptions {
        repeat: false,
        ..Default::default()
    };
    let mut g = LbjGenerator::new(960e3, vec![burst], opt).expect("a generator");
    let mut x = vec![Complex32::new(0.0, 0.0); 960_000 * 3];
    let n = g.generate(&mut x);
    x.truncate(n);
    let mut rx = stack_graph(&LBJ, &[("locomotives", J::String(locomotives_text()))]);
    let s = run_graph(&mut rx, input(960e3), &x, &[]);
    let m = &messages(&s)[0];
    assert_eq!(m["train"], "K123");
    assert_eq!(m["locomotive"], "HXD3C-0931");
    assert_eq!(m["route"], "津山线");
    assert_eq!(m["paired"], true);
    // A change of frequency keeps the messages and forgets the offset of the newest.
    let n = table_len(rx.results(), "messages");
    assert!(n > 0 && value(rx.results(), "offset_hz").is_some());
    rx.forget();
    assert_eq!(table_len(rx.results(), "messages"), n);
    assert_eq!(value(rx.results(), "offset_hz"), None);
}

#[test]
fn rate_below_minimum_is_refused() {
    let mut rx = stack_graph(&LBJ, &[]);
    assert!(rx.configure(&input(200e3)).is_err());
    assert_eq!(
        Owner::results(&rx).status().summary,
        "LBJ needs a sample rate of at least 256 ksps"
    );
}
