//! Hop planning: the centre of each hop, the part of each capture that is kept, and the order of the visits. Hops overlap,
//! hops inside an exclusion are skipped, and a priority hop is visited again after a number of hops.

use crate::SweepError;

/// A frequency range, Hz.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
}

impl Range {
    /// `low_hz ..= high_hz`.
    pub fn new(low_hz: f64, high_hz: f64) -> Self {
        Range { low_hz, high_hz }
    }
    /// True when the range covers all of `other`.
    pub fn covers(&self, other: &Range) -> bool {
        self.low_hz <= other.low_hz && self.high_hz >= other.high_hz
    }
}

/// What a plan is made from.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanConfig {
    /// The low end of the range, Hz.
    pub low_hz: f64,
    /// The high end of the range, Hz.
    pub high_hz: f64,
    /// The radio's sample rate, Hz.
    pub rate_hz: f64,
    /// The share of the rate kept around each centre (the filter edges are dropped). Default 0.75.
    pub usable: f64,
    /// The least share of the kept width two neighbouring hops share (0 to 0.5). Default 0.1.
    pub overlap: f64,
    /// Ranges never visited: a hop whose kept part lies inside one is skipped (and their bins are masked by the detector).
    pub exclusions: Vec<Range>,
    /// Frequencies whose hops are visited more often.
    pub priority_hz: Vec<f64>,
    /// A priority hop is visited again after every this many hops (0: never).
    pub priority_every: usize,
    /// The radio's tuning limits: centres are kept inside them.
    pub tune_limits: Option<Range>,
}

impl PlanConfig {
    #[cfg(test)]
    /// A plan over `low_hz .. high_hz` at `rate_hz` with the defaults.
    pub fn new(low_hz: f64, high_hz: f64, rate_hz: f64) -> Self {
        PlanConfig {
            low_hz,
            high_hz,
            rate_hz,
            usable: 0.75,
            overlap: 0.1,
            exclusions: Vec::new(),
            priority_hz: Vec::new(),
            priority_every: 8,
            tune_limits: None,
        }
    }
}

/// One hop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hop {
    /// Its place in [`HopPlan::hops`].
    pub index: usize,
    /// Where the radio is tuned, Hz.
    pub centre_hz: f64,
    /// The low edge of the part kept, Hz.
    pub keep_lo_hz: f64,
    /// The high edge of the part kept, Hz.
    pub keep_hi_hz: f64,
}

/// The hops of a sweep and the order they are visited in.
#[derive(Clone, Debug, PartialEq)]
pub struct HopPlan {
    /// The hops, lowest first.
    pub hops: Vec<Hop>,
    /// Indices into `hops`, in visiting order for one sweep (priority hops appear more than once).
    pub order: Vec<usize>,
    /// The width kept around each centre, Hz.
    pub keep_hz: f64,
    /// The rate, Hz.
    pub rate_hz: f64,
}

impl HopPlan {
    /// Plans the hops.
    pub fn new(cfg: &PlanConfig) -> Result<Self, SweepError> {
        if !(cfg.rate_hz.is_finite() && cfg.rate_hz > 0.0) {
            return Err(SweepError::BadRate {
                rate_hz: cfg.rate_hz,
            });
        }
        if cfg.high_hz.is_nan() || cfg.low_hz.is_nan() || cfg.high_hz <= cfg.low_hz {
            return Err(SweepError::EmptyRange {
                low_hz: cfg.low_hz,
                high_hz: cfg.high_hz,
            });
        }
        let overlap = cfg.overlap.clamp(0.0, 0.5);
        let keep = cfg.usable.clamp(0.05, 1.0) * cfg.rate_hz;
        let span = cfg.high_hz - cfg.low_hz;
        let mut centres = Vec::new();
        if span <= keep {
            centres.push((cfg.low_hz + cfg.high_hz) / 2.0);
        } else {
            let step = keep * (1.0 - overlap);
            let n = ((span - keep) / step - 1e-9).ceil() as usize + 1;
            let actual = (span - keep) / (n - 1) as f64;
            for k in 0..n {
                centres.push(cfg.low_hz + keep / 2.0 + k as f64 * actual);
            }
        }
        let mut hops = Vec::with_capacity(centres.len());
        for c in centres {
            let mut c = c;
            if let Some(l) = cfg.tune_limits {
                c = c.clamp(l.low_hz, l.high_hz);
            }
            let lo = (c - keep / 2.0).max(cfg.low_hz);
            let hi = (c + keep / 2.0).min(cfg.high_hz);
            if hi <= lo {
                continue;
            }
            let kept = Range::new(lo, hi);
            if cfg.exclusions.iter().any(|e| e.covers(&kept)) {
                continue;
            }
            hops.push(Hop {
                index: hops.len(),
                centre_hz: c,
                keep_lo_hz: lo,
                keep_hi_hz: hi,
            });
        }
        if hops.is_empty() {
            return Err(SweepError::AllExcluded);
        }
        let priority: Vec<usize> = hops
            .iter()
            .filter(|h| {
                cfg.priority_hz
                    .iter()
                    .any(|&f| f >= h.keep_lo_hz && f <= h.keep_hi_hz)
            })
            .map(|h| h.index)
            .collect();
        let mut order = Vec::with_capacity(hops.len() * 2);
        for h in &hops {
            order.push(h.index);
            if !priority.is_empty()
                && cfg.priority_every > 0
                && (h.index + 1) % cfg.priority_every == 0
            {
                for &p in &priority {
                    if order.last() != Some(&p) {
                        order.push(p);
                    }
                }
            }
        }
        Ok(HopPlan {
            hops,
            order,
            keep_hz: keep,
            rate_hz: cfg.rate_hz,
        })
    }

    /// The hop whose kept part holds `hz` nearest its centre.
    pub fn hop_for(&self, hz: f64) -> Option<&Hop> {
        self.hops
            .iter()
            .filter(|h| hz >= h.keep_lo_hz && hz <= h.keep_hi_hz)
            .min_by(|a, b| {
                (a.centre_hz - hz)
                    .abs()
                    .total_cmp(&(b.centre_hz - hz).abs())
            })
    }

    /// The number of distinct hops.
    pub fn len(&self) -> usize {
        self.hops.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hops_cover_the_range_with_overlap() {
        let cfg = PlanConfig::new(88e6, 108e6, 2.4e6);
        let p = HopPlan::new(&cfg).unwrap();
        assert_eq!(p.keep_hz, 1.8e6);
        assert_eq!(p.hops[0].keep_lo_hz, 88e6);
        assert!((p.hops.last().unwrap().keep_hi_hz - 108e6).abs() < 1.0);
        for w in p.hops.windows(2) {
            let shared = w[0].keep_hi_hz - w[1].keep_lo_hz;
            assert!(shared >= 0.1 * 1.8e6 - 1.0, "shared {shared}");
        }
        assert_eq!(p.order, (0..p.hops.len()).collect::<Vec<_>>());
    }

    #[test]
    fn a_narrow_range_is_one_hop() {
        let p = HopPlan::new(&PlanConfig::new(100e6, 101e6, 2.4e6)).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p.hops[0].centre_hz, 100.5e6);
        assert_eq!((p.hops[0].keep_lo_hz, p.hops[0].keep_hi_hz), (100e6, 101e6));
    }

    #[test]
    fn exclusions_skip_hops_and_priority_repeats() {
        let mut cfg = PlanConfig::new(100e6, 120e6, 2e6);
        cfg.overlap = 0.0;
        cfg.exclusions.push(Range::new(104e6, 110e6));
        cfg.priority_hz.push(118.2e6);
        cfg.priority_every = 3;
        let p = HopPlan::new(&cfg).unwrap();
        assert!(p
            .hops
            .iter()
            .all(|h| !(h.keep_lo_hz >= 104e6 && h.keep_hi_hz <= 110e6)));
        let pri = p.hop_for(118.2e6).unwrap().index;
        let visits = p.order.iter().filter(|&&i| i == pri).count();
        assert!(visits >= 2, "{:?}", p.order);
        cfg.exclusions = vec![Range::new(0.0, 1e9)];
        assert!(HopPlan::new(&cfg).is_err());
    }
}
