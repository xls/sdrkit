//! Two detectors on levels and spectra: the burst monitor reports a level that rises over the median of the quiet levels for a
//! time, and the drift finder follows narrow carriers whose frequency moves steadily.

use sdrkit_algo_dsp::peak::parabolic;
use std::collections::VecDeque;

/// A burst.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    /// Start, s.
    pub start_s: f64,
    /// Duration, s.
    pub duration_s: f64,
    /// The highest level, dB.
    pub peak_db: f32,
    /// The quiet median when it started, dB.
    pub base_db: f32,
    /// Levels in it.
    pub samples: u32,
}

/// Counts bursts on one level.
#[derive(Clone, Debug)]
pub struct BurstMonitor {
    /// The rise over the median that makes a burst, dB.
    pub rise_db: f32,
    /// The shortest burst reported, s.
    pub min_s: f64,
    quiet: VecDeque<f32>,
    qbuf: Vec<f32>,
    scratch: Vec<f32>,
    open: Option<Burst>,
    log: VecDeque<Burst>,
}

const QUIET: usize = 15;
const LOG_CAP: usize = 500;

impl Default for BurstMonitor {
    fn default() -> Self {
        Self::new(6.0, 0.2)
    }
}

impl BurstMonitor {
    /// A monitor.
    pub fn new(rise_db: f32, min_s: f64) -> Self {
        BurstMonitor {
            rise_db,
            min_s,
            quiet: VecDeque::with_capacity(QUIET + 1),
            qbuf: Vec::with_capacity(QUIET + 1),
            scratch: Vec::with_capacity(QUIET + 1),
            open: None,
            log: VecDeque::new(),
        }
    }

    #[cfg(test)]
    /// The bursts reported, oldest first (at most 500).
    pub fn log(&self) -> &VecDeque<Burst> {
        &self.log
    }

    /// Clears the log.
    pub fn clear(&mut self) {
        self.log.clear();
    }

    /// The quiet median, dB (NaN until there are levels).
    pub fn median_db(&mut self) -> f32 {
        self.qbuf.clear();
        self.qbuf.extend(self.quiet.iter().copied());
        crate::median(&self.qbuf, &mut self.scratch)
    }

    /// A level at `now_s`. Returns a burst when one ends.
    pub fn update(&mut self, now_s: f64, level_db: f32) -> Option<Burst> {
        if self.quiet.len() < 3 {
            self.quiet.push_back(level_db);
            return None;
        }
        let m = self.median_db();
        if let Some(b) = &mut self.open {
            if level_db >= b.base_db + self.rise_db {
                b.peak_db = b.peak_db.max(level_db);
                b.samples += 1;
                return None;
            }
            let mut done = *b;
            self.open = None;
            done.duration_s = now_s - done.start_s;
            self.push_quiet(level_db);
            if done.duration_s + 1e-9 >= self.min_s {
                if self.log.len() >= LOG_CAP {
                    self.log.pop_front();
                }
                self.log.push_back(done);
                return Some(done);
            }
            return None;
        }
        if level_db >= m + self.rise_db {
            self.open = Some(Burst {
                start_s: now_s,
                duration_s: 0.0,
                peak_db: level_db,
                base_db: m,
                samples: 1,
            });
        } else {
            self.push_quiet(level_db);
        }
        None
    }

    fn push_quiet(&mut self, v: f32) {
        self.quiet.push_back(v);
        while self.quiet.len() > QUIET {
            self.quiet.pop_front();
        }
    }
}

/// How drifting carriers are found.
#[derive(Clone, Debug, PartialEq)]
pub struct DriftConfig {
    /// Height over the frame median, dB.
    pub rise_db: f32,
    /// Widest at half the rise, bins.
    pub max_width_bins: usize,
    /// Fastest drift, Hz per second.
    pub max_drift_hz_s: f64,
    /// Longest gap followed, s.
    pub max_gap_s: f64,
    /// Shortest track kept, s.
    pub min_duration_s: f64,
    /// Fewest frames.
    pub min_frames: u32,
    /// Least movement, Hz.
    pub min_move_hz: f64,
}

impl Default for DriftConfig {
    fn default() -> Self {
        DriftConfig {
            rise_db: 8.0,
            max_width_bins: 6,
            max_drift_hz_s: 400.0,
            max_gap_s: 1.0,
            min_duration_s: 3.0,
            min_frames: 5,
            min_move_hz: 300.0,
        }
    }
}

/// A drifting carrier found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DriftTrack {
    /// Its first frequency, Hz.
    pub first_hz: f64,
    /// Its last frequency, Hz.
    pub last_hz: f64,
    /// Drift, Hz per second (least squares).
    pub drift_hz_s: f64,
    /// First seen, s.
    pub first_s: f64,
    /// Last seen, s.
    pub last_s: f64,
    /// Peak level, dB.
    pub peak_db: f32,
    /// Frames it was seen in.
    pub hits: u32,
}

impl DriftTrack {
    /// How long it lasted, s.
    pub fn duration_s(&self) -> f64 {
        self.last_s - self.first_s
    }
}

#[derive(Clone, Debug)]
struct Open {
    t: DriftTrack,
    // Least-squares sums, time relative to first_s.
    n: f64,
    st: f64,
    sf: f64,
    stt: f64,
    stf: f64,
}

impl Open {
    fn slope(&self) -> f64 {
        let d = self.n * self.stt - self.st * self.st;
        if self.n < 2.0 || d.abs() < 1e-12 {
            0.0
        } else {
            (self.n * self.stf - self.st * self.sf) / d
        }
    }
    fn add(&mut self, t: f64, f: f64, level: f32) {
        let x = t - self.t.first_s;
        let y = f - self.t.first_hz;
        self.n += 1.0;
        self.st += x;
        self.sf += y;
        self.stt += x * x;
        self.stf += x * y;
        self.t.last_s = t;
        self.t.last_hz = f;
        self.t.peak_db = self.t.peak_db.max(level);
        self.t.hits += 1;
        self.t.drift_hz_s = self.slope();
    }
}

/// Finds narrow carriers that drift steadily in a stream of spectra.
#[derive(Clone, Debug, Default)]
pub struct DriftFinder {
    cfg: DriftConfig,
    open: Vec<Open>,
    found: Vec<DriftTrack>,
    scratch: Vec<f32>,
    peaks: Vec<(f64, f32)>,
    used: Vec<bool>,
}

impl DriftFinder {
    /// A finder.
    pub fn new(cfg: DriftConfig) -> Self {
        DriftFinder {
            cfg,
            ..Default::default()
        }
    }

    /// The tracks kept, oldest first.
    pub fn found(&self) -> &[DriftTrack] {
        &self.found
    }

    /// A spectrum frame: `bins_db` from `low_hz` in steps of `bin_hz`.
    pub fn update(&mut self, now_s: f64, low_hz: f64, bin_hz: f64, bins_db: &[f32]) {
        let n = bins_db.len();
        if n < 3 {
            return;
        }
        let med = crate::median(bins_db, &mut self.scratch);
        let rise = self.cfg.rise_db;
        self.peaks.clear();
        for k in 1..n - 1 {
            let v = bins_db[k];
            if v < med + rise || v < bins_db[k - 1] || v < bins_db[k + 1] {
                continue;
            }
            let half = med + (v - med) / 2.0;
            let mut a = k;
            while a > 0 && bins_db[a - 1] > half {
                a -= 1;
            }
            let mut b = k;
            while b + 1 < n && bins_db[b + 1] > half {
                b += 1;
            }
            if b - a + 1 > self.cfg.max_width_bins {
                continue;
            }
            let d = parabolic(
                f64::from(bins_db[k - 1]),
                f64::from(v),
                f64::from(bins_db[k + 1]),
            )
            .offset;
            self.peaks.push((low_hz + (k as f64 + 0.5 + d) * bin_hz, v));
        }
        let used = &mut self.used;
        used.clear();
        used.resize(self.peaks.len(), false);
        for o in &mut self.open {
            let dt = now_s - o.t.last_s;
            let pred = o.t.last_hz + o.slope() * dt;
            let tol = self.cfg.max_drift_hz_s * dt.max(0.0) + 2.0 * bin_hz;
            let best = self
                .peaks
                .iter()
                .enumerate()
                .filter(|(i, p)| !used[*i] && (p.0 - pred).abs() <= tol)
                .min_by(|a, b| (a.1 .0 - pred).abs().total_cmp(&(b.1 .0 - pred).abs()))
                .map(|(i, _)| i);
            if let Some(i) = best {
                used[i] = true;
                let (f, l) = self.peaks[i];
                o.add(now_s, f, l);
            }
        }
        for (i, &(f, l)) in self.peaks.iter().enumerate() {
            if !used[i] {
                let mut o = Open {
                    t: DriftTrack {
                        first_hz: f,
                        last_hz: f,
                        drift_hz_s: 0.0,
                        first_s: now_s,
                        last_s: now_s,
                        peak_db: l,
                        hits: 0,
                    },
                    n: 0.0,
                    st: 0.0,
                    sf: 0.0,
                    stt: 0.0,
                    stf: 0.0,
                };
                o.add(now_s, f, l);
                self.open.push(o);
            }
        }
        self.close(now_s);
    }

    fn close(&mut self, now_s: f64) {
        let cfg = &self.cfg;
        let mut i = 0;
        while i < self.open.len() {
            if now_s - self.open[i].t.last_s > cfg.max_gap_s {
                let o = self.open.swap_remove(i);
                let t = o.t;
                if t.duration_s() >= cfg.min_duration_s
                    && t.hits >= cfg.min_frames
                    && (t.last_hz - t.first_hz).abs() >= cfg.min_move_hz
                    && t.drift_hz_s.abs() <= cfg.max_drift_hz_s
                {
                    if self.found.len() >= 500 {
                        self.found.remove(0);
                    }
                    self.found.push(t);
                }
            } else {
                i += 1;
            }
        }
    }

    /// Ends every open track (at the end of a recording).
    pub fn flush(&mut self) {
        let far =
            self.open.iter().map(|o| o.t.last_s).fold(0.0f64, f64::max) + self.cfg.max_gap_s + 1.0;
        self.close(far);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_of_200_ms_count_shorter_do_not() {
        let mut m = BurstMonitor::default();
        let mut t = 0.0;
        let mut found = Vec::new();
        let mut feed = |m: &mut BurstMonitor, t: &mut f64, level: f32, n: usize| {
            for _ in 0..n {
                if let Some(b) = m.update(*t, level) {
                    found.push(b);
                }
                *t += 0.05;
            }
        };
        feed(&mut m, &mut t, -80.0, 20);
        feed(&mut m, &mut t, -73.0, 4); // 200 ms at +7 dB
        feed(&mut m, &mut t, -80.0, 10);
        feed(&mut m, &mut t, -73.0, 2); // 100 ms
        feed(&mut m, &mut t, -80.0, 10);
        feed(&mut m, &mut t, -77.0, 10); // +3 dB: not a burst
        feed(&mut m, &mut t, -80.0, 5);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!((found[0].duration_s - 0.2).abs() < 1e-6);
        assert_eq!(found[0].base_db, -80.0);
        assert_eq!(found[0].peak_db, -73.0);
        assert_eq!(found[0].samples, 4);
        assert!((found[0].start_s - 1.0).abs() < 1e-6);
        assert_eq!(m.log().len(), 1);
    }

    #[test]
    fn a_drifting_carrier_is_found_a_still_one_is_not() {
        let mut f = DriftFinder::new(DriftConfig::default());
        let bin = 50.0;
        let low = 0.0;
        let mut t = 0.0;
        while t < 6.0 {
            let mut v = vec![-90.0f32; 400];
            // Moving: 5000 Hz + 100 Hz/s. Still: 15000 Hz.
            for (hz, lvl) in [(5000.0 + 100.0 * t, -60.0f32), (15025.0, -60.0)] {
                let k = ((hz - low) / bin) as usize;
                v[k] = lvl;
            }
            f.update(t, low, bin, &v);
            t += 0.25;
        }
        f.flush();
        assert_eq!(f.found().len(), 1, "{:?}", f.found());
        let d = f.found()[0];
        assert!((d.drift_hz_s - 100.0).abs() < 15.0, "{}", d.drift_hz_s);
        assert!((d.first_hz - 5000.0).abs() < 60.0);
        assert!(d.duration_s() >= 5.0);
        assert!(d.hits >= 20);
    }
}
