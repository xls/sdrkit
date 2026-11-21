//! The coefficient of a one-pole smoother.

/// The coefficient of a one-pole smoother with time constant `tau_s` at `rate` samples a second: `1 - exp(-1 / (tau_s rate))`.
/// A time constant or a rate of 0 or less returns 1 (no smoothing).
#[inline]
pub fn one_pole(tau_s: f64, rate: f64) -> f32 {
    if tau_s <= 0.0 || rate <= 0.0 {
        return 1.0;
    }
    (1.0 - (-1.0 / (tau_s * rate)).exp()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_coefficient_of_one_time_constant() {
        assert!((one_pole(1.0, 1.0) - (1.0 - (-1.0f64).exp()) as f32).abs() < 1e-7);
        assert_eq!(one_pole(0.0, 48e3), 1.0);
    }
}
