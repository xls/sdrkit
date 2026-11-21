//! Synthetic waveforms for the identifier's tests: serial-tone 8-PSK with frames, a Link 11-like comb, MFSK,
//! white noise to an SNR in 3 kHz. 12 kHz real audio.

use sdrkit_algo_dsp::pulse::rrc;
use sdrkit_algo_sim::rng::Rng;
use std::f64::consts::{PI, TAU};

/// Adds white noise for an SNR (dB) in 3 kHz relative to the signal's power.
pub fn add_noise(x: &mut [f32], fs: f64, snr_db: f64, seed: u64) {
    let p = x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len().max(1) as f64;
    // Real noise of variance s^2 has a density of 2 s^2 / fs over 0 .. fs/2.
    let s = (p / 10f64.powf(snr_db / 10.0) * fs / (2.0 * 3000.0)).sqrt();
    let mut r = Rng::new(seed);
    for v in x.iter_mut() {
        *v += (r.normal() * s) as f32;
    }
}

/// 8-PSK at `baud` on `centre` Hz with root-raised-cosine pulses (0.35, span 8), frames of `frame` symbols whose first `known` are a
/// fixed sequence.
pub fn serial_tone(
    fs: f64,
    secs: f64,
    baud: f64,
    centre: f64,
    frame: usize,
    known: usize,
    seed: u64,
) -> Vec<f32> {
    let n = (fs * secs) as usize;
    let nsym = (secs * baud) as usize + 16;
    let mut r = Rng::new(seed);
    let syms: Vec<(f64, f64)> = (0..nsym)
        .map(|k| {
            let s = if k % frame < known {
                let mut kr = Rng::new(0x4285 + (k % frame) as u64);
                kr.below(8)
            } else {
                r.below(8)
            };
            let p = TAU * s as f64 / 8.0;
            (p.cos(), p.sin())
        })
        .collect();
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            let ts = t * baud;
            let k = ts.floor() as i64;
            let (mut re, mut im) = (0.0, 0.0);
            for j in (k - 4).max(0)..=(k + 4) {
                let (a, b) = syms[j as usize];
                let h = rrc(ts - j as f64, 0.35);
                re += a * h;
                im += b * h;
            }
            let p = TAU * centre * t;
            (0.3 * (re * p.cos() - im * p.sin())) as f32
        })
        .collect()
}

/// A Link 11-like signal: 14 tones 935 + 110 n Hz with QPSK phase steps at 75 Bd, a 605 Hz tone at twice and a 2915 Hz tone at equal
/// amplitude to a data tone.
pub fn link11(fs: f64, secs: f64, seed: u64) -> Vec<f32> {
    let n = (fs * secs) as usize;
    let mut r = Rng::new(seed);
    let nsym = (secs * 75.0) as usize + 2;
    let phases: Vec<Vec<f64>> = (0..14)
        .map(|_| {
            let mut p = 0.0;
            (0..nsym)
                .map(|_| {
                    p += PI / 2.0 * r.below(4) as f64 + PI / 4.0;
                    p
                })
                .collect()
        })
        .collect();
    let a = 0.05;
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            let k = (t * 75.0) as usize;
            let mut v = 2.0 * a * (TAU * 605.0 * t).cos() + a * (TAU * 2915.0 * t).cos();
            for (c, ph) in phases.iter().enumerate() {
                v += a * (TAU * (935.0 + 110.0 * c as f64) * t + ph[k]).cos();
            }
            v as f32
        })
        .collect()
}

/// Continuous-phase MFSK: `tones` tones from `first` Hz, `spacing` apart, at `baud`.
pub fn mfsk(
    fs: f64,
    secs: f64,
    tones: usize,
    first: f64,
    spacing: f64,
    baud: f64,
    seed: u64,
) -> Vec<f32> {
    let n = (fs * secs) as usize;
    let mut r = Rng::new(seed);
    let nsym = (secs * baud) as usize + 2;
    let seq: Vec<usize> = (0..nsym).map(|_| r.below(tones as u64) as usize).collect();
    let mut ph = 0.0;
    (0..n)
        .map(|i| {
            let t = i as f64 / fs;
            let f = first + spacing * seq[(t * baud) as usize] as f64;
            ph = (ph + TAU * f / fs) % TAU;
            (0.3 * ph.cos()) as f32
        })
        .collect()
}
