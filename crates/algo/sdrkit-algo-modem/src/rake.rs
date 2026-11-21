//! RAKE combining of a few multipath components found on a known sequence: the paths picked from a channel profile, and their
//! despread values added with the conjugates of their gains (maximal-ratio combining) or in power.
//!
//! ```
//! use sdrkit_algo_modem::rake::{combine, select};
//! use sdrkit_algo_modem::Complex32;
//! let c = |re: f32| Complex32::new(re, 0.0);
//! // A main path at lag 0 and an echo 10 samples later at -6 dB.
//! let mut profile = vec![c(0.0); 25];
//! profile[12] = c(1.0);
//! profile[14] = c(0.5);
//! let p = select(&profile, -60, 5, 3, 0.2, 10);
//! assert_eq!(p.len(), 2);
//! assert_eq!((p[0].lag, p[1].lag), (0, 10));
//! let z = combine(&p, &[c(1.0), c(0.5)]);
//! assert!((z.re - 1.25).abs() < 1e-6);
//! ```

use sdrkit_core::Complex32;

/// One path: its lag and its complex gain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Path {
    /// The lag, in the units of the profile's lags (samples).
    pub lag: i32,
    /// The gain.
    pub gain: Complex32,
}

/// Picks paths from a channel profile whose value `i` is at lag `lag0 + i step`: the strongest, then the other local maxima in
/// falling power whose power is at least `min_rel` of the strongest and that lie at least `sep` from every path taken, up to
/// `max` paths. The first path is the strongest. An empty profile gives none.
pub fn select(
    profile: &[Complex32],
    lag0: i32,
    step: i32,
    max: usize,
    min_rel: f32,
    sep: i32,
) -> Vec<Path> {
    let p: Vec<f32> = profile.iter().map(|v| v.norm_sqr()).collect();
    let Some(top) = p
        .iter()
        .copied()
        .fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))))
    else {
        return Vec::new();
    };
    let mut cand: Vec<usize> = (0..p.len())
        .filter(|&i| {
            let left = i == 0 || p[i] >= p[i - 1];
            let right = i + 1 == p.len() || p[i] >= p[i + 1];
            left && right && p[i] >= min_rel * top && p[i] > 0.0
        })
        .collect();
    cand.sort_by(|&a, &b| p[b].total_cmp(&p[a]));
    let mut out: Vec<Path> = Vec::new();
    for i in cand {
        if out.len() >= max {
            break;
        }
        let lag = lag0 + i as i32 * step;
        if out.iter().all(|q| (q.lag - lag).abs() >= sep) {
            out.push(Path {
                lag,
                gain: profile[i],
            });
        }
    }
    out
}

/// The coherent sum `sum conj(g_p) x_p` of the values `x` of each path.
pub fn combine(paths: &[Path], x: &[Complex32]) -> Complex32 {
    paths.iter().zip(x).map(|(p, v)| p.gain.conj() * v).sum()
}

/// The power sum `sum |x_p|^2` of the values `x` of each path (for a channel that changes within a symbol).
pub fn combine_power(x: &[Complex32]) -> f32 {
    x.iter().map(|v| v.norm_sqr()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weak_and_near_paths_are_left_out() {
        let c = |re: f32| Complex32::new(re, 0.0);
        let mut prof = vec![c(0.0); 20];
        prof[5] = c(1.0);
        prof[6] = c(0.9);
        prof[10] = c(0.3);
        prof[15] = c(0.6);
        let p = select(&prof, 0, 1, 3, 0.2, 2);
        let lags: Vec<i32> = p.iter().map(|q| q.lag).collect();
        // 6 is next to 5; 10 is 0.09 of the power, below 0.2.
        assert_eq!(lags, [5, 15]);
        assert!(select(&[], 0, 1, 3, 0.2, 1).is_empty());
        assert_eq!(combine_power(&[c(3.0), c(4.0)]), 25.0);
    }
}
