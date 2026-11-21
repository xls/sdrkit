//! The address generator of the DVB OFDM symbol and frequency interleavers: a register `R'` of `N_r - 1` bits starts 0, 0,
//! then 0..01, then shifts down with the sum of its tap bits as the new top bit; a bit permutation makes `R` from `R'`; the
//! address is `H(q) = (i mod 2) 2^(N_r - 1) + R_i`, kept while below the number of cells (ETSI EN 300 744 clause 4.3.4.2,
//! ETSI EN 302 755 clause 6.5, ETSI EN 302 769 clause 9.4.5).

use crate::error::{FecError, Param, Result};

/// One register rule: the bits of `R'` whose sum becomes the new top bit, and the bit of `R` that bit `j` of `R'` goes to
/// (`perm[j]`; its length is `N_r - 1`).
#[derive(Clone, Copy, Debug)]
pub struct AddressRule<'a> {
    /// The tap bits of `R'`.
    pub taps: &'a [u32],
    /// `perm[j]`: the bit of `R` that bit `j` of `R'` goes to.
    pub perm: &'a [u32],
}

impl AddressRule<'_> {
    /// Writes `H(q)` for `q = 0 .. cells - 1` into `out` (`out[q]` is the address). Errors when `cells` exceeds `2^N_r` or
    /// the rule is malformed.
    pub fn addresses(&self, cells: usize, out: &mut Vec<u32>) -> Result<()> {
        out.clear();
        let bits = self.perm.len() as u32;
        if bits == 0 || bits > 20 || self.perm.iter().any(|&p| p >= bits) || self.taps.iter().any(|&t| t >= bits) {
            return Err(FecError::Conflict {
                param: Param::Permutation,
                with: Param::Bits,
            });
        }
        let mmax = 1usize << (bits + 1);
        if cells > mmax {
            return Param::Inputs.range(0, mmax as i64, cells);
        }
        let mut rp = 0u32;
        for i in 0..mmax {
            if out.len() == cells {
                break;
            }
            rp = match i {
                0 | 1 => 0,
                2 => 1,
                _ => {
                    let fb = self.taps.iter().fold(0, |a, &t| a ^ (rp >> t)) & 1;
                    (rp >> 1) | (fb << (bits - 1))
                }
            };
            let r = self
                .perm
                .iter()
                .enumerate()
                .fold(0u32, |a, (j, &to)| a | ((rp >> j) & 1) << to);
            let v = (((i % 2) as u32) << bits) | r;
            if (v as usize) < cells {
                out.push(v);
            }
        }
        if out.len() != cells {
            return Err(FecError::Degenerate {
                param: Param::Permutation,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_a_permutation() {
        // The 4K rule of DVB-T (EN 302 583 table 5.18).
        let rule = AddressRule {
            taps: &[0, 2],
            perm: &[6, 3, 0, 9, 4, 2, 1, 8, 5, 10, 7],
        };
        let mut h = Vec::new();
        rule.addresses(3024, &mut h).unwrap();
        let mut s = h.clone();
        s.sort_unstable();
        assert!(s.iter().enumerate().all(|(i, &v)| v as usize == i));
    }
}
