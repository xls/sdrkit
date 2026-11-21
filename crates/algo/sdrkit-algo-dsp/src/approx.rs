//! Fast approximations of `atan2`, `sin` and `cos` for the per-sample work of demodulators and phase-locked loops, and of the
//! power of a level in dB for the per-bin work on spectra, with stated accuracy and no branches (so loops over them
//! vectorise).

use std::f32::consts::{FRAC_PI_2, PI};

/// Odd minimax polynomial for `atan(a)` on `[0, 1]`: `a (c0 + c1 a^2 + ... + c6 a^12)`.
const ATAN: [f32; 7] = [
    0.999_996_1,
    -0.333_173_7,
    0.198_078_16,
    -0.132_333_43,
    0.079_623_67,
    -0.033_604_22,
    0.006_811_792_5,
];

/// The largest error of [`atan2`] in radians (measured over the whole circle; `f32::atan2` itself is within about 1e-7).
pub const ATAN2_MAX_ERROR: f32 = 6e-7;

/// `atan2(y, x)` in radians, within [`ATAN2_MAX_ERROR`], without branches (so a loop over it vectorises). `atan2(0, 0)` is 0.
/// At 75 kHz deviation on a 500 kHz channel the error is 120 dB below full deviation.
#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (x.abs(), y.abs());
    let (lo, hi) = (ax.min(ay), ax.max(ay));
    let a = if hi > 0.0 { lo / hi } else { 0.0 };
    let s = a * a;
    let p = ATAN[0]
        + s * (ATAN[1]
            + s * (ATAN[2] + s * (ATAN[3] + s * (ATAN[4] + s * (ATAN[5] + s * ATAN[6])))));
    let mut r = a * p;
    r = if ay > ax { FRAC_PI_2 - r } else { r };
    r = if x < 0.0 { PI - r } else { r };
    r.copysign(y)
}

/// The largest error of [`sin_cos`] (absolute, on both outputs).
pub const SIN_COS_MAX_ERROR: f32 = 4e-7;

/// `(sin x, cos x)` for any `x` in radians (reduced in `f64`, so a phase accumulated over hours stays exact), within
/// [`SIN_COS_MAX_ERROR`]: a quarter-turn reduction and short polynomials on `[-pi/4, pi/4]`.
#[inline]
pub fn sin_cos(x: f64) -> (f32, f32) {
    let k = (x * std::f64::consts::FRAC_2_PI).round();
    let r = (x - k * std::f64::consts::FRAC_PI_2) as f32;
    let q = (k as i64 & 3) as u32;
    let r2 = r * r;
    // Taylor to r^9 and r^8: within 3e-8 on |r| <= pi/4, plus the f32 rounding.
    let s = r
        * (1.0
            + r2 * (-1.0 / 6.0
                + r2 * (1.0 / 120.0 + r2 * (-1.0 / 5040.0 + r2 * (1.0 / 362_880.0)))));
    let c = 1.0 + r2 * (-0.5 + r2 * (1.0 / 24.0 + r2 * (-1.0 / 720.0 + r2 * (1.0 / 40_320.0))));
    let (s, c) = if q & 1 == 1 { (c, -s) } else { (s, c) };
    if q & 2 == 2 {
        (-s, -c)
    } else {
        (s, c)
    }
}

/// The largest error of [`power_of_db`], dB.
pub const POWER_OF_DB_MAX_ERROR: f32 = 1e-3;
/// The lowest and the highest level that [`power_of_db`] takes, dB.
pub const POWER_OF_DB_RANGE: (f32, f32) = (-370.0, 370.0);
/// log2(10) / 10: dB to the exponent of 2.
const DB_TO_LOG2: f32 = std::f32::consts::LOG2_10 / 10.0;

/// The linear power of `db` dB, within [`POWER_OF_DB_MAX_ERROR`], for levels in [`POWER_OF_DB_RANGE`]: a level outside
/// is taken as the nearer end, one that is not a number as the low end. The whole part of the exponent of 2 goes into
/// the exponent bits; a polynomial gives the rest.
#[inline]
pub fn power_of_db(db: f32) -> f32 {
    let (lo, hi) = POWER_OF_DB_RANGE;
    let db = if db >= lo { db.min(hi) } else { lo };
    let x = db * DB_TO_LOG2;
    let whole = x.floor();
    let f = (x - whole) * std::f32::consts::LN_2;
    let part = 1.0
        + f * (1.0
            + f * (0.5 + f * (1.0 / 6.0 + f * (1.0 / 24.0 + f * (1.0 / 120.0 + f / 720.0)))));
    part * f32::from_bits(((whole as i32 + 127) as u32) << 23)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_of_db_is_within_its_bound() {
        let (lo, hi) = POWER_OF_DB_RANGE;
        let mut worst = 0.0f64;
        for k in -36_990..=36_990 {
            let db = k as f32 * 0.01 + 0.0037;
            let got = 10.0 * f64::from(power_of_db(db)).log10();
            worst = worst.max((got - f64::from(db)).abs());
        }
        assert!(
            worst <= f64::from(POWER_OF_DB_MAX_ERROR),
            "worst {worst} dB"
        );
        assert_eq!(power_of_db(0.0), 1.0);
        assert_eq!(power_of_db(f32::NEG_INFINITY), power_of_db(lo));
        assert_eq!(power_of_db(f32::NAN), power_of_db(lo));
        assert_eq!(power_of_db(f32::INFINITY), power_of_db(hi));
    }

    #[test]
    fn sin_cos_is_within_its_bound() {
        let mut worst = 0.0f32;
        for i in 0..400_001 {
            let x = (i as f64 - 200_000.0) / 200_000.0 * 40.0;
            let (s, c) = sin_cos(x);
            worst = worst
                .max((s - x.sin() as f32).abs())
                .max((c - x.cos() as f32).abs());
        }
        assert!(worst <= SIN_COS_MAX_ERROR, "worst {worst}");
        // A phase far from zero is still exact.
        let x = 1e6 * std::f64::consts::TAU + 0.3;
        assert!((sin_cos(x).0 - 0.3f32.sin()).abs() < 1e-6);
    }
}
