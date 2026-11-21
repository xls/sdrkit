//! The AFSK tuner: four cases of rate, deviation, twist, clock error and offset; a sweep of the noise; noise alone; one long
//! stream.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::afsk_tuner::gen::{
    GEN_BURST_S, GEN_DEVIATION_HZ, GEN_PERIOD_S, GEN_TWIST_DB,
};
use sdrkit_stack_meter::afsk_tuner::DESCRIPTOR;
use sdrkit_stack_meter::typed::{AfskTunerBurstEvent, AFSK_TUNER_BURST};

const TUNED: f64 = 144.39e6;

fn at(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
}

fn bursts(r: &Streamed) -> Vec<AfskTunerBurstEvent> {
    r.events
        .iter()
        .filter_map(|e| AFSK_TUNER_BURST.read::<AfskTunerBurstEvent>(e))
        .collect()
}

const CASES: [(f64, f64, f64, f64, f64); 4] = [
    (48e3, 3000.0, 0.0, 0.0, 0.0),
    (48e3, 2500.0, -4.0, 300.0, 1500.0),
    (96e3, 3500.0, 3.0, -400.0, -2000.0),
    (250e3, 2000.0, -6.0, 0.0, 3000.0),
];

/// The bursts of case `k`.
fn case(k: usize) -> Vec<AfskTunerBurstEvent> {
    let (rate, dev, twist, ppm, off) = CASES[k];
    let seed = 1 + k as u64;
    let h = at(30.0)
        .set(&GEN_DEVIATION_HZ, dev)
        .set(&GEN_TWIST_DB, twist)
        .set(&GEN_CLOCK_PPM, ppm)
        .set(&GEN_OFFSET_HZ, off)
        .seed(seed);
    bursts(&stream(&DESCRIPTOR, &h, rate, TUNED, 14.0, seed, |_, _| {}))
}

#[test]
fn four_cases_read_the_mark_tone_the_deviation_and_the_length() {
    for (k, &(_, dev, twist, _, _)) in CASES.iter().enumerate() {
        let b = case(k);
        assert!((5..=7).contains(&b.len()), "case {k}: {b:?}");
        println!(
            "case {k}: twists {:.2?}",
            b.iter().map(|e| e.twist_db).collect::<Vec<_>>()
        );
        for e in &b {
            println!(
                "case {k}: mark {:.1}, space {:.1}, twist {:.2} ({twist}), {:.3} kHz ({dev}), {:.3} s",
                e.mark_hz.0, e.space_hz.0, e.twist_db, e.peak_deviation_khz, e.length_s.0
            );
            assert!((e.mark_hz.0 - 1200.0).abs() <= 25.0, "case {k}: {e:?}");
            assert!(
                (e.peak_deviation_khz * 1000.0 - dev).abs() <= 0.16 * dev,
                "case {k}: {e:?}"
            );
        }
        let last = b.last().unwrap();
        assert!((last.length_s.0 - 0.5).abs() <= 0.12, "case {k}: {last:?}");
    }
}

#[test]
fn four_cases_read_the_space_tone_of_each_burst() {
    for k in 0..CASES.len() {
        let sp: Vec<f64> = case(k).iter().map(|e| e.space_hz.0).collect();
        println!("case {k}: space {sp:.1?}");
        assert!(
            sp.iter().all(|f| (f - 2200.0).abs() <= 70.0),
            "case {k}: {sp:?}"
        );
    }
}

#[test]
fn four_cases_read_the_twist_of_each_burst() {
    for (k, c) in CASES.iter().enumerate() {
        let twist = c.2;
        let tw: Vec<f64> = case(k).iter().map(|e| e.twist_db).collect();
        println!("case {k}: twists {tw:.2?} ({twist})");
        assert!(
            tw.iter().all(|t| (t - twist).abs() <= 1.5),
            "case {k}: {tw:?}"
        );
    }
}

#[test]
fn the_twist_holds_down_to_15_db() {
    for (k, cnr) in [30.0, 20.0, 15.0, 12.0, 10.0].into_iter().enumerate() {
        let seed = 11 + k as u64;
        let h = at(cnr).set(&GEN_TWIST_DB, -4.0).seed(seed);
        let b = bursts(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 14.0, seed, |_, _| {}));
        let tw: Vec<f64> = b.iter().map(|e| e.twist_db).collect();
        println!("{cnr} dB: {} bursts, twist {tw:.2?}", b.len());
        if cnr >= 15.0 {
            assert!(
                b.len() >= 5 && tw.iter().all(|t| (t + 4.0).abs() <= 2.0),
                "{cnr}: {b:?}"
            );
        }
    }
}

#[test]
fn noise_alone_gives_no_burst_and_a_long_stream_at_most_one() {
    for seed in [21, 22, 23] {
        let h = at(30.0).set(&GEN_DEVIATION_HZ, 0.0).seed(seed);
        let b = bursts(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 20.0, seed, |_, _| {}));
        assert!(b.is_empty(), "{b:?}");
        let h = at(30.0)
            .set(&GEN_PERIOD_S, 0.0)
            .set(&GEN_BURST_S, 20.0)
            .seed(seed);
        let b = bursts(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 21.0, seed, |_, _| {}));
        assert!(b.len() <= 1, "{b:?}");
    }
}
