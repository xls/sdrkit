//! The mixed-radix digit-reversal permutation: a position written as digits of the given radices (most significant first) goes
//! to the number whose digits are the same, read in reverse significance. DTMB's frequency interleaver of 3780 carriers is the
//! case of radices 3, 3, 3, 2, 2, 5, 7.

use crate::error::{Param, Result};
use crate::interleave::Permutation;

/// The digit-reversal permutation of `radices[0] x radices[1] x ...` positions; position `i` goes to [`Permutation::dest`]
/// `(i)`. An error when a radix is below 2 or the product is 0 or above 2^31.
pub fn digit_reverse(radices: &[usize]) -> Result<Permutation> {
    let mut n = 1usize;
    for &r in radices {
        Param::Permutation.check(r, 2, 1 << 16)?;
        n = n.saturating_mul(r);
    }
    if radices.is_empty() || n > 1 << 31 {
        return Param::N.not_allowed(n.min(i64::MAX as usize) as i64);
    }
    Ok(Permutation::from_fn(n, |i| {
        // Digits from the least significant (the last radix); each becomes the most significant of the result.
        let (mut rest, mut out, mut weight) = (i, 0usize, n);
        for &r in radices.iter().rev() {
            weight /= r;
            out += (rest % r) * weight;
            rest /= r;
        }
        out
    })?)
}
