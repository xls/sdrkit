//! The band survey: per bin the occupancy, the mean and the peak over repeated sweeps, and one row of levels per sweep, with
//! the older rows merged in pairs beyond a cap. It returns persistent signals, quiet slots, a comparison with an older survey
//! and a compact file.

use crate::grid::{Grid, UNCOVERED_DB};
use crate::{db, lin, SweepError};
use sdrkit_core::results::Raster;
use sdrkit_core::{StorageError, StorePath};
use std::io::{self, Read, Write};

const Q_BASE: f32 = -150.0;
const Q_STEP: f32 = 0.6;
const MAGIC: &[u8; 4] = b"SKSV";
const VERSION: u16 = 1;

fn quantise(v: f32) -> u8 {
    if v <= UNCOVERED_DB || !v.is_finite() {
        0
    } else {
        (((v - Q_BASE) / Q_STEP).round()).clamp(1.0, 255.0) as u8
    }
}

fn dequantise(q: u8) -> f32 {
    if q == 0 {
        UNCOVERED_DB
    } else {
        Q_BASE + q as f32 * Q_STEP
    }
}

/// How a survey is built.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyConfig {
    /// A bin is occupied this far over the floor, dB.
    pub threshold_db: f32,
    /// The most rows kept (older rows are merged beyond it).
    pub max_rows: usize,
}

impl Default for SurveyConfig {
    fn default() -> Self {
        SurveyConfig {
            threshold_db: 10.0,
            max_rows: 512,
        }
    }
}

/// One row: one sweep, or several merged.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyRow {
    /// Start, s.
    pub t_start: f64,
    /// End, s.
    pub t_end: f64,
    /// Sweeps in it.
    pub sweeps: u32,
    /// Levels per bin (0 for uncovered, else -150 dBFS + q × 0.6 dB).
    pub q: Vec<u8>,
}

impl SurveyRow {
    /// The level of bin `k`, dBFS.
    pub fn db(&self, k: usize) -> f32 {
        dequantise(self.q[k])
    }
}

/// A signal present most of the time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurveySignal {
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
    /// The occupancy (0–1), the mean over its bins.
    pub occupancy: f64,
    /// Mean level, dBFS.
    pub mean_db: f32,
    /// Peak level, dBFS.
    pub peak_db: f32,
}

/// A quiet slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
    /// The occupancy (0–1).
    pub occupancy: f64,
    /// Mean level, dBFS.
    pub mean_db: f32,
}

sdrkit_core::choice_enum! {
    /// How a range changed between two surveys.
    pub enum ChangeKind {
        /// It is busier now.
        New = ("new", "New"),
        /// It is quieter now.
        Gone = ("gone", "Gone"),
    }
}

/// What changed between two surveys.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurveyChange {
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
    /// Occupancy now minus before.
    pub occupancy_change: f64,
    /// Busier or quieter.
    pub kind: ChangeKind,
}

/// A survey compared with an older one.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyDiff {
    /// The change of the median of the mean levels, dB.
    pub level_change_db: f32,
    /// Ranges whose occupancy rose or fell by 0.3 or more.
    pub changes: Vec<SurveyChange>,
}

/// A band survey.
#[derive(Clone, Debug)]
pub struct BandSurvey {
    grid: Grid,
    cfg: SurveyConfig,
    rows: Vec<SurveyRow>,
    count: Vec<u32>,
    occupied: Vec<u32>,
    sum_lin: Vec<f64>,
    peak: Vec<f32>,
    sweeps: u64,
    spare: Vec<Vec<u8>>,
}

impl BandSurvey {
    /// An empty survey over `grid`.
    pub fn new(grid: Grid, cfg: SurveyConfig) -> Self {
        let n = grid.bins;
        BandSurvey {
            grid,
            cfg,
            rows: Vec::new(),
            count: vec![0; n],
            occupied: vec![0; n],
            sum_lin: vec![0.0; n],
            peak: vec![UNCOVERED_DB; n],
            sweeps: 0,
            spare: Vec::new(),
        }
    }

    #[cfg(test)]
    /// The grid.
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    #[cfg(test)]
    /// The rows, oldest first.
    pub fn rows(&self) -> &[SurveyRow] {
        &self.rows
    }

    /// Sweeps added.
    pub fn sweeps(&self) -> u64 {
        self.sweeps
    }

    /// Adds a sweep taken at `now_s`, with the floor it was measured against.
    pub fn add(&mut self, now_s: f64, sweep_db: &[f32], floor_db: &[f32]) {
        let n = self.grid.bins.min(sweep_db.len());
        let mut q = self.spare.pop().unwrap_or_default();
        q.clear();
        q.resize(self.grid.bins, 0);
        for k in 0..n {
            let x = sweep_db[k];
            if x <= UNCOVERED_DB {
                continue;
            }
            self.count[k] += 1;
            let f = floor_db.get(k).copied().unwrap_or(f32::NAN);
            if f.is_finite() && x > f + self.cfg.threshold_db {
                self.occupied[k] += 1;
            }
            self.sum_lin[k] += lin(x) as f64;
            self.peak[k] = self.peak[k].max(x);
            q[k] = quantise(x);
        }
        self.rows.push(SurveyRow {
            t_start: now_s,
            t_end: now_s,
            sweeps: 1,
            q,
        });
        self.sweeps += 1;
        if self.rows.len() > self.cfg.max_rows.max(2) {
            self.merge_old();
        }
    }

    /// Merges the older half of the rows in pairs. The freed rows are kept for the next sweeps.
    fn merge_old(&mut self) {
        let half = (self.rows.len() / 2) & !1;
        for i in (0..half).step_by(2) {
            let b = std::mem::take(&mut self.rows[i + 1].q);
            let (t_end, wb) = (self.rows[i + 1].t_end, self.rows[i + 1].sweeps);
            let a = &mut self.rows[i];
            let (fa, fb) = (a.sweeps as f32, wb as f32);
            for (x, &y) in a.q.iter_mut().zip(&b) {
                *x = match (*x, y) {
                    (0, y) => y,
                    (x, 0) => x,
                    (x, y) => quantise(db(
                        (lin(dequantise(x)) * fa + lin(dequantise(y)) * fb) / (fa + fb)
                    )),
                };
            }
            a.t_end = t_end;
            a.sweeps += wb;
            self.spare.push(b);
        }
        let mut i = 0;
        self.rows.retain(|_| {
            let keep = i >= half || i % 2 == 0;
            i += 1;
            keep
        });
    }

    /// The occupancy of bin `k` (0–1).
    pub fn occupancy(&self, k: usize) -> f64 {
        self.occupied[k] as f64 / self.count[k].max(1) as f64
    }

    /// The mean level of bin `k` (in power), dBFS.
    pub fn mean_db(&self, k: usize) -> f32 {
        if self.count[k] == 0 {
            UNCOVERED_DB
        } else {
            db((self.sum_lin[k] / self.count[k] as f64) as f32)
        }
    }

    #[cfg(test)]
    /// The peak level of bin `k`, dBFS.
    pub fn peak_db(&self, k: usize) -> f32 {
        self.peak[k]
    }

    /// Puts into `out` the signals occupied at least `min_occupancy` of the time: runs of such bins.
    pub fn persistent(&self, min_occupancy: f64, out: &mut Vec<SurveySignal>) {
        out.clear();
        let n = self.grid.bins;
        let mut k = 0;
        while k < n {
            if self.count[k] > 0 && self.occupancy(k) >= min_occupancy {
                let a = k;
                while k < n && self.count[k] > 0 && self.occupancy(k) >= min_occupancy {
                    k += 1;
                }
                let occ = (a..k).map(|j| self.occupancy(j)).sum::<f64>() / (k - a) as f64;
                let mean = db(((a..k).map(|j| lin(self.mean_db(j)) as f64).sum::<f64>()
                    / (k - a) as f64) as f32);
                let peak = (a..k).map(|j| self.peak[j]).fold(UNCOVERED_DB, f32::max);
                out.push(SurveySignal {
                    low_hz: self.grid.low_hz + a as f64 * self.grid.bin_hz,
                    high_hz: self.grid.low_hz + k as f64 * self.grid.bin_hz,
                    occupancy: occ,
                    mean_db: mean,
                    peak_db: peak,
                });
            } else {
                k += 1;
            }
        }
    }

    /// The `count` quietest slots `width_hz` wide (least occupied, then lowest mean), not overlapping, best first. Slots with an
    /// uncovered bin are skipped.
    pub fn quietest(&self, width_hz: f64, count: usize) -> Vec<Slot> {
        let n = self.grid.bins;
        let w = ((width_hz / self.grid.bin_hz).ceil() as usize).max(1);
        if w > n {
            return Vec::new();
        }
        let mut cands: Vec<(f64, f32, usize)> = Vec::new();
        for a in 0..=n - w {
            if (a..a + w).any(|j| self.count[j] == 0) {
                continue;
            }
            let occ = (a..a + w).map(|j| self.occupancy(j)).sum::<f64>() / w as f64;
            let mean = db(
                ((a..a + w).map(|j| lin(self.mean_db(j)) as f64).sum::<f64>() / w as f64) as f32,
            );
            cands.push((occ, mean, a));
        }
        cands.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let mut taken: Vec<usize> = Vec::new();
        let mut out = Vec::new();
        for (occ, mean, a) in cands {
            if out.len() >= count {
                break;
            }
            if taken.iter().any(|&t| a < t + w && t < a + w) {
                continue;
            }
            taken.push(a);
            out.push(Slot {
                low_hz: self.grid.low_hz + a as f64 * self.grid.bin_hz,
                high_hz: self.grid.low_hz + (a + w) as f64 * self.grid.bin_hz,
                occupancy: occ,
                mean_db: mean,
            });
        }
        out
    }

    /// Compares this survey with an older one (each bin takes the older bin holding its centre).
    pub fn diff(&self, older: &BandSurvey) -> SurveyDiff {
        let n = self.grid.bins;
        let mut dl = Vec::new();
        let mut delta = vec![f64::NAN; n];
        for (k, d) in delta.iter_mut().enumerate() {
            let Some(j) = older.grid.index_of(self.grid.centre_hz(k)) else {
                continue;
            };
            if self.count[k] == 0 || older.count[j] == 0 {
                continue;
            }
            *d = self.occupancy(k) - older.occupancy(j);
            dl.push(self.mean_db(k) - older.mean_db(j));
        }
        let level = if dl.is_empty() {
            0.0
        } else {
            crate::median(&dl, &mut Vec::new())
        };
        let mut changes = Vec::new();
        let mut k = 0;
        while k < n {
            let d = delta[k];
            if d.is_finite() && d.abs() >= 0.3 {
                let up = d > 0.0;
                let a = k;
                while k < n
                    && delta[k].is_finite()
                    && delta[k].abs() >= 0.3
                    && (delta[k] > 0.0) == up
                {
                    k += 1;
                }
                let mean = delta[a..k].iter().sum::<f64>() / (k - a) as f64;
                changes.push(SurveyChange {
                    low_hz: self.grid.low_hz + a as f64 * self.grid.bin_hz,
                    high_hz: self.grid.low_hz + k as f64 * self.grid.bin_hz,
                    occupancy_change: mean,
                    kind: if up {
                        ChangeKind::New
                    } else {
                        ChangeKind::Gone
                    },
                });
            } else {
                k += 1;
            }
        }
        SurveyDiff {
            level_change_db: level,
            changes,
        }
    }

    /// Writes the rows into `r` as a picture of at most `width` columns (each the highest level of the bins it covers, in
    /// dBFS), one row per stored row, oldest first.
    pub fn fill_raster(&self, r: &mut Raster, width: usize) {
        let n = self.grid.bins;
        let w = width.clamp(1, n);
        let h = self.rows.len();
        r.width = w;
        r.height = h;
        r.x.step = self.grid.bin_hz * n as f64 / w as f64 / 1e6;
        r.x.start = self.grid.low_hz / 1e6 + r.x.step / 2.0;
        if let (Some(a), Some(b)) = (self.rows.first(), self.rows.last()) {
            r.y.start = a.t_start;
            r.y.step = if h > 1 {
                (b.t_start - a.t_start) / (h - 1) as f64
            } else {
                0.0
            };
        }
        r.values.clear();
        for row in &self.rows {
            for c in 0..w {
                let a = c * n / w;
                let b = ((c + 1) * n / w).max(a + 1);
                r.values
                    .push((a..b).map(|k| row.db(k)).fold(UNCOVERED_DB, f32::max));
            }
        }
    }

    /// Writes the statistics as CSV: `frequency_hz,occupancy,mean_dbfs,peak_dbfs`.
    pub fn write_csv<W: Write>(&self, w: &mut W) -> io::Result<()> {
        writeln!(w, "frequency_hz,occupancy,mean_dbfs,peak_dbfs")?;
        for k in 0..self.grid.bins {
            if self.count[k] == 0 {
                writeln!(w, "{:.0},,,", self.grid.centre_hz(k))?;
            } else {
                writeln!(
                    w,
                    "{:.0},{:.3},{:.2},{:.2}",
                    self.grid.centre_hz(k),
                    self.occupancy(k),
                    self.mean_db(k),
                    self.peak[k]
                )?;
            }
        }
        Ok(())
    }

    /// Writes the compact file: a header, the per-bin statistics, then the rows at one byte per bin (little-endian throughout).
    pub fn write_to<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(MAGIC)?;
        w.write_all(&VERSION.to_le_bytes())?;
        w.write_all(&self.grid.low_hz.to_le_bytes())?;
        w.write_all(&self.grid.bin_hz.to_le_bytes())?;
        w.write_all(&(self.grid.bins as u32).to_le_bytes())?;
        w.write_all(&self.cfg.threshold_db.to_le_bytes())?;
        w.write_all(&(self.cfg.max_rows as u32).to_le_bytes())?;
        w.write_all(&self.sweeps.to_le_bytes())?;
        for k in 0..self.grid.bins {
            w.write_all(&self.count[k].to_le_bytes())?;
            w.write_all(&self.occupied[k].to_le_bytes())?;
            w.write_all(&self.sum_lin[k].to_le_bytes())?;
            w.write_all(&self.peak[k].to_le_bytes())?;
        }
        w.write_all(&(self.rows.len() as u32).to_le_bytes())?;
        for r in &self.rows {
            w.write_all(&r.t_start.to_le_bytes())?;
            w.write_all(&r.t_end.to_le_bytes())?;
            w.write_all(&r.sweeps.to_le_bytes())?;
            w.write_all(&r.q)?;
        }
        Ok(())
    }

    /// Reads a compact file, the file `path` of the storage.
    ///
    /// # Errors
    ///
    /// [`SweepError::NotASurvey`] if the file is not a survey file of this version, [`SweepError::Storage`] if it does not read.
    pub fn read_from<R: Read>(r: &mut R, path: &StorePath) -> Result<Self, SweepError> {
        let fail = |e: io::Error| match e.kind() {
            io::ErrorKind::UnexpectedEof => SweepError::NotASurvey { path: path.clone() },
            _ => SweepError::Storage {
                path: path.clone(),
                source: StorageError::from(e),
            },
        };
        let not = || SweepError::NotASurvey { path: path.clone() };
        let take = |r: &mut R, n: usize, b: &mut [u8; 8]| r.read_exact(&mut b[..n]).map_err(fail);
        let mut b = [0u8; 8];
        take(r, 4, &mut b)?;
        if &b[..4] != MAGIC {
            return Err(not());
        }
        take(r, 2, &mut b)?;
        if u16::from_le_bytes([b[0], b[1]]) != VERSION {
            return Err(not());
        }
        take(r, 8, &mut b)?;
        let low = f64::from_le_bytes(b);
        take(r, 8, &mut b)?;
        let bin = f64::from_le_bytes(b);
        take(r, 4, &mut b)?;
        let bins = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        if bins == 0 || bins > crate::grid::MAX_BINS || !(bin.is_finite() && bin > 0.0) {
            return Err(not());
        }
        take(r, 4, &mut b)?;
        let threshold_db = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        take(r, 4, &mut b)?;
        let max_rows = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        take(r, 8, &mut b)?;
        let sweeps = u64::from_le_bytes(b);
        let grid = Grid {
            low_hz: low,
            bin_hz: bin,
            bins,
        };
        let mut s = BandSurvey::new(
            grid,
            SurveyConfig {
                threshold_db,
                max_rows,
            },
        );
        s.sweeps = sweeps;
        for k in 0..bins {
            take(r, 4, &mut b)?;
            s.count[k] = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            take(r, 4, &mut b)?;
            s.occupied[k] = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            take(r, 8, &mut b)?;
            s.sum_lin[k] = f64::from_le_bytes(b);
            take(r, 4, &mut b)?;
            s.peak[k] = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        }
        take(r, 4, &mut b)?;
        let rows = u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize;
        for _ in 0..rows.min(max_rows.max(2) + 1) {
            take(r, 8, &mut b)?;
            let t_start = f64::from_le_bytes(b);
            take(r, 8, &mut b)?;
            let t_end = f64::from_le_bytes(b);
            take(r, 4, &mut b)?;
            let sw = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let mut q = vec![0u8; bins];
            r.read_exact(&mut q).map_err(fail)?;
            s.rows.push(SurveyRow {
                t_start,
                t_end,
                sweeps: sw,
                q,
            });
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep(t: usize) -> Vec<f32> {
        let mut v = vec![-90.0f32; 100];
        // Always on at bins 20-22, on half the time at 60, never elsewhere.
        for x in &mut v[20..23] {
            *x = -40.0;
        }
        if t.is_multiple_of(2) {
            v[60] = -50.0;
        }
        // Bins 80-89 a little louder than the rest of the quiet ones.
        for x in &mut v[80..90] {
            *x = -88.0;
        }
        v
    }

    fn survey(n: usize) -> BandSurvey {
        let g = Grid::new(0.0, 100e3, 1e3).unwrap();
        let mut s = BandSurvey::new(
            g,
            SurveyConfig {
                threshold_db: 10.0,
                max_rows: 8,
            },
        );
        let floor = vec![-90.0f32; 100];
        for t in 0..n {
            s.add(t as f64, &sweep(t), &floor);
        }
        s
    }

    #[test]
    fn statistics_and_lists() {
        let s = survey(10);
        assert_eq!(s.occupancy(21), 1.0);
        assert_eq!(s.occupancy(60), 0.5);
        assert_eq!(s.occupancy(5), 0.0);
        assert!((s.mean_db(60) - db((lin(-50.0) + lin(-90.0)) / 2.0)).abs() < 1e-3);
        assert_eq!(s.peak_db(60), -50.0);
        let mut p = Vec::new();
        s.persistent(0.8, &mut p);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].low_hz, p[0].high_hz), (20e3, 23e3));
        let q = s.quietest(5e3, 3);
        assert_eq!(q.len(), 3);
        assert!(q.iter().all(|sl| sl.occupancy == 0.0 && sl.mean_db < -89.0));
        assert!(q.iter().all(|sl| sl.high_hz <= 20e3 || sl.low_hz >= 23e3));
        assert!(q.iter().all(|sl| sl.high_hz <= 80e3 || sl.low_hz >= 90e3));
    }

    #[test]
    fn rows_are_capped_by_merging() {
        let s = survey(40);
        assert!(s.rows().len() <= 8);
        assert_eq!(s.rows().iter().map(|r| r.sweeps).sum::<u32>(), 40);
        assert_eq!(s.rows().last().unwrap().sweeps, 1);
        assert!(s.rows()[0].sweeps > 1);
        assert!((s.rows()[0].db(21) + 40.0).abs() < 0.4);
    }

    #[test]
    fn diff_picture_csv_and_file() {
        let older = survey(10);
        let g = *older.grid();
        let mut newer = BandSurvey::new(g, SurveyConfig::default());
        let floor = vec![-90.0f32; 100];
        for t in 0..10 {
            let mut v = sweep(t);
            for x in &mut v[20..23] {
                *x = -90.0; // gone
            }
            v[40] = -30.0; // new
            newer.add(t as f64, &v, &floor);
        }
        let d = newer.diff(&older);
        assert!(d
            .changes
            .iter()
            .any(|c| c.kind == ChangeKind::New && c.low_hz == 40e3));
        assert!(d
            .changes
            .iter()
            .any(|c| c.kind == ChangeKind::Gone && c.low_hz == 20e3 && c.high_hz == 23e3));
        let mut pic = Raster::default();
        older.fill_raster(&mut pic, 50);
        assert_eq!((pic.width, pic.height), (50, older.rows().len()));
        assert_eq!(pic.values.len(), 50 * older.rows().len());
        let top = pic.values[..50]
            .iter()
            .copied()
            .fold(UNCOVERED_DB, f32::max);
        assert_eq!(pic.values[10], top); // bins 20-21: the strongest
        let mut csv = Vec::new();
        older.write_csv(&mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        assert_eq!(text.lines().count(), 101);
        assert_eq!(text.lines().nth(22).unwrap(), "21500,1.000,-40.00,-40.00");
        let mut bin = Vec::new();
        older.write_to(&mut bin).unwrap();
        assert!(bin.len() < 100 * 20 + 8 * (100 + 20) + 64);
        let path = StorePath::new("a.survey").unwrap();
        let back = BandSurvey::read_from(&mut bin.as_slice(), &path).unwrap();
        assert_eq!(back.rows(), older.rows());
        assert_eq!(back.occupancy(60), 0.5);
        assert_eq!(back.sweeps(), 10);
        assert_eq!(
            BandSurvey::read_from(&mut &b"nope"[..], &path).err(),
            Some(SweepError::NotASurvey { path })
        );
    }
}
