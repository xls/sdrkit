//! WINMOR: the pilot at several tuning errors and noise levels, at 12 and 96 kHz, and nothing from noise or a plain carrier.

use crate::common::*;
use hf::decoder::Modulator;
use sdrkit_core::Complex32;
use sdrkit_stack_hfkbd as hf;

/// One frame start (24 pilot symbols, tones 1 6 3 4) at `rate` with the tuning error `off`, noise of `sigma` a component.
fn frame(rate: f64, off: f64, sigma: f64, seed: u64) -> Vec<Complex32> {
    let mut m = hf::winmor::WinmorModulator::new(rate, 24, [1, 6, 3, 4], off);
    let mut x = vec![Complex32::default(); (4.0 * rate) as usize];
    m.fill(&mut x);
    sdrkit_algo_sim::noise::Awgn::with_power(2.0 * sigma * sigma, seed)
        .unwrap()
        .process(&mut x);
    x
}

fn pilots(rate: f64, x: &[Complex32]) -> Vec<sdrkit_core::Record> {
    let r = feed(&harness(&hf::WINMOR).rate(rate), x);
    rows(&r, "pilots")
}

#[test]
fn one_report_per_frame_start() {
    for off in [0.0, 42.0, -83.0, 150.0] {
        for sigma in [0.03, 0.15] {
            let p = pilots(RATE, &frame(RATE, off, sigma, 3));
            assert_eq!(p.len(), 1, "{off} {sigma}");
            assert!(
                (cell_num(&p[0], "error_hz") - off).abs() <= 6.0,
                "{off} {sigma}: {}",
                cell_num(&p[0], "error_hz")
            );
            assert_eq!(cell_text(&p[0], "tones"), "1 6 3 4", "{off} {sigma}");
            assert!(
                (22.0..=26.0).contains(&cell_num(&p[0], "pilot_symbols")),
                "{}",
                cell_num(&p[0], "pilot_symbols")
            );
        }
    }
}

#[test]
fn at_96_khz() {
    let p = pilots(96_000.0, &frame(96_000.0, 60.0, 0.03, 4));
    assert_eq!(p.len(), 1);
    assert!((cell_num(&p[0], "error_hz") - 60.0).abs() <= 6.0);
}

#[test]
fn nothing_from_noise_or_a_carrier() {
    assert!(pilots(RATE, &noise_only(50.0, 5)).is_empty());
    let carrier: Vec<Complex32> = (0..(10.0 * RATE) as usize)
        .map(|i| {
            Complex32::from_polar(
                1.0,
                (std::f64::consts::TAU * 1500.0 * i as f64 / RATE) as f32,
            )
        })
        .collect();
    assert!(pilots(RATE, &carrier).is_empty());
}

#[test]
fn pilots_at_20_db_in_their_band() {
    // The noise of the verify case: 20 dB below the signal in its 500 Hz.
    for rate in [12_000.0, 24_000.0] {
        let h = harness(&hf::WINMOR)
            .rate(rate)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 20.0)
            .seconds(12.0);
        assert_eq!(rows(&run(&h), "pilots").len(), 3, "{rate}");
    }
}
