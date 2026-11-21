//! Max-log soft bits and their quantisation for the Viterbi decoder.

use sdrkit_core::Complex32;

/// Max-log soft bits of `y` against the labelled points `points` (label `l` at `points[l]`, `bits` bits a label, the first bit
/// the most significant): for each bit, `(min |y - p|^2 over labels with the bit 1 - min over labels with it 0) / sigma2`.
/// Positive favours 0. Writes `bits` values to `out`.
pub fn max_log(y: Complex32, points: &[Complex32], bits: usize, sigma2: f32, out: &mut [f32]) {
    let mut d0 = [f32::INFINITY; 8];
    let mut d1 = [f32::INFINITY; 8];
    for (l, p) in points.iter().enumerate() {
        let d = (y - p).norm_sqr();
        for b in 0..bits {
            if (l >> (bits - 1 - b)) & 1 == 0 {
                d0[b] = d0[b].min(d);
            } else {
                d1[b] = d1[b].min(d);
            }
        }
    }
    let s = 1.0 / sigma2.max(1e-6);
    for b in 0..bits {
        out[b] = (d1[b] - d0[b]) * s;
    }
}

/// A soft value as a Viterbi LLR: scaled by `scale`, rounded, clamped to +-127 (0 for a value that is not finite).
pub fn llr8(v: f32, scale: f32) -> i8 {
    let x = v * scale;
    if x.is_finite() {
        x.round().clamp(-127.0, 127.0) as i8
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::psk8;

    #[test]
    fn signs_follow_the_labels() {
        let pts: Vec<Complex32> = (0..4).map(|l| psk8([0, 2, 6, 4][l])).collect();
        let mut o = [0.0; 2];
        max_log(psk8(6), &pts, 2, 0.1, &mut o);
        assert!(o[0] < 0.0 && o[1] > 0.0);
        assert_eq!(llr8(1e9, 1.0), 127);
        assert_eq!(llr8(f32::NAN, 1.0), 0);
    }
}
