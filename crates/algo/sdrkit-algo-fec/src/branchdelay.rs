//! The branch delays of the DVB-SH time interleaver: the five signalled parameters (ETSI EN 302 583 clause 5.4.3, table 5.38),
//! their 32-bit form, and the delay of each of the 48 branches by the rule of ETSI TS 102 584 clause 7.2.3.3 (figure 7.12). The
//! delays are given from the receiver's side, in cells of one interleaving unit; [`crate::blockinterleave::ConvBlocks`] runs
//! them, and `ConvBlocks::transmit_delays` gives the transmitter's.

use crate::error::{FecError, Param, Result};

/// Branches of the DVB-SH time interleaver.
pub const SH_BRANCHES: usize = 48;
/// Bits of one interleaving unit (IU).
pub const SH_UNIT_BITS: usize = 126;

/// The five parameters of the DVB-SH time interleaver.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShTimeInterleaver {
    /// common_multiplier, 1 to 63.
    pub common_multiplier: u8,
    /// nof_late_taps, 0 to 48.
    pub late_taps: u8,
    /// nof_slices, 1 to 63.
    pub slices: u8,
    /// slice_distance in SH frames, 0 to 255.
    pub slice_distance: u8,
    /// non_late_increment, 0 to 63.
    pub non_late_increment: u8,
}

impl Default for ShTimeInterleaver {
    /// The uniform profile 4 / 48 / 1 / 0 / 0: branch `j` delays by `4 j` cells.
    fn default() -> Self {
        ShTimeInterleaver {
            common_multiplier: 4,
            late_taps: 48,
            slices: 1,
            slice_distance: 0,
            non_late_increment: 0,
        }
    }
}

impl ShTimeInterleaver {
    /// The parameters from the 32 bits B1 .. B32 (B1 the most significant bit of `b`): B1-B6 common_multiplier, B7-B12
    /// nof_late_taps, B13-B18 nof_slices, B19-B26 slice_distance, B27-B32 non_late_increment.
    pub fn from_bits(b: u32) -> Self {
        ShTimeInterleaver {
            common_multiplier: (b >> 26 & 63) as u8,
            late_taps: (b >> 20 & 63) as u8,
            slices: (b >> 14 & 63) as u8,
            slice_distance: (b >> 6 & 255) as u8,
            non_late_increment: (b & 63) as u8,
        }
    }

    /// The 32 bits B1 .. B32 of the parameters (inverse of [`ShTimeInterleaver::from_bits`] for values in range).
    pub fn to_bits(&self) -> u32 {
        (self.common_multiplier as u32 & 63) << 26
            | (self.late_taps as u32 & 63) << 20
            | (self.slices as u32 & 63) << 14
            | (self.slice_distance as u32) << 6
            | self.non_late_increment as u32 & 63
    }

    /// The receiver's delay of each branch in cells. `slice_unit` is the SH frame capacity in interleaving units divided by
    /// 48 (272 for OFDM); the slice offset is `slice_unit x slice_distance`.
    ///
    /// Branches `0 .. late_taps` delay by `j x common_multiplier`. The other branches form `nnl` non-late slices of `tps`
    /// branches each (`nnl = nof_slices` without late taps, else `nof_slices - 1`); tap `i` of slice `s` delays by
    /// `(s + nls) x slice offset + i x common_multiplier x non_late_increment`, where `nls` is 1 when there are late taps.
    ///
    /// # Errors
    ///
    /// [`FecError::OutOfRange`] for a common_multiplier of 0, more than 48 late taps or no slice; [`FecError::Conflict`]
    /// when the branches that are not late do not divide evenly among the slices.
    pub fn delays(&self, slice_unit: u32) -> Result<[u32; SH_BRANCHES]> {
        let cm = self.common_multiplier as u32;
        let lt = self.late_taps as usize;
        if cm == 0 || cm > 63 {
            return Param::Depth.range(1, 63, cm);
        }
        if lt > SH_BRANCHES {
            return Param::Depth.range(0, SH_BRANCHES as i64, lt);
        }
        let mut d = [0u32; SH_BRANCHES];
        for (j, v) in d.iter_mut().enumerate().take(lt) {
            *v = j as u32 * cm;
        }
        if lt == SH_BRANCHES {
            return Ok(d);
        }
        let ns = self.slices as usize;
        let nnl = if lt == 0 { ns } else { ns.saturating_sub(1) };
        if ns == 0 || nnl == 0 {
            return Param::Depth.range(1, 63, ns);
        }
        if !(SH_BRANCHES - lt).is_multiple_of(nnl) {
            return Err(FecError::Conflict {
                param: Param::Depth,
                with: Param::Columns,
            });
        }
        let taps = (SH_BRANCHES - lt) / nnl;
        let nls = (ns - nnl) as u32;
        let offset = slice_unit * self.slice_distance as u32;
        let step = cm * self.non_late_increment as u32;
        for s in 0..nnl {
            for i in 0..taps {
                d[lt + s * taps + i] = (s as u32 + nls) * offset + i as u32 * step;
            }
        }
        Ok(d)
    }

    /// The largest branch delay `L_max` in cells.
    ///
    /// # Errors
    ///
    /// As [`ShTimeInterleaver::delays`].
    pub fn max_delay(&self, slice_unit: u32) -> Result<u32> {
        Ok(self.delays(slice_unit)?.into_iter().max().unwrap_or(0))
    }

    /// Whether a class 1 receiver can de-interleave the profile at code rate `num / den` (EN 302 583 clause 5.7.4.3.2.13): late
    /// taps and `rate x 48 / late_taps < 1`.
    pub fn class1(&self, num: u32, den: u32) -> bool {
        let lt = self.late_taps as u32;
        lt > 0 && num * (SH_BRANCHES as u32) < den * lt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockinterleave::ConvBlocks;

    fn ti(cm: u8, lt: u8, ns: u8, sd: u8, nli: u8) -> ShTimeInterleaver {
        ShTimeInterleaver {
            common_multiplier: cm,
            late_taps: lt,
            slices: ns,
            slice_distance: sd,
            non_late_increment: nli,
        }
    }

    #[test]
    fn the_worked_delays_of_the_rule() {
        let d = ti(1, 48, 1, 0, 0).delays(272).unwrap();
        assert_eq!(d, std::array::from_fn(|j| j as u32));
        let d = ti(4, 48, 1, 0, 0).delays(272).unwrap();
        assert_eq!((d[1], d[47]), (4, 188));
        assert_eq!(ti(4, 48, 1, 0, 0).max_delay(272).unwrap(), 188);
        let d = ti(2, 24, 2, 1, 1).delays(272).unwrap();
        assert_eq!((d[0], d[23], d[24], d[47]), (0, 46, 272, 318));
        let d = ti(1, 0, 2, 3, 1).delays(272).unwrap();
        assert_eq!((d[0], d[23], d[24], d[47]), (0, 23, 816, 839));
        let d = ti(1, 16, 3, 10, 0).delays(272).unwrap();
        assert_eq!(
            (d[15], d[16], d[31], d[32], d[47]),
            (15, 2720, 2720, 5440, 5440)
        );
        assert_eq!(ti(1, 16, 3, 10, 0).max_delay(272).unwrap(), 5440);
        assert!(ti(1, 10, 4, 1, 1).delays(272).is_err());
        assert!(ti(0, 48, 1, 0, 0).delays(272).is_err());
        assert!(ti(1, 20, 1, 0, 0).delays(272).is_err());
    }

    #[test]
    fn bits_and_class1() {
        let p = ti(4, 48, 1, 0, 0);
        assert_eq!(ShTimeInterleaver::from_bits(p.to_bits()), p);
        let q = ti(63, 17, 33, 255, 9);
        assert_eq!(ShTimeInterleaver::from_bits(q.to_bits()), q);
        // B1 is the most significant bit: common_multiplier 4 is 000100 in B1-B6.
        assert_eq!(p.to_bits() >> 26, 4);
        assert!(p.class1(1, 3));
        assert!(!ti(2, 24, 2, 1, 1).class1(2, 3) && ti(2, 24, 2, 1, 1).class1(1, 3));
        assert!(!ti(1, 0, 2, 3, 1).class1(1, 5));
    }

    #[test]
    fn transmitter_and_receiver_give_the_input_back_after_48_lmax_units() {
        let p = ti(2, 24, 2, 1, 1);
        let rx = p.delays(4).unwrap();
        let lmax = *rx.iter().max().unwrap() as usize;
        let mut tx =
            ConvBlocks::new(&ConvBlocks::<u32>::transmit_delays(&rx), 1, u32::MAX).unwrap();
        let mut de = ConvBlocks::new(&rx, 1, u32::MAX).unwrap();
        let lag = SH_BRANCHES * lmax;
        let (mut a, mut b) = ([0u32], [0u32]);
        for i in 0..(lag + 500) as u32 {
            tx.push(&[i], &mut a);
            let valid = de.push(&a, &mut b);
            if i as usize >= lag {
                assert_eq!(b[0] as usize, i as usize - lag);
                assert!(valid);
            }
        }
    }
}
