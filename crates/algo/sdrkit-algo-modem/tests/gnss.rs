//! A whole GNSS tracking channel from the shared parts: FFT acquisition, the code tracker (early, prompt and late correlators with
//! a delay-locked loop) and the carrier loop (`CarrierSync` with a Costas BPSK detector and a frequency-loop assist) on the
//! prompts, with the carrier's Doppler aiding the code. GPS L1 C/A (IS-GPS-200) at 2 samples per chip, 45 dB-Hz, a Doppler of
//! 2.3 kHz moving at 30 Hz/s, the code Doppler that goes with it, and 50 bit/s navigation data.

use sdrkit_algo_modem::carrier::{CarrierConfig, CarrierSync, Costas};
use sdrkit_algo_modem::dsss::{chips_from_bits, CodeAcquisition, CodeTracker, TrackerConfig};
use sdrkit_algo_modem::Complex32;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_bits::seq::gps_ca;
use std::f64::consts::TAU;

const FS: f64 = 2.046e6;
const L1: f64 = 1575.42e6;

#[test]
fn acquisition_tracking_and_data() {
    let mut ca = [0u8; 1023];
    gps_ca(7, &mut ca).unwrap();
    let mut code = Vec::new();
    chips_from_bits(&ca, &mut code);

    let seconds = 1.5;
    let n = (seconds * FS) as usize;
    let (fd0, fdd) = (2300.0, 30.0); // Hz, Hz/s
    let chip0 = 700.3;
    let mut rng = Rng::new(5);
    let nav: Vec<f32> = (0..100)
        .map(|_| if rng.bit() == 1 { 1.0 } else { -1.0 })
        .collect();
    // 45 dB-Hz: C/N0 / fs per sample, unit carrier amplitude.
    let n0 = FS / 10f64.powf(4.5);
    let mut carrier_phase = 0.4f64;
    let mut chip = chip0;
    let mut chips_at = vec![0f64; n + 1];
    let x: Vec<Complex32> = (0..n)
        .map(|k| {
            let t = k as f64 / FS;
            let fd = fd0 + fdd * t;
            chips_at[k] = chip;
            let c = code[(chip.rem_euclid(1023.0)) as usize % 1023];
            // Bit edges fall on code epochs (every 20 periods), as GPS sends them.
            let d = nav[(chip / 1023.0 / 20.0) as usize];
            let v =
                Complex32::new(carrier_phase.cos() as f32, carrier_phase.sin() as f32) * (c * d);
            carrier_phase = (carrier_phase + TAU * fd / FS).rem_euclid(TAU);
            chip += 1.023e6 * (1.0 + fd / L1) / FS;
            v + rng.complex_normal(n0)
        })
        .collect();
    chips_at[n] = chip;

    // Acquisition over 4 ms: +-5 kHz in 1 kHz bins with 4 sub-bins (250 Hz).
    let p = 2046usize;
    let mut acq = CodeAcquisition::new(&code, 2.0, 5, 4).unwrap();
    for k in 0..4 {
        acq.accumulate(&x[k * p..(k + 1) * p]).unwrap();
    }
    let a = acq.result().unwrap();
    let f0 = a.freq;
    assert!((f0 * FS - fd0).abs() < 200.0, "acquired {} Hz", f0 * FS);
    assert!(a.ratio > 5.0, "{}", a.ratio);

    // Tracking from sample 4P: the code phase of that sample from the acquisition's period start.
    let start = 4 * p;
    let mut tracker = CodeTracker::new(
        &code,
        2.0,
        TrackerConfig {
            carrier_to_code: 1.023e6 / L1,
            ..TrackerConfig::new(0.002)
        },
    )
    .unwrap();
    tracker.set_carrier(f0);
    tracker.set_code_phase((-(a.code_phase) / 2.0).rem_euclid(1023.0));
    let mut ccfg = CarrierConfig::new(0.015);
    ccfg.fll_bn_t = Some(0.01);
    ccfg.max_freq = 0.25;
    let mut cs = CarrierSync::new(Costas::<2>, ccfg).unwrap();
    let mut corr = Vec::new();
    let mut prompts = Vec::new();
    let mut freqs = Vec::new();
    let mut fed = start;
    for piece in x[start..].chunks(1000) {
        corr.clear();
        tracker.process(piece, &mut corr);
        fed += piece.len();
        for c in &corr {
            let y = cs.push(c.prompt);
            prompts.push(y);
            freqs.push((f0 + cs.freq() / p as f64) * FS);
            tracker.set_code_aid(f0 + cs.freq() / p as f64);
        }
    }
    // Code phase against the truth at the next sample.
    let err = (tracker.code_phase() - chips_at[fed].rem_euclid(1023.0) + 511.5).rem_euclid(1023.0)
        - 511.5;
    assert!(err.abs() < 0.08, "code phase error {err} chips");
    // Carrier frequency against the truth: the mean over the last 0.2 s (the loop's own frequency noise averaged out) against the
    // Doppler at the middle of that time. Under a Doppler ramp the integrator of a type-2 loop lags by `kp e`, where the steady
    // phase error `e = ramp / ki`: at 30 Hz/s, e = 0.038 cycles and the lag is 1.5 Hz, so the tolerance is 2.5 Hz.
    let f_end = freqs[freqs.len() - 200..].iter().sum::<f64>() / 200.0;
    let f_true = fd0 + fdd * (fed as f64 / FS - 0.1);
    assert!(
        (f_end - f_true).abs() < 2.5,
        "carrier {f_end} Hz vs {f_true}"
    );
    assert!(cs.locked(), "lock {}", cs.lock_metric());

    // Navigation bits: sum the 20 prompts of each bit (prompt i is code epoch `first + i`, the first one partial); after the first
    // 0.5 s every bit is right up to the BPSK sign ambiguity.
    let first = (chips_at[start] / 1023.0) as usize;
    let mut errors = [0usize; 2];
    let mut bits = 0;
    for (b, &want) in nav.iter().enumerate().take((n / p) / 20 - 1).skip(25) {
        let s: f32 = (b * 20..b * 20 + 20)
            .filter_map(|k| prompts.get(k.wrapping_sub(first)))
            .map(|z| z.re)
            .sum();
        errors[0] += ((s > 0.0) != (want > 0.0)) as usize;
        errors[1] += ((s < 0.0) != (want > 0.0)) as usize;
        bits += 1;
    }
    assert!(bits > 40);
    assert_eq!(errors[0].min(errors[1]), 0, "{errors:?} of {bits}");
}
