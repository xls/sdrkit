//! The coding of an ATSC 3.0 PLP: the constellations, the sizes of a FEC block, and the decoder of one block (demapping,
//! bit de-interleaving, LDPC, the outer code and descrambling).
//!
//! Standard: ATSC A/322:2023-03 clauses 5.2.3, 6.1 to 6.3 and Annex C.

use super::l1::{crc32, Plp, Scrambler};
use super::tables::{NUC16, NUC256, NUC64};
use sdrkit_algo_dsp::constellation::{PointSet, Shape};
use sdrkit_algo_fec::a322::BitInterleaver;
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::ldpc::tables::atsc3::Atsc3Table;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_core::Complex32;

/// The constellation of `bits` bits per cell (2, 4, 6 or 8) for code rate `rate_fifteenths / 15` (2 to 13): QPSK of table
/// C.1.1 or a non-uniform constellation of tables C.1.2 to C.1.7. `None` for another size or rate.
pub fn constellation(bits: usize, rate_fifteenths: u8) -> Option<Constellation> {
    let r = usize::from(rate_fifteenths)
        .checked_sub(2)
        .filter(|&r| r < 12)?;
    let quadrant: Vec<Complex32> = match bits {
        2 => vec![Complex32::new(1.0, 1.0)],
        4 => NUC16[r]
            .iter()
            .map(|&(a, b)| Complex32::new(a, b))
            .collect(),
        6 => NUC64[r]
            .iter()
            .map(|&(a, b)| Complex32::new(a, b))
            .collect(),
        8 => NUC256[r]
            .iter()
            .map(|&(a, b)| Complex32::new(a, b))
            .collect(),
        _ => return None,
    };
    // Clause 6.3.4.2: w, −conj(w), conj(w), −w.
    let points: Vec<Complex32> = quadrant
        .iter()
        .copied()
        .chain(quadrant.iter().map(|w| -w.conj()))
        .chain(quadrant.iter().map(|w| w.conj()))
        .chain(quadrant.iter().map(|w| -*w))
        .collect();
    let set = PointSet::new(points, Shape::Other).ok()?;
    Constellation::new(set).ok()
}

/// The outer code of a FEC block (table 9.17).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outer {
    /// The BCH code that corrects 12 errors.
    Bch,
    /// A CRC of 32 bits.
    Crc,
    /// None.
    None,
}

/// The coding of a PLP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlpCoding {
    /// The LDPC code word, 16200 or 64800 bits.
    pub ldpc_bits: usize,
    /// The code rate in fifteenths, 2 to 13.
    pub rate: u8,
    /// Bits per cell: 2, 4, 6 or 8.
    pub bits: usize,
    /// The outer code.
    pub outer: Outer,
}

impl PlpCoding {
    /// The coding that L1-Detail signals for `plp`, `None` when it is reserved or one the stack does not decode (1024 and
    /// 4096 points, a code and modulation without an interleaver).
    pub fn of(plp: &Plp) -> Option<PlpCoding> {
        let ldpc_bits = if plp.fec_type.is_multiple_of(2) {
            16200
        } else {
            64800
        };
        let outer = match plp.fec_type {
            0 | 1 => Outer::Bch,
            2 | 3 => Outer::Crc,
            4 | 5 => Outer::None,
            _ => return None,
        };
        let bits = match plp.modulation? {
            m @ 0..=3 => 2 * (usize::from(m) + 1),
            _ => return None,
        };
        let rate = plp.code_rate?.checked_add(2).filter(|&r| r <= 13)?;
        Some(PlpCoding {
            ldpc_bits,
            rate,
            bits,
            outer,
        })
    }
    /// The L1-Detail codes of this coding: FEC type, modulation and code rate.
    pub fn codes(&self) -> (u8, u8, u8) {
        let long = u8::from(self.ldpc_bits == 64800);
        let outer = match self.outer {
            Outer::Bch => 0,
            Outer::Crc => 2,
            Outer::None => 4,
        };
        (outer + long, (self.bits / 2 - 1) as u8, self.rate - 2)
    }
    /// The cells of one FEC block (table 6.14).
    pub fn cells(&self) -> usize {
        self.ldpc_bits / self.bits
    }
    /// The information bits of the LDPC code, `N_outer`.
    pub fn outer_bits(&self) -> usize {
        self.ldpc_bits * usize::from(self.rate) / 15
    }
    /// The bytes of a baseband packet, `K_payload / 8` (tables 6.1 and 6.2).
    pub fn packet_bytes(&self) -> usize {
        let parity = match (self.outer, self.ldpc_bits) {
            (Outer::Bch, 64800) => 192,
            (Outer::Bch, _) => 168,
            (Outer::Crc, _) => 32,
            (Outer::None, _) => 0,
        };
        (self.outer_bits() - parity) / 8
    }
    /// A short name: "QPSK 8/15, 64K LDPC".
    pub fn name(&self) -> String {
        let m = match self.bits {
            2 => "QPSK".to_owned(),
            b => format!("{}QAM", 1 << b),
        };
        format!(
            "{m} {}/15, {}K LDPC",
            self.rate,
            if self.ldpc_bits == 64800 { 64 } else { 16 }
        )
    }
}

/// What the decoding of one FEC block gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockResult {
    /// The baseband packet is good: the LDPC checks closed or the outer code accepted the word.
    pub good: bool,
    /// The LDPC iterations used.
    pub iterations: u32,
    /// The bits that the BCH code corrected after the LDPC decoder.
    pub outer_corrected: u32,
}

/// The LDPC check rule of the code of `coding`: the smallest magnitude times a scale, less an offset of 1 (0.25 in true
/// LLR at [`LLR_GAIN`]). The 16200-bit rate 2/15 code keeps a scale of 13/16 with no offset. The scale in sixteenths of
/// the others:
///
/// - 13 for the type A codes up to 16 points, 14 above;
/// - 15 for the type B codes of rates 6/15 and 7/15 at 64 points and more, and of rates 8/15 and 9/15 at 256 points and more;
/// - 14 for the other type B codes.
///
/// A scale alone corrects these codes too little: at [`LLR_GAIN`] it does not change the magnitudes 1 and 2, and at a
/// higher gain a scale low enough to decode leaves checks open. A rule one step stronger than the listed one stalls the
/// type B codes of rates 6/15 to 9/15, so the scale of a type B code must not go below the listed one.
///
/// Measured by `tests::ldpc_scale` (white noise, max-log demapping, [`LLR_GAIN`], 50 iterations, 4 to 256 points): the
/// rule of the lowest Es/N0 at a frame error rate of 0.1 among those with the fewest failures 0.3 dB above the rule in
/// use. `tests::ldpc_probe` compares other gains, iteration counts and sum-product decoding.
fn ldpc_rule(coding: PlpCoding) -> MinSum {
    let type_a = matches!(
        Atsc3Table::find(coding.ldpc_bits, coding.rate),
        Some(Atsc3Table::A(_))
    );
    let scale_q4 = match (coding.ldpc_bits, coding.rate) {
        (16200, 2) => return MinSum::with_scale(13),
        _ if type_a && coding.bits <= 4 => 13,
        _ if type_a => 14,
        (_, 6 | 7) if coding.bits >= 6 => 15,
        (_, 8 | 9) if coding.bits >= 8 => 15,
        _ => 14,
    };
    MinSum {
        scale_q4,
        offset: 1,
    }
}

/// The decoder of the FEC blocks of one PLP.
pub struct PlpDecoder {
    coding: PlpCoding,
    constellation: Constellation,
    interleaver: BitInterleaver,
    ldpc: LdpcDecoder,
    bch: Option<Bch>,
    soft: Vec<f32>,
    llr: Vec<i8>,
    word: Vec<i8>,
    hard: Vec<u8>,
}

/// The gain from true LLRs to the 8-bit LLRs of the LDPC decoder.
const LLR_GAIN: f32 = 4.0;

impl PlpDecoder {
    /// The decoder for `coding`, `None` when a part of it does not exist.
    pub fn new(coding: PlpCoding) -> Option<PlpDecoder> {
        let table = Atsc3Table::find(coding.ldpc_bits, coding.rate)?;
        let code = LdpcCode::from_atsc3(table).ok()?;
        let n = coding.ldpc_bits;
        Some(PlpDecoder {
            coding,
            constellation: constellation(coding.bits, coding.rate)?,
            interleaver: BitInterleaver::new(n, coding.rate, coding.bits).ok()?,
            ldpc: LdpcDecoder::new(&code, ldpc_rule(coding)),
            bch: match coding.outer {
                Outer::Bch => Some(table.bch().ok()?),
                _ => None,
            },
            soft: vec![0.0; n],
            llr: vec![0; n],
            word: vec![0; n],
            hard: vec![0; n],
        })
    }

    /// The coding.
    pub fn coding(&self) -> &PlpCoding {
        &self.coding
    }

    /// Demaps, quantises and de-interleaves one FEC block into `self.word`, the 8-bit LLRs of the LDPC code word. False
    /// when a length is wrong.
    fn soft_word(&mut self, cells: &[Complex32], weights: &[f32]) -> bool {
        if cells.len() != self.coding.cells() || weights.len() != cells.len() {
            return false;
        }
        if self
            .constellation
            .demap_block_weighted(cells, weights, Demap::MaxLog, &mut self.soft)
            .is_err()
        {
            return false;
        }
        for (q, &l) in self.llr.iter_mut().zip(&self.soft) {
            *q = (l * LLR_GAIN).round().clamp(-127.0, 127.0) as i8;
        }
        self.interleaver
            .deinterleave(&self.llr, &mut self.word)
            .is_ok()
    }

    /// Decodes one FEC block from its equalised cells `cells` and their weights `|H|^2 / sigma^2` (a weight of 0 erases a
    /// cell), with at most `max_iter` LDPC iterations. `packet` receives the descrambled baseband packet
    /// ([`PlpCoding::packet_bytes`] bytes), also when the block is not good.
    pub fn decode(
        &mut self,
        cells: &[Complex32],
        weights: &[f32],
        max_iter: u32,
        packet: &mut Vec<u8>,
    ) -> BlockResult {
        let c = self.coding;
        packet.clear();
        packet.resize(c.packet_bytes(), 0);
        let mut result = BlockResult {
            good: false,
            iterations: 0,
            outer_corrected: 0,
        };
        if !self.soft_word(cells, weights) {
            return result;
        }
        let ldpc = self.ldpc.decode(&self.word, max_iter, &mut self.hard);
        result.iterations = ldpc.as_ref().map_or(max_iter, |d| d.iterations);
        let k = c.outer_bits();
        let payload = c.packet_bytes() * 8;
        result.good = match (c.outer, &mut self.bch) {
            (Outer::Bch, Some(bch)) => match bch.decode(&mut self.hard[..k]) {
                Ok(d) => {
                    result.outer_corrected = d.corrected;
                    true
                }
                Err(_) => false,
            },
            (Outer::Crc, _) => {
                let crc = self.hard[payload..k]
                    .iter()
                    .fold(0u32, |v, &b| v << 1 | u32::from(b));
                crc == crc32(&self.hard[..payload])
            }
            _ => ldpc.is_ok(),
        };
        for (byte, bits) in packet
            .iter_mut()
            .zip(self.hard[..payload].as_chunks::<8>().0)
        {
            *byte = bits.iter().fold(0, |v, &b| v << 1 | b);
        }
        Scrambler::apply(packet);
        result
    }
}

/// The FEC blocks of one PLP from frame to frame: a block that begins in one frame and ends in the next is joined (A/322
/// clause 9.3.4, `L1D_plp_fec_block_start`).
pub struct PlpStream {
    decoder: PlpDecoder,
    cells: Vec<Complex32>,
    weights: Vec<f32>,
    packet: Vec<u8>,
}

impl PlpStream {
    /// The stream of the blocks that `decoder` decodes.
    pub fn new(decoder: PlpDecoder) -> PlpStream {
        PlpStream {
            decoder,
            cells: Vec::new(),
            weights: Vec::new(),
            packet: Vec::new(),
        }
    }

    /// The coding of the PLP.
    pub fn coding(&self) -> &PlpCoding {
        self.decoder.coding()
    }

    /// Forgets the cells of a block that has begun: the next frame does not follow the last one.
    pub fn restart(&mut self) {
        self.cells.clear();
        self.weights.clear();
    }

    /// Takes the cells of the PLP in one frame, with `block_start` the first cell of the first block that begins in the
    /// frame (`None`: none begins). `each` gets the result and the baseband packet of every block that ends in the frame. A
    /// block whose first part is missing is dropped without a call.
    pub fn frame(
        &mut self,
        cells: &[Complex32],
        weights: &[f32],
        block_start: Option<usize>,
        max_iter: u32,
        mut each: impl FnMut(BlockResult, &[u8]),
    ) {
        let block = self.decoder.coding().cells();
        let start = block_start.unwrap_or(cells.len()).min(cells.len());
        // The cells before the first new block end the block of the frame before, when they complete it.
        if !self.cells.is_empty() && self.cells.len() + start <= block {
            self.cells.extend_from_slice(&cells[..start]);
            self.weights.extend_from_slice(&weights[..start]);
        } else {
            self.restart();
        }
        if block_start.is_none() && self.cells.len() < block {
            return;
        }
        if self.cells.len() == block {
            let r = self
                .decoder
                .decode(&self.cells, &self.weights, max_iter, &mut self.packet);
            each(r, &self.packet);
        }
        self.restart();
        let mut at = start;
        while at + block <= cells.len() {
            let r = self.decoder.decode(
                &cells[at..at + block],
                &weights[at..at + block],
                max_iter,
                &mut self.packet,
            );
            each(r, &self.packet);
            at += block;
        }
        self.cells.extend_from_slice(&cells[at..]);
        self.weights.extend_from_slice(&weights[at..]);
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// Table C.1.1 and clause 6.3.4.2: the labels of QPSK and the quadrants of a non-uniform constellation.
    #[test]
    fn constellations_follow_annex_c() {
        let q = constellation(2, 8).unwrap();
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let want = [(s, s), (-s, s), (s, -s), (-s, -s)];
        for (p, w) in q.points().iter().zip(want) {
            assert!((p.re - w.0).abs() < 1e-6 && (p.im - w.1).abs() < 1e-6);
        }
        // Clause 6.3.4.2.1: 16 points at rate 6/15, the cell word 1100 is −w0 = −0.5115 − j1.2092.
        let c = constellation(4, 6).unwrap();
        let p = c.points()[12];
        assert!(
            (p.re + 0.5115).abs() < 2e-3 && (p.im + 1.2092).abs() < 2e-3,
            "{p}"
        );
        let p = c.points()[5];
        assert!(
            (p.re + 1.2092).abs() < 2e-3 && (p.im - 0.5115).abs() < 2e-3,
            "{p}"
        );
        for bits in [2, 4, 6, 8] {
            for rate in 2..=13 {
                let c = constellation(bits, rate).unwrap();
                assert_eq!(c.len(), 1 << bits);
                let power: f32 =
                    c.points().iter().map(|p| p.norm_sqr()).sum::<f32>() / c.len() as f32;
                assert!((power - 1.0).abs() < 1e-4, "{bits} {rate}: {power}");
            }
        }
        assert!(constellation(10, 8).is_none());
        assert!(constellation(4, 14).is_none());
        assert!(constellation(4, 1).is_none());
    }

    /// Tables 6.1, 6.2 and 6.14.
    #[test]
    fn block_sizes_follow_tables_6_1_6_2_and_6_14() {
        let c = |ldpc_bits, rate, bits, outer| PlpCoding {
            ldpc_bits,
            rate,
            bits,
            outer,
        };
        assert_eq!(c(64800, 8, 2, Outer::Bch).packet_bytes() * 8, 34368);
        assert_eq!(c(64800, 8, 2, Outer::Crc).packet_bytes() * 8, 34528);
        assert_eq!(c(64800, 13, 2, Outer::None).packet_bytes() * 8, 56160);
        assert_eq!(c(16200, 2, 2, Outer::Bch).packet_bytes() * 8, 1992);
        assert_eq!(c(16200, 13, 8, Outer::Crc).packet_bytes() * 8, 14008);
        assert_eq!(c(64800, 8, 2, Outer::Bch).cells(), 32400);
        assert_eq!(c(16200, 8, 8, Outer::Bch).cells(), 2025);
        assert_eq!(c(64800, 8, 2, Outer::Bch).name(), "QPSK 8/15, 64K LDPC");
        assert_eq!(c(16200, 5, 8, Outer::Bch).name(), "256QAM 5/15, 16K LDPC");
        let plp = Plp {
            fec_type: 1,
            modulation: Some(0),
            code_rate: Some(6),
            ..Plp::default()
        };
        let coding = PlpCoding::of(&plp).unwrap();
        assert_eq!(coding, c(64800, 8, 2, Outer::Bch));
        assert_eq!(coding.codes(), (1, 0, 6));
        assert_eq!(c(16200, 13, 8, Outer::None).codes(), (4, 3, 11));
        let refuse = |f: &dyn Fn(&mut Plp)| {
            let mut p = plp;
            f(&mut p);
            PlpCoding::of(&p)
        };
        assert!(refuse(&|p| p.fec_type = 6).is_none());
        assert!(refuse(&|p| p.modulation = Some(4)).is_none());
        assert!(refuse(&|p| p.code_rate = Some(12)).is_none());
        assert!(refuse(&|p| p.modulation = None).is_none());
    }

    /// The scales in sixteenths that the measurements compare.
    pub(in crate::atsc3) const SCALES: [u8; 5] = [12, 13, 14, 15, 16];
    /// The frame error rate at which the measurements read the Es/N0 of a scale.
    pub(in crate::atsc3) const TARGET: f64 = 0.1;
    const LDPC_ITER: u32 = super::super::components::LDPC_ITERATIONS;
    /// The check rules `(scale in sixteenths, offset)` that [`ldpc_scale`] compares.
    const RULES: [(u8, u8); 7] = [(12, 0), (13, 0), (14, 0), (13, 1), (14, 1), (15, 1), (16, 1)];
    const N: usize = RULES.len();

    /// The frame error rate of each rule of [`RULES`] at `esn0` dB over `frames` frames of `coding` (the same frames for
    /// every rule), and the frames whose information bits are right while checks stay open. Stops after `probe` frames
    /// when every rule failed every frame.
    fn point(
        coding: PlpCoding,
        decs: &mut [LdpcDecoder],
        front: &mut PlpDecoder,
        esn0: f64,
        frames: usize,
        probe: usize,
        seed: u64,
    ) -> ([f64; N], [usize; N]) {
        use sdrkit_algo_sim::{noise::Awgn, rng::Rng};
        let table = Atsc3Table::find(coding.ldpc_bits, coding.rate).unwrap();
        let code = LdpcCode::from_atsc3(table).unwrap();
        let k = coding.outer_bits();
        let n0 = 10f64.powf(-esn0 / 10.0);
        let weights = vec![(1.0 / n0) as f32; coding.cells()];
        let mut rng = Rng::new(seed);
        let mut noise = Awgn::with_power(n0, seed ^ 0x5DEE_CE66).unwrap();
        let (mut word, mut q, mut cells) = (
            vec![0u8; coding.ldpc_bits],
            vec![0u8; coding.ldpc_bits],
            Vec::new(),
        );
        let mut hard = vec![0u8; coding.ldpc_bits];
        let (mut failed, mut open) = ([0usize; N], [0usize; N]);
        let mut done = 0;
        while done < frames {
            let info: Vec<u8> = (0..k).map(|_| rng.bit()).collect();
            match table {
                Atsc3Table::A(t) => t.encode(&info, &mut word),
                Atsc3Table::B(_) => code.encode(&info, &mut word),
            }
            .unwrap();
            front.interleaver.interleave(&word, &mut q).unwrap();
            cells.clear();
            front.constellation.set().map_bits(&q, &mut cells).unwrap();
            noise.process(&mut cells);
            assert!(front.soft_word(&cells, &weights));
            for ((dec, f), o) in decs.iter_mut().zip(&mut failed).zip(&mut open) {
                let r = dec.decode(&front.word, LDPC_ITER, &mut hard);
                if hard[..k] != info[..] {
                    *f += 1;
                } else if r.is_err() {
                    *o += 1;
                }
            }
            done += 1;
            if done == probe && failed.iter().all(|&f| f == done) {
                break;
            }
        }
        (failed.map(|f| f as f64 / done as f64), open)
    }

    /// The Es/N0 at which the error rates `fer` (one per point from `first` in steps of `step`) first fall to [`TARGET`],
    /// interpolated in log scale; `frames` sets the floor of a rate of zero.
    pub(in crate::atsc3) fn crossing(fer: &[f64], first: f64, step: f64, frames: usize) -> f64 {
        let lg = |p: f64| p.max(0.5 / frames as f64).log10();
        let i = fer
            .iter()
            .position(|&p| p <= TARGET)
            .unwrap_or(fer.len() - 1);
        if i == 0 {
            return first;
        }
        let (a, b) = (lg(fer[i - 1]), lg(fer[i]));
        let t = if a > b {
            (a - lg(TARGET)) / (a - b)
        } else {
            1.0
        };
        first + step * (i as f64 - 1.0 + t)
    }

    /// One line of results for `coding`: the Es/N0 of [`TARGET`] per rule, the rule of [`ldpc_rule`], the best rule and its
    /// gain against it, the failures of each rule in ten times the frames 0.3 dB above the crossing of the rule of
    /// [`ldpc_rule`], and the waterfall rows. The best rule is the one of the lowest crossing among those with no more of
    /// these failures than the least.
    fn measure(coding: PlpCoding, frames: usize) -> String {
        let table = Atsc3Table::find(coding.ldpc_bits, coding.rate).unwrap();
        let code = LdpcCode::from_atsc3(table).unwrap();
        let rule = |&(scale_q4, offset): &(u8, u8)| MinSum { scale_q4, offset };
        let mut decs: Vec<LdpcDecoder> = RULES.iter().map(|r| LdpcDecoder::new(&code, rule(r))).collect();
        let used = RULES
            .iter()
            .position(|r| rule(r) == ldpc_rule(coding))
            .expect("the rule in use is one of RULES");
        let mut front = PlpDecoder::new(coding).unwrap();
        let rate = f64::from(coding.rate) / 15.0;
        let step = 0.05;
        let bound = 10.0 * (2f64.powf(coding.bits as f64 * rate) - 1.0).log10();
        let first = (bound / step).floor() * step;
        let seed =
            |i: usize| (coding.ldpc_bits as u64) << 32 ^ u64::from(coding.rate) << 16 ^ i as u64;
        let (mut fers, mut open, mut rows) = (Vec::new(), [0usize; N], String::new());
        let mut clear = 0;
        for i in 0..400 {
            let esn0 = first + step * i as f64;
            let (fer, o) = point(coding, &mut decs, &mut front, esn0, frames, 10, seed(i));
            for (a, b) in open.iter_mut().zip(o) {
                *a += b;
            }
            if fer.iter().any(|&p| p > 0.0 && p < 1.0) {
                rows += &format!(
                    "    {esn0:6.2}:{}\n",
                    fer.map(|p| format!(" {p:5.3}")).concat()
                );
            }
            fers.push(fer);
            clear = if fer.iter().all(|&p| p == 0.0) {
                clear + 1
            } else {
                0
            };
            if clear == 2 {
                break;
            }
        }
        let at: Vec<f64> = (0..N)
            .map(|s| {
                crossing(
                    &fers.iter().map(|f| f[s]).collect::<Vec<_>>(),
                    first,
                    step,
                    frames,
                )
            })
            .collect();
        // A rule that wins the waterfall can leave a floor: ten times the frames 0.3 dB above the crossing of the rule in use.
        let end = at[used] + 0.3;
        let (floor, _) = point(
            coding,
            &mut decs,
            &mut front,
            end,
            10 * frames,
            usize::MAX,
            seed(1000),
        );
        let least = floor.iter().copied().fold(f64::INFINITY, f64::min);
        let best = (0..N)
            .filter(|&s| floor[s] <= least)
            .min_by(|&a, &b| at[a].total_cmp(&at[b]))
            .unwrap();
        format!(
            "{:5} {:2}/15 bits {}: Es/N0 at {TARGET}:{} | used {:?} best {:?} gain {:+.2} dB | {end:.2} dB fails:{} | open right:{:?}\n{rows}",
            coding.ldpc_bits,
            coding.rate,
            coding.bits,
            at.iter().map(|a| format!(" {a:6.2}")).collect::<String>(),
            RULES[used],
            RULES[best],
            at[used] - at[best],
            floor.map(|p| format!(" {:.0}", p * 10.0 * frames as f64)).concat(),
            open,
        )
    }

    /// Measures the frame error rate of each PLP code against the LDPC check rules of [`RULES`] in white noise, through
    /// the decoder's own demapping and 8-bit LLRs, 50 iterations at most:
    /// `cargo test --release -p sdrkit-stack-tvterr --lib ldpc_scale -- --ignored --nocapture`.
    /// `ATSC3_SCALE_BITS` sets the bits per cell (default 2), `ATSC3_SCALE_CODES` a list such as `64800/2,16200/13`
    /// (default all), `ATSC3_SCALE_FRAMES` the frames of a point (default 100 at 64800 bits, 300 at 16200). A frame fails
    /// when an information bit is wrong; every rule decodes the same frames. [`ldpc_rule`] holds the outcome.
    #[test]
    #[ignore = "a measurement of minutes"]
    fn ldpc_scale() {
        let env = |name: &str| std::env::var(name).ok();
        let bits: usize = env("ATSC3_SCALE_BITS")
            .and_then(|b| b.parse().ok())
            .unwrap_or(2);
        let frames: Option<usize> = env("ATSC3_SCALE_FRAMES").and_then(|f| f.parse().ok());
        let codes: Vec<(usize, u8)> = match env("ATSC3_SCALE_CODES") {
            Some(list) => list
                .split(',')
                .map(|c| {
                    let (n, r) = c.split_once('/').expect("n/rate");
                    (n.trim().parse().unwrap(), r.trim().parse().unwrap())
                })
                .collect(),
            None => [64800, 16200]
                .iter()
                .flat_map(|&n| (2..=13).map(move |r| (n, r)))
                .collect(),
        };
        let next = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get());
        let mut lines: Vec<(usize, String)> = std::thread::scope(|s| {
            let jobs: Vec<_> = (0..workers)
                .map(|_| {
                    s.spawn(|| {
                        let mut out = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&(n, rate)) = codes.get(i) else {
                                break;
                            };
                            let coding = PlpCoding {
                                ldpc_bits: n,
                                rate,
                                bits,
                                outer: Outer::None,
                            };
                            let f = frames.unwrap_or(if n == 64800 { 100 } else { 300 });
                            let line = measure(coding, f);
                            eprint!("{line}");
                            out.push((i, line));
                        }
                        out
                    })
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
        });
        lines.sort();
        println!("Es/N0 in dB at a frame error rate of {TARGET}, rules (scale/16, offset) = {RULES:?}");
        for (_, l) in lines {
            print!("{}", l.lines().next().unwrap_or_default());
            println!();
        }
    }

    /// One decoder setup of [`ldpc_probe`]: the fields of a token such as `g4.s12.o0.i50`.
    #[derive(Clone, Copy, Debug)]
    struct Setup {
        gain: f32,
        rule: MinSum,
        iter: u32,
        exact: bool,
        plain: bool,
        float: bool,
    }

    impl Setup {
        /// Parses `g<gain>`, `s<scale/16>`, `o<offset>`, `i<iterations>`, `exact` (exact demapping), `plain` (the plain
        /// kernel) and `bp` (sum-product on unquantised LLRs), joined by dots.
        fn parse(token: &str) -> Setup {
            let mut s = Setup {
                gain: LLR_GAIN,
                rule: MinSum::with_scale(13),
                iter: LDPC_ITER,
                exact: false,
                plain: false,
                float: false,
            };
            for part in token.trim().split('.') {
                let num = || part[1..].parse::<f32>().expect("a number");
                match part {
                    "exact" => s.exact = true,
                    "plain" => s.plain = true,
                    "bp" => s.float = true,
                    _ => match part.as_bytes()[0] {
                        b'g' => s.gain = num(),
                        b's' => s.rule.scale_q4 = num() as u8,
                        b'o' => s.rule.offset = num() as u8,
                        b'i' => s.iter = num() as u32,
                        _ => panic!("unknown part {part}"),
                    },
                }
            }
            s
        }
    }

    /// Compares decoder setups on the same frames of one code in white noise: the frame error rate and the mean
    /// iterations of each setup per Es/N0.
    /// `cargo test --release -p sdrkit-stack-tvterr --lib ldpc_probe -- --ignored --nocapture`.
    /// `ATSC3_PROBE_CODE` is `n/rate/bits` (default `64800/2/2`), `ATSC3_PROBE_SETUPS` a comma list of [`Setup::parse`]
    /// tokens, `ATSC3_PROBE_ESN0` is `first:step:count` in dB, `ATSC3_PROBE_FRAMES` the frames of a point (default 40).
    #[test]
    #[ignore = "a measurement of minutes"]
    fn ldpc_probe() {
        use sdrkit_algo_fec::ldpc::{BpDecoder, LdpcKernel, BACKENDS};
        use sdrkit_algo_sim::{noise::Awgn, rng::Rng};
        let env = |name: &str, default: &str| std::env::var(name).unwrap_or(default.to_string());
        let code_text = env("ATSC3_PROBE_CODE", "64800/2/2");
        let nums: Vec<usize> = code_text.split('/').map(|v| v.parse().unwrap()).collect();
        let coding = PlpCoding {
            ldpc_bits: nums[0],
            rate: nums[1] as u8,
            bits: nums[2],
            outer: Outer::None,
        };
        let setups: Vec<Setup> = env("ATSC3_PROBE_SETUPS", "g4.s12.i50,bp.exact.i50")
            .split(',')
            .map(Setup::parse)
            .collect();
        let range: Vec<f64> = env("ATSC3_PROBE_ESN0", "-6:0.2:8")
            .split(':')
            .map(|v| v.parse().unwrap())
            .collect();
        let frames: usize = env("ATSC3_PROBE_FRAMES", "40").parse().unwrap();
        let table = Atsc3Table::find(coding.ldpc_bits, coding.rate).unwrap();
        let code = LdpcCode::from_atsc3(table).unwrap();
        let (n, k) = (coding.ldpc_bits, coding.outer_bits());
        let points: Vec<f64> = (0..range[2] as usize)
            .map(|i| range[0] + range[1] * i as f64)
            .collect();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let workers = std::thread::available_parallelism().map_or(4, |w| w.get());
        let mut lines: Vec<(usize, String)> = std::thread::scope(|s| {
            let jobs: Vec<_> = (0..workers.min(points.len()))
                .map(|_| {
                    s.spawn(|| {
                        let front = PlpDecoder::new(coding).unwrap();
                        let mut kernels: Vec<Box<dyn LdpcKernel>> = setups
                            .iter()
                            .map(|u| {
                                let plain = BACKENDS.plain().decoder(&code, u.rule).unwrap();
                                if u.plain {
                                    plain
                                } else {
                                    BACKENDS.build(|b| b.decoder(&code, u.rule)).unwrap_or(plain)
                                }
                            })
                            .collect();
                        let mut bp = BpDecoder::new(&code);
                        let mut out = Vec::new();
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(&esn0) = points.get(i) else {
                                break;
                            };
                            let n0 = 10f64.powf(-esn0 / 10.0);
                            let weights = vec![(1.0 / n0) as f32; coding.cells()];
                            let mut rng = Rng::new(i as u64 + 77);
                            let mut noise = Awgn::with_power(n0, i as u64 ^ 0x5DEE_CE66).unwrap();
                            let (mut word, mut q, mut cells) = (vec![0u8; n], vec![0u8; n], Vec::new());
                            let mut soft = [vec![0f32; n], vec![0f32; n]];
                            let (mut li, mut wf, mut wi) = (vec![0i8; n], vec![0f32; n], vec![0i8; n]);
                            let mut hard = vec![0u8; n];
                            let mut failed = vec![0usize; setups.len()];
                            let mut iters = vec![0u64; setups.len()];
                            for _ in 0..frames {
                                let info: Vec<u8> = (0..k).map(|_| rng.bit()).collect();
                                match table {
                                    Atsc3Table::A(t) => t.encode(&info, &mut word),
                                    Atsc3Table::B(_) => code.encode(&info, &mut word),
                                }
                                .unwrap();
                                front.interleaver.interleave(&word, &mut q).unwrap();
                                cells.clear();
                                front.constellation.set().map_bits(&q, &mut cells).unwrap();
                                noise.process(&mut cells);
                                for (method, s) in [Demap::MaxLog, Demap::Exact].into_iter().zip(&mut soft) {
                                    front
                                        .constellation
                                        .demap_block_weighted(&cells, &weights, method, s)
                                        .unwrap();
                                }
                                for (j, u) in setups.iter().enumerate() {
                                    let s = &soft[usize::from(u.exact)];
                                    let used = if u.float {
                                        front.interleaver.deinterleave(s, &mut wf).unwrap();
                                        let _ = bp.decode(&wf, u.iter, &mut hard);
                                        bp.last_iterations()
                                    } else {
                                        for (o, &l) in li.iter_mut().zip(s) {
                                            *o = (l * u.gain).round().clamp(-127.0, 127.0) as i8;
                                        }
                                        front.interleaver.deinterleave(&li, &mut wi).unwrap();
                                        kernels[j]
                                            .decode(&code, &wi, u.iter, &mut hard)
                                            .map_or(u.iter, |d| d.iterations)
                                    };
                                    iters[j] += u64::from(used);
                                    failed[j] += usize::from(hard[..k] != info[..]);
                                }
                            }
                            let cols: String = failed
                                .iter()
                                .zip(&iters)
                                .map(|(&f, &it)| {
                                    format!(" {:5.3}/{:5.1}", f as f64 / frames as f64, it as f64 / frames as f64)
                                })
                                .collect();
                            out.push((i, format!("{esn0:7.2}:{cols}")));
                        }
                        out
                    })
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
        });
        lines.sort();
        println!("{code_text}: frame error rate / mean iterations of {}", env("ATSC3_PROBE_SETUPS", "g4.s12.i50,bp.exact.i50"));
        for (_, l) in lines {
            println!("{l}");
        }
    }
}
