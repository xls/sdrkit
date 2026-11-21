//! A sparse LDPC encoder for codes of any size: back-substitution through the parity checks, with a small dense solve for the
//! parity bits that back-substitution cannot reach.
//!
//! Standard: T. Richardson and R. Urbanke, "Efficient encoding of low-density parity-check codes", IEEE Trans. Inf. Theory 47,
//! 2001 (the greedy triangulation with a gap).

use super::LdpcCode;
use crate::error::{FecError, Param, Result};
use std::ops::BitXor;

/// The most parity bits the dense solve takes (64 words of 64 bits).
const MAX_GAP: usize = 4096;

/// An encoder for one LDPC code, built once from its parity checks.
///
/// Construction orders the checks so that each one solves one parity bit from bits already known (back-substitution). When no
/// check has a single unknown parity bit left, one unknown bit becomes a "gap" bit (a bit found later by a dense solve). For
/// dual-diagonal and lower-triangular parity parts (DVB, most broadcast codes) there are no gap bits. For the 5G NR base
/// graphs there is one. Each encode runs the order once, solves the gap bits from the unused checks, and runs the order again when a gap
/// bit is 1. The encoder honours masked lanes and any information layout ([`LdpcCode::info_range`]).
///
/// ```
/// use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcEncoder};
/// let code = LdpcCode::nr(2, 384).unwrap();
/// assert!(!code.can_encode());
/// let enc = LdpcEncoder::new(&code).unwrap();
/// let info: Vec<u8> = (0..code.k()).map(|i| (i % 3 == 0) as u8).collect();
/// let mut cw = vec![0u8; code.n()];
/// enc.encode(&info, &mut cw).unwrap();
/// assert!(code.is_codeword(&cw));
/// ```
#[derive(Clone, Debug)]
pub struct LdpcEncoder {
    n: usize,
    k: usize,
    info_start: usize,
    /// The parity bit each step solves (code word position).
    solve: Vec<u32>,
    /// The other bits of the check of each step, then all bits of each pivot check.
    bits: Vec<u32>,
    /// Start of each step's and each pivot check's bits in `bits`.
    start: Vec<u32>,
    /// Code word positions of the gap bits.
    gap: Vec<u32>,
    /// The inverse of the gap system: `x = minv s`, `gap.len()` rows of `gw` words.
    minv: Vec<u64>,
    gw: usize,
}

/// A small fixed random generator for the self-check (SplitMix64).
fn splitmix(s: &mut u64) -> u64 {
    *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *s;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl LdpcEncoder {
    /// Builds the encoder of `code`. An error when the parity part of the checks is singular (no unique parity for some
    /// information), when the checks have rank above `n − k`, or when more than 4096 parity bits need the dense solve.
    pub fn new(code: &LdpcCode) -> Result<Self> {
        let (n, k) = (code.n(), code.k());
        let info = code.info_range();
        let is_info = |p: usize| info.contains(&p);
        // The checks as sorted position lists; a bit listed twice in one check cancels.
        let mut cbits: Vec<u32> = Vec::new();
        let mut cstart = vec![0u32];
        let mut tmp: Vec<u32> = Vec::new();
        code.for_each_check(|b| {
            tmp.clear();
            tmp.extend(b.iter().map(|&x| x as u32));
            tmp.sort_unstable();
            let mut i = 0;
            while i < tmp.len() {
                if i + 1 < tmp.len() && tmp[i] == tmp[i + 1] {
                    i += 2;
                } else {
                    cbits.push(tmp[i]);
                    i += 1;
                }
            }
            cstart.push(cbits.len() as u32);
        });
        let m = cstart.len() - 1;
        let check = |c: usize| &cbits[cstart[c] as usize..cstart[c + 1] as usize];
        // The checks of each parity bit.
        let mut deg = vec![0u32; n + 1];
        for &p in &cbits {
            if !is_info(p as usize) {
                deg[p as usize + 1] += 1;
            }
        }
        for i in 0..n {
            deg[i + 1] += deg[i];
        }
        let vstart = deg;
        let mut fill = vstart.clone();
        let mut vchecks = vec![0u32; vstart[n] as usize];
        for c in 0..m {
            for &p in check(c) {
                if !is_info(p as usize) {
                    vchecks[fill[p as usize] as usize] = c as u32;
                    fill[p as usize] += 1;
                }
            }
        }
        let vdeg = |p: usize| vstart[p + 1] - vstart[p];
        // Greedy triangulation: buckets of checks by their count of unknown parity bits.
        let mut known: Vec<bool> = (0..n).map(is_info).collect();
        let mut used = vec![false; m];
        let mut cnt: Vec<u32> = (0..m)
            .map(|c| check(c).iter().filter(|&&p| !known[p as usize]).count() as u32)
            .collect();
        let maxcnt = cnt.iter().copied().max().unwrap_or(0) as usize;
        let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); maxcnt + 1];
        for c in (0..m).rev() {
            buckets[cnt[c] as usize].push(c as u32);
        }
        let mut solve: Vec<u32> = Vec::new();
        let mut bits: Vec<u32> = Vec::new();
        let mut start = vec![0u32];
        let mut gap: Vec<u32> = Vec::new();
        let singular = FecError::Degenerate {
            param: Param::Checks,
        };
        let mut remaining = n - k;
        while remaining > 0 {
            let mut found = None;
            'scan: for d in 1..=maxcnt {
                while let Some(c) = buckets[d].pop() {
                    let c = c as usize;
                    if !used[c] && cnt[c] as usize == d {
                        found = Some((c, d));
                        break 'scan;
                    }
                }
            }
            let Some((c, d)) = found else {
                // The unknown bits left are in no unused check.
                return Err(singular);
            };
            let unknown = check(c).iter().copied().filter(|&p| !known[p as usize]);
            let p = if d == 1 {
                let p = unknown.clone().next().ok_or(singular)?;
                used[c] = true;
                solve.push(p);
                bits.extend(check(c).iter().filter(|&&x| x != p));
                start.push(bits.len() as u32);
                p
            } else {
                let p = unknown.max_by_key(|&p| vdeg(p as usize)).ok_or(singular)?;
                gap.push(p);
                if gap.len() > MAX_GAP {
                    return Param::Gap.range(0, MAX_GAP as i64, gap.len());
                }
                p
            };
            known[p as usize] = true;
            remaining -= 1;
            for &c2 in &vchecks[vstart[p as usize] as usize..vstart[p as usize + 1] as usize] {
                let c2 = c2 as usize;
                if !used[c2] {
                    cnt[c2] -= 1;
                    buckets[cnt[c2] as usize].push(c2 as u32);
                }
            }
        }
        let left: Vec<usize> = (0..m).filter(|&c| !used[c]).collect();
        let g = gap.len();
        let gw = g.div_ceil(64);
        let mut enc = LdpcEncoder {
            n,
            k,
            info_start: info.start,
            solve,
            bits,
            start,
            gap,
            minv: Vec::new(),
            gw,
        };
        let syn = |v: &[u64], c: usize| check(c).iter().fold(0u64, |a, &p| a ^ v[p as usize]);
        // The gap system: column i of `mat` is the syndrome of the unused checks when gap bit i is 1 and all else is 0.
        let mut mat = vec![0u64; left.len() * gw];
        let mut v = vec![0u64; n];
        for b in 0..gw {
            v.fill(0);
            for t in 0..64.min(g - b * 64) {
                v[enc.gap[b * 64 + t] as usize] = 1 << t;
            }
            enc.run(&mut v);
            for (j, &c) in left.iter().enumerate() {
                mat[j * gw + b] = syn(&v, c);
            }
        }
        // Pick `g` independent unused checks (row reduction that remembers where each row came from).
        let mut work = mat.clone();
        let mut from: Vec<usize> = (0..left.len()).collect();
        for col in 0..g {
            let bit = |w: &[u64], r: usize| w[r * gw + col / 64] >> (col % 64) & 1 == 1;
            let piv = (col..left.len()).find(|&r| bit(&work, r)).ok_or(singular)?;
            for x in 0..gw {
                work.swap(piv * gw + x, col * gw + x);
            }
            from.swap(piv, col);
            for r in col + 1..left.len() {
                if bit(&work, r) {
                    for x in 0..gw {
                        let w = work[col * gw + x];
                        work[r * gw + x] ^= w;
                    }
                }
            }
        }
        // Invert the chosen rows: [M | I] to [I | M^-1].
        let mut aug = vec![0u64; g * 2 * gw];
        for (i, &r) in from[..g].iter().enumerate() {
            aug[i * 2 * gw..i * 2 * gw + gw].copy_from_slice(&mat[r * gw..(r + 1) * gw]);
            aug[i * 2 * gw + gw + i / 64] |= 1 << (i % 64);
        }
        for col in 0..g {
            let bit = |a: &[u64], r: usize| a[r * 2 * gw + col / 64] >> (col % 64) & 1 == 1;
            let piv = (col..g).find(|&r| bit(&aug, r)).ok_or(singular)?;
            for x in 0..2 * gw {
                aug.swap(piv * 2 * gw + x, col * 2 * gw + x);
            }
            for r in 0..g {
                if r != col && bit(&aug, r) {
                    for x in 0..2 * gw {
                        let w = aug[col * 2 * gw + x];
                        aug[r * 2 * gw + x] ^= w;
                    }
                }
            }
        }
        enc.minv = vec![0u64; g * gw];
        for i in 0..g {
            enc.minv[i * gw..(i + 1) * gw].copy_from_slice(&aug[i * 2 * gw + gw..(i + 1) * 2 * gw]);
        }
        for &r in &from[..g] {
            enc.bits.extend_from_slice(check(left[r]));
            enc.start.push(enc.bits.len() as u32);
        }
        // Unused checks beyond the `g` chosen ones must hold for every information word. Each such check is a linear function
        // of the information, so 64 random words find a wrong one with probability 1 - 2^-64.
        if left.len() > g {
            let mut seed = 0x5EED_u64;
            v.fill(0);
            for p in info.clone() {
                v[p] = splitmix(&mut seed);
            }
            enc.run(&mut v);
            let s: Vec<u64> = from[..g].iter().map(|&r| syn(&v, left[r])).collect();
            for i in 0..g {
                let row = &enc.minv[i * gw..(i + 1) * gw];
                let x = (0..g)
                    .filter(|&j| row[j / 64] >> (j % 64) & 1 == 1)
                    .fold(0u64, |a, j| a ^ s[j]);
                v[enc.gap[i] as usize] = x;
            }
            enc.run(&mut v);
            if left.iter().any(|&c| syn(&v, c) != 0) {
                return Param::Checks.degenerate();
            }
        }
        Ok(enc)
    }

    /// Code word length.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Information bits.
    pub fn k(&self) -> usize {
        self.k
    }

    /// The number of parity bits found by the dense solve (0 when the parity part is triangular).
    pub fn gap(&self) -> usize {
        self.gap.len()
    }

    /// Runs the back-substitution order over `v` (one value per code word position).
    fn run<T: Copy + Default + BitXor<Output = T>>(&self, v: &mut [T]) {
        for (i, &b) in self.solve.iter().enumerate() {
            let r = &self.bits[self.start[i] as usize..self.start[i + 1] as usize];
            let mut acc = T::default();
            for &p in r {
                acc = acc ^ v[p as usize];
            }
            v[b as usize] = acc;
        }
    }

    /// Encodes `info` (`k` bits, one per byte) into `out` (`n` bits, code word order, the information at
    /// [`LdpcCode::info_range`]). Allocates nothing. An error when a length is wrong.
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
        for (o, &b) in out[self.info_start..self.info_start + self.k]
            .iter_mut()
            .zip(info)
        {
            *o = b & 1;
        }
        for &p in &self.gap {
            out[p as usize] = 0;
        }
        self.run(out);
        let g = self.gap.len();
        if g == 0 {
            return Ok(());
        }
        let gw = self.gw;
        let steps = self.solve.len();
        let mut s = [0u64; MAX_GAP / 64];
        for j in 0..g {
            let r = &self.bits[self.start[steps + j] as usize..self.start[steps + j + 1] as usize];
            let v = r.iter().fold(0u8, |a, &p| a ^ out[p as usize]);
            s[j / 64] |= (v as u64) << (j % 64);
        }
        let mut any = 0u8;
        for i in 0..g {
            let row = &self.minv[i * gw..(i + 1) * gw];
            let x = row
                .iter()
                .zip(&s[..gw])
                .fold(0u32, |a, (m, v)| a ^ (m & v).count_ones())
                & 1;
            out[self.gap[i] as usize] = x as u8;
            any |= x as u8;
        }
        if any != 0 {
            self.run(out);
        }
        Ok(())
    }
}
