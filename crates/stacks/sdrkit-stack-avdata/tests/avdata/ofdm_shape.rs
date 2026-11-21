//! LDACS and AeroMACS: the structure detector on the test signals with offsets and noise, AeroMACS bursts, the wrong numerology,
//! noise alone, a setting, and the data files.

use crate::common::{self, noise, offset};
use sdrkit_core::{Complex32, GenConfig, LockState, Results, StackDescriptor, Value, Values};
use sdrkit_stack_avdata::typed::{OfdmShapeFields, OFDM_DETECTIONS};
use sdrkit_stack_avdata::{aeromacs, ldacs};

fn signal(d: &StackDescriptor, rate: f64, seconds: f64) -> Vec<Complex32> {
    let mut g =
        (d.make_generator.unwrap())(&GenConfig::new(rate, Values::defaults(&(d.settings)())))
            .unwrap();
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

fn on_power(x: &[Complex32]) -> f64 {
    let on: Vec<f64> = x
        .iter()
        .map(|v| f64::from(v.norm_sqr()))
        .filter(|p| *p > 0.0)
        .collect();
    on.iter().sum::<f64>() / on.len().max(1) as f64
}

fn detect(d: &StackDescriptor, x: &[Complex32], rate: f64, settings: &[(&str, &str)]) -> Results {
    let f = d.tuning.default_hz;
    let mut rx = common::receiver(d, rate, f, f, settings);
    common::run(rx.as_mut(), x, 65_536);
    sdrkit_core::owner::Owner::results(rx.as_ref()).clone()
}

#[test]
fn both_found_with_offsets_and_noise() {
    for (d, rate, seconds) in [
        (&ldacs::DESCRIPTOR, 2e6, 1.0),
        (&aeromacs::DESCRIPTOR, 5.6e6, 0.3),
    ] {
        for cfo in [0.0, 2000.0] {
            for snr in [25.0, 10.0, 5.0] {
                let mut x = signal(d, rate, seconds);
                offset(&mut x, rate, cfo);
                let p = on_power(&x);
                noise(&mut x, p, snr, rate, rate, 3);
                let r = detect(d, &x, rate, &[]);
                let what = format!("{} at {cfo} Hz, {snr} dB", d.id);
                assert_eq!(
                    r.fields().at(&OfdmShapeFields::DETECTED),
                    Some(true),
                    "{what}"
                );
                let off = r
                    .fields()
                    .get(&sdrkit_core::publish::keys::OFFSET_HZ)
                    .unwrap();
                assert!((off - cfo).abs() < 200.0, "{what}: {off}");
                assert!(r.fields().at(&OfdmShapeFields::CORRELATION).unwrap() > 0.05);
                assert_eq!(r.status().lock_state, LockState::Locked, "{what}");
            }
        }
    }
}

#[test]
fn ldacs_reads_its_numerology_and_a_continuous_frame() {
    let rate = 2e6;
    let x = signal(&ldacs::DESCRIPTOR, rate, 1.0);
    let r = detect(&ldacs::DESCRIPTOR, &x, rate, &[]);
    let f = |k: &str| crate::common::field(&r, k);
    assert!((f("symbol_us") - 120.0).abs() < 1e-9 && (f("prefix_us") - 17.6).abs() < 1e-9);
    assert_eq!(
        r.fields().at(&OfdmShapeFields::FRAME),
        Some("continuous over each 240 ms frame")
    );
    assert!(r
        .log_lines()
        .iter()
        .any(|l| l.contains("OFDM symbol 120.0 us, prefix 17.6 us, 625000 samples/s")));
    assert_eq!(r.fields().at(&OfdmShapeFields::NOTE), Some(ldacs::NOTE));
    assert!(
        r.status().summary.ends_with("structure only"),
        "{}",
        r.status().summary
    );
    let rows = common::rows(&r, &OFDM_DETECTIONS);
    assert_eq!(rows.len(), 1);
    assert!((common::num(&rows[0], "symbol_us") - 120.0).abs() < 1e-9);
    // Another FFT length finds nothing.
    let r = detect(&ldacs::DESCRIPTOR, &x, rate, &[("fft_len", "48")]);
    assert_eq!(r.fields().at(&OfdmShapeFields::DETECTED), Some(false));
}

#[test]
fn aeromacs_bursts_of_the_5_ms_frame() {
    let rate = 5.6e6;
    let x = signal(&aeromacs::DESCRIPTOR, rate, 0.3);
    let r = detect(&aeromacs::DESCRIPTOR, &x, rate, &[]);
    let frame = r.fields().at(&OfdmShapeFields::FRAME).unwrap().to_string();
    assert!(frame.starts_with("bursts: on for 6"), "{frame}");
    assert!(frame.ends_with("of each 5 ms frame"), "{frame}");
    assert_eq!(r.fields().at(&OfdmShapeFields::NOTE), Some(aeromacs::NOTE));
}

#[test]
fn ldacs_does_not_take_aeromacs_and_noise_is_nothing() {
    let rate = 5.6e6;
    let x = signal(&aeromacs::DESCRIPTOR, rate, 0.5);
    let r = detect(&ldacs::DESCRIPTOR, &x, rate, &[]);
    assert_eq!(r.fields().at(&OfdmShapeFields::DETECTED), Some(false));
    for (d, rate) in [(&ldacs::DESCRIPTOR, 2e6), (&aeromacs::DESCRIPTOR, 5.6e6)] {
        let mut z = vec![Complex32::default(); (0.6 * rate) as usize];
        noise(&mut z, 1.0, 0.0, rate, rate, 8);
        let r = detect(d, &z, rate, &[]);
        assert_eq!(
            r.fields().at(&OfdmShapeFields::DETECTED),
            Some(false),
            "{}",
            d.id
        );
        assert_ne!(r.status().lock_state, LockState::Locked);
        let mut rx =
            (d.make_receiver)(&sdrkit_core::StackConfig::new(common::values(d, &[]))).unwrap();
        let f = d.tuning.default_hz;
        assert!(rx
            .configure(&sdrkit_core::InputInfo::new(d.tuning.min_rate_hz / 2.0, f, f).recording())
            .is_err());
        assert!(sdrkit_core::owner::Owner::results(rx.as_ref())
            .status()
            .summary
            .contains("needs a sample rate of at least"));
    }
    let _ = Value::Bool(true);
}

#[test]
fn the_data_files_have_the_links() {
    let l = crate::plan_file("ldacs.json");
    assert_eq!(l.groups[0].bands.len(), 3);
    let a = crate::plan_file("aeromacs.json");
    let n = a.groups[0].bands[1].numbering.as_ref().unwrap();
    assert_eq!(n.last - n.first + 1, 12);
}
