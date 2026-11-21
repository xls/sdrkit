//! A systematic erasure code over GF(256) with a Cauchy parity matrix: `g` data cells and `p` parity cells of one length, and
//! any `g` of the `g + p` cells restore the data.
//!
//! Parity cell `j` is the sum over data cells `i` of `C[j][i]` times cell `i`, byte by byte, with `C[j][i] = 1 / (j XOR (p + i))`
//! in GF(256) with the polynomial x^8 + x^4 + x^3 + x^2 + 1 (0x11D). Addition is XOR. Recovery takes the first `g` cells present,
//! in cell order, and solves the `g x g` system: a data cell gives a unit row, parity cell `j` the row `C[j][0 .. g]`.
//!
//! ```
//! use sdrkit_algo_fec::cauchy::CauchyCode;
//! let code = CauchyCode::new(4, 2).unwrap();
//! let mut cells: Vec<u8> = (0..16).collect();
//! cells.resize(24, 0);
//! let (data, parity) = cells.split_at_mut(16);
//! code.encode(data, 4, parity).unwrap();
//! assert_eq!(&cells[16..], &[0x02, 0x98, 0x2B, 0xB1, 0x50, 0xCA, 0x79, 0xE3]);
//! // Lose data cells 1 and 3, then restore them.
//! let mut present = [true, false, true, false, true, true];
//! cells[4..8].fill(0);
//! cells[12..16].fill(0);
//! code.recover(&mut cells, 4, &mut present).unwrap();
//! assert_eq!(&cells[..16], &(0..16).collect::<Vec<u8>>()[..]);
//! ```

use crate::error::{FecError, Param, Result};
use crate::gf::Gf;

/// GF(256) with the polynomial 0x11D.
fn field() -> &'static Gf {
    static F: std::sync::OnceLock<Gf> = std::sync::OnceLock::new();
    F.get_or_init(|| Gf::new(8, 0x11D).expect("x^8 + x^4 + x^3 + x^2 + 1 is primitive"))
}

fn mul(a: u8, b: u8) -> u8 {
    field().mul(u16::from(a), u16::from(b)) as u8
}

fn inv(a: u8) -> u8 {
    field().inv(u16::from(a)).unwrap_or(0) as u8
}

/// Adds `c` times `src` to `dst`, byte by byte.
fn mul_add(dst: &mut [u8], src: &[u8], c: u8) {
    if c == 0 {
        return;
    }
    let f = field();
    let lc = f.log(u16::from(c)).unwrap_or(0);
    for (d, &s) in dst.iter_mut().zip(src) {
        if let Some(ls) = f.log(u16::from(s)) {
            *d ^= f.exp(lc + ls) as u8;
        }
    }
}

/// The Cauchy erasure code with `g` data cells and `p` parity cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CauchyCode {
    g: usize,
    p: usize,
}

impl CauchyCode {
    /// The code with `g` data cells (at least 1) and `p` parity cells. An error when `g + p` is above 256.
    pub fn new(g: usize, p: usize) -> Result<CauchyCode> {
        Param::K.check(g, 1, 256)?;
        Param::Parity.check(g + p, 1, 256)?;
        Ok(CauchyCode { g, p })
    }

    /// The number of data cells.
    pub fn data_cells(&self) -> usize {
        self.g
    }

    /// The number of parity cells.
    pub fn parity_cells(&self) -> usize {
        self.p
    }

    /// The matrix entry `C[j][i] = 1 / (j XOR (p + i))` for parity cell `j` and data cell `i`.
    pub fn coefficient(&self, j: usize, i: usize) -> u8 {
        inv((j ^ (self.p + i)) as u8)
    }

    /// Writes the `p` parity cells of the `g` data cells `data` (cells of `cell` bytes, back to back) into `parity`.
    /// An error when the lengths are not `g × cell` and `p × cell`.
    pub fn encode(&self, data: &[u8], cell: usize, parity: &mut [u8]) -> Result<()> {
        self.lengths(data.len(), self.g, cell)?;
        self.lengths(parity.len(), self.p, cell)?;
        parity.fill(0);
        for (j, pc) in parity.chunks_exact_mut(cell.max(1)).enumerate() {
            for (i, dc) in data.chunks_exact(cell.max(1)).enumerate() {
                mul_add(pc, dc, self.coefficient(j, i));
            }
        }
        Ok(())
    }

    /// Restores the missing data cells of `cells` (all `g + p` cells of `cell` bytes, back to back; a missing cell holds any
    /// bytes) from the cells marked in `present`, and marks the restored cells present. Nothing to do when every data cell is
    /// present. `Err(Uncorrectable)` when fewer than `g` cells are present; the cells are then unchanged.
    pub fn recover(&self, cells: &mut [u8], cell: usize, present: &mut [bool]) -> Result<()> {
        let (g, n) = (self.g, self.g + self.p);
        self.lengths(cells.len(), n, cell)?;
        if present.len() != n {
            return Err(FecError::InputLength {
                expected: n,
                got: present.len(),
            });
        }
        if present[..g].iter().all(|&p| p) {
            return Ok(());
        }
        let used: Vec<usize> = (0..n).filter(|&c| present[c]).take(g).collect();
        if used.len() < g {
            return Err(FecError::Uncorrectable);
        }
        // Rows of the system: a data cell is a unit row, parity cell j the row C[j][..]. Invert the g x g matrix by Gauss-Jordan.
        let mut a = vec![0u8; g * g];
        for (r, &c) in used.iter().enumerate() {
            if c < g {
                a[r * g + c] = 1;
            } else {
                for i in 0..g {
                    a[r * g + i] = self.coefficient(c - g, i);
                }
            }
        }
        let mut b = vec![0u8; g * g];
        for r in 0..g {
            b[r * g + r] = 1;
        }
        for col in 0..g {
            let piv = (col..g)
                .find(|&r| a[r * g + col] != 0)
                .ok_or(FecError::Uncorrectable)?;
            if piv != col {
                for k in 0..g {
                    a.swap(piv * g + k, col * g + k);
                    b.swap(piv * g + k, col * g + k);
                }
            }
            let s = inv(a[col * g + col]);
            for k in 0..g {
                a[col * g + k] = mul(a[col * g + k], s);
                b[col * g + k] = mul(b[col * g + k], s);
            }
            for r in 0..g {
                let f = a[r * g + col];
                if r != col && f != 0 {
                    for k in 0..g {
                        a[r * g + k] ^= mul(f, a[col * g + k]);
                        b[r * g + k] ^= mul(f, b[col * g + k]);
                    }
                }
            }
        }
        // Data cell i = sum over the used cells r of B[i][r] times cell r.
        let src: Vec<u8> = used
            .iter()
            .flat_map(|&c| cells[c * cell..(c + 1) * cell].iter().copied())
            .collect();
        for i in 0..g {
            if present[i] {
                continue;
            }
            let out = &mut cells[i * cell..(i + 1) * cell];
            out.fill(0);
            for r in 0..g {
                mul_add(out, &src[r * cell..(r + 1) * cell], b[i * g + r]);
            }
            present[i] = true;
        }
        Ok(())
    }

    fn lengths(&self, got: usize, cells: usize, cell: usize) -> Result<()> {
        if got != cells * cell {
            return Err(FecError::InputLength {
                expected: cells * cell,
                got,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parity_of_the_group_vector() {
        // G = 14, P = 2, C = 178, data cell i byte t = (31 i + t) mod 256.
        let (g, p, c) = (14, 2, 178);
        let code = CauchyCode::new(g, p).unwrap();
        let data: Vec<u8> = (0..g * c).map(|k| ((31 * (k / c) + k % c) % 256) as u8).collect();
        let mut par = vec![0u8; p * c];
        code.encode(&data, c, &mut par).unwrap();
        assert_eq!(
            &par[..16],
            &[0xEF, 0x6B, 0xD3, 0xF4, 0x1C, 0x03, 0x5F, 0x18, 0x47, 0x4C, 0x17, 0x5D, 0x34, 0x38, 0x6A, 0x42]
        );
        assert_eq!(
            &par[c..c + 16],
            &[0x2F, 0x70, 0xE5, 0x4D, 0xAC, 0x64, 0xB7, 0xE3, 0xAE, 0x1A, 0x85, 0x25, 0x9B, 0x8D, 0xA5, 0xF7]
        );
    }

    #[test]
    fn any_g_cells_restore_and_fewer_fail() {
        let (g, p, c) = (6, 3, 5);
        let code = CauchyCode::new(g, p).unwrap();
        let mut all: Vec<u8> = (0..(g + p) * c).map(|k| (k * 7 + 3) as u8).collect();
        let (d, par) = all.split_at_mut(g * c);
        code.encode(d, c, par).unwrap();
        let want = all.clone();
        for lost in [[0usize, 1, 2], [0, 4, 7], [3, 5, 8]] {
            let mut cells = all.clone();
            let mut present = vec![true; g + p];
            for &l in &lost {
                present[l] = false;
                cells[l * c..(l + 1) * c].fill(0xAA);
            }
            code.recover(&mut cells, c, &mut present).unwrap();
            assert_eq!(&cells[..g * c], &want[..g * c], "{lost:?}");
        }
        let mut present = vec![true; g + p];
        present[..4].fill(false);
        assert_eq!(code.recover(&mut all, c, &mut present), Err(FecError::Uncorrectable));
        assert!(CauchyCode::new(200, 57).is_err());
    }
}
