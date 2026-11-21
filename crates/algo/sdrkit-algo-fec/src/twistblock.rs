//! The twisted row-column block interleaver: `N_r` rows, `N_c` columns; input item `i` goes to column `C = i mod N_c` and row
//! `R = (C mod N_r + i div N_c) mod N_r`, and the output is read row by row. A mask marks the positions that hold items: the
//! input fills only those, in order of `i`, and the output skips the others. ETSI EN 302 769 (DVB-C2) uses it for the data
//! slices (clause 9.4.4, rows are OFDM symbols and columns carriers) and for the L1 signalling (clause 8.5, no mask).

use crate::error::{FecError, Param, Result};

/// A twisted row-column interleaver of fixed size. It keeps one scratch table and allocates nothing after
/// [`TwistedBlock::new`].
#[derive(Clone, Debug)]
pub struct TwistedBlock {
    rows: usize,
    cols: usize,
    /// For each row-major position, its rank among the marked positions.
    rank: Vec<u32>,
}

impl TwistedBlock {
    /// An interleaver of `rows` by `cols`. Errors when either is 0.
    pub fn new(rows: usize, cols: usize) -> Result<Self> {
        Param::Rows.check(rows, 1, 1 << 16)?;
        Param::Columns.check(cols, 1, 1 << 20)?;
        Ok(TwistedBlock {
            rows,
            cols,
            rank: vec![0; rows * cols],
        })
    }

    /// The number of rows.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The number of columns.
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// The (row, column) that input position `i` (0 to `rows cols - 1`) is written to.
    pub fn position(&self, i: usize) -> (usize, usize) {
        let c = i % self.cols;
        let r = (c % self.rows + i / self.cols) % self.rows;
        (r, c)
    }

    /// Ranks the marked positions of `mask` (row-major, `rows cols` entries) and returns how many there are. Errors on a
    /// mask of the wrong length.
    fn rank_mask(&mut self, mask: &[bool]) -> Result<usize> {
        if mask.len() != self.rank.len() {
            return Err(FecError::InputLength {
                expected: self.rank.len(),
                got: mask.len(),
            });
        }
        let mut n = 0u32;
        for (r, &m) in self.rank.iter_mut().zip(mask) {
            *r = n;
            n += m as u32;
        }
        Ok(n as usize)
    }

    /// Interleaves: `input[k]` goes to the `k`-th marked position in the order of `i`; `out` receives the marked positions row
    /// by row. `mask` is row-major (`None`: every position is marked). Returns the number of items moved. Errors when `input`
    /// or `out` is shorter than the marked positions or the mask has the wrong length.
    pub fn interleave<T: Copy>(
        &mut self,
        mask: Option<&[bool]>,
        input: &[T],
        out: &mut [T],
    ) -> Result<usize> {
        self.walk(mask, input.len(), out.len(), |dst, src| out[dst] = input[src])
    }

    /// The inverse of [`TwistedBlock::interleave`]: `input` holds the marked positions row by row; `out[k]` receives the item of
    /// the `k`-th marked position in the order of `i`.
    pub fn deinterleave<T: Copy>(
        &mut self,
        mask: Option<&[bool]>,
        input: &[T],
        out: &mut [T],
    ) -> Result<usize> {
        self.walk(mask, out.len(), input.len(), |rowpos, k| out[k] = input[rowpos])
    }

    /// Calls `f(rank of the position in row order, k)` for the `k`-th marked position in the order of `i`.
    fn walk(
        &mut self,
        mask: Option<&[bool]>,
        in_order_len: usize,
        row_order_len: usize,
        mut f: impl FnMut(usize, usize),
    ) -> Result<usize> {
        let total = self.rows * self.cols;
        let n = match mask {
            Some(m) => self.rank_mask(m)?,
            None => total,
        };
        if in_order_len < n || row_order_len < n {
            return Err(FecError::OutputTooShort { need: n });
        }
        let mut k = 0;
        for i in 0..total {
            let (r, c) = self.position(i);
            let p = r * self.cols + c;
            match mask {
                Some(m) if !m[p] => continue,
                Some(_) => f(self.rank[p] as usize, k),
                None => f(p, k),
            }
            k += 1;
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_and_round_trip() {
        let t = TwistedBlock::new(4, 50).unwrap();
        assert_eq!(t.position(1), (1, 1));
        assert_eq!(t.position(50), (1, 0));
        let mut t = TwistedBlock::new(4, 50).unwrap();
        let mask: Vec<bool> = (0..200).map(|p| p % 7 != 3).collect();
        let n = mask.iter().filter(|&&m| m).count();
        let input: Vec<u32> = (0..n as u32).collect();
        let (mut mid, mut back) = (vec![0u32; n], vec![0u32; n]);
        assert_eq!(t.interleave(Some(&mask), &input, &mut mid).unwrap(), n);
        t.deinterleave(Some(&mask), &mid, &mut back).unwrap();
        assert_eq!(back, input);
        let mut seen = mid.clone();
        seen.sort_unstable();
        assert_eq!(seen, input);
    }

    #[test]
    fn one_row_is_the_identity() {
        let mut t = TwistedBlock::new(1, 30).unwrap();
        let input: Vec<u8> = (0..30).collect();
        let mut out = vec![0u8; 30];
        t.interleave(None, &input, &mut out).unwrap();
        assert_eq!(out, input);
    }
}
