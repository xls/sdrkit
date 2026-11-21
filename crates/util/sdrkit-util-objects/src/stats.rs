//! Counts by type and running statistics.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Running statistics of a measured value (the SNR or the level of a station): the last value, the least, the most and the mean.
/// Values that are not finite are not counted.
///
/// ```
/// use sdrkit_util_objects::Stats;
/// let mut snr = Stats::default();
/// for v in [4.0, 10.0, f64::NAN, 7.0] {
///     snr.add(v);
/// }
/// assert_eq!((snr.count, snr.last, snr.min, snr.max), (3, Some(7.0), Some(4.0), Some(10.0)));
/// assert_eq!(snr.mean(), Some(7.0));
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    /// Values counted.
    pub count: u64,
    /// The last value.
    pub last: Option<f64>,
    /// The least value.
    pub min: Option<f64>,
    /// The most value.
    pub max: Option<f64>,
    /// The sum of the values.
    pub sum: f64,
}

impl Stats {
    /// Counts a value. A value that is not finite is not counted. Returns true if it was counted.
    pub fn add(&mut self, v: f64) -> bool {
        if !v.is_finite() {
            return false;
        }
        self.count += 1;
        self.last = Some(v);
        self.min = Some(self.min.map_or(v, |m| m.min(v)));
        self.max = Some(self.max.map_or(v, |m| m.max(v)));
        self.sum += v;
        true
    }

    /// The mean, if any value was counted.
    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }
}

/// Counts by kind (frames by type, messages by label), with a cap on the number of kinds.
///
/// ```
/// use sdrkit_util_objects::Tally;
/// let mut t = Tally::new(100);
/// t.add("position");
/// t.add("position");
/// t.add("telemetry");
/// assert_eq!(t.get(&"position"), 2);
/// assert_eq!(t.total(), 3);
/// ```
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Tally<K: Ord> {
    counts: BTreeMap<K, u64>,
    cap: usize,
    /// Counts of kinds that came after the cap was reached.
    pub other: u64,
}

impl<K: Ord> Tally<K> {
    /// No counts. At most `kinds` kinds are counted one by one. Later kinds are added to [`Tally::other`].
    pub fn new(kinds: usize) -> Self {
        Tally {
            counts: BTreeMap::new(),
            cap: kinds.max(1),
            other: 0,
        }
    }

    /// Counts one of `kind`.
    pub fn add(&mut self, kind: K) {
        if let Some(c) = self.counts.get_mut(&kind) {
            *c = c.saturating_add(1);
        } else if self.counts.len() < self.cap {
            self.counts.insert(kind, 1);
        } else {
            self.other = self.other.saturating_add(1);
        }
    }

    /// The count of `kind`.
    pub fn get(&self, kind: &K) -> u64 {
        self.counts.get(kind).copied().unwrap_or(0)
    }

    /// Every count, including [`Tally::other`].
    pub fn total(&self) -> u64 {
        self.counts.values().sum::<u64>() + self.other
    }

    /// The kinds and their counts, in the order of the kinds.
    pub fn iter(&self) -> impl Iterator<Item = (&K, u64)> {
        self.counts.iter().map(|(k, &c)| (k, c))
    }

    /// Forgets every count.
    pub fn clear(&mut self) {
        self.counts.clear();
        self.other = 0;
    }
}

/// Statistics hold no times.
impl crate::Shift for Stats {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// Counts hold no times.
impl<K: Ord> crate::Shift for Tally<K> {
    fn shift_times(&mut self, _by_s: f64) {}
}
