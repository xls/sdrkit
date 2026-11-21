//! The D8PSK pieces that the VDL receivers share: the turns that take the phase steps of a known 16-symbol training sequence
//! out of a differential correlation, and that correlation.
//! Standard: ICAO Annex 10 Volume III chapter 6 (D8PSK, 10 500 symbol/s).

use sdrkit_core::Complex32;
use std::f64::consts::FRAC_PI_4;

/// The turns of a training sequence of 16 symbols: entry `k` (1 to 15) is `e^(-j pi/4 step(k))` for the phase step `step(k)`
/// (units of pi/4) from symbol `k - 1` to symbol `k`; entry 0 is 1 and unused.
pub fn turns(step: impl Fn(usize) -> i32) -> [Complex32; 16] {
    let mut r = [Complex32::new(1.0, 0.0); 16];
    for (k, t) in r.iter_mut().enumerate().skip(1) {
        let d = f64::from(step(k)) * FRAC_PI_4;
        *t = Complex32::new(d.cos() as f32, -(d.sin() as f32));
    }
    r
}

/// The differential correlation of 16 symbols `z` with a training sequence: the sum over the 15 phase changes of
/// `z[k] conj(z[k - 1])` turned back by `turns[k]`. Its angle is the carrier phase change per symbol; its magnitude reaches the
/// sum of `|z[k]| |z[k - 1]|` on the sequence.
pub fn correlate(z: &[Complex32; 16], turns: &[Complex32; 16]) -> Complex32 {
    let mut s = Complex32::default();
    for k in 1..16 {
        s += z[k] * z[k - 1].conj() * turns[k];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_known_sequence_correlates_fully() {
        let steps = [0, 3, 6, 1, 1, 2, 7, 4, 0, 5, 2, 3, 1, 6, 5, 3];
        let t = turns(|k| steps[k]);
        let mut z = [Complex32::default(); 16];
        let mut ph = 0.3f64;
        for (k, v) in z.iter_mut().enumerate() {
            ph += f64::from(steps[k]) * FRAC_PI_4 + 0.05;
            *v = Complex32::from_polar(1.0, ph as f32);
        }
        let d = correlate(&z, &t);
        assert!((d.norm() - 15.0).abs() < 1e-3);
        assert!((d.arg() - 0.05).abs() < 1e-4);
    }
}
