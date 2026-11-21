//! Many narrow spectral lines followed through time. Each update brings the peaks of one spectrum frame; every active track,
//! the strongest first, takes the nearest unused peak to its predicted frequency inside a gate, and every peak left over
//! starts a new track. The prediction is the least-squares line through the last points of the track. A track that is not
//! seen for a while ends; it is kept when it lasted long enough with enough points.
//!
//! ```
//! use sdrkit_algo_dsp::linetrack::{LinePeak, LineTracker, TrackParams};
//! let mut t = LineTracker::new(TrackParams::new(10.0, 3.0, 2.0));
//! for k in 0..40 {
//!     let s = k as f64 * 0.25;
//!     t.update(s, &[LinePeak { hz: 1000.0 + 2.0 * s, level_db: 20.0 }]);
//! }
//! assert_eq!(t.active().len(), 1);
//! assert!((t.active()[0].drift_hz_s(16).unwrap() - 2.0).abs() < 1e-9);
//! ```

/// A peak of one spectrum frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinePeak {
    /// Its frequency, Hz.
    pub hz: f64,
    /// Its level, dB (any zero point; higher is stronger).
    pub level_db: f64,
}

/// One point of a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinePoint {
    /// The time of the frame, s.
    pub t: f64,
    /// The frequency, Hz.
    pub hz: f64,
    /// The level, dB.
    pub level_db: f64,
}

/// The rules of a [`LineTracker`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackParams {
    /// The fastest change of frequency a line may have, Hz/s.
    pub max_slope_hz_s: f64,
    /// The least half-width of the gate, Hz.
    pub gate_min_hz: f64,
    /// The share of `max_slope_hz_s` × the time since the last point that widens the gate.
    pub gate_slope_share: f64,
    /// Added to the widened gate, Hz.
    pub gate_extra_hz: f64,
    /// The most recent points the prediction fits.
    pub fit_points: usize,
    /// The least points for a fitted prediction; with fewer, the last frequency is the prediction.
    pub fit_min_points: usize,
    /// A track not seen for this long ends, s.
    pub end_after_s: f64,
    /// An ended track is kept when it lasted this long, s ...
    pub keep_min_s: f64,
    /// ... and has this many points.
    pub keep_min_points: usize,
    /// The most active tracks; a peak starts no track beyond it.
    pub max_active: usize,
    /// The most points of a track. A full track drops every second point and goes on.
    pub max_points: usize,
}

impl TrackParams {
    /// The rules with a slope limit, a least gate and a gate extra (all Hz or Hz/s), the gate widened by a quarter of the slope
    /// limit times the gap, a fit over the last 12 points when there are 4, an end after 5 s unseen, a track kept from 5 s and
    /// 8 points, at most 64 active tracks of 4000 points.
    pub const fn new(max_slope_hz_s: f64, gate_min_hz: f64, gate_extra_hz: f64) -> Self {
        TrackParams {
            max_slope_hz_s,
            gate_min_hz,
            gate_slope_share: 0.25,
            gate_extra_hz,
            fit_points: 12,
            fit_min_points: 4,
            end_after_s: 5.0,
            keep_min_s: 5.0,
            keep_min_points: 8,
            max_active: 64,
            max_points: 4000,
        }
    }
}

/// One followed line.
#[derive(Clone, Debug, PartialEq)]
pub struct LineTrack {
    id: u64,
    points: Vec<LinePoint>,
}

impl LineTrack {
    /// Its number: tracks are numbered from 1 in the order they start.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Its points, oldest first (never empty).
    pub fn points(&self) -> &[LinePoint] {
        &self.points
    }

    /// The first point.
    pub fn first(&self) -> LinePoint {
        self.points[0]
    }

    /// The last point.
    pub fn last(&self) -> LinePoint {
        self.points[self.points.len() - 1]
    }

    /// The time from the first to the last point, s.
    pub fn duration_s(&self) -> f64 {
        self.last().t - self.first().t
    }

    /// The least-squares slope over the last `n` points, Hz/s. `None` with fewer than two points or no spread in time.
    pub fn drift_hz_s(&self, n: usize) -> Option<f64> {
        let k = self.points.len().saturating_sub(n.max(2));
        fit(&self.points[k..]).map(|(slope, _)| slope)
    }

    /// The frequency predicted at `t`: the line through the last `p.fit_points` points when there are `p.fit_min_points`,
    /// else the last frequency.
    pub fn predict(&self, t: f64, p: &TrackParams) -> f64 {
        let last = self.last();
        if self.points.len() >= p.fit_min_points.max(2) {
            let k = self.points.len().saturating_sub(p.fit_points.max(2));
            if let Some((slope, at_last)) = fit(&self.points[k..]) {
                return at_last + slope * (t - last.t);
            }
        }
        last.hz
    }

    fn push(&mut self, pt: LinePoint, max_points: usize) {
        if self.points.len() >= max_points.max(2) {
            let last = self.last();
            let mut i = 0;
            self.points.retain(|_| {
                i += 1;
                i % 2 == 1
            });
            if self.last() != last {
                self.points.push(last);
            }
        }
        self.points.push(pt);
    }
}

/// The least-squares line through `pts`: (slope Hz/s, value at the last point's time). `None` with fewer than two points or
/// all at one time.
fn fit(pts: &[LinePoint]) -> Option<(f64, f64)> {
    if pts.len() < 2 {
        return None;
    }
    let t0 = pts[pts.len() - 1].t;
    let n = pts.len() as f64;
    let mt = pts.iter().map(|p| p.t - t0).sum::<f64>() / n;
    let mf = pts.iter().map(|p| p.hz).sum::<f64>() / n;
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for p in pts {
        let dx = p.t - t0 - mt;
        sxx += dx * dx;
        sxy += dx * (p.hz - mf);
    }
    if sxx <= 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    Some((slope, mf - slope * mt))
}

/// The tracker (module documentation).
#[derive(Clone, Debug)]
pub struct LineTracker {
    p: TrackParams,
    active: Vec<LineTrack>,
    finished: Vec<LineTrack>,
    next_id: u64,
    used: Vec<bool>,
    order: Vec<usize>,
}

impl LineTracker {
    /// A tracker with no track.
    pub fn new(p: TrackParams) -> Self {
        LineTracker {
            p,
            active: Vec::new(),
            finished: Vec::new(),
            next_id: 1,
            used: Vec::new(),
            order: Vec::new(),
        }
    }

    /// The rules.
    pub fn params(&self) -> &TrackParams {
        &self.p
    }

    /// Changes the rules from the next update on; the tracks stay.
    pub fn set_params(&mut self, p: TrackParams) {
        self.p = p;
    }

    /// The active tracks, in the order they started.
    pub fn active(&self) -> &[LineTrack] {
        &self.active
    }

    /// The tracks that ended and were kept since the last call, in the order they ended.
    pub fn take_finished(&mut self) -> Vec<LineTrack> {
        std::mem::take(&mut self.finished)
    }

    /// Forgets every track; numbering goes on.
    pub fn clear(&mut self) {
        self.active.clear();
        self.finished.clear();
    }

    /// Takes the peaks of the frame at time `t` (s, not earlier than the last frame): extends the tracks, ends the stale ones
    /// and starts new ones.
    pub fn update(&mut self, t: f64, peaks: &[LinePeak]) {
        let p = self.p;
        self.used.clear();
        self.used.resize(peaks.len(), false);
        // The strongest track picks first.
        self.order.clear();
        self.order.extend(0..self.active.len());
        let active = &self.active;
        self.order.sort_by(|&a, &b| {
            active[b]
                .last()
                .level_db
                .total_cmp(&active[a].last().level_db)
                .then(a.cmp(&b))
        });
        for &k in &self.order {
            let track = &self.active[k];
            let last = track.last();
            let want = track.predict(t, &p);
            let gate = p.gate_min_hz.max(
                p.gate_slope_share * p.max_slope_hz_s * (t - last.t).max(0.0) + p.gate_extra_hz,
            );
            let mut best: Option<(usize, f64)> = None;
            for (i, pk) in peaks.iter().enumerate() {
                if self.used[i] {
                    continue;
                }
                let d = (pk.hz - want).abs();
                if d <= gate && best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
            if let Some((i, _)) = best {
                self.used[i] = true;
                let pt = LinePoint {
                    t,
                    hz: peaks[i].hz,
                    level_db: peaks[i].level_db,
                };
                self.active[k].push(pt, p.max_points);
            }
        }
        // Stale tracks end; the long ones are kept.
        let mut i = 0;
        while i < self.active.len() {
            if t - self.active[i].last().t >= p.end_after_s {
                let tr = self.active.remove(i);
                if tr.duration_s() >= p.keep_min_s && tr.points.len() >= p.keep_min_points {
                    self.finished.push(tr);
                }
            } else {
                i += 1;
            }
        }
        // Every unused peak starts a track, the strongest first.
        self.order.clear();
        self.order
            .extend((0..peaks.len()).filter(|&i| !self.used[i]));
        self.order.sort_by(|&a, &b| {
            peaks[b]
                .level_db
                .total_cmp(&peaks[a].level_db)
                .then(a.cmp(&b))
        });
        for &i in &self.order {
            if self.active.len() >= p.max_active {
                break;
            }
            self.active.push(LineTrack {
                id: self.next_id,
                points: vec![LinePoint {
                    t,
                    hz: peaks[i].hz,
                    level_db: peaks[i].level_db,
                }],
            });
            self.next_id += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pk(hz: f64, level_db: f64) -> LinePeak {
        LinePeak { hz, level_db }
    }

    #[test]
    fn two_drifting_lines_keep_their_tracks() {
        let mut tr = LineTracker::new(TrackParams::new(50.0, 3.0, 2.0));
        for k in 0..200 {
            let t = k as f64 * 0.25;
            tr.update(t, &[pk(500.0 + 10.0 * t, 20.0), pk(900.0 - 5.0 * t, 15.0)]);
        }
        assert_eq!(tr.active().len(), 2);
        let a = &tr.active()[0];
        let b = &tr.active()[1];
        assert_eq!(a.points().len(), 200);
        assert!((a.drift_hz_s(16).unwrap() - 10.0).abs() < 1e-9);
        assert!((b.drift_hz_s(16).unwrap() + 5.0).abs() < 1e-9);
        assert!((a.predict(50.0, tr.params()) - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn a_peak_outside_the_gate_starts_another_track() {
        let mut tr = LineTracker::new(TrackParams::new(4.0, 3.0, 2.0));
        tr.update(0.0, &[pk(100.0, 10.0)]);
        // Gate: max(3, 0.25 × 4 × 1 + 2) = 3 Hz.
        tr.update(1.0, &[pk(103.5, 10.0)]);
        assert_eq!(tr.active().len(), 2);
        tr.update(2.0, &[pk(105.0, 10.0)]);
        assert_eq!(tr.active()[1].points().len(), 2);
    }

    #[test]
    fn stale_tracks_end_and_long_ones_are_kept() {
        let mut tr = LineTracker::new(TrackParams::new(10.0, 3.0, 2.0));
        for k in 0..30 {
            let t = k as f64 * 0.25;
            let mut peaks = vec![pk(1000.0, 20.0)];
            if k < 4 {
                peaks.push(pk(3000.0, 20.0));
            }
            tr.update(t, &peaks);
        }
        // The short line ended after 5 s unseen and was not kept; the long one is active.
        assert_eq!(tr.active().len(), 1);
        assert!(tr.take_finished().is_empty());
        for k in 30..60 {
            tr.update(k as f64 * 0.25, &[]);
        }
        let done = tr.take_finished();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].points().len(), 30);
        assert!(tr.active().is_empty());
    }

    #[test]
    fn the_strongest_track_picks_first() {
        let mut tr = LineTracker::new(TrackParams::new(10.0, 3.0, 2.0));
        tr.update(0.0, &[pk(100.0, 10.0), pk(104.0, 30.0)]);
        // One peak between both: the stronger track (at 104 Hz) takes it.
        tr.update(0.25, &[pk(102.5, 25.0)]);
        let strong = tr.active().iter().find(|t| t.first().hz == 104.0).unwrap();
        assert_eq!(strong.points().len(), 2);
    }

    #[test]
    fn at_most_the_active_limit_and_full_tracks_are_thinned() {
        let mut p = TrackParams::new(10.0, 3.0, 2.0);
        p.max_active = 3;
        p.max_points = 10;
        let mut tr = LineTracker::new(p);
        let peaks: Vec<LinePeak> = (0..5).map(|i| pk(1000.0 * i as f64, i as f64)).collect();
        for k in 0..25 {
            tr.update(k as f64, &peaks);
        }
        assert_eq!(tr.active().len(), 3);
        // The strongest peaks started the tracks.
        assert!(tr.active().iter().all(|t| t.first().hz >= 2000.0));
        let t = &tr.active()[0];
        assert!(t.points().len() <= 10);
        assert_eq!(t.first().t, 0.0);
        assert_eq!(t.last().t, 24.0);
    }
}
