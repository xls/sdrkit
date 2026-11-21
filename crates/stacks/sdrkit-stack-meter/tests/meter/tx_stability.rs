//! The stability meter: four cases of rate, final frequency, start offset, time constant, drift and modulation over three
//! transmissions; noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::tx_stability::gen::{
    GEN_FINAL_HZ, GEN_KEYED, GEN_MOD_HZ, GEN_START_OFFSET_HZ, GEN_TAU_S, GEN_TX_DRIFT_HZ_S,
};
use sdrkit_stack_meter::tx_stability::DESCRIPTOR;
use sdrkit_stack_meter::typed::{TxStabilityTransmissionEvent, TX_STABILITY_TRANSMISSION};

const TUNED: f64 = 146.52e6;

fn rows(r: &Streamed) -> Vec<TxStabilityTransmissionEvent> {
    r.events
        .iter()
        .filter_map(|e| TX_STABILITY_TRANSMISSION.read::<TxStabilityTransmissionEvent>(e))
        .collect()
}

type Case = (f64, f64, f64, f64, f64, f64);

#[test]
fn four_cases_read_start_final_drift_and_settling() {
    check(&[
        (48e3, 500.0, 250.0, 0.15, 0.0, 0.0),
        (48e3, -1200.0, -400.0, 0.3, 2.0, 1000.0),
        (96e3, 100.0, 120.0, 0.08, -1.5, 0.0),
        (250e3, 2000.0, 600.0, 0.5, 0.5, 800.0),
    ]);
}

/// The settling time that the definition of the meter gives for the frequency law of the generator over a transmission of
/// `len` s: the last time from key-up at which the law is outside the band around its mean over the final stretch, plus 0.05 s.
fn settle_of(fe: f64, so: f64, tau: f64, drift: f64, len: f64) -> f64 {
    let f = |u: f64| fe + so * (-u / tau).exp() + drift * u;
    let mean = |a: f64, b: f64| {
        let n = 1000;
        (0..n)
            .map(|i| f(a + (b - a) * (i as f64 + 0.5) / n as f64))
            .sum::<f64>()
            / n as f64
    };
    let (start, last) = (mean(0.05, 0.2), mean(len - 0.4, len - 0.05));
    let band = 10f64.max(0.1 * (start - last).abs());
    (0..=((len - 0.05) * 1e3) as usize)
        .map(|i| i as f64 * 1e-3)
        .rfind(|&u| (f(u) - last).abs() > band)
        .map_or(0.0, |u| u + 0.05)
}

/// Runs `cases`.
fn check(cases: &[Case]) {
    for (k, &(rate, fe, so, tau, drift, md)) in cases.iter().enumerate() {
        let seed = 1 + k as u64 + (rate / 1e3) as u64;
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 35.0)
            .set(&GEN_FINAL_HZ, fe)
            .set(&GEN_START_OFFSET_HZ, so)
            .set(&GEN_TAU_S, tau)
            .set(&GEN_TX_DRIFT_HZ_S, drift)
            .set(&GEN_MOD_HZ, md)
            .seed(seed);
        let r = rows(&stream(&DESCRIPTOR, &h, rate, TUNED, 26.0, seed, |_, _| {}));
        assert_eq!(r.len(), 2, "{rate} {fe}: {r:?}");
        let start_want = fe + so * (tau / 0.15) * ((-0.05 / tau).exp() - (-0.2 / tau).exp());
        // The spread of the settling time follows the drift-free value of the specification.
        let settle_spread = tau * (so.abs() / 10f64.max(0.1 * so.abs())).ln();
        for e in &r {
            let len = e.length_s.0;
            let final_want = fe + drift * (len - 0.15);
            // Settling is measured against the measured final frequency, so the drift counts.
            let settle_want = settle_of(fe, so, tau, drift, len);
            let d = e.drift_hz_s.unwrap();
            println!(
                "{rate} sps, case {k}: {len:.2} s, start {:.1} ({start_want:.1}), final {:.1} ({final_want:.1}), \
                 settle {:.3} ({settle_want:.3}), drift {d:.2} ({drift})",
                e.start_hz.0, e.final_hz.0, e.settle_s.0
            );
            assert!((e.final_hz.0 - final_want).abs() <= 25.0, "case {k}: {e:?}");
            assert!(
                (e.start_hz.0 - start_want).abs() <= 0.12 * so.abs() + 25.0,
                "case {k}: {e:?}"
            );
            assert!(
                (e.settle_s.0 - settle_want).abs() <= 0.12 + 0.2 * settle_spread,
                "case {k}: {e:?}"
            );
            let tol = if len > 5.0 { 1.5 } else { 5.0 };
            assert!((d - drift).abs() <= tol, "case {k}: {e:?}");
        }
    }
}

#[test]
fn noise_alone_for_a_minute_gives_no_row() {
    for seed in [11, 12, 13] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 35.0)
            .set(&GEN_KEYED, "")
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 60.0, seed, |_, _| {});
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
    }
}
