//! Point sets of unit mean energy with Gray labels, for [`crate::modulate::Linear`]: M-PSK, square QAM, PAM and APSK rings, as
//! plain vectors of the definitions in [`sdrkit_algo_dsp::constellation`]. Square QAM uses the IEEE 802.11 map
//! ([`QamMap::IEEE_802_11`]): the first half of the bits label the I level and the second half the Q level, each by Gray over
//! the levels in increasing order (clause 17.3.5.8). Other maps come from [`PointSet`] directly.

pub use sdrkit_algo_dsp::constellation::{PointSet, QamMap, Ring};

use crate::{Param, SimError};
use sdrkit_core::Complex32;

/// M-PSK on the unit circle, the point at `phase0 + 2 pi j / M` labelled `gray(j)`.
///
/// ```
/// use sdrkit_algo_sim::points::psk;
/// let q = psk(4, std::f64::consts::FRAC_PI_4).unwrap();
/// // Label 0 at 45 degrees, label 1 (gray(1) = 1) at 135, label 3 (gray(2)) at 225, label 2 (gray(3)) at 315.
/// assert!(q[3].re < 0.0 && q[3].im < 0.0);
/// ```
pub fn psk(m: usize, phase0: f64) -> Result<Vec<Complex32>, SimError> {
    Ok(PointSet::psk(m, phase0)?.points().to_vec())
}

/// The `m` PAM levels `-(m-1), ..., -1, +1, ..., m-1` scaled to unit mean energy, indexed by Gray label.
pub fn pam(m: usize) -> Result<Vec<f32>, SimError> {
    Ok(PointSet::pam(m)?.points().iter().map(|p| p.re).collect())
}

/// Square `m`-QAM (4, 16, 64, 256, 1024, ...: an even number of bits) of unit mean energy with the IEEE 802.11 map (module
/// documentation).
pub fn qam(m: usize) -> Result<Vec<Complex32>, SimError> {
    Ok(PointSet::qam(m, QamMap::IEEE_802_11)?.points().to_vec())
}

/// APSK from rings, scaled to unit mean energy (equal use of every point), numbered ring by ring and round each ring.
///
/// ```
/// use sdrkit_algo_sim::points::{apsk, Ring};
/// use std::f64::consts::PI;
/// // The DVB-S2 16APSK geometry: 4 + 12 points, ring ratio 2.85 (rate 3/4).
/// let p = apsk(&[Ring { points: 4, radius: 1.0, phase: PI / 4.0 }, Ring { points: 12, radius: 2.85, phase: PI / 12.0 }]).unwrap();
/// assert_eq!(p.len(), 16);
/// ```
pub fn apsk(rings: &[Ring]) -> Result<Vec<Complex32>, SimError> {
    Ok(PointSet::apsk(rings)?.points().to_vec())
}

/// Maps bits (0 or 1 per byte, most significant first in each label) to `points`, appending one point per `log2(points.len())`
/// bits. Errors when the point count is not a power of two or the bits do not fill whole labels.
pub fn map_bits(
    points: &[Complex32],
    bits: &[u8],
    out: &mut Vec<Complex32>,
) -> Result<(), SimError> {
    let m = points.len();
    if !(m >= 2 && m.is_power_of_two()) {
        return Param::Points.not_allowed(m as f64);
    }
    let b = m.trailing_zeros() as usize;
    if !bits.len().is_multiple_of(b) {
        return Param::Bits.not_allowed(bits.len() as f64);
    }
    for label in bits.chunks_exact(b) {
        let v = label
            .iter()
            .fold(0usize, |a, &x| (a << 1) | (x != 0) as usize);
        out.push(points[v]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn energy(p: &[Complex32]) -> f64 {
        p.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / p.len() as f64
    }

    #[test]
    fn qam16_is_the_ieee80211_table() {
        // IEEE 802.11-2016 clause 17.3.5.8, the 16-QAM encoding table: b0b1 -> I, b2b3 -> Q: 00 -3, 01 -1, 11 +1, 10 +3, times 1/sqrt(10).
        let p = qam(16).unwrap();
        let level = |two: usize| match two {
            0b00 => -3.0,
            0b01 => -1.0,
            0b11 => 1.0,
            _ => 3.0,
        };
        for (label, v) in p.iter().enumerate() {
            let want = Complex32::new(level(label >> 2), level(label & 3)) / 10f32.sqrt();
            assert!((v - want).norm() < 1e-6, "label {label:04b}");
        }
        // QPSK of the same table: b0 -> I, b1 -> Q, 0 -> -1, 1 -> +1, times 1/sqrt(2).
        let q = qam(4).unwrap();
        assert!((q[0b10] - Complex32::new(1.0, -1.0) / 2f32.sqrt()).norm() < 1e-6);
        // 64-QAM: b0b1b2 = 000, 001, 011, 010, 110, 111, 101, 100 give -7 .. +7 (the 64-QAM table).
        let q64 = qam(64).unwrap();
        let order = [0b000usize, 0b001, 0b011, 0b010, 0b110, 0b111, 0b101, 0b100];
        for (k, &l) in order.iter().enumerate() {
            let want = (2 * k as i32 - 7) as f32 / 42f32.sqrt();
            assert!((q64[l << 3].re - want).abs() < 1e-6);
        }
    }

    #[test]
    fn every_set_has_unit_energy_and_gray_neighbours() {
        for m in [4usize, 16, 64, 256, 1024] {
            assert!((energy(&qam(m).unwrap()) - 1.0).abs() < 1e-6, "{m}-QAM");
        }
        for m in [2usize, 4, 8, 16, 32] {
            let p = psk(m, 0.3).unwrap();
            assert!((energy(&p) - 1.0).abs() < 1e-6);
            // Neighbours on the circle differ in exactly one bit.
            let step = TAU / m as f64;
            for a in 0..m {
                for b in 0..m {
                    let d = (p[a] - p[b]).norm() as f64;
                    if (d - 2.0 * (step / 2.0).sin()).abs() < 1e-5 && m > 2 {
                        assert_eq!((a ^ b).count_ones(), 1, "{m}-PSK labels {a} {b}");
                    }
                }
            }
        }
        let a = pam(8).unwrap();
        let e: f64 = a.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / 8.0;
        assert!((e - 1.0).abs() < 1e-6);
        let ap = apsk(&[
            Ring {
                points: 4,
                radius: 1.0,
                phase: 0.0,
            },
            Ring {
                points: 12,
                radius: 2.7,
                phase: 0.1,
            },
            Ring {
                points: 16,
                radius: 4.6,
                phase: 0.0,
            },
        ])
        .unwrap();
        assert_eq!(ap.len(), 32);
        assert!((energy(&ap) - 1.0).abs() < 1e-6);
        // Ring ratios are kept.
        assert!((ap[4].norm() / ap[0].norm() - 2.7).abs() < 1e-5);
        assert!(qam(32).is_err() && psk(3, 0.0).is_err() && apsk(&[]).is_err());
    }

    #[test]
    fn bits_map_most_significant_first() {
        let p = qam(16).unwrap();
        let mut out = Vec::new();
        map_bits(&p, &[1, 0, 1, 1, 0, 0, 0, 1], &mut out).unwrap();
        assert_eq!(out, vec![p[0b1011], p[0b0001]]);
        assert!(map_bits(&p, &[1, 0, 1], &mut out).is_err());
    }
}
