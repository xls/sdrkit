//! The noise figure meter: Y-factors at three rates, the noise figure of a 15 dB source, steps under the smallest, and
//! steady noise.

use crate::stream::{stream, Streamed};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::noise_figure::gen::{GEN_DRIFT_DB_S, GEN_Y_DB};
use sdrkit_stack_meter::noise_figure::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    NoiseFigureEventRow, NoiseFigureFields, NoiseFigureRow, NOISE_FIGURE_EVENTS, NOISE_FIGURE_TABLE,
};

const TUNED: f64 = 145e6;

fn run(y: f64, rate: f64, seed: u64) -> Streamed {
    let h = Harness::new(&DESCRIPTOR).set(&GEN_Y_DB, y).seed(seed);
    stream(&DESCRIPTOR, &h, rate, TUNED, 62.0, seed, |_, _| {})
}

fn rows(r: &Streamed) -> Vec<NoiseFigureEventRow> {
    NOISE_FIGURE_EVENTS
        .rows::<NoiseFigureEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn the_y_factor_reads_the_step_at_three_rates() {
    let mut seed = 0;
    for (rate, ys) in [
        (250e3, &[0.5, 3.0, 10.0, 15.0][..]),
        (1e6, &[0.5, 3.0, 10.0, 15.0][..]),
        (2e6, &[0.5, 1.0, 3.0, 6.0, 10.0, 15.0][..]),
    ] {
        for &y in ys {
            seed += 1;
            let f = NoiseFigureFields::read(run(y, rate, seed).results.fields()).unwrap();
            println!(
                "{rate} sps, Y {y}: {:?} dB (spread {:?}), {} steps",
                f.y_db, f.y_spread_db, f.steps
            );
            assert!(
                (f.y_db.unwrap() - y).abs() <= 0.12,
                "{rate} {y}: {:?}",
                f.y_db
            );
        }
    }
}

#[test]
fn a_15_db_source_and_a_10_db_step_give_5_46_db() {
    for seed in [41, 42] {
        let r = run(10.0, 250e3, seed);
        let t: Vec<NoiseFigureRow> = NOISE_FIGURE_TABLE
            .rows::<NoiseFigureRow>(&r.results)
            .map(|(_, e)| e)
            .collect();
        let nf = t
            .iter()
            .find(|e| e.enr_db == 15.0)
            .and_then(|e| e.nf_db)
            .unwrap();
        println!("NF {nf:.3} dB");
        assert!((nf - (15.0 - 10.0 * 9f64.log10())).abs() <= 0.15, "{nf}");
        assert!(t.iter().any(|e| e.enr_db == 5.0 && e.negative), "{t:?}");
    }
}

#[test]
fn small_steps_and_steady_noise_give_no_row() {
    for seed in [51, 52, 53] {
        let r = run(0.1, 250e3, seed);
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_Y_DB, 0.0)
            .set(&GEN_DRIFT_DB_S, 0.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 250e3, TUNED, 62.0, seed, |_, _| {});
        let f = NoiseFigureFields::read(r.results.fields()).unwrap();
        assert!(
            rows(&r).is_empty() && f.y_db.is_none() && f.steps == 0,
            "{f:?}"
        );
    }
}
