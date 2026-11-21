//! The accuracy of the inverse DCT by the procedure of IEEE Std 1180-1990: random blocks in three ranges and with both
//! signs, compared with the transform in 64-bit floating point.

use sdrkit_codec_mpeg2video::idct::inverse;
use std::f64::consts::PI;

/// The random numbers of the standard.
struct Random(u32);

impl Random {
    fn next(&mut self, low: i32, high: i32) -> i32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let x = (self.0 & 0x7fff_fffe) as f64 / 0x7fff_ffff as f64 * (low + high + 1) as f64;
        x as i32 - low
    }
}

fn basis() -> [[f64; 8]; 8] {
    std::array::from_fn(|k| {
        std::array::from_fn(|n| {
            let s = if k == 0 { (0.125f64).sqrt() } else { 0.5 };
            s * ((2 * n + 1) as f64 * k as f64 * PI / 16.0).cos()
        })
    })
}

/// The forward (`forward`) or the inverse transform of `x` in floating point.
fn exact(x: &[f64; 64], forward: bool) -> [f64; 64] {
    let c = basis();
    let mut out = [0.0; 64];
    for a in 0..8 {
        for b in 0..8 {
            let mut sum = 0.0;
            for i in 0..8 {
                for j in 0..8 {
                    sum += if forward {
                        c[a][i] * c[b][j] * x[8 * i + j]
                    } else {
                        c[i][a] * c[j][b] * x[8 * i + j]
                    };
                }
            }
            out[8 * a + b] = sum;
        }
    }
    out
}

#[test]
fn the_inverse_dct_meets_ieee_1180() {
    for (low, high) in [(256, 255), (5, 5), (300, 300)] {
        for sign in [1, -1] {
            let mut random = Random(1);
            let mut sum = [0i64; 64];
            let mut squares = [0i64; 64];
            let blocks = 10_000;
            for _ in 0..blocks {
                let samples: [f64; 64] =
                    std::array::from_fn(|_| (sign * random.next(low, high)) as f64);
                let coefficients = exact(&samples, true).map(|v| v.round().clamp(-2048.0, 2047.0));
                let want =
                    exact(&coefficients, false).map(|v| v.round().clamp(-256.0, 255.0) as i64);
                let got = inverse(&coefficients.map(|v| v as i16));
                for i in 0..64 {
                    let e = got[i] as i64 - want[i];
                    assert!(e.abs() <= 1, "peak error {e} in range {low}/{high}");
                    sum[i] += e;
                    squares[i] += e * e;
                }
            }
            let n = blocks as f64;
            for i in 0..64 {
                assert!(
                    squares[i] as f64 / n <= 0.06,
                    "mean square error at {i}: {}",
                    squares[i] as f64 / n
                );
                assert!(
                    (sum[i] as f64 / n).abs() <= 0.015,
                    "mean error at {i}: {}",
                    sum[i] as f64 / n
                );
            }
            let all_squares = squares.iter().sum::<i64>() as f64 / (64.0 * n);
            let all = sum.iter().sum::<i64>() as f64 / (64.0 * n);
            assert!(
                all_squares <= 0.02,
                "overall mean square error {all_squares}"
            );
            assert!(all.abs() <= 0.0015, "overall mean error {all}");
        }
    }
}

#[test]
fn a_block_of_zeros_gives_zeros() {
    assert_eq!(inverse(&[0; 64]), [0; 64]);
}
