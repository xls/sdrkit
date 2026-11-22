//! Bit-interleaved coding and modulation of one PLP: BCH and LDPC (from `sdrkit-algo-fec`), the bit interleaver and demultiplexer,
//! the constellation map with rotation and cyclic Q delay. The encoder is the generator's path, the decoder the receiver's.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 6.1, 6.2 and 6.3.

use crate::error::{Rule, T2Error};
use crate::interleave::bits_to_cells;
use crate::params::{k_bch, ldpc_table, outer_code_key, CodeRate, FecFrame, Modulation};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::bch::{codes, Bch};
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;

/// The coding and modulation of one PLP (the L1-post fields PLP_COD, PLP_MOD, PLP_ROTATION and PLP_FEC_TYPE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Coding {
    /// FEC frame length.
    pub frame: FecFrame,
    /// Code rate.
    pub rate: CodeRate,
    /// Constellation.
    pub modulation: Modulation,
    /// Rotated constellation with cyclic Q delay.
    pub rotated: bool,
}

impl Coding {
    /// K_bch, the BBFRAME length in bits.
    pub fn k_bch(&self) -> usize {
        k_bch(self.frame, self.rate).unwrap_or(0)
    }

    /// N_cells, the cells of one FEC block.
    pub fn cells(&self) -> usize {
        self.frame.n_ldpc() / self.modulation.bits()
    }
}

/// The constellation of tables 14(a) to 14(i) with the normalisation of table 15: y_0 is the first (most significant) label bit,
/// even bits drive the real part and odd bits the imaginary part, 0 is the positive side.
pub fn point_set(m: Modulation) -> PointSet {
    match m {
        Modulation::Bpsk => PointSet::bpsk(),
        Modulation::Qpsk => PointSet::qam(4, QamMap::DVB).unwrap_or_else(|_| PointSet::bpsk()),
        Modulation::Qam16 => PointSet::qam(16, QamMap::DVB).unwrap_or_else(|_| PointSet::bpsk()),
        Modulation::Qam64 => PointSet::qam(64, QamMap::DVB).unwrap_or_else(|_| PointSet::bpsk()),
        Modulation::Qam256 => PointSet::qam(256, QamMap::DVB).unwrap_or_else(|_| PointSet::bpsk()),
    }
}

/// The rotation phasor R_RQD = e^(j 2 pi phi / 360) of table 16.
pub fn rotation(m: Modulation) -> Complex32 {
    let a = m.rotation_degrees().to_radians();
    Complex32::new(a.cos() as f32, a.sin() as f32)
}

fn codes_for(c: &Coding) -> Result<(Bch, LdpcCode), T2Error> {
    let key = outer_code_key(c.frame, c.rate).ok_or(Rule::NoCode)?;
    let code = codes::by_key(key).ok_or(Rule::NoCode)?;
    let bch = Bch::from_code(code)?;
    let table = ldpc_table(c.frame, c.rate).ok_or(Rule::NoCode)?;
    let ldpc = LdpcCode::from_dvb(table)?;
    Ok((bch, ldpc))
}

/// The transmit chain of one PLP: BBFRAME bits in, the cells of one FEC block out.
#[derive(Clone, Debug)]
pub struct Encoder {
    coding: Coding,
    bch: Bch,
    ldpc: LdpcCode,
    perm: Permutation,
    points: Vec<Complex32>,
    word: Vec<u8>,
    code: Vec<u8>,
    cellbits: Vec<u8>,
    plain: Vec<Complex32>,
}

impl Encoder {
    /// The encoder for a coding. Errors for a coding the standard does not define.
    pub fn new(coding: Coding) -> Result<Encoder, T2Error> {
        let (bch, ldpc) = codes_for(&coding)?;
        let perm = bits_to_cells(coding.frame, coding.modulation, coding.rate)?;
        let n = coding.frame.n_ldpc();
        Ok(Encoder {
            coding,
            word: vec![0; bch.n()],
            code: vec![0; n],
            cellbits: vec![0; n],
            plain: vec![Complex32::default(); coding.cells()],
            points: point_set(coding.modulation).points().to_vec(),
            bch,
            ldpc,
            perm,
        })
    }

    /// The coding.
    pub fn coding(&self) -> &Coding {
        &self.coding
    }

    /// Encodes one BBFRAME (`k_bch` bits, the first bit of the BBHEADER first) into the N_cells cells of one FEC block, rotated
    /// and Q-delayed when the coding says so. Errors when a slice has the wrong length.
    pub fn encode(&mut self, bbframe: &[u8], cells: &mut [Complex32]) -> Result<(), T2Error> {
        let k = self.bch.k();
        if bbframe.len() != k || cells.len() != self.coding.cells() {
            return Err(if bbframe.len() != k {
                T2Error::Length {
                    expected: k,
                    got: bbframe.len(),
                }
            } else {
                T2Error::Length {
                    expected: self.coding.cells(),
                    got: cells.len(),
                }
            });
        }
        self.word[..k].copy_from_slice(bbframe);
        self.bch.encode(&mut self.word)?;
        self.ldpc.encode(&self.word, &mut self.code)?;
        self.perm.apply(&self.code, &mut self.cellbits)?;
        let eta = self.coding.modulation.bits();
        for (f, bits) in self.plain.iter_mut().zip(self.cellbits.chunks_exact(eta)) {
            let label = bits.iter().fold(0usize, |a, &b| (a << 1) | b as usize);
            *f = self.points[label];
        }
        map_rotated(
            &self.plain,
            self.coding
                .rotated
                .then(|| rotation(self.coding.modulation)),
            cells,
        );
        Ok(())
    }
}

/// Rotation and cyclic Q delay over one FEC block (clause 6.3): g_q = Re(R f_q) + j Im(R f_(q-1)), g_0 taking Im from the last
/// cell. Without a rotation the cells are copied.
pub fn map_rotated(f: &[Complex32], rot: Option<Complex32>, g: &mut [Complex32]) {
    let Some(r) = rot else {
        g[..f.len()].copy_from_slice(f);
        return;
    };
    let n = f.len();
    for q in 0..n {
        let re = (r * f[q]).re;
        let im = (r * f[(q + n - 1) % n]).im;
        g[q] = Complex32::new(re, im);
    }
}

/// The result of decoding one FEC block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockResult {
    /// Every LDPC parity equation holds.
    pub ldpc_ok: bool,
    /// LDPC iterations used.
    pub iterations: u32,
    /// The BCH decoder accepted the word.
    pub bch_ok: bool,
    /// Bits the BCH decoder corrected.
    pub bch_corrected: u32,
}

/// The receive chain of one PLP: the cells of one FEC block with their weights in, BBFRAME bits out.
pub struct Decoder {
    coding: Coding,
    bch: Bch,
    ldpc: LdpcDecoder,
    perm: Permutation,
    cons: Constellation,
    /// Turns a received cell back by the rotation of the constellation. 1 without rotation.
    turn: Complex32,
    max_iter: u32,
    quant: Quantizer,
    z: Vec<Complex32>,
    w: Vec<f32>,
    llr_cells: Vec<f32>,
    llr: Vec<f32>,
    llr8: Vec<Llr8>,
    hard: Vec<u8>,
}

impl Decoder {
    /// The decoder for a coding, with at most `max_iter` LDPC iterations and the measured min-sum rule of the code
    /// ([`data_rule`]). Errors for a coding the standard does not define.
    pub fn new(coding: Coding, max_iter: u32) -> Result<Decoder, T2Error> {
        let (bch, ldpc) = codes_for(&coding)?;
        let perm = bits_to_cells(coding.frame, coding.modulation, coding.rate)?;
        let cons = Constellation::new(point_set(coding.modulation))?;
        let turn = if coding.rotated {
            let a = -coding.modulation.rotation_degrees().to_radians();
            Complex32::new(a.cos() as f32, a.sin() as f32)
        } else {
            Complex32::new(1.0, 0.0)
        };
        let n = coding.frame.n_ldpc();
        let nc = coding.cells();
        Ok(Decoder {
            coding,
            ldpc: LdpcDecoder::new(&ldpc, data_rule(&coding)),
            bch,
            perm,
            cons,
            turn,
            max_iter,
            quant: Quantizer::default(),
            z: vec![Complex32::default(); nc],
            w: vec![0.0; nc],
            llr_cells: vec![0.0; n],
            llr: vec![0.0; n],
            llr8: vec![0; n],
            hard: vec![0; n],
        })
    }

    /// The coding.
    pub fn coding(&self) -> &Coding {
        &self.coding
    }

    /// Decodes one FEC block from its equalised cells `g` and their weights `|H|^2 / sigma^2` into `bbframe` (`k_bch` bits).
    /// With rotation the Q delay is undone first: f_q takes Re of g_q and Im of g_(q+1), and the weaker of the two weights.
    /// Errors when a slice has the wrong length.
    pub fn decode(
        &mut self,
        g: &[Complex32],
        weights: &[f32],
        bbframe: &mut [u8],
    ) -> Result<BlockResult, T2Error> {
        let nc = self.coding.cells();
        if g.len() != nc || weights.len() != nc || bbframe.len() != self.bch.k() {
            let (expected, got) = if bbframe.len() != self.bch.k() {
                (self.bch.k(), bbframe.len())
            } else {
                (nc, g.len().min(weights.len()))
            };
            return Err(T2Error::Length { expected, got });
        }
        if self.coding.rotated {
            // One weight counts for both parts of a cell, so the distances to the rotated points are those of
            // the cell turned back to the points that are not rotated.
            for q in 0..nc {
                let nq = (q + 1) % nc;
                self.z[q] = Complex32::new(g[q].re, g[nq].im) * self.turn;
                self.w[q] = weights[q].min(weights[nq]);
            }
        } else {
            self.z.copy_from_slice(g);
            self.w.copy_from_slice(weights);
        }
        for v in self.w.iter_mut() {
            if !v.is_finite() || *v < 0.0 {
                *v = 0.0;
            }
        }
        for zz in self.z.iter_mut() {
            if !(zz.re.is_finite() && zz.im.is_finite()) {
                *zz = Complex32::default();
            }
        }
        self.cons
            .demap_block_weighted(&self.z, &self.w, Demap::MaxLog, &mut self.llr_cells)?;
        self.perm.invert(&self.llr_cells, &mut self.llr)?;
        let scale = self.quant.scale_for(&self.llr);
        self.quant.quantize(&self.llr, scale, &mut self.llr8)?;
        let mut res = BlockResult::default();
        match self.ldpc.decode(&self.llr8, self.max_iter, &mut self.hard) {
            Ok(d) => {
                res.ldpc_ok = true;
                res.iterations = d.iterations;
            }
            Err(_) => res.iterations = self.max_iter,
        }
        let nb = self.bch.n();
        match self.bch.decode(&mut self.hard[..nb]) {
            Ok(d) => {
                res.bch_ok = true;
                res.bch_corrected = d.corrected;
            }
            Err(_) => res.bch_ok = false,
        }
        bbframe.copy_from_slice(&self.hard[..self.bch.k()]);
        Ok(res)
    }
}

/// The min-sum rule of a data code.
///
/// Measured by `scale::data_short` and `scale::data_normal` (white noise, max-log demapping, the default quantiser,
/// 50 iterations, a block lost when BCH fails): an offset of 1 gains 0.04 dB to 1.2 dB at a frame error rate of 0.1 on
/// the short codes of rates 1/3 to 3/5 at 64 and 256 points and on the normal codes at 64 points, and costs up to 1.6 dB
/// at rate 3/4 with 256 points, so it is used only where measured. QPSK keeps 13/16 (14/16 for the short rates 1/4 and 1/3),
/// except the normal rate 3/5 code, which gains 0.3 dB with 13/16 less 1.
pub(crate) fn data_rule(coding: &Coding) -> MinSum {
    use CodeRate::*;
    use FecFrame::*;
    use Modulation::*;
    let (scale_q4, offset) = match (coding.frame, coding.rate, coding.modulation) {
        (_, R1_2, Qam64 | Qam256) | (Normal, R3_5 | R2_3 | R3_4, Qam64) | (Normal, R2_3, Qam256) => (16, 1),
        (Short, R1_3 | R2_5 | R3_5, Qam64 | Qam256) | (Short, R1_2, Qam16) => (15, 1),
        (Normal, R3_5, Qpsk) => (13, 1),
        (Normal, R3_5, Qam256) | (Short, R1_4 | R1_3, _) => (14, 0),
        _ => (13, 0),
    };
    MinSum { scale_q4, offset }
}

/// The min-sum rule of the data codes:
/// `cargo test --release -p sdrkit-stack-dvbt2 --lib bicm::scale -- --ignored --nocapture`.
#[cfg(test)]
pub(crate) mod scale {
    use super::*;
    pub(crate) use sdrkit_dev_test::fer::{compare, header, rules};
    use sdrkit_dev_test::fer::{self, frames, noise, Trial};

    struct Data {
        enc: Encoder,
        decs: Vec<Decoder>,
        bits: Vec<u8>,
        out: Vec<u8>,
        cells: Vec<Complex32>,
        weights: Vec<f32>,
        snr_db: f32,
        seed: u64,
    }

    impl Data {
        fn new(c: Coding) -> Data {
            let decs = rules()
                .into_iter()
                .map(|r| {
                    let mut d = Decoder::new(c, 50).expect("decoder");
                    d.ldpc = LdpcDecoder::new(d.ldpc.code(), r);
                    d
                })
                .collect();
            Data {
                enc: Encoder::new(c).expect("encoder"),
                decs,
                bits: vec![0; c.k_bch()],
                out: vec![0; c.k_bch()],
                cells: vec![Complex32::default(); c.cells()],
                weights: vec![0.0; c.cells()],
                snr_db: 0.0,
                seed: 0,
            }
        }
    }

    impl Trial for Data {
        fn start(&mut self, db: f64, seed: u64) {
            (self.snr_db, self.seed) = (db as f32, seed);
        }
        fn send(&mut self, f: u64) {
            let seed = self.seed << 32 | f;
            sdrkit_algo_sim::rng::Rng::new(seed).fill_bits(&mut self.bits);
            self.enc.encode(&self.bits, &mut self.cells).expect("encode");
            noise(&mut self.cells, &mut self.weights, self.snr_db, !seed);
        }
        /// A block is lost when the BCH code fails or its bits are wrong.
        fn lost(&mut self, i: usize) -> bool {
            let r = self.decs[i]
                .decode(&self.cells, &self.weights, &mut self.out)
                .expect("decode");
            !r.bch_ok || self.out != self.bits
        }
    }

    /// The data codings of a frame size: QPSK at every rate the frame has, 16-QAM at rate 1/2, 64-QAM and 256-QAM at rates
    /// 1/3 to 3/4.
    fn codings(frame: FecFrame) -> Vec<Coding> {
        use CodeRate::*;
        use Modulation::*;
        let mut rows: Vec<(CodeRate, Modulation)> =
            [R1_4, R1_3, R2_5, R1_2, R3_5, R2_3, R3_4, R4_5, R5_6].iter().map(|&r| (r, Qpsk)).collect();
        rows.push((R1_2, Qam16));
        for m in [Qam64, Qam256] {
            rows.extend([R1_3, R2_5, R1_2, R3_5, R2_3, R3_4].map(|r| (r, m)));
        }
        rows.into_iter()
            .map(|(rate, modulation)| Coding { frame, rate, modulation, rotated: false })
            .filter(|&c| Decoder::new(c, 1).is_ok())
            .collect()
    }

    /// Measures the codings of [`codings`] for `frame` with `frames` frames a fine point. `LDPC_SCALE_ROWS`, a comma list,
    /// keeps the rows whose name holds one of its items.
    fn measure(frame: FecFrame, frames: u64, deep: bool) {
        header("Es/N0", frames);
        let keep = std::env::var("LDPC_SCALE_ROWS").ok();
        let name = |c: &Coding| format!("{frame:?} {:?} {:?}", c.rate, c.modulation);
        let rows: Vec<Coding> = codings(frame)
            .into_iter()
            .filter(|c| keep.as_deref().is_none_or(|k| k.split(',').any(|item| name(c).contains(item.trim()))))
            .collect();
        let lines = fer::parallel(&rows, |&c| {
            let r = c.k_bch() as f64 / c.frame.n_ldpc() as f64;
            let from = 10.0 * (2f64.powf(r * c.modulation.bits() as f64) - 1.0).log10();
            compare(&mut Data::new(c), &format!("{:<24}", name(&c)), data_rule(&c), from, frames, deep)
        });
        for l in lines {
            println!("{l}");
        }
    }

    /// The short data codes, 1000 frames a point, read at 0.1 and 0.01.
    #[test]
    #[ignore = "measurement"]
    fn data_short() {
        measure(FecFrame::Short, frames(1000), true);
    }

    /// The normal data codes, 200 frames a point, read at 0.1.
    #[test]
    #[ignore = "measurement"]
    fn data_normal() {
        measure(FecFrame::Normal, frames(200), false);
    }
}
