//! One-dimensional k-means (Lloyd's rounds) for level finding: each value joins the nearest centre (a tie joins the lower
//! index), each centre with members moves to their mean. The rounds stop early when no centre moves, which gives the same
//! result as running them all. The start is spread over the sorted values or given. [`kmeans_1d`] takes any `k`;
//! [`kmeans_1d_fixed`] takes `K` known at compile time, `f32` or `f64` values, and allocates nothing.
//!
//! ```
//! use sdrkit_algo_dsp::kmeans::{kmeans_1d, kmeans_1d_fixed, Start};
//! let c = kmeans_1d(&[1.0, 1.1, 3.0, 3.1], 2, Start::Spread, 30).unwrap();
//! assert!((c.centres[0] - 1.05).abs() < 1e-12 && (c.centres[1] - 3.05).abs() < 1e-12);
//! let f = kmeans_1d_fixed(&[1.0f32, 1.5, 3.0, 3.5], [0.0, 4.0], 30).unwrap();
//! assert_eq!((f.centres, f.counts), ([1.25, 3.25], [2, 2]));
//! ```

/// Where the centres start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Start<'a> {
    /// The sorted values at index `floor((i + 0.5) / k x (n - 1))` for `i = 0 .. k - 1`.
    Spread,
    /// These centres (`k` of them).
    Given(&'a [f64]),
}

/// The result of [`kmeans_1d`].
#[derive(Clone, Debug, PartialEq)]
pub struct Clusters {
    /// The centres, in increasing order.
    pub centres: Vec<f64>,
    /// The members of each centre in the assignment of the last round, in the order of `centres`.
    pub counts: Vec<usize>,
    /// The sum of the squared distances of each value to its centre in the last round (to its nearest centre in `centres`
    /// when the rounds stopped early).
    pub residual: f64,
}

impl Clusters {
    /// The index of the centre nearest to `v` (a tie gives the lower index).
    pub fn nearest(&self, v: f64) -> usize {
        nearest(&self.centres, v)
    }
}

/// The result of [`kmeans_1d_fixed`]: as [`Clusters`], in arrays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedClusters<const K: usize> {
    /// The centres, in increasing order.
    pub centres: [f64; K],
    /// The members of each centre in the assignment of the last round, in the order of `centres`.
    pub counts: [usize; K],
    /// The sum of the squared distances of each value to its centre in the last round (to its nearest centre in `centres`
    /// when the rounds stopped early).
    pub residual: f64,
}

impl<const K: usize> FixedClusters<K> {
    /// The index of the centre nearest to `v` (a tie gives the lower index).
    pub fn nearest(&self, v: f64) -> usize {
        nearest(&self.centres, v)
    }
}

fn nearest(c: &[f64], v: f64) -> usize {
    let mut best = 0;
    for (i, &x) in c.iter().enumerate().skip(1) {
        if (v - x).abs() < (v - c[best]).abs() {
            best = i;
        }
    }
    best
}

/// At most `rounds` (at least 1) Lloyd's rounds of `values` on the centres `c`, with the sums and counts in `sum` and `count`
/// (all of one length). Returns the residual of the last round.
fn lloyd<T: Copy + Into<f64>>(
    values: &[T],
    c: &mut [f64],
    sum: &mut [f64],
    count: &mut [usize],
    rounds: usize,
) -> f64 {
    let mut residual = 0.0;
    for _ in 0..rounds.max(1) {
        sum.iter_mut().for_each(|s| *s = 0.0);
        count.iter_mut().for_each(|s| *s = 0);
        residual = 0.0;
        for &v in values {
            let v: f64 = v.into();
            let j = nearest(c, v);
            sum[j] += v;
            count[j] += 1;
            residual += (v - c[j]) * (v - c[j]);
        }
        let mut moved = false;
        for j in 0..c.len() {
            if count[j] > 0 {
                let m = sum[j] / count[j] as f64;
                moved |= m != c[j];
                c[j] = m;
            }
        }
        if !moved {
            break;
        }
    }
    residual
}

/// At most `rounds` rounds of k-means of `values` into `k` centres from `start`. `None` when `values` is empty, `k` is 0, or
/// a given start does not hold `k` centres.
pub fn kmeans_1d(values: &[f64], k: usize, start: Start<'_>, rounds: usize) -> Option<Clusters> {
    let n = values.len();
    if n == 0 || k == 0 {
        return None;
    }
    let mut c: Vec<f64> = match start {
        Start::Spread => {
            let mut s = values.to_vec();
            s.sort_by(f64::total_cmp);
            (0..k)
                .map(|i| s[((i as f64 + 0.5) / k as f64 * (n - 1) as f64).floor() as usize])
                .collect()
        }
        Start::Given(g) if g.len() == k => g.to_vec(),
        Start::Given(_) => return None,
    };
    let mut sum = vec![0.0; k];
    let mut count = vec![0usize; k];
    let residual = lloyd(values, &mut c, &mut sum, &mut count, rounds);
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| c[a].total_cmp(&c[b]));
    Some(Clusters {
        centres: order.iter().map(|&j| c[j]).collect(),
        counts: order.iter().map(|&j| count[j]).collect(),
        residual,
    })
}

/// At most `rounds` rounds of k-means of `values` into `K` centres from `start`, with no allocation; sums are `f64`. `None`
/// when `values` is empty or `K` is 0.
pub fn kmeans_1d_fixed<T: Copy + Into<f64>, const K: usize>(
    values: &[T],
    start: [f64; K],
    rounds: usize,
) -> Option<FixedClusters<K>> {
    if values.is_empty() || K == 0 {
        return None;
    }
    let mut c = start;
    let mut sum = [0.0; K];
    let mut count = [0usize; K];
    let residual = lloyd(values, &mut c, &mut sum, &mut count, rounds);
    let mut order: [usize; K] = std::array::from_fn(|i| i);
    order.sort_unstable_by(|&a, &b| c[a].total_cmp(&c[b]).then(a.cmp(&b)));
    Some(FixedClusters {
        centres: order.map(|j| c[j]),
        counts: order.map(|j| count[j]),
        residual,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_pairs_give_their_means_and_four_levels_need_four_centres() {
        let c = kmeans_1d(&[1.0, 1.1, 3.0, 3.1], 2, Start::Spread, 30).unwrap();
        assert_eq!(c.centres, vec![1.05, 3.05]);
        assert_eq!(c.counts, vec![2, 2]);
        let v: Vec<f64> = (0..400)
            .map(|i| [-3.0, -1.0, 1.0, 3.0][i % 4] + 0.05 * ((i * 7919) % 13) as f64 / 13.0)
            .collect();
        let r2 = kmeans_1d(&v, 2, Start::Spread, 30).unwrap().residual;
        let r4 = kmeans_1d(&v, 4, Start::Spread, 30).unwrap();
        assert!(r4.residual < 0.25 * r2, "{} {r2}", r4.residual);
        assert!(r4.centres.windows(2).all(|w| w[1] - w[0] > 1.9));
        let g = kmeans_1d(
            &[10.0, 11.0, 30.0, 31.0, 29.0],
            2,
            Start::Given(&[10.0, 30.0]),
            30,
        )
        .unwrap();
        assert_eq!((g.nearest(12.0), g.nearest(28.0)), (0, 1));
        assert!(kmeans_1d(&[1.0], 2, Start::Given(&[1.0]), 30).is_none());
    }

    #[test]
    fn known_centres_counts_and_residual() {
        // {0, 1, 2} and {10, 12}: centres 1 and 11, residual 1 + 0 + 1 + 1 + 1 = 4.
        let v = [0.0, 1.0, 2.0, 10.0, 12.0];
        let c = kmeans_1d(&v, 2, Start::Given(&[0.0, 2.0]), 30).unwrap();
        assert_eq!(
            (c.centres.as_slice(), c.counts.as_slice(), c.residual),
            (&[1.0, 11.0][..], &[3, 2][..], 4.0)
        );
        // One round moves the centres once: 0 and 1 (a tie) to the lower centre, 2, 10 and 12 to the upper. The residual is
        // that of the round, against the start: 0 + 1 + 0 + 64 + 100.
        let one = kmeans_1d(&v, 2, Start::Given(&[0.0, 2.0]), 1).unwrap();
        assert_eq!(
            (one.centres.as_slice(), one.counts.as_slice()),
            (&[0.5, 8.0][..], &[2, 3][..])
        );
        assert_eq!(one.residual, 165.0);
    }

    #[test]
    fn the_fixed_form_matches_the_vector_form() {
        let v: Vec<f64> = (0..997)
            .map(|i| ((i * 7919) % 1000) as f64 / 100.0 + [0.0, 20.0, 50.0][i % 3])
            .collect();
        let a = kmeans_1d(&v, 3, Start::Given(&[0.0, 1.0, 2.0]), 50).unwrap();
        let b = kmeans_1d_fixed(&v, [0.0, 1.0, 2.0], 50).unwrap();
        assert_eq!(a.centres, b.centres.to_vec());
        assert_eq!(a.counts, b.counts.to_vec());
        assert_eq!(a.residual, b.residual);
        // f32 values: sums in f64, centres sorted, the nearest index of the sorted centres.
        let f = kmeans_1d_fixed(&[5.0f32, 5.5, -1.0, -1.5], [10.0, -10.0], 30).unwrap();
        assert_eq!((f.centres, f.counts), ([-1.25, 5.25], [2, 2]));
        assert_eq!((f.nearest(-3.0), f.nearest(4.0)), (0, 1));
        assert!(kmeans_1d_fixed::<f64, 2>(&[], [0.0, 1.0], 30).is_none());
    }

    #[test]
    fn a_tie_joins_the_lower_centre() {
        // 1 lies half way between 0 and 2: it joins centre 0.
        let f = kmeans_1d_fixed(&[0.0, 1.0, 2.0], [0.0, 2.0], 1).unwrap();
        assert_eq!((f.centres, f.counts), ([0.5, 2.0], [2, 1]));
    }
}
