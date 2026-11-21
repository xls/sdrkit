//! The level relation of two tracked peaks: a straight-line fit of one level series against another over the snapshots
//! where both exist, and a verdict whether the second follows the first dB for dB. A product of a nonlinearity whose level
//! must move `a` dB for each dB of a parent `A` and `b` dB for each dB of `B` follows `x = a L_A + b L_B` with slope 1.
//! Also the level of a snapshot spectrum near a frequency. The rules are general statistics; no public document states the
//! thresholds.
//!
//! ```
//! use sdrkit_algo_dsp::levelfit::{fit, LevelVerdict};
//! let x: Vec<Option<f64>> = (0..50).map(|i| Some((i as f64 * 0.3).sin() * 3.0)).collect();
//! let y: Vec<Option<f64>> = x.iter().map(|v| v.map(|v| v + 3.0)).collect();
//! let f = fit(&x, &y).unwrap();
//! assert_eq!(f.verdict(), LevelVerdict::Follows);
//! ```

/// What a level fit says of the second series against the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelVerdict {
    /// Too few points, or the first series spread less than 1 dB.
    CannotTell,
    /// Slope 0.7 to 1.35 and correlation over 0.8.
    Follows,
    /// Slope under 0.3 in size, or correlation under 0.4.
    Independent,
    /// Neither.
    Unclear,
}

/// The fit of `y` against `x` over the points where both exist.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelFit {
    /// The points.
    pub n: usize,
    /// `cov / var(x)`; `None` when `var(x)` is 1e-9 or less.
    pub slope: Option<f64>,
    /// `cov / sqrt(var(x) var(y))`; `None` when either variance is 1e-9 or less.
    pub correlation: Option<f64>,
    /// The population standard deviation of `x`, dB.
    pub sd_x: f64,
}

/// The least points of a verdict other than "cannot tell".
pub const MIN_POINTS: usize = 12;
/// The least spread of `x` of a verdict other than "cannot tell", dB.
pub const MIN_SPREAD_DB: f64 = 1.0;

/// Fits `y` against `x` (equal lengths; `None` where a series was absent). `None` with fewer than 3 common points.
pub fn fit(x: &[Option<f64>], y: &[Option<f64>]) -> Option<LevelFit> {
    let pts: Vec<(f64, f64)> = x
        .iter()
        .zip(y)
        .filter_map(|(a, b)| Some(((*a)?, (*b)?)))
        .collect();
    let n = pts.len();
    if n < 3 {
        return None;
    }
    let m = n as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / m;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / m;
    let (mut vx, mut vy, mut c) = (0.0, 0.0, 0.0);
    for (a, b) in &pts {
        vx += (a - mx) * (a - mx);
        vy += (b - my) * (b - my);
        c += (a - mx) * (b - my);
    }
    let (vx, vy, c) = (vx / m, vy / m, c / m);
    Some(LevelFit {
        n,
        slope: (vx > 1e-9).then(|| c / vx),
        correlation: (vx > 1e-9 && vy > 1e-9).then(|| c / (vx * vy).sqrt()),
        sd_x: vx.sqrt(),
    })
}

impl LevelFit {
    /// The verdict, by the first rule that holds: cannot tell, follows, independent, unclear.
    pub fn verdict(&self) -> LevelVerdict {
        if self.n < MIN_POINTS || self.sd_x < MIN_SPREAD_DB {
            return LevelVerdict::CannotTell;
        }
        let s = self.slope.unwrap_or(0.0);
        let r = self.correlation.unwrap_or(0.0);
        if (0.7..=1.35).contains(&s) && r > 0.8 {
            LevelVerdict::Follows
        } else if s.abs() < 0.3 || r < 0.4 {
            LevelVerdict::Independent
        } else {
            LevelVerdict::Unclear
        }
    }
}

/// The largest `10 log10(power / noise)` within `half` bins of the signed bin `bin` of a spectrum in signed order (element
/// `i` is bin `i - n / 2`), dB; `None` when no such bin is in the spectrum.
pub fn level_near(power: &[f32], noise: f32, bin: f64, half: i64) -> Option<f64> {
    let n = power.len() as i64;
    let centre = bin.round() as i64 + n / 2;
    let best = (centre - half..=centre + half)
        .filter(|&i| (0..n).contains(&i))
        .map(|i| f64::from(power[i as usize]))
        .max_by(f64::total_cmp)?;
    Some(10.0 * (best / f64::from(noise).max(1e-30)).max(1e-30).log10())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wave(n: usize, spread: f64) -> Vec<Option<f64>> {
        // A sine of `spread` dB standard deviation.
        (0..n)
            .map(|i| Some(spread * 2f64.sqrt() * (i as f64 * std::f64::consts::TAU / 25.0).sin()))
            .collect()
    }

    #[test]
    fn a_follower_a_constant_a_small_spread_and_few_points() {
        let x = wave(50, 2.0);
        let y: Vec<Option<f64>> = x.iter().map(|v| v.map(|v| v + 3.0)).collect();
        let f = fit(&x, &y).unwrap();
        assert!(
            (f.slope.unwrap() - 1.0).abs() < 1e-9 && (f.correlation.unwrap() - 1.0).abs() < 1e-9
        );
        assert!((f.sd_x - 2.0).abs() < 0.01, "{}", f.sd_x);
        assert_eq!(f.verdict(), LevelVerdict::Follows);
        let flat = vec![Some(-20.0); 50];
        assert_eq!(fit(&x, &flat).unwrap().verdict(), LevelVerdict::Independent);
        let small = wave(50, 0.5);
        let ys: Vec<Option<f64>> = small.iter().map(|v| v.map(|v| v + 3.0)).collect();
        assert_eq!(
            fit(&small, &ys).unwrap().verdict(),
            LevelVerdict::CannotTell
        );
        assert_eq!(
            fit(&x[..11], &y[..11]).unwrap().verdict(),
            LevelVerdict::CannotTell
        );
        assert!(fit(&x[..2], &y[..2]).is_none());
        let gaps: Vec<Option<f64>> = (0..50)
            .map(|i| if i % 2 == 0 { y[i] } else { None })
            .collect();
        assert_eq!(fit(&x, &gaps).unwrap().n, 25);
    }

    #[test]
    fn the_level_near_a_bin_is_the_largest_within_the_half_width() {
        let mut p = vec![1.0f32; 64];
        p[32 + 5] = 100.0;
        assert!((level_near(&p, 1.0, 4.0, 2).unwrap() - 20.0).abs() < 1e-6);
        assert!((level_near(&p, 1.0, 0.0, 2).unwrap()).abs() < 1e-6);
        assert!(level_near(&p, 1.0, 100.0, 2).is_none());
    }
}
