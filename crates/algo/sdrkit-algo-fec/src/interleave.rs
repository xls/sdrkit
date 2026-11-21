//! Interleavers of FEC chains, built on `sdrkit_util_bits::interleave`: delay banks for convolutional and time interleavers,
//! and the permutation formulas FEC chains use.

use crate::error::{Param, Result};
pub use sdrkit_util_bits::interleave::{DelayLanes, Permutation, ReadOrder};

/// The affine permutation `i -> (a i + b) mod n` (`a` prime to `n`): TETRA, DMR BPTC (181, 196), MAC.
pub fn affine(a: usize, b: usize, n: usize) -> Result<Permutation> {
    Ok(Permutation::multiplicative(a, b, n)?)
}

/// The DVB-S2 bit interleaver (EN 302 307-1 clause 5.3.3): `n` bits written column by column into `cols` columns, read row by
/// row; with `reverse` the columns are read from the last (8PSK rate 3/5). Input `i` goes to the returned destination.
pub fn dvbs2_bits(n: usize, cols: usize, reverse: bool) -> Result<Permutation> {
    if cols == 0 {
        return Param::Columns.range(1, i64::MAX, 0);
    }
    if !n.is_multiple_of(cols) {
        return Param::N.conflict(Param::Columns);
    }
    let rows = n / cols;
    Ok(Permutation::from_fn(n, |i| {
        let (col, row) = (i / rows, i % rows);
        let col = if reverse { cols - 1 - col } else { col };
        row * cols + col
    })?)
}

/// The bit-reversal permutation of `n = 2^bits` items.
pub fn bit_reverse(bits: u32) -> Result<Permutation> {
    if bits == 0 || bits > 24 {
        return Param::Bits.range(1, 24, bits);
    }
    Ok(Permutation::from_fn(1 << bits, |i| {
        (i as u32).reverse_bits() as usize >> (32 - bits)
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permutations() {
        let p = dvbs2_bits(12, 3, false).unwrap();
        let mut out = [0u8; 12];
        p.apply(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], &mut out)
            .unwrap();
        // Column-wise write (4 rows), row-wise read.
        assert_eq!(out, [0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11]);
        let r = bit_reverse(3).unwrap();
        assert_eq!(r.dests(), &[0, 4, 2, 6, 1, 5, 3, 7]);
        assert!(affine(2, 0, 4).is_err());
    }
}
