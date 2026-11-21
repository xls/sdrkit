//! The constants of the design and the numeric helpers that every stage uses. Each `f32` math function runs in `f64` and
//! rounds once to `f32`, so the output does not depend on the `f32` math library of the platform.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), sections 3.2 to 3.8.

/// pi as the design states it (10 digits). Formulas that name this constant use it, not the exact value.
#[allow(clippy::approx_constant)]
pub const PI: f64 = 3.141592654;
/// Two times pi, as the design states it. It is not exactly two times [`PI`].
#[allow(clippy::approx_constant)]
pub const TWO_PI: f64 = 6.283185307;
/// The exact pi, which the rate-K (700C) stages use.
pub const M_PI: f64 = std::f64::consts::PI;
/// ln(10) rounded to `f32`.
pub const LN10F: f32 = std::f64::consts::LN_10 as f32;

/// Samples in one internal frame (10 ms at 8 kHz).
pub const N_SAMP: usize = 80;
/// The analysis buffer length.
pub const M_PITCH: usize = 320;
/// The FFT size for analysis and synthesis.
pub const FFT_SIZE: usize = 512;
/// The size of the harmonic arrays. Index 1 to 160 is valid.
pub const MAX_AMP: usize = 160;
/// The LPC order.
pub const LPC_ORD: usize = 10;
/// The rate-K vector length for 700C.
pub const K: usize = 20;
/// The FFT size of the 700C phase synthesis.
pub const PHASE_NFFT: usize = 128;

/// The lowest fundamental, in radians per sample (50 Hz).
pub fn wo_min() -> f32 {
    (TWO_PI / 160.0) as f32
}

/// The highest fundamental, in radians per sample (400 Hz).
pub fn wo_max() -> f32 {
    (TWO_PI / 20.0) as f32
}

/// The cosine.
#[inline]
pub fn cosf(x: f32) -> f32 {
    (x as f64).cos() as f32
}

/// The sine.
#[inline]
pub fn sinf(x: f32) -> f32 {
    (x as f64).sin() as f32
}

/// The two-argument arc tangent of `y / x`.
#[inline]
pub fn atan2f(y: f32, x: f32) -> f32 {
    (y as f64).atan2(x as f64) as f32
}

/// The exponential.
#[inline]
pub fn expf(x: f32) -> f32 {
    (x as f64).exp() as f32
}

/// The natural logarithm.
#[inline]
pub fn logf(x: f32) -> f32 {
    (x as f64).ln() as f32
}

/// The base-10 logarithm.
#[inline]
pub fn log10f(x: f32) -> f32 {
    (x as f64).log10() as f32
}

/// `x` to the power `y`.
#[inline]
pub fn powf(x: f32, y: f32) -> f32 {
    (x as f64).powf(y as f64) as f32
}

/// 10 to the power of an `f32` value: the product with ln(10) is in `f32`.
#[inline]
pub fn pow10f(x: f32) -> f32 {
    expf(LN10F * x)
}

/// 10 to the power of an `f64` expression: the product with ln(10) is in `f64` and rounds once.
#[inline]
pub fn pow10d(x: f64) -> f32 {
    expf((LN10F as f64 * x) as f32)
}

/// Truncation toward zero of an `f64`, as a C cast to `int` does. Values outside the `i32` range saturate.
#[inline]
pub fn trunc_i(x: f64) -> i32 {
    x as i32
}
