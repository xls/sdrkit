//! LDPC codes: the code description (quasi-cyclic layers of `z` checks), encoders, a layered min-sum decoder with `i8`
//! messages, and a sum-product decoder with ordered-statistics post-processing for short codes.
//!
//! # Structure
//!
//! Every code is held as **layers** of `z` parity checks. Each layer lists its edges: a column group of `z` bits and a cyclic
//! shift, so lane `l` of the layer checks bit `(l + shift) mod z` of the group. Quasi-cyclic codes (DVB-S2/S2X/T2 with `z = 360`
//! after the standard row and parity permutation, 5G NR base graphs with `z` = the lifting size) decode `z` checks at once, which
//! the compiler turns into vector instructions; a code given by plain check lists (WSJT) is the case `z = 1`.
//!
//! DVB codes become quasi-cyclic by numbering check `i = b q + a` as lane `b` of layer `a`, and parity bit `j = b q + a` as lane
//! `b` of parity group `a`; the accumulator then becomes one edge with shift 359 whose lane 0 does not exist (the first parity bit
//! has no predecessor), held as a masked lane.
//!
//! ```
//! use sdrkit_algo_fec::ldpc::{tables::dvb_s2, LdpcCode, LdpcDecoder, MinSum};
//! let code = LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap();
//! let info: Vec<u8> = (0..code.k()).map(|i| (i % 7 == 1) as u8).collect();
//! let mut cw = vec![0u8; code.n()];
//! code.encode(&info, &mut cw).unwrap();
//! // Hard bits to LLRs (positive means 0) with 60 errors.
//! let mut llr: Vec<i8> = cw.iter().map(|&b| if b == 0 { 12 } else { -12 }).collect();
//! for i in (0..llr.len()).step_by(270) {
//!     llr[i] = -llr[i];
//! }
//! let mut dec = LdpcDecoder::new(&code, MinSum::default());
//! let mut out = vec![0u8; code.n()];
//! let r = dec.decode(&llr, 50, &mut out).unwrap();
//! assert_eq!(out, cw);
//! assert_eq!(r.corrected, 60);
//! ```

pub mod alist;
mod atsc3;
mod bp;
mod dtmb;
mod encoder;
mod minsum;
pub mod staircase;
#[cfg(target_arch = "aarch64")]
mod minsum_simd;
#[rustfmt::skip]
pub mod tables;

pub use bp::{BpDecoder, Osd};
pub use encoder::LdpcEncoder;
pub use minsum::{check_backend, LdpcBackend, LdpcDecoder, LdpcKernel, MinSum, BACKENDS};

use crate::error::{FecError, Param, Result};
use tables::DvbTable;

/// One nonzero entry of an NR base graph: row, column, shift per lifting set.
type BaseEntry = (u8, u8, [u16; 8]);

/// Marks an edge with no masked lane.
pub(crate) const NO_MASK: u16 = u16::MAX;

/// One edge group of a layer: `z` checks to `z` bits of column group `col`, lane `l` checking bit `(l + shift) mod z`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Edge {
    pub col: u32,
    pub shift: u16,
    /// A lane of this edge that does not exist (DVB's first parity bit), or `NO_MASK`.
    pub mask: u16,
}

/// Where the information bits sit in a code word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InfoAt {
    /// The first `k` bits (DVB, NR, FT8).
    Start,
    /// The last `k` bits (JS8).
    End,
}

#[derive(Clone, Debug)]
enum Encoding {
    /// DVB IRA: parity accumulator from the address table.
    Dvb(&'static DvbTable),
    /// Dense parity equations `p = A u`, one row of `k` bits per parity bit.
    Dense {
        a: Vec<u64>,
        words: usize,
        ppos: Vec<u32>,
    },
    /// No encoder (too large for the dense one).
    None,
}

/// An LDPC code as layers of `z` checks. See the [module documentation](self).
#[derive(Clone, Debug)]
pub struct LdpcCode {
    name: String,
    n: usize,
    k: usize,
    z: usize,
    ncols: usize,
    edges: Vec<Edge>,
    layer_start: Vec<u32>,
    /// Code word position of each internal bit (`col * z + lane`); empty for the identity.
    perm: Vec<u32>,
    info_at: InfoAt,
    encoding: Encoding,
}

impl LdpcCode {
    /// A DVB-S2, S2X or T2 code from its address table (EN 302 307-1 clause 5.3.2). An error when the table is inconsistent.
    pub fn from_dvb(t: &'static DvbTable) -> Result<Self> {
        let (n, k, q) = (t.n as usize, t.k as usize, t.q as usize);
        if k % 360 != 0 || n <= k || n - k != 360 * q || t.rows.len() != k / 360 {
            return Param::Table.conflict(Param::K);
        }
        let ninfo = k / 360;
        let mut layers: Vec<Vec<Edge>> = vec![Vec::new(); q];
        for (g, row) in t.rows.iter().enumerate() {
            for &x in row.iter() {
                let x = x as usize;
                if x >= n - k {
                    return Param::Table.range(0, (n - k) as i64 - 1, x);
                }
                let (x0, x1) = (x % q, x / q);
                layers[x0].push(Edge {
                    col: g as u32,
                    shift: ((360 - x1) % 360) as u16,
                    mask: NO_MASK,
                });
            }
        }
        for (a, layer) in layers.iter_mut().enumerate() {
            layer.push(Edge {
                col: (ninfo + a) as u32,
                shift: 0,
                mask: NO_MASK,
            });
            if a >= 1 {
                layer.push(Edge {
                    col: (ninfo + a - 1) as u32,
                    shift: 0,
                    mask: NO_MASK,
                });
            } else {
                layer.push(Edge {
                    col: (ninfo + q - 1) as u32,
                    shift: 359,
                    mask: 0,
                });
            }
        }
        let mut perm = vec![0u32; n];
        for col in 0..ninfo + q {
            for lane in 0..360 {
                perm[col * 360 + lane] = if col < ninfo {
                    (col * 360 + lane) as u32
                } else {
                    (k + lane * q + (col - ninfo)) as u32
                };
            }
        }
        let mut c = Self::assemble(
            t.name.to_string(),
            n,
            k,
            360,
            ninfo + q,
            layers,
            perm,
            InfoAt::Start,
        )?;
        c.encoding = Encoding::Dvb(t);
        Ok(c)
    }

    /// A code given by its parity checks: `checks[i]` lists the code word bits of check `i`. With `info_at`, the information bits
    /// are the first or the last `k`. A dense encoder is built when `n − k ≤ 4096` and the parity columns are independent.
    pub fn from_checks<T: Copy + Into<u32>>(
        name: &str,
        n: usize,
        k: usize,
        checks: &[&[T]],
        info_at: InfoAt,
    ) -> Result<Self> {
        Param::N.check(n, 2, 1 << 16)?;
        Param::K.check(k, 1, n as i64 - 1)?;
        let mut layers = Vec::with_capacity(checks.len());
        for c in checks {
            if c.iter().any(|&b| b.into() as usize >= n) {
                return Param::Checks.conflict(Param::N);
            }
            let mut layer: Vec<Edge> = c
                .iter()
                .map(|&b| Edge {
                    col: b.into(),
                    shift: 0,
                    mask: NO_MASK,
                })
                .collect();
            layer.sort_by_key(|e| e.col);
            layer.dedup_by_key(|e| e.col);
            layers.push(layer);
        }
        let mut code = Self::assemble(name.to_string(), n, k, 1, n, layers, Vec::new(), info_at)?;
        code.build_dense_encoder();
        Ok(code)
    }

    /// A systematic code of rate 1/2 whose parity part is an accumulator: the code word is `k` data bits, then `k` parity bits.
    /// Check `i` holds the data bits `rows[i]`, parity bit `i` and, for `i > 0`, parity bit `i − 1`, so the encoder is
    /// `p_0 = s_0`, `p_i = p_(i−1) + s_i` with `s_i` the sum of the data bits of row `i`. An error when `rows` does not have
    /// `k` rows or names a bit that is not a data bit.
    pub fn from_accumulator<T: Copy + Into<u32>>(name: &str, k: usize, rows: &[&[T]]) -> Result<Self> {
        Param::K.check(k, 1, 1 << 15)?;
        if rows.len() != k || rows.iter().any(|r| r.iter().any(|&b| b.into() as usize >= k)) {
            return Param::Checks.conflict(Param::K);
        }
        let checks: Vec<Vec<u32>> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let mut c: Vec<u32> = r.iter().map(|&b| b.into()).collect();
                if i > 0 {
                    c.push((k + i - 1) as u32);
                }
                c.push((k + i) as u32);
                c
            })
            .collect();
        let refs: Vec<&[u32]> = checks.iter().map(Vec::as_slice).collect();
        Self::from_checks(name, 2 * k, k, &refs, InfoAt::Start)
    }

    /// A quasi-cyclic code from its base matrix: `rows[i]` lists `(column group, shift)` for block row `i`; `ncols` column groups
    /// of `z` bits. The information bits are the first `k`. A dense encoder is built when `n − k ≤ 4096`.
    pub fn from_qc(
        name: &str,
        z: usize,
        ncols: usize,
        k: usize,
        rows: &[&[(u16, u16)]],
    ) -> Result<Self> {
        let layers = rows
            .iter()
            .map(|r| r.iter().map(|&(c, s)| (c, s, None)).collect())
            .collect();
        Self::qc_layers(name, z, ncols, k, layers)
    }

    /// A quasi-cyclic code whose circulants can have a hole: `rows[i]` lists `(column group, shift, masked lane)` for block
    /// row `i`. A masked lane `Some(m)` removes the one of check `m` of the block row from that circulant, so check `m` does not
    /// see bit `(m + shift) mod z` of the column group through this entry. Each entry has at most one masked lane. Other rules
    /// and the dense encoder are as [`LdpcCode::from_qc`]. An error when a masked lane is not below `z`.
    pub fn from_qc_masked(
        name: &str,
        z: usize,
        ncols: usize,
        k: usize,
        rows: &[&[(u16, u16, Option<u16>)]],
    ) -> Result<Self> {
        let layers = rows.iter().map(|r| r.to_vec()).collect();
        Self::qc_layers(name, z, ncols, k, layers)
    }

    fn qc_layers(
        name: &str,
        z: usize,
        ncols: usize,
        k: usize,
        rows: Vec<Vec<(u16, u16, Option<u16>)>>,
    ) -> Result<Self> {
        Param::Z.check(z, 1, i64::MAX)?;
        Param::Columns.check(ncols, 1, i64::MAX)?;
        Param::K.check(k, 1, (ncols * z) as i64 - 1)?;
        let mut layers = Vec::with_capacity(rows.len());
        for r in rows {
            let mut layer = Vec::with_capacity(r.len());
            for (c, s, m) in r {
                if c as usize >= ncols {
                    return Param::Column.range(0, ncols as i64 - 1, c);
                }
                let mask = match m {
                    None => NO_MASK,
                    Some(m) if (m as usize) < z && m != NO_MASK => m,
                    Some(_) => return Param::Lane.conflict(Param::Z),
                };
                layer.push(Edge {
                    col: c as u32,
                    shift: (s as usize % z) as u16,
                    mask,
                });
            }
            layers.push(layer);
        }
        let mut code = Self::assemble(
            name.to_string(),
            ncols * z,
            k,
            z,
            ncols,
            layers,
            Vec::new(),
            InfoAt::Start,
        )?;
        code.build_dense_encoder();
        Ok(code)
    }

    /// The 5G NR LDPC code of base graph `bg` (1 or 2) lifted by `z` (3GPP TS 38.212 clause 5.3.2, tables 5.3.2-1 to 5.3.2-3):
    /// `n = 68 z` (BG1) or `52 z` (BG2), `k = 22 z` or `10 z`, the first `2 z` bits never sent. An error when `z` is not a lifting
    /// size of table 5.3.2-1. A dense encoder is built for small `z` only.
    pub fn nr(bg: u8, z: usize) -> Result<Self> {
        use crate::tables_3gpp::{NR_LDPC_BG1, NR_LDPC_BG2, NR_LDPC_LIFTING_SETS};
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
        let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
        let name = format!("nr bg{bg} z{z}");
        Self::from_qc(&name, z, ncols, kb * z, &refs)
    }

    #[allow(clippy::too_many_arguments)]
    fn assemble(
        name: String,
        n: usize,
        k: usize,
        z: usize,
        ncols: usize,
        layers: Vec<Vec<Edge>>,
        perm: Vec<u32>,
        info_at: InfoAt,
    ) -> Result<Self> {
        if layers.is_empty() || layers.iter().any(|l| l.is_empty()) {
            return Param::Checks.degenerate();
        }
        let mut edges = Vec::new();
        let mut layer_start = vec![0u32];
        for l in layers {
            edges.extend_from_slice(&l);
            layer_start.push(edges.len() as u32);
        }
        Ok(LdpcCode {
            name,
            n,
            k,
            z,
            ncols,
            edges,
            layer_start,
            perm,
            info_at,
            encoding: Encoding::None,
        })
    }

    /// The code's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Code word length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Information bits.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Checks per layer (the circulant size); 1 for codes given by check lists.
    pub fn z(&self) -> usize {
        self.z
    }

    /// Number of parity checks.
    pub fn checks(&self) -> usize {
        (self.layer_start.len() - 1) * self.z
    }

    /// Number of edges of the Tanner graph (ones of the parity-check matrix).
    pub fn edges(&self) -> usize {
        self.edges.len() * self.z - self.edges.iter().filter(|e| e.mask != NO_MASK).count()
    }

    pub(crate) fn layers(&self) -> usize {
        self.layer_start.len() - 1
    }

    pub(crate) fn layer(&self, i: usize) -> &[Edge] {
        &self.edges[self.layer_start[i] as usize..self.layer_start[i + 1] as usize]
    }

    pub(crate) fn all_edges(&self) -> &[Edge] {
        &self.edges
    }

    pub(crate) fn ncols(&self) -> usize {
        self.ncols
    }

    /// Code word position of internal bit `i`.
    #[inline]
    pub(crate) fn pos(&self, i: usize) -> usize {
        if self.perm.is_empty() {
            i
        } else {
            self.perm[i] as usize
        }
    }

    /// The code word positions of the information bits.
    pub fn info_range(&self) -> std::ops::Range<usize> {
        match self.info_at {
            InfoAt::Start => 0..self.k,
            InfoAt::End => self.n - self.k..self.n,
        }
    }

    /// Calls `f` with the code word positions of each parity check.
    pub fn for_each_check(&self, mut f: impl FnMut(&[usize])) {
        let mut buf = Vec::new();
        for li in 0..self.layers() {
            for lane in 0..self.z {
                buf.clear();
                for e in self.layer(li) {
                    if e.mask as usize == lane {
                        continue;
                    }
                    let b = e.col as usize * self.z + (lane + e.shift as usize) % self.z;
                    buf.push(self.pos(b));
                }
                f(&buf);
            }
        }
    }

    /// Whether `word` (one bit per byte, code word order) satisfies every check.
    pub fn is_codeword(&self, word: &[u8]) -> bool {
        if word.len() != self.n {
            return false;
        }
        let z = self.z;
        for li in 0..self.layers() {
            for lane in 0..z {
                let mut s = 0u8;
                for e in self.layer(li) {
                    if e.mask as usize != lane {
                        s ^= word[self.pos(e.col as usize * z + (lane + e.shift as usize) % z)];
                    }
                }
                if s & 1 != 0 {
                    return false;
                }
            }
        }
        true
    }

    /// Whether [`LdpcCode::encode`] is available for this code.
    pub fn can_encode(&self) -> bool {
        !matches!(self.encoding, Encoding::None)
    }

    /// Encodes `info` (`k` bits) into `out` (`n` bits, code word order).
    pub fn encode(&self, info: &[u8], out: &mut [u8]) -> Result<()> {
        if info.len() != self.k {
            return Err(FecError::InputLength {
                expected: self.k,
                got: info.len(),
            });
        }
        if out.len() != self.n {
            return Err(FecError::OutputTooShort { need: self.n });
        }
        match &self.encoding {
            Encoding::Dvb(t) => {
                let (n, k, q) = (self.n, self.k, t.q as usize);
                out[..k].copy_from_slice(info);
                let p = &mut out[k..];
                p.fill(0);
                for (g, row) in t.rows.iter().enumerate() {
                    for m in 0..360 {
                        if info[g * 360 + m] & 1 == 1 {
                            for &x in row.iter() {
                                p[(x as usize + m * q) % (n - k)] ^= 1;
                            }
                        }
                    }
                }
                for j in 1..n - k {
                    p[j] ^= p[j - 1];
                }
                Ok(())
            }
            Encoding::Dense { a, words, ppos } => {
                let r = self.info_range();
                for (o, &b) in out[r.clone()].iter_mut().zip(info) {
                    *o = b & 1;
                }
                let mut u = [0u64; 64];
                let u = &mut u[..*words];
                for (i, &b) in info.iter().enumerate() {
                    u[i / 64] |= ((b & 1) as u64) << (i % 64);
                }
                for (j, &pos) in ppos.iter().enumerate() {
                    let row = &a[j * words..(j + 1) * words];
                    let v = row
                        .iter()
                        .zip(u.iter())
                        .fold(0u32, |acc, (x, y)| acc ^ (x & y).count_ones());
                    out[pos as usize] = (v & 1) as u8;
                }
                Ok(())
            }
            Encoding::None => Param::Encoder.unsupported(),
        }
    }

    /// The parity equations `p = A u` as `k`-bit rows (for [`Osd`] and for checking published generators), if built.
    pub fn parity_rows(&self) -> Option<(&[u64], usize)> {
        match &self.encoding {
            Encoding::Dense { a, words, .. } => Some((a, *words)),
            _ => None,
        }
    }

    /// Builds `A` with `p = A u` by Gaussian elimination of `H = [H_u | H_p]` (when small enough and `H_p` has full rank).
    fn build_dense_encoder(&mut self) {
        let (n, k) = (self.n, self.k);
        let r = n - k;
        if r > 4096 || k > 4096 || self.checks() < r {
            return;
        }
        let info = self.info_range();
        let parity: Vec<usize> = (0..n).filter(|i| !info.contains(i)).collect();
        let mut pidx = vec![usize::MAX; n];
        for (j, &p) in parity.iter().enumerate() {
            pidx[p] = j;
        }
        // Rows: [parity part (r bits) | info part (k bits)].
        let pw = r.div_ceil(64);
        let iw = k.div_ceil(64);
        let w = pw + iw;
        let m = self.checks();
        let mut rows = vec![0u64; m * w];
        let mut ci = 0;
        self.for_each_check(|bits| {
            let row = &mut rows[ci * w..(ci + 1) * w];
            for &b in bits {
                if info.contains(&b) {
                    let i = b - info.start;
                    row[pw + i / 64] ^= 1 << (i % 64);
                } else {
                    let j = pidx[b];
                    row[j / 64] ^= 1 << (j % 64);
                }
            }
            ci += 1;
        });
        // Eliminate the parity part to the identity.
        // Column `col` becomes the pivot of row `col` (rank equals the column, or the code has no dense encoder).
        for col in 0..r {
            let rank = col;
            let Some(piv) = (rank..m).find(|&i| rows[i * w + col / 64] >> (col % 64) & 1 == 1)
            else {
                return;
            };
            if piv != rank {
                for x in 0..w {
                    rows.swap(piv * w + x, rank * w + x);
                }
            }
            for i in 0..m {
                if i != rank && rows[i * w + col / 64] >> (col % 64) & 1 == 1 {
                    for x in 0..w {
                        let v = rows[rank * w + x];
                        rows[i * w + x] ^= v;
                    }
                }
            }
        }
        let mut a = vec![0u64; r * iw];
        for j in 0..r {
            a[j * iw..(j + 1) * iw].copy_from_slice(&rows[j * w + pw..(j + 1) * w]);
        }
        if iw <= 64 {
            let ppos = parity.iter().map(|&p| p as u32).collect();
            self.encoding = Encoding::Dense { a, words: iw, ppos };
        }
    }
}
