//! The tracked span of `intermod` and `spur`: Hann 16384-point snapshots of about a quarter second, the narrow peaks over a
//! threshold followed from snapshot to snapshot, the strong steady tracks, the level series of a track over a range of
//! snapshots, and the verdict that holds for two evaluations before a row.

use sdrkit_algo_dsp::snapshot::{snapshot_peaks, PeakTrack, PeakTracker, SnapshotPeak, Snapshots};
use sdrkit_core::Complex32;

/// The points of the FFT.
pub const FFT_N: usize = 16384;
/// Signed bins `-DC..=DC` hold no peak.
pub const DC_BINS: i64 = 3;
/// The most peaks of a snapshot.
pub const MAX_PEAKS: usize = 200;
/// A peak joins a track within this many bins.
pub const GATE_BINS: f64 = 2.5;
/// The history of a track, snapshots.
pub const HISTORY: usize = 240;
/// A track not seen for more than this many snapshots is dropped.
pub const DROP_AFTER: usize = 60;
/// The first evaluation, snapshots.
pub const FIRST_EVALUATION: u64 = 16;
/// Snapshots between evaluations.
pub const EVALUATE_EVERY: u64 = 8;
/// The fit uses at most this many of the last snapshots.
pub const FIT_SPAN: u64 = 200;
/// A track is recent when seen in this many of the last snapshots.
pub const RECENT: usize = 3;
/// A match lies this many bins from its place at most.
pub const MATCH_BINS: f64 = 2.5;
/// A match is present in at least this many snapshots.
pub const MIN_PRESENT: usize = 6;
/// A match stands at least this far under its signal, dB.
pub const MIN_BELOW_DB: f64 = 3.0;
/// Two findings are the same within this, Hz.
pub const SAME_HZ: f64 = 30.0;

/// A strong steady track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strong {
    /// The index of the track in [`Span::tracks`].
    pub index: usize,
    /// The smoothed frequency, signed bins.
    pub bin: f64,
    /// The mean of its present levels, dB over the noise.
    pub mean: f64,
}

/// The tracked span.
#[derive(Debug)]
pub struct Span {
    spec: Option<Snapshots>,
    tracker: PeakTracker,
    peaks: Vec<SnapshotPeak>,
    last_power: Vec<f32>,
    last_noise: f32,
    threshold_db: f64,
    rate: f64,
}

impl Default for Span {
    fn default() -> Self {
        Span {
            spec: None,
            tracker: PeakTracker::new(GATE_BINS, HISTORY, DROP_AFTER, 2 * MAX_PEAKS),
            peaks: Vec::new(),
            last_power: Vec::new(),
            last_noise: 0.0,
            threshold_db: 10.0,
            rate: 0.0,
        }
    }
}

impl Span {
    /// A span at `rate_hz` with snapshots of about `snapshot_s` and peaks `threshold_db` over the noise.
    pub fn new(rate_hz: f64, snapshot_s: f64, threshold_db: f64) -> Span {
        let k = ((snapshot_s * rate_hz / FFT_N as f64).round() as usize).max(1);
        Span {
            spec: Snapshots::new(FFT_N, k).ok(),
            threshold_db,
            rate: rate_hz,
            ..Span::default()
        }
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / FFT_N as f64
    }

    /// The snapshots taken.
    pub fn snapshots(&self) -> u64 {
        self.tracker.snapshots()
    }

    /// The tracks.
    pub fn tracks(&self) -> &[PeakTrack] {
        self.tracker.tracks()
    }

    /// The length of a snapshot, s.
    pub fn snapshot_s(&self) -> f64 {
        self.spec
            .as_ref()
            .map_or(0.0, |s| s.samples() as f64 / self.rate.max(1.0))
    }

    /// The largest bin of the last snapshot within 2 bins of `bin` over its noise, dB.
    pub fn level_near(&self, bin: f64) -> Option<f64> {
        sdrkit_algo_dsp::levelfit::level_near(&self.last_power, self.last_noise, bin, 2)
    }

    /// Takes `x`; calls `f` after each snapshot that is an evaluation (the 16th and every 8th after).
    pub fn push(&mut self, x: &[Complex32], mut f: impl FnMut(&Span)) {
        let Some(mut spec) = self.spec.take() else {
            return;
        };
        // A piece of one block completes at most one snapshot.
        for piece in x.chunks(FFT_N) {
            let mut due = false;
            spec.push(piece, |s| {
                self.peaks.clear();
                snapshot_peaks(
                    s.power,
                    s.noise,
                    self.threshold_db,
                    DC_BINS,
                    MAX_PEAKS,
                    &mut self.peaks,
                );
                self.tracker.update(&self.peaks);
                self.last_power.clear();
                self.last_power.extend_from_slice(s.power);
                self.last_noise = s.noise;
                let k = self.tracker.snapshots();
                due =
                    k >= FIRST_EVALUATION && (k - FIRST_EVALUATION).is_multiple_of(EVALUATE_EVERY);
            });
            if due {
                f(self);
            }
        }
        self.spec = Some(spec);
    }

    /// The tracks present in at least `share` of `min(history, snapshots)`, seen recently, with a mean level over
    /// `min_db`; the strongest first, at most `max`.
    pub fn strong(&self, share: f64, min_db: f64, max: usize) -> Vec<Strong> {
        let k = self.snapshots() as usize;
        let mut v: Vec<Strong> = self
            .tracks()
            .iter()
            .enumerate()
            .filter(|(_, t)| t.seen_within(RECENT))
            .filter_map(|(i, t)| {
                let (n, mean) = presence(t);
                let of = t.history.len().min(k);
                (n as f64 >= share * of as f64 && n > 0 && mean > min_db).then_some(Strong {
                    index: i,
                    bin: t.bin,
                    mean,
                })
            })
            .collect();
        v.sort_by(|a, b| b.mean.total_cmp(&a.mean));
        v.truncate(max);
        v
    }

    /// The level series of track `i` over the last [`FIT_SPAN`] snapshots (oldest first), `None` where absent.
    pub fn series(&self, i: usize) -> Vec<Option<f64>> {
        let now = self.snapshots();
        let from = now.saturating_sub(FIT_SPAN);
        let t = &self.tracks()[i];
        let len = t.history.len() as u64;
        (from..now)
            .map(|s| {
                let back = t.last_snapshot.checked_sub(s)?;
                let idx = (len - 1).checked_sub(back)?;
                t.history[idx as usize].map(|h| h.level_db)
            })
            .collect()
    }

    /// The tracks other than `skip` within [`MATCH_BINS`] of `bin`, seen recently, present in [`MIN_PRESENT`]
    /// snapshots or more, at least [`MIN_BELOW_DB`] under `ref_db`: `(index, below)`.
    pub fn matches(&self, bin: f64, ref_db: f64, skip: &[usize]) -> Vec<(usize, f64)> {
        self.tracks()
            .iter()
            .enumerate()
            .filter(|(i, t)| {
                !skip.contains(i) && (t.bin - bin).abs() <= MATCH_BINS && t.seen_within(RECENT)
            })
            .filter_map(|(i, t)| {
                let (n, mean) = presence(t);
                (n >= MIN_PRESENT && mean <= ref_db - MIN_BELOW_DB).then_some((i, ref_db - mean))
            })
            .collect()
    }
}

/// The present snapshots of a track and the mean of their levels.
pub fn presence(t: &PeakTrack) -> (usize, f64) {
    let lv: Vec<f64> = t.history.iter().flatten().map(|s| s.level_db).collect();
    let n = lv.len();
    (
        n,
        if n > 0 {
            lv.iter().sum::<f64>() / n as f64
        } else {
            0.0
        },
    )
}

/// The weighted sum `a x + b y` of two series, absent where either is.
pub fn weighted(x: &[Option<f64>], a: f64, y: &[Option<f64>], b: f64) -> Vec<Option<f64>> {
    x.iter()
        .zip(y)
        .map(|(p, q)| Some(a * (*p)? + b * (*q)?))
        .collect()
}

/// `f` folded into `-rate / 2 .. rate / 2`.
pub fn fold(f: f64, rate: f64) -> f64 {
    (f + rate / 2.0).rem_euclid(rate) - rate / 2.0
}

/// The verdicts of findings at the evaluation before and the verdicts reported, keyed by a place.
#[derive(Clone, Debug)]
pub struct HeldVerdicts<K, V> {
    before: Vec<(K, V)>,
    reported: Vec<(K, V)>,
}

impl<K, V> Default for HeldVerdicts<K, V> {
    fn default() -> Self {
        HeldVerdicts {
            before: Vec::new(),
            reported: Vec::new(),
        }
    }
}

impl<K: Copy, V: Copy + PartialEq> HeldVerdicts<K, V> {
    /// Takes the findings `now` of an evaluation, `same` telling whether two places are one. Returns those whose verdict
    /// equals that of the evaluation before and differs from the verdict last reported for the place.
    pub fn update(&mut self, now: &[(K, V)], same: impl Fn(&K, &K) -> bool) -> Vec<(K, V)> {
        let mut rows = Vec::new();
        for (k, v) in now {
            let held = self.before.iter().any(|(b, bv)| same(b, k) && bv == v);
            if !held {
                continue;
            }
            match self.reported.iter_mut().find(|(r, _)| same(r, k)) {
                Some(r) if r.1 == *v => {}
                Some(r) => {
                    *r = (*k, *v);
                    rows.push((*k, *v));
                }
                None => {
                    self.reported.push((*k, *v));
                    rows.push((*k, *v));
                }
            }
        }
        self.before = now.to_vec();
        rows
    }

    /// Forgets the verdicts.
    pub fn clear(&mut self) {
        self.before.clear();
        self.reported.clear();
    }
}
