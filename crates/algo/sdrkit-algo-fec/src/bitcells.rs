//! The bit interleaver and the cell-word demultiplexer of the second-generation DVB systems as one permutation, from the tables
//! of the caller: parity interleaving, column-twist interleaving, then the demultiplexer into cell words (ETSI EN 302 755
//! clauses 6.1.3 and 6.2.1; ETSI EN 302 769 clauses 6.1.3 and 6.2.1). The same function builds the plain row-column interleaver
//! of the L1 signalling (no parity interleaving, every twist 0).

use crate::error::{FecError, Param, Result};
use sdrkit_util_bits::interleave::Permutation;

/// The parts of one bit interleaver and demultiplexer.
#[derive(Clone, Copy, Debug)]
pub struct BitCells<'a> {
    /// Bits per code word, `N_ldpc` (or the L1 length).
    pub n: usize,
    /// Information bits `K_ldpc`; parity interleaving acts on the bits after them. Ignored without parity interleaving.
    pub k: usize,
    /// Parity interleaving `u_(K + 360 t + s) = lambda_(K + Q s + t)` with `Q = (n - k) / 360`.
    pub parity: bool,
    /// The twist `t_c` of each column; its length is the number of columns. Empty: no column interleaving.
    pub twist: &'a [u16],
    /// The demultiplexer: input bit `d_i` goes to sub-stream `demux[i mod len]`. Its length is the number of sub-streams.
    pub demux: &'a [u8],
    /// Bits per cell, `eta_MOD`. With twice as many sub-streams as bits, a demultiplexed word is two cells (first half, second
    /// half).
    pub bits_per_cell: usize,
}

impl BitCells<'_> {
    /// The permutation: `apply` takes the code word (`lambda`) and returns the cell words, `bits_per_cell` bits per cell with
    /// `y_0` first; a receiver uses `invert` on the soft bits of the cells. Errors when the lengths do not divide.
    pub fn permutation(&self) -> Result<Permutation> {
        let n = self.n;
        let eta = self.bits_per_cell;
        let nsub = self.demux.len();
        if eta == 0 || (nsub != eta && nsub != 2 * eta) {
            return Err(FecError::Conflict {
                param: Param::Table,
                with: Param::Bits,
            });
        }
        let nc = self.twist.len().max(1);
        if !n.is_multiple_of(nc) || !n.is_multiple_of(nsub) || self.demux.iter().any(|&e| usize::from(e) >= nsub) {
            return Err(FecError::Conflict {
                param: Param::Table,
                with: Param::N,
            });
        }
        let q = if self.parity {
            if self.k >= n || !(n - self.k).is_multiple_of(360) {
                return Err(FecError::Conflict {
                    param: Param::K,
                    with: Param::N,
                });
            }
            (n - self.k) / 360
        } else {
            0
        };
        let k = self.k;
        // u_i = lambda_parity(i).
        let parity_src = |i: usize| -> usize {
            if !self.parity || i < k {
                i
            } else {
                let j = i - k;
                k + q * (j % 360) + j / 360
            }
        };
        let nr = n / nc;
        // v_j = u_twist(j): bit i of u is written to column i div N_r, row (i mod N_r + t_c) mod N_r; read row by row.
        let twist_src = |j: usize| -> usize {
            if self.twist.is_empty() {
                return j;
            }
            let (r, c) = (j / nc, j % nc);
            c * nr + (r + nr - usize::from(self.twist[c]) % nr) % nr
        };
        let cell_bit = |di: usize| -> usize {
            let d_o = di / nsub;
            let e = usize::from(self.demux[di % nsub]);
            if nsub == 2 * eta {
                (2 * d_o + e / eta) * eta + e % eta
            } else {
                d_o * eta + e
            }
        };
        let mut dest = vec![0u32; n];
        for di in 0..n {
            dest[parity_src(twist_src(di))] = cell_bit(di) as u32;
        }
        Ok(Permutation::from_dest(dest)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_row_column_with_identity_demux() {
        // 8 columns, written column-wise and read row-wise.
        let p = BitCells {
            n: 16,
            k: 0,
            parity: false,
            twist: &[0; 8],
            demux: &[0, 1, 2, 3, 4, 5, 6, 7],
            bits_per_cell: 4,
        }
        .permutation()
        .unwrap();
        let x: Vec<u32> = (0..16).collect();
        let mut y = vec![0u32; 16];
        p.apply(&x, &mut y).unwrap();
        assert_eq!(y, [0, 2, 4, 6, 8, 10, 12, 14, 1, 3, 5, 7, 9, 11, 13, 15]);
    }
}
