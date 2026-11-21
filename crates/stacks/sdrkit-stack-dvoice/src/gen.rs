//! The test signals: frequency modulation of the symbols of a stack through the shared continuous-phase modulator, and a
//! prepared 48 kHz signal read out at any rate. Each generator makes the clean signal at unit power. The framework adds the
//! impairments.

use crate::front::RATE;
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_sim::modulate::Cpm;
use sdrkit_core::{Complex32, Generator, Hz, SignalInfo};

/// The pulse an FSK transmitter shapes its frequency steps with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TxPulse {
    RootRaisedCosine(f64),
    RaisedCosine(f64),
    /// Gaussian with this bandwidth-time product (GMSK).
    Gaussian(f64),
}

/// An FSK or GMSK modulator at 48 kHz: symbol levels in, unit-amplitude complex baseband out, phase continuous across calls.
#[derive(Debug)]
pub struct FskModulator {
    cpm: Cpm,
}

impl FskModulator {
    /// Symbols at `baud` with the frequency pulse `pulse`. A level of 1 moves the frequency by `hz_per_level` (so +3 moves
    /// it by 3 times that).
    pub fn new(baud: f64, pulse: TxPulse, hz_per_level: f64) -> FskModulator {
        let (shape, span) = match pulse {
            TxPulse::RootRaisedCosine(b) => (Shape::Rrc { beta: b }, 8.0),
            TxPulse::RaisedCosine(b) => (Shape::Rc { beta: b }, 8.0),
            TxPulse::Gaussian(bt) => (
                Shape::GaussianFreq { bt },
                (1.0 + 8.0 * 2f64.ln().sqrt() / (std::f64::consts::TAU * bt)).ceil(),
            ),
        };
        FskModulator {
            cpm: Cpm::with_deviation(shape, span, hz_per_level, baud, RATE)
                .expect("a valid pulse and rate"),
        }
    }

    /// Appends the samples of `symbols` (levels) to `out`.
    pub fn symbols(&mut self, symbols: &[f32], out: &mut Vec<Complex32>) {
        self.cpm.push(symbols, out);
    }

    /// Lets the pulse run out with the carrier on (`n` symbols of level 0), for a clean end.
    pub fn flush(&mut self, n: usize, out: &mut Vec<Complex32>) {
        let z = vec![0.0; n];
        self.cpm.push(&z, out);
        self.cpm.flush(out);
    }

    /// Starts again between transmissions.
    pub fn reset(&mut self) {}
}

/// Appends `seconds` of silence (no carrier).
pub fn silence(seconds: f64, out: &mut Vec<Complex32>) {
    let n = (seconds * RATE).round() as usize;
    out.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), n));
}

/// Dibits to symbol levels (01 +3, 00 +1, 10 -1, 11 -3) from bits.
pub fn bits_to_levels(bits: &[u8]) -> Vec<f32> {
    bits.chunks(2)
        .map(|c| {
            sdrkit_algo_fec::conv::tcm::dibit_symbol((c[0] << 1) | c.get(1).copied().unwrap_or(0))
                as f32
        })
        .collect()
}

/// A prepared 48 kHz signal, read out at any rate, once or repeated, with what it is.
pub struct Prepared {
    info: Option<SignalInfo>,
    source_rate: Hz,
    signal: Vec<Complex32>,
    pos: usize,
    repeat: bool,
    rate: Hz,
    resampler: Option<Resampler<Complex32>>,
    pending: Vec<Complex32>,
    pending_pos: usize,
    ended: bool,
}

impl Prepared {
    pub fn new(signal: Vec<Complex32>, rate: Hz, repeat: bool) -> Prepared {
        Prepared::with_source_rate(signal, RATE, rate, repeat)
    }
    /// A signal prepared at `source_rate`.
    pub fn with_source_rate(
        signal: Vec<Complex32>,
        source_rate: Hz,
        rate: Hz,
        repeat: bool,
    ) -> Prepared {
        let resampler = if (rate - source_rate).abs() < 1e-6 {
            None
        } else {
            Resampler::with_quality(source_rate, rate, 0.46, 70.0).ok()
        };
        Prepared {
            info: None,
            source_rate,
            signal,
            pos: 0,
            repeat,
            rate,
            resampler,
            pending: Vec::new(),
            pending_pos: 0,
            ended: false,
        }
    }
    /// The same signal, declaring its bandwidth, symbol rate and bit rate (unit power while it transmits).
    pub fn with_info(mut self, bandwidth_hz: Hz, symbol_rate_hz: Hz, bit_rate_hz: Hz) -> Prepared {
        let mut i = SignalInfo::new(bandwidth_hz);
        i.symbol_rate_hz = Some(symbol_rate_hz);
        i.bit_rate_hz = Some(bit_rate_hz);
        self.info = Some(i);
        self
    }
    /// The same signal, read once (it ends instead of repeating).
    pub fn once(mut self) -> Prepared {
        self.repeat = false;
        self
    }
    /// The prepared signal's length in seconds.
    pub fn seconds(&self) -> f64 {
        self.signal.len() as f64 / self.source_rate
    }
}

impl Generator for Prepared {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.info
    }
    /// The channel is at 0 Hz, on the tuned frequency.
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut n = 0;
        while n < out.len() {
            if self.pending_pos < self.pending.len() {
                let k = (self.pending.len() - self.pending_pos).min(out.len() - n);
                out[n..n + k]
                    .copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + k]);
                self.pending_pos += k;
                n += k;
                continue;
            }
            if self.ended || self.signal.is_empty() {
                break;
            }
            // Next piece of the 48 kHz signal.
            let piece = ((self.source_rate / 10.0) as usize)
                .max(1)
                .min(self.signal.len() - self.pos);
            let src = &self.signal[self.pos..self.pos + piece];
            self.pending.clear();
            self.pending_pos = 0;
            match self.resampler.as_mut() {
                Some(r) => r.process(src, &mut self.pending),
                None => self.pending.extend_from_slice(src),
            }
            self.pos += piece;
            if self.pos >= self.signal.len() {
                if self.repeat {
                    self.pos = 0;
                } else {
                    // Flush the resampler with silence so the end is not cut.
                    if let Some(r) = self.resampler.as_mut() {
                        let z = vec![Complex32::new(0.0, 0.0); 4800];
                        r.process(&z, &mut self.pending);
                    }
                    self.ended = true;
                }
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fsk_levels_reach_their_deviation() {
        let mut m = FskModulator::new(4800.0, TxPulse::RootRaisedCosine(0.2), 648.0);
        let mut out = Vec::new();
        m.symbols(&[3.0; 40], &mut out);
        // The mean over one symbol in the middle of the run.
        let d: f64 = (201..211)
            .map(|i| (out[i] * out[i - 1].conj()).arg() as f64)
            .sum::<f64>()
            / 10.0;
        let f = d * RATE / std::f64::consts::TAU;
        assert!((f - 1944.0).abs() < 5.0, "{f}");
    }

    #[test]
    fn prepared_signals_resample_and_end() {
        let sig = vec![Complex32::new(1.0, 0.0); 48_000];
        let mut p = Prepared::new(sig, 96_000.0, false);
        let mut buf = vec![Complex32::new(0.0, 0.0); 50_000];
        let mut total = 0;
        loop {
            let n = p.generate(&mut buf);
            total += n;
            if n < buf.len() {
                break;
            }
        }
        assert!((total as i64 - 96_000).abs() < 12_000, "{total}");
    }
}
