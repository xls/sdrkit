//! The DVB-C orders and the mapping of bytes to symbols and back, on the constellations of `sdrkit-algo-dsp` (`PointSet::dvb_c`), the
//! slicer of `sdrkit-algo-modem` and its differential quadrant decoder.
//!
//! Standard: ETSI EN 300 429 clause 8.

use sdrkit_algo_dsp::constellation::PointSet;
use sdrkit_algo_modem::differential::QuadrantDecoder;
use sdrkit_core::Complex32;

sdrkit_core::choice_enum! {
    /// A DVB-C QAM order.
    pub enum Qam {
        /// 16-QAM.
        Q16 = ("16", "16-QAM"),
        /// 32-QAM.
        Q32 = ("32", "32-QAM"),
        /// 64-QAM.
        Q64 = ("64", "64-QAM"),
        /// 128-QAM.
        Q128 = ("128", "128-QAM"),
        /// 256-QAM.
        Q256 = ("256", "256-QAM"),
    }
}

impl Qam {
    /// The order the trial tries them in when the order is automatic.
    pub const TRIAL: [Qam; 5] = [Qam::Q256, Qam::Q64, Qam::Q16, Qam::Q128, Qam::Q32];
    /// Bits per symbol.
    pub fn bits(self) -> usize {
        match self {
            Qam::Q16 => 4,
            Qam::Q32 => 5,
            Qam::Q64 => 6,
            Qam::Q128 => 7,
            Qam::Q256 => 8,
        }
    }
    /// Points.
    pub fn order(self) -> u32 {
        1 << self.bits()
    }
    /// "64-QAM".
    pub fn name(self) -> &'static str {
        match self {
            Qam::Q16 => "16-QAM",
            Qam::Q32 => "32-QAM",
            Qam::Q64 => "64-QAM",
            Qam::Q128 => "128-QAM",
            Qam::Q256 => "256-QAM",
        }
    }
    /// From 16, 32, 64, 128, 256.
    pub fn from_order(m: u32) -> Option<Qam> {
        [Qam::Q16, Qam::Q32, Qam::Q64, Qam::Q128, Qam::Q256]
            .into_iter()
            .find(|q| q.order() == m)
    }
}

/// The quadrant code `Ik Qk` of each quarter turn (EN 300 429 clause 8: 00 first, 10 second, 11 third, 01 fourth).
const CODE: [u32; 4] = [0b00, 0b10, 0b11, 0b01];
/// The quarter turns of each quadrant code.
const TURNS: [u32; 4] = [0, 3, 1, 2];

/// A DVB-C constellation: the points of `PointSet::dvb_c` and their slicer.
#[derive(Clone, Debug)]
pub struct Constellation {
    /// The order.
    pub qam: Qam,
    slicer: sdrkit_algo_modem::Constellation,
    /// Half the step of the grid that the points are on.
    margin: f32,
}

impl Constellation {
    /// The constellation of `qam`.
    pub fn new(qam: Qam) -> Constellation {
        // The five DVB-C orders are always valid point sets.
        let set = PointSet::dvb_c(qam.order() as usize)
            .unwrap_or_else(|e| unreachable!("DVB-C {qam:?}: {e}"));
        let slicer = sdrkit_algo_modem::Constellation::new(set)
            .unwrap_or_else(|e| unreachable!("DVB-C {qam:?}: {e}"));
        let step = slicer
            .points()
            .iter()
            .map(|p| p.re.abs())
            .fold(f32::INFINITY, f32::min)
            * 2.0;
        Constellation {
            qam,
            slicer,
            margin: 0.5 * step,
        }
    }
    /// The constant modulus radius `E|a|^4 / E|a|^2` at unit mean power (`sdrkit_algo_modem::equalizer::cma_radius`).
    pub fn cma_radius(&self) -> f32 {
        sdrkit_algo_modem::equalizer::cma_radius(&self.slicer)
    }
    /// The point of quadrant `quad` (quarter turns) and low bits `q`.
    pub fn point(&self, quad: u32, q: u32) -> Complex32 {
        let b = self.qam.bits();
        self.slicer.points()[((CODE[quad as usize & 3] << (b - 2)) | q) as usize]
    }
    /// The nearest point: its label and value.
    #[inline]
    pub fn decide(&self, z: Complex32) -> (u16, Complex32) {
        let l = self.slicer.decide(z);
        (l, self.slicer.points()[l as usize])
    }
    /// A quick test that `d`, a point of the set, is the point nearest `z`: `z` is in the grid square of `d`. A `z` outside the
    /// square can still be nearest to a point at the edge of the set, so `false` asks for [`Constellation::decide`].
    #[inline]
    pub fn near(&self, z: Complex32, d: Complex32) -> bool {
        let e = z - d;
        e.re.abs() < self.margin && e.im.abs() < self.margin
    }
}

/// Bytes to symbols with the differential quadrant code (EN 300 429 clause 8).
#[derive(Clone, Debug)]
pub struct Mapper {
    c: Constellation,
    quad: u32,
    acc: u32,
    nacc: usize,
}

impl Mapper {
    /// For `qam`.
    pub fn new(qam: Qam) -> Mapper {
        Mapper {
            c: Constellation::new(qam),
            quad: 0,
            acc: 0,
            nacc: 0,
        }
    }
    /// Appends the symbols of `bytes` (bits most significant first). Bits left over wait for the next call.
    pub fn push(&mut self, bytes: &[u8], out: &mut Vec<Complex32>) {
        let m = self.c.qam.bits();
        for &b in bytes {
            self.acc = (self.acc << 8) | b as u32;
            self.nacc += 8;
            while self.nacc >= m {
                let t = (self.acc >> (self.nacc - m)) & ((1 << m) - 1);
                self.nacc -= m;
                self.quad = (self.quad + TURNS[(t >> (m - 2)) as usize]) & 3;
                out.push(self.c.point(self.quad, t & ((1 << (m - 2)) - 1)));
            }
            self.acc &= (1 << self.nacc) - 1;
        }
    }
}

/// Symbols back to tuple bits: the slicer, then the quadrant decoder of `sdrkit-algo-modem`.
#[derive(Clone, Debug)]
pub struct Demapper {
    /// The constellation.
    pub c: Constellation,
    quad: QuadrantDecoder,
}

impl Demapper {
    /// For `qam`.
    pub fn new(qam: Qam) -> Demapper {
        Demapper {
            c: Constellation::new(qam),
            quad: QuadrantDecoder::new(),
        }
    }
    /// Decides `z` (conjugated first when `inverted`) and appends its bits (most significant first) to `bits`.
    #[inline]
    pub fn push(&mut self, z: Complex32, inverted: bool, bits: &mut Vec<u8>) {
        let z = if inverted { z.conj() } else { z };
        self.push_label(self.c.slicer.decide(z), bits);
    }
    /// Appends the bits of a symbol that was decided as the point of `label`.
    #[inline]
    pub fn push_label(&mut self, label: u16, bits: &mut Vec<u8>) {
        let m = self.c.qam.bits();
        let t = self.quad.decode(label, m as u32) as u32;
        for i in (0..m).rev() {
            bits.push(((t >> i) & 1) as u8);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_symbols_and_a_half_turn_give_the_bytes_back() {
        for qam in [Qam::Q16, Qam::Q32, Qam::Q64, Qam::Q128, Qam::Q256] {
            let bytes: Vec<u8> = (0..300).map(|i| (i * 37 + 11) as u8).collect();
            let mut m = Mapper::new(qam);
            let mut syms = Vec::new();
            m.push(&bytes, &mut syms);
            let mut d = Demapper::new(qam);
            let mut bits = Vec::new();
            for s in &syms {
                d.push(-*s, false, &mut bits);
            }
            let got: Vec<u8> = bits
                .chunks(8)
                .filter(|c| c.len() == 8)
                .map(|c| c.iter().fold(0, |a, &b| (a << 1) | b))
                .collect();
            // The first symbol's quadrant is relative to an unknown start.
            let skip = 1;
            assert_eq!(
                &got[skip..got.len() - 1],
                &bytes[skip..got.len() - 1],
                "{qam:?}"
            );
        }
    }
}
