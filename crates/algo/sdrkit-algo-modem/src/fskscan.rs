//! Candidates for a start-stop 2-FSK signal (a teleprinter) of unknown tones, shift and speed, for trial decoding.
//!
//! | Step | What it does |
//! |---|---|
//! | Tone pairs | averaged Hann spectrum; a line is a local maximum at least 4 times the median of the band, refined by a parabola through the logarithms of three bins; two lines whose spacing is within a tolerance of a listed shift form a pair, scored by the weaker line over the median; near copies and pairs under a share of the best are dropped |
//! | Speed | the stronger of the two tones over a short moving sum gives a state per sample; the run lengths of one state score each listed speed by the mean of `cos(2 pi L v 2)` (the half-unit grid of a 1.5-unit stop element) |
//!
//! The input is complex with the signal at positive frequencies (an audio tone of f Hz at +f Hz).
//!
//! ```
//! use sdrkit_algo_modem::fskscan::{run_speed, tone_pairs, PairScan};
//! use sdrkit_algo_modem::Complex32;
//! let rate = 8000.0;
//! // Characters of a start unit, five random units and a 1.5-unit stop at 50 Bd on 1000 Hz (mark) and 1170 Hz.
//! let (mut x, mut ph, mut r) = (Vec::new(), 0.0f64, 1u32);
//! let mut send = |mark: bool, units: f64, x: &mut Vec<Complex32>| {
//!     let f = if mark { 1000.0 } else { 1170.0 };
//!     for _ in 0..(units * rate / 50.0) as usize {
//!         ph += std::f64::consts::TAU * f / rate;
//!         x.push(Complex32::from_polar(1.0, ph as f32));
//!     }
//! };
//! while x.len() < 48_000 {
//!     r = r.wrapping_mul(1_103_515_245).wrapping_add(12_345);
//!     send(false, 1.0, &mut x);
//!     for b in 0..5 {
//!         send((r >> (16 + b)) & 1 == 1, 1.0, &mut x);
//!     }
//!     send(true, 1.5, &mut x);
//! }
//! let pairs = tone_pairs(&x, &PairScan::new(rate, &[170.0, 425.0, 850.0]));
//! assert!((pairs[0].lower_hz - 1000.0).abs() < 10.0 && pairs[0].shift_hz == 170.0);
//! let speed = run_speed(&x, rate, pairs[0].lower_hz, 170.0, &[45.45, 50.0, 75.0, 100.0]).unwrap();
//! assert_eq!(speed.order[0], 50.0);
//! ```

use crate::Complex32;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use std::f64::consts::TAU;

/// The settings of [`tone_pairs`].
#[derive(Clone, Debug)]
pub struct PairScan {
    /// The sample rate, Hz.
    pub rate: f64,
    /// The shifts a pair may have, Hz.
    pub shifts_hz: Vec<f64>,
    /// The band both tones lie in, Hz.
    pub band_hz: (f64, f64),
    /// How far the spacing of two lines may be from the nearest listed shift, Hz.
    pub shift_tol_hz: f64,
    /// A line is at least this many times the median power of the band.
    pub line_over_floor: f64,
    /// A pair whose lower tone is this close to a better pair of the same shift is dropped, Hz.
    pub same_pair_hz: f64,
    /// A pair scoring under this share of the best is dropped.
    pub min_share: f64,
    /// The most pairs returned.
    pub max_pairs: usize,
}

impl PairScan {
    /// A scan at `rate` for the shifts `shifts_hz`: band 300 to 3300 Hz, tolerance 10 Hz, lines 4 times the median, near copies
    /// within 25 Hz, pairs under 0.15 of the best dropped, at most 4 pairs.
    pub fn new(rate: f64, shifts_hz: &[f64]) -> Self {
        PairScan {
            rate,
            shifts_hz: shifts_hz.to_vec(),
            band_hz: (300.0, 3300.0),
            shift_tol_hz: 10.0,
            line_over_floor: 4.0,
            same_pair_hz: 25.0,
            min_share: 0.15,
            max_pairs: 4,
        }
    }
}

/// A tone pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TonePair {
    /// The lower tone, Hz.
    pub lower_hz: f64,
    /// The listed shift nearest to the spacing of the two lines, Hz.
    pub shift_hz: f64,
    /// The weaker line over the median power of the band.
    pub score: f64,
}

/// The tone pairs of `x`, best first (see the module documentation). The transform has about 0.256 s of samples (2048 at
/// 8 kHz) and moves by half of that; `x` shorter than one transform gives none.
pub fn tone_pairs(x: &[Complex32], scan: &PairScan) -> Vec<TonePair> {
    let n = ((scan.rate * 0.256).round() as usize)
        .next_power_of_two()
        .max(64);
    let hop = n / 2;
    if x.len() < n {
        return Vec::new();
    }
    let Ok(mut fft) = WindowedFft::new(n, n, Window::Hann) else {
        return Vec::new();
    };
    let mut avg = vec![0.0f64; n];
    let mut frames = 0usize;
    let mut start = 0;
    while start + n <= x.len() {
        for (a, &p) in avg.iter_mut().zip(fft.run_power(&x[start..start + n])) {
            *a += f64::from(p);
        }
        frames += 1;
        start += hop;
    }
    avg.iter_mut().for_each(|a| *a /= frames as f64);
    let bin = scan.rate / n as f64;
    let lo = ((scan.band_hz.0 / bin).ceil() as usize).max(1);
    let hi = ((scan.band_hz.1 / bin).floor() as usize).min(n / 2 - 2);
    if hi <= lo + 2 {
        return Vec::new();
    }
    let mut band: Vec<f64> = avg[lo..=hi].to_vec();
    let mid = band.len() / 2;
    let floor = *band.select_nth_unstable_by(mid, f64::total_cmp).1;
    let floor = floor.max(1e-30);
    let mut lines: Vec<(f64, f64)> = Vec::new();
    for i in lo..=hi {
        let (a, b, c) = (avg[i - 1], avg[i], avg[i + 1]);
        if b < a || b <= c || b < scan.line_over_floor * floor {
            continue;
        }
        let (la, lb, lc) = (a.max(1e-30).ln(), b.ln(), c.max(1e-30).ln());
        let den = la - 2.0 * lb + lc;
        let d = if den.abs() > 1e-12 {
            (0.5 * (la - lc) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        lines.push(((i as f64 + d) * bin, b / floor));
    }
    let mut pairs = Vec::new();
    for (i, &(f0, p0)) in lines.iter().enumerate() {
        for &(f1, p1) in &lines[i + 1..] {
            let spacing = f1 - f0;
            let Some(&shift) = scan
                .shifts_hz
                .iter()
                .min_by(|a, b| (*a - spacing).abs().total_cmp(&(*b - spacing).abs()))
            else {
                continue;
            };
            if (spacing - shift).abs() <= scan.shift_tol_hz {
                pairs.push(TonePair {
                    lower_hz: f0,
                    shift_hz: shift,
                    score: p0.min(p1),
                });
            }
        }
    }
    pairs.sort_by(|a, b| b.score.total_cmp(&a.score));
    let best = pairs.first().map_or(0.0, |p| p.score);
    let mut kept: Vec<TonePair> = Vec::new();
    for p in pairs {
        if kept.len() >= scan.max_pairs || p.score < scan.min_share * best {
            break;
        }
        let copy = kept.iter().any(|k| {
            k.shift_hz == p.shift_hz && (k.lower_hz - p.lower_hz).abs() <= scan.same_pair_hz
        });
        if !copy {
            kept.push(p);
        }
    }
    kept
}

/// The speeds to try for one tone pair, from [`run_speed`].
#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGuess {
    /// The speeds to try, best first: the best and the second when the confidence is at least 0.2, else every listed speed.
    pub order: Vec<f64>,
    /// `(best - second) / max(0.05, |best|)`, limited to 0 to 1.
    pub confidence: f64,
}

/// The speed of a start-stop signal on the tones `lower_hz` and `lower_hz + shift_hz`, from the lengths of its runs of one tone,
/// among `speeds` (Bd). The moving sum is 3.5 ms (at least 8 samples); a run shorter than 0.4 / 110 s is added to the next one.
/// `None` with fewer than 8 runs or no listed speed.
pub fn run_speed(
    x: &[Complex32],
    rate: f64,
    lower_hz: f64,
    shift_hz: f64,
    speeds: &[f64],
) -> Option<SpeedGuess> {
    if speeds.is_empty() {
        return None;
    }
    let l = ((0.0035 * rate).round() as usize).max(8);
    let rot = |f: f64| Complex32::from_polar(1.0, (-TAU * f / rate) as f32);
    let (r0, r1) = (rot(lower_hz), rot(lower_hz + shift_hz));
    let (mut p0, mut p1) = (Complex32::new(1.0, 0.0), Complex32::new(1.0, 0.0));
    let (mut m0, mut m1) = (Vec::with_capacity(x.len()), Vec::with_capacity(x.len()));
    for (i, &v) in x.iter().enumerate() {
        m0.push(v * p0);
        m1.push(v * p1);
        p0 *= r0;
        p1 *= r1;
        if i % 1024 == 1023 {
            let ph = |f: f64| {
                Complex32::from_polar(
                    1.0,
                    (-TAU * f / rate * (i + 1) as f64).rem_euclid(TAU) as f32,
                )
            };
            p0 = ph(lower_hz);
            p1 = ph(lower_hz + shift_hz);
        }
    }
    let shortest = (0.4 / 110.0 * rate).round() as usize;
    let (mut s0, mut s1) = (Complex32::default(), Complex32::default());
    let mut runs: Vec<usize> = Vec::new();
    let (mut state, mut len, mut carry) = (None::<bool>, 0usize, 0usize);
    for i in 0..x.len() {
        s0 += m0[i];
        s1 += m1[i];
        if i >= l {
            s0 -= m0[i - l];
            s1 -= m1[i - l];
        }
        if i + 1 < l {
            continue;
        }
        let s = s1.norm_sqr() > s0.norm_sqr();
        if state == Some(s) {
            len += 1;
            continue;
        }
        if state.is_some() {
            let run = len + carry;
            if run < shortest {
                carry = run;
            } else {
                runs.push(run);
                carry = 0;
            }
        }
        state = Some(s);
        len = 1;
    }
    if runs.len() < 8 {
        return None;
    }
    let mut scored: Vec<(f64, f64)> = speeds
        .iter()
        .map(|&v| {
            let m = runs
                .iter()
                .map(|&r| (TAU * r as f64 / rate * v * 2.0).cos())
                .sum::<f64>()
                / runs.len() as f64;
            (v, m)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    let best = scored[0].1;
    let second = scored.get(1).map_or(-1.0, |s| s.1);
    let confidence = ((best - second) / best.abs().max(0.05)).clamp(0.0, 1.0);
    let order = if confidence >= 0.2 {
        scored.iter().take(2).map(|s| s.0).collect()
    } else {
        scored.iter().map(|s| s.0).collect()
    };
    Some(SpeedGuess { order, confidence })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(rate: f64, baud: f64, f0: f64, shift: f64, codes: &[u8]) -> Vec<Complex32> {
        let mut x = Vec::new();
        let mut ph = 0.0f64;
        let mut tone = |mark: bool, units: f64, x: &mut Vec<Complex32>| {
            let f = if mark { f0 } else { f0 + shift };
            for _ in 0..(units * rate / baud).round() as usize {
                ph += TAU * f / rate;
                x.push(Complex32::from_polar(1.0, ph as f32));
            }
        };
        for &c in codes {
            tone(false, 1.0, &mut x);
            for b in 0..5 {
                tone((c >> b) & 1 == 1, 1.0, &mut x);
            }
            tone(true, 1.5, &mut x);
        }
        x
    }

    #[test]
    fn finds_pair_and_speed_of_a_start_stop_signal() {
        let mut r = 7u32;
        let codes: Vec<u8> = (0..300)
            .map(|_| {
                r = r.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (r >> 16) as u8 & 31
            })
            .collect();
        for (baud, shift) in [(45.45, 170.0), (75.0, 450.0), (100.0, 850.0)] {
            let x = keyed(8000.0, baud, 1200.0, shift, &codes);
            let pairs = tone_pairs(
                &x[..64_000],
                &PairScan::new(8000.0, &[170.0, 200.0, 425.0, 450.0, 850.0]),
            );
            assert_eq!(pairs[0].shift_hz, shift, "{baud}/{shift}: {pairs:?}");
            assert!((pairs[0].lower_hz - 1200.0).abs() < 20.0, "{pairs:?}");
            let s = run_speed(
                &x[..64_000],
                8000.0,
                1200.0,
                shift,
                &[45.45, 50.0, 75.0, 100.0],
            )
            .unwrap();
            assert_eq!(s.order[0], baud, "{s:?}");
        }
    }

    #[test]
    fn a_single_tone_is_not_a_pair() {
        let mut s = 1u32;
        let mut g = move || {
            s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (s >> 8) as f32 / 16_777_216.0 - 0.5
        };
        let x: Vec<Complex32> = (0..16_000)
            .map(|i| {
                Complex32::from_polar(1.0, (TAU * 1500.0 * i as f64 / 8000.0) as f32)
                    + Complex32::new(g(), g()) * 0.3
            })
            .collect();
        assert!(tone_pairs(&x, &PairScan::new(8000.0, &[170.0, 850.0])).is_empty());
    }
}
