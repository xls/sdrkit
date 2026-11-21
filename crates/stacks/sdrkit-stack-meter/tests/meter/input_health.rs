//! The input health meter: three rates with DC and an IQ mismatch, low and high levels, clipping against the true share of
//! clipped samples, and noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{GEN_DC_I, GEN_DC_Q, GEN_IQ_GAIN_DB, GEN_IQ_PHASE_DEG};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::Complex32;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::input_health::gen::{GEN_CLIP, GEN_POWER, GEN_SCALE, GEN_TONES};
use sdrkit_stack_meter::input_health::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    InputEventKind, InputHealthEventRow, InputHealthFields, InputVerdict, INPUT_HEALTH_EVENTS,
};

const TUNED: f64 = 145e6;

fn fields(r: &Streamed) -> InputHealthFields {
    InputHealthFields::read(r.results.fields()).unwrap()
}

fn rows(r: &Streamed) -> Vec<InputHealthEventRow> {
    INPUT_HEALTH_EVENTS
        .rows::<InputHealthEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

#[test]
fn three_rates_read_the_level_the_dc_and_the_mismatch() {
    let mut seed = 0;
    for rate in [250e3, 1e6, 2.4e6] {
        seed += 1;
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_DC_I, 0.005)
            .set(&GEN_DC_Q, -0.003)
            .set(&GEN_IQ_GAIN_DB, 0.8)
            .set(&GEN_IQ_PHASE_DEG, -2.5)
            .seed(seed);
        let f = fields(&stream(&DESCRIPTOR, &h, rate, TUNED, 3.0, seed, |_, _| {}));
        let (l, g, ph, dc) = (
            f.rms_dbfs.unwrap(),
            f.iq_gain_db.unwrap(),
            f.iq_phase_deg.unwrap(),
            f.dc_i_pct.unwrap(),
        );
        println!(
            "{rate}: {l:.2} dBFS, {g:.3} dB, {ph:.3} deg, DC I {dc:.4} %, {:?} ppm, IR {:?}",
            f.clipping_ppm, f.image_rejection_db
        );
        assert!(
            (l - (-26.0206 + 10.0 * 1.18f64.log10())).abs() <= 1.5,
            "{l}"
        );
        assert!(
            (g - 0.8).abs() <= 0.15 && (ph - 2.5).abs() <= 0.4,
            "{g} {ph}"
        );
        assert!((dc - 0.5).abs() <= 0.05, "{dc}");
        assert_eq!(f.verdict, Some(InputVerdict::Good));
        assert!(f.clipping_ppm.unwrap() < 5.0);
    }
}

#[test]
fn a_weak_input_is_low_and_a_strong_one_high() {
    for seed in [11, 12] {
        let f = fields(&stream(
            &DESCRIPTOR,
            &Harness::new(&DESCRIPTOR).set(&GEN_POWER, 1e-6).seed(seed),
            1e6,
            TUNED,
            2.0,
            seed,
            |_, _| {},
        ));
        assert_eq!(f.verdict, Some(InputVerdict::Low), "{f:?}");
        let f = fields(&stream(
            &DESCRIPTOR,
            &Harness::new(&DESCRIPTOR).set(&GEN_POWER, 0.2).seed(seed),
            1e6,
            TUNED,
            2.0,
            seed,
            |_, _| {},
        ));
        assert!(
            matches!(f.verdict, Some(InputVerdict::High | InputVerdict::Overload)),
            "{f:?}"
        );
    }
}

#[test]
fn clipping_reads_the_share_of_clipped_samples() {
    let rate = 1e6;
    for (k, clip) in [0.5, 0.35, 0.25].into_iter().enumerate() {
        let seed = 21 + k as u64;
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_POWER, 0.0316)
            .set(&GEN_CLIP, clip)
            .set(&GEN_SCALE, 1.0 / clip)
            .seed(seed);
        let mut at_limit = 0u64;
        let r = stream(
            &DESCRIPTOR,
            &h,
            rate,
            TUNED,
            3.0,
            seed,
            |t, x: &mut [Complex32]| {
                if t >= 2.0 {
                    at_limit += x
                        .iter()
                        .filter(|z| z.re.abs().max(z.im.abs()) >= 1.0 - 1e-6)
                        .count() as u64;
                }
            },
        );
        let f = fields(&r);
        let truth = 1e6 * at_limit as f64 / rate;
        let got = f.clipping_ppm.unwrap();
        println!(
            "clip {clip}: {got:.1} ppm, true {truth:.1} ppm, {:?}",
            f.verdict
        );
        assert!((got - truth).abs() <= 0.15 * truth + 50.0, "{got} {truth}");
        if truth > 500.0 {
            assert_eq!(f.verdict, Some(InputVerdict::Overload));
            assert!(rows(&r).iter().any(|e| e.kind == InputEventKind::Clipping));
        }
    }
}

#[test]
fn noise_alone_reads_its_own_level() {
    for seed in [31, 32, 33] {
        let h = Harness::new(&DESCRIPTOR).set(&GEN_TONES, "").seed(seed);
        let r = stream(&DESCRIPTOR, &h, 1e6, TUNED, 3.0, seed, |_, _| {});
        let f = fields(&r);
        let l = f.rms_dbfs.unwrap();
        assert!(
            (l + 26.0206).abs() < 0.1 && f.verdict == Some(InputVerdict::Good),
            "{f:?}"
        );
        assert!(
            f.iq_gain_db.unwrap().abs() < 0.05 && f.iq_phase_deg.unwrap().abs() < 0.2,
            "{f:?}"
        );
        let r = rows(&r);
        assert!(
            r.len() == 1 && r[0].kind == InputEventKind::Verdict,
            "{r:?}"
        );
    }
}
