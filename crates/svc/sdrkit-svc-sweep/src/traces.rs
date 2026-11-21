//! The pseudo-spectrum of the whole range as result series: the current sweep, the average, a max hold that decays, the
//! floor and the threshold, each reduced to at most a number of points.

use crate::grid::{Grid, UNCOVERED_DB};
use crate::{db, lin};
use sdrkit_core::results::Series;
use sdrkit_core::{Results, SeriesId};

/// The keys of the series, in the order of [`Traces::publish`].
pub const SERIES: [(&str, &str); 5] = [
    ("sweep.current", "Current"),
    ("sweep.average", "Average"),
    ("sweep.max", "Max hold"),
    ("sweep.floor", "Noise floor"),
    ("sweep.threshold", "Threshold"),
];

/// How the traces behave.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceConfig {
    /// Sweeps averaged, exponentially. 1 for none.
    pub averaging: usize,
    /// How fast the max hold falls, dB per sweep (0: never).
    pub max_decay_db: f32,
    /// The most points per series.
    pub points: usize,
}

impl Default for TraceConfig {
    fn default() -> Self {
        TraceConfig {
            averaging: 8,
            max_decay_db: 0.5,
            points: 1024,
        }
    }
}

/// The traces of a sweep.
#[derive(Debug)]
pub struct Traces {
    grid: Grid,
    cfg: TraceConfig,
    current: Vec<f32>,
    avg_lin: Vec<f32>,
    average: Vec<f32>,
    max: Vec<f32>,
    sweeps: u64,
}

impl Traces {
    /// Empty traces over `grid`.
    pub fn new(grid: Grid, cfg: TraceConfig) -> Self {
        let n = grid.bins;
        Traces {
            grid,
            cfg,
            current: vec![UNCOVERED_DB; n],
            avg_lin: vec![0.0; n],
            average: vec![UNCOVERED_DB; n],
            max: vec![UNCOVERED_DB; n],
            sweeps: 0,
        }
    }

    /// Takes a stitched sweep.
    pub fn update(&mut self, sweep_db: &[f32]) {
        self.sweeps += 1;
        let k = self.sweeps.min(self.cfg.averaging.max(1) as u64) as f32;
        for (i, &x) in sweep_db.iter().enumerate().take(self.grid.bins) {
            self.current[i] = x;
            if x <= UNCOVERED_DB {
                continue;
            }
            let p = lin(x);
            if self.average[i] <= UNCOVERED_DB {
                self.avg_lin[i] = p;
            } else {
                self.avg_lin[i] += (p - self.avg_lin[i]) / k;
            }
            self.average[i] = db(self.avg_lin[i]);
            let decayed = self.max[i] - self.cfg.max_decay_db;
            self.max[i] = if self.max[i] <= UNCOVERED_DB {
                x
            } else {
                decayed.max(x)
            };
        }
    }

    /// Resets the max hold to the current sweep.
    pub fn reset_max(&mut self) {
        self.max.copy_from_slice(&self.current);
    }

    /// The current sweep, dBFS per bin.
    pub fn current(&self) -> &[f32] {
        &self.current
    }

    /// The average, dBFS per bin.
    pub fn average(&self) -> &[f32] {
        &self.average
    }

    /// The decaying max hold, dBFS per bin.
    pub fn max(&self) -> &[f32] {
        &self.max
    }

    /// Writes the series `ids` (declared in the order of [`SERIES`]) into `results`, with the floor and the threshold over it.
    pub fn publish(
        &self,
        results: &mut Results,
        ids: &[SeriesId; 5],
        floor_db: &[f32],
        threshold_db: f32,
    ) {
        let pts = self.cfg.points.max(2);
        let g = &self.grid;
        let traces: [(&[f32], bool, f32); 5] = [
            (&self.current, false, 0.0),
            (&self.average, false, 0.0),
            (&self.max, false, 0.0),
            (floor_db, true, 0.0),
            (floor_db, true, threshold_db),
        ];
        for (&id, (v, take_min, add)) in ids.iter().zip(traces) {
            results.update_series(id, |s| {
                write_series(s, g, v, pts, take_min, add);
                true
            });
        }
    }
}

/// Writes `v` reduced to at most `points` points into `s`, plus `add` dB.
fn write_series(s: &mut Series, grid: &Grid, v: &[f32], points: usize, take_min: bool, add: f32) {
    s.line = true;
    s.points.clear();
    let n = v.len().min(grid.bins);
    if n == 0 {
        return;
    }
    let m = points.min(n);
    for j in 0..m {
        let a = j * n / m;
        let b = ((j + 1) * n / m).max(a + 1);
        let mut val = if take_min {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        };
        for &x in &v[a..b] {
            if !x.is_finite() || x <= UNCOVERED_DB {
                continue;
            }
            val = if take_min { val.min(x) } else { val.max(x) };
        }
        let val = if val.is_finite() {
            val + add
        } else {
            UNCOVERED_DB
        };
        let mhz = (grid.low_hz + (a + b) as f64 / 2.0 * grid.bin_hz) / 1e6;
        s.points.push([mhz as f32, val]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::ResultsDecl;
    use std::sync::Arc;

    fn results() -> (Results, [SeriesId; 5]) {
        let mut d = ResultsDecl::new();
        for (key, label) in SERIES {
            d = d.series(key, label, "MHz", "dBFS");
        }
        let d = Arc::new(d);
        let ids = SERIES.map(|(key, _)| d.series_id(key).unwrap());
        (Results::new(d), ids)
    }

    #[test]
    fn average_in_power_max_decays_and_resets() {
        let g = Grid::new(0.0, 4e3, 1e3).unwrap();
        let mut t = Traces::new(
            g,
            TraceConfig {
                averaging: 2,
                max_decay_db: 1.0,
                points: 4,
            },
        );
        t.update(&[-10.0, -20.0, UNCOVERED_DB, -40.0]);
        t.update(&[-20.0, -20.0, UNCOVERED_DB, -40.0]);
        // Mean of 0.1 and 0.01 = 0.055 -> -12.6 dB.
        assert!((t.average()[0] + 12.596).abs() < 0.01);
        assert_eq!(t.max()[0], -11.0);
        assert_eq!(t.average()[2], UNCOVERED_DB);
        t.reset_max();
        assert_eq!(t.max()[0], -20.0);
        let (mut r, ids) = results();
        let floor = vec![-50.0; 4];
        t.publish(&mut r, &ids, &floor, 10.0);
        let thr = &r.series(ids[4]).unwrap().points;
        assert_eq!(thr.len(), 4);
        assert_eq!(thr[0][1], -40.0);
        let cur = &r.series(ids[0]).unwrap().points;
        assert_eq!(cur[2][1], UNCOVERED_DB);
        assert!((cur[0][0] - 0.0005).abs() < 1e-6);
    }

    #[test]
    fn reduction_keeps_narrow_peaks() {
        let g = Grid::new(0.0, 10_000.0, 1.0).unwrap();
        let mut t = Traces::new(g, TraceConfig::default());
        let mut v = vec![-90.0; 10_000];
        v[5_001] = -20.0;
        t.update(&v);
        let (mut r, ids) = results();
        t.publish(&mut r, &ids, &vec![-90.0; 10_000], 10.0);
        let c = &r.series(ids[0]).unwrap().points;
        assert_eq!(c.len(), 1024);
        assert!(c.iter().any(|p| p[1] == -20.0));
    }
}
