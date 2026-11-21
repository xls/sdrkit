//! Small dense least squares in `f64`: the Cholesky solver for a symmetric positive definite system, and a least-squares fit that
//! adds rows one at a time and solves the normal equations with an optional ridge.

use crate::{DspError, Param};

/// Solves `A x = b` for a symmetric positive definite `A` of `n` by `n` values (row major) by the Cholesky factorisation
/// `A = L L^T`. Only the lower triangle of `A` is read. `A` is overwritten: its lower triangle becomes `L`. `b` becomes `x`.
/// Nothing is allocated. Errors when the sizes do not match `n`, or when `A` is not positive definite (a pivot is not positive,
/// or a value is not finite).
///
/// Standard: G. H. Golub and C. F. Van Loan, "Matrix Computations", 4th ed., algorithm 4.2.2 (the Cholesky factorisation).
///
/// ```
/// use sdrkit_algo_dsp::linalg::cholesky_solve;
/// let mut a = [4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0];
/// let mut b = [-20.0, -43.0, 192.0]; // A times (1, 2, 3)
/// cholesky_solve(&mut a, &mut b, 3).unwrap();
/// assert!(b.iter().zip([1.0, 2.0, 3.0]).all(|(x, y)| (x - y).abs() < 1e-12));
/// ```
pub fn cholesky_solve(a: &mut [f64], b: &mut [f64], n: usize) -> Result<(), DspError> {
    if a.len() != n * n {
        return Param::Matrix.length(n * n, a.len());
    }
    if b.len() != n {
        return Param::Values.length(n, b.len());
    }
    for j in 0..n {
        let mut s = a[j * n + j];
        for k in 0..j {
            s -= a[j * n + k] * a[j * n + k];
        }
        if !(s > 0.0 && s.is_finite()) {
            return Param::Matrix.degenerate();
        }
        let d = s.sqrt();
        a[j * n + j] = d;
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= a[i * n + k] * a[j * n + k];
            }
            a[i * n + j] = s / d;
        }
    }
    // L y = b, then L^T x = y.
    for i in 0..n {
        let mut s = b[i];
        for k in 0..i {
            s -= a[i * n + k] * b[k];
        }
        b[i] = s / a[i * n + i];
    }
    for i in (0..n).rev() {
        let mut s = b[i];
        for k in i + 1..n {
            s -= a[k * n + i] * b[k];
        }
        b[i] = s / a[i * n + i];
    }
    Ok(())
}

/// A linear least-squares fit `y = sum_j x_j row_j` that takes its rows one at a time.
///
/// It keeps the normal equations `(A^T W A) x = A^T W y` of the rows so far (`W` holds the row weights), so a fit over many rows
/// needs only `n^2` values of memory. [`LeastSquares::solve`] adds a ridge `lambda (trace / n) I` to the matrix and solves it by
/// [`cholesky_solve`]. A small ridge (1e-3, for example) keeps a fit stable when some unknowns are hardly excited. The fit keeps
/// its rows after a solve, so more rows can follow. Nothing is allocated after [`LeastSquares::new`].
///
/// ```
/// use sdrkit_algo_dsp::linalg::LeastSquares;
/// // y = 3.5 + 1.4 x through (1, 6), (2, 5), (3, 7), (4, 10).
/// let mut ls = LeastSquares::new(2).unwrap();
/// for (x, y) in [(1.0, 6.0), (2.0, 5.0), (3.0, 7.0), (4.0, 10.0)] {
///     ls.add_row(&[1.0, x], y, 1.0).unwrap();
/// }
/// let mut p = [0.0; 2];
/// ls.solve(0.0, &mut p).unwrap();
/// assert!((p[0] - 3.5).abs() < 1e-12 && (p[1] - 1.4).abs() < 1e-12);
/// ```
#[derive(Clone, Debug)]
pub struct LeastSquares {
    n: usize,
    ata: Vec<f64>,
    atb: Vec<f64>,
    work: Vec<f64>,
    rows: u64,
}

impl LeastSquares {
    /// A fit of `n` unknowns (1 to 4096). Errors for another `n`.
    pub fn new(n: usize) -> Result<Self, DspError> {
        if !(1..=4096).contains(&n) {
            return Param::Unknowns.range(1.0, 4096.0, n as f64);
        }
        Ok(LeastSquares {
            n,
            ata: vec![0.0; n * n],
            atb: vec![0.0; n],
            work: vec![0.0; n * n],
            rows: 0,
        })
    }

    /// The number of unknowns.
    pub fn unknowns(&self) -> usize {
        self.n
    }

    /// Rows added since the start or the last reset.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Forgets every row.
    pub fn reset(&mut self) {
        self.ata.fill(0.0);
        self.atb.fill(0.0);
        self.rows = 0;
    }

    /// Adds the row `row` (`n` values) with the measured value `y` and the weight `w` (at least 0). Errors when `row` does not hold
    /// `n` values, or a value is not finite, or the weight is negative.
    pub fn add_row(&mut self, row: &[f64], y: f64, w: f64) -> Result<(), DspError> {
        let n = self.n;
        if row.len() != n {
            return Param::Row.length(n, row.len());
        }
        if !(w >= 0.0 && w.is_finite()) {
            return Param::Weight.at_least(0.0, w);
        }
        if !y.is_finite() || row.iter().any(|v| !v.is_finite()) {
            return Param::Values.not_finite();
        }
        for i in 0..n {
            let ri = w * row[i];
            if ri == 0.0 {
                continue;
            }
            // The upper triangle only, which `solve` mirrors.
            for (a, &rj) in self.ata[i * n + i..(i + 1) * n].iter_mut().zip(&row[i..]) {
                *a += ri * rj;
            }
            self.atb[i] += ri * y;
        }
        self.rows += 1;
        Ok(())
    }

    /// Solves for the unknowns with the ridge `lambda` (at least 0, relative to the mean diagonal) into `x` (`n` values). Errors when
    /// `x` is the wrong length, `lambda` is negative, or the system has no unique solution (too few rows, or unknowns no row
    /// excites, with no ridge).
    pub fn solve(&mut self, lambda: f64, x: &mut [f64]) -> Result<(), DspError> {
        let n = self.n;
        if x.len() != n {
            return Param::Output.length(n, x.len());
        }
        if !(lambda >= 0.0 && lambda.is_finite()) {
            return Param::Weight.at_least(0.0, lambda);
        }
        let trace: f64 = (0..n).map(|i| self.ata[i * n + i]).sum::<f64>() / n as f64;
        for i in 0..n {
            for j in i..n {
                let v = self.ata[i * n + j];
                self.work[i * n + j] = v;
                self.work[j * n + i] = v;
            }
            self.work[i * n + i] += lambda * trace;
        }
        x.copy_from_slice(&self.atb);
        cholesky_solve(&mut self.work, x, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cholesky_factor_of_the_classic_example() {
        // A = L L^T with L = [[2, 0, 0], [6, 1, 0], [-8, 5, 3]] (the worked example of the Cholesky factorisation).
        let mut a = [4.0, 12.0, -16.0, 12.0, 37.0, -43.0, -16.0, -43.0, 98.0];
        let mut b = [1.0, 0.0, 0.0];
        cholesky_solve(&mut a, &mut b, 3).unwrap();
        let l = [a[0], a[3], a[4], a[6], a[7], a[8]];
        assert_eq!(l, [2.0, 6.0, 1.0, -8.0, 5.0, 3.0]);
        // The first column of the inverse of A: (49.361..., -13.555..., 2.111...) = (1777, -488, 76) / 36.
        let want = [1777.0 / 36.0, -488.0 / 36.0, 76.0 / 36.0];
        for (x, w) in b.iter().zip(want) {
            assert!((x - w).abs() < 1e-9, "{x} {w}");
        }
        // Not positive definite, and wrong sizes.
        let mut a = [1.0, 2.0, 2.0, 1.0];
        assert!(cholesky_solve(&mut a, &mut [1.0, 1.0], 2).is_err());
        assert!(cholesky_solve(&mut [1.0; 3], &mut [1.0; 2], 2).is_err());
        assert!(cholesky_solve(&mut [f64::NAN], &mut [1.0], 1).is_err());
    }

    #[test]
    fn least_squares_fits_lines_weights_and_ridges() {
        // A straight line through exact points comes back exactly.
        let mut ls = LeastSquares::new(2).unwrap();
        for k in 0..10 {
            let x = k as f64;
            ls.add_row(&[1.0, x], -2.0 + 0.5 * x, 1.0).unwrap();
        }
        let mut p = [0.0; 2];
        ls.solve(0.0, &mut p).unwrap();
        assert!((p[0] + 2.0).abs() < 1e-12 && (p[1] - 0.5).abs() < 1e-12);
        // A weight of 0 leaves a row out.
        ls.add_row(&[1.0, 3.0], 100.0, 0.0).unwrap();
        ls.solve(0.0, &mut p).unwrap();
        assert!((p[0] + 2.0).abs() < 1e-12);
        // One row cannot fix two unknowns; a ridge makes it solvable and pulls towards 0.
        let mut one = LeastSquares::new(2).unwrap();
        one.add_row(&[1.0, 1.0], 2.0, 1.0).unwrap();
        assert!(one.solve(0.0, &mut p).is_err());
        one.solve(1e-3, &mut p).unwrap();
        assert!((p[0] - p[1]).abs() < 1e-9 && (p[0] + p[1] - 2.0).abs() < 1e-2);
        // An FIR channel of three taps from a known input.
        let h = [0.9, -0.3, 0.1];
        let u: Vec<f64> = (0..200)
            .map(|i| if (i * 7 + i / 5) % 3 == 0 { 1.0 } else { -1.0 })
            .collect();
        let mut fir = LeastSquares::new(3).unwrap();
        for t in 2..u.len() {
            let row = [u[t], u[t - 1], u[t - 2]];
            let y: f64 = row.iter().zip(h).map(|(a, b)| a * b).sum();
            fir.add_row(&row, y, 1.0).unwrap();
        }
        let mut g = [0.0; 3];
        fir.solve(0.0, &mut g).unwrap();
        assert!(g.iter().zip(h).all(|(a, b)| (a - b).abs() < 1e-10), "{g:?}");
        assert_eq!(fir.rows(), 198);
        fir.reset();
        assert_eq!(fir.rows(), 0);
        assert!(fir.add_row(&[1.0], 1.0, 1.0).is_err());
        assert!(fir.add_row(&[1.0, 1.0, f64::NAN], 1.0, 1.0).is_err());
        assert!(fir.solve(-1.0, &mut g).is_err());
        assert!(LeastSquares::new(0).is_err());
    }
}
