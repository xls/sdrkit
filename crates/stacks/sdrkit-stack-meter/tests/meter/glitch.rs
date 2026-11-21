//! The stream glitch detector: drops and repeats on tones at both sides of the centre, a zero run, a repeated block and a
//! level step, a drifting tone, a weak tone and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_NOISE_FLOOR_DBFS};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::glitch::gen::{
    GEN_DROPS, GEN_GAIN_STEPS, GEN_REPEAT_BLOCKS, GEN_TONE_AMP, GEN_TONE_HZ, GEN_ZERO_RUNS,
};
use sdrkit_stack_meter::glitch::DESCRIPTOR;
use sdrkit_stack_meter::typed::{GlitchEventRow, GlitchFields, GlitchKind, GLITCH_EVENTS};

const TUNED: f64 = 145e6;
const RATE: f64 = 2e6;

fn noise(floor_dbfs: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Floor)
        .set(&GEN_NOISE_FLOOR_DBFS, floor_dbfs)
}

fn run(h: Harness, seconds: f64, seed: u64) -> Streamed {
    stream(
        &DESCRIPTOR,
        &h.seed(seed),
        RATE,
        TUNED,
        seconds,
        seed,
        |_, _| {},
    )
}

fn rows(r: &Streamed) -> Vec<GlitchEventRow> {
    let mut v: Vec<GlitchEventRow> = GLITCH_EVENTS
        .rows::<GlitchEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect();
    v.sort_by(|a, b| a.time_s.0.total_cmp(&b.time_s.0));
    v
}

#[test]
fn drops_and_repeats_read_their_sizes_and_times() {
    for (seed, f) in [(1, 123_456.0), (2, -45_678.0)] {
        let r = rows(&run(noise(-40.0).set(&GEN_TONE_HZ, f), 5.0, seed));
        let drops: Vec<&GlitchEventRow> = r
            .iter()
            .filter(|e| e.kind == GlitchKind::DroppedSamples)
            .collect();
        let reps: Vec<&GlitchEventRow> = r
            .iter()
            .filter(|e| e.kind == GlitchKind::RepeatedSamples)
            .collect();
        println!(
            "{f} Hz: {:?}",
            r.iter()
                .map(|e| (e.kind, e.time_s.0, e.samples))
                .collect::<Vec<_>>()
        );
        assert_eq!((drops.len(), reps.len()), (4, 2), "{r:?}");
        let sizes: Vec<i64> = drops.iter().map(|e| e.samples.unwrap()).collect();
        assert_eq!(sizes, vec![1, 2, 3, 5]);
        assert_eq!(
            reps.iter().map(|e| e.samples.unwrap()).collect::<Vec<_>>(),
            vec![1, 2]
        );
        for (e, t) in drops
            .iter()
            .chain(&reps)
            .zip([1.0, 1.5, 2.0, 2.5, 3.0, 3.5])
        {
            assert!((e.time_s.0 - t).abs() <= 1e-3, "{} against {t}", e.time_s.0);
        }
    }
}

#[test]
fn a_zero_run_a_repeated_block_and_a_level_step() {
    for seed in [11, 12] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_DROPS, "")
            .set(&GEN_ZERO_RUNS, "1.0, 40")
            .set(&GEN_REPEAT_BLOCKS, "2.0")
            .set(&GEN_GAIN_STEPS, "3.0, 3");
        let r = rows(&run(h, 4.0, seed));
        let of = |k| r.iter().filter(|e| e.kind == k).collect::<Vec<_>>();
        let (z, b, s) = (
            of(GlitchKind::ZeroRun),
            of(GlitchKind::RepeatedBlock),
            of(GlitchKind::LevelStep),
        );
        println!(
            "{:?}",
            r.iter()
                .map(|e| (e.kind, e.time_s.0, e.step_db))
                .collect::<Vec<_>>()
        );
        assert!(z.len() == 1 && b.len() == 1 && s.len() == 1, "{r:?}");
        assert!((z[0].time_s.0 - 1.0).abs() < 1e-6 && (b[0].time_s.0 - 2.000_128).abs() < 1e-3);
        let step = s[0].step_db.unwrap();
        assert!(
            (step - 3.0).abs() <= 0.3 && (s[0].time_s.0 - 3.0).abs() <= 0.01,
            "{step} at {}",
            s[0].time_s.0
        );
    }
}

#[test]
fn a_drifting_tone_a_weak_tone_and_noise_alone_give_no_row() {
    for seed in [21, 22] {
        let r = run(
            noise(-40.0).set(&GEN_DROPS, "").set(&GEN_DRIFT_HZ_S, 50.0),
            6.0,
            seed,
        );
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
        let r = run(
            noise(-30.0).set(&GEN_DROPS, "").set(&GEN_TONE_AMP, 0.05),
            6.0,
            seed,
        );
        assert!(rows(&r).is_empty(), "{:?}", rows(&r));
        let r = run(
            noise(-40.0).set(&GEN_DROPS, "").set(&GEN_TONE_AMP, 0.0),
            6.0,
            seed,
        );
        let f = GlitchFields::read(r.results.fields()).unwrap();
        assert!(
            rows(&r).is_empty() && f.tone_hz.is_none(),
            "{:?} {f:?}",
            rows(&r)
        );
    }
}
