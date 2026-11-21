//! AIS bursts at any sample rate: the shared GMSK modulator placed at a start time, with clock error, carrier offset, start
//! phase and power ramps. The generators, the remodulation of a decoded burst for cancellation, and VDES use it.
//!
//! Standard: ITU-R M.1371-5 Annex 2, 2.4 (GMSK, BT 0.4, h = 0.5, 9600 bit/s)

use sdrkit_algo_sim::modulate::{Cpm, Ramp};
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The AIS bit rate.
pub const BIT_RATE: f64 = 9600.0;
/// The bandwidth-time product of the Gaussian filter.
pub const BT: f64 = 0.4;

/// How a burst is sent: when, how strong, where and how its bits are clocked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    /// The start of the first bit, seconds.
    pub start_s: f64,
    /// The amplitude.
    pub amplitude: f64,
    /// The carrier, Hz from 0.
    pub offset_hz: f64,
    /// The carrier phase at the start, radians.
    pub phase: f64,
    /// The bit clock error, ppm (the bits run fast by this much).
    pub clock_ppm: f64,
    /// The raised-cosine power ramp up and down, bits (0 for none).
    pub ramp_bits: f64,
}

impl Params {
    fn bit_rate(&self) -> f64 {
        BIT_RATE * (1.0 + self.clock_ppm * 1e-6)
    }

    /// The end of a burst of `bits` line bits, seconds.
    pub fn end_s(&self, bits: usize) -> f64 {
        self.start_s + bits as f64 / self.bit_rate()
    }
}

/// The GMSK modulator of AIS bursts at one sample rate, from the shared CPM modulator (BT 0.4, h 0.5). Each burst runs from
/// the start of its first bit to the end of its last, with the first and last bits held beyond the ends. It keeps its
/// modulator and buffer, so steady state allocates nothing.
#[derive(Clone, Debug, Default)]
pub struct Modulator {
    /// The modulator, with the samples per bit and the ramp in bits it was made for.
    cpm: Option<(f64, f64, Cpm)>,
    levels: Vec<f32>,
    burst: Vec<Complex32>,
}

impl Modulator {
    /// Adds the burst of line `levels` (0 or 1, one per bit) sent as `p` into `buf`, whose first sample is at `buf_t0`
    /// seconds, at `rate` samples per second. Samples outside `buf` are dropped.
    pub fn add(
        &mut self,
        levels: &[u8],
        p: &Params,
        buf: &mut [Complex32],
        buf_t0: f64,
        rate: f64,
    ) {
        let sps = rate / p.bit_rate();
        if levels.is_empty() || !(sps > 0.0 && sps.is_finite()) {
            return;
        }
        let ramp = p.ramp_bits.max(0.0);
        if self
            .cpm
            .as_ref()
            .is_none_or(|(s, r, _)| *s != sps || *r != ramp)
        {
            let m = Cpm::gmsk(BT, sps).and_then(|m| {
                let r = if ramp > 0.0 {
                    Ramp::RaisedCosine(ramp * sps)
                } else {
                    Ramp::None
                };
                m.with_held_edges().with_ramp(r)
            });
            // The rate is positive and finite, so only an absurd rate fails: such a burst is not added.
            let Ok(m) = m else {
                return;
            };
            self.cpm = Some((sps, ramp, m));
        }
        let Some((_, _, cpm)) = self.cpm.as_mut() else {
            return;
        };
        // The burst starts at sample `s0` of `buf`. Its first sample falls on the next whole sample.
        let s0 = (p.start_s - buf_t0) * rate;
        let first = s0.ceil();
        cpm.reset();
        if cpm.set_start(first - s0).is_err() {
            return;
        }
        self.levels.clear();
        self.levels
            .extend(levels.iter().map(|&l| if l == 1 { 1.0 } else { -1.0 }));
        self.burst.clear();
        cpm.push(&self.levels, &mut self.burst);
        cpm.flush(&mut self.burst);
        let w = TAU * p.offset_hz / rate;
        for (k, z) in self.burst.iter().enumerate() {
            let i = first + k as f64;
            if i < 0.0 {
                continue;
            }
            let Some(s) = buf.get_mut(i as usize) else {
                break;
            };
            // The carrier phase: `p.phase` at the start of the first bit.
            let ph = p.phase + w * (i - s0);
            let c = Complex32::new(ph.cos() as f32, ph.sin() as f32);
            *s += z * c * p.amplitude as f32;
        }
    }
}

/// Adds the burst of line `levels` sent as `p` into `buf`, as [`Modulator::add`] does, with a modulator of its own.
pub fn add(levels: &[u8], p: &Params, buf: &mut [Complex32], buf_t0: f64, rate: f64) {
    Modulator::default().add(levels, p, buf, buf_t0, rate);
}

/// One burst to modulate: its line levels (0 or 1) and how it is sent.
#[derive(Clone, Debug)]
pub struct Burst {
    /// The line levels, one per bit.
    pub levels: Vec<u8>,
    /// The start of the first bit, seconds.
    pub start_s: f64,
    /// The amplitude.
    pub amplitude: f64,
    /// The carrier, Hz from 0.
    pub offset_hz: f64,
    /// The carrier phase at the start, radians.
    pub phase: f64,
    /// The bit clock error, ppm (the bits run fast by this much).
    pub clock_ppm: f64,
    /// The power ramp up and down, bits (0 for none).
    pub ramp_bits: f64,
}

impl Burst {
    /// How the burst is sent.
    pub fn params(&self) -> Params {
        Params {
            start_s: self.start_s,
            amplitude: self.amplitude,
            offset_hz: self.offset_hz,
            phase: self.phase,
            clock_ppm: self.clock_ppm,
            ramp_bits: self.ramp_bits,
        }
    }

    /// The end, seconds.
    pub fn end_s(&self) -> f64 {
        self.params().end_s(self.levels.len())
    }

    /// Adds the burst into `buf`, whose first sample is at `buf_t0` seconds, at `rate` (see [`add`]).
    pub fn add_to(&self, buf: &mut [Complex32], buf_t0: f64, rate: f64) {
        add(&self.levels, &self.params(), buf, buf_t0, rate);
    }

    /// The burst alone at `rate`, from its start.
    pub fn samples(&self, rate: f64) -> Vec<Complex32> {
        let n = ((self.end_s() - self.start_s) * rate).ceil() as usize + 1;
        let mut v = vec![Complex32::default(); n];
        self.add_to(&mut v, self.start_s, rate);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bit_turns_by_a_quarter() {
        // A long run of ones turns the phase by pi/2 per bit.
        let b = Burst {
            levels: vec![1; 40],
            start_s: 0.0,
            amplitude: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            clock_ppm: 0.0,
            ramp_bits: 0.0,
        };
        let x = b.samples(48_000.0);
        let mut acc = 0.0;
        for i in 101..106 {
            acc += f64::from((x[i] * x[i - 1].conj()).arg());
        }
        assert!((acc - std::f64::consts::FRAC_PI_2).abs() < 0.02, "{acc}");
    }
}
