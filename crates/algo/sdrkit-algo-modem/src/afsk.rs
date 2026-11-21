//! Hard bits of an AFSK signal for every sampling phase: two tone correlators over one bit of real audio, a decision every
//! `window / phases` samples, and the decisions dealt round-robin to the phases. A frame search per phase then finds a sync word
//! at the phase nearest the bit centre without a bit clock.
//!
//! With the window exactly one bit long and each tone a whole or half number of cycles in it, the image of a real tone at the
//! negative frequency sums to zero over the window, so real audio needs no Hilbert transformer.
//!
//! ```
//! use sdrkit_algo_modem::afsk::PhaseBits;
//! let rate = 48_000.0;
//! let mut d = PhaseBits::new(rate, 1200.0, 1200.0, 1800.0, 8).unwrap();
//! let mut ones = 0;
//! for n in 0..400 {
//!     let x = (std::f64::consts::TAU * 1200.0 * n as f64 / rate).cos() as f32;
//!     if let Some(b) = d.push(x) {
//!         ones += usize::from(b.bit);
//!     }
//! }
//! assert!(ones > 60);
//! ```

use crate::error::Param;
use crate::fsk::ToneCorrelator;
use crate::{Complex32, ModemError};

/// One decision: the sampling phase it belongs to and the bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseBit {
    /// The sampling phase, 0 to `phases - 1`.
    pub phase: usize,
    /// 1 when the tone of a one is the stronger.
    pub bit: u8,
    /// `(|S_one| - |S_zero|) / (|S_one| + |S_zero|)`, from -1 to 1 (0 with no signal).
    pub margin: f32,
}

/// The AFSK hard-bit detector (module documentation).
pub struct PhaseBits {
    corr: ToneCorrelator,
    step: usize,
    phases: usize,
    since: usize,
    next_phase: usize,
}

impl PhaseBits {
    /// A detector of `baud` bits a second at `rate` with the tone of a one at `one_hz` and of a zero at `zero_hz`, with `phases`
    /// sampling phases a bit.
    ///
    /// # Errors
    ///
    /// [`ModemError`] if a bit is not a whole number of samples, the bit is not a whole multiple of `phases` samples, or a tone
    /// lies above half the rate.
    pub fn new(
        rate: f64,
        baud: f64,
        one_hz: f64,
        zero_hz: f64,
        phases: usize,
    ) -> Result<Self, ModemError> {
        let spb = rate / baud;
        let window = spb.round();
        if !(window >= 2.0 && (spb - window).abs() < 1e-9 * spb) {
            return Param::Window.range(2.0, 65_536.0, spb);
        }
        let window = window as usize;
        if phases == 0 || !window.is_multiple_of(phases) {
            return Param::Window.range(1.0, window as f64, phases as f64);
        }
        let corr = ToneCorrelator::new(&[one_hz / rate, zero_hz / rate], window)?;
        Ok(PhaseBits {
            corr,
            step: window / phases,
            phases,
            since: 0,
            next_phase: 0,
        })
    }

    /// The sampling phases of a bit.
    pub fn phases(&self) -> usize {
        self.phases
    }

    /// Samples between two decisions.
    pub fn step(&self) -> usize {
        self.step
    }

    /// Takes one audio sample. Returns a decision every [`PhaseBits::step`] samples once a whole bit has been seen.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<PhaseBit> {
        self.corr.push(Complex32::new(x, 0.0));
        self.since += 1;
        if self.since < self.step {
            return None;
        }
        self.since = 0;
        let phase = self.next_phase;
        self.next_phase = (self.next_phase + 1) % self.phases;
        if !self.corr.ready() {
            return None;
        }
        let s = self.corr.sums();
        let (one, zero) = (s[0].norm(), s[1].norm());
        let total = one + zero;
        Some(PhaseBit {
            phase,
            bit: u8::from(one > zero),
            margin: if total > 0.0 {
                (one - zero) / total
            } else {
                0.0
            },
        })
    }

    /// Forgets the signal and restarts the phases at 0.
    pub fn reset(&mut self) {
        self.corr.reset();
        self.since = 0;
        self.next_phase = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Phase-continuous AFSK audio of `bits` at 48 kHz, 1200 bit/s, 1200 Hz for a one and 1800 Hz for a zero.
    fn audio(bits: &[u8]) -> Vec<f32> {
        let mut ph = 0.0f64;
        let mut out = Vec::new();
        for &b in bits {
            let f = if b == 1 { 1200.0 } else { 1800.0 };
            for _ in 0..40 {
                out.push(ph.cos() as f32);
                ph += TAU * f / 48_000.0;
            }
        }
        out
    }

    #[test]
    fn every_phase_reads_the_bits_and_the_centre_phase_is_clean() {
        let bits: Vec<u8> = (0..64u32).map(|i| ((i * 7 + 3) % 5 % 2) as u8).collect();
        let x = audio(&bits);
        let mut d = PhaseBits::new(48_000.0, 1200.0, 1200.0, 1800.0, 8).unwrap();
        let mut per_phase: Vec<Vec<u8>> = vec![Vec::new(); 8];
        for &v in &x {
            if let Some(b) = d.push(v) {
                per_phase[b.phase].push(b.bit);
            }
        }
        // Phase 7 decides at the end of each bit: its window is the bit itself.
        assert_eq!(per_phase[7][..], bits[..per_phase[7].len()]);
        assert!(per_phase.iter().all(|p| p.len() >= 62));
    }

    #[test]
    fn refuses_a_bit_that_is_not_whole_samples() {
        assert!(PhaseBits::new(44_100.0, 1200.0, 1200.0, 1800.0, 8).is_err());
        assert!(PhaseBits::new(48_000.0, 1200.0, 1200.0, 1800.0, 7).is_err());
    }
}
