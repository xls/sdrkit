//! Signal helpers shared by the search stacks: "resample a capture to rate R centred on f" on the shared resampler of
//! `sdrkit-algo-dsp`, and a Welch spectrum over a limited number of frames.

use sdrkit_algo_dsp::resample::FractionalResampler;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

/// Shared resamplers by input rate and channel width. A search builds a few and reuses them.
#[derive(Default)]
pub struct ResamplerCache {
    map: HashMap<(u64, u64), FractionalResampler<Complex32>>,
}

/// Clears `out` and fills it with `n_out` samples of `input` (at `fs`): mixed down by `centre_hz`, low-passed to a channel
/// `width_hz` wide (two-sided) and taken at `rate`, from input position `start` with the step made longer by `ppm`. The mixer phase
/// is zero at input sample 0, and input outside the slice counts as zero. If the rates or the width do not make a filter, `out`
/// gets `n_out` zeros.
#[allow(clippy::too_many_arguments)]
pub fn resample_to(
    cache: &mut ResamplerCache,
    input: &[Complex32],
    fs: f64,
    centre_hz: f64,
    rate: f64,
    width_hz: f64,
    start: f64,
    ppm: f64,
    n_out: usize,
    out: &mut Vec<Complex32>,
) {
    out.clear();
    let half = (width_hz / 2.0).min(0.49 * fs);
    let key = (fs.to_bits(), half.to_bits());
    let r = match cache.map.entry(key) {
        Entry::Occupied(e) => e.into_mut(),
        Entry::Vacant(v) => {
            // Flat to 0.6 of the cut-off, 70 dB down from 1.4 times it.
            match FractionalResampler::new(fs, rate, 0.6 * half, 1.4 * half, 70.0) {
                Ok(r) => v.insert(r),
                Err(_) => {
                    out.resize(n_out, Complex32::new(0.0, 0.0));
                    return;
                }
            }
        }
    };
    r.set_rates(fs, rate);
    r.set_correction(1.0 + ppm * 1e-6);
    if r.set_shift(centre_hz).is_err() || r.process_slice(input, start, n_out, out).is_err() {
        out.clear();
        out.resize(n_out, Complex32::new(0.0, 0.0));
    }
}

/// A Welch power spectrum: Hann window, 50 % overlap, at most `max_avg` frames spread evenly over `x`, lowest frequency first (bin 0 is
/// -fs/2). A bin is `|X|^2 / (N^2 x 0.375)`, so the bins add up to the mean power. Empty when `x` holds less than one FFT.
pub fn welch(x: &[Complex32], nfft: usize, max_avg: usize) -> Vec<f32> {
    if x.len() < nfft || nfft < 2 {
        return Vec::new();
    }
    let Ok(mut w) = Welch::new(nfft, Window::Hann, 0.5, Scaling::Power) else {
        return Vec::new();
    };
    let hop = w.hop();
    let segs = w.frames_for(x.len());
    let navg = segs.min(max_avg.max(1));
    // Spread the frames over the whole input.
    let stride = if navg > 1 {
        (segs - 1) as f64 / (navg - 1) as f64
    } else {
        0.0
    };
    let mut acc = vec![0f32; nfft];
    let mut one = vec![0f32; nfft];
    for a in 0..navg {
        let s = ((a as f64 * stride).round() as usize) * hop;
        if w.compute(&x[s..s + nfft], &mut one).is_err() {
            return Vec::new();
        }
        for (p, v) in acc.iter_mut().zip(&one) {
            *p += v;
        }
    }
    let k = 1.0 / navg as f32;
    for p in acc.iter_mut() {
        *p *= k;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn tone(n: usize, f: f64) -> Vec<Complex32> {
        (0..n)
            .map(|i| {
                let p = TAU * f * i as f64;
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect()
    }

    #[test]
    fn resample_to_shifts_passes_and_places_a_tone() {
        // A tone at 100 kHz + 2 kHz in 1 Msps, centred on 100 kHz and taken at 250 ksps from position 1000.5: a 2 kHz tone with the
        // phase of the input at each position, because the mixer phase is zero at input sample 0.
        let x = tone(20000, 0.102);
        let mut c = ResamplerCache::default();
        let mut out = Vec::new();
        resample_to(
            &mut c, &x, 1e6, 1e5, 250e3, 100e3, 1000.5, 0.0, 4000, &mut out,
        );
        assert_eq!(out.len(), 4000);
        for (n, v) in out[..4000 - 100].iter().enumerate() {
            let p = TAU * 0.002 * (1000.5 + 4.0 * n as f64);
            let want = Complex32::new(p.cos() as f32, p.sin() as f32);
            assert!((v - want).norm() < 1e-3, "{n}: {v}");
        }
        // A tone outside the channel is removed.
        let y = tone(20000, 0.3);
        resample_to(&mut c, &y, 1e6, 1e5, 250e3, 100e3, 0.0, 0.0, 4000, &mut out);
        let p: f32 = out[500..3500].iter().map(|c| c.norm_sqr()).sum::<f32>() / 3000.0;
        assert!(p < 1e-6, "{p}");
        // Outside the input: zeros.
        resample_to(&mut c, &x, 1e6, 0.0, 1e6, 100e3, 30000.0, 0.0, 10, &mut out);
        assert!(out.len() == 10 && out.iter().all(|c| c.norm() == 0.0));
    }

    #[test]
    fn welch_finds_a_tone() {
        let x = tone(8192, 0.25);
        let s = welch(&x, 256, 20);
        let peak = (0..256)
            .max_by(|&a, &b| s[a].partial_cmp(&s[b]).unwrap())
            .unwrap();
        assert_eq!(peak, 128 + 64);
    }
}
