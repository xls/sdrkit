//! Radio astronomy: the processing of spectra and the receiver.

use sci::astro::process::{self, BandShape, ProcOptions, HI_HZ};
use sci::astro::AstroGenerator;
use sci::typed::{AstroCalibratedRow, AstroFields, ASTRO_CALIBRATED, ASTRO_SESSION};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Owner, Receiver, Results, StackConfig};
use sdrkit_stack_science as sci;

fn average(line: bool, seconds: f64, n: usize, centre: f64, seed: u64) -> Vec<f64> {
    let rate = 2.5e6;
    let mut g = AstroGenerator::new(rate, line, -42.0 - process::velocity(centre, HI_HZ), seed);
    let win = sdrkit_algo_dsp::window::Window::Hann.coefficients(n);
    let mut fft = sdrkit_algo_dsp::fft::Fft::forward(n);
    let mut buf = vec![Complex32::new(0.0, 0.0); n];
    let mut acc = vec![0.0f64; n];
    let frames = (seconds * rate / n as f64) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    for _ in 0..frames {
        g.generate(&mut x);
        for k in 0..n {
            buf[k] = x[k] * win[k];
        }
        fft.process(&mut buf);
        for k in 0..n {
            acc[k] += buf[(k + n / 2) % n].norm_sqr() as f64;
        }
    }
    acc.iter().map(|v| v / frames as f64).collect()
}

#[test]
fn line_found_baseline_flattened_and_band_shapes() {
    let on = average(true, 16.0, 2048, HI_HZ, 1);
    let o = ProcOptions {
        smooth: 64,
        ..Default::default()
    };
    let p = process::process(&on, None, &o);
    assert!((p.peak_kms + 42.0).abs() < 3.5, "{}", p.peak_kms);
    assert!(p.snr > 5.0, "{}", p.snr);
    assert!((p.excess - 0.01).abs() < 0.004, "{}", p.excess);
    // The band shape from an off-source spectrum.
    let off = average(false, 16.0, 2048, HI_HZ, 2);
    let q = process::process(
        &on,
        Some(&off),
        &ProcOptions {
            band_shape: BandShape::OffSource,
            smooth: 64,
            ..Default::default()
        },
    );
    assert!((q.peak_kms + 42.0).abs() < 4.0, "{}", q.peak_kms);
    // The band slope is flattened by the baseline (no line).
    let flat = process::process(
        &off,
        None,
        &ProcOptions {
            smooth: 64,
            ..Default::default()
        },
    );
    let edge = 165;
    let tilt = (flat.ratio[2048 - edge] - flat.ratio[edge]).abs();
    assert!(tilt < 0.003, "{tilt}");
    // CSV round trip.
    let t = process::csv(&p, &on, &o, 4.0);
    let (_, rows) = process::read_csv(&t).unwrap();
    assert_eq!(rows.len(), 2048);
    assert!((rows[1000][1] - p.vel[1000]).abs() < 1e-3);
}

#[test]
fn frequency_switching_with_fold() {
    let shift_hz = 500e3;
    let n = 2048;
    let a = average(true, 16.0, n, HI_HZ, 3);
    let b = average(true, 16.0, n, HI_HZ + shift_hz, 4);
    let shift = (shift_hz / (2.5e6 / n as f64)).round() as isize;
    let o = ProcOptions {
        band_shape: BandShape::Switching(shift),
        fold: true,
        smooth: 48,
        window_kms: (-100.0, 20.0),
        ..Default::default()
    };
    let p = process::process(&a, Some(&b), &o);
    // The statistics of 16 s per phase allow about ±4 km/s on the centroid.
    assert!((p.peak_kms + 42.0).abs() < 4.0, "{}", p.peak_kms);
    assert!((p.excess - 0.01).abs() < 0.004, "{}", p.excess);
}

/// The rows of a table of the results.
fn rows<'a>(r: &'a Results, key: &str) -> &'a sdrkit_core::Table {
    r.table(r.decl().table_id(key).unwrap()).unwrap()
}

/// A receiver with a 2048-bin FFT and integrations of `seconds`, configured for a recording at `rate` on the H I line.
fn receiver(rate: f64, seconds: f64) -> sdrkit_graph::StackGraph {
    let values = sdrkit_dev_test::Harness::new(&sci::ASTRO)
        .set(
            &sdrkit_stack_science::astro::FFT,
            sdrkit_stack_science::astro::FftSize::S2048,
        )
        .set(&sdrkit_stack_science::astro::INTEGRATION, seconds)
        .values()
        .unwrap();
    let mut rx = sci::astro::receiver(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(rate, HI_HZ, HI_HZ).recording())
        .unwrap();
    rx
}

#[test]
fn receiver_finishes_integrations() {
    let rate = 1e6;
    let mut rx = receiver(rate, 1.0);
    let mut g = AstroGenerator::new(rate, true, -42.0, 5);
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    let mut out = Output::default();
    for _ in 0..40 {
        g.generate(&mut buf);
        rx.feed(&buf, &mut out);
    }
    rx.refresh_results();
    let r = Owner::results(&rx);
    assert!(r.fields().at(&AstroFields::FINISHED).unwrap() >= 2);
    assert_eq!(rows(r, ASTRO_CALIBRATED.key).len(), 2048);
}

#[test]
fn the_line_shows_through_the_harness() {
    let r = sdrkit_dev_test::Harness::new(&sci::ASTRO)
        .rate(2.5e6)
        .seconds(11.0)
        .set(&sdrkit_stack_science::astro::SMOOTH, 64)
        .run()
        .unwrap();
    let v = r.results.fields().at(&AstroFields::PEAK_KM_S).unwrap();
    assert!((v + 42.0).abs() < 5.0, "{v}");
}

fn run_with_site(
    extra: fn(sdrkit_dev_test::Harness) -> sdrkit_dev_test::Harness,
) -> sdrkit_dev_test::Run {
    let h = sdrkit_dev_test::Harness::new(&sci::ASTRO)
        .seconds(2.3)
        .set(
            &sdrkit_stack_science::astro::FFT,
            sdrkit_stack_science::astro::FftSize::S2048,
        )
        .set(&sdrkit_stack_science::astro::INTEGRATION, 1.0)
        .station(sdrkit_core::Station::at(sdrkit_util_geo::Position::new(
            52.0, 5.0,
        )));
    extra(h).run().unwrap()
}

#[test]
fn a_custom_ra_dec_target_and_the_lsr_velocity() {
    // Sgr A* typed as RA and Dec lies near the galactic centre.
    let r = run_with_site(|h| {
        h.set(
            &sdrkit_stack_science::astro::TARGET_SETTING,
            sdrkit_stack_science::astro::Target::RaDec,
        )
        .set(&sdrkit_stack_science::astro::TARGET_RA, 266.41683)
        .set(&sdrkit_stack_science::astro::TARGET_DEC, -29.00781)
    });
    let f = r.results.fields();
    let peak = f.at(&AstroFields::PEAK_KM_S).unwrap();
    let vlsr = f.at(&AstroFields::VEL_LSR_KMS).unwrap();
    let sun = f.at(&AstroFields::LSR_SUN_KMS).unwrap();
    let total = f.at(&AstroFields::LSR_ROTATION_KMS).unwrap() + f.at(&AstroFields::LSR_ORBIT_KMS).unwrap() + sun;
    assert!(
        (vlsr - (peak + total)).abs() < 0.02,
        "{vlsr} {peak} {total}"
    );
    // The Sun term towards the galactic centre (l 0, b 0) is the solar motion's U component, 10.3 km/s (IAU kinematic LSR).
    assert!((9.9..10.5).contains(&sun), "{sun}");
    assert!((f.at(&AstroFields::TARGET_DEC_DEG).unwrap() + 29.0).abs() < 0.1);
}

#[test]
fn the_calibrated_spectrum_and_the_session_are_typed_results() {
    let r = run_with_site(|h| h);
    let cal = rows(&r.results, ASTRO_CALIBRATED.key);
    assert_eq!(cal.len(), 2048);
    let labels: Vec<&str> = cal
        .columns()
        .fields()
        .iter()
        .map(|f| f.label.as_ref())
        .collect();
    assert_eq!(
        labels,
        [
            "Frequency",
            "Velocity (topocentric)",
            "Velocity (LSR)",
            "Antenna temperature",
            "Flagged"
        ]
    );
    assert!(cal.row("00000").unwrap().at(&AstroCalibratedRow::VEL_LSR_KMS).is_some());
    assert_eq!(rows(&r.results, ASTRO_SESSION.key).len(), 2);
    // Without a station position the LSR column is empty.
    let r = sdrkit_dev_test::Harness::new(&sci::ASTRO)
        .seconds(1.2)
        .set(
            &sdrkit_stack_science::astro::FFT,
            sdrkit_stack_science::astro::FftSize::S2048,
        )
        .set(&sdrkit_stack_science::astro::INTEGRATION, 1.0)
        .run()
        .unwrap();
    let cal = rows(&r.results, ASTRO_CALIBRATED.key);
    assert!(cal.row("00000").unwrap().at(&AstroCalibratedRow::VEL_LSR_KMS).is_none());
    assert!(r.results.fields().at(&AstroFields::VEL_LSR_KMS).is_none());
}

#[test]
fn the_session_saves_and_restores() {
    let rate = 2.5e6;
    let mut rx = receiver(rate, 0.5);
    let mut g = AstroGenerator::new(rate, true, -42.0, 3);
    let mut x = vec![Complex32::new(0.0, 0.0); 65536];
    let mut out = Output::default();
    for _ in 0..40 {
        g.generate(&mut x);
        rx.feed(&x, &mut out);
    }
    rx.refresh_results();
    assert_eq!(rows(Owner::results(&rx), ASTRO_SESSION.key).len(), 2);
    let saved = rx.save_state().expect("a session");
    let mut other = receiver(rate, 0.5);
    other.restore_state(&saved).unwrap();
    other.refresh_results();
    assert_eq!(rows(Owner::results(&other), ASTRO_SESSION.key).len(), 2);
    let junk = sdrkit_core::StateBlob::new("astro", 1, b"{\"junk\": 1}".to_vec());
    assert!(other.restore_state(&junk).is_err());
}
