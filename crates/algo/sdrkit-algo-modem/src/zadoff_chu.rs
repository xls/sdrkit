//! Zadoff-Chu sequences with the phase computed exactly in integers.
//!
//! For root `u` and length `N`, `x[n] = exp(-j pi u q / N)` with `q = n (n + 1)` for odd `N` and `q = n^2` for even `N`. The phase
//! index `t = (u mod 2N) (q mod 2N) mod 2N` is an integer, so `x[n] = exp(-j pi t / N)` keeps full accuracy for long sequences.
//! A negative root uses `|u|` and the opposite sign of the exponent.
//!
//! ```
//! use sdrkit_algo_modem::zadoff_chu::zadoff_chu;
//! let x = zadoff_chu(7, 32);
//! assert!((x[1].re - 0.7730104).abs() < 1e-6 && (x[1].im + 0.6343933).abs() < 1e-6);
//! ```

use crate::Complex32;

/// The phase index `t` of element `n`: the angle is `-pi t / len` (its sign turned for a negative root).
pub fn phase_index(root: i64, len: usize, n: usize) -> u64 {
    let m = 2 * len as u128;
    let n = n as u128;
    let q = if len % 2 == 1 { n * (n + 1) } else { n * n };
    let u = root.unsigned_abs() as u128;
    ((u % m) * (q % m) % m) as u64
}

/// Element `n` of the sequence of root `root` and length `len`, in double precision.
pub fn element(root: i64, len: usize, n: usize) -> (f64, f64) {
    let t = phase_index(root, len, n) as f64;
    let a = -std::f64::consts::PI * t / len as f64;
    let a = if root < 0 { -a } else { a };
    (a.cos(), a.sin())
}

/// The whole sequence of root `root` and length `len` (unit magnitude).
pub fn zadoff_chu(root: i64, len: usize) -> Vec<Complex32> {
    (0..len)
        .map(|n| {
            let (c, s) = element(root, len, n);
            Complex32::new(c as f32, s as f32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_7_length_32_matches_its_table() {
        let x = zadoff_chu(7, 32);
        let want = [
            (0, 1.0, 0.0),
            (2, -0.9238795, -0.3826834),
            (3, 0.9951847, 0.0980171),
            (4, 0.0, 1.0),
            (7, -0.6343933, -0.7730104),
            (13, -0.9951847, -0.0980171),
            (25, -0.6343933, -0.7730104),
            (31, 0.7730104, -0.6343933),
        ];
        for (i, re, im) in want {
            assert!((x[i].re - re).abs() < 1e-6 && (x[i].im - im).abs() < 1e-6, "{i}");
        }
    }

    #[test]
    fn odd_length_and_negative_root() {
        let x = zadoff_chu(25, 63);
        let y = zadoff_chu(-25, 63);
        for n in 0..63 {
            assert!((x[n] - y[n].conj()).norm() < 1e-6);
            assert!((x[n].norm() - 1.0).abs() < 1e-6);
        }
        // n (n + 1) for odd lengths: element 1 has q = 2.
        let (c, s) = element(1, 5, 1);
        let a = -std::f64::consts::PI * 2.0 / 5.0;
        assert!((c - a.cos()).abs() < 1e-12 && (s - a.sin()).abs() < 1e-12);
        // Exact for long sequences: the index stays below 2N.
        assert!(phase_index(839, 1_000_003, 999_999) < 2_000_006);
    }
}
