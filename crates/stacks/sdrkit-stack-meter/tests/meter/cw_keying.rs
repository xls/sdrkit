//! The CW keying meter: six keyers of speed, dah ratio, weight, jitter, edges, chirp and offset; a sweep of the noise; noise
//! alone and a steady carrier.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::cw_keying::gen::{
    marks, GEN_CHIRP_HZ, GEN_DAH_RATIO, GEN_JITTER, GEN_RAMP_S, GEN_TEXT, GEN_WEIGHT, GEN_WPM,
};
use sdrkit_stack_meter::cw_keying::DESCRIPTOR;
use sdrkit_stack_meter::typed::{CwKeyingEventRow, CwKeyingFields, CW_KEYING_EVENTS};

const TUNED: f64 = 7.03e6;

/// The keyer of a case: wpm, dah ratio, weight, jitter, ramp s, chirp Hz, offset Hz, drift Hz/s.
type Keyer = (f64, f64, f64, f64, f64, f64, f64, f64);

fn keyer(cnr: f64, k: Keyer) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
        .set(&GEN_WPM, k.0)
        .set(&GEN_DAH_RATIO, k.1)
        .set(&GEN_WEIGHT, k.2)
        .set(&GEN_JITTER, k.3)
        .set(&GEN_RAMP_S, k.4)
        .set(&GEN_CHIRP_HZ, k.5)
        .set(&GEN_OFFSET_HZ, k.6)
        .set(&GEN_DRIFT_HZ_S, k.7)
}

/// The run of `h` until 3 s after the last mark, at 48 ksps.
fn run(h: Harness, seed: u64) -> CwKeyingFields {
    let h = h.seed(seed);
    let v = h.values().unwrap();
    let end = marks(&v, &mut Rng::stream(seed, 47))
        .last()
        .map_or(10.0, |m| m.1)
        + 3.0;
    let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, end, seed, |_, _| {});
    CwKeyingFields::read(r.results.fields()).unwrap()
}

#[test]
fn six_keyers_read_speed_ratio_edges_scatter_chirp_and_weight() {
    let cases: [Keyer; 6] = [
        (20.0, 3.0, 1.0, 0.0, 0.006, 0.0, 0.0, 0.0),
        (12.0, 3.0, 1.0, 0.0, 0.010, 0.0, 400.0, 2.0),
        (30.0, 3.0, 1.0, 0.0, 0.004, 0.0, -300.0, 0.0),
        (20.0, 3.3, 1.15, 0.0, 0.006, 0.0, 100.0, 0.0),
        (20.0, 3.0, 1.0, 0.12, 0.006, 0.0, 0.0, 0.0),
        (20.0, 3.0, 1.0, 0.0, 0.006, 90.0, 0.0, 0.0),
    ];
    for (i, k) in cases.into_iter().enumerate() {
        let f = run(keyer(40.0, k), 1 + i as u64);
        let (w, r, rise, fall, sc, ch, wt) = (
            f.wpm.unwrap(),
            f.dah_ratio.unwrap(),
            f.rise_ms.unwrap(),
            f.fall_ms.unwrap(),
            f.dit_scatter_pct.unwrap(),
            f.chirp_hz.unwrap_or(0.0),
            f.weight.unwrap(),
        );
        println!("case {i}: {w:.2} wpm, dah {r:.2}, rise {rise:.2} fall {fall:.2} ms, scatter {sc:.2} %, chirp {ch:.1} Hz, weight {wt:.3}");
        let edge = (0.5904 * k.4 * 1000.0).max(1.5);
        let want_w = k.2 / (2.0 - k.2);
        assert!((w - k.0).abs() <= 0.04 * k.0, "case {i}: {f:?}");
        assert!((r - k.1).abs() <= 0.2, "case {i}: {f:?}");
        assert!(
            (rise - edge).abs() <= 0.25 * edge + 0.5 && (fall - edge).abs() <= 0.25 * edge + 0.5,
            "case {i}: {f:?}"
        );
        if k.3 > 0.0 {
            assert!((4.0..=12.0).contains(&sc), "case {i}: {f:?}");
        } else {
            assert!(sc < 3.0, "case {i}: {f:?}");
        }
        if k.5 > 0.0 {
            assert!((0.3 * k.5..=1.2 * k.5).contains(&ch), "case {i}: {f:?}");
        } else {
            assert!(ch.abs() < 8.0, "case {i}: {f:?}");
        }
        assert!(
            (wt - want_w).abs() <= 0.08 * want_w + 0.03,
            "case {i}: {f:?}"
        );
    }
}

#[test]
fn the_speed_holds_down_to_20_db() {
    for (i, cnr) in [40.0, 30.0, 20.0, 15.0, 10.0, 6.0].into_iter().enumerate() {
        let f = run(
            keyer(cnr, (20.0, 3.0, 1.0, 0.0, 0.006, 0.0, 200.0, 0.0)),
            11 + i as u64,
        );
        println!(
            "{cnr} dB: {:?} wpm, scatter {:?} %",
            f.wpm, f.dit_scatter_pct
        );
        if cnr >= 20.0 {
            assert!(
                f.wpm.is_some_and(|w| (w - 20.0).abs() <= 1.0),
                "{cnr}: {f:?}"
            );
        }
    }
}

fn rows(r: &Streamed) -> Vec<CwKeyingEventRow> {
    CW_KEYING_EVENTS
        .rows::<CwKeyingEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn noise_alone_and_a_steady_carrier_give_no_row() {
    for seed in [21, 22] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -30.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 60.0, seed, |_, _| {});
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
        let h = keyer(40.0, (20.0, 3.0, 1.0, 0.0, 0.006, 0.0, 0.0, 0.0))
            .set(&GEN_TEXT, "")
            .seed(seed);
        let step = std::f64::consts::TAU * 300.0 / 48e3;
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 60.0, seed, |t, x| {
            let n0 = (t * 48e3).round();
            for (i, z) in x.iter_mut().enumerate() {
                *z += sdrkit_core::Complex32::from_polar(
                    1.0,
                    ((n0 + i as f64) * step % std::f64::consts::TAU) as f32,
                );
            }
        });
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    }
}
