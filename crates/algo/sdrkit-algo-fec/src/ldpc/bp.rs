//! Sum-product (belief propagation) decoding in `f32` for short codes, and ordered-statistics decoding (OSD) after it fails
//! (the WSJT family: R. Gallager, 1963; D. MacKay, *Information Theory, Inference and Learning Algorithms*, chapter 47;
//! M. Fossorier and S. Lin, "Soft-decision decoding of linear block codes based on ordered statistics", IEEE Trans. Inf.
//! Theory 41, 1995).

use super::LdpcCode;
use crate::error::{Decoded, FecError, Param, Result};

/// A flooding sum-product decoder with `f32` messages, sized at construction.
#[derive(Clone, Debug)]
pub struct BpDecoder {
    n: usize,
    /// Code word positions of each edge, grouped by check.
    bit: Vec<u32>,
    check_start: Vec<u32>,
    c2v: Vec<f32>,
    v2c: Vec<f32>,
    total: Vec<f32>,
    fwd: Vec<f32>,
    /// Stop after this many iterations without fewer unsatisfied checks (0: never).
    patience: u32,
    last_iterations: u32,
}

impl BpDecoder {
    /// A decoder for `code`.
    pub fn new(code: &LdpcCode) -> Self {
        let mut bit = Vec::new();
        let mut check_start = vec![0u32];
        let mut maxdeg = 0;
        code.for_each_check(|b| {
            bit.extend(b.iter().map(|&x| x as u32));
            check_start.push(bit.len() as u32);
            maxdeg = maxdeg.max(b.len());
        });
        let e = bit.len();
        BpDecoder {
            n: code.n(),
            bit,
            check_start,
            c2v: vec![0.0; e],
            v2c: vec![0.0; e],
            total: vec![0.0; code.n()],
            fwd: vec![0.0; maxdeg + 1],
            patience: 0,
            last_iterations: 0,
        }
    }

    /// The same decoder with early stop: [`BpDecoder::decode`] stops when `patience` iterations in a row do not bring the
    /// number of unsatisfied checks below the lowest number seen so far. A `patience` of 0 turns early stop off (the default).
    /// When a decode with early stop succeeds, its result is the same as without early stop. A decode that converges only after
    /// a long stall fails with early stop.
    pub fn with_early_stop(mut self, patience: u32) -> Self {
        self.patience = patience;
        self
    }

    /// Sets the early stop patience of [`BpDecoder::with_early_stop`] (0 turns it off).
    pub fn set_early_stop(&mut self, patience: u32) {
        self.patience = patience;
    }

    /// The iterations the last decode used, also when it failed or stopped early.
    pub fn last_iterations(&self) -> u32 {
        self.last_iterations
    }

    /// The number of checks the hard decisions of the posteriors do not satisfy.
    fn unsatisfied(&self) -> u32 {
        (0..self.check_start.len() - 1)
            .map(|c| {
                let (a, b) = (
                    self.check_start[c] as usize,
                    self.check_start[c + 1] as usize,
                );
                self.bit[a..b]
                    .iter()
                    .fold(0u32, |s, &x| s ^ (self.total[x as usize] < 0.0) as u32)
            })
            .sum()
    }

    fn checks_ok(&self) -> bool {
        (0..self.check_start.len() - 1).all(|c| {
            let (a, b) = (
                self.check_start[c] as usize,
                self.check_start[c + 1] as usize,
            );
            self.bit[a..b]
                .iter()
                .fold(0u8, |s, &x| s ^ (self.total[x as usize] < 0.0) as u8)
                == 0
        })
    }

    /// Decodes `llr` (`f32`, positive means 0) with at most `max_iter` iterations into `out` (hard bits). The posterior LLRs stay
    /// available through [`BpDecoder::posterior`]. On failure `out` holds the last hard decisions and the result is
    /// `Err(Uncorrectable)`. With early stop ([`BpDecoder::with_early_stop`]) a decode can also fail before `max_iter`;
    /// [`BpDecoder::last_iterations`] returns the number of iterations used.
    pub fn decode(&mut self, llr: &[f32], max_iter: u32, out: &mut [u8]) -> Result<Decoded> {
        if llr.len() != self.n {
            return Err(FecError::InputLength {
                expected: self.n,
                got: llr.len(),
            });
        }
        if out.len() < self.n {
            return Err(FecError::OutputTooShort { need: self.n });
        }
        crate::llr::check_finite(llr)?;
        self.total.copy_from_slice(llr);
        for (v, &b) in self.v2c.iter_mut().zip(&self.bit) {
            *v = llr[b as usize];
        }
        self.c2v.fill(0.0);
        let mut used = 0;
        let mut ok = self.checks_ok();
        let mut best = u32::MAX;
        let mut stalled = 0u32;
        if self.patience > 0 && !ok {
            best = self.unsatisfied();
        }
        while !ok && used < max_iter {
            // Check nodes: c2v = 2 atanh(Π tanh(v2c / 2)) over the other edges, by forward and backward products.
            for c in 0..self.check_start.len() - 1 {
                let (a, b) = (
                    self.check_start[c] as usize,
                    self.check_start[c + 1] as usize,
                );
                self.fwd[0] = 1.0;
                for (i, e) in (a..b).enumerate() {
                    self.fwd[i + 1] = self.fwd[i] * (0.5 * self.v2c[e]).tanh();
                }
                let mut back = 1.0f32;
                for (i, e) in (a..b).enumerate().rev() {
                    let p = (self.fwd[i] * back).clamp(-0.999_999, 0.999_999);
                    back *= (0.5 * self.v2c[e]).tanh();
                    self.c2v[e] = 2.0 * p.atanh();
                }
            }
            // Bit nodes.
            self.total.copy_from_slice(llr);
            for (e, &b) in self.bit.iter().enumerate() {
                self.total[b as usize] += self.c2v[e];
            }
            for (e, &b) in self.bit.iter().enumerate() {
                self.v2c[e] = self.total[b as usize] - self.c2v[e];
            }
            used += 1;
            if self.patience > 0 {
                let u = self.unsatisfied();
                ok = u == 0;
                if u < best {
                    best = u;
                    stalled = 0;
                } else {
                    stalled += 1;
                    if !ok && stalled >= self.patience {
                        break;
                    }
                }
            } else {
                ok = self.checks_ok();
            }
        }
        self.last_iterations = used;
        let mut corrected = 0;
        for (i, o) in out[..self.n].iter_mut().enumerate() {
            *o = (self.total[i] < 0.0) as u8;
            if llr[i] != 0.0 && ((llr[i] < 0.0) as u8) != *o {
                corrected += 1;
            }
        }
        if ok {
            Ok(Decoded {
                corrected,
                metric: 0,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }

    /// The posterior LLRs of the last decode.
    pub fn posterior(&self) -> &[f32] {
        &self.total
    }
}

/// Ordered-statistics decoding of a short code (`n ≤ 256`) with a dense encoder: Gaussian elimination on the most reliable
/// positions, then the re-encoded hard decisions with every single flip (order 1) and pairs among the 16 least reliable basis
/// bits (order 2), keeping the code word nearest the input in soft distance.
#[derive(Clone, Debug)]
pub struct Osd {
    n: usize,
    k: usize,
    w: usize,
    /// Generator rows (k x n bits).
    g: Vec<u64>,
    m: Vec<u64>,
    order: Vec<u16>,
    piv: Vec<u16>,
    hard: Vec<u64>,
    cand: Vec<u64>,
    best: Vec<u64>,
}

impl Osd {
    /// The decoder for `code`; an error when `n > 256` or the code has no dense encoder.
    pub fn new(code: &LdpcCode) -> Result<Self> {
        let (n, k) = (code.n(), code.k());
        if n > 256 {
            return Param::N.range(1, 256, n);
        }
        let (a, aw) = code.parity_rows().ok_or(FecError::Unsupported {
            param: Param::Encoder,
        })?;
        let info = code.info_range();
        let parity: Vec<usize> = (0..n).filter(|i| !info.contains(i)).collect();
        let w = n.div_ceil(64);
        let mut g = vec![0u64; k * w];
        for i in 0..k {
            let p = info.start + i;
            g[i * w + p / 64] |= 1 << (p % 64);
            for (j, &pp) in parity.iter().enumerate() {
                if a[j * aw + i / 64] >> (i % 64) & 1 == 1 {
                    g[i * w + pp / 64] |= 1 << (pp % 64);
                }
            }
        }
        Ok(Osd {
            n,
            k,
            w,
            m: g.clone(),
            g,
            order: (0..n as u16).collect(),
            piv: vec![0; k],
            hard: vec![0; w],
            cand: vec![0; w],
            best: vec![0; w],
        })
    }

    fn cost(&self, c: &[u64], llr: &[f32]) -> f32 {
        let mut s = 0.0;
        for (wi, (&x, &h)) in c.iter().zip(&self.hard).enumerate() {
            let mut d = x ^ h;
            while d != 0 {
                let b = d.trailing_zeros() as usize;
                s += llr[wi * 64 + b].abs();
                d &= d - 1;
            }
        }
        s
    }

    /// Decodes `llr` (`f32`, positive means 0; posteriors from [`BpDecoder`] work well) with search order 0, 1 or 2 into `out`.
    /// `metric` is the soft distance of the chosen code word scaled by 100; `corrected` the hard decisions it changes.
    pub fn decode(&mut self, llr: &[f32], order: u8, out: &mut [u8]) -> Result<Decoded> {
        let (n, k, w) = (self.n, self.k, self.w);
        if llr.len() != n {
            return Err(FecError::InputLength {
                expected: n,
                got: llr.len(),
            });
        }
        if out.len() < n {
            return Err(FecError::OutputTooShort { need: n });
        }
        crate::llr::check_finite(llr)?;
        self.hard.fill(0);
        for (i, &v) in llr.iter().enumerate() {
            if v < 0.0 {
                self.hard[i / 64] |= 1 << (i % 64);
            }
        }
        // Most reliable first (insertion sort keeps this allocation-free; n is small).
        for (i, o) in self.order.iter_mut().enumerate() {
            *o = i as u16;
        }
        for i in 1..n {
            let mut j = i;
            while j > 0 && llr[self.order[j] as usize].abs() > llr[self.order[j - 1] as usize].abs()
            {
                self.order.swap(j, j - 1);
                j -= 1;
            }
        }
        self.m.copy_from_slice(&self.g);
        let mut rank = 0;
        for oi in 0..n {
            if rank == k {
                break;
            }
            let c = self.order[oi] as usize;
            let Some(p) = (rank..k).find(|&r| self.m[r * w + c / 64] >> (c % 64) & 1 == 1) else {
                continue;
            };
            for x in 0..w {
                self.m.swap(p * w + x, rank * w + x);
            }
            for r in 0..k {
                if r != rank && self.m[r * w + c / 64] >> (c % 64) & 1 == 1 {
                    for x in 0..w {
                        let v = self.m[rank * w + x];
                        self.m[r * w + x] ^= v;
                    }
                }
            }
            self.piv[rank] = c as u16;
            rank += 1;
        }
        if rank < k {
            return Err(FecError::Uncorrectable);
        }
        // Re-encode the hard decisions of the basis: row r is selected by the hard bit of its pivot column.
        self.cand.fill(0);
        for r in 0..k {
            let pc = self.piv[r] as usize;
            if self.hard[pc / 64] >> (pc % 64) & 1 == 1 {
                for x in 0..w {
                    self.cand[x] ^= self.m[r * w + x];
                }
            }
        }
        self.best.copy_from_slice(&self.cand);
        let mut best_cost = self.cost(&self.cand, llr);
        if order >= 1 {
            for r in 0..k {
                for x in 0..w {
                    self.cand[x] ^= self.m[r * w + x];
                }
                let c = self.cost(&self.cand, llr);
                if c < best_cost {
                    best_cost = c;
                    self.best.copy_from_slice(&self.cand);
                }
                for x in 0..w {
                    self.cand[x] ^= self.m[r * w + x];
                }
            }
        }
        if order >= 2 {
            let lo = k.saturating_sub(16);
            for r1 in lo..k {
                for r2 in r1 + 1..k {
                    for x in 0..w {
                        self.cand[x] ^= self.m[r1 * w + x] ^ self.m[r2 * w + x];
                    }
                    let c = self.cost(&self.cand, llr);
                    if c < best_cost {
                        best_cost = c;
                        self.best.copy_from_slice(&self.cand);
                    }
                    for x in 0..w {
                        self.cand[x] ^= self.m[r1 * w + x] ^ self.m[r2 * w + x];
                    }
                }
            }
        }
        let mut corrected = 0;
        for (i, o) in out[..n].iter_mut().enumerate() {
            *o = (self.best[i / 64] >> (i % 64)) as u8 & 1;
            corrected += (*o != (self.hard[i / 64] >> (i % 64)) as u8 & 1) as u32;
        }
        Ok(Decoded {
            corrected,
            metric: (best_cost * 100.0).min(i32::MAX as f32) as i32,
            iterations: 1,
        })
    }
}
