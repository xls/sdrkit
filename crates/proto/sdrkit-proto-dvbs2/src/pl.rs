//! The DVB-S2 physical layer framing: the SOF, the PLS code, pi/2-BPSK, pilots and dummy frames, with the PL scrambling, the
//! constellations and the bit interleaver of the shared crates.
//!
//! Standard: ETSI EN 302 307-1 clauses 5.3.3, 5.4 and 5.5.

use super::modcod::{ModCod, Modulation};
use sdrkit_algo_dsp::constellation::{Dvbs2Rate, PointSet};
use sdrkit_algo_fec::interleave::dvbs2_bits;
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::seq::gold_dvbs2;
use std::f32::consts::FRAC_1_SQRT_2;

/// The start of frame, 26 bits.
pub const SOF: u32 = 0x18D_2E82;
/// The scrambling of the 64 PLS bits.
pub const PLS_SCRAMBLE: u64 = sdrkit_util_bits::seq::DVBS2_PLS_SCRAMBLE;
/// Symbols of the PLHEADER.
pub const HEADER: usize = 90;

/// The (32, 6) code word of 6 bits (first bit most significant) of the PLS code.
pub fn pls32(b: u8) -> u32 {
    const G: [u32; 6] = [
        0x5555_5555,
        0x3333_3333,
        0x0F0F_0F0F,
        0x00FF_00FF,
        0x0000_FFFF,
        0xFFFF_FFFF,
    ];
    let mut w = 0;
    for (i, g) in G.iter().enumerate() {
        if (b >> (5 - i)) & 1 == 1 {
            w ^= g;
        }
    }
    w
}

/// The 64 scrambled PLS bits (first bit most significant) of 7 signalling bits: MODCOD (5), short frame, pilots.
pub fn pls_code(code7: u8) -> u64 {
    let w = pls32(code7 >> 1);
    let b7 = (code7 & 1) as u64;
    let mut out = 0u64;
    for i in (0..32).rev() {
        let y = ((w >> i) & 1) as u64;
        out = (out << 2) | (y << 1) | (y ^ b7);
    }
    out ^ PLS_SCRAMBLE
}

/// The 7 signalling bits of a frame.
pub fn code7(modcod: u8, short: bool, pilots: bool) -> u8 {
    (modcod << 2) | ((short as u8) << 1) | pilots as u8
}

/// pi/2-BPSK of bit `b` at position `i` (from the first header symbol).
#[inline]
pub fn half_pi_bpsk(b: u8, i: usize) -> Complex32 {
    let s = if b == 0 {
        FRAC_1_SQRT_2
    } else {
        -FRAC_1_SQRT_2
    };
    if i.is_multiple_of(2) {
        Complex32::new(s, s)
    } else {
        Complex32::new(-s, s)
    }
}

/// The 90 PLHEADER symbols of 7 signalling bits.
pub fn header_symbols(code7: u8) -> [Complex32; HEADER] {
    let mut h = [Complex32::new(0.0, 0.0); HEADER];
    let pls = pls_code(code7);
    for (i, s) in h.iter_mut().enumerate() {
        let b = if i < 26 {
            (SOF >> (25 - i)) & 1
        } else {
            ((pls >> (63 - (i - 26))) & 1) as u32
        };
        *s = half_pi_bpsk(b as u8, i);
    }
    h
}

/// A pilot symbol (and the symbols of a dummy frame).
pub const PILOT: Complex32 = Complex32::new(FRAC_1_SQRT_2, FRAC_1_SQRT_2);

/// The PL scrambling sequence of Gold code `n` (each value 0 to 3: the symbol is turned by that many quarter turns), for the
/// `len` symbols after a PLHEADER, from `sdrkit_util_bits::seq::gold_dvbs2`. A code number above 262 141 selects code 0.
pub fn pl_scrambling(n: u32, len: usize, out: &mut Vec<u8>) {
    out.clear();
    out.resize(len, 0);
    if gold_dvbs2(n, 0, out).is_err() {
        let _ = gold_dvbs2(0, 0, out);
    }
}

/// Turns `s` by `r` quarter turns (scrambling: R = 1 multiplies by j).
#[inline]
pub fn quarter(s: Complex32, r: u8) -> Complex32 {
    match r & 3 {
        0 => s,
        1 => Complex32::new(-s.im, s.re),
        2 => -s,
        _ => Complex32::new(s.im, -s.re),
    }
}

fn dvbs2_rate(rate: (u16, u16)) -> Dvbs2Rate {
    match rate {
        (2, 3) => Dvbs2Rate::R2_3,
        (3, 4) => Dvbs2Rate::R3_4,
        (4, 5) => Dvbs2Rate::R4_5,
        (5, 6) => Dvbs2Rate::R5_6,
        (8, 9) => Dvbs2Rate::R8_9,
        _ => Dvbs2Rate::R9_10,
    }
}

/// The constellation of a MODCOD (EN 302 307-1 clause 5.4) from `sdrkit_algo_dsp::constellation`, with the ring ratios of its code
/// rate, points by label at unit mean energy.
pub fn point_set(m: &ModCod) -> PointSet {
    let r = dvbs2_rate(m.rate);
    let set = match m.modulation {
        Modulation::Qpsk => Ok(PointSet::qpsk()),
        Modulation::Psk8 => Ok(PointSet::dvbs2_8psk()),
        Modulation::Apsk16 => PointSet::dvbs2_16apsk(r.gamma_16apsk()),
        Modulation::Apsk32 => {
            let (g1, g2) = r.gammas_32apsk().unwrap_or((2.53, 4.30));
            PointSet::dvbs2_32apsk(g1, g2)
        }
    };
    set.unwrap_or_else(|_| PointSet::qpsk())
}

/// The points of a MODCOD's constellation by label.
pub fn constellation(m: &ModCod) -> Vec<Complex32> {
    point_set(m).points().to_vec()
}

/// The bit interleaver of clause 5.3.3 (`sdrkit_algo_fec::interleave::dvbs2_bits`) for the `n` bits of a frame, as the map from a
/// received position to the code word position. QPSK has none.
pub fn deinterleaver(m: &ModCod, n: usize) -> Option<Permutation> {
    let cols = m.modulation.bits();
    if cols == 2 {
        return None;
    }
    let reverse = m.modulation == Modulation::Psk8 && m.rate == (3, 5);
    dvbs2_bits(n, cols, reverse).ok().map(Permutation::inverse)
}

/// The bit interleaver of clause 5.3.3 for the generator: the map from a code word position to the sent position.
pub fn interleaver(m: &ModCod, n: usize) -> Option<Permutation> {
    let cols = m.modulation.bits();
    if cols == 2 {
        return None;
    }
    let reverse = m.modulation == Modulation::Psk8 && m.rate == (3, 5);
    dvbs2_bits(n, cols, reverse).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pls_code_words_are_distinct_and_the_header_starts_with_the_sof() {
        let mut seen = std::collections::BTreeSet::new();
        for c in 0..128u8 {
            assert!(seen.insert(pls_code(c)));
        }
        // Minimum distance of the (64, 7) code is 32.
        let a = pls_code(code7(5, false, false));
        let b = pls_code(code7(5, false, true));
        assert_eq!((a ^ b).count_ones(), 32);
        let h = header_symbols(code7(5, false, false));
        assert!((h[0] - half_pi_bpsk(0, 0)).norm() < 1e-6);
        assert!((h[1] - half_pi_bpsk(1, 1)).norm() < 1e-6);
        assert!((h[3] - half_pi_bpsk(0, 3)).norm() < 1e-6);
    }

    #[test]
    fn constellations_have_unit_energy_and_distinct_points() {
        for m in ModCod::all() {
            let c = constellation(&m);
            let e: f32 = c.iter().map(|p| p.norm_sqr()).sum::<f32>() / c.len() as f32;
            assert!((e - 1.0).abs() < 1e-4);
            for i in 0..c.len() {
                for j in 0..i {
                    assert!((c[i] - c[j]).norm() > 0.05, "{} {i} {j}", m.name());
                }
            }
        }
    }

    #[test]
    fn gold_code_zero_is_not_constant() {
        let mut r = Vec::new();
        pl_scrambling(0, 1000, &mut r);
        // z(0) = x(0) + y(0) = 0.
        assert_eq!(r[0] & 1, 0);
        assert!(r.iter().any(|&v| v != r[0]));
    }
}
