//! The complementary cumulative distribution (CCDF) of instantaneous power: the level over the mean power that a share `t`
//! of the samples exceeds, the peak and the crest factor. For `n` powers with mean `m` the level at share `t` is
//! `10 log10(P_(k) / m)`, `P_(k)` the `k`-th smallest from 0 with `k = floor((1 - t) n)` capped at `n - 1`. For complex
//! Gaussian noise the level is `10 log10(-ln t)`: 3.62, 6.63, 8.39 and 9.64 dB at 10, 1, 0.1 and 0.01 %. General theory; no
//! standard.
//!
//! ```
//! use sdrkit_algo_dsp::ccdf::ccdf;
//! let p = [1.0, 1.0, 1.0, 1.0];
//! let c = ccdf(&p, &[0.1], &mut Vec::new()).unwrap();
//! assert!(c.crest_db.abs() < 1e-12 && c.levels_db[0].abs() < 1e-12);
//! ```

/// The CCDF readings of a list of powers.
#[derive(Clone, Debug, PartialEq)]
pub struct Ccdf {
    /// The mean power.
    pub mean: f64,
    /// The largest power.
    pub peak: f64,
    /// `10 log10(peak / mean)`, dB.
    pub crest_db: f64,
    /// The level over the mean exceeded by each share asked for, dB, in the order asked.
    pub levels_db: Vec<f64>,
}

/// The CCDF of `powers` at `shares` (each in `(0, 1)`), with `scratch` for the copy. `None` for an empty list or a mean of
/// 1e-18 or less.
pub fn ccdf(powers: &[f64], shares: &[f64], scratch: &mut Vec<f64>) -> Option<Ccdf> {
    let n = powers.len();
    if n == 0 {
        return None;
    }
    let mean = powers.iter().sum::<f64>() / n as f64;
    if mean <= 1e-18 {
        return None;
    }
    let peak = powers.iter().copied().fold(f64::MIN, f64::max);
    scratch.clear();
    scratch.extend_from_slice(powers);
    // Indices from the smallest up, so that each selection works on the upper part left by the one before.
    let mut order: Vec<(usize, usize)> = shares
        .iter()
        .enumerate()
        .map(|(j, &t)| {
            (
                j,
                (((1.0 - t) * n as f64).floor().max(0.0) as usize).min(n - 1),
            )
        })
        .collect();
    order.sort_by_key(|o| o.1);
    let mut levels = vec![0.0; shares.len()];
    let mut from = 0;
    for (j, k) in order {
        let (_, v, _) = scratch[from..].select_nth_unstable_by(k - from, f64::total_cmp);
        levels[j] = 10.0 * (*v / mean).max(1e-30).log10();
        from = k;
    }
    Some(Ccdf {
        mean,
        peak,
        crest_db: 10.0 * (peak / mean).log10(),
        levels_db: levels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exponential powers (the power of complex Gaussian noise) from a fixed generator.
    fn exponential(n: usize, mut s: u64) -> Vec<f64> {
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                let u = ((s >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
                -u.ln()
            })
            .collect()
    }

    #[test]
    fn gaussian_noise_gives_the_levels_of_theory() {
        let shares = [0.1, 0.01, 0.001, 0.0001];
        let bounds = [0.1, 0.15, 0.3, 0.6];
        for seed in [1, 2, 3] {
            let p = exponential(1 << 20, 0x9e37_79b9_7f4a_7c15 ^ seed);
            let c = ccdf(&p, &shares, &mut Vec::new()).unwrap();
            for ((t, b), l) in shares.iter().zip(bounds).zip(&c.levels_db) {
                let want = 10.0 * (-t.ln()).log10();
                assert!(
                    (l - want).abs() <= b,
                    "seed {seed} share {t}: {l} against {want}"
                );
            }
        }
    }

    #[test]
    fn the_order_of_the_shares_does_not_matter_and_an_empty_list_has_none() {
        let p = exponential(10_000, 77);
        let a = ccdf(&p, &[0.001, 0.1, 0.01], &mut Vec::new()).unwrap();
        let b = ccdf(&p, &[0.1, 0.01, 0.001], &mut Vec::new()).unwrap();
        assert_eq!(
            a.levels_db,
            vec![b.levels_db[2], b.levels_db[0], b.levels_db[1]]
        );
        assert!(ccdf(&[], &[0.1], &mut Vec::new()).is_none());
        assert!(ccdf(&[0.0, 0.0], &[0.1], &mut Vec::new()).is_none());
    }
}
