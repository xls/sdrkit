//! Shortening and puncturing of a systematic code by groups of bits taken in a given order, as the L1 signalling of DVB-T2
//! (ETSI EN 302 755 clauses 7.3.1.2 and 7.3.1.3) and DVB-C2 (ETSI EN 302 769 clauses 8.4.1 and 8.4.2) uses it on its LDPC code.
//!
//! - Shortening: the information bits are cut into groups of `group` bits (the last group may be shorter). Whole groups are
//!   padded with zeros in the given order while the bits to pad allow; the rest is padded at the end of the next group.
//! - Puncturing: the parity bits `p_k` form `q` groups `P_j = {p_k : k mod q = j}`. Whole groups are punctured in the given
//!   order; the rest is punctured at the start of the next group.

use crate::error::{FecError, Param, Result};

/// The information positions (0 to `k - 1`, increasing) that carry zero padding when `k - k_sig` of the `k` bits are padded,
/// in groups of `group` bits taken in `order`. Errors when `k_sig > k`, `group` is 0, or `order` does not cover the groups.
pub fn shortened_positions(
    k: usize,
    k_sig: usize,
    group: usize,
    order: &[u8],
    out: &mut Vec<u32>,
) -> Result<()> {
    out.clear();
    if k_sig > k || group == 0 || order.len() * group < k {
        return Err(FecError::Conflict {
            param: Param::Shortening,
            with: Param::K,
        });
    }
    let mut left = k - k_sig;
    for &g in order {
        if left == 0 {
            break;
        }
        let start = usize::from(g) * group;
        if start >= k {
            return Err(FecError::Conflict {
                param: Param::Shortening,
                with: Param::K,
            });
        }
        let end = (start + group).min(k);
        let take = left.min(end - start);
        out.extend((end - take..end).map(|p| p as u32));
        left -= take;
    }
    out.sort_unstable();
    Ok(())
}

/// The parity positions (0 to `parity - 1`, increasing) punctured when `n_punc` of the `parity` bits are punctured, in the `q`
/// groups `P_j = {p_k : k mod q = j}` taken in `order`. Errors when `n_punc > parity` or `order` does not name `q` groups.
pub fn punctured_positions(
    parity: usize,
    q: usize,
    order: &[u8],
    n_punc: usize,
    out: &mut Vec<u32>,
) -> Result<()> {
    out.clear();
    if n_punc > parity || q == 0 || order.len() != q || order.iter().any(|&g| usize::from(g) >= q) {
        return Err(FecError::Conflict {
            param: Param::Puncturing,
            with: Param::Parity,
        });
    }
    let mut left = n_punc;
    for &g in order {
        if left == 0 {
            break;
        }
        let members = (usize::from(g)..parity).step_by(q);
        let size = members.len();
        let take = left.min(size);
        out.extend(members.take(take).map(|p| p as u32));
        left -= take;
    }
    out.sort_unstable();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_groups_then_the_end_of_the_next() {
        let mut v = Vec::new();
        // 3 groups of 4 bits (the last one of 2), order 2 0 1: pad 2 bits of group 2, then 3 bits at the end of group 0.
        shortened_positions(10, 5, 4, &[2, 0, 1], &mut v).unwrap();
        assert_eq!(v, [1, 2, 3, 8, 9]);
        punctured_positions(12, 3, &[1, 2, 0], 6, &mut v).unwrap();
        // Group 1 = {1, 4, 7, 10}, then the first two of group 2 = {2, 5}.
        assert_eq!(v, [1, 2, 4, 5, 7, 10]);
    }
}
