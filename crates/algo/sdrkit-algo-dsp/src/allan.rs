//! The Allan deviation of a series of readings: the non-overlapping estimator of IEEE Std 1139 and ITU-R TF.538.
//!
//! For readings `y_1 .. y_m`, one per window of length `tau0`, and an averaging factor `t`: `G = floor(m / t)` groups of `t`
//! consecutive readings from the oldest, the group means `Y_g`, and
//! `sigma(t tau0) = sqrt( sum_{g=2..G} (Y_g - Y_(g-1))^2 / (2 (G - 1)) )`. A factor is given only when `G >= 4`. White
//! frequency noise of standard deviation `s` per reading gives `sigma(tau0) = s`.
//!
//! ```
//! use sdrkit_algo_dsp::allan::allan_deviation;
//! let y = [1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0];
//! // Steps of 1 between every pair: sqrt(1 / 2).
//! assert!((allan_deviation(&y, 1).unwrap().sigma - 0.5f64.sqrt()).abs() < 1e-12);
//! assert!(allan_deviation(&y, 4).is_none());
//! ```

/// The Allan deviation at one averaging factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Allan {
    /// The averaging factor `t`.
    pub factor: usize,
    /// The deviation, in the unit of the readings.
    pub sigma: f64,
    /// The groups `G` it is from.
    pub groups: usize,
}

/// The least number of groups for a value.
pub const MIN_GROUPS: usize = 4;

/// The non-overlapping Allan deviation of `y` (the oldest first) at the averaging factor `t`. `None` when `t` is 0 or fewer
/// than [`MIN_GROUPS`] groups exist.
pub fn allan_deviation(y: &[f64], t: usize) -> Option<Allan> {
    if t == 0 {
        return None;
    }
    let groups = y.len() / t;
    if groups < MIN_GROUPS {
        return None;
    }
    let mean = |g: usize| y[g * t..(g + 1) * t].iter().sum::<f64>() / t as f64;
    let mut prev = mean(0);
    let mut sum = 0.0;
    for g in 1..groups {
        let m = mean(g);
        sum += (m - prev) * (m - prev);
        prev = m;
    }
    Some(Allan {
        factor: t,
        sigma: (sum / (2.0 * (groups - 1) as f64)).sqrt(),
        groups,
    })
}

/// The deviations at the factors 1, 2, 4, ... up to `max_factor`, stopping at the first factor with fewer than
/// [`MIN_GROUPS`] groups.
pub fn allan_octaves(y: &[f64], max_factor: usize) -> Vec<Allan> {
    let mut out = Vec::new();
    let mut t = 1;
    while t <= max_factor {
        match allan_deviation(y, t) {
            Some(a) => out.push(a),
            None => break,
        }
        t *= 2;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_frequency_noise_reads_its_deviation_a_constant_reads_zero() {
        // A Gaussian series of standard deviation 0.2 from SplitMix64 and Box-Muller.
        let mut s = 12345u64;
        let mut u = || {
            s = s.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = s;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            (((z ^ (z >> 31)) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        for _ in 0..5 {
            let y: Vec<f64> = (0..400)
                .map(|_| 0.2 * (-2.0 * u().ln()).sqrt() * (std::f64::consts::TAU * u()).cos())
                .collect();
            let a = allan_deviation(&y, 1).unwrap();
            assert!((a.sigma - 0.2).abs() < 0.02, "{}", a.sigma);
        }
        let c = vec![3.5; 64];
        let o = allan_octaves(&c, 16);
        assert_eq!(o.len(), 5);
        assert!(o.iter().all(|a| a.sigma == 0.0));
        assert_eq!(allan_octaves(&c[..15], 16).len(), 2);
        assert!(allan_deviation(&c[..3], 1).is_none());
    }
}
