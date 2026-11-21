//! The encoder of a quasi-cyclic LDPC code whose parity part is a block staircase (an IRA code): block row `r` has the identity
//! in parity column `kb + r` and, for `r > 0`, in `kb + r - 1`. Encoding is back substitution, so it works for any length.
//!
//! For block row `r`, bit `t` of the `z`-bit vector `s_r` is the XOR over the information entries `(c, shift)` of the row of code
//! bit `c z + ((t + shift) mod z)`. Then `p_0 = s_0` and `p_r = s_r XOR p_(r-1)`; parity block `p_r` is code bits
//! `k + r z .. k + r z + z`. The code word is the `k = kb z` information bits, then the parity bits.
//!
//! ```
//! use sdrkit_algo_fec::ldpc::staircase::StaircaseEncoder;
//! // z = 4, one information column, two block rows.
//! let enc = StaircaseEncoder::new(4, 1, &[&[(0, 1), (1, 0)], &[(0, 3), (1, 0), (2, 0)]]).unwrap();
//! let mut cw = [0u8; 12];
//! enc.encode(&[1, 0, 0, 0], &mut cw).unwrap();
//! assert_eq!(cw, [1, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1]);
//! ```

use crate::error::{FecError, Param, Result};

/// A staircase encoder: the information entries of each block row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaircaseEncoder {
    z: usize,
    kb: usize,
    rows: Vec<Vec<(usize, usize)>>,
}

impl StaircaseEncoder {
    /// The encoder of the code with lifting size `z`, `kb` information columns and the base matrix `rows`: block row `r` lists its
    /// entries `(column, shift)`. An error when a parity entry is not the staircase (shift 0 in columns `kb + r` and `kb + r - 1`),
    /// a column is beyond `kb + rows.len()`, or a shift is not below `z`.
    pub fn new(z: usize, kb: usize, rows: &[&[(u16, u16)]]) -> Result<StaircaseEncoder> {
        Param::Z.check(z, 1, i64::MAX)?;
        let mb = rows.len();
        let mut info = Vec::with_capacity(mb);
        for (r, row) in rows.iter().enumerate() {
            let mut seen = [false; 2];
            let mut keep = Vec::new();
            for &(c, s) in row.iter() {
                let (c, s) = (c as usize, s as usize);
                if s >= z {
                    return Param::Shift.range(0, z as i64 - 1, s);
                }
                if c < kb {
                    keep.push((c, s));
                } else if c == kb + r && s == 0 {
                    seen[0] = true;
                } else if r > 0 && c == kb + r - 1 && s == 0 {
                    seen[1] = true;
                } else {
                    return Param::Column.not_allowed(c);
                }
            }
            if !seen[0] || (r > 0 && !seen[1]) {
                return Param::Rows.not_allowed(r);
            }
            info.push(keep);
        }
        Ok(StaircaseEncoder { z, kb, rows: info })
    }

    /// The code length.
    pub fn n(&self) -> usize {
        (self.kb + self.rows.len()) * self.z
    }

    /// The number of information bits.
    pub fn k(&self) -> usize {
        self.kb * self.z
    }

    /// Encodes the `k` bits `info` (0 or 1) into the `n` bits of `out`.
    pub fn encode(&self, info: &[u8], out: &mut [u8]) -> Result<()> {
        let (z, k) = (self.z, self.k());
        if info.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: info.len(),
            });
        }
        if out.len() != self.n() {
            return Err(FecError::OutputTooShort { need: self.n() });
        }
        for (o, &b) in out[..k].iter_mut().zip(info) {
            *o = b & 1;
        }
        let (head, parity) = out.split_at_mut(k);
        for (r, row) in self.rows.iter().enumerate() {
            let (done, rest) = parity.split_at_mut(r * z);
            let p = &mut rest[..z];
            match done.len().checked_sub(z) {
                Some(prev) => p.copy_from_slice(&done[prev..]),
                None => p.fill(0),
            }
            for &(c, s) in row {
                let block = &head[c * z..(c + 1) * z];
                for (t, pt) in p.iter_mut().enumerate() {
                    let i = t + s;
                    *pt ^= block[if i >= z { i - z } else { i }];
                }
            }
        }
        Ok(())
    }
}
