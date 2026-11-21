//! Carrier frequency offset with a linear drift. The instantaneous frequency at sample `n` is `f0 + d n / fs` (Hz), so the phase
//! is `phi(n) = phi0 + f0 n / fs + d n (n - 1) / (2 fs^2)` cycles. The output does not depend on how the stream is split across
//! calls.

use crate::{Param, SimError};
use sdrkit_algo_dsp::approx::sin_cos;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const CHUNK: u32 = 64;

#[inline]
fn phasor(cycles: f64) -> Complex32 {
    let (s, c) = sin_cos(TAU * cycles);
    Complex32::new(c, s)
}

/// Moves a signal by `offset_hz`, with the offset changing by `drift_hz_s` hertz per second (a warming oscillator, a satellite's
/// Doppler ramp). Positive moves the signal up.
///
/// ```
/// use sdrkit_algo_sim::offset::FrequencyOffset;
/// use sdrkit_core::Complex32;
/// let mut f = FrequencyOffset::new(48_000.0, 1_000.0, 0.0).unwrap();
/// let mut x = vec![Complex32::new(1.0, 0.0); 48];
/// f.process(&mut x);
/// // 1 kHz at 48 kHz: a full turn every 48 samples, so sample 12 is a quarter turn.
/// assert!((x[12].im - 1.0).abs() < 1e-5);
/// ```
#[derive(Clone, Debug)]
pub struct FrequencyOffset {
    rate: f64,
    /// Phase (cycles, in [0, 1)) and frequency (cycles per sample) at the start of the current chunk, which is chunk `chunks`.
    phase: f64,
    freq: f64,
    f0: f64,
    chunks: u64,
    /// Drift, cycles per sample per sample.
    drift: f64,
    /// Position inside the chunk, and the phasor and per-sample rotation there.
    pos: u32,
    z: Complex32,
    r: Complex32,
    /// The rotation's own turn per sample (the drift).
    c: Complex32,
}

impl FrequencyOffset {
    /// An offset of `offset_hz` drifting by `drift_hz_s` for samples at `rate_hz`. Errors unless the rate is positive and both
    /// values are finite.
    pub fn new(rate_hz: f64, offset_hz: f64, drift_hz_s: f64) -> Result<Self, SimError> {
        Param::Rate.check_positive(rate_hz)?;
        Param::OffsetHz.check_finite(offset_hz)?;
        Param::Drift.check_finite(drift_hz_s)?;
        let mut f = FrequencyOffset {
            rate: rate_hz,
            phase: 0.0,
            freq: offset_hz / rate_hz,
            f0: offset_hz / rate_hz,
            chunks: 0,
            drift: drift_hz_s / (rate_hz * rate_hz),
            pos: 0,
            z: Complex32::new(1.0, 0.0),
            r: Complex32::new(1.0, 0.0),
            c: Complex32::new(1.0, 0.0),
        };
        f.anchor();
        Ok(f)
    }

    fn anchor(&mut self) {
        self.z = phasor(self.phase);
        self.r = phasor(self.freq);
        self.c = phasor(self.drift);
    }

    /// The offset now (at the next sample), Hz.
    pub fn offset_hz(&self) -> f64 {
        (self.freq + self.drift * self.pos as f64) * self.rate
    }

    /// The phase of the next sample, cycles in `[0, 1)`.
    pub fn phase_cycles(&self) -> f64 {
        let p = self.pos as f64;
        (self.phase + self.freq * p + self.drift * p * (p - 1.0) / 2.0).rem_euclid(1.0)
    }

    /// Advances the exact state to the end of the current chunk and starts the next one.
    fn next_chunk(&mut self) {
        let l = CHUNK as f64;
        self.phase =
            (self.phase + self.freq * l + self.drift * l * (l - 1.0) / 2.0).rem_euclid(1.0);
        // The frequency from the chunk count, so rounding never accumulates in it.
        self.chunks += 1;
        self.freq = self.f0 + self.drift * l * self.chunks as f64;
        self.pos = 0;
        self.anchor();
    }

    /// Shifts `buf` in place.
    pub fn process(&mut self, buf: &mut [Complex32]) {
        let mut i = 0;
        while i < buf.len() {
            if self.pos == CHUNK {
                self.next_chunk();
            }
            let k = ((CHUNK - self.pos) as usize).min(buf.len() - i);
            let (mut z, mut r, c) = (self.z, self.r, self.c);
            for x in &mut buf[i..i + k] {
                *x *= z;
                z *= r;
                r *= c;
            }
            self.z = z;
            self.r = r;
            self.pos += k as u32;
            i += k;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phase_follows_the_quadratic_law_for_a_minute() {
        // 1 MHz rate, 12.345 kHz offset drifting 250 Hz/s, for 60 s: compare the phase of samples spread over the run with the
        // closed form computed in f64 from the sample number.
        let rate = 1e6;
        let (f0, d) = (12_345.0, 250.0);
        let mut f = FrequencyOffset::new(rate, f0, d).unwrap();
        let mut buf = vec![Complex32::new(1.0, 0.0); 1_000_003];
        let mut n0 = 0u64;
        for _ in 0..60 {
            buf.iter_mut().for_each(|v| *v = Complex32::new(1.0, 0.0));
            f.process(&mut buf);
            for &k in &[0usize, 1, 777, 500_000, 1_000_002] {
                let n = (n0 + k as u64) as f64;
                // Split n into whole seconds and the rest so the f64 closed form keeps its precision.
                let want =
                    ((f0 * n / rate) + d * n * (n - 1.0) / (2.0 * rate * rate)).rem_euclid(1.0);
                let got = (buf[k].im as f64).atan2(buf[k].re as f64) / TAU;
                let mut e = (got - want).rem_euclid(1.0);
                if e > 0.5 {
                    e -= 1.0;
                }
                assert!(e.abs() < 2e-5, "sample {n}: phase error {e} cycles");
                assert!((buf[k].norm() - 1.0).abs() < 1e-4);
            }
            n0 += buf.len() as u64;
        }
        assert!((f.offset_hz() - (f0 + d * n0 as f64 / rate)).abs() < 1e-6);
    }

    #[test]
    fn any_cut_returns_the_same_output() {
        let mut a = FrequencyOffset::new(48e3, -1234.5, 77.0).unwrap();
        let mut b = FrequencyOffset::new(48e3, -1234.5, 77.0).unwrap();
        let mut x = vec![Complex32::new(0.5, 0.25); 10_000];
        let mut y = x.clone();
        a.process(&mut x);
        for c in y.chunks_mut(61) {
            b.process(c);
        }
        assert_eq!(x, y);
        assert!(FrequencyOffset::new(0.0, 1.0, 0.0).is_err());
        assert!(FrequencyOffset::new(1.0, f64::NAN, 0.0).is_err());
        assert!((b.phase_cycles() - a.phase_cycles()).abs() < 1e-12);
    }
}
