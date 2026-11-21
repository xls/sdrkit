//! Bit-interleaved coding and modulation of one PLP: BCH and LDPC of `sdrkit-algo-fec`, the bit interleaver and demultiplexer,
//! and the square QAM of 16 to 4096 points (`QamMap::DVB`, unit mean power). The encoder is the generator's path, the decoder
//! the receiver's: per-axis Gray demapping with the weight of each cell, LDPC, BCH and BB descrambling.
//!
//! Standard: ETSI EN 302 769 V1.3.1 clauses 5.2.2, 6.1 and 6.2.

use super::tables::{self, CodeRate, Fec, ModCod};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::bitcells::BitCells;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_modem::graydemap::GrayQam;
use sdrkit_core::Complex32;
use sdrkit_proto_ipcast::bbheader::bb_descramble;
use sdrkit_util_bits::interleave::Permutation;

/// The point set of a constellation (tables 12 and 13).
pub fn point_set(qam: tables::Qam) -> PointSet {
    PointSet::qam(qam.points(), QamMap::DVB).unwrap_or_else(|_| unreachable!("square QAM of 16 to 4096 points exists"))
}

/// The bit interleaver and demultiplexer of a coding.
pub fn bit_cells(m: &ModCod) -> Option<Permutation> {
    let f = m.fec();
    BitCells {
        n: f.n,
        k: f.k_ldpc,
        parity: true,
        twist: tables::twist(m.qam, m.long),
        demux: tables::demux(m.qam, m.long, Some(m.rate)),
        bits_per_cell: m.qam.bits(),
    }
    .permutation()
    .ok()
}

/// The transmit chain of one PLP coding: a BBFRAME (K_bch / 8 bytes, before scrambling) in, the cells of one FEC block out.
pub struct PlpEncoder {
    modcod: ModCod,
    fec: Fec,
    bch: Bch,
    ldpc: LdpcCode,
    perm: Permutation,
    points: Vec<Complex32>,
    word: Vec<u8>,
    code: Vec<u8>,
    cellbits: Vec<u8>,
    frame: Vec<u8>,
}

impl PlpEncoder {
    /// The encoder of a coding.
    pub fn new(modcod: ModCod) -> Option<PlpEncoder> {
        let fec = modcod.fec();
        let bch = Bch::from_code(fec.bch).ok()?;
        let ldpc = LdpcCode::from_dvb(fec.ldpc).ok()?;
        Some(PlpEncoder {
            perm: bit_cells(&modcod)?,
            points: point_set(modcod.qam).points().to_vec(),
            word: vec![0; bch.n()],
            code: vec![0; fec.n],
            cellbits: vec![0; fec.n],
            frame: vec![0; fec.k_bch / 8],
            modcod,
            fec,
            bch,
            ldpc,
        })
    }

    /// The coding.
    pub fn modcod(&self) -> ModCod {
        self.modcod
    }

    /// K_bch / 8.
    pub fn frame_bytes(&self) -> usize {
        self.fec.k_bch / 8
    }

    /// Scrambles and encodes one BBFRAME into `cells` (N_ldpc / eta_MOD of them). `None` on wrong lengths.
    pub fn encode(&mut self, bbframe: &[u8], cells: &mut [Complex32]) -> Option<()> {
        if bbframe.len() != self.frame.len() || cells.len() != self.modcod.cells() {
            return None;
        }
        self.frame.copy_from_slice(bbframe);
        bb_descramble(&mut self.frame);
        for (k, b) in self.frame.iter().enumerate() {
            for i in 0..8 {
                self.word[8 * k + i] = (b >> (7 - i)) & 1;
            }
        }
        self.bch.encode(&mut self.word).ok()?;
        self.ldpc.encode(&self.word, &mut self.code).ok()?;
        self.perm.apply(&self.code, &mut self.cellbits).ok()?;
        let eta = self.modcod.qam.bits();
        for (c, bits) in cells.iter_mut().zip(self.cellbits.chunks_exact(eta)) {
            let label = bits.iter().fold(0usize, |a, &b| (a << 1) | usize::from(b));
            *c = self.points[label];
        }
        Some(())
    }
}

/// The 8-bit LLR that the weakest bits of a cell one level from their decision boundary map to at least.
const LSB_TARGET: f64 = 6.0;

/// The min-sum rule of the data code of `modcod`: scale 15/16 less 1 for the 64800-bit codes of rates 2/3 to 5/6, else
/// 13/16 with no offset.
///
/// Measured by `scale::data_long` and `scale::data_short` (white noise, Gray demapping, the decoder's quantiser,
/// 50 iterations, a block lost when BCH fails): on the 64800-bit codes of rates 3/4 and 5/6 at 256 and 1024 points the
/// offset gains 0.17 dB to 0.37 dB at a frame error rate of 0.1, on the others up to 0.1 dB; at rate 9/10 and on the
/// 16200-bit codes it gains nothing and costs up to 0.2 dB.
pub(crate) fn data_rule(modcod: &ModCod) -> MinSum {
    match (modcod.long, modcod.rate) {
        (true, CodeRate::R9_10) | (false, _) => MinSum::with_scale(13),
        (true, _) => MinSum {
            scale_q4: 15,
            offset: 1,
        },
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
    /// Hard decisions before the LDPC decoder that it changed.
    pub raw_errors: u32,
}

/// The receive chain of one PLP coding: equalised cells and their weights in, the descrambled BBFRAME out.
pub struct PlpDecoder {
    modcod: ModCod,
    bch: Bch,
    ldpc: LdpcDecoder,
    perm: Permutation,
    cons: GrayQam,
    max_iter: u32,
    quant: Quantizer,
    /// Half the distance between neighbouring levels of an axis at unit mean power.
    half_step: f64,
    llr_cells: Vec<f32>,
    llr: Vec<f32>,
    llr8: Vec<Llr8>,
    hard: Vec<u8>,
}

impl PlpDecoder {
    /// The decoder of a coding with at most `max_iter` LDPC iterations and the check rule of [`data_rule`].
    pub fn new(modcod: ModCod, max_iter: u32) -> Option<PlpDecoder> {
        let fec = modcod.fec();
        let bch = Bch::from_code(fec.bch).ok()?;
        let code = LdpcCode::from_dvb(fec.ldpc).ok()?;
        Some(PlpDecoder {
            ldpc: LdpcDecoder::new(&code, data_rule(&modcod)),
            perm: bit_cells(&modcod)?,
            cons: GrayQam::new(modcod.qam.points()).ok()?,
            llr_cells: vec![0.0; fec.n],
            llr: vec![0.0; fec.n],
            llr8: vec![0; fec.n],
            hard: vec![0; fec.n],
            quant: Quantizer::default(),
            half_step: (1.5 / (modcod.qam.points() as f64 - 1.0)).sqrt(),
            max_iter,
            modcod,
            bch,
        })
    }

    /// The coding.
    pub fn modcod(&self) -> ModCod {
        self.modcod
    }

    /// Sets the most LDPC iterations.
    pub fn set_iterations(&mut self, n: u32) {
        self.max_iter = n.max(1);
    }

    /// Decodes one FEC block from its cells `z` and weights `w` (`|H|^2 / sigma^2`) into `bbframe` (K_bch / 8 bytes,
    /// descrambled). `None` on wrong lengths.
    pub fn decode(&mut self, z: &[Complex32], w: &[f32], bbframe: &mut [u8]) -> Option<BlockResult> {
        let nc = self.modcod.cells();
        if z.len() != nc || w.len() != nc || bbframe.len() * 8 != self.bch.k() {
            return None;
        }
        self.cons.demap_block(z, w, &mut self.llr_cells).ok()?;
        self.perm.invert(&self.llr_cells, &mut self.llr).ok()?;
        // The weakest bits must keep resolution in 8 bits: a bit one level from its decision boundary maps to at least
        // LSB_TARGET; stronger bits may clip.
        let mean_w = w.iter().map(|&x| f64::from(x)).sum::<f64>() / nc as f64;
        let lsb = 4.0 * self.half_step * self.half_step * mean_w;
        let scale = if lsb > 0.0 {
            self.quant.scale_for(&self.llr).max((LSB_TARGET / lsb) as f32).min(self.quant.max_scale)
        } else {
            self.quant.scale_for(&self.llr)
        };
        self.quant.quantize(&self.llr, scale, &mut self.llr8).ok()?;
        let mut res = BlockResult::default();
        match self.ldpc.decode(&self.llr8, self.max_iter, &mut self.hard) {
            Ok(d) => {
                res.ldpc_ok = true;
                res.iterations = d.iterations;
                res.raw_errors = d.corrected;
            }
            Err(_) => res.iterations = self.max_iter,
        }
        let nb = self.bch.n();
        // A code word needs no decoding; the check is the fast remainder.
        res.bch_ok = self.bch.is_codeword(&self.hard[..nb]) || self.bch.decode(&mut self.hard[..nb]).is_ok();
        for (k, b) in bbframe.iter_mut().enumerate() {
            *b = self.hard[8 * k..8 * k + 8].iter().fold(0u8, |a, &x| (a << 1) | x);
        }
        bb_descramble(bbframe);
        Some(res)
    }
}

/// The min-sum rule of the data codes:
/// `cargo test --release -p sdrkit-stack-dvbcs --lib dvbc2::bicm::scale -- --ignored --nocapture`.
#[cfg(test)]
pub(crate) mod scale {
    use super::*;
    use super::tables::Qam;
    use sdrkit_algo_sim::rng::Rng;
    use sdrkit_dev_test::fer::{self, frames, noise, Trial, SCALES};

    /// The check rules the measurements compare: every scale of [`SCALES`] with offset 0, then with offset 1.
    pub(crate) fn rules() -> Vec<MinSum> {
        [0, 1].iter().flat_map(|&offset| SCALES.iter().map(move |&scale_q4| MinSum { scale_q4, offset })).collect()
    }

    /// A ratio or `-` when there is none.
    fn db(v: Option<f64>) -> String {
        v.map_or("     -".into(), |v| format!(" {v:5.2}"))
    }

    /// The result line of one code: `t` holds one decoder per rule of [`rules`]. Searches the waterfall from `from_db` with
    /// `frames` frames a fine point (down to 0.01 with `deep`, else 0.1), then sends ten times `frames` frames 0.3 dB above
    /// the crossing at 0.1 of `used`. The line gives the crossing at 0.1 and at 0.01 per rule, the lost frames per rule at
    /// that point, and the best rule (the lowest crossing at 0.1 among the rules with no more of these losses than `used`)
    /// with its gain at 0.1 and 0.01.
    pub(crate) fn compare<T: Trial>(t: &mut T, name: &str, used: MinSum, from_db: f64, frames: u64, deep: bool) -> String {
        let rules = rules();
        let n = rules.len();
        let u = rules.iter().position(|&r| r == used).expect("the rule in use is one of the rules");
        let points = fer::search(t, n, from_db, from_db + 8.0, frames, if deep { 0.01 } else { 0.1 });
        let at = |x: f64| -> Vec<Option<f64>> { (0..n).map(|i| fer::threshold(&points, i, x)).collect() };
        let (a1, a2) = (at(0.1), if deep { at(0.01) } else { vec![None; n] });
        let floor_db = a1[u].map(|d| d + 0.3);
        let floor = floor_db.map(|d| fer::point(t, n, d, 0xF1_00D, 10 * frames, None).lost);
        let ok = |i: usize| floor.as_ref().is_none_or(|f| f[i] <= f[u]);
        let key = |i: usize| a1[i].unwrap_or(f64::MAX);
        let best = (0..n).filter(|&i| ok(i) && a1[i].is_some()).min_by(|&a, &b| key(a).total_cmp(&key(b)));
        let gain = |a: &[Option<f64>], b: usize| a[u].zip(a[b]).map(|(x, y)| x - y);
        let rule = |i: usize| format!("{}/{}", rules[i].scale_q4, rules[i].offset);
        let mut line = format!("{name} | used {} | at 0.1:", rule(u));
        line += &a1.iter().map(|&v| db(v)).collect::<String>();
        line += " | at 0.01:";
        line += &a2.iter().map(|&v| db(v)).collect::<String>();
        line += &format!(" | floor{}:", db(floor_db));
        line += &floor.map_or(" -".to_string(), |f| f.iter().map(|n| format!(" {n}")).collect());
        if let Some(b) = best {
            line += &format!(" | best {} gain{}{}", rule(b), db(gain(&a1, b)), db(gain(&a2, b)));
        }
        line
    }

    /// The header of a table of [`compare`] lines.
    pub(crate) fn header(what: &str, frames: u64) {
        let labels: Vec<String> = rules().iter().map(|r| format!("{}/{}", r.scale_q4, r.offset)).collect();
        println!("{what} at frame error rate 0.1 and 0.01, {frames} frames a point; rules (scale/16, offset) = {labels:?}");
    }

    /// One coding with a decoder per rule.
    struct Data {
        enc: PlpEncoder,
        decs: Vec<PlpDecoder>,
        frame: Vec<u8>,
        out: Vec<u8>,
        cells: Vec<Complex32>,
        weights: Vec<f32>,
        snr_db: f32,
        seed: u64,
    }

    impl Data {
        fn new(m: ModCod) -> Data {
            let decs = rules()
                .into_iter()
                .map(|r| {
                    let mut d = PlpDecoder::new(m, 50).expect("decoder");
                    d.ldpc = LdpcDecoder::new(d.ldpc.code(), r);
                    d
                })
                .collect();
            let enc = PlpEncoder::new(m).expect("encoder");
            Data {
                frame: vec![0; enc.frame_bytes()],
                out: vec![0; enc.frame_bytes()],
                cells: vec![Complex32::default(); m.cells()],
                weights: vec![0.0; m.cells()],
                enc,
                decs,
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
            let mut rng = Rng::new(seed);
            for b in &mut self.frame {
                *b = (rng.next_u64() & 0xFF) as u8;
            }
            self.enc.encode(&self.frame, &mut self.cells).expect("encode");
            noise(&mut self.cells, &mut self.weights, self.snr_db, !seed);
        }
        /// A block is lost when the BCH code fails or its bytes are wrong.
        fn lost(&mut self, i: usize) -> bool {
            let r = self.decs[i].decode(&self.cells, &self.weights, &mut self.out).expect("decode");
            !r.bch_ok || self.out != self.frame
        }
    }

    /// The codings of tables 11(a) and 11(b) of a frame size up to 1024-QAM: every rate, 16- to 1024-QAM where listed.
    fn codings(long: bool) -> Vec<ModCod> {
        use CodeRate::*;
        use Qam::*;
        let mut rows = Vec::new();
        for rate in [R2_3, R3_4, R4_5, R5_6, R9_10] {
            for qam in [Q16, Q64, Q256, Q1024] {
                let m = ModCod { long, qam, rate };
                if m.valid() {
                    rows.push(m);
                }
            }
        }
        rows
    }

    /// Measures every coding of [`codings`] for a frame size with `frames` frames a fine point.
    fn measure(long: bool, frames: u64, deep: bool) {
        header("Es/N0", frames);
        let rows = codings(long);
        let lines = fer::parallel(&rows, |&m| {
            let f = m.fec();
            let r = f.k_bch as f64 / f.n as f64;
            let from = 10.0 * (2f64.powf(r * m.qam.bits() as f64) - 1.0).log10();
            let name = format!("{:<22}", format!("{} {:?} {:?}", m.n(), m.rate, m.qam));
            compare(&mut Data::new(m), &name, data_rule(&m), from, frames, deep)
        });
        for l in lines {
            println!("{l}");
        }
    }

    /// The 16200-bit data codes, 1000 frames a point, read at 0.1 and 0.01.
    #[test]
    #[ignore = "measurement"]
    fn data_short() {
        measure(false, frames(1000), true);
    }

    /// The 64800-bit data codes, 200 frames a point, read at 0.1.
    #[test]
    #[ignore = "measurement"]
    fn data_long() {
        measure(true, frames(200), false);
    }
}
