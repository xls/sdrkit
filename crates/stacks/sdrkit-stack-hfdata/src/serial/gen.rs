//! The test-signal modulator of the serial-tone stacks: complex symbols at 2400 Bd through the root-raised-cosine pulse, on the
//! 1800 Hz carrier above a dial placed where a receiver of the stack looks, at any sample rate, repeated without end.

use super::front::ROLL_OFF;
use super::{BAUD, CARRIER_HZ, TUNE_OFFSET_HZ};
use num_complex::Complex64;
use sdrkit_algo_sim::modulate::Linear;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// Where a test signal at `rate` puts the dial, Hz from the centre of its samples: [`TUNE_OFFSET_HZ`] below the centre, where a
/// receiver of the stack looks, when the rate holds the offset and the audio band; else at the centre.
pub fn test_dial(rate: f64) -> f64 {
    if rate / 2.0 >= TUNE_OFFSET_HZ + 4000.0 {
        -TUNE_OFFSET_HZ
    } else {
        0.0
    }
}

/// A generator of a repeated symbol sequence (silence is a run of zero symbols in it) at unit mean power while it sends.
pub struct SymbolGen {
    rate: f64,
    lin: Linear,
    symbols: Vec<Complex32>,
    pos: usize,
    pending: Vec<Complex32>,
    taken: usize,
    osc: Complex64,
    step: Complex64,
    n: u64,
    bandwidth_hz: f64,
    baud: f64,
}

impl SymbolGen {
    /// The sequence `symbols` at `baud` with the carrier `carrier_hz` above the dial, at `rate`; the SNR is set in
    /// `bandwidth_hz`.
    pub fn new(
        rate: f64,
        symbols: Vec<Complex32>,
        baud: f64,
        carrier_hz: f64,
        bandwidth_hz: f64,
    ) -> Self {
        let f = test_dial(rate) + carrier_hz;
        let w = std::f64::consts::TAU * f / rate;
        SymbolGen {
            rate,
            lin: Linear::rrc(ROLL_OFF, 12.0, rate / baud).expect("the pulse is valid"),
            symbols,
            pos: 0,
            pending: Vec::new(),
            taken: 0,
            osc: Complex64::new(1.0, 0.0),
            step: Complex64::new(w.cos(), w.sin()),
            n: 0,
            bandwidth_hz,
            baud,
        }
    }

    /// The serial-tone signal of `symbols` (2400 Bd, 1800 Hz, SNR in 3 kHz).
    pub fn serial(rate: f64, symbols: Vec<Complex32>) -> Self {
        Self::new(rate, symbols, BAUD, CARRIER_HZ, 3000.0)
    }

    fn refill(&mut self) {
        if self.symbols.is_empty() {
            let n = (self.rate / self.baud).ceil() as usize;
            self.pending
                .extend(std::iter::repeat_n(Complex32::default(), n));
            return;
        }
        let end = (self.pos + 64).min(self.symbols.len());
        let chunk = &self.symbols[self.pos..end];
        self.lin.push(chunk, &mut self.pending);
        self.pos = if end == self.symbols.len() { 0 } else { end };
    }
}

impl Generator for SymbolGen {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(self.bandwidth_hz).with_symbol_rate(self.baud))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        test_dial(self.rate)
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for o in out.iter_mut() {
            while self.taken >= self.pending.len() {
                self.pending.clear();
                self.taken = 0;
                self.refill();
            }
            let s = self.pending[self.taken];
            self.taken += 1;
            let v = Complex64::new(f64::from(s.re), f64::from(s.im)) * self.osc;
            *o = Complex32::new(v.re as f32, v.im as f32);
            self.osc *= self.step;
            self.n += 1;
            if self.n.is_multiple_of(4096) {
                self.osc /= self.osc.norm();
            }
        }
        out.len()
    }
}
