//! The bit interleaver of an ATSC 3.0 PLP (A/322 clause 6.2).

use super::tables::{BLOCK_TYPE_B_16K, BLOCK_TYPE_B_64K, GROUPWISE_16K, GROUPWISE_64K};
use crate::error::{FecError, Param, Result};
use crate::ldpc::tables::atsc3::Atsc3Table;
use sdrkit_util_bits::interleave::Permutation;

/// The two block interleavers of A/322 clause 6.2.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockType {
    /// Columns of whole bit groups, read row by row (clause 6.2.3.1).
    A,
    /// Sets of bit groups written as rows of 360 bits and read column by column (clause 6.2.3.2).
    B,
}

fn conflict<T>() -> Result<T> {
    Err(FecError::Conflict {
        param: Param::N,
        with: Param::Rate,
    })
}

/// The index of a modulation of `bits` bits per cell in the tables (QPSK first), or `None`.
fn mod_index(n_inner: usize, bits: usize) -> Option<usize> {
    let max = if n_inner == 64800 { 12 } else { 8 };
    (bits >= 2 && bits <= max && bits.is_multiple_of(2)).then_some(bits / 2 - 1)
}

/// The block interleaver of `n_inner` bits (16200 or 64800) for `bits` bits per cell as a permutation: input `i` is bit `v_i`
/// of the group-wise interleaver, output `j` is bit `q_j` that goes to the mapper. An error for a length or a modulation that
/// A/322 tables 6.10 and 6.11 do not hold.
pub fn block_interleaver(n_inner: usize, bits: usize, kind: BlockType) -> Result<Permutation> {
    if !(n_inner == 64800 || n_inner == 16200) || mod_index(n_inner, bits).is_none() {
        return conflict();
    }
    let groups = n_inner / 360;
    let nc = bits;
    let dest: Vec<u32> = match kind {
        BlockType::A => {
            let nr1 = (groups / nc) * 360;
            let nr2 = n_inner / nc - nr1;
            (0..n_inner)
                .map(|i| {
                    let (c, r) = if i < nc * nr1 {
                        (i / nr1, i % nr1)
                    } else {
                        let i = i - nc * nr1;
                        (i / nr2, nr1 + i % nr2)
                    };
                    (r * nc + c) as u32
                })
                .collect()
        }
        BlockType::B => {
            let part1 = (groups / nc) * nc * 360;
            (0..n_inner)
                .map(|i| {
                    if i < part1 {
                        let set = i / (nc * 360);
                        let (row, col) = ((i / 360) % nc, i % 360);
                        (set * nc * 360 + col * nc + row) as u32
                    } else {
                        i as u32
                    }
                })
                .collect()
        }
    };
    Permutation::from_dest(dest).or_else(|_| conflict())
}

/// The bit interleaver of one PLP: the parity interleaver (type B codes), the group-wise interleaver of Annex B and the block
/// interleaver of table 6.8 or 6.9, as one permutation between the LDPC code word `Λ` and the bits `q` that go to the mapper
/// (`bits` of them per cell, the first is the most significant bit of the cell).
#[derive(Clone, Debug)]
pub struct BitInterleaver {
    perm: Permutation,
    kind: BlockType,
}

impl BitInterleaver {
    /// The interleaver of the code of `n_inner` bits (16200 or 64800) and rate `rate_fifteenths / 15` (2 to 13) for a
    /// constellation of `bits` bits per cell (2, 4, 6, 8, and 10 or 12 at 64800). An error for another combination.
    pub fn new(n_inner: usize, rate_fifteenths: u8, bits: usize) -> Result<Self> {
        let (Some(table), Some(m)) = (
            Atsc3Table::find(n_inner, rate_fifteenths),
            mod_index(n_inner, bits),
        ) else {
            return conflict();
        };
        let r = usize::from(rate_fifteenths) - 2;
        let (order, type_b): (&[u8], bool) = if n_inner == 64800 {
            (&GROUPWISE_64K[m][r], BLOCK_TYPE_B_64K[m][r])
        } else {
            (&GROUPWISE_16K[m][r], BLOCK_TYPE_B_16K[m][r])
        };
        let kind = if type_b { BlockType::B } else { BlockType::A };
        let k = table.k();
        // Clause 6.2.1: u[k + 360 t + s] = λ[k + Q s + t]. A type A code word is already in this order.
        let parity: Vec<u32> = match table {
            Atsc3Table::A(_) => (0..n_inner as u32).collect(),
            Atsc3Table::B(t) => {
                let q = t.q as usize;
                (0..n_inner)
                    .map(|i| {
                        if i < k {
                            i as u32
                        } else {
                            let (s, t) = ((i - k) / q, (i - k) % q);
                            (k + 360 * t + s) as u32
                        }
                    })
                    .collect()
            }
        };
        // Clause 6.2.2: Y_j = X_π(j), so group π(j) of the input goes to group j of the output.
        let mut to = vec![0usize; order.len()];
        for (j, &g) in order.iter().enumerate() {
            to[usize::from(g)] = j;
        }
        let block = block_interleaver(n_inner, bits, kind)?;
        let dest: Vec<u32> = parity
            .iter()
            .map(|&u| {
                let u = u as usize;
                let v = to[u / 360] * 360 + u % 360;
                block.dests()[v]
            })
            .collect();
        Ok(BitInterleaver {
            perm: Permutation::from_dest(dest).or_else(|_| conflict())?,
            kind,
        })
    }

    /// The number of bits, `N_inner`.
    pub fn len(&self) -> usize {
        self.perm.len()
    }

    /// Whether the interleaver holds no bit (never).
    pub fn is_empty(&self) -> bool {
        self.perm.is_empty()
    }

    /// The block interleaver type of this code and modulation.
    pub fn block_type(&self) -> BlockType {
        self.kind
    }

    /// Interleaves the code word `cw` (`N_inner` items in the order of `Λ`) into `out`. An error when a length is wrong.
    pub fn interleave<T: Copy>(&self, cw: &[T], out: &mut [T]) -> Result<()> {
        self.perm.apply(cw, out).map_err(|_| FecError::InputLength {
            expected: self.perm.len(),
            got: cw.len(),
        })
    }

    /// Puts the received items `q` (`N_inner` of them, in the order of the mapper) back in the order of `Λ`. An error when a
    /// length is wrong.
    pub fn deinterleave<T: Copy>(&self, q: &[T], out: &mut [T]) -> Result<()> {
        self.perm.invert(q, out).map_err(|_| FecError::InputLength {
            expected: self.perm.len(),
            got: q.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ldpc::tables::atsc3::ALL;

    /// Annex B: every sequence holds each group once.
    #[test]
    fn every_group_wise_order_is_a_permutation() {
        for m in GROUPWISE_64K.iter() {
            for row in m {
                let mut seen = [false; 180];
                row.iter().for_each(|&g| seen[usize::from(g)] = true);
                assert!(seen.iter().all(|&s| s));
            }
        }
        for m in GROUPWISE_16K.iter() {
            for row in m {
                let mut seen = [false; 45];
                row.iter().for_each(|&g| seen[usize::from(g)] = true);
                assert!(seen.iter().all(|&s| s));
            }
        }
        // First and last values as printed: table B.1.1 rate 2/15, table B.2.4 rate 13/15.
        assert_eq!(GROUPWISE_64K[0][0][..4], [70, 149, 136, 153]);
        assert_eq!(GROUPWISE_64K[0][0][179], 17);
        assert_eq!(GROUPWISE_64K[0][6][..4], [0, 2, 4, 6]);
        assert_eq!(GROUPWISE_16K[3][11][44], 34);
    }

    /// Clause 6.2.3.1: the output order printed for 256QAM and 64800 bits.
    #[test]
    fn type_a_block_interleaver_reads_as_the_standard_prints() {
        let p = block_interleaver(64800, 8, BlockType::A).unwrap();
        let src = |j: usize| p.src(j).unwrap();
        let head: Vec<usize> = (0..16).map(src).collect();
        assert_eq!(
            head,
            [
                0, 7920, 15840, 23760, 31680, 39600, 47520, 55440, 1, 7921, 15841, 23761, 31681,
                39601, 47521, 55441
            ]
        );
        // The end of part 1 and the start of part 2.
        let seam: Vec<usize> = (63352..63368).map(src).collect();
        assert_eq!(
            seam,
            [
                7919, 15839, 23759, 31679, 39599, 47519, 55439, 63359, 63360, 63540, 63720, 63900,
                64080, 64260, 64440, 64620
            ]
        );
        let tail: Vec<usize> = (64792..64800).map(src).collect();
        assert_eq!(
            tail,
            [63539, 63719, 63899, 64079, 64259, 64439, 64619, 64799]
        );
        // Table 6.10: rows of part 1 and part 2.
        for (n, bits, nr1, nr2) in [
            (64800, 2, 32400, 0),
            (64800, 12, 5400, 0),
            (16200, 2, 7920, 180),
            (16200, 4, 3960, 90),
            (16200, 6, 2520, 180),
            (16200, 8, 1800, 225),
        ] {
            let groups = n / 360;
            assert_eq!((groups / bits) * 360, nr1);
            assert_eq!(n / bits - nr1, nr2);
            assert!(block_interleaver(n, bits, BlockType::A).is_ok());
        }
    }

    /// Clause 6.2.3.2: a set of 8 groups is read one bit of each group at a time; the bits after part 1 keep their place.
    #[test]
    fn type_b_block_interleaver_reads_one_bit_of_each_group() {
        let p = block_interleaver(64800, 8, BlockType::B).unwrap();
        let head: Vec<usize> = (0..10).map(|j| p.src(j).unwrap()).collect();
        assert_eq!(head, [0, 360, 720, 1080, 1440, 1800, 2160, 2520, 1, 361]);
        assert_eq!(p.src(2880).unwrap(), 2880);
        // Table 6.11: N_part1 = 63360 for 256QAM at 64800 and 14400 at 16200.
        assert_eq!(p.src(63359).unwrap(), 63359);
        assert_eq!(p.src(63360).unwrap(), 63360);
        assert_eq!(p.src(64799).unwrap(), 64799);
        let p = block_interleaver(16200, 8, BlockType::B).unwrap();
        assert_eq!(p.src(14399).unwrap(), 14399);
        assert_eq!(p.src(14400).unwrap(), 14400);
        assert_eq!(p.src(8).unwrap(), 1);
    }

    #[test]
    fn every_combination_goes_there_and_back() {
        for &(n, r, _) in ALL {
            let n = n as usize;
            for bits in (2..=12).step_by(2) {
                let made = BitInterleaver::new(n, r, bits);
                if n == 16200 && bits > 8 {
                    assert!(made.is_err());
                    continue;
                }
                let il = made.unwrap();
                assert_eq!(il.len(), n);
                let cw: Vec<u32> = (0..n as u32).collect();
                let mut q = vec![0u32; n];
                il.interleave(&cw, &mut q).unwrap();
                let mut back = vec![0u32; n];
                il.deinterleave(&q, &mut back).unwrap();
                assert_eq!(back, cw);
                assert!(il.interleave(&cw[1..], &mut q).is_err());
            }
        }
        // Tables 6.8 and 6.9.
        let kind = |n, r, b| BitInterleaver::new(n, r, b).unwrap().block_type();
        assert_eq!(kind(64800, 8, 2), BlockType::A);
        assert_eq!(kind(64800, 8, 4), BlockType::B);
        assert_eq!(kind(64800, 13, 8), BlockType::B);
        assert_eq!(kind(16200, 6, 2), BlockType::B);
        assert_eq!(kind(16200, 8, 2), BlockType::A);
        assert!(BitInterleaver::new(64800, 14, 2).is_err());
        assert!(BitInterleaver::new(64800, 8, 3).is_err());
    }

    /// QPSK, 64800, rate 8/15 (type B code, type A block interleaver): the first information group goes where π puts it,
    /// and the parity bits of one accumulator step go to one group.
    #[test]
    fn the_parts_compose_in_the_order_of_the_standard() {
        let il = BitInterleaver::new(64800, 8, 2).unwrap();
        let order = &GROUPWISE_64K[0][6];
        let j = order.iter().position(|&g| g == 0).unwrap();
        let cw: Vec<u32> = (0..64800).collect();
        let mut q = vec![0u32; 64800];
        il.interleave(&cw, &mut q).unwrap();
        // Bit 0 of Λ is bit 0 of group 0: v index 360 j, column 360 j / 32400, row 360 j mod 32400.
        let v = 360 * j;
        assert_eq!(q[(v % 32400) * 2 + v / 32400], 0);
        // λ[k + Q s + t] is u[k + 360 t + s]: parity bit Q (s = 1, t = 0) is bit 1 of parity group 0.
        let (k, qq) = (34560usize, 84usize);
        let g = k / 360;
        let j = order.iter().position(|&x| usize::from(x) == g).unwrap();
        let v = 360 * j + 1;
        assert_eq!(q[(v % 32400) * 2 + v / 32400], (k + qq) as u32);
    }
}
