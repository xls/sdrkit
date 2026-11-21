//! Interleavers: permutations and delay-line (convolutional) interleavers, generic over the item type.
//!
//! - [`Permutation`] is built once (at configure time) from a formula or a table, checked to be a bijection, and applied with no
//!   allocation to any `T: Copy` (hard bits `u8`, `i8` or `f32` LLRs, bytes, symbols).
//! - [`DelayLanes`] holds lanes with fixed delays in one flat buffer: the Forney interleaver of DVB, ISDB-T and ATSC, the DAB time
//!   interleaver, the ISDB-T bit and time delays, and diagonal interleavers are all parameter sets of it. `sdrkit-algo-fec` uses the
//!   same type for its FEC-chain interleavers.
//! - [`combine_copies`] merges the copies of a time-diversity scheme (NAVTEX and SITOR-B DX/RX, DSC, EDACS, ALE).
//!
//! The convention for a permutation is **where each input goes**: `apply` writes `out[dest(i)] = input[i]`, and `invert` undoes
//! it (`out[i] = input[dest(i)]`). Standards that define the reverse ("output j takes input f(j)") use [`Permutation::gather_fn`].
//!
//! ```
//! use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
//! // A 2 x 3 matrix written by rows and read by columns.
//! let p = Permutation::row_column(2, 3, ReadOrder::Columns).unwrap();
//! let mut out = [0u8; 6];
//! p.apply(&[1, 2, 3, 4, 5, 6], &mut out).unwrap();
//! assert_eq!(out, [1, 4, 2, 5, 3, 6]);
//! let mut back = [0u8; 6];
//! p.invert(&out, &mut back).unwrap();
//! assert_eq!(back, [1, 2, 3, 4, 5, 6]);
//! ```

use crate::Param;
use crate::{Error, Llr, Result};

/// How a row-column matrix is read out after being written row by row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOrder {
    /// Column by column (the usual row-column interleaver).
    Columns,
    /// Row by row (no change; useful with a row or column permutation).
    Rows,
}

/// A permutation of `n` items, with its inverse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Permutation {
    dest: Vec<u32>,
    src: Vec<u32>,
}

impl Permutation {
    /// The permutation that sends input `i` to output `dest[i]`; an error unless `dest` holds each of `0..n` exactly once.
    pub fn from_dest(dest: Vec<u32>) -> Result<Self> {
        let n = dest.len();
        if n > u32::MAX as usize {
            return Err(Error::BadLength);
        }
        let mut src = vec![u32::MAX; n];
        for (i, &d) in dest.iter().enumerate() {
            let slot = src.get_mut(d as usize).ok_or(Error::Invalid { at: i })?;
            if *slot != u32::MAX {
                return Err(Error::Invalid { at: i });
            }
            *slot = i as u32;
        }
        Ok(Permutation { dest, src })
    }

    /// Input `i` goes to output `f(i)`.
    pub fn from_fn(n: usize, f: impl Fn(usize) -> usize) -> Result<Self> {
        Self::from_dest((0..n).map(|i| f(i) as u32).collect())
    }

    /// Output `j` takes input `f(j)`.
    pub fn gather_fn(n: usize, f: impl Fn(usize) -> usize) -> Result<Self> {
        Ok(Self::from_fn(n, f)?.inverse())
    }

    /// From a table of destinations (input `i` goes to `t[i]`).
    pub fn from_table(t: &[u16]) -> Result<Self> {
        Self::from_dest(t.iter().map(|&x| x as u32).collect())
    }

    /// The identity on `n` items.
    pub fn identity(n: usize) -> Self {
        let v: Vec<u32> = (0..n as u32).collect();
        Permutation {
            dest: v.clone(),
            src: v,
        }
    }

    /// The inverse permutation.
    pub fn inverse(self) -> Self {
        Permutation {
            dest: self.src,
            src: self.dest,
        }
    }

    /// `self` then `next` (both of the same length).
    pub fn then(&self, next: &Permutation) -> Result<Self> {
        if self.len() != next.len() {
            return Err(Error::BadLength);
        }
        Self::from_dest(self.dest.iter().map(|&d| next.dest[d as usize]).collect())
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.dest.len()
    }

    /// Whether it has no items.
    pub fn is_empty(&self) -> bool {
        self.dest.is_empty()
    }

    /// Where input `i` goes.
    pub fn dest(&self, i: usize) -> Option<usize> {
        self.dest.get(i).map(|&d| d as usize)
    }

    /// Which input output `j` takes.
    pub fn src(&self, j: usize) -> Option<usize> {
        self.src.get(j).map(|&s| s as usize)
    }

    /// The destination table.
    pub fn dests(&self) -> &[u32] {
        &self.dest
    }

    /// Interleaves: `out[dest(i)] = input[i]`. Both slices must be exactly `len()` long.
    #[inline]
    pub fn apply<T: Copy>(&self, input: &[T], out: &mut [T]) -> Result<()> {
        if input.len() != self.len() || out.len() != self.len() {
            return Err(Error::BadLength);
        }
        // Gathering through the inverse writes `out` in order, which is faster than scattering.
        for (o, &s) in out.iter_mut().zip(&self.src) {
            *o = input[s as usize];
        }
        Ok(())
    }

    /// Deinterleaves: `out[i] = input[dest(i)]`.
    #[inline]
    pub fn invert<T: Copy>(&self, input: &[T], out: &mut [T]) -> Result<()> {
        if input.len() != self.len() || out.len() != self.len() {
            return Err(Error::BadLength);
        }
        for (o, &d) in out.iter_mut().zip(&self.dest) {
            *o = input[d as usize];
        }
        Ok(())
    }

    // ----- generic forms -----

    /// Written row by row into `rows` x `cols`, read as `read` says.
    pub fn row_column(rows: usize, cols: usize, read: ReadOrder) -> Result<Self> {
        Self::rows_cols_permuted(rows, cols, read, |r| r, |c| c)
    }

    /// Written row by row into `rows` x `cols`; written row `r` becomes row `row_of(r)` and column `c` becomes column
    /// `col_of(c)`; then read as `read` says. The Aero (64 rows, 27 i mod 64) and Inmarsat-C (39 j mod 64) interleavers are of
    /// this form.
    pub fn rows_cols_permuted(
        rows: usize,
        cols: usize,
        read: ReadOrder,
        row_of: impl Fn(usize) -> usize,
        col_of: impl Fn(usize) -> usize,
    ) -> Result<Self> {
        let n = rows.checked_mul(cols).ok_or(Error::BadLength)?;
        Self::from_fn(n, |i| {
            let (r, c) = (row_of(i / cols), col_of(i % cols));
            match read {
                ReadOrder::Columns => c * rows + r,
                ReadOrder::Rows => r * cols + c,
            }
        })
    }

    /// The multiplicative interleaver: input `i` goes to `(a i + b) mod k`. `a` must be prime to `k`. TETRA (EN 300 392-2 clause
    /// 8.2.4.1, "bit i goes to 1 + (a i mod K)" counted from 1) is `b = a`; DMR BPTC is `a = 181, k = 196`; DMR short LC
    /// `a = 4, k = 67`; D2-MAC `a = 94, k = 751`.
    pub fn multiplicative(a: usize, b: usize, k: usize) -> Result<Self> {
        if k == 0 || gcd(a % k, k) != 1 {
            return Err(Error::BadParameter(Param::MultiplierNotPrime));
        }
        Self::from_fn(k, |i| {
            ((a as u128 * i as u128 + b as u128) % k as u128) as usize
        })
    }

    /// The quadratic permutation polynomial `(f1 i + f2 i^2) mod k`, as a destination (M17: input `i` goes to it, f1 = 45, f2 =
    /// 92, k = 368). LTE turbo (3GPP TS 36.212 clause 5.1.3.2.3) defines `c'_i = c_(Π(i))`, which is [`Permutation::invert`] of
    /// this (or `apply` of its [`Permutation::inverse`]).
    pub fn qpp(f1: usize, f2: usize, k: usize) -> Result<Self> {
        Self::from_fn(k, |i| {
            let (i, k) = (i as u128, k as u128);
            ((f1 as u128 * i + f2 as u128 * i * i) % k) as usize
        })
    }

    /// The bit-reversal interleaver of WSPR (n = 162) and JT9 (n = 206): input `p` goes to the `p`-th 8-bit bit-reversed value
    /// below `n`. `n` at most 256.
    pub fn bit_reverse8(n: usize) -> Result<Self> {
        if n > 256 {
            return Err(Error::BadParameter(Param::BitReversalLength));
        }
        let order: Vec<u32> = (0..256u32)
            .map(|i| (i as u8).reverse_bits() as u32)
            .filter(|&j| (j as usize) < n)
            .collect();
        Self::from_dest(order)
    }

    /// The 3GPP sub-block interleaver (TS 36.212 clause 5.1.4.1.1 and 5.1.4.2.1) of `d` items with the column pattern `pattern`
    /// ([`LTE_SUBBLOCK_TURBO`] or [`LTE_SUBBLOCK_CONV`]): 32 columns, the items written row by row after the dummy items that
    /// fill the first row, columns permuted, read column by column, dummy items left out.
    pub fn subblock_3gpp(d: usize, pattern: &[u8; 32]) -> Result<Self> {
        let r = d.div_ceil(32);
        let nd = 32 * r - d;
        let mut src = Vec::with_capacity(d);
        for &p in pattern {
            for row in 0..r {
                let y = row * 32 + p as usize;
                if y >= nd {
                    src.push((y - nd) as u32);
                }
            }
        }
        Ok(Self::from_dest(src)?.inverse())
    }

    /// The third-stream sub-block interleaver of LTE turbo rate matching (TS 36.212 clause 5.1.4.1.1, d(2)): output `k` of the
    /// `32 R` positions takes `(P(k / R) + 32 (k mod R) + 1) mod 32 R`, dummy items left out.
    pub fn subblock_3gpp_parity2(d: usize) -> Result<Self> {
        let r = d.div_ceil(32);
        let kpi = 32 * r;
        let nd = kpi - d;
        let mut src = Vec::with_capacity(d);
        for k in 0..kpi {
            let y = (LTE_SUBBLOCK_TURBO[k / r] as usize + 32 * (k % r) + 1) % kpi;
            if y >= nd {
                src.push((y - nd) as u32);
            }
        }
        Ok(Self::from_dest(src)?.inverse())
    }

    // ----- named standards -----

    /// The IEEE 802.11 BCC interleaver (IEEE 802.11-2020 equations 17-17 and 17-18, and the HT/VHT forms with `n_col` columns):
    /// coded bit `k` of a symbol goes to `j`. `n_col` is 16 for non-HT OFDM, 13 or 18 for HT 20/40 MHz.
    pub fn wifi(n_cbps: usize, n_bpsc: usize, n_col: usize) -> Result<Self> {
        if n_col == 0 || !n_cbps.is_multiple_of(n_col) {
            return Err(Error::BadParameter(Param::CodedBitsPerSymbol));
        }
        let s = (n_bpsc / 2).max(1);
        Self::from_fn(n_cbps, |k| {
            let i = (n_cbps / n_col) * (k % n_col) + k / n_col;
            s * (i / s) + (i + n_cbps - (n_col * i / n_cbps)) % s
        })
    }

    /// The DRM pseudo-random permutation (ETSI ES 201 980 clause 7.3.3) of `n` items with multiplier `t0` (5 for bits, 13 or 21
    /// for cells): `s = 2^ceil(log2 n)`, `q = s/4 - 1`, `Π(0) = 0`, `Π(i) = (t0 Π(i-1) + q) mod s`, values of `n` or more skipped;
    /// input `i` goes to output `Π(i)`.
    pub fn drm(n: usize, t0: usize) -> Result<Self> {
        if n == 0 {
            return Ok(Self::identity(0));
        }
        let s = n.next_power_of_two().max(4);
        let q = s / 4 - 1;
        let mut dest = Vec::with_capacity(n);
        dest.push(0u32);
        let mut p = 0usize;
        let mut guard = 0usize;
        while dest.len() < n {
            p = (t0 * p + q) % s;
            if p < n {
                dest.push(p as u32);
            }
            guard += 1;
            if guard > 4 * s {
                return Err(Error::BadParameter(Param::NotAPermutation));
            }
        }
        Self::from_dest(dest)
    }

    /// The DVB-T bit interleaver of branch `e` (0 to 5) (ETSI EN 300 744 clause 4.3.4.1): `H_e(w) = (w + offset_e) mod 126`, with
    /// offsets 0, 63, 105, 42, 21, 84; output `w` takes input `H_e(w)`.
    pub fn dvbt_bit(e: usize) -> Result<Self> {
        let off = *DVBT_BIT_OFFSETS
            .get(e)
            .ok_or(Error::BadParameter(Param::DvbtBranch))?;
        Self::gather_fn(126, |w| (w + off) % 126)
    }

    /// The DVB-T symbol interleaver H(q) (ETSI EN 300 744 clause 4.3.4.2) for the 2K (`mode_8k` false) or 8K mode: input `q` of a
    /// symbol goes to carrier `H(q)` (the even-symbol direction).
    pub fn dvbt_symbol(mode_8k: bool) -> Result<Self> {
        let (nr, mmax, nmax): (u32, usize, usize) = if mode_8k {
            (13, 8192, 6048)
        } else {
            (11, 2048, 1512)
        };
        // R' bit j goes to R bit perm[j].
        let perm: &[u32] = if mode_8k {
            &[7, 1, 4, 2, 9, 6, 8, 10, 0, 3, 11, 5]
        } else {
            &[4, 3, 9, 6, 2, 8, 1, 5, 7, 0]
        };
        let bits = nr - 1;
        let mut rp: u32 = 0;
        let mut h = Vec::with_capacity(nmax);
        for i in 0..mmax {
            if i < 2 {
                rp = 0;
            } else if i == 2 {
                rp = 1;
            } else {
                let fb = if mode_8k {
                    (rp ^ (rp >> 1) ^ (rp >> 4) ^ (rp >> 6)) & 1
                } else {
                    (rp ^ (rp >> 3)) & 1
                };
                rp = (rp >> 1) | (fb << (bits - 1));
            }
            let mut r = 0u32;
            for (j, &to) in perm.iter().enumerate() {
                r |= ((rp >> j) & 1) << to;
            }
            let v = (((i % 2) as u32) << bits) | r;
            if (v as usize) < nmax {
                h.push(v);
            }
        }
        Self::from_dest(h)
    }

    /// The NICAM 728 bit interleaver (ETSI EN 300 163 clause 3.6): transmitted bit `t` (0 to 703) carries structure bit
    /// `t/16 + 44 (t mod 16)`.
    pub fn nicam() -> Result<Self> {
        Self::gather_fn(704, |t| t / 16 + 44 * (t % 16))
    }

    /// The GSM interleaver of the control channels (3GPP TS 45.003 clause 4.1.4): coded bit `k` of 456 goes to burst `k mod 4`,
    /// position `2 ((49 k) mod 57) + ((k mod 8) div 4)`; output index `burst * 114 + position`.
    pub fn gsm_xcch() -> Result<Self> {
        Self::from_fn(456, |k| (k % 4) * 114 + 2 * ((49 * k) % 57) + (k % 8) / 4)
    }

    /// The HFDL interleaver (ARINC 635 clause 4.4.6) for `n` coded bits (a multiple of 40) and the column `shift` of the data
    /// rate: coded bits are written into a 40-row matrix with a row step of 9, and read down the rows with the column moving back
    /// by `shift` after every bit and on by one after each pass.
    pub fn hfdl(n: usize, shift: usize) -> Result<Self> {
        if n == 0 || !n.is_multiple_of(40) {
            return Err(Error::BadParameter(Param::HfdlLength));
        }
        let cols = n / 40;
        let fetch = |i: usize| ((i * 9) % 40) * cols + i / 40;
        let load = |j: usize| {
            let row = j % 40;
            let pass = j / 40;
            let back = (j % cols) * (shift % cols) % cols;
            let col = (pass % cols + cols - back) % cols;
            row * cols + col
        };
        let into_matrix = Self::from_fn(n, fetch)?;
        let out_of_matrix = Self::gather_fn(n, load)?;
        into_matrix.then(&out_of_matrix)
    }

    /// The LoRa diagonal interleaver (Semtech LoRa modem; rows = SF, or SF - 2 in reduced rate; cols = CR + 4): input index
    /// `r * cols + i` (codeword `r`, its bit `i` counted from the most significant) goes to output `i * rows + j` (symbol `i`, its
    /// bit `j` counted from the most significant), with `j = (i - r - 1) mod rows`.
    pub fn lora(rows: usize, cols: usize) -> Result<Self> {
        if rows == 0 || cols == 0 {
            return Err(Error::BadParameter(Param::LoraInterleaver));
        }
        Self::from_fn(rows * cols, |x| {
            let (r, i) = (x / cols, x % cols);
            let j = (i + 2 * rows - r - 1) % rows;
            i * rows + j
        })
    }

    /// The NR PBCH payload interleaver (3GPP TS 38.212 clause 7.1.1, table 7.1.1-1 pattern G) for the 32 payload bits: input `i`
    /// goes to `G(j)` where `j` counts the bits of each kind as the clause says. `kind(i)` returns the kind of input bit `i`: 0 for the
    /// SFN bits other than the 2nd and 3rd LSBs, 1 for the half-frame bit, 2 for the SSB index bits, 3 for the others.
    pub fn nr_pbch(kind: impl Fn(usize) -> u8) -> Result<Self> {
        let (mut js, mut jo, mut jb) = (0usize, 14usize, 11usize);
        let mut dest = Vec::with_capacity(32);
        for i in 0..32 {
            let j = match kind(i) {
                0 => {
                    js += 1;
                    js - 1
                }
                1 => 10,
                2 => {
                    jb += 1;
                    jb - 1
                }
                _ => {
                    jo += 1;
                    jo - 1
                }
            };
            dest.push(
                *NR_PBCH_G
                    .get(j)
                    .ok_or(Error::BadParameter(Param::PbchBitKinds))? as u32,
            );
        }
        Self::from_dest(dest)
    }

    /// The DMR rate 3/4 trellis dibit interleaver (ETSI TS 102 361-1 clause B.2.2, 98 dibits): transmitted dibit `i` carries coded
    /// dibit [`DMR_TRELLIS_98`]`[i]`.
    pub fn dmr_trellis() -> Result<Self> {
        Self::gather_fn(98, |i| DMR_TRELLIS_98[i] as usize)
    }
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// The column pattern of the LTE turbo sub-block interleaver (3GPP TS 36.212 table 5.1.4-1).
pub const LTE_SUBBLOCK_TURBO: [u8; 32] = [
    0, 16, 8, 24, 4, 20, 12, 28, 2, 18, 10, 26, 6, 22, 14, 30, 1, 17, 9, 25, 5, 21, 13, 29, 3, 19,
    11, 27, 7, 23, 15, 31,
];

/// The column pattern of the LTE convolutional-code sub-block interleaver (3GPP TS 36.212 table 5.1.4-2).
pub const LTE_SUBBLOCK_CONV: [u8; 32] = [
    1, 17, 9, 25, 5, 21, 13, 29, 3, 19, 11, 27, 7, 23, 15, 31, 0, 16, 8, 24, 4, 20, 12, 28, 2, 18,
    10, 26, 6, 22, 14, 30,
];

/// The DVB-T bit interleaver offsets of the six branches (ETSI EN 300 744 clause 4.3.4.1).
pub const DVBT_BIT_OFFSETS: [usize; 6] = [0, 63, 105, 42, 21, 84];

/// The NR PBCH interleaver pattern G(j) (3GPP TS 38.212 table 7.1.1-1).
pub const NR_PBCH_G: [u8; 32] = [
    16, 23, 18, 17, 8, 30, 10, 6, 24, 7, 0, 5, 3, 2, 1, 4, 9, 11, 12, 13, 14, 15, 19, 20, 21, 22,
    25, 26, 27, 28, 29, 31,
];

/// The DMR rate 3/4 trellis interleave (ETSI TS 102 361-1 clause B.2.2, "Trellis interleave"): transmitted dibit `i` carries coded
/// dibit `DMR_TRELLIS_98[i]`.
pub const DMR_TRELLIS_98: [u8; 98] = [
    0, 1, 8, 9, 16, 17, 24, 25, 32, 33, 40, 41, 48, 49, 56, 57, 64, 65, 72, 73, 80, 81, 88, 89, 96,
    97, 2, 3, 10, 11, 18, 19, 26, 27, 34, 35, 42, 43, 50, 51, 58, 59, 66, 67, 74, 75, 82, 83, 90,
    91, 4, 5, 12, 13, 20, 21, 28, 29, 36, 37, 44, 45, 52, 53, 60, 61, 68, 69, 76, 77, 84, 85, 92,
    93, 6, 7, 14, 15, 22, 23, 30, 31, 38, 39, 46, 47, 54, 55, 62, 63, 70, 71, 78, 79, 86, 87, 94,
    95,
];

/// The DAB mode I frequency interleaver (ETSI EN 300 401 clause 14.6.1): `Π(i) = (13 Π(i-1) + 511) mod 2048` from 0, keeping
/// values from 256 to 1792 except 1024. Appends, for each of the 1536 QPSK symbols in order, its carrier index `k = Π - 1024`
/// (from -768 to 768, never 0).
pub fn dab_frequency_mode1(out: &mut Vec<i16>) {
    let mut pi = 0i32;
    for _ in 0..2048 {
        if (256..=1792).contains(&pi) && pi != 1024 {
            out.push((pi - 1024) as i16);
        }
        pi = (13 * pi + 511) % 2048;
    }
}

/// Lanes with fixed delays in one flat buffer. Items are dealt to the lanes round robin; an item entering lane `i` comes out
/// `delay[i]` visits of that lane later, the lane first giving out its fill value. No allocation after [`DelayLanes::new`].
///
/// ```
/// use sdrkit_util_bits::interleave::DelayLanes;
/// // DVB outer interleaver, I = 12, M = 17 (ETSI EN 300 421 clause 4.4.3): transmit and receive together delay every byte by
/// // 12 x 11 x 17 = 2244 bytes.
/// let mut tx = DelayLanes::forney(12, 17, false, 0u8);
/// let mut rx = DelayLanes::forney(12, 17, true, 0u8);
/// let data: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8 + 1).collect();
/// let mut line = data.clone();
/// tx.process(&mut line);
/// rx.process(&mut line);
/// assert_eq!(&line[2244..], &data[..5000 - 2244]);
/// ```
#[derive(Clone, Debug)]
pub struct DelayLanes<T: Copy> {
    buf: Vec<T>,
    start: Vec<u32>,
    len: Vec<u32>,
    pos: Vec<u32>,
    lane: usize,
}

impl<T: Copy> DelayLanes<T> {
    /// Lanes with the given delays (in visits of the lane), filled with `fill`.
    pub fn new(delays: &[u32], fill: T) -> Self {
        let mut start = Vec::with_capacity(delays.len());
        let mut total = 0u32;
        for &d in delays {
            start.push(total);
            total += d;
        }
        DelayLanes {
            buf: vec![fill; total as usize],
            start,
            len: delays.to_vec(),
            pos: vec![0; delays.len()],
            lane: 0,
        }
    }

    /// The Forney convolutional interleaver of `b` branches with unit delay `m`: branch `i` delays by `i m` when transmitting and
    /// by `(b - 1 - i) m` when receiving. DVB-C/S/T and ISDB-T: `b = 12, m = 17`; ATSC: `b = 52, m = 4`.
    pub fn forney(b: u32, m: u32, receive: bool, fill: T) -> Self {
        let delays: Vec<u32> = (0..b)
            .map(|i| if receive { (b - 1 - i) * m } else { i * m })
            .collect();
        Self::new(&delays, fill)
    }

    /// The DAB time interleaver (ETSI EN 300 401 clause 12): bit `i` of each logical frame of `cif_bits` bits (a multiple of 16)
    /// is delayed by `rev4(i mod 16)` logical frames, `rev4` reversing the 4-bit index (0, 8, 4, 12, 2, 10, 6, 14, 1, 9, 5, 13, 3,
    /// 11, 7, 15). With `receive`, the delays are `15 - rev4(i mod 16)`, so sender and receiver together delay every bit by 15
    /// frames. An error when `cif_bits` is not a multiple of 16.
    pub fn dab_time(cif_bits: u32, receive: bool, fill: T) -> Result<Self> {
        if cif_bits == 0 || !cif_bits.is_multiple_of(16) {
            return Err(Error::BadParameter(Param::DabFrameLength));
        }
        let per = cif_bits / 16;
        let delays: Vec<u32> = (0..16u32)
            .map(|i| {
                let r = (i as u8).reverse_bits() as u32 >> 4;
                per * if receive { 15 - r } else { r }
            })
            .collect();
        Ok(Self::new(&delays, fill))
    }

    /// Number of lanes.
    pub fn lanes(&self) -> usize {
        self.len.len()
    }

    /// The lane the next item enters.
    pub fn lane(&self) -> usize {
        self.lane
    }

    /// Puts `x` into the current lane and returns what comes out of it; moves to the next lane.
    #[inline]
    pub fn push(&mut self, x: T) -> T {
        if self.len.is_empty() {
            return x;
        }
        let l = self.lane;
        let n = self.len[l];
        let out = if n == 0 {
            x
        } else {
            let k = (self.start[l] + self.pos[l]) as usize;
            let o = self.buf[k];
            self.buf[k] = x;
            self.pos[l] = if self.pos[l] + 1 == n {
                0
            } else {
                self.pos[l] + 1
            };
            o
        };
        self.lane = if l + 1 == self.len.len() { 0 } else { l + 1 };
        out
    }

    /// Replaces each item of `xs` by what [`DelayLanes::push`] returns for it.
    pub fn process(&mut self, xs: &mut [T]) {
        for x in xs {
            *x = self.push(*x);
        }
    }

    /// Makes the next item enter lane 0 (at a DVB sync byte or an ATSC segment start). The lanes keep their contents.
    pub fn sync(&mut self) {
        self.lane = 0;
    }

    /// Fills every lane with `fill` and goes back to lane 0.
    pub fn reset(&mut self, fill: T) {
        self.buf.fill(fill);
        self.pos.fill(0);
        self.lane = 0;
    }
}

/// Merges the copies of a time-diversity scheme into one LLR per bit: the LLRs add, and a copy sent inverted (`inverted[c]`) is
/// flipped first (EDACS sends its middle copy inverted). Each copy must have the length of `out`. An error otherwise.
pub fn combine_copies<T: Llr>(copies: &[&[T]], inverted: &[bool], out: &mut [T]) -> Result<()> {
    if copies.iter().any(|c| c.len() != out.len()) || inverted.len() != copies.len() {
        return Err(Error::BadLength);
    }
    for (i, o) in out.iter_mut().enumerate() {
        let mut acc = T::ZERO;
        for (c, &inv) in copies.iter().zip(inverted) {
            let v = c[i];
            acc = acc.add_llr(if inv { v.flip() } else { v });
        }
        *o = acc;
    }
    Ok(())
}

/// Majority vote of hard copies (an odd number of them), each possibly inverted. Ties (an even number) decide 0.
pub fn vote_copies(copies: &[&[u8]], inverted: &[bool], out: &mut [u8]) -> Result<()> {
    if copies.iter().any(|c| c.len() != out.len()) || inverted.len() != copies.len() {
        return Err(Error::BadLength);
    }
    for (i, o) in out.iter_mut().enumerate() {
        let ones = copies
            .iter()
            .zip(inverted)
            .filter(|(c, &inv)| (c[i] & 1 == 1) != inv)
            .count();
        *o = (2 * ones > copies.len()) as u8;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Soft;

    fn is_bijection(p: &Permutation) {
        let data: Vec<u32> = (0..p.len() as u32).collect();
        let mut out = vec![0u32; p.len()];
        p.apply(&data, &mut out).unwrap();
        let mut sorted = out.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, data);
        let mut back = vec![0u32; p.len()];
        p.invert(&out, &mut back).unwrap();
        assert_eq!(back, data);
    }

    #[test]
    fn every_constructor_returns_a_bijection() {
        let all = [
            Permutation::row_column(20, 10, ReadOrder::Columns).unwrap(),
            Permutation::rows_cols_permuted(64, 5, ReadOrder::Columns, |r| (27 * r) % 64, |c| c)
                .unwrap(),
            Permutation::multiplicative(181, 0, 196).unwrap(),
            Permutation::multiplicative(4, 0, 67).unwrap(),
            Permutation::multiplicative(94, 0, 751).unwrap(),
            Permutation::qpp(45, 92, 368).unwrap(),
            Permutation::qpp(43, 88, 1408).unwrap(),
            Permutation::bit_reverse8(162).unwrap(),
            Permutation::bit_reverse8(206).unwrap(),
            Permutation::subblock_3gpp(40, &LTE_SUBBLOCK_CONV).unwrap(),
            Permutation::subblock_3gpp(100, &LTE_SUBBLOCK_TURBO).unwrap(),
            Permutation::subblock_3gpp_parity2(100).unwrap(),
            Permutation::wifi(48, 1, 16).unwrap(),
            Permutation::wifi(288, 6, 16).unwrap(),
            Permutation::wifi(52 * 4, 4, 13).unwrap(),
            Permutation::drm(1000, 5).unwrap(),
            Permutation::drm(2000, 13).unwrap(),
            Permutation::dvbt_bit(3).unwrap(),
            Permutation::dvbt_symbol(false).unwrap(),
            Permutation::dvbt_symbol(true).unwrap(),
            Permutation::nicam().unwrap(),
            Permutation::gsm_xcch().unwrap(),
            Permutation::hfdl(1800, 17).unwrap(),
            Permutation::lora(7, 8).unwrap(),
            Permutation::nr_pbch(|i| match i {
                0..=9 => 0,
                10 => 1,
                11..=13 => 2,
                _ => 3,
            })
            .unwrap(),
            Permutation::dmr_trellis().unwrap(),
        ];
        for p in &all {
            is_bijection(p);
        }
    }

    #[test]
    fn bad_parameters_are_errors() {
        assert!(Permutation::multiplicative(4, 0, 64).is_err());
        assert!(Permutation::from_dest(vec![0, 0]).is_err());
        assert!(Permutation::from_dest(vec![0, 2]).is_err());
        assert!(Permutation::qpp(2, 0, 4).is_err()); // not a permutation
        assert!(Permutation::bit_reverse8(300).is_err());
        assert!(Permutation::wifi(50, 1, 16).is_err());
        assert!(Permutation::dvbt_bit(6).is_err());
        assert!(Permutation::hfdl(41, 1).is_err());
        let p = Permutation::identity(4);
        assert_eq!(p.apply(&[1, 2, 3], &mut [0; 4]), Err(Error::BadLength));
    }

    #[test]
    fn standards_values() {
        // 802.11a BPSK 6 Mb/s: coded bit k goes to 3 (k mod 16) + k / 16 (equation 17-17 with s = 1).
        let p = Permutation::wifi(48, 1, 16).unwrap();
        assert_eq!(&p.dests()[..4], &[0, 3, 6, 9]);
        assert_eq!(p.dest(16), Some(1));
        // DRM permutation of 4 items, t0 = 5: s = 4, q = 0: 0, 0, ... the sequence must skip; of 13 items with t0 = 5:
        // s = 16, q = 3: 0, 3, 2, 13 (skipped), 4, 7, 6, 1, 8, 11, 10, 5, 12, 15 (skipped), 14 (skipped), 9.
        let d = Permutation::drm(13, 5).unwrap();
        assert_eq!(d.dests(), &[0, 3, 2, 4, 7, 6, 1, 8, 11, 10, 5, 12, 9]);
        // DVB-T 2K: H(q) starts 0, 1, ... no: the first kept values of the R' generator (EN 300 744 clause 4.3.4.2).
        let h = Permutation::dvbt_symbol(false).unwrap();
        assert_eq!(h.len(), 1512);
        assert_eq!(&h.dests()[..2], &[0, 1024]);
        // GSM: bit 0 to burst 0 position 0; bit 1 to burst 1 position 2 * 49 = 98.
        let g = Permutation::gsm_xcch().unwrap();
        assert_eq!(g.dest(1), Some(114 + 98));
        // NICAM: transmitted bit 1 carries structure bit 44.
        assert_eq!(Permutation::nicam().unwrap().src(1), Some(44));
        // WSPR: the first inputs go to 0, 128, 64, 32, 160, 96 (bit reversal skipping values of 162 and up).
        assert_eq!(
            &Permutation::bit_reverse8(162).unwrap().dests()[..6],
            &[0, 128, 64, 32, 160, 96]
        );
        // DAB mode I frequency interleaver: 1536 carriers, none at 0, each once.
        let mut k = Vec::new();
        dab_frequency_mode1(&mut k);
        assert_eq!(k.len(), 1536);
        assert!(!k.contains(&0));
        let mut s = k.clone();
        s.sort_unstable();
        s.dedup();
        assert_eq!(s.len(), 1536);
        // TETRA, K = 432, a = 103: bit 1 (counted from 1) goes to 1 + 103.
        assert_eq!(
            Permutation::multiplicative(103, 103, 432).unwrap().dest(0),
            Some(103)
        );
        // LTE convolutional sub-block interleaver, D = 40: R = 2, 24 dummy items, column 1 then 17 ... first output is item 1 - 24
        // ... the first real item read is row 1 column 1: y = 33 - 24 = 9.
        let sb = Permutation::subblock_3gpp(40, &LTE_SUBBLOCK_CONV).unwrap();
        assert_eq!(sb.src(0), Some(9));
    }

    #[test]
    fn forney_and_lanes() {
        let mut tx = DelayLanes::forney(52, 4, false, 0i8);
        let mut rx = DelayLanes::forney(52, 4, true, 0i8);
        let data: Vec<i8> = (0..30_000)
            .map(|i| ((i % 200) as i8).wrapping_add(1))
            .collect();
        let mut line = data.clone();
        tx.process(&mut line);
        rx.process(&mut line);
        let d = 52 * 51 * 4;
        assert_eq!(&line[d..], &data[..data.len() - d]);
        let mut lanes = DelayLanes::new(&[0, 2, 1], 9.0f32);
        let out: Vec<f32> = (1..=9).map(|x| lanes.push(x as f32)).collect();
        assert_eq!(out, [1.0, 9.0, 9.0, 4.0, 9.0, 3.0, 7.0, 2.0, 6.0]);
        lanes.sync();
        assert_eq!(lanes.lane(), 0);
        lanes.reset(0.0);
        assert_eq!(lanes.push(5.0), 5.0);
        assert_eq!(lanes.push(1.0), 0.0);
        // DAB: 15 logical frames end to end.
        let cif = 64u32;
        let mut tx = DelayLanes::dab_time(cif, false, 0u8).unwrap();
        let mut rx = DelayLanes::dab_time(cif, true, 0u8).unwrap();
        let data: Vec<u8> = (0..cif * 40).map(|i| (i % 7 + 1) as u8).collect();
        let mut line = data.clone();
        tx.process(&mut line);
        rx.process(&mut line);
        let d = 15 * cif as usize;
        assert_eq!(&line[d..], &data[..data.len() - d]);
        assert!(DelayLanes::dab_time(10, false, 0u8).is_err());
        let mut none = DelayLanes::<u8>::new(&[], 0);
        assert_eq!(none.push(3), 3);
    }

    #[test]
    fn diversity_combining() {
        let a = [10i8, -10, 5];
        let b = [-10i8, 10, 5]; // sent inverted
        let c = [8i8, -2, -20];
        let mut out = [0i8; 3];
        combine_copies(&[&a, &b, &c], &[false, true, false], &mut out).unwrap();
        assert_eq!(out, [28, -22, -20]);
        let mut h = [0u8; 3];
        vote_copies(
            &[&[1, 0, 1], &[0, 1, 1], &[1, 0, 0]],
            &[false, true, false],
            &mut h,
        )
        .unwrap();
        assert_eq!(h, [1, 0, 0]);
        assert!(combine_copies(&[&a[..2]], &[false], &mut out).is_err());
        assert_eq!(out[0].hard(), 0);
    }
}
