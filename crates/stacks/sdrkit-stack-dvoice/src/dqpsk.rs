//! The π/4-DQPSK test signal of TETRA and P25 Phase 2: dibits to phase steps, shaped by the shared linear modulator with a
//! root raised cosine pulse. The dibits 00, 01, 10 and 11 turn the phase by +π/4, +3π/4, -π/4 and -3π/4.

use sdrkit_algo_sim::modulate::Linear;
use sdrkit_core::Complex32;

/// The differential detector's output units per quarter of π.
pub const UNITS_PER_QUARTER: f32 = 1000.0;

/// A π/4-DQPSK modulator with root raised cosine shaping at `rate`, unit mean power.
#[derive(Debug)]
pub struct DqpskModulator {
    m: Linear,
    phase: f32,
    syms: Vec<Complex32>,
}

impl DqpskModulator {
    pub fn new(baud: f64, rate: f64, alpha: f64) -> DqpskModulator {
        DqpskModulator {
            m: Linear::rrc(alpha, 8.0, rate / baud).expect("a valid pulse and rate"),
            phase: 0.0,
            syms: Vec::new(),
        }
    }

    /// Sends dibits (from bits, two per symbol).
    pub fn bits(&mut self, bits: &[u8], out: &mut Vec<Complex32>) {
        self.syms.clear();
        for c in bits.chunks(2) {
            let d = (c[0] << 1) | c.get(1).copied().unwrap_or(0);
            let q = sdrkit_algo_fec::conv::tcm::dibit_symbol(d) as f32;
            self.phase =
                (self.phase + q * std::f32::consts::FRAC_PI_4).rem_euclid(std::f32::consts::TAU);
            self.syms.push(Complex32::from_polar(1.0, self.phase));
        }
        self.m.push(&self.syms, out);
    }

    /// Lets the pulse run out.
    pub fn flush(&mut self, out: &mut Vec<Complex32>) {
        self.m.flush(out);
    }
}
