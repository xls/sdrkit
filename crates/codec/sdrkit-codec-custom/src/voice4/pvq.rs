//! The pyramid vector quantiser (PVQ) of voice codec 4: codeword counts, index sizes and the enumeration in both directions.

/// The largest vector length in the count table.
pub(crate) const MAX_N: usize = 81;
/// The largest pulse count in the count table.
pub(crate) const MAX_K: usize = 64;

const fn build() -> [[u64; MAX_K + 1]; MAX_N + 1] {
    let mut v = [[0u64; MAX_K + 1]; MAX_N + 1];
    let mut n = 0;
    while n <= MAX_N {
        v[n][0] = 1;
        n += 1;
    }
    let mut n = 1;
    while n <= MAX_N {
        let mut k = 1;
        while k <= MAX_K {
            v[n][k] = v[n - 1][k]
                .saturating_add(v[n][k - 1])
                .saturating_add(v[n - 1][k - 1]);
            k += 1;
        }
        n += 1;
    }
    v
}

static V: [[u64; MAX_K + 1]; MAX_N + 1] = build();

/// The number of codewords V(n, k), saturated at 2^64 - 1. Values outside the table are 0.
#[inline]
pub(crate) fn count(n: usize, k: usize) -> u64 {
    if n > MAX_N || k > MAX_K {
        return 0;
    }
    V[n][k]
}

/// The size of a PVQ index in bits for length `n` and `k` pulses. "Too large" is 99.
pub(crate) fn index_bits(n: usize, k: usize) -> u32 {
    let v = count(n, k);
    if v == 0 || v > 1u64 << 63 {
        return 99;
    }
    if v <= 1 {
        return 0;
    }
    64 - (v - 1).leading_zeros()
}

/// The index of the codeword `y` with `k` pulses.
pub(crate) fn encode(y: &[i32], k: usize) -> u64 {
    let n = y.len();
    let mut idx = 0u64;
    let mut k = k;
    for (i, &v) in y.iter().enumerate() {
        if k == 0 {
            break;
        }
        let rem = n - i - 1;
        let a = v.unsigned_abs() as usize;
        if a == 0 {
            continue;
        }
        idx = idx.wrapping_add(count(rem, k));
        for j in 1..a {
            idx = idx.wrapping_add(count(rem, k - j).wrapping_mul(2));
        }
        if v < 0 {
            idx = idx.wrapping_add(count(rem, k - a));
        }
        k -= a;
    }
    idx
}

/// Writes the codeword of `idx` with `k` pulses into `y`. The index is first reduced modulo V(n, k).
pub(crate) fn decode(idx: u64, k: usize, y: &mut [i32]) {
    let n = y.len();
    let total = count(n, k);
    let mut idx = if total == 0 { 0 } else { idx % total };
    let mut k = k;
    for i in 0..n {
        y[i] = 0;
        if k == 0 {
            continue;
        }
        if i == n - 1 {
            y[i] = if idx == 0 { k as i32 } else { -(k as i32) };
            k = 0;
            continue;
        }
        let rem = n - i - 1;
        let c = count(rem, k);
        if idx < c {
            continue;
        }
        idx -= c;
        for a in 1..=k {
            let c = count(rem, k - a);
            if idx < c {
                y[i] = a as i32;
                break;
            }
            idx -= c;
            if idx < c {
                y[i] = -(a as i32);
                break;
            }
            idx -= c;
        }
        k -= y[i].unsigned_abs() as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Section 3.5.4: the index sizes for L = 40, K = 1 to 18 and L = 80, K = 1 to 13.
    #[test]
    fn index_sizes_match_the_spec() {
        let l40: Vec<u32> = (1..=18).map(|k| index_bits(40, k)).collect();
        assert_eq!(
            l40,
            [7, 12, 17, 21, 25, 29, 32, 36, 39, 42, 45, 48, 50, 53, 55, 58, 60, 62]
        );
        let l80: Vec<u32> = (1..=13).map(|k| index_bits(80, k)).collect();
        assert_eq!(l80, [8, 14, 20, 25, 30, 35, 39, 44, 48, 52, 56, 60, 63]);
        assert_eq!(index_bits(40, 0), 0);
    }

    #[test]
    fn every_index_round_trips_for_a_small_size() {
        let mut y = [0i32; 5];
        for k in 0..5 {
            for idx in 0..count(5, k) {
                decode(idx, k, &mut y);
                assert_eq!(
                    y.iter().map(|v| v.unsigned_abs() as usize).sum::<usize>(),
                    k
                );
                assert_eq!(encode(&y, k), idx);
            }
        }
    }

    #[test]
    fn enumeration_order_starts_with_zero_then_plus_then_minus() {
        let mut y = [0i32; 3];
        decode(0, 1, &mut y);
        assert_eq!(y, [0, 0, 1]);
        decode(1, 1, &mut y);
        assert_eq!(y, [0, 0, -1]);
        decode(count(2, 1), 1, &mut y);
        assert_eq!(y, [1, 0, 0]);
        decode(count(2, 1) + 1, 1, &mut y);
        assert_eq!(y, [-1, 0, 0]);
    }
}
