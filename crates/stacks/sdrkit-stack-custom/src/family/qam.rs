//! The Gray QAM map of the family: BPSK (bit 0 is +1), and square QPSK to 1024-QAM whose first half of the label bits gives the
//! I level and the second half the Q level. On each axis the first label bit is the sign (0 positive) and the levels
//! `L - 1 - 2 i` carry the Gray code of `i`. Every set has unit mean power.

use sdrkit_algo_dsp::constellation::{AxisFlip, PointSet, QamLayout, QamMap};
use sdrkit_algo_modem::Constellation;

/// The square QAM map of the family.
pub const FAMILY_MAP: QamMap = QamMap {
    layout: QamLayout::Split,
    flip: AxisFlip::Msb,
};

/// The point set with `bits` bits a symbol (1, 2, 4, 6, 8 or 10); `None` for another count.
pub fn point_set(bits: usize) -> Option<PointSet> {
    match bits {
        1 => Some(PointSet::bpsk()),
        2 | 4 | 6 | 8 | 10 => PointSet::qam(1 << bits, FAMILY_MAP).ok(),
        _ => None,
    }
}

/// The slicer and soft demapper of `bits` bits a symbol.
pub fn constellation(bits: usize) -> Option<Constellation> {
    Constellation::new(point_set(bits)?).ok()
}

/// Maps `bits` (a multiple of `per` long, else the last symbol is filled with zero bits) to symbols of `per` bits each.
pub fn map(set: &PointSet, per: usize, bits: &[u8], out: &mut Vec<sdrkit_core::Complex32>) {
    let pts = set.points();
    for c in bits.chunks(per) {
        let mut label = 0usize;
        for i in 0..per {
            label = (label << 1) | usize::from(c.get(i).copied().unwrap_or(0) & 1);
        }
        out.push(pts[label]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_family_rule() {
        let s = point_set(4).unwrap();
        let k = 10f32.sqrt();
        // I labels 00, 01, 11, 10 give +3, +1, -1, -3; Q the same on the last two bits.
        for (label, re) in [(0b0000, 3.0), (0b0100, 1.0), (0b1100, -1.0), (0b1000, -3.0)] {
            assert!((s.points()[label].re * k - re).abs() < 1e-5, "{label}");
            assert!((s.points()[label].im * k - 3.0).abs() < 1e-5);
        }
        let b = point_set(1).unwrap();
        assert!(b.points()[0].re > 0.0 && b.points()[1].re < 0.0);
        for bits in [1, 2, 4, 6, 8, 10] {
            let c = constellation(bits).unwrap();
            assert_eq!(c.bits(), bits);
            let p = point_set(bits).unwrap();
            let e: f32 = p.points().iter().map(|z| z.norm_sqr()).sum::<f32>() / p.points().len() as f32;
            assert!((e - 1.0).abs() < 1e-4, "{bits}");
        }
        assert!(point_set(3).is_none());
    }
}
