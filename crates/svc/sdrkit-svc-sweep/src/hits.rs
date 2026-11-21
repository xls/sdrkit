//! Hits: runs of bins over the floor with hysteresis in level, and tracks that follow them from sweep to sweep with a
//! minimum duration and a hang time. A signal that comes back to an ended track keeps its row and counts again.

use crate::grid::{Grid, UNCOVERED_DB};
use crate::{db, lin};

/// How hits are found and followed.
#[derive(Clone, Debug, PartialEq)]
pub struct HitConfig {
    /// A run starts at a bin this far over the floor, dB.
    pub open_db: f32,
    /// A run goes on while bins stay this far over the floor, dB (the hysteresis: below `open_db`).
    pub close_db: f32,
    /// Runs this many bins apart or closer are joined.
    pub merge_gap_bins: usize,
    /// A track opens after being present this long, s (0: at once).
    pub min_duration_s: f64,
    /// An open track stays open while missed for less than this, s.
    pub hang_s: f64,
    /// A detection belongs to a track this close, Hz (0: two bins).
    pub match_hz: f64,
    /// The most tracks kept (the oldest ended ones go first).
    pub max_tracks: usize,
    /// Ended tracks are forgotten after this long, s.
    pub forget_s: f64,
}

impl Default for HitConfig {
    fn default() -> Self {
        HitConfig {
            open_db: 10.0,
            close_db: 6.0,
            merge_gap_bins: 1,
            min_duration_s: 0.0,
            hang_s: 2.0,
            match_hz: 0.0,
            max_tracks: 1024,
            forget_s: 24.0 * 3600.0,
        }
    }
}

/// A signal found in one sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    /// The first bin.
    pub start: usize,
    /// One past the last bin.
    pub end: usize,
    /// The power-weighted centre, Hz.
    pub centre_hz: f64,
    /// The width: the bins within 6 dB of the peak, Hz.
    pub width_hz: f64,
    /// The strongest bin's centre, Hz.
    pub peak_hz: f64,
    /// The strongest bin's level, dBFS.
    pub peak_db: f32,
    /// The floor under the peak, dBFS.
    pub floor_db: f32,
    /// `peak_db - floor_db`.
    pub snr_db: f32,
    /// The peak over the mean of the run, dB (small for a flat-topped digital signal).
    pub peak_to_mean_db: f32,
}

/// Finds the detections of a sweep. `mask` (when not empty) marks bins never to use (lockouts, exclusions).
pub fn detect(
    grid: &Grid,
    sweep_db: &[f32],
    floor_db: &[f32],
    mask: &[bool],
    cfg: &HitConfig,
    runs: &mut Vec<(usize, usize)>,
    out: &mut Vec<Detection>,
) {
    out.clear();
    runs.clear();
    let n = grid.bins.min(sweep_db.len()).min(floor_db.len());
    let usable = |k: usize| {
        sweep_db[k] > UNCOVERED_DB
            && floor_db[k].is_finite()
            && !mask.get(k).copied().unwrap_or(false)
    };
    let over = |k: usize, t: f32| usable(k) && sweep_db[k] > floor_db[k] + t;
    let mut k = 0;
    while k < n {
        if over(k, cfg.open_db) {
            let mut a = k;
            while a > 0 && over(a - 1, cfg.close_db) {
                a -= 1;
            }
            let mut b = k + 1;
            while b < n && over(b, cfg.close_db) {
                b += 1;
            }
            match runs.last_mut() {
                Some(last) if a <= last.1 + cfg.merge_gap_bins => last.1 = last.1.max(b),
                _ => runs.push((a, b)),
            }
            k = b;
        } else {
            k += 1;
        }
    }
    for &(a, b) in runs.iter() {
        let mut peak = a;
        let mut wsum = 0.0f64;
        let mut fsum = 0.0f64;
        let mut psum = 0.0f64;
        let mut cnt = 0usize;
        for j in a..b {
            if !usable(j) {
                continue;
            }
            if sweep_db[j] > sweep_db[peak] || !usable(peak) {
                peak = j;
            }
            let p = lin(sweep_db[j]) as f64;
            let above = (p - lin(floor_db[j]) as f64).max(0.0);
            wsum += above;
            fsum += above * grid.centre_hz(j);
            psum += p;
            cnt += 1;
        }
        if cnt == 0 {
            continue;
        }
        let centre = if wsum > 0.0 {
            fsum / wsum
        } else {
            grid.centre_hz(peak)
        };
        let mean = db((psum / cnt as f64) as f32);
        // The width is the bins within 6 dB of the peak (the run itself also holds the window's skirts).
        let top = sweep_db[peak] - 6.0;
        let wide = (a..b)
            .filter(|&j| usable(j) && sweep_db[j] >= top)
            .count()
            .max(1);
        out.push(Detection {
            start: a,
            end: b,
            centre_hz: centre,
            width_hz: wide as f64 * grid.bin_hz,
            peak_hz: grid.centre_hz(peak),
            peak_db: sweep_db[peak],
            floor_db: floor_db[peak],
            snr_db: sweep_db[peak] - floor_db[peak],
            peak_to_mean_db: sweep_db[peak] - mean,
        });
    }
}

/// Where a track is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackState {
    /// Seen, not yet for the minimum duration.
    Pending,
    /// Open: present now.
    Active,
    /// Open but missed, within the hang time.
    Hanging,
    /// Ended (it keeps its row and count).
    Ended,
}

/// A signal followed across sweeps.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// A stable id.
    pub id: u64,
    /// The centre, Hz (smoothed).
    pub centre_hz: f64,
    /// The width, Hz (smoothed).
    pub width_hz: f64,
    /// The last peak level, dBFS.
    pub peak_db: f32,
    /// The best SNR seen, dB.
    pub best_snr_db: f32,
    /// The last SNR, dB.
    pub snr_db: f32,
    /// The last peak over mean, dB.
    pub peak_to_mean_db: f32,
    /// First seen, s.
    pub first_seen_s: f64,
    /// Last seen, s.
    pub last_seen_s: f64,
    /// The start of the current (or last) opening, s.
    pub opened_s: f64,
    /// Sweeps it was seen in.
    pub sweeps_seen: u64,
    /// Sweeps since it was first seen (seen or not).
    pub sweeps_total: u64,
    /// Times it opened (a returning signal counts again).
    pub count: u32,
    /// The state.
    pub state: TrackState,
    /// It has been open at least once.
    pub was_open: bool,
}

impl Track {
    /// The share of sweeps since it was first seen in which it was present.
    pub fn share_present(&self) -> f64 {
        self.sweeps_seen as f64 / self.sweeps_total.max(1) as f64
    }
    /// True while open (active or hanging).
    pub fn is_open(&self) -> bool {
        matches!(self.state, TrackState::Active | TrackState::Hanging)
    }
}

/// Something that happened to a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitEvent {
    /// A new signal opened.
    Started {
        /// The track.
        id: u64,
    },
    /// A known signal opened again.
    Returned {
        /// The track.
        id: u64,
        /// Its count now.
        count: u32,
    },
    /// A signal ended.
    Ended {
        /// The track.
        id: u64,
        /// How long it was open, s.
        length_s: f64,
    },
}

/// Follows detections across sweeps.
#[derive(Debug, Default)]
pub struct HitTracker {
    cfg: HitConfig,
    tracks: Vec<Track>,
    next_id: u64,
    events: Vec<HitEvent>,
    used: Vec<bool>,
    order: Vec<usize>,
    drop_ids: Vec<u64>,
}

impl HitTracker {
    /// A tracker.
    pub fn new(cfg: HitConfig) -> Self {
        HitTracker {
            cfg,
            next_id: 1,
            ..Default::default()
        }
    }

    /// The settings.
    pub fn config(&self) -> &HitConfig {
        &self.cfg
    }

    /// The tracks.
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// A track by id.
    pub fn track(&self, id: u64) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Removes a track.
    pub fn remove(&mut self, id: u64) -> bool {
        let n = self.tracks.len();
        self.tracks.retain(|t| t.id != id);
        n != self.tracks.len()
    }

    /// Removes the tracks inside `lo_hz ..= hi_hz`.
    pub fn remove_between(&mut self, lo_hz: f64, hi_hz: f64) {
        self.tracks
            .retain(|t| t.centre_hz < lo_hz || t.centre_hz > hi_hz);
    }

    /// Takes a sweep's detections at time `now_s`, with the grid's bin width. Returns what happened.
    pub fn update(&mut self, now_s: f64, detections: &[Detection], bin_hz: f64) -> &[HitEvent] {
        self.events.clear();
        let cfg = &self.cfg;
        let tol = if cfg.match_hz > 0.0 {
            cfg.match_hz
        } else {
            2.0 * bin_hz
        };
        self.used.clear();
        self.used.resize(self.tracks.len(), false);
        self.order.clear();
        self.order.extend(0..detections.len());
        self.order.sort_unstable_by(|&a, &b| {
            detections[b]
                .snr_db
                .total_cmp(&detections[a].snr_db)
                .then(a.cmp(&b))
        });
        for &di in &self.order {
            let d = &detections[di];
            let mut best: Option<(usize, f64)> = None;
            for (ti, t) in self.tracks.iter().enumerate() {
                if self.used[ti] {
                    continue;
                }
                let dist = (t.centre_hz - d.centre_hz).abs();
                let allowed = tol.max(t.width_hz.min(d.width_hz) / 2.0);
                let better = match best {
                    Some((_, bd)) => dist < bd,
                    None => true,
                };
                if dist <= allowed && better {
                    best = Some((ti, dist));
                }
            }
            match best {
                Some((ti, _)) => {
                    self.used[ti] = true;
                    let t = &mut self.tracks[ti];
                    t.centre_hz += 0.3 * (d.centre_hz - t.centre_hz);
                    t.width_hz += 0.3 * (d.width_hz - t.width_hz);
                    t.peak_db = d.peak_db;
                    t.snr_db = d.snr_db;
                    t.best_snr_db = t.best_snr_db.max(d.snr_db);
                    t.peak_to_mean_db = d.peak_to_mean_db;
                    t.sweeps_seen += 1;
                    t.sweeps_total += 1;
                    if t.state == TrackState::Ended {
                        t.state = TrackState::Pending;
                        t.opened_s = now_s;
                    }
                    if t.state == TrackState::Hanging {
                        t.state = TrackState::Active;
                    }
                    t.last_seen_s = now_s;
                    if t.state == TrackState::Pending && now_s - t.opened_s >= cfg.min_duration_s {
                        t.state = TrackState::Active;
                        t.count += 1;
                        self.events.push(if t.was_open {
                            HitEvent::Returned {
                                id: t.id,
                                count: t.count,
                            }
                        } else {
                            HitEvent::Started { id: t.id }
                        });
                        t.was_open = true;
                    }
                }
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    let mut t = Track {
                        id,
                        centre_hz: d.centre_hz,
                        width_hz: d.width_hz,
                        peak_db: d.peak_db,
                        best_snr_db: d.snr_db,
                        snr_db: d.snr_db,
                        peak_to_mean_db: d.peak_to_mean_db,
                        first_seen_s: now_s,
                        last_seen_s: now_s,
                        opened_s: now_s,
                        sweeps_seen: 1,
                        sweeps_total: 1,
                        count: 0,
                        state: TrackState::Pending,
                        was_open: false,
                    };
                    if cfg.min_duration_s <= 0.0 {
                        t.state = TrackState::Active;
                        t.count = 1;
                        t.was_open = true;
                        self.events.push(HitEvent::Started { id });
                    }
                    self.tracks.push(t);
                    self.used.push(true);
                }
            }
        }
        let hang = cfg.hang_s;
        let drop_ids = &mut self.drop_ids;
        drop_ids.clear();
        for (ti, t) in self.tracks.iter_mut().enumerate() {
            if self.used[ti] {
                continue;
            }
            t.sweeps_total += 1;
            match t.state {
                TrackState::Active | TrackState::Hanging => {
                    if now_s - t.last_seen_s >= hang {
                        t.state = TrackState::Ended;
                        self.events.push(HitEvent::Ended {
                            id: t.id,
                            length_s: t.last_seen_s - t.opened_s,
                        });
                    } else {
                        t.state = TrackState::Hanging;
                    }
                }
                TrackState::Pending => {
                    if now_s - t.last_seen_s >= hang {
                        if t.was_open {
                            t.state = TrackState::Ended;
                        } else {
                            // A blip shorter than the minimum duration: never reported.
                            drop_ids.push(t.id);
                        }
                    }
                }
                TrackState::Ended => {
                    if now_s - t.last_seen_s >= self.cfg.forget_s {
                        drop_ids.push(t.id);
                    }
                }
            }
        }
        if !self.drop_ids.is_empty() {
            let drop_ids = &self.drop_ids;
            self.tracks.retain(|t| !drop_ids.contains(&t.id));
        }
        while self.tracks.len() > self.cfg.max_tracks {
            let victim = self
                .tracks
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let ka = (a.1.is_open(), a.1.last_seen_s);
                    let kb = (b.1.is_open(), b.1.last_seen_s);
                    ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
                })
                .map(|(i, _)| i);
            match victim {
                Some(i) => {
                    self.tracks.remove(i);
                }
                None => break,
            }
        }
        &self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep(levels: &[(usize, f32)]) -> Vec<f32> {
        let mut v = vec![-90.0f32; 200];
        for &(k, l) in levels {
            v[k] = l;
        }
        v
    }

    #[test]
    fn runs_with_hysteresis_and_merging() {
        let g = Grid::new(0.0, 200e3, 1e3).unwrap();
        let floor = vec![-90.0f32; 200];
        let cfg = HitConfig::default();
        let mut out = Vec::new();
        // 50 opens (+20), 49 and 51-52 continue (+8 > close 6), 53 is +4: stops.
        let s = sweep(&[
            (49, -82.0),
            (50, -70.0),
            (51, -82.0),
            (52, -82.0),
            (53, -86.0),
        ]);
        detect(&g, &s, &floor, &[], &cfg, &mut Vec::new(), &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].start, out[0].end), (49, 53));
        assert_eq!(out[0].peak_hz, 50.5e3);
        assert!((out[0].snr_db - 20.0).abs() < 1e-3);
        // A bin at +8 alone never opens.
        detect(
            &g,
            &sweep(&[(10, -82.0)]),
            &floor,
            &[],
            &cfg,
            &mut Vec::new(),
            &mut out,
        );
        assert!(out.is_empty());
        // Two runs one bin apart merge.
        let s = sweep(&[(100, -70.0), (102, -70.0)]);
        detect(&g, &s, &floor, &[], &cfg, &mut Vec::new(), &mut out);
        assert_eq!(out.len(), 1);
        // Masked bins are ignored.
        let mut mask = vec![false; 200];
        mask[100] = true;
        mask[102] = true;
        detect(&g, &s, &floor, &mask, &cfg, &mut Vec::new(), &mut out);
        assert!(out.is_empty());
    }

    fn det(hz: f64) -> Detection {
        Detection {
            start: 0,
            end: 1,
            centre_hz: hz,
            width_hz: 1e3,
            peak_hz: hz,
            peak_db: -60.0,
            floor_db: -90.0,
            snr_db: 30.0,
            peak_to_mean_db: 0.0,
        }
    }

    #[test]
    fn minimum_duration_hang_and_return() {
        let cfg = HitConfig {
            min_duration_s: 1.0,
            hang_s: 2.0,
            ..Default::default()
        };
        let mut t = HitTracker::new(cfg);
        // A blip: seen once, never opens, dropped after the hang.
        assert!(t.update(0.0, &[det(5e3)], 1e3).is_empty());
        assert!(t.update(3.0, &[], 1e3).is_empty());
        assert!(t.tracks().is_empty());
        // A signal present for 1.5 s opens.
        assert!(t.update(10.0, &[det(50e3)], 1e3).is_empty());
        assert!(t.update(10.5, &[det(50.2e3)], 1e3).is_empty());
        let e = t.update(11.0, &[det(50e3)], 1e3).to_vec();
        let id = match e[..] {
            [HitEvent::Started { id }] => id,
            _ => panic!("{e:?}"),
        };
        // Missed briefly: hangs, still open, no event.
        assert!(t.update(12.0, &[], 1e3).is_empty());
        assert_eq!(t.track(id).unwrap().state, TrackState::Hanging);
        assert!(t.update(12.5, &[det(50e3)], 1e3).is_empty());
        // Gone for longer than the hang: ends with its length.
        t.update(13.0, &[], 1e3);
        let e = t.update(15.0, &[], 1e3).to_vec();
        assert_eq!(e, vec![HitEvent::Ended { id, length_s: 2.5 }]);
        // It returns: the same row, count 2.
        t.update(20.0, &[det(50e3)], 1e3);
        let e = t.update(21.0, &[det(50e3)], 1e3).to_vec();
        assert_eq!(e, vec![HitEvent::Returned { id, count: 2 }]);
        assert_eq!(t.tracks().len(), 1);
        let tr = t.track(id).unwrap();
        assert!(tr.share_present() < 1.0 && tr.share_present() > 0.5);
    }

    #[test]
    fn tracks_are_capped() {
        let cfg = HitConfig {
            max_tracks: 3,
            ..Default::default()
        };
        let mut t = HitTracker::new(cfg);
        let dets: Vec<Detection> = (0..6).map(|i| det(i as f64 * 100e3)).collect();
        t.update(0.0, &dets, 1e3);
        assert_eq!(t.tracks().len(), 3);
    }
}
