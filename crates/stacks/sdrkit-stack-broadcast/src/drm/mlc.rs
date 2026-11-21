//! Multilevel coding of DRM: the partitioning of the bits over the levels, the punctured rate 1/6 mother code, the bit
//! interleavers, the set-partitioned mapping, and multistage soft decoding with iterations.
//!
//! Standard: ETSI ES 201 980 V4.3.1 clauses 7.2.1, 7.3 and 7.4.

use super::params::Qam;
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::{Llr8, Quantizer};
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;

/// A code rate `rx / ry` of table 27.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rate {
    /// Information bits per period (RX).
    pub rx: u8,
    /// Coded bits per period (RY).
    pub ry: u8,
}

impl Rate {
    /// The rate `rx / ry`.
    pub const fn new(rx: u8, ry: u8) -> Rate {
        Rate { rx, ry }
    }
    /// The rate as a number.
    pub fn value(self) -> f64 {
        self.rx as f64 / self.ry as f64
    }
    /// The puncturing pattern of table 27, or `None` for a rate the table does not have.
    pub fn pattern(self) -> Option<Puncture<'static>> {
        Some(match (self.rx, self.ry) {
            (1, 6) => codes::DRM_1_6,
            (1, 4) => codes::DRM_1_4,
            (3, 10) => codes::DRM_3_10,
            (1, 3) => codes::DRM_1_3,
            (4, 11) => codes::DRM_4_11,
            (2, 5) => codes::DRM_2_5,
            (1, 2) => codes::DRM_1_2,
            (4, 7) => codes::DRM_4_7,
            (3, 5) => codes::DRM_3_5,
            (2, 3) => codes::DRM_2_3,
            (8, 11) => codes::DRM_8_11,
            (3, 4) => codes::DRM_3_4,
            (4, 5) => codes::DRM_4_5,
            (7, 8) => codes::DRM_7_8,
            (8, 9) => codes::DRM_8_9,
            _ => return None,
        })
    }
}

/// Why a multilevel code cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MlcError {
    /// The number of rates is not the number of levels of the constellation.
    Levels,
    /// A rate that table 27 does not have.
    Rate,
    /// The cells cannot carry the code: too few cells for the tail, or `2 N1 R_p` is not a whole number of bits.
    Cells,
}

impl std::fmt::Display for MlcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MlcError::Levels => "one code rate is needed for each level of the constellation",
            MlcError::Rate => "a code rate that ES 201 980 table 27 does not have",
            MlcError::Cells => "the cells cannot carry this code",
        })
    }
}

impl std::error::Error for MlcError {}

/// How the six tail steps of the mother code are punctured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tail {
    /// With table 28 (the MSC and the SDC).
    Table28,
    /// With the level's own pattern of table 27, which runs on over the tail, for this many information bits (the FAC).
    Pattern(usize),
}

/// The multiplier `t` of the bit interleaver of level `p` (clause 7.3.3), or `None` when the level is not interleaved.
pub fn interleaver_multiplier(qam: Qam, p: usize) -> Option<usize> {
    match (qam, p) {
        (Qam::Q64, 1) | (Qam::Q16, 0) => Some(13),
        (Qam::Q64, 2) | (Qam::Q16, 1) | (Qam::Q4, 0) => Some(21),
        _ => None,
    }
}

/// The number `N1` of cells of the higher protected part for `bytes_a` bytes in part A (clause 7.2.1.1):
/// `ceil(8 X / (2 RYlcm sum R_p)) RYlcm`.
pub fn higher_part_cells(bytes_a: usize, rates_a: &[Rate], ry_lcm: usize) -> usize {
    if bytes_a == 0 {
        return 0;
    }
    let sum: f64 = rates_a.iter().map(|r| r.value()).sum();
    let n = (8.0 * bytes_a as f64 / (2.0 * ry_lcm as f64 * sum) - 1e-9).ceil();
    n as usize * ry_lcm
}

/// One level: its rates, sizes, interleavers and buffers.
#[derive(Clone, Debug)]
struct Level {
    a: Option<Puncture<'static>>,
    b: Puncture<'static>,
    tail: Puncture<'static>,
    /// The FAC form: one pattern over the whole stream.
    whole: bool,
    m1: usize,
    m2: usize,
    /// The transmit interleavers of the two parts: `apply` computes `y_i = v_Π(i)`.
    il1: Option<Permutation>,
    il2: Option<Permutation>,
    /// The coded bits of this level as sent (after interleaving), for the decoding of the other levels.
    coded: Vec<u8>,
    info: Vec<u8>,
    known: bool,
}

/// A multilevel code over `N1 + N2` cells (clause 7.3).
#[derive(Clone, Debug)]
pub struct Mlc {
    qam: Qam,
    n1: usize,
    n2: usize,
    levels: Vec<Level>,
    vit: Viterbi,
    // Scratch, sized at construction.
    mother: Vec<u8>,
    punct: Vec<u8>,
    ysoft: Vec<f32>,
    vsoft: Vec<f32>,
    msoft: Vec<f32>,
    mllr: Vec<Llr8>,
    quant: Quantizer,
}

impl Mlc {
    /// The code of the MSC or the SDC: the higher protected part on `n1` cells with `rates_a`, the lower protected part
    /// (with the tail) on `n2` cells with `rates_b`, one rate per level. With one protection level, `n1` is 0 and `rates_a` is
    /// not used.
    pub fn new(
        qam: Qam,
        rates_a: &[Rate],
        rates_b: &[Rate],
        n1: usize,
        n2: usize,
    ) -> Result<Mlc, MlcError> {
        Self::build(qam, rates_a, rates_b, n1, n2, Tail::Table28)
    }

    /// The code of one protection level on `cells` cells.
    pub fn eep(qam: Qam, rates: &[Rate], cells: usize) -> Result<Mlc, MlcError> {
        Self::new(qam, &[], rates, 0, cells)
    }

    /// The code of the FAC: 4-QAM at `rate`, the pattern running on over the tail, `info` bits on `cells` cells.
    pub fn fac(rate: Rate, info: usize, cells: usize) -> Result<Mlc, MlcError> {
        let p = rate.pattern().ok_or(MlcError::Rate)?;
        if p.punctured_len(0, 6 * (info + 6)) != 2 * cells {
            return Err(MlcError::Cells);
        }
        Self::build(Qam::Q4, &[], &[rate], 0, cells, Tail::Pattern(info))
    }

    fn build(
        qam: Qam,
        rates_a: &[Rate],
        rates_b: &[Rate],
        n1: usize,
        n2: usize,
        tail: Tail,
    ) -> Result<Mlc, MlcError> {
        let nl = qam.bits_per_axis();
        if rates_b.len() != nl || (n1 > 0 && rates_a.len() != nl) {
            return Err(MlcError::Levels);
        }
        if 2 * n2 < 12 {
            return Err(MlcError::Cells);
        }
        let mut levels = Vec::with_capacity(nl);
        let mut max_steps = 0;
        for p in 0..nl {
            let rb = rates_b[p];
            let b = rb.pattern().ok_or(MlcError::Rate)?;
            let (a, m1) = if n1 > 0 {
                let ra = rates_a[p];
                let a = ra.pattern().ok_or(MlcError::Rate)?;
                // M_p,1 = 2 N1 R_p must be whole.
                if !(2 * n1 * ra.rx as usize).is_multiple_of(ra.ry as usize) {
                    return Err(MlcError::Cells);
                }
                (Some(a), 2 * n1 * ra.rx as usize / ra.ry as usize)
            } else {
                (None, 0)
            };
            // M_p,2 = RX_p floor((2 N2 - 12) / RY_p); the rest of the part goes to the tail (r_p of table 28).
            let span = 2 * n2 - 12;
            let r_p = span % rb.ry as usize;
            let (m2, tail_p, whole) = match tail {
                Tail::Table28 => (
                    rb.rx as usize * (span / rb.ry as usize),
                    codes::DRM_TAIL[r_p],
                    false,
                ),
                Tail::Pattern(info) => (info, b, true),
            };
            let il = |n: usize| {
                interleaver_multiplier(qam, p)
                    .filter(|_| n > 0)
                    .and_then(|t| Permutation::drm(n, t).ok())
                    .map(Permutation::inverse)
            };
            max_steps = max_steps.max(m1 + m2 + 6);
            levels.push(Level {
                a,
                b,
                tail: tail_p,
                whole,
                m1,
                m2,
                il1: il(2 * n1),
                il2: il(2 * n2),
                coded: vec![0; 2 * (n1 + n2)],
                info: vec![0; m1 + m2],
                known: false,
            });
        }
        let vit = Viterbi::new(codes::DRM_K7_R6, max_steps.max(7)).map_err(|_| MlcError::Cells)?;
        Ok(Mlc {
            qam,
            n1,
            n2,
            levels,
            vit,
            mother: Vec::with_capacity(6 * max_steps),
            punct: Vec::with_capacity(2 * (n1 + n2)),
            ysoft: vec![0.0; 2 * (n1 + n2)],
            vsoft: vec![0.0; 2 * (n1 + n2)],
            msoft: vec![0.0; 6 * max_steps],
            mllr: vec![0; 6 * max_steps],
            quant: Quantizer::default(),
        })
    }

    /// Information bits per level: the higher part first, then the lower part.
    pub fn level_bits(&self) -> Vec<(usize, usize)> {
        self.levels.iter().map(|l| (l.m1, l.m2)).collect()
    }
    /// Information bits of the higher protected part (`L1`).
    pub fn higher_bits(&self) -> usize {
        self.levels.iter().map(|l| l.m1).sum()
    }
    /// All information bits (`L1 + L2`).
    pub fn info_bits(&self) -> usize {
        self.levels.iter().map(|l| l.m1 + l.m2).sum()
    }
    /// Cells (`N1 + N2`).
    pub fn cells(&self) -> usize {
        self.n1 + self.n2
    }
    /// The constellation.
    pub fn qam(&self) -> Qam {
        self.qam
    }

    /// Encodes level `p`'s information bits into its sent bits (punctured and interleaved), kept in the level.
    fn encode_level(&mut self, p: usize) {
        let lv = &mut self.levels[p];
        let (m1, m2) = (lv.m1, lv.m2);
        self.mother.clear();
        let mut enc = Encoder::new(codes::DRM_K7_R6);
        enc.encode(&lv.info, &mut self.mother);
        enc.terminate(&mut self.mother);
        self.punct.clear();
        if let Some(a) = &lv.a {
            puncture(a, 0, &self.mother[..6 * m1], &mut self.punct);
        }
        if lv.whole {
            puncture(&lv.b, 0, &self.mother, &mut self.punct);
        } else {
            puncture(
                &lv.b,
                0,
                &self.mother[6 * m1..6 * (m1 + m2)],
                &mut self.punct,
            );
            puncture(&lv.tail, 0, &self.mother[6 * (m1 + m2)..], &mut self.punct);
        }
        let n1 = 2 * self.n1;
        let (head, rest) = lv.coded.split_at_mut(n1);
        let (p1, p2) = self.punct.split_at(n1.min(self.punct.len()));
        match &lv.il1 {
            Some(il) => {
                let _ = il.apply(p1, head);
            }
            None => head.copy_from_slice(p1),
        }
        match &lv.il2 {
            Some(il) => {
                let _ = il.apply(p2, rest);
            }
            None => rest[..p2.len()].copy_from_slice(p2),
        }
    }

    /// Encodes the information bits (the order of clause 7.3.1.1: the higher part over the levels, then the lower part) into
    /// `N1 + N2` cells. Missing bits are zeros.
    pub fn encode(&mut self, bits: &[u8], cells: &mut [Complex32]) {
        let mut off = 0;
        let get = |off: &mut usize, n: usize, out: &mut [u8]| {
            for o in out.iter_mut().take(n) {
                *o = bits.get(*off).copied().unwrap_or(0) & 1;
                *off += 1;
            }
        };
        for lv in &mut self.levels {
            let m1 = lv.m1;
            get(&mut off, m1, &mut lv.info[..m1]);
        }
        for lv in &mut self.levels {
            let (m1, m2) = (lv.m1, lv.m2);
            get(&mut off, m2, &mut lv.info[m1..m1 + m2]);
        }
        for p in 0..self.levels.len() {
            self.encode_level(p);
        }
        for (c, cell) in cells.iter_mut().enumerate().take(self.cells()) {
            let (mut i, mut q) = (0usize, 0usize);
            for (p, lv) in self.levels.iter().enumerate() {
                i |= (lv.coded[2 * c] as usize) << p;
                q |= (lv.coded[2 * c + 1] as usize) << p;
            }
            *cell = self.qam.map(i, q);
        }
    }

    /// The soft value (positive means 0) of bit `level` of the axis value `y`, with every level in `mask` fixed to its bit in
    /// `known`, weighted by `w`.
    fn soft_given(&self, y: f32, level: usize, known: usize, mask: usize, w: f32) -> f32 {
        let (mut d0, mut d1) = (f32::MAX, f32::MAX);
        for idx in 0..self.qam.levels() {
            if idx & mask != known & mask {
                continue;
            }
            let d = (y - self.qam.axis(idx)).powi(2);
            if (idx >> level) & 1 == 0 {
                d0 = d0.min(d);
            } else {
                d1 = d1.min(d);
            }
        }
        w * (d1 - d0)
    }

    /// Decodes equalised cells with their weights into information bits (the order of [`Mlc::encode`]), with `iterations`
    /// passes over the levels: each level is decoded with the bits of the levels decoded before it.
    pub fn decode(&mut self, z: &[Complex32], w: &[f32], iterations: usize, out: &mut Vec<u8>) {
        let nl = self.levels.len();
        let ncell = self.cells().min(z.len()).min(w.len());
        for lv in &mut self.levels {
            lv.known = false;
        }
        for it in 0..iterations.max(1) {
            for l in 0..nl {
                let mut mask = 0usize;
                for (o, lv) in self.levels.iter().enumerate() {
                    if lv.known && o != l && (it > 0 || o < l) {
                        mask |= 1 << o;
                    }
                }
                for c in 0..self.cells() {
                    if c >= ncell {
                        self.ysoft[2 * c] = 0.0;
                        self.ysoft[2 * c + 1] = 0.0;
                        continue;
                    }
                    let (mut ki, mut kq) = (0usize, 0usize);
                    for (o, lv) in self.levels.iter().enumerate() {
                        if mask & (1 << o) != 0 {
                            ki |= (lv.coded[2 * c] as usize) << o;
                            kq |= (lv.coded[2 * c + 1] as usize) << o;
                        }
                    }
                    let wc = if w[c].is_finite() { w[c] } else { 0.0 };
                    self.ysoft[2 * c] = self.soft_given(z[c].re, l, ki, mask, wc);
                    self.ysoft[2 * c + 1] = self.soft_given(z[c].im, l, kq, mask, wc);
                }
                self.decode_level(l);
                self.encode_level(l);
                self.levels[l].known = true;
            }
        }
        out.clear();
        for lv in &self.levels {
            out.extend_from_slice(&lv.info[..lv.m1]);
        }
        for lv in &self.levels {
            out.extend_from_slice(&lv.info[lv.m1..lv.m1 + lv.m2]);
        }
    }

    /// Deinterleaves, depunctures and Viterbi-decodes level `l` from `ysoft` into its information bits.
    fn decode_level(&mut self, l: usize) {
        let n1 = 2 * self.n1;
        let lv = &self.levels[l];
        let (m1, m2) = (lv.m1, lv.m2);
        let (s1, s2) = self.ysoft.split_at(n1);
        let (v1, v2) = self.vsoft.split_at_mut(n1);
        match &lv.il1 {
            Some(il) => {
                let _ = il.invert(s1, v1);
            }
            None => v1.copy_from_slice(s1),
        }
        match &lv.il2 {
            Some(il) => {
                let _ = il.invert(s2, v2);
            }
            None => v2.copy_from_slice(s2),
        }
        let steps = m1 + m2 + 6;
        let mother = &mut self.msoft[..6 * steps];
        let mut at = 0;
        if let Some(a) = &lv.a {
            let k = a.punctured_len(0, 6 * m1);
            let _ = depuncture_frame(a, 0, &self.vsoft[at..at + k], &mut mother[..6 * m1]);
            at += k;
        }
        if lv.whole {
            let _ = depuncture_frame(&lv.b, 0, &self.vsoft[at..], mother);
        } else {
            let k = lv.b.punctured_len(0, 6 * m2);
            let _ = depuncture_frame(
                &lv.b,
                0,
                &self.vsoft[at..at + k],
                &mut mother[6 * m1..6 * (m1 + m2)],
            );
            at += k;
            let _ = depuncture_frame(&lv.tail, 0, &self.vsoft[at..], &mut mother[6 * (m1 + m2)..]);
        }
        let llr = &mut self.mllr[..6 * steps];
        let _ = self.quant.quantize_auto(mother, llr);
        let lv = &mut self.levels[l];
        let _ = self.vit.decode(llr, Ends::Zero, &mut lv.info[..m1 + m2]);
    }
}
