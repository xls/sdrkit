//! Differential detection: DBPSK, DQPSK, pi/4-DQPSK, D8PSK and the differentially coded quadrant of DVB-C QAM.
//!
//! A differential PSK sends its bits as the **phase step** between consecutive symbols. [`Differential`] takes the step's
//! constellation (a [`PointSet`] whose `points()[label]` is `e^(j step)` for that label), forms `d = z[k] conj(z[k-1])`, and decides
//! or demaps `d` on that constellation. No absolute carrier phase is needed, only a frequency offset small against the symbol rate.
//!
//! | Builder | Steps | Used by |
//! |---|---|---|
//! | [`Differential::dbpsk`] | 0: 0, 1: pi | IEEE 802.11b DBPSK (clause 16.4.6.5), Sigfox, PSK31 |
//! | [`Differential::dqpsk`] | 00: 0, 01: pi/2, 11: pi, 10: 3 pi/2 | IEEE 802.11b DQPSK (table 16-8) |
//! | [`Differential::pi4_dqpsk_dab`] | 00: pi/4, 01: -pi/4, 10: 3 pi/4, 11: -3 pi/4 | DAB (EN 300 401 clause 14.5) |
//! | [`Differential::pi4_dqpsk_tetra`] | 00: pi/4, 01: 3 pi/4, 11: -3 pi/4, 10: -pi/4 | TETRA (EN 300 392-2 clause 5.5.2), P25 phase 2 |
//! | [`Differential::d8psk`] | Gray: label `gray(j)` steps `j pi / 4` | VDL Mode 2 |
//!
//! Soft values use the two-symbol metric: `d / |z[k-1]|` has the step's phase and, at a useful SNR, about twice the noise of one
//! symbol, so [`Differential::demap`] demaps it with twice the given noise variance.
//!
//! ```
//! use sdrkit_algo_modem::differential::Differential;
//! use sdrkit_algo_modem::Complex32;
//! let mut d = Differential::dqpsk();
//! let mut bits = [0u8; 2];
//! d.decide_bits(Complex32::new(1.0, 0.0), &mut bits).unwrap(); // the reference symbol
//! d.decide_bits(Complex32::new(0.0, 1.0), &mut bits).unwrap(); // a step of pi/2
//! assert_eq!(bits, [0, 1]);
//! ```

use crate::{Complex32, Constellation, Demap, ModemError, PointSet};
use std::f64::consts::PI;

/// A differential detector over a step constellation (module documentation). It keeps the previous symbol between calls.
#[derive(Clone, Debug)]
pub struct Differential {
    steps: Constellation,
    prev: Complex32,
}

impl Differential {
    /// A detector whose phase steps are the points of `steps` (each on the unit circle). Errors when `steps` cannot be demapped.
    pub fn new(steps: PointSet) -> Result<Self, ModemError> {
        Ok(Differential {
            steps: Constellation::new(steps)?,
            prev: Complex32::new(1.0, 0.0),
        })
    }

    fn from_angles(angles: &[f64]) -> Self {
        let p = angles
            .iter()
            .map(|a| Complex32::new(a.cos() as f32, a.sin() as f32))
            .collect();
        // The angle tables below are fixed and valid.
        let set = PointSet::new(
            p,
            sdrkit_algo_dsp::constellation::Shape::Psk {
                m: angles.len() as u32,
            },
        )
        .unwrap_or_else(|_| PointSet::bpsk());
        Differential::new(set).unwrap_or_else(|_| unreachable!("fixed PSK tables are valid"))
    }

    /// DBPSK: 0 keeps the phase, 1 turns it by pi.
    pub fn dbpsk() -> Self {
        Self::from_angles(&[0.0, PI])
    }

    /// DQPSK as IEEE 802.11b sends it: 00 no change, 01 +pi/2, 11 +pi, 10 +3 pi/2.
    pub fn dqpsk() -> Self {
        Self::from_angles(&[0.0, PI / 2.0, 1.5 * PI, PI])
    }

    /// pi/4-DQPSK with the DAB map: `(1 - 2 b0) + j (1 - 2 b1)` over sqrt 2.
    pub fn pi4_dqpsk_dab() -> Self {
        Self::from_angles(&[PI / 4.0, -PI / 4.0, 0.75 * PI, -0.75 * PI])
    }

    /// pi/4-DQPSK with the TETRA map: 00 +pi/4, 01 +3 pi/4, 10 -pi/4, 11 -3 pi/4.
    pub fn pi4_dqpsk_tetra() -> Self {
        Self::from_angles(&[PI / 4.0, 0.75 * PI, -PI / 4.0, -0.75 * PI])
    }

    /// D8PSK with Gray steps: label `gray(j)` turns the phase by `j pi / 4`.
    pub fn d8psk() -> Self {
        let mut a = [0.0; 8];
        for j in 0..8u32 {
            a[sdrkit_util_bits::gray::gray(j) as usize] = j as f64 * PI / 4.0;
        }
        Self::from_angles(&a)
    }

    /// Bits per symbol.
    pub fn bits(&self) -> usize {
        self.steps.bits()
    }

    /// The step constellation.
    pub fn steps(&self) -> &Constellation {
        &self.steps
    }

    /// Sets the reference symbol (the one before the next call), for example a known preamble symbol.
    pub fn set_reference(&mut self, z: Complex32) {
        self.prev = z;
    }

    /// Forgets the previous symbol (the reference becomes `1 + 0j`).
    pub fn reset(&mut self) {
        self.prev = Complex32::new(1.0, 0.0);
    }

    /// `z conj(previous) / |previous|`, the step estimate, and moves on.
    #[inline]
    pub fn step(&mut self, z: Complex32) -> Complex32 {
        let p = self.prev;
        self.prev = z;
        let n = p.norm();
        if n > 0.0 && n.is_finite() {
            z * p.conj() / n
        } else {
            Complex32::default()
        }
    }

    /// The label of the step from the previous symbol to `z`.
    #[inline]
    pub fn decide(&mut self, z: Complex32) -> u16 {
        let d = self.step(z);
        self.steps.decide(d)
    }

    /// The hard bits of the step to `z` into `out[0..bits]`.
    pub fn decide_bits(&mut self, z: Complex32, out: &mut [u8]) -> Result<(), ModemError> {
        let d = self.step(z);
        self.steps.decide_bits(d, out)
    }

    /// The LLRs of the step to `z` into `out[0..bits]`, for symbols with complex noise variance `noise_var` (the step is demapped
    /// with `2 noise_var`).
    pub fn demap(
        &mut self,
        z: Complex32,
        noise_var: f32,
        method: Demap,
        out: &mut [f32],
    ) -> Result<(), ModemError> {
        let d = self.step(z);
        self.steps.demap(d, 2.0 * noise_var, method, out)
    }
}

/// The DVB-C differential quadrant code (EN 300 429 clause 8): the two most significant bits `Ik Qk` of each symbol are sent as a
/// quarter-turn step of the quadrant (00: 0, 10: +1, 11: +2, 01: +3 quarter turns), so the receiver needs no absolute phase. Labels
/// from a slicer over `PointSet::dvb_c` go in; labels with `Ik Qk` decoded come out.
#[derive(Clone, Debug, Default)]
pub struct QuadrantDecoder {
    prev: u16,
}

impl QuadrantDecoder {
    /// A decoder with the previous quadrant 0.
    pub fn new() -> Self {
        QuadrantDecoder { prev: 0 }
    }

    /// Takes the label a slicer decided (`bits` bits, the top two the absolute quadrant code) and returns the label with the top
    /// two bits replaced by the decoded `Ik Qk`.
    pub fn decode(&mut self, label: u16, bits: u32) -> u16 {
        // Quadrant number of each code: 00 -> 0, 10 -> 1, 11 -> 2, 01 -> 3.
        const QUAD: [u16; 4] = [0, 3, 1, 2];
        const CODE: [u16; 4] = [0b00, 0b10, 0b11, 0b01];
        let top = (label >> (bits - 2)) & 3;
        let q = QUAD[top as usize];
        let step = (q + 4 - self.prev) & 3;
        self.prev = q;
        (CODE[step as usize] << (bits - 2)) | (label & ((1 << (bits - 2)) - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(d: &mut Differential, steps: &[usize], start: f64) -> Vec<u16> {
        // Turn a symbol by each label's step and check the decision.
        let pts = d.steps().points().to_vec();
        let mut z = Complex32::from_polar(0.8, start as f32);
        d.set_reference(z);
        steps
            .iter()
            .map(|&l| {
                z *= pts[l];
                d.decide(z)
            })
            .collect()
    }

    #[test]
    fn standard_step_tables() {
        let ang = |d: &Differential, l: usize| {
            let p = d.steps().points()[l];
            (p.im as f64).atan2(p.re as f64) / PI
        };
        let close = |a: f64, b: f64| ((a - b + 3.0).rem_euclid(2.0) - 1.0).abs() < 1e-6;
        let d = Differential::dqpsk();
        assert!(
            close(ang(&d, 0b00), 0.0)
                && close(ang(&d, 0b01), 0.5)
                && close(ang(&d, 0b11), 1.0)
                && close(ang(&d, 0b10), 1.5)
        );
        let t = Differential::pi4_dqpsk_tetra();
        assert!(
            close(ang(&t, 0b11), -0.75)
                && close(ang(&t, 0b01), 0.75)
                && close(ang(&t, 0b10), -0.25)
        );
        let a = Differential::pi4_dqpsk_dab();
        assert!(close(ang(&a, 0b01), -0.25) && close(ang(&a, 0b10), 0.75));
        let e = Differential::d8psk();
        assert!(close(ang(&e, 0b011), 0.5) && close(ang(&e, 0b100), 1.75));
        for mut d in [
            Differential::dbpsk(),
            Differential::dqpsk(),
            Differential::pi4_dqpsk_dab(),
            Differential::pi4_dqpsk_tetra(),
            Differential::d8psk(),
        ] {
            let m = d.steps().len();
            let labels: Vec<usize> = (0..64).map(|k| (k * 5 + k / 3) % m).collect();
            let got = run(&mut d, &labels, 1.234);
            assert_eq!(got.iter().map(|&g| g as usize).collect::<Vec<_>>(), labels);
        }
    }

    #[test]
    fn soft_values_have_the_right_sign() {
        let mut d = Differential::pi4_dqpsk_tetra();
        let pts = d.steps().points().to_vec();
        let mut z = Complex32::new(1.0, 0.0);
        d.set_reference(z);
        let mut l = [0f32; 2];
        for lab in [0usize, 1, 2, 3, 2, 1] {
            z *= pts[lab];
            d.demap(z, 0.05, Demap::Exact, &mut l).unwrap();
            assert_eq!((l[0] < 0.0) as usize * 2 + (l[1] < 0.0) as usize, lab);
        }
    }

    #[test]
    fn dvb_c_quadrants_decode_after_any_turn() {
        let set = PointSet::dvb_c(64).unwrap();
        let c = Constellation::new(set.clone()).unwrap();
        // Encode: Ik Qk choose the step; the transmitted quadrant accumulates.
        let tuples: Vec<u16> = (0..50u16).map(|k| (k * 37 + 5) % 64).collect();
        const TURN: [u16; 4] = [0, 3, 1, 2];
        const CODE: [u16; 4] = [0b00, 0b10, 0b11, 0b01];
        let mut quad = 0u16;
        let mut sent = Vec::new();
        for &t in &tuples {
            quad = (quad + TURN[(t >> 4) as usize]) & 3;
            sent.push(set.points()[((CODE[quad as usize] << 4) | (t & 15)) as usize]);
        }
        // Received turned by a half turn: every tuple after the first is still right.
        let mut dec = QuadrantDecoder::new();
        let got: Vec<u16> = sent.iter().map(|&z| dec.decode(c.decide(-z), 6)).collect();
        assert_eq!(&got[1..], &tuples[1..]);
    }
}
