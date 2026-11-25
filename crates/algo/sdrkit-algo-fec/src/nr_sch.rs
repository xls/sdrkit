//! The 5G NR shared-channel coding chain (3GPP TS 38.212 clauses 5.1, 5.2.2, 5.3.2, 5.4.2, 5.5 and 7.2): the transport block
//! CRC, the choice of the LDPC base graph, code block segmentation, LDPC encoding of any lifting size, rate matching with
//! redundancy versions, bit interleaving and code block concatenation; and the inverse with soft combining.
//!
//! One layer and no limited-buffer rate matching (`N_cb = N`).

use crate::error::{FecError, Param, Result};
use crate::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use crate::llr::Llr8;
use crate::tables_3gpp::{NR_LDPC_BG1, NR_LDPC_BG2, NR_LDPC_LIFTING_SETS};
use sdrkit_util_bits::crc::catalog::{CRC16_XMODEM, CRC24_LTE_A, CRC24_LTE_B};
use sdrkit_util_bits::pack::push_u64;

/// The LDPC base graph of a transport block of `a` bits at code rate `rate` (38.212 clause 7.2.2).
pub fn base_graph(a: usize, rate: f64) -> u8 {
    if a <= 292 || (a <= 3824 && rate <= 0.67) || rate <= 0.25 {
        2
    } else {
        1
    }
}

/// One nonzero entry of a base graph: row, column, shift per lifting set.
type BaseEntry = (u8, u8, [u16; 8]);

/// An NR LDPC encoder for one base graph and lifting size, from the structure of the base graphs: the first parity group from
/// the sum of the four core rows, every other parity group from a row that holds one unknown group.
#[derive(Clone, Debug)]
pub struct NrLdpcEncoder {
    z: usize,
    ncols: usize,
    kb: usize,
    rows: Vec<Vec<(u16, u16)>>,
    /// The shift of the first parity group that is left when the four core rows are added.
    core_shift: usize,
}

impl NrLdpcEncoder {
    /// The encoder of base graph `bg` (1 or 2) lifted by `z`. An error when `z` is not a lifting size.
    pub fn new(bg: u8, z: usize) -> Result<Self> {
        let ils = NR_LDPC_LIFTING_SETS
            .iter()
            .position(|set| set.contains(&(z as u16)))
            .ok_or(FecError::NotAllowed {
                param: Param::Z,
                got: z as i64,
            })?;
        let (table, nrows, ncols, kb): (&[BaseEntry], usize, usize, usize) = match bg {
            1 => (&NR_LDPC_BG1, 46, 68, 22),
            2 => (&NR_LDPC_BG2, 42, 52, 10),
            _ => return Param::BaseGraph.not_allowed(bg),
        };
        let mut rows: Vec<Vec<(u16, u16)>> = vec![Vec::new(); nrows];
        for &(r, c, v) in table {
            rows[r as usize].push((c as u16, (v[ils] as usize % z) as u16));
        }
        // Equal shifts of the first parity group cancel in the sum of the core rows; one is left.
        let mut left: Vec<usize> = Vec::new();
        for row in &rows[..4] {
            for &(c, s) in row {
                if c as usize == kb {
                    match left.iter().position(|&x| x == s as usize) {
                        Some(i) => {
                            left.swap_remove(i);
                        }
                        None => left.push(s as usize),
                    }
                }
            }
        }
        if left.len() != 1 {
            return Param::BaseGraph.degenerate();
        }
        Ok(NrLdpcEncoder {
            z,
            ncols,
            kb,
            rows,
            core_shift: left[0],
        })
    }

    /// Information bits (`K`).
    pub fn k(&self) -> usize {
        self.kb * self.z
    }

    /// Code word bits with the first `2 z` (`68 z` or `52 z`).
    pub fn n(&self) -> usize {
        self.ncols * self.z
    }

    /// Encodes `info` (`K` bits) into `out` (the whole code word, information first).
    pub fn encode(&self, info: &[u8], out: &mut [u8]) -> Result<()> {
        let (z, kb) = (self.z, self.kb);
        if info.len() != self.k() {
            return Err(FecError::InputLength {
                expected: self.k(),
                got: info.len(),
            });
        }
        if out.len() != self.n() {
            return Err(FecError::OutputTooShort { need: self.n() });
        }
        out[..kb * z].copy_from_slice(info);
        out[kb * z..].fill(0);
        let mut known = vec![false; self.ncols];
        known[..kb].fill(true);
        let mut acc = vec![0u8; z];
        // The sum of the core rows over the information groups.
        for row in &self.rows[..4] {
            for &(c, s) in row {
                if (c as usize) < kb {
                    let g = &out[c as usize * z..(c as usize + 1) * z];
                    for (m, a) in acc.iter_mut().enumerate() {
                        *a ^= g[(m + s as usize) % z];
                    }
                }
            }
        }
        for (m, &a) in acc.iter().enumerate() {
            out[kb * z + (m + self.core_shift) % z] = a & 1;
        }
        known[kb] = true;
        for row in &self.rows {
            acc.fill(0);
            let mut unknown: Option<(usize, usize)> = None;
            for &(c, s) in row {
                let (c, s) = (c as usize, s as usize);
                if known[c] {
                    let g = &out[c * z..(c + 1) * z];
                    for (m, a) in acc.iter_mut().enumerate() {
                        *a ^= g[(m + s) % z];
                    }
                } else if unknown.is_some() {
                    return Param::BaseGraph.degenerate();
                } else {
                    unknown = Some((c, s));
                }
            }
            if let Some((c, s)) = unknown {
                for (m, &a) in acc.iter().enumerate() {
                    out[c * z + (m + s) % z] = a & 1;
                }
                known[c] = true;
            }
        }
        Ok(())
    }
}

/// The sizes of the coding chain of one transport block.
///
/// ```
/// use sdrkit_algo_fec::nr_sch::{NrSch, NrSchDecoder};
/// let sch = NrSch::new(552, 0.3).unwrap();
/// let tb: Vec<u8> = (0..552).map(|i| (i * 7 % 5 == 0) as u8).collect();
/// let f = sch.encode(&tb, 0, 2, 1800).unwrap();
/// let llr: Vec<f32> = f.iter().map(|&b| if b == 0 { 4.0 } else { -4.0 }).collect();
/// let mut dec = NrSchDecoder::new(&sch).unwrap();
/// dec.add(&llr, 0, 2).unwrap();
/// assert_eq!(dec.decode(20).unwrap(), tb);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NrSch {
    /// Transport block bits (`A`).
    pub a: usize,
    /// The base graph (1 or 2).
    pub bg: u8,
    /// Transport block CRC bits (16 or 24).
    pub tb_crc: usize,
    /// Code blocks (`C`).
    pub c: usize,
    /// Bits of a code block with its CRC, without filler (`K'`).
    pub k_prime: usize,
    /// The lifting size (`Z_c`).
    pub z: usize,
    /// Information bits of the code (`K`).
    pub k: usize,
    /// Bits of the circular buffer (`N`: `66 z` or `50 z`).
    pub n: usize,
}

impl NrSch {
    /// The chain of a transport block of `a` bits at the code rate `rate` of its MCS (38.212 clauses 7.2.1, 7.2.2, 5.2.2).
    /// An error when `a` is 0.
    pub fn new(a: usize, rate: f64) -> Result<Self> {
        Param::Bits.check(a, 1, 1 << 21)?;
        let bg = base_graph(a, rate);
        let tb_crc = if a > 3824 { 24 } else { 16 };
        let b = a + tb_crc;
        let k_cb = if bg == 1 { 8448 } else { 3840 };
        let (c, b_all) = if b <= k_cb {
            (1, b)
        } else {
            let c = b.div_ceil(k_cb - 24);
            (c, b + c * 24)
        };
        let k_prime = b_all.div_ceil(c);
        let kb = if bg == 1 {
            22
        } else if b > 640 {
            10
        } else if b > 560 {
            9
        } else if b > 192 {
            8
        } else {
            6
        };
        let z = NR_LDPC_LIFTING_SETS
            .iter()
            .flat_map(|s| s.iter().map(|&z| z as usize))
            .filter(|&z| kb * z >= k_prime)
            .min()
            .ok_or(FecError::Unsupported { param: Param::Z })?;
        let (k, n) = if bg == 1 { (22 * z, 66 * z) } else { (10 * z, 50 * z) };
        Ok(NrSch {
            a,
            bg,
            tb_crc,
            c,
            k_prime,
            z,
            k,
            n,
        })
    }

    /// Rate-matched bits of code block `r` when the transport block gets `g` bits at `qm` bits per symbol (clause 5.4.2.1).
    pub fn e(&self, r: usize, qm: usize, g: usize) -> usize {
        let per = g / qm;
        if r < self.c - per % self.c {
            qm * (per / self.c)
        } else {
            qm * per.div_ceil(self.c)
        }
    }

    /// The start of redundancy version `rv` in the circular buffer (table 5.4.2.1-2).
    pub fn k0(&self, rv: u8) -> usize {
        let num = match (self.bg, rv & 3) {
            (_, 0) => 0,
            (1, 1) => 17,
            (1, 2) => 33,
            (1, _) => 56,
            (_, 1) => 13,
            (_, 2) => 25,
            _ => 43,
        };
        let den = if self.bg == 1 { 66 } else { 50 };
        (num * self.n / (den * self.z)) * self.z
    }

    /// Whether position `i` of the circular buffer is a filler bit.
    fn filler(&self, i: usize) -> bool {
        (self.k_prime - 2 * self.z..self.k - 2 * self.z).contains(&i)
    }

    /// The positions of the circular buffer that the `e` bits of redundancy version `rv` take, in the order sent after bit
    /// interleaving with `qm` bits per symbol: `f[x]` is buffer bit `map[x]`.
    fn map(&self, rv: u8, qm: usize, e: usize, map: &mut Vec<u32>) {
        let mut sel = Vec::with_capacity(e);
        let mut j = self.k0(rv);
        while sel.len() < e {
            let i = j % self.n;
            if !self.filler(i) {
                sel.push(i as u32);
            }
            j += 1;
        }
        let rows = e / qm;
        for j in 0..rows {
            for i in 0..qm {
                map.push(sel[i * rows + j]);
            }
        }
    }

    /// Encodes the transport block `tb` (`A` bits) into `g` bits (a multiple of `qm`) for redundancy version `rv`.
    pub fn encode(&self, tb: &[u8], rv: u8, qm: usize, g: usize) -> Result<Vec<u8>> {
        if tb.len() != self.a {
            return Err(FecError::InputLength {
                expected: self.a,
                got: tb.len(),
            });
        }
        if qm == 0 || !g.is_multiple_of(qm) || g / qm < self.c {
            return Param::E.not_allowed(g);
        }
        let mut b = tb.to_vec();
        let crc = if self.tb_crc == 24 {
            CRC24_LTE_A.bits(&b) as u64
        } else {
            CRC16_XMODEM.bits(&b) as u64
        };
        push_u64(&mut b, crc, self.tb_crc as u32);
        let enc = NrLdpcEncoder::new(self.bg, self.z)?;
        let cb_crc = if self.c > 1 { 24 } else { 0 };
        let data = self.k_prime - cb_crc;
        let mut out = Vec::with_capacity(g);
        let mut word = vec![0u8; enc.n()];
        let mut map = Vec::new();
        for r in 0..self.c {
            let mut cb: Vec<u8> = b[r * data..(r + 1) * data].to_vec();
            if cb_crc > 0 {
                let c = CRC24_LTE_B.bits(&cb) as u64;
                push_u64(&mut cb, c, 24);
            }
            cb.resize(self.k, 0);
            enc.encode(&cb, &mut word)?;
            map.clear();
            self.map(rv, qm, self.e(r, qm, g), &mut map);
            out.extend(map.iter().map(|&i| word[i as usize + 2 * self.z]));
        }
        Ok(out)
    }
}

/// The decoder of one transport block: soft values of one or more redundancy versions are added, then decoded.
#[derive(Clone, Debug)]
pub struct NrSchDecoder {
    sch: NrSch,
    ldpc: LdpcDecoder,
    /// The soft circular buffer of every code block.
    acc: Vec<f32>,
    map: Vec<u32>,
    q: Vec<Llr8>,
    hard: Vec<u8>,
}

impl NrSchDecoder {
    /// A decoder for `sch` with empty buffers.
    pub fn new(sch: &NrSch) -> Result<Self> {
        let code = LdpcCode::nr(sch.bg, sch.z)?;
        let n_all = code.n();
        Ok(NrSchDecoder {
            sch: sch.clone(),
            ldpc: LdpcDecoder::new(&code, MinSum::with_scale(12)),
            acc: vec![0.0; sch.c * sch.n],
            map: Vec::new(),
            q: vec![0; n_all],
            hard: vec![0; n_all],
        })
    }

    /// The chain.
    pub fn sch(&self) -> &NrSch {
        &self.sch
    }

    /// Forgets the soft values added so far.
    pub fn reset(&mut self) {
        self.acc.fill(0.0);
    }

    /// Adds the soft values `llr` (positive means 0) of one transmission with redundancy version `rv` and `qm` bits per symbol.
    /// An error when `llr` is not a multiple of `qm` or holds a value that is not finite.
    pub fn add(&mut self, llr: &[f32], rv: u8, qm: usize) -> Result<()> {
        let g = llr.len();
        if qm == 0 || !g.is_multiple_of(qm) || g / qm < self.sch.c {
            return Param::E.not_allowed(g);
        }
        crate::llr::check_finite(llr)?;
        let mut at = 0;
        for r in 0..self.sch.c {
            let e = self.sch.e(r, qm, g);
            self.map.clear();
            let mut map = std::mem::take(&mut self.map);
            self.sch.map(rv, qm, e, &mut map);
            let acc = &mut self.acc[r * self.sch.n..(r + 1) * self.sch.n];
            for (&i, &v) in map.iter().zip(&llr[at..at + e]) {
                acc[i as usize] += v;
            }
            self.map = map;
            at += e;
        }
        Ok(())
    }

    /// Decodes what was added with at most `max_iter` LDPC iterations per code block. Returns the transport block (`A` bits)
    /// when every CRC checks, else `Err(Uncorrectable)`.
    pub fn decode(&mut self, max_iter: u32) -> Result<Vec<u8>> {
        let s = &self.sch;
        let two_z = 2 * s.z;
        let cb_crc = if s.c > 1 { 24 } else { 0 };
        let mut b = Vec::with_capacity(s.a + s.tb_crc);
        let quant = crate::llr::Quantizer::default();
        for r in 0..s.c {
            let acc = &self.acc[r * s.n..(r + 1) * s.n];
            let scale = quant.scale_for(acc);
            self.q.fill(0);
            quant.quantize(acc, scale, &mut self.q[two_z..two_z + s.n])?;
            // Filler bits are known zeros.
            self.q[s.k_prime..s.k].fill(127);
            // An open check with a good CRC is still a block: the CRC decides.
            let _ = self.ldpc.decode(&self.q, max_iter, &mut self.hard);
            let cb = &self.hard[..s.k_prime];
            if cb_crc > 0 && !CRC24_LTE_B.verify_bits(cb) {
                return Err(FecError::Uncorrectable);
            }
            b.extend_from_slice(&cb[..s.k_prime - cb_crc]);
        }
        b.truncate(s.a + s.tb_crc);
        let ok = if s.tb_crc == 24 {
            CRC24_LTE_A.verify_bits(&b)
        } else {
            CRC16_XMODEM.verify_bits(&b)
        };
        if !ok {
            return Err(FecError::Uncorrectable);
        }
        b.truncate(s.a);
        Ok(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(n: usize, seed: u32) -> Vec<u8> {
        let mut x = seed.wrapping_mul(2654435761).max(1);
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x >> 9) as u8 & 1
            })
            .collect()
    }

    #[test]
    fn encoder_gives_code_words() {
        for (bg, z) in [(2u8, 2usize), (2, 36), (2, 104), (2, 384), (1, 7), (1, 80), (1, 208)] {
            let enc = NrLdpcEncoder::new(bg, z).unwrap();
            let code = LdpcCode::nr(bg, z).unwrap();
            assert_eq!((enc.k(), enc.n()), (code.k(), code.n()));
            let info = bits(enc.k(), z as u32);
            let mut word = vec![0u8; enc.n()];
            enc.encode(&info, &mut word).unwrap();
            assert_eq!(&word[..enc.k()], &info[..]);
            assert!(code.is_codeword(&word), "bg {bg} z {z}");
            assert!(word[enc.k()..].contains(&1));
        }
        assert!(NrLdpcEncoder::new(2, 17).is_err());
        assert!(NrLdpcEncoder::new(3, 16).is_err());
    }

    #[test]
    fn sizes_follow_the_clauses() {
        // 38.212 7.2.2: the base graph by size and rate.
        assert_eq!(base_graph(292, 0.9), 2);
        assert_eq!(base_graph(293, 0.9), 1);
        assert_eq!(base_graph(3824, 0.67), 2);
        assert_eq!(base_graph(3825, 0.67), 1);
        assert_eq!(base_graph(8000, 0.25), 2);
        // A = 552: B = 568, K_b = 9, Z = 64 (9 x 64 = 576), K = 640, N = 3200.
        let s = NrSch::new(552, 0.3).unwrap();
        assert_eq!((s.bg, s.tb_crc, s.c, s.k_prime, s.z, s.k, s.n), (2, 16, 1, 568, 64, 640, 3200));
        // Redundancy version starts of base graph 2: 0, 13 Z, 25 Z, 43 Z.
        assert_eq!([s.k0(0), s.k0(1), s.k0(2), s.k0(3)], [0, 832, 1600, 2752]);
        // A = 20000 at rate 0.8: base graph 1, B = 20024, C = 3, K' = 6699, Z = 320.
        let s = NrSch::new(20000, 0.8).unwrap();
        assert_eq!((s.bg, s.tb_crc, s.c, s.k_prime, s.z), (1, 24, 3, 6699, 320));
        assert_eq!([s.k0(1), s.k0(2), s.k0(3)], [17 * 320, 33 * 320, 56 * 320]);
        // G = 30002 bits at QPSK over 3 blocks: 15001 symbols, two blocks of 5000 and one of 5001.
        assert_eq!([s.e(0, 2, 30002), s.e(1, 2, 30002), s.e(2, 2, 30002)], [10000, 10000, 10002]);
    }

    #[test]
    fn round_trip_and_combining() {
        for (a, rate, qm, g) in [(24usize, 0.12, 2usize, 400usize), (552, 0.3, 2, 1800), (2976, 0.44, 4, 6800), (9000, 0.7, 2, 13000)] {
            let sch = NrSch::new(a, rate).unwrap();
            let tb = bits(a, a as u32);
            let mut dec = NrSchDecoder::new(&sch).unwrap();
            for rv in 0..4u8 {
                let f = sch.encode(&tb, rv, qm, g).unwrap();
                assert_eq!(f.len(), g);
                let llr: Vec<f32> = f.iter().map(|&b| if b == 0 { 3.0 } else { -3.0 }).collect();
                dec.reset();
                dec.add(&llr, rv, qm).unwrap();
                // Redundancy versions 0 and 3 decode alone; 1 and 2 need another version.
                if rv == 0 || rv == 3 {
                    assert_eq!(dec.decode(30).unwrap(), tb, "a {a} rv {rv}");
                }
            }
            // Two versions with errors that either alone could not bear at this length.
            let f0 = sch.encode(&tb, 0, qm, g).unwrap();
            let f2 = sch.encode(&tb, 2, qm, g).unwrap();
            let soft = |f: &[u8], step: usize| -> Vec<f32> {
                f.iter()
                    .enumerate()
                    .map(|(i, &b)| {
                        let v = if b == 0 { 1.0 } else { -1.0 };
                        if i % step == 0 {
                            -0.5 * v
                        } else {
                            v
                        }
                    })
                    .collect()
            };
            dec.reset();
            dec.add(&soft(&f0, 11), 0, qm).unwrap();
            dec.add(&soft(&f2, 13), 2, qm).unwrap();
            assert_eq!(dec.decode(40).unwrap(), tb, "a {a} combined");
            // A wrong block does not pass.
            dec.reset();
            dec.add(&vec![0.5; g], 0, qm).unwrap();
            let got = dec.decode(5);
            assert!(got.is_err() || got.unwrap() != tb);
        }
    }
}
