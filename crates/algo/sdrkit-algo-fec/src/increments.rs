//! The block interleaver of MIL-STD-188-141B Appendix C section C.5.1.2: a matrix of `rows x columns` written along a walk
//! of row and column increments and read along another.
//!
//! A walk starts at row 0, column 0. After bit `s` (from 0) the column moves by its increment, plus the delta increment when
//! `s + 1` is a multiple of the rows, modulo the columns; the row moves by its increment, plus the delta increment when `s + 1` is
//! a multiple of the columns, modulo the rows. Bit `s` of the input is written at the `s`-th place of the load walk; output bit
//! `t` is read from the `t`-th place of the fetch walk.
//!
//! ```
//! use sdrkit_algo_fec::increments::{interleaver, Walk};
//! // Written by rows of 13, read by columns.
//! let p = interleaver(4, 13, Walk::new(0, 1, 1, 0), Walk::new(1, 0, 0, 1)).unwrap();
//! let fetched: Vec<usize> = (0..6).map(|t| p.src(t).unwrap()).collect();
//! assert_eq!(fetched, [0, 13, 26, 39, 1, 14]);
//! ```

use crate::error::Result;
use sdrkit_util_bits::interleave::Permutation;

/// The increments of a walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    /// Row increment after each bit.
    pub row: i32,
    /// Column increment after each bit.
    pub col: i32,
    /// Extra row increment after each `columns` bits.
    pub delta_row: i32,
    /// Extra column increment after each `rows` bits.
    pub delta_col: i32,
}

impl Walk {
    /// A walk of row increment `row`, column increment `col` and the delta increments `delta_row` and `delta_col`.
    pub const fn new(row: i32, col: i32, delta_row: i32, delta_col: i32) -> Walk {
        Walk {
            row,
            col,
            delta_row,
            delta_col,
        }
    }
}

/// The cells (row major, `row x columns + column`) a walk visits, in order, `rows x columns` of them.
pub fn cells(rows: usize, cols: usize, w: Walk) -> Vec<u32> {
    let n = rows * cols;
    let (r, c) = (rows as i64, cols as i64);
    let (mut row, mut col) = (0i64, 0i64);
    let mut out = Vec::with_capacity(n);
    for s in 0..n {
        out.push((row * c + col) as u32);
        let k = s + 1;
        let mut dc = i64::from(w.col);
        if k % rows == 0 {
            dc += i64::from(w.delta_col);
        }
        let mut dr = i64::from(w.row);
        if k % cols == 0 {
            dr += i64::from(w.delta_row);
        }
        col = (col + dc).rem_euclid(c);
        row = (row + dr).rem_euclid(r);
    }
    out
}

/// The interleaver of a load walk and a fetch walk as a permutation (input bit to output bit).
///
/// # Errors
///
/// When a walk visits a cell twice: the parameters give no permutation (as the printed parameters of the two largest BW3 sizes
/// of 141B Table C-XVI do).
pub fn interleaver(rows: usize, cols: usize, load: Walk, fetch: Walk) -> Result<Permutation> {
    let n = rows * cols;
    let l = cells(rows, cols, load);
    let f = cells(rows, cols, fetch);
    let mut at = vec![u32::MAX; n];
    for (t, &cell) in f.iter().enumerate() {
        at[cell as usize] = t as u32;
    }
    let dest: Vec<u32> = l.iter().map(|&cell| at[cell as usize]).collect();
    Ok(Permutation::from_dest(dest)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_burst_interleavers() {
        let w0 = (Walk::new(0, 1, 1, 0), Walk::new(1, 0, 0, 1));
        let p = interleaver(16, 9, w0.0, w0.1).unwrap();
        let f: Vec<usize> = (0..18).map(|t| p.src(t).unwrap()).collect();
        assert_eq!(
            &f[..17],
            &[0, 9, 18, 27, 36, 45, 54, 63, 72, 81, 90, 99, 108, 117, 126, 135, 1]
        );
        let w3 = (Walk::new(7, 0, 0, 1), Walk::new(1, -7, 0, -7));
        assert!(interleaver(24, 24, w3.0, w3.1).is_ok());
        assert!(interleaver(32, 34, w3.0, w3.1).is_ok());
        assert!(interleaver(44, 48, w3.0, w3.1).is_err());
        assert!(interleaver(64, 65, w3.0, w3.1).is_err());
    }
}
