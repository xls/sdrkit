//! Ridge search over a dispersion law in a spectrogram.
//!
//! A dispersed signal reaches frequency `f` at `t(f) = t_pivot + D (g(f) - g(f_pivot))`: `g` is the law (the delay per unit of
//! dispersion `D`) and `t_pivot` the time at the pivot frequency. [`eckersley`] is the law of whistlers (`g = 1/sqrt(f)`, `D` in
//! s sqrt(Hz)); [`cold_plasma`] is the law of pulsar pulses (`g = 1/f^2`, `D` in s Hz^2).
//!
//! [`RidgeSearch`] holds, for each dispersion of a grid, the row offset of each path point relative to the pivot row:
//! `round(D (g(f) - g(f_pivot)) / row_s)`. It scores a path through a spectrogram held as rows of dB values (oldest row first,
//! `width` columns a row) as the mean of the values clamped to `[low, high]`, counts the points above `hit_above`, and keeps only
//! paths whose share of such points is at least `min_share`. [`RidgeSearch::best`] returns the best path over every dispersion
//! and every pivot row whose whole path lies inside the rows given.
//!
//! ```
//! use sdrkit_algo_dsp::dispersion::{eckersley, geometric_grid, RidgeSearch, Scoring};
//! // 40 rows of 8 columns, 1 dB everywhere, with a ridge of 12 dB along the path of D = 1 through pivot row 20.
//! let points: Vec<(f64, usize)> = (0..8).map(|c| (1000.0 + 500.0 * c as f64, c)).collect();
//! let search = RidgeSearch::new(eckersley, &points, 2000.0, &geometric_grid(0.5, 2.0, 2.0).unwrap(), 0.001).unwrap();
//! let mut cells = vec![1.0f32; 40 * 8];
//! for (k, &(f, c)) in points.iter().enumerate() {
//!     let row = (20.0 + (1.0 / f.sqrt() - 1.0 / 2000f64.sqrt()) / 0.001).round() as usize;
//!     cells[row * 8 + c] = 12.0;
//! }
//! let s = Scoring { low: 0.0, high: 15.0, hit_above: 4.0, min_share: 0.75 };
//! let r = search.best(&cells, 8, 40, &s).unwrap();
//! assert_eq!((r.dispersion, r.pivot_row, r.hits), (1.0, 20, 8));
//! ```

use crate::{DspError, Param};

/// Eckersley's law of whistlers: the delay per unit of dispersion at `f` Hz, `1 / sqrt(f)`.
pub fn eckersley(f: f64) -> f64 {
    1.0 / f.sqrt()
}

/// The cold-plasma dispersion constant, s MHz² per pc cm⁻³: a pulse of dispersion measure `DM` arrives at `f_lo` (MHz)
/// `COLD_PLASMA_S_MHZ2 x DM x (1 / f_lo^2 - 1 / f_hi^2)` seconds after `f_hi`. Document: Lorimer and Kramer, Handbook of
/// Pulsar Astronomy.
pub const COLD_PLASMA_S_MHZ2: f64 = 4.148808e3;

/// The cold-plasma law of pulsar pulses: the delay per unit of dispersion at `f` Hz, `1 / f^2`.
pub fn cold_plasma(f: f64) -> f64 {
    1.0 / (f * f)
}

/// The geometric grid `start, start * factor, start * factor^2, ...` while a value is at most `end` (with a relative margin of
/// 1e-9 for rounding).
///
/// # Errors
///
/// [`DspError`] unless `start` is above 0, `factor` above 1 and `end` at least `start`.
pub fn geometric_grid(start: f64, factor: f64, end: f64) -> Result<Vec<f64>, DspError> {
    if !(start > 0.0 && start.is_finite()) {
        return Param::Start.positive(start);
    }
    if !(factor > 1.0 && factor.is_finite()) {
        return Param::GridStep.range(1.0, f64::INFINITY, factor);
    }
    if !(end >= start && end.is_finite()) {
        return Param::Span.range(start, f64::INFINITY, end);
    }
    let mut v = Vec::new();
    let mut d = start;
    while d <= end * (1.0 + 1e-9) {
        v.push(d);
        d *= factor;
    }
    Ok(v)
}

/// How a path is scored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scoring {
    /// Values are clamped to `[low, high]` before the mean.
    pub low: f32,
    /// See `low`.
    pub high: f32,
    /// A point counts as a hit when its value is above this.
    pub hit_above: f32,
    /// The least share of hits (0 to 1) of a path that counts.
    pub min_share: f64,
}

/// The best path of a search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ridge {
    /// The dispersion of the path.
    pub dispersion: f64,
    /// The index in the grid of the dispersion.
    pub index: usize,
    /// The row of the path at the pivot frequency (it can lie outside the rows when the pivot is outside the path).
    pub pivot_row: i64,
    /// The mean of the clamped values along the path.
    pub score: f64,
    /// The points above the hit level.
    pub hits: usize,
    /// The points of the path.
    pub points: usize,
}

impl Ridge {
    /// The share of hits, 0 to 1.
    pub fn share(&self) -> f64 {
        self.hits as f64 / self.points.max(1) as f64
    }
}

/// The path of one dispersion: the row offset of each point and the extremes of the offsets.
#[derive(Clone, Debug)]
struct Path {
    dispersion: f64,
    offsets: Vec<i64>,
    first: i64,
    last: i64,
}

/// The path search over a grid of dispersions (module docs).
#[derive(Clone, Debug)]
pub struct RidgeSearch {
    columns: Vec<usize>,
    paths: Vec<Path>,
}

impl RidgeSearch {
    /// The search for the points `(frequency, column)` of a path, the pivot frequency `pivot_hz`, the dispersions `grid` and rows
    /// `row_s` seconds apart. `law(f)` is the delay per unit of dispersion at `f`.
    ///
    /// # Errors
    ///
    /// [`DspError`] when there are no points or no dispersions, `row_s` is not above 0, or the law or a dispersion is not finite.
    pub fn new(
        law: impl Fn(f64) -> f64,
        points: &[(f64, usize)],
        pivot_hz: f64,
        grid: &[f64],
        row_s: f64,
    ) -> Result<Self, DspError> {
        if points.is_empty() {
            return Param::Points.length(1, 0);
        }
        if grid.is_empty() {
            return Param::Values.length(1, 0);
        }
        if !(row_s > 0.0 && row_s.is_finite()) {
            return Param::Tau.positive(row_s);
        }
        let g0 = law(pivot_hz);
        if !g0.is_finite() {
            return Param::Frequency.range(0.0, f64::INFINITY, pivot_hz);
        }
        let delays: Vec<f64> = points.iter().map(|&(f, _)| law(f) - g0).collect();
        if let Some(k) = delays.iter().position(|d| !d.is_finite()) {
            return Param::Frequency.range(0.0, f64::INFINITY, points[k].0);
        }
        let mut paths = Vec::with_capacity(grid.len());
        for &d in grid {
            if !d.is_finite() {
                return Param::Values.range(f64::MIN, f64::MAX, d);
            }
            let offsets: Vec<i64> = delays
                .iter()
                .map(|&g| (d * g / row_s).round() as i64)
                .collect();
            let first = offsets.iter().copied().min().unwrap_or(0);
            let last = offsets.iter().copied().max().unwrap_or(0);
            paths.push(Path {
                dispersion: d,
                offsets,
                first,
                last,
            });
        }
        Ok(RidgeSearch {
            columns: points.iter().map(|p| p.1).collect(),
            paths,
        })
    }

    /// The points of a path.
    pub fn points(&self) -> usize {
        self.columns.len()
    }

    /// The dispersions of the grid.
    pub fn dispersions(&self) -> impl Iterator<Item = f64> + '_ {
        self.paths.iter().map(|p| p.dispersion)
    }

    /// The row offsets of the earliest and the latest point of the path of grid index `i`, relative to the pivot row. `None`
    /// for an index outside the grid.
    pub fn span(&self, i: usize) -> Option<(i64, i64)> {
        self.paths.get(i).map(|p| (p.first, p.last))
    }

    /// The score and the hits of the path of grid index `i` through pivot row `pivot` in `cells` (rows of `width` columns,
    /// oldest first), counting only the rows before `end_row`. `None` when a point of the path lies outside those rows, a
    /// column lies outside a row, or `i` is outside the grid. The least share of `s` is not applied.
    pub fn score_at(
        &self,
        cells: &[f32],
        width: usize,
        end_row: usize,
        i: usize,
        pivot: i64,
        s: &Scoring,
    ) -> Option<(f64, usize)> {
        let rows = (cells.len() / width.max(1)).min(end_row) as i64;
        let p = self.paths.get(i)?;
        if width == 0 || self.columns.iter().any(|&c| c >= width) {
            return None;
        }
        if pivot + p.first < 0 || pivot + p.last >= rows {
            return None;
        }
        let w = width as i64;
        let (sum, hits) =
            p.offsets
                .iter()
                .zip(&self.columns)
                .fold((0f64, 0usize), |(sum, hits), (&o, &c)| {
                    let v = cells[((pivot + o) * w) as usize + c];
                    (
                        sum + f64::from(v.clamp(s.low, s.high)),
                        hits + usize::from(v > s.hit_above),
                    )
                });
        Some((sum / self.columns.len() as f64, hits))
    }

    /// The best path in `cells` (rows of `width` columns, oldest first) over every dispersion of the grid and every pivot row
    /// whose whole path lies in the rows before `end_row`. A path counts when its share of hits is at least `s.min_share`; the
    /// highest score wins, and on equal scores the smaller dispersion, then the earlier pivot row. `None` when no path counts or
    /// a column lies outside a row.
    pub fn best(&self, cells: &[f32], width: usize, end_row: usize, s: &Scoring) -> Option<Ridge> {
        if width == 0 || self.columns.iter().any(|&c| c >= width) {
            return None;
        }
        let w = width as i64;
        let rows = (cells.len() / width).min(end_row) as i64;
        let n = self.columns.len();
        let need = (s.min_share * n as f64 - 1e-9).ceil().max(0.0) as usize;
        let mut best: Option<Ridge> = None;
        for (i, p) in self.paths.iter().enumerate() {
            // Pivot rows with the whole path inside `0..rows`.
            let lo = -p.first;
            let hi = rows - 1 - p.last;
            let mut pivot = lo;
            while pivot <= hi {
                let mut sum = 0f64;
                let mut hits = 0usize;
                let mut complete = true;
                for (k, (&o, &c)) in p.offsets.iter().zip(&self.columns).enumerate() {
                    let v = cells[((pivot + o) * w) as usize + c];
                    sum += f64::from(v.clamp(s.low, s.high));
                    hits += usize::from(v > s.hit_above);
                    // Stop when the remaining points cannot reach the hits needed.
                    if hits + (n - 1 - k) < need {
                        complete = false;
                        break;
                    }
                }
                if complete && hits >= need {
                    let score = sum / n as f64;
                    if best.is_none_or(|b| score > b.score) {
                        best = Some(Ridge {
                            dispersion: p.dispersion,
                            index: i,
                            pivot_row: pivot,
                            score,
                            hits,
                            points: n,
                        });
                    }
                }
                pivot += 1;
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: Scoring = Scoring {
        low: 0.0,
        high: 15.0,
        hit_above: 4.0,
        min_share: 0.75,
    };

    /// `(row_s, rows, width)`: rows `row_s` seconds apart, how many, and their columns.
    type Layout = (f64, usize, usize);

    /// Rows of the layout, each cell 0 dB, with `v` dB along the path `t(f) = t_pivot + d (law(f) - law(pivot_hz))`.
    fn draw(
        law: fn(f64) -> f64,
        points: &[(f64, usize)],
        pivot_hz: f64,
        d: f64,
        pivot_row: i64,
        (row_s, rows, width): Layout,
        v: f32,
    ) -> Vec<f32> {
        let mut cells = vec![0f32; rows * width];
        for &(f, c) in points {
            let r = pivot_row + (d * (law(f) - law(pivot_hz)) / row_s).round() as i64;
            if (0..rows as i64).contains(&r) {
                cells[r as usize * width + c] = v;
            }
        }
        cells
    }

    #[test]
    fn grid_is_geometric_and_ends_at_the_limit() {
        let g = geometric_grid(10.0, 1.07, 300.0).unwrap();
        assert_eq!(g.len(), 51);
        assert!((g[1] - 10.7).abs() < 1e-12);
        assert!(*g.last().unwrap() <= 300.0);
        assert_eq!(geometric_grid(2.0, 2.0, 8.0).unwrap(), vec![2.0, 4.0, 8.0]);
        assert!(geometric_grid(0.0, 1.07, 300.0).is_err());
        assert!(geometric_grid(10.0, 1.0, 300.0).is_err());
        assert!(geometric_grid(10.0, 1.07, 5.0).is_err());
    }

    #[test]
    fn whistler_path_is_found_with_its_dispersion() {
        // 46.875 Hz columns, rows of 5.333 ms, points every 3 columns from 1500 Hz to 9000 Hz, pivot 4 kHz.
        let bin = 46.875;
        let row_s = 128.0 / 24_000.0;
        let points: Vec<(f64, usize)> =
            (32..=192).step_by(3).map(|c| (c as f64 * bin, c)).collect();
        let grid = geometric_grid(10.0, 1.07, 300.0).unwrap();
        let s = RidgeSearch::new(eckersley, &points, 4000.0, &grid, row_s).unwrap();
        let d = grid[26];
        let cells = draw(eckersley, &points, 4000.0, d, 300, (row_s, 750, 257), 12.0);
        let r = s.best(&cells, 257, 750, &S).unwrap();
        assert_eq!((r.dispersion, r.pivot_row), (d, 300));
        assert_eq!(r.hits, points.len());
        assert!((r.share() - 1.0).abs() < 1e-12);
        assert!((r.score - 12.0).abs() < 1e-9);
        assert_eq!(
            s.score_at(&cells, 257, 750, 26, 300, &S),
            Some((12.0, points.len()))
        );
    }

    #[test]
    fn pulsar_law_is_searched_the_same_way() {
        // Channels of 1 MHz from 400 to 463 MHz, rows of 1 ms, pivot at the top channel: the lower channels arrive later.
        let points: Vec<(f64, usize)> = (0..64).map(|c| ((400 + c) as f64 * 1e6, c)).collect();
        let grid: Vec<f64> = (1..=40).map(|k| k as f64 * 1e15).collect();
        let s = RidgeSearch::new(cold_plasma, &points, 463e6, &grid, 1e-3).unwrap();
        let d = grid[24];
        let cells = draw(cold_plasma, &points, 463e6, d, 100, (1e-3, 400, 64), 9.0);
        let r = s.best(&cells, 64, 400, &S).unwrap();
        assert_eq!((r.dispersion, r.pivot_row, r.hits), (d, 100, 64));
        // 25e15 (1/400e6^2 - 1/463e6^2) = 39.6 ms.
        assert_eq!(s.span(24), Some((0, 40)));
    }

    #[test]
    fn a_path_that_leaves_the_rows_or_lacks_hits_does_not_count() {
        let points: Vec<(f64, usize)> = (0..10).map(|c| (1000.0 + 300.0 * c as f64, c)).collect();
        let s = RidgeSearch::new(eckersley, &points, 2000.0, &[2.0], 0.001).unwrap();
        let (first, last) = s.span(0).unwrap();
        let pivot = -first + 2;
        let cells = draw(
            eckersley,
            &points,
            2000.0,
            2.0,
            pivot,
            (0.001, 200, 10),
            12.0,
        );
        // The rows end before the last point of the path.
        let end = (pivot + last) as usize;
        assert!(s.best(&cells, 10, end, &S).is_none());
        assert!(s.best(&cells, 10, end + 1, &S).is_some());
        // Seven of ten points: below the share of 0.75.
        let mut part = cells.clone();
        for &(f, c) in points.iter().take(3) {
            let r = pivot + (2.0 * (eckersley(f) - eckersley(2000.0)) / 0.001).round() as i64;
            part[r as usize * 10 + c] = 0.0;
        }
        assert!(s.best(&part, 10, 200, &S).is_none());
        // A column outside the rows.
        assert!(s.best(&cells[..9 * 20], 9, 20, &S).is_none());
    }

    #[test]
    fn equal_scores_keep_the_smaller_dispersion_and_the_earlier_row() {
        let points: Vec<(f64, usize)> = (0..4).map(|c| (1000.0 * (c + 1) as f64, c)).collect();
        let s = RidgeSearch::new(eckersley, &points, 2000.0, &[1.0, 2.0], 0.01).unwrap();
        // Every cell equal: every path of every dispersion scores the same.
        let cells = vec![6.0f32; 100 * 4];
        let r = s.best(&cells, 4, 100, &S).unwrap();
        assert_eq!(r.index, 0);
        assert_eq!(r.pivot_row, -s.span(0).unwrap().0);
    }

    #[test]
    fn bad_arguments_are_refused() {
        assert!(RidgeSearch::new(eckersley, &[], 4000.0, &[1.0], 0.01).is_err());
        assert!(RidgeSearch::new(eckersley, &[(1000.0, 0)], 4000.0, &[], 0.01).is_err());
        assert!(RidgeSearch::new(eckersley, &[(1000.0, 0)], 4000.0, &[1.0], 0.0).is_err());
        assert!(RidgeSearch::new(eckersley, &[(0.0, 0)], 4000.0, &[1.0], 0.01).is_err());
        assert!(RidgeSearch::new(eckersley, &[(1000.0, 0)], 4000.0, &[f64::NAN], 0.01).is_err());
    }
}
