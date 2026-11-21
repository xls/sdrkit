//! The filter design the channel filters of the stacks use, on the windowed-sinc design of sdrkit-algo-dsp.

use sdrkit_algo_dsp::fir::design_by_taps;
use sdrkit_algo_dsp::window::Window;

/// A Blackman-windowed low-pass of `taps` taps (made odd, at least 3) with its -6 dB point at `cutoff_hz` at `rate`, unity gain
/// at DC. A cut-off at or above half the rate returns the pass-through filter `[1]`.
pub fn blackman_lowpass(rate: f64, cutoff_hz: f64, taps: usize) -> Vec<f32> {
    design_by_taps(taps.max(3) | 1, cutoff_hz / rate, Window::Blackman)
        .unwrap_or_else(|_| vec![1.0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_gain_at_dc() {
        let h = blackman_lowpass(80e3, 15e3, 63);
        assert_eq!(h.len(), 63);
        assert!((h.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert_eq!(blackman_lowpass(80e3, 50e3, 63), vec![1.0]);
    }
}
