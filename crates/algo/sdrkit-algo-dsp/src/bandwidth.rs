//! Bandwidth measures of a power spectrum (bins in frequency order, linear power).
//!
//! - [`bridged_runs`]: every run of hot bins, where gaps of up to a given number of cold bins do not end a run.
//! - [`bridged_region`]: the one of those runs with the most power.
//! - [`power_share_edges`]: the band that holds a share of the power of a range, with half the rest below it and half above
//!   (ITU-R SM.328: the occupied bandwidth is the band with 0.5 % of the power below and 0.5 % above).
//! - [`x_db_edges`]: the outermost bins on each side of the peak that are no more than `x` dB under it (the x dB bandwidth).

/// A run of bins: first, last (inclusive), and its power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    /// The first bin.
    pub lo: usize,
    /// The last bin.
    pub hi: usize,
    /// The sum of the power over the run.
    pub power: f64,
}

/// The runs of hot values of `hot`, in order, as first and last index (inclusive): a run starts at a hot value, goes on over
/// gaps of at most `max_gap` cold values, and ends at its last hot value.
pub fn bridged_runs<I>(hot: I, max_gap: usize) -> BridgedRuns<I::IntoIter>
where
    I: IntoIterator<Item = bool>,
{
    BridgedRuns {
        hot: hot.into_iter(),
        at: 0,
        run: None,
        max_gap,
    }
}

/// The iterator of [`bridged_runs`].
#[derive(Clone, Debug)]
pub struct BridgedRuns<I> {
    hot: I,
    at: usize,
    run: Option<(usize, usize)>,
    max_gap: usize,
}

impl<I: Iterator<Item = bool>> Iterator for BridgedRuns<I> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        for h in self.hot.by_ref() {
            let k = self.at;
            self.at += 1;
            if !h {
                continue;
            }
            match self.run {
                Some((lo, hi)) if k - hi - 1 <= self.max_gap => self.run = Some((lo, k)),
                Some(done) => {
                    self.run = Some((k, k));
                    return Some(done);
                }
                None => self.run = Some((k, k)),
            }
        }
        self.run.take()
    }
}

/// The run of hot bins (`hot[k]`) with the most power of `power`, of the runs of [`bridged_runs`]; of runs with the same
/// power the first. `None` without a hot bin.
pub fn bridged_region(hot: &[bool], power: &[f64], max_gap: usize) -> Option<Region> {
    let mut best: Option<Region> = None;
    for (lo, hi) in bridged_runs(hot.iter().copied(), max_gap) {
        let power = power[lo..=hi].iter().sum::<f64>();
        if best.is_none_or(|b| power > b.power) {
            best = Some(Region { lo, hi, power });
        }
    }
    best
}

/// The edges of the band in `power[lo..=hi]` with the share `share` of its power: the lower edge is the first bin from `lo`
/// where the running sum reaches `(1 - share) / 2` of the total, the upper edge the first bin from `hi` downwards where it
/// does. `None` when the range is empty or holds no power.
pub fn power_share_edges(
    power: &[f64],
    lo: usize,
    hi: usize,
    share: f64,
) -> Option<(usize, usize)> {
    if lo > hi || hi >= power.len() {
        return None;
    }
    let total: f64 = power[lo..=hi].iter().sum();
    if total.is_nan() || total <= 0.0 {
        return None;
    }
    let tail = 0.5 * (1.0 - share) * total;
    let mut sum = 0.0;
    let mut a = lo;
    for (k, &p) in power.iter().enumerate().take(hi + 1).skip(lo) {
        sum += p;
        a = k;
        if sum >= tail {
            break;
        }
    }
    sum = 0.0;
    let mut b = hi;
    for k in (lo..=hi).rev() {
        sum += power[k];
        b = k;
        if sum >= tail {
            break;
        }
    }
    Some((a.min(b), a.max(b)))
}

/// The peak of `power[lo..=hi]` and the outermost bins below and above it in the range that are at least the peak times
/// `10^(-x_db / 10)`: `(low edge, peak, high edge)`. `None` for an empty range.
pub fn x_db_edges(power: &[f64], lo: usize, hi: usize, x_db: f64) -> Option<(usize, usize, usize)> {
    if lo > hi || hi >= power.len() {
        return None;
    }
    let (peak, &top) = power[lo..=hi]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, v)| (k + lo, v))?;
    let min = top * 10f64.powf(-x_db / 10.0);
    let a = (lo..=peak).find(|&k| power[k] >= min).unwrap_or(peak);
    let b = (peak..=hi).rev().find(|&k| power[k] >= min).unwrap_or(peak);
    Some((a, peak, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_region_with_the_most_power_wins_over_a_wider_weak_one() {
        let mut hot = vec![false; 100];
        let mut p = vec![0.0; 100];
        for k in 10..40 {
            hot[k] = k % 5 != 0;
            p[k] = 1.0;
        }
        for k in 60..70 {
            hot[k] = true;
            p[k] = 10.0;
        }
        let r = bridged_region(&hot, &p, 4).unwrap();
        assert_eq!((r.lo, r.hi, r.power), (60, 69, 100.0));
        let r = bridged_region(&hot[..50], &p[..50], 4).unwrap();
        assert_eq!((r.lo, r.hi), (11, 39));
        // A gap longer than the bridge splits the run.
        let r = bridged_region(&hot[..50], &p[..50], 0).unwrap();
        assert_eq!(r.hi - r.lo, 3);
    }

    #[test]
    fn every_run_is_given_in_order_with_gaps_up_to_the_bridge_inside() {
        let hot = |s: &str| s.bytes().map(|c| c == b'x').collect::<Vec<_>>();
        let runs = |s: &str, gap| bridged_runs(hot(s), gap).collect::<Vec<_>>();
        assert_eq!(runs("..xx.x...x..", 1), [(2, 5), (9, 9)]);
        assert_eq!(runs("..xx.x...x..", 0), [(2, 3), (5, 5), (9, 9)]);
        assert_eq!(runs("x.xx.x...x.x", 3), [(0, 11)]);
        assert_eq!(runs("....", 2), []);
        assert_eq!(runs("", 2), []);
    }

    #[test]
    fn a_flat_band_holds_99_percent_inside_its_edges_and_a_triangle_has_its_x_db_points() {
        let mut p = vec![0.0; 300];
        for v in &mut p[100..200] {
            *v = 1.0;
        }
        assert_eq!(power_share_edges(&p, 50, 250, 0.99), Some((100, 199)));
        assert_eq!(power_share_edges(&p, 50, 250, 0.9), Some((104, 195)));
        let tri: Vec<f64> = (0..101).map(|k| 50.0 - (k as f64 - 50.0).abs()).collect();
        // 6 dB under the peak is a quarter of 50: 12.5, reached from bin 13 to 87.
        assert_eq!(
            x_db_edges(&tri, 0, 100, 10.0 * 4f64.log10()),
            Some((13, 50, 87))
        );
    }
}
